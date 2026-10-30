//! What the app remembers between runs, in `~/Library/Application Support/SnoutCode/` on a
//! Mac and `~/.config/SnoutCode/` (`$XDG_CONFIG_HOME/SnoutCode`) on Linux.
//!
//! On the first launch, with nothing saved there yet, the repository list comes from the
//! earlier app's eframe storage in `~/Library/Application Support/snout/app.ron`. That file
//! is only ever read: this app never writes, moves or deletes anything in that folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snout_core::DiffBase;
use snout_core::groups::RepoGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

/// The colour theme, each in light and dark (see `theme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ColorTheme {
    /// Warm, after Snout Studio: cream and walnut grounds, a terracotta or amber accent.
    #[default]
    Studio,
    /// Neutral greys, a violet accent, colour wherever it carries meaning.
    Vivid,
    /// Midnight indigo, an aqua accent that glows.
    Deep,
    /// The earlier neutral greys and blue accent.
    Classic,
}

impl ColorTheme {
    pub const ALL: [Self; 4] = [Self::Studio, Self::Vivid, Self::Deep, Self::Classic];

    pub fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::Vivid => "Vivid",
            Self::Deep => "Deep",
            Self::Classic => "Classic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct Settings {
    /// The repositories in the list, in its order.
    pub repos: Vec<PathBuf>,
    /// The named groups of repositories in the list.
    pub groups: Vec<RepoGroup>,
    /// The repository open last.
    pub current: Option<PathBuf>,
    /// Which two versions of a file the Git view's diff compares.
    pub diff_base: DiffBase,
    /// The Git view's diff shows the two versions side by side.
    pub side_by_side: bool,
    /// The Git view's diff hides the lines that did not change, but for a few around each change.
    pub compact: bool,
    /// The bottom dock (the Claude Code console) is showing.
    pub console_open: bool,
    /// Light, dark, or as the system is.
    pub theme: ThemeChoice,
    /// The colour theme.
    pub colors: ColorTheme,
    /// The dock layout as GPUI Kit's dock writes it: panels, sizes, open docks.
    pub dock: Option<serde_json::Value>,
    /// The editors' font size (⌘+ and ⌘-).
    pub editor_font_size: f32,
    /// The activity the window showed last, to open on it again.
    pub activity: Option<crate::panels::Activity>,
    /// The projects opened, most recent first, for File ▸ Open Recent and the Welcome
    /// page. Unset until a project opens or the list is cleared: then the repository list
    /// stands in, newest first.
    pub recent: Option<Vec<PathBuf>>,
    /// The JDK each Gradle build runs on (build root to JDK home), as chosen or downloaded.
    pub project_jdks: std::collections::BTreeMap<PathBuf, PathBuf>,
    /// The interface's typeface: "Inter" or ".SystemUIFont".
    pub ui_font: String,
    /// The interface's text size, from 10 to 20.
    #[schemars(range(min = 10, max = 20))]
    pub ui_font_size: f32,
    /// The editors' typeface (their size is `editor_font_size`).
    pub editor_font: String,
    /// The indentation, in spaces, a file gets when its own text does not show one (1 to 8).
    #[schemars(range(min = 1, max = 8))]
    pub tab_size: usize,
    /// New editor tabs wrap long lines.
    pub soft_wrap: bool,
    /// Editors show spaces and tabs.
    pub show_whitespace: bool,
    /// Java editors show the names of the parameters literal arguments are passed to.
    pub param_hints: bool,
    /// When the editor's hover (problems, their fixes and what a symbol is) shows: `after_delay`
    /// (the pointer rests for `hover_delay_ms`) or `while_command_held`. ⌘ always shows it.
    pub show_hover: ShowHover,
    /// How long the pointer rests before the hover shows, in milliseconds (0 to 3000).
    #[schemars(range(min = 0, max = 3000))]
    pub hover_delay_ms: u64,
    /// The key held to click through to a declaration: `primary` (⌘ on the Mac, Ctrl on Linux
    /// and Windows) or `alt` (⌥). With either, ⌥⌘-click goes to the implementations.
    pub click_nav_modifier: NavModifier,
    /// Who wrote each class and method shows after its line ("Ada Lovelace +2": the main
    /// author and how many others), from Git's blame of the file.
    pub blame_author_hints: bool,
    /// The caret line's blame shows faintly after its end ("Ada Lovelace, 3 years ago ·
    /// message"), once the caret rests there for the hover's delay.
    pub blame_current_line: bool,
    /// How often the projects are fetched in the background, in minutes (5 to 120).
    #[schemars(range(min = 5, max = 120))]
    pub fetch_minutes: u64,
    /// The `claude` program to run; looked up on the usual paths when unset.
    pub claude_path: Option<PathBuf>,
    /// The agent the console runs: Claude Code, Codex, or a command of your own.
    pub console_agent: crate::agents::ConsoleAgent,
    /// The `codex` program to run; looked up on the usual paths when unset.
    pub codex_path: Option<PathBuf>,
    /// The command line of your own agent (run by `/bin/sh -c` in the project), and its name.
    pub custom_agent_command: String,
    pub custom_agent_name: String,
    /// The editor follows the files Claude Code changes: opens them, scrolls to the
    /// change and lights it briefly. Off, a dot on the tab says so.
    pub follow_claude: bool,
    /// The Project tool window shows the plain file tree (Files) rather than the project
    /// as its Gradle model has it.
    pub plain_files: bool,
    /// The size each resizable sheet was left at (Settings, Project Structure, New
    /// Project), by its name: width and height in points.
    pub dialog_sizes: std::collections::BTreeMap<String, [f32; 2]>,
    /// When each project was last opened (seconds since 1970), for the Welcome page.
    pub opened: std::collections::BTreeMap<PathBuf, u64>,
    /// Each recent project's editor tabs as it left them (the center of the dock layout),
    /// to open them again with the project.
    pub sessions: std::collections::BTreeMap<PathBuf, serde_json::Value>,
    /// The shortcuts changed from the defaults: each command's keys, as `"cmd-shift-p"`; an
    /// empty list takes every shortcut of the command away.
    pub keymap: crate::keymap::Overrides,
    /// How long the Java engine's own Gradle daemons stay without a build before they stop, in
    /// minutes.
    #[schemars(range(min = 1, max = 1440))]
    pub daemon_idle_minutes: u64,
    /// Ask before quitting, even when nothing would be lost or stopped (when something
    /// would, SnoutCode always asks).
    pub confirm_quit: bool,
    /// The profiles each project's Spring model is evaluated under (none kept: any profile).
    pub spring_profiles: std::collections::BTreeMap<PathBuf, crate::spring::ProfileMode>,
    /// The JDKs added by hand in Project Structure, by their home folders, on this Mac.
    pub sdks: Vec<PathBuf>,
    /// The JDKs added by hand on each remote machine, by its SSH host name.
    pub remote_sdks: std::collections::BTreeMap<String, Vec<PathBuf>>,
    /// The engine that parses what runs in the terminals and the Claude Code console:
    /// `Builtin`, or `Ghostty` in builds that include it (the default there). New terminals
    /// start with it; open ones keep the engine they started with.
    pub terminal_engine: snout_core::term::EngineKind,
    /// The terminals' typeface; unset, the editor's (`editor_font`). Applies to open terminals.
    pub terminal_font: Option<String>,
    /// The terminals' text size, 8 to 32; unset, the editor's (`editor_font_size`).
    #[schemars(range(min = 8, max = 32))]
    pub terminal_font_size: Option<f32>,
    /// The height of a terminal's line, as a multiple of its text size (1 to 2).
    #[schemars(range(min = 1, max = 2))]
    pub terminal_line_height: f32,
    /// The cursor's shape: `Block`, `Bar` or `Underline`. A program can still ask for another.
    pub terminal_cursor: TerminalCursor,
    /// The cursor blinks while the terminal has the keyboard.
    pub terminal_cursor_blink: bool,
    /// How many lines of history each terminal keeps (1,000 to 1,000,000), for the terminals
    /// started after a change.
    #[schemars(range(min = 1000, max = 1000000))]
    pub terminal_scrollback: usize,
    /// The program the Terminal tab runs. Empty: your login shell (`$SHELL`); on a remote
    /// machine, the shell of your account there.
    pub terminal_shell: String,
    /// The arguments it starts with (`["-l"]`: a login shell).
    pub terminal_shell_args: Vec<String>,
    /// Variables added to the environment of the terminals and the Claude Code console, as
    /// names and values (`{ "JAVA_TOOL_OPTIONS": "-Xmx2g" }`), for the terminals started
    /// after a change.
    pub terminal_env: std::collections::BTreeMap<String, String>,
    /// Text selected in a terminal is copied at once, without ⌘C.
    pub terminal_copy_on_select: bool,
    /// Option sends Meta (Esc and the key) in the terminals, as shells and editors expect;
    /// off, Option types the characters of the keyboard layout (⌥E é, ⌥2 ™).
    pub terminal_option_as_meta: bool,
    /// The terminal app Open in External Terminal opens: `Terminal`, `iTerm`, `Ghostty`,
    /// `WezTerm`, `Alacritty`, `kitty`, `Warp`, or `Custom` (the command in
    /// `external_terminal_command`).
    pub external_terminal: crate::external_terminal::ExternalTerminal,
    /// The command a `Custom` external terminal runs with `/bin/sh -c`: `{dir}` stands for
    /// the folder (quoted), and on a remote machine `{ssh}` for the ssh command that opens a
    /// shell there (`open -na MyTerm --args -e {ssh}`).
    pub external_terminal_command: String,
}

