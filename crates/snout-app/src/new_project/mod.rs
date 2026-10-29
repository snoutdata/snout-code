//! The New Project dialog (File ▸ New Project…, and the Welcome page): a Gradle project,
//! either plain Java, written here and so made without the network, or Spring Boot, from
//! start.spring.io with its versions and dependencies (and, should start.spring.io not
//! answer, from a template of our own). The new project then opens.

pub mod generate;
pub mod initializr;

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Task, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::look;
use crate::settings_view::{choice, row, section};
use crate::theme::{mix, palette};
use generate::{Basics, Boot, Dsl};
use initializr::Catalog;

/// The kinds of project the dialog makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Java,
    SpringBoot,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Java, Kind::SpringBoot];

    pub fn title(self) -> &'static str {
        match self {
            Kind::Java => "Java",
            Kind::SpringBoot => "Spring Boot",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Kind::Java => "An application, with Gradle",
            Kind::SpringBoot => "From start.spring.io",
        }
    }
}

/// The Java versions a plain Java project can target.
pub const JAVA_VERSIONS: [u32; 5] = [25, 21, 17, 11, 8];

pub enum NewProjectEvent {
    /// The project is made in this folder: open it.
    Created(PathBuf),
    Cancel,
}

pub struct NewProjectView {
    pub kind: Kind,
    pub name: Entity<InputState>,
    pub location: Entity<InputState>,
    pub group: Entity<InputState>,
    pub artifact: Entity<InputState>,
    pub package: Entity<InputState>,
    pub dep_search: Entity<InputState>,
    pub dsl: Dsl,
    /// The Java level of a plain Java project, and of a Spring Boot one (none: the default).
    pub java: u32,
    pub boot_java: Option<u32>,
    /// The Spring Boot version chosen (none: the default) and the dependencies, by id.
    pub boot: Option<String>,
    pub deps: Vec<String>,
    /// Create a Git repository in the new folder.
    pub git: bool,
    /// A Spring Boot project's second page, its dependencies, shows.
    pub picking: bool,
    /// start.spring.io's offer, once it answered (or the offline list, and why).
    pub catalog: Option<Catalog>,
    pub catalog_error: Option<String>,
    pub loading: bool,
    /// The project is being made.
    pub creating: bool,
    /// Whether the project's folder is taken, as the host said for the folder named then.
    occupied: Option<(PathBuf, bool)>,
    _occupied: Option<Task<()>>,
    /// Why the last Create failed; with `offer_offline`, a Spring Boot project can be made
    /// here instead.
    pub error: Option<String>,
    pub offer_offline: bool,
    /// start.spring.io, or a stand-in.
    pub service: String,
    cache: Option<PathBuf>,
    /// The artifact and package the placeholders show.
    derived: (String, String),
    focus: FocusHandle,
    _task: Option<Task<()>>,
}

impl EventEmitter<NewProjectEvent> for NewProjectView {}

