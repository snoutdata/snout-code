use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Key, RichText, Sense};
use egui_extras::{Column, TableBuilder};
use serde::{Deserialize, Serialize};
use snout_core::groups::dir_name;
use snout_core::{
    Branch, BranchKind, CheckoutTarget, Commit, CommitDetails, CommitFile, DiffBase, FileEntry, LocalChanges, PullMode,
    PullProblem, Repo, Scope, Status, Tag, Tracking, on_disk, repo_dir_name, stage_paths, unstage_paths,
};

use crate::art::{self, Glyph, Paint as _, glyph_button};
use crate::diff_view::DiffView;
use crate::graph::{self, GraphRow};
use crate::theme::{self, Palette, ThemeChoice};
use console::{Console, ConsoleEvent};
use dialogs::{CheckoutPicker, Dialog, Mood, RemoteRef, StartPoint};
use draft::Draft;
use repos::RepoGroup;
use survey::Survey;

mod console;
mod dialogs;
mod draft;
mod menu;
mod repos;
mod survey;
#[cfg(test)]
mod tests;

const LOG_LIMIT: usize = 300;
/// How deep "Find repositories in folder…" searches.
const SCAN_DEPTH: usize = 4;

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    repos: Vec<PathBuf>,
    groups: Vec<RepoGroup>,
    current: Option<PathBuf>,
    diff_base: DiffBase,
    side_by_side: bool,
    compact: bool,
    /// The Claude Code console is showing.
    console_open: bool,
    theme: ThemeChoice,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            groups: Vec::new(),
            current: None,
            diff_base: DiffBase::default(),
            side_by_side: true,
            compact: true,
            console_open: false,
            theme: ThemeChoice::default(),
        }
    }
}

/// The part of [`Settings`] written to disk as soon as it changes.
#[derive(Default)]
struct Persisted {
    repos: Vec<PathBuf>,
    groups: Vec<RepoGroup>,
    current: Option<PathBuf>,
}

impl Persisted {
    fn of(s: &Settings) -> Self {
        Self { repos: s.repos.clone(), groups: s.groups.clone(), current: s.current.clone() }
    }

    fn matches(&self, s: &Settings) -> bool {
        self.repos == s.repos && self.groups == s.groups && self.current == s.current
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    Name,
    State,
    Directory,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Journal,
    Output,
}

enum Action {
    Refresh,
    /// Open a repository from the list.
    OpenRepo(PathBuf),
    /// Ask for a folder, then [`Action::OpenPath`].
    PickFolder,
    /// Open a picked or dropped path; offers to initialize or search it if it is not a repository.
    OpenPath(PathBuf),
    InitRepo(PathBuf),
    PickScanFolder,
    Scan(PathBuf),
    RemoveRepo(PathBuf),
    /// Ask for a name, then make a group (and put the repository in it).
    ShowNewGroup(Option<PathBuf>),
    ShowRenameGroup(String),
    SaveGroup {
        /// The group being renamed; a new one when `None`.
        rename: Option<String>,
        name: String,
        repo: Option<PathBuf>,
    },
    /// Put a repository in a group, or in none.
    MoveToGroup(PathBuf, Option<String>),
    /// Remove a group; its repositories stay in the list.
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
    /// Ask, then move files to the Trash.
    AskDeleteFiles(Vec<FileEntry>),
    DeleteFiles(Vec<FileEntry>),
    Commit,
    /// Have Claude Code write the commit message from what the commit would record.
    DraftMessage,
    CancelDraft,
    ShowCheckout,
    /// Check out, asking first what to do with local changes if there are any.
    RequestCheckout(CheckoutTarget),
    Checkout(CheckoutTarget, LocalChanges),
    ShowNewBranch(Option<StartPoint>),
    CreateBranch {
        name: String,
        start: Option<String>,
        checkout: bool,
    },
    /// Ask, then delete a local branch (and maybe its upstream) or a remote one.
    AskDeleteBranch(Branch),
    DeleteBranch {
        name: String,
        force: bool,
        /// Also delete this branch on its remote.
        upstream: Option<RemoteRef>,
    },
    DeleteRemoteBranch(RemoteRef),
    Fetch,
    Pull(PullMode),
    /// Make the current branch match its upstream, keeping a backup branch.
    ResetToUpstream,
    Push,
    OpenGithub,
    ToggleConsole,
    /// Show one commit in place of the working tree.
    ShowCommit(String),
    CloseCommit,
    SetTheme(ThemeChoice),
    Reveal(PathBuf),
    Copy(String),
}

enum JobOutcome {
    Done(String),
    Cloned(Repo, String),
    Found { root: PathBuf, repos: Vec<PathBuf> },
}

struct Job {
    label: String,
    rx: mpsc::Receiver<Result<JobOutcome, String>>,
}

/// A commit shown in place of the working tree, after a click in the Journal.
struct CommitView {
    details: Result<CommitDetails, String>,
    /// Path of the file whose diff is showing.
    selected: Option<String>,
    diff_key: Option<(String, bool)>,
    diff: Option<Result<DiffView, String>>,
}

impl CommitView {
    fn load(repo: &Repo, rev: &str) -> Self {
        let details = repo.commit_details(rev).map_err(|e| e.to_string());
        let selected = details.as_ref().ok().and_then(|d| d.files.first()).map(|f| f.path.clone());
        Self { details, selected, diff_key: None, diff: None }
    }

    fn id(&self) -> Option<&str> {
        self.details.as_ref().ok().map(|d| d.id.as_str())
    }

    fn selected_file(&self) -> Option<&CommitFile> {
        let details = self.details.as_ref().ok()?;
        details.files.iter().find(|f| Some(&f.path) == self.selected.as_ref())
    }
}

struct OutputLine {
    text: String,
    error: bool,
}

pub struct SnoutApp {
    settings: Settings,
    repo: Option<Repo>,
    status: Status,
    branches: Vec<Branch>,
    tags: Vec<Tag>,
    log: Vec<Commit>,
    graph: Vec<GraphRow>,
    journal_filter: String,
    commit_view: Option<CommitView>,
    github_url: Option<String>,
    /// Working-tree directories, for the tree under the repository.
    dirs: Vec<String>,
    /// Directory picked in the tree; the Files pane shows only what is under it.
    dir_filter: Option<String>,

    repo_filter: String,
    selected: BTreeSet<String>,
    anchor: Option<String>,
    filter: String,
    sort: (SortColumn, bool),

    diff_key: Option<(String, DiffBase, bool)>,
    diff: Option<Result<DiffView, String>>,

    commit_msg: String,
    amend: bool,
    /// Claude Code writing the commit message.
    draft: Option<Draft>,

