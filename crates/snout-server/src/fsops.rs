//! File system requests, on std::fs.

use snout_remote_proto::{DirEntry, FileKind, HostInfo, Metadata};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn kind_of(t: fs::FileType) -> FileKind {
    if t.is_symlink() {
        FileKind::Symlink
    } else if t.is_dir() {
        FileKind::Dir
    } else if t.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    }
}

/// Write `data` to a temporary file beside `path`, then rename it over `path`, keeping the
/// permissions `path` had. A symlink is followed: its target gets the new contents.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let target = match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    let existing = fs::metadata(&target).ok();
    if existing.as_ref().is_some_and(|m| m.is_dir()) {
        return Err(io::Error::new(io::ErrorKind::IsADirectory, "is a folder"));
    }
    let dir = target.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = target.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let tmp = dir.join(format!(
        ".{}.snout-{}-{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(data)?;
        if let Some(m) = &existing {
            f.set_permissions(m.permissions())?;
        }
        f.sync_all()?;
        fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn remove(path: &Path, recursive: bool) -> io::Result<()> {
    let m = fs::symlink_metadata(path)?;
    if m.is_dir() {
        if recursive { fs::remove_dir_all(path) } else { fs::remove_dir(path) }
    } else {
        fs::remove_file(path)
    }
}

pub fn copy(from: &Path, to: &Path, recursive: bool) -> io::Result<()> {
    let m = fs::symlink_metadata(from)?;
    if m.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(from)?, to)
    } else if m.is_dir() {
        if !recursive {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "is a folder (copy it recursively)"));
        }
        if to.starts_with(from) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "cannot copy a folder into itself"));
        }
        fs::create_dir(to)?;
        fs::set_permissions(to, m.permissions())?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()), true)?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

pub fn metadata(path: &Path, follow_symlinks: bool) -> io::Result<Metadata> {
    let m = if follow_symlinks { fs::metadata(path)? } else { fs::symlink_metadata(path)? };
    Ok(Metadata {
        kind: kind_of(m.file_type()),
        len: m.len(),
        mode: m.permissions().mode(),
        readonly: m.permissions().readonly(),
        modified: m.modified().ok(),
        accessed: m.accessed().ok(),
        created: m.created().ok(),
    })
}

pub fn read_dir(path: &Path) -> io::Result<Vec<DirEntry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type().map(kind_of).unwrap_or(FileKind::Other);
        out.push(DirEntry { name: entry.file_name().to_string_lossy().into_owned(), kind });
    }
    Ok(out)
}

fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer and its length.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return String::new();
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

fn user() -> String {
    if let Some(u) = std::env::var_os("USER").or_else(|| std::env::var_os("LOGNAME")) {
        return u.to_string_lossy().into_owned();
    }
    // SAFETY: getpwuid returns a static record or null.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_name.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
    }
}

pub fn info() -> HostInfo {
    HostInfo {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        home: std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default(),
        temp_dir: std::env::temp_dir(),
        user: user(),
        hostname: hostname(),
        env: std::env::vars().collect(),
        server_version: snout_remote_proto::VERSION.into(),
        server_pid: std::process::id(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_writes_replace_and_keep_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        write_atomic(&f, b"one").unwrap();
        fs::set_permissions(&f, fs::Permissions::from_mode(0o741)).unwrap();
        write_atomic(&f, b"two").unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"two");
        assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o741);
        // Through a symlink: the target changes, the link stays.
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&f, &link).unwrap();
        write_atomic(&link, b"three").unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(&f).unwrap(), b"three");
        // Nothing left behind.
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        assert_eq!(write_atomic(dir.path(), b"x").unwrap_err().kind(), io::ErrorKind::IsADirectory);
    }

    #[test]
    fn copies_and_removes_folders() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        fs::create_dir_all(a.join("b/c")).unwrap();
        fs::write(a.join("b/c/f"), "x").unwrap();
        std::os::unix::fs::symlink("c/f", a.join("b/l")).unwrap();
        assert_eq!(copy(&a, &dir.path().join("z"), false).unwrap_err().kind(), io::ErrorKind::IsADirectory);
        assert!(copy(&a, &a.join("b/inside"), true).is_err());
        copy(&a, &dir.path().join("z"), true).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("z/b/l")).unwrap(), "x");
        assert!(remove(&a, false).is_err());
        remove(&a, true).unwrap();
        assert!(!a.exists());
        let mut names: Vec<_> = read_dir(dir.path()).unwrap().into_iter().map(|e| (e.name, e.kind)).collect();
        names.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(names, [("z".to_owned(), FileKind::Dir)]);
    }
}
