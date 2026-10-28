//! Files open in GPUI Kit's `Editor` as center tabs, highlighted with tree-sitter: Java,
//! Kotlin and YAML from GPUI Kit, Groovy (Gradle scripts) and Java properties registered
//! here. The status bar's breadcrumbs show where the file sits in the repository. ⌘S saves.
//! A tab remembers its file, so the dock layout brings it back. A tab opened by a single
//! click in the file tree is a preview: the next preview takes its place until it is
//! edited or double-clicked.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, PromptLevel, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    Task, Window, actions, div, px,
};
use gpui::{Bounds, Hsla, Pixels, canvas, point, size};
use gpui_component::dock::{PanelInfo, PanelState};
use gpui_component::input::{
    Editor, EditorState, InputEvent, InputState, Position, RangeDecoration, RangeDecorationCollection,
    RangeDecorationStyle, RopeExt as _, TabSize,
};
use snout_core::gutter::{LineChange, LineChangeKind};

use crate::editing::{self, Edit};
use crate::lang::providers::DocLink;

mod claude;
mod lang_features;
mod run_gutter;
use crate::look;
use crate::theme::{mix, palette};
pub use claude::{Answer, Outcome, Proposal, Review, STREAM_TICK};
pub use lang_features::{class_file_name, is_class_file_path};
pub use run_gutter::{GutterMenu, runnables_of};

actions!(
    editor,
    [
        Save,
        ToggleComment,
        DuplicateLine,
        MoveLineUp,
        MoveLineDown,
        ToggleSoftWrap,
        FindReferences,
        RenameSymbol,
        FormatDocument,
        OrganizeImports,
        ShowSignature,
        CancelRename,
        AcceptChanges,
        RejectChanges,
        SendToClaude,
    ]
);

/// The editors' font size, which ⌘+ and ⌘- change for every tab.
pub struct EditorFont(pub f32);

impl Global for EditorFont {}

pub const FONT_MIN: f32 = 8.;
pub const FONT_MAX: f32 = 32.;

/// What new editor tabs start with, from the settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorDefaults {
    /// The indentation of a file whose text shows none.
    pub tab_size: usize,
    pub soft_wrap: bool,
    pub show_whitespace: bool,
}

impl Global for EditorDefaults {}

impl EditorDefaults {
    pub fn of(s: &crate::settings::Settings) -> Self {
        Self { tab_size: s.tab_size.clamp(1, 8), soft_wrap: s.soft_wrap, show_whitespace: s.show_whitespace }
    }

    pub fn now(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or(Self { tab_size: 4, soft_wrap: false, show_whitespace: false })
    }
}

/// The editors' font size now.
pub fn font_size(cx: &App) -> f32 {
    cx.try_global::<EditorFont>().map_or(look::MONO_SIZE, |f| f.0)
}

/// How often an open file is checked for changes made elsewhere.
pub const DISK_POLL: Duration = Duration::from_millis(700);

/// How far bracket matching looks from the caret.
const BRACKET_REACH: usize = 16 * 1024;

/// The name an editor tab goes by in a saved layout.
pub const PANEL_NAME: &str = "Editor";

/// Register the grammars GPUI Kit does not carry.
pub fn register_languages() {
    use gpui_component::highlighter::{LanguageConfig, LanguageRegistry};
    let registry = LanguageRegistry::singleton();
    let groovy = tree_sitter::Language::new(dekobon_tree_sitter_groovy::LANGUAGE);
    registry.register(
        "groovy",
        &LanguageConfig::new("groovy", groovy, vec![], dekobon_tree_sitter_groovy::HIGHLIGHTS_QUERY, "", ""),
    );
    let properties = tree_sitter::Language::new(tree_sitter_properties::LANGUAGE);
    registry.register(
        "properties",
        &LanguageConfig::new("properties", properties, vec![], tree_sitter_properties::HIGHLIGHTS_QUERY, "", ""),
    );
}

/// The highlighter for `path`, by its name.
pub fn language_for(path: &Path) -> &'static str {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "groovy" | "gradle" | "gvy" => "groovy",
        "yml" | "yaml" => "yaml",
        "properties" => "properties",
        // A class file of a library, shown as the source the server gives.
        "class" => "java",
        _ if name == "Jenkinsfile" => "groovy",
        _ => "text",
    }
}

