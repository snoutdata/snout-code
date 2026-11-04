//! Node projects' runs: the scripts of `package.json` as run configurations (`npm run dev`,
//! with the package manager the lockfile names), and script files run on the project's
//! Node (`.ts` through the project's `tsx`, else Node's own type stripping). Each is a
//! process run; Debug gives it a `debug_node`, and js-debug does the rest (see
//! [`super::node_debug`]). A configuration of the Node kind with a port attaches to a Node
//! started elsewhere with `--inspect`.

use std::path::{Path, PathBuf};

use gpui::Context;
use serde_json::Value;
use snout_host::Host;

use super::config::{Kind, RunConfig};
use super::process::ProcessLaunch;
use super::{Launch, Place, Runs};
use crate::lang::node::{self, Node, PackageManager};

/// A script of `package.json`: its name, its command and the line (from 0) its name is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub name: String,
    pub command: String,
    pub line: u32,
}

/// The scripts of a `package.json`'s text, in the file's order.
pub fn scripts(text: &str) -> Vec<Script> {
    let Ok(json) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let Some(all) = json.get("scripts").and_then(Value::as_object) else { return Vec::new() };
    let lines: Vec<&str> = text.lines().collect();
    // Where the `scripts` object starts: names are looked for below it.
    let start = lines.iter().position(|l| l.contains("\"scripts\"")).unwrap_or(0);
    let mut out: Vec<Script> = all
        .iter()
        .filter_map(|(name, command)| {
            let key = serde_json::to_string(name).ok()?;
            let line = lines
                .iter()
                .enumerate()
                .skip(start)
                .find(|(i, l)| {
                    let rest = if *i == start { l.split_once("\"scripts\"").map_or("", |(_, r)| r) } else { l };
                    rest.split_once(&key).is_some_and(|(_, after)| after.trim_start().starts_with(':'))
                })
                .map_or(start, |(i, _)| i);
            Some(Script { name: name.clone(), command: command.as_str()?.to_owned(), line: line as u32 })
        })
        .collect();
    out.sort_by_key(|s| s.line);
    out
}

/// Whether `dir` holds a Gradle or Maven build (then it is a Java project with a
/// `package.json` beside, not a Node project).
pub fn has_java_build(host: &dyn Host, dir: &Path) -> bool {
    ["settings.gradle", "settings.gradle.kts", "build.gradle", "build.gradle.kts", "pom.xml"]
        .iter()
        .any(|f| host.is_file(&dir.join(f)))
}

/// Whether `dir` is a Node project's folder: a `package.json`, no Playwright, no Java build.
pub fn is_node_project(host: &dyn Host, dir: &Path) -> bool {
    host.is_file(&dir.join("package.json")) && !crate::playwright::is_root(host, dir) && !has_java_build(host, dir)
}

/// Whether `dir/package.json` lists packages to install.
pub fn has_dependencies(host: &dyn Host, dir: &Path) -> bool {
    let Ok(text) = host.read_to_string(&dir.join("package.json")) else { return false };
    let Ok(json) = serde_json::from_str::<Value>(&text) else { return false };
    json.get("workspaces").is_some()
        || ["dependencies", "devDependencies", "optionalDependencies"]
            .iter()
            .any(|k| json.get(k).and_then(Value::as_object).is_some_and(|m| !m.is_empty()))
}

/// Whether `path` is a script Current File can run: JavaScript or TypeScript, not a type
/// declaration, not a test file, not in `node_modules`.
pub fn is_script_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    matches!(ext, "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" | "tsx" | "jsx")
        && !name.ends_with(".d.ts")
        && !name.ends_with(".d.mts")
        && !name.ends_with(".d.cts")
        && !crate::playwright::is_test_file(path)
        && !path.components().any(|c| c.as_os_str() == "node_modules")
}

/// Folders never looked in for packages: installed packages and build output.
const NOT_PACKAGES: [&str; 8] =
    ["node_modules", "dist", "build", "out", "target", "coverage", "vendor", "bower_components"];

