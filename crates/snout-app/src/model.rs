//! The Git Panel's state and everything it does, with no drawing: the GPUI counterpart of
//! the egui app's `SnoutApp` minus its panes. Views read it and send it [`Action`]s.

use std::collections::{BTreeSet, HashMap, VecDeque};
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
    Info {
        title: String,
        text: String,
        mood: Mood,
    },
    ConfirmDiscard(Vec<FileEntry>),
    ConfirmCheckout(CheckoutTarget),
    Checkout,
    NewBranch(Option<StartPoint>),
    DeleteBranch {
        name: String,
        upstream: Option<RemoteRef>,
    },
    DeleteRemoteBranch(RemoteRef),
    ConfirmDeleteFiles(Vec<FileEntry>),
    PullProblem {
        problem: PullProblem,
        details: String,
    },
    InitRepo(PathBuf),
    GroupName {
        rename: Option<String>,
        repo: Option<PathBuf>,
    },
    Clone {
        parent: String,
    },
    /// Quit SnoutCode? What would be lost or stopped.
    Quit(QuitInfo),
}

/// What quitting now would lose or stop, for the Quit dialog.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuitInfo {
    /// The names of the tabs with unsaved changes.
    pub unsaved: Vec<String>,
    /// What would stop, in plain words: "Debugging Main", "Gradle build :app:build".
    pub stops: Vec<String>,
}

impl QuitInfo {
    /// Nothing would be lost or stopped: the dialog may be skipped.
    pub fn is_empty(&self) -> bool {
        self.unsaved.is_empty() && self.stops.is_empty()
    }
}

/// How the user answered the Quit dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuitChoice {
    /// Save the unsaved tabs first (true), drop them (false), or there were none.
    pub save: Option<bool>,
    /// "Don't ask again" was ticked.
    pub dont_ask: bool,
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
    /// Open the external terminal at a folder (a file: at its parent).
    OpenInTerminal(PathBuf),
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
    /// Quit, as the Quit dialog was answered.
    Quit(QuitChoice),
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
    /// Ask for a folder of the remote machine the window works on (the Mac's folder dialog
    /// cannot see it), then handle the action made of it.
    PickRemoteFolder(AfterPick),
    /// Quit, as the Quit dialog was answered.
    Quit(QuitChoice),
}

/// What to do with a folder picked: the action made of it.
#[derive(Clone, Copy, Debug)]
pub struct AfterPick(pub fn(PathBuf) -> Action);

impl PartialEq for AfterPick {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.0, other.0)
    }
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

/// What the Git Panel shows of a repository, read off the UI thread.
struct Snapshot {
    status: Result<Status, String>,
    branches: Vec<Branch>,
    tags: Vec<Tag>,
    log: Vec<Commit>,
    dirs: Vec<String>,
    errors: Vec<String>,
}

impl Snapshot {
    /// Read `repo` (blocking: git on its host).
    fn read(repo: &Repo) -> Self {
        let status = repo.status().map_err(|e| e.to_string());
        if status.is_err() {
            let (branches, tags, log, dirs, errors) = Default::default();
            return Self { status, branches, tags, log, dirs, errors };
        }
        let mut errors = Vec::new();
        let branches = repo.branches().map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        let tags = repo.tags().map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        let log = repo.log(LOG_LIMIT).map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        let dirs = repo.directories().map_err(|e| errors.push(e.to_string())).unwrap_or_default();
        Self { status, branches, tags, log, dirs, errors }
    }
}

/// A project opened off the UI thread: its folder, its repository and what that shows.
struct Opened {
    /// The folder as it was asked for.
    asked: PathBuf,
    /// The folder as opened, in the form Git gives the root (symbolic links resolved).
    project: PathBuf,
    repo: Repo,
    github_url: Option<String>,
    snapshot: Snapshot,
}

impl Opened {
    /// Open the folder `path` on `host` as a project, with the repository at or above it.
    fn load(host: &snout_core::HostRef, path: &Path) -> snout_core::Result<Self> {
        let repo = Repo::open(host, path)?;
        let project = host.canonicalize(path).unwrap_or_else(|_| path.to_owned());
        let github_url = repo.github_url();
        let snapshot = Snapshot::read(&repo);
        Ok(Self { asked: path.to_owned(), project, repo, github_url, snapshot })
    }
}

