//! The native macOS menu bar, through GPUI, with the items of the egui app's `menu.rs`.
//!
//! Items run GPUI actions. The workspace handles the repository actions only while they
//! would do something, so GPUI greys the items out otherwise; check marks follow the
//! settings because the bar is rebuilt when they change.

use gpui::{Menu, MenuItem, OsAction, SharedString, SystemMenuType, actions};
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
        ShowProject,
        ShowGit,
        Back,
        PrevCommit,
        NextCommit,
        GoToFile,
        FindInFiles,
        GoToLine,
        CloseTab,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        CommandPalette,
        SplitRight,
        CloseProject,
        ClearRecent,
        GoToSymbol,
        GoToSymbolInFile,
        ShowProblems,
        OpenSettings,
        SearchEverywhere,
        HideSide,
        LocateFile,
        CollapseAll,
        CollapseGradle,
        ClearOutput,
        EditConfigurations,
        RunSelected,
        DebugSelected,
        RunContext,
        DebugContext,
        StopRuns,
        StepOver,
        StepInto,
        StepOut,
        Resume,
        SyncGradle,
        RefreshGradle,
        RefreshSpring,
        CollapseSpring,
        ShowSpring,
        NewProject,
    ]
);

/// Hide a tool window's dock: the right one when true, else the bottom one.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct HideToolWindow(pub bool);

/// File ▸ Open Recent: open this project.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct OpenRecent(pub std::path::PathBuf);

/// Every command, by the name the command palette lists it under; `recent` are the
/// recent projects, each a command of its own.
pub fn commands(recent: &[std::path::PathBuf]) -> Vec<(SharedString, Box<dyn gpui::Action>)> {
    let mut all: Vec<(SharedString, Box<dyn gpui::Action>)> =
        fixed_commands().into_iter().map(|(name, action)| (name.into(), action)).collect();
    let at = all.iter().position(|(n, _)| n == "Close Project").map_or(all.len(), |i| i + 1);
    let opens = recent.iter().map(|path| {
        let name = format!("Open Recent: {}", crate::model::Model::dir_name(path));
        (SharedString::from(name), Box::new(OpenRecent(path.clone())) as Box<dyn gpui::Action>)
    });
    all.splice(at..at, opens);
    all
}

