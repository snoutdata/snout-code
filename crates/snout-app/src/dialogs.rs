//! The dialogs the model asks for, on GPUI Kit's dialog layer, drawn with the app's own
//! components: a header with the dialog's spot drawing and title, its body, and a footer of
//! buttons with their keys. Each dialog's content is its own view ([`DialogBody`]), so its
//! fields and toggles redraw it as they change.
//!
//! ⏎ does what the main button does, unless that destroys something: Discard, Delete and
//! the like take a click or ⌘⌫. Esc cancels.

use std::path::Path;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, actions, div, px,
};
use gpui_component::WindowExt as _;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use snout_core::art::{Drawing, spots};
use snout_core::groups::{dir_name, group_name_problem};
use snout_core::{BranchKind, CheckoutTarget, LocalChanges, PullMode, PullProblem, branch_name_problem, repo_dir_name};

use crate::components::{
    Button, FileChip, IconButton, IconName, Tone, badge, callout, choice_card, dialog_body, dialog_footer,
    dialog_header, field_label,
};
use crate::menus::{SOURCE_URL, WEBSITE_URL};
use crate::model::{Action, Dialog, Model, Mood};
use crate::theme::palette;
use crate::ui::state_color;

actions!(snout_dialog, [ConfirmDestructive]);

/// Put up the dialog the model is waiting with.
pub fn open(model: &Entity<Model>, window: &mut Window, cx: &mut gpui::App) {
    let Some(dialog) = model.read(cx).dialog.clone() else { return };
    if window.has_active_dialog(cx) {
        window.close_all_dialogs(cx);
    }
    let width = match dialog {
        Dialog::Checkout => 600.,
        Dialog::Error(_) | Dialog::PullProblem { .. } | Dialog::Clone { .. } | Dialog::ConfirmCheckout(_) => 560.,
        _ => 520.,
    };
    let p = palette(cx);
    let body = cx.new(|cx| DialogBody::new(model.clone(), dialog, window, cx));
    let body_focus = body.clone();
    let m = model.clone();
    window.open_dialog(cx, move |d, _, _| {
        let m = m.clone();
        let enter = body.clone();
        d.w(px(width))
            .p_0()
            .bg(p.card)
            .rounded(px(12.))
            .close_button(false)
            .child(body.clone())
            .on_ok(move |_, window, cx| {
                enter.update(cx, |b, cx| b.enter(window, cx));
                false
            })
            .on_close(move |_, _, cx| {
                m.update(cx, |m, _| m.dialog = None);
            })
    });
    // After the dialog layer has taken the keyboard for itself.
    window.defer(cx, move |window, cx| body_focus.update(cx, |b, cx| b.take_focus(window, cx)));
}

