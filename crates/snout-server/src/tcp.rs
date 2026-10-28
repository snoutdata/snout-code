//! TCP connections made from the server (the debugger's port), carried on a channel.

use snout_remote_proto::Channel;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
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
    let stream = stream.ok_or(last)?;
    stream.set_nodelay(true)?;
    let (mut from_socket, mut to_socket) = (stream.try_clone()?, stream);
    let up = channel.clone();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; snout_remote_proto::frame::CHUNK];
        loop {
            match from_socket.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = up.send_eof();
                    break;
                }
                Ok(n) => {
                    if up.send(&buf[..n]).is_err() {
                        let _ = from_socket.shutdown(Shutdown::Both);
                        break;
                    }
                }
            }
        }
    });
    std::thread::spawn(move || {
        let mut buf = vec![0u8; snout_remote_proto::frame::CHUNK];
        loop {
            match channel.recv(&mut buf) {
                Ok(0) => {
                    let _ = to_socket.shutdown(Shutdown::Write);
                    break;
                }
                Ok(n) => {
                    if to_socket.write_all(&buf[..n]).is_err() {
                        channel.close();
                        break;
                    }
                }
                Err(_) => {
                    let _ = to_socket.shutdown(Shutdown::Both);
                    break;
                }
            }
        }
    });
    Ok(())
}
