//! THE WAITER: this program again, without a window, started as the app quits (as the reaper is).
//! It waits for the app to be gone, puts the new version in place, and on a Restart starts it.
//!
//! The decisions are here with every side effect behind [`Deps`], so all of them are tested;
//! [`OsDeps`] is the thin real implementation.
//!
//! On a Mac the swap is one atomic exchange of two folders, so there is no moment without an app;
//! where the volume cannot do that (not APFS), two renames, in the order that keeps an install
//! whole. On Windows the installer does the work, and the waiter runs from a copy of the program,
//! since the installer replaces the original. Whatever fails, Restart still starts whichever
//! version is there: nobody is left looking at nothing.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The variable that makes this program the waiter: it names the [`WaiterConfig`] file.
pub const WAITER_ENV: &str = "SNOUT_UPDATE_WAITER";

const LOCK: &str = "swapping";
/// The app has at most `EXIT_CAP` (4 s) after its window went.
const APP_GONE_WITHIN: Duration = Duration::from_secs(30);
/// What the app leaves running under the install folder (the reaper stops it within 12 s).
const OTHERS_GONE_WITHIN: Duration = Duration::from_secs(20);
const RENAME_TRIES: u32 = 5;
/// Nothing but us has a reason to hold the folder we just moved.
const RESTORE_TRIES: u32 = 600;
/// Long enough for someone to answer an administrator's prompt; a stuck installer must not hold
/// the update, and every launch behind it, for ever.
const INSTALLER_WITHIN: Duration = Duration::from_secs(10 * 60);
/// A launch waits as long as a waiter that is really there can take: starting sooner would put the
/// app in the folder its installer is about to replace, which would close it.
const LOCK_WAIT: Duration = Duration::from_secs(11 * 60);
/// A lock older than any waiter lasts was left behind (a power cut), and is not waited on.
const STALE_LOCK: Duration = Duration::from_secs(15 * 60);

