//! TypeScript and JavaScript through vtsls (MIT), a language server around `tsserver`: one
//! per TypeScript project (the folder of the nearest `tsconfig.json`, `jsconfig.json` or
//! `package.json`), started when one of its files opens, on the Node the project asks for.
//! vtsls is installed on first use into SnoutCode's data folder with that Node's npm; it
//! uses the project's own `node_modules/typescript` when there is one, else the TypeScript it
//! carries. Nothing here starts Java.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{Context, Task, WeakEntity};
use gpui_component::input::Rope;
use lsp_types::{Diagnostic, Uri};
use serde_json::{Value, json};
use snout_host::{Cmd, Host};
use snout_lang::progress::Progress;
use snout_lang::{Client, ServerMessage};

use super::hub::{Doc, HubEvent, LangHub, uri_key};
use super::node::{self, Node};
use crate::editor::EditorPanel;

/// The vtsls release SnoutCode installs.
pub const VTSLS_VERSION: &str = "0.3.0";

/// Where a TypeScript server is, for the status bar.
#[derive(Clone, Debug, PartialEq)]
pub enum TsStage {
    /// Finding the Node the project asks for.
    FindingNode,
    /// No Node fits: what the project asks for, for the message.
    NeedsNode(String),
    /// Installing vtsls (first use).
    Installing,
    Starting,
    Ready,
    Restarting,
    Failed(String),
}

impl TsStage {
    pub fn label(&self) -> String {
        match self {
            TsStage::FindingNode => "Finding Node".into(),
            TsStage::NeedsNode(what) => format!("Needs Node {what}"),
            TsStage::Installing => "Installing the language server".into(),
            TsStage::Starting => "Starting".into(),
            TsStage::Ready => "Ready".into(),
            TsStage::Restarting => "Restarting".into(),
            TsStage::Failed(why) => why.clone(),
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self, TsStage::FindingNode | TsStage::Installing | TsStage::Starting | TsStage::Restarting)
    }
}

/// The bar asking to download the Node a TypeScript project needs, and how it goes.
#[derive(Clone)]
pub struct NodePrompt {
    pub root: PathBuf,
    pub wanted: node::Wanted,
    /// What the project asks for, as the bar says it (`22`, `20 or newer`).
    pub label: String,
    pub progress: Option<super::java::DownloadProgress>,
    pub error: Option<String>,
    /// Said no to, this session.
    pub dismissed: bool,
}

/// One TypeScript server.
pub struct TsServer {
    pub root: PathBuf,
    pub node: Option<Node>,
    pub(super) client: Option<Client>,
    pub stage: TsStage,
    pub progress: Progress,
    pub(super) initialized: bool,
    /// What the server published, by document URI.
    pub diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    crashes: u32,
    started: Instant,
    /// Bumped on each start, so messages of an earlier process are ignored.
    generation: u64,
    pub(super) stopped: bool,
    _tasks: Vec<Task<()>>,
}

impl TsServer {
    fn new(root: &Path) -> Self {
        TsServer {
            root: root.to_owned(),
            node: None,
            client: None,
            stage: TsStage::FindingNode,
            progress: Progress::default(),
            initialized: false,
            diagnostics: BTreeMap::new(),
            crashes: 0,
            started: Instant::now(),
            generation: next_generation(),
            stopped: false,
            _tasks: Vec::new(),
        }
    }

    /// "TypeScript: Ready", "TypeScript: Needs Node 22" and the like.
    pub fn status(&self) -> String {
        let what = match (&self.stage, self.progress.current()) {
            (TsStage::Ready, Some(work)) => work.label(),
            (stage, _) => stage.label(),
        };
        format!("TypeScript: {what}")
    }

    pub fn ready(&self) -> bool {
        self.initialized && self.client.is_some()
    }

    pub fn busy(&self) -> bool {
        self.stage.busy() || (self.stage == TsStage::Ready && self.progress.is_busy())
    }

    pub fn pid(&self) -> Option<u32> {
        self.client.as_ref().map(Client::pid)
    }
}

fn next_generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A `file:` URI as SnoutCode writes it, however the server wrote it: vtsls writes Windows
/// paths as VS Code does (`file:///c%3A/Users/…`), the documents are kept by
/// `file:///C:/Users/…`.
pub fn canonical_uri(uri: &str) -> String {
    use std::str::FromStr as _;
    Uri::from_str(uri)
        .ok()
        .and_then(|u| snout_lang::uri::to_path(&u))
        .map_or_else(|| uri.to_owned(), |p| snout_lang::uri::from_path(&p).as_str().to_owned())
}

