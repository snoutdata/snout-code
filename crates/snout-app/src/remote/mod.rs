//! Remote SSH: the window on this Mac, the project on another machine reached with the
//! system's ssh, where `snout-server` (installed by the app) serves it as a `snout_host::Host`.
//!
//! File ▸ Connect to SSH Host… asks for a host ([`dialog`]); the app connects (installing
//! the server when it is not there, in the status bar's background tasks), puts the Java
//! engine there meanwhile ([`crate::lang::remote_engine`]), and asks for a folder of the
//! remote machine ([`picker`]). The window then opens again on that folder with the remote
//! machine as the workspace's host. Its settings (open tabs, recent projects of that host)
//! are kept apart, per host; the app's recent remote projects and hosts are in
//! `remote.json` beside the app's settings. File ▸ Close Remote Connection opens the window
//! again on this Mac.

pub mod askpass;
pub mod dialog;
pub mod picker;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{AnyWindowHandle, App, Entity, Global};
use serde::{Deserialize, Serialize};
use snout_host::HostRef;
use snout_remote::{RemoteHost, RemoteSession, SshTarget, Status};

use crate::background::BgTask;
use crate::model::Model;
use crate::settings::{Settings, Store};

/// The folder of the Linux builds of `snout-server` the app installs on remote machines:
/// `SNOUT_SERVER_DIST`, else the app's resources (`Contents/Resources/server` in the Mac's
/// bundle, `/usr/lib/snout-code/server` in a Linux package), else (development)
/// `target/snout-server-dist` of the workspace the app was built in, made by
/// `scripts/build-server-linux.sh`.
pub fn server_binaries() -> Option<PathBuf> {
    let has =
        |d: &Path| d.join("snout-server-linux-x86_64").is_file() || d.join("snout-server-linux-aarch64").is_file();
    if let Some(d) = std::env::var_os("SNOUT_SERVER_DIST").map(PathBuf::from) {
        return has(&d).then_some(d);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(found) = crate::platform::resource_dirs(&exe).into_iter().map(|d| d.join("server")).find(|d| has(d))
    {
        return Some(found);
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snout-server-dist");
    has(&built).then(|| snout_host::canonical(&built).unwrap_or(built))
}

/// The variable naming an ssh config file to use instead of `~/.ssh/config` (`ssh -F`):
/// for tests, with scratch keys.
pub const SSH_CONFIG_ENV: &str = "SNOUT_SSH_CONFIG";

/// The ssh config the app reads its hosts from (and passes to ssh when it is not the user's).
pub fn ssh_config() -> Option<PathBuf> {
    std::env::var_os(SSH_CONFIG_ENV).map(PathBuf::from).or_else(snout_remote::ssh_config::user_config)
}

/// How the app reaches `name` with ssh: the scratch config when one is set, prompts through
/// this program (see [`askpass`]), a shared connection.
pub fn target(name: &str) -> SshTarget {
    if let Some(distro) = name.strip_prefix(snout_remote::WSL_PREFIX) {
        return SshTarget::wsl(distro);
    }
    let mut t = SshTarget::new(name);
    t.config_file = std::env::var_os(SSH_CONFIG_ENV).map(PathBuf::from);
    t.askpass = std::env::current_exe().ok();
    t.control_dir = control_dir();
    t
}

/// Where the shared connection's control socket lives: short, since the socket's path must
/// fit in 104 bytes. Windows's OpenSSH cannot share a connection (it has no ControlMaster), so
/// there each ssh connects on its own.
fn control_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        // SAFETY: getuid cannot fail.
        let uid = unsafe { libc::getuid() };
        Some(
            std::env::var_os("SNOUT_SSH_CONTROL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp").join(format!("snout-ssh-{uid}"))),
        )
    }
    #[cfg(not(unix))]
    None
}

/// A project on a remote machine, as File ▸ Recent Projects lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProject {
    pub host: String,
    pub path: PathBuf,
}

impl RemoteProject {
    /// `demo on devbox`.
    pub fn label(&self) -> String {
        t!("remote-on", path = (Model::dir_name(&self.path)).to_string(), host = (self.host).to_string())
    }
}