/// How deep below the project packages are looked for.
const PACKAGE_DEPTH: usize = 4;

/// The Node packages of the project at `root`: each folder with a `package.json` (the
/// project's own, a workspace's packages, a front end beside a Java build), the project's
/// first, then by path; not in `node_modules`, build output or hidden folders.
pub fn packages(host: &dyn Host, root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_owned(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if host.is_file(&dir.join("package.json")) {
            out.push(dir.clone());
        }
        if depth >= PACKAGE_DEPTH {
            continue;
        }
        let Ok(entries) = host.read_dir(&dir) else { continue };
        for e in entries.into_iter().filter(|e| e.is_dir) {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && !NOT_PACKAGES.contains(&name.as_str()) {
                stack.push((e.path.clone(), depth + 1));
            }
        }
    }
    out.sort_by(|a, b| (a != root, a).cmp(&(b != root, b)));
    out
}

/// The package `file` is in: the nearest folder above it with a `package.json`, up to
/// `root`.
pub fn package_of(host: &dyn Host, file: &Path, root: &Path) -> Option<PathBuf> {
    let mut at = file.parent();
    while let Some(d) = at {
        if host.is_file(&d.join("package.json")) {
            return Some(d.to_owned());
        }
        if d == root {
            break;
        }
        at = d.parent();
    }
    None
}

/// The package manager of the package at `dir`: by the lockfile beside it, else by the
/// nearest one above it up to `root` (a workspace keeps one, at its root).
pub fn manager_of(host: &dyn Host, dir: &Path, root: &Path) -> PackageManager {
    const LOCKS: [&str; 5] = ["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lockb", "bun.lock"];
    for d in dir.ancestors() {
        if LOCKS.iter().any(|l| host.is_file(&d.join(l))) {
            return PackageManager::of(host, d);
        }
        if d == root {
            break;
        }
    }
    PackageManager::Npm
}

/// Where the packages of the package at `dir` are installed: the nearest folder from it up
/// to `root` with a lockfile or a workspace's list of packages (`workspaces`,
/// `pnpm-workspace.yaml`), else its own.
pub fn install_dir(host: &dyn Host, dir: &Path, root: &Path) -> PathBuf {
    const MARKS: [&str; 6] =
        ["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lockb", "bun.lock", "pnpm-workspace.yaml"];
    for d in dir.ancestors() {
        let workspace = host
            .read_to_string(&d.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|j| j.get("workspaces").is_some());
        if workspace || MARKS.iter().any(|m| host.is_file(&d.join(m))) {
            return d.to_owned();
        }
        if d == root {
            break;
        }
    }
    dir.to_owned()
}

/// A package's folder as configurations name it, relative to the project (`apps/web`);
/// empty for the project's own.
fn rel(dir: &Path, root: &Path) -> String {
    dir.strip_prefix(root).unwrap_or(dir).to_string_lossy().replace('\\', "/")
}

/// The id of the configuration of script `name` of the package at `dir`.
pub fn script_id(dir: &Path, root: &Path, name: &str) -> String {
    match rel(dir, root) {
        r if r.is_empty() => format!("npm:{name}"),
        r => format!("npm:{r}:{name}"),
    }
}

/// The run configurations of the project at `root`: one for each script of each of its
/// packages, named as they run (`npm run dev`, `pnpm run build (apps/web)`).
pub fn configs(host: &dyn Host, root: &Path) -> Vec<RunConfig> {
    let mut out = Vec::new();
    for dir in packages(host, root) {
        let Ok(text) = host.read_to_string(&dir.join("package.json")) else { continue };
        let manager = manager_of(host, &dir, root);
        let folder = rel(&dir, root);
        for s in scripts(&text) {
            out.push(RunConfig {
                id: script_id(&dir, root, &s.name),
                kind: Kind::Npm,
                name: match folder.as_str() {
                    "" => format!("{} run {}", manager.name(), s.name),
                    f => format!("{} run {} ({f})", manager.name(), s.name),
                },
                build: Some(folder.clone()).filter(|f| !f.is_empty()),
                task: Some(s.name),
                ..Default::default()
            });
        }
    }
    out
}

