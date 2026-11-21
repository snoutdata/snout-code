//! Cypress: a package that depends on `cypress` (or has a `cypress.config.*`) runs its specs
//! with its own Cypress (`cypress run`) and a reporter of ours (`snout-cypress-reporter.cjs`),
//! which tells the run in the protocol of the Playwright reporter, so the Tests window follows
//! it as it follows a Playwright run. The Cypress window, on the right, lists the specs with
//! how each last went, and runs them in the browser picked. A run that goes wrong in a way
//! known here (the browser crashed, the Cypress app is missing, the server the tests visit is
//! down) says so with the fix a click away.

pub mod window;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snout_host::Host;

use crate::jstest::{Runner, Scope};
use crate::lang::node::{Node, PackageManager};
use crate::run::Place;
use crate::run::process::ProcessLaunch;

pub const REPORTER: &str = include_str!("snout-cypress-reporter.cjs");

/// The configuration files that say a package tests with Cypress.
pub const CONFIGS: &[&str] = &[
    "cypress.config.ts",
    "cypress.config.js",
    "cypress.config.mjs",
    "cypress.config.cjs",
    "cypress.config.mts",
    "cypress.config.cts",
];

/// Where Cypress looks for end-to-end specs unless the configuration says otherwise.
pub const DEFAULT_SPECS: &str = "cypress/e2e/**/*.cy.{js,jsx,ts,tsx}";

/// Whether `path` is a Cypress spec as the default pattern names them: `login.cy.ts`.
pub fn is_spec(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let script = matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "mjs" | "cts" | "cjs")
    );
    script && name.contains(".cy.")
}

/// The configuration file of the package at `dir`, if it has one.
pub fn config_file(host: &dyn Host, dir: &Path) -> Option<PathBuf> {
    CONFIGS.iter().map(|c| dir.join(c)).find(|p| host.is_file(p))
}

