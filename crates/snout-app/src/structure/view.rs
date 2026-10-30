//! How the Project Structure window draws: the tree of pages, each page, and the foot.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, uniform_list,
};
use gpui_component::scroll::ScrollableElement as _;

use super::data::{Edge, ModuleInfo, Node, NodeKind, Scope, SourceSet, Structure};
use super::{Load, ModuleSel, Page, ProjectStructureView, StructureEvent, Target};
use crate::components::{Button, EmptyState, IconButton, IconName, RowState, Tone, badge, badge_with_icon, icon};
use crate::glyphs::Icon;
use crate::look;
use crate::model::Model;
use crate::settings_view::{choice, row, section};
use crate::theme::{Palette, mix, palette};

/// `path` as `~/…` under `home`, else whole.
fn short(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home).ok().filter(|_| !home.as_os_str().is_empty()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `path` inside `dir` when it is there, else as [`short`] has it.
fn inside(path: &Path, dir: &Path, home: &Path) -> String {
    match path.strip_prefix(dir) {
        Ok(rest) if !rest.as_os_str().is_empty() => rest.display().to_string(),
        _ => short(path, home),
    }
}

/// A value Gradle owns, read only: a lock before it.
fn gradle_value(text: impl Into<SharedString>, p: &Palette) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .min_w_0()
        .child(icon(IconName::Lock, 12., p.fg_dim))
        .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(text.into()))
        .into_any_element()
}

/// "Edit in build file": a small ghost button.
fn edit_button(id: impl Into<SharedString>) -> Button {
    Button::new(id, IconName::SquarePen).compact().ghost().label("Edit in build file")
}

/// A tile with `ico` in a wash of `color`, as the pages' headers and cards draw them.
fn tile(ico: impl Into<crate::components::Ico>, color: Hsla, size: f32, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(size / 3.5))
        .flex()
        .items_center()
        .justify_center()
        .bg(p.wash(color))
        .border_1()
        .border_color(mix(color, 0.3, p.card))
        .child(icon(ico, size * 0.55, color))
        .into_any_element()
}

/// A card of a page: a rounded box on the card's ground.
fn card(p: &Palette) -> gpui::Div {
    div().flex().flex_col().gap(px(4.)).p(px(12.)).rounded(px(10.)).border_1().border_color(p.border).bg(p.panel)
}

/// A dim line under a row.
fn hint(text: impl Into<SharedString>, p: &Palette) -> gpui::Div {
    div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(text.into())
}

/// One row of the Dependencies page's tree.
#[derive(Clone, Debug)]
struct DepRow {
    /// The nodes from the declared dependency down to this one (empty for an unresolved one).
    key: Vec<usize>,
    depth: usize,
    node: Option<usize>,
    unresolved: Option<(String, String)>,
    requested: Option<String>,
    declared_in: Option<String>,
    has_children: bool,
    open: bool,
    /// Matched by the filter (or a conflict, with conflicts only).
    hit: bool,
}

impl ProjectStructureView {
    fn home(&self, cx: &App) -> PathBuf {
        crate::host::host(cx).info().home.clone()
    }

    // ----- The tree of pages ----------------------------------------------------------

