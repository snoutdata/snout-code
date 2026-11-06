//! What an update check does in this build, on this machine.

use std::path::{Path, PathBuf};

use crate::status::ManualReason;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// A build from the source: it has no feed, the check says so and asks nothing.
    FromSource,
    /// A newer version is only announced.
    Manual(ManualReason),
    /// A Mac app bundle, replaced by its new version.
    MacBundle { app: PathBuf },
    /// Windows: the new version's installer runs.
    WindowsInstaller { folder: PathBuf, exe: PathBuf },
}

/// The route for an `official` build on `os`, running `exe`. `writable` says whether a folder can
/// be written to.
pub fn route(official: bool, os: &str, exe: &Path, writable: impl Fn(&Path) -> bool) -> Route {
    if !official {
        return Route::FromSource;
    }
    match os {
        "macos" => mac_route(exe, writable),
        "windows" => windows_route(exe),
        _ => Route::Manual(ManualReason::PackageManager),
    }
}

fn mac_route(exe: &Path, writable: impl Fn(&Path) -> bool) -> Route {
    let cannot = Route::Manual(ManualReason::CannotReplace);
    // <app>.app/Contents/MacOS/snout-code
    let Some(app) = exe.ancestors().nth(3).filter(|a| a.extension().is_some_and(|e| e == "app")) else {
        return cannot;
    };
    // Gatekeeper runs a downloaded app from a random read-only place until it is moved.
    if app.to_string_lossy().contains("/AppTranslocation/") {
        return cannot;
    }
    match app.parent() {
        Some(folder) if writable(folder) && writable(app) => Route::MacBundle { app: app.to_owned() },
        _ => cannot,
    }
}

fn windows_route(exe: &Path) -> Route {
    let Some(folder) = exe.parent() else { return Route::Manual(ManualReason::CannotReplace) };
    // Installed by the installer: its uninstaller is beside the program.
    let installed = std::fs::read_dir(folder).is_ok_and(|dir| {
        dir.flatten().any(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.starts_with("unins") && name.ends_with(".exe")
        })
    });
    if installed {
        Route::WindowsInstaller { folder: folder.to_owned(), exe: exe.to_owned() }
    } else {
        Route::Manual(ManualReason::CannotReplace)
    }
}

/// Whether a file can be made in `folder`. Actually making one, since the permission bits do not
/// say (Windows only reads the read-only attribute, and a read-only disk image has fine bits).
pub fn can_write(folder: &Path) -> bool {
    let probe = folder.join(format!(".snout-update-probe-{}", std::process::id()));
    match std::fs::create_dir(&probe) {
        Ok(()) => std::fs::remove_dir(&probe).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_from_the_source_never_asks_the_feed() {
        assert_eq!(route(false, "macos", Path::new("/x"), |_| true), Route::FromSource);
        assert_eq!(route(false, "linux", Path::new("/x"), |_| true), Route::FromSource);
    }

    #[test]
    fn linux_leaves_the_install_to_the_package_manager() {
        let r = route(true, "linux", Path::new("/usr/bin/snout-code"), |_| true);
        assert_eq!(r, Route::Manual(ManualReason::PackageManager));
    }

    #[test]
    fn a_mac_app_replaces_itself_only_from_where_it_can() {
        let exe = Path::new("/Applications/SnoutCode.app/Contents/MacOS/snout-code");
        let app = PathBuf::from("/Applications/SnoutCode.app");
        assert_eq!(route(true, "macos", exe, |_| true), Route::MacBundle { app: app.clone() });
        let cannot = Route::Manual(ManualReason::CannotReplace);
        assert_eq!(route(true, "macos", exe, |_| false), cannot, "not writable, as on a disk image");
        assert_eq!(route(true, "macos", exe, |f| f != app.as_path()), cannot, "a bundle owned by someone else");
        let moved = Path::new("/private/var/folders/x/AppTranslocation/y/d/SnoutCode.app/Contents/MacOS/snout-code");
        assert_eq!(route(true, "macos", moved, |_| true), cannot, "translocated by Gatekeeper");
        assert_eq!(route(true, "macos", Path::new("/usr/local/bin/snout-code"), |_| true), cannot, "not in a bundle");
    }

    #[test]
    fn windows_updates_only_what_the_installer_installed() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("snout-code.exe");
        std::fs::write(&exe, "").unwrap();
        let cannot = Route::Manual(ManualReason::CannotReplace);
        assert_eq!(route(true, "windows", &exe, |_| true), cannot);
        std::fs::write(dir.path().join("unins000.exe"), "").unwrap();
        let r = route(true, "windows", &exe, |_| true);
        assert_eq!(r, Route::WindowsInstaller { folder: dir.path().to_owned(), exe });
    }

    #[test]
    fn writing_is_found_by_trying_and_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(can_write(dir.path()));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(!can_write(&dir.path().join("not-there")));
    }
}
