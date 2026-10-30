//! ssh: probing, installing and launching the server.

use snout_remote_proto::{Client, VERSION};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

/// How to reach a host with the system's ssh.
#[derive(Debug, Clone)]
pub struct SshTarget {
    /// As ssh takes it: a `Host` of the ssh config, `user@host`, `host`.
    pub host: String,
    /// `ssh -F`: another config file instead of `~/.ssh/config`.
    pub config_file: Option<PathBuf>,
    /// More ssh arguments, before the host (`-p 2222`, `-o Option=value`).
    pub extra_args: Vec<String>,
    /// Where the shared connection's control socket lives (`ControlMaster`); `None` turns
    /// connection sharing off. Keep it short: its path plus 34 bytes must stay under 104
    /// (a Unix socket path's limit).
    pub control_dir: Option<PathBuf>,
    /// No password or passphrase prompts (the app has no terminal for them): keys come
    /// from the agent or unencrypted files.
    pub batch_mode: bool,
    pub ssh_program: PathBuf,
    /// The program ssh runs to ask for a password, a passphrase or whether to trust a host
    /// key (`SSH_ASKPASS`); it runs with [`ASKPASS_ENV`] set. With one, prompts are allowed
    /// (no `BatchMode`).
    pub askpass: Option<PathBuf>,
    /// The daemon stops after this long without a client.
    pub idle_timeout: Duration,
    /// How long the connection and the handshake may take.
    pub connect_timeout: Duration,
}

impl SshTarget {
    pub fn new(host: impl Into<String>) -> SshTarget {
        SshTarget {
            host: host.into(),
            config_file: None,
            extra_args: Vec::new(),
            // Windows's OpenSSH has no ControlMaster: each ssh connects on its own there.
            control_dir: (!cfg!(windows)).then(|| std::env::temp_dir().join("snout-ssh")),
            batch_mode: true,
            ssh_program: "ssh".into(),
            askpass: None,
            idle_timeout: Duration::from_secs(3 * 60 * 60),
            connect_timeout: Duration::from_secs(30),
        }
    }

