//! A Debug Adapter Protocol client over TCP: java-debug (inside jdtls) listens on a local
//! port the engine hands out, and speaks DAP there.
//!
//! The framing is LSP's (`Content-Length`, a blank line, the JSON body). Requests return
//! futures any executor can await, resolving with the response's `body`; events arrive on a
//! channel. The adapter's own requests (`runInTerminal` and the like) are answered with a
//! failure here: SnoutCode launches the debuggee itself.

use std::collections::HashMap;
use std::future::Future;
use std::io::{BufReader, BufWriter};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use futures_channel::oneshot;
use serde_json::{Value, json};

use crate::rpc;

/// What the adapter sends without being asked.
#[derive(Clone, Debug, PartialEq)]
pub enum DapMessage {
    /// An event: `stopped`, `continued`, `output`, `terminated`, `hotcodereplace`…
    Event { event: String, body: Value },
    /// The connection closed.
    Closed,
}

/// A request the adapter refused, or a connection that went away.
#[derive(Clone, Debug, PartialEq)]
pub struct DapError {
    pub message: String,
}

impl std::fmt::Display for DapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DapError {}

impl DapError {
    fn gone() -> Self {
        Self { message: "the debugger is not connected".into() }
    }
}

type Pending = Mutex<HashMap<i64, oneshot::Sender<Result<Value, DapError>>>>;

struct Inner {
    out: Mutex<Option<mpsc::Sender<Value>>>,
    seq: AtomicI64,
    pending: Pending,
    open: AtomicBool,
    stream: TcpStream,
}

impl Inner {
    fn close(&self) {
        self.open.store(false, Ordering::SeqCst);
        self.out.lock().unwrap().take();
        let _ = self.stream.shutdown(Shutdown::Both);
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(Err(DapError::gone()));
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// A connection to a debug adapter. Cheap to clone; it closes when the last clone goes.
#[derive(Clone)]
pub struct Dap {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Dap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dap").field("open", &self.is_open()).finish()
    }
}

impl Dap {
    /// Connect to the adapter at `host:port`, waiting up to `timeout` for it to accept.
    pub fn connect(
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> std::io::Result<(Dap, async_channel::Receiver<DapMessage>)> {
        let addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::other(format!("no address for {host}")))?;
        let stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_nodelay(true)?;
        Self::over(stream)
    }

    /// Speak DAP over a connected stream.
    pub fn over(stream: TcpStream) -> std::io::Result<(Dap, async_channel::Receiver<DapMessage>)> {
        let reader = stream.try_clone()?;
        let writer = stream.try_clone()?;
        let (events_tx, events) = async_channel::unbounded();
        let (out_tx, out_rx) = mpsc::channel::<Value>();
        let inner = Arc::new(Inner {
            out: Mutex::new(Some(out_tx)),
            seq: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            open: AtomicBool::new(true),
            stream,
        });

        std::thread::Builder::new().name("dap-writer".into()).spawn(move || {
            let mut writer = BufWriter::new(writer);
            while let Ok(message) = out_rx.recv() {
                if rpc::write_message(&mut writer, &message).is_err() {
                    break;
                }
            }
        })?;

        let weak = Arc::downgrade(&inner);
        std::thread::Builder::new().name("dap-reader".into()).spawn(move || {
            let mut reader = BufReader::new(reader);
            loop {
                let message = match rpc::read_message(&mut reader) {
                    Ok(Some(m)) => m,
                    Ok(None) | Err(_) => break,
                };
                let Some(inner) = weak.upgrade() else { break };
                dispatch(&inner, message, &events_tx);
            }
            if let Some(inner) = weak.upgrade() {
                inner.close();
            }
            let _ = events_tx.send_blocking(DapMessage::Closed);
        })?;

        Ok((Dap { inner }, events))
    }

    pub fn is_open(&self) -> bool {
        self.inner.open.load(Ordering::SeqCst)
    }

    fn send(&self, message: Value) -> bool {
        match self.inner.out.lock().unwrap().as_ref() {
            Some(tx) => tx.send(message).is_ok(),
            None => false,
        }
    }

    /// Send `command`; the future resolves with the response's body (null when it has
    /// none), or the adapter's message when it refuses.
    pub fn request(
        &self,
        command: &str,
        arguments: Value,
    ) -> impl Future<Output = Result<Value, DapError>> + Send + 'static + use<> {
        let seq = self.inner.seq.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().unwrap().insert(seq, tx);
        let sent = self.send(json!({ "seq": seq, "type": "request", "command": command, "arguments": arguments }));
        if !sent {
            self.inner.pending.lock().unwrap().remove(&seq);
        }
        async move {
            if !sent {
                return Err(DapError::gone());
            }
            rx.await.unwrap_or_else(|_| Err(DapError::gone()))
        }
    }

    /// Close the connection; pending requests fail.
    pub fn close(&self) {
        self.inner.close();
    }
}