/// The strings a setting of the configuration's text has: `specPattern: 'a'` or
/// `specPattern: ['a', 'b']`. The configuration is code; only a literal is read.
fn setting(text: &str, name: &str) -> Vec<String> {
    let pattern = format!(r#"{name}\s*:\s*(\[[^\]]*\]|'[^']*'|"[^"]*"|`[^`$]*`)"#);
    let Ok(re) = regex::Regex::new(&pattern) else { return Vec::new() };
    let Some(m) = re.captures(text).and_then(|c| c.get(1)) else { return Vec::new() };
    let quoted = regex::Regex::new(r#"'([^']*)'|"([^"]*)"|`([^`]*)`"#).expect("a pattern");
    quoted
        .captures_iter(m.as_str())
        .filter_map(|c| c.get(1).or(c.get(2)).or(c.get(3)).map(|g| g.as_str().to_owned()))
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// The spec patterns of the package at `dir`: its configuration's `specPattern`, else
/// Cypress's own.
pub fn spec_patterns(host: &dyn Host, dir: &Path) -> Vec<String> {
    let text = config_file(host, dir).and_then(|f| host.read_to_string(&f).ok()).unwrap_or_default();
    let found = setting(&text, "specPattern");
    if found.is_empty() { vec![DEFAULT_SPECS.to_owned()] } else { found }
}

/// The folder Cypress keeps the screenshots of failed tests in.
pub fn screenshots_dir(host: &dyn Host, dir: &Path) -> PathBuf {
    let text = config_file(host, dir).and_then(|f| host.read_to_string(&f).ok()).unwrap_or_default();
    let folder = setting(&text, "screenshotsFolder").into_iter().next().unwrap_or_else(|| "cypress/screenshots".into());
    dir.join(folder)
}

/// The specs of the package at `dir`, as its patterns find them, in order.
pub fn specs(host: &dyn Host, dir: &Path) -> Vec<PathBuf> {
    let mut set = globset::GlobSetBuilder::new();
    for p in spec_patterns(host, dir) {
        if let Ok(g) = globset::GlobBuilder::new(p.trim_start_matches("./")).literal_separator(true).build() {
            set.add(g);
        }
    }
    let Ok(set) = set.build() else { return Vec::new() };
    let options = snout_host::WalkOptions {
        gitignore: true,
        skip: vec!["node_modules".into(), ".git".into()],
        files_only: true,
        ..Default::default()
    };
    let mut out: Vec<PathBuf> = host
        .walk(dir, &options)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| !e.is_dir)
        .filter(|e| e.path.strip_prefix(dir).is_ok_and(|rel| set.is_match(rel.to_string_lossy().replace('\\', "/"))))
        .map(|e| e.path)
        .collect();
    out.sort();
    out
}

/// The version of the Cypress installed for the package at `dir`.
pub fn version(host: &dyn Host, dir: &Path) -> Option<String> {
    let pkg = crate::playwright::setup::package_dir(host, dir, "cypress")?;
    let manifest: serde_json::Value =
        serde_json::from_str(&host.read_to_string(&pkg.join("package.json")).ok()?).ok()?;
    manifest.get("version")?.as_str().map(str::to_owned)
}

/// The browser Cypress runs the specs in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Browser {
    /// The one Cypress comes with.
    #[default]
    Electron,
    Chrome,
    Firefox,
    Edge,
}

impl Browser {
    pub const ALL: [Browser; 4] = [Browser::Electron, Browser::Chrome, Browser::Firefox, Browser::Edge];

    pub fn name(self) -> &'static str {
        match self {
            Browser::Electron => "Electron",
            Browser::Chrome => "Chrome",
            Browser::Firefox => "Firefox",
            Browser::Edge => "Edge",
        }
    }

    /// What `--browser` takes.
    pub fn arg(self) -> &'static str {
        match self {
            Browser::Electron => "electron",
            Browser::Chrome => "chrome",
            Browser::Firefox => "firefox",
            Browser::Edge => "edge",
        }
    }

    /// Whether it is installed on `host` (Electron comes with Cypress).
    pub fn installed(self, host: &dyn Host) -> bool {
        let info = host.info();
        let home = info.home.clone();
        let app = |name: &str| {
            host.is_dir(&Path::new("/Applications").join(name)) || host.is_dir(&home.join("Applications").join(name))
        };
        let on_path = |names: &[&str]| snout_host::which(host, names, &[]).is_some();
        match self {
            Browser::Electron => true,
            _ if info.is_mac() => app(match self {
                Browser::Chrome => "Google Chrome.app",
                Browser::Firefox => "Firefox.app",
                _ => "Microsoft Edge.app",
            }),
            _ if info.is_windows() => {
                let roots = [r"C:\Program Files", r"C:\Program Files (x86)"];
                let rel = match self {
                    Browser::Chrome => r"Google\Chrome\Application\chrome.exe",
                    Browser::Firefox => r"Mozilla Firefox\firefox.exe",
                    _ => r"Microsoft\Edge\Application\msedge.exe",
                };
                roots.iter().any(|r| host.is_file(&Path::new(r).join(rel)))
            }
            Browser::Chrome => on_path(&["google-chrome", "google-chrome-stable"]),
            Browser::Firefox => on_path(&["firefox"]),
            Browser::Edge => on_path(&["microsoft-edge", "microsoft-edge-stable"]),
        }
    }
}

/// How a run goes, besides what it runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub browser: Browser,
    /// The browser shows (`--headed`).
    pub headed: bool,
}

/// The variables a run gets from the environment picked: each as it is (a configuration
/// file reads `process.env`) and as `CYPRESS_<name>` (which `Cypress.env()` reads), and the
/// application's URL as Cypress's `baseUrl`.
pub fn environment_vars(vars: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (k, v) in vars {
        out.push((k.clone(), v.clone()));
        if k == "BASE_URL" {
            out.push(("CYPRESS_baseUrl".to_owned(), v.clone()));
        } else if !k.starts_with("CYPRESS_") {
            out.push((format!("CYPRESS_{k}"), v.clone()));
        }
    }
    out
}

