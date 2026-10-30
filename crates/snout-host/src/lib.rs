//! The machine a SnoutCode workspace lives on, behind one interface: its files, a watch on
//! them, the processes and terminals that run there, connections to its ports, and what it
//! is (system, home, environment). [`LocalHost`] is the machine the app runs on (a Mac or a
//! Linux PC, called "this Mac" below); a remote host (a server on
//! another machine, spoken to over SSH: `snout_remote::RemoteHost`) implements the same
//! [`Host`].
//!
//! The app reaches the workspace only through a [`Host`]: project files and the file tree,
//! the editor's loads and saves, diffs, the file watcher, Git, the Java engine and its
//! JDKs, Gradle and project caches, run, debug and test processes, terminals and the Claude
//! console, Claude Code's IDE lock file, New Project, and the Trash. What belongs to the
//! app itself stays on the Mac and uses `std::fs` directly: the app's settings, layout and
//! session, recent projects, keymaps, fonts, art and logs, and the run configurations and
//! breakpoints it keeps per project in its own folder (`run::config::ProjectSettings`).
//!
//! Paths are [`PathBuf`]s in the host's own terms (both sides are Unix). A path from the
//! host is never assumed to exist on the Mac.
//!
//! A remote host answers every call over the network, so nothing should call it on the UI
//! thread. [`LatencyHost`] adds a delay to every call (`SNOUT_HOST_LATENCY_MS`, see
//! [`from_env`]) and reports the calls made on the UI thread, to find the ones left.
//!
//! Deliberate exceptions, where the workspace side does not (yet) go through a [`Host`]:
//! - Claude Code's IDE server and its hooks' endpoint (`snout-app`'s `claude_ide`) listen on
//!   127.0.0.1 of the Mac; [`Host::forward_to_app`] gives a remote host a port of its own
//!   that leads back to each, and the lock file and the hooks name that port.
//! - The engine distribution is looked for next to the program serving the host
//!   ([`HostInfo::exe`]), and in development in the source tree the app was built from; a
//!   remote host gets its jars copied from the Mac's (`snout-app`'s `remote_engine`).
//! - The start.spring.io catalog New Project offers is fetched on the Mac (`curl`) and kept
//!   in the app's folder (`new_project::initializr`); the project itself is made on the host.
//! - The token in the IDE lock file comes from the Mac's `/dev/urandom`.
//! - snout.sh's engine build status file (`background::EngineBuild`, development only) is
//!   read on the Mac.
//! - The folder pickers (Open, Find repositories, New Project's location) are the Mac's
//!   file dialogs; a remote host needs a picker of its own.
//! - Tests set up their fixtures with `std::fs` and `std::process`.
//!
//! The UI thread makes no host call: opening a project, the editor's loads, disk checks
//! and saves, Find References, language server edits, console links, the IDE tools, the
//! Settings sheet's JDKs and the session's tabs all ask the host off it. The app's
//! `host_probe` test drives the real app on a [`LatencyHost`] and fails on any call made
//! there (`SNOUT_HOST_PROBE=1 cargo test -p snout-app --test host_probe`).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

pub mod dirs;
mod latency;
mod local;
mod signals;
pub mod trash;
#[cfg(windows)]
pub mod win;

pub use latency::LatencyHost;
pub use local::{LocalHost, canonical};
pub use signals::default_signals;

/// A host as the app holds it.
pub type HostRef = Arc<dyn Host>;

/// The environment variable [`from_env`] reads: milliseconds to add to every call.
pub const LATENCY_ENV: &str = "SNOUT_HOST_LATENCY_MS";

/// This Mac, shared: the host the app and its tests use unless told otherwise.
pub fn local() -> HostRef {
    static LOCAL: OnceLock<HostRef> = OnceLock::new();
    LOCAL.get_or_init(|| Arc::new(LocalHost::new())).clone()
}

/// This Mac, slowed down by [`LATENCY_ENV`] milliseconds per call when it is set (to try the
/// app as a remote host would feel), the calling thread taken as the UI thread.
pub fn from_env() -> HostRef {
    match std::env::var(LATENCY_ENV).ok().and_then(|v| v.trim().parse::<u64>().ok()).filter(|ms| *ms > 0) {
        Some(ms) => Arc::new(LatencyHost::new(local(), Duration::from_millis(ms)).ui_thread_here()),
        None => local(),
    }
}