    bottom_tab: BottomTab,
    output: Vec<OutputLine>,
    message: String,
    job: Option<Job>,
    dialog: Option<Dialog>,
    was_focused: bool,
    title: String,
    console: Console,
    /// The repository list, groups and current repository as last written to disk.
    persisted: Persisted,
    /// How each listed repository stands against its upstream (`None`: no upstream).
    tracking: HashMap<PathBuf, Option<Tracking>>,
    survey: Option<Survey>,
    /// When a survey last fetched, and how often one should; never in tests.
    fetched_at: Option<Instant>,
    fetch_every: Option<Duration>,
    /// Count again without fetching, e.g. after work done in other windows.
    recount: bool,
    /// The native menu bar, once installed (see [`SnoutApp::install_menu`]).
    native: Option<menu::Native>,
    /// Draw the menu bar in the window: where there is no native one.
    draw_menu: bool,
    /// Menu commands to run at the start of the next frame, before any widget.
    picked: Vec<menu::Command>,
}

impl SnoutApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open_arg: Option<PathBuf>) -> Self {
        install_fonts(&cc.egui_ctx);
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        theme::apply(&cc.egui_ctx, settings.theme);
        let mut app = Self {
            settings,
            repo: None,
            status: Status::default(),
            branches: Vec::new(),
            tags: Vec::new(),
            log: Vec::new(),
            graph: Vec::new(),
            journal_filter: String::new(),
            commit_view: None,
            github_url: None,
            dirs: Vec::new(),
            dir_filter: None,
            repo_filter: String::new(),
            selected: BTreeSet::new(),
            anchor: None,
            filter: String::new(),
            sort: (SortColumn::Name, true),
            diff_key: None,
            diff: None,
            commit_msg: String::new(),
            amend: false,
            draft: None,
            bottom_tab: BottomTab::Journal,
            output: Vec::new(),
            message: "Ready".into(),
            job: None,
            dialog: None,
            was_focused: true,
            title: String::new(),
            console: Console::default(),
            persisted: Persisted::default(),
            tracking: HashMap::new(),
            survey: None,
            fetched_at: None,
            fetch_every: (!cfg!(test)).then_some(survey::FETCH_EVERY),
            recount: false,
            native: None,
            draw_menu: !cfg!(target_os = "macos"),
            picked: Vec::new(),
        };
        app.persisted = Persisted::of(&app.settings);
        if let Some(path) = open_arg {
            app.open_path(&path);
        } else if let Some(path) = app.settings.current.clone() {
            app.open_repo(&path);
        }
        app
    }

    /// Put up the native menu bar. Not done in [`SnoutApp::new`], so tests run without one.
    pub fn install_menu(&mut self, ctx: &egui::Context) {
        self.native = menu::Native::install(ctx);
    }

    // ----- State ---------------------------------------------------------------------

    fn open_repo(&mut self, path: &Path) {
        match Repo::open(path) {
            Ok(repo) => {
                let root = repo.root().to_owned();
                self.remember(&root);
                self.settings.current = Some(root);
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
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// Open a path chosen by the user. If it is not inside a repository, offer to
    /// initialize one there or to search it for repositories.
    fn open_path(&mut self, path: &Path) {
        let dir = if path.is_file() { path.parent().unwrap_or(path) } else { path };
        if Repo::open(dir).is_ok() {
            self.open_repo(dir);
        } else if dir.is_dir() {
            self.dialog = Some(Dialog::InitRepo(dir.to_owned()));
        } else {
            self.fail(format!("{} does not exist", dir.display()));
        }
    }

    fn remember(&mut self, root: &Path) {
        if !self.settings.repos.iter().any(|p| p == root) {
            self.settings.repos.push(root.to_owned());
        }
    }

    fn refresh(&mut self) {
        self.diff_key = None;
        let Some(repo) = self.repo.clone() else {
            self.status = Status::default();
            self.branches.clear();
            self.tags.clear();
            self.log.clear();
            self.graph.clear();
            self.dirs.clear();
            self.commit_view = None;
            return;
        };
        match repo.status() {
            Ok(status) => {
                let present: BTreeSet<&String> = status.files.iter().map(|f| &f.path).collect();
                self.selected.retain(|p| present.contains(p));
                self.status = status;
            }
            Err(e) => return self.fail(e.to_string()),
        }
        let b = &self.status.branch;
        let tracking = b.upstream.is_some().then_some(Tracking { ahead: b.ahead, behind: b.behind });
        self.tracking.insert(repo.root().to_owned(), tracking);
        self.branches = repo.branches().unwrap_or_else(|e| {
            self.log_output(e.to_string(), true);
            Vec::new()
        });
        self.tags = repo.tags().unwrap_or_else(|e| {
            self.log_output(e.to_string(), true);
            Vec::new()
        });
        self.log = repo.log(LOG_LIMIT).unwrap_or_else(|e| {
            self.log_output(e.to_string(), true);
            Vec::new()
        });
        self.graph = graph::layout(&self.log);
        self.dirs = repo.directories().unwrap_or_else(|e| {
            self.log_output(e.to_string(), true);
            Vec::new()
        });
        if self.dir_filter.as_ref().is_some_and(|d| self.dirs.binary_search(d).is_err()) {
            self.dir_filter = None;
        }
    }

    /// What Commit records: the staged changes; with nothing staged, the selected files,
    /// or every changed file when none is selected.
    /// A message, something to record, and Claude not writing the message.
    fn can_commit(&self) -> bool {
        self.repo.is_some()
            && self.draft.is_none()
            && !self.commit_msg.trim().is_empty()
            && self.commit_scope() != Scope::Nothing
    }

    fn commit_scope(&self) -> Scope {
        Scope::of(&self.status, &self.selected_entries(), self.amend)
    }

    fn selected_entries(&self) -> Vec<FileEntry> {
        self.status.files.iter().filter(|f| self.selected.contains(&f.path)).cloned().collect()
    }

    /// Uncommitted changes to tracked files (untracked files never block a checkout
    /// unless the target contains the same path, which git reports anyway).
    fn has_tracked_changes(&self) -> bool {
        self.status.files.iter().any(|f| !f.is_untracked())
    }

    fn head_label(&self) -> String {
        let b = &self.status.branch;
        match (&b.head, &b.oid) {
            (Some(head), _) => head.clone(),
            (None, Some(oid)) => format!("detached at {}", &oid[..oid.len().min(7)]),
            (None, None) => "detached HEAD".into(),
        }
    }

    fn log_output(&mut self, text: String, error: bool) {
        if !text.trim().is_empty() {
            self.output.push(OutputLine { text, error });
        }
    }

    fn fail(&mut self, message: String) {
        self.message = message.lines().next().unwrap_or_default().to_owned();
        self.log_output(message.clone(), true);
        self.dialog = Some(Dialog::Error(message));
    }

    fn done(&mut self, summary: impl Into<String>) {
        self.message = summary.into();
        self.log_output(self.message.clone(), false);
    }

    /// Run a slow operation (network, filesystem scan) on a worker thread.
    fn spawn(
        &mut self,
        ctx: &egui::Context,
        label: &str,
        f: impl FnOnce() -> snout_core::Result<JobOutcome> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(f().map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
        self.message = format!("{label}…");
        self.job = Some(Job { label: label.to_owned(), rx });
    }

    fn poll_job(&mut self) {
        let Some(job) = &self.job else { return };
        let Ok(result) = job.rx.try_recv() else { return };
        let label = job.label.clone();
        self.job = None;
        match result {
            Ok(JobOutcome::Done(text)) => {
                self.log_output(text, false);
                self.message = format!("{label} finished");
            }
            Ok(JobOutcome::Cloned(repo, text)) => {
                self.log_output(text, false);
                let root = repo.root().to_owned();
                self.open_repo(&root);
                self.message = format!("Cloned into {}", root.display());
            }
            Ok(JobOutcome::Found { root, repos }) => self.add_found(&root, repos),
            Err(e) => match PullProblem::from_error(&e).filter(|_| label.starts_with("Pull")) {
                Some(problem) => {
                    self.message = format!("{label} failed");
                    self.log_output(e.clone(), true);
                    self.dialog = Some(Dialog::PullProblem { problem, details: e });
                }
                None => self.fail(format!("{label} failed:\n{e}")),
            },
        }
        self.refresh();
    }

    fn poll_draft(&mut self) {
        let Some(result) = self.draft.as_ref().and_then(Draft::poll) else { return };
        self.draft = None;
        match result {
            Ok(message) => {
                self.commit_msg = message;
                self.done("Claude drafted the commit message");
            }
            Err(e) => self.fail(format!("Claude could not draft the message:\n{e}")),
        }
    }

    fn start_draft(&mut self, ctx: &egui::Context) {
        let Some(repo) = self.repo.clone() else { return };
        let Some(program) = self.console.claude() else {
            return self.fail(
                "Claude Code was not found. Install it from https://docs.claude.com/en/docs/claude-code.".into(),
            );
        };
        let patch = match repo.commit_patch(self.amend, &self.commit_scope().paths()) {
            Ok(patch) => patch,
            Err(e) => return self.fail(e.to_string()),
        };
        // When amending, the last subject is the one being rewritten.
        let recent: Vec<&str> =
            self.log.iter().skip(usize::from(self.amend)).take(12).map(|c| c.summary.as_str()).collect();
        let prompt = draft::prompt(&recent, &self.commit_msg, self.amend);
        match Draft::start(&program, repo.root(), &prompt, &patch, {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        }) {
            Ok(draft) => {
                self.draft = Some(draft);
                self.message = "Claude is drafting the commit message…".into();
            }
            Err(e) => self.fail(e),
        }
    }

    fn add_found(&mut self, root: &Path, repos: Vec<PathBuf>) {
        if repos.is_empty() {
            self.dialog = Some(Dialog::Info {
                title: "No repositories found".into(),
                text: format!("No Git repositories within {SCAN_DEPTH} levels of\n{}", root.display()),
                mood: Mood::Empty,
            });
            return;
        }
        let before = self.settings.repos.len();
        for r in &repos {
            self.remember(r);
        }
        let added = self.settings.repos.len() - before;
        if self.repo.is_none() {
            self.open_repo(&repos[0]);
        }
        self.done(format!("Found {} repositories in {} ({added} new)", repos.len(), root.display()));
    }

    fn handle(&mut self, action: Action, ctx: &egui::Context) {
        let repo = self.repo.clone();
        // Local operations run synchronously; they are fast and the result is needed
        // immediately for the next frame anyway.
        let local = |app: &mut Self, label: &str, r: snout_core::Result<()>| {
            match r {
                Ok(()) => app.done(label),
                Err(e) => app.fail(e.to_string()),
            }
            app.refresh();
        };
        match action {
            Action::Refresh => {
                self.refresh();
                self.message = "Refreshed".into();
            }
            Action::OpenRepo(path) => self.open_repo(&path),
            Action::PickFolder => {
                if let Some(dir) = rfd::FileDialog::new().set_title("Open local repository").pick_folder() {
                    self.open_path(&dir);
                }
            }
            Action::OpenPath(path) => self.open_path(&path),
            Action::InitRepo(path) => match Repo::init(&path) {
                Ok(repo) => {
                    let root = repo.root().to_owned();
                    self.open_repo(&root);
                    self.done(format!("Initialized repository in {}", root.display()));
                }
                Err(e) => self.fail(e.to_string()),
            },
            Action::PickScanFolder => {
                if let Some(dir) = rfd::FileDialog::new().set_title("Find repositories in folder").pick_folder() {
                    self.handle(Action::Scan(dir), ctx);
                }
            }
            Action::Scan(root) => self.spawn(ctx, "Searching for repositories", move || {
                let repos = snout_core::find_repos(&root, SCAN_DEPTH);
                Ok(JobOutcome::Found { root, repos })
            }),
            Action::RemoveRepo(path) => {
                self.settings.repos.retain(|p| p != &path);
                repos::move_to_group(&mut self.settings.groups, &path, None);
                if self.settings.current.as_ref() == Some(&path) {
                    self.settings.current = None;
                    self.repo = None;
                    self.github_url = None;
                    self.refresh();
                }
            }
            Action::ShowNewGroup(repo) => {
                self.dialog = Some(Dialog::GroupName { rename: None, name: String::new(), repo });
            }
            Action::ShowRenameGroup(name) => {
                self.dialog = Some(Dialog::GroupName { rename: Some(name.clone()), name, repo: None });
            }
            Action::SaveGroup { rename, name, repo } => {
                snout_core::groups::save_group(&mut self.settings.groups, rename.as_deref(), &name, repo.as_deref());
            }
            Action::MoveToGroup(path, group) => {
                repos::move_to_group(&mut self.settings.groups, &path, group.as_deref())
            }
            Action::DeleteGroup(name) => self.settings.groups.retain(|g| g.name != name),
            Action::ShowClone => {
                let parent = self
                    .settings
                    .current
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.dialog = Some(Dialog::Clone { url: String::new(), parent });
            }
            Action::Clone { url, dest } => self.spawn(ctx, "Clone", move || {
                Repo::clone_from(&url, &dest).map(|(repo, text)| JobOutcome::Cloned(repo, text))
            }),
            Action::SetTheme(choice) => {
                self.settings.theme = choice;
                theme::apply(ctx, choice);
            }
            _ if repo.is_none() => {}
            Action::Stage(paths) => {
                let n = paths.len();
                local(self, &format!("Staged {n} file(s)"), repo.unwrap().stage(&paths));
            }
            Action::Unstage(paths) => {
                local(self, "Unstaged", repo.unwrap().unstage(&paths));
            }
            Action::AskDiscard(entries) => self.dialog = Some(Dialog::ConfirmDiscard(entries)),
            Action::Discard(entries) => {
                let n = entries.len();
                local(self, &format!("Discarded {n} file(s)"), repo.unwrap().discard(&entries));
            }
            Action::AskDeleteFiles(entries) => {
                let entries: Vec<FileEntry> = entries.into_iter().filter(on_disk).collect();
                if !entries.is_empty() {
                    self.dialog = Some(Dialog::ConfirmDeleteFiles(entries));
                }
            }
            Action::DeleteFiles(entries) => {
                let n = entries.len();
                let label = if snout_core::can_trash() {
                    format!("Moved {n} file(s) to the Trash")
                } else {
                    format!("Deleted {n} file(s)")
                };
                local(self, &label, repo.unwrap().delete_files(&entries, true));
            }
            Action::Commit => match repo.unwrap().commit(&self.commit_msg, self.amend, &self.commit_scope().paths()) {
                Ok(summary) => {
                    self.done(format!("Committed {summary}"));
                    self.commit_msg.clear();
                    self.amend = false;
                    self.refresh();
                }
                Err(e) => self.fail(e.to_string()),
            },
            Action::DraftMessage => self.start_draft(ctx),
            Action::CancelDraft => {
                self.draft = None;
                self.message = "Stopped drafting".into();
            }
            Action::ShowCheckout => self.dialog = Some(Dialog::Checkout(CheckoutPicker::default())),
            Action::RequestCheckout(target) => {
                if self.has_tracked_changes() {
                    self.dialog = Some(Dialog::ConfirmCheckout(target));
                } else {
                    self.handle(Action::Checkout(target, LocalChanges::Keep), ctx);
                }
            }
            Action::Checkout(target, mode) => {
                match repo.unwrap().checkout(&target, mode) {
                    Ok(outcome) => {
                        self.log_output(outcome.output, false);
                        self.done(format!("Checked out {}", target.label()));
                        if let Some(warning) = outcome.warning {
                            self.log_output(warning.clone(), true);
                            self.dialog = Some(Dialog::Info {
                                title: "Local changes conflicted".into(),
                                text: warning,
                                mood: Mood::Warning,
                            });
                        }
                    }
                    Err(e) => self.fail(e.to_string()),
                }
                self.refresh();
            }
            Action::ShowNewBranch(start) => {
                self.dialog = Some(Dialog::NewBranch { name: String::new(), start, checkout: true });
            }
            Action::CreateBranch { name, start, checkout } => {
                let r = repo.unwrap().create_branch(&name, start.as_deref(), checkout);
                local(self, &format!("Created branch {name}"), r);
            }
            Action::AskDeleteBranch(b) => {
                self.dialog = Some(match b.kind {
                    BranchKind::Local => {
                        // Offer the upstream too, when it still exists.
                        let upstream = b
                            .upstream
                            .as_ref()
                            .filter(|_| !b.upstream_gone)
                            .and_then(|u| self.branches.iter().find(|r| r.kind == BranchKind::Remote && &r.name == u))
                            .and_then(RemoteRef::of);
                        Dialog::DeleteBranch { name: b.name, force: false, upstream, with_upstream: false }
                    }
                    BranchKind::Remote => match RemoteRef::of(&b) {
                        Some(r) => Dialog::DeleteRemoteBranch(r),
                        None => return,
                    },
                });
            }
            Action::DeleteBranch { name, force, upstream } => {
                let deleted = repo.unwrap().delete_branch(&name, force);
                let ok = deleted.is_ok();
                local(self, &format!("Deleted branch {name}"), deleted);
                if let Some(upstream) = upstream.filter(|_| ok) {
                    self.handle(Action::DeleteRemoteBranch(upstream), ctx);
                }
            }
            Action::DeleteRemoteBranch(r) => {
                let repo = repo.unwrap();
                self.spawn(ctx, &format!("Delete {}", r.label()), move || {
                    repo.delete_remote_branch(&r.remote, &r.branch).map(JobOutcome::Done)
                });
            }
            Action::Fetch => {
                let repo = repo.unwrap();
                self.spawn(ctx, "Fetch", move || repo.fetch().map(JobOutcome::Done));
            }
            Action::Pull(mode) => {
                let repo = repo.unwrap();
                let label = match mode {
                    PullMode::Default => "Pull",
                    PullMode::Merge => "Pull (merge)",
                    PullMode::Rebase => "Pull (rebase)",
                };
                self.spawn(ctx, label, move || repo.pull_with(mode).map(JobOutcome::Done));
            }
            Action::ResetToUpstream => {
                let upstream = self.status.branch.upstream.clone().unwrap_or_default();
                let head = self.status.branch.head.clone().unwrap_or_default();
                match repo.unwrap().reset_to_upstream() {
                    Ok(backup) => {
                        self.done(format!("Reset {head} to {upstream}"));
                        self.dialog = Some(Dialog::Info {
                            title: format!("{head} now matches {upstream}"),
                            text: format!(
                                "Your previous commits are kept on the branch {backup}. Delete it once you no longer need them."
                            ),
                            mood: Mood::Done,
                        });
                    }
                    Err(e) => self.fail(e.to_string()),
                }
                self.refresh();
            }
            Action::Push => {
                let repo = repo.unwrap();
                self.spawn(ctx, "Push", move || repo.push().map(JobOutcome::Done));
            }
            Action::OpenGithub => {
                if let Some(url) = &self.github_url {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            }
            Action::Reveal(path) => {
                let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
            }
            Action::Copy(text) => ctx.copy_text(text),
            Action::ShowCommit(rev) => {
                let view = CommitView::load(repo.as_ref().unwrap(), &rev);
                if let Ok(d) = &view.details {
                    self.message = format!("Commit {} {}", d.short_id, d.summary());
                }
                self.commit_view = Some(view);
            }
            Action::CloseCommit => {
                self.commit_view = None;
                self.message = "Working tree".into();
            }
            Action::ToggleConsole => {
                self.settings.console_open = !self.settings.console_open;
                if self.settings.console_open {
                    self.console.focus();
                }
            }
        }
    }

    // ----- Frame-level behaviour -----------------------------------------------------

    /// Re-read the repository when the window regains focus, so changes made elsewhere
    /// (terminal, editor, an agent) show up without pressing F5.
    fn auto_refresh(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        if focused && !self.was_focused && self.job.is_none() {
            self.refresh();
            self.recount = true;
        }
        self.was_focused = focused;
    }

    /// Keep the ahead and behind counts of the listed repositories current: take in what a
    /// running survey found, and start the next one when a fetch is due, when asked to
    /// count again, or when the list has repositories not yet counted.
    fn survey(&mut self, ctx: &egui::Context) {
        if let Some(survey) = &self.survey {
            survey.hold(self.job.is_some());
            let (done, over) = survey.poll();
            let fetched = survey.fetches;
            for (path, tracking) in done {
                let open = self.settings.current.as_ref() == Some(&path);
                self.tracking.insert(path, tracking);
                if fetched && open && self.job.is_none() {
                    self.refresh(); // its remote branches may have moved too
                }
            }
            if !over {
                return;
            }
            self.survey = None;
        }
        let repos = self.settings.repos.clone();
        let due = self.fetch_every.is_some_and(|every| self.fetched_at.is_none_or(|at| at.elapsed() >= every));
        let wake = {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        if due {
            self.fetched_at = Some(Instant::now());
            self.survey = Some(Survey::start(repos, true, wake));
        } else if self.recount || repos.iter().any(|p| !self.tracking.contains_key(p)) {
            self.survey = Some(Survey::start(repos, false, wake));
        }
        self.recount = false;
        if let (Some(every), Some(at)) = (self.fetch_every, self.fetched_at) {
            ctx.request_repaint_after(every.saturating_sub(at.elapsed()));
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.dialog.is_some() {
            return;
        }
        let (refresh, open, checkout, console, delete) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                i.key_pressed(Key::F5) || (cmd && i.key_pressed(Key::R)),
                cmd && !i.modifiers.shift && i.key_pressed(Key::O),
                cmd && i.modifiers.shift && i.key_pressed(Key::O),
                cmd && i.key_pressed(Key::J),
                cmd && i.key_pressed(Key::Backspace),
            )
        });
        // ⌘⌫ moves the selected files to the Trash, as in Finder, unless a text field or
        // the console has the keyboard.
        if delete && self.commit_view.is_none() && ctx.memory(|m| m.focused().is_none()) {
            let entries = self.selected_entries();
            if entries.iter().any(on_disk) {
                actions.push(Action::AskDeleteFiles(entries));
            }
        }
        if console {
            actions.push(Action::ToggleConsole);
        }
        if refresh {
            actions.push(Action::Refresh);
        }
        if open {
            actions.push(Action::PickFolder);
        }
        if checkout && self.repo.is_some() {
            actions.push(Action::ShowCheckout);
        }
        self.commit_view_keys(ctx, actions);
    }

    /// In the commit view, Esc goes back to the working tree and ↑ ↓ step through the
    /// Journal, unless a text field or the console has the keyboard.
    fn commit_view_keys(&self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let Some(view) = &self.commit_view else { return };
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let (esc, up, down) =
            ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
        if esc {
            actions.push(Action::CloseCommit);
            return;
        }
        let visible = self.journal_rows();
        let at = visible.iter().position(|&i| Some(self.log[i].id.as_str()) == view.id());
        let next = match (at, up, down) {
            (Some(at), true, _) => at.checked_sub(1),
            (Some(at), _, true) => Some(at + 1),
            (None, true, _) | (None, _, true) => Some(0),
            _ => None,
        };
        if let Some(&i) = next.and_then(|n| visible.get(n)) {
            actions.push(Action::ShowCommit(self.log[i].id.clone()));
        }
    }

    /// Indices into `log` of the commits the Journal shows, after its filter.
    fn journal_rows(&self) -> Vec<usize> {
        let needle = self.journal_filter.trim().to_lowercase();
        (0..self.log.len())
            .filter(|&i| {
                let c = &self.log[i];
                needle.is_empty()
                    || c.summary.to_lowercase().contains(&needle)
                    || c.author.to_lowercase().contains(&needle)
                    || c.id.starts_with(&needle)
                    || c.refs.iter().any(|r| r.to_lowercase().contains(&needle))
            })
            .collect()
    }

    /// Folders dropped onto the window are opened; show a hint while hovering.
    fn drag_and_drop(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let (hovering, dropped) = ctx.input(|i| {
            let dropped: Vec<PathBuf> = i
                .raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
            (!i.raw.hovered_files.is_empty(), dropped)
        });
        actions.extend(dropped.into_iter().map(Action::OpenPath));
        if hovering {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop")));
            let rect = ctx.content_rect();
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(160));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop a folder to open it",
                egui::FontId::proportional(24.0),
                Color32::WHITE,
            );
        }
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
                .map(|d| DiffView::new(d, self.settings.compact))
                .map_err(|e| e.to_string()),
        );
        self.diff_key = Some(key);
    }

    /// Write the settings as soon as the repository list or the current repository
    /// changes, instead of waiting for eframe's periodic save, so a crash or a forced
    /// quit cannot lose them.
    fn persist_repos(&mut self, frame: &mut eframe::Frame) {
        if self.persisted.matches(&self.settings) {
            return;
        }
        if let Some(storage) = frame.storage_mut() {
            eframe::set_value(storage, eframe::APP_KEY, &self.settings);
            storage.flush();
        }
        self.persisted = Persisted::of(&self.settings);
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match &self.repo {
            Some(repo) => format!("{} ({}) · SnoutCode", repo.name(), self.head_label()),
            None => "SnoutCode".into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    // ----- Panes ---------------------------------------------------------------------

    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let has_repo = self.repo.is_some();
        let idle = self.job.is_none();
        let entries = self.selected_entries();
        let in_tree = self.commit_view.is_none();
        let can_stage = in_tree && entries.iter().any(FileEntry::has_unstaged);
        let can_unstage = in_tree && entries.iter().any(FileEntry::has_staged);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if glyph_button(ui, Glyph::Open, "Open", true).on_hover_text("Open local repository (⌘O)").clicked() {
                actions.push(Action::PickFolder);
            }
            ui.separator();
            let net = has_repo && idle;
            if glyph_button(ui, Glyph::Pull, "Pull", net).on_hover_text("git pull").clicked() {
                actions.push(Action::Pull(PullMode::Default));
            }
            if glyph_button(ui, Glyph::Fetch, "Fetch", net).on_hover_text("git fetch --all --prune").clicked() {
                actions.push(Action::Fetch);
            }
            let push_label = match self.status.branch.ahead {
                0 => "Push".to_owned(),
                n => format!("Push ({n})"),
            };
            if glyph_button(ui, Glyph::Push, &push_label, net)
                .on_hover_text("git push (sets upstream on first push)")
                .clicked()
            {
                actions.push(Action::Push);
            }
            ui.separator();
            if glyph_button(ui, Glyph::Stage, "Stage", can_stage).clicked() {
                actions.push(Action::Stage(stage_paths(&entries)));
            }
            if glyph_button(ui, Glyph::Unstage, "Unstage", can_unstage).clicked() {
                actions.push(Action::Unstage(unstage_paths(&entries)));
            }
            if glyph_button(ui, Glyph::Discard, "Discard", in_tree && !entries.is_empty()).clicked() {
                actions.push(Action::AskDiscard(entries.clone()));
            }
            if glyph_button(ui, Glyph::Trash, "Delete", in_tree && entries.iter().any(on_disk))
                .on_hover_text("Move to the Trash (⌘⌫)")
                .clicked()
            {
                actions.push(Action::AskDeleteFiles(entries.clone()));
            }
            ui.separator();
            if glyph_button(ui, Glyph::Refresh, "Refresh", has_repo).on_hover_text("F5 / ⌘R").clicked() {
                actions.push(Action::Refresh);
            }
            if glyph_button(ui, Glyph::Switch, "Check Out…", has_repo)
                .on_hover_text("Branch, tag or commit (⇧⌘O)")
                .clicked()
            {
                actions.push(Action::ShowCheckout);
            }
            if glyph_button(ui, Glyph::Branch, "New Branch", has_repo).clicked() {
                actions.push(Action::ShowNewBranch(None));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let open = self.settings.console_open;
                let claude = glyph_button(ui, Glyph::Spark, if open { "Hide Claude" } else { "Claude" }, true)
                    .on_hover_text("Claude Code console in this repository (⌘J)");
                if claude.clicked() {
                    actions.push(Action::ToggleConsole);
                }
                let gh = glyph_button(ui, Glyph::Web, "GitHub", self.github_url.is_some())
                    .on_hover_text(self.github_url.as_deref().unwrap_or("No GitHub remote"));
                if gh.clicked() {
                    actions.push(Action::OpenGithub);
                }
                let theme = glyph_button(ui, Glyph::Theme, "", true).on_hover_text("Appearance");
                egui::Popup::menu(&theme).show(|ui| {
                    for choice in ThemeChoice::ALL {
                        if ui.radio(self.settings.theme == choice, choice.label()).clicked() {
                            actions.push(Action::SetTheme(choice));
                        }
                    }
                });
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.job.is_some() {
                ui.spinner();
            }
            ui.label(&self.message);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = &self.status.branch;
                if self.repo.is_some() {
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
                    ui.weak(s);
                    Glyph::Branch.drawing().show(ui, egui::vec2(14.0, 14.0));
                }
            });
        });
    }

    fn branches_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            pane_title(ui, "Branches");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_repo = self.repo.is_some();
                ui.spacing_mut().item_spacing.x = 2.0;
                if art::icon_button(ui, Glyph::Fetch, has_repo && self.job.is_none()).on_hover_text("Fetch").clicked() {
                    actions.push(Action::Fetch);
                }
                if art::icon_button(ui, Glyph::Branch, has_repo).on_hover_text("New branch").clicked() {
                    actions.push(Action::ShowNewBranch(None));
                }
                if art::icon_button(ui, Glyph::Switch, has_repo).on_hover_text("Check out… (⇧⌘O)").clicked() {
                    actions.push(Action::ShowCheckout);
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            // Keep the full area even when empty, or the panel shrinks to fit and stays small.
            if self.repo.is_none() {
                ui.weak("No repository open");
                return;
            }
            if self.status.branch.head.is_none() {
                ui.horizontal(|ui| {
                    ui.add_sized([12.0, ui.spacing().interact_size.y], egui::Label::new("●"));
                    ui.label(RichText::new(format!("HEAD ({})", self.head_label())).strong())
                        .on_hover_text("Not on a branch. Create a branch here to keep new commits.");
                });
            }
            let local: Vec<&Branch> = self.branches.iter().filter(|b| b.kind == BranchKind::Local).collect();
            egui::CollapsingHeader::new("Local Branches").default_open(true).show(ui, |ui| {
                for b in local {
                    branch_row(ui, b, actions);
                }
            });
            let mut remotes: Vec<&str> = self.branches.iter().filter_map(|b| b.remote.as_deref()).collect();
            remotes.dedup();
            for remote in remotes {
                egui::CollapsingHeader::new(remote).default_open(false).show(ui, |ui| {
                    for b in self.branches.iter().filter(|b| b.remote.as_deref() == Some(remote)) {
                        branch_row(ui, b, actions);
                    }
                });
            }
            if !self.tags.is_empty() {
                egui::CollapsingHeader::new(format!("Tags ({})", self.tags.len())).default_open(false).show(ui, |ui| {
                    for t in &self.tags {
                        tag_row(ui, t, actions);
                    }
                });
            }
        });
    }

    /// Shown in place of the working-tree panes when no repository is open.
    fn welcome_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let p = Palette::of(ui.visuals());
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.06).max(12.0));
                art::welcome().show(ui, egui::vec2(360.0, 228.0));
                ui.add_space(6.0);
                wordmark(ui, p);
                ui.add_space(2.0);
                ui.label(RichText::new("See what changed, read the diff, commit, ship.").color(p.fg_dim).size(15.0));
                ui.add_space(22.0);
                let size = egui::vec2(300.0, 36.0);
                for (glyph, text, action) in [
                    (Glyph::Open, "Open local repository…   ⌘O", Action::PickFolder),
                    (Glyph::Fetch, "Find repositories in folder…", Action::PickScanFolder),
                    (Glyph::Pull, "Clone from GitHub…", Action::ShowClone),
                ] {
                    if art::glyph_button_sized(ui, glyph, text, true, size).clicked() {
                        actions.push(action);
                    }
                }
                ui.add_space(8.0);
                ui.weak("or drop a folder onto this window");

                if !self.settings.repos.is_empty() {
                    ui.add_space(26.0);
                    ui.label(RichText::new("RECENT").small().color(p.fg_dim));
                    ui.add_space(2.0);
                    for path in self.settings.repos.iter().rev().take(8) {
                        let resp = ui.link(dir_name(path)).on_hover_text(path.display().to_string());
                        if resp.clicked() {
                            actions.push(Action::OpenRepo(path.clone()));
                        }
                    }
                }
            });
        });
    }

    fn files_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            pane_title(ui, "Files");
            ui.weak(format!("{} changed, {} staged", self.status.files.len(), self.status.staged_count()));
            if let Some(dir) = &self.dir_filter {
                let chip = ui.add(egui::Button::new(format!("in {dir}/  ✕")).small()).on_hover_text("Show all files");
                if chip.clicked() {
                    self.dir_filter = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("🔍 File filter").desired_width(220.0));
            });
        });
        ui.separator();
        if self.status.files.is_empty() {
            let text = match &self.status.branch.head {
                Some(head) => format!("Nothing to commit on {head}"),
                None => "Nothing to commit".to_owned(),
            };
            art_note(ui, art::clean(), "The working tree is clean", &text);
            return;
        }

        // Visible rows: filtered, then sorted.
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

        // Cmd+A selects all visible files when no text field has focus.
        let select_all =
            ui.input(|i| i.modifiers.command && i.key_pressed(Key::A)) && ui.ctx().memory(|m| m.focused().is_none());
        if select_all {
            self.selected = rows.iter().map(|f| f.path.clone()).collect();
        }

        let modifiers = ui.input(|i| i.modifiers);
        let mut clicked: Option<usize> = None;
        let mut sort_clicked: Option<SortColumn> = None;
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
        // Selectable labels would swallow clicks meant for selecting the row.
        ui.style_mut().interaction.selectable_labels = false;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .auto_shrink(false)
            .min_scrolled_height(0.0)
            .max_scroll_height(f32::INFINITY)
            .column(Column::initial(260.0).at_least(80.0).clip(true))
            .column(Column::initial(170.0).at_least(60.0).clip(true))
            .column(Column::remainder().clip(true))
            .header(row_h, |mut header| {
                for (label, c) in [
                    ("Name", SortColumn::Name),
                    ("State", SortColumn::State),
                    ("Relative Directory", SortColumn::Directory),
                ] {
                    header.col(|ui| {
                        let arrow = match (self.sort.0 == c, self.sort.1) {
                            (true, true) => " ⏶",
                            (true, false) => " ⏷",
                            _ => "",
                        };
                        let r = ui.add(
                            egui::Label::new(RichText::new(format!("{label}{arrow}")).strong()).sense(Sense::click()),
                        );
                        if r.clicked() {
                            sort_clicked = Some(c);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, rows.len(), |mut row| {
                    let f = rows[row.index()];
                    row.set_selected(self.selected.contains(&f.path));
                    row.col(|ui| {
                        ui.label(f.file_name());
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(f.state_label()).color(state_color(f, ui.visuals())));
                    });
                    row.col(|ui| {
                        ui.weak(f.directory());
                    });
                    let resp = row.response();
                    if resp.clicked() || resp.secondary_clicked() && !self.selected.contains(&f.path) {
                        clicked = Some(row.index());
                    }
                    resp.context_menu(|ui| {
                        let entries =
                            if self.selected.contains(&f.path) { self.selected_entries() } else { vec![f.clone()] };
                        if ui
                            .add_enabled(entries.iter().any(FileEntry::has_unstaged), egui::Button::new("Stage"))
                            .clicked()
                        {
                            actions.push(Action::Stage(stage_paths(&entries)));
                        }
                        if ui
                            .add_enabled(entries.iter().any(FileEntry::has_staged), egui::Button::new("Unstage"))
                            .clicked()
                        {
                            actions.push(Action::Unstage(unstage_paths(&entries)));
                        }
                        if ui.button("Discard…").clicked() {
                            actions.push(Action::AskDiscard(entries.clone()));
                        }
                        if ui.add_enabled(entries.iter().any(on_disk), egui::Button::new("Delete…")).clicked() {
                            actions.push(Action::AskDeleteFiles(entries.clone()));
                        }
                        ui.separator();
                        if let Some(repo) = &self.repo
                            && ui.button("Reveal in Finder").clicked()
                        {
                            actions.push(Action::Reveal(repo.root().join(&f.path)));
                        }
                        if ui.button("Copy path").clicked() {
                            actions.push(Action::Copy(f.path.clone()));
                        }
                    });
                });
            });

        if let Some(c) = sort_clicked {
            self.sort = if self.sort.0 == c { (c, !self.sort.1) } else { (c, true) };
        }
        if let Some(i) = clicked {
            let path = rows[i].path.clone();
            if modifiers.shift {
                let anchor = self.anchor.as_ref().and_then(|a| rows.iter().position(|f| &f.path == a)).unwrap_or(i);
                let (lo, hi) = (anchor.min(i), anchor.max(i));
                if !modifiers.command {
                    self.selected.clear();
                }
                self.selected.extend(rows[lo..=hi].iter().map(|f| f.path.clone()));
            } else if modifiers.command {
                if !self.selected.remove(&path) {
                    self.selected.insert(path.clone());
                }
                self.anchor = Some(path);
            } else {
                self.selected = BTreeSet::from([path.clone()]);
                self.anchor = Some(path);
            }
        }
    }

    fn changes_ui(&mut self, ui: &mut egui::Ui) {
        self.update_diff();
        ui.horizontal(|ui| {
            pane_title(ui, "Changes");
            diff_title(ui, self.diff.as_ref());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                diff_controls(ui, &mut self.settings);
                ui.separator();
                egui::ComboBox::from_id_salt("compare")
                    .selected_text(self.settings.diff_base.label())
                    .show_ui(ui, |ui| {
                        for base in DiffBase::ALL {
                            ui.selectable_value(&mut self.settings.diff_base, base, base.label());
                        }
                    })
                    .response
                    .on_hover_text("What to compare");
            });
        });
        ui.separator();

        let empty = if self.selected.len() > 1 {
            format!("{} files selected", self.selected.len())
        } else {
            "Select a file to see its changes".to_owned()
        };
        diff_body(ui, self.diff.as_ref(), self.settings.diff_base.sides(), self.settings.side_by_side, &empty);
    }

    fn commit_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let scope = self.commit_scope();
        let has_changes = self.repo.is_some() && scope != Scope::Nothing;
        ui.horizontal(|ui| {
            pane_title(ui, "Commit");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.draft.is_some() {
                    if art::chip_button(ui, Glyph::Close, "Stop", true).clicked() {
                        actions.push(Action::CancelDraft);
                    }
                    ui.spinner();
                    return;
                }
                let tip = if has_changes {
                    format!("Claude Code writes the message from {}, building on anything you typed", scope.what())
                } else {
                    "Nothing to commit".to_owned()
                };
                if art::chip_button(ui, Glyph::Spark, "Draft", has_changes).on_hover_text(tip).clicked() {
                    actions.push(Action::DraftMessage);
                }
            });
        });
        ui.separator();
        let can_commit = has_changes && !self.commit_msg.trim().is_empty();

        egui::Panel::bottom("commit_actions").show(ui, |ui| {
            // Side by side when there is room, the button under the checkbox when not.
            let narrow = ui.available_width() < 320.0;
            let had_amend = self.amend;
            let has_log = !self.log.is_empty();
            let commit = |ui: &mut egui::Ui, actions: &mut Vec<Action>| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let hint = format!("Commits {}  ⌘↩", scope.what());
                    if theme::primary_button(ui, can_commit, "Commit").on_hover_text(hint).clicked() {
                        actions.push(Action::Commit);
                    }
                    ui.add(egui::Label::new(RichText::new(scope.label()).weak()).truncate());
                });
            };
            let amend = &mut self.amend;
            if narrow {
                ui.add_enabled(has_log, egui::Checkbox::new(amend, "Amend last commit"));
                ui.horizontal(|ui| commit(ui, actions));
            } else {
                ui.horizontal(|ui| {
                    ui.add_enabled(has_log, egui::Checkbox::new(amend, "Amend last commit"));
                    commit(ui, actions);
                });
            }
            if self.amend
                && !had_amend
                && self.commit_msg.trim().is_empty()
                && let Some(repo) = &self.repo
            {
                self.commit_msg = repo.last_commit_message().unwrap_or_default();
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let hint = if self.draft.is_some() { "Claude is writing…" } else { "Commit message" };
            let edit = ui.add_enabled_ui(self.draft.is_none(), |ui| {
                ui.add_sized(ui.available_size(), egui::TextEdit::multiline(&mut self.commit_msg).hint_text(hint))
            });
            let edit = edit.inner;
            if edit.has_focus() && can_commit && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
                actions.push(Action::Commit);
            }
        });
    }

    fn journal_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Journal, RichText::new("Journal").strong());
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Output, RichText::new("Output").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match self.bottom_tab {
                BottomTab::Output => {
                    if ui.small_button("Clear").clicked() {
                        self.output.clear();
                    }
                }
                BottomTab::Journal => {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.journal_filter)
                            .hint_text("🔍 Message, author, SHA or ref")
                            .desired_width(240.0),
                    );
                    // Only where it fits beside the tabs.
                    if self.commit_view.is_none() && !self.log.is_empty() && ui.available_width() > 420.0 {
                        ui.weak("Click a commit to see what it changed");
                    }
                }
            });
        });
        ui.separator();
        match self.bottom_tab {
            BottomTab::Journal if self.log.is_empty() => {
                art_note(ui, art::seed(), "No commits yet", "Stage some files and make the first one");
            }
            BottomTab::Journal => self.journal_table(ui, actions),
            BottomTab::Output => {
                egui::ScrollArea::vertical().auto_shrink(false).stick_to_bottom(true).show(ui, |ui| {
                    for line in &self.output {
                        let text = RichText::new(&line.text).monospace();
                        ui.label(if line.error { text.color(ui.visuals().error_fg_color) } else { text });
                    }
                });
            }
        }
    }

    fn journal_table(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // No gap between rows, so the graph's lanes run unbroken from row to row.
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + 10.0;
        ui.style_mut().interaction.selectable_labels = false;
        ui.spacing_mut().item_spacing.y = 0.0;
        let rows = self.journal_rows();
        // Lanes only mean something over the whole history, so a filter hides them.
        let show_graph = rows.len() == self.log.len();
        let graph_w = if show_graph { graph::width(&self.graph) } else { 14.0 };
        let shown = self.commit_view.as_ref().and_then(CommitView::id);
        let p = Palette::of(ui.visuals());
        TableBuilder::new(ui)
            .id_salt("journal")
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .auto_shrink(false)
            .min_scrolled_height(0.0)
            .max_scroll_height(f32::INFINITY)
            .column(Column::exact(graph_w))
            .column(Column::remainder().at_least(200.0).clip(true))
            .column(Column::initial(160.0).clip(true))
            .column(Column::initial(130.0))
            .column(Column::initial(80.0))
            .header(row_h, |mut h| {
                h.col(|_| {});
                for t in ["Message", "Author", "Date", "SHA"] {
                    h.col(|ui| {
                        ui.strong(t);
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, rows.len(), |mut row| {
                    let i = rows[row.index()];
                    let c = &self.log[i];
                    row.set_selected(shown == Some(c.id.as_str()));
                    let is_head = c.refs.iter().any(|r| r == "HEAD" || r.starts_with("HEAD -> "));
                    row.col(|ui| {
                        let rect = ui.max_rect();
                        if show_graph {
                            graph::paint(ui, rect, &self.graph[i], c.unpushed, is_head);
                        } else if c.unpushed {
                            ui.painter().circle_filled(rect.center(), 3.5, p.ill_accent);
                        }
                        let hover = ui.allocate_rect(rect, Sense::hover());
                        if c.unpushed {
                            hover.on_hover_text("Not pushed yet");
                        }
                    });
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            for r in &c.refs {
                                ref_badge(ui, r);
                            }
                            let text = RichText::new(&c.summary);
                            ui.label(if c.unpushed { text.strong() } else { text });
                        });
                    });
                    row.col(|ui| {
                        ui.label(&c.author).on_hover_text(&c.email);
                    });
                    row.col(|ui| {
                        ui.weak(&c.date);
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(&c.short_id).monospace()).on_hover_text(&c.id);
                    });
                    let resp = row.response();
                    if resp.clicked() && shown != Some(c.id.as_str()) {
                        actions.push(Action::ShowCommit(c.id.clone()));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Show changes").clicked() {
                            actions.push(Action::ShowCommit(c.id.clone()));
                        }
                        ui.separator();
                        if ui.button("Check out this commit (detached)").clicked() {
                            actions.push(Action::RequestCheckout(CheckoutTarget::Commit(c.id.clone())));
                        }
                        if ui.button("Create branch here…").clicked() {
                            let start =
                                StartPoint { label: format!("{} {}", c.short_id, c.summary), rev: c.id.clone() };
                            actions.push(Action::ShowNewBranch(Some(start)));
                        }
                        ui.separator();
                        if ui.button("Copy SHA").clicked() {
                            actions.push(Action::Copy(c.id.clone()));
                        }
                        if ui.button("Copy message").clicked() {
                            actions.push(Action::Copy(c.summary.clone()));
                        }
                    });
                });
            });
    }

    // ----- Commit view ---------------------------------------------------------------

    fn update_commit_diff(&mut self) {
        let Some(view) = &mut self.commit_view else { return };
        let compact = self.settings.compact;
        let Some(file) = view.selected_file().cloned() else {
            view.diff = None;
            view.diff_key = None;
            return;
        };
        let key = (file.path.clone(), compact);
        if view.diff_key.as_ref() == Some(&key) {
            return;
        }
        let (Some(repo), Ok(details)) = (&self.repo, &view.details) else { return };
        let parent = details.parents.first().map(String::as_str);
        view.diff = Some(
            repo.commit_file_diff(&details.id, parent, &file)
                .map(|d| DiffView::new(d, compact))
                .map_err(|e| e.to_string()),
        );
        view.diff_key = Some(key);
    }

    /// The commit view: its files, its details and the diff of the picked file, laid out
    /// like the working tree it replaces.
    fn commit_view_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        self.update_commit_diff();
        let Some(view) = &mut self.commit_view else { return };
        let p = Palette::of(ui.visuals());
        let details = match &view.details {
            Ok(d) => d,
            Err(e) => {
                if glyph_button(ui, Glyph::Back, "Working tree", true).clicked() {
                    actions.push(Action::CloseCommit);
                }
                centered_note(ui, e);
                return;
            }
        };
        let frame = theme::pane(p);

        egui::Panel::bottom("commit_changes").resizable(true).default_size(380.0).frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                pane_title(ui, "Changes");
                diff_title(ui, view.diff.as_ref());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    diff_controls(ui, &mut self.settings);
                });
            });
            ui.separator();
            let empty = if details.files.is_empty() { "This commit changes no files" } else { "Select a file" };
            let parent = if details.parents.is_empty() { "(nothing)" } else { "Parent" };
            diff_body(ui, view.diff.as_ref(), (parent, "This commit"), self.settings.side_by_side, empty);
        });

        let details_max = (ui.available_width() * 0.5).max(260.0);
        egui::Panel::right("commit_details")
            .resizable(true)
            .default_size(420.0)
            .max_size(details_max)
            .frame(frame)
            .show(ui, |ui| commit_details_ui(ui, details, &self.log, actions));

        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                if glyph_button(ui, Glyph::Back, "Working tree", true).on_hover_text("Back (Esc)").clicked() {
                    actions.push(Action::CloseCommit);
                }
                ui.add_space(4.0);
                Glyph::Commit.drawing().show(ui, egui::vec2(18.0, 18.0));
                ui.label(RichText::new(&details.short_id).monospace().strong());
                ui.label(RichText::new(details.summary()).strong());
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                pane_title(ui, "Files");
                let n = details.files.len();
                ui.weak(format!("{n} changed"));
            });
            ui.separator();
            if details.files.is_empty() {
                art_note(ui, art::clean(), "", "This commit changes no files");
                return;
            }
            let row_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
            ui.style_mut().interaction.selectable_labels = false;
            let mut clicked = None;
            TableBuilder::new(ui)
                .id_salt("commit_files")
                .striped(true)
                .resizable(true)
                .sense(Sense::click())
                .auto_shrink(false)
                .min_scrolled_height(0.0)
                .max_scroll_height(f32::INFINITY)
                .column(Column::initial(260.0).at_least(80.0).clip(true))
                .column(Column::initial(110.0).at_least(60.0).clip(true))
                .column(Column::remainder().clip(true))
                .header(row_h, |mut h| {
                    for t in ["Name", "State", "Relative Directory"] {
                        h.col(|ui| {
                            ui.strong(t);
                        });
                    }
                })
                .body(|body| {
                    body.rows(row_h, details.files.len(), |mut row| {
                        let f = &details.files[row.index()];
                        row.set_selected(view.selected.as_ref() == Some(&f.path));
                        row.col(|ui| {
                            ui.label(f.file_name());
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(f.change.label()).color(change_color(f.change, p)));
                        });
                        row.col(|ui| {
                            let dir = match &f.orig_path {
                                Some(orig) => format!("{}  (from {orig})", f.directory()),
                                None => f.directory().to_owned(),
                            };
                            ui.weak(dir);
                        });
                        let resp = row.response();
                        if resp.clicked() {
                            clicked = Some(f.path.clone());
                        }
                        resp.context_menu(|ui| {
                            if let Some(repo) = &self.repo
                                && ui.button("Reveal in Finder").clicked()
                            {
                                actions.push(Action::Reveal(repo.root().join(&f.path)));
                            }
                            if ui.button("Copy path").clicked() {
                                actions.push(Action::Copy(f.path.clone()));
                            }
                        });
                    });
                });
            if clicked.is_some() {
                view.selected = clicked;
            }
        });
    }
}

