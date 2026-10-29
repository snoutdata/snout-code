//! What the app remembers between runs, in `~/Library/Application Support/SnoutCode/`.
//!
//! On the first launch, with nothing saved there yet, the repository list comes from the
//! earlier app's eframe storage in `~/Library/Application Support/snout/app.ron`. That file
//! is only ever read: this app never writes, moves or deletes anything in that folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snout_core::DiffBase;
use snout_core::groups::RepoGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Match the system",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub repos: Vec<PathBuf>,
    pub groups: Vec<RepoGroup>,
    pub current: Option<PathBuf>,
    pub diff_base: DiffBase,
    pub side_by_side: bool,
    pub compact: bool,
    /// The bottom dock (the Claude Code console) is showing.
    pub console_open: bool,
    pub theme: ThemeChoice,
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
    /// The interface's typeface and size.
    pub ui_font: String,
    pub ui_font_size: f32,
    /// The editors' typeface (their size is `editor_font_size`).
    pub editor_font: String,
    /// The indentation a file gets when its own text does not show one.
    pub tab_size: usize,
    /// New editor tabs wrap long lines.
    pub soft_wrap: bool,
    /// Editors show spaces and tabs.
    pub show_whitespace: bool,
    /// How often the projects are fetched in the background, in minutes.
    pub fetch_minutes: u64,
    /// The `claude` program to run; looked up on the usual paths when unset.
    pub claude_path: Option<PathBuf>,
    /// The editor follows the files Claude Code changes: opens them, scrolls to the
    /// change and lights it briefly. Off, a dot on the tab says so.
    pub follow_claude: bool,
    /// The Project tool window shows the plain file tree (Files) rather than the project
    /// as its Gradle model has it.
    pub plain_files: bool,
    /// When each project was last opened (seconds since 1970), for the Welcome page.
    pub opened: std::collections::BTreeMap<PathBuf, u64>,
    /// Each recent project's editor tabs as it left them (the center of the dock layout),
    /// to open them again with the project.
    pub sessions: std::collections::BTreeMap<PathBuf, serde_json::Value>,
    /// The shortcuts changed from the defaults (see [`crate::keymap`]).
    pub keymap: crate::keymap::Overrides,
    /// How long the Java engine's own Gradle daemons stay without a build before they stop, in
    /// minutes.
    pub daemon_idle_minutes: u64,
}

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
            fetch_minutes: snout_core::survey::FETCH_EVERY.as_secs() / 60,
            claude_path: None,
            follow_claude: true,
            plain_files: false,
            opened: Default::default(),
            sessions: Default::default(),
            keymap: Default::default(),
            daemon_idle_minutes: crate::processes::DEFAULT_IDLE_MINUTES,
        }
    }
}

/// Where the files live; `home` is `$HOME`, so a scratch `HOME` gives a clean start.
#[derive(Debug, Clone)]
pub struct Store {
    pub file: PathBuf,
    pub legacy: PathBuf,
}

impl Store {
    pub fn in_home(home: &Path) -> Self {
        let support = home.join("Library").join("Application Support");
        Self { file: support.join("SnoutCode").join("settings.json"), legacy: support.join("snout").join("app.ron") }
    }

    /// A store of its own at `file`, with no earlier app's settings to start from.
    pub fn at(file: PathBuf) -> Self {
        Self { file, legacy: PathBuf::from("/nonexistent/snout/app.ron") }
    }

    /// The folder of the app's own files.
    pub fn dir(&self) -> PathBuf {
        self.file.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    /// The store for this user, or none without a home folder (then nothing is saved).
    pub fn for_user() -> Option<Self> {
        std::env::var_os("HOME").map(|h| Self::in_home(Path::new(&h)))
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
fn migrate(mut s: Settings) -> Settings {
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
