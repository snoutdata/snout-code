//! The Settings sheet (the gear, or ⌘,): a searchable tree of pages on the left, the page
//! on the right, and Cancel, Apply and OK at the foot. It edits a copy of the settings;
//! Apply and OK put the copy in place, save it to `settings.json`, and the window follows.
//!
//! A switch over the page picks the scope: the user's settings, for every project, or the
//! open project's own (`.snout/settings.json`, see [`crate::project_settings`]). Each row a
//! project may set says so: in the user's scope, that this project overrides it; in the
//! project's, whether the project sets it (with a button to reset it to the user's value).

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;

use crate::components::IconName;
use crate::glyphs::{Icon, icon};
use crate::lang::java::Jdk;
use crate::look::{self, Trigger};
use crate::model::Model;
use crate::project_settings::{self as ps, Key, ProjectSettings};
use crate::settings::{ColorTheme, Settings, ThemeChoice};
use crate::spring::ProfileMode;
use crate::theme::{Palette, palette};

/// A page of the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Appearance,
    Keymap,
    Editor,
    VersionControl,
    Gradle,
    Spring,
    ClaudeCode,
}

/// Whose settings the sheet shows: the user's, for every project, or the open project's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::Appearance,
        Page::Keymap,
        Page::Editor,
        Page::VersionControl,
        Page::Gradle,
        Page::Spring,
        Page::ClaudeCode,
    ];

    /// The page has settings a project may set for itself.
    pub fn per_project(self) -> bool {
        matches!(self, Page::Editor | Page::VersionControl | Page::Gradle | Page::Spring)
    }

    /// The page's icon, in the list and over the page.
    pub fn icon(self) -> crate::components::IconName {
        use crate::components::IconName;
        match self {
            Page::Appearance => IconName::Palette,
            Page::Keymap => IconName::Keyboard,
            Page::Editor => IconName::FileCode,
            Page::VersionControl => IconName::GitBranch,
            Page::Gradle => IconName::Hammer,
            Page::Spring => IconName::Leaf,
            Page::ClaudeCode => IconName::Sparkles,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Appearance => "Appearance",
            Page::Keymap => "Keymap",
            Page::Editor => "Editor",
            Page::VersionControl => "Version Control",
            Page::Gradle => "Gradle",
            Page::Spring => "Spring",
            Page::ClaudeCode => "Claude Code",
        }
    }

    /// The group the page sits under in the tree, if any.
    fn group(self) -> Option<&'static str> {
        match self {
            Page::Gradle => Some("Build Tools"),
            Page::Spring => Some("Frameworks"),
            Page::ClaudeCode => Some("Tools"),
            _ => None,
        }
    }

    /// Words the search finds the page by, besides its title.
    fn keywords(self) -> &'static str {
        match self {
            Page::Appearance => {
                "theme colors studio vivid deep classic dark light system font size interface quit quitting confirm"
            }
            Page::Keymap => "shortcuts keys bindings",
            Page::Editor => "tab size indent soft wrap whitespace font",
            Page::VersionControl => "git fetch interval background",
            Page::Gradle => "build tools jdk java daemon idle timeout",
            Page::Spring => "frameworks boot profiles active beans",
            Page::ClaudeCode => "tools agent console program path",
        }
    }

    /// Whether the search `query` finds the page.
    pub fn matches(self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.title().to_lowercase().contains(&q)
            || self.group().is_some_and(|g| g.to_lowercase().contains(&q))
            || self.keywords().contains(&q)
    }
}

/// The typefaces offered: the bundled ones first, then the system's.
const UI_FONTS: [(&str, &str); 2] = [("Inter", "Inter"), (".SystemUIFont", "System")];
const EDITOR_FONTS: [(&str, &str); 2] = [("Hack", "Hack"), ("Menlo", "Menlo")];

pub enum SettingsEvent {
    /// Put the copies in place: the user's settings and, when they changed, the project's
    /// (and close when `close` says so).
    Apply {
        user: Box<Settings>,
        project: Option<ProjectSettings>,
        close: bool,
    },
    Cancel,
}