/// What a Git job hands back to the model, on the UI thread.
type Then = Box<dyn FnOnce(&mut Model, &mut Context<Model>) + Send>;

/// A change to the open repository, made off the UI thread.
type Change = Box<dyn FnOnce(&Repo) -> snout_core::Result<()> + Send>;

/// Work on the open repository, off the UI thread; jobs run one at a time, in order.
struct GitJob {
    repo: Repo,
    /// The job reads the repository for the panel again.
    refresh: bool,
    work: Box<dyn FnOnce(&Repo) -> Then + Send>,
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
    /// What the open project's `.snout/settings.json` sets, as read last (see
    /// [`crate::project_settings`]); `settings` has it put over the user's own values.
    pub project_settings: crate::project_settings::ProjectSettings,
    /// The project those values are of, and the file's stamp when they were read.
    pub project_settings_root: Option<PathBuf>,
    pub project_settings_stamp: crate::project_settings::Stamp,
    /// Why the project's file could not be read (its last good values stay in use).
    pub project_settings_error: Option<String>,
    /// The user's own values under those the project sets, given back when saving.
    project_under: crate::project_settings::ProjectSettings,
    /// The user's settings file's stamp when it was last read or written here, to tell an
    /// edit made elsewhere (the editor, another program).
    user_file_stamp: Option<(Option<std::time::SystemTime>, u64)>,
    /// Why the user's settings file, edited by hand, cannot be read: the settings in use
    /// stay, and the file is not written over until it is fixed (or the app quits).
    pub user_settings_error: Option<String>,
    /// The `claude` to run; looked up when `None`.
    pub claude: Option<PathBuf>,
    /// A file given on the command line, to open once the window is there.
    pub pending_file: Option<(PathBuf, Option<(u32, u32)>)>,
    /// Opening another project asks the workspace first ([`ModelEvent::Switch`]); off
    /// without a window, as in the model's own tests.
    pub confirm_switch: bool,
    /// The Git jobs waiting, whether one runs, and whether a refresh waits among them.
    git_jobs: VecDeque<GitJob>,
    git_busy: bool,
    refresh_queued: bool,
    /// A project opened and waiting for the workspace to switch to it.
    pending_open: Option<Opened>,
    /// A draft of the commit message is being started.
    starting_draft: bool,
    _open: Option<Task<()>>,
    _diff: Option<Task<()>>,
    _commit_diff: Option<Task<()>>,
    _show_commit: Option<Task<()>>,
}

impl EventEmitter<ModelEvent> for Model {}