/// When the editor's hover shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShowHover {
    /// Once the pointer rests on a symbol or a problem for the delay.
    #[default]
    AfterDelay,
    /// Only while ⌘ is held.
    WhileCommandHeld,
}

impl ShowHover {
    pub const ALL: [Self; 2] = [Self::AfterDelay, Self::WhileCommandHeld];

    pub fn label(self) -> &'static str {
        match self {
            Self::AfterDelay => "After a delay",
            Self::WhileCommandHeld => "Only while \u{2318} is held",
        }
    }
}

/// The key held to click through to a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NavModifier {
    /// ⌘ on the Mac, Ctrl on Linux and Windows.
    #[default]
    Primary,
    /// ⌥ (Alt).
    Alt,
}

impl NavModifier {
    pub const ALL: [Self; 2] = [Self::Primary, Self::Alt];

    pub fn label(self) -> &'static str {
        match (self, cfg!(target_os = "macos")) {
            (Self::Primary, true) => "\u{2318} Command",
            (Self::Primary, false) => "Ctrl",
            (Self::Alt, true) => "\u{2325} Option",
            (Self::Alt, false) => "Alt",
        }
    }
}

/// How long the pointer rests before the hover shows, by default, and at most.
pub const HOVER_DELAY_MS: u64 = 500;
pub const HOVER_DELAY_MAX: u64 = 3000;

