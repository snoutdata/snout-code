//! What the Tests window shows of the open project: a Playwright project's tests as
//! `playwright test --list` finds them, and the packages that test with Vitest or Jest
//! ([`crate::jstest`]); the run in front (the newest test session, live while it runs), the
//! runs kept before it, and what the window has picked (a test, a filter, a tab, the suite
//! runs go to). Runs start through [`crate::run::Runs`]; this follows them.

use std::collections::HashSet;
use std::path::PathBuf;

use gpui::{App, Context, Entity, EventEmitter, Global, Subscription, Task};

use super::history::{History, Record, cell_key};
use super::report::{Row, Run, State};
use super::run::{Options, Scope};
use crate::lang::node;
use crate::model::Model;
use crate::run::{Runs, RunsEvent};

/// Which part of the window shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Live,
    History,
}

/// Which tests the grid lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Failed,
    Flaky,
    /// Over [`SLOW_MS`].
    Slow,
}

/// What counts as slow.
pub const SLOW_MS: u64 = 4000;

/// A run that took longer than this says so when it ends, wherever the window is.
pub const LONG_MS: u64 = 60_000;

pub struct PwTests {
    model: Entity<Model>,
    runs: Entity<Runs>,
    /// The open project, when it has tests: it is a Playwright one, or packages of it test
    /// with Vitest or Jest.
    pub root: Option<PathBuf>,
    /// The open project, whether it has tests or not.
    project: Option<PathBuf>,
    /// The project is a Playwright one.
    pub playwright: bool,
    /// Its packages that test with Vitest or Jest.
    pub suites: Vec<crate::jstest::Suite>,
    /// The suite runs go to, as picked (by its folder); none for Playwright's, or the one
    /// the run in front ran.
    pub picked: Option<PathBuf>,
    suites_task: Option<Task<()>>,
    /// The tests as listed (nothing run).
    pub listing: Option<Run>,
    pub listing_error: Option<String>,
    pub listing_now: bool,
    pub history: History,
    /// The session in front: the newest Playwright run.
    pub live: Option<u64>,
    /// Sessions already kept in the history.
    kept: HashSet<u64>,
    /// The test picked: its row's key, and the project of the cell.
    pub selected: Option<(String, String)>,
    pub filter: Filter,
    pub text: String,
    pub tab: Tab,
    /// Watch: runs show the browsers, one test at a time.
    pub watch: bool,
    /// What the last long run said when it ended.
    pub ended_note: Option<String>,
    listing_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<()> for PwTests {}

pub struct PwTestsHandle(pub Entity<PwTests>);
impl Global for PwTestsHandle {}

pub fn tests(cx: &App) -> Option<Entity<PwTests>> {
    cx.try_global::<PwTestsHandle>().map(|h| h.0.clone())
}

impl PwTests {
    pub fn new(model: Entity<Model>, runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        let subs = vec![
            cx.observe(&model, |this, _, cx| this.project_changed(cx)),
            cx.observe(&runs, |this, _, cx| this.runs_changed(cx)),
            cx.subscribe(&runs, |this, _, e: &RunsEvent, cx| {
                if let RunsEvent::Ended(id) = e {
                    this.ended(*id, cx);
                }
            }),
        ];
        let mut this = PwTests {
            model,
            runs,
            root: None,
            project: None,
            playwright: false,
            suites: Vec::new(),
            picked: None,
            suites_task: None,
            listing: None,
            listing_error: None,
            listing_now: false,
            history: History::default(),
            live: None,
            kept: HashSet::new(),
            selected: None,
            filter: Filter::All,
            text: String::new(),
            tab: Tab::Live,
            watch: false,
            ended_note: None,
            listing_task: None,
            _subs: subs,
        };
        this.project_changed(cx);
        this
    }

    fn project_changed(&mut self, cx: &mut Context<Self>) {
        let host = crate::host::host(cx);
        let project = self.model.read(cx).project_root();
        if project == self.project {
            return;
        }
        self.project = project.clone();
        self.playwright = project.as_deref().is_some_and(|r| super::is_root(&*host, r));
        let root = project.clone().filter(|_| self.playwright);
        self.root = root.clone();
        self.suites.clear();
        self.picked = None;
        self.listing = None;
        self.listing_error = None;
        self.live = None;
        self.selected = None;
        let support = self.model.read(cx).support_dir();
        self.history =
            project.as_deref().map_or_else(History::default, |r| History::load(History::file(support.as_deref(), r)));
        if root.is_some() {
            self.refresh(cx);
        }
        self.find_suites(cx);
        cx.notify();
    }

