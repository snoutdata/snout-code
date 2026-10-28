//! The Git Panel's state and everything it does, with no drawing: the GPUI counterpart of
//! the egui app's `SnoutApp` minus its panes. Views read it and send it [`Action`]s.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{Context, EventEmitter, PathPromptOptions, Task};
use snout_core::claude::{Draft, draft_prompt, find_claude};
use snout_core::diff_layout::DiffLayout;
use snout_core::graph::{GraphRow, journal_matches};
use snout_core::groups::{RepoGroup, dir_name, move_to_group, save_group};
use snout_core::{
    Branch, BranchKind, CheckoutTarget, Commit, CommitDetails, CommitFile, FileEntry, LocalChanges, PullMode,
    PullProblem, Repo, Scope, Status, Tag, Tracking, on_disk,
};

use crate::settings::{Settings, Store, ThemeChoice};

pub const LOG_LIMIT: usize = 300;
/// How deep "Find repositories in folder…" searches.
pub const SCAN_DEPTH: usize = 4;
/// `path:12` or `path:12:5` as the path and a line and column from 0.
pub fn split_line_suffix(arg: &str) -> Option<(PathBuf, (u32, u32))> {
    let (rest, last) = arg.rsplit_once(':')?;
    let last: u32 = last.parse().ok()?;
    match rest.rsplit_once(':').and_then(|(p, l)| l.parse::<u32>().ok().map(|l| (p, l))) {
        Some((path, line)) => Some((PathBuf::from(path), (line.max(1) - 1, last.max(1) - 1))),
        None => Some((PathBuf::from(rest), (last.max(1) - 1, 0))),
    }
}

/// The projects opened lately, most recent first.
pub fn recent_projects(settings: &crate::settings::Settings) -> Vec<PathBuf> {
    match &settings.recent {
        Some(recent) => recent.clone(),
        None => settings.repos.iter().rev().take(RECENT_LIMIT).cloned().collect(),
    }
}

/// How many projects File ▸ Open Recent lists.
pub const RECENT_LIMIT: usize = 12;

/// A branch on a remote: `origin` and `feature/x` for `origin/feature/x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRef {
    pub remote: String,
    pub branch: String,
}

impl RemoteRef {
    pub fn of(b: &Branch) -> Option<Self> {
        Some(Self { remote: b.remote.clone()?, branch: b.local_name().to_owned() })
    }

    pub fn label(&self) -> String {
        format!("{}/{}", self.remote, self.branch)
    }
}

/// Where a new branch starts. `None` means HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPoint {
    pub label: String,
    pub rev: String,
}

/// The column the Git Panel sorts its changed files by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortColumn {
    Name,
    State,
    Directory,
}

/// How an [`Dialog::Info`] message feels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Done,
    Empty,
    Warning,
}

/// A question or a message the workspace puts up.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    About,
    Error(String),
    Info { title: String, text: String, mood: Mood },
    ConfirmDiscard(Vec<FileEntry>),
    ConfirmCheckout(CheckoutTarget),
    Checkout,
    NewBranch(Option<StartPoint>),
    DeleteBranch { name: String, upstream: Option<RemoteRef> },
    DeleteRemoteBranch(RemoteRef),
    ConfirmDeleteFiles(Vec<FileEntry>),
    PullProblem { problem: PullProblem, details: String },
    InitRepo(PathBuf),
    GroupName { rename: Option<String>, repo: Option<PathBuf> },
    Clone { parent: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Refresh,
    OpenRepo(PathBuf),
    PickFolder,
    OpenPath(PathBuf),
    InitRepo(PathBuf),
    PickScanFolder,
    Scan(PathBuf),
    RemoveRepo(PathBuf),
    ShowNewGroup(Option<PathBuf>),
    ShowRenameGroup(String),
    SaveGroup {
        rename: Option<String>,
        name: String,
        repo: Option<PathBuf>,
    },
    MoveToGroup(PathBuf, Option<String>),
    DeleteGroup(String),
    ShowClone,
    Clone {
        url: String,
        dest: PathBuf,
    },
    Stage(Vec<String>),
    Unstage(Vec<String>),
    AskDiscard(Vec<FileEntry>),
    Discard(Vec<FileEntry>),
    AskDeleteFiles(Vec<FileEntry>),
    DeleteFiles(Vec<FileEntry>),
    Commit,
    DraftMessage,
    CancelDraft,
    ShowCheckout,
    RequestCheckout(CheckoutTarget),
    Checkout(CheckoutTarget, LocalChanges),
    ShowNewBranch(Option<StartPoint>),
    CreateBranch {
        name: String,
        start: Option<String>,
        checkout: bool,
    },
    AskDeleteBranch(Branch),
    DeleteBranch {
        name: String,
        force: bool,
        upstream: Option<RemoteRef>,
    },
    DeleteRemoteBranch(RemoteRef),
    Fetch,
    Pull(PullMode),
    ResetToUpstream,
    Push,
    OpenGithub,
    ToggleConsole,
    ShowCommit(String),
    CloseCommit,
    SetTheme(ThemeChoice),
    Reveal(PathBuf),
    Copy(String),
    /// Narrow the changed files to a directory, or show them all.
    FilterDir(Option<String>),
    /// Open a file in an editor tab, the caret on a line and column (from 0) if given.
    OpenFile(PathBuf, Option<(u32, u32)>),
    /// Show a file in the preview tab, which the next preview replaces until it is kept.
    PreviewFile(PathBuf),
    /// Close the open project: no repository is open afterwards.
    CloseRepo,
    /// Empty the recent projects list.
    ClearRecent,
    /// Put up the New Project dialog.
    ShowNewProject,
}

