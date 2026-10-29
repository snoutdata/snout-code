//! Parameter name hints: the names of the parameters that literal arguments are passed to,
//! from the engine's `textDocument/inlayHint`, and the resolved values of Spring `@Value`
//! placeholders. GPUI Kit's editor cannot make room inside a line, so they are drawn after
//! the end of their line, quietly: `name: "Ada"` for each argument, in order.
//!
//! While the text changes they move with it (an edit before a hint shifts it, one over its
//! argument drops it), and the engine is asked again once typing pauses: nothing blinks.

use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Context, IntoElement, Styled as _, canvas, point, px, size};
use gpui_component::input::RopeExt as _;
use serde_json::json;

use super::run_gutter::visible_lines;
use super::{EditorDefaults, EditorPanel, font_size};
use crate::lang::text;
use crate::theme::{mix, palette};

/// A parameter's name, before the argument passed to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamHint {
    /// Where the argument starts, in the editor's text.
    pub at: usize,
    pub name: String,
}

/// How long typing pauses before the hints are asked for again.
const SETTLE: Duration = Duration::from_millis(350);

/// How often a request the engine could not answer yet is made again, and how many times.
const RETRY: Duration = Duration::from_millis(1500);
const TRIES: u32 = 80;

/// The longest argument a hint shows in full.
const ARG_MAX: usize = 24;

/// A hint's text.
pub fn hint_label(h: &lsp_types::InlayHint) -> String {
    match &h.label {
        lsp_types::InlayHintLabel::String(s) => s.clone(),
        lsp_types::InlayHintLabel::LabelParts(parts) => parts.iter().map(|p| p.value.as_str()).collect(),
    }
}

/// Whether `h` names a parameter: jdtls leaves the kind out, and ends the name with a colon.
pub fn is_param_hint(h: &lsp_types::InlayHint) -> bool {
    h.kind == Some(lsp_types::InlayHintKind::PARAMETER) || (h.kind.is_none() && hint_label(h).trim_end().ends_with(':'))
}

/// The parameter hints of an `inlayHint` answer (protocol positions, in `rope`).
pub fn parse_param_hints(v: &serde_json::Value, rope: &gpui_component::input::Rope) -> Vec<ParamHint> {
    let list: Vec<lsp_types::InlayHint> = serde_json::from_value(v.clone()).unwrap_or_default();
    let mut out: Vec<ParamHint> = list
        .into_iter()
        .filter(is_param_hint)
        .map(|h| {
            let label = hint_label(&h);
            ParamHint {
                at: text::from_lsp(rope, h.position),
                name: label.trim().trim_end_matches(':').trim().to_owned(),
            }
        })
        .filter(|h| !h.name.is_empty())
        .collect();
    out.sort_by_key(|h| h.at);
    out
}

/// `hints` after the text from `range` was replaced by `new` bytes: those after it move,
/// those whose argument it touched go.
pub fn shift(hints: &[ParamHint], range: &Range<usize>, new: usize) -> Vec<ParamHint> {
    hints
        .iter()
        .filter_map(|h| {
            if h.at >= range.end {
                Some(ParamHint { at: h.at - range.len() + new, name: h.name.clone() })
            } else if h.at < range.start {
                Some(h.clone())
            } else {
                None
            }
        })
        .collect()
}

/// The argument that starts at byte `from` of `line`: up to the comma or the bracket that
/// ends it, shortened when long.
pub fn argument_at(line: &str, from: usize) -> String {
    let b = line.as_bytes();
    let (mut i, mut depth) = (from.min(b.len()), 0i32);
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => break,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => break,
            _ => {}
        }
        i += 1;
    }
    let end = i.min(b.len());
    let arg = line.get(from.min(end)..end).unwrap_or_default().trim();
    if arg.chars().count() > ARG_MAX {
        let cut: String = arg.chars().take(ARG_MAX - 1).collect();
        format!("{cut}…")
    } else {
        arg.to_owned()
    }
}

impl EditorPanel {
    /// Whether this tab shows parameter hints: a Java file the engine serves, with the
    /// setting on.
    fn wants_param_hints(&self, cx: &App) -> bool {
        EditorDefaults::now(cx).param_hints
            && self.class_file.is_none()
            && self.lang.as_ref().is_some_and(|l| !l.config)
    }

