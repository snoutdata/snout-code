//! Whole-file, line-aligned diffs suitable for side-by-side or unified display.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};

/// Which two versions of a file to compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiffBase {
    /// Everything not yet committed.
    #[default]
    HeadToWorktree,
    /// What is not yet staged.
    IndexToWorktree,
    /// What is staged.
    HeadToIndex,
}

impl DiffBase {
    pub const ALL: [Self; 3] = [Self::HeadToWorktree, Self::IndexToWorktree, Self::HeadToIndex];

    pub fn label(self) -> &'static str {
        match self {
            Self::HeadToWorktree => "HEAD ↔ Working tree",
            Self::IndexToWorktree => "Index ↔ Working tree",
            Self::HeadToIndex => "HEAD ↔ Index",
        }
    }

    pub fn sides(self) -> (&'static str, &'static str) {
        match self {
            Self::HeadToWorktree => ("HEAD", "Working tree"),
            Self::IndexToWorktree => ("Index", "Working tree"),
            Self::HeadToIndex => ("HEAD", "Index"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Equal,
    Delete,
    Insert,
    /// Old and new line differ (part of a replaced block).
    Replace,
}

/// One display row. `old`/`new` are `(1-based line number, text without newline)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffRow {
    pub kind: RowKind,
    pub old: Option<(u32, String)>,
    pub new: Option<(u32, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileDiff {
    pub path: String,
    /// What was compared; `None` for a commit against its parent.
    pub base: Option<DiffBase>,
    /// Set when either side is binary; `rows` is then empty.
    pub binary: bool,
    /// Set when either side exceeds the size limit; `rows` is then empty.
    pub too_large: bool,
    pub rows: Vec<DiffRow>,
    pub additions: usize,
    pub deletions: usize,
}

impl FileDiff {
    pub fn is_identical(&self) -> bool {
        !self.binary && !self.too_large && self.additions == 0 && self.deletions == 0
    }
}

pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;

fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(8000)].contains(&0)
}

fn split_lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').map(|l| l.trim_end_matches('\n').trim_end_matches('\r')).collect()
}

pub(crate) fn compute(path: String, base: Option<DiffBase>, old: &[u8], new: &[u8]) -> FileDiff {
    let mut diff =
        FileDiff { path, base, binary: false, too_large: false, rows: Vec::new(), additions: 0, deletions: 0 };
    if old.len() > MAX_BYTES || new.len() > MAX_BYTES {
        diff.too_large = true;
        return diff;
    }
    if is_binary(old) || is_binary(new) {
        diff.binary = old != new;
        return diff;
    }

    let old_text = String::from_utf8_lossy(old);
    let new_text = String::from_utf8_lossy(new);
    let (o, n) = (split_lines(&old_text), split_lines(&new_text));
    let deadline = Instant::now() + Duration::from_secs(2);
    let ops = capture_diff_slices_deadline(Algorithm::Patience, &o, &n, Some(deadline));

    let line = |lines: &[&str], i: usize| Some((i as u32 + 1, lines[i].to_owned()));
    for op in ops {
        match op {
            DiffOp::Equal { old_index, new_index, len } => {
                for k in 0..len {
                    diff.rows.push(DiffRow {
                        kind: RowKind::Equal,
                        old: line(&o, old_index + k),
                        new: line(&n, new_index + k),
                    });
                }
            }
            DiffOp::Delete { old_index, old_len, .. } => {
                diff.deletions += old_len;
                for k in 0..old_len {
                    diff.rows.push(DiffRow { kind: RowKind::Delete, old: line(&o, old_index + k), new: None });
                }
            }
            DiffOp::Insert { new_index, new_len, .. } => {
                diff.additions += new_len;
                for k in 0..new_len {
                    diff.rows.push(DiffRow { kind: RowKind::Insert, old: None, new: line(&n, new_index + k) });
                }
            }
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                diff.deletions += old_len;
                diff.additions += new_len;
                for k in 0..old_len.max(new_len) {
                    diff.rows.push(DiffRow {
                        kind: RowKind::Replace,
                        old: (k < old_len).then(|| line(&o, old_index + k)).flatten(),
                        new: (k < new_len).then(|| line(&n, new_index + k)).flatten(),
                    });
                }
            }
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_replacements() {
        let d = compute("f".into(), None, b"a\nb\nc\n", b"a\nB\nB2\nc\nd\n");
        assert_eq!((d.additions, d.deletions), (3, 1));
        let kinds: Vec<RowKind> = d.rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, [RowKind::Equal, RowKind::Replace, RowKind::Replace, RowKind::Equal, RowKind::Insert]);
        assert_eq!(d.rows[2].old, None);
        assert_eq!(d.rows[2].new, Some((3, "B2".into())));
    }

    #[test]
    fn detects_binary() {
        let d = compute("f".into(), None, b"\0\x01", b"\0\x02");
        assert!(d.binary && d.rows.is_empty());
    }
}