/// The hosts and remote projects opened lately, most recent first (`remote.json`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recents {
    pub hosts: Vec<String>,
    pub projects: Vec<RemoteProject>,
    /// The remote project the window was on when the app last left it: the next start opens
    /// it again. Closing the connection forgets it.
    pub last: Option<RemoteProject>,
}

const RECENT_MAX: usize = 10;

impl Recents {
    fn file(dir: &Path) -> PathBuf {
        dir.join("remote.json")
    }

    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::file(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self, dir: &Path) {
        let _ = std::fs::create_dir_all(dir);
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::file(dir), text);
        }
    }

    fn add_host(&mut self, host: &str) {
        self.hosts.retain(|h| h != host);
        self.hosts.insert(0, host.to_owned());
        self.hosts.truncate(RECENT_MAX);
    }

    fn add_project(&mut self, p: RemoteProject) {
        self.projects.retain(|q| q != &p);
        self.projects.insert(0, p);
        self.projects.truncate(RECENT_MAX);
    }
}

/// The connection the window works over.
pub struct Connection {
    /// The host as the user named it.
    pub name: String,
    /// None in the screenshots, which draw a connection without one.
    pub host: Option<Arc<RemoteHost>>,
    pub status: Status,
}

/// The app's remote state: where to go back to, what was opened lately, the connection,
/// and what it does in the background.
#[derive(Default)]
pub struct Remote {
    /// This Mac, as the app was given it (a slowed-down one with `SNOUT_HOST_LATENCY_MS`).
    pub local: Option<HostRef>,
    /// The app's own settings store, for the Mac's window.
    pub main_store: Option<Store>,
    pub recents: Recents,
    pub connection: Option<Connection>,
    /// Connecting, installing the server, putting the engine there.
    pub tasks: Vec<BgTask>,
    /// A connection made, waiting for a folder to be picked.
    pub pending: Option<(String, Arc<RemoteHost>)>,
    /// This machine's WSL distros, to offer beside the hosts of ssh.
    pub wsl_distros: Vec<String>,
    /// The ports the open project names (`forwarded_ports`), and how forwarding each went.
    pub forwards: Vec<Forwarding>,
}

impl Global for Remote {}

impl Remote {
    /// Keep the recents in the app's own folder.
    fn save_recents(&self) {
        if let Some(store) = &self.main_store {
            self.recents.save(&store.dir());
        }
    }

    /// `p` was opened: first among the recent projects, and the one the next start opens again.
    fn opened(&mut self, p: RemoteProject) {
        self.recents.add_project(p.clone());
        self.recents.last = Some(p);
        self.save_recents();
    }

    /// The connection was closed on purpose: the next start stays on this Mac.
    fn closed_on_purpose(&mut self) {
        self.recents.last = None;
        self.save_recents();
    }
}

/// Keep the Mac's host and store to come back to, read the recent remote projects, and
/// follow the connection's state.
pub fn init(store: Option<Store>, cx: &mut App) {
    let recents = store.as_ref().map(|s| Recents::load(&s.dir())).unwrap_or_default();
    cx.set_global(Remote { local: Some(crate::host::host(cx)), main_store: store, recents, ..Default::default() });
    if cfg!(windows) {
        let list = cx.background_executor().spawn(async { crate::wsl::distros() });
        cx.spawn(async move |cx| {
            let distros = list.await;
            cx.update(|cx| cx.global_mut::<Remote>().wsl_distros = distros);
        })
        .detach();
    }
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            cx.update(poll);
        }
    })
    .detach();
}

/// Look at the connection's state; redraw the windows when it changed.
fn poll(cx: &mut App) {
    let Some(r) = cx.try_global::<Remote>() else { return };
    let Some(c) = r.connection.as_ref() else { return };
    let Some(host) = c.host.clone() else { return };
    let now = host.status();
    if now != c.status {
        if let Some(c) = cx.global_mut::<Remote>().connection.as_mut() {
            c.status = now;
        }
        cx.refresh_windows();
    }
}

