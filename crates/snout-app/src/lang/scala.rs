//! Scala through Metals (Apache 2.0), the Scala language server: one per Gradle build that
//! holds Scala, started when one of its Scala files opens. Metals imports the build through
//! Bloop (the import it offers is accepted for the user), compiles it, and answers what jdtls
//! answers for Java: completion, hover, definitions (into the build's Java too), references,
//! rename, diagnostics, formatting and code actions. A shared source folder that no build
//! owns (several builds compile it) goes to the server of one of the builds that compile it,
//! the one already running when there is one.
//!
//! Metals is fetched on first use with Coursier (run on a JDK, from SnoutCode's data folder
//! into a cache of its own), at the release whose presentation compiler still covers Scala
//! 2.11.12. It runs on a JDK 17 or newer; the build's own JDK compiles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use gpui::{App, Context, Task, WeakEntity};
use gpui_component::input::Rope;
use lsp_types::{Diagnostic, Uri};
use serde_json::{Value, json};
use snout_host::{Cmd, Host};
use snout_lang::progress::Progress;
use snout_lang::{Client, ServerMessage};

use super::hub::{Doc, DocKind, HubEvent, LangHub, uri_key};
use super::java::{self, DownloadProgress};
use crate::editor::EditorPanel;

/// The Metals release SnoutCode runs: the newest whose presentation compiler (`mtags`) is
/// published for Scala 2.11.12 as well as the later Scala 2 and 3 releases.
pub const METALS_VERSION: &str = "1.6.7";
/// The Coursier launcher that fetches it, and its SHA-256.
const COURSIER_VERSION: &str = "2.1.26";
const COURSIER_SHA256: &str = "d3135830850eddfb9998be66ab4c79478ee6cff56012bb5a177532b1df47c7cc";
/// The oldest Java Metals runs on.
pub const MIN_JAVA: u32 = 17;

/// Whether `path` is a Scala file Metals serves.
pub fn is_scala(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e, "scala" | "sc"))
}

/// Where a Scala server is, for the status bar.
#[derive(Clone, Debug, PartialEq)]
pub enum ScalaStage {
    /// Finding a JDK to run Metals on.
    FindingJdk,
    /// No JDK 17 or newer was found.
    NeedsJdk,
    /// Fetching Metals (first use).
    Installing,
    Starting,
    Ready,
    Restarting,
    Failed(String),
}

impl ScalaStage {
    pub fn label(&self) -> String {
        match self {
            ScalaStage::FindingJdk => ts!("editor-finding-a-jdk").into(),
            ScalaStage::NeedsJdk => t!("editor-scala-needs-jdk", major = MIN_JAVA.to_string()),
            ScalaStage::Installing => ts!("editor-installing-the-language-server").into(),
            ScalaStage::Starting => ts!("console-starting").into(),
            ScalaStage::Ready => ts!("model-ready").into(),
            ScalaStage::Restarting => ts!("editor-restarting").into(),
            ScalaStage::Failed(why) => why.clone(),
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self, ScalaStage::FindingJdk | ScalaStage::Installing | ScalaStage::Starting | ScalaStage::Restarting)
    }
}

/// One Metals server.
pub struct ScalaServer {
    pub root: PathBuf,
    pub(super) client: Option<Client>,
    pub stage: ScalaStage,
    pub progress: Progress,
    /// What Metals last put in its status bar item (`Importing build`, `Compiling bamboo-web`),
    /// and whether it is still at it.
    pub status_text: Option<(String, bool)>,
    pub(super) initialized: bool,
    /// What the server published, by document URI.
    pub diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    /// The JDK the build compiles with (`metals.javaHome`), when the model knows it.
    build_jdk: Option<PathBuf>,
    crashes: u32,
    /// Whether this start asked Metals to import again after an import that failed before.
    reimported: bool,
    started: Instant,
    /// Bumped on each start, so messages of an earlier process are ignored.
    generation: u64,
    pub(super) stopped: bool,
    _tasks: Vec<Task<()>>,
}

impl ScalaServer {
    fn new(root: &Path, build_jdk: Option<PathBuf>) -> Self {
        ScalaServer {
            root: root.to_owned(),
            client: None,
            stage: ScalaStage::FindingJdk,
            progress: Progress::default(),
            status_text: None,
            initialized: false,
            diagnostics: BTreeMap::new(),
            build_jdk,
            crashes: 0,
            reimported: false,
            started: Instant::now(),
            generation: next_generation(),
            stopped: false,
            _tasks: Vec::new(),
        }
    }

