//! What a commit records, and the file selections that feed it: the rules both front ends
//! share for Commit, Draft, Stage, Unstage and Delete.

use crate::{Change, FileEntry, Status};

/// What Commit records: the staged changes; with nothing staged, the selected files, or
/// every changed file when none is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Staged(usize),
    /// Files to add first; `all` when nothing was selected.
    Files {
        paths: Vec<String>,
        all: bool,
    },
    /// Amending with nothing new: only the message changes.
    MessageOnly,
    Nothing,
}

impl Scope {
    /// See [`Scope`]. `selected` are the selected entries of `status`.
    pub fn of(status: &Status, selected: &[FileEntry], amend: bool) -> Self {
        let staged = status.staged_count();
        if staged > 0 {
            return Scope::Staged(staged);
        }
        let (entries, all) = if selected.is_empty() { (status.files.as_slice(), true) } else { (selected, false) };
        match stage_paths(entries) {
            paths if !paths.is_empty() => Scope::Files { paths, all },
            _ if amend => Scope::MessageOnly,
            _ => Scope::Nothing,
        }
    }

    /// Paths the commit adds first.
    pub fn paths(&self) -> Vec<String> {
        match self {
            Scope::Files { paths, .. } => paths.clone(),
            _ => Vec::new(),
        }
    }

    /// Beside the Commit button.
    pub fn label(&self) -> String {
        match self {
            Scope::Staged(n) => format!("{n} staged"),
            Scope::Files { paths, all: true } => format!("all {} changed", paths.len()),
            Scope::Files { paths, all: false } => format!("{} selected", paths.len()),
            Scope::MessageOnly => "message only".to_owned(),
            Scope::Nothing => "nothing to commit".to_owned(),
        }
    }

    /// In a sentence: "Commits …".
    pub fn what(&self) -> String {
        match self {
            Scope::Staged(_) => "the staged changes".to_owned(),
            Scope::Files { paths, all: true } if paths.len() == 1 => "the changed file".to_owned(),
            Scope::Files { paths, all: true } => format!("all {} changed files", paths.len()),
            Scope::Files { paths, .. } if paths.len() == 1 => "the selected file".to_owned(),
            Scope::Files { paths, .. } => format!("the {} selected files", paths.len()),
            Scope::MessageOnly => "the new message for the last commit".to_owned(),
            Scope::Nothing => "nothing".to_owned(),
        }
    }
}

/// Paths to stage.
pub fn stage_paths(entries: &[FileEntry]) -> Vec<String> {
    entries.iter().filter(|e| e.has_unstaged()).map(|e| e.path.clone()).collect()
}

/// Paths to unstage; renames need both sides restored in the index.
pub fn unstage_paths(entries: &[FileEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|e| e.has_staged())
        .flat_map(|e| std::iter::once(e.path.clone()).chain(e.orig_path.clone()))
        .collect()
}

/// Whether there is a file on disk to delete.
pub fn on_disk(e: &FileEntry) -> bool {
    e.worktree != Some(Change::Deleted) && e.index != Some(Change::Deleted)
}

/// `https://github.com/o/repo.git` -> `repo`: the folder a clone lands in.
pub fn repo_dir_name(url: &str) -> Option<String> {
    let name = url.trim().trim_end_matches('/').rsplit(['/', ':']).next()?.trim_end_matches(".git");
    (!name.is_empty()).then(|| name.to_owned())
}

/// Why `name` cannot be a branch, for the common cases (`git check-ref-format` has the rest).
pub fn branch_name_problem(name: &str) -> Option<&'static str> {
    if name.starts_with('-') {
        Some("A branch name cannot start with a dash.")
    } else if name.contains("..") || name.contains("@{") {
        Some("A branch name cannot contain “..” or “@{”.")
    } else if name.chars().any(|c| c.is_control() || "~^:?*[\\".contains(c)) {
        Some("A branch name cannot contain ~ ^ : ? * [ or \\.")
    } else if name.ends_with('/') || name.ends_with('.') || name.ends_with(".lock") || name.starts_with('/') {
        Some("A branch name cannot start with “/” or end with “/”, “.” or “.lock”.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_folders_and_branch_names() {
        assert_eq!(repo_dir_name("https://github.com/o/repo.git").as_deref(), Some("repo"));
        assert_eq!(repo_dir_name("git@github.com:o/thing").as_deref(), Some("thing"));
        assert!(branch_name_problem("feature/x").is_none());
        assert!(branch_name_problem("-x").is_some() && branch_name_problem("a..b").is_some());
    }

    #[test]
    fn nothing_to_commit_without_changes() {
        let status = Status::default();
        assert_eq!(Scope::of(&status, &[], false), Scope::Nothing);
        assert_eq!(Scope::of(&status, &[], true), Scope::MessageOnly);
        assert_eq!(Scope::Files { paths: vec!["a".into()], all: true }.what(), "the changed file");
    }
}
