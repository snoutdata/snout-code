//! The debugger's dialogs: View Breakpoints (⇧⌘F8), every breakpoint of the project with
//! its tick box (enabled or not), its condition and Remove; and Evaluate Expression (⌥F8),
//! an expression evaluated in the frame the debugger stopped in.

use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;

use super::Runs;
use super::config::Breakpoint;
use crate::components::{self, Button, IconButton, IconName, RowState, Tone};
use crate::look;
use crate::theme::palette;

pub enum DebugDialogEvent {
    Close,
    /// Show this file at this line (from 0).
    Open(PathBuf, u32),
}

/// The breakpoints in the order the dialog lists them: by file, then line.
pub fn sorted(list: &[Breakpoint]) -> Vec<Breakpoint> {
    let mut out = list.to_vec();
    out.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    out
}

// ----- View Breakpoints ---------------------------------------------------------------------

pub struct BreakpointsDialog {
    runs: Entity<Runs>,
    root: PathBuf,
    pub selected: usize,
    pub condition: Entity<InputState>,
    focus: FocusHandle,
}

impl EventEmitter<DebugDialogEvent> for BreakpointsDialog {}

impl Focusable for BreakpointsDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl BreakpointsDialog {
    pub fn new(runs: Entity<Runs>, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let condition = cx.new(|cx| InputState::new(window, cx).placeholder("Stop only when this is true, as `i > 3`"));
        cx.subscribe(&condition, |this, _, e: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = e {
                this.apply_condition(cx);
            }
        })
        .detach();
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        let mut this = Self { runs, root, selected: 0, condition, focus: cx.focus_handle() };
        this.load_condition(window, cx);
        this
    }

    pub fn list(&self, cx: &gpui::App) -> Vec<Breakpoint> {
        sorted(&self.runs.read(cx).project.breakpoints)
    }

    fn load_condition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.list(cx).get(self.selected).and_then(|b| b.condition.clone()).unwrap_or_default();
        self.condition.update(cx, |s, cx| s.set_value(text, window, cx));
    }

    pub fn pick(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = ix;
        self.load_condition(window, cx);
        cx.notify();
    }

    /// The condition field's text becomes the selected breakpoint's condition.
    pub fn apply_condition(&mut self, cx: &mut Context<Self>) {
        let Some(b) = self.list(cx).get(self.selected).cloned() else { return };
        let text = self.condition.read(cx).value().to_string();
        self.runs.update(cx, |r, cx| r.set_condition(&b.path, b.line, Some(text), cx));
    }

    pub fn set_enabled(&mut self, b: &Breakpoint, on: bool, cx: &mut Context<Self>) {
        self.runs.update(cx, |r, cx| r.set_breakpoint_enabled(&b.path, b.line, on, cx));
    }

    pub fn remove(&mut self, b: &Breakpoint, window: &mut Window, cx: &mut Context<Self>) {
        self.runs.update(cx, |r, cx| r.remove_breakpoint(&b.path, b.line, cx));
        let n = self.list(cx).len();
        self.selected = self.selected.min(n.saturating_sub(1));
        self.load_condition(window, cx);
    }

    pub fn remove_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.runs.update(cx, |r, cx| r.remove_all_breakpoints(cx));
        self.selected = 0;
        self.load_condition(window, cx);
    }

    fn go_to(&mut self, cx: &mut Context<Self>) {
        if let Some(b) = self.list(cx).get(self.selected) {
            cx.emit(DebugDialogEvent::Open(b.path.clone(), b.line));
        }
    }

    fn relative<'a>(&self, path: &'a Path) -> std::borrow::Cow<'a, str> {
        path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy()
    }
}

