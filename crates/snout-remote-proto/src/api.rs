//! Requests and replies.
//!
//! A request is a [`Request`] header, with the file contents as the blob for
//! [`Request::Write`]. The reply header is a [`Reply`] of the type each request names,
//! with the file contents as the blob for [`Request::Read`].
//!
//! Requests that start a stream name the channel it uses; the client opens the channel
//! before it sends the request, so nothing the server sends on it can arrive too early.
//!
//! Processes and ptys belong to the server, not to the connection: they keep running when
//! the connection drops, their output waiting (up to a limit, then their pipes fill and
//! they block) until a client attaches again with [`Request::Attach`]. They go when they
//! have exited and a [`Request::Wait`] has returned their status, on
//! [`Request::Release`], or when the server stops. File watches and TCP tunnels end with
//! the connection.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

pub type ChannelId = u32;

/// A process or pty the server runs, unique for the server's lifetime.
pub type ProcessId = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Reply `()`.
    Ping,
    /// Reply [`HostInfo`].
    Info,
    /// Reply `()`, the contents as the blob.
    Read { path: PathBuf },
    /// Atomic: the contents (the blob) go to a temporary file in the same folder, which then
    /// replaces `path`, keeping its permissions. Reply `()`.
    Write { path: PathBuf },
    /// Reply `()`.
    CreateDirAll { path: PathBuf },
    /// A file, a symlink, an empty folder, or with `recursive` any folder. Reply `()`.
    Remove { path: PathBuf, recursive: bool },
    /// Reply `()`.
    Rename { from: PathBuf, to: PathBuf },
    /// A file, or with `recursive` a folder and everything in it. Reply `()`.
    Copy { from: PathBuf, to: PathBuf, recursive: bool },
    /// Reply [`Metadata`].
    Metadata { path: PathBuf, follow_symlinks: bool },
    /// Reply `Vec<DirEntry>`, in no particular order.
    ReadDir { path: PathBuf },
    /// Reply `PathBuf`.
    Canonicalize { path: PathBuf },
    /// Reply `bool`.
    Exists { path: PathBuf },
    /// Moves `path` to the user's trash (the freedesktop.org trash on Linux). Reply `()`.
    Trash { path: PathBuf },
    /// Watch `path`; each change arrives on `channel` as a message with a `Vec<WatchEvent>`
    /// header. Close the channel to stop. Reply `()`.
    Watch { path: PathBuf, recursive: bool, channel: ChannelId },
    /// Reply [`Spawned`].
    Spawn(SpawnRequest),
    /// Reply [`Spawned`].
    SpawnPty(PtyRequest),
    /// Connect new channels to a running process (after a reconnect). The channels it had
    /// are closed. For a pty, `stdio.stdout` carries both directions. Reply [`Spawned`].
    Attach { id: ProcessId, stdio: Stdio },
    /// Reply `()`.
    Resize { id: ProcessId, size: PtySize },
    /// Signal the process's group. Reply `()`.
    Kill { id: ProcessId, signal: Signal },
    /// Blocks until the process exits. Reply [`ExitStatus`].
    Wait { id: ProcessId },
    /// Forget a process (it keeps running if it still is). Reply `()`.
    Release { id: ProcessId },
    /// Reply `Vec<ProcessInfo>`.
    ListProcesses,
    /// Connect to `host` (default 127.0.0.1) and `port` from the server, and carry the
    /// connection's bytes on `channel`. Reply `()`.
    ConnectTcp { host: Option<String>, port: u16, channel: ChannelId },
    /// Stop the server and every process it runs. Reply `()`.
    Shutdown,
}

