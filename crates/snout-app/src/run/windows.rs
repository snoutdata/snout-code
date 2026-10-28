//! The Run, Debug and Build tool windows: one tab per session, a toolbar (Rerun, Stop, and
//! for tests Rerun Failed; the debugger's steps in Debug), and what the session shows: its
//! console with colours and links, its test results, its Gradle tasks, or the debugger's
//! threads, frames, variables and watches.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollStrategy, SharedString, StatefulInteractiveElement as _,
    Styled as _, UniformListScrollHandle, Window, div, px, uniform_list,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::DropdownMenu as _;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::tooltip::Tooltip;

use super::ansi::{self, Color};
use super::test_tree::TestState;
use super::{ConsoleLine, Outcome, Place, Runs, RunsEvent, Session, Stream};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::theme::{Palette, mix, palette};

/// Which part of the Debug window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugTab {
    Debugger,
    Console,
}

pub struct RunWindow {
    pub runs: Entity<Runs>,
    pub place: Place,
    focus: FocusHandle,
    console: UniformListScrollHandle,
    /// The session and line count the console last showed, to follow its end.
    seen: (u64, usize),
    /// How many lines the shown session's console has (the snapshot leaves them out).
    live_lines: usize,
    pub debug_tab: DebugTab,
    /// The Debug window's expression box.
    pub eval: Option<Entity<InputState>>,
}

impl RunWindow {
    pub fn new(runs: Entity<Runs>, place: Place, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        let eval =
            (place == Place::Debug).then(|| {
                let state = cx.new(|cx| InputState::new(window, cx).placeholder("Evaluate an expression"));
                cx.subscribe(&state, |this, state, e: &InputEvent, cx| {
                    if let InputEvent::PressEnter { secondary, shift } = e {
                        let expr = state.read(cx).value().to_string();
                        let Some(id) = this.runs.read(cx).shown(Place::Debug).map(|s| s.id) else { return };
                        let secondary = *secondary || *shift;
                        this.runs.update(cx, |r, cx| {
                            if secondary { r.add_watch(id, expr, cx) } else { r.evaluate(id, expr, cx) }
                        });
                    }
                })
                .detach();
                state
            });
        Self {
            runs,
            place,
            focus: cx.focus_handle(),
            console: UniformListScrollHandle::new(),
            seen: (0, 0),
            live_lines: 0,
            debug_tab: DebugTab::Debugger,
            eval,
        }
    }

    fn name(&self) -> &'static str {
        match self.place {
            Place::Run => "Run",
            Place::Debug => "Debug",
            Place::Build => "Build",
        }
    }
}

impl gpui::EventEmitter<gpui_component::dock::PanelEvent> for RunWindow {}

impl gpui::Focusable for RunWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl gpui_component::dock::BasePanel for RunWindow {
    fn panel_name(&self) -> &'static str {
        self.name()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl gpui_component::dock::Panel for RunWindow {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.name()
    }

    fn title_bar(&self, _: &App) -> bool {
        true
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }

    fn zoom_control(&self, _: &App) -> Option<gpui_component::dock::PanelControl> {
        None
    }
}

/// A colour of the console's palette.
pub fn ansi_color(c: Color, p: &Palette) -> Hsla {
    match c {
        Color::Indexed(i) if i < 16 => {
            let (h, s, l) = ansi::base16(i, p.dark);
            gpui::hsla(h, s, l, 1.)
        }
        Color::Indexed(i) => {
            let (r, g, b) = ansi::indexed_rgb(i).unwrap_or((128, 128, 128));
            gpui::rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).into()
        }
        Color::Rgb(r, g, b) => gpui::rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).into(),
    }
}