    /// Ask the engine for the file's parameter hints (or put them away, with the setting off).
    pub fn refresh_param_hints(&mut self, cx: &mut Context<Self>) {
        if !self.wants_param_hints(cx) {
            self._param_hints = None;
            if !self.param_hints.is_empty() {
                self.param_hints = Rc::new(Vec::new());
                cx.notify();
            }
            return;
        }
        let Some(link) = self.lang.clone() else { return };
        let rope = self.state.read(cx).text().clone();
        let params = json!({
            "textDocument": { "uri": link.uri.as_str() },
            // The whole file, ending where it ends: jdtls finds no hints for a range past it.
            "range": text::range_to_lsp(&rope, &(0..rope.len()))
        });
        let call = link.ask(&rope, "textDocument/inlayHint", params, cx);
        let revision = self.revision;
        self._param_hints = Some(cx.spawn(async move |this, cx| {
            let answer = call.await;
            let _ = this.update(cx, |this, cx| {
                if this.revision != revision {
                    // Typing went on: the next pause asks again.
                    return;
                }
                match answer {
                    Ok(v) => {
                        this.param_tries = 0;
                        let hints = parse_param_hints(&v, &rope);
                        this.param_text = rope;
                        if *this.param_hints != hints {
                            this.param_hints = Rc::new(hints);
                            cx.notify();
                        }
                    }
                    // The engine is not ready for the file yet: again in a moment.
                    Err(_) if this.param_tries < TRIES => {
                        this.param_tries += 1;
                        this._param_settle = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(RETRY).await;
                            let _ = this.update(cx, |this, cx| this.refresh_param_hints(cx));
                        }));
                    }
                    Err(_) => {}
                }
            });
        }));
    }

    /// The text changed: the hints move with it (from the text they were placed in), and
    /// are asked for again once typing pauses.
    pub(super) fn param_hints_changed(&mut self, cx: &mut Context<Self>) {
        let now = self.state.read(cx).text().clone();
        if !self.param_hints.is_empty()
            && let Some((range, new)) = text::change(&self.param_text, &now)
        {
            self.param_hints = Rc::new(shift(&self.param_hints, &range, new.len()));
        }
        self.param_text = now;
        self.param_hints_soon(cx);
    }

    /// Ask for the hints once things settle (typing pauses, the engine has checked the file).
    pub(super) fn param_hints_soon(&mut self, cx: &mut Context<Self>) {
        if !self.wants_param_hints(cx) {
            return;
        }
        self._param_settle = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.refresh_param_hints(cx));
        }));
    }

    /// The hints after the end of their lines: each argument's parameter name in a quiet
    /// chip, then a Spring `@Value`'s value in italics.
    pub(super) fn end_of_line_hints(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let params = self.param_hints.clone();
        let values = self.hints.clone();
        let state = self.state.clone();
        let mono = crate::look::mono_font(cx);
        let ui = crate::look::ui_font(cx);
        let size_pt = font_size(cx);
        let spring = crate::spring::green(&p);
        canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                if params.is_empty() && values.is_empty() {
                    return;
                }
                let s = state.read(cx);
                let Some(line_height) = s.line_height() else { return };
                let rope = s.text();
                let visible = visible_lines(s);
                let mut lines: Vec<usize> = params
                    .iter()
                    .filter(|h| h.at <= rope.len())
                    .map(|h| rope.offset_to_point(h.at).row)
                    .chain(values.iter().map(|(l, _)| *l as usize))
                    .filter(|l| *l < rope.lines_len() && visible.iter().any(|(v, _)| v == l))
                    .collect();
                lines.sort_unstable();
                lines.dedup();
                // Where each line ends, and what goes after it, before anything is drawn.
                let places: Vec<(gpui::Bounds<gpui::Pixels>, Vec<String>, Vec<String>)> = lines
                    .into_iter()
                    .filter_map(|line| {
                        let (start, end) = (rope.line_start_offset(line), rope.line_end_offset(line));
                        let b = s.range_to_bounds(&(end..end))?;
                        let text = rope.slice(start..end).to_string();
                        let chips = params
                            .iter()
                            .filter(|h| h.at >= start && h.at <= end)
                            .map(|h| format!("{}: {}", h.name, argument_at(&text, h.at - start)))
                            .collect();
                        let notes =
                            values.iter().filter(|(l, _)| *l as usize == line).map(|(_, v)| v.clone()).collect();
                        Some((b, chips, notes))
                    })
                    .collect();
                let chip_ink = mix(p.fg_dim, 0.85, p.panel);
                let chip_bg = mix(p.fg_dim, if p.dark { 0.13 } else { 0.10 }, p.panel);
                let small = px((size_pt * 0.82).max(9.));
                for (b, chips, notes) in places {
                    let mut x = b.right() + px(16.);
                    for label in chips {
                        let run = gpui::TextRun {
                            len: label.len(),
                            font: gpui::font(ui.clone()),
                            color: chip_ink,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = window.text_system().shape_line(label.into(), small, &[run], None);
                        let chip = gpui::Bounds::new(
                            point(x, b.top() + px(3.)),
                            size(shaped.width + px(10.), line_height - px(6.)),
                        );
                        window.paint_quad(gpui::fill(chip, chip_bg).corner_radii(px(4.)));
                        let at = point(x + px(5.), b.top());
                        let _ = shaped.paint(at, line_height, gpui::TextAlign::Left, None, window, cx);
                        x = chip.right() + px(5.);
                    }
                    for value in notes {
                        let run = gpui::TextRun {
                            len: value.len(),
                            font: gpui::Font { style: gpui::FontStyle::Italic, ..gpui::font(mono.clone()) },
                            color: mix(spring, 0.75, p.panel),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = window.text_system().shape_line(value.into(), px(size_pt * 0.92), &[run], None);
                        let _ = shaped.paint(point(x, b.top()), line_height, gpui::TextAlign::Left, None, window, cx);
                        x += shaped.width + px(12.);
                    }
                }
            },
        )
        .absolute()
        .inset_0()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_move_with_the_text_and_show_their_arguments() {
        let hints = vec![ParamHint { at: 10, name: "a".into() }, ParamHint { at: 20, name: "b".into() }];
        // Typed before both: both move.
        assert_eq!(shift(&hints, &(0..0), 3).iter().map(|h| h.at).collect::<Vec<_>>(), [13, 23]);
        // Over the first argument: it goes.
        assert_eq!(shift(&hints, &(9..12), 1).iter().map(|h| h.at).collect::<Vec<_>>(), [18]);
        // After both: nothing moves.
        assert_eq!(shift(&hints, &(25..26), 0), hints);
        let line = "    label(\"Total, all\", 42, f(1, 2), true);";
        assert_eq!(argument_at(line, 10), "\"Total, all\"");
        assert_eq!(argument_at(line, 24), "42");
        assert_eq!(argument_at(line, 28), "f(1, 2)");
        assert_eq!(argument_at(line, 37), "true");
        assert_eq!(argument_at("x(\"a very long string literal indeed\")", 2), "\"a very long string lit…");
    }
}
