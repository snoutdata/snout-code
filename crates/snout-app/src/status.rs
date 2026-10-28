//! The status bar across the foot of the window. Left: where the file in front sits
//! (project › folders › file), or what the app last did; in the Git view, the branch.
//! Right, for the editor tab in front: the caret's line and column, how much is selected,
//! the line endings, encoding and indentation, what the Java engine is doing, and a lock
//! on a file that cannot be edited.

use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window, div, px,
};
use gpui_component::Sizable as _;
use gpui_component::input::RopeExt as _;
use gpui_component::spinner::Spinner;

use crate::editor::EditorPanel;
use crate::glyphs::{Icon, icon};
use crate::model::Model;
use crate::theme::palette;

pub struct StatusBar {
    model: Entity<Model>,
    editor: Option<WeakEntity<EditorPanel>>,
    /// The Java engine is not there: Java files get no language features.
    pub java_missing: bool,
    /// The note saying why is open.
    pub java_note: bool,
    /// The language servers, for what Java is doing.
    hub: Option<Entity<crate::lang::LangHub>>,
    /// The Git view shows: the branch takes the breadcrumbs' place.
    pub git: bool,
    _hub_watch: Option<Subscription>,
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
    /// The file cannot be edited (a library's class file).
    pub read_only: bool,
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
            position: format!("{}:{}", pos.line + 1, pos.character + 1),
            selection,
            indent: editor.info.indent_label(),
            endings: if editor.info.crlf { "CRLF" } else { "LF" },
            encoding: "UTF-8",
            language: crate::icons::FileKind::of(&editor.path).language(),
            read_only: editor.class_file.is_some(),
        }
    }
}

impl StatusBar {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        let watch = vec![cx.observe(&model, |_, _, cx| cx.notify())];
        let java_missing = crate::lang::java::engine_dist().is_none();
        Self {
            model,
            editor: None,
            java_missing,
            java_note: false,
            hub: None,
            git: false,
            _hub_watch: None,
            _watch: watch,
        }
    }

    /// Show what the language servers are doing.
    pub fn set_hub(&mut self, hub: Entity<crate::lang::LangHub>, cx: &mut Context<Self>) {
        self._hub_watch = Some(cx.observe(&hub, |_, _, cx| cx.notify()));
        self.hub = Some(hub);
    }

    /// What the Java server of the editor in front (or of the project) is doing, and
    /// whether it is busy.
    pub fn java_status(&self, cx: &gpui::App) -> Option<(String, bool)> {
        let hub = self.hub.as_ref()?.read(cx);
        let path = self.editor().map(|e| {
            let e = e.read(cx);
            e.class_file.as_ref().map(|(_, root)| root.clone()).unwrap_or_else(|| e.path.clone())
        });
        let path = path.or_else(|| self.model.read(cx).repo.as_ref().map(|r| r.root().to_owned()));
        hub.server_for(path.as_deref()).map(|s| (s.status(), s.busy()))
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

    /// Whether to say Java support is missing: the engine is not there, and a Java file is
    /// in front or the project is a Gradle build.
    pub fn shows_java_missing(&self, cx: &gpui::App) -> bool {
        if !self.java_missing {
            return false;
        }
        let java_file = self.editor().is_some_and(|e| crate::editor::language_for(&e.read(cx).path) == "java");
        let gradle = self.model.read(cx).repo.as_ref().is_some_and(|r| crate::lang::java::is_gradle_root(r.root()));
        java_file || gradle
    }

    /// What it says about the editor in front, if one is.
    pub fn editor_status(&self, cx: &gpui::App) -> Option<EditorStatus> {
        self.editor().map(|e| EditorStatus::of(e.read(cx), cx))
    }
}

impl StatusBar {
    /// Where the file in front sits: the project, its folders, the file with its icon.
    fn breadcrumbs(&self, cx: &gpui::App) -> Option<gpui::AnyElement> {
        let editor = self.editor()?;
        let e = editor.read(cx);
        let p = palette(cx);
        let crumbs = e.crumbs();
        let last = crumbs.len().saturating_sub(1);
        let path = e.path.clone();
        Some(
            div()
                .id("breadcrumbs")
                .debug_selector(|| "breadcrumbs".into())
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(3.))
                .overflow_hidden()
                .children(crumbs.into_iter().enumerate().map(move |(i, name)| {
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(4.))
                        .when(i == last, |d| d.text_color(p.fg).child(crate::icons::file_icon(&path, 12., &p)))
                        .when(i == 0, |d| d.child(icon(Icon::Folder, 12., p.fg_dim)))
                        .child(name)
                        .when(i < last, |d| d.child(icon(Icon::ChevronRight, 10., p.fg_dim)))
                }))
                .into_any_element(),
        )
    }
}