/// Author, committer, parents, refs and the whole message of a commit.
fn commit_details_ui(ui: &mut egui::Ui, d: &CommitDetails, log: &[Commit], actions: &mut Vec<Action>) {
    let p = Palette::of(ui.visuals());
    pane_title(ui, "Commit");
    ui.separator();
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        if !d.refs.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for r in &d.refs {
                    ref_badge(ui, r);
                }
            });
            ui.add_space(4.0);
        }
        ui.label(RichText::new(d.summary()).size(16.0).strong());
        let body = d.body();
        if !body.is_empty() {
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(body).color(p.fg)).selectable(true));
        }
        ui.add_space(12.0);
        egui::Grid::new("commit_facts").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
            let key = |ui: &mut egui::Ui, k: &str| {
                ui.label(RichText::new(k).color(p.fg_dim));
            };
            key(ui, "Author");
            ui.label(format!("{} <{}>", d.author, d.email));
            ui.end_row();
            key(ui, "Date");
            ui.label(&d.date);
            ui.end_row();
            if (&d.committer, &d.committer_email) != (&d.author, &d.email) || d.committer_date != d.date {
                key(ui, "Committer");
                ui.label(format!("{} <{}>", d.committer, d.committer_email));
                ui.end_row();
                key(ui, "Committed");
                ui.label(&d.committer_date);
                ui.end_row();
            }
            key(ui, "SHA");
            ui.horizontal(|ui| {
                ui.label(RichText::new(&d.id).monospace().small());
                if ui.small_button("Copy").clicked() {
                    actions.push(Action::Copy(d.id.clone()));
                }
            });
            ui.end_row();
            key(ui, if d.parents.len() > 1 { "Parents" } else { "Parent" });
            ui.vertical(|ui| {
                if d.parents.is_empty() {
                    ui.weak("none, this is the first commit");
                }
                for parent in &d.parents {
                    let summary = log.iter().find(|c| &c.id == parent).map(|c| c.summary.as_str()).unwrap_or("");
                    let short = &parent[..parent.len().min(7)];
                    let link = ui.link(RichText::new(short).monospace()).on_hover_text(summary);
                    if link.clicked() {
                        actions.push(Action::ShowCommit(parent.clone()));
                    }
                }
            });
            ui.end_row();
        });
    });
}

