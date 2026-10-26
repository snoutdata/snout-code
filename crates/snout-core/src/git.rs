//! Thin wrapper around invoking the `git` binary.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Error, Result};

pub(crate) struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub success: bool,
}

impl Output {
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Combined, trimmed stdout + stderr; what a user would have seen in a terminal.
    pub fn text(&self) -> String {
        let mut s = String::from_utf8_lossy(&self.stdout).trim_end().to_owned();
        let err = String::from_utf8_lossy(&self.stderr);
        let err = err.trim_end();
        if !err.is_empty() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(err);
        }
        s
    }
}

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=off", "-c", "color.ui=never"])
        .args(args)
        // Never block on an interactive credential prompt; the GUI has no terminal.
        .env("GIT_TERMINAL_PROMPT", "0")
        // Read-only commands (status) should not take index.lock, so we never fight
        // with another process (an editor, an agent) working in the same repository.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    cmd
}

/// Run git and return its output regardless of the exit status.
pub(crate) fn run_raw(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<Output> {
    let mut cmd = command(dir, args);
    let out = match stdin {
        None => cmd.output()?,
        Some(input) => {
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut child = cmd.spawn()?;
            child.stdin.take().expect("piped stdin").write_all(input)?;
            child.wait_with_output()?
        }
    };
    Ok(Output { stdout: out.stdout, stderr: out.stderr, success: out.status.success() })
}

/// Run git against another index file, leaving the repository's own index alone.
pub(crate) fn run_in_index(dir: &Path, index: &Path, args: &[&str]) -> Result<Output> {
    let out = command(dir, args).env("GIT_INDEX_FILE", index).output()?;
    let out = Output { stdout: out.stdout, stderr: out.stderr, success: out.status.success() };
    if out.success { Ok(out) } else { Err(Error::Git { args: args.join(" "), message: out.text() }) }
}

/// Run git with nobody watching: SSH never asks anything on the terminal (a host key, a
/// passphrase) and a stalled connection gives up instead of hanging. The user's own SSH
/// command, if they set one, is left alone.
pub(crate) fn run_unattended(dir: &Path, args: &[&str]) -> Result<Output> {
    let slow = ["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=30"];
    let mut cmd = command(dir, &[&slow[..], args].concat());
    let own_ssh = std::env::var_os("GIT_SSH_COMMAND").is_some()
        || std::env::var_os("GIT_SSH").is_some()
        || run_raw(dir, &["config", "core.sshCommand"], None).is_ok_and(|o| o.success);
    if !own_ssh {
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=20");
    }
    let out = cmd.output()?;
    let out = Output { stdout: out.stdout, stderr: out.stderr, success: out.status.success() };
    if out.success { Ok(out) } else { Err(Error::Git { args: args.join(" "), message: out.text() }) }
}

/// Run git and turn a non-zero exit status into [`Error::Git`].
pub(crate) fn run(dir: &Path, args: &[&str]) -> Result<Output> {
    run_with_stdin(dir, args, None)
}

pub(crate) fn run_with_stdin(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<Output> {
    let out = run_raw(dir, args, stdin)?;
    if out.success { Ok(out) } else { Err(Error::Git { args: args.join(" "), message: out.text() }) }
}