/// One console line: its spans in their colours, the links clickable.
pub fn console_line(line: &ConsoleLine, ix: usize, runs: &Entity<Runs>, p: &Palette) -> AnyElement {
    let base = match line.stream {
        Stream::Out => p.fg,
        Stream::Err => mix(p.error, 0.85, p.fg),
        Stream::System => p.fg_dim,
    };
    let mut row = div().h(px(18.)).flex().items_center().whitespace_nowrap().text_color(base);
    for (k, span) in line.spans.iter().enumerate() {
        let mut fg = span.style.fg.map_or(base, |c| ansi_color(c, p));
        if span.style.dim {
            fg = mix(fg, 0.6, p.ground);
        }
        let text = SharedString::from(span.text.clone());
        let el = div()
            .text_color(fg)
            .when_some(span.style.bg, |d, c| d.bg(ansi_color(c, p)))
            .when(span.style.bold, |d| d.font_weight(FontWeight::BOLD))
            .when(span.style.italic, |d| d.italic())
            .when(span.style.underline, |d| d.underline());
        match &span.link {
            Some(link) => {
                let link = link.clone();
                let runs = runs.clone();
                let accent = p.accent;
                row = row.child(
                    el.id(("console-link", ix * 64 + k))
                        .text_color(accent)
                        .underline()
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            let link = link.clone();
                            runs.update(cx, |_, cx| cx.emit(RunsEvent::OpenLink(link)));
                        })
                        .child(text),
                )
            }
            None => row = row.child(el.child(text)),
        }
    }
    row.into_any_element()
}

/// A toolbar button of the run windows.
fn tool(
    id: &'static str,
    g: Icon,
    tip: &'static str,
    enabled: bool,
    ink: Hsla,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let hover = p.button_hover;
    let ink = if enabled { ink } else { mix(p.fg_dim, 0.5, p.ground) };
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .tooltip(move |w, cx| Tooltip::new(tip).build(w, cx))
        .child(icon(g, 15., ink))
}

/// The glyph and colour of a session's state.
fn session_mark(s: &Session, p: &Palette) -> (Icon, Hsla) {
    match &s.ended {
        None if s.debug.as_ref().is_some_and(|d| d.paused) => (Icon::Pause, p.warn),
        None => (if s.place() == Place::Debug { Icon::Bug } else { Icon::Play }, p.ok),
        Some(e) => match e.outcome {
            Outcome::Success => (Icon::Check, p.ok),
            Outcome::Failed => (Icon::Close, p.error),
            Outcome::Cancelled => (Icon::Stop, p.fg_dim),
        },
    }
}

fn state_mark(state: TestState, p: &Palette) -> (Icon, Hsla) {
    match state {
        TestState::Passed => (Icon::Check, p.ok),
        TestState::Failed => (Icon::Close, p.error),
        TestState::Skipped => (Icon::Minus, p.fg_dim),
        TestState::Running => (Icon::Refresh, p.accent),
    }
}

fn millis_text(ms: u64) -> String {
    if ms < 1000 { format!("{ms} ms") } else { format!("{:.1} s", ms as f32 / 1000.) }
}

