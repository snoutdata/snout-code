//! The dock panels: the left side panel the activity bar switches, and the tabs of the
//! center and the bottom dock.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyView, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use gpui_component::Sizable as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dock::{BasePanel, Panel, PanelEvent};
use gpui_component::menu::DropdownMenu as _;
use gpui_component::scroll::ScrollableElement as _;

use crate::model::{Action, Model};
use crate::projects::open_items;
use crate::theme::palette;
use crate::ui::{icon, icons};

/// Implements the dock traits for a panel holding a `focus` handle.
macro_rules! panel {
    ($ty:ty, $name:literal, $title:expr, $bar:expr) => {
        impl EventEmitter<PanelEvent> for $ty {}
        impl Focusable for $ty {
            fn focus_handle(&self, _: &App) -> FocusHandle {
                self.focus.clone()
            }
        }
        impl BasePanel for $ty {
            fn panel_name(&self) -> &'static str {
                $name
            }
            fn closable(&self, _: &App) -> bool {
                false
            }
        }
        impl Panel for $ty {
            fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let f: fn(&Self, &App) -> SharedString = $title;
                f(self, cx)
            }
            fn title_bar(&self, _: &App) -> bool {
                $bar
            }
            fn inner_padding(&self, _: &App) -> bool {
                false
            }
            fn zoom_control(&self, _: &App) -> Option<gpui_component::dock::PanelControl> {
                None
            }
        }
    };
}
#[allow(unused_imports)]
pub(crate) use panel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    Projects,
    Git,
}

/// The left dock's one panel: whichever view the activity bar picked.
pub struct SidePanel {
    pub activity: Activity,
    pub projects: AnyView,
    pub git: AnyView,
    focus: FocusHandle,
}

impl SidePanel {
    pub fn new(projects: AnyView, git: AnyView, cx: &mut Context<Self>) -> Self {
        Self { activity: Activity::Projects, projects, git, focus: cx.focus_handle() }
    }
}

panel!(
    SidePanel,
    "Side",
    |s, _| match s.activity {
        Activity::Projects => "Projects".into(),
        Activity::Git => "Git".into(),
    },
    false
);

impl Render for SidePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div().size_full().bg(p.panel).child(match self.activity {
            Activity::Projects => self.projects.clone(),
            Activity::Git => self.git.clone(),
        })
    }
}

/// The center's first tab while no repository is open: what to do first.
pub struct WelcomePanel {
    model: Entity<Model>,
    focus: FocusHandle,
}

impl WelcomePanel {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, focus: cx.focus_handle() }
    }
}

panel!(WelcomePanel, "Welcome", |_, _| "Welcome".into(), true);

impl Render for WelcomePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let model = self.model.clone();
        let recent: Vec<_> = self.model.read(cx).settings.repos.iter().rev().take(8).cloned().collect();
        let button = |id: &'static str, glyph: &'static str, label: &'static str, action: Action| {
            let model = model.clone();
            Button::new(id).icon(icon(glyph)).label(label).w(px(300.)).on_click(move |_, _, cx| {
                let action = action.clone();
                model.update(cx, |m, cx| m.handle(action, cx));
            })
        };
        div()
            .id("welcome")
            .size_full()
            .bg(p.panel)
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(60.))
            .gap(px(8.))
            .child(div().text_size(px(40.)).font_weight(FontWeight::BOLD).child("SnoutCode"))
            .child(
                div().text_color(p.fg_dim).text_size(px(15.)).child("See what changed, read the diff, commit, ship."),
            )
            .child(div().h(px(14.)))
            .child(button("welcome-open", icons::OPEN, "Open local repository…", Action::PickFolder))
            .child(button("welcome-find", icons::SEARCH, "Find repositories in folder…", Action::PickScanFolder))
            .child(button("welcome-clone", icons::PULL, "Clone from GitHub…", Action::ShowClone))
            .when(!recent.is_empty(), |d| {
                d.child(div().h(px(18.))).child(div().text_size(px(11.)).text_color(p.fg_dim).child("RECENT")).children(
                    recent.into_iter().enumerate().map(|(i, path)| {
                        let model = model.clone();
                        let name = Model::dir_name(&path);
                        Button::new(("recent", i)).link().label(name).tooltip(path.display().to_string()).on_click(
                            move |_, _, cx| {
                                let a = Action::OpenRepo(path.clone());
                                model.update(cx, |m, cx| m.handle(a, cx));
                            },
                        )
                    }),
                )
            })
            .child(Button::new("welcome-more").ghost().small().label("More").dropdown_menu({
                let model = model.clone();
                move |menu, _, _| open_items(menu, &model)
            }))
    }
}

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
        let model = self.model.clone();
        div()
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .child(div().flex().justify_end().p(px(4.)).child(
                Button::new("clear-output").ghost().xsmall().label("Clear").on_click(move |_, _, cx| {
                    model.update(cx, |m, cx| {
                        m.output.clear();
                        cx.notify();
                    })
                }),
            ))
            .child(
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