    fn base_command(&self) -> Command {
        let mut c = Command::new(&self.ssh_program);
        snout_host::default_signals(&mut c);
        if let Some(f) = &self.config_file {
            c.arg("-F").arg(f);
        }
        let mut opt = |o: String| {
            c.arg("-o").arg(o);
        };
        opt(format!("ConnectTimeout={}", self.connect_timeout.as_secs().max(1)));
        opt("ServerAliveInterval=15".into());
        opt("ServerAliveCountMax=3".into());
        if self.batch_mode && self.askpass.is_none() {
            opt("BatchMode=yes".into());
        }
        if let Some(dir) = &self.control_dir {
            let _ = std::fs::create_dir_all(dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
            opt("ControlMaster=auto".into());
            opt(format!("ControlPath={}", dir.join(self.control_name()).display()));
            opt("ControlPersist=10m".into());
        }
        c.args(&self.extra_args);
        if let Some(askpass) = &self.askpass {
            c.env("SSH_ASKPASS", askpass).env("SSH_ASKPASS_REQUIRE", "force").env(ASKPASS_ENV, "1");
            // Older ssh asks only with a display.
            if std::env::var_os("DISPLAY").is_none() {
                c.env("DISPLAY", ":0");
            }
        }
        c
    }

    /// The control socket's name: short, since ssh adds 17 bytes while it sets the socket
    /// up and the whole path must stay under 104 (ssh's own `%C` is 40 bytes long).
    fn control_name(&self) -> String {
        let mut key = self.host.clone();
        if let Some(f) = &self.config_file {
            key.push_str(&format!("\0{}", f.display()));
        }
        for a in &self.extra_args {
            key.push_str(&format!("\0{a}"));
        }
        sha256_hex(key.as_bytes())[..16].to_owned()
    }

    /// The command line (program first) of an interactive session that runs `remote` on the
    /// host in a terminal of its own (`ssh -t`): the same config file, options and shared
    /// connection as the app's, prompts allowed.
    pub fn terminal_command(&self, remote: &str) -> Vec<String> {
        let mut v = vec![self.ssh_program.display().to_string()];
        if let Some(f) = &self.config_file {
            v.extend(["-F".to_owned(), f.display().to_string()]);
        }
        if let Some(dir) = &self.control_dir {
            v.extend(["-o".to_owned(), "ControlMaster=auto".to_owned()]);
            v.extend(["-o".to_owned(), format!("ControlPath={}", dir.join(self.control_name()).display())]);
            v.extend(["-o".to_owned(), "ControlPersist=10m".to_owned()]);
        }
        v.extend(self.extra_args.iter().cloned());
        v.extend(["-t".to_owned(), self.host.clone(), remote.to_owned()]);
        v
    }

    /// `ssh <host> <remote command>`, no terminal.
    pub fn command(&self, remote: &str) -> Command {
        let mut c = self.base_command();
        c.arg("-T").arg(&self.host).arg(remote);
        c
    }

    /// Run `script` with `sh` on the host, `input` on its stdin.
    pub fn run_sh(&self, script: &str, input: &[u8]) -> io::Result<std::process::Output> {
        assert!(!script.contains('\''), "the script travels in single quotes");
        let mut child = self
            .command(&format!("sh -c '{script}'"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| io::Error::new(e.kind(), format!("cannot run ssh: {e}")))?;
        let mut stdin = child.stdin.take().expect("piped");
        let input = input.to_vec();
        let feeder = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
        let out = child.wait_with_output()?;
        let _ = feeder.join();
        Ok(out)
    }

    /// Stop the shared connection, if one runs (it may hang on to a dead link).
    pub fn close_master(&self) {
        if self.control_dir.is_none() {
            return;
        }
        let mut c = self.base_command();
        let _ = c
            .arg("-O")
            .arg("exit")
            .arg(&self.host)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Set in the environment of the [`SshTarget::askpass`] program ssh runs.
pub const ASKPASS_ENV: &str = "SNOUT_ASKPASS";

/// What [`install`] found or put on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// `linux`.
    pub os: String,
    /// `x86_64`, `aarch64`.
    pub arch: String,
    /// The server binary, for the remote shell (it starts with `$HOME`).
    pub path: String,
    /// The binary's SHA-256.
    pub sha256: String,
    /// Whether this call uploaded it.
    pub uploaded: bool,
}

fn remote_dir() -> String {
    format!("$HOME/.snout-code/server/{VERSION}")
}

/// The server binary for a host's `uname -s` and `uname -m`, in `binaries`.
pub fn platform_binary(binaries: &Path, os: &str, arch: &str) -> io::Result<(String, String, PathBuf)> {
    let os = match os {
        "Linux" | "linux" => "linux",
        other => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("{other} hosts are not supported (Linux only)"),
            ));
        }
    };
    let arch = match arch {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" | "armv8l" => "aarch64",
        other => {
            return Err(io::Error::new(io::ErrorKind::Unsupported, format!("{other} processors are not supported")));
        }
    };
    let file = binaries.join(format!("snout-server-{os}-{arch}"));
    if !file.is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("no server binary at {}", file.display())));
    }
    Ok((os.into(), arch.into(), file))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn ssh_failure(what: &str, out: &std::process::Output) -> io::Error {
    let err = String::from_utf8_lossy(&out.stderr);
    let err = err.trim();
    let kind = match out.status.code() {
        Some(255) => io::ErrorKind::ConnectionRefused,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("{what} failed ({}): {err}", out.status))
}

/// Make sure the host has the server of this version: find its os and processor, compare
/// the binary there (if any) with ours by checksum, and upload ours through ssh's stdin
/// when they differ. The upload is checked on the host before it replaces anything.
pub fn install(target: &SshTarget, binaries: &Path) -> io::Result<Installed> {
    install_with(target, binaries, &|| {})
}

