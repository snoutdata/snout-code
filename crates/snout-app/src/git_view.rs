//! The Git view: when Git is picked in the activity bar, everything right of the bar is the
//! egui app's window, pane for pane: the toolbar; the repositories and branches on the
//! left; the changed files, the commit box, the diff and the Journal in the middle (or the
//! commit picked in the Journal); the Claude Code console on the right; the status bar.
//! Same palette, typefaces, sizes, drawings and proportions.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyView, App, AppContext as _, BorderStyle, Bounds, ClickEvent, Context, Div, Entity, FocusHandle,
    Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _,
    PathBuilder, Pixels, Point, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas,
    div, point, px, quad, uniform_list,
};
use gpui_component::input::{InputEvent, InputState, Textarea, TextareaState};
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_component::spinner::Spinner;
use gpui_component::tooltip::Tooltip;
use gpui_component::{Sizable as _, scroll::ScrollableElement as _};
use snout_core::art::Glyph;
use snout_core::diff_layout::{DiffLayout, SbsLine, UniLine, gap_label};
use snout_core::graph::GraphRow;
use snout_core::{
    Branch, BranchKind, CheckoutTarget, DiffBase, FileEntry, PullMode, RowKind, Tag, on_disk, stage_paths,
    unstage_paths,
};

use crate::art::{drawing, glyph};
use crate::components::{
    Column, RowState, TableColumns, file_label, path_text, state_pill, state_pills, table, table_header, table_row,
};
use crate::look::{self, BODY, MONO, MONO_SIZE, Trigger, pane, separator, title, weak};
use crate::model::{Action, Model, ModelEvent, SortColumn, StartPoint};
use crate::settings::ThemeChoice;
use crate::theme::{Palette, mix, palette};
use crate::ui::{change_pill, state_pills as file_pills};

/// A pane edge the pointer can drag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Split {
    Left,
    Branches,
    Commit,
    Changes,
    Journal,
    Console,
    Details,
    CommitChanges,
}

impl Split {
    /// Whether the edge moves along x, and whether moving right or down grows the pane.
    fn axis(self) -> (bool, bool) {
        match self {
            Self::Left => (true, true),
            Self::Commit | Self::Console | Self::Details => (true, false),
            Self::Branches | Self::Changes | Self::Journal | Self::CommitChanges => (false, false),
        }
    }

    fn min(self) -> f32 {
        match self {
            Self::Console => 360.,
            Self::Left => 180.,
            _ => 80.,
        }
    }
}

/// Pane sizes, as the egui app's panels start out (frames and margins included).
struct Sizes {
    left: f32,
    branches: f32,
    commit: f32,
    changes: f32,
    journal: f32,
    console: f32,
    details: f32,
    commit_changes: f32,
}

impl Default for Sizes {
    fn default() -> Self {
        Self {
            left: 260.,
            branches: 360.,
            commit: 380.,
            changes: 340.,
            journal: 190.,
            console: 680.,
            details: 420.,
            commit_changes: 380.,
        }
    }
}

