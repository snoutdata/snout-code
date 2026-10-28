//! Processes and ptys. They belong to the daemon, not to a connection: their output waits
//! in a buffer while no client is attached (and once that is full, in their pipe, which
//! stops them), and a client that connects again attaches new channels to them.

use crate::daemon::log;
use portable_pty::{CommandBuilder, MasterPty, PtySize as PortablePtySize, native_pty_system};
use snout_remote_proto::frame::CHUNK;
use snout_remote_proto::{
    Channel, ExitStatus, Mux, ProcessId, ProcessInfo, PtyRequest, PtySize, Signal, SpawnRequest, Spawned, Stdio,
};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The output a stream keeps for a client while none is attached.
const RELAY_BUFFER: usize = 4 << 20;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Carries one output stream (stdout, stderr, a pty's output) to whichever channel is
/// attached, buffering while none is. Lossless: what the buffer cannot take stays in the
/// process's pipe.
pub struct Relay {
    st: Mutex<RelayState>,
    cv: Condvar,
}

#[derive(Default)]
struct RelayState {
    buf: VecDeque<Vec<u8>>,
    len: usize,
    /// The stream ended (everything is in `buf`).
    eof: bool,
    /// The end was sent to a client.
    done: bool,
    target: Option<Channel>,
    /// Counts attaches, so a failed send clears only the target it was made to.
    generation: u64,
}

impl Relay {
    fn start(mut source: Box<dyn Read + Send>, name: &str) -> Arc<Relay> {
        let relay = Arc::new(Relay { st: Mutex::default(), cv: Condvar::new() });
        let r = relay.clone();
        std::thread::Builder::new()
            .name(format!("{name}-read"))
            .spawn(move || {
                let mut buf = vec![0u8; CHUNK];
                loop {
                    // A pty reads EIO once its program has exited: that is its end too.
                    let n = source.read(&mut buf).unwrap_or(0);
                    let mut st = lock(&r.st);
                    if n == 0 {
                        st.eof = true;
                        r.cv.notify_all();
                        return;
                    }
                    while st.len >= RELAY_BUFFER {
                        st = r.cv.wait(st).unwrap_or_else(|p| p.into_inner());
                    }
                    st.len += n;
                    st.buf.push_back(buf[..n].to_vec());
                    r.cv.notify_all();
                }
            })
            .expect("spawn a thread");
        let r = relay.clone();
        std::thread::Builder::new().name(format!("{name}-send")).spawn(move || r.forward()).expect("spawn a thread");
        relay
    }

    fn forward(&self) {
        loop {
            let (target, generation, chunk) = {
                let mut st = lock(&self.st);
                loop {
                    if let Some(t) = &st.target {
                        if let Some(front) = st.buf.front() {
                            break (t.clone(), st.generation, Some(front.clone()));
                        }
                        if st.eof {
                            break (t.clone(), st.generation, None);
                        }
                    }
                    st = self.cv.wait(st).unwrap_or_else(|p| p.into_inner());
                }
            };
            match chunk {
                Some(chunk) => {
                    let sent = target.send(&chunk);
                    let mut st = lock(&self.st);
                    if sent.is_ok() {
                        // Only this thread takes from the front.
                        st.buf.pop_front();
                        st.len -= chunk.len();
                    } else if st.generation == generation {
                        st.target = None;
                    }
                    self.cv.notify_all();
                }
                None => {
                    let ended = target.send_eof();
                    let mut st = lock(&self.st);
                    if ended.is_ok() {
                        st.done = true;
                        st.target = None;
                        return;
                    }
                    if st.generation == generation {
                        st.target = None;
                    }
                }
            }
        }
    }

    /// Send what comes from now on (and what waits) to `channel`; the channel attached
    /// before is closed.
    fn attach(&self, channel: Channel) {
        let mut st = lock(&self.st);
        if st.done {
            let _ = channel.send_eof();
            return;
        }
        st.generation += 1;
        if let Some(old) = st.target.replace(channel) {
            old.close();
        }
        self.cv.notify_all();
    }

    fn attached(&self) -> bool {
        lock(&self.st).target.is_some()
    }
}

type Input = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

pub struct Proc {
    id: ProcessId,
    pid: u32,
    program: String,
    args: Vec<String>,
    pty: Option<Mutex<Box<dyn MasterPty + Send>>>,
    /// stdin, or the pty's input. `None` once closed.
    input: Option<Input>,
    /// The channel feeding `input`.
    input_channel: Mutex<Option<Channel>>,
    stdout: Option<Arc<Relay>>,
    stderr: Option<Arc<Relay>>,
    exit: Mutex<Option<ExitStatus>>,
    exited: Condvar,
}

impl Proc {
    fn info(&self) -> ProcessInfo {
        ProcessInfo {
            id: self.id,
            pid: self.pid,
            program: self.program.clone(),
            args: self.args.clone(),
            pty: self.pty.is_some(),
            exit: *lock(&self.exit),
            attached: [&self.stdout, &self.stderr].into_iter().flatten().any(|r| r.attached()),
        }
    }

    fn set_exit(&self, status: ExitStatus) {
        *lock(&self.exit) = Some(status);
        self.exited.notify_all();
    }

    /// Feed `channel` into the process's input; the channel fed before stops.
    fn attach_input(&self, channel: Channel) {
        let Some(input) = self.input.clone() else { return };
        if let Some(old) = lock(&self.input_channel).replace(channel.clone())
            && (old.id() != channel.id() || !Mux::same(old.mux(), channel.mux()))
        {
            old.close();
        }
        let pty = self.pty.is_some();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; CHUNK];
            loop {
                match channel.recv(&mut buf) {
                    // The client closed its input. A pty's input stays open (closing it
                    // would type an end of file); a pipe closes, and the process sees EOF.
                    Ok(0) => {
                        if !pty {
                            lock(&input).take();
                        }
                        return;
                    }
                    Ok(n) => {
                        let mut w = lock(&input);
                        let Some(w) = w.as_mut() else { return };
                        if w.write_all(&buf[..n]).and_then(|_| w.flush()).is_err() {
                            return;
                        }
                    }
                    // Detached (the connection dropped, or another attach took over).
                    Err(_) => return,
                }
            }
        });
    }

    fn signal(&self, signal: Signal) -> io::Result<()> {
        if lock(&self.exit).is_some() {
            return Ok(());
        }
        self.signal_group(signal)
    }

    /// Signal the process's group even after the process itself has exited: what it
    /// started may still run there.
    fn signal_group(&self, signal: Signal) -> io::Result<()> {
        let sig = match signal {
            Signal::Hup => libc::SIGHUP,
            Signal::Int => libc::SIGINT,
            Signal::Quit => libc::SIGQUIT,
            Signal::Kill => libc::SIGKILL,
            Signal::Term => libc::SIGTERM,
        };
        let pid = self.pid as libc::pid_t;
        // The process leads its own group (and a pty's its own session): signal them all.
        // SAFETY: plain kill(2).
        if unsafe { libc::kill(-pid, sig) } == 0 || unsafe { libc::kill(pid, sig) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() == Some(libc::ESRCH) { Ok(()) } else { Err(e) }
    }
}

