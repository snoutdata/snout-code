//! Answers a connection's requests.

use crate::daemon::Daemon;
use crate::{fsops, tcp, trash, watch};
use serde_json::Value;
use snout_host::Host as _;
use snout_remote_proto::{Handler, Message, Mux, RemoteError, Reply, Request, SystemProcess, TextHit, WalkEntry};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub struct ServerHandler {
    pub daemon: Arc<Daemon>,
    /// This machine, as the app's own code walks, searches and lists it.
    pub local: Arc<snout_host::LocalHost>,
}

/// An I/O error, its message naming the path.
fn at(path: &Path) -> impl FnOnce(io::Error) -> RemoteError + '_ {
    move |e| RemoteError {
        kind: format!("{:?}", e.kind()),
        message: format!("{}: {e}", path.display()),
        os_code: e.raw_os_error(),
    }
}

fn reply<T: serde::Serialize>(r: Result<T, RemoteError>) -> Message {
    match r {
        Ok(v) => Message::new(&Reply::<T>::Ok(v), Vec::new()),
        Err(e) => Message::new(&Reply::<Value>::Err(e), Vec::new()),
    }
}

fn unit(r: Result<(), RemoteError>) -> Message {
    reply(r)
}

impl Handler for ServerHandler {
    fn handle(&self, mux: &Mux, request: Message) -> Message {
        let req: Request = match request.parse() {
            Ok(r) => r,
            Err(e) => return reply::<()>(Err(RemoteError::new(io::ErrorKind::InvalidInput, e.to_string()))),
        };
        let procs = &self.daemon.procs;
        match req {
            Request::Ping => unit(Ok(())),
            Request::Info => {
                let mut info = fsops::info();
                info.env = self.daemon.login_env();
                reply(Ok(info))
            }
            Request::Read { path } => match std::fs::read(&path) {
                Ok(bytes) => Message::new(&Reply::<()>::Ok(()), bytes),
                Err(e) => reply::<()>(Err(at(&path)(e))),
            },
            Request::ReadRange { path, offset, len } => match self.local.read_range(&path, offset, len) {
                Ok(bytes) => Message::new(&Reply::<()>::Ok(()), bytes),
                Err(e) => reply::<()>(Err(at(&path)(e))),
            },
            Request::Write { path, mode } => unit(fsops::write_atomic(&path, &request.blob, mode).map_err(at(&path))),
            Request::Append { path } => unit(fsops::append(&path, &request.blob).map_err(at(&path))),
            Request::CreateDirAll { path, mode } => {
                unit(self.local.create_dir_all_mode(&path, mode).map_err(at(&path)))
            }
            Request::Remove { path, recursive } => unit(fsops::remove(&path, recursive).map_err(at(&path))),
            Request::Rename { from, to } => unit(std::fs::rename(&from, &to).map_err(at(&from))),
            Request::Copy { from, to, recursive } => unit(fsops::copy(&from, &to, recursive).map_err(at(&from))),
            Request::Metadata { path, follow_symlinks } => {
                reply(fsops::metadata(&path, follow_symlinks).map_err(at(&path)))
            }
            Request::ReadDir { path } => reply(fsops::read_dir(&path).map_err(at(&path))),
            Request::Canonicalize { path } => reply(std::fs::canonicalize(&path).map_err(at(&path))),
            Request::Exists { path } => reply(path.try_exists().map_err(at(&path))),
            Request::Trash { path } => unit(trash::trash(&path).map_err(at(&path))),
            Request::Watch { path, recursive, channel } => {
                let r = mux.register_channel(channel).and_then(|ch| watch::start(path.clone(), recursive, ch));
                unit(r.map_err(at(&path)))
            }
            Request::Spawn(req) => {
                let program = req.program.clone();
                reply(procs.spawn(mux, req, &self.daemon.login_env()).map_err(at(Path::new(&program))))
            }
            Request::SpawnPty(req) => {
                let program = req.program.clone();
                reply(procs.spawn_pty(mux, req, &self.daemon.login_env()).map_err(at(Path::new(&program))))
            }
            Request::Attach { id, stdio } => reply(procs.attach(mux, id, stdio).map_err(RemoteError::from)),
            Request::Resize { id, size } => unit(procs.resize(id, size).map_err(RemoteError::from)),
            Request::Kill { id, signal } => unit(procs.kill(id, signal).map_err(RemoteError::from)),
            Request::Wait { id } => reply(procs.wait(id).map_err(RemoteError::from)),
            Request::Release { id } => unit(procs.release(id).map_err(RemoteError::from)),
            Request::Status { id } => reply(procs.status(id).map_err(RemoteError::from)),
            Request::ListProcesses => reply(Ok(procs.list())),
            Request::Walk { root, options } => {
                let options = snout_host::WalkOptions {
                    max_depth: options.max_depth,
                    hidden: options.hidden,
                    gitignore: options.gitignore,
                    skip: options.skip,
                    files_only: options.files_only,
                };
                let entries = self.local.walk(&root, &options).map_err(at(&root));
                reply(entries.map(|v| {
                    v.into_iter()
                        .map(|e| WalkEntry { path: e.path, depth: e.depth, is_dir: e.is_dir })
                        .collect::<Vec<_>>()
                }))
            }
            Request::FindText { root, files, needle, limit } => {
                let hits = self.local.find_text(&root, &files, &needle, limit).map_err(at(&root));
                reply(hits.map(|v| {
                    v.into_iter()
                        .map(|h| TextHit { path: h.path, line: h.line, column: h.column, text: h.text })
                        .collect::<Vec<_>>()
                }))
            }
            Request::SystemProcesses { cpu } => reply(self.local.processes(cpu).map_err(RemoteError::from).map(|v| {
                v.into_iter()
                    .map(|p| SystemProcess {
                        pid: p.pid,
                        parent: p.parent,
                        name: p.name,
                        cmd: p.cmd,
                        memory: p.memory,
                        cpu: p.cpu,
                        start: p.start,
                    })
                    .collect::<Vec<_>>()
            })),
            Request::StopProcesses { targets, grace_ms } => {
                reply(Ok(self.local.stop_processes(&targets, Duration::from_millis(grace_ms))))
            }
            Request::ProcessAlive { pid } => reply(Ok(self.local.process_alive(pid))),
            Request::LoginEnv => reply(Ok(self.daemon.login_env())),
            Request::ConnectTcp { host, port, channel } => {
                let host = host.unwrap_or_else(|| "127.0.0.1".into());
                let r = mux.register_channel(channel).and_then(|ch| tcp::connect(&host, port, ch));
                unit(r.map_err(|e| RemoteError {
                    kind: format!("{:?}", e.kind()),
                    message: format!("{host}:{port}: {e}"),
                    os_code: e.raw_os_error(),
                }))
            }
            Request::Shutdown => {
                // Answer first, then go.
                let daemon = self.daemon.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(100));
                    daemon.stop("a client asked");
                });
                unit(Ok(()))
            }
        }
    }
}
