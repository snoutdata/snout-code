//! Go to file (⌘P) and find in files (⇧⌘F) in the open repository: a box over the window
//! with a field and what matches. Files are listed with `ignore` (so `.gitignore` holds),
//! names matched fuzzily (`fuzzy-matcher`), and text searched with `grep-searcher`.
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
use grep_matcher::Matcher as _;

use crate::look;
use crate::model::{Action, Model};
use crate::theme::palette;

actions!(search, [SearchUp, SearchDown, SearchClose]);

/// The most rows either search shows.
pub const LIMIT: usize = 200;

/// The files of `root` a search looks through, relative to it, sorted.
pub fn list_files(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().into_owned()))
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    /// From 0.
    pub line: u32,
    /// From 0, in bytes.
    pub column: u32,
    pub text: String,
}

/// Every line of the files of `root` holding `text` (a plain string; case matters only
/// when it has capitals), up to [`LIMIT`].
pub fn find_in_files(root: &Path, files: &[String], text: &str) -> Vec<Hit> {
    use grep_searcher::{BinaryDetection, SearcherBuilder, sinks::UTF8};
    if text.is_empty() {
        return Vec::new();
    }
    let Ok(matcher) = grep_regex::RegexMatcherBuilder::new().case_smart(true).fixed_strings(true).build(text) else {
        return Vec::new();
    };
    let mut searcher = SearcherBuilder::new().binary_detection(BinaryDetection::quit(0)).line_number(true).build();
    let mut hits = Vec::new();
    for file in files {
        if hits.len() >= LIMIT {
            break;
        }
        let _ = searcher.search_path(
            &matcher,
            root.join(file),
            UTF8(|line, found| {
                let column = matcher.find(found.as_bytes()).ok().flatten().map_or(0, |m| m.start());
                hits.push(Hit {
                    path: file.clone(),
                    line: line.saturating_sub(1) as u32,
                    column: column as u32,
                    text: found.trim_end().to_owned(),
                });
                Ok(hits.len() < LIMIT)
            }),
        );
    }
    hits
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    GoToFile,
    FindInFiles,
    /// Go to a line (and column) of the editor in front: `12` or `12:5`.
    GoToLine,
    /// The command palette: every command, with its shortcut.
    Commands,
}

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
#[derive(Clone, Debug)]
pub struct Item {
    pub path: String,
    pub at: Option<(u32, u32)>,
    pub detail: Option<String>,
    /// For a command: its index in the palette's list, and its shortcut.
    pub command: Option<usize>,
    pub keys: Option<String>,
}

pub enum SearchEvent {
    Close,
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
    pub items: Vec<Item>,
    pub selected: usize,
    /// The palette's commands.
    pub commands: Vec<Command>,
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
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        cx.subscribe(&input, |this, _, e: &InputEvent, cx| match e {
            InputEvent::Change => this.update(cx),
            InputEvent::PressEnter { .. } => this.open_selected(cx),
            _ => {}
        })
        .detach();
        let files =
            Arc::new(if matches!(mode, Mode::GoToLine | Mode::Commands) { Vec::new() } else { list_files(&root) });
        let mut this = Self {
            model,
            root,
            mode,
            input,
            files,
            items: Vec::new(),
            selected: 0,
            commands: Vec::new(),
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
                    })
                    .collect();
                cx.notify();
            }
            Mode::GoToFile => {
                self.items = go_to_file(&self.files, &query)
                    .into_iter()
                    .map(|(path, _)| Item { path, at: None, detail: None, command: None, keys: None })
                    .collect();
                cx.notify();
            }
            Mode::FindInFiles => {
                let (root, files) = (self.root.clone(), self.files.clone());
                self._search = Some(cx.spawn(async move |this, cx| {
                    // Wait for the typing to pause.
                    cx.background_executor().timer(Duration::from_millis(120)).await;
                    let hits =
                        cx.background_executor().spawn(async move { find_in_files(&root, &files, &query) }).await;
                    let _ = this.update(cx, |this, cx| {
                        this.items = hits
                            .into_iter()
                            .map(|h| Item {
                                detail: Some(format!("{}: {}", h.line + 1, h.text.trim())),
                                at: Some((h.line, h.column)),
                                path: h.path,
                                command: None,
                                keys: None,
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
        };
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
                .font_family(look::FONT)
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
            .font_family(look::FONT)
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(look::field(&self.input, true, None, &p))
            .when(count == 0, |d| d.child(look::centered_note(empty, &p)))
            .when(count > 0, |d| d.child(list))
    }
}
