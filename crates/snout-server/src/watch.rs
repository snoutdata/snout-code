//! File watching with notify (inotify on Linux), sent as batches of events.

use notify::event::{EventKind, ModifyKind};
use notify::{RecursiveMode, Watcher};
use snout_remote_proto::{Channel, Message, WatchEvent, WatchKind};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Events closer together than this go out as one batch.
const GATHER: Duration = Duration::from_millis(20);
const MAX_BATCH: usize = 2000;

fn to_io(e: notify::Error) -> io::Error {
    match e.kind {
        notify::ErrorKind::Io(e) => e,
        notify::ErrorKind::PathNotFound => io::Error::new(io::ErrorKind::NotFound, "no such file or folder"),
        _ => io::Error::other(e.to_string()),
    }
}

fn convert(r: notify::Result<notify::Event>) -> WatchEvent {
    match r {
        Ok(e) => {
            let kind = if e.need_rescan() {
                WatchKind::Rescan
            } else {
                match e.kind {
                    EventKind::Create(_) => WatchKind::Create,
                    EventKind::Modify(ModifyKind::Name(_)) => WatchKind::Rename,
                    EventKind::Modify(_) => WatchKind::Modify,
                    EventKind::Remove(_) => WatchKind::Remove,
                    EventKind::Access(_) => WatchKind::Access,
                    EventKind::Any | EventKind::Other => WatchKind::Other,
                }
            };
            WatchEvent { kind, paths: e.paths }
        }
        // Lost events (an inotify queue overflow): whatever is watched may have changed.
        Err(e) => WatchEvent { kind: WatchKind::Rescan, paths: e.paths },
    }
}

/// Watch `path` and send its changes on `channel` until the channel or the connection
/// closes. A consumer that falls behind gets bigger batches, not unbounded frames.
pub fn start(path: PathBuf, recursive: bool, channel: Channel) -> io::Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |r| {
        let _ = tx.send(r);
    })
    .map_err(to_io)?;
    let mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
    watcher.watch(&path, mode).map_err(to_io)?;
    std::thread::Builder::new().name("snout-watch".into()).spawn(move || {
        let _watcher = watcher;
        loop {
            let first = match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(e) => e,
                Err(RecvTimeoutError::Timeout) if channel.is_closed() => return,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            let mut batch = vec![convert(first)];
            let until = Instant::now() + GATHER;
            while batch.len() < MAX_BATCH {
                let left = until.saturating_duration_since(Instant::now());
                match rx.recv_timeout(left) {
                    Ok(e) => batch.push(convert(e)),
                    Err(_) => break,
                }
            }
            if channel.send_message(&Message::new(&batch, Vec::new())).is_err() {
                return;
            }
        }
    })?;
    Ok(())
}
