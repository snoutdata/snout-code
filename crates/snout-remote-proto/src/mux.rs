//! Multiplexing: many channels over one byte stream.
//!
//! One thread reads frames and routes them; one thread writes. Requests and responses
//! travel on a channel named by the request id; streams on channels the client opens
//! (client ids are odd, server ids even, and neither is ever reused on a connection, so a
//! late frame for a closed channel is simply dropped).
//!
//! Fairness: the writer keeps a queue per channel and takes one frame from each in turn,
//! with flow control frames ahead of everything. A 50 MB file read is many frames, and a
//! request made meanwhile is answered between two of them.
//!
//! Backpressure: a stream sender holds a window of [`WINDOW`] bytes and spends it on each
//! data or message frame; the receiver gives it back (a window frame) as its consumer
//! reads. A stream whose consumer stops reading stops its producer, and nothing on the
//! connection buffers more than a window per stream.

use crate::frame::{self, CHUNK, Frame, Kind};
use crate::message::Message;
use std::collections::{HashMap, VecDeque};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, mpsc};
use std::time::Duration;

/// The bytes a stream may have in flight before its receiver reads them.
pub const WINDOW: usize = 256 * 1024;

/// The receiver gives the window back once its consumer has read this much.
const GRANT_AT: usize = WINDOW / 4;

/// The writer takes at most this many frames per turn of its lock.
const BATCH: usize = 16;

pub type ChannelId = u32;

/// Answers requests. Each request runs on a thread of its own, so a slow one (waiting for
/// a process to exit) holds up nothing else.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, mux: &Mux, request: Message) -> Message;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

/// A connection. Clones share it.
#[derive(Clone)]
pub struct Mux(Arc<Inner>);

struct Inner {
    state: Mutex<State>,
    out: Mutex<Outbox>,
    out_cv: Condvar,
    next_id: AtomicU32,
    handler: Option<Arc<dyn Handler>>,
    closer: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

#[derive(Default)]
struct State {
    chans: HashMap<ChannelId, Arc<Chan>>,
    calls: HashMap<ChannelId, mpsc::Sender<io::Result<Message>>>,
    /// Why the connection ended, once it has.
    dead: Option<String>,
    on_close: Vec<Box<dyn FnOnce() + Send>>,
}

#[derive(Default)]
struct Outbox {
    /// Window frames: tiny, and what unblocks the other side.
    urgent: VecDeque<Frame>,
    queues: HashMap<ChannelId, VecDeque<Frame>>,
    /// Channels with frames waiting, in turn order.
    ring: VecDeque<ChannelId>,
    closed: bool,
}

impl Outbox {
    fn push(&mut self, f: Frame) {
        if self.closed {
            return;
        }
        let q = self.queues.entry(f.channel).or_default();
        if q.is_empty() {
            self.ring.push_back(f.channel);
        }
        q.push_back(f);
    }

