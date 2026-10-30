//! The project as the engine's Gradle model describes it (format 2, see `engine/README.md`):
//! its modules with their typed source roots, the source folders several modules share,
//! the libraries, each build's Gradle scripts and the builds that failed to sync.
//!
//! [`ProjectModels`] keeps the model of the open project: it reads the model the Java engine
//! writes when it starts (`<cache>/jdtls/<hash>/model.json`), or runs `snout-engine sync`
//! itself when no engine runs for the project, notices when a Gradle script changes, and
//! syncs again when asked.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{App, Context, Entity, Global, Task};
use serde_json::Value;
use snout_host::{Cmd, Host, HostRef, Stdio};

use crate::lang::LangHub;
use crate::model::Model;

/// What a source root holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RootKind {
    Main,
    Test,
    Resources,
    TestResources,
    Generated,
}

impl RootKind {
    fn parse(kind: &str) -> Option<Self> {
        Some(match kind {
            "main" => Self::Main,
            "test" => Self::Test,
            "resources" => Self::Resources,
            "test-resources" => Self::TestResources,
            "generated" => Self::Generated,
            _ => return None,
        })
    }

    /// What the tree's tooltip calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Main => "Sources",
            Self::Test => "Test sources",
            Self::Resources => "Resources",
            Self::TestResources => "Test resources",
            Self::Generated => "Generated sources",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRoot {
    pub dir: PathBuf,
    pub kind: RootKind,
    pub exists: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jdk {
    pub home: PathBuf,
    pub version: String,
    pub level: Option<String>,
}

impl Jdk {
    /// "JDK 21.0.13".
    pub fn label(&self) -> String {
        format!("JDK {}", self.version)
    }
}

/// One Gradle project of any build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub build: String,
    pub gradle_path: String,
    pub content_root: PathBuf,
    /// The source roots it owns (not those another module or a shared folder holds).
    pub roots: Vec<SourceRoot>,
    pub excluded: Vec<PathBuf>,
    /// The shared source folders it compiles, by name.
    pub uses: Vec<String>,
    pub jdk: Option<Jdk>,
    /// Its build failed to sync now; the module is from the last sync that worked.
    pub stale: Option<String>,
    /// It is a module of a Maven build.
    pub maven: bool,
}

impl Module {
    /// The build's name, when it says more than the module's own.
    pub fn build_note(&self) -> Option<&str> {
        (self.build != self.name).then_some(self.build.as_str())
    }
}

/// Source folders no module owns that several compile: one node, named after its folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shared {
    pub name: String,
    pub content_root: PathBuf,
    pub roots: Vec<SourceRoot>,
    /// The modules that compile it, by name.
    pub modules: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    /// `group:artifact:version`, or the file for a plain jar.
    pub id: String,
    pub file: Option<PathBuf>,
    pub sources: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptKind {
    Settings,
    Build,
    Applied,
    Properties,
    VersionCatalog,
    Wrapper,
}

impl ScriptKind {
    fn parse(kind: &str) -> Self {
        match kind {
            "settings" => Self::Settings,
            "build" => Self::Build,
            "properties" => Self::Properties,
            "versionCatalog" => Self::VersionCatalog,
            "wrapper" => Self::Wrapper,
            _ => Self::Applied,
        }
    }
}

/// A Gradle script (or a Maven build's POM, `.mvn` config and wrapper), and the builds that
/// read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub path: PathBuf,
    pub kind: ScriptKind,
    pub builds: Vec<String>,
    /// A Maven build's file.
    pub maven: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedBuild {
    pub build: String,
    pub root_dir: PathBuf,
    pub gradle_version: Option<String>,
    pub reason: String,
    pub error: String,
    /// The JDK (its major version) the build needs and the machine lacks.
    pub needs_jdk: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProjectModel {
    pub root: PathBuf,
    pub name: String,
    pub modules: Vec<Module>,
    pub shared: Vec<Shared>,
    pub libraries: Vec<Library>,
    pub scripts: Vec<Script>,
    pub failed: Vec<FailedBuild>,
    /// The modules' JDKs, newest first.
    pub jdks: Vec<Jdk>,
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn array<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).into_iter().flatten()
}

fn source_root(v: &Value) -> Option<SourceRoot> {
    Some(SourceRoot {
        dir: PathBuf::from(str_of(v, "dir")?),
        kind: RootKind::parse(str_of(v, "type")?)?,
        exists: v.get("exists").and_then(Value::as_bool).unwrap_or(true),
    })
}

/// "21.0.13" before "17.0.2", by number.
fn version_key(v: &str) -> Vec<u64> {
    v.split(['.', '_', '-', '+']).map(|p| p.parse().unwrap_or(0)).collect()
}

/// The project view of a model, if it is one of format 2 or later.
pub fn parse(json: &Value) -> Option<ProjectModel> {
    if json.get("modelVersion").and_then(Value::as_u64).unwrap_or(0) < 2 {
        return None;
    }
    let root = PathBuf::from(str_of(json, "rootDir")?);
    let name = str_of(json, "name")
        .map(str::to_owned)
        .unwrap_or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let mut shared: Vec<Shared> = Vec::new();
    let mut shared_dirs: HashMap<PathBuf, String> = HashMap::new();
    for s in array(json, "sharedSourceRoots") {
        let Some(root) = source_root(s) else { continue };
        let name = str_of(s, "name").unwrap_or_default().to_owned();
        let content_root = str_of(s, "contentRoot").map(PathBuf::from).unwrap_or_else(|| root.dir.clone());
        shared_dirs.insert(root.dir.clone(), name.clone());
        let users: Vec<String> = array(s, "modules").filter_map(Value::as_str).map(str::to_owned).collect();
        match shared.iter_mut().find(|g| g.content_root == content_root) {
            Some(group) => {
                group.roots.push(root);
                for u in users {
                    if !group.modules.contains(&u) {
                        group.modules.push(u);
                    }
                }
            }
            None => shared.push(Shared { name, content_root, roots: vec![root], modules: users }),
        }
    }
    let stale: HashMap<String, String> = array(json, "failedBuilds")
        .filter(|f| f.get("stale").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|f| Some((str_of(f, "build")?.to_owned(), str_of(f, "reason").unwrap_or_default().to_owned())))
        .collect();
    let mut modules = Vec::new();
    let mut names: HashMap<String, String> = HashMap::new();
    for m in array(json, "modules") {
        let Some(id) = str_of(m, "id") else { continue };
        let Some(content_root) = str_of(m, "contentRoot") else { continue };
        let mut roots = Vec::new();
        let mut uses = Vec::new();
        for r in array(m, "sourceRoots") {
            let Some(root) = source_root(r) else { continue };
            if r.get("shared").and_then(Value::as_bool).unwrap_or(false) || shared_dirs.contains_key(&root.dir) {
                if let Some(name) = shared_dirs.get(&root.dir).filter(|n| !uses.contains(*n)) {
                    uses.push(name.clone());
                }
                continue;
            }
            if str_of(r, "owner").is_some_and(|o| o != id) {
                continue;
            }
            roots.push(root);
        }
        let jdk = m.get("jdk").and_then(|j| {
            Some(Jdk {
                home: PathBuf::from(str_of(j, "javaHome")?),
                version: str_of(j, "version").unwrap_or_default().to_owned(),
                level: str_of(j, "languageLevel").map(str::to_owned),
            })
        });
        let build = str_of(m, "build").unwrap_or(id).to_owned();
        let name = str_of(m, "name").unwrap_or(id).to_owned();
        names.insert(id.to_owned(), name.clone());
        modules.push(Module {
            id: id.to_owned(),
            name,
            stale: stale.get(&build).cloned(),
            build,
            gradle_path: str_of(m, "gradlePath").unwrap_or(":").to_owned(),
            content_root: PathBuf::from(content_root),
            roots,
            excluded: array(m, "excludedDirs").filter_map(Value::as_str).map(PathBuf::from).collect(),
            uses,
            jdk,
            maven: str_of(m, "tool") == Some("maven"),
        });
    }
    for group in &mut shared {
        for u in &mut group.modules {
            if let Some(name) = names.get(u) {
                *u = name.clone();
            }
        }
        group.modules.sort();
    }
    modules.sort_by(|a, b| a.content_root.cmp(&b.content_root).then_with(|| a.id.cmp(&b.id)));
    let mut libraries: Vec<Library> = array(json, "libraries")
        .filter_map(|l| {
            Some(Library {
                id: str_of(l, "id")?.to_owned(),
                file: str_of(l, "file").map(PathBuf::from),
                sources: str_of(l, "sources").map(PathBuf::from),
            })
        })
        .collect();
    libraries.sort_by_key(|l| l.id.to_lowercase());
    let mut scripts: Vec<Script> = Vec::new();
    for group in array(json, "gradleScripts") {
        let build = str_of(group, "build").unwrap_or_default().to_owned();
        let maven = str_of(group, "tool") == Some("maven");
        for f in array(group, "files") {
            let Some(path) = str_of(f, "path").map(PathBuf::from) else { continue };
            match scripts.iter_mut().find(|s| s.path == path) {
                Some(s) => s.builds.push(build.clone()),
                None => scripts.push(Script {
                    path,
                    kind: ScriptKind::parse(str_of(f, "kind").unwrap_or_default()),
                    builds: vec![build.clone()],
                    maven,
                }),
            }
        }
    }
    let failed = array(json, "failedBuilds")
        .filter(|f| !f.get("stale").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|f| {
            Some(FailedBuild {
                build: str_of(f, "build")?.to_owned(),
                root_dir: PathBuf::from(str_of(f, "rootDir")?),
                gradle_version: str_of(f, "gradleVersion").map(str::to_owned),
                reason: str_of(f, "reason").unwrap_or("The build did not sync").to_owned(),
                error: str_of(f, "error").unwrap_or_default().to_owned(),
                needs_jdk: f.get("needsJdk").and_then(Value::as_u64).map(|v| v as u32),
            })
        })
        .collect();
    let mut jdks: Vec<Jdk> = Vec::new();
    for jdk in modules.iter().filter_map(|m| m.jdk.clone()) {
        if !jdks.iter().any(|j| j.home == jdk.home) {
            jdks.push(jdk);
        }
    }
    jdks.sort_by_key(|j| std::cmp::Reverse(version_key(&j.version)));
    Some(ProjectModel { root, name, modules, shared, libraries, scripts, failed, jdks })
}

impl ProjectModel {
    /// What the build files' node is called: Gradle Scripts, Maven Files, or Build Files
    /// when the project has both.
    pub fn scripts_label(&self) -> &'static str {
        match (self.scripts.iter().any(|s| s.maven), self.scripts.iter().any(|s| !s.maven)) {
            (true, true) => "Build Files",
            (true, false) => "Maven Files",
            _ => "Gradle Scripts",
        }
    }