/// [`install`], calling `uploading` before it uploads the server.
pub fn install_with(target: &SshTarget, binaries: &Path, uploading: &dyn Fn()) -> io::Result<Installed> {
    let dir = remote_dir();
    let probe = format!(
        "uname -s; uname -m; f=\"{dir}/snout-server\"; if [ -f \"$f\" ]; then \
         (sha256sum \"$f\" 2>/dev/null || shasum -a 256 \"$f\" 2>/dev/null) | cut -d\" \" -f1; fi"
    );
    let out = target.run_sh(&probe, b"")?;
    if !out.status.success() {
        return Err(ssh_failure("probing the host", &out));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>();
    // Login scripts may print before our output; ours is the last two or three lines.
    let present =
        lines.last().filter(|l| l.len() == 64 && l.bytes().all(|b| b.is_ascii_hexdigit())).map(|s| s.to_string());
    if present.is_some() {
        lines.pop();
    }
    let (arch, os) = (lines.pop().unwrap_or_default(), lines.pop().unwrap_or_default());
    let (os, arch, file) = platform_binary(binaries, os, arch)?;
    let bytes = std::fs::read(&file)?;
    let sha256 = sha256_hex(&bytes);
    let path = format!("{dir}/snout-server");
    if present.as_deref() == Some(sha256.as_str()) {
        return Ok(Installed { os, arch, path, sha256, uploaded: false });
    }
    uploading();
    let upload = format!(
        "set -e; d=\"{dir}\"; mkdir -p \"$d\"; chmod 700 \"$HOME/.snout-code\" 2>/dev/null || true; \
         t=\"$d/snout-server.$$.part\"; cat > \"$t\"; \
         h=$( (sha256sum \"$t\" 2>/dev/null || shasum -a 256 \"$t\") | cut -d\" \" -f1); \
         if [ \"$h\" != \"{sha256}\" ]; then rm -f \"$t\"; echo \"the upload arrived damaged ($h)\" >&2; exit 3; fi; \
         chmod 755 \"$t\"; mv -f \"$t\" \"$d/snout-server\"; \"$d/snout-server\" --version"
    );
    let out = target.run_sh(&upload, &bytes)?;
    if !out.status.success() {
        return Err(ssh_failure("installing snout-server", &out));
    }
    Ok(Installed { os, arch, path, sha256, uploaded: true })
}

/// The last few KB ssh wrote on stderr.
#[derive(Clone, Default)]
struct StderrTail(Arc<Mutex<Vec<u8>>>);

impl StderrTail {
    fn collect(&self, mut from: impl Read + Send + 'static) -> mpsc::Receiver<()> {
        let tail = self.0.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n @ 1..) = from.read(&mut buf) {
                let mut t = tail.lock().unwrap();
                t.extend(&buf[..n]);
                let excess = t.len().saturating_sub(8192);
                t.drain(..excess);
            }
            let _ = tx.send(());
        });
        rx
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).trim().to_owned()
    }
}

fn spawn_server(target: &SshTarget, installed: &Installed) -> io::Result<(Child, StderrTail, mpsc::Receiver<()>)> {
    let remote = format!("{} --stdio --idle-timeout {}", installed.path, target.idle_timeout.as_secs());
    let mut child = target
        .command(&remote)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| io::Error::new(e.kind(), format!("cannot run ssh: {e}")))?;
    let tail = StderrTail::default();
    let done = tail.collect(child.stderr.take().expect("piped"));
    Ok((child, tail, done))
}

