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
        RecentFiles,
        NewFile,
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
        CheckForUpdates,
        ReleaseNotes,
        RestartToUpdate,
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
        ReloadFromDisk,
        CollapseGradle,
        CollapseMaven,
        ClearOutput,
        EditConfigurations,
        EditEnvironments,
        ShowTests,
        RecordAtCursor,
        PickLocator,
        StopRecording,
        OpenUiMode,
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
        RefreshMaven,
        RefreshSpring,
        CollapseSpring,
        ShowSpring,
        NewProject,
        ShowProcesses,
        ShowAgentLog,
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
        OpenInTerminal,
        BuildProject,
        RebuildProject,
        RunMavenGoals,
        ShowTerminal,
        ShowOutput,
        ShowBuild,
        ShowGradle,
        ShowMaven,
        SettingsKeymap,
        SettingsTerminal,
        SettingsAgents,
        SettingsBuildTools,
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

/// Run Gradle Task…: run these Maven goals (or phases) in this module of this build.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct RunMavenGoal {
    pub build: String,
    pub project: String,
    pub module: String,
    pub goals: String,
}

/// File ▸ Open Recent: open this project of a remote machine.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct OpenRemoteRecent(pub crate::remote::RemoteProject);

/// Connect to this machine (`WSL: Ubuntu`, say) and pick a folder there: the console's way to an
/// agent installed in a distro.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct ConnectHost(pub String);

/// Hide a tool window's dock: the one that holds the panel of this dock name.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct HideToolWindow(pub &'static str);

/// Move a tool window to the left, right or bottom dock (its header's menu, its stripe
/// button's): the dock layout keeps it there.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct MoveToolWindow(pub crate::workspace::Tool, pub crate::workspace::Side);

/// Open in External Terminal at this file's folder (or this folder): from a tab's menu.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct OpenInTerminalAt(pub std::path::PathBuf);

/// Close the editor tab of this panel (by its entity id), even the last one.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct CloseEditorTab(pub u64);

/// File ▸ Open Recent: open this project.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = snout, no_json)]
pub struct OpenRecent(pub std::path::PathBuf);

/// Every command, by the name the command palette lists it under; `recent` are the
/// recent projects, each a command of its own.
pub fn commands(recent: &[std::path::PathBuf]) -> Vec<(SharedString, Box<dyn gpui::Action>)> {
    let mut all: Vec<(SharedString, Box<dyn gpui::Action>)> =
        fixed_commands().into_iter().map(|(name, action)| (name.into(), action)).collect();
    let at = all.iter().position(|(_, a)| a.partial_eq(&CloseProject)).map_or(all.len(), |i| i + 1);
    let opens = recent.iter().map(|path| {
        let name = t!("cmd-open-recent-project", name = crate::model::Model::dir_name(path));
        (SharedString::from(name), Box::new(OpenRecent(path.clone())) as Box<dyn gpui::Action>)
    });
    all.splice(at..at, opens);
    all
}

