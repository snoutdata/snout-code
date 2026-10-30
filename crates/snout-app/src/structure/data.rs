//! What the Project Structure window shows, read from the engine's model (`gradleBuilds`, see
//! `engine/README.md`): each build's Gradle version and the JVM it ran on, every module of every
//! build (root, included, `buildSrc`) with its source sets, their roots, outputs and Java
//! levels, and each source set's resolved dependency graphs. Read off the UI thread, once per
//! model file, only while the window is open.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::project_model::RootKind;

/// Which build of a composite a module belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildKind {
    Root,
    Included,
    BuildSrc,
}

impl BuildKind {
    fn parse(kind: &str) -> Self {
        match kind {
            "included" => Self::Included,
            "buildSrc" => Self::BuildSrc,
            _ => Self::Root,
        }
    }
}

/// One Gradle build the project holds (a `gradleBuilds` entry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    pub id: String,
    pub root_dir: PathBuf,
    pub gradle_version: Option<String>,
    /// The JVM Gradle ran on in the last sync, and its version.
    pub gradle_java_home: Option<PathBuf>,
    pub java_version: Option<String>,
    pub synced: bool,
    /// A Maven build: `gradle_version` is its Maven version.
    pub maven: bool,
}

/// A source set of a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSet {
    pub name: String,
    pub test: bool,
    pub roots: Vec<(PathBuf, RootKind, bool)>,
    /// Where it compiles to: the classes folders, then the resources folder.
    pub outputs: Vec<PathBuf>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub release: Option<String>,
    pub toolchain: Option<String>,
    /// Its resolved graphs, by configuration role: `compileClasspath`, `runtimeClasspath`,
    /// `annotationProcessor`.
    pub graphs: BTreeMap<String, Graph>,
}

impl SourceSet {
    /// The Java level its sources are compiled at: `--release`, else the source compatibility.
    pub fn level(&self) -> Option<&str> {
        self.release.as_deref().or(self.source.as_deref())
    }

    /// The bytecode level: `--release`, else the target compatibility.
    pub fn target_level(&self) -> Option<&str> {
        self.release.as_deref().or(self.target.as_deref())
    }
}

/// One Gradle project of any build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleInfo {
    /// As the project model names it: `ledger:core`, `shop/tools`.
    pub id: String,
    pub name: String,
    /// The `gradleBuilds` entry it came from.
    pub build: String,
    pub kind: BuildKind,
    /// The included build (or `buildSrc`) it belongs to, when not the root build.
    pub included: Option<String>,
    pub gradle_path: String,
    pub dir: PathBuf,
    pub build_file: Option<PathBuf>,
    /// How deep it sits in the tree of modules.
    pub depth: usize,
    pub source: Option<String>,
    pub target: Option<String>,
    pub toolchain: Option<String>,
    pub source_sets: Vec<SourceSet>,
    pub plugins: Vec<String>,
    pub spring_boot: Option<String>,
    /// The scripts it applies (`apply from`), where a setting may be made too.
    pub applied: Vec<PathBuf>,
}

impl ModuleInfo {
    pub fn main(&self) -> Option<&SourceSet> {
        self.source_sets.iter().find(|s| s.name == "main")
    }

    /// The source set its tests compile in: `test`, else the first test source set.
    pub fn test(&self) -> Option<&SourceSet> {
        self.source_sets.iter().find(|s| s.name == "test").or_else(|| self.source_sets.iter().find(|s| s.test))
    }

    /// Its Java level: the main source set's, else the project's.
    pub fn level(&self) -> Option<&str> {
        self.main().and_then(SourceSet::level).or(self.source.as_deref())
    }

    pub fn target_level(&self) -> Option<&str> {
        self.main().and_then(SourceSet::target_level).or(self.target.as_deref())
    }

    /// Whether it compiles Java at all (a root project without the java plugin does not).
    pub fn is_jvm(&self) -> bool {
        !self.source_sets.is_empty()
    }

    /// What the tree says beside its name: `buildSrc`, the included build.
    pub fn note(&self) -> Option<String> {
        match self.kind {
            BuildKind::BuildSrc if self.gradle_path == ":" => Some("buildSrc".into()),
            BuildKind::Included if self.gradle_path == ":" => Some("included build".into()),
            _ => None,
        }
    }

