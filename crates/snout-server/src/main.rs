//! snout-server: the remote half of SnoutCode's Remote SSH mode.
//!
//! The app runs `ssh <host> ~/.snout-code/server/<version>/snout-server --stdio`. That
//! process is a thin proxy: it connects its stdin and stdout to the per-user daemon's Unix
//! socket (`daemon.sock` next to the binary), starting the daemon first if none runs. The
//! daemon owns everything that must outlive a dropped connection (processes, ptys), so a
//! network blip costs a reconnect, not the engine or the terminals.
//!
//! ```text
//! snout-server --stdio [--idle-timeout SECS] [--dir DIR]   proxy for ssh (starts the daemon)
//! snout-server --daemon [--idle-timeout SECS] [--dir DIR]  the daemon, in the foreground
//! snout-server --stop [--dir DIR]                          stop the daemon and its processes
//! snout-server --version
//! ```
//!
//! `DIR` defaults to `$SNOUT_SERVER_DIR`, else `~/.snout-code/server/<version>`. The daemon
//! stops after `SECS` (default 3 hours) without a client, and stops its processes when it
//! goes.

mod daemon;
mod fsops;
mod handler;
mod procs;
mod proxy;
mod tcp;
mod trash;
mod watch;

use snout_remote_proto::{PROTOCOL, VERSION};
use std::path::PathBuf;
use std::time::Duration;

pub struct Options {
    pub dir: PathBuf,
    pub idle: Duration,
}

const DEFAULT_IDLE: Duration = Duration::from_secs(3 * 60 * 60);

fn usage() -> ! {
    eprintln!("usage: snout-server (--stdio | --daemon | --stop | --version) [--idle-timeout SECS] [--dir DIR]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut mode = None;
    let mut dir = std::env::var_os("SNOUT_SERVER_DIR").map(PathBuf::from);
    let mut idle = DEFAULT_IDLE;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--stdio" | "--daemon" | "--stop" | "--version" => mode = Some(a),
            "--dir" => dir = Some(args.next().unwrap_or_else(|| usage()).into()),
            "--idle-timeout" => {
                let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
                idle = Duration::from_secs(secs);
            }
            _ => usage(),
        }
    }
    let dir = dir.unwrap_or_else(|| {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/tmp".into());
        home.join(".snout-code/server").join(VERSION)
    });
    let opts = Options { dir, idle };
    let result = match mode.as_deref() {
        Some("--version") => {
            println!("snout-server {VERSION} protocol {PROTOCOL}");
            Ok(())
        }
        Some("--stdio") => proxy::run(&opts),
        Some("--daemon") => daemon::run(&opts),
        Some("--stop") => proxy::stop(&opts),
        _ => usage(),
    };
    if let Err(e) = result {
        eprintln!("snout-server: {e}");
        std::process::exit(1);
    }
}
