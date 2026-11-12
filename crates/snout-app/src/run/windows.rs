//! The Run, Debug and Build tool windows: one tab per session, a toolbar (Rerun, Stop, and
//! for tests Rerun Failed; the debugger's steps in Debug), and what the session shows: its
//! console with colours and links, its test results, its Gradle tasks, or the debugger's
//! threads, frames, variables and watches.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels,
    Point, Render, ScrollStrategy, SharedString, StatefulInteractiveElement as _, Styled as _, UniformListScrollHandle,
    Window, div, px, uniform_list,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::input::{Copy, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
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
    /// The console's own focus, so ⌘C copies what is selected in it.
    console_focus: FocusHandle,
    /// What is selected in the console, and whether the mouse is still selecting.
    sel: Option<ConsoleSel>,
    selecting: bool,
    /// The width of a character of the console's font, as last measured.
    cell_w: Pixels,
    /// The console's longest line, by (session, line count, dropped): the list is as wide as
    /// it, so a long line scrolls sideways.
    widest: ((u64, usize, usize), usize),
}

/// A place in a console: (line, character). Lines count from the session's first, the ones
/// dropped from the top included, so a selection stays on its text as the console trims.
type ConsolePos = (usize, usize);

/// A selection in a session's console, from where the mouse went down to where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConsoleSel {
    session: u64,
    anchor: ConsolePos,
    head: ConsolePos,
}

impl ConsoleSel {
    fn ordered(&self) -> (ConsolePos, ConsolePos) {
        if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) }
    }

    fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The characters of line `line` (of `len`) it covers, the end past the last when it goes on
    /// to the next line.
    fn cols(&self, line: usize, len: usize) -> Option<(usize, usize)> {
        let (a, b) = self.ordered();
        if self.is_empty() || line < a.0 || line > b.0 {
            return None;
        }
        let from = if line == a.0 { a.1.min(len) } else { 0 };
        let to = if line == b.0 { b.1.min(len) } else { len + 1 };
        (to > from).then_some((from, to))
    }
}

/// The text `sel` covers in `lines`, the first of which is line `base`.
fn selected_text(lines: &[ConsoleLine], base: usize, sel: &ConsoleSel) -> String {
    let (a, b) = sel.ordered();
    let mut out = Vec::new();
    for line in a.0..=b.0 {
        let Some(l) = line.checked_sub(base).and_then(|i| lines.get(i)) else { continue };
        let chars: Vec<char> = l.text().chars().collect();
        let from = if line == a.0 { a.1.min(chars.len()) } else { 0 };
        let to = if line == b.0 { b.1.min(chars.len()) } else { chars.len() };
        out.push(chars[from..to.max(from)].iter().collect::<String>());
    }
    out.join("\n")
}

/// The word of `text` at character `col`: letters, digits and `_ . $ -`, as a class name, a
/// path or a number has them. None off a word.
fn word_at(text: &str, col: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || "_.$-".contains(c);
    if !chars.get(col).is_some_and(|c| is_word(*c)) {
        return None;
    }
    let from = (0..col).rev().take_while(|i| is_word(chars[*i])).last().unwrap_or(col);
    let to = (col..chars.len()).take_while(|i| is_word(chars[*i])).last().map_or(col, |i| i + 1);
    Some((from, to))
}

