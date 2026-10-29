//! Spring Boot, through the engine's `snout/spring*` methods: the beans of each module's
//! application context and the request mappings of its controllers, for the Spring tool
//! window, and per file the injection points, the beans and the endpoints, for the
//! editor's gutter.
//!
//! [`Spring`] keeps the project's model for the window. It reads it once the engine is
//! ready, and again after a Java or Scala file is saved, and tells the editors (through
//! its revision) that their files' marks may have changed.
//!
//! It also says which profiles the engine evaluates ([`ProfileMode`], kept per project):
//! none (a bean with a `@Profile` may be absent), those the application files make active,
//! or those chosen in the Spring window. Beans the active profiles leave out come back
//! marked inactive; the window dims them, and injections no longer take them.

pub mod window;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{App, Context, Entity, EventEmitter, Global, Hsla, Task};
use lsp_types::{Position, Range};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::lang::{HubEvent, LangHub};
use crate::run::Runs;
use crate::theme::Palette;

/// The Spring accent: a soft leaf green.
pub fn green(p: &Palette) -> Hsla {
    p.spring
}

fn yes() -> bool {
    true
}

/// Which profiles the engine evaluates the beans under.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub enum ProfileMode {
    /// None: a bean with a `@Profile` may be there or not.
    #[default]
    Any,
    /// Those `spring.profiles.active` names in the application files (else the default ones).
    Config,
    /// These, as `--spring.profiles.active` would.
    Chosen(Vec<String>),
}

impl ProfileMode {
    /// What the engine's `snout/springSetProfiles` takes.
    pub fn params(&self) -> Value {
        match self {
            ProfileMode::Any => json!({ "mode": "any" }),
            ProfileMode::Config => json!({ "mode": "config" }),
            ProfileMode::Chosen(p) => json!({ "mode": "chosen", "profiles": p }),
        }
    }

    /// The mode with `profile` chosen or not (from the files' active ones when none were chosen).
    pub fn toggled(&self, profile: &str, info: &ProfileInfo) -> Self {
        let mut chosen = match self {
            ProfileMode::Chosen(p) => p.clone(),
            ProfileMode::Config => info.config_active(),
            ProfileMode::Any => Vec::new(),
        };
        match chosen.iter().position(|p| p == profile) {
            Some(i) => {
                chosen.remove(i);
            }
            None => chosen.push(profile.to_owned()),
        }
        ProfileMode::Chosen(chosen)
    }
}

/// A profile a module knows, and where it is mentioned.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct KnownProfile {
    pub name: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

/// What `snout/springProfiles` says of one module.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleProfiles {
    pub module: String,
    #[serde(default)]
    pub config_active: Vec<String>,
    /// The active profiles, while they are evaluated.
    pub active: Option<Vec<String>>,
    #[serde(default)]
    pub known: Vec<KnownProfile>,
    #[serde(default)]
    pub inactive_beans: usize,
}

/// The profiles of the project's modules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileInfo {
    pub modules: Vec<ModuleProfiles>,
}

impl ProfileInfo {
    pub fn parse(v: &Value) -> Self {
        Self { modules: v.get("modules").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or_default() }
    }

    /// Every profile any module knows, sorted, with where each is mentioned.
    pub fn known(&self) -> Vec<KnownProfile> {
        let mut out: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for m in &self.modules {
            for k in &m.known {
                let s = out.entry(k.name.clone()).or_default();
                for src in &k.sources {
                    if !s.contains(src) {
                        s.push(src.clone());
                    }
                }
            }
        }
        out.into_iter().map(|(name, sources)| KnownProfile { name, sources }).collect()
    }

    /// The profiles the application files make active, every module's.
    pub fn config_active(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in self.modules.iter().flat_map(|m| m.config_active.iter()) {
            if !out.contains(p) {
                out.push(p.clone());
            }
        }
        out
    }

    /// The profiles active now, every module's (none while they are not evaluated).
    pub fn active(&self) -> Option<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        let mut any = false;
        for p in self.modules.iter().filter_map(|m| m.active.as_ref()) {
            any = true;
            for x in p {
                if !out.contains(x) {
                    out.push(x.clone());
                }
            }
        }
        any.then_some(out)
    }

    /// How many beans the active profiles leave out.
    pub fn inactive_beans(&self) -> usize {
        self.modules.iter().map(|m| m.inactive_beans).sum()
    }
}

