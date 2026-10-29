//! The Project tool window's Project view: the project as its Gradle model has it. The
//! project's folder, its modules (each with its content and its source roots marked by
//! what they hold), the source folders several modules share, the builds that failed to
//! sync, every build's Gradle scripts, and the external libraries down to their classes.
//! Until the model is read it shows the plain file tree, with a note saying so.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, EventEmitter, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::Sizable as _;
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;

use crate::files::{Entry, FileTree, GitMark, ROW, git_marks};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::model::{Action, Model};
use crate::project_model::{JarIndex, ProjectModel, ProjectModels, RootKind, ScriptKind, jar_index};
use crate::theme::{Palette, mix, palette};

const INDENT: f32 = 12.0;

/// A node that opens and closes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NodeId {
    Root,
    /// A module, by id.
    Module(String),
    /// A shared source folder, by its folder.
    Shared(PathBuf),
    /// A build that failed to sync, by build id.
    Failed(String),
    Dir(PathBuf),
    Scripts,
    Libraries,
    /// A library, by id.
    Library(String),
    /// A package of a library: the library's id and the package.
    Package(String, String),
}

/// What a row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// The project's folder; the module it is, when it is one.
    Root(Option<usize>),
    Module(usize),
    Shared(usize),
    Failed(usize),
    /// Why a build failed, under its node.
    Reason(usize),
    Dir {
        path: PathBuf,
        root: Option<RootKind>,
        excluded: bool,
        ignored: bool,
    },
    File {
        path: PathBuf,
        ignored: bool,
    },
    Scripts,
    Script(usize),
    Libraries,
    Jdk(usize),
    Library(usize),
    Package {
        library: usize,
        name: String,
    },
    Class {
        library: usize,
        package: String,
        name: String,
    },
    /// A line of quiet text.
    Note(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub depth: usize,
    /// The node, when the row opens and closes.
    pub id: Option<NodeId>,
    pub open: bool,
}

/// A folder that is a node of its own: a module's content, a shared folder, a failed build.
#[derive(Clone, Copy)]
enum Special {
    Module(usize),
    Shared(usize),
    Failed(usize),
}

/// Lists a folder (the file tree's cache) and a jar's classes, for [`rows`].
pub trait Source {
    fn list(&mut self, dir: &Path) -> Vec<Entry>;
    fn jar(&mut self, file: &Path) -> Option<Rc<JarIndex>>;
}

struct Walk<'a> {
    model: &'a ProjectModel,
    expanded: &'a HashSet<NodeId>,
    source: &'a mut dyn Source,
    special: HashMap<PathBuf, Special>,
    roots: HashMap<PathBuf, RootKind>,
    excluded: HashSet<PathBuf>,
    out: Vec<Row>,
}

impl Walk<'_> {
    fn push(&mut self, kind: RowKind, depth: usize, id: Option<NodeId>) -> bool {
        let open = id.as_ref().is_some_and(|id| self.expanded.contains(id));
        self.out.push(Row { kind, depth, id, open });
        open
    }

    fn special(&mut self, s: Special, depth: usize) {
        let m = self.model;
        match s {
            Special::Module(i) => {
                if self.push(RowKind::Module(i), depth, Some(NodeId::Module(m.modules[i].id.clone()))) {
                    self.dir(&m.modules[i].content_root.clone(), depth + 1, false);
                }
            }
            Special::Shared(i) => {
                let root = m.shared[i].content_root.clone();
                if self.push(RowKind::Shared(i), depth, Some(NodeId::Shared(root.clone()))) {
                    self.dir(&root, depth + 1, false);
                }
            }
            Special::Failed(i) => {
                if self.push(RowKind::Failed(i), depth, Some(NodeId::Failed(m.failed[i].build.clone()))) {
                    self.push(RowKind::Reason(i), depth + 1, None);
                    self.dir(&m.failed[i].root_dir.clone(), depth + 1, false);
                }
            }
        }
    }

    /// The folders and files in `dir`; a folder that is a node of its own shows as that.
    fn dir(&mut self, dir: &Path, depth: usize, ignored: bool) {
        for e in self.source.list(dir) {
            if e.dir
                && let Some(s) = self.special.get(&e.path).copied()
            {
                self.special(s, depth);
                continue;
            }
            let ignored = ignored || e.ignored;
            if e.dir {
                let root = self.roots.get(&e.path).copied();
                let excluded = self.excluded.contains(&e.path);
                let kind = RowKind::Dir { path: e.path.clone(), root, excluded, ignored };
                if self.push(kind, depth, Some(NodeId::Dir(e.path.clone()))) {
                    self.dir(&e.path, depth + 1, ignored || excluded);
                }
            } else {
                self.push(RowKind::File { path: e.path, ignored }, depth, None);
            }
        }
    }

    fn scripts(&mut self, depth: usize) {
        if self.push(RowKind::Scripts, depth, Some(NodeId::Scripts)) {
            for i in 0..self.model.scripts.len() {
                self.push(RowKind::Script(i), depth + 1, None);
            }
        }
    }

    fn libraries(&mut self, depth: usize) {
        if !self.push(RowKind::Libraries, depth, Some(NodeId::Libraries)) {
            return;
        }
        let m = self.model;
        for i in 0..m.jdks.len() {
            self.push(RowKind::Jdk(i), depth + 1, None);
        }
        for (i, lib) in m.libraries.iter().enumerate() {
            let has_classes = lib.file.as_ref().is_some_and(|f| f.extension().is_some_and(|e| e == "jar"));
            let id = has_classes.then(|| NodeId::Library(lib.id.clone()));
            if !self.push(RowKind::Library(i), depth + 1, id) {
                continue;
            }
            let Some(index) = lib.file.as_deref().and_then(|f| self.source.jar(f)) else {
                self.push(RowKind::Note("No classes to show".into()), depth + 2, None);
                continue;
            };
            for (package, classes) in index.iter() {
                let id = NodeId::Package(lib.id.clone(), package.clone());
                if self.push(RowKind::Package { library: i, name: package.clone() }, depth + 2, Some(id)) {
                    for name in classes {
                        let kind = RowKind::Class { library: i, package: package.clone(), name: name.clone() };
                        self.push(kind, depth + 3, None);
                    }
                }
            }
        }
    }
}