    /// The graph of `scope`, if the module has one.
    pub fn graph(&self, scope: Scope) -> Option<&Graph> {
        let (set, role) = match scope {
            Scope::Compile => (self.main(), "compileClasspath"),
            Scope::Runtime => (self.main(), "runtimeClasspath"),
            Scope::Test => (self.test(), "runtimeClasspath"),
            Scope::AnnotationProcessor => (self.main(), "annotationProcessor"),
        };
        set.and_then(|s| s.graphs.get(role))
    }
}

/// The classpaths the Dependencies page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Compile,
    Runtime,
    Test,
    AnnotationProcessor,
}

impl Scope {
    pub const ALL: [Scope; 4] = [Scope::Compile, Scope::Runtime, Scope::Test, Scope::AnnotationProcessor];

    pub fn label(self) -> &'static str {
        match self {
            Scope::Compile => "Compile",
            Scope::Runtime => "Runtime",
            Scope::Test => "Test",
            Scope::AnnotationProcessor => "Annotation processor",
        }
    }

    /// What the classpath is, for the line under the scopes.
    pub fn detail(self) -> &'static str {
        match self {
            Scope::Compile => "What the main code compiles against (compileClasspath).",
            Scope::Runtime => "What the main code runs with (runtimeClasspath).",
            Scope::Test => "What the tests compile and run with (the test source set's runtimeClasspath).",
            Scope::AnnotationProcessor => "The annotation processors javac runs on the main code.",
        }
    }
}

/// A resolved dependency graph: the declared dependencies (`roots`) and every component
/// selected, once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub roots: Vec<Edge>,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// `group:name:version` from a repository.
    Module {
        group: String,
        name: String,
        version: String,
    },
    /// Another Gradle project: its build (`:` the same) and path.
    Project {
        build: String,
        path: String,
    },
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    /// Why this version was selected, when it is more than being asked for: `conflict`,
    /// `forced`, `rule` (a dependency management rule), `constraint`.
    pub reason: Option<String>,
    pub deps: Vec<Edge>,
}

impl Node {
    /// `group:name`, or the project's path.
    pub fn coordinates(&self) -> String {
        match &self.kind {
            NodeKind::Module { group, name, .. } => format!("{group}:{name}"),
            NodeKind::Project { path, .. } => format!("project {path}"),
            NodeKind::Other(id) => id.clone(),
        }
    }

