//! The right stripe's Notifications tool window (what went wrong lately), and the titles
//! the tool windows' headers show. The Gradle window is the run module's.

use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use gpui_component::scroll::ScrollableElement as _;

use crate::model::Model;
use crate::theme::palette;

/// The title a tool window's header shows, by the dock name of its panel.
pub fn title(panel_name: &str) -> &str {
    match panel_name {
        "Claude" => "Claude Code",
        "Side" => "Project",
        other => other,
    }
}

/// An empty tool window's note: its drawing, centred.
fn empty(title: &'static str, text: &'static str, _: &App) -> crate::components::EmptyState {
    crate::components::EmptyState::new(snout_core::art::spots::quiet_bell(), title).hint(text).compact()
}

pub struct NotificationsPanel {
    model: Entity<Model>,
    focus: FocusHandle,
}

impl NotificationsPanel {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, focus: cx.focus_handle() }
    }

    /// What it lists: the errors Git and the tools reported, newest first.
    pub fn items(&self, cx: &App) -> Vec<String> {
        self.model.read(cx).output.iter().rev().filter(|l| l.error).take(50).map(|l| l.text.clone()).collect()
    }
}

crate::panels::panel!(NotificationsPanel, "Notifications", |_, _| "Notifications".into(), true);

impl Render for NotificationsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let items = self.items(cx);
        let base = div().size_full().bg(p.ground).text_size(px(crate::look::BODY)).text_color(p.fg);
        if items.is_empty() {
            return base.child(empty("No notifications", "Errors from Git and the tools show here.", cx));
        }
        base.child(div().id("notifications").size_full().p(px(8.)).overflow_y_scrollbar().children(
            items.into_iter().map(|text| {
                div()
                    .mb(px(6.))
                    .p(px(8.))
                    .flex()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .bg(p.card)
                    .border_1()
                    .border_color(p.border)
                    .child(crate::components::icon(crate::components::IconName::OctagonAlert, 15., p.error))
                    .child(div().flex_1().min_w_0().text_size(px(12.5)).line_height(px(18.)).child(text))
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tool_windows_have_readable_titles() {
        assert_eq!(super::title("Claude"), "Claude Code");
        assert_eq!(super::title("Terminal"), "Terminal");
        assert_eq!(super::title("Side"), "Project");
    }
}
