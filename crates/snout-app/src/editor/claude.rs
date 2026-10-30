//! Coding agents in an editor tab. A change Claude proposes types itself into the tab, hunk
//! by hunk, and waits there for Accept (⌘↩) or Reject (Esc): new lines lit green, removed
//! ones marked red where they were. A change an agent makes on its own plays out live, as
//! a collaborator's would: its caret, in the agent's colour with a flag of its name, moves
//! through the text as it types in, and the new text fades from the agent's colour to
//! normal over a second. Only the lines being written change; the rest of the tab stays as
//! it is (its colours, its view, its marks). An edit off screen shows a chip at the edge
//! and a mark on the scrollbar; the gutter marks the lines until the change is reviewed.

use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
/// How long new text takes to fade from the agent's colour to normal.
pub const FADE: Duration = Duration::from_millis(1_000);
/// How long the agent's caret stays where it stopped writing, fading over the last part.
pub const LINGER: Duration = Duration::from_millis(1_600);
/// How long the caret takes to glide to where the typing is.
const GLIDE: Duration = Duration::from_millis(90);
/// One step of the fading.
const FADE_TICK: Duration = Duration::from_millis(33);
/// The new text's band at its strongest.
const FADE_ALPHA: f32 = 0.24;
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
    /// Ask the console's agent to fix a problem on these lines (0-based, inclusive): what
    /// it is asked after the mention of the file and lines.
    AskToFix {
        lines: (u32, u32),
        text: String,
    },
}

/// Who writes in a tab: the agent's name as its caret's flag says it, and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Writer {
    pub name: gpui::SharedString,
    pub color: gpui::Hsla,
}

impl Writer {
    pub fn of(agent: &crate::agents::Agent, p: &crate::theme::Palette) -> Self {
        Self { name: agent.short.clone(), color: agent.color(p) }
    }
}

/// An edit an agent made on its own, playing out in the tab.
pub struct Live {
    plan: Stream,
    pub original: String,
    pub proposed: String,
    pub frame: Frame,
    tick: u32,
    ticks: u32,
    /// Held where it is (a screenshot).
    paused: bool,
    pub writer: Writer,
    /// The lines typed so far, and when each was last written to.
    lit: Vec<(usize, Instant)>,
}

/// An agent's caret in the tab: where it writes (or stopped writing), and since when it
/// has stopped.
#[derive(Clone, Debug)]
pub struct Presence {
    pub writer: Writer,
    pub at: usize,
    pub left: Option<Instant>,
}

impl Presence {
    /// How much of it shows: all while it writes, fading out at the end of its linger.
    pub fn strength(&self) -> f32 {
        match self.left {
            None => 1.,
            Some(t) => {
                let e = t.elapsed().as_secs_f32() / LINGER.as_secs_f32();
                ((1. - e) * 2.5).clamp(0., 1.)
            }
        }
    }

    pub fn gone(&self) -> bool {
        self.left.is_some_and(|t| t.elapsed() >= LINGER)
    }
}

/// Where the agent's caret was drawn, gliding to where the typing is.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glide {
    from: Option<gpui::Point<gpui::Pixels>>,
    to: Option<gpui::Point<gpui::Pixels>>,
    since: Option<Instant>,
}

impl Glide {
    /// The caret's place now, heading for `target`.
    fn toward(&mut self, target: gpui::Point<gpui::Pixels>) -> gpui::Point<gpui::Pixels> {
        if self.to != Some(target) {
            self.from = Some(self.now().unwrap_or(target));
            self.to = Some(target);
            self.since = Some(Instant::now());
        }
        self.now().unwrap_or(target)
    }

    fn now(&self) -> Option<gpui::Point<gpui::Pixels>> {
        let (from, to, since) = (self.from?, self.to?, self.since?);
        let t = (since.elapsed().as_secs_f32() / GLIDE.as_secs_f32()).clamp(0., 1.);
        let t = 1. - (1. - t) * (1. - t);
        Some(gpui::point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t))
    }
}