fn fixed_commands() -> Vec<(&'static str, Box<dyn gpui::Action>)> {
    use crate::editor as e;
    use gpui_component::input as i;
    vec![
        ("New Project…", Box::new(NewProject)),
        ("Open Project…", Box::new(Open)),
        ("Close Project", Box::new(CloseProject)),
        ("Clear Recent Projects", Box::new(ClearRecent)),
        ("Search Everywhere…", Box::new(SearchEverywhere)),
        ("Settings…", Box::new(OpenSettings)),
        ("Go to File…", Box::new(GoToFile)),
        ("Go to Line…", Box::new(GoToLine)),
        ("Find in Files…", Box::new(FindInFiles)),
        ("Find in File", Box::new(i::Search)),
        ("Replace in File", Box::new(i::Replace)),
        ("Save", Box::new(e::Save)),
        ("Close Tab", Box::new(CloseTab)),
        ("Split Editor Right", Box::new(SplitRight)),
        ("Toggle Comment", Box::new(e::ToggleComment)),
        ("Duplicate Line", Box::new(e::DuplicateLine)),
        ("Move Line Up", Box::new(e::MoveLineUp)),
        ("Move Line Down", Box::new(e::MoveLineDown)),
        ("Toggle Soft Wrap", Box::new(e::ToggleSoftWrap)),
        ("Go to Definition", Box::new(i::GoToDefinition)),
        ("Find References", Box::new(e::FindReferences)),
        ("Rename Symbol…", Box::new(e::RenameSymbol)),
        ("Quick Fix…", Box::new(i::ToggleCodeActions)),
        ("Format Document", Box::new(e::FormatDocument)),
        ("Organize Imports", Box::new(e::OrganizeImports)),
        ("Show Parameter Hints", Box::new(e::ShowSignature)),
        ("Trigger Completion", Box::new(i::ShowCompletions)),
        ("Go to Symbol in Workspace…", Box::new(GoToSymbol)),
        ("Go to Symbol in File…", Box::new(GoToSymbolInFile)),
        ("Show Problems", Box::new(ShowProblems)),
        ("Run", Box::new(RunSelected)),
        ("Debug", Box::new(DebugSelected)),
        ("Run Context Configuration", Box::new(RunContext)),
        ("Debug Context Configuration", Box::new(DebugContext)),
        ("Stop", Box::new(StopRuns)),
        ("Edit Configurations…", Box::new(EditConfigurations)),
        ("Step Over", Box::new(StepOver)),
        ("Step Into", Box::new(StepInto)),
        ("Step Out", Box::new(StepOut)),
        ("Resume Program", Box::new(Resume)),
        ("Sync Gradle Project", Box::new(SyncGradle)),
        ("Refresh Gradle Tasks", Box::new(RefreshGradle)),
        ("Spring Beans and Endpoints", Box::new(ShowSpring)),
        ("Read the Spring Model Again", Box::new(RefreshSpring)),
        ("Send Selection to Claude", Box::new(e::SendToClaude)),
        ("Accept Claude's Change", Box::new(e::AcceptChanges)),
        ("Reject Claude's Change", Box::new(e::RejectChanges)),
        ("Undo", Box::new(Undo)),
        ("Redo", Box::new(Redo)),
        ("Zoom In", Box::new(ZoomIn)),
        ("Zoom Out", Box::new(ZoomOut)),
        ("Actual Size", Box::new(ZoomReset)),
        ("Show Project", Box::new(ShowProject)),
        ("Show Git", Box::new(ShowGit)),
        ("Toggle Claude Code Console", Box::new(ToggleConsole)),
        ("Find Repositories in Folder…", Box::new(FindRepos)),
        ("Clone from GitHub…", Box::new(CloneRepo)),
        ("New Group…", Box::new(NewGroup)),
        ("Refresh", Box::new(Refresh)),
        ("Pull", Box::new(Pull)),
        ("Fetch", Box::new(Fetch)),
        ("Push", Box::new(Push)),
        ("Commit", Box::new(Commit)),
        ("Draft Commit Message", Box::new(Draft)),
        ("Stage", Box::new(Stage)),
        ("Unstage", Box::new(Unstage)),
        ("Discard Changes…", Box::new(Discard)),
        ("Check Out…", Box::new(Checkout)),
        ("New Branch…", Box::new(NewBranch)),
        ("Side by Side Diff", Box::new(SideBySide)),
        ("Unified Diff", Box::new(Unified)),
        ("Compact Diff", Box::new(Compact)),
        ("Theme: Match the System", Box::new(ThemeSystem)),
        ("Theme: Dark", Box::new(ThemeDark)),
        ("Theme: Light", Box::new(ThemeLight)),
        ("Open on GitHub", Box::new(Github)),
        ("Reveal in Finder", Box::new(Reveal)),
        ("About SnoutCode", Box::new(About)),
        ("Quit SnoutCode", Box::new(Quit)),
    ]
}

pub const SOURCE_URL: &str = "https://github.com/snoutdata/snout-code";
pub const ISSUES_URL: &str = "https://github.com/snoutdata/snout-code/issues";
pub const WEBSITE_URL: &str = "https://snoutdata.com";

/// File ▸ Open Recent: the recent projects, most recent first, then Clear Recent.
fn recent_items(recent: &[std::path::PathBuf]) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = recent
        .iter()
        .map(|path| MenuItem::action(crate::model::Model::dir_name(path), OpenRecent(path.clone())))
        .collect();
    if !items.is_empty() {
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::action("Clear Recent", ClearRecent));
    items
}

