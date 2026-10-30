//! The Problems window: every problem the engines and the last compile report, grouped by
//! file (each group with its counts, collapsible), with toggles for errors, warnings and
//! information (each with its count) and a filter. A double click (or ↩) goes to a problem;
//! a right click (or ⌥↩) offers its fixes, Fix all in file and Ask the agent to fix. The rows
//! are built once per change of the problems, and only the visible ones are drawn, so
//! hundreds of warnings stay quick.

use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr as _;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, UniformListScrollHandle, Window, canvas, div, point, px,
    uniform_list,
};
use gpui_component::input::InputState;
use lsp_types::{CodeAction, CodeActionOrCommand, Diagnostic, DiagnosticSeverity, Uri};
use serde_json::json;

use super::fixes;
use super::hub::{HubEvent, LangHub};
use crate::components::{IconButton, IconName, RowState, icon};
use crate::editor::context_actions::{self, ActionsList, Entry, Key};
use crate::look;
use crate::theme::{Palette, mix, palette};

/// What the Problems window asks of the workspace.
pub enum ProblemsEvent {
    /// Ask the console's agent to fix a problem: the file, its lines, and the words.
    AskToFix { path: PathBuf, lines: (u32, u32), text: String },
}

/// One problem, ready to draw.
#[derive(Clone)]
pub struct Problem {
    pub uri: Uri,
    /// The build whose engine reported it (none for the last compile's).
    pub root: Option<PathBuf>,
    pub diagnostic: Diagnostic,
    /// 0 an error, 1 a warning, 2 information (and hints).
    pub rank: u8,
    pub text: SharedString,
    pub place: SharedString,
    /// What the filter looks in, in lower case.
    search: String,
}

/// The problems of one file.
#[derive(Clone)]
pub struct Group {
    pub key: String,
    pub name: SharedString,
    pub dir: SharedString,
    /// Its problems (indexes into the window's), worst first, then by line.
    pub problems: Vec<usize>,
    /// Errors, warnings and information.
    pub counts: [usize; 3],
}

/// A row of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Group(usize),
    Problem(usize),
}

/// The problems grouped by file: the files with errors first, then by path.
pub fn group(problems: &[Problem], root: Option<&Path>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut at: std::collections::HashMap<String, usize> = Default::default();
    for (i, p) in problems.iter().enumerate() {
        let key = p.uri.as_str().to_owned();
        let g = *at.entry(key.clone()).or_insert_with(|| {
            let file = short(&p.uri, root);
            let (dir, name) = match file.rsplit_once('/') {
                Some((d, n)) => (d.to_owned(), n.to_owned()),
                None => (String::new(), file),
            };
            groups.push(Group { key, name: name.into(), dir: dir.into(), problems: Vec::new(), counts: [0; 3] });
            groups.len() - 1
        });
        groups[g].problems.push(i);
        groups[g].counts[p.rank.min(2) as usize] += 1;
    }
    for g in &mut groups {
        g.problems.sort_by_key(|&i| {
            let d = &problems[i].diagnostic;
            (problems[i].rank, d.range.start.line, d.range.start.character)
        });
    }
    groups.sort_by(|a, b| {
        let worst = |g: &Group| g.counts.iter().position(|&n| n > 0).unwrap_or(3);
        (worst(a), a.dir.as_ref(), a.name.as_ref()).cmp(&(worst(b), b.dir.as_ref(), b.name.as_ref()))
    });
    groups
}

/// The rows shown: each group with problems that pass the toggles (`show`, by rank) and the
/// filter, then those problems unless the group is collapsed.
pub fn visible_rows(
    problems: &[Problem],
    groups: &[Group],
    show: [bool; 3],
    filter: &str,
    collapsed: &HashSet<String>,
) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let words: Vec<&str> = filter.split_whitespace().collect();
    let mut out = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let shown: Vec<usize> = g
            .problems
            .iter()
            .copied()
            .filter(|&i| {
                let p = &problems[i];
                show[p.rank.min(2) as usize] && words.iter().all(|w| p.search.contains(w))
            })
            .collect();
        if shown.is_empty() {
            continue;
        }
        out.push(Row::Group(gi));
        if !collapsed.contains(&g.key) {
            out.extend(shown.into_iter().map(Row::Problem));
        }
    }
    out
}

