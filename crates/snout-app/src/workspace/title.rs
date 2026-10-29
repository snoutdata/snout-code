//! The title bar, drawn by the app across the top of the window, round the window buttons:
//! the project and branch widgets on the left; the run configuration, Run and Debug,
//! Search Everywhere and Settings on the right.

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
use crate::look::Trigger;
use crate::menus::{self, CloseProject, OpenRecent};
use crate::model::{Action, Model};
use crate::theme::{Palette, mix, palette};

/// The title bar's height.
pub const TITLE_H: f32 = 40.;

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
pub fn badge_color(name: &str, p: &Palette) -> gpui::Hsla {
    let hues = [0.58, 0.08, 0.36, 0.75, 0.95, 0.15, 0.5];
    let hash = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    let hue = hues[hash as usize % hues.len()];
    if p.tinted {
        // One of the palette's hues, deepened until its letters read (WCAG AA).
        return p.badge(p.hues[[0usize, 2, 1, 4, 3, 5, 7][hash as usize % 7]]);
    }
    p.badge(gpui::hsla(hue, 0.45, if p.dark { 0.42 } else { 0.5 }, 1.))
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

/// A title bar icon button in its own colour, with a tooltip made at run time.
fn tip_button(
    id: &'static str,
    g: Icon,
    tip: gpui::SharedString,
    enabled: bool,
    ink: gpui::Hsla,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let hover = p.button_hover;
    let ink = if enabled { ink } else { mix(p.fg_dim, 0.6, p.ground) };
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
        .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
        .child(icon(g, 16., ink))
}

impl Workspace {
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
        let m = self.model.read(cx);
        let name = m.repo.as_ref().and(m.project_root()).map(|r| Model::dir_name(&r));
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
                .text_color(p.on_badge)
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
                menu = menu.item(
                    PopupMenuItem::new("Open Project…")
                        .icon(crate::components::menu_icon(crate::components::IconName::FolderOpen))
                        .on_click(send(Action::PickFolder)),
                );
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
                        .icon(crate::components::menu_icon(crate::components::IconName::X))
                        .disabled(!has_project)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(CloseProject), cx)),
                )
            })
            .into_any_element()
        };

        // The remote machine the project is on: its name, a dot saying how the connection
        // is, and a menu to reconnect or close it.
        let remote_widget = crate::remote::connection(cx).map(|(host, status)| {
            let (text, dot) = crate::remote::describe_status(&host, &status, &p);
            let lost = matches!(status, snout_remote::Status::Lost(_));
            Trigger(
                bar_button("title-remote", &p)
                    .child(div().flex_none().size(px(7.)).rounded_full().bg(dot))
                    .child(icon(Icon::Terminal, 14., p.fg_dim))
                    .child(div().max_w(px(200.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(host))
                    .child(icon(Icon::ChevronDown, 12., p.fg_dim)),
            )
            .dropdown_menu(move |menu, _, _| {
                let menu = menu.item(PopupMenuItem::new(text.clone()).disabled(true)).separator();
                let menu = if lost {
                    menu.item(
                        PopupMenuItem::new("Reconnect")
                            .icon(crate::components::menu_icon(crate::components::IconName::RefreshCw))
                            .on_click(|_, _, cx| crate::remote::reconnect(cx)),
                    )
                } else {
                    menu
                };
                menu.item(
                    PopupMenuItem::new("Close Remote Connection")
                        .icon(crate::components::menu_icon(crate::components::IconName::Unplug))
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(menus::CloseRemote), cx)),
                )
            })
            .into_any_element()
        });

        // The branch: its name, and the popup of branches and repository actions.
        let branch_widget = branch.map(|b| {
            div()
                .relative()
                .flex_none()
                .child(
                    bar_button("title-branch", &p)
                        .when(p.tinted, |d| {
                            d.h(px(24.))
                                .bg(p.wash(p.branch()))
                                .border_1()
                                .border_color(mix(p.branch(), 0.35, p.ground))
                                .rounded(px(12.))
                                .text_color(if p.dark { p.branch() } else { p.fg })
                        })
                        .child(icon(Icon::Branch, 15., if p.tinted { p.branch() } else { p.fg_dim }))
                        .child(div().max_w(px(240.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(b))
                        .child(icon(Icon::ChevronDown, 12., if p.tinted { p.branch() } else { p.fg_dim }))
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_branches(window, cx))),
                )
                .children(self.branch_popup.clone().map(|popup| {
                    div().absolute().top(px(32.)).left_0().child(
                        gpui::deferred(gpui::anchored().snap_to_window_with_margin(px(8.)).child(popup))
                            .with_priority(2),
                    )
                }))
        });

        // The run configuration: the current file, or one of the project's; Run and Debug
        // start it, Stop ends what runs.
        let runs = self.runs.read(cx);
        let configs = runs.configurations();
        let selected = runs.selected_config();
        let ready = runs.engine(cx).is_some();
        let loading = runs.configs_loading;
        let running = runs.any_running();
        // The selected configuration runs: Run becomes Rerun.
        let rerun = selected.as_ref().and_then(|c| runs.running_config(&c.id)).is_some_and(|s| !s.launch.debug());
        let runs_entity = self.runs.clone();
        let run_config = {
            let label = selected.as_ref().map_or_else(|| "Current File".to_owned(), |c| c.name.clone());
            let selected_id = selected.as_ref().map(|c| c.id.clone());
            let kind_icon = selected.as_ref().map(|c| match c.kind {
                crate::run::config::Kind::Main if c.spring_boot => Icon::Leaf,
                crate::run::config::Kind::Main => Icon::Play,
                crate::run::config::Kind::Gradle => Icon::Build,
                crate::run::config::Kind::Tests => Icon::Check,
            });
            Trigger(
                bar_button("title-run-config", &p)
                    .children(kind_icon.map(|g| icon(g, 13., p.ok)))
                    .child(div().max_w(px(220.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(label))
                    .child(icon(Icon::ChevronDown, 12., p.fg_dim)),
            )
            .dropdown_menu(move |mut menu, _, _| {
                let pick = |id: Option<String>| {
                    let runs = runs_entity.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        runs.update(cx, |r, cx| r.select_config(id.clone(), cx))
                    }
                };
                menu =
                    menu.item(PopupMenuItem::new("Current File").checked(selected_id.is_none()).on_click(pick(None)));
                let mut group = "";
                for c in &configs {
                    let g = if c.user && c.id.starts_with("user:") { "Yours" } else { c.group() };
                    if g != group {
                        group = g;
                        menu = menu.separator().label(g);
                    }
                    let on = selected_id.as_deref() == Some(c.id.as_str());
                    menu = menu.item(PopupMenuItem::new(c.name.clone()).checked(on).on_click(pick(Some(c.id.clone()))));
                }
                if configs.is_empty() {
                    let note = if loading {
                        "Reading the configurations…"
                    } else {
                        "Configurations appear once the Java engine is ready"
                    };
                    menu = menu.separator().item(PopupMenuItem::new(note).disabled(true));
                }
                menu.separator().item(
                    PopupMenuItem::new("Edit Configurations…")
                        .icon(crate::components::menu_icon(crate::components::IconName::Settings))
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(menus::EditConfigurations), cx)),
                )
            })
            .into_any_element()
        };
        let what = selected.as_ref().map_or_else(|| "the Current File".to_owned(), |c| format!("'{}'", c.name));
        let run_tip: gpui::SharedString =
            if rerun { format!("Rerun {what} (⌃R)") } else { format!("Run {what} (⌃R)") }.into();
        let debug_tip: gpui::SharedString = format!("Debug {what} (⌃D)").into();
        let run_icon = if rerun { Icon::Rerun } else { Icon::Play };
        let run_button = tip_button("title-run", run_icon, run_tip, ready, p.ok, &p)
            .when(ready, |d| d.on_click(|_, w, cx| w.dispatch_action(Box::new(menus::RunSelected), cx)));
        let debug_button =
            tip_button("title-debug", Icon::Bug, debug_tip, ready, if p.tinted { p.hues[3] } else { p.ok }, &p)
                .when(ready, |d| d.on_click(|_, w, cx| w.dispatch_action(Box::new(menus::DebugSelected), cx)));
        let stop_button = running.then(|| {
            tip_button("title-stop", Icon::Stop, "Stop (⌘F2)".into(), true, p.error, &p)
                .on_click(|_, w, cx| w.dispatch_action(Box::new(menus::StopRuns), cx))
        });

        let right = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(run_config)
            .child(run_button)
            .child(debug_button)
            .children(stop_button)
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
            .bg(p.title_bg)
            .when_some(p.title_wash(), |d, tint| {
                // The accent washes in from the left, fading into the bar.
                d.bg(gpui::linear_gradient(
                    90.,
                    gpui::linear_color_stop(tint, 0.),
                    gpui::linear_color_stop(p.title_bg, 0.35),
                ))
            })
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
                    .children(remote_widget)
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