/// A spec as `--spec` takes it: relative to the package, `/` between.
fn spec_arg(dir: &Path, file: &Path) -> String {
    file.strip_prefix(dir).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// The specs `scope` runs (none: every spec). Cypress runs whole specs: a test runs its spec.
pub fn scope_specs(dir: &Path, scope: &Scope) -> Vec<String> {
    let mut files: Vec<String> = match scope {
        Scope::All => Vec::new(),
        Scope::File(f) | Scope::At { file: f, .. } => vec![spec_arg(dir, f)],
        Scope::Tests(tests) => tests.iter().map(|(f, _)| spec_arg(dir, f)).collect(),
    };
    files.sort();
    files.dedup();
    files
}

/// What a `cypress run`'s arguments ran: its `--spec` files (else every spec), and whether
/// headed.
pub fn scope_of_args(dir: &Path, args: &[String]) -> (Scope, bool) {
    let headed = args.iter().any(|a| a == "--headed");
    let specs: Vec<PathBuf> = args
        .iter()
        .position(|a| a == "--spec" || a == "-s")
        .and_then(|i| args.get(i + 1))
        .map(|list| list.split(',').filter(|f| !f.trim().is_empty()).map(|f| dir.join(f.trim())).collect())
        .unwrap_or_default();
    let scope = match specs.len() {
        0 => Scope::All,
        1 => Scope::File(specs[0].clone()),
        _ => Scope::Tests(specs.into_iter().map(|f| (f, Vec::new())).collect()),
    };
    (scope, headed)
}

/// The command that runs `scope` of the package at `dir` with Cypress, on `node`, with the
/// variables `vars` (the environment's, as [`environment_vars`] makes them).
pub fn run_launch(
    host: &dyn Host,
    dir: &Path,
    root: &Path,
    node: &Node,
    scope: &Scope,
    options: &Options,
    vars: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let cli = crate::jstest::cli(host, dir, Runner::Cypress)
        .ok_or_else(|| t!("tests-is-not-installed-install-the-packages-first", runner = "Cypress".to_string()))?;
    let reporter = crate::jstest::reporter(host, Runner::Cypress)
        .map_err(|e| t!("tests-the-snoutcode-reporter-could-not-be-written", e = e.to_string()))?;
    let mut args = vec![cli.display().to_string(), "run".into(), "--reporter".into(), reporter.display().to_string()];
    let specs = scope_specs(dir, scope);
    if !specs.is_empty() {
        args.extend(["--spec".to_owned(), specs.join(",")]);
    }
    if options.browser != Browser::Electron {
        args.extend(["--browser".to_owned(), options.browser.arg().to_owned()]);
    }
    if options.headed {
        args.push("--headed".into());
    }
    let mut env = vec![
        ("PATH".to_owned(), crate::lang::typescript::path_with(host, node)),
        ("FORCE_COLOR".to_owned(), "1".to_owned()),
        ("SNOUT_CYPRESS_ROOT".to_owned(), dir.display().to_string()),
        ("SNOUT_CYPRESS_BROWSER".to_owned(), options.browser.name().to_owned()),
    ];
    env.extend(vars);
    let title = match scope {
        Scope::All => {
            let rel = spec_arg(root, dir);
            if rel.is_empty() { "Cypress".to_owned() } else { t!("tests-in-2", runner = "Cypress", rel = rel) }
        }
        _ if specs.len() == 1 => {
            Path::new(&specs[0]).file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        }
        _ => t!("cypress-specs", count = (specs.len() as u64)),
    };
    Ok(ProcessLaunch {
        title,
        program: node.bin.clone(),
        args,
        cwd: dir.to_owned(),
        env,
        place: Place::Run,
        report: true,
        debug_node: None,
        attach: None,
    })
}

/// When script `name` of the package at `dir` is a plain `cypress run` (`npx cypress run
/// --browser chrome`, nothing chained after it), the browser it runs in: the arguments a run
/// adds reach Cypress, so our reporter can join it.
pub fn script_runs_cypress(host: &dyn Host, dir: &Path, name: &str) -> Option<Browser> {
    let text = host.read_to_string(&dir.join("package.json")).ok()?;
    let script = crate::run::node_run::scripts(&text).into_iter().find(|s| s.name == name)?;
    let command = script.command.trim();
    if command.contains("&&") || command.contains("||") || command.contains(';') || command.contains('|') {
        return None;
    }
    let words: Vec<&str> = command.split_whitespace().collect();
    let at = words.iter().position(|w| *w == "cypress" || w.ends_with("/cypress"))?;
    let launcher = words[..at].iter().all(|w| matches!(*w, "npx" | "pnpm" | "exec" | "yarn" | "bunx" | "--no-install"));
    if !launcher || words.get(at + 1) != Some(&"run") {
        return None;
    }
    let named = |b: &str| Browser::ALL.into_iter().find(|x| x.arg() == b);
    let after = words.iter().position(|w| *w == "--browser" || *w == "-b").and_then(|i| words.get(i + 1));
    let inline = words.iter().find_map(|w| w.strip_prefix("--browser="));
    Some(after.copied().or(inline).and_then(named).unwrap_or_default())
}

/// Arguments for a command, and variables for its environment.
pub type ArgsAndVars = (Vec<String>, Vec<(String, String)>);

/// The arguments that add our reporter to a `cypress run`, and the variables it reads.
pub fn reporter_args(host: &dyn Host, dir: &Path, browser: Browser) -> Result<ArgsAndVars, String> {
    let reporter = crate::jstest::reporter(host, Runner::Cypress)
        .map_err(|e| t!("tests-the-snoutcode-reporter-could-not-be-written", e = e.to_string()))?;
    Ok((
        vec!["--reporter".into(), reporter.display().to_string()],
        vec![
            ("SNOUT_CYPRESS_ROOT".to_owned(), dir.display().to_string()),
            ("SNOUT_CYPRESS_BROWSER".to_owned(), browser.name().to_owned()),
        ],
    ))
}

/// The command that opens the Cypress app on the package at `dir`, its end-to-end specs
/// in the browser picked, to step through a test.
pub fn open_launch(
    host: &dyn Host,
    dir: &Path,
    node: &Node,
    browser: Browser,
    vars: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let cli = crate::jstest::cli(host, dir, Runner::Cypress)
        .ok_or_else(|| t!("tests-is-not-installed-install-the-packages-first", runner = "Cypress".to_string()))?;
    let mut env = vec![("PATH".to_owned(), crate::lang::typescript::path_with(host, node))];
    env.extend(vars);
    Ok(ProcessLaunch {
        title: ts!("cypress-open-cypress").to_owned(),
        program: node.bin.clone(),
        args: vec![cli.display().to_string(), "open".into(), "--e2e".into(), "--browser".into(), browser.arg().into()],
        cwd: dir.to_owned(),
        env,
        place: Place::Run,
        report: false,
        debug_node: None,
        attach: None,
    })
}

/// Something that went wrong in a run, known here, with its fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The browser (Electron, unless another was picked) went down with a signal: most often
    /// an old Cypress on a newer system, as it closes once the specs are done.
    Crashed { signal: String },
    /// The browser crashed, and this Mac has no Rosetta for Cypress's Intel helper: the
    /// crash comes from that (it stops once Rosetta is in), so the two are one problem.
    /// [`ranked`] makes it from the two; a scan never reports it.
    CrashedNoRosetta { signal: String },
    /// The `cypress` package is installed but not the Cypress app it downloads.
    NoBinary,
    /// A helper of Cypress's is built for Intel Macs, and this Mac has no Rosetta.
    NeedsRosetta,
    /// The application the tests visit (`baseUrl`) did not answer.
    ServerDown { url: String },
}

