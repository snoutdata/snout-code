//! Claude Code in an editor tab. A change it proposes types itself into the tab, hunk by
//! hunk, and waits there for Accept (⌘↩) or Reject (Esc): new lines lit green, removed
//! ones marked red where they were. Changes it makes on its own flash briefly when the
//! editor follows them.

use std::ops::Range;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, TextAlign, TextRun, Window, canvas, div, point, px, size,
};
use gpui_component::input::RopeExt as _;
use snout_core::gutter::{LineChange, LineChangeKind};

use super::{EditorEvent, EditorPanel};
use crate::claude_ide::review::{self, Frame, Mark, Stream};
use crate::look;
use crate::theme::{mix, palette};

/// One step of the change typing itself in.
pub const STREAM_TICK: Duration = Duration::from_millis(16);
/// One step of a follow highlight fading, and how many steps it lasts (about 2 s).
pub const FLASH_TICK: Duration = Duration::from_millis(50);
const FLASH_STEPS: u32 = 40;

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
        if self.review.is_some() {
            self.queue.push_back(proposal);
            cx.notify();
            return;
        }
        self.start_review(proposal, window, cx);
    }

    fn start_review(&mut self, mut p: Proposal, window: &mut Window, cx: &mut Context<Self>) {
        let original = self.text(cx);
        let new_file = !self.path.exists();
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

    /// Accept: write the text shown (with any edits) to the file and say so.
    pub fn accept_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.review.is_none() {
            return;
        }
        self.finish_stream(window, cx);
        let Some(mut r) = self.review.take() else { return };
        if r.new_file
            && let Some(dir) = self.path.parent()
        {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = self.text(cx);
        self.save(cx);
        if self.dirty {
            // Not written (the error shows); the review stays.
            self.review = Some(r);
            return;
        }
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

    fn next_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    /// many), and the fading light of a followed edit.
    pub(super) fn claude_overlay(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        if self.review.is_none() && self.flash.is_none() {
            return None;
        }
        let p = palette(cx);
        let marks = self.review.as_ref().map(|r| r.marks.clone()).unwrap_or_default();
        let flash = self.flash.as_ref().map(|f| (f.lines.clone(), f.strength()));
        let state = self.state.clone();
        let (added, removed, lit, panel) = (p.ok.opacity(0.16), p.error, p.accent, p.panel);
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
                        for m in &marks {
                            band(&m.lines, added);
                        }
                        if let Some((runs, strength)) = &flash {
                            for run in runs {
                                band(run, lit.opacity(0.28 * strength));
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
                        .child(div().text_color(p.ok).child(format!("+{plus}")))
                        .child(div().text_color(p.error).child(format!("−{minus}"))),
                )
                .when(waiting > 0, |d| {
                    let text =
                        if waiting == 1 { "1 more waiting".to_owned() } else { format!("{waiting} more waiting") };
                    d.child(look::chip(text, p.accent, &p))
                })
                .child(div().flex_1())
                .child(
                    look::small_button("review-reject", "Reject", &p)
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
                        .child("Accept")
                        .child(div().text_size(px(11.)).opacity(0.8).child("⌘↩"))
                        .on_click(cx.listener(|this, _, w, cx| this.accept_review(w, cx))),
                ),
        )
    }
}
