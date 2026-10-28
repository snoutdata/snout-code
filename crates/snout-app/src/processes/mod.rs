//! The processes SnoutCode runs for the window: the Java engine of the open project, the
//! Java language server it starts, the engine's own Gradle daemons and what they start (test
//! JVMs, workers, programs), the programs run and debugged from the editor, the terminals, and
//! anything an earlier session left behind. [`Processes`] looks every few seconds; the
//! Processes tool window shows it, and its actions stop and restart them.
//!
//! At startup it stops what an earlier SnoutCode left running when it crashed: engines and
//! language servers whose owner is gone, and the Gradle daemons of the engine's registries that
//! no running engine owns. Processes SnoutCode did not start (a Gradle daemon started from a
//! terminal, another app's engines) are never touched.

pub mod scan;
pub mod window;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{App, Context, Entity, Global, Task};

use crate::lang::LangHub;
use crate::model::Model;
use crate::run::Runs;
use scan::{Proc, Snapshot};

/// How often the processes are looked at.
pub const EVERY: Duration = Duration::from_secs(3);

/// What a process is for.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// SnoutCode itself.
    App,
    Engine {
        root: PathBuf,
    },
    LanguageServer {
        root: PathBuf,
    },
    Daemon {
        version: String,
    },
    /// A program run or debugged from the editor, and its session when known.
    Run {
        session: Option<u64>,
    },
    /// A test JVM Gradle started, and the test run's session when known.
    Tests {
        session: Option<u64>,
    },
    /// A program or worker Gradle started.
    GradleChild {
        session: Option<u64>,
    },
    Terminal,
    Claude,
    Other,
    /// Left running by an earlier session.
    Leftover,
}

/// What can be done to a process from the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Stop,
    Restart,
}

/// One line of the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub pid: Option<u32>,
    /// When it started (seconds since 1970), to tell it from a later process with its pid.
    pub start: u64,
    pub depth: usize,
    pub kind: Kind,
    /// What it is for, in plain words: "Java language server for shop: completion, errors,
    /// navigation".
    pub purpose: String,
    pub project: Option<String>,
    /// Resident memory, in bytes.
    pub memory: u64,
    /// Percent of one core.
    pub cpu: f32,
    /// Seconds it has run.
    pub uptime: u64,
    pub state: String,
    pub busy: bool,
    /// Stop Idle Processes stops it.
    pub idle: bool,
}

impl Row {
    /// What the window offers for it.
    pub fn actions(&self) -> Vec<Act> {
        match &self.kind {
            Kind::Engine { .. } if self.pid.is_some() && self.state != "Stopping" => vec![Act::Restart, Act::Stop],
            Kind::Engine { .. } if self.pid.is_none() => vec![Act::Restart],
            Kind::LanguageServer { .. } => vec![Act::Restart],
            Kind::Run { session: Some(_) } => vec![Act::Restart, Act::Stop],
            Kind::Daemon { .. } | Kind::Run { .. } | Kind::Tests { .. } | Kind::GradleChild { .. } | Kind::Leftover => {
                vec![Act::Stop]
            }
            _ => Vec::new(),
        }
    }

    /// A key that stays the same while the process runs.
    pub fn key(&self) -> String {
        match (&self.kind, self.pid) {
            (_, Some(pid)) => format!("{pid}"),
            (Kind::Engine { root }, None) => format!("engine:{}", root.display()),
            _ => self.purpose.clone(),
        }
    }
}