impl RunWindow {
    pub fn new(runs: Entity<Runs>, place: Place, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        let eval =
            (place == Place::Debug).then(|| {
                let state = cx.new(|cx| InputState::new(window, cx).placeholder(ts!("runwin-evaluate-an-expression")));
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
            console_focus: cx.focus_handle(),
            sel: None,
            selecting: false,
            cell_w: px(7.5),
            widest: ((u64::MAX, 0, 0), 0),
        }
    }

    fn name(&self) -> &'static str {
        match self.place {
            Place::Run => ts!("menu-run"),
            Place::Debug => ts!("cmd-debug"),
            Place::Build => ts!("stripes-build"),
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

/// Whether `s` ended in failure: what Fix with the agent is offered on.
fn failed(s: &Session) -> bool {
    s.ended.as_ref().is_some_and(|e| e.outcome == Outcome::Failed)
}

/// A console's context menu: Clear All (a live console only), Copy All, and Save Output to
/// File. `lines` stands in for the session's console, as [`RunWindow::console`] takes it.
fn console_menu(menu: PopupMenu, runs: &Entity<Runs>, id: u64, lines: Option<Rc<Vec<ConsoleLine>>>) -> PopupMenu {
    let text = {
        let (runs, lines) = (runs.clone(), lines.clone());
        move |cx: &App| match &lines {
            Some(lines) => lines.iter().map(|l| l.text() + "\n").collect::<String>(),
            None => runs.read(cx).session(id).map(super::Session::console_text).unwrap_or_default(),
        }
    };
    let menu = if lines.is_none() {
        let runs = runs.clone();
        menu.item(
            PopupMenuItem::new(ts!("runwin-clear-all"))
                .icon(crate::components::menu_icon(crate::components::IconName::Eraser))
                .on_click(move |_, _, cx| runs.update(cx, |r, cx| r.clear_console(id, cx))),
        )
        .separator()
    } else {
        menu
    };
    let copy = text.clone();
    let menu = menu.item(
        PopupMenuItem::new(ts!("runwin-copy-all"))
            .icon(crate::components::menu_icon(crate::components::IconName::Copy))
            .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy(cx)))),
    );
    if lines.is_none() {
        let runs = runs.clone();
        menu.item(
            PopupMenuItem::new(ts!("runwin-save-output-to-file"))
                .icon(crate::components::menu_icon(crate::components::IconName::Save))
                .on_click(move |_, _, cx| runs.update(cx, |r, cx| r.save_console(id, cx))),
        )
    } else {
        menu
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

/// One console line: its spans in their colours, the links clickable, and behind the
/// characters `selected` (from, to) the selection, `cell_w` wide each.
pub fn console_line(
    line: &ConsoleLine,
    ix: usize,
    runs: &Entity<Runs>,
    p: &Palette,
    selected: Option<(usize, usize)>,
    cell_w: Pixels,
) -> AnyElement {
    let base = match line.stream {
        Stream::Out => p.fg,
        Stream::Err => mix(p.error, 0.85, p.fg),
        Stream::System => p.fg_dim,
    };
    let mut row = div().relative().h(px(18.)).flex().items_center().whitespace_nowrap().text_color(base);
    if let Some((from, to)) = selected {
        row = row.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(cell_w * from as f32)
                .w(cell_w * (to - from) as f32)
                .bg(p.selection()),
        );
    }
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
        .tooltip(move |w, cx| Tooltip::new(crate::keymap::shown(tip)).build(w, cx))
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
            .h(px(32.))
            .px(px(8.))
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
            row = row.child(div().flex_1());
            if failed(s) {
                let agent = crate::agents::console_agent(cx);
                let (runs, id) = (self.runs.clone(), s.id);
                row = row.child(
                    crate::components::Button::new("run-fix-with-agent", agent.icon)
                        .label(t!("runwin-fix-with", agent = agent.short.to_string()))
                        .compact()
                        .ghost()
                        .on_click(move |_, _, cx| runs.update(cx, |r, cx| r.fix_with_agent(id, cx))),
                );
            }
            row = row.child(
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
                tool("run-rerun", Icon::Rerun, ts!("runwin-rerun-r"), true, ok, &p)
                    .on_click(move |_, _, cx| a.update(cx, |r, cx| r.rerun(id, cx))),
            )
            .child(
                tool("run-stop", Icon::Stop, ts!("title-stop-f2"), s.running(), p.error, &p)
                    .on_click(move |_, _, cx| b.update(cx, |r, cx| r.stop(id, cx))),
            )
            .when(s.launch.test_module().is_some(), |d| {
                d.child(
                    tool(
                        "run-rerun-failed",
                        Icon::RerunFailed,
                        ts!("runwin-rerun-failed-tests-r"),
                        failed,
                        p.error,
                        &p,
                    )
                    .on_click(move |_, _, cx| c.update(cx, |r, cx| r.rerun_failed(id, cx))),
                )
            })
            .into_any_element()
    }