impl Sizes {
    fn get(&mut self, s: Split) -> &mut f32 {
        match s {
            Split::Left => &mut self.left,
            Split::Branches => &mut self.branches,
            Split::Commit => &mut self.commit,
            Split::Changes => &mut self.changes,
            Split::Journal => &mut self.journal,
            Split::Console => &mut self.console,
            Split::Details => &mut self.details,
            Split::CommitChanges => &mut self.commit_changes,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Journal,
    Output,
}

pub struct GitView {
    model: Entity<Model>,
    projects: AnyView,
    console: Option<AnyView>,
    filter: Entity<InputState>,
    journal_filter: Entity<InputState>,
    message: Entity<TextareaState>,
    /// Branch sections folded or unfolded against their default.
    toggled: HashSet<String>,
    /// The branch row under the pointer, which shows a trash can.
    hovered: Option<String>,
    bottom: BottomTab,
    sizes: Sizes,
    /// The edge being dragged, where the drag started and the size then.
    dragging: Option<(Split, f32, f32)>,
    /// The commit box is too narrow for the checkbox and the button side by side.
    narrow_commit: bool,
    /// The width of the middle column, for what fits beside the Journal's tabs.
    middle_w: f32,
    /// The tables' columns, as sized by hand.
    file_columns: Entity<TableColumns>,
    commit_columns: Entity<TableColumns>,
    journal_columns: Entity<TableColumns>,
    /// The tables' keyboard focus: a click on a row gives it to its table, whose selection
    /// then shows in the accent.
    files_focus: FocusHandle,
    commit_focus: FocusHandle,
    journal_focus: FocusHandle,
    /// Which of them has it, as of this frame: files, commit files, Journal.
    focused: (bool, bool, bool),
}

fn file_columns(state_w: f32) -> Vec<Column> {
    vec![
        Column::new("Name", Some(260.)).min(120.),
        Column::new("State", Some(state_w)).min(80.),
        Column::new("Relative Directory", None),
    ]
}
const DIFF_ROW: f32 = 17.;
const LANE_W: f32 = 12.;
/// Width of a Hack digit at the diff's size.
const CHAR_W: f32 = 7.8;

impl GitView {
    pub fn new(
        model: Entity<Model>,
        projects: AnyView,
        console: Option<AnyView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("File filter"));
        let journal_filter = cx.new(|cx| InputState::new(window, cx).placeholder("Message, author, SHA or ref"));
        let message = cx.new(|cx| TextareaState::new(window, cx).placeholder("Commit message"));
        cx.subscribe(&filter, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                this.model.update(cx, |m, cx| {
                    m.filter = text;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.subscribe(&journal_filter, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                this.model.update(cx, |m, cx| {
                    m.journal_filter = text;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.subscribe(&message, |this, input, e: &InputEvent, cx| match e {
            InputEvent::Change => {
                let text = input.read(cx).value().to_string();
                this.model.update(cx, |m, cx| {
                    m.commit_msg = text;
                    cx.notify();
                });
            }
            // ⌘↩ commits.
            InputEvent::PressEnter { secondary: true, .. } => {
                this.model.update(cx, |m, cx| m.handle(Action::Commit, cx))
            }
            _ => {}
        })
        .detach();
        cx.subscribe_in(&model, window, |this, model, e: &ModelEvent, window, cx| {
            if *e == ModelEvent::Message {
                let text = model.read(cx).commit_msg.clone();
                this.message.update(cx, |s, cx| s.set_value(text, window, cx));
            }
        })
        .detach();
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        let text = model.read(cx).commit_msg.clone();
        message.update(cx, |s, cx| s.set_value(text, window, cx));
        Self {
            model,
            projects,
            console,
            filter,
            journal_filter,
            message,
            toggled: HashSet::new(),
            hovered: None,
            bottom: BottomTab::Journal,
            sizes: Sizes::default(),
            dragging: None,
            narrow_commit: false,
            middle_w: 1000.,
            file_columns: TableColumns::new(file_columns(170.), cx),
            commit_columns: TableColumns::new(file_columns(130.), cx),
            journal_columns: TableColumns::new(
                vec![
                    Column::new("", Some(14.)),
                    Column::new("Message", None),
                    Column::new("Author", Some(160.)).min(60.),
                    Column::new("Date", Some(130.)).min(60.),
                    Column::new("SHA", Some(80.)).min(50.),
                ],
                cx,
            ),
            files_focus: cx.focus_handle(),
            commit_focus: cx.focus_handle(),
            journal_focus: cx.focus_handle(),
            focused: (false, false, false),
        }
    }

    /// The commit box's width as dragged (before the 45 % cap).
    pub fn commit_width(&self) -> f32 {
        self.sizes.commit
    }

    /// The Files table's column widths, as sized by hand.
    pub fn file_column_widths(&self, cx: &App) -> Vec<Option<f32>> {
        self.file_columns.read(cx).widths()
    }

    /// Whether the bottom pane shows Output rather than the Journal.
    pub fn shows_output(&self) -> bool {
        self.bottom == BottomTab::Output
    }

    fn send(&mut self, action: Action, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.handle(action, cx));
    }

    fn on_click(&self, action: Action) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let model = self.model.clone();
        move |_, _, cx| {
            let a = action.clone();
            model.update(cx, |m, cx| m.handle(a, cx));
        }
    }

    // ----- Resizing --------------------------------------------------------------------

    /// The strip between two panes that drags `split`.
    fn handle(&self, split: Split, cx: &mut Context<Self>) -> Div {
        let (horizontal, _) = split.axis();
        let grip = div()
            .id(SharedString::from(format!("split-{split:?}")))
            .debug_selector(move || format!("split-{split:?}"))
            .absolute()
            .when(horizontal, |d| d.top_0().bottom_0().left(px(-3.)).w(px(6.)).cursor_col_resize())
            .when(!horizontal, |d| d.left_0().right_0().top(px(-3.)).h(px(6.)).cursor_row_resize())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    let at = if horizontal { e.position.x } else { e.position.y };
                    let size = *this.sizes.get(split);
                    this.dragging = Some((split, f32::from(at), size));
                    cx.stop_propagation();
                }),
            );
        // No room of its own: the grip overhangs the gap between the panes.
        div()
            .flex_none()
            .relative()
            .when(horizontal, |d| d.w(px(0.)).h_full())
            .when(!horizontal, |d| d.h(px(0.)).w_full())
            .child(grip)
    }

    fn drag_to(&mut self, pos: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((split, from, size)) = self.dragging else { return };
        let (horizontal, grows) = split.axis();
        let at = f32::from(if horizontal { pos.x } else { pos.y });
        let delta = if grows { at - from } else { from - at };
        *self.sizes.get(split) = (size + delta).max(split.min());
        cx.notify();
    }

    // ----- Toolbar and status bar -------------------------------------------------------

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let has_repo = m.repo.is_some();
        let net = has_repo && m.job.is_none();
        let entries = m.selected_entries();
        let in_tree = m.commit_view.is_none();
        let push = match m.status.branch.ahead {
            0 => "Push".to_owned(),
            n => format!("Push ({n})"),
        };
        let console = if m.settings.console_open { "Hide Claude" } else { "Claude" };
        let theme = m.settings.theme;
        let has_github = m.github_url.is_some();
        let github_tip = m.github_url.clone().unwrap_or_else(|| "No GitHub remote".into());
        let model = self.model.clone();
        let tool = |id: &'static str, g: Glyph, label: String, enabled: bool, tip: &'static str, action: Action| {
            let b = look::glyph_button(id, g, label, enabled, &p);
            let b = if tip.is_empty() { b } else { b.tooltip(move |w, cx| Tooltip::new(tip).build(w, cx)) };
            b.when(enabled, |b| b.on_click(self.on_click(action)))
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .py(px(6.))
            .bg(p.ground)
            .child(tool("tb-open", Glyph::Open, "Open".into(), true, "Open local repository (⌘O)", Action::PickFolder))
            .child(look::vseparator(&p))
            .child(tool("tb-pull", Glyph::Pull, "Pull".into(), net, "git pull", Action::Pull(PullMode::Default)))
            .child(tool("tb-fetch", Glyph::Fetch, "Fetch".into(), net, "git fetch --all --prune", Action::Fetch))
            .child(tool("tb-push", Glyph::Push, push, net, "git push (sets upstream on first push)", Action::Push))
            .child(look::vseparator(&p))
            .child(tool(
                "tb-stage",
                Glyph::Stage,
                "Stage".into(),
                in_tree && entries.iter().any(FileEntry::has_unstaged),
                "",
                Action::Stage(stage_paths(&entries)),
            ))
            .child(tool(
                "tb-unstage",
                Glyph::Unstage,
                "Unstage".into(),
                in_tree && entries.iter().any(FileEntry::has_staged),
                "",
                Action::Unstage(unstage_paths(&entries)),
            ))
            .child(tool(
                "tb-discard",
                Glyph::Discard,
                "Discard".into(),
                in_tree && !entries.is_empty(),
                "",
                Action::AskDiscard(entries.clone()),
            ))
            .child(tool(
                "tb-trash",
                Glyph::Trash,
                "Delete".into(),
                in_tree && entries.iter().any(on_disk),
                "Move to the Trash (⌘⌫)",
                Action::AskDeleteFiles(entries.clone()),
            ))
            .child(look::vseparator(&p))
            .child(tool("tb-refresh", Glyph::Refresh, "Refresh".into(), has_repo, "F5 / ⌘R", Action::Refresh))
            .child(tool(
                "tb-checkout",
                Glyph::Switch,
                "Check Out…".into(),
                has_repo,
                "Branch, tag or commit (⇧⌘O)",
                Action::ShowCheckout,
            ))
            .child(tool("tb-branch", Glyph::Branch, "New Branch".into(), has_repo, "", Action::ShowNewBranch(None)))
            .child(div().flex_1())
            .child(Trigger(look::glyph_button("tb-theme", Glyph::Theme, "", true, &p)).dropdown_menu(
                move |mut menu, _, _| {
                    for choice in ThemeChoice::ALL {
                        let model = model.clone();
                        menu = menu.item(PopupMenuItem::new(choice.label()).checked(theme == choice).on_click(
                            move |_, _, cx| model.update(cx, |m, cx| m.handle(Action::SetTheme(choice), cx)),
                        ));
                    }
                    menu
                },
            ))
            .child(
                look::glyph_button("tb-github", Glyph::Web, "GitHub", has_github, &p)
                    .tooltip(move |w, cx| Tooltip::new(github_tip.clone()).build(w, cx))
                    .when(has_github, |b| b.on_click(self.on_click(Action::OpenGithub))),
            )
            .child(tool(
                "tb-claude",
                Glyph::Spark,
                console.into(),
                true,
                "Claude Code console in this repository (⌘J)",
                Action::ToggleConsole,
            ))
            .into_any_element()
    }

    // ----- Left column -------------------------------------------------------------------

    fn is_open(&self, key: &str, default: bool) -> bool {
        default != self.toggled.contains(key)
    }

    /// egui's collapsing header: a triangle and the title; a click folds or unfolds.
    fn section(&self, key: &str, text: String, default: bool, cx: &mut Context<Self>) -> (bool, AnyElement) {
        let open = self.is_open(key, default);
        let p = palette(cx);
        let k = key.to_owned();
        let header = div()
            .id(SharedString::from(format!("section-{key}")))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(crate::components::TREE_ROW_H))
            .mt(px(4.))
            .pl(px(2.))
            .cursor_pointer()
            .child(look::chevron(open, p.fg_dim))
            .child(crate::components::section_title(text, &p))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.toggled.remove(&k) {
                    this.toggled.insert(k.clone());
                }
                cx.notify();
            }));
        (open, header.into_any_element())
    }

    fn branches(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let has_repo = m.repo.is_some();
        let idle = m.job.is_none();
        let detached = (m.status.branch.head.is_none() && has_repo).then(|| m.head_label());
        let branches = m.branches.clone();
        let tags = m.tags.clone();
        let mut rows: Vec<AnyElement> = Vec::new();
        if !has_repo {
            rows.push(weak("No repository open", &p).into_any_element());
        }
        if let Some(label) = detached {
            rows.push(
                crate::components::tree_row("detached-head", RowState::default(), &p)
                    .pl(px(20.))
                    .child(crate::components::icon(crate::components::IconName::GitCommitHorizontal, 14., p.branch()))
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(format!("HEAD ({label})")))
                    .into_any_element(),
            );
        }
        if has_repo {
            let (open, header) = self.section("local", "Local Branches".into(), true, cx);
            rows.push(header);
            if open {
                for b in branches.iter().filter(|b| b.kind == BranchKind::Local) {
                    rows.push(self.branch_row(b, &p, cx));
                }
            }
            let mut remotes: Vec<String> = branches.iter().filter_map(|b| b.remote.clone()).collect();
            remotes.dedup();
            for remote in remotes {
                let (open, header) = self.section(&format!("remote:{remote}"), remote.clone(), false, cx);
                rows.push(header);
                if open {
                    for b in branches.iter().filter(|b| b.remote.as_deref() == Some(remote.as_str())) {
                        rows.push(self.branch_row(b, &p, cx));
                    }
                }
            }
            if !tags.is_empty() {
                let (open, header) = self.section("tags", format!("Tags ({})", tags.len()), false, cx);
                rows.push(header);
                if open {
                    for t in &tags {
                        rows.push(self.tag_row(t, &p, cx));
                    }
                }
            }
        }
        let button = |id: &'static str, g: Glyph, tip: &'static str, enabled: bool, action: Action| {
            look::icon_button(id, g, tip, enabled, &p).when(enabled, |b| b.on_click(self.on_click(action)))
        };
        pane(&p)
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex_none()
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(title("Branches"))
                    .child(div().flex_1())
                    .child(button("br-checkout", Glyph::Switch, "Check out… (⇧⌘O)", has_repo, Action::ShowCheckout))
                    .child(button("br-new", Glyph::Branch, "New branch", has_repo, Action::ShowNewBranch(None)))
                    .child(button("br-fetch", Glyph::Fetch, "Fetch", has_repo && idle, Action::Fetch)),
            )
            .child(separator(&p))
            .child(div().id("branch-list").flex_1().min_h_0().flex().flex_col().overflow_y_scrollbar().children(rows))
            .into_any_element()
    }

    fn branch_row(&mut self, b: &Branch, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let label = match b.kind {
            BranchKind::Local => b.name.clone(),
            BranchKind::Remote => b.local_name().to_owned(),
        };
        let tip: SharedString = format!(
            "{}{}\n{} {}\n\nDouble-click to check out",
            b.name,
            b.upstream.as_ref().map(|u| format!(" → {u}")).unwrap_or_default(),
            b.target,
            b.subject
        )
        .into();
        let key = format!("{:?}:{}", b.kind, b.name);
        let hovered = self.hovered.as_deref() == Some(key.as_str());
        let delete_label = match b.kind {
            BranchKind::Local => "Delete…".to_owned(),
            BranchKind::Remote => format!("Delete from {}…", b.remote.as_deref().unwrap_or("remote")),
        };
        let checkout_label = match b.kind {
            BranchKind::Local => "Check out".to_owned(),
            BranchKind::Remote => format!("Check out as local “{}”", b.local_name()),
        };
        let (b1, b2, b3) = (b.clone(), b.clone(), b.clone());
        let model = self.model.clone();
        let hover_key = key.clone();
        let trash_tip = if b.kind == BranchKind::Local { "Delete branch…" } else { "Delete from the remote…" };
        let head_ink = if b.is_head { p.fg } else { mix(p.fg, 0.8, p.fg_dim) };
        let (ahead, behind, gone) = (b.ahead, b.behind, b.upstream_gone);
        crate::components::tree_row(SharedString::from(format!("branch-{key}")), RowState::default(), p)
            .pl(px(20.))
            .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                this.hovered = on.then(|| hover_key.clone());
                cx.notify();
            }))
            .child(crate::components::icon(
                if b.kind == BranchKind::Local {
                    crate::components::IconName::GitBranch
                } else {
                    crate::components::IconName::Cloud
                },
                14.,
                if b.is_head { p.branch() } else { p.fg_dim },
            ))
            .child(
                div()
                    .id("label")
                    .h_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_color(head_ink)
                            .when(b.is_head, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                            .child(label),
                    )
                    .when(ahead > 0, |d| d.child(crate::components::dim(format!("↑{ahead}"), p)))
                    .when(behind > 0, |d| d.child(crate::components::dim(format!("↓{behind}"), p)))
                    .when(gone, |d| d.child(crate::components::badge("gone", crate::components::Tone::Warning, p)))
                    .when(b.is_head, |d| d.child(crate::components::badge("HEAD", crate::components::Tone::Git, p)))
                    .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                        if e.click_count() == 2 && !b1.is_head {
                            this.send(Action::RequestCheckout((&b1).into()), cx);
                        }
                    }))
                    .context_menu(move |menu, _, _| {
                        let b = b2.clone();
                        let send = |a: Action| {
                            let model = model.clone();
                            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                let a = a.clone();
                                model.update(cx, |m, cx| m.handle(a, cx))
                            }
                        };
                        menu.item(
                            PopupMenuItem::new(checkout_label.clone())
                                .icon(crate::components::menu_icon(crate::components::IconName::ArrowRightLeft))
                                .disabled(b.is_head)
                                .on_click(send(Action::RequestCheckout((&b).into()))),
                        )
                        .item(
                            PopupMenuItem::new("Create branch from here…")
                                .icon(crate::components::menu_icon(crate::components::IconName::GitBranch))
                                .on_click(send(Action::ShowNewBranch(Some(StartPoint {
                                    label: b.name.clone(),
                                    rev: b.name.clone(),
                                })))),
                        )
                        .item(
                            PopupMenuItem::new(delete_label.clone())
                                .icon(crate::components::menu_icon(crate::components::IconName::Trash))
                                .disabled(b.is_head)
                                .on_click(send(Action::AskDeleteBranch(b.clone()))),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Copy name")
                                .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                .on_click(send(Action::Copy(b.name.clone()))),
                        )
                    }),
            )
            .child(div().flex_1())
            .when(hovered && !b.is_head, |d| {
                d.child(
                    look::icon_button("trash", Glyph::Trash, trash_tip, true, p).on_click(
                        cx.listener(move |this, _, _, cx| this.send(Action::AskDeleteBranch(b3.clone()), cx)),
                    ),
                )
            })
            .into_any_element()
    }

    fn tag_row(&mut self, t: &Tag, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let tip: SharedString =
            format!("{} {}\n{}\n\nDouble-click to check out (detached)", t.target, t.subject, t.date).into();
        let target = CheckoutTarget::Tag(t.name.clone());
        let (t1, name) = (target.clone(), t.name.clone());
        let model = self.model.clone();
        crate::components::tree_row(SharedString::from(format!("tag-row-{}", t.name)), RowState::default(), p)
            .pl(px(20.))
            .child(crate::components::icon(crate::components::IconName::Tag, 14., p.fg_dim))
            .child(
                div()
                    .id(SharedString::from(format!("tag-{}", t.name)))
                    .h_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                    .child(t.name.clone())
                    .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                        if e.click_count() == 2 {
                            this.send(Action::RequestCheckout(t1.clone()), cx);
                        }
                    }))
                    .context_menu(move |menu, _, _| {
                        let send = |a: Action| {
                            let model = model.clone();
                            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                let a = a.clone();
                                model.update(cx, |m, cx| m.handle(a, cx))
                            }
                        };
                        let start = StartPoint { label: format!("tag {name}"), rev: format!("refs/tags/{name}") };
                        menu.item(
                            PopupMenuItem::new("Check out (detached)")
                                .icon(crate::components::menu_icon(crate::components::IconName::ArrowRightLeft))
                                .on_click(send(Action::RequestCheckout(target.clone()))),
                        )
                        .item(
                            PopupMenuItem::new("Create branch from tag…")
                                .icon(crate::components::menu_icon(crate::components::IconName::GitBranch))
                                .on_click(send(Action::ShowNewBranch(Some(start)))),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Copy name")
                                .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                .on_click(send(Action::Copy(name.clone()))),
                        )
                    }),
            )
            .into_any_element()
    }

    // ----- Working tree ------------------------------------------------------------------

    fn files(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let summary = format!("{} changed, {} staged", m.status.files.len(), m.status.staged_count());
        let dir = m.dir_filter.clone();
        let clean = m.status.files.is_empty();
        let clean_text = match &m.status.branch.head {
            Some(head) => format!("Nothing to commit on {head}"),
            None => "Nothing to commit".to_owned(),
        };
        let rows: Vec<FileEntry> = m.visible_files().into_iter().cloned().collect();
        let selected = m.selected.clone();
        let (by, asc) = m.sort;
        let root = m.repo.as_ref().map(|r| r.root().to_owned());
        let model = self.model.clone();
        let header = div()
            .flex_none()
            .h(px(22.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(title("Files"))
            .child(weak(summary, &p))
            .when_some(dir, |d, dir| {
                d.child(
                    look::small_button("dir-chip", crate::components::IconName::FolderX, format!("in {dir}/"), &p)
                        .tooltip(|w, cx| Tooltip::new("Show all files").build(w, cx))
                        .on_click(self.on_click(Action::FilterDir(None))),
                )
            })
            .child(div().flex_1())
            .child(look::field(&self.filter, true, Some(220.), &p).flex_shrink(1.).min_w(px(80.)));
        let body = if clean {
            look::art_note(snout_core::art::clean(), "The working tree is clean", clean_text, cx)
        } else {
            let sorts = [SortColumn::Name, SortColumn::State, SortColumn::Directory];
            let sorted = sorts.iter().position(|c| *c == by).map(|i| (i, asc));
            let header = table_header(
                "files-head",
                &self.file_columns,
                sorted,
                Some(Rc::new({
                    let model = self.model.clone();
                    move |i: usize, _: &mut Window, cx: &mut App| model.update(cx, |m, cx| m.sort_by(sorts[i], cx))
                })),
                cx,
            );
            let widths = self.file_columns.read(cx).widths();
            let focused = self.focused.0;
            let lead = m.anchor.clone();
            let list = uniform_list("files", rows.len(), move |range, _, cx| {
                range
                    .map(|i| {
                        let f = &rows[i];
                        let is_selected = selected.contains(&f.path);
                        let entries: Vec<FileEntry> =
                            if is_selected { model.read(cx).selected_entries() } else { vec![f.clone()] };
                        let (model, root) = (model.clone(), root.clone());
                        let (path, f2) = (f.path.clone(), f.clone());
                        let cells = vec![
                            file_label(f.file_name().to_owned(), &p).into_any_element(),
                            state_pills(&file_pills(f), &p).into_any_element(),
                            path_text(("file-dir", i), f.directory().to_owned(), p.fg_dim).into_any_element(),
                        ];
                        let state = RowState::new(is_selected, focused).lead(lead.as_deref() == Some(f.path.as_str()));
                        table_row(("file", i), state, &widths, cells, &p)
                            .on_click({
                                let model = model.clone();
                                move |e: &ClickEvent, _, cx| {
                                    let mods = e.modifiers();
                                    model.update(cx, |m, cx| m.click_file(&path, mods.shift, mods.platform, cx));
                                }
                            })
                            .on_mouse_down(MouseButton::Right, {
                                let model = model.clone();
                                let path = f2.path.clone();
                                move |_, _, cx| {
                                    // A right click on an unselected file selects it first.
                                    if !model.read(cx).selected.contains(&path) {
                                        model.update(cx, |m, cx| m.click_file(&path, false, false, cx));
                                    }
                                }
                            })
                            .context_menu(move |menu, _, _| {
                                let send = |a: Action| {
                                    let model = model.clone();
                                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                        let a = a.clone();
                                        model.update(cx, |m, cx| m.handle(a, cx))
                                    }
                                };
                                let mut menu = menu
                                    .item(
                                        PopupMenuItem::new("Stage")
                                            .icon(crate::components::menu_icon(crate::components::IconName::Plus))
                                            .disabled(!entries.iter().any(FileEntry::has_unstaged))
                                            .on_click(send(Action::Stage(stage_paths(&entries)))),
                                    )
                                    .item(
                                        PopupMenuItem::new("Unstage")
                                            .icon(crate::components::menu_icon(crate::components::IconName::Minus))
                                            .disabled(!entries.iter().any(FileEntry::has_staged))
                                            .on_click(send(Action::Unstage(unstage_paths(&entries)))),
                                    )
                                    .item(
                                        PopupMenuItem::new("Discard…")
                                            .icon(crate::components::menu_icon(crate::components::IconName::Undo2))
                                            .on_click(send(Action::AskDiscard(entries.clone()))),
                                    )
                                    .item(
                                        PopupMenuItem::new("Delete…")
                                            .icon(crate::components::menu_icon(crate::components::IconName::Trash))
                                            .disabled(!entries.iter().any(on_disk))
                                            .on_click(send(Action::AskDeleteFiles(entries.clone()))),
                                    )
                                    .separator();
                                if let Some(root) = &root {
                                    menu = menu.item(
                                        PopupMenuItem::new("Reveal in Finder")
                                            .icon(crate::components::menu_icon(crate::components::IconName::FolderOpen))
                                            .on_click(send(Action::Reveal(root.join(&f2.path)))),
                                    );
                                }
                                menu.item(
                                    PopupMenuItem::new("Copy path")
                                        .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                        .on_click(send(Action::Copy(f2.path.clone()))),
                                )
                            })
                            .into_any_element()
                    })
                    .collect()
            })
            .flex_1();
            let focus = self.files_focus.clone();
            table(header, list)
                .track_focus(&self.files_focus)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| focus.focus(window, cx))
                .on_mouse_down(MouseButton::Right, {
                    let focus = self.files_focus.clone();
                    move |_, window, cx| focus.focus(window, cx)
                })
                .into_any_element()
        };
        pane(&p).child(header).child(separator(&p)).child(body).into_any_element()
    }

    fn commit_box(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let scope = m.commit_scope();
        let has_changes = m.repo.is_some() && scope != snout_core::Scope::Nothing;
        let drafting = m.draft.is_some();
        let can_commit = m.can_commit();
        let amend = m.amend;
        let has_log = !m.log.is_empty();
        let draft_tip: SharedString = if has_changes {
            format!("Claude Code writes the message from {}, building on anything you typed", scope.what()).into()
        } else {
            "Nothing to commit".into()
        };
        let commit_tip: SharedString = format!("Commits {}  ⌘↩", scope.what()).into();
        let header = div()
            .flex_none()
            .h(px(22.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(title("Commit"))
            .child(div().flex_1())
            .when(drafting, |d| {
                d.child(Spinner::new().xsmall()).child(
                    look::chip_button("stop-draft", Glyph::Close, "Stop", true, &p)
                        .on_click(self.on_click(Action::CancelDraft)),
                )
            })
            .when(!drafting, |d| {
                d.child(
                    look::chip_button("draft", Glyph::Spark, "Draft", has_changes, &p)
                        .tooltip(move |w, cx| Tooltip::new(draft_tip.clone()).build(w, cx))
                        .when(has_changes, |b| b.on_click(self.on_click(Action::DraftMessage))),
                )
            });
        let message = div().flex_1().min_h_0().p(px(8.)).child(
            div()
                .size_full()
                .bg(p.input)
                .border_1()
                .border_color(mix(p.fg, 0.08, p.button))
                .rounded(px(6.))
                .child(Textarea::new(&self.message).appearance(false).small().h_full().text_size(px(BODY))),
        );
        let amend_box = look::checkbox("amend", "Amend last commit", amend, has_log, &p).when(has_log, |c| {
            c.on_click(cx.listener(move |this, _, _, cx| this.model.update(cx, |m, cx| m.set_amend(!amend, cx))))
        });
        let commit =
            div().flex_1().flex().items_center().justify_end().gap(px(8.)).child(weak(scope.label(), &p)).child(
                look::primary_button("commit", crate::components::IconName::Check, "Commit", can_commit, &p)
                    .tooltip(move |w, cx| Tooltip::new(commit_tip.clone()).build(w, cx))
                    .when(can_commit, |b| b.on_click(self.on_click(Action::Commit))),
            );
        // Side by side when there is room, the button under the checkbox when not.
        let actions = if self.narrow_commit {
            div()
                .flex_none()
                .px(px(8.))
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(div().h(px(22.)).flex().items_center().child(amend_box))
                .child(div().h(px(32.)).flex().child(commit))
        } else {
            div().flex_none().h(px(32.)).px(px(8.)).flex().items_center().gap(px(8.)).child(amend_box).child(commit)
        };
        pane(&p)
            .child(header)
            .child(separator(&p))
            .child(message)
            // egui draws a hairline over the panel holding the checkbox and the button.
            .child(div().flex_none().h(px(1.)).bg(p.border))
            .child(actions)
            .into_any_element()
    }

    fn changes(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let base = m.settings.diff_base;
        let empty = if m.selected.len() > 1 {
            format!("{} files selected", m.selected.len())
        } else {
            "Select a file to see its changes".to_owned()
        };
        let model = self.model.clone();
        let compare = Trigger(
            div()
                .id("compare")
                .flex_none()
                .h(px(22.))
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
                .child(base.label())
                .child(look::disclosure(true, p.fg)),
        )
        .dropdown_menu(move |mut menu, _, _| {
            for b in DiffBase::ALL {
                let model = model.clone();
                menu = menu.item(
                    PopupMenuItem::new(b.label())
                        .checked(b == base)
                        .on_click(move |_, _, cx| model.update(cx, |m, cx| m.set_diff_base(b, cx))),
                );
            }
            menu
        });
        let header = div()
            .flex_none()
            .h(px(22.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(title("Changes"))
            .child(diff_title(m.diff.as_ref(), &p))
            .child(div().flex_1())
            .child(compare)
            .child(look::vseparator(&p))
            .child(diff_controls(&self.model, cx));
        let body = if base == DiffBase::HeadToIndex && m.status.staged_count() == 0 && m.diff.is_none() {
            look::art_note(
                snout_core::art::spots::nothing_staged(),
                "Nothing staged",
                "Stage files to see what the next commit holds.",
                cx,
            )
        } else {
            diff_body("diff-lines", m.diff.as_ref(), base.sides(), m.settings.side_by_side, &empty, cx)
        };
        pane(&p).child(header).child(separator(&p)).child(body).into_any_element()
    }

    // ----- Journal -----------------------------------------------------------------------

    fn journal(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let tab = self.bottom;
        let m = self.model.read(cx);
        // Only where it fits beside the tabs.
        let hint = m.commit_view.is_none() && !m.log.is_empty() && self.middle_w > 836.;
        let tabs = div()
            .flex_none()
            .h(px(22.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                look::toggle(
                    "tab-journal",
                    crate::components::IconName::NotebookText,
                    "Journal",
                    tab == BottomTab::Journal,
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.bottom = BottomTab::Journal;
                    cx.notify();
                })),
            )
            .child(
                look::toggle(
                    "tab-output",
                    crate::components::IconName::SquareTerminal,
                    "Output",
                    tab == BottomTab::Output,
                    &p,
                )
                .debug_selector(|| "tab-output".into())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.bottom = BottomTab::Output;
                    cx.notify();
                })),
            )
            .child(div().flex_1())
            .when(tab == BottomTab::Journal, |d| {
                d.when(hint, |d| d.child(weak("Click a commit to see what it changed", &p))).child(look::field(
                    &self.journal_filter,
                    true,
                    Some(240.),
                    &p,
                ))
            })
            .when(tab == BottomTab::Output, |d| {
                d.child(look::small_button("clear-output", crate::components::IconName::Eraser, "Clear", &p).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.model.update(cx, |m, cx| {
                            m.output.clear();
                            cx.notify();
                        })
                    }),
                ))
            });
        let body = match tab {
            BottomTab::Output => div()
                .id("output")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .font_family(MONO)
                .text_size(px(MONO_SIZE))
                .children(
                    m.output
                        .iter()
                        .map(|l| div().text_color(if l.error { p.error } else { p.fg }).child(l.text.clone())),
                )
                .into_any_element(),
            BottomTab::Journal if m.log.is_empty() => {
                look::art_note(snout_core::art::seed(), "No commits yet", "Stage some files and make the first one", cx)
            }
            BottomTab::Journal => self.journal_table(cx),
        };
        pane(&p).child(tabs).child(separator(&p)).child(body).into_any_element()
    }

    fn journal_table(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let rows = m.journal_rows();
        // Lanes only mean something over the whole history, so a filter hides them.
        let show_graph = rows.len() == m.log.len();
        let graph_w = if show_graph { snout_core::graph::lane_count(&m.graph) as f32 * LANE_W + 8.0 } else { 14.0 };
        let log = Rc::new(m.log.clone());
        let graph = Rc::new(m.graph.clone());
        let shown = m.commit_view.as_ref().and_then(|v| v.id()).map(str::to_owned);
        let model = self.model.clone();
        self.journal_columns.update(cx, |c, _| c.fit(0, graph_w));
        let columns = self.journal_columns.read(cx).widths();
        let focused = self.focused.2;
        let list = uniform_list("journal", rows.len(), move |range, _, _| {
            range
                .map(|k| {
                    let i = rows[k];
                    let c = &log[i];
                    let is_head = c.refs.iter().any(|r| r == "HEAD" || r.starts_with("HEAD -> "));
                    let selected = shown.as_deref() == Some(c.id.as_str());
                    let (row, unpushed) = (graph.get(i).cloned().unwrap_or_default(), c.unpushed);
                    let (id, summary, short) = (c.id.clone(), c.summary.clone(), c.short_id.clone());
                    let model = model.clone();
                    let lanes = canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            // Over the row's frame too, so the lanes run on from row to row.
                            let bounds = Bounds::new(
                                point(bounds.origin.x, bounds.origin.y - px(1.)),
                                gpui::size(bounds.size.width, bounds.size.height + px(2.)),
                            );
                            if show_graph {
                                paint_graph(bounds, &row, unpushed, is_head, &p, window);
                            } else if unpushed {
                                let c = bounds.center();
                                let b = Bounds::new(point(c.x - px(3.5), c.y - px(3.5)), gpui::size(px(7.), px(7.)));
                                window.paint_quad(quad(
                                    b,
                                    px(3.5),
                                    p.ill_accent,
                                    px(0.),
                                    p.ill_accent,
                                    BorderStyle::Solid,
                                ));
                            }
                        },
                    )
                    .size_full();
                    let cells = vec![
                        lanes.into_any_element(),
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .children(c.refs.iter().map(|r| look::ref_badge(r, &p)))
                            .child(c.summary.clone())
                            .into_any_element(),
                        crate::components::dim(c.author.clone(), &p).into_any_element(),
                        crate::components::dim(c.date.clone(), &p).into_any_element(),
                        div()
                            .font_family(MONO)
                            .text_size(px(MONO_SIZE - 1.))
                            .text_color(p.fg_dim)
                            .child(c.short_id.clone())
                            .into_any_element(),
                    ];
                    table_row(("commit", k), RowState::new(selected, focused), &columns, cells, &p)
                        .on_click({
                            let (model, id) = (model.clone(), id.clone());
                            move |_, _, cx| {
                                if !selected {
                                    let a = Action::ShowCommit(id.clone());
                                    model.update(cx, |m, cx| m.handle(a, cx));
                                }
                            }
                        })
                        .context_menu(move |menu, _, _| {
                            let send = |a: Action| {
                                let model = model.clone();
                                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    let a = a.clone();
                                    model.update(cx, |m, cx| m.handle(a, cx))
                                }
                            };
                            let start = StartPoint { label: format!("{short} {summary}"), rev: id.clone() };
                            menu.item(
                                PopupMenuItem::new("Show changes")
                                    .icon(crate::components::menu_icon(crate::components::IconName::FileDiff))
                                    .on_click(send(Action::ShowCommit(id.clone()))),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Check out this commit (detached)")
                                    .icon(crate::components::menu_icon(crate::components::IconName::ArrowRightLeft))
                                    .on_click(send(Action::RequestCheckout(CheckoutTarget::Commit(id.clone())))),
                            )
                            .item(
                                PopupMenuItem::new("Create branch here…")
                                    .icon(crate::components::menu_icon(crate::components::IconName::GitBranch))
                                    .on_click(send(Action::ShowNewBranch(Some(start)))),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Copy SHA")
                                    .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                    .on_click(send(Action::Copy(id.clone()))),
                            )
                            .item(
                                PopupMenuItem::new("Copy message")
                                    .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                    .on_click(send(Action::Copy(summary.clone()))),
                            )
                        })
                        .into_any_element()
                })
                .collect()
        })
        .flex_1();
        let focus = self.journal_focus.clone();
        table(table_header("journal-head", &self.journal_columns, None, None, cx), list)
            .track_focus(&self.journal_focus)
            .on_any_mouse_down(move |_, window, cx| focus.focus(window, cx))
            .into_any_element()
    }

    // ----- Commit view -------------------------------------------------------------------

    fn commit_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let model = self.model.clone();
        let m = self.model.read(cx);
        let Some(view) = &m.commit_view else { return div().into_any_element() };
        let back = look::glyph_button("back", Glyph::Back, "Working tree", true, &p)
            .tooltip(|w, cx| Tooltip::new("Back (Esc)").build(w, cx))
            .on_click(self.on_click(Action::CloseCommit));
        let d = match &view.details {
            Ok(d) => d.clone(),
            Err(e) => {
                return div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(back)
                    .child(look::centered_note(e.clone(), &p))
                    .into_any_element();
            }
        };
        let root = m.repo.as_ref().map(|r| r.root().to_owned());
        let selected = view.selected.clone();
        let rows = d.files.clone();
        let widths = self.commit_columns.read(cx).widths();
        let focused = self.focused.1;
        let list = uniform_list("commit-files", rows.len(), {
            let model = model.clone();
            move |range, _, _| {
                range
                    .map(|i| {
                        let f = &rows[i];
                        let dir = match &f.orig_path {
                            Some(orig) => format!("{}  (from {orig})", f.directory()),
                            None => f.directory().to_owned(),
                        };
                        let is_sel = selected.as_ref() == Some(&f.path);
                        let path = f.path.clone();
                        let (model, root) = (model.clone(), root.clone());
                        let cells = vec![
                            file_label(f.file_name().to_owned(), &p).into_any_element(),
                            state_pill(change_pill(f.change), &p).into_any_element(),
                            path_text(("cfile-dir", i), dir, p.fg_dim).into_any_element(),
                        ];
                        table_row(("cfile", i), RowState::new(is_sel, focused), &widths, cells, &p)
                            .on_click({
                                let (model, path) = (model.clone(), path.clone());
                                move |_, _, cx| model.update(cx, |m, cx| m.select_commit_file(path.clone(), cx))
                            })
                            .context_menu(move |menu, _, _| {
                                let send = |a: Action| {
                                    let model = model.clone();
                                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                        let a = a.clone();
                                        model.update(cx, |m, cx| m.handle(a, cx))
                                    }
                                };
                                let menu = match &root {
                                    Some(root) => menu.item(
                                        PopupMenuItem::new("Reveal in Finder")
                                            .icon(crate::components::menu_icon(crate::components::IconName::FolderOpen))
                                            .on_click(send(Action::Reveal(root.join(&path)))),
                                    ),
                                    None => menu,
                                };
                                menu.item(
                                    PopupMenuItem::new("Copy path")
                                        .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                                        .on_click(send(Action::Copy(path.clone()))),
                                )
                            })
                            .into_any_element()
                    })
                    .collect()
            }
        })
        .flex_1();
        let files_body = if d.files.is_empty() {
            look::art_note(snout_core::art::clean(), "", "This commit changes no files", cx)
        } else {
            let focus = self.commit_focus.clone();
            table(table_header("commit-files-head", &self.commit_columns, None, None, cx), list)
                .track_focus(&self.commit_focus)
                .on_any_mouse_down(move |_, window, cx| focus.focus(window, cx))
                .into_any_element()
        };
        let files = pane(&p)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(back)
                    .child(div().w(px(4.)))
                    .child(glyph(Glyph::Commit, 18., p.ill_ink))
                    .child(div().font_family(MONO).text_size(px(MONO_SIZE)).child(d.short_id.clone()))
                    .child(div().overflow_hidden().whitespace_nowrap().child(d.summary().to_owned())),
            )
            .child(div().flex_none().h(px(8.)))
            .child(
                div()
                    .flex_none()
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(title("Files"))
                    .child(weak(format!("{} changed", d.files.len()), &p)),
            )
            .child(separator(&p))
            .child(files_body);

        let differs = (&d.committer, &d.committer_email) != (&d.author, &d.email) || d.committer_date != d.date;
        // egui's grid: the keys' column is as wide as its widest key.
        let key_w = if differs { 72. } else { 46. };
        let key = |k: &str| div().w(px(key_w)).flex_none().whitespace_nowrap().text_color(p.fg_dim).child(k.to_owned());
        let fact = |k: &str, v: AnyElement| div().h(px(22.)).flex().items_center().gap(px(14.)).child(key(k)).child(v);
        let parents: Vec<AnyElement> = d
            .parents
            .iter()
            .enumerate()
            .map(|(i, parent)| {
                let summary: SharedString =
                    m.log.iter().find(|c| &c.id == parent).map(|c| c.summary.clone()).unwrap_or_default().into();
                div()
                    .id(("parent", i))
                    .font_family(MONO)
                    .text_size(px(MONO_SIZE))
                    .text_color(if p.dark { p.accent } else { p.fg })
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(parent[..parent.len().min(7)].to_owned())
                    .tooltip(move |w, cx| Tooltip::new(summary.clone()).build(w, cx))
                    .on_click(self.on_click(Action::ShowCommit(parent.clone())))
                    .into_any_element()
            })
            .collect();
        let details = pane(&p)
            .child(div().flex_none().h(px(15.)).flex().items_center().child(title("Commit")))
            .child(separator(&p))
            .child(
                div()
                    .id("commit-details")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_y_scrollbar()
                    .when(!d.refs.is_empty(), |e| {
                        e.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(8.))
                                .pt(px(2.5))
                                .mb(px(6.5))
                                .children(d.refs.iter().map(|r| look::ref_badge(r, &p))),
                        )
                    })
                    .child(div().text_size(px(16.)).child(d.summary().to_owned()))
                    .when(!d.body().is_empty(), |e| e.child(div().mt(px(9.)).child(d.body().to_owned())))
                    .child(div().h(px(13.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(fact(
                                "Author",
                                div().child(format!("{} <{}>", d.author, d.email)).into_any_element(),
                            ))
                            .child(fact("Date", div().child(d.date.clone()).into_any_element()))
                            .when(differs, |e| {
                                e.child(fact(
                                    "Committer",
                                    div().child(format!("{} <{}>", d.committer, d.committer_email)).into_any_element(),
                                ))
                                .child(fact("Committed", div().child(d.committer_date.clone()).into_any_element()))
                            })
                            .child(fact(
                                "SHA",
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().font_family(MONO).text_size(px(look::SMALL)).child(d.id.clone()))
                                    .child(
                                        look::small_button("copy-sha", crate::components::IconName::Copy, "Copy", &p)
                                            .on_click(self.on_click(Action::Copy(d.id.clone()))),
                                    )
                                    .into_any_element(),
                            ))
                            .child(fact(
                                if d.parents.len() > 1 { "Parents" } else { "Parent" },
                                if d.parents.is_empty() {
                                    weak("none, this is the first commit", &p).into_any_element()
                                } else {
                                    div().flex().flex_col().children(parents).into_any_element()
                                },
                            )),
                    ),
            );

        let empty = if d.files.is_empty() { "This commit changes no files" } else { "Select a file" };
        let parent = if d.parents.is_empty() { "(nothing)" } else { "Parent" };
        let diff = pane(&p)
            .child(
                div()
                    .flex_none()
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(title("Changes"))
                    .child(diff_title(view.diff.as_ref(), &p))
                    .child(div().flex_1())
                    .child(diff_controls(&self.model, cx)),
            )
            .child(separator(&p))
            .child(diff_body(
                "commit-diff",
                view.diff.as_ref(),
                (parent, "This commit"),
                m.settings.side_by_side,
                empty,
                cx,
            ));

        let s = &self.sizes;
        let (details_w, changes_h) = (s.details, s.commit_changes);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(div().flex_1().min_w_0().flex().flex_col().child(files))
                    .child(self.handle(Split::Details, cx))
                    .child(div().flex_none().w(px(details_w)).flex().flex_col().child(details)),
            )
            .child(self.handle(Split::CommitChanges, cx))
            .child(div().flex_none().h(px(changes_h)).flex().flex_col().child(diff))
            .into_any_element()
    }

    // ----- Welcome -----------------------------------------------------------------------

    /// No project open: the Git view says so and offers to open one (the Welcome page, with
    /// the recent projects, is the Project view's).
    fn welcome(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let button = |id: &'static str, g: Glyph, label: &'static str, action: Action| {
            look::glyph_button(id, g, label, true, &p).w(px(240.)).h(px(34.)).on_click(self.on_click(action))
        };
        pane(&p)
            .child(
                div()
                    .id("git-empty")
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .child(drawing(snout_core::art::welcome(), 240., 152., None))
                    .child(div().mt(px(6.)).text_size(px(16.)).child("No project open"))
                    .child(div().text_color(p.fg_dim).child("Open a project to see its changes, branches and history."))
                    .child(div().h(px(10.)))
                    .child(button("git-empty-open", Glyph::Open, "Open Project…   ⌘O", Action::PickFolder))
                    .child(button("git-empty-clone", Glyph::Pull, "Clone Repository…", Action::ShowClone)),
            )
            .into_any_element()
    }
}