/// The language id a file goes to vtsls with, when it is one vtsls serves.
pub fn language_id(path: &Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("typescriptreact"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "jsx" => Some("javascriptreact"),
        _ => None,
    }
}

/// The TypeScript project `file` belongs to: the nearest folder above it with a
/// `tsconfig.json`, `jsconfig.json` or `package.json`, not above `stop` (the open project);
/// else `stop` itself when the file lies in it. Never inside `node_modules`.
pub fn project_root(host: &dyn Host, file: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    if file.components().any(|c| c.as_os_str() == "node_modules") {
        return None;
    }
    let mut at = file.parent();
    while let Some(d) = at {
        if ["tsconfig.json", "jsconfig.json", "package.json"].iter().any(|n| host.is_file(&d.join(n))) {
            return Some(d.to_owned());
        }
        if stop.is_some_and(|s| s == d) {
            return Some(d.to_owned());
        }
        at = d.parent();
    }
    None
}

/// Where vtsls is installed on a host.
pub fn server_dir(info: &snout_host::HostInfo) -> PathBuf {
    node::downloads_dir(info).with_file_name("vtsls").join(VTSLS_VERSION)
}

/// vtsls's program in `dir`, once it is installed there.
pub fn server_script(host: &dyn Host, dir: &Path) -> Option<PathBuf> {
    let script = dir.join("node_modules/@vtsls/language-server/bin/vtsls.js");
    host.is_file(&script).then_some(script)
}

/// npm's own program beside `node` (`lib/node_modules/npm/bin/npm-cli.js`), run with that
/// Node, so neither a shell nor `PATH` decides which npm it is.
pub fn npm_cli(host: &dyn Host, node: &Node) -> Option<PathBuf> {
    let bin = node.bin_dir();
    let prefix = bin.parent().unwrap_or(bin);
    [prefix.join("lib/node_modules/npm/bin/npm-cli.js"), bin.join("node_modules/npm/bin/npm-cli.js")]
        .into_iter()
        .find(|p| host.is_file(p))
}

/// `PATH` with `node`'s folder first, for the programs a Node program starts.
pub fn path_with(host: &dyn Host, node: &Node) -> String {
    let sep = if host.info().is_windows() { ";" } else { ":" };
    let rest = host.info().var("PATH").unwrap_or_default();
    format!("{}{sep}{rest}", node.bin_dir().display())
}

/// Install vtsls into `dir` with `node`'s npm; its program.
pub fn install_server(host: &dyn Host, node: &Node, dir: &Path) -> anyhow::Result<PathBuf> {
    let npm = npm_cli(host, node).ok_or_else(|| anyhow::anyhow!("no npm beside {}", node.bin.display()))?;
    host.create_dir_all(dir)?;
    let package = format!("@vtsls/language-server@{VTSLS_VERSION}");
    let cmd = Cmd::new(&node.bin)
        .arg(&npm)
        .args(["install", "--prefix"])
        .arg(dir)
        .args(["--no-audit", "--no-fund", "--no-save", "--loglevel=error", &package])
        .cwd(dir)
        .env("PATH", path_with(host, node));
    let out = host.run(&cmd, None)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("npm install {package} failed: {}", err.trim());
    }
    server_script(host, dir).ok_or_else(|| anyhow::anyhow!("npm installed {package}, but its program is missing"))
}

/// The command that runs vtsls for the project at `root`.
pub fn server_command(host: &dyn Host, node: &Node, script: &Path, root: &Path) -> Cmd {
    Cmd::new(&node.bin).arg(script).arg("--stdio").cwd(root).env("PATH", path_with(host, node))
}

/// vtsls's settings, by section, as `workspace/configuration` asks for them.
fn settings() -> Value {
    let language = json!({
        "suggest": { "completeFunctionCalls": false, "autoImports": true },
        "inlayHints": { "parameterNames": { "enabled": "literals" } },
        "updateImportsOnFileMove": { "enabled": "always" },
        "preferences": { "importModuleSpecifier": "shortest", "quoteStyle": "auto" },
        "format": { "enable": true }
    });
    json!({
        "typescript": language,
        "javascript": language,
        "vtsls": {
            "autoUseWorkspaceTsdk": true,
            "enableMoveToFileCodeAction": true
        }
    })
}

/// The value of `section` (`typescript.inlayHints`) in [`settings`].
fn setting(section: &str) -> Value {
    let mut at = settings();
    for part in section.split('.').filter(|p| !p.is_empty()) {
        at = at.get(part).cloned().unwrap_or(Value::Null);
    }
    at
}

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
        "initializationOptions": settings()
    })
}

