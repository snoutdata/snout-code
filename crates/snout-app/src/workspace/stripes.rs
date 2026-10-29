//! The tool window stripes: thin icon strips down both edges of the window. Left, at the
//! top: the Project and Git activities; at the bottom: Run, Debug, Build, Output,
//! Terminal, Problems and Claude Code, which open in the bottom dock. Right: Gradle, Spring, Notifications
//! and Claude Changes, in the right dock. The item showing is a filled accent square with a white glyph.

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
use crate::glyphs::{Icon, icon};
use crate::model::Action;
use crate::panels::Activity;
use crate::theme::{Palette, palette};

/// A stripe's width.
pub const STRIPE_W: f32 = 40.;

/// The least width the right dock opens to for the Spring window.
const SPRING_W: f32 = 460.;

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
    Spring,
    Notifications,
    ClaudeChanges,
    /// Not on a stripe: the status bar and View ▸ Processes open it.
    Processes,
}

impl Tool {
    pub fn placement(self) -> DockPlacement {
        match self {
            Tool::Gradle | Tool::Spring | Tool::Notifications | Tool::ClaudeChanges => DockPlacement::Right,
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
            "Spring" => Tool::Spring,
            "Notifications" => Tool::Notifications,
            "Claude Changes" => Tool::ClaudeChanges,
            "Processes" => Tool::Processes,
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
            Tool::Spring => Icon::Leaf,
            Tool::Notifications => Icon::Bell,
            Tool::ClaudeChanges => Icon::ClaudeChanges,
            Tool::Processes => Icon::Gear,
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
            Tool::Spring => 1,
            Tool::Notifications => 6,
            Tool::Processes => 0,
            Tool::ClaudeChanges => 2,
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
            Tool::Claude => "Claude Code (⌘J)",
            Tool::Gradle => "Gradle",
            Tool::Spring => "Spring",
            Tool::Notifications => "Notifications",
            Tool::ClaudeChanges => "Claude Changes",
            Tool::Processes => "Processes",
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
            Tool::Spring => "stripe-spring",
            Tool::Notifications => "stripe-notifications",
            Tool::ClaudeChanges => "stripe-claude-changes",
            Tool::Processes => "stripe-processes",
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
        .tooltip(move |w, cx| Tooltip::new(tip).build(w, cx))
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
            Tool::Spring => Some(self.tabs.spring),
            Tool::Notifications => Some(self.tabs.notifications),
            Tool::ClaudeChanges => Some(self.tabs.claude_changes),
            Tool::Processes => Some(self.tabs.processes),
        }
    }

    /// Whether `tool`'s window shows: its dock is open with it in front.
    pub fn tool_open(&self, tool: Tool, cx: &gpui::App) -> bool {
        if tool == Tool::Claude && self.side.read(cx).activity == Activity::Git {
            return self.model.read(cx).settings.console_open;
        }
        let dock = self.dock.read(cx);
        let place = tool.placement();
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
        let place = tool.placement();
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
        cx.notify();
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
            .into_any_element()
    }

    pub(super) fn left_stripe(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let current = self.side.read(cx).activity;
        let open = current == Activity::Git || self.dock.read(cx).is_dock_open(DockPlacement::Left);
        let changed = self.model.read(cx).status.files.len();
        let errors = self.error_count(cx);
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
            .child(div().flex_1())
            .child(self.tool_item(Tool::Run, None, cx))
            .child(self.tool_item(Tool::Debug, None, cx))
            .child(self.tool_item(Tool::Build, None, cx))
            .child(self.tool_item(Tool::Output, None, cx))
            .child(self.tool_item(Tool::Terminal, None, cx))
            .child(self.tool_item(Tool::Problems, Some((errors, p.error)), cx))
            .child(self.tool_item(Tool::Claude, None, cx))
            .into_any_element()
    }

    pub(super) fn right_stripe(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let notes = self.notifications.read(cx).items(cx).len();
        let claude = crate::claude_ide::bridge::changes_of(self, cx).read(cx).set.files.len();
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
            .child(self.tool_item(Tool::Gradle, None, cx))
            .child(self.tool_item(Tool::Spring, None, cx))
            .child(self.tool_item(Tool::Notifications, Some((notes, p.accent)), cx))
            .child(self.tool_item(Tool::ClaudeChanges, Some((claude, p.claude().mark)), cx))
            .into_any_element()
    }
}
