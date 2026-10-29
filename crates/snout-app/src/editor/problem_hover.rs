//! The editor's hover card, IntelliJ style: the problem under the pointer (or beside its
//! gutter mark) with its severity and message, the best fix as a button with ⌥⇧↩, More
//! actions… (⌥↩), and below them what the engine says the symbol is; the bulb in the gutter
//! of the caret's line when the engine has fixes there; ⌥⇧↩, the best fix at the caret; and the
//! context actions (⌥↩, ⌘.) with Fix all in file and Ask the agent to fix.
//!
//! Settings ▸ Editor ▸ Show hover says when the card shows: once the pointer rests for the
//! delay, or only while ⌘ is held. ⌘ always shows it at once, beside GPUI Kit's link to the
//! declaration. GPUI Kit's own hover and problem popovers give way to the card.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, Context, Global, InteractiveElement as _, IntoElement, KeyDownEvent, Modifiers,
    ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Styled as _, Task, Window, div, point, px,
};
use gpui_component::input::RopeExt as _;
use lsp_types::{CodeAction, CodeActionOrCommand, Diagnostic};
use serde_json::json;

use super::context_actions::{self, ActionsList, Entry, Key};
use super::{ClaudeRequest, EditorDefaults, EditorEvent, EditorPanel, ShowContextActions};
use crate::components::{Button, IconName, icon};
use crate::lang::fixes::{self, FixAll};
use crate::lang::{HubEvent, text};
use crate::look;
use crate::settings::ShowHover;
use crate::theme::{mix, palette};

/// Engine answers made up, for tests and screenshots of files no engine serves: the code
/// actions for a range of a file, and what a word is (markdown).
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct CannedLang {
    pub actions: Rc<dyn Fn(&std::path::Path, &lsp_types::Range) -> Vec<CodeAction>>,
    pub hover: Rc<dyn Fn(&std::path::Path, &str) -> Option<String>>,
}

impl Global for CannedLang {}

/// Where the pointer rests: on text (the bytes the card is about: the problems' range, or
/// the word), or on a line's problem mark in the gutter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Spot {
    Text(Range<usize>),
    Line(u32),
}

/// The fixes of the card's problem.
#[derive(Clone, Debug, PartialEq)]
pub enum Fixes {
    Looking,
    Found(Vec<CodeAction>),
}

/// The card while it shows (or waits for what the engine says).
pub struct HoverCard {
    pub spot: Spot,
    /// Where it shows: under the line, window coordinates.
    pub at: Point<Pixels>,
    /// The problems there, worst first.
    pub problems: Vec<Diagnostic>,
    pub fixes: Fixes,
    /// What the engine says the symbol is (markdown).
    pub info: Option<String>,
    id: u64,
    /// The text's scroll when it showed: a scroll puts the card away.
    scroll: Point<Pixels>,
}

impl HoverCard {
    /// It has something to show yet.
    pub fn ready(&self) -> bool {
        !self.problems.is_empty() || self.info.is_some()
    }
}

/// What is known about a symbol, on its way: its markdown and the bytes it is about.
type InfoTask = Task<Option<(String, Range<usize>)>>;

/// The caret's line has fixes: the bulb, red for an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bulb {
    pub line: u32,
    pub error: bool,
}

/// How long the card stays after the pointer leaves it or its spot.
pub const HIDE_GRACE: Duration = Duration::from_millis(300);
/// With ⌘ held the card shows at once, but for a pointer passing over.
const COMMAND_DELAY: Duration = Duration::from_millis(40);
/// How long the caret rests on a line before the engine is asked whether it has fixes.
pub const BULB_SETTLE: Duration = Duration::from_millis(250);