impl Render for GitView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focused = (
            self.files_focus.contains_focused(window, cx),
            self.commit_focus.contains_focused(window, cx),
            self.journal_focus.contains_focused(window, cx),
        );
        let p = palette(cx);
        let m = self.model.read(cx);
        let has_repo = m.repo.is_some();
        let in_commit = m.commit_view.is_some();
        let console_open = m.settings.console_open;
        let toolbar = self.toolbar(cx);
        let s = &self.sizes;
        let (left_w, branches_h, commit_w, changes_h, journal_h, console_w) =
            (s.left, s.branches, s.commit, s.changes, s.journal, s.console);
        // As in the egui app: the console never so wide that the panes in the middle get
        // less than about 600 points, the commit box at most 45 % of what is left.
        let width = f32::from(window.viewport_size().width) - crate::workspace::ACTIVITY_W;
        let console_w = console_w.min((width - 860.).max(360.));
        let middle = width - left_w - if console_open { console_w } else { 0. };
        let commit_w = commit_w.min((middle * 0.45).max(220.));
        self.narrow_commit = commit_w < 364.;
        self.middle_w = middle;

        let left = div()
            .flex_none()
            .w(px(left_w))
            .h_full()
            .px(px(5.))
            .flex()
            .flex_col()
            .child(pane(&p).child(self.projects.clone()))
            .child(self.handle(Split::Branches, cx))
            .child(div().flex_none().h(px(branches_h)).flex().flex_col().child(self.branches(cx)));

        let center = if !has_repo {
            self.welcome(cx)
        } else {
            let top = if in_commit {
                self.commit_view(cx)
            } else {
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(div().flex_1().min_w_0().flex().flex_col().child(self.files(cx)))
                            .child(self.handle(Split::Commit, cx))
                            .child(div().flex_none().w(px(commit_w)).flex().flex_col().child(self.commit_box(cx))),
                    )
                    .child(self.handle(Split::Changes, cx))
                    .child(div().flex_none().h(px(changes_h)).flex().flex_col().child(self.changes(cx)))
                    .into_any_element()
            };
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().flex_1().min_h_0().child(top))
                .child(self.handle(Split::Journal, cx))
                .child(div().flex_none().h(px(journal_h)).flex().flex_col().child(self.journal(cx)))
                .into_any_element()
        };

        let console = self.console.clone().filter(|_| console_open).map(|view| {
            div().flex_none().w(px(console_w)).h_full().pr(px(5.)).flex().flex_col().child(pane(&p).child(view))
        });

        div()
            .id("git-view")
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_color(p.fg)
            .font_family(look::ui_font(cx))
            .text_size(px(BODY))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.dragging.is_some() {
                    if e.pressed_button == Some(MouseButton::Left) {
                        this.drag_to(e.position, cx);
                    } else {
                        this.dragging = None;
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.dragging = None))
            .child(toolbar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(left)
                    .child(self.handle(Split::Left, cx))
                    .child(div().flex_1().min_w_0().h_full().px(px(5.)).flex().flex_col().child(center))
                    .when_some(console, |d, c| d.child(self.handle(Split::Console, cx)).child(c)),
            )
    }
}

