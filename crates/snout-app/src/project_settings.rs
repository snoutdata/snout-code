//! Per-project settings: `.snout/settings.json` in the project's root, beside its sources (so
//! it can be committed and shared). Each value set there overrides the user's own setting of
//! the same name, for that project only; a value it leaves out comes from the user's
//! settings. It is read and written through the window's [`Host`](snout_host::Host), so a
//! project on a remote machine keeps its file there, and it is read again whenever it changes
//! on disk. A file that cannot be read (not JSON, a value of the wrong type) leaves the values
//! read last in place; the error is shown instead.
//!
//! The model keeps the effective settings in `Model::settings` (the user's with the project's
//! put over them, see [`put_over`]) and what the user's own values were under them, to give
//! them back when the user's settings are saved (see [`take_back`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snout_host::Host;

use crate::settings::Settings;
use crate::spring::ProfileMode;

/// The file, relative to the project's root.
pub const FILE: &str = ".snout/settings.json";

/// The file of the project at `root`.
pub fn file_of(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// What a project sets for itself: each value left out (`None`) is the user's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct ProjectSettings {
    /// The indentation, in spaces, a file gets when its own text does not show one (1 to 8).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 8))]
    pub tab_size: Option<usize>,
    /// New editor tabs wrap long lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_wrap: Option<bool>,
    /// Editors show spaces and tabs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_whitespace: Option<bool>,
    /// Java editors show the names of the parameters literal arguments are passed to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_hints: Option<bool>,
    /// How often the project is fetched in the background, in minutes (5 to 120).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 5, max = 120))]
    pub fetch_minutes: Option<u64>,
    /// How long the Java engine's own Gradle daemons stay without a build before they stop,
    /// in minutes (1 to 240).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 240))]
    pub daemon_idle_minutes: Option<u64>,
    /// The JDK home every Gradle build of the project runs on, instead of the one its Java
    /// level picks (or the one picked per build in the user's settings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jdk: Option<PathBuf>,
    /// The profiles the Spring model is evaluated under: "any" (none chosen: a bean with a
    /// `@Profile` may be there or not), "config" (those `spring.profiles.active` names in the
    /// application files), or a list of profile names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spring_profiles: Option<ActiveProfiles>,
}

/// The Spring profiles as the project file writes them: `"any"`, `"config"` or a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ActiveProfiles {
    Mode(ProfilesWord),
    Chosen(Vec<String>),
}

/// `"any"` or `"config"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProfilesWord {
    Any,
    Config,
}

impl From<&ActiveProfiles> for ProfileMode {
    fn from(a: &ActiveProfiles) -> Self {
        match a {
            ActiveProfiles::Mode(ProfilesWord::Any) => ProfileMode::Any,
            ActiveProfiles::Mode(ProfilesWord::Config) => ProfileMode::Config,
            ActiveProfiles::Chosen(names) => ProfileMode::Chosen(names.clone()),
        }
    }
}

impl From<&ProfileMode> for ActiveProfiles {
    fn from(m: &ProfileMode) -> Self {
        match m {
            ProfileMode::Any => ActiveProfiles::Mode(ProfilesWord::Any),
            ProfileMode::Config => ActiveProfiles::Mode(ProfilesWord::Config),
            ProfileMode::Chosen(names) => ActiveProfiles::Chosen(names.clone()),
        }
    }
}

/// A setting a project may set for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    TabSize,
    SoftWrap,
    ShowWhitespace,
    ParamHints,
    FetchMinutes,
    DaemonIdleMinutes,
    Jdk,
    SpringProfiles,
}

impl Key {
    pub const ALL: [Key; 8] = [
        Key::TabSize,
        Key::SoftWrap,
        Key::ShowWhitespace,
        Key::ParamHints,
        Key::FetchMinutes,
        Key::DaemonIdleMinutes,
        Key::Jdk,
        Key::SpringProfiles,
    ];

