use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("snout-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    canonical(&dir).unwrap()
}

#[test]
fn paths_get_the_separators_of_their_host() {
    // As text: on Windows, paths with either separator compare equal.
    let native = |info: &HostInfo, p: &str| info.native_path(Path::new(p)).to_str().unwrap().to_owned();
    let windows = HostInfo { os: "windows".into(), ..Default::default() };
    assert_eq!(native(&windows, r"C:\repo/src/App.java"), r"C:\repo\src\App.java");
    assert_eq!(native(&windows, r"\\?\C:\a/b"), r"\\?\C:\a/b");
    let linux = HostInfo { os: "linux".into(), ..Default::default() };
    assert_eq!(native(&linux, "/repo/src/App.java"), "/repo/src/App.java");
}

#[test]
fn files_round_trip() {
    let host = LocalHost::new();
    let dir = scratch("files");
    let file = dir.join("a/b/c.txt");
    host.create_dir_all(file.parent().unwrap()).unwrap();
    host.write(&file, b"hello, host\n").unwrap();
    assert_eq!(host.read_to_string(&file).unwrap(), "hello, host\n");
    assert_eq!(host.read_range(&file, 7, 4).unwrap(), b"host");
    assert_eq!(host.read_range(&file, 7, 100).unwrap(), b"host\n");
    let m = host.metadata(&file).unwrap();
    assert!(m.is_file() && m.len == 12 && m.modified.is_some());
    assert!(host.is_dir(&dir.join("a")) && !host.is_file(&dir.join("a")) && !host.exists(&dir.join("nope")));

    // A write keeps the file's permission bits, and leaves nothing beside it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        host.write(&file, b"again").unwrap();
        assert_eq!(host.metadata(&file).unwrap().mode & 0o777, 0o755);
        host.write_mode(&dir.join("private"), b"x", Some(0o600)).unwrap();
        assert_eq!(host.metadata(&dir.join("private")).unwrap().mode & 0o777, 0o600);
        // Through a link, the file it points to changes.
        std::os::unix::fs::symlink(&file, dir.join("link")).unwrap();
        host.write(&dir.join("link"), b"linked").unwrap();
        assert!(host.symlink_metadata(&dir.join("link")).unwrap().is_symlink());
        assert_eq!(host.read_to_string(&file).unwrap(), "linked");
    }
    #[cfg(not(unix))]
    {
        // A read-only file shows as one; a write replaces the file and leaves nothing beside it.
        host.write(&file, b"again").unwrap();
        assert_eq!(host.metadata(&file).unwrap().mode & 0o777, 0o644);
        host.write_mode(&dir.join("private"), b"x", Some(0o600)).unwrap();
        std::fs::write(dir.join("link"), "not a link here").unwrap();
    }

    let mut names: Vec<String> =
        host.read_dir(&dir).unwrap().iter().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["a", "link", "private"]);
    host.rename(&dir.join("private"), &dir.join("moved")).unwrap();
    host.copy(&dir.join("moved"), &dir.join("copied")).unwrap();
    host.remove_file(&dir.join("moved")).unwrap();
    assert_eq!(host.canonicalize(&dir.join("a/b/../b")).unwrap(), dir.join("a/b"));
    host.create_dir_all(&dir.join("empty")).unwrap();
    host.remove_dir(&dir.join("empty")).unwrap();
    host.remove_dir_all(&dir).unwrap();
    assert!(!host.exists(&dir));
}

