//! Runs that SnoutCode starts itself, on the host, without the Java engine: a program with
//! its arguments, folder and environment (`npm ci`, `npx playwright install`, a test run).
//! Its stdout and stderr stream into the session's console line by line; Stop asks it to
//! end, then ends it; its exit code ends the session.

use std::io::{BufRead as _, BufReader, Read};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::Context;
use snout_host::{Cmd, Process, Stdio};

use super::{Ended, Outcome, Place, Runs, Stream};

/// What a process run runs.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessLaunch {
    /// The session's title (`Install packages`).
    pub title: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Set on top of the inherited environment.
    pub env: Vec<(String, String)>,
    pub place: Place,
}

impl ProcessLaunch {
    /// The command line, as the console's first line says it.
    pub fn command_line(&self) -> String {
        let word = |w: &str| if w.contains(char::is_whitespace) { format!("\"{w}\"") } else { w.to_owned() };
        let program = self
            .program
            .file_name()
            .map_or_else(|| self.program.display().to_string(), |n| n.to_string_lossy().into_owned());
        std::iter::once(word(&program)).chain(self.args.iter().map(|a| word(a))).collect::<Vec<_>>().join(" ")
    }
}

/// The running process of a session, to stop it.
#[derive(Clone)]
pub struct Handle {
    child: Arc<Mutex<Box<dyn Process>>>,
    pub pid: Option<u32>,
    /// Stop was asked for: the end is a cancel, whatever the exit code.
    stopping: Arc<std::sync::atomic::AtomicBool>,
}

impl Handle {
    /// Ask the process to end (SIGTERM); end it (SIGKILL) if it is still there after a while.
    pub fn stop(&self) {
        self.stopping.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut c) = self.child.lock() {
            let _ = c.terminate();
        }
        let child = self.child.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            if let Ok(mut c) = child.lock()
                && matches!(c.try_wait(), Ok(None))
            {
                let _ = c.kill();
            }
        });
    }

    fn stopped(&self) -> bool {
        self.stopping.load(std::sync::atomic::Ordering::Relaxed)
    }
}

type Sender = futures::channel::mpsc::UnboundedSender<Event>;

enum Event {
    Line(Stream, String),
    Exit(Option<i32>),
    Failed(String),
}

/// Read `from` a line at a time into `tx` as `stream`, until it ends.
fn pump(from: Box<dyn Read + Send>, stream: Stream, tx: Sender) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(from);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    let line = line.trim_end_matches(['\n', '\r']);
                    // A progress line redrawn with `\r` shows as it ends.
                    let line = line.rsplit('\r').next().unwrap_or(line).to_owned();
                    if tx.unbounded_send(Event::Line(stream, line)).is_err() {
                        return;
                    }
                }
            }
        }
    });
}

impl Runs {
    /// Start session `id`'s process on the host.
    pub(super) fn spawn_process(&mut self, id: u64, launch: ProcessLaunch, cx: &mut Context<Self>) {
        let host = crate::host::host(cx);
        let mut cmd = Cmd::new(&launch.program).args(&launch.args).cwd(&launch.cwd);
        for (k, v) in &launch.env {
            cmd = cmd.env(k, v);
        }
        let cmd = cmd.stdin(Stdio::Null).stdout(Stdio::Piped).stderr(Stdio::Piped);
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Event>();
        match host.spawn(&cmd) {
            Ok(mut child) => {
                let pid = child.pid();
                if let Some(out) = child.take_stdout() {
                    pump(out, Stream::Out, tx.clone());
                }
                if let Some(err) = child.take_stderr() {
                    pump(err, Stream::Err, tx.clone());
                }
                let handle = Handle { child: Arc::new(Mutex::new(child)), pid, stopping: Default::default() };
                let waiter = handle.child.clone();
                std::thread::spawn(move || {
                    loop {
                        let status = waiter.lock().map(|mut c| c.try_wait());
                        match status {
                            Ok(Ok(Some(status))) => {
                                // What it printed last comes before its end.
                                std::thread::sleep(Duration::from_millis(50));
                                let _ = tx.unbounded_send(Event::Exit(status.code));
                                return;
                            }
                            Ok(Ok(None)) => std::thread::sleep(Duration::from_millis(100)),
                            Ok(Err(e)) => {
                                let _ = tx.unbounded_send(Event::Failed(e.to_string()));
                                return;
                            }
                            Err(_) => return,
                        }
                    }
                });
                if let Some(s) = self.session_mut(id) {
                    s.pid = pid;
                    s.process = Some(handle);
                }
            }
            Err(e) => {
                let _ = tx.unbounded_send(Event::Failed(format!("{} did not start: {e}", launch.program.display())));
            }
        }
        cx.spawn(async move |this, cx| {
            while let Some(event) = futures::StreamExt::next(&mut rx).await {
                let done = matches!(event, Event::Exit(_) | Event::Failed(_));
                let gone = this.update(cx, |this, cx| this.on_process_event(id, event, cx)).is_err();
                if done || gone {
                    return;
                }
            }
        })
        .detach();
    }

    fn on_process_event(&mut self, id: u64, event: Event, cx: &mut Context<Self>) {
        let Some(s) = self.session_mut(id) else { return };
        match event {
            Event::Line(stream, line) => s.push(stream, &line),
            Event::Exit(code) => {
                let stopped = s.process.as_ref().is_some_and(Handle::stopped);
                let outcome = match code {
                    _ if stopped => Outcome::Cancelled,
                    Some(0) => Outcome::Success,
                    _ => Outcome::Failed,
                };
                let what = match (outcome, code) {
                    (Outcome::Cancelled, _) => "Stopped".to_owned(),
                    (_, Some(c)) => format!("Process finished with exit code {c}"),
                    (_, None) => "Process ended by a signal".to_owned(),
                };
                s.push(Stream::System, &what);
                s.ended =
                    Some(Ended { outcome, elapsed: s.started.elapsed(), exit_code: code.map(i64::from), error: None });
                s.process = None;
                cx.emit(super::RunsEvent::Ended(id));
            }
            Event::Failed(why) => {
                s.push(Stream::Err, &why);
                s.ended = Some(Ended {
                    outcome: Outcome::Failed,
                    elapsed: s.started.elapsed(),
                    exit_code: None,
                    error: Some(why),
                });
                s.process = None;
                cx.emit(super::RunsEvent::Ended(id));
            }
        }
        cx.notify();
    }
}
