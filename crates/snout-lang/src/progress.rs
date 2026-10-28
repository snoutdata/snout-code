//! What a server is busy with, from its `$/progress` reports, for a status bar.

use std::collections::BTreeMap;

use lsp_types::{NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress};

/// One piece of work in progress.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Work {
    pub title: String,
    pub message: Option<String>,
    pub percentage: Option<u32>,
}

impl Work {
    /// "Building workspace: 45%" and the like.
    pub fn label(&self) -> String {
        let mut s = self.title.clone();
        if let Some(m) = self.message.as_deref().filter(|m| !m.is_empty() && *m != self.title) {
            if !s.is_empty() {
                s.push_str(": ");
            }
            s.push_str(m);
        }
        if let Some(p) = self.percentage {
            s.push_str(&format!(" ({p}%)"));
        }
        s
    }
}

/// The work a server has begun and not yet ended, by token, oldest first.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    work: BTreeMap<u64, (String, Work)>,
    next: u64,
}

fn token_key(t: &NumberOrString) -> String {
    match t {
        NumberOrString::Number(n) => n.to_string(),
        NumberOrString::String(s) => s.clone(),
    }
}

impl Progress {
    /// Take a `$/progress` notification; true when what shows changed.
    pub fn apply(&mut self, params: ProgressParams) -> bool {
        let key = token_key(&params.token);
        let ProgressParamsValue::WorkDone(report) = params.value;
        let slot = self.work.iter().find(|(_, (k, _))| *k == key).map(|(i, _)| *i);
        match report {
            WorkDoneProgress::Begin(b) => {
                if let Some(i) = slot {
                    self.work.remove(&i);
                }
                self.next += 1;
                let work = Work { title: b.title, message: b.message, percentage: b.percentage };
                self.work.insert(self.next, (key, work));
                true
            }
            WorkDoneProgress::Report(r) => {
                let Some(i) = slot else { return false };
                let work = &mut self.work.get_mut(&i).expect("found").1;
                if r.message.is_some() {
                    work.message = r.message;
                }
                if r.percentage.is_some() {
                    work.percentage = r.percentage;
                }
                true
            }
            WorkDoneProgress::End(_) => slot.and_then(|i| self.work.remove(&i)).is_some(),
        }
    }

    /// The newest work still running.
    pub fn current(&self) -> Option<&Work> {
        self.work.values().next_back().map(|(_, w)| w)
    }

    pub fn is_busy(&self) -> bool {
        !self.work.is_empty()
    }

    pub fn clear(&mut self) {
        self.work.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(v: serde_json::Value) -> ProgressParams {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn follows_begin_report_and_end() {
        let mut pr = Progress::default();
        assert!(pr.apply(p(json!({"token": "a", "value": {"kind": "begin", "title": "Building workspace"}}))));
        assert!(pr.apply(p(json!({"token": 7, "value": {"kind": "begin", "title": "Indexing", "percentage": 0}}))));
        assert_eq!(pr.current().unwrap().label(), "Indexing (0%)");
        pr.apply(p(json!({"token": 7, "value": {"kind": "report", "message": "jars", "percentage": 40}})));
        assert_eq!(pr.current().unwrap().label(), "Indexing: jars (40%)");
        pr.apply(p(json!({"token": 7, "value": {"kind": "end"}})));
        assert_eq!(pr.current().unwrap().label(), "Building workspace");
        pr.apply(p(json!({"token": "a", "value": {"kind": "end"}})));
        assert!(!pr.is_busy());
        assert!(!pr.apply(p(json!({"token": "zzz", "value": {"kind": "end"}}))), "an unknown token changes nothing");
    }
}
