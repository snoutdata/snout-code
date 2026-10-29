//! The Claude Code console in the bottom dock: `claude` running in the open repository on
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

use crate::art::glyph;
use crate::look;
use crate::model::{Action, Model};
use crate::theme::{Palette, mix, palette};
use snout_core::art::Glyph;

/// Refresh the Git views once the agent has been quiet this long...
const QUIET: Duration = Duration::from_millis(800);
/// ...and at most this often while it keeps printing.
const BUSY_REFRESH: Duration = Duration::from_secs(3);
const FONT_SIZE: f32 = 13.0;

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
}

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
}

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
        self.kind.title()
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
        }
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

    /// Start the console's program in `root`, on the workspace's host: found and started off
    /// the UI thread, the session shows once it runs.
    pub fn start(&mut self, root: &Path, args: &[String], window: &mut Window, cx: &mut Context<Self>) {
        self.sessions.remove(root);
        self.note_engine(root, cx);
        self.error = None;
        let host = crate::host::host(cx);
        let kind = self.kind;
        let configured = self.model.read(cx).settings.claude_path.clone();
        let engine = self.model.read(cx).settings.terminal_engine;
        let program = self.program.clone();
        let shell_args = ["-l".to_owned()];
        let args = if kind == Kind::Shell && args.is_empty() { &shell_args[..] } else { args }.to_vec();
        let p = palette(cx);
        let (fg, bg) = (rgb(p.fg), rgb(p.panel));
        // Claude Code connects to this window's IDE server, and its hooks report its edits.
        let env = match kind {
            Kind::Claude => cx.try_global::<crate::claude_ide::IdeEnv>().map(|e| e.0.clone()).unwrap_or_default(),
            Kind::Shell => Vec::new(),
        };
        let mut args = args;
        if kind == Kind::Claude
            && let Some(extra) = cx.try_global::<crate::claude_ide::IdeArgs>()
        {
            args.splice(0..0, extra.0.iter().cloned());
        }
        let root = root.to_owned();
        self.starting.insert(root.clone());
        let started = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                let found = match kind {
                    Kind::Claude => configured.or_else(|| find_claude(&*host)),
                    Kind::Shell => {
                        Some(host.info().var("SHELL").map_or_else(|| PathBuf::from("/bin/zsh"), PathBuf::from))
                    }
                };
                let program = program.or(found).ok_or(None)?;
                let env_remove: &[&str] = if kind == Kind::Claude { ENV_REMOVE } else { &[] };
                let launch = Launch { program: &program, args: &args, cwd: &root, env: &env, env_remove };
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
                        this.sessions.insert(root.clone(), Session { pty, seen_gen: 0, refreshed: Instant::now() });
                        this.note_engine(&root, cx);
                        window.focus(&this.focus, cx);
                    }
                    Err(None) => {
                        this.error = Some(
                            "Claude Code was not found on PATH or in ~/.local/bin, ~/.claude/local, /opt/homebrew/bin, \
                             /usr/local/bin.\nInstall it from https://docs.claude.com/en/docs/claude-code and press Start."
                                .into(),
                        )
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
        let Some(s) = self.sessions.get(root) else { return };
        let mods = pty::Mods { shift: m.shift, alt: m.alt, ctrl: m.control, cmd: m.platform };
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
        let input = KeyInput { key, mods, text: k.key_char.as_deref(), base };
        if s.pty.key(&input) {
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
        let connected =
            kind == Kind::Claude && cx.try_global::<crate::claude_ide::IdeStatus>().is_some_and(|s| s.clients > 0);
        let follow = self.model.read(cx).settings.follow_claude;
        let header = |title: String, running: Option<bool>, buttons: Vec<gpui::AnyElement>| {
            let title = title.trim().trim_start_matches(['✳', '✻', '·', '✶', '✢', '*', ' ']).to_owned();
            div()
                .flex_none()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .when(kind == Kind::Claude, |d| d.child(glyph(Glyph::Spark, 16., p.fg)))
                        .child(div().whitespace_nowrap().child(kind.title()))
                        .when_some(running, |d, on| {
                            d.child(div().size(px(7.)).rounded(px(4.)).bg(if on { p.ok } else { p.fg_dim }))
                        })
                        .when(connected, |d| {
                            d.child(
                                div()
                                    .id("claude-connected")
                                    .debug_selector(|| "claude-connected".into())
                                    .flex_none()
                                    .whitespace_nowrap()
                                    .px(px(7.))
                                    .h(px(18.))
                                    .flex()
                                    .items_center()
                                    .rounded(px(9.))
                                    .bg(p.wash(p.ok))
                                    .text_color(p.ok)
                                    .text_size(px(11.5))
                                    .tooltip(|w, cx| {
                                        gpui_component::tooltip::Tooltip::new(
                                            "Claude Code sees the editor: its selection, open files and problems, \
                                             and proposes changes there for you to accept or reject.",
                                        )
                                        .build(w, cx)
                                    })
                                    .child("Claude connected"),
                            )
                        })
                        .when(!title.is_empty() && title != kind.title(), |d| {
                            d.child(
                                div().min_w_0().text_color(p.fg_dim).overflow_hidden().whitespace_nowrap().child(title),
                            )
                        })
                        .child(div().flex_1())
                        .child(div().flex().items_center().gap(px(2.)).children(buttons)),
                )
                .child(div().h(px(4.)))
                .child(look::separator(&p))
        };
        let hide = look::icon_button("hide", Glyph::Close, "Hide (⌘J); the session keeps running", true, &p)
            .on_click(cx.listener(|this, _, _, cx| this.model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx))))
            .into_any_element();
        let Some(root) = root else {
            return div()
                .size_full()
                .bg(p.panel)
                .flex()
                .flex_col()
                .child(header(String::new(), None, vec![hide]))
                .child(
                    crate::components::EmptyState::new(snout_core::art::spots::no_project(), "No repository open")
                        .hint(format!("Open a repository to run {} in it.", kind.title()))
                        .on(p.panel)
                        .compact(),
                );
        };
        let wanted = kind == Kind::Shell || self.model.read(cx).settings.console_open;
        if wanted && !self.sessions.contains_key(&root) && !self.starting.contains(&root) && self.error.is_none() {
            self.start(&root, &[], window, cx);
        }
        let (title, exit) = match self.sessions.get_mut(&root) {
            Some(s) => (s.pty.title(), s.pty.exit_status().map(str::to_owned)),
            None => (String::new(), None),
        };
        let running = self.sessions.contains_key(&root) && exit.is_none();
        let restart =
            |id: &'static str, g: Glyph, label: &'static str, tip: &'static str, args: Vec<String>, root: PathBuf| {
                look::chip_button(id, g, label, true, &p)
                    .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip).build(w, cx))
                    .on_click(cx.listener(move |this, _, window, cx| this.start(&root, &args, window, cx)))
                    .into_any_element()
            };
        let follow_button = look::toggle("follow", "Follow", follow, &p)
            .h(px(22.))
            .tooltip(move |w, cx| {
                let tip = if follow {
                    "Following: the editor opens the files Claude Code changes and lights the change"
                } else {
                    "Not following: a dot on a tab marks a file Claude Code changed"
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
            .into_any_element();
        let mut buttons = match kind {
            Kind::Claude => vec![
                follow_button,
                restart("new", Glyph::Plus, "New", "Restart with a new conversation", Vec::new(), root.clone()),
                restart(
                    "continue",
                    Glyph::Refresh,
                    "Continue",
                    "Restart with the most recent conversation in this folder (claude --continue)",
                    vec!["--continue".into()],
                    root.clone(),
                ),
            ],
            Kind::Shell => vec![restart("new", Glyph::Plus, "New", "Start a new shell", Vec::new(), root.clone())],
        };
        if running {
            let r = root.clone();
            buttons.push(
                look::chip_button("end", Glyph::Stop, "End", true, &p)
                    .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("End this session").build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(mut s) = this.sessions.remove(&r) {
                            s.pty.kill();
                        }
                        this.note_engine(&r, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if kind == Kind::Claude {
            buttons.push(div().w(px(4.)).into_any_element());
            buttons.push(hide);
        }

        let body = if self.sessions.contains_key(&root) {
            self.terminal(&root, exit, window, cx).into_any_element()
        } else {
            let r = root.clone();
            div()
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(self.error.clone().unwrap_or_else(|| "Not running".into()))
                .child(look::small_button("start", "Start", &p).on_click(cx.listener(move |this, _, window, cx| {
                    this.error = None;
                    this.start(&r, &[], window, cx)
                })))
                .into_any_element()
        };
        div().size_full().bg(p.panel).flex().flex_col().child(header(title, Some(running), buttons)).child(body)
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
        let mono: SharedString = look::MONO.into();
        let (fg0, bg0) = (rgb(p.fg), rgb(p.panel));
        if let Some(s) = self.sessions.get(root) {
            s.pty.set_colors(fg0, bg0);
        }
        let view = cx.entity();
        let selection = self.selection;
        let focused = self.focus.is_focused(_window);
        let (r1, r2, r3) = (root.to_owned(), root.to_owned(), root.to_owned());
        let start = root.to_owned();
        let find_bar = self.find_bar(&p, cx);
        let grid = div()
            .id("terminal")
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
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.selecting = false))
            .child(
                canvas(|_, _, _| {}, {
                    let root = root.to_owned();
                    move |bounds, _, window, cx| {
                        view.update(cx, |this, cx| {
                            this.paint(&root, bounds, &mono, focused, selection, &p, window, cx)
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
        family: &SharedString,
        focused: bool,
        selection: Option<((u16, u16), (u16, u16))>,
        p: &Palette,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(s) = self.sessions.get_mut(root) else { return };
        let font: Font = font(family.clone());
        let font_size = px(FONT_SIZE);
        let ts = window.text_system().clone();
        let id = ts.resolve_font(&font);
        let cell_w = ts.advance(id, font_size, 'M').map(|s| s.width).unwrap_or(px(8.));
        let cell_h = (font_size * 1.3).round();
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
        let (fg0, bg0) = (rgb(p.fg), rgb(p.panel));
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
                let run = cell_run(&frame, c, &font, (fg0, bg0), p);
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
        if let Some(cur) = frame.cursor {
            let at = point(inner.left() + cell_w * f32::from(cur.col), inner.top() + cell_h * f32::from(cur.row));
            let rect = match cur.shape {
                CursorShape::Block => Bounds::new(at, size(cell_w, cell_h)),
                CursorShape::Bar => Bounds::new(at, size(px(2.), cell_h)),
                CursorShape::Underline => Bounds::new(point(at.x, at.y + cell_h - px(2.)), size(cell_w, px(2.))),
            };
            if focused || cur.shape != CursorShape::Block {
                window.paint_quad(gpui::fill(rect, p.fg.opacity(0.7)));
            } else {
                window.paint_quad(gpui::outline(rect, p.fg.opacity(0.6), gpui::BorderStyle::Solid));
            }
        }
    }
}

/// A cell as text to shape: its glyph (a wide one kept to one cell so the grid stays
/// aligned, a space filling the other), its own background when it has one, and its look.
fn cell_run(
    frame: &Frame,
    c: &snout_core::term::Cell,
    font: &Font,
    (fg0, bg0): (Rgb, Rgb),
    p: &Palette,
) -> (String, Option<Hsla>, TextRun) {
    let mut fg = pty::color(c.fg, fg0, c.bold());
    let mut bg = pty::color(c.bg, bg0, false);
    if c.attrs.contains(Attrs::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
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
        other => hsla(pty::color(other, fg0, false)),
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
