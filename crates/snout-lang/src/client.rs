//! A language server as a process on a [`Host`], spoken to over its stdin and stdout.
//!
//! Requests return futures any executor can await; what the server sends on its own
//! (notifications, its requests, its stderr lines, and its exit) arrives on a channel. A
//! few server requests every client answers the same way are answered here; the rest are
//! the caller's to answer with [`Client::respond`].

use std::collections::HashMap;
use std::future::Future;
use std::io::{BufRead, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use futures_channel::oneshot;
use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde_json::{Value, json};
use snout_host::{Cmd, Host, Process, Stdio};

use crate::rpc::{self, ResponseError};

/// What the server sends without being asked, and what happens to it.
#[derive(Clone, Debug)]
pub enum ServerMessage {
    Notification {
        method: String,
        params: Value,
    },
    /// A request the caller answers with [`Client::respond`].
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A line the server wrote on stderr.
    Stderr(String),
    /// The process ended (its exit code, if it had one).
    Exited(Option<i32>),
}

type Pending = Mutex<HashMap<i64, oneshot::Sender<Result<Value, ResponseError>>>>;

struct Inner {
    out: Mutex<Option<mpsc::Sender<Value>>>,
    next_id: AtomicI64,
    pending: Pending,
    child: Mutex<Box<dyn Process>>,
    alive: AtomicBool,
    pid: u32,
}

impl Inner {
    fn fail_pending(&self) {
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(Err(ResponseError::gone()));
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if self.alive.load(Ordering::SeqCst) {
            let _ = self.child.lock().map(|mut c| c.terminate());
        }
    }
}

/// A running language server. Cheap to clone; the process stops when the last clone goes
/// (politely: SIGTERM, so a launcher's shutdown hook can stop the server it started).
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").field("pid", &self.inner.pid).finish()
    }
}

impl Client {
    /// Start `command` on `host` and speak LSP to it. Its stdin, stdout and stderr are taken over.
    pub fn spawn(host: &dyn Host, command: &Cmd) -> std::io::Result<(Client, async_channel::Receiver<ServerMessage>)> {
        let command = command.clone().stdin(Stdio::Piped).stdout(Stdio::Piped).stderr(Stdio::Piped);
        Self::over(host.spawn(&command)?)
    }

