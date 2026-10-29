//! A terminal widget: the pseudo-terminal and xterm parser of `snout_core::pty`, painted
//! with egui, with selection and egui's key events turned into bytes.

use eframe::egui::{self, Color32, Event, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2};
pub use snout_core::pty::Launch;
use snout_core::pty::{self, Pty, Rgb};
use snout_core::term::{Attrs, Color, EngineKind, Frame, Underline, Width};

const FONT_SIZE: f32 = 13.0;

pub struct Terminal {
    pty: Pty,
    /// Anchor and end of a mouse selection, in screen cells (row, col) at the current
    /// scrollback offset.
    selection: Option<((u16, u16), (u16, u16))>,
}

impl Terminal {
    pub fn spawn(launch: &Launch<'_>, ctx: &egui::Context) -> anyhow::Result<Self> {
        let visuals = ctx.global_style().visuals.clone();
        let ctx = ctx.clone();
        let pty = Pty::spawn(
            &*snout_host::local(),
            launch,
            EngineKind::Builtin,
            rgb(default_fg(&visuals)),
            rgb(default_bg(&visuals)),
            move || ctx.request_repaint(),
        )?;
        Ok(Self { pty, selection: None })
    }

    /// `None` while running; a description of how it ended afterwards.
    pub fn exit_status(&mut self) -> Option<&str> {
        self.pty.exit_status()
    }

    pub fn title(&self) -> String {
        self.pty.title()
    }

    pub fn output_gen(&self) -> u64 {
        self.pty.output_gen()
    }

    pub fn last_output(&self) -> Option<std::time::Instant> {
        self.pty.last_output()
    }

    /// The visible screen as text, one line per row.
    #[cfg(test)]
    pub fn contents(&self) -> String {
        self.pty.contents()
    }

    pub fn kill(&mut self) {
        self.pty.kill();
    }

    fn resize(&mut self, size: (u16, u16)) {
        if self.pty.resize(size) {
            self.selection = None;
        }
    }

    /// Paint the terminal into the remaining space and, while it has keyboard focus, send
    /// it the keyboard.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let font = FontId::monospace(FONT_SIZE);
        let (cell_w, cell_h) = ui.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
        let cell = Vec2::new(cell_w, cell_h.ceil());
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let id = response.id;
        let pad = 4.0;
        let inner = rect.shrink(pad);
        let rows = ((inner.height() / cell.y).floor() as u16).max(2);
        let cols = ((inner.width() / cell.x).floor() as u16).max(10);
        self.resize((rows, cols));

        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        let focused = response.has_focus();
        if focused {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true },
                );
            });
        }

        let cell_at = |pos: Pos2| -> (u16, u16) {
            let v = (pos - inner.min) / cell;
            ((v.y.max(0.0) as u16).min(rows - 1), (v.x.max(0.0) as u16).min(cols - 1))
        };
        if let Some(pos) = response.interact_pointer_pos() {
            if response.drag_started() {
                self.selection = Some((cell_at(pos), cell_at(pos)));
            } else if response.dragged()
                && let Some((_, end)) = &mut self.selection
            {
                *end = cell_at(pos);
            }
        }
        if response.clicked() {
            self.selection = None;
        }

        let events = if response.hovered() || focused { ui.input(|i| i.events.clone()) } else { Vec::new() };
        let mut out = Vec::new();
        for event in &events {
            match event {
                Event::MouseWheel { unit, delta, .. } if response.hovered() => {
                    let lines = match unit {
                        egui::MouseWheelUnit::Point => delta.y / cell.y,
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Page => delta.y * f32::from(rows),
                    };
                    if self.pty.scroll(lines) {
                        self.selection = None;
                    }
                }
                Event::Copy if focused => {
                    let text = self.selection.and_then(|s| self.pty.frame().selected_text(s));
                    if let Some(text) = text {
                        ui.ctx().copy_text(text);
                    }
                }
                _ if focused => {
                    let bytes = {
                        let modes = self.pty.frame().modes;
                        encode_event(event, modes.app_cursor, modes.bracketed_paste)
                    };
                    if !bytes.is_empty() {
                        out.extend_from_slice(&bytes);
                        self.selection = None;
                    }
                }
                _ => {}
            }
        }
        // Follow the theme, also for the colour queries TUIs send later.
        let (fg0, bg0) = (default_fg(ui.visuals()), default_bg(ui.visuals()));
        self.pty.set_colors(rgb(fg0), rgb(bg0));
        paint(ui, &self.pty.frame(), (fg0, bg0), inner, cell, &font, focused, self.selection);
        // Typing jumps back to the live screen.
        self.pty.send(&out);
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        response
    }
}