/// Try again after the connection was given up on.
pub fn reconnect(cx: &mut App) {
    if let Some(host) = cx.try_global::<Remote>().and_then(|r| r.connection.as_ref()).and_then(|c| c.host.clone()) {
        host.reconnect();
    }
}

/// How the connection is, in words, and its colour.
pub fn describe_status(name: &str, status: &Status, p: &crate::theme::Palette) -> (String, gpui::Hsla) {
    match status {
        Status::Connected => (t!("remote-connected-to", name = name.to_string()), p.ok),
        Status::Reconnecting { attempt: 0, .. } => (t!("remote-reconnecting-to", name = name.to_string()), p.warn),
        Status::Reconnecting { attempt, .. } => {
            (t!("remote-reconnecting-to-attempt", name = name.to_string(), attempt = (attempt + 1).to_string()), p.warn)
        }
        Status::Lost(_) => (t!("remote-the-connection-to-is-lost", name = name.to_string()), p.error),
        Status::Closed => (t!("remote-disconnected-from", name = name.to_string()), p.fg_dim),
    }
}

/// The connection's name and state, when the window works on a remote machine.
pub fn connection(cx: &App) -> Option<(String, Status)> {
    cx.try_global::<Remote>()?.connection.as_ref().map(|c| (c.name.clone(), c.status.clone()))
}

pub fn is_remote(cx: &App) -> bool {
    cx.try_global::<Remote>().is_some_and(|r| r.connection.is_some())
}

/// What Remote SSH does in the background, for the status bar.
pub fn tasks(cx: &App) -> Vec<BgTask> {
    cx.try_global::<Remote>().map(|r| r.tasks.clone()).unwrap_or_default()
}

/// The recent hosts and remote projects.
pub fn recents(cx: &App) -> Recents {
    cx.try_global::<Remote>().map(|r| r.recents.clone()).unwrap_or_default()
}

/// The hosts to offer: the recent ones first, then those of the ssh config.
pub fn known_hosts(cx: &App) -> Vec<String> {
    let mut out = recents(cx).hosts;
    let distros = cx.try_global::<Remote>().map(|r| r.wsl_distros.clone()).unwrap_or_default();
    for d in distros {
        let name = format!("{}{d}", snout_remote::WSL_PREFIX);
        if !out.contains(&name) {
            out.push(name);
        }
    }
    for h in ssh_config().map(|f| snout_remote::ssh_config::hosts(&f)).unwrap_or_default() {
        if !out.contains(&h) {
            out.push(h);
        }
    }
    out
}

fn set_task(title: &str, task: Option<BgTask>, cx: &mut App) {
    if !cx.has_global::<Remote>() {
        return;
    }
    let tasks = &mut cx.global_mut::<Remote>().tasks;
    tasks.retain(|t| t.title != title);
    if let Some(t) = task {
        tasks.push(t);
    }
    cx.refresh_windows();
}

fn task(title: &str, stage: &str, percent: Option<u32>) -> BgTask {
    BgTask { title: title.to_owned(), stage: stage.to_owned(), percent, ..Default::default() }
}

/// What a connection that failed says.
pub fn describe(e: &std::io::Error) -> String {
    let text = e.to_string();
    match e.kind() {
        std::io::ErrorKind::Unsupported => text,
        std::io::ErrorKind::NotFound if server_binaries().is_none() => {
            ts!("remote-the-linux-server-builds-are-missing-run-scripts").into()
        }
        _ => text,
    }
}

