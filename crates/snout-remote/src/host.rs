//! [`RemoteHost`]: a [`Host`] whose files, processes and terminals are on a snout-server,
//! reached through a [`Connector`] (ssh, in the app).
//!
//! The connection comes back by itself: when it drops, a thread connects again with
//! backoff while calls wait for it (a call never waits longer than the backoff gives up
//! after); once it gives up the host is [`Status::Lost`] and calls fail at once until
//! [`RemoteHost::reconnect`]. Processes and terminals live in the server's daemon, so they
//! keep running meanwhile: their streams attach again on the new connection and carry on
//! where they were (output waits on the server). A file watch starts again and reports a
//! change at its root, since what happened in between is unknown. TCP tunnels end.
//!
//! Calls that can be repeated without harm (reads, metadata, listings, writes of a whole
//! file) are made again on the new connection when the connection dropped under them;
//! the others (starting a process, renaming) fail with the connection's error.

use crate::Backoff;
use snout_host::{
    Cmd, DirEntry, ExitStatus, FileKind, Host, HostInfo, Metadata, ProcInfo, Process, PtyProcess, Stdio, Stream,
    TextHit, WalkEntry, WalkOptions, WatchEvent, WatchKind, Watcher,
};
use snout_remote_proto::{self as proto, Channel, Client, Pipes, ProcessId, Signal};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Makes connections to one server.
pub trait Connector: Send + Sync + 'static {
    /// One attempt at a new connection.
    fn connect(&self) -> io::Result<Client>;

    /// Before trying again after a failure (an ssh connection shared through a control
    /// socket may be hanging on to a dead link).
    fn reset(&self) {}

    /// The host, as the user knows it (`devbox`, `joel@10.0.0.5`).
    fn name(&self) -> String;
}

/// How the connection is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Connected,
    /// The connection dropped; connecting again. `attempt` counts the failed tries.
    Reconnecting {
        attempt: u32,
        error: Option<String>,
    },
    /// Reconnecting gave up; calls fail until [`RemoteHost::reconnect`].
    Lost(String),
    /// [`RemoteHost::close`]d.
    Closed,
}

struct LinkState {
    client: Option<Client>,
    /// Counts connections, so a stream knows whether it is on the current one.
    generation: u64,
    status: Status,
}

/// The connection, and its comings and goings.
struct Link {
    connector: Arc<dyn Connector>,
    backoff: Backoff,
    keepalive: Duration,
    st: Mutex<LinkState>,
    cv: Condvar,
}

fn not_connected(why: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, why.into())
}

