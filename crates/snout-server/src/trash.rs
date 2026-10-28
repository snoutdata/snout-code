//! The freedesktop.org trash (`$XDG_DATA_HOME/Trash`, else `~/.local/share/Trash`): what
//! desktop file managers on Linux show and restore from.

use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

fn trash_dir() -> io::Result<PathBuf> {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(d).join("Trash"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(".local/share/Trash"))
}

fn percent_encode(path: &Path) -> String {
    let mut out = String::new();
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn local_time_now() -> String {
    // SAFETY: time and localtime_r with valid pointers.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}

pub fn trash(path: &Path) -> io::Result<()> {
    let path = std::path::absolute(path)?;
    fs::symlink_metadata(&path)?;
    let trash = trash_dir()?;
    let (files, info) = (trash.join("files"), trash.join("info"));
    fs::create_dir_all(&files)?;
    fs::create_dir_all(&info)?;
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let name = name.to_string_lossy();
    // Claim a name by creating its info file: two trashings at once cannot take the same.
    let (slot, mut info_file) = (1..10_000)
        .find_map(|n| {
            let slot = if n == 1 { name.to_string() } else { format!("{name}.{n}") };
            if files.join(&slot).exists() {
                return None;
            }
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(info.join(format!("{slot}.trashinfo")))
                .ok()
                .map(|f| (slot, f))
        })
        .ok_or_else(|| io::Error::other("the trash has no free name for this file"))?;
    let info_path = info.join(format!("{slot}.trashinfo"));
    let written =
        write!(info_file, "[Trash Info]\nPath={}\nDeletionDate={}\n", percent_encode(&path), local_time_now());
    let moved = written.and_then(|_| match fs::rename(&path, files.join(&slot)) {
        // Another file system: copy, then remove the original.
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            crate::fsops::copy(&path, &files.join(&slot), true)?;
            crate::fsops::remove(&path, true)
        }
        r => r,
    });
    if moved.is_err() {
        let _ = fs::remove_file(&info_path);
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_percent_encoded() {
        assert_eq!(percent_encode(Path::new("/home/a b/ü.txt")), "/home/a%20b/%C3%BC.txt");
    }
}