/// How a file is laid out: its indentation and line endings, read when it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileInfo {
    /// Indented with tabs rather than spaces.
    pub tabs: bool,
    /// Columns per indentation level.
    pub width: usize,
    pub crlf: bool,
}

impl FileInfo {
    /// Guess from the text: the step most lines indent by, and the line ending it uses.
    pub fn detect(text: &str) -> Self {
        Self::detect_or(text, 4)
    }

    /// As [`Self::detect`], with `width` for a text that shows no indentation.
    pub fn detect_or(text: &str, width: usize) -> Self {
        let mut tab_lines = 0;
        let mut space_lines = 0;
        let mut steps = [0usize; 9];
        let mut last = 0usize;
        for line in text.lines().take(5000) {
            if line.trim().is_empty() {
                continue;
            }
            if line.starts_with('\t') {
                tab_lines += 1;
                continue;
            }
            let indent = line.len() - line.trim_start_matches(' ').len();
            if indent > 0 {
                space_lines += 1;
            }
            let step = indent.abs_diff(last);
            if (2..=8).contains(&step) {
                steps[step] += 1;
            }
            last = indent;
        }
        let width =
            [4, 2, 8, 3].into_iter().max_by_key(|&w| (steps[w], w == 4)).filter(|&w| steps[w] > 0).unwrap_or(width);
        Self { tabs: tab_lines > space_lines, width, crlf: text.contains("\r\n") }
    }

    /// What the status bar says about the indentation.
    pub fn indent_label(&self) -> String {
        if self.tabs { format!("Tab, {}", self.width) } else { format!("{} spaces", self.width) }
    }
}

/// What an editor tab tells the workspace.
pub enum EditorEvent {
    /// It has the keyboard.
    Focused,
    /// Its text changed.
    Changed,
    /// It wrote its text to the file.
    Saved,
    /// It took the file's new text from disk; this was the text before.
    Reloaded(String),
    /// A new file Claude proposed was rejected: the tab has nothing left to show.
    Discarded,
}

