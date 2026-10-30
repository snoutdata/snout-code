//! The Maven tool window: the project's Maven profiles, which can be turned on and off, then
//! each Maven build, its modules, and per module the lifecycle phases and the plugins with
//! their goals, as the engine lists them (`snout/mavenGoals`). A double click runs a phase or
//! a goal in that module, in the Build tool window, with the profiles chosen and, when the
//! toggles above the list say so, offline and without tests.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, uniform_list,
};
use gpui_component::tooltip::Tooltip;

use super::{Launch, MavenBuild, Runs};
use crate::glyphs::{Icon, icon};
use crate::theme::palette;

#[derive(Clone, Debug, PartialEq)]
enum Row {
    Section {
        key: String,
        name: SharedString,
        count: usize,
        open: bool,
    },
    Profile {
        id: String,
        active: bool,
        on: bool,
    },
    Build {
        key: String,
        name: SharedString,
        note: SharedString,
        error: Option<String>,
        open: bool,
    },
    Module {
        key: String,
        name: SharedString,
        note: SharedString,
        open: bool,
    },
    Group {
        key: String,
        name: SharedString,
        lifecycle: bool,
        open: bool,
    },
    Plugin {
        key: String,
        name: SharedString,
        version: Option<String>,
        open: bool,
    },
    Goal {
        build: String,
        project: String,
        module: String,
        goals: String,
        name: SharedString,
        tip: Option<String>,
        phase: bool,
    },
}

pub struct MavenWindow {
    runs: Entity<Runs>,
    /// What is open, by key; builds and the profiles start open, the rest closed.
    open: HashSet<String>,
    closed: HashSet<String>,
    focus: FocusHandle,
}

impl MavenWindow {
    pub fn new(runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        Self { runs, open: HashSet::new(), closed: HashSet::new(), focus: cx.focus_handle() }
    }

