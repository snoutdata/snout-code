//! What a Playwright project needs before its tests can run: its packages installed
//! (`node_modules` holds the test runner) and the browsers of that Playwright version
//! downloaded. When either is missing, a bar above the editor offers to install it; the
//! install runs in the Build window, where its output shows, and the bar looks again when
//! it ends.

use std::path::{Path, PathBuf};

use gpui::{Context, Entity, EventEmitter, Global, Subscription, Task};
use serde_json::Value;
use snout_host::Host;

use crate::lang::node::{self, Node, PackageManager};
use crate::model::Model;
use crate::run::process::ProcessLaunch;
use crate::run::{Launch, Place, Runs, RunsEvent};

/// What the project lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    /// `node_modules` does not hold the test runner yet.
    Packages { manager: PackageManager },
    /// The browsers this Playwright version runs are not downloaded: their names.
    Browsers { version: String, missing: Vec<String> },
}

/// Where Playwright keeps its browsers on `host`: `PLAYWRIGHT_BROWSERS_PATH`, else its
/// cache folder.
pub fn browsers_dir(host: &dyn Host) -> PathBuf {
    let info = host.info();
    if let Some(p) = info.var("PLAYWRIGHT_BROWSERS_PATH").filter(|p| !p.is_empty() && *p != "0") {
        return PathBuf::from(p);
    }
    if info.is_mac() {
        info.home.join("Library/Caches/ms-playwright")
    } else if info.is_windows() {
        info.var("LOCALAPPDATA").map_or_else(|| info.home.join("AppData/Local"), PathBuf::from).join("ms-playwright")
    } else {
        info.var("XDG_CACHE_HOME").map_or_else(|| info.home.join(".cache"), PathBuf::from).join("ms-playwright")
    }
}

/// The folder of an installed package `name` for the project at `dir`: its own
/// `node_modules`, or one above (a workspace hoists them).
pub fn package_dir(host: &dyn Host, dir: &Path, name: &str) -> Option<PathBuf> {
    dir.ancestors().map(|d| d.join("node_modules").join(name)).find(|p| host.is_file(&p.join("package.json")))
}