/// What the profile switch says: "Any profile", "dev, local (from the files)", "dev".
pub fn mode_label(mode: &ProfileMode, info: &ProfileInfo) -> String {
    let list = |v: &[String]| if v.is_empty() { "default".to_owned() } else { v.join(", ") };
    match mode {
        ProfileMode::Any => "Any profile".into(),
        ProfileMode::Config => {
            format!("{} (from the files)", list(&info.active().unwrap_or_else(|| info.config_active())))
        }
        ProfileMode::Chosen(p) => list(p),
    }
}

/// Where a bean, an injection site or an endpoint is.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    /// A `file:` URI; none for a library's bean.
    pub uri: Option<String>,
    pub range: Option<Range>,
    /// The class to open when there is no URI (a library's bean).
    pub class: Option<String>,
}

impl Target {
    /// The file and line (from 0), when it is a file of the project.
    pub fn file(&self) -> Option<(PathBuf, u32)> {
        let uri: lsp_types::Uri = self.uri.as_deref()?.parse().ok()?;
        let path = snout_lang::uri::to_path(&uri)?;
        Some((path, self.range.map_or(0, |r| r.start.line)))
    }

    pub fn can_open(&self) -> bool {
        self.uri.is_some() || self.class.is_some()
    }
}

/// A place a bean is injected into (`springInjections`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub uri: String,
    pub range: Range,
    #[serde(default)]
    pub declaring_type: String,
    #[serde(default)]
    pub kind: String,
    pub member: Option<String>,
    pub name: Option<String>,
}

impl Site {
    /// "GreetingController.clock", or "GreetingController(clock)" for a constructor's.
    pub fn label(&self) -> String {
        let class = simple(&self.declaring_type);
        match (self.kind.as_str(), &self.member, &self.name) {
            ("constructor", _, Some(n)) => format!("{class}({n})"),
            ("beanMethod" | "method" | "setter", Some(m), Some(n)) => format!("{class}.{m}({n})"),
            (_, _, Some(n)) => format!("{class}.{n}"),
            (_, Some(m), None) => format!("{class}.{m}"),
            _ => class.to_owned(),
        }
    }

    pub fn target(&self) -> Target {
        Target { uri: Some(self.uri.clone()), range: Some(self.range), class: None }
    }
}

/// A bean definition.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bean {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(rename = "type", default)]
    pub ty: String,
    #[serde(default)]
    pub kind: String,
    pub stereotype: Option<String>,
    pub declaring_type: Option<String>,
    /// The `@Bean` method that makes it.
    pub method: Option<String>,
    pub uri: Option<String>,
    pub range: Option<Range>,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub qualifiers: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub conditions: Vec<Value>,
    /// The active profiles leave it out (only while they are evaluated).
    #[serde(default = "yes")]
    pub active: bool,
    /// A count (`springBeans`) or the sites (`springInjections`).
    #[serde(default)]
    injected_into: Value,
}

impl Bean {
    /// How many places it is injected into.
    pub fn injected_count(&self) -> usize {
        match &self.injected_into {
            Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
            Value::Array(a) => a.len(),
            _ => 0,
        }
    }

    /// Where it is injected (from `springInjections` only).
    pub fn sites(&self) -> Vec<Site> {
        serde_json::from_value(self.injected_into.clone()).unwrap_or_default()
    }

    /// What the window groups it under: the stereotype's simple name ("Service", "Bean").
    pub fn stereotype_label(&self) -> String {
        if let Some(s) = &self.stereotype {
            return simple(s).to_owned();
        }
        kind_label(&self.kind).to_owned()
    }

    /// The package of the class it is (or of the class with its `@Bean` method).
    pub fn package(&self) -> String {
        let class = self.declaring_type.as_deref().unwrap_or(&self.ty);
        match class.rfind('.') {
            Some(i) => class[..i].to_owned(),
            None => String::new(),
        }
    }

    pub fn target(&self) -> Target {
        Target { uri: self.uri.clone(), range: self.range, class: self.declaring_type.clone() }
    }
}

