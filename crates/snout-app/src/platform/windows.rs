//! Windows: the menus in the window, the title bar's own minimize, maximize and close buttons
//! (GPUI Kit's, on Windows's snap layouts), Explorer, the installer's layout (`engine\` beside
//! `snout-code.exe`), a job object that ends what the app started when it ends, and the
//! console of a command prompt the app was started from.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The window draws the menus in its title bar.
pub const IN_WINDOW_MENUS: bool = true;

/// What the app calls showing a file in the file manager.
pub const REVEAL: &str = "Show in Explorer";

/// The machine the app runs on, as messages name it.
pub const THIS_MACHINE: &str = "this PC";

/// The app draws the title bar and its buttons; Windows draws the frame and its shadow.
pub fn window_decorations() -> Option<gpui::WindowDecorations> {
    None
}

/// The program's own folder (the installer puts `engine\` and `server\` beside
/// `snout-code.exe`).
pub fn resource_dirs(exe: &Path) -> Vec<PathBuf> {
    exe.parent().map(|d| vec![d.to_path_buf()]).unwrap_or_default()
}

/// When this process started, by the system's record of it.
pub fn process_start() -> Option<SystemTime> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: the pseudo handle of this process, and four places for the times.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }
    // 100 ns intervals since 1601-01-01.
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    const UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;
    let since_1970 = ticks.checked_sub(UNIX_EPOCH_TICKS)?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(since_1970 * 100))
}

/// The app is starting (not the reaper, not ssh's password prompt): what it starts from now
/// on ends with it (a job object, see `snout_host::win`), and what it writes to stderr goes
/// to the console of the command prompt it was started from, if it was.
pub fn app_started() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE};
    // SAFETY: plain calls. Started with stderr going somewhere already (a file, a pipe), it
    // stays there; else to the console of the command prompt, if there is one.
    unsafe {
        let err = GetStdHandle(STD_ERROR_HANDLE);
        if err.is_null() || err == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
    if let Err(e) = snout_host::win::make_app_job() {
        eprintln!("[windows] no job object for the app's processes: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_process_started_a_moment_ago() {
        let start = super::process_start().expect("Windows says when");
        let ago = std::time::SystemTime::now().duration_since(start).expect("in the past");
        assert!(ago < std::time::Duration::from_secs(3600), "{ago:?}");
    }
}
