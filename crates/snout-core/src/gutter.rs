//! What an editor's gutter marks: the lines of a file added, changed or removed since the
//! last commit, from a line diff of the committed text against the text being edited.

use std::ops::Range;
use std::path::Path;
use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};

use crate::git::run_raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineChangeKind {
    Added,
    Modified,
    /// Lines were removed just above `lines.start` (an empty range).
    Deleted,
}

/// A run of lines in the edited text (from 0) and how they differ from the commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineChange {
    pub kind: LineChangeKind,
    pub lines: Range<usize>,
}

/// The text of `path` (relative to `root` on `host`, `/`-separated) as HEAD has it, or `None`
/// when HEAD does not have the file (or there is no HEAD, or it is not text).
pub fn head_text(host: &dyn snout_host::Host, root: &Path, path: &str) -> Option<String> {
    let out = run_raw(host, root, &["cat-file", "blob", &format!("HEAD:{path}")], None).ok()?;
    if !out.success {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// How `new` differs from `old`, line by line, for the gutter. A diff that takes too
/// long is cut short rather than holding up typing.
pub fn line_changes(old: &str, new: &str) -> Vec<LineChange> {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let deadline = Instant::now() + Duration::from_millis(250);
    let ops = capture_diff_slices_deadline(Algorithm::Myers, &a, &b, Some(deadline));
    let mut out: Vec<LineChange> = Vec::new();
    for op in ops {
        let change = match op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Insert { new_index, new_len, .. } => {
                LineChange { kind: LineChangeKind::Added, lines: new_index..new_index + new_len }
            }
            DiffOp::Delete { new_index, .. } => {
                LineChange { kind: LineChangeKind::Deleted, lines: new_index..new_index }
            }
            DiffOp::Replace { new_index, new_len, .. } => {
                LineChange { kind: LineChangeKind::Modified, lines: new_index..new_index + new_len }
            }
        };
        // A removal next to an insertion reads as a change of those lines.
        if let Some(last) = out.last_mut()
            && last.lines.end == change.lines.start
            && matches!(
                (last.kind, change.kind),
                (LineChangeKind::Deleted, LineChangeKind::Added) | (LineChangeKind::Added, LineChangeKind::Deleted)
            )
        {
            last.kind = LineChangeKind::Modified;
            last.lines.end = last.lines.end.max(change.lines.end);
            continue;
        }
        out.push(change);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(old: &str, new: &str) -> Vec<(LineChangeKind, Range<usize>)> {
        line_changes(old, new).into_iter().map(|c| (c.kind, c.lines)).collect()
    }

    #[test]
    fn added_changed_and_removed_lines() {
        use LineChangeKind::*;
        let old = "a\nb\nc\nd\ne\n";
        assert!(kinds(old, old).is_empty());
        assert_eq!(kinds(old, "a\nb\nX\nY\nc\nd\ne\n"), [(Added, 2..4)]);
        assert_eq!(kinds(old, "a\nB\nc\nd\ne\n"), [(Modified, 1..2)]);
        assert_eq!(kinds(old, "a\nb\ne\n"), [(Deleted, 2..2)]);
        assert_eq!(kinds(old, "a\nb\nc\nd\ne\nf"), [(Added, 5..6)]);
        assert_eq!(kinds(old, "Z\nb\nc\ne\n"), [(Modified, 0..1), (Deleted, 3..3)]);
    }

    #[test]
    fn reads_the_committed_text() {
        let dir = std::env::temp_dir().join(format!("snout-gutter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git").arg("-C").arg(&dir).args(args).status().unwrap().success())
        };
        git(&["init", "-q"]);
        assert_eq!(head_text(&*snout_host::local(), &dir, "src/a.txt"), None, "no HEAD yet");
        std::fs::write(dir.join("src/a.txt"), "one\n").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "x",
        ]);
        std::fs::write(dir.join("src/a.txt"), "two\n").unwrap();
        assert_eq!(head_text(&*snout_host::local(), &dir, "src/a.txt").as_deref(), Some("one\n"));
        assert_eq!(head_text(&*snout_host::local(), &dir, "src/b.txt"), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
