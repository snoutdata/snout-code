//! A changed file's diff in a window of its own, opened by a double click in the Files table:
//! both versions whole (unless Compact is on), side by side or unified, with what to compare,
//! the changes one after another (F7, ⇧F7), Stage or Unstage, and the file in the editor. It
//! moves, maximizes and goes full screen as any window does, apart from the main one. One
//! such window: a double click on another file shows that file in it. Esc closes it.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Bounds, ClickEvent, Context, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, WeakEntity, Window, WindowBounds, WindowOptions, div, px, size,
};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::tooltip::Tooltip;
use snout_core::diff_layout::{DiffLayout, SbsLine, UniLine};
use snout_core::{DiffBase, FileEntry, RowKind, stage_paths, unstage_paths};

use super::{GitView, diff_body, diff_title};
use crate::components::IconName;
use crate::look::{self, Trigger};
use crate::model::Action;
use crate::theme::{mix, palette};

/// The diff list's id, for its scroll, focus and selection.
pub(super) const LIST: &str = "window-diff";

/// The window's size the first time, its least, and where its size is kept (`dialog_sizes`).
const DEFAULT_SIZE: (f32, f32) = (1400., 880.);
const LEAST: (f32, f32) = (640., 400.);
const SIZE_KEY: &str = "git-diff-window";

gpui::actions!(git, [NextChange, PreviousChange]);

/// The file the window shows, how, and its diff.
pub struct DiffWindow {
    /// The file as the status last listed it.
    pub entry: FileEntry,
    /// What is compared: HEAD and the working tree, say.
    pub base: DiffBase,
    /// Unchanged lines away from the changes left out. Off: the whole files.
    pub compact: bool,
    pub diff: Option<Result<Rc<DiffLayout>, String>>,
    /// The change last gone to, by its place among the changes.
    pub change: Option<usize>,
    /// Go to the first change once the diff is in.
    first: bool,
    /// The repository read the diff is of ([`crate::model::Model::snapshots`]).
    seen: u64,
    pub focus: FocusHandle,
    /// Its window, once open; and the main window, to bring forward for Open in Editor.
    pub handle: Option<AnyWindowHandle>,
    main: AnyWindowHandle,
    _read: Option<Task<()>>,
}

/// The window's root: what the Git view draws for it, drawn again as the view changes.
pub struct DiffWindowView {
    /// Weak: the window lives no longer than the project window's Git view.
    git: WeakEntity<GitView>,
    title: String,
    _subscriptions: Vec<Subscription>,
}

impl DiffWindowView {
    fn new(git: Entity<GitView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let model = git.read(cx).model.clone();
        let subscriptions = vec![
            cx.observe(&git, |_, _, cx| cx.notify()),
            cx.observe(&model, |_, _, cx| cx.notify()),
            // The project window closed, and its Git view with it: so does this window.
            cx.observe_release_in(&git, window, |_, _, window, _| window.remove_window()),
            // Its size is kept for next time, as it was last left unmaximized.
            cx.observe_window_bounds(window, move |this, window, cx| {
                if window.is_maximized() || window.is_fullscreen() {
                    return;
                }
                let b = window.bounds().size;
                let Some(git) = this.git.upgrade() else { return };
                let model = git.read(cx).model.clone();
                model.update(cx, |m, _| {
                    m.settings.dialog_sizes.insert(SIZE_KEY.into(), [f32::from(b.width), f32::from(b.height)]);
                });
            }),
            // Closed (Esc, or the window's own button): the view forgets it.
            cx.on_release({
                let git = git.downgrade();
                move |_, cx| {
                    let _ = git.update(cx, |g, cx| {
                        g.diff_window = None;
                        g.model.update(cx, |m, _| m.persist());
                        cx.notify();
                    });
                }
            }),
        ];
        Self { git: git.downgrade(), title: String::new(), _subscriptions: subscriptions }
    }
}