impl Link {
    /// The current connection, waiting while one is being made.
    fn client(self: &Arc<Self>) -> io::Result<(Client, u64)> {
        let deadline = Instant::now() + self.backoff.give_up_after + Duration::from_secs(30);
        let mut st = lock(&self.st);
        loop {
            match &st.status {
                Status::Connected => match &st.client {
                    Some(c) if c.is_alive() => return Ok((c.clone(), st.generation)),
                    _ => self.start_reconnect(&mut st),
                },
                Status::Reconnecting { .. } => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "still reconnecting"));
                    }
                    st = self.cv.wait_timeout(st, deadline - now).unwrap_or_else(|p| p.into_inner()).0;
                }
                Status::Lost(why) => {
                    return Err(not_connected(format!("the connection to {} is lost: {why}", self.connector.name())));
                }
                Status::Closed => return Err(not_connected("the remote connection is closed")),
            }
        }
    }

    fn status(&self) -> Status {
        lock(&self.st).status.clone()
    }

    fn start_reconnect(self: &Arc<Self>, st: &mut LinkState) {
        st.status = Status::Reconnecting { attempt: 0, error: None };
        st.client = None;
        self.cv.notify_all();
        let link = self.clone();
        std::thread::Builder::new()
            .name("snout-remote-reconnect".into())
            .spawn(move || link.reconnect_loop())
            .expect("spawn a thread");
    }

    fn reconnect_loop(self: Arc<Self>) {
        let start = Instant::now();
        let mut delay = self.backoff.first;
        let mut attempt = 0;
        loop {
            if lock(&self.st).status == Status::Closed {
                return;
            }
            self.connector.reset();
            match self.connector.connect() {
                Ok(c) => {
                    self.adopt(c);
                    return;
                }
                Err(e) => {
                    attempt += 1;
                    let mut st = lock(&self.st);
                    if st.status == Status::Closed {
                        return;
                    }
                    if start.elapsed() + delay > self.backoff.give_up_after {
                        st.status = Status::Lost(e.to_string());
                        self.cv.notify_all();
                        return;
                    }
                    st.status = Status::Reconnecting { attempt, error: Some(e.to_string()) };
                    self.cv.notify_all();
                    // Sleep, but wake for a close.
                    let until = Instant::now() + delay;
                    while st.status != Status::Closed && Instant::now() < until {
                        let left = until.saturating_duration_since(Instant::now());
                        st = self.cv.wait_timeout(st, left).unwrap_or_else(|p| p.into_inner()).0;
                    }
                    delay = (delay * 2).min(self.backoff.max);
                }
            }
        }
    }

    /// Take `c` as the current connection, and watch it.
    fn adopt(self: &Arc<Self>, c: Client) {
        let generation = {
            let mut st = lock(&self.st);
            if st.status == Status::Closed {
                drop(st);
                c.close();
                return;
            }
            st.generation += 1;
            st.client = Some(c.clone());
            st.status = Status::Connected;
            self.cv.notify_all();
            st.generation
        };
        let weak = Arc::downgrade(self);
        c.mux().on_close(move || {
            if let Some(link) = weak.upgrade() {
                link.dropped(generation);
            }
        });
        let weak = Arc::downgrade(self);
        let every = self.keepalive;
        std::thread::Builder::new()
            .name("snout-remote-keepalive".into())
            .spawn(move || keepalive(weak, c, generation, every))
            .expect("spawn a thread");
    }

    /// Connection `generation` ended.
    fn dropped(self: &Arc<Self>, generation: u64) {
        let mut st = lock(&self.st);
        if st.generation == generation && st.status == Status::Connected {
            self.start_reconnect(&mut st);
        }
    }

    fn close(&self) {
        let client = {
            let mut st = lock(&self.st);
            st.status = Status::Closed;
            self.cv.notify_all();
            st.client.take()
        };
        if let Some(c) = client {
            c.close();
        }
    }

    fn reconnect(self: &Arc<Self>) {
        let mut st = lock(&self.st);
        if matches!(st.status, Status::Lost(_)) {
            self.start_reconnect(&mut st);
        }
    }
}

/// Ping the connection now and then; one that stops answering is closed, which starts a
/// reconnect (ssh's own keepalive takes much longer to notice).
fn keepalive(link: Weak<Link>, c: Client, generation: u64, every: Duration) {
    loop {
        std::thread::sleep(every);
        let Some(l) = link.upgrade() else { return };
        if !c.is_alive() || lock(&l.st).generation != generation {
            return;
        }
        drop(l);
        if let Err(e) = c.ping(every.max(Duration::from_secs(5))) {
            if e.kind() == io::ErrorKind::TimedOut {
                c.close();
            }
            return;
        }
    }
}

/// A remote machine as a [`Host`].
pub struct RemoteHost {
    link: Arc<Link>,
    info: HostInfo,
    remote: proto::HostInfo,
}

impl RemoteHost {
    /// Connect (once: an error comes back as it is) and read what the host is.
    pub fn connect(connector: Arc<dyn Connector>) -> io::Result<RemoteHost> {
        Self::connect_with(connector, Backoff::default(), Duration::from_secs(15))
    }

