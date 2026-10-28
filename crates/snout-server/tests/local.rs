//! The whole stack on this machine, without ssh: the client talks to `snout-server --stdio`
//! as ssh would start it, and that proxy to the daemon it starts.

use snout_remote::Client;
use snout_remote::proto::{FileKind, Pipes, PtyRequest, PtySize, Signal, SpawnRequest, WatchKind};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SERVER: &str = env!("CARGO_BIN_EXE_snout-server");

/// A scratch home and server folder, and the daemon stopped at the end.
struct Host {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    dir: PathBuf,
    idle: u64,
}

impl Host {
    fn new(idle: u64) -> Host {
        // Short: the daemon's socket path must fit in 104 bytes.
        let tmp = tempfile::Builder::new().prefix("snt").tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        Host { dir: root.join("srv"), home, _tmp: tmp, idle }
    }

    fn command(&self) -> Command {
        let mut c = Command::new(SERVER);
        c.arg("--dir").arg(&self.dir).env("HOME", &self.home).env_remove("XDG_DATA_HOME");
        c
    }

    /// Connect as ssh would: through `--stdio`. Also returns the proxy, to kill it.
    fn connect(&self) -> (Client, Arc<Mutex<Child>>) {
        let mut child = self
            .command()
            .arg("--stdio")
            .arg("--idle-timeout")
            .arg(self.idle.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let child = Arc::new(Mutex::new(child));
        let c = child.clone();
        let client = Client::connect(stdout, stdin, move || {
            let mut c = c.lock().unwrap();
            let _ = c.kill();
            let _ = c.wait();
        })
        .unwrap();
        (client, child)
    }

    fn daemon_pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.dir.join("daemon.pid")).ok()?.trim().parse().ok()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let pid = self.daemon_pid();
        let _ = self.command().arg("--stop").output();
        if let Some(pid) = pid {
            let start = Instant::now();
            while alive(pid) && start.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn wait_until(what: &str, timeout: Duration, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Read from `r` until the text seen so far contains `needle`.
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

fn spawn(program: &str, args: &[&str], cwd: Option<&Path>) -> SpawnRequest {
    SpawnRequest {
        program: program.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: cwd.map(Path::to_path_buf),
        ..Default::default()
    }
}

#[test]
fn files() {
    let host = Host::new(600);
    let (c, _) = host.connect();
    let info = c.info().unwrap();
    assert_eq!(info.home, host.home);
    assert_eq!(info.os, std::env::consts::OS);
    let root = host.home.join("proj");
    c.create_dir_all(root.join("src/main")).unwrap();
    let f = root.join("src/main/A.java");
    c.write(&f, "class A {}").unwrap();
    assert_eq!(c.read_to_string(&f).unwrap(), "class A {}");
    let big: Vec<u8> = (0..5_000_000u32).map(|i| (i % 251) as u8).collect();
    c.write(root.join("big.bin"), big.clone()).unwrap();
    assert_eq!(c.read(root.join("big.bin")).unwrap(), big);
    let m = c.metadata(&f, true).unwrap();
    assert_eq!((m.kind, m.len), (FileKind::File, 10));
    assert!(m.modified.is_some());
    c.rename(&f, root.join("src/main/B.java")).unwrap();
    assert!(!c.exists(&f).unwrap());
    c.copy(root.join("src"), root.join("src2"), true).unwrap();
    let mut names: Vec<_> = c.read_dir(&root).unwrap().into_iter().map(|e| (e.name, e.kind)).collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        names,
        [("big.bin".into(), FileKind::File), ("src".into(), FileKind::Dir), ("src2".into(), FileKind::Dir)]
    );
    assert_eq!(c.canonicalize(root.join("src/../src2")).unwrap(), root.join("src2"));
    c.remove(root.join("src2"), true).unwrap();
    // Errors keep their kind and name the path.
    let e = c.read(root.join("missing")).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    assert!(e.to_string().contains("missing"), "{e}");
    // The trash: the file goes, with its record of where it was.
    c.trash(root.join("big.bin")).unwrap();
    assert!(!c.exists(root.join("big.bin")).unwrap());
    let trash = host.home.join(".local/share/Trash");
    assert!(trash.join("files/big.bin").is_file());
    let record = std::fs::read_to_string(trash.join("info/big.bin.trashinfo")).unwrap();
    assert!(record.contains(&format!("Path={}", root.join("big.bin").display())), "{record}");
}

#[test]
fn watching() {
    let host = Host::new(600);
    let (c, _) = host.connect();
    let root = host.home.join("w");
    c.create_dir_all(root.join("sub")).unwrap();
    let watch = c.watch(&root, true).unwrap();
    // Let the watcher settle (FSEvents reports from a little before it started).
    std::thread::sleep(Duration::from_millis(300));
    while watch.next(Some(Duration::from_millis(200))).is_ok() {}
    c.write(root.join("sub/new.txt"), "x").unwrap();
    let want = root.join("sub/new.txt");
    let start = Instant::now();
    loop {
        let events = watch.next(Some(Duration::from_secs(10))).unwrap().unwrap();
        if events.iter().any(|e| e.paths.iter().any(|p| p.ends_with("sub/new.txt")) && e.kind != WatchKind::Access) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no event for {}", want.display());
    }
}

#[test]
fn processes_git_ptys_and_tunnels() {
    let host = Host::new(600);
    let (c, _) = host.connect();
    let repo = host.home.join("repo");
    c.create_dir_all(&repo).unwrap();

    // git, as the Git panel will run it.
    let git = |args: &[&str]| {
        let p = c.spawn(spawn("git", args, Some(&repo)), Pipes { stdin: false, stdout: true, stderr: true }).unwrap();
        let mut out = String::new();
        p.stdout.clone().unwrap().read_to_string(&mut out).unwrap();
        assert!(p.wait().unwrap().success(), "git {args:?}");
        out
    };
    git(&["init", "-q"]);
    c.write(repo.join("a.txt"), "a").unwrap();
    assert!(git(&["status", "--porcelain"]).contains("?? a.txt"));

    // stdin to stdout, and stdin's end is the process's end of input.
    let mut cat = c.spawn(spawn("cat", &[], None), Pipes::ALL).unwrap();
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 253) as u8).collect();
    let mut stdin = cat.stdin.clone().unwrap();
    let d = data.clone();
    let feeder = std::thread::spawn(move || stdin.write_all(&d).unwrap());
    let mut back: Vec<u8> = Vec::new();
    let mut out = cat.stdout.clone().unwrap();
    let mut buf = vec![0; 65536];
    while back.len() < data.len() {
        let n = out.read(&mut buf).unwrap();
        back.extend(&buf[..n]);
    }
    feeder.join().unwrap();
    assert_eq!(back, data);
    cat.close_stdin().unwrap();
    assert!(cat.wait().unwrap().success());

