//! End to end against a real Linux machine: an Ubuntu container with sshd (see
//! `tests/docker/Dockerfile`), reached with the system's ssh, a scratch key and a scratch
//! ssh config (never the user's own).
//!
//! Runs only with `SNOUT_REMOTE_DOCKER_TEST=1` (it needs Docker, and the Linux server
//! binaries, which it builds with `scripts/build-server-linux.sh` when they are missing):
//!
//! ```sh
//! SNOUT_REMOTE_DOCKER_TEST=1 cargo test -p snout-server --test docker -- --nocapture
//! ```

use snout_remote::proto::{Pipes, PtyRequest, PtySize, Signal, SpawnRequest, WatchKind};
use snout_remote::{Client, RemoteSession, SshTarget};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const IMAGE: &str = "snout-remote-test:2";

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn run(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(out.status.success(), "{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// The container, removed at the end.
struct Container(String);

impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
    }
}

fn binaries() -> PathBuf {
    let dist = std::env::var_os("SNOUT_SERVER_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target/snout-server-dist"));
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    if !dist.join(format!("snout-server-linux-{arch}")).is_file() {
        run(Command::new(workspace().join("scripts/build-server-linux.sh")).arg(arch));
    }
    dist
}

fn read_until(r: &mut impl Read, needle: &str) -> String {
    let mut seen = Vec::new();
    let mut buf = [0u8; 4096];
    while !String::from_utf8_lossy(&seen).contains(needle) {
        let n = r.read(&mut buf).unwrap();
        assert!(n > 0, "the stream ended before {needle:?}; got {:?}", String::from_utf8_lossy(&seen));
        seen.extend(&buf[..n]);
    }
    String::from_utf8_lossy(&seen).into_owned()
}

fn spawn(program: &str, args: &[&str], cwd: Option<&str>) -> SpawnRequest {
    SpawnRequest {
        program: program.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: cwd.map(PathBuf::from),
        ..Default::default()
    }
}

fn output(c: &Client, program: &str, args: &[&str], cwd: Option<&str>) -> String {
    let p = c.spawn(spawn(program, args, cwd), Pipes { stdin: false, stdout: true, stderr: true }).unwrap();
    let mut out = String::new();
    p.stdout.clone().unwrap().read_to_string(&mut out).unwrap();
    let mut err = String::new();
    p.stderr.clone().unwrap().read_to_string(&mut err).unwrap();
    assert!(p.wait().unwrap().success(), "{program} {args:?}: {err}");
    out
}