impl LangHub {
    /// The TypeScript server of `root`, once it answers requests.
    pub fn ts_client(&self, root: &Path) -> Option<Client> {
        self.ts.get(root).filter(|s| s.ready()).and_then(|s| s.client.clone())
    }

    /// An editor shows the TypeScript or JavaScript file `path` of the project at `root`:
    /// tell its server (starting it). Returns the document's URI and project, for the
    /// editor's providers.
    pub fn open_ts_doc(
        &mut self,
        editor: &WeakEntity<EditorPanel>,
        path: &Path,
        root: &Path,
        text: Rope,
        cx: &mut Context<Self>,
    ) -> Option<(Uri, PathBuf, Vec<Diagnostic>)> {
        let language = language_id(path)?;
        let uri = snout_lang::uri::from_path(path);
        let key = uri_key(&uri);
        if let Some(doc) = self.docs.get_mut(&key) {
            doc.editors.retain(|e| e.upgrade().is_some());
            if !doc.editors.iter().any(|e| e.entity_id() == editor.entity_id()) {
                doc.editors.push(editor.clone());
            }
            let diags = self.ts.get(&doc.root).and_then(|s| s.diagnostics.get(&key)).cloned();
            return Some((uri, doc.root.clone(), diags.unwrap_or_default()));
        }
        self.ensure_ts_server(root, cx);
        if let Some(client) = self.ts_client(root) {
            client.notify_raw(
                "textDocument/didOpen",
                json!({ "textDocument": { "uri": uri.as_str(), "languageId": language, "version": 1, "text": text.to_string() } }),
            );
        }
        self.docs.insert(
            key,
            Doc {
                root: root.to_owned(),
                uri: uri.clone(),
                text,
                version: 1,
                editors: vec![editor.clone()],
                virtual_doc: false,
                language,
                ts: true,
            },
        );
        Some((uri, root.to_owned(), Vec::new()))
    }

