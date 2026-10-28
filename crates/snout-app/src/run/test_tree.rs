//! The results of a test run as a tree, from the engine's `snout/testEvent`s: the classes
//! (suites), then their tests, each passed, failed or skipped, with its time, its failure
//! and what it printed. Gradle's own suites (`Gradle Test Run :app:test`, `Gradle Test
//! Executor 1`) are left out: their children hang from the run itself.

use std::collections::HashMap;

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestState {
    Running,
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestNode {
    pub id: String,
    /// The node it shows under (none for the run's top level).
    pub parent: Option<String>,
    pub name: String,
    pub class_name: Option<String>,
    pub suite: bool,
    pub state: TestState,
    pub millis: Option<u64>,
    pub message: Option<String>,
    pub exception: Option<String>,
    pub stack: Option<String>,
    /// What the test printed, stdout and stderr together.
    pub output: String,
    pub children: Vec<String>,
}

impl TestNode {
    /// An assertion's expected and actual values, when its message says them.
    pub fn diff(&self) -> Option<(String, String)> {
        assertion_diff(self.message.as_deref()?)
    }
}

/// The tree of one run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestTree {
    pub nodes: HashMap<String, TestNode>,
    pub roots: Vec<String>,
    /// Gradle's own suites, left out: their id, and the node their children go under.
    hidden: HashMap<String, Option<String>>,
}

fn hidden_suite(name: &str) -> bool {
    name.starts_with("Gradle Test Run") || name.starts_with("Gradle Test Executor") || name.starts_with("Gradle suite")
}

impl TestTree {
    /// Take one `snout/testEvent`.
    pub fn apply(&mut self, ev: &Value) {
        let s = |k: &str| ev.get(k).and_then(Value::as_str).map(str::to_owned);
        let Some(id) = s("id") else { return };
        let event = s("event").unwrap_or_default();
        let suite = s("kind").as_deref() == Some("suite");
        let name = s("displayName").filter(|n| !n.is_empty()).or_else(|| s("name")).unwrap_or_default();
        if event == "output" {
            let text = s("text").unwrap_or_default();
            if let Some(node) = self.nodes.get_mut(&id) {
                node.output.push_str(&text);
            }
            return;
        }
        let parent = s("parent").map(|p| self.shown_parent(&p));
        if suite && (hidden_suite(&name) || self.hidden.contains_key(&id)) {
            self.hidden.entry(id).or_insert(parent.flatten());
            return;
        }
        let parent = parent.flatten();
        if !self.nodes.contains_key(&id) {
            match &parent {
                Some(p) if self.nodes.contains_key(p) => {
                    self.nodes.get_mut(p).expect("there").children.push(id.clone())
                }
                _ => self.roots.push(id.clone()),
            }
            self.nodes.insert(
                id.clone(),
                TestNode {
                    id: id.clone(),
                    parent: parent.clone(),
                    name: name.clone(),
                    class_name: s("className"),
                    suite,
                    state: TestState::Running,
                    millis: None,
                    message: None,
                    exception: None,
                    stack: None,
                    output: String::new(),
                    children: Vec::new(),
                },
            );
        }
        if event == "finished" {
            let node = self.nodes.get_mut(&id).expect("inserted");
            node.millis = ev.get("durationMillis").and_then(Value::as_u64);
            node.message = s("message");
            node.exception = s("exception");
            node.stack = s("stackTrace");
            node.state = match s("outcome").as_deref() {
                Some("failed") => TestState::Failed,
                Some("skipped") => TestState::Skipped,
                _ => TestState::Passed,
            };
            // A suite that failed on its own (a setup error) keeps that; otherwise it is what
            // its tests are.
            if suite && (node.state != TestState::Failed || node.message.is_none()) {
                let state = self.rollup(&id);
                self.nodes.get_mut(&id).expect("inserted").state = state;
            }
        }
    }

    /// Where a child of `parent` shows: under the nearest ancestor that is not hidden.
    fn shown_parent(&self, parent: &str) -> Option<String> {
        let mut at = parent.to_owned();
        for _ in 0..16 {
            match self.hidden.get(&at) {
                Some(Some(up)) => at = up.clone(),
                Some(None) => return None,
                None => return Some(at),
            }
        }
        None
    }

    /// A suite's state, from its tests.
    fn rollup(&self, id: &str) -> TestState {
        let Some(node) = self.nodes.get(id) else { return TestState::Passed };
        let states: Vec<TestState> = node.children.iter().map(|c| self.state_of(c)).collect();
        if states.contains(&TestState::Failed) {
            TestState::Failed
        } else if states.contains(&TestState::Running) {
            TestState::Running
        } else if !states.is_empty() && states.iter().all(|s| *s == TestState::Skipped) {
            TestState::Skipped
        } else {
            TestState::Passed
        }
    }

    /// A node's state; a suite still running shows what its tests are so far.
    pub fn state_of(&self, id: &str) -> TestState {
        match self.nodes.get(id) {
            Some(n) if n.suite && n.state == TestState::Running => {
                let r = self.rollup(id);
                if r == TestState::Failed { r } else { TestState::Running }
            }
            Some(n) => n.state,
            None => TestState::Passed,
        }
    }

