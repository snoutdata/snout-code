//! The Git Panel in the left dock: the branches and tags, the changed files, and the commit
//! box (commit without staging, amend, and a message drafted by Claude Code). The diff of
//! the selected file opens as a center tab.

use std::collections::HashSet;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyView, AppContext as _, ClickEvent, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px, uniform_list,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::spinner::Spinner;
use gpui_component::tooltip::Tooltip;
use gpui_component::{Disableable as _, Sizable as _};
use snout_core::{Branch, BranchKind, CheckoutTarget, FileEntry, Tag, on_disk, stage_paths, unstage_paths};

use crate::model::{Action, Model, ModelEvent, SortColumn, StartPoint};
use crate::theme::{Palette, palette};
use crate::ui::{chip, icon, icons, pane_title, state_color};

pub fn build<T>(model: &Entity<Model>, window: &mut Window, cx: &mut Context<T>) -> AnyView {
    let model = model.clone();
    cx.new(|cx| GitPanel::new(model, window, cx)).into()
}

pub struct GitPanel {
    model: Entity<Model>,
    filter: Entity<InputState>,
    message: Entity<TextareaState>,
    /// Branch sections folded or unfolded against their default.
    toggled: HashSet<String>,
    /// The branch row under the pointer, which shows a trash can.
    hovered: Option<String>,
}

