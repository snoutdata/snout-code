//! The Project Structure window (⌘;, File ▸ Project Structure…): the project as Gradle and
//! SnoutCode configure it, on five pages in the Settings sheet's layout (a searchable tree on
//! the left, Cancel, Apply and OK at the foot).
//!
//! - **Project**: its name and folder, the JDK it runs on (a picker), its Java level, each
//!   build's Gradle version and the JVM Gradle ran on.
//! - **Modules**: every Gradle project of every build (included builds and `buildSrc` too) and
//!   its source sets: their source, resource, test and generated roots, output folders and Java
//!   levels.
//! - **Dependencies**: per module and scope, what is declared and what that brings in, the
//!   versions that lost a conflict, links between projects, a filter, and where each is
//!   declared.
//! - **SDKs**: the JDKs found on the machine and those added by hand; add one from a folder,
//!   remove one added, download one.
//! - **Facets**: Spring and Claude Code, as they stand, with links to their Settings pages.
//!
//! What SnoutCode owns is edited here: the project's JDK goes to its `.snout/settings.json`,
//! the JDKs added by hand to the user's settings. What Gradle owns is shown read only, with
//! "Edit in build file" opening the script, catalog or properties file at the line that
//! declares it (a sync is offered once it is saved, by the bar the window shows then). Nothing
//! is written that a sync would throw away. The model is read off the UI thread, from the file
//! the last sync wrote, through the window's host (so it works on a remote machine too).

pub mod data;
pub mod locate;
mod view;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, Task, UniformListScrollHandle, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::lang::java::{self, DownloadProgress, Jdk};
use crate::model::Model;
use crate::project_settings::ProjectSettings;
use crate::settings::Settings;
pub use data::{Scope, Structure};
pub use locate::{Place, Target};

/// A page of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Project,
    Modules,
    Dependencies,
    Facets,
    Sdks,
}

impl Page {
    pub const ALL: [Page; 5] = [Page::Project, Page::Modules, Page::Dependencies, Page::Facets, Page::Sdks];

    /// The page's spot drawing, at its head.
    pub fn art(self) -> &'static snout_core::art::Drawing {
        use snout_core::art::spots;
        match self {
            Page::Project => spots::no_project(),
            Page::Modules => spots::modules(),
            Page::Dependencies => spots::dependencies(),
            Page::Facets => spots::facets(),
            Page::Sdks => spots::jdk(),
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Project => "Project",
            Page::Modules => "Modules",
            Page::Dependencies => "Dependencies",
            Page::Facets => "Facets",
            Page::Sdks => "SDKs",
        }
    }

    pub fn icon(self) -> crate::components::IconName {
        use crate::components::IconName;
        match self {
            Page::Project => IconName::FolderCog,
            Page::Modules => IconName::Boxes,
            Page::Dependencies => IconName::Network,
            Page::Facets => IconName::Blocks,
            Page::Sdks => IconName::Coffee,
        }
    }

    /// The group it sits under in the tree.
    pub fn group(self) -> &'static str {
        match self {
            Page::Sdks => "Platform Settings",
            _ => "Project Settings",
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Page::Project => "name folder jdk java language level gradle version jvm wrapper",
            Page::Modules => {
                "source sets roots resources tests generated output folders target compatibility buildsrc included composite"
            }
            Page::Dependencies => {
                "libraries transitive conflicts versions scopes compile runtime test annotation processor catalog"
            }
            Page::Facets => "spring boot profiles configuration application claude code",
            Page::Sdks => "jdk java download add remove installed",
        }
    }

    /// Whether the tree's search `query` finds the page.
    pub fn matches(self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.title().to_lowercase().contains(&q)
            || self.group().to_lowercase().contains(&q)
            || self.keywords().contains(&q)
    }
}

/// What the window asks of the workspace.
pub enum StructureEvent {
    /// Put the copies in place: the user's settings and, when changed, the project's (and
    /// close when `close` says so).
    Apply {
        user: Box<Settings>,
        project: Option<ProjectSettings>,
        close: bool,
    },
    /// Close, and show this place (the changes put in place first).
    Open(Place),
    /// Close, and open the Settings sheet on this page.
    Settings(crate::settings_view::Page),
    /// Ask for a folder of a remote machine, for a JDK to add.
    PickRemoteJdk,
    Cancel,
}

/// How the model's reading goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Load {
    Loading,
    Ready(Arc<Structure>),
    /// No model was written for the project (it is not a Gradle build, or never synced).
    Missing,
    Failed(String),
}