    /// [`Self::connect`], with the given reconnect backoff and keepalive period.
    pub fn connect_with(
        connector: Arc<dyn Connector>,
        backoff: Backoff,
        keepalive: Duration,
    ) -> io::Result<RemoteHost> {
        let client = connector.connect()?;
        let remote = client.info()?;
        let link = Arc::new(Link {
            connector,
            backoff,
            keepalive,
            st: Mutex::new(LinkState { client: None, generation: 0, status: Status::Connected }),
            cv: Condvar::new(),
        });
        link.adopt(client);
        let info = HostInfo {
            os: remote.os.clone(),
            arch: remote.arch.clone(),
            home: remote.home.clone(),
            temp_dir: remote.temp_dir.clone(),
            env: remote.env.clone(),
            exe: remote.exe.clone(),
            pid: remote.server_pid,
            can_trash: true,
        };
        Ok(RemoteHost { link, info, remote })
    }

    /// The host as the user named it.
    pub fn name(&self) -> String {
        self.link.connector.name()
    }

    /// What the server said about its machine when it was reached.
    pub fn remote_info(&self) -> &proto::HostInfo {
        &self.remote
    }

    pub fn status(&self) -> Status {
        self.link.status()
    }

    /// The current connection (waiting while one is being made), for what the [`Host`]
    /// interface has no call for.
    pub fn client(&self) -> io::Result<Client> {
        Ok(self.link.client()?.0)
    }

    /// End the connection for good: calls fail from now on. The server keeps running (it
    /// stops on its own when no client comes back).
    pub fn close(&self) {
        self.link.close();
    }

    /// Try again after [`Status::Lost`].
    pub fn reconnect(&self) {
        self.link.reconnect();
    }

    /// Drop the connection now, as a broken network would (for tests).
    pub fn drop_connection(&self) {
        if let Some(c) = lock(&self.link.st).client.clone() {
            c.close();
        }
    }

    /// Run `f` on the connection; when the connection dropped under it and `again`, once
    /// more on the next one.
    fn call<T>(&self, again: bool, f: impl Fn(&Client) -> io::Result<T>) -> io::Result<T> {
        let (c, _) = self.link.client()?;
        match f(&c) {
            Err(e) if !c.is_alive() => {
                if again {
                    let (c, _) = self.link.client()?;
                    f(&c)
                } else {
                    Err(e)
                }
            }
            r => r,
        }
    }
}

fn kind(k: proto::FileKind) -> FileKind {
    match k {
        proto::FileKind::File => FileKind::File,
        proto::FileKind::Dir => FileKind::Dir,
        proto::FileKind::Symlink => FileKind::Symlink,
        proto::FileKind::Other => FileKind::Other,
    }
}

fn metadata(m: proto::Metadata) -> Metadata {
    Metadata { kind: kind(m.kind), len: m.len, modified: m.modified, mode: m.mode }
}

fn status(s: proto::ExitStatus) -> ExitStatus {
    ExitStatus { code: s.code, signal: s.signal }
}

fn lossy(s: &std::ffi::OsStr) -> String {
    s.to_string_lossy().into_owned()
}

fn watch_kind(k: proto::WatchKind) -> WatchKind {
    match k {
        proto::WatchKind::Create => WatchKind::Create,
        proto::WatchKind::Modify | proto::WatchKind::Rename => WatchKind::Modify,
        proto::WatchKind::Remove => WatchKind::Remove,
        proto::WatchKind::Access => WatchKind::Access,
        proto::WatchKind::Rescan | proto::WatchKind::Other => WatchKind::Other,
    }
}

impl Host for RemoteHost {
    fn info(&self) -> &HostInfo {
        &self.info
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.call(true, |c| c.read(path))
    }

    fn read_range(&self, path: &Path, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        self.call(true, |c| c.read_range(path, offset, len))
    }

    fn write_mode(&self, path: &Path, data: &[u8], mode: Option<u32>) -> io::Result<()> {
        self.call(true, |c| c.write_mode(path, data.to_vec(), mode))
    }

    fn append(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        self.call(false, |c| c.append(path, data.to_vec()))
    }

