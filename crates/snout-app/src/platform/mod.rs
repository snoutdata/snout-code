//! What differs between the systems SnoutCode runs on, behind one small interface: macOS
//! (`mac.rs`), Linux (`linux.rs`) and Windows (`windows.rs`). The rest of the app asks here rather than testing the
//! system itself. Other platform code lives in modules of its own: the menu bar's AppKit icons
//! (`menu_icons`), the terminal apps (`external_terminal`), the keys (`keymap::platform_keys`),
//! and the in-window menu bar drawn where there is no global one (`menu_bar`).

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as imp;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
use linux as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

/// The app's id on the desktop: the windows' Wayland app id and X11 class, which the
/// `.desktop` file of the Linux packages names (`StartupWMClass`), so the desktop shows the
/// windows under its icon.
pub const APP_ID: &str = "snout-code";

pub use imp::{IN_WINDOW_MENUS, process_start, reveal, this_machine, window_decorations};

/// Answer the system's "Open in SnoutCode" service, where it has one (a Mac's Services menu):
/// what it hands over goes to `asked`, as `file://` URLs.
pub fn offer_open_service(_asked: futures::channel::mpsc::UnboundedSender<Vec<String>>) {
    #[cfg(target_os = "macos")]
    imp::offer_open_service(_asked);
}

/// `url` as a `file://` URL with a path: on a Mac, Finder may hand a file over by reference
/// instead, which is resolved here. Elsewhere, and for any other URL, it is as it was.
pub fn file_url(url: String) -> String {
    #[cfg(target_os = "macos")]
    return imp::file_url(url);
    #[cfg(not(target_os = "macos"))]
    url
}

/// A long run ended: where the system has a way to say so while the app is in the
/// background (the Dock's bounce and badge on a Mac), it does. `badge` is what it wears.
pub fn attention(_badge: &str) {
    #[cfg(target_os = "macos")]
    imp::attention(_badge);
}

/// The window is in front again: what [`attention`] put up goes.
pub fn clear_attention() {
    #[cfg(target_os = "macos")]
    imp::clear_attention();
}

/// The app (not the reaper, not ssh's password prompt) is starting: on Windows, what it
/// starts is tied to it, and stderr goes to the console it was started from.
pub fn app_started() {
    #[cfg(windows)]
    imp::app_started();
}

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

    #[cfg(target_os = "macos")]
    #[test]
    fn a_folder_finder_hands_over_by_reference_opens_at_its_path() {
        use objc2_foundation::{NSString, NSURL};
        let dir = std::env::temp_dir().join(format!("snout-file-ref-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let url = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
        let by_reference = url.fileReferenceURL().and_then(|u| u.absoluteString()).unwrap().to_string();
        assert!(by_reference.starts_with("file:///.file/id="), "{by_reference}");
        let path = crate::system_open::path_of_url(&file_url(by_reference)).unwrap();
        assert_eq!(path, dir);
        assert_eq!(file_url("file:///tmp/a%20b".into()), "file:///tmp/a%20b", "a path URL is left as it is");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_packaged_app_finds_its_resources_beside_it() {
        let exe = if cfg!(target_os = "macos") {
            Path::new("/Applications/SnoutCode.app/Contents/MacOS/snout-code")
        } else if cfg!(windows) {
            Path::new(r"C:\Users\ann\AppData\Local\Programs\SnoutCode\snout-code.exe")
        } else {
            Path::new("/usr/lib/snout-code/snout-code")
        };
        let want = if cfg!(target_os = "macos") {
            Path::new("/Applications/SnoutCode.app/Contents/Resources/engine")
        } else if cfg!(windows) {
            Path::new(r"C:\Users\ann\AppData\Local\Programs\SnoutCode\engine")
        } else {
            Path::new("/usr/lib/snout-code/engine")
        };
        assert_eq!(find_resource(exe, "engine", "engine/build/dist", |d| d == want).as_deref(), Some(want));
        let dev = Path::new("/w/snout-code/engine/build/dist");
        let exe = Path::new("/w/snout-code/target/release/snout-code");
        assert_eq!(find_resource(exe, "engine", "engine/build/dist", |d| d == dev).as_deref(), Some(dev));
    }
}