impl Focusable for NewProjectView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl NewProjectView {
    /// The dialog, the new project to go in `location` by default; `cache` keeps what
    /// start.spring.io last said.
    pub fn new(location: PathBuf, cache: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |text: &str, placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let (text, placeholder) = (text.to_owned(), placeholder.to_owned());
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(text));
            cx.subscribe_in(&state, window, |this, _, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) {
                    this.error = None;
                    this.offer_offline = false;
                    cx.notify();
                }
            })
            .detach();
            state
        };
        let name = input("untitled", "Project name", window, cx);
        let location = input(&location.display().to_string(), "Folder", window, cx);
        let group = input("com.example", "com.example", window, cx);
        let artifact = input("", "", window, cx);
        let package = input("", "", window, cx);
        let dep_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search dependencies"));
        cx.subscribe(&dep_search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let service = cx.try_global::<ServiceGlobal>().map(|s| s.0.clone());
        let mut this = Self {
            kind: Kind::Java,
            name,
            location,
            group,
            artifact,
            package,
            dep_search,
            dsl: Dsl::Kotlin,
            java: 21,
            boot_java: None,
            boot: None,
            deps: Vec::new(),
            git: true,
            picking: false,
            catalog: None,
            catalog_error: None,
            loading: false,
            creating: false,
            occupied: None,
            _occupied: None,
            error: None,
            offer_offline: false,
            service: service.unwrap_or_else(|| initializr::START_SPRING_IO.to_owned()),
            cache,
            derived: Default::default(),
            focus: cx.focus_handle(),
            _task: None,
        };
        this.derive(window, cx);
        this
    }

    /// The artifact and package placeholders, if what they derive from changed.
    fn derive_if_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let artifact = self.artifact_of(cx);
        let now = (artifact.clone(), generate::package_for(&self.text(&self.group, cx), &artifact));
        if now != self.derived {
            self.derive(window, cx);
        }
    }

    /// The artifact and package fields show what they get when left empty.
    fn derive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let artifact = self.artifact_of(cx);
        let package = generate::package_for(&self.text(&self.group, cx), &artifact);
        self.derived = (artifact.clone(), package.clone());
        self.artifact.update(cx, |i, cx| i.set_placeholder(artifact, window, cx));
        self.package.update(cx, |i, cx| i.set_placeholder(package, window, cx));
    }

    fn text(&self, input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().trim().to_owned()
    }

    /// The artifact: as typed, else the name in lower case with dashes.
    fn artifact_of(&self, cx: &App) -> String {
        let typed = self.text(&self.artifact, cx);
        if !typed.is_empty() {
            return typed;
        }
        let name = self.text(&self.name, cx).to_lowercase();
        name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect()
    }

    /// What the form asks for, as it stands.
    pub fn basics(&self, cx: &App) -> Basics {
        let artifact = self.artifact_of(cx);
        let group = self.text(&self.group, cx);
        let package = match self.text(&self.package, cx) {
            p if p.is_empty() => generate::package_for(&group, &artifact),
            p => p,
        };
        let java = match self.kind {
            Kind::Java => self.java,
            Kind::SpringBoot => self.boot_java.or(self.catalog.as_ref().map(|c| c.java_default)).unwrap_or(17),
        };
        Basics { name: self.text(&self.name, cx), group, artifact, package, java, dsl: self.dsl }
    }

    pub fn boot(&self) -> Boot {
        let version = self
            .boot
            .clone()
            .or_else(|| self.catalog.as_ref().map(|c| c.boot_default.clone()))
            .unwrap_or_else(|| generate::OFFLINE_BOOT.to_owned());
        Boot { version, dependencies: self.deps.clone() }
    }

    /// The folder the project goes in.
    pub fn target(&self, cx: &App) -> PathBuf {
        let location = PathBuf::from(expand_home(&self.text(&self.location, cx), cx));
        location.join(self.text(&self.name, cx))
    }

    /// Ask the host, off the UI thread, whether the project's folder is taken, unless that is
    /// known for the folder named now.
    fn check_target(&mut self, cx: &mut Context<Self>) {
        let target = self.target(cx);
        if self.occupied.as_ref().is_some_and(|(t, _)| *t == target) || !target.is_absolute() {
            return;
        }
        self.occupied = Some((target.clone(), false));
        let host = crate::host::host(cx);
        let look = cx.background_executor().spawn({
            let target = target.clone();
            async move { generate::occupied(&*host, &target) }
        });
        self._occupied = Some(cx.spawn(async move |this, cx| {
            let taken = look.await;
            let _ = this.update(cx, |this, cx| {
                if this.occupied.as_ref().is_some_and(|(t, _)| *t == target) {
                    this.occupied = Some((target, taken));
                    cx.notify();
                }
            });
        }));
    }

    /// Why Create cannot go ahead, if it cannot.
    pub fn problem(&self, cx: &App) -> Option<String> {
        let location = PathBuf::from(expand_home(&self.text(&self.location, cx), cx));
        let target = self.target(cx);
        let occupied = self.occupied.as_ref().filter(|(t, _)| *t == target).map(|(_, taken)| *taken);
        if let Some(p) = generate::problem(&self.basics(cx), &location, occupied) {
            return Some(p);
        }
        if self.kind == Kind::SpringBoot && self.catalog.is_none() {
            return Some("Waiting for start.spring.io…".into());
        }
        None
    }

    pub fn set_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.picking = false;
        self.error = None;
        self.offer_offline = false;
        if kind == Kind::SpringBoot && self.catalog.is_none() && !self.loading {
            self.load_catalog(cx);
        }
        cx.notify();
    }

    /// Ask start.spring.io what it offers; without an answer, the offline list stands in.
    pub fn load_catalog(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.catalog_error = None;
        let (service, cache) = (self.service.clone(), self.cache.clone());
        let fetch = cx.background_spawn(async move { initializr::fetch(&service, cache.as_deref()) });
        let cache = self.cache.clone();
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(c) => this.catalog = Some(c),
                    Err(e) => {
                        this.catalog_error = Some(format!("start.spring.io could not be reached ({e})."));
                        this.catalog = Some(Catalog::offline(cache.as_deref().and_then(initializr::cached_boot)));
                    }
                }
                this.boot = None;
                this.boot_java = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Make the project; `offline` makes a Spring Boot one from the template here.
    pub fn create(&mut self, offline: bool, cx: &mut Context<Self>) {
        if self.creating || self.problem(cx).is_some() {
            return;
        }
        let (kind, basics, boot, dir, git) = (self.kind, self.basics(cx), self.boot(), self.target(cx), self.git);
        let offline = offline || self.catalog.as_ref().is_some_and(|c| c.offline);
        let service = self.service.clone();
        let host = crate::host::host(cx);
        self.creating = true;
        self.error = None;
        self.offer_offline = false;
        let work = cx.background_spawn({
            let dir = dir.clone();
            async move {
                if generate::occupied(&*host, &dir) {
                    anyhow::bail!("{}", generate::occupied_problem(&dir));
                }
                match kind {
                    Kind::Java => generate::write_java(&*host, &dir, &basics),
                    Kind::SpringBoot if offline => generate::write_boot_offline(&*host, &dir, &basics, &boot),
                    Kind::SpringBoot => generate::download_boot(&*host, &service, &dir, &basics, &boot),
                }?;
                if git && snout_core::Repo::open(&host, &dir).is_err() {
                    snout_core::Repo::init(&host, &dir).map_err(|e| anyhow::anyhow!("{e}"))?;
                }
                anyhow::Ok(())
            }
        });
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.creating = false;
                match result {
                    Ok(()) => cx.emit(NewProjectEvent::Created(dir)),
                    Err(e) => {
                        this.offer_offline = kind == Kind::SpringBoot && !offline;
                        this.error = Some(format!("{e:#}"));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn pick_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        let location = self.location.clone();
        cx.spawn_in(window, async move |_, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(dir) = paths.into_iter().next()
            {
                let _ = cx.update(|window, cx| {
                    location.update(cx, |i, cx| i.set_value(dir.display().to_string(), window, cx));
                });
            }
        })
        .detach();
    }

    fn toggle_dep(&mut self, id: String, cx: &mut Context<Self>) {
        match self.deps.iter().position(|d| *d == id) {
            Some(i) => {
                self.deps.remove(i);
            }
            None => self.deps.push(id),
        }
        cx.notify();
    }
}

