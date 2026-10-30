//! `RemoteHost`, the app's `Host` on a server, against `snout-server --stdio` on this
//! machine (as ssh would start it): the same calls the app makes, and what happens to them
//! when the connection drops.

#![cfg(unix)]

use snout_host::{Cmd, Host, Stdio, WalkOptions, WatchKind};
use snout_remote::{Backoff, Client, Connector, RemoteHost, Status};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SERVER: &str = env!("CARGO_BIN_EXE_snout-server");

/// Connects as ssh would, to a server with a scratch home; can be made to fail, and to
/// drop its connection (the proxy killed, as ssh dies with the network).
struct Local {
    home: PathBuf,
    dir: PathBuf,
    proxy: Mutex<Option<Child>>,
    refuse: AtomicBool,
}

impl Local {
    fn command(&self) -> Command {
        let mut c = Command::new(SERVER);
        c.arg("--dir")
            .arg(&self.dir)
            .env("HOME", &self.home)
            .env("SHELL", "/bin/sh")
            .env("SNOUT_TEST_VAR", "from-login");
        c
    }

    fn kill_proxy(&self) {
        if let Some(mut c) = self.proxy.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Connector for Local {
    fn connect(&self) -> io::Result<Client> {
        if self.refuse.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::ConnectionRefused, "refused for the test"));
        }
        let mut child = self
            .command()
            .args(["--stdio", "--idle-timeout", "600"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let pid = child.id();
        *self.proxy.lock().unwrap() = Some(child);
        Client::connect(stdout, stdin, move || {
            // SAFETY: plain kill(2) of the proxy we started.
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        })
    }

    fn name(&self) -> String {
        "local-test".into()
    }
}

struct Fixture {
    _tmp: tempfile::TempDir,
    local: Arc<Local>,
    host: RemoteHost,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.host.close();
        let _ = self.local.command().arg("--stop").output();
    }
}

fn fixture() -> Fixture {
    // Short: the daemon's socket path must fit in 104 bytes.
    let tmp = tempfile::Builder::new().prefix("snt").tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let local =
        Arc::new(Local { home, dir: root.join("srv"), proxy: Mutex::new(None), refuse: AtomicBool::new(false) });
    let backoff = Backoff {
        first: Duration::from_millis(50),
        max: Duration::from_millis(200),
        give_up_after: Duration::from_secs(3),
    };
    let host = RemoteHost::connect_with(local.clone(), backoff, Duration::from_secs(5)).unwrap();
    Fixture { _tmp: tmp, local, host }
}