/// The hover card, the bulb and the context actions of one editor.
#[derive(Default)]
pub struct HoverState {
    pub card: Option<HoverCard>,
    /// The pointer is on the card.
    pub over_card: bool,
    /// The spot a card is on its way for.
    pending: Option<Spot>,
    /// Where the pointer was last, for a change of modifiers without a move.
    last: Option<Point<Pixels>>,
    next_id: u64,
    _show: Option<Task<()>>,
    _hide: Option<Task<()>>,
    _fixes: Option<Task<()>>,
    _info: Option<Task<()>>,
    pub bulb: Option<Bulb>,
    /// What the bulb was asked for: the line, the text's revision, the problems' revision.
    bulb_for: Option<(u32, u64, u64)>,
    _bulb: Option<Task<()>>,
    /// Where the bulb was drawn last.
    pub bulb_painted: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub actions: Option<ActionsList>,
    _actions: Option<Task<()>>,
    _top: Option<Task<()>>,
    _fix_all: Option<Task<()>>,
    /// Bumped when the problems change.
    pub problems_rev: u64,
}

/// The text offset of the character under `at` (window coordinates): the visible line at
/// that height, searched a character at a time.
pub fn offset_near(state: &gpui_component::input::EditorState, at: Point<Pixels>) -> Option<usize> {
    let lines = super::run_gutter::visible_lines(state);
    let line_height = state.line_height()?;
    let i = lines.iter().rposition(|(_, top)| *top <= at.y)?;
    let (line, top) = lines[i];
    let bottom = lines.get(i + 1).map_or(top + line_height, |(_, t)| *t);
    if at.y >= bottom {
        return None;
    }
    let text = state.text();
    let (start, end) = (text.line_start_offset(line), text.line_end_offset(line));
    let mut offset = start;
    for c in text.slice(start..end).chars() {
        let next = offset + c.len_utf8();
        if state.range_to_bounds(&(offset..next)).is_some_and(|b| b.contains(&at)) {
            return Some(offset);
        }
        offset = next;
    }
    None
}

impl EditorPanel {
    /// The keys of ⌥⇧↩ and ⌥↩, as the keymap has them now.
    pub fn fix_keys(cx: &App) -> (String, String) {
        let top = crate::keymap::shortcut(&super::ApplyTopFix, cx).unwrap_or_else(|| "\u{2325}\u{21e7}\u{21a9}".into());
        let more = crate::keymap::shortcut(&ShowContextActions, cx).unwrap_or_else(|| "\u{2325}\u{21a9}".into());
        (top, more)
    }

    /// The problems at byte `offset`, worst first.
    pub fn problems_at(&self, offset: usize, cx: &App) -> Vec<Diagnostic> {
        let rope = self.state.read(cx).text();
        let at = text::to_lsp(rope, offset);
        let end = rope.word_range(offset).map_or(at, |w| text::to_lsp(rope, w.end));
        fixes::worst_first(fixes::at_position(&self.problems, at, end))
    }

    /// The problems that take in `line`, worst first.
    pub fn problems_on_line(&self, line: u32) -> Vec<Diagnostic> {
        let on: Vec<Diagnostic> = self
            .problems
            .iter()
            .filter(|d| {
                let (a, b) = fixes::lines_of(d);
                a <= line && line <= b
            })
            .cloned()
            .collect();
        fixes::worst_first(on)
    }

    /// The problems at the caret (else on its line), worst first.
    pub fn problems_at_caret(&self, cx: &App) -> Vec<Diagnostic> {
        let s = self.state.read(cx);
        let here = self.problems_at(s.cursor(), cx);
        if here.is_empty() { self.problems_on_line(s.cursor_position().line) } else { here }
    }

    /// Ask the engine for the code actions on `range` with `diagnostics` as their context
    /// (the canned answers without an engine).
    pub fn request_actions(
        &self,
        range: lsp_types::Range,
        diagnostics: Vec<Diagnostic>,
        cx: &mut App,
    ) -> Task<Vec<CodeAction>> {
        if let Some(link) = self.lang.clone() {
            let rope = self.state.read(cx).text().clone();
            let params = json!({
                "textDocument": { "uri": link.uri.as_str() },
                "range": range,
                "context": { "diagnostics": diagnostics, "triggerKind": 1 }
            });
            let call = link.ask(&rope, "textDocument/codeAction", params, cx);
            return cx.background_executor().spawn(async move {
                match call.await {
                    Ok(v) => serde_json::from_value::<Vec<CodeActionOrCommand>>(v)
                        .unwrap_or_default()
                        .into_iter()
                        .map(crate::lang::providers::as_action)
                        .collect(),
                    Err(e) => {
                        eprintln!("[java] code actions: {e}");
                        Vec::new()
                    }
                }
            });
        }
        match cx.try_global::<CannedLang>() {
            Some(c) => Task::ready((c.actions)(&self.path, &range)),
            None => Task::ready(Vec::new()),
        }
    }