pub struct EditorPanel {
    pub path: PathBuf,
    pub state: Entity<EditorState>,
    /// The text as it is on disk.
    saved: String,
    pub dirty: bool,
    /// The tab a single click opened; the next preview replaces it.
    pub preview: bool,
    pub info: FileInfo,
    /// The repository the file is in, for the breadcrumbs.
    root: Option<PathBuf>,
    error: Option<String>,
    /// Lines wrap at the window edge.
    pub soft_wrap: bool,
    /// The file changed on disk while the tab had changes of its own.
    pub conflict: bool,
    /// The file's modification time and length when last read or written.
    disk: Option<(SystemTime, u64)>,
    /// The frames around the bracket at the caret and its match.
    brackets: Option<RangeDecorationCollection>,
    pub bracket_pair: Option<(usize, usize)>,
    /// What the brackets were found for: the caret and the text's revision.
    seen: (usize, u64),
    revision: u64,
    /// The file as the last commit has it (none for a file Git does not track).
    head: Option<Arc<String>>,
    /// The lines added, changed and removed since the last commit, for the gutter.
    pub changes: Arc<Vec<LineChange>>,
    /// The gutter marks drawn on the last frame, in window coordinates.
    pub painted: Rc<RefCell<Vec<GutterMark>>>,
    /// The document as its language server knows it (a Java file of a Gradle build).
    pub lang: Option<Rc<DocLink>>,
    /// What the server reports about the file, as it sent it.
    pub problems: Vec<lsp_types::Diagnostic>,
    /// The call whose arguments are being typed, and the task asking about it.
    pub signature: Option<lsp_types::SignatureHelp>,
    _signature: Option<Task<()>>,
    /// The line signature help was asked on.
    signature_line: Option<u32>,
    /// The new name being typed for the symbol at the caret (F2).
    pub rename: Option<Entity<InputState>>,
    /// A class file of a library (`jdt://`), shown read only.
    pub class_file: Option<(lsp_types::Uri, PathBuf)>,
    /// A change Claude proposes, under review, and the ones waiting after it.
    pub review: Option<Review>,
    pub queue: VecDeque<Proposal>,
    /// Claude changed the file while the editor did not follow: a dot on the tab.
    pub touched: bool,
    /// Lines of a followed change, fading.
    pub flash: Option<claude::Flash>,
    _review_task: Option<Task<()>>,
    _flash_task: Option<Task<()>>,
    _diff: Option<Task<()>>,
    /// What the file holds to run (test classes and methods, main classes), for the gutter.
    pub runnables: Option<Rc<crate::run::FileRunnables>>,
    _runnables: Option<Task<()>>,
    /// The gutter's menu, while it shows.
    pub gutter_menu: Option<GutterMenu>,
    /// Where the gutter's lanes were drawn last.
    pub lanes: run_gutter::LanesCell,
    focus: FocusHandle,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for EditorPanel {}

impl EditorPanel {
    /// Open `path` in a new editor.
    pub fn open(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let defaults = EditorDefaults::now(cx);
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .soft_wrap(defaults.soft_wrap)
                .show_whitespaces(defaults.show_whitespace)
        });
        let subscriptions = vec![
            cx.subscribe(&state, |this, state, e: &InputEvent, cx| match e {
                InputEvent::Change => {
                    this.revision += 1;
                    this.lang_changed(cx);
                    // Compared in place: no copy of a large file on every keystroke.
                    let text = state.read(cx).text();
                    let dirty = text.len() != this.saved.len() || *text != this.saved.as_str();
                    if dirty {
                        // Editing keeps a preview tab.
                        this.preview = false;
                    }
                    if dirty != this.dirty {
                        this.dirty = dirty;
                    }
                    this.diff_soon(cx);
                    this.review_changed(cx);
                    cx.emit(EditorEvent::Changed);
                    cx.notify();
                }
                InputEvent::Focus => {
                    if this.touched {
                        this.touched = false;
                        cx.notify();
                    }
                    cx.emit(EditorEvent::Focused)
                }
                _ => {}
            }),
            // The caret moved or the text changed: find the brackets again, and let the
            // signature help go once the caret leaves its line.
            cx.observe(&state, |this, _, cx| {
                this.match_brackets(cx);
                this.follow_signature(cx);
            }),
            // The server forgets the document with its last tab.
            cx.on_release(|this, cx| this.detach_lang(cx)),
        ];
        let mut subscriptions = subscriptions;
        // Breakpoints and the debugger's line are drawn in the gutter.
        if let Some(marks) = crate::run::marks(cx) {
            subscriptions.push(cx.observe(&marks, |_, _, cx| cx.notify()));
        }
        let brackets = state.update(cx, |s, cx| s.create_range_decorations_collection(Vec::new(), cx));
        // Look at the file on disk now and then, for changes made elsewhere.
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(DISK_POLL).await;
                if this.update_in(cx, |this, window, cx| this.check_disk(window, cx)).is_err() {
                    return;
                }
            }
        });
        let mut this = Self {
            path: PathBuf::new(),
            state,
            saved: String::new(),
            dirty: false,
            preview: false,
            info: FileInfo::detect(""),
            root: None,
            error: None,
            soft_wrap: defaults.soft_wrap,
            conflict: false,
            disk: None,
            brackets: Some(brackets),
            bracket_pair: None,
            seen: (usize::MAX, 0),
            revision: 0,
            head: None,
            changes: Arc::new(Vec::new()),
            painted: Rc::new(RefCell::new(Vec::new())),
            lang: None,
            problems: Vec::new(),
            signature: None,
            _signature: None,
            signature_line: None,
            rename: None,
            class_file: None,
            review: None,
            queue: VecDeque::new(),
            touched: false,
            flash: None,
            _review_task: None,
            _flash_task: None,
            _diff: None,
            runnables: None,
            _runnables: None,
            gutter_menu: None,
            lanes: Default::default(),
            focus: cx.focus_handle(),
            _poll: poll,
            _subscriptions: subscriptions,
        };
        this.load(path, window, cx);
        this
    }

    /// Signature help shows while the caret stays on the line it was asked on.
    fn follow_signature(&mut self, cx: &mut Context<Self>) {
        if self.signature.is_some() {
            let line = self.state.read(cx).cursor_position().line;
            if self.signature_line.is_some_and(|l| l != line) {
                self.signature = None;
                cx.notify();
            }
        }
    }

    /// Show the file at `path` in this tab instead (a preview tab moving on).
    pub fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let (text, error) = match std::fs::read_to_string(&path) {
            Ok(text) => (text, None),
            Err(e) => (String::new(), Some(format!("Could not read {}: {e}", path.display()))),
        };
        self.info = FileInfo::detect_or(&text, EditorDefaults::now(cx).tab_size);
        let info = self.info;
        let language = language_for(&path);
        self.state.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_tab_size(TabSize { tab_size: info.width, hard_tabs: info.tabs }, cx);
            s.set_value(text.clone(), window, cx);
        });
        self.root = path.ancestors().skip(1).find(|d| d.join(".git").exists()).map(Path::to_owned);
        self.disk = disk_stamp(&path);
        self.path = path;
        self.saved = text;
        self.dirty = false;
        self.conflict = false;
        self.error = error;
        self.head = None;
        self.changes = Arc::new(Vec::new());
        self.refresh_head(cx);
        self.attach_lang(cx);
        self.runnables = None;
        self.refresh_runnables(cx);
        cx.notify();
    }

    /// Read the file as the last commit has it, then mark the gutter again.
    pub fn refresh_head(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let Ok(rel) = self.path.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/")) else { return };
        let path = self.path.clone();
        self._diff = Some(cx.spawn(async move |this, cx| {
            let head = cx.background_executor().spawn(async move { snout_core::gutter::head_text(&root, &rel) }).await;
            let _ = this.update(cx, |this, cx| {
                if this.path == path {
                    this.head = head.map(Arc::new);
                    this.diff_now(cx);
                }
            });
        }));
    }

    /// Mark the gutter again once typing pauses.
    fn diff_soon(&mut self, cx: &mut Context<Self>) {
        if self.head.is_none() && self.changes.is_empty() {
            return;
        }
        self._diff = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(120)).await;
            let _ = this.update(cx, |this, cx| this.diff_now(cx));
        }));
    }

    /// Diff the text against the commit (off the main thread) and mark the gutter.
    fn diff_now(&mut self, cx: &mut Context<Self>) {
        let Some(head) = self.head.clone() else {
            if !self.changes.is_empty() {
                self.changes = Arc::new(Vec::new());
                cx.notify();
            }
            return;
        };
        let text = self.state.read(cx).text().to_string();
        let revision = self.revision;
        self._diff = Some(cx.spawn(async move |this, cx| {
            let changes =
                cx.background_executor().spawn(async move { snout_core::gutter::line_changes(&head, &text) }).await;
            let _ = this.update(cx, |this, cx| {
                if this.revision == revision {
                    this.changes = Arc::new(changes);
                    cx.notify();
                }
            });
        }));
    }

    /// Read the file again, keeping the caret where it was.
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.state.read(cx).cursor_position();
        let preview = self.preview;
        self.load(self.path.clone(), window, cx);
        self.preview = preview;
        self.state.update(cx, |s, cx| s.set_cursor_position(at, window, cx));
    }

    /// Keep this tab's text over what changed on disk; saving writes it.
    pub fn keep_mine(&mut self, cx: &mut Context<Self>) {
        self.conflict = false;
        self.disk = disk_stamp(&self.path);
        cx.notify();
    }

    /// If the file changed on disk: take the new text when this tab has no changes, or
    /// say so when it has.
    pub fn check_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.class_file.is_some() {
            return;
        }
        let now = disk_stamp(&self.path);
        if now.is_none() || now == self.disk || self.conflict {
            return;
        }
        self.disk = now;
        let Ok(text) = std::fs::read_to_string(&self.path) else { return };
        if text == self.saved {
            return;
        }
        if self.dirty {
            self.conflict = true;
            cx.notify();
            return;
        }
        let before = std::mem::take(&mut self.saved);
        self.reload(window, cx);
        cx.emit(EditorEvent::Reloaded(before));
    }

    /// Frame the bracket at the caret and its match, once per caret move or edit.
    fn match_brackets(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let caret = state.cursor();
        if self.seen == (caret, self.revision) {
            return;
        }
        self.seen = (caret, self.revision);
        let text = state.text();
        // Look in a window around the caret, cut at characters.
        let snap = |at: usize| text.char_index_to_offset(text.offset_to_char_index(at));
        let start = snap(caret.saturating_sub(BRACKET_REACH));
        let end = snap((caret + BRACKET_REACH).min(text.len()));
        let window = text.slice(start..end).to_string();
        let pair =
            editing::matching_bracket(&window, caret - start, BRACKET_REACH).map(|(a, b)| (a + start, b + start));
        if pair == self.bracket_pair {
            return;
        }
        self.bracket_pair = pair;
        let p = palette(cx);
        let color = mix(p.fg, 0.45, p.panel);
        let frames = pair
            .map(|(a, b)| {
                [a, b]
                    .map(|at| {
                        RangeDecoration::new(at..at + 1).with_style(RangeDecorationStyle::Frame).with_color(color)
                    })
                    .to_vec()
            })
            .unwrap_or_default();
        if let Some(brackets) = self.brackets.clone() {
            brackets.set(frames, cx);
        }
    }

    /// Replace a part of the text and select what the edit says, as one undo step.
    fn apply(&mut self, edit: Edit, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.set_selected_range(edit.range.clone(), cx);
            s.replace(edit.text, window, cx);
            s.set_selected_range(edit.select, cx);
        });
    }

    /// The text and the selection, for the line commands.
    fn text_and_selection(&self, cx: &App) -> (String, std::ops::Range<usize>) {
        let s = self.state.read(cx);
        (s.text().to_string(), s.selected_range())
    }

    pub fn toggle_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prefix) = editing::comment_prefix(language_for(&self.path)) else { return };
        let (text, sel) = self.text_and_selection(cx);
        self.apply(editing::toggle_comment(&text, sel, prefix), window, cx);
    }

    pub fn duplicate_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, sel) = self.text_and_selection(cx);
        self.apply(editing::duplicate(&text, sel), window, cx);
    }

    pub fn move_lines(&mut self, up: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (text, sel) = self.text_and_selection(cx);
        if let Some(edit) = editing::move_lines(&text, sel, up) {
            self.apply(edit, window, cx);
        }
    }

    /// Take the settings' soft wrap and whitespace.
    pub fn apply_defaults(&mut self, d: EditorDefaults, window: &mut Window, cx: &mut Context<Self>) {
        self.soft_wrap = d.soft_wrap;
        self.state.update(cx, |s, cx| {
            s.set_soft_wrap(d.soft_wrap, window, cx);
            s.set_show_whitespaces(d.show_whitespace, window, cx);
        });
        cx.notify();
    }

    pub fn toggle_soft_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.soft_wrap = !self.soft_wrap;
        let wrap = self.soft_wrap;
        self.state.update(cx, |s, cx| s.set_soft_wrap(wrap, window, cx));
        cx.notify();
    }

    /// The number of lines in the file.
    pub fn line_count(&self, cx: &App) -> usize {
        self.state.read(cx).text().lines_len()
    }

    /// Write the text to the file.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        if self.class_file.is_some() {
            return;
        }
        let text = self.state.read(cx).value().to_string();
        match std::fs::write(&self.path, &text) {
            Ok(()) => {
                self.disk = disk_stamp(&self.path);
                self.conflict = false;
                if let Some(link) = &self.lang
                    && let Some(hub) = link.hub.upgrade()
                {
                    let uri = link.uri.clone();
                    hub.update(cx, |h, _| h.saved(&uri));
                }
                cx.emit(EditorEvent::Saved);
                // A saved Scala file: the engine compiles the Scala again.
                if self.path.extension().is_some_and(|e| e == "scala")
                    && let Some(runs) = crate::run::runs(cx)
                {
                    let path = self.path.clone();
                    runs.update(cx, |r, cx| r.scala_changed(&path, cx));
                }
                self.saved = text;
                self.refresh_runnables(cx);
                self.dirty = false;
                self.preview = false;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Could not save {}: {e}", self.path.display())),
        }
        cx.notify();
    }

    /// Take `text` from the other view of this file (a split), changing only what
    /// differs, so this view's caret and scroll stay where they are.
    pub fn sync_from(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (range, insert) = {
            let mine = self.state.read(cx).text();
            if mine.len() == text.len() && *mine == text {
                return;
            }
            let mine = mine.to_string();
            let (a, b) = (mine.as_bytes(), text.as_bytes());
            let mut start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
            while !mine.is_char_boundary(start) || !text.is_char_boundary(start) {
                start -= 1;
            }
            let max_tail = a.len().min(b.len()) - start;
            let mut tail = a.iter().rev().zip(b.iter().rev()).take(max_tail).take_while(|(x, y)| x == y).count();
            while !mine.is_char_boundary(a.len() - tail) || !text.is_char_boundary(b.len() - tail) {
                tail -= 1;
            }
            (start..a.len() - tail, text[start..b.len() - tail].to_owned())
        };
        self.state.update(cx, |s, cx| {
            let sel = s.selected_range();
            let delta = insert.len() as isize - range.len() as isize;
            let shift = |p: usize| if p >= range.end { (p as isize + delta) as usize } else { p.min(range.start) };
            s.set_selected_range(range.clone(), cx);
            s.replace(insert, window, cx);
            s.set_selected_range(shift(sel.start)..shift(sel.end), cx);
        });
    }

    /// The other view of this file saved it: this one is saved too.
    pub fn mark_saved(&mut self, cx: &mut Context<Self>) {
        self.saved = self.text(cx);
        self.disk = disk_stamp(&self.path);
        self.dirty = false;
        self.conflict = false;
        cx.notify();
    }

    /// Drop the unsaved changes (the window is closing without saving them).
    pub fn discard(&mut self, cx: &mut Context<Self>) {
        self.dirty = false;
        cx.notify();
    }

    /// Keep the tab: it stops being the preview.
    pub fn pin(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            self.preview = false;
            cx.notify();
        }
    }

    /// Put the caret on `line` (from 0), at `column`, and give the editor the keyboard.
    pub fn go_to(&mut self, line: u32, column: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.set_cursor_position(Position::new(line, column), window, cx);
            s.focus(window, cx);
        });
    }

    /// The text in the editor.
    pub fn text(&self, cx: &App) -> String {
        self.state.read(cx).value().to_string()
    }

    /// The file's name, as its tab shows it.
    pub fn name(&self) -> SharedString {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into()
    }

    /// The folders from the repository down to the file, then the file.
    pub fn crumbs(&self) -> Vec<String> {
        let rel = match &self.root {
            Some(root) => self.path.strip_prefix(root).unwrap_or(&self.path),
            None => self.path.file_name().map(Path::new).unwrap_or(&self.path),
        };
        let mut out: Vec<String> = Vec::new();
        if let Some(root) = &self.root {
            out.push(crate::model::Model::dir_name(root));
        }
        out.extend(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()));
        out
    }
}

