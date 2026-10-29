//! What Claude Code changed in the session and nobody has reviewed yet, without any UI:
//! per file, the text before Claude's first edit since the last review (the snapshot a
//! revert puts back, or none for a file Claude created) and the text now, with the hunks
//! between them. Accepting a hunk moves it into the snapshot; reverting one moves it back
//! out of the file. A file whose text is its snapshot again leaves the list.
//!
//! The diffs are computed off the UI thread ([`Diffed`]); the set only holds them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::review::{self, Hunk};

/// How a change was seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Claude Code's hooks reported the edit, with the tool.
    Hook,
    /// The file watcher saw it while Claude Code was connected (a `claude` started
    /// elsewhere, without the hooks).
    Watcher,
}

/// A file Claude changed.
#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: PathBuf,
    /// The text before Claude's first edit since the last review; none: Claude made the file.
    pub base: Option<Arc<String>>,
    /// The text now; none: the file is gone.
    pub now: Option<Arc<String>>,
    /// The hunks from `base` to `now`, and what they add and remove, in lines.
    pub hunks: Arc<Vec<Hunk>>,
    pub added: usize,
    pub removed: usize,
    /// The last tool that changed it, and how many edits so far.
    pub tool: String,
    pub edits: u32,
    /// The turn of its first edit.
    pub turn: u32,
    pub source: Source,
    /// The agent that changed it last ([`crate::agents::CLAUDE`] and the others).
    pub agent: &'static str,
}

impl FileChange {
    pub fn created(&self) -> bool {
        self.base.is_none()
    }
}

/// A diff worked out off the UI thread: the hunks between two texts and their counts.
#[derive(Clone, Debug, Default)]
pub struct Diffed {
    pub hunks: Arc<Vec<Hunk>>,
    pub added: usize,
    pub removed: usize,
}

impl Diffed {
    pub fn of(base: Option<&str>, now: Option<&str>) -> Self {
        let hunks = review::hunks(base.unwrap_or_default(), now.unwrap_or_default());
        let added = hunks.iter().map(|h| h.new_lines.len()).sum();
        let removed = hunks.iter().map(|h| h.old_lines.len()).sum();
        Self { hunks: Arc::new(hunks), added, removed }
    }
}

/// A file's snapshot and its text now, after a hunk was accepted.
pub type Texts = (Option<Arc<String>>, Option<Arc<String>>);

/// What to write to put a file back: its text, or none to delete it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restore {
    pub path: PathBuf,
    pub text: Option<String>,
}

/// A turn: one prompt to an agent and what it did about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub id: u32,
    pub agent: &'static str,
    /// The first line of the prompt, once known (Claude's hooks say it as the turn starts,
    /// Codex as it ends; an agent only watched never does).
    pub prompt: Option<String>,
    /// When it began.
    pub at: std::time::SystemTime,
}

#[derive(Default, Debug)]
pub struct ChangeSet {
    /// In the order the agents first changed them.
    pub files: Vec<FileChange>,
    /// Snapshots of files a tool is about to change, until it has.
    before: HashMap<PathBuf, Option<Arc<String>>>,
    /// The turn: one per prompt.
    pub turn: u32,
    /// Every turn of the session, in order.
    pub turns: Vec<Turn>,
}

impl ChangeSet {
    pub fn file(&self, path: &Path) -> Option<&FileChange> {
        self.files.iter().find(|f| f.path == path)
    }

    fn file_mut(&mut self, path: &Path) -> Option<&mut FileChange> {
        self.files.iter_mut().find(|f| f.path == path)
    }

    /// A new prompt to `agent` (its first line, if known).
    pub fn new_turn(&mut self, agent: &'static str, prompt: Option<String>) {
        self.turn += 1;
        self.before.clear();
        self.turns.push(Turn { id: self.turn, agent, prompt, at: std::time::SystemTime::now() });
    }

    /// The turn under way (the last one).
    pub fn current(&self) -> Option<&Turn> {
        self.turns.last()
    }

    pub fn turn_of(&self, id: u32) -> Option<&Turn> {
        self.turns.iter().find(|t| t.id == id)
    }

    /// `agent`'s last turn ended, and this was its prompt: kept unless one is known.
    pub fn turn_ended(&mut self, agent: &'static str, prompt: Option<String>) {
        if let Some(t) = self.turns.iter_mut().rev().find(|t| t.agent == agent)
            && t.prompt.is_none()
        {
            t.prompt = prompt;
        }
    }

    /// The files whose first unreviewed edit came in turn `id`.
    pub fn files_of_turn(&self, id: u32) -> Vec<PathBuf> {
        self.files.iter().filter(|f| f.turn == id).map(|f| f.path.clone()).collect()
    }

    /// A tool is about to change `path`, which holds `text` (none: missing). Kept as the
    /// file's snapshot unless it already has one.
    pub fn before_edit(&mut self, path: &Path, text: Option<Arc<String>>) {
        if self.file(path).is_none() {
            self.before.entry(path.to_owned()).or_insert(text);
        }
    }

