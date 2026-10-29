//! The WebSocket server Claude Code connects to: 127.0.0.1 only, on a port the system
//! picks, each client checked against the lock file's token in the
//! `x-claude-code-ide-authorization` header. A `claude` on a remote machine reaches it
//! through a port of that machine forwarded back here ([`snout_host::Host::forward_to_app`]):
//! that port is the one the lock file there names. Messages are JSON-RPC 2.0 with MCP's
//! `initialize`, `tools/list` and `tools/call`. The server answers what it can by itself;
//! tool calls go to the app through the event callback, with a [`Reply`] to answer them
//! when the app is ready (an `openDiff` waits for the user).
//!
//! Every connection runs on a thread of its own that reads with a short timeout and, in
//! between, writes what the app queued for it: answers and notifications.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use serde_json::{Value, json};
use snout_host::HostRef;
use tungstenite::Message;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

use super::{lockfile, tools};

/// The header a client presents the token in.
pub const AUTH_HEADER: &str = "x-claude-code-ide-authorization";

/// The MCP revision this server speaks, unless the client asks for one it knows.
pub const PROTOCOL_VERSION: &str = "2025-03-26";
const KNOWN_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// How long a connection waits for input before writing what is queued.
const POLL: Duration = Duration::from_millis(20);

/// What the server tells the app.
pub enum Event {
    /// A client authenticated; its id.
    Connected(u64),
    Disconnected(u64),
    /// A tool call to answer.
    Call(Call),
}

/// A `tools/call` request.
pub struct Call {
    pub client: u64,
    pub name: String,
    pub arguments: Value,
    pub reply: Reply,
}

/// The way back to the client that made a request. Dropping it unanswered answers with
/// an error, so a client never waits forever.
pub struct Reply {
    id: Value,
    out: mpsc::Sender<String>,
    sent: bool,
}

impl Reply {
    fn send(&mut self, body: Value) {
        self.sent = true;
        let mut msg = json!({ "jsonrpc": "2.0", "id": self.id });
        if let (Some(m), Some(b)) = (msg.as_object_mut(), body.as_object()) {
            m.extend(b.clone());
        }
        let _ = self.out.send(msg.to_string());
    }

    pub fn ok(mut self, result: Value) {
        self.send(json!({ "result": result }));
    }

    pub fn error(mut self, code: i64, message: &str, data: Option<String>) {
        let mut error = json!({ "code": code, "message": message });
        if let Some(data) = data {
            error["data"] = json!(data);
        }
        self.send(json!({ "error": error }));
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if !self.sent {
            self.send(json!({ "error": { "code": tools::TOOL_ERROR, "message": "The request was dropped" } }));
        }
    }
}

type Emit = Arc<dyn Fn(Event) + Send + Sync>;

struct Shared {
    clients: Mutex<HashMap<u64, mpsc::Sender<String>>>,
    stop: AtomicBool,
    next: AtomicU64,
    token: String,
    emit: Emit,
}

/// What the lock file's worker does, in order, off the UI thread.
type LockJob = Box<dyn FnOnce() + Send>;

/// Where the lock file is: the port the host knows the server by, and the forwarding that
/// gives a remote host that port.
#[derive(Default)]
struct OnHost {
    port: Option<u16>,
    lock: Option<PathBuf>,
    forward: Option<snout_host::Forward>,
}

/// A running server. Dropping it stops it and removes its lock file.
pub struct Server {
    port: u16,
    dir: PathBuf,
    on_host: Arc<Mutex<OnHost>>,
    stopped: bool,
    shared: Arc<Shared>,
    host: HostRef,
    /// The lock file's worker: it writes and removes the file on the host, in order.
    locks: mpsc::Sender<LockJob>,
}