    pub fn version(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Module { version, .. } => Some(version),
            _ => None,
        }
    }

    /// The text a filter is matched against.
    pub fn haystack(&self) -> String {
        match &self.kind {
            NodeKind::Module { group, name, version } => format!("{group}:{name}:{version}").to_lowercase(),
            NodeKind::Project { path, .. } => format!("project {path}").to_lowercase(),
            NodeKind::Other(id) => id.to_lowercase(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    To {
        to: usize,
        /// The version asked for, when another one won (empty: none, a platform gives it).
        requested: Option<String>,
        /// The configuration declaring it (`implementation`), for a declared dependency.
        declared_in: Option<String>,
    },
    Unresolved {
        requested: String,
        reason: String,
    },
}

/// A version asked for that lost to another: who asked (none for a declared dependency),
/// what for, and what was selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub from: Option<usize>,
    pub to: usize,
    pub requested: String,
}

impl Graph {
    /// Every version asked for that lost, once per component and version asked for.
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut out: Vec<Conflict> = Vec::new();
        let edges = self
            .roots
            .iter()
            .map(|e| (None, e))
            .chain(self.nodes.iter().enumerate().flat_map(|(i, n)| n.deps.iter().map(move |e| (Some(i), e))));
        for (from, e) in edges {
            if let Edge::To { to, requested: Some(r), .. } = e
                && !r.is_empty()
                && !out.iter().any(|c| c.to == *to && c.requested == *r)
            {
                out.push(Conflict { from, to: *to, requested: r.clone() });
            }
        }
        out
    }

    /// Whether some edge into `node` asked for another version.
    pub fn lost_versions(&self, node: usize) -> Vec<String> {
        self.conflicts().into_iter().filter(|c| c.to == node).map(|c| c.requested).collect()
    }

    /// For each node, the declared dependency that brings it in first (breadth first), so a
    /// transitive one can be shown in the build file through it.
    pub fn brought_by(&self) -> Vec<Option<usize>> {
        let mut out = vec![None; self.nodes.len()];
        let mut queue = std::collections::VecDeque::new();
        for (i, e) in self.roots.iter().enumerate() {
            if let Edge::To { to, .. } = e
                && out[*to].is_none()
            {
                out[*to] = Some(i);
                queue.push_back(*to);
            }
        }
        while let Some(n) = queue.pop_front() {
            let root = out[n];
            for e in &self.nodes[n].deps {
                if let Edge::To { to, .. } = e
                    && out[*to].is_none()
                {
                    out[*to] = root;
                    queue.push_back(*to);
                }
            }
        }
        out
    }

    /// The paths (node indexes from a declared dependency down) to each node `matches` takes,
    /// the shortest one per node, for a filtered tree.
    pub fn paths_to(&self, matches: impl Fn(usize, &Node) -> bool) -> Vec<Vec<usize>> {
        let mut parent: Vec<Option<Option<usize>>> = vec![None; self.nodes.len()];
        let mut queue = std::collections::VecDeque::new();
        for e in &self.roots {
            if let Edge::To { to, .. } = e
                && parent[*to].is_none()
            {
                parent[*to] = Some(None);
                queue.push_back(*to);
            }
        }
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            for e in &self.nodes[n].deps {
                if let Edge::To { to, .. } = e
                    && parent[*to].is_none()
                {
                    parent[*to] = Some(Some(n));
                    queue.push_back(*to);
                }
            }
        }
        order
            .into_iter()
            .filter(|n| matches(*n, &self.nodes[*n]))
            .map(|n| {
                let mut path = vec![n];
                let mut at = n;
                while let Some(Some(p)) = parent[at] {
                    path.push(p);
                    at = p;
                }
                path.reverse();
                path
            })
            .collect()
    }
}

/// The project as the window shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Structure {
    pub root: PathBuf,
    pub name: String,
    pub builds: Vec<BuildInfo>,
    /// Every module, in the order of the tree: each build's root project, its subprojects,
    /// then its included builds and `buildSrc`, each under it.
    pub modules: Vec<ModuleInfo>,
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn array<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).into_iter().flatten()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    array(v, key).filter_map(Value::as_str).map(str::to_owned).collect()
}

fn opt(v: &Value, key: &str) -> Option<String> {
    str_of(v, key).filter(|s| !s.is_empty()).map(str::to_owned)
}

fn edge(v: &Value, nodes: usize) -> Option<Edge> {
    if let Some(requested) = str_of(v, "unresolved") {
        return Some(Edge::Unresolved {
            requested: requested.to_owned(),
            reason: str_of(v, "reason").unwrap_or_default().to_owned(),
        });
    }
    let to = v.get("to")?.as_u64()? as usize;
    (to < nodes).then(|| Edge::To {
        to,
        requested: str_of(v, "requested").map(str::to_owned),
        declared_in: opt(v, "declaredIn"),
    })
}

fn graph(v: &Value) -> Graph {
    let raw: Vec<&Value> = array(v, "nodes").collect();
    let count = raw.len();
    let nodes = raw
        .into_iter()
        .map(|n| Node {
            kind: match str_of(n, "kind") {
                Some("module") => NodeKind::Module {
                    group: str_of(n, "group").unwrap_or_default().to_owned(),
                    name: str_of(n, "name").unwrap_or_default().to_owned(),
                    version: str_of(n, "version").unwrap_or_default().to_owned(),
                },
                Some("project") => NodeKind::Project {
                    build: str_of(n, "build").unwrap_or(":").to_owned(),
                    path: str_of(n, "path").unwrap_or(":").to_owned(),
                },
                _ => NodeKind::Other(str_of(n, "id").unwrap_or_default().to_owned()),
            },
            reason: opt(n, "reason"),
            deps: array(n, "deps").filter_map(|e| edge(e, count)).collect(),
        })
        .collect();
    Graph { roots: array(v, "roots").filter_map(|e| edge(e, count)).collect(), nodes }
}

