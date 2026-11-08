//! The agent console in the bottom dock: the coding agent of the settings (Claude Code,
//! Codex, opencode, Amp or a command of the user's) running in the open repository on
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
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use snout_core::claude::ENV_REMOVE;
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
/// Whether Ctrl+C on this system copies a selection in a terminal: Windows's. Elsewhere Ctrl+C is
/// always the interrupt, and copying takes Shift.
fn ctrl_c_copies(windows: bool, only_ctrl: bool, key: &str) -> bool {
    windows && only_ctrl && key == "c"
}

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
/// The rows and columns of a terminal painted in `inner`, cells of `cell`.
fn grid_size(inner: Bounds<Pixels>, cell: gpui::Size<Pixels>) -> (u16, u16) {
    let rows = ((inner.size.height / cell.height).floor() as u16).max(2);
    let cols = ((inner.size.width / cell.width).floor() as u16).max(10);
    (rows, cols)
}

/// The session's agent is up to take text: it runs, it is not an installer, it has printed.
fn agent_up(s: &Session) -> bool {
    s.install.is_none() && s.pty.is_running() && !s.pty.contents().trim().is_empty()
}

/// The agent has nothing else on: its turn is over, it has gone quiet, and it is not asking
/// the user something first. Return must not answer "Do you trust this folder?" for them.
fn agent_idle(s: &Session) -> bool {
    let quiet = s.pty.last_output().is_some_and(|t| t.elapsed() >= IDLE_QUIET);
    settled(quiet, s.started.elapsed()) && s.turn.is_none() && !asks_the_user(&s.pty.contents())
}

/// Whether a session up for `age` has done with starting. Going quiet says so, but some agents
/// never do: Amp animates its welcome screen for as long as it waits. A turn at work still
/// counts as a turn (see `agent_idle`), so an agent past this is not taken for idle while busy.
fn settled(quiet: bool, age: Duration) -> bool {
    quiet || age >= STARTED_AFTER
}

/// The screen `text` shows the agent asking the user something: a menu of answers (a tool to
/// allow, say), or whether to trust a folder. Return would answer it for them.
pub fn asks_the_user(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_matches(|c: char| c.is_whitespace() || "│┃|".contains(c)).to_lowercase();
        let menu = line.strip_prefix('❯').or_else(|| line.strip_prefix('›')).map(str::trim_start);
        let answer = |m: &str| {
            let first = m.split(|c: char| !c.is_alphanumeric() && c != '.').next().unwrap_or_default();
            matches!(first, "1." | "yes" | "no")
        };
        menu.is_some_and(answer)
            || (line.contains("trust")
                && ["folder", "directory", "files", "hook", "project"].iter().any(|w| line.contains(w)))
    })
}

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
    /// When the session began.
    started: Instant,
    /// The agent whose installer this session runs, if it runs one.
    install: Option<&'static str>,
    /// The WSL distro that installer runs in, when not in this machine's own shell.
    install_in: Option<String>,
}

/// Text waiting to go to a folder's session.
struct Queued {
    root: PathBuf,
    text: String,
    /// Press Return after it, once the agent is idle: the prompt is sent, not just typed.
    submit: bool,
    /// When the text went in, for the Return that follows.
    pasted: Option<Instant>,
    deadline: Instant,
}

