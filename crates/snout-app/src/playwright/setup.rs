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
use crate::project_kind::Kind;
use crate::run::process::ProcessLaunch;
use crate::run::{Launch, Place, Runs, RunsEvent};

/// What the project lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    /// `node_modules` does not hold the test runner (or a package's packages) yet; `dir` is
    /// the folder to install in, below the project's (a package of its own, a workspace's
    /// root), none for the project's.
    Packages { manager: PackageManager, dir: Option<PathBuf> },
    /// The browsers this Playwright version runs are not downloaded: their names.
    Browsers { version: String, missing: Vec<String> },
}

impl Need {
    /// The folder the install runs in, when not the project's.
    pub fn dir(&self) -> Option<&Path> {
        match self {
            Need::Packages { dir, .. } => dir.as_deref(),
            Need::Browsers { .. } => None,
        }
    }
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
                "ffmpeg" => ts!("pwsetup-ffmpeg-for-videos"),
                other => other,
            };
            missing.push(title.to_owned());
        }
    }
    missing
}

/// A line about the project at `root` (of `kind`) for the Project view: the package manager
/// and the Node it runs on, with its Playwright. A Playwright suite names Playwright first
/// (`Playwright 1.64.1 · npm · Node 24.13.0`), an app last (`npm · Node 24.13.0 · Playwright
/// 1.64.1`).
pub fn summary(host: &dyn Host, root: &Path, kind: Kind, stop: Option<&Path>) -> Option<String> {
    let playwright = super::project_at(host, root).map(|project| match (&project.installed, &project.declared) {
        (Some(v), _) => format!("Playwright {v}"),
        (None, Some(v)) => t!("pwsetup-playwright-not-installed", v = v.to_string()),
        (None, None) => "Playwright".to_owned(),
    });
    let node = || format!("{} · {}", PackageManager::of(host, root).name(), node_line(host, root, stop));
    match (kind, playwright) {
        (Kind::Node { .. }, Some(pw)) => Some(format!("{} · {pw}", node())),
        (Kind::Node { .. }, None) => Some(node()),
        (Kind::Playwright | Kind::Java { playwright: true }, Some(pw)) => Some(format!("{pw} · {}", node())),
        _ => None,
    }
}

/// The Node the project at `root` runs on, for the Project view (`Node 24.13.0`).
fn node_line(host: &dyn Host, root: &Path, stop: Option<&Path>) -> String {
    let wanted = node::wanted(host, root, stop);
    let all = node::discover(host);
    match node::choose(&wanted, &all) {
        Some(n) => format!("Node {}", n.version),
        None => ts!("pwsetup-no-node-fits").to_owned(),
    }
}

/// What the project at `root` (of `kind`) needs, if anything: a Node project's packages
/// first, then what its Playwright tests need.
pub fn check(host: &dyn Host, root: &Path, kind: Kind) -> Option<Need> {
    if kind == Kind::Playwright {
        return playwright_need(host, root);
    }
    // A Node package needs its packages once it lists some: the project's own first.
    use crate::run::node_run::{has_dependencies, install_dir, manager_of, packages};
    let packages_need = packages(host, root).into_iter().find_map(|pkg| {
        let at = install_dir(host, &pkg, root);
        let installed =
            pkg.ancestors().take_while(|d| d.starts_with(root)).any(|d| host.is_dir(&d.join("node_modules")));
        (has_dependencies(host, &pkg) && !installed)
            .then(|| Need::Packages { manager: manager_of(host, &at, root), dir: Some(at).filter(|a| a != root) })
    });
    packages_need.or_else(|| kind.has_playwright().then(|| playwright_need(host, root)).flatten())
}

/// What the Playwright tests at `root` need: the test runner installed, then its browsers.
fn playwright_need(host: &dyn Host, root: &Path) -> Option<Need> {
    let project = super::project_at(host, root)?;
    let Some(version) = project.installed.clone() else {
        return Some(Need::Packages { manager: project.package_manager, dir: None });
    };
    let core = package_dir(host, root, "playwright-core")?;
    let manifest: Value = serde_json::from_str(&host.read_to_string(&core.join("browsers.json")).ok()?).ok()?;
    let missing = missing_browsers(host, &manifest, &browsers_dir(host));
    (!missing.is_empty()).then_some(Need::Browsers { version, missing })
}

