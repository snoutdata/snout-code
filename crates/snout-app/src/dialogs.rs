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

use crate::claude_ide::capabilities::AskAnswer;
use crate::components::{
    Button, CANCEL_ICON, FileChip, IconButton, IconName, OK_ICON, Tone, badge, callout, choice_card, dialog_body,
    dialog_footer, dialog_header, field_label,
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
        Dialog::Checkout | Dialog::Quit(_) => 600.,
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
                m.update(cx, |m, cx| {
                    m.dialog = None;
                    cx.notify();
                });
            })
    });
    // After the dialog layer has taken the keyboard for itself.
    window.defer(cx, move |window, cx| body_focus.update(cx, |b, cx| b.take_focus(window, cx)));
}

/// What a dialog looks like: its tone, its drawing, and the line under its title.
fn look(d: &Dialog, m: &Model) -> (Tone, Option<&'static Drawing>, Option<SharedString>) {
    match d {
        Dialog::About => (
            Tone::Neutral,
            Some(snout_core::art::mark()),
            Some(t!("dialogs-version", cargo_pkg_version = (env!("CARGO_PKG_VERSION")).to_string()).into()),
        ),
        Dialog::Error(msg) => {
            let art = if msg.contains("[rejected]") || msg.contains("failed to push") {
                spots::push_rejected()
            } else if msg.contains("CONFLICT") || msg.contains("conflict") {
                spots::merge_conflict()
            } else {
                spots::warning()
            };
            (Tone::Danger, Some(art), Some(ts!("dialogs-git-stopped-and-said-why").into()))
        }
        Dialog::Info { mood: Mood::Warning, .. } => (Tone::Warning, Some(spots::warning()), None),
        Dialog::Info { mood: Mood::Done, .. } => (Tone::Success, Some(spots::success()), None),
        Dialog::Info { .. } => (Tone::Info, Some(spots::no_results()), None),
        Dialog::ConfirmDiscard(entries) => {
            let text = t!("dialogs-discard-body", count = entries.len());
            (Tone::Danger, Some(spots::discard()), Some(text.into()))
        }
        Dialog::ConfirmDeleteFiles(entries) => {
            let text = if snout_core::can_trash(&*m.host) {
                t!("dialogs-trash-body", count = entries.len())
            } else {
                t!("dialogs-delete-body", count = entries.len())
            };
            (Tone::Danger, Some(spots::discard()), Some(text.into()))
        }
        Dialog::ConfirmCheckout(_) => (
            Tone::Git,
            Some(spots::stash()),
            Some(ts!("dialogs-you-have-uncommitted-changes-what-should-happen-to").into()),
        ),
        Dialog::Checkout => (Tone::Git, None, Some(ts!("dialogs-a-branch-a-tag-or-any-commit").into())),
        Dialog::NewBranch(_) => {
            (Tone::Git, Some(spots::new_branch()), Some(ts!("dialogs-a-line-of-work-of-its-own-from").into()))
        }
        Dialog::DeleteBranch { .. } => (
            Tone::Danger,
            Some(spots::discard()),
            Some(ts!("dialogs-its-commits-stay-wherever-else-they-are-reachable").into()),
        ),
        Dialog::DeleteRemoteBranch(_) => {
            (Tone::Danger, Some(spots::discard()), Some(ts!("dialogs-it-goes-for-everyone-who-uses-the-remote").into()))
        }
        Dialog::PullProblem { problem: PullProblem::Divergent, .. } => {
            (Tone::Warning, Some(spots::merge_conflict()), None)
        }
        Dialog::PullProblem { .. } => (Tone::Warning, Some(spots::push_rejected()), None),
        Dialog::GroupName { .. } => (Tone::Info, Some(spots::no_project()), None),
        Dialog::Clone { .. } => (
            Tone::Git,
            Some(spots::clone_repo()),
            Some(ts!("dialogs-copy-a-remote-repository-to-this-computer-and").into()),
        ),
        Dialog::AgentAsks { agent, .. } => (
            Tone::Warning,
            Some(spots::agent_console()),
            Some(t!("dialogs-is-waiting-in-the-console-for-your-answer", agent = agent.to_string()).into()),
        ),
        Dialog::Quit(info) if info.is_empty() => (
            Tone::Neutral,
            Some(spots::info()),
            Some(ts!("dialogs-the-window-closes-and-the-java-engine-stops").into()),
        ),
        Dialog::Quit(info) if info.unsaved.is_empty() => (
            Tone::Warning,
            Some(spots::warning()),
            Some(ts!("dialogs-what-runs-now-stops-when-snoutcode-quits").into()),
        ),
        Dialog::Quit(info) => {
            let text = t!("dialogs-unsaved-files", count = info.unsaved.len());
            (Tone::Warning, Some(spots::warning()), Some(text.into()))
        }
    }
}