#[test]
fn walks_as_gitignore_says_and_finds_text() {
    let host = LocalHost::new();
    let dir = scratch("walk");
    std::fs::create_dir_all(dir.join("src/deep")).unwrap();
    std::fs::create_dir_all(dir.join("build")).unwrap();
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    std::fs::write(dir.join("src/deep/A.java"), "class A {\n  // Hello, there\n}\n").unwrap();
    std::fs::write(dir.join("build/out.txt"), "Hello\n").unwrap();
    std::fs::write(dir.join("bin.dat"), b"Hello\0\x01").unwrap();

    let options = WalkOptions { hidden: true, gitignore: true, skip: vec![".git".into()], ..Default::default() };
    let mut all: Vec<String> =
        host.walk(&dir, &options).unwrap().iter().map(|e| slash_path(e.path.strip_prefix(&dir).unwrap())).collect();
    all.sort();
    assert_eq!(all, [".gitignore", "bin.dat", "src", "src/deep", "src/deep/A.java"]);
    let top = host.walk(&dir, &WalkOptions { max_depth: Some(1), files_only: true, ..options.clone() }).unwrap();
    assert!(top.iter().all(|e| e.depth == 1 && !e.is_dir));

    let files = vec!["src/deep/A.java".to_owned(), "build/out.txt".to_owned(), "bin.dat".to_owned()];
    let hits = host.find_text(&dir, &files, "hello", 10).unwrap();
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!((hits[0].line, hits[0].column, hits[0].text.as_str()), (1, 5, "  // Hello, there"));
    assert!(host.find_text(&dir, &files, "Hello", 1).unwrap().len() == 1);
    assert!(host.find_text(&dir, &files, "hello,", 10).unwrap().len() == 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn watches_a_tree() {
    let host = LocalHost::new();
    let dir = scratch("watch");
    let watcher = host.watch(&dir).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(dir.join("new.txt"), "x").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = false;
    while !seen && Instant::now() < deadline {
        if let Ok(e) = watcher.events.recv_timeout(Duration::from_millis(100)) {
            seen = e.paths.iter().any(|p| p.ends_with("new.txt"));
        }
    }
    assert!(seen, "the watcher saw the new file");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn runs_processes() {
    let host = LocalHost::new();
    let out = host.run(&Cmd::new("/bin/sh").args(["-c", "cat; echo err >&2; exit 3"]), Some(b"in\n")).unwrap();
    assert_eq!((out.stdout.as_slice(), out.stderr.as_slice()), (&b"in\n"[..], &b"err\n"[..]));
    assert_eq!(out.status.code, Some(3));
    assert!(!out.status.success());

    let dir = scratch("proc");
    let out = host.run(&Cmd::new("/bin/pwd").cwd(&dir), None).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), dir.to_string_lossy());
    let log = dir.join("err.log");
    let out = host.run(&Cmd::new("/bin/sh").args(["-c", "echo to-file >&2"]).stderr(Stdio::File(log.clone())), None);
    assert!(out.unwrap().status.success());
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "to-file\n");
    let out = host.run(&Cmd::new("/usr/bin/env").env("SNOUT_X", "1").env_remove("HOME"), None).unwrap();
    let env = String::from_utf8_lossy(&out.stdout);
    assert!(env.lines().any(|l| l == "SNOUT_X=1") && !env.lines().any(|l| l.starts_with("HOME=")));

    // The default `run`, through `spawn`, does the same.
    struct Spawning(LocalHost);
    impl Host for Spawning {
        fn info(&self) -> &HostInfo {
            self.0.info()
        }
        fn read(&self, p: &std::path::Path) -> std::io::Result<Vec<u8>> {
            self.0.read(p)
        }
        fn read_range(&self, p: &std::path::Path, o: u64, l: u64) -> std::io::Result<Vec<u8>> {
            self.0.read_range(p, o, l)
        }
        fn write_mode(&self, p: &std::path::Path, d: &[u8], m: Option<u32>) -> std::io::Result<()> {
            self.0.write_mode(p, d, m)
        }
        fn create_dir_all_mode(&self, p: &std::path::Path, m: Option<u32>) -> std::io::Result<()> {
            self.0.create_dir_all_mode(p, m)
        }
        fn remove_file(&self, p: &std::path::Path) -> std::io::Result<()> {
            self.0.remove_file(p)
        }
        fn remove_dir(&self, p: &std::path::Path) -> std::io::Result<()> {
            self.0.remove_dir(p)
        }
        fn remove_dir_all(&self, p: &std::path::Path) -> std::io::Result<()> {
            self.0.remove_dir_all(p)
        }
        fn rename(&self, a: &std::path::Path, b: &std::path::Path) -> std::io::Result<()> {
            self.0.rename(a, b)
        }
        fn copy(&self, a: &std::path::Path, b: &std::path::Path) -> std::io::Result<u64> {
            self.0.copy(a, b)
        }
        fn metadata(&self, p: &std::path::Path) -> std::io::Result<Metadata> {
            self.0.metadata(p)
        }
        fn symlink_metadata(&self, p: &std::path::Path) -> std::io::Result<Metadata> {
            self.0.symlink_metadata(p)
        }
        fn read_dir(&self, p: &std::path::Path) -> std::io::Result<Vec<DirEntry>> {
            self.0.read_dir(p)
        }
        fn canonicalize(&self, p: &std::path::Path) -> std::io::Result<PathBuf> {
            self.0.canonicalize(p)
        }
        fn trash(&self, p: &[PathBuf]) -> std::io::Result<()> {
            self.0.trash(p)
        }
        fn walk(&self, r: &std::path::Path, o: &WalkOptions) -> std::io::Result<Vec<WalkEntry>> {
            self.0.walk(r, o)
        }
        fn find_text(&self, r: &std::path::Path, f: &[String], n: &str, l: usize) -> std::io::Result<Vec<TextHit>> {
            self.0.find_text(r, f, n, l)
        }
        fn watch(&self, r: &std::path::Path) -> std::io::Result<Watcher> {
            self.0.watch(r)
        }
        fn spawn(&self, c: &Cmd) -> std::io::Result<Box<dyn Process>> {
            self.0.spawn(c)
        }
        fn spawn_pty(&self, c: &Cmd, r: u16, k: u16) -> std::io::Result<Box<dyn PtyProcess>> {
            self.0.spawn_pty(c, r, k)
        }
        fn connect_tcp(&self, p: u16, t: Duration) -> std::io::Result<Box<dyn Stream>> {
            self.0.connect_tcp(p, t)
        }
        fn processes(&self, c: bool) -> std::io::Result<Vec<ProcInfo>> {
            self.0.processes(c)
        }
        fn stop_processes(&self, t: &[(u32, u64)], g: Duration) -> usize {
            self.0.stop_processes(t, g)
        }
        fn process_alive(&self, p: u32) -> bool {
            self.0.process_alive(p)
        }
    }
    let spawning = Spawning(LocalHost::new());
    let out = spawning.run(&Cmd::new("/bin/sh").args(["-c", "cat; exit 2"]), Some(b"through spawn")).unwrap();
    assert_eq!((out.stdout.as_slice(), out.status.code), (&b"through spawn"[..], Some(2)));

    // A long-running one, ended politely.
    let mut child = host.spawn(&Cmd::new("/bin/sleep").arg("30")).unwrap();
    let pid = child.pid().unwrap();
    assert!(host.process_alive(pid));
    assert!(child.try_wait().unwrap().is_none());
    child.terminate().unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.signal, Some(15));
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn lists_and_stops_processes() {
    let host = LocalHost::new();
    let mut child = host.spawn(&Cmd::new("/bin/sleep").arg("30")).unwrap();
    let pid = child.pid().unwrap();
    let procs = host.processes(false).unwrap();
    let me = procs.iter().find(|p| p.pid == pid).expect("the child is listed");
    assert_eq!(me.parent, Some(std::process::id()));
    assert_eq!(me.cmd, ["/bin/sleep", "30"]);
    assert_eq!(host.stop_processes(&[(pid, me.start + 1)], Duration::from_secs(1)), 0, "another start is left alone");
    assert_eq!(host.stop_processes(&[(pid, me.start)], Duration::from_secs(2)), 1);
    assert_eq!(child.wait().unwrap().signal, Some(15));
}

