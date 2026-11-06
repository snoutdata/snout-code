//! SnoutCode's updates, without the window: what the release feed says and whether to believe
//! it, what to do on this machine, the download, and putting the new version in place. The app
//! (`snout-app`'s `updates` module) shows the [`status::UpdateStatus`] this reports.
//!
//! The flow: some 30 seconds after launch, and every 4 hours after that, read the feed
//! (`latest.json` and its signature, from the newest published GitHub release). A newer version
//! downloads in the background, whole, and is checked and made ready beside the live app. The
//! app then offers Restart; a plain quit installs it too.
//!
//! THE RULE: AN UPDATE MUST NEVER STRAND AN INSTALL, because a broken install has no updater left
//! to fix it. Whatever fails leaves the running version as it is, and says where to get the new
//! one by hand.

pub mod failure;
pub mod fetch;
pub mod flow;
pub mod manifest;
pub mod record;
pub mod route;
pub mod stage;
pub mod status;
pub mod swap;
pub mod version;

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// How often a running app looks at the feed again: an app left open for days still hears of a
/// release, and the feed is asked rarely.
pub const RECHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(4 * 60 * 60);
/// How long after launch the first look waits: nothing competes with the first window.
pub const FIRST_CHECK_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// The folder of downloads and stages' records, in the app's cache.
pub fn updates_dir() -> PathBuf {
    snout_host::dirs::AppDirs::local().cache.join("updates")
}

/// `update.log`, among the app's logs: every step, since an install that goes wrong does so on
/// someone's machine, with nothing else to say why.
pub fn log_path() -> PathBuf {
    snout_host::dirs::AppDirs::local().state.join("update.log")
}

/// A line in `update.log`. Logging never fails an update. The log starts over at 1 MB.
pub fn log_to(file: &Path, message: &str) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(file).is_ok_and(|m| m.len() > 1 << 20) {
        let _ = std::fs::remove_file(file);
    }
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{seconds} {message}");
    }
}

/// A process of ours: no signal blocked, as `snout_host::default_signals` says, since the threads
/// this runs on block them and a child inherits that (and the program a child starts, so a new
/// version started by the waiter would not answer a `kill`); on Windows, no console window.
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    snout_host::default_signals(&mut c);
    c
}
