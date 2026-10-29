//! Go to file (⌘P) and find in files (⇧⌘F) in the open repository: a box over the window
//! with a field and what matches. The host lists the files (so `.gitignore` holds) and
//! searches their text; names are matched fuzzily (`fuzzy-matcher`).
//! ↑ ↓ pick, ↩ opens the file (at the line), Esc closes.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fuzzy_matcher::FuzzyMatcher as _;
use fuzzy_matcher::skim::SkimMatcherV2;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Task, Window, actions, div,
    px, uniform_list,
};
use gpui_component::input::{InputEvent, InputState};
use snout_host::{Host, HostRef, WalkOptions};

use crate::look;
use crate::model::{Action, Model};
use crate::theme::palette;

actions!(search, [SearchUp, SearchDown, SearchClose]);

/// The most rows either search shows.
pub const LIMIT: usize = 200;

/// The files of `root` on `host` a search looks through, relative to it, sorted.
pub fn list_files(host: &dyn Host, root: &Path) -> Vec<String> {
    let options = WalkOptions {
        hidden: true,
        gitignore: true,
        skip: vec![".git".into()],
        files_only: true,
        ..Default::default()
    };
    let mut out: Vec<String> = host
        .walk(root, &options)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| e.path.strip_prefix(root).ok().map(|p| p.to_string_lossy().into_owned()))
        .collect();
    out.sort();
    out
}

/// The files whose path matches `query` fuzzily, best first, with the matched characters.
pub fn go_to_file(files: &[String], query: &str) -> Vec<(String, Vec<usize>)> {
    if query.trim().is_empty() {
        return files.iter().take(LIMIT).map(|f| (f.clone(), Vec::new())).collect();
    }
    let matcher = SkimMatcherV2::default().smart_case();
    let mut hits: Vec<(i64, &String, Vec<usize>)> = files
        .iter()
        .filter_map(|f| matcher.fuzzy_indices(f, query.trim()).map(|(score, idx)| (score, f, idx)))
        .collect();
    // Best score first; among equals, the shorter path.
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
    hits.into_iter().take(LIMIT).map(|(_, f, idx)| (f.clone(), idx)).collect()
}

/// A line that holds what find in files looked for.
pub type Hit = snout_host::TextHit;

/// Every line of the files of `root` on `host` holding `text` (a plain string; case matters
/// only when it has capitals), up to [`LIMIT`].
pub fn find_in_files(host: &dyn Host, root: &Path, files: &[String], text: &str) -> Vec<Hit> {
    host.find_text(root, files, text, LIMIT).unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    GoToFile,
    FindInFiles,
    /// Go to a line (and column) of the editor in front: `12` or `12:5`.
    GoToLine,
    /// The command palette: every command, with its shortcut.
    Commands,
    /// Symbols, from a language server: the workspace's (⌘T) or the file's (⇧⌘O).
    Symbols,
    /// The file's symbols as a tree, the one the caret is in picked (File Structure, ⌘F12).
    Structure,
    /// Classes of the project, its libraries and the JDK (Go to Class, ⌥⌘O).
    Classes,
}

/// Where the symbol box gets its rows: the query in, the matching symbols out.
pub type SymbolSource = std::rc::Rc<dyn Fn(String, &mut gpui::App) -> Task<Vec<Item>>>;

/// A command the palette offers: its name, what it does, and its shortcut (if any).
pub struct Command {
    pub name: SharedString,
    pub action: Box<dyn gpui::Action>,
    pub keys: Option<String>,
}

/// The commands whose name matches `query` fuzzily, best first (all of them, in order,
/// for an empty query).
pub fn match_commands(commands: &[Command], query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..commands.len()).collect();
    }
    let matcher = SkimMatcherV2::default().ignore_case();
    let mut hits: Vec<(i64, usize)> = commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| matcher.fuzzy_match(&c.name, query.trim()).map(|s| (s, i)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, i)| i).collect()
}

/// The line and column (from 0) that `12` or `12:5` (from 1) names.
pub fn parse_line(text: &str) -> Option<(u32, u32)> {
    let mut parts = text.trim().splitn(2, [':', ',']);
    let line: u32 = parts.next()?.trim().parse().ok()?;
    let column: u32 = match parts.next() {
        Some(c) => c.trim().parse().ok()?,
        None => 1,
    };
    Some((line.max(1) - 1, column.max(1) - 1))
}

