//! The language servers of the window: one Java engine per Gradle build, started when a
//! Java file of the build opens (or the project is one), restarted when it crashes, and
//! stopped when the app quits or another project opens in its place. The hub keeps the documents in sync, holds what the
//! servers publish (diagnostics, progress), and asks the workspace to open locations and
//! apply edits.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::time::{Duration, Instant};

use gpui::{App, Context, Entity, EventEmitter, Global, Task, WeakEntity};
use gpui_component::input::Rope;
use lsp_types::{Diagnostic, Location, Position, Uri, WorkspaceEdit};
use serde_json::{Value, json};
use snout_lang::progress::Progress;
use snout_lang::{Client, ResponseError, ServerMessage};

use super::java::{self, DownloadProgress, Jdk};
use super::text;
use crate::editor::EditorPanel;
use crate::host::HostRef;
use crate::model::Model;

/// Where a server is, for the status bar.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    /// No JDK fits the build yet.
    NeedsJdk(u32),
    Importing,
    CompilingScala,
    Starting,
    Indexing,
    Ready,
    Restarting,
    /// The engine is being built (snout.sh, in development): it starts once it is.
    WaitingForEngine,
    /// Stopped from the Processes window: it starts again on Restart.
    Stopped,
    Failed(String),
}

impl Stage {
    pub fn label(&self) -> String {
        match self {
            Stage::NeedsJdk(v) => format!("Needs JDK {v}"),
            Stage::Importing => "Importing Gradle model".into(),
            Stage::CompilingScala => "Compiling Scala".into(),
            Stage::Starting => "Starting".into(),
            Stage::Indexing => "Indexing".into(),
            Stage::Ready => "Ready".into(),
            Stage::Restarting => "Restarting".into(),
            Stage::WaitingForEngine => "Waiting for snout-engine".into(),
            Stage::Stopped => "Stopped".into(),
            Stage::Failed(why) => why.clone(),
        }
    }

    pub fn busy(&self) -> bool {
        matches!(
            self,
            Stage::Importing
                | Stage::CompilingScala
                | Stage::Starting
                | Stage::Indexing
                | Stage::Restarting
                | Stage::WaitingForEngine
        )
    }
}

/// How far a module, or the whole project, is (`snout/readiness`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ready {
    /// `syncing`, `compilingScala`, `importing` or `ready`.
    pub stage: String,
    pub percent: Option<u32>,
    /// A shared source folder rather than a module.
    pub shared: bool,
}

impl Ready {
    pub fn is_ready(&self) -> bool {
        self.stage == "ready"
    }

    /// "Compiling Scala" and the like.
    pub fn label(&self) -> &'static str {
        readiness_label(&self.stage)
    }
}

/// The readable name of a readiness stage.
pub fn readiness_label(stage: &str) -> &'static str {
    match stage {
        "syncing" => "Syncing the build model",
        "compilingScala" => "Compiling Scala",
        "importing" => "Importing",
        "ready" => "Ready",
        _ => "Starting",
    }
}

/// What the engine said last about how far the project and each module are.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Readiness {
    pub project: Option<Ready>,
    /// How many modules there are, and how many are ready.
    pub total: usize,
    pub ready: usize,
    pub modules: BTreeMap<String, Ready>,
}

impl Readiness {
    /// Take a `snout/readiness` notification: one module's, or (`module: null`) the
    /// project's. True when something changed.
    pub fn apply(&mut self, v: &Value) -> bool {
        let before = self.clone();
        let ready = Ready {
            stage: v.get("stage").and_then(Value::as_str).unwrap_or_default().to_owned(),
            percent: v.get("percent").and_then(Value::as_f64).map(|p| p.round().clamp(0., 100.) as u32),
            shared: v.get("shared").and_then(Value::as_bool).unwrap_or(false),
        };
        match v.get("module").and_then(Value::as_str) {
            Some(module) => {
                self.modules.insert(module.to_owned(), ready);
            }
            None => {
                if let Some(n) = v.get("modules").and_then(Value::as_u64) {
                    self.total = n as usize;
                }
                if let Some(n) = v.get("ready").and_then(Value::as_u64) {
                    self.ready = n as usize;
                }
                self.project = Some(ready);
            }
        }
        *self != before
    }

    /// From the answer to the `snout/readiness` request.
    pub fn from_answer(v: &Value) -> Self {
        let mut out = Self::default();
        for m in v.get("modules").and_then(Value::as_array).into_iter().flatten() {
            out.apply(m);
        }
        let mut project = v.clone();
        if let Some(o) = project.as_object_mut() {
            o.remove("modules");
            o.insert("module".into(), Value::Null);
        }
        out.apply(&project);
        out.total = out.modules.len();
        out.ready = out.modules.values().filter(|m| m.is_ready()).count();
        out
    }

    /// The project is not ready yet.
    pub fn pending(&self) -> bool {
        self.project.as_ref().is_some_and(|p| !p.is_ready())
    }

    /// "7 of 12 modules ready", or the project's stage while the modules are not known.
    pub fn summary(&self) -> String {
        match (&self.project, self.total) {
            (_, n) if n > 0 => format!("{} of {n} modules ready", self.ready),
            (Some(p), _) => p.label().to_owned(),
            (None, _) => "Starting".to_owned(),
        }
    }
}

/// One Java engine.
pub struct Server {
    pub root: PathBuf,
    pub jdk: Option<Jdk>,
    client: Option<Client>,
    pub stage: Stage,
    pub progress: Progress,
    initialized: bool,
    /// What the server published, by document URI.
    pub diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    /// Crashes in a row, for the restart backoff.
    crashes: u32,
    /// Starting again after the engine stopped unexpectedly (or its language server did): the
    /// how many-th time in a row. Cleared once it is ready.
    pub recovering: Option<u32>,
    started: Instant,
    /// From start to the first diagnostics for an open file.
    pub first_diagnostics: Option<Duration>,
    /// Bumped on each start, so messages of an earlier process are ignored.
    generation: u64,
    /// Stopped on purpose (the app is quitting): no restart.
    stopped: bool,
    /// How far the project and its modules are, as the engine says.
    pub readiness: Readiness,
    _tasks: Vec<Task<()>>,
}

impl Server {
    /// "Java: Indexing (40%)", "Java: 7 of 12 modules ready" and the like.
    pub fn status(&self) -> String {
        if matches!(self.stage, Stage::Indexing | Stage::Ready) && self.readiness.pending() {
            return format!("Java: {}", self.readiness.summary());
        }
        let what = match (&self.stage, self.progress.current()) {
            (Stage::Ready | Stage::Indexing, Some(work)) => work.label(),
            (stage, _) => stage.label(),
        };
        format!("Java: {what}")
    }

    pub fn busy(&self) -> bool {
        self.stage.busy() || (self.stage == Stage::Ready && self.progress.is_busy()) || self.readiness.pending()
    }