/// What the workspace should do in response to the model.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelEvent {
    /// A dialog is waiting in [`Model::dialog`].
    Dialog,
    /// The commit view has a commit to show.
    ShowCommit,
    /// Show the diff of the selected file.
    ShowDiff,
    ToggleConsole,
    Theme,
    /// The commit message changed from outside its text field.
    Message,
    /// Open a file in an editor tab (the preview tab when the flag says so).
    OpenFile(PathBuf, Option<(u32, u32)>, bool),
    /// The project was closed.
    Closed,
    /// Put up the New Project dialog.
    NewProject,
    /// Another project is to open in place of the one open: the workspace settles the
    /// open tabs first, then calls [`Model::switch_project`].
    Switch(PathBuf),
}

pub struct OutputLine {
    pub text: String,
    pub error: bool,
}

/// A commit shown in its own tab, after a click in the Journal.
pub struct CommitView {
    pub details: Result<CommitDetails, String>,
    pub selected: Option<String>,
    pub diff: Option<Result<std::rc::Rc<DiffLayout>, String>>,
}

impl CommitView {
    pub fn id(&self) -> Option<&str> {
        self.details.as_ref().ok().map(|d| d.id.as_str())
    }

    pub fn selected_file(&self) -> Option<&CommitFile> {
        let details = self.details.as_ref().ok()?;
        details.files.iter().find(|f| Some(&f.path) == self.selected.as_ref())
    }
}

enum Outcome {
    Done(String),
    Cloned(Repo, String),
    Found { root: PathBuf, repos: Vec<PathBuf> },
}

pub struct Model {
    pub settings: Settings,
    store: Option<Store>,
    saved: Settings,
    pub repo: Option<Repo>,
    /// The folder opened as the project, as it was opened: the repository's root or any
    /// folder inside it. Git works on the repository found at or above it.
    pub project: Option<PathBuf>,
    pub status: Status,
    pub branches: Vec<Branch>,
    pub tags: Vec<Tag>,
    pub log: Vec<Commit>,
    pub graph: Vec<GraphRow>,
    pub github_url: Option<String>,
    pub dirs: Vec<String>,
    pub dir_filter: Option<String>,
    pub repo_filter: String,
    pub selected: BTreeSet<String>,
    pub anchor: Option<String>,
    pub filter: String,
    /// The changed files' sort column, and whether it is ascending.
    pub sort: (SortColumn, bool),
    pub diff: Option<Result<std::rc::Rc<DiffLayout>, String>>,
    diff_key: Option<(String, snout_core::DiffBase, bool)>,
    pub commit_msg: String,
    pub amend: bool,
    pub draft: Option<Draft>,
    _draft_poll: Option<Task<()>>,
    pub journal_filter: String,
    pub commit_view: Option<CommitView>,
    pub output: Vec<OutputLine>,
    pub message: String,
    /// The label of the slow operation running, if any.
    pub job: Option<String>,
    _job_task: Option<Task<()>>,
    pub dialog: Option<Dialog>,
    pub tracking: HashMap<PathBuf, Option<Tracking>>,
    /// A survey pass is running; the flag holds its fetches while a job runs.
    surveying: bool,
    survey_hold: Arc<AtomicBool>,
    _survey: Option<Task<()>>,
    fetched_at: Option<Instant>,
    /// How often a survey fetches; `None` in tests.
    pub fetch_every: Option<Duration>,
    /// The host the projects are on.
    pub host: snout_core::HostRef,
    /// The `claude` to run; looked up when `None`.
    pub claude: Option<PathBuf>,
    /// A file given on the command line, to open once the window is there.
    pub pending_file: Option<(PathBuf, Option<(u32, u32)>)>,
    /// Opening another project asks the workspace first ([`ModelEvent::Switch`]); off
    /// without a window, as in the model's own tests.
    pub confirm_switch: bool,
}

impl EventEmitter<ModelEvent> for Model {}

impl Model {
    pub fn new(settings: Settings, store: Option<Store>, cx: &mut Context<Self>) -> Self {
        let mut model = Self {
            host: crate::host::host(cx),
            pending_file: None,
            confirm_switch: false,
            saved: settings.clone(),
            settings,
            store,
            repo: None,
            project: None,
            status: Status::default(),
            branches: Vec::new(),
            tags: Vec::new(),
            log: Vec::new(),
            graph: Vec::new(),
            github_url: None,
            dirs: Vec::new(),
            dir_filter: None,
            repo_filter: String::new(),
            selected: BTreeSet::new(),
            anchor: None,
            filter: String::new(),
            sort: (SortColumn::Name, true),
            diff: None,
            diff_key: None,
            commit_msg: String::new(),
            amend: false,
            draft: None,
            _draft_poll: None,
            journal_filter: String::new(),
            commit_view: None,
            output: Vec::new(),
            message: "Ready".into(),
            job: None,
            _job_task: None,
            dialog: None,
            tracking: HashMap::new(),
            surveying: false,
            survey_hold: Arc::new(AtomicBool::new(false)),
            _survey: None,
            fetched_at: None,
            fetch_every: None,
            claude: None,
        };
        if let Some(path) = model.settings.current.clone() {
            model.open_repo(&path, cx);
        }
        model
    }