fn change_color(change: snout_core::Change, p: &Palette) -> Color32 {
    use snout_core::Change;
    match change {
        Change::Added | Change::Copied => p.ok,
        Change::Deleted => p.error,
        Change::Renamed => p.warn,
        _ => p.fg,
    }
}

impl eframe::App for SnoutApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_job();
        self.poll_draft();
        self.auto_refresh(&ctx);
        self.survey(&ctx);
        let mut actions = Vec::new();
        // Before any widget runs: an Edit item hands egui a key or clipboard event.
        self.native_menu(&ctx, &mut actions);
        self.shortcuts(&ctx, &mut actions);

        let p = Palette::of(&ctx.global_style().visuals);
        let pane = theme::pane(p);
        let bar = egui::Frame::new().fill(p.ground).inner_margin(egui::Margin::symmetric(8, 6));
        if self.draw_menu {
            egui::Panel::top("menu")
                .frame(bar.inner_margin(egui::Margin::symmetric(8, 2)))
                .show_separator_line(false)
                .show(ui, |ui| self.menu_bar_ui(ui, &mut actions));
        }
        egui::Panel::top("toolbar").frame(bar).show_separator_line(false).show(ui, |ui| {
            self.toolbar(ui, &mut actions);
        });
        egui::Panel::bottom("statusbar")
            .frame(bar.inner_margin(egui::Margin::symmetric(10, 4)))
            .show_separator_line(false)
            .show(ui, |ui| self.status_bar(ui));
        if self.settings.console_open {
            let root = self.repo.as_ref().map(|r| r.root().to_owned());
            // On the ground like the other panes; the card alone would leave its margin
            // unpainted, which shows as a black edge.
            // Never so wide that the panes in the middle overlap: they need about 600 points
            // beside the left column.
            let max = (ui.available_width() - 860.0).max(360.0);
            let event = egui::Panel::right("console")
                .resizable(true)
                .default_size(680.0)
                .min_size(360.0)
                .max_size(max)
                .frame(egui::Frame::new().fill(p.ground).inner_margin(egui::Margin {
                    left: 0,
                    right: 5,
                    top: 0,
                    bottom: 0,
                }))
                .show_separator_line(false)
                .show(ui, |ui| pane.show(ui, |ui| self.console.ui(ui, root.as_deref())).inner)
                .inner;
            match event {
                Some(ConsoleEvent::Activity) if self.job.is_none() => self.refresh(),
                Some(ConsoleEvent::Close) => self.settings.console_open = false,
                _ => {}
            }
        }
        let ground = egui::Frame::new().fill(p.ground).inner_margin(egui::Margin::symmetric(5, 0));
        egui::Panel::left("left").resizable(true).default_size(260.0).frame(ground).show_separator_line(false).show(
            ui,
            |ui| {
                egui::Panel::bottom("branches")
                    .resizable(true)
                    .default_size(360.0)
                    .frame(pane)
                    .show_separator_line(false)
                    .show(ui, |ui| self.branches_ui(ui, &mut actions));
                egui::CentralPanel::default().frame(pane).show(ui, |ui| self.repos_ui(ui, &mut actions));
            },
        );
        egui::CentralPanel::default().frame(ground).show(ui, |ui| {
            if self.repo.is_none() {
                egui::CentralPanel::default().frame(pane).show(ui, |ui| self.welcome_ui(ui, &mut actions));
                return;
            }
            egui::Panel::bottom("journal")
                .resizable(true)
                .default_size(190.0)
                .frame(pane)
                .show_separator_line(false)
                .show(ui, |ui| self.journal_ui(ui, &mut actions));
            if self.commit_view.is_some() {
                self.commit_view_ui(ui, &mut actions);
                return;
            }
            egui::Panel::bottom("changes")
                .resizable(true)
                .default_size(340.0)
                .frame(pane)
                .show_separator_line(false)
                .show(ui, |ui| self.changes_ui(ui));
            // Capped so the file list keeps room when the console narrows the window.
            let commit_max = (ui.available_width() * 0.45).max(220.0);
            egui::Panel::right("commit")
                .resizable(true)
                .default_size(380.0)
                .max_size(commit_max)
                .frame(pane)
                .show_separator_line(false)
                .show(ui, |ui| self.commit_ui(ui, &mut actions));
            egui::CentralPanel::default().frame(pane).show(ui, |ui| self.files_ui(ui, &mut actions));
        });

        self.dialogs(&ctx, &mut actions);
        self.drag_and_drop(&ctx, &mut actions);
        for action in actions {
            self.handle(action, &ctx);
        }
        self.update_title(&ctx);
        self.persist_repos(frame);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.persisted = Persisted::of(&self.settings);
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }
}

