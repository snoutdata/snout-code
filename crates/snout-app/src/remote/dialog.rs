//! File ▸ Connect to SSH Host…: a host typed (`user@host`, `host`) or picked from the
//! recent ones and those of the ssh config (its `Include`s too). While it connects the
//! dialog says so; a failure shows in it, to try again.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::look;
use crate::theme::palette;

pub enum ConnectEvent {
    Connect(String),
    Cancel,
}

pub struct ConnectDialog {
    pub input: Entity<InputState>,
    /// The recent hosts, then those of the ssh config.
    hosts: Vec<String>,
    /// How many of `hosts` are recent.
    recent: usize,
    /// Connecting to this host now.
    pub connecting: Option<String>,
    pub error: Option<String>,
    focus: FocusHandle,
}

impl EventEmitter<ConnectEvent> for ConnectDialog {}

impl Focusable for ConnectDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ConnectDialog {
    pub fn new(recent: Vec<String>, configured: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let first = recent.first().cloned().unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("user@host, or a Host of your ssh config").default_value(first)
        });
        cx.subscribe(&input, |this, _, e: &InputEvent, cx| match e {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => this.connect(cx),
            _ => {}
        })
        .detach();
        let n = recent.len();
        let mut hosts = recent;
        hosts.extend(configured.into_iter().filter(|h| !hosts.contains(h)).collect::<Vec<_>>());
        Self { input, hosts, recent: n, connecting: None, error: None, focus: cx.focus_handle() }
    }

    /// The host typed.
    pub fn host(&self, cx: &App) -> String {
        self.input.read(cx).value().trim().to_owned()
    }

    /// Connect to the host typed.
    pub fn connect(&mut self, cx: &mut Context<Self>) {
        let host = self.host(cx);
        if host.is_empty() || self.connecting.is_some() {
            return;
        }
        self.error = None;
        self.connecting = Some(host.clone());
        cx.emit(ConnectEvent::Connect(host));
        cx.notify();
    }

    /// The connection failed: say why, and let it be tried again.
    pub fn failed(&mut self, why: String, cx: &mut Context<Self>) {
        self.connecting = None;
        self.error = Some(why);
        cx.notify();
    }

    fn pick(&mut self, host: String, e: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set_value(host, window, cx));
        if e.click_count() >= 2 {
            self.connect(cx);
        }
    }

    fn rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = palette(cx);
        let typed = self.host(cx);
        let hover = p.button_hover;
        let mut out = Vec::new();
        for (i, host) in self.hosts.iter().enumerate() {
            if i == 0 && self.recent > 0 {
                out.push(heading("Recent", cx));
            }
            if i == self.recent {
                out.push(heading("From your ssh config", cx));
            }
            let on = *host == typed;
            let id = format!("ssh-host-{i}");
            let sel = id.clone();
            let h = host.clone();
            out.push(
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .h(px(30.))
                    .px(px(10.))
                    .mx(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.selection()))
                    .when(!on, |d| d.hover(move |s| s.bg(hover)))
                    .child(icon(Icon::Terminal, 15., p.fg_dim))
                    .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(h))
                    .on_click(cx.listener({
                        let host = host.clone();
                        move |this, e: &ClickEvent, window, cx| this.pick(host.clone(), e, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        out
    }
}

fn heading(text: &'static str, cx: &App) -> AnyElement {
    let p = palette(cx);
    div()
        .px(px(20.))
        .pt(px(8.))
        .pb(px(4.))
        .text_size(px(11.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(p.fg_dim)
        .child(text)
        .into_any_element()
}

/// A dialog button: the accent for the main one. The shared [`crate::components::Button`]'s
/// look, as an element to add a handler to.
pub fn button(
    id: &'static str,
    text: &'static str,
    primary: bool,
    enabled: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    use crate::components::{Variant, button_base};
    let p = palette(cx);
    let variant = if primary { Variant::Primary } else { Variant::Secondary };
    button_base(id, variant, !enabled, false, false, false, &p).child(text)
}

/// A dialog's frame: a header with its title and an icon, its body, and a footer.
pub fn frame(id: &'static str, title: String, w: f32, h: f32, cx: &App) -> gpui::Stateful<gpui::Div> {
    let tone = crate::components::Tone::Info;
    crate::components::dialog_frame_icon(id, tone, tone.icon(), title, None, w, Some(h), cx)
}

/// A dialog's footer: a status line, then its buttons.
pub fn footer(status: Option<(String, gpui::Hsla)>, buttons: Vec<AnyElement>, cx: &App) -> gpui::Div {
    crate::components::dialog_footer(status, buttons, &palette(cx))
}

impl Render for ConnectDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::components::{EmptyState, IconName, Tone, dialog_frame, field_label, icon as ico};
        use snout_core::art::spots;
        let p = palette(cx);
        let busy = self.connecting.is_some();
        let can = !busy && !self.host(cx).is_empty();
        let status = match (&self.connecting, &self.error) {
            (Some(h), _) => Some((format!("Connecting to {h}…"), p.fg_dim)),
            (None, Some(e)) => Some((e.clone(), p.error)),
            _ => None,
        };
        let rows = self.rows(cx);
        let empty = rows.is_empty();
        let list = match &self.connecting {
            Some(host) => div()
                .id("connect-ssh-hosts")
                .flex_1()
                .min_h_0()
                .child(
                    EmptyState::new(spots::remote_connecting(), format!("Connecting to {host}…"))
                        .hint("The first time takes a little longer: SnoutCode installs its server there.")
                        .on(p.card)
                        .compact(),
                )
                .into_any_element(),
            None => div()
                .id("connect-ssh-hosts")
                .flex_1()
                .min_h_0()
                .pb(px(6.))
                .overflow_y_scrollbar()
                .children(rows)
                .when(empty, |d| {
                    d.child(
                        div()
                            .px(px(20.))
                            .py(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_color(p.fg_dim)
                            .child(ico(IconName::Info, 14., p.fg_dim))
                            .child("No hosts in your ssh config: type one."),
                    )
                })
                .into_any_element(),
        };
        dialog_frame(
            "connect-ssh",
            Tone::Info,
            Some(spots::ssh()),
            "Connect to SSH Host",
            Some("SnoutCode installs its server on the host the first time (Linux, x86_64 or aarch64).".into()),
            520.,
            Some(480.),
            cx,
        )
        .key_context("ConnectSsh")
        .track_focus(&self.focus)
        .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(ConnectEvent::Cancel)))
        .child(
            div()
                .flex_none()
                .px(px(20.))
                .pt(px(4.))
                .pb(px(8.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(field_label("Host", &p))
                .child(look::field(&self.input, false, None, &p)),
        )
        .child(list)
        .child(footer(
            status,
            vec![
                button("connect-ssh-cancel", "Cancel", false, true, cx)
                    .child(crate::components::kbd("Esc", p.fg_dim))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ConnectEvent::Cancel)))
                    .into_any_element(),
                crate::components::button_base(
                    "connect-ssh-connect",
                    crate::components::Variant::Primary,
                    !can,
                    false,
                    false,
                    false,
                    &p,
                )
                .child(ico(
                    IconName::Plug,
                    14.,
                    crate::components::button_ink(crate::components::Variant::Primary, !can, &p),
                ))
                .child("Connect")
                .child(crate::components::kbd(
                    "⏎",
                    crate::components::button_ink(crate::components::Variant::Primary, !can, &p).opacity(0.85),
                ))
                .when(can, |d| d.on_click(cx.listener(|this, _, _, cx| this.connect(cx))))
                .into_any_element(),
            ],
            cx,
        ))
    }
}