/// The shape of a terminal's cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum TerminalCursor {
    #[default]
    Block,
    Bar,
    Underline,
}

impl TerminalCursor {
    pub const ALL: [Self; 3] = [Self::Block, Self::Bar, Self::Underline];

    pub fn label(self) -> &'static str {
        match self {
            Self::Block => "Block",
            Self::Bar => "Bar",
            Self::Underline => "Underline",
        }
    }
}

/// The terminal settings' defaults that are not plain values.
pub const TERMINAL_LINE_HEIGHT: f32 = 1.3;
pub const TERMINAL_FONT_MIN: f32 = 8.;
pub const TERMINAL_FONT_MAX: f32 = 32.;

impl Default for Settings {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            groups: Vec::new(),
            current: None,
            diff_base: DiffBase::default(),
            side_by_side: true,
            compact: true,
            console_open: false,
            theme: ThemeChoice::default(),
            colors: ColorTheme::default(),
            dock: None,
            editor_font_size: crate::look::MONO_SIZE,
            activity: None,
            recent: None,
            project_jdks: Default::default(),
            ui_font: crate::look::FONT.into(),
            ui_font_size: crate::look::BODY,
            editor_font: crate::look::MONO.into(),
            tab_size: 4,
            soft_wrap: false,
            show_whitespace: false,
            param_hints: true,
            show_hover: ShowHover::default(),
            hover_delay_ms: HOVER_DELAY_MS,
            click_nav_modifier: NavModifier::default(),
            blame_author_hints: true,
            blame_current_line: false,
            fetch_minutes: snout_core::survey::FETCH_EVERY.as_secs() / 60,
            claude_path: None,
            console_agent: Default::default(),
            codex_path: None,
            custom_agent_command: String::new(),
            custom_agent_name: String::new(),
            follow_claude: true,
            plain_files: false,
            dialog_sizes: Default::default(),
            opened: Default::default(),
            sessions: Default::default(),
            keymap: Default::default(),
            daemon_idle_minutes: crate::processes::DEFAULT_IDLE_MINUTES,
            confirm_quit: true,
            spring_profiles: Default::default(),
            sdks: Vec::new(),
            remote_sdks: Default::default(),
            terminal_engine: snout_core::term::EngineKind::preferred(),
            terminal_font: None,
            terminal_font_size: None,
            terminal_line_height: TERMINAL_LINE_HEIGHT,
            terminal_cursor: TerminalCursor::default(),
            terminal_cursor_blink: true,
            terminal_scrollback: snout_core::term::SCROLLBACK,
            terminal_shell: String::new(),
            terminal_shell_args: vec!["-l".into()],
            terminal_env: Default::default(),
            terminal_copy_on_select: false,
            terminal_option_as_meta: true,
            external_terminal: Default::default(),
            external_terminal_command: String::new(),
        }
    }
}