impl Render for DiffWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let Some(git) = self.git.upgrade() else { return div().size_full().bg(p.panel) };
        let title = git.read(cx).diff_window.as_ref().map(|w| {
            let name = w.entry.path.rsplit('/').next().unwrap_or(&w.entry.path).to_owned();
            format!("{name} ({})", crate::core_text::diff_base(w.base))
        });
        if let Some(title) = title.filter(|t| *t != self.title) {
            window.set_window_title(&title);
            self.title = title;
        }
        let body = git.update(cx, |g, cx| g.diff_window_view(cx));
        div().size_full().bg(p.panel).children(body)
    }
}

/// Where each change starts, as screen lines of `layout` in the chosen view.
pub fn change_starts(layout: &DiffLayout, side_by_side: bool) -> Vec<usize> {
    let rows = &layout.diff.rows;
    let changed = |i: usize| rows.get(i).is_some_and(|r| r.kind != RowKind::Equal);
    let flags: Vec<bool> = if side_by_side {
        layout.sbs.iter().map(|l| matches!(l, SbsLine::Row(i) if changed(*i))).collect()
    } else {
        layout.unified.iter().map(|l| matches!(l, UniLine::Old(_) | UniLine::New(_))).collect()
    };
    (0..flags.len()).filter(|&i| flags[i] && (i == 0 || !flags[i - 1])).collect()
}

/// The new file's line (from 0) at screen line `at` of `layout`, or the nearest before it.
fn new_line_at(layout: &DiffLayout, side_by_side: bool, at: usize) -> Option<u32> {
    let rows = &layout.diff.rows;
    let row_of = |k: usize| -> Option<usize> {
        if side_by_side {
            match layout.sbs.get(k)? {
                SbsLine::Row(i) => Some(*i),
                SbsLine::Gap(_) => None,
            }
        } else {
            match layout.unified.get(k)? {
                UniLine::Both(i) | UniLine::Old(i) | UniLine::New(i) => Some(*i),
                UniLine::Gap(_) => None,
            }
        }
    };
    (0..=at).rev().filter_map(row_of).find_map(|i| rows[i].new.as_ref().map(|l| l.0.saturating_sub(1)))
}

impl GitView {
    /// Open the window on the file at `path` (relative to the repository), as the Files
    /// table's double click does.
    pub fn open_diff_window(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let Some(entry) = m.status.files.iter().find(|f| f.path == path).cloned() else { return };
        let base = m.settings.diff_base;
        let kept = m.settings.dialog_sizes.get(SIZE_KEY).map(|[w, h]| (*w, *h));
        // Its window when it is open already: the file shows there, the window in front.
        let (handle, focus) = match self.diff_window.take() {
            Some(w) => (w.handle, w.focus),
            None => (None, cx.focus_handle()),
        };
        // The window's selection was in the file shown before; the panel's stays.
        if self.diff_sel.as_ref().is_some_and(|s| s.list == LIST) {
            self.diff_sel = None;
        }
        let seen = m.snapshots;
        self.diff_window = Some(DiffWindow {
            entry,
            base,
            compact: false,
            diff: None,
            change: None,
            first: true,
            seen,
            focus: focus.clone(),
            handle,
            main: window.window_handle(),
            _read: None,
        });
        self.read_window_diff(cx);
        cx.notify();
        let git = cx.entity();
        // Once this update is over: the window draws the view.
        cx.defer(move |cx| match handle {
            Some(handle) => {
                let _ = handle.update(cx, |_, window, cx| {
                    window.activate_window();
                    window.focus(&focus, cx);
                });
            }
            None => open_os_window(git, focus, kept, cx),
        });
    }