    /// "Scala: Ready", "Scala: Importing build" and the like.
    pub fn status(&self) -> String {
        let what = match (&self.stage, &self.status_text, self.progress.current()) {
            (ScalaStage::Ready, _, Some(work)) => work.label(),
            (ScalaStage::Ready, Some((text, _)), None) => text.clone(),
            (stage, _, _) => stage.label(),
        };
        t!("editor-scala", what = what.to_string())
    }

    pub fn ready(&self) -> bool {
        self.initialized && self.client.is_some()
    }

    /// Not running, and not going to start on its own: no JDK, a failed download or start,
    /// or stopped too often.
    pub fn given_up(&self) -> bool {
        self.client.is_none() && matches!(self.stage, ScalaStage::NeedsJdk | ScalaStage::Failed(_))
    }

    pub fn busy(&self) -> bool {
        self.stage.busy()
            || (self.stage == ScalaStage::Ready
                && (self.progress.is_busy() || self.status_text.as_ref().is_some_and(|(_, busy)| *busy)))
    }

    pub fn pid(&self) -> Option<u32> {
        self.client.as_ref().map(Client::pid)
    }
}

fn next_generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Where Metals and its Coursier are kept on a host: beside SnoutCode's JDKs.
pub fn metals_dir(info: &snout_host::HostInfo) -> PathBuf {
    java::downloads_dir(info).with_file_name("metals")
}

/// The classpath Metals runs with, once it was fetched into `dir` and its jars are all
/// still there.
pub fn installed_classpath(host: &dyn Host, dir: &Path) -> Option<String> {
    let text = host.read_to_string(&dir.join(METALS_VERSION).join("classpath.txt")).ok()?;
    let cp = text.trim().to_owned();
    let sep = if host.info().is_windows() { ';' } else { ':' };
    let jars: Vec<&str> = cp.split(sep).filter(|s| !s.is_empty()).collect();
    (!jars.is_empty() && jars.iter().all(|j| host.is_file(Path::new(j)))).then_some(cp)
}

/// Fetch Metals into `dir` with Coursier (downloading Coursier first), running `java`;
/// the classpath to run it with. Blocking; run it off the main thread.
pub fn install(host: &dyn Host, java: &Path, dir: &Path, progress: &DownloadProgress) -> anyhow::Result<String> {
    host.create_dir_all(dir)?;
    let coursier = dir.join(format!("coursier-{COURSIER_VERSION}.jar"));
    if !host.is_file(&coursier) {
        let url = format!("https://github.com/coursier/coursier/releases/download/v{COURSIER_VERSION}/coursier.jar");
        let part = dir.join(format!(".coursier-{COURSIER_VERSION}.jar.part"));
        java::fetch_file(host, &url, &part, progress).context("download Coursier")?;
        if java::sha256_on(host, &part)? != COURSIER_SHA256 {
            let _ = host.remove_file(&part);
            anyhow::bail!("the Coursier download is damaged (its checksum does not match)");
        }
        host.rename(&part, &coursier)?;
    }
    let cmd = Cmd::new(java)
        .arg("-jar")
        .arg(&coursier)
        .args(["fetch", &format!("org.scalameta:metals_2.13:{METALS_VERSION}"), "--classpath"])
        .cwd(dir)
        .env("COURSIER_CACHE", dir.join("cache"))
        .env("COURSIER_PROGRESS", "false");
    let out = host.run(&cmd, None)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("Coursier could not fetch Metals {METALS_VERSION}: {}", err.trim());
    }
    let cp = String::from_utf8_lossy(&out.stdout).lines().last().unwrap_or_default().trim().to_owned();
    if cp.is_empty() {
        anyhow::bail!("Coursier fetched no Metals");
    }
    let version = dir.join(METALS_VERSION);
    host.create_dir_all(&version)?;
    host.write(&version.join("classpath.txt"), cp.as_bytes())?;
    installed_classpath(host, dir).context("Metals was fetched, but its jars are missing")
}

/// The JDK Metals runs on: the build's when it is new enough, else Java 21, else 17, else
/// the newest found that is new enough.
pub fn runtime_jdk(found: &[java::Jdk], build: Option<&Path>) -> Option<java::Jdk> {
    let fits = |j: &&java::Jdk| j.major >= MIN_JAVA;
    if let Some(b) = build
        && let Some(j) = found.iter().filter(fits).find(|j| j.home == b)
    {
        return Some(j.clone());
    }
    let rank = |j: &java::Jdk| match j.major {
        21 => (2, 0),
        17 => (1, 0),
        m => (0, m),
    };
    found.iter().filter(fits).max_by_key(|j| rank(j)).cloned()
}

