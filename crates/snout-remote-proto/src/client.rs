//! The app's side: typed calls over a [`Mux`].

use crate::api::*;
use crate::message::Message;
use crate::mux::{Channel, Mux, Role};
use crate::{Hello, client_handshake};
use serde::de::DeserializeOwned;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// A connection to a snout-server. Clones share it; it closes when the last clone (and
/// every process, pty, watch and tunnel made from it) is gone, or on [`Client::close`].
#[derive(Clone)]
pub struct Client {
    mux: Mux,
    server: Arc<Hello>,
    _guard: Arc<Guard>,
}

struct Guard(Mux);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

/// Which standard streams of a process to connect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pipes {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
}

impl Pipes {
    pub const ALL: Pipes = Pipes { stdin: true, stdout: true, stderr: true };
}

impl Client {
    /// Handshake over `reader` and `writer` (the stdio of `ssh <host> snout-server --stdio`,
    /// or a socket), then run the connection. `closer` closes the stream (kills ssh).
    pub fn connect(
        reader: impl Read + Send + 'static,
        mut writer: impl Write + Send + 'static,
        closer: impl FnOnce() + Send + 'static,
    ) -> io::Result<Client> {
        let mut reader = BufReader::with_capacity(64 * 1024, reader);
        let server = match client_handshake(&mut reader, &mut writer) {
            Ok(h) => h,
            Err(e) => {
                closer();
                return Err(e);
            }
        };
        let mux = Mux::start(reader, writer, Role::Client, None, closer);
        Ok(Client { _guard: Arc::new(Guard(mux.clone())), mux, server: Arc::new(server) })
    }

    /// The server's hello.
    pub fn server(&self) -> &Hello {
        &self.server
    }

    pub fn mux(&self) -> &Mux {
        &self.mux
    }

    pub fn is_alive(&self) -> bool {
        self.mux.is_alive()
    }

    /// End the connection (the server's processes keep running).
    pub fn close(&self) {
        self.mux.shutdown();
    }

    fn call<T: DeserializeOwned>(&self, request: &Request, blob: Vec<u8>) -> io::Result<(T, Vec<u8>)> {
        let reply = self.mux.call(&Message::new(request, blob))?;
        match reply.parse::<Reply<T>>()? {
            Ok(t) => Ok((t, reply.blob)),
            Err(e) => Err(e.into()),
        }
    }

    fn get<T: DeserializeOwned>(&self, request: Request) -> io::Result<T> {
        Ok(self.call(&request, Vec::new())?.0)
    }

    /// The round trip time.
    pub fn ping(&self, timeout: Duration) -> io::Result<Duration> {
        let start = std::time::Instant::now();
        let reply = self.mux.call_timeout(&Message::new(&Request::Ping, Vec::new()), Some(timeout))?;
        reply.parse::<Reply<()>>()?.map_err(io::Error::from)?;
        Ok(start.elapsed())
    }

    pub fn info(&self) -> io::Result<HostInfo> {
        self.get(Request::Info)
    }

    pub fn read(&self, path: impl Into<PathBuf>) -> io::Result<Vec<u8>> {
        Ok(self.call::<()>(&Request::Read { path: path.into() }, Vec::new())?.1)
    }

