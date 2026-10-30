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
use snout_core::term::EngineKind;

/// A page of the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Appearance,
    Keymap,
    Editor,
    VersionControl,
    Gradle,
    Spring,
    Terminal,
    ClaudeCode,
}

/// Whose settings the sheet shows: the user's, for every project, or the open project's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

impl Page {
    pub const ALL: [Page; 8] = [
        Page::Appearance,
        Page::Keymap,
        Page::Editor,
        Page::VersionControl,
        Page::Gradle,
        Page::Spring,
        Page::Terminal,
        Page::ClaudeCode,
    ];

    /// The page has settings a project may set for itself.
    pub fn per_project(self) -> bool {
        matches!(self, Page::Editor | Page::VersionControl | Page::Gradle | Page::Spring)
    }

    /// Some of the page's settings may be set for a project (all of them when
    /// [`Page::per_project`]): the Terminal page's shell and variables.
    pub fn has_project_rows(self) -> bool {
        self.per_project() || self == Page::Terminal
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
            Page::Terminal => IconName::SquareTerminal,
            Page::ClaudeCode => IconName::Sparkles,
        }
    }

    /// The page's spot drawing, at its head.
    pub fn art(self) -> &'static snout_core::art::Drawing {
        use snout_core::art::spots;
        match self {
            Page::Appearance => spots::appearance(),
            Page::Keymap => spots::keymap(),
            Page::Editor => spots::editor(),
            Page::VersionControl => spots::version_control(),
            Page::Gradle => spots::gradle(),
            Page::Spring => spots::spring(),
            Page::Terminal => spots::terminal(),
            Page::ClaudeCode => spots::claude_review(),
        }
    }

    /// What the page is for, under its title.
    pub fn blurb(self) -> &'static str {
        match self {
            Page::Appearance => "Colours, light or dark, and the typefaces of the window and the editor.",
            Page::Keymap => "The keys of every command. Add keys, take them away, or go back to the defaults.",
            Page::Editor => "How files indent, wrap and show as you type.",
            Page::VersionControl => "How SnoutCode keeps your projects' Git up to date.",
            Page::Gradle => "The JDK each build runs on, and how long SnoutCode's Gradle daemons stay.",
            Page::Spring => "The profiles the Spring window evaluates the beans under.",
            Page::Terminal => "How the terminals look and what they run, and the terminal app to open folders in.",
            Page::ClaudeCode => "The coding agents SnoutCode follows, and the one its console runs.",
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
            Page::Terminal => "Terminal",
            Page::ClaudeCode => "Agents",
        }
    }

    /// The group the page sits under in the tree, if any.
    fn group(self) -> Option<&'static str> {
        match self {
            Page::Gradle => Some("Build Tools"),
            Page::Spring => Some("Frameworks"),
            Page::Terminal | Page::ClaudeCode => Some("Tools"),
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
            Page::Editor => "tab size indent soft wrap whitespace font navigation click declaration modifier",
            Page::VersionControl => "git fetch interval background blame annotate annotations author hints line",
            Page::Gradle => "build tools jdk java daemon idle timeout",
            Page::Spring => "frameworks boot profiles active beans",
            Page::Terminal => {
                "tools engine ghostty built-in builtin shell console emulator vt font cursor blink scrollback \
                 environment variables copy select option meta external iterm wezterm alacritty kitty warp"
            }
            Page::ClaudeCode => "tools agents ai claude code codex openai console program path command colour color",
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
        project: Option<Box<ProjectSettings>>,
        close: bool,
    },
    /// Close, and open the settings of this scope as JSON in a tab.
    OpenJson(crate::settings_json::Which),
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
    codex: Entity<InputState>,
    agent_command: Entity<InputState>,
    agent_name: Entity<InputState>,
    /// The Terminal page's shell, its arguments, and a variable to add (`NAME=value`).
    shell: Entity<InputState>,
    shell_args: Entity<InputState>,
    new_var: Entity<InputState>,
    /// The external terminal's custom command, and the terminal apps installed.
    external_command: Entity<InputState>,
    installed_terminals: Vec<crate::external_terminal::ExternalTerminal>,
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
        let text_field = |placeholder: &'static str,
                          value: String,
                          window: &mut Window,
                          cx: &mut Context<Self>,
                          apply: fn(&mut Settings, String)| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value));
            cx.subscribe(&input, move |this, input, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    let text = input.read(cx).value().trim().to_owned();
                    apply(&mut this.draft, text);
                    cx.notify();
                }
            })
            .detach();
            input
        };
        let codex = text_field(
            "Found on PATH and the usual places",
            draft.codex_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            window,
            cx,
            |s, t| s.codex_path = (!t.is_empty()).then(|| PathBuf::from(t)),
        );
        let agent_command =
            text_field("aider --model sonnet", draft.custom_agent_command.clone(), window, cx, |s, t| {
                s.custom_agent_command = t
            });
        let agent_name = text_field("Its program's name", draft.custom_agent_name.clone(), window, cx, |s, t| {
            s.custom_agent_name = t
        });
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
        let shown = draft.clone();
        let shell = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Your login shell ($SHELL)")
                .default_value(shown.terminal_shell.clone())
        });
        cx.subscribe(&shell, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().trim().to_owned();
                this.set(cx, |s| s.terminal_shell = text);
            }
        })
        .detach();
        let shell_args = cx.new(|cx| {
            InputState::new(window, cx).placeholder("No arguments").default_value(shown.terminal_shell_args.join(" "))
        });
        cx.subscribe(&shell_args, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let args: Vec<String> = input.read(cx).value().split_whitespace().map(str::to_owned).collect();
                this.set(cx, |s| s.terminal_shell_args = args);
            }
        })
        .detach();
        let new_var = cx.new(|cx| InputState::new(window, cx).placeholder("NAME=value, then Return"));
        cx.subscribe_in(&new_var, window, |this, _, e: &InputEvent, window, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                this.add_var(window, cx);
            }
        })
        .detach();
        let external_command = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("open -na MyTerm --args --cwd {dir}")
                .default_value(shown.external_terminal_command.clone())
        });
        cx.subscribe(&external_command, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().trim().to_owned();
                this.set(cx, |s| s.external_terminal_command = text);
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
            codex,
            agent_command,
            agent_name,
            shell,
            shell_args,
            new_var,
            external_command,
            installed_terminals: crate::external_terminal::installed(),
            jdks,
            builds,
            key_search,
            recording: None,
            taken: None,
            focus: cx.focus_handle(),
        }
    }

    /// Whether a project is open, so the project's scope can show.
    pub fn has_project(&self) -> bool {
        self.root.is_some()
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

    /// Put the changes in place, then open the file of the scope shown in a tab.
    pub fn edit_as_json(&mut self, cx: &mut Context<Self>) {
        if self.changed(cx) {
            self.apply(false, cx);
        }
        let which = match self.scope {
            Scope::User => crate::settings_json::Which::User,
            Scope::Project => crate::settings_json::Which::Project,
        };
        cx.emit(SettingsEvent::OpenJson(which));
    }

    /// Show the user's settings or the project's (there is none without a project).
    pub fn set_scope(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        if scope == Scope::Project && self.root.is_none() {
            return;
        }
        self.scope = scope;
        self.sync_profiles_field(window, cx);
        self.sync_shell_fields(window, cx);
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
        next.show_hover = d.show_hover;
        next.hover_delay_ms = d.hover_delay_ms;
        next.click_nav_modifier = d.click_nav_modifier;
        next.fetch_minutes = d.fetch_minutes;
        next.blame_author_hints = d.blame_author_hints;
        next.blame_current_line = d.blame_current_line;
        next.daemon_idle_minutes = d.daemon_idle_minutes;
        next.claude_path = d.claude_path.clone();
        next.codex_path = d.codex_path.clone();
        next.console_agent = d.console_agent;
        next.custom_agent_command = d.custom_agent_command.clone();
        next.custom_agent_name = d.custom_agent_name.clone();
        next.project_jdks = d.project_jdks.clone();
        next.spring_profiles = d.spring_profiles.clone();
        next.keymap = d.keymap.clone();
        next.confirm_quit = d.confirm_quit;
        next.terminal_engine = d.terminal_engine;
        next.terminal_font = d.terminal_font.clone();
        next.terminal_font_size = d.terminal_font_size;
        next.terminal_line_height = d.terminal_line_height;
        next.terminal_cursor = d.terminal_cursor;
        next.terminal_cursor_blink = d.terminal_cursor_blink;
        next.terminal_scrollback = d.terminal_scrollback;
        next.terminal_shell = d.terminal_shell.clone();
        next.terminal_shell_args = d.terminal_shell_args.clone();
        next.terminal_env = d.terminal_env.clone();
        next.terminal_copy_on_select = d.terminal_copy_on_select;
        next.terminal_option_as_meta = d.terminal_option_as_meta;
        next.external_terminal = d.external_terminal;
        next.external_terminal_command = d.external_terminal_command.clone();
        self.draft = next.clone();
        cx.emit(SettingsEvent::Apply { user: Box::new(next), project: project.map(Box::new), close });
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
        if matches!(key, Key::TerminalShell | Key::TerminalShellArgs) {
            self.sync_shell_fields(window, cx);
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

    /// The shell fields show the scope's values.
    fn sync_shell_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.shown();
        let args = s.terminal_shell_args.join(" ");
        if self.shell.read(cx).value() != s.terminal_shell {
            self.shell.update(cx, |i, cx| i.set_value(s.terminal_shell.clone(), window, cx));
        }
        let typed: Vec<String> = self.shell_args.read(cx).value().split_whitespace().map(str::to_owned).collect();
        if typed != s.terminal_shell_args {
            self.shell_args.update(cx, |i, cx| i.set_value(args, window, cx));
        }
    }

    /// Add the variable typed (`NAME=value`) to the terminals' environment.
    fn add_var(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.new_var.read(cx).value().to_string();
        let Some((name, value)) = parse_var(&text) else { return };
        self.set(cx, |s| {
            s.terminal_env.insert(name, value);
        });
        self.new_var.update(cx, |i, cx| i.set_value("", window, cx));
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
        let m = self.model.read(cx);
        let (error, file) = match self.scope {
            Scope::Project => (m.project_settings_error.clone(), ps::FILE),
            Scope::User => (m.user_settings_error.clone(), "Your settings.json"),
        };
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
            .when_some(error, |d, e| {
                d.child(
                    crate::components::callout_with(
                        crate::components::Tone::Warning,
                        Some(format!("{file} cannot be read").into()),
                        format!("The values it had before stay in use until it is fixed. {e}"),
                        &p,
                    )
                    .debug_selector(|| "settings-project-error".into()),
                )
            })
            .into_any_element()
    }
}

/// A variable typed as `NAME=value` (spaces around the name trimmed); `None` without a name.
fn parse_var(text: &str) -> Option<(String, String)> {
    let (name, value) = text.split_once('=')?;
    let name = name.trim();
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| (name.to_owned(), value.to_owned()))
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

/// A count with thousands apart: `10,000`.
fn group_digits(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The scrollback one step up (`by` 1) or down: by a thousand lines to 10,000, then by
/// 10,000 to 100,000, then by 100,000.
fn step_lines(lines: usize, by: i32) -> usize {
    let at = if by < 0 { lines.saturating_sub(1) } else { lines };
    let step = if at < 10_000 {
        1_000
    } else if at < 100_000 {
        10_000
    } else {
        100_000
    };
    (lines as i64 + i64::from(by) * step as i64).clamp(1_000, 1_000_000) as usize
}

/// The hover's delay one step up (`by` 1) or down: 50 ms at a time to 300, then 100.
fn step_delay(ms: u64, by: i32) -> u64 {
    let step = if ms < 300 || (ms == 300 && by < 0) { 50 } else { 100 };
    (ms as i64 + i64::from(by) * step).clamp(0, crate::settings::HOVER_DELAY_MAX as i64) as u64
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
    let ink = p.fg;
    let button = |suffix: &'static str, sign: i32, ico: IconName| {
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
            .child(crate::components::icon(ico, 13., ink))
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
        .child(button("down", -1, IconName::Minus))
        .child(div().min_w(px(44.)).text_center().child(value))
        .child(button("up", 1, IconName::Plus))
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

/// A terminal engine to pick: a small terminal drawn in the theme's colours with the
/// engine's icon, its name and what it brings beneath, outlined in the accent when chosen.
/// One this build lacks is dimmed and cannot be picked.
fn engine_card(engine: EngineKind, on: bool, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let name = engine.label().to_lowercase().replace('-', "");
    let id = format!("settings-engine-{name}");
    let available = engine.unavailable().is_none();
    let (ico, blurb) = match engine {
        EngineKind::Builtin => (IconName::SquareTerminal, "In every build"),
        EngineKind::Ghostty => (IconName::Ghost, "Reflow, kitty keys, links"),
    };
    let hover = p.button_hover;
    let term_bg = if p.dark { gpui::black().opacity(0.35) } else { p.panel };
    let line = |w: f32, c: gpui::Hsla| div().h(px(3.)).w(px(w)).rounded(px(2.)).bg(c);
    let hues = p.hues;
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .w(px(150.))
        .flex()
        .flex_col()
        .gap(px(5.))
        .p(px(4.))
        .rounded(px(8.))
        .when(on, |d| d.bg(p.selection()))
        .when(available && !on, |d| d.cursor_pointer().hover(move |st| st.bg(hover)))
        .when(!available, |d| d.opacity(0.55))
        .child(
            div()
                .h(px(64.))
                .flex()
                .flex_col()
                .rounded(px(5.))
                .overflow_hidden()
                .border_1()
                .border_color(if on { p.accent } else { p.border })
                .bg(term_bg)
                .child(
                    div()
                        .h(px(14.))
                        .px(px(5.))
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .bg(p.ground)
                        .border_b_1()
                        .border_color(p.border)
                        .children([hues[3], hues[6], hues[1]].map(|c| div().size(px(4.)).rounded(px(2.)).bg(c)))
                        .child(div().flex_1())
                        .child(crate::components::icon(ico, 10., if on { p.accent } else { p.fg_dim })),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .p(px(7.))
                        .child(div().flex().gap(px(3.)).child(line(6., p.accent)).child(line(40., p.fg_dim)))
                        .child(div().flex().gap(px(3.)).child(line(22., hues[1])).child(line(30., hues[0])))
                        .child(
                            div()
                                .flex()
                                .gap(px(3.))
                                .child(line(6., p.accent))
                                .child(div().w(px(5.)).h(px(7.)).bg(p.fg)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .text_size(px(12.))
                .text_color(if on { p.fg } else { p.fg_dim })
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(crate::components::icon(ico, 12., if on { p.accent } else { p.fg_dim }))
                .child(engine.label()),
        )
        .child(div().text_size(px(11.)).text_color(p.fg_dim).child(if available { blurb } else { "Unavailable" }))
}

/// A terminal as the chosen engine draws it, in the theme's colours: its title bar with the
/// engine's name, then a short session.
fn terminal_preview(engine: EngineKind, look: crate::console::TermLook, shell: &str, p: &Palette) -> gpui::Div {
    let shell = std::path::Path::new(shell.trim()).file_name().and_then(|n| n.to_str()).unwrap_or("zsh").to_owned();
    let shown = engine.or_available();
    let ico = if shown == EngineKind::Ghostty { IconName::Ghost } else { IconName::SquareTerminal };
    let hues = p.hues;
    div()
        .flex()
        .flex_col()
        .rounded(px(8.))
        .overflow_hidden()
        .border_1()
        .border_color(p.border)
        .bg(p.panel)
        .child(
            div()
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .bg(p.ground)
                .border_b_1()
                .border_color(p.border)
                .children([hues[3], hues[6], hues[1]].map(|c| div().size(px(8.)).rounded_full().bg(c)))
                .child(div().ml(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child(format!("{shell} in shop")))
                .child(div().flex_1())
                .child(crate::components::badge_with_icon(
                    format!("{} engine", shown.label()),
                    ico,
                    crate::components::Tone::Accent,
                    p,
                )),
        )
        .child(div().px(px(10.)).py(px(8.)).child(crate::console::preview(shown, look)))
}

/// Java code as the editor shows it with these settings: its indentation at the tab size,
/// whitespace when shown, a parameter name hint, and a long line wrapped or cut.
fn editor_preview(d: &Settings, p: &Palette) -> gpui::Div {
    use crate::theme::syntax_colors;
    let syn = syntax_colors(p);
    let mono: SharedString = d.editor_font.clone().into();
    let indent = |level: usize| -> AnyElement {
        let n = level * d.tab_size;
        let text = if d.show_whitespace { "\u{b7}".repeat(n) } else { " ".repeat(n) };
        div().flex_none().text_color(p.fg_dim.opacity(0.6)).child(text).into_any_element()
    };
    let t = |text: &'static str, c: gpui::Hsla| div().flex_none().text_color(c).child(text).into_any_element();
    let hint = |name: &'static str| {
        div()
            .flex_none()
            .mx(px(2.))
            .px(px(4.))
            .rounded(px(3.))
            .bg(p.wash(p.fg_dim))
            .text_size(px(11.))
            .text_color(p.fg_dim)
            .child(name)
            .into_any_element()
    };
    let line = |n: usize, parts: Vec<AnyElement>| {
        div()
            .flex()
            .child(div().w(px(26.)).flex_none().text_color(syn.gutter).child(n.to_string()))
            .child(div().flex_1().min_w_0().flex().when(d.soft_wrap, |d| d.flex_wrap()).children(parts))
    };
    let mut retry = vec![indent(2), t("return ", syn.keyword), t("retry", syn.function), t("(", syn.punctuation)];
    if d.param_hints {
        retry.push(hint("times:"));
    }
    retry.extend([t("3", syn.number), t(", ", syn.punctuation)]);
    if d.param_hints {
        retry.push(hint("reason:"));
    }
    retry.extend([t("\"the payment service did not answer in time\"", syn.string), t(");", syn.punctuation)]);
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(10.))
        .rounded(px(8.))
        .overflow_hidden()
        .border_1()
        .border_color(p.border)
        .bg(p.panel)
        .font_family(mono)
        .text_size(px(12.5))
        .line_height(px(19.))
        .whitespace_nowrap()
        .child(line(1, vec![t("class ", syn.keyword), t("OrderService", syn.ty), t(" {", syn.punctuation)]))
        .child(line(
            2,
            vec![
                indent(1),
                t("Result ", syn.ty),
                t("place", syn.function),
                t("(", syn.punctuation),
                t("Order ", syn.ty),
                t("order", p.fg),
                t(") {", syn.punctuation),
            ],
        ))
        .child(line(3, retry))
        .child(line(4, vec![indent(1), t("}", syn.punctuation)]))
        .child(line(5, vec![t("}", syn.punctuation)]))
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
        if self.scope == Scope::Project && !self.page.has_project_rows() {
            let title = self.page.title();
            return crate::components::EmptyState::new(
                snout_core::art::spots::shared_settings(),
                "The same in every project",
            )
            .hint(format!("{title} settings are yours: a project cannot set them for itself."))
            .action(
                crate::components::Button::new("settings-show-user", IconName::User)
                    .debug_selector("settings-show-user")
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
                    let ico = match t {
                        ThemeChoice::System => IconName::SunMoon,
                        ThemeChoice::Light => IconName::Sun,
                        ThemeChoice::Dark => IconName::Moon,
                    };
                    look::toggle(id, ico, label, d.theme == t, &p)
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
                let marks = [
                    Key::TabSize,
                    Key::SoftWrap,
                    Key::ShowWhitespace,
                    Key::ParamHints,
                    Key::ShowHover,
                    Key::HoverDelayMs,
                ]
                .map(|k| self.mark(k, cx));
                let [tab_mark, wrap_mark, space_mark, hints_mark, hover_mark, delay_mark] = marks;
                let delay_view = cx.entity();
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
                    .child(section("Hover", &p))
                    .child(marked(
                        row(
                            "Show hover",
                            div().flex().items_center().gap(px(6.)).children(crate::settings::ShowHover::ALL.map(
                                |h| {
                                    let (id, ico) = match h {
                                        crate::settings::ShowHover::AfterDelay => {
                                            ("settings-hover-delay", IconName::Timer)
                                        }
                                        crate::settings::ShowHover::WhileCommandHeld => {
                                            ("settings-hover-command", IconName::Command)
                                        }
                                    };
                                    look::toggle(id, ico, h.label(), d.show_hover == h, &p)
                                        .debug_selector(move || id.to_owned())
                                        .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.show_hover = h)))
                                },
                            )),
                            &p,
                        ),
                        hover_mark,
                    ))
                    .child(marked(
                        row(
                            "Delay",
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(stepper(
                                    "settings-hover-delay-ms",
                                    d.hover_delay_ms.to_string(),
                                    move |by, _, cx| {
                                        delay_view.update(cx, |v, cx| {
                                            v.set(cx, |s| s.hover_delay_ms = step_delay(s.hover_delay_ms, by))
                                        })
                                    },
                                    &p,
                                ))
                                .child(div().text_color(p.fg_dim).child("ms")),
                            &p,
                        ),
                        delay_mark,
                    ))
                    .child(div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(
                        "For problems and their fixes, and what a symbol is. \u{2318} always shows it at once, \
                         with the link to the declaration.",
                    ))
                    .child(section("Navigation", &p))
                    .child(row(
                        "Click with",
                        div().flex().items_center().gap(px(6.)).children(crate::settings::NavModifier::ALL.map(|m| {
                            let (id, ico) = match m {
                                crate::settings::NavModifier::Primary => ("settings-nav-primary", IconName::Command),
                                crate::settings::NavModifier::Alt => ("settings-nav-alt", IconName::Option),
                            };
                            look::toggle(id, ico, m.label(), d.click_nav_modifier == m, &p)
                                .debug_selector(move || id.to_owned())
                                .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.click_nav_modifier = m)))
                        })),
                        &p,
                    ))
                    .child(div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(
                        "Hold it to underline a name as a link, and click to go to its declaration (on the declaration, \
                         its usages). \u{2325}\u{2318}-click goes to the implementations.",
                    ))
                    .child(section("Preview", &p))
                    .child(editor_preview(&d, &p))
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
                    .child(section("Annotations", &p))
                    .child(check_line(
                        look::checkbox(
                            "settings-author-hints",
                            "Show who wrote each class and method (\u{201c}Ada Lovelace +2\u{201d})",
                            d.blame_author_hints,
                            true,
                            &p,
                        )
                        .debug_selector(|| "settings-author-hints".into())
                        .on_click(
                            cx.listener(|this, _, _, cx| this.set(cx, |s| s.blame_author_hints = !s.blame_author_hints)),
                        ),
                        None,
                    ))
                    .child(check_line(
                        look::checkbox(
                            "settings-line-blame",
                            "Show the current line\u{2019}s author and commit after its end",
                            d.blame_current_line,
                            true,
                            &p,
                        )
                        .debug_selector(|| "settings-line-blame".into())
                        .on_click(
                            cx.listener(|this, _, _, cx| this.set(cx, |s| s.blame_current_line = !s.blame_current_line)),
                        ),
                        None,
                    ))
                    .child(div().text_size(px(12.)).text_color(p.fg_dim).child(
                        "From Git\u{2019}s blame of the file. The line\u{2019}s author shows once the caret rests for the \
                         hover\u{2019}s delay (Editor \u{25b8} Hover). Annotate with Git Blame is in the gutter\u{2019}s \
                         right-click menu and the Git menu.",
                    ))
                    .child(crate::components::tips(
                        "Good to know",
                        &[
                            (
                                IconName::CloudDownload,
                                "A background fetch leaves your branches as they are: it only counts the commits there are to pull.",
                            ),
                            (
                                IconName::GitBranch,
                                "Click the branch in the title bar for New Branch, Fetch, Pull and Push, and every branch to check out.",
                            ),
                            (IconName::Keyboard, "\u{2318}2 shows the Git view. In the message field, \u{2318}\u{21a9} commits."),
                        ],
                        &p,
                    ))
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
                    body = body.child(
                        div()
                            .pt(px(8.))
                            .text_color(p.fg_dim)
                            .child(format!("No JDK was found on {}.", crate::platform::THIS_MACHINE)),
                    );
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
                .child(crate::components::tips(
                    "Good to know",
                    &[
                        (IconName::Hammer, "Double click a task in the Gradle tool window to run it. Its output shows in the Build window."),
                        (IconName::Activity, "The Processes window lists the Gradle daemons running, and stops the idle ones on request."),
                        (IconName::LayoutGrid, "Project Structure (\u{2318};) downloads JDKs and sets the project's own."),
                    ],
                    &p,
                ))
                .into_any_element()
            }
            Page::Spring => {
                let tips = crate::components::tips(
                    "Good to know",
                    &[
                        (
                            IconName::Leaf,
                            "The leaf on the right stripe opens the Spring window: the beans and the endpoints SnoutCode found.",
                        ),
                        (
                            IconName::Keyboard,
                            "Spring Active Profiles (\u{2303}\u{2325}P) changes the profiles from anywhere.",
                        ),
                        (
                            IconName::EyeOff,
                            "A bean the active profiles leave out is dimmed, and no injection takes it.",
                        ),
                    ],
                    &p,
                );
                if self.root.is_none() {
                    return body
                        .child(section("Active profiles", &p))
                        .child(div().text_color(p.fg_dim).child("Open a project to choose its profiles."))
                        .child(tips)
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
                    .child(tips)
                    .into_any_element()
            }
            Page::Terminal => self.terminal_page(&d, cx),
            Page::ClaudeCode => body.child(self.agents_page(&d, cx)).into_any_element(),
        }
    }

    /// Settings ▸ Tools ▸ Agents: the agents SnoutCode knows (each one's colour, icon and how
    /// SnoutCode follows it), the one the console runs, and the programs.
    fn agents_page(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        use crate::agents::{Channel, ConsoleAgent};
        let p = palette(cx);
        let chosen = d.console_agent;
        let cards = crate::agents::known(d).into_iter().zip(ConsoleAgent::ALL).map(|(agent, which)| {
            let on = chosen == which;
            let color = agent.color(&p);
            let how = match agent.channel {
                Channel::ClaudeIde => "Sees the editor; its hooks report each edit and prompt",
                Channel::CodexNotify => "Its turns and prompts reported; files seen as it works",
                Channel::Watcher => "Files seen on disk while it works in the console",
            };
            let hover = p.button_hover;
            let id = format!("settings-agent-{}", agent.id);
            div()
                .id(gpui::SharedString::from(id.clone()))
                .debug_selector(move || id.clone())
                .w(px(196.))
                .p(px(10.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .rounded(px(8.))
                .border_1()
                .border_color(if on { p.accent } else { p.border })
                .when(on, |d| d.bg(p.wash(p.accent)))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.console_agent = which)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_none()
                                .size(px(26.))
                                .rounded(px(7.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(crate::theme::mix(color, 0.18, p.card))
                                .child(crate::components::icon(agent.icon, 15., color)),
                        )
                        .child(div().flex_1().min_w_0().font_weight(FontWeight::MEDIUM).child(agent.name.clone()))
                        .when(on, |d| d.child(crate::components::icon(IconName::CircleCheck, 14., p.accent))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(11.5))
                        .text_color(p.fg_dim)
                        .child(div().flex_none().size(px(9.)).rounded_full().bg(color))
                        .child("Its colour, caret and marks"),
                )
                .child(div().text_size(px(11.5)).line_height(px(15.)).text_color(p.fg_dim).child(how))
        });
        let note = |text: &'static str| div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(text);
        div()
            .flex()
            .flex_col()
            .child(section("The console runs", &p))
            .child(div().flex().gap(px(8.)).children(cards))
            .child(div().pt(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child(
                "Each agent writes in its own colour: a caret with its name while it types, a mark in the gutter \
                 beside the lines it changed, and a dot on the tabs and files it changed until you review them.",
            ))
            .child(section("Programs", &p))
            .child(row("Claude Code", div().w(px(360.)).child(look::field(&self.claude, false, None, &p)), &p))
            .child(row("Codex", div().w(px(360.)).child(look::field(&self.codex, false, None, &p)), &p))
            .child(note("Leave them empty to find claude and codex on PATH, in ~/.local/bin and the usual places."))
            .child(row("Your agent", div().w(px(360.)).child(look::field(&self.agent_command, false, None, &p)), &p))
            .child(row("Its name", div().w(px(200.)).child(look::field(&self.agent_name, false, None, &p)), &p))
            .child(note("Any command line agent: the console runs it in the project with /bin/sh -c."))
            .child(crate::components::tips(
                "Good to know",
                &[
                    (
                        IconName::SquareTerminal,
                        "\u{2318}J shows or hides the console. The session keeps running while it is hidden.",
                    ),
                    (
                        IconName::TextCursorInput,
                        "\u{2325}\u{2318}K sends the selection, or the whole file, to the agent.",
                    ),
                    (IconName::Check, "In a proposed change's tab, \u{2318}\u{21a9} accepts it and Esc rejects it."),
                    (
                        IconName::ListChecks,
                        "The Agent Changes window keeps every edit an agent made, by prompt, to accept or revert.",
                    ),
                ],
                &p,
            ))
            .into_any_element()
    }

    /// Settings ▸ Tools ▸ Terminal: the engine, the look (with a preview), what the Terminal
    /// tab runs, and how keys and selection behave. In the project's scope only what a
    /// project may set: the shell and the variables.
    fn terminal_page(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let shell = self.terminal_shell_rows(d, cx);
        if self.scope == Scope::Project {
            return div()
                .flex()
                .flex_col()
                .child(shell)
                .child(div().pt(px(14.)).text_size(px(12.)).text_color(p.fg_dim).child(
                    "The terminals' look, engine and keys, and the external terminal, are yours: they are the same \
                     in every project.",
                ))
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .child(self.terminal_engine_rows(d, cx))
            .child(self.terminal_look_rows(d, cx))
            .child(shell)
            .child(self.terminal_behaviour_rows(d, cx))
            .child(self.external_terminal_rows(d, cx))
            .child(self.terminal_running(d, cx))
            .into_any_element()
    }

    /// The engine cards, and beside them what the engine is and why Ghostty may be missing.
    fn terminal_engine_rows(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        use crate::components::{Tone, callout_with};
        let p = palette(cx);
        let chosen = d.terminal_engine;
        let cards = div().flex().gap(px(6.)).children(EngineKind::ALL.map(|e| {
            let card = engine_card(e, chosen == e, &p);
            if e.unavailable().is_none() {
                card.on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.terminal_engine = e)))
            } else {
                card
            }
        }));
        let about = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(div().text_size(px(12.)).line_height(px(17.)).text_color(p.fg_dim).child(
                "The engine reads what runs in the terminals and the Claude Code console. A change applies to \
                 the terminals you start after it; open terminals keep the engine they started with.",
            ))
            .when_some(EngineKind::Ghostty.unavailable(), |d, why| {
                d.child(
                    callout_with(Tone::Info, Some("Ghostty is unavailable".into()), why, &p)
                        .debug_selector(|| "settings-ghostty-unavailable".into()),
                )
            });
        div()
            .flex()
            .flex_col()
            .child(section("Terminal engine", &p))
            .child(div().flex().items_start().gap(px(16.)).child(cards).child(about))
            .into_any_element()
    }

    /// Typeface, size, line height and cursor, then the preview drawn with them.
    fn terminal_look_rows(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        use crate::settings::{TERMINAL_FONT_MAX, TERMINAL_FONT_MIN, TerminalCursor};
        let p = palette(cx);
        let view = cx.entity();
        let mut fonts = vec![(String::new(), format!("Editor's ({})", d.editor_font))];
        fonts.extend(EDITOR_FONTS.iter().map(|(v, l)| ((*v).to_owned(), (*l).to_owned())));
        let font_now = d.terminal_font.clone().unwrap_or_default();
        let pick_font = {
            let view = view.clone();
            move |v: String, cx: &mut App| {
                view.update(cx, |this, cx| this.set(cx, |s| s.terminal_font = (!v.is_empty()).then_some(v)))
            }
        };
        let size = d.terminal_font_size.unwrap_or(d.editor_font_size);
        let size_view = view.clone();
        let size_row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(stepper(
                "settings-terminal-size",
                format!("{size}"),
                move |by, _, cx| {
                    size_view.update(cx, |v, cx| {
                        v.set(cx, |s| {
                            let now = s.terminal_font_size.unwrap_or(s.editor_font_size);
                            s.terminal_font_size = Some((now + by as f32).clamp(TERMINAL_FONT_MIN, TERMINAL_FONT_MAX));
                        })
                    })
                },
                &p,
            ))
            .child(if d.terminal_font_size.is_some() {
                crate::components::Button::new("settings-terminal-size-editor", IconName::RotateCcw)
                    .debug_selector("settings-terminal-size-editor")
                    .compact()
                    .ghost()
                    .label("Use the editor's")
                    .on_click(cx.listener(|this, _, _, cx| this.set(cx, |s| s.terminal_font_size = None)))
                    .into_any_element()
            } else {
                div().text_size(px(12.)).text_color(p.fg_dim).child("The editor's size").into_any_element()
            });
        let height_view = view.clone();
        let cursor = |c: TerminalCursor, cx: &mut Context<Self>| {
            let ico = match c {
                TerminalCursor::Block => IconName::Square,
                TerminalCursor::Bar => IconName::TextCursor,
                TerminalCursor::Underline => IconName::Underline,
            };
            let id = format!("settings-terminal-cursor-{}", c.label().to_lowercase());
            look::toggle(SharedString::from(id.clone()), ico, c.label(), d.terminal_cursor == c, &p)
                .debug_selector(move || id.clone())
                .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.terminal_cursor = c)))
        };
        let cursors = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .children(TerminalCursor::ALL.map(|c| cursor(c, cx)))
            .child(div().w(px(12.)))
            .child(
                look::checkbox("settings-terminal-blink", "Blink", d.terminal_cursor_blink, true, &p)
                    .debug_selector(|| "settings-terminal-blink".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set(cx, |s| s.terminal_cursor_blink = !s.terminal_cursor_blink)
                    })),
            );
        div()
            .flex()
            .flex_col()
            .child(section("Look", &p))
            .child(row("Font", choice("settings-terminal-font", &font_now, fonts, pick_font, &p), &p))
            .child(row("Size", size_row, &p))
            .child(row(
                "Line height",
                stepper(
                    "settings-terminal-line-height",
                    format!("{:.1}", d.terminal_line_height),
                    move |by, _, cx| {
                        height_view.update(cx, |v, cx| {
                            v.set(cx, |s| {
                                let next = (s.terminal_line_height * 10.).round() + by as f32;
                                s.terminal_line_height = (next / 10.).clamp(1., 2.);
                            })
                        })
                    },
                    &p,
                ),
                &p,
            ))
            .child(row("Cursor", cursors, &p))
            .child(section("Preview", &p))
            .child(terminal_preview(d.terminal_engine, crate::console::TermLook::of(d), &d.terminal_shell, &p))
            .when(d.terminal_engine.or_available() == EngineKind::Builtin, |b| {
                b.child(div().pt(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child(
                    "Ghostty also draws the deprecated call struck through, its curly warning line, and the test \
                     report's link.",
                ))
            })
            .into_any_element()
    }

    /// The program the Terminal tab runs, its arguments, and the variables added: the rows a
    /// project may set for itself.
    fn terminal_shell_rows(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let [shell_mark, args_mark, env_mark] =
            [Key::TerminalShell, Key::TerminalShellArgs, Key::TerminalEnv].map(|k| self.mark(k, cx));
        let remote = crate::remote::is_remote(cx);
        let vars = d.terminal_env.iter().map(|(name, value)| {
            let name2 = name.clone();
            let id = format!("settings-terminal-var-{name}");
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(2.))
                .child(crate::components::icon(IconName::Variable, 13., p.fg_dim))
                .child(div().font_family(look::MONO).text_size(px(12.5)).child(format!("{name}={value}")))
                .child(
                    crate::components::IconButton::new(SharedString::from(id), IconName::X, "Remove this variable")
                        .size(20.)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let name = name2.clone();
                            this.set(cx, move |s| {
                                s.terminal_env.remove(&name);
                            })
                        })),
                )
                .into_any_element()
        });
        let vars: Vec<AnyElement> = vars.collect();
        div()
            .flex()
            .flex_col()
            .child(section("Shell", &p))
            .child(marked(
                row("Program", div().w(px(300.)).child(look::field(&self.shell, false, None, &p)), &p),
                shell_mark,
            ))
            .child(div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(if remote {
                "Empty: the login shell of your account on the remote machine."
            } else {
                "Empty: your login shell. On a remote machine, your shell there."
            }))
            .child(marked(
                row("Arguments", div().w(px(300.)).child(look::field(&self.shell_args, false, None, &p)), &p),
                args_mark,
            ))
            .child(
                div()
                    .pl(px(162.))
                    .text_size(px(12.))
                    .text_color(p.fg_dim)
                    .child("Separated by spaces. -l starts a login shell."),
            )
            .child(marked(
                row(
                    "Environment",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().w(px(300.)).child(look::field(&self.new_var, false, None, &p)))
                        .child(
                            crate::components::Button::new("settings-terminal-add-var", IconName::Plus)
                                .debug_selector("settings-terminal-add-var")
                                .compact()
                                .label("Add")
                                .on_click(cx.listener(|this, _, w, cx| this.add_var(w, cx))),
                        ),
                    &p,
                ),
                env_mark,
            ))
            .child(div().pl(px(162.)).flex().flex_col().children(vars).when(d.terminal_env.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child("Added to the terminals and the Claude Code console you start after a change."),
                )
            }))
            .into_any_element()
    }

    /// Scrollback, selection and the Option key.
    fn terminal_behaviour_rows(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let view = cx.entity();
        div()
            .flex()
            .flex_col()
            .child(section("Behaviour", &p))
            .child(row(
                "Scrollback",
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(stepper(
                        "settings-terminal-scrollback",
                        group_digits(d.terminal_scrollback),
                        move |by, _, cx| {
                            view.update(cx, |v, cx| {
                                v.set(cx, |s| s.terminal_scrollback = step_lines(s.terminal_scrollback, by))
                            })
                        },
                        &p,
                    ))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child("lines, for terminals started after a change"),
                    ),
                &p,
            ))
            .child(check_line(
                look::checkbox(
                    "settings-terminal-copy-on-select",
                    "Copy text as soon as it is selected",
                    d.terminal_copy_on_select,
                    true,
                    &p,
                )
                .debug_selector(|| "settings-terminal-copy-on-select".into())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set(cx, |s| s.terminal_copy_on_select = !s.terminal_copy_on_select)
                })),
                None,
            ))
            .child(check_line(
                look::checkbox(
                    "settings-terminal-option-meta",
                    "Use Option as Meta (\u{2325}F moves a word, not \u{192})",
                    d.terminal_option_as_meta,
                    true,
                    &p,
                )
                .debug_selector(|| "settings-terminal-option-meta".into())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set(cx, |s| s.terminal_option_as_meta = !s.terminal_option_as_meta)
                })),
                None,
            ))
            .into_any_element()
    }

    /// The terminal app Open in External Terminal opens: those installed, or a command.
    fn external_terminal_rows(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        use crate::external_terminal::ExternalTerminal;
        let p = palette(cx);
        let chosen = d.external_terminal;
        let name = |t: ExternalTerminal| format!("{t:?}");
        let mut apps: Vec<(String, String)> =
            self.installed_terminals.iter().map(|t| (name(*t), t.label().to_owned())).collect();
        if chosen != ExternalTerminal::Custom && !self.installed_terminals.contains(&chosen) {
            apps.push((name(chosen), format!("{} (not installed)", chosen.label())));
        }
        apps.push((name(ExternalTerminal::Custom), ExternalTerminal::Custom.label().to_owned()));
        let view = cx.entity();
        let pick = move |v: String, cx: &mut App| {
            let all = ExternalTerminal::APPS.iter().copied().chain([ExternalTerminal::Custom]);
            if let Some(t) = all.into_iter().find(|t| format!("{t:?}") == v) {
                view.update(cx, |this, cx| this.set(cx, |s| s.external_terminal = t));
            }
        };
        let custom = chosen == ExternalTerminal::Custom;
        div()
            .flex()
            .flex_col()
            .child(section("External terminal", &p))
            .child(row(
                "Open in",
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(crate::components::icon(IconName::SquareTerminal, 14., p.fg_dim))
                    .child(choice("settings-external-terminal", &name(chosen), apps, pick, &p)),
                &p,
            ))
            .when(custom, |b| {
                b.child(row(
                    "Command",
                    div().w(px(360.)).child(look::field(&self.external_command, false, None, &p)),
                    &p,
                ))
                .child(div().pl(px(162.)).text_size(px(12.)).text_color(p.fg_dim).child(
                    "Run with /bin/sh. {dir} is the folder; on a remote machine, {ssh} is the ssh command that \
                     opens a shell there.",
                ))
            })
            .child(div().pl(px(162.)).pt(px(4.)).text_size(px(12.)).text_color(p.fg_dim).child(
                "Open in External Terminal, in the File menu and on a file's or folder's menu, opens it at the \
                 folder. On a remote project it opens an ssh session there.",
            ))
            .into_any_element()
    }

    /// The terminals running and their engines.
    fn terminal_running(&mut self, d: &Settings, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let chosen = d.terminal_engine;
        let open: Vec<(String, EngineKind)> = cx
            .try_global::<crate::console::OpenTerminals>()
            .map(|o| o.0.iter().map(|((kind, root), e)| (format!("{kind} in {}", Model::dir_name(root)), *e)).collect())
            .unwrap_or_default();
        let others = open.iter().filter(|(_, e)| *e != chosen).count();
        let mut body = div().flex().flex_col();
        if !open.is_empty() {
            body = body.child(section("Running now", &p)).children(open.into_iter().map(|(what, e)| {
                let ico = if e == EngineKind::Ghostty { IconName::Ghost } else { IconName::SquareTerminal };
                div()
                    .pl(px(12.))
                    .py(px(3.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(crate::components::icon(ico, 13., p.fg_dim))
                    .child(what)
                    .child(div().text_color(p.fg_dim).child(e.label()))
            }));
            if others > 0 {
                body =
                    body.child(div().pl(px(12.)).pt(px(4.)).text_size(px(12.)).text_color(p.fg_dim).child(
                        "A terminal running another engine keeps it until it starts again (New, in its header).",
                    ));
            }
        }
        body.into_any_element()
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
                            look::small_button("keymap-take", IconName::Unlink, "Remove It There", &p)
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
                        d.child(look::small_button("keymap-reset-all", IconName::RotateCcw, "Reset All", &p).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.draft.keymap.clear();
                                this.taken = None;
                                cx.notify();
                            }),
                        ))
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
        let button = |id: &'static str, ico: IconName, text: &'static str, primary: bool, enabled: bool| {
            crate::components::footer_button(id, ico, text, primary, enabled, &p)
        };
        div()
            .id("settings")
            .debug_selector(|| "settings".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(SettingsEvent::Cancel)))
            .occlude()
            .size_full()
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
                        .child(crate::components::page_header(
                            self.page.art(),
                            self.page.title(),
                            self.page.blurb(),
                            div().flex().flex_col().items_end().gap(px(6.)).child(scope_switch).child(
                                crate::components::Button::new("settings-edit-json", IconName::FileBraces)
                                    .debug_selector("settings-edit-json")
                                    .compact()
                                    .ghost()
                                    .label("Edit as JSON")
                                    .tooltip("Open these settings' file in a tab, with completion and checks")
                                    .on_click(cx.listener(|this, _, _, cx| this.edit_as_json(cx))),
                            ),
                            &p,
                        ))
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
                        button("settings-cancel", crate::components::CANCEL_ICON, "Cancel", false, true)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Cancel))),
                    )
                    .child(
                        button("settings-apply", crate::components::APPLY_ICON, "Apply", false, changed)
                            .when(changed, |d| d.on_click(cx.listener(|this, _, _, cx| this.apply(false, cx)))),
                    )
                    .child(
                        button("settings-ok", crate::components::OK_ICON, "OK", true, true)
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
    fn the_scrollback_steps_by_thousands_then_more() {
        assert_eq!(super::step_lines(10_000, 1), 20_000);
        assert_eq!(super::step_lines(10_000, -1), 9_000);
        assert_eq!(super::step_lines(1_000, -1), 1_000);
        assert_eq!(super::step_lines(100_000, -1), 90_000);
        assert_eq!(super::group_digits(1_000_000), "1,000,000");
        assert_eq!(super::group_digits(999), "999");
    }

    #[test]
    fn a_variable_is_typed_as_name_equals_value() {
        assert_eq!(super::parse_var(" JAVA_OPTS =-Xmx2g -Da=b"), Some(("JAVA_OPTS".into(), "-Xmx2g -Da=b".into())));
        assert_eq!(super::parse_var("=x"), None);
        assert_eq!(super::parse_var("NO VALUE"), None);
        assert_eq!(super::parse_var("A B=c"), None);
    }

    #[test]
    fn the_search_finds_pages_by_title_group_and_keyword() {
        assert!(Page::Terminal.matches("cursor"));
        assert!(Page::Terminal.matches("scrollback"));
        assert!(Page::Gradle.matches("build"));
        assert!(Page::Gradle.matches("JDK"));
        assert!(Page::Gradle.matches("daemon"));
        assert!(Page::Editor.matches("wrap"));
        assert!(Page::ClaudeCode.matches("tools"));
        assert!(!Page::Keymap.matches("theme"));
        assert!(Page::ALL.iter().all(|p| p.matches("")));
    }
}
