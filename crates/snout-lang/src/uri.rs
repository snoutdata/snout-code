//! `file:` URIs for paths and back, percent-encoded the way language servers write them.

use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use lsp_types::Uri;

fn keep(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'/'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b'@'
        )
}

/// The `file:` URI of an absolute path. A Windows path is written as language servers write
/// them: `C:\\x\\A.java` as `file:///C:/x/A.java`, `\\\\server\\share\\x` as
/// `file://server/share/x`.
pub fn from_path(path: &Path) -> Uri {
    let text = path.to_string_lossy();
    // On Windows a path may join with `\\` (a remote Unix path too, joined here).
    let text = if cfg!(windows) { text.replace('\\', "/") } else { text.into_owned() };
    let mut s = String::from("file://");
    let mut rest = text.as_str();
    if let Some(unc) = rest.strip_prefix("//").filter(|_| cfg!(windows)) {
        // The server is the URI's host.
        let (host, path) = unc.split_once('/').unwrap_or((unc, ""));
        s.push_str(host);
        s.push('/');
        rest = path;
    } else if is_drive(rest.as_bytes()) {
        s.push('/');
        s.push_str(&rest[..2]);
        rest = &rest[2..];
    }
    for &b in rest.as_bytes() {
        if keep(b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    Uri::from_str(&s).unwrap_or_else(|_| Uri::from_str("file:///").expect("a valid URI"))
}

/// Whether `b` starts with a Windows drive: a letter and a colon (`C:`).
fn is_drive(b: &[u8]) -> bool {
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/' || b[2] == b'\\')
}

/// The path a `file:` URI names; `None` for other schemes. `file:///C:/x` (or `c%3A`) is the
/// Windows path `C:\\x`; on Windows `file://server/share` is `\\\\server\\share`.
pub fn to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    let rest = s.strip_prefix("file://")?;
    // `file://host/path` is not ours (but for a Windows share); `file:///path` is.
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    let unc = !path.starts_with('/');
    if unc && !cfg!(windows) {
        return None;
    }
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&path[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    let text = String::from_utf8_lossy(&out).into_owned();
    if unc {
        return Some(PathBuf::from(format!("//{text}").replace('/', "\\")));
    }
    if is_drive(&out[1..]) {
        // `/c:/x` is `C:\\x`.
        let mut drive = text[1..].to_owned();
        drive[..1].make_ascii_uppercase();
        return Some(PathBuf::from(if cfg!(windows) { drive.replace('/', "\\") } else { drive }));
    }
    Some(PathBuf::from(text))
}

/// The URI as text.
pub fn text(uri: &Uri) -> &str {
    uri.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_paths_with_spaces_and_unicode() {
        for p in ["/Users/a/My Project/Ça.java", "/tmp/x#y%z.java", "/a/b"] {
            let uri = from_path(Path::new(p));
            assert!(uri.as_str().starts_with("file:///"), "{}", uri.as_str());
            assert_eq!(to_path(&uri).unwrap(), PathBuf::from(p));
        }
        assert_eq!(from_path(Path::new("/a b")).as_str(), "file:///a%20b");
        let jdt = Uri::from_str("jdt://contents/rt.jar/java.lang/String.class").unwrap();
        assert_eq!(to_path(&jdt), None);
        // Servers may encode what we leave alone, or leave a drive-style colon.
        assert_eq!(to_path(&Uri::from_str("file:///a%2Bb/c").unwrap()).unwrap(), PathBuf::from("/a+b/c"));
    }

    #[test]
    fn windows_drives_are_written_as_language_servers_write_them() {
        let lower = Uri::from_str("file:///c%3A/Users/a/My%20Project/A.java").unwrap();
        let upper = Uri::from_str("file:///C:/Users/a/My%20Project/A.java").unwrap();
        let want = if cfg!(windows) { r"C:\Users\a\My Project\A.java" } else { "C:/Users/a/My Project/A.java" };
        assert_eq!(to_path(&lower).unwrap(), PathBuf::from(want));
        assert_eq!(to_path(&upper).unwrap(), PathBuf::from(want));
        assert_eq!(from_path(Path::new(want)).as_str(), "file:///C:/Users/a/My%20Project/A.java");
    }

    #[cfg(windows)]
    #[test]
    fn windows_shares_and_remote_paths_round_trip() {
        let share = Path::new(r"\\srv\share\p\A.java");
        let uri = from_path(share);
        assert_eq!(uri.as_str(), "file://srv/share/p/A.java");
        assert_eq!(to_path(&uri).unwrap(), share);
        // A remote machine's path, joined on Windows.
        let remote = Path::new("/home/a/p").join("A.java");
        assert_eq!(from_path(&remote).as_str(), "file:///home/a/p/A.java");
    }
}