fn wait_until(what: &str, timeout: Duration, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
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

#[test]
fn files_walks_and_searches() {
    let f = fixture();
    let (host, home) = (&f.host, f.local.home.clone());
    assert_eq!(host.info().home, home);
    assert_eq!(host.info().os, std::env::consts::OS);
    assert!(host.info().exe.is_some() && host.info().can_trash);
    assert_eq!(host.name(), "local-test");

    let dir = home.join("p");
    let file = dir.join("a/b/c.txt");
    host.create_dir_all(file.parent().unwrap()).unwrap();
    host.write(&file, b"hello, host\n").unwrap();
    assert_eq!(host.read_to_string(&file).unwrap(), "hello, host\n");
    assert_eq!(host.read_range(&file, 7, 4).unwrap(), b"host");
    let m = host.metadata(&file).unwrap();
    assert!(m.is_file() && m.len == 12 && m.modified.is_some());
    assert!(host.is_dir(&dir.join("a")) && !host.exists(&dir.join("nope")));
    host.write_mode(&dir.join("private"), b"x", Some(0o600)).unwrap();
    assert_eq!(host.metadata(&dir.join("private")).unwrap().mode & 0o777, 0o600);
    host.create_dir_all_mode(&dir.join("locked/in"), Some(0o700)).unwrap();
    assert_eq!(host.metadata(&dir.join("locked")).unwrap().mode & 0o777, 0o700);
    std::os::unix::fs::symlink(dir.join("a"), dir.join("link")).unwrap();
    let mut entries: Vec<(String, bool)> =
        host.read_dir(&dir).unwrap().iter().map(|e| (e.file_name().to_string_lossy().into_owned(), e.is_dir)).collect();
    entries.sort();
    assert_eq!(
        entries,
        [("a".into(), true), ("link".into(), true), ("locked".into(), true), ("private".into(), false)]
    );
    assert_eq!(host.copy(&dir.join("private"), &dir.join("copied")).unwrap(), 1);
    host.rename(&dir.join("copied"), &dir.join("moved")).unwrap();
    assert!(host.remove_dir(&dir.join("moved")).is_err());
    host.remove_file(&dir.join("moved")).unwrap();
    assert!(host.remove_file(&dir.join("a")).is_err());
    assert_eq!(host.canonicalize(&dir.join("a/b/../b")).unwrap(), dir.join("a/b"));

    std::fs::write(dir.join(".gitignore"), "a/\n").unwrap();
    let options = WalkOptions { hidden: true, gitignore: true, ..Default::default() };
    let mut all: Vec<String> = host
        .walk(&dir, &options)
        .unwrap()
        .iter()
        .map(|e| e.path.strip_prefix(&dir).unwrap().to_string_lossy().into_owned())
        .collect();
    all.sort();
    assert!(!all.iter().any(|p| p.starts_with("a/")), "{all:?}");
    assert!(all.contains(&"private".to_owned()), "{all:?}");
    let hits = host.find_text(&dir, &["a/b/c.txt".into()], "host", 10).unwrap();
    assert_eq!((hits.len(), hits[0].line, hits[0].column), (1, 0, 7));

    host.trash(&[dir.join("private")]).unwrap();
    assert!(!host.exists(&dir.join("private")));
    host.remove_dir_all(&dir).unwrap();
    assert!(!host.exists(&dir));
}

#[test]
fn processes_terminals_and_ports() {
    let f = fixture();
    let host = &f.host;
    // The login shell's environment is the processes' environment.
    assert_eq!(host.info().var("SNOUT_TEST_VAR"), Some("from-login"));
    let out = host.run(&Cmd::new("/bin/sh").args(["-c", "cat; echo err >&2; exit 3"]), Some(b"in\n")).unwrap();
    assert_eq!((out.stdout.as_slice(), out.stderr.as_slice()), (&b"in\n"[..], &b"err\n"[..]));
    assert_eq!(out.status.code, Some(3));
    let out = host.run(&Cmd::new("env").env("SNOUT_X", "1").env_remove("SNOUT_TEST_VAR"), None).unwrap();
    let env = String::from_utf8_lossy(&out.stdout);
    assert!(env.lines().any(|l| l == "SNOUT_X=1"), "{env}");
    assert!(!env.lines().any(|l| l.starts_with("SNOUT_TEST_VAR=")), "{env}");
    let log = f.local.home.join("err.log");
    let out = host.run(&Cmd::new("/bin/sh").args(["-c", "echo to-file >&2"]).stderr(Stdio::File(log.clone())), None);
    assert!(out.unwrap().status.success());
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "to-file\n");
    let out = host.run(&Cmd::new("pwd").cwd(&f.local.home), None).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), f.local.home.to_string_lossy());

    let mut child = host.spawn(&Cmd::new("sleep").arg("30")).unwrap();
    let pid = child.pid().unwrap();
    assert!(host.process_alive(pid));
    let listed = host.processes(false).unwrap();
    let me = listed.iter().find(|p| p.pid == pid).expect("the child is listed");
    assert_eq!(me.cmd, ["sleep", "30"]);
    assert!(child.try_wait().unwrap().is_none());
    assert_eq!(host.stop_processes(&[(pid, me.start)], Duration::from_secs(2)), 1);
    assert_eq!(child.wait().unwrap().signal, Some(15));
    assert_eq!(child.try_wait().unwrap().unwrap().signal, Some(15));

    let mut pty = host.spawn_pty(&Cmd::new("/bin/sh").args(["-c", "stty size; read x; echo got $x"]), 24, 80).unwrap();
    let mut reader = pty.take_reader().unwrap();
    let mut writer = pty.take_writer().unwrap();
    writer.write_all(b"it\n").unwrap();
    let text = read_until(&mut reader, "got it");
    assert!(text.contains("24 80"), "{text}");
    assert!(pty.wait().unwrap().success());

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        s.write_all(&buf).unwrap();
    });
    let mut stream = host.connect_tcp(port, Duration::from_secs(2)).unwrap();
    let mut reader = stream.try_clone().unwrap();
    stream.write_all(b"ping").unwrap();
    let mut back = [0u8; 4];
    reader.read_exact(&mut back).unwrap();
    assert_eq!(&back, b"ping");
    stream.shutdown().unwrap();
    server.join().unwrap();
}

