//! SnoutCode on GPUI and GPUI Kit: an activity bar, docks, and the Git Panel.

pub mod art;
pub mod background;
pub mod branch_popup;
pub mod claude_ide;
pub mod console;
pub mod dialogs;
pub mod editing;
pub mod editor;
pub mod files;
pub mod git_view;
pub mod glyphs;
pub mod gradle;
pub mod icons;
pub mod lang;
pub mod look;
pub mod menus;
pub mod model;
pub mod panels;
pub mod projects;
pub mod search;
pub mod settings;
pub mod settings_view;
pub mod splash;
pub mod status;
pub mod tabs;
pub mod theme;
pub mod tool_windows;
pub mod ui;
pub mod workspace;

#[cfg(test)]
mod code_tests;
#[cfg(test)]
mod lang_tests;
#[cfg(test)]
mod tests;

use gpui::{App, AppContext as _, Bounds, Entity, WindowBounds, WindowOptions, px, size};

use crate::model::Model;
use crate::settings::{Settings, Store};

/// The typefaces the app carries: Inter for the interface (SIL Open Font License 1.1, see
/// `assets/fonts/inter/OFL.txt`) and Hack for code.
const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/inter/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/inter/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/inter/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/inter/Inter-Italic.ttf"),
    epaint_default_fonts::HACK_REGULAR,
];

/// Everything the app needs before its first window: the Kit, theme, keys, menus.
pub fn init(settings: &Settings, cx: &mut App) {
    gpui_component::init(cx);
    if let Err(e) = cx.text_system().add_fonts(FONTS.map(std::borrow::Cow::Borrowed).to_vec()) {
        eprintln!("Could not load the fonts: {e}");
    }
    theme::set_fonts(settings, cx);
    theme::apply(settings.theme, None, cx);
    menus::bind_keys(cx);
    editor::register_languages();
    workspace::app_actions(cx);
    cx.set_menus(menus::bar(settings, &model::recent_projects(settings)));
}

/// The model over the settings in `store`, opening `open` (a path from the command line).
pub fn model(store: Option<Store>, open: Option<std::path::PathBuf>, cx: &mut App) -> Entity<Model> {
    let settings = store.as_ref().map(Store::load).unwrap_or_default();
    let model = cx.new(|cx| Model::new(settings, store, cx));
    if let Some(path) = open {
        model.update(cx, |m, cx| {
            m.open_arg(&path, cx);
            m.persist();
            cx.notify();
        });
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
        // Language servers start in the app (tests opt in).
        cx.set_global(lang::hub::LangEnabled(true));
        // So does the server Claude Code connects to (tests use a scratch folder).
        if let Some(lock_dir) = claude_ide::lockfile::default_dir() {
            cx.set_global(claude_ide::IdeConfig { lock_dir });
        }
        // The splash covers the moment until the window can paint; it goes on its first frame.
        let splash = splash::open(cx);
        cx.spawn(async move |cx| {
            // Let the splash paint before the work of opening the project.
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            cx.update(|cx| open_main(store, open, splash, cx));
        })
        .detach();
    });
}

/// The main window, on the project last open; the splash closes, and the heavy work starts,
/// once it has painted.
fn open_main(
    store: Option<Store>,
    open: Option<std::path::PathBuf>,
    splash: Option<gpui::WindowHandle<splash::Splash>>,
    cx: &mut App,
) {
    let model = model(store, open, cx);
    let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.), px(500.))),
            // The app draws the title bar; the window buttons sit in it, centred.
            titlebar: Some(gpui::TitlebarOptions {
                traffic_light_position: Some(gpui::point(px(13.), px(13.))),
                ..gpui_component::TitleBar::title_bar_options()
            }),
            ..gpui_component::TitleBar::window_options()
        },
        |window, cx| {
            let started = model.clone();
            window.on_next_frame(move |_, cx| {
                if let Some(splash) = splash {
                    let _ = splash.update(cx, |_, window, _| window.remove_window());
                }
                // SNOUT_NO_FETCH=1 keeps the app from fetching on its own (a read only look
                // at a repository).
                if std::env::var_os("SNOUT_NO_FETCH").is_none() {
                    let every = std::time::Duration::from_secs(started.read(cx).settings.fetch_minutes.max(1) * 60);
                    started.update(cx, |m, cx| m.start_background_fetch(every, cx));
                }
            });
            let view = cx.new(|cx| workspace::Workspace::new(model, window, cx));
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        },
    );
    if let Err(e) = opened {
        eprintln!("Could not open the window: {e}");
        cx.quit();
        return;
    }
    cx.activate(true);
}