/// What a dialog looks like: its tone, its drawing, and the line under its title.
fn look(d: &Dialog, m: &Model) -> (Tone, Option<&'static Drawing>, Option<SharedString>) {
    let files = |n: usize| if n == 1 { "this file".to_owned() } else { format!("these {n} files") };
    match d {
        Dialog::About => (
            Tone::Neutral,
            Some(snout_core::art::mark()),
            Some(format!("Version {}", env!("CARGO_PKG_VERSION")).into()),
        ),
        Dialog::Error(msg) => {
            let art = if msg.contains("[rejected]") || msg.contains("failed to push") {
                spots::push_rejected()
            } else if msg.contains("CONFLICT") || msg.contains("conflict") {
                spots::merge_conflict()
            } else {
                spots::warning()
            };
            (Tone::Danger, Some(art), Some("Git stopped and said why.".into()))
        }
        Dialog::Info { mood: Mood::Warning, .. } => (Tone::Warning, Some(spots::warning()), None),
        Dialog::Info { mood: Mood::Done, .. } => (Tone::Success, Some(spots::success()), None),
        Dialog::Info { .. } => (Tone::Info, Some(spots::no_results()), None),
        Dialog::ConfirmDiscard(entries) => {
            let it = if entries.len() == 1 { "it is" } else { "they are" };
            let text = format!("This puts {} back as {it} in HEAD. Untracked files are deleted.", files(entries.len()));
            (Tone::Danger, Some(spots::discard()), Some(text.into()))
        }
        Dialog::ConfirmDeleteFiles(entries) => {
            let one = entries.len() == 1;
            let what = capital(&files(entries.len()));
            let text = if snout_core::can_trash(&*m.host) {
                format!(
                    "{what} {} to the Trash. Tracked files then show as deleted, ready to commit.",
                    if one { "goes" } else { "go" }
                )
            } else {
                format!(
                    "{what} {} deleted from disk. Tracked files then show as deleted, ready to commit.",
                    if one { "is" } else { "are" }
                )
            };
            (Tone::Danger, Some(spots::discard()), Some(text.into()))
        }
        Dialog::ConfirmCheckout(_) => {
            (Tone::Git, Some(spots::stash()), Some("You have uncommitted changes. What should happen to them?".into()))
        }
        Dialog::Checkout => (Tone::Git, None, Some("A branch, a tag, or any commit.".into())),
        Dialog::NewBranch(_) => {
            (Tone::Git, Some(spots::new_branch()), Some("A line of work of its own, from where you choose.".into()))
        }
        Dialog::DeleteBranch { .. } => {
            (Tone::Danger, Some(spots::discard()), Some("Its commits stay wherever else they are reachable.".into()))
        }
        Dialog::DeleteRemoteBranch(_) => (
            Tone::Danger,
            Some(spots::discard()),
            Some("It goes for everyone who uses the remote. Local branches stay as they are.".into()),
        ),
        Dialog::PullProblem { problem: PullProblem::Divergent, .. } => {
            (Tone::Warning, Some(spots::merge_conflict()), None)
        }
        Dialog::PullProblem { .. } => (Tone::Warning, Some(spots::push_rejected()), None),
        Dialog::InitRepo(path) => (Tone::Git, Some(spots::no_project()), Some(path.display().to_string().into())),
        Dialog::GroupName { .. } => (Tone::Info, Some(spots::no_project()), None),
        Dialog::Clone { .. } => (
            Tone::Git,
            Some(spots::clone_repo()),
            Some("Copy a remote repository to this computer and open it.".into()),
        ),
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn title(d: &Dialog, m: &Model) -> SharedString {
    match d {
        Dialog::About => "About SnoutCode".into(),
        Dialog::Error(_) => "Something went wrong".into(),
        Dialog::Info { title, .. } => title.clone().into(),
        Dialog::ConfirmDiscard(_) => "Discard changes?".into(),
        Dialog::ConfirmCheckout(t) => format!("Check out {}?", t.label()).into(),
        Dialog::Checkout => "Check out".into(),
        Dialog::NewBranch(_) => "New branch".into(),
        Dialog::DeleteBranch { .. } => "Delete branch?".into(),
        Dialog::DeleteRemoteBranch(r) => format!("Delete branch from {}?", r.remote).into(),
        Dialog::ConfirmDeleteFiles(_) if snout_core::can_trash(&*m.host) => "Move to the Trash?".into(),
        Dialog::ConfirmDeleteFiles(_) => "Delete files?".into(),
        Dialog::PullProblem { problem: PullProblem::Divergent, .. } => "Both sides have new commits".into(),
        Dialog::PullProblem { .. } => "No shared history".into(),
        Dialog::InitRepo(_) => "Not a Git repository".into(),
        Dialog::GroupName { rename: Some(_), .. } => "Rename group".into(),
        Dialog::GroupName { .. } => "New group".into(),
        Dialog::Clone { .. } => {
            let _ = m;
            "Clone repository".into()
        }
    }
}

pub struct DialogBody {
    model: Entity<Model>,
    dialog: Dialog,
    name: Entity<InputState>,
    url: Entity<InputState>,
    parent: Entity<InputState>,
    checkout: bool,
    force: bool,
    with_upstream: bool,
    local: LocalChanges,
    cursor: usize,
    /// Holds the keyboard in a dialog with no field, so ⌘⌫ and ⏎ reach it.
    focus: FocusHandle,
}

impl DialogBody {
    fn new(model: Entity<Model>, dialog: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (placeholder, value) = match &dialog {
            Dialog::NewBranch(_) => ("feature/my-idea", String::new()),
            Dialog::GroupName { rename, .. } => ("Work", rename.clone().unwrap_or_default()),
            Dialog::Checkout => ("Filter branches and tags, or enter a commit (SHA, HEAD~2, …)", String::new()),
            _ => ("", String::new()),
        };
        let name = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder(placeholder);
            s.set_value(value, window, cx);
            s
        });
        let url = cx.new(|cx| InputState::new(window, cx).placeholder("https://github.com/owner/repo.git"));
        let parent_dir = match &dialog {
            Dialog::Clone { parent } => parent.clone(),
            _ => String::new(),
        };
        let parent = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(parent_dir, window, cx);
            s
        });
        for input in [&name, &url, &parent] {
            cx.subscribe_in(input, window, |this, input, e: &InputEvent, window, cx| match e {
                InputEvent::Change => {
                    if matches!(this.dialog, Dialog::NewBranch(_)) {
                        // Branch names cannot hold spaces; most people mean a dash.
                        let v = input.read(cx).value();
                        if v.contains(' ') {
                            let fixed = v.replace(' ', "-");
                            input.update(cx, |s, cx| s.set_value(fixed, window, cx));
                        }
                    }
                    this.cursor = 0;
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.confirm(window, cx),
                _ => {}
            })
            .detach();
        }
        let focus = cx.focus_handle();
        Self {
            focus,
            model,
            dialog,
            name,
            url,
            parent,
            checkout: true,
            force: false,
            with_upstream: false,
            local: LocalChanges::Keep,
            cursor: 0,
        }
    }

    /// Put the keyboard in the dialog: its first field, else the dialog itself (so ⌘⌫
    /// and ⏎ reach it).
    fn take_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.dialog {
            Dialog::Clone { .. } => self.url.update(cx, |s, cx| s.focus(window, cx)),
            Dialog::NewBranch(_) | Dialog::GroupName { .. } | Dialog::Checkout => {
                self.name.update(cx, |s, cx| s.focus(window, cx))
            }
            _ => window.focus(&self.focus, cx),
        }
    }

    fn act(&self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        self.model.update(cx, |m, cx| {
            m.dialog = None;
            m.handle(action, cx);
        });
    }

    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        self.model.update(cx, |m, _| m.dialog = None);
    }

    fn value(input: &Entity<InputState>, cx: &gpui::App) -> String {
        input.read(cx).value().trim().to_owned()
    }

    /// What the primary button would do now, if anything.
    fn primary(&self, cx: &gpui::App) -> Option<Action> {
        let m = self.model.read(cx);
        let name = Self::value(&self.name, cx);
        match &self.dialog {
            Dialog::NewBranch(start) => (!name.is_empty() && branch_name_problem(&name).is_none()).then(|| {
                Action::CreateBranch { name, start: start.as_ref().map(|s| s.rev.clone()), checkout: self.checkout }
            }),
            Dialog::GroupName { rename, repo } => (!name.is_empty()
                && group_name_problem(&m.settings.groups, &name, rename.as_deref()).is_none())
            .then(|| Action::SaveGroup { rename: rename.clone(), name, repo: repo.clone() }),
            Dialog::Clone { .. } => {
                let url = Self::value(&self.url, cx);
                let parent = Self::value(&self.parent, cx);
                let dest = repo_dir_name(&url).map(|n| Path::new(&parent).join(n));
                dest.filter(|_| !parent.is_empty()).map(|dest| Action::Clone { url, dest })
            }
            Dialog::Checkout => {
                let items = pick_items(m, &name);
                items.into_iter().nth(self.cursor).map(|i| Action::Checkout(i.target, self.local))
            }
            _ => None,
        }
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = self.primary(cx) {
            self.act(a, window, cx);
        }
    }

    /// ⏎: the main button, unless it destroys something.
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.dialog {
            Dialog::About | Dialog::Error(_) | Dialog::Info { .. } => self.close(window, cx),
            Dialog::NewBranch(_) | Dialog::GroupName { .. } | Dialog::Clone { .. } | Dialog::Checkout => {
                self.confirm(window, cx)
            }
            _ => {}
        }
    }

    /// What the destructive button does, for ⌘⌫.
    fn destructive(&self) -> Option<Action> {
        match self.dialog.clone() {
            Dialog::ConfirmDiscard(entries) => Some(Action::Discard(entries)),
            Dialog::ConfirmDeleteFiles(entries) => Some(Action::DeleteFiles(entries)),
            Dialog::DeleteBranch { name, upstream } => Some(Action::DeleteBranch {
                name,
                force: self.force,
                upstream: upstream.filter(|_| self.with_upstream),
            }),
            Dialog::DeleteRemoteBranch(r) => Some(Action::DeleteRemoteBranch(r)),
            _ => None,
        }
    }
}