/// The rows showing, given what is open.
pub fn rows(model: &ProjectModel, expanded: &HashSet<NodeId>, source: &mut dyn Source) -> Vec<Row> {
    let mut special = HashMap::new();
    let root_module = model.modules.iter().position(|m| m.content_root == model.root);
    for (i, m) in model.modules.iter().enumerate() {
        if Some(i) != root_module {
            special.entry(m.content_root.clone()).or_insert(Special::Module(i));
        }
    }
    for (i, s) in model.shared.iter().enumerate() {
        special.entry(s.content_root.clone()).or_insert(Special::Shared(i));
    }
    for (i, f) in model.failed.iter().enumerate() {
        special.entry(f.root_dir.clone()).or_insert(Special::Failed(i));
    }
    let roots = model
        .modules
        .iter()
        .flat_map(|m| &m.roots)
        .chain(model.shared.iter().flat_map(|s| &s.roots))
        .map(|r| (r.dir.clone(), r.kind))
        .collect();
    let excluded = model.modules.iter().flat_map(|m| m.excluded.iter().cloned()).collect();
    let mut walk = Walk { model, expanded, source, special, roots, excluded, out: Vec::new() };
    if walk.push(RowKind::Root(root_module), 0, Some(NodeId::Root)) {
        walk.dir(&model.root, 1, false);
        // What lies outside the project's folder: a shared folder of a build it holds.
        let mut outside: Vec<(PathBuf, Special)> =
            walk.special.iter().filter(|(p, _)| !p.starts_with(&model.root)).map(|(p, s)| (p.clone(), *s)).collect();
        outside.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));
        for (_, s) in outside {
            walk.special(s, 1);
        }
    }
    walk.scripts(0);
    walk.libraries(0);
    walk.out
}

/// The nodes to open so `path` shows.
pub fn path_to(model: &ProjectModel, path: &Path) -> Vec<NodeId> {
    let mut out = vec![NodeId::Root];
    let owner = |dir: &Path| -> Option<NodeId> {
        if dir == model.root {
            return None;
        }
        if let Some(m) = model.modules.iter().find(|m| m.content_root == dir) {
            return Some(NodeId::Module(m.id.clone()));
        }
        if let Some(s) = model.shared.iter().find(|s| s.content_root == dir) {
            return Some(NodeId::Shared(s.content_root.clone()));
        }
        if let Some(f) = model.failed.iter().find(|f| f.root_dir == dir) {
            return Some(NodeId::Failed(f.build.clone()));
        }
        Some(NodeId::Dir(dir.to_owned()))
    };
    // The folder the walk starts from: the project's, or above a node outside it.
    let base = if path.starts_with(&model.root) {
        Some(model.root.clone())
    } else {
        let outside = model.modules.iter().map(|m| &m.content_root);
        let outside = outside.chain(model.shared.iter().map(|s| &s.content_root));
        outside
            .filter(|r| path.starts_with(r))
            .max_by_key(|r| r.components().count())
            .and_then(|r| r.parent().map(Path::to_owned))
    };
    let Some(base) = base else { return out };
    let mut dirs: Vec<&Path> = path.ancestors().skip(1).take_while(|d| *d != base && d.starts_with(&base)).collect();
    dirs.reverse();
    out.extend(dirs.into_iter().filter_map(owner));
    out
}

