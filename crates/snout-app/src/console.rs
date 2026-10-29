//! The agent console in the bottom dock: the coding agent of the settings (Claude Code,
//! Codex or a command of the user's) running in the open repository on
//! the pseudo-terminal of `snout_core::pty`, painted cell by cell from the frame its
//! terminal engine publishes (the built-in one or Ghostty's, as Settings ▸ Terminal says
//! when the session starts). One session per repository, kept alive while switching between
//! them. When the agent has printed and gone quiet, the Git views are re-read, since it may
//! have changed files.
//!
//! ⌘F finds in the terminal's text and scrollback; ⌘-click opens a link.
//!
//! `claude` starts with the variables that connect it to this window's IDE server; the
//! header says when it is connected, and switches the editor's following of its edits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle, Font, FontStyle, FontWeight, Global,
    Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement as _, Pixels, Point, Render, ScrollEvent, SharedString, StatefulInteractiveElement as _,
    StrikethroughStyle, Styled as _, Subscription, TextAlign, TextRun, UnderlineStyle, Window, canvas, div, font,
    point, px, size,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dock::{BasePanelView, panel_handle};
use gpui_component::input::{Copy, InputEvent, InputState, Paste};
use snout_core::claude::{ENV_REMOVE, find_claude};
use snout_core::pty::{self, KeyInput, Launch, Pty, Rgb};
use snout_core::term::{Attrs, CursorShape, EngineKind, Frame, Underline, Width};

use crate::look;
use crate::model::{Action, Model};
use crate::theme::{Palette, mix, palette};
use snout_core::art::Glyph;

/// Refresh the Git views once the agent has been quiet this long...
const QUIET: Duration = Duration::from_millis(800);
/// ...and at most this often while it keeps printing.
const BUSY_REFRESH: Duration = Duration::from_secs(3);
/// How long the cursor shows, and then hides, while it blinks.
const BLINK: Duration = Duration::from_millis(530);

/// How the terminals look, as the settings say: their typeface and size (the editor's
/// unless set), the line height, and the cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct TermLook {
    pub family: SharedString,
    pub size: f32,
    pub line_height: f32,
    pub cursor: crate::settings::TerminalCursor,
    pub blink: bool,
}

impl TermLook {
    pub fn of(s: &crate::settings::Settings) -> Self {
        Self {
            family: s.terminal_font.clone().unwrap_or_else(|| s.editor_font.clone()).into(),
            size: s.terminal_font_size.unwrap_or(s.editor_font_size),
            line_height: s.terminal_line_height,
            cursor: s.terminal_cursor,
            blink: s.terminal_cursor_blink,
        }
    }

    /// A row's height.
    pub fn cell_h(&self) -> Pixels {
        (px(self.size) * self.line_height).round()
    }

    /// The cursor's shape: the program's when it asked for one (bar, underline), else the
    /// settings'.
    fn shape(&self, asked: CursorShape) -> CursorShape {
        use crate::settings::TerminalCursor;
        match (asked, self.cursor) {
            (CursorShape::Block, TerminalCursor::Bar) => CursorShape::Bar,
            (CursorShape::Block, TerminalCursor::Underline) => CursorShape::Underline,
            (asked, _) => asked,
        }
    }
}

/// The cursor's rectangle at `at`, one cell of `cell` in size, in `shape`.
fn cursor_rect(at: Point<Pixels>, cell: gpui::Size<Pixels>, shape: CursorShape) -> Bounds<Pixels> {
    match shape {
        CursorShape::Block => Bounds::new(at, cell),
        CursorShape::Bar => Bounds::new(at, size(px(2.), cell.height)),
        CursorShape::Underline => Bounds::new(point(at.x, at.y + cell.height - px(2.)), size(cell.width, px(2.))),
    }
}

pub fn build<T>(model: &Entity<Model>, _window: &mut Window, cx: &mut Context<T>) -> Option<Arc<dyn BasePanelView>> {
    let console = cx.new(|cx| ConsolePanel::new(model.clone(), Kind::Claude, cx));
    Some(panel_handle(console))
}

/// The plain terminal tab beside the console: the login shell in the open repository.
pub fn build_terminal<T>(model: &Entity<Model>, cx: &mut Context<T>) -> Arc<dyn BasePanelView> {
    panel_handle(cx.new(|cx| ConsolePanel::new(model.clone(), Kind::Shell, cx)))
}

/// What a terminal panel runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Claude Code, while the console is showing.
    Claude,
    /// The user's shell (`$SHELL -l`), whenever a repository is open.
    Shell,
}

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Shell => "Terminal",
        }
    }
}

struct Session {
    pty: Pty,
    seen_gen: u64,
    refreshed: Instant,
    /// The agent it runs (the agent console's sessions).
    agent: Option<crate::agents::Agent>,
    /// The agent's turn: since the Return that began it, while it works.
    turn: Option<Instant>,
}

/// After Return, the agent counts as at work this long before it prints anything...
const TURN_START: Duration = Duration::from_millis(1_500);
/// ...and as done once it has been quiet this long.
const TURN_QUIET: Duration = Duration::from_millis(2_500);

/// The terminals running, and the engine each runs: Settings ▸ Terminal lists them, since
/// a new choice of engine holds only for the terminals started after it.
#[derive(Default)]
pub struct OpenTerminals(pub std::collections::BTreeMap<(&'static str, PathBuf), EngineKind>);

impl Global for OpenTerminals {}

/// Find in the terminal (⌘F): the query's matches in its lines, scrollback first.
struct Find {
    input: Entity<InputState>,
    /// Each match: its line (0 is the oldest in the scrollback) and which of the line's.
    matches: Vec<(usize, usize)>,
    current: Option<usize>,
    _changes: Subscription,
}

pub struct ConsolePanel {
    model: Entity<Model>,
    pub kind: Kind,
    sessions: HashMap<PathBuf, Session>,
    /// Program override (tests); otherwise `claude` is looked up.
    pub program: Option<PathBuf>,
    error: Option<String>,
    /// Anchor and end of a mouse selection, in cells (row, col).
    selection: Option<((u16, u16), (u16, u16))>,
    selecting: bool,
    /// Where the grid was last painted, and its cell size, for the pointer.
    grid: Option<(Bounds<Pixels>, gpui::Size<Pixels>)>,
    shown_gen: u64,
    /// The folders whose session is being started.
    starting: std::collections::HashSet<PathBuf>,
    find: Option<Find>,
    focus: FocusHandle,
    /// Whether the terminal had the keyboard when last drawn, and the blinking cursor's
    /// phase: shown or not, since when.
    focused: bool,
    blink_shown: bool,
    blink_since: Instant,
    /// A note over the console's corner for a moment: what was just sent to the agent.
    note: Option<(SharedString, Instant)>,
    /// The folders whose session the user ended: none starts there on its own.
    ended: std::collections::HashSet<PathBuf>,
    /// When the console was made: an agent waits a moment for the window's IDE server, so
    /// its first session starts connected, with the hooks.
    born: Instant,
}

/// How long the first session waits for the IDE server's variables at most.
const IDE_WAIT: Duration = Duration::from_secs(4);

/// How long the console's note shows.
pub const NOTE_TIME: Duration = Duration::from_millis(4_000);

impl gpui::EventEmitter<gpui_component::dock::PanelEvent> for ConsolePanel {}

impl gpui::Focusable for ConsolePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl gpui_component::dock::BasePanel for ConsolePanel {
    fn panel_name(&self) -> &'static str {
        match self.kind {
            Kind::Claude => "Claude",
            Kind::Shell => "Terminal",
        }
    }

    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl gpui_component::dock::Panel for ConsolePanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The agent's name is in the header row under it, with its status.
        match self.kind {
            Kind::Claude => "Console",
            Kind::Shell => self.kind.title(),
        }
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }

    fn zoom_control(&self, _: &App) -> Option<gpui_component::dock::PanelControl> {
        None
    }
}