impl Render for BreakpointsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let list = self.list(cx);
        let view = cx.entity();
        let rows: Vec<AnyElement> = list
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let rel = self.relative(&b.path).into_owned();
                let (dir, name) = match rel.rsplit_once('/') {
                    Some((d, n)) => (d.to_owned(), n.to_owned()),
                    None => (String::new(), rel.clone()),
                };
                let (b1, b2) = (b.clone(), b.clone());
                let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                let on = b.enabled;
                components::list_row(("bp-row", i), RowState::new(i == self.selected, true), &p)
                    .debug_selector(move || format!("bp-row-{i}"))
                    .cursor_pointer()
                    .child(
                        look::checkbox(("bp-enabled", i), "", on, true, &p)
                            .debug_selector(move || format!("bp-enabled-{i}"))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                v1.update(cx, |d, cx| d.set_enabled(&b1, !on, cx))
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .size(px(10.))
                            .rounded_full()
                            .when(on, |d| d.bg(p.error))
                            .when(!on, |d| d.border_2().border_color(crate::theme::mix(p.error, 0.55, p.card))),
                    )
                    .child(crate::icons::file_icon(&b.path, 16., &p))
                    .child(
                        div()
                            .flex_none()
                            .text_color(if on { p.fg } else { p.fg_dim })
                            .child(format!("{name}:{}", b.line + 1)),
                    )
                    .child(components::path_text(("bp-dir", i), dir, p.fg_dim))
                    .when_some(b.condition.clone(), |d, c| {
                        d.child(components::badge(format!("if {c}"), Tone::Warning, &p).font_family(look::MONO))
                    })
                    .child(IconButton::new(format!("bp-remove-{i}"), IconName::Trash, "Remove").size(22.).on_click(
                        move |_, w, cx| {
                            cx.stop_propagation();
                            v2.update(cx, |d, cx| d.remove(&b2, w, cx))
                        },
                    ))
                    .on_click(move |e, w, cx| {
                        v3.update(cx, |d, cx| {
                            d.pick(i, w, cx);
                            if e.click_count() > 1 {
                                d.go_to(cx);
                            }
                        })
                    })
                    .into_any_element()
            })
            .collect();
        let empty = list.is_empty();
        let selected = list.get(self.selected).cloned();
        let body = if empty {
            div()
                .flex_1()
                .child(
                    components::EmptyState::new(snout_core::art::spots::no_results(), "No breakpoints")
                        .hint(crate::keymap::shown(
                            "Click left of a line number, or press ⌘F8 in an editor, to set one.",
                        ))
                        .on(p.card)
                        .compact(),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(div().id("bp-list").flex_1().min_h_0().px(px(8.)).overflow_y_scrollbar().children(rows))
                .when_some(selected, |d, b| {
                    d.child(
                        div()
                            .flex_none()
                            .px(px(20.))
                            .py(px(10.))
                            .border_t_1()
                            .border_color(p.border)
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(components::field_label(
                                format!("Condition of {}:{}", self.relative(&b.path), b.line + 1),
                                &p,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .child(div().flex_1().child(look::field(&self.condition, false, None, &p)))
                                    .child(
                                        Button::new("bp-apply", crate::components::APPLY_ICON)
                                            .label("Apply")
                                            .compact()
                                            .on_click(cx.listener(|this, _, _, cx| this.apply_condition(cx))),
                                    ),
                            ),
                    )
                })
                .into_any_element()
        };
        let count = list.len();
        let off = list.iter().filter(|b| !b.enabled).count();
        let status = match (count, off) {
            (0, _) => String::new(),
            (n, 0) => format!("{n} {}", if n == 1 { "breakpoint" } else { "breakpoints" }),
            (n, off) => format!("{n} {}, {off} disabled", if n == 1 { "breakpoint" } else { "breakpoints" }),
        };
        div()
            .id("breakpoints-dialog")
            .debug_selector(|| "breakpoints-dialog".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(DebugDialogEvent::Close)))
            .occlude()
            .w(px(720.))
            .h(px(500.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .font_family(look::ui_font(cx))
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(components::dialog_header_icon(
                Tone::Danger,
                IconName::CircleDot,
                "Breakpoints",
                Some("Untick one to keep it without stopping there. A double click shows its line.".into()),
                &p,
            ))
            .child(body)
            .child(components::dialog_footer(
                Some((status, p.fg_dim)),
                vec![
                    Button::new("bp-remove-all", IconName::Trash)
                        .label("Remove All")
                        .disabled(empty)
                        .on_click(cx.listener(|this, _, w, cx| this.remove_all(w, cx)))
                        .into_any_element(),
                    Button::new("bp-close", crate::components::CLOSE_ICON)
                        .label("Close")
                        .kbd("Esc")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DebugDialogEvent::Close)))
                        .into_any_element(),
                    Button::new("bp-go-to", IconName::CornerDownRight)
                        .label("Go to Line")
                        .primary()
                        .disabled(empty)
                        .on_click(cx.listener(|this, _, _, cx| this.go_to(cx)))
                        .into_any_element(),
                ],
                &p,
            ))
    }
}

// ----- Evaluate Expression ------------------------------------------------------------------

pub struct EvaluateDialog {
    runs: Entity<Runs>,
    pub input: Entity<InputState>,
    focus: FocusHandle,
}

impl EventEmitter<DebugDialogEvent> for EvaluateDialog {}