/// A row of the Modules page's tree: a module, or one of its source sets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModuleSel {
    Module(String),
    SourceSet(String, String),
}

/// A JDK added by hand, as read from its folder (`None`: no JDK is there now).
#[derive(Clone, Debug, PartialEq)]
pub struct Added {
    pub home: PathBuf,
    pub jdk: Option<Jdk>,
}

pub struct ProjectStructureView {
    pub(crate) model: Entity<Model>,
    pub page: Page,
    pub search: Entity<InputState>,
    pub load: Load,
    /// The model file read, and when it was written.
    source: Option<(PathBuf, SystemTime)>,
    pub root: Option<PathBuf>,
    /// The project's own settings as the window shows them.
    pub project: ProjectSettings,
    /// The JDKs added by hand on this machine, as the window shows them.
    pub added: Vec<PathBuf>,
    /// The machine's name when it is a remote one (its added JDKs are kept under it).
    pub host_name: Option<String>,
    /// The JDKs found where JDKs usually are; `None` while they are looked for.
    pub detected: Option<Vec<Jdk>>,
    /// The added JDKs, each read from its folder.
    pub added_read: Vec<Added>,
    // The Modules page.
    pub selected: Option<ModuleSel>,
    pub folded: HashSet<String>,
    pub(crate) module_scroll: UniformListScrollHandle,
    // The Dependencies page.
    pub dep_module: Option<String>,
    pub scope: Scope,
    pub filter: Entity<InputState>,
    /// The dependency rows opened, by their path of nodes.
    pub open: HashSet<Vec<usize>>,
    pub conflicts_only: bool,
    pub(crate) dep_scroll: UniformListScrollHandle,
    // The SDKs page.
    pub selected_jdk: Option<PathBuf>,
    pub download: Option<(u32, DownloadProgress)>,
    /// A line saying what just happened, or what went wrong (true).
    pub note: Option<(String, bool)>,
    /// The Spring modules' configuration files.
    pub config_files: Vec<PathBuf>,
    /// The libraries the builds' version catalogs declare, by group and name.
    pub catalog: HashSet<(String, String)>,
    focus: FocusHandle,
    _read: Option<Task<()>>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl EventEmitter<StructureEvent> for ProjectStructureView {}

impl Focusable for ProjectStructureView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// The JDKs added by hand for the machine `host_name` (`None`: this Mac) in `s`.
pub fn added_jdks(s: &Settings, host_name: Option<&str>) -> Vec<PathBuf> {
    match host_name {
        None => s.sdks.clone(),
        Some(h) => s.remote_sdks.get(h).cloned().unwrap_or_default(),
    }
}

/// `s` with the JDKs added by hand for `host_name` set to `list`.
pub fn set_added_jdks(s: &mut Settings, host_name: Option<&str>, list: Vec<PathBuf>) {
    match host_name {
        None => s.sdks = list,
        Some(h) if list.is_empty() => {
            s.remote_sdks.remove(h);
        }
        Some(h) => {
            s.remote_sdks.insert(h.to_owned(), list);
        }
    }
}

/// The JDKs added by hand for the window's machine, from the settings in use.
pub fn window_added_jdks(cx: &App) -> Vec<PathBuf> {
    let name = crate::remote::connection(cx).map(|(n, _)| n);
    let model = crate::lang::hub::hub(cx).map(|h| h.read(cx).model.clone());
    model.map(|m| added_jdks(&m.read(cx).settings, name.as_deref())).unwrap_or_default()
}

/// The configuration files of Spring in the resource roots of the Spring modules:
/// `application*` and `bootstrap*` properties and YAML files.
pub fn spring_config_files(host: &dyn snout_host::Host, s: &Structure) -> Vec<PathBuf> {
    use crate::project_model::RootKind;
    let mut out = Vec::new();
    for m in s.spring_modules() {
        for set in &m.source_sets {
            for (dir, kind, _) in &set.roots {
                if !matches!(kind, RootKind::Resources | RootKind::TestResources) {
                    continue;
                }
                let Ok(entries) = host.read_dir(dir) else { continue };
                let mut files: Vec<PathBuf> =
                    entries.into_iter().filter(|e| !e.is_dir).map(|e| e.path).filter(|p| is_spring_config(p)).collect();
                files.sort();
                out.extend(files);
            }
        }
    }
    out
}

/// `application.yml`, `application-dev.properties`, `bootstrap.yaml`.
pub fn is_spring_config(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    (name.starts_with("application") || name.starts_with("bootstrap")) && matches!(ext, "properties" | "yml" | "yaml")
}

impl ProjectStructureView {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let m = model.read(cx);
        let root = m.project_root();
        let project = m.project_settings.clone();
        let host_name = crate::remote::connection(cx).map(|(n, _)| n);
        let added = added_jdks(&m.user_settings(), host_name.as_deref());
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by group, name or version"));
        let mut subscriptions = vec![
            cx.subscribe(&search, |this, input, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    let query = input.read(cx).value().to_string();
                    if !this.page.matches(&query)
                        && let Some(first) = Page::ALL.into_iter().find(|p| p.matches(&query))
                    {
                        this.page = first;
                    }
                    cx.notify();
                }
            }),
            cx.subscribe(&filter, |_, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        // A sync wrote a new model meanwhile: read it again.
        if let Some(store) = crate::project_model::store(cx) {
            subscriptions.push(cx.observe(&store, |this, _, cx| this.read_model(false, cx)));
        }
        // The Spring model's profiles and Claude Code's connection, as they change.
        if let Some(spring) = crate::spring::spring(cx) {
            subscriptions.push(cx.observe(&spring, |_, _, cx| cx.notify()));
        }
        subscriptions.push(cx.observe_global::<crate::claude_ide::IdeStatus>(|_, cx| cx.notify()));
        let mut this = Self {
            model,
            page: Page::Project,
            search,
            load: Load::Loading,
            source: None,
            root,
            project,
            added_read: added.iter().map(|h| Added { home: h.clone(), jdk: None }).collect(),
            added,
            host_name,
            detected: None,
            selected: None,
            folded: HashSet::new(),
            module_scroll: UniformListScrollHandle::new(),
            dep_module: None,
            scope: Scope::Compile,
            filter,
            open: HashSet::new(),
            conflicts_only: false,
            dep_scroll: UniformListScrollHandle::new(),
            selected_jdk: None,
            download: None,
            note: None,
            config_files: Vec::new(),
            catalog: HashSet::new(),
            focus: cx.focus_handle(),
            _read: None,
            _subscriptions: subscriptions,
        };
        this.read_model(true, cx);
        this.look_for_jdks(cx);
        this
    }

