//! The tool window stripes: thin icon strips down both edges of the window. Left, at the
//! top: the Project and Git activities, then the tool windows moved to the left dock; at
//! the bottom: those of the bottom dock (Run, Debug, Build, Output, Terminal, Problems and
//! the agent console to begin with). Right: those of the right dock (Structure, Hierarchy,
//! Gradle unless the project is Maven alone, Maven when it has a Maven build, Spring,
//! Notifications and Agent Changes to begin with). Each tool window can move to another
//! dock (Move to, in its header's menu and its stripe button's); the dock layout, saved
//! with the session, keeps it there. The item showing is a filled accent square with a
//! white glyph.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use gpui_base::dock::PaneRef;
use gpui_component::dock::{DockArea, DockPlacement, PanelId};
use gpui_component::tooltip::Tooltip;
use lsp_types::DiagnosticSeverity;

use super::Workspace;
use crate::components::{ContextMenuExt as _, IconName, PopupMenu, PopupMenuItem, menu_icon};
use crate::glyphs::{Icon, icon};
use crate::model::Action;
use crate::panels::Activity;
use crate::theme::{Palette, palette};

/// A stripe's width.
pub const STRIPE_W: f32 = 40.;

/// The least width the right dock opens to for the Spring window.
const SPRING_W: f32 = 460.;

/// The least width a side dock takes when a terminal (the agent console, the Terminal)
/// moves into it: an agent's screen needs its columns.
const TERMINAL_SIDE_W: f32 = 480.;

/// A tool window a stripe opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Run,
    Debug,
    Build,
    Output,
    Terminal,
    Problems,
    Claude,
    Gradle,
    Maven,
    Spring,
    Notifications,
    ClaudeChanges,
    Structure,
    Hierarchy,
    /// Not on a stripe: the status bar and View ▸ Processes open it.
    Processes,
    /// A Playwright project's tests.
    Tests,
}

/// A dock a tool window can be moved to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Left, Side::Right, Side::Bottom];

    pub fn placement(self) -> DockPlacement {
        match self {
            Side::Left => DockPlacement::Left,
            Side::Right => DockPlacement::Right,
            Side::Bottom => DockPlacement::Bottom,
        }
    }

    pub fn of(place: DockPlacement) -> Option<Side> {
        Side::ALL.into_iter().find(|s| s.placement() == place)
    }

    fn label(self) -> &'static str {
        match self {
            Side::Left => "Left",
            Side::Right => "Right",
            Side::Bottom => "Bottom",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Side::Left => IconName::PanelLeft,
            Side::Right => IconName::PanelRight,
            Side::Bottom => IconName::PanelBottom,
        }
    }
}

/// Where each tool window is now, for the Move to menus (drawn where the workspace is not
/// at hand): refreshed whenever the dock layout changes.
#[derive(Clone, Debug, Default)]
pub struct ToolPlaces(pub Vec<(Tool, Side)>);

impl gpui::Global for ToolPlaces {}

/// The Move to items of `tool`'s menus: its dock checked, the others a click away.
pub fn move_menu(mut menu: PopupMenu, tool: Tool, cx: &gpui::App) -> PopupMenu {
    let now = cx.try_global::<ToolPlaces>().and_then(|p| p.0.iter().find(|(t, _)| *t == tool).map(|(_, s)| *s));
    let now = now.or_else(|| Side::of(tool.placement()));
    menu = menu.label("Move to");
    for side in Side::ALL {
        menu = menu.item(
            PopupMenuItem::new(side.label())
                .icon(menu_icon(side.icon()))
                .checked(now == Some(side))
                .on_click(move |_, w, cx| w.dispatch_action(Box::new(crate::menus::MoveToolWindow(tool, side)), cx)),
        );
    }
    menu
}