// ----- Input ---------------------------------------------------------------------------

/// The bytes an xterm sends for an egui input event.
fn encode_event(event: &Event, app_cursor: bool, bracketed: bool) -> Vec<u8> {
    match event {
        Event::Text(t) => t.as_bytes().to_vec(),
        Event::Ime(egui::ImeEvent::Commit(t)) => t.as_bytes().to_vec(),
        Event::Paste(t) => pty::paste_bytes(t, bracketed),
        // Claude Code reads an image from the clipboard itself on Ctrl+V.
        Event::PasteImage(_) => vec![0x16],
        Event::Key { key, pressed: true, modifiers, .. } => encode_key(*key, *modifiers, app_cursor),
        _ => Vec::new(),
    }
}

fn encode_key(key: Key, m: Modifiers, app_cursor: bool) -> Vec<u8> {
    use pty::Key as K;
    let mods = pty::Mods { shift: m.shift, alt: m.alt, ctrl: m.ctrl, cmd: m.mac_cmd || (m.command && !m.ctrl) };
    let key = match key {
        Key::ArrowUp => K::Up,
        Key::ArrowDown => K::Down,
        Key::ArrowRight => K::Right,
        Key::ArrowLeft => K::Left,
        Key::Home => K::Home,
        Key::End => K::End,
        Key::Insert => K::Insert,
        Key::Delete => K::Delete,
        Key::PageUp => K::PageUp,
        Key::PageDown => K::PageDown,
        Key::Enter => K::Enter,
        Key::Tab => K::Tab,
        Key::Backspace => K::Backspace,
        Key::Escape => K::Escape,
        Key::Space => K::Space,
        Key::OpenBracket => K::Char('['),
        Key::Backslash => K::Char('\\'),
        Key::CloseBracket => K::Char(']'),
        Key::Slash => K::Char('/'),
        Key::Minus => K::Char('-'),
        other => match other.name().as_bytes() {
            [c @ b'A'..=b'Z'] => K::Char(char::from(*c)),
            _ => return Vec::new(),
        },
    };
    pty::encode_key(key, mods, app_cursor)
}

// ----- Painting ------------------------------------------------------------------------

/// The pane's own colours, so the terminal reads as part of it rather than a box inside it.
fn default_fg(v: &egui::Visuals) -> Color32 {
    crate::theme::Palette::of(v).fg
}

fn default_bg(v: &egui::Visuals) -> Color32 {
    crate::theme::Palette::of(v).panel
}

fn rgb(c: Color32) -> Rgb {
    (c.r(), c.g(), c.b())
}

fn color32((r, g, b): Rgb) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn color(c: Color, default: Color32, bold: bool) -> Color32 {
    color32(pty::color(c, rgb(default), bold))
}