/// The JDK the build's Gradle runs on, which Metals imports and compiles with: the one the
/// project uses when the build's Gradle runs on it, else the one the build declares or the
/// nearest that fits (as for the Java engine). A JDK found nowhere is kept as it is.
pub fn build_jdk_for(req: &java::Requirement, found: &[java::Jdk], used: Option<PathBuf>) -> Option<PathBuf> {
    let known = used.as_ref().and_then(|u| found.iter().find(|j| &j.home == u));
    match (used, known) {
        (Some(u), Some(j)) if req.fits(j.major) => Some(u),
        (used, None) if used.is_some() => used,
        _ => java::choose(req, found).map(|j| j.home.clone()),
    }
}

/// The property on Metals's command line that names its build, for the Processes window.
pub const ROOT_MARK: &str = "-Dsnout.metals.root=";

/// The command that runs Metals for the build at `root`.
pub fn server_command(host: &dyn Host, jdk: &Path, classpath: &str, root: &Path, build_jdk: &Path) -> Cmd {
    Cmd::new(java::java_bin(host.info(), jdk))
        // Without a limit the JVM takes up to a quarter of the machine's memory, and Metals
        // fills it while it indexes; 2 GB is what its own guide gives a large build.
        .args([
            "-Xss4m",
            "-Xms100m",
            "-Xmx2G",
            "-XX:+UseG1GC",
            "-XX:+UseStringDeduplication",
            "-Dmetals.client=snout-code",
        ])
        .arg(format!("{ROOT_MARK}{}", root.display()))
        .arg("-cp")
        .arg(classpath)
        .arg("scala.meta.metals.Main")
        .cwd(root)
        // Gradle, which Metals runs to import the build, and Bloop compile on the build's JDK.
        .env("JAVA_HOME", build_jdk)
}

/// The folders Metals and Bloop write into the build (`.metals`, `.bloop`, `.bsp`), left out
/// of Git in the repository's own exclude file (`.git/info/exclude`, never committed) unless
/// it already leaves them out.
pub fn exclude_from_git(host: &dyn Host, root: &Path) {
    let Some(repo) = root.ancestors().find(|d| host.is_dir(&d.join(".git"))) else { return };
    let file = repo.join(".git/info/exclude");
    let text = host.read_to_string(&file).unwrap_or_default();
    let missing: Vec<&str> =
        [".metals/", ".bloop/", ".bsp/"].into_iter().filter(|p| !text.lines().any(|l| l.trim() == *p)).collect();
    if missing.is_empty() {
        return;
    }
    let mut add = String::new();
    if !text.is_empty() && !text.ends_with('\n') {
        add.push('\n');
    }
    add.push_str("# Metals and Bloop (SnoutCode's Scala support)\n");
    for p in missing {
        add.push_str(p);
        add.push('\n');
    }
    let _ = host.create_dir_all(&repo.join(".git/info"));
    let _ = host.append(&file, add.as_bytes());
}

/// Metals's settings (the `metals` section).
fn settings(build_jdk: Option<&Path>) -> Value {
    let mut s = json!({
        "superMethodLensesEnabled": false,
        "showImplicitArguments": false,
        "showInferredType": "false",
        "enableSemanticHighlighting": false,
        "defaultBspToBuildTool": false,
    });
    if let Some(home) = build_jdk {
        s["javaHome"] = json!(home);
    }
    s
}

fn initialize_params(root: &Path, build_jdk: Option<&Path>) -> Value {
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
            "compilerOptions": {
                "isCompletionItemDetailEnabled": true,
                "isCompletionItemDocumentationEnabled": true,
                "isHoverDocumentationEnabled": true,
                "isSignatureHelpDocumentationEnabled": true,
                "snippetAutoIndent": false
            },
            "statusBarProvider": "on",
            "doctorProvider": "json",
            "icons": "none",
            "isExitOnShutdown": true,
            "isHttpEnabled": false,
            "inputBoxProvider": false,
            "quickPickProvider": false,
            "executeClientCommandProvider": false,
            "decorationProvider": false,
            "inlineDecorationProvider": false,
            "didFocusProvider": false,
            "slowTaskProvider": false,
            "treeViewProvider": false,
            "testExplorerProvider": false,
            "openFilesOnRenameProvider": false,
            "debuggingProvider": false,
            "disableColorOutput": true
        },
        "settings": { "metals": settings(build_jdk) }
    })
}

/// What Metals's status text says, without its icons (`$(sync~spin) Importing build`), and
/// whether it shows work going on.
pub fn status_text(text: &str) -> (String, bool) {
    let busy = text.contains("~spin") || text.contains("$(loading");
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("$(") {
        out.push_str(&rest[..at]);
        rest = rest[at..].find(')').map_or("", |end| &rest[at + end + 1..]);
    }
    out.push_str(rest);
    (out.split_whitespace().collect::<Vec<_>>().join(" "), busy)
}

