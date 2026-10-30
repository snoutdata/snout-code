//! What differs between the systems SnoutCode runs on, behind one small interface: macOS
//! (`mac.rs`) and Linux (`linux.rs`). The rest of the app asks here rather than testing the
//! system itself. Other platform code lives in modules of its own: the menu bar's AppKit icons
//! (`menu_icons`), the terminal apps (`external_terminal`), the keys (`keymap::platform_keys`),
//! and the in-window menu bar drawn where there is no global one (`menu_bar`).

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as imp;

#[cfg(not(target_os = "macos"))]
mod linux;
#[cfg(not(target_os = "macos"))]
use linux as imp;

/// The app's id on the desktop: the windows' Wayland app id and X11 class, which the
/// `.desktop` file of the Linux packages names (`StartupWMClass`), so the desktop shows the
/// windows under its icon.
pub const APP_ID: &str = "snout-code";

pub use imp::{IN_WINDOW_MENUS, REVEAL, THIS_MACHINE, process_start, window_decorations};

/// The folders a packaged SnoutCode keeps its resources in (the engine, the Linux servers for
/// Remote SSH), for the program at `exe`, most likely first.
pub fn resource_dirs(exe: &Path) -> Vec<PathBuf> {
    imp::resource_dirs(exe)
}

/// The resource `name` of a packaged app (see [`resource_dirs`]) that `ok` accepts, else the
/// one a development build has at `dev` (relative to an ancestor of the program, as
/// `engine/build/dist`).
pub fn find_resource(exe: &Path, name: &str, dev: &str, ok: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    resource_dirs(exe)
        .into_iter()
        .map(|d| d.join(name))
        .chain(exe.ancestors().skip(1).map(|d| d.join(dev)))
        .find(|d| ok(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packaged_app_finds_its_resources_beside_it() {
        let exe = if cfg!(target_os = "macos") {
            Path::new("/Applications/SnoutCode.app/Contents/MacOS/snout-code")
        } else {
            Path::new("/usr/lib/snout-code/snout-code")
        };
        let want = if cfg!(target_os = "macos") {
            Path::new("/Applications/SnoutCode.app/Contents/Resources/engine")
        } else {
            Path::new("/usr/lib/snout-code/engine")
        };
        assert_eq!(find_resource(exe, "engine", "engine/build/dist", |d| d == want).as_deref(), Some(want));
        let dev = Path::new("/w/snout-code/engine/build/dist");
        let exe = Path::new("/w/snout-code/target/release/snout-code");
        assert_eq!(find_resource(exe, "engine", "engine/build/dist", |d| d == dev).as_deref(), Some(dev));
    }
}
