//! macOS: a global menu bar, the window's own title bar, Finder, and the app bundle.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The menus are the system's menu bar.
pub const IN_WINDOW_MENUS: bool = false;

/// What the app calls showing a file in the file manager.
pub const REVEAL: &str = "Reveal in Finder";

/// The machine the app runs on, as messages name it.
pub const THIS_MACHINE: &str = "this Mac";

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