/// What the Project view asks of the workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectViewEvent {
    /// Open a library's class, read only.
    OpenClass { package: String, class: String },
}

pub struct ProjectView {
    model: Entity<Model>,
    files: Entity<FileTree>,
    pub store: Entity<ProjectModels>,
    pub expanded: HashSet<NodeId>,
    /// The jars' classes: `None` while being read.
    jars: HashMap<PathBuf, Option<Option<Rc<JarIndex>>>>,
    root: Option<PathBuf>,
}

impl EventEmitter<ProjectViewEvent> for ProjectView {}

struct Lister<'a, 'b> {
    files: &'a Entity<FileTree>,
    jars: &'a mut HashMap<PathBuf, Option<Option<Rc<JarIndex>>>>,
    cx: &'a mut Context<'b, ProjectView>,
}

impl Source for Lister<'_, '_> {
    fn list(&mut self, dir: &Path) -> Vec<Entry> {
        self.files.update(self.cx, |t, cx| t.children(dir, cx))
    }

    /// The jar's classes, read off the UI thread: none until they are (the view redraws).
    fn jar(&mut self, file: &Path) -> Option<Rc<JarIndex>> {
        match self.jars.get(file) {
            Some(Some(known)) => return known.clone(),
            Some(None) => return None,
            None => {}
        }
        self.jars.insert(file.to_owned(), None);
        let (host, path) = (crate::host::host(self.cx), file.to_owned());
        let read = self.cx.background_executor().spawn(async move { jar_index(&*host, &path) });
        let path = file.to_owned();
        self.cx
            .spawn(async move |this, cx| {
                let index = read.await;
                let _ = this.update(cx, |this, cx| {
                    this.jars.insert(path, Some(index.map(Rc::new)));
                    cx.notify();
                });
            })
            .detach();
        None
    }
}

impl ProjectView {
    pub fn new(
        model: Entity<Model>,
        files: Entity<FileTree>,
        store: Entity<ProjectModels>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        cx.observe(&files, |_, _, cx| cx.notify()).detach();
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        cx.observe_global::<crate::claude_ide::AgentFiles>(|_, cx| cx.notify()).detach();
        Self { model, files, store, expanded: HashSet::from([NodeId::Root]), jars: HashMap::new(), root: None }
    }

    /// The model, once it is read for the open project.
    pub fn project_model(&self, cx: &gpui::App) -> Option<Rc<ProjectModel>> {
        let root = self.model.read(cx).project_root()?;
        self.store.read(cx).model_for(&root)
    }

    pub fn toggle(&mut self, id: NodeId, cx: &mut Context<Self>) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        cx.notify();
    }

    /// Close every node but the project's.
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.expanded.clear();
        self.expanded.insert(NodeId::Root);
        cx.notify();
    }

    /// Open the nodes down to `path`.
    pub fn reveal(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(model) = self.project_model(cx) else { return };
        self.expanded.extend(path_to(&model, path));
        cx.notify();
    }

    /// The rows showing now.
    pub fn rows(&mut self, cx: &mut Context<Self>) -> Vec<Row> {
        let Some(model) = self.project_model(cx) else { return Vec::new() };
        let files = self.files.clone();
        let mut lister = Lister { files: &files, jars: &mut self.jars, cx };
        rows(&model, &self.expanded, &mut lister)
    }
}

/// A source root's glyph and colour.
pub fn root_look(kind: Option<RootKind>, excluded: bool, p: &Palette) -> (Icon, Hsla) {
    let [green, warm, blue, clay, violet, _] = p.lanes();
    if excluded {
        return (Icon::ExcludedFolder, mix(clay, 0.6, p.fg_dim));
    }
    match kind {
        Some(RootKind::Main) => (Icon::SourceFolder, blue),
        Some(RootKind::Test) => (Icon::TestFolder, green),
        Some(RootKind::Resources) => (Icon::ResourceFolder, warm),
        Some(RootKind::TestResources) => (Icon::TestResourceFolder, mix(green, 0.5, warm)),
        Some(RootKind::Generated) => (Icon::GeneratedFolder, violet),
        None => (Icon::Folder, p.ill_ink),
    }
}