pub struct SettingsView {
    model: Entity<Model>,
    /// The user's settings as the sheet shows them.
    pub draft: Settings,
    /// The project's own settings as the sheet shows them, and the project's root.
    pub project: ProjectSettings,
    root: Option<PathBuf>,
    pub scope: Scope,
    pub page: Page,
    /// The Spring page's profile names, when chosen.
    profiles: Entity<InputState>,
    pub search: Entity<InputState>,
    claude: Entity<InputState>,
    /// The JDKs on this Mac, for the Gradle page.
    jdks: Vec<Jdk>,
    /// The Gradle builds the Gradle page lists.
    builds: Vec<PathBuf>,
    /// The Keymap page's search box.
    pub key_search: Entity<InputState>,
    /// The command whose new shortcut is being pressed, and what catches the keys.
    pub recording: Option<(String, gpui::Subscription)>,
    /// What the last shortcut recorded took from other commands: the command it went to,
    /// its keys, and the commands that had them.
    pub taken: Option<(String, String, Vec<String>)>,
    focus: FocusHandle,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsView {
    pub fn new(
        model: Entity<Model>,
        builds: Vec<PathBuf>,
        jdks: Vec<Jdk>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let m = model.read(cx);
        let draft = m.user_settings();
        let project = m.project_settings.clone();
        let root = m.project_root();
        let chosen = match m.spring_mode() {
            ProfileMode::Chosen(names) => names.join(", "),
            _ => String::new(),
        };
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
        cx.subscribe(&search, |this, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let query = this.search.read(cx).value().to_string();
                if !this.page.matches(&query)
                    && let Some(first) = Page::ALL.into_iter().find(|p| p.matches(&query))
                {
                    this.page = first;
                }
                cx.notify();
            }
        })
        .detach();
        let path = draft.claude_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let claude = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Found on PATH and the usual places").default_value(path)
        });
        cx.subscribe(&claude, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().trim().to_owned();
                this.draft.claude_path = (!text.is_empty()).then(|| PathBuf::from(text));
                cx.notify();
            }
        })
        .detach();
        let profiles = cx.new(|cx| InputState::new(window, cx).placeholder("dev, local").default_value(chosen));
        cx.subscribe(&profiles, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let names = profile_names(&input.read(cx).value());
                if matches!(this.spring_mode(), ProfileMode::Chosen(_)) {
                    this.set_spring_mode(ProfileMode::Chosen(names), cx);
                }
            }
        })
        .detach();
        let key_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search commands or shortcuts"));
        cx.subscribe(&key_search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        Self {
            model,
            draft,
            project,
            root,
            scope: Scope::User,
            profiles,
            page: Page::Appearance,
            search,
            claude,
            jdks,
            builds,
            key_search,
            recording: None,
            taken: None,
            focus: cx.focus_handle(),
        }
    }

    /// The settings differ from those in use.
    pub fn changed(&self, cx: &App) -> bool {
        let m = self.model.read(cx);
        self.draft != m.user_settings() || self.project != m.project_settings
    }

    /// The settings the page shows: the user's, or the project's over them.
    pub fn shown(&self) -> Settings {
        let mut s = self.draft.clone();
        if self.scope == Scope::Project {
            ps::put_over(&self.project, &mut s);
        }
        s
    }

    /// Show the user's settings or the project's (there is none without a project).
    pub fn set_scope(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        if scope == Scope::Project && self.root.is_none() {
            return;
        }
        self.scope = scope;
        self.sync_profiles_field(window, cx);
        cx.notify();
    }

    pub fn apply(&mut self, close: bool, cx: &mut Context<Self>) {
        // Only what the sheet edits: the rest may have moved on while it was open.
        let m = self.model.read(cx);
        let mut next = m.user_settings();
        let project = (self.project != m.project_settings).then(|| self.project.clone());
        let d = &self.draft;
        next.theme = d.theme;
        next.colors = d.colors;
        next.ui_font = d.ui_font.clone();
        next.ui_font_size = d.ui_font_size;
        next.editor_font = d.editor_font.clone();
        next.editor_font_size = d.editor_font_size;
        next.tab_size = d.tab_size;
        next.soft_wrap = d.soft_wrap;
        next.show_whitespace = d.show_whitespace;
        next.param_hints = d.param_hints;
        next.fetch_minutes = d.fetch_minutes;
        next.daemon_idle_minutes = d.daemon_idle_minutes;
        next.claude_path = d.claude_path.clone();
        next.project_jdks = d.project_jdks.clone();
        next.spring_profiles = d.spring_profiles.clone();
        next.keymap = d.keymap.clone();
        next.confirm_quit = d.confirm_quit;
        self.draft = next.clone();
        cx.emit(SettingsEvent::Apply { user: Box::new(next), project, close });
    }

    /// Change what the page shows: the user's settings, or in the project's scope, the
    /// project's (a value changed there is set for the project).
    fn set(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Settings)) {
        match self.scope {
            Scope::User => f(&mut self.draft),
            Scope::Project => {
                let before = self.shown();
                let mut after = before.clone();
                f(&mut after);
                ps::take_changes(&before, &after, &mut self.project);
            }
        }
        cx.notify();
    }

    /// Leave `key` to the user's settings in the project's scope.
    pub fn reset(&mut self, key: Key, window: &mut Window, cx: &mut Context<Self>) {
        key.clear(&mut self.project);
        if key == Key::SpringProfiles {
            self.sync_profiles_field(window, cx);
        }
        cx.notify();
    }

    /// The Spring profiles the page shows.
    fn spring_mode(&self) -> ProfileMode {
        match (self.scope, &self.project.spring_profiles) {
            (Scope::Project, Some(p)) => p.into(),
            _ => self.root.as_ref().and_then(|r| self.draft.spring_profiles.get(r).cloned()).unwrap_or_default(),
        }
    }

    fn set_spring_mode(&mut self, mode: ProfileMode, cx: &mut Context<Self>) {
        match self.scope {
            Scope::Project => self.project.spring_profiles = Some((&mode).into()),
            Scope::User => {
                let Some(root) = self.root.clone() else { return };
                if mode == ProfileMode::Any {
                    self.draft.spring_profiles.remove(&root);
                } else {
                    self.draft.spring_profiles.insert(root, mode);
                }
            }
        }
        cx.notify();
    }

    /// The profile names field shows those of the scope's mode.
    fn sync_profiles_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let names = match self.spring_mode() {
            ProfileMode::Chosen(names) => names.join(", "),
            _ => String::new(),
        };
        self.profiles.update(cx, |i, cx| i.set_value(names, window, cx));
    }

    /// What a row the project may set says of it: overridden by the project (the user's
    /// scope), or set for the project with a reset button, or the user's value (the
    /// project's).
    fn mark(&self, key: Key, cx: &mut Context<Self>) -> Option<AnyElement> {
        use crate::components::{IconButton, Tone, badge_with_icon};
        let p = palette(cx);
        let model = self.model.read(cx);
        match self.scope {
            Scope::User => key.is_set(&model.project_settings).then(|| {
                badge_with_icon("Overridden by this project", IconName::FolderCog, Tone::Warning, &p)
                    .debug_selector(move || format!("settings-overridden-{}", key.name()))
                    .into_any_element()
            }),
            Scope::Project if key.is_set(&self.project) => Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        badge_with_icon("Set for this project", IconName::FolderCog, Tone::Accent, &p)
                            .debug_selector(move || format!("settings-project-set-{}", key.name())),
                    )
                    .child(
                        IconButton::new(
                            SharedString::from(format!("settings-reset-{}", key.name())),
                            IconName::RotateCcw,
                            "Reset to your setting",
                        )
                        .size(22.)
                        .on_click(cx.listener(move |this, _, w, cx| this.reset(key, w, cx))),
                    )
                    .into_any_element(),
            ),
            Scope::Project => Some(
                badge_with_icon("Your setting", IconName::User, Tone::Neutral, &p)
                    .debug_selector(move || format!("settings-inherited-{}", key.name()))
                    .into_any_element(),
            ),
        }
    }
}