/// A process its parent reaps as it ends (an engine's jdtls and Gradle daemon, which the
/// engine waits for) is gone at once: the stop does not wait out its grace for it.
#[cfg(unix)]
#[test]
fn a_stop_ends_when_the_processes_are_gone_not_after_the_grace() {
    let host = LocalHost::new();
    let child = host.spawn(&Cmd::new("/bin/sleep").arg("30")).unwrap();
    let pid = child.pid().unwrap();
    let start = host.processes(false).unwrap().into_iter().find(|p| p.pid == pid).unwrap().start;
    // Reaped as soon as it ends, as a parent waiting on it does.
    let reaper = std::thread::spawn(move || {
        let mut child = child;
        child.wait().unwrap()
    });
    let t0 = std::time::Instant::now();
    assert_eq!(host.stop_processes(&[(pid, start)], Duration::from_secs(10)), 1);
    assert!(t0.elapsed() < Duration::from_secs(2), "stopped in {:?}", t0.elapsed());
    assert_eq!(reaper.join().unwrap().signal, Some(15), "ended by SIGTERM, not killed");
}

#[cfg(unix)]
#[test]
fn runs_a_terminal() {
    let host = LocalHost::new();
    let mut pty = host.spawn_pty(&Cmd::new("/bin/sh").args(["-c", "stty size; read x; echo got $x"]), 24, 80).unwrap();
    let mut reader = pty.take_reader().unwrap();
    let mut writer = pty.take_writer().unwrap();
    writer.write_all(b"it\n").unwrap();
    let mut all = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
        if String::from_utf8_lossy(&all).contains("got it") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&all);
    assert!(text.contains("24 80") && text.contains("got it"), "{text}");
    pty.resize(30, 100).unwrap();
    assert!(pty.wait().unwrap().success());
}