impl EditorPanel {
    /// The warning when the file changed on disk under unsaved changes.
    fn conflict_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .id("conflict")
            .flex_none()
            .mx(px(8.))
            .mb(px(4.))
            .px(px(10.))
            .py(px(5.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(6.))
            .bg(p.wash(p.warn))
            .border_1()
            .border_color(mix(p.warn, 0.4, p.panel))
            .child(crate::art::glyph(snout_core::art::Glyph::Alert, 14., p.warn))
            .child(div().flex_1().child("This file changed on disk, and this tab has unsaved changes."))
            .child(
                look::small_button("conflict-reload", "Reload from disk", &p)
                    .on_click(cx.listener(|this, _, w, cx| this.reload(w, cx))),
            )
            .child(
                look::small_button("conflict-keep", "Keep my changes", &p)
                    .on_click(cx.listener(|this, _, _, cx| this.keep_mine(cx))),
            )
    }
}

impl EditorPanel {
    /// The Git marks in the gutter, drawn over the editor beside the line numbers and
    /// placed from the editor's own line layout, so they follow scrolling, folding and
    /// wrapping.
    fn gutter(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let colors = [p.ok, if p.dark { p.accent } else { p.warn }, p.error];
        // Under review, the gutter shows the proposed change instead of Git's.
        let changes = self.review_gutter().map_or_else(|| self.changes.clone(), Arc::new);
        let (state, painted) = (self.state.clone(), self.painted.clone());
        canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                let marks = gutter_marks(state.read(cx), &changes);
                for (kind, b) in &marks {
                    let color = match kind {
                        LineChangeKind::Added => colors[0],
                        LineChangeKind::Modified => colors[1],
                        LineChangeKind::Deleted => colors[2],
                    };
                    paint_mark(*kind, *b, color, window);
                }
                *painted.borrow_mut() = marks;
            },
        )
        .absolute()
        .inset_0()
    }
}