fn dispatch(inner: &Inner, message: Value, events: &async_channel::Sender<DapMessage>) {
    match message.get("type").and_then(Value::as_str) {
        Some("response") => {
            let Some(seq) = message.get("request_seq").and_then(Value::as_i64) else { return };
            let Some(tx) = inner.pending.lock().unwrap().remove(&seq) else { return };
            let ok = message.get("success").and_then(Value::as_bool).unwrap_or(false);
            let result = if ok {
                Ok(message.get("body").cloned().unwrap_or(Value::Null))
            } else {
                let text = message
                    .get("body")
                    .and_then(|b| b.get("error"))
                    .and_then(|e| e.get("format"))
                    .and_then(Value::as_str)
                    .or_else(|| message.get("message").and_then(Value::as_str))
                    .unwrap_or("the request failed");
                Err(DapError { message: text.to_owned() })
            };
            let _ = tx.send(result);
        }
        Some("event") => {
            let event = message.get("event").and_then(Value::as_str).unwrap_or_default().to_owned();
            let body = message.get("body").cloned().unwrap_or(Value::Null);
            let _ = events.send_blocking(DapMessage::Event { event, body });
        }
        Some("request") => {
            let seq = inner.seq.fetch_add(1, Ordering::SeqCst);
            let answer = json!({
                "seq": seq, "type": "response", "request_seq": message.get("seq").cloned().unwrap_or(Value::Null),
                "command": message.get("command").cloned().unwrap_or(Value::Null),
                "success": false, "message": "not supported by this client"
            });
            if let Some(tx) = inner.out.lock().unwrap().as_ref() {
                let _ = tx.send(answer);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_on;
    use std::net::TcpListener;

    /// A tiny adapter: answers `initialize` with capabilities, refuses `evaluate`, sends an
    /// `initialized` event after `attach`, and asks the client to run something in a
    /// terminal (which the client refuses).
    fn fake_adapter() -> (u16, std::thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            let mut seen = Vec::new();
            let mut seq = 100;
            while let Ok(Some(m)) = rpc::read_message(&mut reader) {
                seen.push(m.clone());
                if m["type"] == "response" {
                    continue;
                }
                let command = m["command"].as_str().unwrap_or_default().to_owned();
                seq += 1;
                let reply = match command.as_str() {
                    "initialize" => {
                        json!({ "seq": seq, "type": "response", "request_seq": m["seq"], "command": command,
                        "success": true, "body": { "supportsConfigurationDoneRequest": true } })
                    }
                    "evaluate" => json!({ "seq": seq, "type": "response", "request_seq": m["seq"], "command": command,
                        "success": false, "message": "cannot evaluate", "body": { "error": { "id": 1, "format": "x cannot be resolved" } } }),
                    _ => {
                        json!({ "seq": seq, "type": "response", "request_seq": m["seq"], "command": command, "success": true })
                    }
                };
                rpc::write_message(&mut writer, &reply).unwrap();
                if command == "attach" {
                    let ev = json!({ "seq": seq + 1, "type": "event", "event": "initialized" });
                    rpc::write_message(&mut writer, &ev).unwrap();
                    let ask = json!({ "seq": seq + 2, "type": "request", "command": "runInTerminal", "arguments": {} });
                    rpc::write_message(&mut writer, &ask).unwrap();
                    seq += 2;
                }
                if command == "disconnect" {
                    break;
                }
            }
            seen
        });
        (port, handle)
    }

    #[test]
    fn requests_resolve_events_arrive_and_reverse_requests_are_refused() {
        let (port, adapter) = fake_adapter();
        let (dap, events) = Dap::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        let caps = block_on(dap.request("initialize", json!({ "adapterID": "java" }))).unwrap();
        assert_eq!(caps["supportsConfigurationDoneRequest"], true);
        assert_eq!(block_on(dap.request("attach", json!({ "port": 5005 }))).unwrap(), Value::Null);
        let ev = block_on(events.recv()).unwrap();
        assert_eq!(ev, DapMessage::Event { event: "initialized".into(), body: Value::Null });
        let err = block_on(dap.request("evaluate", json!({ "expression": "x" }))).unwrap_err();
        assert_eq!(err.message, "x cannot be resolved");
        let _ = block_on(dap.request("disconnect", json!({})));
        let seen = adapter.join().unwrap();
        let refused = seen.iter().find(|m| m["type"] == "response").expect("the client answered runInTerminal");
        assert_eq!(refused["command"], "runInTerminal");
        assert_eq!(refused["success"], false);
        loop {
            match block_on(events.recv()) {
                Ok(DapMessage::Closed) | Err(_) => break,
                Ok(_) => {}
            }
        }
        assert!(!dap.is_open());
        assert_eq!(block_on(dap.request("threads", json!({}))).unwrap_err(), DapError::gone());
    }
}
