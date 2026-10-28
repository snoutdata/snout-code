//! The dialogs the model asks for, on GPUI Kit's dialog layer. Each dialog's content is
//! its own view ([`DialogBody`]), so its fields and toggles redraw it as they change.

use std::path::Path;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{Disableable as _, Sizable as _, WindowExt as _};
use snout_core::groups::{dir_name, group_name_problem};
use snout_core::{BranchKind, CheckoutTarget, LocalChanges, PullMode, PullProblem, branch_name_problem, repo_dir_name};

use crate::menus::{SOURCE_URL, WEBSITE_URL};
use snout_core::art::Glyph;

use crate::model::{Action, Dialog, Model, Mood};
use crate::theme::palette;
use crate::ui::{chip, state_color};

/// Put up the dialog the model is waiting with.
pub fn open(model: &Entity<Model>, window: &mut Window, cx: &mut gpui::App) {
    let Some(dialog) = model.read(cx).dialog.clone() else { return };
    if window.has_active_dialog(cx) {
        window.close_all_dialogs(cx);
    }
    let width = match dialog {
        Dialog::Checkout => 580.,
        Dialog::About | Dialog::Error(_) | Dialog::PullProblem { .. } | Dialog::Clone { .. } => 520.,
        _ => 460.,
    };
    let title = title(&dialog, model.read(cx));
    let body = cx.new(|cx| DialogBody::new(model.clone(), dialog, window, cx));
    let m = model.clone();
    window.open_dialog(cx, move |d, _, _| {
        let m = m.clone();
        d.title(div().font_weight(FontWeight::SEMIBOLD).child(title.clone())).w(px(width)).child(body.clone()).on_close(
            move |_, _, cx| {
                m.update(cx, |m, _| m.dialog = None);
            },
        )
    });
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
        Dialog::ConfirmDeleteFiles(_) if snout_core::can_trash() => "Move to the Trash?".into(),
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
        let focus = if matches!(dialog, Dialog::Clone { .. }) { &url } else { &name };
        focus.update(cx, |s, cx| s.focus(window, cx));
        Self {
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
        let dim = |t: String| div().text_color(p.fg_dim).child(t);
        let code = |t: &str| {
            div()
                .id("code")
                .max_h(px(260.))
                .overflow_y_scrollbar()
                .p(px(8.))
                .rounded(px(8.))
                .bg(p.input)
                .border_1()
                .border_color(p.border)
                .font_family(gpui_component::Theme::global(cx).mono_font_family.clone())
                .text_size(px(12.))
                .child(t.to_owned())
        };
        let callout =
            |t: &str, color| div().p(px(8.)).rounded(px(8.)).bg(p.wash(color)).text_color(color).child(t.to_owned());
        let files = |entries: &[snout_core::FileEntry]| {
            div().id("files").max_h(px(200.)).overflow_y_scrollbar().p(px(6.)).rounded(px(8.)).bg(p.input).children(
                entries.iter().map(|e| {
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(div().w(px(90.)).text_color(state_color(e, &p)).child(e.state_label()))
                        .child(e.path.clone())
                }),
            )
        };
        let primary_enabled = self.primary(cx).is_some();
        let branch = m.status.branch.head.clone().unwrap_or_else(|| "HEAD".into());
        let upstream = m.status.branch.upstream.clone().unwrap_or_else(|| "its upstream".into());
        let has_tracked = m.status.files.iter().any(|f| !f.is_untracked());
        let has_changes = !m.status.files.is_empty();

        let mut body: Vec<AnyElement> = Vec::new();
        let mut buttons: Vec<Button> = Vec::new();
        let ok = |label: &str| Button::new("ok").primary().label(label.to_owned());
        let danger = |label: &str| Button::new("danger").danger().label(label.to_owned());
        let cancel = Button::new("cancel").label("Cancel").on_click(cx.listener(|this, _, w, cx| this.close(w, cx)));
        macro_rules! on {
            ($b:expr, $action:expr) => {{
                let action = $action;
                $b.on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.act(action.clone(), w, cx)))
            }};
        }
        let choice = |id: &'static str, g: Glyph, title: &str, detail: &str| {
            div()
                .id(id)
                .flex()
                .gap(px(10.))
                .p(px(10.))
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .hover(|s| s.bg(p.button_hover))
                .child(crate::art::glyph(g, 20., p.fg))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(title.to_owned()))
                        .child(div().text_color(p.fg_dim).child(detail.to_owned())),
                )
        };

        match self.dialog.clone() {
            Dialog::About => {
                body.push(dim(format!("Version {}", env!("CARGO_PKG_VERSION"))).into_any_element());
                body.push(
                    div().child("A small, fast desktop Git client, with Claude Code beside it.").into_any_element(),
                );
                body.push(
                    Button::new("src")
                        .link()
                        .label("github.com/snoutdata/snout-code")
                        .on_click(|_, _, cx| cx.open_url(SOURCE_URL))
                        .into_any_element(),
                );
                body.push(
                    Button::new("web")
                        .link()
                        .label("snoutdata.com")
                        .on_click(|_, _, cx| cx.open_url(WEBSITE_URL))
                        .into_any_element(),
                );
                body.push(dim("Made by SnoutData. MIT License.".into()).into_any_element());
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))));
            }
            Dialog::Error(msg) => {
                body.push(dim("Git reported this:".into()).into_any_element());
                body.push(code(&msg).into_any_element());
                let copy = msg.clone();
                buttons.push(
                    Button::new("copy")
                        .label("Copy")
                        .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()))),
                );
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))));
            }
            Dialog::Info { text, mood, .. } => {
                let color = if mood == Mood::Warning { p.warn } else { p.fg_dim };
                body.push(div().text_color(color).child(text).into_any_element());
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))));
            }
            Dialog::ConfirmDiscard(entries) => {
                let what =
                    if entries.len() == 1 { "this file".to_owned() } else { format!("these {} files", entries.len()) };
                body.push(
                    dim(format!("This puts {what} back as they are in HEAD. Untracked files are deleted."))
                        .into_any_element(),
                );
                body.push(files(&entries).into_any_element());
                body.push(
                    div().text_color(p.error).text_size(px(12.)).child("This cannot be undone.").into_any_element(),
                );
                buttons.push(cancel);
                buttons.push(on!(danger("Discard"), Action::Discard(entries)));
            }
            Dialog::ConfirmDeleteFiles(entries) => {
                let what =
                    if entries.len() == 1 { "This file".to_owned() } else { format!("These {} files", entries.len()) };
                let (text, label) = if snout_core::can_trash() {
                    (format!("{what} go to the Trash, where you can still get them back."), "Move to Trash")
                } else {
                    (format!("{what} are deleted from disk for good."), "Delete")
                };
                body.push(
                    dim(format!("{text} Tracked files then show as deleted, ready to commit.")).into_any_element(),
                );
                body.push(files(&entries).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(danger(label), Action::DeleteFiles(entries)));
            }
            Dialog::ConfirmCheckout(target) => {
                body.push(dim("You have uncommitted changes. What should happen to them?".into()).into_any_element());
                let (t1, t2) = (target.clone(), target.clone());
                body.push(
                    choice("stash", Glyph::Stage, "Stash, check out, re-apply", "Put the changes aside, switch, then bring them back. Works even if they touch files that differ on the target.")
                        .on_click(cx.listener(move |this, _, w, cx| this.act(Action::Checkout(t1.clone(), LocalChanges::Stash), w, cx)))
                        .into_any_element(),
                );
                body.push(
                    choice(
                        "keep",
                        Glyph::Switch,
                        "Check out, keep changes",
                        "Carry the changes across as they are. Git refuses if they would be overwritten.",
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.act(Action::Checkout(t2.clone(), LocalChanges::Keep), w, cx)
                    }))
                    .into_any_element(),
                );
                if target.detaches() {
                    body.push(
                        callout("This leaves HEAD detached. Create a branch if you want to keep new commits.", p.warn)
                            .into_any_element(),
                    );
                }
                buttons.push(cancel);
            }
            Dialog::PullProblem { problem: PullProblem::Divergent, details } => {
                body.push(dim(format!("{branch} and {upstream} have each moved on, and this repository's settings do not say how to combine them.")).into_any_element());
                body.push(
                    choice(
                        "merge",
                        Glyph::Commit,
                        "Merge",
                        "Join both histories with a merge commit. Nothing is rewritten.",
                    )
                    .on_click(cx.listener(|this, _, w, cx| this.act(Action::Pull(PullMode::Merge), w, cx)))
                    .into_any_element(),
                );
                body.push(
                    choice(
                        "rebase",
                        Glyph::Branch,
                        "Rebase",
                        "Replay your commits on top of the remote ones, for a straight history.",
                    )
                    .on_click(cx.listener(|this, _, w, cx| this.act(Action::Pull(PullMode::Rebase), w, cx)))
                    .into_any_element(),
                );
                body.push(code(&details).into_any_element());
                buttons.push(cancel);
            }
            Dialog::PullProblem { details, .. } => {
                body.push(dim(format!("{branch} and {upstream} have no commit in common, so they cannot be merged. This usually means the remote repository was recreated or its history was rewritten.")).into_any_element());
                body.push(div().child(format!("Resetting makes {branch} match {upstream} exactly. Your current commits stay on a backup branch, so nothing is lost.")).into_any_element());
                if has_tracked {
                    body.push(
                        callout(
                            "Commit or stash your uncommitted changes first; resetting would discard them.",
                            p.error,
                        )
                        .into_any_element(),
                    );
                }
                body.push(code(&details).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(ok(&format!("Reset to {upstream}")).disabled(has_tracked), Action::ResetToUpstream));
            }
            Dialog::InitRepo(path) => {
                body.push(dim(path.display().to_string()).into_any_element());
                let (a, b) = (path.clone(), path.clone());
                body.push(
                    choice(
                        "find",
                        Glyph::Search,
                        "Find repositories inside",
                        "Look through its subfolders and add every repository found.",
                    )
                    .on_click(cx.listener(move |this, _, w, cx| this.act(Action::Scan(a.clone()), w, cx)))
                    .into_any_element(),
                );
                body.push(
                    choice(
                        "init",
                        Glyph::Spark,
                        "Initialize a repository here",
                        "Start a fresh history in this folder with git init.",
                    )
                    .on_click(cx.listener(move |this, _, w, cx| this.act(Action::InitRepo(b.clone()), w, cx)))
                    .into_any_element(),
                );
                buttons.push(cancel);
            }
            Dialog::NewBranch(start) => {
                let from = start.as_ref().map_or("HEAD".to_owned(), |s| s.label.clone());
                body.push(
                    div()
                        .flex()
                        .gap(px(6.))
                        .items_center()
                        .child(dim("Starting at".into()))
                        .child(chip(from, p.fg_dim, &p))
                        .into_any_element(),
                );
                body.push(Input::new(&self.name).into_any_element());
                if let Some(problem) = branch_name_problem(&Self::value(&self.name, cx)) {
                    body.push(div().text_color(p.warn).text_size(px(12.)).child(problem).into_any_element());
                }
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
                    ok("Create").disabled(!primary_enabled).on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx))),
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
                    body.push(div().text_color(p.warn).text_size(px(12.)).child(problem).into_any_element());
                }
                buttons.push(cancel);
                let verb = if rename.is_some() { "Rename" } else { "Create" };
                buttons.push(
                    ok(verb).disabled(!primary_enabled).on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx))),
                );
            }
            Dialog::DeleteBranch { name, upstream } => {
                body.push(
                    div()
                        .flex()
                        .gap(px(6.))
                        .items_center()
                        .child(chip(name.clone(), p.fg_dim, &p))
                        .child(dim("goes away. Its commits stay wherever else they are reachable.".into()))
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
                            callout("Removing it from the remote affects everyone who uses it.", p.error)
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
                body.push(
                    div()
                        .flex()
                        .gap(px(6.))
                        .items_center()
                        .child(chip(r.label(), p.fg_dim, &p))
                        .child(dim(
                            "is removed from the remote for everyone who uses it. Local branches stay as they are."
                                .into(),
                        ))
                        .into_any_element(),
                );
                buttons.push(cancel);
                buttons.push(on!(danger(&format!("Delete from {}", r.remote)), Action::DeleteRemoteBranch(r.clone())));
            }
            Dialog::Clone { .. } => {
                body.push(dim("Copy a remote repository to this computer and open it.".into()).into_any_element());
                body.push(div().text_size(px(12.)).text_color(p.fg_dim).child("URL").into_any_element());
                body.push(Input::new(&self.url).into_any_element());
                body.push(div().text_size(px(12.)).text_color(p.fg_dim).child("Into folder").into_any_element());
                body.push(Input::new(&self.parent).into_any_element());
                if let Some(Action::Clone { dest, .. }) = self.primary(cx) {
                    body.push(
                        div()
                            .flex()
                            .gap(px(6.))
                            .items_center()
                            .child(dim("Lands in".into()))
                            .child(chip(dest.display().to_string(), p.fg_dim, &p))
                            .into_any_element(),
                    );
                }
                buttons.push(cancel);
                buttons.push(
                    ok("Clone").disabled(!primary_enabled).on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx))),
                );
            }
            Dialog::Checkout => {
                let query = Self::value(&self.name, cx);
                let items = pick_items(m, &query);
                self.cursor = self.cursor.min(items.len().saturating_sub(1));
                body.push(dim("A branch, a tag, or any commit.".into()).into_any_element());
                body.push(Input::new(&self.name).into_any_element());
                let mut list = div().id("picks").flex().flex_col().h(px(280.)).overflow_y_scrollbar();
                let mut section = "";
                for (i, item) in items.iter().enumerate() {
                    if item.section != section {
                        section = item.section;
                        list = list.child(
                            div().pt(px(8.)).text_size(px(10.5)).text_color(p.fg_dim).child(section.to_uppercase()),
                        );
                    }
                    let selected = i == self.cursor;
                    list = list.child(
                        div()
                            .id(("pick", i))
                            .flex()
                            .gap(px(8.))
                            .px(px(8.))
                            .h(px(28.))
                            .items_center()
                            .rounded(px(6.))
                            .when(selected, |d| d.bg(p.selection()))
                            .hover(|s| s.bg(p.button_hover))
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
                    list =
                        list.child(div().p(px(30.)).text_color(p.fg_dim).child("No matching branch, tag, or commit"));
                }
                body.push(list.into_any_element());
                if has_changes {
                    let seg = |id: &'static str, label: &'static str, value: LocalChanges, current: LocalChanges| {
                        Button::new(id).small().label(label).when(value == current, |b| b.primary()).on_click(
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
                        callout("This leaves HEAD detached. Create a branch if you want to keep new commits.", p.warn)
                            .into_any_element(),
                    );
                }
                buttons.push(cancel);
                buttons.push(
                    ok("Check Out")
                        .disabled(items.is_empty())
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx))),
                );
            }
        }
        let _ = window;
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(body)
            .child(div().flex().justify_end().gap(px(8.)).pt(px(6.)).children(buttons))
    }
}