struct PickItem {
    target: CheckoutTarget,
    label: String,
    detail: String,
    section: &'static str,
}

/// Candidates for the picker, filtered by `query`. Remote branches with a local
/// counterpart are left out: checking them out would just switch to the local one.
fn pick_items(m: &Model, query: &str) -> Vec<PickItem> {
    let q = query.to_lowercase();
    let matches = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
    let locals: Vec<&str> =
        m.branches.iter().filter(|b| b.kind == BranchKind::Local).map(|b| b.name.as_str()).collect();
    let mut items = Vec::new();
    for b in m.branches.iter().filter(|b| matches(&b.name)) {
        let (section, include) = match b.kind {
            BranchKind::Local => ("Local branches", !b.is_head),
            BranchKind::Remote => ("Remote branches", !locals.contains(&b.local_name())),
        };
        if include {
            let detail = format!("{} {}", b.target, b.subject);
            items.push(PickItem { target: b.into(), label: b.name.clone(), detail, section });
        }
    }
    items.sort_by_key(|i| i.section != "Local branches");
    for t in m.tags.iter().filter(|t| matches(&t.name)) {
        let detail = format!("{} {}", t.target, t.subject);
        items.push(PickItem {
            target: CheckoutTarget::Tag(t.name.clone()),
            label: t.name.clone(),
            detail,
            section: "Tags",
        });
    }
    // A query that names a commit but nothing above exactly: offer the commit itself.
    if !query.is_empty()
        && !items.iter().any(|i| i.label == query)
        && let Some(described) = m.repo.as_ref().and_then(|r| r.describe_commit(query).ok())
    {
        let target = CheckoutTarget::Commit(query.to_owned());
        items.push(PickItem { target, label: query.to_owned(), detail: described, section: "Commit" });
    }
    items
}

