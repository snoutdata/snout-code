//! The bottom dock's References tab (the last Find References), each row opening its place.
//! The Problems tab is [`super::problems`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, relative, uniform_list,
};
use lsp_types::{DiagnosticSeverity, Location, Position, Uri};

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
                use crate::components::IconName as I;
                let sev = match row.severity {
                    Some(DiagnosticSeverity::ERROR) => Some((I::CircleX, p.error)),
                    Some(DiagnosticSeverity::WARNING) => Some((I::TriangleAlert, p.warn)),
                    Some(_) => Some((I::Info, p.fg_dim)),
                    None => None,
                };
                let (dir, name) = match row.file.rsplit_once('/') {
                    Some((d, n)) => (d.to_owned(), n.to_owned()),
                    None => (String::new(), row.file.to_string()),
                };
                crate::components::list_row_h(
                    (id, i),
                    crate::components::RowState::default(),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || format!("{id}-row-{i}"))
                .gap(px(7.))
                .cursor_pointer()
                .when_some(sev, |d, (g, c)| d.child(crate::components::icon(g, 14., c)))
                .child(crate::components::file_label(name, &p).flex_none())
                .child(div().flex_none().text_color(p.fg_dim).child(row.place.clone()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(p.fg)
                        .when(row.severity.is_none(), |d| d.font_family(look::MONO))
                        .child(row.text.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .w(relative(0.3))
                        .max_w(px(360.))
                        .h_full()
                        .flex()
                        .child(crate::components::path_text(("problem-dir", i), dir, p.fg_dim)),
                )
                .on_click(move |_, _, cx| {
                    let row = row.clone();
                    hub.update(cx, |h, cx| {
                        let root = h.root_of(&row.uri).or_else(|| h.servers.keys().next().cloned()).unwrap_or_default();
                        cx.emit(HubEvent::Open { root, uri: row.uri, at: row.at });
                    });
                })
            })
            .collect()
    })
    .flex_1()
    .px(px(4.))
}

pub use super::problems::{ProblemsEvent, ProblemsPanel};

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
            hub,
            title: crate::keymap::shown("Find References (⇧F12) lists them here").into(),
            rows: Vec::new(),
            focus: cx.focus_handle(),
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
        let base = div().id("references").size_full().bg(p.ground).flex().flex_col().text_size(px(12.5));
        if self.rows.is_empty() {
            let searched = self.title.contains(':');
            let (title, hint) = if searched {
                ("No usages found", self.title.clone())
            } else {
                ("No usages yet", "Find Usages on a symbol lists every place it is used.".into())
            };
            return base.child(
                crate::components::EmptyState::new(snout_core::art::spots::no_usages(), title)
                    .hint(hint)
                    .compact()
                    .primary_action(
                        "references-empty-find",
                        crate::components::IconName::Link,
                        "Find Usages at the Caret",
                        crate::editor::FindReferences,
                    ),
            );
        }
        // What was looked for and how many: a quiet line over the results, not a header.
        base.child(
            div()
                .flex_none()
                .px(px(12.))
                .pt(px(6.))
                .pb(px(4.))
                .text_size(px(11.5))
                .text_color(p.weak())
                .overflow_hidden()
                .whitespace_nowrap()
                .child(self.title.clone()),
        )
        .child(rows_view("reference", self.rows.clone(), self.hub.clone(), cx))
    }
}