/// Connect to `name` off the UI thread, installing the server there when it is not; then put
/// the Java engine there in the background. `done` hears how it went, on the UI thread.
pub fn connect(name: String, done: impl FnOnce(Result<Arc<RemoteHost>, String>, &mut App) + 'static, cx: &mut App) {
    let title = t!("remote-connecting-to-3", name = name.to_string());
    set_task(&title, Some(task(&title, ts!("remote-connecting"), None)), cx);
    let (tx, rx) = std::sync::mpsc::channel::<&'static str>();
    let target = target(&name);
    let work = cx.background_executor().spawn(async move {
        let binaries = server_binaries()
            .ok_or_else(|| ts!("remote-the-linux-server-builds-are-missing-run-scripts").to_owned())?;
        let say = move |what: &str| {
            let _ = tx.send(if what == "installing" {
                ts!("remote-installing-snout-server")
            } else {
                ts!("remote-connecting")
            });
        };
        let session = RemoteSession::open_with(target, &binaries, &say).map_err(|e| describe(&e))?;
        RemoteHost::connect(Arc::new(session)).map(Arc::new).map_err(|e| describe(&e))
    });
    cx.spawn(async move |cx| {
        let mut work = std::pin::pin!(work);
        let result = loop {
            let tick = cx.background_executor().timer(Duration::from_millis(200));
            match futures::future::select(work.as_mut(), tick).await {
                futures::future::Either::Left((r, _)) => break r,
                futures::future::Either::Right(_) => {
                    if let Some(stage) = rx.try_iter().last() {
                        let t = title.clone();
                        cx.update(|cx| set_task(&t, Some(task(&t, stage, None)), cx));
                    }
                }
            }
        };
        cx.update(|cx| {
            set_task(&title, None, cx);
            if let Ok(host) = &result {
                if cx.has_global::<Remote>() {
                    let r = cx.global_mut::<Remote>();
                    r.recents.add_host(&name);
                    if let Some(store) = &r.main_store {
                        r.recents.save(&store.dir());
                    }
                }
                install_engine(name.clone(), host.clone(), cx);
            }
            done(result, cx);
        });
    })
    .detach();
}

/// Put the Java engine on the remote machine, in the background tasks.
fn install_engine(name: String, host: Arc<RemoteHost>, cx: &mut App) {
    let title = t!("remote-java-engine-on", name = name.to_string());
    set_task(&title, Some(task(&title, ts!("remote-looking-for-the-engine"), None)), cx);
    let progress = crate::lang::java::DownloadProgress::default();
    let stage = Arc::new(std::sync::Mutex::new(String::from(ts!("remote-looking-for-the-engine"))));
    let installing = host.clone();
    let work = cx.background_executor().spawn({
        let (progress, stage) = (progress.clone(), stage.clone());
        async move {
            let say = |s: &str| *stage.lock().unwrap_or_else(|e| e.into_inner()) = s.to_owned();
            crate::lang::remote_engine::install(&*host, &say, &progress).map_err(|e| format!("{e:#}"))
        }
    });
    cx.spawn(async move |cx| {
        let mut work = std::pin::pin!(work);
        let result = loop {
            let tick = cx.background_executor().timer(Duration::from_millis(250));
            match futures::future::select(work.as_mut(), tick).await {
                futures::future::Either::Left((r, _)) => break r,
                futures::future::Either::Right(_) => {
                    let now = stage.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    let t = title.clone();
                    let pc = progress.percent();
                    cx.update(|cx| set_task(&t, Some(task(&t, &now, pc)), cx));
                }
            }
        };
        cx.update(|cx| match result {
            Ok(dir) => {
                eprintln!("[remote] the engine is at {} on {name}", dir.display());
                set_task(&title, None, cx);
            }
            Err(e) => {
                eprintln!("[remote] the engine could not be put on {name}: {e}");
                // The window left the host and closed its connection (another host, or Close
                // Remote Connection): that is why, and the task goes with it.
                let left = installing.status() == Status::Closed;
                let failed = task(&title, &t!("remote-could-not-install-it", e = e.to_string()), None);
                set_task(&title, (!left).then_some(failed), cx);
            }
        });
    })
    .detach();
}

/// A file name for `host` (its settings' folder).
fn safe_name(host: &str) -> String {
    host.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.@".contains(c) { c } else { '_' }).collect()
}

