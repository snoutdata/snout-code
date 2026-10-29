//! Claude Code in an editor tab. A change it proposes types itself into the tab, hunk by
//! hunk, and waits there for Accept (⌘↩) or Reject (Esc): new lines lit green, removed
//! ones marked red where they were. A change it makes on its own plays out live: the
//! lines it removes show struck through in red, the new text types itself in, then its
//! green band fades; the gutter marks the lines until the change is reviewed.

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, TextAlign, TextRun, Window, canvas, div, point, px, size,
};
use gpui_component::input::RopeExt as _;
use snout_core::gutter::{LineChange, LineChangeKind};

use super::{EditorEvent, EditorPanel};
use crate::claude_ide::review::{self, Frame, Hunk, Mark, Stream};
use crate::look;
use crate::theme::{mix, palette};

/// One step of the change typing itself in.
pub const STREAM_TICK: Duration = Duration::from_millis(16);
/// One step of a follow highlight fading, and how many steps it lasts (about 3 s).
pub const FLASH_TICK: Duration = Duration::from_millis(50);
const FLASH_STEPS: u32 = 60;
/// One step of a live edit.
pub const LIVE_TICK: Duration = Duration::from_millis(16);
/// How long the text an edit removes shows struck through before the new text comes.
const STRIKE_TIME: Duration = Duration::from_millis(420);
/// How long the disk check waits for Claude's report of an edit it announced.
pub const HOLD: Duration = Duration::from_secs(10);
/// Past these, an edit is not typed in: it shows at once and lights up.
const LIVE_MAX_TEXT: usize = 512 * 1024;
const LIVE_MAX_CHARS: usize = 20_000;

/// How long an edit takes to type itself in: quick, a little longer for more, capped.
pub fn live_time(hunks: usize, chars: usize) -> Duration {
    Duration::from_millis((160 + 60 * hunks as u64 + chars as u64 / 2).clamp(220, 1_400))
}

/// What an editor asks of Claude Code's changes (the workspace answers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaudeRequest {
    /// Keep, or put back, the hunk whose new lines are these (in the tab's text).
    AcceptHunk(Range<usize>),
    RevertHunk(Range<usize>),
    AcceptFile,
    RevertFile,
    /// No more changes this way in this file: go to the next (or previous) file's.
    NextFile,
    PrevFile,
}

/// An edit Claude made on its own, playing out in the tab.
pub struct Live {
    plan: Stream,
    pub original: String,
    pub proposed: String,
    pub frame: Frame,
    tick: u32,
    /// Steps of the struck text alone, then of the typing.
    strike: u32,
    ticks: u32,
    /// Held where it is (a screenshot).
    paused: bool,
}

impl Live {
    /// Still showing the struck text, before the typing.
    pub fn striking(&self) -> bool {
        self.tick < self.strike
    }
}

/// How a review ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Saved, with the text written (the user may have edited the proposal).
    Accepted(String),
    Rejected,
}

/// Called once with the outcome of a review.
pub type Answer = Box<dyn FnOnce(Outcome)>;

/// A change waiting for its turn. Dropped unseen, it counts as rejected.
pub struct Proposal {
    pub tab_name: String,
    pub text: String,
    answer: Option<Answer>,
}

impl Drop for Proposal {
    fn drop(&mut self) {
        if let Some(answer) = self.answer.take() {
            answer(Outcome::Rejected);
        }
    }
}

struct Streaming {
    plan: Stream,
    tick: u32,
    ticks: u32,
    /// What the last step showed, to notice the user typing in between.
    frame: Frame,
    /// Held where it is (a screenshot).
    paused: bool,
}

/// The change under review in a tab.
pub struct Review {
    pub tab_name: String,
    /// The tab's text before the change.
    pub original: String,
    pub proposed: String,
    /// The file did not exist: rejecting closes the tab.
    pub new_file: bool,
    pub marks: Vec<Mark>,
    stream: Option<Streaming>,
    answer: Option<Answer>,
}

impl Review {
    pub fn streaming(&self) -> bool {
        self.stream.is_some()
    }
}

impl Drop for Review {
    fn drop(&mut self) {
        if let Some(answer) = self.answer.take() {
            answer(Outcome::Rejected);
        }
    }
}

/// Lines a followed edit changed, fading.
pub struct Flash {
    pub lines: Vec<Range<usize>>,
    pub step: u32,
}