    pub fn read_to_string(&self, path: impl Into<PathBuf>) -> io::Result<String> {
        String::from_utf8(self.read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Up to `len` bytes from `offset` on.
    pub fn read_range(&self, path: impl Into<PathBuf>, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        Ok(self.call::<()>(&Request::ReadRange { path: path.into(), offset, len }, Vec::new())?.1)
    }

    /// Atomic: readers see the old contents or the new, never a part.
    pub fn write(&self, path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) -> io::Result<()> {
        self.write_mode(path, contents, None)
    }

    /// [`Self::write`], the file getting the permission bits `mode` (else the ones it had).
    pub fn write_mode(
        &self,
        path: impl Into<PathBuf>,
        contents: impl Into<Vec<u8>>,
        mode: Option<u32>,
    ) -> io::Result<()> {
        self.call::<()>(&Request::Write { path: path.into(), mode }, contents.into()).map(|_| ())
    }

    /// Add `contents` to the end of `path` (made if missing).
    pub fn append(&self, path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) -> io::Result<()> {
        self.call::<()>(&Request::Append { path: path.into() }, contents.into()).map(|_| ())
    }

    pub fn create_dir_all(&self, path: impl Into<PathBuf>) -> io::Result<()> {
        self.create_dir_all_mode(path, None)
    }

    pub fn create_dir_all_mode(&self, path: impl Into<PathBuf>, mode: Option<u32>) -> io::Result<()> {
        self.get(Request::CreateDirAll { path: path.into(), mode })
    }

    pub fn walk(&self, root: impl Into<PathBuf>, options: WalkOptions) -> io::Result<Vec<WalkEntry>> {
        self.get(Request::Walk { root: root.into(), options })
    }

    pub fn find_text(
        &self,
        root: impl Into<PathBuf>,
        files: Vec<String>,
        needle: impl Into<String>,
        limit: usize,
    ) -> io::Result<Vec<TextHit>> {
        self.get(Request::FindText { root: root.into(), files, needle: needle.into(), limit })
    }

    pub fn system_processes(&self, cpu: bool) -> io::Result<Vec<SystemProcess>> {
        self.get(Request::SystemProcesses { cpu })
    }

    pub fn stop_processes(&self, targets: Vec<(u32, u64)>, grace: Duration) -> io::Result<usize> {
        self.get(Request::StopProcesses { targets, grace_ms: grace.as_millis() as u64 })
    }

    pub fn process_alive(&self, pid: u32) -> io::Result<bool> {
        self.get(Request::ProcessAlive { pid })
    }

    /// The login shell's environment on the server.
    pub fn login_env(&self) -> io::Result<std::collections::BTreeMap<String, String>> {
        self.get(Request::LoginEnv)
    }

    pub fn remove(&self, path: impl Into<PathBuf>, recursive: bool) -> io::Result<()> {
        self.get(Request::Remove { path: path.into(), recursive })
    }

    pub fn rename(&self, from: impl Into<PathBuf>, to: impl Into<PathBuf>) -> io::Result<()> {
        self.get(Request::Rename { from: from.into(), to: to.into() })
    }

    pub fn copy(&self, from: impl Into<PathBuf>, to: impl Into<PathBuf>, recursive: bool) -> io::Result<()> {
        self.get(Request::Copy { from: from.into(), to: to.into(), recursive })
    }

    pub fn metadata(&self, path: impl Into<PathBuf>, follow_symlinks: bool) -> io::Result<Metadata> {
        self.get(Request::Metadata { path: path.into(), follow_symlinks })
    }

    pub fn read_dir(&self, path: impl Into<PathBuf>) -> io::Result<Vec<DirEntry>> {
        self.get(Request::ReadDir { path: path.into() })
    }

    pub fn canonicalize(&self, path: impl Into<PathBuf>) -> io::Result<PathBuf> {
        self.get(Request::Canonicalize { path: path.into() })
    }

    pub fn exists(&self, path: impl Into<PathBuf>) -> io::Result<bool> {
        self.get(Request::Exists { path: path.into() })
    }

    pub fn trash(&self, path: impl Into<PathBuf>) -> io::Result<()> {
        self.get(Request::Trash { path: path.into() })
    }

    /// Watch `path`. Changes arrive in batches from [`Watch::next`].
    pub fn watch(&self, path: impl AsRef<Path>, recursive: bool) -> io::Result<Watch> {
        let channel = self.mux.open_channel()?;
        self.get::<()>(Request::Watch { path: path.as_ref().into(), recursive, channel: channel.id() })?;
        Ok(Watch { channel, _client: self.clone() })
    }

    fn open_pipes(&self, pipes: Pipes) -> io::Result<[Option<Channel>; 3]> {
        let open = |on: bool| on.then(|| self.mux.open_channel()).transpose();
        Ok([open(pipes.stdin)?, open(pipes.stdout)?, open(pipes.stderr)?])
    }

    /// Start a process; `request.stdio` is filled in from `pipes`.
    pub fn spawn(&self, mut request: SpawnRequest, pipes: Pipes) -> io::Result<RemoteProcess> {
        let [stdin, stdout, stderr] = self.open_pipes(pipes)?;
        let id = |c: &Option<Channel>| c.as_ref().map(Channel::id);
        request.stdio = Stdio { stdin: id(&stdin), stdout: id(&stdout), stderr: id(&stderr) };
        let s: Spawned = self.get(Request::Spawn(request))?;
        Ok(RemoteProcess { id: s.id, pid: s.pid, stdin, stdout, stderr, client: self.clone() })
    }

    /// Start a program on a pseudo terminal; `request.channel` is filled in.
    pub fn spawn_pty(&self, mut request: PtyRequest) -> io::Result<RemotePty> {
        let io = self.mux.open_channel()?;
        request.channel = io.id();
        let s: Spawned = self.get(Request::SpawnPty(request))?;
        Ok(RemotePty { id: s.id, pid: s.pid, io, client: self.clone() })
    }

    /// Connect to a process the server still runs (one started on an earlier connection).
    pub fn attach(&self, id: ProcessId, pipes: Pipes) -> io::Result<RemoteProcess> {
        let [stdin, stdout, stderr] = self.open_pipes(pipes)?;
        let ch = |c: &Option<Channel>| c.as_ref().map(Channel::id);
        let stdio = Stdio { stdin: ch(&stdin), stdout: ch(&stdout), stderr: ch(&stderr) };
        let s: Spawned = self.get(Request::Attach { id, stdio })?;
        Ok(RemoteProcess { id: s.id, pid: s.pid, stdin, stdout, stderr, client: self.clone() })
    }

    /// Connect to a pty the server still runs.
    pub fn attach_pty(&self, id: ProcessId) -> io::Result<RemotePty> {
        let io = self.mux.open_channel()?;
        let stdio = Stdio { stdin: None, stdout: Some(io.id()), stderr: None };
        let s: Spawned = self.get(Request::Attach { id, stdio })?;
        Ok(RemotePty { id: s.id, pid: s.pid, io, client: self.clone() })
    }

    pub fn kill(&self, id: ProcessId, signal: Signal) -> io::Result<()> {
        self.get(Request::Kill { id, signal })
    }

    pub fn wait(&self, id: ProcessId) -> io::Result<ExitStatus> {
        self.get(Request::Wait { id })
    }

    pub fn resize(&self, id: ProcessId, size: PtySize) -> io::Result<()> {
        self.get(Request::Resize { id, size })
    }

    /// Whether the process exited (it is not forgotten).
    pub fn status(&self, id: ProcessId) -> io::Result<Option<ExitStatus>> {
        self.get(Request::Status { id })
    }

    pub fn release(&self, id: ProcessId) -> io::Result<()> {
        self.get(Request::Release { id })
    }

    pub fn list_processes(&self) -> io::Result<Vec<ProcessInfo>> {
        self.get(Request::ListProcesses)
    }

    /// A TCP connection made from the server to `port` on its loopback interface.
    pub fn connect_tcp(&self, port: u16) -> io::Result<Channel> {
        self.connect_tcp_to(None, port)
    }

    pub fn connect_tcp_to(&self, host: Option<String>, port: u16) -> io::Result<Channel> {
        let channel = self.mux.open_channel()?;
        self.get::<()>(Request::ConnectTcp { host, port, channel: channel.id() })?;
        Ok(channel)
    }

    /// Listen on `port` of the server's loopback (a free one for 0); returns the port.
    pub fn listen(&self, port: u16) -> io::Result<u16> {
        self.get(Request::Listen { port })
    }

    /// The next connection to the listener of `port`, once one comes (blocks until then).
    pub fn accept(&self, port: u16) -> io::Result<Channel> {
        let channel = self.mux.open_channel()?;
        match self.get::<()>(Request::Accept { port, channel: channel.id() }) {
            Ok(()) => Ok(channel),
            Err(e) => {
                channel.close();
                Err(e)
            }
        }
    }

    pub fn unlisten(&self, port: u16) -> io::Result<()> {
        self.get(Request::Unlisten { port })
    }

    /// Stop the server and everything it runs.
    pub fn shutdown_server(&self) -> io::Result<()> {
        self.get(Request::Shutdown)
    }
}

/// A file watch; it stops when dropped.
pub struct Watch {
    channel: Channel,
    _client: Client,
}

impl Watch {
    /// The next batch of changes; `Ok(None)` once the watch ended. With a timeout, a
    /// `TimedOut` error when nothing changed in time.
    pub fn next(&self, timeout: Option<Duration>) -> io::Result<Option<Vec<WatchEvent>>> {
        match self.channel.recv_message(timeout)? {
            Some(m) => Ok(Some(m.parse()?)),
            None => Ok(None),
        }
    }
}

/// A process on the server. Dropping it closes its pipes (the process keeps running).
pub struct RemoteProcess {
    pub id: ProcessId,
    pub pid: u32,
    pub stdin: Option<Channel>,
    pub stdout: Option<Channel>,
    pub stderr: Option<Channel>,
    client: Client,
}

impl RemoteProcess {
    pub fn kill(&self, signal: Signal) -> io::Result<()> {
        self.client.kill(self.id, signal)
    }

    /// Wait for it to exit. The server forgets it then.
    pub fn wait(&self) -> io::Result<ExitStatus> {
        self.client.wait(self.id)
    }

    /// Close its stdin.
    pub fn close_stdin(&mut self) -> io::Result<()> {
        match self.stdin.take() {
            Some(c) => c.send_eof(),
            None => Ok(()),
        }
    }
}

/// A program on a pseudo terminal on the server. `io` reads the terminal's output and
/// writes its input.
pub struct RemotePty {
    pub id: ProcessId,
    pub pid: u32,
    pub io: Channel,
    client: Client,
}

impl RemotePty {
    pub fn resize(&self, size: PtySize) -> io::Result<()> {
        self.client.resize(self.id, size)
    }

    pub fn kill(&self, signal: Signal) -> io::Result<()> {
        self.client.kill(self.id, signal)
    }

    pub fn wait(&self) -> io::Result<ExitStatus> {
        self.client.wait(self.id)
    }
}