/// The source sets a test task runs, and those named `test` or ending in `Test` or `Tests`.
fn test_sets(project: &Value) -> Vec<String> {
    let mut out: Vec<String> = array(project, "testTasks").flat_map(|t| strings(t, "sourceSets")).collect();
    for s in array(project, "sourceSets") {
        if let Some(name) = str_of(s, "name")
            && (name == "test" || name.ends_with("Test") || name.ends_with("Tests"))
        {
            out.push(name.to_owned());
        }
    }
    out.retain(|n| n != "main");
    out
}

fn source_set(s: &Value, tests: &[String]) -> SourceSet {
    let name = str_of(s, "name").unwrap_or_default().to_owned();
    let test = tests.contains(&name);
    let mut roots = Vec::new();
    // Sources, then resources: a folder that is both is a source root, as the engine has it.
    for resources in [false, true] {
        for r in array(s, "roots") {
            let kind = str_of(r, "kind").unwrap_or("source");
            if (kind == "resource") != resources {
                continue;
            }
            let Some(dir) = str_of(r, "dir").map(PathBuf::from) else { continue };
            if roots.iter().any(|(d, _, _)| *d == dir) {
                continue;
            }
            let kind = match (kind, test) {
                ("resource", false) => RootKind::Resources,
                ("resource", true) => RootKind::TestResources,
                ("generated", _) => RootKind::Generated,
                (_, false) => RootKind::Main,
                (_, true) => RootKind::Test,
            };
            let exists = r.get("exists").and_then(Value::as_bool).unwrap_or(true);
            roots.push((dir, kind, exists));
        }
    }
    let output = s.get("output").cloned().unwrap_or(Value::Null);
    let mut outputs: Vec<PathBuf> = strings(&output, "classesDirs").into_iter().map(PathBuf::from).collect();
    if let Some(r) = str_of(&output, "resourcesDir") {
        outputs.push(PathBuf::from(r));
    }
    let compile = s.get("compile").cloned().unwrap_or(Value::Null);
    let javac = compile
        .get("java")
        .or_else(|| compile.as_object().and_then(|o| o.values().next()))
        .cloned()
        .unwrap_or(Value::Null);
    let graphs = s
        .get("dependencyGraph")
        .and_then(Value::as_object)
        .map(|o| o.iter().map(|(k, v)| (k.clone(), graph(v))).collect())
        .unwrap_or_default();
    SourceSet {
        name,
        test,
        roots,
        outputs,
        source: opt(&javac, "sourceCompatibility"),
        target: opt(&javac, "targetCompatibility"),
        release: opt(&javac, "release"),
        toolchain: javac.get("toolchain").and_then(|t| opt(t, "languageVersion")),
        graphs,
    }
}