/// The dock that holds the panel `id`, if any.
pub fn dock_of(dock: &DockArea, id: PanelId) -> Option<DockPlacement> {
    [DockPlacement::Left, DockPlacement::Bottom, DockPlacement::Right, DockPlacement::Center]
        .into_iter()
        .find(|place| dock.layout(*place).is_some_and(|t| t.panels().any(|p| p == id)))
}

/// The tool windows in the order the stripes list them, whichever dock they are in.
const ORDER: [Tool; 15] = [
    Tool::Tests,
    Tool::Run,
    Tool::Debug,
    Tool::Build,
    Tool::Output,
    Tool::Terminal,
    Tool::Problems,
    Tool::Claude,
    Tool::Structure,
    Tool::Hierarchy,
    Tool::Gradle,
    Tool::Maven,
    Tool::Spring,
    Tool::Notifications,
    Tool::ClaudeChanges,
];

impl Tool {
    /// The dock it starts in.
    pub fn placement(self) -> DockPlacement {
        match self {
            Tool::Gradle
            | Tool::Maven
            | Tool::Spring
            | Tool::Notifications
            | Tool::ClaudeChanges
            | Tool::Structure
            | Tool::Hierarchy => DockPlacement::Right,
            _ => DockPlacement::Bottom,
        }
    }

    /// The tool window whose panel is called `name` in the dock.
    pub fn of_panel(name: &str) -> Option<Tool> {
        Some(match name {
            "Run" => Tool::Run,
            "Debug" => Tool::Debug,
            "Build" => Tool::Build,
            "Output" => Tool::Output,
            "Terminal" => Tool::Terminal,
            "Problems" | "References" => Tool::Problems,
            "Claude" => Tool::Claude,
            "Gradle" => Tool::Gradle,
            "Maven" => Tool::Maven,
            "Spring" => Tool::Spring,
            "Notifications" => Tool::Notifications,
            "Claude Changes" => Tool::ClaudeChanges,
            "Processes" => Tool::Processes,
            "Structure" => Tool::Structure,
            "Hierarchy" => Tool::Hierarchy,
            "Tests" => Tool::Tests,
            _ => return None,
        })
    }

    pub fn icon(self) -> Icon {
        match self {
            Tool::Run => Icon::RunWindow,
            Tool::Debug => Icon::Bug,
            Tool::Build => Icon::Hammer,
            Tool::Output => Icon::Output,
            Tool::Terminal => Icon::Terminal,
            Tool::Problems => Icon::Problems,
            Tool::Claude => Icon::Spark,
            Tool::Gradle => Icon::Build,
            Tool::Maven => Icon::Feather,
            Tool::Spring => Icon::Leaf,
            Tool::Notifications => Icon::Bell,
            Tool::ClaudeChanges => Icon::ClaudeChanges,
            Tool::Structure => Icon::Structure,
            Tool::Hierarchy => Icon::Hierarchy,
            Tool::Processes => Icon::Gear,
            Tool::Tests => Icon::TestFolder,
        }
    }

    /// The hue its icon takes when icons are in colour.
    pub fn hue(self) -> usize {
        match self {
            Tool::Run => 1,
            Tool::Debug => 3,
            Tool::Build => 2,
            Tool::Output => 0,
            Tool::Terminal => 5,
            Tool::Problems => 6,
            Tool::Claude => 7,
            Tool::Gradle => 5,
            Tool::Maven => 3,
            Tool::Spring => 1,
            Tool::Notifications => 6,
            Tool::Processes => 0,
            Tool::ClaudeChanges => 2,
            Tool::Structure => 4,
            Tool::Hierarchy => 0,
            Tool::Tests => 4,
        }
    }