/// The package a configuration runs in: its own folder for a script, the file's package
/// for a file; the project's folder when there is none.
pub fn config_package(host: &dyn Host, root: &Path, config: &RunConfig) -> PathBuf {
    match config.kind {
        Kind::Npm => {
            config.build.as_deref().filter(|b| !b.is_empty()).map_or_else(|| root.to_owned(), |b| root.join(b))
        }
        _ => config
            .task
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .and_then(|t| package_of(host, &root.join(t), root))
            .unwrap_or_else(|| root.to_owned()),
    }
}

/// The variables every Node run gets: `node` first on `PATH`, and colours.
fn base_env(host: &dyn Host, node: &Node) -> Vec<(String, String)> {
    vec![
        ("PATH".to_owned(), crate::lang::typescript::path_with(host, node)),
        ("FORCE_COLOR".to_owned(), "1".to_owned()),
    ]
}

/// The command that runs script `script` of the package at `dir` (in the project at
/// `root`) with its package manager, `args` after it.
pub fn script_launch(
    host: &dyn Host,
    dir: &Path,
    root: &Path,
    node: &Node,
    script: &str,
    args: &[String],
    env: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let manager = manager_of(host, dir, root);
    let (program, mut all) = match manager {
        PackageManager::Npm => {
            let npm = crate::lang::typescript::npm_cli(host, node).ok_or("npm is missing beside Node")?;
            (node.bin.clone(), vec![npm.display().to_string(), "run".to_owned(), script.to_owned()])
        }
        other => {
            let name = other.name();
            let program = snout_host::which(host, &[name], &[node.bin_dir().to_owned()])
                .ok_or_else(|| format!("{name} is not installed (the project's lockfile is {name}'s)"))?;
            (program, vec!["run".to_owned(), script.to_owned()])
        }
    };
    if !args.is_empty() {
        if manager == PackageManager::Npm {
            all.push("--".to_owned());
        }
        all.extend(args.iter().cloned());
    }
    let mut vars = base_env(host, node);
    vars.extend(env);
    Ok(ProcessLaunch {
        title: format!("{} run {script}", manager.name()),
        program,
        args: all,
        cwd: dir.to_owned(),
        env: vars,
        place: Place::Run,
        report: false,
        debug_node: None,
        attach: None,
    })
}

/// Whether `node` strips TypeScript's types by itself, and whether it wants a flag for it
/// (`--experimental-strip-types` before 22.18 and 23.6).
fn strips_types(node: &Node) -> Option<bool> {
    let mut parts = node.version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    match (major, minor) {
        (m, _) if m >= 24 => Some(false),
        (23, m) => Some(m < 6),
        (22, m) if m >= 18 => Some(false),
        (22, m) if m >= 6 => Some(true),
        _ => None,
    }
}

/// The program of the project's `tsx`, when it has the package.
fn tsx_cli(host: &dyn Host, root: &Path) -> Option<PathBuf> {
    let dir = crate::playwright::setup::package_dir(host, root, "tsx")?;
    let manifest: Value = serde_json::from_str(&host.read_to_string(&dir.join("package.json")).ok()?).ok()?;
    let bin = match manifest.get("bin")? {
        Value::String(s) => s.clone(),
        Value::Object(m) => m.get("tsx")?.as_str()?.to_owned(),
        _ => return None,
    };
    Some(dir.join(bin.trim_start_matches("./"))).filter(|p| host.is_file(p))
}