impl Model {
    pub fn new(settings: Settings, store: Option<Store>, cx: &mut Context<Self>) -> Self {
        let mut model = Self {
            host: crate::host::host(cx),
            project_settings: Default::default(),
            project_settings_root: None,
            project_settings_stamp: None,
            project_settings_error: None,
            project_under: Default::default(),
            user_file_stamp: store.as_ref().and_then(Store::stamp),
            user_settings_error: None,
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
            git_jobs: VecDeque::new(),
            git_busy: false,
            refresh_queued: false,
            pending_open: None,
            starting_draft: false,
            _open: None,
            _diff: None,
            _commit_diff: None,
            _show_commit: None,
        };
        if let Some(path) = model.settings.current.clone() {
            // The project is known at once (its tabs come back with the window); its
            // repository opens off the UI thread.
            model.project = Some(path.clone());
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
        self.repo.is_some() && self.draft.is_none() && !self.starting_draft && self.commit_scope() != Scope::Nothing
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
        self.update_diff(cx);
        if self.selected.len() == 1 {
            cx.emit(ModelEvent::ShowDiff);
        }
        cx.notify();
    }

    pub fn select_all_files(&mut self, cx: &mut Context<Self>) {
        self.selected = self.visible_files().iter().map(|f| f.path.clone()).collect();
        self.update_diff(cx);
        cx.notify();
    }

    pub fn set_amend(&mut self, amend: bool, cx: &mut Context<Self>) {
        let had = self.amend;
        self.amend = amend && !self.log.is_empty();
        if self.amend && !had && self.commit_msg.trim().is_empty() {
            // The message being amended, read off the UI thread.
            self.queue(cx, |repo| {
                let message = repo.last_commit_message().unwrap_or_default();
                Box::new(move |m, cx| {
                    if m.amend && m.commit_msg.trim().is_empty() {
                        m.commit_msg = message;
                        cx.emit(ModelEvent::Message);
                    }
                })
            });
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
        self.update_diff(cx);
        self.update_commit_diff(cx);
        self.persist();
        cx.notify();
    }

    pub fn set_diff_base(&mut self, base: snout_core::DiffBase, cx: &mut Context<Self>) {
        self.settings.diff_base = base;
        self.update_diff(cx);
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
        self.update_commit_diff(cx);
        cx.notify();
    }

    // ----- State ----------------------------------------------------------------------

    /// Open the folder `path` as the project, with the repository found at or above it, off
    /// the UI thread. When another project is open, the workspace is asked first
    /// ([`ModelEvent::Switch`]), and [`Model::switch_project`] finishes the opening.
    fn open_repo(&mut self, path: &Path, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let load = cx.background_executor().spawn({
            let path = path.to_owned();
            async move { Opened::load(&host, &path).map_err(|e| e.to_string()) }
        });
        self._open = Some(cx.spawn(async move |this, cx| {
            let opened = load.await;
            let _ = this.update(cx, |m, cx| {
                match opened {
                    Ok(o) if m.confirm_switch && m.repo.is_some() && m.project.as_ref() != Some(&o.project) => {
                        let asked = o.asked.clone();
                        m.pending_open = Some(o);
                        cx.emit(ModelEvent::Switch(asked));
                    }
                    Ok(o) => {
                        m.apply_opened(o, cx);
                        m.open_pending_file(cx);
                    }
                    Err(e) => {
                        // A project remembered from last time that is gone: none is open.
                        if m.repo.is_none() {
                            m.project = None;
                        }
                        m.pending_file = None;
                        m.fail(e, cx);
                    }
                }
                m.persist();
                cx.notify();
            });
        }));
    }

    /// Make `o` the open project.
    fn apply_opened(&mut self, o: Opened, cx: &mut Context<Self>) {
        let root = o.repo.root().to_owned();
        self.remember(&root, &o.project);
        self.settings.current = Some(o.project.clone());
        self.project = Some(o.project);
        self.github_url = o.github_url;
        self.repo = Some(o.repo);
        self.git_jobs.clear();
        self.refresh_queued = false;
        self.selected.clear();
        self.anchor = None;
        self.dir_filter = None;
        self.commit_view = None;
        self.journal_filter.clear();
        self.commit_msg.clear();
        self.amend = false;
        self.draft = None;
        self.apply_snapshot(o.snapshot, cx);
        self.message = format!("Opened {}", o.asked.display());
        cx.emit(ModelEvent::Message);
        // Whatever follows the project (its engine, runs, models) sees the change now.
        cx.notify();
    }

    /// The file asked for with the project opens in a tab of it.
    fn open_pending_file(&mut self, cx: &mut Context<Self>) {
        if let Some((path, at)) = self.pending_file.clone() {
            cx.emit(ModelEvent::OpenFile(path, at, false));
        }
    }

    /// Open `path` in place of the open project, once the workspace has settled its tabs.
    pub fn switch_project(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        match self.pending_open.take() {
            Some(o) if o.asked == path => {
                self.apply_opened(o, cx);
                self.persist();
                true
            }
            _ => {
                let confirm = std::mem::replace(&mut self.confirm_switch, false);
                self.open_repo(path, cx);
                self.confirm_switch = confirm;
                false
            }
        }
    }

    /// Open `path` as the command line names it: `File.java:12` or `File.java:12:5` puts the
    /// caret on that line (and column), from 1.
    pub fn open_arg(&mut self, arg: &Path, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let arg = arg.to_owned();
        let look = cx.background_executor().spawn(async move {
            if !host.exists(&arg)
                && let Some((path, at)) = split_line_suffix(&arg.to_string_lossy())
                && host.is_file(&path)
            {
                return (path, Some(at));
            }
            (arg, None)
        });
        cx.spawn(async move |this, cx| {
            let (path, at) = look.await;
            let _ = this.update(cx, |m, cx| m.open_path_at(&path, at, cx));
        })
        .detach();
    }

    fn open_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.open_path_at(path, None, cx);
    }

    /// Open the folder `path` as the project, or the file `path` (at `at`) in its repository,
    /// once the host says which it is.
    fn open_path_at(&mut self, path: &Path, at: Option<(u32, u32)>, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let path = path.to_owned();
        let look = cx.background_executor().spawn({
            let path = path.clone();
            async move {
                let is_file = host.is_file(&path);
                let dir = if is_file { path.parent().unwrap_or(&path).to_owned() } else { path.clone() };
                let root = Repo::open(&host, &dir).ok().map(|r| r.root().to_owned());
                let is_dir = root.is_some() || host.is_dir(&dir);
                (is_file, dir, root, is_dir)
            }
        });
        cx.spawn(async move |this, cx| {
            let (is_file, dir, root, is_dir) = look.await;
            let _ = this.update(cx, |m, cx| {
                match root {
                    Some(root) => {
                        // A folder is the project as it is; a file opens in its repository
                        // (once the window is there, or once the switch to its project is done).
                        if is_file {
                            m.pending_file = Some((path, at));
                        }
                        m.open_repo(if is_file { &root } else { &dir }, cx);
                    }
                    None if is_dir => m.ask(Dialog::InitRepo(dir), cx),
                    None => m.fail(format!("{} does not exist", dir.display()), cx),
                }
                cx.notify();
            });
        })
        .detach();
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
        self.git_jobs.clear();
        self.refresh_queued = false;
        self.refresh(cx);
        self.message = format!("Closed {}", repo.root().display());
        cx.emit(ModelEvent::Closed);
        cx.notify();
    }

    /// Run `work` on the open repository off the UI thread, after the jobs before it; what
    /// it hands back runs on the model, if that repository is still open.
    fn queue(&mut self, cx: &mut Context<Self>, work: impl FnOnce(&Repo) -> Then + Send + 'static) {
        let Some(repo) = self.repo.clone() else { return };
        self.git_jobs.push_back(GitJob { repo, refresh: false, work: Box::new(work) });
        self.pump(cx);
    }

    /// Start the next Git job, unless one runs.
    fn pump(&mut self, cx: &mut Context<Self>) {
        if self.git_busy {
            return;
        }
        let Some(job) = self.git_jobs.pop_front() else { return };
        if job.refresh {
            self.refresh_queued = false;
        }
        self.git_busy = true;
        let repo = job.repo.clone();
        let work = cx.background_executor().spawn(async move { (job.work)(&job.repo) });
        cx.spawn(async move |this, cx| {
            let then = work.await;
            let _ = this.update(cx, |m, cx| {
                m.git_busy = false;
                if m.repo.as_ref() == Some(&repo) {
                    then(m, cx);
                    m.persist();
                }
                m.pump(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Read the repository again, off the UI thread (once, however often it is asked while
    /// it waits).
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            self.diff_key = None;
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
        if self.refresh_queued {
            return;
        }
        self.refresh_queued = true;
        self.git_jobs.push_back(GitJob {
            repo,
            refresh: true,
            work: Box::new(|repo| {
                let snapshot = Snapshot::read(repo);
                Box::new(move |m, cx| m.apply_snapshot(snapshot, cx))
            }),
        });
        self.pump(cx);
    }

    /// Show what the repository holds now.
    fn apply_snapshot(&mut self, snap: Snapshot, cx: &mut Context<Self>) {
        self.diff_key = None;
        let Some(repo) = self.repo.clone() else { return };
        match snap.status {
            Ok(status) => {
                let present: BTreeSet<&String> = status.files.iter().map(|f| &f.path).collect();
                self.selected.retain(|p| present.contains(p));
                self.status = status;
            }
            Err(e) => {
                self.log_output(e, true);
                return;
            }
        }
        let b = &self.status.branch;
        let tracking = b.upstream.is_some().then_some(Tracking { ahead: b.ahead, behind: b.behind });
        self.tracking.insert(repo.root().to_owned(), tracking);
        self.branches = snap.branches;
        self.tags = snap.tags;
        self.log = snap.log;
        self.dirs = snap.dirs;
        for e in snap.errors {
            self.log_output(e, true);
        }
        self.graph = snout_core::graph::layout(&self.log);
        if self.dir_filter.as_ref().is_some_and(|d| self.dirs.binary_search(d).is_err()) {
            self.dir_filter = None;
        }
        self.update_diff(cx);
        self.persist();
    }

    /// Show the selected file's diff: read off the UI thread, the one shown stays until it is in.
    fn update_diff(&mut self, cx: &mut Context<Self>) {
        let entry = (self.selected.len() == 1)
            .then(|| self.status.files.iter().find(|f| self.selected.contains(&f.path)))
            .flatten();
        let Some(entry) = entry.cloned() else {
            self.diff_key = None;
            self.diff = None;
            self._diff = None;
            return;
        };
        let key = (entry.path.clone(), self.settings.diff_base, self.settings.compact);
        if self.diff_key.as_ref() == Some(&key) {
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        self.diff_key = Some(key.clone());
        let (base, compact) = (self.settings.diff_base, self.settings.compact);
        let read = cx.background_executor().spawn(async move {
            repo.file_diff(&entry, base).map(|d| DiffLayout::new(d, compact)).map_err(|e| e.to_string())
        });
        self._diff = Some(cx.spawn(async move |this, cx| {
            let diff = read.await;
            let _ = this.update(cx, |m, cx| {
                if m.diff_key.as_ref() == Some(&key) {
                    m.diff = Some(diff.map(std::rc::Rc::new));
                    cx.notify();
                }
            });
        }));
    }

    /// Show the diff of the commit view's file, read off the UI thread.
    fn update_commit_diff(&mut self, cx: &mut Context<Self>) {
        let compact = self.settings.compact;
        let Some(view) = &mut self.commit_view else { return };
        let Some(file) = view.selected_file().cloned() else {
            view.diff = None;
            return;
        };
        let (Some(repo), Ok(details)) = (self.repo.clone(), &view.details) else { return };
        let (id, parent) = (details.id.clone(), details.parents.first().cloned());
        let read = cx.background_executor().spawn({
            let id = id.clone();
            async move {
                repo.commit_file_diff(&id, parent.as_deref(), &file)
                    .map(|d| (file.path, DiffLayout::new(d, compact)))
                    .map_err(|e| e.to_string())
            }
        });
        let path = view.selected.clone();
        self._commit_diff = Some(cx.spawn(async move |this, cx| {
            let diff = read.await;
            let _ = this.update(cx, |m, cx| {
                let Some(view) = m.commit_view.as_mut().filter(|v| v.id() == Some(&id) && v.selected == path) else {
                    return;
                };
                view.diff = Some(diff.map(|(_, d)| std::rc::Rc::new(d)));
                cx.notify();
            });
        }));
    }

    pub fn log_output(&mut self, text: String, error: bool) {
        if !text.trim().is_empty() {
            self.output.push(OutputLine { text, error });
        }
    }

    /// Open the external terminal the settings choose at `path` (a file: at its folder), on
    /// the window's host: on a remote machine, an ssh session there.
    pub fn open_in_terminal(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        use crate::external_terminal as et;
        let term = self.settings.external_terminal;
        let custom = self.settings.external_terminal_command.clone();
        let remote = crate::remote::connection(cx).map(|(name, _)| crate::remote::target(&name));
        let host = crate::host::host(cx);
        let scripts = std::env::temp_dir().join("snoutcode-terminal");
        let opened = cx.background_executor().spawn(async move {
            let dir = match host.metadata(&path) {
                Ok(m) if m.is_dir() => path,
                _ => path.parent().map(Path::to_path_buf).unwrap_or(path),
            };
            let at = match remote {
                Some(t) => et::Where::Remote { ssh: t.terminal_command(&et::remote_shell(&dir)), dir },
                None => et::Where::Local(dir),
            };
            et::command(term, &custom, &at, &scripts).map_err(|e| e.to_string()).and_then(|argv| et::run(&argv))
        });
        cx.spawn(async move |this, cx| {
            if let Err(e) = opened.await {
                let _ = this.update(cx, |m, cx| {
                    m.fail(e, cx);
                    cx.notify();
                });
            }
        })
        .detach();
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

    /// Save the user's settings when they changed (the values the project sets for itself
    /// are not the user's: the user's own go to the file).
    pub fn persist(&mut self) {
        let user = self.user_settings();
        if self.saved == user || self.user_settings_error.is_some() {
            return;
        }
        if let Some(store) = &self.store {
            match store.save(&user) {
                Ok(()) => self.user_file_stamp = store.stamp(),
                Err(e) => self.log_output(format!("Could not save settings: {e}"), true),
            }
        }
        self.saved = user;
    }

    /// Save the user's settings on the way out: a settings file left broken by a hand edit
    /// is copied aside (`settings.json.unreadable`) first, then written over.
    pub fn persist_at_quit(&mut self) {
        if self.user_settings_error.take().is_some()
            && let Some(store) = &self.store
        {
            let _ = std::fs::copy(&store.file, store.file.with_extension("json.unreadable"));
            let _ = store.save(&self.user_settings());
            return;
        }
        self.persist();
    }

    /// The user's settings file, if the settings are kept in one.
    pub fn store_file(&self) -> Option<PathBuf> {
        self.store.as_ref().map(|s| s.file.clone())
    }

    /// Write the user's settings file now, if it is not there.
    pub fn ensure_store_file(&mut self) {
        if let Some(store) = &self.store
            && !store.file.exists()
            && store.save(&self.user_settings()).is_ok()
        {
            self.user_file_stamp = store.stamp();
        }
    }

    /// Read the user's settings file again if it changed since it was last read or
    /// written here: its settings go in place (true), or, when it cannot be read, the
    /// settings in use stay and the error is shown.
    pub fn check_user_file(&mut self) -> bool {
        let Some(store) = &self.store else { return false };
        let stamp = store.stamp();
        if stamp == self.user_file_stamp {
            return false;
        }
        self.user_file_stamp = stamp;
        match store.read_strict() {
            None => false,
            Some(Err(e)) => {
                self.message = format!("Your settings.json has an error, so the settings in use stay: {e}");
                self.user_settings_error = Some(e);
                false
            }
            Some(Ok(read)) => {
                if self.user_settings_error.take().is_some() {
                    self.message = "Your settings are read again".into();
                }
                self.saved = read.clone();
                if read == self.user_settings() {
                    return false;
                }
                self.set_user_settings(read);
                true
            }
        }
    }

    /// The user's own values under those the project sets.
    pub fn project_under(&self) -> &crate::project_settings::ProjectSettings {
        &self.project_under
    }

    /// The user's own settings: those in use, but for what the project sets, the user's.
    pub fn user_settings(&self) -> Settings {
        let mut s = self.settings.clone();
        crate::project_settings::take_back(&self.project_under, &mut s);
        s
    }

    /// Put the user's settings `user` in place, the project's values over them.
    pub fn set_user_settings(&mut self, user: Settings) {
        self.settings = user;
        self.project_under = crate::project_settings::put_over(&self.project_settings, &mut self.settings);
    }

    /// Put the project's values `values` in place of those it set before.
    pub fn set_project_settings(&mut self, values: crate::project_settings::ProjectSettings) {
        let user = self.user_settings();
        self.project_settings = values;
        self.set_user_settings(user);
    }

    /// Set the project's values and write them to its file, off the UI thread. The file's
    /// stamp is taken again once written, so the write does not read as a change.
    pub fn save_project_settings(&mut self, values: crate::project_settings::ProjectSettings, cx: &mut Context<Self>) {
        let Some(root) = self.project_root() else { return };
        self.set_project_settings(values.clone());
        self.project_settings_error = None;
        let host = self.host.clone();
        let write = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                let done = crate::project_settings::write(&*host, &root, &values);
                (done, crate::project_settings::stamp(&*host, &root))
            }
        });
        cx.spawn(async move |this, cx| {
            let (done, stamp) = write.await;
            let _ = this.update(cx, |m, cx| {
                match done {
                    Ok(()) if m.project_settings_root.as_ref() == Some(&root) => m.project_settings_stamp = stamp,
                    Ok(()) => {}
                    Err(e) => m.log_output(format!("Could not save the project's settings: {e}"), true),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The JDK the Gradle build at `root` runs on, as chosen: for the project when it sets
    /// one, else for the build in the user's settings.
    pub fn jdk_for(&self, root: &Path) -> Option<PathBuf> {
        let project = self.project_settings_root.as_ref().filter(|p| root.starts_with(p));
        match (project, &self.project_settings.jdk) {
            (Some(_), Some(jdk)) => Some(jdk.clone()),
            _ => self.settings.project_jdks.get(root).cloned(),
        }
    }

    /// The profiles the open project's Spring model is evaluated under.
    pub fn spring_mode(&self) -> crate::spring::ProfileMode {
        if let Some(p) = &self.project_settings.spring_profiles {
            return p.into();
        }
        self.project_root().and_then(|r| self.settings.spring_profiles.get(&r).cloned()).unwrap_or_default()
    }

    /// Keep `mode` for the open project: in its settings file when that sets the profiles,
    /// else in the user's settings.
    pub fn set_spring_mode(&mut self, mode: crate::spring::ProfileMode, cx: &mut Context<Self>) {
        let Some(root) = self.project_root() else { return };
        if self.project_settings.spring_profiles.is_some() {
            let mut values = self.project_settings.clone();
            values.spring_profiles = Some((&mode).into());
            self.save_project_settings(values, cx);
            return;
        }
        if mode == crate::spring::ProfileMode::Any {
            self.settings.spring_profiles.remove(&root);
        } else {
            self.settings.spring_profiles.insert(root, mode);
        }
        self.persist();
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
        self.refresh(cx);
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

    /// Have Claude draft the commit message: the patch is read, `claude` found and started
    /// off the UI thread.
    fn start_draft(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let known = self.claude.clone().or_else(|| self.settings.claude_path.clone());
        let (amend, paths) = (self.amend, self.commit_scope().paths());
        // When amending, the last subject is the one being rewritten.
        let recent: Vec<&str> =
            self.log.iter().skip(usize::from(self.amend)).take(12).map(|c| c.summary.as_str()).collect();
        let prompt = draft_prompt(&recent, &self.commit_msg, self.amend);
        self.starting_draft = true;
        let start = cx.background_executor().spawn(async move {
            let Some(program) = known.or_else(|| find_claude(&**repo.host())) else {
                return Err("Claude Code was not found. Install it from https://docs.claude.com/en/docs/claude-code."
                    .to_owned());
            };
            let patch = repo.commit_patch(amend, &paths).map_err(|e| e.to_string())?;
            Draft::start(&**repo.host(), &program, repo.root(), &prompt, &patch, || {})
        });
        cx.spawn(async move |this, cx| {
            let started = start.await;
            let _ = this.update(cx, |m, cx| {
                m.starting_draft = false;
                match started {
                    Ok(draft) => {
                        m.draft = Some(draft);
                        m.message = "Claude is drafting the commit message…".into();
                        m._draft_poll = Some(cx.spawn(async move |this, cx| {
                            loop {
                                cx.background_executor().timer(Duration::from_millis(100)).await;
                                let Ok(done) = this.update(cx, |m, cx| m.poll_draft(cx)) else { return };
                                if done {
                                    return;
                                }
                            }
                        }));
                    }
                    Err(e) => m.fail(e, cx),
                }
                cx.notify();
            });
        })
        .detach();
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
                        m.refresh(cx); // its remote branches may have moved too
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
        // A change to the repository, made off the UI thread; the panel is read again after it.
        let local = |m: &mut Self, label: String, op: Change, cx: &mut Context<Self>| {
            m.queue(cx, move |repo| {
                let r = op(repo).map_err(|e| e.to_string());
                Box::new(move |m, cx| match r {
                    Ok(()) => m.done(label),
                    Err(e) => m.fail(e, cx),
                })
            });
            m.refresh(cx);
        };
        match action {
            Action::Refresh => {
                self.refresh(cx);
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
            Action::InitRepo(path) => {
                let init = cx.background_executor().spawn(async move {
                    Repo::init(&host, &path).map(|r| r.root().to_owned()).map_err(|e| e.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let root = init.await;
                    let _ = this.update(cx, |m, cx| {
                        match root {
                            Ok(root) => {
                                m.open_repo(&root, cx);
                                m.done(format!("Initialized repository in {}", root.display()));
                            }
                            Err(e) => m.fail(e, cx),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
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
            Action::Quit(choice) => {
                if choice.dont_ask {
                    self.settings.confirm_quit = false;
                }
                cx.emit(ModelEvent::Quit(choice));
            }
            Action::Copy(text) => cx.write_to_clipboard(gpui::ClipboardItem::new_string(text)),
            Action::Reveal(path) => cx.reveal_path(&path),
            Action::OpenInTerminal(path) => self.open_in_terminal(path, cx),
            Action::OpenFile(path, at) => cx.emit(ModelEvent::OpenFile(path, at, false)),
            Action::PreviewFile(path) => cx.emit(ModelEvent::OpenFile(path, None, true)),
            Action::FilterDir(dir) => {
                self.dir_filter = dir;
                self.anchor = None;
            }
            _ if repo.is_none() => {}
            Action::Stage(paths) => {
                let n = paths.len();
                local(self, format!("Staged {n} file(s)"), Box::new(move |r| r.stage(&paths)), cx);
            }
            Action::Unstage(paths) => local(self, "Unstaged".into(), Box::new(move |r| r.unstage(&paths)), cx),
            Action::AskDiscard(entries) => self.ask(Dialog::ConfirmDiscard(entries), cx),
            Action::Discard(entries) => {
                let n = entries.len();
                local(self, format!("Discarded {n} file(s)"), Box::new(move |r| r.discard(&entries)), cx);
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
                local(self, label, Box::new(move |r| r.delete_files(&entries, true)), cx);
            }
            Action::Commit => {
                if !self.can_commit() {
                    return;
                }
                let (message, amend, paths) = (self.commit_msg.clone(), self.amend, self.commit_scope().paths());
                self.queue(cx, move |repo| {
                    let r = repo.commit(&message, amend, &paths).map_err(|e| e.to_string());
                    Box::new(move |m, cx| match r {
                        Ok(summary) => {
                            m.done(format!("Committed {summary}"));
                            m.commit_msg.clear();
                            m.amend = false;
                            cx.emit(ModelEvent::Message);
                        }
                        Err(e) => m.fail(e, cx),
                    })
                });
                self.refresh(cx);
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
                self.queue(cx, move |repo| {
                    let r = repo.checkout(&target, mode).map_err(|e| e.to_string());
                    Box::new(move |m, cx| match r {
                        Ok(outcome) => {
                            m.log_output(outcome.output, false);
                            m.done(format!("Checked out {}", target.label()));
                            if let Some(warning) = outcome.warning {
                                m.log_output(warning.clone(), true);
                                let title = "Local changes conflicted".into();
                                m.ask(Dialog::Info { title, text: warning, mood: Mood::Warning }, cx);
                            }
                        }
                        Err(e) => m.fail(e, cx),
                    })
                });
                self.refresh(cx);
            }
            Action::ShowNewBranch(start) => self.ask(Dialog::NewBranch(start), cx),
            Action::CreateBranch { name, start, checkout } => {
                let label = format!("Created branch {name}");
                local(self, label, Box::new(move |r| r.create_branch(&name, start.as_deref(), checkout)), cx);
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
                self.queue(cx, move |repo| {
                    let r = repo.delete_branch(&name, force).map_err(|e| e.to_string());
                    Box::new(move |m, cx| match r {
                        Ok(()) => {
                            m.done(format!("Deleted branch {name}"));
                            if let Some(upstream) = upstream {
                                m.run(Action::DeleteRemoteBranch(upstream), cx);
                            }
                        }
                        Err(e) => m.fail(e, cx),
                    })
                });
                self.refresh(cx);
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
                self.queue(cx, move |repo| {
                    let r = repo.reset_to_upstream().map_err(|e| e.to_string());
                    Box::new(move |m, cx| match r {
                        Ok(backup) => {
                            m.done(format!("Reset {head} to {upstream}"));
                            let text = format!(
                                "Your previous commits are kept on the branch {backup}. Delete it once you no longer need them."
                            );
                            let title = format!("{head} now matches {upstream}");
                            m.ask(Dialog::Info { title, text, mood: Mood::Done }, cx);
                        }
                        Err(e) => m.fail(e, cx),
                    })
                });
                self.refresh(cx);
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
                let read = cx.background_executor().spawn(async move { repo.commit_details(&rev) });
                self._show_commit = Some(cx.spawn(async move |this, cx| {
                    let details = read.await.map_err(|e| e.to_string());
                    let _ = this.update(cx, |m, cx| {
                        let selected = details.as_ref().ok().and_then(|d| d.files.first()).map(|f| f.path.clone());
                        if let Ok(d) = &details {
                            m.message = format!("Commit {} {}", d.short_id, d.summary());
                        }
                        m.commit_view = Some(CommitView { details, selected, diff: None });
                        m.update_commit_diff(cx);
                        cx.emit(ModelEvent::ShowCommit);
                        cx.notify();
                    });
                }));
            }
            Action::CloseCommit => {
                self.commit_view = None;
                self.message = "Working tree".into();
            }
        }
    }

    /// Ask for a folder, then run `then` with it.
    fn pick_folder(&mut self, prompt: &str, then: fn(PathBuf) -> Action, cx: &mut Context<Self>) {
        if crate::remote::is_remote(cx) {
            cx.emit(ModelEvent::PickRemoteFolder(AfterPick(then)));
            return;
        }
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
