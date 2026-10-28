use std::path::{Path, PathBuf};

use serde::Serialize;
use snout_host::{Host, HostRef};

use crate::git::{run, run_in_index, run_raw, run_unattended, run_with_stdin};
use crate::status::FileEntry;
use crate::{Branch, Commit, CommitDetails, CommitFile, DiffBase, Error, FileDiff, Result, Status, Tag};
use crate::{branches, diff, log, status, tags};

/// How the current branch stands against its upstream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tracking {
    /// Commits to push.
    pub ahead: u32,
    /// Commits to pull.
    pub behind: u32,
}

/// A Git working tree on a [`Host`]. Cheap to clone and `Send`, so it can be moved to
/// worker threads. Two are equal when they are the same tree on the same host.
#[derive(Clone, Serialize)]
pub struct Repo {
    root: PathBuf,
    #[serde(skip)]
    host: HostRef,
}

impl std::fmt::Debug for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repo").field("root", &self.root).finish()
    }
}

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && std::ptr::addr_eq(std::sync::Arc::as_ptr(&self.host), std::sync::Arc::as_ptr(&other.host))
    }
}

impl Eq for Repo {}

impl Repo {
    /// Open the working tree on `host` containing `path`.
    pub fn open(host: &HostRef, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !host.is_dir(path) {
            return Err(Error::NotARepo(path.to_owned()));
        }
        let out = run_raw(&**host, path, &["rev-parse", "--show-toplevel"], None)?;
        if !out.success {
            return Err(Error::NotARepo(path.to_owned()));
        }
        Ok(Self { root: PathBuf::from(out.stdout_str().trim_end()), host: host.clone() })
    }

    /// Create a new repository in the existing directory `path` on `host`.
    pub fn init(host: &HostRef, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !host.is_dir(path) {
            return Err(Error::Invalid(format!("{} is not a folder", path.display())));
        }
        run(&**host, path, &["init", "-q"])?;
        Self::open(host, path)
    }

    /// Clone `url` into `dest` on `host` (which must not exist or be empty). Network operation.
    pub fn clone_from(host: &HostRef, url: &str, dest: &Path) -> Result<(Self, String)> {
        let parent = dest.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let dest_str = dest.to_string_lossy();
        let out = run(&**host, parent, &["clone", "--progress", url, &dest_str])?;
        Ok((Self::open(host, dest)?, out.text()))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The host the working tree is on.
    pub fn host(&self) -> &HostRef {
        &self.host
    }

    pub fn name(&self) -> String {
        self.root.file_name().map_or_else(|| self.root.display().to_string(), |n| n.to_string_lossy().into_owned())
    }

    pub(crate) fn git(&self, args: &[&str]) -> Result<crate::git::Output> {
        run(&*self.host, &self.root, args)
    }

    /// Run git in the working tree, whatever its exit status.
    pub(crate) fn git_raw(&self, args: &[&str]) -> Result<crate::git::Output> {
        run_raw(&*self.host, &self.root, args, None)
    }

    // ----- Queries ---------------------------------------------------------------------

    pub fn status(&self) -> Result<Status> {
        let out = self.git(&["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=all"])?;
        status::parse(&out.stdout)
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let out = self.git(&["for-each-ref", branches::FORMAT, "refs/heads", "refs/remotes"])?;
        Ok(branches::parse(&out.stdout_str()))
    }

    /// How the current branch stands against its upstream, as of the last fetch; `None`
    /// when it has none (or HEAD is detached or unborn). Cheaper than [`Self::status`],
    /// which also scans the working tree.
    pub fn tracking(&self) -> Result<Option<Tracking>> {
        let out = self.git_raw(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])?;
        if !out.success {
            return Ok(None);
        }
        let text = out.stdout_str();
        let mut counts = text.split_whitespace().map(|n| n.parse().unwrap_or(0));
        Ok(Some(Tracking { ahead: counts.next().unwrap_or(0), behind: counts.next().unwrap_or(0) }))
    }