/// The path of `uri` from `root` (or from the folder `root` shares with it), or its name.
fn short(uri: &Uri, root: Option<&Path>) -> String {
    match snout_lang::uri::to_path(uri) {
        Some(p) => match root.and_then(|r| r.ancestors().find(|a| a.parent().is_some() && p.starts_with(a))) {
            Some(base) => p.strip_prefix(base).map(|r| r.display().to_string()).unwrap_or_default(),
            None => p.display().to_string(),
        },
        None => uri.as_str().rsplit('/').next().unwrap_or_default().split('?').next().unwrap_or_default().to_owned(),
    }
}

/// A problem as the window keeps it.
pub fn problem(uri: Uri, d: Diagnostic, root: Option<PathBuf>) -> Problem {
    let rank = fixes::rank(&d).min(2);
    let text = context_actions::first_line(&d.message);
    let file = uri.as_str().rsplit('/').next().unwrap_or_default().to_lowercase();
    let source = d.source.clone().unwrap_or_default().to_lowercase();
    let search = format!("{} {file} {source}", text.to_lowercase());
    Problem {
        place: format!("{}:{}", d.range.start.line + 1, d.range.start.character + 1).into(),
        text: text.into(),
        uri,
        root,
        rank,
        search,
        diagnostic: d,
    }
}

/// Every problem the servers report.
pub struct ProblemsPanel {
    hub: Entity<LangHub>,
    focus: FocusHandle,
    pub problems: Vec<Problem>,
    pub groups: Vec<Group>,
    /// The rows shown, rebuilt when the problems, the toggles, the filter or a group change.
    pub rows: Rc<Vec<Row>>,
    pub collapsed: HashSet<String>,
    /// Errors, warnings and information shown.
    pub show: [bool; 3],
    pub filter: Entity<InputState>,
    filter_seen: String,
    /// The folder the files' paths are shown from.
    root: Option<PathBuf>,
    pub selected: Option<Row>,
    /// Where the selected row was drawn last, for ⌥↩.
    selected_at: Rc<Cell<Option<Bounds<Pixels>>>>,
    scroll: UniformListScrollHandle,
    pub actions: Option<ActionsList>,
    /// The problem the context actions are about.
    actions_for: Option<usize>,
    _actions: Option<Task<()>>,
    _fix_all: Option<Task<()>>,
}

impl EventEmitter<ProblemsEvent> for ProblemsPanel {}

