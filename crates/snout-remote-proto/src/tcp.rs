//! A TCP connection carried on a channel, both ways: the server's end of a tunnel
//! ([`crate::Request::ConnectTcp`], [`crate::Request::Accept`]) and the client's end of a
//! forwarded port.

use crate::Channel;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};

/// Carry `stream`'s bytes on `channel` and the channel's into `stream`, on two threads,
/// until both directions end.
pub fn carry(stream: TcpStream, channel: Channel) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    let (mut from_socket, mut to_socket) = (stream.try_clone()?, stream);
    let up = channel.clone();
    std::thread::Builder::new().name("snout-remote-tcp-up".into()).spawn(move || {
        let mut buf = vec![0u8; crate::frame::CHUNK];
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
    })?;
    std::thread::Builder::new().name("snout-remote-tcp-down".into()).spawn(move || {
        let mut buf = vec![0u8; crate::frame::CHUNK];
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
    })?;
    Ok(())
}