/// What a click on a problem's button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fix {
    /// Install the latest Cypress into the package, then run again.
    UpdateCypress,
    /// Run in this browser from now on, and run again.
    UseBrowser(Browser),
    /// Download the Cypress app (`cypress install`), then run again.
    InstallBinary,
    /// Install Rosetta (`softwareupdate --install-rosetta`).
    InstallRosetta,
    /// Pick or edit the environment the tests point at.
    EditEnvironments,
    RunAgain,
}

impl Fix {
    pub fn label(self) -> String {
        match self {
            Fix::UpdateCypress => ts!("cypress-fix-update").to_owned(),
            Fix::UseBrowser(b) => t!("cypress-fix-use-browser", browser = b.name()),
            Fix::InstallBinary => ts!("cypress-fix-install-binary").to_owned(),
            Fix::InstallRosetta => ts!("cypress-fix-install-rosetta").to_owned(),
            Fix::EditEnvironments => ts!("cypress-fix-edit-environments").to_owned(),
            Fix::RunAgain => ts!("cypress-fix-run-again").to_owned(),
        }
    }
}

impl Problem {
    /// The problem's headline; `passed` is whether every test that ran passed.
    pub fn title(&self, passed: bool) -> String {
        match self {
            Problem::Crashed { .. } | Problem::CrashedNoRosetta { .. } if passed => {
                ts!("cypress-problem-crashed-closing").to_owned()
            }
            Problem::Crashed { .. } | Problem::CrashedNoRosetta { .. } => ts!("cypress-problem-crashed").to_owned(),
            Problem::NoBinary => ts!("cypress-problem-no-binary").to_owned(),
            Problem::NeedsRosetta => ts!("cypress-problem-rosetta").to_owned(),
            Problem::ServerDown { url } => t!("cypress-problem-server-down", url = url.clone()),
        }
    }

