//! The context actions (⌥↩, ⌘., the gutter's bulb, More actions… of the hover card, and the
//! Problems window's ⌥↩ and right click): the engine's fixes for the problem at hand, the best
//! one first, Fix all in file, Ask the agent to fix it, and the other actions the engine
//! offers there (refactorings, source actions). The list is drawn here for both owners; each
//! keeps it, its keyboard and where it shows.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Div, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use lsp_types::{CodeAction, Diagnostic};

use crate::components::{IconName, icon, kbd};
use crate::lang::fixes::{self, FixAll, Sort};
use crate::theme::{Palette, mix};

/// One line of the list.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// A fix the engine offers for the problem.
    Fix(CodeAction),
    /// The same fix on every problem of its kind in the file.
    FixAll(FixAll),
    /// The engine's own fix of the file (`source.fixAll`).
    EngineFixAll(CodeAction),
    /// Ask the console's agent to fix the problem.
    Ask(Diagnostic),
    /// Another action the engine offers: a refactoring, a source action.
    Other(CodeAction),
}

impl Entry {
    /// What the line says; `agent` is the console agent's short name.
    pub fn label(&self, agent: &str) -> String {
        match self {
            Entry::Fix(a) | Entry::EngineFixAll(a) | Entry::Other(a) => a.title.clone(),
            Entry::FixAll(f) => f.label.clone(),
            Entry::Ask(_) => format!("Ask {agent} to fix"),
        }
    }
}

/// The lines for the actions the engine offered at a place, where `here` are the problems
/// there (the first the one the list is about) and `problems` every problem of the file: the
/// fixes, the best first; Fix all; Ask the agent; then the rest.
pub fn entries(problems: &[Diagnostic], here: &[Diagnostic], actions: Vec<CodeAction>) -> Vec<Entry> {
    let actions = fixes::usable(actions);
    let best = fixes::best_fix(&actions).cloned();
    let mut out: Vec<Entry> = Vec::new();
    if let Some(b) = &best {
        out.push(Entry::Fix(b.clone()));
    }
    for a in &actions {
        if fixes::sort_of(a) == Sort::Fix && Some(a) != best.as_ref() {
            out.push(Entry::Fix(a.clone()));
        }
    }
    for a in &actions {
        if fixes::sort_of(a) == Sort::FixAll {
            out.push(Entry::EngineFixAll(a.clone()));
        }
    }
    // Fix all for the best fix: the one that problem's kind takes (the others of a problem,
    // such as organizing the imports, are no fix of each problem).
    if let Some(b) = &best {
        let alike: Vec<CodeAction> = actions
            .iter()
            .filter(|a| fixes::sort_of(a) == Sort::Fix && a.diagnostics == b.diagnostics)
            .cloned()
            .collect();
        let offers = fixes::fix_all_offers(problems, here, &alike);
        out.extend(offers.into_iter().filter(|f| f.title == b.title).map(Entry::FixAll));
    }
    if let Some(problem) = here.first() {
        out.push(Entry::Ask(problem.clone()));
    }
    out.extend(actions.into_iter().filter(|a| fixes::sort_of(a) == Sort::Other).map(Entry::Other));
    out
}

/// What a key does to the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Moved,
    Pick(usize),
    Close,
    Ignored,
}

/// The list while it shows.
pub struct ActionsList {
    /// The problem it is about, for its heading.
    pub problem: Option<Diagnostic>,
    pub entries: Vec<Entry>,
    /// The engine has yet to answer.
    pub loading: bool,
    pub selected: usize,
    /// Where it shows (window coordinates of its top left).
    pub at: Point<Pixels>,
    pub focus: FocusHandle,
    pub scroll: ScrollHandle,
}

impl ActionsList {
    pub fn new(problem: Option<Diagnostic>, at: Point<Pixels>, focus: FocusHandle) -> Self {
        let entries = problem.iter().cloned().map(Entry::Ask).collect();
        Self { problem, entries, loading: true, selected: 0, at, focus, scroll: ScrollHandle::new() }
    }

    /// The engine answered: `entries` in place of the placeholder.
    pub fn fill(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
        self.loading = false;
        self.selected = 0;
    }

    /// Up, down, Return and Esc.
    pub fn key(&mut self, key: &str) -> Key {
        let n = self.entries.len();
        match key {
            "up" if n > 0 => self.selected = (self.selected + n - 1) % n,
            "down" if n > 0 => self.selected = (self.selected + 1) % n,
            "enter" if n > 0 => return Key::Pick(self.selected.min(n - 1)),
            "escape" => return Key::Close,
            _ => return Key::Ignored,
        }
        self.scroll.scroll_to_item(self.selected + 1);
        Key::Moved
    }
}

/// What a click on a line does: its index.
pub type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The colour of a problem's severity.
pub fn severity_color(d: &Diagnostic, p: &Palette) -> gpui::Hsla {
    match fixes::rank(d) {
        0 => p.error,
        1 => p.warn,
        _ => p.accent,
    }
}

/// The icon of a problem's severity.
pub fn severity_icon(d: &Diagnostic) -> IconName {
    match fixes::rank(d) {
        0 => IconName::CircleX,
        1 => IconName::TriangleAlert,
        _ => IconName::Info,
    }
}

