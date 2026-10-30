//! The splash: a small borderless window that covers the moment before the main window can
//! paint (well under a second), in the house art style. Nothing slow waits behind it: the
//! main window shows as soon as it can, and the heavy work starts after its first paint.

use gpui::{
    App, AppContext as _, Bounds, Context, FontWeight, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, px, size,
};

use crate::theme::palette;

pub const SPLASH_W: f32 = 560.;
pub const SPLASH_H: f32 = 360.;

pub struct Splash;

impl Render for Splash {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .bg(p.ground)
            .border_1()
            .border_color(p.border)
            .rounded(px(12.))
            .font_family(crate::look::ui_font(cx))
            .text_color(p.fg)
            .child(crate::art::drawing(snout_core::art::building(), 300., 200., None))
            .child(div().text_size(px(26.)).font_weight(FontWeight::SEMIBOLD).child("SnoutCode"))
            .child(
                div().text_size(px(12.)).text_color(p.fg_dim).child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
            )
            .child(div().h(px(10.)))
            .child(div().text_size(px(12.5)).text_color(p.fg_dim).child("Starting SnoutCode"))
            .child(crate::status::progress_bar(None, 180., &p))
    }
}

/// Run `then` once `splash` is on the screen (at once without one): the work that would hold
/// up its first frame waits for it. A frame that never comes (a display asleep) holds
/// nothing up for long.
pub fn once_shown(
    splash: Option<WindowHandle<Splash>>,
    cx: &mut App,
    then: impl FnOnce(Option<WindowHandle<Splash>>, &mut App) + 'static,
) {
    let then = std::rc::Rc::new(std::cell::Cell::new(Some(then)));
    let run = move |cx: &mut App| {
        if let Some(then) = then.take() {
            then(splash, cx);
        }
    };
    let Some(handle) = splash else {
        run(cx);
        return;
    };
    let shown = run.clone();
    // A window's first frame is drawn as it opens and shown at the next frame; the frame
    // after that comes once it is on the screen.
    let _ = handle.update(cx, |_, window, _| {
        window.on_next_frame(move |window, _| window.on_next_frame(move |_, cx| shown(cx)));
    });
    cx.spawn(async move |cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
        cx.update(run);
    })
    .detach();
}

/// Open the splash, centred on the screen.
pub fn open(cx: &mut App) -> Option<WindowHandle<Splash>> {
    let bounds = Bounds::centered(None, size(px(SPLASH_W), px(SPLASH_H)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            kind: WindowKind::PopUp,
            is_movable: false,
            focus: false,
            show: true,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |_, cx| cx.new(|_| Splash),
    )
    .ok()
}
