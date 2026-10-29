//! The built-in engine: the `vt100` crate, with answers to the queries TUIs send at
//! start-up (cursor position, device attributes, colours) and the window title.

use super::{Attrs, Cell, Color, Cursor, Engine, EngineKind, Frame, Modes, Underline, Width};
use crate::pty::Rgb;

/// Parser callbacks: replies to the child's queries are queued here.
struct Hooks {
    replies: Vec<u8>,
    title: String,
    /// The default colours, which TUIs ask for to pick a light or dark theme.
    fg: Rgb,
    bg: Rgb,
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
        // Default foreground (10) / background (11) colour queries.
        if let [which @ (b"10" | b"11"), b"?"] = params {
            let (r, g, b) = if *which == b"10" { self.fg } else { self.bg };
            let hex = |v: u8| format!("{v:02x}{v:02x}");
            let reply = format!("\x1b]{};rgb:{}/{}/{}\x1b\\", String::from_utf8_lossy(which), hex(r), hex(g), hex(b));
            self.replies.extend_from_slice(reply.as_bytes());
        }
    }
}

pub(super) struct Builtin {
    parser: vt100::Parser<Hooks>,
}

impl Builtin {
    pub(super) fn new(rows: u16, cols: u16, fg: Rgb, bg: Rgb, scrollback: usize) -> Self {
        let hooks = Hooks { replies: Vec::new(), title: String::new(), fg, bg };
        Self { parser: vt100::Parser::new_with_callbacks(rows, cols, scrollback, hooks) }
    }
}

fn color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Default,
        vt100::Color::Idx(i) => Color::Idx(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

impl Engine for Builtin {
    fn kind(&self) -> EngineKind {
        EngineKind::Builtin
    }

    fn write(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.parser.callbacks_mut().replies)
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);
    }

    fn set_colors(&mut self, fg: Rgb, bg: Rgb) {
        let hooks = self.parser.callbacks_mut();
        (hooks.fg, hooks.bg) = (fg, bg);
    }

    fn scrollback(&mut self) -> (usize, usize) {
        let screen = self.parser.screen_mut();
        let at = screen.scrollback();
        // vt100 tells how far it can scroll only by clamping.
        screen.set_scrollback(usize::MAX);
        let max = screen.scrollback();
        screen.set_scrollback(at);
        (at, max)
    }

    fn set_scrollback(&mut self, offset: usize) {
        self.parser.screen_mut().set_scrollback(offset);
    }

    fn modes(&self) -> Modes {
        let s = self.parser.screen();
        Modes {
            bracketed_paste: s.bracketed_paste(),
            app_cursor: s.application_cursor(),
            alt_screen: s.alternate_screen(),
            mouse: s.mouse_protocol_mode() != vt100::MouseProtocolMode::None,
            mouse_sgr: s.mouse_protocol_encoding() == vt100::MouseProtocolEncoding::Sgr,
            kitty_keyboard: 0,
            sync_output: false,
        }
    }

    fn frame(&mut self, frame: &mut Frame) {
        let (scrollback, history) = self.scrollback();
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        frame.begin(rows, cols);
        for row in 0..rows {
            for col in 0..cols {
                let Some(c) = screen.cell(row, col) else { break };
                let mut attrs = Attrs::default();
                attrs.set(Attrs::BOLD, c.bold());
                attrs.set(Attrs::DIM, c.dim());
                attrs.set(Attrs::ITALIC, c.italic());
                attrs.set(Attrs::INVERSE, c.inverse());
                let cell = Cell {
                    fg: color(c.fgcolor()),
                    bg: color(c.bgcolor()),
                    attrs,
                    underline: if c.underline() { Underline::Single } else { Underline::None },
                    width: if c.is_wide_continuation() {
                        Width::Continuation
                    } else if c.is_wide() {
                        Width::Wide
                    } else {
                        Width::Narrow
                    },
                    ..Cell::default()
                };
                frame.push(cell, c.contents());
            }
            frame.end_row(screen.row_wrapped(row));
        }
        if scrollback == 0 && !screen.hide_cursor() {
            let (row, col) = screen.cursor_position();
            frame.cursor = Some(Cursor { row, col, ..Cursor::default() });
        }
        frame.scrollback = scrollback;
        frame.history = history;
        frame.title = self.parser.callbacks().title.clone();
        frame.modes = self.modes();
        frame.engine = EngineKind::Builtin;
    }
}
