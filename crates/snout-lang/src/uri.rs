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

/// The `file:` URI of an absolute path.
pub fn from_path(path: &Path) -> Uri {
    let mut s = String::from("file://");
    for &b in path.to_string_lossy().as_bytes() {
        if keep(b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    Uri::from_str(&s).unwrap_or_else(|_| Uri::from_str("file:///").expect("a valid URI"))
}

/// The path a `file:` URI names; `None` for other schemes.
pub fn to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    let rest = s.strip_prefix("file://")?;
    // `file://host/path` is not ours; `file:///path` is.
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    if !path.starts_with('/') {
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
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
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
}