/// The gutter runs of `hunks` (Claude's unreviewed changes) and each one's first line.
pub fn hunk_changes(hunks: &[Hunk]) -> (Vec<LineChange>, Vec<usize>) {
    let mut runs = Vec::new();
    let mut first = Vec::new();
    for h in hunks {
        let kind = match (h.old_lines.is_empty(), h.new_lines.is_empty()) {
            (_, true) => LineChangeKind::Deleted,
            (true, false) => LineChangeKind::Added,
            (false, false) => LineChangeKind::Modified,
        };
        runs.push(LineChange { kind, lines: h.new_lines.clone() });
        first.push(h.new_lines.start);
    }
    (runs, first)
}

impl Flash {
    fn strength(&self) -> f32 {
        let t = (self.step as f32 / FLASH_STEPS as f32).clamp(0., 1.);
        (1. - t) * (1. - t)
    }
}

impl EditorPanel {
    /// Changes waiting here, the one under review included.
    pub fn review_count(&self) -> usize {
        usize::from(self.review.is_some()) + self.queue.len()
    }

    /// Show `text` as a change Claude proposes for this file; `answer` hears how it
    /// ends. A tab already reviewing one queues it.
    pub fn propose(
        &mut self,
        tab_name: String,
        text: String,
        answer: Answer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let proposal = Proposal { tab_name, text, answer: Some(answer) };
        self.preview = false;
        // A tab still reading its file shows the change once it has the file to compare.
        if self.review.is_some() || self.loading {
            self.queue.push_back(proposal);
            cx.notify();
            return;
        }
        self.start_review(proposal, window, cx);
    }

