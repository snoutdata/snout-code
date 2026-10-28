//! Discovery: the lock file Claude Code reads to find this window's server. It lives in
//! `~/.claude/ide/<port>.lock` (or under `$CLAUDE_CONFIG_DIR/ide`) and names the process,
//! the open folders, the transport and the token a client must present.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The name Claude Code shows for this IDE.
pub const IDE_NAME: &str = "SnoutCode";

/// Overrides the lock file folder, so tests and scratch runs never write into the
/// user's `~/.claude`.
pub const DIR_ENV: &str = "SNOUT_CLAUDE_IDE_DIR";

/// Where lock files go: the override, else `$CLAUDE_CONFIG_DIR/ide`, else `~/.claude/ide`.
pub fn default_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(DIR_ENV) {
        return Some(PathBuf::from(dir));
    }
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("ide"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude").join("ide"))
}

/// A fresh token: 128 bits from the operating system's generator, as 32 lowercase hex
/// digits.
pub fn new_token() -> std::io::Result<String> {
    use std::io::Read as _;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// What the lock file says.
pub fn contents(pid: u32, folders: &[PathBuf], token: &str) -> Value {
    let folders: Vec<String> = folders.iter().map(|f| f.to_string_lossy().into_owned()).collect();
    json!({
        "pid": pid,
        "workspaceFolders": folders,
        "ideName": IDE_NAME,
        "transport": "ws",
        "authToken": token,
    })
}

/// The lock file of `port` in `dir`.
pub fn path(dir: &Path, port: u16) -> PathBuf {
    dir.join(format!("{port}.lock"))
}

/// Write the lock file of `port`, readable by this user only, through a temporary file
/// so a reader never sees half of it.
pub fn write(dir: &Path, port: u16, folders: &[PathBuf], token: &str) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let target = path(dir, port);
    let tmp = dir.join(format!("{port}.lock.tmp.{}", std::process::id()));
    let text = serde_json::to_string(&contents(std::process::id(), folders, token)).map_err(std::io::Error::other)?;
    {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

/// Whether process `pid` still runs.
pub fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
    if pid <= 0 {
        return false;
    }
    // Signal 0 only asks; EPERM means it runs under another user.
    let r = unsafe { libc::kill(pid, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Remove the lock files in `dir` whose process has gone. Returns how many went.
pub fn clean_stale(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "lock") {
            continue;
        }
        let pid = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("pid").and_then(Value::as_u64));
        // A file that cannot be read as a lock is left alone: it is not ours to judge.
        if let Some(pid) = pid
            && !alive(u32::try_from(pid).unwrap_or(0))
            && std::fs::remove_file(&path).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snout-ide-lock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn tokens_are_32_hex_digits_and_differ() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn the_lock_file_names_the_process_folders_and_token() {
        let dir = scratch("write");
        let path = write(&dir, 41234, &[PathBuf::from("/tmp/project")], "00ff").unwrap();
        assert_eq!(path, dir.join("41234.lock"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["pid"], std::process::id());
        assert_eq!(v["workspaceFolders"], json!(["/tmp/project"]));
        assert_eq!(v["ideName"], IDE_NAME);
        assert_eq!(v["transport"], "ws");
        assert_eq!(v["authToken"], "00ff");
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stale_lock_files_of_dead_processes_are_removed() {
        let dir = scratch("stale");
        std::fs::create_dir_all(&dir).unwrap();
        // A child that has exited and been reaped: its pid is free.
        let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
        let dead = child.id();
        child.wait().unwrap();
        std::fs::write(dir.join("1.lock"), contents(dead, &[], "t").to_string()).unwrap();
        std::fs::write(dir.join("2.lock"), contents(std::process::id(), &[], "t").to_string()).unwrap();
        std::fs::write(dir.join("3.lock"), "not json").unwrap();
        assert_eq!(clean_stale(&dir), 1);
        assert!(!dir.join("1.lock").exists());
        assert!(dir.join("2.lock").exists() && dir.join("3.lock").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