/// A readable name for a bean's kind.
pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "component" => "Component",
        "service" => "Service",
        "repository" => "Repository",
        "controller" => "Controller",
        "restController" => "RestController",
        "configuration" => "Configuration",
        "bean" => "Bean",
        "xml" => "XML",
        "dataRepository" => "Data repository",
        "mapper" => "Mapper",
        "properties" => "ConfigurationProperties",
        "library" => "Library",
        _ => "Component",
    }
}

/// The last part of a qualified name.
pub fn simple(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

/// A bean an injection point resolves to.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeanRef {
    pub name: String,
    #[serde(rename = "type", default)]
    pub ty: String,
    #[serde(default)]
    pub kind: String,
    pub declaring_type: Option<String>,
    pub method: Option<String>,
    pub uri: Option<String>,
    pub range: Option<Range>,
}

impl BeanRef {
    pub fn target(&self) -> Target {
        Target { uri: self.uri.clone(), range: self.range, class: self.declaring_type.clone() }
    }

    /// "englishGreeting (EnglishGreeting)" or "clock (AppConfig.clock())".
    pub fn label(&self) -> String {
        let from = match (&self.declaring_type, &self.method) {
            (Some(d), Some(m)) => format!("{}.{m}()", simple(d)),
            (Some(d), None) => simple(d).to_owned(),
            (None, _) => simple(&self.ty).to_owned(),
        };
        format!("{} ({from})", self.name)
    }
}

/// An injection point.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Injection {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub annotation: String,
    #[serde(default)]
    pub declaring_type: String,
    pub member: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub wrapper: Option<String>,
    #[serde(default)]
    pub required: bool,
    /// `@Value`'s expression.
    pub value: Option<String>,
    pub range: Range,
    #[serde(default)]
    pub status: String,
    pub message: Option<String>,
    #[serde(default)]
    pub beans: Vec<BeanRef>,
}

impl Injection {
    /// No bean, or several where one is wanted: drawn in the warning colour.
    pub fn is_problem(&self) -> bool {
        matches!(self.status.as_str(), "missing" | "ambiguous")
    }
}

/// A request mapping.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub paths: Vec<String>,
    /// Empty: any method.
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub produces: Vec<String>,
    #[serde(default)]
    pub consumes: Vec<String>,
    pub handler: Handler,
    pub uri: Option<String>,
    pub range: Option<Range>,
    #[serde(default)]
    pub language: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handler {
    pub class: String,
    pub declaring_class: Option<String>,
    pub method: String,
}

impl Endpoint {
    /// "GET, POST", or "ANY".
    pub fn methods_label(&self) -> String {
        if self.methods.is_empty() { "ANY".to_owned() } else { self.methods.join(", ") }
    }

    /// "LogisticsController.list".
    pub fn handler_label(&self) -> String {
        format!("{}.{}", simple(&self.handler.class), self.handler.method)
    }

    /// Every path it answers, "GET /status" for the tooltip, one per line.
    pub fn summary(&self) -> String {
        let paths = if self.paths.is_empty() { vec![self.path.clone()] } else { self.paths.clone() };
        let mut out = paths.iter().map(|p| format!("{} {p}", self.methods_label())).collect::<Vec<_>>().join("\n");
        if !self.consumes.is_empty() {
            out.push_str(&format!("\nConsumes {}", self.consumes.join(", ")));
        }
        if !self.produces.is_empty() {
            out.push_str(&format!("\nProduces {}", self.produces.join(", ")));
        }
        out
    }

    pub fn target(&self) -> Target {
        Target { uri: self.uri.clone(), range: self.range, class: Some(self.handler.class.clone()) }
    }
}

/// The beans of one module's application context.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleBeans {
    pub module: String,
    #[serde(default)]
    pub library_beans: usize,
    #[serde(default)]
    pub beans: Vec<Bean>,
}

/// What a line of a file holds for Spring.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SpringLine {
    pub line: u32,
    #[serde(default)]
    pub injections: Vec<Injection>,
    #[serde(default)]
    pub beans: Vec<Bean>,
    /// From `springEndpoints`, the handlers declared on this line.
    #[serde(skip)]
    pub endpoints: Vec<Endpoint>,
}

/// The kind of gutter mark a line gets: one per line, the first that applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Endpoint,
    Bean,
    Injection { problem: bool },
}