/// The whole bar, its check marks taken from `s`; `recent` fills File ▸ Open Recent.
pub fn bar(s: &Settings, recent: &[std::path::PathBuf]) -> Vec<Menu> {
    let check = |item: MenuItem, on: bool| item.checked(on);
    vec![
        Menu::new("SnoutCode").items([
            MenuItem::action("About SnoutCode", About),
            MenuItem::separator(),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit SnoutCode", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Project…", NewProject),
            MenuItem::action("Open Project…", Open),
            MenuItem::submenu(Menu::new("Open Recent").items(recent_items(recent))),
            MenuItem::action("Close Project", CloseProject),
            MenuItem::separator(),
            MenuItem::action("Find Repositories in Folder…", FindRepos),
            MenuItem::action("Clone from GitHub…", CloneRepo),
            MenuItem::separator(),
            MenuItem::action("New Group…", NewGroup),
            MenuItem::separator(),
            MenuItem::action("Search Everywhere…", SearchEverywhere),
            MenuItem::action("Go to File…", GoToFile),
            MenuItem::action("Find in Files…", FindInFiles),
            MenuItem::action("Command Palette…", CommandPalette),
            MenuItem::separator(),
            MenuItem::action("Save", crate::editor::Save),
            MenuItem::action("Close Tab", CloseTab),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action("Find", gpui_component::input::Search),
            MenuItem::action("Replace", gpui_component::input::Replace),
            MenuItem::action("Go to Line…", GoToLine),
            MenuItem::separator(),
            MenuItem::action("Toggle Comment", crate::editor::ToggleComment),
            MenuItem::action("Duplicate Line", crate::editor::DuplicateLine),
            MenuItem::action("Move Line Up", crate::editor::MoveLineUp),
            MenuItem::action("Move Line Down", crate::editor::MoveLineDown),
        ]),
        Menu::new("View").items([
            MenuItem::action("Project", ShowProject),
            MenuItem::action("Git", ShowGit),
            check(MenuItem::action("Claude Code Console", ToggleConsole), s.console_open),
            MenuItem::separator(),
            check(MenuItem::action("Side by Side Diff", SideBySide), s.side_by_side),
            check(MenuItem::action("Unified Diff", Unified), !s.side_by_side),
            check(MenuItem::action("Compact Diff", Compact), s.compact),
            MenuItem::separator(),
            MenuItem::action("Zoom In", ZoomIn),
            MenuItem::action("Zoom Out", ZoomOut),
            MenuItem::action("Actual Size", ZoomReset),
            MenuItem::action("Soft Wrap", crate::editor::ToggleSoftWrap),
            MenuItem::action("Split Editor Right", SplitRight),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new("Appearance").items([
                check(MenuItem::action(ThemeChoice::System.label(), ThemeSystem), s.theme == ThemeChoice::System),
                check(MenuItem::action(ThemeChoice::Dark.label(), ThemeDark), s.theme == ThemeChoice::Dark),
                check(MenuItem::action(ThemeChoice::Light.label(), ThemeLight), s.theme == ThemeChoice::Light),
            ])),
            MenuItem::separator(),
            MenuItem::action("Refresh", Refresh),
        ]),
        Menu::new("Code").items([
            MenuItem::action("Go to Definition", gpui_component::input::GoToDefinition),
            MenuItem::action("Find References", crate::editor::FindReferences),
            MenuItem::action("Rename Symbol…", crate::editor::RenameSymbol),
            MenuItem::separator(),
            MenuItem::action("Quick Fix…", gpui_component::input::ToggleCodeActions),
            MenuItem::action("Format Document", crate::editor::FormatDocument),
            MenuItem::action("Organize Imports", crate::editor::OrganizeImports),
            MenuItem::action("Show Parameter Hints", crate::editor::ShowSignature),
            MenuItem::separator(),
            MenuItem::action("Go to Symbol in Workspace…", GoToSymbol),
            MenuItem::action("Go to Symbol in File…", GoToSymbolInFile),
            MenuItem::action("Show Problems", ShowProblems),
            MenuItem::separator(),
            MenuItem::action("Send Selection to Claude", crate::editor::SendToClaude),
        ]),
        Menu::new("Run").items([
            MenuItem::action("Run", RunSelected),
            MenuItem::action("Debug", DebugSelected),
            MenuItem::action("Run Context Configuration", RunContext),
            MenuItem::action("Debug Context Configuration", DebugContext),
            MenuItem::action("Stop", StopRuns),
            MenuItem::separator(),
            MenuItem::action("Edit Configurations…", EditConfigurations),
            MenuItem::separator(),
            MenuItem::action("Step Over", StepOver),
            MenuItem::action("Step Into", StepInto),
            MenuItem::action("Step Out", StepOut),
            MenuItem::action("Resume Program", Resume),
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
pub type MenuState = (bool, bool, bool, ThemeChoice, Vec<std::path::PathBuf>);

pub fn checks(s: &Settings, recent: &[std::path::PathBuf]) -> MenuState {
    (s.console_open, s.side_by_side, s.compact, s.theme, recent.to_vec())
}
