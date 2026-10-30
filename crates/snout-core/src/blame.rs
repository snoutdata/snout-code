//! Who last changed each line of a file: `git blame --porcelain` at a commit, parsed, with
//! each commit's whole message; and the blamed lines carried over to a text that has changed
//! since (the editor's), so lines changed after the commit read as not committed yet.
//!
//! Everything runs `git` on a [`Host`], so it works the same on a remote machine.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};
use snout_host::Host;

use crate::git::{run_raw, run_with_stdin};
use crate::{Error, Result};

/// The id Git gives lines that are not committed yet.
pub const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

/// The most commits whose whole messages are read for one file.
const MESSAGES_MAX: usize = 4000;

/// A commit that last changed some lines of the file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BlameCommit {
    pub id: String,
    pub author: String,
    /// Without the angle brackets.
    pub email: String,
    /// When it was written (the author's time), in seconds since 1970.
    pub time: i64,
    /// The first line of its message.
    pub summary: String,
    /// Its whole message, once read ([`blame_file`] reads them); else empty.
    pub message: String,
    /// The file's path in that commit (the file may have been renamed since).
    pub path: String,
    /// The commit before it that had the file, and the file's path there: where to annotate
    /// the previous revision of these lines. None for the commit that brought the file in.
    pub previous: Option<(String, String)>,
}

impl BlameCommit {
    /// The id, shortened as Git does.
    pub fn short_id(&self) -> &str {
        &self.id[..self.id.len().min(8)]
    }

    /// The message, or the summary while the message has not been read.
    pub fn full_message(&self) -> &str {
        if self.message.trim().is_empty() { &self.summary } else { self.message.trim_end() }
    }
}

/// A file as a commit has it, and for each of its lines the commit that last changed it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Blame {
    /// The commit the file was annotated at (empty when there is no commit yet).
    pub rev: String,
    /// The file's path there, relative to the repository's root.
    pub path: String,
    pub commits: Vec<BlameCommit>,
    /// For each line, its commit (an index into `commits`).
    pub lines: Vec<u32>,
    /// For each line, its number (from 0) in the commit that last changed it.
    pub orig: Vec<u32>,
    /// The file's lines as blamed, without their line endings.
    pub text: Vec<String>,
}

impl Blame {
    /// The commit of `line`, if the file has that line.
    pub fn commit_of(&self, line: usize) -> Option<&BlameCommit> {
        self.lines.get(line).and_then(|&c| self.commits.get(c as usize))
    }

    /// The commit with `id`.
    pub fn commit(&self, id: &str) -> Option<&BlameCommit> {
        self.commits.iter().find(|c| c.id == id)
    }
}

/// Parse `git blame --porcelain` output. A commit's details come with its first lines only;
/// the lines after point back to it by id.
pub fn parse_porcelain(out: &str) -> Blame {
    let mut b = Blame::default();
    let mut index: std::collections::HashMap<String, u32> = Default::default();
    let mut current: Option<u32> = None;
    let mut orig = 0u32;
    for line in out.split('\n') {
        if let Some(content) = line.strip_prefix('\t') {
            if let Some(c) = current {
                b.lines.push(c);
                b.orig.push(orig);
                b.text.push(content.strip_suffix('\r').unwrap_or(content).to_owned());
            }
            continue;
        }
        let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
        if key.len() == 40 && key.bytes().all(|c| c.is_ascii_hexdigit()) {
            let i = *index.entry(key.to_owned()).or_insert_with(|| {
                b.commits.push(BlameCommit { id: key.to_owned(), ..Default::default() });
                (b.commits.len() - 1) as u32
            });
            current = Some(i);
            orig = rest.split(' ').next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(1).saturating_sub(1);
            continue;
        }
        let Some(c) = current.and_then(|i| b.commits.get_mut(i as usize)) else { continue };
        match key {
            "author" => c.author = rest.to_owned(),
            "author-mail" => c.email = rest.trim_start_matches('<').trim_end_matches('>').to_owned(),
            "author-time" => c.time = rest.parse().unwrap_or(0),
            "summary" => c.summary = rest.to_owned(),
            "filename" => c.path = rest.to_owned(),
            "previous" => {
                if let Some((id, path)) = rest.split_once(' ') {
                    c.previous = Some((id.to_owned(), path.to_owned()));
                }
            }
            _ => {}
        }
    }
    b
}

