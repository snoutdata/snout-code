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
use std::time::{Duration, Instant, SystemTime};

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
mod refactor;
mod run_gutter;
mod spring_gutter;
use crate::look;
use crate::theme::{mix, palette};
pub use claude::{Answer, ClaudeRequest, LIVE_TICK, Outcome, Proposal, Review, STREAM_TICK};
pub use lang_features::{class_file_name, is_class_file_path};
pub use refactor::{Refactoring, new_name};
pub use run_gutter::{CODE_PAD, GutterMenu, Lanes, PLAIN_PAD, runnables_of};
pub use spring_gutter::{SpringItem, spring_items, spring_of};

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
        ExtractMethod,
        ExtractVariable,
        ExtractConstant,
        ExtractField,
        Inline,
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
    /// The caret moved (the session remembers where it is).
    CaretMoved,
    /// Something to do with Claude Code's changes (a key, a button of the tab's bar).
    Claude(ClaudeRequest),
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
    /// The ways a refactoring can go, while the menu at the caret asks which.
    pub choices: Option<refactor::Choices>,
    /// A note at the caret: why a command did nothing.
    pub hint: Option<String>,
    _hint: Option<Task<()>>,
    _refactor: Option<Task<()>>,
    /// A class file of a library (`jdt://`), shown read only.
    pub class_file: Option<(lsp_types::Uri, PathBuf)>,
    /// A change Claude proposes, under review, and the ones waiting after it.
    pub review: Option<Review>,
    pub queue: VecDeque<Proposal>,
    /// Claude changed the file while the editor did not follow: a dot on the tab.
    pub touched: bool,
    /// Lines of a followed change, fading.
    pub flash: Option<claude::Flash>,
    /// An edit Claude made on its own, playing out: struck, typed in, lit.
    pub live: Option<claude::Live>,
    _live_task: Option<Task<()>>,
    /// The struck text of a live edit.
    struck: Option<gpui_component::input::TextDecorationCollection>,
    /// Claude is about to change the file: the disk check waits for its report.
    pub claude_hold: Option<Instant>,
    /// The file before Claude's unreviewed changes (empty for a file Claude made), and
    /// the hunks from it to the text, for the gutter's Claude marks.
    pub claude_base: Option<Arc<String>>,
    pub claude_hunks: Arc<Vec<crate::claude_ide::review::Hunk>>,
    /// Claude's changes shown inline, the bar above them.
    pub claude_diff: bool,
    /// The Claude marks drawn on the last frame: the hunk's first new line and where.
    pub claude_painted: Rc<RefCell<Vec<ClaudeMark>>>,
    /// When the user last typed here, and how many changes the app made that the editor
    /// has yet to report (they are not the user's typing).
    pub edited_at: Option<Instant>,
    writing: u32,
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
    /// The padding left of the line numbers, for the gutter's lanes.
    pub gutter_pad: f32,
    /// The file's Spring marks (beans, injection points, endpoints), by line.
    pub spring: Option<Rc<crate::spring::FileSpring>>,
    /// The Spring model's revision the marks were asked for under.
    spring_seen: u64,
    _spring: Option<Task<()>>,
    _spring_settle: Option<Task<()>>,
    /// The resolved values of `@Value` placeholders, by line.
    pub hints: Rc<Vec<(u32, String)>>,
    _hints: Option<Task<()>>,
    /// The Spring mark under the pointer: its line, where the pointer is, and what it says.
    pub gutter_tip: Option<(u32, gpui::Point<Pixels>, String)>,
    /// Where the caret was when last looked at, to tell when it moves.
    caret_seen: usize,
    /// How far down a restored tab's view was, until it has been laid out to scroll there.
    pending_scroll: Option<f32>,
    /// The file is being read from the host; the tab shows it once it is.
    pub loading: bool,
    /// What was asked of the tab while its file was being read, done once it is: where
    /// to put the caret (and whether to take the keyboard), and text to take from a split.
    pending_caret: Option<(Position, bool)>,
    pending_sync: Option<String>,
    /// What waits for the file to be read.
    #[allow(clippy::type_complexity)]
    after_load: Vec<Box<dyn FnOnce(&mut Self, &mut Window, &mut Context<Self>)>>,
    /// The Gradle build the file belongs to, as the host showed it when the file was read.
    pub(crate) build_root: Option<PathBuf>,
    /// The file was there when last read (a new file Claude proposes is not).
    on_disk: bool,
    /// A look at the file on the host is under way.
    checking: bool,
    /// A tab a saved session brought back: it closes when its file has gone.
    pub close_if_gone: bool,
    _load: Option<Task<()>>,
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
                    this.spring_changed(cx);
                    if this.writing > 0 {
                        this.writing -= 1;
                    } else {
                        this.edited_at = Some(Instant::now());
                    }
                    // A live edit decides at its end whether the tab differs from the file.
                    if this.live.is_none() {
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
            cx.observe(&state, |this, state, cx| {
                this.match_brackets(cx);
                this.follow_signature(cx);
                let caret = state.read(cx).cursor();
                if caret != this.caret_seen {
                    this.caret_seen = caret;
                    cx.emit(EditorEvent::CaretMoved);
                }
            }),
            // The server forgets the document with its last tab.
            cx.on_release(|this, cx| this.detach_lang(cx)),
        ];
        let mut subscriptions = subscriptions;
        // Breakpoints and the debugger's line are drawn in the gutter.
        if let Some(marks) = crate::run::marks(cx) {
            subscriptions.push(cx.observe(&marks, |_, _, cx| cx.notify()));
        }
        // The Spring model read again: the file's marks may have changed.
        if let Some(spring) = crate::spring::spring(cx) {
            subscriptions.push(cx.observe(&spring, |this, _, cx| this.spring_model_changed(cx)));
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
            choices: None,
            hint: None,
            _hint: None,
            _refactor: None,
            class_file: None,
            review: None,
            queue: VecDeque::new(),
            touched: false,
            flash: None,
            live: None,
            _live_task: None,
            struck: None,
            claude_hold: None,
            claude_base: None,
            claude_hunks: Arc::new(Vec::new()),
            claude_diff: false,
            claude_painted: Rc::new(RefCell::new(Vec::new())),
            edited_at: None,
            writing: 0,
            _review_task: None,
            _flash_task: None,
            _diff: None,
            runnables: None,
            _runnables: None,
            gutter_menu: None,
            lanes: Default::default(),
            gutter_pad: run_gutter::PLAIN_PAD,
            spring: None,
            spring_seen: 0,
            _spring: None,
            _spring_settle: None,
            hints: Rc::new(Vec::new()),
            _hints: None,
            gutter_tip: None,
            caret_seen: 0,
            pending_scroll: None,
            loading: false,
            pending_caret: None,
            pending_sync: None,
            after_load: Vec::new(),
            build_root: None,
            on_disk: false,
            checking: false,
            close_if_gone: false,
            _load: None,
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

    /// Show the file at `path` in this tab instead (a preview tab moving on). The file is
    /// read off the UI thread; the tab shows it when it comes.
    pub fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.detach_lang(cx);
        self.path = path.clone();
        self.loading = true;
        self.error = None;
        self.conflict = false;
        self.head = None;
        self.changes = Arc::new(Vec::new());
        self.runnables = None;
        self.spring = None;
        self.hints = Rc::new(Vec::new());
        if path.as_os_str().is_empty() {
            // A tab about to show something else (a class file).
            self.loading = false;
            self._load = None;
            self.after_load.clear();
            cx.notify();
            return;
        }
        let host = crate::host::host(cx);
        let project = crate::lang::hub(cx).and_then(|h| h.read(cx).model.read(cx).project_root());
        let served = language_for(&path) == "java" || crate::spring::is_config_file(&path);
        let read = cx.background_executor().spawn(async move { Loaded::read(&*host, &path, project, served) });
        self._load = Some(cx.spawn_in(window, async move |this, cx| {
            let loaded = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.path == loaded.path && this.loading {
                    this.show_loaded(loaded, window, cx);
                }
            });
        }));
        cx.notify();
    }

    /// The file as [`Loaded::read`] found it, in the tab.
    fn show_loaded(&mut self, loaded: Loaded, window: &mut Window, cx: &mut Context<Self>) {
        let gone = loaded.text.as_ref().is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
        if std::mem::take(&mut self.close_if_gone) && gone {
            self.loading = false;
            self._load = None;
            cx.emit(EditorEvent::Discarded);
            return;
        }
        let (text, error) = match loaded.text {
            Ok(text) => (text, None),
            Err(e) => (String::new(), Some(format!("Could not read {}: {e}", loaded.path.display()))),
        };
        self.on_disk = error.is_none();
        self.info = FileInfo::detect_or(&text, EditorDefaults::now(cx).tab_size);
        let info = self.info;
        let language = language_for(&loaded.path);
        self.gutter_pad = run_gutter::gutter_pad(&loaded.path);
        self.state.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_tab_size(TabSize { tab_size: info.width, hard_tabs: info.tabs }, cx);
            s.set_value(text.clone(), window, cx);
        });
        self.loading = false;
        self._load = None;
        self.root = loaded.root;
        self.disk = loaded.disk;
        self.build_root = loaded.build;
        self.saved = text;
        self.dirty = false;
        self.conflict = false;
        self.error = error;
        self.refresh_head(cx);
        self.attach_lang(cx);
        self.refresh_runnables(cx);
        self.refresh_spring(cx);
        if let Some(text) = self.pending_sync.take() {
            self.sync_from(&text, window, cx);
        }
        // The caret goes where it was asked (taking the keyboard, for a go to).
        if let Some((at, focus)) = self.pending_caret.take() {
            self.state.update(cx, |s, cx| {
                if focus {
                    s.set_cursor_position(at, window, cx);
                } else {
                    let offset = s.text().position_to_offset(&at);
                    s.set_selected_range(offset..offset, cx);
                }
            });
            self.caret_seen = self.state.read(cx).cursor();
        }
        if self.review.is_none() && !self.queue.is_empty() {
            self.next_review(window, cx);
        }
        for f in std::mem::take(&mut self.after_load) {
            f(self, window, cx);
        }
        cx.notify();
    }

    /// Run `f` once the tab shows its file: now, or when it has been read.
    pub fn after_load(
        &mut self,
        f: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            self.after_load.push(Box::new(f));
        } else {
            f(self, window, cx);
        }
    }

    /// Read the file as the last commit has it, then mark the gutter again.
    pub fn refresh_head(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let Ok(rel) = self.path.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/")) else { return };
        let path = self.path.clone();
        let host = crate::host::host(cx);
        self._diff = Some(cx.spawn(async move |this, cx| {
            let head =
                cx.background_executor().spawn(async move { snout_core::gutter::head_text(&*host, &root, &rel) }).await;
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
        if self.head.is_none() && self.changes.is_empty() && self.claude_base.is_none() && self.claude_hunks.is_empty()
        {
            return;
        }
        self._diff = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(120)).await;
            let _ = this.update(cx, |this, cx| this.diff_now(cx));
        }));
    }

    /// Diff the text against the commit (off the main thread) and mark the gutter.
    fn diff_now(&mut self, cx: &mut Context<Self>) {
        let (head, base) = (self.head.clone(), self.claude_base.clone());
        if head.is_none() && !self.changes.is_empty() {
            self.changes = Arc::new(Vec::new());
            cx.notify();
        }
        if base.is_none() && !self.claude_hunks.is_empty() {
            self.claude_hunks = Arc::new(Vec::new());
            cx.notify();
        }
        if head.is_none() && base.is_none() {
            return;
        }
        let text = self.state.read(cx).text().to_string();
        let revision = self.revision;
        self._diff = Some(cx.spawn(async move |this, cx| {
            let (changes, claude) = cx
                .background_executor()
                .spawn(async move {
                    let changes = head.map(|head| snout_core::gutter::line_changes(&head, &text));
                    let claude = base.map(|base| crate::claude_ide::review::hunks(&base, &text));
                    (changes, claude)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.revision == revision {
                    if let Some(changes) = changes {
                        this.changes = Arc::new(changes);
                    }
                    if let Some(claude) = claude
                        && this.claude_base.is_some()
                    {
                        this.claude_hunks = Arc::new(claude);
                    }
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
        self.pending_caret = Some((at, false));
    }

    /// Note the file's modification time and length as they are now (asked of the host off
    /// the UI thread; the disk check waits for it).
    pub(crate) fn take_disk_stamp(&mut self, cx: &mut Context<Self>) {
        self.conflict = false;
        if self.path.as_os_str().is_empty() {
            return;
        }
        let (host, path) = (crate::host::host(cx), self.path.clone());
        let stamp = cx.background_executor().spawn(async move { disk_stamp(&*host, &path) });
        self.checking = true;
        let path = self.path.clone();
        cx.spawn(async move |this, cx| {
            let disk = stamp.await;
            let _ = this.update(cx, |this, _| {
                this.checking = false;
                if this.path == path {
                    this.disk = disk;
                }
            });
        })
        .detach();
    }

    /// Keep this tab's text over what changed on disk; saving writes it.
    pub fn keep_mine(&mut self, cx: &mut Context<Self>) {
        self.conflict = false;
        let (host, path) = (crate::host::host(cx), self.path.clone());
        let stamp = cx.background_executor().spawn(async move { disk_stamp(&*host, &path) });
        cx.spawn(async move |this, cx| {
            let disk = stamp.await;
            let _ = this.update(cx, |this, _| this.disk = disk);
        })
        .detach();
        cx.notify();
    }

    /// If the file changed on disk: take the new text when this tab has no changes, or
    /// say so when it has. The host is asked off the UI thread.
    pub fn check_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.class_file.is_some() || self.loading || self.checking || self.conflict || self.live.is_some() {
            return;
        }
        // Claude is changing the file: its report shows the change (for a while at most).
        if self.claude_hold.is_some_and(|t| t.elapsed() < claude::HOLD) {
            return;
        }
        if self.path.as_os_str().is_empty() {
            return;
        }
        self.checking = true;
        let (host, path, known) = (crate::host::host(cx), self.path.clone(), self.disk);
        let look = cx.background_executor().spawn(async move {
            let now = disk_stamp(&*host, &path);
            let text = (now.is_some() && now != known).then(|| host.read_to_string(&path).ok()).flatten();
            (now, text)
        });
        let path = self.path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let (now, text) = look.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.checking = false;
                if this.path != path || this.loading || this.conflict || now.is_none() || now == this.disk {
                    return;
                }
                this.disk = now;
                let Some(text) = text else { return };
                if text == this.saved {
                    return;
                }
                if this.dirty {
                    this.conflict = true;
                    cx.notify();
                    return;
                }
                let before = std::mem::take(&mut this.saved);
                let at = this.state.read(cx).cursor_position();
                let loaded = Loaded {
                    path: this.path.clone(),
                    text: Ok(text),
                    root: this.root.clone(),
                    disk: now,
                    build: this.build_root.clone(),
                };
                this.loading = true;
                this.pending_caret = Some((at, false));
                this.show_loaded(loaded, window, cx);
                cx.emit(EditorEvent::Reloaded(before));
            });
        })
        .detach();
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

    /// Write the text to the file, off the UI thread. The task says whether it was written;
    /// the tab stays dirty until it is.
    pub fn save(&mut self, cx: &mut Context<Self>) -> Task<bool> {
        self.save_making_folder(false, cx)
    }

    /// [`Self::save`], making the file's folder first when `make_folder` (a new file).
    pub fn save_making_folder(&mut self, make_folder: bool, cx: &mut Context<Self>) -> Task<bool> {
        if self.class_file.is_some() || self.loading {
            return Task::ready(!self.dirty);
        }
        let text = self.state.read(cx).value().to_string();
        let (host, path, data) = (crate::host::host(cx), self.path.clone(), text.clone());
        let write = cx.background_executor().spawn(async move {
            if make_folder && let Some(dir) = path.parent() {
                host.create_dir_all(dir)?;
            }
            host.write(&path, data.as_bytes())?;
            Ok::<_, std::io::Error>(disk_stamp(&*host, &path))
        });
        let path = self.path.clone();
        cx.spawn(async move |this, cx| {
            let written = write.await;
            this.update(cx, |this, cx| {
                if this.path != path {
                    return false;
                }
                let ok = written.is_ok();
                this.saved_as(text, written, cx);
                ok
            })
            .unwrap_or(false)
        })
    }

    /// A write of `text` ended as `written` says.
    fn saved_as(&mut self, text: String, written: std::io::Result<Option<(SystemTime, u64)>>, cx: &mut Context<Self>) {
        match written {
            Ok(disk) => {
                self.disk = disk;
                self.on_disk = true;
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
                // Typed on while it was written: still dirty.
                let now = self.state.read(cx).text();
                self.dirty = now.len() != text.len() || *now != text.as_str();
                self.saved = text;
                self.refresh_runnables(cx);
                self.refresh_spring(cx);
                if let Some(spring) = crate::spring::spring(cx) {
                    let path = self.path.clone();
                    spring.update(cx, |s, cx| s.file_saved(&path, cx));
                }
                self.preview = false;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Could not save {}: {e}", self.path.display())),
        }
        cx.notify();
    }

    /// When the file was last read or written (its modification time and length).
    pub fn disk_stamp(&self) -> Option<(SystemTime, u64)> {
        self.disk
    }

    /// Whether the file was there when last read or written.
    pub fn on_disk(&self) -> bool {
        self.on_disk
    }

    /// Take `text` from the other view of this file (a split), changing only what
    /// differs, so this view's caret and scroll stay where they are.
    pub fn sync_from(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            self.pending_sync = Some(text.to_owned());
            return;
        }
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

    /// The other view of this file saved it (the file then had `disk`): this one is saved too.
    pub fn mark_saved(&mut self, disk: Option<(SystemTime, u64)>, cx: &mut Context<Self>) {
        self.saved = self.text(cx);
        self.disk = disk;
        self.on_disk = true;
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
    /// Put the caret on `line` and `column` (from 0) and the view's top `scroll` pixels down,
    /// as a saved session had them, without taking the keyboard.
    pub fn restore_view(&mut self, line: u32, column: u32, scroll: f32, cx: &mut Context<Self>) {
        // The view scrolls once it has been laid out (see `render`).
        self.pending_scroll = Some(scroll);
        if self.loading {
            self.pending_caret = Some((Position::new(line, column), false));
            return;
        }
        self.state.update(cx, |s, cx| {
            let offset = s.text().position_to_offset(&Position::new(line, column));
            s.set_selected_range(offset..offset, cx);
        });
        self.caret_seen = self.state.read(cx).cursor();
        // The view scrolls once it has been laid out (see `render`).
        self.pending_scroll = Some(scroll);
        cx.notify();
    }

    pub fn go_to(&mut self, line: u32, column: u32, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            self.pending_caret = Some((Position::new(line, column), true));
            return;
        }
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
        let colors = [p.added, p.modified, p.removed];
        // Under review, the gutter shows the proposed change instead of Git's.
        let changes = self.review_gutter().map_or_else(|| self.changes.clone(), Arc::new);
        let (state, painted) = (self.state.clone(), self.painted.clone());
        // Over Git's marks, the lines Claude changed and nobody has reviewed yet.
        let claude = (self.review.is_none() && self.claude_base.is_some()).then(|| self.claude_hunks.clone());
        let (claude_painted, claude_color) = (self.claude_painted.clone(), p.claude().mark);
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
                let mut drawn = Vec::new();
                if let Some(hunks) = &claude {
                    let runs = claude::hunk_changes(hunks);
                    for ((kind, b), first) in gutter_marks(state.read(cx), &runs.0).into_iter().zip(runs.1) {
                        let wide =
                            Bounds::new(point(b.left() - px(0.5), b.top()), size(b.size.width + px(1.), b.size.height));
                        paint_mark(kind, if kind == LineChangeKind::Deleted { b } else { wide }, claude_color, window);
                        drawn.push((first, b));
                    }
                }
                *claude_painted.borrow_mut() = drawn;
            },
        )
        .absolute()
        .inset_0()
    }
}