/// A row the box offers.
#[derive(Clone, Debug, Default)]
pub struct Item {
    pub path: String,
    pub at: Option<(u32, u32)>,
    pub detail: Option<String>,
    /// For a command: its index in the palette's list, and its shortcut.
    pub command: Option<usize>,
    pub keys: Option<String>,
    /// For a symbol: its build, document and position.
    pub target: Option<(PathBuf, lsp_types::Uri, lsp_types::Position)>,
    /// For a symbol of the file's tree: its kind and visibility, how deep it is, and whether
    /// the caret is in it.
    pub symbol: Option<(lsp_types::SymbolKind, Option<crate::lang::symbols::Visibility>)>,
    pub depth: usize,
    pub current: bool,
}

pub enum SearchEvent {
    Close,
    /// Go to a symbol: its build, document and position.
    OpenLocation(PathBuf, lsp_types::Uri, lsp_types::Position),
    GoToLine(u32, u32),
    /// Run a command from the palette.
    Run(Box<dyn gpui::Action>),
}

pub struct SearchView {
    model: Entity<Model>,
    root: PathBuf,
    pub mode: Mode,
    pub input: Entity<InputState>,
    files: Arc<Vec<String>>,
    host: HostRef,
    /// Listing the files, off the UI thread.
    _listing: Option<Task<()>>,
    pub items: Vec<Item>,
    pub selected: usize,
    /// The palette's commands.
    pub commands: Vec<Command>,
    /// The symbols' source, in the symbol box.
    pub symbols: Option<SymbolSource>,
    /// The text search running, if any.
    _search: Option<Task<()>>,
    /// The field has been given the keyboard (on the first frame).
    focused: bool,
    focus: FocusHandle,
}

impl EventEmitter<SearchEvent> for SearchView {}