/// The settings store of the windows on `host`: their tabs, recent projects and layout,
/// apart from the Mac's. A new one starts from the Mac's look and keys.
fn store_for(host: &str, main: Option<&Store>, base: &Settings) -> Option<Store> {
    let main = main?;
    let store = Store::at(main.dir().join("remote").join(safe_name(host)).join("settings.json"));
    if !store.file.is_file() {
        let seed = Settings {
            repos: Vec::new(),
            groups: Vec::new(),
            current: None,
            recent: None,
            opened: Default::default(),
            sessions: Default::default(),
            project_jdks: Default::default(),
            dock: None,
            ..base.clone()
        };
        let _ = store.save(&seed);
    }
    Some(store)
}

/// The host the windows use: `host`, slowed down as this Mac is with `SNOUT_HOST_LATENCY_MS`.
fn as_workspace_host(host: Arc<RemoteHost>) -> HostRef {
    match std::env::var(snout_host::LATENCY_ENV).ok().and_then(|v| v.trim().parse::<u64>().ok()).filter(|m| *m > 0) {
        Some(ms) => Arc::new(snout_host::LatencyHost::new(host, Duration::from_millis(ms)).ui_thread_here()),
        None => host,
    }
}

/// Open the window again on `folder` of the remote machine `host` (reached as `name`), in
/// place of `old` (whose model `from` records it among the recent projects). Coming from
/// another host, its connection closes after `before` (which runs off the UI thread, while
/// it is still there).
pub fn open_project(
    name: String,
    host: Arc<RemoteHost>,
    folder: PathBuf,
    from: &Entity<Model>,
    old: AnyWindowHandle,
    before: Option<Box<dyn FnOnce() + Send>>,
    cx: &mut App,
) {
    let base = from.read(cx).settings.clone();
    let main = cx.try_global::<Remote>().and_then(|r| r.main_store.clone());
    if cx.has_global::<Remote>() {
        let r = cx.global_mut::<Remote>();
        r.opened(RemoteProject { host: name.clone(), path: folder.clone() });
        r.pending = None;
    }
    let store = store_for(&name, main.as_ref(), &base);
    cx.set_global(crate::host::HostGlobal(as_workspace_host(host.clone())));
    // The project before's ports go with it; the new one's follow once its settings are read.
    cx.global_mut::<Remote>().forwards.clear();
    let left = cx.global_mut::<Remote>().connection.replace(Connection {
        name: name.clone(),
        host: Some(host),
        status: Status::Connected,
    });
    // From another host: close its connection, once `before` has cleaned up there.
    if let Some(old_host) = left.filter(|c| c.name != name).and_then(|c| c.host) {
        cx.background_executor()
            .spawn(async move {
                if let Some(before) = before {
                    before();
                }
                old_host.close();
            })
            .detach();
    }
    switch_window(store, Some(folder), old, cx);
}

/// Close the connection and open the window again on this Mac, in place of `old`, on `folder`
/// if there is one. `before` runs first (off the UI thread), while the connection is still
/// there.
pub fn disconnect(
    old: AnyWindowHandle,
    folder: Option<PathBuf>,
    before: Option<Box<dyn FnOnce() + Send>>,
    cx: &mut App,
) {
    let Some(r) = cx.try_global::<Remote>() else { return };
    let local = r.local.clone().unwrap_or_else(snout_host::local);
    let store = r.main_store.clone();
    let r = cx.global_mut::<Remote>();
    r.closed_on_purpose();
    let conn = r.connection.take();
    r.forwards.clear();
    if let Some(host) = conn.and_then(|c| c.host) {
        cx.background_executor()
            .spawn(async move {
                if let Some(before) = before {
                    before();
                }
                host.close();
            })
            .detach();
    }
    cx.global_mut::<Remote>().tasks.clear();
    cx.set_global(crate::host::HostGlobal(local));
    switch_window(store, folder, old, cx);
}

/// A new window over a new model from `store` (opening `folder`), then `old` closes.
fn switch_window(store: Option<Store>, folder: Option<PathBuf>, old: AnyWindowHandle, cx: &mut App) {
    let model = crate::model(store, folder, cx);
    if crate::open_window(model, None, cx).is_some() {
        let _ = old.update(cx, |_, window, _| window.remove_window());
    }
    cx.refresh_windows();
}