impl Focusable for EvaluateDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EvaluateDialog {
    pub fn new(runs: Entity<Runs>, initial: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("An expression, as `total * 2`"));
        input.update(cx, |s, cx| s.set_value(initial.clone(), window, cx));
        cx.subscribe(&input, |this, _, e: &InputEvent, cx| {
            if let InputEvent::PressEnter { secondary, .. } = e {
                if *secondary { this.add_watch(cx) } else { this.evaluate(cx) }
            }
        })
        .detach();
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        let this = Self { runs, input, focus: cx.focus_handle() };
        if !initial.trim().is_empty() {
            let this2 = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = this2.update(cx, |d, cx| d.evaluate(cx));
            });
        }
        this
    }

    pub fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |s, cx| s.focus(window, cx));
    }

    pub fn evaluate(&mut self, cx: &mut Context<Self>) {
        let expr = self.input.read(cx).value().trim().to_owned();
        if expr.is_empty() {
            return;
        }
        let Some(id) = self.runs.read(cx).paused_debug() else { return };
        self.runs.update(cx, |r, cx| r.evaluate(id, expr, cx));
    }

    pub fn add_watch(&mut self, cx: &mut Context<Self>) {
        let expr = self.input.read(cx).value().to_string();
        let Some(id) = self.runs.read(cx).paused_debug() else { return };
        self.runs.update(cx, |r, cx| r.add_watch(id, expr, cx));
    }
}

impl Render for EvaluateDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let runs = self.runs.read(cx);
        let paused = runs.paused_debug();
        let evaluated =
            paused.and_then(|id| runs.session(id)).and_then(|s| s.debug.as_ref()).and_then(|d| d.evaluated.clone());
        let result: AnyElement = match (&paused, evaluated) {
            (None, _) => components::callout(
                Tone::Info,
                crate::keymap::shown(
                    "Evaluate works while the program is stopped at a breakpoint: debug it (⌃D) and set one (⌘F8).",
                ),
                &p,
            )
            .into_any_element(),
            (Some(_), None) => {
                look::weak("Press ⏎ to evaluate it in the frame the debugger stopped in.", &p).into_any_element()
            }
            (Some(_), Some((expr, Err(e)))) => {
                components::callout_with(Tone::Danger, Some(expr.into()), e, &p).into_any_element()
            }
            (Some(_), Some((expr, Ok(v)))) => div()
                .id("evaluate-result")
                .debug_selector(|| "evaluate-result".into())
                .p(px(10.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .bg(p.panel)
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().text_size(px(12.)).text_color(p.fg_dim).child(expr))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .font_family(look::MONO)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.fg)
                                .child(v.value.clone()),
                        )
                        .when_some(v.ty.clone(), |d, ty| d.child(components::badge(ty, Tone::Neutral, &p))),
                )
                .when(v.reference > 0, |d| {
                    d.child(div().text_size(px(12.)).text_color(p.fg_dim).child(crate::keymap::shown(
                        "Add it to the watches (⌘⏎) to open its fields in the Debug window.",
                    )))
                })
                .into_any_element(),
        };
        let live = paused.is_some();
        div()
            .id("evaluate-dialog")
            .debug_selector(|| "evaluate-dialog".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(DebugDialogEvent::Close)))
            .occlude()
            .w(px(560.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .font_family(look::ui_font(cx))
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(components::dialog_header_icon(
                Tone::Accent,
                IconName::SquareFunction,
                "Evaluate Expression",
                Some("Java, in the frame the debugger stopped in.".into()),
                &p,
            ))
            .child(
                components::dialog_body()
                    .child(look::field(&self.input, false, None, &p).font_family(look::MONO))
                    .child(result),
            )
            .child(components::dialog_footer(
                None,
                vec![
                    Button::new("evaluate-watch", IconName::Eye)
                        .label("Add to Watches")
                        .kbd(crate::keymap::shown("⌘⏎"))
                        .disabled(!live)
                        .on_click(cx.listener(|this, _, _, cx| this.add_watch(cx)))
                        .into_any_element(),
                    Button::new("evaluate-close", crate::components::CLOSE_ICON)
                        .label("Close")
                        .kbd("Esc")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DebugDialogEvent::Close)))
                        .into_any_element(),
                    Button::new("evaluate-run", IconName::Play)
                        .label("Evaluate")
                        .kbd("⏎")
                        .primary()
                        .disabled(!live)
                        .on_click(cx.listener(|this, _, _, cx| this.evaluate(cx)))
                        .into_any_element(),
                ],
                &p,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_by_file_then_line() {
        let list = [
            Breakpoint::new("/w/B.java", 2, None),
            Breakpoint::new("/w/A.java", 9, None),
            Breakpoint::new("/w/A.java", 3, Some("i > 1".into())),
        ];
        let lines: Vec<(String, u32)> =
            sorted(&list).into_iter().map(|b| (b.path.to_string_lossy().into_owned(), b.line)).collect();
        assert_eq!(lines, [("/w/A.java".into(), 3), ("/w/A.java".into(), 9), ("/w/B.java".into(), 2)]);
    }
}