    /// What it means and what fixes it; `version` is the Cypress installed, `passed` whether
    /// every test that ran passed.
    pub fn detail(&self, version: Option<&str>, passed: bool) -> String {
        match self {
            Problem::Crashed { signal } => {
                let v = version.unwrap_or("?").to_owned();
                if passed {
                    t!("cypress-problem-crashed-passed", version = v, signal = signal.clone())
                } else {
                    t!("cypress-problem-crashed-detail", version = v, signal = signal.clone())
                }
            }
            Problem::CrashedNoRosetta { signal } => {
                if passed {
                    t!("cypress-problem-crashed-rosetta-passed", signal = signal.clone())
                } else {
                    t!("cypress-problem-crashed-rosetta-detail", signal = signal.clone())
                }
            }
            Problem::NoBinary => ts!("cypress-problem-no-binary-detail").to_owned(),
            Problem::NeedsRosetta => ts!("cypress-problem-rosetta-detail").to_owned(),
            Problem::ServerDown { .. } => ts!("cypress-problem-server-down-detail").to_owned(),
        }
    }

    /// Its fixes, the first the one to try first; `browser` is the one the run used, and
    /// `installed` says which others are there.
    pub fn fixes(&self, browser: Browser, installed: &[Browser]) -> Vec<Fix> {
        match self {
            Problem::Crashed { .. } => {
                let mut out = vec![Fix::UpdateCypress];
                if let Some(other) = [Browser::Chrome, Browser::Edge, Browser::Firefox]
                    .into_iter()
                    .find(|b| *b != browser && installed.contains(b))
                {
                    out.push(Fix::UseBrowser(other));
                }
                out
            }
            Problem::CrashedNoRosetta { .. } => vec![Fix::InstallRosetta],
            Problem::NoBinary => vec![Fix::InstallBinary],
            Problem::NeedsRosetta => vec![Fix::InstallRosetta],
            Problem::ServerDown { .. } => vec![Fix::EditEnvironments, Fix::RunAgain],
        }
    }

    /// Whether it is only a nuisance: the run's results stand. A crash as the browser closes
    /// is one when every test passed.
    pub fn minor(&self, passed: bool) -> bool {
        match self {
            Problem::NeedsRosetta => true,
            Problem::Crashed { .. } | Problem::CrashedNoRosetta { .. } => passed,
            Problem::NoBinary | Problem::ServerDown { .. } => false,
        }
    }
}