fn fixed_commands() -> Vec<(String, Box<dyn gpui::Action>)> {
    use crate::editor as e;
    use gpui_component::input as i;
    vec![
        (t!("cmd-new-project"), Box::new(NewProject)),
        (t!("cmd-open-project"), Box::new(Open)),
        (t!("cmd-close-project"), Box::new(CloseProject)),
        (
            if cfg!(windows) { t!("cmd-connect-to-ssh-host-or-wsl") } else { t!("cmd-connect-to-ssh-host") },
            Box::new(ConnectSsh),
        ),
        (t!("cmd-close-remote-connection"), Box::new(CloseRemote)),
        (t!("cmd-clear-recent-projects"), Box::new(ClearRecent)),
        (t!("cmd-search-everywhere"), Box::new(SearchEverywhere)),
        (t!("cmd-settings"), Box::new(OpenSettings)),
        (t!("cmd-open-settings-json"), Box::new(OpenSettingsJson)),
        (t!("cmd-open-project-settings-json"), Box::new(OpenProjectSettingsJson)),
        (t!("cmd-go-to-file"), Box::new(GoToFile)),
        (t!("cmd-recent-files"), Box::new(RecentFiles)),
        (t!("cmd-new-file"), Box::new(NewFile)),
        (t!("cmd-go-to-class"), Box::new(GoToClass)),
        (t!("cmd-go-to-line"), Box::new(GoToLine)),
        (t!("cmd-find-in-files"), Box::new(FindInFiles)),
        (t!("cmd-replace-in-files"), Box::new(ReplaceInFiles)),
        (t!("cmd-undo-replace-in-files"), Box::new(UndoReplaceInFiles)),
        (t!("cmd-find-in-file"), Box::new(i::Search)),
        (t!("cmd-replace-in-file"), Box::new(i::Replace)),
        (t!("cmd-save"), Box::new(e::Save)),
        (t!("cmd-close-tab"), Box::new(CloseTab)),
        (t!("cmd-split-editor-right"), Box::new(SplitRight)),
        (t!("cmd-toggle-comment"), Box::new(e::ToggleComment)),
        (t!("cmd-duplicate-line"), Box::new(e::DuplicateLine)),
        (t!("cmd-move-line-up"), Box::new(e::MoveLineUp)),
        (t!("cmd-move-line-down"), Box::new(e::MoveLineDown)),
        (t!("cmd-toggle-soft-wrap"), Box::new(e::ToggleSoftWrap)),
        (t!("cmd-toggle-parameter-name-hints"), Box::new(ToggleParamHints)),
        (t!("cmd-go-to-definition"), Box::new(i::GoToDefinition)),
        (t!("cmd-go-to-declaration-or-usages"), Box::new(e::GoToDeclaration)),
        (t!("cmd-go-to-implementations"), Box::new(e::GoToImplementations)),
        (t!("cmd-find-references"), Box::new(e::FindReferences)),
        (t!("cmd-rename-symbol"), Box::new(e::RenameSymbol)),
        (t!("cmd-extract-method"), Box::new(e::ExtractMethod)),
        (t!("cmd-extract-variable"), Box::new(e::ExtractVariable)),
        (t!("cmd-extract-constant"), Box::new(e::ExtractConstant)),
        (t!("cmd-extract-field"), Box::new(e::ExtractField)),
        (t!("cmd-inline"), Box::new(e::Inline)),
        (t!("cmd-show-context-actions"), Box::new(e::ShowContextActions)),
        (t!("cmd-apply-top-fix"), Box::new(e::ApplyTopFix)),
        (t!("cmd-format-document"), Box::new(e::FormatDocument)),
        (t!("cmd-format-selection"), Box::new(e::FormatSelection)),
        (t!("cmd-organize-imports"), Box::new(e::OrganizeImports)),
        (t!("cmd-show-parameter-hints"), Box::new(e::ShowSignature)),
        (t!("cmd-trigger-completion"), Box::new(i::ShowCompletions)),
        (t!("cmd-go-to-symbol-in-workspace"), Box::new(GoToSymbol)),
        (t!("cmd-go-to-symbol-in-file"), Box::new(GoToSymbolInFile)),
        (t!("cmd-file-structure"), Box::new(FileStructure)),
        (t!("cmd-structure"), Box::new(ShowStructure)),
        (t!("cmd-type-hierarchy"), Box::new(TypeHierarchy)),
        (t!("cmd-call-hierarchy"), Box::new(CallHierarchy)),
        (t!("cmd-hierarchy"), Box::new(ShowHierarchy)),
        (t!("cmd-show-problems"), Box::new(ShowProblems)),
        (t!("cmd-run"), Box::new(RunSelected)),
        (t!("cmd-debug"), Box::new(DebugSelected)),
        (t!("cmd-run-context-configuration"), Box::new(RunContext)),
        (t!("cmd-debug-context-configuration"), Box::new(DebugContext)),
        (t!("cmd-stop"), Box::new(StopRuns)),
        (t!("cmd-rerun"), Box::new(Rerun)),
        (t!("cmd-rerun-failed-tests"), Box::new(RerunFailedTests)),
        (t!("cmd-build-project"), Box::new(BuildProject)),
        (t!("cmd-rebuild-project"), Box::new(RebuildProject)),
        (t!("cmd-run-gradle-task"), Box::new(RunGradleTask)),
        (t!("cmd-run-maven-goal"), Box::new(RunMavenGoals)),
        (t!("cmd-show-the-build-window"), Box::new(ShowBuild)),
        (t!("cmd-show-the-gradle-window"), Box::new(ShowGradle)),
        (t!("cmd-show-the-maven-window"), Box::new(ShowMaven)),
        (t!("cmd-terminal"), Box::new(ShowTerminal)),
        (t!("cmd-output"), Box::new(ShowOutput)),
        (t!("cmd-settings-keymap"), Box::new(SettingsKeymap)),
        (t!("cmd-settings-terminal"), Box::new(SettingsTerminal)),
        (t!("cmd-settings-coding-agents"), Box::new(SettingsAgents)),
        (t!("cmd-settings-gradle-and-maven"), Box::new(SettingsBuildTools)),
        (t!("cmd-edit-configurations"), Box::new(EditConfigurations)),
        (t!("cmd-edit-environments"), Box::new(EditEnvironments)),
        (t!("cmd-show-the-tests-window"), Box::new(ShowTests)),
        (t!("cmd-record-at-cursor"), Box::new(RecordAtCursor)),
        (t!("cmd-pick-locator"), Box::new(PickLocator)),
        (t!("cmd-stop-recording"), Box::new(StopRecording)),
        (t!("cmd-open-ui-mode"), Box::new(OpenUiMode)),
        (t!("cmd-toggle-breakpoint"), Box::new(ToggleBreakpoint)),
        (t!("cmd-toggle-breakpoint-enabled"), Box::new(ToggleBreakpointEnabled)),
        (t!("cmd-edit-breakpoint-condition"), Box::new(EditBreakpointCondition)),
        (t!("cmd-view-breakpoints"), Box::new(ViewBreakpoints)),
        (t!("cmd-evaluate-expression"), Box::new(EvaluateExpression)),
        (t!("cmd-run-to-cursor"), Box::new(RunToCursor)),
        (t!("cmd-step-over"), Box::new(StepOver)),
        (t!("cmd-step-into"), Box::new(StepInto)),
        (t!("cmd-step-out"), Box::new(StepOut)),
        (t!("cmd-pause-program"), Box::new(PauseProgram)),
        (t!("cmd-resume-program"), Box::new(Resume)),
        (t!("cmd-clear-output"), Box::new(ClearOutput)),
        (t!("cmd-sync-gradle-and-maven-projects"), Box::new(SyncGradle)),
        (t!("cmd-project-structure"), Box::new(ProjectStructure)),
        (t!("cmd-refresh-gradle-tasks"), Box::new(RefreshGradle)),
        (t!("cmd-collapse-all-gradle-tasks"), Box::new(CollapseGradle)),
        (t!("cmd-reload-maven-project"), Box::new(SyncGradle)),
        (t!("cmd-read-the-maven-goals-again"), Box::new(RefreshMaven)),
        (t!("cmd-collapse-all-maven-goals"), Box::new(CollapseMaven)),
        (t!("cmd-spring-beans-and-endpoints"), Box::new(ShowSpring)),
        (t!("cmd-read-the-spring-model-again"), Box::new(RefreshSpring)),
        (t!("cmd-go-to-spring-bean-or-usages"), Box::new(SpringAtCaret)),
        (t!("cmd-spring-active-profiles"), Box::new(SpringProfiles)),
        (t!("cmd-collapse-the-spring-window"), Box::new(CollapseSpring)),
        (t!("cmd-reload-all-from-disk"), Box::new(ReloadFromDisk)),
        (t!("cmd-select-opened-file"), Box::new(LocateFile)),
        (t!("cmd-collapse-the-project-tree"), Box::new(CollapseAll)),
        (t!("cmd-hide-the-side-panel"), Box::new(HideSide)),
        (t!("cmd-sort-structure-by-name"), Box::new(SortStructure)),
        (t!("cmd-show-processes"), Box::new(ShowProcesses)),
        (t!("cmd-show-the-agent-log"), Box::new(ShowAgentLog)),
        (t!("cmd-stop-idle-processes"), Box::new(StopIdleProcesses)),
        (t!("cmd-restart-the-java-engine"), Box::new(RestartJava)),
        (t!("cmd-send-selection-to-claude"), Box::new(e::SendToClaude)),
        (t!("cmd-accept-claudes-change"), Box::new(e::AcceptChanges)),
        (t!("cmd-reject-claudes-change"), Box::new(e::RejectChanges)),
        (t!("cmd-next-claude-change"), Box::new(crate::claude_ide::NextClaudeChange)),
        (t!("cmd-previous-claude-change"), Box::new(crate::claude_ide::PrevClaudeChange)),
        (t!("cmd-accept-claude-change-at-caret"), Box::new(crate::claude_ide::AcceptClaudeHunk)),
        (t!("cmd-revert-claude-change-at-caret"), Box::new(crate::claude_ide::RevertClaudeHunk)),
        (t!("cmd-accept-all-claude-changes"), Box::new(crate::claude_ide::AcceptAllClaude)),
        (t!("cmd-revert-all-claude-changes"), Box::new(crate::claude_ide::RevertAllClaude)),
        (t!("cmd-show-agent-changes"), Box::new(crate::claude_ide::ShowAgentChanges)),
        (t!("cmd-annotate-with-agents"), Box::new(crate::claude_ide::AnnotateWithAgents)),
        (t!("cmd-annotate-with-git-blame"), Box::new(e::AnnotateWithGitBlame)),
        (t!("cmd-undo"), Box::new(Undo)),
        (t!("cmd-redo"), Box::new(Redo)),
        (t!("cmd-zoom-in"), Box::new(ZoomIn)),
        (t!("cmd-zoom-out"), Box::new(ZoomOut)),
        (t!("cmd-actual-size"), Box::new(ZoomReset)),
        (t!("cmd-show-project"), Box::new(ShowProject)),
        (t!("cmd-show-git"), Box::new(ShowGit)),
        (t!("cmd-toggle-agent-console"), Box::new(ToggleConsole)),
        (t!("cmd-find-repositories-in-folder"), Box::new(FindRepos)),
        (t!("cmd-clone-from-github"), Box::new(CloneRepo)),
        (t!("cmd-new-group"), Box::new(NewGroup)),
        (t!("cmd-refresh"), Box::new(Refresh)),
        (t!("cmd-pull"), Box::new(Pull)),
        (t!("cmd-fetch"), Box::new(Fetch)),
        (t!("cmd-push"), Box::new(Push)),
        (t!("cmd-commit"), Box::new(Commit)),
        (t!("cmd-draft-commit-message"), Box::new(Draft)),
        (t!("cmd-stage"), Box::new(Stage)),
        (t!("cmd-unstage"), Box::new(Unstage)),
        (t!("cmd-discard-changes"), Box::new(Discard)),
        (t!("cmd-check-out"), Box::new(Checkout)),
        (t!("cmd-new-branch"), Box::new(NewBranch)),
        (t!("cmd-side-by-side-diff"), Box::new(SideBySide)),
        (t!("cmd-unified-diff"), Box::new(Unified)),
        (t!("cmd-compact-diff"), Box::new(Compact)),
        (t!("cmd-theme-match-the-system"), Box::new(ThemeSystem)),
        (t!("cmd-theme-dark"), Box::new(ThemeDark)),
        (t!("cmd-theme-light"), Box::new(ThemeLight)),
        (t!("cmd-open-on-github"), Box::new(Github)),
        (crate::platform::reveal(), Box::new(Reveal)),
        (t!("cmd-open-in-external-terminal"), Box::new(OpenInTerminal)),
        (t!("cmd-about-snoutcode"), Box::new(About)),
        (t!("cmd-check-for-updates"), Box::new(CheckForUpdates)),
        (t!("cmd-release-notes"), Box::new(ReleaseNotes)),
        (t!("cmd-quit-snoutcode"), Box::new(Quit)),
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
    items.push(MenuItem::action(t!("cmd-clear-recent"), ClearRecent));
    items
}

/// The whole bar, its check marks taken from `s`; `recent` and `remote` (the remote
/// machines' projects) fill File ▸ Open Recent.
pub fn bar(s: &Settings, recent: &[std::path::PathBuf], remote: &[crate::remote::RemoteProject]) -> Vec<Menu> {
    let check = |item: MenuItem, on: bool| item.checked(on);
    // Without a global menu bar (Linux) there is no app menu: its items go where IntelliJ IDEA
    // has them there, Settings and Quit at the end of File, About at the end of Help.
    let app_menu = !crate::platform::IN_WINDOW_MENUS;
    let settings_items = || {
        [
            MenuItem::action(t!("cmd-settings"), OpenSettings),
            MenuItem::action(t!("cmd-open-settings-json"), OpenSettingsJson),
            MenuItem::action(t!("cmd-open-project-settings-json"), OpenProjectSettingsJson),
        ]
    };
    let file_end: Vec<MenuItem> = if app_menu {
        Vec::new()
    } else {
        [
            vec![MenuItem::separator()],
            settings_items().into(),
            // Windows calls it Exit.
            vec![
                MenuItem::separator(),
                MenuItem::action(if cfg!(windows) { t!("cmd-exit") } else { t!("cmd-quit") }, Quit),
            ],
        ]
        .into_iter()
        .flatten()
        .collect()
    };
    let help_end: Vec<MenuItem> = if app_menu {
        Vec::new()
    } else {
        vec![
            MenuItem::separator(),
            MenuItem::action(t!("cmd-check-for-updates"), CheckForUpdates),
            MenuItem::action(t!("cmd-about-snoutcode"), About),
        ]
    };
    let app = Menu::new(t!("menu-snoutcode")).items(
        [
            vec![
                MenuItem::action(t!("cmd-about-snoutcode"), About),
                MenuItem::action(t!("cmd-check-for-updates"), CheckForUpdates),
                MenuItem::separator(),
            ],
            settings_items().into(),
            vec![
                MenuItem::separator(),
                MenuItem::os_submenu(t!("menu-services"), SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-quit-snoutcode"), Quit),
            ],
        ]
        .into_iter()
        .flatten(),
    );
    let zoom = if app_menu { t!("cmd-zoom") } else { t!("cmd-maximize") };
    let mut bar = vec![
        Menu::new(t!("menu-file")).items(
            [
                MenuItem::action(t!("cmd-new-file"), NewFile),
                MenuItem::action(t!("cmd-new-project"), NewProject),
                MenuItem::action(t!("cmd-open-project"), Open),
                MenuItem::submenu(Menu::new(t!("menu-open-recent")).items(recent_items(recent, remote))),
                MenuItem::action(t!("cmd-close-project"), CloseProject),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-project-structure"), ProjectStructure),
                MenuItem::action(t!("cmd-open-in-external-terminal"), OpenInTerminal),
                MenuItem::separator(),
                MenuItem::action(
                    if cfg!(windows) { t!("cmd-connect-to-ssh-host-or-wsl") } else { t!("cmd-connect-to-ssh-host") },
                    ConnectSsh,
                ),
                MenuItem::action(t!("cmd-close-remote-connection"), CloseRemote),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-find-repositories-in-folder"), FindRepos),
                MenuItem::action(t!("cmd-clone-from-github"), CloneRepo),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-new-group"), NewGroup),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-search-everywhere"), SearchEverywhere),
                MenuItem::action(t!("cmd-go-to-file"), GoToFile),
                MenuItem::action(t!("cmd-recent-files"), RecentFiles),
                MenuItem::action(t!("cmd-find-in-files"), FindInFiles),
                MenuItem::action(t!("cmd-replace-in-files"), ReplaceInFiles),
                MenuItem::action(t!("cmd-command-palette"), CommandPalette),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-save"), crate::editor::Save),
                MenuItem::action(t!("cmd-reload-all-from-disk"), ReloadFromDisk),
                MenuItem::action(t!("cmd-close-tab"), CloseTab),
            ]
            .into_iter()
            .chain(file_end),
        ),
        Menu::new(t!("menu-edit")).items([
            MenuItem::os_action(t!("cmd-undo"), Undo, OsAction::Undo),
            MenuItem::os_action(t!("cmd-redo"), Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(t!("cmd-cut"), Cut, OsAction::Cut),
            MenuItem::os_action(t!("cmd-copy"), Copy, OsAction::Copy),
            MenuItem::os_action(t!("cmd-paste"), Paste, OsAction::Paste),
            MenuItem::os_action(t!("cmd-select-all"), SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-find"), gpui_component::input::Search),
            MenuItem::action(t!("cmd-replace"), gpui_component::input::Replace),
            MenuItem::action(t!("cmd-find-in-files"), FindInFiles),
            MenuItem::action(t!("cmd-replace-in-files"), ReplaceInFiles),
            MenuItem::action(t!("cmd-undo-replace-in-files"), UndoReplaceInFiles),
            MenuItem::action(t!("cmd-go-to-line"), GoToLine),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-toggle-comment"), crate::editor::ToggleComment),
            MenuItem::action(t!("cmd-duplicate-line"), crate::editor::DuplicateLine),
            MenuItem::action(t!("cmd-move-line-up"), crate::editor::MoveLineUp),
            MenuItem::action(t!("cmd-move-line-down"), crate::editor::MoveLineDown),
        ]),
        Menu::new(t!("menu-view")).items([
            MenuItem::action(t!("cmd-project"), ShowProject),
            MenuItem::action(t!("cmd-git"), ShowGit),
            MenuItem::action(t!("cmd-structure"), ShowStructure),
            MenuItem::action(t!("cmd-hierarchy"), ShowHierarchy),
            MenuItem::submenu(Menu::new(t!("menu-tool-window-actions")).items([
                MenuItem::action(t!("cmd-select-opened-file"), LocateFile),
                MenuItem::action(t!("cmd-collapse-the-project-tree"), CollapseAll),
                MenuItem::action(t!("cmd-hide-the-side-panel"), HideSide),
                MenuItem::separator(),
                MenuItem::action(t!("cmd-sort-structure-by-name"), SortStructure),
                MenuItem::action(t!("cmd-collapse-all-gradle-tasks"), CollapseGradle),
                MenuItem::action(t!("cmd-collapse-all-maven-goals"), CollapseMaven),
                MenuItem::action(t!("cmd-collapse-the-spring-window"), CollapseSpring),
                MenuItem::action(t!("cmd-clear-output"), ClearOutput),
            ])),
            check(MenuItem::action(t!("cmd-agent-console"), ToggleConsole), s.console_open),
            MenuItem::separator(),
            check(MenuItem::action(t!("cmd-side-by-side-diff"), SideBySide), s.side_by_side),
            check(MenuItem::action(t!("cmd-unified-diff"), Unified), !s.side_by_side),
            check(MenuItem::action(t!("cmd-compact-diff"), Compact), s.compact),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-zoom-in"), ZoomIn),
            MenuItem::action(t!("cmd-zoom-out"), ZoomOut),
            MenuItem::action(t!("cmd-actual-size"), ZoomReset),
            MenuItem::action(t!("cmd-soft-wrap"), crate::editor::ToggleSoftWrap),
            check(MenuItem::action(t!("cmd-parameter-name-hints"), ToggleParamHints), s.param_hints),
            MenuItem::action(t!("cmd-split-editor-right"), SplitRight),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-output"), ShowOutput),
            MenuItem::action(t!("cmd-processes"), ShowProcesses),
            MenuItem::action(t!("cmd-agent-log"), ShowAgentLog),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new(t!("menu-appearance")).items([
                check(MenuItem::action(ThemeChoice::System.label(), ThemeSystem), s.theme == ThemeChoice::System),
                check(MenuItem::action(ThemeChoice::Light.label(), ThemeLight), s.theme == ThemeChoice::Light),
                check(MenuItem::action(ThemeChoice::Dark.label(), ThemeDark), s.theme == ThemeChoice::Dark),
            ])),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-refresh"), Refresh),
        ]),
        Menu::new(t!("menu-code")).items([
            MenuItem::action(t!("cmd-go-to-definition"), gpui_component::input::GoToDefinition),
            MenuItem::action(t!("cmd-find-references"), crate::editor::FindReferences),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-show-context-actions"), crate::editor::ShowContextActions),
            MenuItem::action(t!("cmd-apply-top-fix"), crate::editor::ApplyTopFix),
            MenuItem::action(t!("cmd-format-document"), crate::editor::FormatDocument),
            MenuItem::action(t!("cmd-format-selection"), crate::editor::FormatSelection),
            MenuItem::action(t!("cmd-organize-imports"), crate::editor::OrganizeImports),
            MenuItem::action(t!("cmd-show-parameter-hints"), crate::editor::ShowSignature),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-show-problems"), ShowProblems),
            MenuItem::action(t!("cmd-go-to-spring-bean-or-usages"), SpringAtCaret),
            MenuItem::action(t!("cmd-spring-active-profiles"), SpringProfiles),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-send-selection-to-claude"), crate::editor::SendToClaude),
            MenuItem::action(t!("cmd-accept-all-claude-changes"), crate::claude_ide::AcceptAllClaude),
            MenuItem::action(t!("cmd-revert-all-claude-changes"), crate::claude_ide::RevertAllClaude),
            MenuItem::action(t!("cmd-show-agent-changes"), crate::claude_ide::ShowAgentChanges),
            MenuItem::action(t!("cmd-annotate-with-agents"), crate::claude_ide::AnnotateWithAgents),
        ]),
        Menu::new(t!("menu-navigate")).items([
            MenuItem::action(t!("cmd-go-to-declaration-or-usages"), crate::editor::GoToDeclaration),
            MenuItem::action(t!("cmd-go-to-implementations"), crate::editor::GoToImplementations),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-go-to-class"), GoToClass),
            MenuItem::action(t!("cmd-go-to-file"), GoToFile),
            MenuItem::action(t!("cmd-go-to-symbol-in-workspace"), GoToSymbol),
            MenuItem::action(t!("cmd-go-to-symbol-in-file"), GoToSymbolInFile),
            MenuItem::action(t!("cmd-go-to-line"), GoToLine),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-file-structure"), FileStructure),
            MenuItem::action(t!("cmd-type-hierarchy"), TypeHierarchy),
            MenuItem::action(t!("cmd-call-hierarchy"), CallHierarchy),
        ]),
        Menu::new(t!("menu-refactor")).items([
            MenuItem::action(t!("cmd-rename-symbol"), crate::editor::RenameSymbol),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-extract-method"), crate::editor::ExtractMethod),
            MenuItem::action(t!("cmd-extract-variable"), crate::editor::ExtractVariable),
            MenuItem::action(t!("cmd-extract-constant"), crate::editor::ExtractConstant),
            MenuItem::action(t!("cmd-extract-field"), crate::editor::ExtractField),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-inline"), crate::editor::Inline),
        ]),
        Menu::new(t!("menu-run")).items([
            MenuItem::action(t!("cmd-run"), RunSelected),
            MenuItem::action(t!("cmd-debug"), DebugSelected),
            MenuItem::action(t!("cmd-run-context-configuration"), RunContext),
            MenuItem::action(t!("cmd-debug-context-configuration"), DebugContext),
            MenuItem::action(t!("cmd-stop"), StopRuns),
            MenuItem::action(t!("cmd-rerun"), Rerun),
            MenuItem::action(t!("cmd-rerun-failed-tests"), RerunFailedTests),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-edit-configurations"), EditConfigurations),
            MenuItem::action(t!("cmd-edit-environments"), EditEnvironments),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-record-at-cursor"), RecordAtCursor),
            MenuItem::action(t!("cmd-pick-locator"), PickLocator),
            MenuItem::action(t!("cmd-open-ui-mode"), OpenUiMode),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-toggle-breakpoint"), ToggleBreakpoint),
            MenuItem::action(t!("cmd-toggle-breakpoint-enabled"), ToggleBreakpointEnabled),
            MenuItem::action(t!("cmd-edit-breakpoint-condition"), EditBreakpointCondition),
            MenuItem::action(t!("cmd-view-breakpoints"), ViewBreakpoints),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-step-over"), StepOver),
            MenuItem::action(t!("cmd-step-into"), StepInto),
            MenuItem::action(t!("cmd-step-out"), StepOut),
            MenuItem::action(t!("cmd-run-to-cursor"), RunToCursor),
            MenuItem::action(t!("cmd-pause-program"), PauseProgram),
            MenuItem::action(t!("cmd-resume-program"), Resume),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-evaluate-expression"), EvaluateExpression),
        ]),
        Menu::new(t!("menu-build")).items([
            MenuItem::action(t!("cmd-build-project"), BuildProject),
            MenuItem::action(t!("cmd-rebuild-project"), RebuildProject),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-sync-gradle-and-maven-projects"), SyncGradle),
            MenuItem::action(t!("cmd-refresh-gradle-tasks"), RefreshGradle),
            MenuItem::action(t!("cmd-read-the-maven-goals-again"), RefreshMaven),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-run-gradle-task"), RunGradleTask),
            MenuItem::action(t!("cmd-run-maven-goal"), RunMavenGoals),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-build-window"), ShowBuild),
            MenuItem::action(t!("cmd-gradle"), ShowGradle),
            MenuItem::action(t!("cmd-maven"), ShowMaven),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-project-structure"), ProjectStructure),
        ]),
        Menu::new(t!("menu-git")).items([
            MenuItem::action(t!("cmd-pull"), Pull),
            MenuItem::action(t!("cmd-fetch"), Fetch),
            MenuItem::action(t!("cmd-push"), Push),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-commit"), Commit),
            MenuItem::action(t!("cmd-draft-commit-message"), Draft),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-stage"), Stage),
            MenuItem::action(t!("cmd-unstage"), Unstage),
            MenuItem::action(t!("cmd-discard-changes"), Discard),
            MenuItem::action(t!("cmd-move-to-trash"), Trash),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-check-out"), Checkout),
            MenuItem::action(t!("cmd-new-branch"), NewBranch),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-annotate-with-git-blame"), crate::editor::AnnotateWithGitBlame),
            MenuItem::action(t!("cmd-annotate-with-agents"), crate::claude_ide::AnnotateWithAgents),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-open-on-github"), Github),
            MenuItem::action(crate::platform::reveal(), Reveal),
        ]),
        Menu::new(t!("menu-tools")).items([
            MenuItem::action(t!("cmd-terminal"), ShowTerminal),
            MenuItem::action(t!("cmd-open-in-external-terminal"), OpenInTerminal),
            MenuItem::separator(),
            check(MenuItem::action(t!("cmd-agent-console"), ToggleConsole), s.console_open),
            MenuItem::action(t!("cmd-agent-changes"), crate::claude_ide::ShowAgentChanges),
            MenuItem::action(t!("cmd-annotate-with-agents"), crate::claude_ide::AnnotateWithAgents),
            MenuItem::separator(),
            MenuItem::action(t!("cmd-processes"), ShowProcesses),
            MenuItem::action(t!("cmd-agent-log"), ShowAgentLog),
            MenuItem::action(t!("cmd-stop-idle-processes"), StopIdleProcesses),
            MenuItem::action(t!("cmd-restart-the-java-engine"), RestartJava),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new(t!("menu-tool-settings")).items([
                MenuItem::action(t!("cmd-keymap"), SettingsKeymap),
                MenuItem::action(t!("cmd-terminal-more"), SettingsTerminal),
                MenuItem::action(t!("cmd-coding-agents"), SettingsAgents),
                MenuItem::action(t!("cmd-gradle-and-maven"), SettingsBuildTools),
            ])),
            MenuItem::action(t!("cmd-settings"), OpenSettings),
        ]),
        Menu::new(t!("menu-window"))
            .items([MenuItem::action(t!("cmd-minimize"), Minimize), MenuItem::action(zoom, Zoom)]),
        Menu::new(t!("menu-help")).items(
            [
                MenuItem::action(t!("cmd-snoutcode-on-github"), Source),
                MenuItem::action(t!("cmd-report-an-issue"), Issues),
                MenuItem::action(t!("cmd-snoutdata-website"), Website),
                MenuItem::action(t!("cmd-release-notes"), ReleaseNotes),
            ]
            .into_iter()
            .chain(help_end),
        ),
    ];
    if app_menu {
        bar.insert(0, app);
    }
    bar
}

/// What the bar shows that can change, to rebuild it only then.
pub type MenuState = (bool, bool, bool, bool, ThemeChoice, Vec<std::path::PathBuf>, Vec<crate::remote::RemoteProject>);

pub fn checks(s: &Settings, recent: &[std::path::PathBuf], remote: &[crate::remote::RemoteProject]) -> MenuState {
    (s.console_open, s.side_by_side, s.compact, s.param_hints, s.theme, recent.to_vec(), remote.to_vec())
}
