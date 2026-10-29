//! ssh's questions as SnoutCode dialogs. The app points `SSH_ASKPASS` at its own program
//! (with `SNOUT_ASKPASS` set); ssh runs it with the question as its argument and reads the
//! answer from its standard output: a password or a passphrase (typed hidden), or yes or
//! no to trusting a host key it has not seen. Cancel exits with 1, and ssh gives up.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, WindowBounds, WindowOptions, div,
    px, size,
};
use gpui_component::input::{InputEvent, InputState};

use super::dialog::button;
use crate::look;
use crate::theme::palette;

/// Whether ssh asks a yes or no question (a new host key) rather than for a secret.
pub fn is_confirmation(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    p.contains("(yes/no") || p.contains("continue connecting")
}

/// The dialog's title for `prompt`.
pub fn title_of(prompt: &str) -> &'static str {
    let p = prompt.to_lowercase();
    if is_confirmation(prompt) {
        "Trust This Host?"
    } else if p.contains("passphrase") {
        "Key Passphrase"
    } else if p.contains("password") {
        "Password"
    } else {
        "SSH"
    }
}

pub struct Askpass {
    pub prompt: String,
    pub input: Option<Entity<InputState>>,
    focus: FocusHandle,
}

fn answer(text: &str) -> ! {
    println!("{text}");
    std::process::exit(0);
}

fn cancel() -> ! {
    std::process::exit(1);
}

impl Askpass {
    pub fn new(prompt: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = (!is_confirmation(&prompt)).then(|| {
            let input = cx.new(|cx| InputState::new(window, cx).masked(true));
            cx.subscribe(&input, |_, input, e: &InputEvent, cx| {
                if let InputEvent::PressEnter { .. } = e {
                    answer(&input.read(cx).value());
                }
            })
            .detach();
            input
        });
        Self { prompt, input, focus: cx.focus_handle() }
    }
}

impl Render for Askpass {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let confirm = self.input.is_none();
        let input = self.input.clone();
        use crate::components::{IconName, Tone, dialog_header_icon, kbd};
        let (tone, ico) =
            if confirm { (Tone::Warning, IconName::ShieldAlert) } else { (Tone::Info, IconName::KeyRound) };
        div()
            .id("askpass")
            .debug_selector(|| "askpass".into())
            .track_focus(&self.focus)
            .size_full()
            .pt(px(22.))
            .flex()
            .flex_col()
            .bg(p.card)
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(dialog_header_icon(
                tone,
                ico,
                title_of(&self.prompt),
                Some(self.prompt.trim().to_owned().into()),
                &p,
            ))
            .when_some(self.input.as_ref(), |d, i| d.child(div().px(px(20.)).child(look::field(i, false, None, &p))))
            .child(div().flex_1())
            .child(
                div()
                    .px(px(20.))
                    .pb(px(16.))
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button(
                            "askpass-cancel",
                            crate::components::CANCEL_ICON,
                            if confirm { "No" } else { "Cancel" },
                            false,
                            true,
                            cx,
                        )
                        .on_click(|_, _, _| {
                            if is_confirmation_reply() {
                                answer("no")
                            }
                            cancel()
                        }),
                    )
                    .child(
                        button(
                            "askpass-ok",
                            if confirm { crate::components::APPLY_ICON } else { crate::components::OK_ICON },
                            if confirm { "Yes" } else { "OK" },
                            true,
                            true,
                            cx,
                        )
                        .when(!confirm, |d| d.child(kbd("⏎", p.primary().ink.opacity(0.85))))
                        .on_click(move |_, _, cx| match &input {
                            Some(i) => answer(&i.read(cx).value()),
                            None => answer("yes"),
                        }),
                    ),
            )
    }
}

/// No to a host key is said, not a cancel (ssh then says why it stopped).
fn is_confirmation_reply() -> bool {
    std::env::args().nth(1).is_some_and(|p| is_confirmation(&p))
}

/// The helper's whole life: one dialog, its answer on stdout.
pub fn run() {
    let prompt = std::env::args().nth(1).unwrap_or_else(|| "Password:".into());
    gpui_platform::application().with_assets(gpui_kit_assets::AllAssets).run(move |cx: &mut App| {
        let settings = crate::settings::Store::for_user().map(|s| s.load()).unwrap_or_default();
        crate::init(&settings, cx);
        cx.set_menus(Vec::new());
        cx.on_window_closed(|_, _| cancel()).detach();
        let tall = is_confirmation_reply();
        let bounds = Bounds::centered(None, size(px(460.), px(if tall { 270. } else { 210. })), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("SnoutCode".into()),
                    ..gpui_component::TitleBar::title_bar_options()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| Askpass::new(prompt, window, cx));
                if let Some(i) = view.read(cx).input.clone() {
                    i.update(cx, |i, cx| i.focus(window, cx));
                }
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        );
        if opened.is_err() {
            cancel();
        }
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_the_questions_apart() {
        let key = "The authenticity of host 'devbox (10.0.0.5)' can't be established.\nED25519 key fingerprint is \
                   SHA256:abc.\nAre you sure you want to continue connecting (yes/no/[fingerprint])?";
        assert!(is_confirmation(key));
        assert_eq!(title_of(key), "Trust This Host?");
        assert_eq!(title_of("Enter passphrase for key '/Users/me/.ssh/id_ed25519':"), "Key Passphrase");
        assert_eq!(title_of("me@devbox's password:"), "Password");
        assert!(!is_confirmation("me@devbox's password:"));
    }
}