    /// Whether every build of the project is a Maven build.
    pub fn maven_only(&self) -> bool {
        !self.modules.is_empty() && self.modules.iter().all(|m| m.maven)
    }

    /// The module named `name`, if there is one.
    pub fn module_named(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name == name)
    }

    /// Whether `path` lies in a source folder of the model (a module's or a shared one).
    /// The module (or shared folder) whose source root holds `path`: the innermost one.
    pub fn module_of(&self, path: &Path) -> Option<String> {
        let modules = self.modules.iter().flat_map(|m| m.roots.iter().map(move |r| (&r.dir, &m.id)));
        let shared = self.shared.iter().flat_map(|s| s.roots.iter().map(move |r| (&r.dir, &s.name)));
        modules
            .chain(shared)
            .filter(|(dir, _)| path.starts_with(dir))
            .max_by_key(|(dir, _)| dir.as_os_str().len())
            .map(|(_, id)| id.clone())
    }

    pub fn holds(&self, path: &Path) -> bool {
        self.modules
            .iter()
            .flat_map(|m| &m.roots)
            .chain(self.shared.iter().flat_map(|s| &s.roots))
            .any(|r| path.starts_with(&r.dir))
    }
}

// ----- Library jars ----------------------------------------------------------------------

/// The classes of a jar, by package: read from the zip's central directory alone, so it
/// is cheap. Nested classes (`Outer$Inner`) are left out.
pub type JarIndex = BTreeMap<String, Vec<String>>;

