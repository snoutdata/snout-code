//! `--stdio`: carry ssh's stdin and stdout to the daemon's socket, starting the daemon when
//! none answers.

use crate::Options;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn socket_path(dir: &Path) -> std::path::PathBuf {
    dir.join("daemon.sock")
}

/// Connect to the daemon, starting it if it does not answer.
fn connect_or_start(opts: &Options) -> io::Result<UnixStream> {
    let sock = socket_path(&opts.dir);
    if let Ok(s) = UnixStream::connect(&sock) {
        return Ok(s);
    }
    start_daemon(opts)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match UnixStream::connect(&sock) {
            Ok(s) => return Ok(s),
            Err(e) if Instant::now() > deadline => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("the daemon did not start (see {}): {e}", opts.dir.join("daemon.log").display()),
                ));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

/// Start the daemon in a session of its own, so the end of the ssh session does not take
/// it along. Two proxies racing both start one; the daemon's lock lets only one stay.
fn start_daemon(opts: &Options) -> io::Result<()> {
    std::fs::create_dir_all(&opts.dir)?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(opts.dir.join("daemon.log"))?;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("--daemon")
        .arg("--dir")
        .arg(&opts.dir)
        .arg("--idle-timeout")
        .arg(opts.idle.as_secs().to_string())
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .current_dir("/");
    // SAFETY: setsid is async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd.spawn()?;
    Ok(())
}

pub fn run(opts: &Options) -> io::Result<()> {
    let sock = connect_or_start(opts)?;
    let mut up = sock.try_clone()?;
    std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if up.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        // ssh is gone: the daemon sees the connection end.
        let _ = up.shutdown(std::net::Shutdown::Both);
    });
    let mut down = sock;
    let mut stdout = io::stdout().lock();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match down.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if stdout.write_all(&buf[..n]).and_then(|_| stdout.flush()).is_err() {
                    break;
                }
            }
        }
    }
    let _ = down.shutdown(std::net::Shutdown::Both);
    // The stdin thread may still sit in a read.
    std::process::exit(0);
}

/// `--stop`: ask a running daemon to stop.
pub fn stop(opts: &Options) -> io::Result<()> {
    let sock = match UnixStream::connect(socket_path(&opts.dir)) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("snout-server: no daemon runs in {}", opts.dir.display());
            return Ok(());
        }
    };
    let closer = sock.try_clone()?;
    let client = snout_remote_proto::Client::connect(sock.try_clone()?, sock, move || {
        let _ = closer.shutdown(std::net::Shutdown::Both);
    })?;
    client.shutdown_server()
}