/// Where the files live; `home` is `$HOME`, so a scratch `HOME` gives a clean start (on
/// Windows, a scratch `APPDATA`).
#[derive(Debug, Clone)]
pub struct Store {
    pub file: PathBuf,
    pub legacy: PathBuf,
}

impl Store {
    pub fn in_home(home: &Path) -> Self {
        Self::in_config(snout_host::dirs::AppDirs::for_os(std::env::consts::OS, home, |_| None).config, home)
    }

    /// The store in the folder `config`, for the user whose home is `home`.
    fn in_config(config: PathBuf, home: &Path) -> Self {
        // The earlier app only ever ran on a Mac: elsewhere the file is not there.
        Self { file: config.join("settings.json"), legacy: home.join("Library/Application Support/snout/app.ron") }
    }

    /// A store of its own at `file`, with no earlier app's settings to start from.
    pub fn at(file: PathBuf) -> Self {
        Self { file, legacy: PathBuf::from("/nonexistent/snout/app.ron") }
    }

    /// The settings file's change stamp: when it was written and how long it is.
    pub fn stamp(&self) -> Option<(Option<std::time::SystemTime>, u64)> {
        std::fs::metadata(&self.file).ok().map(|m| (m.modified().ok(), m.len()))
    }

    /// The settings as the file now says, read strictly (a value of the wrong type is an
    /// error, not a default), or why it cannot be read; `None` without a file.
    pub fn read_strict(&self) -> Option<Result<Settings, String>> {
        let text = std::fs::read_to_string(&self.file).ok()?;
        Some(serde_json::from_str::<Settings>(&text).map(migrate).map_err(|e| e.to_string()))
    }

    /// The folder of the app's own files.
    pub fn dir(&self) -> PathBuf {
        self.file.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    /// The store for this user, or none without a home folder (then nothing is saved).
    pub fn for_user() -> Option<Self> {
        // `HOME`, else (Windows) the user's profile folder.
        let home = snout_host::home_dir();
        if home.as_os_str().is_empty() {
            return None;
        }
        Some(Self::in_config(snout_host::dirs::AppDirs::local().config, &home))
    }

    /// The saved settings; on a first launch, those of the earlier app; else the defaults.
    ///
    /// A file written by another build loads as far as this build can read it: a field it
    /// cannot read (a value of another shape, an unknown choice) takes its default and the
    /// rest is kept. A file that is not JSON at all is copied aside first
    /// (`settings.json.unreadable`), so the next save does not lose it.
    pub fn load(&self) -> Settings {
        if let Ok(text) = std::fs::read_to_string(&self.file) {
            return match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => migrate(lenient(value)),
                Err(e) => {
                    eprintln!("[settings] {} is not JSON ({e}); starting from the defaults", self.file.display());
                    let _ = std::fs::copy(&self.file, self.file.with_extension("json.unreadable"));
                    Settings::default()
                }
            };
        }
        std::fs::read_to_string(&self.legacy).ok().and_then(|text| import_legacy(&text)).unwrap_or_default()
    }

    /// Write the settings (never to the legacy folder).
    pub fn save(&self, settings: &Settings) -> std::io::Result<()> {
        debug_assert!(!self.file.starts_with(self.legacy.parent().unwrap_or(Path::new("/nonexistent"))));
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, &self.file)
    }
}

