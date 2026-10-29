//! The bottom dock's Problems tab (every error and warning the servers report) and
//! References tab (the last Find References), each row opening its place.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, uniform_list,
};
use lsp_types::{DiagnosticSeverity, Location, Position, Uri};
use std::str::FromStr as _;

use super::hub::{HubEvent, LangHub};
use crate::look;
use crate::theme::palette;

/// A row: where, what, and how bad.
#[derive(Clone)]
struct Row {
    uri: Uri,
    at: Position,
    file: SharedString,
    place: SharedString,
    text: SharedString,
    severity: Option<DiagnosticSeverity>,
}

/// The path of `uri` from `root` (or from the folder `root` shares with it, for a source
/// folder outside the build), or its file name, for a row.
fn short(uri: &Uri, root: Option<&Path>) -> String {
    match snout_lang::uri::to_path(uri) {
        Some(p) => match root.and_then(|r| r.ancestors().find(|a| a.parent().is_some() && p.starts_with(a))) {
            Some(base) => p.strip_prefix(base).map(|r| r.display().to_string()).unwrap_or_default(),
            None => p.display().to_string(),
        },
        None => uri.as_str().rsplit('/').next().unwrap_or_default().split('?').next().unwrap_or_default().to_owned(),
    }
}

fn rows_view(id: &'static str, rows: Vec<Row>, hub: Entity<LangHub>, cx: &mut gpui::App) -> impl IntoElement {
    let p = palette(cx);
    let count = rows.len();
    uniform_list(id, count, move |range, _, _| {
        range
            .map(|i| {
                let row = rows[i].clone();
                let hub = hub.clone();
                let color = match row.severity {
                    Some(DiagnosticSeverity::ERROR) => Some(p.error),
                    Some(DiagnosticSeverity::WARNING) => Some(p.warn),
                    Some(_) => Some(p.fg_dim),
                    None => None,
                };
                div()
                    .id((id, i))
                    .debug_selector(move || format!("{id}-row-{i}"))
                    .w_full()
                    .h(px(24.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(p.button_hover))
                    .when_some(color, |d, c| d.child(div().flex_none().size(px(8.)).rounded_full().bg(c)))
                    .child(div().flex_none().whitespace_nowrap().text_color(p.fg).child(row.file.clone()))
                    .child(div().flex_none().whitespace_nowrap().text_color(p.weak()).child(row.place.clone()))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_color(p.fg_dim)
                            .when(row.severity.is_none(), |d| d.font_family(look::MONO))
                            .child(row.text.clone()),
                    )
                    .on_click(move |_, _, cx| {
                        let row = row.clone();
                        hub.update(cx, |h, cx| {
                            let root =
                                h.root_of(&row.uri).or_else(|| h.servers.keys().next().cloned()).unwrap_or_default();
                            cx.emit(HubEvent::Open { root, uri: row.uri, at: row.at });
                        });
                    })
            })
            .collect()
    })
    .flex_1()
}

/// Every problem the servers report.
pub struct ProblemsPanel {
    hub: Entity<LangHub>,
    focus: FocusHandle,
}

impl ProblemsPanel {
    pub fn new(hub: Entity<LangHub>, cx: &mut Context<Self>) -> Self {
        cx.observe(&hub, |_, _, cx| cx.notify()).detach();
        Self { hub, focus: cx.focus_handle() }
    }

    /// Errors and warnings, as the tab counts them.
    pub fn counts(&self, cx: &gpui::App) -> (usize, usize) {
        let all = self.hub.read(cx).all_problems();
        let errors = all.iter().filter(|(_, d)| d.severity == Some(DiagnosticSeverity::ERROR)).count();
        (errors, all.len() - errors)
    }
}

crate::panels::panel!(ProblemsPanel, "Problems", |_, _| "Problems".into(), true);

