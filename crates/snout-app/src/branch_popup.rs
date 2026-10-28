//! The title bar's branch popup: the repository's actions (new branch, fetch, pull, push),
//! then its local and remote branches, narrowed by a search field. A click checks one out.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use snout_core::{BranchKind, CheckoutTarget, PullMode};

use crate::glyphs::{Icon, icon};
use crate::look;
use crate::model::{Action, Model};
use crate::theme::palette;

/// The popup is done with: the workspace takes it away.
pub struct Dismiss;

pub struct BranchPopup {
    model: Entity<Model>,
    pub input: Entity<InputState>,
    focus: FocusHandle,
}

impl EventEmitter<Dismiss> for BranchPopup {}

impl Focusable for BranchPopup {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl BranchPopup {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search branches"));
        cx.subscribe(&input, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, input, focus: cx.focus_handle() }
    }

    /// Do `action`, then go away.
    fn run(&mut self, action: Action, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.handle(action, cx));
        cx.emit(Dismiss);
    }

    /// The branches the search keeps: (local, remote), the current one first.
    pub fn branches(&self, cx: &App) -> (Vec<(String, bool)>, Vec<String>) {
        let needle = self.input.read(cx).value().trim().to_lowercase();
        let keep = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
        let m = self.model.read(cx);
        let mut local: Vec<(String, bool)> = m
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::Local && keep(&b.name))
            .map(|b| (b.name.clone(), b.is_head))
            .collect();
        local.sort_by_key(|(_, head)| !*head);
        let remote = m
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::Remote && !b.name.ends_with("/HEAD") && keep(&b.name))
            .map(|b| b.name.clone())
            .collect();
        (local, remote)
    }
}

/// A row of the popup: an icon (or room for one), its text, and what it does.
fn row(
    id: impl Into<SharedString>,
    glyph: Option<Icon>,
    text: String,
    on: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let p = palette(cx);
    let id: SharedString = id.into();
    let hover = p.button_hover;
    div()
        .id(gpui::ElementId::Name(id.clone()))
        .debug_selector(move || id.to_string())
        .h(px(26.))
        .px(px(8.))
        .mx(px(4.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(4.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(div().flex_none().size(px(16.)).children(glyph.map(|g| icon(g, 16., p.fg_dim))))
        .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(text))
        .when(on, |d| d.child(icon(Icon::Check, 14., p.accent)))
}

fn heading(text: &'static str, cx: &App) -> AnyElement {
    let p = palette(cx);
    div().px(px(12.)).pt(px(8.)).pb(px(2.)).text_size(px(11.5)).text_color(p.fg_dim).child(text).into_any_element()
}

impl Render for BranchPopup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let idle = m.job.is_none();
        let ahead = m.status.branch.ahead;
        let (local, remote) = self.branches(cx);
        let mut rows: Vec<AnyElement> = Vec::new();
        let action_row = |id: &'static str, g: Icon, text: &str, action: Action, cx: &mut Context<Self>| {
            row(id, Some(g), text.to_owned(), false, cx)
                .on_click(cx.listener(move |this, _, _, cx| this.run(action.clone(), cx)))
                .into_any_element()
        };
        rows.push(action_row("branch-new", Icon::Plus, "New Branch…", Action::ShowNewBranch(None), cx));
        if idle {
            rows.push(action_row("branch-fetch", Icon::Fetch, "Fetch", Action::Fetch, cx));
            rows.push(action_row("branch-pull", Icon::Pull, "Pull", Action::Pull(PullMode::Default), cx));
            let push = if ahead > 0 { format!("Push ({ahead})") } else { "Push".to_owned() };
            rows.push(action_row("branch-push", Icon::Push, &push, Action::Push, cx));
        }
        if !local.is_empty() {
            rows.push(heading("Local", cx));
        }
        for (i, (name, head)) in local.into_iter().enumerate() {
            let target = CheckoutTarget::Branch(name.clone());
            rows.push(
                row(format!("branch-local-{i}"), Some(Icon::Branch), name, head, cx)
                    .when(!head, |d| {
                        d.on_click(
                            cx.listener(move |this, _, _, cx| this.run(Action::RequestCheckout(target.clone()), cx)),
                        )
                    })
                    .into_any_element(),
            );
        }
        if !remote.is_empty() {
            rows.push(heading("Remote", cx));
        }
        for (i, name) in remote.into_iter().enumerate() {
            let target = CheckoutTarget::RemoteBranch(name.clone());
            rows.push(
                row(format!("branch-remote-{i}"), Some(Icon::Branch), name, false, cx)
                    .on_click(cx.listener(move |this, _, _, cx| this.run(Action::RequestCheckout(target.clone()), cx)))
                    .into_any_element(),
            );
        }
        div()
            .id("branch-popup")
            .debug_selector(|| "branch-popup".into())
            .key_context("BranchPopup")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(Dismiss)))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(Dismiss)))
            .occlude()
            .w(px(300.))
            .max_h(px(460.))
            .py(px(6.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(div().px(px(8.)).pb(px(6.)).child(look::field(&self.input, true, None, &p)))
            .child(div().id("branch-rows").flex_1().min_h_0().overflow_y_scrollbar().children(rows))
    }
}