/// How strong the band of a line written at `at` is now.
fn fade(at: Instant) -> f32 {
    let t = (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).clamp(0., 1.);
    FADE_ALPHA * (1. - t) * (1. - t)
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

/// Lines a followed edit changed (or lines sent to an agent), fading.
pub struct Flash {
    pub lines: Vec<Range<usize>>,
    pub step: u32,
    /// The colour they light in; none: the new text's.
    pub color: Option<gpui::Hsla>,
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

    /// Take `text` from elsewhere (the file changed on disk): only what differs, the caret
    /// and the view kept, the tab no longer differing from the file.
    pub fn write_quietly(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.write_text(text, false, window, cx);
        self.settle(cx, window);
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
        self.flash = Some(Flash { lines, step: 0, color: None });
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

    /// The bands over the text: a proposed change's new lines green, a red rule where lines
    /// went (with how many), the fading light of a followed edit. An agent at work: the
    /// lines it writes in its colour, fading to normal, and its caret with the flag of its
    /// name gliding to where the typing is; marks on the scrollbar where it is.
    pub(super) fn claude_overlay(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let inline = self.review.is_none() && self.claude_diff && self.claude_base.is_some();
        let presence = self.presence.clone().filter(|p| !p.gone());
        // The lines written: the live edit's (the one being typed on at full strength), and
        // those still fading after it.
        let mut written: Vec<(usize, f32, gpui::Hsla)> =
            self.fades.iter().map(|(l, at, c)| (*l, fade(*at), *c)).filter(|(_, a, _)| *a > 0.004).collect();
        if let Some(live) = self.live.as_ref() {
            let caret_line = presence.as_ref().map(|p| {
                let t = &live.frame.text;
                t[..p.at.min(t.len())].matches('\n').count()
            });
            for (l, at) in &live.lit {
                let a = if Some(*l) == caret_line { FADE_ALPHA } else { fade(*at) };
                written.push((*l, a, live.writer.color));
            }
        }
        let tint = self.agent_tint(cx);
        if self.review.is_none()
            && self.flash.is_none()
            && presence.is_none()
            && written.is_empty()
            && tint.is_empty()
            && !inline
        {
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
        let flash = self.flash.as_ref().map(|f| (f.lines.clone(), f.strength(), f.color));
        let state = self.state.clone();
        let glide = self.glide.clone();
        let (added, removed, panel) = (c.added_band, c.removed, p.panel);
        let lit = c.added_band;
        let label_font = look::ui_font(cx);
        let flag_ink = gpui::white();
        Some(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let mut quads: Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)> = Vec::new();
                    let mut labels: Vec<(gpui::Pixels, usize)> = Vec::new();
                    let mut caret: Option<(gpui::Point<gpui::Pixels>, gpui::Pixels, Presence)> = None;
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
                        for (run, color) in &tint {
                            band(run, *color);
                        }
                        for m in &marks {
                            band(&m.lines, added);
                        }
                        for (l, a, color) in &written {
                            band(&(*l..*l + 1), color.opacity(*a));
                        }
                        if let Some((runs, strength, color)) = &flash {
                            let (c, a) = match color {
                                Some(c) => (*c, 0.28),
                                None => (lit, (lit.a * 1.8).min(0.45)),
                            };
                            for run in runs {
                                band(run, c.opacity(a * strength));
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
                        // On the scrollbar: where the agent writes, and its lines to review.
                        let total = lines.max(1) as f32;
                        let tick = |line: usize, h: f32| {
                            let y = bounds.top() + (bounds.size.height - px(h)) * (line as f32 / total);
                            gpui::Bounds::new(point(bounds.right() - px(9.), y), size(px(7.), px(h)))
                        };
                        for (run, color) in &tint {
                            quads.push((tick(run.start, 3.), color.opacity(0.85)));
                        }
                        if let Some(pr) = &presence {
                            let at = pr.at.min(text.len());
                            let line = text.offset_to_point(at).row;
                            quads.push((tick(line, 4.), pr.writer.color.opacity(pr.strength())));
                            if (first..last.max(first + 1)).contains(&line)
                                && let Some(b) = s.range_to_bounds(&(at..at))
                            {
                                caret = Some((point(b.left() - scroll_x, b.top()), line_height, pr.clone()));
                            }
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
                    // The agent's caret, gliding, and the flag of its name above it (below,
                    // on the first line showing).
                    if let Some((target, line_height, pr)) = caret {
                        let at = glide.borrow_mut().toward(target);
                        let alpha = pr.strength();
                        let color = pr.writer.color.opacity(alpha);
                        window.paint_quad(gpui::fill(gpui::Bounds::new(at, size(px(2.), line_height)), color));
                        let mut font = gpui::font(label_font.clone());
                        font.weight = gpui::FontWeight::SEMIBOLD;
                        let name = pr.writer.name.to_string();
                        let run = TextRun {
                            len: name.len(),
                            font,
                            color: flag_ink.opacity(alpha),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = ts.shape_line(name.into(), px(10.5), &[run], None);
                        let (w, h) = (shaped.width + px(10.), px(15.));
                        let above = at.y - h >= bounds.top();
                        let y = if above { at.y - h } else { at.y + line_height };
                        let flag = gpui::Bounds::new(point(at.x, y), size(w, h));
                        let radii = if above {
                            gpui::Corners {
                                top_left: px(4.),
                                top_right: px(4.),
                                bottom_right: px(4.),
                                bottom_left: px(0.),
                            }
                        } else {
                            gpui::Corners {
                                top_left: px(0.),
                                top_right: px(4.),
                                bottom_right: px(4.),
                                bottom_left: px(4.),
                            }
                        };
                        window.paint_quad(gpui::fill(flag, color).corner_radii(radii));
                        let _ = shaped.paint(
                            point(flag.left() + px(5.), flag.top() + px(1.)),
                            px(13.),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    } else {
                        *glide.borrow_mut() = Glide::default();
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }

    /// The chip at the editor's edge while the agent writes out of sight: where, and a
    /// click goes there.
    pub fn presence_chip(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let pr = self.presence.as_ref().filter(|p| !p.gone())?;
        let s = self.state.read(cx);
        let rows = s.visible_row_range()?;
        let text = s.text();
        let lines = text.lines_len();
        let map = s.display_map();
        let first = map.display_row_to_buffer_line(rows.start);
        let last = (map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start)) + 1).min(lines);
        let at = pr.at.min(text.len());
        let line = text.offset_to_point(at).row;
        if (first..last).contains(&line) {
            return None;
        }
        let up = line < first;
        let p = palette(cx);
        let color = pr.writer.color;
        let what = if pr.left.is_none() { "is editing" } else { "edited" };
        let label = format!("{} {what} line {} {}", pr.writer.name, line + 1, if up { "\u{2191}" } else { "\u{2193}" });
        let chip = div()
            .id("agent-chip")
            .debug_selector(|| "agent-chip".into())
            .h(px(26.))
            .pl(px(8.))
            .pr(px(10.))
            .flex()
            .items_center()
            .gap(px(7.))
            .rounded(px(13.))
            .bg(p.card)
            .border_1()
            .border_color(mix(color, 0.55, p.border))
            .shadow_md()
            .font_family(look::ui_font(cx))
            .text_size(px(12.))
            .text_color(p.fg)
            .cursor_pointer()
            .child(div().flex_none().size(px(8.)).rounded_full().bg(color))
            .child(div().whitespace_nowrap().child(label))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
                cx.notify();
            }));
        Some(div().absolute().right(px(28.)).when(up, |d| d.top(px(8.))).when(!up, |d| d.bottom(px(10.))).child(chip))
    }

    /// The soft tint of the lines agents changed and nobody has reviewed yet, in each
    /// agent's colour (Claude's when the ledger does not say).
    pub fn agent_tint(&self, cx: &App) -> Vec<(Range<usize>, gpui::Hsla)> {
        if self.review.is_some() || self.claude_base.is_none() || self.live.is_some() {
            return Vec::new();
        }
        let p = palette(cx);
        let a = if p.dark { 0.10 } else { 0.075 };
        self.claude_hunks
            .iter()
            .filter(|h| !h.new_lines.is_empty())
            .map(|h| (h.new_lines.clone(), self.agent_color_of(h.new_lines.start, cx).opacity(a)))
            .collect()
    }

    /// The dot on the tab: an agent writing here (its colour, pulsing), or changes of an
    /// agent's waiting for review (or made while the editor did not follow).
    pub fn agent_dot(&self, cx: &App) -> Option<(gpui::Hsla, bool)> {
        if let Some(p) = self.presence.as_ref().filter(|p| p.left.is_none()) {
            return Some((p.writer.color, true));
        }
        let unreviewed = self.review.is_none() && self.claude_base.is_some() && !self.claude_hunks.is_empty();
        (self.touched || unreviewed).then(|| (self.agent_color_of(0, cx), false))
    }

    /// The colour of the agent behind `line` (Claude's when nobody says).
    pub fn agent_color_of(&self, line: usize, cx: &App) -> gpui::Hsla {
        let p = palette(cx);
        let _ = line;
        self.claude_agent.as_ref().map_or(p.claude().mark, |a| a.color(&p))
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
    pub fn animate_edit(
        &mut self,
        after: &str,
        visible: bool,
        writer: Option<Writer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.claude_hold = None;
        // The tab is still reading its file: the edit plays once it has it.
        if self.loading {
            let after = after.to_owned();
            self.after_load.push(Box::new(move |e, window, cx| {
                e.animate_edit(&after, visible, writer, window, cx);
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
        self.preview_edit(&original, after, visible, writer, window, cx);
        true
    }

    /// Show `before` in the tab first, then play the edit to `after` (a file just opened to
    /// follow Claude, which already holds `after`).
    pub fn animate_from(
        &mut self,
        before: &str,
        after: &str,
        writer: Option<Writer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.claude_hold = None;
        if self.loading {
            let (before, after) = (before.to_owned(), after.to_owned());
            self.after_load.push(Box::new(move |e, window, cx| e.animate_from(&before, &after, writer, window, cx)));
            return;
        }
        if self.review.is_some() || self.dirty {
            return;
        }
        self.end_live(window, cx);
        // Only what differs is put back (never the whole text: that would light the tab
        // up afresh, its colours and marks gone for a moment).
        let at = self.state.read(cx).selected_range();
        self.write_text(before, false, window, cx);
        let at = at.start.min(before.len());
        self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        self.preview_edit(before, after, true, writer, window, cx);
    }

    fn preview_edit(
        &mut self,
        original: &str,
        after: &str,
        visible: bool,
        writer: Option<Writer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let plan = Stream::new(original, after);
        let lines: Vec<Range<usize>> = plan.hunks.iter().map(|h| lit_lines(h, after)).collect();
        let big = original.len().max(after.len()) > LIVE_MAX_TEXT || plan.chars > LIVE_MAX_CHARS;
        self.saved = after.to_owned();
        let writer = writer.filter(|_| visible);
        if writer.is_none() || big || plan.hunks.is_empty() {
            self.write_text(after, false, window, cx);
            self.settle(cx, window);
            if let Some(w) = writer {
                // At once (too big to type in), the new lines fading all the same.
                let now = Instant::now();
                self.fade_lines(lines.iter().flat_map(|r| r.clone()).map(|l| (l, now)), w.color, cx);
                let at = plan.hunks.last().map_or(0, |h| h.new.end.min(after.len()));
                self.presence = Some(Presence { writer: w, at, left: Some(now) });
                self.run_fades(cx);
            }
            return;
        }
        let Some(writer) = writer else { return };
        let ticks = (live_time(plan.hunks.len(), plan.chars).as_millis() / LIVE_TICK.as_millis()).max(1) as u32;
        let frame = plan.frame(original, after, 0);
        // The view goes to the first change; the caret too, when nobody is typing here.
        let follow = self.follows_agents(cx);
        if let Some(h) = plan.hunks.first() {
            let at = h.old.start;
            if follow {
                self.state.update(cx, |s, cx| s.set_selected_range(at..at, cx));
            }
            self.presence = Some(Presence { writer: writer.clone(), at, left: None });
        }
        self.live = Some(Live {
            plan,
            original: original.to_owned(),
            proposed: after.to_owned(),
            frame,
            tick: 0,
            ticks,
            paused: false,
            writer,
            lit: Vec::new(),
        });
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
        let step = live.tick;
        let total = live.plan.total;
        let typed = if step >= live.ticks { total } else { total * step as usize / live.ticks as usize };
        let frame = live.plan.frame(&live.original, &live.proposed, typed);
        let text = frame.text.clone();
        live.frame = frame;
        let done = typed >= total;
        // Follow mode: the view follows the typing, unless someone types here.
        let follow = self.follows_agents(cx);
        self.write_text(&text, follow, window, cx);
        self.note_typing(cx);
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
        self.settle(cx, window);
        // The lines typed fade on from when they were written; any left (the user typed and
        // the rest went in at once) from now.
        let stamp = Instant::now();
        let mut lit = live.lit.clone();
        for h in &live.plan.hunks {
            for l in lit_lines(h, &live.proposed) {
                if !lit.iter().any(|(x, _)| *x == l) {
                    lit.push((l, stamp));
                }
            }
        }
        self.fade_lines(lit, live.writer.color, cx);
        let at = self.presence.as_ref().map_or(0, |p| p.at).min(text.len());
        self.presence = Some(Presence { writer: live.writer, at, left: Some(stamp) });
        self.run_fades(cx);
    }

    /// Whether the view follows an agent's typing: Follow is on, and nobody types here.
    fn follows_agents(&self, cx: &App) -> bool {
        let typing = self.edited_at.is_some_and(|t| t.elapsed() < Duration::from_millis(1_500));
        !typing && cx.try_global::<crate::agents::Following>().is_none_or(|f| f.0)
    }

    /// Where the typing is now: the agent's caret goes there, and the line it writes on is
    /// lit (its band fades once the typing moves on).
    fn note_typing(&mut self, _: &mut Context<Self>) {
        let Some(live) = self.live.as_mut() else { return };
        let text = &live.frame.text;
        let at = live.frame.typing.or_else(|| live.frame.typed.last().map(|r| r.end)).unwrap_or(0).min(text.len());
        let line_of = |offset: usize| text[..offset.min(text.len())].matches('\n').count();
        let now = Instant::now();
        let caret_line = line_of(at);
        for r in &live.frame.typed {
            let (a, b) = (line_of(r.start), line_of(r.end.saturating_sub(1).max(r.start)));
            for l in a..=b {
                match live.lit.iter_mut().find(|(x, _)| *x == l) {
                    Some(e) if l == caret_line => e.1 = now,
                    Some(_) => {}
                    None => live.lit.push((l, now)),
                }
            }
        }
        if let Some(p) = self.presence.as_mut() {
            p.at = at;
            p.left = None;
        }
    }

    /// Light `lines` (each with when it was written) in `color`, to fade to normal.
    fn fade_lines(
        &mut self,
        lines: impl IntoIterator<Item = (usize, Instant)>,
        color: gpui::Hsla,
        _: &mut Context<Self>,
    ) {
        for (l, at) in lines {
            self.fades.retain(|(x, _, _)| *x != l);
            self.fades.push((l, at, color));
        }
    }

    /// Keep the tab painting while lines fade and the agent's caret lingers.
    fn run_fades(&mut self, cx: &mut Context<Self>) {
        if self._fade_task.is_some() {
            return;
        }
        self._fade_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FADE_TICK).await;
                let more = this.update(cx, |this, cx| {
                    this.fades.retain(|(_, at, _)| at.elapsed() < FADE);
                    if this.presence.as_ref().is_some_and(Presence::gone) {
                        this.presence = None;
                    }
                    cx.notify();
                    let more = !this.fades.is_empty() || this.presence.is_some() || this.live.is_some();
                    if !more {
                        this._fade_task = None;
                    }
                    more
                });
                if !matches!(more, Ok(true)) {
                    return;
                }
            }
        }));
    }

    /// The line the agent writes on (or stopped on), if one does.
    pub fn presence_line(&self, cx: &App) -> Option<usize> {
        let p = self.presence.as_ref()?;
        let text = self.state.read(cx).text();
        Some(text.offset_to_point(p.at.min(text.len())).row)
    }

    /// Hold a live edit `fraction` of the way through (0 the struck text alone, 1 all
    /// typed), for a look at it part way.
    pub fn hold_live_at(&mut self, fraction: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.live.as_mut() else { return };
        live.paused = true;
        let typed = ((live.plan.total as f32) * fraction.clamp(0., 1.)) as usize;
        live.tick = 1;
        live.frame = live.plan.frame(&live.original, &live.proposed, typed);
        let text = live.frame.text.clone();
        self.write_text(&text, false, window, cx);
        self.note_typing(cx);
        cx.notify();
    }

    /// The text is the file's again. The text itself is never set afresh: that would drop
    /// the tab's highlighting, the language server's colours and its marks, and repaint the
    /// whole file (the blinking a followed edit used to cause). Only the lines written
    /// changed; the edit stays in the tab's undo history.
    fn settle(&mut self, cx: &mut Context<Self>, _window: &mut Window) {
        let text = self.text(cx);
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

    /// Light `lines` as a fading light is `step` steps in, and hold it there (for a look
    /// at it part way).
    pub fn hold_flash(&mut self, lines: Vec<Range<usize>>, step: u32, cx: &mut Context<Self>) {
        self._flash_task = None;
        self.flash = Some(Flash { lines, step, color: None });
        cx.notify();
    }

    /// Light `lines` in `color` and let them fade, the caret and the view staying where they
    /// are (the lines just sent to an agent).
    pub fn light_in(
        &mut self,
        lines: Vec<Range<usize>>,
        color: gpui::Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.light(lines, window, cx);
        if let Some(f) = self.flash.as_mut() {
            f.color = Some(color);
        }
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
            .on_action(cx.listener(|this, _: &crate::claude_ide::AnnotateWithAgents, _, cx| this.toggle_annotate(cx)))
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

// ----- Who wrote which lines: the agents' lane in the gutter, its card, Annotate ----------

/// A run of lines in the agents' lane: the lines (empty where lines were only taken out),
/// the agent's colour and mark, whether they still wait for review, and the ledger's edit.
#[derive(Clone, Debug)]
pub struct LaneRun {
    pub lines: Range<usize>,
    pub color: gpui::Hsla,
    pub glyph: char,
    pub unreviewed: bool,
    pub edit: Option<u32>,
}

/// The card of an agent's mark: the run it is about, and where the pointer found it.
#[derive(Clone, Debug)]
pub struct AgentCard {
    pub lines: Range<usize>,
    pub edit: Option<u32>,
    pub at: gpui::Point<gpui::Pixels>,
}

/// A run of the lane as drawn: its lines, its edit, where (for the pointer).
pub type LanePainted = (Range<usize>, Option<u32>, gpui::Bounds<gpui::Pixels>);

/// Width of the lane's bar, and how far left of the code it sits (left of Git's bar).
const LANE_W: f32 = 3.;
const LANE_GAP: f32 = 14.;

/// Draw the agents' lane: a bar in each agent's colour beside the lines it wrote (full
/// while they wait for review, paler after), a bead with the agent's mark where each run
/// begins, a wedge where it took lines out. Returns where each run went, for the pointer.
pub fn paint_lane(
    state: &gpui::Entity<gpui_component::input::EditorState>,
    lane: &[LaneRun],
    panel: gpui::Hsla,
    font: &gpui::SharedString,
    window: &mut Window,
    cx: &mut App,
) -> Vec<LanePainted> {
    let mut out = Vec::new();
    // Where each run goes, worked out first; then painted (the beads' marks need `cx`).
    let mut beads: Vec<(gpui::Point<gpui::Pixels>, gpui::Hsla, char)> = Vec::new();
    let state = state.read(cx);
    let (Some(rows), Some(line_height)) = (state.visible_row_range(), state.line_height()) else { return out };
    let text = state.text();
    let lines = text.lines_len();
    let map = state.display_map();
    let first = map.display_row_to_buffer_line(rows.start);
    let last = (map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start)) + 1).min(lines);
    let scroll_x = state.scroll_offset().x.min(px(0.));
    let top_of = |line: usize| {
        let at = text.line_start_offset(line.min(lines.saturating_sub(1)));
        state.range_to_bounds(&(at..at)).map(|b| {
            let x = b.left() - scroll_x - px(LANE_GAP + LANE_W);
            if line >= lines { (x, b.top() + line_height) } else { (x, b.top()) }
        })
    };
    for run in lane {
        let color = if run.unreviewed { run.color } else { run.color.opacity(0.45) };
        if run.lines.is_empty() {
            let at = run.lines.start;
            if !(first..=last).contains(&at) {
                continue;
            }
            let Some((x, y)) = top_of(at) else { continue };
            let b = gpui::Bounds::new(point(x - px(1.), y - px(4.)), size(px(6.), px(8.)));
            let mut path = gpui::PathBuilder::fill();
            path.move_to(b.origin);
            path.line_to(point(b.right(), b.center().y));
            path.line_to(b.bottom_left());
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
            out.push((run.lines.clone(), run.edit, b));
            continue;
        }
        let (a, b) = (run.lines.start.max(first), run.lines.end.min(last));
        if a >= b {
            continue;
        }
        let (Some((x, top)), Some((_, bottom))) = (top_of(a), top_of(b - 1)) else { continue };
        let bar = gpui::Bounds::new(point(x, top + px(1.)), size(px(LANE_W), bottom + line_height - top - px(2.)));
        window.paint_quad(gpui::fill(bar, color).corner_radii(px(1.5)));
        // The bead with the agent's mark, where the run begins.
        if run.lines.start >= first {
            beads.push((point(x + px(LANE_W / 2.), top + line_height / 2.), color, run.glyph));
        }
        let hit = gpui::Bounds::new(point(x - px(5.), top), size(px(LANE_W + 10.), bottom + line_height - top));
        out.push((run.lines.clone(), run.edit, hit));
    }
    let ts = window.text_system().clone();
    for (center, color, glyph) in beads {
        let r = px(5.5);
        let bead = gpui::Bounds::new(point(center.x - r, center.y - r), size(r * 2., r * 2.));
        window.paint_quad(gpui::fill(bead, panel).corner_radii(r));
        let inner = gpui::Bounds::new(
            point(center.x - r + px(1.), center.y - r + px(1.)),
            size(r * 2. - px(2.), r * 2. - px(2.)),
        );
        window.paint_quad(gpui::fill(inner, color).corner_radii(r - px(1.)));
        let glyph = glyph.to_string();
        let tr = TextRun {
            len: glyph.len(),
            font: gpui::font(font.clone()),
            color: gpui::white(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = ts.shape_line(glyph.into(), px(7.5), &[tr], None);
        let origin = point(center.x - shaped.width / 2., center.y - px(5.));
        let _ = shaped.paint(origin, px(10.), TextAlign::Left, None, window, cx);
    }
    out
}

impl EditorPanel {
    /// The agents' lane: the runs of lines the ledger puts down to an agent, and the
    /// unreviewed changes it has no record of (in the colour of the file's agent).
    pub fn agent_lane(&self, cx: &App) -> Vec<LaneRun> {
        if self.review.is_some() {
            return Vec::new();
        }
        let unreviewed: Vec<Range<usize>> = if self.claude_base.is_some() {
            self.claude_hunks.iter().map(|h| h.new_lines.clone()).collect()
        } else {
            Vec::new()
        };
        let overlaps = |r: &Range<usize>| unreviewed.iter().any(|u| u.start < r.end && r.start < u.end);
        let p = palette(cx);
        let mut out: Vec<LaneRun> = Vec::new();
        if let Some(o) = &self.agent_lines {
            for (lines, id) in o.runs() {
                let agent = self.agent_edits.get(id as usize).map(|e| crate::agents::by_id(&e.agent, cx));
                let agent = agent.or_else(|| self.claude_agent.clone()).unwrap_or_else(crate::agents::Agent::claude);
                let unreviewed = overlaps(&lines);
                out.push(LaneRun { color: agent.color(&p), glyph: agent.glyph, unreviewed, edit: Some(id), lines });
            }
        }
        let fallback = self.claude_agent.clone().unwrap_or_else(crate::agents::Agent::claude);
        for h in self.claude_hunks.iter().filter(|_| self.claude_base.is_some()) {
            let r = h.new_lines.clone();
            let covered = !r.is_empty() && out.iter().any(|o| o.lines.start <= r.start && r.end <= o.lines.end);
            if !covered {
                out.push(LaneRun {
                    lines: r,
                    color: fallback.color(&p),
                    glyph: fallback.glyph,
                    unreviewed: true,
                    edit: None,
                });
            }
        }
        out
    }

    /// The pointer moved over the editor: the card of the agent's mark under it shows, and
    /// stays while the pointer is on the card.
    pub(super) fn agent_hover(&mut self, at: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let hit = self.agent_painted.borrow().iter().find(|(_, _, b)| b.contains(&at)).cloned();
        match hit {
            Some((lines, edit, b)) => {
                if self.agent_card.as_ref().is_none_or(|c| c.lines != lines) {
                    self.agent_card = Some(AgentCard { lines, edit, at: point(b.right() + px(2.), b.top()) });
                    cx.notify();
                }
            }
            // The annotation column decides for its own rows.
            None if !self.card_hovered && self.agent_card.is_some() && !self.on_column(at) => {
                self.agent_card = None;
                cx.notify();
            }
            None => {}
        }
    }

    /// The change (hunk) of the agents' that the run `lines` is in.
    fn hunk_of(&self, lines: &Range<usize>) -> Option<Hunk> {
        self.claude_base.as_ref()?;
        self.claude_hunks
            .iter()
            .find(|h| {
                (h.new_lines.start < lines.end.max(lines.start + 1)
                    && lines.start < h.new_lines.end.max(h.new_lines.start + 1))
                    || h.new_lines.start == lines.start
            })
            .cloned()
    }

    /// The card of an agent's mark: who changed the lines and when, the prompt behind it,
    /// and Accept, Revert and Show Diff for that change.
    pub(super) fn agent_card_view(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use crate::components::IconName;
        let card = self.agent_card.as_ref()?;
        let p = palette(cx);
        let edit = card.edit.and_then(|id| self.agent_edits.get(id as usize)).cloned();
        let agent = edit
            .as_ref()
            .map(|e| crate::agents::by_id(&e.agent, cx))
            .or_else(|| self.claude_agent.clone())
            .unwrap_or_else(crate::agents::Agent::claude);
        let color = agent.color(&p);
        let when = edit.as_ref().map(|e| crate::claude_ide::ledger::clock(e.time()));
        let title = match &when {
            Some(t) => format!("Changed by {} at {t}", agent.short),
            None => format!("Changed by {}", agent.short),
        };
        let prompt = edit.as_ref().and_then(|e| e.prompt.clone());
        let hunk = self.hunk_of(&card.lines);
        let lines = card.lines.clone();
        let what = if lines.is_empty() {
            "Lines taken out here".to_owned()
        } else if lines.len() == 1 {
            format!("Line {}", lines.start + 1)
        } else {
            format!("Lines {}\u{2013}{}", lines.start + 1, lines.end)
        };
        let (h1, h2, h3) = (hunk.clone(), hunk.clone(), hunk.clone());
        let body = div()
            .id("agent-card")
            .debug_selector(|| "agent-card".into())
            .occlude()
            .w(px(320.))
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(7.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(mix(color, 0.45, p.border))
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .text_color(p.fg)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.card_hovered = *hovered;
                if !*hovered {
                    this.agent_card = None;
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_none()
                            .size(px(22.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(mix(color, 0.2, p.card))
                            .child(crate::components::icon(agent.icon, 13., color)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(gpui::FontWeight::MEDIUM).child(title))
                            .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(what)),
                    )
                    .when(hunk.is_none(), |d| d.child(look::chip("Reviewed", p.ok, &p))),
            )
            .when_some(prompt, |d, prompt| {
                d.child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(5.))
                        .bg(p.wash(color))
                        .child(crate::components::icon(IconName::MessageSquare, 12., color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(p.fg)
                                .child(format!("\u{201c}{prompt}\u{201d}")),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(6.))
                    .child(
                        look::small_button("agent-card-diff", IconName::GitCompare, "Show Diff", &p)
                            .debug_selector(|| "agent-card-diff".into())
                            .h(px(24.))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                let line = h1.as_ref().map_or(lines.start, |h| h.new_lines.start);
                                this.agent_card = None;
                                this.go_to_claude_line(line, w, cx);
                            })),
                    )
                    .when_some(h2, |d, h| {
                        let lines = h.new_lines.clone();
                        d.child(
                            look::small_button("agent-card-revert", IconName::Undo2, "Revert", &p)
                                .debug_selector(|| "agent-card-revert".into())
                                .h(px(24.))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.agent_card = None;
                                    cx.emit(EditorEvent::Claude(ClaudeRequest::RevertHunk(lines.clone())));
                                })),
                        )
                    })
                    .when_some(h3, |d, h| {
                        let lines = h.new_lines.clone();
                        d.child(
                            div()
                                .id("agent-card-accept")
                                .debug_selector(|| "agent-card-accept".into())
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
                                .child(crate::components::icon(IconName::Check, 13., p.on_accent))
                                .child("Accept")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.agent_card = None;
                                    cx.emit(EditorEvent::Claude(ClaudeRequest::AcceptHunk(lines.clone())));
                                })),
                        )
                    }),
            );
        Some(
            gpui::deferred(gpui::anchored().position(card.at).snap_to_window_with_margin(px(8.)).child(body))
                .with_priority(2)
                .into_any_element(),
        )
    }
}
