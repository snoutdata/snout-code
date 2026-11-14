//! What the Branches pane does to a branch beyond checking it out: merge, rebase,
//! fast-forward, push, reset, rename and tracking.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::repo::Repo;

/// How far `git reset` goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetMode {
    /// Move the branch only; the changes since stay staged.
    Soft,
    /// Move the branch and the index; the changes since stay in the files, unstaged.
    Mixed,
    /// Move the branch and make the files match; uncommitted changes are lost.
    Hard,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

impl Repo {
    /// Merge `rev` into the current branch. On conflicts the merge is undone, so the
    /// working tree is as it was, and the error says which files clashed.
    pub fn merge(&self, rev: &str) -> Result<String> {
        let rev = self.verified(rev)?;
        match self.git(&["merge", "--no-edit", &rev]) {
            Ok(out) => Ok(out.text()),
            Err(e) => {
                if self.git_raw(&["rev-parse", "-q", "--verify", "MERGE_HEAD"])?.success {
                    let _ = self.git(&["merge", "--abort"]);
                    return Err(Error::Invalid(format!("{e}\n\nThe merge stopped on conflicts and was undone.")));
                }
                Err(e)
            }
        }
    }

    /// Rebase the current branch onto `rev`. On conflicts the rebase is undone.
    pub fn rebase(&self, rev: &str) -> Result<String> {
        let rev = self.verified(rev)?;
        match self.git(&["rebase", &rev]) {
            Ok(out) => Ok(out.text()),
            Err(e) => {
                if self.git_raw(&["rev-parse", "-q", "--verify", "REBASE_HEAD"])?.success {
                    let _ = self.git(&["rebase", "--abort"]);
                    return Err(Error::Invalid(format!("{e}\n\nThe rebase stopped on conflicts and was undone.")));
                }
                Err(e)
            }
        }
    }

    /// Move local branch `name` forward to its upstream, only when no commit of its own is
    /// left behind. The checked-out branch moves its files along; any other moves just its ref.
    pub fn fast_forward(&self, name: &str) -> Result<String> {
        let up = self.git(&["for-each-ref", "--format=%(upstream)", &format!("refs/heads/{name}")])?;
        let up = up.stdout_str().trim().to_owned();
        if up.is_empty() {
            return Err(Error::Invalid(format!("`{name}` has no upstream")));
        }
        let head = self.status()?.branch.head;
        if head.as_deref() == Some(name) {
            Ok(self.git(&["merge", "--ff-only", &up])?.text())
        } else {
            Ok(self.git(&["fetch", ".", &format!("{up}:refs/heads/{name}")])?.text())
        }
    }

    /// Push local branch `name` to its upstream; one without an upstream goes to a branch of
    /// its own name on `origin` (else the first remote), which becomes its upstream.
    pub fn push_branch(&self, name: &str) -> Result<String> {
        let remote = self.config(&format!("branch.{name}.remote"));
        let merge = self.config(&format!("branch.{name}.merge"));
        if let (Some(remote), Some(merge)) = (remote, merge)
            && remote != "."
        {
            return Ok(self.git(&["push", &remote, &format!("refs/heads/{name}:{merge}")])?.text());
        }
        let remotes = self.remotes()?;
        let remote = remotes
            .iter()
            .find(|(n, _)| n == "origin")
            .or(remotes.first())
            .map(|(n, _)| n.clone())
            .ok_or_else(|| Error::Invalid("no remote configured".into()))?;
        self.push_to(name, &remote, name, true)
    }

    /// Push local branch `name` to `branch` on `remote`; `track` makes that its upstream.
    pub fn push_to(&self, name: &str, remote: &str, branch: &str, track: bool) -> Result<String> {
        let branch = branch.trim();
        self.git(&["check-ref-format", "--branch", branch])
            .map_err(|_| Error::Invalid(format!("`{branch}` is not a valid branch name")))?;
        if !self.remotes()?.iter().any(|(n, _)| n == remote) {
            return Err(Error::Invalid(format!("no remote named `{remote}`")));
        }
        let spec = format!("refs/heads/{name}:refs/heads/{branch}");
        let mut args = vec!["push"];
        if track {
            args.push("-u");
        }
        args.extend([remote, spec.as_str()]);
        Ok(self.git(&args)?.text())
    }

    /// Move the current branch to `rev`, as far as `mode` says.
    pub fn reset(&self, rev: &str, mode: ResetMode) -> Result<()> {
        let rev = self.verified(rev)?;
        self.git(&["reset", mode.flag(), &rev])?;
        Ok(())
    }