// ----- What the host is ------------------------------------------------------------------

/// What a host is, read once when it is reached.
#[derive(Clone, Debug, Default)]
pub struct HostInfo {
    /// `macos`, `linux`, … (as `std::env::consts::OS`).
    pub os: String,
    /// `aarch64`, `x86_64`, … (as `std::env::consts::ARCH`).
    pub arch: String,
    pub home: PathBuf,
    pub temp_dir: PathBuf,
    /// The environment of the program serving the host, which processes started there inherit.
    pub env: BTreeMap<String, String>,
    /// The program serving the host: the app on this Mac, the server on a remote one.
    pub exe: Option<PathBuf>,
    /// That program's process id.
    pub pid: u32,
    /// Whether [`Host::trash`] moves files to a Trash (otherwise there is none).
    pub can_trash: bool,
}

impl HostInfo {
    pub fn var(&self, name: &str) -> Option<&str> {
        self.env.get(name).map(String::as_str)
    }

    /// The folders of `PATH`, split as the host's system splits it (`;` on Windows, `:`
    /// elsewhere), whichever system the app runs on.
    pub fn path_dirs(&self) -> Vec<PathBuf> {
        let sep = if self.is_windows() { ';' } else { ':' };
        self.var("PATH")
            .or_else(|| self.is_windows().then(|| self.var("Path")).flatten())
            .map(|p| p.split(sep).filter(|d| !d.is_empty()).map(|d| PathBuf::from(d.trim_matches('"'))).collect())
            .unwrap_or_default()
    }

    pub fn is_mac(&self) -> bool {
        self.os == "macos"
    }

    pub fn is_windows(&self) -> bool {
        self.os == "windows"
    }

    /// The names a program `name` has on the host: on Windows `name` with each of the
    /// extensions of `PATHEXT` (`git.exe`, `gradlew.bat`) unless it has one already, then
    /// `name` itself; elsewhere just `name`.
    pub fn program_names(&self, name: &str) -> Vec<String> {
        if !self.is_windows() || Path::new(name).extension().is_some() {
            return vec![name.to_owned()];
        }
        let exts = self.var("PATHEXT").unwrap_or(".COM;.EXE;.BAT;.CMD");
        exts.split(';')
            .filter(|e| !e.is_empty())
            .map(|e| format!("{name}{}", e.to_ascii_lowercase()))
            .chain(std::iter::once(name.to_owned()))
            .collect()
    }
}

// ----- Files -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub kind: FileKind,
    pub len: u64,
    pub modified: Option<SystemTime>,
    /// The permission bits.
    pub mode: u32,
}

impl Metadata {
    pub fn is_file(&self) -> bool {
        self.kind == FileKind::File
    }

    pub fn is_dir(&self) -> bool {
        self.kind == FileKind::Dir
    }

    pub fn is_symlink(&self) -> bool {
        self.kind == FileKind::Symlink
    }
}

/// An entry of a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub path: PathBuf,
    /// What it is, a symbolic link not followed.
    pub kind: FileKind,
    /// Whether it is a folder or a link to one.
    pub is_dir: bool,
}

impl DirEntry {
    pub fn file_name(&self) -> &OsStr {
        self.path.file_name().unwrap_or_default()
    }
}

/// How [`Host::walk`] goes through a tree.
#[derive(Clone, Debug, Default)]
pub struct WalkOptions {
    /// How deep to go: 1 is the folder's own entries.
    pub max_depth: Option<usize>,
    /// Hidden files and folders too.
    pub hidden: bool,
    /// Leave out what `.gitignore` (and Git's other exclude files) leave out.
    pub gitignore: bool,
    /// Names never entered nor listed (`.git`).
    pub skip: Vec<String>,
    /// Files only, no folders.
    pub files_only: bool,
}

/// An entry [`Host::walk`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalkEntry {
    pub path: PathBuf,
    /// 1 for the root's own entries.
    pub depth: usize,
    pub is_dir: bool,
}

