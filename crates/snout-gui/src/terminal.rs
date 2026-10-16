//! A terminal widget: a child process on a pseudo-terminal, parsed with `vt100` and painted
//! with egui. Enough of xterm for full-screen TUIs such as Claude Code: 256/true colour,
//! bracketed paste, application cursor keys, mouse wheel reporting, scrollback, selection,
//! and answers to the queries TUIs send at start-up (cursor position, device attributes,
//! colours).

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use eframe::egui::{self, Color32, Event, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

const SCROLLBACK: usize = 10_000;
const FONT_SIZE: f32 = 13.0;

/// How to start the child process.
pub struct Launch<'a> {
    pub program: &'a Path,
    pub args: &'a [String],
    pub cwd: &'a Path,
    /// Variables to set in the child, on top of the inherited environment.
    pub env: &'a [(&'a str, String)],
    /// Inherited variables to drop.
    pub env_remove: &'a [&'a str],
}

pub struct Terminal {
    parser: Arc<Mutex<vt100::Parser<Hooks>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    shared: Arc<Shared>,
    size: (u16, u16),
    /// Anchor and end of a mouse selection, in screen cells (row, col) at the current
    /// scrollback offset.
    selection: Option<((u16, u16), (u16, u16))>,
    exit: Option<String>,
}

/// State written by the reader thread.
#[derive(Default)]
struct Shared {
    exited: AtomicBool,
    /// Bumped on every chunk of output.
    output_gen: AtomicU64,
    last_output: Mutex<Option<Instant>>,
}

/// Parser callbacks: replies to the child's queries are queued here and written back by
/// the reader thread.
struct Hooks {
    replies: Vec<u8>,
    title: String,
    fg: Color32,
    bg: Color32,
}

impl vt100::Callbacks for Hooks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
    }

    fn set_window_icon_name(&mut self, screen: &mut vt100::Screen, name: &[u8]) {
        self.set_window_title(screen, name);
    }

    fn unhandled_csi(&mut self, screen: &mut vt100::Screen, i1: Option<u8>, _: Option<u8>, params: &[&[u16]], c: char) {
        let first = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, c, first) {
            // Device status report: OK.
            (None, 'n', 5) => self.replies.extend_from_slice(b"\x1b[0n"),
            // Cursor position report.
            (None, 'n', 6) => {
                let (row, col) = screen.cursor_position();
                self.replies.extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
            }
            // Primary device attributes: a VT220 with ANSI colour. TUIs send this after
            // optional queries (kitty keyboard, ...) and treat the answer as "done".
            (None, 'c', 0) => self.replies.extend_from_slice(b"\x1b[?62;22c"),
            // Secondary device attributes.
            (Some(b'>'), 'c', 0) => self.replies.extend_from_slice(b"\x1b[>0;10;1c"),
            _ => {}
        }
    }

    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        // Default foreground (10) / background (11) colour queries; TUIs pick a light or
        // dark theme from the answer.
        if let [which @ (b"10" | b"11"), b"?"] = params {
            let c = if *which == b"10" { self.fg } else { self.bg };
            let hex = |v: u8| format!("{v:02x}{v:02x}");
            let reply = format!(
                "\x1b]{};rgb:{}/{}/{}\x1b\\",
                String::from_utf8_lossy(which),
                hex(c.r()),
                hex(c.g()),
                hex(c.b())
            );
            self.replies.extend_from_slice(reply.as_bytes());
        }
    }
}

