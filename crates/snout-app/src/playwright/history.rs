//! The project's past Playwright runs, kept with its settings outside the repository: when
//! each ran, against which environment, on which branch and commit, its counts and time,
//! and how each test came out in each project. The grid's last results, the flaky rate,
//! the ETA and "New since last run" come from them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::report::{Counts, Run, State};

/// How many runs are kept.
pub const KEEP: usize = 200;

/// One test's result in one project, kept as `p1234`: its state's code and its time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub state: State,
    pub millis: u64,
}

impl Serialize for Cell {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}{}", self.state.code(), self.millis))
    }
}

impl<'de> Deserialize<'de> for Cell {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let mut chars = s.chars();
        let state = chars.next().and_then(State::from_code).ok_or_else(|| serde::de::Error::custom("a state"))?;
        Ok(Cell { state, millis: chars.as_str().parse().unwrap_or(0) })
    }
}

/// A finished run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// When it started, in seconds since 1970.
    pub when: u64,
    /// What it ran (`All tests`, `checkout.spec.ts:12`).
    pub scope: String,
    pub environment: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub passed: usize,
    pub failed: usize,
    pub flaky: usize,
    pub skipped: usize,
    pub millis: u64,
    /// By [`cell_key`].
    pub results: BTreeMap<String, Cell>,
}

/// A test in a project, as the history files it: `chromium › login.spec.ts › signs in`.
pub fn cell_key(project: &str, key: &str) -> String {
    format!("{project} › {key}")
}

impl Record {
    pub fn of(
        run: &Run,
        when: u64,
        scope: String,
        environment: Option<String>,
        git: (Option<String>, Option<String>),
    ) -> Self {
        let Counts { passed, failed, flaky, skipped, .. } = run.counts();
        let mut results = BTreeMap::new();
        for t in run.tests.values() {
            let state = t.state(true);
            // A test the run never got to says nothing of it.
            if t.tries.is_empty() && state == State::Skipped && t.expected != "skipped" {
                continue;
            }
            results.insert(cell_key(&t.project, &t.key()), Cell { state, millis: t.millis().unwrap_or(0) });
        }
        Record {
            when,
            scope,
            environment,
            branch: git.0,
            commit: git.1,
            passed,
            failed,
            flaky,
            skipped,
            millis: run.millis.unwrap_or(0),
            results,
        }
    }

    /// The tests that failed in it but not in `before` (passed there, or were not run).
    pub fn new_failures(&self, before: Option<&Record>) -> Vec<String> {
        self.results
            .iter()
            .filter(|(_, c)| c.state == State::Failed)
            .filter(|(k, _)| before.and_then(|b| b.results.get(*k)).is_none_or(|c| c.state != State::Failed))
            .map(|(k, _)| k.clone())
            .collect()
    }
}

/// The project's runs, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    pub runs: Vec<Record>,
    file: Option<PathBuf>,
}

impl History {
    /// The file the runs of the project at `root` are kept in (none in tests).
    pub fn file(support: Option<&Path>, root: &Path) -> Option<PathBuf> {
        support.map(|s| crate::run::config::project_file(s, root).with_file_name("playwright-runs.json"))
    }

    pub fn load(file: Option<PathBuf>) -> Self {
        let runs = file
            .as_deref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        History { runs, file }
    }

    /// Keep `record`, and write the file.
    pub fn add(&mut self, record: Record) {
        self.runs.push(record);
        if self.runs.len() > KEEP {
            self.runs.drain(..self.runs.len() - KEEP);
        }
        if let Some(f) = &self.file {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match serde_json::to_string(&self.runs) {
                Ok(text) => {
                    if let Err(e) = std::fs::write(f, text) {
                        eprintln!("[playwright] could not keep the run in {}: {e}", f.display());
                    }
                }
                Err(e) => eprintln!("[playwright] {e}"),
            }
        }
    }

    /// The last `n` results of a test in a project, oldest first.
    pub fn last(&self, key: &str, n: usize) -> Vec<State> {
        let mut out: Vec<State> =
            self.runs.iter().rev().filter_map(|r| r.results.get(key)).take(n).map(|c| c.state).collect();
        out.reverse();
        out
    }

    /// How often a test was flaky (or failed, then passed) in its last 20 runs, in percent;
    /// none when it never was.
    pub fn flaky_rate(&self, key: &str) -> Option<u32> {
        let last = self.last(key, 20);
        let ran: Vec<&State> =
            last.iter().filter(|s| matches!(s, State::Passed | State::Failed | State::Flaky)).collect();
        if ran.is_empty() {
            return None;
        }
        let mut flips = ran.iter().filter(|s| ***s == State::Flaky).count();
        // A failure next to a pass counts too: it passes or not without a change.
        flips += ran.windows(2).filter(|w| (*w[0] == State::Failed) != (*w[1] == State::Failed)).count() / 2;
        let rate = (flips * 100 / ran.len()) as u32;
        (rate > 0).then_some(rate)
    }

    /// The time a test took last, in a project.
    pub fn millis(&self, key: &str) -> Option<u64> {
        self.runs.iter().rev().filter_map(|r| r.results.get(key)).find(|c| c.millis > 0).map(|c| c.millis)
    }

    /// The run before the newest.
    pub fn previous(&self) -> Option<&Record> {
        self.runs.len().checked_sub(2).and_then(|i| self.runs.get(i))
    }

    /// About how long what is left of `run` takes, in milliseconds, by the tests' past
    /// times and its workers.
    pub fn eta(&self, run: &Run) -> Option<u64> {
        let mut left = 0u64;
        let mut known = false;
        for t in run.tests.values() {
            if matches!(t.state(run.over), State::Queued | State::Running) {
                let past = self.millis(&cell_key(&t.project, &t.key()))?;
                let spent = if t.tries.last().is_some_and(|tr| tr.status.is_none()) { past / 2 } else { 0 };
                left += past - spent;
                known = true;
            }
        }
        known.then(|| left / u64::from(run.workers.max(1)))
    }
}