impl GitPanel {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("File filter"));
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
        Self { model, filter, message, toggled: HashSet::new(), hovered: None }
    }

    fn send(&mut self, action: Action, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.handle(action, cx));
    }

    fn is_open(&self, key: &str, default: bool) -> bool {
        default != self.toggled.contains(key)
    }

    fn section(&self, key: &str, title: String, default: bool, cx: &mut Context<Self>) -> (bool, AnyElement) {
        let open = self.is_open(key, default);
        let p = palette(cx);
        let k = key.to_owned();
        let header = div()
            .id(SharedString::from(format!("section-{key}")))
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(22.))
            .rounded(px(4.))
            .hover(|s| s.bg(p.button_hover))
            .child(icon(if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }).xsmall().text_color(p.fg_dim))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.toggled.remove(&k) {
                    this.toggled.insert(k.clone());
                }
                cx.notify();
            }));
        (open, header.into_any_element())
    }

    fn branches(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let has_repo = m.repo.is_some();
        let idle = m.job.is_none();
        let detached = (m.status.branch.head.is_none() && has_repo).then(|| m.head_label());
        let branches = m.branches.clone();
        let tags = m.tags.clone();
        let mut rows: Vec<AnyElement> = Vec::new();
        if !has_repo {
            rows.push(div().text_color(p.fg_dim).child("No repository open").into_any_element());
        }
        if let Some(label) = detached {
            rows.push(
                div()
                    .id("detached")
                    .flex()
                    .gap(px(4.))
                    .child(div().w(px(12.)).child("●"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("HEAD ({label})")))
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
        let tool = |id: &'static str, glyph: &'static str, tip: &'static str, enabled: bool, action: Action| {
            Button::new(id)
                .ghost()
                .xsmall()
                .icon(icon(glyph))
                .tooltip(tip)
                .disabled(!enabled)
                .on_click(cx.listener(move |this, _, _, cx| this.send(action.clone(), cx)))
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(pane_title("Branches"))
                    .child(div().flex_1())
                    .child(tool("br-checkout", icons::SWITCH, "Check out… (⇧⌘O)", has_repo, Action::ShowCheckout))
                    .child(tool("br-new", icons::BRANCH, "New branch", has_repo, Action::ShowNewBranch(None)))
                    .child(tool("br-fetch", icons::FETCH, "Fetch", has_repo && idle, Action::Fetch)),
            )
            .child(div().id("branch-list").flex_1().flex().flex_col().overflow_y_scrollbar().children(rows))
    }

    fn branch_row(&mut self, b: &Branch, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut label = match b.kind {
            BranchKind::Local => b.name.clone(),
            BranchKind::Remote => b.local_name().to_owned(),
        };
        if b.ahead > 0 {
            label += &format!("  ↑{}", b.ahead);
        }
        if b.behind > 0 {
            label += &format!("  ↓{}", b.behind);
        }
        if b.upstream_gone {
            label += "  (gone)";
        }
        let tip = format!(
            "{}{}\n{} {}\n\nDouble-click to check out",
            b.name,
            b.upstream.as_ref().map(|u| format!(" → {u}")).unwrap_or_default(),
            b.target,
            b.subject
        );
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
        div()
            .id(SharedString::from(format!("branch-{key}")))
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(22.))
            .pl(px(10.))
            .rounded(px(4.))
            .hover(|s| s.bg(p.button_hover))
            .tooltip({
                let t: SharedString = (tip).into();
                move |w, cx| Tooltip::new(t.clone()).build(w, cx)
            })
            .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                this.hovered = on.then(|| hover_key.clone());
                cx.notify();
            }))
            .child(div().w(px(12.)).flex_none().child(if b.is_head { "●" } else { "" }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .when(b.is_head, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(label),
            )
            .when(hovered && !b.is_head, |d| {
                d.child(
                    Button::new("trash")
                        .ghost()
                        .xsmall()
                        .icon(icon(icons::TRASH))
                        .tooltip(delete_label.clone())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.send(Action::AskDeleteBranch(b3.clone()), cx)),
                        ),
                )
            })
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 && !b1.is_head {
                    this.send(Action::RequestCheckout((&b1).into()), cx);
                }
            }))
            .context_menu(move |menu, _, _| {
                let b = b2.clone();
                let send = |a: Action| {
                    let model = model.clone();
                    move |_: &ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        let a = a.clone();
                        model.update(cx, |m, cx| m.handle(a, cx))
                    }
                };
                menu.item(
                    PopupMenuItem::new(checkout_label.clone())
                        .disabled(b.is_head)
                        .on_click(send(Action::RequestCheckout((&b).into()))),
                )
                .item(PopupMenuItem::new("Create branch from here…").on_click(send(Action::ShowNewBranch(Some(
                    StartPoint { label: b.name.clone(), rev: b.name.clone() },
                )))))
                .item(
                    PopupMenuItem::new(delete_label.clone())
                        .disabled(b.is_head)
                        .on_click(send(Action::AskDeleteBranch(b.clone()))),
                )
                .separator()
                .item(PopupMenuItem::new("Copy name").on_click(send(Action::Copy(b.name.clone()))))
            })
            .into_any_element()
    }

    fn tag_row(&mut self, t: &Tag, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let tip = format!("{} {}\n{}\n\nDouble-click to check out (detached)", t.target, t.subject, t.date);
        let target = CheckoutTarget::Tag(t.name.clone());
        let (t1, name) = (target.clone(), t.name.clone());
        let model = self.model.clone();
        div()
            .id(SharedString::from(format!("tag-{}", t.name)))
            .flex()
            .items_center()
            .h(px(22.))
            .pl(px(26.))
            .rounded(px(4.))
            .hover(|s| s.bg(p.button_hover))
            .tooltip({
                let t: SharedString = (tip).into();
                move |w, cx| Tooltip::new(t.clone()).build(w, cx)
            })
            .child(t.name.clone())
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    this.send(Action::RequestCheckout(t1.clone()), cx);
                }
            }))
            .context_menu(move |menu, _, _| {
                let send = |a: Action| {
                    let model = model.clone();
                    move |_: &ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        let a = a.clone();
                        model.update(cx, |m, cx| m.handle(a, cx))
                    }
                };
                let start = StartPoint { label: format!("tag {name}"), rev: format!("refs/tags/{name}") };
                menu.item(
                    PopupMenuItem::new("Check out (detached)").on_click(send(Action::RequestCheckout(target.clone()))),
                )
                .item(PopupMenuItem::new("Create branch from tag…").on_click(send(Action::ShowNewBranch(Some(start)))))
                .separator()
                .item(PopupMenuItem::new("Copy name").on_click(send(Action::Copy(name.clone()))))
            })
            .into_any_element()
    }

    fn files(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
        let sort = m.sort;
        let root = m.repo.as_ref().map(|r| r.root().to_owned());
        let view = cx.entity();
        let model = self.model.clone();
        let list = uniform_list("files", rows.len(), move |range, _, cx| {
            range
                .map(|i| {
                    let f = &rows[i];
                    let is_selected = selected.contains(&f.path);
                    let entries: Vec<FileEntry> =
                        if is_selected { model.read(cx).selected_entries() } else { vec![f.clone()] };
                    let path = f.path.clone();
                    let (model, root, view) = (model.clone(), root.clone(), view.clone());
                    let f2 = f.clone();
                    div()
                        .id(("file", i))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(24.))
                        .px(px(6.))
                        .rounded(px(4.))
                        .when(is_selected, |d| d.bg(p.selection()))
                        .when(!is_selected, |d| d.hover(|s| s.bg(p.button_hover)))
                        .child(
                            div()
                                .flex_none()
                                .w(px(140.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(f.file_name().to_owned()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(118.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_color(state_color(f, &p))
                                .child(f.state_label()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_color(p.fg_dim)
                                .child(f.directory().to_owned()),
                        )
                        .on_click({
                            let model = model.clone();
                            move |e: &ClickEvent, _, cx| {
                                let mods = e.modifiers();
                                model.update(cx, |m, cx| m.click_file(&path, mods.shift, mods.platform, cx));
                            }
                        })
                        .on_mouse_down(gpui::MouseButton::Right, {
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
                                move |_: &ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    let a = a.clone();
                                    model.update(cx, |m, cx| m.handle(a, cx))
                                }
                            };
                            let _ = &view;
                            let mut menu = menu
                                .item(
                                    PopupMenuItem::new("Stage")
                                        .disabled(!entries.iter().any(FileEntry::has_unstaged))
                                        .on_click(send(Action::Stage(stage_paths(&entries)))),
                                )
                                .item(
                                    PopupMenuItem::new("Unstage")
                                        .disabled(!entries.iter().any(FileEntry::has_staged))
                                        .on_click(send(Action::Unstage(unstage_paths(&entries)))),
                                )
                                .item(
                                    PopupMenuItem::new("Discard…").on_click(send(Action::AskDiscard(entries.clone()))),
                                )
                                .item(
                                    PopupMenuItem::new("Delete…")
                                        .disabled(!entries.iter().any(on_disk))
                                        .on_click(send(Action::AskDeleteFiles(entries.clone()))),
                                )
                                .separator();
                            if let Some(root) = &root {
                                menu = menu.item(
                                    PopupMenuItem::new("Reveal in Finder")
                                        .on_click(send(Action::Reveal(root.join(&f2.path)))),
                                );
                            }
                            menu.item(PopupMenuItem::new("Copy path").on_click(send(Action::Copy(f2.path.clone()))))
                        })
                })
                .collect()
        })
        .flex_1();
        let header = self.file_header(sort, &p, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(pane_title("Files"))
                    .child(div().text_color(p.fg_dim).text_size(px(12.)).child(summary))
                    .when_some(dir, |d, dir| {
                        d.child(
                            div()
                                .id("dir-chip")
                                .child(chip(format!("in {dir}/  ✕"), p.fg_dim, &p))
                                .tooltip({
                                    let t: SharedString = ("Show all files").into();
                                    move |w, cx| Tooltip::new(t.clone()).build(w, cx)
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.send(Action::FilterDir(None), cx))),
                        )
                    }),
            )
            .child(Input::new(&self.filter).small())
            .when(clean, |d| d.child(crate::ui::note("The working tree is clean", clean_text, cx)))
            .when(!clean, |d| d.child(header).child(list))
    }

    /// The column headers over the changed files; a click sorts by that column.
    fn file_header(&self, (by, asc): (SortColumn, bool), p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let fg = p.fg;
        let column = |id: &'static str, label: &'static str, col: SortColumn, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(2.))
                .overflow_hidden()
                .whitespace_nowrap()
                .rounded(px(4.))
                .hover(move |s| s.text_color(fg))
                .child(label)
                .when(by == col, |d| d.child(icon(if asc { icons::CHEVRON_UP } else { icons::CHEVRON_DOWN }).xsmall()))
                .on_click(cx.listener(move |this, _, _, cx| this.model.update(cx, |m, cx| m.sort_by(col, cx))))
        };
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(6.))
            .h(px(20.))
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(p.fg_dim)
            .child(column("sort-name", "Name", SortColumn::Name, cx).flex_none().w(px(140.)))
            .child(column("sort-state", "State", SortColumn::State, cx).flex_none().w(px(118.)))
            .child(column("sort-dir", "Relative Directory", SortColumn::Directory, cx).flex_1().min_w_0())
            .into_any_element()
    }

    fn commit_box(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let scope = m.commit_scope();
        let has_changes = m.repo.is_some() && scope != snout_core::Scope::Nothing;
        let drafting = m.draft.is_some();
        let can_commit = m.can_commit();
        let amend = m.amend;
        let has_log = !m.log.is_empty();
        let draft_tip = if has_changes {
            format!("Claude Code writes the message from {}, building on anything you typed", scope.what())
        } else {
            "Nothing to commit".to_owned()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(pane_title("Commit"))
                    .child(div().flex_1())
                    .when(drafting, |d| {
                        d.child(Spinner::new().xsmall()).child(
                            Button::new("stop-draft")
                                .xsmall()
                                .icon(icon(icons::CLOSE))
                                .label("Stop")
                                .on_click(cx.listener(|this, _, _, cx| this.send(Action::CancelDraft, cx))),
                        )
                    })
                    .when(!drafting, |d| {
                        d.child(
                            Button::new("draft")
                                .xsmall()
                                .icon(icon(icons::SPARK))
                                .label("Draft")
                                .tooltip(draft_tip)
                                .disabled(!has_changes)
                                .on_click(cx.listener(|this, _, _, cx| this.send(Action::DraftMessage, cx))),
                        )
                    }),
            )
            .child(div().flex_1().min_h(px(60.)).child(Textarea::new(&self.message).h_full().disabled(drafting)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Checkbox::new("amend").label("Amend last commit").checked(amend).disabled(!has_log).on_click(
                            cx.listener(|this, v: &bool, _, cx| {
                                let v = *v;
                                this.model.update(cx, |m, cx| m.set_amend(v, cx))
                            }),
                        ),
                    )
                    .child(div().flex_1())
                    .child(div().text_color(p.fg_dim).text_size(px(12.)).child(scope.label()))
                    .child(
                        Button::new("commit")
                            .primary()
                            .label("Commit")
                            .tooltip(format!("Commits {}  ⌘↩", scope.what()))
                            .disabled(!can_commit)
                            .on_click(cx.listener(|this, _, _, cx| this.send(Action::Commit, cx))),
                    ),
            )
    }
}

impl Render for GitPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        if self.model.read(cx).repo.is_none() {
            return div().size_full().bg(p.panel).child(crate::ui::note(
                "",
                "Open a repository to see its changes",
                cx,
            ));
        }
        let branches = self.branches(cx).into_any_element();
        let files = self.files(cx).into_any_element();
        let commit = self.commit_box(cx).into_any_element();
        div().size_full().bg(p.panel).child(
            v_resizable("git-panel")
                .child(resizable_panel().size(px(220.)).child(branches))
                .child(resizable_panel().child(files))
                .child(resizable_panel().size(px(230.)).child(commit)),
        )
    }
}
