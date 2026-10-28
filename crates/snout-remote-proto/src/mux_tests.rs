use super::*;
use serde_json::{Value, json};
use std::os::unix::net::UnixStream;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

/// Requests by `op`: `echo` returns the request; `big` a blob of `len` bytes; `flood` sends
/// `len` bytes on `channel` and counts what it got out in `sent`; `events` sends `count`
/// messages on `channel`; `upload` reads `channel` to its end and returns the byte count.
struct TestHandler {
    sent: Arc<AtomicUsize>,
}

impl Handler for TestHandler {
    fn handle(&self, mux: &Mux, request: Message) -> Message {
        let h: Value = request.parse().unwrap();
        let len = h["len"].as_u64().unwrap_or(0) as usize;
        match h["op"].as_str().unwrap() {
            "echo" => request,
            "big" => Message::new(&json!("big"), vec![0x5a; len]),
            "flood" => {
                let ch = mux.register_channel(h["channel"].as_u64().unwrap() as u32).unwrap();
                let sent = self.sent.clone();
                std::thread::spawn(move || {
                    let piece = vec![7u8; CHUNK];
                    let mut left = len;
                    while left > 0 {
                        let n = left.min(CHUNK);
                        if ch.send(&piece[..n]).is_err() {
                            return;
                        }
                        sent.fetch_add(n, Ordering::SeqCst);
                        left -= n;
                    }
                    ch.send_eof().unwrap();
                });
                Message::new(&json!("ok"), vec![])
            }
            "events" => {
                let ch = mux.register_channel(h["channel"].as_u64().unwrap() as u32).unwrap();
                let count = h["count"].as_u64().unwrap();
                std::thread::spawn(move || {
                    for i in 0..count {
                        ch.send_message(&Message::new(&json!({"i": i}), vec![1; 1000])).unwrap();
                    }
                    ch.send_eof().unwrap();
                });
                Message::new(&json!("ok"), vec![])
            }
            "upload" => {
                let ch = mux.register_channel(h["channel"].as_u64().unwrap() as u32).unwrap();
                let mut total = 0;
                let mut buf = vec![0; 10_000];
                loop {
                    match ch.recv(&mut buf).unwrap() {
                        0 => break,
                        n => total += n,
                    }
                }
                Message::new(&json!(total), vec![])
            }
            op => panic!("unknown op {op}"),
        }
    }
}

/// Writes slowly: as if over a slow link.
struct Slow<W>(W, Duration);

impl<W: Write> Write for Slow<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        std::thread::sleep(self.1);
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

struct Pair {
    client: Mux,
    sent: Arc<AtomicUsize>,
    socket: UnixStream,
}

fn pair(server_delay: Duration) -> Pair {
    let (a, b) = UnixStream::pair().unwrap();
    let sent = Arc::new(AtomicUsize::new(0));
    let handler: Arc<dyn Handler> = Arc::new(TestHandler { sent: sent.clone() });
    let b2 = b.try_clone().unwrap();
    let b3 = b.try_clone().unwrap();
    Mux::start(b.try_clone().unwrap(), Slow(b2, server_delay), Role::Server, Some(handler), move || {
        let _ = b3.shutdown(std::net::Shutdown::Both);
    });
    let a2 = a.try_clone().unwrap();
    let a3 = a.try_clone().unwrap();
    let client = Mux::start(a.try_clone().unwrap(), a2, Role::Client, None, move || {
        let _ = a3.shutdown(std::net::Shutdown::Both);
    });
    Pair { client, sent, socket: a }
}

fn req(v: Value) -> Message {
    Message::new(&v, vec![])
}