impl SpringLine {
    pub fn mark(&self) -> Option<MarkKind> {
        if !self.endpoints.is_empty() {
            Some(MarkKind::Endpoint)
        } else if !self.beans.is_empty() {
            Some(MarkKind::Bean)
        } else if !self.injections.is_empty() {
            Some(MarkKind::Injection { problem: self.injections.iter().any(Injection::is_problem) })
        } else {
            None
        }
    }

    /// What the gutter's tooltip says about the line.
    pub fn tooltip(&self) -> String {
        let mut parts = Vec::new();
        for e in &self.endpoints {
            parts.push(e.summary());
        }
        for b in &self.beans {
            if !b.active {
                parts.push(format!(
                    "Bean '{}' ({}): not active with the active profiles",
                    b.name,
                    b.stereotype_label()
                ));
                continue;
            }
            let n = b.injected_count();
            let used = match n {
                0 => "not injected anywhere".to_owned(),
                1 => "injected in 1 place".to_owned(),
                n => format!("injected in {n} places"),
            };
            parts.push(format!("Bean '{}' ({}), {used}", b.name, b.stereotype_label()));
        }
        for i in &self.injections {
            let what = i.name.clone().unwrap_or_default();
            let text = match (i.status.as_str(), i.beans.as_slice()) {
                ("value", _) => format!("{what}: {}", i.value.clone().unwrap_or_default()),
                (_, [one]) => format!("{what}: bean '{}'", one.name),
                (_, []) => i.message.clone().unwrap_or_else(|| format!("{what}: no bean")),
                (_, many) => format!("{what}: {} beans", many.len()),
            };
            parts.push(text);
        }
        parts.join("\n")
    }
}

/// The Spring marks of one file, by line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileSpring {
    pub lines: Vec<SpringLine>,
}

impl FileSpring {
    /// From `springInjections`' answer, with the endpoints whose handlers are in `uri` (two
    /// spellings of one file on `host` are one).
    pub fn parse(host: &dyn snout_host::Host, v: &Value, endpoints: &[Endpoint]) -> Self {
        let mut lines: Vec<SpringLine> =
            v.get("lines").and_then(|l| serde_json::from_value(l.clone()).ok()).unwrap_or_default();
        let uri = v.get("uri").and_then(Value::as_str).unwrap_or_default();
        for e in endpoints.iter().filter(|e| e.uri.as_deref().is_some_and(|u| same_uri(&*host, u, uri))) {
            let Some(line) = e.range.map(|r| r.start.line) else { continue };
            match lines.iter_mut().find(|l| l.line == line) {
                Some(l) => l.endpoints.push(e.clone()),
                None => lines.push(SpringLine { line, endpoints: vec![e.clone()], ..Default::default() }),
            }
        }
        lines.sort_by_key(|l| l.line);
        lines.retain(|l| l.mark().is_some());
        Self { lines }
    }

    pub fn line(&self, line: u32) -> Option<&SpringLine> {
        self.lines.iter().find(|l| l.line == line)
    }
}

/// Two URIs of one file on `host` (the engine may spell the path differently).
fn same_uri(host: &dyn snout_host::Host, a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let path = |u: &str| u.parse::<lsp_types::Uri>().ok().and_then(|u| snout_lang::uri::to_path(&u));
    match (path(a), path(b)) {
        (Some(a), Some(b)) => {
            a == b || host.canonicalize(&a).ok().zip(host.canonicalize(&b).ok()).is_some_and(|(a, b)| a == b)
        }
        _ => false,
    }
}

/// Parse `springBeans`' answer.
pub fn parse_beans(v: &Value) -> Vec<ModuleBeans> {
    v.get("modules").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or_default()
}

/// Parse `springEndpoints`' answer.
pub fn parse_endpoints(v: &Value) -> Vec<Endpoint> {
    v.get("endpoints").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or_default()
}

/// Whether a file can have Spring marks.
pub fn has_marks(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("java" | "scala"))
}

/// Whether a file is one of Spring Boot's configuration files, which the engine serves.
pub fn is_config_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    (name.starts_with("application") || name.starts_with("bootstrap")) && matches!(ext, "properties" | "yml" | "yaml")
}

/// The language id a Spring configuration file goes to the engine with.
pub fn config_language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("yml" | "yaml") => "spring-boot-properties-yaml",
        _ => "spring-boot-properties",
    }
}