    // A shell on a pty: input, output, resize.
    let pty = c
        .spawn_pty(PtyRequest {
            program: "/bin/sh".into(),
            size: PtySize { rows: 30, cols: 100 },
            ..Default::default()
        })
        .unwrap();
    let mut io = pty.io.clone();
    io.write_all(b"echo hi-$((40+2))\n").unwrap();
    read_until(&mut io, "hi-42");
    pty.resize(PtySize { rows: 50, cols: 132 }).unwrap();
    io.write_all(b"stty size\n").unwrap();
    read_until(&mut io, "50 132");
    io.write_all(b"exit 3\n").unwrap();
    assert_eq!(pty.wait().unwrap().code, Some(3));

    // A TCP port on the server's side.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        loop {
            let n = s.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            s.write_all(&buf[..n].to_ascii_uppercase()).unwrap();
        }
    });
    let mut tunnel = c.connect_tcp(port).unwrap();
    tunnel.write_all(b"dap hello").unwrap();
    let mut got = [0u8; 9];
    tunnel.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"DAP HELLO");
    tunnel.send_eof().unwrap();
    assert_eq!(tunnel.read(&mut got).unwrap(), 0);
    assert_eq!(c.connect_tcp(1).unwrap_err().kind(), std::io::ErrorKind::ConnectionRefused);

    // Signals go to the process group.
    let sleeper = c.spawn(spawn("sh", &["-c", "sleep 100 & sleep 100; wait"], None), Pipes::default()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    sleeper.kill(Signal::Term).unwrap();
    assert_eq!(sleeper.wait().unwrap().signal, Some(libc::SIGTERM));
}