/// The window's view of a model of format 2 or later; `None` for another.
pub fn parse(json: &Value) -> Option<Structure> {
    if json.get("modelVersion").and_then(Value::as_u64).unwrap_or(0) < 2 {
        return None;
    }
    let root = PathBuf::from(str_of(json, "rootDir")?);
    let name = str_of(json, "name")
        .map(str::to_owned)
        .unwrap_or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let mut builds = Vec::new();
    let mut modules = Vec::new();
    for gb in array(json, "gradleBuilds") {
        let Some(id) = str_of(gb, "id") else { continue };
        let maven = str_of(gb, "tool") == Some("maven");
        builds.push(BuildInfo {
            id: id.to_owned(),
            maven,
            root_dir: str_of(gb, "rootDir").map(PathBuf::from).unwrap_or_default(),
            gradle_version: if maven { opt(gb, "mavenVersion") } else { opt(gb, "gradleVersion") },
            gradle_java_home: opt(gb, "gradleJavaHome").map(PathBuf::from),
            java_version: opt(gb, "javaVersion"),
            synced: str_of(gb, "status") != Some("failed"),
        });
        // The root build first, then the included builds and buildSrc, each a level down.
        let mut parts: Vec<&Value> = array(gb, "builds").collect();
        parts.sort_by_key(|b| match str_of(b, "kind") {
            Some("root") => 0,
            Some("included") => 1,
            _ => 2,
        });
        for b in parts {
            let kind = BuildKind::parse(str_of(b, "kind").unwrap_or("root"));
            let included = (kind != BuildKind::Root).then(|| str_of(b, "name").unwrap_or_default().to_owned());
            let prefix = match &included {
                None => id.to_owned(),
                Some(n) => format!("{id}/{n}"),
            };
            let mut projects: Vec<&Value> = array(b, "projects").collect();
            projects.sort_by(|a, b| {
                let (pa, pb) = (str_of(a, "path").unwrap_or(":"), str_of(b, "path").unwrap_or(":"));
                (pa != ":").cmp(&(pb != ":")).then_with(|| pa.cmp(pb))
            });
            for p in projects {
                let path = str_of(p, "path").unwrap_or(":").to_owned();
                let depth = path.matches(':').count() - usize::from(path == ":") + usize::from(included.is_some());
                let tests = test_sets(p);
                let java = p.get("java").cloned().unwrap_or(Value::Null);
                let boot = p.get("springBoot").filter(|v| v.is_object());
                modules.push(ModuleInfo {
                    id: if path == ":" { prefix.clone() } else { format!("{prefix}{path}") },
                    name: str_of(p, "name").unwrap_or_default().to_owned(),
                    build: id.to_owned(),
                    kind,
                    included: included.clone(),
                    dir: str_of(p, "projectDir").map(PathBuf::from).unwrap_or_default(),
                    build_file: str_of(p, "buildFile").map(PathBuf::from),
                    depth,
                    source: opt(&java, "sourceCompatibility"),
                    target: opt(&java, "targetCompatibility"),
                    toolchain: java.get("toolchain").and_then(|t| opt(t, "languageVersion")),
                    source_sets: array(p, "sourceSets").map(|s| source_set(s, &tests)).collect(),
                    plugins: array(p, "plugins").filter_map(|x| str_of(x, "id")).map(str::to_owned).collect(),
                    spring_boot: boot.map(|b| str_of(b, "pluginVersion").unwrap_or_default().to_owned()),
                    applied: p
                        .get("appliedScripts")
                        .map(|a| strings(a, "scripts"))
                        .unwrap_or_default()
                        .into_iter()
                        .map(PathBuf::from)
                        .collect(),
                    gradle_path: path,
                });
            }
        }
    }
    Some(Structure { root, name, builds, modules })
}

impl Structure {
    pub fn module(&self, id: &str) -> Option<&ModuleInfo> {
        self.modules.iter().find(|m| m.id == id)
    }

    pub fn build_of(&self, m: &ModuleInfo) -> Option<&BuildInfo> {
        self.builds.iter().find(|b| b.id == m.build)
    }

    /// The module a project dependency of `from` points to: same build when `build` is `:`,
    /// else the included build of that name.
    pub fn project_module(&self, from: &ModuleInfo, build: &str, path: &str) -> Option<&ModuleInfo> {
        let name = build.trim_start_matches(':');
        let same = |m: &&ModuleInfo| m.build == from.build && m.gradle_path == path;
        let hit = if name.is_empty() {
            self.modules.iter().filter(same).find(|m| m.included == from.included)
        } else {
            self.modules.iter().filter(same).find(|m| m.included.as_deref() == Some(name))
        };
        hit.or_else(|| self.modules.iter().filter(same).find(|m| m.included.is_none()))
    }

    /// The modules that compile Java, in tree order.
    pub fn jvm_modules(&self) -> impl Iterator<Item = &ModuleInfo> {
        self.modules.iter().filter(|m| m.is_jvm())
    }

    /// Modules with a Spring Boot plugin.
    pub fn spring_modules(&self) -> impl Iterator<Item = &ModuleInfo> {
        self.modules.iter().filter(|m| m.spring_boot.is_some())
    }

    /// The module whose folder holds `path` most closely.
    pub fn module_at(&self, path: &Path) -> Option<&ModuleInfo> {
        self.modules.iter().filter(|m| path.starts_with(&m.dir)).max_by_key(|m| m.dir.as_os_str().len())
    }
}

/// Read the model file at `file` on `host` for the window.
pub fn read(host: &dyn snout_host::Host, file: &Path) -> Result<Structure, String> {
    let text = host.read_to_string(file).map_err(|e| format!("The Gradle model could not be read: {e}"))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("The Gradle model is not JSON: {e}"))?;
    parse(&json).ok_or_else(|| "The Gradle model is of an older format: sync again".to_owned())
}
