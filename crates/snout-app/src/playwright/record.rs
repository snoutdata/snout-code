//! Writing tests: Record at Cursor, Pick Locator and UI Mode.
//!
//! Record at Cursor runs Playwright's own recorder (`playwright codegen`) on the project's
//! Node, against the environment picked, with the configuration's browser, test id attribute
//! and saved session. The recorder rewrites its output file as you act in the browser;
//! SnoutCode reads it as it changes and writes the steps of its test where the caret was:
//! into the test it is in, as a new test between tests, or as the whole spec in an empty
//! file. Pick Locator opens the browser at the same place and puts the locator of each
//! element clicked at the caret (`page.pickLocator()`, from a small script of ours). UI Mode
//! is Playwright's `--ui`, served to the system browser.

use std::path::{Path, PathBuf};

use snout_host::Host;

use super::gutter::Spot;
use super::report::Project;
use super::run::{base_env, cli, write_tool};
use crate::lang::node::Node;
use crate::run::Place;
use crate::run::config::Environment;
use crate::run::process::ProcessLaunch;

/// The locator picker, as the app carries it.
const PICKER: &str = include_str!("snout-picker.cjs");

/// What a recording or a pick starts from: where, in which browser, as whom.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Start {
    pub url: Option<String>,
    /// `chromium`, `firefox` or `webkit`.
    pub browser: Option<String>,
    pub channel: Option<String>,
    pub test_id_attribute: Option<String>,
    /// The session a setup project saved (`use.storageState`), when the file is there.
    pub storage: Option<PathBuf>,
    pub viewport: Option<(u32, u32)>,
}

impl Start {
    /// From the configuration's projects (as listed) and the environment picked: the
    /// project that logs in first (it has a saved session), else the first that names a
    /// browser; the environment's URL before the configuration's.
    pub fn of(projects: &[Project], environment: Option<&Environment>, root: &Path) -> Self {
        let p = projects
            .iter()
            .find(|p| !p.storage_state.is_empty())
            .or_else(|| projects.iter().find(|p| !p.browser.is_empty()))
            .or(projects.first());
        let some = |s: &str| Some(s.trim().to_owned()).filter(|s| !s.is_empty());
        Start {
            url: environment.and_then(|e| some(&e.base_url)).or_else(|| p.and_then(|p| some(&p.base_url))),
            browser: p.and_then(|p| some(&p.browser)),
            channel: p.and_then(|p| some(&p.channel)),
            test_id_attribute: p.and_then(|p| some(&p.test_id_attribute)),
            storage: p.and_then(|p| some(&p.storage_state)).map(|s| root.join(s)),
            viewport: p.and_then(|p| p.viewport),
        }
    }

    /// `codegen`'s arguments to record into `out` (the session only when its file is there).
    pub fn codegen_args(&self, out: &Path, storage: bool) -> Vec<String> {
        let mut args =
            vec!["codegen".to_owned(), "--target=playwright-test".to_owned(), format!("--output={}", out.display())];
        if let Some(b) = &self.browser {
            args.push(format!("--browser={b}"));
        }
        if let Some(c) = &self.channel {
            args.push(format!("--channel={c}"));
        }
        if let Some(t) = &self.test_id_attribute {
            args.push(format!("--test-id-attribute={t}"));
        }
        if let Some(s) = self.storage.as_ref().filter(|_| storage) {
            args.push(format!("--load-storage={}", s.display()));
        }
        if let Some((w, h)) = self.viewport {
            args.push(format!("--viewport-size={w},{h}"));
        }
        args.extend(self.url.clone());
        args
    }

    /// What the picker is told, as JSON.
    pub fn picker_args(&self, storage: bool) -> String {
        serde_json::json!({
            "url": self.url,
            "browser": self.browser.as_deref().unwrap_or("chromium"),
            "channel": self.channel,
            "testIdAttribute": self.test_id_attribute,
            "storage": self.storage.as_ref().filter(|_| storage).map(|s| s.display().to_string()),
            "viewport": self.viewport.map(|(w, h)| serde_json::json!({ "width": w, "height": h })),
        })
        .to_string()
    }
}

/// Where a recording is written for `host`: beside the reporter, named for the moment.
pub fn recording_file(info: &snout_host::HostInfo) -> PathBuf {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    super::run::reporter_path(info).with_file_name(format!("recording-{now}.ts"))
}

/// The variables the tools get: the run's, and the environment's on top.
fn tool_env(host: &dyn Host, node: &Node, vars: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut env = base_env(host, node);
    env.extend(vars);
    env
}

