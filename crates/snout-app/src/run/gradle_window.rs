//! The Gradle tool window: the project's builds, then their projects, task groups and
//! tasks, as the engine lists them (`snout/gradleTasks`). A double click runs a task, in the
//! Build tool window. Sync reads the build model again; Refresh reads the tasks again.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, uniform_list,
};
use gpui_component::tooltip::Tooltip;

use super::{GradleBuild, Launch, Runs};
use crate::glyphs::{Icon, icon};
use crate::theme::palette;

#[derive(Clone, Debug, PartialEq)]
enum Row {
    Build { key: String, name: SharedString, error: Option<String>, open: bool },
    Project { key: String, name: SharedString, open: bool },
    Group { key: String, name: SharedString, open: bool },
    Task { build: String, path: String, name: SharedString, description: Option<String> },
}

pub struct GradleWindow {
    runs: Entity<Runs>,
    /// What is open, by key; builds start open, the rest closed.
    open: HashSet<String>,
    closed_builds: HashSet<String>,
    focus: FocusHandle,
}

impl GradleWindow {
    pub fn new(runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        Self { runs, open: HashSet::new(), closed_builds: HashSet::new(), focus: cx.focus_handle() }
    }

    /// Fold everything shut.
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.open.clear();
        let builds: Vec<String> = self.runs.read(cx).gradle.builds.iter().map(|b| format!("b:{}", b.build)).collect();
        self.closed_builds.extend(builds);
        cx.notify();
    }

    /// Open the way to every task named `name` (tests use it).
    pub fn reveal(&mut self, name: &str, cx: &mut Context<Self>) {
        for b in &self.runs.read(cx).gradle.builds {
            for p in &b.projects {
                for (g, tasks) in &p.groups {
                    if tasks.iter().any(|t| t.name == name) {
                        self.open.insert(format!("p:{}:{}", b.build, p.path));
                        self.open.insert(format!("g:{}:{}:{g}", b.build, p.path));
                    }
                }
            }
        }
        cx.notify();
    }

    fn rows(&self, builds: &[GradleBuild]) -> Vec<Row> {
        let mut out = Vec::new();
        let single = builds.len() == 1 && builds[0].projects.len() == 1;
        for b in builds {
            let bkey = format!("b:{}", b.build);
            let bopen = !self.closed_builds.contains(&bkey);
            out.push(Row::Build { key: bkey, name: b.build.clone().into(), error: b.error.clone(), open: bopen });
            if !bopen {
                continue;
            }
            for p in &b.projects {
                let pkey = format!("p:{}:{}", b.build, p.path);
                let popen = single || self.open.contains(&pkey);
                let name = if p.path == ":" { format!("{} (root)", p.name) } else { p.path.clone() };
                out.push(Row::Project { key: pkey, name: name.into(), open: popen });
                if !popen {
                    continue;
                }
                for (group, tasks) in &p.groups {
                    let gkey = format!("g:{}:{}:{group}", b.build, p.path);
                    let gopen = self.open.contains(&gkey);
                    out.push(Row::Group { key: gkey, name: group.clone().into(), open: gopen });
                    if gopen {
                        for t in tasks {
                            out.push(Row::Task {
                                build: b.build.clone(),
                                path: t.path.clone(),
                                name: t.name.clone().into(),
                                description: t.description.clone(),
                            });
                        }
                    }
                }
            }
        }
        out
    }

    fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if key.starts_with("b:") {
            if !self.closed_builds.remove(key) {
                self.closed_builds.insert(key.to_owned());
            }
        } else if !self.open.remove(key) {
            self.open.insert(key.to_owned());
        }
        cx.notify();
    }
}

crate::panels::panel!(GradleWindow, "Gradle", |_, _| "Gradle".into(), true);

