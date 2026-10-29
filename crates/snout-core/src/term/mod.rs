//! The terminal engine: what turns a child's output into a screen, behind one trait, so the
//! front ends draw the same [`Frame`] whichever parser runs.
//!
//! Two engines: the built-in one on the `vt100` crate, always there, and Ghostty's
//! (`libghostty-vt`), in builds made with the `ghostty` feature. An engine lives on one
//! thread, the terminal's own (see [`crate::pty`]): Ghostty's handles may not leave the
//! thread that made them. What the front ends read is the [`Frame`] it publishes, plain
//! data that any thread can read.

mod builtin;
#[cfg(feature = "ghostty")]
mod ghostty;

use serde::{Deserialize, Serialize};

use crate::pty::{KeyInput, Rgb};

/// How many lines of scrollback a terminal keeps unless told otherwise.
pub const SCROLLBACK: usize = 10_000;

/// Which engine a terminal runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum EngineKind {
    /// The built-in parser (the `vt100` crate): in every build.
    #[default]
    Builtin,
    /// Ghostty's terminal emulation (`libghostty-vt`): in builds made with Ghostty.
    Ghostty,
}

impl EngineKind {
    pub const ALL: [Self; 2] = [Self::Builtin, Self::Ghostty];

    pub fn label(self) -> &'static str {
        match self {
            Self::Builtin => "Built-in",
            Self::Ghostty => "Ghostty",
        }
    }

    /// Why this build cannot run the engine; `None` when it can.
    pub fn unavailable(self) -> Option<&'static str> {
        match self {
            Self::Builtin => None,
            Self::Ghostty if cfg!(feature = "ghostty") => None,
            Self::Ghostty => Some("This build was made without Ghostty. Release builds include it."),
        }
    }

    /// The engine new settings start with: Ghostty when this build has it, else the
    /// built-in one.
    pub fn preferred() -> Self {
        Self::Ghostty.or_available()
    }

    /// The engines this build can run.
    pub fn available() -> Vec<Self> {
        Self::ALL.into_iter().filter(|k| k.unavailable().is_none()).collect()
    }

    /// The engine to run for this choice: itself, or the built-in one when this build lacks it.
    pub fn or_available(self) -> Self {
        if self.unavailable().is_none() { self } else { Self::Builtin }
    }

    /// A new engine of this kind on a `rows`×`cols` screen, with default colours `fg` on `bg`.
    /// Call it on the thread that will use the engine.
    pub fn create(self, rows: u16, cols: u16, fg: Rgb, bg: Rgb) -> Result<Box<dyn Engine>, String> {
        self.create_with(rows, cols, fg, bg, SCROLLBACK)
    }

    /// [`EngineKind::create`], keeping `scrollback` lines of history.
    pub fn create_with(
        self,
        rows: u16,
        cols: u16,
        fg: Rgb,
        bg: Rgb,
        scrollback: usize,
    ) -> Result<Box<dyn Engine>, String> {
        match self {
            Self::Builtin => Ok(Box::new(builtin::Builtin::new(rows, cols, fg, bg, scrollback))),
            #[cfg(feature = "ghostty")]
            Self::Ghostty => {
                ghostty::Ghostty::new(rows, cols, fg, bg, scrollback).map(|e| Box::new(e) as Box<dyn Engine>)
            }
            #[cfg(not(feature = "ghostty"))]
            Self::Ghostty => Err(self.unavailable().unwrap_or_default().to_owned()),
        }
    }
}

/// A terminal engine: parses output, keeps the screen and its scrollback, answers queries.
/// Not `Send`: it stays on the thread that made it.
pub trait Engine {
    fn kind(&self) -> EngineKind;
    /// Parse a chunk of the child's output.
    fn write(&mut self, bytes: &[u8]);
    /// What the engine answers the child (cursor position reports, device attributes...),
    /// to write back to it.
    fn take_replies(&mut self) -> Vec<u8>;
    fn resize(&mut self, rows: u16, cols: u16);
    /// The default colours, also for the colour queries TUIs send.
    fn set_colors(&mut self, fg: Rgb, bg: Rgb);
    /// How far the view is scrolled back into the history, and how far it can go.
    fn scrollback(&mut self) -> (usize, usize);
    /// Scroll the view back `offset` lines (0: the live screen), within the history.
    fn set_scrollback(&mut self, offset: usize);
    fn modes(&self) -> Modes;
    /// The visible screen, into `frame` (whose storage it reuses).
    fn frame(&mut self, frame: &mut Frame);
    /// The bytes for a key when the engine encodes keys itself (the kitty keyboard
    /// protocol); `None` leaves it to the legacy xterm encoding.
    fn encode_key(&mut self, _key: &KeyInput<'_>) -> Option<Vec<u8>> {
        None
    }