#[test]
fn a_dropped_connection_comes_back_with_its_processes() {
    let f = fixture();
    let host = &f.host;
    let root = f.local.home.join("w");
    host.create_dir_all(&root).unwrap();
    let watcher = host.watch(&root).unwrap();

    let mut ticker = host
        .spawn(&Cmd::new("sh").args(["-c", "i=0; while true; do echo tick $i; i=$((i+1)); sleep 0.02; done"]))
        .unwrap();
    let mut out = ticker.take_stdout().unwrap();
    read_until(&mut out, "tick 2\n");
    let mut term = host.spawn_pty(&Cmd::new("/bin/sh"), 24, 80).unwrap();
    let (mut tr, mut tw) = (term.take_reader().unwrap(), term.take_writer().unwrap());
    tw.write_all(b"X=remembered\n").unwrap();
    // The terminal's echo: the line arrived before the drop.
    read_until(&mut tr, "X=remembered");

    // The link drops (ssh dies): calls made meanwhile wait for the new connection.
    f.local.kill_proxy();
    wait_until("the reconnect to start", Duration::from_secs(5), || {
        host.status() != Status::Connected || {
            // It may already be back.
            host.client().is_ok()
        }
    });
    assert!(host.exists(&root), "a call made during the drop goes through once it is back");
    wait_until("the connection to come back", Duration::from_secs(10), || host.status() == Status::Connected);

    // The process carried on, and its output is all there: no tick lost or repeated.
    let text = read_until(&mut out, "tick 60\n");
    let ticks: Vec<u32> = text.lines().filter_map(|l| l.strip_prefix("tick ")).filter_map(|n| n.parse().ok()).collect();
    assert!(ticks.windows(2).all(|w| w[1] == w[0] + 1), "{ticks:?}");
    tw.write_all(b"echo X=$X\n").unwrap();
    read_until(&mut tr, "X=remembered");

    // The watch is back too, after saying anything may have changed.
    host.write(&root.join("after.txt"), b"x").unwrap();
    let start = Instant::now();
    let (mut rescan, mut seen) = (false, false);
    while !(rescan && seen) && start.elapsed() < Duration::from_secs(10) {
        if let Ok(e) = watcher.events.recv_timeout(Duration::from_millis(200)) {
            rescan |= e.kind == WatchKind::Other && e.paths == [root.clone()];
            seen |= e.kind != WatchKind::Access && e.paths.iter().any(|p| p.ends_with("after.txt"));
        }
    }
    assert!(rescan && seen, "rescan {rescan}, seen {seen}");

    ticker.kill().unwrap();
    assert_eq!(ticker.wait().unwrap().signal, Some(9));
    term.kill().unwrap();
    term.wait().unwrap();
}

#[test]
fn a_connection_that_cannot_come_back_is_lost_and_calls_fail_at_once() {
    let f = fixture();
    let host = &f.host;
    f.local.refuse.store(true, Ordering::SeqCst);
    f.local.kill_proxy();
    wait_until("the connection to be lost", Duration::from_secs(10), || matches!(host.status(), Status::Lost(_)));
    let start = Instant::now();
    assert_eq!(host.read(&f.local.home).unwrap_err().kind(), io::ErrorKind::NotConnected);
    assert!(start.elapsed() < Duration::from_millis(100));

    // Reconnecting by hand once the host is back.
    f.local.refuse.store(false, Ordering::SeqCst);
    host.reconnect();
    wait_until("the reconnect", Duration::from_secs(10), || host.status() == Status::Connected);
    assert!(host.is_dir(&f.local.home));

    // Closed: nothing goes through any more, and nothing reconnects.
    host.close();
    assert_eq!(host.status(), Status::Closed);
    assert_eq!(host.metadata(&f.local.home).unwrap_err().kind(), io::ErrorKind::NotConnected);
}

#[test]
fn a_connection_dropped_by_hand_comes_back_and_calls_wait_for_it() {
    let f = fixture();
    let host = &f.host;
    f.local.refuse.store(true, Ordering::SeqCst);
    host.drop_connection();
    wait_until("the reconnect to start", Duration::from_secs(10), || {
        matches!(host.status(), Status::Reconnecting { .. })
    });
    // A call waits for the connection, and gets through once it is back.
    let home = f.local.home.clone();
    std::thread::scope(|s| {
        let waiting = s.spawn(|| host.is_dir(&home));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!waiting.is_finished(), "the call waits");
        f.local.refuse.store(false, Ordering::SeqCst);
        assert!(waiting.join().unwrap());
    });
    assert_eq!(host.status(), Status::Connected);
    assert_eq!(host.connections(), 2);
}

#[test]
fn a_port_forwarded_back_to_the_app_stays_through_a_reconnect() {
    let f = fixture();
    let host = &f.host;
    // A server of the app's, on this side: it answers each line with the line in capitals.
    let app = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let app_port = app.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in app.incoming().flatten() {
            std::thread::spawn(move || {
                let mut r = io::BufReader::new(stream.try_clone().unwrap());
                let mut w = stream;
                let mut line = String::new();
                while io::BufRead::read_line(&mut r, &mut line).is_ok_and(|n| n > 0) {
                    let _ = w.write_all(line.to_uppercase().as_bytes());
                    line.clear();
                }
            });
        }
    });
    let forward = host.forward_to_app(app_port).unwrap();
    assert_ne!(forward.port, 0);
    // A program on the host connects to the port there, as `claude` does.
    let ask = |what: &str| {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", forward.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(format!("{what}\n").as_bytes()).unwrap();
        read_until(&mut s, &what.to_uppercase())
    };
    assert_eq!(ask("hello"), "HELLO\n");
    assert_eq!(ask("again"), "AGAIN\n", "one connection after another");

    // The connection drops: the port stays, and leads to the app again once it is back.
    let connections = host.connections();
    f.local.kill_proxy();
    wait_until("the reconnect", Duration::from_secs(10), || {
        host.connections() > connections && host.status() == Status::Connected
    });
    assert_eq!(ask("after"), "AFTER\n");

    // Dropped: nothing listens there any more.
    let port = forward.port;
    drop(forward);
    wait_until("the port to close", Duration::from_secs(5), || {
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err()
    });
}
