//! A folder of a remote machine, picked by browsing it through its host (the Mac's own
//! folder dialog cannot see it): a path to type, the folders of the one shown, up and in.

use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Task, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use snout_host::HostRef;

use super::dialog::{button, footer, frame};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::theme::palette;

pub enum PickerEvent {
    Open(PathBuf),
    Cancel,
}

pub struct FolderPicker {
    host: HostRef,
    /// The machine's name, for the title.
    name: String,
    /// What the dialog is for: "Open".
    verb: &'static str,
    pub input: Entity<InputState>,
    /// The folder shown, and its folders (hidden ones last), once read.
    pub dir: PathBuf,
    pub folders: Option<Vec<String>>,
    pub selected: Option<String>,
    pub error: Option<String>,
    show_hidden: bool,
    _list: Option<Task<()>>,
    focus: FocusHandle,
}

impl EventEmitter<PickerEvent> for FolderPicker {}

impl Focusable for FolderPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FolderPicker {
    pub fn new(host: HostRef, name: String, start: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(start.to_string_lossy().into_owned()));
        cx.subscribe_in(&input, window, |this, input, e: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = e {
                let typed = PathBuf::from(input.read(cx).value().trim());
                this.go(typed, window, cx);
            }
        })
        .detach();
        let mut this = Self {
            host,
            name,
            verb: "Open",
            input,
            dir: start.clone(),
            folders: None,
            selected: None,
            error: None,
            show_hidden: false,
            _list: None,
            focus: cx.focus_handle(),
        };
        this.list(start, cx);
        this
    }

    /// Show `dir`, read off the UI thread.
    fn list(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        self.dir = dir.clone();
        self.folders = None;
        self.selected = None;
        self.error = None;
        let host = self.host.clone();
        let read = cx.background_executor().spawn(async move {
            let entries = host.read_dir(&dir)?;
            let mut names: Vec<String> = entries
                .into_iter()
                .filter(|e| e.is_dir)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort_by_key(|n| (n.starts_with('.'), n.to_lowercase()));
            Ok::<_, std::io::Error>(names)
        });
        self._list = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(names) => this.folders = Some(names),
                    Err(e) => {
                        this.folders = Some(Vec::new());
                        this.error = Some(format!("{}: {e}", this.dir.display()));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Open `dir` (as Open does with the folder chosen).
    pub fn open(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::Open(dir));
    }

    /// Go to `dir`, and show its path in the field.
    pub fn go(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let text = dir.to_string_lossy().into_owned();
        self.input.update(cx, |i, cx| i.set_value(text, window, cx));
        self.list(dir, cx);
    }

    fn up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parent) = self.dir.parent().map(Path::to_path_buf) {
            self.go(parent, window, cx);
        }
    }

    /// The folder Open opens: the one selected, else the one shown.
    pub fn chosen(&self) -> PathBuf {
        match &self.selected {
            Some(name) => self.dir.join(name),
            None => self.dir.clone(),
        }
    }

    fn click(&mut self, name: String, e: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if e.click_count() >= 2 {
            let dir = self.dir.join(&name);
            self.go(dir, window, cx);
        } else {
            self.selected = Some(name);
            cx.notify();
        }
    }

    fn rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = palette(cx);
        let hover = p.button_hover;
        let Some(folders) = &self.folders else { return Vec::new() };
        folders
            .iter()
            .filter(|n| self.show_hidden || !n.starts_with('.'))
            .enumerate()
            .map(|(i, name)| {
                let on = self.selected.as_deref() == Some(name.as_str());
                let id = format!("remote-folder-{i}");
                let sel = id.clone();
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .h(px(26.))
                    .px(px(10.))
                    .mx(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.selection()))
                    .when(!on, |d| d.hover(move |s| s.bg(hover)))
                    .child(icon(Icon::Folder, 15., p.fg_dim))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(name.clone()),
                    )
                    .on_click(cx.listener({
                        let name = name.clone();
                        move |this, e: &ClickEvent, window, cx| this.click(name.clone(), e, window, cx)
                    }))
                    .into_any_element()
            })
            .collect()
    }
}

impl Render for FolderPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let rows = self.rows(cx);
        let loading = self.folders.is_none();
        let empty = !loading && rows.is_empty() && self.error.is_none();
        let hover = p.button_hover;
        let status = match &self.error {
            Some(e) => Some((e.clone(), p.error)),
            None => Some((self.chosen().to_string_lossy().into_owned(), p.fg_dim)),
        };
        let has_parent = self.dir.parent().is_some();
        frame("remote-picker", format!("{} a Folder on {}", self.verb, self.name), 560., 480., cx)
            .key_context("RemotePicker")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(PickerEvent::Cancel)))
            .child(
                div()
                    .flex_none()
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("remote-picker-up")
                            .debug_selector(|| "remote-picker-up".into())
                            .flex_none()
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.))
                            .when(has_parent, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
                            .when(!has_parent, |d| d.opacity(0.4))
                            .child(icon(Icon::ChevronUp, 14., p.fg))
                            .on_click(cx.listener(|this, _, window, cx| this.up(window, cx))),
                    )
                    .child(div().flex_1().min_w_0().child(look::field(&self.input, false, None, &p))),
            )
            .child(
                div()
                    .id("remote-picker-folders")
                    .flex_1()
                    .min_h_0()
                    .pb(px(6.))
                    .overflow_y_scrollbar()
                    .children(rows)
                    .when(loading, |d| d.child(div().px(px(16.)).py(px(10.)).text_color(p.fg_dim).child("Reading…")))
                    .when(empty, |d| {
                        d.child(div().px(px(16.)).py(px(10.)).text_color(p.fg_dim).child("No folders here"))
                    }),
            )
            .child(div().flex_none().px(px(16.)).pb(px(8.)).child(
                look::checkbox("remote-picker-hidden", "Show hidden folders", self.show_hidden, true, &p).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.show_hidden = !this.show_hidden;
                        cx.notify();
                    }),
                ),
            ))
            .child(footer(
                status,
                vec![
                    button("remote-picker-cancel", "Cancel", false, true, cx)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Cancel)))
                        .into_any_element(),
                    button("remote-picker-open", "Open", true, self.error.is_none(), cx)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.error.is_none() {
                                cx.emit(PickerEvent::Open(this.chosen()))
                            }
                        }))
                        .into_any_element(),
                ],
                cx,
            ))
    }
}