    fn tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let query = self.search.read(cx).value().to_string();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut group = None;
        for page in Page::ALL.into_iter().filter(|pg| pg.matches(&query)) {
            if Some(page.group()) != group {
                group = Some(page.group());
                rows.push(
                    div()
                        .h(px(26.))
                        .px(px(10.))
                        .mt(px(if rows.is_empty() { 0. } else { 6. }))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .font_weight(FontWeight::MEDIUM)
                        .child(crate::glyphs::icon(Icon::ChevronDown, 11., p.fg_dim))
                        .child(page.group())
                        .into_any_element(),
                );
            }
            let on = page == self.page;
            let id = format!("structure-page-{}", page.title().to_lowercase());
            let sel = id.clone();
            rows.push(
                crate::components::list_row_h(
                    SharedString::from(id),
                    RowState::current(on),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || sel.clone())
                .pl(px(26.))
                .pr(px(8.))
                .rounded(px(4.))
                .cursor_pointer()
                .gap(px(8.))
                .child(icon(page.icon(), 14., if on { p.accent } else { p.fg_dim }))
                .child(page.title())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.page = page;
                    cx.notify();
                }))
                .into_any_element(),
            );
        }
        if rows.is_empty() {
            rows.push(div().px(px(12.)).text_color(p.fg_dim).child("Nothing matches").into_any_element());
        }
        div()
            .w(px(210.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(8.))
            .bg(p.ground)
            .border_r_1()
            .border_color(p.border)
            .child(look::field(&self.search, true, None, &p))
            .child(div().id("structure-tree").flex_1().min_h_0().overflow_y_scrollbar().children(rows))
            .into_any_element()
    }

    // ----- What a page shows without a model ----------------------------------------------

    /// The empty state of a page that needs the model, when there is none to show.
    fn without_model(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = palette(cx);
        let sync = || Button::new("structure-sync-empty", IconName::RefreshCw).label("Sync Gradle").primary();
        let state = match &self.load {
            Load::Ready(_) => return None,
            Load::Loading => EmptyState::new(snout_core::art::building(), "Reading the Gradle model")
                .hint("The pages fill in as soon as the last sync's model is read."),
            Load::Missing => EmptyState::new(snout_core::art::spots::no_project(), "No Gradle model yet")
                .hint(
                    "This project has not been synced as a Gradle build. Sync it to see its modules and dependencies.",
                )
                .action(sync().on_click(cx.listener(|this, _, _, cx| this.sync(cx)))),
            Load::Failed(e) => {
                EmptyState::new(snout_core::art::spots::gradle_failed(), "The Gradle model could not be read")
                    .hint(e.clone())
                    .action(sync().on_click(cx.listener(|this, _, _, cx| this.sync(cx))))
            }
        };
        Some(div().h(px(420.)).child(state.on(p.card)).into_any_element())
    }

    // ----- Project ------------------------------------------------------------------------

    fn project_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let home = self.home(cx);
        let s = self.structure();
        let name = s
            .as_ref()
            .map(|s| s.name.clone())
            .or_else(|| self.root.as_deref().map(Model::dir_name))
            .unwrap_or_default();
        let root_module = s.as_ref().and_then(|s| s.modules.first().map(|m| m.id.clone()));
        let mut body = div().flex().flex_col();
        body = body.child(section("Project", &p)).child(row(
            "Name",
            div().flex().items_center().gap(px(8.)).child(gradle_value(name, &p)).when_some(
                root_module.clone(),
                |d, id| {
                    d.child(edit_button("structure-edit-name").on_click(
                        cx.listener(move |this, _, _, cx| this.edit_in_build_file(id.clone(), Target::Include, cx)),
                    ))
                },
            ),
            &p,
        ));
        if let Some(root) = &self.root {
            body = body.child(row(
                "Location",
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(icon(IconName::FolderOpen, 13., p.fg_dim))
                    .child(short(root, &home)),
                &p,
            ));
        }
        // The JDK: SnoutCode's, for this project.
        let current = self.project.jdk.as_ref().map(|h| h.display().to_string()).unwrap_or_default();
        let mut choices = vec![(String::new(), "Each build's own (your settings)".to_owned())];
        let jdks = self.all_jdks();
        choices.extend(jdks.iter().map(|j| (j.home.display().to_string(), format!("JDK {} ({})", j.major, j.version))));
        if let Some(h) = &self.project.jdk
            && !jdks.iter().any(|j| &j.home == h)
        {
            choices.push((h.display().to_string(), short(h, &home)));
        }
        let view = cx.entity();
        let set = self.project.jdk.is_some();
        body = body
            .child(section("Java", &p))
            .child(row(
                "JDK",
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(choice(
                        "structure-jdk",
                        &current,
                        choices,
                        move |v, cx| {
                            view.update(cx, |this, cx| this.set_jdk((!v.is_empty()).then(|| PathBuf::from(&v)), cx))
                        },
                        &p,
                    ))
                    .when(set, |d| {
                        d.child(badge_with_icon("Set for this project", IconName::FolderCog, Tone::Accent, &p))
                    })
                    .when(self.detected.is_none(), |d| d.child(look::weak("Looking for JDKs…", &p))),
                &p,
            ))
            .child(hint(
                "Gradle and the Java engine run on it. Kept in .snout/settings.json; the engine restarts on it \
                 the next time it starts.",
                &p,
            ));
        let Some(s) = s else {
            return body.children(self.without_model(cx)).into_any_element();
        };
        // The Java level, as Gradle sets it.
        let mut levels: Vec<String> = s.jvm_modules().filter_map(|m| m.level().map(str::to_owned)).collect();
        levels.sort_by_key(|l| crate::lang::java::major_of(l).unwrap_or(0));
        levels.dedup();
        let level = match levels.as_slice() {
            [] => "Not set (the JDK's own)".to_owned(),
            [one] => format!("Java {one}"),
            [lo, .., hi] => format!("Java {lo} to {hi}, per module"),
        };
        let first_jvm = s.jvm_modules().next().map(|m| m.id.clone());
        body = body.child(row(
            "Language level",
            div().flex().items_center().gap(px(8.)).child(gradle_value(level, &p)).when_some(first_jvm, |d, id| {
                d.child(edit_button("structure-edit-level").on_click(
                    cx.listener(move |this, _, _, cx| this.edit_in_build_file(id.clone(), Target::Level, cx)),
                ))
            }),
            &p,
        ));
        // Each build's Gradle or Maven, and the JVM it ran on.
        let tools = match (s.builds.iter().any(|b| b.maven), s.builds.iter().any(|b| !b.maven)) {
            (true, true) => "Gradle and Maven",
            (true, false) => "Maven",
            _ => "Gradle",
        };
        body = body.child(section(tools, &p));
        for (i, b) in s.builds.iter().enumerate() {
            let module = s.modules.iter().find(|m| m.build == b.id && m.included.is_none() && m.gradle_path == ":");
            let tool = if b.maven { "Maven" } else { "Gradle" };
            let version = b.gradle_version.clone().map(|v| format!("{tool} {v}")).unwrap_or_else(|| "Unknown".into());
            let several = s.builds.len() > 1;
            body = body
                .when(several, |d| {
                    d.child(
                        div()
                            .pt(px(6.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(crate::glyphs::icon(
                                if b.maven { Icon::Feather } else { Icon::Build },
                                13.,
                                p.fg_dim,
                            ))
                            .child(div().font_weight(FontWeight::MEDIUM).child(b.id.clone()))
                            .when(!b.synced, |d| {
                                d.child(badge_with_icon("Did not sync", IconName::TriangleAlert, Tone::Danger, &p))
                            }),
                    )
                })
                .child(row(
                    if b.maven { "Maven version" } else { "Gradle version" },
                    div().flex().items_center().gap(px(8.)).child(gradle_value(version, &p)).when_some(
                        module.map(|m| m.id.clone()),
                        |d, id| {
                            d.child(edit_button(SharedString::from(format!("structure-edit-gradle-{i}"))).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.edit_in_build_file(id.clone(), Target::GradleVersion, cx)
                                }),
                            ))
                        },
                    ),
                    &p,
                ))
                .child(row(
                    if b.maven { "Maven JVM" } else { "Gradle JVM" },
                    match (&b.java_version, &b.gradle_java_home) {
                        (Some(v), Some(h)) => div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .min_w_0()
                            .child(crate::glyphs::icon(Icon::Jdk, 14., p.lanes()[1]))
                            .child(format!("JDK {v}"))
                            .child(look::weak(short(h, &home), &p)),
                        _ => div().text_color(p.fg_dim).child("Not known until the build syncs"),
                    },
                    &p,
                ));
        }
        body = body.child(hint(
            if tools == "Maven" {
                "The JDK Maven runs on and compiles with, chosen for the build's Java level: the JDK above when one is set."
            } else {
                "The JVM the last sync ran Gradle on: the JDK above when one is set."
            },
            &p,
        ));
        body.child(div().pt(px(14.)).child(crate::components::callout(
            Tone::Info,
            if tools == "Maven" {
                "Values with a lock are Maven's: Edit in build file opens the pom.xml line that sets them. Once \
                     you save it, the bar over the editor offers to reload."
            } else {
                "Values with a lock are the build's: Edit in build file opens the line that sets them. Once you \
                     save it, the bar over the editor offers to sync."
            },
            &p,
        )))
        .into_any_element()
    }

    // ----- Modules ------------------------------------------------------------------------

    fn modules_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(e) = self.without_model(cx) {
            return e;
        }
        let Some(s) = self.structure() else { return div().into_any_element() };
        let p = palette(cx);
        let [green, _, blue, _, violet, _] = p.lanes();
        // The tree: modules, and under each its source sets.
        let mut rows: Vec<ModuleSel> = Vec::new();
        for m in &s.modules {
            rows.push(ModuleSel::Module(m.id.clone()));
            if !self.folded.contains(&m.id) {
                rows.extend(m.source_sets.iter().map(|set| ModuleSel::SourceSet(m.id.clone(), set.name.clone())));
            }
        }
        let view = cx.entity();
        let selected = self.selected.clone();
        let folded = self.folded.clone();
        let tree_s = s.clone();
        let count = rows.len();
        let list = uniform_list("structure-modules", count, move |range, _, cx| {
            let p = palette(cx);
            range
                .map(|i| {
                    let r = rows[i].clone();
                    let on = selected.as_ref() == Some(&r);
                    let (m, set) = match &r {
                        ModuleSel::Module(id) => (tree_s.module(id), None),
                        ModuleSel::SourceSet(id, n) => {
                            let m = tree_s.module(id);
                            (m, m.and_then(|m| m.source_sets.iter().find(|s| &s.name == n)))
                        }
                    };
                    let Some(m) = m else { return div().id(("structure-module-row", i)).into_any_element() };
                    let label = set.map(|s| s.name.clone()).unwrap_or_else(|| m.name.clone());
                    let sel = format!(
                        "structure-module-{}",
                        if let Some(s) = set { format!("{}-{}", m.name, s.name) } else { m.name.clone() }
                    );
                    let depth = m.depth + usize::from(set.is_some());
                    let has_sets = set.is_none() && !m.source_sets.is_empty();
                    let open = !folded.contains(&m.id);
                    let (fold_view, click_view) = (view.clone(), view.clone());
                    let id = m.id.clone();
                    let glyph = match set {
                        Some(s) if s.test => icon(Icon::TestFolder, 14., green),
                        Some(_) => icon(Icon::SourceFolder, 14., blue),
                        None if m.kind != super::data::BuildKind::Root && m.gradle_path == ":" => {
                            icon(Icon::Module, 15., violet)
                        }
                        None if m.is_jvm() => icon(Icon::Module, 15., blue),
                        None => icon(Icon::Module, 15., p.fg_dim),
                    };
                    crate::components::list_row_h(
                        ("structure-module-row", i),
                        RowState::current(on),
                        crate::components::TREE_ROW_H,
                        &p,
                    )
                    .debug_selector(move || sel.clone())
                    .pl(px(6. + depth as f32 * 14.))
                    .cursor_pointer()
                    .gap(px(6.))
                    .child(div().flex_none().w(px(12.)).when(has_sets, |d| {
                        d.child(
                            div()
                                .id(("structure-module-fold", i))
                                .child(crate::glyphs::icon(
                                    if open { Icon::ChevronDown } else { Icon::ChevronRight },
                                    11.,
                                    p.fg_dim,
                                ))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let id = id.clone();
                                    fold_view.update(cx, |this, cx| {
                                        if !this.folded.remove(&id) {
                                            this.folded.insert(id);
                                        }
                                        cx.notify();
                                    })
                                }),
                        )
                    }))
                    .child(glyph)
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .when(set.is_none(), |d| d.font_weight(FontWeight::MEDIUM))
                            .child(label),
                    )
                    .when_some(m.note().filter(|_| set.is_none()), |d, n| {
                        d.child(look::weak(n, &p).text_size(px(11.5)))
                    })
                    .when(set.is_some_and(|s| s.test), |d| d.child(look::weak("tests", &p).text_size(px(11.5))))
                    .on_click(move |_, _, cx| {
                        let r = r.clone();
                        click_view.update(cx, |this, cx| {
                            this.selected = Some(r);
                            cx.notify();
                        })
                    })
                    .into_any_element()
                })
                .collect()
        })
        .track_scroll(&self.module_scroll)
        .flex_1();
        let detail = self.module_detail(&s, cx);
        div()
            .pt(px(8.))
            .h(px(440.))
            .flex()
            .gap(px(12.))
            .child(
                div()
                    .w(px(220.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .p(px(4.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.ground)
                    .child(list),
            )
            .child(div().id("structure-module-detail").flex_1().min_w_0().overflow_y_scrollbar().child(detail))
            .into_any_element()
    }

    fn module_detail(&mut self, s: &Arc<Structure>, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let home = self.home(cx);
        let (id, only) = match &self.selected {
            Some(ModuleSel::Module(id)) => (id.clone(), None),
            Some(ModuleSel::SourceSet(id, n)) => (id.clone(), Some(n.clone())),
            None => return div().into_any_element(),
        };
        let Some(m) = s.module(&id) else { return div().into_any_element() };
        let [_, _, blue, _, violet, _] = p.lanes();
        let kind = match m.kind {
            super::data::BuildKind::Root => None,
            super::data::BuildKind::Included => Some(("Included build", Tone::Info)),
            super::data::BuildKind::BuildSrc => Some(("buildSrc", Tone::Info)),
        };
        let mut out = div().flex().flex_col().gap(px(10.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(tile(Icon::Module, if kind.is_some() { violet } else { blue }, 30., &p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(m.name.clone()))
                                .when_some(kind, |d, (k, t)| d.child(badge(k, t, &p)))
                                .when(m.spring_boot.is_some(), |d| {
                                    d.child(badge_with_icon("Spring Boot", Icon::Leaf, Tone::Success, &p))
                                }),
                        )
                        .child(look::weak(format!("{}  {}", m.build, m.gradle_path), &p).text_size(px(12.))),
                )
                .child(edit_button("structure-module-edit").on_click({
                    let id = id.clone();
                    cx.listener(move |this, _, _, cx| this.edit_in_build_file(id.clone(), Target::Include, cx))
                })),
        );
        let mut facts = div().flex().flex_col();
        facts = facts.child(row(
            "Folder",
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(icon(IconName::FolderOpen, 13., p.fg_dim))
                .child(short(&m.dir, &home)),
            &p,
        ));
        if let Some(f) = m.build_file.clone() {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            facts = facts.child(row(
                "Build file",
                div()
                    .id("structure-module-build-file")
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_color(p.accent)
                    .child(crate::icons::file_icon(&f, 14., &p))
                    .child(name)
                    .on_click(cx.listener(move |this, _, _, cx| this.open_file(f.clone(), cx))),
                &p,
            ));
        }
        if m.is_jvm() {
            let level_id = id.clone();
            facts = facts
                .child(row(
                    "Language level",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(gradle_value(
                            m.level().map(|l| format!("Java {l}")).unwrap_or_else(|| "Not set".into()),
                            &p,
                        ))
                        .when_some(m.toolchain.clone(), |d, t| {
                            d.child(badge(format!("toolchain {t}"), Tone::Neutral, &p))
                        })
                        .child(edit_button("structure-module-level").on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_in_build_file(level_id.clone(), Target::Level, cx)
                        }))),
                    &p,
                ))
                .child(row(
                    "Target compatibility",
                    gradle_value(m.target_level().map(|l| format!("Java {l}")).unwrap_or_else(|| "Not set".into()), &p),
                    &p,
                ));
        } else {
            facts = facts.child(hint("This project compiles no Java: it has no source sets.", &p));
        }
        out = out.child(facts);
        for set in m.source_sets.iter().filter(|set| only.as_ref().is_none_or(|n| *n == set.name)) {
            out = out.child(self.source_set_card(m, set, &home, cx));
        }
        out.into_any_element()
    }

    fn source_set_card(&self, m: &ModuleInfo, set: &SourceSet, home: &Path, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let [green, _, blue, _, _, _] = p.lanes();
        let levels = match (set.level(), set.target_level()) {
            (Some(a), Some(b)) if a == b => format!("Java {a}"),
            (Some(a), Some(b)) => format!("Java {a}, bytecode {b}"),
            (Some(a), None) | (None, Some(a)) => format!("Java {a}"),
            _ => String::new(),
        };
        let mut c = card(&p).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .pb(px(4.))
                .child(icon(
                    if set.test { Icon::TestFolder } else { Icon::SourceFolder },
                    15.,
                    if set.test { green } else { blue },
                ))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(set.name.clone()))
                .when(set.test, |d| d.child(badge_with_icon("Tests", IconName::FlaskConical, Tone::Success, &p)))
                .child(div().flex_1())
                .when(!levels.is_empty(), |d| d.child(badge_with_icon(levels, Icon::Jdk, Tone::Neutral, &p))),
        );
        c = c.child(div().pt(px(2.)).text_size(px(12.)).text_color(p.fg_dim).child("Content roots"));
        if set.roots.is_empty() {
            c = c.child(look::weak("None", &p));
        }
        for (dir, kind, exists) in &set.roots {
            let (g, color) = crate::project_view::root_look(Some(*kind), false, &p);
            c = c.child(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(g, 14., color))
                    .child(crate::components::path_text(
                        SharedString::from(format!("structure-root-{}-{}", set.name, dir.display())),
                        inside(dir, &m.dir, home),
                        if *exists { p.fg } else { p.fg_dim },
                    ))
                    .when(!exists, |d| d.child(badge("not there yet", Tone::Neutral, &p)))
                    .child(look::weak(kind.label(), &p).text_size(px(11.5))),
            );
        }
        c = c.child(div().pt(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child("Output folders"));
        for (i, out) in set.outputs.iter().enumerate() {
            let what = if i + 1 == set.outputs.len() && set.outputs.len() > 1 { "Resources" } else { "Classes" };
            c = c.child(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(IconName::FolderOutput, 14., p.fg_dim))
                    .child(crate::components::path_text(
                        SharedString::from(format!("structure-output-{}-{i}", set.name)),
                        inside(out, &m.dir, home),
                        p.fg,
                    ))
                    .child(look::weak(what, &p).text_size(px(11.5))),
            );
        }
        c.into_any_element()
    }

    // ----- Dependencies -------------------------------------------------------------------

    /// The rows of the dependency tree as it stands: opened by hand, or filtered.
    fn dep_rows(&self, graph: &super::data::Graph, query: &str) -> Vec<DepRow> {
        let lost: std::collections::HashSet<usize> = graph.conflicts().into_iter().map(|c| c.to).collect();
        let filtering = !query.is_empty() || self.conflicts_only;
        let takes = |n: &Node, i: usize| {
            (query.is_empty() || n.haystack().contains(query)) && (!self.conflicts_only || lost.contains(&i))
        };
        let edge_into = |from: Option<usize>, to: usize| -> (Option<String>, Option<String>) {
            let edges: &[Edge] = match from {
                None => &graph.roots,
                Some(f) => &graph.nodes[f].deps,
            };
            edges
                .iter()
                .find_map(|e| match e {
                    Edge::To { to: t, requested, declared_in } if *t == to => {
                        Some((requested.clone(), declared_in.clone()))
                    }
                    _ => None,
                })
                .unwrap_or((None, None))
        };
        let row = |key: Vec<usize>, open: bool, hit: bool| {
            let node = *key.last().unwrap();
            let from = key.len().checked_sub(2).map(|i| key[i]);
            let (requested, declared_in) = edge_into(from, node);
            let has_children = graph.nodes[node].deps.iter().any(|e| match e {
                Edge::To { to, .. } => !key.contains(to),
                Edge::Unresolved { .. } => true,
            });
            DepRow {
                depth: key.len() - 1,
                node: Some(node),
                unresolved: None,
                requested,
                declared_in,
                has_children,
                open,
                hit,
                key,
            }
        };
        let unresolved = |depth: usize, requested: &str, reason: &str| DepRow {
            key: Vec::new(),
            depth,
            node: None,
            unresolved: Some((requested.to_owned(), reason.to_owned())),
            requested: None,
            declared_in: None,
            has_children: false,
            open: false,
            hit: true,
        };
        let mut out = Vec::new();
        if filtering {
            let mut seen = std::collections::HashSet::new();
            let paths = graph.paths_to(|i, n| takes(n, i));
            for path in paths {
                for k in 1..=path.len() {
                    let key = path[..k].to_vec();
                    if seen.insert(key.clone()) {
                        out.push(row(key, k < path.len(), k == path.len()));
                    }
                }
            }
            for e in &graph.roots {
                if let Edge::Unresolved { requested, reason } = e
                    && !self.conflicts_only
                    && requested.to_lowercase().contains(query)
                {
                    out.push(unresolved(0, requested, reason));
                }
            }
            return out;
        }
        fn walk(
            graph: &super::data::Graph,
            key: Vec<usize>,
            open: &std::collections::HashSet<Vec<usize>>,
            out: &mut Vec<DepRow>,
            row: &dyn Fn(Vec<usize>, bool, bool) -> DepRow,
            unresolved: &dyn Fn(usize, &str, &str) -> DepRow,
        ) {
            let is_open = open.contains(&key);
            let node = *key.last().unwrap();
            let depth = key.len();
            out.push(row(key.clone(), is_open, false));
            if !is_open {
                return;
            }
            for e in &graph.nodes[node].deps {
                match e {
                    Edge::To { to, .. } if !key.contains(to) => {
                        let mut k = key.clone();
                        k.push(*to);
                        walk(graph, k, open, out, row, unresolved);
                    }
                    Edge::Unresolved { requested, reason } => out.push(unresolved(depth, requested, reason)),
                    _ => {}
                }
            }
        }
        for e in &graph.roots {
            match e {
                Edge::To { to, .. } => walk(graph, vec![*to], &self.open, &mut out, &row, &unresolved),
                Edge::Unresolved { requested, reason } => out.push(unresolved(0, requested, reason)),
            }
        }
        out
    }

    fn dependencies_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(e) = self.without_model(cx) {
            return e;
        }
        let Some(s) = self.structure() else { return div().into_any_element() };
        let p = palette(cx);
        let Some(m) = self.dep_module.as_ref().and_then(|id| s.module(id)) else {
            return div()
                .h(px(420.))
                .child(
                    EmptyState::new(snout_core::art::spots::no_project(), "No module compiles Java")
                        .hint("Dependencies show for the projects that apply a Java plugin.")
                        .on(p.card),
                )
                .into_any_element();
        };
        // The bar: the module, the scope, the filter, conflicts only.
        let modules: Vec<(String, String)> = s
            .jvm_modules()
            .map(|x| {
                (
                    x.id.clone(),
                    if x.gradle_path == ":" { x.name.clone() } else { format!("{}  {}", x.name, x.gradle_path) },
                )
            })
            .collect();
        let view = cx.entity();
        let picker = choice(
            "structure-dep-module",
            &m.id,
            modules,
            move |v, cx| view.update(cx, |this, cx| this.show_dependencies_of(v, cx)),
            &p,
        );
        let scopes = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(7.))
            .border_1()
            .border_color(p.border)
            .bg(p.ground)
            .children(Scope::ALL.map(|sc| {
                let on = self.scope == sc;
                let has = m.graph(sc).is_some();
                let id = format!("structure-scope-{}", sc.label().to_lowercase().replace(' ', "-"));
                let sel = id.clone();
                let hover = p.button_hover;
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .h(px(24.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .text_size(px(12.5))
                    .text_color(if on {
                        p.fg
                    } else if has {
                        p.fg_dim
                    } else {
                        p.fg_dim.opacity(0.55)
                    })
                    .when(on, |d| d.bg(p.card).shadow_sm().font_weight(FontWeight::MEDIUM))
                    .when(!on, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
                    .child(sc.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.scope = sc;
                        this.open.clear();
                        cx.notify();
                    }))
            }));
        let on = self.conflicts_only;
        let hover = p.button_hover;
        let conflicts_toggle = div()
            .id("structure-conflicts-only")
            .debug_selector(|| "structure-conflicts-only".into())
            .flex_none()
            .h(px(26.))
            .px(px(9.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(6.))
            .border_1()
            .cursor_pointer()
            .text_size(px(12.5))
            .when(on, |d| d.bg(p.wash(p.warn)).border_color(mix(p.warn, 0.45, p.card)).text_color(p.fg))
            .when(!on, |d| d.border_color(p.border).text_color(p.fg_dim).hover(move |s| s.bg(hover)))
            .child(icon(IconName::TriangleAlert, 13., if on { p.warn } else { p.fg_dim }))
            .child("Conflicts only")
            .on_click(cx.listener(|this, _, _, cx| {
                this.conflicts_only = !this.conflicts_only;
                cx.notify();
            }));
        let bar = div()
            .pt(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().flex().items_center().gap(px(10.)).child(picker).child(scopes))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().child(look::field(&self.filter, true, None, &p)))
                    .child(conflicts_toggle),
            );
        let Some(graph) = m.graph(self.scope) else {
            return div()
                .flex()
                .flex_col()
                .child(bar)
                .child(
                    div().h(px(340.)).child(
                        EmptyState::new(
                            snout_core::art::spots::no_results(),
                            format!("No {} dependencies", self.scope.label().to_lowercase()),
                        )
                        .hint(format!(
                            "{} declares nothing for this scope. {}",
                            m.name,
                            self.scope.detail_for(s.is_maven(m))
                        ))
                        .on(p.card)
                        .compact(),
                    ),
                )
                .into_any_element();
        };
        let conflicts = graph.conflicts();
        let unresolved_count = graph
            .roots
            .iter()
            .chain(graph.nodes.iter().flat_map(|n| n.deps.iter()))
            .filter(|e| matches!(e, Edge::Unresolved { .. }))
            .count();
        let declared = graph.roots.len();
        let summary = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .text_color(p.fg_dim)
            .child(format!("{declared} declared, {} in all.", graph.nodes.len()))
            .child(self.scope.detail_for(s.is_maven(m)))
            .child(div().flex_1())
            .when(!conflicts.is_empty(), |d| {
                let n = conflicts.len();
                d.child(badge_with_icon(
                    format!("{n} version conflict{}", if n == 1 { "" } else { "s" }),
                    IconName::TriangleAlert,
                    Tone::Warning,
                    &p,
                ))
            })
            .when(unresolved_count > 0, |d| {
                d.child(badge_with_icon(
                    format!("{unresolved_count} unresolved"),
                    IconName::OctagonAlert,
                    Tone::Danger,
                    &p,
                ))
            });
        let query = self.filter.read(cx).value().trim().to_lowercase();
        let rows = Rc::new(self.dep_rows(graph, &query));
        let brought = Rc::new(graph.brought_by());
        let catalog = Rc::new(self.catalog.clone());
        let list: AnyElement = if rows.is_empty() {
            EmptyState::new(snout_core::art::spots::no_results(), "Nothing matches")
                .hint(if self.conflicts_only {
                    "No version lost a conflict here."
                } else {
                    "Try fewer letters of the group, name or version."
                })
                .on(p.card)
                .compact()
                .into_any_element()
        } else {
            let view = cx.entity();
            let s2 = s.clone();
            let (mid, scope) = (m.id.clone(), self.scope);
            let count = rows.len();
            uniform_list("structure-deps", count, move |range, _, cx| {
                let Some(m) = s2.module(&mid) else { return Vec::new() };
                let Some(graph) = m.graph(scope) else { return Vec::new() };
                range.map(|i| dep_row(i, &rows[i], graph, &brought, &catalog, &s2, m, &view, cx)).collect()
            })
            .track_scroll(&self.dep_scroll)
            .flex_1()
            .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(bar)
            .child(summary)
            .child(
                div()
                    .h(px(330.))
                    .flex()
                    .flex_col()
                    .p(px(4.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.ground)
                    .child(list),
            )
            .into_any_element()
    }

    // ----- SDKs ---------------------------------------------------------------------------

    fn sdks_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let home = self.home(cx);
        let warm = p.lanes()[1];
        let machine = match &self.host_name {
            Some(h) => format!("on {h}"),
            None => "on this Mac".to_owned(),
        };
        let downloading = self.download.clone();
        let view = cx.entity();
        let download = crate::look::Trigger(
            div().id("structure-sdk-download").debug_selector(|| "structure-sdk-download".into()).child(
                Button::new("structure-sdk-download-button", IconName::Download)
                    .compact()
                    .label("Download JDK")
                    .disabled(downloading.is_some()),
            ),
        );
        use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
        let download = download.dropdown_menu(move |mut menu, _, _| {
            for major in [25u32, 21, 17, 11] {
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(format!("JDK {major} (Eclipse Temurin)"))
                        .on_click(move |_, _, cx| view.update(cx, |this, cx| this.download_jdk(major, cx))),
                );
            }
            menu
        });
        let mut body = div().flex().flex_col().gap(px(8.)).child(
            div()
                .pt(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.5))
                        .text_color(p.fg_dim)
                        .child(format!("Found where JDKs are installed {machine}, and added by you.")),
                )
                .child(
                    Button::new("structure-sdk-add", IconName::FolderPlus)
                        .compact()
                        .label("Add from Folder…")
                        .on_click(cx.listener(|this, _, _, cx| this.pick_jdk_folder(cx))),
                )
                .child(download),
        );
        if let Some((major, progress)) = &downloading {
            let pct = progress.percent();
            body = body.child(
                card(&p).child(
                    div().flex().items_center().gap(px(10.)).child(tile(IconName::Download, p.accent, 26., &p)).child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(format!(
                                "Downloading JDK {major}{}",
                                pct.map(|p| format!(", {p}%")).unwrap_or_default()
                            ))
                            .child(
                                div().h(px(4.)).w_full().rounded(px(2.)).bg(p.border).child(
                                    div()
                                        .h_full()
                                        .rounded(px(2.))
                                        .bg(p.accent)
                                        .w(gpui::relative(pct.unwrap_or(5) as f32 / 100.)),
                                ),
                            ),
                    ),
                ),
            );
        }
        let Some(detected) = self.detected.clone() else {
            return body.child(look::weak("Looking for JDKs…", &p)).into_any_element();
        };
        let in_use = self.project.jdk.clone();
        let mut list: Vec<(crate::lang::java::Jdk, bool)> = detected.into_iter().map(|j| (j, false)).collect();
        let mut missing: Vec<PathBuf> = Vec::new();
        for a in &self.added_read {
            match &a.jdk {
                Some(j) if !list.iter().any(|(o, _)| o.home == j.home) => list.push((j.clone(), true)),
                Some(_) => {}
                None => missing.push(a.home.clone()),
            }
        }
        // Added homes not read yet count as added.
        for h in &self.added {
            if !self.added_read.iter().any(|a| &a.home == h) && !list.iter().any(|(j, _)| &j.home == h) {
                missing.push(h.clone());
            }
        }
        list.sort_by_key(|(j, _)| std::cmp::Reverse(crate::lang::java::major_of(&j.version).unwrap_or(0)));
        if list.is_empty() && missing.is_empty() {
            return body
                .child(
                    div().h(px(360.)).child(
                        EmptyState::new(snout_core::art::spots::jdk(), "No JDK found")
                            .hint(format!("No JDK is installed {machine} where JDKs usually are. Download one, or add the folder of one."))
                            .on(p.card),
                    ),
                )
                .into_any_element();
        }
        let downloads = crate::lang::java::downloads_dir(crate::host::host(cx).info());
        let mut rows = div().flex().flex_col().gap(px(6.));
        for (i, (j, added)) in list.iter().enumerate() {
            let on = self.selected_jdk.as_ref() == Some(&j.home);
            let used = in_use.as_ref() == Some(&j.home);
            let origin = if *added {
                ("Added", Tone::Accent)
            } else if j.home.starts_with(&downloads) {
                ("Downloaded", Tone::Info)
            } else {
                ("Detected", Tone::Neutral)
            };
            let home_path = j.home.clone();
            let remove_path = j.home.clone();
            let hover = p.row_hover();
            rows = rows.child(
                div()
                    .id(("structure-sdk", i))
                    .debug_selector(move || format!("structure-sdk-{i}"))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if on { mix(p.accent, 0.5, p.border) } else { p.border })
                    .when(on, |d| d.bg(p.row_selected()))
                    .when(!on, |d| d.bg(p.panel).hover(move |s| s.bg(hover)))
                    .cursor_pointer()
                    .child(tile(Icon::Jdk, warm, 30., &p))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("JDK {}", j.major)))
                                    .child(look::weak(j.version.clone(), &p))
                                    .child(badge(origin.0, origin.1, &p))
                                    .when(used, |d| {
                                        d.child(badge_with_icon(
                                            "This project's",
                                            IconName::FolderCog,
                                            Tone::Success,
                                            &p,
                                        ))
                                    }),
                            )
                            .child(look::weak(short(&j.home, &home), &p).text_size(px(12.))),
                    )
                    .when(*added, |d| {
                        d.child(
                            IconButton::new(
                                SharedString::from(format!("structure-sdk-remove-{i}")),
                                IconName::Trash,
                                "Remove from the list",
                            )
                            .size(24.)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove_jdk(&remove_path, cx))),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_jdk = Some(home_path.clone());
                        cx.notify();
                    })),
            );
        }
        for (i, h) in missing.iter().enumerate() {
            let remove_path = h.clone();
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.panel)
                    .child(tile(IconName::TriangleAlert, p.warn, 30., &p))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("No JDK there now"))
                            .child(look::weak(short(h, &home), &p).text_size(px(12.))),
                    )
                    .child(
                        IconButton::new(
                            SharedString::from(format!("structure-sdk-remove-missing-{i}")),
                            IconName::Trash,
                            "Remove from the list",
                        )
                        .size(24.)
                        .on_click(cx.listener(move |this, _, _, cx| this.remove_jdk(&remove_path, cx))),
                    ),
            );
        }
        let detail = self
            .selected_jdk
            .as_ref()
            .and_then(|h| list.iter().find(|(j, _)| &j.home == h))
            .map(|(j, added)| self.jdk_detail(j, *added, &home, cx));
        body.child(rows)
            .children(detail)
            .child(look::weak("Removing an added JDK takes it off the list; its files stay where they are.", &p))
            .into_any_element()
    }

    /// What the SDKs page says of the JDK picked in its list, and the button that makes it
    /// the project's.
    fn jdk_detail(&self, j: &crate::lang::java::Jdk, added: bool, home: &Path, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let used = self.project.jdk.as_ref() == Some(&j.home);
        let home_path = j.home.clone();
        card(&p)
            .debug_selector(|| "structure-sdk-detail".into())
            .child(section("Selected JDK", &p).pt(px(0.)))
            .child(row("Version", div().child(format!("{} (Java {})", j.version, j.major)), &p))
            .child(row("Home", div().child(short(&j.home, home)), &p))
            .child(row("Kept in", div().child(if added { "Your settings" } else { "Found by SnoutCode" }), &p))
            .child(row(
                "This project",
                if used {
                    div().child(badge_with_icon("Runs on it", IconName::CircleCheck, Tone::Success, &p))
                } else {
                    div().child(
                        Button::new("structure-sdk-use", IconName::FolderCog)
                            .compact()
                            .label("Use for This Project")
                            .on_click(cx.listener(move |this, _, _, cx| this.set_jdk(Some(home_path.clone()), cx))),
                    )
                },
                &p,
            ))
            .into_any_element()
    }

    // ----- Facets -------------------------------------------------------------------------

    fn facets_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let green = crate::spring::green(&p);
        let s = self.structure();
        let spring_modules: Vec<ModuleInfo> =
            s.iter().flat_map(|s| s.spring_modules().cloned().collect::<Vec<_>>()).collect();
        let spring = crate::spring::spring(cx);
        let (mode, info, beans, loaded) = match &spring {
            Some(sp) => {
                let sp = sp.read(cx);
                (sp.profile_mode.clone(), sp.profile_info.clone(), sp.bean_count(), sp.loaded)
            }
            None => (self.model.read(cx).spring_mode(), Default::default(), 0, false),
        };
        let detected = !spring_modules.is_empty() || (loaded && beans > 0);
        let root = self.root.clone().unwrap_or_default();
        let home = self.home(cx);
        let mut spring_card = card(&p).gap(px(2.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .pb(px(4.))
                .child(tile(Icon::Leaf, green, 30., &p))
                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("Spring"))
                .child(if detected {
                    badge_with_icon("Detected", IconName::CircleCheck, Tone::Success, &p)
                } else {
                    badge("Not detected", Tone::Neutral, &p)
                })
                .child(div().flex_1())
                .child(
                    Button::new("structure-facet-spring-settings", IconName::Settings)
                        .compact()
                        .ghost()
                        .label("Spring Settings")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.open_settings(crate::settings_view::Page::Spring, cx)),
                        ),
                ),
        );
        if detected {
            let names: Vec<String> = spring_modules
                .iter()
                .map(|m| match m.spring_boot.as_deref().filter(|v| !v.is_empty()) {
                    Some(v) => format!("{} (Spring Boot {v})", m.name),
                    None => m.name.clone(),
                })
                .collect();
            spring_card = spring_card
                .child(row(
                    "Modules",
                    div().child(if names.is_empty() {
                        "None with the Spring Boot plugin".to_owned()
                    } else {
                        names.join(", ")
                    }),
                    &p,
                ))
                .child(row(
                    "Beans",
                    div().child(if loaded {
                        format!("{beans} in the Spring model")
                    } else {
                        "Read once the Java engine is ready".into()
                    }),
                    &p,
                ))
                .child(row("Active profiles", div().child(crate::spring::mode_label(&mode, &info)), &p));
            let known = info.known();
            if !known.is_empty() {
                spring_card = spring_card.child(row(
                    "Profiles known",
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .children(known.iter().map(|k| badge(k.name.clone(), Tone::Success, &p))),
                    &p,
                ));
            }
            spring_card = spring_card
                .child(div().pt(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child("Configuration files"));
            if self.config_files.is_empty() {
                spring_card =
                    spring_card.child(look::weak("No application.properties or application.yml in the resources", &p));
            }
            for (i, f) in self.config_files.iter().enumerate() {
                let file = f.clone();
                let hover = p.row_hover();
                spring_card = spring_card.child(
                    div()
                        .id(("structure-config-file", i))
                        .debug_selector(move || format!("structure-config-file-{i}"))
                        .h(px(26.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(crate::icons::file_icon(f, 15., &p))
                        .child(inside(f, &root, &home))
                        .on_click(cx.listener(move |this, _, _, cx| this.open_file(file.clone(), cx))),
                );
            }
        } else {
            spring_card = spring_card.child(
                div().h(px(170.)).child(
                    EmptyState::new(snout_core::art::spots::no_beans(), "No Spring here")
                        .hint("No module applies the Spring Boot plugin, and the Spring model has no beans.")
                        .on(p.panel)
                        .compact(),
                ),
            );
        }
        // Claude Code.
        let clients = cx.try_global::<crate::claude_ide::IdeStatus>().map(|s| s.clients).unwrap_or(0);
        let program = self.model.read(cx).claude.clone();
        let claude = p.claude_ink();
        let claude_card = card(&p)
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pb(px(4.))
                    .child(tile(IconName::Sparkles, claude, 30., &p))
                    .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("Claude Code"))
                    .child(match (clients, program.is_some()) {
                        (n, _) if n > 0 => badge_with_icon("Connected", IconName::Plug, Tone::Success, &p),
                        (_, true) => badge_with_icon("Ready", IconName::CircleCheck, Tone::Claude, &p),
                        (_, false) => badge_with_icon("Not found", IconName::Unplug, Tone::Warning, &p),
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("structure-facet-claude-settings", IconName::Settings)
                            .compact()
                            .ghost()
                            .label("Claude Code Settings")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_settings(crate::settings_view::Page::ClaudeCode, cx)
                            })),
                    ),
            )
            .child(row(
                "Program",
                match &program {
                    Some(path) => div().child(short(path, &home)),
                    None => div().text_color(p.fg_dim).child("claude was not found on PATH or the usual places"),
                },
                &p,
            ))
            .child(row(
                "IDE connection",
                div().child(match clients {
                    0 => "No session connected. A claude started in the console connects by itself.".to_owned(),
                    1 => "1 session connected: it sees the open files, selection and problems".to_owned(),
                    n => format!("{n} sessions connected"),
                }),
                &p,
            ));
        div().pt(px(8.)).flex().flex_col().gap(px(12.)).child(spring_card).child(claude_card).into_any_element()
    }

    fn page_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        match self.page {
            Page::Project => self.project_page(cx),
            Page::Modules => self.modules_page(cx),
            Page::Dependencies => self.dependencies_page(cx),
            Page::Sdks => self.sdks_page(cx),
            Page::Facets => self.facets_page(cx),
        }
    }

    /// What the page says under its title.
    fn subtitle(&self) -> &'static str {
        if self.structure().is_some_and(|s| s.tool() == "Maven") {
            match self.page {
                Page::Project => {
                    return "The project as a whole: the JDK it runs on is yours to pick, the rest is Maven's.";
                }
                Page::Modules => return "Every Maven module of every build, with its source sets, roots and outputs.",
                Page::Dependencies => return "What each module declares and what that brings in, as Maven resolved it.",
                _ => {}
            }
        }
        match self.page {
            Page::Project => "The project as a whole: the JDK it runs on is yours to pick, the rest is Gradle's.",
            Page::Modules => "Every Gradle project of every build, with its source sets, roots and outputs.",
            Page::Dependencies => "What each module declares and what that brings in, as Gradle resolved it.",
            Page::Sdks => "The JDKs SnoutCode can run Gradle and the Java engine on.",
            Page::Facets => "Frameworks and tools, as they stand. Read only.",
        }
    }
}