    // ----- Reading --------------------------------------------------------------------

    /// The open project's folder: the folder opened, else the repository's root.
    pub fn project_root(&self) -> Option<PathBuf> {
        self.project.clone().or_else(|| self.repo.as_ref().map(|r| r.root().to_owned()))
    }

    /// The root of the repository the project is in.
    pub fn repo_root(&self) -> Option<PathBuf> {
        self.repo.as_ref().map(|r| r.root().to_owned())
    }

    pub fn selected_entries(&self) -> Vec<FileEntry> {
        self.status.files.iter().filter(|f| self.selected.contains(&f.path)).cloned().collect()
    }

    pub fn commit_scope(&self) -> Scope {
        Scope::of(&self.status, &self.selected_entries(), self.amend)
    }

    pub fn can_commit(&self) -> bool {
        self.repo.is_some()
            && self.draft.is_none()
            && !self.commit_msg.trim().is_empty()
            && self.commit_scope() != Scope::Nothing
    }

    pub fn can_draft(&self) -> bool {
        self.repo.is_some() && self.draft.is_none() && self.commit_scope() != Scope::Nothing
    }

    fn has_tracked_changes(&self) -> bool {
        self.status.files.iter().any(|f| !f.is_untracked())
    }

    pub fn head_label(&self) -> String {
        let b = &self.status.branch;
        match (&b.head, &b.oid) {
            (Some(head), _) => head.clone(),
            (None, Some(oid)) => format!("detached at {}", &oid[..oid.len().min(7)]),
            (None, None) => "detached HEAD".into(),
        }
    }

    /// The status bar's branch summary: `main → origin/main  ↑1  ↓2`.
    pub fn branch_summary(&self) -> Option<String> {
        self.repo.as_ref()?;
        let b = &self.status.branch;
        let mut s = self.head_label();
        if let Some(up) = &b.upstream {
            s += &format!(" → {up}");
            if b.ahead > 0 {
                s += &format!("  ↑{}", b.ahead);
            }
            if b.behind > 0 {
                s += &format!("  ↓{}", b.behind);
            }
        } else if b.head.is_some() {
            s += "  (no upstream)";
        }
        Some(s)
    }

    pub fn title(&self) -> String {
        match (&self.repo, self.project_root()) {
            (Some(_), Some(project)) => format!("{} ({}) · SnoutCode", dir_name(&project), self.head_label()),
            _ => "SnoutCode".into(),
        }
    }

    /// The changed files the Git Panel lists: filtered by text and directory, then sorted.
    pub fn visible_files(&self) -> Vec<&FileEntry> {
        let needle = self.filter.to_lowercase();
        let mut rows: Vec<&FileEntry> = self
            .status
            .files
            .iter()
            .filter(|f| needle.is_empty() || f.path.to_lowercase().contains(&needle))
            .filter(|f| {
                self.dir_filter
                    .as_ref()
                    .is_none_or(|d| f.path.strip_prefix(d.as_str()).is_some_and(|rest| rest.starts_with('/')))
            })
            .collect();
        let (col, asc) = self.sort;
        rows.sort_by(|a, b| {
            let ord = match col {
                SortColumn::Name => a.file_name().cmp(b.file_name()).then(a.path.cmp(&b.path)),
                SortColumn::State => a.state_label().cmp(&b.state_label()).then(a.path.cmp(&b.path)),
                SortColumn::Directory => a.directory().cmp(b.directory()).then(a.file_name().cmp(b.file_name())),
            };
            if asc { ord } else { ord.reverse() }
        });
        rows
    }

    /// A click on a column header: sort by it, or flip the order when it already sorts.
    pub fn sort_by(&mut self, col: SortColumn, cx: &mut Context<Self>) {
        self.sort = if self.sort.0 == col { (col, !self.sort.1) } else { (col, true) };
        cx.notify();
    }

    /// Indices into `log` of the commits the Journal shows, after its filter.
    pub fn journal_rows(&self) -> Vec<usize> {
        let needle = self.journal_filter.trim().to_lowercase();
        (0..self.log.len()).filter(|&i| journal_matches(&self.log[i], &needle)).collect()
    }

    // ----- Selection ------------------------------------------------------------------

    /// A click on a changed file, with ⇧ extending and ⌘ toggling, as in Finder.
    pub fn click_file(&mut self, path: &str, shift: bool, command: bool, cx: &mut Context<Self>) {
        let rows: Vec<String> = self.visible_files().iter().map(|f| f.path.clone()).collect();
        let i = rows.iter().position(|p| p == path).unwrap_or(0);
        if shift {
            let anchor = self.anchor.as_ref().and_then(|a| rows.iter().position(|p| p == a)).unwrap_or(i);
            let (lo, hi) = (anchor.min(i), anchor.max(i));
            if !command {
                self.selected.clear();
            }
            self.selected.extend(rows[lo..=hi].iter().cloned());
        } else if command {
            if !self.selected.remove(path) {
                self.selected.insert(path.to_owned());
            }
            self.anchor = Some(path.to_owned());
        } else {
            self.selected = BTreeSet::from([path.to_owned()]);
            self.anchor = Some(path.to_owned());
        }
        self.update_diff();
        if self.selected.len() == 1 {
            cx.emit(ModelEvent::ShowDiff);
        }
        cx.notify();
    }