fn rgb(c: Hsla) -> Rgb {
    let c: gpui::Rgba = c.into();
    let b = |v: f32| (v * 255.0).round() as u8;
    (b(c.r), b(c.g), b(c.b))
}

fn hsla((r, g, b): Rgb) -> Hsla {
    gpui::Rgba { r: f32::from(r) / 255.0, g: f32::from(g) / 255.0, b: f32::from(b) / 255.0, a: 1.0 }.into()
}

impl ConsolePanel {
    fn new(model: Entity<Model>, kind: Kind, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        cx.observe_global::<crate::claude_ide::IdeStatus>(|_, cx| cx.notify()).detach();
        cx.observe_global::<crate::claude_ide::IdeEnv>(|_, cx| cx.notify()).detach();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(IDE_WAIT).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
        // The reader thread cannot reach GPUI; look for new output on a short timer.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        Self {
            model,
            kind,
            sessions: HashMap::new(),
            program: None,
            error: None,
            selection: None,
            selecting: false,
            grid: None,
            shown_gen: 0,
            starting: Default::default(),
            find: None,
            focus: cx.focus_handle(),
            focused: false,
            blink_shown: true,
            blink_since: Instant::now(),
            note: None,
            ended: Default::default(),
            born: Instant::now(),
        }
    }

    /// Show `text` over the console's corner for a moment.
    pub fn show_note(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.note = Some((text.into(), Instant::now()));
        cx.notify();
    }

    /// The note showing now, if any.
    pub fn note(&self) -> Option<&str> {
        self.note.as_ref().filter(|(_, at)| at.elapsed() < NOTE_TIME).map(|(t, _)| t.as_ref())
    }

    /// The note over the console's lower right corner.
    fn note_view(&self, p: &Palette, cx: &App) -> Option<gpui::AnyElement> {
        let text = self.note()?.to_owned();
        let mark = p.claude().mark;
        Some(
            div()
                .absolute()
                .bottom(px(10.))
                .right(px(14.))
                .child(
                    div()
                        .id("console-note")
                        .debug_selector(|| "console-note".into())
                        .h(px(28.))
                        .pl(px(8.))
                        .pr(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .rounded(px(7.))
                        .bg(p.card)
                        .border_1()
                        .border_color(mix(mark, 0.45, p.border))
                        .shadow_md()
                        .font_family(look::ui_font(cx))
                        .text_size(px(12.))
                        .text_color(p.fg)
                        .child(crate::components::icon(crate::components::IconName::Send, 13., mark))
                        .child(div().whitespace_nowrap().child(text)),
                )
                .into_any_element(),
        )
    }

    /// Show the cursor at once and start its blinking over (after a key, a click).
    fn wake_cursor(&mut self) {
        self.blink_shown = true;
        self.blink_since = Instant::now();
    }

    /// Note which engine the session of `root` runs, or that it ended.
    fn note_engine(&self, root: &Path, cx: &mut App) {
        let engine = self.sessions.get(root).map(|s| s.pty.engine());
        let key = (self.kind.title(), root.to_owned());
        let open = &mut cx.default_global::<OpenTerminals>().0;
        match engine {
            Some(e) => open.insert(key, e),
            None => open.remove(&key),
        };
    }

    /// The engine the session of `root` runs, if one runs.
    pub fn session_engine(&self, root: &Path) -> Option<EngineKind> {
        self.sessions.get(root).map(|s| s.pty.engine())
    }

    fn root(&self, cx: &App) -> Option<PathBuf> {
        let m = self.model.read(cx);
        let on = self.kind == Kind::Shell || m.settings.console_open;
        on.then(|| m.project_root()).flatten()
    }

    /// Redraw on new output; re-read the repository once the agent goes quiet.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.note.as_ref().is_some_and(|(_, at)| at.elapsed() >= NOTE_TIME) {
            self.note = None;
            cx.notify();
        }
        if self.focused && self.model.read(cx).settings.terminal_cursor_blink {
            let shown = (self.blink_since.elapsed().as_millis() / BLINK.as_millis()).is_multiple_of(2);
            if shown != self.blink_shown {
                self.blink_shown = shown;
                cx.notify();
            }
        }
        if self.kind == Kind::Claude {
            self.track_turn(cx);
        }
        let Some(root) = self.root(cx) else { return };
        let Some(s) = self.sessions.get_mut(&root) else { return };
        if s.pty.frame_gen() != self.shown_gen {
            self.shown_gen = s.pty.frame_gen();
            cx.notify();
        }
        let generation = s.pty.output_gen();
        let quiet = s.pty.last_output().is_some_and(|t| t.elapsed() >= QUIET);
        if generation != s.seen_gen && (quiet || s.refreshed.elapsed() >= BUSY_REFRESH) {
            s.seen_gen = generation;
            s.refreshed = Instant::now();
            self.model.update(cx, |m, cx| {
                if m.job.is_none() {
                    m.refresh(cx);
                    cx.notify();
                }
            });
        }
    }

    /// The agent the settings name for the console.
    pub fn agent(&self, cx: &App) -> crate::agents::Agent {
        crate::agents::of_settings(&self.model.read(cx).settings)
    }

    /// The agent the session of `root` runs, else the one the settings name.
    pub fn session_agent(&self, root: &Path, cx: &App) -> crate::agents::Agent {
        self.sessions.get(root).and_then(|s| s.agent.clone()).unwrap_or_else(|| self.agent(cx))
    }

    /// Keep the window's notes of the console up to date: the agent the settings name, and
    /// whether the agent of the session in front is at work on a turn.
    fn track_turn(&mut self, cx: &mut Context<Self>) {
        use crate::agents::{ConsoleAgentNow, ConsoleTurn};
        let agent = self.agent(cx);
        if cx.try_global::<ConsoleAgentNow>().is_none_or(|a| a.0 != agent) {
            cx.set_global(ConsoleAgentNow(agent));
        }
        let root = self.root(cx);
        let mut ended = None;
        let now = match root.as_ref().and_then(|r| self.sessions.get_mut(r)) {
            Some(s) => {
                let last = s.pty.last_output();
                let active = s.turn.is_some_and(|t| {
                    t.elapsed() < TURN_START || last.is_some_and(|o| o > t && o.elapsed() < TURN_QUIET)
                });
                if !active && s.turn.take().is_some() {
                    ended = Some(Instant::now());
                }
                ConsoleTurn {
                    agent: s.agent.as_ref().map(|a| a.id),
                    active,
                    root: root.clone(),
                    since: s.turn,
                    last_output: last,
                    ended: None,
                }
            }
            None => ConsoleTurn::default(),
        };
        let before = cx.try_global::<ConsoleTurn>().cloned().unwrap_or_default();
        let ended = ended.or(before.ended);
        if before.agent != now.agent
            || before.active != now.active
            || before.since != now.since
            || before.ended != ended
        {
            cx.set_global(ConsoleTurn { ended, ..now });
        }
    }

    /// Start the console's program in `root`, on the workspace's host: found and started off
    /// the UI thread, the session shows once it runs.
    pub fn start(&mut self, root: &Path, args: &[String], window: &mut Window, cx: &mut Context<Self>) {
        self.sessions.remove(root);
        self.ended.remove(root);
        self.note_engine(root, cx);
        self.error = None;
        let host = crate::host::host(cx);
        let kind = self.kind;
        let agent = (kind == Kind::Claude).then(|| self.agent(cx));
        let settings = &self.model.read(cx).settings;
        let configured = match agent.as_ref().map(|a| a.id) {
            Some(crate::agents::CODEX) => settings.codex_path.clone(),
            Some(crate::agents::CUSTOM) => Some(PathBuf::from("/bin/sh")),
            _ => settings.claude_path.clone(),
        };
        let engine = settings.terminal_engine;
        let scrollback = settings.terminal_scrollback;
        let shell = Some(settings.terminal_shell.trim()).filter(|s| !s.is_empty()).map(PathBuf::from);
        let shell_args = settings.terminal_shell_args.clone();
        let added: Vec<(String, String)> = settings.terminal_env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let program = self.program.clone();
        let args = if kind == Kind::Shell && args.is_empty() { &shell_args[..] } else { args }.to_vec();
        let p = palette(cx);
        let (fg, bg) = (rgb(p.fg), rgb(p.panel));
        // The settings' variables, then for Claude Code those that connect it to this
        // window's IDE server, and its hooks report its edits.
        // Codex gets the hooks' port, to tell the end of each turn; a command of the
        // user's runs through the shell.
        let mut env = added;
        let agent_id = agent.as_ref().map(|a| a.id);
        if let Some(ide) = cx.try_global::<crate::claude_ide::IdeEnv>() {
            let take = |k: &str| match agent_id {
                Some(crate::agents::CLAUDE) => true,
                Some(crate::agents::CODEX) => k == crate::claude_ide::hooks::PORT_ENV,
                _ => false,
            };
            env.extend(ide.0.iter().filter(|(k, _)| take(k)).map(|(k, v)| ((*k).to_owned(), v.clone())));
        }
        let mut args = args;
        match agent.as_ref() {
            Some(a) if a.id == crate::agents::CLAUDE => {
                if let Some(extra) = cx.try_global::<crate::claude_ide::IdeArgs>() {
                    args.splice(0..0, extra.0.iter().cloned());
                }
            }
            Some(a) if a.id == crate::agents::CODEX => {
                if let Some(extra) = cx.try_global::<crate::claude_ide::CodexArgs>() {
                    args.splice(0..0, extra.0.iter().cloned());
                }
            }
            Some(a) => args = vec!["-c".into(), a.command.clone().unwrap_or_default()],
            None => {}
        }
        let session_agent = agent.clone();
        let root = root.to_owned();
        self.starting.insert(root.clone());
        let started = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                let found = match kind {
                    Kind::Claude if agent_id == Some(crate::agents::CODEX) => {
                        configured.or_else(|| crate::agents::find_codex(&*host))
                    }
                    Kind::Claude => configured.or_else(|| find_claude(&*host)),
                    Kind::Shell => shell.or_else(|| {
                        Some(host.info().var("SHELL").map_or_else(|| PathBuf::from("/bin/zsh"), PathBuf::from))
                    }),
                };
                let program = program.or(found).ok_or(None)?;
                let env_remove: &[&str] = if kind == Kind::Claude { ENV_REMOVE } else { &[] };
                let env: Vec<(&str, String)> = env.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                let launch = Launch { program: &program, args: &args, cwd: &root, env: &env, env_remove, scrollback };
                Pty::spawn(&*host, &launch, engine, fg, bg, || {})
                    .map_err(|e| Some(format!("Could not start {}: {e}", program.display())))
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = started.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.starting.remove(&root);
                match result {
                    Ok(pty) => {
                        let session =
                            Session { pty, seen_gen: 0, refreshed: Instant::now(), agent: session_agent, turn: None };
                        this.sessions.insert(root.clone(), session);
                        this.note_engine(&root, cx);
                        window.focus(&this.focus, cx);
                    }
                    Err(None) => {
                        this.error = Some(match agent_id {
                            Some(crate::agents::CODEX) => "Codex was not found on PATH or in ~/.local/bin, \
                                ~/.npm-global/bin, /opt/homebrew/bin, /usr/local/bin. Install it with \
                                npm install -g @openai/codex, or set its path in Settings, Agents."
                                .into(),
                            _ => "Claude Code was not found on PATH or in ~/.local/bin, ~/.claude/local, \
                                /opt/homebrew/bin, /usr/local/bin. Install it from \
                                https://docs.claude.com/en/docs/claude-code, or set its path in Settings, Agents."
                                .into(),
                        })
                    }
                    Err(Some(e)) => this.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Why the program could not start, if it could not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn session_text(&self, root: &Path) -> Option<String> {
        self.sessions.get(root).map(|s| s.pty.contents())
    }

    /// Send `bytes` to the session of `root`, as keys typed.
    pub fn send_keys(&self, root: &Path, bytes: &[u8]) {
        if let Some(s) = self.sessions.get(root) {
            s.pty.send(bytes);
        }
    }

    /// Type `text` into the session of `root`, as a paste.
    pub fn type_text(&self, root: &Path, text: &str) {
        if let Some(s) = self.sessions.get(root) {
            s.pty.paste(text);
        }
    }

    /// Send `bytes` to the session of `root` as keys typed (`\r` for Return).
    pub fn type_keys(&self, root: &Path, bytes: &[u8]) {
        if let Some(s) = self.sessions.get(root) {
            s.pty.send(bytes);
        }
    }

    fn cell_at(&self, pos: Point<Pixels>) -> Option<(u16, u16)> {
        let (b, cell) = self.grid?;
        let x = ((pos.x - b.left()) / cell.width).max(0.0) as u16;
        let y = ((pos.y - b.top()) / cell.height).max(0.0) as u16;
        Some((y, x))
    }

    fn on_key(&mut self, e: &KeyDownEvent, root: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = k.modifiers;
        if m.platform && !m.control && !m.alt && k.key == "f" {
            self.open_find(window, cx);
            cx.stop_propagation();
            return;
        }
        let meta = self.model.read(cx).settings.terminal_option_as_meta;
        let Some(s) = self.sessions.get(root) else { return };
        let mut mods = pty::Mods { shift: m.shift, alt: m.alt, ctrl: m.control, cmd: m.platform };
        let key = match k.key.as_str() {
            "up" => Some(pty::Key::Up),
            "down" => Some(pty::Key::Down),
            "left" => Some(pty::Key::Left),
            "right" => Some(pty::Key::Right),
            "home" => Some(pty::Key::Home),
            "end" => Some(pty::Key::End),
            "insert" => Some(pty::Key::Insert),
            "delete" => Some(pty::Key::Delete),
            "pageup" => Some(pty::Key::PageUp),
            "pagedown" => Some(pty::Key::PageDown),
            "enter" => Some(pty::Key::Enter),
            "tab" => Some(pty::Key::Tab),
            "backspace" => Some(pty::Key::Backspace),
            "escape" => Some(pty::Key::Escape),
            "space" if m.control => Some(pty::Key::Space),
            other if m.control && other.chars().count() == 1 => other.chars().next().map(pty::Key::Char),
            _ => None,
        };
        let mut chars = k.key.chars();
        let base = chars.next().filter(|_| chars.next().is_none());
        let base = base.or((k.key == "space").then_some(' '));
        // Option as Meta: the key itself after Esc (⌥F is Esc f, not ƒ). Otherwise Option
        // types what the keyboard layout makes of the key, as plain text.
        let meta_text = base.map(|c| if m.shift { c.to_uppercase().collect::<String>() } else { c.to_string() });
        let text = if key.is_none() && m.alt && meta {
            meta_text.as_deref()
        } else {
            if key.is_none() && m.alt {
                mods.alt = false;
            }
            k.key_char.as_deref()
        };
        let input = KeyInput { key, mods, text, base };
        let enter = key == Some(pty::Key::Enter) && !m.shift && !m.alt && !m.control && !m.platform;
        if s.pty.key(&input) {
            if enter
                && self.kind == Kind::Claude
                && let Some(s) = self.sessions.get_mut(root)
            {
                s.turn = Some(Instant::now());
            }
            self.wake_cursor();
            self.selection = None;
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// The find bar's state, while it is open: how many matches, and which one shows.
    pub fn find_state(&self) -> Option<(usize, Option<usize>)> {
        self.find.as_ref().map(|f| (f.matches.len(), f.current))
    }

    /// Open the find bar, on the selected text when there is some.
    fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.root(cx).and_then(|root| {
            let s = self.sessions.get(&root)?;
            self.selection.and_then(|sel| s.pty.frame().selected_text(sel)).filter(|t| !t.contains('\n'))
        });
        if self.find.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find in terminal"));
            let changes = cx.subscribe_in(&input, window, |this, _, e: &InputEvent, _, cx| match e {
                InputEvent::Change => this.search(cx),
                InputEvent::PressEnter { secondary, shift } => {
                    this.step_find(if *secondary || *shift { 1 } else { -1 }, cx)
                }
                _ => {}
            });
            self.find = Some(Find { input, matches: Vec::new(), current: None, _changes: changes });
        }
        let Some(input) = self.find.as_ref().map(|f| f.input.clone()) else { return };
        if let Some(text) = selected {
            input.update(cx, |i, cx| i.set_value(text, window, cx));
            self.search(cx);
        }
        input.update(cx, |i, cx| i.focus(window, cx));
        cx.notify();
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Find the query in the session's lines; show the newest match.
    fn search(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let Some(f) = self.find.as_mut() else { return };
        let query = f.input.read(cx).value().to_string();
        f.matches.clear();
        f.current = None;
        if let Some(s) = self.sessions.get(&root)
            && !query.is_empty()
        {
            for (i, line) in s.pty.lines().iter().enumerate() {
                let n = snout_core::term::find_spans(line, &query).len();
                f.matches.extend((0..n).map(|nth| (i, nth)));
            }
        }
        if !f.matches.is_empty() {
            f.current = Some(f.matches.len() - 1);
        }
        self.reveal_match(cx);
    }

    /// Go to the match `by` before (negative: older) or after the current one.
    fn step_find(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(f) = self.find.as_mut() else { return };
        let n = f.matches.len();
        if n == 0 {
            return;
        }
        let at = f.current.unwrap_or(n - 1) as isize;
        f.current = Some((at + by).rem_euclid(n as isize) as usize);
        self.reveal_match(cx);
    }

    /// Scroll so that the current match shows.
    fn reveal_match(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let (Some(f), Some(s)) = (self.find.as_ref(), self.sessions.get(&root)) else { return };
        if let Some(&(line, _)) = f.current.and_then(|i| f.matches.get(i)) {
            let (rows, history, offset) = {
                let fr = s.pty.frame();
                (usize::from(fr.rows), fr.history, fr.scrollback)
            };
            let top = history - offset.min(history);
            if line < top || line >= top + rows {
                let want_top = line.saturating_sub(rows / 2).min(history);
                s.pty.scroll_to(history - want_top);
            }
        }
        cx.notify();
    }

    /// The find bar, over the terminal.
    fn find_bar(&self, p: &Palette, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use crate::components::{IconButton, IconName};
        let f = self.find.as_ref()?;
        let count = match (f.current, f.matches.len()) {
            (_, 0) if f.input.read(cx).value().is_empty() => String::new(),
            (_, 0) => "No matches".to_owned(),
            (Some(i), n) => format!("{} of {n}", i + 1),
            (None, n) => format!("{n}"),
        };
        Some(
            div()
                .id("terminal-find")
                .debug_selector(|| "terminal-find".into())
                .key_context("TerminalFind")
                .on_action(cx.listener(|this, _: &crate::search::SearchClose, w, cx| this.close_find(w, cx)))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .py(px(4.))
                .border_b_1()
                .border_color(p.border)
                .child(look::field(&f.input, true, Some(260.), p))
                .child(div().min_w(px(70.)).text_size(px(12.)).text_color(p.fg_dim).child(count))
                .child(
                    IconButton::new("terminal-find-prev", IconName::ChevronUp, "Previous match (Return)")
                        .size(22.)
                        .on_click(cx.listener(|this, _, _, cx| this.step_find(-1, cx))),
                )
                .child(
                    IconButton::new("terminal-find-next", IconName::ChevronDown, "Next match (Shift+Return)")
                        .size(22.)
                        .on_click(cx.listener(|this, _, _, cx| this.step_find(1, cx))),
                )
                .child(
                    IconButton::new("terminal-find-close", IconName::X, "Close (Esc)")
                        .size(22.)
                        .on_click(cx.listener(|this, _, w, cx| this.close_find(w, cx))),
                )
                .into_any_element(),
        )
    }
}

impl Render for ConsolePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let root = self.root(cx).or_else(|| self.model.read(cx).project_root());
        let kind = self.kind;
        let Some(root) = root else {
            let hint = match kind {
                Kind::Claude => format!("Open a repository to run {} in it.", self.agent(cx).name),
                Kind::Shell => "Open a repository to run a shell in it.".to_owned(),
            };
            return div().size_full().bg(p.panel).flex().flex_col().child(self.header(None, &p, cx)).child(
                crate::components::EmptyState::new(snout_core::art::spots::no_project(), "No repository open")
                    .hint(hint)
                    .on(p.panel)
                    .compact(),
            );
        };
        let wanted = kind == Kind::Shell || self.model.read(cx).settings.console_open;
        // The window's IDE server gives the agent its variables a moment after the window
        // opens: the first session waits for them (a little while at most).
        let ide_ready = kind == Kind::Shell
            || cx.try_global::<crate::claude_ide::IdeConfig>().is_none()
            || cx.try_global::<crate::claude_ide::IdeEnv>().is_some_and(|e| !e.0.is_empty())
            || self.born.elapsed() >= IDE_WAIT;
        if wanted
            && ide_ready
            && !self.sessions.contains_key(&root)
            && !self.starting.contains(&root)
            && !self.ended.contains(&root)
            && self.error.is_none()
        {
            self.start(&root, &[], window, cx);
        }
        let exit = self.sessions.get_mut(&root).and_then(|s| s.pty.exit_status().map(str::to_owned));
        let body = if self.sessions.contains_key(&root) {
            self.terminal(&root, exit, window, cx).into_any_element()
        } else if self.starting.contains(&root) {
            div().flex_1().into_any_element()
        } else {
            self.not_running(&root, &p, cx)
        };
        let note = self.note_view(&p, cx);
        div()
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .child(self.header(Some(&root), &p, cx))
            .child(div().relative().flex_1().min_h_0().flex().flex_col().child(body).children(note))
    }
}

