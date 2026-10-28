//! The title bar, drawn by the app across the top of the window, round the window buttons:
//! the project and branch widgets on the left; the run configuration, Run and Debug,
//! Search Everywhere and Settings on the right.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::tooltip::Tooltip;

use super::Workspace;
use crate::branch_popup::{BranchPopup, Dismiss};
use crate::glyphs::{Icon, icon};
use crate::gradle::Build;
use crate::look::Trigger;
use crate::menus::{self, CloseProject, OpenRecent};
use crate::model::{Action, Model};
use crate::theme::{Palette, mix, palette};

/// The title bar's height.
pub const TITLE_H: f32 = 40.;

/// The engine's model of the project's build, kept while its file stays the same.
#[derive(Default)]
pub struct GradleCache {
    root: Option<PathBuf>,
    loaded: Option<(SystemTime, Rc<Build>)>,
    looked: Option<Instant>,
}

/// Up to two letters standing for a project: the first of its first two words.
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let letters: String = match words.as_slice() {
        [] => "?".into(),
        [one] => one.chars().take(2).collect(),
        [a, b, ..] => a.chars().take(1).chain(b.chars().take(1)).collect(),
    };
    letters.to_uppercase()
}

/// The badge colour of a project, picked from its name so it stays the same.
fn badge_color(name: &str, p: &Palette) -> gpui::Hsla {
    let hues = [0.58, 0.08, 0.36, 0.75, 0.95, 0.15, 0.5];
    let hash = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    let hue = hues[hash as usize % hues.len()];
    gpui::hsla(hue, 0.45, if p.dark { 0.42 } else { 0.5 }, 1.)
}