    fn pop(&mut self) -> Option<Frame> {
        if let Some(f) = self.urgent.pop_front() {
            return Some(f);
        }
        let ch = self.ring.pop_front()?;
        let q = self.queues.get_mut(&ch).expect("a queued channel has a queue");
        let f = q.pop_front().expect("a queued channel has a frame");
        if q.is_empty() {
            self.queues.remove(&ch);
        } else {
            self.ring.push_back(ch);
        }
        Some(f)
    }
}

/// One end of a channel.
struct Chan {
    st: Mutex<ChanState>,
    cv: Condvar,
}

#[derive(Default)]
struct ChanState {
    inq: VecDeque<Inbound>,
    /// Bytes the consumer read that were not yet given back to the sender.
    unacked: usize,
    /// Bytes we may still send.
    credit: usize,
    closed: Option<Closed>,
}

enum Inbound {
    Data(Vec<u8>, usize),
    Msg(Message, usize),
    Eof,
}

#[derive(Clone)]
enum Closed {
    Local,
    Remote,
    Dead(String),
}

impl Closed {
    fn error(&self) -> io::Error {
        match self {
            Closed::Local => io::Error::new(io::ErrorKind::NotConnected, "the channel is closed"),
            Closed::Remote => io::Error::new(io::ErrorKind::BrokenPipe, "the other side closed the channel"),
            Closed::Dead(why) => io::Error::new(io::ErrorKind::ConnectionAborted, why.clone()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Mux {
    /// Run a connection over `reader` and `writer` (after the handshake). `handler` answers
    /// the peer's requests (the server has one). `closer` closes the underlying stream; it
    /// runs once, when the connection ends for whatever reason.
    pub fn start(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        role: Role,
        handler: Option<Arc<dyn Handler>>,
        closer: impl FnOnce() + Send + 'static,
    ) -> Mux {
        let mux = Mux(Arc::new(Inner {
            state: Mutex::new(State::default()),
            out: Mutex::new(Outbox::default()),
            out_cv: Condvar::new(),
            next_id: AtomicU32::new(match role {
                Role::Client => 1,
                Role::Server => 2,
            }),
            handler,
            closer: Mutex::new(Some(Box::new(closer))),
        }));
        let m = mux.clone();
        std::thread::Builder::new()
            .name("snout-remote-write".into())
            .spawn(move || {
                let r = m.write_loop(BufWriter::with_capacity(64 * 1024, writer));
                m.die(match r {
                    Ok(()) => "the connection closed".into(),
                    Err(e) => format!("the connection failed: {e}"),
                });
            })
            .expect("spawn a thread");
        let m = mux.clone();
        std::thread::Builder::new()
            .name("snout-remote-read".into())
            .spawn(move || {
                let r = m.read_loop(BufReader::with_capacity(64 * 1024, reader));
                m.die(match r {
                    Ok(()) => "the connection closed".into(),
                    Err(e) => format!("the connection failed: {e}"),
                });
            })
            .expect("spawn a thread");
        mux
    }

    /// Whether the connection still runs.
    pub fn is_alive(&self) -> bool {
        lock(&self.0.state).dead.is_none()
    }

    /// Why the connection ended, once it has.
    pub fn dead_reason(&self) -> Option<String> {
        lock(&self.0.state).dead.clone()
    }

    /// Run `f` when the connection ends (at once, if it has).
    pub fn on_close(&self, f: impl FnOnce() + Send + 'static) {
        let mut st = lock(&self.0.state);
        if st.dead.is_some() {
            drop(st);
            f();
        } else {
            st.on_close.push(Box::new(f));
        }
    }

    /// End the connection.
    pub fn shutdown(&self) {
        self.die("the connection was shut down".into());
    }

    /// Whether `a` and `b` are the same connection.
    pub fn same(a: &Mux, b: &Mux) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }

    fn die(&self, why: String) {
        let (chans, calls, on_close) = {
            let mut st = lock(&self.0.state);
            if st.dead.is_some() {
                return;
            }
            st.dead = Some(why.clone());
            (std::mem::take(&mut st.chans), std::mem::take(&mut st.calls), std::mem::take(&mut st.on_close))
        };
        {
            let mut out = lock(&self.0.out);
            out.closed = true;
            out.urgent.clear();
            out.queues.clear();
            out.ring.clear();
        }
        self.0.out_cv.notify_all();
        if let Some(close) = lock(&self.0.closer).take() {
            close();
        }
        for chan in chans.values() {
            let mut st = lock(&chan.st);
            st.closed.get_or_insert(Closed::Dead(why.clone()));
            chan.cv.notify_all();
        }
        for (_, tx) in calls {
            let _ = tx.send(Err(io::Error::new(io::ErrorKind::ConnectionAborted, why.clone())));
        }
        for f in on_close {
            f();
        }
    }

    fn dead_error(&self) -> io::Error {
        io::Error::new(io::ErrorKind::ConnectionAborted, self.dead_reason().unwrap_or_default())
    }

    fn push(&self, f: Frame) {
        lock(&self.0.out).push(f);
        self.0.out_cv.notify_one();
    }

    fn push_all(&self, frames: Vec<Frame>) {
        let mut out = lock(&self.0.out);
        for f in frames {
            out.push(f);
        }
        drop(out);
        self.0.out_cv.notify_one();
    }

    fn push_urgent(&self, f: Frame) {
        let mut out = lock(&self.0.out);
        if !out.closed {
            out.urgent.push_back(f);
        }
        drop(out);
        self.0.out_cv.notify_one();
    }

    fn write_loop(&self, mut w: impl Write) -> io::Result<()> {
        let mut batch = Vec::with_capacity(BATCH);
        loop {
            {
                let mut out = lock(&self.0.out);
                loop {
                    if out.closed {
                        return Ok(());
                    }
                    while batch.len() < BATCH {
                        match out.pop() {
                            Some(f) => batch.push(f),
                            None => break,
                        }
                    }
                    if !batch.is_empty() {
                        break;
                    }
                    out = self.0.out_cv.wait(out).unwrap_or_else(|p| p.into_inner());
                }
            }
            for f in batch.drain(..) {
                frame::write_frame(&mut w, &f)?;
            }
            w.flush()?;
        }
    }

    fn read_loop(&self, mut r: impl Read) -> io::Result<()> {
        // Messages in pieces, by kind and channel.
        let mut partial: HashMap<(u8, ChannelId), Vec<u8>> = HashMap::new();
        while let Some(f) = frame::read_frame(&mut r)? {
            if !self.is_alive() {
                return Ok(());
            }
            let ch = f.channel;
            match f.kind {
                Kind::Hello => return Err(io::Error::new(io::ErrorKind::InvalidData, "a second hello")),
                Kind::Request | Kind::Response | Kind::Message => {
                    let key = (f.kind as u8, ch);
                    let fin = f.fin();
                    partial.entry(key).or_default().extend(f.payload);
                    if !fin {
                        continue;
                    }
                    let bytes = partial.remove(&key).unwrap_or_default();
                    // A message's window comes back whole when it is read.
                    let total = bytes.len();
                    let msg = Message::decode(bytes)?;
                    match f.kind {
                        Kind::Request => self.dispatch(ch, msg),
                        Kind::Response => {
                            if let Some(tx) = lock(&self.0.state).calls.remove(&ch) {
                                let _ = tx.send(Ok(msg));
                            }
                        }
                        _ => {
                            if let Some(chan) = self.chan(ch) {
                                lock(&chan.st).inq.push_back(Inbound::Msg(msg, total));
                                chan.cv.notify_all();
                            }
                        }
                    }
                }
                Kind::Data => {
                    if let Some(chan) = self.chan(ch) {
                        lock(&chan.st).inq.push_back(Inbound::Data(f.payload, 0));
                        chan.cv.notify_all();
                    }
                }
                Kind::Eof => {
                    if let Some(chan) = self.chan(ch) {
                        lock(&chan.st).inq.push_back(Inbound::Eof);
                        chan.cv.notify_all();
                    }
                }
                Kind::Window => {
                    let n = f
                        .payload
                        .get(..4)
                        .map(|b| u32::from_be_bytes(b.try_into().unwrap()) as usize)
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "a short window frame"))?;
                    if let Some(chan) = self.chan(ch) {
                        lock(&chan.st).credit += n;
                        chan.cv.notify_all();
                    }
                }
                Kind::Close => {
                    let chan = lock(&self.0.state).chans.remove(&ch);
                    if let Some(chan) = chan {
                        lock(&chan.st).closed.get_or_insert(Closed::Remote);
                        chan.cv.notify_all();
                    }
                }
            }
        }
        Ok(())
    }

    fn dispatch(&self, id: ChannelId, request: Message) {
        let Some(handler) = self.0.handler.clone() else {
            return;
        };
        let mux = self.clone();
        std::thread::Builder::new()
            .name("snout-remote-request".into())
            .spawn(move || {
                let reply = handler.handle(&mux, request);
                mux.push_all(reply.frames(id, Kind::Response));
            })
            .expect("spawn a thread");
    }

    fn chan(&self, id: ChannelId) -> Option<Arc<Chan>> {
        lock(&self.0.state).chans.get(&id).cloned()
    }

    /// A fresh id, for a request or a channel.
    pub fn next_id(&self) -> ChannelId {
        self.0.next_id.fetch_add(2, Ordering::Relaxed)
    }

    /// Send a request and wait for its response.
    pub fn call(&self, request: &Message) -> io::Result<Message> {
        self.call_timeout(request, None)
    }

    /// [`Self::call`], giving up after `timeout`.
    pub fn call_timeout(&self, request: &Message, timeout: Option<Duration>) -> io::Result<Message> {
        let id = self.next_id();
        let (tx, rx) = mpsc::channel();
        {
            let mut st = lock(&self.0.state);
            if st.dead.is_some() {
                drop(st);
                return Err(self.dead_error());
            }
            st.calls.insert(id, tx);
        }
        self.push_all(request.frames(id, Kind::Request));
        let reply = match timeout {
            None => rx.recv().map_err(|_| self.dead_error()),
            Some(t) => rx.recv_timeout(t).map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => io::Error::new(io::ErrorKind::TimedOut, "no answer in time"),
                mpsc::RecvTimeoutError::Disconnected => self.dead_error(),
            }),
        };
        if reply.is_err() {
            lock(&self.0.state).calls.remove(&id);
        }
        reply?
    }