    /// How far the module `module` is, when the engine said and it is not ready yet.
    pub fn module_pending(&self, module: &str) -> Option<&Ready> {
        self.readiness.modules.get(module).filter(|r| !r.is_ready())
    }

    pub fn ready(&self) -> bool {
        self.initialized && self.client.as_ref().is_some_and(Client::is_alive)
    }

    /// The engine's process, while it runs.
    pub fn pid(&self) -> Option<u32> {
        self.client.as_ref().filter(|c| c.is_alive()).map(Client::pid)
    }

    /// It can be started again by hand: stopped, or given up on after crashing again and again.
    pub fn restartable(&self) -> bool {
        matches!(self.stage, Stage::Stopped | Stage::Failed(_)) && self.jdk.is_some()
    }
}

/// A document the servers know: a file open in one or more editor tabs.
struct Doc {
    root: PathBuf,
    uri: Uri,
    /// The text the server has, as last sent.
    text: Rope,
    version: i32,
    editors: Vec<WeakEntity<EditorPanel>>,
    /// A class file (`jdt://`): read only, never opened on the server.
    virtual_doc: bool,
    /// The language the server is told it is: `java`, or a Spring Boot configuration file's.
    language: &'static str,
}

/// The bar asking to download a JDK, and how the download goes.
#[derive(Clone)]
pub struct JdkPrompt {
    pub root: PathBuf,
    pub major: u32,
    /// Asked for by the Gradle model (a build that failed for want of this JDK): the model
    /// is synced again once it is downloaded.
    pub resync: bool,
    pub progress: Option<DownloadProgress>,
    pub error: Option<String>,
}

/// What the hub asks of the workspace.
#[derive(Clone, Debug)]
pub enum HubEvent {
    /// Diagnostics or status changed.
    Changed,
    /// Show this location (a file, or a class file of a library).
    Open { root: PathBuf, uri: Uri, at: Position },
    /// Apply this edit; answer the server's request `reply` when there is one.
    ApplyEdit { edit: WorkspaceEdit, reply: Option<(PathBuf, Value)> },
    /// Show these locations in the References panel.
    References { title: String, locations: Vec<Location> },
    /// One of the engine's own notifications (`snout/taskOutput`, `snout/testEvent`…).
    Engine { root: PathBuf, method: String, params: Value },
}

pub struct LangHub {
    pub(crate) model: Entity<Model>,
    /// Servers start only when this is set (the app sets it; most tests do not).
    pub enabled: bool,
    pub servers: HashMap<PathBuf, Server>,
    docs: HashMap<String, Doc>,
    pub jdk_prompt: Option<JdkPrompt>,
    /// The JDKs the user said no to, this session.
    dismissed_jdks: Vec<u32>,
    /// The home folder JDKs are looked for and downloaded under.
    pub home: PathBuf,
    /// The host the engines run on.
    pub host: HostRef,
    /// Held until the window has painted: the builds asked for meanwhile start then.
    pub paused: bool,
    pending: Vec<PathBuf>,
    /// The engine build snout.sh runs beside the app, in development.
    pub engine_build: Option<crate::background::EngineBuild>,
    /// What the last build's compile reported (`snout/buildProblems`), by document URI.
    pub build_problems: Vec<(String, Diagnostic)>,
    /// The project open when the servers were last checked: those of another go.
    project: Option<PathBuf>,
    _engine_watch: Option<Task<()>>,
    _quit: gpui::Subscription,
    _project: gpui::Subscription,
}

impl EventEmitter<HubEvent> for LangHub {}

/// The window's hub, for the editors to find.
pub struct HubHandle(pub Entity<LangHub>);

impl Global for HubHandle {}

/// The hub, if the app has one.
pub fn hub(cx: &App) -> Option<Entity<LangHub>> {
    cx.try_global::<HubHandle>().map(|h| h.0.clone())
}

/// Whether servers should start: the app turns them on; `SNOUT_NO_LSP` turns them off.
pub struct LangEnabled(pub bool);

impl Global for LangEnabled {}

/// A number for each engine process the window starts, never reused: what a process says
/// after it was replaced (or stopped and its build started again) is told apart.
fn next_generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn uri_key(uri: &Uri) -> String {
    uri.as_str().to_owned()
}