    /// Tags, newest first.
    pub fn tags(&self) -> Result<Vec<Tag>> {
        let out = self.git(&["for-each-ref", "--sort=-creatordate", tags::FORMAT, "refs/tags"])?;
        Ok(tags::parse(&out.stdout_str()))
    }

    /// Most recent commits reachable from HEAD, newest first, never a parent before its
    /// children (so a graph can be drawn from the list).
    pub fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        let n = format!("-n{limit}");
        let out = self.git_raw(&["log", "--date-order", &n, log::FORMAT, log::DATE, "HEAD"])?;
        if !out.success {
            // An unborn branch has no history; that is not an error.
            return if self.has_head()? { Err(git_error("log", &out)) } else { Ok(Vec::new()) };
        }
        let mut commits = log::parse(&out.stdout_str());

        let unpushed = self.git_raw(&["rev-list", "@{upstream}..HEAD"])?;
        if unpushed.success {
            let ids = unpushed.stdout_str();
            let ids: std::collections::HashSet<&str> = ids.lines().collect();
            for c in &mut commits {
                c.unpushed = ids.contains(c.id.as_str());
            }
        } else {
            // No upstream: everything on this branch is unpushed.
            commits.iter_mut().for_each(|c| c.unpushed = true);
        }
        Ok(commits)
    }

    /// Every directory of the working tree that holds a tracked or untracked (not ignored)
    /// file, as repository-relative paths with `/` separators, sorted. The root is not
    /// included.
    pub fn directories(&self) -> Result<Vec<String>> {
        let out = self.git(&["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?;
        let mut dirs = std::collections::BTreeSet::new();
        for path in out.stdout_str().split('\0') {
            let mut end = path.len();
            while let Some(slash) = path[..end].rfind('/') {
                if !dirs.insert(path[..slash].to_owned()) {
                    break; // its ancestors are in already
                }
                end = slash;
            }
        }
        Ok(dirs.into_iter().collect())
    }

    pub fn remotes(&self) -> Result<Vec<(String, String)>> {
        let out = self.git(&["remote", "-v"])?;
        let mut remotes: Vec<(String, String)> = out
            .stdout_str()
            .lines()
            .filter(|l| l.ends_with("(fetch)"))
            .filter_map(|l| {
                let mut parts = l.split_whitespace();
                Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
            })
            .collect();
        remotes.dedup();
        Ok(remotes)
    }

    /// The GitHub web page for the `origin` remote (or the first GitHub remote).
    pub fn github_url(&self) -> Option<String> {
        let remotes = self.remotes().ok()?;
        let origin = remotes.iter().find(|(name, _)| name == "origin");
        origin.into_iter().chain(remotes.iter()).find_map(|(_, url)| crate::github_web_url(url))
    }

    pub fn last_commit_message(&self) -> Result<String> {
        Ok(self.git(&["log", "-1", "--format=%B"])?.stdout_str().trim_end().to_owned())
    }

    /// What the next commit would record, as a stat summary followed by the patch: the
    /// staged changes, or with `amend` everything the amended commit would hold. `paths`
    /// are added first, as [`Repo::commit`] would add them, in a scratch copy of the index.
    pub fn commit_patch(&self, amend: bool, paths: &[String]) -> Result<String> {
        let base = amend && self.git(&["rev-parse", "-q", "--verify", "HEAD^"]).is_ok();
        let mut args = vec!["diff", "--cached", "--no-ext-diff", "--find-renames"];
        if base {
            args.push("HEAD^");
        }
        let stat_args = [&args[..], &["--stat=100"]].concat();
        if paths.is_empty() {
            let stat = self.git(&stat_args)?.stdout_str();
            return Ok(format!("{stat}\n{}", self.git(&args)?.stdout_str()));
        }
        let index = self.root.join(self.git(&["rev-parse", "--git-path", "index"])?.stdout_str().trim_end());
        let host = &*self.host;
        let scratch = host.info().temp_dir.join(format!(
            "snout-index-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
        ));
        let patch = (|| {
            if host.exists(&index) {
                host.copy(&index, &scratch)?;
            }
            run_in_index(host, &self.root, &scratch, &with_paths(&["add", "-A"], paths))?;
            let stat = run_in_index(host, &self.root, &scratch, &stat_args)?.stdout_str();
            Ok(format!("{stat}\n{}", run_in_index(host, &self.root, &scratch, &args)?.stdout_str()))
        })();
        let _ = host.remove_file(&scratch);
        patch
    }

    /// Full-file diff of one path. For renames, pass the entry so the old side uses the
    /// original path.
    pub fn file_diff(&self, entry: &FileEntry, base: DiffBase) -> Result<FileDiff> {
        let old_path = entry.orig_path.as_deref().unwrap_or(&entry.path);
        let (old, new) = match base {
            DiffBase::HeadToWorktree => (self.blob("HEAD", old_path)?, self.worktree_file(&entry.path)?),
            DiffBase::IndexToWorktree => (self.blob("", &entry.path)?, self.worktree_file(&entry.path)?),
            DiffBase::HeadToIndex => (self.blob("HEAD", old_path)?, self.blob("", &entry.path)?),
        };
        Ok(diff::compute(entry.path.clone(), Some(base), &old, &new))
    }

    /// One commit in full: message, author and committer, parents, and the files it
    /// changed relative to its first parent (renames detected).
    pub fn commit_details(&self, rev: &str) -> Result<CommitDetails> {
        let id = self.resolve_commit(rev)?;
        let out = self.git(&["show", "-s", log::DETAILS_FORMAT, log::DATE, &id])?;
        let mut details =
            log::parse_details(&out.stdout_str()).ok_or_else(|| Error::Invalid(format!("cannot read commit {id}")))?;
        let mut args = vec!["diff-tree", "-r", "-z", "-M", "--name-status", "--no-commit-id"];
        match details.parents.first() {
            Some(parent) => args.extend([parent.as_str(), &id]),
            None => args.extend(["--root", &id]),
        }
        details.files = log::parse_name_status(&self.git(&args)?.stdout_str());
        Ok(details)
    }

    /// What `file` looked like before and after `commit`. `parent` is the side to compare
    /// against (normally the first parent); `None` for a root commit.
    pub fn commit_file_diff(&self, commit: &str, parent: Option<&str>, file: &CommitFile) -> Result<FileDiff> {
        let old_path = file.orig_path.as_deref().unwrap_or(&file.path);
        let old = match parent {
            Some(parent) => self.blob(parent, old_path)?,
            None => Vec::new(),
        };
        let new = self.blob(commit, &file.path)?;
        Ok(diff::compute(file.path.clone(), None, &old, &new))
    }

    /// Contents of `path` at `rev` (`""` = index). Missing objects yield empty content.
    fn blob(&self, rev: &str, path: &str) -> Result<Vec<u8>> {
        let spec = format!("{rev}:{path}");
        let out = self.git_raw(&["cat-file", "blob", &spec])?;
        Ok(if out.success { out.stdout } else { Vec::new() })
    }

    fn worktree_file(&self, path: &str) -> Result<Vec<u8>> {
        let full = self.root.join(path);
        match self.host.metadata(&full) {
            Ok(m) if m.is_file() && m.len as usize > diff::MAX_BYTES => {
                // Report as too large without reading it all into memory.
                Ok(vec![b' '; diff::MAX_BYTES + 1])
            }
            Ok(m) if m.is_file() => Ok(self.host.read(&full)?),
            Ok(_) => Ok(Vec::new()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn has_head(&self) -> Result<bool> {
        Ok(self.git_raw(&["rev-parse", "--verify", "-q", "HEAD"])?.success)
    }

    // ----- Working tree operations -----------------------------------------------------

    pub fn stage(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        self.git(&with_paths(&["add", "-A"], paths))?;
        Ok(())
    }

    pub fn unstage(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        if self.has_head()? {
            self.git(&with_paths(&["restore", "--staged"], paths))?;
        } else {
            self.git(&with_paths(&["rm", "--cached", "-r", "-q"], paths))?;
        }
        Ok(())
    }

    /// Revert entries to their HEAD state. Untracked files are deleted; files that are
    /// only added to the index are un-added (and so become untracked) rather than deleted.
    pub fn discard(&self, entries: &[FileEntry]) -> Result<()> {
        let mut restore = Vec::new();
        let mut unadd = Vec::new();
        for e in entries {
            if e.is_untracked() {
                self.host.remove_file(&self.root.join(&e.path))?;
                continue;
            }
            match e.index {
                Some(crate::Change::Added) => unadd.push(e.path.clone()),
                Some(crate::Change::Renamed | crate::Change::Copied) => {
                    unadd.push(e.path.clone());
                    if let Some(orig) = &e.orig_path {
                        restore.push(orig.clone());
                    }
                }
                _ => restore.push(e.path.clone()),
            }
        }
        if !unadd.is_empty() {
            self.git(&with_paths(&["rm", "--cached", "-q", "-f"], &unadd))?;
        }
        if !restore.is_empty() {
            self.git(&with_paths(&["restore", "--source=HEAD", "--staged", "--worktree"], &restore))?;
        }
        Ok(())
    }

    /// Delete files from the working tree: to the Trash when `to_trash` and the host has one
    /// ([`can_trash`]), for good otherwise. Tracked files then show as
    /// deleted, ready to stage; files that were only added to the index are un-added too, so
    /// nothing is left staged for a file that is gone.
    pub fn delete_files(&self, entries: &[FileEntry], to_trash: bool) -> Result<()> {
        let host = &*self.host;
        let paths: Vec<PathBuf> =
            entries.iter().map(|e| self.root.join(&e.path)).filter(|p| host.symlink_metadata(p).is_ok()).collect();
        if paths.is_empty() {
            return Ok(());
        }
        if to_trash && can_trash(host) {
            host.trash(&paths).map_err(|e| Error::Invalid(format!("could not move to the Trash: {e}")))?;
        } else {
            for p in &paths {
                if host.symlink_metadata(p)?.is_dir() {
                    host.remove_dir_all(p)?;
                } else {
                    host.remove_file(p)?;
                }
            }
        }
        let unadd: Vec<String> =
            entries.iter().filter(|e| e.index == Some(crate::Change::Added)).map(|e| e.path.clone()).collect();
        if !unadd.is_empty() {
            self.git(&with_paths(&["rm", "--cached", "-q", "-f"], &unadd))?;
        }
        Ok(())
    }

    /// Commit the index. Returns git's summary output.
    /// Commit the index, after adding `paths` to it. If the commit fails, `paths` are
    /// unstaged again so the index is as it was.
    pub fn commit(&self, message: &str, amend: bool, paths: &[String]) -> Result<String> {
        if message.trim().is_empty() {
            return Err(Error::Invalid("commit message is empty".into()));
        }
        let mut args = vec!["commit", "-q", "-F", "-"];
        if amend {
            args.push("--amend");
        }
        self.stage(paths)?;
        if let Err(e) = run_with_stdin(&*self.host, &self.root, &args, Some(message.as_bytes())) {
            let _ = self.unstage(paths);
            return Err(e);
        }
        Ok(self.git(&["log", "-1", "--format=%h %s"])?.stdout_str().trim_end().to_owned())
    }

    // ----- Network operations (slow; call off the UI thread) ---------------------------

    pub fn fetch(&self) -> Result<String> {
        Ok(self.git(&["fetch", "--all", "--prune"])?.text())
    }

    /// [`Self::fetch`] with nobody watching: it never waits on a prompt or a stalled
    /// connection.
    pub fn fetch_unattended(&self) -> Result<()> {
        run_unattended(&*self.host, &self.root, &["fetch", "--all", "--prune", "--quiet"]).map(drop)
    }

    /// Push the current branch, setting the upstream on first push.
    pub fn push(&self) -> Result<String> {
        let status = self.status()?;
        let Some(head) = status.branch.head else {
            return Err(Error::Invalid("HEAD is detached; check out a branch to push".into()));
        };
        if status.branch.upstream.is_some() {
            return Ok(self.git(&["push"])?.text());
        }
        let remotes = self.remotes()?;
        let remote = remotes
            .iter()
            .find(|(n, _)| n == "origin")
            .or(remotes.first())
            .map(|(n, _)| n.clone())
            .ok_or_else(|| Error::Invalid("no remote configured".into()))?;
        Ok(self.git(&["push", "-u", &remote, &head])?.text())
    }

    /// Delete `branch` on `remote` (`git push <remote> --delete <branch>`).
    pub fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<String> {
        Ok(self.git(&["push", remote, "--delete", branch])?.text())
    }
}

/// Whether [`Repo::delete_files`] can move files on `host` to the Trash rather than delete
/// them for good.
pub fn can_trash(host: &dyn Host) -> bool {
    host.info().can_trash
}

fn with_paths<'a>(args: &[&'a str], paths: &'a [String]) -> Vec<&'a str> {
    let mut v = args.to_vec();
    v.push("--");
    v.extend(paths.iter().map(String::as_str));
    v
}

fn git_error(args: &str, out: &crate::git::Output) -> Error {
    Error::Git { args: args.into(), message: out.text() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_run as run;
    use std::fs;

    fn scratch_repo(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("snout-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q", "-b", "main"]).unwrap();
        run(&dir, &["config", "user.name", "Test"]).unwrap();
        run(&dir, &["config", "user.email", "test@example.com"]).unwrap();
        run(&dir, &["config", "commit.gpgsign", "false"]).unwrap();
        Repo::open(&snout_host::local(), &dir).unwrap()
    }

    #[test]
    fn full_local_workflow() {
        let repo = scratch_repo("workflow");
        assert!(repo.log(10).unwrap().is_empty(), "unborn branch has empty log");

        fs::write(repo.root().join("a.txt"), "one\n").unwrap();
        let st = repo.status().unwrap();
        assert!(st.branch.is_unborn());
        assert_eq!(st.files.len(), 1);
        assert!(st.files[0].is_untracked());

        repo.stage(&["a.txt".into()]).unwrap();
        repo.unstage(&["a.txt".into()]).unwrap(); // unborn path: rm --cached
        repo.stage(&["a.txt".into()]).unwrap();
        assert_eq!(repo.status().unwrap().staged_count(), 1);

        let summary = repo.commit("first", false, &[]).unwrap();
        assert!(summary.ends_with("first"));
        assert!(repo.status().unwrap().files.is_empty());

        fs::write(repo.root().join("a.txt"), "one\ntwo\n").unwrap();
        let st = repo.status().unwrap();
        let d = repo.file_diff(&st.files[0], DiffBase::HeadToWorktree).unwrap();
        assert_eq!((d.additions, d.deletions), (1, 0));
        assert!(repo.file_diff(&st.files[0], DiffBase::HeadToIndex).unwrap().is_identical());

        repo.discard(&st.files).unwrap();
        assert!(repo.status().unwrap().files.is_empty());
        assert_eq!(fs::read_to_string(repo.root().join("a.txt")).unwrap(), "one\n");

        fs::create_dir_all(repo.root().join("src/deep/er")).unwrap();
        fs::write(repo.root().join("src/deep/er/x.rs"), "").unwrap();
        fs::write(repo.root().join(".gitignore"), "ignored/\n").unwrap();
        fs::create_dir_all(repo.root().join("ignored")).unwrap();
        fs::write(repo.root().join("ignored/y"), "").unwrap();
        assert_eq!(repo.directories().unwrap(), ["src", "src/deep", "src/deep/er"]);
        fs::remove_dir_all(repo.root().join("src")).unwrap();
        fs::remove_file(repo.root().join(".gitignore")).unwrap();
        fs::remove_dir_all(repo.root().join("ignored")).unwrap();

        repo.create_branch("feature/x", None, true).unwrap();
        let branches = repo.branches().unwrap();
        assert!(branches.iter().any(|b| b.name == "feature/x" && b.is_head));
        let main = branches.iter().find(|b| b.name == "main").unwrap();
        repo.checkout(&main.into(), crate::LocalChanges::Keep).unwrap();
        assert_eq!(repo.status().unwrap().branch.head.as_deref(), Some("main"));

        fs::write(repo.root().join("a.txt"), "one\nTWO\n").unwrap();
        fs::write(repo.root().join("b.txt"), "bee\n").unwrap();
        repo.stage(&["a.txt".into(), "b.txt".into()]).unwrap();
        repo.commit("second\n\nwith a body", false, &[]).unwrap();
        let details = repo.commit_details("HEAD").unwrap();
        assert_eq!((details.summary(), details.body()), ("second", "with a body"));
        assert_eq!(details.parents.len(), 1);
        let paths: Vec<&str> = details.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.txt", "b.txt"]);
        let d = repo.commit_file_diff(&details.id, Some(&details.parents[0]), &details.files[0]).unwrap();
        assert_eq!((d.additions, d.deletions), (1, 0));
        let root = repo.commit_details(&details.parents[0]).unwrap();
        assert!(root.parents.is_empty());
        assert_eq!(root.files.len(), 1, "a root commit lists what it added");
        assert_eq!(root.files[0].change, crate::Change::Added);

        let log = repo.log(10).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].parents, [log[1].id.clone()]);
        assert!(log[0].unpushed, "no upstream means unpushed");
        assert!(serde_json::to_string(&log).is_ok());

        fs::remove_dir_all(repo.root()).unwrap();
    }

    #[test]
    fn delete_files_leaves_tracked_ones_deleted_and_unadds_new_ones() {
        let repo = scratch_repo("delete");
        fs::write(repo.root().join("kept.txt"), "one\n").unwrap();
        repo.stage(&["kept.txt".into()]).unwrap();
        repo.commit("first", false, &[]).unwrap();
        fs::create_dir_all(repo.root().join("dir")).unwrap();
        fs::write(repo.root().join("dir/new.txt"), "new\n").unwrap();
        fs::write(repo.root().join("added.txt"), "added\n").unwrap();
        repo.stage(&["added.txt".into()]).unwrap();

        let files = repo.status().unwrap().files;
        assert_eq!(files.len(), 2);
        let tracked = FileEntry {
            path: "kept.txt".into(),
            orig_path: None,
            index: None,
            worktree: Some(crate::Change::Modified),
            conflicted: false,
        };
        let all: Vec<FileEntry> = files.into_iter().chain([tracked]).collect();
        repo.delete_files(&all, false).unwrap();

        let st = repo.status().unwrap();
        assert_eq!(st.files.len(), 1, "{:?}", st.files);
        assert_eq!(st.files[0].path, "kept.txt");
        assert_eq!(st.files[0].worktree, Some(crate::Change::Deleted));
        assert!(!repo.root().join("dir/new.txt").exists() && !repo.root().join("added.txt").exists());

        fs::remove_dir_all(repo.root()).unwrap();
    }

    #[test]
    fn commit_patch_holds_what_the_commit_would() {
        let repo = scratch_repo("patch");
        fs::write(repo.root().join("a.txt"), "one\n").unwrap();
        repo.stage(&["a.txt".into()]).unwrap();
        assert!(repo.commit_patch(false, &[]).unwrap().contains("+one"), "staged, before the first commit");
        repo.commit("first", false, &[]).unwrap();
        fs::write(repo.root().join("a.txt"), "one\ntwo\n").unwrap();
        repo.stage(&["a.txt".into()]).unwrap();
        repo.commit("second", false, &[]).unwrap();
        fs::write(repo.root().join("b.txt"), "bee\n").unwrap();
        repo.stage(&["b.txt".into()]).unwrap();
        fs::write(repo.root().join("a.txt"), "unstaged\n").unwrap();

        let next = repo.commit_patch(false, &[]).unwrap();
        assert!(next.contains("b.txt | 1 +") && next.contains("+bee"), "{next}");
        assert!(!next.contains("two") && !next.contains("unstaged"), "{next}");
        let amended = repo.commit_patch(true, &[]).unwrap();
        assert!(amended.contains("+two") && amended.contains("+bee") && !amended.contains("unstaged"), "{amended}");

        // Picked files, untracked ones too, without touching the real index.
        fs::write(repo.root().join("c.txt"), "sea\n").unwrap();
        let picked = repo.commit_patch(false, &["a.txt".into(), "c.txt".into()]).unwrap();
        assert!(picked.contains("+unstaged") && picked.contains("+sea") && picked.contains("+bee"), "{picked}");
        let st = repo.status().unwrap();
        assert_eq!(st.staged_count(), 1, "the index is untouched");

        // Committing them adds them first.
        repo.commit("third", false, &["a.txt".into(), "c.txt".into()]).unwrap();
        assert!(repo.status().unwrap().files.is_empty());
        // A failed commit leaves them unstaged again.
        fs::write(repo.root().join("d.txt"), "dee\n").unwrap();
        run(repo.root(), &["config", "core.hooksPath", ".hooks"]).unwrap();
        fs::create_dir_all(repo.root().join(".hooks")).unwrap();
        fs::write(repo.root().join(".hooks/pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
        std::process::Command::new("chmod").arg("+x").arg(repo.root().join(".hooks/pre-commit")).status().unwrap();
        assert!(repo.commit("fourth", false, &["d.txt".into()]).is_err());
        assert_eq!(repo.status().unwrap().staged_count(), 0);

        fs::remove_dir_all(repo.root()).unwrap();
    }

    #[test]
    fn push_sets_upstream_then_fetch_and_pull() {
        let repo = scratch_repo("push");
        let remote = repo.root().with_extension("remote.git");
        let _ = fs::remove_dir_all(&remote);
        run(repo.root(), &["init", "-q", "--bare", "-b", "main", &remote.to_string_lossy()]).unwrap();
        run(repo.root(), &["remote", "add", "origin", &remote.to_string_lossy()]).unwrap();

        fs::write(repo.root().join("a.txt"), "one\n").unwrap();
        repo.stage(&["a.txt".into()]).unwrap();
        repo.commit("first", false, &[]).unwrap();
        assert_eq!(repo.status().unwrap().branch.upstream, None);
        assert_eq!(repo.tracking().unwrap(), None);

        repo.push().unwrap();
        let st = repo.status().unwrap();
        assert_eq!(st.branch.upstream.as_deref(), Some("origin/main"));
        assert!(repo.log(10).unwrap().iter().all(|c| !c.unpushed));

        assert_eq!(repo.tracking().unwrap(), Some(Tracking::default()));
        // Another clone pushes; a fetch makes it a commit to pull.
        let other = repo.root().with_extension("other");
        let _ = fs::remove_dir_all(&other);
        run(repo.root(), &["clone", "-q", &remote.to_string_lossy(), &other.to_string_lossy()]).unwrap();
        run(
            &other,
            &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "theirs"],
        )
        .unwrap();
        run(&other, &["push", "-q"]).unwrap();
        assert_eq!(repo.tracking().unwrap().unwrap().behind, 0, "not fetched yet");
        repo.fetch_unattended().unwrap();
        assert_eq!(repo.tracking().unwrap(), Some(Tracking { ahead: 0, behind: 1 }));
        fs::remove_dir_all(&other).unwrap();

        repo.fetch().unwrap();
        repo.pull().unwrap();
        let remote_branch = repo.branches().unwrap().into_iter().find(|b| b.kind == crate::BranchKind::Remote).unwrap();
        assert_eq!(remote_branch.name, "origin/main");

        // A branch pushed, then deleted on the remote.
        repo.create_branch("topic", None, false).unwrap();
        run(repo.root(), &["push", "-q", "origin", "topic"]).unwrap();
        assert!(repo.branches().unwrap().iter().any(|b| b.name == "origin/topic"));
        repo.delete_remote_branch("origin", "topic").unwrap();
        assert!(!repo.branches().unwrap().iter().any(|b| b.name == "origin/topic"));
        assert!(repo.branches().unwrap().iter().any(|b| b.name == "topic"), "local branch stays");

        fs::remove_dir_all(repo.root()).unwrap();
        fs::remove_dir_all(&remote).unwrap();
    }
}
