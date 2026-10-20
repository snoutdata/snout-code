use serde::Serialize;

use crate::Change;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub email: String,
    /// Unix timestamp of the author date.
    pub time: i64,
    /// Author date formatted in local time, `YYYY-MM-DD HH:MM`.
    pub date: String,
    /// Decorations such as `HEAD -> main`, `origin/main`, `tag: v1.0`.
    pub refs: Vec<String>,
    /// Full ids of the parents, first parent first. Empty for a root commit.
    pub parents: Vec<String>,
    pub summary: String,
    /// Reachable from HEAD but not from its upstream (i.e. would be pushed).
    pub unpushed: bool,
}

/// Everything about one commit: the full message, both identities, and the files it changed
/// relative to its first parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitDetails {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub email: String,
    /// Author date, `YYYY-MM-DD HH:MM` in local time.
    pub date: String,
    pub committer: String,
    pub committer_email: String,
    pub committer_date: String,
    pub refs: Vec<String>,
    pub parents: Vec<String>,
    /// The whole message, subject and body, without trailing blank lines.
    pub message: String,
    pub files: Vec<CommitFile>,
}

impl CommitDetails {
    pub fn summary(&self) -> &str {
        self.message.lines().next().unwrap_or_default()
    }

    /// The message after the subject line, if there is any.
    pub fn body(&self) -> &str {
        self.message.split_once('\n').map_or("", |(_, rest)| rest.trim())
    }
}

/// One path a commit changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitFile {
    pub path: String,
    /// Source path of a rename or copy.
    pub orig_path: Option<String>,
    pub change: Change,
}

impl CommitFile {
    pub fn file_name(&self) -> &str {
        self.path.rsplit_once('/').map_or(&self.path, |(_, name)| name)
    }

    pub fn directory(&self) -> &str {
        self.path.rsplit_once('/').map_or("", |(dir, _)| dir)
    }
}

pub(crate) const FORMAT: &str = "--format=%H%x00%h%x00%an%x00%ae%x00%at%x00%ad%x00%D%x00%P%x00%s%x1e";
pub(crate) const DATE: &str = "--date=format-local:%Y-%m-%d %H:%M";
pub(crate) const DETAILS_FORMAT: &str = "--format=%H%x00%h%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd%x00%D%x00%P%x00%B";

fn split_refs(s: &str) -> Vec<String> {
    s.split(", ").filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

fn split_parents(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}

pub(crate) fn parse(raw: &str) -> Vec<Commit> {
    raw.split('\x1e')
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.is_empty())
        .filter_map(|r| {
            let f: Vec<&str> = r.splitn(9, '\0').collect();
            (f.len() == 9).then(|| Commit {
                id: f[0].to_owned(),
                short_id: f[1].to_owned(),
                author: f[2].to_owned(),
                email: f[3].to_owned(),
                time: f[4].parse().unwrap_or(0),
                date: f[5].to_owned(),
                refs: split_refs(f[6]),
                parents: split_parents(f[7]),
                summary: f[8].to_owned(),
                unpushed: false,
            })
        })
        .collect()
}

/// Parse `git show -s DETAILS_FORMAT` output; `files` is left empty.
pub(crate) fn parse_details(raw: &str) -> Option<CommitDetails> {
    let f: Vec<&str> = raw.splitn(11, '\0').collect();
    (f.len() == 11).then(|| CommitDetails {
        id: f[0].to_owned(),
        short_id: f[1].to_owned(),
        author: f[2].to_owned(),
        email: f[3].to_owned(),
        date: f[4].to_owned(),
        committer: f[5].to_owned(),
        committer_email: f[6].to_owned(),
        committer_date: f[7].to_owned(),
        refs: split_refs(f[8]),
        parents: split_parents(f[9]),
        message: f[10].trim_end().to_owned(),
        files: Vec::new(),
    })
}

/// Parse `git diff-tree -z --name-status` output: `M\0path\0`, or `R100\0old\0new\0` for a
/// rename or copy.
pub(crate) fn parse_name_status(raw: &str) -> Vec<CommitFile> {
    let mut fields = raw.split('\0').filter(|s| !s.is_empty());
    let mut files = Vec::new();
    while let Some(code) = fields.next() {
        let Some(change) = code.bytes().next().and_then(Change::from_code) else { continue };
        let Some(first) = fields.next() else { break };
        let file = if matches!(change, Change::Renamed | Change::Copied) {
            let Some(second) = fields.next() else { break };
            CommitFile { path: second.to_owned(), orig_path: Some(first.to_owned()), change }
        } else {
            CommitFile { path: first.to_owned(), orig_path: None, change }
        };
        files.push(file);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_status_with_renames() {
        let files = parse_name_status("M\0a.txt\0R087\0old/b.rs\0new/b.rs\0D\0c\0");
        assert_eq!(files.len(), 3);
        assert_eq!(files[0], CommitFile { path: "a.txt".into(), orig_path: None, change: Change::Modified });
        assert_eq!(files[1].path, "new/b.rs");
        assert_eq!(files[1].orig_path.as_deref(), Some("old/b.rs"));
        assert_eq!((files[1].file_name(), files[1].directory()), ("b.rs", "new"));
        assert_eq!(files[2].change, Change::Deleted);
    }
}