    /// Close the window (its view then forgets it).
    pub fn close_diff_window(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.diff_window.as_ref().and_then(|w| w.handle) else {
            self.diff_window = None;
            return;
        };
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        });
    }

    /// Read the window's diff again, off the UI thread.
    fn read_window_diff(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.model.read(cx).repo.clone() else { return };
        let Some(w) = &mut self.diff_window else { return };
        let (entry, base, compact) = (w.entry.clone(), w.base, w.compact);
        let read = cx.background_executor().spawn(async move {
            repo.file_diff(&entry, base).map(|d| DiffLayout::new(d, compact)).map_err(|e| e.to_string())
        });
        let path = w.entry.path.clone();
        w._read = Some(cx.spawn(async move |this, cx| {
            let diff = read.await;
            let _ = this.update(cx, |this, cx| {
                let sbs = this.model.read(cx).settings.side_by_side;
                let Some(w) = this.diff_window.as_mut().filter(|w| w.entry.path == path) else { return };
                let diff = diff.map(Rc::new);
                let first = match &diff {
                    Ok(l) if w.first => change_starts(l, sbs).first().copied(),
                    _ => None,
                };
                w.diff = Some(diff);
                if let Some(at) = first {
                    w.first = false;
                    w.change = Some(0);
                    this.window_diff_scroll.scroll_to_item(at, gpui::ScrollStrategy::Center);
                }
                cx.notify();
            });
        }));
    }

    /// Change what the window compares, or whether it leaves unchanged lines out.
    fn set_window(&mut self, base: Option<DiffBase>, compact: Option<bool>, cx: &mut Context<Self>) {
        let Some(w) = &mut self.diff_window else { return };
        if let Some(b) = base {
            w.base = b;
        }
        if let Some(c) = compact {
            w.compact = c;
        }
        w.change = None;
        w.first = true;
        self.read_window_diff(cx);
        cx.notify();
    }

    /// Go to the next change (`forward`) or the one before, from the last gone to.
    pub fn step_change(&mut self, forward: bool, cx: &mut Context<Self>) {
        let sbs = self.model.read(cx).settings.side_by_side;
        let Some(w) = &mut self.diff_window else { return };
        let Some(Ok(layout)) = &w.diff else { return };
        let starts = change_starts(layout, sbs);
        if starts.is_empty() {
            return;
        }
        let next = match (w.change, forward) {
            (None, _) => 0,
            (Some(c), true) => (c + 1).min(starts.len() - 1),
            (Some(c), false) => c.saturating_sub(1),
        };
        w.change = Some(next);
        self.window_diff_scroll.scroll_to_item(starts[next], gpui::ScrollStrategy::Center);
        cx.notify();
    }

    /// The repository was read again (a stage, a save, a refresh): the window's file as it
    /// is listed now, and its diff read again, as its text may have changed with its status
    /// the same.
    fn follow_status(&mut self, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let Some(w) = &self.diff_window else { return };
        if w.seen == m.snapshots {
            return;
        }
        let (snapshots, now) = (m.snapshots, m.status.files.iter().find(|f| f.path == w.entry.path).cloned());
        let Some(w) = &mut self.diff_window else { return };
        w.seen = snapshots;
        if let Some(now) = now {
            w.entry = now;
            self.read_window_diff(cx);
        }
    }

    /// The window's content: its bar of what to show and do, then the diff.
    pub fn diff_window_view(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.follow_status(cx);
        let w = self.diff_window.as_ref()?;
        let p = palette(cx);
        let m = self.model.read(cx);
        let sbs = m.settings.side_by_side;
        let listed = m.status.files.iter().any(|f| f.path == w.entry.path);
        let (entry, base, compact, focus) = (w.entry.clone(), w.base, w.compact, w.focus.clone());
        let root = m.repo.as_ref().map(|r| r.root().to_owned());
        let diff = w.diff.clone();
        let layout = super::shown_layout(diff.as_ref());
        let starts = layout.as_ref().map(|l| change_starts(l, sbs)).unwrap_or_default();
        let change = w.change;
        let this = cx.entity().downgrade();
        let compare = Trigger(
            div()
                .id("window-compare")
                .debug_selector(|| "window-compare".into())
                .flex_none()
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(6.))
                .bg(p.button)
                .border_1()
                .border_color(mix(p.fg, 0.08, p.button))
                .cursor_pointer()
                .whitespace_nowrap()
                .child(crate::core_text::diff_base(base))
                .child(look::disclosure(true, p.fg)),
        )
        .dropdown_menu({
            let this = this.clone();
            move |mut menu, _, _| {
                for b in DiffBase::ALL {
                    let this = this.clone();
                    menu = menu.item(PopupMenuItem::new(crate::core_text::diff_base(b)).checked(b == base).on_click(
                        move |_, _, cx| {
                            let _ = this.update(cx, |g, cx| g.set_window(Some(b), None, cx));
                        },
                    ));
                }
                menu
            }
        });
        let model = self.model.clone();
        let set_sbs = |side: bool| {
            let model = model.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                model.update(cx, |m, cx| m.set_diff_options(Some(side), None, cx))
            }
        };
        let tip = |text: String| move |w: &mut Window, cx: &mut App| Tooltip::new(text.clone()).build(w, cx);
        let step = |id: &'static str, ico: IconName, forward: bool, text: String| {
            let this = this.clone();
            look::icon_toggle(id, ico, false, &p)
                .debug_selector(move || id.to_owned())
                .tooltip(tip(crate::keymap::shown(&text)))
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |g, cx| g.step_change(forward, cx));
                })
        };
        let where_ = match (change, starts.len()) {
            (_, 0) => String::new(),
            (Some(c), n) => t!("git-change-of", n = (c + 1).to_string(), count = n.to_string()),
            (None, n) => t!("git-changes-count", count = n.to_string()),
        };
        let staged = entry.has_staged();
        let stage = {
            let (model, entries) = (self.model.clone(), vec![entry.clone()]);
            let (text, ico) = if base == DiffBase::HeadToIndex && staged {
                (ts!("cmd-unstage"), IconName::Minus)
            } else {
                (ts!("cmd-stage"), IconName::Plus)
            };
            let unstage = base == DiffBase::HeadToIndex && staged;
            let enabled = listed && if unstage { entry.has_staged() } else { entry.has_unstaged() };
            look::small_button("window-stage", ico, text, &p).debug_selector(|| "window-stage".into()).when(
                enabled,
                |b| {
                    b.on_click(move |_, _, cx| {
                        let a = if unstage {
                            Action::Unstage(unstage_paths(&entries))
                        } else {
                            Action::Stage(stage_paths(&entries))
                        };
                        model.update(cx, |m, cx| m.handle(a, cx))
                    })
                },
            )
        };
        // The file in the main window's editor, that window brought forward; this one stays.
        let open = {
            let (model, layout, main) = (self.model.clone(), layout.clone(), w.main);
            let path = root.map(|r| r.join(&entry.path));
            let at = change.and_then(|c| starts.get(c).copied()).unwrap_or(0);
            look::small_button("window-open", IconName::SquarePen, ts!("git-open-in-editor"), &p)
                .debug_selector(|| "window-open".into())
                .when_some(path, |b, path| {
                    b.on_click(move |_, _, cx| {
                        let line = layout.as_ref().and_then(|l| new_line_at(l, sbs, at)).unwrap_or(0);
                        model.update(cx, |m, cx| m.handle(Action::OpenFile(path.clone(), Some((line, 0))), cx));
                        cx.defer(move |cx| {
                            let _ = main.update(cx, |_, window, _| window.activate_window());
                        });
                    })
                })
        };
        let bar = div()
            .flex_none()
            .h(px(40.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_none().font_weight(gpui::FontWeight::SEMIBOLD).child(ts!("git-changes")))
            .child(div().min_w_0().flex_shrink(1.).child(diff_title(diff.as_ref(), &p)))
            .child(div().flex_1())
            .child(step("window-previous", IconName::ChevronUp, false, ts!("git-previous-change").to_owned()))
            .child(step("window-next", IconName::ChevronDown, true, ts!("git-next-change").to_owned()))
            .when(!where_.is_empty(), |d| d.child(look::weak(where_, &p)))
            .child(look::vseparator(&p))
            .child(compare)
            .child(
                look::icon_toggle("window-uni", IconName::Rows2, !sbs, &p)
                    .tooltip(tip(ts!("git-unified").to_owned()))
                    .on_click(set_sbs(false)),
            )
            .child(
                look::icon_toggle("window-sbs", IconName::Columns2, sbs, &p)
                    .tooltip(tip(ts!("git-side-by-side").to_owned()))
                    .on_click(set_sbs(true)),
            )
            .child(
                look::icon_toggle("window-compact", IconName::FoldVertical, compact, &p)
                    .debug_selector(|| "window-compact".into())
                    .tooltip(tip(ts!("git-hide-unchanged-lines-away-from-changes").to_owned()))
                    .on_click(cx.listener(move |g, _, _, cx| g.set_window(None, Some(!compact), cx))),
            )
            .child(look::vseparator(&p))
            .child(stage)
            .child(open);
        let empty = ts!("git-select-a-file-to-see-its-changes");
        let raw = diff_body(
            LIST,
            diff.as_ref(),
            crate::core_text::diff_sides(base),
            sbs,
            empty,
            false,
            self.diff_sel_for(LIST, layout.as_ref(), sbs),
            &self.window_diff_scroll,
            cx,
        );
        let body = self.diff_area(LIST, layout, sbs, raw, cx);
        Some(
            div()
                .id("git-diff-window")
                .debug_selector(|| "git-diff-window".into())
                .key_context("GitDiffWindow")
                .track_focus(&focus)
                .on_action(cx.listener(|g, _: &crate::search::SearchClose, _, cx| g.close_diff_window(cx)))
                .on_action(cx.listener(|g, _: &NextChange, _, cx| g.step_change(true, cx)))
                .on_action(cx.listener(|g, _: &PreviousChange, _, cx| g.step_change(false, cx)))
                .size_full()
                .flex()
                .flex_col()
                .bg(p.panel)
                .overflow_hidden()
                .text_size(px(look::BODY))
                .text_color(p.fg)
                .font_family(look::ui_font(cx))
                .child(bar)
                .child(look::separator(&p))
                .child(div().flex_1().min_h_0().p(px(8.)).flex().flex_col().child(body))
                .into_any_element(),
        )
    }
}

