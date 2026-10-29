//! The wire protocol between SnoutCode (the app, on the Mac) and `snout-server` (on the
//! remote machine), carried over one byte stream: the stdin and stdout of
//! `ssh <host> snout-server --stdio`.
//!
//! # Layers
//!
//! - [`frame`]: length prefixed frames. Each frame names a channel, a kind and flags.
//! - [`message`]: a message is a JSON header plus a binary blob (file contents travel as
//!   the blob, never inside JSON). A message is cut into frames of at most
//!   [`frame::CHUNK`] bytes, so a large file read never holds the stream for long.
//! - [`mux`]: multiplexing. Requests and responses each travel on a channel of their own
//!   (the request id), and streams (process stdio, a pty, file watch events, a TCP tunnel)
//!   on channels the client opens. The writer serves the channels round robin, one frame at
//!   a time, so neither a big response nor a chatty process starves anything else. Streams
//!   have flow control: a sender may have at most [`mux::WINDOW`] bytes unread by the
//!   receiver, and the receiver grants more as its consumer reads, so a process whose
//!   output nobody reads is stopped by its pipe, not buffered without bound.
//! - [`api`]: the requests and their replies: file system and host information, file
//!   watching, processes and ptys (which outlive a connection and can be attached again),
//!   and TCP connections from the remote machine and back to the client's ([`tcp`]).
//! - [`client`]: the typed client the app uses.
//!
//! # Handshake
//!
//! Before anything else the client sends a [`Hello`] frame and the server answers with
//! its own. The server refuses a client whose [`VERSION`] or [`PROTOCOL`] differs from its
//! own: the app installs the server binary of its own version on the remote machine.

pub mod api;
pub mod client;
pub mod frame;
pub mod message;
pub mod mux;
pub mod tcp;

pub use api::*;
pub use client::{Client, Pipes, RemoteProcess, RemotePty, Watch};
pub use message::Message;
pub use mux::{Channel, Handler, Mux, Role};

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// The version of SnoutCode this protocol belongs to. App and server must agree on it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The wire protocol revision, raised on any incompatible change.
pub const PROTOCOL: u32 = 2;

/// The first frame each side sends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hello {
    /// Always `"snout-remote"`: tells a stray shell banner apart from the protocol.
    pub magic: String,
    pub protocol: u32,
    pub version: String,
    /// The server's SHA-256 of its own executable, so the app can tell a daemon still
    /// running an older build of the same version from the binary it installed.
    #[serde(default)]
    pub build: Option<String>,
    /// Set by a server that refuses the client; it then closes the stream.
    #[serde(default)]
    pub error: Option<String>,
}

pub const MAGIC: &str = "snout-remote";

impl Hello {
    pub fn new(build: Option<String>) -> Self {
        Hello { magic: MAGIC.into(), protocol: PROTOCOL, version: VERSION.into(), build, error: None }
    }

    /// Why `self` (the peer's hello) cannot talk to this side, if it cannot.
    pub fn incompatibility(&self) -> Option<String> {
        if self.magic != MAGIC {
            Some("the peer does not speak the snout remote protocol".into())
        } else if self.protocol != PROTOCOL || self.version != VERSION {
            Some(format!(
                "version mismatch: this side is {VERSION} (protocol {PROTOCOL}), the peer is {} (protocol {})",
                self.version, self.protocol
            ))
        } else {
            None
        }
    }
}

fn write_hello(w: &mut impl Write, hello: &Hello) -> io::Result<()> {
    let f = frame::Frame {
        channel: 0,
        kind: frame::Kind::Hello,
        flags: frame::FIN,
        payload: serde_json::to_vec(hello).map_err(io::Error::other)?,
    };
    frame::write_frame(w, &f)?;
    w.flush()
}

fn read_hello(r: &mut impl Read) -> io::Result<Hello> {
    let f = frame::read_frame(r)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "the connection closed before the handshake"))?;
    if f.kind != frame::Kind::Hello {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "expected a hello frame"));
    }
    serde_json::from_slice(&f.payload).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// What the server writes before its hello. The client skips everything before it, so a
/// login script that prints to stdout on the remote machine cannot break the stream.
pub const BANNER: &[u8] = b"\n\0snout-remote\0\n";

/// Read and drop bytes up to and including [`BANNER`]; give up after 64 KiB.
fn skip_to_banner(r: &mut impl Read) -> io::Result<()> {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    while !seen.ends_with(BANNER) {
        if r.read(&mut byte)? == 0 {
            let shown = String::from_utf8_lossy(&seen).trim().chars().take(400).collect::<String>();
            let msg = if shown.is_empty() {
                "the connection closed before the server answered".to_owned()
            } else {
                format!("the connection closed before the server answered: {shown}")
            };
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, msg));
        }
        seen.push(byte[0]);
        if seen.len() > 64 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "no snout-server answered on the stream"));
        }
    }
    Ok(())
}

/// The client's half of the handshake: send our hello, read the server's, and check it.
pub fn client_handshake(r: &mut impl Read, w: &mut impl Write) -> io::Result<Hello> {
    write_hello(w, &Hello::new(None))?;
    skip_to_banner(r)?;
    let server = read_hello(r)?;
    if let Some(e) = &server.error {
        return Err(io::Error::new(io::ErrorKind::ConnectionRefused, format!("the server refused: {e}")));
    }
    if let Some(e) = server.incompatibility() {
        return Err(io::Error::new(io::ErrorKind::ConnectionRefused, e));
    }
    Ok(server)
}

/// The server's half: read the client's hello and answer. A client that cannot be served
/// gets a hello with the reason in `error`, and the handshake fails.
pub fn server_handshake(r: &mut impl Read, w: &mut impl Write, build: Option<String>) -> io::Result<Hello> {
    w.write_all(BANNER)?;
    w.flush()?;
    let client = read_hello(r)?;
    let mut ours = Hello::new(build);
    if let Some(e) = client.incompatibility() {
        ours.error = Some(e.clone());
        let _ = write_hello(w, &ours);
        return Err(io::Error::new(io::ErrorKind::ConnectionRefused, e));
    }
    write_hello(w, &ours)?;
    Ok(client)
}