    /// The snapshot `path` would get: its listed one, else the one taken before the edit
    /// under way. `None` when there is none at all; `Some(None)` for a file Claude made.
    pub fn base_of(&self, path: &Path) -> Option<Option<Arc<String>>> {
        match self.file(path) {
            Some(f) => Some(f.base.clone()),
            None => self.before.get(path).cloned(),
        }
    }

    /// `path` now holds `now` (none: gone), `diff` from its snapshot (see [`Self::base_of`])
    /// to it. `base` is the snapshot to use if the file has none yet (the watcher's last
    /// look). Returns whether the file is listed after.
    #[allow(clippy::too_many_arguments)]
    pub fn after_edit(
        &mut self,
        path: &Path,
        base: Option<Option<Arc<String>>>,
        now: Option<Arc<String>>,
        diff: Diffed,
        tool: &str,
        source: Source,
        agent: &'static str,
    ) -> bool {
        let unchanged = diff.hunks.is_empty();
        if let Some(f) = self.file_mut(path) {
            f.now = now;
            f.hunks = diff.hunks;
            (f.added, f.removed) = (diff.added, diff.removed);
            f.tool = tool.to_owned();
            f.edits += 1;
            f.agent = agent;
            if source == Source::Hook {
                f.source = Source::Hook;
            }
            if unchanged && f.base.as_deref().map(String::as_str) == f.now.as_deref().map(String::as_str) {
                self.files.retain(|f| f.path != path);
                return false;
            }
            return true;
        }
        let base = self.before.remove(path).or(base);
        let Some(base) = base else { return false };
        if unchanged && base.as_deref().map(String::as_str) == now.as_deref().map(String::as_str) {
            return false;
        }
        self.files.push(FileChange {
            path: path.to_owned(),
            base,
            now,
            hunks: diff.hunks,
            added: diff.added,
            removed: diff.removed,
            tool: tool.to_owned(),
            edits: 1,
            turn: self.turn,
            source,
            agent,
        });
        true
    }

    /// The file's diff worked out again, after its texts changed here.
    pub fn set_diff(&mut self, path: &Path, diff: Diffed) {
        let gone = match self.file_mut(path) {
            Some(f) => {
                let same = diff.hunks.is_empty();
                f.hunks = diff.hunks;
                (f.added, f.removed) = (diff.added, diff.removed);
                same
            }
            None => false,
        };
        if gone {
            self.files.retain(|f| f.path != path);
        }
    }

    /// Keep the file as it is: it leaves the list.
    pub fn accept_file(&mut self, path: &Path) -> bool {
        let n = self.files.len();
        self.files.retain(|f| f.path != path);
        self.files.len() != n
    }

    /// Put the file back as it was; it leaves the list.
    pub fn revert_file(&mut self, path: &Path) -> Option<Restore> {
        let at = self.files.iter().position(|f| f.path == path)?;
        let f = self.files.remove(at);
        Some(Restore { path: f.path, text: f.base.map(|b| b.as_str().to_owned()) })
    }

    pub fn accept_all(&mut self) -> Vec<PathBuf> {
        self.files.drain(..).map(|f| f.path).collect()
    }

    pub fn revert_all(&mut self) -> Vec<Restore> {
        self.files.drain(..).map(|f| Restore { path: f.path, text: f.base.map(|b| b.as_str().to_owned()) }).collect()
    }

    /// Keep hunk `ix` of `path`: the snapshot takes it. Returns the new snapshot and the
    /// text now, for the diff to be worked out again (off the UI thread).
    pub fn accept_hunk(&mut self, path: &Path, ix: usize) -> Option<Texts> {
        let f = self.file_mut(path)?;
        let h = f.hunks.get(ix)?.clone();
        let (base, now) = (f.base.as_deref().map(String::as_str).unwrap_or_default(), f.now.as_deref()?);
        let mut next = String::with_capacity(base.len() + h.new.len());
        next.push_str(&base[..h.old.start]);
        next.push_str(&now[h.new.clone()]);
        next.push_str(&base[h.old.end..]);
        f.base = Some(Arc::new(next));
        Some((f.base.clone(), f.now.clone()))
    }

    /// Put hunk `ix` of `path` back as it was. Returns what to write.
    pub fn revert_hunk(&mut self, path: &Path, ix: usize) -> Option<Restore> {
        let f = self.file_mut(path)?;
        let h = f.hunks.get(ix)?.clone();
        let (base, now) = (f.base.as_deref().map(String::as_str).unwrap_or_default(), f.now.as_deref()?);
        let mut next = String::with_capacity(now.len() + h.old.len());
        next.push_str(&now[..h.new.start]);
        next.push_str(&base[h.old.clone()]);
        next.push_str(&now[h.new.end..]);
        // A file Claude made, with its only hunk reverted: it goes.
        let text = (f.base.is_some() || !next.is_empty()).then_some(next);
        f.now = text.clone().map(Arc::new);
        Some(Restore { path: path.to_owned(), text })
    }