/// What the status pill says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    NotRunning,
    Starting,
    /// Running, and (for Claude Code) connected to the window: it sees the editor.
    Connected,
    /// Claude Code running without the connection (it cannot see the editor).
    NotConnected,
    Running,
    /// The agent is at work on a turn.
    Working,
    Exited,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::NotRunning => "Not running",
            Status::Starting => "Starting",
            Status::Connected => "Connected",
            Status::NotConnected => "Not connected",
            Status::Running => "Running",
            Status::Working => "Working",
            Status::Exited => "Ended",
        }
    }

    fn tip(self, agent: &str) -> String {
        match self {
            Status::NotRunning => format!("{agent} is not running in this project"),
            Status::Starting => format!("{agent} is starting"),
            Status::Connected => format!(
                "{agent} sees the editor: its selection, open files and problems; it proposes changes there for \
                 you to accept or reject, and its edits show as it makes them"
            ),
            Status::NotConnected => format!("{agent} runs, but has not connected to the editor yet"),
            Status::Running => format!("{agent} runs; the files it changes show as it works"),
            Status::Working => format!("{agent} is at work on your prompt"),
            Status::Exited => format!("{agent} has ended"),
        }
    }
}

impl ConsolePanel {
    /// What the pill in the header says for the session of `root`.
    pub fn status(&mut self, root: Option<&Path>, cx: &App) -> Status {
        let Some(root) = root else { return Status::NotRunning };
        if self.starting.contains(root) {
            return Status::Starting;
        }
        let Some(s) = self.sessions.get_mut(root) else { return Status::NotRunning };
        if s.pty.exit_status().is_some() {
            return Status::Exited;
        }
        if self.kind == Kind::Shell {
            return Status::Running;
        }
        if s.turn.is_some() {
            return Status::Working;
        }
        match s.agent.as_ref().map(|a| a.id) {
            Some(crate::agents::CLAUDE) => {
                if cx.try_global::<crate::claude_ide::IdeStatus>().is_some_and(|s| s.clients > 0) {
                    Status::Connected
                } else {
                    Status::NotConnected
                }
            }
            _ => Status::Running,
        }
    }