    fn create_dir_all_mode(&self, path: &Path, mode: Option<u32>) -> io::Result<()> {
        self.call(true, |c| c.create_dir_all_mode(path, mode))
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        if self.symlink_metadata(path)?.is_dir() {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, format!("{}: is a folder", path.display())));
        }
        self.call(false, |c| c.remove(path, false))
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        if !self.symlink_metadata(path)?.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotADirectory, format!("{}: not a folder", path.display())));
        }
        self.call(false, |c| c.remove(path, false))
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.call(false, |c| c.remove(path, true))
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.call(false, |c| c.rename(from, to))
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        self.call(true, |c| {
            c.copy(from, to, false)?;
            Ok(c.metadata(to, true)?.len)
        })
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        self.call(true, |c| c.metadata(path, true)).map(metadata)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata> {
        self.call(true, |c| c.metadata(path, false)).map(metadata)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        let entries = self.call(true, |c| c.read_dir(path))?;
        Ok(entries
            .into_iter()
            .map(|e| {
                let path = path.join(&e.name);
                let kind = kind(e.kind);
                let is_dir = kind == FileKind::Dir || (kind == FileKind::Symlink && self.is_dir(&path));
                DirEntry { path, kind, is_dir }
            })
            .collect())
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.call(true, |c| c.canonicalize(path))
    }

    fn trash(&self, paths: &[PathBuf]) -> io::Result<()> {
        for p in paths {
            self.call(false, |c| c.trash(p))?;
        }
        Ok(())
    }

    fn walk(&self, root: &Path, options: &WalkOptions) -> io::Result<Vec<WalkEntry>> {
        let options = proto::WalkOptions {
            max_depth: options.max_depth,
            hidden: options.hidden,
            gitignore: options.gitignore,
            skip: options.skip.clone(),
            files_only: options.files_only,
        };
        let entries = self.call(true, |c| c.walk(root, options.clone()))?;
        Ok(entries.into_iter().map(|e| WalkEntry { path: e.path, depth: e.depth, is_dir: e.is_dir }).collect())
    }

    fn find_text(&self, root: &Path, files: &[String], needle: &str, limit: usize) -> io::Result<Vec<TextHit>> {
        let hits = self.call(true, |c| c.find_text(root, files.to_vec(), needle, limit))?;
        Ok(hits.into_iter().map(|h| TextHit { path: h.path, line: h.line, column: h.column, text: h.text }).collect())
    }

    fn watch(&self, root: &Path) -> io::Result<Watcher> {
        let watch = self.call(true, |c| c.watch(root, true))?;
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (link, root, stopped) = (self.link.clone(), root.to_owned(), stop.clone());
        std::thread::Builder::new()
            .name("snout-remote-watch".into())
            .spawn(move || forward_watch(link, root, watch, tx, stopped))?;
        Ok(Watcher::new(rx, Box::new(StopOnDrop(stop))))
    }

    fn spawn(&self, cmd: &Cmd) -> io::Result<Box<dyn Process>> {
        let mut req = proto::SpawnRequest {
            program: lossy(&cmd.program),
            args: cmd.args.iter().map(|a| lossy(a)).collect(),
            cwd: cmd.cwd.clone(),
            ..Default::default()
        };
        for (k, v) in &cmd.env {
            match v {
                Some(v) => req.env.push((lossy(k), lossy(v))),
                None => req.env_remove.push(lossy(k)),
            }
        }
        let file = |s: &Stdio| match s {
            Stdio::File(p) => Some(p.clone()),
            _ => None,
        };
        req.stdout_file = file(&cmd.stdout);
        req.stderr_file = file(&cmd.stderr);
        let pipes = Pipes {
            stdin: cmd.stdin == Stdio::Piped,
            stdout: cmd.stdout == Stdio::Piped,
            stderr: cmd.stderr == Stdio::Piped,
        };
        let (c, generation) = self.link.client()?;
        let p = c.spawn(req, pipes)?;
        Ok(Box::new(RemoteProc::new(self.link.clone(), generation, p.id, p.pid, false, [p.stdin, p.stdout, p.stderr])))
    }

    fn spawn_pty(&self, cmd: &Cmd, rows: u16, cols: u16) -> io::Result<Box<dyn PtyProcess>> {
        let mut req = proto::PtyRequest {
            program: lossy(&cmd.program),
            args: cmd.args.iter().map(|a| lossy(a)).collect(),
            cwd: cmd.cwd.clone(),
            size: proto::PtySize { rows, cols },
            ..Default::default()
        };
        for (k, v) in &cmd.env {
            match v {
                Some(v) => req.env.push((lossy(k), lossy(v))),
                None => req.env_remove.push(lossy(k)),
            }
        }
        let (c, generation) = self.link.client()?;
        let p = c.spawn_pty(req)?;
        Ok(Box::new(RemoteProc::new(self.link.clone(), generation, p.id, p.pid, true, [None, Some(p.io), None])))
    }

    fn connect_tcp(&self, port: u16, _timeout: Duration) -> io::Result<Box<dyn Stream>> {
        let channel = self.call(false, |c| c.connect_tcp(port))?;
        Ok(Box::new(Tunnel(channel)))
    }

    fn processes(&self, cpu: bool) -> io::Result<Vec<ProcInfo>> {
        let list = self.call(true, |c| c.system_processes(cpu))?;
        Ok(list
            .into_iter()
            .map(|p| ProcInfo {
                pid: p.pid,
                parent: p.parent,
                name: p.name,
                cmd: p.cmd,
                memory: p.memory,
                cpu: p.cpu,
                start: p.start,
            })
            .collect())
    }

    fn stop_processes(&self, targets: &[(u32, u64)], grace: Duration) -> usize {
        self.call(false, |c| c.stop_processes(targets.to_vec(), grace)).unwrap_or(0)
    }

    fn process_alive(&self, pid: u32) -> bool {
        self.call(true, |c| c.process_alive(pid)).unwrap_or(false)
    }
}

struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Carry a watch's events to the [`Watcher`]; watch again after a reconnect.
fn forward_watch(
    link: Arc<Link>,
    root: PathBuf,
    mut watch: proto::Watch,
    tx: std::sync::mpsc::Sender<WatchEvent>,
    stop: Arc<AtomicBool>,
) {
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        match watch.next(Some(Duration::from_millis(500))) {
            Ok(Some(events)) => {
                for e in events {
                    if tx.send(WatchEvent { kind: watch_kind(e.kind), paths: e.paths }).is_err() {
                        return;
                    }
                }
            }
            Ok(None) => return,
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => {
                // The connection dropped: watch again on the next one, and say that
                // anything may have changed meanwhile.
                loop {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    match link.client().and_then(|(c, _)| c.watch(&root, true)) {
                        Ok(w) => {
                            watch = w;
                            break;
                        }
                        Err(_) if matches!(link.status(), Status::Lost(_) | Status::Closed) => return,
                        Err(_) => std::thread::sleep(Duration::from_millis(500)),
                    }
                }
                if tx.send(WatchEvent { kind: WatchKind::Other, paths: vec![root.clone()] }).is_err() {
                    return;
                }
            }
        }
    }
}

/// A TCP connection made on the remote machine.
struct Tunnel(Channel);

impl Read for Tunnel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }
}

impl Write for Tunnel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stream for Tunnel {
    fn try_clone(&self) -> io::Result<Box<dyn Stream>> {
        Ok(Box::new(Tunnel(self.0.clone())))
    }

    fn shutdown(&self) -> io::Result<()> {
        self.0.close();
        Ok(())
    }
}

// ----- Processes and terminals ----------------------------------------------------------

const STDIN: usize = 0;
const STDOUT: usize = 1;
const STDERR: usize = 2;

struct Streams {
    /// The connection the channels are on.
    generation: u64,
    channels: [Option<Channel>; 3],
    /// The streams still wanted (a stream whose reader or writer was dropped is not
    /// attached again).
    wanted: [bool; 3],
    /// The server no longer knows the process: its streams have ended.
    gone: bool,
}