    /// Fold everything shut.
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.open.clear();
        let keys: Vec<String> = self.runs.read(cx).maven.builds.iter().map(|b| format!("b:{}", b.build)).collect();
        self.closed.extend(keys);
        self.closed.insert("profiles".into());
        cx.notify();
    }

    /// Open the way to every goal or phase called `name` (tests and the docs use it).
    pub fn reveal(&mut self, name: &str, cx: &mut Context<Self>) {
        for b in &self.runs.read(cx).maven.builds {
            for m in &b.modules {
                let mkey = format!("m:{}:{}", b.build, m.project);
                if m.lifecycle.iter().any(|p| p == name) {
                    for k in [mkey.clone(), format!("{mkey}:lifecycle")] {
                        self.closed.remove(&k);
                        self.open.insert(k);
                    }
                }
                for p in &m.plugins {
                    if p.goals.iter().any(|(g, _)| g == name || format!("{}:{g}", p.prefix) == name) {
                        for k in [mkey.clone(), format!("{mkey}:plugins"), format!("{mkey}:p:{}", p.artifact)] {
                            self.closed.remove(&k);
                            self.open.insert(k);
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    fn rows(&self, builds: &[MavenBuild], runs: &Runs) -> Vec<Row> {
        let mut out = Vec::new();
        let mut profiles: Vec<(String, bool)> = Vec::new();
        for b in builds {
            for (id, active, _) in &b.profiles {
                if !profiles.iter().any(|(p, _)| p == id) {
                    profiles.push((id.clone(), *active));
                }
            }
        }
        if !profiles.is_empty() {
            let open = self.is_open("profiles", true);
            out.push(Row::Section { key: "profiles".into(), name: "Profiles".into(), count: profiles.len(), open });
            if open {
                for (id, active) in profiles {
                    let on = runs.project.maven.profile_on(&id, active);
                    out.push(Row::Profile { id, active, on });
                }
            }
        }
        let single = builds.len() == 1 && builds[0].modules.len() == 1;
        for b in builds {
            let bkey = format!("b:{}", b.build);
            let bopen = self.is_open(&bkey, true);
            let note =
                if b.wrapper { format!("Maven {} (wrapper)", b.version) } else { format!("Maven {}", b.version) };
            out.push(Row::Build {
                key: bkey,
                name: b.build.clone().into(),
                note: note.into(),
                error: b.error.clone(),
                open: bopen,
            });
            if !bopen {
                continue;
            }
            for m in &b.modules {
                let mkey = format!("m:{}:{}", b.build, m.project);
                let mopen = self.is_open(&mkey, single);
                let name = if m.project == ":" { format!("{} (root)", m.name) } else { m.name.clone() };
                out.push(Row::Module {
                    key: mkey.clone(),
                    name: name.into(),
                    note: m.packaging.clone().into(),
                    open: mopen,
                });
                if !mopen {
                    continue;
                }
                let lkey = format!("{mkey}:lifecycle");
                let lopen = self.is_open(&lkey, single);
                out.push(Row::Group { key: lkey, name: "Lifecycle".into(), lifecycle: true, open: lopen });
                if lopen {
                    for phase in &m.lifecycle {
                        out.push(Row::Goal {
                            build: b.build.clone(),
                            project: m.project.clone(),
                            module: m.name.clone(),
                            goals: phase.clone(),
                            name: phase.clone().into(),
                            tip: phase_tip(phase).map(str::to_owned),
                            phase: true,
                        });
                    }
                }
                let pkey = format!("{mkey}:plugins");
                let popen = self.is_open(&pkey, false);
                out.push(Row::Group { key: pkey, name: "Plugins".into(), lifecycle: false, open: popen });
                if !popen {
                    continue;
                }
                for p in &m.plugins {
                    let key = format!("{mkey}:p:{}", p.artifact);
                    let open = self.is_open(&key, false);
                    out.push(Row::Plugin { key, name: p.prefix.clone().into(), version: p.version.clone(), open });
                    if open {
                        for (goal, description) in &p.goals {
                            out.push(Row::Goal {
                                build: b.build.clone(),
                                project: m.project.clone(),
                                module: m.name.clone(),
                                goals: format!("{}:{goal}", p.prefix),
                                name: format!("{}:{goal}", p.prefix).into(),
                                tip: description.clone(),
                                phase: false,
                            });
                        }
                    }
                }
            }
        }
        out
    }

    /// Whether `key` is open: as the user left it, else `default`.
    fn is_open(&self, key: &str, default: bool) -> bool {
        if self.open.contains(key) {
            true
        } else if self.closed.contains(key) {
            false
        } else {
            default
        }
    }

    /// Fold `key` the other way; it shows `open` now.
    fn toggle(&mut self, key: &str, open: bool, cx: &mut Context<Self>) {
        if open {
            self.open.remove(key);
            self.closed.insert(key.to_owned());
        } else {
            self.closed.remove(key);
            self.open.insert(key.to_owned());
        }
        cx.notify();
    }
}

/// What a lifecycle phase does, for its tooltip.
fn phase_tip(phase: &str) -> Option<&'static str> {
    Some(match phase {
        "clean" => "Delete the build output (target)",
        "validate" => "Check the project is correct and complete",
        "compile" => "Compile the main sources",
        "test" => "Run the unit tests",
        "package" => "Package the compiled code (a jar or war)",
        "verify" => "Run the checks on the package, integration tests among them",
        "install" => "Put the package in the local repository",
        "site" => "Make the project's site",
        "deploy" => "Copy the package to the remote repository",
        _ => return None,
    })
}

crate::panels::panel!(MavenWindow, "Maven", |_, _| "Maven".into(), true);

impl Render for MavenWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let runs = self.runs.read(cx);
        let base = div().size_full().flex().flex_col().bg(p.ground).text_size(px(crate::look::BODY)).text_color(p.fg);
        use crate::components::{EmptyState, IconName};
        use snout_core::art::spots;
        let note = |art: &'static snout_core::art::Drawing, title: &'static str, text: String, sync: bool| {
            EmptyState::new(art, title).hint(text).compact().when(sync, |e| {
                e.primary_action(
                    "maven-empty-sync",
                    IconName::RefreshCw,
                    "Reload the Maven Project",
                    crate::menus::SyncGradle,
                )
            })
        };
        if runs.maven.builds.is_empty() {
            let ready = runs.engine(cx).is_some();
            return base.child(match (&runs.maven.error, runs.maven.loading, ready) {
                (_, true, _) => {
                    note(spots::maven(), "Reading the Maven goals", "The plugins' goals are being read.".into(), false)
                }
                (Some(e), _, _) => note(spots::gradle_failed(), "The goals could not be read", e.clone(), true),
                (None, _, false) => note(
                    spots::maven(),
                    "No goals yet",
                    "The goals appear once the Java engine has imported the project.".into(),
                    true,
                ),
                (None, _, true) => note(spots::maven(), "No Maven builds", "This project has no pom.xml.".into(), true),
            });
        }
        let choices = runs.project.maven.clone();
        let rows: Rc<Vec<Row>> = Rc::new(self.rows(&runs.maven.builds, runs));
        let count = rows.len();
        let this = cx.entity();
        let runs_entity = self.runs.clone();
        let (offline_runs, skip_runs) = (self.runs.clone(), self.runs.clone());
        let toolbar = div()
            .flex_none()
            .px(px(8.))
            .pt(px(6.))
            .pb(px(4.))
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                crate::look::toggle("maven-offline", IconName::WifiOff, "Offline", choices.offline, &p)
                    .debug_selector(|| "maven-offline".into())
                    .tooltip(|w, cx| Tooltip::new("Run Maven offline (-o): only the local repository").build(w, cx))
                    .on_click(move |_, _, cx| {
                        offline_runs.update(cx, |r, cx| {
                            let on = !r.project.maven.offline;
                            r.set_maven_offline(on, cx)
                        })
                    }),
            )
            .child(
                crate::look::toggle(
                    "maven-skip-tests",
                    IconName::FlaskConicalOff,
                    "Skip Tests",
                    choices.skip_tests,
                    &p,
                )
                .debug_selector(|| "maven-skip-tests".into())
                .tooltip(|w, cx| Tooltip::new("Run goals without the tests (-DskipTests)").build(w, cx))
                .on_click(move |_, _, cx| {
                    skip_runs.update(cx, |r, cx| {
                        let on = !r.project.maven.skip_tests;
                        r.set_maven_skip_tests(on, cx)
                    })
                }),
            );
        base.child(toolbar).child(
            uniform_list("maven-goals", count, move |range, _, _| {
                range
                    .map(|i| {
                        let row = rows[i].clone();
                        let (depth, chevron, glyph, label, note, tip) = match &row {
                            Row::Section { name, count, open, .. } => {
                                (0, Some(*open), Icon::Gear, name.clone(), Some(count.to_string().into()), None)
                            }
                            Row::Profile { id, active, .. } => (
                                1,
                                None,
                                Icon::Check,
                                id.clone().into(),
                                active.then(|| SharedString::from("active in the POM")),
                                None,
                            ),
                            Row::Build { name, note, open, error, .. } => (
                                0,
                                Some(*open),
                                if error.is_some() { Icon::Warning } else { Icon::Feather },
                                name.clone(),
                                Some(note.clone()),
                                error.clone(),
                            ),
                            Row::Module { name, note, open, .. } => {
                                (1, Some(*open), Icon::Module, name.clone(), Some(note.clone()), None)
                            }
                            Row::Group { name, open, lifecycle, .. } => (
                                2,
                                Some(*open),
                                if *lifecycle { Icon::Rerun } else { Icon::Tools },
                                name.clone(),
                                None,
                                None,
                            ),
                            Row::Plugin { name, version, open, .. } => {
                                (3, Some(*open), Icon::Jar, name.clone(), version.clone().map(SharedString::from), None)
                            }
                            Row::Goal { name, tip, phase, .. } => {
                                (if *phase { 3 } else { 4 }, None, Icon::Gear, name.clone(), None, tip.clone())
                            }
                        };
                        let this = this.clone();
                        let runs = runs_entity.clone();
                        let ink = match (&row, glyph) {
                            (_, Icon::Warning) => p.warn,
                            (Row::Build { .. } | Row::Module { .. }, _) => p.accent,
                            (Row::Plugin { .. }, _) => p.hue(5),
                            _ => p.fg_dim,
                        };
                        let check = match &row {
                            Row::Profile { on, .. } => Some(*on),
                            _ => None,
                        };
                        crate::components::list_row_h(
                            ("maven-row", i),
                            crate::components::RowState::default(),
                            crate::components::TREE_ROW_H,
                            &p,
                        )
                        .debug_selector(move || format!("maven-row-{i}"))
                        .pl(px(8. + 16. * depth as f32))
                        .pr(px(8.))
                        .gap(px(5.))
                        .cursor_pointer()
                        .child(div().flex_none().w(px(12.)).when_some(chevron, |d, open| {
                            d.child(icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 12., p.fg_dim))
                        }))
                        .map(|d| match check {
                            Some(on) => d.child(
                                div()
                                    .flex_none()
                                    .size(px(14.))
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_color(if on { p.accent } else { p.border })
                                    .when(on, |d| d.bg(p.accent))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(on, |d| d.child(icon(Icon::Check, 11., p.on_accent))),
                            ),
                            None => d.child(icon(glyph, 14., ink)),
                        })
                        .child(div().whitespace_nowrap().child(label))
                        .when_some(note, |d, n| {
                            d.child(div().whitespace_nowrap().text_color(p.fg_dim).text_size(px(11.5)).child(n))
                        })
                        .when_some(tip, |d, tip| {
                            let tip: SharedString = tip.into();
                            d.tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                        })
                        .on_click(move |e, _, cx| match &row {
                            Row::Section { key, open, .. }
                            | Row::Build { key, open, .. }
                            | Row::Module { key, open, .. }
                            | Row::Group { key, open, .. }
                            | Row::Plugin { key, open, .. } => {
                                let (key, open) = (key.clone(), *open);
                                this.update(cx, |w, cx| w.toggle(&key, open, cx));
                            }
                            Row::Profile { id, active, .. } => {
                                let (id, active) = (id.clone(), *active);
                                runs.update(cx, |r, cx| r.toggle_maven_profile(&id, active, cx));
                            }
                            Row::Goal { build, project, module, goals, .. } => {
                                if e.click_count() >= 2 {
                                    let launch = Launch::Maven {
                                        build: build.clone(),
                                        project: project.clone(),
                                        module: module.clone(),
                                        goals: goals.clone(),
                                    };
                                    runs.update(cx, |r, cx| {
                                        r.start(launch, cx);
                                    });
                                }
                            }
                        })
                    })
                    .collect()
            })
            .flex_1()
            .w_full()
            .py(px(4.)),
        )
    }
}

/// The Maven window's rows, for tests.
pub fn goal_rows(w: &MavenWindow, cx: &App) -> Vec<String> {
    let runs = w.runs.read(cx);
    w.rows(&runs.maven.builds, runs)
        .into_iter()
        .map(|r| match r {
            Row::Section { name, .. }
            | Row::Build { name, .. }
            | Row::Module { name, .. }
            | Row::Group { name, .. }
            | Row::Plugin { name, .. } => name.to_string(),
            Row::Profile { id, on, .. } => format!("[{}] {id}", if on { "x" } else { " " }),
            Row::Goal { goals, .. } => goals,
        })
        .collect()
}
