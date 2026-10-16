use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Branch {
    /// Short name, e.g. `main` or `origin/main`.
    pub name: String,
    /// Full ref, e.g. `refs/heads/main`.
    pub full_ref: String,
    pub kind: BranchKind,
    /// For remote branches: the remote's name.
    pub remote: Option<String>,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream is configured but no longer exists.
    pub upstream_gone: bool,
    pub target: String,
    pub subject: String,
}

impl Branch {
    /// For a remote branch, the name without the remote prefix (`origin/foo` -> `foo`).
    pub fn local_name(&self) -> &str {
        match &self.remote {
            Some(remote) => {
                self.name.strip_prefix(remote.as_str()).and_then(|s| s.strip_prefix('/')).unwrap_or(&self.name)
            }
            None => &self.name,
        }
    }
}

pub(crate) const FORMAT: &str = "--format=%(refname)%00%(refname:short)%00%(HEAD)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(objectname:short)%00%(contents:subject)";

pub(crate) fn parse(raw: &str) -> Vec<Branch> {
    raw.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Branch> {
    let f: Vec<&str> = line.splitn(7, '\0').collect();
    if f.len() != 7 {
        return None;
    }
    let full_ref = f[0];
    let (kind, remote) = if full_ref.starts_with("refs/heads/") {
        (BranchKind::Local, None)
    } else {
        let rest = full_ref.strip_prefix("refs/remotes/")?;
        // `refs/remotes/origin/HEAD` is a symbolic pointer, not a branch.
        if rest.ends_with("/HEAD") {
            return None;
        }
        (BranchKind::Remote, rest.split('/').next().map(str::to_owned))
    };

    let (mut ahead, mut behind, mut gone) = (0, 0, false);
    for part in f[4].split(", ") {
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        } else if part == "gone" {
            gone = true;
        }
    }

    Some(Branch {
        name: f[1].to_owned(),
        full_ref: full_ref.to_owned(),
        kind,
        remote,
        is_head: f[2] == "*",
        upstream: (!f[3].is_empty()).then(|| f[3].to_owned()),
        ahead,
        behind,
        upstream_gone: gone,
        target: f[5].to_owned(),
        subject: f[6].to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_and_remote() {
        let raw = "refs/heads/main\0main\0*\0origin/main\0ahead 1, behind 2\0abc123\0Fix things\n\
refs/remotes/origin/HEAD\0origin\0 \0\0\0abc123\0Fix things\n\
refs/remotes/origin/feature/x\0origin/feature/x\0 \0\0\0def456\0Feature\n";
        let b = parse(raw);
        assert_eq!(b.len(), 2);
        assert!(b[0].is_head);
        assert_eq!((b[0].ahead, b[0].behind), (1, 2));
        assert_eq!(b[1].kind, BranchKind::Remote);
        assert_eq!(b[1].local_name(), "feature/x");
    }
}