#[test]
fn processes_outlive_a_dropped_connection_and_attach_again() {
    let host = Host::new(600);
    let (c, proxy) = host.connect();
    let ticker = c
        .spawn(
            spawn("sh", &["-c", "i=0; while true; do echo tick $i; i=$((i+1)); sleep 0.05; done"], None),
            Pipes { stdin: false, stdout: true, stderr: false },
        )
        .unwrap();
    let mut out = ticker.stdout.clone().unwrap();
    read_until(&mut out, "tick 2");
    let pty = c.spawn_pty(PtyRequest { program: "/bin/sh".into(), ..Default::default() }).unwrap();
    let mut io = pty.io.clone();
    io.write_all(b"X=remembered\n").unwrap();
    let (ticker_id, pty_id, daemon) = (ticker.id, pty.id, host.daemon_pid().unwrap());

    // The link drops: ssh (here the proxy) dies.
    proxy.lock().unwrap().kill().unwrap();
    wait_until("the connection to drop", Duration::from_secs(5), || !c.is_alive());
    let e = loop {
        if let Err(e) = out.read(&mut [0u8; 4096]) {
            break e;
        }
    };
    assert_eq!(e.kind(), std::io::ErrorKind::ConnectionAborted);
    std::thread::sleep(Duration::from_millis(300));

    // A new connection finds them running, in the same daemon.
    let (c2, _) = host.connect();
    assert_eq!(host.daemon_pid(), Some(daemon));
    let list = c2.list_processes().unwrap();
    let t = list.iter().find(|p| p.id == ticker_id).unwrap();
    assert!(t.exit.is_none() && !t.attached);
    let again = c2.attach(ticker_id, Pipes { stdin: false, stdout: true, stderr: false }).unwrap();
    let mut out = again.stdout.clone().unwrap();
    // What it printed while nobody listened is kept, and it carries on.
    let text = read_until(&mut out, "tick 30");
    assert!(text.starts_with("tick "), "{text}");
    assert!(c2.list_processes().unwrap().iter().find(|p| p.id == ticker_id).unwrap().attached);

    let pty = c2.attach_pty(pty_id).unwrap();
    let mut io = pty.io.clone();
    io.write_all(b"echo X=$X\n").unwrap();
    read_until(&mut io, "X=remembered");

    again.kill(Signal::Kill).unwrap();
    assert_eq!(again.wait().unwrap().signal, Some(libc::SIGKILL));
    pty.kill(Signal::Kill).unwrap();
    pty.wait().unwrap();
    assert!(c2.list_processes().unwrap().is_empty());
}

#[test]
fn the_daemon_stops_when_idle_and_takes_its_processes() {
    let host = Host::new(1);
    let (c, _) = host.connect();
    let p = c.spawn(spawn("sleep", &["1000"], None), Pipes::default()).unwrap();
    let (daemon, pid) = (host.daemon_pid().unwrap(), p.pid as i32);
    assert!(alive(pid));
    c.close();
    drop(p);
    drop(c);
    wait_until("the daemon to stop", Duration::from_secs(10), || !alive(daemon));
    wait_until("its process to stop", Duration::from_secs(5), || !alive(pid));
    assert!(!host.dir.join("daemon.sock").exists());
}

#[test]
fn a_mismatched_client_is_refused() {
    let host = Host::new(600);
    // Start the daemon, then speak to its socket with another version.
    let (c, _) = host.connect();
    drop(c);
    let mut sock = std::os::unix::net::UnixStream::connect(host.dir.join("daemon.sock")).unwrap();
    let hello = serde_json::json!({"magic": "snout-remote", "protocol": 1, "version": "0.0.1"});
    let mut frame = Vec::new();
    let payload = serde_json::to_vec(&hello).unwrap();
    frame.extend(((6 + payload.len()) as u32).to_be_bytes());
    frame.extend(0u32.to_be_bytes());
    frame.extend([0u8, 1]);
    frame.extend(payload);
    sock.write_all(&frame).unwrap();
    let mut reply = Vec::new();
    sock.read_to_end(&mut reply).unwrap();
    let text = String::from_utf8_lossy(&reply);
    assert!(text.contains("version mismatch"), "{text}");
}

#[test]
fn a_terminated_daemon_stops_its_processes() {
    let host = Host::new(600);
    let (c, _) = host.connect();
    let p = c.spawn(spawn("sh", &["-c", "sleep 1000 & sleep 1000; wait"], None), Pipes::default()).unwrap();
    let (daemon, pid) = (host.daemon_pid().unwrap(), p.pid as i32);
    std::thread::sleep(Duration::from_millis(100));
    // SAFETY: plain kill(2).
    unsafe { libc::kill(daemon, libc::SIGTERM) };
    wait_until("the daemon to stop", Duration::from_secs(10), || !alive(daemon));
    wait_until("its process to stop", Duration::from_secs(5), || !alive(pid));
    // The background sleep of the process's group went too.
    let ps = Command::new("pgrep").args(["-g", &pid.to_string()]).output().unwrap();
    assert!(String::from_utf8_lossy(&ps.stdout).trim().is_empty());
    assert!(!c.is_alive() || c.ping(Duration::from_secs(1)).is_err());
}
