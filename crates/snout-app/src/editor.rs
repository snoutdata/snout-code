//! Files open in GPUI Kit's `Editor` as center tabs, highlighted with tree-sitter: Java,
//! Kotlin and YAML from GPUI Kit, Groovy (Gradle scripts) and Java properties registered
//! here. Above the text, breadcrumbs show where the file sits in the repository. ⌘S saves.
//! A tab remembers its file, so the dock layout brings it back. A tab opened by a single
//! click in the file tree is a preview: the next preview takes its place until it is
//! edited or double-clicked.

use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, actions, div, px,
};
use gpui_component::dock::{PanelInfo, PanelState};
use gpui_component::input::{Editor, EditorState, InputEvent, Position, TabSize};

use crate::look;
use crate::theme::palette;

actions!(editor, [Save]);

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
        let width = [4, 2, 8, 3].into_iter().max_by_key(|&w| (steps[w], w == 4)).filter(|&w| steps[w] > 0).unwrap_or(4);
        Self { tabs: tab_lines > space_lines, width, crlf: text.contains("\r\n") }
    }

    /// What the status bar says about the indentation.
    pub fn indent_label(&self) -> String {
        if self.tabs { format!("Tab Size: {}", self.width) } else { format!("Spaces: {}", self.width) }
    }
}

/// What an editor tab tells the workspace.
pub enum EditorEvent {
    /// It has the keyboard.
    Focused,
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
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for EditorPanel {}

impl EditorPanel {
    /// Open `path` in a new editor.
    pub fn open(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| EditorState::new(window, cx).line_number(true).soft_wrap(false));
        let subscriptions = vec![cx.subscribe(&state, |this, state, e: &InputEvent, cx| match e {
            InputEvent::Change => {
                let dirty = state.read(cx).value().as_ref() != this.saved;
                if dirty {
                    // Editing keeps a preview tab.
                    this.preview = false;
                }
                if dirty != this.dirty {
                    this.dirty = dirty;
                }
                cx.notify();
            }
            InputEvent::Focus => cx.emit(EditorEvent::Focused),
            _ => {}
        })];
        let mut this = Self {
            path: PathBuf::new(),
            state,
            saved: String::new(),
            dirty: false,
            preview: false,
            info: FileInfo::detect(""),
            root: None,
            error: None,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.load(path, window, cx);
        this
    }

    /// Show the file at `path` in this tab instead (a preview tab moving on).
    pub fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let (text, error) = match std::fs::read_to_string(&path) {
            Ok(text) => (text, None),
            Err(e) => (String::new(), Some(format!("Could not read {}: {e}", path.display()))),
        };
        self.info = FileInfo::detect(&text);
        let info = self.info;
        let language = language_for(&path);
        self.state.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_tab_size(TabSize { tab_size: info.width, hard_tabs: info.tabs }, cx);
            s.set_value(text.clone(), window, cx);
        });
        self.root = path.ancestors().skip(1).find(|d| d.join(".git").exists()).map(Path::to_owned);
        self.path = path;
        self.saved = text;
        self.dirty = false;
        self.error = error;
        cx.notify();
    }

    /// Write the text to the file.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        match std::fs::write(&self.path, &text) {
            Ok(()) => {
                self.saved = text;
                self.dirty = false;
                self.preview = false;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Could not save {}: {e}", self.path.display())),
        }
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

    fn breadcrumbs(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let crumbs = self.crumbs();
        let last = crumbs.len().saturating_sub(1);
        div()
            .id("breadcrumbs")
            .flex_none()
            .h(px(26.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(3.))
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(p.fg_dim)
            .children(crumbs.into_iter().enumerate().map(move |(i, name)| {
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(5.))
                    .when(i == last, |d| d.text_color(p.fg).child(crate::icons::file_icon(&self.path, 13., &p)))
                    .child(name)
                    .when(i < last, |d| d.child(look::chevron(false, p.fg_dim.opacity(0.7))))
            }))
    }
}

impl gpui_component::dock::BasePanel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::panel(serde_json::json!({ "path": self.path }));
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

impl Render for EditorPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let _ = &self.focus;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.panel)
            .key_context("EditorPanel")
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save(cx)))
            .child(self.breadcrumbs(cx))
            .when_some(self.error.clone(), |d, e| d.child(div().px(px(12.)).py(px(4.)).text_color(p.error).child(e)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .font_family(look::MONO)
                    .child(Editor::new(&self.state).appearance(false).h_full().text_size(px(look::MONO_SIZE))),
            )
    }
}

/// Close an editor tab (`then` does the closing).
pub fn confirm_close(
    editor: Option<Entity<EditorPanel>>,
    window: &mut Window,
    cx: &mut App,
    then: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    let _ = editor;
    then(window, cx);
}