impl Render for GradleWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let runs = self.runs.read(cx);
        let base = div().size_full().bg(p.ground).text_size(px(crate::look::BODY)).text_color(p.fg);
        use crate::components::{Button, EmptyState, IconName};
        use snout_core::art::spots;
        let note = |art: &'static snout_core::art::Drawing, title: &'static str, text: String, sync: bool| {
            EmptyState::new(art, title).hint(text).compact().when(sync, |e| {
                e.action(
                    Button::new("gradle-empty-sync")
                        .compact()
                        .icon(IconName::RefreshCw)
                        .label("Sync the Gradle Project")
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::SyncGradle), cx)),
                )
            })
        };
        if runs.gradle.builds.is_empty() {
            let ready = runs.engine(cx).is_some();
            return base.child(match (&runs.gradle.error, runs.gradle.loading, ready) {
                (_, true, _) => note(
                    snout_core::art::building(),
                    "Reading the Gradle tasks",
                    "The builds are being configured.".into(),
                    false,
                ),
                (Some(e), _, _) => note(spots::gradle_failed(), "The tasks could not be read", e.clone(), true),
                (None, _, false) => note(
                    snout_core::art::building(),
                    "No tasks yet",
                    "The tasks appear once the Java engine has imported the project.".into(),
                    false,
                ),
                (None, _, true) => {
                    note(spots::no_project(), "No Gradle tasks", "This project has no Gradle build.".into(), false)
                }
            });
        }
        let rows: Rc<Vec<Row>> = Rc::new(self.rows(&runs.gradle.builds));
        let count = rows.len();
        let this = cx.entity();
        let runs_entity = self.runs.clone();
        let hover = p.button_hover;
        base.child(
            uniform_list("gradle-tasks", count, move |range, _, _| {
                range
                    .map(|i| {
                        let row = rows[i].clone();
                        let (depth, chevron, glyph, label, tip) = match &row {
                            Row::Build { name, open, error, .. } => (
                                0,
                                Some(*open),
                                if error.is_some() { Icon::Warning } else { Icon::Build },
                                name.clone(),
                                error.clone(),
                            ),
                            Row::Project { name, open, .. } => (1, Some(*open), Icon::Module, name.clone(), None),
                            Row::Group { name, open, .. } => (2, Some(*open), Icon::Folder, name.clone(), None),
                            Row::Task { name, description, .. } => {
                                (3, None, Icon::Gear, name.clone(), description.clone())
                            }
                        };
                        let this = this.clone();
                        let runs = runs_entity.clone();
                        let ink = match (&row, glyph) {
                            (_, Icon::Warning) => p.warn,
                            (Row::Build { .. } | Row::Project { .. }, _) => p.accent,
                            _ => p.fg_dim,
                        };
                        div()
                            .id(("gradle-row", i))
                            .debug_selector(move || format!("gradle-row-{i}"))
                            .h(px(24.))
                            .pl(px(8. + 16. * depth as f32))
                            .pr(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover))
                            .child(div().flex_none().w(px(12.)).when_some(chevron, |d, open| {
                                d.child(icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 12., p.fg_dim))
                            }))
                            .child(icon(glyph, 14., ink))
                            .child(div().whitespace_nowrap().child(label))
                            .when_some(tip, |d, tip| {
                                let tip: SharedString = tip.into();
                                d.tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                            })
                            .on_click(move |e, _, cx| match &row {
                                Row::Build { key, .. } | Row::Project { key, .. } | Row::Group { key, .. } => {
                                    let key = key.clone();
                                    this.update(cx, |w, cx| w.toggle(&key, cx));
                                }
                                Row::Task { build, path, .. } => {
                                    if e.click_count() >= 2 {
                                        let launch = Launch::Task { build: Some(build.clone()), task: path.clone() };
                                        runs.update(cx, |r, cx| {
                                            r.start(launch, cx);
                                        });
                                    }
                                }
                            })
                    })
                    .collect()
            })
            .size_full()
            .py(px(4.)),
        )
    }
}

/// The Gradle window, for tests.
pub fn task_rows(w: &GradleWindow, cx: &App) -> Vec<String> {
    w.rows(&w.runs.read(cx).gradle.builds)
        .into_iter()
        .map(|r| match r {
            Row::Build { name, .. } | Row::Project { name, .. } | Row::Group { name, .. } => name.to_string(),
            Row::Task { path, .. } => path,
        })
        .collect()
}
