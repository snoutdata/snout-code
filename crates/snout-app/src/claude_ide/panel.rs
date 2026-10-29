//! The Claude Changes tool window: what Claude Code changed in the session and nobody has
//! reviewed yet, live as it edits. Each file with its lines added and removed, opened
//! inline on a click; each change (hunk) and each file to accept or revert, and all of
//! them at once. The changes Claude asks to make (`openDiff`) wait at the top, to accept
//! or reject here or in their tab.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Task, WeakEntity, Window,
    div, px,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::tooltip::Tooltip;
use snout_core::art::Glyph;

use super::bridge;
use super::changes::{ChangeSet, Diffed, Source};
use crate::look;
use crate::theme::{Palette, mix, palette};
use crate::workspace::Workspace;

/// How long Claude counts as busy with a file after its last word on it.
pub const ACTIVE: Duration = Duration::from_secs(20);

/// What Claude is doing now.
#[derive(Clone, Debug)]
pub struct Activity {
    pub path: PathBuf,
    pub tool: String,
    pub at: Instant,
    /// The tool ran; Claude goes on with something else.
    pub done: bool,
}

/// A change Claude asks to make, waiting in a tab for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDiff {
    pub tab: String,
    pub path: PathBuf,
    pub added: usize,
    pub removed: usize,
    pub new_file: bool,
    /// Under review in its tab now (the others wait their turn behind it).
    pub front: bool,
}

/// The session's changes, and what Claude is doing.
#[derive(Default)]
pub struct ClaudeChanges {
    pub set: ChangeSet,
    pub activity: Option<Activity>,
    pub pending: Vec<PendingDiff>,
    /// Claude Code's hooks have reported an edit: attribution is precise.
    pub hooks_heard: bool,
    /// The files whose hunks show.
    pub expanded: HashSet<PathBuf>,
    /// The project, to show paths from.
    pub root: Option<PathBuf>,
    diffs: HashMap<PathBuf, Task<()>>,
    /// Who wrote which lines this session; changed since it was last kept.
    pub ledger: super::ledger::Ledger,
    pub ledger_changed: bool,
}

/// The set changed (a file listed, changed, or gone).
pub struct Updated;

impl EventEmitter<Updated> for ClaudeChanges {}

impl ClaudeChanges {
    /// What Claude is doing now, if it is.
    pub fn busy(&self) -> Option<&Activity> {
        self.activity.as_ref().filter(|a| !a.done && a.at.elapsed() < ACTIVE)
    }

    /// `path` now holds `now`: work out its diff off the UI thread, then list it.
    /// `base` is its snapshot if it has none yet.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        path: PathBuf,
        now: Option<Arc<String>>,
        base: Option<Option<Arc<String>>>,
        tool: String,
        source: Source,
        agent: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(from) = self.set.base_of(&path).or_else(|| base.clone()) else { return };
        let (f, n) = (from.clone(), now.clone());
        let diff = cx.background_spawn(async move {
            Diffed::of(f.as_deref().map(String::as_str), n.as_deref().map(String::as_str))
        });
        let p = path.clone();
        let task = cx.spawn(async move |this, cx| {
            let diff = diff.await;
            let _ = this.update(cx, |c, cx| {
                c.diffs.remove(&p);
                c.set.after_edit(&p, base, now, diff, &tool, source, agent);
                cx.emit(Updated);
                cx.notify();
            });
        });
        self.diffs.insert(path, task);
    }

    /// Work out the diff of `path` again (after a hunk was accepted or reverted here).
    pub fn rediff(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(f) = self.set.file(path) else { return };
        let (base, now) = (f.base.clone(), f.now.clone());
        let diff = cx.background_spawn(async move {
            Diffed::of(base.as_deref().map(String::as_str), now.as_deref().map(String::as_str))
        });
        let p = path.to_owned();
        let task = cx.spawn(async move |this, cx| {
            let diff = diff.await;
            let _ = this.update(cx, |c, cx| {
                c.diffs.remove(&p);
                c.set.set_diff(&p, diff);
                cx.emit(Updated);
                cx.notify();
            });
        });
        self.diffs.insert(path.to_owned(), task);
    }

    /// Whether diffs are still being worked out.
    pub fn settling(&self) -> bool {
        !self.diffs.is_empty()
    }

    /// The file's path from the project, for display.
    pub fn display(&self, path: &Path) -> (String, String) {
        let rel = self.root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(path);
        let name = rel.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let dir = rel.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        (name, dir)
    }
}