    pub fn select_all_files(&mut self, cx: &mut Context<Self>) {
        self.selected = self.visible_files().iter().map(|f| f.path.clone()).collect();
        self.update_diff();
        cx.notify();
    }

    pub fn set_amend(&mut self, amend: bool, cx: &mut Context<Self>) {
        let had = self.amend;
        self.amend = amend && !self.log.is_empty();
        if self.amend
            && !had
            && self.commit_msg.trim().is_empty()
            && let Some(repo) = &self.repo
        {
            self.commit_msg = repo.last_commit_message().unwrap_or_default();
            cx.emit(ModelEvent::Message);
        }
        cx.notify();
    }

    pub fn set_diff_options(&mut self, side_by_side: Option<bool>, compact: Option<bool>, cx: &mut Context<Self>) {
        if let Some(s) = side_by_side {
            self.settings.side_by_side = s;
        }
        if let Some(c) = compact {
            self.settings.compact = c;
        }
        self.update_diff();
        self.update_commit_diff();
        self.persist();
        cx.notify();
    }

    pub fn set_diff_base(&mut self, base: snout_core::DiffBase, cx: &mut Context<Self>) {
        self.settings.diff_base = base;
        self.update_diff();
        self.persist();
        cx.notify();
    }

    /// In the commit view, step to the commit above (`up`) or below in the Journal as it is
    /// filtered; with the shown commit filtered out, start at the top.
    pub fn step_commit(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(view) = &self.commit_view else { return };
        let visible = self.journal_rows();
        let at = visible.iter().position(|&i| Some(self.log[i].id.as_str()) == view.id());
        let next = match at {
            Some(at) if up => at.checked_sub(1),
            Some(at) => Some(at + 1),
            None => Some(0),
        };
        if let Some(&i) = next.and_then(|n| visible.get(n)) {
            let id = self.log[i].id.clone();
            self.handle(Action::ShowCommit(id), cx);
        }
    }

    pub fn select_commit_file(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(view) = &mut self.commit_view {
            view.selected = Some(path);
        }
        self.update_commit_diff();
        cx.notify();
    }

    // ----- State ----------------------------------------------------------------------

    /// Open the folder `path` as the project, with the repository found at or above it.
    /// False when the workspace is asked first (another project is open).
    fn open_repo(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        if self.confirm_switch && self.repo.is_some() {
            let project = self.host.canonicalize(path).unwrap_or_else(|_| path.to_owned());
            if self.project.as_ref() != Some(&project) && Repo::open(&self.host, path).is_ok() {
                cx.emit(ModelEvent::Switch(path.to_owned()));
                return false;
            }
        }
        match Repo::open(&self.host, path) {
            Ok(repo) => {
                let root = repo.root().to_owned();
                // The folder as opened, in the form Git gives the root (symbolic links
                // resolved), so paths under the project and under the repository compare.
                let project = self.host.canonicalize(path).unwrap_or_else(|_| path.to_owned());
                self.remember(&root, &project);
                self.settings.current = Some(project.clone());
                self.project = Some(project);
                self.github_url = repo.github_url();
                self.repo = Some(repo);
                self.selected.clear();
                self.anchor = None;
                self.dir_filter = None;
                self.commit_view = None;
                self.journal_filter.clear();
                self.commit_msg.clear();
                self.amend = false;
                self.draft = None;
                self.refresh();
                self.message = format!("Opened {}", path.display());
                cx.emit(ModelEvent::Message);
                // Whatever follows the project (its engine, runs, models) sees the change now.
                cx.notify();
                true
            }
            Err(e) => {
                self.fail(e.to_string(), cx);
                false
            }
        }
    }

    /// Open `path` in place of the open project, once the workspace has settled its tabs.
    pub fn switch_project(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        let confirm = std::mem::replace(&mut self.confirm_switch, false);
        let opened = self.open_repo(path, cx);
        self.confirm_switch = confirm;
        opened
    }

    /// Open `path` as the command line names it: `File.java:12` or `File.java:12:5` puts the
    /// caret on that line (and column), from 1.
    pub fn open_arg(&mut self, arg: &Path, cx: &mut Context<Self>) {
        if !self.host.exists(arg)
            && let Some((path, at)) = split_line_suffix(&arg.to_string_lossy())
            && self.host.is_file(&path)
        {
            self.open_path(&path, cx);
            if let Some(pending) = self.pending_file.as_mut() {
                pending.1 = Some(at);
            }
            return;
        }
        self.open_path(arg, cx);
    }