/// The packages and classes in the jar at `file` on `host`, or `None` when it cannot be read.
pub fn jar_index(host: &dyn Host, file: &Path) -> Option<JarIndex> {
    let len = host.metadata(file).ok()?.len;
    // The end of central directory record: 22 bytes, and a comment of up to 64 KiB.
    let tail = len.min(22 + 65_535);
    let buf = host.read_range(file, len - tail, tail).ok().filter(|b| b.len() as u64 == tail)?;
    let at = (0..buf.len().saturating_sub(21)).rev().find(|&i| buf[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])?;
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]) as usize;
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as u64;
    let size = u32_at(&buf, at + 12);
    let offset = u32_at(&buf, at + 16);
    if offset == 0xFFFF_FFFF || offset + size > len {
        return None;
    }
    let dir = host.read_range(file, offset, size).ok().filter(|d| d.len() as u64 == size)?;
    let mut out = JarIndex::new();
    let mut i = 0;
    while i + 46 <= dir.len() && dir[i..i + 4] == [0x50, 0x4b, 0x01, 0x02] {
        let (n, extra, comment) = (u16_at(&dir, i + 28), u16_at(&dir, i + 30), u16_at(&dir, i + 32));
        let name = dir.get(i + 46..i + 46 + n).map(String::from_utf8_lossy).unwrap_or_default();
        if let Some(class) = name.strip_suffix(".class")
            && !class.contains('$')
            && !class.starts_with("META-INF/")
            && !class.ends_with("module-info")
            && !class.ends_with("package-info")
        {
            let (package, simple) = class.rsplit_once('/').unwrap_or(("", class));
            out.entry(package.replace('/', ".")).or_default().push(simple.to_owned());
        }
        i += 46 + n + extra + comment;
    }
    for classes in out.values_mut() {
        classes.sort();
    }
    Some(out)
}