impl Server {
    /// Listen on 127.0.0.1, have `host` forward a port of its own here, clear away stale
    /// lock files in `dir` on `host`, and write ours there naming that port (on a worker;
    /// [`Server::sync_lock`] waits for it).
    pub fn start(
        host: HostRef,
        dir: &Path,
        folders: &[PathBuf],
        emit: impl Fn(Event) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let token = lockfile::new_token()?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let (locks, jobs) = mpsc::channel::<LockJob>();
        std::thread::Builder::new().name("claude-ide-lock".into()).spawn(move || {
            for job in jobs {
                job();
            }
        })?;
        let on_host = Arc::new(Mutex::new(OnHost::default()));
        let (h, d, f, t, o) = (host.clone(), dir.to_owned(), folders.to_vec(), token.clone(), on_host.clone());
        let _ = locks.send(Box::new(move || {
            let forward = match h.forward_to_app(port) {
                Ok(fwd) => fwd,
                Err(e) => {
                    eprintln!("Claude Code integration: no way back from the host: {e}");
                    return;
                }
            };
            let host_port = forward.port;
            lockfile::clean_stale(&*h, &d);
            let lock = match lockfile::write(&*h, &d, host_port, &f, &t) {
                Ok(lock) => Some(lock),
                Err(e) => {
                    eprintln!("Claude Code integration: could not write the lock file: {e}");
                    None
                }
            };
            *o.lock().unwrap_or_else(|p| p.into_inner()) =
                OnHost { port: Some(host_port), lock, forward: Some(forward) };
        }));
        let shared = Arc::new(Shared {
            clients: Mutex::new(HashMap::new()),
            stop: AtomicBool::new(false),
            next: AtomicU64::new(1),
            token,
            emit: Arc::new(emit),
        });
        let s = shared.clone();
        std::thread::Builder::new().name("claude-ide-listen".into()).spawn(move || listen(listener, s))?;
        Ok(Self { port, dir: dir.to_owned(), on_host, stopped: false, shared, host, locks })
    }

    /// Wait until the lock file's worker has done what it was given; false if it is gone.
    pub fn sync_lock(&self) -> bool {
        let (tx, rx) = mpsc::channel();
        let _ = self.locks.send(Box::new(move || {
            let _ = tx.send(());
        }));
        rx.recv_timeout(Duration::from_secs(10)).is_ok()
    }

    /// The port the server listens on, on this Mac.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The port a `claude` on the host connects to (the same on this Mac), once the lock
    /// file's worker has it.
    pub fn host_port(&self) -> Option<u16> {
        self.on_host.lock().unwrap_or_else(|p| p.into_inner()).port
    }

    /// Run `f` with the port on the host, on the lock file's worker once it has it (never
    /// when it could not get one).
    pub fn when_ready(&self, f: impl FnOnce(u16) + Send + 'static) {
        let on_host = self.on_host.clone();
        let _ = self.locks.send(Box::new(move || {
            let port = on_host.lock().unwrap_or_else(|p| p.into_inner()).port;
            if let Some(port) = port {
                f(port);
            }
        }));
    }

    /// The host the lock file is on.
    pub fn host(&self) -> HostRef {
        self.host.clone()
    }

    pub fn token(&self) -> &str {
        &self.shared.token
    }

    /// The lock file, once written.
    pub fn lock_path(&self) -> Option<PathBuf> {
        self.on_host.lock().unwrap_or_else(|p| p.into_inner()).lock.clone()
    }

    /// The variables that make a `claude` started on the host connect to this server, its
    /// port there being `host_port`.
    pub fn env(host_port: u16) -> Vec<(&'static str, String)> {
        vec![("CLAUDE_CODE_SSE_PORT", host_port.to_string()), ("ENABLE_IDE_INTEGRATION", "true".into())]
    }

    /// Write the lock file again with other workspace folders.
    pub fn set_folders(&mut self, folders: &[PathBuf]) {
        if self.stopped {
            return;
        }
        let (h, d, f, t, o) =
            (self.host.clone(), self.dir.clone(), folders.to_vec(), self.shared.token.clone(), self.on_host.clone());
        let _ = self.locks.send(Box::new(move || {
            let port = o.lock().unwrap_or_else(|p| p.into_inner()).port;
            if let Some(port) = port {
                let _ = lockfile::write(&*h, &d, port, &f, &t);
            }
        }));
    }