fn connect_once(target: &SshTarget, installed: &Installed) -> io::Result<Client> {
    let (mut child, tail, stderr_done) = spawn_server(target, installed)?;
    let stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");
    let child = Arc::new(Mutex::new(child));
    // A link that hangs during the handshake: kill ssh after the timeout.
    let shaken = Arc::new(AtomicBool::new(false));
    {
        let (child, shaken, timeout) = (child.clone(), shaken.clone(), target.connect_timeout);
        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            if !shaken.load(Ordering::SeqCst) {
                let _ = child.lock().unwrap().kill();
            }
        });
    }
    let killer = child.clone();
    let result = Client::connect(stdout, stdin, move || {
        let mut c = killer.lock().unwrap();
        let _ = c.kill();
        let _ = c.wait();
    });
    shaken.store(true, Ordering::SeqCst);
    result.map_err(|e| {
        let _ = stderr_done.recv_timeout(Duration::from_secs(2));
        let status = child.lock().unwrap().wait().ok();
        let stderr = tail.text();
        let kind = match status.and_then(|s| s.code()) {
            // The remote shell found no server.
            Some(126 | 127) => io::ErrorKind::NotFound,
            Some(255) => io::ErrorKind::ConnectionRefused,
            _ => e.kind(),
        };
        let mut msg = format!("cannot connect to {}: {e}", target.host);
        if !stderr.is_empty() {
            msg.push_str(&format!(" ({stderr})"));
        }
        io::Error::new(kind, msg)
    })
}

/// Connect to the installed server (the daemon starts on the host if none runs). A daemon
/// still running another build of this version (a reinstall during development) is
/// stopped and started afresh.
pub fn connect(target: &SshTarget, installed: &Installed) -> io::Result<Client> {
    let client = connect_once(target, installed)?;
    match &client.server().build {
        Some(build) if *build != installed.sha256 => {
            let _ = client.shutdown_server();
            client.close();
            drop(client);
            std::thread::sleep(Duration::from_millis(500));
            connect_once(target, installed)
        }
        _ => Ok(client),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uname_names_pick_the_binary() {
        let dir = std::env::temp_dir().join(format!("snout-remote-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("snout-server-linux-x86_64"), b"x").unwrap();
        std::fs::write(dir.join("snout-server-linux-aarch64"), b"a").unwrap();
        let pick = |os, arch| platform_binary(&dir, os, arch).map(|(o, a, _)| format!("{o}-{a}"));
        assert_eq!(pick("Linux", "x86_64").unwrap(), "linux-x86_64");
        assert_eq!(pick("Linux", "amd64").unwrap(), "linux-x86_64");
        assert_eq!(pick("Linux", "aarch64").unwrap(), "linux-aarch64");
        assert_eq!(pick("Linux", "arm64").unwrap(), "linux-aarch64");
        assert_eq!(pick("Darwin", "arm64").unwrap_err().kind(), io::ErrorKind::Unsupported);
        assert_eq!(pick("Linux", "riscv64").unwrap_err().kind(), io::ErrorKind::Unsupported);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(pick("Linux", "x86_64").unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn a_terminal_session_shares_the_apps_connection() {
        let mut t = SshTarget::new("devbox");
        t.config_file = Some("/cfg/ssh".into());
        t.control_dir = Some("/tmp/snout-ssh-1".into());
        let argv = t.terminal_command("cd /srv; exec $SHELL -l");
        let control = format!("ControlPath=/tmp/snout-ssh-1/{}", t.control_name());
        assert_eq!(argv[..3], ["ssh", "-F", "/cfg/ssh"]);
        assert!(argv.contains(&control), "{argv:?}");
        assert!(!argv.iter().any(|a| a.starts_with("BatchMode")), "a terminal may ask for a password");
        assert_eq!(argv[argv.len() - 3..], ["-t", "devbox", "cd /srv; exec $SHELL -l"]);
    }

    #[test]
    fn the_control_socket_name_is_short_and_per_target() {
        let a = SshTarget::new("devbox");
        let mut b = SshTarget::new("devbox");
        b.extra_args = vec!["-p".into(), "2222".into()];
        assert_eq!(a.control_name().len(), 16);
        assert_ne!(a.control_name(), b.control_name());
        assert_eq!(a.control_name(), SshTarget::new("devbox").control_name());
        let args: Vec<String> = a.command("true").get_args().map(|s| s.to_string_lossy().into_owned()).collect();
        assert!(args.contains(&"ControlMaster=auto".to_owned()), "{args:?}");
        assert!(args.contains(&"BatchMode=yes".to_owned()), "{args:?}");
        assert_eq!(&args[args.len() - 3..], ["-T", "devbox", "true"]);
    }
}
