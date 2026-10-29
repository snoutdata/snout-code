//! The debugger of a session: a DAP client attached to java-debug (in the engine's jdtls),
//! which the engine started for the run. Breakpoints are set before the program goes on;
//! when it stops, the threads, the stack, the variables of the selected frame and the
//! watches are read; the editor shows the line. A saved change jdtls has built is
//! reloaded into the running program (`hotcodereplace`, then `redefineClasses`).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use gpui::{Context, Task};
use serde_json::{Value, json};
use snout_lang::dap::{Dap, DapMessage};

use super::config::Breakpoint;
use super::{Runs, RunsEvent, Stream};

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub id: i64,
    pub name: String,
    pub path: Option<PathBuf>,
    /// The source's name, for frames without a file (a library's class).
    pub source: Option<String>,
    /// From 1.
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    pub name: String,
    pub value: String,
    pub ty: Option<String>,
    /// Non zero when it has children to show.
    pub reference: i64,
}

impl Var {
    fn of(v: &Value) -> Self {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        Var {
            name: s("name").unwrap_or_default(),
            value: s("value").or_else(|| s("result")).unwrap_or_default(),
            ty: s("type"),
            reference: v.get("variablesReference").and_then(Value::as_i64).unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Watch {
    pub expr: String,
    pub value: Option<Result<Var, String>>,
}

/// A row of the Variables pane.
#[derive(Clone, Debug, PartialEq)]
pub struct VarRow {
    pub depth: usize,
    /// The path of names that leads to it, to remember it open across stops.
    pub key: String,
    pub var: Var,
    pub scope: bool,
    pub watch: Option<usize>,
    pub open: bool,
}

#[derive(Default)]
pub struct Debugger {
    pub dap: Option<Dap>,
    pub ended: bool,
    pub paused: bool,
    pub reason: Option<String>,
    pub thread: Option<i64>,
    pub threads: Vec<(i64, String)>,
    pub frames: Vec<Frame>,
    pub frame: usize,
    pub scopes: Vec<Var>,
    pub children: HashMap<i64, Vec<Var>>,
    loading: HashSet<i64>,
    pub expanded: HashSet<String>,
    primed: bool,
    pub watches: Vec<Watch>,
    /// The last expression evaluated, and what it gave.
    pub evaluated: Option<(String, Result<Var, String>)>,
    /// What the debugger last said (a reload, an error).
    pub note: Option<String>,
    /// A connection to the debugger is being made: the start's answer and the engine's
    /// `snout/debugReady` both offer one, and only the first is taken (a second attach to
    /// the same JVM is refused, and could leave the session on the refused connection).
    pub attaching: bool,
    _events: Option<Task<()>>,
}

impl Debugger {
    /// A copy of what the Debug window shows (the connection shared, no event task).
    pub fn snapshot(&self) -> Self {
        Self {
            dap: self.dap.clone(),
            ended: self.ended,
            paused: self.paused,
            reason: self.reason.clone(),
            thread: self.thread,
            threads: self.threads.clone(),
            frames: self.frames.clone(),
            frame: self.frame,
            scopes: self.scopes.clone(),
            children: self.children.clone(),
            loading: HashSet::new(),
            expanded: self.expanded.clone(),
            primed: self.primed,
            watches: self.watches.clone(),
            evaluated: self.evaluated.clone(),
            note: self.note.clone(),
            attaching: self.attaching,
            _events: None,
        }
    }

    /// Connected, and the program not ended.
    pub fn live(&self) -> bool {
        self.dap.as_ref().is_some_and(Dap::is_open) && !self.ended
    }

    pub fn disconnect(&mut self) {
        if let Some(dap) = self.dap.take() {
            let bye = dap.request("disconnect", json!({ "terminateDebuggee": true }));
            std::thread::spawn(move || {
                let _ = snout_lang::block_on(bye);
                dap.close();
            });
        }
        self.end();
    }

    pub fn end(&mut self) {
        self.ended = true;
        self.paused = false;
        self.frames.clear();
        self.scopes.clear();
        self.children.clear();
        if let Some(dap) = self.dap.take() {
            dap.close();
        }
    }

    /// The selected frame's file and line (from 0), while paused.
    pub fn exec_point(&self) -> Option<(PathBuf, u32)> {
        if !self.paused {
            return None;
        }
        let f = self.frames.get(self.frame)?;
        Some((f.path.clone()?, f.line.saturating_sub(1)))
    }

    /// The selected frame's first scope (its local variables), name and value.
    pub fn locals(&self) -> Vec<(String, String)> {
        if !self.paused {
            return Vec::new();
        }
        let Some(scope) = self.scopes.first() else { return Vec::new() };
        self.children
            .get(&scope.reference)
            .map(|vars| vars.iter().take(40).map(|v| (v.name.clone(), v.value.clone())).collect())
            .unwrap_or_default()
    }

    /// The Variables pane: the watches, then each scope and what is open in it.
    pub fn rows(&self) -> Vec<VarRow> {
        let mut out = Vec::new();
        for (i, w) in self.watches.iter().enumerate() {
            let key = format!("watch:{i}:{}", w.expr);
            let var = match &w.value {
                Some(Ok(v)) => Var { name: w.expr.clone(), ..v.clone() },
                Some(Err(e)) => Var { name: w.expr.clone(), value: e.clone(), ty: None, reference: 0 },
                None => Var { name: w.expr.clone(), value: String::new(), ty: None, reference: 0 },
            };
            let open = self.expanded.contains(&key);
            out.push(VarRow { depth: 0, key: key.clone(), var: var.clone(), scope: false, watch: Some(i), open });
            if open {
                self.walk(var.reference, &key, 1, &mut out);
            }
        }
        for scope in &self.scopes {
            let key = format!("scope:{}", scope.name);
            let open = self.expanded.contains(&key);
            out.push(VarRow { depth: 0, key: key.clone(), var: scope.clone(), scope: true, watch: None, open });
            if open {
                self.walk(scope.reference, &key, 1, &mut out);
            }
        }
        out
    }

    fn walk(&self, reference: i64, parent: &str, depth: usize, out: &mut Vec<VarRow>) {
        if depth > 12 {
            return;
        }
        for v in self.children.get(&reference).into_iter().flatten() {
            let key = format!("{parent}/{}", v.name);
            let open = v.reference > 0 && self.expanded.contains(&key);
            out.push(VarRow { depth, key: key.clone(), var: v.clone(), scope: false, watch: None, open });
            if open {
                self.walk(v.reference, &key, depth + 1, out);
            }
        }
    }

    /// The references shown open whose children are not read yet.
    fn missing(&self) -> Vec<i64> {
        self.rows()
            .into_iter()
            .filter(|r| r.open && r.var.reference > 0)
            .map(|r| r.var.reference)
            .filter(|r| !self.children.contains_key(r) && !self.loading.contains(r))
            .collect()
    }
}

const WAIT: Duration = Duration::from_secs(5);

impl Runs {
    fn dbg(&mut self, id: u64) -> Option<&mut Debugger> {
        self.session_mut(id)?.debug.as_mut()
    }

    fn dap_of(&self, id: u64) -> Option<Dap> {
        self.session(id)?.debug.as_ref()?.dap.clone().filter(Dap::is_open)
    }

    /// Connect session `id`'s debugger to java-debug on `port`, and attach to the program.
    pub(super) fn attach(&mut self, id: u64, port: u16, attach: Value, cx: &mut Context<Self>) {
        match self.dbg(id) {
            Some(d) if d.attaching || d.dap.is_some() || d.ended => return,
            Some(d) => d.attaching = true,
            None => return,
        }
        let host = crate::host::host(cx);
        let connect = cx.background_executor().spawn(async move { Dap::connect(&*host, port, WAIT) });
        cx.spawn(async move |this, cx| {
            let (dap, events) = match connect.await {
                Ok(c) => c,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(d) = this.dbg(id) {
                            d.attaching = false;
                        }
                        if let Some(s) = this.session_mut(id) {
                            s.push(Stream::Err, &format!("Could not connect the debugger: {e}"));
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let listen = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    while let Ok(m) = events.recv().await {
                        let closed = m == DapMessage::Closed;
                        if this.update(cx, |this, cx| this.on_dap(id, m, cx)).is_err() || closed {
                            return;
                        }
                    }
                }
            });
            let ok = this.update(cx, |this, cx| {
                let Some(d) = this.dbg(id).filter(|d| !d.ended) else { return false };
                d.dap = Some(dap.clone());
                d._events = Some(listen);
                if let Some(s) = this.session_mut(id) {
                    s.push(Stream::System, "Debugger attached");
                }
                cx.notify();
                true
            });
            if !matches!(ok, Ok(true)) {
                dap.close();
                return;
            }
            let init = dap.request(
                "initialize",
                json!({ "clientID": "snoutcode", "clientName": "SnoutCode", "adapterID": "java",
                        "linesStartAt1": true, "columnsStartAt1": true, "pathFormat": "path",
                        "supportsVariableType": true, "supportsRunInTerminalRequest": false }),
            );
            if let Err(e) = init.await {
                eprintln!("[debug] initialize: {e}");
            }
            if let Err(e) = dap.request("attach", attach).await {
                let _ = this.update(cx, |this, cx| {
                    if let Some(s) = this.session_mut(id) {
                        s.push(Stream::Err, &format!("The debugger could not attach: {e}"));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn on_dap(&mut self, id: u64, m: DapMessage, cx: &mut Context<Self>) {
        match m {
            DapMessage::Closed => {
                if let Some(d) = self.dbg(id) {
                    d.end();
                }
                self.sync_marks(cx);
            }
            DapMessage::Event { event, body } => match event.as_str() {
                "initialized" => self.configure(id, cx),
                "stopped" => {
                    let Some(d) = self.dbg(id) else { return };
                    d.paused = true;
                    d.reason = body.get("reason").and_then(Value::as_str).map(str::to_owned);
                    d.thread = body.get("threadId").and_then(Value::as_i64).or(d.thread);
                    d.frame = 0;
                    self.clear_run_to(id, cx);
                    self.refresh_stack(id, cx);
                    cx.emit(RunsEvent::Show(super::Place::Debug));
                }
                "continued" => {
                    if let Some(d) = self.dbg(id) {
                        d.paused = false;
                        d.frames.clear();
                        d.scopes.clear();
                        d.children.clear();
                    }
                    self.sync_marks(cx);
                }
                "terminated" | "exited" => {
                    self.run_to.take_if(|(sid, _, _)| *sid == id);
                    if let Some(d) = self.dbg(id) {
                        d.paused = false;
                        d.frames.clear();
                    }
                    self.sync_marks(cx);
                }
                "hotcodereplace" => {
                    let done = body.get("changeType").and_then(Value::as_str) == Some("BUILD_COMPLETE");
                    if done {
                        self.hot_swap(id, cx);
                    }
                }
                _ => {}
            },
        }
        cx.notify();
    }

    /// Set every breakpoint, then let the program go on.
    fn configure(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let mut by_file: HashMap<PathBuf, Vec<Breakpoint>> = HashMap::new();
        for b in &self.project.breakpoints {
            by_file.entry(b.path.clone()).or_default().push(b.clone());
        }
        let calls: Vec<_> =
            by_file.into_iter().map(|(path, list)| dap.request("setBreakpoints", bp_args(&path, &list))).collect();
        cx.spawn(async move |_, _| {
            for call in calls {
                if let Err(e) = call.await {
                    eprintln!("[debug] setBreakpoints: {e}");
                }
            }
            let _ = dap.request("setExceptionBreakpoints", json!({ "filters": [] })).await;
            if let Err(e) = dap.request("configurationDone", json!({})).await {
                eprintln!("[debug] configurationDone: {e}");
            }
        })
        .detach();
    }

    /// Send `path`'s breakpoints to session `id`'s debugger.
    pub(super) fn send_breakpoints(&mut self, id: u64, path: PathBuf, list: Vec<Breakpoint>, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let call = dap.request("setBreakpoints", bp_args(&path, &list));
        cx.background_executor()
            .spawn(async move {
                if let Err(e) = call.await {
                    eprintln!("[debug] setBreakpoints: {e}");
                }
            })
            .detach();
    }

    /// Read the threads and the stopped thread's stack, then select its top frame.
    fn refresh_stack(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let thread = self.dbg(id).and_then(|d| d.thread);
        let host = crate::host::host(cx);
        cx.spawn(async move |this, cx| {
            let threads = dap.request("threads", json!({})).await.unwrap_or_default();
            let threads: Vec<(i64, String)> = threads
                .get("threads")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|t| {
                    Some((t.get("id")?.as_i64()?, t.get("name").and_then(Value::as_str).unwrap_or("").to_owned()))
                })
                .collect();
            let thread = thread.or_else(|| threads.first().map(|t| t.0));
            let frames = match thread {
                Some(t) => dap
                    .request("stackTrace", json!({ "threadId": t, "startFrame": 0, "levels": 60 }))
                    .await
                    .unwrap_or_default(),
                None => Value::Null,
            };
            // Which frames' files are there: asked of the host, off the UI thread.
            let frames: Vec<Frame> = cx
                .background_executor()
                .spawn(async move {
                    frames
                        .get("stackFrames")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|f| Frame {
                            id: f.get("id").and_then(Value::as_i64).unwrap_or(0),
                            name: f.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
                            path: f
                                .pointer("/source/path")
                                .and_then(Value::as_str)
                                .map(PathBuf::from)
                                .filter(|p| host.is_file(p)),
                            source: f.pointer("/source/name").and_then(Value::as_str).map(str::to_owned),
                            line: f.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
                        })
                        .collect()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(d) = this.dbg(id) else { return };
                d.threads = threads;
                d.thread = thread;
                d.frames = frames;
                // The first frame with a file of the project's, as a debugger shows it.
                let first = d.frames.iter().position(|f| f.path.is_some()).unwrap_or(0);
                this.select_frame(id, first, cx);
            });
        })
        .detach();
    }

    /// Switch to another thread's stack.
    pub fn select_thread(&mut self, id: u64, thread: i64, cx: &mut Context<Self>) {
        if let Some(d) = self.dbg(id) {
            d.thread = Some(thread);
            d.frame = 0;
        }
        self.refresh_stack(id, cx);
    }

    /// Show frame `ix`: its line in the editor, its scopes and the watches in it.
    pub fn select_frame(&mut self, id: u64, ix: usize, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let Some(d) = self.dbg(id) else { return };
        d.frame = ix.min(d.frames.len().saturating_sub(1));
        d.scopes.clear();
        d.children.clear();
        d.loading.clear();
        let Some(frame) = d.frames.get(d.frame).cloned() else {
            cx.notify();
            return;
        };
        if let Some(path) = &frame.path {
            cx.emit(RunsEvent::OpenAt(path.clone(), frame.line.saturating_sub(1)));
        }
        let call = dap.request("scopes", json!({ "frameId": frame.id }));
        cx.spawn(async move |this, cx| {
            let scopes = call.await.unwrap_or_default();
            let _ = this.update(cx, |this, cx| {
                let Some(d) = this.dbg(id) else { return };
                d.scopes = scopes.get("scopes").and_then(Value::as_array).into_iter().flatten().map(Var::of).collect();
                if !d.primed
                    && let Some(first) = d.scopes.first()
                {
                    d.primed = true;
                    d.expanded.insert(format!("scope:{}", first.name));
                }
                this.load_missing(id, cx);
                this.eval_watches(id, cx);
                cx.notify();
            });
        })
        .detach();
        self.sync_marks(cx);
        cx.notify();
    }

    /// Read the children of every open row that lacks them.
    fn load_missing(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let Some(d) = self.dbg(id) else { return };
        for reference in d.missing() {
            d.loading.insert(reference);
            let call = dap.request("variables", json!({ "variablesReference": reference }));
            cx.spawn(async move |this, cx| {
                let vars = call.await.unwrap_or_default();
                let _ = this.update(cx, |this, cx| {
                    let Some(d) = this.dbg(id) else { return };
                    d.loading.remove(&reference);
                    let list =
                        vars.get("variables").and_then(Value::as_array).into_iter().flatten().map(Var::of).collect();
                    d.children.insert(reference, list);
                    this.load_missing(id, cx);
                    this.sync_marks(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// Open or close a row of the Variables pane.
    pub fn toggle_var(&mut self, id: u64, key: &str, cx: &mut Context<Self>) {
        let Some(d) = self.dbg(id) else { return };
        if !d.expanded.remove(key) {
            d.expanded.insert(key.to_owned());
        }
        self.load_missing(id, cx);
        cx.notify();
    }

    /// Step, continue or pause: `next`, `stepIn`, `stepOut`, `continue`, `pause`.
    pub fn step(&mut self, id: u64, command: &'static str, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let Some(d) = self.dbg(id) else { return };
        let thread = d.thread.or_else(|| d.threads.first().map(|t| t.0)).unwrap_or(0);
        if command != "pause" && !d.paused {
            return;
        }
        let call = dap.request(command, json!({ "threadId": thread }));
        if command != "pause" {
            d.paused = false;
            self.sync_marks(cx);
        }
        cx.spawn(async move |this, cx| {
            if let Err(e) = call.await {
                let _ = this.update(cx, |this, cx| {
                    if let Some(d) = this.dbg(id) {
                        d.note = Some(e.to_string());
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    fn frame_id(&mut self, id: u64) -> Option<i64> {
        let d = self.dbg(id)?;
        d.frames.get(d.frame).map(|f| f.id)
    }

    /// Evaluate `expr` in the selected frame.
    pub fn evaluate(&mut self, id: u64, expr: String, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let frame = self.frame_id(id);
        let call = dap.request("evaluate", json!({ "expression": expr, "frameId": frame, "context": "repl" }));
        cx.spawn(async move |this, cx| {
            let result = call.await.map(|v| Var::of(&v)).map_err(|e| e.message);
            let _ = this.update(cx, |this, cx| {
                if let Some(d) = this.dbg(id) {
                    let reference = result.as_ref().map(|v| v.reference).unwrap_or(0);
                    d.evaluated = Some((expr, result));
                    if reference > 0 {
                        d.children.remove(&reference);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn add_watch(&mut self, id: u64, expr: String, cx: &mut Context<Self>) {
        let expr = expr.trim().to_owned();
        let Some(d) = self.dbg(id) else { return };
        if expr.is_empty() || d.watches.iter().any(|w| w.expr == expr) {
            return;
        }
        d.watches.push(Watch { expr, value: None });
        self.eval_watches(id, cx);
        cx.notify();
    }

    pub fn remove_watch(&mut self, id: u64, ix: usize, cx: &mut Context<Self>) {
        if let Some(d) = self.dbg(id)
            && ix < d.watches.len()
        {
            d.watches.remove(ix);
        }
        cx.notify();
    }

    fn eval_watches(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let frame = self.frame_id(id);
        let Some(d) = self.dbg(id) else { return };
        if !d.paused {
            return;
        }
        for (ix, w) in d.watches.iter().enumerate() {
            let expr = w.expr.clone();
            let call = dap.request("evaluate", json!({ "expression": expr, "frameId": frame, "context": "watch" }));
            cx.spawn(async move |this, cx| {
                let value = call.await.map(|v| Var::of(&v)).map_err(|e| e.message);
                let _ = this.update(cx, |this, cx| {
                    let Some(d) = this.dbg(id) else { return };
                    if let Some(w) = d.watches.get_mut(ix).filter(|w| w.expr == expr) {
                        w.value = Some(value);
                    }
                    this.load_missing(id, cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// jdtls built a saved change: reload the changed classes into the program.
    fn hot_swap(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(dap) = self.dap_of(id) else { return };
        let call = dap.request("redefineClasses", json!({}));
        cx.spawn(async move |this, cx| {
            let result = call.await;
            let _ = this.update(cx, |this, cx| {
                let text = match &result {
                    Ok(v) => {
                        let classes: Vec<&str> = v
                            .get("changedClasses")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        match classes.len() {
                            0 => "Nothing to reload".to_owned(),
                            1 => format!("Reloaded {}", classes[0]),
                            n => format!("Reloaded {n} classes"),
                        }
                    }
                    Err(e) => format!("Could not reload the changed classes: {e}"),
                };
                if let Some(s) = this.session_mut(id) {
                    s.push(Stream::System, &text);
                    if let Some(d) = s.debug.as_mut() {
                        d.note = Some(text.clone());
                    }
                }
                this.say(text, cx);
                // The stack may have moved (frames dropped by the reload).
                if this.dbg(id).is_some_and(|d| d.paused) {
                    this.refresh_stack(id, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn bp_args(path: &std::path::Path, list: &[Breakpoint]) -> Value {
    let list: Vec<&Breakpoint> = list.iter().filter(|b| b.enabled).collect();
    let bps: Vec<Value> = list
        .iter()
        .map(|b| match &b.condition {
            Some(c) => json!({ "line": b.line + 1, "condition": c }),
            None => json!({ "line": b.line + 1 }),
        })
        .collect();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    json!({ "source": { "path": path, "name": name }, "breakpoints": bps, "lines": list.iter().map(|b| b.line + 1).collect::<Vec<_>>() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_show_watches_then_open_scopes_and_their_children() {
        let mut d = Debugger { paused: true, ..Default::default() };
        d.scopes = vec![Var { name: "Local".into(), value: String::new(), ty: None, reference: 1 }];
        d.children.insert(
            1,
            vec![
                Var { name: "i".into(), value: "1".into(), ty: Some("int".into()), reference: 0 },
                Var { name: "list".into(), value: "ArrayList (size=1)".into(), ty: None, reference: 2 },
            ],
        );
        d.watches.push(Watch {
            expr: "i + 1".into(),
            value: Some(Ok(Var { name: String::new(), value: "2".into(), ty: None, reference: 0 })),
        });
        d.expanded.insert("scope:Local".into());
        let rows = d.rows();
        let names: Vec<&str> = rows.iter().map(|r| r.var.name.as_str()).collect();
        assert_eq!(names, ["i + 1", "Local", "i", "list"]);
        assert_eq!(rows[0].var.value, "2");
        assert!(d.missing().is_empty());
        d.expanded.insert("scope:Local/list".into());
        assert_eq!(d.missing(), [2]);
        assert_eq!(
            d.locals(),
            [("i".to_owned(), "1".to_owned()), ("list".to_owned(), "ArrayList (size=1)".to_owned())]
        );
        let args = bp_args(
            std::path::Path::new("/w/A.java"),
            &[Breakpoint::new("/w/A.java", 4, Some("i > 2".into())), {
                let mut off = Breakpoint::new("/w/A.java", 9, None);
                off.enabled = false;
                off
            }],
        );
        assert_eq!(args["breakpoints"][0], json!({ "line": 5, "condition": "i > 2" }));
        assert_eq!(args["breakpoints"].as_array().unwrap().len(), 1, "a disabled breakpoint is not sent");
    }
}
