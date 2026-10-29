//! Ghostty's engine (`libghostty-vt`): the terminal emulation of the Ghostty terminal, with
//! reflow on resize, the kitty keyboard protocol, hyperlinks (OSC 8), synchronized output,
//! cursor shapes and the full set of SGR attributes.
//!
//! Its handles are neither `Send` nor `Sync`: an engine is made, used and dropped on its
//! terminal's own thread (see [`crate::pty`]).

use std::cell::RefCell;
use std::rc::Rc;

use libghostty_vt::render::{CellIterator, CursorVisualStyle, RowIterator};
use libghostty_vt::screen::{CellContentTag, CellWide, Screen};
use libghostty_vt::style::{self, PaletteIndex, RgbColor, StyleColor};
use libghostty_vt::terminal::{
    ColorScheme, ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType, Mode, Point, PointCoordinate,
    PrimaryDeviceAttributes, ScrollViewport, SecondaryDeviceAttributes, TertiaryDeviceAttributes,
};
use libghostty_vt::{RenderState, Terminal, TerminalOptions, key};

use super::{Attrs, Cell, Color, Cursor, CursorShape, Engine, EngineKind, Frame, Modes, SCROLLBACK, Underline, Width};
use crate::pty::{self, KeyInput, Rgb};

/// What the terminal's callbacks write, for the engine to pick up.
#[derive(Default)]
struct Shared {
    replies: Vec<u8>,
    dark: bool,
}

pub(super) struct Ghostty {
    term: Terminal<'static, 'static>,
    render: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    keys: key::Encoder<'static>,
    shared: Rc<RefCell<Shared>>,
    plain: style::Style,
}

fn rgb((r, g, b): Rgb) -> RgbColor {
    RgbColor { r, g, b }
}

fn dark((r, g, b): Rgb) -> bool {
    u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 < 128_000
}

fn color(c: StyleColor) -> Color {
    match c {
        StyleColor::None => Color::Default,
        StyleColor::Palette(PaletteIndex(i)) => Color::Idx(i),
        StyleColor::Rgb(RgbColor { r, g, b }) => Color::Rgb(r, g, b),
    }
}

fn err(e: libghostty_vt::Error) -> String {
    format!("Ghostty: {e:?}")
}

impl Ghostty {
    pub(super) fn new(rows: u16, cols: u16, fg: Rgb, bg: Rgb) -> Result<Self, String> {
        let shared = Rc::new(RefCell::new(Shared { replies: Vec::new(), dark: dark(bg) }));
        let mut term = Terminal::new(TerminalOptions { cols, rows, max_scrollback: SCROLLBACK }).map_err(err)?;
        // The same palette as the built-in engine, for the colour queries (OSC 4).
        let mut palette = term.default_color_palette().map_err(err)?;
        for i in 0..=255u8 {
            palette.set(PaletteIndex(i), rgb(pty::indexed(i)));
        }
        term.set_default_color_palette(Some(palette)).map_err(err)?;
        term.set_default_fg_color(Some(rgb(fg))).map_err(err)?;
        term.set_default_bg_color(Some(rgb(bg))).map_err(err)?;
        let s = shared.clone();
        term.on_pty_write(move |_, data| s.borrow_mut().replies.extend_from_slice(data)).map_err(err)?;
        // Primary device attributes: TUIs send them after optional queries (kitty keyboard,
        // ...) and treat the answer as "done".
        term.on_device_attributes(|_| {
            Some(DeviceAttributes {
                primary: PrimaryDeviceAttributes::new(ConformanceLevel::VT220, &[DeviceAttributeFeature::ANSI_COLOR]),
                secondary: SecondaryDeviceAttributes {
                    device_type: DeviceType::VT220,
                    firmware_version: 10,
                    rom_cartridge: 1,
                },
                tertiary: TertiaryDeviceAttributes { unit_id: 0 },
            })
        })
        .map_err(err)?;
        term.on_xtversion(|_| Some(concat!("SnoutCode ", env!("CARGO_PKG_VERSION")))).map_err(err)?;
        let s = shared.clone();
        term.on_color_scheme(move |_| Some(if s.borrow().dark { ColorScheme::Dark } else { ColorScheme::Light }))
            .map_err(err)?;
        Ok(Self {
            term,
            render: RenderState::new().map_err(err)?,
            rows: RowIterator::new().map_err(err)?,
            cells: CellIterator::new().map_err(err)?,
            keys: key::Encoder::new().map_err(err)?,
            shared,
            plain: style::Style::default(),
        })
    }