impl RunWindow {
    /// The sessions' tabs, and the shown one's state at the right.
    fn tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let runs = self.runs.read(cx);
        let shown = runs.shown(self.place).map(|s| s.id);
        let place = self.place;
        let mut row = div()
            .id("run-tabs")
            .flex_none()
            .h(px(30.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(2.))
            .border_b_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .overflow_x_scrollbar();
        for s in runs.sessions_in(place) {
            let id = s.id;
            let on = Some(id) == shown;
            let (g, ink) = session_mark(s, &p);
            let hover = p.button_hover;
            let runs_entity = self.runs.clone();
            let closer = self.runs.clone();
            row = row.child(
                div()
                    .id(("run-tab", id as usize))
                    .debug_selector(move || format!("run-tab-{id}"))
                    .flex_none()
                    .h(px(24.))
                    .pl(px(8.))
                    .pr(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.button))
                    .when(!on, |d| d.text_color(p.fg_dim).hover(move |s| s.bg(hover)))
                    .child(icon(g, 12., ink))
                    .child(div().whitespace_nowrap().child(s.title.clone()))
                    .child(
                        div()
                            .id(("run-tab-close", id as usize))
                            .size(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .hover(move |s| s.bg(hover))
                            .child(icon(Icon::Close, 10., p.fg_dim))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                closer.update(cx, |r, cx| r.close(id, cx));
                            }),
                    )
                    .on_click(move |_, _, cx| runs_entity.update(cx, |r, cx| r.select(place, id, cx))),
            );
        }
        if let Some(s) = runs.shown(place) {
            let status = match s.test_summary() {
                Some(t) => format!("{t}  ·  {}", s.status()),
                None => s.status(),
            };
            row = row.child(div().flex_1()).child(
                div()
                    .id("run-status")
                    .debug_selector(|| "run-status".into())
                    .flex_none()
                    .pr(px(6.))
                    .text_color(p.fg_dim)
                    .text_size(px(12.))
                    .whitespace_nowrap()
                    .child(status),
            );
        }
        row.into_any_element()
    }

    /// Rerun, Stop and the like, down the window's left edge.
    fn side_tools(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let id = s.id;
        let (a, b, c) = (self.runs.clone(), self.runs.clone(), self.runs.clone());
        let failed = s.tests.as_ref().is_some_and(|t| t.counts().1 > 0) && !s.running();
        let ok = p.ok;
        div()
            .flex_none()
            .w(px(34.))
            .h_full()
            .py(px(4.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .border_r_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .child(
                tool("run-rerun", Icon::Rerun, "Rerun", true, ok, &p)
                    .on_click(move |_, _, cx| a.update(cx, |r, cx| r.rerun(id, cx))),
            )
            .child(
                tool("run-stop", Icon::Stop, "Stop", s.running(), p.error, &p)
                    .on_click(move |_, _, cx| b.update(cx, |r, cx| r.stop(id, cx))),
            )
            .when(s.launch.test_module().is_some(), |d| {
                d.child(
                    tool("run-rerun-failed", Icon::RerunFailed, "Rerun Failed Tests", failed, p.error, &p)
                        .on_click(move |_, _, cx| c.update(cx, |r, cx| r.rerun_failed(id, cx))),
                )
            })
            .into_any_element()
    }

    /// The console of `s` (or `lines` in its place), following its end. Rows are read from
    /// the session as they draw, so a long console is never copied.
    fn console(&mut self, s: &Session, lines: Option<Rc<Vec<ConsoleLine>>>, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let count = lines.as_ref().map_or(self.live_lines, |l| l.len());
        let seen = (s.id, count + s.dropped + if lines.is_some() { 1 << 40 } else { 0 });
        if seen != self.seen {
            let follow = self.seen.0 != s.id || self.console.is_scrolled_to_end() != Some(false);
            self.seen = seen;
            if follow && count > 0 && lines.is_none() {
                self.console.scroll_to_item(count - 1, ScrollStrategy::Bottom);
            }
        }
        let runs = self.runs.clone();
        let id = s.id;
        let mono = look::mono_font(cx);
        div()
            .size_full()
            .font_family(mono)
            .text_size(px(12.5))
            .child(
                uniform_list("run-console", count, move |range, _, cx| match &lines {
                    Some(lines) => range.filter_map(|i| Some(console_line(lines.get(i)?, i, &runs, &p))).collect(),
                    None => {
                        let all = runs.read(cx);
                        let Some(s) = all.session(id) else { return Vec::new() };
                        range.filter_map(|i| Some(console_line(s.lines.get(i)?, i, &runs, &p))).collect()
                    }
                })
                .track_scroll(&self.console)
                .with_horizontal_sizing_behavior(gpui::ListHorizontalSizingBehavior::Unconstrained)
                .size_full()
                .px(px(8.))
                .py(px(4.)),
            )
            .into_any_element()
    }

    /// The test results: the tree on the left, the selected test on the right.
    fn tests(&mut self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let tree = s.tests.clone().unwrap_or_default();
        let id = s.id;
        let rows = tree.rows(&s.collapsed);
        let hover = p.button_hover;
        let selected = s.selected_test.clone();
        let mut list = Vec::new();
        // The run itself first: its counts.
        let (g, ink) = session_mark(s, &p);
        let runs = self.runs.clone();
        list.push(
            div()
                .id("test-root")
                .debug_selector(|| "test-root".into())
                .h(px(22.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .when(selected.is_none(), |d| d.bg(p.selection()))
                .hover(move |st| st.bg(hover))
                .child(icon(g, 12., ink))
                .child(div().font_weight(FontWeight::MEDIUM).child("Test Results"))
                .child(div().text_color(p.fg_dim).child(millis_text(s.elapsed().as_millis() as u64)))
                .on_click(move |_, _, cx| {
                    runs.update(cx, |r, cx| {
                        if let Some(s) = r.session_mut(id) {
                            s.selected_test = None;
                        }
                        cx.notify();
                    })
                })
                .into_any_element(),
        );
        for (i, (depth, tid)) in rows.iter().enumerate() {
            let Some(n) = tree.nodes.get(tid) else { continue };
            let state = tree.state_of(tid);
            let (g, ink) = state_mark(state, &p);
            let on = selected.as_deref() == Some(tid.as_str());
            let (runs, runs2) = (self.runs.clone(), self.runs.clone());
            let (t1, t2) = (tid.clone(), tid.clone());
            let open = !s.collapsed.contains(tid);
            let suite = n.suite;
            list.push(
                div()
                    .id(("test-row", i))
                    .debug_selector(move || format!("test-row-{i}"))
                    .h(px(22.))
                    .pl(px(8. + 16. * (*depth as f32 + 1.)))
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.selection()))
                    .hover(move |st| st.bg(hover))
                    .when(suite, |d| {
                        d.child(
                            div()
                                .id(("test-fold", i))
                                .child(icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 11., p.fg_dim))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    runs2.update(cx, |r, cx| {
                                        if let Some(s) = r.session_mut(id)
                                            && !s.collapsed.remove(&t2)
                                        {
                                            s.collapsed.insert(t2.clone());
                                        }
                                        cx.notify();
                                    })
                                }),
                        )
                    })
                    .child(icon(g, 12., ink))
                    .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(n.name.clone()))
                    .children(
                        n.millis
                            .filter(|_| state != TestState::Skipped)
                            .map(|ms| div().flex_none().text_color(p.fg_dim).child(millis_text(ms))),
                    )
                    .on_click(move |_, _, cx| {
                        runs.update(cx, |r, cx| {
                            if let Some(s) = r.session_mut(id) {
                                s.selected_test = Some(t1.clone());
                            }
                            cx.notify();
                        })
                    })
                    .into_any_element(),
            );
        }
        let detail = match selected.as_ref().and_then(|t| tree.nodes.get(t)) {
            Some(n) => self.test_detail(s, n, cx),
            None => self.console(s, None, cx),
        };
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .id("test-tree")
                    .debug_selector(|| "test-tree".into())
                    .flex_none()
                    .w(px(360.))
                    .h_full()
                    .py(px(2.))
                    .border_r_1()
                    .border_color(mix(p.border, 0.6, p.ground))
                    .overflow_y_scrollbar()
                    .children(list),
            )
            .child(div().flex_1().min_w_0().h_full().child(detail))
            .into_any_element()
    }

    /// A test's failure (with the expected and actual values when an assertion says them),
    /// its stack, and what it printed.
    fn test_detail(&mut self, s: &Session, n: &super::test_tree::TestNode, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let mut lines: Vec<ConsoleLine> = Vec::new();
        let mut style = ansi::Style::default();
        let mut add = |stream: Stream, text: &str, lines: &mut Vec<ConsoleLine>| {
            for l in text.lines() {
                lines.push(ConsoleLine { stream, spans: ansi::parse(l, &mut style) });
            }
        };
        let diff = n.diff();
        if n.state == TestState::Failed {
            if let Some(m) = &n.message {
                add(Stream::Err, m, &mut lines);
            }
            if let Some(stack) = &n.stack {
                add(Stream::Err, stack, &mut lines);
            }
        }
        if !n.output.is_empty() {
            lines.push(ConsoleLine::system("Output"));
            add(Stream::Out, &n.output, &mut lines);
        }
        if lines.is_empty() {
            let what = match n.state {
                TestState::Passed => "Passed",
                TestState::Skipped => "Skipped",
                TestState::Running => "Running",
                TestState::Failed => "Failed",
            };
            lines.push(ConsoleLine::system(format!("{what}; nothing printed")));
        }
        let header = diff.map(|(expected, actual)| {
            let pane = |title: &'static str, text: String, color: Hsla| {
                div()
                    .flex_1()
                    .min_w_0()
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(p.wash(color))
                    .border_1()
                    .border_color(mix(color, 0.4, p.panel))
                    .child(div().text_size(px(11.)).text_color(p.fg_dim).child(title))
                    .child(div().font_family(look::mono_font(cx)).text_size(px(12.5)).child(text))
            };
            div()
                .id("test-diff")
                .debug_selector(|| "test-diff".into())
                .flex_none()
                .p(px(8.))
                .flex()
                .gap(px(8.))
                .child(pane("Expected", expected, p.ok))
                .child(pane("Actual", actual, p.error))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(header)
            .child(div().flex_1().min_h_0().child(self.console(s, Some(Rc::new(lines)), cx)))
            .into_any_element()
    }

    /// The Build window's tree: the run, then its tasks.
    fn build_tree(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let (g, ink) = session_mark(s, &p);
        let mut rows = vec![
            div()
                .h(px(22.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(icon(g, 12., ink))
                .child(div().font_weight(FontWeight::MEDIUM).child(s.title.clone()))
                .child(div().text_color(p.fg_dim).child(millis_text(s.elapsed().as_millis() as u64)))
                .into_any_element(),
        ];
        if s.problems.0 + s.problems.1 > 0 {
            let runs = self.runs.clone();
            let text = match s.problems {
                (e, 0) => format!("{e} {}", if e == 1 { "error" } else { "errors" }),
                (0, w) => format!("{w} {}", if w == 1 { "warning" } else { "warnings" }),
                (e, w) => format!("{e} errors, {w} warnings"),
            };
            rows.push(
                div()
                    .id("build-problems")
                    .debug_selector(|| "build-problems".into())
                    .h(px(22.))
                    .pl(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_color(p.error)
                    .child(icon(Icon::Problems, 12., p.error))
                    .child(div().underline().child(text))
                    .on_click(move |_, _, cx| runs.update(cx, |_, cx| cx.emit(RunsEvent::ShowProblems)))
                    .into_any_element(),
            );
        }
        for t in &s.tasks {
            let (g, ink) = match t.outcome.as_deref() {
                None => (Icon::Refresh, p.accent),
                Some("failed") => (Icon::Close, p.error),
                Some("success") => (Icon::Check, p.ok),
                Some(_) => (Icon::Check, p.fg_dim),
            };
            let note = match t.outcome.as_deref() {
                Some("up-to-date") => Some("up to date"),
                Some("from-cache") => Some("from cache"),
                Some("skipped") => Some("skipped"),
                Some("no-source") => Some("no source"),
                _ => None,
            };
            rows.push(
                div()
                    .h(px(22.))
                    .pl(px(28.))
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(icon(g, 12., ink))
                    .child(div().whitespace_nowrap().child(t.path.clone()))
                    .children(note.map(|n| div().text_color(p.fg_dim).child(n)))
                    .children(t.millis.map(|ms| div().text_color(p.fg_dim).child(millis_text(ms))))
                    .into_any_element(),
            );
        }
        div()
            .id("build-tree")
            .debug_selector(|| "build-tree".into())
            .flex_none()
            .w(px(320.))
            .h_full()
            .py(px(2.))
            .border_r_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .overflow_y_scrollbar()
            .children(rows)
            .into_any_element()
    }

    /// The debugger's toolbar: resume, pause, the steps; rerun and stop.
    fn debug_tools(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let id = s.id;
        let d = s.debug.as_ref();
        let live = d.is_some_and(|d| d.live());
        let paused = live && d.is_some_and(|d| d.paused);
        let step = |name: &'static str, g: Icon, tip: &'static str, command: &'static str, on: bool, ink: Hsla| {
            let runs = self.runs.clone();
            tool(name, g, tip, on, ink, &p).on_click(move |_, _, cx| runs.update(cx, |r, cx| r.step(id, command, cx)))
        };
        let (a, b) = (self.runs.clone(), self.runs.clone());
        let tab = |name: &'static str, which: DebugTab, on: bool, cx: &mut Context<Self>| {
            let hover = p.button_hover;
            div()
                .id(name)
                .debug_selector(move || name.to_owned())
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .cursor_pointer()
                .when(on, |d| d.bg(p.button).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.text_color(p.fg_dim).hover(move |s| s.bg(hover)))
                .child(match which {
                    DebugTab::Debugger => "Debugger",
                    DebugTab::Console => "Console",
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.debug_tab = which;
                    cx.notify();
                }))
        };
        div()
            .flex_none()
            .h(px(32.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(2.))
            .border_b_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .child(tab("debug-tab-debugger", DebugTab::Debugger, self.debug_tab == DebugTab::Debugger, cx))
            .child(tab("debug-tab-console", DebugTab::Console, self.debug_tab == DebugTab::Console, cx))
            .child(div().w(px(10.)))
            .child(
                tool("debug-rerun", Icon::Rerun, "Rerun", true, p.ok, &p)
                    .on_click(move |_, _, cx| a.update(cx, |r, cx| r.rerun(id, cx))),
            )
            .child(step("debug-resume", Icon::Resume, "Resume (F9)", "continue", paused, p.ok))
            .child(step("debug-pause", Icon::Pause, "Pause", "pause", live && !paused, p.fg))
            .child(
                tool("debug-stop", Icon::Stop, "Stop", s.running(), p.error, &p)
                    .on_click(move |_, _, cx| b.update(cx, |r, cx| r.stop(id, cx))),
            )
            .child(div().w(px(10.)))
            .child(step("debug-step-over", Icon::StepOver, "Step Over (F8)", "next", paused, p.accent))
            .child(step("debug-step-into", Icon::StepInto, "Step Into (F7)", "stepIn", paused, p.accent))
            .child(step("debug-step-out", Icon::StepOut, "Step Out (⇧F8)", "stepOut", paused, p.accent))
            .child(div().flex_1())
            .children(
                d.and_then(|d| d.note.clone())
                    .map(|n| div().text_size(px(12.)).text_color(p.fg_dim).whitespace_nowrap().pr(px(6.)).child(n)),
            )
            .into_any_element()
    }

    /// Threads and frames on the left; watches and variables, with the expression box, on
    /// the right.
    fn debugger(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let id = s.id;
        let Some(d) = s.debug.as_ref() else { return div().into_any_element() };
        let hover = p.button_hover;
        let mono = look::mono_font(cx);
        let note = |text: &str| div().p(px(10.)).text_color(p.fg_dim).child(text.to_owned()).into_any_element();
        // Threads and frames.
        let mut frames: Vec<AnyElement> = Vec::new();
        if !d.paused {
            let text = if d.ended {
                "The program ended."
            } else if d.dap.is_none() {
                "Waiting for the program to start…"
            } else {
                "The program is running. Breakpoints stop it here."
            };
            frames.push(note(text));
        } else {
            let thread = d.thread;
            let runs = self.runs.clone();
            let threads = d.threads.clone();
            let name = threads.iter().find(|t| Some(t.0) == thread).map(|t| t.1.clone()).unwrap_or_default();
            let reason = d.reason.clone().unwrap_or_default();
            frames.push(
                gpui_component::button::Button::new("debug-thread")
                    .debug_selector(|| "debug-thread".into())
                    .ghost()
                    .label(format!("{name}  ({reason})"))
                    .dropdown_menu(move |mut menu, _, _| {
                        for (tid, tname) in &threads {
                            let runs = runs.clone();
                            let tid = *tid;
                            menu = menu.item(
                                gpui_component::menu::PopupMenuItem::new(tname.clone())
                                    .checked(Some(tid) == thread)
                                    .on_click(move |_, _, cx| runs.update(cx, |r, cx| r.select_thread(id, tid, cx))),
                            );
                        }
                        menu
                    })
                    .into_any_element(),
            );
            for (i, f) in d.frames.iter().enumerate() {
                let on = i == d.frame;
                let runs = self.runs.clone();
                let place = match (&f.path, &f.source) {
                    (Some(path), _) => format!(
                        "{}:{}",
                        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        f.line
                    ),
                    (None, Some(source)) => format!("{source}:{}", f.line),
                    _ => String::new(),
                };
                let own = f.path.is_some();
                frames.push(
                    div()
                        .id(("debug-frame", i))
                        .debug_selector(move || format!("debug-frame-{i}"))
                        .h(px(22.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(p.selection()))
                        .hover(move |st| st.bg(hover))
                        .text_color(if own { p.fg } else { p.fg_dim })
                        .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(f.name.clone()))
                        .child(div().flex_none().text_color(p.fg_dim).child(place))
                        .on_click(move |_, _, cx| runs.update(cx, |r, cx| r.select_frame(id, i, cx)))
                        .into_any_element(),
                );
            }
        }
        // Watches and variables.
        let mut vars: Vec<AnyElement> = Vec::new();
        if let Some(eval) = &self.eval {
            let runs = self.runs.clone();
            let eval2 = eval.clone();
            vars.push(
                div()
                    .flex_none()
                    .p(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(div().flex_1().child(look::field(eval, false, None, &p)))
                    .child(tool("debug-add-watch", Icon::Eye, "Add to Watches (⇧↩)", d.paused, p.fg, &p).on_click(
                        move |_, _, cx| {
                            let expr = eval2.read(cx).value().to_string();
                            runs.update(cx, |r, cx| r.add_watch(id, expr, cx));
                        },
                    ))
                    .into_any_element(),
            );
        }
        if let Some((expr, result)) = &d.evaluated {
            let (value, color) = match result {
                Ok(v) => (v.value.clone(), p.fg),
                Err(e) => (e.clone(), p.error),
            };
            vars.push(
                div()
                    .id("debug-evaluated")
                    .debug_selector(|| "debug-evaluated".into())
                    .mx(px(6.))
                    .mb(px(4.))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .bg(p.card)
                    .font_family(mono.clone())
                    .text_size(px(12.5))
                    .flex()
                    .gap(px(6.))
                    .child(div().text_color(p.fg_dim).child(format!("{expr} =")))
                    .child(div().text_color(color).child(value))
                    .into_any_element(),
            );
        }
        if d.paused {
            for (i, row) in d.rows().into_iter().enumerate() {
                let runs = self.runs.clone();
                let key = row.key.clone();
                let can_open = row.var.reference > 0;
                let remove = row.watch.map(|w| {
                    let runs = self.runs.clone();
                    div()
                        .id(("watch-remove", i))
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .hover(move |st| st.bg(hover))
                        .child(icon(Icon::Close, 10., p.fg_dim))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            runs.update(cx, |r, cx| r.remove_watch(id, w, cx))
                        })
                });
                let g = if row.watch.is_some() {
                    Icon::Eye
                } else if row.scope {
                    Icon::Frames
                } else {
                    Icon::Variable
                };
                vars.push(
                    div()
                        .id(("debug-var", i))
                        .debug_selector(move || format!("debug-var-{i}"))
                        .h(px(22.))
                        .pl(px(8. + 16. * row.depth as f32))
                        .pr(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .hover(move |st| st.bg(hover))
                        .when(can_open, |d| d.cursor_pointer())
                        .child(div().flex_none().w(px(11.)).when(can_open, |d| {
                            d.child(icon(if row.open { Icon::ChevronDown } else { Icon::ChevronRight }, 11., p.fg_dim))
                        }))
                        .child(icon(g, 12., if row.watch.is_some() { p.accent } else { p.fg_dim }))
                        .child(div().flex_none().whitespace_nowrap().child(row.var.name.clone()))
                        .when(!row.scope, |d| {
                            d.child(div().flex_none().text_color(p.fg_dim).child("="))
                                .child(
                                    div()
                                        .min_w_0()
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .font_family(mono.clone())
                                        .text_size(px(12.5))
                                        .child(row.var.value.clone()),
                                )
                                .children(
                                    row.var
                                        .ty
                                        .clone()
                                        .map(|t| div().flex_none().text_color(p.fg_dim).text_size(px(11.5)).child(t)),
                                )
                        })
                        .child(div().flex_1())
                        .children(remove)
                        .on_click(move |_, _, cx| {
                            if can_open {
                                runs.update(cx, |r, cx| r.toggle_var(id, &key, cx));
                            }
                        })
                        .into_any_element(),
                );
            }
        }
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .id("debug-frames")
                    .debug_selector(|| "debug-frames".into())
                    .flex_none()
                    .w(px(340.))
                    .h_full()
                    .py(px(2.))
                    .border_r_1()
                    .border_color(mix(p.border, 0.6, p.ground))
                    .overflow_y_scrollbar()
                    .children(frames),
            )
            .child(
                div()
                    .id("debug-variables")
                    .debug_selector(|| "debug-variables".into())
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scrollbar()
                    .children(vars),
            )
            .into_any_element()
    }

    fn empty(&self, cx: &App) -> AnyElement {
        let p = palette(cx);
        let (title, text) = match self.place {
            Place::Run => ("Nothing running", "Run a configuration from the title bar, or a test from the gutter."),
            Place::Debug => {
                ("No debug session", "Debug a configuration from the title bar, or a test from the gutter.")
            }
            Place::Build => ("No build output", "Double click a task in the Gradle tool window to run it."),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .child(div().text_color(p.fg).child(title))
            .child(div().text_color(p.fg_dim).text_size(px(12.)).child(text))
            .into_any_element()
    }
}

impl Render for RunWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let base = div()
            .id(match self.place {
                Place::Run => "run-window",
                Place::Debug => "debug-window",
                Place::Build => "build-window",
            })
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_size(px(look::BODY))
            .text_color(p.fg);
        let runs = self.runs.clone();
        let Some(s) = runs.read(cx).shown(self.place) else {
            // Show the newest session when none is picked.
            let newest = runs.read(cx).sessions_in(self.place).last().map(|s| s.id);
            if let Some(id) = newest {
                let place = self.place;
                cx.defer(move |cx| runs.update(cx, |r, cx| r.select(place, id, cx)));
            }
            return base.child(self.empty(cx));
        };
        // Sessions are read from a snapshot: the closures below hold the entity, not it.
        self.live_lines = s.lines.len();
        let snapshot = SessionView::of(s);
        let s = &snapshot.0;
        let tabs = self.tabs(cx);
        let body = match self.place {
            Place::Debug => {
                let tools = self.debug_tools(s, cx);
                let content = match self.debug_tab {
                    DebugTab::Debugger => self.debugger(s, cx),
                    DebugTab::Console => self.console(s, None, cx),
                };
                div().flex_1().min_h_0().flex().flex_col().child(tools).child(div().flex_1().min_h_0().child(content))
            }
            Place::Build => {
                let tools = self.side_tools(s, cx);
                let tree = self.build_tree(s, cx);
                let console = self.console(s, None, cx);
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(tools)
                    .child(tree)
                    .child(div().flex_1().min_w_0().h_full().child(console))
            }
            Place::Run => {
                let tools = self.side_tools(s, cx);
                let content = if s.tests.is_some() { self.tests(s, cx) } else { self.console(s, None, cx) };
                div().flex_1().min_h_0().flex().child(tools).child(div().flex_1().min_w_0().h_full().child(content))
            }
        };
        base.child(tabs).child(body)
    }
}

/// A copy of what a session shows (its console lines left out: the console reads them as
/// it draws), so rendering does not hold the runs borrowed.
struct SessionView(Session);

impl SessionView {
    fn of(s: &Session) -> Self {
        Self(Session {
            id: s.id,
            run_id: s.run_id.clone(),
            title: s.title.clone(),
            launch: s.launch.clone(),
            lines: Vec::new(),
            style: s.style,
            started: s.started,
            ended: s.ended.clone(),
            tasks: s.tasks.clone(),
            tests: s.tests.clone(),
            selected_test: s.selected_test.clone(),
            collapsed: s.collapsed.clone(),
            problems: s.problems,
            debug: s.debug.as_ref().map(super::debug::Debugger::snapshot),
            dropped: s.dropped,
            waiting: s.waiting,
            pid: s.pid,
        })
    }
}