/// A line [`Host::find_text`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextHit {
    /// As it was given.
    pub path: String,
    /// From 0.
    pub line: u32,
    /// From 0, in bytes.
    pub column: u32,
    pub text: String,
}

// ----- Watching --------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchKind {
    Access,
    Create,
    Modify,
    Remove,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchEvent {
    pub kind: WatchKind,
    pub paths: Vec<PathBuf>,
}

/// A watch on a tree: its events arrive on [`Watcher::events`] until it is dropped.
pub struct Watcher {
    pub events: std::sync::mpsc::Receiver<WatchEvent>,
    _keep: Box<dyn Send>,
}

impl Watcher {
    /// A watcher whose events come on `events` while `keep` lives.
    pub fn new(events: std::sync::mpsc::Receiver<WatchEvent>, keep: Box<dyn Send>) -> Self {
        Self { events, _keep: keep }
    }
}

// ----- Processes -------------------------------------------------------------------------

/// What happens to one of a process's standard streams.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Stdio {
    #[default]
    Null,
    Piped,
    Inherit,
    /// A file of the host, made empty first (output only). Unlike a pipe, nothing is held
    /// open by processes the program leaves behind.
    File(PathBuf),
}

/// A program to start on a host.
#[derive(Clone, Debug)]
pub struct Cmd {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    /// Variables set (`Some`) or removed (`None`) on top of the inherited environment, in order.
    pub env: Vec<(OsString, Option<OsString>)>,
    pub stdin: Stdio,
    pub stdout: Stdio,
    pub stderr: Stdio,
}

impl Cmd {
    /// `program`, its stdin empty and its output captured.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_owned(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            stdin: Stdio::Null,
            stdout: Stdio::Piped,
            stderr: Stdio::Piped,
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    pub fn args<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, args: I) -> Self {
        self.args.extend(args.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    pub fn cwd(mut self, dir: impl AsRef<Path>) -> Self {
        self.cwd = Some(dir.as_ref().to_owned());
        self
    }

    pub fn env(mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env.push((name.as_ref().to_owned(), Some(value.as_ref().to_owned())));
        self
    }

    pub fn env_remove(mut self, name: impl AsRef<OsStr>) -> Self {
        self.env.push((name.as_ref().to_owned(), None));
        self
    }

    pub fn stdin(mut self, s: Stdio) -> Self {
        self.stdin = s;
        self
    }

    pub fn stdout(mut self, s: Stdio) -> Self {
        self.stdout = s;
        self
    }

    pub fn stderr(mut self, s: Stdio) -> Self {
        self.stderr = s;
        self
    }

    /// The program and its arguments, for messages.
    pub fn display(&self) -> String {
        std::iter::once(&self.program).chain(&self.args).map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ")
    }
}

/// How a process ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: Option<i32>,
    /// The signal that ended it, if one did.
    pub signal: Option<i32>,
}

impl ExitStatus {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

impl From<std::process::ExitStatus> for ExitStatus {
    #[cfg(unix)]
    fn from(s: std::process::ExitStatus) -> Self {
        use std::os::unix::process::ExitStatusExt as _;
        Self { code: s.code(), signal: s.signal() }
    }

    #[cfg(not(unix))]
    fn from(s: std::process::ExitStatus) -> Self {
        Self { code: s.code(), signal: None }
    }
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.code, self.signal) {
            (Some(code), _) => write!(f, "exit status: {code}"),
            (None, Some(signal)) => write!(f, "signal: {signal}"),
            (None, None) => write!(f, "exited"),
        }
    }
}

/// What a finished process wrote.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A process started on a host.
pub trait Process: Send {
    fn pid(&self) -> Option<u32>;
    /// Its stdin, when piped; once.
    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>>;
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>>;
    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>>;
    /// Ask it to end (SIGTERM).
    fn terminate(&mut self) -> io::Result<()>;
    /// End it now (SIGKILL).
    fn kill(&mut self) -> io::Result<()>;
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn wait(&mut self) -> io::Result<ExitStatus>;
}