/// Forward the ports the open project names (`forwarded_ports` in its `.snout/settings.json`)
/// from this Mac to the connection's host, in place of those forwarded before. A port that
/// cannot be (taken here, say) is said in the status bar and the output; the others are
/// forwarded all the same.
pub fn forward_ports(model: &Entity<Model>, cx: &mut App) {
    let Some(r) = cx.try_global::<Remote>() else { return };
    let Some((name, host)) = r.connection.as_ref().and_then(|c| Some((c.name.clone(), c.host.clone()?))) else {
        return;
    };
    // The ports before go first, so one forwarded again is free to take.
    cx.global_mut::<Remote>().forwards.clear();
    let wanted = model.read(cx).project_settings.forwarded_ports.clone().unwrap_or_default();
    let forwards = forward_each(&wanted, |app_port, port| snout_host::Host::forward_to_host(&*host, app_port, port));
    let mut problems = Vec::new();
    for f in &forwards {
        match &f.state {
            Ok(_) => eprintln!("[remote] forwarding {}", f.short(&name)),
            Err(p) => problems.push(p.long(&f.named, &name)),
        }
    }
    cx.global_mut::<Remote>().forwards = forwards;
    if !problems.is_empty() {
        model.update(cx, |m, cx| {
            m.message = problems.join("; ");
            for p in problems {
                m.log_output(p, true);
            }
            cx.notify();
        });
    }
}

/// The project's forwarded ports as the host's menu lists them: what each is, and whether it
/// is forwarded.
pub fn forwarded_ports(cx: &App) -> Vec<(String, bool)> {
    let Some(r) = cx.try_global::<Remote>() else { return Vec::new() };
    let Some(name) = r.connection.as_ref().map(|c| c.name.clone()) else { return Vec::new() };
    r.forwards.iter().map(|f| (f.short(&name), f.state.is_ok())).collect()
}

/// A port the open project names, and how forwarding it went.
pub struct Forwarding {
    /// As the project's file names it.
    pub named: crate::project_settings::ForwardedPort,
    /// Forwarded (until dropped), or why not.
    pub state: Result<snout_host::Forward, PortProblem>,
}

/// Why a port is not forwarded.
#[derive(Debug, PartialEq)]
pub enum PortProblem {
    NotAPort,
    /// This Mac's port is taken.
    Taken,
    Failed(String),
}

impl Forwarding {
    /// `4200 → devbox:4200`, or `8080: taken on this Mac`.
    pub fn short(&self, name: &str) -> String {
        let port = match self.named.ports() {
            Some((app_port, _)) => app_port.to_string(),
            None => self.named.to_string(),
        };
        match &self.state {
            Ok(f) => t!("remote-forwarded-port-to", port = port, name = name.to_string(), remote = f.port.to_string()),
            Err(PortProblem::NotAPort) => t!("remote-forwarded-port-not-a-port-short", port = port),
            Err(PortProblem::Taken) => {
                t!("remote-forwarded-port-taken-short", port = port, machine = crate::platform::this_machine())
            }
            Err(PortProblem::Failed(e)) => t!("remote-forwarded-port-failed-short", port = port, e = e.clone()),
        }
    }
}

impl PortProblem {
    /// What the status bar says of `named`, not forwarded to `name`.
    fn long(&self, named: &crate::project_settings::ForwardedPort, name: &str) -> String {
        let port = named.ports().map(|(app_port, _)| app_port.to_string()).unwrap_or_else(|| named.to_string());
        match self {
            Self::NotAPort => t!("remote-forwarded-port-not-a-port", port = port),
            Self::Taken => t!(
                "remote-forwarded-port-taken",
                port = port,
                machine = crate::platform::this_machine(),
                name = name.to_string()
            ),
            Self::Failed(e) => t!("remote-forwarded-port-failed", port = port, name = name.to_string(), e = e.clone()),
        }
    }
}

