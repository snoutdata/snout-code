//! Messages: a JSON header and a binary blob, encoded as `u32` header length (big endian),
//! header, blob.

use crate::frame::{CHUNK, FIN, Frame, Kind};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    /// JSON.
    pub header: Vec<u8>,
    pub blob: Vec<u8>,
}

impl Message {
    pub fn new(header: &impl Serialize, blob: Vec<u8>) -> Message {
        Message { header: serde_json::to_vec(header).expect("headers serialize"), blob }
    }

    pub fn parse<T: DeserializeOwned>(&self) -> io::Result<T> {
        serde_json::from_slice(&self.header).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.header.len() + self.blob.len());
        out.extend((self.header.len() as u32).to_be_bytes());
        out.extend(&self.header);
        out.extend(&self.blob);
        out
    }

    pub fn decode(mut bytes: Vec<u8>) -> io::Result<Message> {
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "malformed message");
        let len = u32::from_be_bytes(bytes.get(..4).ok_or_else(bad)?.try_into().unwrap()) as usize;
        if bytes.len() < 4 + len {
            return Err(bad());
        }
        let blob = bytes.split_off(4 + len);
        bytes.drain(..4);
        Ok(Message { header: bytes, blob })
    }

    /// The frames that carry this message on `channel`, the last one marked [`FIN`].
    pub fn frames(&self, channel: u32, kind: Kind) -> Vec<Frame> {
        let bytes = self.encode();
        let mut frames: Vec<Frame> =
            bytes.chunks(CHUNK).map(|c| Frame { channel, kind, flags: 0, payload: c.to_vec() }).collect();
        frames.last_mut().expect("a message encodes to at least its length").flags |= FIN;
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip_through_frames() {
        for blob_len in [0, 1, CHUNK - 10, CHUNK, 3 * CHUNK + 17] {
            let m = Message::new(&serde_json::json!({"op": "write", "path": "/a/b"}), vec![7; blob_len]);
            let frames = m.frames(5, Kind::Request);
            assert!(frames.iter().all(|f| f.payload.len() <= CHUNK && f.channel == 5));
            assert!(frames.iter().rev().skip(1).all(|f| !f.fin()) && frames.last().unwrap().fin());
            let joined: Vec<u8> = frames.into_iter().flat_map(|f| f.payload).collect();
            assert_eq!(Message::decode(joined).unwrap(), m);
        }
    }

    #[test]
    fn truncated_messages_are_refused() {
        let m = Message::new(&"hello", vec![1, 2]).encode();
        assert!(Message::decode(m[..3].to_vec()).is_err());
        assert!(Message::decode(m[..6].to_vec()).is_err());
    }
}
