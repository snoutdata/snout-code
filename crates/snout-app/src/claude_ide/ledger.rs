//! Who wrote which lines this session, without any UI: each edit an agent made (which
//! agent, when, in which turn, from which prompt), and for each file the edit behind each
//! of its lines. Lines move with the edits that come after (an agent's or the user's): the
//! lines an edit changes take its record, the lines a person changes lose theirs, the rest
//! keep what they had. Reviewing a change keeps the record; only reverting it (or typing
//! over it) takes it away.
//!
//! The ledger is kept per project between runs when the app has a folder of its own: a
//! file's lines come back only if the file still holds the text they were recorded for.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::review;

/// One edit an agent made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditRecord {
    /// The agent ([`crate::agents::CLAUDE`] and the others), by id.
    pub agent: String,
    /// When, in seconds since 1970.
    pub at: u64,
    /// The turn it came in, and the first line of the prompt that started it, if known.
    pub turn: u32,
    pub prompt: Option<String>,
    /// The tool that made it (`Edit`, `Write`, `apply_patch`), if known.
    pub tool: String,
}

impl EditRecord {
    pub fn now(agent: &str, turn: u32, prompt: Option<String>, tool: &str) -> Self {
        let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs();
        Self { agent: agent.to_owned(), at, turn, prompt, tool: tool.to_owned() }
    }

    pub fn time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.at)
    }
}

/// A file's lines and the edit behind each (an index into [`Ledger::edits`]), for the text
/// they were worked out on.
#[derive(Clone, Debug, Default)]
pub struct Owners {
    pub text: Arc<String>,
    pub lines: Arc<Vec<Option<u32>>>,
}

impl Owners {
    /// The edit behind `line`, if an agent wrote it.
    pub fn at(&self, line: usize) -> Option<u32> {
        self.lines.get(line).copied().flatten()
    }

    /// The runs of lines written by one edit: their lines and the edit.
    pub fn runs(&self) -> Vec<(Range<usize>, u32)> {
        let mut out: Vec<(Range<usize>, u32)> = Vec::new();
        for (i, o) in self.lines.iter().enumerate() {
            let Some(id) = *o else { continue };
            match out.last_mut() {
                Some((r, last)) if *last == id && r.end == i => r.end = i + 1,
                _ => out.push((i..i + 1, id)),
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(Option::is_none)
    }
}

fn line_count(text: &str) -> usize {
    text.split_inclusive('\n').count()
}

/// `lines` (worked out on `old`) carried over to `new`: unchanged lines keep their edit,
/// lines that changed get `fill`.
pub fn carry(old: &str, lines: &[Option<u32>], new: &str, fill: Option<u32>) -> Vec<Option<u32>> {
    let mut out = Vec::with_capacity(line_count(new));
    let (mut o, mut n) = (0usize, 0usize);
    for h in review::hunks(old, new) {
        while n < h.new_lines.start {
            out.push(lines.get(o).copied().flatten());
            o += 1;
            n += 1;
        }
        out.extend(std::iter::repeat_n(fill, h.new_lines.len()));
        n = h.new_lines.end;
        o = h.old_lines.end;
    }
    let total = line_count(new);
    while n < total {
        out.push(lines.get(o).copied().flatten());
        o += 1;
        n += 1;
    }
    out
}

impl Owners {
    /// These lines as they stand in `now` (the user's changes take the records of the lines
    /// they touch away).
    pub fn to(&self, now: &str) -> Owners {
        if self.text.as_str() == now {
            return self.clone();
        }
        Owners { text: Arc::new(now.to_owned()), lines: Arc::new(carry(&self.text, &self.lines, now, None)) }
    }
}

/// The session's record of the agents' edits.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pub edits: Arc<Vec<EditRecord>>,
    pub files: HashMap<PathBuf, Owners>,
}

impl Ledger {
    /// An agent changed `path` from `before` (none: what the ledger last saw, or nothing)
    /// to `after`. Returns the edit's index.
    pub fn record(&mut self, path: &Path, before: Option<&str>, after: &str, edit: EditRecord) -> u32 {
        let id = self.edits.len() as u32;
        Arc::make_mut(&mut self.edits).push(edit);
        let known = self.files.get(path).cloned();
        let start = match (known, before) {
            (Some(o), Some(b)) => o.to(b),
            (Some(o), None) => o,
            (None, Some(b)) => Owners { text: Arc::new(b.to_owned()), lines: Arc::new(vec![None; line_count(b)]) },
            (None, None) => Owners::default(),
        };
        let lines = carry(&start.text, &start.lines, after, Some(id));
        self.files.insert(path.to_owned(), Owners { text: Arc::new(after.to_owned()), lines: Arc::new(lines) });
        id
    }

    /// The file holds `now` (typed, reverted, reloaded): its lines follow.
    pub fn sync(&mut self, path: &Path, now: &str) {
        if let Some(o) = self.files.get_mut(path)
            && o.text.as_str() != now
        {
            *o = o.to(now);
        }
    }

    /// The file's lines as worked out elsewhere (an open editor's, as it was typed in).
    pub fn take(&mut self, path: &Path, owners: Owners) {
        self.files.insert(path.to_owned(), owners);
    }

    pub fn owners(&self, path: &Path) -> Option<&Owners> {
        self.files.get(path).filter(|o| !o.is_empty())
    }

    pub fn edit(&self, id: u32) -> Option<&EditRecord> {
        self.edits.get(id as usize)
    }