/// A mark in the gutter: what changed, and where it is drawn.
pub type GutterMark = (LineChangeKind, Bounds<Pixels>);

/// Width of a gutter mark, and how far left of the code it sits.
const MARK_W: f32 = 3.;
const MARK_GAP: f32 = 6.;

/// Where the gutter marks of `changes` go in the editor's last layout: one bar per run
/// of visible changed lines, and a small wedge where lines were removed.
pub fn gutter_marks(state: &EditorState, changes: &[LineChange]) -> Vec<GutterMark> {
    let Some(rows) = state.visible_row_range() else { return Vec::new() };
    let Some(line_height) = state.line_height() else { return Vec::new() };
    let text = state.text();
    let lines = text.lines_len();
    let map = state.display_map();
    let first = map.display_row_to_buffer_line(rows.start);
    let last = (map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start)) + 1).min(lines);
    let scroll_x = state.scroll_offset().x.min(px(0.));
    let top_of = |line: usize| -> Option<(Pixels, Pixels)> {
        let at = text.line_start_offset(line.min(lines.saturating_sub(1)));
        let b = state.range_to_bounds(&(at..at))?;
        let x = b.left() - scroll_x - px(MARK_GAP + MARK_W);
        Some(if line >= lines { (x, b.top() + line_height) } else { (x, b.top()) })
    };
    let mut out = Vec::new();
    for c in changes {
        if c.kind == LineChangeKind::Deleted {
            if (first..=last).contains(&c.lines.start)
                && let Some((x, y)) = top_of(c.lines.start)
            {
                out.push((c.kind, Bounds::new(point(x - px(1.), y - px(4.)), size(px(6.), px(8.)))));
            }
            continue;
        }
        let (a, b) = (c.lines.start.max(first), c.lines.end.min(last));
        if a >= b {
            continue;
        }
        let (Some((x, top)), Some((_, bottom))) = (top_of(a), top_of(b - 1)) else { continue };
        out.push((c.kind, Bounds::new(point(x, top), size(px(MARK_W), bottom + line_height - top))));
    }
    out
}