#[test]
fn remote_ssh_against_a_linux_container() {
    if std::env::var_os("SNOUT_REMOTE_DOCKER_TEST").is_none() {
        eprintln!("skipped: set SNOUT_REMOTE_DOCKER_TEST=1 to run it (needs Docker)");
        return;
    }
    let dist = binaries();

    // A scratch key and ssh config. The folder name stays short for ssh's control socket.
    let scratch = tempfile::Builder::new().prefix("snt").tempdir().unwrap();
    let dir = scratch.path();
    let key = dir.join("key");
    run(Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", "snout-test", "-f"]).arg(&key));
    let public = std::fs::read_to_string(dir.join("key.pub")).unwrap();

    run(Command::new("docker")
        .args(["build", "-q", "-t", IMAGE])
        .arg(workspace().join("crates/snout-server/tests/docker")));
    let name = format!("snout-remote-test-{}", std::process::id());
    let container = Container(name.clone());
    run(Command::new("docker")
        .args(["run", "-d", "--rm", "--name", &name, "-p", "127.0.0.1::22", "-e"])
        .arg(format!("AUTHORIZED_KEY={}", public.trim()))
        .arg(IMAGE));
    let port = run(Command::new("docker").args(["port", &name, "22"]));
    let port = port.lines().next().unwrap().rsplit(':').next().unwrap().to_owned();

    let config = dir.join("ssh_config");
    std::fs::write(
        &config,
        format!(
            "Host snout-test\n  HostName 127.0.0.1\n  Port {port}\n  User dev\n  IdentityFile {}\n  IdentitiesOnly yes\n  \
             IdentityAgent none\n  StrictHostKeyChecking no\n  UserKnownHostsFile /dev/null\n  LogLevel ERROR\n",
            key.display()
        ),
    )
    .unwrap();
    let mut target = SshTarget::new("snout-test");
    target.config_file = Some(config.clone());
    target.control_dir = Some(dir.join("c"));
    target.connect_timeout = Duration::from_secs(20);

    // sshd takes a moment to come up.
    let start = Instant::now();
    while !target.command("true").output().map(|o| o.status.success()).unwrap_or(false) {
        assert!(start.elapsed() < Duration::from_secs(30), "sshd did not come up");
        std::thread::sleep(Duration::from_millis(200));
    }

    // Install: uploaded the first time, found by its checksum the second.
    let session = RemoteSession::open(target.clone(), &dist).unwrap();
    assert!(session.installed().uploaded);
    assert!(!snout_remote::install(&target, &dist).unwrap().uploaded);
    let c = session.client().unwrap();
    let info = c.info().unwrap();
    assert_eq!(info.os, "linux");
    assert_eq!(info.arch, std::env::consts::ARCH);
    assert_eq!(info.home, Path::new("/home/dev"));
    assert_eq!(info.user, "dev");
    eprintln!(
        "connected to {}@{} ({} {}), ping {:?}",
        info.user,
        info.hostname,
        info.os,
        info.arch,
        c.ping(Duration::from_secs(5)).unwrap()
    );

    // Files.
    let proj = "/home/dev/proj";
    c.create_dir_all(format!("{proj}/src")).unwrap();
    c.write(format!("{proj}/src/A.java"), "class A {}").unwrap();
    assert_eq!(c.read_to_string(format!("{proj}/src/A.java")).unwrap(), "class A {}");
    let big: Vec<u8> = (0..8_000_000u32).map(|i| (i % 251) as u8).collect();
    let t = Instant::now();
    c.write(format!("{proj}/big.bin"), big.clone()).unwrap();
    assert_eq!(c.read(format!("{proj}/big.bin")).unwrap(), big);
    eprintln!("8 MB written and read back in {:?}", t.elapsed());
    assert_eq!(c.metadata(format!("{proj}/src/A.java"), true).unwrap().len, 10);
    c.trash(format!("{proj}/big.bin")).unwrap();
    assert!(c.exists("/home/dev/.local/share/Trash/files/big.bin").unwrap());

    // Watching (inotify).
    let watch = c.watch(proj, true).unwrap();
    c.write(format!("{proj}/src/B.java"), "class B {}").unwrap();
    let start = Instant::now();
    'seen: loop {
        let events = watch.next(Some(Duration::from_secs(10))).unwrap().unwrap();
        for e in events {
            if e.kind != WatchKind::Access && e.paths.iter().any(|p| p.ends_with("B.java")) {
                break 'seen;
            }
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no event for B.java");
    }

    // git.
    output(&c, "git", &["init", "-q"], Some(proj));
    let status = output(&c, "git", &["status", "--porcelain"], Some(proj));
    assert!(status.contains("?? src/"), "{status}");

    // A process with stdin and stdout.
    let mut cat = c.spawn(spawn("cat", &[], None), Pipes::ALL).unwrap();
    cat.stdin.clone().unwrap().write_all(b"through cat\n").unwrap();
    read_until(&mut cat.stdout.clone().unwrap(), "through cat");
    cat.close_stdin().unwrap();
    assert!(cat.wait().unwrap().success());

    // A pty.
    let pty = c
        .spawn_pty(PtyRequest {
            program: "/bin/bash".into(),
            size: PtySize { rows: 30, cols: 90 },
            ..Default::default()
        })
        .unwrap();
    let mut io = pty.io.clone();
    io.write_all(b"echo hi-$((40+2)); stty size\n").unwrap();
    read_until(&mut io, "30 90");
    pty.resize(PtySize { rows: 40, cols: 120 }).unwrap();
    io.write_all(b"stty size\n").unwrap();
    read_until(&mut io, "40 120");
    io.write_all(b"exit 4\n").unwrap();
    assert_eq!(pty.wait().unwrap().code, Some(4));

    // A TCP tunnel to a port on the remote's loopback (a debugger's DAP port).
    let nc = c.spawn(spawn("nc", &["-l", "127.0.0.1", "4555"], None), Pipes::ALL).unwrap();
    let start = Instant::now();
    let mut tunnel = loop {
        match c.connect_tcp(4555) {
            Ok(t) => break t,
            Err(e) if start.elapsed() > Duration::from_secs(5) => panic!("cannot reach the listener: {e}"),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    tunnel.write_all(b"to the remote\n").unwrap();
    read_until(&mut nc.stdout.clone().unwrap(), "to the remote");
    nc.stdin.clone().unwrap().write_all(b"from the remote\n").unwrap();
    read_until(&mut tunnel, "from the remote");
    drop(tunnel);
    nc.kill(Signal::Kill).unwrap();
    nc.wait().unwrap();

    // Processes that must outlive the connection: an engine-like process and a terminal.
    let ticker = c
        .spawn(
            spawn("sh", &["-c", "i=0; while true; do echo tick $i; i=$((i+1)); sleep 0.1; done"], None),
            Pipes { stdin: false, stdout: true, stderr: false },
        )
        .unwrap();
    read_until(&mut ticker.stdout.clone().unwrap(), "tick 1");
    let term = c.spawn_pty(PtyRequest { program: "/bin/bash".into(), ..Default::default() }).unwrap();
    term.io.clone().write_all(b"export KEPT=across-the-drop\n").unwrap();
    let (ticker_id, term_id, daemon_pid) = (ticker.id, term.id, info.server_pid);

    // The link drops: the shared ssh connection goes away, and the session with it.
    target.close_master();
    let start = Instant::now();
    while c.is_alive() {
        assert!(start.elapsed() < Duration::from_secs(10), "the client did not notice the drop");
        std::thread::sleep(Duration::from_millis(50));
    }
    drop((ticker, term, watch, c));
    std::thread::sleep(Duration::from_secs(1));

    // Reconnect: the same daemon, the processes still there.
    let c = session.client().unwrap();
    assert_eq!(c.info().unwrap().server_pid, daemon_pid);
    let list = c.list_processes().unwrap();
    assert!(list.iter().any(|p| p.id == ticker_id && p.exit.is_none()), "{list:?}");
    assert!(list.iter().any(|p| p.id == term_id && p.exit.is_none()), "{list:?}");
    let ticker = c.attach(ticker_id, Pipes { stdin: false, stdout: true, stderr: false }).unwrap();
    let text = read_until(&mut ticker.stdout.clone().unwrap(), "tick 40");
    assert!(text.starts_with("tick "), "{text}");
    let term = c.attach_pty(term_id).unwrap();
    let mut io = term.io.clone();
    io.write_all(b"echo KEPT=$KEPT\n").unwrap();
    read_until(&mut io, "KEPT=across-the-drop");
    // The old session's proxy is gone; only the new one runs.
    let proxies = output(&c, "pgrep", &["-fc", "snout-server --stdio"], None);
    assert_eq!(proxies.trim(), "1");

    // Stopping the server stops what it runs.
    ticker.kill(Signal::Term).unwrap();
    assert_eq!(ticker.wait().unwrap().signal, Some(15));
    c.shutdown_server().unwrap();
    let start = Instant::now();
    while c.is_alive() {
        assert!(start.elapsed() < Duration::from_secs(10), "the server did not stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    let left = run(Command::new("docker").args(["exec", &name, "sh", "-c", "pgrep -u dev -fa 'bash|sh -c' || true"]));
    assert!(left.is_empty(), "left running: {left}");
    target.close_master();
    drop(container);
}