/// Whether the package around `file` loads `.ts` files as ES modules: a `.mts` file, or a
/// package whose `package.json` says `"type": "module"`.
fn es_module(host: &dyn Host, file: &Path) -> bool {
    match file.extension().and_then(|e| e.to_str()) {
        Some("mts") => return true,
        Some("cts") => return false,
        _ => {}
    }
    let manifest = file.ancestors().skip(1).map(|d| d.join("package.json")).find(|p| host.is_file(p));
    manifest
        .and_then(|p| host.read_to_string(&p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|j| j.get("type").and_then(Value::as_str) == Some("module"))
}

/// The Node options that load TypeScript through the project's `ts-node`, when it has the
/// package and the file is CommonJS: its transpiler (no type check, as the editor checks),
/// and `tsconfig-paths` for the `paths` of `tsconfig.json` when that is installed too.
fn ts_node_args(host: &dyn Host, root: &Path, file: &Path) -> Option<Vec<String>> {
    let ts_node = crate::playwright::setup::package_dir(host, root, "ts-node")?;
    let register = ts_node.join("register").join("transpile-only.js");
    if !host.is_file(&register) || es_module(host, file) {
        return None;
    }
    let mut args = vec!["-r".to_owned(), register.display().to_string()];
    if let Some(paths) = crate::playwright::setup::package_dir(host, root, "tsconfig-paths") {
        let register = paths.join("register.js");
        if host.is_file(&register) {
            args.extend(["-r".to_owned(), register.display().to_string()]);
        }
    }
    Some(args)
}

/// The command that runs `file` of the project at `root` on `node`: `node_args` for Node,
/// `args` for the program, in `cwd` (the project's folder when none).
#[allow(clippy::too_many_arguments)]
pub fn file_launch(
    host: &dyn Host,
    root: &Path,
    node: &Node,
    file: &Path,
    node_args: &[String],
    args: &[String],
    cwd: Option<&Path>,
    env: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or_default();
    let typescript = matches!(ext, "ts" | "mts" | "cts" | "tsx");
    let mut all: Vec<String> = Vec::new();
    if typescript {
        match (tsx_cli(host, root), ts_node_args(host, root, file), strips_types(node)) {
            (Some(tsx), _, _) => all.push(tsx.display().to_string()),
            (None, Some(register), _) => all.extend(register),
            (None, None, Some(flag)) if ext != "tsx" => {
                if flag {
                    all.push("--experimental-strip-types".to_owned());
                }
            }
            _ => {
                return Err(format!(
                    "Running a TypeScript file needs the tsx package in the project ({} add -D tsx) or Node 22.6 \
                     or later (this project runs on Node {}).",
                    PackageManager::of(host, root).name(),
                    node.version
                ));
            }
        }
    }
    all.extend(node_args.iter().cloned());
    all.push(file.display().to_string());
    all.extend(args.iter().cloned());
    let mut vars = base_env(host, node);
    vars.extend(env);
    Ok(ProcessLaunch {
        title: file.file_name().map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned()),
        program: node.bin.clone(),
        args: all,
        cwd: cwd.unwrap_or(root).to_owned(),
        env: vars,
        place: Place::Run,
        report: false,
        debug_node: None,
        attach: None,
    })
}

/// The configuration that runs `file` of the project at `root`.
pub fn file_config(root: &Path, file: &Path) -> RunConfig {
    let rel = file.strip_prefix(root).unwrap_or(file).to_string_lossy().replace('\\', "/");
    RunConfig {
        id: format!("node:file:{rel}"),
        kind: Kind::Node,
        name: file.file_name().map_or_else(|| rel.clone(), |n| n.to_string_lossy().into_owned()),
        task: Some(rel),
        ..Default::default()
    }
}