/// What declares the dependency of row `r` (node `n`): itself when declared, else the
/// declared dependency that brings it in.
fn declaration(graph: &super::data::Graph, brought: &[Option<usize>], r: &DepRow, n: usize) -> Option<Target> {
    let through = if r.depth == 0 { Some(n) } else { r.key.first().copied().or(brought.get(n).copied().flatten()) };
    through.and_then(|t| match &graph.nodes[t].kind {
        NodeKind::Module { group, name, .. } => Some(Target::Library { group: group.clone(), name: name.clone() }),
        NodeKind::Project { path, .. } => Some(Target::Project { path: path.clone() }),
        NodeKind::Other(_) => None,
    })
}

/// One row of the dependency tree.
#[allow(clippy::too_many_arguments)]
fn dep_row(
    i: usize,
    r: &DepRow,
    graph: &super::data::Graph,
    brought: &Rc<Vec<Option<usize>>>,
    catalog: &Rc<std::collections::HashSet<(String, String)>>,
    s: &Arc<Structure>,
    m: &ModuleInfo,
    view: &gpui::Entity<ProjectStructureView>,
    cx: &mut App,
) -> AnyElement {
    let p = palette(cx);
    let [_, warm, blue, _, _, _] = p.lanes();
    let base = crate::components::list_row_h(
        ("structure-dep", i),
        RowState::current(false),
        crate::components::TREE_ROW_H,
        &p,
    )
    .pl(px(6. + r.depth as f32 * 16.))
    .gap(px(6.));
    if let Some((requested, reason)) = &r.unresolved {
        return base
            .debug_selector(move || format!("structure-dep-{i}"))
            .child(div().w(px(12.)))
            .child(icon(IconName::CircleAlert, 14., p.error))
            .child(div().text_color(p.error).child(requested.clone()))
            .child(badge("Unresolved", Tone::Danger, &p))
            .child(look::weak(reason.clone(), &p).text_size(px(11.5)).flex_1())
            .into_any_element();
    }
    let Some(n) = r.node else { return base.into_any_element() };
    let node = &graph.nodes[n];
    let key = r.key.clone();
    let fold_view = view.clone();
    let chevron = div().flex_none().w(px(12.)).when(r.has_children, |d| {
        let open = r.open;
        d.child(
            div()
                .id(("structure-dep-fold", i))
                .debug_selector(move || format!("structure-dep-fold-{i}"))
                .cursor_pointer()
                .child(crate::glyphs::icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 11., p.fg_dim))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let key = key.clone();
                    fold_view.update(cx, |this, cx| {
                        if !this.open.remove(&key) {
                            this.open.insert(key);
                        }
                        cx.notify();
                    })
                }),
        )
    });
    let lost = graph.lost_versions(n);
    let declared = r.depth == 0;
    let text_ink = if r.hit { p.accent } else { p.fg };
    let mut row = base.debug_selector(move || format!("structure-dep-{i}")).child(chevron);
    match &node.kind {
        NodeKind::Project { build, path } => {
            let target = s.project_module(m, build, path).map(|t| (t.id.clone(), t.name.clone()));
            row = row.child(icon(Icon::Module, 14., blue));
            match target {
                Some((id, name)) => {
                    let (v1, v2, id2) = (view.clone(), view.clone(), id.clone());
                    let hover_ink = mix(p.accent, 0.7, p.fg);
                    row = row
                        .child(
                            div()
                                .id(("structure-dep-link", i))
                                .debug_selector(move || format!("structure-dep-link-{i}"))
                                .cursor_pointer()
                                .text_color(p.accent)
                                .font_weight(FontWeight::MEDIUM)
                                .hover(move |s| s.text_color(hover_ink).underline())
                                .child(format!("project {path}"))
                                .tooltip(move |w, cx| {
                                    gpui_component::tooltip::Tooltip::new(format!("Show {name} in Modules"))
                                        .build(w, cx)
                                })
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    v1.update(cx, |this, cx| this.show_module(id, cx))
                                }),
                        )
                        .child(badge_with_icon("project", IconName::Link, Tone::Accent, &p))
                        .when_some(r.declared_in.clone(), |d, c| d.child(badge(c, Tone::Neutral, &p)))
                        .child(div().flex_1())
                        .children(declaration(graph, brought, r, n).map(|target| {
                            let (v, mid) = (view.clone(), m.id.clone());
                            IconButton::new(
                                SharedString::from(format!("structure-dep-show-{i}")),
                                IconName::FileSearch,
                                if declared { "Show in build file" } else { "Show the declaration that brings it in" },
                            )
                            .size(22.)
                            .on_click(move |_, _, cx| {
                                let (mid, target) = (mid.clone(), target.clone());
                                v.update(cx, |this, cx| this.edit_in_build_file(mid, target, cx))
                            })
                        }))
                        .child(
                            IconButton::new(
                                SharedString::from(format!("structure-dep-go-{i}")),
                                IconName::ArrowRight,
                                "Show its dependencies",
                            )
                            .size(22.)
                            .on_click(move |_, _, cx| {
                                let id = id2.clone();
                                v2.update(cx, |this, cx| this.show_dependencies_of(id, cx))
                            }),
                        );
                }
                None => {
                    row = row.child(div().text_color(text_ink).child(format!("project {path}"))).child(div().flex_1());
                }
            }
        }
        _ => {
            let coords = node.coordinates();
            let version = node.version().unwrap_or_default().to_owned();
            row = row.child(icon(Icon::Jar, 14., warm)).child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_color(text_ink)
                    .when(declared, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(coords),
            );
            match r.requested.as_deref() {
                Some(asked) if !asked.is_empty() => {
                    let why = match node.reason.as_deref() {
                        Some("conflict") => "a newer version asked for elsewhere won",
                        Some("rule") => "dependency management chose it",
                        Some("forced") => "it is forced",
                        Some("constraint") => "a constraint asks for it",
                        _ => "another request won",
                    };
                    let tip = format!("{asked} was asked for here; {version} is used: {why}");
                    row = row
                        .child(
                            div()
                                .id(("structure-dep-conflict", i))
                                .debug_selector(move || format!("structure-dep-conflict-{i}"))
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(div().text_color(p.fg_dim).line_through().child(asked.to_owned()))
                                .child(icon(IconName::ArrowRight, 11., p.warn))
                                .child(div().text_color(p.warn).font_weight(FontWeight::MEDIUM).child(version))
                                .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx)),
                        )
                        .child(badge_with_icon("Conflict", IconName::TriangleAlert, Tone::Warning, &p));
                }
                Some(_) => {
                    row = row.child(look::weak(version, &p)).child(badge("managed", Tone::Neutral, &p));
                }
                None => {
                    row = row.child(look::weak(version.clone(), &p));
                    if !lost.is_empty() {
                        row = row.child(badge_with_icon(
                            format!("won over {}", lost.join(", ")),
                            IconName::BadgeCheck,
                            Tone::Success,
                            &p,
                        ));
                    }
                }
            }
            row =
                row.when_some(r.declared_in.clone(), |d, c| d.child(badge(c, Tone::Neutral, &p))).child(div().flex_1());
            // Where it is declared: its own line, or the declared dependency that brings it in.
            let (group, name) = match &node.kind {
                NodeKind::Module { group, name, .. } => (group.clone(), name.clone()),
                _ => (String::new(), String::new()),
            };
            let target = declaration(graph, brought, r, n);
            if let Some(target) = target {
                let (v, id) = (view.clone(), m.id.clone());
                let tip = if declared {
                    "Show in build file".to_owned()
                } else {
                    "Show the declaration that brings it in".to_owned()
                };
                row = row.child(
                    IconButton::new(SharedString::from(format!("structure-dep-show-{i}")), IconName::FileSearch, tip)
                        .size(22.)
                        .on_click(move |_, _, cx| {
                            let (id, target) = (id.clone(), target.clone());
                            v.update(cx, |this, cx| this.edit_in_build_file(id, target, cx))
                        }),
                );
            }
            if declared && !group.is_empty() && catalog.contains(&(group.clone(), name.clone())) {
                let (v, id) = (view.clone(), m.id.clone());
                row = row.child(
                    IconButton::new(
                        SharedString::from(format!("structure-dep-catalog-{i}")),
                        IconName::BookOpen,
                        "Show in version catalog",
                    )
                    .size(22.)
                    .on_click(move |_, _, cx| {
                        let (id, g, n) = (id.clone(), group.clone(), name.clone());
                        v.update(cx, |this, cx| this.show_in_catalog(id, g, n, cx))
                    }),
                );
            }
        }
    }
    row.into_any_element()
}