#[test]
fn connects_to_a_port() {
    let host = LocalHost::new();
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
fn info_and_which() {
    let host = LocalHost::new();
    let info = host.info();
    assert_eq!(info.os, std::env::consts::OS);
    assert!(!info.home.as_os_str().is_empty() && info.exe.is_some());
    assert!(which(&host, &[if cfg!(windows) { "cmd" } else { "sh" }], &[]).is_some());
    assert_eq!(which(&host, &["no-such-program-anywhere"], &[]), None);
}

#[test]
fn latency_slows_every_call_and_counts_the_ui_threads() {
    let host = LatencyHost::new(Arc::new(LocalHost::new()), Duration::from_millis(30)).ui_thread_here();
    let started = Instant::now();
    assert!(!host.exists(&PathBuf::from("/no/such/file")));
    assert!(host.metadata(&PathBuf::from("/")).is_ok());
    assert!(started.elapsed() >= Duration::from_millis(60));
    assert_eq!(host.ui_calls(), 2);
    let host = Arc::new(host);
    let other = host.clone();
    std::thread::spawn(move || other.exists(&PathBuf::from("/"))).join().unwrap();
    assert_eq!(host.ui_calls(), 2, "a worker's calls are not the UI thread's");
    // What the host is costs nothing.
    let started = Instant::now();
    assert!(!host.info().home.as_os_str().is_empty());
    assert!(started.elapsed() < Duration::from_millis(30));
}

/// Windows: a command line through `cmd.exe`, its output, its folder and environment; a
/// process tree stopped as one; a terminal on ConPTY.
#[cfg(windows)]
mod windows {
    use super::*;

    fn cmd(line: &str) -> Cmd {
        Cmd::new("cmd.exe").args(["/d", "/c", line])
    }

    #[test]
    fn runs_processes() {
        let host = LocalHost::new();
        let out = host.run(&Cmd::new("findstr.exe").arg("^"), Some(b"in\r\n")).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "in");
        let out = host.run(&cmd("echo err 1>&2 & exit /b 3"), None).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "err");
        assert_eq!(out.status.code, Some(3));

        let dir = scratch("proc");
        let out = host.run(&cmd("cd").cwd(&dir), None).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), dir.to_string_lossy());
        let out = host.run(&cmd("set SNOUT_").env("SNOUT_X", "1"), None).unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).lines().any(|l| l.trim() == "SNOUT_X=1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A process and what it started end together: `cmd.exe` running `ping` (as
    /// `gradlew.bat` runs Java).
    #[test]
    fn stopping_a_process_stops_what_it_started() {
        let host = LocalHost::new();
        let mut child = host.spawn(&cmd("ping -n 60 127.0.0.1 >NUL")).unwrap();
        let pid = child.pid().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let ping = loop {
            let procs = host.processes(false).unwrap();
            if let Some(p) = procs.iter().find(|p| p.parent == Some(pid)) {
                break p.pid;
            }
            assert!(Instant::now() < deadline, "ping started");
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(host.process_alive(ping));
        child.terminate().unwrap();
        child.wait().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.process_alive(ping) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!host.process_alive(ping), "the child's child ended too");
    }

    #[test]
    fn lists_and_stops_processes() {
        let host = LocalHost::new();
        let mut child = host.spawn(&Cmd::new("ping.exe").args(["-n", "60", "127.0.0.1"])).unwrap();
        let pid = child.pid().unwrap();
        let procs = host.processes(false).unwrap();
        let me = procs.iter().find(|p| p.pid == pid).expect("the child is listed");
        assert_eq!(me.parent, Some(std::process::id()));
        assert_eq!(
            host.stop_processes(&[(pid, me.start + 1)], Duration::from_secs(1)),
            0,
            "another start is left alone"
        );
        assert_eq!(host.stop_processes(&[(pid, me.start)], Duration::from_secs(2)), 1);
        child.wait().unwrap();
        assert!(!host.process_alive(pid));
    }

    #[test]
    fn runs_a_terminal() {
        let host = LocalHost::new();
        // No spaces in what is looked for: the console may draw them as cursor moves.
        let mut pty = host.spawn_pty(&cmd("set /p x= & call echo got:%x%"), 24, 80).unwrap();
        let mut reader = pty.take_reader().unwrap();
        let mut writer = pty.take_writer().unwrap();
        writer.write_all(b"it\r\n").unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (tx, rx) = std::sync::mpsc::channel();
        let all = seen.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut all = all.lock().unwrap();
                all.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&all).contains("got:it") {
                    break;
                }
            }
            let _ = tx.send(());
        });
        let answered = rx.recv_timeout(Duration::from_secs(20));
        let text = String::from_utf8_lossy(&seen.lock().unwrap()).into_owned();
        assert!(answered.is_ok() && text.contains("got:it"), "the terminal showed {text:?}");
        pty.resize(30, 100).unwrap();
        assert!(pty.wait().unwrap().success());
    }

    #[test]
    fn a_terminal_ends_when_its_program_does() {
        let host = LocalHost::new();
        let mut pty = host.spawn_pty(&cmd("exit /b 3"), 24, 80).unwrap();
        let mut reader = pty.take_reader().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = tx.send(reader.read_to_end(&mut sink).is_ok());
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(20)), Ok(true), "the reader saw the end");
        assert_eq!(pty.wait().unwrap().code, Some(3));
        pty.resize(30, 100).unwrap();
    }

    #[test]
    fn programs_are_found_with_their_extensions() {
        let host = LocalHost::new();
        let git_like = which(&host, &["cmd"], &[]).unwrap();
        assert!(git_like.to_string_lossy().to_ascii_lowercase().ends_with("cmd.exe"), "{}", git_like.display());
        let info = HostInfo { os: "windows".into(), ..Default::default() };
        assert_eq!(info.program_names("gradlew")[..2], ["gradlew.com", "gradlew.exe"]);
        assert_eq!(info.program_names("gradlew.bat"), ["gradlew.bat"]);
        let linux = HostInfo { os: "linux".into(), ..Default::default() };
        assert_eq!(linux.program_names("git"), ["git"]);
    }
}