    fn open_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        let is_file = self.host.is_file(path);
        let dir = if is_file { path.parent().unwrap_or(path) } else { path };
        if let Ok(repo) = Repo::open(&self.host, dir) {
            // A folder is the project as it is; a file opens in its repository.
            let project = if is_file { repo.root().to_owned() } else { dir.to_owned() };
            let opened = self.open_repo(&project, cx);
            // A file opens in a tab of its repository (once the window is there, or once
            // the switch to its project is done).
            if is_file {
                self.pending_file = Some((path.to_owned(), None));
                if opened {
                    cx.emit(ModelEvent::OpenFile(path.to_owned(), None, false));
                }
            }
        } else if self.host.is_dir(dir) {
            self.ask(Dialog::InitRepo(dir.to_owned()), cx);
        } else {
            self.fail(format!("{} does not exist", dir.display()), cx);
        }
    }

    /// Keep the repository at `root` in the list, and `project` (the folder opened) first
    /// among the recent projects.
    fn remember(&mut self, root: &Path, project: &Path) {
        if !self.settings.repos.iter().any(|p| p == root) {
            self.settings.repos.push(root.to_owned());
        }
        let mut recent = self.recent_projects();
        recent.retain(|p| p != project);
        recent.insert(0, project.to_owned());
        recent.truncate(RECENT_LIMIT);
        self.settings.recent = Some(recent);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        self.settings.opened.insert(project.to_owned(), now);
    }

    /// The folder the app keeps its settings in, when it keeps them (not in tests).
    pub fn support_dir(&self) -> Option<PathBuf> {
        self.store.as_ref().and_then(|s| s.file.parent().map(Path::to_owned))
    }

    /// The projects opened lately, most recent first.
    pub fn recent_projects(&self) -> Vec<PathBuf> {
        recent_projects(&self.settings)
    }

    /// Close the open project.
    fn close_repo(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.take() else { return };
        self.project = None;
        self.settings.current = None;
        self.github_url = None;
        self.selected.clear();
        self.anchor = None;
        self.dir_filter = None;
        self.commit_view = None;
        self.draft = None;
        self.refresh();
        self.message = format!("Closed {}", repo.root().display());
        cx.emit(ModelEvent::Closed);
        cx.notify();
    }

    pub fn refresh(&mut self) {
        self.diff_key = None;
        let Some(repo) = self.repo.clone() else {
            self.status = Status::default();
            self.branches.clear();
            self.tags.clear();
            self.log.clear();
            self.graph.clear();
            self.dirs.clear();
            self.commit_view = None;
            self.diff = None;
            return;
        };
        match repo.status() {
            Ok(status) => {
                let present: BTreeSet<&String> = status.files.iter().map(|f| &f.path).collect();
                self.selected.retain(|p| present.contains(p));
                self.status = status;
            }
            Err(e) => {
                self.log_output(e.to_string(), true);
                return;
            }
        }
        let b = &self.status.branch;
        let tracking = b.upstream.is_some().then_some(Tracking { ahead: b.ahead, behind: b.behind });
        self.tracking.insert(repo.root().to_owned(), tracking);
        let mut errors = Vec::new();
        self.branches = repo.branches().map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        self.tags = repo.tags().map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        self.log = repo.log(LOG_LIMIT).map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        self.dirs = repo.directories().map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        for e in errors {
            self.log_output(e, true);
        }
        self.graph = snout_core::graph::layout(&self.log);
        if self.dir_filter.as_ref().is_some_and(|d| self.dirs.binary_search(d).is_err()) {
            self.dir_filter = None;
        }
        self.update_diff();
        self.persist();
    }

    fn update_diff(&mut self) {
        let entry = (self.selected.len() == 1)
            .then(|| self.status.files.iter().find(|f| self.selected.contains(&f.path)))
            .flatten();
        let Some(entry) = entry else {
            self.diff_key = None;
            self.diff = None;
            return;
        };
        let key = (entry.path.clone(), self.settings.diff_base, self.settings.compact);
        if self.diff_key.as_ref() == Some(&key) {
            return;
        }
        let Some(repo) = &self.repo else { return };
        self.diff = Some(
            repo.file_diff(entry, self.settings.diff_base)
                .map(|d| std::rc::Rc::new(DiffLayout::new(d, self.settings.compact)))
                .map_err(|e| e.to_string()),
        );
        self.diff_key = Some(key);
    }

    fn update_commit_diff(&mut self) {
        let compact = self.settings.compact;
        let Some(view) = &mut self.commit_view else { return };
        let Some(file) = view.selected_file().cloned() else {
            view.diff = None;
            return;
        };
        let (Some(repo), Ok(details)) = (&self.repo, &view.details) else { return };
        let parent = details.parents.first().map(String::as_str);
        view.diff = Some(
            repo.commit_file_diff(&details.id, parent, &file)
                .map(|d| std::rc::Rc::new(DiffLayout::new(d, compact)))
                .map_err(|e| e.to_string()),
        );
    }

    pub fn log_output(&mut self, text: String, error: bool) {
        if !text.trim().is_empty() {
            self.output.push(OutputLine { text, error });
        }
    }

    fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.message = message.lines().next().unwrap_or_default().to_owned();
        self.log_output(message.clone(), true);
        self.ask(Dialog::Error(message), cx);
    }

    fn done(&mut self, summary: impl Into<String>) {
        self.message = summary.into();
        self.log_output(self.message.clone(), false);
    }

    fn ask(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        cx.emit(ModelEvent::Dialog);
    }

    /// Save the settings when they changed.
    pub fn persist(&mut self) {
        if self.saved == self.settings {
            return;
        }
        if let Some(store) = &self.store
            && let Err(e) = store.save(&self.settings)
        {
            self.log_output(format!("Could not save settings: {e}"), true);
        }
        self.saved = self.settings.clone();
    }

    /// Run a slow operation on the background executor.
    fn spawn(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        f: impl FnOnce() -> snout_core::Result<Outcome> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        self.message = format!("{label}…");
        self.job = Some(label.to_owned());
        self.survey_hold.store(true, Ordering::Relaxed);
        let label = label.to_owned();
        let work = cx.background_executor().spawn(async move { f().map_err(|e| e.to_string()) });
        self._job_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |m, cx| m.finish_job(&label, result, cx));
        }));
        cx.notify();
    }

    fn finish_job(&mut self, label: &str, result: Result<Outcome, String>, cx: &mut Context<Self>) {
        self.job = None;
        self.survey_hold.store(false, Ordering::Relaxed);
        match result {
            Ok(Outcome::Done(text)) => {
                self.log_output(text, false);
                self.message = format!("{label} finished");
            }
            Ok(Outcome::Cloned(repo, text)) => {
                self.log_output(text, false);
                let root = repo.root().to_owned();
                self.open_repo(&root, cx);
                self.message = format!("Cloned into {}", root.display());
            }
            Ok(Outcome::Found { root, repos }) => self.add_found(&root, repos, cx),
            Err(e) => match PullProblem::from_error(&e).filter(|_| label.starts_with("Pull")) {
                Some(problem) => {
                    self.message = format!("{label} failed");
                    self.log_output(e.clone(), true);
                    self.ask(Dialog::PullProblem { problem, details: e }, cx);
                }
                None => self.fail(format!("{label} failed:\n{e}"), cx),
            },
        }
        self.refresh();
        cx.notify();
    }

    fn add_found(&mut self, root: &Path, repos: Vec<PathBuf>, cx: &mut Context<Self>) {
        if repos.is_empty() {
            let text = format!("No Git repositories within {SCAN_DEPTH} levels of\n{}", root.display());
            self.ask(Dialog::Info { title: "No repositories found".into(), text, mood: Mood::Empty }, cx);
            return;
        }
        let before = self.settings.repos.len();
        for r in &repos {
            self.remember(r, r);
        }
        let added = self.settings.repos.len() - before;
        if self.repo.is_none() {
            self.open_repo(&repos[0], cx);
        }
        self.done(format!("Found {} repositories in {} ({added} new)", repos.len(), root.display()));
    }

    fn start_draft(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let Some(program) =
            self.claude.clone().or_else(|| self.settings.claude_path.clone()).or_else(|| find_claude(&*self.host))
        else {
            return self.fail(
                "Claude Code was not found. Install it from https://docs.claude.com/en/docs/claude-code.".into(),
                cx,
            );
        };
        let patch = match repo.commit_patch(self.amend, &self.commit_scope().paths()) {
            Ok(patch) => patch,
            Err(e) => return self.fail(e.to_string(), cx),
        };
        // When amending, the last subject is the one being rewritten.
        let recent: Vec<&str> =
            self.log.iter().skip(usize::from(self.amend)).take(12).map(|c| c.summary.as_str()).collect();
        let prompt = draft_prompt(&recent, &self.commit_msg, self.amend);
        match Draft::start(&**repo.host(), &program, repo.root(), &prompt, &patch, || {}) {
            Ok(draft) => {
                self.draft = Some(draft);
                self.message = "Claude is drafting the commit message…".into();
                self._draft_poll = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_millis(100)).await;
                        let Ok(done) = this.update(cx, |m, cx| m.poll_draft(cx)) else { return };
                        if done {
                            return;
                        }
                    }
                }));
            }
            Err(e) => self.fail(e, cx),
        }
    }

    /// Take in the draft's result if there is one; whether the draft is over.
    fn poll_draft(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(draft) = &self.draft else { return true };
        let Some(result) = draft.poll() else { return false };
        self.draft = None;
        match result {
            Ok(message) => {
                self.commit_msg = message;
                self.done("Claude drafted the commit message");
                cx.emit(ModelEvent::Message);
            }
            Err(e) => self.fail(format!("Claude could not draft the message:\n{e}"), cx),
        }
        cx.notify();
        true
    }

    /// Keep the ahead and behind counts of the listed repositories current: fetch when
    /// due, count again when asked (`recount`) or when some are not counted yet.
    pub fn survey(&mut self, recount: bool, cx: &mut Context<Self>) {
        if self.surveying {
            return;
        }
        let due = self.fetch_every.is_some_and(|every| self.fetched_at.is_none_or(|at| at.elapsed() >= every));
        let repos = self.settings.repos.clone();
        if !(due || recount || repos.iter().any(|p| !self.tracking.contains_key(p))) {
            return;
        }
        if due {
            self.fetched_at = Some(Instant::now());
        }
        self.surveying = true;
        let hold = self.survey_hold.clone();
        let host = self.host.clone();
        self._survey = Some(cx.spawn(async move |this, cx| {
            for path in repos {
                let fetch = due && !hold.load(Ordering::Relaxed);
                let (p, host) = (path.clone(), host.clone());
                let tracking =
                    cx.background_executor().spawn(async move { snout_core::survey::count(&host, &p, fetch) }).await;
                let ok = this.update(cx, |m, cx| {
                    let open = m.repo.as_ref().is_some_and(|r| r.root() == path);
                    m.tracking.insert(path.clone(), tracking);
                    if fetch && open && m.job.is_none() {
                        m.refresh(); // its remote branches may have moved too
                    }
                    cx.notify();
                });
                if ok.is_err() {
                    return;
                }
            }
            let _ = this.update(cx, |m, _| m.surveying = false);
        }));
    }

    /// Start the timer that runs [`Self::survey`] every so often.
    pub fn start_background_fetch(&mut self, every: Duration, cx: &mut Context<Self>) {
        self.fetch_every = Some(every);
        cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |m, cx| m.survey(false, cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(Duration::from_secs(30)).await;
            }
        })
        .detach();
    }

    // ----- Actions --------------------------------------------------------------------

    pub fn handle(&mut self, action: Action, cx: &mut Context<Self>) {
        self.run(action, cx);
        self.persist();
        cx.notify();
    }

    fn run(&mut self, action: Action, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        let host = self.host.clone();
        let local = |m: &mut Self, label: &str, r: snout_core::Result<()>, cx: &mut Context<Self>| {
            match r {
                Ok(()) => m.done(label),
                Err(e) => m.fail(e.to_string(), cx),
            }
            m.refresh();
        };
        match action {
            Action::Refresh => {
                self.refresh();
                self.message = "Refreshed".into();
                self.survey(true, cx);
            }
            Action::OpenRepo(path) => {
                self.open_repo(&path, cx);
            }
            Action::CloseRepo => self.close_repo(cx),
            Action::ClearRecent => self.settings.recent = Some(Vec::new()),
            Action::ShowNewProject => cx.emit(ModelEvent::NewProject),
            Action::PickFolder => self.pick_folder("Open project", Action::OpenPath, cx),
            Action::OpenPath(path) => self.open_path(&path, cx),
            Action::InitRepo(path) => match Repo::init(&self.host, &path) {
                Ok(repo) => {
                    let root = repo.root().to_owned();
                    self.open_repo(&root, cx);
                    self.done(format!("Initialized repository in {}", root.display()));
                }
                Err(e) => self.fail(e.to_string(), cx),
            },
            Action::PickScanFolder => self.pick_folder("Find repositories in folder", Action::Scan, cx),
            Action::Scan(root) => self.spawn("Searching for repositories", cx, move || {
                let repos = snout_core::find_repos(&*host, &root, SCAN_DEPTH);
                Ok(Outcome::Found { root, repos })
            }),
            Action::RemoveRepo(path) => {
                self.settings.repos.retain(|p| p != &path);
                move_to_group(&mut self.settings.groups, &path, None);
                // The open project goes as Close Project would close it, tabs and all.
                if self.repo.as_ref().is_some_and(|r| r.root() == path) {
                    self.close_repo(cx);
                }
            }
            Action::ShowNewGroup(repo) => self.ask(Dialog::GroupName { rename: None, repo }, cx),
            Action::ShowRenameGroup(name) => self.ask(Dialog::GroupName { rename: Some(name), repo: None }, cx),
            Action::SaveGroup { rename, name, repo } => {
                save_group(&mut self.settings.groups, rename.as_deref(), &name, repo.as_deref())
            }
            Action::MoveToGroup(path, group) => move_to_group(&mut self.settings.groups, &path, group.as_deref()),
            Action::DeleteGroup(name) => self.settings.groups.retain(|g| g.name != name),
            Action::ShowClone => {
                let parent = self
                    .settings
                    .current
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.ask(Dialog::Clone { parent }, cx);
            }
            Action::Clone { url, dest } => self.spawn("Clone", cx, move || {
                Repo::clone_from(&host, &url, &dest).map(|(repo, text)| Outcome::Cloned(repo, text))
            }),
            Action::SetTheme(choice) => {
                self.settings.theme = choice;
                cx.emit(ModelEvent::Theme);
            }
            Action::ToggleConsole => {
                self.settings.console_open = !self.settings.console_open;
                cx.emit(ModelEvent::ToggleConsole);
            }
            Action::Copy(text) => cx.write_to_clipboard(gpui::ClipboardItem::new_string(text)),
            Action::Reveal(path) => cx.reveal_path(&path),
            Action::OpenFile(path, at) => cx.emit(ModelEvent::OpenFile(path, at, false)),
            Action::PreviewFile(path) => cx.emit(ModelEvent::OpenFile(path, None, true)),
            Action::FilterDir(dir) => {
                self.dir_filter = dir;
                self.anchor = None;
            }
            _ if repo.is_none() => {}
            Action::Stage(paths) => {
                let n = paths.len();
                local(self, &format!("Staged {n} file(s)"), repo.unwrap().stage(&paths), cx);
            }
            Action::Unstage(paths) => local(self, "Unstaged", repo.unwrap().unstage(&paths), cx),
            Action::AskDiscard(entries) => self.ask(Dialog::ConfirmDiscard(entries), cx),
            Action::Discard(entries) => {
                let n = entries.len();
                local(self, &format!("Discarded {n} file(s)"), repo.unwrap().discard(&entries), cx);
            }
            Action::AskDeleteFiles(entries) => {
                let entries: Vec<FileEntry> = entries.into_iter().filter(on_disk).collect();
                if !entries.is_empty() {
                    self.ask(Dialog::ConfirmDeleteFiles(entries), cx);
                }
            }
            Action::DeleteFiles(entries) => {
                let n = entries.len();
                let label = if snout_core::can_trash(&*self.host) {
                    format!("Moved {n} file(s) to the Trash")
                } else {
                    format!("Deleted {n} file(s)")
                };
                local(self, &label, repo.unwrap().delete_files(&entries, true), cx);
            }
            Action::Commit => {
                if !self.can_commit() {
                    return;
                }
                match repo.unwrap().commit(&self.commit_msg, self.amend, &self.commit_scope().paths()) {
                    Ok(summary) => {
                        self.done(format!("Committed {summary}"));
                        self.commit_msg.clear();
                        self.amend = false;
                        self.refresh();
                        cx.emit(ModelEvent::Message);
                    }
                    Err(e) => self.fail(e.to_string(), cx),
                }
            }
            Action::DraftMessage => {
                if self.can_draft() {
                    self.start_draft(cx)
                }
            }
            Action::CancelDraft => {
                self.draft = None;
                self._draft_poll = None;
                self.message = "Stopped drafting".into();
            }
            Action::ShowCheckout => self.ask(Dialog::Checkout, cx),
            Action::RequestCheckout(target) => {
                if self.has_tracked_changes() {
                    self.ask(Dialog::ConfirmCheckout(target), cx);
                } else {
                    self.run(Action::Checkout(target, LocalChanges::Keep), cx);
                }
            }
            Action::Checkout(target, mode) => {
                match repo.unwrap().checkout(&target, mode) {
                    Ok(outcome) => {
                        self.log_output(outcome.output, false);
                        self.done(format!("Checked out {}", target.label()));
                        if let Some(warning) = outcome.warning {
                            self.log_output(warning.clone(), true);
                            let title = "Local changes conflicted".into();
                            self.ask(Dialog::Info { title, text: warning, mood: Mood::Warning }, cx);
                        }
                    }
                    Err(e) => self.fail(e.to_string(), cx),
                }
                self.refresh();
            }
            Action::ShowNewBranch(start) => self.ask(Dialog::NewBranch(start), cx),
            Action::CreateBranch { name, start, checkout } => {
                let r = repo.unwrap().create_branch(&name, start.as_deref(), checkout);
                local(self, &format!("Created branch {name}"), r, cx);
            }
            Action::AskDeleteBranch(b) => {
                let dialog = match b.kind {
                    BranchKind::Local => {
                        // Offer the upstream too, when it still exists.
                        let upstream = b
                            .upstream
                            .as_ref()
                            .filter(|_| !b.upstream_gone)
                            .and_then(|u| self.branches.iter().find(|r| r.kind == BranchKind::Remote && &r.name == u))
                            .and_then(RemoteRef::of);
                        Dialog::DeleteBranch { name: b.name, upstream }
                    }
                    BranchKind::Remote => match RemoteRef::of(&b) {
                        Some(r) => Dialog::DeleteRemoteBranch(r),
                        None => return,
                    },
                };
                self.ask(dialog, cx);
            }
            Action::DeleteBranch { name, force, upstream } => {
                let deleted = repo.unwrap().delete_branch(&name, force);
                let ok = deleted.is_ok();
                local(self, &format!("Deleted branch {name}"), deleted, cx);
                if let Some(upstream) = upstream.filter(|_| ok) {
                    self.run(Action::DeleteRemoteBranch(upstream), cx);
                }
            }
            Action::DeleteRemoteBranch(r) => {
                let repo = repo.unwrap();
                self.spawn(&format!("Delete {}", r.label()), cx, move || {
                    repo.delete_remote_branch(&r.remote, &r.branch).map(Outcome::Done)
                });
            }
            Action::Fetch => {
                let repo = repo.unwrap();
                self.spawn("Fetch", cx, move || repo.fetch().map(Outcome::Done));
            }
            Action::Pull(mode) => {
                let repo = repo.unwrap();
                let label = match mode {
                    PullMode::Default => "Pull",
                    PullMode::Merge => "Pull (merge)",
                    PullMode::Rebase => "Pull (rebase)",
                };
                self.spawn(label, cx, move || repo.pull_with(mode).map(Outcome::Done));
            }
            Action::ResetToUpstream => {
                let upstream = self.status.branch.upstream.clone().unwrap_or_default();
                let head = self.status.branch.head.clone().unwrap_or_default();
                match repo.unwrap().reset_to_upstream() {
                    Ok(backup) => {
                        self.done(format!("Reset {head} to {upstream}"));
                        let text = format!(
                            "Your previous commits are kept on the branch {backup}. Delete it once you no longer need them."
                        );
                        let title = format!("{head} now matches {upstream}");
                        self.ask(Dialog::Info { title, text, mood: Mood::Done }, cx);
                    }
                    Err(e) => self.fail(e.to_string(), cx),
                }
                self.refresh();
            }
            Action::Push => {
                let repo = repo.unwrap();
                self.spawn("Push", cx, move || repo.push().map(Outcome::Done));
            }
            Action::OpenGithub => {
                if let Some(url) = &self.github_url {
                    cx.open_url(url);
                }
            }
            Action::ShowCommit(rev) => {
                let repo = repo.unwrap();
                let details = repo.commit_details(&rev).map_err(|e| e.to_string());
                let selected = details.as_ref().ok().and_then(|d| d.files.first()).map(|f| f.path.clone());
                if let Ok(d) = &details {
                    self.message = format!("Commit {} {}", d.short_id, d.summary());
                }
                self.commit_view = Some(CommitView { details, selected, diff: None });
                self.update_commit_diff();
                cx.emit(ModelEvent::ShowCommit);
            }
            Action::CloseCommit => {
                self.commit_view = None;
                self.message = "Working tree".into();
            }
        }
    }

    /// Ask for a folder, then run `then` with it.
    fn pick_folder(&mut self, prompt: &str, then: fn(PathBuf) -> Action, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(prompt.to_owned().into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(dir) = paths.into_iter().next()
            {
                let _ = this.update(cx, |m, cx| m.handle(then(dir), cx));
            }
        })
        .detach();
    }

    /// The group `path` is in, if any.
    pub fn group_of(&self, path: &Path) -> Option<&RepoGroup> {
        self.settings.groups.iter().find(|g| g.repos.iter().any(|p| p == path))
    }

    pub fn dir_name(path: &Path) -> String {
        dir_name(path)
    }
}