/// The commit HEAD is at, or None before the first commit (or outside a repository).
pub fn head(host: &dyn Host, root: &Path) -> Option<String> {
    let out = run_raw(host, root, &["rev-parse", "--verify", "-q", "HEAD"], None).ok()?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.success && !id.is_empty()).then_some(id)
}

/// Annotate `path` (relative to `root`, `/`-separated) as commit `rev` has it, with each
/// commit's whole message. A file the commit does not have is an error.
pub fn blame_file(host: &dyn Host, root: &Path, rev: &str, path: &str) -> Result<Blame> {
    let out = run_raw(host, root, &["blame", "--porcelain", rev, "--", path], None)?;
    if !out.success {
        return Err(Error::Git { args: format!("blame {rev} -- {path}"), message: out.text() });
    }
    let mut blame = parse_porcelain(&String::from_utf8_lossy(&out.stdout));
    blame.rev = rev.to_owned();
    blame.path = path.to_owned();
    read_messages(host, root, &mut blame);
    Ok(blame)
}

/// Fill in the commits' whole messages, in one `git log` (best effort: a failure leaves the
/// summaries).
fn read_messages(host: &dyn Host, root: &Path, blame: &mut Blame) {
    let ids: Vec<&str> =
        blame.commits.iter().map(|c| c.id.as_str()).filter(|id| *id != UNCOMMITTED).take(MESSAGES_MAX).collect();
    if ids.is_empty() {
        return;
    }
    let stdin = ids.join("\n") + "\n";
    let args = ["log", "--no-walk=unsorted", "--stdin", "--format=%H%x00%B%x1e"];
    let Ok(out) = run_with_stdin(host, root, &args, Some(stdin.as_bytes())) else { return };
    for (id, message) in parse_messages(&String::from_utf8_lossy(&out.stdout)) {
        if let Some(c) = blame.commits.iter_mut().find(|c| c.id == id) {
            c.message = message;
        }
    }
}

/// `%H%x00%B%x1e` records: each commit's id and message.
fn parse_messages(out: &str) -> Vec<(String, String)> {
    out.split('\x1e')
        .filter_map(|r| {
            let (id, msg) = r.trim_start_matches('\n').split_once('\0')?;
            Some((id.trim().to_owned(), msg.trim_end().to_owned()))
        })
        .collect()
}

/// For each line of `now`, the line of `blamed` it still is, or None where it changed (or is
/// new). A diff that takes too long is cut short rather than holding things up.
pub fn map_lines(blamed: &[String], now: &str) -> Vec<Option<u32>> {
    let new: Vec<&str> = now.lines().collect();
    if blamed.len() == new.len() && blamed.iter().zip(&new).all(|(a, b)| a == b) {
        return (0..new.len() as u32).map(Some).collect();
    }
    let old: Vec<&str> = blamed.iter().map(String::as_str).collect();
    let deadline = Instant::now() + Duration::from_millis(250);
    let ops = capture_diff_slices_deadline(Algorithm::Myers, &old, &new, Some(deadline));
    let mut out = vec![None; new.len()];
    for op in ops {
        if let DiffOp::Equal { old_index, new_index, len } = op {
            for k in 0..len {
                out[new_index + k] = Some((old_index + k) as u32);
            }
        }
    }
    out
}

/// `map` (one entry per line) after an edit that replaced the lines `from..=to` of the old
/// text with `inserted` lines' worth of new text (`inserted` counts the new line breaks, so 0
/// is an edit within one line). `whole` says the edit took or put whole lines at the start
/// of `from`: the line there keeps its place in history. Lines an edit touches read as
/// changed until the next diff says otherwise.
pub fn shift_lines(map: &[Option<u32>], from: usize, to: usize, inserted: usize, whole: bool) -> Vec<Option<u32>> {
    let len = map.len();
    let from = from.min(len);
    let to = to.max(from);
    let mut out = Vec::with_capacity(len + inserted + 1);
    out.extend_from_slice(&map[..from]);
    if whole {
        // Whole lines taken out (`to - from` of them) or put in (`inserted`), before line
        // `to`, which itself is untouched.
        out.extend(std::iter::repeat_n(None, inserted));
        out.extend_from_slice(&map[to.min(len)..]);
    } else {
        out.extend(std::iter::repeat_n(None, inserted + 1));
        out.extend_from_slice(&map[(to + 1).min(len)..]);
    }
    out
}