impl Render for DialogBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let (tone, art, subtitle) = look(&self.dialog, m);
        let heading = title(&self.dialog, m);
        let dim = |t: String| div().text_color(p.fg_dim).line_height(px(19.)).child(t);
        let code = |t: &str| {
            div()
                .id("code")
                .max_h(px(220.))
                .overflow_y_scrollbar()
                .px(px(10.))
                .py(px(8.))
                .rounded(px(8.))
                .bg(p.input)
                .border_1()
                .border_color(p.border)
                .font_family(gpui_component::Theme::global(cx).mono_font_family.clone())
                .text_size(px(12.))
                .line_height(px(18.))
                .child(t.to_owned())
        };
        // The files a dialog is about, each as a chip; a long list scrolls.
        let files = |entries: &[snout_core::FileEntry]| {
            div().id("files").max_h(px(204.)).overflow_y_scrollbar().flex().flex_col().gap(px(6.)).children(
                entries.iter().enumerate().map(|(i, e)| {
                    FileChip::new(("file", i), e.path.clone()).status(e.state_label(), state_color(e, &p)).max_chars(46)
                }),
            )
        };
        let primary_enabled = self.primary(cx).is_some();
        let branch = m.status.branch.head.clone().unwrap_or_else(|| "HEAD".into());
        let upstream = m.status.branch.upstream.clone().unwrap_or_else(|| "its upstream".into());
        let has_tracked = m.status.files.iter().any(|f| !f.is_untracked());
        let has_changes = !m.status.files.is_empty();

        let mut body: Vec<AnyElement> = Vec::new();
        let mut buttons: Vec<AnyElement> = Vec::new();
        let ok = |label: &str| Button::new("ok").primary().focusable().label(label.to_owned()).kbd("⏎");
        let danger = |label: &str| {
            Button::new("danger").danger().focusable().icon(IconName::Trash).label(label.to_owned()).kbd("⌘⌫")
        };
        let cancel = Button::new("cancel")
            .focusable()
            .label("Cancel")
            .kbd("Esc")
            .on_click(cx.listener(|this, _, w, cx| this.close(w, cx)))
            .into_any_element();
        macro_rules! on {
            ($b:expr, $action:expr) => {{
                let action = $action;
                $b.on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.act(action.clone(), w, cx)))
                    .into_any_element()
            }};
        }

        match self.dialog.clone() {
            Dialog::About => {
                body.push(
                    div()
                        .line_height(px(19.))
                        .child("An IDE for Java, Gradle and Spring Boot, with Git and Claude Code built in.")
                        .into_any_element(),
                );
                body.push(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            Button::new("src")
                                .compact()
                                .icon(IconName::Github)
                                .label("github.com/snoutdata/snout-code")
                                .on_click(|_, _, cx| cx.open_url(SOURCE_URL)),
                        )
                        .child(
                            Button::new("web")
                                .compact()
                                .icon(IconName::Globe)
                                .label("snoutdata.com")
                                .on_click(|_, _, cx| cx.open_url(WEBSITE_URL)),
                        )
                        .into_any_element(),
                );
                body.push(dim("Made by SnoutData. MIT License.".into()).text_size(px(12.)).into_any_element());
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))).into_any_element());
            }
            Dialog::Error(msg) => {
                body.push(code(&msg).into_any_element());
                let copy = msg.clone();
                buttons.push(
                    Button::new("copy")
                        .ghost()
                        .icon(IconName::Copy)
                        .label("Copy")
                        .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone())))
                        .into_any_element(),
                );
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))).into_any_element());
            }
            Dialog::Info { text, .. } => {
                body.push(dim(text).into_any_element());
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))).into_any_element());
            }
            Dialog::ConfirmDiscard(entries) => {
                body.push(files(&entries).into_any_element());
                body.push(callout(Tone::Danger, "This cannot be undone.", &p).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(danger("Discard"), Action::Discard(entries)));
            }
            Dialog::ConfirmDeleteFiles(entries) => {
                let trash = snout_core::can_trash(&*crate::host::host(cx));
                body.push(files(&entries).into_any_element());
                body.push(if trash {
                    callout(Tone::Info, "You can still get them back from the Trash.", &p).into_any_element()
                } else {
                    callout(Tone::Danger, "This cannot be undone.", &p).into_any_element()
                });
                buttons.push(cancel);
                buttons.push(on!(danger(if trash { "Move to Trash" } else { "Delete" }), Action::DeleteFiles(entries)));
            }
            Dialog::ConfirmCheckout(target) => {
                let (t1, t2) = (target.clone(), target.clone());
                body.push(
                    choice_card(
                        "stash",
                        IconName::Archive,
                        "Stash, check out, re-apply",
                        "Put the changes aside, switch, then bring them back. Works even if they touch files that differ on the target.",
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.act(Action::Checkout(t1.clone(), LocalChanges::Stash), w, cx)
                    }))
                    .into_any_element(),
                );
                body.push(
                    choice_card(
                        "keep",
                        IconName::ArrowRightLeft,
                        "Check out, keep changes",
                        "Carry the changes across as they are. Git refuses if they would be overwritten.",
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.act(Action::Checkout(t2.clone(), LocalChanges::Keep), w, cx)
                    }))
                    .into_any_element(),
                );
                if target.detaches() {
                    body.push(
                        callout(
                            Tone::Warning,
                            "This leaves HEAD detached. Create a branch if you want to keep new commits.",
                            &p,
                        )
                        .into_any_element(),
                    );
                }
                buttons.push(cancel);
            }
            Dialog::PullProblem { problem: PullProblem::Divergent, details } => {
                body.push(dim(format!("{branch} and {upstream} have each moved on, and this repository's settings do not say how to combine them.")).into_any_element());
                body.push(
                    choice_card(
                        "merge",
                        IconName::GitMerge,
                        "Merge",
                        "Join both histories with a merge commit. Nothing is rewritten.",
                        &p,
                    )
                    .on_click(cx.listener(|this, _, w, cx| this.act(Action::Pull(PullMode::Merge), w, cx)))
                    .into_any_element(),
                );
                body.push(
                    choice_card(
                        "rebase",
                        IconName::GitBranch,
                        "Rebase",
                        "Replay your commits on top of the remote ones, for a straight history.",
                        &p,
                    )
                    .on_click(cx.listener(|this, _, w, cx| this.act(Action::Pull(PullMode::Rebase), w, cx)))
                    .into_any_element(),
                );
                body.push(code(&details).into_any_element());
                buttons.push(cancel);
            }
            Dialog::PullProblem { details, .. } => {
                body.push(dim(format!("{branch} and {upstream} have no commit in common, so they cannot be merged. This usually means the remote repository was recreated or its history was rewritten.")).into_any_element());
                body.push(
                    callout(
                        Tone::Info,
                        format!("Resetting makes {branch} match {upstream} exactly. Your current commits stay on a backup branch, so nothing is lost."),
                        &p,
                    )
                    .into_any_element(),
                );
                if has_tracked {
                    body.push(
                        callout(
                            Tone::Danger,
                            "Commit or stash your uncommitted changes first; resetting would discard them.",
                            &p,
                        )
                        .into_any_element(),
                    );
                }
                body.push(code(&details).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(
                    Button::new("ok")
                        .primary()
                        .focusable()
                        .icon(IconName::RotateCcw)
                        .label(format!("Reset to {upstream}"))
                        .disabled(has_tracked),
                    Action::ResetToUpstream
                ));
            }
            Dialog::InitRepo(path) => {
                let (a, b) = (path.clone(), path.clone());
                body.push(
                    choice_card(
                        "find",
                        IconName::Search,
                        "Find repositories inside",
                        "Look through its subfolders and add every repository found.",
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, w, cx| this.act(Action::Scan(a.clone()), w, cx)))
                    .into_any_element(),
                );
                body.push(
                    choice_card(
                        "init",
                        IconName::Sparkles,
                        "Initialize a repository here",
                        "Start a fresh history in this folder with git init.",
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, w, cx| this.act(Action::InitRepo(b.clone()), w, cx)))
                    .into_any_element(),
                );
                buttons.push(cancel);
            }
            Dialog::NewBranch(start) => {
                let from = start.as_ref().map_or("HEAD".to_owned(), |s| s.label.clone());
                body.push(field_label("Name", &p).into_any_element());
                body.push(Input::new(&self.name).into_any_element());
                if let Some(problem) = branch_name_problem(&Self::value(&self.name, cx)) {
                    body.push(callout(Tone::Warning, problem, &p).into_any_element());
                }
                body.push(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(dim("Starting at".into()))
                        .child(badge(from, Tone::Git, &p))
                        .into_any_element(),
                );
                body.push(
                    Checkbox::new("checkout")
                        .label("Check out the new branch")
                        .checked(self.checkout)
                        .on_click(cx.listener(|this, v: &bool, _, cx| {
                            this.checkout = *v;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                buttons.push(cancel);
                buttons.push(
                    ok("Create")
                        .icon(IconName::GitBranch)
                        .disabled(!primary_enabled)
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx)))
                        .into_any_element(),
                );
            }
            Dialog::GroupName { rename, repo } => {
                let text = match (&rename, &repo) {
                    (Some(old), _) => format!("Renaming {old}"),
                    (None, Some(repo)) => format!("{} goes into the new group.", dir_name(repo)),
                    (None, None) => "Groups keep related repositories together. Drag repositories onto one.".into(),
                };
                body.push(dim(text).into_any_element());
                body.push(Input::new(&self.name).into_any_element());
                if let Some(problem) =
                    group_name_problem(&m.settings.groups, &Self::value(&self.name, cx), rename.as_deref())
                {
                    body.push(callout(Tone::Warning, problem, &p).into_any_element());
                }
                buttons.push(cancel);
                let (verb, icon) =
                    if rename.is_some() { ("Rename", IconName::Pencil) } else { ("Create", IconName::FolderPlus) };
                buttons.push(
                    ok(verb)
                        .icon(icon)
                        .disabled(!primary_enabled)
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx)))
                        .into_any_element(),
                );
            }
            Dialog::DeleteBranch { name, upstream } => {
                body.push(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(badge(name.clone(), Tone::Git, &p))
                        .child(dim("goes away.".into()))
                        .into_any_element(),
                );
                body.push(
                    Checkbox::new("force")
                        .label("Delete even if it is not fully merged")
                        .checked(self.force)
                        .on_click(cx.listener(|this, v: &bool, _, cx| {
                            this.force = *v;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                if let Some(up) = &upstream {
                    body.push(
                        Checkbox::new("upstream")
                            .label(format!("Also delete {} on the remote", up.label()))
                            .checked(self.with_upstream)
                            .on_click(cx.listener(|this, v: &bool, _, cx| {
                                this.with_upstream = *v;
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                    if self.with_upstream {
                        body.push(
                            callout(Tone::Danger, "Removing it from the remote affects everyone who uses it.", &p)
                                .into_any_element(),
                        );
                    }
                }
                buttons.push(cancel);
                let action =
                    Action::DeleteBranch { name, force: self.force, upstream: upstream.filter(|_| self.with_upstream) };
                buttons.push(on!(danger("Delete branch"), action));
            }
            Dialog::DeleteRemoteBranch(r) => {
                body.push(badge(r.label(), Tone::Git, &p).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(danger(&format!("Delete from {}", r.remote)), Action::DeleteRemoteBranch(r.clone())));
            }
            Dialog::Clone { .. } => {
                body.push(field_label("Repository URL", &p).into_any_element());
                body.push(Input::new(&self.url).into_any_element());
                body.push(field_label("Into folder", &p).into_any_element());
                body.push(Input::new(&self.parent).into_any_element());
                if let Some(Action::Clone { dest, .. }) = self.primary(cx) {
                    body.push(
                        div()
                            .flex()
                            .gap(px(8.))
                            .items_center()
                            .child(dim("Lands in".into()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(FileChip::new("dest", dest.display().to_string()).max_chars(52)),
                            )
                            .into_any_element(),
                    );
                }
                buttons.push(cancel);
                buttons.push(
                    ok("Clone")
                        .icon(IconName::Download)
                        .disabled(!primary_enabled)
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx)))
                        .into_any_element(),
                );
            }
            Dialog::Checkout => {
                let query = Self::value(&self.name, cx);
                let items = pick_items(m, &query);
                self.cursor = self.cursor.min(items.len().saturating_sub(1));
                body.push(Input::new(&self.name).into_any_element());
                let mut list = div().id("picks").flex().flex_col().h(px(280.)).overflow_y_scrollbar();
                let mut section = "";
                for (i, item) in items.iter().enumerate() {
                    if item.section != section {
                        section = item.section;
                        list = list.child(
                            div()
                                .pt(px(10.))
                                .pb(px(2.))
                                .px(px(8.))
                                .text_size(px(10.5))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(p.fg_dim)
                                .child(section.to_uppercase()),
                        );
                    }
                    let selected = i == self.cursor;
                    let kind = match item.target {
                        CheckoutTarget::Tag(_) => IconName::Tag,
                        CheckoutTarget::Commit(_) => IconName::GitCommitHorizontal,
                        _ => IconName::GitBranch,
                    };
                    list = list.child(
                        div()
                            .id(("pick", i))
                            .flex()
                            .gap(px(8.))
                            .px(px(8.))
                            .h(px(30.))
                            .items_center()
                            .rounded(px(6.))
                            .when(selected, |d| d.bg(p.row_selected()))
                            .hover(|s| s.bg(p.row_hover()))
                            .child(crate::components::icon(kind, 14., p.fg_dim))
                            .child(item.label.clone())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_color(p.fg_dim)
                                    .child(item.detail.clone()),
                            )
                            .on_click(cx.listener(move |this, e: &ClickEvent, w, cx| {
                                this.cursor = i;
                                if e.click_count() > 1 {
                                    this.confirm(w, cx);
                                }
                                cx.notify();
                            })),
                    );
                }
                if items.is_empty() {
                    list = list.child(
                        crate::components::EmptyState::new(spots::no_results(), "No matching branch, tag, or commit")
                            .compact(),
                    );
                }
                body.push(list.into_any_element());
                if has_changes {
                    let seg = |id: &'static str, label: &'static str, value: LocalChanges, current: LocalChanges| {
                        Button::new(id).compact().label(label).when(value == current, |b| b.primary()).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.local = value;
                                cx.notify();
                            }),
                        )
                    };
                    body.push(
                        div()
                            .flex()
                            .gap(px(6.))
                            .items_center()
                            .child(dim("Uncommitted changes:".into()))
                            .child(seg("keep", "Keep them", LocalChanges::Keep, self.local))
                            .child(seg("stash", "Stash and re-apply", LocalChanges::Stash, self.local))
                            .into_any_element(),
                    );
                }
                if items.get(self.cursor).is_some_and(|i| i.target.detaches()) {
                    body.push(
                        callout(
                            Tone::Warning,
                            "This leaves HEAD detached. Create a branch if you want to keep new commits.",
                            &p,
                        )
                        .into_any_element(),
                    );
                }
                buttons.push(cancel);
                buttons.push(
                    ok("Check Out")
                        .icon(IconName::ArrowRightLeft)
                        .disabled(items.is_empty())
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx)))
                        .into_any_element(),
                );
            }
        }
        let _ = window;
        div()
            .relative()
            .track_focus(&self.focus)
            .key_context("SnoutDialog")
            .on_action(cx.listener(|this, _: &ConfirmDestructive, w, cx| {
                if let Some(a) = this.destructive() {
                    this.act(a, w, cx);
                }
            }))
            .flex()
            .flex_col()
            .text_color(p.fg)
            .child(dialog_header(tone, art, heading, subtitle, &p))
            .child(dialog_body().children(body))
            .child(dialog_footer(None, buttons, &p))
            .child(
                div().absolute().top(px(10.)).right(px(10.)).child(
                    IconButton::new("dialog-close", IconName::X, "Close")
                        .on_click(cx.listener(|this, _, w, cx| this.close(w, cx))),
                ),
            )
    }
}