impl SearchView {
    pub fn new(model: Entity<Model>, root: PathBuf, mode: Mode, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = match mode {
            Mode::GoToFile => "Go to file",
            Mode::FindInFiles => "Find in files",
            Mode::GoToLine => "Line, or line:column",
            Mode::Commands => "Type a command",
            Mode::Symbols => "Go to symbol",
            Mode::Structure => "Filter the file's symbols",
            Mode::Classes => "Go to class, in the project, its libraries and the JDK",
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        cx.subscribe(&input, |this, _, e: &InputEvent, cx| match e {
            InputEvent::Change => this.update(cx),
            InputEvent::PressEnter { .. } => this.open_selected(cx),
            _ => {}
        })
        .detach();
        let host = crate::host::host(cx);
        let listing =
            (!matches!(mode, Mode::GoToLine | Mode::Commands | Mode::Symbols | Mode::Structure | Mode::Classes)).then(
                || {
                    let list = cx.background_executor().spawn({
                        let (host, root) = (host.clone(), root.clone());
                        async move { list_files(&*host, &root) }
                    });
                    cx.spawn(async move |this, cx| {
                        let files = list.await;
                        let _ = this.update(cx, |this, cx| {
                            this.files = Arc::new(files);
                            this.update(cx);
                        });
                    })
                },
            );
        let mut this = Self {
            model,
            root,
            mode,
            input,
            files: Arc::new(Vec::new()),
            host,
            _listing: listing,
            items: Vec::new(),
            selected: 0,
            commands: Vec::new(),
            symbols: None,
            _search: None,
            focused: false,
            focus: cx.focus_handle(),
        };
        this.update(cx);
        this
    }

    pub fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |s, cx| s.focus(window, cx));
    }

    /// Take symbols from `source` (the symbol box).
    pub fn set_symbols(&mut self, source: SymbolSource, cx: &mut Context<Self>) {
        self.symbols = Some(source);
        self.update(cx);
    }

    /// Offer `commands` (the palette).
    pub fn set_commands(&mut self, commands: Vec<Command>, cx: &mut Context<Self>) {
        self.commands = commands;
        self.update(cx);
    }

    /// Put `text` in the field and search for it.
    pub fn set_query(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |s, cx| s.set_value(text.to_owned(), window, cx));
        self.update(cx);
    }

    /// Search again for what the field says.
    fn update(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        self.selected = 0;
        match self.mode {
            Mode::GoToLine => cx.notify(),
            Mode::Commands => {
                self.items = match_commands(&self.commands, &query)
                    .into_iter()
                    .map(|i| Item {
                        path: self.commands[i].name.to_string(),
                        at: None,
                        detail: None,
                        command: Some(i),
                        keys: self.commands[i].keys.clone(),
                        ..Default::default()
                    })
                    .collect();
                cx.notify();
            }
            Mode::Symbols | Mode::Structure | Mode::Classes => {
                let Some(source) = self.symbols.clone() else { return };
                self._search = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(120)).await;
                    let Ok(task) = this.update(cx, |_, cx| source(query, cx)) else { return };
                    let items = task.await;
                    let _ = this.update(cx, |this, cx| {
                        // The file's tree opens on the symbol the caret is in.
                        this.selected = items.iter().position(|i| i.current).unwrap_or(0);
                        this.items = items;
                        cx.notify();
                    });
                }));
            }
            Mode::GoToFile => {
                self.items = go_to_file(&self.files, &query)
                    .into_iter()
                    .map(|(path, _)| Item { path, ..Default::default() })
                    .collect();
                cx.notify();
                // Search Everywhere: the symbols that match follow the files.
                if let Some(source) = self.symbols.clone().filter(|_| !query.trim().is_empty()) {
                    self._search = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_millis(120)).await;
                        let Ok(task) = this.update(cx, |_, cx| source(query, cx)) else { return };
                        let symbols = task.await;
                        let _ = this.update(cx, |this, cx| {
                            this.items.retain(|i| i.target.is_none());
                            this.items.extend(symbols.into_iter().take(50));
                            cx.notify();
                        });
                    }));
                }
            }
            Mode::FindInFiles => {
                let (root, files, host) = (self.root.clone(), self.files.clone(), self.host.clone());
                self._search = Some(cx.spawn(async move |this, cx| {
                    // Wait for the typing to pause.
                    cx.background_executor().timer(Duration::from_millis(120)).await;
                    let hits = cx
                        .background_executor()
                        .spawn(async move { find_in_files(&*host, &root, &files, &query) })
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        this.items = hits
                            .into_iter()
                            .map(|h| Item {
                                detail: Some(format!("{}: {}", h.line + 1, h.text.trim())),
                                at: Some((h.line, h.column)),
                                path: h.path,
                                ..Default::default()
                            })
                            .collect();
                        this.selected = 0;
                        cx.notify();
                    });
                }));
            }
        }
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        self.selected = (self.selected as isize + by).rem_euclid(n) as usize;
        cx.notify();
    }

    pub fn open_selected(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::GoToLine {
            if let Some((line, column)) = parse_line(&self.input.read(cx).value()) {
                cx.emit(SearchEvent::GoToLine(line, column));
            }
            return;
        }
        let Some(item) = self.items.get(self.selected).cloned() else { return };
        if let Some((root, uri, at)) = item.target {
            cx.emit(SearchEvent::OpenLocation(root, uri, at));
            return;
        }
        if let Some(i) = item.command {
            let action = self.commands[i].action.boxed_clone();
            cx.emit(SearchEvent::Run(action));
            return;
        }
        let action = Action::OpenFile(self.root.join(&item.path), item.at);
        cx.emit(SearchEvent::Close);
        self.model.update(cx, |m, cx| m.handle(action, cx));
    }
}