/// Why the console's agent cannot start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// The agent `id` is not installed (nor its path set).
    Program(&'static str),
    /// The custom agent has no command.
    Command,
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
    /// The link under the pointer when the terminal's right-click menu opened.
    menu_link: Option<String>,
    model: Entity<Model>,
    pub kind: Kind,
    sessions: HashMap<PathBuf, Session>,
    /// Program override (tests); otherwise `claude` is looked up.
    pub program: Option<PathBuf>,
    /// Why the session of a folder could not start.
    error: Option<(PathBuf, String)>,
    /// Anchor and end of a mouse selection, in cells (row, col).
    selection: Option<((u16, u16), (u16, u16))>,
    selecting: bool,
    /// Where the grid was last painted, and its cell size, for the pointer.
    grid: Option<(Bounds<Pixels>, gpui::Size<Pixels>)>,
    shown_gen: u64,
    /// The folders whose session is being started: the attempt that counts, and the agent it
    /// starts. A later attempt replaces an earlier one, whose result is then dropped.
    starting: HashMap<PathBuf, (u64, Option<&'static str>)>,
    /// The last attempt given out.
    attempts: u64,
    /// The next session takes the keyboard (the user asked for it, not the Draft button).
    focus_next: bool,
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
    /// Text to type into a session once its agent is up (it has printed), by folder.
    queued: Vec<Queued>,
    _queue: Option<gpui::Task<()>>,
    /// The width the header was last laid out at: a console moved to a side dock is
    /// narrow, and its toolbar keeps to icons.
    width: f32,
    /// Why a folder's agent cannot start, when it is not installed or has no command.
    missing: Option<(PathBuf, Missing)>,
    /// The agent the settings named when last looked: Settings can change it under a session.
    named: crate::agents::ConsoleAgent,
    /// Which agents are installed, as last looked for (the agent menu marks the others).
    found: HashMap<&'static str, bool>,
    /// The agents whose installer ran and finished well here.
    installed_here: std::collections::HashSet<&'static str>,
    /// Whether the agents have been looked for.
    probed: bool,
    /// The distros that have an agent its makers would run in WSL, as last looked for (Windows).
    in_wsl: HashMap<&'static str, Vec<String>>,
    /// The agents being looked for in the distros now.
    wsl_looking: std::collections::HashSet<&'static str>,
}

/// The install lines the not-installed view shows, each under its label, if it has one.
type InstallLines = Vec<(Option<&'static str>, String)>;

/// Below this width (points) the console's toolbar shows icons only.
const COMPACT_W: f32 = 560.;

/// Below this width (points) the console's header shows the agent's icon alone, and no pill.
const TINY_W: f32 = 340.;
/// Under this, the header's status pill gives way to its buttons.
const PILL_W: f32 = 480.;

/// Below this width (points) the console's empty states stack: the drawing, the gap and
/// the words beside it take some 650.
const STACKED_W: f32 = 700.;

/// How long a prompt waits for its agent (it may be installing, or finishing a turn)...
const SEND_WAIT: Duration = Duration::from_secs(300);
/// ...how long the agent must have been quiet to count as idle...
const IDLE_QUIET: Duration = Duration::from_millis(800);
/// ...or how long it must have been up, for an agent that animates while it waits.
const STARTED_AFTER: Duration = Duration::from_secs(8);
/// ...and how long after the text Return follows.
const AFTER_PASTE: Duration = Duration::from_millis(300);

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
        let named = model.read(cx).settings.console_agent;
        cx.observe(&model, |this: &mut Self, _, cx| {
            this.agent_named(cx);
            cx.notify();
        })
        .detach();
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
            queued: Vec::new(),
            _queue: None,
            program: None,
            error: None,
            selection: None,
            menu_link: None,
            selecting: false,
            grid: None,
            shown_gen: 0,
            starting: Default::default(),
            attempts: 0,
            focus_next: true,
            find: None,
            focus: cx.focus_handle(),
            focused: false,
            blink_shown: true,
            blink_since: Instant::now(),
            note: None,
            ended: Default::default(),
            born: Instant::now(),
            width: f32::MAX,
            missing: None,
            named,
            found: HashMap::new(),
            installed_here: Default::default(),
            probed: false,
            in_wsl: HashMap::new(),
            wsl_looking: Default::default(),
        }
    }

    /// The settings may name another agent now (Settings, Agents, or the settings file): what
    /// kept the last one from starting no longer holds, and a session at work is left alone.
    fn agent_named(&mut self, cx: &mut Context<Self>) {
        if self.kind != Kind::Claude {
            return;
        }
        let named = self.model.read(cx).settings.console_agent;
        if named == self.named {
            return;
        }
        self.named = named;
        self.missing = None;
        self.error = None;
        let agent = self.agent(cx);
        let root = self.root(cx);
        if root.as_ref().and_then(|r| self.sessions.get(r)).and_then(|s| s.agent.as_ref()).is_some_and(|a| *a != agent)
        {
            self.show_note(format!("{} starts with the next session", agent.name), cx);
        }
    }

    /// The window's IDE server gives the agent its variables a moment after the window opens:
    /// the first session waits for them (a little while at most).
    fn ide_ready(&self, cx: &App) -> bool {
        self.kind == Kind::Shell
            || cx.try_global::<crate::claude_ide::IdeConfig>().is_none()
            || cx.try_global::<crate::claude_ide::IdeEnv>().is_some_and(|e| !e.0.is_empty())
            || self.born.elapsed() >= IDE_WAIT
    }

    /// A new attempt at starting `root`'s session; the one before it, if any, no longer counts.
    fn begin_start(&mut self, root: &Path, agent: Option<&'static str>) -> u64 {
        self.attempts += 1;
        self.starting.insert(root.to_owned(), (self.attempts, agent));
        self.attempts
    }

    /// The attempt `attempt` at starting `root`'s session is over: whether it still counts.
    fn finish_start(&mut self, root: &Path, attempt: u64) -> bool {
        if self.starting.get(root).is_none_or(|(a, _)| *a != attempt) {
            return false;
        }
        self.starting.remove(root);
        true
    }

    /// Why `root`'s session could not start, if it could not.
    fn error_of(&self, root: &Path) -> Option<&str> {
        self.error.as_ref().filter(|(r, _)| r == root).map(|(_, e)| e.as_str())
    }

    /// Why `root`'s agent cannot start, if it cannot.
    fn missing_of(&self, root: &Path) -> Option<Missing> {
        self.missing.as_ref().filter(|(r, _)| r == root).map(|(_, m)| *m)
    }

    /// Forget why `root`'s session could not start.
    fn clear_trouble(&mut self, root: &Path) {
        self.error.take_if(|(r, _)| r == root);
        self.missing.take_if(|(r, _)| r == root);
    }

    /// The size the next session starts at: the terminal's as last painted, so an agent draws
    /// for the pane from its first frame instead of being resized under it.
    fn start_size(&self) -> Option<(u16, u16)> {
        self.grid.map(|(inner, cell)| grid_size(inner, cell))
    }

    /// Look for the agents off the UI thread, once, for the menu to mark those not
    /// installed (not when a test gives the console its program).
    fn probe(&mut self, cx: &mut Context<Self>) {
        // Not in the unit tests: it starts the user's shell, which their windows do not need.
        if self.probed || self.kind != Kind::Claude || self.program.is_some() || cfg!(test) {
            return;
        }
        self.probed = true;
        let host = crate::host::host(cx);
        let s = &self.model.read(cx).settings;
        let paths = [
            (crate::agents::CLAUDE, s.claude_path.clone()),
            (crate::agents::CODEX, s.codex_path.clone()),
            (crate::agents::OPENCODE, s.opencode_path.clone()),
            (crate::agents::AMP, s.amp_path.clone()),
        ];
        let looked = cx.background_executor().spawn(async move {
            // Side by side: on Windows each one missing can take seconds to be sure of.
            std::thread::scope(|scope| {
                let host = &host;
                paths
                    .map(|(id, path)| (id, scope.spawn(move || crate::agents::locate(&**host, id, path).is_some())))
                    .map(|(id, look)| (id, look.join().unwrap_or(false)))
            })
        });
        cx.spawn(async move |this, cx| {
            let found = looked.await;
            let _ = this.update(cx, |this, cx| {
                this.found.extend(found);
                cx.notify();
            });
        })
        .detach();
    }

    /// Run the console's agent `which` from now on: the session in front ends if it runs
    /// another agent, and the chosen one starts (or the console says how to install it).
    pub fn switch_agent(&mut self, which: crate::agents::ConsoleAgent, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.root(cx);
        // An installer at work keeps going until it is done or ended.
        if let Some(s) = root.as_ref().and_then(|r| self.sessions.get_mut(r))
            && s.install.is_some()
            && s.pty.exit_status().is_none()
        {
            self.show_note("The installer is still at work; End stops it", cx);
            return;
        }
        self.model.update(cx, |m, cx| {
            m.settings.console_agent = which;
            m.persist();
            cx.notify();
        });
        let Some(root) = root else { return };
        // With no project, picking an agent is not asking it to start in the home folder.
        if self.model.read(cx).project_root().is_none()
            && !self.sessions.contains_key(&root)
            && !self.starting.contains_key(&root)
        {
            return;
        }
        let agent = self.agent(cx);
        // It is on its way already: a second click must not start it twice.
        if !self.sessions.contains_key(&root) && self.starting.get(&root).is_some_and(|(_, a)| *a == Some(agent.id)) {
            return;
        }
        let mut note = None;
        if let Some(s) = self.sessions.get_mut(&root) {
            let same = s.install.is_none() && s.agent.as_ref().is_some_and(|a| *a == agent);
            if same && s.pty.exit_status().is_none() {
                return;
            }
            if s.install.is_none()
                && s.pty.exit_status().is_none()
                && let Some(was) = s.agent.as_ref()
            {
                let resume = if continue_args(was).is_empty() { "" } else { "; Continue resumes it" };
                note = Some(format!("{} ended{resume}", was.name));
            }
        }
        if let Some(text) = note {
            self.show_note(text, cx);
        }
        self.drop_draft_of_session(&root, cx);
        if let Some(mut s) = self.sessions.remove(&root) {
            s.pty.kill();
        }
        self.start(&root, &[], window, cx);
    }

    /// A live session is about to be replaced (another agent picked, a new session): the
    /// agent a draft asked is going, so nobody would answer. Say so now, not after the wait.
    fn drop_draft_of_session(&self, root: &Path, cx: &mut Context<Self>) {
        let alive = self.sessions.get(root).is_some_and(|s| s.pty.is_running());
        let waits = self.model.read(cx).draft.as_ref().is_some_and(|d| d.root == root);
        if alive && waits {
            let name = self.agent(cx).name;
            let why = format!("Draft stopped: the console started a new session. Click Draft to ask {name}.");
            self.model.update(cx, |m, cx| m.draft_gone(why, cx));
        }
    }

    /// Run the installer of the agent `id` in the console, in the user's shell (or in the WSL
    /// distro `distro`'s): its output shows as it goes, and the agent starts once it is done.
    pub fn install(
        &mut self,
        root: &Path,
        id: &'static str,
        distro: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let host = crate::host::host(cx);
        let windows = host.info().is_windows() && distro.is_none();
        let Some(line) = crate::agents::install_line(id, windows) else { return };
        if let Some(mut s) = self.sessions.remove(root) {
            s.pty.kill();
        }
        self.ended.remove(root);
        self.clear_trouble(root);
        let settings = &self.model.read(cx).settings;
        let engine = settings.terminal_engine;
        let scrollback = settings.terminal_scrollback;
        let p = palette(cx);
        let (fg, bg) = (rgb(p.fg), rgb(p.panel));
        let root = root.to_owned();
        let attempt = self.begin_start(&root, Some(id));
        let size = self.start_size();
        let started = cx.background_executor().spawn({
            let root = root.clone();
            let distro = distro.clone();
            async move {
                let (program, args): (PathBuf, Vec<String>) = if let Some(d) = distro {
                    // A login shell of the distro's: its PATH, as its own terminal has it.
                    let args = ["-d", d.as_str(), "-e", "bash", "-lc", line].map(str::to_owned);
                    (PathBuf::from("wsl.exe"), args.to_vec())
                } else if windows {
                    let ps = snout_host::which(&*host, &["pwsh", "powershell"], &[])
                        .unwrap_or_else(|| PathBuf::from("powershell.exe"));
                    (ps, vec!["-NoProfile".into(), "-Command".into(), line.into()])
                } else {
                    // Interactive and login: the PATH of the user's own terminal (npm, nvm).
                    (default_shell(&*host), vec!["-ilc".into(), line.into()])
                };
                let launch =
                    Launch { program: &program, args: &args, cwd: &root, env: &[], env_remove: &[], scrollback, size };
                Pty::spawn(&*host, &launch, engine, fg, bg, || {})
                    .map_err(|e| format!("Could not start {}: {e}", program.display()))
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = started.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.finish_start(&root, attempt) {
                    return;
                }
                match result {
                    Ok(pty) => {
                        let session = Session {
                            pty,
                            seen_gen: 0,
                            refreshed: Instant::now(),
                            agent: None,
                            turn: None,
                            started: Instant::now(),
                            install: Some(id),
                            install_in: distro.clone(),
                        };
                        this.sessions.insert(root.clone(), session);
                        this.note_engine(&root, cx);
                        window.focus(&this.focus, cx);
                    }
                    Err(e) => this.error = Some((root.clone(), e)),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The installer of `root`'s session finished well: start its agent. One installed in a
    /// distro is looked for there again, for the console to offer a folder of it.
    fn after_install(&mut self, root: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, distro)) = self.sessions.get_mut(root).and_then(|s| {
            let id = s.install?;
            (s.pty.exit_status() == Some("exited")).then(|| (id, s.install_in.clone()))
        }) else {
            return;
        };
        self.sessions.remove(root);
        if distro.is_some() {
            self.in_wsl.remove(id);
        } else {
            self.installed_here.insert(id);
            self.found.insert(id, true);
        }
        self.start(root, &[], window, cx);
    }

    /// Look for the agent `id` in this machine's WSL distros, off the UI thread, when its makers
    /// would have it run there and the console runs on Windows: the console offers its distro.
    /// Again at each start that does not find it, for one installed there since.
    fn look_in_wsl(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let host = crate::host::host(cx);
        // Not in the unit tests: it starts the distros.
        if !host.info().is_windows() || !crate::agents::wsl_first(id) || cfg!(test) || !self.wsl_looking.insert(id) {
            return;
        }
        let Some(name) = crate::agents::program_name(id) else { return };
        let looked = cx.background_executor().spawn(async move { crate::wsl::distros_with(name) });
        cx.spawn(async move |this, cx| {
            let found = looked.await;
            let _ = this.update(cx, |this, cx| {
                this.wsl_looking.remove(id);
                this.in_wsl.insert(id, found);
                cx.notify();
            });
        })
        .detach();
    }

    fn compact(&self) -> bool {
        self.width < COMPACT_W
    }

    /// The console is too narrow for the agent's name and the status pill as well as the
    /// toolbar: the toolbar stays.
    fn tiny(&self) -> bool {
        self.width < TINY_W
    }

    /// The empty states stack their drawing over their words: side by side they would not
    /// fit.
    fn stacked(&self) -> bool {
        self.width < STACKED_W
    }

    /// A toolbar chip: its glyph and label, or its glyph alone when the console is narrow
    /// (its tooltip says what it does).
    fn chip(&self, id: &'static str, g: Glyph, label: &'static str, p: &Palette) -> gpui::Stateful<gpui::Div> {
        if self.compact() {
            look::chip_button(id, g, "", true, p).gap(px(0.)).px(px(4.))
        } else {
            look::chip_button(id, g, label, true, p)
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
        on.then(|| self.folder(cx)).flatten()
    }

    /// The folder the console works in: the project, else the home folder, so that an agent can
    /// make the project in the first place.
    pub fn folder(&self, cx: &App) -> Option<PathBuf> {
        self.model.read(cx).project_root().or_else(|| {
            let host = crate::host::host(cx);
            let home = &host.info().home;
            (!home.as_os_str().is_empty()).then(|| home.clone())
        })
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
            self.watch_draft(cx);
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
        let root = self.root(cx);
        // What Ask to Fix and the like name, and reach: the agent showing in the console.
        let agent = root.as_ref().map_or_else(|| self.agent(cx), |r| self.session_agent(r, cx));
        if cx.try_global::<ConsoleAgentNow>().is_none_or(|a| a.0 != agent) {
            cx.set_global(ConsoleAgentNow(agent));
        }
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
        let focus = std::mem::replace(&mut self.focus_next, true);
        self.drop_draft_of_session(root, cx);
        self.sessions.remove(root);
        self.ended.remove(root);
        self.note_engine(root, cx);
        self.clear_trouble(root);
        // What was selected or searched for belongs to the session just gone.
        self.selection = None;
        self.find = None;
        self.starting.remove(root);
        let host = crate::host::host(cx);
        let kind = self.kind;
        let agent = (kind == Kind::Claude).then(|| self.agent(cx));
        if agent.as_ref().is_some_and(|a| a.command.as_deref().is_some_and(|c| c.trim().is_empty())) {
            self.missing = Some((root.to_owned(), Missing::Command));
            cx.notify();
            return;
        }
        let settings = &self.model.read(cx).settings;
        let configured = match agent.as_ref().map(|a| a.id) {
            Some(crate::agents::CODEX) => settings.codex_path.clone(),
            Some(crate::agents::OPENCODE) => settings.opencode_path.clone(),
            Some(crate::agents::AMP) => settings.amp_path.clone(),
            Some(crate::agents::CUSTOM) => {
                Some(PathBuf::from(if host.info().is_windows() { "cmd.exe" } else { "/bin/sh" }))
            }
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
        // Codex, opencode and Amp get the hooks' port and the token's file, for their hooks
        // or SnoutCode's plugin to report their edits, and for the editor's API; opencode the
        // variable that loads the plugin. A command of the user's runs through the shell.
        let mut env = added;
        let agent_id = agent.as_ref().map(|a| a.id);
        let mut hooked = false;
        if let Some(ide) = cx.try_global::<crate::claude_ide::IdeEnv>() {
            let is_api = |k: &str| {
                k == crate::claude_ide::hooks::PORT_ENV
                    || k == crate::claude_ide::api::HEADER_ENV
                    || k == crate::claude_ide::hooks::SCRIPT_ENV
            };
            let take = |k: &str| match agent_id {
                Some(crate::agents::CLAUDE) => k != crate::claude_ide::hooks::TOKEN_ENV,
                // Its MCP client reads the token from a variable.
                Some(crate::agents::CODEX) => is_api(k) || k == crate::claude_ide::hooks::TOKEN_ENV,
                // A command of the user's own can call the API too.
                Some(crate::agents::OPENCODE | crate::agents::AMP | crate::agents::CUSTOM) => is_api(k),
                _ => false,
            };
            hooked = ide.0.iter().any(|(k, _)| *k == crate::claude_ide::hooks::PORT_ENV);
            env.extend(ide.0.iter().filter(|(k, _)| take(k)).map(|(k, v)| ((*k).to_owned(), v.clone())));
        }
        match agent_id {
            Some(crate::agents::OPENCODE) => {
                if let Some(extra) = cx.try_global::<crate::claude_ide::OpencodeEnv>() {
                    env.extend(extra.0.iter().map(|(k, v)| ((*k).to_owned(), v.clone())));
                }
            }
            // Amp loads plugins only with PLUGINS=all, unless the user says otherwise.
            Some(crate::agents::AMP)
                if hooked && host.info().var("PLUGINS").is_none() && !env.iter().any(|(k, _)| k == "PLUGINS") =>
            {
                env.push(("PLUGINS".into(), "all".into()));
            }
            _ => {}
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
            // A command of the user's own runs through the shell; the other agents take
            // their arguments as they are (Continue's among them).
            Some(a) if a.id == crate::agents::CUSTOM => {
                let command = a.command.clone().unwrap_or_default();
                args = if host.info().is_windows() {
                    vec!["/d".into(), "/c".into(), command]
                } else {
                    vec!["-c".into(), command]
                };
            }
            Some(_) | None => {}
        }
        let session_agent = agent.clone();
        let program_name = agent.as_ref().and_then(|a| a.program());
        let agent_name = agent.as_ref().map(|a| a.name.to_string()).unwrap_or_default();
        let root = root.to_owned();
        let attempt = self.begin_start(&root, agent_id);
        let size = self.start_size();
        let started = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                // SnoutCode's plugin in Amp's plugins folder, for its edits to be reported.
                if agent_id == Some(crate::agents::AMP)
                    && hooked
                    && let Err(e) = crate::claude_ide::hooks::install_amp_plugin(&*host)
                {
                    eprintln!("Amp integration: could not put the plugin in Amp's plugins folder: {e}");
                }
                let found = match kind {
                    Kind::Claude if program.is_some() => None,
                    Kind::Claude if program_name.is_some() || agent_id.is_none() => {
                        crate::agents::locate(&*host, agent_id.unwrap_or(crate::agents::CLAUDE), configured)
                    }
                    Kind::Claude => configured,
                    Kind::Shell => shell.or_else(|| Some(default_shell(&*host))),
                };
                let program = program.or(found).ok_or(None)?;
                let env_remove: &[&str] = if kind == Kind::Claude { ENV_REMOVE } else { &[] };
                let mut env: Vec<(&str, String)> = env.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                // Windows: what was installed since this app started is on the user's PATH, not on
                // the one it started with, and an agent npm installed looks for `node` there.
                let path = crate::agents::windows_fresh_path(&*host);
                if let Some(path) = &path {
                    env.push(("PATH", path.clone()));
                }
                let launch =
                    Launch { program: &program, args: &args, cwd: &root, env: &env, env_remove, scrollback, size };
                Pty::spawn(&*host, &launch, engine, fg, bg, || {})
                    .map_err(|e| Some(format!("Could not start {}: {e}", program.display())))
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = started.await;
            let _ = this.update_in(cx, |this, window, cx| {
                // Another attempt took over meanwhile (the user picked another agent): this
                // one's session ends here, and neither its result nor its trouble counts.
                if !this.finish_start(&root, attempt) {
                    return;
                }
                match result {
                    Ok(pty) => {
                        if let Some(a) = session_agent.as_ref() {
                            this.found.insert(a.id, true);
                        }
                        let session = Session {
                            pty,
                            seen_gen: 0,
                            refreshed: Instant::now(),
                            agent: session_agent,
                            turn: None,
                            started: Instant::now(),
                            install: None,
                            install_in: None,
                        };
                        this.sessions.insert(root.clone(), session);
                        this.note_engine(&root, cx);
                        if focus {
                            window.focus(&this.focus, cx);
                        }
                    }
                    Err(None) => {
                        let id = agent_id.unwrap_or(crate::agents::CLAUDE);
                        this.found.insert(id, false);
                        if this.installed_here.contains(id) {
                            let e = format!(
                                "The installer finished, but {agent_name} is not on the PATH SnoutCode sees. \
                                 Set its path in Settings, Agents, or restart SnoutCode."
                            );
                            this.error = Some((root.clone(), e));
                        } else {
                            this.missing = Some((root.clone(), Missing::Program(id)));
                            this.look_in_wsl(id, cx);
                        }
                    }
                    Err(Some(e)) => this.error = Some((root.clone(), e)),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Why the program could not start, if it could not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_ref().map(|(_, e)| e.as_str())
    }

    pub fn session_text(&self, root: &Path) -> Option<String> {
        self.sessions.get(root).map(|s| s.pty.contents())
    }

    /// How long `root`'s session has printed nothing.
    pub fn quiet_for(&self, root: &Path) -> Option<Duration> {
        self.sessions.get(root)?.pty.last_output().map(|t| t.elapsed())
    }

    /// Whether `root`'s session runs an agent's installer.
    pub fn installing(&self, root: &Path) -> bool {
        self.sessions.get(root).is_some_and(|s| s.install.is_some())
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

    /// Type `text` into the session of `root` as a paste once its agent is up (the session
    /// runs and has printed): at once when it is, else as soon as it is (within 30 seconds).
    pub fn type_when_ready(&mut self, root: &Path, text: &str, cx: &mut Context<Self>) {
        if self.sessions.get(root).is_some_and(agent_up) {
            return self.type_text(root, text);
        }
        self.enqueue(root, text, false, Duration::from_secs(30), cx);
    }

    /// Send `text` to the agent of `root` as a prompt: typed and Return pressed once the agent
    /// is up and has nothing else on (its turn over, no question of its own on the screen).
    pub fn send_when_ready(&mut self, root: &Path, text: &str, cx: &mut Context<Self>) {
        self.enqueue(root, text, true, SEND_WAIT, cx);
    }

    /// Take back a prompt that is waiting, or stop the turn it began: whether it was sent.
    pub fn take_back(&mut self, root: &Path) -> bool {
        let before = self.queued.len();
        self.queued.retain(|q| !(q.submit && q.root == root));
        let waiting = self.queued.len() != before;
        if !waiting && let Some(s) = self.sessions.get(root) {
            s.pty.send(b"\x1b");
        }
        !waiting
    }

    /// Run `commands` in `root`'s shell, once it is up, as if typed: the user's own click asked.
    pub fn run_when_ready(&mut self, root: &Path, commands: &str, cx: &mut Context<Self>) {
        let (root, keys) = (root.to_owned(), format!("{}\r", commands.trim_end().replace('\n', "\r")));
        cx.spawn(async move |this, cx| {
            for _ in 0..150 {
                let sent = this.update(cx, |this, cx| {
                    let Some(s) = this.sessions.get_mut(&root).filter(|s| agent_up(s)) else { return false };
                    s.pty.send(keys.as_bytes());
                    cx.notify();
                    true
                });
                if !matches!(sent, Ok(false)) {
                    return;
                }
                cx.background_executor().timer(Duration::from_millis(200)).await;
            }
        })
        .detach();
    }

    fn enqueue(&mut self, root: &Path, text: &str, submit: bool, wait: Duration, cx: &mut Context<Self>) {
        let deadline = Instant::now() + wait;
        self.queued.push(Queued { root: root.to_owned(), text: text.to_owned(), submit, pasted: None, deadline });
        if self._queue.is_some() {
            return;
        }
        self._queue = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                let left = this.update(cx, |this, cx| this.send_queued(cx));
                if !matches!(left, Ok(true)) {
                    break;
                }
            }
            let _ = this.update(cx, |this, _| this._queue = None);
        }));
    }

    /// Send what can go now: whether anything is still waiting.
    fn send_queued(&mut self, cx: &mut Context<Self>) -> bool {
        let now = Instant::now();
        let mut sent = false;
        // A prompt goes only while its draft waits: stopped or given up, it stays unsent.
        let asked = self.model.read(cx).draft.as_ref().map(|d| d.root.clone());
        let sessions = &mut self.sessions;
        self.queued.retain_mut(|q| {
            if now > q.deadline || (q.submit && asked.as_ref() != Some(&q.root)) {
                return false;
            }
            let Some(s) = sessions.get_mut(&q.root) else { return true };
            if let Some(at) = q.pasted {
                // Return a moment after the text, or the agent takes it as part of the paste.
                if at.elapsed() < AFTER_PASTE {
                    return true;
                }
                s.pty.send(b"\r");
                s.turn = Some(Instant::now());
                sent = true;
                return false;
            }
            if !agent_up(s) || (q.submit && !agent_idle(s)) {
                return true;
            }
            s.pty.paste(&q.text);
            if q.submit {
                q.pasted = Some(Instant::now());
            }
            q.submit
        });
        if sent {
            self.model.update(cx, |m, cx| m.draft_sent(cx));
        }
        !self.queued.is_empty()
    }

    /// Make sure `root` has a session for a prompt to go to, without taking the keyboard: the
    /// user may be typing in the commit message.
    pub fn ensure_session(&mut self, root: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let alive = self.sessions.get_mut(root).is_some_and(|s| s.pty.exit_status().is_none());
        if alive || self.starting.contains_key(root) {
            return;
        }
        self.focus_next = false;
        if self.ide_ready(cx) {
            self.start(root, &[], window, cx);
        } else {
            // The window's IDE server is still on its way: the first draw starts the agent.
            self.ended.remove(root);
            self.clear_trouble(root);
        }
    }

    /// Why a prompt waiting for `root`'s session will not reach it, while it is waiting: what
    /// the Draft button tells the user when the agent is not there to ask.
    fn prompt_stuck(&mut self, root: &Path, agent: &crate::agents::Agent, cx: &App) -> Option<String> {
        if self.starting.contains_key(root) || (!self.sessions.contains_key(root) && !self.ide_ready(cx)) {
            return None;
        }
        if let Some(missing) = self.missing_of(root) {
            return Some(match missing {
                Missing::Program(_) => {
                    format!("{} is not installed yet. Install it in the console, then click Draft again.", agent.name)
                }
                Missing::Command => {
                    "Your own agent has no command yet. Give it one in Settings, Agents, then click Draft again."
                        .to_owned()
                }
            });
        }
        if let Some(e) = self.error_of(root) {
            return Some(e.to_owned());
        }
        let alive = self.sessions.get_mut(root).is_some_and(|s| s.pty.exit_status().is_none());
        (!alive).then(|| format!("{} is not running.", agent.name))
    }

    /// While a draft of the commit message waits on the agent: give up when the agent is gone.
    fn watch_draft(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.model.read(cx).draft.as_ref().map(|d| d.root.clone()) else { return };
        let agent = self.session_agent(&root, cx);
        let asking = self.sessions.get(&root).is_some_and(|s| s.pty.is_running() && asks_the_user(&s.pty.contents()));
        self.model.update(cx, |m, cx| m.draft_asking(asking, &agent.short, cx));
        if let Some(why) = self.prompt_stuck(&root, &agent, cx) {
            self.queued.retain(|q| !(q.submit && q.root == root));
            self.show_note(why.clone(), cx);
            self.model.update(cx, |m, cx| m.draft_gone(why, cx));
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

    /// What is selected in `root`'s terminal, if anything is.
    fn selected_text(&self, root: &Path) -> Option<String> {
        let sel = self.selection.filter(|(a, b)| a != b)?;
        self.sessions.get(root).and_then(|s| s.pty.frame().selected_text(sel))
    }

    /// Put what is selected on the clipboard. False when nothing is.
    fn copy_selection(&mut self, root: &Path, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.selected_text(root) else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    /// Type what is on the clipboard into `root`'s terminal.
    fn paste_clipboard(&mut self, root: &Path, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|c| c.text());
        let Some(s) = self.sessions.get(root) else { return };
        match text {
            Some(text) => s.pty.paste(&text),
            // Windows: no text to paste (a screenshot, say), so Ctrl+V goes to the program as it
            // would have: an agent may attach the image, and vim and the shell have uses for the key.
            None if cfg!(windows) => s.pty.send(&[0x16]),
            None => {}
        }
    }

    fn on_key(&mut self, e: &KeyDownEvent, root: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = k.modifiers;
        // Windows: Ctrl+C copies what is selected, as Windows Terminal has it. Sent on as an
        // interrupt instead, it would stop what is showing, such as an agent's login, with nothing
        // copied. With nothing selected it interrupts.
        if ctrl_c_copies(cfg!(windows), m.control && !m.shift && !m.alt && !m.platform, &k.key)
            && self.copy_selection(root, cx)
        {
            self.selection = None;
            cx.notify();
            cx.stop_propagation();
            return;
        }
        // Find: ⌘F on a Mac; Ctrl+Shift+F on Linux and Windows, where Ctrl+F belongs to the shell.
        let find = if cfg!(target_os = "macos") {
            m.platform && !m.control && !m.alt
        } else {
            m.control && m.shift && !m.alt && !m.platform
        };
        if find && k.key == "f" {
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
        let root = self.root(cx).or_else(|| self.folder(cx));
        let kind = self.kind;
        let Some(root) = root else {
            let hint = match kind {
                Kind::Claude => format!("Open a project to run {} in it.", self.agent(cx).name),
                Kind::Shell => "Open a project to run a shell in it.".to_owned(),
            };
            return div().size_full().bg(p.panel).flex().flex_col().child(self.header(None, &p, cx)).child(
                crate::components::EmptyState::new(snout_core::art::spots::no_project(), "No project open")
                    .hint(hint)
                    .on(p.panel)
                    .compact(),
            );
        };
        self.probe(cx);
        // An agent acts on the folder it is in: in the home folder, only when asked to start.
        let wanted = kind == Kind::Shell
            || (self.model.read(cx).settings.console_open && self.model.read(cx).project_root().is_some());
        if wanted
            && self.ide_ready(cx)
            && !self.sessions.contains_key(&root)
            && !self.starting.contains_key(&root)
            && !self.ended.contains(&root)
            && self.error_of(&root).is_none()
            && self.missing_of(&root).is_none()
        {
            self.start(&root, &[], window, cx);
        }
        self.after_install(&root, window, cx);
        let exit = self.sessions.get_mut(&root).and_then(|s| s.pty.exit_status().map(str::to_owned));
        let body = if self.sessions.contains_key(&root) {
            self.terminal(&root, exit, window, cx).into_any_element()
        } else if self.starting.contains_key(&root) {
            self.starting_view(&root, &p, cx)
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
    /// An agent's installer runs.
    Installing,
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
            Status::Installing => "Installing",
            Status::Exited => "Ended",
        }
    }

    fn tip(self, agent: &str) -> String {
        match self {
            Status::NotRunning => format!("{agent} is not running"),
            Status::Starting => format!("{agent} is starting"),
            Status::Connected => format!(
                "{agent} sees the editor: its selection, open files and problems; it proposes changes there for \
                 you to accept or reject, and its edits show as it makes them"
            ),
            Status::NotConnected => format!("{agent} runs, but has not connected to the editor yet"),
            Status::Running => format!("{agent} runs; the files it changes show as it works"),
            Status::Working => format!("{agent} is at work on your prompt"),
            Status::Installing => format!("{agent} is being installed"),
            Status::Exited => format!("{agent} has ended"),
        }
    }
}

impl ConsolePanel {
    /// What the pill in the header says for the session of `root`.
    pub fn status(&mut self, root: Option<&Path>, cx: &App) -> Status {
        let Some(root) = root else { return Status::NotRunning };
        if self.starting.contains_key(root) {
            return Status::Starting;
        }
        let Some(s) = self.sessions.get_mut(root) else { return Status::NotRunning };
        if s.pty.exit_status().is_some() {
            return Status::Exited;
        }
        if self.kind == Kind::Shell {
            return Status::Running;
        }
        if s.install.is_some() {
            return Status::Installing;
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
        let running = matches!(
            status,
            Status::Connected | Status::NotConnected | Status::Running | Status::Working | Status::Installing
        );
        let title = root
            .and_then(|r| self.sessions.get_mut(r))
            .map(|s| s.pty.title())
            .unwrap_or_default()
            .trim()
            .trim_start_matches(['✳', '✻', '·', '✶', '✢', '*', ' '])
            .to_owned();
        let color = agent.color(p);
        // The session's own title, unless it only names the agent again.
        let show_title = !title.is_empty()
            && running
            && !self.compact()
            && title != agent.name.as_ref()
            && title != agent.short.as_ref();
        let who = match kind {
            Kind::Claude => self.agent_picker(&agent, p, cx),
            // The dock's title says Terminal: the row names the shell it runs.
            Kind::Shell => {
                let m = self.model.read(cx);
                let program = Some(m.settings.terminal_shell.trim())
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| default_shell_name(crate::host::host(cx).info()));
                let name = std::path::Path::new(&program)
                    .file_name()
                    .map_or_else(|| program.clone(), |n| n.to_string_lossy().into_owned());
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(crate::components::icon(IconName::SquareTerminal, 14., p.fg_dim))
                    .child(div().font_weight(gpui::FontWeight::MEDIUM).child(name))
                    .into_any_element()
            }
        };
        let pill_color = match status {
            Status::Connected => p.ok,
            Status::Working => color,
            Status::NotConnected => p.warn,
            Status::Running | Status::Starting | Status::Installing => p.info(),
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
                look::toggle(
                    "follow",
                    IconName::ArrowDownToLine,
                    if self.compact() { "" } else { "Follow" },
                    follow,
                    p,
                )
                .h(px(22.))
                .when(self.compact(), |d| d.gap(px(0.)).px(px(5.)))
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
                self.chip("new", Glyph::Plus, "New", p)
                    .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip).build(w, cx))
                    .on_click(cx.listener(move |this, _, window, cx| this.start(&r, &[], window, cx)))
                    .into_any_element(),
            );
            let resume = continue_args(&agent);
            if kind == Kind::Claude && !resume.is_empty() {
                let r = root.clone();
                buttons.push(
                    self.chip("continue", Glyph::Refresh, "Continue", p)
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
                    self.chip("end", Glyph::Stop, "End", p)
                        .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("End this session").build(w, cx))
                        .on_click(cx.listener(move |this, _, _, cx| this.end_session(&r, cx)))
                        .into_any_element(),
                );
            }
        }
        // Hide: in the Git view's console pane; as a tool window its header's Hide does it.
        let in_git = self.model.read(cx).settings.activity == Some(crate::panels::Activity::Git);
        if kind == Kind::Claude && in_git {
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
            .relative()
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
            .when(self.width >= PILL_W, |d| d.child(pill))
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
            .when(!show_title, |d| d.child(div().flex_1().min_w_0()))
            .child(div().flex_none().flex().items_center().gap(px(2.)).children(buttons))
            .child(self.measure(cx))
            .into_any_element()
    }

    /// An empty layer over the header that notes the console's width as it is laid out,
    /// and draws it again when it crosses into or out of the compact toolbar.
    fn measure(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let view = cx.entity();
        canvas(
            move |bounds, _, cx| {
                let w = f32::from(bounds.size.width);
                view.update(cx, |this, cx| {
                    let was = (this.compact(), this.stacked(), this.tiny(), this.width >= PILL_W);
                    this.width = w;
                    if was != (this.compact(), this.stacked(), this.tiny(), this.width >= PILL_W) {
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }

    /// The agent's name with its icon in its colour, a menu of the others under it.
    fn agent_picker(&self, agent: &crate::agents::Agent, p: &Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::agents::ConsoleAgent;
        use gpui_component::menu::DropdownMenu as _;
        let color = agent.color(p);
        let view = cx.entity();
        // The agent showing, which a project's session can keep after the settings moved on.
        let chosen = agent.id;
        let agents = crate::agents::known(&self.model.read(cx).settings);
        // Those not installed, or (the custom agent) with no command yet, say so.
        let labels: Vec<SharedString> = agents
            .iter()
            .map(|a| match a.command.as_deref() {
                Some(c) if c.trim().is_empty() => format!("{} (set up)", a.name).into(),
                None if self.found.get(a.id) == Some(&false) => format!("{} (install)", a.name).into(),
                _ => a.name.clone(),
            })
            .collect();
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
                .when(!self.tiny(), |d| {
                    d.child(div().whitespace_nowrap().font_weight(gpui::FontWeight::MEDIUM).child(agent.name.clone()))
                })
                .child(crate::components::icon(crate::components::IconName::ChevronDown, 12., p.fg_dim)),
        )
        .dropdown_menu(move |mut menu, _, _| {
            for ((a, label), which) in agents.iter().zip(&labels).zip(ConsoleAgent::ALL) {
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(label.clone())
                        .icon(crate::components::menu_icon(a.icon))
                        .checked(which.id() == chosen)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| this.switch_agent(which, window, cx))
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
        self.starting.remove(root);
        self.ended.insert(root.to_owned());
        self.note_engine(root, cx);
        cx.notify();
    }

    /// The console while a session is on its way: looking for an agent can take a moment
    /// (the user's shell may be asked), and an empty pane looks like nothing happened.
    fn starting_view(&self, root: &Path, p: &Palette, cx: &App) -> gpui::AnyElement {
        let what = match self.starting.get(root) {
            _ if self.kind == Kind::Shell => "Starting the shell\u{2026}".to_owned(),
            Some((_, Some(id))) => {
                format!("Starting {}\u{2026}", crate::agents::Agent::of(id, &self.model.read(cx).settings).name)
            }
            _ => format!("Starting {}\u{2026}", self.agent(cx).name),
        };
        div()
            .debug_selector(|| "console-starting".into())
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .px(px(16.))
            .child(gpui_component::Sizable::xsmall(gpui_component::spinner::Spinner::new()))
            .child(div().min_w_0().text_size(px(12.5)).text_color(p.fg_dim).child(what))
            .into_any_element()
    }

    /// The console with no session: the agent's drawing, what it does, Start and Continue.
    fn not_running(&self, root: &Path, p: &Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::components::IconName;
        let agent = self.agent(cx);
        if self.kind == Kind::Claude
            && let Some(missing) = self.missing_of(root)
        {
            return self.missing_view(root, &agent, missing, p, cx);
        }
        let (title, hint, start): (String, String, &'static str) = match self.kind {
            Kind::Shell => ("The shell is not running".into(), "Start a shell in the project.".into(), "Start Shell"),
            Kind::Claude => {
                let start = match agent.id {
                    crate::agents::CLAUDE => "Start Claude",
                    crate::agents::CODEX => "Start Codex",
                    crate::agents::OPENCODE => "Start opencode",
                    crate::agents::AMP => "Start Amp",
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
        let hint = if self.kind == Kind::Claude && self.model.read(cx).project_root().is_none() {
            format!(
                "No project is open: {} starts in your home folder, where it can make one. Open the project when it \
                 is there, to follow its edits.",
                agent.name
            )
        } else {
            hint
        };
        let hint = self.error_of(root).map_or(hint, str::to_owned);
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
            .overflow_y_scroll()
            .child(
                div()
                    .min_h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(28.))
                    .px(px(24.))
                    .py(px(12.))
                    .when(self.stacked(), |d| d.flex_col().gap(px(16.)))
                    .child(div().flex_none().child(crate::art::drawing_on(
                        snout_core::art::spots::agent_console(),
                        156.,
                        104.,
                        p.panel,
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .max_w(px(420.))
                            .min_w_0()
                            .when(self.stacked(), |d| d.w_full())
                            .child(div().text_size(px(14.)).font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                            .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(p.fg_dim).child(hint))
                            .child(
                                div()
                                    .mt(px(10.))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(8.))
                                    .child(
                                        look::primary_button("console-start", IconName::Play, start, true, p)
                                            .debug_selector(|| "console-start".into())
                                            .on_click(
                                                cx.listener(move |this, _, window, cx| {
                                                    this.start(&r1, &[], window, cx)
                                                }),
                                            ),
                                    )
                                    .children(continue_button),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl ConsolePanel {
    /// The console when its agent cannot start: not installed (Install runs its installer
    /// here), or the custom agent with no command (Settings, Agents names one).
    fn missing_view(
        &self,
        root: &Path,
        agent: &crate::agents::Agent,
        missing: Missing,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        use crate::components::IconName;
        let host = crate::host::host(cx);
        if let Missing::Program(id) = missing
            && host.info().is_windows()
            && crate::agents::wsl_first(id)
        {
            let (title, hint, lines, buttons) = self.wsl_guide(root, agent, id, p, cx);
            return self.missing_layout(title, hint, lines, buttons, p, cx);
        }
        let line = match missing {
            Missing::Program(id) => crate::agents::install_command(id, host.info()),
            Missing::Command => None,
        };
        let (title, hint) = match (missing, line) {
            (Missing::Program(_), Some(line)) => {
                let needs = if line.starts_with("npm ") { " It needs Node.js." } else { "" };
                (
                    format!("{} is not installed", agent.name),
                    format!(
                        "Install runs its installer here in the console.{needs} If it is installed somewhere \
                         else, set its path in Settings, Agents."
                    ),
                )
            }
            (Missing::Program(_), None) => {
                (format!("{} is not installed", agent.name), "Set its path in Settings, Agents.".to_owned())
            }
            (Missing::Command, _) => (
                "Your own agent has no command yet".to_owned(),
                "Name the command line agent to run here, and the name to show for it, in Settings, Agents.".to_owned(),
            ),
        };
        let settings_button = |primary: bool| {
            let on_click =
                cx.listener(|_, _, window, cx| window.dispatch_action(Box::new(crate::menus::SettingsAgents), cx));
            if primary {
                look::primary_button("console-agent-settings", IconName::Settings, "Agent Settings", true, p)
                    .debug_selector(|| "console-agent-settings".into())
                    .on_click(on_click)
                    .into_any_element()
            } else {
                look::small_button("console-agent-settings", IconName::Settings, "Set Its Path", p)
                    .debug_selector(|| "console-agent-settings".into())
                    .h(px(28.))
                    .px(px(12.))
                    .on_click(on_click)
                    .into_any_element()
            }
        };
        let mut buttons: Vec<gpui::AnyElement> = Vec::new();
        if let (Missing::Program(id), Some(_)) = (missing, line) {
            let r = root.to_owned();
            buttons.push(
                look::primary_button("console-install", IconName::Download, install_label(id), true, p)
                    .debug_selector(|| "console-install".into())
                    .on_click(cx.listener(move |this, _, window, cx| this.install(&r, id, None, window, cx)))
                    .into_any_element(),
            );
            buttons.push(settings_button(false));
        } else {
            buttons.push(settings_button(true));
        }
        let lines = line.map(|l| vec![(None, l.to_owned())]).unwrap_or_default();
        self.missing_layout(title, hint, lines, buttons, p, cx)
    }

    /// What the console says, on Windows, of an agent whose makers would have it run in WSL and
    /// that is not installed on Windows: the distro that has it, with a way to a folder there;
    /// else how to install it in a distro, and after that how to on Windows itself.
    fn wsl_guide(
        &self,
        root: &Path,
        agent: &crate::agents::Agent,
        id: &'static str,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> (String, String, InstallLines, Vec<gpui::AnyElement>) {
        use crate::components::IconName;
        let name = agent.name.clone();
        let unix = crate::agents::install_line(id, false).unwrap_or_default().to_owned();
        let windows = crate::agents::install_line(id, true).unwrap_or_default().to_owned();
        let makers = format!("{name}'s developers recommend running it in WSL.");
        let mut buttons: Vec<gpui::AnyElement> = Vec::new();
        let settings = look::small_button("console-agent-settings", IconName::Settings, "Set Its Path", p)
            .debug_selector(|| "console-agent-settings".into())
            .h(px(28.))
            .px(px(12.))
            .on_click(
                cx.listener(|_, _, window, cx| window.dispatch_action(Box::new(crate::menus::SettingsAgents), cx)),
            )
            .into_any_element();
        if self.wsl_looking.contains(id) && !self.in_wsl.contains_key(id) {
            let title = format!("Looking for {name} in WSL…");
            return (title, makers, Vec::new(), vec![settings]);
        }
        let has = self.in_wsl.get(id).cloned().unwrap_or_default();
        if !has.is_empty() {
            for (i, d) in has.iter().take(3).enumerate() {
                let host = format!("{}{d}", snout_remote::WSL_PREFIX);
                buttons.push(
                    look::primary_button(
                        ("console-open-wsl", i),
                        IconName::FolderOpen,
                        format!("Open a Folder in {d}"),
                        true,
                        p,
                    )
                    .debug_selector(|| "console-open-wsl".into())
                    .on_click(cx.listener(move |_, _, window, cx| {
                        window.dispatch_action(Box::new(crate::menus::ConnectHost(host.clone())), cx)
                    }))
                    .into_any_element(),
                );
            }
            buttons.push(settings);
            let title = format!("{name} is installed in {}", has.join(" and "));
            let hint = format!("{makers} Open a folder in {} and the console runs {name} there.", has[0]);
            return (title, hint, Vec::new(), buttons);
        }
        let distros = cx.try_global::<crate::remote::Remote>().map(|r| r.wsl_distros.clone()).unwrap_or_default();
        let title = format!("{name} is not installed");
        let Some(first) = distros.first().cloned() else {
            let hint = format!(
                "{makers} This PC has no WSL distribution yet: `wsl --install` in PowerShell sets one up. \
                 To run {name} on Windows itself instead, install it with the line below, or set its path."
            );
            return (title, hint, vec![(Some("On Windows itself"), windows)], vec![settings]);
        };
        let hint = format!(
            "{makers} Install runs its installer in {first}; then open a folder there and the console runs \
             {name} in it. On Windows itself, it installs with the second line."
        );
        for (i, d) in distros.iter().take(3).enumerate() {
            let (r, d) = (root.to_owned(), d.clone());
            let label = format!("Install in {d}");
            let button = if i == 0 {
                look::primary_button(("console-install-wsl", i), IconName::Download, label, true, p)
            } else {
                look::small_button(("console-install-wsl", i), IconName::Download, label, p).h(px(28.)).px(px(12.))
            };
            buttons.push(
                button
                    .debug_selector(|| "console-install-wsl".into())
                    .on_click(cx.listener(move |this, _, window, cx| this.install(&r, id, Some(d.clone()), window, cx)))
                    .into_any_element(),
            );
        }
        buttons.push(settings);
        (title, hint, vec![(Some("In WSL"), unix), (Some("On Windows itself"), windows)], buttons)
    }

    /// The not-installed view: the agent's drawing beside `title`, `hint`, the install `lines`
    /// (each under its label, if it has one) and `buttons`.
    fn missing_layout(
        &self,
        title: String,
        hint: String,
        lines: InstallLines,
        buttons: Vec<gpui::AnyElement>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mono = look::mono_font(cx);
        let lines = lines.into_iter().map(|(label, line)| {
            div()
                .mt(px(6.))
                .flex()
                .flex_col()
                .gap(px(3.))
                .when_some(label, |d, l| d.child(div().text_size(px(11.5)).text_color(p.fg_dim).child(l)))
                .child(
                    div()
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(5.))
                        .bg(p.wash(p.fg_dim))
                        .font_family(mono.clone())
                        .text_size(px(11.5))
                        .child(line),
                )
        });
        div()
            .id("console-not-running")
            .debug_selector(|| "console-missing".into())
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .min_h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(28.))
                    .px(px(24.))
                    .py(px(12.))
                    .when(self.stacked(), |d| d.flex_col().gap(px(16.)))
                    .child(div().flex_none().child(crate::art::drawing_on(
                        snout_core::art::spots::agent_console(),
                        156.,
                        104.,
                        p.panel,
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .max_w(px(420.))
                            .min_w_0()
                            .when(self.stacked(), |d| d.w_full())
                            .child(div().text_size(px(14.)).font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                            .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(p.fg_dim).child(hint))
                            .children(lines)
                            .child(div().mt(px(10.)).flex().flex_wrap().gap(px(8.)).children(buttons)),
                    ),
            )
            .into_any_element()
    }

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
        let install = self.sessions.get(root).and_then(|s| s.install);
        let install_in = self.sessions.get(root).and_then(|s| s.install_in.clone());
        let (ended, again) = match (self.kind, install) {
            (_, Some(_)) => ("The installer".to_owned(), "Try again"),
            (Kind::Claude, None) => (self.session_agent(root, cx).name.to_string(), "Start a new session"),
            (Kind::Shell, None) => (self.kind.title().to_owned(), "Start a new session"),
        };
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
                        .flex_wrap()
                        .gap(px(6.))
                        .px(px(8.))
                        .text_color(p.fg_dim)
                        .child(format!("{ended} {how}."))
                        .child(
                            Button::new("again")
                                .link()
                                .icon(crate::components::menu_icon(crate::components::IconName::Play))
                                .label(again)
                                .on_click(cx.listener(move |this, _, window, cx| match install {
                                    Some(id) => this.install(&start, id, install_in.clone(), window, cx),
                                    None => this.start(&start, &[], window, cx),
                                })),
                        ),
                )
            })
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, w, cx| this.on_key(e, &r1, w, cx)))
            .on_action(cx.listener(move |this, _: &Paste, _, cx| this.paste_clipboard(&r2, cx)))
            .on_action(cx.listener(move |this, _: &Copy, _, cx| {
                this.copy_selection(&r3, cx);
            }))
            // What the right-click menu is about: the link under the pointer, if one is.
            .on_mouse_down(MouseButton::Right, {
                let root = root.to_owned();
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.menu_link = this
                        .cell_at(e.position)
                        .and_then(|(row, col)| this.sessions.get(&root).and_then(|s| s.pty.frame().link_at(row, col)));
                })
            })
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
                    // ⌘-click (Ctrl+click on Linux) opens the link under the pointer.
                    if e.modifiers.secondary(cx)
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
        let panel = cx.entity();
        let menu_root = root.to_owned();
        let grid = grid.context_menu(move |menu, _, cx| {
            use crate::components::{IconName, menu_icon};
            let this = panel.read(cx);
            let link = this.menu_link.clone();
            let has_selection = this.selected_text(&menu_root).is_some();
            let menu = match link {
                Some(link) => {
                    let copy = link.clone();
                    menu.item(
                        PopupMenuItem::new("Open Link")
                            .icon(menu_icon(IconName::Globe))
                            .on_click(move |_, _, cx| cx.open_url(&link)),
                    )
                    .item(
                        PopupMenuItem::new("Copy Link")
                            .icon(menu_icon(IconName::Copy))
                            .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))),
                    )
                    .separator()
                }
                None => menu,
            };
            let (copy_panel, copy_root) = (panel.clone(), menu_root.clone());
            let (paste_panel, paste_root) = (panel.clone(), menu_root.clone());
            menu.item(PopupMenuItem::new("Copy").icon(menu_icon(IconName::Copy)).disabled(!has_selection).on_click(
                move |_, _, cx| {
                    copy_panel.update(cx, |this, cx| {
                        this.copy_selection(&copy_root, cx);
                    })
                },
            ))
            .item(
                PopupMenuItem::new("Paste")
                    .on_click(move |_, _, cx| paste_panel.update(cx, |this, cx| this.paste_clipboard(&paste_root, cx))),
            )
        });
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
        if s.pty.resize(grid_size(inner, size(cell_w, cell_h))) {
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

/// The Terminal tab's panel.
pub fn terminal_of(ws: &crate::workspace::Workspace, cx: &App) -> Option<Entity<ConsolePanel>> {
    let view = ws.dock.read(cx).panel(ws.tabs.terminal)?.clone();
    view.view().downcast::<ConsolePanel>().ok()
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
        crate::agents::OPENCODE => vec!["--continue".into()],
        crate::agents::AMP => vec!["threads".into(), "continue".into(), "--last".into()],
        _ => Vec::new(),
    }
}

/// The Install button's words for the agent `id`.
fn install_label(id: &str) -> &'static str {
    match id {
        crate::agents::CLAUDE => "Install Claude Code",
        crate::agents::CODEX => "Install Codex",
        crate::agents::OPENCODE => "Install opencode",
        crate::agents::AMP => "Install Amp",
        _ => "Install",
    }
}

/// The shell the Terminal tab starts when Settings names none: the user's `SHELL`; on Windows
/// PowerShell (7 where it is installed), else the Command Prompt; on a Mac zsh; else bash, or
/// `sh` where there is no bash. `SHELL` is often unset where nobody logged in: in WSL reached
/// through `wsl.exe`, in a CI job, in a container.
pub fn default_shell(host: &dyn snout_host::Host) -> PathBuf {
    let info = host.info();
    if let Some(shell) = info.var("SHELL").filter(|s| !s.trim().is_empty()) {
        return PathBuf::from(shell);
    }
    if info.is_windows() {
        return snout_host::which(host, &["pwsh", "powershell"], &[])
            .or_else(|| info.var("ComSpec").or_else(|| info.var("COMSPEC")).map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("cmd.exe"));
    }
    if info.is_mac() {
        return PathBuf::from("/bin/zsh");
    }
    ["/bin/bash", "/usr/bin/bash"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| host.exists(p))
        .unwrap_or_else(|| PathBuf::from("/bin/sh"))
}

/// The name of [`default_shell`], without looking for it.
fn default_shell_name(info: &snout_host::HostInfo) -> String {
    match info.var("SHELL") {
        Some(shell) => shell.to_owned(),
        None if info.is_windows() => "powershell".into(),
        None if info.is_mac() => "zsh".into(),
        None => "bash".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_copies_only_on_windows_and_only_as_ctrl_c() {
        assert!(ctrl_c_copies(true, true, "c"));
        assert!(!ctrl_c_copies(false, true, "c"), "elsewhere it is always the interrupt");
        assert!(!ctrl_c_copies(true, false, "c"), "Ctrl+Shift+C has its own binding");
        assert!(!ctrl_c_copies(true, true, "v"));
    }

    #[test]
    fn an_agent_that_never_goes_quiet_still_counts_as_started() {
        assert!(!super::settled(false, Duration::from_secs(1)), "still starting");
        assert!(super::settled(true, Duration::from_secs(1)), "quiet says so");
        assert!(super::settled(false, super::STARTED_AFTER), "Amp's welcome screen animates");
    }

    #[test]
    fn a_menu_or_a_question_of_trust_on_the_screen_keeps_a_prompt_back() {
        for screen in [
            " Do you trust the files in this folder?\n ❯ 1. Yes, proceed\n   2. No, exit\n",
            "│ Do you want to run this command?\n│ ❯ 1. Yes\n│   2. No\n",
            "› 1. Yes, proceed\n  2. No\n",
            "Do you trust the contents of this directory?\n",
        ] {
            assert!(asks_the_user(screen), "{screen}");
        }
        for screen in [
            "❯ now fix the test\n",
            "❯ \n  ? for shortcuts\n",
            "Claude Code\n> Try \"fix the test\"\n",
            "We trust you have read the docs.\n",
        ] {
            assert!(!asks_the_user(screen), "{screen}");
        }
    }
}
