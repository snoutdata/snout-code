//! Frames: `u32` length (big endian) of what follows, `u32` channel, `u8` kind, `u8` flags,
//! then the payload.

use std::io::{self, Read, Write};

/// The most payload one frame carries when a message or a stream is cut into frames.
pub const CHUNK: usize = 32 * 1024;

/// The largest frame accepted. Anything longer means the stream is out of step.
pub const MAX_PAYLOAD: usize = 1024 * 1024;

const HEADER: usize = 4 + 1 + 1;

/// The last frame of a message.
pub const FIN: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The handshake, on channel 0.
    Hello,
    /// A fragment of a request; the channel is the request id.
    Request,
    /// A fragment of the response to the request with the channel's id.
    Response,
    /// Stream bytes. Counts against the channel's window.
    Data,
    /// A fragment of a message on a stream (a batch of watch events). Counts against the
    /// channel's window.
    Message,
    /// The receiver read this many more bytes (`u32` payload): the sender may send them.
    Window,
    /// The sender will send no more on this channel; the other direction stays open.
    Eof,
    /// The channel is gone in both directions.
    Close,
}

impl Kind {
    fn code(self) -> u8 {
        match self {
            Kind::Hello => 0,
            Kind::Request => 1,
            Kind::Response => 2,
            Kind::Data => 3,
            Kind::Message => 4,
            Kind::Window => 5,
            Kind::Eof => 6,
            Kind::Close => 7,
        }
    }

    fn from_code(c: u8) -> Option<Kind> {
        Some(match c {
            0 => Kind::Hello,
            1 => Kind::Request,
            2 => Kind::Response,
            3 => Kind::Data,
            4 => Kind::Message,
            5 => Kind::Window,
            6 => Kind::Eof,
            7 => Kind::Close,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub channel: u32,
    pub kind: Kind,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(channel: u32, kind: Kind, payload: Vec<u8>) -> Frame {
        Frame { channel, kind, flags: 0, payload }
    }

    pub fn fin(&self) -> bool {
        self.flags & FIN != 0
    }

    /// The bytes on the wire.
    pub fn encoded_len(&self) -> usize {
        4 + HEADER + self.payload.len()
    }
}

pub fn write_frame(w: &mut impl Write, f: &Frame) -> io::Result<()> {
    assert!(f.payload.len() <= MAX_PAYLOAD, "frame payload too large");
    let mut head = [0u8; 4 + HEADER];
    head[..4].copy_from_slice(&((HEADER + f.payload.len()) as u32).to_be_bytes());
    head[4..8].copy_from_slice(&f.channel.to_be_bytes());
    head[8] = f.kind.code();
    head[9] = f.flags;
    w.write_all(&head)?;
    w.write_all(&f.payload)
}

/// The next frame, or `None` at a clean end of the stream (between frames).
pub fn read_frame(r: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut len = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut len[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let len = u32::from_be_bytes(len) as usize;
    if !(HEADER..=HEADER + MAX_PAYLOAD).contains(&len) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad frame length {len}")));
    }
    let mut head = [0u8; HEADER];
    r.read_exact(&mut head)?;
    let kind = Kind::from_code(head[4])
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("unknown frame kind {}", head[4])))?;
    let mut payload = vec![0u8; len - HEADER];
    r.read_exact(&mut payload)?;
    Ok(Some(Frame { channel: u32::from_be_bytes(head[..4].try_into().unwrap()), kind, flags: head[5], payload }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let frames = vec![
            Frame { channel: 0, kind: Kind::Hello, flags: FIN, payload: b"{}".to_vec() },
            Frame::new(7, Kind::Data, vec![1, 2, 3]),
            Frame::new(u32::MAX, Kind::Close, vec![]),
            Frame::new(9, Kind::Window, 65536u32.to_be_bytes().to_vec()),
            Frame::new(3, Kind::Data, vec![0xab; MAX_PAYLOAD]),
        ];
        let mut wire = Vec::new();
        for f in &frames {
            write_frame(&mut wire, f).unwrap();
        }
        assert_eq!(wire.len(), frames.iter().map(Frame::encoded_len).sum::<usize>());
        let mut r = &wire[..];
        for f in &frames {
            assert_eq!(read_frame(&mut r).unwrap().as_ref(), Some(f));
        }
        assert_eq!(read_frame(&mut r).unwrap(), None);
    }

    #[test]
    fn a_stream_cut_inside_a_frame_is_an_error() {
        let mut wire = Vec::new();
        write_frame(&mut wire, &Frame::new(1, Kind::Data, vec![5; 100])).unwrap();
        for cut in [2, 7, 50] {
            assert!(read_frame(&mut &wire[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn bad_lengths_and_kinds_are_refused() {
        let mut wire = ((HEADER + MAX_PAYLOAD + 1) as u32).to_be_bytes().to_vec();
        wire.extend([0; 16]);
        assert_eq!(read_frame(&mut &wire[..]).unwrap_err().kind(), io::ErrorKind::InvalidData);
        let mut wire = Vec::new();
        write_frame(&mut wire, &Frame::new(1, Kind::Data, vec![])).unwrap();
        wire[8] = 99;
        assert_eq!(read_frame(&mut &wire[..]).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