/// How the swap was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Restart: the new version starts, and an installer shows itself.
    Restart,
    /// A plain quit: it is put in place quietly, and nothing starts.
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plan {
    Bundle { install: PathBuf, next: PathBuf, old: PathBuf },
    Installer { installer: PathBuf, folder: PathBuf, exe: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaiterConfig {
    pub app_pid: u32,
    pub mode: Mode,
    pub version: String,
    pub plan: Plan,
    /// Where the `swapping` lock goes.
    pub updates: PathBuf,
    pub log: PathBuf,
    /// Start the new app directly rather than through the system: a test run, which keeps its
    /// environment (its scratch HOME).
    #[serde(default)]
    pub direct_launch: bool,
}

/// What the waiter can do to the world.
pub trait Deps {
    fn alive(&self, pid: u32) -> bool;
    /// Processes running from inside `folder`, other than this one.
    fn others_in(&self, folder: &Path) -> usize;
    fn exists(&self, path: &Path) -> bool;
    /// Exchange two folders atomically; false when this volume cannot.
    fn exchange(&self, a: &Path, b: &Path) -> bool;
    fn rename(&self, from: &Path, to: &Path) -> bool;
    fn remove_dir(&self, path: &Path);
    fn write(&self, path: &Path, text: &str);
    /// Whether this user can write in `folder`.
    fn can_write(&self, folder: &Path) -> bool;
    /// This process.
    fn own_pid(&self) -> u32;
    fn remove_file(&self, path: &Path);
    fn sleep(&self, d: Duration);
    /// Run a program to its end: its exit code, None when it never ran or crashed.
    fn run(&self, file: &Path, args: &[&str]) -> Option<i32>;
    /// Start the app, detached.
    fn launch(&self, app: &Path, direct: bool);
    fn log(&self, message: &str);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The app never went: nothing was touched.
    AppStayed,
    Swapped,
    /// It could not be put in place; the live install is as it was.
    Failed,
}

/// The installer's arguments. It needs no answers either way; on a Restart it shows its progress.
pub fn installer_args(mode: Mode) -> Vec<&'static str> {
    let shown = if mode == Mode::Restart { "/SILENT" } else { "/VERYSILENT" };
    vec![shown, "/SUPPRESSMSGBOXES", "/NORESTART", "/NOCANCEL", "/SP-", "/CLOSEAPPLICATIONS", "/FORCECLOSEAPPLICATIONS"]
}

pub fn run_waiter(cfg: &WaiterConfig, d: &dyn Deps) -> Outcome {
    d.log(&format!("waiting for the app (pid {}) to quit, v{} {:?}", cfg.app_pid, cfg.version, cfg.mode));
    if !wait_until(d, APP_GONE_WITHIN, || !d.alive(cfg.app_pid)) {
        d.log("the app is still there: no swap");
        return Outcome::AppStayed;
    }
    let lock = cfg.updates.join(LOCK);
    // Who holds it: a launch ignores the lock of a waiter that is gone.
    d.write(&lock, &d.own_pid().to_string());
    let outcome = match &cfg.plan {
        Plan::Bundle { install, next, old } => swap_bundle(cfg, install, next, old, d),
        Plan::Installer { installer, folder, exe } => {
            let installed = run_installer(cfg, installer, folder, d);
            // Installed or not, Restart starts whichever version is there.
            if cfg.mode == Mode::Restart {
                d.launch(exe, true);
            }
            if installed { Outcome::Swapped } else { Outcome::Failed }
        }
    };
    d.remove_file(&lock);
    outcome
}

fn swap_bundle(cfg: &WaiterConfig, install: &Path, next: &Path, old: &Path, d: &dyn Deps) -> Outcome {
    let start = |d: &dyn Deps| {
        if cfg.mode == Mode::Restart {
            d.launch(install, cfg.direct_launch);
        }
    };
    if !d.exists(next) {
        d.log("the stage is missing");
        start(d);
        return Outcome::Failed;
    }
    if d.exchange(next, install) {
        d.log(&format!("swapped in v{} (exchanged)", cfg.version));
        start(d);
        // Now `next` holds the old app. Started first: this can take a moment.
        d.remove_dir(next);
        return Outcome::Swapped;
    }
    // A volume with no exchange. The order keeps an install whole: the live app moves aside only
    // with the stage known to be there, and if the stage then cannot move in, it goes back first.
    d.remove_dir(old);
    if !retry(d, RENAME_TRIES, || d.rename(install, old)) {
        d.log("the live app could not be moved aside");
        start(d);
        return Outcome::Failed;
    }
    if !retry(d, RENAME_TRIES, || d.rename(next, install)) {
        d.log("the stage could not be moved in; putting the live app back");
        if !retry(d, RESTORE_TRIES, || d.rename(old, install)) {
            d.log("the live app could not be put back");
        }
        start(d);
        return Outcome::Failed;
    }
    d.log(&format!("swapped in v{} (renamed)", cfg.version));
    start(d);
    d.remove_dir(old);
    Outcome::Swapped
}

fn run_installer(cfg: &WaiterConfig, installer: &Path, folder: &Path, d: &dyn Deps) -> bool {
    // The reaper and what it is stopping run from the install folder, and hold its files.
    if !wait_until(d, OTHERS_GONE_WITHIN, || d.others_in(folder) == 0) {
        d.log("something still runs from the install folder; the installer closes it");
    }
    // Its own account of what it did, beside the update's. Only where the installer runs as this
    // user: run as an administrator it may not reach this user's folder, and Setup gives up when it
    // cannot write its log.
    let log = format!("/LOG={}", cfg.log.with_file_name("update-installer.log").display());
    let mut args = installer_args(cfg.mode);
    if d.can_write(folder) {
        args.push(&log);
    }
    let code = d.run(installer, &args);
    d.log(&format!("the installer of v{} exited: {code:?}", cfg.version));
    code == Some(0)
}

fn wait_until(d: &dyn Deps, within: Duration, mut done: impl FnMut() -> bool) -> bool {
    const STEP: Duration = Duration::from_millis(250);
    let mut waited = Duration::ZERO;
    while !done() {
        if waited >= within {
            return false;
        }
        d.sleep(STEP);
        waited += STEP;
    }
    true
}

fn retry(d: &dyn Deps, tries: u32, mut attempt: impl FnMut() -> bool) -> bool {
    for _ in 0..tries {
        if attempt() {
            return true;
        }
        d.sleep(Duration::from_secs(1));
    }
    false
}

/// Called first thing at launch. A launch that lands while a swap is under way (someone clicks the
/// icon the second they quit) must not start from what is being replaced: it waits for the swap,
/// then starts what is there now. True when this process should exit, having started that.
/// One file check when there is no swap, which is every launch but that one.
///
/// This wait shows nothing, so it never outlasts the reason for it: a lock whose waiter is gone
/// (killed, or left by an older version, which did not say who it was) is ignored and removed, and
/// a waiter that takes too long is left to it while the app starts.
pub fn wait_out_swap(updates: &Path, exe: &Path) -> bool {
    let lock = updates.join(LOCK);
    let Ok(holder) = std::fs::read_to_string(&lock) else { return false };
    let fresh = std::fs::metadata(&lock)
        .and_then(|m| m.modified())
        .is_ok_and(|at| at.elapsed().unwrap_or(Duration::MAX) <= STALE_LOCK);
    let waiter = holder.trim().parse::<u32>().ok().filter(|pid| fresh && pid_alive(*pid));
    let Some(waiter) = waiter else {
        let _ = std::fs::remove_file(&lock);
        return false;
    };
    let started = Instant::now();
    while lock.exists() && started.elapsed() < LOCK_WAIT {
        if !pid_alive(waiter) {
            let _ = std::fs::remove_file(&lock);
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if lock.exists() {
        return false;
    }
    crate::command(exe).args(std::env::args_os().skip(1)).spawn().is_ok()
}

/// Whether the waiter `pid` runs: a process of ours, since the system reuses numbers.
fn pid_alive(pid: u32) -> bool {
    snout_host::local()
        .processes(false)
        .is_ok_and(|all| all.iter().any(|p| p.pid == pid && p.name.to_lowercase().contains("snout")))
}

/// Where the waiter's program goes for an installer's update, for a running program of `version`:
/// a copy left by another version is never taken for this one's.
pub fn waiter_path(updates: &Path, version: &str) -> PathBuf {
    updates.join("waiter").join(format!("snout-code-update-{version}.exe"))
}

/// Copy the program `exe`, of `version`, to where the waiter runs from. A real copy, never a hard
/// link: a link is the very file the installer has to replace, and Windows does not replace a file
/// that is running. Whole or not at all: a process killed halfway leaves no program to run.
pub fn make_waiter_copy(updates: &Path, exe: &Path, version: &str) -> std::io::Result<PathBuf> {
    let copy = waiter_path(updates, version);
    let dir = copy.parent().unwrap_or(updates);
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    let part = copy.with_extension("part");
    std::fs::copy(exe, &part)?;
    std::fs::rename(&part, &copy)?;
    Ok(copy)
}

/// The real world, for the waiter process.
pub struct OsDeps {
    pub log: PathBuf,
    /// This process, which is itself under the old install on a Mac.
    pub own_pid: u32,
}

impl Deps for OsDeps {
    fn alive(&self, pid: u32) -> bool {
        snout_host::local().processes(false).is_ok_and(|all| all.iter().any(|p| p.pid == pid))
    }

    fn others_in(&self, folder: &Path) -> usize {
        let Ok(all) = snout_host::local().processes(false) else { return 0 };
        all.iter()
            .filter(|p| p.pid != self.own_pid)
            .filter(|p| p.cmd.first().is_some_and(|c| Path::new(c).starts_with(folder)))
            .count()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    #[cfg(target_os = "macos")]
    fn exchange(&self, a: &Path, b: &Path) -> bool {
        use std::os::unix::ffi::OsStrExt as _;
        let (Ok(a), Ok(b)) =
            (std::ffi::CString::new(a.as_os_str().as_bytes()), std::ffi::CString::new(b.as_os_str().as_bytes()))
        else {
            return false;
        };
        // SAFETY: two valid C strings, which the call only reads.
        unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) == 0 }
    }

    #[cfg(not(target_os = "macos"))]
    fn exchange(&self, _: &Path, _: &Path) -> bool {
        false
    }

    fn rename(&self, from: &Path, to: &Path) -> bool {
        std::fs::rename(from, to).is_ok()
    }

    fn remove_dir(&self, path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn write(&self, path: &Path, text: &str) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }

    fn can_write(&self, folder: &Path) -> bool {
        crate::route::can_write(folder)
    }

    fn own_pid(&self) -> u32 {
        self.own_pid
    }

    fn remove_file(&self, path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }

    fn run(&self, file: &Path, args: &[&str]) -> Option<i32> {
        let mut child = crate::command(file).args(args).env_remove(WAITER_ENV).spawn().ok()?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().ok()? {
                return status.code();
            }
            if started.elapsed() > INSTALLER_WITHIN {
                self.log("the installer is taking too long: stopping it");
                stop_tree(&mut child);
                return None;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn launch(&self, app: &Path, direct: bool) {
        let mut c;
        if cfg!(target_os = "macos") && !direct {
            c = crate::command("open");
            c.arg(app);
        } else if cfg!(target_os = "macos") {
            c = crate::command(crate::stage::bundle_exe(app));
        } else {
            c = crate::command(app);
        }
        c.env_remove(WAITER_ENV);
        if let Err(e) = spawn_detached(&mut c) {
            self.log(&format!("the app did not start: {e}"));
        }
    }

    fn log(&self, message: &str) {
        crate::log_to(&self.log, &format!("waiter: {message}"));
    }
}

/// End `child` and what it started (an installer unpacks itself and runs on as its child).
fn stop_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let _ = crate::command("taskkill").args(["/T", "/F", "/PID", &pid]).status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Start `c` so that it outlives this process, with nothing of its console or its group.
pub fn spawn_detached(c: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    use std::process::Stdio;
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        c.process_group(0).spawn()
    }
    #[cfg(windows)]
    {
        snout_host::win::spawn_detached(c)
    }
}

/// Start the waiter: `program` as the waiter, on the plan in `cfg`, written to the updates folder.
pub fn start_waiter(program: &Path, cfg: &WaiterConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.updates)?;
    let file = cfg.updates.join("waiter.json");
    std::fs::write(&file, serde_json::to_vec(cfg)?)?;
    spawn_detached(crate::command(program).env(WAITER_ENV, &file)).map(drop)
}

/// The waiter's whole life, in this process. Runs when [`WAITER_ENV`] is set.
pub fn waiter_main(config: &Path) {
    let Some(cfg) = std::fs::read(config).ok().and_then(|t| serde_json::from_slice::<WaiterConfig>(&t).ok()) else {
        return;
    };
    let deps = OsDeps { log: cfg.log.clone(), own_pid: std::process::id() };
    let outcome = run_waiter(&cfg, &deps);
    deps.log(&format!("done: {outcome:?}"));
    let _ = std::fs::remove_file(config);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use super::*;

    /// A world of folders, recording what was done.
    #[derive(Default)]
    struct Fake {
        folders: RefCell<HashSet<PathBuf>>,
        calls: RefCell<Vec<String>>,
        no_exchange: bool,
        /// Renames to these fail.
        stuck: Vec<PathBuf>,
        app_never_goes: bool,
        others: usize,
        installer_says: Option<i32>,
        all_users: bool,
        installer_log: std::cell::Cell<bool>,
    }

    impl Fake {
        fn with(mut self, folders: &[&Path]) -> Self {
            self.folders = RefCell::new(folders.iter().map(|f| f.to_path_buf()).collect());
            self
        }
        /// What was done to the world, without the lock around it.
        fn calls(&self) -> Vec<String> {
            let lock = format!("/u{}{LOCK}", std::path::MAIN_SEPARATOR);
            let calls = self.calls.borrow();
            calls.iter().filter(|c| !c.contains(&lock)).cloned().collect()
        }
        fn note(&self, s: String) {
            self.calls.borrow_mut().push(s);
        }
        fn has(&self, p: &Path) -> bool {
            self.folders.borrow().contains(p)
        }
    }

    impl Deps for Fake {
        fn alive(&self, _: u32) -> bool {
            self.app_never_goes
        }
        fn others_in(&self, _: &Path) -> usize {
            self.others
        }
        fn exists(&self, path: &Path) -> bool {
            self.has(path)
        }
        fn exchange(&self, a: &Path, b: &Path) -> bool {
            if self.no_exchange {
                return false;
            }
            self.note(format!("exchange {} {}", a.display(), b.display()));
            true
        }
        fn rename(&self, from: &Path, to: &Path) -> bool {
            if self.stuck.iter().any(|s| s == to) || !self.has(from) {
                return false;
            }
            self.note(format!("rename {} {}", from.display(), to.display()));
            let mut f = self.folders.borrow_mut();
            f.remove(from);
            f.insert(to.to_owned());
            true
        }
        fn remove_dir(&self, path: &Path) {
            self.note(format!("remove {}", path.display()));
            self.folders.borrow_mut().remove(path);
        }
        fn write(&self, path: &Path, text: &str) {
            self.note(format!("write {} {text}", path.display()));
        }
        fn can_write(&self, _: &Path) -> bool {
            !self.all_users
        }
        fn own_pid(&self) -> u32 {
            42
        }
        fn remove_file(&self, path: &Path) {
            self.note(format!("remove-file {}", path.display()));
        }
        fn sleep(&self, _: Duration) {}
        fn run(&self, file: &Path, args: &[&str]) -> Option<i32> {
            self.note(format!("run {} {}", file.display(), args[0]));
            self.installer_log.replace(args.iter().any(|a| a.starts_with("/LOG=")));
            self.installer_says
        }
        fn launch(&self, app: &Path, _: bool) {
            self.note(format!("launch {}", app.display()));
        }
        fn log(&self, _: &str) {}
    }

    fn bundle_cfg(mode: Mode) -> WaiterConfig {
        WaiterConfig {
            app_pid: 1,
            mode,
            version: "1.0.1".into(),
            plan: Plan::Bundle { install: "/a/S.app".into(), next: "/a/n.app".into(), old: "/a/o.app".into() },
            updates: "/u".into(),
            log: "/l".into(),
            direct_launch: false,
        }
    }

    const LIVE: &str = "/a/S.app";
    const NEXT: &str = "/a/n.app";
    const OLD: &str = "/a/o.app";

    fn world() -> Fake {
        Fake::default().with(&[Path::new(LIVE), Path::new(NEXT)])
    }

    #[test]
    fn restart_exchanges_the_folders_and_starts_the_new_app_before_removing_the_old() {
        let d = world();
        assert_eq!(run_waiter(&bundle_cfg(Mode::Restart), &d), Outcome::Swapped);
        assert_eq!(d.calls(), [format!("exchange {NEXT} {LIVE}"), format!("launch {LIVE}"), format!("remove {NEXT}")]);
    }

    #[test]
    fn a_plain_quit_swaps_and_starts_nothing() {
        let d = world();
        assert_eq!(run_waiter(&bundle_cfg(Mode::Quit), &d), Outcome::Swapped);
        assert!(!d.calls().iter().any(|c| c.starts_with("launch")));
    }

    #[test]
    fn an_app_that_never_goes_is_left_alone() {
        let d = Fake { app_never_goes: true, ..world() };
        assert_eq!(run_waiter(&bundle_cfg(Mode::Restart), &d), Outcome::AppStayed);
        assert_eq!(d.calls(), Vec::<String>::new());
    }

    #[test]
    fn without_an_exchange_it_renames_in_the_order_that_keeps_an_install() {
        let d = Fake { no_exchange: true, ..world() };
        assert_eq!(run_waiter(&bundle_cfg(Mode::Restart), &d), Outcome::Swapped);
        assert_eq!(
            d.calls(),
            [
                format!("remove {OLD}"),
                format!("rename {LIVE} {OLD}"),
                format!("rename {NEXT} {LIVE}"),
                format!("launch {LIVE}"),
                format!("remove {OLD}"),
            ]
        );
    }

    #[test]
    fn a_stage_that_cannot_move_in_puts_the_live_app_back_and_still_starts_it() {
        // Moving onto the install's path fails only the first RENAME_TRIES times in real life;
        // here the stage's own rename is what is stuck, and the app goes back by another way.
        struct StuckStage(Fake);
        impl Deps for StuckStage {
            fn alive(&self, p: u32) -> bool {
                self.0.alive(p)
            }
            fn others_in(&self, f: &Path) -> usize {
                self.0.others_in(f)
            }
            fn exists(&self, p: &Path) -> bool {
                self.0.exists(p)
            }
            fn exchange(&self, _: &Path, _: &Path) -> bool {
                false
            }
            fn rename(&self, from: &Path, to: &Path) -> bool {
                from != Path::new(NEXT) && self.0.rename(from, to)
            }
            fn remove_dir(&self, p: &Path) {
                self.0.remove_dir(p)
            }
            fn write(&self, _: &Path, _: &str) {}
            fn can_write(&self, _: &Path) -> bool {
                true
            }
            fn own_pid(&self) -> u32 {
                42
            }
            fn remove_file(&self, _: &Path) {}
            fn sleep(&self, _: Duration) {}
            fn run(&self, f: &Path, a: &[&str]) -> Option<i32> {
                self.0.run(f, a)
            }
            fn launch(&self, a: &Path, b: bool) {
                self.0.launch(a, b)
            }
            fn log(&self, _: &str) {}
        }
        let d = StuckStage(world());
        assert_eq!(run_waiter(&bundle_cfg(Mode::Restart), &d), Outcome::Failed);
        assert!(d.0.has(Path::new(LIVE)), "the live app is back");
        assert_eq!(d.0.calls().last().unwrap(), &format!("launch {LIVE}"));
    }

    #[test]
    fn a_live_app_that_cannot_move_aside_stays_and_starts() {
        let d = Fake { no_exchange: true, stuck: vec![OLD.into()], ..world() };
        assert_eq!(run_waiter(&bundle_cfg(Mode::Restart), &d), Outcome::Failed);
        assert!(d.has(Path::new(LIVE)) && d.has(Path::new(NEXT)));
        assert_eq!(d.calls().last().unwrap(), &format!("launch {LIVE}"));
    }

    #[test]
    fn a_missing_stage_touches_nothing_and_still_starts_the_app() {
        let d = Fake::default().with(&[Path::new(LIVE)]);
        assert_eq!(run_waiter(&bundle_cfg(Mode::Restart), &d), Outcome::Failed);
        assert_eq!(d.calls(), [format!("launch {LIVE}")]);
    }

    fn installer_cfg(mode: Mode) -> WaiterConfig {
        WaiterConfig {
            plan: Plan::Installer { installer: "/u/setup.exe".into(), folder: "/p".into(), exe: "/p/s.exe".into() },
            ..bundle_cfg(mode)
        }
    }

    #[test]
    fn the_installer_runs_then_a_restart_starts_the_app() {
        let d = Fake { installer_says: Some(0), ..Default::default() };
        assert_eq!(run_waiter(&installer_cfg(Mode::Restart), &d), Outcome::Swapped);
        assert_eq!(d.calls(), ["run /u/setup.exe /SILENT", "launch /p/s.exe"]);
        let d = Fake { installer_says: Some(0), ..Default::default() };
        run_waiter(&installer_cfg(Mode::Quit), &d);
        assert_eq!(d.calls(), ["run /u/setup.exe /VERYSILENT"], "a plain quit is quiet and starts nothing");
    }

    #[test]
    fn the_installer_runs_even_if_something_stays_in_the_folder() {
        let d = Fake { others: 1, ..Default::default() };
        run_waiter(&installer_cfg(Mode::Quit), &d);
        assert_eq!(d.calls(), ["run /u/setup.exe /VERYSILENT"], "it closes what is left");
    }

    #[test]
    fn an_installer_that_fails_or_never_ran_is_a_failure_and_a_restart_still_starts_the_app() {
        for says in [Some(5), None] {
            let d = Fake { installer_says: says, ..Default::default() };
            assert_eq!(run_waiter(&installer_cfg(Mode::Restart), &d), Outcome::Failed);
            assert_eq!(d.calls().last().unwrap(), "launch /p/s.exe");
        }
    }

    #[test]
    fn the_lock_says_who_holds_it_and_is_lifted_after_the_installer() {
        let d = Fake { installer_says: Some(0), ..Default::default() };
        run_waiter(&installer_cfg(Mode::Quit), &d);
        let all = d.calls.borrow().clone();
        let lock = Path::new("/u").join(LOCK);
        assert_eq!(all[0], format!("write {} 42", lock.display()));
        assert_eq!(all.last().unwrap(), &format!("remove-file {}", lock.display()));
    }

    #[test]
    fn a_launch_is_not_held_by_a_lock_nobody_holds() {
        let dir = tempfile::tempdir().unwrap();
        let none = Path::new("/none");
        assert!(!wait_out_swap(dir.path(), none), "no swap: one file check");
        let lock = dir.path().join(LOCK);
        // Left by an older version, which wrote nothing; a waiter that is gone; nonsense.
        let mut gone = std::process::Command::new(if cfg!(windows) { "whoami" } else { "true" })
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = gone.id();
        gone.wait().unwrap();
        for held_by in [String::new(), pid.to_string(), "not a pid".to_owned()] {
            std::fs::write(&lock, &held_by).unwrap();
            let started = Instant::now();
            assert!(!wait_out_swap(dir.path(), none), "it starts as usual: {held_by:?}");
            assert!(started.elapsed() < Duration::from_secs(5), "at once: {held_by:?}");
            assert!(!lock.exists(), "and the lock is gone: {held_by:?}");
        }
    }

    #[test]
    fn a_launch_waits_for_a_waiter_that_is_there_then_starts_what_is_there_now() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join(LOCK);
        std::fs::write(&lock, std::process::id().to_string()).unwrap();
        let lifted = lock.clone();
        let lifting = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            std::fs::remove_file(lifted).unwrap();
        });
        let started = Instant::now();
        // Something harmless that ends at once. Never this test's own program: it would run this
        // test again, which would start it again, without end.
        let exe = Path::new(if cfg!(windows) { "whoami" } else { "true" });
        assert!(wait_out_swap(dir.path(), exe), "it hands over to a new start");
        assert!(started.elapsed() >= Duration::from_millis(300), "after the swap");
        lifting.join().unwrap();
    }

    #[test]
    fn the_installers_log_is_asked_for_only_where_it_can_be_written() {
        let d = Fake { installer_says: Some(0), ..Default::default() };
        run_waiter(&installer_cfg(Mode::Quit), &d);
        assert!(d.installer_log.get(), "a per-user install keeps its log");
        let d = Fake { installer_says: Some(0), all_users: true, ..Default::default() };
        run_waiter(&installer_cfg(Mode::Restart), &d);
        assert!(!d.installer_log.get(), "Setup, elevated, would give up on this user's folder");
    }

    #[test]
    fn a_waiter_outlasts_any_installer_and_a_lock_outlasts_any_waiter() {
        assert!(LOCK_WAIT > INSTALLER_WITHIN + APP_GONE_WITHIN + OTHERS_GONE_WITHIN);
        assert!(STALE_LOCK > LOCK_WAIT);
    }

    #[test]
    fn the_waiters_program_is_a_real_copy_of_this_version_never_a_link_to_the_file_being_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("app.exe");
        std::fs::write(&exe, "the program").unwrap();
        let updates = dir.path().join("updates");
        let copy = make_waiter_copy(&updates, &exe, "1.0.1").unwrap();
        assert_eq!(copy, waiter_path(&updates, "1.0.1"));
        assert_ne!(
            waiter_path(&updates, "1.0.1"),
            waiter_path(&updates, "1.0.2"),
            "another version's is not this one's"
        );
        // Writing the original leaves the copy, as it never would a hard link.
        std::fs::write(&exe, "the new version").unwrap();
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "the program");
        // Nothing half made stays beside it.
        assert_eq!(std::fs::read_dir(copy.parent().unwrap()).unwrap().count(), 1);
        // A later update starts from what is running then, and the old copy goes.
        let next = make_waiter_copy(&updates, &exe, "1.0.2").unwrap();
        assert_eq!(std::fs::read_to_string(next).unwrap(), "the new version");
        assert!(!copy.exists());
    }

    /// Real folders, the real exchange, a real relaunch.
    #[cfg(unix)]
    #[test]
    fn the_real_swap_replaces_a_real_folder_and_runs_the_new_app() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("apps");
        let paths = crate::stage::bundle_paths(&apps.join("SnoutCode.app"));
        let ran = dir.path().join("ran");
        for (bundle, name) in [(&paths.install, "old"), (&paths.next, "new")] {
            let exe = crate::stage::bundle_exe(bundle);
            std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
            std::fs::write(&exe, format!("#!/bin/sh\necho {name} > '{}'\n", ran.display())).unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // An app that is already gone.
        let mut gone = std::process::Command::new("true").spawn().unwrap();
        let pid = gone.id();
        gone.wait().unwrap();
        let cfg = WaiterConfig {
            app_pid: pid,
            mode: Mode::Restart,
            version: "1.0.1".into(),
            plan: Plan::Bundle { install: paths.install.clone(), next: paths.next.clone(), old: paths.old.clone() },
            updates: dir.path().join("updates"),
            log: dir.path().join("update.log"),
            direct_launch: true,
        };
        let deps = OsDeps { log: cfg.log.clone(), own_pid: std::process::id() };
        // On Linux this is a program path, so the test's exe path is what launches.
        if cfg!(target_os = "macos") {
            assert_eq!(run_waiter(&cfg, &deps), Outcome::Swapped);
            for _ in 0..50 {
                if ran.exists() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            assert_eq!(std::fs::read_to_string(&ran).unwrap().trim(), "new");
            assert!(!paths.next.exists(), "the old app is gone");
            assert!(!cfg.updates.join(LOCK).exists(), "the lock is lifted");
            let log = std::fs::read_to_string(&cfg.log).unwrap();
            assert!(log.contains("swapped in v1.0.1"), "{log}");
        }
    }
}
