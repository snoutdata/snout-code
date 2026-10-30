//! Linux: no global menu bar (the window draws its own), client-side decorations where the
//! compositor wants them, the desktop's file manager, and the package's layout
//! (`/usr/lib/snout-code` beside `/usr/bin/snout-code`).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The window draws the menus in its title bar.
pub const IN_WINDOW_MENUS: bool = true;

/// What the app calls showing a file in the file manager.
pub const REVEAL: &str = "Show in Files";

/// The machine the app runs on, as messages name it.
pub const THIS_MACHINE: &str = "this computer";

/// The app draws the title bar, its buttons and the window's edges (a compositor that
/// decorates windows itself still may: the title bar then leaves the buttons out).
pub fn window_decorations() -> Option<gpui::WindowDecorations> {
    Some(gpui::WindowDecorations::Client)
}

/// The program's own folder (`/usr/lib/snout-code/snout-code` keeps `engine/` beside it),
/// then `lib/snout-code` and `share/snout-code` of the prefix it is in (`/usr/bin`, or a
/// tarball's `bin`).
pub fn resource_dirs(exe: &Path) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else { return Vec::new() };
    let mut out = vec![dir.to_path_buf()];
    if let Some(prefix) = dir.parent() {
        out.push(prefix.join("lib/snout-code"));
        out.push(prefix.join("share/snout-code"));
    }
    out
}

/// When this process started: its start in clock ticks after boot (`/proc/self/stat`),
/// after the boot time (`/proc/stat`).
pub fn process_start() -> Option<SystemTime> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The command's name is in parentheses and may hold spaces: the fields follow the last one.
    let fields: Vec<&str> = stat.get(stat.rfind(')')? + 2..)?.split_whitespace().collect();
    // The start time is field 22; the fields here start at field 3.
    let ticks: u64 = fields.get(19)?.parse().ok()?;
    let boot: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()?;
    // SAFETY: sysconf has no preconditions.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let hz = u64::try_from(hz).ok().filter(|h| *h > 0)?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(boot) + Duration::from_millis(ticks * 1000 / hz))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_process_started_a_moment_ago() {
        let start = super::process_start().expect("/proc says when");
        let ago = std::time::SystemTime::now().duration_since(start).expect("in the past");
        assert!(ago < std::time::Duration::from_secs(3600), "{ago:?}");
    }
}
