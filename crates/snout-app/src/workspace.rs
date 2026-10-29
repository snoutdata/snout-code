//! The window's content: a toolbar, the activity bar beside GPUI Kit's dock area (the left
//! dock holding the panel the activity bar picked, tabs in the center, the Claude Code
//! console and Output in the bottom dock), and a status bar.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, ExternalPaths, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Window, div, px,
};
use gpui::{Global, Task};
use gpui_component::Theme;
use gpui_component::dock::{
    BasePanelView, DockArea, DockAreaState, DockEvent, DockLayout, DockPlacement, DockToggleEvent, PanelId,
    panel_handle, register_panel,
};
use snout_core::{FileEntry, PullMode, on_disk, stage_paths, unstage_paths};

use crate::editor::{EditorEvent, EditorPanel};
use crate::git_view::GitView;
use crate::menus::{self, *};
use crate::model::{Action, Dialog, Model, ModelEvent};
use crate::panels::{Activity, OutputPanel, SidePanel, WelcomePanel};
use crate::projects::ProjectsView;
use crate::search::{Mode, SearchEvent, SearchView};
use crate::settings::ThemeChoice;
use crate::status::StatusBar;
use crate::theme::{self, palette};

mod lang_ui;
mod remote_ui;
mod run_ui;
pub mod session;
mod stripes;
mod title;
pub use stripes::{STRIPE_W, Tool, active_in};
pub use title::{TITLE_H, badge_color, branch_popup, initials};

/// The center and bottom tabs, so events can bring one forward.
pub struct Tabs {
    pub welcome: PanelId,
    pub console: Option<PanelId>,
    pub terminal: PanelId,
    pub output: PanelId,
    pub problems: PanelId,
    pub gradle: PanelId,
    pub spring: PanelId,
    pub notifications: PanelId,
    pub claude_changes: PanelId,
    pub structure: PanelId,
    pub hierarchy: PanelId,
    pub run: PanelId,
    pub debug: PanelId,
    pub build: PanelId,
    pub processes: PanelId,
}