/// A process on a pseudo-terminal.
pub trait PtyProcess: Send {
    fn pid(&self) -> Option<u32>;
    /// What the terminal shows; once.
    fn take_reader(&mut self) -> io::Result<Box<dyn Read + Send>>;
    /// What is typed; once.
    fn take_writer(&mut self) -> io::Result<Box<dyn Write + Send>>;
    fn resize(&self, rows: u16, cols: u16) -> io::Result<()>;
    fn kill(&mut self) -> io::Result<()>;
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn wait(&mut self) -> io::Result<ExitStatus>;
}

/// A connection to a port of the host.
pub trait Stream: Read + Write + Send + Sync {
    fn try_clone(&self) -> io::Result<Box<dyn Stream>>;
    /// Close both ways; readers and writers on clones see the end.
    fn shutdown(&self) -> io::Result<()>;
}

/// A port of the host whose connections arrive at a port of this Mac (see
/// [`Host::forward_to_app`]); the forwarding ends when it is dropped.
pub struct Forward {
    /// The port on the host's loopback.
    pub port: u16,
    _keep: Box<dyn Send + Sync>,
}

impl Forward {
    /// `port` on the host, forwarded while `keep` lives.
    pub fn new(port: u16, keep: Box<dyn Send + Sync>) -> Self {
        Self { port, _keep: keep }
    }
}

impl std::fmt::Debug for Forward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forward").field("port", &self.port).finish()
    }
}

/// A process of the host, as a listing of them sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cmd: Vec<String>,
    /// Resident memory, in bytes.
    pub memory: u64,
    /// Percent of one core, since the listing before.
    pub cpu: f32,
    /// Seconds since 1970 when it started.
    pub start: u64,
}

// ----- The host --------------------------------------------------------------------------

/// The machine a workspace lives on. Every call may take a network round trip on a remote
/// host: call it off the UI thread.
pub trait Host: Send + Sync {
    /// What the host is; read once, so this one is free.
    fn info(&self) -> &HostInfo;