/// The problems of a run as the run shows them, the one that matters most first: that one in
/// full, the rest as a line each under it. A crash on a Mac without Rosetta is one problem,
/// Rosetta's.
pub fn ranked(problems: &[Problem], passed: bool) -> Vec<Problem> {
    let fold =
        problems.contains(&Problem::NeedsRosetta) && problems.iter().any(|p| matches!(p, Problem::Crashed { .. }));
    let mut out: Vec<Problem> = problems
        .iter()
        .filter(|p| !(fold && **p == Problem::NeedsRosetta))
        .map(|p| match p {
            Problem::Crashed { signal } if fold => Problem::CrashedNoRosetta { signal: signal.clone() },
            p => p.clone(),
        })
        .collect();
    // Rosetta last of all: the others have a cause worth fixing first.
    out.sort_by_key(|p| (p.minor(passed), matches!(p, Problem::NeedsRosetta)));
    out
}

/// Watches a Cypress run's output for the problems known here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub problems: Vec<Problem>,
    /// The last line said the server did not answer: its URL comes next.
    server_next: bool,
}

impl Scan {
    /// Look at one line Cypress printed (its colours taken out).
    pub fn see(&mut self, line: &str) {
        let line = crate::playwright::report::strip_ansi(line);
        let l = line.trim();
        let found = if let Some(at) =
            l.find("exited via a exit event with signal ").or_else(|| l.find("exited via an exit event with signal "))
        {
            let rest = &l[at..];
            let signal = rest.rsplit(' ').next().unwrap_or("").trim_end_matches('.').to_owned();
            Some(Problem::Crashed { signal })
        } else if l.contains("the Cypress binary is missing") || l.contains("No version of Cypress is installed in") {
            Some(Problem::NoBinary)
        } else if l.contains("term-size") && l.contains("Bad CPU type in executable") {
            Some(Problem::NeedsRosetta)
        } else if l.contains("Cypress could not verify that this server is running") {
            self.server_next = true;
            None
        } else if self.server_next && (l.contains("http://") || l.contains("https://")) {
            self.server_next = false;
            let url = l.split_whitespace().find(|w| w.starts_with("http")).unwrap_or(l).to_owned();
            Some(Problem::ServerDown { url })
        } else {
            None
        };
        if let Some(p) = found
            && !self.problems.contains(&p)
        {
            self.problems.push(p);
        }
    }
}

/// The command that carries out `fix` for the package at `dir` of the project at `root`
/// (none for the fixes that start nothing).
pub fn fix_launch(host: &dyn Host, dir: &Path, root: &Path, node: &Node, fix: Fix) -> Result<ProcessLaunch, String> {
    let env = vec![
        ("PATH".to_owned(), crate::lang::typescript::path_with(host, node)),
        ("FORCE_COLOR".to_owned(), "1".to_owned()),
    ];
    let launch = |title: String, program: PathBuf, args: Vec<String>, cwd: &Path| ProcessLaunch {
        title,
        program,
        args,
        cwd: cwd.to_owned(),
        env: env.clone(),
        place: Place::Build,
        report: false,
        debug_node: None,
        attach: None,
    };
    match fix {
        Fix::UpdateCypress => {
            let manager = crate::run::node_run::manager_of(host, dir, root);
            let title = ts!("cypress-fix-update").to_owned();
            match manager {
                PackageManager::Npm => {
                    let npm =
                        crate::lang::typescript::npm_cli(host, node).ok_or(ts!("run-npm-is-missing-beside-node"))?;
                    let args =
                        vec![npm.display().to_string(), "install".into(), "--save-dev".into(), "cypress@latest".into()];
                    Ok(launch(title, node.bin.clone(), args, dir))
                }
                other => {
                    let name = other.name();
                    let program = snout_host::which(host, &[name], &[node.bin_dir().to_owned()]).ok_or_else(|| {
                        t!("run-is-not-installed-the-projects-lockfile-is-s", name = name.to_string())
                    })?;
                    let dev = if other == PackageManager::Bun { "-d" } else { "-D" };
                    Ok(launch(title, program, vec!["add".into(), dev.into(), "cypress@latest".into()], dir))
                }
            }
        }
        Fix::InstallBinary => {
            let cli = crate::jstest::cli(host, dir, Runner::Cypress).ok_or_else(|| {
                t!("tests-is-not-installed-install-the-packages-first", runner = "Cypress".to_string())
            })?;
            Ok(launch(
                ts!("cypress-fix-install-binary").to_owned(),
                node.bin.clone(),
                vec![cli.display().to_string(), "install".into()],
                dir,
            ))
        }
        Fix::InstallRosetta => Ok(launch(
            ts!("cypress-fix-install-rosetta").to_owned(),
            "/usr/sbin/softwareupdate".into(),
            vec!["--install-rosetta".into(), "--agree-to-license".into()],
            dir,
        )),
        Fix::UseBrowser(_) | Fix::EditEnvironments | Fix::RunAgain => Err(String::new()),
    }
}