impl SettingsView {
    /// User or Project: whose settings the sheet shows.
    fn scope_switch(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let has_project = self.root.is_some();
        let segment = |scope: Scope, id: &'static str, ico: IconName, text: &'static str, cx: &mut Context<Self>| {
            let on = self.scope == scope;
            let enabled = scope == Scope::User || has_project;
            let hover = p.button_hover;
            let ink = if on {
                p.fg
            } else if enabled {
                p.fg_dim
            } else {
                p.fg_dim.opacity(0.5)
            };
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .h(px(24.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(5.))
                .text_size(px(12.5))
                .text_color(ink)
                .when(on, |d| d.bg(p.card).shadow_sm().font_weight(FontWeight::MEDIUM))
                .when(!on && enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
                .child(crate::components::icon(ico, 13., if on { p.accent } else { ink }))
                .child(text)
                .when(enabled, |d| d.on_click(cx.listener(move |this, _, w, cx| this.set_scope(scope, w, cx))))
        };
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(7.))
            .border_1()
            .border_color(p.border)
            .bg(p.ground)
            .child(segment(Scope::User, "settings-scope-user", IconName::User, "User", cx))
            .child(segment(Scope::Project, "settings-scope-project", IconName::FolderCog, "Project", cx))
            .into_any_element()
    }

    /// Under the page's title: whose settings these are and where they are kept, and the
    /// project file's error if it has one.
    fn scope_line(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let error = self.model.read(cx).project_settings_error.clone();
        let (ico, text) = match (self.scope, &self.root) {
            (Scope::Project, Some(root)) => (
                IconName::FolderCog,
                format!(
                    "For {} only, kept in {} in the project. What it sets overrides your settings.",
                    Model::dir_name(root),
                    ps::FILE
                ),
            ),
            _ => (IconName::User, "Your settings, for every project. A project can override some of them.".to_owned()),
        };
        div()
            .px(px(20.))
            .pb(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .debug_selector(|| "settings-scope-line".into())
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(p.fg_dim)
                    .child(crate::components::icon(ico, 12., p.fg_dim))
                    .child(text),
            )
            .when_some(error.filter(|_| self.scope == Scope::Project), |d, e| {
                d.child(
                    crate::components::callout_with(
                        crate::components::Tone::Warning,
                        Some(format!("{} cannot be read", ps::FILE).into()),
                        format!("The values it had before stay in use until it is fixed. {e}"),
                        &p,
                    )
                    .debug_selector(|| "settings-project-error".into()),
                )
            })
            .into_any_element()
    }
}

/// Profile names typed in a field: separated by commas or spaces.
fn profile_names(text: &str) -> Vec<String> {
    text.split([',', ' ']).map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned).collect()
}

/// A row with what [`SettingsView::mark`] says after its control.
fn marked(row: gpui::Div, mark: Option<AnyElement>) -> gpui::Div {
    row.children(mark)
}

/// A check box line with its mark after it.
fn check_line(check: impl IntoElement, mark: Option<AnyElement>) -> gpui::Div {
    div().py(px(6.)).flex().items_center().gap(px(12.)).child(check).children(mark)
}

/// A labelled row of a page: the label on the left, the control after it.
pub(crate) fn row(label: &'static str, control: impl IntoElement, p: &Palette) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(34.))
        .child(div().w(px(150.)).flex_none().text_color(p.fg).child(label))
        .child(control)
}

pub(crate) fn section(text: &'static str, p: &Palette) -> gpui::Div {
    div().pt(px(14.)).pb(px(6.)).text_color(p.fg_dim).text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(text)
}

/// The daemons' idle time one step up (`by` 1) or down: a minute at a time up to 5, then 5.
fn step_idle(minutes: u64, by: i32) -> u64 {
    let step = if minutes < 5 || (minutes == 5 && by < 0) { 1 } else { 5 };
    (minutes as i64 + i64::from(by) * step).clamp(1, 240) as u64
}

/// A − value + stepper.
fn stepper(
    id: &'static str,
    value: String,
    on_step: impl Fn(i32, &mut Window, &mut App) + 'static,
    p: &Palette,
) -> AnyElement {
    let on_step = std::rc::Rc::new(on_step);
    let hover = p.button_hover;
    let button = |suffix: &'static str, sign: i32, text: &'static str| {
        let on_step = on_step.clone();
        div()
            .id(SharedString::from(format!("{id}-{suffix}")))
            .debug_selector(move || format!("{id}-{suffix}"))
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(text)
            .on_click(move |_, w, cx| on_step(sign, w, cx))
    };
    div()
        .flex()
        .items_center()
        .h(px(28.))
        .px(px(2.))
        .rounded(px(5.))
        .border_1()
        .border_color(p.border)
        .bg(p.input)
        .child(button("down", -1, "−"))
        .child(div().min_w(px(44.)).text_center().child(value))
        .child(button("up", 1, "+"))
        .into_any_element()
}