    // Files.

    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        String::from_utf8(self.read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Up to `len` bytes from `offset` on.
    fn read_range(&self, path: &Path, offset: u64, len: u64) -> io::Result<Vec<u8>>;

    /// Write `data` to `path` all at once (through a file beside it, renamed into place), with
    /// the permission bits `mode`, else the file's own, else the default.
    fn write_mode(&self, path: &Path, data: &[u8], mode: Option<u32>) -> io::Result<()>;

    fn write(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        self.write_mode(path, data, None)
    }

    /// Add `data` to the end of `path`, made if missing (a large file written in pieces).
    fn append(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut all = match self.read(path) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        all.extend_from_slice(data);
        self.write(path, &all)
    }

    /// Create `path` and its missing parents, those it creates with the permission bits `mode`.
    fn create_dir_all_mode(&self, path: &Path, mode: Option<u32>) -> io::Result<()>;

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.create_dir_all_mode(path, None)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()>;
    /// An empty folder.
    fn remove_dir(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64>;
    /// Following symbolic links.
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;
    /// Not following a symbolic link.
    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata>;
    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    fn exists(&self, path: &Path) -> bool {
        self.metadata(path).is_ok()
    }

    fn is_file(&self, path: &Path) -> bool {
        self.metadata(path).is_ok_and(|m| m.is_file())
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.metadata(path).is_ok_and(|m| m.is_dir())
    }

    /// Move to the Trash (see [`HostInfo::can_trash`]).
    fn trash(&self, paths: &[PathBuf]) -> io::Result<()>;

    /// The entries under `root`, as `options` say, in no particular order.
    fn walk(&self, root: &Path, options: &WalkOptions) -> io::Result<Vec<WalkEntry>>;

    /// The lines of `files` (relative to `root`) holding `needle` (a plain string, whose case
    /// matters only when it has capitals), up to `limit`, binary files skipped.
    fn find_text(&self, root: &Path, files: &[String], needle: &str, limit: usize) -> io::Result<Vec<TextHit>>;

    /// Watch `root` and everything below it.
    fn watch(&self, root: &Path) -> io::Result<Watcher>;

    // Processes.

    fn spawn(&self, cmd: &Cmd) -> io::Result<Box<dyn Process>>;

    /// Run `cmd` to the end, `input` on its stdin, its piped output captured.
    fn run(&self, cmd: &Cmd, input: Option<&[u8]>) -> io::Result<Output> {
        let cmd = cmd.clone().stdin(if input.is_some() { Stdio::Piped } else { Stdio::Null });
        let mut child = self.spawn(&cmd)?;
        let collect = |r: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut r) = r {
                    let _ = r.read_to_end(&mut buf);
                }
                buf
            })
        };
        let (out, err) = (collect(child.take_stdout()), collect(child.take_stderr()));
        if let (Some(input), Some(mut stdin)) = (input, child.take_stdin()) {
            // A closed pipe means it already quit; its status says why.
            let _ = stdin.write_all(input);
        }
        let status = child.wait()?;
        Ok(Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() })
    }

    /// Start `cmd` on a terminal of `rows` × `cols`.
    fn spawn_pty(&self, cmd: &Cmd, rows: u16, cols: u16) -> io::Result<Box<dyn PtyProcess>>;

    /// Connect to `port` on the host's loopback, waiting up to `timeout`.
    fn connect_tcp(&self, port: u16, timeout: Duration) -> io::Result<Box<dyn Stream>>;

    /// Make connections to a port of the host's loopback arrive at `app_port` on this Mac's
    /// (the way back for a program on the host to a server of the app: Claude Code's IDE
    /// server and its hooks' endpoint). Returns the port on the host, kept through a
    /// reconnect. On this Mac itself that is `app_port`.
    fn forward_to_app(&self, app_port: u16) -> io::Result<Forward> {
        Ok(Forward::new(app_port, Box::new(())))
    }

    /// Every process of the host; with `cpu`, after measuring their CPU use if the listing
    /// before is too old to measure against.
    fn processes(&self, cpu: bool) -> io::Result<Vec<ProcInfo>>;

    /// Stop the processes `(pid, start)`: ask each to end, and kill those still there after
    /// `grace`. One whose start time differs (its pid was taken since) is left alone. Blocks
    /// for up to `grace`; returns how many it asked.
    fn stop_processes(&self, targets: &[(u32, u64)], grace: Duration) -> usize;

    /// Whether a process with this id runs.
    fn process_alive(&self, pid: u32) -> bool;
}

/// The first of `names` found in the folders of the host's `PATH`, then in `extra` (on a
/// Windows host with the extensions of `PATHEXT`, see [`HostInfo::program_names`]).
pub fn which(host: &dyn Host, names: &[&str], extra: &[PathBuf]) -> Option<PathBuf> {
    let info = host.info();
    let names: Vec<String> = names.iter().flat_map(|n| info.program_names(n)).collect();
    let dirs = info.path_dirs();
    dirs.iter().chain(extra).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| host.is_file(p))
}

/// `path` as text with `/` between its parts on every system (Windows's `\\` too): how the
/// app writes paths within a project (`src/main/A.java`), as Git does.
pub fn slash_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) { text.replace('\\', "/") } else { text.into_owned() }
}

/// The user's home folder on this machine: `HOME`, else (Windows) `USERPROFILE`.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .or_else(std::env::home_dir)
        .unwrap_or_default()
}

/// A time (seconds since 1970) in this machine's time zone: year, month (1 to 12), day, hour,
/// minute, second.
// `time_t` is deprecated on musl (it is to become 64-bit there); it is what `localtime_r` takes.
#[allow(deprecated)]
pub fn local_time(secs: i64) -> Option<[i32; 6]> {
    // SAFETY: `tm` is plain data, valid all zeros.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t = secs as libc::time_t;
    #[cfg(unix)]
    // SAFETY: both pointers are valid for the call.
    let ok = !unsafe { libc::localtime_r(&t, &mut tm) }.is_null();
    #[cfg(windows)]
    // SAFETY: both pointers are valid for the call.
    let ok = unsafe { libc::localtime_s(&mut tm, &t) } == 0;
    ok.then_some([tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec])
}

#[cfg(test)]
mod tests;