    /// Speak LSP to a process started with its stdin, stdout and stderr piped.
    pub fn over(mut child: Box<dyn Process>) -> std::io::Result<(Client, async_channel::Receiver<ServerMessage>)> {
        let missing = || std::io::Error::other("the server's stdio is not piped");
        let stdin = child.take_stdin().ok_or_else(missing)?;
        let stdout = child.take_stdout().ok_or_else(missing)?;
        let stderr = child.take_stderr().ok_or_else(missing)?;
        let pid = child.pid().unwrap_or(0);
        let (events_tx, events) = async_channel::unbounded();
        let (out_tx, out_rx) = mpsc::channel::<Value>();
        let inner = Arc::new(Inner {
            out: Mutex::new(Some(out_tx)),
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            child: Mutex::new(child),
            alive: AtomicBool::new(true),
            pid,
        });

        // Writer: one thread owns stdin, so messages never interleave.
        std::thread::Builder::new().name("lsp-writer".into()).spawn(move || {
            let mut stdin = BufWriter::new(stdin);
            while let Ok(message) = out_rx.recv() {
                if rpc::write_message(&mut stdin, &message).is_err() {
                    break;
                }
            }
        })?;

        // Stderr, line by line.
        let tx = events_tx.clone();
        std::thread::Builder::new().name("lsp-stderr".into()).spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if tx.send_blocking(ServerMessage::Stderr(line)).is_err() {
                    break;
                }
            }
        })?;

        // Reader: responses complete their futures; the rest goes to the channel. The
        // inner state is held weakly, so dropping the last client stops the server.
        let weak = Arc::downgrade(&inner);
        std::thread::Builder::new().name("lsp-reader".into()).spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let message = match rpc::read_message(&mut stdout) {
                    Ok(Some(m)) => m,
                    Ok(None) | Err(_) => break,
                };
                let Some(inner) = weak.upgrade() else { break };
                dispatch(&inner, message, &events_tx);
            }
            let code = weak.upgrade().and_then(|inner| {
                inner.alive.store(false, Ordering::SeqCst);
                inner.fail_pending();
                inner.out.lock().unwrap().take();
                let status = inner.child.lock().unwrap().wait().ok();
                status.and_then(|s| s.code)
            });
            let _ = events_tx.send_blocking(ServerMessage::Exited(code));
        })?;

        Ok((Client { inner }, events))
    }

    /// The server process's id.
    pub fn pid(&self) -> u32 {
        self.inner.pid
    }

    pub fn is_alive(&self) -> bool {
        self.inner.alive.load(Ordering::SeqCst)
    }

    fn send(&self, message: Value) -> bool {
        match self.inner.out.lock().unwrap().as_ref() {
            Some(tx) => tx.send(message).is_ok(),
            None => false,
        }
    }

    /// Send a request by method name; the future resolves with the result.
    pub fn request_raw(
        &self,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value, ResponseError>> + Send + 'static + use<> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().unwrap().insert(id, tx);
        let sent = self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        if !sent {
            self.inner.pending.lock().unwrap().remove(&id);
        }
        async move {
            if !sent {
                return Err(ResponseError::gone());
            }
            rx.await.unwrap_or_else(|_| Err(ResponseError::gone()))
        }
    }

    /// Send a typed request.
    pub fn request<R: Request>(
        &self,
        params: R::Params,
    ) -> impl Future<Output = Result<R::Result, ResponseError>> + Send + 'static + use<R> {
        let params = serde_json::to_value(params).unwrap_or(Value::Null);
        let response = self.request_raw(R::METHOD, params);
        async move {
            let value = response.await?;
            serde_json::from_value(value)
                .map_err(|e| ResponseError { code: -32700, message: format!("{}: {e}", R::METHOD) })
        }
    }

    /// Send a notification by method name.
    pub fn notify_raw(&self, method: &str, params: Value) -> bool {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    /// Send a typed notification.
    pub fn notify<N: Notification>(&self, params: N::Params) -> bool {
        self.notify_raw(N::METHOD, serde_json::to_value(params).unwrap_or(Value::Null))
    }

    /// Answer a request the server made.
    pub fn respond(&self, id: Value, result: Result<Value, ResponseError>) {
        let message = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
        };
        self.send(message);
    }

    /// Stop the server the polite way, `shutdown` then `exit`, giving it `grace` to end on
    /// its own before it is told to (SIGTERM) and then made to (SIGKILL). Blocks.
    pub fn shutdown_blocking(&self, grace: Duration) {
        if self.is_alive() {
            let (tx, rx) = mpsc::channel();
            let response = self.request_raw("shutdown", Value::Null);
            std::thread::spawn(move || {
                let _ = tx.send(block_on(response));
            });
            let _ = rx.recv_timeout(grace);
            self.notify_raw("exit", Value::Null);
        }
        let deadline = Instant::now() + grace;
        while self.is_alive() && Instant::now() < deadline {
            if let Ok(Some(_)) = self.inner.child.lock().unwrap().try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.kill(Duration::from_secs(3));
    }

    /// End the process now: SIGTERM, then SIGKILL after `grace`.
    pub fn kill(&self, grace: Duration) {
        let exited = |c: &Client| matches!(c.inner.child.lock().unwrap().try_wait(), Ok(Some(_)));
        if exited(self) {
            return;
        }
        let _ = self.inner.child.lock().unwrap().terminate();
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if exited(self) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.inner.child.lock().unwrap().kill();
    }
}

/// Answers for the requests every client answers alike; `None` for the rest.
fn standard_answer(method: &str, params: &Value) -> Option<Value> {
    match method {
        // Settings come with `initialize`; each item asked for gets null.
        "workspace/configuration" => {
            let n = params.get("items").and_then(Value::as_array).map_or(0, Vec::len);
            Some(Value::Array(vec![Value::Null; n]))
        }
        "window/workDoneProgress/create" | "client/registerCapability" | "client/unregisterCapability" => {
            Some(Value::Null)
        }
        _ => None,
    }
}

