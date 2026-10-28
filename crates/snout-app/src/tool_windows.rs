//! The right stripe's tool windows: Gradle (the build's projects and tasks, as the engine
//! imported them) and Notifications (what went wrong lately).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::gradle::Build;
use crate::lang::LangHub;
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

/// An empty tool window's note, centred.
fn empty(title: &'static str, text: &'static str, cx: &App) -> gpui::Div {
    let p = palette(cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .px(px(24.))
        .child(div().text_color(p.fg).child(title))
        .child(div().text_color(p.fg_dim).text_size(px(12.)).text_center().child(text))
}

pub struct GradlePanel {
    model: Entity<Model>,
    hub: Entity<LangHub>,
    root: Option<PathBuf>,
    loaded: Option<(SystemTime, Rc<Build>)>,
    looked: Option<Instant>,
    /// The projects folded shut, by Gradle path.
    pub closed: BTreeSet<String>,
    focus: FocusHandle,
}

impl GradlePanel {
    pub fn new(model: Entity<Model>, hub: Entity<LangHub>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        cx.observe(&hub, |_, _, cx| cx.notify()).detach();
        Self { model, hub, root: None, loaded: None, looked: None, closed: BTreeSet::new(), focus: cx.focus_handle() }
    }

    /// The Gradle build of the open project: one its server runs, or the project itself.
    pub fn root(&self, cx: &App) -> Option<PathBuf> {
        let repo = self.model.read(cx).project_root()?;
        let hub = self.hub.read(cx);
        let mut roots: Vec<&PathBuf> = hub.servers.keys().filter(|r| r.starts_with(&repo)).collect();
        roots.sort();
        roots.first().map(|r| (*r).clone()).or_else(|| crate::lang::java::is_gradle_root(&repo).then_some(repo))
    }

    /// The build as the engine last imported it, if it has. Looked for now and then: the
    /// engine writes its model when it imports the build.
    pub fn build(&mut self, cx: &App) -> Option<Rc<Build>> {
        let root = self.root(cx);
        if root != self.root {
            self.root = root.clone();
            self.loaded = None;
            self.looked = None;
        }
        let root = root?;
        if self.looked.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
            self.looked = Some(Instant::now());
            let home = self.hub.read(cx).home.clone();
            if let Some((build, when)) = crate::gradle::load(&home, &root)
                && self.loaded.as_ref().is_none_or(|(t, _)| *t != when)
            {
                self.loaded = Some((when, Rc::new(build)));
            }
        }
        self.loaded.as_ref().map(|(_, b)| b.clone())
    }

    /// Fold every project shut.
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        if let Some((_, build)) = &self.loaded {
            self.closed = build.projects.iter().map(|p| p.path.clone()).collect();
            cx.notify();
        }
    }
}

crate::panels::panel!(GradlePanel, "Gradle", |_, _| "Gradle".into(), true);

impl Render for GradlePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let root = self.root(cx);
        let build = self.build(cx);
        let base = div().size_full().bg(p.ground).text_size(px(crate::look::BODY)).text_color(p.fg);
        let Some(build) = build else {
            return base.child(match root {
                None => empty("No Gradle build", "Open a project with a Gradle build to see its tasks.", cx),
                Some(_) => {
                    empty("Not imported yet", "The tasks appear once the Java engine has imported the build.", cx)
                }
            });
        };
        let hover = p.button_hover;
        let mut rows = Vec::new();
        for (i, project) in build.projects.iter().enumerate() {
            let open = !self.closed.contains(&project.path);
            let path = project.path.clone();
            let name = if project.path == ":" { project.name.clone() } else { project.path.clone() };
            rows.push(
                div()
                    .id(("gradle-project", i))
                    .debug_selector(move || format!("gradle-project-{i}"))
                    .h(px(24.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 12., p.fg_dim))
                    .child(icon(Icon::Build, 14., p.accent))
                    .child(name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.closed.remove(&path) {
                            this.closed.insert(path.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            if open {
                for task in &project.tasks {
                    rows.push(
                        div()
                            .h(px(22.))
                            .pl(px(40.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(p.fg)
                            .child(icon(Icon::Gear, 12., p.fg_dim))
                            .child(task.clone())
                            .into_any_element(),
                    );
                }
            }
        }
        base.child(div().id("gradle-tasks").size_full().py(px(4.)).overflow_y_scrollbar().children(rows))
    }
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
                    .child(icon(Icon::Problems, 15., p.error))
                    .child(div().flex_1().min_w_0().text_size(px(12.5)).child(text))
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