    /// Look for the packages that test with Vitest or Jest, off the UI thread.
    pub fn find_suites(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project.clone() else { return };
        let host = crate::host::host(cx);
        let scan = cx.background_executor().spawn(async move { crate::jstest::suites(&*host, &project) });
        self.suites_task = Some(cx.spawn(async move |this, cx| {
            let found = scan.await;
            let _ = this.update(cx, |this, cx| {
                if this.suites != found {
                    this.suites = found;
                    if this.root.is_none() && !this.suites.is_empty() {
                        this.root = this.project.clone();
                    }
                    cx.notify();
                }
            });
        }));
    }

    /// The suite runs go to, when not Playwright's: the one picked, else the one the run in
    /// front ran, else (in a project without Playwright) the first.
    pub fn unit(&self, cx: &App) -> Option<crate::jstest::Suite> {
        let by_dir = |d: &PathBuf| self.suites.iter().find(|s| &s.dir == d).cloned();
        if let Some(d) = &self.picked {
            return by_dir(d);
        }
        if let Some(run) = self.run(cx) {
            return match (&run.dir, run.runner) {
                (Some(d), Some(runner)) => by_dir(d).or(Some(crate::jstest::Suite { dir: d.clone(), runner })),
                _ if self.playwright => None,
                _ => self.suites.first().cloned(),
            };
        }
        if self.playwright { None } else { self.suites.first().cloned() }
    }

    /// Run in this suite from now on (none: Playwright's).
    pub fn pick_suite(&mut self, dir: Option<PathBuf>, cx: &mut Context<Self>) {
        self.picked = dir;
        cx.notify();
    }