// ----- Pieces ------------------------------------------------------------------------------

/// Path and +/− counts of the diff showing.
fn diff_title(diff: Option<&Result<Rc<DiffLayout>, String>>, p: &Palette) -> AnyElement {
    let Some(Ok(view)) = diff else { return div().into_any_element() };
    let d = &view.diff;
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(div().font_family(MONO).text_size(px(MONO_SIZE)).child(d.path.clone()))
        .when(!d.binary && !d.too_large, |e| {
            e.child(div().text_color(p.added).child(format!("+{}", d.additions)))
                .child(div().text_color(p.removed).child(format!("−{}", d.deletions)))
        })
        .into_any_element()
}

/// Layout toggles shared by every diff.
fn diff_controls(model: &Entity<Model>, cx: &App) -> AnyElement {
    let p = palette(cx);
    let s = &model.read(cx).settings;
    let (sbs, compact) = (s.side_by_side, s.compact);
    let set = |side: Option<bool>, compact: Option<bool>| {
        let model = model.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            model.update(cx, |m, cx| m.set_diff_options(side, compact, cx))
        }
    };
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            look::toggle("uni", crate::components::IconName::Rows2, "Unified", !sbs, &p)
                .on_click(set(Some(false), None)),
        )
        .child(
            look::toggle("sbs", crate::components::IconName::Columns2, "Side by Side", sbs, &p)
                .on_click(set(Some(true), None)),
        )
        .child(look::vseparator(&p))
        .child(
            look::toggle("compact", crate::components::IconName::FoldVertical, "Compact", compact, &p)
                .tooltip(|w, cx| Tooltip::new("Hide unchanged lines away from changes").build(w, cx))
                .on_click(set(None, Some(!compact))),
        )
        .into_any_element()
}