/// The channels of a process's standard streams; `None` connects the stream to
/// `/dev/null` (at spawn) or leaves it unread (at attach).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stdio {
    pub stdin: Option<ChannelId>,
    pub stdout: Option<ChannelId>,
    pub stderr: Option<ChannelId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnRequest {
    pub program: String,
    pub args: Vec<String>,
    /// Variables to set, over the server's environment (or none, with `env_clear`).
    pub env: Vec<(String, String)>,
    pub env_remove: Vec<String>,
    pub env_clear: bool,
    pub cwd: Option<PathBuf>,
    pub stdio: Stdio,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    pub size: PtySize,
    /// Carries the terminal's output from the server and its input to it.
    pub channel: ChannelId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        PtySize { rows: 24, cols: 80 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    Hup,
    Int,
    Quit,
    Kill,
    Term,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Spawned {
    pub id: ProcessId,
    pub pid: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExitStatus {
    /// The exit code, when the process exited by itself.
    pub code: Option<i32>,
    /// The signal that ended it, otherwise.
    pub signal: Option<i32>,
}

impl ExitStatus {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessInfo {
    pub id: ProcessId,
    pub pid: u32,
    pub program: String,
    pub args: Vec<String>,
    pub pty: bool,
    /// `None` while it runs.
    pub exit: Option<ExitStatus>,
    /// Whether a connected client reads its output.
    pub attached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostInfo {
    /// As Rust names them: `linux`, `macos`.
    pub os: String,
    /// `x86_64`, `aarch64`.
    pub arch: String,
    pub home: PathBuf,
    pub temp_dir: PathBuf,
    pub user: String,
    pub hostname: String,
    /// The server's environment.
    pub env: BTreeMap<String, String>,
    pub server_version: String,
    pub server_pid: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metadata {
    pub kind: FileKind,
    pub len: u64,
    /// Unix permission bits.
    pub mode: u32,
    pub readonly: bool,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub created: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirEntry {
    /// The file name (lossy for a name that is not UTF-8).
    pub name: String,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchKind {
    Create,
    Modify,
    Remove,
    /// The paths are the old name then the new one, when both are known.
    Rename,
    Access,
    /// Events were lost: whatever is watched may have changed anywhere.
    Rescan,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchEvent {
    pub kind: WatchKind,
    pub paths: Vec<PathBuf>,
}

/// A failed request: the I/O error on the server, kept as an [`io::ErrorKind`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteError {
    /// The [`io::ErrorKind`]'s name (`NotFound`).
    pub kind: String,
    pub message: String,
    pub os_code: Option<i32>,
}

/// A reply header.
pub type Reply<T> = Result<T, RemoteError>;

const KINDS: &[io::ErrorKind] = &[
    io::ErrorKind::NotFound,
    io::ErrorKind::PermissionDenied,
    io::ErrorKind::ConnectionRefused,
    io::ErrorKind::ConnectionReset,
    io::ErrorKind::ConnectionAborted,
    io::ErrorKind::NotConnected,
    io::ErrorKind::AddrInUse,
    io::ErrorKind::AddrNotAvailable,
    io::ErrorKind::BrokenPipe,
    io::ErrorKind::AlreadyExists,
    io::ErrorKind::WouldBlock,
    io::ErrorKind::NotADirectory,
    io::ErrorKind::IsADirectory,
    io::ErrorKind::DirectoryNotEmpty,
    io::ErrorKind::ReadOnlyFilesystem,
    io::ErrorKind::StorageFull,
    io::ErrorKind::CrossesDevices,
    io::ErrorKind::InvalidInput,
    io::ErrorKind::InvalidData,
    io::ErrorKind::InvalidFilename,
    io::ErrorKind::TimedOut,
    io::ErrorKind::Interrupted,
    io::ErrorKind::Unsupported,
    io::ErrorKind::UnexpectedEof,
    io::ErrorKind::OutOfMemory,
    io::ErrorKind::ResourceBusy,
    io::ErrorKind::ExecutableFileBusy,
    io::ErrorKind::Other,
];

impl RemoteError {
    pub fn new(kind: io::ErrorKind, message: impl Into<String>) -> Self {
        RemoteError { kind: format!("{kind:?}"), message: message.into(), os_code: None }
    }

    pub fn io_kind(&self) -> io::ErrorKind {
        KINDS.iter().copied().find(|k| format!("{k:?}") == self.kind).unwrap_or(io::ErrorKind::Other)
    }
}

impl From<io::Error> for RemoteError {
    fn from(e: io::Error) -> Self {
        RemoteError { kind: format!("{:?}", e.kind()), message: e.to_string(), os_code: e.raw_os_error() }
    }
}

impl From<RemoteError> for io::Error {
    fn from(e: RemoteError) -> Self {
        io::Error::new(e.io_kind(), e.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_keep_their_kind_across_the_wire() {
        for kind in KINDS {
            let e: RemoteError = io::Error::new(*kind, "boom").into();
            let back: RemoteError = serde_json::from_slice(&serde_json::to_vec(&e).unwrap()).unwrap();
            assert_eq!(io::Error::from(back).kind(), *kind);
        }
        let os: RemoteError = io::Error::from_raw_os_error(2).into();
        assert_eq!(os.os_code, Some(2));
        assert_eq!(os.io_kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn requests_are_tagged_json() {
        let r = Request::Remove { path: "/tmp/x".into(), recursive: true };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json, serde_json::json!({"op": "remove", "path": "/tmp/x", "recursive": true}));
        let spawn = Request::Spawn(SpawnRequest { program: "git".into(), ..Default::default() });
        let back: Request = serde_json::from_slice(&serde_json::to_vec(&spawn).unwrap()).unwrap();
        assert_eq!(back, spawn);
    }
}