impl Render for ProjectStructureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let changed = self.changed(cx);
        let page = self.page_body(cx);
        let tree = self.tree(cx);
        let button = |id: &'static str, ico: IconName, text: &'static str, primary: bool, enabled: bool| {
            crate::components::footer_button(id, ico, text, primary, enabled, &p)
        };
        let syncing = crate::project_model::store(cx).is_some_and(|s| s.read(cx).syncing.is_some());
        let gradle_page = matches!(self.page, Page::Project | Page::Modules | Page::Dependencies);
        let tool = self.structure().map_or("Gradle", |s| s.tool());
        let status = self.note.clone().map(|(t, err)| (t, if err { p.error } else { p.fg_dim })).or_else(|| {
            self.structure().map(|s| {
                let modules = s.modules.len();
                (
                    format!(
                        "{} · {modules} module{} from the last {} sync",
                        s.name,
                        if modules == 1 { "" } else { "s" },
                        s.tool()
                    ),
                    p.fg_dim,
                )
            })
        });
        div()
            .id("project-structure")
            .debug_selector(|| "project-structure".into())
            .key_context("ProjectStructure")
            .track_focus(&gpui::Focusable::focus_handle(self, cx))
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(StructureEvent::Cancel)))
            .occlude()
            .w(px(1000.))
            .h(px(660.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(
                crate::components::dialog_header_icon(
                    Tone::Accent,
                    IconName::LayoutGrid,
                    "Project Structure",
                    None,
                    &p,
                )
                .pt(px(14.))
                .pb(px(12.))
                .border_b_1()
                .border_color(p.border),
            )
            .child(
                div().flex_1().min_h_0().flex().child(tree).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(crate::components::page_header(
                            self.page.art(),
                            self.page.title(),
                            self.subtitle(),
                            div().when(gradle_page, |d| {
                                d.child(
                                    Button::new("structure-sync", IconName::RefreshCw)
                                        .debug_selector("structure-sync")
                                        .compact()
                                        .ghost()
                                        .label(match (syncing, tool) {
                                            (true, _) => "Syncing…",
                                            (_, "Maven") => "Reload Maven",
                                            (_, "Gradle") => "Sync Gradle",
                                            _ => "Sync",
                                        })
                                        .disabled(syncing)
                                        .tooltip(match tool {
                                            "Maven" => "Read the Maven builds again",
                                            "Gradle" => "Read the Gradle builds again",
                                            _ => "Read the builds again",
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| this.sync(cx))),
                                )
                            }),
                            &p,
                        ))
                        .child(
                            div()
                                .id("structure-page")
                                .flex_1()
                                .min_h_0()
                                .px(px(20.))
                                .pb(px(12.))
                                .overflow_y_scrollbar()
                                .child(page),
                        ),
                ),
            )
            .child(crate::components::dialog_footer(
                status,
                vec![
                    button("structure-cancel", crate::components::CANCEL_ICON, "Cancel", false, true)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(StructureEvent::Cancel)))
                        .into_any_element(),
                    button("structure-apply", crate::components::APPLY_ICON, "Apply", false, changed)
                        .when(changed, |d| d.on_click(cx.listener(|this, _, _, cx| this.apply(false, cx))))
                        .into_any_element(),
                    button("structure-ok", crate::components::OK_ICON, "OK", true, true)
                        .on_click(cx.listener(|this, _, _, cx| this.apply(true, cx)))
                        .into_any_element(),
                ],
                &p,
            ))
    }
}