    /// The agent behind most of `lines` of `path`.
    pub fn agent_of(&self, path: &Path, lines: Range<usize>) -> Option<&str> {
        let o = self.files.get(path)?;
        let mut count: HashMap<&str, usize> = HashMap::new();
        for line in lines {
            if let Some(e) = o.at(line).and_then(|id| self.edit(id)) {
                *count.entry(e.agent.as_str()).or_default() += 1;
            }
        }
        count.into_iter().max_by_key(|(_, n)| *n).map(|(a, _)| a)
    }

    /// The ledger as it is kept between runs: the edits, and each file's runs with the
    /// digest of the text they belong to.
    pub fn to_saved(&self, root: &Path) -> Saved {
        let files = self
            .files
            .iter()
            .filter(|(p, o)| p.starts_with(root) && !o.is_empty())
            .map(|(p, o)| {
                let runs = o.runs().into_iter().map(|(r, id)| (r.start, r.len(), id)).collect();
                (p.clone(), SavedFile { digest: digest(&o.text), runs })
            })
            .collect();
        Saved { version: 1, edits: (*self.edits).clone(), files }
    }

    /// Take back what was kept: a file's lines only when `read` gives the text they were
    /// recorded for.
    pub fn restore(saved: Saved, read: impl Fn(&Path) -> Option<String>) -> Self {
        let mut ledger = Ledger { edits: Arc::new(saved.edits), files: HashMap::new() };
        for (path, f) in saved.files {
            let Some(text) = read(&path).filter(|t| digest(t) == f.digest) else { continue };
            let mut lines = vec![None; line_count(&text)];
            for (start, len, id) in f.runs {
                for l in lines.iter_mut().skip(start).take(len) {
                    *l = Some(id);
                }
            }
            ledger.files.insert(path, Owners { text: Arc::new(text), lines: Arc::new(lines) });
        }
        ledger
    }
}

/// The ledger as saved: [`Ledger::to_saved`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Saved {
    pub version: u32,
    pub edits: Vec<EditRecord>,
    pub files: HashMap<PathBuf, SavedFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedFile {
    pub digest: String,
    /// Runs of lines: first line, how many, the edit.
    pub runs: Vec<(usize, usize, u32)>,
}

/// `at` as the clock on the wall said it here: "3:39 PM".
pub fn clock(at: SystemTime) -> String {
    let secs = at.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs() as libc::time_t;
    // SAFETY: localtime_r only writes the struct it is given.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = !unsafe { libc::localtime_r(&secs, &mut tm) }.is_null();
    let (h, m) = if ok { (tm.tm_hour, tm.tm_min) } else { (((secs / 3600) % 24) as i32, ((secs / 60) % 60) as i32) };
    let (h12, half) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    format!("{h12}:{m:02} {half}")
}

pub fn digest(text: &str) -> String {
    use sha2::Digest as _;
    let d = sha2::Sha256::digest(text.as_bytes());
    d.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// Where a project's ledger is kept in the app's folder.
pub fn saved_path(dir: &Path, root: &Path) -> PathBuf {
    dir.join("agents").join(format!("{}.json", digest(&root.to_string_lossy())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(agent: &str) -> EditRecord {
        EditRecord::now(agent, 1, Some("do it".into()), "Edit")
    }

    #[test]
    fn lines_keep_their_writer_as_the_file_moves_on() {
        let mut l = Ledger::default();
        let p = Path::new("/p/A.java");
        let v0 = "a\nb\nc\nd\n";
        let v1 = "a\nB\nB2\nc\nd\n";
        let claude = l.record(p, Some(v0), v1, edit("claude"));
        let o = l.owners(p).unwrap();
        assert_eq!(o.runs(), vec![(1..3, claude)]);
        // Codex changes another line; Claude's stay.
        let v2 = "a\nB\nB2\nc\nD\n";
        let codex = l.record(p, None, v2, edit("codex"));
        assert_eq!(l.owners(p).unwrap().runs(), vec![(1..3, claude), (4..5, codex)]);
        // The user adds a line above: the runs move down.
        let v3 = "top\na\nB\nB2\nc\nD\n";
        l.sync(p, v3);
        assert_eq!(l.owners(p).unwrap().runs(), vec![(2..4, claude), (5..6, codex)]);
        // The user rewrites one of Claude's lines: it is theirs now.
        let v4 = "top\na\nmine\nB2\nc\nD\n";
        l.sync(p, v4);
        assert_eq!(l.owners(p).unwrap().runs(), vec![(3..4, claude), (5..6, codex)]);
        assert_eq!(l.agent_of(p, 3..4), Some("claude"));
        assert_eq!(l.edit(codex).unwrap().agent, "codex");
        // An edit reported with the text before it, after the user typed in between.
        let v5 = "top\na\nmine\nB2\nc\nD\nend\n";
        let c2 = l.record(p, Some(v4), v5, edit("claude"));
        assert_eq!(l.owners(p).unwrap().at(6), Some(c2));
    }

    #[test]
    fn the_ledger_comes_back_only_for_files_that_did_not_change() {
        let mut l = Ledger::default();
        let (a, b) = (Path::new("/p/A.java"), Path::new("/p/B.java"));
        l.record(a, Some("x\n"), "x\ny\n", edit("claude"));
        l.record(b, Some("1\n"), "2\n", edit("codex"));
        let saved: Saved = serde_json::from_str(&serde_json::to_string(&l.to_saved(Path::new("/p"))).unwrap()).unwrap();
        let back = Ledger::restore(saved, |p| Some(if p == a { "x\ny\n".to_owned() } else { "changed\n".to_owned() }));
        assert_eq!(back.owners(a).unwrap().runs(), vec![(1..2, 0)]);
        assert!(back.owners(b).is_none(), "B changed since: its lines are not trusted");
        assert_eq!(back.edits.len(), 2);
    }
}