#[derive(Default)]
pub struct Procs {
    table: Mutex<HashMap<ProcessId, Arc<Proc>>>,
    next: AtomicU64,
}

fn signal_number(name: &str) -> Option<i32> {
    // portable-pty reports a signal by its strsignal(3) name.
    (1..65).find(|s| {
        // SAFETY: strsignal returns a static string (or one in a thread local buffer).
        let p = unsafe { libc::strsignal(*s) };
        !p.is_null() && unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy() == name
    })
}

impl Procs {
    fn get(&self, id: ProcessId) -> io::Result<Arc<Proc>> {
        lock(&self.table)
            .get(&id)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no process {id}")))
    }

    fn insert(&self, proc_: Proc) -> Arc<Proc> {
        let p = Arc::new(proc_);
        lock(&self.table).insert(p.id, p.clone());
        p
    }

    fn new_id(&self) -> ProcessId {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn spawn(&self, mux: &Mux, req: SpawnRequest, base_env: &BTreeMap<String, String>) -> io::Result<Spawned> {
        let open = |id: Option<u32>| id.map(|id| mux.register_channel(id)).transpose();
        let (stdin_ch, stdout_ch, stderr_ch) =
            (open(req.stdio.stdin)?, open(req.stdio.stdout)?, open(req.stdio.stderr)?);
        let pipe = |on: bool| if on { std::process::Stdio::piped() } else { std::process::Stdio::null() };
        let output = |on: bool, file: &Option<std::path::PathBuf>| -> io::Result<std::process::Stdio> {
            Ok(match file {
                Some(f) => std::fs::File::create(f)
                    .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", f.display())))?
                    .into(),
                None => pipe(on),
            })
        };
        let mut cmd = Command::new(&req.program);
        cmd.args(&req.args)
            .stdin(pipe(stdin_ch.is_some()))
            .stdout(output(stdout_ch.is_some(), &req.stdout_file)?)
            .stderr(output(stderr_ch.is_some(), &req.stderr_file)?)
            .process_group(0);
        cmd.env_clear();
        if !req.env_clear {
            cmd.envs(base_env);
        }
        for k in &req.env_remove {
            cmd.env_remove(k);
        }
        cmd.envs(req.env.iter().map(|(k, v)| (k, v)));
        if let Some(cwd) = &req.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn().map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", req.program)))?;
        let id = self.new_id();
        let name = format!("proc-{id}");
        let input: Option<Input> =
            child.stdin.take().map(|s| Arc::new(Mutex::new(Some(Box::new(s) as Box<dyn Write + Send>))));
        let stdout = child.stdout.take().map(|s| Relay::start(Box::new(s), &format!("{name}-out")));
        let stderr = child.stderr.take().map(|s| Relay::start(Box::new(s), &format!("{name}-err")));
        let p = self.insert(Proc {
            id,
            pid: child.id(),
            program: req.program.clone(),
            args: req.args.clone(),
            pty: None,
            input,
            input_channel: Mutex::new(None),
            stdout,
            stderr,
            exit: Mutex::new(None),
            exited: Condvar::new(),
        });
        let waiter = p.clone();
        std::thread::spawn(move || {
            let status = match child.wait() {
                Ok(s) => ExitStatus { code: s.code(), signal: s.signal() },
                Err(_) => ExitStatus { code: None, signal: None },
            };
            waiter.set_exit(status);
        });
        self.connect(&p, [stdin_ch, stdout_ch, stderr_ch]);
        log(format!("started {} ({id}, pid {})", req.program, p.pid));
        Ok(Spawned { id, pid: p.pid })
    }

    fn connect(&self, p: &Proc, [stdin, stdout, stderr]: [Option<Channel>; 3]) {
        if let Some(c) = stdin {
            p.attach_input(c);
        }
        if let (Some(c), Some(r)) = (stdout, &p.stdout) {
            r.attach(c);
        }
        if let (Some(c), Some(r)) = (stderr, &p.stderr) {
            r.attach(c);
        }
    }

    pub fn spawn_pty(&self, mux: &Mux, req: PtyRequest, base_env: &BTreeMap<String, String>) -> io::Result<Spawned> {
        let channel = mux.register_channel(req.channel)?;
        let other = |e: anyhow::Error| io::Error::other(e.to_string());
        let pair = native_pty_system()
            .openpty(PortablePtySize { rows: req.size.rows, cols: req.size.cols, pixel_width: 0, pixel_height: 0 })
            .map_err(other)?;
        let mut cmd = CommandBuilder::new(&req.program);
        cmd.args(&req.args);
        cmd.env_clear();
        for (k, v) in base_env {
            cmd.env(k, v);
        }
        if !req.env.iter().any(|(k, _)| k == "TERM") {
            cmd.env("TERM", "xterm-256color");
        }
        for k in &req.env_remove {
            cmd.env_remove(k);
        }
        for (k, v) in &req.env {
            cmd.env(k, v);
        }
        match &req.cwd {
            Some(cwd) => cmd.cwd(cwd),
            None => {
                if let Some(home) = std::env::var_os("HOME") {
                    cmd.cwd(home)
                }
            }
        }
        let mut child = pair.slave.spawn_command(cmd).map_err(|e| io::Error::other(format!("{}: {e}", req.program)))?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(other)?;
        let writer = pair.master.take_writer().map_err(other)?;
        let id = self.new_id();
        let pid = child.process_id().unwrap_or(0);
        let p = self.insert(Proc {
            id,
            pid,
            program: req.program.clone(),
            args: req.args.clone(),
            pty: Some(Mutex::new(pair.master)),
            input: Some(Arc::new(Mutex::new(Some(writer)))),
            input_channel: Mutex::new(None),
            stdout: Some(Relay::start(reader, &format!("pty-{id}"))),
            stderr: None,
            exit: Mutex::new(None),
            exited: Condvar::new(),
        });
        let waiter = p.clone();
        std::thread::spawn(move || {
            let status = match child.wait() {
                Ok(s) => match s.signal() {
                    Some(name) => ExitStatus { code: None, signal: Some(signal_number(name).unwrap_or(0)) },
                    None => ExitStatus { code: Some(s.exit_code() as i32), signal: None },
                },
                Err(_) => ExitStatus::default(),
            };
            waiter.set_exit(status);
        });
        p.attach_input(channel.clone());
        if let Some(r) = &p.stdout {
            r.attach(channel);
        }
        log(format!("started {} on a pty ({id}, pid {pid})", req.program));
        Ok(Spawned { id, pid })
    }

    pub fn attach(&self, mux: &Mux, id: ProcessId, stdio: Stdio) -> io::Result<Spawned> {
        let p = self.get(id)?;
        let open = |id: Option<u32>| id.map(|id| mux.register_channel(id)).transpose();
        if p.pty.is_some() {
            if let Some(c) = open(stdio.stdout)? {
                p.attach_input(c.clone());
                if let Some(r) = &p.stdout {
                    r.attach(c);
                }
            }
        } else {
            let chans = [open(stdio.stdin)?, open(stdio.stdout)?, open(stdio.stderr)?];
            self.connect(&p, chans);
        }
        Ok(Spawned { id, pid: p.pid })
    }

    pub fn resize(&self, id: ProcessId, size: PtySize) -> io::Result<()> {
        let p = self.get(id)?;
        let pty = p.pty.as_ref().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a pty"))?;
        lock(pty)
            .resize(PortablePtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| io::Error::other(e.to_string()))
    }

    pub fn kill(&self, id: ProcessId, signal: Signal) -> io::Result<()> {
        self.get(id)?.signal(signal)
    }

    /// Wait for the process to exit, then forget it.
    pub fn wait(&self, id: ProcessId) -> io::Result<ExitStatus> {
        let p = self.get(id)?;
        let mut exit = lock(&p.exit);
        while exit.is_none() {
            exit = p.exited.wait(exit).unwrap_or_else(|e| e.into_inner());
        }
        let status = exit.expect("set");
        drop(exit);
        lock(&self.table).remove(&id);
        Ok(status)
    }

    /// How it ended, if it has; it is not forgotten.
    pub fn status(&self, id: ProcessId) -> io::Result<Option<ExitStatus>> {
        Ok(*lock(&self.get(id)?.exit))
    }

    pub fn release(&self, id: ProcessId) -> io::Result<()> {
        lock(&self.table).remove(&id).map(|_| ()).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no process"))
    }

    pub fn list(&self) -> Vec<ProcessInfo> {
        let mut v: Vec<_> = lock(&self.table).values().map(|p| p.info()).collect();
        v.sort_by_key(|p| p.id);
        v
    }

    /// SIGTERM to everything still running, then SIGKILL to what is left after `grace`.
    pub fn stop_all(&self, grace: Duration) {
        let all: Vec<_> = lock(&self.table).values().cloned().collect();
        for p in &all {
            let _ = p.signal_group(Signal::Term);
        }
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline && all.iter().any(|p| lock(&p.exit).is_none()) {
            std::thread::sleep(Duration::from_millis(20));
        }
        for p in &all {
            let _ = p.signal_group(Signal::Kill);
        }
    }
}