    /// The tests (not suites) by state: passed, failed, skipped, running.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for n in self.nodes.values().filter(|n| !n.suite) {
            match n.state {
                TestState::Passed => c.0 += 1,
                TestState::Failed => c.1 += 1,
                TestState::Skipped => c.2 += 1,
                TestState::Running => c.3 += 1,
            }
        }
        c
    }

    /// The run never finished its suites (it was stopped): what still runs is done.
    pub fn settle(&mut self) {
        let ids: Vec<String> = self.nodes.keys().cloned().collect();
        for id in &ids {
            if let Some(n) = self.nodes.get_mut(id).filter(|n| !n.suite && n.state == TestState::Running) {
                n.state = TestState::Skipped;
            }
        }
        for id in &ids {
            if self.nodes[id].suite && self.nodes[id].state == TestState::Running {
                let s = self.rollup(id);
                self.nodes.get_mut(id).expect("there").state = s;
            }
        }
    }

    /// The rows to show, depth first: (depth, id).
    pub fn rows(&self, collapsed: &std::collections::HashSet<String>) -> Vec<(usize, String)> {
        fn walk(
            t: &TestTree,
            ids: &[String],
            depth: usize,
            collapsed: &std::collections::HashSet<String>,
            out: &mut Vec<(usize, String)>,
        ) {
            for id in ids {
                out.push((depth, id.clone()));
                if let Some(n) = t.nodes.get(id)
                    && !collapsed.contains(id)
                {
                    walk(t, &n.children, depth + 1, collapsed, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, &self.roots, 0, collapsed, &mut out);
        out
    }

    /// The first failed test, to select when the run ends.
    pub fn first_failure(&self) -> Option<String> {
        self.rows(&Default::default())
            .into_iter()
            .map(|(_, id)| id)
            .find(|id| self.nodes.get(id).is_some_and(|n| !n.suite && n.state == TestState::Failed))
    }
}

/// `expected: <5> but was: <4>` (JUnit 5), `expected:<5> but was:<4>` (JUnit 4),
/// `Expected: x / but: was y` (Hamcrest), `expected [5] but found [4]` (TestNG).
pub fn assertion_diff(message: &str) -> Option<(String, String)> {
    let strip = |s: &str| {
        let s = s.trim();
        let s = s.strip_prefix('<').and_then(|s| s.strip_suffix('>')).unwrap_or(s);
        let s = s.strip_prefix('[').and_then(|s| s.strip_suffix(']')).unwrap_or(s);
        s.trim().to_owned()
    };
    for (a, b) in [("expected: ", " but was: "), ("expected:", " but was:"), ("expected ", " but found ")] {
        if let Some(at) = message.find(a) {
            let rest = &message[at + a.len()..];
            if let Some(mid) = rest.find(b) {
                return Some((strip(&rest[..mid]), strip(&rest[mid + b.len()..])));
            }
        }
    }
    if let Some(at) = message.find("Expected: ") {
        let rest = &message[at + 10..];
        if let Some(mid) = rest.find("but: was ") {
            return Some((strip(&rest[..mid]), strip(&rest[mid + 9..])));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(event: &str, kind: &str, id: &str, parent: Option<&str>, name: &str) -> Value {
        json!({ "event": event, "kind": kind, "id": id, "parent": parent, "name": name, "displayName": name,
                "className": "demo.CalcTest" })
    }

    #[test]
    fn gradles_suites_are_left_out_and_states_roll_up() {
        let mut t = TestTree::default();
        t.apply(&ev("started", "suite", "1", None, "Gradle Test Run :app:test"));
        t.apply(&ev("started", "suite", "2", Some("1"), "Gradle Test Executor 3"));
        t.apply(&ev("started", "suite", "3", Some("2"), "CalcTest"));
        t.apply(&ev("started", "test", "4", Some("3"), "adds()"));
        t.apply(&json!({ "event": "output", "kind": "test", "id": "4", "text": "hello\n" }));
        let mut done = ev("finished", "test", "4", Some("3"), "adds()");
        done["outcome"] = json!("passed");
        done["durationMillis"] = json!(12);
        t.apply(&done);
        t.apply(&ev("started", "test", "5", Some("3"), "divides()"));
        assert_eq!(t.state_of("3"), TestState::Running);
        let mut failed = ev("finished", "test", "5", Some("3"), "divides()");
        failed["outcome"] = json!("failed");
        failed["message"] = json!("expected: <2> but was: <3>");
        t.apply(&failed);
        let mut suite = ev("finished", "suite", "3", Some("2"), "CalcTest");
        suite["outcome"] = json!("failed");
        t.apply(&suite);
        assert_eq!(t.roots, ["3"]);
        assert_eq!(t.nodes["3"].children, ["4", "5"]);
        assert_eq!(t.nodes["4"].output, "hello\n");
        assert_eq!(t.nodes["4"].millis, Some(12));
        assert_eq!(t.state_of("3"), TestState::Failed);
        assert_eq!(t.counts(), (1, 1, 0, 0));
        assert_eq!(t.nodes["5"].diff(), Some(("2".into(), "3".into())));
        assert_eq!(t.first_failure().as_deref(), Some("5"));
        let rows: Vec<(usize, String)> = t.rows(&Default::default());
        assert_eq!(rows, [(0, "3".into()), (1, "4".into()), (1, "5".into())]);
    }

    #[test]
    fn reads_the_usual_assertion_messages() {
        assert_eq!(assertion_diff("expected:<[5]> but was:<[4]>"), Some(("5".into(), "4".into())));
        assert_eq!(
            assertion_diff("\nExpected: is \"a\"\n     but: was \"b\""),
            Some(("is \"a\"".into(), "\"b\"".into()))
        );
        assert_eq!(assertion_diff("expected [1] but found [2]"), Some(("1".into(), "2".into())));
        assert_eq!(assertion_diff("boom"), None);
    }
}