/// The diff itself, or a note saying why there is none.
fn diff_body(
    id: &'static str,
    diff: Option<&Result<Rc<DiffLayout>, String>>,
    (left, right): (&str, &str),
    side_by_side: bool,
    empty: &str,
    cx: &App,
) -> AnyElement {
    let p = palette(cx);
    let view = match diff {
        None => return look::art_note(snout_core::art::pick(), "", empty.to_owned(), cx),
        Some(Err(e)) => return look::centered_note(e.clone(), &p).into_any_element(),
        Some(Ok(view)) => view.clone(),
    };
    let d = &view.diff;
    if d.binary {
        return look::centered_note("Binary file differs", &p).into_any_element();
    }
    if d.too_large {
        return look::centered_note("File too large to display", &p).into_any_element();
    }
    if d.is_identical() {
        return look::centered_note(format!("No differences between {left} and {right}"), &p).into_any_element();
    }
    let count = view.len(side_by_side);
    let digits = view.number_digits;
    let list = uniform_list(id, count, move |range, _, _| {
        range.map(|i| diff_line(&view, i, side_by_side, digits, &p)).collect()
    })
    .flex_1()
    .font_family(MONO)
    .text_size(px(MONO_SIZE));
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .when(side_by_side, |e| {
            e.child(
                div()
                    .flex_none()
                    .h(px(15.))
                    .mb(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_color(p.weak())
                    .child(div().flex_1().child(left.to_owned()))
                    .child(div().flex_1().child(right.to_owned())),
            )
        })
        .child(list)
        .into_any_element()
}

/// A line-number gutter (one column per entry in `numbers`), a marker and the text.
fn cell(
    bg: Option<Hsla>,
    numbers: &[Option<u32>],
    marker: &str,
    text: Option<&str>,
    digits: usize,
    p: &Palette,
) -> Div {
    let num_w = (digits as f32 + 1.0) * CHAR_W;
    let mut row = div()
        .flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .items_center()
        .pl(px(4.))
        .overflow_hidden()
        .when_some(bg, |d, bg| d.bg(bg));
    for n in numbers {
        row = row.child(
            div()
                .flex_none()
                .w(px(num_w))
                .pr(px(CHAR_W * 0.5))
                .text_right()
                .text_color(p.weak())
                .child(n.map(|n| n.to_string()).unwrap_or_default()),
        );
    }
    row = row.child(div().flex_none().w(px(CHAR_W * 0.5)));
    if !marker.is_empty() {
        row = row.child(div().flex_none().w(px(CHAR_W * 1.5)).text_color(p.weak()).child(marker.to_owned()));
    }
    row.child(div().whitespace_nowrap().child(text.unwrap_or_default().replace('\t', "    ")))
}

fn diff_line(view: &DiffLayout, i: usize, side_by_side: bool, digits: usize, p: &Palette) -> AnyElement {
    let rows = &view.diff.rows;
    let gap = |n: usize| {
        div()
            .h(px(DIFF_ROW))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(p.input)
            .text_color(p.weak())
            .child(gap_label(n))
            .into_any_element()
    };
    let line = div().h(px(DIFF_ROW)).w_full().flex();
    if side_by_side {
        let row = match view.sbs[i] {
            SbsLine::Gap(n) => return gap(n),
            SbsLine::Row(i) => &rows[i],
        };
        let (old_bg, new_bg) = match row.kind {
            RowKind::Equal => (None, None),
            RowKind::Delete => (Some(p.diff_del()), None),
            RowKind::Insert => (None, Some(p.diff_add())),
            RowKind::Replace => (Some(p.diff_del()), Some(p.diff_add())),
        };
        let old_bg = if row.old.is_none() { Some(p.stripe) } else { old_bg };
        let new_bg = if row.new.is_none() { Some(p.stripe) } else { new_bg };
        line.child(cell(
            old_bg,
            &[row.old.as_ref().map(|l| l.0)],
            "",
            row.old.as_ref().map(|l| l.1.as_str()),
            digits,
            p,
        ))
        .child(
            div().flex_none().w(px(2.)).h_full().flex().justify_center().child(div().w(px(1.)).h_full().bg(p.border)),
        )
        .child(cell(new_bg, &[row.new.as_ref().map(|l| l.0)], "", row.new.as_ref().map(|l| l.1.as_str()), digits, p))
        .into_any_element()
    } else {
        match view.unified[i] {
            UniLine::Gap(n) => gap(n),
            UniLine::Both(i) => {
                let r = &rows[i];
                let (o, n) = (r.old.as_ref().unwrap(), r.new.as_ref().unwrap());
                line.child(cell(None, &[Some(o.0), Some(n.0)], " ", Some(&n.1), digits, p)).into_any_element()
            }
            UniLine::Old(i) => {
                let o = rows[i].old.as_ref().unwrap();
                line.child(cell(Some(p.diff_del()), &[Some(o.0), None], "-", Some(&o.1), digits, p)).into_any_element()
            }
            UniLine::New(i) => {
                let n = rows[i].new.as_ref().unwrap();
                line.child(cell(Some(p.diff_add()), &[None, Some(n.0)], "+", Some(&n.1), digits, p)).into_any_element()
            }
        }
    }
}

fn lane_x(bounds: Bounds<Pixels>, lane: usize) -> Pixels {
    bounds.left() + px(LANE_W * 0.5 + 4.0 + lane as f32 * LANE_W)
}

/// Paint one row of the graph. `filled` paints the commit solid (not pushed yet); `head`
/// adds a ring.
fn paint_graph(bounds: Bounds<Pixels>, row: &GraphRow, filled: bool, head: bool, p: &Palette, window: &mut Window) {
    let colors = p.lanes();
    let color = |lane: usize| colors[lane % colors.len()];
    let (top, mid, bottom) = (bounds.top(), bounds.center().y, bounds.bottom());
    for e in &row.edges {
        let (y0, y1) = if e.top { (top, mid) } else { (mid, bottom) };
        let (a, b) = (point(lane_x(bounds, e.from), y0), point(lane_x(bounds, e.to), y1));
        // A line keeps the colour of the lane it belongs to: the branch it comes from when
        // it converges, the branch it goes to when it forks.
        let c = color(if e.top { e.from } else { e.to });
        let mut path = PathBuilder::stroke(px(1.6));
        path.move_to(a);
        if a.x == b.x {
            path.line_to(b);
        } else {
            let k = (b.y - a.y) * 0.6;
            path.cubic_bezier_to(b, point(a.x, a.y + k), point(b.x, b.y - k));
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, c);
        }
    }
    let c = point(lane_x(bounds, row.lane), mid);
    let lane = color(row.lane);
    let dot = |r: f32| Bounds::new(point(c.x - px(r), c.y - px(r)), gpui::size(px(2. * r), px(2. * r)));
    if head {
        window.paint_quad(quad(dot(6.5), px(6.5), gpui::transparent_black(), px(1.5), lane, BorderStyle::Solid));
    }
    window.paint_quad(quad(dot(4.0), px(4.0), if filled { lane } else { p.panel }, px(1.8), lane, BorderStyle::Solid));
}

/// "SnoutCode" with its mark beside it and a green stroke under it, after the website's lockup.
pub(crate) fn wordmark(p: &Palette) -> impl IntoElement {
    let accent = p.ill_accent;
    div().flex().items_center().gap(px(10.)).child(drawing(snout_core::art::mark(), 52., 44., None)).child(
        div().relative().text_size(px(40.)).child("SnoutCode").child(
            canvas(
                |_, _, _| {},
                move |b, _, window, _| {
                    let y = b.bottom() - px(1.);
                    let (x0, x1) = (b.left() + px(2.), b.right() - px(2.));
                    let mut path = PathBuilder::stroke(px(4.));
                    path.move_to(point(x0, y + px(2.)));
                    path.cubic_bezier_to(
                        point(x1, y - px(1.)),
                        point(x0 + px(20.), y - px(2.)),
                        point(x1 - px(30.), y - px(3.)),
                    );
                    if let Ok(path) = path.build() {
                        window.paint_path(path, accent);
                    }
                },
            )
            .absolute()
            .inset_0(),
        ),
    )
}
