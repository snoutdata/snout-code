//! TCP connections made from the server (the debugger's port), and listeners whose
//! connections go back to the client's machine (Claude Code's IDE server and hooks on the
//! Mac), carried on channels.

use snout_remote_proto::{Channel, Mux};
use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn connect(host: &str, port: u16, channel: Channel) -> io::Result<()> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, format!("{host} has no address"));
    let mut stream = None;
    for addr in (host, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(10)) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last = e,
        }
    }
    snout_remote_proto::tcp::carry(stream.ok_or(last)?, channel)
}

/// The server's listeners on its loopback, by port. They outlive connections.
#[derive(Default)]
pub struct Listeners(Mutex<HashMap<u16, Arc<TcpListener>>>);

impl Listeners {
    /// Listen on `port` (a free one for 0), or keep listening there.
    pub fn listen(&self, port: u16) -> io::Result<u16> {
        let mut all = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if port != 0 && all.contains_key(&port) {
            return Ok(port);
        }
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        all.insert(port, Arc::new(listener));
        Ok(port)
    }

    pub fn unlisten(&self, port: u16) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).remove(&port);
    }

    fn get(&self, port: u16) -> Option<Arc<TcpListener>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).get(&port).cloned()
    }

    /// Wait for the next connection on `port` and carry it on `channel`. Gives up when the
    /// listener stops, the channel closes or the connection ends.
    pub fn accept(&self, mux: &Mux, port: u16, channel: Channel) -> io::Result<()> {
        let Some(listener) = self.get(port) else {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("nothing listens on {port}")));
        };
        loop {
            if channel.is_closed() || !mux.is_alive() {
                return Err(io::Error::new(io::ErrorKind::NotConnected, "the client went"));
            }
            if !self.get(port).is_some_and(|l| Arc::ptr_eq(&l, &listener)) {
                return Err(io::Error::new(io::ErrorKind::NotFound, format!("{port} stopped listening")));
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    return snout_remote_proto::tcp::carry(stream, channel);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => return Err(e),
            }
        }
    }
}