    fn mode(&self, mode: Mode) -> bool {
        self.term.mode(mode).unwrap_or(false)
    }

    /// The hyperlink of the visible cell at (`x`, `y`).
    fn link(&self, x: u16, y: u16) -> Option<String> {
        let at = self.term.grid_ref(Point::Viewport(PointCoordinate { x, y: u32::from(y) })).ok()?;
        let mut buf = vec![0u8; 512];
        let n = match at.hyperlink_uri(&mut buf) {
            Ok(n) => n,
            Err(libghostty_vt::Error::OutOfSpace { required }) => {
                buf.resize(required, 0);
                at.hyperlink_uri(&mut buf).ok()?
            }
            Err(_) => return None,
        };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n]).into_owned())
    }

    fn fill(&mut self, frame: &mut Frame) -> Result<(), libghostty_vt::Error> {
        let rows = self.term.rows()?;
        let cols = self.term.cols()?;
        frame.begin(rows, cols);
        let mut links: Vec<(u16, u16)> = Vec::new();
        let mut grapheme = String::with_capacity(16);
        let snapshot = self.render.update(&self.term)?;
        let mut row_iter = self.rows.update(&snapshot)?;
        let mut y = 0u16;
        while let Some(row) = row_iter.next() {
            let wrapped = row.raw_row().and_then(|r| r.is_wrapped()).unwrap_or(false);
            let mut cell_iter = self.cells.update(row)?;
            let mut x = 0u16;
            while let Some(c) = cell_iter.next() {
                let raw = c.raw_cell()?;
                let style = if c.has_styling()? { c.style()? } else { self.plain };
                let mut attrs = Attrs::default();
                attrs.set(Attrs::BOLD, style.bold);
                attrs.set(Attrs::DIM, style.faint);
                attrs.set(Attrs::ITALIC, style.italic);
                attrs.set(Attrs::INVERSE, style.inverse);
                attrs.set(Attrs::INVISIBLE, style.invisible);
                attrs.set(Attrs::STRIKETHROUGH, style.strikethrough);
                attrs.set(Attrs::OVERLINE, style.overline);
                attrs.set(Attrs::BLINK, style.blink);
                let bg = match raw.content_tag()? {
                    CellContentTag::BgColorPalette => Color::Idx(raw.bg_color_palette()?.0),
                    CellContentTag::BgColorRgb => {
                        let RgbColor { r, g, b } = raw.bg_color_rgb()?;
                        Color::Rgb(r, g, b)
                    }
                    _ => color(style.bg_color),
                };
                let underline = match style.underline {
                    style::Underline::None => Underline::None,
                    style::Underline::Double => Underline::Double,
                    style::Underline::Curly => Underline::Curly,
                    style::Underline::Dotted => Underline::Dotted,
                    style::Underline::Dashed => Underline::Dashed,
                    _ => Underline::Single,
                };
                let width = match raw.wide()? {
                    CellWide::Wide => Width::Wide,
                    CellWide::SpacerTail => Width::Continuation,
                    _ => Width::Narrow,
                };
                if raw.has_hyperlink()? {
                    links.push((y, x));
                }
                let cell = Cell {
                    fg: color(style.fg_color),
                    bg,
                    underline_color: color(style.underline_color),
                    attrs,
                    underline,
                    width,
                    ..Cell::default()
                };
                // graphemes_utf8 writes the buffer from its start.
                grapheme.clear();
                if raw.has_text()? {
                    c.graphemes_utf8(&mut grapheme)?;
                }
                frame.push(cell, &grapheme);
                x += 1;
            }
            frame.end_row(wrapped);
            y += 1;
        }
        // A frame of fewer rows than the terminal (it never is) still has all of them.
        while frame.wrapped.len() < usize::from(rows) {
            frame.end_row(false);
        }
        let shape = snapshot.cursor_visual_style()?;
        let cursor = if snapshot.cursor_visible()? { snapshot.cursor_viewport()? } else { None };
        frame.cursor = cursor.map(|c| Cursor {
            row: c.y,
            col: c.x,
            shape: match shape {
                CursorVisualStyle::Bar => CursorShape::Bar,
                CursorVisualStyle::Underline => CursorShape::Underline,
                _ => CursorShape::Block,
            },
        });
        for (y, x) in links {
            if let Some(uri) = self.link(x, y) {
                let link = frame.add_link(&uri);
                let i = usize::from(y) * usize::from(cols) + usize::from(x);
                if let Some(cell) = frame.cells.get_mut(i) {
                    cell.link = link;
                }
            }
        }
        Ok(())
    }
}