/// What the configuration `config` of the project at `root` runs, on `node` (the one its
/// package asks for); `debug` has js-debug launch it.
pub fn config_launch(
    host: &dyn Host,
    root: &Path,
    node: &Node,
    config: &RunConfig,
    debug: bool,
) -> Result<ProcessLaunch, String> {
    if let (Kind::Node, Some(port)) = (config.kind, config.port) {
        // Attaching is debugging, whichever button started it.
        return Ok(ProcessLaunch {
            title: if config.name.trim().is_empty() { format!("Attach to port {port}") } else { config.name.clone() },
            program: node.bin.clone(),
            args: vec![format!("--inspect=127.0.0.1:{port}")],
            cwd: root.to_owned(),
            env: Vec::new(),
            place: Place::Debug,
            report: false,
            debug_node: Some(node.bin.clone()),
            attach: Some(port),
        });
    }
    let env: Vec<(String, String)> = config.env.clone().into_iter().collect();
    let cwd = config.working_dir.as_deref().filter(|d| !d.trim().is_empty()).map(|d| root.join(d));
    let task = config.task.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let package = config_package(host, root, config);
    let mut launch = match config.kind {
        Kind::Npm => {
            let script = task.ok_or("The configuration names no script")?;
            let mut l = script_launch(host, &package, root, node, script, &config.args, env)?;
            if let Some(cwd) = cwd {
                l.cwd = cwd;
            }
            l
        }
        _ => {
            let file = task.ok_or("The configuration names no file to run")?;
            let file = root.join(file);
            if !host.is_file(&file) {
                return Err(format!("{} is not there", file.display()));
            }
            file_launch(host, &package, node, &file, &config.jvm_args, &config.args, cwd.as_deref(), env)?
        }
    };
    if !config.name.trim().is_empty() {
        launch.title = config.name.clone();
    }
    if debug {
        launch.place = Place::Debug;
        launch.debug_node = Some(node.bin.clone());
    }
    Ok(launch)
}

impl Runs {
    /// Read the open project's `package.json` scripts again, off the UI thread.
    pub fn load_node_configs(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.project_root.clone() else {
            self.node_configs.clear();
            return;
        };
        let host = crate::host::host(cx);
        let read = cx.background_executor().spawn(async move { configs(&*host, &root) });
        cx.spawn(async move |this, cx| {
            let list = read.await;
            let _ = this.update(cx, |this, cx| {
                if this.node_configs != list {
                    this.node_configs = list;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Run (or debug) a configuration of the npm or Node kind: its Node is found off the
    /// UI thread first.
    pub(super) fn start_node_config(&mut self, config: RunConfig, debug: bool, cx: &mut Context<Self>) {
        let Some(project) = self.model.read(cx).project_root() else {
            self.say("Open a project first", cx);
            return;
        };
        let host = crate::host::host(cx);
        let id = config.id.clone();
        let find = cx.background_executor().spawn(async move {
            let n = project_node(&*host, &config_package(&*host, &project, &config), &project)?;
            config_launch(&*host, &project, &n, &config, debug)
        });
        cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| match found {
                Ok(launch) => {
                    if let Some(session) = this.start(Launch::Process(launch), cx)
                        && let Some(s) = this.session_mut(session)
                    {
                        s.config = Some(id);
                    }
                }
                Err(why) => this.say(why, cx),
            });
        })
        .detach();
    }

    /// Run (or debug) `file` of the open project on its Node.
    pub fn run_node_file(&mut self, file: &Path, debug: bool, cx: &mut Context<Self>) {
        let Some(project) = self.model.read(cx).project_root() else {
            self.say("Open a project first", cx);
            return;
        };
        let config = file_config(&project, file);
        // As the title bar's configuration, when it is one of the list.
        let config = match self.configurations().into_iter().find(|c| c.id == config.id) {
            Some(known) => {
                self.select_config(Some(known.id.clone()), cx);
                known
            }
            None => config,
        };
        self.start_node_config(config, debug, cx);
    }

    /// Run (or debug) script `name` of the package at `dir`.
    pub fn run_npm_script(&mut self, dir: &Path, name: &str, debug: bool, cx: &mut Context<Self>) {
        let Some(project) = self.model.read(cx).project_root() else { return };
        let id = script_id(dir, &project, name);
        let folder = rel(dir, &project);
        let config = self.configurations().into_iter().find(|c| c.id == id).unwrap_or_else(|| RunConfig {
            id,
            kind: Kind::Npm,
            name: format!("run {name}"),
            build: Some(folder).filter(|f| !f.is_empty()),
            task: Some(name.to_owned()),
            ..Default::default()
        });
        if self.configurations().iter().any(|c| c.id == config.id) {
            self.select_config(Some(config.id.clone()), cx);
        }
        self.start_node_config(config, debug, cx);
    }
}

impl Runs {
    /// Run (or debug) `scope` of the package at `dir` with its unit test runner, into the
    /// Tests window.
    pub fn start_js_tests(
        &mut self,
        dir: PathBuf,
        runner: crate::jstest::Runner,
        scope: crate::jstest::Scope,
        debug: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.model.read(cx).project_root() else {
            self.say("Open a project first", cx);
            return;
        };
        let host = crate::host::host(cx);
        let at = dir.clone();
        let find = cx.background_executor().spawn(async move {
            let n = project_node(&*host, &at, &project)?;
            crate::jstest::test_launch(&*host, &at, &project, &n, runner, &scope, debug)
        });
        cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| match found {
                Ok(launch) => {
                    if let Some(id) = this.start(Launch::Process(launch), cx)
                        && let Some(run) = this.session_mut(id).and_then(|s| s.playwright.as_mut())
                    {
                        run.runner = Some(runner);
                        run.dir = Some(dir);
                    }
                }
                Err(why) => this.say(why, cx),
            });
        })
        .detach();
    }
}