// ----- Where models are -------------------------------------------------------------------

/// SnoutCode's cache folder on a host, where the engine keeps its builds, models and jdtls
/// data: `~/Library/Caches/SnoutCode` on a Mac, the XDG cache elsewhere (as the engine has
/// it by default).
pub fn cache_dir(info: &snout_host::HostInfo) -> PathBuf {
    snout_host::dirs::AppDirs::of(info).cache
}

/// A short, stable name for a folder (FNV-1a of its path).
fn folder_key(root: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in root.to_string_lossy().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Where the app's own sync of `root` writes its model.
pub fn synced_model(info: &snout_host::HostInfo, root: &Path) -> PathBuf {
    cache_dir(info).join("project").join(folder_key(root)).join("model.json")
}

/// The `rootDir` a model file names, read from its first lines.
fn root_dir_of(host: &dyn Host, file: &Path) -> Option<PathBuf> {
    let head = host.read_range(file, 0, 4096).ok()?;
    let text = String::from_utf8_lossy(&head);
    let at = text.find("\"rootDir\"")?;
    let rest = text[at + 9..].trim_start().strip_prefix(':')?.trim_start();
    let mut de = serde_json::Deserializer::from_str(rest).into_iter::<String>();
    de.next()?.ok().map(PathBuf::from)
}

/// The model files written for `root`: the Java engine's and the app's own sync, with
/// when each was written.
pub fn model_files(host: &dyn Host, root: &Path) -> Vec<(PathBuf, SystemTime)> {
    let want = host.canonicalize(root).unwrap_or_else(|_| root.to_owned());
    let mut out = Vec::new();
    let stamp = |f: &Path| modified(host, f);
    if let Ok(entries) = host.read_dir(&cache_dir(host.info()).join("jdtls")) {
        for e in entries {
            let file = e.path.join("model.json");
            if let Some(dir) = root_dir_of(host, &file)
                && (dir == want || host.canonicalize(&dir).is_ok_and(|d| d == want))
                && let Some(t) = stamp(&file)
            {
                out.push((file, t));
            }
        }
    }
    let own = synced_model(host.info(), &want);
    if let Some(t) = stamp(&own) {
        out.push((own, t));
    }
    out
}

/// Whether the open project `dir` holds Gradle builds, for the views: as the project models
/// last saw on the host (none until they have), so the UI thread never waits on the host.
/// Without the project models (no window), the host is asked.
pub fn project_has_gradle(dir: &Path, cx: &App) -> bool {
    match store(cx) {
        Some(s) => s.read(cx).gradle_of(dir).is_some_and(|(g, _)| g),
        None => has_gradle(&*crate::host::host(cx), dir),
    }
}

/// Whether the open project `dir` is a Gradle build itself, as [`project_has_gradle`] knows it.
pub fn project_is_gradle_root(dir: &Path, cx: &App) -> bool {
    match store(cx) {
        Some(s) => s.read(cx).gradle_of(dir).is_some_and(|(_, r)| r),
        None => crate::lang::java::is_gradle_root(&*crate::host::host(cx), dir),
    }
}

/// Whether `dir` holds Gradle builds: it is one, lies in one, or has one below it (a few
/// levels down, as the engine looks). Remembered for a few seconds, as views ask often.
pub fn has_gradle(host: &dyn Host, dir: &Path) -> bool {
    thread_local! {
        static SEEN: std::cell::RefCell<HashMap<PathBuf, (Instant, bool)>> = Default::default();
    }
    let known = SEEN.with(|s| s.borrow().get(dir).filter(|(t, _)| t.elapsed() < Duration::from_secs(5)).map(|e| e.1));
    if let Some(known) = known {
        return known;
    }
    let found = look_for_gradle(host, dir);
    SEEN.with(|s| s.borrow_mut().insert(dir.to_owned(), (Instant::now(), found)));
    found
}

fn look_for_gradle(host: &dyn Host, dir: &Path) -> bool {
    use crate::lang::java;
    if java::is_gradle_root(host, dir) || java::gradle_root(host, &dir.join("x"), None).is_some() {
        return true;
    }
    fn below(host: &dyn Host, dir: &Path, depth: usize) -> bool {
        let Ok(entries) = host.read_dir(dir) else { return false };
        entries.into_iter().any(|e| {
            let name = e.file_name().to_string_lossy();
            let skip = name.starts_with('.') || matches!(name.as_ref(), "build" | "node_modules" | "out" | "target");
            !skip && e.is_dir && (java::is_gradle_root(host, &e.path) || (depth > 1 && below(host, &e.path, depth - 1)))
        })
    }
    below(host, dir, 3)
}

// ----- The store --------------------------------------------------------------------------

/// The open project's model, kept current.
pub struct ProjectModels {
    model: Entity<Model>,
    hub: Option<Entity<LangHub>>,
    /// The project folder the model is for.
    pub root: Option<PathBuf>,
    pub current: Option<Rc<ProjectModel>>,
    /// The file it was read from, and when that was written.
    source: Option<(PathBuf, SystemTime)>,
    reading: bool,
    /// A look at the host for [`Self::poll`] is under way.
    polling: bool,
    /// Whether the project holds Gradle builds, and is one itself, as the last look saw.
    gradle: Option<(PathBuf, bool, bool)>,
    /// The app's own `snout-engine sync` is running, since then.
    pub syncing: Option<Instant>,
    pub sync_error: Option<String>,
    /// When each Gradle script was last changed, as the model was read.
    stamps: Vec<(PathBuf, Option<SystemTime>)>,
    /// A Gradle script changed since the model was read.
    pub scripts_changed: bool,
    /// Sync on its own when the project has no model yet (the app; not the tests).
    pub auto_sync: bool,
    tried: bool,
    host: HostRef,
    _poll: Option<Task<()>>,
    _sync: Option<Task<()>>,
    _engine: Option<gpui::Subscription>,
}

/// The window's project models, for the views that show them.
pub struct ProjectModelsHandle(pub Entity<ProjectModels>);

impl Global for ProjectModelsHandle {}

/// The window's project models, if it has them.
pub fn store(cx: &App) -> Option<Entity<ProjectModels>> {
    cx.try_global::<ProjectModelsHandle>().map(|h| h.0.clone())
}

impl ProjectModels {
    pub fn new(model: Entity<Model>, hub: Option<Entity<LangHub>>, cx: &mut Context<Self>) -> Self {
        let auto_sync = hub.as_ref().is_some_and(|h| h.read(cx).enabled);
        let host = crate::host::host(cx);
        let poll = cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(Duration::from_millis(1500)).await;
            }
        });
        // A background sync changed the model the engine wrote: read it again now.
        let engine = hub.as_ref().map(|h| {
            cx.subscribe(h, |this, _, e: &crate::lang::HubEvent, cx| {
                if let crate::lang::HubEvent::Engine { method, .. } = e
                    && method == "snout/modelChanged"
                {
                    this.model_changed(cx);
                }
            })
        });
        Self {
            model,
            hub,
            root: None,
            current: None,
            source: None,
            reading: false,
            polling: false,
            gradle: None,
            syncing: None,
            sync_error: None,
            stamps: Vec::new(),
            scripts_changed: false,
            auto_sync,
            tried: false,
            host,
            _poll: Some(poll),
            _sync: None,
            _engine: engine,
        }
    }

    /// The engine says the model changed: read the newest model file again, keeping the
    /// one shown until the new one is read, so the Project view never empties.
    pub fn model_changed(&mut self, cx: &mut Context<Self>) {
        eprintln!("[project] the engine changed the Gradle model");
        self.source = None;
        self.poll(cx);
    }

    /// Show `model` as the project's (tests, and models read elsewhere).
    pub fn set_model(&mut self, model: ProjectModel, cx: &mut Context<Self>) {
        self.root = Some(model.root.clone());
        self.stamps = stamps(&*self.host, &model);
        self.scripts_changed = false;
        self.current = Some(Rc::new(model));
        cx.notify();
    }

    /// The model file read last, and when it was written.
    pub fn source(&self) -> Option<(PathBuf, SystemTime)> {
        self.source.clone()
    }

    /// The model, if it is the open project's.
    pub fn model_for(&self, root: &Path) -> Option<Rc<ProjectModel>> {
        self.current.clone().filter(|_| self.root.as_deref() == Some(root))
    }

    /// The Java engine of the project runs (or is starting), so it syncs the model itself.
    /// One waiting for a JDK, or failed, does not.
    fn engine_runs(&self, root: &Path, cx: &App) -> bool {
        use crate::lang::hub::Stage;
        self.hub
            .as_ref()
            .and_then(|h| h.read(cx).servers.get(root))
            .is_some_and(|s| !matches!(s.stage, Stage::NeedsJdk(_) | Stage::Failed(_)))
    }

    /// Look again: another project, a newer model file, changed scripts.
    pub fn poll(&mut self, cx: &mut Context<Self>) {
        let root = self.model.read(cx).project_root();
        if root != self.root {
            let keep = self.current.as_ref().is_some_and(|m| Some(&m.root) == root.as_ref());
            self.root = root.clone();
            if !keep {
                self.current = None;
                self.source = None;
                self.stamps.clear();
            }
            self.scripts_changed = false;
            self.sync_error = None;
            self.tried = false;
            self._sync = None;
            self.syncing = None;
            cx.notify();
        }
        let Some(root) = root else { return };
        if self.polling {
            return;
        }
        // What the host holds is looked at off the UI thread.
        self.polling = true;
        let host = self.host.clone();
        let stamps = if self.current.is_some() && !self.scripts_changed { self.stamps.clone() } else { Vec::new() };
        let look = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                let newest = model_files(&*host, &root).into_iter().max_by_key(|(_, t)| *t);
                let gradle = (has_gradle(&*host, &root), crate::lang::java::is_gradle_root(&*host, &root));
                let changed = stamps.iter().any(|(path, t)| modified(&*host, path) != *t);
                (newest, gradle, changed)
            }
        });
        cx.spawn(async move |this, cx| {
            let (newest, gradle, changed) = look.await;
            let _ = this.update(cx, |this, cx| {
                this.polling = false;
                if this.root.as_ref() == Some(&root) {
                    this.looked(root, newest, gradle, changed, cx);
                }
            });
        })
        .detach();
    }

    /// What a look at the host saw: the newest model file, whether the project is a Gradle
    /// build, and whether its scripts changed.
    fn looked(
        &mut self,
        root: PathBuf,
        newest: Option<(PathBuf, SystemTime)>,
        (gradle, gradle_root): (bool, bool),
        changed: bool,
        cx: &mut Context<Self>,
    ) {
        if self.gradle.as_ref() != Some(&(root.clone(), gradle, gradle_root)) {
            self.gradle = Some((root.clone(), gradle, gradle_root));
            cx.notify();
        }
        if let Some((file, when)) = newest {
            if !self.reading && self.source.as_ref().is_none_or(|(f, t)| *f != file || *t != when) {
                self.read(file, when, root.clone(), cx);
            }
        } else if self.current.is_none()
            && self.auto_sync
            && !self.tried
            && self.syncing.is_none()
            && !self.hub.as_ref().is_some_and(|h| h.read(cx).paused)
            && !self.engine_runs(&root, cx)
            && gradle
        {
            self.tried = true;
            self.sync(cx);
        }
        if changed && self.current.is_some() && !self.scripts_changed && !self.stamps.is_empty() {
            self.scripts_changed = true;
            cx.notify();
        }
    }

    /// Whether `dir` (the open project) holds Gradle builds, and whether it is one, as the
    /// last look at the host saw; `None` until one has.
    pub fn gradle_of(&self, dir: &Path) -> Option<(bool, bool)> {
        self.gradle.as_ref().filter(|(r, _, _)| r == dir).map(|(_, g, r)| (*g, *r))
    }

    fn read(&mut self, file: PathBuf, when: SystemTime, root: PathBuf, cx: &mut Context<Self>) {
        self.reading = true;
        let host = self.host.clone();
        let task = cx.background_executor().spawn({
            let file = file.clone();
            async move {
                let text = host.read_to_string(&file).ok()?;
                let json: Value = serde_json::from_str(&text).ok()?;
                let model = parse(&json)?;
                let stamps = stamps(&*host, &model);
                Some((model, stamps))
            }
        });
        cx.spawn(async move |this, cx| {
            let read = task.await;
            let _ = this.update(cx, |this, cx| {
                this.reading = false;
                this.source = Some((file, when));
                if this.root.as_ref() != Some(&root) {
                    return;
                }
                if let Some((model, stamps)) = read {
                    eprintln!(
                        "[project] read the Gradle model of {}: {} modules, {} shared, {} failed, {} libraries",
                        root.display(),
                        model.modules.len(),
                        model.shared.len(),
                        model.failed.len(),
                        model.libraries.len()
                    );
                    // A build that needs a JDK this machine lacks: offer to download it.
                    let wanted = model.failed.iter().find_map(|f| f.needs_jdk);
                    this.current = Some(Rc::new(model));
                    this.stamps = stamps;
                    this.scripts_changed = false;
                    if let (Some(major), Some(hub)) = (wanted, this.hub.clone()) {
                        hub.update(cx, |h, cx| h.offer_jdk(&root, major, cx));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Sync the Gradle model again: the project's Java engine restarts when it runs (it
    /// syncs as it starts), else `snout-engine sync` runs in the background.
    pub fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        self.scripts_changed = false;
        self.sync_error = None;
        if self.engine_runs(&root, cx)
            && let Some(hub) = self.hub.clone()
        {
            hub.update(cx, |h, cx| h.restart(&root, cx));
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let out = synced_model(self.host.info(), &root);
        let jdk = self.model.read(cx).jdk_for(&root);
        self.syncing = Some(Instant::now());
        let run = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                let dist = crate::lang::java::engine_dist(&*host)
                    .ok_or_else(|| "The Java engine is not built (run snout.sh)".to_owned())?;
                run_sync(&*host, &dist, &root, &out, jdk.as_deref())
            }
        });
        self._sync = Some(cx.spawn(async move |this, cx| {
            let result = run.await;
            let _ = this.update(cx, |this, cx| {
                if this.root.as_ref() != Some(&root) {
                    return;
                }
                this.syncing = None;
                if let Err(e) = result {
                    this.sync_error = Some(e);
                }
                this.poll(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Put the bar about changed scripts away until they change again.
    pub fn dismiss_changes(&mut self, cx: &mut Context<Self>) {
        self.scripts_changed = false;
        for (path, t) in &mut self.stamps {
            *t = modified(&*self.host, path);
        }
        cx.notify();
    }

    /// What the status bar lists while the app's own sync runs.
    pub fn task(&self) -> Option<crate::background::BgTask> {
        let root = self.root.as_ref()?;
        self.syncing.map(|_| crate::background::BgTask {
            title: format!("Sync: {}", Model::dir_name(root)),
            stage: "Syncing the build model".into(),
            percent: None,
            ..Default::default()
        })
    }
}

fn modified(host: &dyn Host, path: &Path) -> Option<SystemTime> {
    host.metadata(path).ok().and_then(|m| m.modified)
}

/// When each of the model's Gradle scripts was last changed.
fn stamps(host: &dyn Host, model: &ProjectModel) -> Vec<(PathBuf, Option<SystemTime>)> {
    model.scripts.iter().map(|s| (s.path.clone(), modified(host, &s.path))).collect()
}

/// Run `snout-engine sync` on `root`, its model to `out`, Gradle's project caches kept
/// beside it so nothing is written into the project.
pub fn run_sync(host: &dyn Host, dist: &Path, root: &Path, out: &Path, jdk: Option<&Path>) -> Result<(), String> {
    let dir = out.parent().ok_or("no folder for the model")?;
    host.create_dir_all(dir).map_err(|e| e.to_string())?;
    let partial = dir.join("model.json.part");
    let mut c = Cmd::new(crate::lang::java::java_bin(host.info(), &dist.join("runtime")))
        .arg("-cp")
        .arg(crate::lang::java::engine_classpath(dist))
        .arg("com.snoutcode.engine.cli.SnoutEngine")
        .arg("sync")
        .arg(root)
        .arg("--out")
        .arg(&partial)
        .arg("--project-cache-dir")
        .arg(dir.join("gradle"))
        .cwd(dir)
        .stdin(Stdio::Null)
        .stdout(Stdio::Null);
    // Into a file, not a pipe: the Gradle daemons the sync starts outlive it and would hold
    // a pipe open.
    let log = dir.join("sync.log");
    c = c.stderr(Stdio::File(log.clone()));
    if let Some(jdk) = jdk {
        c = c.arg("--java-home").arg(jdk);
    }
    for extra in crate::lang::java::downloaded_jdks(host) {
        c = c.arg("--jdk").arg(extra.home);
    }
    host.run(&c, None).map_err(|e| format!("snout-engine did not start: {e}"))?;
    // A build that fails does not stop the others: the model is written all the same.
    if host.is_file(&partial) {
        return host.rename(&partial, out).map_err(|e| e.to_string());
    }
    let err = host.read_to_string(&log).unwrap_or_default();
    Err(err.lines().last().unwrap_or("The Gradle sync failed").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn fixture(root: &Path) -> Value {
        let text = include_str!("../tests/fixtures/project-model.json");
        serde_json::from_str(&text.replace("$ROOT", &root.to_string_lossy().replace('\\', "\\\\"))).unwrap()
    }

    #[test]
    fn reads_modules_shared_roots_libraries_scripts_and_failures() {
        let root = Path::new("/w/shop");
        let m = parse(&fixture(root)).unwrap();
        assert_eq!(m.name, "shop");
        let names: Vec<&str> = m.modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["shop", "app", "billing"]);
        let app = m.module_named("app").unwrap();
        assert_eq!(app.build_note(), Some("shop"));
        assert_eq!(app.uses, ["core"]);
        assert!(app.roots.iter().all(|r| !r.dir.starts_with("/w/shop/core")), "shared roots stay out");
        let kinds: Vec<RootKind> = app.roots.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&RootKind::Main) && kinds.contains(&RootKind::TestResources));
        assert_eq!(m.shared.len(), 1);
        assert_eq!(m.shared[0].name, "core");
        assert_eq!(m.shared[0].roots.len(), 2);
        assert_eq!(m.shared[0].modules, ["app", "billing"]);
        assert_eq!(m.failed.len(), 1);
        assert_eq!(m.failed[0].build, "legacy");
        assert_eq!(m.libraries[0].id, "com.google.guava:guava:33.0.0-jre");
        let applied = m.scripts.iter().find(|s| s.kind == ScriptKind::Applied).unwrap();
        assert_eq!(applied.builds, ["shop", "billing"], "a script two builds apply is listed once");
        assert_eq!(m.jdks[0].label(), "JDK 21.0.2");
        assert!(m.holds(Path::new("/w/shop/core/src/main/java/core/Money.java")));
        assert!(!m.holds(Path::new("/w/shop/README.md")));
    }

    #[test]
    fn an_older_model_is_not_a_project_view() {
        assert!(parse(&serde_json::json!({ "rootDir": "/w", "builds": [] })).is_none());
    }

    #[test]
    fn finds_the_root_a_model_file_names() {
        let dir = std::env::temp_dir().join(format!("snout-model-head-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("model.json");
        std::fs::write(&file, "{\n  \"modelVersion\": 2,\n  \"rootDir\": \"/w/a \\\"b\\\"\",\n  \"x\": 1\n}").unwrap();
        assert_eq!(root_dir_of(&*snout_host::local(), &file), Some(PathBuf::from("/w/a \"b\"")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lists_a_jars_packages_and_classes() {
        let dir = std::env::temp_dir().join(format!("snout-jar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("demo.jar");
        write_fixture_jar(
            &jar,
            &["META-INF/MANIFEST.MF", "demo/Greeter.class", "demo/Greeter$1.class", "demo/util/Text.class"],
        );
        let index = jar_index(&*snout_host::local(), &jar).unwrap();
        assert_eq!(index.keys().collect::<Vec<_>>(), ["demo", "demo.util"]);
        assert_eq!(index["demo"], ["Greeter"]);
        assert!(jar_index(&*snout_host::local(), &dir.join("missing.jar")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    include!("../tests/fixtures/project_fixture.rs");
}
