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
        ReplaceInFiles,
        UndoReplaceInFiles,
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
        OpenSettingsJson,
        OpenProjectSettingsJson,
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
        ShowProcesses,
        StopIdleProcesses,
        RestartJava,
        ConnectSsh,
        CloseRemote,
        ToggleParamHints,
        ShowStructure,
        FileStructure,
        TypeHierarchy,
        CallHierarchy,
        ShowHierarchy,
        GoToClass,
        ToggleBreakpoint,
        ToggleBreakpointEnabled,
        EditBreakpointCondition,
        ViewBreakpoints,
        EvaluateExpression,
        RunToCursor,
        PauseProgram,
        Rerun,
        RerunFailedTests,
        RunGradleTask,
        SortStructure,
        SpringAtCaret,
        SpringProfiles,
        ProjectStructure,
    ]
);

/// Spring Active Profiles…: evaluate the Spring model under these profiles.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct SetSpringProfiles(pub crate::spring::ProfileMode);

/// Run Gradle Task…: run this task of this build, in the Build window.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct RunTask {
    pub build: Option<String>,
    pub task: String,
}

/// File ▸ Open Recent: open this project of a remote machine.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct OpenRemoteRecent(pub crate::remote::RemoteProject);

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
        ("Connect to SSH Host…", Box::new(ConnectSsh)),
        ("Close Remote Connection", Box::new(CloseRemote)),
        ("Clear Recent Projects", Box::new(ClearRecent)),
        ("Search Everywhere…", Box::new(SearchEverywhere)),
        ("Settings…", Box::new(OpenSettings)),
        ("Open Settings (JSON)", Box::new(OpenSettingsJson)),
        ("Open Project Settings (JSON)", Box::new(OpenProjectSettingsJson)),
        ("Go to File…", Box::new(GoToFile)),
        ("Go to Class…", Box::new(GoToClass)),
        ("Go to Line…", Box::new(GoToLine)),
        ("Find in Files…", Box::new(FindInFiles)),
        ("Replace in Files…", Box::new(ReplaceInFiles)),
        ("Undo Replace in Files", Box::new(UndoReplaceInFiles)),
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
        ("Toggle Parameter Name Hints", Box::new(ToggleParamHints)),
        ("Go to Definition", Box::new(i::GoToDefinition)),
        ("Find References", Box::new(e::FindReferences)),
        ("Rename Symbol…", Box::new(e::RenameSymbol)),
        ("Extract Method", Box::new(e::ExtractMethod)),
        ("Extract Variable", Box::new(e::ExtractVariable)),
        ("Extract Constant", Box::new(e::ExtractConstant)),
        ("Extract Field", Box::new(e::ExtractField)),
        ("Inline", Box::new(e::Inline)),
        ("Quick Fix…", Box::new(i::ToggleCodeActions)),
        ("Format Document", Box::new(e::FormatDocument)),
        ("Format Selection", Box::new(e::FormatSelection)),
        ("Organize Imports", Box::new(e::OrganizeImports)),
        ("Show Parameter Hints", Box::new(e::ShowSignature)),
        ("Trigger Completion", Box::new(i::ShowCompletions)),
        ("Go to Symbol in Workspace…", Box::new(GoToSymbol)),
        ("Go to Symbol in File…", Box::new(GoToSymbolInFile)),
        ("File Structure…", Box::new(FileStructure)),
        ("Structure", Box::new(ShowStructure)),
        ("Type Hierarchy", Box::new(TypeHierarchy)),
        ("Call Hierarchy", Box::new(CallHierarchy)),
        ("Hierarchy", Box::new(ShowHierarchy)),
        ("Show Problems", Box::new(ShowProblems)),
        ("Run", Box::new(RunSelected)),
        ("Debug", Box::new(DebugSelected)),
        ("Run Context Configuration", Box::new(RunContext)),
        ("Debug Context Configuration", Box::new(DebugContext)),
        ("Stop", Box::new(StopRuns)),
        ("Rerun", Box::new(Rerun)),
        ("Rerun Failed Tests", Box::new(RerunFailedTests)),
        ("Run Gradle Task…", Box::new(RunGradleTask)),
        ("Edit Configurations…", Box::new(EditConfigurations)),
        ("Toggle Breakpoint", Box::new(ToggleBreakpoint)),
        ("Toggle Breakpoint Enabled", Box::new(ToggleBreakpointEnabled)),
        ("Edit Breakpoint Condition…", Box::new(EditBreakpointCondition)),
        ("View Breakpoints…", Box::new(ViewBreakpoints)),
        ("Evaluate Expression…", Box::new(EvaluateExpression)),
        ("Run to Cursor", Box::new(RunToCursor)),
        ("Step Over", Box::new(StepOver)),
        ("Step Into", Box::new(StepInto)),
        ("Step Out", Box::new(StepOut)),
        ("Pause Program", Box::new(PauseProgram)),
        ("Resume Program", Box::new(Resume)),
        ("Clear Output", Box::new(ClearOutput)),
        ("Sync Gradle Project", Box::new(SyncGradle)),
        ("Project Structure…", Box::new(ProjectStructure)),
        ("Refresh Gradle Tasks", Box::new(RefreshGradle)),
        ("Collapse All Gradle Tasks", Box::new(CollapseGradle)),
        ("Spring Beans and Endpoints", Box::new(ShowSpring)),
        ("Read the Spring Model Again", Box::new(RefreshSpring)),
        ("Go to Spring Bean or Usages…", Box::new(SpringAtCaret)),
        ("Spring Active Profiles…", Box::new(SpringProfiles)),
        ("Collapse the Spring Window", Box::new(CollapseSpring)),
        ("Select Opened File", Box::new(LocateFile)),
        ("Collapse the Project Tree", Box::new(CollapseAll)),
        ("Hide the Side Panel", Box::new(HideSide)),
        ("Sort Structure by Name", Box::new(SortStructure)),
        ("Show Processes", Box::new(ShowProcesses)),
        ("Stop Idle Processes", Box::new(StopIdleProcesses)),
        ("Restart the Java Engine", Box::new(RestartJava)),
        ("Send Selection to Claude", Box::new(e::SendToClaude)),
        ("Accept Claude's Change", Box::new(e::AcceptChanges)),
        ("Reject Claude's Change", Box::new(e::RejectChanges)),
        ("Next Claude Change", Box::new(crate::claude_ide::NextClaudeChange)),
        ("Previous Claude Change", Box::new(crate::claude_ide::PrevClaudeChange)),
        ("Accept Claude Change at Caret", Box::new(crate::claude_ide::AcceptClaudeHunk)),
        ("Revert Claude Change at Caret", Box::new(crate::claude_ide::RevertClaudeHunk)),
        ("Accept All Claude Changes", Box::new(crate::claude_ide::AcceptAllClaude)),
        ("Revert All Claude Changes", Box::new(crate::claude_ide::RevertAllClaude)),
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

