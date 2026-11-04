//! Unit tests of Node packages, run with Vitest or Jest into the Tests window. A package
//! tests with the runner its `package.json` depends on (or whose configuration file it
//! has); SnoutCode runs that runner from the package's own `node_modules`, with a reporter
//! of ours beside the runner's own, which tells the run in the protocol of the Playwright
//! reporter ([`crate::playwright::report`]). Test files get run icons at each `test`, `it`
//! and `describe`; a test runs by its file and its full name (`-t`).

use std::path::{Path, PathBuf};

use serde_json::Value;
use snout_host::Host;

use crate::lang::node::Node;
use crate::run::process::ProcessLaunch;
use crate::run::{FileRunnables, Place, Runnable, Target};

const VITEST_REPORTER: &str = include_str!("snout-vitest-reporter.mjs");
const JEST_REPORTER: &str = include_str!("snout-jest-reporter.cjs");

/// A unit test runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Runner {
    Vitest,
    Jest,
}

impl Runner {
    pub fn name(self) -> &'static str {
        match self {
            Runner::Vitest => "Vitest",
            Runner::Jest => "Jest",
        }
    }

    /// The package it comes in, and the program in it.
    fn package(self) -> &'static str {
        match self {
            Runner::Vitest => "vitest",
            Runner::Jest => "jest",
        }
    }

    /// The configuration files that say a package tests with it.
    fn configs(self) -> &'static [&'static str] {
        match self {
            Runner::Vitest => &["vitest.config.ts", "vitest.config.mts", "vitest.config.js", "vitest.config.mjs"],
            Runner::Jest => {
                &["jest.config.ts", "jest.config.js", "jest.config.mjs", "jest.config.cjs", "jest.config.json"]
            }
        }
    }
}

/// The runner the package at `dir` tests with: the one it depends on, else the one whose
/// configuration it has.
pub fn runner_of(host: &dyn Host, dir: &Path) -> Option<Runner> {
    let json: Value = serde_json::from_str(&host.read_to_string(&dir.join("package.json")).ok()?).ok()?;
    let depends = |name: &str| {
        ["devDependencies", "dependencies", "optionalDependencies"]
            .iter()
            .any(|k| json.get(k).and_then(|d| d.get(name)).is_some())
    };
    [Runner::Vitest, Runner::Jest].into_iter().find(|r| depends(r.package())).or_else(|| {
        [Runner::Vitest, Runner::Jest].into_iter().find(|r| r.configs().iter().any(|c| host.is_file(&dir.join(c))))
    })
}

/// The runner's program, installed for the package at `dir` (in its `node_modules` or a
/// workspace's above it).
pub fn cli(host: &dyn Host, dir: &Path, runner: Runner) -> Option<PathBuf> {
    let pkg = crate::playwright::setup::package_dir(host, dir, runner.package())?;
    let manifest: Value = serde_json::from_str(&host.read_to_string(&pkg.join("package.json")).ok()?).ok()?;
    let bin = match manifest.get("bin")? {
        Value::String(s) => s.clone(),
        Value::Object(m) => m.get(runner.package())?.as_str()?.to_owned(),
        _ => return None,
    };
    Some(pkg.join(bin.trim_start_matches("./"))).filter(|p| host.is_file(p))
}

/// A package that has unit tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suite {
    pub dir: PathBuf,
    pub runner: Runner,
}

impl Suite {
    /// What the Tests window calls it: `Vitest`, `Vitest in apps/web`.
    pub fn label(&self, root: &Path) -> String {
        let rel = self.dir.strip_prefix(root).unwrap_or(&self.dir).to_string_lossy().replace('\\', "/");
        if rel.is_empty() { self.runner.name().to_owned() } else { format!("{} in {rel}", self.runner.name()) }
    }
}

/// The packages of the project at `root` that test with Vitest or Jest.
pub fn suites(host: &dyn Host, root: &Path) -> Vec<Suite> {
    crate::run::node_run::packages(host, root)
        .into_iter()
        .filter_map(|dir| runner_of(host, &dir).map(|runner| Suite { dir, runner }))
        .collect()
}

/// The suite `file` belongs to: its package, when that tests with a runner.
pub fn suite_of(host: &dyn Host, file: &Path, root: &Path) -> Option<Suite> {
    let dir = crate::run::node_run::package_of(host, file, root)?;
    runner_of(host, &dir).map(|runner| Suite { dir, runner })
}

/// Whether `path` is a test file as the runners' defaults have them: `*.test.ts`,
/// `*.spec.js`, or a script in a `__tests__` folder.
pub fn is_test_file(path: &Path) -> bool {
    let script = matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "mjs" | "cts" | "cjs")
    );
    crate::playwright::is_test_file(path) || script && path.components().any(|c| c.as_os_str() == "__tests__")
}

/// What a run runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every test of the package.
    All,
    File(PathBuf),
    /// A test (by its `describe`s and title), or a `describe` and all in it.
    At {
        file: PathBuf,
        path: Vec<String>,
        group: bool,
    },
    /// These tests: each by its file and its path.
    Tests(Vec<(PathBuf, Vec<String>)>),
}

/// `text` as a regular expression that matches it as it is.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\^$.|?*+()[]{}/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The `-t` pattern for tests by their path: the full name (`describe` titles and the
/// test's, a space between) at the end of what the runner matches; a group takes all its
/// tests.
pub fn name_pattern(paths: &[(Vec<String>, bool)]) -> String {
    let one = |(path, group): &(Vec<String>, bool)| {
        let name = escape(&path.join(" "));
        if *group { format!("{name}( |$)") } else { format!("{name}$") }
    };
    match paths {
        [single] => one(single),
        many => format!("({})", many.iter().map(one).collect::<Vec<_>>().join("|")),
    }
}