    /// Files, lines added and lines removed, in all.
    pub fn totals(&self) -> (usize, usize, usize) {
        self.files.iter().fold((0, 0, 0), |(n, a, r), f| (n + 1, a + f.added, r + f.removed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arc(s: &str) -> Option<Arc<String>> {
        Some(Arc::new(s.to_owned()))
    }

    /// An edit as the hooks report it: before, then after, the diff worked out here.
    fn edit(set: &mut ChangeSet, path: &str, before: Option<&str>, after: Option<&str>) -> bool {
        let p = Path::new(path);
        set.before_edit(p, before.and_then(arc));
        let base = set.base_of(p).unwrap();
        let diff = Diffed::of(base.as_deref().map(String::as_str), after);
        set.after_edit(p, None, after.and_then(arc), diff, "Edit", Source::Hook, crate::agents::CLAUDE)
    }

    const A0: &str = "one\ntwo\nthree\nfour\nfive\n";
    const A1: &str = "one\nTWO\nthree\nfour\nfive\nsix\n";

    #[test]
    fn edits_keep_the_snapshot_from_before_the_first() {
        let mut set = ChangeSet::default();
        assert!(edit(&mut set, "/a", Some(A0), Some(A1)));
        // A second edit: the snapshot stays the first one.
        let a2 = A1.replace("four", "FOUR");
        assert!(edit(&mut set, "/a", Some(A1), Some(&a2)));
        let f = set.file(Path::new("/a")).unwrap();
        assert_eq!(f.base.as_deref().unwrap(), A0);
        assert_eq!((f.added, f.removed, f.edits, f.hunks.len()), (3, 2, 2, 3));
        assert_eq!(set.totals(), (1, 3, 2));
        // Edited back to the snapshot: nothing left to review.
        assert!(!edit(&mut set, "/a", Some(&a2), Some(A0)));
        assert!(set.files.is_empty());
        // A tool that never ran (rejected) lists nothing.
        set.before_edit(Path::new("/b"), arc("x\n"));
        assert!(set.files.is_empty());
    }

    #[test]
    fn hunks_are_accepted_into_the_snapshot_or_reverted_out_of_the_file() {
        let mut set = ChangeSet::default();
        edit(&mut set, "/a", Some(A0), Some(A1));
        let p = Path::new("/a");
        // Revert the first hunk (TWO): the file gets two back, six stays.
        let r = set.revert_hunk(p, 0).unwrap();
        assert_eq!(r.text.as_deref(), Some("one\ntwo\nthree\nfour\nfive\nsix\n"));
        let f = set.file(p).unwrap();
        let d = Diffed::of(f.base.as_deref().map(String::as_str), f.now.as_deref().map(String::as_str));
        set.set_diff(p, d);
        assert_eq!(set.file(p).unwrap().hunks.len(), 1);
        // Accept the other (six): the snapshot takes it, and nothing is left.
        let (base, now) = set.accept_hunk(p, 0).unwrap();
        assert_eq!(base, now);
        set.set_diff(p, Diffed::of(base.as_deref().map(String::as_str), now.as_deref().map(String::as_str)));
        assert!(set.files.is_empty());
    }

    #[test]
    fn a_created_file_reverts_to_nothing() {
        let mut set = ChangeSet::default();
        assert!(edit(&mut set, "/new", None, Some("class New {}\n")));
        assert!(set.file(Path::new("/new")).unwrap().created());
        edit(&mut set, "/a", Some(A0), Some(A1));
        let r = set.revert_all();
        assert_eq!(r[0], Restore { path: "/new".into(), text: None }, "deleted");
        assert_eq!(r[1].text.as_deref(), Some(A0));
        assert!(set.files.is_empty());
        // Reverting a created file's only hunk deletes it too.
        edit(&mut set, "/new2", None, Some("x\n"));
        assert_eq!(set.revert_hunk(Path::new("/new2"), 0).unwrap().text, None);
    }

    #[test]
    fn accepting_everything_empties_the_list() {
        let mut set = ChangeSet::default();
        edit(&mut set, "/a", Some(A0), Some(A1));
        edit(&mut set, "/b", Some("b\n"), Some("B\n"));
        assert_eq!(set.accept_all(), [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(set.totals(), (0, 0, 0));
        // The next edit starts from the accepted text.
        assert!(edit(&mut set, "/b", Some("B\n"), Some("BB\n")));
        assert_eq!(set.file(Path::new("/b")).unwrap().base.as_deref().unwrap(), "B\n");
    }

    #[test]
    fn the_watcher_lists_a_change_against_its_last_look() {
        let mut set = ChangeSet::default();
        let p = Path::new("/w");
        let diff = Diffed::of(Some("a\n"), Some("b\n"));
        assert!(set.after_edit(p, Some(arc("a\n")), arc("b\n"), diff, "", Source::Watcher, crate::agents::CODEX));
        assert_eq!(set.file(p).unwrap().agent, crate::agents::CODEX);
        assert_eq!(set.file(p).unwrap().source, Source::Watcher);
        // Without any snapshot there is nothing to compare: not listed.
        assert!(!set.after_edit(Path::new("/x"), None, arc("b\n"), Diffed::default(), "", Source::Watcher, "codex"));
    }
}