/// Forward each of `ports` with `forward` (this Mac's port, the host's), and say how each went.
fn forward_each(
    ports: &[crate::project_settings::ForwardedPort],
    forward: impl Fn(u16, u16) -> std::io::Result<snout_host::Forward>,
) -> Vec<Forwarding> {
    ports
        .iter()
        .map(|named| {
            let state = match named.ports() {
                None => Err(PortProblem::NotAPort),
                Some((app_port, port)) => forward(app_port, port).map_err(|e| match e.kind() {
                    std::io::ErrorKind::AddrInUse => PortProblem::Taken,
                    _ => PortProblem::Failed(e.to_string()),
                }),
            };
            Forwarding { named: named.clone(), state }
        })
        .collect()
}

/// The remote project to open again at this start: the one the app was on when it last quit,
/// unless the start was asked to open something else.
pub fn to_restore(asked: bool, cx: &App) -> Option<RemoteProject> {
    if asked {
        return None;
    }
    cx.try_global::<Remote>()?.recents.last.clone()
}

/// What the start does with the remote project it opens again, once connected.
#[derive(Debug, PartialEq, Eq)]
pub enum Restore {
    Open,
    /// A project was opened while it connected: that one stays.
    Leave,
    /// The folder is no longer on the host: the next start does not try it.
    Forget,
}

pub fn restore_choice(opened_meanwhile: bool, folder_there: bool) -> Restore {
    if opened_meanwhile {
        Restore::Leave
    } else if folder_there {
        Restore::Open
    } else {
        Restore::Forget
    }
}

/// A remote project could not be opened: if it was the one to open again at start, stop
/// trying, so the next start opens this machine's project.
pub fn forget_last(p: &RemoteProject, cx: &mut App) {
    let Some(r) = cx.try_global::<Remote>() else { return };
    if r.recents.last.as_ref() != Some(p) {
        return;
    }
    let r = cx.global_mut::<Remote>();
    r.recents.last = None;
    r.save_recents();
}

