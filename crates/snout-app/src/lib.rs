//! SnoutCode on GPUI and GPUI Kit: an activity bar, docks, and the Git Panel.

pub mod art;
pub mod background;
pub mod branch_popup;
pub mod claude_ide;
pub mod components;
pub mod console;
pub mod dialogs;
pub mod editing;
pub mod editor;
pub mod external_terminal;
pub mod files;
pub mod find_in_files;
pub mod git_view;
pub mod glyphs;
pub mod host;
pub mod icons;
pub mod keymap;
pub mod lang;
pub mod look;
pub mod menus;
pub mod model;
pub mod new_project;
pub mod panels;
pub mod processes;
pub mod project_model;
pub mod project_settings;
pub mod project_view;
pub mod projects;
pub mod remote;
pub mod run;
pub mod search;
pub mod settings;
pub mod settings_json;
pub mod settings_view;
pub mod splash;
pub mod spring;
pub mod status;
pub mod structure;
pub mod tabs;
pub mod theme;
pub mod tool_windows;
pub mod ui;
pub mod welcome;
pub mod workspace;

#[cfg(test)]
mod code_tests;
#[cfg(test)]
mod java_tools_tests;
#[cfg(test)]
mod lang_tests;
#[cfg(test)]
mod project_settings_tests;
#[cfg(test)]
mod project_tests;
#[cfg(test)]
mod quit_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod settings_json_tests;
#[cfg(test)]
mod spring_tests;
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
    theme::apply(settings.colors, settings.theme, None, cx);
    keymap::init(&settings.keymap, cx);
    editor::register_languages();
    workspace::app_actions(cx);
    cx.set_menus(menus::bar(settings, &model::recent_projects(settings), &remote::recents(cx).projects));
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
    // ssh runs this program to ask for a password or a passphrase (see `remote::askpass`).
    if std::env::var_os(snout_remote::ASKPASS_ENV).is_some() {
        remote::askpass::run();
        return;
    }
    // A quitting app hands its processes to this program again, run as the reaper.
    if std::env::var_os(workspace::quit::REAP_ENV).is_some() {
        workspace::quit::reap();
        return;
    }
    let open = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    gpui_platform::application().with_assets(gpui_kit_assets::AllAssets).run(move |cx: &mut App| {
        // The workspace's host, made here: this thread is the UI thread.
        cx.set_global(host::HostGlobal(snout_host::from_env()));
        // Quitting hides the app and exits it (the tests keep their process).
        cx.set_global(workspace::quit::RealQuit);
        let store = Store::for_user();
        let settings = store.as_ref().map(Store::load).unwrap_or_default();
        init(&settings, cx);
        quit_on_signals(cx);
        // One window is the app: closing it quits, so the engines stop with it rather than
        // run on behind an app with no window to show.
        cx.set_quit_mode(gpui::QuitMode::LastWindowClosed);
        // Language servers start in the app (tests opt in).
        cx.set_global(lang::hub::LangEnabled(true));
        // So does the server Claude Code connects to (tests use a scratch folder).
        if let Some(lock_dir) = claude_ide::lockfile::default_dir(&*host::host(cx)) {
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

/// SIGTERM, SIGINT and SIGHUP quit the app as ⌘Q does once answered, without asking about
/// unsaved tabs: the window goes and the engines (and with them jdtls, the Gradle runs and
/// the programs being run or debugged) are stopped rather than left behind. A second signal
/// while that runs ends the app at once.
fn quit_on_signals(cx: &mut App) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SIGNALLED: AtomicBool = AtomicBool::new(false);
    extern "C" fn on_signal(_: libc::c_int) {
        if SIGNALLED.swap(true, Ordering::SeqCst) {
            // SAFETY: `_exit` is async-signal-safe.
            unsafe { libc::_exit(130) };
        }
    }
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: the handler only touches an atomic, or calls `_exit`.
        unsafe { libc::signal(signal, on_signal as *const () as libc::sighandler_t) };
    }
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            if SIGNALLED.load(Ordering::SeqCst) {
                eprintln!("Quitting on a signal");
                cx.update(workspace::quit::quit_from_signal);
                return;
            }
        }
    })
    .detach();
}

/// The main window, on the project last open; the splash closes, and the heavy work starts,
/// once it has painted.
fn open_main(
    store: Option<Store>,
    open: Option<std::path::PathBuf>,
    splash: Option<gpui::WindowHandle<splash::Splash>>,
    cx: &mut App,
) {
    remote::init(store.clone(), cx);
    let model = model(store, open, cx);
    if open_window(model, splash, cx).is_none() {
        cx.quit();
        return;
    }
    cx.activate(true);
}

/// A main window on `model`; `splash` closes once it has painted, and the background fetch
/// starts then.
pub fn open_window(
    model: Entity<Model>,
    splash: Option<gpui::WindowHandle<splash::Splash>>,
    cx: &mut App,
) -> Option<gpui::AnyWindowHandle> {
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
    match opened {
        Ok(handle) => Some(handle.into()),
        Err(e) => {
            eprintln!("Could not open the window: {e}");
            None
        }
    }
}
