//! macOS: a global menu bar, the window's own title bar, Finder, and the app bundle.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The menus are the system's menu bar.
pub const IN_WINDOW_MENUS: bool = false;

/// The file manager's command that shows a file there.
pub fn reveal() -> String {
    t!("reveal-mac")
}

/// The machine the app runs on, as messages name it.
pub fn this_machine() -> &'static str {
    ts!("machine-mac")
}

/// The window keeps macOS's own decorations (the app draws in its title bar).
pub fn window_decorations() -> Option<gpui::WindowDecorations> {
    None
}

/// The bundle's `Contents/Resources`: in an ancestor of the program, as `Contents/MacOS/x`.
pub fn resource_dirs(exe: &Path) -> Vec<PathBuf> {
    exe.ancestors().skip(1).map(|d| d.join("Resources")).collect()
}

/// When this process started, by the kernel's record of it.
pub fn process_start() -> Option<SystemTime> {
    // SAFETY: `proc_bsdinfo` is plain data, valid all zeros.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` is a `proc_bsdinfo` of the size given.
    let got = unsafe {
        libc::proc_pidinfo(std::process::id() as libc::c_int, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size)
    };
    (got == size).then(|| {
        SystemTime::UNIX_EPOCH
            + Duration::from_secs(info.pbi_start_tvsec)
            + Duration::from_micros(info.pbi_start_tvusec)
    })
}

/// A long run ended while the app was in the background: the Dock icon bounces once and
/// wears `badge` until the app is in front again.
pub fn attention(badge: &str) {
    use objc2_app_kit::{NSApplication, NSRequestUserAttentionType};
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if app.isActive() {
        return;
    }
    app.requestUserAttention(NSRequestUserAttentionType::InformationalRequest);
    app.dockTile().setBadgeLabel(Some(&objc2_foundation::NSString::from_str(badge)));
}

/// The app is in front: the Dock icon's badge goes.
pub fn clear_attention() {
    use objc2_app_kit::NSApplication;
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    NSApplication::sharedApplication(mtm).dockTile().setBadgeLabel(None);
}