    /// What is known about the symbol at `offset`, when anything can say: its markdown and
    /// the bytes it is about.
    fn request_info(&self, offset: usize, cx: &mut App) -> Option<InfoTask> {
        let rope = self.state.read(cx).text().clone();
        if let Some(link) = self.lang.clone() {
            return Some(link.hover_markdown(&rope, offset, cx));
        }
        if let Some(which) = self.settings_file {
            return Some(Task::ready(crate::settings_json::hover(which, &rope.to_string(), offset)));
        }
        let canned = cx.try_global::<CannedLang>()?.clone();
        let word = rope.word_range(offset)?;
        let md = (canned.hover)(&self.path, &rope.slice(word.clone()).to_string());
        Some(Task::ready(md.map(|m| (m, word))))
    }

    /// The pointer moved over the editor (`dragging`: with a button down).
    pub(super) fn hover_move(&mut self, at: Point<Pixels>, mods: Modifiers, dragging: bool, cx: &mut Context<Self>) {
        // GPUI Kit's own popover of the problem under the pointer gives way to the card.
        if self.state.read(cx).diagnostic_popover().is_some() {
            self.state.update(cx, |s, cx| s.clear_diagnostic_popover(cx));
        }
        self.hover.last = Some(at);
        if self.hover.over_card {
            return;
        }
        let command = mods.secondary(cx);
        let wanted = command || EditorDefaults::now(cx).show_hover == ShowHover::AfterDelay;
        let spot = (!dragging && !mods.alt && wanted).then(|| self.spot_at(at, cx)).flatten();
        let Some((spot, offset)) = spot else {
            self.hover.pending = None;
            self.hover._show = None;
            if !wanted && !self.hover.over_card {
                self.hide_card(cx);
            } else {
                self.hide_card_soon(cx);
            }
            return;
        };
        if self.hover.card.as_ref().is_some_and(|c| c.spot == spot) {
            self.hover._hide = None;
            return;
        }
        if self.hover.card.is_some() {
            self.hide_card_soon(cx);
        }
        if self.hover.pending.as_ref() == Some(&spot) {
            return;
        }
        self.hover.pending = Some(spot.clone());
        let delay = if command { COMMAND_DELAY } else { EditorDefaults::now(cx).hover_delay };
        self.hover._show = Some(cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let _ = this.update(cx, |this, cx| {
                if this.hover.pending.as_ref() == Some(&spot) {
                    this.hover.pending = None;
                    this.show_card(spot, offset, cx);
                }
            });
        }));
    }

    /// The pointer left the editor: no card on its way, the one showing goes in a moment.
    pub(super) fn hover_left(&mut self, cx: &mut Context<Self>) {
        self.hover.pending = None;
        self.hover._show = None;
        self.hover.last = None;
        self.hide_card_soon(cx);
    }

    /// ⌘ went down or up with the pointer still: as if it moved.
    pub(super) fn hover_modifiers(&mut self, mods: Modifiers, cx: &mut Context<Self>) {
        if let Some(at) = self.hover.last {
            self.hover_move(at, mods, false, cx);
        }
    }

    /// What is under `at`: a problem mark in the gutter (its line), or text (the problems'
    /// range there, else the word), with the byte under the pointer (the line's start in the
    /// gutter).
    fn spot_at(&self, at: Point<Pixels>, cx: &App) -> Option<(Spot, usize)> {
        let s = self.state.read(cx);
        let lanes = self.lanes.get();
        let (left, right) = lanes.problems();
        if lanes.bounds.size.width > px(0.) && at.x >= left && at.x < right && lanes.bounds.contains(&at) {
            let line_height = s.line_height()?;
            let lines = super::run_gutter::visible_lines(s);
            let (line, _) = lines.iter().rev().find(|(_, top)| at.y >= *top && at.y < *top + line_height)?;
            let line = *line as u32;
            if self.problems.iter().any(|d| d.range.start.line == line) {
                return Some((Spot::Line(line), s.text().line_start_offset(line as usize)));
            }
            return None;
        }
        let offset = offset_near(s, at)?;
        let rope = s.text();
        let here = self.problems_at(offset, cx);
        let span = if here.is_empty() {
            let word = rope.word_range(offset).filter(|w| !w.is_empty())?;
            if rope.slice(word.clone()).to_string().trim().is_empty() {
                return None;
            }
            word
        } else {
            let word = rope.word_range(offset).unwrap_or(offset..offset + 1);
            let ranges: Vec<Range<usize>> = here
                .iter()
                .map(|d| {
                    let r = text::range_from_lsp(rope, d.range);
                    if r.is_empty() { word.clone() } else { r }
                })
                .collect();
            let start = ranges.iter().map(|r| r.start).min().unwrap_or(offset);
            let end = ranges.iter().map(|r| r.end).max().unwrap_or(offset + 1);
            start..end
        };
        Some((Spot::Text(span), offset))
    }

    /// Show the card for `spot`: its problems at once (and their fixes as the engine finds
    /// them), what the symbol is once the engine says.
    pub fn show_card(&mut self, spot: Spot, offset: usize, cx: &mut Context<Self>) {
        if self.loading
            || self.hover.actions.is_some()
            || self.context.is_open()
            || self.gutter_menu.is_some()
            || self.rename.is_some()
            || self.choices.is_some()
        {
            return;
        }
        let (problems, at) = {
            let s = self.state.read(cx);
            match &spot {
                Spot::Line(line) => {
                    let lanes = self.lanes.get();
                    let at =
                        s.range_to_bounds(&(offset..offset)).map(|b| point(lanes.problems().1 + px(4.), b.bottom()));
                    (fixes::worst_first(fixes::on_line(&self.problems, *line)), at)
                }
                Spot::Text(span) => {
                    let line_of = |o: usize| s.text().offset_to_position(o).line;
                    let here = s.range_to_bounds(&(offset..offset));
                    let start = s.range_to_bounds(&(span.start..span.start));
                    let x = match (start, here) {
                        (Some(a), Some(_)) if line_of(span.start) == line_of(offset) => Some(a.left()),
                        (_, Some(b)) => Some(b.left()),
                        _ => None,
                    };
                    (self.problems_at(offset, cx), x.zip(here).map(|(x, b)| point(x, b.bottom())))
                }
            }
        };
        let Some(at) = at else { return };
        let info = matches!(spot, Spot::Text(_)).then(|| self.request_info(offset, cx)).flatten();
        if problems.is_empty() && info.is_none() {
            return;
        }
        self.hover.next_id += 1;
        let id = self.hover.next_id;
        let looking = !problems.is_empty();
        self.hover.card = Some(HoverCard {
            spot,
            at,
            problems: problems.clone(),
            fixes: if looking { Fixes::Looking } else { Fixes::Found(Vec::new()) },
            info: None,
            id,
            scroll: self.state.read(cx).scroll_offset(),
        });
        self.hover._hide = None;
        if let Some(first) = problems.first() {
            let task = self.request_actions(first.range, problems.clone(), cx);
            self.hover._fixes = Some(cx.spawn(async move |this, cx| {
                let found = fixes::usable(task.await);
                let _ = this.update(cx, |this, cx| {
                    if let Some(card) = this.hover.card.as_mut().filter(|c| c.id == id) {
                        card.fixes = Fixes::Found(found);
                        cx.notify();
                    }
                });
            }));
        }
        if let Some(info) = info {
            self.hover._info = Some(cx.spawn(async move |this, cx| {
                let answer = info.await;
                let _ = this.update(cx, |this, cx| {
                    let Some(card) = this.hover.card.as_mut().filter(|c| c.id == id) else { return };
                    card.info = answer.map(|(md, _)| md).filter(|m| !m.trim().is_empty());
                    if !card.ready() {
                        this.hover.card = None;
                    }
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    /// Put the card away.
    pub fn hide_card(&mut self, cx: &mut Context<Self>) {
        self.hover._hide = None;
        self.hover.over_card = false;
        if self.hover.card.take().is_some() {
            self.hover._fixes = None;
            self.hover._info = None;
            cx.notify();
        }
    }

    /// The text scrolled under the card: it goes.
    pub(super) fn card_follow_scroll(&mut self, cx: &mut Context<Self>) {
        let scroll = self.state.read(cx).scroll_offset();
        if self.hover.card.as_ref().is_some_and(|c| c.scroll != scroll) {
            self.hide_card(cx);
        }
    }

    /// Put the card away in a moment, unless the pointer gets onto it (or back to its spot).
    fn hide_card_soon(&mut self, cx: &mut Context<Self>) {
        if self.hover.card.is_none() || self.hover._hide.is_some() {
            return;
        }
        self.hover._hide = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HIDE_GRACE).await;
            let _ = this.update(cx, |this, cx| {
                this.hover._hide = None;
                if !this.hover.over_card {
                    this.hide_card(cx);
                }
            });
        }));
    }

    /// The card, under the line it is about.
    pub(super) fn card_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self.hover.card.as_ref().filter(|c| c.ready())?;
        let p = palette(cx);
        let agent = crate::agents::console_agent(cx);
        let (top_keys, more_keys) = Self::fix_keys(cx);
        let this = cx.weak_entity();
        let problems = card.problems.iter().take(3).enumerate().map(|(i, d)| {
            let color = context_actions::severity_color(d, &p);
            let source = match (&d.source, &d.code) {
                (Some(s), _) if s != "Java" => Some(s.clone()),
                _ => None,
            };
            div()
                .id(("hover-problem", i))
                .debug_selector(move || format!("hover-problem-{i}"))
                .flex()
                .items_start()
                .gap(px(8.))
                .child(
                    div()
                        .flex_none()
                        .mt(px(1.))
                        .size(px(20.))
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(mix(color, 0.18, p.card))
                        .child(icon(context_actions::severity_icon(d), 12., color)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pt(px(2.))
                        .line_height(px(17.))
                        .child(d.message.trim().to_owned())
                        .when_some(source, |e, s| {
                            e.child(div().text_size(px(11.)).text_color(p.fg_dim).child(s.to_string()))
                        }),
                )
        });
        let first = card.problems.first().cloned();
        let fix_row = first.map(|problem| {
            let row = div().flex().flex_wrap().items_center().gap(px(6.)).pl(px(28.));
            match &card.fixes {
                Fixes::Looking => row.child(
                    div()
                        .id("hover-looking")
                        .debug_selector(|| "hover-looking".into())
                        .h(px(24.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_color(p.fg_dim)
                        .child(icon(IconName::LoaderCircle, 13., p.fg_dim))
                        .child("Looking for fixes\u{2026}"),
                ),
                Fixes::Found(list) => {
                    let main = match fixes::best_fix(list).cloned() {
                        Some(fix) => {
                            let this = this.clone();
                            Button::new("hover-fix", IconName::Lightbulb)
                                .compact()
                                .primary()
                                .label(fix.title.clone())
                                .kbd(top_keys)
                                .on_click(move |_, window, cx| {
                                    let fix = fix.clone();
                                    let _ = this.update(cx, |e, cx| {
                                        e.hide_card(cx);
                                        e.apply_action(fix, window, cx);
                                    });
                                })
                        }
                        None => {
                            let (this, problem) = (this.clone(), problem.clone());
                            Button::new("hover-ask", agent.icon)
                                .compact()
                                .label(format!("Ask {} to fix", agent.short))
                                .on_click(move |_, _, cx| {
                                    let problem = problem.clone();
                                    let _ = this.update(cx, |e, cx| {
                                        e.hide_card(cx);
                                        e.ask_agent_to_fix(&problem, cx);
                                    });
                                })
                        }
                    };
                    let this = this.clone();
                    row.child(main).child(
                        Button::new("hover-more", IconName::Ellipsis)
                            .compact()
                            .ghost()
                            .label("More actions\u{2026}")
                            .kbd(more_keys)
                            .on_click(move |_, window, cx| {
                                let problem = problem.clone();
                                let _ = this.update(cx, |e, cx| e.more_actions(problem, window, cx));
                            }),
                    )
                }
            }
        });
        let info = card.info.clone().map(|md| {
            div()
                .id("hover-info")
                .debug_selector(|| "hover-info".into())
                .max_h(px(260.))
                .overflow_y_scroll()
                .text_size(px(12.5))
                .child(gpui_component::text::TextView::markdown("hover-info-text", md).selectable(true))
        });
        let has_problems = !card.problems.is_empty();
        let body = div()
            .id("hover-card")
            .debug_selector(|| "hover-card".into())
            .occlude()
            .min_w(px(300.))
            .max_w(px(560.))
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(match card.problems.first() {
                Some(d) => mix(context_actions::severity_color(d, &p), 0.4, p.border),
                None => p.border,
            })
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .text_color(p.fg)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hover.over_card = *hovered;
                if *hovered {
                    this.hover._hide = None;
                } else {
                    this.hide_card_soon(cx);
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.hide_card(cx)))
            .children(problems)
            .children(fix_row)
            .when(has_problems && info.is_some(), |d| d.child(div().h(px(1.)).bg(p.border)))
            .children(info);
        Some(
            gpui::deferred(gpui::anchored().position(card.at).snap_to_window_with_margin(px(8.)).child(body))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// More actions… of the card: the caret to the problem, and the context actions there.
    fn more_actions(&mut self, problem: Diagnostic, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_card(cx);
        let at = {
            let rope = self.state.read(cx).text();
            text::from_lsp(rope, problem.range.start)
        };
        self.state.update(cx, |s, cx| {
            s.set_selected_range(at..at, cx);
            s.focus(window, cx);
        });
        let here = self.problems_at(at, cx);
        let here = if here.is_empty() { vec![problem.clone()] } else { here };
        let anchor = self.caret_bounds(cx).map(|b| b.bottom_left()).unwrap_or_default();
        self.open_actions(here, problem.range, anchor, window, cx);
    }

    /// Apply a code action: its edit (here at once when it changes only this file, else as
    /// the workspace applies a server's edits), then its command.
    pub fn apply_action(&mut self, action: CodeAction, window: &mut Window, cx: &mut Context<Self>) {
        let (edit, command) = fixes::edit_of(&action);
        let uri = self.lang.as_ref().map(|l| l.uri.clone()).unwrap_or_else(|| snout_lang::uri::from_path(&self.path));
        if let Some(edit) = edit {
            match fixes::edits_in(&edit, &uri) {
                Some(list) => self.apply_lsp_edits(&list, window, cx),
                None => {
                    if let Some(hub) = self.lang.as_ref().and_then(|l| l.hub.upgrade()) {
                        hub.update(cx, |_, cx| cx.emit(HubEvent::ApplyEdit { edit, reply: None }));
                    }
                }
            }
        }
        if let Some(command) = command
            && let Some(link) = self.lang.clone()
            && let Some(hub) = link.hub.upgrade()
        {
            hub.update(cx, |h, cx| h.execute_command(&link.uri, command, cx));
        }
    }

    /// Fix all in file: the fix asked for each problem of its kind, the edits made at once.
    pub fn fix_all(&mut self, all: FixAll, window: &mut Window, cx: &mut Context<Self>) {
        let tasks: Vec<Task<Vec<CodeAction>>> =
            all.targets.iter().map(|d| self.request_actions(d.range, vec![d.clone()], cx)).collect();
        let revision = self.revision;
        let uri = self.lang.as_ref().map(|l| l.uri.clone()).unwrap_or_else(|| snout_lang::uri::from_path(&self.path));
        self.hover._fix_all = Some(cx.spawn_in(window, async move |this, cx| {
            let answers = futures::future::join_all(tasks).await;
            let (edits, n) = fixes::fix_all_from(&all, &uri, answers);
            let _ = this.update_in(cx, |this, window, cx| {
                if this.revision != revision {
                    return this.show_hint("The file changed meanwhile: try Fix all again", cx);
                }
                if n == 0 {
                    return this.show_hint("None of these problems could be fixed this way", cx);
                }
                this.apply_lsp_edits(&edits, window, cx);
                if n < all.targets.len() {
                    this.show_hint(format!("Fixed {n} of {} problems; the rest need a look", all.targets.len()), cx);
                }
            });
        }));
    }

    /// Ask the console's agent to fix `problem`: the file and lines, and what is wrong.
    pub fn ask_agent_to_fix(&mut self, problem: &Diagnostic, cx: &mut Context<Self>) {
        cx.emit(EditorEvent::Claude(ClaudeRequest::AskToFix {
            lines: fixes::lines_of(problem),
            text: fixes::ask_text(problem),
        }));
    }

    /// ⌥⇧↩: the best fix of the problem at the caret.
    pub fn apply_top_fix(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_card(cx);
        let here = self.problems_at_caret(cx);
        let Some(first) = here.first().cloned() else {
            return self.show_hint("No problem at the caret", cx);
        };
        let task = self.request_actions(first.range, here, cx);
        let revision = self.revision;
        self.hover._top = Some(cx.spawn_in(window, async move |this, cx| {
            let list = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.revision != revision {
                    return;
                }
                match fixes::best_fix(&list).cloned() {
                    Some(fix) => this.apply_action(fix, window, cx),
                    None => {
                        let (_, more) = Self::fix_keys(cx);
                        this.show_hint(format!("No quick fix for this problem: {more} for more actions"), cx);
                    }
                }
            });
        }));
    }

    /// ⌥↩ (and ⌘.): the context actions at the caret, for the problem there.
    pub fn show_context_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_card(cx);
        let here = self.problems_at_caret(cx);
        let range = {
            let s = self.state.read(cx);
            let selection = s.selected_range();
            match here.first() {
                _ if !selection.is_empty() => text::range_to_lsp(s.text(), &selection),
                Some(d) => d.range,
                None => text::range_to_lsp(s.text(), &(s.cursor()..s.cursor())),
            }
        };
        let at = self.caret_bounds(cx).map(|b| b.bottom_left()).unwrap_or_default();
        self.open_actions(here, range, at, window, cx);
    }

    /// The context actions at `at` (window coordinates) for `range`, about the problems `here`.
    pub fn open_actions(
        &mut self,
        here: Vec<Diagnostic>,
        range: lsp_types::Range,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hide_card(cx);
        // The engine takes the problems of the lines asked about as the actions' context.
        let context: Vec<Diagnostic> = self
            .problems
            .iter()
            .filter(|d| d.range.start.line <= range.end.line && d.range.end.line >= range.start.line)
            .cloned()
            .collect();
        let task = self.request_actions(range, context, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.hover.actions = Some(ActionsList::new(here.first().cloned(), at, focus));
        self.hover._actions = Some(cx.spawn_in(window, async move |this, cx| {
            let actions = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let entries = context_actions::entries(&this.problems, &here, actions);
                if this.hover.actions.is_none() {
                    return;
                }
                if entries.is_empty() {
                    this.close_actions(window, cx);
                    return this.show_hint("No actions here", cx);
                }
                if let Some(list) = this.hover.actions.as_mut() {
                    list.fill(entries);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Put the context actions away, and the keyboard back in the text.
    pub fn close_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.hover.actions.take().is_some() {
            self.hover._actions = None;
            self.state.update(cx, |s, cx| s.focus(window, cx));
            cx.notify();
        }
    }

    /// The line `i` of the context actions, picked.
    pub fn pick_action(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.hover.actions.as_ref().and_then(|l| l.entries.get(i).cloned()) else { return };
        self.close_actions(window, cx);
        match entry {
            Entry::Fix(a) | Entry::EngineFixAll(a) | Entry::Other(a) => self.apply_action(a, window, cx),
            Entry::FixAll(all) => self.fix_all(all, window, cx),
            Entry::Ask(problem) => self.ask_agent_to_fix(&problem, cx),
        }
    }

    fn actions_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self.hover.actions.as_mut() else { return };
        match list.key(e.keystroke.key.as_str()) {
            Key::Ignored => return,
            Key::Moved => cx.notify(),
            Key::Pick(i) => self.pick_action(i, window, cx),
            Key::Close => self.close_actions(window, cx),
        }
        cx.stop_propagation();
    }

    /// The context actions, where they were asked for.
    pub(super) fn actions_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let list = self.hover.actions.as_ref()?;
        let p = palette(cx);
        let agent = crate::agents::console_agent(cx);
        let (top, _) = Self::fix_keys(cx);
        let this = cx.weak_entity();
        let pick: context_actions::Pick = Rc::new(move |i, window, cx| {
            let _ = this.update(cx, |e, cx| e.pick_action(i, window, cx));
        });
        let card = context_actions::view("context-actions", list, &agent, Some(top), &p, look::ui_font(cx), pick)
            .track_focus(&list.focus)
            .key_context("ContextActions")
            .on_key_down(cx.listener(Self::actions_key))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_actions(window, cx)));
        Some(
            gpui::deferred(
                gpui::anchored()
                    .position(list.at + point(px(0.), px(2.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(card),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// The caret moved, the text or the problems changed: ask (once the caret rests) whether
    /// the engine has fixes on the caret's line, for the bulb.
    pub(super) fn bulb_soon(&mut self, cx: &mut Context<Self>) {
        let line = self.state.read(cx).cursor_position().line;
        let here = self.problems_on_line(line);
        if here.is_empty() {
            self.hover.bulb_for = None;
            self.hover._bulb = None;
            if self.hover.bulb.take().is_some() {
                cx.notify();
            }
            return;
        }
        let key = (line, self.revision, self.hover.problems_rev);
        if self.hover.bulb_for == Some(key) {
            return;
        }
        self.hover.bulb_for = Some(key);
        if self.hover.bulb.is_some_and(|b| b.line != line) {
            self.hover.bulb = None;
            cx.notify();
        }
        let error = here.iter().any(|d| fixes::rank(d) == 0);
        let range = lsp_types::Range::new(lsp_types::Position::new(line, 0), lsp_types::Position::new(line + 1, 0));
        self.hover._bulb = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BULB_SETTLE).await;
            let Ok(task) = this.update(cx, |this, cx| this.request_actions(range, here, cx)) else { return };
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.hover.bulb_for != Some(key) {
                    return;
                }
                let bulb = fixes::best_fix(&list).map(|_| Bulb { line, error });
                if this.hover.bulb != bulb {
                    this.hover.bulb = bulb;
                    cx.notify();
                }
            });
        }));
    }

    /// A click on the bulb: the context actions of its line.
    pub(super) fn bulb_press(&mut self, e: &gpui::MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if e.button != gpui::MouseButton::Left {
            return false;
        }
        let (Some(bulb), Some(b)) = (self.hover.bulb, self.hover.bulb_painted.get()) else { return false };
        if !b.dilate(px(3.)).contains(&e.position) {
            return false;
        }
        let here = self.problems_on_line(bulb.line);
        let Some(first) = here.first().cloned() else { return false };
        self.open_actions(here, first.range, b.bottom_left() + point(px(0.), px(2.)), window, cx);
        true
    }
}
