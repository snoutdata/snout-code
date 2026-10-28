//! SnoutCode on GPUI and GPUI Kit: an activity bar, docks, and the Git Panel.

pub mod art;
pub mod console;
pub mod dialogs;
pub mod editing;
pub mod editor;
pub mod files;
pub mod git_view;
pub mod icons;
pub mod look;
pub mod menus;
pub mod model;
pub mod panels;
pub mod projects;
pub mod search;
pub mod settings;
pub mod status;
pub mod tabs;
pub mod theme;
pub mod ui;
pub mod workspace;

#[cfg(test)]
mod code_tests;
#[cfg(test)]
mod tests;

use gpui::{App, AppContext as _, Bounds, Entity, WindowBounds, WindowOptions, px, size};

use crate::model::Model;
use crate::settings::{Settings, Store};

/// Everything the app needs before its first window: the Kit, theme, keys, menus.
pub fn init(settings: &Settings, cx: &mut App) {
    gpui_component::init(cx);
    let fonts = [epaint_default_fonts::UBUNTU_LIGHT, epaint_default_fonts::HACK_REGULAR];
    if let Err(e) = cx.text_system().add_fonts(fonts.map(std::borrow::Cow::Borrowed).to_vec()) {
        eprintln!("Could not load the fonts: {e}");
    }
    theme::apply(settings.theme, None, cx);
    menus::bind_keys(cx);
    editor::register_languages();
    workspace::app_actions(cx);
    cx.set_menus(menus::bar(settings));
}

/// The model over the settings in `store`, opening `open` (a path from the command line).
pub fn model(store: Option<Store>, open: Option<std::path::PathBuf>, cx: &mut App) -> Entity<Model> {
    let settings = store.as_ref().map(Store::load).unwrap_or_default();
    let model = cx.new(|cx| Model::new(settings, store, cx));
    if let Some(path) = open {
        model.update(cx, |m, cx| m.handle(model::Action::OpenPath(path), cx));
    }
    model
}

/// Start the app: one window on the saved repositories.
pub fn run() {
    let open = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    gpui_platform::application().with_assets(gpui_kit_assets::AllAssets).run(move |cx: &mut App| {
        let store = Store::for_user();
        let settings = store.as_ref().map(Store::load).unwrap_or_default();
        init(&settings, cx);
        let model = model(store, open, cx);
        model.update(cx, |m, cx| m.start_background_fetch(snout_core::survey::FETCH_EVERY, cx));
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(800.), px(500.))),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| workspace::Workspace::new(model, window, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .expect("open the window");
        cx.activate(true);
    });
}
