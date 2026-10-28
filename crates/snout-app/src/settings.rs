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
            Self::Dark => "Gold (dark)",
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

    /// The store for this user, or none without a home folder (then nothing is saved).
    pub fn for_user() -> Option<Self> {
        std::env::var_os("HOME").map(|h| Self::in_home(Path::new(&h)))
    }

    /// The saved settings; on a first launch, those of the earlier app; else the defaults.
    pub fn load(&self) -> Settings {
        if let Ok(text) = std::fs::read_to_string(&self.file) {
            return serde_json::from_str(&text).unwrap_or_default();
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