/// A letter in a wash of `color`, as the file icons draw them.
fn letter_badge(text: &'static str, color: Hsla, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .size(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(mix(color, if p.dark { 0.3 } else { 0.22 }, p.panel))
        .border_1()
        .border_color(mix(color, 0.45, p.panel))
        .text_color(color)
        .font_family(look::MONO)
        .font_weight(FontWeight::BOLD)
        .text_size(px(9.))
        .line_height(px(14.))
        .child(text)
        .into_any_element()
}

/// "~/dev/shop" for a folder in the home folder (`home`).
fn short_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home).ok().filter(|_| !home.as_os_str().is_empty()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// What one row draws: its glyph, label, quiet note, tooltip and marks.
struct Look {
    glyph: AnyElement,
    label: String,
    bold: bool,
    ink: Hsla,
    note: Option<String>,
    chip: Option<(String, Hsla)>,
    tip: Option<String>,
    badge: Option<(Icon, Hsla)>,
    mark: Option<GitMark>,
    /// The file a click opens.
    file: Option<PathBuf>,
    /// The folder or file the context menu acts on.
    path: Option<PathBuf>,
}

impl Render for ProjectView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let root = self.model.read(cx).project_root();
        if root != self.root {
            self.root = root.clone();
            self.expanded = HashSet::from([NodeId::Root]);
        }
        let Some(model) = self.project_model(cx) else {
            // The plain tree until the model is read, with a note saying so.
            let store = self.store.read(cx);
            let failed = store.sync_error.is_some();
            let note = match (&store.sync_error, root.is_some()) {
                (Some(e), true) => Some(format!("The Gradle model could not be read: {e}")),
                (None, true) if root.as_deref().is_some_and(|r| crate::project_model::project_has_gradle(r, cx)) => {
                    Some("Reading Gradle model".to_owned())
                }
                _ => None,
            };
            return div()
                .size_full()
                .flex()
                .flex_col()
                .when_some(note, |d, note| {
                    d.child(
                        div()
                            .id("project-model-note")
                            .debug_selector(|| "project-model-note".into())
                            .flex_none()
                            .h(px(22.))
                            .px(px(6.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(11.5))
                            .text_color(p.fg_dim)
                            .when(failed, |d| {
                                d.child(crate::components::icon(
                                    crate::components::IconName::TriangleAlert,
                                    13.,
                                    p.warn,
                                ))
                            })
                            .when(!failed, |d| d.child(gpui_component::spinner::Spinner::new().xsmall()))
                            .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(note)),
                    )
                })
                .child(div().flex_1().min_h_0().child(self.files.clone()))
                .into_any_element();
        };
        let rows = self.rows(cx);
        let home = crate::host::host(cx).info().home.clone();
        let m = self.model.read(cx);
        let marks = match m.repo_root() {
            Some(repo) => git_marks(&repo, &m.status.files),
            None => HashMap::new(),
        };
        let active = self.files.read(cx).active.clone();
        let guide = mix(p.fg, 0.1, p.panel);
        let [green, warm, blue, _, violet, _] = p.lanes();
        let items: Vec<AnyElement> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let look = row_look(&row, &model, &marks, &p, (green, warm, blue, violet), &home);
                self.row(i, row, look, active.as_ref(), guide, &p, cx)
            })
            .collect();
        div()
            .id("project-tree")
            .debug_selector(|| "project-tree".into())
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .children(items)
            .into_any_element()
    }
}