impl LangHub {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        let enabled = cx.try_global::<LangEnabled>().is_some_and(|e| e.0) && std::env::var_os("SNOUT_NO_LSP").is_none();
        let host = crate::host::host(cx);
        let home = host.info().home.clone();
        let quit = cx.on_app_quit(|this, _| {
            this.shutdown_all();
            async {}
        });
        let project = model.read(cx).project_root();
        let project_sub = cx.observe(&model, |this, model, cx| {
            let project = model.read(cx).project_root();
            if project != this.project {
                this.project = project;
                this.keep_project_servers(cx);
            }
        });
        let mut this = Self {
            model,
            enabled,
            servers: HashMap::new(),
            docs: HashMap::new(),
            jdk_prompt: None,
            dismissed_jdks: Vec::new(),
            home,
            host,
            paused: false,
            pending: Vec::new(),
            engine_build: None,
            build_problems: Vec::new(),
            project,
            _engine_watch: None,
            _quit: quit,
            _project: project_sub,
        };
        if let Some(file) = std::env::var_os(crate::background::STATUS_ENV).filter(|_| enabled) {
            this.watch_engine_build(PathBuf::from(file), cx);
        }
        this
    }

    /// Follow snout.sh's engine build through its status file; when it ends, start the
    /// servers that waited for it.
    pub fn watch_engine_build(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        self.engine_build = Some(crate::background::EngineBuild::read(&file));
        self._engine_watch = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(400)).await;
                let now = crate::background::EngineBuild::read(&file);
                let Ok(finished) = this.update(cx, |this, cx| {
                    let changed = this.engine_build.as_ref() != Some(&now);
                    let finished = now.finished();
                    this.engine_build = Some(now);
                    if changed {
                        cx.emit(HubEvent::Changed);
                        cx.notify();
                    }
                    if finished {
                        this.engine_ready(cx);
                    }
                    finished
                }) else {
                    return;
                };
                if finished {
                    return;
                }
            }
        }));
    }

    /// The engine build ended: start the servers that waited.
    fn engine_ready(&mut self, cx: &mut Context<Self>) {
        let waiting: Vec<PathBuf> = self
            .servers
            .iter()
            .filter(|(_, s)| s.stage == Stage::WaitingForEngine)
            .map(|(root, _)| root.clone())
            .collect();
        for root in waiting {
            self.spawn(&root, cx);
        }
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// The window has painted: start the servers asked for meanwhile.
    pub fn resume(&mut self, cx: &mut Context<Self>) {
        self.paused = false;
        for root in std::mem::take(&mut self.pending) {
            self.ensure_server(&root, cx);
        }
    }

    /// The folder whose engine serves `path`: the project, when it holds Gradle builds and
    /// `path` is in it or in a source folder its model lists (one engine syncs every build
    /// of the folder); else the Gradle build of `path` inside the project. A file of no
    /// build of the project gets none: engines run for the project shown, and no other.
    /// `build` is the Gradle build of `path` within the project (`java::gradle_root`), as
    /// the host showed it off the UI thread.
    pub fn root_for(&self, path: &Path, build: Option<PathBuf>, cx: &App) -> Option<PathBuf> {
        let project = self.model.read(cx).project_root();
        let Some(project) = &project else { return build };
        let listed =
            crate::project_model::store(cx).and_then(|s| s.read(cx).model_for(project)).is_some_and(|m| m.holds(path));
        let inside = path.starts_with(project) && build.is_some();
        // A build a few levels down in the project is one the project's look for builds finds
        // (`project_model::has_gradle`): no need to wait for that look, which a tab restored
        // at startup would otherwise beat, starting an engine of its own for its build.
        let near = build
            .as_deref()
            .and_then(|b| b.strip_prefix(project).ok())
            .is_some_and(|rel| rel.components().count() <= crate::project_model::GRADLE_DEPTH);
        if (inside || listed) && (near || crate::project_model::project_has_gradle(project, cx)) {
            return Some(project.clone());
        }
        build.filter(|r| r.starts_with(project))
    }

    /// The engine that serves the folder `project`: its own, else one of a build inside it,
    /// else the one of the build it lies in.
    pub fn engine_root(&self, project: &Path) -> Option<PathBuf> {
        if self.servers.contains_key(project) {
            return Some(project.to_owned());
        }
        let mut inside: Vec<&PathBuf> = self.servers.keys().filter(|r| r.starts_with(project)).collect();
        inside.sort();
        if let Some(r) = inside.first() {
            return Some((*r).clone());
        }
        self.servers.keys().filter(|r| project.starts_with(r)).max_by_key(|r| r.as_os_str().len()).cloned()
    }

    /// Another project opened (or none): stop the engines of builds outside it, with their
    /// jdtls and Gradle daemons, and forget what they said.
    fn keep_project_servers(&mut self, cx: &mut Context<Self>) {
        let keep = |root: &Path, project: &Option<PathBuf>| project.as_deref().is_some_and(|p| root.starts_with(p));
        let gone: Vec<PathBuf> = self.servers.keys().filter(|r| !keep(r, &self.project)).cloned().collect();
        let project = self.project.clone();
        self.pending.retain(|r| keep(r, &project));
        if self.jdk_prompt.as_ref().is_some_and(|p| !keep(&p.root, &project)) {
            self.jdk_prompt = None;
        }
        if gone.is_empty() {
            return;
        }
        for root in &gone {
            self.stop_server(root);
        }
        self.docs.retain(|_, d| !gone.contains(&d.root));
        self.build_problems.clear();
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// Stop the engine of `root` and forget it: `shutdown` and `exit`, then SIGTERM if it
    /// lingers (its own hook stops jdtls and its Gradle daemons), on a thread of its own.
    pub fn stop_server(&mut self, root: &Path) {
        let Some(mut server) = self.servers.remove(root) else { return };
        server.stopped = true;
        // What it says until it ends is read and let go (no server of its generation).
        for task in server._tasks.drain(..) {
            task.detach();
        }
        let Some(client) = server.client.take() else { return };
        let (root, pid) = (root.to_owned(), client.pid());
        eprintln!("[java] stopping the engine for {} (pid {pid})", root.display());
        std::thread::spawn(move || {
            let started = Instant::now();
            client.shutdown_blocking(Duration::from_secs(5));
            eprintln!(
                "[java] the engine for {} (pid {pid}) stopped in {:.1} s",
                root.display(),
                started.elapsed().as_secs_f32()
            );
        });
    }

    /// The server of the build at `root`, once it answers requests.
    pub fn client(&self, root: &Path) -> Option<Client> {
        self.servers.get(root).filter(|s| s.ready()).and_then(|s| s.client.clone())
    }

    /// The server a document goes to.
    pub fn root_of(&self, uri: &Uri) -> Option<PathBuf> {
        self.docs.get(&uri_key(uri)).map(|d| d.root.clone())
    }

    /// Start the server of the build at `root` unless it runs.
    pub fn ensure_server(&mut self, root: &Path, cx: &mut Context<Self>) {
        if !self.enabled || self.servers.contains_key(root) {
            return;
        }
        if self.paused {
            if !self.pending.iter().any(|r| r == root) {
                self.pending.push(root.to_owned());
            }
            return;
        }
        // What the build needs and which JDK fits are asked of the host off the UI thread.
        let saved = self.model.read(cx).jdk_for(root);
        let added = crate::structure::added_jdks(
            &self.model.read(cx).settings,
            crate::remote::connection(cx).map(|(n, _)| n).as_deref(),
        );
        let (host, home, root_buf) = (self.host.clone(), self.home.clone(), root.to_owned());
        let look = cx.background_executor().spawn(async move {
            // On a remote machine the engine and its Java may still be on their way there:
            // they come first (the JDK it downloads is one the build may run on).
            if java::is_remote(&*host) && java::engine_dist(&*host).is_none() {
                let progress = java::DownloadProgress::default();
                if let Err(e) = super::remote_engine::install(&*host, &|_| {}, &progress) {
                    eprintln!("[java] the engine could not be installed: {e:#}");
                }
            }
            let saved = saved.and_then(|h| java::jdk_at(&*host, &h));
            let req = java::requirement(&*host, &root_buf);
            let jdk = saved.filter(|j| req.fits(j.major)).or_else(|| {
                let all = java::discover_with(&*host, &home, &added);
                java::choose(&req, &all).cloned()
            });
            (req, jdk)
        });
        let server = Server {
            root: root.to_owned(),
            jdk: None,
            client: None,
            stage: Stage::Starting,
            progress: Progress::default(),
            initialized: false,
            diagnostics: BTreeMap::new(),
            crashes: 0,
            recovering: None,
            started: Instant::now(),
            first_diagnostics: None,
            generation: next_generation(),
            stopped: false,
            readiness: Readiness::default(),
            _tasks: Vec::new(),
        };
        let generation = server.generation;
        self.servers.insert(root.to_owned(), server);
        let root_buf = root.to_owned();
        cx.spawn(async move |this, cx| {
            let (req, jdk) = look.await;
            let _ = this.update(cx, |this, cx| this.jdk_found(&root_buf, generation, req, jdk, cx));
        })
        .detach();
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// The JDK for the build at `root` is found (or none fits `req`): start its engine, or
    /// offer the JDK it needs.
    fn jdk_found(
        &mut self,
        root: &Path,
        generation: u64,
        req: java::Requirement,
        jdk: Option<Jdk>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) =
            self.servers.get_mut(root).filter(|s| s.generation == generation && s.client.is_none() && !s.stopped)
        else {
            return;
        };
        match jdk {
            Some(jdk) => {
                server.jdk = Some(jdk.clone());
                self.remember_jdk(root, &jdk, cx);
                self.spawn(root, cx);
            }
            None => {
                server.stage =
                    Stage::NeedsJdk(req.wanted_on(self.host.info().arch == "aarch64" && self.host.info().is_mac()));
                if self.jdk_prompt.is_none() {
                    eprintln!(
                        "[java] no JDK fits {}: offering JDK {}",
                        root.display(),
                        req.wanted_on(self.host.info().arch == "aarch64" && self.host.info().is_mac())
                    );
                    self.jdk_prompt = Some(JdkPrompt {
                        root: root.to_owned(),
                        major: req.wanted_on(self.host.info().arch == "aarch64" && self.host.info().is_mac()),
                        resync: false,
                        progress: None,
                        error: None,
                    });
                }
            }
        }
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    fn remember_jdk(&mut self, root: &Path, jdk: &Jdk, cx: &mut Context<Self>) {
        self.model.update(cx, |m, _| {
            if m.settings.project_jdks.get(root) != Some(&jdk.home) {
                m.settings.project_jdks.insert(root.to_owned(), jdk.home.clone());
                m.persist();
            }
        });
    }

    /// Start the engine of `root` again (it syncs the Gradle model as it starts).
    pub fn restart(&mut self, root: &Path, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root) else { return };
        // Messages of the process going away are those of an earlier generation.
        server.generation = next_generation();
        server.crashes = 0;
        server.recovering = None;
        server.stopped = false;
        server.initialized = false;
        server.progress.clear();
        if let Some(old) = server.client.take() {
            std::thread::spawn(move || {
                drop(old.request_raw("shutdown", Value::Null));
                std::thread::sleep(Duration::from_millis(300));
                old.notify_raw("exit", Value::Null);
                old.kill(Duration::from_millis(1500));
            });
        }
        server.stage = Stage::Importing;
        self.spawn(root, cx);
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// Stop the engine of `root` (and with it its language server and Gradle daemons), and
    /// keep it stopped until it is restarted. Its documents stay, for the restart.
    pub fn stop_engine(&mut self, root: &Path, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root) else { return };
        // What the process says until it ends is of an earlier generation.
        server.generation = next_generation();
        server.initialized = false;
        server.recovering = None;
        server.progress.clear();
        server.stage = Stage::Stopped;
        if let Some(client) = server.client.take() {
            let pid = client.pid();
            eprintln!("[java] stopping the engine for {} (pid {pid}) on request", root.display());
            std::thread::spawn(move || client.shutdown_blocking(Duration::from_secs(5)));
        }
        self.clear_diagnostics(root, cx);
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// Forget what the engine of `root` published, in the editors too.
    fn clear_diagnostics(&mut self, root: &Path, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root) else { return };
        let cleared: Vec<String> = std::mem::take(&mut server.diagnostics).into_keys().collect();
        for uri in cleared {
            if let Some(doc) = self.docs.get(&uri) {
                for editor in doc.editors.iter().filter_map(|e| e.upgrade()) {
                    editor.update(cx, |e, cx| e.set_diagnostics(Vec::new(), cx));
                }
            }
        }
    }

    /// Start (or restart) the engine of `root` on its JDK.
    fn spawn(&mut self, root: &Path, cx: &mut Context<Self>) {
        let idle_minutes = self.model.read(cx).settings.daemon_idle_minutes;
        let maven_profiles = crate::run::maven_profiles_of(&self.model, root, cx);
        let Some(server) = self.servers.get_mut(root) else { return };
        let Some(jdk) = server.jdk.clone() else { return };
        if self.engine_build.as_ref().is_some_and(|b| !b.finished()) {
            server.stage = Stage::WaitingForEngine;
            return;
        }
        // The engine is found and started on the host off the UI thread.
        let host = self.host.clone();
        server.generation = next_generation();
        server.stage = Stage::Importing;
        let generation = server.generation;
        let root_buf = root.to_owned();
        let start = cx.background_executor().spawn(async move {
            // On a remote machine the engine may still be on its way there (or not yet asked
            // for): put it there first.
            if java::is_remote(&*host) && java::engine_dist(&*host).is_none() {
                let progress = java::DownloadProgress::default();
                super::remote_engine::install(&*host, &|_| {}, &progress)
                    .map_err(|e| format!("The Java engine could not be installed: {e:#}"))?;
            }
            let dist =
                java::engine_dist(&*host).ok_or_else(|| "The Java engine is not built (run snout.sh)".to_owned())?;
            let command = java::engine_command(&*host, &dist, &root_buf, &jdk.home, idle_minutes, &maven_profiles);
            let started = Client::spawn(&*host, &command).map_err(|e| format!("The Java engine did not start: {e}"))?;
            Ok((started, jdk))
        });
        let root_buf = root.to_owned();
        cx.spawn(async move |this, cx| {
            let started = start.await;
            let _ = this.update(cx, |this, cx| {
                this.started(&root_buf, generation, started, cx);
                cx.emit(HubEvent::Changed);
                cx.notify();
            });
        })
        .detach();
    }

    /// The engine of `root` started (or did not) for the generation `generation`.
    fn started(
        &mut self,
        root: &Path,
        generation: u64,
        started: Result<((Client, snout_lang::Messages), Jdk), String>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) =
            self.servers.get_mut(root).filter(|s| s.generation == generation && s.client.is_none() && !s.stopped)
        else {
            // Stopped or restarted meanwhile: this process is not wanted (dropping it stops it).
            return;
        };
        let ((client, events), jdk) = match started {
            Ok(s) => s,
            Err(e) => {
                server.stage = Stage::Failed(e);
                return;
            }
        };
        eprintln!("[java] started the engine for {} on JDK {} (pid {})", root.display(), jdk.version, client.pid());
        crate::startup::mark("Java engine started");
        server.client = Some(client.clone());
        server.stage = Stage::Importing;
        server.progress.clear();
        server.initialized = false;
        server.started = Instant::now();
        server.diagnostics.clear();
        server.readiness = Readiness::default();
        let root_buf = root.to_owned();
        let listen = cx.spawn(async move |this, cx| {
            while let Ok(message) = events.recv().await {
                let gone = this.update(cx, |this, cx| this.on_message(&root_buf, generation, message, cx)).is_err();
                if gone {
                    return;
                }
            }
        });
        let root_buf = root.to_owned();
        let init = client.request_raw("initialize", initialize_params(root));
        let initialize = cx.spawn(async move |this, cx| {
            let result = init.await;
            let _ = this.update(cx, |this, cx| this.on_initialized(&root_buf, generation, result, cx));
        });
        server._tasks = vec![listen, initialize];
    }

    fn on_initialized(
        &mut self,
        root: &Path,
        generation: u64,
        result: Result<Value, ResponseError>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.servers.get_mut(root).filter(|s| s.generation == generation) else { return };
        let Some(client) = server.client.clone() else { return };
        if let Err(e) = result {
            eprintln!("[java] initialize failed: {e}");
            return;
        }
        client.notify_raw("initialized", json!({}));
        server.initialized = true;
        // How far the project is now; notifications tell the rest.
        let ask = client.request_raw("snout/readiness", json!({}));
        let root_buf = root.to_owned();
        server._tasks.push(cx.spawn(async move |this, cx| {
            let Ok(v) = ask.await else { return };
            let _ = this.update(cx, |this, cx| {
                let Some(s) = this.servers.get_mut(&root_buf).filter(|s| s.generation == generation) else { return };
                if s.readiness.project.is_none() {
                    s.readiness = Readiness::from_answer(&v);
                    cx.emit(HubEvent::Changed);
                    cx.notify();
                }
            });
        }));
        if matches!(server.stage, Stage::Importing | Stage::CompilingScala | Stage::Starting) {
            server.stage = Stage::Indexing;
        }
        // The documents open in editors, as they are now.
        for doc in self.docs.values_mut().filter(|d| d.root == root && !d.virtual_doc) {
            doc.version = 1;
            client.notify_raw(
                "textDocument/didOpen",
                json!({ "textDocument": {
                    "uri": doc.uri.as_str(), "languageId": doc.language, "version": doc.version,
                    "text": doc.text.to_string()
                } }),
            );
        }
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    fn on_message(&mut self, root: &Path, generation: u64, message: ServerMessage, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root).filter(|s| s.generation == generation) else { return };
        match message {
            ServerMessage::Stderr(line) => {
                eprintln!("[java] {line}");
                if let Some(rest) = line.strip_prefix("snout-engine: ") {
                    let stage = if rest.starts_with("syncing") {
                        Some(Stage::Importing)
                    } else if rest.starts_with("compiling Scala") {
                        Some(Stage::CompilingScala)
                    } else if rest.starts_with("starting jdtls") {
                        Some(Stage::Starting)
                    } else {
                        None
                    };
                    if let Some(stage) = stage.filter(|_| !server.initialized) {
                        server.stage = stage;
                        cx.notify();
                    }
                }
            }
            ServerMessage::Notification { method, params } => self.on_notification(root, &method, params, cx),
            ServerMessage::Request { id, method, params } => self.on_request(root, id, &method, params, cx),
            ServerMessage::Exited(code) => self.on_exit(root, code, cx),
        }
    }

    fn on_notification(&mut self, root: &Path, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root) else { return };
        match method {
            "textDocument/publishDiagnostics" => {
                let Some(uri) = params.get("uri").and_then(Value::as_str).map(str::to_owned) else { return };
                let list: Vec<Diagnostic> =
                    serde_json::from_value(params.get("diagnostics").cloned().unwrap_or_default()).unwrap_or_default();
                if server.first_diagnostics.is_none() && self.docs.contains_key(&uri) {
                    let took = server.started.elapsed();
                    server.first_diagnostics = Some(took);
                    eprintln!("[java] first diagnostics for an open file after {:.1} s", took.as_secs_f32());
                    crate::startup::mark("first diagnostics for an open file");
                }
                if list.is_empty() {
                    server.diagnostics.remove(&uri);
                } else {
                    server.diagnostics.insert(uri.clone(), list.clone());
                }
                if let Some(doc) = self.docs.get(&uri) {
                    for editor in doc.editors.iter().filter_map(|e| e.upgrade()) {
                        let list = list.clone();
                        editor.update(cx, |e, cx| e.set_diagnostics(list, cx));
                    }
                }
                cx.emit(HubEvent::Changed);
                cx.notify();
            }
            "$/progress" => {
                if let Ok(p) = serde_json::from_value(params)
                    && server.progress.apply(p)
                {
                    cx.notify();
                }
            }
            "language/status" => {
                let kind = params.get("type").and_then(Value::as_str).unwrap_or_default();
                match kind {
                    "ServiceReady" => {
                        server.stage = Stage::Ready;
                        crate::startup::mark("Java language server ready");
                        if server.recovering.take().is_some() {
                            eprintln!("[java] the engine for {} is back", root.display());
                        }
                        // Build once, so the search index covers the imported projects.
                        if let Some(c) = server.client.clone() {
                            let build = c.request_raw("java/buildWorkspace", json!(false));
                            cx.background_executor()
                                .spawn(async move {
                                    let _ = build.await;
                                })
                                .detach();
                        }
                    }
                    "Starting" if !server.initialized => server.stage = Stage::Starting,
                    "Error" => {
                        let message = params.get("message").and_then(Value::as_str).unwrap_or("error");
                        eprintln!("[java] status error: {message}");
                    }
                    _ => {}
                }
                cx.emit(HubEvent::Changed);
                cx.notify();
            }
            "window/showMessage" => {
                let level = params.get("type").and_then(Value::as_u64).unwrap_or(4);
                let text = params.get("message").and_then(Value::as_str).unwrap_or_default().to_owned();
                eprintln!("[java] message: {text}");
                if level <= 1 {
                    self.model.update(cx, |m, cx| {
                        m.message = format!("Java: {text}");
                        cx.notify();
                    });
                }
            }
            "snout/readiness" => {
                if server.readiness.apply(&params) {
                    if params.get("module").is_none_or(Value::is_null)
                        && let Some(p) = &server.readiness.project
                    {
                        eprintln!("[java] readiness: {}, {}", p.label(), server.readiness.summary());
                        if p.is_ready() {
                            crate::startup::mark("every module ready");
                        }
                    }
                    // The editors of the build show their module's state.
                    for doc in self.docs.values().filter(|d| d.root == root) {
                        for editor in doc.editors.iter().filter_map(|e| e.upgrade()) {
                            editor.update(cx, |_, cx| cx.notify());
                        }
                    }
                    cx.emit(HubEvent::Changed);
                    cx.notify();
                }
            }
            m if m.starts_with("snout/") => {
                cx.emit(HubEvent::Engine { root: root.to_owned(), method: m.to_owned(), params });
            }
            _ => {}
        }
    }

    /// Put what a build's compile reported in place of what the last one did.
    pub fn set_build_problems(&mut self, list: Vec<(String, Diagnostic)>, cx: &mut Context<Self>) {
        self.build_problems = list;
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    fn on_request(&mut self, root: &Path, id: Value, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(client) = self.servers.get(root).and_then(|s| s.client.clone()) else { return };
        match method {
            "workspace/applyEdit" => match serde_json::from_value::<WorkspaceEdit>(params["edit"].clone()) {
                Ok(edit) => cx.emit(HubEvent::ApplyEdit { edit, reply: Some((root.to_owned(), id)) }),
                Err(e) => client.respond(id, Ok(json!({ "applied": false, "failureReason": e.to_string() }))),
            },
            "window/showDocument" => {
                let uri = params.get("uri").and_then(Value::as_str).and_then(|u| Uri::from_str(u).ok());
                let at = params
                    .get("selection")
                    .and_then(|s| serde_json::from_value::<lsp_types::Range>(s.clone()).ok())
                    .map_or(Position::default(), |r| r.start);
                let external = params.get("external").and_then(Value::as_bool).unwrap_or(false);
                match uri {
                    Some(uri) if external => cx.open_url(uri.as_str()),
                    Some(uri) => cx.emit(HubEvent::Open { root: root.to_owned(), uri, at }),
                    None => {}
                }
                client.respond(id, Ok(json!({ "success": true })));
            }
            // Tokens are already answered; the rest have no answer here.
            "window/workDoneProgress/create" => {}
            _ => client.respond(id, Ok(Value::Null)),
        }
    }

    /// The workspace applied (or could not apply) an edit the server asked for.
    pub fn reply_apply(&self, root: &Path, id: Value, applied: bool) {
        if let Some(client) = self.servers.get(root).and_then(|s| s.client.clone()) {
            client.respond(id, Ok(json!({ "applied": applied })));
        }
    }

    fn on_exit(&mut self, root: &Path, code: Option<i32>, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root) else { return };
        eprintln!("[java] the engine for {} exited ({code:?})", root.display());
        if server.stopped {
            return;
        }
        server.client = None;
        server.initialized = false;
        server.progress.clear();
        if server.started.elapsed() > Duration::from_secs(300) {
            server.crashes = 0;
        }
        server.crashes += 1;
        self.clear_diagnostics(root, cx);
        let server = self.servers.get_mut(root).expect("still there");
        // It stopped unexpectedly (killed, crashed, or its language server did): start it again
        // at once, then after longer and longer waits, and give up after five times in a row
        // (the status bar then offers Restart). The documents open go to the new one.
        if server.crashes > 5 {
            server.recovering = None;
            server.stage = Stage::Failed("The Java engine keeps stopping".into());
            eprintln!("[java] the engine for {} keeps stopping; not starting it again", root.display());
        } else {
            server.stage = Stage::Restarting;
            server.recovering = Some(server.crashes);
            let delay = if server.crashes == 1 {
                Duration::from_millis(200)
            } else {
                Duration::from_secs(1 << (server.crashes - 1).min(5)).min(Duration::from_secs(30))
            };
            let root = root.to_owned();
            server._tasks.push(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(delay).await;
                let _ = this.update(cx, |this, cx| {
                    if this.servers.get(&root).is_some_and(|s| s.stage == Stage::Restarting) {
                        this.spawn(&root, cx);
                        cx.notify();
                    }
                });
            }));
        }
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// The process ids of the running engines.
    pub fn server_pids(&self) -> Vec<u32> {
        self.servers.values().filter_map(|s| s.client.as_ref().map(Client::pid)).collect()
    }

    /// Stop every server: `shutdown` and `exit`, then a moment for them to go, then
    /// SIGTERM (the engine's own hook stops jdtls).
    pub fn shutdown_all(&mut self) {
        for s in self.servers.values_mut() {
            s.stopped = true;
        }
        let clients: Vec<Client> = self.servers.values_mut().filter_map(|s| s.client.take()).collect();
        for c in &clients {
            drop(c.request_raw("shutdown", Value::Null));
        }
        std::thread::sleep(Duration::from_millis(300));
        for c in &clients {
            c.notify_raw("exit", Value::Null);
        }
        let deadline = Instant::now() + Duration::from_millis(1500);
        while clients.iter().any(Client::is_alive) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        for c in &clients {
            c.kill(Duration::from_millis(500));
        }
    }

    // ----- Documents ---------------------------------------------------------------------

    /// An editor shows the Java file (or Spring Boot configuration file) `path` with
    /// `text`: tell its server (starting it). Returns the document's URI and build, for the
    /// editor's providers.
    pub fn open_doc(
        &mut self,
        editor: &WeakEntity<EditorPanel>,
        path: &Path,
        build: Option<PathBuf>,
        text: Rope,
        cx: &mut Context<Self>,
    ) -> Option<(Uri, PathBuf, Vec<Diagnostic>)> {
        let root = self.root_for(path, build, cx)?;
        let uri = snout_lang::uri::from_path(path);
        let key = uri_key(&uri);
        if let Some(doc) = self.docs.get_mut(&key) {
            doc.editors.retain(|e| e.upgrade().is_some());
            if !doc.editors.iter().any(|e| e.entity_id() == editor.entity_id()) {
                doc.editors.push(editor.clone());
            }
            let diags = self.servers.get(&doc.root).and_then(|s| s.diagnostics.get(&key)).cloned();
            return Some((uri, doc.root.clone(), diags.unwrap_or_default()));
        }
        self.ensure_server(&root, cx);
        let language = language_id(path);
        if let Some(client) = self.client(&root) {
            client.notify_raw(
                "textDocument/didOpen",
                json!({ "textDocument": { "uri": uri.as_str(), "languageId": language, "version": 1, "text": text.to_string() } }),
            );
        }
        self.docs.insert(
            key,
            Doc {
                root: root.clone(),
                uri: uri.clone(),
                text,
                version: 1,
                editors: vec![editor.clone()],
                virtual_doc: false,
                language,
            },
        );
        Some((uri, root, Vec::new()))
    }

    /// An editor shows a class file of the build at `root`.
    pub fn open_virtual(&mut self, editor: &WeakEntity<EditorPanel>, root: &Path, uri: &Uri, text: Rope) {
        self.docs.entry(uri_key(uri)).or_insert_with(|| Doc {
            root: root.to_owned(),
            uri: uri.clone(),
            text,
            version: 1,
            editors: Vec::new(),
            virtual_doc: true,
            language: "java",
        });
        if let Some(doc) = self.docs.get_mut(&uri_key(uri)) {
            doc.editors.push(editor.clone());
        }
    }

    /// Send what changed since the server last saw the document (one ranged change).
    /// Returns the change: the replaced bytes of the text before, and what replaced them.
    pub fn sync(&mut self, uri: &Uri, now: &Rope) -> Option<(std::ops::Range<usize>, String)> {
        let doc = self.docs.get_mut(&uri_key(uri))?;
        if doc.virtual_doc {
            return None;
        }
        let (range, new_text) = text::change(&doc.text, now)?;
        let lsp_range = text::range_to_lsp(&doc.text, &range);
        doc.text = now.clone();
        doc.version += 1;
        let client = self.servers.get(&doc.root).filter(|s| s.ready()).and_then(|s| s.client.clone());
        if let Some(client) = client {
            client.notify_raw(
                "textDocument/didChange",
                json!({
                    "textDocument": { "uri": doc.uri.as_str(), "version": doc.version },
                    "contentChanges": [{ "range": lsp_range, "text": new_text }]
                }),
            );
        }
        Some((range, new_text))
    }

    pub fn saved(&mut self, uri: &Uri) {
        let Some(doc) = self.docs.get(&uri_key(uri)) else { return };
        if let Some(client) = self.client(&doc.root) {
            client.notify_raw("textDocument/didSave", json!({ "textDocument": { "uri": uri.as_str() } }));
        }
    }

    /// An editor of the document closed (or moved on to another file).
    pub fn close_doc(&mut self, uri: &Uri, editor: gpui::EntityId) {
        let key = uri_key(uri);
        let Some(doc) = self.docs.get_mut(&key) else { return };
        doc.editors.retain(|e| e.entity_id() != editor && e.upgrade().is_some());
        if !doc.editors.is_empty() {
            return;
        }
        let doc = self.docs.remove(&key).expect("present");
        if !doc.virtual_doc
            && let Some(client) = self.client(&doc.root)
        {
            client.notify_raw("textDocument/didClose", json!({ "textDocument": { "uri": uri.as_str() } }));
        }
    }

    /// Files that changed on disk outside the editors (an edit applied to a closed file).
    pub fn files_changed(&self, paths: &[PathBuf]) {
        for server in self.servers.values().filter(|s| s.ready()) {
            let changes: Vec<Value> = paths
                .iter()
                .filter(|p| p.starts_with(&server.root))
                .map(|p| json!({ "uri": snout_lang::uri::from_path(p).as_str(), "type": 2 }))
                .collect();
            if !changes.is_empty()
                && let Some(c) = &server.client
            {
                c.notify_raw("workspace/didChangeWatchedFiles", json!({ "changes": changes }));
            }
        }
    }

    /// The language id a document went to its server with.
    pub fn language_of(&self, uri: &Uri) -> Option<&'static str> {
        self.docs.get(&uri_key(uri)).map(|d| d.language)
    }

    /// The diagnostics of one document.
    pub fn diagnostics_of(&self, uri: &Uri) -> Vec<Diagnostic> {
        let key = uri_key(uri);
        self.servers.values().find_map(|s| s.diagnostics.get(&key)).cloned().unwrap_or_default()
    }

    /// The server the status bar speaks of: the one of `path`'s build, else any.
    pub fn server_for(&self, path: Option<&Path>) -> Option<&Server> {
        let by_path = path.and_then(|p| {
            self.servers.values().filter(|s| p.starts_with(&s.root)).max_by_key(|s| s.root.as_os_str().len())
        });
        by_path.or_else(|| if self.servers.len() == 1 { self.servers.values().next() } else { None })
    }

    // ----- The JDK bar -------------------------------------------------------------------

    /// Download the JDK the bar offers, then start the server that waits for it.
    pub fn download_jdk(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.jdk_prompt.as_mut() else { return };
        if prompt.progress.is_some() {
            return;
        }
        let progress = DownloadProgress::default();
        prompt.progress = Some(progress.clone());
        prompt.error = None;
        let (major, root, resync) = (prompt.major, prompt.root.clone(), prompt.resync);
        let dir = java::downloads_dir(self.host.info());
        let host = self.host.clone();
        // The builds without a JDK: those the new one fits are found with it, off the UI thread.
        let without: Vec<PathBuf> = self.servers.values().filter(|s| s.jdk.is_none()).map(|s| s.root.clone()).collect();
        let job = cx.background_executor().spawn({
            let progress = progress.clone();
            async move {
                let jdk = java::download_temurin(&*host, major, &dir, &progress)?;
                let fits: Vec<PathBuf> =
                    without.into_iter().filter(|r| java::requirement(&*host, r).fits(jdk.major)).collect();
                anyhow::Ok((jdk, fits))
            }
        });
        cx.spawn(async move |this, cx| {
            // Repaint the bar as the bytes come in, until the download ends.
            let ticker = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_millis(250)).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            });
            let result = job.await;
            drop(ticker);
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((jdk, fits)) => {
                        eprintln!("[java] downloaded JDK {} to {}", jdk.version, jdk.home.display());
                        this.jdk_prompt = None;
                        // Every build waiting for this version can use it.
                        let waiting: Vec<PathBuf> = fits.into_iter().chain(std::iter::once(root.clone())).collect();
                        for r in waiting {
                            if let Some(s) = this.servers.get_mut(&r).filter(|s| s.client.is_none()) {
                                s.jdk = Some(jdk.clone());
                                this.remember_jdk(&r, &jdk, cx);
                                this.spawn(&r, cx);
                            }
                        }
                        // A build of the model wanted it: sync the model again, so it can.
                        if resync && let Some(store) = crate::project_model::store(cx) {
                            store.update(cx, |s, cx| s.sync(cx));
                        }
                    }
                    Err(e) => {
                        if let Some(p) = this.jdk_prompt.as_mut() {
                            p.progress = None;
                            p.error = Some(format!("{e:#}"));
                        }
                    }
                }
                cx.emit(HubEvent::Changed);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn dismiss_jdk(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.jdk_prompt.as_ref().filter(|p| p.progress.is_none()) {
            self.dismissed_jdks.push(p.major);
            self.jdk_prompt = None;
            cx.notify();
        }
    }

    /// A build of the Gradle model at `root` failed for want of JDK `major`: offer it, unless
    /// it was turned down or is here already.
    pub fn offer_jdk(&mut self, root: &Path, major: u32, cx: &mut Context<Self>) {
        if self.jdk_prompt.is_some() || self.dismissed_jdks.contains(&major) {
            return;
        }
        // Whether it was downloaded already is asked of the host off the UI thread.
        let host = self.host.clone();
        let have = cx
            .background_executor()
            .spawn(async move { java::downloaded_jdks(&*host).iter().any(|j| j.major == major) });
        let root = root.to_owned();
        cx.spawn(async move |this, cx| {
            if have.await {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                if this.jdk_prompt.is_some() || this.dismissed_jdks.contains(&major) {
                    return;
                }
                eprintln!("[java] a build of {} needs JDK {major}: offering it", root.display());
                this.jdk_prompt = Some(JdkPrompt { root, major, resync: true, progress: None, error: None });
                cx.emit(HubEvent::Changed);
                cx.notify();
            });
        })
        .detach();
    }

    // ----- Requests the editors and the workspace make --------------------------------

    /// Ask the server of `root`; fails when it is not ready.
    pub fn request(
        &self,
        root: &Path,
        method: &str,
        params: Value,
    ) -> impl std::future::Future<Output = anyhow::Result<Value>> + Send + 'static + use<> {
        let call = self.client(root).map(|c| c.request_raw(method, params));
        let method = method.to_owned();
        async move {
            match call {
                Some(call) => call.await.map_err(|e| anyhow::anyhow!("{method}: {e}")),
                None => Err(anyhow::anyhow!("the Java server is not ready")),
            }
        }
    }

    /// Find the references of the symbol at `at` in `uri`, for the References panel.
    pub fn find_references(&mut self, uri: Uri, at: Position, name: String, cx: &mut Context<Self>) {
        let Some(root) = self.root_of(&uri) else { return };
        let call = self.request(
            &root,
            "textDocument/references",
            json!({ "textDocument": { "uri": uri.as_str() }, "position": at, "context": { "includeDeclaration": true } }),
        );
        cx.spawn(async move |this, cx| {
            let locations: Vec<Location> = match call.await {
                Ok(v) => serde_json::from_value(v).unwrap_or_default(),
                Err(e) => {
                    eprintln!("[java] {e}");
                    return;
                }
            };
            let title = format!("References to {name}");
            let _ = this.update(cx, |_, cx| cx.emit(HubEvent::References { title, locations }));
        })
        .detach();
    }

    /// Ask the server to run `command` (from a code action); edits come back as requests.
    pub fn execute_command(&mut self, uri: &Uri, command: lsp_types::Command, cx: &mut Context<Self>) {
        // jdtls hands some edits to the client to apply itself.
        if command.command == "java.apply.workspaceEdit"
            && let Some(edit) = command
                .arguments
                .as_ref()
                .and_then(|a| a.first())
                .and_then(|v| serde_json::from_value::<WorkspaceEdit>(v.clone()).ok())
        {
            cx.emit(HubEvent::ApplyEdit { edit, reply: None });
            return;
        }
        let Some(root) = self.root_of(uri) else { return };
        let call = self.request(
            &root,
            "workspace/executeCommand",
            json!({ "command": command.command, "arguments": command.arguments.unwrap_or_default() }),
        );
        cx.background_executor()
            .spawn(async move {
                if let Err(e) = call.await {
                    eprintln!("[java] {e}");
                }
            })
            .detach();
    }

    /// The text of a class file (`jdt://` URI), its source when a source jar has it.
    pub fn class_file(
        &self,
        root: &Path,
        uri: &Uri,
    ) -> impl std::future::Future<Output = anyhow::Result<String>> + use<> {
        let call = self.request(root, "java/classFileContents", json!({ "uri": uri.as_str() }));
        async move { Ok(call.await?.as_str().unwrap_or_default().to_owned()) }
    }

    /// Every diagnostic of every server and of the last compile, information and hints too,
    /// with the build each came from (none for the compile's), in no order.
    pub fn all_diagnostics(&self) -> Vec<(String, Diagnostic, Option<PathBuf>)> {
        self.servers
            .values()
            .flat_map(|s| s.diagnostics.iter().map(move |(uri, list)| (s.root.clone(), uri, list)))
            .flat_map(|(root, uri, list)| list.iter().map(move |d| (uri.clone(), d.clone(), Some(root.clone()))))
            .chain(self.build_problems.iter().map(|(u, d)| (u.clone(), d.clone(), None)))
            .collect()
    }

    /// The build whose engine reports on `uri` (or has it open), else any.
    pub fn engine_root_for(&self, uri: &Uri) -> Option<PathBuf> {
        let key = uri_key(uri);
        self.servers
            .values()
            .find(|s| s.diagnostics.contains_key(&key))
            .map(|s| s.root.clone())
            .or_else(|| self.root_of(uri))
            .or_else(|| self.servers.keys().next().cloned())
    }

    /// Every diagnostic of every server, worst first then by file and line.
    pub fn all_problems(&self) -> Vec<(String, Diagnostic)> {
        let mut out: Vec<(String, Diagnostic)> = self
            .servers
            .values()
            .flat_map(|s| s.diagnostics.iter())
            .flat_map(|(uri, list)| list.iter().map(move |d| (uri.clone(), d.clone())))
            .chain(self.build_problems.iter().cloned())
            .filter(|(_, d)| {
                matches!(
                    d.severity,
                    Some(lsp_types::DiagnosticSeverity::ERROR | lsp_types::DiagnosticSeverity::WARNING) | None
                )
            })
            .collect();
        out.sort_by(|a, b| {
            let sev = |d: &Diagnostic| {
                d.severity.map_or(1, |s| if s == lsp_types::DiagnosticSeverity::ERROR { 0 } else { 1 })
            };
            (sev(&a.1), &a.0, a.1.range.start.line).cmp(&(sev(&b.1), &b.0, b.1.range.start.line))
        });
        out
    }
}