fn title(d: &Dialog, m: &Model) -> SharedString {
    match d {
        Dialog::About => ts!("cmd-about-snoutcode").into(),
        Dialog::Error(_) => ts!("dialogs-something-went-wrong").into(),
        Dialog::Info { title, .. } => title.clone().into(),
        Dialog::ConfirmDiscard(_) => ts!("dialogs-discard-changes").into(),
        Dialog::ConfirmCheckout(t) => t!("dialogs-check-out-2", t = (t.label()).to_string()).into(),
        Dialog::Checkout => ts!("dialogs-check-out-3").into(),
        Dialog::NewBranch(_) => ts!("dialogs-new-branch").into(),
        Dialog::DeleteBranch { .. } => ts!("dialogs-delete-branch-2").into(),
        Dialog::DeleteRemoteBranch(r) => t!("dialogs-delete-branch-from", remote = (r.remote).to_string()).into(),
        Dialog::ConfirmDeleteFiles(_) if snout_core::can_trash(&*m.host) => ts!("dialogs-move-to-the-trash").into(),
        Dialog::ConfirmDeleteFiles(_) => ts!("dialogs-delete-files").into(),
        Dialog::PullProblem { problem: PullProblem::Divergent, .. } => {
            ts!("dialogs-both-sides-have-new-commits").into()
        }
        Dialog::PullProblem { .. } => ts!("dialogs-no-shared-history").into(),
        Dialog::GroupName { rename: Some(_), .. } => ts!("dialogs-rename-group").into(),
        Dialog::GroupName { .. } => ts!("dialogs-new-group").into(),
        Dialog::Clone { .. } => {
            let _ = m;
            ts!("dialogs-clone-repository").into()
        }
        Dialog::Quit(_) => ts!("dialogs-quit-snoutcode").into(),
        Dialog::AgentAsks { agent, what, .. } => {
            t!("dialogs-allow-to", agent = agent.to_string(), what = what.to_string()).into()
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
    /// Quit: "Don't ask again" is ticked.
    dont_ask: bool,
    /// Holds the keyboard in a dialog with no field, so ⌘⌫ and ⏎ reach it.
    focus: FocusHandle,
}

impl DialogBody {
    fn new(model: Entity<Model>, dialog: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (placeholder, value) = match &dialog {
            Dialog::NewBranch(_) => ("feature/my-idea", String::new()),
            Dialog::GroupName { rename, .. } => (ts!("dialogs-group-example"), rename.clone().unwrap_or_default()),
            Dialog::Checkout => (ts!("dialogs-checkout-placeholder"), String::new()),
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
            dont_ask: false,
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
        self.model.update(cx, |m, cx| m.answer_dialog(action, cx));
    }

    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        self.model.update(cx, |m, cx| {
            m.dialog = None;
            cx.notify();
        });
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
            Dialog::Quit(ref info) => {
                let save = (!info.unsaved.is_empty()).then_some(true);
                let dont_ask = self.dont_ask && info.is_empty();
                self.act(Action::Quit(crate::model::QuitChoice { save, dont_ask }), window, cx)
            }
            // It comes up while the user may be typing: Return must not answer it for them.
            Dialog::AgentAsks { .. } => {}
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
            Dialog::AgentAsks { .. } => Some(Action::AgentAnswer(AskAnswer::Once)),
            Dialog::Quit(info) if !info.unsaved.is_empty() => {
                Some(Action::Quit(crate::model::QuitChoice { save: Some(false), dont_ask: false }))
            }
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
            BranchKind::Local => (ts!("dialogs-local-branches"), !b.is_head),
            BranchKind::Remote => (ts!("dialogs-remote-branches"), !locals.contains(&b.local_name())),
        };
        if include {
            let detail = format!("{} {}", b.target, b.subject);
            items.push(PickItem { target: b.into(), label: b.name.clone(), detail, section });
        }
    }
    items.sort_by_key(|i| i.section != ts!("dialogs-local-branches"));
    for t in m.tags.iter().filter(|t| matches(&t.name)) {
        let detail = format!("{} {}", t.target, t.subject);
        items.push(PickItem {
            target: CheckoutTarget::Tag(t.name.clone()),
            label: t.name.clone(),
            detail,
            section: ts!("dialogs-tags"),
        });
    }
    // A query that names a commit but nothing above exactly: offer the commit itself.
    if !query.is_empty()
        && !items.iter().any(|i| i.label == query)
        && let Some(described) = m.repo.as_ref().and_then(|r| r.describe_commit(query).ok())
    {
        let target = CheckoutTarget::Commit(query.to_owned());
        items.push(PickItem { target, label: query.to_owned(), detail: described, section: ts!("cmd-commit") });
    }
    items
}

/// The Cancel button of a dialog.
fn cancel_button(cx: &mut Context<DialogBody>) -> AnyElement {
    Button::new("cancel", CANCEL_ICON)
        .debug_selector("dialog-cancel")
        .focusable()
        .label(ts!("settings-cancel"))
        .kbd("Esc")
        .on_click(cx.listener(|this, _, w, cx| this.close(w, cx)))
        .into_any_element()
}

impl DialogBody {
    /// Quit SnoutCode?: what stops and what is unsaved, and the buttons for it.
    fn quit_body(
        &mut self,
        info: &crate::model::QuitInfo,
        body: &mut Vec<AnyElement>,
        buttons: &mut Vec<AnyElement>,
        cancel: AnyElement,
        cx: &mut Context<Self>,
    ) {
        use crate::model::QuitChoice;
        let p = palette(cx);
        if !info.unsaved.is_empty() {
            body.push(
                div()
                    .id("quit-unsaved")
                    .max_h(px(150.))
                    .overflow_y_scrollbar()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(info.unsaved.iter().enumerate().map(|(i, name)| {
                        FileChip::new(("quit-file", i), name.clone())
                            .status(ts!("dialogs-unsaved"), p.warn)
                            .max_chars(46)
                    }))
                    .into_any_element(),
            );
        }
        if !info.stops.is_empty() {
            let lines = info.stops.iter().map(|s| format!("• {s}")).collect::<Vec<_>>().join("\n");
            body.push(
                crate::components::callout_with(Tone::Warning, Some(ts!("dialogs-these-stop").into()), lines, &p)
                    .debug_selector(|| "quit-stops".into())
                    .into_any_element(),
            );
        }
        if info.is_empty() {
            let on = self.dont_ask;
            body.push(
                Checkbox::new("quit-dont-ask")
                    .debug_selector(|| "quit-dont-ask".into())
                    .label(ts!("dialogs-dont-ask-again-settings-appearance-confirm-before-quitti"))
                    .checked(on)
                    .on_click(cx.listener(|this, v: &bool, _, cx| {
                        this.dont_ask = *v;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        buttons.push(cancel);
        let quit = |save: Option<bool>, dont_ask: bool| Action::Quit(QuitChoice { save, dont_ask });
        if info.unsaved.is_empty() {
            buttons.push(
                Button::new("ok", IconName::Power)
                    .debug_selector("quit-ok")
                    .primary()
                    .focusable()
                    .label(ts!("cmd-quit"))
                    .kbd("⏎")
                    .on_click(cx.listener(move |this, _, w, cx| {
                        let dont_ask = this.dont_ask;
                        this.act(quit(None, dont_ask), w, cx)
                    }))
                    .into_any_element(),
            );
        } else {
            buttons.push(
                Button::new("danger", IconName::Trash)
                    .debug_selector("quit-discard")
                    .danger()
                    .focusable()
                    .label(ts!("dialogs-discard-and-quit"))
                    .kbd(crate::keymap::shown_keys("⌘⌫"))
                    .on_click(cx.listener(move |this, _, w, cx| this.act(quit(Some(false), false), w, cx)))
                    .into_any_element(),
            );
            buttons.push(
                Button::new("ok", IconName::Save)
                    .debug_selector("quit-save-all")
                    .primary()
                    .focusable()
                    .label(ts!("dialogs-save-all-and-quit"))
                    .kbd("⏎")
                    .on_click(cx.listener(move |this, _, w, cx| this.act(quit(Some(true), false), w, cx)))
                    .into_any_element(),
            );
        }
    }
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
                    FileChip::new(("file", i), e.path.clone())
                        .status(crate::core_text::file_state(e), state_color(e, &p))
                        .max_chars(46)
                }),
            )
        };
        let primary_enabled = self.primary(cx).is_some();
        let status = m.dialog_status();
        let branch = status.branch.head.clone().unwrap_or_else(|| "HEAD".into());
        let upstream = status.branch.upstream.clone().unwrap_or_else(|| ts!("dialogs-its-upstream").into());
        let has_tracked = status.files.iter().any(|f| !f.is_untracked());
        let has_changes = !status.files.is_empty();

        let mut body: Vec<AnyElement> = Vec::new();
        let mut buttons: Vec<AnyElement> = Vec::new();
        let ok = |label: &str| Button::new("ok", OK_ICON).primary().focusable().label(label.to_owned()).kbd("⏎");
        let danger = |label: &str| {
            Button::new("danger", IconName::Trash)
                .danger()
                .focusable()
                .label(label.to_owned())
                .kbd(crate::keymap::shown_keys("⌘⌫"))
        };
        let cancel = Button::new("cancel", CANCEL_ICON)
            .focusable()
            .label(ts!("settings-cancel"))
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
                        .child(ts!("dialogs-an-ide-for-java-gradle-and-spring-boot"))
                        .into_any_element(),
                );
                body.push(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            Button::new("src", IconName::Github)
                                .compact()
                                .label("github.com/snoutdata/snout-code")
                                .on_click(|_, _, cx| cx.open_url(SOURCE_URL)),
                        )
                        .child(
                            Button::new("web", IconName::Globe)
                                .compact()
                                .label("snoutdata.com")
                                .on_click(|_, _, cx| cx.open_url(WEBSITE_URL)),
                        )
                        .into_any_element(),
                );
                body.push(
                    dim(ts!("dialogs-made-by-snoutdata-mit-license").into()).text_size(px(12.)).into_any_element(),
                );
                buttons.push(ok("OK").on_click(cx.listener(|this, _, w, cx| this.close(w, cx))).into_any_element());
            }
            Dialog::Error(msg) => {
                body.push(code(&msg).into_any_element());
                let copy = msg.clone();
                buttons.push(
                    Button::new("copy", IconName::Copy)
                        .ghost()
                        .label(ts!("cmd-copy"))
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
                body.push(callout(Tone::Danger, ts!("dialogs-this-cannot-be-undone"), &p).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(danger(ts!("dialogs-discard")), Action::Discard(entries)));
            }
            Dialog::ConfirmDeleteFiles(entries) => {
                let trash = snout_core::can_trash(&*crate::host::host(cx));
                body.push(files(&entries).into_any_element());
                body.push(if trash {
                    callout(Tone::Info, ts!("dialogs-you-can-still-get-them-back-from-the"), &p).into_any_element()
                } else {
                    callout(Tone::Danger, ts!("dialogs-this-cannot-be-undone"), &p).into_any_element()
                });
                buttons.push(cancel);
                buttons.push(on!(
                    danger(if trash { ts!("dialogs-move-to-trash") } else { ts!("dialogs-delete") }),
                    Action::DeleteFiles(entries)
                ));
            }
            Dialog::ConfirmCheckout(target) => {
                let (t1, t2) = (target.clone(), target.clone());
                body.push(
                    choice_card(
                        "stash",
                        IconName::Archive,
                        ts!("dialogs-stash-check-out-re-apply"),
                        ts!("dialogs-put-the-changes-aside-switch-then-bring-them"),
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
                        ts!("dialogs-check-out-keep-changes"),
                        ts!("dialogs-carry-the-changes-across-as-they-are-git"),
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.act(Action::Checkout(t2.clone(), LocalChanges::Keep), w, cx)
                    }))
                    .into_any_element(),
                );
                if target.detaches() {
                    body.push(
                        callout(Tone::Warning, ts!("dialogs-this-leaves-head-detached-create-a-branch-if"), &p)
                            .into_any_element(),
                    );
                }
                buttons.push(cancel);
            }
            Dialog::PullProblem { problem: PullProblem::Divergent, details } => {
                body.push(
                    dim(t!(
                        "dialogs-and-have-each-moved-on-and-this-repositorys",
                        branch = branch.to_string(),
                        upstream = upstream.to_string()
                    ))
                    .into_any_element(),
                );
                body.push(
                    choice_card(
                        "merge",
                        IconName::GitMerge,
                        ts!("dialogs-merge"),
                        ts!("dialogs-join-both-histories-with-a-merge-commit-nothing"),
                        &p,
                    )
                    .on_click(cx.listener(|this, _, w, cx| this.act(Action::Pull(PullMode::Merge), w, cx)))
                    .into_any_element(),
                );
                body.push(
                    choice_card(
                        "rebase",
                        IconName::GitBranch,
                        ts!("dialogs-rebase"),
                        ts!("dialogs-replay-your-commits-on-top-of-the-remote"),
                        &p,
                    )
                    .on_click(cx.listener(|this, _, w, cx| this.act(Action::Pull(PullMode::Rebase), w, cx)))
                    .into_any_element(),
                );
                body.push(code(&details).into_any_element());
                buttons.push(cancel);
            }
            Dialog::PullProblem { details, .. } => {
                body.push(
                    dim(t!(
                        "dialogs-and-have-no-commit-in-common-so-they",
                        branch = branch.to_string(),
                        upstream = upstream.to_string()
                    ))
                    .into_any_element(),
                );
                body.push(
                    callout(
                        Tone::Info,
                        t!(
                            "dialogs-resetting-makes-match-exactly-your-current-commits-stay",
                            branch = branch.to_string(),
                            upstream = upstream.to_string()
                        ),
                        &p,
                    )
                    .into_any_element(),
                );
                if has_tracked {
                    body.push(
                        callout(
                            Tone::Danger,
                            ts!("dialogs-commit-or-stash-your-uncommitted-changes-first-resetting"),
                            &p,
                        )
                        .into_any_element(),
                    );
                }
                body.push(code(&details).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(
                    Button::new("ok", IconName::RotateCcw)
                        .primary()
                        .focusable()
                        .label(t!("dialogs-reset-to", upstream = upstream.to_string()))
                        .disabled(has_tracked),
                    Action::ResetToUpstream
                ));
            }
            Dialog::NewBranch(start) => {
                let from = start.as_ref().map_or("HEAD".to_owned(), |s| s.label.clone());
                body.push(field_label(ts!("dialogs-name"), &p).into_any_element());
                body.push(Input::new(&self.name).into_any_element());
                if let Some(problem) = branch_name_problem(&Self::value(&self.name, cx)) {
                    body.push(callout(Tone::Warning, crate::core_text::name_problem(problem), &p).into_any_element());
                }
                body.push(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(dim(ts!("dialogs-starting-at").into()))
                        .child(badge(from, Tone::Git, &p))
                        .into_any_element(),
                );
                body.push(
                    Checkbox::new("checkout")
                        .label(ts!("dialogs-check-out-the-new-branch"))
                        .checked(self.checkout)
                        .on_click(cx.listener(|this, v: &bool, _, cx| {
                            this.checkout = *v;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                buttons.push(cancel);
                buttons.push(
                    ok(ts!("dialogs-create"))
                        .icon(IconName::GitBranch)
                        .disabled(!primary_enabled)
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx)))
                        .into_any_element(),
                );
            }
            Dialog::GroupName { rename, repo } => {
                let text = match (&rename, &repo) {
                    (Some(old), _) => t!("dialogs-renaming", old = old.to_string()),
                    (None, Some(repo)) => t!("dialogs-goes-into-the-new-group", repo = (dir_name(repo)).to_string()),
                    (None, None) => ts!("dialogs-groups-keep-related-repositories-together-drag-repositor").into(),
                };
                body.push(dim(text).into_any_element());
                body.push(Input::new(&self.name).into_any_element());
                if let Some(problem) =
                    group_name_problem(&m.settings.groups, &Self::value(&self.name, cx), rename.as_deref())
                {
                    body.push(callout(Tone::Warning, crate::core_text::name_problem(problem), &p).into_any_element());
                }
                buttons.push(cancel);
                let (verb, icon) = if rename.is_some() {
                    (ts!("dialogs-rename"), IconName::Pencil)
                } else {
                    (ts!("dialogs-create"), IconName::FolderPlus)
                };
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
                        .child(dim(ts!("dialogs-goes-away").into()))
                        .into_any_element(),
                );
                body.push(
                    Checkbox::new("force")
                        .label(ts!("dialogs-delete-even-if-it-is-not-fully-merged"))
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
                            .label(t!("dialogs-also-delete-on-the-remote", up = (up.label()).to_string()))
                            .checked(self.with_upstream)
                            .on_click(cx.listener(|this, v: &bool, _, cx| {
                                this.with_upstream = *v;
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                    if self.with_upstream {
                        body.push(
                            callout(Tone::Danger, ts!("dialogs-removing-it-from-the-remote-affects-everyone-who"), &p)
                                .into_any_element(),
                        );
                    }
                }
                buttons.push(cancel);
                let action =
                    Action::DeleteBranch { name, force: self.force, upstream: upstream.filter(|_| self.with_upstream) };
                buttons.push(on!(danger(ts!("dialogs-delete-branch")), action));
            }
            Dialog::DeleteRemoteBranch(r) => {
                body.push(badge(r.label(), Tone::Git, &p).into_any_element());
                buttons.push(cancel);
                buttons.push(on!(
                    danger(&t!("dialogs-delete-from", remote = (r.remote).to_string())),
                    Action::DeleteRemoteBranch(r.clone())
                ));
            }
            Dialog::Clone { .. } => {
                body.push(field_label(ts!("dialogs-repository-url"), &p).into_any_element());
                body.push(Input::new(&self.url).into_any_element());
                body.push(field_label(ts!("dialogs-into-folder"), &p).into_any_element());
                body.push(Input::new(&self.parent).into_any_element());
                if let Some(Action::Clone { dest, .. }) = self.primary(cx) {
                    body.push(
                        div()
                            .flex()
                            .gap(px(8.))
                            .items_center()
                            .child(dim(ts!("dialogs-lands-in").into()))
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
                    ok(ts!("dialogs-clone"))
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
                        crate::components::EmptyState::new(
                            spots::no_results(),
                            ts!("dialogs-no-matching-branch-tag-or-commit"),
                        )
                        .compact(),
                    );
                }
                body.push(list.into_any_element());
                if has_changes {
                    let seg = |id: &'static str,
                               ico: IconName,
                               label: &'static str,
                               value: LocalChanges,
                               current: LocalChanges| {
                        Button::new(id, ico).compact().label(label).when(value == current, |b| b.primary()).on_click(
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
                            .child(dim(ts!("dialogs-uncommitted-changes").into()))
                            .child(seg(
                                "keep",
                                IconName::ArrowRightLeft,
                                ts!("dialogs-keep-them"),
                                LocalChanges::Keep,
                                self.local,
                            ))
                            .child(seg(
                                "stash",
                                IconName::Archive,
                                ts!("dialogs-stash-and-re-apply"),
                                LocalChanges::Stash,
                                self.local,
                            ))
                            .into_any_element(),
                    );
                }
                if items.get(self.cursor).is_some_and(|i| i.target.detaches()) {
                    body.push(
                        callout(Tone::Warning, ts!("dialogs-this-leaves-head-detached-create-a-branch-if"), &p)
                            .into_any_element(),
                    );
                }
                buttons.push(cancel);
                buttons.push(
                    ok(ts!("dialogs-check-out"))
                        .icon(IconName::ArrowRightLeft)
                        .disabled(items.is_empty())
                        .on_click(cx.listener(|this, _, w, cx| this.confirm(w, cx)))
                        .into_any_element(),
                );
            }
            // Below, in its own function.
            Dialog::Quit(_) => {}
            Dialog::AgentAsks { ref tool, .. } => {
                body.push(
                    dim(t!("dialogs-allow-for-this-session-lets-it-use-again", tool = tool.to_string()))
                        .into_any_element(),
                );
                buttons.push(
                    Button::new("agent-refuse", CANCEL_ICON)
                        .debug_selector("agent-refuse")
                        .focusable()
                        .label(ts!("dialogs-dont-allow"))
                        .on_click(cx.listener(|this, _, w, cx| this.close(w, cx)))
                        .into_any_element(),
                );
                buttons.push(on!(
                    Button::new("agent-session", OK_ICON)
                        .debug_selector("agent-session")
                        .focusable()
                        .label(ts!("dialogs-allow-for-this-session")),
                    Action::AgentAnswer(AskAnswer::Session)
                ));
                buttons.push(on!(
                    Button::new("agent-once", OK_ICON)
                        .debug_selector("agent-once")
                        .primary()
                        .focusable()
                        .label(ts!("dialogs-allow")),
                    Action::AgentAnswer(AskAnswer::Once)
                ));
            }
        }
        if let Dialog::Quit(info) = self.dialog.clone() {
            self.quit_body(&info, &mut body, &mut buttons, cancel_button(cx), cx);
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
                    IconButton::new("dialog-close", IconName::X, ts!("dialogs-close"))
                        .on_click(cx.listener(|this, _, w, cx| this.close(w, cx))),
                ),
            )
    }
}
