//! Reading the feed and downloading, with curl, as the app's other downloads do. It runs on the
//! machine the app runs on, whatever host the workspace is on.

use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest as _, Sha256};

use crate::failure::{Failure, describe};

fn curl(url: &str) -> Command {
    let mut c = crate::command("curl");
    c.args(["-sSL", "-H", "Cache-Control: no-cache", "-H", "Pragma: no-cache"]);
    // Only what the feed is: https, and its redirects. (A feed named for a test can be local.)
    if url.starts_with("https://") {
        c.args(["--proto", "=https", "--proto-redir", "=https"]);
    }
    c
}

/// The body at `url`: a status of its own is a failure, so a proxy's page is never read as the feed.
pub fn text(url: &str) -> Result<Vec<u8>, Failure> {
    let out = curl(url)
        .args(["--max-time", "30", "-w", "\n%{http_code}"])
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| describe(&format!("curl did not start: {e}"), None))?;
    if !out.status.success() {
        return Err(describe(&String::from_utf8_lossy(&out.stderr), None));
    }
    let (body, code) = split_status(&out.stdout);
    match code {
        // file:// has no status.
        0 | 200 => Ok(body.to_vec()),
        code => Err(describe(&format!("HTTP {code} for {url}"), Some(code))),
    }
}

/// curl's `-w "\n%{http_code}"`, off the end of its output.
fn split_status(out: &[u8]) -> (&[u8], u16) {
    match out.iter().rposition(|b| *b == b'\n') {
        Some(at) => (&out[..at], String::from_utf8_lossy(&out[at + 1..]).trim().parse().unwrap_or(0)),
        None => (out, 0),
    }
}

/// Download `url` into `out`, calling `progress` with the bytes so far. Stopped by `cancel` (the
/// app is quitting), and by a connection that has stalled for a minute.
pub fn download(url: &str, out: &Path, cancel: &AtomicBool, mut progress: impl FnMut(u64)) -> Result<(), Failure> {
    let mut child = curl(url)
        .args(["--fail", "--retry", "2", "--speed-limit", "1024", "--speed-time", "60", "-o"])
        .arg(out)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| describe(&format!("curl did not start: {e}"), None))?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(describe(&format!("curl: {e}"), None)),
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(describe("the download was stopped: the app is quitting", None));
        }
        if let Ok(m) = std::fs::metadata(out) {
            progress(m.len());
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if status.success() {
        return Ok(());
    }
    let mut why = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut why);
    }
    let code = http_status_in(&why);
    Err(describe(&why, code))
}

/// `--fail` says `curl: (22) The requested URL returned error: 404`.
fn http_status_in(text: &str) -> Option<u16> {
    text.split("returned error:").nth(1)?.split_whitespace().next()?.parse().ok()
}

/// The SHA-256 of a file, in hex.
pub fn sha256(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        match file.read(&mut buf)? {
            0 => return Ok(crate::manifest::to_hex(&hash.finalize())),
            n => hash.update(&buf[..n]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_comes_off_the_end() {
        assert_eq!(split_status(b"{\"a\":1}\n200"), (&b"{\"a\":1}"[..], 200));
        assert_eq!(split_status(b"line\nline\n404"), (&b"line\nline"[..], 404));
        assert_eq!(split_status(b"\n000"), (&b""[..], 0));
        assert_eq!(http_status_in("curl: (22) The requested URL returned error: 404\n"), Some(404));
        assert_eq!(http_status_in("curl: (6) Could not resolve host"), None);
    }

    #[test]
    fn a_local_file_reads_and_downloads_with_its_progress() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.bin");
        std::fs::write(&src, b"hello").unwrap();
        let url = format!("file://{}", src.display());
        assert_eq!(text(&url).unwrap(), b"hello");
        let out = dir.path().join("b.bin");
        download(&url, &out, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"hello");
        assert_eq!(sha256(&out).unwrap(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assert!(text(&format!("file://{}", dir.path().join("none").display())).is_err());
        assert!(
            download(&format!("file://{}", dir.path().join("none").display()), &out, &AtomicBool::new(false), |_| {})
                .is_err()
        );
    }
}