    /// Every line, the history first, as text (for search). The view stays where it was.
    fn lines(&mut self) -> Vec<String> {
        let (was, max) = self.scrollback();
        let mut frame = Frame::default();
        self.set_scrollback(max);
        self.frame(&mut frame);
        let rows = usize::from(frame.rows);
        let mut lines = vec![String::new(); max + rows];
        let mut offset = max;
        loop {
            self.set_scrollback(offset);
            let at = self.scrollback().0;
            self.frame(&mut frame);
            for row in 0..frame.rows {
                if let Some(line) = lines.get_mut(max - at + usize::from(row)) {
                    *line = frame.row_text(row).trim_end().to_owned();
                }
            }
            if offset == 0 {
                break;
            }
            offset = offset.saturating_sub(rows);
        }
        self.set_scrollback(was);
        lines
    }
}

/// The modes the child set that the front end acts on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modes {
    /// Pastes are bracketed (mode 2004).
    pub bracketed_paste: bool,
    /// Cursor keys send application sequences (DECCKM).
    pub app_cursor: bool,
    /// The alternate screen is showing (full-screen programs).
    pub alt_screen: bool,
    /// The child asked for mouse events (modes 9, 1000, 1002, 1003).
    pub mouse: bool,
    /// ...in the SGR encoding (mode 1006).
    pub mouse_sgr: bool,
    /// The kitty keyboard protocol flags the child pushed (0: off).
    pub kitty_keyboard: u8,
    /// The child is drawing a synchronized update (mode 2026): wait for its end.
    pub sync_output: bool,
}

/// A colour as the child set it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

/// Text attributes of a cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attrs(u16);

impl Attrs {
    pub const BOLD: Self = Self(1);
    pub const DIM: Self = Self(1 << 1);
    pub const ITALIC: Self = Self(1 << 2);
    pub const INVERSE: Self = Self(1 << 3);
    pub const INVISIBLE: Self = Self(1 << 4);
    pub const STRIKETHROUGH: Self = Self(1 << 5);
    pub const OVERLINE: Self = Self(1 << 6);
    pub const BLINK: Self = Self(1 << 7);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// How much of the grid a cell takes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Width {
    #[default]
    Narrow,
    /// The first of two columns.
    Wide,
    /// The second column of a wide character: nothing of its own to draw.
    Continuation,
}

/// One cell of a [`Frame`]: its text is in the frame ([`Frame::text`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cell {
    start: u32,
    len: u16,
    /// 1 + the index of its hyperlink (OSC 8) in the frame; 0 without one.
    link: u16,
    pub fg: Color,
    pub bg: Color,
    /// The underline's own colour (SGR 58); `Default` draws it in the text's.
    pub underline_color: Color,
    pub attrs: Attrs,
    pub underline: Underline,
    pub width: Width,
}

impl Cell {
    pub fn bold(&self) -> bool {
        self.attrs.contains(Attrs::BOLD)
    }

    pub fn has_link(&self) -> bool {
        self.link != 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
}

/// Where the cursor shows, in the visible screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub shape: CursorShape,
}

/// The visible screen as the engine last published it: what the front ends paint.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub rows: u16,
    pub cols: u16,
    cells: Vec<Cell>,
    text: String,
    /// Each row continues on the next (a line the width wrapped).
    wrapped: Vec<bool>,
    links: Vec<String>,
    /// `None` when hidden, or when the view is scrolled back.
    pub cursor: Option<Cursor>,
    /// How far the view is scrolled back, and how far it can go.
    pub scrollback: usize,
    pub history: usize,
    pub title: String,
    pub modes: Modes,
    /// The engine that made it.
    pub engine: EngineKind,
}

