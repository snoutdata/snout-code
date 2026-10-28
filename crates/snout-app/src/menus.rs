//! The native macOS menu bar, through GPUI, with the items of the egui app's `menu.rs`.
//!
//! Items run GPUI actions. The workspace handles the repository actions only while they
//! would do something, so GPUI greys the items out otherwise; check marks follow the
//! settings because the bar is rebuilt when they change.

use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};
use gpui_component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};

use crate::settings::{Settings, ThemeChoice};

actions!(
    snout,
    [
        About,
        Quit,
        Open,
        FindRepos,
        CloneRepo,
        NewGroup,
        ToggleConsole,
        SideBySide,
        Unified,
        Compact,
        ThemeSystem,
        ThemeDark,
        ThemeLight,
        Refresh,
        Pull,
        Fetch,
        Push,
        Commit,
        Draft,
        Stage,
        Unstage,
        Discard,
        Trash,
        Checkout,
        NewBranch,
        Github,
        Reveal,
        Source,
        Issues,
        Website,
        Minimize,
        Zoom,
        ShowProjects,
        ShowGit,
    ]
);

pub const SOURCE_URL: &str = "https://github.com/snoutdata/snout-code";
pub const ISSUES_URL: &str = "https://github.com/snoutdata/snout-code/issues";
pub const WEBSITE_URL: &str = "https://snoutdata.com";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", Open, None),
        KeyBinding::new("cmd-j", ToggleConsole, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("f5", Refresh, None),
        KeyBinding::new("cmd-shift-o", Checkout, None),
        KeyBinding::new("cmd-1", ShowProjects, None),
        KeyBinding::new("cmd-0", ShowGit, None),
    ]);
}

/// The whole bar, its check marks taken from `s`.
pub fn bar(s: &Settings) -> Vec<Menu> {
    let check = |item: MenuItem, on: bool| item.checked(on);
    vec![
        Menu::new("SnoutCode").items([
            MenuItem::action("About SnoutCode", About),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit SnoutCode", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open Local Repository…", Open),
            MenuItem::action("Find Repositories in Folder…", FindRepos),
            MenuItem::action("Clone from GitHub…", CloneRepo),
            MenuItem::separator(),
            MenuItem::action("New Group…", NewGroup),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Projects", ShowProjects),
            MenuItem::action("Git", ShowGit),
            check(MenuItem::action("Claude Code Console", ToggleConsole), s.console_open),
            MenuItem::separator(),
            check(MenuItem::action("Side by Side Diff", SideBySide), s.side_by_side),
            check(MenuItem::action("Unified Diff", Unified), !s.side_by_side),
            check(MenuItem::action("Compact Diff", Compact), s.compact),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new("Appearance").items([
                check(MenuItem::action(ThemeChoice::System.label(), ThemeSystem), s.theme == ThemeChoice::System),
                check(MenuItem::action(ThemeChoice::Dark.label(), ThemeDark), s.theme == ThemeChoice::Dark),
                check(MenuItem::action(ThemeChoice::Light.label(), ThemeLight), s.theme == ThemeChoice::Light),
            ])),
            MenuItem::separator(),
            MenuItem::action("Refresh", Refresh),
        ]),
        Menu::new("Repository").items([
            MenuItem::action("Pull", Pull),
            MenuItem::action("Fetch", Fetch),
            MenuItem::action("Push", Push),
            MenuItem::separator(),
            MenuItem::action("Commit", Commit),
            MenuItem::action("Draft Commit Message", Draft),
            MenuItem::separator(),
            MenuItem::action("Stage", Stage),
            MenuItem::action("Unstage", Unstage),
            MenuItem::action("Discard Changes…", Discard),
            MenuItem::action("Move to Trash…", Trash),
            MenuItem::separator(),
            MenuItem::action("Check Out…", Checkout),
            MenuItem::action("New Branch…", NewBranch),
            MenuItem::separator(),
            MenuItem::action("Open on GitHub", Github),
            MenuItem::action("Reveal in Finder", Reveal),
        ]),
        Menu::new("Window").items([MenuItem::action("Minimize", Minimize), MenuItem::action("Zoom", Zoom)]),
        Menu::new("Help").items([
            MenuItem::action("SnoutCode on GitHub", Source),
            MenuItem::action("Report an Issue", Issues),
            MenuItem::action("SnoutData Website", Website),
        ]),
    ]
}

/// What the bar shows that can change, to rebuild it only then.
pub fn checks(s: &Settings) -> (bool, bool, bool, ThemeChoice) {
    (s.console_open, s.side_by_side, s.compact, s.theme)
}