    /// The one header row: the agent (a menu to pick another), the status pill, the
    /// session's title, then Follow, New, Continue, End and Hide.
    fn header(&mut self, root: Option<&PathBuf>, p: &Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::components::IconName;
        let kind = self.kind;
        let agent = match root {
            Some(r) => self.session_agent(r, cx),
            None => self.agent(cx),
        };
        let status = self.status(root.map(PathBuf::as_path), cx);
        let running = matches!(status, Status::Connected | Status::NotConnected | Status::Running | Status::Working);
        let title = root
            .and_then(|r| self.sessions.get_mut(r))
            .map(|s| s.pty.title())
            .unwrap_or_default()
            .trim()
            .trim_start_matches(['✳', '✻', '·', '✶', '✢', '*', ' '])
            .to_owned();
        let color = agent.color(p);
        // The session's own title, unless it only names the agent again.
        let show_title = !title.is_empty() && running && title != agent.name.as_ref() && title != agent.short.as_ref();
        let who = match kind {
            Kind::Claude => self.agent_picker(&agent, p, cx),
            Kind::Shell => div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(crate::components::icon(IconName::SquareTerminal, 14., p.fg))
                .child(div().font_weight(gpui::FontWeight::MEDIUM).child("Terminal"))
                .into_any_element(),
        };
        let pill_color = match status {
            Status::Connected => p.ok,
            Status::Working => color,
            Status::NotConnected => p.warn,
            Status::Running | Status::Starting => p.info(),
            Status::NotRunning | Status::Exited => p.fg_dim,
        };
        let tip = status.tip(if kind == Kind::Shell { "The shell" } else { agent.name.as_ref() });
        let pill = div()
            .id("console-status")
            .debug_selector(|| "console-status".into())
            .flex_none()
            .h(px(20.))
            .pl(px(7.))
            .pr(px(8.))
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(px(10.))
            .bg(p.wash(pill_color))
            .text_color(if p.dark { pill_color } else { mix(pill_color, 0.8, p.fg) })
            .text_size(px(11.5))
            .whitespace_nowrap()
            .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
            .child(if status == Status::Working {
                crate::claude_ide::bridge::pulse_dot(pill_color, "console-status-dot")
            } else {
                div().flex_none().size(px(7.)).rounded_full().bg(pill_color).into_any_element()
            })
            .child(status.label());
        let mut buttons: Vec<gpui::AnyElement> = Vec::new();
        if kind == Kind::Claude {
            let follow = self.model.read(cx).settings.follow_claude;
            buttons.push(
                look::toggle("follow", IconName::ArrowDownToLine, "Follow", follow, p)
                    .h(px(22.))
                    .tooltip(move |w, cx| {
                        let tip = if follow {
                            "Following: the editor goes to the files the agent changes, as it types"
                        } else {
                            "Not following: the editor stays where you are; dots mark what the agent changed"
                        };
                        gpui_component::tooltip::Tooltip::new(tip).build(w, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model.update(cx, |m, cx| {
                            m.settings.follow_claude = !m.settings.follow_claude;
                            m.persist();
                            cx.notify();
                        })
                    }))
                    .into_any_element(),
            );
        }
        if let Some(root) = root.cloned() {
            let tip: &'static str = match kind {
                Kind::Claude => "Start again with a new conversation",
                Kind::Shell => "Start a new shell",
            };
            let r = root.clone();
            buttons.push(
                look::chip_button("new", Glyph::Plus, "New", true, p)
                    .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip).build(w, cx))
                    .on_click(cx.listener(move |this, _, window, cx| this.start(&r, &[], window, cx)))
                    .into_any_element(),
            );
            let resume = continue_args(&agent);
            if kind == Kind::Claude && !resume.is_empty() {
                let r = root.clone();
                buttons.push(
                    look::chip_button("continue", Glyph::Refresh, "Continue", true, p)
                        .tooltip(|w, cx| {
                            gpui_component::tooltip::Tooltip::new(
                                "Start again on the most recent conversation in this folder",
                            )
                            .build(w, cx)
                        })
                        .on_click(cx.listener(move |this, _, window, cx| this.start(&r, &resume, window, cx)))
                        .into_any_element(),
                );
            }
            if running {
                let r = root.clone();
                buttons.push(
                    look::chip_button("end", Glyph::Stop, "End", true, p)
                        .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("End this session").build(w, cx))
                        .on_click(cx.listener(move |this, _, _, cx| this.end_session(&r, cx)))
                        .into_any_element(),
                );
            }
        }
        if kind == Kind::Claude {
            buttons.push(div().w(px(2.)).into_any_element());
            buttons.push(
                look::icon_button("hide", Glyph::Close, "Hide (⌘J); the session keeps running", true, p)
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx))
                        }),
                    )
                    .into_any_element(),
            );
        }
        div()
            .id("console-header")
            .debug_selector(|| "console-header".into())
            .flex_none()
            .h(px(36.))
            .pl(px(12.))
            .pr(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .border_b_1()
            .border_color(p.border)
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .child(who)
            .child(pill)
            .when(show_title, |d| {
                d.child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_color(p.fg_dim)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(title),
                )
            })
            .when(!show_title, |d| d.child(div().flex_1()))
            .child(div().flex_none().flex().items_center().gap(px(2.)).children(buttons))
            .into_any_element()
    }

    /// The agent's name with its icon in its colour, a menu of the others under it.
    fn agent_picker(&self, agent: &crate::agents::Agent, p: &Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::agents::ConsoleAgent;
        use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
        let color = agent.color(p);
        let model = self.model.clone();
        let chosen = self.model.read(cx).settings.console_agent;
        let agents = crate::agents::known(&self.model.read(cx).settings);
        let hover = p.button_hover;
        look::Trigger(
            div()
                .id("console-agent")
                .debug_selector(|| "console-agent".into())
                .flex_none()
                .h(px(26.))
                .pl(px(4.))
                .pr(px(6.))
                .flex()
                .items_center()
                .gap(px(7.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(
                    div()
                        .flex_none()
                        .size(px(20.))
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(mix(color, 0.2, p.panel))
                        .child(crate::components::icon(agent.icon, 12., color)),
                )
                .child(div().whitespace_nowrap().font_weight(gpui::FontWeight::MEDIUM).child(agent.name.clone()))
                .child(crate::components::icon(crate::components::IconName::ChevronDown, 12., p.fg_dim)),
        )
        .dropdown_menu(move |mut menu, _, _| {
            for (a, which) in agents.iter().zip(ConsoleAgent::ALL) {
                let model = model.clone();
                menu = menu.item(
                    PopupMenuItem::new(a.name.clone())
                        .icon(crate::components::menu_icon(a.icon))
                        .checked(which == chosen)
                        .on_click(move |_, _, cx| {
                            model.update(cx, |m, cx| {
                                m.settings.console_agent = which;
                                m.persist();
                                cx.notify();
                            })
                        }),
                );
            }
            menu
        })
        .into_any_element()
    }

    /// End the session of `root`: it stays ended (no new one starts on its own) until
    /// Start, New or Continue.
    pub fn end_session(&mut self, root: &Path, cx: &mut Context<Self>) {
        if let Some(mut s) = self.sessions.remove(root) {
            s.pty.kill();
        }
        self.ended.insert(root.to_owned());
        self.note_engine(root, cx);
        cx.notify();
    }

    /// The console with no session: the agent's drawing, what it does, Start and Continue.
    fn not_running(&self, root: &Path, p: &Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::components::IconName;
        let agent = self.agent(cx);
        let (title, hint, start): (String, String, &'static str) = match self.kind {
            Kind::Shell => ("The shell is not running".into(), "Start a shell in the project.".into(), "Start Shell"),
            Kind::Claude => {
                let start = match agent.id {
                    crate::agents::CLAUDE => "Start Claude",
                    crate::agents::CODEX => "Start Codex",
                    _ => "Start Agent",
                };
                let hint = match agent.id {
                    crate::agents::CLAUDE => {
                        "It sees the editor, and its edits type themselves in, in its colour, for you to review.".into()
                    }
                    _ => "The files it changes type themselves in, in its colour, for you to review.".to_owned(),
                };
                (format!("{} is not running", agent.name), hint, start)
            }
        };
        let hint = self.error.clone().unwrap_or(hint);
        let (r1, r2) = (root.to_owned(), root.to_owned());
        let resume = continue_args(&agent);
        let continue_button = (self.kind == Kind::Claude && !resume.is_empty()).then(|| {
            look::small_button("console-continue", IconName::RotateCcw, "Continue last session", p)
                .debug_selector(|| "console-continue".into())
                .h(px(28.))
                .px(px(12.))
                .on_click(cx.listener(move |this, _, window, cx| this.start(&r2, &resume, window, cx)))
        });
        // Side by side: the console is a short pane.
        div()
            .id("console-not-running")
            .debug_selector(|| "console-not-running".into())
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(28.))
            .px(px(24.))
            .py(px(12.))
            .child(crate::art::drawing_on(snout_core::art::spots::agent_console(), 156., 104., p.panel))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .max_w(px(420.))
                    .child(div().text_size(px(14.)).font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                    .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(p.fg_dim).child(hint))
                    .child(
                        div()
                            .mt(px(10.))
                            .flex()
                            .gap(px(8.))
                            .child(
                                look::primary_button("console-start", IconName::Play, start, true, p)
                                    .debug_selector(|| "console-start".into())
                                    .on_click(cx.listener(move |this, _, window, cx| this.start(&r1, &[], window, cx))),
                            )
                            .children(continue_button),
                    ),
            )
            .into_any_element()
    }
}