/// File ▸ Open Recent: the recent projects, most recent first, then those of remote
/// machines, then Clear Recent.
fn recent_items(recent: &[std::path::PathBuf], remote: &[crate::remote::RemoteProject]) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = recent
        .iter()
        .map(|path| MenuItem::action(crate::model::Model::dir_name(path), OpenRecent(path.clone())))
        .collect();
    if !remote.is_empty() {
        if !items.is_empty() {
            items.push(MenuItem::separator());
        }
        items.extend(remote.iter().map(|p| MenuItem::action(p.label(), OpenRemoteRecent(p.clone()))));
    }
    if !items.is_empty() {
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::action("Clear Recent", ClearRecent));
    items
}

/// The whole bar, its check marks taken from `s`; `recent` and `remote` (the remote
/// machines' projects) fill File ▸ Open Recent.
pub fn bar(s: &Settings, recent: &[std::path::PathBuf], remote: &[crate::remote::RemoteProject]) -> Vec<Menu> {
    let check = |item: MenuItem, on: bool| item.checked(on);
    vec![
        Menu::new("SnoutCode").items([
            MenuItem::action("About SnoutCode", About),
            MenuItem::separator(),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::action("Open Settings (JSON)", OpenSettingsJson),
            MenuItem::action("Open Project Settings (JSON)", OpenProjectSettingsJson),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit SnoutCode", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Project…", NewProject),
            MenuItem::action("Open Project…", Open),
            MenuItem::submenu(Menu::new("Open Recent").items(recent_items(recent, remote))),
            MenuItem::action("Close Project", CloseProject),
            MenuItem::separator(),
            MenuItem::action("Project Structure…", ProjectStructure),
            MenuItem::separator(),
            MenuItem::action("Connect to SSH Host…", ConnectSsh),
            MenuItem::action("Close Remote Connection", CloseRemote),
            MenuItem::separator(),
            MenuItem::action("Find Repositories in Folder…", FindRepos),
            MenuItem::action("Clone from GitHub…", CloneRepo),
            MenuItem::separator(),
            MenuItem::action("New Group…", NewGroup),
            MenuItem::separator(),
            MenuItem::action("Search Everywhere…", SearchEverywhere),
            MenuItem::action("Go to File…", GoToFile),
            MenuItem::action("Find in Files…", FindInFiles),
            MenuItem::action("Replace in Files…", ReplaceInFiles),
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
            MenuItem::action("Find in Files…", FindInFiles),
            MenuItem::action("Replace in Files…", ReplaceInFiles),
            MenuItem::action("Undo Replace in Files", UndoReplaceInFiles),
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
            MenuItem::action("Structure", ShowStructure),
            MenuItem::action("Hierarchy", ShowHierarchy),
            MenuItem::submenu(Menu::new("Tool Window Actions").items([
                MenuItem::action("Select Opened File", LocateFile),
                MenuItem::action("Collapse the Project Tree", CollapseAll),
                MenuItem::action("Hide the Side Panel", HideSide),
                MenuItem::separator(),
                MenuItem::action("Sort Structure by Name", SortStructure),
                MenuItem::action("Collapse All Gradle Tasks", CollapseGradle),
                MenuItem::action("Collapse the Spring Window", CollapseSpring),
                MenuItem::action("Clear Output", ClearOutput),
            ])),
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
            check(MenuItem::action("Parameter Name Hints", ToggleParamHints), s.param_hints),
            MenuItem::action("Split Editor Right", SplitRight),
            MenuItem::separator(),
            MenuItem::action("Processes", ShowProcesses),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new("Appearance").items([
                check(MenuItem::action(ThemeChoice::System.label(), ThemeSystem), s.theme == ThemeChoice::System),
                check(MenuItem::action(ThemeChoice::Light.label(), ThemeLight), s.theme == ThemeChoice::Light),
                check(MenuItem::action(ThemeChoice::Dark.label(), ThemeDark), s.theme == ThemeChoice::Dark),
            ])),
            MenuItem::separator(),
            MenuItem::action("Refresh", Refresh),
        ]),
        Menu::new("Code").items([
            MenuItem::action("Go to Definition", gpui_component::input::GoToDefinition),
            MenuItem::action("Find References", crate::editor::FindReferences),
            MenuItem::separator(),
            MenuItem::action("Quick Fix…", gpui_component::input::ToggleCodeActions),
            MenuItem::action("Format Document", crate::editor::FormatDocument),
            MenuItem::action("Format Selection", crate::editor::FormatSelection),
            MenuItem::action("Organize Imports", crate::editor::OrganizeImports),
            MenuItem::action("Show Parameter Hints", crate::editor::ShowSignature),
            MenuItem::separator(),
            MenuItem::action("Show Problems", ShowProblems),
            MenuItem::action("Go to Spring Bean or Usages…", SpringAtCaret),
            MenuItem::action("Spring Active Profiles…", SpringProfiles),
            MenuItem::separator(),
            MenuItem::action("Send Selection to Claude", crate::editor::SendToClaude),
            MenuItem::action("Accept All Claude Changes", crate::claude_ide::AcceptAllClaude),
            MenuItem::action("Revert All Claude Changes", crate::claude_ide::RevertAllClaude),
        ]),
        Menu::new("Navigate").items([
            MenuItem::action("Go to Class…", GoToClass),
            MenuItem::action("Go to File…", GoToFile),
            MenuItem::action("Go to Symbol in Workspace…", GoToSymbol),
            MenuItem::action("Go to Symbol in File…", GoToSymbolInFile),
            MenuItem::action("Go to Line…", GoToLine),
            MenuItem::separator(),
            MenuItem::action("File Structure…", FileStructure),
            MenuItem::action("Type Hierarchy", TypeHierarchy),
            MenuItem::action("Call Hierarchy", CallHierarchy),
        ]),
        Menu::new("Refactor").items([
            MenuItem::action("Rename Symbol…", crate::editor::RenameSymbol),
            MenuItem::separator(),
            MenuItem::action("Extract Method", crate::editor::ExtractMethod),
            MenuItem::action("Extract Variable", crate::editor::ExtractVariable),
            MenuItem::action("Extract Constant", crate::editor::ExtractConstant),
            MenuItem::action("Extract Field", crate::editor::ExtractField),
            MenuItem::separator(),
            MenuItem::action("Inline", crate::editor::Inline),
        ]),
        Menu::new("Run").items([
            MenuItem::action("Run", RunSelected),
            MenuItem::action("Debug", DebugSelected),
            MenuItem::action("Run Context Configuration", RunContext),
            MenuItem::action("Debug Context Configuration", DebugContext),
            MenuItem::action("Stop", StopRuns),
            MenuItem::action("Rerun", Rerun),
            MenuItem::action("Rerun Failed Tests", RerunFailedTests),
            MenuItem::separator(),
            MenuItem::action("Run Gradle Task…", RunGradleTask),
            MenuItem::action("Edit Configurations…", EditConfigurations),
            MenuItem::separator(),
            MenuItem::action("Toggle Breakpoint", ToggleBreakpoint),
            MenuItem::action("Toggle Breakpoint Enabled", ToggleBreakpointEnabled),
            MenuItem::action("Edit Breakpoint Condition…", EditBreakpointCondition),
            MenuItem::action("View Breakpoints…", ViewBreakpoints),
            MenuItem::separator(),
            MenuItem::action("Step Over", StepOver),
            MenuItem::action("Step Into", StepInto),
            MenuItem::action("Step Out", StepOut),
            MenuItem::action("Run to Cursor", RunToCursor),
            MenuItem::action("Pause Program", PauseProgram),
            MenuItem::action("Resume Program", Resume),
            MenuItem::separator(),
            MenuItem::action("Evaluate Expression…", EvaluateExpression),
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
pub type MenuState = (bool, bool, bool, bool, ThemeChoice, Vec<std::path::PathBuf>, Vec<crate::remote::RemoteProject>);

pub fn checks(s: &Settings, recent: &[std::path::PathBuf], remote: &[crate::remote::RemoteProject]) -> MenuState {
    (s.console_open, s.side_by_side, s.compact, s.param_hints, s.theme, recent.to_vec(), remote.to_vec())
}