impl Render for SearchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !std::mem::replace(&mut self.focused, true) {
            self.focus_input(window, cx);
        }
        let p = palette(cx);
        let items = self.items.clone();
        let selected = self.selected;
        let view = cx.entity();
        let count = items.len();
        let empty: SharedString = match self.mode {
            Mode::GoToFile => "No file matches".into(),
            Mode::FindInFiles if self.input.read(cx).value().is_empty() => "Type what to look for".into(),
            Mode::FindInFiles => "Nothing found".into(),
            Mode::GoToLine => SharedString::default(),
            Mode::Commands => "No command matches".into(),
            Mode::Symbols if self.input.read(cx).value().is_empty() => "Type a name".into(),
            Mode::Symbols => "No symbol matches".into(),
            Mode::Structure => "No symbol matches".into(),
            Mode::Classes if self.input.read(cx).value().is_empty() => "Type a class name, or its capitals".into(),
            Mode::Classes => "No class matches".into(),
        };
        // Nothing typed yet: a plain line saying what to type, no drawing.
        let blank = self.input.read(cx).value().trim().is_empty();
        if self.mode == Mode::GoToLine {
            let hint = match parse_line(&self.input.read(cx).value()) {
                Some((line, column)) => format!("Go to line {}, column {}", line + 1, column + 1),
                None => "Type a line number, or line:column".to_owned(),
            };
            return div()
                .id("search")
                .key_context("SearchBox")
                .debug_selector(|| "line-box".into())
                .track_focus(&self.focus)
                .on_action(cx.listener(|_, _: &SearchClose, _, cx| cx.emit(SearchEvent::Close)))
                .w(px(360.))
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .bg(p.card)
                .border_1()
                .border_color(p.border)
                .rounded(px(10.))
                .shadow_lg()
                .font_family(look::ui_font(cx))
                .text_size(px(look::BODY))
                .text_color(p.fg)
                .child(look::field(&self.input, false, None, &p))
                .child(div().px(px(4.)).text_size(px(12.)).text_color(p.weak()).child(hint));
        }
        let list = uniform_list("search-items", count, move |range, _, _| {
            range
                .map(|i| {
                    let item = &items[i];
                    let commands = item.command.is_some();
                    let (dir, name) = match item.path.rsplit_once('/').filter(|_| !commands) {
                        Some((d, n)) => (d.to_owned(), n.to_owned()),
                        None => (String::new(), item.path.clone()),
                    };
                    let view = view.clone();
                    div()
                        .id(("search-item", i))
                        .w_full()
                        .h(px(26.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(i == selected, |d| d.bg(p.selection()))
                        .when_some(item.symbol, |d, (kind, visibility)| {
                            d.pl(px(8. + 16. * item.depth as f32))
                                .child(crate::lang::symbols::badge(kind, &p))
                                .when_some(visibility, |d, v| d.child(crate::lang::symbols::visibility_mark(v, &p)))
                        })
                        .child(div().flex_none().whitespace_nowrap().child(name))
                        .child(div().flex_none().whitespace_nowrap().text_color(p.weak()).child(dir))
                        .when(commands, |d| d.child(div().flex_1()))
                        .when_some(item.keys.clone(), |d, keys| {
                            d.child(
                                div()
                                    .flex_none()
                                    .px(px(6.))
                                    .rounded(px(4.))
                                    .bg(p.button)
                                    .text_color(p.fg_dim)
                                    .text_size(px(12.))
                                    .child(keys),
                            )
                        })
                        .when_some(item.detail.clone(), |d, detail| {
                            d.child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .font_family(look::MONO)
                                    .text_color(p.fg_dim)
                                    .child(detail),
                            )
                        })
                        .on_click(move |_, _, cx| {
                            view.update(cx, |v, cx| {
                                v.selected = i;
                                v.open_selected(cx);
                            })
                        })
                })
                .collect()
        })
        .flex_1();
        div()
            .id("search")
            .key_context("SearchBox")
            .debug_selector(|| "search-box".into())
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SearchUp, _, cx| this.step(-1, cx)))
            .on_action(cx.listener(|this, _: &SearchDown, _, cx| this.step(1, cx)))
            .on_action(cx.listener(|_, _: &SearchClose, _, cx| cx.emit(SearchEvent::Close)))
            .w(px(640.))
            .h(px(420.))
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(10.))
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(look::field(&self.input, true, None, &p))
            .when(count == 0 && !blank, |d| {
                d.child(
                    crate::components::EmptyState::new(snout_core::art::spots::no_results(), empty.clone())
                        .hint("Try fewer letters, or another word.")
                        .on(p.card)
                        .compact(),
                )
            })
            .when(count == 0 && blank, |d| d.child(look::centered_note(empty, &p)))
            .when(count > 0, |d| d.child(list))
    }
}