/// Whether the install `need` asks for runs `npm ci`: npm, and a `package-lock.json` in
/// the folder it installs in (without one, `npm ci` fails, so it is `npm install`).
pub fn uses_ci(host: &dyn Host, root: &Path, need: &Need) -> bool {
    matches!(need, Need::Packages { manager: PackageManager::Npm, .. })
        && host.is_file(&need.dir().unwrap_or(root).join("package-lock.json"))
}

/// The command that installs what `need` asks for, run with `node`; or why there is none.
pub fn install_launch(host: &dyn Host, root: &Path, need: &Need, node: &Node) -> Result<ProcessLaunch, String> {
    let root = need.dir().unwrap_or(root);
    let path = crate::lang::typescript::path_with(host, node);
    let env = vec![("PATH".to_owned(), path), ("FORCE_COLOR".to_owned(), "1".to_owned())];
    match need {
        Need::Packages { manager: PackageManager::Npm, .. } => {
            let npm = crate::lang::typescript::npm_cli(host, node).ok_or(ts!("run-npm-is-missing-beside-node"))?;
            let ci = uses_ci(host, root, need);
            Ok(ProcessLaunch {
                title: ts!("pwsetup-install-packages").into(),
                program: node.bin.clone(),
                args: vec![npm.display().to_string(), if ci { "ci" } else { "install" }.into()],
                cwd: root.to_owned(),
                env,
                place: Place::Build,
                report: false,
                debug_node: None,
                attach: None,
            })
        }
        Need::Packages { manager, .. } => {
            let name = manager.name();
            let program = snout_host::which(host, &[name], &[node.bin_dir().to_owned()])
                .ok_or_else(|| t!("run-is-not-installed-the-projects-lockfile-is-s", name = name.to_string()))?;
            Ok(ProcessLaunch {
                title: ts!("pwsetup-install-packages").into(),
                program,
                args: vec!["install".into()],
                cwd: root.to_owned(),
                env,
                place: Place::Build,
                report: false,
                debug_node: None,
                attach: None,
            })
        }
        Need::Browsers { .. } => {
            let cli = package_dir(host, root, super::TEST_PACKAGE)
                .map(|d| d.join("cli.js"))
                .filter(|c| host.is_file(c))
                .ok_or(ts!("pwsetup-runner-missing"))?;
            Ok(ProcessLaunch {
                title: ts!("pwsetup-install-playwright-browsers").into(),
                program: node.bin.clone(),
                args: vec![cli.display().to_string(), "install".into()],
                cwd: root.to_owned(),
                env,
                place: Place::Build,
                report: false,
                debug_node: None,
                attach: None,
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
    /// What the project is, as last looked at.
    pub kind: Kind,
    /// The project is not a Node package itself (a Java build with a front end in a
    /// folder): a package's bar waits until one of that package's files is open.
    pub quiet: bool,
    /// The install the bar offers is `npm ci` (npm, with a `package-lock.json`), not
    /// `install`.
    pub ci: bool,
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
            kind: Kind::Plain,
            quiet: false,
            ci: false,
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
            let kind = Kind::of(&*host, &root);
            let need = check(&*host, &root, kind);
            let quiet = !kind.has_playwright() && !kind.is_node_package();
            let ci = need.as_ref().is_some_and(|n| uses_ci(&*host, &root, n));
            (need, summary(&*host, &root, kind, Some(&root)), kind, quiet, ci)
        });
        self.looking = Some(cx.spawn(async move |this, cx| {
            let (need, summary, kind, quiet, ci) = task.await;
            let _ = this.update(cx, |this, cx| {
                let now = (&need, &summary, kind, quiet, ci);
                if now != (&this.need, &this.summary, this.kind, this.quiet, this.ci) {
                    this.need = need;
                    this.summary = summary;
                    this.kind = kind;
                    this.quiet = quiet;
                    this.ci = ci;
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
            let at = need.dir().map_or_else(|| root.clone(), Path::to_path_buf);
            let wanted = node::wanted(&*host, &at, project.as_deref());
            let all = node::discover(&*host);
            let n = node::choose(&wanted, &all)
                .cloned()
                .ok_or(ts!("pwsetup-no-node-fits-this-project-download-it-from").to_owned())?;
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
            self.error = Some(ts!("pwsetup-the-install-did-not-finish-see-the-build").into());
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
