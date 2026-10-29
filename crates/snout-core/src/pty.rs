//! A terminal's insides, without drawing: a child process on a pseudo-terminal, parsed by
//! a terminal engine ([`crate::term`]). Enough of xterm for full-screen TUIs such as Claude
//! Code: 256 and true colour, bracketed paste, application cursor keys, mouse wheel
//! reporting, scrollback, and answers to the queries TUIs send at start-up.
//!
//! Three threads per terminal: the reader takes the child's output, the terminal's own
//! thread owns the engine (which must stay on one thread) and publishes a [`Frame`] after
//! each batch of work, and the front end's thread reads that frame to paint it and sends
//! the engine what to do (resize, scroll...). Front ends turn their key events into bytes
//! with [`Pty::key`] and [`paste_bytes`].

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use snout_host::{Cmd, Host, PtyProcess};

use crate::term::{Color, Engine};
pub use crate::term::{EngineKind, Frame};

pub type Rgb = (u8, u8, u8);

/// How long a synchronized update (mode 2026) may hold the screen back.
const SYNC_TIMEOUT: Duration = Duration::from_millis(150);

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

/// State written by the reader and terminal threads.
#[derive(Default)]
struct Shared {
    exited: AtomicBool,
    /// Bumped on every chunk of output.
    output_gen: AtomicU64,
    /// Bumped on every frame published.
    frame_gen: AtomicU64,
    last_output: Mutex<Option<Instant>>,
}

/// What the terminal thread is asked to do.
enum Msg {
    Output(Vec<u8>),
    Resize(u16, u16),
    Colors(Rgb, Rgb),
    /// Scroll the view back by this many lines (forward when negative).
    Scroll(isize),
    ScrollTo(usize),
    Key {
        key: Option<Key>,
        mods: Mods,
        text: Option<String>,
        base: Option<char>,
        reply: Sender<Option<Vec<u8>>>,
    },
    Lines(Sender<Vec<String>>),
}

pub struct Pty {
    frame: Arc<Mutex<Frame>>,
    to_engine: Sender<Msg>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn PtyProcess>,
    shared: Arc<Shared>,
    size: (u16, u16),
    colors: Mutex<(Rgb, Rgb)>,
    engine: EngineKind,
    /// Why the engine asked for could not start, when the built-in one stands in.
    engine_error: Option<String>,
    exit: Option<String>,
}

impl Pty {
    /// Start `launch` on `host` on a 24×80 terminal run by `engine` (the built-in one when
    /// this build lacks it), default colours `fg` on `bg`. `wake` runs on the terminal's
    /// thread after each frame it publishes and when the process ends.
    pub fn spawn(
        host: &dyn Host,
        launch: &Launch<'_>,
        engine: EngineKind,
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
        let writer = Arc::new(Mutex::new(child.take_writer()?));
        let shared = Arc::new(Shared::default());
        let frame = Arc::new(Mutex::new(Frame::default()));
        let (to_engine, inbox) = mpsc::channel::<Msg>();

        // The terminal's own thread: it makes the engine and keeps it to the end.
        let (ready_tx, ready) = mpsc::channel();
        let (f, w, s) = (frame.clone(), writer.clone(), shared.clone());
        std::thread::Builder::new().name("terminal".into()).spawn(move || {
            let wanted = engine.or_available();
            let mut error = (wanted != engine).then(|| engine.unavailable().unwrap_or_default().to_owned());
            let made = wanted.create(size.0, size.1, fg, bg).or_else(|e| {
                error = Some(e);
                EngineKind::Builtin.create(size.0, size.1, fg, bg)
            });
            let Ok(mut e) = made else { return };
            let _ = ready_tx.send((e.kind(), error));
            run_engine(&mut *e, &inbox, &f, &w, &s, &wake);
        })?;
        let (engine, engine_error) = ready.recv().map_err(|_| anyhow::anyhow!("the terminal could not start"))?;

        let mut reader = child.take_reader()?;
        let (tx, s) = (to_engine.clone(), shared.clone());
        std::thread::Builder::new().name("terminal-reader".into()).spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(Msg::Output(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            s.exited.store(true, Ordering::Relaxed);
            // Wakes the terminal thread, which publishes and wakes the front end.
            let _ = tx.send(Msg::Scroll(0));
        })?;

        Ok(Self {
            frame,
            to_engine,
            writer,
            child,
            shared,
            size,
            colors: Mutex::new((fg, bg)),
            engine,
            engine_error,
            exit: None,
        })
    }