/// What the app knows of its processes, beside what the system says.
#[derive(Clone, Debug, Default)]
pub struct Known {
    /// SnoutCode's own pid.
    pub app: u32,
    /// The engine's cache folder (its daemon registries are under it).
    pub cache: PathBuf,
    /// The open project.
    pub project: Option<PathBuf>,
    pub engines: Vec<KnownEngine>,
    pub sessions: Vec<KnownSession>,
    /// How long the engine's daemons may idle, in minutes.
    pub idle_minutes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct KnownEngine {
    pub root: PathBuf,
    pub pid: Option<u32>,
    pub state: String,
    pub busy: bool,
}

/// A run, test run or debug session that runs now.
#[derive(Clone, Debug, Default)]
pub struct KnownSession {
    pub id: u64,
    pub title: String,
    /// The program's pid, when the engine started it itself (a main class).
    pub pid: Option<u32>,
    pub debug: bool,
    pub tests: bool,
}

fn dir_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

const SHELLS: [&str; 9] = ["zsh", "bash", "fish", "sh", "nu", "tcsh", "csh", "ksh", "dash"];

/// The window's lines, from a snapshot and what the app knows: SnoutCode, under it its engines
/// (each with its language server, its programs and its Gradle daemons with what they started),
/// the other processes it started, then what earlier sessions left.
pub fn rows(snap: &Snapshot, known: &Known) -> Vec<Row> {
    let mut out = Vec::new();
    let row = |p: &Proc, depth: usize, kind: Kind, purpose: String, project: Option<String>| Row {
        pid: Some(p.pid),
        start: p.start,
        depth,
        kind,
        purpose,
        project,
        memory: p.memory,
        cpu: p.cpu,
        uptime: snap.now.saturating_sub(p.start),
        state: "Running".into(),
        busy: false,
        idle: false,
    };
    if let Some(app) = snap.get(known.app) {
        let mut r = row(app, 0, Kind::App, "SnoutCode: the window, the editors and Git".into(), None);
        r.state = "Running".into();
        out.push(r);
    }
    let cache = known.cache.to_string_lossy().into_owned();
    let ours = |p: &Proc| p.arg_after("--cache-dir") == Some(cache.as_str());
    let session_by_pid = |pid: u32| known.sessions.iter().find(|s| s.pid == Some(pid));
    // What a Gradle daemon runs: the latest test run, or the latest run through Gradle.
    let gradle_session = |tests: bool| {
        known.sessions.iter().filter(|s| s.pid.is_none() && s.tests == tests).max_by_key(|s| s.id).or_else(|| {
            if tests { None } else { known.sessions.iter().filter(|s| s.pid.is_none()).max_by_key(|s| s.id) }
        })
    };
    // The registries of the app's engines, and of any other engine of this cache that runs.
    let mut owners: Vec<(String, Option<&KnownEngine>, Option<&Proc>)> =
        known.engines.iter().map(|e| (scan::registry_hash(&e.root), Some(e), None)).collect();
    for p in snap.procs.values().filter(|p| p.is_engine() && ours(p)) {
        if known.engines.iter().any(|e| e.pid == Some(p.pid)) {
            continue;
        }
        if let Some(folder) = p.engine_folder() {
            owners.push((scan::registry_hash(Path::new(folder)), None, Some(p)));
        }
    }
    let idle_text = |since: Option<u64>| {
        let limit = known.idle_minutes;
        match since.map(|s| snap.now.saturating_sub(s) / 60) {
            Some(m) if m >= 1 => format!("Idle for {m} min, stops after {limit}"),
            _ => format!("Idle, stops after {limit} min"),
        }
    };

    let mut shown: Vec<u32> = Vec::new();
    for engine in &known.engines {
        let project = dir_name(&engine.root);
        let alive = engine.pid.and_then(|pid| snap.get(pid));
        let mut r = match alive {
            Some(p) => row(
                p,
                1,
                Kind::Engine { root: engine.root.clone() },
                format!("Java engine for {project}: reads the Gradle build, runs and tests the code"),
                Some(project.clone()),
            ),
            None => Row {
                pid: None,
                start: 0,
                depth: 1,
                kind: Kind::Engine { root: engine.root.clone() },
                purpose: format!("Java engine for {project}: reads the Gradle build, runs and tests the code"),
                project: Some(project.clone()),
                memory: 0,
                cpu: 0.,
                uptime: 0,
                state: String::new(),
                busy: false,
                idle: false,
            },
        };
        r.state = engine.state.clone();
        r.busy = engine.busy;
        let outside = known.project.as_ref().is_none_or(|p| !engine.root.starts_with(p));
        r.idle = outside && alive.is_some();
        out.push(r);
        let Some(p) = alive else { continue };
        shown.push(p.pid);
        for child in snap.children(p.pid) {
            // Its Gradle daemons stay its children until it ends: they show below, by registry.
            if child.is_daemon() {
                continue;
            }
            shown.push(child.pid);
            if child.is_jdtls() {
                let mut r = row(
                    child,
                    2,
                    Kind::LanguageServer { root: engine.root.clone() },
                    format!("Java language server for {project}: completion, errors, navigation"),
                    Some(project.clone()),
                );
                r.state = engine.state.clone();
                r.busy = engine.busy;
                out.push(r);
            } else {
                let session = session_by_pid(child.pid);
                let purpose = match session {
                    Some(s) if s.debug => format!("{}: your program, being debugged", s.title),
                    Some(s) => format!("{}: your program, started with Run", s.title),
                    None => format!("Program started by the Java engine for {project}"),
                };
                let mut r = row(child, 2, Kind::Run { session: session.map(|s| s.id) }, purpose, Some(project.clone()));
                r.state = if session.is_some_and(|s| s.debug) { "Debugging" } else { "Running" }.into();
                r.busy = true;
                out.push(r);
            }
        }
        let hash = scan::registry_hash(&engine.root);
        for d in snap.daemons.iter().filter(|d| d.registry == hash) {
            let Some(dp) = snap.get(d.pid) else { continue };
            shown.push(d.pid);
            let mut r = row(
                dp,
                2,
                Kind::Daemon { version: d.version.clone() },
                format!("Gradle daemon {} for {project}: syncs the build, runs tasks and tests", d.version),
                Some(project.clone()),
            );
            r.busy = d.busy;
            r.idle = !d.busy;
            r.state = if d.busy { "Building".into() } else { idle_text(d.since) };
            out.push(r);
            for child in snap.children(d.pid) {
                shown.push(child.pid);
                let line = child.line();
                let (kind, purpose) = if line.contains("Gradle Test Executor") {
                    let s = gradle_session(true);
                    let purpose = match s {
                        Some(s) => format!("Tests of {}: the test JVM Gradle started", s.title),
                        None => format!("Tests for {project}: the test JVM Gradle started"),
                    };
                    (Kind::Tests { session: s.map(|s| s.id) }, purpose)
                } else if line.contains("GradleWorkerMain") || line.contains("worker.org.gradle.process") {
                    (
                        Kind::GradleChild { session: None },
                        format!("Gradle worker for {project}: compiles or processes in a JVM of its own"),
                    )
                } else {
                    let s = gradle_session(false);
                    let purpose = match s {
                        Some(s) => format!("{}: your program, started by Gradle", s.title),
                        None => format!("Program Gradle started for {project}"),
                    };
                    (Kind::GradleChild { session: s.map(|s| s.id) }, purpose)
                };
                let mut r = row(child, 3, kind, purpose, Some(project.clone()));
                r.busy = true;
                out.push(r);
            }
        }
    }

    // The rest of what SnoutCode started: terminals, Claude Code, Git, engines it let go.
    let project = known.project.as_deref().map(dir_name);
    for child in snap.children(known.app) {
        if shown.contains(&child.pid) {
            continue;
        }
        let name = child.name.rsplit('/').next().unwrap_or(&child.name).trim_start_matches('-').to_owned();
        let within = project.as_deref().map(|p| format!(" in {p}")).unwrap_or_default();
        let (kind, purpose, state) = if child.is_engine() {
            let folder = child.engine_folder().map(|f| dir_name(Path::new(f))).unwrap_or_default();
            (
                Kind::Other,
                format!("Java engine for {folder}: stopping, with its language server and Gradle daemons"),
                "Stopping",
            )
        } else if SHELLS.contains(&name.as_str()) {
            (Kind::Terminal, format!("Terminal: your {name} shell{within}"), "Running")
        } else if name.contains("claude") || child.cmd.iter().take(2).any(|a| a.contains("claude")) {
            (Kind::Claude, "Claude Code: the assistant in the console".into(), "Running")
        } else if name == "git" {
            let what = child.cmd.iter().skip(1).find(|a| !a.starts_with('-')).cloned().unwrap_or_default();
            (Kind::Other, format!("Git: {what}{within}").replace(":  ", ": "), "Running")
        } else {
            (Kind::Other, format!("{name}: started by SnoutCode"), "Running")
        };
        let mut r = row(child, 1, kind, purpose, project.clone());
        r.state = state.into();
        out.push(r);
    }

    // What an earlier session left.
    let left = |p: &Proc, purpose: String, state: String| {
        let mut r = row(p, 0, Kind::Leftover, purpose, None);
        r.state = state;
        r.idle = true;
        r
    };
    let mut leftovers = Vec::new();
    let leftover_engines: Vec<&Proc> = snap
        .procs
        .values()
        .filter(|p| p.is_engine() && ours(p) && snap.orphan(p) && !known.engines.iter().any(|e| e.pid == Some(p.pid)))
        .collect();
    for p in &leftover_engines {
        let folder = p.engine_folder().map(|f| dir_name(Path::new(f))).unwrap_or_default();
        leftovers.push(left(
            p,
            format!("Java engine for {folder}, left over from an earlier session"),
            "Left over".into(),
        ));
    }
    let model_prefix = format!("{}{}", scan::JDTLS_MARK, cache);
    for p in snap.procs.values().filter(|p| p.has_prefix(&model_prefix)) {
        let engine = p.parent.and_then(|pid| snap.get(pid)).filter(|e| e.is_engine() && !snap.orphan(e));
        if engine.is_some() || shown.contains(&p.pid) {
            continue;
        }
        leftovers.push(left(p, "Java language server, left over from an earlier session".into(), "Left over".into()));
    }
    for d in &snap.daemons {
        if shown.contains(&d.pid) {
            continue;
        }
        let owner = owners.iter().find(|(h, _, _)| *h == d.registry);
        let leftover_owner = owner.and_then(|(_, _, p)| *p).is_some_and(|p| snap.orphan(p));
        if owner.is_some() && !leftover_owner {
            // Another engine's (another window, or a sync from the command line).
            continue;
        }
        let Some(p) = snap.get(d.pid) else { continue };
        let state = if d.busy { "Building".to_owned() } else { "Left over".to_owned() };
        leftovers.push(left(p, format!("Gradle daemon {}, left over from an earlier session", d.version), state));
    }
    leftovers.sort_by_key(|r| r.start);
    out.extend(leftovers);
    out
}

/// The totals the window and the status bar show: processes, memory, CPU.
pub fn totals(rows: &[Row]) -> (usize, u64, f32) {
    let live = rows.iter().filter(|r| r.pid.is_some());
    live.fold((0, 0, 0.), |(n, m, c), r| (n + 1, m + r.memory, c + r.cpu))
}

/// "1.4 GB", "312 MB".
pub fn bytes(n: u64) -> String {
    let mb = n as f64 / (1024. * 1024.);
    if mb >= 1024. { format!("{:.1} GB", mb / 1024.) } else { format!("{mb:.0} MB") }
}

/// "3 h 12 min", "4 min", "12 s".
pub fn duration(secs: u64) -> String {
    match secs {
        s if s >= 86_400 => format!("{} d {} h", s / 86_400, s % 86_400 / 3600),
        s if s >= 3600 => format!("{} h {} min", s / 3600, s % 3600 / 60),
        s if s >= 60 => format!("{} min", s / 60),
        s => format!("{s} s"),
    }
}

/// How long the engine's Gradle daemons idle before they stop, unless the settings say.
pub const DEFAULT_IDLE_MINUTES: u64 = 15;

/// SnoutCode's processes, looked at every few seconds.
pub struct Processes {
    model: Entity<Model>,
    hub: Entity<LangHub>,
    runs: Entity<Runs>,
    pub rows: Vec<Row>,
    /// What the last action did, for the window's foot.
    pub notice: Option<String>,
    scanner: Arc<Mutex<scan::Scanner>>,
    looking: bool,
    _loop: Option<Task<()>>,
}

pub struct ProcessesHandle(pub Entity<Processes>);

impl Global for ProcessesHandle {}

pub fn processes(cx: &App) -> Option<Entity<Processes>> {
    cx.try_global::<ProcessesHandle>().map(|h| h.0.clone())
}

impl Processes {
    /// Looks only when the app runs servers (`hub.enabled`); tests set the rows themselves.
    pub fn new(model: Entity<Model>, hub: Entity<LangHub>, runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        // Never in the unit tests: they run engines of their own, under the user's cache.
        let enabled = hub.read(cx).enabled && !cfg!(test);
        let mut this = Self {
            model,
            hub,
            runs,
            rows: Vec::new(),
            notice: None,
            scanner: Arc::new(Mutex::new(scan::Scanner::new())),
            looking: false,
            _loop: None,
        };
        if enabled {
            this.stop_leftovers(cx);
            this._loop = Some(cx.spawn(async move |this, cx| {
                loop {
                    if this.update(cx, |this, cx| this.look(cx)).is_err() {
                        return;
                    }
                    cx.background_executor().timer(EVERY).await;
                }
            }));
        }
        this
    }