fn paint_mark(kind: LineChangeKind, b: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    if kind == LineChangeKind::Deleted {
        let mut path = gpui::PathBuilder::fill();
        path.move_to(b.origin);
        path.line_to(point(b.right(), b.center().y));
        path.line_to(b.bottom_left());
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    } else {
        window.paint_quad(gpui::fill(b, color).corner_radii(px(1.5)));
    }
}

/// A file's modification time and length, to notice changes made elsewhere.
fn disk_stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

impl gpui_component::dock::BasePanel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        let info = match &self.class_file {
            Some((uri, root)) => serde_json::json!({ "path": self.path, "classFile": uri.as_str(), "root": root }),
            None => serde_json::json!({ "path": self.path }),
        };
        state.info = PanelInfo::panel(info);
        state
    }
}

impl gpui::EventEmitter<gpui_component::dock::PanelEvent> for EditorPanel {}

impl gpui::Focusable for EditorPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle(cx)
    }
}

impl gpui_component::dock::Panel for EditorPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.name()
    }

    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.name())
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}

/// The file an editor tab in a saved layout had open.
pub fn saved_path(state: &PanelState) -> Option<PathBuf> {
    match &state.info {
        PanelInfo::Panel(v) => v.get("path").and_then(|p| p.as_str()).map(PathBuf::from),
        _ => None,
    }
}