    fn tip(self) -> &'static str {
        match self {
            Tool::Run => "Run",
            Tool::Debug => "Debug",
            Tool::Build => "Build",
            Tool::Output => "Output",
            Tool::Terminal => "Terminal",
            Tool::Problems => "Problems (⇧⌘M)",
            Tool::Claude => "Agent Console (⌘J)",
            Tool::Gradle => "Gradle",
            Tool::Maven => "Maven",
            Tool::Spring => "Spring",
            Tool::Notifications => "Notifications",
            Tool::ClaudeChanges => "Agent Changes",
            Tool::Structure => "Structure (⌘7)",
            Tool::Hierarchy => "Hierarchy (⌃H, ⌃⌥H)",
            Tool::Processes => "Processes",
            Tool::Tests => "Tests",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Tool::Run => "stripe-run",
            Tool::Debug => "stripe-debug",
            Tool::Build => "stripe-build",
            Tool::Output => "stripe-output",
            Tool::Terminal => "stripe-terminal",
            Tool::Problems => "stripe-problems",
            Tool::Claude => "stripe-claude",
            Tool::Gradle => "stripe-gradle",
            Tool::Maven => "stripe-maven",
            Tool::Spring => "stripe-spring",
            Tool::Notifications => "stripe-notifications",
            Tool::ClaudeChanges => "stripe-claude-changes",
            Tool::Structure => "stripe-structure",
            Tool::Hierarchy => "stripe-hierarchy",
            Tool::Processes => "stripe-processes",
            Tool::Tests => "stripe-tests",
        }
    }
}

/// The panel in front in the dock at `place`.
pub fn active_in(dock: &DockArea, place: DockPlacement) -> Option<PanelId> {
    let tree = dock.layout(place)?;
    let mut out = None;
    tree.root().walk(&mut |node| {
        if out.is_none()
            && let PaneRef::Tabs { panels, active_ix } = node.kind()
        {
            out = panels.get(active_ix).copied();
        }
    });
    out
}

