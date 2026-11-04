//! A Playwright run as the SnoutCode reporter (`snout-reporter.cjs`) tells it: the tests it
//! found, each in a project (a browser), and as they run, the worker on each, the step it
//! is at, each try's result with its errors, steps and attachments (the trace, screenshots,
//! the video). The reporter's lines come on the run's stdout after [`MARK`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

/// What starts each of the reporter's lines.
pub const MARK: &str = "\u{1e}snout:";

/// Split `line` into what it shows in the console and the reporter's event in it, if any.
pub fn split_line(line: &str) -> (Option<&str>, Option<&str>) {
    match line.find(MARK) {
        Some(at) => {
            let before = &line[..at];
            (Some(before).filter(|b| !b.trim().is_empty()), Some(&line[at + MARK.len()..]))
        }
        None => (Some(line), None),
    }
}

/// How a test came out (or where it is), for one project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum State {
    Queued,
    Running,
    Passed,
    /// Passed on a retry.
    Flaky,
    Failed,
    Skipped,
    /// The run ended (it was stopped) before the test did.
    Interrupted,
}

impl State {
    /// The state as the history keeps it: `p`, `f`, `k`, `s`.
    pub fn code(self) -> char {
        match self {
            State::Passed => 'p',
            State::Failed => 'f',
            State::Flaky => 'k',
            State::Skipped => 's',
            State::Interrupted => 'i',
            State::Running => 'r',
            State::Queued => 'q',
        }
    }