    fn start_review(&mut self, mut p: Proposal, window: &mut Window, cx: &mut Context<Self>) {
        let original = self.text(cx);
        // As the host had it when the tab read the file (a proposal waits for that).
        let new_file = !self.on_disk();
        if new_file {
            self.error = None;
        }
        let proposed = std::mem::take(&mut p.text);
        let plan = Stream::new(&original, &proposed);
        let time = review::stream_time(plan.hunks.len(), plan.chars);
        let ticks = (time.as_millis() / STREAM_TICK.as_millis()).max(1) as u32;
        let frame = plan.frame(&original, &proposed, 0);
        let first = plan.hunks.first().map(|h| h.old.start);
        self.review = Some(Review {
            tab_name: std::mem::take(&mut p.tab_name),
            marks: Vec::new(),
            original,
            proposed,
            new_file,
            stream: Some(Streaming { plan, tick: 0, ticks, frame, paused: false }),
            answer: p.answer.take(),
        });
        // The view goes to the first change; the keyboard stays where it is.
        if let Some(at) = first {
            self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        }
        self._review_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(STREAM_TICK).await;
                match this.update_in(cx, |this, window, cx| this.stream_step(window, cx)) {
                    Ok(true) => {}
                    _ => return,
                }
            }
        }));
        cx.notify();
    }

    /// Type the next part of the change in. False once it is all in.
    fn stream_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let now = self.text(cx);
        let Some(r) = self.review.as_mut() else { return false };
        let Some(s) = r.stream.as_mut() else { return false };
        if now != s.frame.text {
            // The user typed in the meantime: the rest goes in at once, with their edit.
            let text = review::carry(&s.frame, &r.proposed, &now);
            r.stream = None;
            self.write_text(&text, false, window, cx);
            return false;
        }
        if s.paused {
            return true;
        }
        s.tick += 1;
        let typed = if s.tick >= s.ticks { s.plan.total } else { s.plan.total * s.tick as usize / s.ticks as usize };
        let frame = s.plan.frame(&r.original, &r.proposed, typed);
        let done = typed >= s.plan.total;
        let text = frame.text.clone();
        s.frame = frame;
        if done {
            r.stream = None;
        }
        let focused = gpui::Focusable::focus_handle(self.state.read(cx), cx).is_focused(window);
        self.write_text(&text, !focused, window, cx);
        cx.notify();
        !done
    }

    /// Hold the stream at `fraction` of the way (for a look at it part way).
    pub fn hold_stream_at(&mut self, fraction: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.review.as_mut() else { return };
        let Some(s) = r.stream.as_mut() else { return };
        let typed = ((s.plan.total as f32) * fraction.clamp(0., 1.)) as usize;
        s.paused = true;
        s.frame = s.plan.frame(&r.original, &r.proposed, typed);
        let text = s.frame.text.clone();
        self.write_text(&text, true, window, cx);
    }

    /// Put the whole change in now, with whatever the user typed meanwhile.
    pub fn finish_stream(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = self.text(cx);
        let Some(r) = self.review.as_mut() else { return };
        let Some(s) = r.stream.take() else { return };
        let text = review::carry(&s.frame, &r.proposed, &now);
        self.write_text(&text, false, window, cx);
    }

    /// Make the editor show `text`, changing only what differs. `follow` puts the caret
    /// after the change (the view follows the typing); otherwise the selection keeps its
    /// place in the text around.
    fn write_text(&mut self, text: &str, follow: bool, window: &mut Window, cx: &mut Context<Self>) {
        let now = self.text(cx);
        let Some((range, insert)) = review::change(&now, text) else { return };
        self.writing += 1;
        self.state.update(cx, |s, cx| {
            let sel = s.selected_range();
            let delta = insert.len() as isize - range.len() as isize;
            let shift = |p: usize| if p >= range.end { (p as isize + delta) as usize } else { p.min(range.start) };
            s.set_selected_range(range.clone(), cx);
            s.replace(insert, window, cx);
            if !follow {
                s.set_selected_range(shift(sel.start)..shift(sel.end), cx);
            }
        });
    }

    /// Recount the marks after the text changed.
    pub(super) fn review_changed(&mut self, cx: &mut Context<Self>) {
        let now = self.text(cx);
        if let Some(r) = self.review.as_mut() {
            r.marks = review::marks(&r.original, &now);
        }
    }

    /// Accept: say so, with the text shown (and any edits made to it). Claude Code writes
    /// the file itself once told (it refuses to when the file changed under it), so the tab
    /// takes that text as the file's and waits for the write.
    pub fn accept_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.review.is_none() {
            return;
        }
        self.finish_stream(window, cx);
        let Some(mut r) = self.review.take() else { return };
        let text = self.text(cx);
        self.saved = text.clone();
        self.dirty = false;
        self.preview = false;
        self.error = None;
        self.claude_hold = Some(std::time::Instant::now());
        if let Some(answer) = r.answer.take() {
            answer(Outcome::Accepted(text));
        }
        drop(r);
        self.next_review(window, cx);
    }

    /// Reject: put the text back as it was and say so.
    pub fn reject_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut r) = self.review.take() else { return };
        r.stream = None;
        self.write_text(&r.original.clone(), false, window, cx);
        if let Some(answer) = r.answer.take() {
            answer(Outcome::Rejected);
        }
        let gone = r.new_file && self.queue.is_empty();
        drop(r);
        if gone {
            self.discard(cx);
            cx.emit(EditorEvent::Discarded);
        }
        self.next_review(window, cx);
    }

    pub(super) fn next_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._review_task = None;
        if let Some(next) = self.queue.pop_front() {
            self.start_review(next, window, cx);
        }
        cx.notify();
    }

    /// End the review or queued change called `tab_name` (Claude Code closed it): one
    /// still waiting counts as rejected. False when there is none by that name.
    pub fn close_review(&mut self, tab_name: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.review.as_ref().is_some_and(|r| r.tab_name == tab_name) {
            self.reject_review(window, cx);
            return true;
        }
        let before = self.queue.len();
        self.queue.retain(|p| p.tab_name != tab_name);
        cx.notify();
        self.queue.len() != before
    }

    /// Reject every change waiting here. Returns how many there were.
    pub fn close_all_reviews(&mut self, window: &mut Window, cx: &mut Context<Self>) -> usize {
        let n = self.review_count();
        self.queue.clear();
        self.reject_review(window, cx);
        n
    }

    /// Light `lines` and let them fade (a change Claude made, followed).
    pub fn flash(&mut self, lines: Vec<Range<usize>>, window: &mut Window, cx: &mut Context<Self>) {
        if lines.is_empty() {
            return;
        }
        if let Some(first) = lines.first().map(|l| l.start) {
            let at = self.state.read(cx).text().line_start_offset(first);
            self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        }
        self.flash = Some(Flash { lines, step: 0 });
        self._flash_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(FLASH_TICK).await;
                let more = this.update(cx, |this, cx| {
                    let Some(f) = this.flash.as_mut() else { return false };
                    f.step += 1;
                    if f.step >= FLASH_STEPS {
                        this.flash = None;
                    }
                    cx.notify();
                    this.flash.is_some()
                });
                if !matches!(more, Ok(true)) {
                    return;
                }
            }
        }));
        cx.notify();
    }

    /// The gutter's marks while a change is under review: its new lines and removals.
    pub(super) fn review_gutter(&self) -> Option<Vec<LineChange>> {
        let r = self.review.as_ref()?;
        let mut out = Vec::new();
        for m in &r.marks {
            if !m.lines.is_empty() {
                out.push(LineChange { kind: LineChangeKind::Added, lines: m.lines.clone() });
            }
            if m.removed > 0 {
                out.push(LineChange { kind: LineChangeKind::Deleted, lines: m.lines.start..m.lines.start });
            }
        }
        Some(out)
    }

    /// The bands over the text: new lines green, a red rule where lines went (with how
    /// many), and the fading light of a followed edit. A live edit: its struck lines red,
    /// the lines typed so far green, a caret where the typing is.
    pub(super) fn claude_overlay(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let inline = self.review.is_none() && self.claude_diff && self.claude_base.is_some();
        if self.review.is_none() && self.flash.is_none() && self.live.is_none() && !inline {
            return None;
        }
        let p = palette(cx);
        let c = p.claude();
        let marks = match &self.review {
            Some(r) => r.marks.clone(),
            None if inline && self.live.is_none() => self
                .claude_hunks
                .iter()
                .map(|h| Mark { lines: h.new_lines.clone(), removed: h.old_lines.len() })
                .collect(),
            None => Vec::new(),
        };
        let flash = self.flash.as_ref().map(|f| (f.lines.clone(), f.strength()));
        // A live edit's parts, in its frame's bytes.
        let live = self.live.as_ref().map(|l| {
            let pulse = if l.striking() { 0.75 + 0.25 * ((l.tick as f32) * 0.35).sin().abs() } else { 1. };
            (l.frame.waiting.clone(), l.frame.typed.clone(), l.frame.typing, pulse)
        });
        let state = self.state.clone();
        let (added, removed, panel) = (c.added_band, c.removed, p.panel);
        let (lit, struck, caret) = (c.added_band, c.removed_band, c.caret);
        let label_font = look::ui_font(cx);
        Some(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let mut quads: Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)> = Vec::new();
                    let mut labels: Vec<(gpui::Pixels, usize)> = Vec::new();
                    {
                        let s = state.read(cx);
                        let (Some(rows), Some(line_height)) = (s.visible_row_range(), s.line_height()) else {
                            return;
                        };
                        let text = s.text();
                        let lines = text.lines_len();
                        let map = s.display_map();
                        let first = map.display_row_to_buffer_line(rows.start);
                        let last =
                            (map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start)) + 1).min(lines);
                        let scroll_x = s.scroll_offset().x.min(px(0.));
                        let top = |line: usize| {
                            let at = text.line_start_offset(line.min(lines.saturating_sub(1)));
                            s.range_to_bounds(&(at..at)).map(|b| {
                                let y = if line >= lines { b.top() + line_height } else { b.top() };
                                (b.left() - scroll_x - px(4.), y)
                            })
                        };
                        let bottom = |line: usize| {
                            let end = text.line_end_offset(line.min(lines.saturating_sub(1)));
                            s.range_to_bounds(&(end..end)).map(|b| b.bottom())
                        };
                        let mut band = |run: &Range<usize>, color: gpui::Hsla| {
                            let (a, b) = (run.start.max(first), run.end.min(last));
                            if a >= b {
                                return;
                            }
                            if let (Some((x, y0)), Some(y1)) = (top(a), bottom(b - 1)) {
                                quads.push((gpui::Bounds::new(point(x, y0), size(bounds.right() - x, y1 - y0)), color));
                            }
                        };
                        // Bytes of the text to the lines they cover.
                        let line_run = |r: &Range<usize>| {
                            let a = text.offset_to_point(r.start.min(text.len())).row;
                            let end = r.end.min(text.len()).max(r.start);
                            let b = text.offset_to_point(end.saturating_sub(1).max(r.start)).row;
                            a..b + 1
                        };
                        for m in &marks {
                            band(&m.lines, added);
                        }
                        if let Some((waiting, typed, _, pulse)) = &live {
                            for r in waiting {
                                band(&line_run(r), struck.opacity(struck.a * pulse));
                            }
                            for r in typed {
                                band(&line_run(r), lit);
                            }
                        }
                        if let Some((runs, strength)) = &flash {
                            for run in runs {
                                // It starts brighter than the typed text's band, then fades.
                                band(run, lit.opacity((lit.a * 1.8).min(0.45) * strength));
                            }
                        }
                        for m in marks.iter().filter(|m| m.removed > 0) {
                            let at = m.lines.start;
                            if at < first || at > last {
                                continue;
                            }
                            let Some((x, y)) = top(at) else { continue };
                            let rule = gpui::Bounds::new(point(x, y - px(1.)), size(bounds.right() - x, px(2.)));
                            quads.push((rule, removed.opacity(0.8)));
                            labels.push((y, m.removed));
                        }
                        // The typing point: a bar the height of a line.
                        if let Some((_, _, Some(at), _)) = &live
                            && let Some(b) = s.range_to_bounds(&(*at..*at))
                        {
                            let x = b.left() - scroll_x;
                            quads.push((gpui::Bounds::new(point(x, b.top()), size(px(2.), line_height)), caret));
                        }
                    }
                    for (rect, color) in quads {
                        window.paint_quad(gpui::fill(rect, color));
                    }
                    let ts = window.text_system().clone();
                    for (y, n) in labels {
                        let label = if n == 1 { "1 line removed".to_owned() } else { format!("{n} lines removed") };
                        let run = TextRun {
                            len: label.len(),
                            font: gpui::font(label_font.clone()),
                            color: removed,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = ts.shape_line(label.into(), px(10.5), &[run], None);
                        let (w, h) = (shaped.width + px(10.), px(15.));
                        let chip_y = (y - h / 2.).max(bounds.top());
                        let chip = gpui::Bounds::new(point(bounds.right() - w - px(18.), chip_y), size(w, h));
                        window.paint_quad(gpui::fill(chip, mix(removed, 0.18, panel)).corner_radii(px(4.)));
                        let origin = point(chip.left() + px(5.), chip.top() + px(1.));
                        let _ = shaped.paint(origin, px(13.), TextAlign::Left, None, window, cx);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }

    /// The bar over a tab under review: what changes, and Accept and Reject.
    pub(super) fn review_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let r = self.review.as_ref()?;
        let p = palette(cx);
        let (plus, minus) = review::counts(&r.marks);
        let waiting = self.queue.len();
        let what = if r.streaming() {
            "Claude Code is writing a change…"
        } else if r.new_file {
            "Claude Code proposes a new file"
        } else {
            "Claude Code proposes a change"
        };
        let hint = |text: &'static str| div().text_color(p.fg_dim).text_size(px(11.)).child(text);
        Some(
            div()
                .id("review-bar")
                .debug_selector(|| "review-bar".into())
                .flex_none()
                .mx(px(8.))
                .mt(px(4.))
                .mb(px(4.))
                .pl(px(10.))
                .pr(px(6.))
                .h(px(34.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(6.))
                .bg(p.wash(p.accent))
                .border_1()
                .border_color(mix(p.accent, 0.4, p.panel))
                .font_family(look::ui_font(cx))
                .text_size(px(look::BODY))
                .child(crate::art::glyph(snout_core::art::Glyph::Spark, 16., p.accent))
                .child(div().whitespace_nowrap().child(what))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .font_family(look::mono_font(cx))
                        .text_size(px(12.))
                        .child(div().text_color(p.added).child(format!("+{plus}")))
                        .child(div().text_color(p.removed).child(format!("−{minus}"))),
                )
                .when(waiting > 0, |d| {
                    let text =
                        if waiting == 1 { "1 more waiting".to_owned() } else { format!("{waiting} more waiting") };
                    d.child(look::chip(text, p.accent, &p))
                })
                .child(div().flex_1())
                .child(
                    look::small_button("review-reject", crate::components::IconName::Ban, "Reject", &p)
                        .debug_selector(|| "review-reject".into())
                        .px(px(8.))
                        .h(px(24.))
                        .gap(px(6.))
                        .child(hint("Esc"))
                        .on_click(cx.listener(|this, _, w, cx| this.reject_review(w, cx))),
                )
                .child(
                    div()
                        .id("review-accept")
                        .debug_selector(|| "review-accept".into())
                        .flex_none()
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded(px(6.))
                        .bg(p.accent)
                        .text_color(p.on_accent)
                        .cursor_pointer()
                        .hover(move |s| s.bg(mix(p.accent, 0.88, p.fg)))
                        .child(crate::components::icon(crate::components::IconName::Check, 13., p.on_accent))
                        .child("Accept")
                        .child(div().text_size(px(11.)).opacity(0.8).child("⌘↩"))
                        .on_click(cx.listener(|this, _, w, cx| this.accept_review(w, cx))),
                ),
        )
    }
}

