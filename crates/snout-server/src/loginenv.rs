//! The environment of the user's login shell. ssh starts the server with the bare
//! environment of a non-interactive shell, which often lacks what the user's profile adds
//! (`PATH` entries, `JAVA_HOME`, SDK managers): the processes the app starts get the login
//! shell's environment instead.

use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Printed before the environment, so whatever the profile prints is told apart from it.
const MARK: &str = "__SNOUT_LOGIN_ENV__";

/// How long a shell may take to start.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Variables that belong to the shell that printed them, not to a new process.
const SHELL_OWN: &[&str] = &["_", "PWD", "OLDPWD", "SHLVL", "PS1", "PS2"];

fn user_shell() -> String {
    if let Some(s) = std::env::var_os("SHELL").filter(|s| !s.is_empty()) {
        return s.to_string_lossy().into_owned();
    }
    // SAFETY: getpwuid returns a static record or null.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_shell.is_null() {
            let s = std::ffi::CStr::from_ptr((*pw).pw_shell).to_string_lossy().into_owned();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "/bin/sh".into()
}

/// The variables `env -0` (or plain `env`) printed after [`MARK`].
fn parse(out: &[u8]) -> Option<BTreeMap<String, String>> {
    let text = String::from_utf8_lossy(out);
    let (_, after) = text.split_once(MARK)?;
    let records: Vec<&str> = if after.contains('\0') { after.split('\0').collect() } else { after.lines().collect() };
    let env: BTreeMap<String, String> = records
        .into_iter()
        .map(|r| r.trim_start_matches('\n'))
        .filter_map(|r| r.split_once('='))
        .filter(|(k, _)| !k.is_empty() && !k.contains(char::is_whitespace) && !SHELL_OWN.contains(k))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    env.contains_key("PATH").then_some(env)
}

fn run_shell(shell: &str, args: &[&str]) -> Option<BTreeMap<String, String>> {
    let script = format!("printf '%s' {MARK}; env -0 2>/dev/null || env");
    let mut child = Command::new(shell)
        .args(args)
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        out
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                // SAFETY: plain kill(2) of the group the shell leads.
                unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
                let _ = child.wait();
                break;
            }
        }
    }
    // What the shell left running in the background may hold stdout open: do not wait on it.
    let deadline = Instant::now() + Duration::from_secs(1);
    while !reader.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !reader.is_finished() {
        return None;
    }
    parse(&reader.join().ok()?)
}

/// The login shell's environment: an interactive login shell first (where most people set
/// `PATH`), then a plain login shell, else the server's own environment.
pub fn read() -> BTreeMap<String, String> {
    if std::env::var_os("SNOUT_SERVER_NO_LOGIN_ENV").is_none() {
        let shell = user_shell();
        for args in [&["-i", "-l", "-c"][..], &["-l", "-c"]] {
            if let Some(env) = run_shell(&shell, args) {
                return env;
            }
        }
    }
    std::env::vars().filter(|(k, _)| !SHELL_OWN.contains(&k.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_follows_the_mark() {
        let out = b"Welcome!\nPATH=/bad\n__SNOUT_LOGIN_ENV__PATH=/usr/bin:/opt/jdk/bin\0JAVA_HOME=/opt/jdk\0X=a=b\nc\0_=/usr/bin/env\0";
        let env = parse(out).unwrap();
        assert_eq!(env["PATH"], "/usr/bin:/opt/jdk/bin");
        assert_eq!(env["JAVA_HOME"], "/opt/jdk");
        assert_eq!(env["X"], "a=b\nc");
        assert!(!env.contains_key("_"));
        // Without `env -0`: one per line.
        let env = parse(b"__SNOUT_LOGIN_ENV__PATH=/bin\nHOME=/h\n").unwrap();
        assert_eq!(env["HOME"], "/h");
        assert!(parse(b"no mark here").is_none());
    }

    #[test]
    fn a_shell_gives_its_environment() {
        let env = run_shell("/bin/sh", &["-c"]).unwrap();
        assert!(env.contains_key("PATH"));
        assert!(!env.contains_key("PWD"));
    }
}