impl Render for StatusBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let java_item = self.shows_java_missing(cx).then(|| self.java_missing_item(cx));
        let java = self.java_status(cx);
        let crumbs = if self.git { None } else { self.breadcrumbs(cx) };
        let status = if self.git { None } else { self.editor_status(cx) };
        let m = self.model.read(cx);
        let busy = m.job.is_some();
        let hover_bg = p.button_hover;
        let item = move |id: &'static str, text: SharedString| {
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
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
        // Left: what runs (with its spinner), else where the file in front sits, else
        // what the app last did. In the Git view, the branch.
        let left = if busy || crumbs.is_none() {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .when(busy, |d| d.child(Spinner::new().xsmall()))
                .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(m.message.clone()))
                .when_some(m.branch_summary().filter(|_| self.git), |d, s| {
                    d.child(div().w(px(10.)))
                        .child(icon(Icon::Branch, 13., p.fg_dim))
                        .child(div().whitespace_nowrap().child(s))
                })
                .into_any_element()
        } else {
            crumbs.unwrap_or_else(|| div().into_any_element())
        };
        div()
            .id("status-bar")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .pl(px(10.))
            .pr(px(8.))
            .h(px(26.))
            .bg(p.ground)
            .border_t_1()
            .border_color(p.border)
            .text_size(px(12.))
            .text_color(p.fg_dim)
            .child(div().flex_1().min_w_0().child(left))
            .when_some(status, |d, s| {
                d.child(item("status-position", s.position.into()))
                    .when_some(s.selection, |d, sel| d.child(item("status-selection", sel.into())))
                    .child(item("status-endings", s.endings.into()))
                    .child(item("status-encoding", s.encoding.into()))
                    .child(item("status-indent", s.indent.into()))
            })
            .children(java_item)
            .when_some(java, |d, (text, busy)| {
                d.child(
                    div()
                        .id("status-java")
                        .debug_selector(|| "status-java".into())
                        .flex_none()
                        .h(px(20.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(4.))
                        .whitespace_nowrap()
                        .hover(move |s| s.bg(hover_bg))
                        .when(busy, |d| d.child(Spinner::new().xsmall()))
                        .child(text)
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::menus::ShowProblems), cx)),
                )
            })
            .when(self.editor_status(cx).is_some_and(|s| s.read_only) && !self.git, |d| {
                d.child(
                    div()
                        .id("status-lock")
                        .debug_selector(|| "status-lock".into())
                        .px(px(4.))
                        .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Read only").build(w, cx))
                        .child(icon(Icon::Lock, 13., p.fg_dim)),
                )
            })
    }
}

/// Why Java support is missing, and how to get it.
pub const JAVA_MISSING_NOTE: &str = "Java support needs SnoutCode's Java engine, which is not built. \
Run ./snout.sh again: it builds the engine with JDK 21, and downloads one when none is installed \
(or set SNOUT_JDK21 to a JDK 21 folder). Then restart SnoutCode.";

impl StatusBar {
    /// The quiet "Java support unavailable" item; a click opens the note saying why.
    fn java_missing_item(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = palette(cx);
        let hover_bg = p.button_hover;
        div()
            .relative()
            .flex_none()
            .child(
                div()
                    .id("status-java-missing")
                    .debug_selector(|| "status-java-missing".into())
                    .h(px(20.))
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .text_color(p.fg_dim)
                    .hover(move |s| s.bg(hover_bg))
                    .child("Java support unavailable")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.java_note = !this.java_note;
                        cx.notify();
                    })),
            )
            .when(self.java_note, |d| {
                d.child(
                    gpui::deferred(
                        gpui::anchored().anchor(gpui::Anchor::BottomRight).child(
                            div()
                                .id("java-missing-note")
                                .debug_selector(|| "java-missing-note".into())
                                .mb(px(26.))
                                .w(px(360.))
                                .p(px(12.))
                                .rounded(px(8.))
                                .bg(p.card)
                                .border_1()
                                .border_color(p.border)
                                .shadow_lg()
                                .text_size(px(12.5))
                                .text_color(p.fg)
                                .child(JAVA_MISSING_NOTE)
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.java_note = false;
                                    cx.notify();
                                })),
                        ),
                    )
                    .with_priority(1),
                )
            })
            .into_any_element()
    }
}