// ----- Helpers -------------------------------------------------------------------------

/// System fonts used as fallbacks, if present: the symbols Claude Code draws (✻ ⎿ ✓ …)
/// are not in egui's bundled fonts.
const SYSTEM_FALLBACKS: &[(&str, &str)] = &[
    ("apple-symbols", "/System/Library/Fonts/Apple Symbols.ttf"),
    ("menlo", "/System/Library/Fonts/Menlo.ttc"),
    ("dejavu-mono", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
    ("segoe-symbol", "C:\\Windows\\Fonts\\seguisym.ttf"),
];

/// The proportional font lacks arrows (↑ ↓ →); let it fall back to the monospace font,
/// then to the system fallbacks.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mono = fonts.families.get(&egui::FontFamily::Monospace).and_then(|f| f.first()).cloned();
    if let Some(mono) = mono {
        fonts.families.entry(egui::FontFamily::Proportional).or_default().push(mono);
    }
    for (name, path) in SYSTEM_FALLBACKS {
        let Ok(bytes) = std::fs::read(path) else { continue };
        fonts.font_data.insert((*name).to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().push((*name).to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

fn reveal_items(ui: &mut egui::Ui, path: &Path, actions: &mut Vec<Action>) {
    if ui.button("Reveal in Finder").clicked() {
        actions.push(Action::Reveal(path.to_owned()));
    }
    if ui.button("Copy path").clicked() {
        actions.push(Action::Copy(path.display().to_string()));
    }
}

/// "SnoutCode" with its mark beside it and a green stroke under it, after the website's lockup.
fn wordmark(ui: &mut egui::Ui, p: &Palette) {
    let galley = ui.painter().layout_no_wrap("SnoutCode".into(), egui::FontId::proportional(40.0), p.fg);
    let fly = egui::vec2(52.0, 44.0);
    let size = egui::vec2(fly.x + 10.0 + galley.size().x, galley.size().y.max(fly.y) + 8.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let fly_rect = egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.center().y - fly.y / 2.0 - 4.0), fly);
    art::mark().paint(ui.painter(), fly_rect, None);
    let text_pos = egui::pos2(fly_rect.max.x + 10.0, rect.center().y - galley.size().y / 2.0 - 4.0);
    let text_rect = egui::Rect::from_min_size(text_pos, galley.size());
    ui.painter().galley(text_pos, galley, p.fg);
    let y = text_rect.bottom() + 1.0;
    let (x0, x1) = (text_rect.left() + 2.0, text_rect.right() - 2.0);
    let swash = egui::epaint::CubicBezierShape::from_points_stroke(
        [
            egui::pos2(x0, y + 2.0),
            egui::pos2(x0 + 20.0, y - 2.0),
            egui::pos2(x1 - 30.0, y - 3.0),
            egui::pos2(x1, y - 1.0),
        ],
        false,
        Color32::TRANSPARENT,
        egui::Stroke::new(4.0, p.ill_accent),
    );
    ui.painter().add(swash);
}

fn pane_title(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).strong());
}

fn centered_note(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.weak(text);
    });
}