    /// Open a channel with a fresh id.
    pub fn open_channel(&self) -> io::Result<Channel> {
        self.register_channel(self.next_id())
    }

    /// Open the channel `id`, which the peer chose (it named it in a request).
    pub fn register_channel(&self, id: ChannelId) -> io::Result<Channel> {
        let chan =
            Arc::new(Chan { st: Mutex::new(ChanState { credit: WINDOW, ..Default::default() }), cv: Condvar::new() });
        let mut st = lock(&self.0.state);
        if st.dead.is_some() {
            drop(st);
            return Err(self.dead_error());
        }
        if st.chans.contains_key(&id) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("channel {id} is already open")));
        }
        st.chans.insert(id, chan.clone());
        drop(st);
        Ok(Channel(Arc::new(ChannelInner { mux: self.clone(), id, chan })))
    }

    /// Wait until `n` bytes of window are free on `chan`, and take them.
    fn take_credit(&self, chan: &Chan, n: usize) -> io::Result<()> {
        let mut st = lock(&chan.st);
        loop {
            if let Some(c) = &st.closed {
                return Err(c.error());
            }
            if st.credit >= n {
                st.credit -= n;
                return Ok(());
            }
            st = chan.cv.wait(st).unwrap_or_else(|p| p.into_inner());
        }
    }

    fn close_channel(&self, id: ChannelId, chan: &Chan) {
        let removed = lock(&self.0.state).chans.remove(&id).is_some();
        {
            let mut st = lock(&chan.st);
            st.closed.get_or_insert(Closed::Local);
        }
        chan.cv.notify_all();
        if removed {
            self.push(Frame::new(id, Kind::Close, vec![]));
        }
    }
}

