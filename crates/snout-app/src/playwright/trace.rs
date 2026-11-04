//! What a failure's detail shows from its `trace.zip`, read natively: the filmstrip (the
//! page's screencast frames), the actions with their times and errors, the requests that
//! failed and the console's errors. The trace format is Playwright's own, not a public
//! API: this keeps to the parts that have stayed put (`before`/`after` actions,
//! `screencast-frame`, `console`, `resource-snapshot`); Open Full Trace always has the rest.

use std::io::Read as _;
use std::path::Path;

use serde_json::Value;

/// A frame of the page's screencast.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The zip entry holding the JPEG.
    pub entry: String,
    /// Milliseconds from the trace's start.
    pub at: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub title: String,
    pub start: f64,
    pub end: Option<f64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    /// The HTTP status, or 0 when none came (the request failed).
    pub status: u32,
    pub millis: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    pub frames: Vec<Frame>,
    pub actions: Vec<Action>,
    pub failed_requests: Vec<Request>,
    pub console_errors: Vec<String>,
}

impl Trace {
    /// About `n` frames spread over the run, the last always among them.
    pub fn filmstrip(&self, n: usize) -> Vec<&Frame> {
        let len = self.frames.len();
        if len <= n {
            return self.frames.iter().collect();
        }
        let mut out: Vec<&Frame> = (0..n).map(|i| &self.frames[i * (len - 1) / (n - 1).max(1)]).collect();
        out.dedup_by(|a, b| a.entry == b.entry);
        out
    }
}

fn lines(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines().filter_map(|l| serde_json::from_str(l).ok())
}

/// Read the trace at `path`.
pub fn read(path: &Path) -> anyhow::Result<Trace> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
    let mut trace = Trace::default();
    let mut start: Option<f64> = None;
    let mut open: Vec<(String, usize)> = Vec::new();
    let text_of = |zip: &mut zip::ZipArchive<std::fs::File>, name: &str| -> String {
        let mut s = String::new();
        if let Ok(mut f) = zip.by_name(name) {
            let _ = f.read_to_string(&mut s);
        }
        s
    };
    // The browser's traces (`0-trace.trace`, …), not the test runner's (`test.trace`).
    for name in names.iter().filter(|n| n.ends_with(".trace") && *n != "test.trace") {
        for v in lines(&text_of(&mut zip, name)) {
            let s = |k: &str| v.get(k).and_then(Value::as_str);
            let f = |k: &str| v.get(k).and_then(Value::as_f64);
            match s("type") {
                Some("context-options") => {
                    if let Some(t) = f("monotonicTime") {
                        start = Some(start.map_or(t, |s| s.min(t)));
                    }
                }
                Some("before") => {
                    let class = s("class").unwrap_or_default();
                    if class == "Tracing" {
                        continue;
                    }
                    let title = s("title").or(s("apiName")).map(str::to_owned).unwrap_or_else(|| {
                        let method = s("method").unwrap_or_default();
                        let selector = v.pointer("/params/selector").and_then(Value::as_str);
                        let url = v.pointer("/params/url").and_then(Value::as_str);
                        let what = selector.or(url).map(|w| format!(" {w}")).unwrap_or_default();
                        format!(
                            "{}.{method}{what}",
                            class.chars().next().map_or(String::new(), |c| c.to_lowercase().collect())
                                + class.get(1..).unwrap_or_default()
                        )
                    });
                    open.push((s("callId").unwrap_or_default().to_owned(), trace.actions.len()));
                    trace.actions.push(Action { title, start: f("startTime").unwrap_or(0.), end: None, error: None });
                }
                Some("after") => {
                    let id = s("callId").unwrap_or_default();
                    if let Some(&(_, ix)) = open.iter().find(|(c, _)| c == id) {
                        let a = &mut trace.actions[ix];
                        a.end = f("endTime");
                        a.error = v
                            .pointer("/error/error/message")
                            .or_else(|| v.pointer("/error/message"))
                            .and_then(Value::as_str)
                            .map(|m| super::report::strip_ansi(m).lines().next().unwrap_or_default().to_owned());
                    }
                }
                Some("screencast-frame") => {
                    let entry = s("file")
                        .map(str::to_owned)
                        .or_else(|| s("sha1").map(|h| format!("resources/{h}")))
                        .unwrap_or_default();
                    if !entry.is_empty() {
                        trace.frames.push(Frame { entry, at: f("timestamp").unwrap_or(0.) });
                    }
                }
                Some("console") if matches!(s("messageType"), Some("error")) => {
                    trace.console_errors.push(s("text").unwrap_or_default().to_owned());
                }
                Some("event") if s("method") == Some("pageError") => {
                    let m = v.pointer("/params/error/error/message").and_then(Value::as_str).unwrap_or("Page error");
                    trace.console_errors.push(m.to_owned());
                }
                _ => {}
            }
        }
    }
    for name in names.iter().filter(|n| n.ends_with(".network")) {
        for v in lines(&text_of(&mut zip, name)) {
            if v.get("type").and_then(Value::as_str) != Some("resource-snapshot") {
                continue;
            }
            let snap = &v["snapshot"];
            let status = snap.pointer("/response/status").and_then(Value::as_i64).unwrap_or(0);
            let failed = snap.pointer("/response/_failureText").is_some() || status <= 0 || status >= 400;
            if failed {
                trace.failed_requests.push(Request {
                    method: snap.pointer("/request/method").and_then(Value::as_str).unwrap_or("GET").to_owned(),
                    url: snap.pointer("/request/url").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    status: status.max(0) as u32,
                    millis: snap.get("time").and_then(Value::as_f64).map(|t| t.max(0.) as u64),
                });
            }
        }
    }
    let start = start.unwrap_or(0.);
    for a in &mut trace.actions {
        a.start -= start;
        a.end = a.end.map(|e| e - start);
    }
    for fr in &mut trace.frames {
        fr.at -= start;
    }
    trace.frames.sort_by(|a, b| a.at.total_cmp(&b.at));
    Ok(trace)
}

/// The bytes of `entry` in the trace at `path` (a frame's JPEG).
pub fn entry(path: &Path, entry: &str) -> anyhow::Result<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut f = zip.by_name(entry)?;
    let mut out = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut out)?;
    Ok(out)
}