/// Ghostty's physical key for a key of ours, or for the character a key types.
fn physical(input: &KeyInput<'_>) -> key::Key {
    use key::Key as G;
    use pty::Key as K;
    let ch = match input.key {
        Some(K::Up) => return G::ArrowUp,
        Some(K::Down) => return G::ArrowDown,
        Some(K::Right) => return G::ArrowRight,
        Some(K::Left) => return G::ArrowLeft,
        Some(K::Home) => return G::Home,
        Some(K::End) => return G::End,
        Some(K::Insert) => return G::Insert,
        Some(K::Delete) => return G::Delete,
        Some(K::PageUp) => return G::PageUp,
        Some(K::PageDown) => return G::PageDown,
        Some(K::Enter) => return G::Enter,
        Some(K::Tab) => return G::Tab,
        Some(K::Backspace) => return G::Backspace,
        Some(K::Escape) => return G::Escape,
        Some(K::Space) => return G::Space,
        Some(K::Char(c)) => Some(c),
        None => input.base.or_else(|| input.text.and_then(|t| t.chars().next())),
    };
    let Some(c) = ch.map(|c| c.to_ascii_lowercase()) else { return G::Unidentified };
    const LETTERS: [G; 26] = [
        G::A,
        G::B,
        G::C,
        G::D,
        G::E,
        G::F,
        G::G,
        G::H,
        G::I,
        G::J,
        G::K,
        G::L,
        G::M,
        G::N,
        G::O,
        G::P,
        G::Q,
        G::R,
        G::S,
        G::T,
        G::U,
        G::V,
        G::W,
        G::X,
        G::Y,
        G::Z,
    ];
    const DIGITS: [G; 10] =
        [G::Digit0, G::Digit1, G::Digit2, G::Digit3, G::Digit4, G::Digit5, G::Digit6, G::Digit7, G::Digit8, G::Digit9];
    match c {
        'a'..='z' => LETTERS[(c as u8 - b'a') as usize],
        '0'..='9' => DIGITS[(c as u8 - b'0') as usize],
        ' ' => G::Space,
        '`' => G::Backquote,
        '\\' => G::Backslash,
        '[' => G::BracketLeft,
        ']' => G::BracketRight,
        ',' => G::Comma,
        '=' => G::Equal,
        '-' => G::Minus,
        '.' => G::Period,
        '\'' => G::Quote,
        ';' => G::Semicolon,
        '/' => G::Slash,
        _ => G::Unidentified,
    }
}

