//! The language servers of the window: one Java engine per Gradle build, started when a
//! Java file of the build opens (or the project is one), restarted when it crashes, and
//! stopped when the app quits. The hub keeps the documents in sync, holds what the
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
    started: Instant,
    /// From start to the first diagnostics for an open file.
    pub first_diagnostics: Option<Duration>,
    /// Bumped on each start, so messages of an earlier process are ignored.
    generation: u64,
    /// Stopped on purpose (the app is quitting): no restart.
    stopped: bool,
    _tasks: Vec<Task<()>>,
}

impl Server {
    /// "Java: Indexing (40%)" and the like.
    pub fn status(&self) -> String {
        let what = match (&self.stage, self.progress.current()) {
            (Stage::Ready | Stage::Indexing, Some(work)) => work.label(),
            (stage, _) => stage.label(),
        };
        format!("Java: {what}")
    }

    pub fn busy(&self) -> bool {
        self.stage.busy() || (self.stage == Stage::Ready && self.progress.is_busy())
    }

    pub fn ready(&self) -> bool {
        self.initialized && self.client.as_ref().is_some_and(Client::is_alive)
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
}

/// The bar asking to download a JDK, and how the download goes.
#[derive(Clone)]
pub struct JdkPrompt {
    pub root: PathBuf,
    pub major: u32,
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
}

pub struct LangHub {
    model: Entity<Model>,
    /// Servers start only when this is set (the app sets it; most tests do not).
    pub enabled: bool,
    pub servers: HashMap<PathBuf, Server>,
    docs: HashMap<String, Doc>,
    pub jdk_prompt: Option<JdkPrompt>,
    /// The home folder JDKs are looked for and downloaded under.
    pub home: PathBuf,
    /// Held until the window has painted: the builds asked for meanwhile start then.
    pub paused: bool,
    pending: Vec<PathBuf>,
    /// The engine build snout.sh runs beside the app, in development.
    pub engine_build: Option<crate::background::EngineBuild>,
    _engine_watch: Option<Task<()>>,
    _quit: gpui::Subscription,
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

fn uri_key(uri: &Uri) -> String {
    uri.as_str().to_owned()
}

impl LangHub {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        let enabled = cx.try_global::<LangEnabled>().is_some_and(|e| e.0) && std::env::var_os("SNOUT_NO_LSP").is_none();
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let quit = cx.on_app_quit(|this, _| {
            this.shutdown_all();
            async {}
        });
        let mut this = Self {
            model,
            enabled,
            servers: HashMap::new(),
            docs: HashMap::new(),
            jdk_prompt: None,
            home,
            paused: false,
            pending: Vec::new(),
            engine_build: None,
            _engine_watch: None,
            _quit: quit,
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

    /// The Gradle build `path` belongs to, inside the open repository.
    pub fn root_for(&self, path: &Path, cx: &App) -> Option<PathBuf> {
        let repo = self.model.read(cx).repo.as_ref().map(|r| r.root().to_owned());
        java::gradle_root(path, repo.as_deref())
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
        let saved = self.model.read(cx).settings.project_jdks.get(root).and_then(|h| java::jdk_at(h));
        let req = java::requirement(root);
        let jdk = saved.filter(|j| req.fits(j.major)).or_else(|| {
            let all = java::discover(&self.home);
            java::choose(&req, &all).cloned()
        });
        let mut server = Server {
            root: root.to_owned(),
            jdk: jdk.clone(),
            client: None,
            stage: Stage::NeedsJdk(req.wanted()),
            progress: Progress::default(),
            initialized: false,
            diagnostics: BTreeMap::new(),
            crashes: 0,
            started: Instant::now(),
            first_diagnostics: None,
            generation: 0,
            stopped: false,
            _tasks: Vec::new(),
        };
        match jdk {
            Some(jdk) => {
                self.remember_jdk(root, &jdk, cx);
                server.jdk = Some(jdk);
                self.servers.insert(root.to_owned(), server);
                self.spawn(root, cx);
            }
            None => {
                self.servers.insert(root.to_owned(), server);
                if self.jdk_prompt.is_none() {
                    self.jdk_prompt =
                        Some(JdkPrompt { root: root.to_owned(), major: req.wanted(), progress: None, error: None });
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

    /// Start (or restart) the engine of `root` on its JDK.
    fn spawn(&mut self, root: &Path, cx: &mut Context<Self>) {
        let Some(server) = self.servers.get_mut(root) else { return };
        let Some(jdk) = server.jdk.clone() else { return };
        if self.engine_build.as_ref().is_some_and(|b| !b.finished()) {
            server.stage = Stage::WaitingForEngine;
            return;
        }
        let Some(dist) = java::engine_dist() else {
            server.stage = Stage::Failed("The Java engine is not built (run snout.sh)".into());
            return;
        };
        let (client, events) = match Client::spawn(java::engine_command(&dist, root, &jdk.home)) {
            Ok(c) => c,
            Err(e) => {
                server.stage = Stage::Failed(format!("The Java engine did not start: {e}"));
                return;
            }
        };
        eprintln!("[java] started the engine for {} on JDK {} (pid {})", root.display(), jdk.version, client.pid());
        server.generation += 1;
        let generation = server.generation;
        server.client = Some(client.clone());
        server.stage = Stage::Importing;
        server.progress.clear();
        server.initialized = false;
        server.started = Instant::now();
        server.diagnostics.clear();
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
        if matches!(server.stage, Stage::Importing | Stage::CompilingScala | Stage::Starting) {
            server.stage = Stage::Indexing;
        }
        // The documents open in editors, as they are now.
        for doc in self.docs.values_mut().filter(|d| d.root == root && !d.virtual_doc) {
            doc.version = 1;
            client.notify_raw(
                "textDocument/didOpen",
                json!({ "textDocument": {
                    "uri": doc.uri.as_str(), "languageId": "java", "version": doc.version,
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
            _ => {}
        }
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
        let cleared: Vec<String> = std::mem::take(&mut server.diagnostics).into_keys().collect();
        if server.started.elapsed() > Duration::from_secs(300) {
            server.crashes = 0;
        }
        server.crashes += 1;
        for uri in cleared {
            if let Some(doc) = self.docs.get(&uri) {
                for editor in doc.editors.iter().filter_map(|e| e.upgrade()) {
                    editor.update(cx, |e, cx| e.set_diagnostics(Vec::new(), cx));
                }
            }
        }
        let server = self.servers.get_mut(root).expect("still there");
        if server.crashes > 5 {
            server.stage = Stage::Failed("The Java server keeps stopping".into());
        } else {
            server.stage = Stage::Restarting;
            let delay = Duration::from_secs(1 << (server.crashes - 1).min(5)).min(Duration::from_secs(30));
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

    /// An editor shows the Java file `path` with `text`: tell its server (starting it).
    /// Returns the document's URI and build, for the editor's providers.
    pub fn open_doc(
        &mut self,
        editor: &WeakEntity<EditorPanel>,
        path: &Path,
        text: Rope,
        cx: &mut Context<Self>,
    ) -> Option<(Uri, PathBuf, Vec<Diagnostic>)> {
        let root = self.root_for(path, cx)?;
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
        if let Some(client) = self.client(&root) {
            client.notify_raw(
                "textDocument/didOpen",
                json!({ "textDocument": { "uri": uri.as_str(), "languageId": "java", "version": 1, "text": text.to_string() } }),
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
        let (major, root) = (prompt.major, prompt.root.clone());
        let dir = java::downloads_dir(&self.home);
        let job = cx.background_executor().spawn({
            let progress = progress.clone();
            async move { java::download_temurin(major, &dir, &progress) }
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
                    Ok(jdk) => {
                        eprintln!("[java] downloaded JDK {} to {}", jdk.version, jdk.home.display());
                        this.jdk_prompt = None;
                        // Every build waiting for this version can use it.
                        let waiting: Vec<PathBuf> = this
                            .servers
                            .values()
                            .filter(|s| s.jdk.is_none() && java::requirement(&s.root).fits(jdk.major))
                            .map(|s| s.root.clone())
                            .chain(std::iter::once(root.clone()))
                            .collect();
                        for r in waiting {
                            if let Some(s) = this.servers.get_mut(&r).filter(|s| s.client.is_none()) {
                                s.jdk = Some(jdk.clone());
                                this.remember_jdk(&r, &jdk, cx);
                                this.spawn(&r, cx);
                            }
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
        if self.jdk_prompt.as_ref().is_some_and(|p| p.progress.is_none()) {
            self.jdk_prompt = None;
            cx.notify();
        }
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

    /// Every diagnostic of every server, worst first then by file and line.
    pub fn all_problems(&self) -> Vec<(String, Diagnostic)> {
        let mut out: Vec<(String, Diagnostic)> = self
            .servers
            .values()
            .flat_map(|s| s.diagnostics.iter())
            .flat_map(|(uri, list)| list.iter().map(move |d| (uri.clone(), d.clone())))
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
            started: Instant::now(),
            first_diagnostics: None,
            generation: 0,
            stopped: true,
            _tasks: Vec::new(),
        });
        server.diagnostics.insert(uri, list);
    }
}