    /// The screen as last published, for painting it; hold it briefly, the terminal's
    /// thread waits to publish the next.
    pub fn frame(&self) -> MutexGuard<'_, Frame> {
        lock(&self.frame)
    }

    /// The engine running this terminal.
    pub fn engine(&self) -> EngineKind {
        self.engine
    }

    /// Why the engine asked for did not start, when the built-in one runs instead.
    pub fn engine_error(&self) -> Option<&str> {
        self.engine_error.as_deref()
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
        self.frame().title.clone()
    }

    pub fn output_gen(&self) -> u64 {
        self.shared.output_gen.load(Ordering::Relaxed)
    }

    /// Bumped on every frame published: output, a resize, a scroll...
    pub fn frame_gen(&self) -> u64 {
        self.shared.frame_gen.load(Ordering::Relaxed)
    }

    pub fn last_output(&self) -> Option<Instant> {
        *lock(&self.shared.last_output)
    }

    /// The visible screen as text, one line per row.
    pub fn contents(&self) -> String {
        self.frame().contents()
    }

    /// Every line, the scrollback first, as text (for search).
    pub fn lines(&self) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        if self.to_engine.send(Msg::Lines(tx)).is_err() {
            return Vec::new();
        }
        rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default()
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
        // The engine first: the child redraws for the new size once it hears of it.
        let _ = self.to_engine.send(Msg::Resize(size.0, size.1));
        let _ = self.child.resize(size.0, size.1);
        true
    }

    /// Follow the theme, also for the colour queries TUIs send later.
    pub fn set_colors(&self, fg: Rgb, bg: Rgb) {
        let mut colors = lock(&self.colors);
        if *colors != (fg, bg) {
            *colors = (fg, bg);
            let _ = self.to_engine.send(Msg::Colors(fg, bg));
        }
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
        let modes = self.frame().modes;
        let mut out = Vec::new();
        if modes.mouse {
            let button = if up { 64 } else { 65 };
            for _ in 0..n {
                if modes.mouse_sgr {
                    out.extend_from_slice(format!("\x1b[<{button};1;1M").as_bytes());
                } else {
                    out.extend_from_slice(&[0x1b, b'[', b'M', 32 + button, 33, 33]);
                }
            }
        } else if modes.alt_screen {
            let key: &[u8] = match (up, modes.app_cursor) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            out.extend(key.repeat(n));
        } else {
            let n = n as isize;
            let _ = self.to_engine.send(Msg::Scroll(if up { n } else { -n }));
            return true;
        }
        self.write(&out);
        false
    }

    /// Show the line `offset` lines back from the live screen (0: the live screen).
    pub fn scroll_to(&self, offset: usize) {
        let _ = self.to_engine.send(Msg::ScrollTo(offset));
    }

    /// Send typed bytes: back to the live screen first.
    pub fn send(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if self.frame().scrollback != 0 {
            let _ = self.to_engine.send(Msg::ScrollTo(0));
        }
        self.write(bytes);
    }

    /// Send a key: in the kitty keyboard protocol when the child turned it on and the
    /// engine speaks it, else as an xterm does. Whether it sent anything.
    pub fn key(&self, input: &KeyInput<'_>) -> bool {
        let modes = self.frame().modes;
        let bytes = if modes.kitty_keyboard != 0 {
            let (reply, rx) = mpsc::channel();
            let text = input.text.map(str::to_owned);
            let _ = self.to_engine.send(Msg::Key { key: input.key, mods: input.mods, text, base: input.base, reply });
            rx.recv_timeout(Duration::from_secs(1)).ok().flatten()
        } else {
            None
        };
        let bytes = bytes.unwrap_or_else(|| encode_input(input, modes.app_cursor));
        self.send(&bytes);
        !bytes.is_empty()
    }

    /// Paste `text`: bracketed when the child asked for it.
    pub fn paste(&self, text: &str) {
        let bracketed = self.frame().modes.bracketed_paste;
        self.send(&paste_bytes(text, bracketed));
    }
}