/// The browsers Playwright installs by default that are not in `browsers`, by the
/// `browsers.json` of its `playwright-core`: their titles (`Chromium`, `Firefox`).
pub fn missing_browsers(host: &dyn Host, manifest: &Value, browsers: &Path) -> Vec<String> {
    let installed: Vec<String> = host
        .read_dir(browsers)
        .map(|es| es.into_iter().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    let mut missing = Vec::new();
    for b in manifest.get("browsers").and_then(Value::as_array).into_iter().flatten() {
        let name = b.get("name").and_then(Value::as_str).unwrap_or_default();
        if b.get("installByDefault").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        // Each lives in `<name with _>-<revision>`; a platform may use another revision.
        let prefix = name.replace('-', "_");
        let mut revisions: Vec<&str> = b.get("revision").and_then(Value::as_str).into_iter().collect();
        if let Some(o) = b.get("revisionOverrides").and_then(Value::as_object) {
            revisions.extend(o.values().filter_map(Value::as_str));
        }
        if !revisions.iter().any(|r| installed.iter().any(|i| *i == format!("{prefix}-{r}"))) {
            let title = match name {
                "chromium" => "Chromium",
                "chromium-headless-shell" => "Chromium Headless Shell",
                "firefox" => "Firefox",
                "webkit" => "WebKit",
                "ffmpeg" => "FFmpeg (for videos)",
                other => other,
            };
            missing.push(title.to_owned());
        }
    }
    missing
}

/// A line about the Playwright project at `root` for the Project view: its Playwright, its
/// package manager and the Node it runs on (`Playwright 1.64.1 · npm · Node 24.13.0`).
pub fn summary(host: &dyn Host, root: &Path, stop: Option<&Path>) -> Option<String> {
    let project = super::project_at(host, root)?;
    let playwright = match (&project.installed, &project.declared) {
        (Some(v), _) => format!("Playwright {v}"),
        (None, Some(v)) => format!("Playwright {v}, not installed"),
        (None, None) => "Playwright".to_owned(),
    };
    let wanted = node::wanted(host, root, stop);
    let all = node::discover(host);
    let node = match node::choose(&wanted, &all) {
        Some(n) => format!("Node {}", n.version),
        None => "no Node fits".to_owned(),
    };
    Some(format!("{playwright} · {} · {node}", project.package_manager.name()))
}

/// What the Playwright project at `root` needs, if anything.
pub fn check(host: &dyn Host, root: &Path) -> Option<Need> {
    let project = super::project_at(host, root)?;
    let Some(version) = project.installed.clone() else {
        return Some(Need::Packages { manager: project.package_manager });
    };
    let core = package_dir(host, root, "playwright-core")?;
    let manifest: Value = serde_json::from_str(&host.read_to_string(&core.join("browsers.json")).ok()?).ok()?;
    let missing = missing_browsers(host, &manifest, &browsers_dir(host));
    (!missing.is_empty()).then_some(Need::Browsers { version, missing })
}

/// The command that installs what `need` asks for, run with `node`; or why there is none.
pub fn install_launch(host: &dyn Host, root: &Path, need: &Need, node: &Node) -> Result<ProcessLaunch, String> {
    let path = crate::lang::typescript::path_with(host, node);
    let env = vec![("PATH".to_owned(), path), ("FORCE_COLOR".to_owned(), "1".to_owned())];
    match need {
        Need::Packages { manager: PackageManager::Npm } => {
            let npm = crate::lang::typescript::npm_cli(host, node).ok_or("npm is missing beside Node")?;
            let ci = host.is_file(&root.join("package-lock.json"));
            Ok(ProcessLaunch {
                title: "Install packages".into(),
                program: node.bin.clone(),
                args: vec![npm.display().to_string(), if ci { "ci" } else { "install" }.into()],
                cwd: root.to_owned(),
                env,
                place: Place::Build,
                report: false,
                debug_node: None,
            })
        }
        Need::Packages { manager } => {
            let name = manager.name();
            let program = snout_host::which(host, &[name], &[node.bin_dir().to_owned()])
                .ok_or_else(|| format!("{name} is not installed (the project's lockfile is {name}'s)"))?;
            Ok(ProcessLaunch {
                title: "Install packages".into(),
                program,
                args: vec!["install".into()],
                cwd: root.to_owned(),
                env,
                place: Place::Build,
                report: false,
                debug_node: None,
            })
        }
        Need::Browsers { .. } => {
            let cli = package_dir(host, root, super::TEST_PACKAGE)
                .map(|d| d.join("cli.js"))
                .filter(|c| host.is_file(c))
                .ok_or("the Playwright test runner is not installed")?;
            Ok(ProcessLaunch {
                title: "Install Playwright browsers".into(),
                program: node.bin.clone(),
                args: vec![cli.display().to_string(), "install".into()],
                cwd: root.to_owned(),
                env,
                place: Place::Build,
                report: false,
                debug_node: None,
            })
        }
    }
}

/// The setup bar's state, for the open project.
pub struct Setup {
    model: Entity<Model>,
    runs: Entity<Runs>,
    /// The Playwright project looked at (the open project, when it is one).
    pub root: Option<PathBuf>,
    pub need: Option<Need>,
    /// What the Project view says of the project (see [`summary`]).
    pub summary: Option<String>,
    /// The install running, as its session.
    pub installing: Option<u64>,
    /// Why the install could not start.
    pub error: Option<String>,
    /// Not now, for this need.
    pub dismissed: Option<Need>,
    looking: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<()> for Setup {}

/// The open window's [`Setup`].
pub struct SetupHandle(pub Entity<Setup>);
impl Global for SetupHandle {}

impl Setup {
    pub fn new(model: Entity<Model>, runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        let subs = vec![
            cx.observe(&model, |this, model, cx| {
                let root = model.read(cx).project_root();
                if root != this.root {
                    this.root = root;
                    this.need = None;
                    this.error = None;
                    this.look(cx);
                }
            }),
            cx.subscribe(&runs, |this, _, e: &RunsEvent, cx| {
                if let RunsEvent::Ended(id) = e
                    && this.installing == Some(*id)
                {
                    this.installed(*id, cx);
                }
            }),
        ];
        let root = model.read(cx).project_root();
        let mut this = Setup {
            model,
            runs,
            root,
            need: None,
            summary: None,
            installing: None,
            error: None,
            dismissed: None,
            looking: None,
            _subs: subs,
        };
        this.look(cx);
        this
    }

    /// The need the bar shows: one not said no to.
    pub fn shown(&self) -> Option<&Need> {
        self.need.as_ref().filter(|n| self.dismissed.as_ref() != Some(*n) || self.installing.is_some())
    }

    /// Look at the project again, off the UI thread.
    pub fn look(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            self.need = None;
            self.summary = None;
            return;
        };
        let host = crate::host::host(cx);
        let task = cx.background_executor().spawn(async move {
            let need = check(&*host, &root);
            (need, summary(&*host, &root, Some(&root)))
        });
        self.looking = Some(cx.spawn(async move |this, cx| {
            let (need, summary) = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.need != need || this.summary != summary {
                    this.need = need;
                    this.summary = summary;
                    cx.notify();
                }
            });
        }));
    }

    /// Install what the bar says is missing, in the Build window.
    pub fn install(&mut self, cx: &mut Context<Self>) {
        let (Some(root), Some(need)) = (self.root.clone(), self.need.clone()) else { return };
        if self.installing.is_some() {
            return;
        }
        let host = crate::host::host(cx);
        let project = self.model.read(cx).project_root();
        // Which Node, and the command, are found off the UI thread.
        let find = cx.background_executor().spawn(async move {
            let wanted = node::wanted(&*host, &root, project.as_deref());
            let all = node::discover(&*host);
            let n = node::choose(&wanted, &all)
                .cloned()
                .ok_or("No Node fits this project: download it from the bar above first.".to_owned())?;
            install_launch(&*host, &root, &need, &n)
        });
        self.looking = Some(cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| {
                match found {
                    Ok(launch) => {
                        this.error = None;
                        this.installing = this.runs.update(cx, |r, cx| r.start(Launch::Process(launch), cx));
                    }
                    Err(why) => this.error = Some(why),
                }
                cx.notify();
            });
        }));
    }

    /// The install `id` ended: look again; packages now there let the TypeScript server see
    /// their types.
    fn installed(&mut self, id: u64, cx: &mut Context<Self>) {
        self.installing = None;
        let ok = self
            .runs
            .read(cx)
            .session(id)
            .and_then(|s| s.ended.as_ref())
            .is_some_and(|e| e.outcome == crate::run::Outcome::Success);
        if !ok {
            self.error = Some("The install did not finish: see the Build window.".into());
        }
        if ok
            && matches!(self.need, Some(Need::Packages { .. }))
            && let (Some(root), Some(hub)) = (self.root.clone(), crate::lang::hub(cx))
        {
            hub.update(cx, |h, cx| {
                let roots: Vec<PathBuf> = h.ts.keys().filter(|r| r.starts_with(&root)).cloned().collect();
                for r in roots {
                    h.restart_ts(&r, cx);
                }
            });
        }
        self.look(cx);
        // With the packages (or browsers) in, the tests can be listed.
        if ok && let Some(tests) = super::tests::tests(cx) {
            tests.update(cx, |t, cx| t.refresh(cx));
        }
        cx.notify();
    }

    /// Not now: hide the bar until something else is missing.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dismissed = self.need.clone();
        cx.notify();
    }
}