/// What the Spring model asks of the workspace.
#[derive(Clone, Debug)]
pub enum SpringEvent {
    /// Show this place: a file of the project, or a library's class.
    Open(Target),
    /// Show the Spring tool window, on the Endpoints tab when this is set.
    Show { endpoints: Option<String> },
}

/// How the model's reading goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Load {
    #[default]
    Idle,
    Loading,
    Failed(String),
}

pub struct Spring {
    runs: Entity<Runs>,
    model: Entity<crate::model::Model>,
    /// Which profiles the engine evaluates, and what it says of them.
    pub profile_mode: ProfileMode,
    pub profile_info: ProfileInfo,
    pub modules: Vec<ModuleBeans>,
    pub endpoints: Vec<Endpoint>,
    pub load: Load,
    /// The engine process the model was read from.
    from: Option<u32>,
    /// Read at least once.
    pub loaded: bool,
    /// Bumped whenever the model was read again: the editors ask for their marks again.
    pub revision: u64,
    _refresh: Option<Task<()>>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl EventEmitter<SpringEvent> for Spring {}

pub struct SpringHandle(pub Entity<Spring>);

impl Global for SpringHandle {}

pub fn spring(cx: &App) -> Option<Entity<Spring>> {
    cx.try_global::<SpringHandle>().map(|h| h.0.clone())
}

impl Spring {
    pub fn new(
        runs: Entity<Runs>,
        hub: Entity<LangHub>,
        model: Entity<crate::model::Model>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.subscribe(&hub, |this, _, e: &HubEvent, cx| {
                if matches!(e, HubEvent::Changed) {
                    this.engine_changed(cx);
                }
            }),
            // Another project: its own profiles.
            cx.observe(&model, |this, model, cx| {
                let mode = Self::saved_mode(&model, cx);
                if mode != this.profile_mode {
                    this.profile_mode = mode;
                    cx.notify();
                }
            }),
        ];
        let profile_mode = Self::saved_mode(&model, cx);
        Self {
            runs,
            model,
            profile_mode,
            profile_info: ProfileInfo::default(),
            modules: Vec::new(),
            endpoints: Vec::new(),
            load: Load::Idle,
            from: None,
            loaded: false,
            revision: 0,
            _refresh: None,
            _subscriptions: subscriptions,
        }
    }

    /// The engine started or changed: read the model once per engine process.
    fn engine_changed(&mut self, cx: &mut Context<Self>) {
        let Some((_, pid)) = self.runs.read(cx).engine(cx) else { return };
        let ready = self.runs.read(cx).engine_ready(cx);
        if ready && self.from != Some(pid) && self.load != Load::Loading {
            self.reload(cx);
        }
    }

    /// The profile mode the open project keeps.
    fn saved_mode(model: &Entity<crate::model::Model>, cx: &App) -> ProfileMode {
        model.read(cx).spring_mode()
    }

    /// Evaluate the beans under `mode` from now on (kept with the project), and read the
    /// model again.
    pub fn set_profile_mode(&mut self, mode: ProfileMode, cx: &mut Context<Self>) {
        self.profile_mode = mode.clone();
        self.model.update(cx, |m, cx| m.set_spring_mode(mode, cx));
        self.reload(cx);
    }

    /// Evaluate the beans under `mode`, as the project's settings now say, and read the model
    /// again if it was read.
    pub fn use_mode(&mut self, mode: ProfileMode, cx: &mut Context<Self>) {
        self.profile_mode = mode;
        if self.loaded {
            self.reload(cx);
        }
        cx.notify();
    }