/// A stream channel. Clones share it, and the channel closes when the last one goes (or
/// on [`Channel::close`]). Reading and writing may happen on different threads.
#[derive(Clone)]
pub struct Channel(Arc<ChannelInner>);

struct ChannelInner {
    mux: Mux,
    id: ChannelId,
    chan: Arc<Chan>,
}

impl Drop for ChannelInner {
    fn drop(&mut self) {
        self.mux.close_channel(self.id, &self.chan);
    }
}

impl Channel {
    pub fn id(&self) -> ChannelId {
        self.0.id
    }

    pub fn mux(&self) -> &Mux {
        &self.0.mux
    }

    /// Send bytes, waiting for window as needed.
    pub fn send(&self, data: &[u8]) -> io::Result<()> {
        for piece in data.chunks(CHUNK) {
            self.0.mux.take_credit(&self.0.chan, piece.len())?;
            self.0.mux.push(Frame::new(self.0.id, Kind::Data, piece.to_vec()));
        }
        Ok(())
    }

    /// Send a message (a batch of watch events), waiting for window as needed.
    pub fn send_message(&self, msg: &Message) -> io::Result<()> {
        for f in msg.frames(self.0.id, Kind::Message) {
            self.0.mux.take_credit(&self.0.chan, f.payload.len())?;
            self.0.mux.push(f);
        }
        Ok(())
    }