/// A process or pty of the server, with its streams, which follow the connection.
struct ProcLink {
    link: Arc<Link>,
    id: ProcessId,
    pid: u32,
    pty: bool,
    streams: Mutex<Streams>,
    exit: Mutex<Option<ExitStatus>>,
}

impl ProcLink {
    /// Stream `i`'s channel and the connection it is on; `None` once it has ended.
    fn channel(&self, i: usize) -> Option<(Channel, u64)> {
        let s = lock(&self.streams);
        if s.gone {
            return None;
        }
        s.channels[i].clone().map(|c| (c, s.generation))
    }

    /// Attach the streams on a new connection, unless another stream did since connection
    /// `generation` (the one that failed).
    fn reattach(&self, generation: u64) -> io::Result<()> {
        let mut s = lock(&self.streams);
        if s.generation != generation || s.gone {
            return Ok(());
        }
        let (c, now) = self.link.client()?;
        let attached = if self.pty {
            c.attach_pty(self.id).map(|p| [None, Some(p.io), None])
        } else {
            let pipes = Pipes { stdin: s.wanted[STDIN], stdout: s.wanted[STDOUT], stderr: s.wanted[STDERR] };
            c.attach(self.id, pipes).map(|p| [p.stdin, p.stdout, p.stderr])
        };
        match attached {
            Ok(channels) => {
                s.channels = channels;
                s.generation = now;
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                s.gone = true;
                s.channels = [None, None, None];
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn unwant(&self, i: usize) {
        let mut s = lock(&self.streams);
        s.wanted[i] = false;
        if !self.pty || i != STDOUT {
            s.channels[i] = None;
        }
    }

    fn read(&self, i: usize, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let Some((ch, generation)) = self.channel(i) else { return Ok(0) };
            match ch.recv(buf) {
                Err(e) if e.kind() == io::ErrorKind::ConnectionAborted => self.reattach(generation)?,
                r => return r,
            }
        }
    }

    fn write(&self, i: usize, buf: &[u8]) -> io::Result<usize> {
        let mut tried = false;
        loop {
            let Some((ch, generation)) = self.channel(i) else {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "the process has ended"));
            };
            match ch.send(buf) {
                Ok(()) => return Ok(buf.len()),
                Err(e) if e.kind() == io::ErrorKind::ConnectionAborted && !tried => {
                    tried = true;
                    self.reattach(generation)?;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn signal(&self, signal: Signal) -> io::Result<()> {
        if lock(&self.exit).is_some() {
            return Ok(());
        }
        let (c, _) = self.link.client()?;
        match c.kill(self.id, signal) {
            Err(e) if !c.is_alive() => {
                let (c, _) = self.link.client()?;
                c.kill(self.id, signal).map_err(|_| e)
            }
            r => r,
        }
    }

    fn try_wait(&self) -> io::Result<Option<ExitStatus>> {
        if let Some(s) = *lock(&self.exit) {
            return Ok(Some(s));
        }
        let (c, _) = self.link.client()?;
        match c.status(self.id) {
            Ok(Some(s)) => {
                let s = status(s);
                *lock(&self.exit) = Some(s);
                let _ = c.release(self.id);
                Ok(Some(s))
            }
            Ok(None) => Ok(None),
            // Forgotten: the server stopped since.
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Some(self.ended_unknown())),
            Err(e) => Err(e),
        }
    }

    fn ended_unknown(&self) -> ExitStatus {
        *lock(&self.exit).get_or_insert(ExitStatus { code: None, signal: None })
    }

    fn wait(&self) -> io::Result<ExitStatus> {
        let mut reconnected = false;
        loop {
            if let Some(s) = *lock(&self.exit) {
                return Ok(s);
            }
            let (c, _) = self.link.client()?;
            match c.wait(self.id) {
                Ok(s) => {
                    let s = status(s);
                    *lock(&self.exit) = Some(s);
                    return Ok(s);
                }
                Err(_) if !c.is_alive() => reconnected = true,
                // After a drop, the answer may have been lost on the way: the server
                // forgot the process when it answered.
                Err(e) if e.kind() == io::ErrorKind::NotFound && reconnected => return Ok(self.ended_unknown()),
                Err(e) => return Err(e),
            }
        }
    }
}

struct RemoteProc {
    p: Arc<ProcLink>,
    taken: [bool; 3],
}

impl RemoteProc {
    fn new(
        link: Arc<Link>,
        generation: u64,
        id: ProcessId,
        pid: u32,
        pty: bool,
        channels: [Option<Channel>; 3],
    ) -> Self {
        let wanted = [channels[0].is_some(), channels[1].is_some(), channels[2].is_some()];
        let streams = Mutex::new(Streams { generation, channels, wanted, gone: false });
        RemoteProc { p: Arc::new(ProcLink { link, id, pid, pty, streams, exit: Mutex::new(None) }), taken: [false; 3] }
    }

    fn take(&mut self, i: usize) -> Option<StreamEnd> {
        if self.taken[i] || !lock(&self.p.streams).wanted[i] {
            return None;
        }
        self.taken[i] = true;
        Some(StreamEnd { p: self.p.clone(), i, writer: i == STDIN, done: false })
    }
}

impl Drop for RemoteProc {
    fn drop(&mut self) {
        // The server keeps a process until its status is taken: take it when it comes.
        if lock(&self.p.exit).is_none() {
            let p = self.p.clone();
            let _ = std::thread::Builder::new().name("snout-remote-reap".into()).spawn(move || {
                let _ = p.wait();
            });
        }
    }
}

/// A reader or writer of one of a process's streams.
struct StreamEnd {
    p: Arc<ProcLink>,
    i: usize,
    writer: bool,
    done: bool,
}

impl Read for StreamEnd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.p.read(if self.p.pty { STDOUT } else { self.i }, buf)
    }
}

impl Write for StreamEnd {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.p.write(if self.p.pty { STDOUT } else { self.i }, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for StreamEnd {
    fn drop(&mut self) {
        if self.done || self.p.pty {
            return;
        }
        self.done = true;
        if self.writer {
            // The process sees the end of its input.
            if let Some((ch, _)) = self.p.channel(self.i) {
                let _ = ch.send_eof();
            }
        }
        self.p.unwant(self.i);
    }
}

impl Process for RemoteProc {
    fn pid(&self) -> Option<u32> {
        Some(self.p.pid)
    }

    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>> {
        self.take(STDIN).map(|s| Box::new(s) as _)
    }

    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.take(STDOUT).map(|s| Box::new(s) as _)
    }

    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.take(STDERR).map(|s| Box::new(s) as _)
    }

