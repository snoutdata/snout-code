//! `git status --porcelain=v2` parsing.

use serde::Serialize;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
}

impl Change {
    pub(crate) fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            b'M' => Self::Modified,
            b'T' => Self::TypeChanged,
            b'A' => Self::Added,
            b'D' => Self::Deleted,
            b'R' => Self::Renamed,
            b'C' => Self::Copied,
            b'U' => Self::Conflicted,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Modified => "Modified",
            Self::TypeChanged => "Type changed",
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::Untracked => "Untracked",
            Self::Conflicted => "Conflict",
        }
    }
}

/// One changed path in the working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    /// Path relative to the repository root, `/`-separated.
    pub path: String,
    /// Source path of a rename or copy.
    pub orig_path: Option<String>,
    /// Change between HEAD and the index (i.e. what is staged).
    pub index: Option<Change>,
    /// Change between the index and the working tree (i.e. what is not staged).
    pub worktree: Option<Change>,
    pub conflicted: bool,
}

impl FileEntry {
    pub fn is_untracked(&self) -> bool {
        self.worktree == Some(Change::Untracked)
    }

    pub fn has_staged(&self) -> bool {
        self.index.is_some() && !self.conflicted
    }

    pub fn has_unstaged(&self) -> bool {
        self.worktree.is_some() || self.conflicted
    }

    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    pub fn directory(&self) -> &str {
        self.path.rsplit_once('/').map_or("", |(dir, _)| dir)
    }

    /// Human readable state, for the "State" column of the Files pane.
    pub fn state_label(&self) -> String {
        if self.conflicted {
            return "Conflict".into();
        }
        match (self.index, self.worktree) {
            (_, Some(Change::Untracked)) => "Untracked".into(),
            (Some(i), None) => format!("{} (staged)", i.label()),
            (None, Some(w)) => w.label().into(),
            (Some(i), Some(_)) => format!("{} (partly staged)", i.label()),
            (None, None) => "Unchanged".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BranchInfo {
    /// Current branch, `None` when HEAD is detached.
    pub head: Option<String>,
    /// Commit HEAD points to, `None` before the first commit.
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

impl BranchInfo {
    pub fn is_unborn(&self) -> bool {
        self.oid.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Status {
    pub branch: BranchInfo,
    pub files: Vec<FileEntry>,
}

impl Status {
    pub fn staged_count(&self) -> usize {
        self.files.iter().filter(|f| f.has_staged()).count()
    }
}

/// Parse `git status --porcelain=v2 -z --branch` output.
pub(crate) fn parse(raw: &[u8]) -> Result<Status> {
    let text = String::from_utf8_lossy(raw);
    let mut status = Status::default();
    let mut records = text.split('\0').filter(|r| !r.is_empty());

    while let Some(rec) = records.next() {
        let bad = || Error::Parse(format!("status record {rec:?}"));
        let (kind, rest) = rec.split_once(' ').ok_or_else(bad)?;
        match kind {
            "#" => parse_header(rest, &mut status.branch),
            "1" | "2" => {
                // XY sub mH mI mW hH hI [Xscore] path
                let n = if kind == "1" { 8 } else { 9 };
                let fields: Vec<&str> = rest.splitn(n, ' ').collect();
                if fields.len() != n {
                    return Err(bad());
                }
                let xy = fields[0].as_bytes();
                let orig_path = if kind == "2" { Some(records.next().ok_or_else(bad)?.to_owned()) } else { None };
                status.files.push(FileEntry {
                    path: fields[n - 1].to_owned(),
                    orig_path,
                    index: Change::from_code(xy[0]),
                    worktree: Change::from_code(xy[1]),
                    conflicted: false,
                });
            }
            "u" => {
                // XY sub m1 m2 m3 mW h1 h2 h3 path
                let fields: Vec<&str> = rest.splitn(10, ' ').collect();
                let path = fields.get(9).ok_or_else(bad)?;
                status.files.push(FileEntry {
                    path: (*path).to_owned(),
                    orig_path: None,
                    index: None,
                    worktree: Some(Change::Conflicted),
                    conflicted: true,
                });
            }
            "?" => status.files.push(FileEntry {
                path: rest.to_owned(),
                orig_path: None,
                index: None,
                worktree: Some(Change::Untracked),
                conflicted: false,
            }),
            "!" => {} // ignored files: not requested, but harmless
            _ => return Err(bad()),
        }
    }
    Ok(status)
}

fn parse_header(rest: &str, branch: &mut BranchInfo) {
    let Some((key, value)) = rest.split_once(' ') else {
        return;
    };
    match key {
        "branch.oid" => branch.oid = (value != "(initial)").then(|| value.to_owned()),
        "branch.head" => branch.head = (value != "(detached)").then(|| value.to_owned()),
        "branch.upstream" => branch.upstream = Some(value.to_owned()),
        "branch.ab" => {
            for part in value.split(' ') {
                if let Some(n) = part.strip_prefix('+') {
                    branch.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    branch.behind = n.parse().unwrap_or(0);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_record_kinds() {
        let raw = b"# branch.oid 1234abcd\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 aaa bbb src/lib.rs\0\
1 A. N... 000000 100644 100644 000 ccc dir with space/new file.txt\0\
2 R. N... 100644 100644 100644 ddd ddd R100 new/name.rs\0old/name.rs\0\
u UU N... 100644 100644 100644 100644 e f g conflict.txt\0\
? untracked.md\0";
        let s = parse(raw).unwrap();
        assert_eq!(s.branch.head.as_deref(), Some("main"));
        assert_eq!(s.branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.branch.ahead, s.branch.behind), (2, 1));
        assert_eq!(s.files.len(), 5);

        assert_eq!(s.files[0].state_label(), "Modified");
        assert_eq!(s.files[0].directory(), "src");
        assert_eq!(s.files[1].path, "dir with space/new file.txt");
        assert_eq!(s.files[1].state_label(), "Added (staged)");
        assert_eq!(s.files[2].orig_path.as_deref(), Some("old/name.rs"));
        assert_eq!(s.files[2].index, Some(Change::Renamed));
        assert!(s.files[3].conflicted);
        assert!(s.files[4].is_untracked());
    }

    #[test]
    fn parses_unborn_and_detached() {
        let s = parse(b"# branch.oid (initial)\0# branch.head (detached)\0").unwrap();
        assert!(s.branch.is_unborn());
        assert_eq!(s.branch.head, None);
    }
}
