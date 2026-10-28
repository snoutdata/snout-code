//! The window's content: a toolbar, the activity bar beside GPUI Kit's dock area (the left
//! dock holding the panel the activity bar picked, tabs in the center, the Claude Code
//! console and Output in the bottom dock), and a status bar.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, ExternalPaths, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use gpui::{Global, Task};
use gpui_component::Theme;
use gpui_component::dock::{
    BasePanelView, DockArea, DockAreaState, DockEvent, DockLayout, DockPlacement, DockToggleEvent, PanelId,
    panel_handle, register_panel,
};
use snout_core::{FileEntry, PullMode, on_disk, stage_paths, unstage_paths};

use crate::art::glyph;
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
use snout_core::art::Glyph;

/// The center and bottom tabs, so events can bring one forward.
pub struct Tabs {
    pub welcome: PanelId,
    pub console: Option<PanelId>,
    pub terminal: PanelId,
}

pub struct Workspace {
    pub model: Entity<Model>,
    pub dock: Entity<DockArea>,
    pub side: Entity<SidePanel>,
    /// The Git view: the whole area right of the activity bar while Git is picked.
    pub git: Entity<GitView>,
    pub tabs: Tabs,
    menu_checks: Option<(bool, bool, bool, ThemeChoice)>,
    /// Holds the keyboard when no field or panel does, so shortcuts reach the workspace.
    focus: gpui::FocusHandle,
    title: String,
    /// The go-to-file or find-in-files box, while it shows.
    pub search: Option<Entity<SearchView>>,
    /// The editor tabs, to find the one a file already has.
    editors: Vec<gpui::WeakEntity<EditorPanel>>,
    /// The status bar under the docks; it follows the editor in front.
    pub status: Entity<StatusBar>,
    _save_layout: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// The activity bar's width.
pub const ACTIVITY_W: f32 = 44.;

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
    });
}

/// Put back the layout saved last time, with the panels this window already made; a
/// panel the saved layout lacks (one added since) goes to its usual place. False when
/// the saved layout cannot be used. Editor tabs open their files again; they come back too.
fn restore_layout(
    dock: &Entity<DockArea>,
    state: serde_json::Value,
    panels: &[(Arc<dyn BasePanelView>, DockPlacement)],
    window: &mut Window,
    cx: &mut App,
) -> (bool, Vec<Entity<EditorPanel>>) {
    let Ok(state) = serde_json::from_value::<DockAreaState>(state) else { return (false, Vec::new()) };
    if state.version != Some(LAYOUT_VERSION) {
        return (false, Vec::new());
    }
    let pool: HashMap<&'static str, Arc<dyn BasePanelView>> =
        panels.iter().map(|(view, _)| (view.panel_name(cx), view.clone())).collect();
    for name in pool.keys().copied() {
        register_panel(cx, name, move |_, _, cx| cx.global::<PanelPool>().0[name].clone());
    }
    register_panel(cx, crate::editor::PANEL_NAME, |ctx, window, cx| {
        let path = crate::editor::saved_path(ctx.state()).unwrap_or_default();
        let editor = cx.new(|cx| EditorPanel::open(path, window, cx));
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
        for (view, place) in panels {
            if d.panel(view.panel_id(cx)).is_none() {
                d.add_panel_view(view.clone(), *place, None, window, cx);
            }
        }
    });
    (true, editors)
}

impl Workspace {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let projects = cx.new(|cx| ProjectsView::new(model.clone(), window, cx));
        let files = cx.new(|cx| crate::files::FileTree::new(model.clone(), cx));
        let side = cx.new(|cx| SidePanel::new(projects.clone().into(), files, cx));
        let welcome = cx.new(|cx| WelcomePanel::new(model.clone(), cx));
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
        let mut tabs =
            Tabs { welcome: PanelId::from(welcome.entity_id()), console: None, terminal: terminal.panel_id(cx) };
        if let Some(console) = console {
            tabs.console = Some(console.panel_id(cx));
            panels.push((console, DockPlacement::Bottom));
        }
        panels.push((terminal, DockPlacement::Bottom));
        panels.push((panel_handle(output.clone()), DockPlacement::Bottom));