pub struct ChangesPanel {
    pub changes: Entity<ClaudeChanges>,
    /// The floating note of what Claude is doing, over the window.
    pub indicator: Entity<Indicator>,
    ws: WeakEntity<Workspace>,
    focus: FocusHandle,
}

/// The floating note of what Claude is doing: over the editor's lower right corner.
pub struct Indicator {
    changes: Entity<ClaudeChanges>,
    ws: WeakEntity<Workspace>,
}

impl Render for Indicator {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The Claude Changes window says the same, when it shows.
        let window_open =
            self.ws.upgrade().is_some_and(|ws| ws.read(cx).tool_open(crate::workspace::Tool::ClaudeChanges, cx));
        let shown = if window_open { None } else { bridge::indicator(self.changes.read(cx), cx) };
        let ws = self.ws.clone();
        div().absolute().bottom(px(40.)).right(px(56.)).when_some(shown, |d, (el, path)| {
            d.child(el.on_click(move |_, window, cx| {
                let path = path.clone();
                let _ = ws.update(cx, |ws, cx| bridge::open_change(ws, &path, None, window, cx));
            }))
        })
    }
}

crate::panels::panel!(ChangesPanel, "Claude Changes", |_, _| "Agent Changes".into(), true);

impl ChangesPanel {
    pub fn new(ws: WeakEntity<Workspace>, cx: &mut Context<Self>) -> Self {
        let changes = cx.new(|_| ClaudeChanges::default());
        cx.observe(&changes, |_, _, cx| cx.notify()).detach();
        let indicator = cx.new(|cx| {
            cx.observe(&changes, |_, _, cx| cx.notify()).detach();
            Indicator { changes: changes.clone(), ws: ws.clone() }
        });
        Self { changes, indicator, ws, focus: cx.focus_handle() }
    }

    /// Do `f` to the workspace, from a click here.
    fn act(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>),
    ) {
        let _ = self.ws.update(cx, |ws, cx| f(ws, window, cx));
    }

    fn toggle(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.changes.update(cx, |c, cx| {
            if !c.expanded.remove(&path) {
                c.expanded.insert(path);
            }
            cx.notify();
        });
    }
}

impl ChangesPanel {
    /// The header of a turn: its agent, its prompt (or its number), when; Accept and Revert
    /// for everything it changed.
    fn turn_header(
        &self,
        changes: &ClaudeChanges,
        id: u32,
        agent: &'static str,
        p: &Palette,
        cx: &Context<Self>,
    ) -> AnyElement {
        let turn = changes.set.turn_of(id);
        let agent = crate::agents::by_id(turn.map_or(agent, |t| t.agent), cx);
        let color = agent.color(p);
        let title = turn.and_then(|t| t.prompt.clone()).unwrap_or_else(|| format!("Turn {id}"));
        let when = turn.map(|t| super::ledger::clock(t.at));
        let files = changes.set.files_of_turn(id).len();
        let sub = [
            Some(agent.name.to_string()),
            when,
            Some(if files == 1 { "1 file".into() } else { format!("{files} files") }),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" \u{00b7} ");
        let i = id as usize;
        div()
            .id(("claude-turn", i))
            .debug_selector(move || format!("claude-turn-{i}"))
            .mx(px(8.))
            .mt(px(8.))
            .mb(px(2.))
            .pl(px(8.))
            .pr(px(4.))
            .py(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(6.))
            .bg(p.wash(color))
            .child(
                div()
                    .flex_none()
                    .size(px(22.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(mix(color, 0.2, p.card))
                    .child(crate::components::icon(agent.icon, 12., color)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(div().text_size(px(11.)).text_color(p.fg_dim).whitespace_nowrap().child(sub)),
            )
            .child(
                row_button(("claude-revert-turn", i), Glyph::Discard, "Revert what this prompt changed", p)
                    .debug_selector(move || format!("claude-revert-turn-{i}"))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.act(w, cx, move |ws, w, cx| bridge::revert_turn(ws, id, w, cx))
                    })),
            )
            .child(
                row_button(("claude-accept-turn", i), Glyph::Stage, "Accept what this prompt changed", p)
                    .debug_selector(move || format!("claude-accept-turn-{i}"))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.act(w, cx, move |ws, w, cx| bridge::accept_turn(ws, id, w, cx))
                    })),
            )
            .into_any_element()
    }
}