    /// Its name in the file.
    pub fn name(self) -> &'static str {
        match self {
            Key::TabSize => "tab_size",
            Key::SoftWrap => "soft_wrap",
            Key::ShowWhitespace => "show_whitespace",
            Key::ParamHints => "param_hints",
            Key::FetchMinutes => "fetch_minutes",
            Key::DaemonIdleMinutes => "daemon_idle_minutes",
            Key::Jdk => "jdk",
            Key::SpringProfiles => "spring_profiles",
        }
    }

    /// The project sets it.
    pub fn is_set(self, p: &ProjectSettings) -> bool {
        match self {
            Key::TabSize => p.tab_size.is_some(),
            Key::SoftWrap => p.soft_wrap.is_some(),
            Key::ShowWhitespace => p.show_whitespace.is_some(),
            Key::ParamHints => p.param_hints.is_some(),
            Key::FetchMinutes => p.fetch_minutes.is_some(),
            Key::DaemonIdleMinutes => p.daemon_idle_minutes.is_some(),
            Key::Jdk => p.jdk.is_some(),
            Key::SpringProfiles => p.spring_profiles.is_some(),
        }
    }

    /// Leave it to the user's settings.
    pub fn clear(self, p: &mut ProjectSettings) {
        match self {
            Key::TabSize => p.tab_size = None,
            Key::SoftWrap => p.soft_wrap = None,
            Key::ShowWhitespace => p.show_whitespace = None,
            Key::ParamHints => p.param_hints = None,
            Key::FetchMinutes => p.fetch_minutes = None,
            Key::DaemonIdleMinutes => p.daemon_idle_minutes = None,
            Key::Jdk => p.jdk = None,
            Key::SpringProfiles => p.spring_profiles = None,
        }
    }
}

/// The settings that are plain values of [`Settings`] too: put over the user's, given back.
macro_rules! plain {
    ($($field:ident),* $(,)?) => {
        /// Put the plain values `values` sets over `s`; returns what `s` had under them.
        pub fn put_over(values: &ProjectSettings, s: &mut Settings) -> ProjectSettings {
            let mut under = ProjectSettings::default();
            $(
                if let Some(v) = &values.$field {
                    under.$field = Some(std::mem::replace(&mut s.$field, v.clone()));
                }
            )*
            under
        }

        /// Give back to `s` the values [`put_over`] took (`under`).
        pub fn take_back(under: &ProjectSettings, s: &mut Settings) {
            $(
                if let Some(v) = &under.$field {
                    s.$field = v.clone();
                }
            )*
        }

        /// The plain values of `changes` that `project` sets too, set there instead.
        pub fn route(changes: &ProjectSettings, project: &mut ProjectSettings) {
            $(
                if project.$field.is_some() && changes.$field.is_some() {
                    project.$field = changes.$field.clone();
                }
            )*
        }

        /// The plain values that differ between `before` and `after`, set in `into`.
        pub fn take_changes(before: &Settings, after: &Settings, into: &mut ProjectSettings) {
            $(
                if before.$field != after.$field {
                    into.$field = Some(after.$field.clone());
                }
            )*
        }
    };
}

plain!(tab_size, soft_wrap, show_whitespace, param_hints, fetch_minutes, daemon_idle_minutes);

impl ProjectSettings {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The values brought within the ranges the app allows (a hand edit may go beyond them).
    fn bounded(mut self) -> Self {
        self.tab_size = self.tab_size.map(|t| t.clamp(1, 8));
        self.fetch_minutes = self.fetch_minutes.map(|m| m.clamp(5, 120));
        self.daemon_idle_minutes = self.daemon_idle_minutes.map(|m| m.clamp(1, 240));
        self
    }
}

/// The project's settings read from `text`, or why they cannot be.
pub fn parse(text: &str) -> Result<ProjectSettings, String> {
    if text.trim().is_empty() {
        return Ok(ProjectSettings::default());
    }
    serde_json::from_str::<ProjectSettings>(text).map(ProjectSettings::bounded).map_err(|e| e.to_string())
}

/// What is on disk for a project, as last seen.
#[derive(Debug, Clone, PartialEq)]
pub enum OnDisk {
    /// No file: nothing is overridden.
    Missing,
    /// The file's text, read at its stamp.
    Text(String),
}