/// The screenshots Cypress took of the run's failed tests, put with their tests: Cypress
/// names each after the test's titles (`math -- fails (failed).png`, ` (attempt 2)` for a
/// retry) in a folder named after its spec.
pub fn attach_screenshots(host: &dyn Host, dir: &Path, run: &mut crate::playwright::report::Run) {
    let failed: Vec<String> = run.failures().iter().map(|t| t.id.clone()).collect();
    if failed.is_empty() {
        return;
    }
    let shots = screenshots_dir(host, dir);
    let options = snout_host::WalkOptions { files_only: true, ..Default::default() };
    let files: Vec<PathBuf> = host
        .walk(&shots, &options)
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.path)
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    for id in failed {
        let Some(t) = run.tests.get_mut(&id) else { continue };
        let spec = t.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stem = t.path.join(" -- ");
        let mine: Vec<PathBuf> = files
            .iter()
            .filter(|f| f.parent().and_then(|p| p.file_name()).is_some_and(|n| n.to_string_lossy() == spec))
            .filter(|f| {
                let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                name.strip_prefix(&stem).is_some_and(|rest| rest.contains("(failed)"))
            })
            .cloned()
            .collect();
        if let Some(last) = t.tries.last_mut() {
            for path in mine {
                if !last.attachments.iter().any(|a| a.path == path) {
                    last.attachments.push(crate::playwright::report::Attachment {
                        name: "screenshot".into(),
                        content_type: "image/png".into(),
                        path,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_named_cy() {
        assert!(is_spec(Path::new("/p/cypress/e2e/login.cy.ts")));
        assert!(is_spec(Path::new("/p/cypress/e2e/shop/cart.cy.jsx")));
        assert!(!is_spec(Path::new("/p/cypress/support/e2e.ts")));
        assert!(!is_spec(Path::new("/p/cypress.config.ts")));
    }

    #[test]
    fn the_configuration_names_its_spec_pattern_as_a_literal() {
        let one = "export default defineConfig({ e2e: { specPattern: 'tests/**/*.spec.ts', baseUrl: 'x' } });";
        assert_eq!(setting(one, "specPattern"), ["tests/**/*.spec.ts"]);
        let many = "e2e: {\n  specPattern: [\"a/*.cy.ts\", 'b/*.cy.js'],\n}";
        assert_eq!(setting(many, "specPattern"), ["a/*.cy.ts", "b/*.cy.js"]);
        assert!(setting("e2e: { specPattern: patterns }", "specPattern").is_empty(), "code is not read");
    }

    #[test]
    fn the_environment_reaches_cypress_by_its_names() {
        let vars =
            vec![("BASE_URL".to_owned(), "https://staging.test".to_owned()), ("USER".to_owned(), "qa".to_owned())];
        assert_eq!(
            environment_vars(&vars),
            [
                ("BASE_URL".to_owned(), "https://staging.test".to_owned()),
                ("CYPRESS_baseUrl".to_owned(), "https://staging.test".to_owned()),
                ("USER".to_owned(), "qa".to_owned()),
                ("CYPRESS_USER".to_owned(), "qa".to_owned()),
            ]
        );
    }

    #[test]
    fn a_test_runs_its_spec() {
        let dir = Path::new("/p");
        let at = Scope::At { file: dir.join("cypress/e2e/a.cy.ts"), path: vec!["a".into()], group: false };
        assert_eq!(scope_specs(dir, &at), ["cypress/e2e/a.cy.ts"]);
        let tests = Scope::Tests(vec![
            (dir.join("cypress/e2e/b.cy.ts"), vec!["x".into()]),
            (dir.join("cypress/e2e/a.cy.ts"), vec!["y".into()]),
            (dir.join("cypress/e2e/b.cy.ts"), vec!["z".into()]),
        ]);
        assert_eq!(scope_specs(dir, &tests), ["cypress/e2e/a.cy.ts", "cypress/e2e/b.cy.ts"]);
        assert!(scope_specs(dir, &Scope::All).is_empty());
    }

    #[test]
    fn the_problems_cypress_prints_are_known() {
        let mut s = Scan::default();
        s.see("\u{1b}[31mThe Test Runner unexpectedly exited via a exit event with signal SIGSEGV\u{1b}[39m");
        s.see("/bin/sh: /Users/x/Library/Caches/Cypress/13.6.4/Cypress.app/Contents/Resources/app/node_modules/term-size/vendor/macos/term-size: Bad CPU type in executable");
        s.see("Cypress could not verify that this server is running:");
        s.see("");
        s.see("  > http://localhost:3000");
        s.see("The Test Runner unexpectedly exited via a exit event with signal SIGSEGV");
        assert_eq!(
            s.problems,
            [
                Problem::Crashed { signal: "SIGSEGV".into() },
                Problem::NeedsRosetta,
                Problem::ServerDown { url: "http://localhost:3000".into() },
            ]
        );
        let crashed = &s.problems[0];
        assert_eq!(
            crashed.fixes(Browser::Electron, &[Browser::Electron, Browser::Chrome]),
            [Fix::UpdateCypress, Fix::UseBrowser(Browser::Chrome)]
        );
        assert_eq!(crashed.fixes(Browser::Chrome, &[Browser::Electron, Browser::Chrome]), [Fix::UpdateCypress]);
        assert!(s.problems[1].minor(false) && !crashed.minor(false));
    }

    #[test]
    fn a_crash_after_every_test_passed_is_minor_and_the_serious_problem_leads() {
        let crashed = Problem::Crashed { signal: "SIGSEGV".into() };
        let server = Problem::ServerDown { url: "http://localhost:3000".into() };
        assert!(crashed.minor(true) && !server.minor(true));
        let problems = [server.clone(), crashed.clone(), Problem::NeedsRosetta];
        assert_eq!(ranked(&problems[..2], true), [server.clone(), crashed.clone()]);
        assert_eq!(ranked(&problems[..2], false), [crashed.clone(), server.clone()]);
        assert_eq!(ranked(&[Problem::NeedsRosetta, server.clone()], true), [server.clone(), Problem::NeedsRosetta]);
        assert!(crashed.detail(Some("13.6.4"), true).contains("the results stand"));
        // Without Rosetta, the crash is Rosetta's: one problem, fixed by installing it.
        let folded = Problem::CrashedNoRosetta { signal: "SIGSEGV".into() };
        assert_eq!(ranked(&problems, true), [server.clone(), folded.clone()]);
        assert_eq!(folded.fixes(Browser::Electron, &[Browser::Electron]), [Fix::InstallRosetta]);
        assert!(folded.detail(None, true).contains("Rosetta"));
    }
}
