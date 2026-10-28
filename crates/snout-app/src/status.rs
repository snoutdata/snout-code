//! The status bar under the docks: what the app last did on the left; on the right, for
//! the editor tab in front, the caret's line and column, how much is selected, the
//! indentation, line endings, encoding and language, then the branch.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window, div, px,
};
use gpui_component::Sizable as _;
use gpui_component::input::RopeExt as _;
use gpui_component::spinner::Spinner;
use snout_core::art::Glyph;

use crate::art::glyph;
use crate::editor::EditorPanel;
use crate::model::Model;
use crate::theme::palette;

pub struct StatusBar {
    model: Entity<Model>,
    editor: Option<WeakEntity<EditorPanel>>,
    _watch: Vec<Subscription>,
}

/// What the status bar says about an editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorStatus {
    pub position: String,
    pub selection: Option<String>,
    pub indent: String,
    pub endings: &'static str,
    pub encoding: &'static str,
    pub language: &'static str,
}

impl EditorStatus {
    pub fn of(editor: &EditorPanel, cx: &gpui::App) -> Self {
        let state = editor.state.read(cx);
        let pos = state.cursor_position();
        let range = state.selected_range();
        let selection = (!range.is_empty()).then(|| {
            let text = state.text();
            let chars = text.offset_to_char_index(range.end) - text.offset_to_char_index(range.start);
            let lines = text.offset_to_point(range.end).row - text.offset_to_point(range.start).row;
            if lines > 0 { format!("{chars} chars, {} lines", lines + 1) } else { format!("{chars} selected") }
        });
        Self {
            position: format!("Ln {}, Col {}", pos.line + 1, pos.character + 1),
            selection,
            indent: editor.info.indent_label(),
            endings: if editor.info.crlf { "CRLF" } else { "LF" },
            encoding: "UTF-8",
            language: crate::icons::FileKind::of(&editor.path).language(),
        }
    }
}

impl StatusBar {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        let watch = vec![cx.observe(&model, |_, _, cx| cx.notify())];
        Self { model, editor: None, _watch: watch }
    }

    /// Follow `editor` (the tab in front), or no editor.
    pub fn set_editor(&mut self, editor: Option<&Entity<EditorPanel>>, cx: &mut Context<Self>) {
        let same = match (&self.editor, editor) {
            (Some(a), Some(b)) => a.entity_id() == b.entity_id(),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self._watch.truncate(1);
        self.editor = editor.map(|e| e.downgrade());
        if let Some(e) = editor {
            let state = e.read(cx).state.clone();
            self._watch.push(cx.observe(e, |_, _, cx| cx.notify()));
            self._watch.push(cx.observe(&state, |_, _, cx| cx.notify()));
        }
        cx.notify();
    }

    pub fn editor(&self) -> Option<Entity<EditorPanel>> {
        self.editor.as_ref().and_then(|e| e.upgrade())
    }

    /// What it says about the editor in front, if one is.
    pub fn editor_status(&self, cx: &gpui::App) -> Option<EditorStatus> {
        self.editor().map(|e| EditorStatus::of(e.read(cx), cx))
    }
}

impl Render for StatusBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let status = self.editor_status(cx);
        let hover_bg = p.button_hover;
        let item = move |id: &'static str, text: SharedString| {
            div()
                .id(id)
                .flex_none()
                .h(px(20.))
                .px(px(6.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .whitespace_nowrap()
                .hover(move |s| s.bg(hover_bg))
                .child(text)
        };
        div()
            .id("status-bar")
            .flex()
            .items_center()
            .gap(px(2.))
            .px(px(10.))
            .h(px(26.))
            .bg(p.ground)
            .border_t_1()
            .border_color(p.border)
            .text_size(px(12.))
            .text_color(p.weak())
            .when(m.job.is_some(), |d| d.child(Spinner::new().xsmall()).child(div().w(px(6.))))
            .child(
                div().text_color(p.fg).whitespace_nowrap().overflow_hidden().text_ellipsis().child(m.message.clone()),
            )
            .child(div().flex_1())
            .when_some(status, |d, s| {
                d.child(item("status-position", s.position.into()))
                    .when_some(s.selection, |d, sel| d.child(item("status-selection", sel.into())))
                    .child(item("status-indent", s.indent.into()))
                    .child(item("status-endings", s.endings.into()))
                    .child(item("status-encoding", s.encoding.into()))
                    .child(item("status-language", s.language.into()))
                    .child(div().w(px(8.)))
            })
            .when_some(m.branch_summary(), |d, s| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(glyph(Glyph::Branch, 14., p.ill_ink))
                        .child(div().whitespace_nowrap().child(s)),
                )
            })
    }
}