/// Open the window for `git`'s diff, at the size kept (or its first), centred.
fn open_os_window(git: Entity<GitView>, focus: FocusHandle, kept: Option<(f32, f32)>, cx: &mut App) {
    let (w, h) = kept.unwrap_or(DEFAULT_SIZE);
    let bounds = Bounds::centered(None, size(px(w.max(LEAST.0)), px(h.max(LEAST.1))), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(LEAST.0), px(LEAST.1))),
            titlebar: Some(gpui::TitlebarOptions { title: Some(ts!("git-changes").into()), ..Default::default() }),
            app_id: Some(crate::platform::APP_ID.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| DiffWindowView::new(git.clone(), window, cx));
            window.focus(&focus, cx);
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        },
    );
    match opened {
        Ok(handle) => git.update(cx, |g, _| {
            if let Some(w) = &mut g.diff_window {
                w.handle = Some(handle.into());
            }
        }),
        Err(e) => {
            eprintln!("Could not open the changes window: {e}");
            git.update(cx, |g, cx| {
                g.diff_window = None;
                cx.notify();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_start_where_a_run_of_changed_lines_does() {
        use snout_core::{DiffRow, FileDiff};
        let eq = |n: u32| DiffRow { kind: RowKind::Equal, old: Some((n, "x".into())), new: Some((n, "x".into())) };
        let mut rows: Vec<DiffRow> = (1..=7).map(eq).collect();
        rows[1] = DiffRow { kind: RowKind::Replace, old: Some((2, "b".into())), new: Some((2, "B".into())) };
        rows.push(DiffRow { kind: RowKind::Insert, old: None, new: Some((8, "h".into())) });
        let diff = FileDiff {
            path: "x".into(),
            base: None,
            binary: false,
            too_large: false,
            rows,
            additions: 2,
            deletions: 1,
        };
        let layout = DiffLayout::new(diff, false);
        assert_eq!(change_starts(&layout, true), [1, 7]);
        // Unified: the old `b` then the new `B` are one change.
        assert_eq!(change_starts(&layout, false), [1, 8]);
        assert_eq!(new_line_at(&layout, true, 7), Some(7));
        assert_eq!(new_line_at(&layout, true, 1), Some(1));
    }
}
