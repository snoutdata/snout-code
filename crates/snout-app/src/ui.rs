//! Small pieces the views share: chips and the colours of file states and changes.

use gpui::{Div, Hsla, ParentElement, SharedString, Styled, div, px};

use crate::theme::{Palette, mix};

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

/// The pill of a kind of change.
pub fn change_pill(change: snout_core::Change) -> crate::components::StatePill {
    use crate::components::StatePill as S;
    use snout_core::Change;
    match change {
        Change::Modified => S::Modified,
        Change::TypeChanged => S::TypeChanged,
        Change::Added => S::Added,
        Change::Deleted => S::Deleted,
        Change::Renamed => S::Renamed,
        Change::Copied => S::Copied,
        Change::Untracked => S::Untracked,
        Change::Conflicted => S::Conflicted,
    }
}

/// A working-tree file's state as pills: what changed, and whether it is staged.
pub fn state_pills(f: &snout_core::FileEntry) -> Vec<crate::components::StatePill> {
    use crate::components::StatePill as S;
    if f.conflicted {
        return vec![S::Conflicted];
    }
    if f.is_untracked() {
        return vec![S::Untracked];
    }
    match (f.index, f.worktree) {
        (Some(i), None) => vec![change_pill(i), S::Staged],
        (Some(i), Some(_)) => vec![change_pill(i), S::PartlyStaged],
        (None, Some(w)) => vec![change_pill(w)],
        (None, None) => vec![],
    }
}
