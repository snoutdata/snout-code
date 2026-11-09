//! Files and folders the system hands to the app: from Open With, a drop on the Dock icon, the
//! Services menu or the command line.

use std::path::{Path, PathBuf};

use crate::model::split_line_suffix;

/// The local path a `file://` URL names, as the system sends them: percent-encoded, a folder
/// with a slash at its end.
pub fn path_of_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let hex = |c: u8| (c as char).to_digit(16);
    let b = rest.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let (Some(hi), Some(lo)) = (b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex))
        {
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    let text = String::from_utf8(out).ok()?;
    let path = text.strip_suffix('/').filter(|t| !t.is_empty()).unwrap_or(&text);
    Some(PathBuf::from(path))
}

/// The file `arg` names on this machine, with any `:line:column` taken off; `None` for a folder
/// or something that is not here.
fn local_file(arg: &Path) -> Option<PathBuf> {
    if arg.exists() {
        return arg.is_file().then(|| arg.to_owned());
    }
    split_line_suffix(&arg.to_string_lossy()).map(|(path, _)| path).filter(|path| path.is_file())
}

/// A start on `open` shows these files on their own, with no project behind them: several
/// files, or one that is in no repository. Then a quick look at a text file starts no engine
/// and brings back no project.
pub fn opens_files_alone(open: &[PathBuf]) -> bool {
    let Some(files) = open.iter().map(|p| local_file(p)).collect::<Option<Vec<_>>>() else {
        return false;
    };
    match files.as_slice() {
        [] => false,
        [file] => !file.ancestors().skip(1).any(|dir| dir.join(".git").exists()),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_becomes_its_path() {
        assert_eq!(path_of_url("file:///tmp/My%20Notes/a%2Bb.txt"), Some("/tmp/My Notes/a+b.txt".into()));
        assert_eq!(path_of_url("file://localhost/tmp/a.txt"), Some("/tmp/a.txt".into()));
        assert_eq!(path_of_url("file:///tmp/project/"), Some("/tmp/project".into()));
        assert_eq!(path_of_url("file:///"), Some("/".into()));
        assert_eq!(path_of_url("file:///tmp/caf%C3%A9.txt"), Some("/tmp/café.txt".into()));
        assert_eq!(path_of_url("file:///tmp/100%.txt"), Some("/tmp/100%.txt".into()));
        assert_eq!(path_of_url("https://example.com/a"), None);
    }

    #[test]
    fn files_open_alone_unless_a_single_one_has_a_repository() {
        let root = std::env::temp_dir().join(format!("snout-system-open-{}", std::process::id()));
        let repo = root.join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let plain = root.join("notes.txt");
        let tracked = repo.join("a.txt");
        std::fs::write(&plain, "x").unwrap();
        std::fs::write(&tracked, "x").unwrap();

        assert!(opens_files_alone(std::slice::from_ref(&plain)));
        assert!(opens_files_alone(&[PathBuf::from(format!("{}:3:1", plain.display()))]));
        assert!(!opens_files_alone(std::slice::from_ref(&tracked)));
        assert!(opens_files_alone(&[plain.clone(), tracked.clone()]));
        assert!(!opens_files_alone(std::slice::from_ref(&repo)));
        assert!(!opens_files_alone(&[plain.clone(), repo.clone()]));
        assert!(!opens_files_alone(&[root.join("gone.txt")]));
        assert!(!opens_files_alone(&[]));
        let _ = std::fs::remove_dir_all(root);
    }
}
