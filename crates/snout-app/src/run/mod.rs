//! Running, testing and debugging, through the engine's `snout/` methods: the run
//! configurations, each run as a session (its console, its Gradle tasks, its test results,
//! its debugger), the Gradle tasks of the project, the breakpoints, and the Scala compile
//! that follows a saved `.scala` file.
//!
//! [`Runs`] keeps all of it for the window; the Run, Debug, Build and Gradle tool windows
//! show it. [`Marks`] is the small part the editors draw (breakpoints and where the
//! debugger stopped), apart so the editors do not repaint on every console line.

pub mod ansi;
pub mod config;
pub mod config_dialog;
pub mod debug;
pub mod gradle_window;
pub mod test_tree;
pub mod windows;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Task};
use serde_json::{Value, json};

use crate::lang::{HubEvent, LangHub};
use crate::model::Model;
use config::{Breakpoint, Kind, ProjectSettings, RunConfig};
use test_tree::TestTree;

/// The tool window a session shows in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Run,
    Debug,
    Build,
}

/// What a session runs, so it can run again.
#[derive(Clone, Debug, PartialEq)]
pub enum Launch {
    Config { config: RunConfig, debug: bool },
    Tests { module: String, filter: Value, label: String, debug: bool, rerun_failed: bool },
    Task { build: Option<String>, task: String },
}

impl Launch {
    pub fn debug(&self) -> bool {
        matches!(self, Launch::Config { debug: true, .. } | Launch::Tests { debug: true, .. })
    }

    pub fn title(&self) -> String {
        match self {
            Launch::Config { config, .. } => config.name.clone(),
            Launch::Tests { label, rerun_failed: true, .. } => format!("{label} (failed)"),
            Launch::Tests { label, .. } => label.clone(),
            Launch::Task { task, .. } => task.clone(),
        }
    }

    pub fn place(&self) -> Place {
        match self {
            Launch::Task { .. } => Place::Build,
            l if l.debug() => Place::Debug,
            _ => Place::Run,
        }
    }

    /// The module whose tests this runs, for Rerun Failed.
    pub fn test_module(&self) -> Option<&str> {
        match self {
            Launch::Tests { module, .. } => Some(module),
            Launch::Config { config, .. } if config.kind == Kind::Tests => Some(&config.module),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
    /// What the app itself says (the command, the outcome).
    System,
}

#[derive(Clone, Debug)]
pub struct ConsoleLine {
    pub stream: Stream,
    pub spans: Vec<ansi::Span>,
}

impl ConsoleLine {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            stream: Stream::System,
            spans: vec![ansi::Span { text: text.into(), style: Default::default(), link: None }],
        }
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

/// A Gradle task of a run, as the Build window's tree shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRow {
    pub path: String,
    /// None while it runs.
    pub outcome: Option<String>,
    pub millis: Option<u64>,
    pub failure: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub outcome: Outcome,
    pub elapsed: Duration,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
}

/// The most lines a console keeps; older ones go.
const MAX_LINES: usize = 50_000;

pub struct Session {
    pub id: u64,
    pub run_id: Option<String>,
    pub title: String,
    pub launch: Launch,
    pub lines: Vec<ConsoleLine>,
    style: ansi::Style,
    pub started: Instant,
    pub ended: Option<Ended>,
    pub tasks: Vec<TaskRow>,
    pub tests: Option<TestTree>,
    pub selected_test: Option<String>,
    pub collapsed: HashSet<String>,
    /// Errors and warnings the run's compile reported.
    pub problems: (usize, usize),
    pub debug: Option<debug::Debugger>,
    /// Lines dropped from the start of the console.
    pub dropped: usize,
}

impl Session {
    pub fn place(&self) -> Place {
        self.launch.place()
    }

    pub fn running(&self) -> bool {
        self.ended.is_none()
    }

    pub fn elapsed(&self) -> Duration {
        self.ended.as_ref().map_or_else(|| self.started.elapsed(), |e| e.elapsed)
    }

    pub fn push(&mut self, stream: Stream, line: &str) {
        let spans = ansi::parse(line, &mut self.style);
        self.lines.push(ConsoleLine { stream, spans });
        if self.lines.len() > MAX_LINES {
            let cut = self.lines.len() - MAX_LINES;
            self.lines.drain(..cut);
            self.dropped += cut;
        }
    }

    /// "Running 0:12", "Finished in 3.2 s", for the window's tab.
    pub fn status(&self) -> String {
        let t = self.elapsed();
        let time = if t < Duration::from_secs(60) {
            format!("{:.1} s", t.as_secs_f32())
        } else {
            format!("{}:{:02}", t.as_secs() / 60, t.as_secs() % 60)
        };
        match &self.ended {
            None if self.debug.as_ref().is_some_and(|d| d.paused) => format!("Paused, running {time}"),
            None => format!("Running {time}"),
            Some(e) => match e.outcome {
                Outcome::Success => format!("Finished in {time}"),
                Outcome::Failed => format!("Failed after {time}"),
                Outcome::Cancelled => format!("Stopped after {time}"),
            },
        }
    }

