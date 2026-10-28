//! The gutter's running and debugging marks: run icons on test classes and methods and on
//! main classes (a click offers Run and Debug), breakpoints over the line numbers (a click
//! sets or clears one; a right click gives it a condition), and while the debugger is
//! stopped, the line it stopped on and the values of the variables it mentions.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, point, px,
    size,
};
use gpui_component::input::{EditorState, InputEvent, InputState, RopeExt as _};

use super::{EditorPanel, font_size};
use crate::glyphs::{Icon, icon};
use crate::run::{FileRunnables, Target};
use crate::theme::{mix, palette};

/// Where the gutter's lanes are, from the editor's last layout.
#[derive(Clone, Copy, Debug, Default)]
pub struct Lanes {
    pub bounds: Bounds<Pixels>,
    /// Where the code starts (unscrolled).
    pub code_x: Pixels,
}

impl Lanes {
    /// The run icons' lane, left of the line numbers.
    fn run(&self) -> (Pixels, Pixels) {
        (self.bounds.left(), self.bounds.left() + px(13.))
    }

    /// The breakpoints' lane: over the line numbers.
    fn breakpoints(&self) -> (Pixels, Pixels) {
        (self.bounds.left() + px(13.), (self.code_x - px(24.)).max(self.bounds.left() + px(26.)))
    }
}

/// A menu the gutter put up: Run and Debug for a run icon, or a breakpoint's condition.
pub enum GutterMenu {
    Run { at: Point<Pixels>, target: Target },
    Condition { at: Point<Pixels>, line: u32, input: Entity<InputState> },
}

/// The visible buffer lines and the top of each, in window coordinates.
fn visible_lines(state: &EditorState) -> Vec<(usize, Pixels)> {
    let Some(rows) = state.visible_row_range() else { return Vec::new() };
    let text = state.text();
    let lines = text.lines_len();
    let map = state.display_map();
    let first = map.display_row_to_buffer_line(rows.start);
    let last = (map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start)) + 1).min(lines);
    (first..last)
        .filter_map(|line| {
            let at = text.line_start_offset(line);
            state.range_to_bounds(&(at..at)).map(|b| (line, b.top()))
        })
        .collect()
}

/// Where code starts, unscrolled.
fn code_x(state: &EditorState) -> Option<Pixels> {
    let text = state.text();
    let rows = state.visible_row_range()?;
    let line = state.display_map().display_row_to_buffer_line(rows.start).min(text.lines_len().saturating_sub(1));
    let at = text.line_start_offset(line);
    let b = state.range_to_bounds(&(at..at))?;
    Some(b.left() - state.scroll_offset().x.min(px(0.)))
}

/// Files breakpoints can be set in.
fn debuggable(path: &std::path::Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("java" | "scala" | "kt" | "groovy"))
}