/// A stripe button: the glyph, filled in the accent colour while its window shows, and a
/// small count in the corner when there is one.
fn item(
    id: &'static str,
    g: Icon,
    tip: &'static str,
    on: bool,
    badge: Option<(usize, gpui::Hsla)>,
    hue: usize,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let ink = if on { p.on_accent } else { p.hue(hue) };
    let glow = p.glow_of(p.accent);
    let hover = p.button_hover;
    let ground = p.ground;
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .relative()
        .flex_none()
        .size(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .when(on, |d| d.bg(p.accent).shadow(glow))
        .when(!on, |d| d.hover(move |s| s.bg(hover)))
        .tooltip(move |w, cx| Tooltip::new(crate::keymap::shown(tip)).build(w, cx))
        .child(icon(g, 18., ink))
        .when_some(badge.filter(|(n, _)| *n > 0), |d, (n, color)| {
            d.child(
                div()
                    .absolute()
                    .top(px(-3.))
                    .right(px(-4.))
                    .min_w(px(14.))
                    .h(px(14.))
                    .px(px(3.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(ground)
                    .bg(p.tile(color).0)
                    .text_color(p.tile(color).1)
                    .text_size(px(9.))
                    .child(if n > 99 { "99+".to_owned() } else { n.to_string() }),
            )
        })
}

impl Workspace {
    /// Whether the project has a Maven build, as its model or the engine's goals say.
    pub fn shows_maven(&self, cx: &gpui::App) -> bool {
        let runs = self.runs.read(cx);
        !runs.maven.builds.is_empty()
            || crate::project_model::store(cx)
                .and_then(|s| s.read(cx).current.clone())
                .is_some_and(|m| m.modules.iter().any(|m| m.maven))
    }

    /// The Gradle window shows unless the project is known to be Maven alone.
    pub fn shows_gradle(&self, cx: &gpui::App) -> bool {
        match crate::project_model::store(cx).and_then(|s| s.read(cx).current.clone()) {
            Some(m) if !m.modules.is_empty() => m.modules.iter().any(|m| !m.maven),
            _ => true,
        }
    }

    /// The dock panel of `tool`.
    pub fn tool_panel(&self, tool: Tool) -> Option<PanelId> {
        match tool {
            Tool::Run => Some(self.tabs.run),
            Tool::Debug => Some(self.tabs.debug),
            Tool::Build => Some(self.tabs.build),
            Tool::Output => Some(self.tabs.output),
            Tool::Terminal => Some(self.tabs.terminal),
            Tool::Problems => Some(self.tabs.problems),
            Tool::Claude => self.tabs.console,
            Tool::Gradle => Some(self.tabs.gradle),
            Tool::Maven => Some(self.tabs.maven),
            Tool::Spring => Some(self.tabs.spring),
            Tool::Notifications => Some(self.tabs.notifications),
            Tool::ClaudeChanges => Some(self.tabs.claude_changes),
            Tool::Structure => Some(self.tabs.structure),
            Tool::Hierarchy => Some(self.tabs.hierarchy),
            Tool::Processes => Some(self.tabs.processes),
            Tool::Tests => Some(self.tabs.tests),
        }
    }

    /// The dock `tool`'s window is in: where it was moved to, else where it starts.
    pub fn placement_of(&self, tool: Tool, cx: &gpui::App) -> DockPlacement {
        let dock = self.dock.read(cx);
        self.tool_panel(tool).and_then(|id| dock_of(dock, id)).unwrap_or(tool.placement())
    }

    /// The dock that holds the panel of dock name `name`.
    pub fn dock_of_panel_named(&self, name: &str, cx: &gpui::App) -> Option<DockPlacement> {
        let (view, _) = self.panels.iter().find(|(v, _)| v.panel_name(cx) == name)?;
        dock_of(self.dock.read(cx), view.panel_id(cx))
    }

    /// The panel id of the side panel (Project), which the left dock shows by default.
    fn side_panel(&self) -> PanelId {
        crate::tabs::panel_id(&self.side)
    }

    /// Whether the left dock shows the side panel (Project) rather than a tool window
    /// moved there.
    pub fn side_in_front(&self, cx: &gpui::App) -> bool {
        active_in(self.dock.read(cx), DockPlacement::Left).is_none_or(|id| id == self.side_panel())
    }

    /// Bring the side panel (Project) to the front of the left dock.
    pub(super) fn front_side(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.side_in_front(cx) {
            let id = self.side_panel();
            self.dock.update(cx, |d, cx| d.select_panel(id, window, cx));
        }
    }

    /// Note where each tool window is, for the Move to menus.
    pub(super) fn refresh_tool_places(&self, cx: &mut gpui::App) {
        let places =
            ORDER.into_iter().filter_map(|t| Side::of(self.placement_of(t, cx)).map(|s| (t, s))).collect::<Vec<_>>();
        cx.set_global(ToolPlaces(places));
    }

    /// Move `tool`'s window to the dock at `to`, and show it there.
    pub fn move_tool(&mut self, tool: Tool, to: Side, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::dock::InsertTarget;
        let Some(id) = self.tool_panel(tool) else { return };
        let place = to.placement();
        if self.placement_of(tool, cx) != place {
            self.dock.update(cx, |d, cx| {
                // Into the dock's tab group (where the stripe buttons pick what shows), or,
                // in a dock left empty, a group of its own.
                let target = d.layout(place).map(|tree| {
                    let mut group = None;
                    tree.root().walk(&mut |node| {
                        if group.is_none() && matches!(node.kind(), PaneRef::Tabs { .. }) {
                            group = Some(node.id());
                        }
                    });
                    match group {
                        Some(node) => InsertTarget::Tabs { node, ix: None, activate: true },
                        None => InsertTarget::Split {
                            node: tree.root().id(),
                            placement: gpui_component::Placement::Right,
                            size: None,
                        },
                    }
                });
                match target {
                    Some(target) => d.move_panel(id, target, window, cx),
                    // A dock never made (none is missing in a layout of this build).
                    None => {
                        if let Some(view) = d.panel(id).cloned() {
                            d.add_panel_view(view, place, None, window, cx);
                        }
                    }
                }
            });
            self.refresh_tool_places(cx);
        }
        if self.side.read(cx).activity == Activity::Git {
            self.show_activity(Activity::Project, window, cx);
        }
        self.dock.update(cx, |d, cx| {
            if !d.is_dock_open(place) {
                d.toggle_dock(place, window, cx);
            }
            d.select_panel(id, window, cx);
            let terminal = matches!(tool, Tool::Claude | Tool::Terminal);
            if terminal && to != Side::Bottom && d.dock_size(place).is_some_and(|w| w < px(TERMINAL_SIDE_W)) {
                d.set_dock_size(place, px(TERMINAL_SIDE_W), window, cx);
            }
        });
        if tool == Tool::Claude {
            self.note_console_open(true, cx);
        }
        cx.notify();
    }

    /// Show or hide the agent console where it is (⌘J, its ×, the View menu): its dock
    /// opens with it in front, or closes; in the left dock, which it shares with Project,
    /// hiding it brings Project back instead.
    pub fn set_console_shown(&mut self, show: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.tool_panel(Tool::Claude) else { return };
        let place = self.placement_of(Tool::Claude, cx);
        if show {
            self.dock.update(cx, |d, cx| {
                if !d.is_dock_open(place) {
                    d.toggle_dock(place, window, cx);
                }
                d.select_panel(id, window, cx);
            });
        } else if place == DockPlacement::Left {
            self.front_side(window, cx);
        } else {
            self.hide_dock(place, window, cx);
        }
        self.note_console_open(show, cx);
        cx.notify();
    }

    /// Keep the setting that says the console shows (and lets its session start) in step
    /// with what the window shows.
    pub(super) fn note_console_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.model.read(cx).settings.console_open != open {
            self.model.update(cx, |m, cx| {
                m.settings.console_open = open;
                m.persist();
                cx.notify();
            });
        }
    }

    /// Whether `tool`'s window shows: its dock is open with it in front.
    pub fn tool_open(&self, tool: Tool, cx: &gpui::App) -> bool {
        if tool == Tool::Claude && self.side.read(cx).activity == Activity::Git {
            return self.model.read(cx).settings.console_open;
        }
        let place = self.placement_of(tool, cx);
        let dock = self.dock.read(cx);
        self.side.read(cx).activity != Activity::Git
            && dock.is_dock_open(place)
            && self.tool_panel(tool).is_some_and(|id| active_in(dock, place) == Some(id))
    }

    /// Show `tool`'s window, or hide it when it shows.
    pub fn toggle_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        let git = self.side.read(cx).activity == Activity::Git;
        if tool == Tool::Claude && git {
            // The Git view has its own place for the console.
            return self.send(Action::ToggleConsole, cx);
        }
        let Some(id) = self.tool_panel(tool) else { return };
        let open = self.tool_open(tool, cx);
        if git {
            self.show_activity(Activity::Project, window, cx);
        }
        let place = self.placement_of(tool, cx);
        self.dock.update(cx, |d, cx| {
            if open {
                d.toggle_dock(place, window, cx);
            } else {
                if !d.is_dock_open(place) {
                    d.toggle_dock(place, window, cx);
                }
                d.select_panel(id, window, cx);
                // The Spring window's table wants more room than the dock's first width.
                if tool == Tool::Spring && d.dock_size(place).is_some_and(|w| w < px(SPRING_W)) {
                    d.set_dock_size(place, px(SPRING_W), window, cx);
                }
            }
        });
        if tool == Tool::Claude {
            let shown = self.tool_open(tool, cx);
            self.note_console_open(shown, cx);
        }
        cx.notify();
    }

    /// Show `tool`'s window (it stays when it already shows).
    pub fn show_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tool_open(tool, cx) {
            self.toggle_tool(tool, window, cx);
        }
    }

    /// Hide the dock at `place` (a tool window header's hide button).
    pub fn hide_dock(&mut self, place: DockPlacement, window: &mut Window, cx: &mut Context<Self>) {
        self.dock.update(cx, |d, cx| {
            if d.is_dock_open(place) {
                d.toggle_dock(place, window, cx);
            }
        });
        cx.notify();
    }

    /// The errors the language servers report.
    fn error_count(&self, cx: &gpui::App) -> usize {
        self.hub.read(cx).all_problems().iter().filter(|(_, d)| d.severity == Some(DiagnosticSeverity::ERROR)).count()
    }

    fn tool_item(&self, tool: Tool, badge: Option<(usize, gpui::Hsla)>, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        item(tool.id(), tool.icon(), tool.tip(), self.tool_open(tool, cx), badge, tool.hue(), &p)
            .on_click(cx.listener(move |this, _, w, cx| this.toggle_tool(tool, w, cx)))
            .context_menu(move |menu, _, cx| move_menu(menu, tool, cx))
            .into_any_element()
    }

    /// The stripe buttons of the tool windows in the dock at `place`, with their counts.
    fn tool_items(&self, place: DockPlacement, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = palette(cx);
        let mut out = Vec::new();
        for tool in ORDER {
            if self.placement_of(tool, cx) != place {
                continue;
            }
            let shown = match tool {
                Tool::Tests => self.pw_tests.read(cx).root.is_some(),
                Tool::Gradle => self.shows_gradle(cx),
                Tool::Maven => self.shows_maven(cx),
                _ => true,
            };
            if !shown {
                continue;
            }
            let badge = match tool {
                Tool::Tests => Some((self.pw_tests.read(cx).run(cx).map_or(0, |r| r.counts().failed), p.error)),
                Tool::Problems => Some((self.error_count(cx), p.error)),
                Tool::Notifications => Some((self.notifications.read(cx).items(cx).len(), p.accent)),
                Tool::ClaudeChanges => {
                    Some((crate::claude_ide::bridge::changes_of(self, cx).read(cx).set.files.len(), p.claude().mark))
                }
                _ => None,
            };
            out.push(self.tool_item(tool, badge, cx));
        }
        out
    }

    pub(super) fn left_stripe(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let current = self.side.read(cx).activity;
        let left_open = self.dock.read(cx).is_dock_open(DockPlacement::Left);
        let open = current == Activity::Git || (left_open && self.side_in_front(cx));
        let changed = self.model.read(cx).status.files.len();
        let (left, bottom) = (self.tool_items(DockPlacement::Left, cx), self.tool_items(DockPlacement::Bottom, cx));
        let pick = |a: Activity| cx.listener(move |this, _, w, cx| this.set_activity(a, w, cx));
        div()
            .id("left-stripe")
            .flex_none()
            .w(px(STRIPE_W))
            .h_full()
            .py(px(6.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .bg(p.ground)
            .border_r_1()
            .border_color(p.border)
            .child(
                item("act-project", Icon::Folder, "Project (⌘1)", open && current == Activity::Project, None, 0, &p)
                    .on_click(pick(Activity::Project)),
            )
            .child(
                item(
                    "act-git",
                    Icon::Branch,
                    "Git (⌘2)",
                    open && current == Activity::Git,
                    Some((changed, p.accent)),
                    2,
                    &p,
                )
                .on_click(pick(Activity::Git)),
            )
            .when(!left.is_empty(), |d| d.child(div().my(px(2.)).w(px(20.)).h(px(1.)).bg(p.border)).children(left))
            .child(div().flex_1())
            .children(bottom)
            .into_any_element()
    }

    pub(super) fn right_stripe(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let items = self.tool_items(DockPlacement::Right, cx);
        div()
            .id("right-stripe")
            .flex_none()
            .w(px(STRIPE_W))
            .h_full()
            .py(px(6.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .bg(p.ground)
            .border_l_1()
            .border_color(p.border)
            .children(items)
            .into_any_element()
    }
}
