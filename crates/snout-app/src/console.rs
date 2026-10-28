//! The Claude Code console in the bottom dock: `claude` running in the open repository on
//! the pseudo-terminal of `snout_core::pty`, painted cell by cell. One session per
//! repository, kept alive while switching between them. When the agent has printed and
//! gone quiet, the Git views are re-read, since it may have changed files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle, Font, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement as _, Pixels, Point, Render, ScrollEvent, SharedString, StatefulInteractiveElement as _, Styled as _,
    TextAlign, TextRun, UnderlineStyle, Window, canvas, div, font, point, px, size,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dock::{BasePanelView, panel_handle};
use gpui_component::input::{Copy, Paste};
use snout_core::claude::{ENV_REMOVE, find_claude};
use snout_core::pty::{self, Launch, Pty, Rgb};

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
            focus: cx.focus_handle(),
        }
    }

    fn root(&self, cx: &App) -> Option<PathBuf> {
        let m = self.model.read(cx);
        let on = self.kind == Kind::Shell || m.settings.console_open;
        on.then(|| m.repo.as_ref().map(|r| r.root().to_owned())).flatten()
    }

    /// Redraw on new output; re-read the repository once the agent goes quiet.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let Some(s) = self.sessions.get_mut(&root) else { return };
        let generation = s.pty.output_gen();
        if generation != self.shown_gen {
            self.shown_gen = generation;
            cx.notify();
        }
        let quiet = s.pty.last_output().is_some_and(|t| t.elapsed() >= QUIET);
        if generation != s.seen_gen && (quiet || s.refreshed.elapsed() >= BUSY_REFRESH) {
            s.seen_gen = generation;
            s.refreshed = Instant::now();
            self.model.update(cx, |m, cx| {
                if m.job.is_none() {
                    m.refresh();
                    cx.notify();
                }
            });
        }
    }

    pub fn start(&mut self, root: &Path, args: &[String], window: &mut Window, cx: &mut Context<Self>) {
        self.sessions.remove(root);
        self.error = None;
        let found = match self.kind {
            Kind::Claude => self.model.read(cx).settings.claude_path.clone().or_else(find_claude),
            Kind::Shell => Some(std::env::var_os("SHELL").map_or_else(|| PathBuf::from("/bin/zsh"), PathBuf::from)),
        };
        let shell_args = ["-l".to_owned()];
        let args = if self.kind == Kind::Shell && args.is_empty() { &shell_args[..] } else { args };
        let Some(program) = self.program.clone().or(found) else {
            self.error = Some(
                "Claude Code was not found on PATH or in ~/.local/bin, ~/.claude/local, /opt/homebrew/bin, \
                 /usr/local/bin.\nInstall it from https://docs.claude.com/en/docs/claude-code and press Start."
                    .into(),
            );
            return;
        };
        let p = palette(cx);
        let env_remove: &[&str] = if self.kind == Kind::Claude { ENV_REMOVE } else { &[] };
        let launch = Launch { program: &program, args, cwd: root, env: &[], env_remove };
        match Pty::spawn(&launch, rgb(p.fg), rgb(p.panel), || {}) {
            Ok(pty) => {
                self.sessions.insert(root.to_owned(), Session { pty, seen_gen: 0, refreshed: Instant::now() });
                window.focus(&self.focus, cx);
            }
            Err(e) => self.error = Some(format!("Could not start {}: {e}", program.display())),
        }
        cx.notify();
    }

    pub fn session_text(&self, root: &Path) -> Option<String> {
        self.sessions.get(root).map(|s| s.pty.contents())
    }

    fn cell_at(&self, pos: Point<Pixels>) -> Option<(u16, u16)> {
        let (b, cell) = self.grid?;
        let x = ((pos.x - b.left()) / cell.width).max(0.0) as u16;
        let y = ((pos.y - b.top()) / cell.height).max(0.0) as u16;
        Some((y, x))
    }

    fn on_key(&mut self, e: &KeyDownEvent, root: &Path, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.get(root) else { return };
        let k = &e.keystroke;
        let m = k.modifiers;
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
        let app_cursor = s.pty.parser().screen().application_cursor();
        let bytes = match key {
            Some(key) => pty::encode_key(key, mods, app_cursor),
            None if !m.platform && !m.control => match &k.key_char {
                Some(text) if m.alt => [b"\x1b".as_slice(), text.as_bytes()].concat(),
                Some(text) => text.clone().into_bytes(),
                None => Vec::new(),
            },
            None => Vec::new(),
        };
        if !bytes.is_empty() {
            s.pty.send(&bytes);
            self.selection = None;
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Render for ConsolePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let root = self.root(cx).or_else(|| self.model.read(cx).repo.as_ref().map(|r| r.root().to_owned()));
        let kind = self.kind;
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
                .child(look::centered_note(format!("Open a repository to run {} in it", kind.title()), &p));
        };
        let wanted = kind == Kind::Shell || self.model.read(cx).settings.console_open;
        if wanted && !self.sessions.contains_key(&root) && self.error.is_none() {
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
        let mut buttons = match kind {
            Kind::Claude => vec![
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
        div()
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
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| this.on_key(e, &r1, cx)))
            .on_action(cx.listener(move |this, _: &Paste, _, cx| {
                let text = cx.read_from_clipboard().and_then(|c| c.text());
                if let (Some(text), Some(s)) = (text, this.sessions.get(&r2)) {
                    let bracketed = s.pty.parser().screen().bracketed_paste();
                    s.pty.send(&pty::paste_bytes(&text, bracketed));
                }
            }))
            .on_action(cx.listener(move |this, _: &Copy, _, cx| {
                let text = this
                    .selection
                    .zip(this.sessions.get(&r3))
                    .and_then(|(sel, s)| pty::selected_text(s.pty.parser().screen(), sel));
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.selection = this.cell_at(e.position).map(|c| (c, c));
                    this.selecting = true;
                    cx.notify();
                }),
            )
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
            )
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
        let parser = s.pty.parser();
        let screen = parser.screen();
        let (fg0, bg0) = (rgb(p.fg), rgb(p.panel));
        let sel = selection.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });
        let selected = |r: u16, c: u16| sel.is_some_and(|(s, e)| (r, c) >= s && (r, c) <= e && s != e);
        let (rows, cols) = screen.size();
        for row in 0..rows {
            let mut text = String::new();
            let mut runs: Vec<TextRun> = Vec::new();
            for col in 0..cols {
                let Some(c) = screen.cell(row, col) else { continue };
                if c.is_wide_continuation() {
                    continue;
                }
                let mut fg = pty::color(c.fgcolor(), fg0, c.bold());
                let mut bg = pty::color(c.bgcolor(), bg0, false);
                if c.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let mut fg = hsla(fg);
                if c.dim() {
                    fg = mix(fg, 0.55, hsla(bg));
                }
                let bg = if selected(row, col) { Some(p.selection()) } else { (bg != bg0).then(|| hsla(bg)) };
                let contents = c.contents();
                let mut glyph = if contents.is_empty() { " ".to_owned() } else { contents.to_owned() };
                if c.is_wide() {
                    // Kept to one cell so the grid stays aligned; the space fills the other.
                    glyph.push(' ');
                }
                let mut f = font.clone();
                if c.bold() {
                    f.weight = FontWeight::BOLD;
                }
                let underline =
                    c.underline().then(|| UnderlineStyle { thickness: px(1.), color: Some(fg), wavy: false });
                runs.push(TextRun {
                    len: glyph.len(),
                    font: f,
                    color: fg,
                    background_color: bg,
                    underline,
                    strikethrough: None,
                });
                text.push_str(&glyph);
            }
            let line = ts.shape_line(text.into(), font_size, &runs, Some(cell_w));
            let origin = point(inner.left(), inner.top() + cell_h * f32::from(row));
            let _ = line.paint_background(origin, cell_h, TextAlign::Left, None, window, cx);
            let _ = line.paint(origin, cell_h, TextAlign::Left, None, window, cx);
        }
        if screen.scrollback() == 0 && !screen.hide_cursor() {
            let (row, col) = screen.cursor_position();
            let at = point(inner.left() + cell_w * f32::from(col), inner.top() + cell_h * f32::from(row));
            let rect = Bounds::new(at, size(cell_w, cell_h));
            if focused {
                window.paint_quad(gpui::fill(rect, p.fg.opacity(0.7)));
            } else {
                window.paint_quad(gpui::outline(rect, p.fg.opacity(0.6), gpui::BorderStyle::Solid));
            }
        }
    }
}

/// The docked console, if the workspace has one.
pub fn console_of(ws: &crate::workspace::Workspace, cx: &App) -> Option<Entity<ConsolePanel>> {
    let id = ws.tabs.console?;
    let view = ws.dock.read(cx).panel(id)?.clone();
    view.view().downcast::<ConsolePanel>().ok()
}
