//! Small pieces every view draws with: icons, chips, pane headers, empty-pane notes.

use gpui::{App, Div, FontWeight, Hsla, ParentElement, SharedString, Styled, div, px};
use gpui_component::Icon;

use crate::theme::{Palette, mix, palette};

/// An icon from GPUI Kit's set (Lucide), by file name.
pub fn icon(name: &'static str) -> Icon {
    Icon::default().path(SharedString::new_static(name))
}

pub mod icons {
    pub const BRANCH: &str = "icons/git-branch.svg";
    pub const COMMIT: &str = "icons/git-commit-horizontal.svg";
    pub const GRAPH: &str = "icons/git-graph.svg";
    pub const COMPARE: &str = "icons/git-compare.svg";
    pub const REPO: &str = "icons/folder-git-2.svg";
    pub const FOLDER: &str = "icons/folder.svg";
    pub const FOLDERS: &str = "icons/folders.svg";
    pub const ALERT: &str = "icons/circle-alert.svg";
    pub const FETCH: &str = "icons/refresh-cw.svg";
    pub const PULL: &str = "icons/arrow-down-to-line.svg";
    pub const PUSH: &str = "icons/arrow-up-from-line.svg";
    pub const PLUS: &str = "icons/plus.svg";
    pub const SPARK: &str = "icons/sparkles.svg";
    pub const TRASH: &str = "icons/trash.svg";
    pub const CLOSE: &str = "icons/x.svg";
    pub const SWITCH: &str = "icons/git-compare-arrows.svg";
    pub const TAG: &str = "icons/tag.svg";
    pub const OPEN: &str = "icons/folder-open.svg";
    pub const SEARCH: &str = "icons/search.svg";
    pub const WEB: &str = "icons/globe.svg";
    pub const TERMINAL: &str = "icons/square-terminal.svg";
    pub const DISCARD: &str = "icons/undo-2.svg";
    pub const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
    pub const CHEVRON_DOWN: &str = "icons/chevron-down.svg";
    pub const CHEVRON_UP: &str = "icons/chevron-up.svg";
}

/// A small rounded label in a soft wash of `color`.
pub fn chip(text: impl Into<SharedString>, color: Hsla, p: &Palette) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .h(px(17.))
        .flex()
        .items_center()
        .rounded(px(9.))
        .border_1()
        .border_color(mix(color, 0.4, p.panel))
        .bg(p.wash(color))
        .text_color(color)
        .text_size(px(11.5))
        .child(text.into())
}

/// A ref beside a commit: the checked-out branch green, tags amber, the rest dim.
pub fn ref_badge(r: &str, p: &Palette) -> Div {
    let (text, color) = if let Some(branch) = r.strip_prefix("HEAD -> ") {
        (branch, p.ok)
    } else if let Some(tag) = r.strip_prefix("tag: ") {
        (tag, p.warn)
    } else if r == "HEAD" {
        (r, p.ok)
    } else {
        (r, p.fg_dim)
    };
    chip(text.to_owned(), color, p)
}

/// The bold title at the top of a pane section.
pub fn pane_title(text: impl Into<SharedString>) -> Div {
    div().font_weight(FontWeight::SEMIBOLD).child(text.into())
}

/// A centred, dimmed message filling a pane that has nothing to show.
pub fn note(title: impl Into<SharedString>, text: impl Into<SharedString>, cx: &App) -> Div {
    let p = palette(cx);
    let title: SharedString = title.into();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .when_some((!title.is_empty()).then_some(title), |d, t| {
            d.child(div().font_weight(FontWeight::SEMIBOLD).child(t))
        })
        .child(div().text_color(p.fg_dim).child(text.into()))
}

/// Colour of a working-tree file's state.
pub fn state_color(f: &snout_core::FileEntry, p: &Palette) -> Hsla {
    if f.conflicted {
        p.error
    } else if f.is_untracked() {
        p.fg_dim
    } else if f.has_staged() && !f.has_unstaged() {
        p.ok
    } else if f.has_staged() {
        p.warn
    } else {
        p.accent
    }
}

/// Colour of a change in a commit.
pub fn change_color(change: snout_core::Change, p: &Palette) -> Hsla {
    use snout_core::Change;
    match change {
        Change::Added | Change::Copied => p.ok,
        Change::Deleted => p.error,
        Change::Renamed => p.warn,
        _ => p.fg,
    }
}

use gpui::prelude::FluentBuilder as _;