/// The language id a document goes to the engine with: a Spring Boot configuration file
/// (`application.properties`, `application-dev.yml`), which the engine serves itself, or Java.
pub fn language_id(path: &Path) -> &'static str {
    if crate::spring::is_config_file(path) { crate::spring::config_language_id(path) } else { "java" }
}

/// `initialize` for jdtls: the editor's capabilities, the folder, and jdtls's settings.
fn initialize_params(root: &Path) -> Value {
    let uri = snout_lang::uri::from_path(root);
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    json!({
        "processId": std::process::id(),
        "clientInfo": { "name": "SnoutCode" },
        "rootUri": uri.as_str(),
        "rootPath": root,
        "workspaceFolders": [{ "uri": uri.as_str(), "name": name }],
        "capabilities": snout_lang::client_capabilities(),
        "initializationOptions": {
            "settings": { "java": {
                "autobuild": { "enabled": true },
                "format": { "enabled": true },
                "signatureHelp": { "enabled": true },
                // Inlay hints: the names of the parameters literal arguments are passed to,
                // and the engine's Spring `@Value` values. The editor draws them at the end of
                // their lines (a setting turns the parameter names off).
                "inlayHints": { "parameterNames": { "enabled": "literals" } },
                "completion": { "enabled": true, "guessMethodArguments": false },
                "configuration": { "updateBuildConfiguration": "disabled" }
            } },
            "extendedClientCapabilities": {
                "classFileContentsSupport": true,
                "progressReportProvider": false,
                "advancedOrganizeImportsSupport": false
            }
        }
    })
}

#[cfg(test)]
impl LangHub {
    /// Take a notification as if the server of `root` had sent it.
    pub fn engine_says(&mut self, root: &Path, method: &str, params: Value, cx: &mut Context<Self>) {
        self.on_notification(root, method, params, cx);
    }

    /// Hold `list` as what a server of `root` published for `uri`, without running one
    /// (tests of what reads the diagnostics).
    pub fn insert_published(&mut self, root: &Path, uri: String, list: Vec<Diagnostic>) {
        let server = self.servers.entry(root.to_owned()).or_insert_with(|| Server {
            root: root.to_owned(),
            jdk: None,
            client: None,
            stage: Stage::Ready,
            progress: Progress::default(),
            initialized: false,
            diagnostics: BTreeMap::new(),
            crashes: 0,
            recovering: None,
            started: Instant::now(),
            first_diagnostics: None,
            generation: 0,
            stopped: true,
            readiness: Readiness::default(),
            _tasks: Vec::new(),
        });
        server.diagnostics.insert(uri, list);
    }
}
