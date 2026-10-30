//! Moving files to the Trash of this machine.
//!
//! On a Mac, with macOS's own `/usr/bin/trash` (macOS 15 and later). On Linux, the desktop's
//! Trash as the freedesktop.org specification has it: through `gio trash` in a desktop
//! session (it handles the Trash folders of other file systems too), else trash-cli's `trash`,
//! else directly into the home Trash (`$XDG_DATA_HOME/Trash`) for files on the same file
//! system as it. A machine with no desktop session and no trash command (a server reached over
//! SSH) has no Trash: deleting there is for good, and the app says so before it does. On
//! Windows, the Recycle Bin (the shell's own file operation, as Explorer's Delete).

use std::io;
use std::path::{Path, PathBuf};

/// trash-cli's command on Linux, and macOS's own.
const TRASH_COMMAND: &str = "/usr/bin/trash";

/// How files go to the Trash here, if they can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// A command that takes the paths as arguments (`/usr/bin/trash`, `gio trash`).
    Command(Vec<String>),
    /// Into the home Trash, by the specification.
    HomeTrash(PathBuf),
    /// Windows's Recycle Bin.
    RecycleBin,
}

/// How this machine trashes files, if it can.
pub fn method(env: &dyn Fn(&str) -> Option<String>, home: &Path) -> Option<Method> {
    if cfg!(windows) {
        return Some(Method::RecycleBin);
    }
    if cfg!(target_os = "macos") {
        return Path::new(TRASH_COMMAND).exists().then(|| Method::Command(vec![TRASH_COMMAND.to_owned()]));
    }
    let desktop = ["WAYLAND_DISPLAY", "DISPLAY"].iter().any(|v| env(v).is_some_and(|s| !s.is_empty()));
    let on_path =
        |name: &str| env("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file()));
    if desktop && let Some(gio) = on_path("gio") {
        return Some(Method::Command(vec![gio.display().to_string(), "trash".to_owned(), "--".to_owned()]));
    }
    if Path::new(TRASH_COMMAND).exists() {
        return Some(Method::Command(vec![TRASH_COMMAND.to_owned(), "--".to_owned()]));
    }
    if desktop && !home.as_os_str().is_empty() {
        let data = env("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute());
        return Some(Method::HomeTrash(data.unwrap_or_else(|| home.join(".local/share")).join("Trash")));
    }
    None
}

/// Move `paths` to the Trash by `method`.
pub fn trash(method: &Method, paths: &[PathBuf]) -> io::Result<()> {
    match method {
        Method::Command(argv) => {
            let (program, args) = argv.split_first().ok_or_else(|| io::Error::other("no trash command"))?;
            let out =
                crate::default_signals(&mut std::process::Command::new(program)).args(args).args(paths).output()?;
            if out.status.success() {
                Ok(())
            } else {
                Err(io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
            }
        }
        Method::HomeTrash(dir) => paths.iter().try_for_each(|p| into_home_trash(dir, p)),
        #[cfg(windows)]
        Method::RecycleBin => crate::win::recycle(paths),
        #[cfg(not(windows))]
        Method::RecycleBin => Err(io::Error::new(io::ErrorKind::Unsupported, "no Recycle Bin here")),
    }
}

/// Move `path` into the home Trash at `trash`: the file into `files/`, a `.trashinfo` naming
/// where it was and when into `info/`. Only a rename: a file on another file system fails.
pub fn into_home_trash(trash: &Path, path: &Path) -> io::Result<()> {
    let files = trash.join("files");
    let info = trash.join("info");
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let path = std::path::absolute(path)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("no file name"))?.to_string_lossy().into_owned();
    // A name not yet taken: the info file is made first, exclusively, as the specification asks.
    let mut n = 1;
    let (taken, info_file) = loop {
        let candidate = if n == 1 { name.clone() } else { format!("{name}.{n}") };
        let info_file = info.join(format!("{candidate}.trashinfo"));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&info_file) {
            Ok(mut f) => {
                use std::io::Write as _;
                writeln!(f, "[Trash Info]\nPath={}\nDeletionDate={}", url_path(&path), now_local())?;
                break (candidate, info_file);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && n < 10_000 => n += 1,
            Err(e) => return Err(e),
        }
    };
    if let Err(e) = std::fs::rename(&path, files.join(&taken)) {
        let _ = std::fs::remove_file(&info_file);
        return Err(e);
    }
    Ok(())
}

/// A path as the `.trashinfo` file writes it: percent-encoded, slashes kept.
fn url_path(path: &Path) -> String {
    let mut out = String::new();
    for &b in path.as_os_str().as_encoded_bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The local time now, as `YYYY-MM-DDThh:mm:ss`.
fn now_local() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let [y, mo, d, h, mi, s] = crate::local_time(now as i64).unwrap_or([1970, 1, 1, 0, 0, 0]);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn the_recycle_bin_takes_files_and_folders() {
        let dir = std::env::temp_dir().join(format!("snout-recycle-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a b.txt"), "x").unwrap();
        let m = method(&|_| None, Path::new("")).unwrap();
        trash(&m, &[dir.join("a b.txt"), dir.join("sub")]).unwrap();
        assert!(!dir.join("a b.txt").exists() && !dir.join("sub").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn the_home_trash_keeps_the_file_and_where_it_was() {
        let dir = std::env::temp_dir().join(format!("snout-trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let trash = dir.join("Trash");
        std::fs::create_dir_all(dir.join("w")).unwrap();
        for _ in 0..2 {
            std::fs::write(dir.join("w/a b.txt"), "x").unwrap();
            into_home_trash(&trash, &dir.join("w/a b.txt")).unwrap();
        }
        assert!(!dir.join("w/a b.txt").exists());
        assert!(trash.join("files/a b.txt").is_file() && trash.join("files/a b.txt.2").is_file());
        let info = std::fs::read_to_string(trash.join("info/a b.txt.trashinfo")).unwrap();
        assert!(info.starts_with("[Trash Info]\nPath=/"), "{info}");
        assert!(info.contains("/w/a%20b.txt\nDeletionDate=2"), "{info}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_desktop_or_a_command_there_is_no_trash() {
        if cfg!(any(target_os = "macos", windows)) || Path::new(TRASH_COMMAND).exists() {
            return;
        }
        assert_eq!(method(&|_| None, Path::new("/home/ann")), None);
        let desktop = |v: &str| (v == "DISPLAY").then(|| ":0".to_owned());
        assert_eq!(
            method(&desktop, Path::new("/home/ann")),
            Some(Method::HomeTrash(PathBuf::from("/home/ann/.local/share/Trash")))
        );
    }
}