    pub fn from_code(c: char) -> Option<State> {
        Some(match c {
            'p' => State::Passed,
            'f' => State::Failed,
            'k' => State::Flaky,
            's' => State::Skipped,
            'i' => State::Interrupted,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            State::Queued => "Queued",
            State::Running => "Running",
            State::Passed => "Passed",
            State::Flaky => "Flaky, passed on a retry",
            State::Failed => "Failed",
            State::Skipped => "Skipped",
            State::Interrupted => "Interrupted",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Location {
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// An error a try ended with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct TestError {
    pub message: String,
    pub stack: String,
    /// The lines around the failing one, as Playwright prints them (`> 12 |  await …`).
    pub snippet: String,
    #[serde(flatten)]
    pub location: Location,
}

impl TestError {
    /// Its message without the terminal's colours.
    pub fn text(&self) -> String {
        strip_ansi(&self.message)
    }

    /// The message's first line.
    pub fn headline(&self) -> String {
        self.text().lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim().to_owned()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Attachment {
    pub name: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Step {
    pub title: String,
    pub category: String,
    pub duration: f64,
    /// The first line of its error.
    pub error: Option<String>,
    pub line: Option<u32>,
    pub depth: u32,
}

/// One try of a test.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Try {
    pub retry: u32,
    pub worker: Option<u32>,
    /// `passed`, `failed`, `timedOut`, `skipped`, `interrupted`; none while it runs.
    pub status: Option<String>,
    pub millis: Option<u64>,
    pub errors: Vec<TestError>,
    pub attachments: Vec<Attachment>,
    pub steps: Vec<Step>,
    pub output: String,
}

impl Try {
    pub fn attachment(&self, name: &str) -> Option<&Attachment> {
        self.attachments.iter().find(|a| a.name == name)
    }

    /// The trace it recorded.
    pub fn trace(&self) -> Option<&Path> {
        self.attachment("trace").map(|a| a.path.as_path())
    }

    pub fn video(&self) -> Option<&Path> {
        self.attachments.iter().find(|a| a.content_type.starts_with("video/")).map(|a| a.path.as_path())
    }

    /// The screenshot comparisons that failed: their name, and the expected, actual and diff
    /// images (`home-expected.png`, `home-actual.png`, `home-diff.png`).
    pub fn comparisons(&self) -> Vec<Comparison> {
        let mut out = Vec::new();
        for a in &self.attachments {
            if let Some(stem) = a.name.strip_suffix("-actual.png") {
                let find = |suffix: &str| self.attachment(&format!("{stem}{suffix}")).map(|a| a.path.clone());
                out.push(Comparison {
                    name: format!("{stem}.png"),
                    expected: find("-expected.png"),
                    actual: a.path.clone(),
                    diff: find("-diff.png"),
                });
            }
        }
        out
    }

    /// The screenshots taken when it failed (`screenshot: 'only-on-failure'`).
    pub fn screenshots(&self) -> Vec<&Path> {
        self.attachments
            .iter()
            .filter(|a| a.name == "screenshot" && a.content_type.starts_with("image/"))
            .map(|a| a.path.as_path())
            .collect()
    }
}

/// A screenshot that did not match the one kept with the tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub name: String,
    pub expected: Option<PathBuf>,
    pub actual: PathBuf,
    pub diff: Option<PathBuf>,
}

/// A test in one project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Test {
    pub id: String,
    pub project: String,
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    /// Its `describe`s, then its title.
    pub path: Vec<String>,
    pub tags: Vec<String>,
    /// What it expects of itself (`passed`, `skipped` for `test.skip`, `failed` for `test.fail`).
    pub expected: String,
    pub tries: Vec<Try>,
    /// Playwright's verdict: `expected`, `unexpected`, `flaky`, `skipped`.
    pub outcome: Option<String>,
}

impl Test {
    pub fn title(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }

    /// Where it is, without the project: `checkout.spec.ts › payment › pays by card`.
    pub fn key(&self) -> String {
        key_of(&self.file, &self.path)
    }

    pub fn last_try(&self) -> Option<&Try> {
        self.tries.last()
    }

    pub fn state(&self, run_over: bool) -> State {
        let running = self.tries.last().is_some_and(|t| t.status.is_none());
        match self.outcome.as_deref() {
            _ if running && !run_over => State::Running,
            _ if running => State::Interrupted,
            Some("expected") if self.tries.last().and_then(|t| t.status.as_deref()) == Some("skipped") => {
                State::Skipped
            }
            Some("expected") => State::Passed,
            Some("flaky") => State::Flaky,
            Some("skipped") => State::Skipped,
            Some(_) if self.tries.last().and_then(|t| t.status.as_deref()) == Some("interrupted") => State::Interrupted,
            Some(_) => State::Failed,
            None if run_over => State::Skipped,
            None => State::Queued,
        }
    }

    /// How long it took, all its tries together.
    pub fn millis(&self) -> Option<u64> {
        let done: Vec<u64> = self.tries.iter().filter_map(|t| t.millis).collect();
        (!done.is_empty()).then(|| done.iter().sum())
    }
}

/// A test's key, from its file (relative to the test folder) and its path.
pub fn key_of(file: &Path, path: &[String]) -> String {
    let mut parts = vec![file.to_string_lossy().replace('\\', "/")];
    parts.extend(path.iter().cloned());
    parts.join(" › ")
}

/// A project of the configuration (`chromium`, `firefox`, `setup`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Project {
    pub name: String,
    /// The browser it runs, when it says (`chromium`, `firefox`, `webkit`).
    pub browser: String,
    /// The branded browser it runs (`chrome`, `msedge`), when it says.
    pub channel: String,
    /// Its `use.baseURL`, as the configuration made it for this run's variables.
    #[serde(rename = "baseURL")]
    pub base_url: String,
    /// Its `use.testIdAttribute`, when not `data-testid`.
    #[serde(rename = "testIdAttribute")]
    pub test_id_attribute: String,
    /// Its `use.storageState`, when a file (the session a setup project saved).
    #[serde(rename = "storageState")]
    pub storage_state: String,
    pub viewport: Option<(u32, u32)>,
}

/// What a worker does now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worker {
    pub test: String,
    pub step: Option<String>,
    /// When the test started, in milliseconds since 1970.
    pub since: u64,
}

/// A row of the results grid: one test, each project's run of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub file: String,
    pub path: Vec<String>,
    /// By project: the test's id.
    pub cells: BTreeMap<String, String>,
}

/// A run, as far as it has got.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub version: Option<String>,
    /// The folder test files are relative to.
    pub root: Option<PathBuf>,
    pub workers: u32,
    pub projects: Vec<Project>,
    pub tests: BTreeMap<String, Test>,
    /// The tests' ids, in the order Playwright found them.
    pub order: Vec<String>,
    /// By parallel index.
    pub busy: BTreeMap<u32, Worker>,
    /// Errors outside any test (the configuration did not load, a file did not compile).
    pub errors: Vec<TestError>,
    /// How it ended (`passed`, `failed`, `timedout`, `interrupted`).
    pub status: Option<String>,
    pub millis: Option<u64>,
    /// It began (the reporter said what it found).
    pub begun: bool,
    /// The process is gone, whether or not the reporter said so.
    pub over: bool,
    /// The environment it ran against.
    pub environment: Option<String>,
    /// The test runner, when not Playwright, and the package it ran in.
    pub runner: Option<crate::jstest::Runner>,
    pub dir: Option<PathBuf>,
}