fn dispatch(inner: &Inner, message: Value, events: &async_channel::Sender<ServerMessage>) {
    let method = message.get("method").and_then(Value::as_str).map(str::to_owned);
    let id = message.get("id").cloned();
    match (method, id) {
        (None, Some(id)) => {
            let Some(id) = id.as_i64() else { return };
            let Some(tx) = inner.pending.lock().unwrap().remove(&id) else { return };
            let result = match message.get("error") {
                Some(e) => Err(ResponseError::of(e)),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            };
            let _ = tx.send(result);
        }
        (Some(method), Some(id)) => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            if let Some(answer) = standard_answer(&method, &params) {
                if let Some(tx) = inner.out.lock().unwrap().as_ref() {
                    let _ = tx.send(json!({ "jsonrpc": "2.0", "id": id, "result": answer }));
                }
                // Progress tokens still interest the caller.
                if method != "window/workDoneProgress/create" {
                    return;
                }
            }
            let _ = events.send_blocking(ServerMessage::Request { id, method, params });
        }
        (Some(method), None) => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let _ = events.send_blocking(ServerMessage::Notification { method, params });
        }
        (None, None) => {}
    }
}

/// Wait for a future on this thread (for the few blocking calls: shutdown, tests).
pub fn block_on<F: Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::park();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// A tiny "server" in shell: echoes each request's params back as its result, sends
    /// one notification and one request first, and writes a line on stderr.
    fn echo_server() -> Cmd {
        let script = r#"
            printf 'starting\n' >&2
            note='{"jsonrpc":"2.0","method":"hello","params":{"n":1}}'
            printf 'Content-Length: %d\r\n\r\n%s' ${#note} "$note"
            ask='{"jsonrpc":"2.0","id":"s1","method":"workspace/configuration","params":{"items":[{},{}]}}'
            printf 'Content-Length: %d\r\n\r\n%s' ${#ask} "$ask"
            while IFS= read -r line; do
                case "$line" in
                    Content-Length:*) len=${line#Content-Length: }; len=${len%$'\r'} ;;
                    $'\r'|'')
                        body=$(dd bs=1 count=$len 2>/dev/null)
                        case "$body" in
                            *'"method":"exit"'*) exit 0 ;;
                            *'"id":"s1"'*) printf 'Content-Length: %d\r\n\r\n%s' ${#body} "$body" >&2 ;;
                            *'"id":'*)
                                id=$(printf '%s' "$body" | sed 's/.*"id":\([0-9]*\).*/\1/')
                                params=$(printf '%s' "$body" | sed 's/.*"params":\(.*\)}$/\1/')
                                out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":$params}"
                                printf 'Content-Length: %d\r\n\r\n%s' ${#out} "$out" ;;
                        esac ;;
                esac
            done
        "#;
        Cmd::new("bash").arg("-c").arg(script)
    }

    #[test]
    fn requests_resolve_and_server_messages_arrive_in_order() {
        let (client, events) = echo_server_client();
        let result = block_on(client.request_raw("echo", json!({"a": [1, 2]}))).unwrap();
        assert_eq!(result, json!({"a": [1, 2]}));
        let mut seen = Vec::new();
        while let Ok(m) = events.try_recv() {
            seen.push(m);
        }
        assert!(
            seen.iter().any(|m| matches!(m, ServerMessage::Notification { method, .. } if method == "hello")),
            "{seen:?}"
        );
        assert!(seen.iter().any(|m| matches!(m, ServerMessage::Stderr(l) if l == "starting")), "{seen:?}");
        assert!(
            !seen.iter().any(|m| matches!(m, ServerMessage::Request { .. })),
            "workspace/configuration is answered here"
        );
        client.shutdown_blocking(Duration::from_secs(2));
        assert!(!client.is_alive());
        let gone = block_on(client.request_raw("echo", json!(1)));
        assert_eq!(gone.unwrap_err().code, ResponseError::SERVER_GONE);
    }

    fn echo_server_client() -> (Client, async_channel::Receiver<ServerMessage>) {
        Client::spawn(&*snout_host::local(), &echo_server()).expect("bash runs")
    }

    #[test]
    fn a_dead_server_fails_pending_requests_and_says_it_exited() {
        let c = Cmd::new("sh").arg("-c").arg("read x; exit 3");
        let (client, events) = Client::spawn(&*snout_host::local(), &c).unwrap();
        let pending = client.request_raw("never", Value::Null);
        assert_eq!(block_on(pending).unwrap_err().code, ResponseError::SERVER_GONE);
        let exit = loop {
            match block_on(events.recv()) {
                Ok(ServerMessage::Exited(code)) => break code,
                Ok(_) => continue,
                Err(_) => panic!("no exit message"),
            }
        };
        assert_eq!(exit, Some(3));
    }
}