/// The terminal thread's work: parse what comes, do what is asked, publish the screen.
fn run_engine(
    engine: &mut dyn Engine,
    inbox: &Receiver<Msg>,
    frame: &Mutex<Frame>,
    writer: &Mutex<Box<dyn Write + Send>>,
    shared: &Shared,
    wake: &dyn Fn(),
) {
    let mut next = Frame::default();
    let mut held_since: Option<Instant> = None;
    let publish = |engine: &mut dyn Engine, next: &mut Frame| {
        engine.frame(next);
        std::mem::swap(&mut *lock(frame), next);
        shared.frame_gen.fetch_add(1, Ordering::Relaxed);
        wake();
    };
    publish(engine, &mut next);
    let mut timeout = None;
    loop {
        let first = match timeout {
            Some(t) => match inbox.recv_timeout(t) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            },
            None => match inbox.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            },
        };
        let mut output = false;
        // Everything waiting, then one frame.
        for msg in first.into_iter().chain(std::iter::from_fn(|| inbox.try_recv().ok())) {
            match msg {
                Msg::Output(bytes) => {
                    engine.write(&bytes);
                    let replies = engine.take_replies();
                    if !replies.is_empty() {
                        let mut w = lock(writer);
                        let _ = w.write_all(&replies);
                        let _ = w.flush();
                    }
                    output = true;
                }
                Msg::Resize(rows, cols) => engine.resize(rows, cols),
                Msg::Colors(fg, bg) => engine.set_colors(fg, bg),
                Msg::Scroll(by) => {
                    let (at, max) = engine.scrollback();
                    engine.set_scrollback(at.saturating_add_signed(by).min(max));
                }
                Msg::ScrollTo(offset) => engine.set_scrollback(offset),
                Msg::Key { key, mods, text, base, reply } => {
                    let _ = reply.send(engine.encode_key(&KeyInput { key, mods, text: text.as_deref(), base }));
                }
                Msg::Lines(reply) => {
                    let _ = reply.send(engine.lines());
                }
            }
        }
        if output {
            shared.output_gen.fetch_add(1, Ordering::Relaxed);
            *lock(&shared.last_output) = Some(Instant::now());
        }
        // A synchronized update shows once it ends, or after a while if it never does.
        if engine.modes().sync_output {
            let since = *held_since.get_or_insert_with(Instant::now);
            if since.elapsed() < SYNC_TIMEOUT {
                timeout = Some(SYNC_TIMEOUT - since.elapsed());
                continue;
            }
        }
        held_since = None;
        timeout = None;
        publish(engine, &mut next);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
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

/// A key event as the front end has it.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyInput<'a> {
    /// A key with a sequence of its own (arrows, Enter...), or a Ctrl combination.
    pub key: Option<Key>,
    pub mods: Mods,
    /// The text the key types, as the keyboard layout makes it.
    pub text: Option<&'a str>,
    /// The key's own character, without Shift (`1` for `!`).
    pub base: Option<char>,
}

/// The bytes an xterm sends for a key event; empty for keys it leaves to the app.
pub fn encode_input(input: &KeyInput<'_>, app_cursor: bool) -> Vec<u8> {
    let m = input.mods;
    match (input.key, input.text) {
        (Some(key), _) => encode_key(key, m, app_cursor),
        (None, Some(text)) if !m.cmd && !m.ctrl && m.alt => [b"\x1b".as_slice(), text.as_bytes()].concat(),
        (None, Some(text)) if !m.cmd && !m.ctrl => text.as_bytes().to_vec(),
        _ => Vec::new(),
    }
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
pub fn color(c: Color, default: Rgb, bold: bool) -> Rgb {
    match c {
        Color::Default => default,
        Color::Idx(i) if bold && i < 8 => indexed(i + 8),
        Color::Idx(i) => indexed(i),
        Color::Rgb(r, g, b) => (r, g, b),
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
        assert_eq!(color(Color::Idx(1), (255, 255, 255), true), indexed(9));
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
        for engine in EngineKind::available() {
            runs_a_process_on(engine);
        }
    }

    fn runs_a_process_on(engine: EngineKind) {
        let dir = std::env::temp_dir();
        // Ask for the cursor position, then echo what the terminal answered (minus ESC).
        let script = r#"printf 'hello \033[31mred\033[0m\n'; stty raw -echo; printf '\033[6n'; r=$(dd bs=1 count=6 2>/dev/null); stty sane; printf 'reply:%s\n' "$(printf %s "$r" | tr -d '\033')"; read line; echo "got:$line""#;
        let args = ["-c".to_owned(), script.to_owned()];
        let launch = Launch { program: Path::new("/bin/sh"), args: &args, cwd: &dir, env: &[], env_remove: &[] };
        let mut term = Pty::spawn(&*snout_host::local(), &launch, engine, (255, 255, 255), (0, 0, 0), || {}).unwrap();
        assert_eq!(term.engine(), engine);
        wait_for(&term, "hello red");
        wait_for(&term, "reply:[2;1R");
        {
            let frame = term.frame();
            let red = frame.cell(0, 6).unwrap();
            assert_eq!((frame.text(red), red.fg), ("r", Color::Idx(1)), "{engine:?}");
        }
        assert!(term.key(&KeyInput { text: Some("typed"), ..KeyInput::default() }));
        assert!(term.key(&KeyInput { key: Some(Key::Enter), ..KeyInput::default() }));
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