impl ConsolePanel {
    fn terminal(
        &mut self,
        root: &Path,
        exit: Option<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let look = TermLook::of(&self.model.read(cx).settings);
        let (fg0, bg0) = (rgb(p.fg), rgb(p.panel));
        if let Some(s) = self.sessions.get(root) {
            s.pty.set_colors(fg0, bg0);
        }
        let view = cx.entity();
        let selection = self.selection;
        let focused = self.focus.is_focused(_window);
        if focused != self.focused {
            self.focused = focused;
            self.wake_cursor();
        }
        let cursor_shown = !(look.blink && focused) || self.blink_shown;
        let (r1, r2, r3) = (root.to_owned(), root.to_owned(), root.to_owned());
        let start = root.to_owned();
        let find_bar = self.find_bar(&p, cx);
        let grid = div()
            .id("terminal")
            .debug_selector(|| "terminal-grid".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .when_some(exit, |d, how| {
                d.child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .px(px(8.))
                        .text_color(p.fg_dim)
                        .child(format!("{} {how}.", self.kind.title()))
                        .child(
                            Button::new("again")
                                .link()
                                .icon(crate::components::menu_icon(crate::components::IconName::Play))
                                .label("Start a new session")
                                .on_click(cx.listener(move |this, _, window, cx| this.start(&start, &[], window, cx))),
                        ),
                )
            })
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, w, cx| this.on_key(e, &r1, w, cx)))
            .on_action(cx.listener(move |this, _: &Paste, _, cx| {
                let text = cx.read_from_clipboard().and_then(|c| c.text());
                if let (Some(text), Some(s)) = (text, this.sessions.get(&r2)) {
                    s.pty.paste(&text);
                }
            }))
            .on_action(cx.listener(move |this, _: &Copy, _, cx| {
                let text =
                    this.selection.zip(this.sessions.get(&r3)).and_then(|(sel, s)| s.pty.frame().selected_text(sel));
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }))
            .on_scroll(cx.listener({
                let root = root.to_owned();
                move |this, e: &ScrollEvent, _, cx| {
                    let cell_h = this.grid.map_or(px(16.), |(_, c)| c.height);
                    let lines = e.delta.pixel_delta(cell_h).y / cell_h;
                    if let Some(s) = this.sessions.get(&root)
                        && s.pty.scroll(lines)
                    {
                        this.selection = None;
                    }
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, {
                let root = root.to_owned();
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.wake_cursor();
                    let cell = this.cell_at(e.position);
                    // ⌘-click opens the link under the pointer.
                    if e.modifiers.platform
                        && let Some((row, col)) = cell
                        && let Some(link) = this.sessions.get(&root).and_then(|s| s.pty.frame().link_at(row, col))
                    {
                        cx.open_url(&link);
                        return;
                    }
                    this.selection = cell.map(|c| (c, c));
                    this.selecting = true;
                    cx.notify();
                })
            })
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.selecting
                    && e.pressed_button == Some(MouseButton::Left)
                    && let (Some(c), Some((_, end))) = (this.cell_at(e.position), this.selection.as_mut())
                {
                    *end = c;
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, {
                let root = root.to_owned();
                cx.listener(move |this, _, _, cx| {
                    this.selecting = false;
                    // Copy on select: the text selected goes to the clipboard at once.
                    if this.model.read(cx).settings.terminal_copy_on_select
                        && let Some(text) = this
                            .selection
                            .filter(|(a, b)| a != b)
                            .zip(this.sessions.get(&root))
                            .and_then(|(sel, s)| s.pty.frame().selected_text(sel))
                    {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                })
            })
            .child(
                canvas(|_, _, _| {}, {
                    let root = root.to_owned();
                    move |bounds, _, window, cx| {
                        view.update(cx, |this, cx| {
                            this.paint(&root, bounds, &look, (focused, cursor_shown), selection, &p, window, cx)
                        });
                    }
                })
                .flex_1()
                .size_full(),
            );
        div().flex_1().min_h_0().flex().flex_col().children(find_bar).child(grid)
    }

    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        root: &Path,
        bounds: Bounds<Pixels>,
        look: &TermLook,
        (focused, cursor_shown): (bool, bool),
        selection: Option<((u16, u16), (u16, u16))>,
        p: &Palette,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(s) = self.sessions.get_mut(root) else { return };
        let font: Font = font(look.family.clone());
        let font_size = px(look.size);
        let ts = window.text_system().clone();
        let id = ts.resolve_font(&font);
        let cell_w = ts.advance(id, font_size, 'M').map(|s| s.width).unwrap_or(px(8.));
        let cell_h = look.cell_h();
        let inner = Bounds::new(
            point(bounds.left() + px(6.), bounds.top() + px(4.)),
            size(bounds.size.width - px(12.), bounds.size.height - px(8.)),
        );
        let rows = ((inner.size.height / cell_h).floor() as u16).max(2);
        let cols = ((inner.size.width / cell_w).floor() as u16).max(10);
        if s.pty.resize((rows, cols)) {
            self.selection = None;
        }
        self.grid = Some((inner, size(cell_w, cell_h)));
        let frame = s.pty.frame();
        // The find bar's matches in the rows showing, the current one apart.
        let query = self.find.as_ref().map(|f| f.input.read(cx).value().to_string()).unwrap_or_default();
        let current = self.find.as_ref().and_then(|f| f.current.and_then(|i| f.matches.get(i)).copied());
        let top = frame.history - frame.scrollback.min(frame.history);
        let colors = Colors::of(p);
        let sel = selection.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });
        let selected = |r: u16, c: u16| sel.is_some_and(|(s, e)| (r, c) >= s && (r, c) <= e && s != e);
        let (found, found_now) = (p.accent.opacity(0.28), p.accent.opacity(0.6));
        let (rows, cols) = frame.size();
        for row in 0..rows {
            let hits = if query.is_empty() { Vec::new() } else { frame.find_in_row(row, &query) };
            let hit = |col: u16| {
                hits.iter()
                    .enumerate()
                    .find(|(_, (a, b))| (*a..=*b).contains(&col))
                    .map(|(nth, _)| if current == Some((top + usize::from(row), nth)) { found_now } else { found })
            };
            let mut text = String::new();
            let mut runs: Vec<TextRun> = Vec::new();
            for col in 0..cols {
                let Some(c) = frame.cell(row, col) else { continue };
                if c.width == Width::Continuation {
                    continue;
                }
                let run = cell_run(&frame, c, &font, &colors, p);
                let bg = if selected(row, col) { Some(p.selection()) } else { hit(col).or(run.1) };
                let glyph = run.0;
                runs.push(TextRun { len: glyph.len(), background_color: bg, ..run.2 });
                text.push_str(&glyph);
            }
            let line = ts.shape_line(text.into(), font_size, &runs, Some(cell_w));
            let origin = point(inner.left(), inner.top() + cell_h * f32::from(row));
            let _ = line.paint_background(origin, cell_h, TextAlign::Left, None, window, cx);
            let _ = line.paint(origin, cell_h, TextAlign::Left, None, window, cx);
        }
        if let Some(cur) = frame.cursor.filter(|_| cursor_shown) {
            let at = point(inner.left() + cell_w * f32::from(cur.col), inner.top() + cell_h * f32::from(cur.row));
            let shape = look.shape(cur.shape);
            let rect = cursor_rect(at, size(cell_w, cell_h), shape);
            if focused || shape != CursorShape::Block {
                window.paint_quad(gpui::fill(rect, p.fg.opacity(0.7)));
            } else {
                window.paint_quad(gpui::outline(rect, p.fg.opacity(0.6), gpui::BorderStyle::Solid));
            }
        }
    }
}