/// A Claude mark in the gutter: its change's first line, and where it is drawn.
pub type ClaudeMark = (usize, Bounds<Pixels>);

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
/// What a tab shows of a file, as read from the host off the UI thread.
struct Loaded {
    path: PathBuf,
    text: std::io::Result<String>,
    /// The repository the file is in.
    root: Option<PathBuf>,
    disk: Option<(SystemTime, u64)>,
    /// The Gradle build it belongs to (within `project`), for a file a server may serve.
    build: Option<PathBuf>,
}

impl Loaded {
    fn read(host: &dyn snout_host::Host, path: &Path, project: Option<PathBuf>, served: bool) -> Self {
        let text = host.read_to_string(path);
        let root = path.ancestors().skip(1).find(|d| host.exists(&d.join(".git"))).map(Path::to_owned);
        let disk = disk_stamp(host, path);
        let build = served.then(|| crate::lang::java::gradle_root(host, path, project.as_deref())).flatten();
        Loaded { path: path.to_owned(), text, root, disk, build }
    }
}

fn disk_stamp(host: &dyn snout_host::Host, path: &Path) -> Option<(SystemTime, u64)> {
    let m = host.metadata(path).ok()?;
    Some((m.modified?, m.len))
}

impl gpui_component::dock::BasePanel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, cx: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        let mut info = match &self.class_file {
            Some((uri, root)) => serde_json::json!({ "path": self.path, "classFile": uri.as_str(), "root": root }),
            None => serde_json::json!({ "path": self.path }),
        };
        // Where the caret and the view were, to come back to them.
        let s = self.state.read(cx);
        let at = s.cursor_position();
        info["line"] = at.line.into();
        info["column"] = at.character.into();
        let scroll = self.pending_scroll.unwrap_or(f32::from(-s.scroll_offset().y));
        info["scroll"] = scroll.max(0.).round().into();
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

