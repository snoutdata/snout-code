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
pub mod debug_dialogs;
pub mod gradle_window;
pub mod maven_window;
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
    Config {
        config: RunConfig,
        debug: bool,
    },
    Tests {
        module: String,
        filter: Value,
        label: String,
        debug: bool,
        rerun_failed: bool,
    },
    Task {
        build: Option<String>,
        task: String,
    },
    /// Maven goals (or phases) in a module of a Maven build (`:` for the whole build), with
    /// the Maven window's profiles, offline and skip tests.
    Maven {
        build: String,
        project: String,
        module: String,
        goals: String,
    },
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
            Launch::Maven { module, goals, .. } => format!("{module} [{goals}]"),
        }
    }

    /// It runs a main class as jdtls compiled it (not through Gradle).
    pub fn needs_classes(&self) -> bool {
        matches!(self, Launch::Config { config, .. } if config.kind == Kind::Main)
    }

    pub fn place(&self) -> Place {
        match self {
            Launch::Task { .. } | Launch::Maven { .. } => Place::Build,
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
    /// Asked for before the Java engine was ready: it starts once it is.
    pub waiting: bool,
    /// The program's process, when the engine started it itself (a main class).
    pub pid: Option<u32>,
    /// The engine it was sent to: should that one stop, the run ends with it.
    pub engine: Option<u32>,
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

    /// The console's lines as text.
    pub fn clone_lines(&self) -> Vec<String> {
        self.lines.iter().map(ConsoleLine::text).collect()
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

/// The project's Maven builds, as `snout/mavenGoals` lists them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MavenBuild {
    pub build: String,
    pub root_dir: PathBuf,
    pub version: String,
    pub wrapper: bool,
    pub error: Option<String>,
    pub modules: Vec<MavenModule>,
    /// Every profile of the build: its id, whether the POMs activate it, and where it is.
    pub profiles: Vec<(String, bool, Option<PathBuf>)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MavenModule {
    /// The module path (`:` for the root, `:app`) and the model's module id.
    pub project: String,
    pub module: String,
    pub name: String,
    pub packaging: String,
    pub build_file: Option<PathBuf>,
    pub lifecycle: Vec<String>,
    pub plugins: Vec<MavenPlugin>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MavenPlugin {
    pub artifact: String,
    pub version: Option<String>,
    pub prefix: String,
    /// Each goal, and what it does.
    pub goals: Vec<(String, Option<String>)>,
}

pub fn parse_maven_goals(v: &Value) -> Vec<MavenBuild> {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let arr = |v: &Value, k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    arr(v, "builds")
        .iter()
        .map(|b| MavenBuild {
            build: s(b, "build").unwrap_or_default(),
            root_dir: s(b, "rootDir").map(PathBuf::from).unwrap_or_default(),
            version: s(b, "mavenVersion").unwrap_or_default(),
            wrapper: b.get("wrapper").and_then(Value::as_bool).unwrap_or(false),
            error: s(b, "error"),
            profiles: arr(b, "profiles")
                .iter()
                .filter_map(|p| {
                    Some((
                        s(p, "id")?,
                        p.get("active").and_then(Value::as_bool).unwrap_or(false),
                        s(p, "source").map(PathBuf::from),
                    ))
                })
                .collect(),
            modules: arr(b, "modules")
                .iter()
                .map(|m| MavenModule {
                    project: s(m, "project").unwrap_or_else(|| ":".into()),
                    module: s(m, "module").unwrap_or_default(),
                    name: s(m, "name").unwrap_or_default(),
                    packaging: s(m, "packaging").unwrap_or_else(|| "jar".into()),
                    build_file: s(m, "buildFile").map(PathBuf::from),
                    lifecycle: arr(m, "lifecycle").iter().filter_map(Value::as_str).map(str::to_owned).collect(),
                    plugins: arr(m, "plugins")
                        .iter()
                        .map(|p| MavenPlugin {
                            artifact: s(p, "artifactId").unwrap_or_default(),
                            version: s(p, "version"),
                            prefix: s(p, "prefix").unwrap_or_default(),
                            goals: arr(p, "goals")
                                .iter()
                                .filter_map(|g| Some((s(g, "name")?, s(g, "description").filter(|d| !d.is_empty()))))
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub struct MavenGoals {
    pub builds: Vec<MavenBuild>,
    pub loading: bool,
    pub error: Option<String>,
    pub from: Option<u32>,
}

/// The Maven profiles the open project's engine reads its Maven builds with: the Maven
/// window's choices, kept with the project's settings.
pub fn maven_profiles_of(model: &Entity<Model>, root: &Path, cx: &App) -> Vec<String> {
    match model.read(cx).support_dir() {
        Some(dir) => ProjectSettings::load(&config::project_file(&dir, root)).maven.profiles,
        None => runs(cx).map(|r| r.read(cx).project.maven.profiles.clone()).unwrap_or_default(),
    }
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

    /// What the Run and Debug items call it: `Application.main()`, `GreeterTest`.
    pub fn menu_label(&self) -> String {
        match self {
            Target::Main { .. } => format!("{}.main()", self.label()),
            _ => self.label(),
        }
    }
}

impl FileRunnables {
    /// What runs from the caret's line (from 0), as Run in the editor's menu and ⌃⇧R take
    /// it: the test method the caret is in, else the test class, else the main class.
    pub fn at_line(&self, line: u32) -> Option<&Target> {
        // The nearest item at or above the caret, when it is a test method.
        let nearest = self.items.iter().filter(|r| r.line <= line).max_by_key(|r| r.line);
        nearest
            .filter(|r| matches!(r.target, Target::TestMethod { .. }))
            .map(|r| &r.target)
            .or_else(|| self.items.iter().find(|r| matches!(r.target, Target::TestClass { .. })).map(|r| &r.target))
            .or_else(|| self.items.iter().find(|r| matches!(r.target, Target::Main { .. })).map(|r| &r.target))
    }
}

/// What the editors draw for debugging: the breakpoints, and the line the debugger
/// stopped on with the values it sees there.
#[derive(Default)]
pub struct Marks {
    /// By file: the line (from 0), the condition, and whether it is enabled.
    pub breakpoints: BTreeMap<PathBuf, Vec<(u32, Option<String>, bool)>>,
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

/// Whether runs can start: now, once the engine is ready, or not without the user.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineState {
    Ready,
    /// The engine is on its way (what it is doing).
    Waiting(String),
    /// Why nothing can run.
    Unavailable(String),
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
    pub maven: MavenGoals,
    /// A Scala compile is running (since then).
    pub scala: Option<Instant>,
    pub scala_error: Option<String>,
    /// Run to Cursor: the session, and the line (from 0) of the file it stops at once.
    pub run_to: Option<(u64, PathBuf, u32)>,
    /// What starts once a session ends well (Rebuild Project: the compile after `clean`).
    pub then: HashMap<u64, Launch>,
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
            maven: MavenGoals::default(),
            scala: None,
            scala_error: None,
            run_to: None,
            then: HashMap::new(),
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
        self.maven = MavenGoals::default();
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

    /// The runs sent to an engine that is gone (it stopped, or was stopped) end: nothing will
    /// say how they went any more.
    fn end_orphaned(&mut self, cx: &mut Context<Self>) {
        let now = self.engine(cx).map(|(_, pid)| pid);
        let hub = self.hub.read(cx);
        let alive: Vec<u32> = hub.servers.values().filter_map(|s| s.pid()).collect();
        let mut ended = false;
        for s in self.sessions.iter_mut().filter(|s| s.running() && !s.waiting) {
            let Some(engine) = s.engine else { continue };
            if Some(engine) == now || alive.contains(&engine) {
                continue;
            }
            let text = "The Java engine stopped, and this run with it.";
            s.push(Stream::Err, text);
            if let Some(d) = s.debug.as_mut() {
                d.disconnect();
            }
            s.ended = Some(Ended {
                outcome: Outcome::Failed,
                elapsed: s.started.elapsed(),
                exit_code: None,
                error: Some(text.into()),
            });
            ended = true;
        }
        if ended {
            cx.notify();
        }
    }

    /// The engine started or changed: read its configurations and tasks once per process.
    fn engine_changed(&mut self, cx: &mut Context<Self>) {
        self.end_orphaned(cx);
        self.start_waiting(cx);
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
        if self.maven.from != Some(pid) && !self.maven.loading {
            self.load_maven_goals(false, cx);
        }
    }

    /// Read the Maven builds' modules, goals and profiles.
    pub fn load_maven_goals(&mut self, refresh: bool, cx: &mut Context<Self>) {
        let Some((_, pid)) = self.engine(cx) else {
            self.maven.error = Some("The Java engine is not ready yet".into());
            cx.notify();
            return;
        };
        self.maven.loading = true;
        self.maven.error = None;
        let call = self.request("snout/mavenGoals", json!({ "refresh": refresh }), cx);
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| {
                this.maven.loading = false;
                this.maven.from = Some(pid);
                match result {
                    Ok(v) => this.maven.builds = parse_maven_goals(&v),
                    Err(e) => this.maven.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Turn a Maven profile on or off; the engine reads the builds again with it.
    pub fn toggle_maven_profile(&mut self, id: &str, active: bool, cx: &mut Context<Self>) {
        self.project.maven.toggle(id, active);
        self.save_project();
        if let Some(root) = self.model.read(cx).project_root() {
            self.maven.from = None;
            self.hub.update(cx, |h, cx| h.restart(&root, cx));
        }
        cx.notify();
    }

    pub fn set_maven_offline(&mut self, on: bool, cx: &mut Context<Self>) {
        self.project.maven.offline = on;
        self.save_project();
        cx.notify();
    }

    pub fn set_maven_skip_tests(&mut self, on: bool, cx: &mut Context<Self>) {
        self.project.maven.skip_tests = on;
        self.save_project();
        cx.notify();
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

    /// The configuration the title bar shows; none for Current File. Until one is picked,
    /// the project's Spring Boot application, else its first main class.
    pub fn selected_config(&self) -> Option<RunConfig> {
        let all = self.configurations();
        match self.project.selected.as_deref() {
            Some(config::CURRENT_FILE) => None,
            Some(id) if all.iter().any(|c| c.id == id) => all.into_iter().find(|c| c.id == id),
            _ => config::default_config(&all),
        }
    }

    pub fn select_config(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.project.selected = Some(id.unwrap_or_else(|| config::CURRENT_FILE.to_owned()));
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

    /// Whether runs can start now, wait for the engine, or cannot start (and why). A main
    /// class (`classes`) runs as jdtls compiled it, so it waits for every module to be
    /// ready, Scala compiled; Gradle compiles for its own runs.
    pub fn engine_state(&self, classes: bool, cx: &App) -> EngineState {
        use crate::lang::hub::Stage;
        let Some(project) = self.model.read(cx).project_root() else {
            return EngineState::Unavailable("Open a project to run it.".into());
        };
        let hub = self.hub.read(cx);
        let Some(root) = hub.engine_root(&project) else {
            return if !crate::project_model::project_has_gradle(&project, cx) {
                EngineState::Unavailable(format!(
                    "{} has no Gradle or Maven build: runs, tests and the debugger need one.",
                    Model::dir_name(&project)
                ))
            } else if !hub.enabled {
                EngineState::Unavailable("Java support is off.".into())
            } else {
                EngineState::Waiting("starting".into())
            };
        };
        let Some(server) = hub.servers.get(&root) else { return EngineState::Waiting("starting".into()) };
        match &server.stage {
            Stage::Ready if classes && server.readiness.pending() => {
                EngineState::Waiting(server.readiness.summary().to_lowercase())
            }
            Stage::Ready if hub.client(&root).is_some() => EngineState::Ready,
            Stage::NeedsJdk(v) => EngineState::Unavailable(format!(
                "This build needs JDK {v}, which isn't installed. Download it from the bar above the editor, \
                 or pick a JDK in Settings."
            )),
            Stage::Failed(why) => EngineState::Unavailable(format!("The Java engine is not running: {why}")),
            other => EngineState::Waiting(other.label().to_lowercase()),
        }
    }

    /// Run `launch` in a new session (a finished one of the same title gives up its tab).
    /// Before the engine is ready the session waits for it; when nothing can run, the
    /// session says why. Either way the window shows it.
    pub fn start(&mut self, launch: Launch, cx: &mut Context<Self>) -> Option<u64> {
        let state = self.engine_state(launch.needs_classes(), cx);
        if state == EngineState::Waiting("starting".into())
            && let Some(project) = self.model.read(cx).project_root()
        {
            self.hub.update(cx, |h, cx| h.ensure_server(&project, cx));
        }
        let place = launch.place();
        let title = launch.title();
        self.sessions.retain(|s| !(s.place() == place && s.title == title && !s.running()));
        let id = self.next_id;
        self.next_id += 1;
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
            waiting: false,
            pid: None,
            engine: None,
        };
        let what = match &launch {
            Launch::Config { config, .. } => match (config.kind, &config.main_class, &config.task) {
                (Kind::Main, Some(c), _) => format!("Starting {c}"),
                (_, _, Some(t)) => format!("Running {t}"),
                _ => format!("Running {}", config.name),
            },
            Launch::Tests { label, .. } => format!("Testing {label}"),
            Launch::Task { task, .. } => format!("Running {task}"),
            Launch::Maven { goals, module, .. } => format!("Running mvn {goals} in {module}"),
        };
        session.lines.push(ConsoleLine::system(what));
        if matches!(&launch, Launch::Tests { .. })
            || matches!(&launch, Launch::Config { config, .. } if config.kind == Kind::Tests)
        {
            session.tests = Some(TestTree::default());
        }
        match &state {
            EngineState::Ready => {}
            EngineState::Waiting(doing) => {
                session.waiting = true;
                session.push(
                    Stream::System,
                    &format!("Waiting for the Java engine ({doing}): the run starts as soon as it is ready."),
                );
            }
            EngineState::Unavailable(why) => {
                session.push(Stream::Err, why);
                session.ended = Some(Ended {
                    outcome: Outcome::Failed,
                    elapsed: Duration::ZERO,
                    exit_code: None,
                    error: Some(why.clone()),
                });
            }
        }
        self.sessions.push(session);
        self.selected.insert(place, id);
        if state == EngineState::Ready {
            self.dispatch(id, cx);
        }
        self.tick(cx);
        cx.emit(RunsEvent::Show(place));
        cx.notify();
        Some(id)
    }

    /// Send session `id`'s launch to the engine.
    fn dispatch(&mut self, id: u64, cx: &mut Context<Self>) {
        let engine = self.engine(cx).map(|(_, pid)| pid);
        let Some(s) = self.session_mut(id) else { return };
        s.waiting = false;
        s.engine = engine;
        let (method, params) = match &s.launch {
            Launch::Config { config, debug } => {
                let mut c = config.to_engine();
                // Spring Boot colours its output only on a terminal, unless told to.
                if config.kind == Kind::Main
                    && config.spring_boot
                    && !config.jvm_args.iter().any(|a| a.starts_with("-Dspring.output.ansi.enabled"))
                    && let Some(args) = c.get_mut("jvmArgs").and_then(Value::as_array_mut)
                {
                    args.push(json!("-Dspring.output.ansi.enabled=always"));
                }
                ("snout/run", json!({ "config": c, "debug": debug }))
            }
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
            Launch::Maven { build, project, goals, .. } => {
                ("snout/runTask", json!({ "build": build, "project": project, "task": goals }))
            }
        };
        let mut params = params;
        if method == "snout/runTask" || method == "snout/runTests" || method == "snout/run" {
            // The Maven window's choices go with every Maven run; Gradle's ignore them.
            let m = &self.project.maven;
            let choices = json!({ "profiles": m.profiles, "offline": m.offline, "skipTests": m.skip_tests });
            let target = if method == "snout/run" { params.get_mut("config") } else { Some(&mut params) };
            if let Some(Value::Object(o)) = target {
                for (k, v) in choices.as_object().expect("an object") {
                    if k != "skipTests" || method == "snout/runTask" {
                        o.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
            }
        }
        let call = self.request(method, params, cx);
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| this.started(id, result, cx));
        })
        .detach();
    }

    /// The engine moved on: start the sessions that waited for it, or tell them why they
    /// cannot start.
    fn start_waiting(&mut self, cx: &mut Context<Self>) {
        let waiting: Vec<(u64, bool)> = self
            .sessions
            .iter()
            .filter(|s| s.waiting && s.running())
            .map(|s| (s.id, s.launch.needs_classes()))
            .collect();
        for (id, classes) in waiting {
            match self.engine_state(classes, cx) {
                EngineState::Ready => {
                    if let Some(s) = self.session_mut(id) {
                        s.push(Stream::System, "The Java engine is ready.");
                    }
                    self.dispatch(id, cx);
                }
                EngineState::Waiting(_) => {}
                EngineState::Unavailable(why) => {
                    let Some(s) = self.session_mut(id) else { continue };
                    s.waiting = false;
                    s.push(Stream::Err, &why);
                    s.ended = Some(Ended {
                        outcome: Outcome::Failed,
                        elapsed: s.started.elapsed(),
                        exit_code: None,
                        error: Some(why.clone()),
                    });
                }
            }
        }
        cx.notify();
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
            waiting: false,
            pid: None,
            engine: None,
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
        s.pid = v.get("pid").and_then(Value::as_u64).map(|p| p as u32);
        // The command line a main class runs with, as the engine launched it.
        if let Some(command) = v.get("command").and_then(Value::as_array) {
            let words: Vec<String> = command
                .iter()
                .filter_map(Value::as_str)
                .map(|w| if w.contains(char::is_whitespace) { format!("\"{w}\"") } else { w.to_owned() })
                .collect();
            s.push(Stream::System, &words.join(" "));
        }
        if let Some(dir) = v.get("workingDir").and_then(Value::as_str) {
            s.push(Stream::System, &format!("Working directory: {dir}"));
        }
        if let Some(tasks) = v.get("tasks").and_then(Value::as_array) {
            let names: Vec<&str> = tasks.iter().filter_map(Value::as_str).collect();
            if !names.is_empty() {
                let tool = if v.get("tool").and_then(Value::as_str) == Some("maven") { "Maven" } else { "Gradle" };
                s.push(Stream::System, &format!("{tool}: {}", names.join(" ")));
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
        s.waiting = false;
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
        let next = (method == "snout/taskFinished").then(|| self.then.remove(&id)).flatten();
        if let Some(next) = next
            && self
                .sessions
                .iter()
                .any(|s| s.id == id && s.ended.as_ref().is_some_and(|e| e.outcome == Outcome::Success))
        {
            self.start(next, cx);
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
                title: format!(
                    "{}: {}",
                    if matches!(s.launch, Launch::Maven { .. }) { "Maven" } else { "Gradle" },
                    s.title
                ),
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
                let known = self.configurations().into_iter().find(|c| {
                    c.kind == Kind::Main
                        && c.main_class.as_deref() == Some(class)
                        && module.is_none_or(|m| c.module == m)
                });
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
                // As the title bar's configuration from now on, when it is one of the list.
                if self.configurations().iter().any(|c| c.id == config.id) {
                    self.select_config(Some(config.id.clone()), cx);
                }
                Launch::Config { config, debug }
            }
        };
        self.start(launch, cx);
    }

    /// The running session of the configuration `id`, in the Run or Debug window.
    pub fn running_config(&self, id: &str) -> Option<&Session> {
        self.sessions
            .iter()
            .rev()
            .find(|s| s.running() && matches!(&s.launch, Launch::Config { config, .. } if config.id == id))
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
            self.project.breakpoints.push(Breakpoint::new(path, line, None));
        }
        self.breakpoints_changed(path, cx);
    }

    /// Give the breakpoint on `line` a condition (none for always), setting it if need be.
    pub fn set_condition(&mut self, path: &Path, line: u32, condition: Option<String>, cx: &mut Context<Self>) {
        let condition = condition.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty());
        match self.project.breakpoints.iter_mut().find(|b| b.path == path && b.line == line) {
            Some(b) => b.condition = condition,
            None => self.project.breakpoints.push(Breakpoint::new(path, line, condition)),
        }
        self.breakpoints_changed(path, cx);
    }

    pub fn remove_breakpoint(&mut self, path: &Path, line: u32, cx: &mut Context<Self>) {
        self.project.breakpoints.retain(|b| !(b.path == path && b.line == line));
        self.breakpoints_changed(path, cx);
    }

    /// Enable or disable the breakpoint on `line` of `path` (a disabled one does not stop
    /// the program). False when there is none there.
    pub fn set_breakpoint_enabled(&mut self, path: &Path, line: u32, on: bool, cx: &mut Context<Self>) -> bool {
        let Some(b) = self.project.breakpoints.iter_mut().find(|b| b.path == path && b.line == line) else {
            return false;
        };
        b.enabled = on;
        self.breakpoints_changed(path, cx);
        true
    }

    /// Enable the breakpoint on `line` if it is disabled, else disable it. False when
    /// there is none there.
    pub fn toggle_breakpoint_enabled(&mut self, path: &Path, line: u32, cx: &mut Context<Self>) -> bool {
        let on = self.project.breakpoints.iter().find(|b| b.path == path && b.line == line).map(|b| b.enabled);
        match on {
            Some(on) => self.set_breakpoint_enabled(path, line, !on, cx),
            None => false,
        }
    }

    /// Remove every breakpoint of the project.
    pub fn remove_all_breakpoints(&mut self, cx: &mut Context<Self>) {
        let paths: std::collections::BTreeSet<PathBuf> = self.project.breakpoints.drain(..).map(|b| b.path).collect();
        for path in paths {
            self.breakpoints_changed(&path, cx);
        }
    }

    /// The breakpoints session `id`'s debugger gets for `path`: the project's, and the
    /// line Run to Cursor stops at.
    pub(crate) fn breakpoints_for(&self, id: u64, path: &Path) -> Vec<Breakpoint> {
        let mut list: Vec<Breakpoint> = self.breakpoints_in(path).into_iter().cloned().collect();
        if let Some((sid, p, line)) = &self.run_to
            && *sid == id
            && p == path
        {
            list.retain(|b| b.line != *line);
            list.push(Breakpoint::new(path, *line, None));
        }
        list
    }

    fn breakpoints_changed(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.save_project();
        let ids: Vec<u64> =
            self.sessions.iter().filter(|s| s.debug.as_ref().is_some_and(|d| d.live())).map(|s| s.id).collect();
        for id in ids {
            let list = self.breakpoints_for(id, path);
            self.send_breakpoints(id, path.to_owned(), list, cx);
        }
        self.sync_marks(cx);
        cx.notify();
    }

    /// The debug session the Debug window shows, while its program is stopped.
    pub fn paused_debug(&self) -> Option<u64> {
        self.shown(Place::Debug).filter(|s| s.debug.as_ref().is_some_and(|d| d.paused && d.live())).map(|s| s.id)
    }

    /// The debug session the Debug window shows, while it is connected.
    pub fn live_debug(&self) -> Option<u64> {
        self.shown(Place::Debug).filter(|s| s.debug.as_ref().is_some_and(|d| d.live())).map(|s| s.id)
    }

    /// Run to Cursor: let session `id`'s program go on until it reaches `line` (from 0)
    /// of `path`, as if a breakpoint were there for once (other breakpoints still stop it).
    pub fn run_to_cursor(&mut self, id: u64, path: &Path, line: u32, cx: &mut Context<Self>) {
        self.run_to = Some((id, path.to_owned(), line));
        let list = self.breakpoints_for(id, path);
        self.send_breakpoints(id, path.to_owned(), list, cx);
        self.step(id, "continue", cx);
    }

    /// The program of session `id` stopped: the line Run to Cursor set goes.
    pub(crate) fn clear_run_to(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some((sid, path, _)) = self.run_to.take_if(|(sid, _, _)| *sid == id) {
            let list = self.breakpoints_for(sid, &path);
            self.send_breakpoints(sid, path, list, cx);
        }
    }

    /// The most recent session, for Rerun: the one the Run or Debug window shows, the
    /// newest of them.
    pub fn last_session(&self) -> Option<u64> {
        [Place::Run, Place::Debug, Place::Build]
            .into_iter()
            .filter_map(|p| self.shown(p))
            .max_by_key(|s| s.id)
            .map(|s| s.id)
    }

    /// The newest session that ran tests, for Rerun Failed Tests.
    pub fn last_tests(&self) -> Option<u64> {
        self.sessions.iter().rev().find(|s| s.launch.test_module().is_some() && s.tests.is_some()).map(|s| s.id)
    }

    /// Hand the editors what they draw.
    pub fn sync_marks(&mut self, cx: &mut Context<Self>) {
        let mut breakpoints: BTreeMap<PathBuf, Vec<(u32, Option<String>, bool)>> = BTreeMap::new();
        for b in &self.project.breakpoints {
            breakpoints.entry(b.path.clone()).or_default().push((b.line, b.condition.clone(), b.enabled));
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

/// The main classes a file declares, with the lines their run icons go on (from 0): the
/// class's own line and its `main` method's, for a class with a `main` method (Java, or a
/// Scala object's `def main`); the class's line for one marked `@SpringBootApplication`.
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
            && let Some((at, name)) = &class_line
        {
            let class = qualify(name);
            if !out.iter().any(|(l, c)| *c == class && l == at) {
                out.push((*at, class.clone()));
            }
            if *at != i as u32 {
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
        let app = |l: u32| (l, "demo.app.App".to_owned());
        assert_eq!(main_classes(Path::new("/w/App.java"), java), [app(3), app(4)]);
        let plain = "package demo;\nclass Tool {\n    static void main(String... a) {}\n}\n";
        let tool = |l: u32| (l, "demo.Tool".to_owned());
        assert_eq!(main_classes(Path::new("/w/Tool.java"), plain), [tool(1), tool(2)]);
        let scala = "package s\nobject Main {\n  def main(args: Array[String]): Unit = ()\n}\n";
        let main = |l: u32| (l, "s.Main".to_owned());
        assert_eq!(main_classes(Path::new("/w/Main.scala"), scala), [main(1), main(2)]);
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