// ----- Live edits: Claude's own changes, played out ---------------------------------

impl EditorPanel {
    /// Claude changed the file to `after`: play it out from the text shown. `visible`
    /// says whether anyone sees the tab; a tab nobody sees takes the text at once.
    /// Returns false when there was nothing to show (the tab already had the text).
    pub fn animate_edit(&mut self, after: &str, visible: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.claude_hold = None;
        // The tab is still reading its file: the edit plays once it has it.
        if self.loading {
            let after = after.to_owned();
            self.after_load.push(Box::new(move |e, window, cx| {
                e.animate_edit(&after, visible, window, cx);
            }));
            return true;
        }
        if self.review.is_some() || self.dirty {
            return false;
        }
        self.end_live(window, cx);
        let original = self.text(cx);
        if original == after {
            self.take_disk(after, cx);
            return false;
        }
        self.preview_edit(&original, after, visible, window, cx);
        true
    }

    /// Show `before` in the tab first, then play the edit to `after` (a file just opened to
    /// follow Claude, which already holds `after`).
    pub fn animate_from(&mut self, before: &str, after: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.claude_hold = None;
        if self.loading {
            let (before, after) = (before.to_owned(), after.to_owned());
            self.after_load.push(Box::new(move |e, window, cx| e.animate_from(&before, &after, window, cx)));
            return;
        }
        if self.review.is_some() || self.dirty {
            return;
        }
        self.end_live(window, cx);
        let at = self.state.read(cx).selected_range();
        self.state.update(cx, |s, cx| s.set_value(before.to_owned(), window, cx));
        let at = at.start.min(before.len());
        self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        self.preview_edit(before, after, true, window, cx);
    }