/// A quiet button of the title bar: filled only under the pointer.
fn bar_button(id: &'static str, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let hover = p.button_hover;
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .h(px(28.))
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

/// A square icon button of the title bar; greyed and inert when `enabled` is false.
fn icon_button(id: &'static str, g: Icon, tip: &'static str, enabled: bool, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let hover = p.button_hover;
    let ink = if enabled { p.fg } else { mix(p.fg_dim, 0.6, p.ground) };
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .tooltip(move |w, cx| Tooltip::new(tip).build(w, cx))
        .child(icon(g, 16., ink))
}

impl Workspace {
    /// The build the engine imported for the open project, if its model is there.
    pub fn gradle_build(&mut self, cx: &mut Context<Self>) -> Option<Rc<Build>> {
        let root = self.gradle_root(cx);
        if root != self.gradle.root {
            self.gradle = GradleCache { root: root.clone(), ..Default::default() };
        }
        let root = root?;
        // Look again now and then: the engine writes the model when it imports the build.
        let due = self.gradle.looked.is_none_or(|t| t.elapsed() > Duration::from_secs(3));
        if due {
            self.gradle.looked = Some(Instant::now());
            let home = self.hub.read(cx).home.clone();
            if let Some((build, when)) = crate::gradle::load(&home, &root)
                && self.gradle.loaded.as_ref().is_none_or(|(t, _)| *t != when)
            {
                self.gradle.loaded = Some((when, Rc::new(build)));
            }
        }
        self.gradle.loaded.as_ref().map(|(_, b)| b.clone())
    }

    /// The Gradle build of the open project: one its server runs, or the project itself.
    fn gradle_root(&self, cx: &Context<Self>) -> Option<PathBuf> {
        let repo = self.model.read(cx).repo.as_ref()?.root().to_owned();
        let hub = self.hub.read(cx);
        let mut roots: Vec<&PathBuf> = hub.servers.keys().filter(|r| r.starts_with(&repo)).collect();
        roots.sort();
        roots.first().map(|r| (*r).clone()).or_else(|| crate::lang::java::is_gradle_root(&repo).then_some(repo))
    }

    /// Show or hide the branch popup.
    pub fn toggle_branches(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.branch_popup.take().is_some() {
            cx.notify();
            return;
        }
        if self.model.read(cx).repo.is_none() {
            return;
        }
        let model = self.model.clone();
        let popup = cx.new(|cx| BranchPopup::new(model, window, cx));
        cx.subscribe_in(&popup, window, |this, _, _: &Dismiss, window, cx| {
            this.branch_popup = None;
            window.focus(&this.focus, cx);
            cx.notify();
        })
        .detach();
        let input = popup.read(cx).input.clone();
        cx.defer_in(window, move |_, window, cx| input.update(cx, |s, cx| s.focus(window, cx)));
        self.branch_popup = Some(popup);
        cx.notify();
    }

    pub(super) fn title_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let gradle = self.gradle_build(cx);
        let m = self.model.read(cx);
        let name = m.repo.as_ref().map(|r| Model::dir_name(r.root()));
        let branch = m.repo.as_ref().map(|_| m.head_label());
        let recent = m.recent_projects();
        let model = self.model.clone();

        // The project: its badge, its name, and a menu to open or close projects.
        let project = {
            let label = name.clone().unwrap_or_else(|| "No Project".into());
            let badge = div()
                .flex_none()
                .size(px(20.))
                .rounded(px(5.))
                .flex()
                .items_center()
                .justify_center()
                .bg(badge_color(&label, &p))
                .text_color(gpui::white())
                .text_size(px(10.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(initials(&label));
            let has_project = name.is_some();
            Trigger(
                bar_button("title-project", &p)
                    .child(badge)
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(label))
                    .child(icon(Icon::ChevronDown, 12., p.fg_dim)),
            )
            .dropdown_menu(move |mut menu, _, _| {
                let send = |action: Action| {
                    let model = model.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        model.update(cx, |m, cx| m.handle(action.clone(), cx))
                    }
                };
                menu = menu.item(PopupMenuItem::new("Open Project…").on_click(send(Action::PickFolder)));
                if !recent.is_empty() {
                    menu = menu.separator().label("Recent Projects");
                    for path in recent.iter().take(10) {
                        let path = path.clone();
                        menu = menu.item(
                            PopupMenuItem::new(Model::dir_name(&path))
                                .on_click(move |_, w, cx| w.dispatch_action(Box::new(OpenRecent(path.clone())), cx)),
                        );
                    }
                }
                menu.separator().item(
                    PopupMenuItem::new("Close Project")
                        .disabled(!has_project)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(CloseProject), cx)),
                )
            })
            .into_any_element()
        };

        // The branch: its name, and the popup of branches and repository actions.
        let branch_widget = branch.map(|b| {
            div()
                .relative()
                .flex_none()
                .child(
                    bar_button("title-branch", &p)
                        .child(icon(Icon::Branch, 15., p.fg_dim))
                        .child(div().max_w(px(240.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(b))
                        .child(icon(Icon::ChevronDown, 12., p.fg_dim))
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_branches(window, cx))),
                )
                .children(self.branch_popup.clone().map(|popup| {
                    div().absolute().top(px(32.)).left_0().child(
                        gpui::deferred(gpui::anchored().snap_to_window_with_margin(px(8.)).child(popup))
                            .with_priority(2),
                    )
                }))
        });

        // The run configuration: the current file, or a Gradle task of the build.
        let run_config = {
            let tasks: Vec<(String, String)> = gradle
                .as_ref()
                .map(|b| {
                    b.projects.iter().flat_map(|pr| pr.tasks.iter().map(|t| (pr.task_path(t), t.clone()))).collect()
                })
                .unwrap_or_default();
            Trigger(bar_button("title-run-config", &p).child(div().child("Current File")).child(icon(
                Icon::ChevronDown,
                12.,
                p.fg_dim,
            )))
            .dropdown_menu(move |mut menu, _, _| {
                menu = menu.item(PopupMenuItem::new("Current File").checked(true));
                menu = menu.separator().label("Gradle");
                if tasks.is_empty() {
                    return menu.item(PopupMenuItem::new("Tasks appear once the build is imported").disabled(true));
                }
                for (path, _) in tasks.iter().take(40) {
                    menu = menu.item(PopupMenuItem::new(path.clone()).disabled(true));
                }
                menu
            })
            .into_any_element()
        };

        let right = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(run_config)
            .child(icon_button("title-run", Icon::Play, "Run (coming soon)", false, &p))
            .child(icon_button("title-debug", Icon::Bug, "Debug (coming soon)", false, &p))
            .child(div().w(px(6.)))
            .child(
                icon_button("title-search", Icon::Search, "Search Everywhere (⇧⌘A)", true, &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(menus::SearchEverywhere), cx)),
            )
            .child(
                icon_button("title-settings", Icon::Gear, "Settings (⌘,)", true, &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(menus::OpenSettings), cx)),
            );

        gpui_component::TitleBar::new()
            .h(px(TITLE_H))
            .bg(p.ground)
            .border_b_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .text_size(px(crate::look::BODY))
            .text_color(p.fg)
            .child(
                div()
                    .id("title-bar")
                    .flex_1()
                    .h_full()
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(project)
                    .children(branch_widget)
                    .child(div().flex_1())
                    .child(right),
            )
            .into_any_element()
    }
}

/// The popup's entity, for tests.
pub fn branch_popup(ws: &Workspace) -> Option<Entity<BranchPopup>> {
    ws.branch_popup.clone()
}

#[cfg(test)]
mod tests {
    #[test]
    fn initials_take_the_first_letters_of_the_first_two_words() {
        assert_eq!(super::initials("simple-localize"), "SL");
        assert_eq!(super::initials("snout"), "SN");
        assert_eq!(super::initials("my_app server"), "MA");
        assert_eq!(super::initials(""), "?");
    }
}