impl Render for ProblemsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let hub = self.hub.read(cx);
        let roots: Vec<PathBuf> = hub.servers.keys().cloned().collect();
        let root_of = |path: &Path| roots.iter().filter(|r| path.starts_with(r)).max_by_key(|r| r.as_os_str().len());
        let rows: Vec<Row> = hub
            .all_problems()
            .into_iter()
            .filter_map(|(uri, d)| {
                let uri = Uri::from_str(&uri).ok()?;
                let path = snout_lang::uri::to_path(&uri);
                let root = path.as_deref().and_then(root_of).or(roots.first());
                Some(Row {
                    file: short(&uri, root.map(PathBuf::as_path)).into(),
                    place: format!("{}:{}", d.range.start.line + 1, d.range.start.character + 1).into(),
                    text: d.message.lines().next().unwrap_or_default().to_owned().into(),
                    severity: Some(d.severity.unwrap_or(DiagnosticSeverity::ERROR)),
                    at: d.range.start,
                    uri,
                })
            })
            .collect();
        let (errors, warnings) = self.counts(cx);
        let summary = match (errors, warnings) {
            (0, 0) => "No problems".to_owned(),
            (e, w) => format!("{e} {}, {w} {}", plural(e, "error"), plural(w, "warning")),
        };
        let empty = rows.is_empty();
        div()
            .id("problems")
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .text_size(px(12.5))
            .child(div().flex_none().px(px(10.)).py(px(4.)).text_color(p.weak()).child(summary))
            .when(empty, |d| d.child(look::centered_note("Nothing to fix in the files the Java server checked", &p)))
            .when(!empty, |d| d.child(rows_view("problem", rows, self.hub.clone(), cx)))
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { word.to_owned() } else { format!("{word}s") }
}

/// The last Find References.
pub struct ReferencesPanel {
    hub: Entity<LangHub>,
    title: SharedString,
    rows: Vec<Row>,
    focus: FocusHandle,
}

impl ReferencesPanel {
    pub fn new(hub: Entity<LangHub>, cx: &mut Context<Self>) -> Self {
        Self {
            hub, title: "Find References (⇧F12) lists them here".into(), rows: Vec::new(), focus: cx.focus_handle()
        }
    }

    /// Show `locations`, each with its line of code (read off the UI thread; the rows show
    /// at once, their lines as they come).
    pub fn show(&mut self, title: String, locations: Vec<Location>, cx: &mut Context<Self>) {
        let roots: Vec<PathBuf> = self.hub.read(cx).servers.keys().cloned().collect();
        let mut locations = locations;
        locations.sort_by(|a, b| (a.uri.as_str(), a.range.start.line).cmp(&(b.uri.as_str(), b.range.start.line)));
        let wanted: Vec<(Option<PathBuf>, u32)> =
            locations.iter().map(|l| (snout_lang::uri::to_path(&l.uri), l.range.start.line)).collect();
        self.rows = locations
            .into_iter()
            .map(|l| {
                let path = snout_lang::uri::to_path(&l.uri);
                let root = path.as_deref().and_then(|p| roots.iter().find(|r| p.starts_with(r))).or(roots.first());
                Row {
                    file: short(&l.uri, root.map(PathBuf::as_path)).into(),
                    place: format!("{}:{}", l.range.start.line + 1, l.range.start.character + 1).into(),
                    text: SharedString::default(),
                    severity: None,
                    at: l.range.start,
                    uri: l.uri,
                }
            })
            .collect();
        let n = self.rows.len();
        self.title = format!("{title}: {n} {}", plural(n, "result")).into();
        let host = crate::host::host(cx);
        let lines = cx.background_executor().spawn(async move {
            let mut files: HashMap<PathBuf, Vec<String>> = HashMap::new();
            wanted
                .into_iter()
                .map(|(path, line)| {
                    let Some(p) = path else { return String::new() };
                    let lines = files.entry(p.clone()).or_insert_with(|| {
                        host.read_to_string(&p).unwrap_or_default().lines().map(str::to_owned).collect()
                    });
                    lines.get(line as usize).map(|l| l.trim().to_owned()).unwrap_or_default()
                })
                .collect::<Vec<String>>()
        });
        let shown = self.title.clone();
        cx.spawn(async move |this, cx| {
            let lines = lines.await;
            let _ = this.update(cx, |this, cx| {
                if this.title != shown || this.rows.len() != lines.len() {
                    return;
                }
                for (row, line) in this.rows.iter_mut().zip(lines) {
                    row.text = line.into();
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

crate::panels::panel!(ReferencesPanel, "References", |_, _| "References".into(), true);

impl Render for ReferencesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .id("references")
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .text_size(px(12.5))
            .child(div().flex_none().px(px(10.)).py(px(4.)).text_color(p.weak()).child(self.title.clone()))
            .when(!self.rows.is_empty(), |d| d.child(rows_view("reference", self.rows.clone(), self.hub.clone(), cx)))
    }
}