    /// List the project's tests again (a Playwright project's).
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone().filter(|_| self.playwright) else { return };
        let host = crate::host::host(cx);
        let env: Vec<(String, String)> = self
            .runs
            .read(cx)
            .environment()
            .cloned()
            .map(|e| e.variables(|f| host.read_to_string(&root.join(f)).ok()))
            .unwrap_or_default();
        self.listing_now = true;
        let task = cx.background_executor().spawn(async move {
            if super::run::cli(&*host, &root).is_none() {
                return Err(String::new());
            }
            let wanted = node::wanted(&*host, &root, Some(&root));
            let all = node::discover(&*host);
            let n = node::choose(&wanted, &all).cloned().ok_or_else(String::new)?;
            super::run::list(&*host, &root, &n, &env)
        });
        self.listing_task = Some(cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                this.listing_now = false;
                match found {
                    Ok(run) => {
                        let projects = run.shown_projects();
                        this.runs.update(cx, |r, cx| r.set_playwright_configs(&projects, cx));
                        this.listing = Some(run);
                        this.listing_error = None;
                    }
                    // Not installed, or no Node: the setup bar says so.
                    Err(why) if why.is_empty() => this.listing_error = None,
                    Err(why) => this.listing_error = Some(why),
                }
                cx.notify();
            });
        }));
    }

    fn runs_changed(&mut self, cx: &mut Context<Self>) {
        let newest = self.runs.read(cx).sessions.iter().rev().find(|s| s.playwright.is_some()).map(|s| s.id);
        if newest != self.live {
            self.live = newest;
        }
        cx.notify();
    }

    /// The run in front, live or the last.
    pub fn run<'a>(&self, cx: &'a App) -> Option<&'a Run> {
        let id = self.live?;
        self.runs.read(cx).session(id)?.playwright.as_ref()
    }

    /// Whether the run in front still runs.
    pub fn running(&self, cx: &App) -> bool {
        self.live.and_then(|id| self.runs.read(cx).session(id)).is_some_and(|s| s.running())
    }

    /// How long the run in front has taken.
    pub fn elapsed_ms(&self, cx: &App) -> u64 {
        self.live.and_then(|id| self.runs.read(cx).session(id)).map_or(0, |s| s.elapsed().as_millis() as u64)
    }

    /// The grid's rows: the listed tests, with those the run found that the listing lacks.
    pub fn rows(&self, cx: &App) -> Vec<Row> {
        let mut rows = self.listing.as_ref().map(Run::rows).unwrap_or_default();
        if let Some(run) = self.run(cx) {
            for r in run.rows() {
                match rows.iter_mut().find(|o| o.key == r.key) {
                    Some(o) => o.cells.extend(r.cells),
                    None => rows.push(r),
                }
            }
        }
        rows
    }

    /// The projects the grid has a column for.
    pub fn projects(&self, cx: &App) -> Vec<String> {
        let mut out = self.listing.as_ref().map(Run::shown_projects).unwrap_or_default();
        for p in self.run(cx).map(Run::shown_projects).unwrap_or_default() {
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }

    /// How a row's test came out in `project`, in the run in front (none: it was not in it).
    pub fn cell(&self, row: &Row, project: &str, cx: &App) -> Option<(State, Option<u64>, String)> {
        let run = self.run(cx)?;
        let t = run.tests.values().find(|t| t.project == project && t.key() == row.key)?;
        Some((t.state(run.over), t.millis(), t.id.clone()))
    }

    /// Whether a row passes the filter.
    pub fn shows(&self, row: &Row, cx: &App) -> bool {
        let text = self.text.trim().to_lowercase();
        if !text.is_empty() && !row.key.to_lowercase().contains(&text) {
            return false;
        }
        let projects = self.projects(cx);
        let cells: Vec<(State, Option<u64>)> =
            projects.iter().filter_map(|p| self.cell(row, p, cx)).map(|(s, ms, _)| (s, ms)).collect();
        match self.filter {
            Filter::All => true,
            Filter::Failed => cells.iter().any(|(s, _)| matches!(s, State::Failed | State::Interrupted)),
            Filter::Flaky => {
                cells.iter().any(|(s, _)| *s == State::Flaky)
                    || projects.iter().any(|p| self.history.flaky_rate(&cell_key(p, &row.key)).is_some())
            }
            Filter::Slow => cells.iter().any(|(_, ms)| ms.is_some_and(|m| m >= SLOW_MS)),
        }
    }

    /// The test picked, as the run in front has it.
    pub fn selected_test<'a>(&self, cx: &'a App) -> Option<&'a super::report::Test> {
        let (key, project) = self.selected.as_ref()?;
        self.run(cx)?.tests.values().find(|t| &t.project == project && &t.key() == key)
    }

    pub fn select(&mut self, key: String, project: String, cx: &mut Context<Self>) {
        self.selected = Some((key, project));
        cx.notify();
    }

    pub fn set_filter(&mut self, f: Filter, cx: &mut Context<Self>) {
        self.filter = f;
        cx.notify();
    }

    pub fn set_tab(&mut self, t: Tab, cx: &mut Context<Self>) {
        self.tab = t;
        cx.notify();
    }

    pub fn toggle_watch(&mut self, cx: &mut Context<Self>) {
        self.watch = !self.watch;
        cx.notify();
    }

    fn options(&self) -> Options {
        Options { headed: self.watch, ..Default::default() }
    }

    /// Run `scope`, headed when Watch is on; in a suite of Vitest or Jest when that is where
    /// runs go.
    pub fn start(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.tab = Tab::Live;
        self.ended_note = None;
        if let Some(suite) = self.unit(cx) {
            let scope = self.unit_scope(scope, &suite, cx);
            self.runs.update(cx, |r, cx| r.start_js_tests(suite.dir, suite.runner, scope, false, cx));
            return;
        }
        let options = self.options();
        self.runs.update(cx, |r, cx| r.start_playwright(scope, options, cx));
    }

    /// A Playwright scope as a unit runner takes it: the tests that failed in the run in
    /// front, a file, a test at a line.
    fn unit_scope(&self, scope: Scope, suite: &crate::jstest::Suite, cx: &App) -> crate::jstest::Scope {
        use crate::jstest::Scope as J;
        let run = self.run(cx).filter(|r| r.dir.as_ref() == Some(&suite.dir));
        let abs = |run: &Run, t: &super::report::Test| run.location(&t.id).map(|(f, _)| f);
        match scope {
            Scope::LastFailed => match run {
                Some(run) => {
                    J::Tests(run.failures().into_iter().filter_map(|t| Some((abs(run, t)?, t.path.clone()))).collect())
                }
                None => J::All,
            },
            Scope::At { file, line: Some(line), .. } => {
                let found = run.and_then(|run| {
                    run.tests
                        .values()
                        .find(|t| t.line == line && abs(run, t).as_ref() == Some(&file))
                        .map(|t| t.path.clone())
                });
                match found {
                    Some(path) => J::At { file, path, group: false },
                    None => J::File(file),
                }
            }
            Scope::At { file, line: None, .. } => J::File(file),
            Scope::All | Scope::Project(_) => J::All,
        }
    }

    /// The test picked as its unit runner runs it: its suite, and its scope.
    fn selected_unit(&self, cx: &App) -> Option<(crate::jstest::Suite, crate::jstest::Scope)> {
        let run = self.run(cx)?;
        let (dir, runner) = (run.dir.clone()?, run.runner?);
        let t = self.selected_test(cx)?;
        let (file, _) = run.location(&t.id)?;
        Some((
            crate::jstest::Suite { dir, runner },
            crate::jstest::Scope::At { file, path: t.path.clone(), group: false },
        ))
    }

    /// Run the test picked, in its project.
    pub fn run_selected(&mut self, cx: &mut Context<Self>) {
        if let Some((suite, scope)) = self.selected_unit(cx) {
            self.runs.update(cx, |r, cx| r.start_js_tests(suite.dir, suite.runner, scope, false, cx));
            return;
        }
        let Some(t) = self.selected_test(cx).cloned() else { return };
        let Some((file, line)) = self.run(cx).and_then(|r| r.location(&t.id)) else { return };
        self.start(Scope::At { file, line: Some(line), project: Some(t.project.clone()) }, cx);
    }

    /// Debug the test picked: it stops at the breakpoints in its code.
    pub fn debug_selected(&mut self, cx: &mut Context<Self>) {
        if let Some((suite, scope)) = self.selected_unit(cx) {
            self.runs.update(cx, |r, cx| r.start_js_tests(suite.dir, suite.runner, scope, true, cx));
            return;
        }
        self.start_selected(Options { breakpoints: true, ..Default::default() }, cx);
    }

    /// Whether the run in front is a unit runner's (no browsers, traces or Inspector).
    pub fn unit_run(&self, cx: &App) -> bool {
        self.run(cx).is_some_and(|r| r.runner.is_some())
    }

    /// Step through the test picked in the Playwright Inspector (`--debug`).
    pub fn inspect_selected(&mut self, cx: &mut Context<Self>) {
        self.start_selected(Options { inspector: true, ..Default::default() }, cx);
    }

    fn start_selected(&mut self, options: Options, cx: &mut Context<Self>) {
        let Some(t) = self.selected_test(cx).cloned() else { return };
        let Some((file, line)) = self.run(cx).and_then(|r| r.location(&t.id)) else { return };
        let scope = Scope::At { file, line: Some(line), project: Some(t.project.clone()) };
        self.runs.update(cx, |r, cx| r.start_playwright(scope, options, cx));
    }

    /// Take the screenshots of the test picked as they are now (`--update-snapshots`).
    pub fn accept_screenshots(&mut self, cx: &mut Context<Self>) {
        let Some(t) = self.selected_test(cx).cloned() else { return };
        let Some((file, line)) = self.run(cx).and_then(|r| r.location(&t.id)) else { return };
        let options = Options { update_snapshots: true, ..Default::default() };
        let scope = Scope::At { file, line: Some(line), project: Some(t.project.clone()) };
        self.runs.update(cx, |r, cx| r.start_playwright(scope, options, cx));
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.live {
            self.runs.update(cx, |r, cx| r.stop(id, cx));
        }
    }

    /// Session `id` ended: keep it, and say how a long one went.
    fn ended(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.kept.contains(&id) {
            return;
        }
        let runs = self.runs.read(cx);
        let Some(s) = runs.session(id) else { return };
        let Some(run) = s.playwright.as_ref().filter(|r| r.begun) else { return };
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            .saturating_sub(s.elapsed().as_secs());
        let elapsed = s.elapsed().as_millis() as u64;
        let model = self.model.read(cx);
        let git = (model.status.branch.head.clone(), model.status.branch.oid.clone());
        let mut record = Record::of(run, started, s.title.clone(), run.environment.clone(), git);
        if record.millis == 0 {
            record.millis = elapsed;
        }
        // A run of every test lists them all: the listing follows it.
        let full = matches!(&s.launch, crate::run::Launch::Process(p) if covers_all(&p.args));
        let listing = full.then(|| {
            let mut l = run.clone();
            l.tests.values_mut().for_each(|t| t.tries.clear());
            l
        });
        let before = self.history.runs.last().cloned();
        let fresh = record.new_failures(before.as_ref()).len();
        let failed = record.failed;
        let note = t!(
            "tests-passed-failed",
            title = (s.title).to_string(),
            passed = (record.passed).to_string(),
            failed = (record.failed).to_string(),
            new = (if record.flaky > 0 {
                t!("tests-flaky-3", flaky = (record.flaky).to_string())
            } else {
                String::new()
            })
            .to_string(),
            new_2 =
                (if fresh > 0 { t!("tests-new-since-the-last-run", fresh = fresh.to_string()) } else { String::new() })
                    .to_string()
        );
        self.kept.insert(id);
        self.history.add(record);
        if let Some(l) = listing.filter(|l| !l.tests.is_empty()) {
            self.listing = Some(l);
        }
        if elapsed >= LONG_MS {
            self.ended_note = Some(note.clone());
            self.model.update(cx, |m, cx| {
                m.message = note;
                cx.notify();
            });
            crate::platform::attention(&if failed > 0 { failed.to_string() } else { "✓".to_owned() });
        }
        cx.notify();
    }
}

/// Whether a run's arguments run every test: no file, project, grep or last-failed filter.
fn covers_all(args: &[String]) -> bool {
    args.iter().skip(2).all(|a| {
        a.starts_with("--")
            && !["--project", "--grep", "--last-failed", "--only-changed"].iter().any(|f| a.starts_with(f))
    })
}