/// Where the caret (line and column, from 0) and the view's top (in pixels) were in an
/// editor tab of a saved layout.
pub fn saved_view(state: &PanelState) -> Option<(u32, u32, f32)> {
    match &state.info {
        PanelInfo::Panel(v) => {
            let n = |k: &str| v.get(k).and_then(|n| n.as_u64()).and_then(|n| u32::try_from(n).ok());
            let scroll = v.get("scroll").and_then(|s| s.as_f64()).unwrap_or(0.) as f32;
            Some((n("line")?, n("column").unwrap_or(0), scroll.max(0.)))
        }
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
        if self.state.read(cx).line_height().is_some()
            && !self.loading
            && let Some(scroll) = self.pending_scroll.take()
        {
            self.state.update(cx, |s, cx| s.set_scroll_offset(gpui::point(px(0.), px(-scroll)), cx));
        }
        let p = palette(cx);
        let _ = &self.focus;
        let reviewing = self.review.is_some();
        let has_lang = self.lang.is_some();
        // What Run and Debug in the menu start: what the caret is in.
        let run_label = self.runnables.as_ref().and_then(|r| {
            let line = self.state.read(cx).cursor_position().line;
            r.at_line(line).map(crate::run::Target::menu_label)
        });
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
            .map(|d| self.claude_actions(d, cx))
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save(cx).detach()))
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
                    .on_action(
                        cx.listener(|this, _: &ExtractMethod, w, cx| this.refactor(Refactoring::ExtractMethod, w, cx)),
                    )
                    .on_action(cx.listener(|this, _: &ExtractVariable, w, cx| {
                        this.refactor(Refactoring::ExtractVariable, w, cx)
                    }))
                    .on_action(cx.listener(|this, _: &ExtractConstant, w, cx| {
                        this.refactor(Refactoring::ExtractConstant, w, cx)
                    }))
                    .on_action(
                        cx.listener(|this, _: &ExtractField, w, cx| this.refactor(Refactoring::ExtractField, w, cx)),
                    )
                    .on_action(cx.listener(|this, _: &Inline, w, cx| this.refactor(Refactoring::Inline, w, cx)))
            })
            .when_some(self.error.clone(), |d, e| d.child(div().px(px(12.)).py(px(4.)).text_color(p.error).child(e)))
            .when(self.conflict, |d| d.child(self.conflict_bar(cx)))
            .children(self.review_bar(cx))
            .children(self.claude_bar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .font_family(look::mono_font(cx))
                    .id("editor-body")
                    .on_mouse_move(
                        cx.listener(|this, e: &gpui::MouseMoveEvent, _, cx| this.gutter_hover(e.position, cx)),
                    )
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !*hovered && this.gutter_tip.take().is_some() {
                            cx.notify();
                        }
                    }))
                    .capture_any_mouse_down(cx.listener(|this, e: &gpui::MouseDownEvent, window, cx| {
                        if this.gutter_press(e, window, cx) || this.command_click(e, window, cx) {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }))
                    // Laid out once the file is read, so the first layout is of its text (the
                    // caret asked for scrolls into view on it).
                    .when(!self.loading, |d| {
                        d.child(
                            // The gutter's lanes (problems, run icons, Spring marks) take a
                            // strip left of the line numbers.
                            Editor::new(&self.state)
                                .appearance(false)
                                .pl(px(self.gutter_pad))
                                .h_full()
                                .text_size(px(font_size(cx)))
                                .context_menu(move |menu, _, _| context_menu(menu, has_lang, run_label.as_deref())),
                        )
                    })
                    .children(self.claude_overlay(cx))
                    .child(self.gutter(cx))
                    .child(self.problem_marks(cx))
                    .child(self.run_gutter(cx))
                    .child(self.spring_gutter(cx))
                    .children(self.spring_tooltip(cx))
                    .children(self.gutter_popup(cx))
                    .children(self.signature_popup(cx))
                    .children(self.inspections(cx))
                    .children(self.rename_box(cx))
                    .children(self.hint_popup(cx))
                    .children(self.choices_popup(cx)),
            )
    }
}

