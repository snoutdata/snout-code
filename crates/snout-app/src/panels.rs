//! The dock panels: the left side panel the activity bar switches, and the tabs of the
//! center and the bottom dock.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::Sizable as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::DropdownMenu as _;
use gpui_component::scroll::ScrollableElement as _;

use crate::model::Model;
use crate::theme::palette;

/// Implements the dock traits for a panel holding a `focus` handle.
macro_rules! panel {
    ($ty:ty, $name:literal, $title:expr, $bar:expr) => {
        impl ::gpui::EventEmitter<::gpui_component::dock::PanelEvent> for $ty {}
        impl ::gpui::Focusable for $ty {
            fn focus_handle(&self, _: &::gpui::App) -> ::gpui::FocusHandle {
                self.focus.clone()
            }
        }
        impl ::gpui_component::dock::BasePanel for $ty {
            fn panel_name(&self) -> &'static str {
                $name
            }
            fn closable(&self, _: &::gpui::App) -> bool {
                false
            }
        }
        impl ::gpui_component::dock::Panel for $ty {
            fn title(&mut self, _: &mut ::gpui::Window, cx: &mut ::gpui::Context<Self>) -> impl ::gpui::IntoElement {
                let f: fn(&Self, &::gpui::App) -> ::gpui::SharedString = $title;
                f(self, cx)
            }
            fn title_bar(&self, _: &::gpui::App) -> bool {
                $bar
            }
            fn inner_padding(&self, _: &::gpui::App) -> bool {
                false
            }
            fn zoom_control(&self, _: &::gpui::App) -> Option<::gpui_component::dock::PanelControl> {
                None
            }
        }
    };
}
#[allow(unused_imports)]
pub(crate) use panel;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Activity {
    /// The project: its file tree, the editor tabs and the bottom dock. Saved settings from
    /// before it was renamed call it Projects.
    #[serde(alias = "Projects")]
    Project,
    Git,
}

/// The left dock's one panel: whichever view the activity bar picked. Git is not docked:
/// it takes the whole window right of the activity bar. The first launch opens on the
/// project.
pub struct SidePanel {
    pub activity: Activity,
    model: Entity<Model>,
    /// The open project's files, the whole height of the panel (the Files view).
    pub files: Entity<crate::files::FileTree>,
    /// The project as its Gradle model has it (the Project view).
    pub project: Entity<crate::project_view::ProjectView>,
    focus: FocusHandle,
}

impl SidePanel {
    pub fn new(
        model: Entity<Model>,
        files: Entity<crate::files::FileTree>,
        project: Entity<crate::project_view::ProjectView>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { activity: Activity::Project, model, files, project, focus: cx.focus_handle() }
    }

    /// The Files view shows (the plain tree), not the Project view.
    pub fn plain_files(&self, cx: &App) -> bool {
        self.model.read(cx).settings.plain_files
    }

    /// Show the Files view (`true`) or the Project view.
    pub fn set_plain_files(&mut self, plain: bool, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| {
            m.settings.plain_files = plain;
            m.persist();
            cx.notify();
        });
        cx.notify();
    }
}

panel!(
    SidePanel,
    "Side",
    |s, _| match s.activity {
        Activity::Project => "Project".into(),
        Activity::Git => "Git".into(),
    },
    false
);

impl Render for SidePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::glyphs::Icon;
        use crate::menus::{CollapseAll, HideSide, LocateFile};
        use crate::tabs::tool_button;
        let p = palette(cx);
        let plain = self.plain_files(cx);
        let view_name = if plain { "Files" } else { "Project" };
        let side = cx.entity();
        // The view switcher: Project (the Gradle model's view) or Files (the plain tree).
        let switcher = Button::new("side-view")
            .debug_selector(|| "side-view".into())
            .ghost()
            .small()
            .label(view_name)
            .dropdown_menu(move |menu, _, _| {
                let pick = |plain: bool| {
                    let side = side.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                        side.update(cx, |s, cx| s.set_plain_files(plain, cx))
                    }
                };
                menu.item(gpui_component::menu::PopupMenuItem::new("Project").checked(!plain).on_click(pick(false)))
                    .item(gpui_component::menu::PopupMenuItem::new("Files").checked(plain).on_click(pick(true)))
            });
        // The tool window's header: its title, the view switcher and the toolbar, then the
        // view.
        let header = div()
            .flex_none()
            .h(px(32.))
            .pl(px(12.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(2.))
            .text_size(px(crate::look::BODY))
            .child(div().ml(px(-8.)).font_weight(FontWeight::MEDIUM).child(switcher))
            .child(div().flex_1())
            .child(
                tool_button("side-locate", Icon::Locate, "Select Opened File", &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(LocateFile), cx)),
            )
            .child(
                tool_button("side-collapse", Icon::CollapseAll, "Collapse All", &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(CollapseAll), cx)),
            )
            .child(
                tool_button("side-hide", Icon::Minus, "Hide", &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(HideSide), cx)),
            );
        div().size_full().bg(p.ground).flex().flex_col().child(header).child(div().flex_1().min_h_0().px(px(4.)).map(
            |d| {
                if plain { d.child(self.files.clone()) } else { d.child(self.project.clone()) }
            },
        ))
    }
}

pub use crate::welcome::WelcomePanel;

/// The bottom dock's Output tab: what Git said.
pub struct OutputPanel {
    model: Entity<Model>,
    focus: FocusHandle,
}

impl OutputPanel {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, focus: cx.focus_handle() }
    }
}

panel!(OutputPanel, "Output", |_, _| "Output".into(), true);

impl Render for OutputPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        div().size_full().bg(p.ground).flex().flex_col().child(
            div()
                .id("output")
                .flex_1()
                .overflow_y_scrollbar()
                .px(px(8.))
                .font_family(cx.theme_mono())
                .text_size(px(12.))
                .children(
                    m.output
                        .iter()
                        .map(|l| div().text_color(if l.error { p.error } else { p.fg }).child(l.text.clone())),
                ),
        )
    }
}

trait MonoFont {
    fn theme_mono(&self) -> SharedString;
}

impl MonoFont for App {
    fn theme_mono(&self) -> SharedString {
        gpui_component::Theme::global(self).mono_font_family.clone()
    }
}
