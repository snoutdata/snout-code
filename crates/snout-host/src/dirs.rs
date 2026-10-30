//! Where SnoutCode keeps its own files on a host, by the conventions of the host's system.
//!
//! On a Mac: `~/Library/Application Support/SnoutCode` for settings and data, and
//! `~/Library/Caches/SnoutCode` for the cache. On Linux, the XDG base directories:
//! `$XDG_CONFIG_HOME/SnoutCode` (else `~/.config/SnoutCode`) for settings, the data, cache
//! and state homes likewise (`~/.local/share`, `~/.cache`, `~/.local/state`). A variable that
//! is empty or not an absolute path is ignored, as the specification says. On Windows:
//! `%APPDATA%\SnoutCode` for settings (it roams with the user), `%LOCALAPPDATA%\SnoutCode`
//! for the data, with `cache` and `logs` folders in it.

use std::path::{Path, PathBuf};

use crate::HostInfo;

/// The folder name under each base directory.
pub const APP: &str = "SnoutCode";

/// SnoutCode's folders on a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// Settings, keymaps, the layout: what the user would back up.
    pub config: PathBuf,
    /// What the app downloads and keeps (JDKs).
    pub data: PathBuf,
    /// What can be made again (the engine's builds, models and jdtls data).
    pub cache: PathBuf,
    /// What is kept between runs but is not worth backing up (logs, history).
    pub state: PathBuf,
}

impl AppDirs {
    /// The folders on the host `info` describes.
    pub fn of(info: &HostInfo) -> Self {
        Self::for_os(&info.os, &info.home, |name| info.var(name).map(str::to_owned))
    }

    /// The folders of this process's user, on this machine.
    pub fn local() -> Self {
        let home = crate::home_dir();
        Self::for_os(std::env::consts::OS, &home, |name| std::env::var(name).ok())
    }

    /// The folders for a system `os` (as `std::env::consts::OS`) and a `home`, with the
    /// environment read through `var`.
    pub fn for_os(os: &str, home: &Path, var: impl Fn(&str) -> Option<String>) -> Self {
        if os == "macos" {
            let support = home.join("Library/Application Support").join(APP);
            return Self {
                config: support.clone(),
                data: support,
                cache: home.join("Library/Caches").join(APP),
                state: home.join("Library/Logs").join(APP),
            };
        }
        if os == "windows" {
            let var_dir = |name: &str, fallback: &str| {
                var(name).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(fallback))
            };
            let roaming = var_dir("APPDATA", "AppData\\Roaming").join(APP);
            let local = var_dir("LOCALAPPDATA", "AppData\\Local").join(APP);
            return Self {
                config: roaming,
                data: local.clone(),
                cache: local.join("cache"),
                state: local.join("logs"),
            };
        }
        let base = |name: &str, fallback: &str| {
            var(name).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(fallback)).join(APP)
        };
        Self {
            config: base("XDG_CONFIG_HOME", ".config"),
            data: base("XDG_DATA_HOME", ".local/share"),
            cache: base("XDG_CACHE_HOME", ".cache"),
            state: base("XDG_STATE_HOME", ".local/state"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_mac_keeps_them_in_library_and_linux_in_the_xdg_folders() {
        let home = Path::new("/home/ann");
        let none = |_: &str| None;
        let mac = AppDirs::for_os("macos", Path::new("/Users/ann"), none);
        assert_eq!(mac.config, Path::new("/Users/ann/Library/Application Support/SnoutCode"));
        assert_eq!(mac.cache, Path::new("/Users/ann/Library/Caches/SnoutCode"));
        let linux = AppDirs::for_os("linux", home, none);
        assert_eq!(linux.config, Path::new("/home/ann/.config/SnoutCode"));
        assert_eq!(linux.data, Path::new("/home/ann/.local/share/SnoutCode"));
        assert_eq!(linux.cache, Path::new("/home/ann/.cache/SnoutCode"));
        assert_eq!(linux.state, Path::new("/home/ann/.local/state/SnoutCode"));
        let set = |name: &str| match name {
            "XDG_CONFIG_HOME" => Some("/cfg".to_owned()),
            "XDG_CACHE_HOME" => Some("relative".to_owned()),
            "XDG_DATA_HOME" => Some(String::new()),
            _ => None,
        };
        let linux = AppDirs::for_os("linux", home, set);
        assert_eq!(linux.config, Path::new("/cfg/SnoutCode"));
        assert_eq!(linux.cache, Path::new("/home/ann/.cache/SnoutCode"), "a relative path is ignored");
        assert_eq!(linux.data, Path::new("/home/ann/.local/share/SnoutCode"), "an empty one too");
    }

    #[cfg(windows)]
    #[test]
    fn windows_keeps_settings_in_roaming_and_the_rest_in_local_app_data() {
        let set = |name: &str| match name {
            "APPDATA" => Some(r"C:\Users\ann\AppData\Roaming".to_owned()),
            "LOCALAPPDATA" => Some(r"C:\Users\ann\AppData\Local".to_owned()),
            _ => None,
        };
        let win = AppDirs::for_os("windows", Path::new(r"C:\Users\ann"), set);
        assert_eq!(win.config, Path::new(r"C:\Users\ann\AppData\Roaming\SnoutCode"));
        assert_eq!(win.data, Path::new(r"C:\Users\ann\AppData\Local\SnoutCode"));
        assert_eq!(win.cache, Path::new(r"C:\Users\ann\AppData\Local\SnoutCode\cache"));
        let bare = AppDirs::for_os("windows", Path::new(r"C:\Users\ann"), |_| None);
        assert_eq!(bare.state, Path::new(r"C:\Users\ann\AppData\Local\SnoutCode\logs"));
    }
}