/// The editor's right-click menu: Run and Debug for what the caret is in (`run`, its name),
/// the code's own items, sending the selection to Claude Code, and the clipboard.
fn context_menu(
    menu: gpui_component::native_menu::NativeMenu,
    lang: bool,
    run: Option<&str>,
) -> gpui_component::native_menu::NativeMenu {
    use gpui_component::input as i;
    let menu = match run {
        Some(name) => menu
            .menu(format!("Run '{name}'"), Box::new(crate::menus::RunContext))
            .menu(format!("Debug '{name}'"), Box::new(crate::menus::DebugContext))
            .separator(),
        None => menu,
    };
    let menu = if lang {
        let refactor = gpui_component::native_menu::NativeMenu::new()
            .menu("Rename Symbol…", Box::new(RenameSymbol))
            .separator()
            .menu("Extract Method", Box::new(ExtractMethod))
            .menu("Extract Variable", Box::new(ExtractVariable))
            .menu("Extract Constant", Box::new(ExtractConstant))
            .menu("Extract Field", Box::new(ExtractField))
            .menu("Inline", Box::new(Inline));
        menu.menu("Go to Definition", Box::new(i::GoToDefinition))
            .menu("Quick Fix…", Box::new(i::ToggleCodeActions))
            .submenu("Refactor", refactor)
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
                    let saved = editor.update(cx, |e, cx| e.save(cx));
                    window
                        .spawn(cx, async move |cx| {
                            if saved.await {
                                let _ = cx.update(then);
                            }
                        })
                        .detach();
                }
                1 => then(window, cx),
                _ => {}
            });
        })
        .detach();
}