    /// Read the beans and the endpoints again, the engine told the profiles first.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some((_, pid)) = self.runs.read(cx).engine(cx) else {
            self.load = Load::Failed("The Java engine is not ready yet".into());
            cx.notify();
            return;
        };
        self.load = Load::Loading;
        let set = self.runs.read(cx).request("snout/springSetProfiles", self.profile_mode.params(), cx);
        let runs = self.runs.clone();
        cx.spawn(async move |this, cx| {
            let profiles = set.await;
            let Ok((beans, endpoints)) = this.update(cx, |_, cx| {
                let r = runs.read(cx);
                (r.request("snout/springBeans", json!({}), cx), r.request("snout/springEndpoints", json!({}), cx))
            }) else {
                return;
            };
            let (beans, endpoints) = (beans.await, endpoints.await);
            let _ = this.update(cx, |this, cx| {
                this.from = Some(pid);
                match &profiles {
                    Ok(v) => this.profile_info = ProfileInfo::parse(v),
                    Err(e) => eprintln!("[spring] profiles: {e}"),
                }
                match (beans, endpoints) {
                    (Ok(b), Ok(e)) => this.set_model(parse_beans(&b), parse_endpoints(&e), cx),
                    (Err(e), _) | (_, Err(e)) => {
                        eprintln!("[spring] {e}");
                        this.load = Load::Failed(e.to_string());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Take a model read from the engine (or given by a test).
    pub fn set_model(&mut self, modules: Vec<ModuleBeans>, endpoints: Vec<Endpoint>, cx: &mut Context<Self>) {
        let beans: usize = modules.iter().map(|m| m.beans.len()).sum();
        eprintln!("[spring] {beans} beans in {} modules, {} endpoints", modules.len(), endpoints.len());
        self.modules = modules;
        self.endpoints = endpoints;
        self.load = Load::Idle;
        self.loaded = true;
        self.revision += 1;
        cx.notify();
    }

    /// A Java or Scala file was saved: read the model again soon (the engine reads the
    /// file again first).
    pub fn file_saved(&mut self, path: &Path, cx: &mut Context<Self>) {
        if !has_marks(path) || !self.loaded {
            return;
        }
        self._refresh = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1200)).await;
            let _ = this.update(cx, |this, cx| this.reload(cx));
        }));
    }

    /// The beans of every module.
    pub fn bean_count(&self) -> usize {
        self.modules.iter().map(|m| m.beans.len()).sum()
    }

    /// Ask for a file's marks: its injection points and beans, with its endpoints.
    pub fn file_marks(&self, path: &Path, cx: &App) -> Task<Option<FileSpring>> {
        if self.runs.read(cx).engine(cx).is_none() {
            return Task::ready(None);
        }
        let call = self.runs.read(cx).request("snout/springInjections", json!({ "path": path }), cx);
        let endpoints = self.endpoints.clone();
        let host = crate::host::host(cx);
        cx.background_executor().spawn(async move {
            match call.await {
                Ok(v) => Some(FileSpring::parse(&*host, &v, &endpoints)),
                Err(e) => {
                    eprintln!("[spring] injections: {e}");
                    None
                }
            }
        })
    }

    /// Show a place.
    pub fn open(&mut self, target: Target, cx: &mut Context<Self>) {
        cx.emit(SpringEvent::Open(target));
    }

    /// Show the Endpoints tab, searching for `path`.
    pub fn show_endpoint(&mut self, path: String, cx: &mut Context<Self>) {
        cx.emit(SpringEvent::Show { endpoints: Some(path) });
    }

    /// The profiles the beans name, sorted.
    pub fn profiles(&self) -> Vec<String> {
        let set: BTreeSet<String> =
            self.modules.iter().flat_map(|m| m.beans.iter()).flat_map(|b| b.profiles.iter().cloned()).collect();
        set.into_iter().collect()
    }

    /// The stereotypes the beans have, sorted.
    pub fn stereotypes(&self) -> Vec<String> {
        let set: BTreeSet<String> =
            self.modules.iter().flat_map(|m| m.beans.iter()).map(Bean::stereotype_label).collect();
        set.into_iter().collect()
    }
}

/// `@Value` hints from `textDocument/inlayHint`: the line (from 0) and its text.
pub fn parse_hints(v: &Value) -> Vec<(u32, String)> {
    let list: Vec<lsp_types::InlayHint> = serde_json::from_value(v.clone()).unwrap_or_default();
    let mut out: Vec<(u32, String)> = list
        .into_iter()
        .filter(|h| !crate::editor::is_param_hint(h))
        .map(|h| (h.position.line, crate::editor::hint_label(&h)))
        .collect();
    out.sort();
    out
}

/// The protocol range of a whole document of `lines` lines.
pub fn whole(lines: u32) -> Range {
    Range::new(Position::new(0, 0), Position::new(lines + 1, 0))
}

#[cfg(test)]
pub(crate) mod tests;