impl ProblemsPanel {
    pub fn new(hub: Entity<LangHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&hub, |this, _, cx| this.rebuild(cx)).detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter problems"));
        // The rows follow the filter's text, typed or set.
        cx.observe(&filter, |this, filter, cx| {
            let text = filter.read(cx).value().to_string();
            if text != this.filter_seen {
                this.filter_seen = text;
                this.reflow(cx);
            }
        })
        .detach();
        let mut this = Self {
            hub,
            focus: cx.focus_handle(),
            problems: Vec::new(),
            groups: Vec::new(),
            rows: Rc::new(Vec::new()),
            collapsed: HashSet::new(),
            show: [true; 3],
            filter,
            filter_seen: String::new(),
            root: None,
            selected: None,
            selected_at: Default::default(),
            scroll: UniformListScrollHandle::new(),
            actions: None,
            actions_for: None,
            _actions: None,
            _fix_all: None,
        };
        this.rebuild(cx);
        this
    }

    /// Errors and warnings, as the tab counts them.
    pub fn counts(&self, cx: &App) -> (usize, usize) {
        let all = self.hub.read(cx).all_problems();
        let errors = all.iter().filter(|(_, d)| d.severity == Some(DiagnosticSeverity::ERROR)).count();
        (errors, all.len() - errors)
    }

    /// The problems changed: build the groups again.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let hub = self.hub.read(cx);
        let roots: Vec<PathBuf> = hub.servers.keys().cloned().collect();
        let mut problems: Vec<Problem> = hub
            .all_diagnostics()
            .into_iter()
            .filter_map(|(uri, d, root)| Some(problem(Uri::from_str(&uri).ok()?, d, root)))
            .collect();
        problems.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));
        // The project's folder, else the builds' common one.
        let root = crate::project_model::store(cx)
            .and_then(|s| s.read(cx).root.clone())
            .or_else(|| roots.iter().min_by_key(|r| r.as_os_str().len()).cloned());
        let same = root == self.root
            && problems.len() == self.problems.len()
            && problems.iter().zip(&self.problems).all(|(a, b)| a.uri == b.uri && a.diagnostic == b.diagnostic);
        if same {
            return;
        }
        let selected = self.selected_key();
        self.groups = group(&problems, root.as_deref());
        self.root = root;
        self.problems = problems;
        self.selected = None;
        self.reflow(cx);
        self.select_key(selected, cx);
    }

    /// What the selection is, to find it again after a rebuild.
    fn selected_key(&self) -> Option<(String, Option<Diagnostic>)> {
        match self.selected? {
            Row::Group(g) => Some((self.groups.get(g)?.key.clone(), None)),
            Row::Problem(i) => {
                let p = self.problems.get(i)?;
                Some((p.uri.as_str().to_owned(), Some(p.diagnostic.clone())))
            }
        }
    }

    fn select_key(&mut self, key: Option<(String, Option<Diagnostic>)>, cx: &mut Context<Self>) {
        let Some((uri, d)) = key else { return };
        self.selected = self.rows.iter().copied().find(|r| match (r, &d) {
            (Row::Group(g), None) => self.groups[*g].key == uri,
            (Row::Problem(i), Some(d)) => self.problems[*i].uri.as_str() == uri && self.problems[*i].diagnostic == *d,
            _ => false,
        });
        cx.notify();
    }

    /// The toggles, the filter or a group changed: the rows again.
    fn reflow(&mut self, cx: &mut Context<Self>) {
        let filter = self.filter.read(cx).value().to_string();
        self.rows = Rc::new(visible_rows(&self.problems, &self.groups, self.show, &filter, &self.collapsed));
        if self.selected.is_some_and(|s| !self.rows.contains(&s)) {
            self.selected = None;
        }
        cx.notify();
    }

    /// Show or hide errors (0), warnings (1) or information (2).
    pub fn toggle_severity(&mut self, rank: usize, cx: &mut Context<Self>) {
        self.show[rank] = !self.show[rank];
        self.reflow(cx);
    }

    /// Collapse or expand a file's group.
    pub fn toggle_group(&mut self, g: usize, cx: &mut Context<Self>) {
        let key = self.groups[g].key.clone();
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key);
        }
        self.reflow(cx);
    }

    /// Collapse every group, or expand them all.
    pub fn collapse_all(&mut self, collapse: bool, cx: &mut Context<Self>) {
        self.collapsed = if collapse { self.groups.iter().map(|g| g.key.clone()).collect() } else { HashSet::new() };
        if let Some(Row::Problem(i)) = self.selected {
            self.selected = self.groups.iter().position(|g| g.problems.contains(&i)).map(Row::Group);
        }
        self.reflow(cx);
    }

    /// Go to problem `i`: its file, at it.
    pub fn open(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(p) = self.problems.get(i).cloned() else { return };
        self.hub.update(cx, |h, cx| {
            let root = p.root.clone().or_else(|| h.engine_root_for(&p.uri)).unwrap_or_default();
            cx.emit(HubEvent::Open { root, uri: p.uri, at: p.diagnostic.range.start });
        });
    }

    fn select(&mut self, row: Row, cx: &mut Context<Self>) {
        self.selected = Some(row);
        if let Some(ix) = self.rows.iter().position(|r| *r == row) {
            self.scroll.scroll_to_item(ix, gpui::ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.actions.is_some() {
            return;
        }
        let at = self.selected.and_then(|s| self.rows.iter().position(|r| *r == s));
        let n = self.rows.len();
        if n == 0 || e.keystroke.modifiers.alt || e.keystroke.modifiers.secondary(cx) {
            return;
        }
        match e.keystroke.key.as_str() {
            "down" => self.select(self.rows[at.map_or(0, |a| (a + 1).min(n - 1))], cx),
            "up" => self.select(self.rows[at.map_or(0, |a| a.saturating_sub(1))], cx),
            "left" => match self.selected {
                Some(Row::Group(g)) if !self.collapsed.contains(&self.groups[g].key) => self.toggle_group(g, cx),
                Some(Row::Problem(i)) => {
                    if let Some(g) = self.groups.iter().position(|g| g.problems.contains(&i)) {
                        self.select(Row::Group(g), cx);
                    }
                }
                _ => return,
            },
            "right" => match self.selected {
                Some(Row::Group(g)) if self.collapsed.contains(&self.groups[g].key) => self.toggle_group(g, cx),
                _ => return,
            },
            "enter" => match self.selected {
                Some(Row::Group(g)) => self.toggle_group(g, cx),
                Some(Row::Problem(i)) => self.open(i, cx),
                None => return,
            },
            _ => return,
        }
        let _ = window;
        cx.stop_propagation();
    }

    /// ⌥↩ on the selected problem: its context actions under its row.
    fn actions_at_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Row::Problem(i)) = self.selected else { return };
        let at = self.selected_at.get().map(|b| b.bottom_left() + point(px(24.), px(0.))).unwrap_or_default();
        self.open_actions(i, at, window, cx);
    }

    /// Ask the engine for problem `i`'s code actions (the canned answers without one).
    fn request_actions(&self, uri: &Uri, root: Option<PathBuf>, d: &Diagnostic, cx: &mut App) -> Task<Vec<CodeAction>> {
        if let Some(c) = cx.try_global::<crate::editor::CannedLang>()
            && let Some(path) = snout_lang::uri::to_path(uri)
        {
            return Task::ready((c.actions)(&path, &d.range));
        }
        let hub = self.hub.read(cx);
        let Some(root) = root.or_else(|| hub.engine_root_for(uri)) else { return Task::ready(Vec::new()) };
        let params = json!({
            "textDocument": { "uri": uri.as_str() },
            "range": d.range,
            "context": { "diagnostics": [d], "triggerKind": 1 }
        });
        let call = hub.request(&root, "textDocument/codeAction", params);
        cx.background_executor().spawn(async move {
            match call.await {
                Ok(v) => serde_json::from_value::<Vec<CodeActionOrCommand>>(v)
                    .unwrap_or_default()
                    .into_iter()
                    .map(super::providers::as_action)
                    .collect(),
                Err(e) => {
                    eprintln!("[java] code actions: {e}");
                    Vec::new()
                }
            }
        })
    }

    /// The context actions of problem `i` at `at` (window coordinates).
    pub fn open_actions(&mut self, i: usize, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.problems.get(i).cloned() else { return };
        self.selected = Some(Row::Problem(i));
        let task = self.request_actions(&p.uri, p.root.clone(), &p.diagnostic, cx);
        let file: Vec<Diagnostic> =
            self.problems.iter().filter(|q| q.uri == p.uri).map(|q| q.diagnostic.clone()).collect();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.actions = Some(ActionsList::new(Some(p.diagnostic.clone()), at, focus));
        self.actions_for = Some(i);
        let here = vec![p.diagnostic.clone()];
        self._actions = Some(cx.spawn(async move |this, cx| {
            let actions = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(list) = this.actions.as_mut() {
                    list.fill(context_actions::entries(&file, &here, actions));
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn close_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.actions.take().is_some() {
            self._actions = None;
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// Line `ix` of the context actions, picked.
    pub fn pick_action(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let entry = self.actions.as_ref().and_then(|l| l.entries.get(ix).cloned());
        let p = self.actions_for.and_then(|i| self.problems.get(i).cloned());
        self.close_actions(window, cx);
        let (Some(entry), Some(p)) = (entry, p) else { return };
        match entry {
            Entry::Fix(a) | Entry::EngineFixAll(a) | Entry::Other(a) => self.apply(&p.uri, a, cx),
            Entry::FixAll(all) => {
                let tasks: Vec<Task<Vec<CodeAction>>> =
                    all.targets.iter().map(|d| self.request_actions(&p.uri, p.root.clone(), d, cx)).collect();
                let hub = self.hub.clone();
                self._fix_all = Some(cx.spawn(async move |_, cx| {
                    let answers = futures::future::join_all(tasks).await;
                    let (edits, n) = fixes::fix_all_from(&all, &p.uri, answers);
                    if n > 0 {
                        let edit = fixes::file_edit(&p.uri, edits);
                        hub.update(cx, |_, cx| cx.emit(HubEvent::ApplyEdit { edit, reply: None }));
                    }
                }));
            }
            Entry::Ask(d) => {
                if let Some(path) = snout_lang::uri::to_path(&p.uri) {
                    cx.emit(ProblemsEvent::AskToFix { path, lines: fixes::lines_of(&d), text: fixes::ask_text(&d) });
                }
            }
        }
    }

    /// Apply a code action of a file: its edit as the workspace applies a server's, then its
    /// command.
    fn apply(&mut self, uri: &Uri, action: CodeAction, cx: &mut Context<Self>) {
        let (edit, command) = fixes::edit_of(&action);
        self.hub.update(cx, |h, cx| {
            if let Some(edit) = edit {
                cx.emit(HubEvent::ApplyEdit { edit, reply: None });
            }
            if let Some(command) = command {
                h.execute_command(uri, command, cx);
            }
        });
    }

    fn actions_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self.actions.as_mut() else { return };
        match list.key(e.keystroke.key.as_str()) {
            Key::Ignored => return,
            Key::Moved => cx.notify(),
            Key::Pick(i) => self.pick_action(i, window, cx),
            Key::Close => self.close_actions(window, cx),
        }
        cx.stop_propagation();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mut counts = [0usize; 3];
        for q in &self.problems {
            counts[q.rank.min(2) as usize] += 1;
        }
        let pill = |rank: usize, cx: &mut Context<Self>| {
            let (id, ico, color, what) = match rank {
                0 => ("problems-errors", IconName::CircleX, p.error, "errors"),
                1 => ("problems-warnings", IconName::TriangleAlert, p.warn, "warnings"),
                _ => ("problems-infos", IconName::Info, p.accent, "information"),
            };
            let on = self.show[rank];
            let hover = p.button_hover;
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .flex_none()
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(5.))
                .rounded(px(6.))
                .cursor_pointer()
                .border_1()
                .border_color(if on { mix(color, 0.45, p.border) } else { gpui::transparent_black() })
                .when(on, |d| d.bg(mix(color, 0.12, p.panel)))
                .when(!on, |d| d.hover(move |s| s.bg(hover)))
                .text_color(if on { p.fg } else { p.fg_dim })
                .child(icon(ico, 13., if on { color } else { p.fg_dim }))
                .child(counts[rank].to_string())
                .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(format!("Show {what}")).build(w, cx))
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_severity(rank, cx)))
        };
        let all_collapsed = !self.groups.is_empty() && self.collapsed.len() >= self.groups.len();
        div()
            .flex_none()
            .h(px(34.))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .border_b_1()
            .border_color(mix(p.border, 0.7, p.panel))
            .child(pill(0, cx))
            .child(pill(1, cx))
            .child(pill(2, cx))
            .child(div().w(px(1.)).h(px(16.)).mx(px(2.)).bg(p.border))
            .child(look::field(&self.filter, true, Some(240.), &p))
            .child(div().flex_1())
            .child(
                IconButton::new(
                    "problems-collapse",
                    if all_collapsed { IconName::ChevronsUpDown } else { IconName::ChevronsDownUp },
                    if all_collapsed { "Expand all" } else { "Collapse all" },
                )
                .on_click(cx.listener(move |this, _, _, cx| this.collapse_all(!all_collapsed, cx))),
            )
    }
}