fn row_look(
    row: &Row,
    model: &ProjectModel,
    marks: &HashMap<PathBuf, GitMark>,
    p: &Palette,
    (green, warm, blue, violet): (Hsla, Hsla, Hsla, Hsla),
    home: &Path,
) -> Look {
    let folder = |g: Icon, c: Hsla| icon(g, 15., c).into_any_element();
    let mut look = Look {
        glyph: div().into_any_element(),
        label: String::new(),
        bold: false,
        ink: p.fg,
        note: None,
        chip: None,
        tip: None,
        badge: None,
        mark: None,
        file: None,
        path: None,
    };
    match &row.kind {
        RowKind::Root(module) => {
            look.label = model.name.clone();
            look.bold = true;
            look.glyph = folder(if module.is_some() { Icon::Module } else { Icon::Folder }, blue);
            look.note = Some(short_path(&model.root, home));
            look.path = Some(model.root.clone());
            if let Some(m) = module.map(|i| &model.modules[i]) {
                if !m.uses.is_empty() {
                    look.note = Some(format!("uses {}", m.uses.join(", ")));
                }
                look.tip = Some(format!("{}  {}", m.gradle_path, model.root.display()));
            }
        }
        RowKind::Module(i) => {
            let m = &model.modules[*i];
            look.label = m.name.clone();
            look.bold = true;
            look.glyph = folder(Icon::Module, blue);
            let mut notes: Vec<String> = m.build_note().map(str::to_owned).into_iter().collect();
            if !m.uses.is_empty() {
                notes.push(format!("uses {}", m.uses.join(", ")));
            }
            look.note = (!notes.is_empty()).then(|| notes.join("  ·  "));
            look.tip = Some(match &m.stale {
                Some(reason) => format!("{reason}\nShowing the last model that synced."),
                None => format!("{} {}\n{}", m.build, m.gradle_path, m.content_root.display()),
            });
            if m.stale.is_some() {
                look.badge = Some((Icon::Warning, p.warn));
            }
            look.path = Some(m.content_root.clone());
            look.mark = marks.get(&m.content_root).copied();
        }
        RowKind::Shared(i) => {
            let s = &model.shared[*i];
            look.label = s.name.clone();
            look.bold = true;
            look.glyph = folder(Icon::Module, violet);
            look.chip = Some(("shared".into(), violet));
            look.tip = Some(format!("Compiled by {}", s.modules.join(", ")));
            look.path = Some(s.content_root.clone());
            look.mark = marks.get(&s.content_root).copied();
        }
        RowKind::Failed(i) => {
            let f = &model.failed[*i];
            look.label = f.build.clone();
            look.bold = true;
            look.glyph = folder(Icon::Module, p.fg_dim);
            look.ink = p.fg_dim;
            look.note = f.gradle_version.as_ref().map(|v| format!("Gradle {v}"));
            look.badge = Some((Icon::Problems, p.error));
            look.tip = Some(format!("Did not sync: {}", f.reason));
            look.path = Some(f.root_dir.clone());
        }
        RowKind::Reason(i) => {
            let f = &model.failed[*i];
            look.label = f.reason.clone();
            look.ink = p.error;
            look.glyph = icon(Icon::Problems, 13., p.error).into_any_element();
            look.tip = Some(if f.error.is_empty() { f.reason.clone() } else { f.error.clone() });
        }
        RowKind::Dir { path, root, excluded, ignored } => {
            look.label = name_of(path);
            let mark = marks.get(path).copied().filter(|_| !ignored && !excluded);
            look.mark = mark;
            let (g, c) = root_look(*root, *excluded, p);
            let c = if *ignored && root.is_none() { p.fg_dim.opacity(0.6) } else { c };
            look.glyph = folder(g, c);
            look.ink = match mark {
                _ if *excluded || *ignored => p.fg_dim.opacity(0.6),
                Some(m) => mix(m.color(p), 0.8, p.fg),
                None => p.fg,
            };
            look.tip = Some(match (root, excluded) {
                (_, true) => format!("Excluded: {}", path.display()),
                (Some(k), _) => format!("{}: {}", k.label(), path.display()),
                _ => path.display().to_string(),
            });
            look.path = Some(path.clone());
        }
        RowKind::File { path, ignored } => {
            look.label = name_of(path);
            let mark = marks.get(path).copied().filter(|_| !ignored);
            look.mark = mark;
            look.ink = match mark {
                _ if *ignored => p.fg_dim.opacity(0.6),
                Some(m) => m.color(p),
                None => p.fg,
            };
            look.glyph = div()
                .opacity(if *ignored { 0.55 } else { 1. })
                .child(crate::icons::file_icon(path, 14., p))
                .into_any_element();
            look.tip = Some(path.display().to_string());
            look.file = Some(path.clone());
            look.path = Some(path.clone());
        }
        RowKind::Scripts => {
            look.label = "Gradle Scripts".into();
            look.glyph = icon(Icon::Build, 14., green).into_any_element();
        }
        RowKind::Script(i) => {
            let s = &model.scripts[*i];
            look.label = name_of(&s.path);
            // A module's own script names the module; others, the build that reads them.
            let module = s.path.parent().and_then(|dir| model.modules.iter().find(|m| m.content_root == dir));
            look.note = Some(match (s.kind, s.builds.len(), module) {
                (ScriptKind::Build | ScriptKind::Settings, _, Some(m)) if m.gradle_path != ":" => {
                    format!("{} {}", m.build, m.gradle_path)
                }
                (_, 0, _) => String::new(),
                (_, 1, _) => s.builds[0].clone(),
                (ScriptKind::Applied, n, _) => format!("{n} builds"),
                (_, n, _) => format!("{} and {} more", s.builds[0], n - 1),
            });
            look.glyph = crate::icons::file_icon(&s.path, 14., p);
            look.tip = Some(s.path.display().to_string());
            look.file = Some(s.path.clone());
            look.path = Some(s.path.clone());
            look.mark = marks.get(&s.path).copied();
            if let Some(m) = look.mark {
                look.ink = m.color(p);
            }
        }
        RowKind::Libraries => {
            look.label = "External Libraries".into();
            look.glyph = icon(Icon::Library, 14., warm).into_any_element();
        }
        RowKind::Jdk(i) => {
            let jdk = &model.jdks[*i];
            look.label = jdk.label();
            look.glyph = icon(Icon::Jdk, 14., warm).into_any_element();
            look.note = Some(short_path(&jdk.home, home));
            look.tip = Some(jdk.home.display().to_string());
            look.path = Some(jdk.home.clone());
        }
        RowKind::Library(i) => {
            let lib = &model.libraries[*i];
            look.label = lib.id.clone();
            look.glyph = icon(Icon::Jar, 14., p.fg_dim).into_any_element();
            look.tip = lib.file.as_ref().map(|f| f.display().to_string());
            look.path = lib.file.clone();
            if lib.sources.is_some() {
                look.note = Some("sources".into());
            }
        }
        RowKind::Package { name, .. } => {
            look.label = name.clone();
            look.glyph = icon(Icon::Package, 14., p.fg_dim).into_any_element();
        }
        RowKind::Class { package, name, .. } => {
            look.label = name.clone();
            look.glyph = letter_badge("C", blue, p);
            look.tip = Some(format!("{package}.{name}"));
        }
        RowKind::Note(text) => {
            look.label = text.clone();
            look.ink = p.fg_dim;
        }
    }
    look
}