/// The list drawn as a card: its heading (the problem, or Context Actions), then a line per
/// entry with its icon, the first fix with ⌥⇧↩. `id` names the rows (`{id}-row-{i}`); a click
/// on a row calls `pick` with its index.
pub fn view(
    id: &'static str,
    list: &ActionsList,
    agent: &crate::agents::Agent,
    top_fix_keys: Option<String>,
    p: &Palette,
    font: gpui::SharedString,
    pick: Pick,
) -> gpui::Stateful<Div> {
    let heading = match &list.problem {
        Some(d) => {
            let color = severity_color(d, p);
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .pt(px(4.))
                .pb(px(6.))
                .child(
                    div()
                        .flex_none()
                        .size(px(20.))
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(mix(color, 0.18, p.card))
                        .child(icon(severity_icon(d), 12., color)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .max_w(px(420.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(p.fg)
                        .child(first_line(&d.message)),
                )
        }
        None => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .pt(px(2.))
            .pb(px(4.))
            .text_color(p.weak())
            .child(icon(IconName::Lightbulb, 13., p.fg_dim))
            .child("Context Actions"),
    };
    let mut rows: Vec<AnyElement> = Vec::new();
    let mut first_fix = true;
    let mut others = false;
    for (i, e) in list.entries.iter().enumerate() {
        if matches!(e, Entry::Other(_)) && !others {
            others = true;
            if i > 0 {
                rows.push(div().mx(px(6.)).my(px(3.)).h(px(1.)).bg(p.border).into_any_element());
            }
        }
        let (ico, ink) = match e {
            Entry::Fix(_) => {
                let c = list.problem.as_ref().map_or(p.warn, |d| severity_color(d, p));
                (IconName::Lightbulb, c)
            }
            Entry::FixAll(_) | Entry::EngineFixAll(_) => (IconName::ListChecks, p.ok),
            Entry::Ask(_) => (agent.icon, agent.color(p)),
            Entry::Other(a) => {
                let kind = a.kind.as_ref().map(|k| k.as_str()).unwrap_or_default();
                let ico = if kind.starts_with("refactor") { IconName::Wrench } else { IconName::FileCode };
                (ico, p.fg_dim)
            }
        };
        let keys = match e {
            Entry::Fix(_) if first_fix => {
                first_fix = false;
                top_fix_keys.clone()
            }
            _ => None,
        };
        let selected = i == list.selected;
        let pick = pick.clone();
        rows.push(
            div()
                .id((id, i))
                .debug_selector(move || format!("{id}-row-{i}"))
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(5.))
                .cursor_pointer()
                .when(selected, |d| d.bg(p.row_selected()))
                .when(!selected, |d| d.hover(|s| s.bg(p.row_hover())))
                .child(icon(ico, 14., ink))
                .child(div().flex_1().min_w_0().whitespace_nowrap().child(e.label(&agent.short)))
                .when_some(keys, |d, k| d.child(kbd(k, p.fg_dim)))
                .on_click(move |_, w, cx| pick(i, w, cx))
                .into_any_element(),
        );
    }
    if list.loading {
        rows.push(
            div()
                .id((id, usize::MAX))
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_color(p.fg_dim)
                .child(icon(IconName::LoaderCircle, 14., p.fg_dim))
                .child("Looking for fixes\u{2026}")
                .into_any_element(),
        );
    }
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .occlude()
        .min_w(px(280.))
        .max_w(px(560.))
        .p(px(4.))
        .flex()
        .flex_col()
        .rounded(px(8.))
        .bg(p.card)
        .border_1()
        .border_color(p.border)
        .shadow_lg()
        .font_family(font)
        .text_size(px(12.5))
        .text_color(p.fg)
        .child(heading)
        .child(
            div()
                .id((id, usize::MAX - 1))
                .max_h(px(340.))
                .overflow_y_scroll()
                .track_scroll(&list.scroll)
                .flex()
                .flex_col()
                .children(rows),
        )
}

/// The first line of a message, for a heading.
pub fn first_line(message: &str) -> String {
    message.lines().next().unwrap_or_default().trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{CodeActionKind, Position, Range};

    #[test]
    fn lists_the_best_fix_first_then_fix_all_ask_and_the_rest() {
        let d = |line: u32| Diagnostic {
            range: Range::new(Position::new(line, 0), Position::new(line, 5)),
            code: Some(lsp_types::NumberOrString::Number(7)),
            message: "The import a.B is never used".into(),
            ..Default::default()
        };
        let fix = |t: &str, preferred: bool| CodeAction {
            title: t.into(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![d(1)]),
            is_preferred: Some(preferred),
            ..Default::default()
        };
        let other = CodeAction {
            title: "Generate toString()".into(),
            kind: Some(CodeActionKind::SOURCE),
            ..Default::default()
        };
        let list = entries(
            &[d(1), d(2)],
            &[d(1)],
            vec![other.clone(), fix("Organize imports", false), fix("Remove unused import", true)],
        );
        let labels: Vec<String> = list.iter().map(|e| e.label("Claude")).collect();
        assert_eq!(
            labels,
            [
                "Remove unused import",
                "Organize imports",
                "Fix all \u{2018}Remove unused import\u{2019} in file",
                "Ask Claude to fix",
                "Generate toString()",
            ]
        );
        // No problem at hand: no Ask.
        assert!(!entries(&[], &[], vec![other]).iter().any(|e| matches!(e, Entry::Ask(_))));
    }
}