    /// The tests' counts, passed, failed and skipped, as a line.
    pub fn test_summary(&self) -> Option<String> {
        let t = self.tests.as_ref()?;
        let (passed, failed, skipped, running) = t.counts();
        let total = passed + failed + skipped + running;
        let mut parts = Vec::new();
        if failed > 0 {
            parts.push(format!("{failed} failed"));
        }
        parts.push(format!("{passed} passed"));
        if skipped > 0 {
            parts.push(format!("{skipped} skipped"));
        }
        Some(format!("{}, {total} {}", parts.join(", "), if total == 1 { "test" } else { "tests" }))
    }
}

/// The project's Gradle tasks, as `snout/gradleTasks` lists them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradleBuild {
    pub build: String,
    pub root_dir: PathBuf,
    pub error: Option<String>,
    pub projects: Vec<GradleProject>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradleProject {
    pub path: String,
    pub name: String,
    pub module: Option<String>,
    pub groups: Vec<(String, Vec<GradleTask>)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradleTask {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
}

pub fn parse_gradle_tasks(v: &Value) -> Vec<GradleBuild> {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let arr = |v: &Value, k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    arr(v, "builds")
        .iter()
        .map(|b| GradleBuild {
            build: s(b, "build").unwrap_or_default(),
            root_dir: s(b, "rootDir").map(PathBuf::from).unwrap_or_default(),
            error: s(b, "error").or_else(|| arr(b, "errors").first().and_then(Value::as_str).map(str::to_owned)),
            projects: arr(b, "projects")
                .iter()
                .map(|p| GradleProject {
                    path: s(p, "project").unwrap_or_else(|| ":".into()),
                    name: s(p, "name").unwrap_or_default(),
                    module: s(p, "module"),
                    groups: arr(p, "groups")
                        .iter()
                        .map(|g| {
                            let tasks = arr(g, "tasks")
                                .iter()
                                .map(|t| GradleTask {
                                    name: s(t, "name").unwrap_or_default(),
                                    path: s(t, "path").unwrap_or_default(),
                                    description: s(t, "description").filter(|d| !d.is_empty()),
                                })
                                .collect();
                            (s(g, "name").unwrap_or_else(|| "other".into()), tasks)
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub struct GradleTasks {
    pub builds: Vec<GradleBuild>,
    pub loading: bool,
    pub error: Option<String>,
    /// The engine process they were read from.
    pub from: Option<u32>,
}

/// What a file holds to run: its test classes and methods, and its main classes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileRunnables {
    pub module: Option<String>,
    pub items: Vec<Runnable>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Runnable {
    /// The line, from 0, the gutter icon sits on.
    pub line: u32,
    pub target: Target,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    TestClass { class: String },
    TestMethod { class: String, method: String },
    Main { class: String },
}

impl Target {
    pub fn label(&self) -> String {
        let simple = |c: &str| c.rsplit(['.', '$']).next().unwrap_or(c).to_owned();
        match self {
            Target::TestClass { class } | Target::Main { class } => simple(class),
            Target::TestMethod { class, method } => format!("{}.{method}", simple(class)),
        }
    }
}

/// What the editors draw for debugging: the breakpoints, and the line the debugger
/// stopped on with the values it sees there.
#[derive(Default)]
pub struct Marks {
    pub breakpoints: BTreeMap<PathBuf, Vec<(u32, Option<String>)>>,
    /// The file and line (from 0) the selected frame is at, while paused.
    pub exec: Option<(PathBuf, u32)>,
    /// The selected frame's local variables, name and value.
    pub locals: Vec<(String, String)>,
}

pub struct MarksHandle(pub Entity<Marks>);

impl Global for MarksHandle {}

pub fn marks(cx: &App) -> Option<Entity<Marks>> {
    cx.try_global::<MarksHandle>().map(|h| h.0.clone())
}

/// What the run windows ask of the workspace.
#[derive(Clone, Debug)]
pub enum RunsEvent {
    /// Bring this tool window forward.
    Show(Place),
    /// Open the place a console line links to.
    OpenLink(ansi::Link),
    /// Show this file at this line (from 0), where the debugger stopped.
    OpenAt(PathBuf, u32),
    /// Show the Edit Configurations dialog.
    EditConfigurations,
    /// Show the Problems tool window.
    ShowProblems,
}

pub struct Runs {
    model: Entity<Model>,
    pub hub: Entity<LangHub>,
    pub marks: Entity<Marks>,
    /// The build's configurations, and the engine process they came from.
    pub engine_configs: Vec<RunConfig>,
    configs_from: Option<u32>,
    pub configs_loading: bool,
    /// The open project, and what is kept for it.
    pub project_root: Option<PathBuf>,
    pub project: ProjectSettings,
    project_file: Option<PathBuf>,
    pub sessions: Vec<Session>,
    /// The session each tool window shows.
    pub selected: HashMap<Place, u64>,
    next_id: u64,
    /// Engine notifications for runs whose id is not known yet.
    orphans: HashMap<String, Vec<(String, Value)>>,
    pub gradle: GradleTasks,
    /// A Scala compile is running (since then).
    pub scala: Option<Instant>,
    pub scala_error: Option<String>,
    _tick: Option<Task<()>>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl EventEmitter<RunsEvent> for Runs {}

pub struct RunsHandle(pub Entity<Runs>);

impl Global for RunsHandle {}

pub fn runs(cx: &App) -> Option<Entity<Runs>> {
    cx.try_global::<RunsHandle>().map(|h| h.0.clone())
}

impl Runs {
    pub fn new(model: Entity<Model>, hub: Entity<LangHub>, cx: &mut Context<Self>) -> Self {
        let marks = cx.new(|_| Marks::default());
        let subscriptions = vec![
            cx.subscribe(&hub, |this, _, e: &HubEvent, cx| match e {
                HubEvent::Engine { method, params, .. } => this.on_engine(method, params.clone(), cx),
                HubEvent::Changed => this.engine_changed(cx),
                _ => {}
            }),
            cx.observe(&model, |this, _, cx| this.project_changed(cx)),
        ];
        let mut this = Self {
            model,
            hub,
            marks,
            engine_configs: Vec::new(),
            configs_from: None,
            configs_loading: false,
            project_root: None,
            project: ProjectSettings::default(),
            project_file: None,
            sessions: Vec::new(),
            selected: HashMap::new(),
            next_id: 1,
            orphans: HashMap::new(),
            gradle: GradleTasks::default(),
            scala: None,
            scala_error: None,
            _tick: None,
            _subscriptions: subscriptions,
        };
        this.project_changed(cx);
        this
    }

    // ----- The project and the engine -----------------------------------------------------

    fn project_changed(&mut self, cx: &mut Context<Self>) {
        let root = self.model.read(cx).project_root();
        if root == self.project_root {
            return;
        }
        self.project_root = root.clone();
        self.project_file = match (&root, self.model.read(cx).support_dir()) {
            (Some(root), Some(dir)) => Some(config::project_file(&dir, root)),
            _ => None,
        };
        self.project = self.project_file.as_deref().map(ProjectSettings::load).unwrap_or_default();
        self.engine_configs.clear();
        self.configs_from = None;
        self.gradle = GradleTasks::default();
        self.sync_marks(cx);
        cx.notify();
    }

    /// Keep the project's settings (no file in tests).
    fn save_project(&self) {
        if let Some(file) = &self.project_file
            && let Err(e) = self.project.save(file)
        {
            eprintln!("[run] could not save {}: {e}", file.display());
        }
    }

    /// The engine serving the open project, when it answers requests.
    pub fn engine(&self, cx: &App) -> Option<(PathBuf, u32)> {
        let project = self.model.read(cx).project_root()?;
        let hub = self.hub.read(cx);
        let root = hub.engine_root(&project)?;
        hub.client(&root).map(|c| (root, c.pid()))
    }

    /// The project's engine has read the build and answers requests.
    pub fn engine_ready(&self, cx: &App) -> bool {
        let Some(project) = self.model.read(cx).project_root() else { return false };
        let hub = self.hub.read(cx);
        hub.engine_root(&project)
            .and_then(|r| hub.servers.get(&r))
            .is_some_and(|s| s.stage == crate::lang::hub::Stage::Ready)
    }

    /// Ask the project's engine; fails when none is ready.
    pub fn request(
        &self,
        method: &str,
        params: Value,
        cx: &App,
    ) -> impl std::future::Future<Output = anyhow::Result<Value>> + 'static + use<> {
        let root = self.engine(cx).map(|(r, _)| r);
        let call = root.map(|r| self.hub.read(cx).request(&r, method, params));
        async move {
            match call {
                Some(call) => call.await,
                None => Err(anyhow::anyhow!("The Java engine is not ready yet")),
            }
        }
    }

    fn say(&self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        self.model.update(cx, |m, cx| {
            m.message = text;
            cx.notify();
        });
    }

    /// The engine started or changed: read its configurations and tasks once per process.
    fn engine_changed(&mut self, cx: &mut Context<Self>) {
        let Some((_, pid)) = self.engine(cx) else { return };
        if !self.engine_ready(cx) {
            return;
        }
        if self.configs_from != Some(pid) && !self.configs_loading {
            self.load_configurations(cx);
        }
        if self.gradle.from != Some(pid) && !self.gradle.loading {
            self.load_gradle_tasks(false, cx);
        }
    }

    pub fn load_configurations(&mut self, cx: &mut Context<Self>) {
        let Some((_, pid)) = self.engine(cx) else { return };
        self.configs_loading = true;
        let call = self.request("snout/runConfigurations", json!({}), cx);
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| {
                this.configs_loading = false;
                match result {
                    Ok(v) => {
                        this.configs_from = Some(pid);
                        this.engine_configs = v
                            .get("configurations")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(RunConfig::from_engine)
                            .collect();
                        eprintln!("[run] {} run configurations", this.engine_configs.len());
                    }
                    Err(e) => eprintln!("[run] run configurations: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Read the Gradle tasks; `refresh` makes the engine configure the builds again.
    pub fn load_gradle_tasks(&mut self, refresh: bool, cx: &mut Context<Self>) {
        let Some((_, pid)) = self.engine(cx) else {
            self.gradle.error = Some("The Java engine is not ready yet".into());
            cx.notify();
            return;
        };
        self.gradle.loading = true;
        self.gradle.error = None;
        let call = self.request("snout/gradleTasks", json!({ "refresh": refresh }), cx);
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| {
                this.gradle.loading = false;
                this.gradle.from = Some(pid);
                match result {
                    Ok(v) => this.gradle.builds = parse_gradle_tasks(&v),
                    Err(e) => this.gradle.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    // ----- Configurations -----------------------------------------------------------------

    /// Every configuration: the build's (as the user changed them), then the user's own.
    pub fn configurations(&self) -> Vec<RunConfig> {
        config::merged(&self.engine_configs, &self.project.configurations)
    }

    /// The configuration the title bar shows; none for Current File.
    pub fn selected_config(&self) -> Option<RunConfig> {
        let id = self.project.selected.as_ref()?;
        self.configurations().into_iter().find(|c| &c.id == id)
    }

    pub fn select_config(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.project.selected = id;
        self.save_project();
        cx.notify();
    }

    /// Put the user's configurations in place (the Edit Configurations dialog).
    pub fn set_user_configurations(&mut self, list: Vec<RunConfig>, selected: Option<String>, cx: &mut Context<Self>) {
        self.project.configurations = list;
        if selected.is_some() {
            self.project.selected = selected;
        }
        self.save_project();
        cx.notify();
    }

    // ----- Sessions -----------------------------------------------------------------------

    pub fn session(&self, id: u64) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn session_mut(&mut self, id: u64) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// The session `place`'s window shows.
    pub fn shown(&self, place: Place) -> Option<&Session> {
        let id = self.selected.get(&place)?;
        self.session(*id).filter(|s| s.place() == place)
    }

    pub fn sessions_in(&self, place: Place) -> impl Iterator<Item = &Session> {
        self.sessions.iter().filter(move |s| s.place() == place)
    }

    /// A Run or Debug session is running.
    pub fn any_running(&self) -> bool {
        self.sessions.iter().any(|s| s.running() && s.place() != Place::Build)
    }

    pub fn select(&mut self, place: Place, id: u64, cx: &mut Context<Self>) {
        self.selected.insert(place, id);
        if place == Place::Debug {
            self.sync_marks(cx);
        }
        cx.notify();
    }

    /// Close a session's tab, stopping it first.
    pub fn close(&mut self, id: u64, cx: &mut Context<Self>) {
        self.stop(id, cx);
        let Some(at) = self.sessions.iter().position(|s| s.id == id) else { return };
        let place = self.sessions[at].place();
        self.sessions.remove(at);
        if self.selected.get(&place) == Some(&id) {
            match self.sessions_in(place).last().map(|s| s.id) {
                Some(next) => {
                    self.selected.insert(place, next);
                }
                None => {
                    self.selected.remove(&place);
                }
            }
        }
        self.sync_marks(cx);
        cx.notify();
    }

    /// Run `launch` in a new session (a finished one of the same title gives up its tab).
    pub fn start(&mut self, launch: Launch, cx: &mut Context<Self>) -> Option<u64> {
        if self.engine(cx).is_none() {
            self.say("Runs start once the Java engine is ready", cx);
            return None;
        }
        let place = launch.place();
        let title = launch.title();
        self.sessions.retain(|s| !(s.place() == place && s.title == title && !s.running()));
        let id = self.next_id;
        self.next_id += 1;
        let (method, params) = match &launch {
            Launch::Config { config, debug } => ("snout/run", json!({ "config": config.to_engine(), "debug": debug })),
            Launch::Tests { module, filter, debug, rerun_failed, .. } => {
                let mut p = json!({ "module": module, "debug": debug });
                if *rerun_failed {
                    p["rerunFailed"] = json!(true);
                } else if !filter.is_null() {
                    p["filter"] = filter.clone();
                }
                ("snout/runTests", p)
            }
            Launch::Task { build, task } => {
                let mut p = json!({ "task": task });
                if let Some(b) = build {
                    p["build"] = json!(b);
                }
                ("snout/runTask", p)
            }
        };
        let mut session = Session {
            id,
            run_id: None,
            title,
            launch: launch.clone(),
            lines: Vec::new(),
            style: Default::default(),
            started: Instant::now(),
            ended: None,
            tasks: Vec::new(),
            tests: None,
            selected_test: None,
            collapsed: HashSet::new(),
            problems: (0, 0),
            debug: launch.debug().then(debug::Debugger::default),
            dropped: 0,
        };
        let what = match &launch {
            Launch::Config { config, .. } => match (config.kind, &config.main_class, &config.task) {
                (Kind::Main, Some(c), _) => format!("Starting {c}"),
                (_, _, Some(t)) => format!("Running {t}"),
                _ => format!("Running {}", config.name),
            },
            Launch::Tests { label, .. } => format!("Testing {label}"),
            Launch::Task { task, .. } => format!("Running {task}"),
        };
        session.lines.push(ConsoleLine::system(what));
        if matches!(&launch, Launch::Tests { .. })
            || matches!(&launch, Launch::Config { config, .. } if config.kind == Kind::Tests)
        {
            session.tests = Some(TestTree::default());
        }
        self.sessions.push(session);
        self.selected.insert(place, id);
        let call = self.request(method, params, cx);
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| this.started(id, result, cx));
        })
        .detach();
        self.tick(cx);
        cx.emit(RunsEvent::Show(place));
        cx.notify();
        Some(id)
    }

    /// A session for a run started elsewhere, under `run_id`: the engine's notifications for
    /// it land in it (tests and screenshots feed them with [`Self::engine_notification`]).
    pub fn adopt(&mut self, launch: Launch, run_id: &str, cx: &mut Context<Self>) -> u64 {
        let place = launch.place();
        let id = self.next_id;
        self.next_id += 1;
        let mut s = Session {
            id,
            run_id: Some(run_id.to_owned()),
            title: launch.title(),
            launch: launch.clone(),
            lines: Vec::new(),
            style: Default::default(),
            started: Instant::now(),
            ended: None,
            tasks: Vec::new(),
            tests: None,
            selected_test: None,
            collapsed: HashSet::new(),
            problems: (0, 0),
            debug: launch.debug().then(debug::Debugger::default),
            dropped: 0,
        };
        s.lines.push(ConsoleLine::system(format!("Running {}", launch.title())));
        if matches!(launch, Launch::Tests { .. }) {
            s.tests = Some(TestTree::default());
        }
        self.sessions.push(s);
        self.selected.insert(place, id);
        cx.notify();
        id
    }

    /// Take one of the engine's notifications, as the hub hands them on.
    pub fn engine_notification(&mut self, method: &str, params: Value, cx: &mut Context<Self>) {
        self.on_engine(method, params, cx);
    }

    /// The engine answered the request that starts session `id`.
    fn started(&mut self, id: u64, result: anyhow::Result<Value>, cx: &mut Context<Self>) {
        let Some(s) = self.session_mut(id) else { return };
        let v = match result {
            Ok(v) => v,
            Err(e) => {
                let text = format!("{e:#}");
                s.push(Stream::Err, &text);
                s.ended = Some(Ended {
                    outcome: Outcome::Failed,
                    elapsed: s.started.elapsed(),
                    exit_code: None,
                    error: Some(text),
                });
                cx.notify();
                return;
            }
        };
        let run_id = v.get("runId").and_then(Value::as_str).map(str::to_owned);
        s.run_id = run_id.clone();
        if let Some(tasks) = v.get("tasks").and_then(Value::as_array) {
            let names: Vec<&str> = tasks.iter().filter_map(Value::as_str).collect();
            if !names.is_empty() {
                s.push(Stream::System, &format!("Gradle: {}", names.join(" ")));
            }
        }
        // A main class debugged answers once its JVM listens: attach now.
        if let Some(d) = v.get("debug")
            && d.get("listening").and_then(Value::as_bool) == Some(true)
            && let (Some(port), Some(attach)) = (d.get("dapPort").and_then(Value::as_u64), d.get("attach").cloned())
        {
            self.attach(id, port as u16, attach, cx);
        }
        if let Some(run_id) = run_id
            && let Some(waiting) = self.orphans.remove(&run_id)
        {
            for (method, params) in waiting {
                self.on_engine(&method, params, cx);
            }
        }
        cx.notify();
    }

    /// Stop session `id`: the engine cancels the Gradle run or ends the process.
    pub fn stop(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(s) = self.session_mut(id) else { return };
        if !s.running() {
            return;
        }
        if let Some(d) = s.debug.as_mut() {
            d.disconnect();
        }
        match s.run_id.clone() {
            Some(run_id) => {
                let call = self.request("snout/cancelTask", json!({ "runId": run_id }), cx);
                cx.background_executor()
                    .spawn(async move {
                        if let Err(e) = call.await {
                            eprintln!("[run] cancel: {e}");
                        }
                    })
                    .detach();
            }
            None => {
                s.ended = Some(Ended {
                    outcome: Outcome::Cancelled,
                    elapsed: s.started.elapsed(),
                    exit_code: None,
                    error: None,
                });
            }
        }
        cx.notify();
    }

    /// Run session `id`'s launch again, stopping it first.
    pub fn rerun(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(launch) = self.session(id).map(|s| s.launch.clone()) else { return };
        self.stop(id, cx);
        self.start(launch, cx);
    }

    /// Run the tests that failed in session `id`'s module.
    pub fn rerun_failed(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(s) = self.session(id) else { return };
        let Some(module) = s.launch.test_module().map(str::to_owned) else { return };
        let label = match &s.launch {
            Launch::Tests { label, .. } => label.clone(),
            other => other.title(),
        };
        let launch = Launch::Tests { module, filter: Value::Null, label, debug: s.launch.debug(), rerun_failed: true };
        self.start(launch, cx);
    }

    /// Repaint once a second while something runs, for the elapsed time.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self._tick.is_some() {
            return;
        }
        self._tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let Ok(running) = this.update(cx, |this, cx| {
                    let running = this.sessions.iter().any(Session::running) || this.scala.is_some();
                    cx.notify();
                    if !running {
                        this._tick = None;
                    }
                    running
                }) else {
                    return;
                };
                if !running {
                    return;
                }
            }
        }));
    }

    // ----- What the engine says -------------------------------------------------------------

    fn on_engine(&mut self, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(run_id) = params.get("runId").and_then(Value::as_str).map(str::to_owned) else { return };
        let Some(at) = self.sessions.iter().position(|s| s.run_id.as_deref() == Some(run_id.as_str())) else {
            if method == "snout/buildProblems" {
                self.take_problems(None, &params, cx);
                return;
            }
            let waiting = self.orphans.entry(run_id).or_default();
            if waiting.len() < 20_000 {
                waiting.push((method.to_owned(), params));
            }
            return;
        };
        let id = self.sessions[at].id;
        let s = &mut self.sessions[at];
        let text = |k: &str| params.get(k).and_then(Value::as_str).map(str::to_owned);
        match method {
            "snout/taskOutput" => {
                let stream = if text("stream").as_deref() == Some("stderr") { Stream::Err } else { Stream::Out };
                s.push(stream, &text("line").unwrap_or_default());
            }
            "snout/taskProgress" => {
                let task = text("task").unwrap_or_default();
                let finished = text("event").as_deref() == Some("finished");
                let row = match s.tasks.iter_mut().find(|t| t.path == task) {
                    Some(row) => row,
                    None => {
                        s.tasks.push(TaskRow { path: task.clone(), outcome: None, millis: None, failure: None });
                        s.tasks.last_mut().expect("pushed")
                    }
                };
                if finished {
                    row.outcome = Some(text("outcome").unwrap_or_else(|| "success".into()));
                    row.millis = params.get("millis").and_then(Value::as_u64);
                    row.failure = text("failure");
                }
            }
            "snout/testEvent" => {
                let tree = s.tests.get_or_insert_with(TestTree::default);
                tree.apply(&params);
            }
            "snout/buildProblems" => {
                self.take_problems(Some(id), &params, cx);
                return;
            }
            "snout/debugReady" => {
                let port = params.get("dapPort").and_then(Value::as_u64);
                let attach = params.get("attach").cloned();
                let fresh = s.debug.as_ref().is_some_and(|d| d.dap.is_none() && !d.ended);
                if let (Some(port), Some(attach), true) = (port, attach, fresh) {
                    self.attach(id, port as u16, attach, cx);
                }
            }
            "snout/taskFinished" => {
                let outcome = match text("outcome").as_deref() {
                    Some("success") => Outcome::Success,
                    Some("cancelled") => Outcome::Cancelled,
                    _ => Outcome::Failed,
                };
                let millis = params.get("millis").and_then(Value::as_u64);
                let exit_code = params.get("exitCode").and_then(Value::as_i64);
                let error = text("error");
                if let Some(e) = &error {
                    s.push(Stream::Err, e);
                }
                let mut summary = match outcome {
                    Outcome::Success => "Finished".to_owned(),
                    Outcome::Failed => "Failed".to_owned(),
                    Outcome::Cancelled => "Stopped".to_owned(),
                };
                if let Some(code) = exit_code {
                    summary.push_str(&format!(" with exit code {code}"));
                }
                s.push(Stream::System, &summary);
                s.ended = Some(Ended {
                    outcome,
                    elapsed: millis.map_or_else(|| s.started.elapsed(), Duration::from_millis),
                    exit_code,
                    error,
                });
                if let Some(tree) = s.tests.as_mut() {
                    tree.settle();
                    if s.selected_test.is_none() {
                        s.selected_test = tree.first_failure();
                    }
                }
                if let Some(d) = s.debug.as_mut() {
                    d.end();
                }
                self.sync_marks(cx);
            }
            _ => {}
        }
        cx.notify();
    }

    /// `snout/buildProblems`: they go to the Problems tool window.
    fn take_problems(&mut self, session: Option<u64>, params: &Value, cx: &mut Context<Self>) {
        let list: Vec<(String, lsp_types::Diagnostic)> = params
            .get("problems")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let file = p.get("file").and_then(Value::as_str)?;
                let line = p.get("line").and_then(Value::as_u64).unwrap_or(1).max(1) as u32 - 1;
                let column = p.get("column").and_then(Value::as_u64).unwrap_or(0).max(1) as u32 - 1;
                let severity = match p.get("severity").and_then(Value::as_str) {
                    Some("warning") => lsp_types::DiagnosticSeverity::WARNING,
                    Some("info") => lsp_types::DiagnosticSeverity::INFORMATION,
                    _ => lsp_types::DiagnosticSeverity::ERROR,
                };
                let at = lsp_types::Position::new(line, column);
                let uri = snout_lang::uri::from_path(Path::new(file));
                Some((
                    uri.as_str().to_owned(),
                    lsp_types::Diagnostic {
                        range: lsp_types::Range::new(at, at),
                        severity: Some(severity),
                        source: Some("build".into()),
                        message: p.get("message").and_then(Value::as_str).unwrap_or_default().to_owned(),
                        ..Default::default()
                    },
                ))
            })
            .collect();
        let errors = list.iter().filter(|(_, d)| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR)).count();
        let warnings = list.len() - errors;
        if let Some(s) = session.and_then(|id| self.session_mut(id)) {
            s.problems = (errors, warnings);
        }
        self.hub.update(cx, |h, cx| h.set_build_problems(list, cx));
        if errors > 0 {
            cx.emit(RunsEvent::ShowProblems);
        }
        cx.notify();
    }

    // ----- Scala ----------------------------------------------------------------------------

    /// A `.scala` file was saved: the engine compiles the Scala again, in the background.
    pub fn scala_changed(&mut self, file: &Path, cx: &mut Context<Self>) {
        if self.engine(cx).is_none() {
            return;
        }
        self.scala = Some(Instant::now());
        self.scala_error = None;
        let call = self.request("snout/scalaChanged", json!({ "files": [file] }), cx);
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| {
                this.scala = None;
                match result {
                    Ok(v) => {
                        let errors: Vec<String> = v
                            .get("builds")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|b| b.get("error").and_then(Value::as_str).map(str::to_owned))
                            .collect();
                        if !errors.is_empty() {
                            this.scala_error = Some(errors.join("; "));
                            this.say("The Scala compile failed: see Problems", cx);
                        }
                    }
                    Err(e) => this.scala_error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        self.tick(cx);
        cx.notify();
    }

    /// What the status bar lists among the background tasks.
    pub fn tasks(&self) -> Vec<crate::background::BgTask> {
        let mut out = Vec::new();
        if self.scala.is_some() {
            out.push(crate::background::BgTask {
                title: "Scala".into(),
                stage: "Compiling the changed Scala".into(),
                percent: None,
                ..Default::default()
            });
        }
        if self.gradle.loading {
            out.push(crate::background::BgTask {
                title: "Gradle".into(),
                stage: "Reading the tasks".into(),
                percent: None,
                ..Default::default()
            });
        }
        for s in self.sessions.iter().filter(|s| s.running() && s.place() == Place::Build) {
            let stage = s
                .tasks
                .iter()
                .rev()
                .find(|t| t.outcome.is_none())
                .map_or_else(|| "Running".to_owned(), |t| t.path.clone());
            out.push(crate::background::BgTask {
                title: format!("Gradle: {}", s.title),
                stage,
                percent: None,
                ..Default::default()
            });
        }
        out
    }

    // ----- Files ------------------------------------------------------------------------------

    /// The test classes and methods of a file, and its main classes, for the gutter.
    pub fn runnables(&self, path: &Path, text: String, cx: &App) -> Task<Option<FileRunnables>> {
        let mains = main_classes(path, &text);
        let call = self.request("snout/testsInFile", json!({ "path": path, "text": text }), cx);
        cx.background_executor().spawn(async move {
            let v = call.await.ok();
            let mut out = FileRunnables {
                module: v.as_ref().and_then(|v| v.get("module")).and_then(Value::as_str).map(str::to_owned),
                items: Vec::new(),
            };
            for c in v.as_ref().and_then(|v| v.get("classes")).and_then(Value::as_array).into_iter().flatten() {
                let Some(class) = c.get("name").and_then(Value::as_str).map(str::to_owned) else { continue };
                let line = |v: &Value| v.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0) as u32;
                out.items.push(Runnable { line: line(c), target: Target::TestClass { class: class.clone() } });
                for m in c.get("methods").and_then(Value::as_array).into_iter().flatten() {
                    let Some(method) = m.get("name").and_then(Value::as_str) else { continue };
                    out.items.push(Runnable {
                        line: line(m),
                        target: Target::TestMethod { class: class.clone(), method: method.to_owned() },
                    });
                }
            }
            for (line, class) in mains {
                if !out.items.iter().any(|r| r.line == line) {
                    out.items.push(Runnable { line, target: Target::Main { class } });
                }
            }
            (v.is_some() || !out.items.is_empty()).then_some(out)
        })
    }

    /// Run (or debug) what a gutter icon stands for.
    pub fn run_target(&mut self, target: &Target, module: Option<&str>, debug: bool, cx: &mut Context<Self>) {
        let launch = match target {
            Target::TestClass { class } | Target::TestMethod { class, .. } => {
                let Some(module) = module else {
                    self.say("The engine does not know this file's module yet", cx);
                    return;
                };
                let filter = match target {
                    Target::TestMethod { method, .. } => json!({ "kind": "method", "class": class, "method": method }),
                    _ => json!({ "kind": "class", "class": class }),
                };
                Launch::Tests { module: module.to_owned(), filter, label: target.label(), debug, rerun_failed: false }
            }
            Target::Main { class } => {
                let known = self.configurations().into_iter().find(|c| c.main_class.as_deref() == Some(class));
                let config = match known {
                    Some(c) => c,
                    None => {
                        let Some(module) = module else {
                            self.say("The engine does not know this file's module yet", cx);
                            return;
                        };
                        RunConfig {
                            id: format!("main:{module}:{class}"),
                            kind: Kind::Main,
                            name: target.label(),
                            module: module.to_owned(),
                            main_class: Some(class.clone()),
                            ..Default::default()
                        }
                    }
                };
                Launch::Config { config, debug }
            }
        };
        self.start(launch, cx);
    }

    // ----- Breakpoints --------------------------------------------------------------------------

    pub fn breakpoints_in(&self, path: &Path) -> Vec<&Breakpoint> {
        self.project.breakpoints.iter().filter(|b| b.path == path).collect()
    }

    /// Set or clear the breakpoint on `line` (from 0) of `path`.
    pub fn toggle_breakpoint(&mut self, path: &Path, line: u32, cx: &mut Context<Self>) {
        let before = self.project.breakpoints.len();
        self.project.breakpoints.retain(|b| !(b.path == path && b.line == line));
        if self.project.breakpoints.len() == before {
            self.project.breakpoints.push(Breakpoint { path: path.to_owned(), line, condition: None });
        }
        self.breakpoints_changed(path, cx);
    }

    /// Give the breakpoint on `line` a condition (none for always), setting it if need be.
    pub fn set_condition(&mut self, path: &Path, line: u32, condition: Option<String>, cx: &mut Context<Self>) {
        let condition = condition.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty());
        match self.project.breakpoints.iter_mut().find(|b| b.path == path && b.line == line) {
            Some(b) => b.condition = condition,
            None => self.project.breakpoints.push(Breakpoint { path: path.to_owned(), line, condition }),
        }
        self.breakpoints_changed(path, cx);
    }

    pub fn remove_breakpoint(&mut self, path: &Path, line: u32, cx: &mut Context<Self>) {
        self.project.breakpoints.retain(|b| !(b.path == path && b.line == line));
        self.breakpoints_changed(path, cx);
    }

    fn breakpoints_changed(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.save_project();
        let list: Vec<Breakpoint> = self.breakpoints_in(path).into_iter().cloned().collect();
        let ids: Vec<u64> =
            self.sessions.iter().filter(|s| s.debug.as_ref().is_some_and(|d| d.live())).map(|s| s.id).collect();
        for id in ids {
            self.send_breakpoints(id, path.to_owned(), list.clone(), cx);
        }
        self.sync_marks(cx);
        cx.notify();
    }

    /// Hand the editors what they draw.
    pub fn sync_marks(&mut self, cx: &mut Context<Self>) {
        let mut breakpoints: BTreeMap<PathBuf, Vec<(u32, Option<String>)>> = BTreeMap::new();
        for b in &self.project.breakpoints {
            breakpoints.entry(b.path.clone()).or_default().push((b.line, b.condition.clone()));
        }
        let (exec, locals) = match self.shown(Place::Debug).and_then(|s| s.debug.as_ref()) {
            Some(d) => (d.exec_point(), d.locals()),
            None => (None, Vec::new()),
        };
        self.marks.update(cx, |m, cx| {
            if m.breakpoints != breakpoints || m.exec != exec || m.locals != locals {
                m.breakpoints = breakpoints;
                m.exec = exec;
                m.locals = locals;
                cx.notify();
            }
        });
    }
}

/// The main classes a file declares, with the line of each (from 0): a class with a
/// `main` method (Java, or a Scala object's `def main`), or one marked
/// `@SpringBootApplication`.
pub fn main_classes(path: &Path, text: &str) -> Vec<(u32, String)> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    if !matches!(ext, "java" | "scala") {
        return Vec::new();
    }
    let package = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("package ").map(|p| p.trim().trim_end_matches(';').trim().to_owned()));
    let qualify = |name: &str| match &package {
        Some(p) if !p.is_empty() => format!("{p}.{name}"),
        _ => name.to_owned(),
    };
    let mut out = Vec::new();
    let mut class_line: Option<(u32, String)> = None;
    let mut spring = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("@SpringBootApplication") {
            spring = true;
        }
        let words: Vec<&str> =
            t.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
        if let Some(k) = words.iter().position(|w| matches!(*w, "class" | "object" | "record" | "enum" | "interface"))
            && let Some(name) = words.get(k + 1)
            && !t.starts_with("//")
            && !t.starts_with('*')
        {
            class_line = Some((i as u32, (*name).to_owned()));
            if spring {
                spring = false;
                out.push((i as u32, qualify(name)));
            }
        }
        let java_main = ext == "java" && t.contains("static") && t.contains("void main(");
        let scala_main = ext == "scala" && t.contains("def main(") && t.contains("Array[String]");
        if (java_main || scala_main)
            && let Some((_, name)) = &class_line
        {
            let class = qualify(name);
            if !out.iter().any(|(_, c)| *c == class) {
                out.push((i as u32, class));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_main_methods_and_spring_boot_applications() {
        let java = "package demo.app;\n\n@SpringBootApplication\npublic class App {\n  public static void main(String[] args) {}\n}\n";
        assert_eq!(main_classes(Path::new("/w/App.java"), java), [(3, "demo.app.App".to_owned())]);
        let plain = "package demo;\nclass Tool {\n    static void main(String... a) {}\n}\n";
        assert_eq!(main_classes(Path::new("/w/Tool.java"), plain), [(2, "demo.Tool".to_owned())]);
        let scala = "package s\nobject Main {\n  def main(args: Array[String]): Unit = ()\n}\n";
        assert_eq!(main_classes(Path::new("/w/Main.scala"), scala), [(2, "s.Main".to_owned())]);
        assert!(main_classes(Path::new("/w/x.kt"), java).is_empty());
    }

    #[test]
    fn reads_the_gradle_tasks() {
        let v = json!({ "builds": [ { "build": "shop", "rootDir": "/w/shop", "millis": 3, "errors": [], "projects": [
            { "project": ":app", "name": "app", "module": "shop:app", "projectDir": "/w/shop/app", "groups": [
                { "name": "build", "tasks": [ { "name": "classes", "path": ":app:classes", "description": "Assembles main classes." } ] }
            ] } ] } ] });
        let builds = parse_gradle_tasks(&v);
        assert_eq!(builds[0].build, "shop");
        assert_eq!(builds[0].projects[0].groups[0].1[0].path, ":app:classes");
        assert_eq!(builds[0].projects[0].module.as_deref(), Some("shop:app"));
    }
}