/// Resolved colours of one cell.
fn cell_colors(cell: &snout_core::term::Cell, fg0: Color32, bg0: Color32) -> (Color32, Color32) {
    let mut fg = color(cell.fg, fg0, cell.bold());
    let mut bg = color(cell.bg, bg0, false);
    if cell.attrs.contains(Attrs::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.attrs.contains(Attrs::DIM) {
        fg = fg.lerp_to_gamma(bg, 0.45);
    }
    (fg, bg)
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &egui::Ui,
    screen: &Frame,
    (fg0, bg0): (Color32, Color32),
    inner: Rect,
    cell: Vec2,
    font: &FontId,
    focused: bool,
    selection: Option<((u16, u16), (u16, u16))>,
) {
    let painter = ui.painter_at(inner.expand(4.0));
    painter.rect_filled(inner.expand(4.0), 0.0, bg0);
    let (rows, cols) = screen.size();
    let selection = selection.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });
    let selected = |r: u16, c: u16| selection.is_some_and(|(s, e)| (r, c) >= s && (r, c) <= e && s != e);
    let select_bg = ui.visuals().selection.bg_fill;

    for row in 0..rows {
        let y = inner.min.y + f32::from(row) * cell.y;
        // A run of plain ASCII cells sharing a colour is painted as one string.
        let mut run = String::new();
        let mut run_start = 0u16;
        let mut run_fg = fg0;
        let flush = |run: &mut String, start: u16, fg: Color32| {
            if !run.trim_end().is_empty() {
                let pos = Pos2::new(inner.min.x + f32::from(start) * cell.x, y);
                painter.text(pos, egui::Align2::LEFT_TOP, run.as_str(), font.clone(), fg);
            }
            run.clear();
        };
        for col in 0..cols {
            let Some(c) = screen.cell(row, col) else { continue };
            if c.width == Width::Continuation {
                continue;
            }
            let wide = c.width == Width::Wide;
            let width = if wide { 2.0 } else { 1.0 };
            let (fg, mut bg) = cell_colors(c, fg0, bg0);
            if selected(row, col) {
                bg = select_bg;
            }
            let x = inner.min.x + f32::from(col) * cell.x;
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell.x * width, cell.y));
            if bg != bg0 {
                painter.rect_filled(rect.expand(0.3), 0.0, bg);
            }
            if c.underline != Underline::None {
                painter.hline(rect.x_range(), rect.bottom() - 1.5, Stroke::new(1.0, fg));
            }
            let text = screen.text(c);
            let plain = text.len() == 1 && text.as_bytes()[0].is_ascii_graphic() || text.is_empty() || text == " ";
            if plain && fg == run_fg && !wide {
                if run.is_empty() {
                    run_start = col;
                }
                run.push_str(if text.is_empty() { " " } else { text });
                continue;
            }
            flush(&mut run, run_start, run_fg);
            run_fg = fg;
            if plain {
                run_start = col;
                run.push_str(if text.is_empty() { " " } else { text });
            } else {
                // Glyphs from fallback fonts have their own advance; centre each in its cell.
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, text, font.clone(), fg);
            }
        }
        flush(&mut run, run_start, run_fg);
    }

    if let Some(cursor) = screen.cursor {
        let (row, col) = (cursor.row, cursor.col);
        let pos = inner.min + Vec2::new(f32::from(col) * cell.x, f32::from(row) * cell.y);
        let rect = Rect::from_min_size(pos, cell);
        if focused {
            painter.rect_filled(rect, 0.0, fg0.gamma_multiply(0.7));
            if let Some(c) = screen.cell(row, col) {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, screen.text(c), font.clone(), bg0);
            }
        } else {
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, fg0.gamma_multiply(0.6)), egui::StrokeKind::Inside);
        }
    }
    if screen.scrollback > 0 {
        let label = format!("↑ {} lines", screen.scrollback);
        let pos = inner.right_top() + Vec2::new(-6.0, 4.0);
        painter.text(pos, egui::Align2::RIGHT_TOP, label, FontId::proportional(11.0), ui.visuals().weak_text_color());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egui_keys_encode_like_xterm() {
        let none = Modifiers::NONE;
        assert_eq!(encode_key(Key::ArrowUp, none, false), b"\x1b[A");
        assert_eq!(encode_key(Key::ArrowLeft, Modifiers::ALT, true), b"\x1b[1;3D");
        assert_eq!(encode_key(Key::C, Modifiers::CTRL, false), [3]);
        assert!(encode_key(Key::C, Modifiers::MAC_CMD, false).is_empty());
        assert_eq!(encode_event(&Event::Paste("a\nb".into()), false, false), b"a\rb");
        assert_eq!(color(Color::Idx(1), Color32::WHITE, true), color32(pty::indexed(9)));
    }
}