/// Where the dialog asks for start.spring.io's metadata, when not start.spring.io itself.
pub struct ServiceGlobal(pub String);

impl gpui::Global for ServiceGlobal {}

/// `~/x` with the home folder (on the host) spelled out.
fn expand_home(s: &str, cx: &App) -> String {
    let host = crate::host::host(cx);
    let home = &host.info().home;
    match s.strip_prefix("~/") {
        Some(rest) if !home.as_os_str().is_empty() => home.join(rest).display().to_string(),
        _ => s.to_owned(),
    }
}

impl NewProjectView {
    fn form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.picking && self.kind == Kind::SpringBoot && self.catalog.is_some() {
            return self.dependencies(cx);
        }
        let p = palette(cx);
        let view = cx.entity();
        let target = self.target(cx);
        let mut body = div()
            .flex()
            .flex_col()
            .child(row("Name", div().w(px(360.)).child(look::field(&self.name, false, None, &p)), &p))
            .child(row(
                "Location",
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().w(px(360.)).child(look::field(&self.location, false, None, &p)))
                    .child(
                        look::small_button(
                            "new-project-browse",
                            crate::components::IconName::FolderOpen,
                            "Choose…",
                            &p,
                        )
                        .px(px(8.))
                        .h(px(24.))
                        .on_click(cx.listener(|this, _, w, cx| this.pick_location(w, cx))),
                    ),
                &p,
            ))
            .child(
                div()
                    .pl(px(162.))
                    .text_size(px(12.))
                    .text_color(p.fg_dim)
                    .child(format!("The project goes in {}", target.display())),
            );
        if self.kind == Kind::SpringBoot && self.catalog.is_none() {
            return body
                .child(div().pt(px(24.)).text_color(p.fg_dim).child("Asking start.spring.io what it offers…"))
                .into_any_element();
        }
        // The build script's language.
        let dsl = |d: Dsl, label: &'static str, id: &'static str, cx: &mut Context<Self>| {
            look::toggle(id, crate::components::IconName::FileCode, label, self.dsl == d, &p)
                .debug_selector(move || id.to_owned())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.dsl = d;
                    cx.notify();
                }))
        };
        body = body.child(section("Build", &p)).child(row(
            "Build script",
            div().flex().gap(px(4.)).child(dsl(Dsl::Kotlin, "Kotlin", "new-project-kotlin", cx)).child(dsl(
                Dsl::Groovy,
                "Groovy",
                "new-project-groovy",
                cx,
            )),
            &p,
        ));
        let (javas, java_now): (Vec<u32>, u32) = match (self.kind, &self.catalog) {
            (Kind::SpringBoot, Some(c)) => (c.java.clone(), self.boot_java.unwrap_or(c.java_default)),
            _ => (JAVA_VERSIONS.to_vec(), self.java),
        };
        let pick_java = view.clone();
        let kind = self.kind;
        body = body.child(row(
            "Java",
            choice(
                "new-project-java",
                &java_now.to_string(),
                javas.iter().map(|j| (j.to_string(), format!("Java {j}"))).collect(),
                move |v, cx| {
                    let Ok(j) = v.parse::<u32>() else { return };
                    pick_java.update(cx, |this, cx| {
                        match kind {
                            Kind::Java => this.java = j,
                            Kind::SpringBoot => this.boot_java = Some(j),
                        }
                        cx.notify();
                    })
                },
                &p,
            ),
            &p,
        ));
        if let (Kind::SpringBoot, Some(c)) = (self.kind, &self.catalog) {
            let now = self.boot.clone().unwrap_or(c.boot_default.clone());
            let pick = view.clone();
            body = body.child(row(
                "Spring Boot",
                choice(
                    "new-project-boot",
                    &now,
                    c.boot.clone(),
                    move |v, cx| {
                        pick.update(cx, |this, cx| {
                            this.boot = Some(v);
                            cx.notify();
                        })
                    },
                    &p,
                ),
                &p,
            ));
        }
        body = body
            .child(section("Coordinates", &p))
            .child(row("Group", div().w(px(360.)).child(look::field(&self.group, false, None, &p)), &p))
            .child(row("Artifact", div().w(px(360.)).child(look::field(&self.artifact, false, None, &p)), &p))
            .child(row("Package", div().w(px(360.)).child(look::field(&self.package, false, None, &p)), &p));
        body.child(
            div().pt(px(14.)).child(
                look::checkbox("new-project-git", "Create a Git repository", self.git, true, &p)
                    .debug_selector(|| "new-project-git".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.git = !this.git;
                        cx.notify();
                    })),
            ),
        )
        .into_any_element()
    }

    /// The dependencies: those chosen as chips, then the searchable list in its groups.
    fn dependencies(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let Some(catalog) = self.catalog.clone() else { return div().into_any_element() };
        let query = self.dep_search.read(cx).value().trim().to_lowercase();
        let hover = p.button_hover;
        let mut chosen = div().flex().flex_wrap().gap(px(4.)).min_h(px(22.));
        if self.deps.is_empty() {
            chosen = chosen.child(div().text_size(px(12.)).text_color(p.fg_dim).child("None chosen yet."));
        }
        for (i, id) in self.deps.iter().enumerate() {
            let name = catalog.dependency(id).map(|d| d.name.clone()).unwrap_or_else(|| id.clone());
            let id2 = id.clone();
            chosen = chosen.child(
                div()
                    .id(("new-project-chosen", i))
                    .debug_selector(move || format!("new-project-chosen-{i}"))
                    .h(px(22.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(11.))
                    .bg(mix(p.accent, 0.25, p.card))
                    .cursor_pointer()
                    .child(name)
                    .child(div().text_color(p.fg_dim).child("×"))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_dep(id2.clone(), cx))),
            );
        }
        let mut list = div().flex().flex_col();
        let mut ix = 0usize;
        let mut any = false;
        for (group, deps) in &catalog.groups {
            let shown: Vec<_> = deps
                .iter()
                .filter(|d| {
                    query.is_empty()
                        || d.name.to_lowercase().contains(&query)
                        || d.id.contains(&query)
                        || d.description.to_lowercase().contains(&query)
                })
                .collect();
            if shown.is_empty() {
                continue;
            }
            any = true;
            list = list.child(
                div()
                    .flex_none()
                    .pt(px(6.))
                    .pb(px(2.))
                    .px(px(6.))
                    .text_size(px(11.5))
                    .text_color(p.fg_dim)
                    .font_weight(FontWeight::MEDIUM)
                    .child(group.clone()),
            );
            for d in shown {
                let on = self.deps.contains(&d.id);
                let id = d.id.clone();
                let sel = format!("new-project-dep-{}", d.id);
                list = list.child(
                    div()
                        .id(("new-project-dep", ix))
                        .debug_selector(move || sel.clone())
                        .flex_none()
                        .px(px(6.))
                        .py(px(3.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(
                            div()
                                .flex_none()
                                .size(px(14.))
                                .rounded(px(3.))
                                .border_1()
                                .border_color(if on { p.accent } else { mix(p.fg, 0.2, p.card) })
                                .when(on, |d| d.bg(p.accent))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(on, |d| d.child(icon(Icon::Check, 10., p.on_accent))),
                        )
                        .child(div().flex_none().child(d.name.clone()))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .text_color(p.fg_dim)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(d.description.clone()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_dep(id.clone(), cx))),
                );
                ix += 1;
            }
        }
        if !any {
            list = list.child(div().p(px(6.)).text_color(p.fg_dim).child("No dependency matches."));
        }
        let mut out = div().flex().flex_col().gap(px(6.)).child(section("Dependencies", &p)).child(
            div().text_size(px(12.)).text_color(p.fg_dim).child(format!(
                "Spring Boot {}, Java {}. Choose what the application uses; more can be added to the build later.",
                self.boot().version,
                self.basics(cx).java
            )),
        );
        if let Some(why) = &self.catalog_error {
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(12.))
                    .text_color(p.warn)
                    .child(format!(
                        "{why} These are the dependencies SnoutCode knows; the project is made here, and Gradle \
                         downloads Spring Boot once there is a network."
                    ))
                    .child(
                        look::small_button(
                            "new-project-retry",
                            crate::components::IconName::RotateCcw,
                            "Try Again",
                            &p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.load_catalog(cx))),
                    ),
            );
        }
        out.child(chosen)
            .child(look::field(&self.dep_search, true, None, &p))
            .child(
                div()
                    .id("new-project-deps")
                    .debug_selector(|| "new-project-deps".into())
                    .h(px(330.))
                    .border_1()
                    .border_color(mix(p.border, 0.7, p.card))
                    .rounded(px(5.))
                    .p(px(2.))
                    .overflow_y_scrollbar()
                    .child(list),
            )
            .into_any_element()
    }
}