/// How long ago `then` was, at `now` (both seconds since 1970), as people say it.
pub fn ago(then: i64, now: i64) -> String {
    let s = (now - then).max(0);
    let (m, h, d) = (s / 60, s / 3600, s / 86_400);
    let plural = |n: i64, unit: &str| if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") };
    match () {
        _ if s < 60 => "just now".to_owned(),
        _ if m < 60 => plural(m, "minute"),
        _ if h < 24 => plural(h, "hour"),
        _ if d < 2 => "yesterday".to_owned(),
        _ if d < 14 => plural(d, "day"),
        _ if d < 60 => plural(d / 7, "week"),
        _ if d < 365 => plural(d / 30, "month"),
        _ => plural(d / 365, "year"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORCELAIN: &str = "\
8f8f3163b510e348521e196b90a53a146017b9ee 1 1 1
author Ada Lovelace
author-mail <ada@example.com>
author-time 1700000000
author-tz +0000
committer Ada Lovelace
committer-mail <ada@example.com>
committer-time 1700000000
committer-tz +0000
summary First
boundary
filename src/Old.java
\ta
c011168066eda3d7d6e81f5f56408dbb48b20a92 2 2 2
author Grace Hopper
author-mail <grace@example.com>
author-time 1710000000
author-tz +0100
committer Grace Hopper
committer-mail <grace@example.com>
committer-time 1710000000
committer-tz +0100
summary Second
previous 8f8f3163b510e348521e196b90a53a146017b9ee src/Old.java
filename src/New.java
\tB\r
c011168066eda3d7d6e81f5f56408dbb48b20a92 3 3
\tc
8f8f3163b510e348521e196b90a53a146017b9ee 2 4 1
filename src/Old.java
\td
";

    #[test]
    fn porcelain_gives_each_line_its_commit_and_each_commit_once() {
        let b = parse_porcelain(PORCELAIN);
        assert_eq!(b.commits.len(), 2);
        assert_eq!(b.lines, [0, 1, 1, 0]);
        assert_eq!(b.orig, [0, 1, 2, 1]);
        assert_eq!(b.text, ["a", "B", "c", "d"]);
        let (ada, grace) = (&b.commits[0], &b.commits[1]);
        assert_eq!(
            (ada.author.as_str(), ada.email.as_str(), ada.time),
            ("Ada Lovelace", "ada@example.com", 1700000000)
        );
        assert_eq!(ada.previous, None, "the first commit of the file");
        assert_eq!(ada.path, "src/Old.java");
        assert_eq!(grace.summary, "Second");
        assert_eq!(grace.short_id(), "c0111680");
        assert_eq!(
            grace.previous,
            Some(("8f8f3163b510e348521e196b90a53a146017b9ee".into(), "src/Old.java".into())),
            "the parent and the file's name there: renames are followed"
        );
        assert_eq!(b.commit_of(2).map(|c| c.author.as_str()), Some("Grace Hopper"));
        assert_eq!(b.commit_of(9), None);
        assert_eq!(parse_porcelain(""), Blame::default());
    }

    #[test]
    fn messages_come_whole() {
        let out = "abc\0Subject\n\nBody line\n\x1e\ndef\0Other\n\x1e\n";
        assert_eq!(
            parse_messages(out),
            [("abc".to_owned(), "Subject\n\nBody line".to_owned()), ("def".to_owned(), "Other".to_owned())]
        );
    }

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_owned).collect()
    }

    #[test]
    fn lines_changed_since_read_as_not_committed() {
        let blamed = lines("a\nb\nc\nd");
        assert_eq!(map_lines(&blamed, "a\nb\nc\nd\n"), [Some(0), Some(1), Some(2), Some(3)]);
        assert_eq!(map_lines(&blamed, "a\nX\nb\nc\nd\n"), [Some(0), None, Some(1), Some(2), Some(3)]);
        assert_eq!(map_lines(&blamed, "a\nc\nd\n"), [Some(0), Some(2), Some(3)]);
        assert_eq!(map_lines(&blamed, "a\nB\nc\nd"), [Some(0), None, Some(2), Some(3)]);
        assert_eq!(map_lines(&[], "new\nfile\n"), [None, None]);
        // Line endings do not count.
        assert_eq!(map_lines(&blamed, "a\r\nb\r\nc\r\nd\r\n"), [Some(0), Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn lines_shift_with_typing() {
        let map: Vec<Option<u32>> = (0..5).map(Some).collect();
        // Typing within line 2.
        assert_eq!(shift_lines(&map, 2, 2, 0, false), [Some(0), Some(1), None, Some(3), Some(4)]);
        // Return pressed in line 1: it and the new line after it changed, the rest move down.
        assert_eq!(shift_lines(&map, 1, 1, 1, false), [Some(0), None, None, Some(2), Some(3), Some(4)]);
        // Two whole lines put in before line 3: line 3 keeps its history.
        assert_eq!(shift_lines(&map, 3, 3, 2, true), [Some(0), Some(1), Some(2), None, None, Some(3), Some(4)]);
        // Lines 1 and 2 taken out whole.
        assert_eq!(shift_lines(&map, 1, 3, 0, true), [Some(0), Some(3), Some(4)]);
        // A selection from line 1 to line 3 typed over with one line.
        assert_eq!(shift_lines(&map, 1, 3, 0, false), [Some(0), None, Some(4)]);
        // Typing at the end of the file.
        assert_eq!(shift_lines(&map, 4, 4, 1, false), [Some(0), Some(1), Some(2), Some(3), None, None]);
        assert_eq!(shift_lines(&[], 0, 0, 0, false), [None]);
    }

    #[test]
    fn ages_read_as_people_say_them() {
        let now = 1_000_000_000;
        assert_eq!(ago(now - 5, now), "just now");
        assert_eq!(ago(now - 60, now), "1 minute ago");
        assert_eq!(ago(now - 3 * 3600, now), "3 hours ago");
        assert_eq!(ago(now - 30 * 3600, now), "yesterday");
        assert_eq!(ago(now - 5 * 86_400, now), "5 days ago");
        assert_eq!(ago(now - 21 * 86_400, now), "3 weeks ago");
        assert_eq!(ago(now - 100 * 86_400, now), "3 months ago");
        assert_eq!(ago(now - 3 * 366 * 86_400, now), "3 years ago");
        assert_eq!(ago(now + 50, now), "just now", "a clock ahead");
    }

    #[test]
    fn blames_a_file_at_a_commit_following_renames() {
        let dir = std::env::temp_dir().join(format!("snout-blame-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let git = |args: &[&str], who: &str| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", &format!("user.name={who}"), "-c", &format!("user.email={who}@example.com")])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "{args:?}");
        };
        let host = snout_host::local();
        git(&["init", "-q"], "T");
        assert_eq!(head(&*host, &dir), None, "no commit yet");
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        git(&["add", "."], "T");
        git(&["commit", "-qm", "First"], "Ada");
        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();
        git(&["commit", "-qam", "Second\n\nWith a body."], "Grace");
        git(&["mv", "a.txt", "b.txt"], "T");
        git(&["commit", "-qm", "Rename"], "Alan");
        let id = head(&*host, &dir).unwrap();
        let b = blame_file(&*host, &dir, &id, "b.txt").unwrap();
        assert_eq!(b.text, ["one", "TWO", "three"]);
        let who: Vec<&str> = (0..3).map(|l| b.commit_of(l).unwrap().author.as_str()).collect();
        assert_eq!(who, ["Ada", "Grace", "Grace"]);
        let grace = b.commit_of(1).unwrap();
        assert_eq!(grace.full_message(), "Second\n\nWith a body.");
        // The previous revision of Grace's lines: the first commit, under the old name.
        let (prev, path) = grace.previous.clone().unwrap();
        assert_eq!(path, "a.txt");
        let before = blame_file(&*host, &dir, &prev, &path).unwrap();
        assert_eq!(before.text, ["one", "two"]);
        assert!(blame_file(&*host, &dir, &id, "missing.txt").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
