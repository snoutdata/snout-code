//! This machine as a [`Host`]: `std::fs`, `std::process`, `notify`, `portable-pty` and `sysinfo`.

use std::io::{self, Read, Seek as _, SeekFrom, Write};
use std::net::{TcpStream, ToSocketAddrs as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{
    Cmd, DirEntry, ExitStatus, FileKind, Host, HostInfo, Metadata, Output, ProcInfo, Process, PtyProcess, Stdio,
    Stream, TextHit, WalkEntry, WalkOptions, WatchEvent, WatchKind, Watcher,
};

/// This machine.
pub struct LocalHost {
    info: HostInfo,
    /// How files go to the Trash here, if they can (see [`crate::trash`]).
    trash: Option<crate::trash::Method>,
    /// The process listing, kept for measuring CPU use between two listings.
    system: Mutex<(sysinfo::System, Option<Instant>)>,
}

impl Default for LocalHost {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalHost {
    pub fn new() -> Self {
        let home = crate::home_dir();
        let trash = crate::trash::method(&|name| std::env::var(name).ok(), &home);
        let info = HostInfo {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            home,
            temp_dir: std::env::temp_dir(),
            env: std::env::vars_os()
                .map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned()))
                .collect(),
            exe: std::env::current_exe().ok(),
            pid: std::process::id(),
            can_trash: trash.is_some(),
        };
        Self { info, trash, system: Mutex::new((sysinfo::System::new(), None)) }
    }
}

fn kind_of(t: std::fs::FileType) -> FileKind {
    if t.is_symlink() {
        FileKind::Symlink
    } else if t.is_dir() {
        FileKind::Dir
    } else if t.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    }
}

fn metadata_of(m: std::fs::Metadata) -> Metadata {
    Metadata { kind: kind_of(m.file_type()), len: m.len(), modified: m.modified().ok(), mode: mode_of(&m) }
}

#[cfg(unix)]
fn mode_of(m: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    m.permissions().mode()
}

/// Windows has no permission bits: those a Unix system would show, from the file's kind and
/// whether it is read-only.
#[cfg(not(unix))]
fn mode_of(m: &std::fs::Metadata) -> u32 {
    let base = if m.is_dir() { 0o040_755 } else { 0o100_644 };
    if m.permissions().readonly() { base & !0o222 } else { base }
}

fn command(cmd: &Cmd) -> io::Result<std::process::Command> {
    let mut c = std::process::Command::new(&cmd.program);
    crate::default_signals(&mut c).args(&cmd.args);
    if let Some(dir) = &cmd.cwd {
        c.current_dir(dir);
    }
    for (k, v) in &cmd.env {
        match v {
            Some(v) => c.env(k, v),
            None => c.env_remove(k),
        };
    }
    let stdio = |s: &Stdio| -> io::Result<std::process::Stdio> {
        Ok(match s {
            Stdio::Null => std::process::Stdio::null(),
            Stdio::Piped => std::process::Stdio::piped(),
            Stdio::Inherit => std::process::Stdio::inherit(),
            Stdio::File(path) => std::fs::File::create(path)?.into(),
        })
    };
    c.stdin(stdio(&cmd.stdin)?).stdout(stdio(&cmd.stdout)?).stderr(stdio(&cmd.stderr)?);
    Ok(c)
}

/// A process this Mac runs. On Windows it is in a job object of its own, so stopping it
/// stops the processes it started too.
struct LocalProcess {
    child: std::process::Child,
    #[cfg(windows)]
    job: Option<crate::win::Job>,
}

impl LocalProcess {
    fn new(child: std::process::Child) -> Self {
        #[cfg(windows)]
        {
            let job = crate::win::Job::new(false).ok().filter(|j| j.assign_child(&child).is_ok());
            Self { child, job }
        }
        #[cfg(not(windows))]
        Self { child }
    }
}

impl Process for LocalProcess {
    fn pid(&self) -> Option<u32> {
        Some(self.child.id())
    }

    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>> {
        self.child.stdin.take().map(|s| Box::new(s) as _)
    }

    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.child.stdout.take().map(|s| Box::new(s) as _)
    }

    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.child.stderr.take().map(|s| Box::new(s) as _)
    }

    #[cfg(unix)]
    fn terminate(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        let pid = libc::pid_t::try_from(self.child.id()).map_err(io::Error::other)?;
        // SAFETY: a plain system call on a child of ours that has not been waited for.
        if unsafe { libc::kill(pid, libc::SIGTERM) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
    }

    /// Windows has no SIGTERM for a program without a window: it ends at once, with what it
    /// started.
    #[cfg(not(unix))]
    fn terminate(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        self.kill()
    }

    fn kill(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        if let Some(job) = &self.job
            && job.terminate(1).is_ok()
        {
            return Ok(());
        }
        self.child.kill()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.child.try_wait()?.map(Into::into))
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        Ok(self.child.wait()?.into())
    }
}