/// A drawing above a short message, centred: how an empty pane says what goes there.
fn art_note(ui: &mut egui::Ui, drawing: &art::Drawing, title: &str, text: &str) {
    let h = ui.available_height();
    // Fill the pane: a panel takes its content's size, and would shrink to the note.
    ui.set_min_size(ui.available_size());
    let art_h = (h * 0.55).clamp(0.0, 150.0);
    ui.vertical_centered(|ui| {
        ui.add_space(((h - art_h - 48.0) * 0.4).max(0.0));
        if art_h >= 40.0 {
            drawing.show(ui, egui::vec2(art_h * 1.5, art_h));
        }
        if !title.is_empty() {
            ui.label(RichText::new(title).strong());
        }
        ui.weak(text);
    });
}

/// Path and +/− counts of the diff showing, for a pane header.
fn diff_title(ui: &mut egui::Ui, diff: Option<&Result<DiffView, String>>) {
    let Some(Ok(view)) = diff else { return };
    let d = &view.diff;
    let p = Palette::of(ui.visuals());
    ui.label(RichText::new(&d.path).monospace());
    if !d.binary && !d.too_large {
        ui.label(RichText::new(format!("+{}", d.additions)).color(p.ok));
        ui.label(RichText::new(format!("−{}", d.deletions)).color(p.error));
    }
}

