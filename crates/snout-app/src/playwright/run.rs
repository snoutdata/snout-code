//! Starting Playwright: the project's own test runner (`node_modules/@playwright/test/cli.js`)
//! on the Node the project asks for, with the SnoutCode reporter beside Playwright's `list`
//! one, so the console reads as on the command line and the Tests window follows each test.
//! What runs is a [`Scope`]; the environment picked adds its variables.

use std::path::{Path, PathBuf};

use snout_host::{Cmd, Host};

use super::report::Run;
use crate::lang::node::Node;
use crate::run::Place;
use crate::run::process::ProcessLaunch;

/// The reporter, as the app carries it.
const REPORTER: &str = include_str!("snout-reporter.cjs");

/// What a run runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    All,
    /// One project of the configuration (a browser).
    Project(String),
    /// A file, or a test or `describe` at a line of it (from 1), in every project or one.
    At {
        file: PathBuf,
        line: Option<u32>,
        project: Option<String>,
    },
    /// The tests that failed last time (Playwright 1.44 or later).
    LastFailed,
}

impl Scope {
    /// Playwright's arguments for it, `root` the project's folder.
    pub fn args(&self, root: &Path) -> Vec<String> {
        match self {
            Scope::All => Vec::new(),
            Scope::Project(p) => vec![format!("--project={p}")],
            Scope::At { file, line, project } => {
                let rel = file.strip_prefix(root).unwrap_or(file);
                let mut at = rel.to_string_lossy().replace('\\', "/");
                if let Some(l) = line {
                    at = format!("{at}:{l}");
                }
                let mut args = vec![at];
                args.extend(project.iter().map(|p| format!("--project={p}")));
                args
            }
            Scope::LastFailed => vec!["--last-failed".into()],
        }
    }

    /// The session's title.
    pub fn title(&self) -> String {
        match self {
            Scope::All => "All tests".into(),
            Scope::Project(p) => format!("Tests in {p}"),
            Scope::At { file, line, project } => {
                let name = file.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let at = line.map_or(name.clone(), |l| format!("{name}:{l}"));
                project.as_ref().map_or(at.clone(), |p| format!("{at} [{p}]"))
            }
            Scope::LastFailed => "Failed tests".into(),
        }
    }
}

/// How a run shows the browsers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Watch: the browser windows show, one test at a time.
    pub headed: bool,
    /// Step through it in the Playwright Inspector (`--debug`).
    pub inspector: bool,
    /// Stop at the breakpoints in the test code: js-debug follows the runner and its worker;
    /// one worker, headed, with no time limit while it is paused.
    pub breakpoints: bool,
    /// Take the screenshots the tests compare with as they are now.
    pub update_snapshots: bool,
    /// Run each test this many times (for a flaky one).
    pub repeat: Option<u32>,
}

impl Options {
    pub fn args(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.headed || self.breakpoints {
            out.extend(["--headed".to_owned(), "--workers=1".to_owned()]);
        }
        if self.breakpoints {
            out.push("--timeout=0".into());
        }
        if self.inspector {
            out.push("--debug".into());
        }
        if self.update_snapshots {
            out.push("--update-snapshots".into());
        }
        if let Some(n) = self.repeat {
            out.push(format!("--repeat-each={n}"));
        }
        out
    }
}

/// The project's test runner program (`cli.js` of `@playwright/test`).
pub fn cli(host: &dyn Host, root: &Path) -> Option<PathBuf> {
    super::setup::package_dir(host, root, super::TEST_PACKAGE).map(|d| d.join("cli.js")).filter(|c| host.is_file(c))
}

/// Where the reporter is written for `host`: beside the Node SnoutCode downloads.
pub fn reporter_path(info: &snout_host::HostInfo) -> PathBuf {
    // Named by its content, so a new app's reporter never meets an old file.
    let hash =
        REPORTER.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    crate::lang::node::downloads_dir(info).with_file_name("playwright").join(format!("snout-reporter-{hash:016x}.cjs"))
}

/// The reporter, written on `host` if it is not there yet.
pub fn reporter(host: &dyn Host) -> std::io::Result<PathBuf> {
    let path = reporter_path(host.info());
    if !host.is_file(&path) {
        if let Some(dir) = path.parent() {
            host.create_dir_all(dir)?;
        }
        host.write(&path, REPORTER.as_bytes())?;
    }
    Ok(path)
}

/// Another script of ours (`name`, holding `content`), written beside the reporter if it is
/// not there yet; named by its content too.
pub fn write_tool(host: &dyn Host, name: &str, content: &str) -> std::io::Result<PathBuf> {
    let hash = content.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    let path = reporter_path(host.info()).with_file_name(format!("{name}-{hash:016x}.cjs"));
    if !host.is_file(&path) {
        if let Some(dir) = path.parent() {
            host.create_dir_all(dir)?;
        }
        host.write(&path, content.as_bytes())?;
    }
    Ok(path)
}

/// The variables every Playwright run gets: `node` first on `PATH`, and colours.
pub(super) fn base_env(host: &dyn Host, node: &Node) -> Vec<(String, String)> {
    vec![
        ("PATH".to_owned(), crate::lang::typescript::path_with(host, node)),
        ("FORCE_COLOR".to_owned(), "1".to_owned()),
        // The HTML reporter, when the configuration has one, never opens a browser itself.
        ("PW_TEST_HTML_REPORT_OPEN".to_owned(), "never".to_owned()),
    ]
}

/// The command that runs `scope` of the project at `root` with `node`, `env` on top.
pub fn test_launch(
    host: &dyn Host,
    root: &Path,
    node: &Node,
    scope: &Scope,
    options: Options,
    extra: &[String],
    env: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let cli = cli(host, root).ok_or("The Playwright test runner is not installed: install the packages first.")?;
    let reporter = reporter(host).map_err(|e| format!("The SnoutCode reporter could not be written: {e}"))?;
    let mut args =
        vec![cli.display().to_string(), "test".to_owned(), format!("--reporter={},list", reporter.display())];
    args.extend(scope.args(root));
    args.extend(options.args());
    args.extend(extra.iter().cloned());
    let mut all = base_env(host, node);
    all.extend(env);
    Ok(ProcessLaunch {
        title: scope.title(),
        program: node.bin.clone(),
        args,
        cwd: root.to_owned(),
        env: all,
        place: if options.breakpoints { Place::Debug } else { Place::Run },
        report: true,
        debug_node: options.breakpoints.then(|| node.bin.clone()),
    })
}

/// The project's tests, as `playwright test --list` finds them (nothing runs).
pub fn list(host: &dyn Host, root: &Path, node: &Node, env: &[(String, String)]) -> Result<Run, String> {
    let cli = cli(host, root).ok_or("The Playwright test runner is not installed.")?;
    let reporter = reporter(host).map_err(|e| e.to_string())?;
    let mut cmd = Cmd::new(&node.bin)
        .arg(&cli)
        .args(["test", "--list"])
        .arg(format!("--reporter={}", reporter.display()))
        .cwd(root);
    for (k, v) in base_env(host, node).iter().chain(env) {
        cmd = cmd.env(k, v);
    }
    let out = host.run(&cmd, None).map_err(|e| e.to_string())?;
    let mut run = Run::default();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let (_, Some(json)) = super::report::split_line(line) {
            run.apply_line(json);
        }
    }
    if !run.begun {
        let err = String::from_utf8_lossy(&out.stderr);
        let why = super::report::strip_ansi(err.trim());
        return Err(if why.is_empty() { format!("Playwright listed no tests ({})", out.status) } else { why });
    }
    Ok(run)
}