/// The Node the package at `dir` of the project at `root` runs on.
pub fn project_node(host: &dyn Host, dir: &Path, root: &Path) -> Result<Node, String> {
    let wanted = node::wanted(host, dir, Some(root));
    let all = node::discover(host);
    node::choose(&wanted, &all)
        .cloned()
        .ok_or_else(|| "No Node fits this project: download it from the bar above the editor first.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_come_in_the_files_order_with_their_lines() {
        let text = r#"{
  "name": "shop",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest"
  },
  "dependencies": { "dev": "1.0.0" }
}"#;
        let s = scripts(text);
        let names: Vec<(&str, u32)> = s.iter().map(|s| (s.name.as_str(), s.line)).collect();
        assert_eq!(names, [("dev", 3), ("build", 4), ("test", 5)]);
        assert_eq!(s[1].command, "tsc && vite build");
        assert!(scripts("{ \"name\": \"x\" }").is_empty());
        assert!(scripts("not json").is_empty());
        let inline = scripts(r#"{"scripts": {"start": "node ."}}"#);
        assert_eq!(inline, [Script { name: "start".into(), command: "node .".into(), line: 0 }]);
    }

    #[test]
    fn script_files_are_scripts_not_tests_types_or_packages() {
        assert!(is_script_file(Path::new("/p/src/server.ts")));
        assert!(is_script_file(Path::new("/p/tools/gen.mjs")));
        assert!(!is_script_file(Path::new("/p/src/types.d.ts")));
        assert!(!is_script_file(Path::new("/p/src/cart.test.ts")));
        assert!(!is_script_file(Path::new("/p/node_modules/x/index.js")));
        assert!(!is_script_file(Path::new("/p/README.md")));
    }

    #[test]
    fn typescript_runs_with_type_stripping_from_node_22_6() {
        let n = |v: &str| Node { bin: "/n/node".into(), version: v.into(), major: node::major_of(v).unwrap() };
        assert_eq!(strips_types(&n("20.18.0")), None);
        assert_eq!(strips_types(&n("22.5.1")), None);
        assert_eq!(strips_types(&n("22.6.0")), Some(true));
        assert_eq!(strips_types(&n("22.18.0")), Some(false));
        assert_eq!(strips_types(&n("23.3.0")), Some(true));
        assert_eq!(strips_types(&n("23.6.0")), Some(false));
        assert_eq!(strips_types(&n("24.13.0")), Some(false));
    }

    #[test]
    fn a_file_configuration_is_named_for_the_file_relative_to_the_project() {
        let c = file_config(Path::new("/p"), Path::new("/p/src/server.ts"));
        assert_eq!(
            (c.id.as_str(), c.name.as_str(), c.task.as_deref()),
            ("node:file:src/server.ts", "server.ts", Some("src/server.ts"))
        );
        assert_eq!(c.kind, Kind::Node);
    }
}