    /// Start the TypeScript server of the project at `root` unless it runs.
    pub fn ensure_ts_server(&mut self, root: &Path, cx: &mut Context<Self>) {
        if !self.enabled || self.ts.contains_key(root) {
            return;
        }
        let server = TsServer::new(root);
        let generation = server.generation;
        self.ts.insert(root.to_owned(), server);
        let (host, root_buf) = (self.host.clone(), root.to_owned());
        let project = self.model.read(cx).project_root();
        let look = cx.background_executor().spawn(async move {
            let wanted = node::wanted(&*host, &root_buf, project.as_deref());
            let all = node::discover(&*host);
            match node::choose(&wanted, &all) {
                Some(n) => Ok(n.clone()),
                None => Err(wanted),
            }
        });
        let root_buf = root.to_owned();
        cx.spawn(async move |this, cx| {
            let found = look.await;
            let _ = this.update(cx, |this, cx| this.ts_node_found(&root_buf, generation, found, cx));
        })
        .detach();
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    fn ts_node_found(
        &mut self,
        root: &Path,
        generation: u64,
        found: Result<Node, node::Wanted>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.ts.get_mut(root).filter(|s| s.generation == generation && !s.stopped) else { return };
        match found {
            Ok(n) => {
                eprintln!("[ts] {} uses Node {} ({})", root.display(), n.version, n.bin.display());
                server.node = Some(n);
                self.ts_spawn(root, cx);
            }
            Err(wanted) => {
                let what = wanted
                    .exact
                    .clone()
                    .or(wanted.major.map(|m| m.to_string()))
                    .or(wanted.min_major.map(|m| format!("{m} or newer")))
                    .unwrap_or_default();
                eprintln!("[ts] no Node fits {} (asks for {what:?})", root.display());
                server.stage = TsStage::NeedsNode(what.clone());
                if !self.node_prompt.as_ref().is_some_and(|p| p.dismissed || p.progress.is_some()) {
                    let label = if what.is_empty() { node::LTS_MAJOR.to_string() } else { what };
                    self.node_prompt = Some(NodePrompt {
                        root: root.to_owned(),
                        wanted,
                        label,
                        progress: None,
                        error: None,
                        dismissed: false,
                    });
                }
            }
        }
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// Start vtsls for `root` on its Node, installing it first if needed.
    fn ts_spawn(&mut self, root: &Path, cx: &mut Context<Self>) {
        let Some(server) = self.ts.get_mut(root) else { return };
        let Some(node) = server.node.clone() else { return };
        server.generation = next_generation();
        let generation = server.generation;
        let host = self.host.clone();
        let dir = self.ts_install_dir.clone().unwrap_or_else(|| server_dir(host.info()));
        let installed = server_script(&*host, &dir);
        server.stage = if installed.is_some() { TsStage::Starting } else { TsStage::Installing };
        let root_buf = root.to_owned();
        let start = cx.background_executor().spawn(async move {
            let script = match installed {
                Some(s) => s,
                None => install_server(&*host, &node, &dir)
                    .map_err(|e| format!("The TypeScript language server could not be installed: {e:#}"))?,
            };
            let command = server_command(&*host, &node, &script, &root_buf);
            Client::spawn(&*host, &command).map_err(|e| format!("The TypeScript language server did not start: {e}"))
        });
        let root_buf = root.to_owned();
        cx.spawn(async move |this, cx| {
            let started = start.await;
            let _ = this.update(cx, |this, cx| {
                this.ts_started(&root_buf, generation, started, cx);
                cx.emit(HubEvent::Changed);
                cx.notify();
            });
        })
        .detach();
    }

    fn ts_started(
        &mut self,
        root: &Path,
        generation: u64,
        started: Result<(Client, snout_lang::Messages), String>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) =
            self.ts.get_mut(root).filter(|s| s.generation == generation && s.client.is_none() && !s.stopped)
        else {
            return;
        };
        let (client, events) = match started {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ts] {e}");
                server.stage = TsStage::Failed(e);
                return;
            }
        };
        eprintln!("[ts] started vtsls for {} (pid {})", root.display(), client.pid());
        server.client = Some(client.clone());
        server.stage = TsStage::Starting;
        server.initialized = false;
        server.started = Instant::now();
        server.diagnostics.clear();
        server.progress.clear();
        let root_buf = root.to_owned();
        let listen = cx.spawn(async move |this, cx| {
            while let Ok(message) = events.recv().await {
                if this.update(cx, |this, cx| this.ts_on_message(&root_buf, generation, message, cx)).is_err() {
                    return;
                }
            }
        });
        let init = client.request_raw("initialize", initialize_params(root));
        let root_buf = root.to_owned();
        let initialize = cx.spawn(async move |this, cx| {
            let result = init.await;
            let _ = this.update(cx, |this, cx| {
                let Some(server) = this.ts.get_mut(&root_buf).filter(|s| s.generation == generation) else { return };
                let Some(client) = server.client.clone() else { return };
                if let Err(e) = result {
                    eprintln!("[ts] initialize failed: {e}");
                    server.stage = TsStage::Failed(format!("The TypeScript language server failed: {e}"));
                    cx.notify();
                    return;
                }
                client.notify_raw("initialized", json!({}));
                client.notify_raw("workspace/didChangeConfiguration", json!({ "settings": settings() }));
                server.initialized = true;
                server.stage = TsStage::Ready;
                for doc in this.docs.values_mut().filter(|d| d.ts && d.root == root_buf) {
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
            });
        });
        server._tasks = vec![listen, initialize];
    }

    fn ts_on_message(&mut self, root: &Path, generation: u64, message: ServerMessage, cx: &mut Context<Self>) {
        if !self.ts.get(root).is_some_and(|s| s.generation == generation) {
            return;
        }
        match message {
            ServerMessage::Stderr(line) => eprintln!("[ts] {line}"),
            ServerMessage::Notification { method, params } => self.ts_on_notification(root, &method, params, cx),
            ServerMessage::Request { id, method, params } => self.ts_on_request(root, id, &method, params, cx),
            ServerMessage::Exited(code) => self.ts_on_exit(root, code, cx),
        }
    }