    /// Send no more; the other direction stays open.
    pub fn send_eof(&self) -> io::Result<()> {
        if let Some(c) = &lock(&self.0.chan.st).closed {
            return Err(c.error());
        }
        self.0.mux.push(Frame::new(self.0.id, Kind::Eof, vec![]));
        Ok(())
    }

    /// Close both directions now, even while clones remain. Senders and readers blocked on
    /// the channel return an error.
    pub fn close(&self) {
        self.0.mux.close_channel(self.0.id, &self.0.chan);
    }

    /// Whether the channel was closed (by either side, or by the connection ending).
    pub fn is_closed(&self) -> bool {
        lock(&self.0.chan.st).closed.is_some()
    }

    fn consumed(&self, st: &mut ChanState, n: usize) {
        st.unacked += n;
        if st.unacked >= GRANT_AT {
            let give = std::mem::take(&mut st.unacked);
            self.0.mux.push_urgent(Frame::new(self.0.id, Kind::Window, (give as u32).to_be_bytes().to_vec()));
        }
    }

    /// Read bytes into `buf`: `Ok(0)` once the sender sent its end. An error when the
    /// channel was closed without an end (`BrokenPipe` by the peer, `NotConnected` here) or
    /// the connection is gone (`ConnectionAborted`).
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let chan = &self.0.chan;
        let mut st = lock(&chan.st);
        loop {
            match st.inq.front_mut() {
                Some(Inbound::Data(bytes, off)) => {
                    let n = (bytes.len() - *off).min(buf.len());
                    buf[..n].copy_from_slice(&bytes[*off..*off + n]);
                    *off += n;
                    if *off == bytes.len() {
                        st.inq.pop_front();
                    }
                    self.consumed(&mut st, n);
                    return Ok(n);
                }
                Some(Inbound::Eof) => return Ok(0),
                Some(Inbound::Msg(..)) => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "a message on a byte stream"));
                }
                None => match &st.closed {
                    Some(c) => return Err(c.error()),
                    None => st = chan.cv.wait(st).unwrap_or_else(|p| p.into_inner()),
                },
            }
        }
    }

    /// The next message: `None` once the sender sent its end; errors as for
    /// [`Self::recv`], and `TimedOut` after `timeout`.
    pub fn recv_message(&self, timeout: Option<Duration>) -> io::Result<Option<Message>> {
        let chan = &self.0.chan;
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        let mut st = lock(&chan.st);
        loop {
            match st.inq.front() {
                Some(Inbound::Msg(..)) => {
                    let Some(Inbound::Msg(msg, size)) = st.inq.pop_front() else { unreachable!() };
                    self.consumed(&mut st, size);
                    return Ok(Some(msg));
                }
                Some(Inbound::Eof) => return Ok(None),
                Some(Inbound::Data(..)) => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "bytes on a message stream"));
                }
                None => match &st.closed {
                    Some(c) => return Err(c.error()),
                    None => match deadline {
                        None => st = chan.cv.wait(st).unwrap_or_else(|p| p.into_inner()),
                        Some(d) => {
                            let now = std::time::Instant::now();
                            if now >= d {
                                return Err(io::Error::new(io::ErrorKind::TimedOut, "no message in time"));
                            }
                            st = chan.cv.wait_timeout(st, d - now).unwrap_or_else(|p| p.into_inner()).0;
                        }
                    },
                },
            }
        }
    }
}

impl Read for Channel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv(buf)
    }
}

impl Write for Channel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = buf.len().min(CHUNK);
        self.send(&buf[..n])?;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel").field("id", &self.0.id).finish()
    }
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
