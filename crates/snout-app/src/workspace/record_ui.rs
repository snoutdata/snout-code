//! The workspace's side of writing tests: Record at Cursor, Pick Locator and UI Mode, from
//! the spec tab in front. A recording follows the recorder's output file and writes its
//! steps where the caret was; a pick puts each locator at the caret, and a pick right after
//! another (the caret not moved) replaces it. Both show a bar on the tab, with Stop; both
//! end when their browser is closed.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{Context, Entity, Task, WeakEntity, Window};

use super::Workspace;
use crate::editor::EditorPanel;
use crate::editor::recording::RecordBar;
use crate::playwright::gutter::{self, Spot};
use crate::playwright::record::{self, Start, scriptable};

/// A recording that writes into a tab.
pub struct Recording {
    /// Its session, once started.
    pub id: Option<u64>,
    editor: WeakEntity<EditorPanel>,
    out: PathBuf,
    spot: Spot,
    unit: String,
    base: Option<String>,
    gap: &'static str,
    /// Where the steps are in the tab, and what was written there (at first, what the
    /// place held).
    at: usize,
    written: String,
    /// The recorder's file as read last.
    last: String,
    steps: usize,
    /// The steps were edited meanwhile: no more writing.
    lost: bool,
    _poll: Option<Task<()>>,
}

/// A locator pick that writes into a tab.
pub struct Picking {
    pub id: Option<u64>,
    editor: WeakEntity<EditorPanel>,
    /// Where the last locator went, while the caret is still after it.
    last: Option<Range<usize>>,
    picked: usize,
}

impl Workspace {
    fn note(&self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        self.model.update(cx, |m, cx| {
            m.message = text;
            cx.notify();
        });
    }

    /// The spec tab in front and its Playwright project, or why there is none.
    fn spec_tab(
        &self,
        what: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<(Entity<EditorPanel>, PathBuf), String> {
        let root = self
            .pw_tests
            .read(cx)
            .root
            .clone()
            .filter(|_| self.pw_tests.read(cx).playwright)
            .ok_or(format!("{what} works in a Playwright project"))?;
        let editor = self
            .active_editor(window, cx)
            .filter(|e| scriptable(&e.read(cx).path))
            .ok_or(format!("{what}: open a spec file and put the caret where it goes"))?;
        Ok((editor, root))
    }

    /// Where recordings and picks start: the configuration's projects as listed, and the
    /// environment picked.
    fn record_start(&self, root: &Path, cx: &mut Context<Self>) -> Start {
        let projects = self.pw_tests.read(cx).listing.as_ref().map(|l| l.projects.clone()).unwrap_or_default();
        let environment = self.runs.read(cx).environment().cloned();
        Start::of(&projects, environment.as_ref(), root)
    }

    /// A recording: `None` when there is none, else whether its recorder has started.
    pub(crate) fn recording_state(&self) -> Option<bool> {
        self.recording.as_ref().map(|r| r.id.is_some())
    }

    /// A pick: `None` when there is none, else whether its browser has started.
    pub(crate) fn picking_state(&self) -> Option<bool> {
        self.picking.as_ref().map(|p| p.id.is_some())
    }

    /// Record at Cursor: the recorder, its steps written where the caret is.
    pub fn record_at_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.note("A recording is going: stop it first", cx);
            return;
        }
        let (editor, root) = match self.spec_tab("Record at Cursor", window, cx) {
            Ok(found) => found,
            Err(why) => return self.note(why, cx),
        };
        let (text, caret, path) = {
            let e = editor.read(cx);
            (e.text(cx), e.state.read(cx).cursor(), e.path.clone())
        };
        let spot = gutter::spot(&path, &text, caret);
        let (range, gap) = record::place(&text, caret, &spot);
        let start = self.record_start(&root, cx);
        let out = record::recording_file(crate::host::host(cx).info());
        self.recording = Some(Recording {
            id: None,
            editor: editor.downgrade(),
            out: out.clone(),
            spot,
            unit: gutter::indent_unit(&text),
            base: start.url.clone(),
            gap,
            at: range.start,
            written: text[range].to_owned(),
            last: String::new(),
            steps: 0,
            lost: false,
            _poll: None,
        });
        editor.update(cx, |e, cx| e.set_record_bar(Some(RecordBar::Recording { steps: 0, lost: false }), cx));
        let (tx, rx) = futures::channel::oneshot::channel();
        self.runs.update(cx, |r, cx| {
            r.start_node_tool(
                Some(path),
                move |host, root, node, vars| record::codegen_launch(host, root, node, &start, &out, vars),
                move |_, started, _| {
                    let _ = tx.send(started);
                },
                cx,
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let started = rx.await.unwrap_or_else(|_| Err("It did not start".into()));
            let _ = this.update_in(cx, |w, window, cx| w.recording_started(started, window, cx));
        })
        .detach();
    }