    fn ts_on_notification(&mut self, root: &Path, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(server) = self.ts.get_mut(root) else { return };
        match method {
            "textDocument/publishDiagnostics" => {
                let Some(uri) = params.get("uri").and_then(Value::as_str).map(canonical_uri) else { return };
                let list: Vec<Diagnostic> =
                    serde_json::from_value(params.get("diagnostics").cloned().unwrap_or_default()).unwrap_or_default();
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
            _ => {}
        }
    }

    fn ts_on_request(&mut self, root: &Path, id: Value, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(client) = self.ts.get(root).and_then(|s| s.client.clone()) else { return };
        match method {
            "workspace/configuration" => {
                let items = params.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                let answers: Vec<Value> = items
                    .iter()
                    .map(|i| i.get("section").and_then(Value::as_str).map_or_else(settings, setting))
                    .collect();
                client.respond(id, Ok(Value::Array(answers)));
            }
            "workspace/applyEdit" => match serde_json::from_value::<lsp_types::WorkspaceEdit>(params["edit"].clone()) {
                Ok(edit) => cx.emit(HubEvent::ApplyEdit { edit, reply: Some((root.to_owned(), id)) }),
                Err(e) => client.respond(id, Ok(json!({ "applied": false, "failureReason": e.to_string() }))),
            },
            "window/workDoneProgress/create" => {}
            _ => client.respond(id, Ok(Value::Null)),
        }
    }

    fn ts_on_exit(&mut self, root: &Path, code: Option<i32>, cx: &mut Context<Self>) {
        let Some(server) = self.ts.get_mut(root) else { return };
        eprintln!("[ts] vtsls for {} exited ({code:?})", root.display());
        if server.stopped {
            return;
        }
        server.client = None;
        server.initialized = false;
        server.progress.clear();
        server.diagnostics.clear();
        if server.started.elapsed() > Duration::from_secs(300) {
            server.crashes = 0;
        }
        server.crashes += 1;
        if server.crashes > 5 {
            server.stage = TsStage::Failed("The TypeScript language server keeps stopping".into());
        } else {
            server.stage = TsStage::Restarting;
            let delay = Duration::from_millis(200) * (1 << server.crashes.min(6));
            let root = root.to_owned();
            server._tasks.push(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(delay).await;
                let _ = this.update(cx, |this, cx| {
                    if this.ts.get(&root).is_some_and(|s| s.stage == TsStage::Restarting) {
                        this.ts_spawn(&root, cx);
                        cx.notify();
                    }
                });
            }));
        }
        let uris: Vec<String> =
            self.docs.iter().filter(|(_, d)| d.ts && d.root == root).map(|(k, _)| k.clone()).collect();
        for uri in uris {
            if let Some(doc) = self.docs.get(&uri) {
                for editor in doc.editors.iter().filter_map(|e| e.upgrade()) {
                    editor.update(cx, |e, cx| e.set_diagnostics(Vec::new(), cx));
                }
            }
        }
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// Download the Node the bar offers, then start the TypeScript servers that waited for it.
    pub fn download_node(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.node_prompt.as_mut() else { return };
        if prompt.progress.is_some() {
            return;
        }
        let progress = super::java::DownloadProgress::default();
        prompt.progress = Some(progress.clone());
        prompt.error = None;
        let (host, wanted) = (self.host.clone(), prompt.wanted.clone());
        let dir = node::downloads_dir(host.info());
        let watch = progress.clone();
        let fetch = cx.background_executor().spawn(async move { node::download(&*host, &wanted, &dir, &watch) });
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
            let result = fetch.await;
            drop(ticker);
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(n) => {
                        eprintln!("[ts] downloaded Node {} to {}", n.version, n.bin.display());
                        this.node_prompt = None;
                        let waiting: Vec<PathBuf> = this
                            .ts
                            .values()
                            .filter(|s| matches!(s.stage, TsStage::NeedsNode(_)))
                            .map(|s| s.root.clone())
                            .collect();
                        for root in waiting {
                            this.restart_ts(&root, cx);
                        }
                    }
                    Err(e) => {
                        eprintln!("[ts] Node could not be downloaded: {e:#}");
                        if let Some(p) = this.node_prompt.as_mut() {
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

    /// Not now: hide the Node bar for this session.
    pub fn dismiss_node(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.node_prompt.as_mut()
            && p.progress.is_none()
        {
            p.dismissed = true;
        }
        cx.notify();
    }

    /// The TypeScript server the status bar speaks of: the one of the project `path` lies
    /// in, else the only one.
    pub fn ts_server_for(&self, path: Option<&Path>) -> Option<&TsServer> {
        let by_path = path
            .and_then(|p| self.ts.values().filter(|s| p.starts_with(&s.root)).max_by_key(|s| s.root.as_os_str().len()));
        by_path.or_else(|| if self.ts.len() == 1 { self.ts.values().next() } else { None })
    }

    /// Stop the TypeScript server of `root` and forget it.
    pub fn stop_ts_server(&mut self, root: &Path) {
        let Some(mut server) = self.ts.remove(root) else { return };
        server.stopped = true;
        for task in server._tasks.drain(..) {
            task.detach();
        }
        let Some(client) = server.client.take() else { return };
        eprintln!("[ts] stopping vtsls for {} (pid {})", root.display(), client.pid());
        std::thread::spawn(move || client.shutdown_blocking(Duration::from_secs(3)));
    }

    /// Stop the TypeScript server of `root` and start it again (Restart in the status bar).
    pub fn restart_ts(&mut self, root: &Path, cx: &mut Context<Self>) {
        self.stop_ts_server(root);
        self.ensure_ts_server(root, cx);
    }
}