/// Forget a connection made for a folder that was not picked.
pub fn drop_pending(cx: &mut App) {
    if let Some((_, host)) = cx.try_global::<Remote>().and_then(|r| r.pending.clone()) {
        host.close();
        cx.global_mut::<Remote>().pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_settings::ForwardedPort;
    use snout_host::Forward;

    #[test]
    fn a_port_that_cannot_be_forwarded_is_said_and_the_others_are_forwarded() {
        let ports = [
            ForwardedPort::Same(4200),
            ForwardedPort::Same(8080),
            ForwardedPort::Pair("13306:3306".into()),
            ForwardedPort::Pair("not-a-port".into()),
            ForwardedPort::Same(0),
            ForwardedPort::Same(9000),
        ];
        let asked = std::sync::Mutex::new(Vec::new());
        let forwards = forward_each(&ports, |app_port, port| {
            asked.lock().unwrap().push((app_port, port));
            match app_port {
                8080 => Err(std::io::ErrorKind::AddrInUse.into()),
                9000 => Err(std::io::Error::other("no luck")),
                _ => Ok(Forward::new(port, Box::new(()))),
            }
        });
        assert_eq!(*asked.lock().unwrap(), [(4200, 4200), (8080, 8080), (13306, 3306), (9000, 9000)]);
        let ok: Vec<_> = forwards.iter().filter_map(|f| f.state.as_ref().ok().map(|f| f.port)).collect();
        assert_eq!(ok, [4200, 3306]);
        let short: Vec<_> = forwards.iter().map(|f| f.short("devbox")).collect();
        assert_eq!(short[0], "4200 → devbox:4200");
        assert_eq!(short[2], "13306 → devbox:3306");
        assert!(short[1].starts_with("8080: taken on "), "{short:?}");
        assert_eq!(short[3], "\"not-a-port\": not a port");
        assert_eq!(short[5], "9000: no luck");
        let problems: Vec<_> =
            forwards.iter().filter_map(|f| f.state.as_ref().err().map(|p| p.long(&f.named, "devbox"))).collect();
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems[0].contains("8080") && problems[0].contains("devbox"), "{problems:?}");
        assert!(problems[1].contains("\"not-a-port\""), "{problems:?}");
        assert!(problems[3].contains("9000") && problems[3].contains("no luck"), "{problems:?}");
    }

    #[test]
    fn forwarded_ports_read_as_numbers_and_pairs() {
        let p = crate::project_settings::parse(r#"{ "forwarded_ports": [4200, "18080:8080", " 9000 "] }"#).unwrap();
        let ports: Vec<_> = p.forwarded_ports.unwrap().iter().map(ForwardedPort::ports).collect();
        assert_eq!(ports, [Some((4200, 4200)), Some((18080, 8080)), Some((9000, 9000))]);
        assert_eq!(ForwardedPort::Pair("1:99999".into()).ports(), None);
    }

    #[gpui::test]
    fn a_project_that_could_not_be_opened_again_is_not_tried_at_the_next_start(cx: &mut gpui::TestAppContext) {
        let dir = crate::tests::scratch_dir("forget-last");
        let shop = RemoteProject { host: "devbox".into(), path: "/home/me/shop".into() };
        let other = RemoteProject { host: "build-host".into(), path: "/srv/media".into() };
        cx.update(|cx| {
            let mut r = Remote { main_store: Some(Store::at(dir.join("settings.json"))), ..Default::default() };
            r.recents.last = Some(shop.clone());
            cx.set_global(r);
            // Another project failing leaves the one to open again.
            forget_last(&other, cx);
            assert_eq!(cx.global::<Remote>().recents.last.as_ref(), Some(&shop));
            forget_last(&shop, cx);
            assert_eq!(cx.global::<Remote>().recents.last, None);
            assert_eq!(Recents::load(&dir).last, None, "and the file says so");
            assert_eq!(to_restore(false, cx), None);
        });
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Opening a remote project makes it the one the next start opens again, and the file
    /// keeps it; closing the connection on purpose forgets it. A start asked to open something
    /// (a folder or files on the command line) does not open it.
    #[gpui::test]
    fn the_next_start_opens_the_remote_project_the_app_was_on(cx: &mut gpui::TestAppContext) {
        let dir = crate::tests::scratch_dir("remote-last");
        let shop = RemoteProject { host: "devbox".into(), path: "/home/me/shop".into() };
        cx.update(|cx| {
            cx.set_global(Remote { main_store: Some(Store::at(dir.join("settings.json"))), ..Default::default() });
            cx.global_mut::<Remote>().opened(shop.clone());
            assert_eq!(cx.global::<Remote>().recents.projects, std::slice::from_ref(&shop));
            assert_eq!(Recents::load(&dir).last.as_ref(), Some(&shop), "the file keeps it");
            assert_eq!(to_restore(false, cx).as_ref(), Some(&shop));
            assert_eq!(to_restore(true, cx), None, "a start asked to open something");
            cx.global_mut::<Remote>().closed_on_purpose();
            assert_eq!(Recents::load(&dir).last, None, "closing the connection forgets it");
            assert_eq!(Recents::load(&dir).projects, std::slice::from_ref(&shop), "but not the recent project");
            assert_eq!(to_restore(false, cx), None);
        });
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Once connected, the start opens the remote project only if nothing was opened while it
    /// connected, and only if the folder is still on the host; a folder gone is forgotten.
    #[test]
    fn the_restore_opens_only_what_is_still_wanted_and_still_there() {
        assert_eq!(restore_choice(false, true), Restore::Open);
        assert_eq!(restore_choice(true, true), Restore::Leave, "a project was opened meanwhile");
        assert_eq!(restore_choice(true, false), Restore::Leave);
        assert_eq!(restore_choice(false, false), Restore::Forget, "the folder is gone");
    }

    /// A `remote.json` from before `last` still loads, with nothing to open again.
    #[test]
    fn recents_saved_before_last_was_kept_still_load() {
        let old = r#"{ "hosts": ["devbox"], "projects": [{ "host": "devbox", "path": "/home/me/shop" }] }"#;
        let r: Recents = serde_json::from_str(old).unwrap();
        assert_eq!((r.hosts.len(), r.projects.len(), r.last), (1, 1, None));
    }
}