    fn terminate(&mut self) -> io::Result<()> {
        self.p.signal(Signal::Term)
    }

    fn kill(&mut self) -> io::Result<()> {
        self.p.signal(Signal::Kill)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.p.try_wait()
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        self.p.wait()
    }
}

impl PtyProcess for RemoteProc {
    fn pid(&self) -> Option<u32> {
        Some(self.p.pid)
    }

    fn take_reader(&mut self) -> io::Result<Box<dyn Read + Send>> {
        if self.taken[STDOUT] {
            return Err(io::Error::other("the terminal's reader was taken"));
        }
        self.taken[STDOUT] = true;
        Ok(Box::new(StreamEnd { p: self.p.clone(), i: STDOUT, writer: false, done: false }))
    }

    fn take_writer(&mut self) -> io::Result<Box<dyn Write + Send>> {
        if self.taken[STDIN] {
            return Err(io::Error::other("the terminal's writer was taken"));
        }
        self.taken[STDIN] = true;
        Ok(Box::new(StreamEnd { p: self.p.clone(), i: STDIN, writer: true, done: false }))
    }

    fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        let (c, _) = self.p.link.client()?;
        c.resize(self.p.id, proto::PtySize { rows, cols })
    }

    fn kill(&mut self) -> io::Result<()> {
        self.p.signal(Signal::Kill)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.p.try_wait()
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        self.p.wait()
    }
}
