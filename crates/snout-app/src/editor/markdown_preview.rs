//! A Markdown file's preview: the text rendered as it reads (headings, lists, tables, code,
//! images), beside the text or in its place, kept up to date as it is typed. ⇧⌘V shows or
//! hides it; three buttons at the top right choose the text, both side by side, or the preview.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::EditorPanel;
use crate::components::IconName;
use crate::theme::{mix, palette};

/// What a Markdown tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MdView {
    /// The text alone.
    #[default]
    Editor,
    /// The text, and the preview beside it.
    Split,
    /// The preview alone.
    Preview,
}

/// Whether the file at `path` is Markdown.
pub fn is_markdown(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "mdown" | "mkd"))
}

impl EditorPanel {
    /// ⇧⌘V: the preview in place of the text, or the text again.
    pub fn toggle_markdown_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !is_markdown(&self.path) {
            return self.show_hint(ts!("editor-markdown-preview-only-for-markdown"), cx);
        }
        let next = if self.md_view == MdView::Preview { MdView::Editor } else { MdView::Preview };
        self.set_md_view(next, window, cx);
    }

    /// Show the text, both, or the preview; the keyboard goes to what is shown.
    pub fn set_md_view(&mut self, view: MdView, window: &mut Window, cx: &mut Context<Self>) {
        self.md_view = view;
        if view == MdView::Preview {
            self.focus.focus(window, cx);
        } else {
            self.state.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    /// The Markdown view this tab shows (always the text for any other file).
    pub fn md_view(&self) -> MdView {
        if is_markdown(&self.path) { self.md_view } else { MdView::Editor }
    }

    /// Where the keyboard goes when the tab is chosen: the preview when it shows alone.
    pub(super) fn md_focus(&self) -> Option<gpui::FocusHandle> {
        (self.md_view() == MdView::Preview).then(|| self.focus.clone())
    }

    /// The text to render: the editor's, kept as one string while it stays the same, so the
    /// preview parses again only when the text changed.
    fn md_source(&mut self, cx: &App) -> SharedString {
        let rope = self.state.read(cx).text();
        if *rope != self.md_text.as_ref() {
            self.md_text = rope.to_string().into();
        }
        self.md_text.clone()
    }

    /// The text the preview last rendered.
    pub fn md_source_now(&self) -> SharedString {
        self.md_text.clone()
    }

    /// The tab's body: `text` (the editor and its overlays), the preview beside it, or the
    /// preview alone, with the buttons that choose.
    pub(super) fn with_markdown_preview(&mut self, text: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let view = self.md_view();
        if !is_markdown(&self.path) {
            return text;
        }
        let p = palette(cx);
        let preview = (view != MdView::Editor).then(|| self.preview_pane(cx));
        let button = |id: &'static str, ico: IconName, tip: &'static str, this: MdView| {
            let tip = SharedString::from(tip);
            let on = view == this;
            crate::look::icon_toggle(id, ico, on, &p)
                .debug_selector(move || id.to_owned())
                .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
                .on_click(cx.listener(move |e, _, window, cx| e.set_md_view(this, window, cx)))
        };
        let switch = div()
            .absolute()
            .top(px(6.))
            .right(px(18.))
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(7.))
            .bg(p.card)
            .border_1()
            .border_color(mix(p.border, 0.8, p.card))
            .shadow_sm()
            .child(button("md-view-editor", IconName::FileCode, ts!("editor-markdown-editor-only"), MdView::Editor))
            .child(button(
                "md-view-split",
                IconName::Columns2,
                ts!("editor-markdown-editor-and-preview"),
                MdView::Split,
            ))
            .child(button("md-view-preview", IconName::Eye, ts!("editor-markdown-preview-only"), MdView::Preview));
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .flex()
            .flex_row()
            .when(view != MdView::Preview, |d| d.child(div().flex_1().min_w_0().h_full().flex().flex_col().child(text)))
            .when(view == MdView::Split, |d| d.child(div().flex_none().w(px(1.)).h_full().bg(p.border)))
            .children(preview)
            .child(switch)
            .into_any_element()
    }

    /// The rendered text.
    fn preview_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let source = self.md_source(cx);
        // Images by relative paths are the file's neighbours.
        let dir = self.path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        let images = move |uri: &gpui::SharedUri| -> gpui::ImageSource {
            let s = uri.as_ref();
            if s.contains("://") || s.starts_with("data:") {
                return uri.clone().into();
            }
            let path = std::path::Path::new(s.split(['?', '#']).next().unwrap_or(s));
            let path = if path.is_absolute() { path.to_path_buf() } else { dir.join(path) };
            gpui::ImageSource::from(path)
        };
        div()
            .id("markdown-preview")
            .debug_selector(|| "markdown-preview".into())
            .track_focus(&self.focus)
            .flex_1()
            .min_w_0()
            .h_full()
            .pl(px(24.))
            .pr(px(8.))
            .pt(px(10.))
            .bg(p.panel)
            .text_color(p.fg)
            .font_family(crate::look::ui_font(cx))
            .text_size(px(crate::look::BODY + 1.))
            .child(
                gpui_component::text::TextView::markdown(("markdown-preview-text", cx.entity_id()), source)
                    .selectable(true)
                    .scrollable(true)
                    .image_source(images)
                    .size_full(),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_files_are_known_by_their_extension() {
        assert!(is_markdown(std::path::Path::new("/p/README.md")));
        assert!(is_markdown(std::path::Path::new("/p/notes.MARKDOWN")));
        assert!(!is_markdown(std::path::Path::new("/p/A.java")));
        assert!(!is_markdown(std::path::Path::new("/p/md")));
    }
}
