//! Where a jump lands. The view scrolls the place to the middle, unless it already shows
//! well clear of the edges, and the word there lights up for a moment, so the eye finds it.

use std::time::Duration;

use gpui::{App, Context, Pixels, Task, Window, point, px};
use gpui_component::input::{RangeDecoration, RangeDecorationStyle, RopeExt as _};

use super::EditorPanel;
use crate::theme::palette;

/// How long the word stays lit before it fades, and how long the fade takes.
const LIT_FOR: Duration = Duration::from_millis(1400);
const FADE_FOR: Duration = Duration::from_millis(600);
const FADE_STEPS: u32 = 6;
/// How strong the light is at first.
const LIGHT: f32 = 0.38;

/// The light of a landing, and the place waiting to be scrolled to.
#[derive(Default)]
pub struct Landing {
    /// The view still has to bring the caret's line to the middle (once it is laid out).
    pub reveal: bool,
    lit: Option<gpui_component::input::RangeDecorationCollection>,
    /// The light is on (fading or not).
    on: bool,
    _fade: Option<Task<()>>,
}

/// The view's new top (pixels down from the text's top) to show a line whose top is at
/// `line_top`, `line_height` tall, in a view `height` tall now `scroll` down: unchanged
/// when the line already shows outside the fifth at either edge, else the line in the
/// middle (never above the text's top).
pub fn centered_top(line_top: f32, line_height: f32, height: f32, scroll: f32) -> Option<f32> {
    let margin = (height / 5.).max(line_height);
    let clear = line_top >= scroll + margin && line_top + line_height <= scroll + height - margin;
    if clear || height <= 0. {
        return None;
    }
    Some((line_top - (height - line_height) / 2.).max(0.))
}

impl EditorPanel {
    /// The caret was put where a jump goes: bring it to the middle and light it up.
    pub(super) fn land(&mut self, cx: &mut Context<Self>) {
        self.landing.reveal = true;
        self.light_caret_word(cx);
        cx.notify();
    }

    /// The pending reveal, once the view has been laid out (else on the next frame).
    pub(super) fn reveal_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.landing.reveal || self.loading {
            return;
        }
        let ready = self.state.read(cx).line_height().is_some() && self.state.read(cx).text_bounds().is_some();
        if !ready {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
            return;
        }
        self.landing.reveal = false;
        let top = {
            let s = self.state.read(cx);
            let (Some(lh), Some(view)) = (s.line_height(), s.text_bounds()) else { return };
            let line = s.cursor_position().line as usize;
            let row = s.display_map().buffer_line_to_display_row(line);
            let px_of = |p: Pixels| f32::from(p);
            let line_top = px_of(lh) * row as f32;
            centered_top(line_top, px_of(lh), px_of(view.size.height), -px_of(s.scroll_offset().y))
        };
        if let Some(top) = top {
            self.state.update(cx, |s, cx| {
                let x = s.scroll_offset().x;
                s.set_scroll_offset(point(x, px(-top)), cx);
            });
        }
    }

    /// Light the word at the caret (the line's text when there is no word), then let the
    /// light fade.
    fn light_caret_word(&mut self, cx: &mut Context<Self>) {
        let range = {
            let s = self.state.read(cx);
            let rope = s.text();
            let caret = s.cursor();
            let word = rope
                .word_range(caret)
                .filter(|w| !w.is_empty() && !rope.slice(w.clone()).to_string().trim().is_empty());
            word.or_else(|| {
                let line = s.cursor_position().line as usize;
                let (start, end) = (rope.line_start_offset(line), rope.line_end_offset(line));
                let text = rope.slice(start..end).to_string();
                let lead = text.len() - text.trim_start().len();
                let body = text.trim().len();
                (body > 0).then(|| start + lead..start + lead + body)
            })
        };
        let Some(range) = range else { return };
        let lit = match self.landing.lit.clone() {
            Some(lit) => lit,
            None => {
                let lit = self.state.update(cx, |s, cx| s.create_range_decorations_collection(Vec::new(), cx));
                self.landing.lit = Some(lit.clone());
                lit
            }
        };
        let accent = palette(cx).accent;
        let paint = move |strength: f32, cx: &mut App| {
            let d = RangeDecoration::new(range.clone())
                .with_style(RangeDecorationStyle::Fill)
                .with_color(accent.opacity(LIGHT * strength));
            lit.set(if strength > 0. { vec![d] } else { Vec::new() }, cx);
        };
        paint(1., cx);
        self.landing.on = true;
        self.landing._fade = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LIT_FOR).await;
            for step in 1..=FADE_STEPS {
                cx.background_executor().timer(FADE_FOR / FADE_STEPS).await;
                let strength = 1. - step as f32 / FADE_STEPS as f32;
                let lit = this.update(cx, |this, cx| {
                    paint(strength, cx);
                    this.landing.on = strength > 0.;
                });
                if lit.is_err() {
                    return;
                }
            }
        }));
    }

    /// Whether the landing light is on.
    pub fn landing_lit(&self) -> bool {
        self.landing.on
    }
}

#[cfg(test)]
mod tests {
    use super::centered_top;

    #[test]
    fn a_place_off_screen_or_at_an_edge_comes_to_the_middle() {
        // 600 tall, 20 a line, the view at the top.
        assert_eq!(centered_top(1000., 20., 600., 0.), Some(710.));
        // Showing, but on the first line: to the middle (not above the text's top).
        assert_eq!(centered_top(0., 20., 600., 0.), Some(0.));
        assert_eq!(centered_top(100., 20., 600., 100.), Some(0.));
        // On the last line showing.
        assert_eq!(centered_top(580., 20., 600., 0.), Some(290.));
        // Well inside the view: it stays.
        assert_eq!(centered_top(300., 20., 600., 0.), None);
    }
}
