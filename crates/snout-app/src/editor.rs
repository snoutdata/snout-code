//! Files open in GPUI Kit's `Editor` as center tabs, highlighted with tree-sitter: Java,
//! Kotlin and YAML from GPUI Kit, Groovy (Gradle scripts) and Java properties registered
//! here. ⌘S saves. A tab remembers its file, so the dock layout brings it back.

use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Window, actions, div, px,
};
use gpui_component::dock::{PanelInfo, PanelState};
use gpui_component::input::{Editor, EditorState, InputEvent, Position};

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

pub struct EditorPanel {
    pub path: PathBuf,
    pub state: Entity<EditorState>,
    /// The text as it is on disk.
    saved: String,
    pub dirty: bool,
    error: Option<String>,
    focus: FocusHandle,
}

impl EditorPanel {
    /// Open `path` in a new editor.
    pub fn open(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (text, error) = match std::fs::read_to_string(&path) {
            Ok(text) => (text, None),
            Err(e) => (String::new(), Some(format!("Could not read {}: {e}", path.display()))),
        };
        let language = language_for(&path);
        let state = cx.new(|cx| {
            let mut s = EditorState::new(window, cx).language(language).line_number(true);
            s.set_value(text.clone(), window, cx);
            s
        });
        cx.subscribe(&state, |this, state, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let dirty = state.read(cx).value().as_ref() != this.saved;
                if dirty != this.dirty {
                    this.dirty = dirty;
                    cx.notify();
                }
            }
        })
        .detach();
        Self { path, state, saved: text, dirty: false, error, focus: cx.focus_handle() }
    }

    /// Write the text to the file.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        match std::fs::write(&self.path, &text) {
            Ok(()) => {
                self.saved = text;
                self.dirty = false;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Could not save {}: {e}", self.path.display())),
        }
        cx.notify();
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

    fn label(&self) -> SharedString {
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if self.dirty { format!("{name} ●").into() } else { name.into() }
    }
}

impl gpui::EventEmitter<gpui_component::dock::PanelEvent> for EditorPanel {}

impl gpui::Focusable for EditorPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle(cx)
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

impl gpui_component::dock::Panel for EditorPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.label()
    }

    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.label())
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
            .when_some(self.error.clone(), |d, e| d.child(div().px(px(8.)).py(px(4.)).text_color(p.error).child(e)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .font_family(look::MONO)
                    .child(Editor::new(&self.state).h_full().text_size(px(look::MONO_SIZE))),
            )
    }
}