impl Terminal {
    pub fn spawn(launch: &Launch<'_>, ctx: &egui::Context) -> anyhow::Result<Self> {
        let size = (24, 80);
        let pair = native_pty_system().openpty(pty_size(size))?;
        let mut cmd = CommandBuilder::new(launch.program);
        cmd.args(launch.args);
        cmd.cwd(launch.cwd);
        for name in launch.env_remove {
            cmd.env_remove(name);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in launch.env {
            cmd.env(k, v);
        }
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let visuals = ctx.global_style().visuals.clone();
        let hooks =
            Hooks { replies: Vec::new(), title: String::new(), fg: default_fg(&visuals), bg: default_bg(&visuals) };
        let parser = Arc::new(Mutex::new(vt100::Parser::new_with_callbacks(size.0, size.1, SCROLLBACK, hooks)));
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let shared = Arc::new(Shared::default());

        let mut reader = pair.master.try_clone_reader()?;
        let (p, w, s, ctx) = (parser.clone(), writer.clone(), shared.clone(), ctx.clone());
        std::thread::Builder::new().name("terminal-reader".into()).spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let replies = {
                            let mut parser = lock(&p);
                            parser.process(&buf[..n]);
                            std::mem::take(&mut parser.callbacks_mut().replies)
                        };
                        if !replies.is_empty() {
                            let _ = lock(&w).write_all(&replies);
                        }
                        s.output_gen.fetch_add(1, Ordering::Relaxed);
                        *lock(&s.last_output) = Some(Instant::now());
                        ctx.request_repaint();
                    }
                }
            }
            s.exited.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        })?;

        Ok(Self { parser, writer, master: pair.master, child, shared, size, selection: None, exit: None })
    }

    /// `None` while running; a description of how it ended afterwards.
    pub fn exit_status(&mut self) -> Option<&str> {
        if self.exit.is_none() && self.shared.exited.load(Ordering::Relaxed) {
            let status = self.child.try_wait().ok().flatten().or_else(|| self.child.wait().ok());
            self.exit = Some(match status {
                Some(s) if s.success() => "exited".to_owned(),
                Some(s) => format!("exited with code {}", s.exit_code()),
                None => "exited".to_owned(),
            });
        }
        self.exit.as_deref()
    }

    pub fn title(&self) -> String {
        lock(&self.parser).callbacks().title.clone()
    }

    pub fn output_gen(&self) -> u64 {
        self.shared.output_gen.load(Ordering::Relaxed)
    }

    pub fn last_output(&self) -> Option<Instant> {
        *lock(&self.shared.last_output)
    }

    /// The visible screen as text, one line per row.
    #[cfg(test)]
    pub fn contents(&self) -> String {
        lock(&self.parser).screen().contents()
    }

    pub fn write(&self, bytes: &[u8]) {
        let mut w = lock(&self.writer);
        let _ = w.write_all(bytes);
        let _ = w.flush();
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

    fn resize(&mut self, size: (u16, u16)) {
        if size != self.size && size.0 > 0 && size.1 > 0 {
            self.size = size;
            self.selection = None;
            lock(&self.parser).screen_mut().set_size(size.0, size.1);
            let _ = self.master.resize(pty_size(size));
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
        {
            let shared_parser = self.parser.clone();
            let mut parser = lock(&shared_parser);
            for event in &events {
                match event {
                    Event::MouseWheel { unit, delta, .. } if response.hovered() => {
                        let lines = match unit {
                            egui::MouseWheelUnit::Point => delta.y / cell.y,
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * f32::from(rows),
                        };
                        self.scroll(&mut parser, lines, &mut out);
                    }
                    Event::Copy if focused => {
                        if let Some(text) = self.selected_text(parser.screen()) {
                            ui.ctx().copy_text(text);
                        }
                    }
                    _ if focused => {
                        let screen = parser.screen();
                        let bytes = encode_event(event, screen.application_cursor(), screen.bracketed_paste());
                        if !bytes.is_empty() {
                            out.extend_from_slice(&bytes);
                            // Typing jumps back to the live screen.
                            parser.screen_mut().set_scrollback(0);
                            self.selection = None;
                        }
                    }
                    _ => {}
                }
            }
            paint(ui, &parser, inner, cell, &font, focused, self.selection);
        }
        if !out.is_empty() {
            self.write(&out);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        response
    }

    /// Wheel: report it if the app asked for mouse events, move through its history with
    /// the arrow keys in the alternate screen, scroll our own scrollback otherwise.
    fn scroll(&mut self, parser: &mut vt100::Parser<Hooks>, lines: f32, out: &mut Vec<u8>) {
        let n = lines.abs().round() as usize;
        if n == 0 {
            return;
        }
        let up = lines > 0.0;
        let screen = parser.screen();
        if screen.mouse_protocol_mode() != vt100::MouseProtocolMode::None {
            let button = if up { 64 } else { 65 };
            for _ in 0..n {
                if screen.mouse_protocol_encoding() == vt100::MouseProtocolEncoding::Sgr {
                    out.extend_from_slice(format!("\x1b[<{button};1;1M").as_bytes());
                } else {
                    out.extend_from_slice(&[0x1b, b'[', b'M', 32 + button, 33, 33]);
                }
            }
        } else if screen.alternate_screen() {
            let key: &[u8] = match (up, screen.application_cursor()) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            out.extend(key.repeat(n));
        } else {
            let offset = screen.scrollback();
            let offset = if up { offset + n } else { offset.saturating_sub(n) };
            parser.screen_mut().set_scrollback(offset);
            self.selection = None;
        }
    }

    fn selected_text(&self, screen: &vt100::Screen) -> Option<String> {
        let (a, b) = self.selection?;
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        if start == end {
            return None;
        }
        let text = screen.contents_between(start.0, start.1, end.0, end.1 + 1);
        Some(text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n"))
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn pty_size((rows, cols): (u16, u16)) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

// ----- Input ---------------------------------------------------------------------------

/// The bytes an xterm sends for an egui input event.
fn encode_event(event: &Event, app_cursor: bool, bracketed: bool) -> Vec<u8> {
    match event {
        Event::Text(t) => t.as_bytes().to_vec(),
        Event::Ime(egui::ImeEvent::Commit(t)) => t.as_bytes().to_vec(),
        Event::Paste(t) => {
            if bracketed {
                let t = t.replace("\x1b[201~", "");
                format!("\x1b[200~{t}\x1b[201~").into_bytes()
            } else {
                t.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
            }
        }
        // Claude Code reads an image from the clipboard itself on Ctrl+V.
        Event::PasteImage(_) => vec![0x16],
        Event::Key { key, pressed: true, modifiers, .. } => encode_key(*key, *modifiers, app_cursor),
        _ => Vec::new(),
    }
}

fn encode_key(key: Key, m: Modifiers, app_cursor: bool) -> Vec<u8> {
    // ⌘ combinations belong to the app (copy, paste, shortcuts).
    if m.mac_cmd || (m.command && !m.ctrl) {
        return Vec::new();
    }
    // xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl.
    let param = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.ctrl);
    let cursor = |c: char| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[1;{param}{c}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        }
    };
    let tilde = |n: u8| -> Vec<u8> {
        if param > 1 { format!("\x1b[{n};{param}~").into_bytes() } else { format!("\x1b[{n}~").into_bytes() }
    };
    match key {
        Key::ArrowUp => cursor('A'),
        Key::ArrowDown => cursor('B'),
        Key::ArrowRight => cursor('C'),
        Key::ArrowLeft => cursor('D'),
        Key::Home => cursor('H'),
        Key::End => cursor('F'),
        Key::Insert => tilde(2),
        Key::Delete => tilde(3),
        Key::PageUp => tilde(5),
        Key::PageDown => tilde(6),
        // Shift/Option+Enter insert a newline in Claude Code (sent as Meta+Enter).
        Key::Enter if m.shift || m.alt => b"\x1b\r".to_vec(),
        Key::Enter => b"\r".to_vec(),
        Key::Tab if m.shift => b"\x1b[Z".to_vec(),
        Key::Tab => b"\t".to_vec(),
        Key::Backspace if m.alt => b"\x1b\x7f".to_vec(),
        Key::Backspace if m.ctrl => vec![0x08],
        Key::Backspace => vec![0x7f],
        Key::Escape => vec![0x1b],
        _ if m.ctrl => ctrl_byte(key).map(|b| vec![b]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Control character for Ctrl+key (Ctrl+C → ETX, ...).
fn ctrl_byte(key: Key) -> Option<u8> {
    let name = key.name();
    match name.as_bytes() {
        [c @ b'A'..=b'Z'] => Some(c & 0x1f),
        _ => match key {
            Key::Space => Some(0),
            Key::OpenBracket => Some(0x1b),
            Key::Backslash => Some(0x1c),
            Key::CloseBracket => Some(0x1d),
            Key::Slash | Key::Minus => Some(0x1f),
            _ => None,
        },
    }
}

// ----- Painting ------------------------------------------------------------------------

fn default_fg(v: &egui::Visuals) -> Color32 {
    if v.dark_mode { Color32::from_gray(220) } else { Color32::from_gray(30) }
}

fn default_bg(v: &egui::Visuals) -> Color32 {
    if v.dark_mode { Color32::from_gray(16) } else { Color32::from_gray(252) }
}

/// The xterm palette.
fn indexed(i: u8) -> Color32 {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    match i {
        0..=15 => {
            let (r, g, b) = BASE[i as usize];
            Color32::from_rgb(r, g, b)
        }
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color32::from_rgb(level(i / 36), level((i / 6) % 6), level(i % 6))
        }
        _ => Color32::from_gray(8 + (i - 232) * 10),
    }
}

fn color(c: vt100::Color, default: Color32, bold: bool) -> Color32 {
    match c {
        vt100::Color::Default => default,
        // Bold brightens the eight base colours, as in most terminals.
        vt100::Color::Idx(i) if bold && i < 8 => indexed(i + 8),
        vt100::Color::Idx(i) => indexed(i),
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

/// Resolved colours of one cell.
fn cell_colors(cell: &vt100::Cell, fg0: Color32, bg0: Color32) -> (Color32, Color32) {
    let mut fg = color(cell.fgcolor(), fg0, cell.bold());
    let mut bg = color(cell.bgcolor(), bg0, false);
    if cell.inverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.dim() {
        fg = fg.lerp_to_gamma(bg, 0.45);
    }
    (fg, bg)
}

fn paint(
    ui: &egui::Ui,
    parser: &vt100::Parser<Hooks>,
    inner: Rect,
    cell: Vec2,
    font: &FontId,
    focused: bool,
    selection: Option<((u16, u16), (u16, u16))>,
) {
    let painter = ui.painter_at(inner.expand(4.0));
    let (fg0, bg0) = {
        let hooks = parser.callbacks();
        (hooks.fg, hooks.bg)
    };
    painter.rect_filled(inner.expand(4.0), 0.0, bg0);
    let screen = parser.screen();
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
            if c.is_wide_continuation() {
                continue;
            }
            let width = if c.is_wide() { 2.0 } else { 1.0 };
            let (fg, mut bg) = cell_colors(c, fg0, bg0);
            if selected(row, col) {
                bg = select_bg;
            }
            let x = inner.min.x + f32::from(col) * cell.x;
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell.x * width, cell.y));
            if bg != bg0 {
                painter.rect_filled(rect.expand(0.3), 0.0, bg);
            }
            if c.underline() {
                painter.hline(rect.x_range(), rect.bottom() - 1.5, Stroke::new(1.0, fg));
            }
            let text = c.contents();
            let plain = text.len() == 1 && text.as_bytes()[0].is_ascii_graphic() || text.is_empty() || text == " ";
            if plain && fg == run_fg && !c.is_wide() {
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

    if screen.scrollback() == 0 && !screen.hide_cursor() {
        let (row, col) = screen.cursor_position();
        let pos = inner.min + Vec2::new(f32::from(col) * cell.x, f32::from(row) * cell.y);
        let rect = Rect::from_min_size(pos, cell);
        if focused {
            painter.rect_filled(rect, 0.0, fg0.gamma_multiply(0.7));
            if let Some(c) = screen.cell(row, col) {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, c.contents(), font.clone(), bg0);
            }
        } else {
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, fg0.gamma_multiply(0.6)), egui::StrokeKind::Inside);
        }
    }
    if screen.scrollback() > 0 {
        let label = format!("↑ {} lines", screen.scrollback());
        let pos = inner.right_top() + Vec2::new(-6.0, 4.0);
        painter.text(pos, egui::Align2::RIGHT_TOP, label, FontId::proportional(11.0), ui.visuals().weak_text_color());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_encode_like_xterm() {
        let none = Modifiers::NONE;
        assert_eq!(encode_key(Key::ArrowUp, none, false), b"\x1b[A");
        assert_eq!(encode_key(Key::ArrowUp, none, true), b"\x1bOA");
        assert_eq!(encode_key(Key::ArrowLeft, Modifiers::ALT, true), b"\x1b[1;3D");
        assert_eq!(encode_key(Key::Tab, Modifiers::SHIFT, false), b"\x1b[Z");
        assert_eq!(encode_key(Key::Enter, Modifiers::SHIFT, false), b"\x1b\r");
        assert_eq!(encode_key(Key::C, Modifiers::CTRL, false), [3]);
        assert_eq!(encode_key(Key::PageDown, none, false), b"\x1b[6~");
        assert!(encode_key(Key::C, Modifiers::MAC_CMD, false).is_empty());
        let paste = encode_event(&Event::Paste("a\nb".into()), false, true);
        assert_eq!(paste, b"\x1b[200~a\nb\x1b[201~");
        assert_eq!(encode_event(&Event::Paste("a\nb".into()), false, false), b"a\rb");
    }

    #[test]
    fn palette() {
        assert_eq!(indexed(16), Color32::from_rgb(0, 0, 0));
        assert_eq!(indexed(231), Color32::from_rgb(255, 255, 255));
        assert_eq!(indexed(255), Color32::from_gray(238));
        assert_eq!(color(vt100::Color::Idx(1), Color32::WHITE, true), indexed(9));
    }

    fn wait_for(term: &Terminal, what: &str) -> String {
        for _ in 0..200 {
            let text = term.contents();
            if text.contains(what) {
                return text;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{what:?} never appeared in:\n{}", term.contents());
    }

    #[test]
    fn runs_a_process_and_answers_its_queries() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir();
        // Ask for the cursor position, then echo what the terminal answered (minus ESC).
        let script = r#"printf 'hello \033[31mred\033[0m\n'; stty raw -echo; printf '\033[6n'; r=$(dd bs=1 count=6 2>/dev/null); stty sane; printf 'reply:%s\n' "$(printf %s "$r" | tr -d '\033')"; read line; echo "got:$line""#;
        let args = ["-c".to_owned(), script.to_owned()];
        let launch = Launch { program: Path::new("/bin/sh"), args: &args, cwd: &dir, env: &[], env_remove: &[] };
        let mut term = Terminal::spawn(&launch, &ctx).unwrap();
        wait_for(&term, "hello red");
        wait_for(&term, "reply:[2;1R");
        {
            let parser = lock(&term.parser);
            let red = parser.screen().cell(0, 6).unwrap();
            assert_eq!((red.contents(), red.fgcolor()), ("r", vt100::Color::Idx(1)));
        }
        term.write(b"typed\r");
        wait_for(&term, "got:typed");
        for _ in 0..200 {
            if term.exit_status().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(term.exit_status(), Some("exited"));
    }
}
