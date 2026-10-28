//! The app's side of Remote SSH: reach a host with the system's `ssh` (so `~/.ssh/config`,
//! the agent, `ProxyJump` and the rest apply), install the matching `snout-server` there,
//! connect, and connect again when the link drops.
//!
//! ```no_run
//! # fn main() -> std::io::Result<()> {
//! use snout_remote::{RemoteSession, SshTarget};
//! let session = RemoteSession::open(SshTarget::new("devbox"), "/path/to/server-binaries".as_ref())?;
//! let client = session.client()?;
//! println!("{}", client.read_to_string("/etc/hostname")?);
//! # Ok(()) }
//! ```
//!
//! Processes and ptys live in the server's daemon, not in the connection: after
//! [`RemoteSession::client`] reconnects, [`Client::list_processes`] shows them and
//! [`Client::attach`] / [`Client::attach_pty`] take up their streams again.

mod host;
mod ssh;

pub use host::{Connector, RemoteHost, Status};
pub use snout_remote_proto as proto;
pub use snout_remote_proto::Client;
pub use ssh::{Installed, SshTarget, connect, install, platform_binary};

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How [`RemoteSession`] retries a lost connection.
#[derive(Debug, Clone)]
pub struct Backoff {
    pub first: Duration,
    pub max: Duration,
    /// Give up after trying this long.
    pub give_up_after: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Backoff {
            first: Duration::from_millis(250),
            max: Duration::from_secs(10),
            give_up_after: Duration::from_secs(120),
        }
    }
}

/// A remote host: the server installed, and a connection that comes back after a drop.
pub struct RemoteSession {
    target: SshTarget,
    binaries: PathBuf,
    installed: Mutex<Installed>,
    current: Mutex<Option<Client>>,
    pub backoff: Backoff,
}

impl RemoteSession {
    /// Install the server if needed (`binaries` holds `snout-server-linux-x86_64` and
    /// `snout-server-linux-aarch64`, see `scripts/build-server-linux.sh`) and connect.
    pub fn open(target: SshTarget, binaries: &Path) -> io::Result<RemoteSession> {
        let installed = install(&target, binaries)?;
        let client = connect(&target, &installed)?;
        Ok(RemoteSession {
            target,
            binaries: binaries.to_path_buf(),
            installed: Mutex::new(installed),
            current: Mutex::new(Some(client)),
            backoff: Backoff::default(),
        })
    }

    pub fn target(&self) -> &SshTarget {
        &self.target
    }

    pub fn installed(&self) -> Installed {
        self.installed.lock().unwrap().clone()
    }

    /// The live connection, reconnecting (with backoff) when the last one dropped.
    pub fn client(&self) -> io::Result<Client> {
        let mut current = self.current.lock().unwrap();
        if let Some(c) = current.as_ref().filter(|c| c.is_alive()) {
            return Ok(c.clone());
        }
        let c = self.reconnect_locked()?;
        *current = Some(c.clone());
        Ok(c)
    }

    /// Drop the connection (if it still runs) and make a new one.
    pub fn reconnect(&self) -> io::Result<Client> {
        let mut current = self.current.lock().unwrap();
        if let Some(c) = current.take() {
            c.close();
        }
        let c = self.reconnect_locked()?;
        *current = Some(c.clone());
        Ok(c)
    }

    /// One attempt: connect to the installed server, installing it again if it is gone
    /// from the host (a wiped home).
    fn connect_once(&self) -> io::Result<Client> {
        let installed = self.installed();
        connect(&self.target, &installed).or_else(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                let again = install(&self.target, &self.binaries)?;
                *self.installed.lock().unwrap() = again.clone();
                connect(&self.target, &again)
            } else {
                Err(e)
            }
        })
    }

    fn reconnect_locked(&self) -> io::Result<Client> {
        let start = Instant::now();
        let mut delay = self.backoff.first;
        // A shared ssh connection may be hanging on to a dead link: start afresh.
        self.target.close_master();
        loop {
            match self.connect_once() {
                Ok(c) => return Ok(c),
                Err(e) if start.elapsed() + delay > self.backoff.give_up_after => return Err(e),
                Err(_) => {
                    std::thread::sleep(delay);
                    delay = (delay * 2).min(self.backoff.max);
                    self.target.close_master();
                }
            }
        }
    }
}

impl Connector for RemoteSession {
    fn connect(&self) -> io::Result<Client> {
        self.connect_once()
    }

    fn reset(&self) {
        self.target.close_master();
    }

    fn name(&self) -> String {
        self.target.host.clone()
    }
}