    /// Read the model the last sync wrote, when it is another than the one shown (or `force`).
    pub fn read_model(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(store) = crate::project_model::store(cx) else {
            self.load = Load::Missing;
            return;
        };
        let s = store.read(cx);
        let source = s.source().filter(|_| s.root == self.root);
        if !force && source == self.source {
            return;
        }
        let Some((file, when)) = source.clone() else {
            if s.syncing.is_none() && s.current.is_none() {
                self.load = Load::Missing;
                cx.notify();
            }
            return;
        };
        self.source = source;
        if !matches!(self.load, Load::Ready(_)) {
            self.load = Load::Loading;
        }
        let host = crate::host::host(cx);
        let read = cx.background_executor().spawn(async move {
            let s = data::read(&*host, &file)?;
            let files = spring_config_files(&*host, &s);
            let mut catalog = HashSet::new();
            let mut seen = HashSet::new();
            for m in &s.modules {
                if let Some((file, c)) = locate::catalog_of(&*host, &s, m)
                    && seen.insert(file)
                {
                    catalog.extend(c.libraries.into_iter().map(|l| (l.group, l.name)));
                }
            }
            Ok::<_, String>((s, files, catalog))
        });
        self._read = Some(cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                if this.source.as_ref().map(|(_, t)| *t) != Some(when) {
                    return;
                }
                match read {
                    Ok((s, files, catalog)) => {
                        this.set_structure(s, files);
                        this.catalog = catalog;
                    }
                    Err(e) => this.load = Load::Failed(e),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Show `s` (the tests give one), keeping what was picked where it still is.
    pub fn set_structure(&mut self, s: Structure, config_files: Vec<PathBuf>) {
        let keep = |id: &Option<String>| id.as_ref().is_some_and(|id| s.module(id).is_some());
        if !keep(&self.dep_module) {
            self.dep_module = s.jvm_modules().next().map(|m| m.id.clone());
            self.open.clear();
        }
        let selected_ok = match &self.selected {
            Some(ModuleSel::Module(id)) | Some(ModuleSel::SourceSet(id, _)) => s.module(id).is_some(),
            None => false,
        };
        if !selected_ok {
            self.selected = s.jvm_modules().next().or(s.modules.first()).map(|m| ModuleSel::Module(m.id.clone()));
        }
        self.config_files = config_files;
        self.load = Load::Ready(Arc::new(s));
    }

    pub fn structure(&self) -> Option<Arc<Structure>> {
        match &self.load {
            Load::Ready(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Look for the machine's JDKs, and read the added ones, off the UI thread.
    pub fn look_for_jdks(&mut self, cx: &mut Context<Self>) {
        let host = crate::host::host(cx);
        let home = host.info().home.clone();
        let added = self.added.clone();
        let look = cx.background_executor().spawn(async move {
            let found = java::discover(&*host, &home);
            let read: Vec<Added> = added
                .into_iter()
                .map(|h| Added { jdk: java::jdk_at(&*host, &h).or_else(|| jdk_below(&*host, &h)), home: h })
                .collect();
            (found, read)
        });
        cx.spawn(async move |this, cx| {
            let (found, read) = look.await;
            let _ = this.update(cx, |this, cx| {
                this.detected = Some(found);
                // Only those still listed: one may have been removed meanwhile.
                this.added_read = read.into_iter().filter(|a| this.added.contains(&a.home)).collect();
                cx.notify();
            });
        })
        .detach();
    }

    /// Every JDK the pickers offer: those found, then those added (not found already).
    pub fn all_jdks(&self) -> Vec<Jdk> {
        let mut out: Vec<Jdk> = self.detected.clone().unwrap_or_default();
        for a in &self.added_read {
            if let Some(j) = &a.jdk
                && !out.iter().any(|o| o.home == j.home)
            {
                out.push(j.clone());
            }
        }
        out
    }

    /// The settings differ from those in use.
    pub fn changed(&self, cx: &App) -> bool {
        let m = self.model.read(cx);
        self.project != m.project_settings || self.added != added_jdks(&m.user_settings(), self.host_name.as_deref())
    }

    /// Put the changes in place (and close when `close` says so).
    pub fn apply(&mut self, close: bool, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let mut user = m.user_settings();
        set_added_jdks(&mut user, self.host_name.as_deref(), self.added.clone());
        let project = (self.project != m.project_settings).then(|| self.project.clone());
        cx.emit(StructureEvent::Apply { user: Box::new(user), project, close });
    }

    /// Run the project on `home` (none: each build's own JDK, as the user's settings say).
    pub fn set_jdk(&mut self, home: Option<PathBuf>, cx: &mut Context<Self>) {
        self.project.jdk = home;
        cx.notify();
    }

    /// Add the JDK in `dir` (its home, or a folder holding one, such as a macOS bundle).
    pub fn add_jdk(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let host = crate::host::host(cx);
        let read = cx
            .background_executor()
            .spawn(async move { java::jdk_at(&*host, &dir).or_else(|| jdk_below(&*host, &dir)).ok_or(dir) });
        cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(jdk) => {
                        let known = this.added.contains(&jdk.home)
                            || this.detected.as_ref().is_some_and(|d| d.iter().any(|j| j.home == jdk.home));
                        if known {
                            this.note = Some((format!("JDK {} is listed already", jdk.version), false));
                        } else {
                            this.note = Some((format!("JDK {} added. Apply keeps it.", jdk.version), false));
                            this.added.push(jdk.home.clone());
                            this.added_read.push(Added { home: jdk.home.clone(), jdk: Some(jdk.clone()) });
                        }
                        this.selected_jdk = Some(jdk.home);
                    }
                    Err(dir) => {
                        this.note = Some((
                            format!(
                                "No JDK in {}: pick the folder holding the bin folder with Java in it",
                                dir.display()
                            ),
                            true,
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Ask for a folder with a JDK in it: the Mac's dialog, or the remote machine's picker.
    pub fn pick_jdk_folder(&mut self, cx: &mut Context<Self>) {
        if crate::remote::is_remote(cx) {
            cx.emit(StructureEvent::PickRemoteJdk);
            return;
        }
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add JDK".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(dir) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.add_jdk(dir, cx));
            }
        })
        .detach();
    }

    /// Take a JDK added by hand off the list (its files stay where they are).
    pub fn remove_jdk(&mut self, home: &Path, cx: &mut Context<Self>) {
        self.added.retain(|h| h != home);
        self.added_read.retain(|a| a.home != home);
        if self.project.jdk.as_deref() == Some(home) {
            self.project.jdk = None;
        }
        if self.selected_jdk.as_deref() == Some(home) {
            self.selected_jdk = None;
        }
        self.note = Some(("Removed from the list. Apply keeps the change.".into(), false));
        cx.notify();
    }

    /// Download JDK `major` (Eclipse Temurin) where SnoutCode keeps the JDKs it downloads, as
    /// the bar offering a missing JDK does.
    pub fn download_jdk(&mut self, major: u32, cx: &mut Context<Self>) {
        if self.download.is_some() {
            return;
        }
        let progress = DownloadProgress::default();
        self.download = Some((major, progress.clone()));
        self.note = None;
        let host = crate::host::host(cx);
        let dir = java::downloads_dir(host.info());
        let job = cx.background_executor().spawn(async move { java::download_temurin(&*host, major, &dir, &progress) });
        cx.spawn(async move |this, cx| {
            let ticker = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    loop {
                        cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            });
            let result = job.await;
            drop(ticker);
            let _ = this.update(cx, |this, cx| {
                this.download = None;
                match result {
                    Ok(jdk) => {
                        this.note = Some((format!("JDK {} downloaded to {}", jdk.version, jdk.home.display()), false));
                        this.selected_jdk = Some(jdk.home.clone());
                        this.look_for_jdks(cx);
                    }
                    Err(e) => this.note = Some((format!("The download failed: {e:#}"), true)),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Find where `target` of the module `module` is declared, off the UI thread, and show it.
    pub fn edit_in_build_file(&mut self, module: String, target: Target, cx: &mut Context<Self>) {
        let Some(s) = self.structure() else { return };
        let host = crate::host::host(cx);
        let s2 = s.clone();
        let find = cx.background_executor().spawn(async move {
            let m = s2.module(&module)?;
            locate::find(&*host, &s2, m, &target)
        });
        self.after_find(find, cx);
    }

    /// Show where the version catalog declares `group:name`, for the module `module`.
    pub fn show_in_catalog(&mut self, module: String, group: String, name: String, cx: &mut Context<Self>) {
        let Some(s) = self.structure() else { return };
        let host = crate::host::host(cx);
        let s2 = s.clone();
        let find = cx.background_executor().spawn(async move {
            let m = s2.module(&module)?;
            locate::catalog_place(&*host, &s2, m, &group, &name)
        });
        self.after_find(find, cx);
    }

    fn after_find(&mut self, find: Task<Option<Place>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let place = find.await;
            let _ = this.update(cx, |this, cx| match place {
                Some(place) => {
                    if this.changed(cx) {
                        this.apply(false, cx);
                    }
                    cx.emit(StructureEvent::Open(place));
                }
                None => {
                    this.note = Some(("The build file could not be found".into(), true));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Open a file of the project (a configuration file, a build script).
    pub fn open_file(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        if self.changed(cx) {
            self.apply(false, cx);
        }
        cx.emit(StructureEvent::Open(Place { file, line: None }));
    }

    /// Open the Settings sheet on `page` (the changes put in place first).
    pub fn open_settings(&mut self, page: crate::settings_view::Page, cx: &mut Context<Self>) {
        if self.changed(cx) {
            self.apply(false, cx);
        }
        cx.emit(StructureEvent::Settings(page));
    }

    /// Show the dependencies of the module `id`.
    pub fn show_dependencies_of(&mut self, id: String, cx: &mut Context<Self>) {
        self.page = Page::Dependencies;
        if self.dep_module.as_ref() != Some(&id) {
            self.dep_module = Some(id);
            self.open.clear();
        }
        cx.notify();
    }

    /// Show the module `id` on the Modules page.
    pub fn show_module(&mut self, id: String, cx: &mut Context<Self>) {
        self.page = Page::Modules;
        self.selected = Some(ModuleSel::Module(id));
        cx.notify();
    }

    /// Sync the Gradle model again.
    pub fn sync(&mut self, cx: &mut Context<Self>) {
        if let Some(store) = crate::project_model::store(cx) {
            store.update(cx, |s, cx| s.sync(cx));
        }
        self.note = Some(("Syncing the Gradle model. The pages follow when it is done.".into(), false));
        cx.notify();
    }
}

/// A JDK in a folder below `dir`: a macOS bundle's `Contents/Home`, or the one JDK a folder
/// of JDKs holds.
fn jdk_below(host: &dyn snout_host::Host, dir: &Path) -> Option<Jdk> {
    java::jdk_at(host, &dir.join("Contents/Home")).or_else(|| {
        let list = java::jdks_in(host, dir);
        (list.len() == 1).then(|| list.into_iter().next()).flatten()
    })
}