/// The answer to a question Metals asks (`window/showMessageRequest`): its offer to import
/// the build (and to import it again when the build changed) is taken, as is Bloop when it
/// asks which build server to use; anything else is left unanswered.
/// Whether a Metals log line says it skips the import because the last one failed.
pub fn skipped_failed_import(line: &str) -> bool {
    line.contains("skipping build import with status 'Failed'")
}

pub fn answer(params: &Value) -> Value {
    let actions = params.get("actions").and_then(Value::as_array).cloned().unwrap_or_default();
    let title = |a: &Value| a.get("title").and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
    let wanted = ["import build", "import changes", "bloop"];
    wanted
        .iter()
        .find_map(|w| actions.iter().find(|a| title(a) == *w || title(a).starts_with(w)))
        .cloned()
        .unwrap_or(Value::Null)
}

impl LangHub {
    /// The Scala server of `root`, once it answers requests.
    pub fn scala_client(&self, root: &Path) -> Option<Client> {
        self.scala.get(root).filter(|s| s.ready()).and_then(|s| s.client.clone())
    }

    /// Ask every Metals that answers requests (Go to Class and Go to Symbol search the Scala
    /// of every build with one).
    pub fn scala_requests(
        &self,
        method: &str,
        params: &Value,
    ) -> Vec<futures::future::BoxFuture<'static, anyhow::Result<Value>>> {
        use futures::FutureExt as _;
        self.scala
            .values()
            .filter(|s| s.ready())
            .filter_map(|s| s.client.clone())
            .map(|c| {
                let call = c.request_raw(method, params.clone());
                let method = method.to_owned();
                async move { call.await.map_err(|e| anyhow::anyhow!("{method}: {e}")) }.boxed()
            })
            .collect()
    }

    /// The build whose Metals serves the Scala file `path`: its own Gradle build (`build`),
    /// else, for a shared source folder, a build that compiles it (one whose server runs,
    /// when there is one).
    pub fn scala_root_for(&self, path: &Path, build: Option<PathBuf>, cx: &App) -> Option<PathBuf> {
        let project = self.model.read(cx).project_root();
        if let Some(b) = build.filter(|b| project.as_deref().is_none_or(|p| b.starts_with(p))) {
            return Some(b);
        }
        let model = crate::project_model::store(cx).and_then(|s| s.read(cx).model_for(project.as_deref()?))?;
        let shared = model.shared.iter().find(|s| s.roots.iter().any(|r| path.starts_with(&r.dir)))?;
        let candidates: Vec<&Path> = shared
            .modules
            .iter()
            .filter_map(|n| model.modules.iter().find(|m| &m.name == n))
            .map(|m| m.content_root.as_path())
            .collect();
        let running = candidates.iter().find_map(|c| self.scala.keys().find(|r| c.starts_with(r)));
        running.cloned().or_else(|| candidates.first().map(|c| c.to_path_buf()))
    }

    /// The JDK the build at `root` compiles with, as the model or its Java engine knows it.
    fn build_jdk_of(&self, root: &Path, cx: &App) -> Option<PathBuf> {
        let project = self.model.read(cx).project_root();
        let model = project.and_then(|p| crate::project_model::store(cx).and_then(|s| s.read(cx).model_for(&p)));
        let from_model = model.and_then(|m| {
            let near = |m: &&crate::project_model::Module| m.content_root.starts_with(root) || root == m.content_root;
            m.modules.iter().filter(near).find_map(|m| m.jdk.as_ref().map(|j| j.home.clone()))
        });
        from_model.or_else(|| {
            self.servers
                .values()
                .filter(|s| root.starts_with(&s.root))
                .max_by_key(|s| s.root.as_os_str().len())
                .and_then(|s| s.jdk.as_ref().map(|j| j.home.clone()))
        })
    }

    /// An editor shows the Scala file `path`: tell the Metals of its build (starting it).
    /// Returns the document's URI and build, for the editor's providers.
    pub fn open_scala_doc(
        &mut self,
        editor: &WeakEntity<EditorPanel>,
        path: &Path,
        build: Option<PathBuf>,
        text: Rope,
        cx: &mut Context<Self>,
    ) -> Option<(Uri, PathBuf, Vec<Diagnostic>)> {
        let uri = snout_lang::uri::from_path(path);
        let key = uri_key(&uri);
        if let Some(doc) = self.docs.get_mut(&key) {
            doc.editors.retain(|e| e.upgrade().is_some());
            if !doc.editors.iter().any(|e| e.entity_id() == editor.entity_id()) {
                doc.editors.push(editor.clone());
            }
            let diags = self.scala.get(&doc.root).and_then(|s| s.diagnostics.get(&key)).cloned();
            return Some((uri, doc.root.clone(), diags.unwrap_or_default()));
        }
        let root = self.scala_root_for(path, build, cx)?;
        self.ensure_scala_server(&root, cx);
        if let Some(client) = self.scala_client(&root) {
            client.notify_raw(
                "textDocument/didOpen",
                json!({ "textDocument": { "uri": uri.as_str(), "languageId": "scala", "version": 1, "text": text.to_string() } }),
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
                language: "scala",
                kind: DocKind::Scala,
            },
        );
        Some((uri, root, Vec::new()))
    }

    /// Start the Metals of the build at `root` unless it runs. One given up on (no JDK, a
    /// failed download, too many crashes) is tried again: opening a Scala file of the build
    /// retries.
    pub fn ensure_scala_server(&mut self, root: &Path, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        if let Some(s) = self.scala.get(root) {
            if !s.given_up() {
                return;
            }
            self.stop_scala_server(root);
        }
        let build_jdk = self.build_jdk_of(root, cx);
        let server = ScalaServer::new(root, build_jdk);
        self.scala.insert(root.to_owned(), server);
        self.scala_spawn(root, cx);
        cx.emit(HubEvent::Changed);
        cx.notify();
    }

    /// Find a JDK, fetch Metals if needed, and start it for `root`.
    fn scala_spawn(&mut self, root: &Path, cx: &mut Context<Self>) {
        let Some(server) = self.scala.get_mut(root) else { return };
        server.generation = next_generation();
        let generation = server.generation;
        let build_jdk = server.build_jdk.clone();
        let host = self.host.clone();
        let home = self.home.clone();
        let dir = self.scala_install_dir.clone().unwrap_or_else(|| metals_dir(host.info()));
        let root_buf = root.to_owned();
        let this = cx.weak_entity();
        let start = cx.background_executor().spawn(async move {
            let found = java::discover(&*host, &home);
            let build_jdk = build_jdk_for(&java::requirement(&*host, &root_buf), &found, build_jdk);
            let jdk = runtime_jdk(&found, build_jdk.as_deref()).ok_or(ScalaStage::NeedsJdk)?;
            // One install at a time: builds that start together share one Coursier and one
            // fetch, the later ones finding what the first fetched.
            static INSTALLING: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let classpath = match installed_classpath(&*host, &dir) {
                Some(cp) => cp,
                None => {
                    let _one = INSTALLING.lock().unwrap_or_else(|e| e.into_inner());
                    match installed_classpath(&*host, &dir) {
                        Some(cp) => cp,
                        None => {
                            let java = java::java_bin(host.info(), &jdk.home);
                            install(&*host, &java, &dir, &DownloadProgress::default()).map_err(|e| {
                                ScalaStage::Failed(t!("editor-scala-install-failed", why = format!("{e:#}")))
                            })?
                        }
                    }
                }
            };
            exclude_from_git(&*host, &root_buf);
            let compile_with = build_jdk.unwrap_or_else(|| jdk.home.clone());
            eprintln!(
                "[scala] Metals {METALS_VERSION} for {} on {} (build JDK {})",
                root_buf.display(),
                jdk.home.display(),
                compile_with.display()
            );
            let command = server_command(&*host, &jdk.home, &classpath, &root_buf, &compile_with);
            let (client, events) = Client::spawn(&*host, &command).map_err(|e| {
                ScalaStage::Failed(t!("editor-the-scala-language-server-did-not-start", e = e.to_string()))
            })?;
            Ok((client, events, compile_with))
        });
        // Until the JDK is found, the stage says so; once Metals is being fetched, that.
        let root_now = root.to_owned();
        let host = self.host.clone();
        let dir = self.scala_install_dir.clone().unwrap_or_else(|| metals_dir(host.info()));
        let check = cx.background_executor().spawn(async move { installed_classpath(&*host, &dir).is_some() });
        let marker = cx.spawn(async move |this, cx| {
            let installed = check.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(s) =
                    this.scala.get_mut(&root_now).filter(|s| s.generation == generation && s.client.is_none())
                    && s.stage == ScalaStage::FindingJdk
                {
                    s.stage = if installed { ScalaStage::Starting } else { ScalaStage::Installing };
                    cx.emit(HubEvent::Changed);
                    cx.notify();
                }
            });
        });
        let root_buf = root.to_owned();
        let task = cx.spawn(async move |_, cx| {
            let started = start.await;
            let _ = this.update(cx, |this, cx| {
                this.scala_started(&root_buf, generation, started, cx);
                cx.emit(HubEvent::Changed);
                cx.notify();
            });
        });
        server_tasks(self, root, vec![marker, task]);
    }

    fn scala_started(
        &mut self,
        root: &Path,
        generation: u64,
        started: Result<(Client, snout_lang::Messages, PathBuf), ScalaStage>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) =
            self.scala.get_mut(root).filter(|s| s.generation == generation && s.client.is_none() && !s.stopped)
        else {
            return;
        };
        let (client, events, build_jdk) = match started {
            Ok(s) => s,
            Err(stage) => {
                eprintln!("[scala] {}", stage.label());
                server.stage = stage;
                return;
            }
        };
        eprintln!("[scala] started Metals for {} (pid {})", root.display(), client.pid());
        server.client = Some(client.clone());
        server.stage = ScalaStage::Starting;
        server.initialized = false;
        server.started = Instant::now();
        server.diagnostics.clear();
        server.progress.clear();
        server.status_text = None;
        server.reimported = false;
        server.build_jdk = Some(build_jdk.clone());
        let build_jdk = Some(build_jdk);
        let root_buf = root.to_owned();
        let listen = cx.spawn(async move |this, cx| {
            while let Ok(message) = events.recv().await {
                if this.update(cx, |this, cx| this.scala_on_message(&root_buf, generation, message, cx)).is_err() {
                    return;
                }
            }
        });
        let init = client.request_raw("initialize", initialize_params(root, build_jdk.as_deref()));
        let root_buf = root.to_owned();
        let initialize = cx.spawn(async move |this, cx| {
            let result = init.await;
            let _ = this.update(cx, |this, cx| {
                let Some(server) = this.scala.get_mut(&root_buf).filter(|s| s.generation == generation) else {
                    return;
                };
                let Some(client) = server.client.clone() else { return };
                if let Err(e) = result {
                    eprintln!("[scala] initialize failed: {e}");
                    server.stage = ScalaStage::Failed(t!("editor-the-scala-language-server-failed", e = e.to_string()));
                    cx.notify();
                    return;
                }
                client.notify_raw("initialized", json!({}));
                client.notify_raw(
                    "workspace/didChangeConfiguration",
                    json!({ "settings": { "metals": settings(server.build_jdk.as_deref()) } }),
                );
                server.initialized = true;
                server.stage = ScalaStage::Ready;
                for doc in this.docs.values_mut().filter(|d| d.kind == DocKind::Scala && d.root == root_buf) {
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
        server._tasks.extend([listen, initialize]);
    }

    fn scala_on_message(&mut self, root: &Path, generation: u64, message: ServerMessage, cx: &mut Context<Self>) {
        if !self.scala.get(root).is_some_and(|s| s.generation == generation) {
            return;
        }
        match message {
            ServerMessage::Stderr(line) => eprintln!("[scala] {line}"),
            ServerMessage::Notification { method, params } => self.scala_on_notification(root, &method, params, cx),
            ServerMessage::Request { id, method, params } => self.scala_on_request(root, id, &method, params, cx),
            ServerMessage::Exited(code) => self.scala_on_exit(root, code, cx),
        }
    }

    fn scala_on_notification(&mut self, root: &Path, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(server) = self.scala.get_mut(root) else { return };
        match method {
            "textDocument/publishDiagnostics" => {
                let Some(uri) = params.get("uri").and_then(Value::as_str).map(super::typescript::canonical_uri) else {
                    return;
                };
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
            "metals/status" => {
                let hide = params.get("hide").and_then(Value::as_bool).unwrap_or(false);
                let text = params.get("text").and_then(Value::as_str).unwrap_or_default();
                // The build server's own item (`statusType: bsp`) says which one is connected.
                if params.get("statusType").and_then(Value::as_str) == Some("bsp") {
                    return;
                }
                server.status_text = (!hide && !text.trim().is_empty()).then(|| status_text(text));
                cx.emit(HubEvent::Changed);
                cx.notify();
            }
            "window/showMessage" | "window/logMessage" => {
                let text = params.get("message").and_then(Value::as_str).unwrap_or_default();
                let kind = params.get("type").and_then(Value::as_i64).unwrap_or(4);
                let line = text.lines().next().unwrap_or_default();
                if method == "window/showMessage" || kind <= 2 {
                    eprintln!("[scala] {line}");
                }
                // An error shown to the user (such as a failed import) stays in the status bar
                // until Metals says what it does next.
                if method == "window/showMessage" && kind == 1 {
                    server.status_text = Some((line.trim_end_matches('.').to_owned(), false));
                    cx.notify();
                }
                // Metals keeps a failed import's state and does not import again on its own
                // until the build scripts change, even when what failed was outside them (such
                // as the JDK it ran Gradle on). Each start tries once more.
                if skipped_failed_import(text)
                    && !server.reimported
                    && let Some(client) = server.client.clone()
                {
                    server.reimported = true;
                    eprintln!("[scala] the last import of {} failed: importing again", root.display());
                    drop(client.request_raw("workspace/executeCommand", json!({ "command": "build-import" })));
                }
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

    fn scala_on_request(&mut self, root: &Path, id: Value, method: &str, params: Value, cx: &mut Context<Self>) {
        let Some(server) = self.scala.get(root) else { return };
        let Some(client) = server.client.clone() else { return };
        match method {
            "window/showMessageRequest" => {
                let reply = answer(&params);
                let asked = params.get("message").and_then(Value::as_str).unwrap_or_default();
                eprintln!(
                    "[scala] Metals asks {asked:?}: {}",
                    reply.get("title").and_then(Value::as_str).unwrap_or("-")
                );
                client.respond(id, Ok(reply));
            }
            "workspace/configuration" => {
                let items = params.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                let all = settings(server.build_jdk.as_deref());
                let answers: Vec<Value> = items
                    .iter()
                    .map(|i| match i.get("section").and_then(Value::as_str) {
                        Some("metals") | None => all.clone(),
                        Some(s) => s.strip_prefix("metals.").and_then(|k| all.get(k).cloned()).unwrap_or(Value::Null),
                    })
                    .collect();
                client.respond(id, Ok(Value::Array(answers)));
            }
            "workspace/applyEdit" => match serde_json::from_value::<lsp_types::WorkspaceEdit>(params["edit"].clone()) {
                Ok(edit) => cx.emit(HubEvent::ApplyEdit { edit, reply: Some((DocKind::Scala, root.to_owned(), id)) }),
                Err(e) => client.respond(id, Ok(json!({ "applied": false, "failureReason": e.to_string() }))),
            },
            "window/showDocument" => {
                let uri = params.get("uri").and_then(Value::as_str).and_then(|u| u.parse::<Uri>().ok());
                if let Some(uri) = uri {
                    let at = params
                        .pointer("/selection/start")
                        .and_then(|p| serde_json::from_value(p.clone()).ok())
                        .unwrap_or_default();
                    cx.emit(HubEvent::Open { root: root.to_owned(), uri, at });
                }
                client.respond(id, Ok(json!({ "success": true })));
            }
            // The client answered it already; it comes here only for its token.
            "window/workDoneProgress/create" => {}
            _ => client.respond(id, Ok(Value::Null)),
        }
    }

    fn scala_on_exit(&mut self, root: &Path, code: Option<i32>, cx: &mut Context<Self>) {
        let Some(server) = self.scala.get_mut(root) else { return };
        eprintln!("[scala] Metals for {} exited ({code:?})", root.display());
        if server.stopped {
            return;
        }
        server.client = None;
        server.initialized = false;
        server.progress.clear();
        server.diagnostics.clear();
        server.status_text = None;
        if server.started.elapsed() > Duration::from_secs(300) {
            server.crashes = 0;
        }
        server.crashes += 1;
        if server.crashes > 5 {
            server.stage = ScalaStage::Failed(ts!("editor-the-scala-language-server-keeps-stopping").into());
        } else {
            server.stage = ScalaStage::Restarting;
            let delay = Duration::from_millis(200) * (1 << server.crashes.min(6));
            let root = root.to_owned();
            server._tasks.push(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(delay).await;
                let _ = this.update(cx, |this, cx| {
                    if this.scala.get(&root).is_some_and(|s| s.stage == ScalaStage::Restarting) {
                        this.scala_spawn(&root, cx);
                        cx.notify();
                    }
                });
            }));
        }
        let uris: Vec<String> = self
            .docs
            .iter()
            .filter(|(_, d)| d.kind == DocKind::Scala && d.root == root)
            .map(|(k, _)| k.clone())
            .collect();
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

    /// The Scala server the status bar speaks of: the one serving `path`, else the one of
    /// the build `path` lies in, else the only one.
    pub fn scala_server_for(&self, path: Option<&Path>) -> Option<&ScalaServer> {
        let by_doc = path
            .map(snout_lang::uri::from_path)
            .and_then(|u| self.docs.get(&uri_key(&u)))
            .filter(|d| d.kind == DocKind::Scala)
            .and_then(|d| self.scala.get(&d.root));
        let by_path = || {
            path.and_then(|p| {
                self.scala.values().filter(|s| p.starts_with(&s.root)).max_by_key(|s| s.root.as_os_str().len())
            })
        };
        by_doc.or_else(by_path).or_else(|| if self.scala.len() == 1 { self.scala.values().next() } else { None })
    }

    /// Stop the Metals of `root` and forget it.
    pub fn stop_scala_server(&mut self, root: &Path) {
        let Some(mut server) = self.scala.remove(root) else { return };
        server.stopped = true;
        for task in server._tasks.drain(..) {
            task.detach();
        }
        let Some(client) = server.client.take() else { return };
        eprintln!("[scala] stopping Metals for {} (pid {})", root.display(), client.pid());
        std::thread::spawn(move || client.shutdown_blocking(Duration::from_secs(5)));
    }
}

/// Keep `tasks` with the server of `root` (they end with it).
fn server_tasks(hub: &mut LangHub, root: &Path, tasks: Vec<Task<()>>) {
    if let Some(s) = hub.scala.get_mut(root) {
        s._tasks = tasks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scala_files_are_known_by_their_extension() {
        assert!(is_scala(Path::new("/p/src/main/java/Web.scala")));
        assert!(is_scala(Path::new("/p/build.sc")));
        assert!(!is_scala(Path::new("/p/App.java")));
    }

    #[test]
    fn status_text_loses_its_icons() {
        assert_eq!(status_text("$(sync~spin) Importing build"), ("Importing build".to_owned(), true));
        assert_eq!(status_text("$(rocket)  Indexing  done"), ("Indexing done".to_owned(), false));
        assert_eq!(status_text("Compiling bamboo-web"), ("Compiling bamboo-web".to_owned(), false));
    }

    #[test]
    fn the_build_import_metals_offers_is_taken() {
        let ask = json!({ "message": "New gradle workspace detected", "actions": [
            { "title": "Import build" }, { "title": "Not now" }, { "title": "Don't show again" }
        ] });
        assert_eq!(answer(&ask), json!({ "title": "Import build" }));
        let changed = json!({ "actions": [{ "title": "Import changes" }, { "title": "Dismiss" }] });
        assert_eq!(answer(&changed), json!({ "title": "Import changes" }));
        let other = json!({ "actions": [{ "title": "Create .scalafmt.conf" }, { "title": "Run anyway" }] });
        assert_eq!(answer(&other), Value::Null);
    }

    #[test]
    fn a_failed_import_is_tried_again() {
        assert!(skipped_failed_import("2026.11.18 00:22:35 INFO  skipping build import with status 'Failed'"));
        assert!(!skipped_failed_import("skipping build import with status 'Dismissed'"));
    }

    #[test]
    fn the_build_jdk_is_one_the_builds_gradle_runs_on() {
        let jdk = |major: u32, home: &str| java::Jdk { home: PathBuf::from(home), major, version: major.to_string() };
        let found = [jdk(17, "/j/17"), jdk(21, "/j/21")];
        // Gradle 8.1.1 runs on Java 19 at most, and the build declares 17.
        let req = java::Requirement { declared: Some(17), gradle_max: java::gradle_max_java("8.1.1") };
        assert_eq!(build_jdk_for(&req, &found, Some("/j/21".into())), Some("/j/17".into()));
        assert_eq!(build_jdk_for(&req, &found, None), Some("/j/17".into()));
        assert_eq!(build_jdk_for(&req, &found, Some("/j/17".into())), Some("/j/17".into()));
        assert_eq!(build_jdk_for(&req, &found, Some("/custom".into())), Some("/custom".into()));
    }

    #[test]
    fn metals_runs_on_the_build_jdk_when_new_enough_else_21() {
        let jdk = |major: u32, home: &str| java::Jdk { home: PathBuf::from(home), major, version: major.to_string() };
        let found = [jdk(11, "/j/11"), jdk(17, "/j/17"), jdk(21, "/j/21"), jdk(25, "/j/25")];
        assert_eq!(runtime_jdk(&found, Some(Path::new("/j/17"))).unwrap().major, 17);
        assert_eq!(runtime_jdk(&found, Some(Path::new("/j/11"))).unwrap().major, 21);
        assert_eq!(runtime_jdk(&found, None).unwrap().major, 21);
        assert!(runtime_jdk(&[jdk(11, "/j/11")], None).is_none());
    }

    #[test]
    fn the_folders_metals_writes_are_left_out_of_git_once() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::create_dir_all(repo.join(".git/info")).unwrap();
        std::fs::write(repo.join(".git/info/exclude"), "# local\n.metals/\n").unwrap();
        let build = repo.join("scala/bamboo-web");
        std::fs::create_dir_all(&build).unwrap();
        let host = snout_host::LocalHost::new();
        exclude_from_git(&host, &build);
        exclude_from_git(&host, &build);
        let text = std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
        assert_eq!(text.matches(".metals/").count(), 1);
        assert_eq!(text.matches(".bloop/").count(), 1);
        assert_eq!(text.matches(".bsp/").count(), 1);
    }
}