    /// The clients connected now.
    pub fn clients(&self) -> usize {
        self.shared.clients.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// Send a notification to every client.
    pub fn notify(&self, method: &str, params: Value) {
        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string();
        if let Ok(clients) = self.shared.clients.lock() {
            for tx in clients.values() {
                let _ = tx.send(msg.clone());
            }
        }
    }

    /// Stop listening, close the connections and remove the lock file (on the worker; the
    /// receiver hears when it is gone).
    pub fn stop(&mut self) -> mpsc::Receiver<()> {
        self.shared.stop.store(true, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        if self.stopped {
            let _ = tx.send(());
            return rx;
        }
        self.stopped = true;
        let (host, on_host) = (self.host.clone(), self.on_host.clone());
        let _ = self.locks.send(Box::new(move || {
            let gone = std::mem::take(&mut *on_host.lock().unwrap_or_else(|p| p.into_inner()));
            if let Some(lock) = gone.lock {
                let _ = host.remove_file(&lock);
            }
            drop(gone.forward);
            let _ = tx.send(());
        }));
        rx
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn listen(listener: TcpListener, shared: Arc<Shared>) {
    while !shared.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let s = shared.clone();
                let _ = std::thread::Builder::new().name("claude-ide-client".into()).spawn(move || serve(stream, s));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Equal strings, compared in time that does not depend on where they differ.
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn serve(stream: TcpStream, shared: Arc<Shared>) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_nodelay(true);
    // A client that never finishes its handshake does not hold a thread for long.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let token = shared.token.clone();
    // tungstenite's own types: the refusal is as large as it is.
    #[allow(clippy::result_large_err)]
    let check = move |req: &Request, mut resp: Response| -> Result<Response, ErrorResponse> {
        let given = req.headers().get(AUTH_HEADER).and_then(|v| v.to_str().ok());
        if given.is_some_and(|g| same_token(g, &token)) {
            // Claude Code asks for a subprotocol (`mcp`) and drops a connection that
            // does not name it back.
            let asked = req.headers().get("sec-websocket-protocol").and_then(|v| v.to_str().ok());
            if let Some(first) = asked.and_then(|p| p.split(',').next()).map(str::trim).filter(|p| !p.is_empty())
                && let Ok(value) = first.parse()
            {
                resp.headers_mut().insert("sec-websocket-protocol", value);
            }
            Ok(resp)
        } else {
            let mut refuse = ErrorResponse::new(Some("Unauthorized".into()));
            *refuse.status_mut() = tungstenite::http::StatusCode::UNAUTHORIZED;
            Err(refuse)
        }
    };
    let Ok(mut ws) = tungstenite::accept_hdr(stream, check) else { return };
    let _ = ws.get_ref().set_read_timeout(Some(POLL));
    let id = shared.next.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel::<String>();
    if let Ok(mut clients) = shared.clients.lock() {
        clients.insert(id, tx.clone());
    }
    (shared.emit)(Event::Connected(id));
    'conn: loop {
        if shared.stop.load(Ordering::SeqCst) {
            let _ = ws.close(None);
            let _ = ws.flush();
            break;
        }
        match ws.read() {
            Ok(Message::Text(text)) => handle(text.as_str(), id, &tx, &shared.emit),
            Ok(Message::Binary(bytes)) => {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    handle(text, id, &tx, &shared.emit);
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
        while let Ok(out) = rx.try_recv() {
            if ws.send(Message::text(out)).is_err() {
                break 'conn;
            }
        }
        // Answers to pings go out here.
        match ws.flush() {
            Ok(()) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    if let Ok(mut clients) = shared.clients.lock() {
        clients.remove(&id);
    }
    (shared.emit)(Event::Disconnected(id));
}

/// One message (or a batch) from a client.
fn handle(text: &str, client: u64, out: &mpsc::Sender<String>, emit: &Emit) {
    let Ok(msg) = serde_json::from_str::<Value>(text) else {
        let err = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } });
        let _ = out.send(err.to_string());
        return;
    };
    match msg {
        Value::Array(batch) => batch.into_iter().for_each(|m| handle_one(m, client, out, emit)),
        m => handle_one(m, client, out, emit),
    }
}

fn handle_one(msg: Value, client: u64, out: &mpsc::Sender<String>, emit: &Emit) {
    let Some(method) = msg.get("method").and_then(Value::as_str) else { return };
    // Notifications (no id) need no answer; none asks anything of the IDE yet.
    let Some(id) = msg.get("id").cloned() else { return };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let reply = Reply { id, out: out.clone(), sent: false };
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
            let version = if KNOWN_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSION };
            reply.ok(json!({
                "protocolVersion": version,
                "capabilities": {
                    "logging": {},
                    "prompts": { "listChanged": true },
                    "tools": { "listChanged": true },
                },
                "serverInfo": { "name": "snout-code", "version": env!("CARGO_PKG_VERSION") },
            }));
        }
        "ping" => reply.ok(json!({})),
        "tools/list" => reply.ok(tools::list()),
        "prompts/list" => reply.ok(json!({ "prompts": [] })),
        "resources/list" => reply.ok(json!({ "resources": [] })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
            if !tools::NAMES.contains(&name.as_str()) {
                return reply.error(tools::METHOD_NOT_FOUND, &format!("Tool not found: {name}"), None);
            }
            let arguments = params.get("arguments").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
            emit(Event::Call(Call { client, name, arguments, reply }));
        }
        _ => reply.error(tools::METHOD_NOT_FOUND, &format!("Method not found: {method}"), None),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::net::TcpStream;
    use tungstenite::client::IntoClientRequest as _;

    pub type Socket = tungstenite::WebSocket<TcpStream>;

    /// Connect to `port` presenting `token` (none: no header).
    pub fn connect(port: u16, token: Option<&str>) -> Result<Socket, tungstenite::Error> {
        let mut req = format!("ws://127.0.0.1:{port}").into_client_request()?;
        if let Some(token) = token {
            req.headers_mut().insert(AUTH_HEADER, token.parse().unwrap());
        }
        let stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        tungstenite::client(req, stream).map(|(ws, _)| ws).map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => e,
            tungstenite::HandshakeError::Interrupted(_) => tungstenite::Error::ConnectionClosed,
        })
    }