    /// The console of `s` (or `lines` in its place), following its end. Rows are read from
    /// the session as they draw, so a long console is never copied. The mouse selects in it
    /// (a double click a word, a triple click a line), and ⌘C or the menu copies that.
    fn console(&mut self, s: &Session, lines: Option<Rc<Vec<ConsoleLine>>>, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let count = lines.as_ref().map_or(self.live_lines, |l| l.len());
        // Lines are numbered from the session's first, those trimmed from the top included.
        let base = if lines.is_some() { 0 } else { s.dropped };
        let seen = (s.id, count + s.dropped + if lines.is_some() { 1 << 40 } else { 0 });
        if seen != self.seen {
            let follow = self.seen.0 != s.id || self.console.is_scrolled_to_end() != Some(false);
            self.seen = seen;
            if follow && count > 0 && lines.is_none() {
                self.console.scroll_to_item(count - 1, ScrollStrategy::Bottom);
            }
        }
        let key = (s.id ^ if lines.is_some() { 1 << 63 } else { 0 }, count, base);
        if self.widest.0 != key {
            let len = |l: &ConsoleLine| l.spans.iter().map(|s| s.text.len()).sum::<usize>();
            let widest = match &lines {
                Some(lines) => lines.iter().enumerate().max_by_key(|(_, l)| len(l)).map(|(i, _)| i),
                None => self
                    .runs
                    .read(cx)
                    .session(s.id)
                    .and_then(|s| s.lines.iter().enumerate().max_by_key(|(_, l)| len(l)).map(|(i, _)| i)),
            };
            self.widest = (key, widest.unwrap_or(0));
        }
        let sel = self.sel.filter(|sel| sel.session == s.id && !sel.is_empty());
        let cell_w = self.cell_w;
        let runs = self.runs.clone();
        let id = s.id;
        let mono = look::mono_font(cx);
        let copy_text = {
            let (runs, lines) = (runs.clone(), lines.clone());
            move |cx: &App| -> Option<String> {
                let sel = sel?;
                Some(match &lines {
                    Some(lines) => selected_text(lines, 0, &sel),
                    None => runs.read(cx).session(id).map(|s| selected_text(&s.lines, s.dropped, &sel))?,
                })
            }
        };
        let menu = {
            let runs = runs.clone();
            let lines = lines.clone();
            let copy_text = copy_text.clone();
            move |menu: PopupMenu, _: &mut Window, cx: &mut Context<PopupMenu>| {
                let menu = match copy_text(cx) {
                    Some(text) => menu
                        .item(
                            PopupMenuItem::new(ts!("cmd-copy"))
                                .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                                }),
                        )
                        .separator(),
                    None => menu,
                };
                let menu = console_menu(menu, &runs, id, lines.clone());
                if lines.is_none() && runs.read(cx).session(id).is_some_and(failed) {
                    let agent = crate::agents::console_agent(cx);
                    let runs = runs.clone();
                    menu.separator().item(
                        PopupMenuItem::new(t!("runwin-fix-with", agent = agent.short.to_string()))
                            .icon(crate::components::menu_icon(agent.icon))
                            .on_click(move |_, _, cx| runs.update(cx, |r, cx| r.fix_with_agent(id, cx))),
                    )
                } else {
                    menu
                }
            }
        };
        let rows = {
            let (runs, lines) = (runs.clone(), lines.clone());
            move |range: std::ops::Range<usize>, _: &mut Window, cx: &mut App| {
                let row = |i: usize, l: &ConsoleLine| {
                    let len = l.spans.iter().map(|s| s.text.chars().count()).sum();
                    console_line(l, i, &runs, &p, sel.and_then(|sel| sel.cols(base + i, len)), cell_w)
                };
                match &lines {
                    Some(lines) => range.filter_map(|i| Some(row(i, lines.get(i)?))).collect(),
                    None => {
                        let all = runs.read(cx);
                        let Some(s) = all.session(id) else { return Vec::new() };
                        range.filter_map(|i| Some(row(i, s.lines.get(i)?))).collect()
                    }
                }
            }
        };
        let line_text = {
            let (runs, lines) = (runs.clone(), lines.clone());
            move |line: usize, cx: &App| -> String {
                match &lines {
                    Some(lines) => lines.get(line).map(ConsoleLine::text).unwrap_or_default(),
                    None => runs
                        .read(cx)
                        .session(id)
                        .and_then(|s| line.checked_sub(s.dropped).and_then(|i| s.lines.get(i)))
                        .map(ConsoleLine::text)
                        .unwrap_or_default(),
                }
            }
        };
        div()
            .id("run-console-area")
            .debug_selector(|| "run-console".into())
            .size_full()
            // As wide as its window, whatever the longest line: the list scrolls sideways inside.
            .min_w_0()
            .overflow_hidden()
            .font_family(mono.clone())
            .text_size(px(12.5))
            .track_focus(&self.console_focus)
            .key_context("RunConsole")
            .on_action(cx.listener({
                let copy_text = copy_text.clone();
                move |_, _: &Copy, _, cx| {
                    if let Some(text) = copy_text(cx) {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.console_focus, cx);
                    let ts = window.text_system().clone();
                    let font = ts.resolve_font(&gpui::font(mono.clone()));
                    this.cell_w = ts.advance(font, px(12.5), 'M').map_or(px(7.5), |a| a.width);
                    let at = this.console_pos(e.position, base);
                    let (anchor, head) = match e.click_count {
                        1 => (at, at),
                        2 => match word_at(&line_text(at.0, cx), at.1) {
                            Some((from, to)) => ((at.0, from), (at.0, to)),
                            None => (at, at),
                        },
                        _ => ((at.0, 0), (at.0, line_text(at.0, cx).chars().count())),
                    };
                    this.sel = Some(ConsoleSel { session: id, anchor, head });
                    this.selecting = e.click_count == 1;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                if this.selecting && e.pressed_button == Some(MouseButton::Left) {
                    let head = this.console_pos(e.position, base);
                    if let Some(sel) = this.sel.as_mut().filter(|s| s.head != head) {
                        sel.head = head;
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.selecting = false))
            .child(
                uniform_list("run-console", count, rows)
                    .track_scroll(&self.console)
                    .with_horizontal_sizing_behavior(gpui::ListHorizontalSizingBehavior::Unconstrained)
                    .with_width_from_item(Some(self.widest.1))
                    .size_full()
                    .px(px(8.))
                    .py(px(4.)),
            )
            .context_menu(menu)
            .into_any_element()
    }

    /// The console place (line, character) under the window point `at`: the rows are 18 high
    /// below 4 of padding, the characters [`Self::cell_w`] wide after 8.
    fn console_pos(&self, at: Point<Pixels>, base: usize) -> ConsolePos {
        let state = self.console.0.borrow();
        let (bounds, offset) = (state.base_handle.bounds(), state.base_handle.offset());
        let y = at.y - bounds.origin.y - px(4.) - offset.y;
        let x = at.x - bounds.origin.x - px(8.) - offset.x;
        let line = (y / px(18.)).floor().max(0.) as usize;
        let col = (x / self.cell_w).round().max(0.) as usize;
        (base + line, col)
    }

    /// The test results: the tree on the left, the selected test on the right.
    fn tests(&mut self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let tree = s.tests.clone().unwrap_or_default();
        let id = s.id;
        let rows = tree.rows(&s.collapsed);
        let selected = s.selected_test.clone();
        let mut list = Vec::new();
        // The run itself first: its counts.
        let (g, ink) = session_mark(s, &p);
        let runs = self.runs.clone();
        list.push(
            crate::components::list_row_h(
                "test-root",
                crate::components::RowState::current(selected.is_none()),
                crate::components::TREE_ROW_H,
                &p,
            )
            .debug_selector(|| "test-root".into())
            .px(px(8.))
            .gap(px(6.))
            .cursor_pointer()
            .child(icon(g, 12., ink))
            .child(div().font_weight(FontWeight::MEDIUM).child(ts!("runwin-test-results")))
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
                crate::components::list_row_h(
                    ("test-row", i),
                    crate::components::RowState::current(on),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || format!("test-row-{i}"))
                .pl(px(8. + 16. * (*depth as f32 + 1.)))
                .pr(px(8.))
                .gap(px(6.))
                .cursor_pointer()
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
                .child(div().flex_1().min_w_0().truncate().child(n.name.clone()))
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
                    .w(gpui::relative(0.5))
                    .min_w(px(200.))
                    .max_w(px(360.))
                    .h_full()
                    .p(px(4.))
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
            lines.push(ConsoleLine::system(ts!("cmd-output")));
            add(Stream::Out, &n.output, &mut lines);
        }
        if lines.is_empty() {
            let what = match n.state {
                TestState::Passed => ts!("runwin-passed"),
                TestState::Skipped => ts!("runwin-skipped"),
                TestState::Running => ts!("console-running"),
                TestState::Failed => ts!("run-failed-3"),
            };
            lines.push(ConsoleLine::system(t!("runwin-nothing-printed", what = what.to_string())));
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
                .child(pane(ts!("runwin-expected"), expected, p.ok))
                .child(pane(ts!("runwin-actual"), actual, p.error))
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
                (e, w) => t!("runwin-errors-warnings", e = e.to_string(), w = w.to_string()),
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
                Some("up-to-date") => Some(ts!("runwin-up-to-date")),
                Some("from-cache") => Some(ts!("runwin-from-cache")),
                Some("skipped") => Some("skipped"),
                Some("no-source") => Some(ts!("runwin-no-source")),
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
        // The column keeps its width and its border whatever the rows' length; they scroll in it.
        div()
            .debug_selector(|| "build-tree".into())
            .flex_none()
            .w(gpui::relative(0.45))
            .max_w(px(320.))
            .h_full()
            .border_r_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .child(div().id("build-tree").size_full().p(px(4.)).overflow_y_scrollbar().children(rows))
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
                    DebugTab::Debugger => ts!("runwin-debugger"),
                    DebugTab::Console => ts!("console-console"),
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
                tool("debug-rerun", Icon::Rerun, ts!("runwin-rerun-r"), true, p.ok, &p)
                    .on_click(move |_, _, cx| a.update(cx, |r, cx| r.rerun(id, cx))),
            )
            .child(step("debug-resume", Icon::Resume, ts!("runwin-resume-f9"), "continue", paused, p.ok))
            .child(step("debug-pause", Icon::Pause, ts!("runwin-pause-program-f9"), "pause", live && !paused, p.fg))
            .child(
                tool("debug-stop", Icon::Stop, ts!("title-stop-f2"), s.running(), p.error, &p)
                    .on_click(move |_, _, cx| b.update(cx, |r, cx| r.stop(id, cx))),
            )
            .child(div().w(px(10.)))
            .child(step("debug-step-over", Icon::StepOver, ts!("runwin-step-over-f8"), "next", paused, p.accent))
            .child(step("debug-step-into", Icon::StepInto, ts!("runwin-step-into-f7"), "stepIn", paused, p.accent))
            .child(step("debug-step-out", Icon::StepOut, ts!("runwin-step-out-f8"), "stepOut", paused, p.accent))
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
                ts!("runwin-the-program-ended")
            } else if d.dap.is_none() {
                ts!("runwin-waiting-for-the-program-to-start")
            } else {
                ts!("runwin-the-program-is-running-breakpoints-stop-it-here")
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
                    .icon(crate::components::menu_icon(crate::components::IconName::ListTree))
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
                    crate::components::list_row_h(
                        ("debug-frame", i),
                        crate::components::RowState::current(on),
                        crate::components::TREE_ROW_H,
                        &p,
                    )
                    .debug_selector(move || format!("debug-frame-{i}"))
                    .px(px(10.))
                    .gap(px(6.))
                    .cursor_pointer()
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
                    .child(
                        tool("debug-add-watch", Icon::Eye, ts!("runwin-add-to-watches"), d.paused, p.fg, &p).on_click(
                            move |_, _, cx| {
                                let expr = eval2.read(cx).value().to_string();
                                runs.update(cx, |r, cx| r.add_watch(id, expr, cx));
                            },
                        ),
                    )
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
                    crate::components::list_row_h(
                        ("debug-var", i),
                        crate::components::RowState::default(),
                        crate::components::TREE_ROW_H,
                        &p,
                    )
                    .debug_selector(move || format!("debug-var-{i}"))
                    .pl(px(8. + 16. * row.depth as f32))
                    .pr(px(8.))
                    .gap(px(6.))
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
                    .w(gpui::relative(0.45))
                    .max_w(px(340.))
                    .h_full()
                    .p(px(4.))
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
                    .p(px(4.))
                    .overflow_y_scrollbar()
                    .children(vars),
            )
            .into_any_element()
    }

    fn empty(&self, cx: &App) -> AnyElement {
        use crate::components::{EmptyState, IconName};
        use snout_core::art::spots;
        let _ = cx;
        let state = match self.place {
            Place::Run => EmptyState::new(spots::no_tests(), ts!("runwin-nothing-running"))
                .hint(ts!("runwin-run-a-configuration-from-the-title-bar-or"))
                .primary_action("run-empty-run", IconName::Play, ts!("menu-run"), crate::menus::RunSelected),
            Place::Debug => EmptyState::new(spots::debugger_idle(), ts!("runwin-no-debug-session"))
                .hint(ts!("runwin-debug-a-configuration-from-the-title-bar-or"))
                .primary_action("debug-empty-debug", IconName::Bug, ts!("cmd-debug"), crate::menus::DebugSelected),
            Place::Build => EmptyState::new(snout_core::art::building(), ts!("runwin-no-build-output"))
                .hint(ts!("runwin-build-the-project-or-run-one-of-its"))
                .primary_action(
                    "build-empty-build",
                    IconName::Hammer,
                    ts!("cmd-build-project"),
                    crate::menus::BuildProject,
                )
                .secondary("build-empty-task", IconName::SquarePlay, ts!("runwin-run-task"), |_, w, cx| {
                    w.dispatch_action(Box::new(crate::menus::RunGradleTask), cx)
                }),
        };
        state.compact().into_any_element()
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
            config: s.config.clone(),
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
            engine: s.engine,
            process: s.process.clone(),
            // The Tests window reads it from the session itself.
            playwright: None,
        })
    }
}
