//! Go to file (⌘P), the command palette and the symbol boxes: a box over the window with a
//! field and what matches. The host lists the files (so `.gitignore` holds); what is typed
//! matches and ranks as in every search ([`snout_find`]), the letters it matched marked.
//! Find in Files has a window of its own ([`crate::find_in_files`]).
//! ↑ ↓ pick, ↩ opens the file (at the line), Esc closes.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, FontWeight, HighlightStyle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText,
    Task, Window, actions, div, px, uniform_list,
};
use gpui_component::input::{InputEvent, InputState};
use snout_find::{Pattern, Place};
use snout_host::{Host, WalkOptions};

use crate::look;
use crate::model::{Action, Model};
use crate::theme::palette;

actions!(search, [SearchUp, SearchDown, SearchClose]);

/// The most rows either search shows.
pub const LIMIT: usize = 200;

/// The files of `root` on `host` a search looks through, relative to it, sorted; then those
/// of the source folders the build compiles from outside it (`outside`, a folder beside the
/// build that it names in its source sets), as `../shared/src/A.java`.
pub fn list_files(host: &dyn Host, root: &Path, outside: &[PathBuf]) -> Vec<String> {
    let options = WalkOptions {
        hidden: true,
        gitignore: true,
        skip: vec![".git".into()],
        files_only: true,
        ..Default::default()
    };
    let walk = |dir: &Path| host.walk(dir, &options).unwrap_or_default().into_iter();
    let mut out: Vec<String> =
        walk(root).filter_map(|e| e.path.strip_prefix(root).ok().map(snout_host::slash_path)).collect();
    out.sort();
    let mut beside: Vec<String> = outside
        .iter()
        .filter(|d| !d.starts_with(root))
        .flat_map(|d| walk(d))
        .filter_map(|e| relative_to(&e.path, root))
        .collect();
    beside.sort();
    beside.dedup();
    out.extend(beside);
    out
}

/// `path` from `root`, with `..` where it lies outside: `../core/src/A.java`. None when they
/// share no ancestor (another drive).
pub fn relative_to(path: &Path, root: &Path) -> Option<String> {
    let (path, root) = (normalize(path), normalize(root));
    if let Ok(inside) = path.strip_prefix(&root) {
        return Some(snout_host::slash_path(inside));
    }
    let (p, r): (Vec<_>, Vec<_>) = (path.components().collect(), root.components().collect());
    let common = p.iter().zip(&r).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return None;
    }
    let ups = std::iter::repeat_n("..".to_owned(), r.len() - common);
    let rest = p[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned());
    Some(ups.chain(rest).collect::<Vec<_>>().join("/"))
}

/// The file a search lists as `rel` from `root`: `root/../core/A.java` as `core/A.java`
/// beside it, so it is the same tab as when it is opened any other way.
pub fn file_at(root: &Path, rel: &str) -> PathBuf {
    normalize(&root.join(rel))
}

/// `path` without `.` and `..`, as written (no link is followed).
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir if matches!(out.components().next_back(), Some(Component::Normal(_))) => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// The files whose path matches `query`, best first, with the characters matched (see
/// [`snout_find`]). Among equals the files of `recent` (the latest first) go first, then the
/// project's own code, then a source folder beside it, then what a build made. Nothing typed:
/// the recent files, then the rest in order.
pub fn go_to_file(files: &[String], query: &str, recent: &[String]) -> Vec<(String, Vec<usize>)> {
    if Pattern::new(query).is_empty() {
        let first: std::collections::HashSet<&String> = recent.iter().collect();
        return recent
            .iter()
            .chain(files.iter().filter(|f| !first.contains(f)))
            .take(LIMIT)
            .map(|f| (f.clone(), Vec::new()))
            .collect();
    }
    let recency: std::collections::HashMap<&str, usize> =
        recent.iter().enumerate().map(|(i, p)| (p.as_str(), i)).collect();
    snout_find::rank(files, query, LIMIT, |p, _, f| {
        let lately = recency.get(f.as_str()).map_or(0, |i| 64usize.saturating_sub(*i) as u32);
        let place = match Place::of(f) {
            Place::Source | Place::Test => 2,
            Place::Outside => 1,
            Place::Generated => 0,
        };
        p.match_path(f).map(|m| m.boosted(lately * 4 + place))
    })
    .into_iter()
    .map(|(m, i)| (files[i].clone(), m.indices))
    .collect()
}