/// A process on one of this Mac's pseudo-terminals.
struct LocalPty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

fn pty_status(s: portable_pty::ExitStatus) -> ExitStatus {
    ExitStatus { code: Some(s.exit_code() as i32), signal: None }
}

impl PtyProcess for LocalPty {
    fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }

    fn take_reader(&mut self) -> io::Result<Box<dyn Read + Send>> {
        self.master.try_clone_reader().map_err(io::Error::other)
    }

    fn take_writer(&mut self) -> io::Result<Box<dyn Write + Send>> {
        self.master.take_writer().map_err(io::Error::other)
    }

    fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        self.master.resize(pty_size(rows, cols)).map_err(io::Error::other)
    }

    fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.child.try_wait()?.map(pty_status))
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        Ok(pty_status(self.child.wait()?))
    }
}

fn pty_size(rows: u16, cols: u16) -> portable_pty::PtySize {
    portable_pty::PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

impl Stream for TcpStream {
    fn try_clone(&self) -> io::Result<Box<dyn Stream>> {
        Ok(Box::new(TcpStream::try_clone(self)?))
    }

    fn shutdown(&self) -> io::Result<()> {
        match TcpStream::shutdown(self, std::net::Shutdown::Both) {
            // The other end closed first.
            Err(e) if e.kind() == io::ErrorKind::NotConnected => Ok(()),
            r => r,
        }
    }
}

/// `path` with every link resolved; on Windows without the `\\?\` prefix a path that does
/// not need it would carry (so it compares with the paths the rest of the app makes).
pub fn canonical(path: &Path) -> io::Result<PathBuf> {
    let p = std::fs::canonicalize(path)?;
    #[cfg(windows)]
    let p = crate::win::simplify(&p);
    Ok(p)
}

/// A file beside `path` to write before renaming it into place.
fn temp_beside(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.snout-{}-{n}.tmp", std::process::id()))
}

impl Host for LocalHost {
    fn info(&self) -> &HostInfo {
        &self.info
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn read_range(&self, path: &Path, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        let mut f = std::fs::File::open(path)?;
        f.seek(SeekFrom::Start(offset))?;
        let mut out = Vec::new();
        f.take(len).read_to_end(&mut out)?;
        Ok(out)
    }

    fn write_mode(&self, path: &Path, data: &[u8], mode: Option<u32>) -> io::Result<()> {
        // A link is written through: the file it points to changes, the link stays.
        let path = match std::fs::symlink_metadata(path) {
            Ok(m) if m.file_type().is_symlink() => canonical(path)?,
            _ => path.to_owned(),
        };
        let mode = mode.or_else(|| std::fs::metadata(&path).ok().map(|m| mode_of(&m) & 0o7777));
        let tmp = temp_beside(&path);
        let written = (|| {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            if let Some(mode) = mode {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(mode);
            }
            let mut f = options.open(&tmp)?;
            f.write_all(data)?;
            #[cfg(unix)]
            if let Some(mode) = mode {
                use std::os::unix::fs::PermissionsExt as _;
                // The umask had its say on creation; the file keeps what it had.
                f.set_permissions(std::fs::Permissions::from_mode(mode))?;
            }
            #[cfg(not(unix))]
            let _ = mode;
            drop(f);
            std::fs::rename(&tmp, &path)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written
    }

    fn append(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        std::fs::OpenOptions::new().append(true).create(true).open(path)?.write_all(data)
    }

    fn create_dir_all_mode(&self, path: &Path, mode: Option<u32>) -> io::Result<()> {
        let mut b = std::fs::DirBuilder::new();
        b.recursive(true);
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::DirBuilderExt as _;
            b.mode(mode);
        }
        #[cfg(not(unix))]
        let _ = mode;
        b.create(path)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        std::fs::copy(from, to)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        std::fs::metadata(path).map(metadata_of)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata> {
        std::fs::symlink_metadata(path).map(metadata_of)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        Ok(std::fs::read_dir(path)?
            .flatten()
            .map(|e| {
                let path = e.path();
                let kind = e.file_type().map_or(FileKind::Other, kind_of);
                let is_dir = kind == FileKind::Dir || (kind == FileKind::Symlink && path.is_dir());
                DirEntry { path, kind, is_dir }
            })
            .collect())
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        canonical(path)
    }

    fn trash(&self, paths: &[PathBuf]) -> io::Result<()> {
        match &self.trash {
            Some(method) => crate::trash::trash(method, paths),
            None => Err(io::Error::new(io::ErrorKind::Unsupported, "this machine has no Trash")),
        }
    }

    fn walk(&self, root: &Path, options: &WalkOptions) -> io::Result<Vec<WalkEntry>> {
        let skip = options.skip.clone();
        let mut b = ignore::WalkBuilder::new(root);
        b.max_depth(options.max_depth)
            .standard_filters(options.gitignore)
            .hidden(!options.hidden)
            .require_git(false)
            .filter_entry(move |e| !skip.iter().any(|s| e.file_name() == s.as_str()));
        Ok(b.build()
            .filter_map(Result::ok)
            .filter(|e| e.depth() > 0)
            .filter_map(|e| {
                let is_dir = e.file_type().is_some_and(|t| t.is_dir());
                if options.files_only && !e.file_type().is_some_and(|t| t.is_file()) {
                    return None;
                }
                Some(WalkEntry { depth: e.depth(), is_dir, path: e.into_path() })
            })
            .collect())
    }

    fn find_text(&self, root: &Path, files: &[String], needle: &str, limit: usize) -> io::Result<Vec<TextHit>> {
        use grep_matcher::Matcher as _;
        use grep_searcher::{BinaryDetection, SearcherBuilder, sinks::UTF8};
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let matcher = grep_regex::RegexMatcherBuilder::new()
            .case_smart(true)
            .fixed_strings(true)
            .build(needle)
            .map_err(io::Error::other)?;
        let mut searcher = SearcherBuilder::new().binary_detection(BinaryDetection::quit(0)).line_number(true).build();
        let mut hits = Vec::new();
        for file in files {
            if hits.len() >= limit {
                break;
            }
            let _ = searcher.search_path(
                &matcher,
                root.join(file),
                UTF8(|line, found| {
                    let column = matcher.find(found.as_bytes()).ok().flatten().map_or(0, |m| m.start());
                    hits.push(TextHit {
                        path: file.clone(),
                        line: line.saturating_sub(1) as u32,
                        column: column as u32,
                        text: found.trim_end().to_owned(),
                    });
                    Ok(hits.len() < limit)
                }),
            );
        }
        Ok(hits)
    }

    fn watch(&self, root: &Path) -> io::Result<Watcher> {
        use notify::Watcher as _;
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let kind = match event.kind {
                    notify::EventKind::Access(_) => WatchKind::Access,
                    notify::EventKind::Create(_) => WatchKind::Create,
                    notify::EventKind::Modify(_) => WatchKind::Modify,
                    notify::EventKind::Remove(_) => WatchKind::Remove,
                    _ => WatchKind::Other,
                };
                let _ = tx.send(WatchEvent { kind, paths: event.paths });
            }
        })
        .map_err(io::Error::other)?;
        watcher.watch(root, notify::RecursiveMode::Recursive).map_err(io::Error::other)?;
        Ok(Watcher::new(rx, Box::new(watcher)))
    }

    fn spawn(&self, cmd: &Cmd) -> io::Result<Box<dyn Process>> {
        Ok(Box::new(LocalProcess::new(command(cmd)?.spawn()?)))
    }

    fn run(&self, cmd: &Cmd, input: Option<&[u8]>) -> io::Result<Output> {
        let mut c = command(cmd)?;
        let out = match input {
            None => c.stdin(std::process::Stdio::null()).output()?,
            Some(input) => {
                let mut child = c.stdin(std::process::Stdio::piped()).spawn()?;
                let mut stdin = child.stdin.take().expect("piped stdin");
                // Written on a thread, so a full stdout pipe cannot hold the writing up.
                let input = input.to_vec();
                let writer = std::thread::spawn(move || {
                    let _ = stdin.write_all(&input);
                });
                let out = child.wait_with_output()?;
                let _ = writer.join();
                out
            }
        };
        Ok(Output { status: out.status.into(), stdout: out.stdout, stderr: out.stderr })
    }

    fn spawn_pty(&self, cmd: &Cmd, rows: u16, cols: u16) -> io::Result<Box<dyn PtyProcess>> {
        let pair = portable_pty::native_pty_system().openpty(pty_size(rows, cols)).map_err(io::Error::other)?;
        let mut c = portable_pty::CommandBuilder::new(&cmd.program);
        c.args(&cmd.args);
        if let Some(dir) = &cmd.cwd {
            c.cwd(dir);
        }
        for (k, v) in &cmd.env {
            match v {
                Some(v) => c.env(k, v),
                None => c.env_remove(k),
            }
        }
        let child = pair.slave.spawn_command(c).map_err(io::Error::other)?;
        drop(pair.slave);
        Ok(Box::new(LocalPty { master: pair.master, child }))
    }

    fn connect_tcp(&self, port: u16, timeout: Duration) -> io::Result<Box<dyn Stream>> {
        let addr = ("127.0.0.1", port).to_socket_addrs()?.next().ok_or_else(|| io::Error::other("no loopback"))?;
        let stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_nodelay(true)?;
        Ok(Box::new(stream))
    }

    fn processes(&self, cpu: bool) -> io::Result<Vec<ProcInfo>> {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let mut guard = self.system.lock().unwrap_or_else(|e| e.into_inner());
        let (system, last) = &mut *guard;
        let refresh = |system: &mut sysinfo::System| {
            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                ProcessRefreshKind::new().with_cpu().with_memory().with_cmd(UpdateKind::OnlyIfNotSet),
            );
        };
        // CPU use is measured between two refreshes: the first one sets a baseline.
        let first = last.is_none_or(|t| t.elapsed() > Duration::from_secs(60));
        refresh(system);
        if first && cpu {
            std::thread::sleep(Duration::from_millis(250));
            refresh(system);
        }
        *last = Some(Instant::now());
        Ok(system
            .processes()
            .iter()
            // On Linux the threads of a process are listed too.
            .filter(|(_, p)| p.thread_kind().is_none())
            .map(|(pid, p)| ProcInfo {
                pid: pid.as_u32(),
                parent: p.parent().map(Pid::as_u32),
                name: p.name().to_string_lossy().into_owned(),
                cmd: p.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect(),
                memory: p.memory(),
                cpu: p.cpu_usage(),
                start: p.start_time(),
            })
            .collect())
    }

    fn stop_processes(&self, targets: &[(u32, u64)], grace: Duration) -> usize {
        use sysinfo::{Pid, Signal};
        if targets.is_empty() {
            return 0;
        }
        let (system, asked) = running(targets);
        for &(pid, _) in &asked {
            if let Some(p) = system.process(Pid::from_u32(pid))
                && p.kill_with(Signal::Term).is_none()
            {
                p.kill();
            }
        }
        let deadline = Instant::now() + grace;
        loop {
            let (system, left) = running(&asked);
            if left.is_empty() {
                break;
            }
            if Instant::now() >= deadline {
                for (pid, _) in left {
                    if let Some(p) = system.process(Pid::from_u32(pid)) {
                        p.kill();
                    }
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        asked.len()
    }

    #[cfg(windows)]
    fn process_alive(&self, pid: u32) -> bool {
        crate::win::process_alive(pid)
    }

    #[cfg(unix)]
    fn process_alive(&self, pid: u32) -> bool {
        let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
        if pid <= 0 {
            return false;
        }
        // SAFETY: signal 0 checks that the process exists and sends nothing.
        let r = unsafe { libc::kill(pid, 0) };
        r == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

/// The processes of `targets` (pid and start time) that still run, each read afresh: a
/// `System` keeps the processes it saw until it lists them all again, so one kept between
/// checks would show a process long gone (its parent reaped it) as running, and the stop
/// would wait out its grace and then kill the pid, whoever has it by then. A process whose
/// start time differs (its pid was taken since) or that is a zombie is not running.
fn running(targets: &[(u32, u64)]) -> (sysinfo::System, Vec<(u32, u64)>) {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pids: Vec<Pid> = targets.iter().map(|(p, _)| Pid::from_u32(*p)).collect();
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), ProcessRefreshKind::new());
    let alive = targets
        .iter()
        .copied()
        .filter(|&(pid, start)| {
            system.process(Pid::from_u32(pid)).is_some_and(|p| {
                p.start_time() == start
                    && !matches!(p.status(), sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead)
            })
        })
        .collect();
    (system, alive)
}