/// What a run counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub total: usize,
    pub done: usize,
    pub passed: usize,
    pub failed: usize,
    pub flaky: usize,
    pub skipped: usize,
    pub running: usize,
}

impl Run {
    /// Take one of the reporter's events (the JSON after [`MARK`]).
    pub fn apply_line(&mut self, json: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(json) {
            self.apply(&v);
        }
    }

    pub fn apply(&mut self, ev: &Value) {
        let s = |k: &str| ev.get(k).and_then(Value::as_str).map(str::to_owned);
        let n = |k: &str| ev.get(k).and_then(Value::as_u64);
        let id = s("id").unwrap_or_default();
        let retry = n("retry").unwrap_or(0) as u32;
        match s("e").as_deref() {
            Some("begin") => {
                self.begun = true;
                self.version = s("version");
                self.root = s("rootDir").map(PathBuf::from);
                self.workers = n("workers").unwrap_or(1) as u32;
                self.projects = serde_json::from_value(ev["projects"].clone()).unwrap_or_default();
                self.add_tests(ev);
            }
            // Tests found as the run goes (Vitest and Jest find a file's as they load it).
            Some("tests") => self.add_tests(ev),
            Some("testBegin") => {
                let worker = n("worker").map(|w| w as u32);
                if let Some(t) = self.tests.get_mut(&id) {
                    t.tries.push(Try { retry, worker, ..Default::default() });
                }
                if let Some(w) = worker {
                    self.busy.insert(w, Worker { test: id, step: None, since: n("start").unwrap_or(0) });
                }
            }
            Some("step") => {
                if let Some(w) = n("worker").and_then(|w| self.busy.get_mut(&(w as u32)))
                    && w.test == id
                {
                    w.step = s("title");
                }
            }
            Some("out") => {
                if let Some(tr) = self.try_mut(&id, retry) {
                    tr.output.push_str(&s("text").unwrap_or_default());
                }
            }
            Some("testEnd") => {
                let worker = n("worker").map(|w| w as u32);
                if let Some(t) = self.tests.get_mut(&id) {
                    t.outcome = s("outcome");
                    if !t.tries.iter().any(|tr| tr.retry == retry) {
                        t.tries.push(Try { retry, worker, ..Default::default() });
                    }
                }
                if let Some(tr) = self.try_mut(&id, retry) {
                    tr.status = s("status");
                    tr.millis = ev.get("duration").and_then(Value::as_f64).map(|d| d.max(0.) as u64);
                    tr.errors = serde_json::from_value(ev["errors"].clone()).unwrap_or_default();
                    tr.attachments = serde_json::from_value(ev["attachments"].clone()).unwrap_or_default();
                    tr.steps = serde_json::from_value(ev["steps"].clone()).unwrap_or_default();
                }
                if let Some(w) = worker
                    && self.busy.get(&w).is_some_and(|b| b.test == id)
                {
                    self.busy.remove(&w);
                }
            }
            Some("error") => {
                if let Ok(e) = serde_json::from_value(ev["error"].clone()) {
                    self.errors.push(e);
                }
            }
            Some("end") => {
                self.status = s("status");
                self.millis = ev.get("duration").and_then(Value::as_f64).map(|d| d as u64);
                self.over = true;
                self.busy.clear();
            }
            _ => {}
        }
    }