    fn preview_edit(
        &mut self,
        original: &str,
        after: &str,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let plan = Stream::new(original, after);
        let lines: Vec<Range<usize>> = plan.hunks.iter().map(|h| lit_lines(h, after)).collect();
        let big = original.len().max(after.len()) > LIVE_MAX_TEXT || plan.chars > LIVE_MAX_CHARS;
        self.saved = after.to_owned();
        if !visible || big || plan.hunks.is_empty() {
            self.write_text(after, false, window, cx);
            self.settle(cx, window);
            if visible {
                self.flash(lines, window, cx);
            }
            return;
        }
        let strike = if plan.hunks.iter().any(|h| !h.old.is_empty()) {
            (STRIKE_TIME.as_millis() / LIVE_TICK.as_millis()) as u32
        } else {
            0
        };
        let ticks = (live_time(plan.hunks.len(), plan.chars).as_millis() / LIVE_TICK.as_millis()).max(1) as u32;
        let frame = plan.frame(original, after, 0);
        // The view goes to the first change; the caret too, when nobody is typing here.
        if let Some(h) = plan.hunks.first() {
            let at = h.old.start;
            self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        }
        self.live = Some(Live {
            plan,
            original: original.to_owned(),
            proposed: after.to_owned(),
            frame,
            tick: 0,
            strike,
            ticks,
            paused: false,
        });
        self.strike_marks(cx);
        self._live_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(LIVE_TICK).await;
                match this.update_in(cx, |this, window, cx| this.live_step(window, cx)) {
                    Ok(true) => {}
                    _ => return,
                }
            }
        }));
        cx.notify();
    }

    /// The next step of a live edit. False once it is over.
    fn live_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let now = self.text(cx);
        let Some(live) = self.live.as_mut() else { return false };
        if now != live.frame.text {
            // The user typed meanwhile: the rest goes in at once, with their edit.
            self.end_live(window, cx);
            return false;
        }
        if live.paused {
            return true;
        }
        live.tick += 1;
        if live.tick <= live.strike {
            cx.notify();
            return true;
        }
        let step = live.tick - live.strike;
        let total = live.plan.total;
        let typed = if step >= live.ticks { total } else { total * step as usize / live.ticks as usize };
        let frame = live.plan.frame(&live.original, &live.proposed, typed);
        let text = frame.text.clone();
        live.frame = frame;
        let done = typed >= total;
        let focused = gpui::Focusable::focus_handle(self.state.read(cx), cx).is_focused(window);
        self.write_text(&text, !focused, window, cx);
        self.strike_marks(cx);
        if done {
            self.end_live(window, cx);
            return false;
        }
        cx.notify();
        true
    }

    /// End a live edit now: the rest of it in (with what the user typed meanwhile), the
    /// new lines lit and fading.
    pub fn end_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.live.take() else { return };
        self._live_task = None;
        let now = self.text(cx);
        let text = review::carry(&live.frame, &live.proposed, &now);
        self.write_text(&text, false, window, cx);
        self.strike_marks(cx);
        let lines: Vec<Range<usize>> = live.plan.hunks.iter().map(|h| lit_lines(h, &live.proposed)).collect();
        self.settle(cx, window);
        self.light(lines, window, cx);
    }

    /// Hold a live edit `fraction` of the way through (0 the struck text alone, 1 all
    /// typed), for a look at it part way.
    pub fn hold_live_at(&mut self, fraction: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.live.as_mut() else { return };
        live.paused = true;
        let typed = ((live.plan.total as f32) * fraction.clamp(0., 1.)) as usize;
        live.tick = if fraction <= 0. { 0 } else { live.strike + 1 };
        live.frame = live.plan.frame(&live.original, &live.proposed, typed);
        let text = live.frame.text.clone();
        self.write_text(&text, false, window, cx);
        self.strike_marks(cx);
        cx.notify();
    }

    /// The text is the file's again: one undo step no more (the edit came from outside),
    /// the caret and the view where they were.
    fn settle(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let text = self.text(cx);
        self.state.update(cx, |s, cx| {
            let (sel, scroll) = (s.selected_range(), s.scroll_offset());
            s.set_value(text.clone(), window, cx);
            s.set_selected_range(sel, cx);
            s.set_scroll_offset(scroll, cx);
        });
        let dirty = text != self.saved;
        self.take_disk_stamp(cx);
        self.dirty = dirty;
        self.revision += 1;
        self.diff_soon(cx);
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// The file on disk holds `text`, which the tab shows.
    fn take_disk(&mut self, text: &str, cx: &mut Context<Self>) {
        self.saved = text.to_owned();
        self.dirty = false;
        self.take_disk_stamp(cx);
        cx.notify();
    }

    /// The struck text of the live edit, as the editor draws it.
    fn strike_marks(&mut self, cx: &mut Context<Self>) {
        let ranges = self.live.as_ref().map(|l| l.frame.waiting.clone()).unwrap_or_default();
        if ranges.is_empty() && self.struck.is_none() {
            return;
        }
        let c = palette(cx).claude();
        let style = gpui::HighlightStyle {
            color: Some(c.removed_text),
            strikethrough: Some(gpui::StrikethroughStyle { thickness: px(1.5), color: Some(c.removed) }),
            ..Default::default()
        };
        let decorations: Vec<gpui_component::input::TextDecoration> =
            ranges.into_iter().map(|r| gpui_component::input::TextDecoration::new(r, style)).collect();
        match &self.struck {
            Some(struck) => struck.set(decorations, cx),
            None => {
                let struck = self.state.update(cx, |s, cx| s.create_decorations_collection(decorations, cx));
                self.struck = Some(struck);
            }
        }
    }

    /// Light `lines` as a fading light is `step` steps in, and hold it there (for a look
    /// at it part way).
    pub fn hold_flash(&mut self, lines: Vec<Range<usize>>, step: u32, cx: &mut Context<Self>) {
        self._flash_task = None;
        self.flash = Some(Flash { lines, step });
        cx.notify();
    }

    /// Light `lines` and let them fade, the caret staying where it is.
    pub fn light(&mut self, lines: Vec<Range<usize>>, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.state.read(cx).selected_range();
        self.flash(lines, window, cx);
        self.state.update(cx, |s, cx| s.set_selected_range(at, cx));
    }

    // ----- Claude's unreviewed changes: the gutter's marks, the inline diff --------------

    /// Mark the changes made since `base` (none: none to review).
    pub fn set_claude_base(&mut self, base: Option<Arc<String>>, cx: &mut Context<Self>) {
        let same = match (&self.claude_base, &base) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.claude_base = base;
        if self.claude_base.is_none() {
            self.claude_diff = false;
        }
        self.diff_now(cx);
        cx.notify();
    }

    /// The hunk of Claude's changes at `line`, or the next one after it (`dir` > 0) or
    /// before it (< 0).
    pub fn claude_hunk_near(&self, line: usize, dir: i32) -> Option<&Hunk> {
        let mut hunks = self.claude_hunks.iter();
        match dir {
            0 => hunks.find(|h| h.new_lines.contains(&line) || (h.new_lines.is_empty() && h.new_lines.start == line)),
            d if d > 0 => hunks.find(|h| h.new_lines.start > line),
            _ => hunks.rev().find(|h| h.new_lines.start < line),
        }
    }

    fn caret_line(&self, cx: &App) -> usize {
        self.state.read(cx).cursor_position().line as usize
    }

    /// Go to Claude's next (or previous) change in this file; at the end, ask for the next
    /// file's.
    pub fn step_claude_change(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let line = self.caret_line(cx);
        let next = self.claude_hunk_near(line, if forward { 1 } else { -1 }).map(|h| h.new_lines.start);
        match next {
            Some(line) => self.go_to_claude_line(line, window, cx),
            None => {
                cx.emit(EditorEvent::Claude(if forward { ClaudeRequest::NextFile } else { ClaudeRequest::PrevFile }))
            }
        }
    }

    /// Put the caret on `line` (a hunk's first) and show the change inline.
    pub fn go_to_claude_line(&mut self, line: usize, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.state.read(cx).text().line_start_offset(line.min(self.line_count(cx).saturating_sub(1)));
        self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        let _ = window;
        self.claude_diff = true;
        cx.notify();
    }

    /// The hunk at the caret: keep it (`accept`) or put it back.
    fn claude_hunk_request(&mut self, accept: bool, cx: &mut Context<Self>) {
        let line = self.caret_line(cx);
        let Some(h) = self.claude_hunk_near(line, 0) else { return };
        let lines = h.new_lines.clone();
        cx.emit(EditorEvent::Claude(if accept {
            ClaudeRequest::AcceptHunk(lines)
        } else {
            ClaudeRequest::RevertHunk(lines)
        }));
    }

    /// The Claude change keys, in this tab.
    pub(super) fn claude_actions(&self, d: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        use crate::claude_ide::{AcceptClaudeHunk, NextClaudeChange, PrevClaudeChange, RevertClaudeHunk};
        d.on_action(cx.listener(|this, _: &NextClaudeChange, w, cx| this.step_claude_change(true, w, cx)))
            .on_action(cx.listener(|this, _: &PrevClaudeChange, w, cx| this.step_claude_change(false, w, cx)))
            .on_action(cx.listener(|this, _: &AcceptClaudeHunk, _, cx| this.claude_hunk_request(true, cx)))
            .on_action(cx.listener(|this, _: &RevertClaudeHunk, _, cx| this.claude_hunk_request(false, cx)))
    }

    /// The gutter's tooltip over a Claude mark.
    pub(super) fn claude_tip_at(&self, at: gpui::Point<gpui::Pixels>) -> Option<(u32, String)> {
        let painted = self.claude_painted.borrow();
        let (line, _) = painted.iter().find(|(_, b)| {
            let grown =
                gpui::Bounds::new(point(b.left() - px(3.), b.top()), size(b.size.width + px(6.), b.size.height));
            grown.contains(&at)
        })?;
        Some((*line as u32, "Changed by Claude, not reviewed yet\n⌥⌘Y keeps the change, ⌥⌘Z reverts it".to_owned()))
    }

    /// The bar over a tab showing Claude's changes inline: how much, keep or revert them.
    pub(super) fn claude_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if !self.claude_diff || self.review.is_some() || self.claude_base.is_none() {
            return None;
        }
        let p = palette(cx);
        let c = p.claude();
        let (plus, minus) =
            self.claude_hunks.iter().fold((0, 0), |(a, r), h| (a + h.new_lines.len(), r + h.old_lines.len()));
        let n = self.claude_hunks.len();
        let what = match n {
            0 => "Claude's changes, all reviewed".to_owned(),
            1 => "Claude changed this file: 1 change to review".to_owned(),
            n => format!("Claude changed this file: {n} changes to review"),
        };
        let hint = |text: &'static str| div().text_color(p.fg_dim).text_size(px(11.)).child(text);
        Some(
            div()
                .id("claude-bar")
                .debug_selector(|| "claude-bar".into())
                .flex_none()
                .mx(px(8.))
                .mt(px(4.))
                .mb(px(4.))
                .pl(px(10.))
                .pr(px(6.))
                .h(px(34.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(6.))
                .bg(c.mark_wash)
                .border_1()
                .border_color(mix(c.mark, 0.4, p.panel))
                .font_family(look::ui_font(cx))
                .text_size(px(look::BODY))
                .child(crate::art::glyph(snout_core::art::Glyph::Spark, 16., c.mark))
                .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().min_w_0().child(what))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .font_family(look::mono_font(cx))
                        .text_size(px(12.))
                        .child(div().text_color(c.added).child(format!("+{plus}")))
                        .child(div().text_color(c.removed).child(format!("−{minus}"))),
                )
                .child(div().flex_1())
                .child(hint("⌃⌥⇧↓ next"))
                .child(
                    look::small_button("claude-revert-file", crate::components::IconName::Undo2, "Revert File", &p)
                        .debug_selector(|| "claude-revert-file".into())
                        .h(px(24.))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Claude(ClaudeRequest::RevertFile)))),
                )
                .child(
                    div()
                        .id("claude-accept-file")
                        .debug_selector(|| "claude-accept-file".into())
                        .flex_none()
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded(px(6.))
                        .bg(p.accent)
                        .text_color(p.on_accent)
                        .cursor_pointer()
                        .hover(move |s| s.bg(mix(p.accent, 0.88, p.fg)))
                        .child(crate::components::icon(crate::components::IconName::Check, 13., p.on_accent))
                        .child("Accept File")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Claude(ClaudeRequest::AcceptFile)))),
                )
                .child(
                    look::icon_button(
                        "claude-bar-close",
                        snout_core::art::Glyph::Close,
                        "Hide the inline changes",
                        true,
                        &p,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.claude_diff = false;
                        cx.notify();
                    })),
                ),
        )
    }
}

/// The lines of `after` a hunk's new text takes, or the line where its old text went.
fn lit_lines(h: &Hunk, after: &str) -> Range<usize> {
    if h.new_lines.is_empty() {
        let count = after.split_inclusive('\n').count().max(1);
        let at = h.new_lines.start.min(count - 1);
        at..at + 1
    } else {
        h.new_lines.clone()
    }
}