/// The recorder, writing into `out`.
pub fn codegen_launch(
    host: &dyn Host,
    root: &Path,
    node: &Node,
    start: &Start,
    out: &Path,
    vars: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let cli = cli(host, root).ok_or(ts!("tests-the-playwright-test-runner-is-not-installed-install"))?;
    if let Some(dir) = out.parent() {
        host.create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let _ = host.remove_file(out);
    let storage = start.storage.as_ref().is_some_and(|s| host.is_file(s));
    let mut args = vec![cli.display().to_string()];
    args.extend(start.codegen_args(out, storage));
    Ok(ProcessLaunch {
        title: ts!("tests-recording").into(),
        program: node.bin.clone(),
        args,
        cwd: root.to_owned(),
        env: tool_env(host, node, vars),
        place: Place::Build,
        report: false,
        debug_node: None,
        attach: None,
    })
}

/// The locator picker.
pub fn picker_launch(
    host: &dyn Host,
    root: &Path,
    node: &Node,
    start: &Start,
    vars: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    cli(host, root).ok_or(ts!("tests-the-playwright-test-runner-is-not-installed-install"))?;
    let script = write_tool(host, "snout-picker", PICKER)
        .map_err(|e| t!("tests-the-snoutcode-locator-picker-could-not-be-written", e = e.to_string()))?;
    let storage = start.storage.as_ref().is_some_and(|s| host.is_file(s));
    Ok(ProcessLaunch {
        title: ts!("cmd-pick-locator").into(),
        program: node.bin.clone(),
        args: vec![script.display().to_string(), start.picker_args(storage)],
        cwd: root.to_owned(),
        env: tool_env(host, node, vars),
        place: Place::Build,
        report: false,
        debug_node: None,
        attach: None,
    })
}

/// Playwright's UI Mode, served to the system browser (which Playwright opens itself).
pub fn ui_launch(
    host: &dyn Host,
    root: &Path,
    node: &Node,
    vars: Vec<(String, String)>,
) -> Result<ProcessLaunch, String> {
    let cli = cli(host, root).ok_or(ts!("tests-the-playwright-test-runner-is-not-installed-install"))?;
    Ok(ProcessLaunch {
        title: ts!("tests-ui-mode").into(),
        program: node.bin.clone(),
        args: vec![cli.display().to_string(), "test".into(), "--ui".into(), "--ui-port=0".into()],
        cwd: root.to_owned(),
        env: tool_env(host, node, vars),
        place: Place::Build,
        report: false,
        debug_node: None,
        attach: None,
    })
}

/// The steps of the test the recorder wrote, one indentation level out; none while the file
/// is not whole (it is rewritten as the recording goes).
pub fn steps(generated: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = generated.lines().collect();
    let open = lines.iter().position(|l| {
        let l = l.trim();
        (l.starts_with("test(") || l.starts_with("test.only(")) && l.ends_with('{')
    })?;
    let close = lines.iter().rposition(|l| l.trim() == "});").filter(|c| *c > open)?;
    let body = &lines[open + 1..close];
    let pad = body.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    Some(body.iter().map(|l| l.get(pad..).unwrap_or_else(|| l.trim_start()).trim_end().to_owned()).collect())
}

/// `step` with a `page.goto` to `base` (a bare origin) made relative to it, as the
/// configuration's `baseURL` resolves it.
pub fn relative(step: &str, base: Option<&str>) -> String {
    let Some(base) = base.map(|b| b.trim_end_matches('/')) else { return step.to_owned() };
    // Only a bare origin: `/cart` against `https://shop.test/app` drops `/app`.
    let bare = base.split_once("://").is_some_and(|(_, rest)| !rest.contains('/'));
    if base.is_empty() || !bare {
        return step.to_owned();
    }
    let mut out = step.to_owned();
    for q in ['\'', '"', '`'] {
        let pat = format!("goto({q}{base}");
        if let Some(i) = out.find(&pat) {
            let rest = &out[i + pat.len()..];
            let path = if rest.starts_with(q) { format!("/{rest}") } else { rest.to_owned() };
            if path.starts_with('/') {
                out = format!("{}goto({q}{path}", &out[..i]);
            }
        }
    }
    out
}

/// What goes at the caret for `steps`: the steps at the test's indentation, a new test
/// holding them, or (for an empty file) the recorder's whole spec.
pub fn render(spot: &Spot, steps: &[String], generated: &str, unit: &str, base: Option<&str>) -> String {
    let lines = |indent: &str| -> Vec<String> {
        steps
            .iter()
            .map(|s| if s.is_empty() { String::new() } else { format!("{indent}{}", relative(s, base)) })
            .collect()
    };
    match spot {
        Spot::Body { indent } => lines(indent).join("\n"),
        Spot::Outside { indent } => {
            let inner = lines(&format!("{indent}{unit}"));
            format!("{indent}test('new test', async ({{ page }}) => {{\n{}\n{indent}}});", inner.join("\n"))
        }
        Spot::Empty => {
            let all: Vec<String> = generated.lines().map(|l| relative(l, base)).collect();
            format!("{}\n", all.join("\n").trim_end())
        }
    }
}

/// How many steps there are (lines that do something).
pub fn count(steps: &[String]) -> usize {
    steps.iter().filter(|s| s.trim_start().starts_with("await ")).count()
}

/// Where a recording writes in `text` with the caret at `caret`: the range it takes over
/// (a blank line, or the end of the caret's line, or the whole of an empty file) and what
/// goes before the steps there.
pub fn place(text: &str, caret: usize, spot: &Spot) -> (std::ops::Range<usize>, &'static str) {
    if matches!(spot, Spot::Empty) {
        return (0..text.len(), "");
    }
    let caret = caret.min(text.len());
    let start = text[..caret].rfind('\n').map_or(0, |i| i + 1);
    let end = text[caret..].find('\n').map_or(text.len(), |i| caret + i);
    if text[start..end].trim().is_empty() {
        (start..end, "")
    } else {
        let gap = if matches!(spot, Spot::Outside { .. }) { "\n\n" } else { "\n" };
        (end..end, gap)
    }
}

/// Files a recording can write into, and picks go to: scripts.
pub fn scriptable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e, "ts" | "js" | "mts" | "mjs" | "cts" | "cjs" | "tsx" | "jsx"))
}

/// What a pick puts at the caret: the locator on `page`, unless the text before the caret
/// already ends in a member access (`page.`, `dialog.`).
pub fn picked_text(before: &str, locator: &str) -> String {
    if before.ends_with('.') { locator.to_owned() } else { format!("page.{locator}") }
}