impl Frame {
    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    pub fn cell(&self, row: u16, col: u16) -> Option<&Cell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        self.cells.get(usize::from(row) * usize::from(self.cols) + usize::from(col))
    }

    /// The cell's text: a grapheme, or empty for a blank or a wide character's second half.
    pub fn text(&self, cell: &Cell) -> &str {
        let start = cell.start as usize;
        self.text.get(start..start + usize::from(cell.len)).unwrap_or("")
    }

    /// The cell's hyperlink (OSC 8), if it has one.
    pub fn link(&self, cell: &Cell) -> Option<&str> {
        (cell.link != 0).then(|| self.links.get(usize::from(cell.link) - 1).map(String::as_str)).flatten()
    }

    pub fn wrapped(&self, row: u16) -> bool {
        self.wrapped.get(usize::from(row)).copied().unwrap_or(false)
    }

    /// A row as text, one character per column (a blank as a space).
    pub fn row_text(&self, row: u16) -> String {
        self.row_columns(row).0
    }

    /// A row as text and the column each of its bytes comes from.
    fn row_columns(&self, row: u16) -> (String, Vec<u16>) {
        let mut text = String::new();
        let mut cols = Vec::new();
        for col in 0..self.cols {
            let Some(c) = self.cell(row, col) else { break };
            if c.width == Width::Continuation {
                continue;
            }
            let t = self.text(c);
            let t = if t.is_empty() { " " } else { t };
            text.push_str(t);
            cols.extend(std::iter::repeat_n(col, t.len()));
        }
        (text, cols)
    }

    /// The visible screen as text: rows trimmed, a wrapped row joined to the next.
    pub fn contents(&self) -> String {
        let mut out = String::new();
        for row in 0..self.rows {
            let text = self.row_text(row);
            if self.wrapped(row) {
                out.push_str(&text);
            } else {
                out.push_str(text.trim_end());
                if row + 1 < self.rows {
                    out.push('\n');
                }
            }
        }
        out.truncate(out.trim_end().len());
        out
    }

    /// The text between two cells (row, col), either way round: lines trimmed, wrapped rows
    /// joined.
    pub fn selected_text(&self, (a, b): ((u16, u16), (u16, u16))) -> Option<String> {
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        if start == end {
            return None;
        }
        let mut out = String::new();
        for row in start.0..=end.0.min(self.rows.saturating_sub(1)) {
            let from = if row == start.0 { start.1 } else { 0 };
            let to = if row == end.0 { end.1 } else { self.cols.saturating_sub(1) };
            let (text, cols) = self.row_columns(row);
            let bytes: String =
                text.char_indices().filter(|(i, _)| (from..=to).contains(&cols[*i])).map(|(_, c)| c).collect();
            if self.wrapped(row) && row != end.0 {
                out.push_str(&bytes);
            } else {
                out.push_str(bytes.trim_end());
                if row != end.0 {
                    out.push('\n');
                }
            }
        }
        Some(out)
    }

    /// The link under a cell: its hyperlink (OSC 8), else a web address written there.
    pub fn link_at(&self, row: u16, col: u16) -> Option<String> {
        let cell = self.cell(row, col)?;
        if let Some(link) = self.link(cell) {
            return Some(link.to_owned());
        }
        let (text, cols) = self.row_columns(row);
        url_spans(&text)
            .into_iter()
            .find(|&(s, e)| cols[s] <= col && col <= cols[e - 1])
            .map(|(s, e)| text[s..e].into())
    }

    /// Where `query` occurs in a row, ignoring case: the first and last column of each.
    pub fn find_in_row(&self, row: u16, query: &str) -> Vec<(u16, u16)> {
        let (text, cols) = self.row_columns(row);
        let text = text.trim_end();
        find_spans(text, query).into_iter().map(|(s, e)| (cols[s], cols[e - 1])).collect()
    }

    // ----- Filling, for the engines.

    fn begin(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.cells.clear();
        self.text.clear();
        self.wrapped.clear();
        self.links.clear();
        self.cursor = None;
    }

    /// Add the next cell, with its text.
    fn push(&mut self, mut cell: Cell, text: &str) {
        cell.start = self.text.len() as u32;
        cell.len = text.len().min(usize::from(u16::MAX)) as u16;
        self.text.push_str(text);
        self.cells.push(cell);
    }

    /// The link index for a cell's `link` field.
    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    fn add_link(&mut self, uri: &str) -> u16 {
        if let Some(i) = self.links.iter().position(|l| l == uri) {
            return i as u16 + 1;
        }
        self.links.push(uri.to_owned());
        self.links.len().min(usize::from(u16::MAX)) as u16
    }

    /// End a row, padding it to the width.
    fn end_row(&mut self, wrapped: bool) {
        let row = self.wrapped.len();
        let want = (row + 1) * usize::from(self.cols);
        while self.cells.len() < want {
            let start = self.text.len() as u32;
            self.cells.push(Cell { start, ..Cell::default() });
        }
        self.cells.truncate(want);
        self.wrapped.push(wrapped);
    }
}

/// Where `query` occurs in `text`, ignoring case (ASCII), as byte ranges.
pub fn find_spans(text: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let (hay, needle) = (text.to_ascii_lowercase(), query.to_ascii_lowercase());
    hay.match_indices(&needle).map(|(i, m)| (i, i + m.len())).collect()
}

/// The web addresses in `text`, as byte ranges.
pub fn url_spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = ["https://", "http://", "file://"].iter().filter_map(|s| text[from..].find(s)).min() {
        let start = from + i;
        let mut end = start;
        for (j, c) in text[start..].char_indices() {
            if c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`' | '\'' | '│') {
                break;
            }
            end = start + j + c.len_utf8();
        }
        // A sentence's punctuation, or the bracket closing around the address.
        while end > start {
            let last = text[..end].chars().next_back().unwrap_or(' ');
            let open = |o: char| text[start..end].matches(o).count();
            let unbalanced = |o, c| last == c && open(c) > open(o);
            if matches!(last, '.' | ',' | ':' | ';' | '!' | '?') || unbalanced('(', ')') || unbalanced('[', ']') {
                end -= last.len_utf8();
            } else {
                break;
            }
        }
        if end > start + 8 {
            out.push((start, end));
        }
        from = end.max(start + 1);
    }
    out
}

#[cfg(test)]
mod tests;