/// Layout toggles shared by every diff pane (right to left).
fn diff_controls(ui: &mut egui::Ui, settings: &mut Settings) {
    ui.toggle_value(&mut settings.compact, "Compact").on_hover_text("Hide unchanged lines away from changes");
    ui.separator();
    ui.selectable_value(&mut settings.side_by_side, true, "Side by Side");
    ui.selectable_value(&mut settings.side_by_side, false, "Unified");
}

/// The diff itself, or a note saying why there is none.
fn diff_body(
    ui: &mut egui::Ui,
    diff: Option<&Result<DiffView, String>>,
    (left, right): (&str, &str),
    side_by_side: bool,
    empty: &str,
) {
    match diff {
        None => art_note(ui, art::pick(), "", empty),
        Some(Err(e)) => centered_note(ui, e),
        Some(Ok(view)) => {
            let d = &view.diff;
            if d.binary {
                centered_note(ui, "Binary file differs");
            } else if d.too_large {
                centered_note(ui, "File too large to display");
            } else if d.is_identical() {
                centered_note(ui, &format!("No differences between {left} and {right}"));
            } else {
                if side_by_side {
                    ui.columns(2, |cols| {
                        cols[0].weak(left);
                        cols[1].weak(right);
                    });
                }
                view.show(ui, side_by_side);
            }
        }
    }
}