/// A count of lines added and removed, in the code's type.
fn counts(added: usize, removed: usize, p: &Palette, cx: &App) -> gpui::Div {
    let c = p.claude();
    div()
        .flex_none()
        .flex()
        .gap(px(5.))
        .font_family(look::mono_font(cx))
        .text_size(px(11.5))
        .child(div().text_color(c.added).child(format!("+{added}")))
        .child(div().text_color(c.removed).child(format!("−{removed}")))
}

/// An action of a row: its label in a quiet button, filled under the pointer.
fn row_button(id: impl Into<gpui::ElementId>, g: Glyph, tip: &'static str, p: &Palette) -> gpui::Stateful<gpui::Div> {
    look::icon_button(id, g, tip, true, p)
}

impl Render for ChangesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let c = p.claude();
        let changes = self.changes.read(cx);
        let base = div()
            .id("claude-changes")
            .debug_selector(|| "claude-changes".into())
            .key_context("ClaudeChanges")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_size(px(look::BODY))
            .text_color(p.fg);
        let mut rows: Vec<AnyElement> = Vec::new();

        // What Claude is doing.
        if let Some(a) = changes.busy() {
            let (name, _) = changes.display(&a.path);
            rows.push(
                div()
                    .debug_selector(|| "claude-activity".into())
                    .mx(px(8.))
                    .mt(px(8.))
                    .px(px(10.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .bg(c.mark_wash)
                    .child(super::bridge::pulse_dot(c.mark, "claude-activity-dot"))
                    .child(div().text_color(p.fg_dim).child("Editing"))
                    .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(name))
                    .child(div().flex_1())
                    .child(look::chip(a.tool.clone(), c.mark, &p))
                    .into_any_element(),
            );
        }

        // The changes Claude asks to make.
        if !changes.pending.is_empty() {
            rows.push(section("Waiting for your review", &p).into_any_element());
            for (ix, d) in changes.pending.iter().enumerate() {
                let (name, dir) = changes.display(&d.path);
                let (accept_tab, reject_tab) = (d.tab.clone(), d.tab.clone());
                let path = d.path.clone();
                rows.push(
                    div()
                        .id(("claude-pending", ix))
                        .debug_selector(move || format!("claude-pending-{ix}"))
                        .mx(px(8.))
                        .mb(px(4.))
                        .p(px(8.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .rounded(px(6.))
                        .bg(p.card)
                        .border_1()
                        .border_color(mix(p.accent, 0.4, p.panel))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, w, cx| {
                            let path = path.clone();
                            this.act(w, cx, move |ws, w, cx| bridge::open_change(ws, &path, None, w, cx))
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(crate::icons::file_icon(&d.path, 13., &p))
                                .child(div().flex_none().child(name))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_color(p.fg_dim)
                                        .text_size(px(11.5))
                                        .child(dir),
                                )
                                .when(d.front, |r| r.child(counts(d.added, d.removed, &p, cx)))
                                .when(d.new_file, |r| r.child(look::chip("new", p.ok, &p))),
                        )
                        .child(div().text_color(p.fg_dim).text_size(px(11.5)).child(if d.front {
                            "Claude asks to make this change"
                        } else {
                            "Waiting behind another change to this file"
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap(px(6.))
                                .child(
                                    look::small_button(
                                        ("claude-reject", ix),
                                        crate::components::IconName::Ban,
                                        "Reject",
                                        &p,
                                    )
                                    .debug_selector(move || format!("claude-reject-{ix}"))
                                    .h(px(22.))
                                    .px(px(8.))
                                    .on_click(cx.listener(
                                        move |this, _, w, cx| {
                                            cx.stop_propagation();
                                            let tab = reject_tab.clone();
                                            this.act(w, cx, move |ws, w, cx| {
                                                bridge::answer_review(ws, &tab, false, w, cx)
                                            })
                                        },
                                    )),
                                )
                                .when(d.front, |r| {
                                    r.child(
                                        div()
                                            .id(("claude-accept", ix))
                                            .debug_selector(move || format!("claude-accept-{ix}"))
                                            .flex_none()
                                            .h(px(22.))
                                            .px(px(10.))
                                            .flex()
                                            .items_center()
                                            .gap(px(5.))
                                            .rounded(px(6.))
                                            .bg(p.accent)
                                            .text_color(p.on_accent)
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(mix(p.accent, 0.88, p.fg)))
                                            .child(crate::components::icon(
                                                crate::components::IconName::Check,
                                                12.,
                                                p.on_accent,
                                            ))
                                            .child("Accept")
                                            .on_click(cx.listener(move |this, _, w, cx| {
                                                cx.stop_propagation();
                                                let tab = accept_tab.clone();
                                                this.act(w, cx, move |ws, w, cx| {
                                                    bridge::answer_review(ws, &tab, true, w, cx)
                                                })
                                            })),
                                    )
                                }),
                        )
                        .into_any_element(),
                );
            }
        }

        // The changes made.
        let (files, added, removed) = changes.set.totals();
        if files > 0 {
            let what = if files == 1 { "1 file changed".to_owned() } else { format!("{files} files changed") };
            rows.push(
                div()
                    .debug_selector(|| "claude-summary".into())
                    .mt(px(6.))
                    .pl(px(12.))
                    .pr(px(8.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().font_weight(gpui::FontWeight::MEDIUM).child(what))
                    .child(counts(added, removed, &p, cx))
                    .into_any_element(),
            );
            rows.push(
                div()
                    .pl(px(8.))
                    .pr(px(8.))
                    .pb(px(6.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    .child(
                        look::chip_button("claude-revert-all", Glyph::Discard, "Revert All", true, &p)
                            .debug_selector(|| "claude-revert-all".into())
                            .tooltip(|w, cx| {
                                Tooltip::new("Put back every file as it was before Claude changed it").build(w, cx)
                            })
                            .on_click(cx.listener(|this, _, w, cx| this.act(w, cx, bridge::revert_all))),
                    )
                    .child(
                        look::chip_button("claude-accept-all", Glyph::Stage, "Accept All", true, &p)
                            .debug_selector(|| "claude-accept-all".into())
                            .tooltip(|w, cx| Tooltip::new("Keep every change; the files leave this list").build(w, cx))
                            .on_click(cx.listener(|this, _, w, cx| this.act(w, cx, bridge::accept_all))),
                    )
                    .into_any_element(),
            );
            rows.push(look::separator(&p).mx(px(8.)).into_any_element());
        }
        let busy_path = changes.busy().map(|a| a.path.clone());
        // By turn, the newest first: each prompt, with what it changed under it.
        let mut order: Vec<usize> = (0..changes.set.files.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(changes.set.files[i].turn), i));
        let mut last_turn = None;
        for (ix, f) in order.into_iter().map(|i| (i, &changes.set.files[i])) {
            if last_turn != Some(f.turn) {
                last_turn = Some(f.turn);
                rows.push(self.turn_header(changes, f.turn, f.agent, &p, cx));
            }
            let (name, dir) = changes.display(&f.path);
            let open = changes.expanded.contains(&f.path);
            let group: SharedString = format!("claude-file-{ix}").into();
            let editing = busy_path.as_ref() == Some(&f.path);
            let (p1, p2, p3, p4) = (f.path.clone(), f.path.clone(), f.path.clone(), f.path.clone());
            rows.push(
                crate::components::list_row_h(
                    ("claude-file", ix),
                    crate::components::RowState::default(),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || format!("claude-file-{ix}"))
                .group(group.clone())
                .mx(px(4.))
                .w_auto()
                .pl(px(6.))
                .pr(px(4.))
                .gap(px(6.))
                .rounded(px(5.))
                .cursor_pointer()
                .on_click(cx.listener(move |this, e: &gpui::ClickEvent, w, cx| {
                    if e.click_count() >= 1 {
                        let path = p1.clone();
                        this.act(w, cx, move |ws, w, cx| bridge::open_change(ws, &path, None, w, cx));
                    }
                }))
                .child(
                    div()
                        .id(("claude-disclose", ix))
                        .flex_none()
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle(p2.clone(), cx)
                        }))
                        .child(look::chevron(open, p.fg_dim)),
                )
                .child(crate::icons::file_icon(&f.path, 13., &p))
                .child(div().flex_none().when(editing, |d| d.text_color(c.mark)).child(name))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(p.fg_dim)
                        .text_size(px(11.5))
                        .child(dir),
                )
                .when(f.created(), |d| d.child(look::chip("new", p.ok, &p)))
                .when(f.source == Source::Watcher, |d| {
                    d.child(
                        div()
                            .id(("claude-seen", ix))
                            .flex_none()
                            .text_color(p.fg_dim)
                            .text_size(px(11.))
                            .tooltip(|w, cx| {
                                Tooltip::new(
                                    "Seen on disk while Claude Code was connected; its hooks did not report it",
                                )
                                .build(w, cx)
                            })
                            .child("seen"),
                    )
                })
                .child(counts(f.added, f.removed, &p, cx))
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .child(
                            row_button(("claude-revert-file", ix), Glyph::Discard, "Revert the file", &p)
                                .debug_selector(move || format!("claude-revert-file-{ix}"))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    cx.stop_propagation();
                                    let path = p3.clone();
                                    this.act(w, cx, move |ws, w, cx| bridge::revert_file(ws, &path, w, cx))
                                })),
                        )
                        .child(
                            row_button(("claude-accept-file", ix), Glyph::Stage, "Accept the file", &p)
                                .debug_selector(move || format!("claude-accept-file-{ix}"))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    cx.stop_propagation();
                                    let path = p4.clone();
                                    this.act(w, cx, move |ws, w, cx| bridge::accept_file(ws, &path, w, cx))
                                })),
                        ),
                )
                .into_any_element(),
            );
            if open {
                let now = f.now.clone();
                for (hx, h) in f.hunks.iter().enumerate() {
                    let id = ix * 10_000 + hx;
                    let preview = now
                        .as_deref()
                        .map(|t| {
                            t[h.new.clone()].lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_owned()
                        })
                        .filter(|l| !l.is_empty())
                        .unwrap_or_else(|| format!("{} lines removed", h.old_lines.len()));
                    let lines = if h.new_lines.len() <= 1 {
                        format!("Line {}", h.new_lines.start + 1)
                    } else {
                        format!("Lines {}–{}", h.new_lines.start + 1, h.new_lines.end)
                    };
                    let hgroup: SharedString = format!("claude-hunk-{id}").into();
                    let (q1, q2, q3) = (f.path.clone(), f.path.clone(), f.path.clone());
                    let line = h.new_lines.start;
                    rows.push(
                        crate::components::list_row_h(
                            ("claude-hunk", id),
                            crate::components::RowState::default(),
                            crate::components::TREE_ROW_H,
                            &p,
                        )
                        .debug_selector(move || format!("claude-hunk-{ix}-{hx}"))
                        .group(hgroup.clone())
                        .mx(px(4.))
                        .w_auto()
                        .pl(px(42.))
                        .pr(px(4.))
                        .gap(px(8.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, w, cx| {
                            let path = q1.clone();
                            this.act(w, cx, move |ws, w, cx| bridge::open_change(ws, &path, Some(line), w, cx))
                        }))
                        .child(div().flex_none().text_color(p.fg_dim).text_size(px(11.5)).child(lines))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .font_family(look::mono_font(cx))
                                .text_size(px(11.5))
                                .child(preview),
                        )
                        .child(counts(h.new_lines.len(), h.old_lines.len(), &p, cx))
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .opacity(0.)
                                .group_hover(hgroup, |s| s.opacity(1.))
                                .child(
                                    row_button(("claude-revert-hunk", id), Glyph::Discard, "Revert this change", &p)
                                        .on_click(cx.listener(move |this, _, w, cx| {
                                            cx.stop_propagation();
                                            let path = q2.clone();
                                            this.act(w, cx, move |ws, w, cx| bridge::revert_hunk(ws, &path, hx, w, cx))
                                        })),
                                )
                                .child(
                                    row_button(("claude-accept-hunk", id), Glyph::Stage, "Accept this change", &p)
                                        .on_click(cx.listener(move |this, _, w, cx| {
                                            cx.stop_propagation();
                                            let path = q3.clone();
                                            this.act(w, cx, move |ws, w, cx| bridge::accept_hunk(ws, &path, hx, w, cx))
                                        })),
                                ),
                        )
                        .into_any_element(),
                    );
                }
            }
        }
        if rows.is_empty() {
            let note = if changes.hooks_heard || cx.try_global::<super::IdeStatus>().is_some_and(|s| s.clients > 0) {
                "Files Claude Code changes show here as it works, with each change to accept or revert."
            } else {
                "Start Claude Code in the console (⌘J): the files it changes show here as it works, with each \
                 change to accept or revert."
            };
            return base.child(
                crate::components::EmptyState::new(snout_core::art::spots::claude_idle(), "No changes from Claude yet")
                    .hint(note)
                    .compact(),
            );
        }
        base.child(div().id("claude-changes-list").flex_1().min_h_0().pb(px(8.)).overflow_y_scrollbar().children(rows))
    }
}

/// A section's label.
fn section(text: &'static str, p: &Palette) -> gpui::Div {
    div()
        .pl(px(12.))
        .pt(px(10.))
        .pb(px(6.))
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(p.fg_dim)
        .child(text)
}