impl Render for NewProjectView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.derive_if_changed(window, cx);
        self.check_target(cx);
        let p = palette(cx);
        let problem = self.problem(cx);
        let can_create = problem.is_none() && !self.creating;
        let hover = p.button_hover;
        let kinds: Vec<AnyElement> = Kind::ALL
            .into_iter()
            .map(|k| {
                let on = k == self.kind;
                let id = format!("new-project-kind-{}", k.title().to_lowercase().replace(' ', "-"));
                let sel = id.clone();
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .px(px(10.))
                    .py(px(7.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.row_selected()))
                    .when(!on, |d| d.hover(move |s| s.bg(hover)))
                    .child(icon(if k == Kind::Java { Icon::Module } else { Icon::Leaf }, 16., p.fg_dim))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::MEDIUM).child(k.title()))
                            .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(k.note())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.set_kind(k, cx)))
                    .into_any_element()
            })
            .collect();
        let button = |id: &'static str, text: &'static str, primary: bool, enabled: bool| {
            use crate::components::{CANCEL_ICON, IconName, footer_button};
            let ico = match id {
                "new-project-create" => IconName::FolderPlus,
                "new-project-next" => IconName::ArrowRight,
                "new-project-back" => IconName::ArrowLeft,
                "new-project-offline" => IconName::WifiOff,
                _ => CANCEL_ICON,
            };
            footer_button(id, ico, text, primary, enabled, &p)
                .when(id == "new-project-cancel", |d| d.child(crate::components::kbd("Esc", p.fg_dim)))
        };
        let status: Option<(String, gpui::Hsla)> = if self.creating {
            Some((
                match self.kind {
                    Kind::SpringBoot if !self.catalog.as_ref().is_some_and(|c| c.offline) => {
                        "Getting the project from start.spring.io…".into()
                    }
                    _ => "Creating the project…".into(),
                },
                p.fg_dim,
            ))
        } else if let Some(e) = &self.error {
            Some((e.clone(), p.error))
        } else {
            problem.clone().filter(|_| self.kind == Kind::Java || self.catalog.is_some()).map(|t| (t, p.fg_dim))
        };
        let form = self.form(cx);
        div()
            .id("new-project")
            .debug_selector(|| "new-project".into())
            .key_context("NewProject")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(NewProjectEvent::Cancel)))
            .occlude()
            .w(px(860.))
            .h(px(640.))
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
                    crate::components::Tone::Accent,
                    crate::components::IconName::FolderPlus,
                    "New Project",
                    Some("A Java or Spring Boot project, built with Gradle.".into()),
                    &p,
                )
                .pb(px(12.))
                .border_b_1()
                .border_color(p.border),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(220.))
                            .flex_none()
                            .h_full()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .p(px(8.))
                            .bg(p.ground)
                            .border_r_1()
                            .border_color(p.border)
                            .children(kinds),
                    )
                    .child(
                        div()
                            .id("new-project-form")
                            .flex_1()
                            .min_w_0()
                            .px(px(20.))
                            .py(px(12.))
                            .overflow_y_scrollbar()
                            .child(form),
                    ),
            )
            .child(
                div()
                    .min_h(px(56.))
                    .flex_none()
                    .px(px(20.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.dialog_footer())
                    .child(
                        div()
                            .id("new-project-status")
                            .debug_selector(|| "new-project-status".into())
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .children(status.map(|(t, c)| div().text_color(c).child(t))),
                    )
                    .when(self.offer_offline && !self.creating, |d| {
                        d.child(
                            button("new-project-offline", "Create Without the Network", false, true)
                                .on_click(cx.listener(|this, _, _, cx| this.create(true, cx))),
                        )
                    })
                    .child(
                        button("new-project-cancel", "Cancel", false, true)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(NewProjectEvent::Cancel))),
                    )
                    .when(self.kind == Kind::SpringBoot && self.picking, |d| {
                        d.child(button("new-project-back", "Previous", false, !self.creating).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.picking = false;
                                cx.notify();
                            },
                        )))
                    })
                    .map(|d| {
                        if self.kind == Kind::SpringBoot && !self.picking {
                            d.child(button("new-project-next", "Next", true, can_create).when(can_create, |d| {
                                d.on_click(cx.listener(|this, _, _, cx| {
                                    this.picking = true;
                                    cx.notify();
                                }))
                            }))
                        } else {
                            d.child(
                                button("new-project-create", "Create", true, can_create).when(can_create, |d| {
                                    d.on_click(cx.listener(|this, _, _, cx| this.create(false, cx)))
                                }),
                            )
                        }
                    }),
            )
    }
}