/// The file's change stamp: when it was written and how long it is.
pub type Stamp = Option<(Option<std::time::SystemTime>, u64)>;

/// The stamp of the project file at `root` on `host`.
pub fn stamp(host: &dyn Host, root: &Path) -> Stamp {
    host.metadata(&file_of(root)).ok().map(|m| (m.modified, m.len))
}

/// Read the project file at `root` on `host`.
pub fn read(host: &dyn Host, root: &Path) -> OnDisk {
    match host.read_to_string(&file_of(root)) {
        Ok(text) => OnDisk::Text(text),
        Err(_) => OnDisk::Missing,
    }
}

/// Write `values` to the project file at `root` on `host`: the keys the app knows are set or
/// removed, the others in the file are kept. With nothing left to say, the file still goes
/// on with `{}` (deleting it would drop a file the user may have committed).
pub fn write(host: &dyn Host, root: &Path, values: &ProjectSettings) -> std::io::Result<()> {
    use serde_json::Value;
    let file = file_of(root);
    let mut object = host
        .read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| match v {
            Value::Object(o) => Some(o),
            _ => None,
        })
        .unwrap_or_default();
    let Ok(Value::Object(set)) = serde_json::to_value(values) else {
        return Err(std::io::Error::other("the settings could not be written as JSON"));
    };
    for key in Key::ALL {
        object.remove(key.name());
    }
    object.extend(set);
    let text = serde_json::to_string_pretty(&Value::Object(object)).map_err(std::io::Error::other)? + "\n";
    if let Some(dir) = file.parent() {
        host.create_dir_all(dir)?;
    }
    host.write(&file, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projects_values_go_over_the_users_and_come_back_off() {
        let user = Settings { tab_size: 4, soft_wrap: false, ..Settings::default() };
        let project = parse(r#"{ "tab_size": 2, "soft_wrap": true, "jdk": "/jdks/21" }"#).unwrap();
        let mut s = user.clone();
        let under = put_over(&project, &mut s);
        assert_eq!((s.tab_size, s.soft_wrap), (2, true));
        assert_eq!(under.tab_size, Some(4));
        assert_eq!(under.show_whitespace, None, "not overridden, nothing kept");
        // The user changes something else meanwhile: kept when the user's values come back.
        s.fetch_minutes = 30;
        take_back(&under, &mut s);
        assert_eq!(s, Settings { fetch_minutes: 30, ..user });
    }

    #[test]
    fn the_file_reads_hand_written_values_and_says_what_is_wrong() {
        let p = parse(r#"{ "spring_profiles": ["dev", "local"], "tab_size": 40 }"#).unwrap();
        assert_eq!(p.spring_profiles, Some(ActiveProfiles::Chosen(vec!["dev".into(), "local".into()])));
        assert_eq!(p.tab_size, Some(8), "brought within range");
        assert_eq!(
            parse(r#"{ "spring_profiles": "config" }"#).unwrap().spring_profiles.as_ref().map(ProfileMode::from),
            Some(ProfileMode::Config)
        );
        assert!(parse("").unwrap().is_empty());
        let e = parse(r#"{ "tab_size": "two" }"#).unwrap_err();
        assert!(e.contains("invalid type"), "{e}");
        assert!(parse("{ \"tab_size\": 2,").is_err());
    }

    #[test]
    fn writing_keeps_the_keys_it_does_not_know() {
        let host = snout_host::local();
        let root = crate::tests::scratch_dir("project-settings-write");
        std::fs::create_dir_all(root.join(".snout")).unwrap();
        std::fs::write(file_of(&root), r#"{ "team_note": "keep me", "tab_size": 8 }"#).unwrap();
        let values = ProjectSettings { soft_wrap: Some(true), ..Default::default() };
        write(&*host, &root, &values).unwrap();
        let OnDisk::Text(text) = read(&*host, &root) else { panic!("written") };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v, serde_json::json!({ "team_note": "keep me", "soft_wrap": true }));
        assert_eq!(parse(&text).unwrap(), values);
        let _ = std::fs::remove_dir_all(root);
    }
}