/// What the Terminal settings page's preview shows: a build of the shop, in colour, with the
/// text styles only some engines draw: the deprecated call struck through under a curly
/// warning line, and the test report's link.
pub const PREVIEW: &str = concat!(
    "\x1b[35mshop\x1b[0m \x1b[1;32m$\x1b[0m ./gradlew build\r\n",
    "\x1b[2m> Task :compileJava\x1b[0m\r\n",
    "OrderService.java:42: \x1b[1;33mwarning:\x1b[0m getAmount() in Money is deprecated\r\n",
    "        return order.total().\x1b[9;4:3;58;5;3mgetAmount\x1b[0m();\r\n",
    "\x1b[32m\u{2713}\x1b[0m OrderServiceTest \x1b[2m(12 tests, 0.8 s)\x1b[0m\r\n",
    "\x1b[1;32mBUILD SUCCESSFUL\x1b[0m in 4s\r\n",
    "Test report: \x1b]8;;file:///Users/alex/projects/shop/build/reports/tests/test/index.html\x1b\\",
    "\x1b[34mbuild/reports/tests/test/index.html\x1b[0m\x1b]8;;\x1b\\\r\n",
    "\x1b[35mshop\x1b[0m \x1b[1;32m$\x1b[0m ",
);

/// The preview's rows.
pub const PREVIEW_ROWS: u16 = 8;