    /// Send a request and read until its answer, keeping notifications aside.
    pub fn request(ws: &mut Socket, id: u64, method: &str, params: Value, notes: &mut Vec<Value>) -> Value {
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        ws.send(Message::text(msg.to_string())).unwrap();
        loop {
            let Message::Text(t) = ws.read().unwrap() else { continue };
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v.get("id") == Some(&json!(id)) {
                return v;
            }
            notes.push(v);
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snout-ide-server-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn only_a_client_with_the_token_gets_in_and_it_can_list_the_tools() {
        let dir = scratch("auth");
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let mut server = Server::start(snout_host::local(), &dir, std::slice::from_ref(&dir), move |e| {
            let e = match e {
                Event::Connected(id) => format!("connected {id}"),
                Event::Disconnected(id) => format!("disconnected {id}"),
                Event::Call(call) => {
                    let name = call.name.clone();
                    call.reply.ok(tools::text(format!("called {name}")));
                    format!("call {name}")
                }
            };
            let _ = tx.lock().unwrap().send(e);
        })
        .unwrap();
        assert!(server.sync_lock());
        let lock: Value = serde_json::from_str(&std::fs::read_to_string(server.lock_path().unwrap()).unwrap()).unwrap();
        assert_eq!(lock["authToken"], server.token());
        assert_eq!(server.host_port(), Some(server.port()), "on this Mac, the port itself");
        assert_eq!(server.lock_path().unwrap(), lockfile::path(&dir, server.port()));

        assert!(connect(server.port(), None).is_err(), "no header, no entry");
        assert!(connect(server.port(), Some("0123456789abcdef0123456789abcdef")).is_err(), "a wrong token neither");
        let token = server.token().to_owned();
        let mut ws = connect(server.port(), Some(&token)).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().starts_with("connected"));
        let mut notes = Vec::new();
        let init = request(&mut ws, 1, "initialize", json!({ "protocolVersion": "2025-03-26" }), &mut notes);
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(init["result"]["capabilities"]["tools"].is_object());
        let list = request(&mut ws, 2, "tools/list", json!({}), &mut notes);
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 10);
        let call = request(&mut ws, 3, "tools/call", json!({ "name": "getOpenEditors", "arguments": {} }), &mut notes);
        assert_eq!(call["result"]["content"][0]["text"], "called getOpenEditors");
        let unknown = request(&mut ws, 4, "tools/call", json!({ "name": "nope" }), &mut notes);
        assert_eq!(unknown["error"]["code"], tools::METHOD_NOT_FOUND);
        let other = request(&mut ws, 5, "what/ever", json!({}), &mut notes);
        assert_eq!(other["error"]["code"], tools::METHOD_NOT_FOUND);

        server.notify("selection_changed", json!({ "text": "x" }));
        let Message::Text(t) = ws.read().unwrap() else { panic!("a text frame") };
        let note: Value = serde_json::from_str(t.as_str()).unwrap();
        assert_eq!(note["method"], "selection_changed");
        assert!(note.get("id").is_none());

        // Claude Code asks for the `mcp` subprotocol and needs it named back.
        let mut req = format!("ws://127.0.0.1:{}", server.port()).into_client_request().unwrap();
        req.headers_mut().insert(AUTH_HEADER, token.parse().unwrap());
        req.headers_mut().insert("sec-websocket-protocol", "mcp".parse().unwrap());
        let stream = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
        let (_, resp) = tungstenite::client(req, stream).expect("the subprotocol is accepted");
        assert_eq!(resp.headers().get("sec-websocket-protocol").unwrap(), "mcp");

        let lock_path = server.lock_path().unwrap();
        server.stop().recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!lock_path.exists(), "the lock file goes with the server");
        let _ = std::fs::remove_dir_all(dir);
    }
}