/// A drop-down of `choices` (value, label), the current one checked.
pub(crate) fn choice(
    id: &'static str,
    current: &str,
    choices: Vec<(String, String)>,
    on_pick: impl Fn(String, &mut App) + 'static,
    p: &Palette,
) -> AnyElement {
    let label = choices.iter().find(|(v, _)| v == current).map(|(_, l)| l.clone()).unwrap_or(current.to_owned());
    let on_pick = std::rc::Rc::new(on_pick);
    let current = current.to_owned();
    let hover = p.button_hover;
    Trigger(
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .h(px(28.))
            .min_w(px(180.))
            .px(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .rounded(px(5.))
            .border_1()
            .border_color(p.border)
            .bg(p.input)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(label)
            .child(icon(Icon::ChevronDown, 12., p.fg_dim)),
    )
    .dropdown_menu(move |mut menu, _, _| {
        for (value, label) in &choices {
            let (value, on_pick) = (value.clone(), on_pick.clone());
            menu = menu.item(
                PopupMenuItem::new(label.clone())
                    .checked(value == current)
                    .on_click(move |_, _, cx| on_pick(value.clone(), cx)),
            );
        }
        menu
    })
    .into_any_element()
}

/// A colour theme to pick: a small window drawn in its colours (in the light or dark showing
/// now), its name beneath, outlined in the accent when it is the one chosen.
fn swatch(t: ColorTheme, dark: bool, on: bool, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let id = format!("settings-colors-{}", t.label().to_lowercase());
    let s = Palette::new(t, dark);
    let line = |w: f32, c: gpui::Hsla| div().h(px(3.)).w(px(w)).rounded(px(2.)).bg(c);
    let hover = p.button_hover;
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .flex()
        .flex_col()
        .gap(px(5.))
        .p(px(4.))
        .rounded(px(8.))
        .cursor_pointer()
        .when(on, |d| d.bg(p.selection()))
        .when(!on, |d| d.hover(move |st| st.bg(hover)))
        .child(
            div()
                .w(px(84.))
                .h(px(52.))
                .flex()
                .rounded(px(5.))
                .overflow_hidden()
                .border_1()
                .border_color(if on { p.accent } else { p.border })
                .bg(s.panel)
                .child(
                    div()
                        .w(px(14.))
                        .h_full()
                        .bg(s.ground)
                        .border_r_1()
                        .border_color(s.border)
                        .p(px(3.))
                        .child(div().size(px(8.)).rounded(px(2.)).bg(s.accent)),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .p(px(7.))
                        .child(line(30., crate::theme::syntax_colors(&s).keyword))
                        .child(line(44., s.fg_dim))
                        .child(div().flex().gap(px(3.)).child(line(18., s.added)).child(line(22., s.removed)))
                        .child(line(34., s.fg_dim)),
                ),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(if on { p.fg } else { p.fg_dim })
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(t.label()),
        )
}

impl SettingsView {
    fn page_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        if self.scope == Scope::Project && !self.page.per_project() {
            let title = self.page.title();
            return crate::components::EmptyState::new(
                snout_core::art::spots::shared_settings(),
                "The same in every project",
            )
            .hint(format!("{title} settings are yours: a project cannot set them for itself."))
            .action(
                crate::components::Button::new("settings-show-user")
                    .debug_selector("settings-show-user")
                    .icon(IconName::User)
                    .label("Show Your Settings")
                    .on_click(cx.listener(|this, _, w, cx| this.set_scope(Scope::User, w, cx))),
            )
            .on(p.card)
            .into_any_element();
        }
        let d = self.shown();
        let view = cx.entity();
        let set = move |f: Box<dyn Fn(&mut Settings)>| {
            let view = view.clone();
            move |cx: &mut App| view.update(cx, |v, cx| v.set(cx, |s| f(s)))
        };
        let body = div().flex().flex_col();
        match self.page {
            Page::Appearance => {
                let theme = |t: ThemeChoice, label: &'static str, id: &'static str, cx: &mut Context<Self>| {
                    look::toggle(id, label, d.theme == t, &p)
                        .debug_selector(move || id.to_owned())
                        .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.theme = t)))
                };
                let ui_fonts = UI_FONTS.iter().map(|(v, l)| ((*v).to_owned(), (*l).to_owned())).collect();
                let editor_fonts = EDITOR_FONTS.iter().map(|(v, l)| ((*v).to_owned(), (*l).to_owned())).collect();
                let pick_ui = set.clone();
                let pick_editor = set.clone();
                let view = cx.entity();
                let view2 = cx.entity();
                let dark = p.dark;
                let swatches = div().flex().flex_wrap().gap(px(6.)).children(ColorTheme::ALL.map(|t| {
                    swatch(t, dark, d.colors == t, &p)
                        .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.colors = t)))
                }));
                body.child(section("Theme", &p))
                    .child(row("Colors", swatches, &p))
                    .child(row(
                        "Light or dark",
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(theme(ThemeChoice::System, "System", "settings-theme-system", cx))
                            .child(theme(ThemeChoice::Light, "Light", "settings-theme-light", cx))
                            .child(theme(ThemeChoice::Dark, "Dark", "settings-theme-dark", cx)),
                        &p,
                    ))
                    .child(section("Interface", &p))
                    .child(row(
                        "Font",
                        choice(
                            "settings-ui-font",
                            &d.ui_font,
                            ui_fonts,
                            move |v, cx| pick_ui(Box::new(move |s: &mut Settings| s.ui_font = v.clone()))(cx),
                            &p,
                        ),
                        &p,
                    ))
                    .child(row(
                        "Size",
                        stepper(
                            "settings-ui-size",
                            format!("{}", d.ui_font_size),
                            move |by, _, cx| {
                                view.update(cx, |v, cx| {
                                    v.set(cx, |s| s.ui_font_size = (s.ui_font_size + by as f32).clamp(10., 20.))
                                })
                            },
                            &p,
                        ),
                        &p,
                    ))
                    .child(section("Editor", &p))
                    .child(row(
                        "Font",
                        choice(
                            "settings-editor-font",
                            &d.editor_font,
                            editor_fonts,
                            move |v, cx| pick_editor(Box::new(move |s: &mut Settings| s.editor_font = v.clone()))(cx),
                            &p,
                        ),
                        &p,
                    ))
                    .child(row(
                        "Size",
                        stepper(
                            "settings-editor-size",
                            format!("{}", d.editor_font_size),
                            move |by, _, cx| {
                                view2.update(cx, |v, cx| {
                                    v.set(cx, |s| {
                                        s.editor_font_size = (s.editor_font_size + by as f32)
                                            .clamp(crate::editor::FONT_MIN, crate::editor::FONT_MAX)
                                    })
                                })
                            },
                            &p,
                        ),
                        &p,
                    ))
                    .child(section("Quitting", &p))
                    .child(
                        div().py(px(6.)).child(
                            look::checkbox(
                                "settings-confirm-quit",
                                "Confirm before quitting",
                                d.confirm_quit,
                                true,
                                &p,
                            )
                            .debug_selector(|| "settings-confirm-quit".into())
                            .on_click(cx.listener(|this, _, _, cx| this.set(cx, |s| s.confirm_quit = !s.confirm_quit))),
                        ),
                    )
                    .child(div().text_size(px(12.)).text_color(p.fg_dim).child(
                        "SnoutCode always asks when quitting would lose unsaved changes or stop something running.",
                    ))
                    .into_any_element()
            }
            Page::Keymap => body.child(self.keymap_page(cx)).into_any_element(),
            Page::Editor => {
                let view = cx.entity();
                let marks =
                    [Key::TabSize, Key::SoftWrap, Key::ShowWhitespace, Key::ParamHints].map(|k| self.mark(k, cx));
                let [tab_mark, wrap_mark, space_mark, hints_mark] = marks;
                body.child(section("Indentation", &p))
                    .child(marked(
                        row(
                            "Tab size",
                            stepper(
                                "settings-tab-size",
                                d.tab_size.to_string(),
                                move |by, _, cx| {
                                    view.update(cx, |v, cx| {
                                        v.set(cx, |s| s.tab_size = (s.tab_size as i32 + by).clamp(1, 8) as usize)
                                    })
                                },
                                &p,
                            ),
                            &p,
                        ),
                        tab_mark,
                    ))
                    .child(
                        div()
                            .pl(px(162.))
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child("For a file whose own indentation cannot be told."),
                    )
                    .child(section("Display", &p))
                    .child(check_line(
                        look::checkbox("settings-soft-wrap", "Soft wrap long lines", d.soft_wrap, true, &p)
                            .debug_selector(|| "settings-soft-wrap".into())
                            .on_click(cx.listener(|this, _, _, cx| this.set(cx, |s| s.soft_wrap = !s.soft_wrap))),
                        wrap_mark,
                    ))
                    .child(check_line(
                        look::checkbox("settings-whitespace", "Show whitespace", d.show_whitespace, true, &p)
                            .debug_selector(|| "settings-whitespace".into())
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set(cx, |s| s.show_whitespace = !s.show_whitespace)),
                            ),
                        space_mark,
                    ))
                    .child(check_line(
                        look::checkbox(
                            "settings-param-hints",
                            "Show parameter names of literal arguments (Java)",
                            d.param_hints,
                            true,
                            &p,
                        )
                        .debug_selector(|| "settings-param-hints".into())
                        .on_click(cx.listener(|this, _, _, cx| this.set(cx, |s| s.param_hints = !s.param_hints))),
                        hints_mark,
                    ))
                    .into_any_element()
            }
            Page::VersionControl => {
                let view = cx.entity();
                let fetch_mark = self.mark(Key::FetchMinutes, cx);
                body.child(section("Git", &p))
                    .child(marked(
                        row(
                            "Fetch every",
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(stepper(
                                    "settings-fetch",
                                    d.fetch_minutes.to_string(),
                                    move |by, _, cx| {
                                        view.update(cx, |v, cx| {
                                            v.set(cx, |s| {
                                                s.fetch_minutes =
                                                    (s.fetch_minutes as i64 + by as i64 * 5).clamp(5, 120) as u64
                                            })
                                        })
                                    },
                                    &p,
                                ))
                                .child(div().text_color(p.fg_dim).child("minutes")),
                            &p,
                        ),
                        fetch_mark,
                    ))
                    .child(
                        div()
                            .pl(px(162.))
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child("The projects are fetched in the background, to count what there is to pull."),
                    )
                    .into_any_element()
            }
            Page::Gradle => {
                let mut body = body;
                if self.scope == Scope::Project {
                    body = body.child(section("JDK", &p));
                    let current = self.project.jdk.as_ref().map(|h| h.display().to_string()).unwrap_or_default();
                    let mut choices = vec![(String::new(), "Each build's own (your settings)".to_owned())];
                    choices.extend(
                        self.jdks
                            .iter()
                            .map(|j| (j.home.display().to_string(), format!("JDK {} ({})", j.major, j.version))),
                    );
                    let view = cx.entity();
                    let mark = self.mark(Key::Jdk, cx);
                    body = body.child(marked(
                        row(
                            "Every build",
                            choice(
                                "settings-project-jdk",
                                &current,
                                choices,
                                move |v, cx| {
                                    view.update(cx, |this, cx| {
                                        this.project.jdk = (!v.is_empty()).then(|| PathBuf::from(&v));
                                        cx.notify();
                                    })
                                },
                                &p,
                            ),
                            &p,
                        ),
                        mark,
                    ));
                } else {
                    body = body.child(section("JDK per project", &p));
                    if self.builds.is_empty() {
                        body = body.child(div().text_color(p.fg_dim).child("No Gradle build is open."));
                    }
                }
                let project_jdk = self.model.read(cx).project_settings.jdk.is_some();
                let builds = if self.scope == Scope::User { self.builds.clone() } else { Vec::new() };
                for (i, root) in builds.into_iter().enumerate() {
                    let ids = ["settings-jdk-0", "settings-jdk-1", "settings-jdk-2", "settings-jdk-3"];
                    let Some(id) = ids.get(i).copied() else { break };
                    let current = d.project_jdks.get(&root).map(|h| h.display().to_string()).unwrap_or_default();
                    let mut choices = vec![(String::new(), "Automatic (the build's Java level)".to_owned())];
                    choices.extend(
                        self.jdks
                            .iter()
                            .map(|j| (j.home.display().to_string(), format!("JDK {} ({})", j.major, j.version))),
                    );
                    let pick = set.clone();
                    let key = root.clone();
                    let overridden = project_jdk && self.root.as_ref().is_some_and(|r| root.starts_with(r));
                    let mark = if overridden { self.mark(Key::Jdk, cx) } else { None };
                    body = body.child(marked(
                        row(
                            "Project",
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(div().text_color(p.fg_dim).child(Model::dir_name(&root)))
                                .child(choice(
                                    id,
                                    &current,
                                    choices,
                                    move |v, cx| {
                                        let key = key.clone();
                                        pick(Box::new(move |s: &mut Settings| {
                                            if v.is_empty() {
                                                s.project_jdks.remove(&key);
                                            } else {
                                                s.project_jdks.insert(key.clone(), PathBuf::from(&v));
                                            }
                                        }))(cx)
                                    },
                                    &p,
                                )),
                            &p,
                        ),
                        mark,
                    ));
                }
                if self.jdks.is_empty() {
                    body = body.child(div().pt(px(8.)).text_color(p.fg_dim).child("No JDK was found on this Mac."));
                }
                let idle_mark = self.mark(Key::DaemonIdleMinutes, cx);
                let view = cx.entity();
                body.child(
                    div()
                        .pt(px(10.))
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child("The Java server restarts on the chosen JDK the next time it starts."),
                )
                .child(section("Gradle daemons", &p))
                .child(marked(
                    row(
                        "Stop idle daemons after",
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(stepper(
                                "settings-daemon-idle",
                                d.daemon_idle_minutes.to_string(),
                                move |by, _, cx| {
                                    view.update(cx, |v, cx| {
                                        v.set(cx, |s| s.daemon_idle_minutes = step_idle(s.daemon_idle_minutes, by))
                                    })
                                },
                                &p,
                            ))
                            .child(div().text_color(p.fg_dim).child("minutes")),
                        &p,
                    ),
                    idle_mark,
                ))
                .child(div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(
                    "SnoutCode's own Gradle daemons stop after this long without a build. A new value \
                             holds for the daemons started after the Java engine starts again.",
                ))
                .into_any_element()
            }
            Page::Spring => {
                if self.root.is_none() {
                    return body
                        .child(section("Active profiles", &p))
                        .child(div().text_color(p.fg_dim).child("Open a project to choose its profiles."))
                        .into_any_element();
                }
                let mode = self.spring_mode();
                let current = match &mode {
                    ProfileMode::Any => "any",
                    ProfileMode::Config => "config",
                    ProfileMode::Chosen(_) => "chosen",
                };
                let choices = vec![
                    ("any".to_owned(), "Any (no profile chosen)".to_owned()),
                    ("config".to_owned(), "Those the application files make active".to_owned()),
                    ("chosen".to_owned(), "These profiles".to_owned()),
                ];
                let view = cx.entity();
                let mark = self.mark(Key::SpringProfiles, cx);
                let names = profile_names(&self.profiles.read(cx).value());
                body.child(section("Active profiles", &p))
                    .child(marked(
                        row(
                            "Evaluate beans under",
                            choice(
                                "settings-spring-mode",
                                current,
                                choices,
                                move |v, cx| {
                                    let mode = match v.as_str() {
                                        "config" => ProfileMode::Config,
                                        "chosen" => ProfileMode::Chosen(names.clone()),
                                        _ => ProfileMode::Any,
                                    };
                                    view.update(cx, |this, cx| this.set_spring_mode(mode, cx))
                                },
                                &p,
                            ),
                            &p,
                        ),
                        mark,
                    ))
                    .when(matches!(mode, ProfileMode::Chosen(_)), |d| {
                        d.child(row(
                            "Profiles",
                            div().w(px(260.)).child(look::field(&self.profiles, false, None, &p)),
                            &p,
                        ))
                    })
                    .child(div().pl(px(162.)).pt(px(4.)).text_size(px(12.)).text_color(p.fg_dim).child(
                        "The Spring window evaluates the beans under these profiles and dims those they leave out. \
                         Its profile menu changes the same setting.",
                    ))
                    .into_any_element()
            }
            Page::ClaudeCode => body
                .child(section("Program", &p))
                .child(row("Claude Code", div().w(px(360.)).child(look::field(&self.claude, false, None, &p)), &p))
                .child(
                    div()
                        .pl(px(162.))
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child("Leave empty to find claude on PATH, in ~/.local/bin and the usual places."),
                )
                .into_any_element(),
        }
    }

    /// Wait for the keys of a new shortcut for `action`; Esc stops waiting.
    pub fn record(&mut self, action: String, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        let catch = cx.intercept_keystrokes(move |e, _, cx| {
            let keystroke = e.keystroke.clone();
            if view.update(cx, |v, cx| v.recorded(keystroke, cx)).is_ok() {
                cx.stop_propagation();
            }
        });
        self.taken = None;
        self.recording = Some((action, catch));
        cx.notify();
    }

    /// The keys pressed for the shortcut being recorded.
    fn recorded(&mut self, keystroke: gpui::Keystroke, cx: &mut Context<Self>) {
        let Some((action, _)) = self.recording.take() else { return };
        let plain = !keystroke.modifiers.modified();
        if plain && keystroke.key == "escape" {
            cx.notify();
            return;
        }
        let keys = keystroke.unparse(cx.platform_info().os);
        let entries = crate::keymap::entries(&self.draft.keymap, cx);
        let Some(entry) = entries.into_iter().find(|e| e.action == action) else { return };
        let mut list = entry.keys.clone();
        if !list.contains(&keys) {
            list.push(keys.clone());
        }
        crate::keymap::set(&mut self.draft.keymap, &entry, list);
        let conflicts = crate::keymap::Conflicts::new(&self.draft.keymap, cx);
        let others = conflicts.others(&keys, entry.context.as_deref(), &action);
        self.taken = (!others.is_empty()).then_some((action, keys, others));
        cx.notify();
    }

    /// Take `keys` away from the commands that had them before the last recording.
    fn take_from_others(&mut self, cx: &mut Context<Self>) {
        let Some((_, keys, others)) = self.taken.take() else { return };
        for entry in crate::keymap::entries(&self.draft.keymap, cx) {
            if others.contains(&entry.action) {
                let list: Vec<String> = entry.keys.iter().filter(|k| **k != keys).cloned().collect();
                crate::keymap::set(&mut self.draft.keymap, &entry, list);
            }
        }
        cx.notify();
    }

    /// The Keymap page: every command and its shortcuts, searchable; + records another
    /// shortcut, × removes one, Reset brings back the defaults.
    fn keymap_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let query = self.key_search.read(cx).value().to_string();
        let entries = crate::keymap::entries(&self.draft.keymap, cx);
        let conflicts = crate::keymap::Conflicts::new(&self.draft.keymap, cx);
        let recording = self.recording.as_ref().map(|(a, _)| a.clone());
        let hover = p.button_hover;
        let row_hover = p.row_hover();
        let mono = look::mono_font(cx);
        let chip = |text: String, warn: bool| {
            div()
                .h(px(20.))
                .px(px(6.))
                .flex()
                .items_center()
                .gap(px(4.))
                .rounded(px(4.))
                .border_1()
                .border_color(if warn { p.warn } else { p.border })
                .bg(p.input)
                .font_family(mono.clone())
                .text_size(px(12.))
                .text_color(if warn { p.warn } else { p.fg })
                .child(text)
        };
        let small = |id: SharedString, text: &'static str| {
            let sel = id.to_string();
            div()
                .id(id)
                .debug_selector(move || sel.clone())
                .h(px(20.))
                .min_w(px(20.))
                .px(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(p.fg_dim)
                .hover(move |s| s.bg(hover))
                .child(text)
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        for (ix, entry) in entries.iter().enumerate().filter(|(_, e)| e.matches(&query, cx)) {
            let mut keys = div().flex().flex_wrap().items_center().gap(px(4.));
            let mut clash: Vec<String> = Vec::new();
            for (k, key) in entry.keys.iter().enumerate() {
                let others = conflicts.others(key, entry.context.as_deref(), &entry.action);
                for o in &others {
                    let name = conflicts.name(o);
                    if !clash.contains(&name) {
                        clash.push(name);
                    }
                }
                let (action, key2) = (entry.clone(), key.clone());
                keys = keys.child(chip(crate::keymap::display(key, cx), !others.is_empty()).child(
                    small(format!("keymap-remove-{ix}-{k}").into(), "×").on_click(cx.listener(
                        move |this, _, _, cx| {
                            let list: Vec<String> = action.keys.iter().filter(|x| **x != key2).cloned().collect();
                            crate::keymap::set(&mut this.draft.keymap, &action, list);
                            this.taken = None;
                            cx.notify();
                        },
                    )),
                ));
            }
            if recording.as_deref() == Some(entry.action.as_str()) {
                keys = keys.child(
                    div()
                        .id(("keymap-recording", ix))
                        .debug_selector(|| "keymap-recording".into())
                        .h(px(20.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(p.accent)
                        .text_size(px(12.))
                        .text_color(p.accent)
                        .child("Press the keys, or Esc"),
                );
            } else if entry.keys.is_empty() {
                keys = keys.child(div().text_size(px(12.)).text_color(p.fg_dim).child("None"));
            }
            let action = entry.action.clone();
            let add = small(format!("keymap-add-{ix}").into(), "+")
                .on_click(cx.listener(move |this, _, _, cx| this.record(action.clone(), cx)));
            let reset = entry.changed().then(|| {
                let action = entry.action.clone();
                small(format!("keymap-reset-{ix}").into(), "Reset").on_click(cx.listener(move |this, _, _, cx| {
                    this.draft.keymap.remove(&action);
                    this.taken = None;
                    cx.notify();
                }))
            });
            let mut row = div()
                .id(("keymap-row", ix))
                .debug_selector(move || format!("keymap-row-{ix}"))
                .py(px(2.))
                .px(px(crate::components::ROW_PAD))
                .flex()
                .flex_col()
                .gap(px(2.))
                .rounded(px(6.))
                .hover(move |s| s.bg(row_hover))
                .child(
                    div()
                        .min_h(px(24.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .when(entry.changed(), |d| d.font_weight(FontWeight::MEDIUM))
                                .child(entry.name.clone()),
                        )
                        .child(keys)
                        .child(add)
                        .children(reset),
                );
            if !clash.is_empty() {
                row = row.child(
                    div().text_size(px(12.)).text_color(p.warn).child(format!("Also bound to {}", clash.join(", "))),
                );
            }
            if let Some((taken_for, keys, others)) = &self.taken
                && *taken_for == entry.action
            {
                let names: Vec<String> = others.iter().map(|o| conflicts.name(o)).collect();
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child(format!(
                            "{} was the shortcut of {}. Here it now runs this command.",
                            crate::keymap::display(keys, cx),
                            names.join(", ")
                        ))
                        .child(
                            look::small_button("keymap-take", "Remove It There", &p)
                                .on_click(cx.listener(|this, _, _, cx| this.take_from_others(cx))),
                        ),
                );
            }
            rows.push(row.into_any_element());
        }
        if rows.is_empty() {
            rows.push(div().py(px(12.)).text_color(p.fg_dim).child("No command matches.").into_any_element());
        }
        let any_changed = !self.draft.keymap.is_empty();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pt(px(10.))
                    .pb(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().child(look::field(&self.key_search, true, None, &p)))
                    .when(any_changed, |d| {
                        d.child(look::small_button("keymap-reset-all", "Reset All", &p).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.draft.keymap.clear();
                                this.taken = None;
                                cx.notify();
                            },
                        )))
                    }),
            )
            .child(
                div()
                    .pb(px(6.))
                    .text_size(px(12.))
                    .text_color(p.fg_dim)
                    .child("+ adds a shortcut: press its keys. Changes apply with Apply or OK."),
            )
            .children(rows)
            .into_any_element()
    }

    fn tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let query = self.search.read(cx).value().to_string();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut group = None;
        for page in Page::ALL.into_iter().filter(|pg| pg.matches(&query)) {
            if page.group() != group {
                group = page.group();
                if let Some(g) = group {
                    rows.push(
                        div()
                            .h(px(26.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(icon(Icon::ChevronDown, 11., p.fg_dim))
                            .child(g)
                            .into_any_element(),
                    );
                }
            }
            let on = page == self.page;
            let id = format!("settings-page-{}", page.title().to_lowercase().replace(' ', "-"));
            let sel = id.clone();
            rows.push(
                crate::components::list_row_h(
                    SharedString::from(id),
                    crate::components::RowState::current(on),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || sel.clone())
                .pl(px(if page.group().is_some() { 28. } else { 12. }))
                .pr(px(8.))
                .rounded(px(4.))
                .cursor_pointer()
                .font_weight(if page.group().is_some() { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                .gap(px(8.))
                .child(crate::components::icon(page.icon(), 14., if on { p.fg } else { p.fg_dim }))
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
            .w(px(220.))
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
            .child(div().id("settings-tree").flex_1().min_h_0().overflow_y_scrollbar().children(rows))
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let changed = self.changed(cx);
        let page = self.page_body(cx);
        let tree = self.tree(cx);
        let scope_switch = self.scope_switch(cx);
        let scope_line = self.scope_line(cx);
        let button = |id: &'static str, text: &'static str, primary: bool, enabled: bool| {
            use crate::components::{Variant, button_base};
            let variant = if primary { Variant::Primary } else { Variant::Secondary };
            button_base(id, variant, !enabled, false, false, false, &p).child(text)
        };
        div()
            .id("settings")
            .debug_selector(|| "settings".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(SettingsEvent::Cancel)))
            .occlude()
            .w(px(860.))
            .h(px(580.))
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
                    crate::components::IconName::Settings,
                    "Settings",
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
                        .child(
                            div()
                                .px(px(20.))
                                .pt(px(14.))
                                .pb(px(4.))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .size(px(28.))
                                        .rounded(px(8.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(p.wash(p.accent))
                                        .child(crate::components::icon(self.page.icon(), 15., p.accent)),
                                )
                                .child(
                                    div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(self.page.title()),
                                )
                                .child(div().flex_1())
                                .child(scope_switch),
                        )
                        .child(scope_line)
                        .child(
                            div()
                                .id("settings-page")
                                .flex_1()
                                .min_h_0()
                                .px(px(20.))
                                .pb(px(12.))
                                .overflow_y_scrollbar()
                                .child(page),
                        ),
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
                    .justify_end()
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.dialog_footer())
                    .child(
                        button("settings-cancel", "Cancel", false, true)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Cancel))),
                    )
                    .child(
                        button("settings-apply", "Apply", false, changed)
                            .when(changed, |d| d.on_click(cx.listener(|this, _, _, cx| this.apply(false, cx)))),
                    )
                    .child(
                        button("settings-ok", "OK", true, true)
                            .on_click(cx.listener(|this, _, _, cx| this.apply(true, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::Page;

    #[test]
    fn the_daemon_idle_time_steps_by_minutes_then_by_five() {
        assert_eq!(super::step_idle(15, 1), 20);
        assert_eq!(super::step_idle(5, -1), 4);
        assert_eq!(super::step_idle(4, 1), 5);
        assert_eq!(super::step_idle(1, -1), 1);
        assert_eq!(super::step_idle(240, 1), 240);
    }

    #[test]
    fn the_search_finds_pages_by_title_group_and_keyword() {
        assert!(Page::Gradle.matches("build"));
        assert!(Page::Gradle.matches("JDK"));
        assert!(Page::Gradle.matches("daemon"));
        assert!(Page::Editor.matches("wrap"));
        assert!(Page::ClaudeCode.matches("tools"));
        assert!(!Page::Keymap.matches("theme"));
        assert!(Page::ALL.iter().all(|p| p.matches("")));
    }
}