impl Engine for Ghostty {
    fn kind(&self) -> EngineKind {
        EngineKind::Ghostty
    }

    fn write(&mut self, bytes: &[u8]) {
        self.term.vt_write(bytes);
    }

    fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.shared.borrow_mut().replies)
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        let _ = self.term.resize(cols, rows, 8, 16);
    }

    fn set_colors(&mut self, fg: Rgb, bg: Rgb) {
        let _ = self.term.set_default_fg_color(Some(rgb(fg)));
        let _ = self.term.set_default_bg_color(Some(rgb(bg)));
        self.shared.borrow_mut().dark = dark(bg);
    }

    fn scrollback(&mut self) -> (usize, usize) {
        match self.term.scrollbar() {
            Ok(s) => {
                let max = s.total.saturating_sub(s.len) as usize;
                (max.saturating_sub(s.offset as usize), max)
            }
            Err(_) => (0, 0),
        }
    }

    fn set_scrollback(&mut self, offset: usize) {
        let (_, max) = self.scrollback();
        let scroll = if offset == 0 { ScrollViewport::Bottom } else { ScrollViewport::Row(max - offset.min(max)) };
        self.term.scroll_viewport(scroll);
    }

    fn modes(&self) -> Modes {
        Modes {
            bracketed_paste: self.mode(Mode::BRACKETED_PASTE),
            app_cursor: self.mode(Mode::DECCKM),
            alt_screen: matches!(self.term.active_screen(), Ok(Screen::Alternate)),
            mouse: self.term.is_mouse_tracking().unwrap_or(false),
            mouse_sgr: self.mode(Mode::SGR_MOUSE),
            kitty_keyboard: self.term.kitty_keyboard_flags().map(|f| f.bits()).unwrap_or(0),
            sync_output: self.mode(Mode::SYNC_OUTPUT),
        }
    }

    fn frame(&mut self, frame: &mut Frame) {
        let (scrollback, history) = self.scrollback();
        if self.fill(frame).is_err() {
            let (rows, cols) = (self.term.rows().unwrap_or(1), self.term.cols().unwrap_or(1));
            frame.begin(rows, cols);
            for _ in 0..rows {
                frame.end_row(false);
            }
        }
        if scrollback != 0 {
            frame.cursor = None;
        }
        frame.scrollback = scrollback;
        frame.history = history;
        frame.title = self.term.title().unwrap_or_default().to_owned();
        frame.modes = self.modes();
        frame.engine = EngineKind::Ghostty;
    }

    /// With the kitty keyboard protocol on, Ghostty's encoder makes the bytes.
    fn encode_key(&mut self, input: &KeyInput<'_>) -> Option<Vec<u8>> {
        if self.term.kitty_keyboard_flags().map(|f| f.is_empty()).unwrap_or(true) || input.mods.cmd {
            return None;
        }
        let mut mods = key::Mods::empty();
        mods.set(key::Mods::SHIFT, input.mods.shift);
        mods.set(key::Mods::ALT, input.mods.alt);
        mods.set(key::Mods::CTRL, input.mods.ctrl);
        let mut event = key::Event::new().ok()?;
        event.set_action(key::Action::Press).set_key(physical(input)).set_mods(mods);
        let text = input.text.filter(|t| !t.is_empty() && !t.chars().any(char::is_control));
        event.set_utf8(text.map(str::to_owned));
        if let Some(base) = input.base.or_else(|| match input.key {
            Some(pty::Key::Char(c)) => Some(c.to_ascii_lowercase()),
            Some(pty::Key::Space) => Some(' '),
            _ => None,
        }) {
            event.set_unshifted_codepoint(base);
        }
        self.keys.set_options_from_terminal(&self.term).set_macos_option_as_alt(key::OptionAsAlt::True);
        let mut out = Vec::new();
        self.keys.encode_to_vec(&event, &mut out).ok()?;
        Some(out)
    }
}