#[test]
fn requests_get_their_own_responses_concurrently() {
    let p = pair(Duration::ZERO);
    let threads: Vec<_> = (0..32)
        .map(|i| {
            let mux = p.client.clone();
            std::thread::spawn(move || {
                let m = Message::new(&json!({"op": "echo", "i": i}), vec![i as u8; i * 10_000]);
                assert_eq!(mux.call(&m).unwrap(), m);
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn a_big_response_does_not_hold_up_a_small_one() {
    // 8 MB at 1 ms per frame takes a quarter second or more.
    let p = pair(Duration::from_millis(1));
    let mux = p.client.clone();
    let started = Instant::now();
    let big = std::thread::spawn(move || {
        let reply = mux.call(&req(json!({"op": "big", "len": 8 << 20}))).unwrap();
        assert_eq!(reply.blob.len(), 8 << 20);
        started.elapsed()
    });
    std::thread::sleep(Duration::from_millis(30));
    let t = Instant::now();
    p.client.call(&req(json!({"op": "echo"}))).unwrap();
    let small = t.elapsed();
    let big = big.join().unwrap();
    assert!(small < big / 4, "the small request took {small:?}, the big one {big:?}");
}

#[test]
fn a_stream_nobody_reads_stops_its_producer_and_blocks_nothing_else() {
    let p = pair(Duration::ZERO);
    let ch = p.client.open_channel().unwrap();
    let len = 16 << 20;
    p.client.call(&req(json!({"op": "flood", "len": len, "channel": ch.id()}))).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let sent = p.sent.load(Ordering::SeqCst);
    assert!(sent <= WINDOW, "the producer got {sent} bytes out with nobody reading");
    assert!(sent > 0);
    // Requests still flow.
    let t = Instant::now();
    p.client.call(&req(json!({"op": "echo"}))).unwrap();
    assert!(t.elapsed() < Duration::from_secs(1));
    // Reading lets it all through, in order and complete.
    let mut got = 0;
    let mut buf = vec![0; 100_000];
    let mut c = ch.clone();
    loop {
        match c.read(&mut buf).unwrap() {
            0 => break,
            n => {
                assert!(buf[..n].iter().all(|b| *b == 7));
                got += n;
            }
        }
    }
    assert_eq!(got, len);
    assert_eq!(p.sent.load(Ordering::SeqCst), len);
}

#[test]
fn uploads_are_flow_controlled_too() {
    let p = pair(Duration::ZERO);
    let ch = p.client.open_channel().unwrap();
    let mux = p.client.clone();
    let id = ch.id();
    let call = std::thread::spawn(move || mux.call(&req(json!({"op": "upload", "channel": id}))).unwrap());
    let data = vec![3u8; 5 << 20];
    ch.send(&data).unwrap();
    ch.send_eof().unwrap();
    let total: usize = call.join().unwrap().parse().unwrap();
    assert_eq!(total, data.len());
}

#[test]
fn messages_on_a_stream_arrive_in_order_under_flow_control() {
    let p = pair(Duration::ZERO);
    let ch = p.client.open_channel().unwrap();
    // 2000 messages of about 1 KB: several windows' worth.
    p.client.call(&req(json!({"op": "events", "count": 2000, "channel": ch.id()}))).unwrap();
    for i in 0..2000 {
        let m = ch.recv_message(Some(Duration::from_secs(5))).unwrap().unwrap();
        assert_eq!(m.parse::<Value>().unwrap()["i"], i);
    }
    assert_eq!(ch.recv_message(None).unwrap(), None);
}

#[test]
fn a_dropped_connection_fails_calls_and_streams() {
    let p = pair(Duration::ZERO);
    let ch = p.client.open_channel().unwrap();
    p.client.call(&req(json!({"op": "flood", "len": 1 << 30, "channel": ch.id()}))).unwrap();
    let closed = Arc::new(AtomicUsize::new(0));
    let c = closed.clone();
    p.client.on_close(move || {
        c.fetch_add(1, Ordering::SeqCst);
    });
    p.socket.shutdown(std::net::Shutdown::Both).unwrap();
    let mut buf = vec![0; 1 << 20];
    let err = loop {
        match ch.recv(&mut buf) {
            Ok(n) => assert!(n > 0),
            Err(e) => break e,
        }
    };
    assert_eq!(err.kind(), io::ErrorKind::ConnectionAborted);
    assert!(!p.client.is_alive());
    assert_eq!(p.client.call(&req(json!({"op": "echo"}))).unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
    assert!(p.client.open_channel().is_err());
    assert_eq!(closed.load(Ordering::SeqCst), 1);
}

#[test]
fn closing_a_channel_stops_its_sender() {
    let p = pair(Duration::ZERO);
    let ch = p.client.open_channel().unwrap();
    p.client.call(&req(json!({"op": "flood", "len": 1 << 30, "channel": ch.id()}))).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    drop(ch);
    std::thread::sleep(Duration::from_millis(100));
    let after_close = p.sent.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(p.sent.load(Ordering::SeqCst), after_close, "the producer kept going after the close");
    assert!(p.client.is_alive());
}