    /// Put these rows in place of what the system says (tests and screenshots).
    pub fn set_rows(&mut self, rows: Vec<Row>, cx: &mut Context<Self>) {
        self.rows = rows;
        cx.notify();
    }

    /// What the app knows now.
    pub fn known(&self, cx: &App) -> Known {
        let hub = self.hub.read(cx);
        let mut engines: Vec<KnownEngine> = hub
            .servers
            .values()
            .map(|s| KnownEngine { root: s.root.clone(), pid: s.pid(), state: s.stage.label(), busy: s.busy() })
            .collect();
        engines.sort_by(|a, b| a.root.cmp(&b.root));
        let sessions = self
            .runs
            .read(cx)
            .sessions
            .iter()
            .filter(|s| s.running())
            .map(|s| KnownSession {
                id: s.id,
                title: s.title.clone(),
                pid: s.pid,
                debug: s.launch.debug(),
                tests: s.tests.is_some(),
            })
            .collect();
        let model = self.model.read(cx);
        Known {
            app: std::process::id(),
            cache: crate::lang::java::engine_cache(&*crate::host::host(cx)),
            project: model.project_root(),
            engines,
            sessions,
            idle_minutes: model.settings.daemon_idle_minutes,
        }
    }

    /// Look at the processes again, off the main thread.
    pub fn look(&mut self, cx: &mut Context<Self>) {
        if self.looking {
            return;
        }
        self.looking = true;
        let known = self.known(cx);
        let scanner = self.scanner.clone();
        let job = cx.background_executor().spawn(async move {
            let snap = scanner.lock().map(|mut s| s.snapshot(&known.cache)).unwrap_or_default();
            rows(&snap, &known)
        });
        cx.spawn(async move |this, cx| {
            let rows = job.await;
            let _ = this.update(cx, |this, cx| {
                this.looking = false;
                if this.rows != rows {
                    this.rows = rows;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Stop what earlier sessions left running (once, as the app starts).
    fn stop_leftovers(&mut self, cx: &mut Context<Self>) {
        let mut known = self.known(cx);
        // Nothing is the app's own yet: what runs of this cache with an owner is another
        // window's, and is left alone.
        known.engines.clear();
        cx.background_executor()
            .spawn(async move {
                let snap = scan::Scanner::new().quick_snapshot(&known.cache);
                let targets: Vec<(u32, u64, String)> = rows(&snap, &known)
                    .into_iter()
                    .filter(|r| r.kind == Kind::Leftover)
                    .filter_map(|r| r.pid.map(|p| (p, r.start, r.purpose)))
                    .collect();
                if targets.is_empty() {
                    return;
                }
                for (pid, _, what) in &targets {
                    eprintln!("[processes] stopping pid {pid}: {what}");
                }
                let pids: Vec<(u32, u64)> = targets.iter().map(|(p, s, _)| (*p, *s)).collect();
                let n = scan::stop(&pids, Duration::from_secs(3));
                eprintln!("[processes] stopped {n} process(es) an earlier session left");
            })
            .detach();
    }

    /// Stop or restart the process of `row`.
    pub fn act(&mut self, row: &Row, act: Act, cx: &mut Context<Self>) {
        match (&row.kind, act) {
            (Kind::Engine { root } | Kind::LanguageServer { root }, Act::Restart) => {
                let root = root.clone();
                self.hub.update(cx, |h, cx| h.restart(&root, cx));
            }
            (Kind::Engine { root }, Act::Stop) => {
                let root = root.clone();
                self.hub.update(cx, |h, cx| h.stop_engine(&root, cx));
            }
            (Kind::Run { session: Some(id) }, Act::Restart) => {
                let id = *id;
                self.runs.update(cx, |r, cx| r.rerun(id, cx));
            }
            (
                Kind::Run { session: Some(id) }
                | Kind::Tests { session: Some(id) }
                | Kind::GradleChild { session: Some(id) },
                Act::Stop,
            ) => {
                let id = *id;
                self.runs.update(cx, |r, cx| r.stop(id, cx));
            }
            (_, Act::Stop) => {
                if let Some(pid) = row.pid {
                    self.kill(vec![(pid, row.start)], cx);
                }
            }
            _ => {}
        }
        self.look_soon(cx);
    }

    /// Stop the idle Gradle daemons, what earlier sessions left, and the engines of projects
    /// not open.
    pub fn stop_idle(&mut self, cx: &mut Context<Self>) -> usize {
        let idle: Vec<Row> = self.rows.iter().filter(|r| r.idle && r.pid.is_some()).cloned().collect();
        let mut kill = Vec::new();
        for r in &idle {
            match &r.kind {
                Kind::Engine { root } => {
                    let root = root.clone();
                    self.hub.update(cx, |h, _| h.stop_server(&root));
                }
                _ => kill.extend(r.pid.map(|p| (p, r.start))),
            }
        }
        let n = idle.len();
        self.notice = Some(match n {
            0 => "Nothing idle to stop.".to_owned(),
            1 => "Stopped 1 idle process.".to_owned(),
            n => format!("Stopped {n} idle processes."),
        });
        self.kill(kill, cx);
        self.look_soon(cx);
        cx.notify();
        n
    }

    fn kill(&mut self, targets: Vec<(u32, u64)>, cx: &mut Context<Self>) {
        if targets.is_empty() {
            return;
        }
        let job = cx.background_executor().spawn(async move { scan::stop(&targets, Duration::from_secs(3)) });
        cx.spawn(async move |this, cx| {
            job.await;
            let _ = this.update(cx, |this, cx| this.look(cx));
        })
        .detach();
    }

    /// Look again in a moment (after an action took effect).
    fn look_soon(&mut self, cx: &mut Context<Self>) {
        if !self.hub.read(cx).enabled {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(600)).await;
            let _ = this.update(cx, |this, cx| this.look(cx));
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::scan::{DaemonSeen, ENGINE_MAIN, Proc, Snapshot};
    use super::*;

    fn proc(pid: u32, parent: u32, name: &str, cmd: &[&str]) -> Proc {
        Proc {
            pid,
            parent: Some(parent),
            name: name.into(),
            cmd: cmd.iter().map(|s| (*s).to_owned()).collect(),
            memory: 100 << 20,
            cpu: 1.5,
            start: 1000 + u64::from(pid),
        }
    }

    #[test]
    fn each_process_says_what_it_is_for() {
        let root = std::env::temp_dir().join(format!("snout-procs-{}", std::process::id())).join("shop");
        std::fs::create_dir_all(&root).unwrap();
        let cache = root.parent().unwrap().join("cache");
        let cache_s = cache.to_string_lossy().into_owned();
        let root_s = root.to_string_lossy().into_owned();
        let hash = scan::registry_hash(&root);
        let model = format!("-Dsnout.model={cache_s}/jdtls/x/model.json");
        let mut procs = vec![
            proc(10, 1, "snout-code", &["snout-code"]),
            proc(20, 10, "java", &["java", "-cp", "x", ENGINE_MAIN, "lsp", &root_s, "--cache-dir", &cache_s]),
            proc(21, 20, "java", &["java", &model, "-jar", "launcher.jar", "-data", "ws"]),
            proc(22, 20, "java", &["java", "-cp", "app", "demo.Main"]),
            proc(30, 20, "java", &["java", "org.gradle.launcher.daemon.bootstrap.GradleDaemon", "8.5"]),
            proc(
                31,
                30,
                "java",
                &["java", "worker.org.gradle.process.internal.worker.GradleWorkerMain", "'Gradle Test Executor 1'"],
            ),
            proc(40, 10, "zsh", &["-zsh"]),
            // Left over: an engine of this cache whose app is gone, its jdtls, and a daemon of
            // a registry nobody owns.
            proc(50, 1, "java", &["java", ENGINE_MAIN, "lsp", "/gone", "--cache-dir", &cache_s]),
            proc(51, 1, "java", &["java", &model, "-data", "ws2"]),
            proc(60, 1, "java", &["java", "org.gradle.launcher.daemon.bootstrap.GradleDaemon", "8.1.1"]),
            // The user's own daemon (not in a registry of the cache) and another app's engine.
            proc(70, 1, "java", &["java", "org.gradle.launcher.daemon.bootstrap.GradleDaemon", "8.5"]),
            proc(80, 81, "java", &["java", ENGINE_MAIN, "lsp", "/other", "--cache-dir", &cache_s]),
            proc(81, 1, "snout-code", &["snout-code"]),
        ];
        procs.iter_mut().for_each(|p| p.memory = 1 << 20);
        let snap = Snapshot {
            procs: procs.into_iter().map(|p| (p.pid, p)).collect(),
            daemons: vec![
                DaemonSeen { pid: 30, registry: hash.clone(), version: "8.5".into(), busy: false, since: Some(1000) },
                DaemonSeen {
                    pid: 60,
                    registry: "0000000000000000".into(),
                    version: "8.1.1".into(),
                    busy: false,
                    since: None,
                },
            ],
            now: 1000 + 600,
        };
        let known = Known {
            app: 10,
            cache: cache.clone(),
            project: Some(root.clone()),
            engines: vec![KnownEngine { root: root.clone(), pid: Some(20), state: "Ready".into(), busy: false }],
            sessions: vec![
                KnownSession { id: 1, title: "Main".into(), pid: Some(22), debug: false, tests: false },
                KnownSession { id: 2, title: "ShopTest".into(), pid: None, debug: false, tests: true },
            ],
            idle_minutes: 15,
        };
        let rows = rows(&snap, &known);
        let said: Vec<(usize, &str)> = rows.iter().map(|r| (r.depth, r.purpose.as_str())).collect();
        assert_eq!(
            said,
            vec![
                (0, "SnoutCode: the window, the editors and Git"),
                (1, "Java engine for shop: reads the Gradle build, runs and tests the code"),
                (2, "Java language server for shop: completion, errors, navigation"),
                (2, "Main: your program, started with Run"),
                (2, "Gradle daemon 8.5 for shop: syncs the build, runs tasks and tests"),
                (3, "Tests of ShopTest: the test JVM Gradle started"),
                (1, "Terminal: your zsh shell in shop"),
                (0, "Java engine for gone, left over from an earlier session"),
                (0, "Java language server, left over from an earlier session"),
                (0, "Gradle daemon 8.1.1, left over from an earlier session"),
            ]
        );
        let daemon = rows.iter().find(|r| matches!(r.kind, Kind::Daemon { .. })).unwrap();
        assert!(daemon.idle);
        assert_eq!(daemon.state, "Idle for 10 min, stops after 15");
        assert_eq!(daemon.actions(), vec![Act::Stop]);
        let engine = rows.iter().find(|r| matches!(r.kind, Kind::Engine { .. })).unwrap();
        assert_eq!(engine.actions(), vec![Act::Restart, Act::Stop]);
        assert!(!engine.idle, "the open project's engine is not idle");
        let (n, memory, _) = totals(&rows);
        assert_eq!((n, memory), (10, 10 << 20));
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn sizes_and_times_read_well() {
        assert_eq!(bytes(312 << 20), "312 MB");
        assert_eq!(bytes(1536 << 20), "1.5 GB");
        assert_eq!(duration(42), "42 s");
        assert_eq!(duration(4 * 60 + 2), "4 min");
        assert_eq!(duration(3 * 3600 + 12 * 60), "3 h 12 min");
    }
}