    fn recording_started(&mut self, started: Result<u64, String>, window: &mut Window, cx: &mut Context<Self>) {
        let id = match started {
            Ok(id) => id,
            Err(why) => {
                if let Some(r) = self.recording.take()
                    && let Some(e) = r.editor.upgrade()
                {
                    e.update(cx, |e, cx| e.set_record_bar(None, cx));
                }
                return self.note(why, cx);
            }
        };
        let Some(r) = self.recording.as_mut() else { return };
        r.id = Some(id);
        let host = crate::host::host(cx);
        let out = r.out.clone();
        r._poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let (h, o) = (host.clone(), out.clone());
                let text = cx.background_executor().spawn(async move { h.read_to_string(&o).ok() }).await;
                let going = this.update_in(cx, |w, window, cx| {
                    if let Some(t) = text {
                        w.recorded(t, window, cx);
                    }
                    w.recording.as_ref().is_some_and(|r| r.id == Some(id))
                });
                if !matches!(going, Ok(true)) {
                    return;
                }
            }
        }));
    }

    /// The recorder's file says `generated` now: write its steps.
    fn recorded(&mut self, generated: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.recording.as_mut() else { return };
        if generated == r.last || r.lost {
            return;
        }
        let Some(steps) = record::steps(&generated) else { return };
        r.last = generated;
        if steps.iter().all(|s| s.trim().is_empty()) {
            return;
        }
        let Some(editor) = r.editor.upgrade() else {
            r.lost = true;
            return;
        };
        let rendered = format!("{}{}", r.gap, record::render(&r.spot, &steps, &r.last, &r.unit, r.base.as_deref()));
        let now = editor.read(cx).text(cx);
        if now.get(r.at..r.at + r.written.len()) != Some(r.written.as_str()) {
            // Moved by an edit before it, or edited itself.
            match Some(&r.written).filter(|w| !w.trim().is_empty()).and_then(|w| now.find(w.as_str())) {
                Some(at) => r.at = at,
                None => {
                    r.lost = true;
                    let steps = r.steps;
                    editor.update(cx, |e, cx| e.set_record_bar(Some(RecordBar::Recording { steps, lost: true }), cx));
                    return;
                }
            }
        }
        let range = r.at..r.at + r.written.len();
        editor.update(cx, |e, cx| e.write_range(range, &rendered, window, cx));
        r.written = rendered;
        r.steps = record::count(&steps);
        let steps = r.steps;
        editor.update(cx, |e, cx| e.set_record_bar(Some(RecordBar::Recording { steps, lost: false }), cx));
    }

    /// Session `id` ended: the recording or the pick it was, if any.
    pub(super) fn tool_ended(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording.as_ref().is_some_and(|r| r.id == Some(id)) {
            let host = crate::host::host(cx);
            let out = self.recording.as_ref().map(|r| r.out.clone()).unwrap_or_default();
            if let Ok(text) = host.read_to_string(&out) {
                self.recorded(text, window, cx);
            }
            let _ = host.remove_file(&out);
            let Some(r) = self.recording.take() else { return };
            let failed = self
                .runs
                .read(cx)
                .session(id)
                .and_then(|s| s.ended.as_ref())
                .is_some_and(|e| e.outcome == crate::run::Outcome::Failed);
            let name = r.editor.upgrade().map(|e| {
                e.update(cx, |e, cx| e.set_record_bar(None, cx));
                e.read(cx).path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            });
            let text = match (r.steps, failed) {
                (0, true) => "The recorder stopped: the Build window says why".to_owned(),
                (0, false) => "The recording ended with no steps".to_owned(),
                (1, _) => format!("Recorded 1 step into {}", name.unwrap_or_default()),
                (n, _) => format!("Recorded {n} steps into {}", name.unwrap_or_default()),
            };
            self.note(text, cx);
        }
        if self.picking.as_ref().is_some_and(|p| p.id == Some(id)) {
            let Some(p) = self.picking.take() else { return };
            if let Some(e) = p.editor.upgrade() {
                e.update(cx, |e, cx| e.set_record_bar(None, cx));
            }
        }
    }

    /// Pick Locator: the browser where the tests start; each element clicked puts its
    /// locator at the caret.
    pub fn pick_locator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picking.is_some() {
            self.note("Picking is on: click an element in the browser", cx);
            return;
        }
        let (editor, root) = match self.spec_tab("Pick Locator", window, cx) {
            Ok(found) => found,
            Err(why) => return self.note(why, cx),
        };
        let path = editor.read(cx).path.clone();
        let start = self.record_start(&root, cx);
        self.picking = Some(Picking { id: None, editor: editor.downgrade(), last: None, picked: 0 });
        editor.update(cx, |e, cx| e.set_record_bar(Some(RecordBar::Picking { picked: 0 }), cx));
        let (tx, rx) = futures::channel::oneshot::channel();
        self.runs.update(cx, |r, cx| {
            r.start_node_tool(
                Some(path),
                move |host, root, node, vars| record::picker_launch(host, root, node, &start, vars),
                move |_, started, _| {
                    let _ = tx.send(started);
                },
                cx,
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let started = rx.await.unwrap_or_else(|_| Err("It did not start".into()));
            let _ = this.update_in(cx, |w, _, cx| match started {
                Ok(id) => {
                    if let Some(p) = w.picking.as_mut() {
                        p.id = Some(id);
                    }
                }
                Err(why) => {
                    if let Some(e) = w.picking.take().and_then(|p| p.editor.upgrade()) {
                        e.update(cx, |e, cx| e.set_record_bar(None, cx));
                    }
                    w.note(why, cx);
                }
            });
        })
        .detach();
    }

    /// A tool of ours said `v`: a locator picked, or why it cannot go on.
    pub(super) fn tool_said(&mut self, id: u64, v: &serde_json::Value, window: &mut Window, cx: &mut Context<Self>) {
        if !self.picking.as_ref().is_some_and(|p| p.id == Some(id)) {
            return;
        }
        match v.get("e").and_then(|e| e.as_str()) {
            Some("picked") => {
                let Some(locator) = v.get("locator").and_then(|l| l.as_str()) else { return };
                self.picked(locator, window, cx);
            }
            Some("error") => {
                let why = v.get("message").and_then(|m| m.as_str()).unwrap_or("Pick Locator stopped").to_owned();
                self.note(why, cx);
            }
            _ => {}
        }
    }

    fn picked(&mut self, locator: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.picking.as_mut() else { return };
        let Some(editor) = p.editor.upgrade() else { return };
        let (text, caret) = {
            let e = editor.read(cx);
            (e.text(cx), e.state.read(cx).cursor())
        };
        // Right after the last pick, a new one takes its place.
        let range = p.last.clone().filter(|r| r.end == caret && r.end <= text.len()).unwrap_or(caret..caret);
        let line_start = text[..range.start].rfind('\n').map_or(0, |i| i + 1);
        let insert = record::picked_text(&text[line_start..range.start], locator);
        let at = editor.update(cx, |e, cx| e.write_range(range, &insert, window, cx));
        p.last = Some(at);
        p.picked += 1;
        let picked = p.picked;
        editor.update(cx, |e, cx| e.set_record_bar(Some(RecordBar::Picking { picked }), cx));
        self.note(format!("Picked {locator}"), cx);
    }

    /// Stop Recording: end the recording and the pick going.
    pub fn stop_recording(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self
            .recording
            .as_ref()
            .and_then(|r| r.id)
            .into_iter()
            .chain(self.picking.as_ref().and_then(|p| p.id))
            .collect();
        self.runs.update(cx, |r, cx| {
            for id in ids {
                r.stop(id, cx);
            }
        });
    }

    /// Open UI Mode: Playwright's own runner, in the system browser.
    pub fn open_ui_mode(&mut self, cx: &mut Context<Self>) {
        if self.pw_tests.read(cx).root.is_none() {
            return self.note("UI Mode works in a Playwright project", cx);
        }
        let model = self.model.clone();
        self.runs.update(cx, |r, cx| {
            r.start_node_tool(
                None,
                record::ui_launch,
                move |_, started, cx| {
                    let text = match started {
                        Ok(_) => "UI Mode opens in your browser".to_owned(),
                        Err(why) => why,
                    };
                    model.update(cx, |m, cx| {
                        m.message = text;
                        cx.notify();
                    });
                },
                cx,
            )
        });
    }
}