crate::panels::panel!(ProblemsPanel, "Problems", |_, _| "Problems".into(), true);

fn severity_ink(rank: u8, p: &Palette) -> (IconName, gpui::Hsla) {
    match rank {
        0 => (IconName::CircleX, p.error),
        1 => (IconName::TriangleAlert, p.warn),
        _ => (IconName::Info, p.accent),
    }
}

impl Render for ProblemsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let none = self.problems.is_empty();
        let rows = self.rows.clone();
        let (problems, groups) = (Rc::new(self.problems.clone()), Rc::new(self.groups.clone()));
        let collapsed = self.collapsed.clone();
        let selected = self.selected;
        let focused = self.focus.contains_focused(window, cx);
        let at_cell = self.selected_at.clone();
        let this = cx.weak_entity();
        let list = uniform_list("problems-list", rows.len(), move |range, _, cx| {
            let p = palette(cx);
            range
                .map(|ix| {
                    let row = rows[ix];
                    let is_sel = selected == Some(row);
                    let state = RowState::new(is_sel, focused);
                    let base = crate::components::list_row_h(("problem", ix), state, crate::components::TREE_ROW_H, &p)
                        .debug_selector(move || format!("problem-row-{ix}"))
                        .gap(px(7.))
                        .cursor_pointer()
                        .when(is_sel, |d| {
                            let cell = at_cell.clone();
                            d.child(canvas(move |b, _, _| cell.set(Some(b)), |_, _, _, _| {}).absolute().inset_0())
                        });
                    let this = this.clone();
                    match row {
                        Row::Group(g) => {
                            let group = &groups[g];
                            let open = !collapsed.contains(&group.key);
                            let counts = group.counts.iter().enumerate().filter(|(_, n)| **n > 0).map(|(r, n)| {
                                let (ico, color) = severity_ink(r as u8, &p);
                                div()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.))
                                    .text_color(p.fg_dim)
                                    .child(icon(ico, 12., color))
                                    .child(n.to_string())
                            });
                            base.child(icon(
                                if open { IconName::ChevronDown } else { IconName::ChevronRight },
                                12.,
                                p.fg_dim,
                            ))
                            .child(crate::components::file_label(group.name.clone(), &p).flex_none())
                            .child(div().flex_1().min_w_0().h_full().flex().child(crate::components::path_text(
                                ("problem-dir", ix),
                                group.dir.clone(),
                                p.fg_dim,
                            )))
                            .children(counts)
                            .on_click(move |e, _, cx| {
                                let _ = this.update(cx, |t, cx| {
                                    t.selected = Some(row);
                                    if e.click_count() <= 1 {
                                        t.toggle_group(g, cx);
                                    }
                                });
                            })
                        }
                        Row::Problem(i) => {
                            let q = &problems[i];
                            let (ico, color) = severity_ink(q.rank, &p);
                            let source = q.diagnostic.source.clone().filter(|s| s != "Java");
                            let this2 = this.clone();
                            base.pl(px(30.))
                                .child(icon(ico, 14., color))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_color(p.fg)
                                        .child(q.text.clone()),
                                )
                                .when_some(source, |d, s| d.child(div().flex_none().text_color(p.fg_dim).child(s)))
                                .child(div().flex_none().w(px(64.)).text_color(p.fg_dim).child(q.place.clone()))
                                .on_click(move |e, _, cx| {
                                    let _ = this.update(cx, |t, cx| {
                                        t.selected = Some(row);
                                        if e.click_count() >= 2 {
                                            t.open(i, cx);
                                        }
                                        cx.notify();
                                    });
                                })
                                .on_mouse_down(MouseButton::Right, move |e, window, cx| {
                                    let at = e.position;
                                    let _ = this2.update(cx, |t, cx| t.open_actions(i, at, window, cx));
                                })
                        }
                    }
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .flex_1()
        .px(px(4.));
        let empty = self.rows.is_empty();
        let actions = self.actions.as_ref().map(|list| {
            let agent = crate::agents::console_agent(cx);
            let this = cx.weak_entity();
            let pick: context_actions::Pick = Rc::new(move |i, window, cx| {
                let _ = this.update(cx, |t, cx| t.pick_action(i, window, cx));
            });
            let card = context_actions::view("problem-actions", list, &agent, None, &p, look::ui_font(cx), pick)
                .track_focus(&list.focus)
                .on_key_down(cx.listener(Self::actions_key))
                .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_actions(window, cx)));
            gpui::deferred(gpui::anchored().position(list.at).snap_to_window_with_margin(px(8.)).child(card))
                .with_priority(2)
        });
        div()
            .id("problems")
            .key_context("ProblemsPanel")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &crate::editor::ShowContextActions, window, cx| {
                this.actions_at_selection(window, cx)
            }))
            .size_full()
            .bg(p.ground)
            .flex()
            .flex_col()
            .text_size(px(12.5))
            .when(!none, |d| d.child(self.toolbar(cx)))
            .when(none, |d| {
                d.child(
                    crate::components::EmptyState::new(snout_core::art::spots::no_problems(), "No problems")
                        .hint("Nothing to fix in the files the Java engine checked.")
                        .compact()
                        .primary_action(
                            "problems-empty-build",
                            crate::components::IconName::Hammer,
                            "Build Project",
                            crate::menus::BuildProject,
                        ),
                )
            })
            .when(!none && empty, |d| {
                d.child(
                    crate::components::EmptyState::new(snout_core::art::spots::no_results(), "No problems match")
                        .hint("Clear the filter, or turn on more of errors, warnings and information above.")
                        .compact(),
                )
            })
            .when(!empty, |d| d.child(list))
            .children(actions)
    }
}