/// `text` with the characters `marks` (indices of its `char`s) shown as what the search
/// matched.
pub fn marked(text: String, marks: impl IntoIterator<Item = usize>, p: &crate::theme::Palette) -> StyledText {
    let style = HighlightStyle { color: Some(p.accent), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    let at: Vec<(usize, usize)> = text.char_indices().map(|(b, c)| (b, b + c.len_utf8())).collect();
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for i in marks {
        let Some(&(start, end)) = at.get(i) else { continue };
        match ranges.last_mut() {
            Some(r) if r.end == start => r.end = end,
            _ => ranges.push(start..end),
        }
    }
    StyledText::new(text).with_highlights(ranges.into_iter().map(|r| (r, style)))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    GoToFile,
    /// Go to a line (and column) of the editor in front: `12` or `12:5`.
    GoToLine,
    /// The command palette: every command, with its shortcut.
    Commands,
    /// Run Gradle Task…: the project's tasks, run as a command.
    Tasks,
    /// Spring Active Profiles…: how the Spring model is evaluated, picked as a command.
    Profiles,
    /// Symbols, from a language server: the workspace's (⌘T) or the file's (⇧⌘O).
    Symbols,
    /// The file's symbols as a tree, the one the caret is in picked (File Structure, ⌘F12).
    Structure,
    /// Classes of the project, its libraries and the JDK (Go to Class, ⌥⌘O).
    Classes,
    /// The files opened this session, the latest first (⌘E).
    RecentFiles,
    /// A new file: its path in the project (⌘N).
    NewFile,
}

/// Where the symbol box gets its rows: the query in, the matching symbols out.
pub type SymbolSource = std::rc::Rc<dyn Fn(String, &mut gpui::App) -> Task<Vec<Item>>>;

/// A command the palette offers: its name, what it does, and its shortcut (if any).
pub struct Command {
    pub name: SharedString,
    pub action: Box<dyn gpui::Action>,
    pub keys: Option<String>,
}

/// The commands whose name matches `query`, best first, with the characters matched (all of
/// them, in order, for an empty query).
pub fn match_commands(commands: &[Command], query: &str) -> Vec<(usize, Vec<usize>)> {
    if Pattern::new(query).is_empty() {
        return (0..commands.len()).map(|i| (i, Vec::new())).collect();
    }
    let names: Vec<&str> = commands.iter().map(|c| c.name.as_ref()).collect();
    snout_find::rank(&names, query, usize::MAX, |p, _, name| p.match_name(name))
        .into_iter()
        .map(|(m, i)| (i, m.indices))
        .collect()
}

/// The `items` whose name (`path`) matches `query`, best first, marked; in their order when
/// nothing is typed. `boost` ranks among equals.
pub fn rank_items(items: Vec<Item>, query: &str, boost: impl Fn(&Item) -> u32 + Sync) -> Vec<Item> {
    if Pattern::new(query).is_empty() {
        return items;
    }
    let ranked =
        snout_find::rank(&items, query, usize::MAX, |p, _, it| p.match_name(&it.path).map(|m| m.boosted(boost(it))));
    let mut slots: Vec<Option<Item>> = items.into_iter().map(Some).collect();
    ranked.into_iter().filter_map(|(m, i)| slots[i].take().map(|it| Item { marks: m.indices, ..it })).collect()
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
    /// The characters of `path` the search matched.
    pub marks: Vec<usize>,
}

pub enum SearchEvent {
    Close,
    /// Make this file and open it (New File).
    NewFile(PathBuf),
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
            Mode::GoToFile => ts!("search-go-to-file"),
            Mode::GoToLine => ts!("search-line-or-line-column"),
            Mode::Commands => ts!("search-type-a-command"),
            Mode::Tasks => ts!("search-run-a-gradle-task-or-maven-goal-its"),
            Mode::Profiles => ts!("search-spring-profiles-pick-one-to-turn-it-on"),
            Mode::Symbols => ts!("search-go-to-symbol"),
            Mode::Structure => ts!("search-filter-the-files-symbols"),
            Mode::Classes => ts!("search-go-to-class-in-the-project-its-libraries"),
            Mode::RecentFiles => ts!("welcome-recent-files-2"),
            Mode::NewFile => ts!("search-new-file-its-path-in-the-project-like"),
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        cx.subscribe(&input, |this, _, e: &InputEvent, cx| match e {
            InputEvent::Change => this.update(cx),
            InputEvent::PressEnter { .. } => this.open_selected(cx),
            _ => {}
        })
        .detach();
        let host = crate::host::host(cx);
        let listing = (!matches!(
            mode,
            Mode::GoToLine
                | Mode::Commands
                | Mode::Tasks
                | Mode::Profiles
                | Mode::Symbols
                | Mode::Structure
                | Mode::Classes
                | Mode::RecentFiles
                | Mode::NewFile
        ))
        .then(|| {
            let list = cx.background_executor().spawn({
                let (host, root) = (host.clone(), root.clone());
                let outside = crate::project_model::outside_roots(&root, cx);
                async move { list_files(&*host, &root, &outside) }
            });
            cx.spawn(async move |this, cx| {
                let files = list.await;
                let _ = this.update(cx, |this, cx| {
                    this.files = Arc::new(files);
                    this.update(cx);
                });
            })
        });
        let mut this = Self {
            model,
            root,
            mode,
            input,
            files: Arc::new(Vec::new()),
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
            Mode::GoToLine | Mode::NewFile => cx.notify(),
            Mode::RecentFiles => {
                let root = self.root.clone();
                let recent: Vec<String> =
                    self.model.read(cx).recent_files.iter().filter_map(|p| relative_to(p, &root)).collect();
                let at = Pattern::new(&query).line();
                // The latest first among equals.
                self.items = if Pattern::new(&query).is_empty() {
                    recent.into_iter().map(|path| Item { path, ..Default::default() }).collect()
                } else {
                    let n = recent.len() as u32;
                    snout_find::rank(&recent, &query, LIMIT, |p, i, f| p.match_path(f).map(|m| m.boosted(n - i as u32)))
                        .into_iter()
                        .map(|(m, i)| Item { path: recent[i].clone(), at, marks: m.indices, ..Default::default() })
                        .collect()
                };
                cx.notify();
            }
            Mode::Commands | Mode::Tasks | Mode::Profiles => {
                self.items = match_commands(&self.commands, &query)
                    .into_iter()
                    .map(|(i, marks)| Item {
                        path: self.commands[i].name.to_string(),
                        command: Some(i),
                        keys: self.commands[i].keys.clone(),
                        marks,
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
                let root = self.root.clone();
                let recent: Vec<String> = self
                    .model
                    .read(cx)
                    .recent_files
                    .iter()
                    .filter(|p| p.starts_with(&root))
                    .filter_map(|p| relative_to(p, &root))
                    .collect();
                // `Foo.java:12` opens at the line.
                let at = Pattern::new(&query).line();
                self.items = go_to_file(&self.files, &query, &recent)
                    .into_iter()
                    .map(|(path, marks)| Item { path, at, marks, ..Default::default() })
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
        if self.mode == Mode::NewFile {
            let rel = self.input.read(cx).value().trim().trim_start_matches('/').to_owned();
            if !rel.is_empty() && !rel.ends_with('/') {
                cx.emit(SearchEvent::NewFile(self.root.join(rel)));
            }
            return;
        }
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
        let action = Action::OpenFile(file_at(&self.root, &item.path), item.at);
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
            Mode::GoToFile => ts!("search-no-file-matches").into(),
            Mode::GoToLine => SharedString::default(),
            Mode::Commands => ts!("search-no-command-matches").into(),
            Mode::Tasks => ts!("search-no-task-matches").into(),
            Mode::Profiles => ts!("search-no-profile-matches").into(),
            Mode::Symbols if self.input.read(cx).value().is_empty() => ts!("search-type-a-name").into(),
            Mode::Symbols => ts!("search-no-symbol-matches").into(),
            Mode::Structure => ts!("search-no-symbol-matches").into(),
            Mode::Classes if self.input.read(cx).value().is_empty() => {
                ts!("search-type-a-class-name-or-its-capitals").into()
            }
            Mode::Classes => ts!("search-no-class-matches").into(),
            Mode::RecentFiles => ts!("search-no-recent-file-matches").into(),
            Mode::NewFile => ts!("search-return-makes-the-file-and-opens-it").into(),
        };
        // Nothing typed yet: a plain line saying what to type, no drawing.
        let blank = self.input.read(cx).value().trim().is_empty();
        if self.mode == Mode::GoToLine {
            let hint = match parse_line(&self.input.read(cx).value()) {
                Some((line, column)) => {
                    t!("search-go-to-line-column", line = ((line + 1) as u64), column = ((column + 1) as u64))
                }
                None => ts!("search-type-a-line-number-or-line-column").to_owned(),
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
                .child(div().px(px(4.)).text_size(px(12.)).text_color(p.weak()).child(crate::look::text(hint)));
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
                    // The marks in the name: those after the folder.
                    let skip = if dir.is_empty() && !item.path.starts_with('/') { 0 } else { dir.chars().count() + 1 };
                    let shown = marked(name.clone(), item.marks.iter().filter_map(|m| m.checked_sub(skip)), &p);
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
                        .when(i == selected, |d| d.bg(p.row_selected()))
                        .when_some(item.symbol, |d, (kind, visibility)| {
                            d.pl(px(8. + 16. * item.depth as f32))
                                .child(crate::lang::symbols::badge(kind, &p))
                                .when_some(visibility, |d, v| d.child(crate::lang::symbols::visibility_mark(v, &p)))
                        })
                        .when(item.symbol.is_none() && !commands, |d| {
                            d.child(crate::icons::file_icon(std::path::Path::new(&name), 16., &p))
                        })
                        .child(div().flex_none().whitespace_nowrap().child(shown))
                        // The folder takes the rest of the row, shortened in its middle, unless
                        // the row has more to say after it.
                        .child(if item.detail.is_none() && !commands {
                            crate::components::path_text(("search-dir", i), dir, p.fg_dim).into_any_element()
                        } else {
                            div().flex_none().whitespace_nowrap().text_color(p.fg_dim).child(dir).into_any_element()
                        })
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
                        .hint(ts!("search-try-fewer-letters-or-another-word"))
                        .on(p.card)
                        .compact(),
                )
            })
            .when(count == 0 && blank, |d| d.child(look::centered_note(empty, &p)))
            .when(count > 0, |d| d.child(list))
    }
}