/// The dot of the agent whose changes to `file` wait for review, in its colour.
pub fn agent_dot(file: Option<&Path>, p: &Palette, cx: &gpui::App) -> Option<gpui::AnyElement> {
    let agent = cx.try_global::<crate::claude_ide::AgentFiles>()?.0.get(file?)?;
    let color = crate::agents::by_id(agent, cx).color(p);
    Some(
        div()
            .debug_selector(|| "agent-file-dot".into())
            .flex_none()
            .ml(px(6.))
            .size(px(7.))
            .rounded_full()
            .bg(color)
            .into_any_element(),
    )
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl ProjectView {
    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        i: usize,
        row: Row,
        look: Look,
        active: Option<&PathBuf>,
        guide: Hsla,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = look.file.is_some() && look.file.as_ref() == active;
        let depth = row.depth;
        let expandable = row.id.is_some();
        let selector = format!("project-row-{}", look.label);
        let tip: Option<SharedString> = look.tip.map(Into::into);
        let model = self.model.clone();
        let menu_path = look.path.clone();
        let menu_file = look.file.is_some();
        crate::components::list_row_h(("project-row", i), crate::components::RowState::current(selected), ROW, p)
            .debug_selector(move || selector.clone())
            .relative()
            .gap_0()
            .pl(px(2. + depth as f32 * INDENT))
            .pr(px(4.))
            .cursor_pointer()
            .text_color(look.ink)
            .children(
                (0..depth).map(|d| {
                    div().absolute().top_0().bottom_0().left(px(2. + d as f32 * INDENT + 7.)).w(px(1.)).bg(guide)
                }),
            )
            .child(
                div().w(px(14.)).h_full().flex_none().when(expandable, |d| d.child(look::chevron(row.open, p.fg_dim))),
            )
            .child(div().w(px(2.)))
            .child(look.glyph)
            .child(
                div()
                    .min_w_0()
                    .ml(px(6.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .when(look.bold, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(look.label),
            )
            .when_some(look.badge, |d, (g, c)| d.child(div().flex_none().ml(px(5.)).child(icon(g, 12., c))))
            .when_some(agent_dot(look.file.as_deref(), p, cx), |d, dot| d.child(dot))
            .when_some(look.chip, |d, (text, c)| {
                d.child(
                    div()
                        .flex_none()
                        .ml(px(6.))
                        .px(px(5.))
                        .h(px(15.))
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .bg(mix(c, if p.dark { 0.22 } else { 0.14 }, p.panel))
                        .text_color(c)
                        .text_size(px(10.5))
                        .child(text),
                )
            })
            .when_some(look.note, |d, note| {
                // A folder or a file's place: shortened in its middle, whole in the tooltip.
                if note.starts_with('/') || note.starts_with('~') {
                    return d.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .ml(px(8.))
                            .h_full()
                            .flex()
                            .text_size(px(11.5))
                            .child(crate::components::path_text(("project-row-note", i), note, p.fg_dim)),
                    );
                }
                d.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .ml(px(8.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(p.fg_dim)
                        .text_size(px(11.5))
                        .child(note),
                )
            })
            .child(div().flex_1())
            .when_some(look.mark.filter(|_| look.file.is_some()), |d, m| {
                d.child(
                    div()
                        .flex_none()
                        .w(px(14.))
                        .text_size(px(11.))
                        .font_family(look::MONO)
                        .text_color(m.color(p))
                        .child(m.letter()),
                )
            })
            .when(look.mark.is_some() && look.file.is_none(), |d| {
                d.child(div().flex_none().mr(px(4.)).size(px(5.)).rounded_full().bg(look.ink.opacity(0.7)))
            })
            .when_some(tip, |d, tip| {
                d.tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
            })
            .on_click(cx.listener({
                let kind = row.kind.clone();
                let id = row.id.clone();
                let file = look.file.clone();
                move |this, e: &ClickEvent, _, cx| {
                    if let Some(id) = &id {
                        this.toggle(id.clone(), cx);
                    } else if let Some(path) = &file {
                        // One click looks at the file; two keep its tab open.
                        let a = if e.click_count() >= 2 {
                            Action::OpenFile(path.clone(), None)
                        } else {
                            Action::PreviewFile(path.clone())
                        };
                        this.model.update(cx, |m, cx| m.handle(a, cx));
                    } else if let RowKind::Class { package, name, .. } = &kind {
                        cx.emit(ProjectViewEvent::OpenClass { package: package.clone(), class: name.clone() });
                    }
                }
            }))
            .context_menu(move |menu, _, _| {
                let Some(path) = menu_path.clone() else { return menu };
                let send = |a: Action| {
                    let model = model.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        let a = a.clone();
                        model.update(cx, |m, cx| m.handle(a, cx))
                    }
                };
                let menu = if menu_file {
                    menu.item(
                        PopupMenuItem::new("Open")
                            .icon(crate::components::menu_icon(crate::components::IconName::File))
                            .on_click(send(Action::OpenFile(path.clone(), None))),
                    )
                    .separator()
                } else {
                    menu
                };
                menu.item(
                    PopupMenuItem::new("Reveal in Finder")
                        .icon(crate::components::menu_icon(crate::components::IconName::FolderOpen))
                        .on_click(send(Action::Reveal(path.clone()))),
                )
                .item(
                    PopupMenuItem::new("Open in External Terminal")
                        .icon(crate::components::menu_icon(crate::components::IconName::SquareTerminal))
                        .on_click(send(Action::OpenInTerminal(path.clone()))),
                )
                .item(
                    PopupMenuItem::new("Copy path")
                        .icon(crate::components::menu_icon(crate::components::IconName::Copy))
                        .on_click(send(Action::Copy(path.display().to_string()))),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_model::parse;

    /// Folders from a list of paths, as the file tree would list them.
    struct Fake(Vec<PathBuf>);

    impl Source for Fake {
        fn list(&mut self, dir: &Path) -> Vec<Entry> {
            let mut out: Vec<Entry> = self
                .0
                .iter()
                .flat_map(|p| p.ancestors())
                .filter(|p| p.parent() == Some(dir))
                .map(|p| Entry { path: p.to_owned(), dir: p.extension().is_none(), ignored: false })
                .collect();
            out.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.path.cmp(&b.path)));
            out.dedup();
            out
        }

        fn jar(&mut self, _: &Path) -> Option<Rc<JarIndex>> {
            let mut index = JarIndex::new();
            index.insert("com.google.common.base".into(), vec!["Strings".into()]);
            Some(Rc::new(index))
        }
    }

    fn fixture() -> (ProjectModel, Fake) {
        let root = Path::new("/w/shop");
        let text = include_str!("../tests/fixtures/project-model.json").replace("$ROOT", "/w/shop");
        let model = parse(&serde_json::from_str(&text.replace("$JARS", "/jars")).unwrap()).unwrap();
        let files = [
            "app/src/main/java/shop/App.java",
            "app/src/main/resources/app.yml",
            "app/src/test/java/shop/AppTest.java",
            "app/build/classes",
            "billing/src/main/java/billing/Bill.java",
            "core/src/main/java/core/Money.java",
            "legacy/build.gradle",
            "build.gradle",
            "settings.gradle",
        ];
        (model, Fake(files.iter().map(|f| root.join(f)).collect()))
    }

    fn labels(rows: &[Row], model: &ProjectModel) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let name = match &r.kind {
                    RowKind::Root(_) => model.name.clone(),
                    RowKind::Module(i) => format!("[{}]", model.modules[*i].name),
                    RowKind::Shared(i) => format!("[{} shared]", model.shared[*i].name),
                    RowKind::Failed(i) => format!("[{} failed]", model.failed[*i].build),
                    RowKind::Reason(_) => "reason".into(),
                    RowKind::Dir { path, root: Some(k), .. } => format!("{} ({k:?})", name_of(path)),
                    RowKind::Dir { path, excluded: true, .. } => format!("{} (excluded)", name_of(path)),
                    RowKind::Dir { path, .. } | RowKind::File { path, .. } => name_of(path),
                    RowKind::Scripts => "Gradle Scripts".into(),
                    RowKind::Script(i) => name_of(&model.scripts[*i].path),
                    RowKind::Libraries => "External Libraries".into(),
                    RowKind::Jdk(i) => model.jdks[*i].label(),
                    RowKind::Library(i) => model.libraries[*i].id.clone(),
                    RowKind::Package { name, .. } | RowKind::Class { name, .. } => name.clone(),
                    RowKind::Note(t) => t.clone(),
                };
                format!("{}{name}", "  ".repeat(r.depth))
            })
            .collect()
    }

    #[test]
    fn the_root_module_holds_the_others_and_shared_and_failed_builds_have_nodes() {
        let (model, mut fake) = fixture();
        let shown = labels(&rows(&model, &HashSet::from([NodeId::Root]), &mut fake), &model);
        assert_eq!(
            shown,
            [
                "shop",
                "  [app]",
                "  [billing]",
                "  [core shared]",
                "  [legacy failed]",
                "  build.gradle",
                "  settings.gradle",
                "Gradle Scripts",
                "External Libraries"
            ]
        );
    }

    #[test]
    fn a_module_shows_its_source_roots_by_kind() {
        let (model, mut fake) = fixture();
        let app = |p: &str| NodeId::Dir(PathBuf::from("/w/shop/app").join(p));
        let open = HashSet::from([
            NodeId::Root,
            NodeId::Module("shop:app".into()),
            app("src"),
            app("src/main"),
            app("src/test"),
        ]);
        let shown = labels(&rows(&model, &open, &mut fake), &model);
        assert!(shown.contains(&"        java (Main)".to_owned()), "{shown:#?}");
        assert!(shown.contains(&"        resources (Resources)".to_owned()));
        assert!(shown.contains(&"    build (excluded)".to_owned()));
        assert!(shown.iter().any(|l| l.ends_with("java (Test)")));
    }

    #[test]
    fn failed_builds_say_why_and_libraries_open_to_classes() {
        let (model, mut fake) = fixture();
        let open = HashSet::from([
            NodeId::Root,
            NodeId::Failed("legacy".into()),
            NodeId::Libraries,
            NodeId::Scripts,
            NodeId::Library("com.google.guava:guava:33.0.0-jre".into()),
            NodeId::Package("com.google.guava:guava:33.0.0-jre".into(), "com.google.common.base".into()),
        ]);
        let shown = labels(&rows(&model, &open, &mut fake), &model);
        let at = shown.iter().position(|l| l == "  [legacy failed]").unwrap();
        assert_eq!(shown[at + 1], "    reason");
        let libs = shown.iter().position(|l| l == "External Libraries").unwrap();
        assert_eq!(shown[libs + 1], "  JDK 21.0.2", "the JDKs come first");
        assert_eq!(shown[libs + 3], "  com.google.guava:guava:33.0.0-jre");
        assert_eq!(shown[libs + 4], "    com.google.common.base");
        assert_eq!(shown[libs + 5], "      Strings");
        let scripts = shown.iter().position(|l| l == "Gradle Scripts").unwrap();
        assert_eq!(shown[scripts + 1..scripts + 4], ["  settings.gradle", "  build.gradle", "  build.gradle"]);
    }

    #[test]
    fn revealing_a_file_opens_its_module_and_folders() {
        let (model, _) = fixture();
        let path = Path::new("/w/shop/app/src/main/java/shop/App.java");
        let ids = path_to(&model, path);
        assert!(ids.contains(&NodeId::Module("shop:app".into())));
        assert!(ids.contains(&NodeId::Dir(PathBuf::from("/w/shop/app/src/main/java/shop"))));
        assert!(!ids.contains(&NodeId::Dir(PathBuf::from("/w/shop/app"))));
        let shared = path_to(&model, Path::new("/w/shop/core/src/main/java/core/Money.java"));
        assert!(shared.contains(&NodeId::Shared(PathBuf::from("/w/shop/core"))));
    }
}