thread_local! {
    /// The previews made: by engine, colours and width. Engines stay on the thread that
    /// made them, as their frames do here.
    static PREVIEWS: std::cell::RefCell<HashMap<(EngineKind, Rgb, Rgb, u16), Frame>> =
        std::cell::RefCell::new(HashMap::new());
}

/// [`PREVIEW`] as `engine` (or the built-in one, when this build lacks it) reads it, `cols`
/// wide, in the colours `fg` on `bg`.
fn preview_frame(engine: EngineKind, cols: u16, fg: Rgb, bg: Rgb) -> Option<Frame> {
    let engine = engine.or_available();
    let key = (engine, fg, bg, cols);
    if let Some(f) = PREVIEWS.with(|m| m.borrow().get(&key).cloned()) {
        return Some(f);
    }
    let mut e = engine.create(PREVIEW_ROWS, cols, fg, bg).ok()?;
    e.write(PREVIEW.as_bytes());
    let mut frame = Frame::default();
    e.frame(&mut frame);
    PREVIEWS.with(|m| m.borrow_mut().insert(key, frame.clone()));
    Some(frame)
}

/// A terminal's screen as `engine` draws a short session, in the theme's colours and the
/// settings' `look`: for the Terminal settings page. As tall as [`PREVIEW_ROWS`], as wide as
/// its container.
pub fn preview(engine: EngineKind, look: TermLook) -> impl IntoElement {
    let cell_h = look.cell_h();
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let p = palette(cx);
            let font: Font = font(look.family.clone());
            let font_size = px(look.size);
            let ts = window.text_system().clone();
            let id = ts.resolve_font(&font);
            let cell_w = ts.advance(id, font_size, 'M').map(|s| s.width).unwrap_or(px(8.));
            let cols = ((bounds.size.width / cell_w).floor() as u16).clamp(20, 200);
            let colors = Colors::of(&p);
            let Some(frame) = preview_frame(engine, cols, colors.fg0, colors.bg0) else { return };
            let (rows, cols) = frame.size();
            for row in 0..rows {
                let mut text = String::new();
                let mut runs: Vec<TextRun> = Vec::new();
                for col in 0..cols {
                    let Some(c) = frame.cell(row, col) else { continue };
                    if c.width == Width::Continuation {
                        continue;
                    }
                    let (glyph, bg, run) = cell_run(&frame, c, &font, &colors, &p);
                    runs.push(TextRun { len: glyph.len(), background_color: bg, ..run });
                    text.push_str(&glyph);
                }
                let line = ts.shape_line(text.into(), font_size, &runs, Some(cell_w));
                let origin = point(bounds.left(), bounds.top() + cell_h * f32::from(row));
                let _ = line.paint_background(origin, cell_h, TextAlign::Left, None, window, cx);
                let _ = line.paint(origin, cell_h, TextAlign::Left, None, window, cx);
            }
            if let Some(cur) = frame.cursor {
                let at = point(bounds.left() + cell_w * f32::from(cur.col), bounds.top() + cell_h * f32::from(cur.row));
                let rect = cursor_rect(at, size(cell_w, cell_h), look.shape(cur.shape));
                window.paint_quad(gpui::fill(rect, p.fg.opacity(0.7)));
            }
        },
    )
    .w_full()
    .h(cell_h * f32::from(PREVIEW_ROWS))
}