    /// Rename local branch `old` to `new`.
    pub fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        let new = new.trim();
        self.git(&["check-ref-format", "--branch", new])
            .map_err(|_| Error::Invalid(format!("`{new}` is not a valid branch name")))?;
        self.git(&["branch", "-m", "--", old, new])?;
        Ok(())
    }

    /// Make remote branch `upstream` (e.g. `origin/main`) the one local branch `name` tracks.
    pub fn set_upstream(&self, name: &str, upstream: &str) -> Result<()> {
        let upstream = self.verified(&format!("refs/remotes/{upstream}"))?;
        self.git(&["branch", &format!("--set-upstream-to={upstream}"), "--", name])?;
        Ok(())
    }

    /// Local branch `name` tracks nothing any more.
    pub fn unset_upstream(&self, name: &str) -> Result<()> {
        self.git(&["branch", "--unset-upstream", "--", name])?;
        Ok(())
    }

    /// `rev`, once Git knows it as a commit (so it cannot pass for an option).
    fn verified(&self, rev: &str) -> Result<String> {
        let rev = rev.trim();
        if rev.starts_with('-')
            || !self.git_raw(&["rev-parse", "-q", "--verify", &format!("{rev}^{{commit}}")])?.success
        {
            return Err(Error::Invalid(format!("`{rev}` is not a commit")));
        }
        Ok(rev.to_owned())
    }

    fn config(&self, key: &str) -> Option<String> {
        let out = self.git_raw(&["config", "--get", key]).ok()?;
        let v = out.stdout_str().trim().to_owned();
        (out.success && !v.is_empty()).then_some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_run as run;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snout-branch-ops-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn configure(dir: &Path) {
        for args in [
            ["config", "user.name", "T"],
            ["config", "user.email", "t@example.com"],
            ["config", "commit.gpgsign", "false"],
        ] {
            run(dir, &args).unwrap();
        }
    }

    fn commit(repo: &Repo, file: &str, text: &str) {
        fs::write(repo.root().join(file), text).unwrap();
        repo.stage(&[file.into()]).unwrap();
        repo.commit(&format!("edit {file}"), false, &[]).unwrap();
    }

    fn head(repo: &Repo) -> String {
        repo.git(&["rev-parse", "HEAD"]).unwrap().stdout_str().trim().to_owned()
    }

    fn branch(repo: &Repo, name: &str) -> crate::Branch {
        repo.branches().unwrap().into_iter().find(|b| b.name == name).unwrap()
    }

    /// A bare remote, and a clone of it on `main` with one commit pushed.
    fn cloned(name: &str) -> (PathBuf, Repo) {
        let dir = scratch(name);
        let remote = dir.join("remote.git");
        run(&dir, &["init", "-q", "--bare", "-b", "main", &remote.to_string_lossy()]).unwrap();
        let work = dir.join("work");
        run(&dir, &["clone", "-q", &remote.to_string_lossy(), &work.to_string_lossy()]).unwrap();
        configure(&work);
        run(&work, &["checkout", "-q", "-b", "main"]).unwrap();
        let repo = Repo::open(&snout_host::local(), &work).unwrap();
        commit(&repo, "a.txt", "a\n");
        repo.push().unwrap();
        (dir, repo)
    }

    #[test]
    fn merge_and_rebase_undo_themselves_on_conflicts() {
        let dir = scratch("merge");
        let repo = Repo::init(&snout_host::local(), &dir).unwrap();
        configure(&dir);
        run(&dir, &["checkout", "-q", "-b", "main"]).unwrap();
        commit(&repo, "f.txt", "base\n");
        repo.create_branch("side", None, true).unwrap();
        commit(&repo, "s.txt", "side\n");
        run(&dir, &["checkout", "-q", "main"]).unwrap();
        commit(&repo, "m.txt", "main\n");

        // Rebase main onto side: side's file arrives, main's commit is replayed.
        repo.rebase("side").unwrap();
        assert!(dir.join("s.txt").exists() && dir.join("m.txt").exists());
        // Merging side again changes nothing; a clash undoes itself.
        repo.merge("side").unwrap();
        repo.create_branch("clash", Some("side"), true).unwrap();
        commit(&repo, "f.txt", "clash\n");
        run(&dir, &["checkout", "-q", "main"]).unwrap();
        commit(&repo, "f.txt", "main\n");
        let before = head(&repo);
        let e = repo.merge("clash").unwrap_err().to_string();
        assert!(e.contains("undone"), "{e}");
        assert_eq!(head(&repo), before);
        assert_eq!(fs::read_to_string(dir.join("f.txt")).unwrap(), "main\n");
        let e = repo.rebase("clash").unwrap_err().to_string();
        assert!(e.contains("undone"), "{e}");
        assert_eq!(head(&repo), before);
        assert!(repo.merge("--help").is_err() && repo.merge("no-such").is_err());

        // Reset main back to side: soft keeps the changes staged.
        repo.reset("side", ResetMode::Soft).unwrap();
        assert!(repo.status().unwrap().staged_count() > 0);
        repo.reset("HEAD", ResetMode::Hard).unwrap();
        assert!(repo.status().unwrap().files.is_empty());

        repo.rename_branch("clash", "clash-2").unwrap();
        assert!(repo.rename_branch("clash-2", "bad name").is_err());
        assert!(repo.branches().unwrap().iter().any(|b| b.name == "clash-2"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn push_track_and_fast_forward() {
        let (dir, repo) = cloned("push");
        // A branch of its own pushes to its own name and tracks it.
        repo.create_branch("feature", None, false).unwrap();
        repo.push_branch("feature").unwrap();
        assert_eq!(branch(&repo, "feature").upstream.as_deref(), Some("origin/feature"));
        // Push To another name, tracking it.
        repo.push_to("feature", "origin", "other", true).unwrap();
        assert_eq!(branch(&repo, "feature").upstream.as_deref(), Some("origin/other"));
        assert!(repo.push_to("feature", "nowhere", "x", false).is_err());
        repo.unset_upstream("feature").unwrap();
        assert_eq!(branch(&repo, "feature").upstream, None);
        repo.set_upstream("feature", "origin/main").unwrap();
        assert_eq!(branch(&repo, "feature").upstream.as_deref(), Some("origin/main"));
        assert!(repo.set_upstream("feature", "origin/none").is_err());

        // main moves on; feature (not checked out) and then main itself fast-forward.
        commit(&repo, "b.txt", "b\n");
        repo.push_branch("main").unwrap();
        repo.fast_forward("feature").unwrap();
        assert_eq!(branch(&repo, "feature").target, branch(&repo, "main").target);
        repo.reset("HEAD~1", ResetMode::Hard).unwrap();
        repo.fast_forward("main").unwrap();
        assert!(repo.root().join("b.txt").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