    /// The tests an event lists, in its `tests`.
    fn add_tests(&mut self, ev: &Value) {
        for t in ev.get("tests").and_then(Value::as_array).into_iter().flatten() {
            let ts = |k: &str| t.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            let strings = |k: &str| -> Vec<String> {
                t.get(k)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            };
            let file = PathBuf::from(ts("file"));
            let file =
                self.root.as_ref().and_then(|r| file.strip_prefix(r).ok()).map_or(file.clone(), Path::to_path_buf);
            let test = Test {
                id: ts("id"),
                project: ts("project"),
                file,
                line: t.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
                column: t.get("column").and_then(Value::as_u64).unwrap_or(0) as u32,
                path: strings("path"),
                tags: strings("tags"),
                expected: ts("expected"),
                tries: Vec::new(),
                outcome: None,
            };
            if !self.tests.contains_key(&test.id) {
                self.order.push(test.id.clone());
            }
            // Found again (a retry, a second collection): keep what it did so far.
            match self.tests.get_mut(&test.id) {
                Some(old) => {
                    old.line = test.line;
                    old.column = test.column;
                    old.path = test.path;
                }
                None => {
                    self.tests.insert(test.id.clone(), test);
                }
            }
        }
    }

    fn try_mut(&mut self, id: &str, retry: u32) -> Option<&mut Try> {
        self.tests.get_mut(id)?.tries.iter_mut().rev().find(|t| t.retry == retry)
    }

    /// The process ended: what still runs did not finish.
    pub fn finish(&mut self) {
        self.over = true;
        self.busy.clear();
    }

    pub fn state_of(&self, id: &str) -> State {
        self.tests.get(id).map_or(State::Queued, |t| t.state(self.over))
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts { total: self.tests.len(), ..Default::default() };
        for t in self.tests.values() {
            match t.state(self.over) {
                State::Passed => c.passed += 1,
                State::Failed | State::Interrupted => c.failed += 1,
                State::Flaky => c.flaky += 1,
                State::Skipped => c.skipped += 1,
                State::Running => c.running += 1,
                State::Queued => {}
            }
        }
        c.done = c.passed + c.failed + c.flaky + c.skipped;
        c
    }

    /// The grid's rows, in the order Playwright found the tests: one per test, a cell per
    /// project it runs in.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        let mut at: BTreeMap<String, usize> = BTreeMap::new();
        for id in &self.order {
            let Some(t) = self.tests.get(id) else { continue };
            let key = t.key();
            let ix = *at.entry(key.clone()).or_insert_with(|| {
                rows.push(Row {
                    key: key.clone(),
                    file: t.file.to_string_lossy().replace('\\', "/"),
                    path: t.path.clone(),
                    cells: BTreeMap::new(),
                });
                rows.len() - 1
            });
            rows[ix].cells.insert(t.project.clone(), id.clone());
        }
        rows
    }

    /// The projects in the order the configuration lists them, those with tests only.
    pub fn shown_projects(&self) -> Vec<String> {
        let used: std::collections::BTreeSet<&str> = self.tests.values().map(|t| t.project.as_str()).collect();
        let mut out: Vec<String> =
            self.projects.iter().filter(|p| used.contains(p.name.as_str())).map(|p| p.name.clone()).collect();
        for p in used {
            if !out.iter().any(|o| o == p) {
                out.push(p.to_owned());
            }
        }
        out
    }

    /// The tests that failed (not flaky ones), in order.
    pub fn failures(&self) -> Vec<&Test> {
        self.order
            .iter()
            .filter_map(|id| self.tests.get(id))
            .filter(|t| matches!(t.state(self.over), State::Failed | State::Interrupted))
            .collect()
    }

    /// The test file and line of `id`, as an absolute path.
    pub fn location(&self, id: &str) -> Option<(PathBuf, u32)> {
        let t = self.tests.get(id)?;
        let file = match &self.root {
            Some(r) if t.file.is_relative() => r.join(&t.file),
            _ => t.file.clone(),
        };
        Some((file, t.line))
    }
}

/// `text` without the terminal's colour codes.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// What a run took or has taken: `4.2 s`, `2:05`.
pub fn duration_text(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.)
    } else {
        let s = ms / 1000;
        if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
        } else {
            format!("{}:{:02}", s / 60, s % 60)
        }
    }
}