/// Our reporter for `runner`, written on `host` beside the Playwright one if it is not
/// there yet; named by its content.
pub fn reporter(host: &dyn Host, runner: Runner) -> std::io::Result<PathBuf> {
    let (content, ext) = match runner {
        Runner::Vitest => (VITEST_REPORTER, "mjs"),
        Runner::Jest => (JEST_REPORTER, "cjs"),
    };
    let hash = content.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    let name = format!("snout-{}-reporter-{hash:016x}.{ext}", runner.package());
    let path = crate::playwright::run::reporter_path(host.info()).with_file_name(name);
    if !host.is_file(&path) {
        if let Some(dir) = path.parent() {
            host.create_dir_all(dir)?;
        }
        host.write(&path, content.as_bytes())?;
    }
    Ok(path)
}

/// A test file as the runner's file filter takes it: relative to the package, `/` between.
fn file_arg(dir: &Path, file: &Path) -> String {
    file.strip_prefix(dir).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// The command that runs `scope` of the package at `dir` with `runner`, on `node`; `debug`
/// runs it in one process, with no timeout, under js-debug.
pub fn test_launch(
    host: &dyn Host,
    dir: &Path,
    root: &Path,
    node: &Node,
    runner: Runner,
    scope: &Scope,
    debug: bool,
) -> Result<ProcessLaunch, String> {
    let cli = cli(host, dir, runner)
        .ok_or_else(|| format!("{} is not installed: install the packages first.", runner.name()))?;
    let reporter = reporter(host, runner).map_err(|e| format!("The SnoutCode reporter could not be written: {e}"))?;
    let mut args = vec![cli.display().to_string()];
    let (files, names): (Vec<String>, Vec<(Vec<String>, bool)>) = match scope {
        Scope::All => (Vec::new(), Vec::new()),
        Scope::File(f) => (vec![file_arg(dir, f)], Vec::new()),
        Scope::At { file, path, group } => (vec![file_arg(dir, file)], vec![(path.clone(), *group)]),
        Scope::Tests(tests) => {
            let mut files: Vec<String> = tests.iter().map(|(f, _)| file_arg(dir, f)).collect();
            files.sort();
            files.dedup();
            (files, tests.iter().map(|(_, p)| (p.clone(), false)).collect())
        }
    };
    match runner {
        Runner::Vitest => {
            args.extend([
                "run".to_owned(),
                "--reporter=default".to_owned(),
                format!("--reporter={}", reporter.display()),
            ]);
            if debug {
                args.extend(["--no-file-parallelism", "--pool=forks", "--test-timeout=0"].map(str::to_owned));
            }
            args.extend(files);
        }
        Runner::Jest => {
            args.extend([
                "--reporters=default".to_owned(),
                format!("--reporters={}", reporter.display()),
                "--testLocationInResults".to_owned(),
            ]);
            if debug {
                args.extend(["--runInBand", "--testTimeout=86400000"].map(str::to_owned));
            }
            // Jest's file filter is a pattern.
            args.extend(files.iter().map(|f| escape(f)));
        }
    }
    if !names.is_empty() {
        args.extend(["-t".to_owned(), name_pattern(&names)]);
    }
    let vars = vec![
        ("PATH".to_owned(), crate::lang::typescript::path_with(host, node)),
        ("FORCE_COLOR".to_owned(), "1".to_owned()),
    ];
    let what = match scope {
        Scope::All => {
            let rel = file_arg(root, dir);
            if rel.is_empty() { runner.name().to_owned() } else { format!("{} in {rel}", runner.name()) }
        }
        Scope::File(f) => f.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        Scope::At { path, .. } => path.last().cloned().unwrap_or_default(),
        Scope::Tests(t) => format!("{} tests", t.len()),
    };
    Ok(ProcessLaunch {
        title: what,
        program: node.bin.clone(),
        args,
        cwd: dir.to_owned(),
        env: vars,
        place: if debug { Place::Debug } else { Place::Run },
        report: true,
        debug_node: debug.then(|| node.bin.clone()),
        attach: None,
    })
}

/// The run icons of a test file of `suite`: one at each `test`, `it` and `describe`, each
/// knowing its `describe`s.
pub fn runnables(path: &Path, text: &str, suite: &Suite) -> FileRunnables {
    let items = crate::playwright::gutter::tests_with_paths(path, text)
        .into_iter()
        .map(|(line, path_titles, group)| Runnable {
            line,
            target: Target::JsTest {
                file: path.to_owned(),
                path: path_titles,
                group,
                dir: suite.dir.clone(),
                runner: suite.runner,
            },
        })
        .collect();
    FileRunnables { module: None, items, whole: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_patterns_that_match_them_whole() {
        let p = |t: &[&str], g: bool| (t.iter().map(|s| s.to_string()).collect::<Vec<_>>(), g);
        assert_eq!(name_pattern(&[p(&["cart", "adds (two) items"], false)]), r"cart adds \(two\) items$");
        assert_eq!(name_pattern(&[p(&["cart"], true)]), "cart( |$)");
        assert_eq!(name_pattern(&[p(&["a"], false), p(&["b.c"], false)]), r"(a$|b\.c$)");
    }

    #[test]
    fn test_files_are_named_so_or_sit_in_tests_folders() {
        assert!(is_test_file(Path::new("/p/src/cart.test.ts")));
        assert!(is_test_file(Path::new("/p/src/__tests__/cart.ts")));
        assert!(!is_test_file(Path::new("/p/src/cart.ts")));
        assert!(!is_test_file(Path::new("/p/src/__tests__/data.json")));
    }
}
