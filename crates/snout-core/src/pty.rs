//! A terminal's insides, without drawing: a child process on a pseudo-terminal, parsed
//! with `vt100`. Enough of xterm for full-screen TUIs such as Claude Code: 256 and true
//! colour, bracketed paste, application cursor keys, mouse wheel reporting, scrollback,
//! and answers to the queries TUIs send at start-up (cursor position, device attributes,
//! colours). Front ends paint [`Pty::parser`]'s screen and turn their key events into
//! bytes with [`encode_key`] and [`paste_bytes`].

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use snout_host::{Cmd, Host, PtyProcess};

const SCROLLBACK: usize = 10_000;

pub type Rgb = (u8, u8, u8);

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
pub struct Hooks {
    replies: Vec<u8>,
    pub title: String,
    /// The default colours, which TUIs ask for to pick a light or dark theme.
    pub fg: Rgb,
    pub bg: Rgb,
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

pub struct Pty {
    parser: Arc<Mutex<vt100::Parser<Hooks>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn PtyProcess>,
    shared: Arc<Shared>,
    size: (u16, u16),
    exit: Option<String>,
}

impl Pty {
    /// Start `launch` on `host` on a 24×80 terminal with default colours `fg` on `bg`. `wake`
    /// runs on the reader thread after each chunk of output and when the process ends.
    pub fn spawn(
        host: &dyn Host,
        launch: &Launch<'_>,
        fg: Rgb,
        bg: Rgb,
        wake: impl Fn() + Send + 'static,
    ) -> anyhow::Result<Self> {
        let size = (24, 80);
        let mut cmd = Cmd::new(launch.program).args(launch.args).cwd(launch.cwd);
        for name in launch.env_remove {
            cmd = cmd.env_remove(name);
        }
        cmd = cmd.env("TERM", "xterm-256color").env("COLORTERM", "truecolor");
        for (k, v) in launch.env {
            cmd = cmd.env(k, v);
        }
        let mut child = host.spawn_pty(&cmd, size.0, size.1)?;

        let hooks = Hooks { replies: Vec::new(), title: String::new(), fg, bg };
        let parser = Arc::new(Mutex::new(vt100::Parser::new_with_callbacks(size.0, size.1, SCROLLBACK, hooks)));
        let writer = Arc::new(Mutex::new(child.take_writer()?));
        let shared = Arc::new(Shared::default());

        let mut reader = child.take_reader()?;
        let (p, w, s) = (parser.clone(), writer.clone(), shared.clone());
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
                        wake();
                    }
                }
            }
            s.exited.store(true, Ordering::Relaxed);
            wake();
        })?;

        Ok(Self { parser, writer, child, shared, size, exit: None })
    }

    /// The parser, for painting its screen; hold it briefly, the reader thread waits.
    pub fn parser(&self) -> MutexGuard<'_, vt100::Parser<Hooks>> {
        lock(&self.parser)
    }

    /// `None` while running; a description of how it ended afterwards.
    pub fn exit_status(&mut self) -> Option<&str> {
        if self.exit.is_none() && self.shared.exited.load(Ordering::Relaxed) {
            let status = self.child.try_wait().ok().flatten().or_else(|| self.child.wait().ok());
            self.exit = Some(match status {
                Some(s) if s.success() => "exited".to_owned(),
                Some(s) => format!("exited with code {}", s.code.unwrap_or(-1)),
                None => "exited".to_owned(),
            });
        }
        self.exit.as_deref()
    }

    pub fn title(&self) -> String {
        self.parser().callbacks().title.clone()
    }

    pub fn output_gen(&self) -> u64 {
        self.shared.output_gen.load(Ordering::Relaxed)
    }

    pub fn last_output(&self) -> Option<Instant> {
        *lock(&self.shared.last_output)
    }

    /// The visible screen as text, one line per row.
    pub fn contents(&self) -> String {
        self.parser().screen().contents()
    }

    pub fn write(&self, bytes: &[u8]) {
        let mut w = lock(&self.writer);
        let _ = w.write_all(bytes);
        let _ = w.flush();
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// Resize to `(rows, cols)`; whether the size changed.
    pub fn resize(&mut self, size: (u16, u16)) -> bool {
        if size == self.size || size.0 == 0 || size.1 == 0 {
            return false;
        }
        self.size = size;
        self.parser().screen_mut().set_size(size.0, size.1);
        let _ = self.child.resize(size.0, size.1);
        true
    }

    /// Follow the theme, also for the colour queries TUIs send later.
    pub fn set_colors(&self, fg: Rgb, bg: Rgb) {
        let mut parser = self.parser();
        let hooks = parser.callbacks_mut();
        (hooks.fg, hooks.bg) = (fg, bg);
    }

    /// Wheel `lines` (up when positive): report it if the app asked for mouse events, move
    /// through its history with the arrow keys in the alternate screen, scroll our own
    /// scrollback otherwise. Whether our scrollback moved (a selection no longer holds).
    pub fn scroll(&self, lines: f32) -> bool {
        let n = lines.abs().round() as usize;
        if n == 0 {
            return false;
        }
        let up = lines > 0.0;
        let mut out = Vec::new();
        let moved = {
            let mut parser = self.parser();
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
                false
            } else if screen.alternate_screen() {
                let key: &[u8] = match (up, screen.application_cursor()) {
                    (true, true) => b"\x1bOA",
                    (true, false) => b"\x1b[A",
                    (false, true) => b"\x1bOB",
                    (false, false) => b"\x1b[B",
                };
                out.extend(key.repeat(n));
                false
            } else {
                let offset = screen.scrollback();
                let offset = if up { offset + n } else { offset.saturating_sub(n) };
                parser.screen_mut().set_scrollback(offset);
                true
            }
        };
        if !out.is_empty() {
            self.write(&out);
        }
        moved
    }

    /// Send typed bytes: back to the live screen first.
    pub fn send(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.parser().screen_mut().set_scrollback(0);
        self.write(bytes);
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// The text between two cells (row, col), either way round, lines trimmed.
pub fn selected_text(screen: &vt100::Screen, (a, b): ((u16, u16), (u16, u16))) -> Option<String> {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    if start == end {
        return None;
    }
    let text = screen.contents_between(start.0, start.1, end.0, end.1 + 1);
    Some(text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n"))
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ----- Input ---------------------------------------------------------------------------

/// A key as a terminal sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Right,
    Left,
    Home,
    End,
    Insert,
    Delete,
    PageUp,
    PageDown,
    Enter,
    Tab,
    Backspace,
    Escape,
    Space,
    /// A printable key, for Ctrl combinations (`c` for Ctrl+C).
    Char(char),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    /// ⌘ on macOS: its combinations belong to the app (copy, paste, shortcuts).
    pub cmd: bool,
}

/// The bytes an xterm sends for `key`; empty for keys it leaves to the app or to text input.
pub fn encode_key(key: Key, m: Mods, app_cursor: bool) -> Vec<u8> {
    if m.cmd {
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
        Key::Up => cursor('A'),
        Key::Down => cursor('B'),
        Key::Right => cursor('C'),
        Key::Left => cursor('D'),
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
    match key {
        Key::Space => Some(0),
        Key::Char(c) if c.is_ascii_alphabetic() => Some(c.to_ascii_uppercase() as u8 & 0x1f),
        Key::Char('[') => Some(0x1b),
        Key::Char('\\') => Some(0x1c),
        Key::Char(']') => Some(0x1d),
        Key::Char('/' | '-') => Some(0x1f),
        _ => None,
    }
}

/// What a paste sends: bracketed when the app asked for it, else with CR line ends.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        let t = text.replace("\x1b[201~", "");
        format!("\x1b[200~{t}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

// ----- Colours -------------------------------------------------------------------------

/// The xterm palette.
pub fn indexed(i: u8) -> Rgb {
    const BASE: [Rgb; 16] = [
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
        0..=15 => BASE[i as usize],
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (level(i / 36), level((i / 6) % 6), level(i % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            (g, g, g)
        }
    }
}

/// A cell colour, `default` for the default; bold brightens the eight base colours.
pub fn color(c: vt100::Color, default: Rgb, bold: bool) -> Rgb {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) if bold && i < 8 => indexed(i + 8),
        vt100::Color::Idx(i) => indexed(i),
        vt100::Color::Rgb(r, g, b) => (r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_encode_like_xterm() {
        let none = Mods::default();
        let with = |f: fn(&mut Mods)| {
            let mut m = Mods::default();
            f(&mut m);
            m
        };
        assert_eq!(encode_key(Key::Up, none, false), b"\x1b[A");
        assert_eq!(encode_key(Key::Up, none, true), b"\x1bOA");
        assert_eq!(encode_key(Key::Left, with(|m| m.alt = true), true), b"\x1b[1;3D");
        assert_eq!(encode_key(Key::Tab, with(|m| m.shift = true), false), b"\x1b[Z");
        assert_eq!(encode_key(Key::Enter, with(|m| m.shift = true), false), b"\x1b\r");
        assert_eq!(encode_key(Key::Char('c'), with(|m| m.ctrl = true), false), [3]);
        assert_eq!(encode_key(Key::PageDown, none, false), b"\x1b[6~");
        assert!(encode_key(Key::Char('c'), with(|m| m.cmd = true), false).is_empty());
        assert_eq!(paste_bytes("a\nb", true), b"\x1b[200~a\nb\x1b[201~");
        assert_eq!(paste_bytes("a\nb", false), b"a\rb");
    }

    #[test]
    fn palette() {
        assert_eq!(indexed(16), (0, 0, 0));
        assert_eq!(indexed(231), (255, 255, 255));
        assert_eq!(indexed(255), (238, 238, 238));
        assert_eq!(color(vt100::Color::Idx(1), (255, 255, 255), true), indexed(9));
    }

    fn wait_for(term: &Pty, what: &str) -> String {
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
        let dir = std::env::temp_dir();
        // Ask for the cursor position, then echo what the terminal answered (minus ESC).
        let script = r#"printf 'hello \033[31mred\033[0m\n'; stty raw -echo; printf '\033[6n'; r=$(dd bs=1 count=6 2>/dev/null); stty sane; printf 'reply:%s\n' "$(printf %s "$r" | tr -d '\033')"; read line; echo "got:$line""#;
        let args = ["-c".to_owned(), script.to_owned()];
        let launch = Launch { program: Path::new("/bin/sh"), args: &args, cwd: &dir, env: &[], env_remove: &[] };
        let mut term = Pty::spawn(&*snout_host::local(), &launch, (255, 255, 255), (0, 0, 0), || {}).unwrap();
        wait_for(&term, "hello red");
        wait_for(&term, "reply:[2;1R");
        {
            let parser = term.parser();
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