pub struct Workspace {
    pub model: Entity<Model>,
    pub dock: Entity<DockArea>,
    pub side: Entity<SidePanel>,
    /// The Git view: the whole area right of the activity bar while Git is picked.
    pub git: Entity<GitView>,
    pub tabs: Tabs,
    menu_checks: Option<menus::MenuState>,
    /// Holds the keyboard when no field or panel does, so shortcuts reach the workspace.
    focus: gpui::FocusHandle,
    title: String,
    /// The go-to-file or find-in-files box, while it shows.
    pub search: Option<Entity<SearchView>>,
    /// Find or Replace in Files, while it shows.
    pub find_in_files: Option<Entity<crate::find_in_files::FindInFiles>>,
    /// The last Replace in Files, to undo.
    pub replace_undo: Option<crate::find_in_files::ReplaceUndo>,
    /// The editor tabs, to find the one a file already has.
    editors: Vec<gpui::WeakEntity<EditorPanel>>,
    /// The commit HEAD was at, to notice when it moves.
    head: Option<String>,
    /// The status bar under the docks; it follows the editor in front.
    pub status: Entity<StatusBar>,
    /// The language servers.
    pub hub: Entity<crate::lang::LangHub>,
    /// The bottom dock's Problems and References tabs.
    pub problems: Entity<crate::lang::panels::ProblemsPanel>,
    pub references: Entity<crate::lang::panels::ReferencesPanel>,
    /// The title bar's branch popup, while it shows.
    branch_popup: Option<Entity<crate::branch_popup::BranchPopup>>,
    /// The Settings sheet, while it shows.
    pub settings_sheet: Option<Entity<crate::settings_view::SettingsView>>,
    /// The Settings sheet is about to come up (the host's JDKs are being looked for).
    settings_opening: bool,
    /// The right stripe's tool windows.
    pub gradle: Entity<crate::run::gradle_window::GradleWindow>,
    pub spring_window: Entity<crate::spring::window::SpringWindow>,
    /// The project's Spring model, for the Spring window and the editors' gutters.
    pub spring: Entity<crate::spring::Spring>,
    pub notifications: Entity<crate::tool_windows::NotificationsPanel>,
    /// The processes SnoutCode runs, and their tool window.
    pub processes: Entity<crate::processes::Processes>,
    pub processes_window: Entity<crate::processes::window::ProcessesWindow>,
    /// Runs, tests and the debugger, and their tool windows.
    pub runs: Entity<crate::run::Runs>,
    pub run_window: Entity<crate::run::windows::RunWindow>,
    pub debug_window: Entity<crate::run::windows::RunWindow>,
    pub build_window: Entity<crate::run::windows::RunWindow>,
    /// The Welcome page, while no file is open.
    pub welcome: Entity<WelcomePanel>,
    /// The Edit Configurations dialog, while it shows.
    pub config_dialog: Option<Entity<crate::run::config_dialog::ConfigDialog>>,
    /// The New Project dialog, while it shows.
    pub new_project: Option<Entity<crate::new_project::NewProjectView>>,
    /// Connect to SSH Host, while it shows.
    pub connect_dialog: Option<Entity<crate::remote::dialog::ConnectDialog>>,
    /// The folder picker of a remote machine, while it shows.
    pub remote_picker: Option<Entity<crate::remote::picker::FolderPicker>>,
    /// Claude Code's connection to this window, when the app runs one.
    pub claude: Option<crate::claude_ide::bridge::Bridge>,
    /// The Structure tool window: the symbols of the file in front.
    pub structure: Entity<crate::lang::structure::StructureWindow>,
    /// The Hierarchy tool window: a type's supertypes or subtypes, a method's calls.
    pub hierarchy: Entity<crate::lang::hierarchy::HierarchyWindow>,
    /// The Claude Changes tool window, and the session's changes it shows.
    pub claude_changes: Entity<crate::claude_ide::panel::ChangesPanel>,
    /// Every panel the window made, and where it goes when a layout does not place it.
    panels: Vec<(Arc<dyn BasePanelView>, DockPlacement)>,
    /// The project whose tabs show.
    shown: Option<PathBuf>,
    _save_layout: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// The width the stripes take from the window, both edges together.
pub const ACTIVITY_W: f32 = 2. * STRIPE_W;

/// The layout format this build writes; a saved layout of another version is ignored.
const LAYOUT_VERSION: usize = 2;

/// The panels of the window being restored, by name, for the dock's panel registry.
struct PanelPool(HashMap<&'static str, Arc<dyn BasePanelView>>, Vec<Entity<EditorPanel>>);

impl Global for PanelPool {}

/// The first layout: the side panel on the left, the tabs in the center, the console and
/// Output in the bottom dock.
fn default_layout(
    dock: &Entity<DockArea>,
    panels: &[(Arc<dyn BasePanelView>, DockPlacement)],
    window: &mut Window,
    cx: &mut App,
) {
    dock.update(cx, |d, cx| {
        let mut regions = [
            (DockPlacement::Left, DockLayout::tabs(), px(320.)),
            (DockPlacement::Center, DockLayout::tabs(), px(0.)),
            (DockPlacement::Bottom, DockLayout::tabs(), px(260.)),
            (DockPlacement::Right, DockLayout::tabs(), px(300.)),
        ];
        for (view, place) in panels {
            if let Some((_, layout, _)) = regions.iter_mut().find(|(p, _, _)| p == place) {
                *layout = std::mem::replace(layout, DockLayout::tabs()).panel_view(view.clone(), cx);
            }
        }
        for (place, layout, size) in regions {
            if place == DockPlacement::Center {
                d.set_center(layout, window, cx);
            } else {
                d.set_dock(place, layout, window, cx);
                d.set_dock_size(place, size, window, cx);
            }
        }
        // The right stripe's tool windows start hidden.
        if d.is_dock_open(DockPlacement::Right) {
            d.toggle_dock(DockPlacement::Right, window, cx);
        }
    });
}

/// Put back the layout `state` (saved last time, or a project's tabs over the layout as it
/// is), with the panels this window already made; a panel the saved layout lacks (one
/// added since) goes to its usual place. What this build cannot place is left out first:
/// a panel it no longer has, a file deleted since, a tab of a project other than
/// `project` (see [`session::sanitize`]). False when the saved layout cannot be used.
/// Editor tabs open their files again, the caret and the view where they were; they come
/// back too.
fn restore_layout(
    dock: &Entity<DockArea>,
    state: serde_json::Value,
    panels: &[(Arc<dyn BasePanelView>, DockPlacement)],
    project: Option<&std::path::Path>,
    window: &mut Window,
    cx: &mut App,
) -> (bool, Vec<Entity<EditorPanel>>) {
    if state.get("version").and_then(|v| v.as_u64()) != Some(LAYOUT_VERSION as u64) {
        return (false, Vec::new());
    }
    let pool: HashMap<&'static str, Arc<dyn BasePanelView>> =
        panels.iter().map(|(view, _)| (view.panel_name(cx), view.clone())).collect();
    let names: std::collections::HashSet<String> = pool.keys().map(|n| n.to_string()).collect();
    let keep = session::Keep { panels: &names, editor: crate::editor::PANEL_NAME, project, host: None };
    let Ok(state) = serde_json::from_value::<DockAreaState>(session::sanitize(state, &keep)) else {
        return (false, Vec::new());
    };
    for name in pool.keys().copied() {
        register_panel(cx, name, move |_, _, cx| cx.global::<PanelPool>().0[name].clone());
    }
    register_panel(cx, crate::editor::PANEL_NAME, |ctx, window, cx| {
        let class_file = crate::editor::saved_class_file(ctx.state());
        let path = if class_file.is_some() {
            PathBuf::new()
        } else {
            crate::editor::saved_path(ctx.state()).unwrap_or_default()
        };
        let editor = cx.new(|cx| EditorPanel::open(path, window, cx));
        editor.update(cx, |e, _| e.close_if_gone = true);
        if let Some((uri, root)) = class_file {
            let loading = "// Loading the class from the Java server…\n".to_owned();
            editor.update(cx, |e, cx| e.show_class_file(uri, root, loading, window, cx));
        } else if let Some((line, column, scroll)) = crate::editor::saved_view(ctx.state()) {
            editor.update(cx, |e, cx| e.restore_view(line, column, scroll, cx));
        }
        cx.global_mut::<PanelPool>().1.push(editor.clone());
        panel_handle(editor)
    });
    cx.set_global(PanelPool(pool, Vec::new()));
    let loaded = dock.update(cx, |d, cx| d.load(state, window, cx));
    let editors = cx.remove_global::<PanelPool>().1;
    if loaded.is_err() {
        return (false, Vec::new());
    }
    dock.update(cx, |d, cx| {
        // A layout saved before the right stripe's tool windows has no right dock: they
        // join one that starts hidden.
        let had_right = d.has_dock(DockPlacement::Right);
        // The Welcome page only fills a center the layout left empty.
        let center_empty = d.layout(DockPlacement::Center).is_none_or(|t| t.panels().next().is_none());
        for (view, place) in panels {
            if d.panel(view.panel_id(cx)).is_none() && (*place != DockPlacement::Center || center_empty) {
                d.add_panel_view(view.clone(), *place, None, window, cx);
            }
        }
        if !had_right && d.is_dock_open(DockPlacement::Right) {
            d.toggle_dock(DockPlacement::Right, window, cx);
        }
    });
    (true, editors)
}

impl Workspace {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let cx_ws = cx.weak_entity();
        // Restored editor tabs start with the settings' defaults.
        let defaults = crate::editor::EditorDefaults::of(&model.read(cx).settings);
        cx.set_global(defaults);
        // The language servers come first: restored editor tabs connect to them.
        let hub = cx.new(|cx| crate::lang::LangHub::new(model.clone(), cx));
        cx.set_global(crate::lang::hub::HubHandle(hub.clone()));
        // The window first: the Java servers start once it has drawn (see `render`).
        hub.update(cx, |h, _| h.paused = true);
        let runs = cx.new(|cx| crate::run::Runs::new(model.clone(), hub.clone(), cx));
        cx.set_global(crate::run::RunsHandle(runs.clone()));
        let marks = runs.read(cx).marks.clone();
        cx.set_global(crate::run::MarksHandle(marks));
        let processes = cx.new(|cx| crate::processes::Processes::new(model.clone(), hub.clone(), runs.clone(), cx));
        cx.set_global(crate::processes::ProcessesHandle(processes.clone()));
        let spring = cx.new(|cx| crate::spring::Spring::new(runs.clone(), hub.clone(), cx));
        cx.set_global(crate::spring::SpringHandle(spring.clone()));
        let problems = cx.new(|cx| crate::lang::panels::ProblemsPanel::new(hub.clone(), cx));
        let references = cx.new(|cx| crate::lang::panels::ReferencesPanel::new(hub.clone(), cx));
        let projects = cx.new(|cx| ProjectsView::new(model.clone(), window, cx));
        let files = cx.new(|cx| crate::files::FileTree::new(model.clone(), cx));
        let models = cx.new(|cx| crate::project_model::ProjectModels::new(model.clone(), Some(hub.clone()), cx));
        cx.set_global(crate::project_model::ProjectModelsHandle(models.clone()));
        let project =
            cx.new(|cx| crate::project_view::ProjectView::new(model.clone(), files.clone(), models.clone(), cx));
        let side = cx.new(|cx| SidePanel::new(model.clone(), files, project.clone(), cx));
        let welcome = cx.new(|cx| WelcomePanel::new(model.clone(), window, cx));
        let output = cx.new(|cx| OutputPanel::new(model.clone(), cx));
        let console = crate::console::build(&model, window, cx);
        let console_view = console.as_ref().map(|c| c.view());
        let git = cx.new(|cx| GitView::new(model.clone(), projects.into(), console_view, window, cx));
        let (dock, _skin) = crate::tabs::dock_area("snout-code", Some(LAYOUT_VERSION), window, cx);

        // Every panel, where it goes when the saved layout does not place it.
        let mut panels: Vec<(Arc<dyn BasePanelView>, DockPlacement)> = vec![
            (panel_handle(side.clone()), DockPlacement::Left),
            (panel_handle(welcome.clone()), DockPlacement::Center),
        ];
        let terminal = crate::console::build_terminal(&model, cx);
        let gradle = cx.new(|cx| crate::run::gradle_window::GradleWindow::new(runs.clone(), cx));
        use crate::run::Place;
        use crate::run::windows::RunWindow;
        let run_window = cx.new(|cx| RunWindow::new(runs.clone(), Place::Run, window, cx));
        let debug_window = cx.new(|cx| RunWindow::new(runs.clone(), Place::Debug, window, cx));
        let build_window = cx.new(|cx| RunWindow::new(runs.clone(), Place::Build, window, cx));
        let notifications = cx.new(|cx| crate::tool_windows::NotificationsPanel::new(model.clone(), cx));
        let claude_changes = cx.new(|cx| crate::claude_ide::panel::ChangesPanel::new(cx_ws.clone(), cx));
        let processes_window = cx.new(|cx| crate::processes::window::ProcessesWindow::new(processes.clone(), cx));
        let spring_window = cx.new(|cx| crate::spring::window::SpringWindow::new(spring.clone(), window, cx));
        let structure = cx.new(|cx| crate::lang::structure::StructureWindow::new(window, cx));
        let hierarchy = cx.new(|cx| crate::lang::hierarchy::HierarchyWindow::new(hub.clone(), cx));
        let mut tabs = Tabs {
            welcome: PanelId::from(welcome.entity_id()),
            console: None,
            terminal: terminal.panel_id(cx),
            output: PanelId::from(output.entity_id()),
            problems: PanelId::from(problems.entity_id()),
            gradle: PanelId::from(gradle.entity_id()),
            spring: PanelId::from(spring_window.entity_id()),
            notifications: PanelId::from(notifications.entity_id()),
            claude_changes: PanelId::from(claude_changes.entity_id()),
            structure: PanelId::from(structure.entity_id()),
            hierarchy: PanelId::from(hierarchy.entity_id()),
            run: PanelId::from(run_window.entity_id()),
            debug: PanelId::from(debug_window.entity_id()),
            build: PanelId::from(build_window.entity_id()),
            processes: PanelId::from(processes_window.entity_id()),
        };
        if let Some(console) = console {
            tabs.console = Some(console.panel_id(cx));
            panels.push((console, DockPlacement::Bottom));
        }
        panels.push((terminal, DockPlacement::Bottom));
        panels.push((panel_handle(output.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(problems.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(references.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(run_window.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(debug_window.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(build_window.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(processes_window.clone()), DockPlacement::Bottom));
        panels.push((panel_handle(structure.clone()), DockPlacement::Right));
        panels.push((panel_handle(hierarchy.clone()), DockPlacement::Right));
        panels.push((panel_handle(gradle.clone()), DockPlacement::Right));
        panels.push((panel_handle(spring_window.clone()), DockPlacement::Right));
        panels.push((panel_handle(notifications.clone()), DockPlacement::Right));
        panels.push((panel_handle(claude_changes.clone()), DockPlacement::Right));

        let saved = model.read(cx).settings.dock.clone();
        let root = model.read(cx).project_root();
        let restored = match saved.clone() {
            Some(state) => restore_layout(&dock, state, &panels, root.as_deref(), window, cx),
            None => (false, Vec::new()),
        };
        // A layout this build cannot use (an earlier build's) gives way to the usual one;
        // the project's files it had open open again in it.
        let mut reopen = Vec::new();
        if !restored.0 {
            default_layout(&dock, &panels, window, cx);
            if let (Some(saved), Some(root)) = (&saved, &root) {
                reopen = session::editor_files(saved, crate::editor::PANEL_NAME);
                // A file gone since closes its tab once it is found so (off the UI thread).
                reopen.retain(|p| p.starts_with(root));
            }
        }
        let open = model.read(cx).settings.console_open;
        dock.update(cx, |d, cx| {
            d.set_dock_collapsible(DockPlacement::Bottom, true, window, cx);
            d.set_dock_collapsible(DockPlacement::Right, true, window, cx);
            if d.is_dock_open(DockPlacement::Bottom) != open {
                d.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });

        let subscriptions = vec![
            // With the theme on System, the window turns light or dark when the Mac does.
            theme::follow_system(window),
            cx.observe_in(&model, window, |this, model, window, cx| {
                // A project opened with none open before: its tabs come back.
                let root = model.read(cx).project_root();
                if root != this.shown {
                    let was = std::mem::replace(&mut this.shown, root.clone());
                    if was.is_none() && root.is_some() {
                        this.load_session(window, cx);
                    }
                }
                // A new commit (or another branch): the editors' gutters compare with it.
                let head = model.read(cx).log.first().map(|c| c.id.clone());
                if head != this.head {
                    this.head = head;
                    for editor in this.editors(cx) {
                        editor.update(cx, |e, cx| e.refresh_head(cx));
                    }
                }
                this.ensure_project_server(cx);
                cx.notify()
            }),
            cx.subscribe_in(&dock, window, |this, _, e: &DockEvent, window, cx| {
                if matches!(e, DockEvent::LayoutChanged) {
                    this.keep_welcome(window, cx);
                    this.save_layout_soon(cx);
                }
            }),
            // The bottom dock's own collapse control shows or hides the console too.
            cx.subscribe(&dock, |this, _, e: &DockToggleEvent, cx| {
                if e.placement() == DockPlacement::Bottom {
                    this.model.update(cx, |m, cx| {
                        m.settings.console_open = e.is_open();
                        m.persist();
                        cx.notify();
                    });
                }
            }),
            cx.subscribe_in(&model, window, Self::on_model_event),
            cx.subscribe_in(&hub, window, Self::on_hub_event),
            cx.subscribe_in(&runs, window, Self::on_runs_event),
            cx.subscribe_in(&spring, window, Self::on_spring_event),
            cx.subscribe_in(&project, window, |this, _, e: &crate::project_view::ProjectViewEvent, window, cx| {
                let crate::project_view::ProjectViewEvent::OpenClass { package, class } = e;
                this.open_library_class(package.clone(), class.clone(), window, cx);
            }),
            // The status bar lists the Scala compile and the Gradle runs among the tasks.
            cx.observe(&runs, |this, _, cx| this.status.update(cx, |_, cx| cx.notify())),
            // The status bar lists the model's sync among the background tasks.
            cx.observe(&models, |this, _, cx| {
                this.status.update(cx, |_, cx| cx.notify());
                cx.notify()
            }),
            // Coming back to the window, re-read the repository and count again, so
            // changes made elsewhere (terminal, editor, an agent) show up at once.
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() && this.model.read(cx).job.is_none() {
                    this.model.update(cx, |m, cx| {
                        m.refresh(cx);
                        m.survey(true, cx);
                        cx.notify();
                    });
                }
            }),
        ];
        let status = cx.new(|cx| StatusBar::new(model.clone(), cx));
        status.update(cx, |s, cx| s.set_hub(hub.clone(), cx));
        let mut this = Self {
            model,
            dock,
            side,
            git,
            tabs,
            menu_checks: None,
            focus: cx.focus_handle(),
            title: String::new(),
            search: None,
            find_in_files: None,
            replace_undo: None,
            editors: Vec::new(),
            head: None,
            status,
            hub,
            problems,
            references,
            branch_popup: None,
            settings_sheet: None,
            settings_opening: false,
            gradle,
            spring_window,
            spring,
            notifications,
            processes,
            processes_window,
            runs,
            run_window,
            debug_window,
            build_window,
            welcome,
            config_dialog: None,
            new_project: None,
            connect_dialog: None,
            remote_picker: None,
            claude: None,
            structure,
            hierarchy,
            claude_changes,
            panels,
            shown: root.clone(),
            _save_layout: None,
            _subscriptions: subscriptions,
        };
        crate::claude_ide::bridge::start(&mut this, window, cx);
        this.model.update(cx, |m, _| m.confirm_switch = true);
        this.adopt_editors(restored.1, window, cx);
        for path in reopen {
            let editor = this.reveal_file(path, false, true, window, cx);
            editor.update(cx, |e, _| e.close_if_gone = true);
        }
        // What the user arranged is kept on the way out: the caret and the view of each
        // tab move without saying so.
        cx.on_app_quit(|this, cx| {
            this.save_layout(cx);
            async {}
        })
        .detach();
        // Open on the activity the window showed last.
        if let Some(activity) = this.model.read(cx).settings.activity {
            this.show_activity(activity, window, cx);
        }
        let size = this.model.read(cx).settings.editor_font_size;
        cx.set_global(crate::editor::EditorFont(size.clamp(crate::editor::FONT_MIN, crate::editor::FONT_MAX)));
        // Closing the window with unsaved changes asks first.
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |this, cx| {
                this.save_layout(cx);
                this.confirm_unsaved(false, window, cx)
            })
            .unwrap_or(true)
        });
        window.focus(&this.focus, cx);
        // A file named on the command line.
        if let Some((path, at)) = this.model.update(cx, |m, _| m.pending_file.take()) {
            this.open_file(path, at, false, window, cx);
        }
        this
    }

    /// Make the editors' font larger or smaller by `by` points, or the usual size again.
    pub fn zoom(&mut self, by: Option<f32>, cx: &mut Context<Self>) {
        let size = match by {
            Some(by) => (crate::editor::font_size(cx) + by).clamp(crate::editor::FONT_MIN, crate::editor::FONT_MAX),
            None => crate::look::MONO_SIZE,
        };
        cx.set_global(crate::editor::EditorFont(size));
        self.model.update(cx, |m, _| {
            m.settings.editor_font_size = size;
            m.persist();
        });
        for editor in self.editors(cx) {
            editor.update(cx, |_, cx| cx.notify());
        }
    }

    /// ⌘W: close the editor tab with the keyboard, asking first if it has changes.
    pub fn close_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self
            .editors(cx)
            .into_iter()
            .find(|e| gpui::Focusable::focus_handle(e.read(cx).state.read(cx), cx).contains_focused(window, cx));
        let Some(editor) = focused else { return };
        let dock = self.dock.clone();
        let e = editor.clone();
        crate::editor::confirm_close(Some(editor), window, cx, move |window, cx| {
            dock.update(cx, |d, cx| d.remove_panel(e, window, cx));
        });
    }

    /// Ask about the tabs with unsaved changes, if any, and go on with `then` once they are
    /// saved or dropped; Cancel (or a save that fails) stops there. True when nothing had
    /// to be asked and `then` ran at once.
    fn settle_unsaved(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> bool {
        let dirty: Vec<Entity<EditorPanel>> = self.editors(cx).into_iter().filter(|e| e.read(cx).dirty).collect();
        if dirty.is_empty() {
            then(self, window, cx);
            return true;
        }
        let message = match dirty.as_slice() {
            [one] => format!("Do you want to save the changes you made to {}?", one.read(cx).name()),
            many => format!("Do you want to save the changes you made to {} files?", many.len()),
        };
        let answer = window.prompt(
            gpui::PromptLevel::Warning,
            &message,
            Some("Your changes are lost if you don't save them."),
            &["Save", "Don't Save", "Cancel"],
            cx,
        );
        let this = cx.weak_entity();
        window
            .spawn(cx, async move |cx| {
                let Ok(answer) = answer.await else { return };
                match answer {
                    0 => {
                        // Written off the UI thread; a save that fails stops here.
                        let Ok(saves) = cx
                            .update(|_, cx| dirty.iter().map(|e| e.update(cx, |e, cx| e.save(cx))).collect::<Vec<_>>())
                        else {
                            return;
                        };
                        let mut all = true;
                        for s in saves {
                            all &= s.await;
                        }
                        if !all {
                            return;
                        }
                    }
                    1 => {
                        let _ = cx.update(|_, cx| {
                            for e in &dirty {
                                e.update(cx, |e, cx| e.discard(cx));
                            }
                        });
                    }
                    _ => return,
                }
                let _ = cx.update(|window, cx| {
                    let _ = this.update(cx, |this, cx| then(this, window, cx));
                });
            })
            .detach();
        false
    }

    /// Before the window closes or the app quits: true when no tab has unsaved changes.
    /// Otherwise ask whether to save them; the answer saves (or drops) them and closes
    /// the window (or quits) itself.
    pub fn confirm_unsaved(&mut self, quit: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let dirty = self.editors(cx).into_iter().any(|e| e.read(cx).dirty);
        if !dirty {
            return true;
        }
        self.settle_unsaved(window, cx, move |_, window, cx| {
            if quit {
                cx.quit();
            } else {
                window.remove_window();
            }
        });
        false
    }

    /// File ▸ Close Project: ask about unsaved changes first, then keep the project's tabs
    /// for when it opens again, close them and the project, and show the Welcome page.
    pub fn close_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).repo.is_none() {
            return;
        }
        self.settle_unsaved(window, cx, |this, _, cx| {
            this.stash_session(cx);
            this.send(Action::CloseRepo, cx);
        });
    }

    /// Another project is to open in place of this one: settle the unsaved tabs, keep
    /// this project's tabs for when it opens again, open the other, and bring back its
    /// tabs (or the Welcome page).
    fn switch_project(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        // A file of the other project (Open on a file) opens once the switch is done.
        let file = self.model.update(cx, |m, _| m.pending_file.take());
        self.settle_unsaved(window, cx, move |this, window, cx| {
            this.stash_session(cx);
            if this.model.update(cx, |m, cx| m.switch_project(&path, cx)) {
                this.shown = this.model.read(cx).project_root();
                this.load_session(window, cx);
                if let Some((path, at)) = file {
                    this.open_file(path, at, false, window, cx);
                }
            }
            cx.notify();
        });
    }

    /// Keep the open project's editor tabs (the center of the layout) for when it opens
    /// again, and save the layout.
    fn stash_session(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.model.read(cx).project_root() else { return };
        let Ok(full) = serde_json::to_value(self.dock.read(cx).dump(cx)) else { return };
        let center = full.get("center").cloned().unwrap_or_else(session::empty_tabs);
        self.model.update(cx, |m, _| {
            let recent = m.recent_projects();
            m.settings.sessions.insert(root.clone(), center);
            m.settings.sessions.retain(|p, _| *p == root || recent.contains(p));
            m.settings.dock = Some(full);
            m.persist();
        });
    }

    /// Bring back the open project's editor tabs as it left them, in place of those
    /// showing; a project without saved tabs shows the Welcome page.
    fn load_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.model.read(cx).project_root();
        let center = project
            .as_ref()
            .and_then(|p| self.model.read(cx).settings.sessions.get(p).cloned())
            .unwrap_or_else(session::empty_tabs);
        let Ok(full) = serde_json::to_value(self.dock.read(cx).dump(cx)) else { return };
        let state = session::with_center(full, center);
        let (loaded, editors) = restore_layout(&self.dock, state, &self.panels, project.as_deref(), window, cx);
        if !loaded {
            return;
        }
        self.editors.retain(|e| e.upgrade().is_some());
        self.set_active_editor(None, cx);
        self.adopt_editors(editors, window, cx);
        // The keyboard was in a tab that went: it goes to the tab in front, or the window.
        match self.status.read(cx).editor() {
            Some(editor) => editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.focus(window, cx))),
            None => window.focus(&self.focus, cx),
        }
        self.save_layout_soon(cx);
    }

    /// Editor tabs a layout brought back: the workspace follows them, a library class
    /// fills once the server can show it, and the Welcome page does not stay beside them.
    fn adopt_editors(&mut self, editors: Vec<Entity<EditorPanel>>, window: &mut Window, cx: &mut Context<Self>) {
        if !editors.is_empty() {
            let welcome = self.welcome.clone();
            self.dock.update(cx, |d, cx| d.remove_panel(welcome, window, cx));
        }
        for editor in editors {
            self.track_editor(&editor, window, cx);
            if let Some((uri, root)) = editor.read(cx).class_file.clone() {
                self.load_class_file(editor, root, uri, None, window, cx);
            }
        }
        // The tab in front is the one the status bar follows.
        let fronts = self.front_panels(cx);
        let front = self.editors(cx).into_iter().find(|e| fronts.contains(&PanelId::from(e.entity_id())));
        self.set_active_editor(front.as_ref(), cx);
    }

    /// The project closed: its editor tabs go, and the Welcome page comes forward.
    fn after_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editors = self.editors(cx);
        let welcome = self.tabs.welcome;
        let page = self.welcome.clone();
        self.dock.update(cx, |d, cx| {
            for e in editors {
                d.remove_panel(e, window, cx);
            }
            if d.panel(welcome).is_none() {
                d.add_panel_view(panel_handle(page), DockPlacement::Center, None, window, cx);
            }
            d.select_panel(welcome, window, cx);
        });
        self.editors.clear();
        self.set_active_editor(None, cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// The center is never left empty: with its last tab closed, the Welcome page comes
    /// back in its place.
    fn keep_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dock = self.dock.read(cx);
        let empty = dock.layout(DockPlacement::Center).is_none_or(|t| t.panels().next().is_none());
        if empty {
            let page = panel_handle(self.welcome.clone());
            self.dock.update(cx, |d, cx| d.add_panel_view(page, DockPlacement::Center, None, window, cx));
        }
    }

    /// Keep `editor` among the tabs, and follow it when it gets the keyboard.
    fn track_editor(&mut self, editor: &Entity<EditorPanel>, window: &mut Window, cx: &mut Context<Self>) {
        self.editors.push(editor.downgrade());
        self._subscriptions.push(cx.subscribe_in(editor, window, |this, editor, e: &EditorEvent, window, cx| {
            match e {
                EditorEvent::Focused => this.set_active_editor(Some(editor), cx),
                // The other views of the same file (a split) follow.
                EditorEvent::Changed => {
                    let twins = this.twins_of(editor, cx);
                    if !twins.is_empty() {
                        let text = editor.read(cx).text(cx);
                        for twin in twins {
                            twin.update(cx, |t, cx| t.sync_from(&text, window, cx));
                        }
                    }
                }
                EditorEvent::Saved => {
                    let disk = editor.read(cx).disk_stamp();
                    for twin in this.twins_of(editor, cx) {
                        twin.update(cx, |t, cx| t.mark_saved(disk, cx));
                    }
                }
                EditorEvent::Reloaded(before) => {
                    crate::claude_ide::bridge::followed_reload(this, editor, before, window, cx)
                }
                EditorEvent::Discarded => {
                    let e = editor.clone();
                    this.dock.update(cx, |d, cx| d.remove_panel(e, window, cx));
                }
                EditorEvent::CaretMoved => this.save_layout_soon(cx),
                EditorEvent::Claude(r) => crate::claude_ide::bridge::editor_request(this, editor, r, window, cx),
            }
        }));
    }

    /// The other editor tabs showing the same file as `editor`.
    fn twins_of(&self, editor: &Entity<EditorPanel>, cx: &App) -> Vec<Entity<EditorPanel>> {
        let path = &editor.read(cx).path;
        self.editors(cx)
            .into_iter()
            .filter(|e| e.entity_id() != editor.entity_id() && &e.read(cx).path == path)
            .collect()
    }

    /// Show the editor in front again in a new tab group to its right, as a second view
    /// of the same file: what is typed in one shows in the other.
    pub fn split_right(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::dock::InsertTarget;
        let Some(editor) = self.active_editor(window, cx) else { return };
        let path = editor.read(cx).path.clone();
        let text = editor.read(cx).text(cx);
        let twin = cx.new(|cx| EditorPanel::open(path, window, cx));
        twin.update(cx, |t, cx| t.sync_from(&text, window, cx));
        self.track_editor(&twin, window, cx);
        let (from, to) = (PanelId::from(editor.entity_id()), PanelId::from(twin.entity_id()));
        let view = panel_handle(twin.clone());
        self.dock.update(cx, |d, cx| {
            let node = d.layout(DockPlacement::Center).and_then(|t| t.find_panel_node(from));
            d.add_panel_view(view, DockPlacement::Center, None, window, cx);
            if let Some(node) = node {
                let target = InsertTarget::Split { node, placement: gpui_component::Placement::Right, size: None };
                d.move_panel(to, target, window, cx);
            }
            d.select_panel(to, window, cx);
        });
        let at = editor.read(cx).state.read(cx).cursor_position();
        twin.update(cx, |t, cx| t.go_to(at.line, at.character, window, cx));
        self.set_active_editor(Some(&twin), cx);
    }

    /// The command palette: every command, with the shortcut it has where the keyboard
    /// is now. The chosen one runs there.
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let before = window.focused(cx);
        let os = cx.platform_info().os;
        let recent = self.model.read(cx).recent_projects();
        let commands: Vec<crate::search::Command> = menus::commands(&recent)
            .into_iter()
            .map(|(name, action)| {
                let binding = match &before {
                    Some(f) => window.highest_precedence_binding_for_action_in(&*action, f),
                    None => window.highest_precedence_binding_for_action(&*action),
                }
                .or_else(|| window.highest_precedence_binding_for_action(&*action));
                let keys = binding
                    .map(|b| b.keystrokes().iter().map(|k| k.display(os).to_string()).collect::<Vec<_>>().join(" "));
                crate::search::Command { name, action, keys }
            })
            .collect();
        let model = self.model.clone();
        let view = cx.new(|cx| SearchView::new(model, PathBuf::new(), Mode::Commands, window, cx));
        view.update(cx, |v, cx| v.set_commands(commands, cx));
        cx.subscribe_in(&view, window, move |this, _, e: &SearchEvent, window, cx| match e {
            SearchEvent::Run(action) => {
                this.search = None;
                match &before {
                    Some(f) => window.focus(f, cx),
                    None => window.focus(&this.focus, cx),
                }
                window.dispatch_action(action.boxed_clone(), cx);
                cx.notify();
            }
            _ => {
                this.search = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
        })
        .detach();
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = Some(view);
        cx.notify();
    }

    /// The editor in front changed: the status bar and the file tree follow it.
    pub fn set_active_editor(&mut self, editor: Option<&Entity<EditorPanel>>, cx: &mut Context<Self>) {
        self.status.update(cx, |s, cx| s.set_editor(editor, cx));
        self.structure.update(cx, |s, cx| s.follow(editor, cx));
        let path = editor.map(|e| e.read(cx).path.clone());
        let files = self.side.read(cx).files.clone();
        files.update(cx, |f, cx| f.reveal(path.clone(), cx));
        if let Some(path) = path {
            let project = self.side.read(cx).project.clone();
            project.update(cx, |v, cx| v.reveal(&path, cx));
        }
    }

    /// The editor tab with the keyboard, or else the one the status bar follows.
    pub fn active_editor(&self, window: &Window, cx: &App) -> Option<Entity<EditorPanel>> {
        self.editors(cx)
            .into_iter()
            .find(|e| gpui::Focusable::focus_handle(e.read(cx).state.read(cx), cx).contains_focused(window, cx))
            .or_else(|| self.status.read(cx).editor())
    }

    fn on_model_event(&mut self, model: &Entity<Model>, e: &ModelEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e {
            ModelEvent::Dialog => crate::dialogs::open(model, window, cx),
            ModelEvent::Theme => {
                let s = &model.read(cx).settings;
                let (colors, choice) = (s.colors, s.theme);
                theme::apply(colors, choice, Some(window), cx);
            }
            ModelEvent::ToggleConsole => {
                let open = model.read(cx).settings.console_open;
                self.dock.update(cx, |d, cx| {
                    if d.is_dock_open(DockPlacement::Bottom) != open {
                        d.toggle_dock(DockPlacement::Bottom, window, cx);
                    }
                    if let Some(id) = self.tabs.console.filter(|_| open) {
                        d.select_panel(id, window, cx);
                    }
                });
            }
            // The commit and the diff show in the Git view.
            ModelEvent::ShowCommit | ModelEvent::ShowDiff => self.show_activity(Activity::Git, window, cx),
            ModelEvent::Message => {}
            ModelEvent::Closed => self.after_close(window, cx),
            ModelEvent::Switch(path) => self.switch_project(path.clone(), window, cx),
            ModelEvent::NewProject => self.open_new_project(window, cx),
            ModelEvent::PickRemoteFolder(then) => self.pick_remote(remote_ui::Purpose::Then(then.0), window, cx),
            ModelEvent::OpenFile(path, at, preview) => {
                model.update(cx, |m, _| m.pending_file = None);
                self.open_file(path.clone(), *at, *preview, window, cx)
            }
        }
    }

    /// Show `activity`: the Git view, or the docks with its panel in the left dock (opened
    /// if it is closed).
    pub fn show_activity(&mut self, activity: Activity, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_activity(activity, cx);
        if activity != Activity::Git && !self.dock.read(cx).is_dock_open(DockPlacement::Left) {
            self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
        }
        self.side.update(cx, |s, cx| {
            s.activity = activity;
            cx.notify();
        });
        self.ensure_project_server(cx);
        cx.notify();
    }

    /// Save the activity showing, so the next start opens on it.
    fn remember_activity(&mut self, activity: Activity, cx: &mut Context<Self>) {
        if self.model.read(cx).settings.activity != Some(activity) {
            self.model.update(cx, |m, _| {
                m.settings.activity = Some(activity);
                m.persist();
            });
        }
    }

    /// The activity bar: show `activity`, or close the left dock when it already shows it.
    pub fn set_activity(&mut self, activity: Activity, window: &mut Window, cx: &mut Context<Self>) {
        if activity == Activity::Git {
            return self.show_activity(activity, window, cx);
        }
        let same = self.side.read(cx).activity == activity;
        let open = self.dock.read(cx).is_dock_open(DockPlacement::Left);
        if same && open {
            self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
        } else {
            if !open {
                self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
            }
            self.remember_activity(activity, cx);
            self.side.update(cx, |s, cx| {
                s.activity = activity;
                cx.notify();
            });
        }
        cx.notify();
    }

    /// ↑ ↓ in the commit view; elsewhere, and in a terminal, the keys go on as usual.
    fn step_commit(&mut self, up: bool, window: &mut Window, cx: &mut Context<Self>) {
        if in_terminal(window) || self.model.read(cx).commit_view.is_none() {
            cx.propagate();
            return;
        }
        self.model.update(cx, |m, cx| m.step_commit(up, cx));
    }

    /// Open `path` in an editor tab (or bring its tab forward), the caret at `at`. A
    /// preview takes the place of the preview tab already open, unless that one has
    /// changes. The docks come forward if the Git view was showing.
    pub fn open_file(
        &mut self,
        path: PathBuf,
        at: Option<(u32, u32)>,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_editor(path, at, preview, true, true, window, cx);
    }

    /// Open `path` in a tab (or find its tab) without taking the keyboard from where it
    /// is, as Claude Code's requests do; `front` brings the tab forward.
    pub fn reveal_file(
        &mut self,
        path: PathBuf,
        preview: bool,
        front: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<EditorPanel> {
        self.open_editor(path, None, preview, front, false, window, cx)
    }

    #[allow(clippy::too_many_arguments)]
    fn open_editor(
        &mut self,
        path: PathBuf,
        at: Option<(u32, u32)>,
        preview: bool,
        front: bool,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<EditorPanel> {
        self.editors.retain(|e| e.upgrade().is_some());
        let open = self.editors(cx).into_iter().find(|e| e.read(cx).path == path);
        let reusable = if open.is_none() && preview {
            self.editors(cx).into_iter().find(|e| e.read(cx).preview && !e.read(cx).dirty)
        } else {
            None
        };
        let editor = match open {
            Some(editor) => {
                if !preview {
                    editor.update(cx, |e, cx| e.pin(cx));
                }
                editor
            }
            None if reusable.is_some() => {
                let editor = reusable.expect("checked");
                editor.update(cx, |e, cx| e.load(path, window, cx));
                editor
            }
            None => {
                let editor = cx.new(|cx| EditorPanel::open(path, window, cx));
                editor.update(cx, |e, _| e.preview = preview);
                self.track_editor(&editor, window, cx);
                let view = panel_handle(editor.clone());
                let welcome = self.welcome.clone();
                let welcome_id = self.tabs.welcome;
                self.dock.update(cx, |d, cx| {
                    d.add_panel_view(view, DockPlacement::Center, None, window, cx);
                    // The first file takes the Welcome page's place.
                    if d.panel(welcome_id).is_some() {
                        d.remove_panel(welcome, window, cx);
                    }
                });
                editor
            }
        };
        if !front {
            cx.notify();
            return editor;
        }
        self.set_active_editor(Some(&editor), cx);
        if self.side.read(cx).activity == Activity::Git {
            self.show_activity(Activity::Project, window, cx);
        }
        let id = PanelId::from(editor.entity_id());
        self.dock.update(cx, |d, cx| d.select_panel(id, window, cx));
        if focus {
            editor.update(cx, |e, cx| match at {
                Some((line, column)) => e.go_to(line, column, window, cx),
                None => e.state.update(cx, |s, cx| s.focus(window, cx)),
            });
        }
        cx.notify();
        editor
    }

    /// Put up the go-to-file or find-in-files box over the open repository.
    pub fn open_search(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.active_editor(window, cx);
        let root = match mode {
            Mode::GoToLine => match &target {
                Some(e) => e.read(cx).path.parent().map(PathBuf::from).unwrap_or_default(),
                None => return,
            },
            _ => match self.model.read(cx).project_root() {
                Some(r) => r,
                None => return,
            },
        };
        let model = self.model.clone();
        let view = cx.new(|cx| SearchView::new(model, root, mode, window, cx));
        let target = target.map(|e| e.downgrade());
        cx.subscribe_in(&view, window, move |this, _, e: &SearchEvent, window, cx| match e {
            SearchEvent::Close => {
                this.search = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
            SearchEvent::GoToLine(line, column) => {
                this.search = None;
                if let Some(editor) = target.as_ref().and_then(|t| t.upgrade()) {
                    editor.update(cx, |e, cx| e.go_to(*line, *column, window, cx));
                }
                cx.notify();
            }
            SearchEvent::OpenLocation(root, uri, at) => {
                this.search = None;
                this.open_location(root.clone(), uri.clone(), *at, window, cx);
                cx.notify();
            }
            SearchEvent::Run(_) => {}
        })
        .detach();
        // Give the field the keyboard once this action is done.
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = Some(view);
        cx.notify();
    }

    /// Put up Find in Files, or Replace in Files when `replace`, over the project: the
    /// selection of the editor in front is what it looks for, the folder of its file the
    /// Directory scope's, its module the Module scope's.
    pub fn open_find_in_files(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        use crate::find_in_files::{FindEvent, FindInFiles};
        if let Some(view) = self.find_in_files.clone() {
            view.update(cx, |v, cx| {
                v.set_replace_mode(replace, window, cx);
                if !replace {
                    v.focus_input(window, cx);
                }
            });
            return;
        }
        let Some(root) = self.model.read(cx).project_root() else { return };
        let editor = self.active_editor(window, cx);
        let initial = editor.as_ref().map(|e| {
            let s = e.read(cx).state.read(cx);
            let range = s.selected_range();
            s.text().slice(range).to_string()
        });
        let file = editor.as_ref().map(|e| e.read(cx).path.clone());
        let directory = file
            .as_ref()
            .and_then(|f| f.parent())
            .and_then(|d| d.strip_prefix(&root).ok())
            .map(|d| d.to_string_lossy().into_owned());
        let mut modules: Vec<(String, PathBuf)> = crate::project_model::store(cx)
            .and_then(|s| s.read(cx).current.clone())
            .map(|m| m.modules.iter().map(|m| (m.name.clone(), m.content_root.clone())).collect())
            .unwrap_or_default();
        modules.sort_by(|a, b| a.0.cmp(&b.0));
        // The module of the file in front: the deepest folder holding it.
        let module = file.as_ref().and_then(|f| {
            modules
                .iter()
                .enumerate()
                .filter(|(_, (_, dir))| f.starts_with(dir))
                .max_by_key(|(_, (_, dir))| dir.components().count())
                .map(|(i, _)| i)
        });
        let weak = cx.entity().downgrade();
        let open_root = root.clone();
        let open: crate::find_in_files::OpenTexts = std::rc::Rc::new(move |cx: &App| {
            let Some(ws) = weak.upgrade() else { return Default::default() };
            ws.read(cx)
                .editors(cx)
                .into_iter()
                .filter(|e| !e.read(cx).loading && e.read(cx).class_file.is_none())
                .filter_map(|e| {
                    let e = e.read(cx);
                    let rel = e.path.strip_prefix(&open_root).ok()?.to_string_lossy().into_owned();
                    Some((rel, Arc::new(e.text(cx))))
                })
                .collect()
        });
        let view = cx.new(|cx| FindInFiles::new(root, replace, initial, modules, module, directory, open, window, cx));
        cx.subscribe_in(&view, window, |this, _, e: &FindEvent, window, cx| match e {
            FindEvent::Close => {
                this.find_in_files = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
            FindEvent::Open(path, at) => {
                this.find_in_files = None;
                this.open_file(path.clone(), Some(*at), false, window, cx);
                cx.notify();
            }
            FindEvent::Replace(changes) => crate::find_in_files::apply(this, changes.clone(), false, window, cx),
        })
        .detach();
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = None;
        self.find_in_files = Some(view);
        cx.notify();
    }

    /// Search Everywhere: the files of the project, and the symbols of its Java server
    /// when one runs, in one box.
    pub fn search_everywhere(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_search(Mode::GoToFile, window, cx);
        let source = self.workspace_symbols(cx);
        if let (Some(view), Some(source)) = (self.search.clone(), source) {
            view.update(cx, |v, cx| v.set_symbols(source, cx));
        }
    }

    /// Put up the Settings sheet.
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_sheet.is_some() {
            return;
        }
        let mut builds: Vec<PathBuf> = self.hub.read(cx).servers.keys().cloned().collect();
        let host = crate::host::host(cx);
        if let Some(root) =
            self.model.read(cx).project_root().filter(|r| crate::project_model::project_has_gradle(r, cx))
            && !builds.contains(&root)
        {
            builds.push(root);
        }
        builds.sort();
        // The JDKs of the host are looked for off the UI thread; the sheet comes up then.
        if self.settings_opening {
            return;
        }
        self.settings_opening = true;
        let home = self.hub.read(cx).home.clone();
        let found = cx.background_executor().spawn(async move { crate::lang::java::discover(&*host, &home) });
        cx.spawn_in(window, async move |this, cx| {
            let jdks = found.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings_opening = false;
                this.show_settings(builds, jdks, window, cx);
            });
        })
        .detach();
    }

    fn show_settings(
        &mut self,
        builds: Vec<PathBuf>,
        jdks: Vec<crate::lang::java::Jdk>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::settings_view::{SettingsEvent, SettingsView};
        if self.settings_sheet.is_some() {
            return;
        }
        let model = self.model.clone();
        let sheet = cx.new(|cx| SettingsView::new(model, builds, jdks, window, cx));
        cx.subscribe_in(&sheet, window, |this, _, e: &SettingsEvent, window, cx| {
            let close = match e {
                SettingsEvent::Apply(next, close) => {
                    this.apply_settings((**next).clone(), window, cx);
                    *close
                }
                SettingsEvent::Cancel => true,
            };
            if close {
                this.settings_sheet = None;
                window.focus(&this.focus, cx);
            }
            cx.notify();
        })
        .detach();
        let focus = gpui::Focusable::focus_handle(sheet.read(cx), cx);
        window.focus(&focus, cx);
        self.settings_sheet = Some(sheet);
        cx.notify();
    }

    /// Put up the New Project dialog. The project it makes opens.
    pub fn open_new_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::new_project::{NewProjectEvent, NewProjectView};
        if self.new_project.is_some() {
            return;
        }
        // Beside the project open last, else in the home folder.
        let m = self.model.read(cx);
        let location = m
            .recent_projects()
            .first()
            .and_then(|p| p.parent().map(PathBuf::from))
            .unwrap_or_else(|| crate::host::host(cx).info().home.clone());
        let cache = m.support_dir().map(|d| d.join("start-spring-io.json"));
        let view = cx.new(|cx| NewProjectView::new(location, cache, window, cx));
        cx.subscribe_in(&view, window, |this, _, e: &NewProjectEvent, window, cx| {
            this.new_project = None;
            window.focus(&this.focus, cx);
            if let NewProjectEvent::Created(dir) = e {
                this.send(Action::OpenPath(dir.clone()), cx);
            }
            cx.notify();
        })
        .detach();
        let name = view.read(cx).name.clone();
        cx.defer_in(window, move |_, window, cx| name.update(cx, |i, cx| i.focus(window, cx)));
        self.new_project = Some(view);
        cx.notify();
    }

    /// Put `next` in place, save it, and make the window follow it.
    pub fn apply_settings(&mut self, next: crate::settings::Settings, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.model.read(cx).settings.clone();
        self.model.update(cx, |m, cx| {
            m.settings = next.clone();
            if m.fetch_every.is_some() {
                m.fetch_every = Some(std::time::Duration::from_secs(next.fetch_minutes.max(1) * 60));
            }
            m.persist();
            cx.notify();
        });
        theme::set_fonts(&next, cx);
        theme::apply(next.colors, next.theme, Some(window), cx);
        cx.set_global(crate::editor::EditorFont(
            next.editor_font_size.clamp(crate::editor::FONT_MIN, crate::editor::FONT_MAX),
        ));
        let defaults = crate::editor::EditorDefaults::of(&next);
        cx.set_global(defaults);
        if before.keymap != next.keymap {
            crate::keymap::apply(&next.keymap, cx);
            // The menus show the shortcuts: they are made again.
            self.menu_checks = None;
        }
        let wrap_changed = (before.soft_wrap, before.show_whitespace) != (next.soft_wrap, next.show_whitespace);
        let hints_changed = before.param_hints != next.param_hints;
        for editor in self.editors(cx) {
            editor.update(cx, |e, cx| {
                if wrap_changed {
                    e.apply_defaults(defaults, window, cx);
                }
                if hints_changed {
                    e.refresh_param_hints(cx);
                }
                cx.notify();
            });
        }
        window.refresh();
    }

    /// The tab in front of each of the center's tab groups.
    pub fn front_panels(&self, cx: &App) -> Vec<PanelId> {
        use gpui_component::dock::PaneRef;
        let mut out = Vec::new();
        if let Some(tree) = self.dock.read(cx).layout(DockPlacement::Center) {
            tree.root().walk(&mut |node| {
                if let PaneRef::Tabs { panels, active_ix } = node.kind()
                    && let Some(id) = panels.get(active_ix).or(panels.last())
                {
                    out.push(*id);
                }
            });
        }
        out
    }

    /// The editor tabs open now.
    pub fn editors(&self, cx: &App) -> Vec<Entity<EditorPanel>> {
        let dock = self.dock.read(cx);
        self.editors
            .iter()
            .filter_map(|e| e.upgrade())
            .filter(|e| dock.panel(PanelId::from(e.entity_id())).is_some())
            .collect()
    }

    /// Save the dock layout once it has stopped changing for a moment (a drag resizes it
    /// many times).
    fn save_layout_soon(&mut self, cx: &mut Context<Self>) {
        self._save_layout = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(400)).await;
            let _ = this.update(cx, |this, cx| this.save_layout(cx));
        }));
    }

    pub fn save_layout(&mut self, cx: &mut Context<Self>) {
        let state = self.dock.read(cx).dump(cx);
        let Ok(value) = serde_json::to_value(state) else { return };
        self.model.update(cx, |m, _| {
            m.settings.dock = Some(value);
            m.persist();
        });
    }

    fn send(&mut self, action: Action, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.handle(action, cx));
    }

    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        // The window's title names the remote machine the project is on.
        let title = match crate::remote::connection(cx) {
            Some((host, _)) => m.title().replacen(" · SnoutCode", &format!(" [{host}] · SnoutCode"), 1),
            None => m.title(),
        };
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
        let recent = m.recent_projects();
        let remote = crate::remote::recents(cx).projects;
        let checks = menus::checks(&m.settings, &recent, &remote);
        if self.menu_checks.as_ref() != Some(&checks) {
            cx.set_menus(menus::bar(&m.settings, &recent, &remote));
            self.menu_checks = Some(checks);
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_window(window, cx);
        // The first frame is drawn: the heavy work may start.
        if self.hub.read(cx).paused {
            cx.defer_in(window, |this, _, cx| {
                this.hub.update(cx, |h, cx| h.resume(cx));
                this.ensure_project_server(cx);
            });
        }
        let p = palette(cx);
        let m = self.model.read(cx);
        let free = m.dialog.is_none();
        let has_repo = m.repo.is_some();
        let idle = m.job.is_none();
        let entries = m.selected_entries();
        let in_tree = m.commit_view.is_none();
        let can_commit = m.can_commit();
        let can_draft = m.can_draft();
        let has_github = m.github_url.is_some();
        let stage = in_tree && entries.iter().any(FileEntry::has_unstaged);
        let unstage = in_tree && entries.iter().any(FileEntry::has_staged);
        let discard = in_tree && !entries.is_empty();
        let trash = in_tree && entries.iter().any(on_disk);

        macro_rules! act {
            ($el:expr, $ty:ty, $on:expr, $action:expr) => {
                $el.when($on, |d| {
                    d.on_action(cx.listener(|this, _: &$ty, _, cx| {
                        let entries = this.model.read(cx).selected_entries();
                        let _ = &entries;
                        let action: Action = $action(entries);
                        this.send(action, cx);
                    }))
                })
            };
        }
        let root = div()
            .id("workspace")
            .key_context("Workspace")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_color(p.fg)
            .font_family(Theme::global(cx).font_family.clone())
            .text_size(px(crate::look::ui_size(cx)))
            .on_action(cx.listener(|this, _: &ShowProject, w, cx| this.show_activity(Activity::Project, w, cx)))
            .on_action(cx.listener(|this, _: &ShowGit, w, cx| this.show_activity(Activity::Git, w, cx)))
            .on_action(cx.listener(|this, _: &ToggleConsole, _, cx| this.send(Action::ToggleConsole, cx)))
            .on_action(cx.listener(|this, _: &About, _, cx| {
                this.model.update(cx, |m, cx| {
                    m.dialog = Some(Dialog::About);
                    cx.emit(ModelEvent::Dialog);
                })
            }))
            .on_action(cx.listener(|this, _: &SideBySide, _, cx| {
                this.model.update(cx, |m, cx| m.set_diff_options(Some(true), None, cx))
            }))
            .on_action(cx.listener(|this, _: &Unified, _, cx| {
                this.model.update(cx, |m, cx| m.set_diff_options(Some(false), None, cx))
            }))
            .on_action(cx.listener(|this, _: &Compact, _, cx| {
                let c = this.model.read(cx).settings.compact;
                this.model.update(cx, |m, cx| m.set_diff_options(None, Some(!c), cx))
            }))
            .on_action(cx.listener(|this, _: &ThemeSystem, _, cx| this.send(Action::SetTheme(ThemeChoice::System), cx)))
            .on_action(cx.listener(|this, _: &ThemeDark, _, cx| this.send(Action::SetTheme(ThemeChoice::Dark), cx)))
            .on_action(cx.listener(|this, _: &ThemeLight, _, cx| this.send(Action::SetTheme(ThemeChoice::Light), cx)))
            .on_action(cx.listener(|_, _: &Minimize, w, _| w.minimize_window()))
            .on_action(cx.listener(|_, _: &Zoom, w, _| w.zoom_window()))
            // ⌘A outside a text field selects every listed file.
            .on_action(cx.listener(|this, _: &gpui_component::input::SelectAll, _, cx| {
                this.model.update(cx, |m, cx| m.select_all_files(cx))
            }))
            .on_action(cx.listener(|this, _: &Back, w, cx| {
                if in_terminal(w) || this.model.read(cx).commit_view.is_none() {
                    cx.propagate();
                } else {
                    this.send(Action::CloseCommit, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &GoToFile, w, cx| this.open_search(Mode::GoToFile, w, cx)))
            .on_action(cx.listener(|this, _: &GoToClass, w, cx| this.open_classes(w, cx)))
            .on_action(cx.listener(|this, _: &SearchEverywhere, w, cx| this.search_everywhere(w, cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, w, cx| this.open_settings(w, cx)))
            .on_action(cx.listener(|this, _: &crate::claude_ide::AcceptAllClaude, w, cx| {
                crate::claude_ide::bridge::accept_all(this, w, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::claude_ide::RevertAllClaude, w, cx| {
                crate::claude_ide::bridge::revert_all(this, w, cx)
            }))
            .on_action(cx.listener(|this, _: &NewProject, w, cx| this.open_new_project(w, cx)))
            .on_action(cx.listener(|this, _: &ConnectSsh, w, cx| this.open_connect(w, cx)))
            .on_action(cx.listener(|this, a: &OpenRemoteRecent, w, cx| this.open_remote_recent(a.0.clone(), w, cx)))
            // Greyed out without a connection.
            .when(crate::remote::is_remote(cx), |d| {
                d.on_action(cx.listener(|this, _: &CloseRemote, w, cx| this.close_remote(w, cx)))
            })
            .on_action(cx.listener(|this, a: &HideToolWindow, w, cx| {
                let place = if a.0 { DockPlacement::Right } else { DockPlacement::Bottom };
                this.hide_dock(place, w, cx)
            }))
            .on_action(cx.listener(|this, _: &HideSide, w, cx| this.hide_dock(DockPlacement::Left, w, cx)))
            .on_action(cx.listener(|this, _: &LocateFile, _, cx| {
                let path = this.status.read(cx).editor().map(|e| e.read(cx).path.clone());
                let files = this.side.read(cx).files.clone();
                files.update(cx, |f, cx| f.reveal(path.clone(), cx));
                if let Some(path) = path {
                    let project = this.side.read(cx).project.clone();
                    project.update(cx, |v, cx| v.reveal(&path, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &CollapseAll, _, cx| {
                let (files, project) = (this.side.read(cx).files.clone(), this.side.read(cx).project.clone());
                files.update(cx, |f, cx| {
                    f.expanded.clear();
                    cx.notify();
                });
                project.update(cx, |v, cx| v.collapse_all(cx));
            }))
            .on_action(cx.listener(|this, _: &CollapseGradle, _, cx| {
                this.gradle.update(cx, |g, cx| g.collapse_all(cx));
            }))
            .on_action(cx.listener(|this, _: &CollapseSpring, _, cx| {
                this.spring_window.update(cx, |w, cx| w.collapse_all(cx));
            }))
            .on_action(cx.listener(|this, _: &RefreshSpring, _, cx| {
                this.spring.update(cx, |s, cx| s.reload(cx));
            }))
            .on_action(cx.listener(|this, _: &ShowSpring, w, cx| {
                if !this.tool_open(Tool::Spring, cx) {
                    this.toggle_tool(Tool::Spring, w, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ShowProcesses, w, cx| {
                if !this.tool_open(Tool::Processes, cx) {
                    this.toggle_tool(Tool::Processes, w, cx);
                }
                this.processes.update(cx, |p, cx| p.look(cx));
            }))
            .on_action(cx.listener(|this, _: &StopIdleProcesses, _, cx| {
                this.processes.update(cx, |p, cx| {
                    p.stop_idle(cx);
                });
            }))
            .on_action(cx.listener(|this, _: &RestartJava, _, cx| this.restart_java(cx)))
            .on_action(cx.listener(|this, _: &EditConfigurations, w, cx| this.open_config_dialog(w, cx)))
            .on_action(cx.listener(|this, _: &RunSelected, w, cx| this.run_selected(false, w, cx)))
            .on_action(cx.listener(|this, _: &DebugSelected, w, cx| this.run_selected(true, w, cx)))
            .on_action(cx.listener(|this, _: &RunContext, w, cx| this.run_context(false, w, cx)))
            .on_action(cx.listener(|this, _: &DebugContext, w, cx| this.run_context(true, w, cx)))
            .on_action(cx.listener(|this, _: &StopRuns, _, cx| this.stop_runs(cx)))
            .on_action(cx.listener(|this, _: &StepOver, _, cx| this.debug_step("next", cx)))
            .on_action(cx.listener(|this, _: &StepInto, _, cx| this.debug_step("stepIn", cx)))
            .on_action(cx.listener(|this, _: &StepOut, _, cx| this.debug_step("stepOut", cx)))
            .on_action(cx.listener(|this, _: &Resume, _, cx| this.debug_step("continue", cx)))
            .on_action(cx.listener(|_, _: &SyncGradle, _, cx| {
                if let Some(store) = crate::project_model::store(cx) {
                    store.update(cx, |s, cx| s.sync(cx));
                }
            }))
            .on_action(cx.listener(|this, _: &RefreshGradle, _, cx| {
                this.runs.update(cx, |r, cx| r.load_gradle_tasks(true, cx));
            }))
            .on_action(cx.listener(|this, _: &ClearOutput, _, cx| {
                this.model.update(cx, |m, cx| {
                    m.output.clear();
                    cx.notify();
                })
            }))
            .on_action(cx.listener(|this, _: &FindInFiles, w, cx| this.open_find_in_files(false, w, cx)))
            .on_action(cx.listener(|this, _: &ReplaceInFiles, w, cx| this.open_find_in_files(true, w, cx)))
            .on_action(cx.listener(|this, _: &UndoReplaceInFiles, w, cx| crate::find_in_files::undo(this, w, cx)))
            .on_action(cx.listener(|this, _: &GoToLine, w, cx| this.open_search(Mode::GoToLine, w, cx)))
            .on_action(cx.listener(|this, _: &CommandPalette, w, cx| this.open_palette(w, cx)))
            .on_action(cx.listener(|this, _: &SplitRight, w, cx| this.split_right(w, cx)))
            .on_action(cx.listener(|this, _: &CloseProject, w, cx| this.close_project(w, cx)))
            .on_action(cx.listener(|this, _: &GoToSymbol, w, cx| this.open_symbols(false, w, cx)))
            .on_action(cx.listener(|this, _: &GoToSymbolInFile, w, cx| this.open_symbols(true, w, cx)))
            .on_action(cx.listener(|this, _: &ShowProblems, w, cx| this.show_problems(w, cx)))
            .on_action(cx.listener(|this, _: &ShowStructure, w, cx| this.toggle_tool(Tool::Structure, w, cx)))
            .on_action(cx.listener(|this, _: &FileStructure, w, cx| this.open_file_structure(w, cx)))
            .on_action(cx.listener(|this, _: &ShowHierarchy, w, cx| this.toggle_tool(Tool::Hierarchy, w, cx)))
            .on_action(cx.listener(|this, _: &TypeHierarchy, w, cx| {
                this.show_hierarchy(crate::lang::hierarchy::Mode::Subtypes, w, cx)
            }))
            .on_action(cx.listener(|this, _: &CallHierarchy, w, cx| {
                this.show_hierarchy(crate::lang::hierarchy::Mode::Callers, w, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleParamHints, w, cx| {
                let mut next = this.model.read(cx).settings.clone();
                next.param_hints = !next.param_hints;
                this.apply_settings(next, w, cx);
            }))
            .on_action(cx.listener(|this, _: &ClearRecent, _, cx| this.send(Action::ClearRecent, cx)))
            .on_action(cx.listener(|this, a: &OpenRecent, _, cx| this.send(Action::OpenRepo(a.0.clone()), cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom(Some(1.), cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom(Some(-1.), cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| this.zoom(None, cx)))
            .on_action(cx.listener(|this, _: &CloseTab, w, cx| this.close_tab(w, cx)))
            .on_action(cx.listener(|this, _: &crate::editor::SendToClaude, w, cx| {
                crate::claude_ide::bridge::send_selection(this, w, cx)
            }))
            .on_action(cx.listener(|this, _: &Quit, w, cx| {
                if this.confirm_unsaved(true, w, cx) {
                    cx.quit();
                }
            }))
            .on_action(cx.listener(|this, _: &PrevCommit, w, cx| this.step_commit(true, w, cx)))
            .on_action(cx.listener(|this, _: &NextCommit, w, cx| this.step_commit(false, w, cx)));
        let root = act!(root, Open, free, |_| Action::PickFolder);
        let root = act!(root, FindRepos, free, |_| Action::PickScanFolder);
        let root = act!(root, CloneRepo, free, |_| Action::ShowClone);
        let root = act!(root, NewGroup, free, |_| Action::ShowNewGroup(None));
        let root = act!(root, Refresh, free && has_repo, |_| Action::Refresh);
        let root = act!(root, Checkout, free && has_repo, |_| Action::ShowCheckout);
        let root = act!(root, NewBranch, free && has_repo, |_| Action::ShowNewBranch(None));
        let root = act!(root, Pull, free && has_repo && idle, |_| Action::Pull(PullMode::Default));
        let root = act!(root, Fetch, free && has_repo && idle, |_| Action::Fetch);
        let root = act!(root, Push, free && has_repo && idle, |_| Action::Push);
        let root = act!(root, Commit, free && can_commit, |_| Action::Commit);
        let root = act!(root, Draft, free && can_draft, |_| Action::DraftMessage);
        let root = act!(root, Stage, free && stage, |e: Vec<FileEntry>| Action::Stage(stage_paths(&e)));
        let root = act!(root, Unstage, free && unstage, |e: Vec<FileEntry>| Action::Unstage(unstage_paths(&e)));
        let root = act!(root, Discard, free && discard, Action::AskDiscard);
        let root = act!(root, Trash, free && trash, Action::AskDeleteFiles);
        let root = act!(root, Github, has_github, |_| Action::OpenGithub);
        let root = root.when(free && has_repo, |d| {
            d.on_action(cx.listener(|this, _: &Reveal, _, cx| {
                if let Some(root) = this.model.read(cx).project_root() {
                    this.send(Action::Reveal(root), cx);
                }
            }))
        });

        // Folders dropped from Finder open; while they hover, the window says so.
        let root = root.relative().on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
            for path in paths.paths() {
                this.send(Action::OpenPath(path.clone()), cx);
            }
        }));
        let (veil, on_veil) = (p.panel.opacity(0.9), p.fg);
        let drop_hint = div()
            .id("drop-hint")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(24.))
            .text_color(gpui::transparent_black())
            .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(veil).text_color(on_veil))
            .child("Drop a folder to open it");

        let git = self.side.read(cx).activity == Activity::Git;
        if self.status.read(cx).git != git {
            self.status.update(cx, |s, cx| {
                s.git = git;
                cx.notify();
            });
        }
        let main = if git {
            self.git.clone().into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .children(self.jdk_bar(cx))
                .children(self.gradle_bar(cx))
                .child(self.dock_area(cx))
                .into_any_element()
        };
        let title = self.title_bar(cx);
        root.child(title)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.left_stripe(cx))
                    .child(div().flex_1().min_w_0().h_full().child(main))
                    .child(self.right_stripe(cx)),
            )
            .child(self.status.clone())
            .child(self.claude_changes.read(cx).indicator.clone())
            .child(drop_hint)
            .children(self.settings_sheet.clone().map(|sheet| {
                div()
                    .id("settings-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.scrim)
                    .occlude()
                    .child(sheet)
            }))
            .children(self.connect_dialog.clone().map(|dialog| {
                div()
                    .id("connect-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.scrim)
                    .occlude()
                    .child(dialog)
            }))
            .children(self.remote_picker.clone().map(|picker| {
                div()
                    .id("remote-picker-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.scrim)
                    .occlude()
                    .child(picker)
            }))
            .children(self.new_project.clone().map(|dialog| {
                div()
                    .id("new-project-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.scrim)
                    .occlude()
                    .child(dialog)
            }))
            .children(self.config_dialog.clone().map(|dialog| {
                div()
                    .id("config-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.scrim)
                    .occlude()
                    .child(dialog)
            }))
            .children(self.find_in_files.clone().map(|view| {
                div().absolute().top(px(TITLE_H + 24.)).left_0().right_0().flex().justify_center().child(view)
            }))
            .children(self.search.clone().map(|view| {
                div().absolute().top(px(TITLE_H + 24.)).left_0().right_0().flex().justify_center().child(view)
            }))
    }
}

/// What GPUI Kit's dock keeps of a closed bottom dock (a strip for its tab bar). The
/// stripes reopen it here, so the strip is cut off below the window's view.
const CLOSED_BOTTOM_STRIP: f32 = 29.;

impl Workspace {
    fn dock_area(&self, cx: &App) -> gpui::AnyElement {
        let closed = !self.dock.read(cx).is_dock_open(DockPlacement::Bottom);
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom(px(if closed { -CLOSED_BOTTOM_STRIP } else { 0. }))
                    .child(self.dock.clone()),
            )
            .into_any_element()
    }
}

/// Whether a terminal has the keyboard: then Esc and the arrows are its own.
fn in_terminal(window: &Window) -> bool {
    window.context_stack().iter().any(|c| c.contains("Terminal"))
}

/// Actions every window answers, whatever has focus.
pub fn app_actions(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Source, cx| cx.open_url(SOURCE_URL));
    cx.on_action(|_: &Issues, cx| cx.open_url(ISSUES_URL));
    cx.on_action(|_: &Website, cx| cx.open_url(WEBSITE_URL));
}