/// The colours a terminal paints with: the theme's text on its panel, and the theme's
/// sixteen terminal colours (see [`Palette::ansi`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub fg0: Rgb,
    pub bg0: Rgb,
    pub ansi: [Rgb; 16],
}

impl Colors {
    pub fn of(p: &Palette) -> Self {
        Self { fg0: rgb(p.fg), bg0: rgb(p.panel), ansi: p.ansi().map(rgb) }
    }

    /// The ink and ground of a cell, swapped when it is inverse. Ink a program picked from
    /// the palette, and the default text on a ground it picked from the palette, is made to
    /// read on that ground (as a terminal's minimum contrast does): bright green on cream,
    /// white on a light ground, dark text on a deep red bar.
    pub fn of_cell(&self, c: &snout_core::term::Cell) -> (Rgb, Rgb) {
        use snout_core::term::Color;
        let base = Some(&self.ansi);
        let mut fg = pty::color_in(c.fg, self.fg0, c.bold(), base);
        let mut bg = pty::color_in(c.bg, self.bg0, false, base);
        let (mut ink_from, mut ground_from) = (c.fg, c.bg);
        if c.attrs.contains(Attrs::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
            std::mem::swap(&mut ink_from, &mut ground_from);
        }
        // The palette's ink, or the default text on a palette ground (dark text on a red bar).
        let picked = matches!(ink_from, Color::Idx(_)) || matches!(ground_from, Color::Idx(_));
        if picked && !matches!(ink_from, Color::Rgb(..)) {
            fg = readable(fg, bg);
        }
        (fg, bg)
    }
}

thread_local! {
    /// Inks made readable, by ink and ground: a screen asks for the same few many times.
    static READABLE: std::cell::RefCell<HashMap<(Rgb, Rgb), Rgb>> = std::cell::RefCell::new(HashMap::new());
}

/// `ink` as [`crate::theme::readable_on`] makes it on `ground`.
fn readable(ink: Rgb, ground: Rgb) -> Rgb {
    if let Some(c) = READABLE.with(|m| m.borrow().get(&(ink, ground)).copied()) {
        return c;
    }
    let c = rgb(crate::theme::readable_on(hsla(ink), hsla(ground)));
    READABLE.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 4096 {
            m.clear();
        }
        m.insert((ink, ground), c);
    });
    c
}

/// A cell as text to shape: its glyph (a wide one kept to one cell so the grid stays
/// aligned, a space filling the other), its own background when it has one, and its look.
fn cell_run(
    frame: &Frame,
    c: &snout_core::term::Cell,
    font: &Font,
    colors: &Colors,
    p: &Palette,
) -> (String, Option<Hsla>, TextRun) {
    let (fg0, bg0) = (colors.fg0, colors.bg0);
    let (fg, bg) = colors.of_cell(c);
    let mut ink = hsla(fg);
    if c.attrs.contains(Attrs::DIM) {
        ink = mix(ink, 0.55, hsla(bg));
    }
    if c.attrs.contains(Attrs::INVISIBLE) {
        ink = hsla(bg);
    }
    let contents = frame.text(c);
    let mut glyph = if contents.is_empty() { " ".to_owned() } else { contents.to_owned() };
    if c.width == Width::Wide {
        glyph.push(' ');
    }
    let mut f = font.clone();
    if c.bold() {
        f.weight = FontWeight::BOLD;
    }
    if c.attrs.contains(Attrs::ITALIC) {
        f.style = FontStyle::Italic;
    }
    let line_color = match c.underline_color {
        snout_core::term::Color::Default => ink,
        other => hsla(pty::color_in(other, fg0, false, Some(&colors.ansi))),
    };
    let underline = match c.underline {
        Underline::None if c.has_link() => {
            Some(UnderlineStyle { thickness: px(1.), color: Some(p.accent), wavy: false })
        }
        Underline::None => None,
        Underline::Curly => Some(UnderlineStyle { thickness: px(1.), color: Some(line_color), wavy: true }),
        Underline::Double => Some(UnderlineStyle { thickness: px(2.), color: Some(line_color), wavy: false }),
        _ => Some(UnderlineStyle { thickness: px(1.), color: Some(line_color), wavy: false }),
    };
    let strikethrough =
        c.attrs.contains(Attrs::STRIKETHROUGH).then(|| StrikethroughStyle { thickness: px(1.), color: Some(ink) });
    let background = (bg != bg0).then(|| hsla(bg));
    let run = TextRun { len: glyph.len(), font: f, color: ink, background_color: None, underline, strikethrough };
    (glyph, background, run)
}

/// The docked console, if the workspace has one.
pub fn console_of(ws: &crate::workspace::Workspace, cx: &App) -> Option<Entity<ConsolePanel>> {
    let id = ws.tabs.console?;
    let view = ws.dock.read(cx).panel(id)?.clone();
    view.view().downcast::<ConsolePanel>().ok()
}

/// The arguments that start `agent` on its most recent conversation in the folder.
pub fn continue_args(agent: &crate::agents::Agent) -> Vec<String> {
    match agent.id {
        crate::agents::CLAUDE => vec!["--continue".into()],
        crate::agents::CODEX => vec!["resume".into(), "--last".into()],
        _ => Vec::new(),
    }
}
