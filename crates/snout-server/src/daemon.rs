//! The daemon: one per user and version, listening on `daemon.sock`.

use crate::Options;
use crate::handler::ServerHandler;
use crate::procs::Procs;
use crate::proxy::socket_path;
use snout_remote_proto::{Handler, Mux, Role, server_handshake};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct Daemon {
    pub procs: Procs,
    /// Listeners whose connections go back to the client's machine.
    pub listeners: crate::tcp::Listeners,
    /// This machine, for the requests the app's own host code answers.
    pub local: Arc<snout_host::LocalHost>,
    /// The login shell's environment, read on first use.
    login_env: OnceLock<BTreeMap<String, String>>,
    dir: PathBuf,
    idle: Duration,
    /// Connected clients, and when the count last changed.
    clients: Mutex<(usize, Instant)>,
    stopping: Mutex<bool>,
}

/// The SHA-256 of this executable, in hex.
pub fn build_hash() -> Option<String> {
    static HASH: OnceLock<Option<String>> = OnceLock::new();
    HASH.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(std::env::current_exe().ok()?).ok()?;
        Some(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
    })
    .clone()
}

pub fn log(msg: impl AsRef<str>) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    eprintln!("[{secs}] {}", msg.as_ref());
}

pub fn run(opts: &Options) -> io::Result<()> {
    std::fs::create_dir_all(&opts.dir)?;
    std::fs::set_permissions(&opts.dir, std::fs::Permissions::from_mode(0o700))?;
    let lock =
        std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(opts.dir.join("daemon.lock"))?;
    // A daemon that is stopping still holds the lock for a moment: wait for it a little.
    let deadline = Instant::now() + Duration::from_secs(5);
    // SAFETY: flock on a descriptor we own; the lock lives as long as `lock`.
    while unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        if Instant::now() > deadline {
            log("another daemon runs here; leaving it be");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let log_file = opts.dir.join("daemon.log");
    if std::fs::metadata(&log_file).map(|m| m.len() > 4 << 20).unwrap_or(false) {
        let _ = std::fs::write(&log_file, b"");
    }
    let signals = catch_signals()?;
    let sock = socket_path(&opts.dir);
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o600))?;
    std::fs::write(opts.dir.join("daemon.pid"), format!("{}\n", std::process::id()))?;
    log(format!(
        "snout-server {} daemon {} listening on {} (idle timeout {}s)",
        snout_remote_proto::VERSION,
        std::process::id(),
        sock.display(),
        opts.idle.as_secs()
    ));
    let daemon = Arc::new(Daemon {
        procs: Procs::default(),
        listeners: Default::default(),
        local: Arc::new(snout_host::LocalHost::new()),
        login_env: OnceLock::new(),
        dir: opts.dir.clone(),
        idle: opts.idle,
        clients: Mutex::new((0, Instant::now())),
        stopping: Mutex::new(false),
    });
    let d = daemon.clone();
    std::thread::spawn(move || d.wait_for_signal(signals));
    let d = daemon.clone();
    std::thread::spawn(move || d.watch_idle());
    let _ = build_hash();
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let d = daemon.clone();
                std::thread::spawn(move || {
                    if let Err(e) = d.serve(s) {
                        log(format!("a client failed to connect: {e}"));
                    }
                });
            }
            Err(e) => log(format!("accept failed: {e}")),
        }
    }
    drop(lock);
    Ok(())
}

/// The write end of the pipe the signal handler reports on.
static SIGNAL_PIPE: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(sig: libc::c_int) {
    let b = sig as u8;
    // SAFETY: write(2) is async-signal-safe.
    unsafe { libc::write(SIGNAL_PIPE.load(Ordering::Relaxed), (&b as *const u8).cast(), 1) };
}

/// Catch SIGTERM, SIGINT and SIGHUP with a handler that reports on a pipe, and return its
/// read end. (Blocking them for sigwait instead would leak the blocked mask into every
/// process the daemon starts, on macOS.) Children get default handlers at exec.
fn catch_signals() -> io::Result<std::fs::File> {
    let mut fds = [0; 2];
    // SAFETY: pipe(2) fills two descriptors, sigaction installs a plain handler.
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
        libc::fcntl(fds[0], libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(fds[1], libc::F_SETFD, libc::FD_CLOEXEC);
        SIGNAL_PIPE.store(fds[1], Ordering::Relaxed);
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_signal as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(sig, &action, std::ptr::null_mut());
        }
        Ok(std::os::fd::FromRawFd::from_raw_fd(fds[0]))
    }
}

impl Daemon {
    /// The environment processes start with: the login shell's.
    pub fn login_env(&self) -> BTreeMap<String, String> {
        self.login_env
            .get_or_init(|| {
                let env = crate::loginenv::read();
                log(format!("the login environment has {} variables", env.len()));
                env
            })
            .clone()
    }

    fn wait_for_signal(self: Arc<Self>, mut pipe: std::fs::File) {
        use std::io::Read;
        let mut sig = [0u8; 1];
        while pipe.read(&mut sig).is_err() {}
        self.stop(&format!("signal {}", sig[0]));
    }

    fn watch_idle(self: Arc<Self>) {
        let tick = (self.idle / 4).clamp(Duration::from_millis(100), Duration::from_secs(10));
        loop {
            std::thread::sleep(tick);
            let (count, since) = *self.clients.lock().unwrap();
            if count == 0 && since.elapsed() >= self.idle {
                self.stop("idle");
            }
        }
    }

    fn serve(self: Arc<Self>, stream: UnixStream) -> io::Result<()> {
        let mut r = stream.try_clone()?;
        let mut w = stream.try_clone()?;
        server_handshake(&mut r, &mut w, build_hash())?;
        {
            let mut c = self.clients.lock().unwrap();
            *c = (c.0 + 1, Instant::now());
            log(format!("a client connected ({} now)", c.0));
        }
        let handler: Arc<dyn Handler> = Arc::new(ServerHandler { daemon: self.clone(), local: self.local.clone() });
        let closer = stream.try_clone()?;
        let mux = Mux::start(r, w, Role::Server, Some(handler), move || {
            let _ = closer.shutdown(std::net::Shutdown::Both);
        });
        let d = self.clone();
        let m = mux.clone();
        mux.on_close(move || {
            let mut c = d.clients.lock().unwrap();
            *c = (c.0.saturating_sub(1), Instant::now());
            log(format!("a client left ({} now): {}", c.0, m.dead_reason().unwrap_or_default()));
        });
        Ok(())
    }

    /// Stop every process and exit. Returns only if a stop is already under way.
    pub fn stop(&self, why: &str) {
        {
            let mut s = self.stopping.lock().unwrap();
            if *s {
                return;
            }
            *s = true;
        }
        log(format!("stopping ({why})"));
        self.procs.stop_all(Duration::from_secs(3));
        let _ = std::fs::remove_file(socket_path(&self.dir));
        let _ = std::fs::remove_file(self.dir.join("daemon.pid"));
        log("stopped");
        std::process::exit(0);
    }
}