/// `value` as a `T`, as far as it can be read: each field that does not fit `T` (and each
/// element of a list that does not) is left out, so it takes its default. Unknown fields
/// are ignored.
pub fn lenient<T: Serialize + serde::de::DeserializeOwned + Default>(value: serde_json::Value) -> T {
    use serde_json::Value;
    if let Ok(t) = serde_json::from_value(value.clone()) {
        return t;
    }
    let Value::Object(fields) = value else { return T::default() };
    let Ok(Value::Object(mut base)) = serde_json::to_value(T::default()) else { return T::default() };
    let fits = |map: &serde_json::Map<String, Value>| serde_json::from_value::<T>(Value::Object(map.clone())).is_ok();
    for (key, v) in fields {
        let before = base.insert(key.clone(), v.clone());
        if fits(&base) {
            continue;
        }
        // A list: keep the elements that can be read.
        let kept = match v {
            Value::Array(items) => items
                .into_iter()
                .filter(|item| {
                    base.insert(key.clone(), Value::Array(vec![item.clone()]));
                    fits(&base)
                })
                .collect(),
            _ => Vec::new(),
        };
        base.insert(key.clone(), Value::Array(kept));
        if !fits(&base) {
            match before {
                Some(old) => base.insert(key, old),
                None => base.remove(&key),
            };
        }
    }
    serde_json::from_value(Value::Object(base)).unwrap_or_default()
}

/// Settings written by an earlier build, brought up to this one's meaning of them.
pub fn migrate(mut s: Settings) -> Settings {
    // Sizes out of range (a hand edit, or a build that allowed more) come back within it.
    let defaults = Settings::default();
    if !(crate::editor::FONT_MIN..=crate::editor::FONT_MAX).contains(&s.editor_font_size) {
        s.editor_font_size = defaults.editor_font_size;
    }
    if !(10.0..=20.0).contains(&s.ui_font_size) {
        s.ui_font_size = defaults.ui_font_size;
    }
    if !(1..=8).contains(&s.tab_size) {
        s.tab_size = defaults.tab_size;
    }
    if s.fetch_minutes == 0 {
        s.fetch_minutes = defaults.fetch_minutes;
    }
    if !(1..=24 * 60).contains(&s.daemon_idle_minutes) {
        s.daemon_idle_minutes = defaults.daemon_idle_minutes;
    }
    if s.ui_font.trim().is_empty() {
        s.ui_font = defaults.ui_font;
    }
    if s.editor_font.trim().is_empty() {
        s.editor_font = defaults.editor_font;
    }
    if !(1.0..=2.0).contains(&s.terminal_line_height) {
        s.terminal_line_height = defaults.terminal_line_height;
    }
    s.terminal_font_size = s.terminal_font_size.filter(|v| (TERMINAL_FONT_MIN..=TERMINAL_FONT_MAX).contains(v));
    s.terminal_font = s.terminal_font.filter(|f| !f.trim().is_empty());
    s.terminal_scrollback = s.terminal_scrollback.clamp(1000, 1_000_000);
    s.hover_delay_ms = s.hover_delay_ms.min(HOVER_DELAY_MAX);
    s
}

/// eframe keeps a RON map of keys to RON strings; the app's settings are under "app", in
/// the shape the earlier app's `Settings` had (the same fields as ours).
pub fn import_legacy(text: &str) -> Option<Settings> {
    let kv: HashMap<String, String> = ron::from_str(text).ok()?;
    ron::from_str(kv.get("app")?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_earlier_apps_settings_and_saves_its_own_elsewhere() {
        let home = std::env::temp_dir().join(format!("snout-app-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let store = Store::in_home(&home);
        assert_eq!(store.load(), Settings::default());

        // What eframe writes: the settings as a RON string inside a RON map.
        let legacy = r#"(repos:["/w/a","/w/b"],groups:[(name:"Work",repos:["/w/a"])],current:Some("/w/b"),diff_base:head_to_index,side_by_side:false,compact:true,console_open:true,theme:Dark)"#;
        let file = format!("{{\n    \"app\": {:?},\n}}", legacy);
        std::fs::create_dir_all(store.legacy.parent().unwrap()).unwrap();
        std::fs::write(&store.legacy, &file).unwrap();
        let s = store.load();
        assert_eq!(s.repos, [PathBuf::from("/w/a"), PathBuf::from("/w/b")]);
        assert_eq!(s.groups[0].name, "Work");
        assert_eq!(s.current.as_deref(), Some(Path::new("/w/b")));
        assert_eq!((s.diff_base, s.side_by_side, s.theme), (DiffBase::HeadToIndex, false, ThemeChoice::Dark));

        let mut changed = s.clone();
        changed.repos.pop();
        store.save(&changed).unwrap();
        assert_eq!(store.load(), changed, "our own file wins once it exists");
        assert_eq!(std::fs::read_to_string(&store.legacy).unwrap(), file, "the legacy file is untouched");
        let _ = std::fs::remove_dir_all(&home);
    }
}
