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
/// `SNOUT_SERVER_DIST`, else the app bundle's `Contents/Resources/server`, else (development)
/// `target/snout-server-dist` of the workspace the app was built in, made by
/// `scripts/build-server-linux.sh`.
pub fn server_binaries() -> Option<PathBuf> {
    let has =
        |d: &Path| d.join("snout-server-linux-x86_64").is_file() || d.join("snout-server-linux-aarch64").is_file();
    if let Some(d) = std::env::var_os("SNOUT_SERVER_DIST").map(PathBuf::from) {
        return has(&d).then_some(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1) {
            let candidate = dir.join("Resources/server");
            if has(&candidate) {
                return Some(candidate);
            }
        }
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snout-server-dist");
    has(&built).then(|| built.canonicalize().unwrap_or(built))
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
    let mut t = SshTarget::new(name);
    t.config_file = std::env::var_os(SSH_CONFIG_ENV).map(PathBuf::from);
    t.askpass = std::env::current_exe().ok();
    // Short: the control socket's path must fit in 104 bytes.
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    t.control_dir = Some(
        std::env::var_os("SNOUT_SSH_CONTROL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp").join(format!("snout-ssh-{uid}"))),
    );
    t
}

/// A project on a remote machine, as File ▸ Open Recent lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProject {
    pub host: String,
    pub path: PathBuf,
}

impl RemoteProject {
    /// `demo on devbox`.
    pub fn label(&self) -> String {
        format!("{} on {}", Model::dir_name(&self.path), self.host)
    }
}

/// The hosts and remote projects opened lately, most recent first (`remote.json`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recents {
    pub hosts: Vec<String>,
    pub projects: Vec<RemoteProject>,
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
}

impl Global for Remote {}

/// Keep the Mac's host and store to come back to, read the recent remote projects, and
/// follow the connection's state.
pub fn init(store: Option<Store>, cx: &mut App) {
    let recents = store.as_ref().map(|s| Recents::load(&s.dir())).unwrap_or_default();
    cx.set_global(Remote { local: Some(crate::host::host(cx)), main_store: store, recents, ..Default::default() });
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
        Status::Connected => (format!("Connected to {name}"), p.ok),
        Status::Reconnecting { attempt: 0, .. } => (format!("Reconnecting to {name}…"), p.warn),
        Status::Reconnecting { attempt, .. } => (format!("Reconnecting to {name}… (attempt {})", attempt + 1), p.warn),
        Status::Lost(_) => (format!("The connection to {name} is lost"), p.error),
        Status::Closed => (format!("Disconnected from {name}"), p.fg_dim),
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
            "The Linux server builds are missing: run scripts/build-server-linux.sh".into()
        }
        _ => text,
    }
}

/// Connect to `name` off the UI thread, installing the server there when it is not; then put
/// the Java engine there in the background. `done` hears how it went, on the UI thread.
pub fn connect(name: String, done: impl FnOnce(Result<Arc<RemoteHost>, String>, &mut App) + 'static, cx: &mut App) {
    let title = format!("Connecting to {name}");
    set_task(&title, Some(task(&title, "Connecting", None)), cx);
    let (tx, rx) = std::sync::mpsc::channel::<&'static str>();
    let target = target(&name);
    let work = cx.background_executor().spawn(async move {
        let binaries = server_binaries()
            .ok_or_else(|| "The Linux server builds are missing: run scripts/build-server-linux.sh".to_owned())?;
        let say = move |what: &str| {
            let _ = tx.send(if what == "installing" { "Installing snout-server" } else { "Connecting" });
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
    let title = format!("Java engine on {name}");
    set_task(&title, Some(task(&title, "Looking for the engine", None)), cx);
    let progress = crate::lang::java::DownloadProgress::default();
    let stage = Arc::new(std::sync::Mutex::new(String::from("Looking for the engine")));
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
                set_task(&title, Some(task(&title, &format!("Could not install it: {e}"), None)), cx);
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
/// place of `old` (whose model `from` records it among the recent projects).
pub fn open_project(
    name: String,
    host: Arc<RemoteHost>,
    folder: PathBuf,
    from: &Entity<Model>,
    old: AnyWindowHandle,
    cx: &mut App,
) {
    let base = from.read(cx).settings.clone();
    let main = cx.try_global::<Remote>().and_then(|r| r.main_store.clone());
    if cx.has_global::<Remote>() {
        let r = cx.global_mut::<Remote>();
        r.recents.add_project(RemoteProject { host: name.clone(), path: folder.clone() });
        if let Some(store) = &r.main_store {
            r.recents.save(&store.dir());
        }
        r.pending = None;
    }
    let store = store_for(&name, main.as_ref(), &base);
    cx.set_global(crate::host::HostGlobal(as_workspace_host(host.clone())));
    cx.global_mut::<Remote>().connection = Some(Connection { name, host: Some(host), status: Status::Connected });
    switch_window(store, Some(folder), old, cx);
}

/// Close the connection and open the window again on this Mac, in place of `old`. `before`
/// runs first (off the UI thread), while the connection is still there.
pub fn disconnect(old: AnyWindowHandle, before: Option<Box<dyn FnOnce() + Send>>, cx: &mut App) {
    let Some(r) = cx.try_global::<Remote>() else { return };
    let local = r.local.clone().unwrap_or_else(snout_host::local);
    let store = r.main_store.clone();
    let conn = cx.global_mut::<Remote>().connection.take();
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
    switch_window(store, None, old, cx);
}

/// A new window over a new model from `store` (opening `folder`), then `old` closes.
fn switch_window(store: Option<Store>, folder: Option<PathBuf>, old: AnyWindowHandle, cx: &mut App) {
    let model = crate::model(store, folder, cx);
    if crate::open_window(model, None, cx).is_some() {
        let _ = old.update(cx, |_, window, _| window.remove_window());
    }
    cx.refresh_windows();
}

/// Forget a connection made for a folder that was not picked.
pub fn drop_pending(cx: &mut App) {
    if let Some((_, host)) = cx.try_global::<Remote>().and_then(|r| r.pending.clone()) {
        host.close();
        cx.global_mut::<Remote>().pending = None;
    }
}