impl EditorPanel {
    /// Ask what the file holds to run, for the gutter's icons.
    pub(super) fn refresh_runnables(&mut self, cx: &mut Context<Self>) {
        let ext = self.path.extension().and_then(|e| e.to_str()).unwrap_or_default();
        if !matches!(ext, "java" | "scala") || self.class_file.is_some() {
            self.runnables = None;
            return;
        }
        let Some(runs) = crate::run::runs(cx) else { return };
        let text = self.state.read(cx).value().to_string();
        let task = runs.read(cx).runnables(&self.path, text, cx);
        let path = self.path.clone();
        self._runnables = Some(cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.path == path && found.is_some() {
                    this.runnables = found.map(Rc::new);
                    cx.notify();
                }
            });
        }));
    }

    /// The marks, drawn over the editor from its own line layout.
    pub(super) fn run_gutter(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let marks = crate::run::marks(cx).map(|m| {
            let m = m.read(cx);
            let bps = m.breakpoints.get(&self.path).cloned().unwrap_or_default();
            let exec = m.exec.as_ref().filter(|(path, _)| *path == self.path).map(|(_, line)| *line);
            (bps, exec, if exec.is_some() { m.locals.clone() } else { Vec::new() })
        });
        let (bps, exec, locals) = marks.unwrap_or_default();
        let runnables = self.runnables.clone();
        let (state, lanes) = (self.state.clone(), self.lanes.clone());
        let mono = crate::look::mono_font(cx);
        let size_pt = font_size(cx);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let s = state.read(cx);
                let Some(code_x) = code_x(s) else { return };
                let l = Lanes { bounds, code_x };
                lanes.set(l);
                let Some(line_height) = s.line_height() else { return };
                let visible = visible_lines(s);
                let labels = match exec {
                    Some(line) => inline_values(s, line as usize, &locals),
                    None => Vec::new(),
                };
                let top_of = |line: u32| visible.iter().find(|(l, _)| *l == line as usize).map(|(_, t)| *t);
                // Where the debugger stopped: the line lit across, and the values it sees.
                if let Some(line) = exec
                    && let Some(top) = top_of(line)
                {
                    let band = Bounds::new(
                        point(l.breakpoints().1, top),
                        size(bounds.right() - l.breakpoints().1, line_height),
                    );
                    window.paint_quad(gpui::fill(band, p.accent.opacity(0.22)));
                }
                for (at, label) in labels {
                    let run = gpui::TextRun {
                        len: label.len(),
                        font: gpui::Font { style: gpui::FontStyle::Italic, ..gpui::font(mono.clone()) },
                        color: p.fg_dim,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped = window.text_system().shape_line(label.into(), px(size_pt * 0.92), &[run], None);
                    let _ = shaped.paint(at, line_height, gpui::TextAlign::Left, None, window, cx);
                }
                // Breakpoints, over the line numbers.
                let (bl, br) = l.breakpoints();
                for (line, condition) in &bps {
                    let Some(top) = top_of(*line) else { continue };
                    window.paint_quad(gpui::fill(Bounds::new(point(bl, top), size(br - bl, line_height)), p.panel));
                    let r = px(6.);
                    let c = point((bl + br) / 2., top + line_height / 2.);
                    let dot = Bounds::new(point(c.x - r, c.y - r), size(r * 2., r * 2.));
                    window.paint_quad(gpui::fill(dot, p.error).corner_radii(r));
                    if condition.is_some() {
                        let q = px(2.5);
                        let inner = Bounds::new(point(c.x - q, c.y - q), size(q * 2., q * 2.));
                        window.paint_quad(gpui::fill(inner, gpui::white()).corner_radii(q));
                    }
                }
                // Run icons, and over them the arrow where the debugger stopped.
                let (rl, rr) = l.run();
                if let Some(r) = &runnables {
                    for item in &r.items {
                        let Some(top) = top_of(item.line) else { continue };
                        let b = Bounds::new(point(rl, top + px(1.)), size(rr - rl, line_height - px(2.)));
                        window.paint_quad(gpui::fill(b, p.panel));
                        crate::glyphs::paint(Icon::Play, b, p.ok, window);
                    }
                }
                if let Some(line) = exec
                    && let Some(top) = top_of(line)
                {
                    let b = Bounds::new(point(rl, top + px(1.)), size(rr - rl, line_height - px(2.)));
                    window.paint_quad(gpui::fill(b, p.panel));
                    crate::glyphs::paint(Icon::Play, b, p.accent, window);
                }
            },
        )
        .absolute()
        .inset_0()
    }

    /// A press in the gutter: a run icon's menu, a breakpoint set or cleared, or (right
    /// click) a breakpoint's condition. True when the press was the gutter's.
    pub(super) fn gutter_press(
        &mut self,
        e: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let l = self.lanes.get();
        let x = e.position.x;
        if l.bounds.size.width <= px(0.)
            || x < l.bounds.left()
            || x >= l.breakpoints().1
            || !l.bounds.contains(&e.position)
        {
            return false;
        }
        let Some(line_height) = self.state.read(cx).line_height() else { return false };
        let visible = visible_lines(self.state.read(cx));
        let Some((line, _)) =
            visible.iter().rev().find(|(_, top)| e.position.y >= *top && e.position.y < *top + line_height * 20.)
        else {
            return false;
        };
        let line = *line as u32;
        let in_run_lane = x < l.run().1;
        let runnable =
            self.runnables.as_ref().and_then(|r| r.items.iter().find(|i| i.line == line).map(|i| i.target.clone()));
        if e.button == gpui::MouseButton::Left
            && in_run_lane
            && let Some(target) = runnable
        {
            self.gutter_menu = Some(GutterMenu::Run { at: e.position, target });
            cx.notify();
            return true;
        }
        if !debuggable(&self.path) {
            return false;
        }
        let Some(runs) = crate::run::runs(cx) else { return false };
        let path = self.path.clone();
        match e.button {
            gpui::MouseButton::Left => {
                runs.update(cx, |r, cx| r.toggle_breakpoint(&path, line, cx));
                true
            }
            gpui::MouseButton::Right => {
                let current = runs.read(cx).breakpoints_in(&path).into_iter().find(|b| b.line == line).cloned();
                let input =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Stop only when this is true, as `i > 3`"));
                let text = current.and_then(|b| b.condition).unwrap_or_default();
                input.update(cx, |s, cx| s.set_value(text, window, cx));
                cx.subscribe_in(&input, window, |this, input, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let text = input.read(cx).value().to_string();
                        this.apply_condition(text, window, cx);
                    }
                })
                .detach();
                let focus = input.clone();
                cx.defer_in(window, move |_, window, cx| focus.update(cx, |s, cx| s.focus(window, cx)));
                self.gutter_menu = Some(GutterMenu::Condition { at: e.position, line, input });
                cx.notify();
                true
            }
            _ => false,
        }
    }

    fn apply_condition(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(GutterMenu::Condition { line, .. }) = self.gutter_menu.take() else { return };
        if let Some(runs) = crate::run::runs(cx) {
            let path = self.path.clone();
            runs.update(cx, |r, cx| r.set_condition(&path, line, Some(text), cx));
        }
        self.state.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// The menu the gutter put up, where it was pressed.
    pub(super) fn gutter_popup(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.gutter_menu.as_ref()?;
        let p = palette(cx);
        let hover = p.button_hover;
        let item = |id: &'static str, g: Icon, ink: gpui::Hsla, text: String| {
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(4.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(icon(g, 13., ink))
                .child(text)
        };
        let (at, body) = match menu {
            GutterMenu::Run { at, target } => {
                let module = self.runnables.as_ref().and_then(|r| r.module.clone());
                let (t1, t2, m1, m2) = (target.clone(), target.clone(), module.clone(), module);
                let label = target.label();
                let body = div()
                    .flex()
                    .flex_col()
                    .child(item("gutter-run", Icon::Play, p.ok, format!("Run '{label}'")).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.gutter_menu = None;
                            if let Some(runs) = crate::run::runs(cx) {
                                runs.update(cx, |r, cx| r.run_target(&t1, m1.as_deref(), false, cx));
                            }
                            cx.notify();
                        },
                    )))
                    .child(item("gutter-debug", Icon::Bug, p.ok, format!("Debug '{label}'")).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.gutter_menu = None;
                            if let Some(runs) = crate::run::runs(cx) {
                                runs.update(cx, |r, cx| r.run_target(&t2, m2.as_deref(), true, cx));
                            }
                            cx.notify();
                        },
                    )))
                    .into_any_element();
                (*at, body)
            }
            GutterMenu::Condition { at, line, input } => {
                let line = *line;
                let input2 = input.clone();
                let body = div()
                    .w(px(320.))
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().font_weight(FontWeight::MEDIUM).child(format!("Breakpoint on line {}", line + 1)))
                    .child(crate::look::field(input, false, None, &p))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(crate::look::small_button("gutter-condition-done", "Done", &p).on_click(
                                cx.listener(move |this, _, w, cx| {
                                    let text = input2.read(cx).value().to_string();
                                    this.apply_condition(text, w, cx);
                                }),
                            ))
                            .child(crate::look::small_button("gutter-condition-remove", "Remove", &p).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.gutter_menu = None;
                                    if let Some(runs) = crate::run::runs(cx) {
                                        let path = this.path.clone();
                                        runs.update(cx, |r, cx| r.remove_breakpoint(&path, line, cx));
                                    }
                                    cx.notify();
                                }),
                            )),
                    )
                    .into_any_element();
                (*at, body)
            }
        };
        Some(
            gpui::deferred(
                gpui::anchored().position(at).snap_to_window_with_margin(px(8.)).child(
                    div()
                        .id("gutter-menu")
                        .debug_selector(|| "gutter-menu".into())
                        .occlude()
                        .p(px(4.))
                        .rounded(px(6.))
                        .bg(p.card)
                        .border_1()
                        .border_color(mix(p.border, 0.8, p.card))
                        .shadow_lg()
                        .text_size(px(crate::look::BODY))
                        .text_color(p.fg)
                        .font_family(crate::look::ui_font(cx))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.gutter_menu = None;
                            cx.notify();
                        }))
                        .child(body),
                ),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }
}