/// The class file (its URI and build) an editor tab in a saved layout showed.
pub fn saved_class_file(state: &PanelState) -> Option<(lsp_types::Uri, PathBuf)> {
    use std::str::FromStr as _;
    match &state.info {
        PanelInfo::Panel(v) => {
            let uri = v.get("classFile").and_then(|u| u.as_str()).and_then(|u| lsp_types::Uri::from_str(u).ok())?;
            let root = v.get("root").and_then(|r| r.as_str()).map(PathBuf::from)?;
            Some((uri, root))
        }
        _ => None,
    }
}

impl Render for EditorPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let _ = &self.focus;
        let reviewing = self.review.is_some();
        let has_lang = self.lang.is_some();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.panel)
            .key_context(if reviewing { "EditorPanel ClaudeReview" } else { "EditorPanel" })
            .when(reviewing, |d| {
                d.on_action(cx.listener(|this, _: &AcceptChanges, w, cx| this.accept_review(w, cx)))
                    .on_action(cx.listener(|this, _: &RejectChanges, w, cx| this.reject_review(w, cx)))
            })
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save(cx)))
            .on_action(cx.listener(|this, _: &ToggleComment, w, cx| this.toggle_comment(w, cx)))
            .on_action(cx.listener(|this, _: &DuplicateLine, w, cx| this.duplicate_line(w, cx)))
            .on_action(cx.listener(|this, _: &MoveLineUp, w, cx| this.move_lines(true, w, cx)))
            .on_action(cx.listener(|this, _: &MoveLineDown, w, cx| this.move_lines(false, w, cx)))
            .on_action(cx.listener(|this, _: &ToggleSoftWrap, w, cx| this.toggle_soft_wrap(w, cx)))
            .when(self.lang.is_some(), |d| {
                d.on_action(cx.listener(|this, _: &FindReferences, _, cx| this.find_references(cx)))
                    .on_action(cx.listener(|this, _: &RenameSymbol, w, cx| this.start_rename(w, cx)))
                    .on_action(cx.listener(|this, _: &FormatDocument, _, cx| this.format_document(cx)))
                    .on_action(cx.listener(|this, _: &OrganizeImports, _, cx| this.organize_imports(cx)))
                    .on_action(cx.listener(|this, _: &ShowSignature, _, cx| this.ask_signature(cx)))
                    .on_action(cx.listener(|this, _: &CancelRename, w, cx| this.cancel_rename(w, cx)))
            })
            .when_some(self.error.clone(), |d, e| d.child(div().px(px(12.)).py(px(4.)).text_color(p.error).child(e)))
            .when(self.conflict, |d| d.child(self.conflict_bar(cx)))
            .children(self.review_bar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .font_family(look::mono_font(cx))
                    .capture_any_mouse_down(cx.listener(|this, e: &gpui::MouseDownEvent, window, cx| {
                        if this.gutter_press(e, window, cx) || this.command_click(e, window, cx) {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        Editor::new(&self.state)
                            .appearance(false)
                            .h_full()
                            .text_size(px(font_size(cx)))
                            .context_menu(move |menu, _, _| context_menu(menu, has_lang)),
                    )
                    .children(self.claude_overlay(cx))
                    .child(self.gutter(cx))
                    .child(self.problem_marks(cx))
                    .child(self.run_gutter(cx))
                    .children(self.gutter_popup(cx))
                    .children(self.signature_popup(cx))
                    .children(self.inspections(cx))
                    .children(self.rename_box(cx)),
            )
    }
}

/// The editor's right-click menu: the code's own items, sending the selection to Claude
/// Code, and the clipboard.
fn context_menu(menu: gpui_component::native_menu::NativeMenu, lang: bool) -> gpui_component::native_menu::NativeMenu {
    use gpui_component::input as i;
    let menu = if lang {
        menu.menu("Go to Definition", Box::new(i::GoToDefinition))
            .menu("Quick Fix…", Box::new(i::ToggleCodeActions))
            .separator()
    } else {
        menu
    };
    menu.menu("Send Selection to Claude", Box::new(SendToClaude))
        .separator()
        .menu("Cut", Box::new(i::Cut))
        .menu("Copy", Box::new(i::Copy))
        .menu("Paste", Box::new(i::Paste))
        .separator()
        .menu("Select All", Box::new(i::SelectAll))
}

/// Close an editor tab (`then` does the closing), asking first whether to save it when
/// it has unsaved changes.
pub fn confirm_close(
    editor: Option<Entity<EditorPanel>>,
    window: &mut Window,
    cx: &mut App,
    then: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    let Some(editor) = editor.filter(|e| e.read(cx).dirty) else { return then(window, cx) };
    let name = editor.read(cx).name();
    let answer = window.prompt(
        PromptLevel::Warning,
        &format!("Do you want to save the changes you made to {name}?"),
        Some("Your changes are lost if you don't save them."),
        &["Save", "Don't Save", "Cancel"],
        cx,
    );
    window
        .spawn(cx, async move |cx| {
            let Ok(answer) = answer.await else { return };
            let _ = cx.update(|window, cx| match answer {
                0 => {
                    editor.update(cx, |e, cx| e.save(cx));
                    if !editor.read(cx).dirty {
                        then(window, cx);
                    }
                }
                1 => then(window, cx),
                _ => {}
            });
        })
        .detach();
}