fn open_menu_items(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    if ui.button("Open local repository…   ⌘O").clicked() {
        actions.push(Action::PickFolder);
    }
    if ui.button("Find repositories in folder…").clicked() {
        actions.push(Action::PickScanFolder);
    }
    if ui.button("Clone from GitHub…").clicked() {
        actions.push(Action::ShowClone);
    }
}

fn state_color(f: &FileEntry, v: &egui::Visuals) -> Color32 {
    let p = Palette::of(v);
    if f.conflicted {
        p.error
    } else if f.is_untracked() {
        p.fg_dim
    } else if f.has_staged() && !f.has_unstaged() {
        p.ok
    } else if f.has_staged() {
        p.warn
    } else {
        p.accent
    }
}

/// A selectable row with a fixed-width marker column, so names line up.
fn marked_row(ui: &mut egui::Ui, marker: &str, text: RichText, hover: String) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_sized([12.0, ui.spacing().interact_size.y], egui::Label::new(marker));
        ui.selectable_label(false, text).on_hover_text(hover)
    })
    .inner
}

fn branch_row(ui: &mut egui::Ui, b: &Branch, actions: &mut Vec<Action>) {
    let mut label = match b.kind {
        BranchKind::Local => b.name.clone(),
        BranchKind::Remote => b.local_name().to_owned(),
    };
    if b.ahead > 0 {
        label += &format!("  ↑{}", b.ahead);
    }
    if b.behind > 0 {
        label += &format!("  ↓{}", b.behind);
    }
    if b.upstream_gone {
        label += "  (gone)";
    }
    let text = if b.is_head { RichText::new(label).strong() } else { RichText::new(label) };
    let hover = format!(
        "{}{}\n{} {}\n\nDouble-click to check out",
        b.name,
        b.upstream.as_ref().map(|u| format!(" → {u}")).unwrap_or_default(),
        b.target,
        b.subject
    );
    // A trash can shows at the end of the row under the pointer.
    let row =
        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), ui.spacing().interact_size.y));
    let hovered = ui.rect_contains_pointer(row);
    let (resp, trash) = ui
        .horizontal(|ui| {
            ui.add_sized([12.0, ui.spacing().interact_size.y], egui::Label::new(if b.is_head { "●" } else { "" }));
            let resp = ui.selectable_label(false, text).on_hover_text(hover);
            let trash = !b.is_head
                && hovered
                && ui
                    .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tip = match b.kind {
                            BranchKind::Local => "Delete branch…".to_owned(),
                            BranchKind::Remote => format!("Delete from {}…", b.remote.as_deref().unwrap_or("remote")),
                        };
                        art::icon_button(ui, Glyph::Trash, true).on_hover_text(tip).clicked()
                    })
                    .inner;
            (resp, trash)
        })
        .inner;
    if trash {
        actions.push(Action::AskDeleteBranch(b.clone()));
    }
    if resp.double_clicked() && !b.is_head {
        actions.push(Action::RequestCheckout(b.into()));
    }
    resp.context_menu(|ui| {
        let checkout_label = match b.kind {
            BranchKind::Local => "Check out".to_owned(),
            BranchKind::Remote => format!("Check out as local “{}”", b.local_name()),
        };
        if ui.add_enabled(!b.is_head, egui::Button::new(checkout_label)).clicked() {
            actions.push(Action::RequestCheckout(b.into()));
        }
        if ui.button("Create branch from here…").clicked() {
            actions.push(Action::ShowNewBranch(Some(StartPoint { label: b.name.clone(), rev: b.name.clone() })));
        }
        let delete_label = match b.kind {
            BranchKind::Local => "Delete…".to_owned(),
            BranchKind::Remote => format!("Delete from {}…", b.remote.as_deref().unwrap_or("remote")),
        };
        if ui.add_enabled(!b.is_head, egui::Button::new(delete_label)).clicked() {
            actions.push(Action::AskDeleteBranch(b.clone()));
        }
        ui.separator();
        if ui.button("Copy name").clicked() {
            actions.push(Action::Copy(b.name.clone()));
        }
    });
}

fn tag_row(ui: &mut egui::Ui, t: &Tag, actions: &mut Vec<Action>) {
    let hover = format!("{} {}\n{}\n\nDouble-click to check out (detached)", t.target, t.subject, t.date);
    let resp = marked_row(ui, "", RichText::new(&t.name), hover);
    let target = CheckoutTarget::Tag(t.name.clone());
    if resp.double_clicked() {
        actions.push(Action::RequestCheckout(target.clone()));
    }
    resp.context_menu(|ui| {
        if ui.button("Check out (detached)").clicked() {
            actions.push(Action::RequestCheckout(target.clone()));
        }
        if ui.button("Create branch from tag…").clicked() {
            let rev = format!("refs/tags/{}", t.name);
            actions.push(Action::ShowNewBranch(Some(StartPoint { label: format!("tag {}", t.name), rev })));
        }
        ui.separator();
        if ui.button("Copy name").clicked() {
            actions.push(Action::Copy(t.name.clone()));
        }
    });
}

fn ref_badge(ui: &mut egui::Ui, r: &str) {
    let p = Palette::of(ui.visuals());
    let (text, color) = if let Some(branch) = r.strip_prefix("HEAD -> ") {
        (branch, p.ok)
    } else if let Some(tag) = r.strip_prefix("tag: ") {
        (tag, p.warn)
    } else if r == "HEAD" {
        (r, p.ok)
    } else {
        (r, p.fg_dim)
    };
    egui::Frame::new()
        .fill(p.wash(color))
        .stroke(egui::Stroke::new(1.0, theme::mix(color, 0.45, p.panel)))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(6, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}