/// After the stopped line and the lines above it that mention them, the variables' values,
/// each once, as `name: value`: where each label goes, and its text.
fn inline_values(s: &EditorState, exec: usize, locals: &[(String, String)]) -> Vec<(Point<Pixels>, String)> {
    if locals.is_empty() {
        return Vec::new();
    }
    let rope = s.text();
    let lines = rope.lines_len();
    let first = exec.saturating_sub(30);
    let mut shown: Vec<&str> = Vec::new();
    let mut by_line: std::collections::BTreeMap<usize, Vec<String>> = Default::default();
    for line in (first..=exec.min(lines.saturating_sub(1))).rev() {
        let start = rope.line_start_offset(line);
        let end = rope.line_end_offset(line);
        let text = rope.slice(start..end).to_string();
        let words: Vec<&str> =
            text.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
        for (name, value) in locals {
            if !shown.contains(&name.as_str()) && words.contains(&name.as_str()) {
                shown.push(name);
                let value: String = value.chars().take(60).collect();
                by_line.entry(line).or_default().push(format!("{name}: {value}"));
            }
        }
    }
    by_line
        .into_iter()
        .filter_map(|(line, items)| {
            let end = rope.line_end_offset(line);
            let b = s.range_to_bounds(&(end..end))?;
            Some((point(b.right() + px(24.), b.top()), items.join("   ")))
        })
        .collect()
}

/// Shared cell type for the lanes, so the canvas can write them as it paints.
pub type LanesCell = Rc<Cell<Lanes>>;

/// The runnables of a file, for tests.
pub fn runnables_of(e: &EditorPanel) -> Option<Rc<FileRunnables>> {
    e.runnables.clone()
}