        let saved = model.read(cx).settings.dock.clone();
        let restored = match saved {
            Some(state) => restore_layout(&dock, state, &panels, window, cx),
            None => (false, Vec::new()),
        };
        if !restored.0 {
            default_layout(&dock, &panels, window, cx);
        }
        let open = model.read(cx).settings.console_open;
        dock.update(cx, |d, cx| {
            d.set_dock_collapsible(DockPlacement::Bottom, true, window, cx);
            if d.is_dock_open(DockPlacement::Bottom) != open {
                d.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });

        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.subscribe(&dock, |this, _, e: &DockEvent, cx| {
                if matches!(e, DockEvent::LayoutChanged) {
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
            // Coming back to the window, re-read the repository and count again, so
            // changes made elsewhere (terminal, editor, an agent) show up at once.
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() && this.model.read(cx).job.is_none() {
                    this.model.update(cx, |m, cx| {
                        m.refresh();
                        m.survey(true, cx);
                        cx.notify();
                    });
                }
            }),
        ];
        let status = cx.new(|cx| StatusBar::new(model.clone(), cx));
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
            editors: Vec::new(),
            status,
            _save_layout: None,
            _subscriptions: subscriptions,
        };
        for editor in restored.1 {
            this.track_editor(&editor, cx);
        }
        let size = this.model.read(cx).settings.editor_font_size;
        cx.set_global(crate::editor::EditorFont(size.clamp(crate::editor::FONT_MIN, crate::editor::FONT_MAX)));
        // Closing the window with unsaved changes asks first.
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |this, cx| this.confirm_unsaved(false, window, cx)).unwrap_or(true)
        });
        window.focus(&this.focus, cx);
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

    /// Before the window closes or the app quits: true when no tab has unsaved changes.
    /// Otherwise ask whether to save them; the answer saves (or drops) them and closes
    /// the window (or quits) itself.
    pub fn confirm_unsaved(&mut self, quit: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let dirty: Vec<Entity<EditorPanel>> = self.editors(cx).into_iter().filter(|e| e.read(cx).dirty).collect();
        if dirty.is_empty() {
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
        window
            .spawn(cx, async move |cx| {
                let Ok(answer) = answer.await else { return };
                let _ = cx.update(|window, cx| {
                    match answer {
                        0 => {
                            for e in &dirty {
                                e.update(cx, |e, cx| e.save(cx));
                            }
                            if dirty.iter().any(|e| e.read(cx).dirty) {
                                return;
                            }
                        }
                        1 => {
                            for e in &dirty {
                                e.update(cx, |e, cx| e.discard(cx));
                            }
                        }
                        _ => return,
                    }
                    if quit {
                        cx.quit();
                    } else {
                        window.remove_window();
                    }
                });
            })
            .detach();
        false
    }

    /// Keep `editor` among the tabs, and follow it when it gets the keyboard.
    fn track_editor(&mut self, editor: &Entity<EditorPanel>, cx: &mut Context<Self>) {
        self.editors.push(editor.downgrade());
        self._subscriptions.push(cx.subscribe(editor, |this, editor, e: &EditorEvent, cx| match e {
            EditorEvent::Focused => this.set_active_editor(Some(&editor), cx),
        }));
    }

    /// The editor in front changed: the status bar and the file tree follow it.
    pub fn set_active_editor(&mut self, editor: Option<&Entity<EditorPanel>>, cx: &mut Context<Self>) {
        self.status.update(cx, |s, cx| s.set_editor(editor, cx));
        let path = editor.map(|e| e.read(cx).path.clone());
        let files = self.side.read(cx).files.clone();
        files.update(cx, |f, cx| f.reveal(path, cx));
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
                let choice = model.read(cx).settings.theme;
                theme::apply(choice, Some(window), cx);
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
            ModelEvent::OpenFile(path, at, preview) => self.open_file(path.clone(), *at, *preview, window, cx),
        }
    }

    /// Show `activity`: the Git view, or the docks with its panel in the left dock (opened
    /// if it is closed).
    pub fn show_activity(&mut self, activity: Activity, window: &mut Window, cx: &mut Context<Self>) {
        if activity != Activity::Git && !self.dock.read(cx).is_dock_open(DockPlacement::Left) {
            self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
        }
        self.side.update(cx, |s, cx| {
            s.activity = activity;
            cx.notify();
        });
        cx.notify();
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
                self.track_editor(&editor, cx);
                let view = panel_handle(editor.clone());
                self.dock.update(cx, |d, cx| d.add_panel_view(view, DockPlacement::Center, None, window, cx));
                editor
            }
        };
        self.set_active_editor(Some(&editor), cx);
        if self.side.read(cx).activity == Activity::Git {
            self.show_activity(Activity::Projects, window, cx);
        }
        let id = PanelId::from(editor.entity_id());
        self.dock.update(cx, |d, cx| d.select_panel(id, window, cx));
        editor.update(cx, |e, cx| match at {
            Some((line, column)) => e.go_to(line, column, window, cx),
            None => e.state.update(cx, |s, cx| s.focus(window, cx)),
        });
        cx.notify();
    }

    /// Put up the go-to-file or find-in-files box over the open repository.
    pub fn open_search(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.active_editor(window, cx);
        let root = match mode {
            Mode::GoToLine => match &target {
                Some(e) => e.read(cx).path.parent().map(PathBuf::from).unwrap_or_default(),
                None => return,
            },
            _ => match self.model.read(cx).repo.as_ref() {
                Some(r) => r.root().to_owned(),
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
        })
        .detach();
        // Give the field the keyboard once this action is done.
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = Some(view);
        cx.notify();
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
        let title = m.title();
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
        let checks = menus::checks(&m.settings);
        if self.menu_checks != Some(checks) {
            cx.set_menus(menus::bar(&m.settings));
            self.menu_checks = Some(checks);
        }
    }

    fn activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let current = self.side.read(cx).activity;
        let open = current == Activity::Git || self.dock.read(cx).is_dock_open(DockPlacement::Left);
        let changed = self.model.read(cx).status.files.len();
        let hover_bg = p.button_hover;
        let item = |id: &'static str, g: Glyph, tip: &'static str, on: bool| {
            div()
                .id(id)
                .flex_none()
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .cursor_pointer()
                .when(on, |d| d.bg(p.selection()))
                .when(!on, |d| d.hover(move |s| s.bg(hover_bg)))
                .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip).build(w, cx))
                .child(glyph(g, 20., p.fg))
        };
        let pick = |a: Activity| cx.listener(move |this, _, w, cx| this.set_activity(a, w, cx));
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .py(px(8.))
            .w(px(ACTIVITY_W))
            .h_full()
            .bg(p.ground)
            .border_r_1()
            .border_color(p.border)
            .child(
                item("act-git", Glyph::Branch, "Git (⌘0)", open && current == Activity::Git)
                    .relative()
                    .on_click(pick(Activity::Git))
                    .when(changed > 0, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(-2.))
                                .right(px(-4.))
                                .px(px(4.))
                                .rounded(px(7.))
                                .bg(p.accent)
                                .text_color(p.on_accent)
                                .text_size(px(10.))
                                .child(changed.to_string()),
                        )
                    }),
            )
            .child(
                item("act-projects", Glyph::Repo, "Projects (⌘1)", open && current == Activity::Projects)
                    .on_click(pick(Activity::Projects)),
            )
            .child(div().flex_1())
            .child(
                item("act-console", Glyph::Spark, "Claude Code console (⌘J)", false)
                    .on_click(cx.listener(|this, _, _, cx| this.send(Action::ToggleConsole, cx))),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_window(window, cx);
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
            .on_action(cx.listener(|this, _: &ShowProjects, w, cx| this.show_activity(Activity::Projects, w, cx)))
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
            .on_action(cx.listener(|this, _: &FindInFiles, w, cx| this.open_search(Mode::FindInFiles, w, cx)))
            .on_action(cx.listener(|this, _: &GoToLine, w, cx| this.open_search(Mode::GoToLine, w, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom(Some(1.), cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom(Some(-1.), cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| this.zoom(None, cx)))
            .on_action(cx.listener(|this, _: &CloseTab, w, cx| this.close_tab(w, cx)))
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
                if let Some(root) = this.model.read(cx).repo.as_ref().map(|r| r.root().to_owned()) {
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
        let drop_hint = div()
            .id("drop-hint")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(24.))
            .text_color(gpui::transparent_black())
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(gpui::black().opacity(0.6)).text_color(gpui::white()))
            .child("Drop a folder to open it");

        let git = self.side.read(cx).activity == Activity::Git;
        let main = if git {
            self.git.clone().into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().flex_1().min_h_0().child(self.dock.clone()))
                .child(self.status.clone())
                .into_any_element()
        };
        root.flex_row()
            .child(self.activity_bar(cx))
            .child(div().flex_1().min_w_0().h_full().child(main))
            .child(drop_hint)
            .children(
                self.search
                    .clone()
                    .map(|view| div().absolute().top(px(60.)).left_0().right_0().flex().justify_center().child(view)),
            )
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
