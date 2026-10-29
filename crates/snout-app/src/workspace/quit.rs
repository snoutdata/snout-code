//! Quitting: ⌘Q, the window's close button and a signal all come here. The Quit dialog asks
//! first (unless Settings says not to and nothing would be lost or stopped); then the window
//! goes at once (the app hides, its Dock icon too) and the work of stopping happens after:
//! the layout is saved, the processes SnoutCode runs (the Java engine and its language server,
//! the engine's Gradle daemons, programs run, debugged and tested, terminals) are handed to a
//! small process of their own that stops them (`reap`), and the app exits. Nothing waits on
//! the window, and the app never lingers: a watchdog ends it after [`EXIT_CAP`]. A relaunch
//! starts a new app at once, as the old one is gone; the reaper is not an app.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{App, Context, Global, Window};

use super::Workspace;
use crate::model::{Dialog, ModelEvent, QuitChoice, QuitInfo};
use crate::processes::Kind;
use crate::run::Place;

/// The longest the app stays after the window went (it is gone well before).
pub const EXIT_CAP: Duration = Duration::from_secs(4);
/// How long the reaper lets a process stop on its own before it ends it (the engine waits
/// for its language server up to 10 s).
pub const REAP_GRACE: Duration = Duration::from_secs(12);

/// The environment variables of the reaper: the processes to stop (`pid:start,…`), how long
/// they get (milliseconds), and the files to delete (one per line).
pub const REAP_ENV: &str = "SNOUT_REAP";
pub const REAP_GRACE_ENV: &str = "SNOUT_REAP_GRACE_MS";
pub const REAP_FILES_ENV: &str = "SNOUT_REAP_FILES";

/// Set by the real app (not the tests): quitting hides the app and exits the process.
pub struct RealQuit;

impl Global for RealQuit {}

/// The window's workspace, for a quit that comes from outside it (a signal).
pub struct CurrentWorkspace(pub gpui::WeakEntity<Workspace>);

impl Global for CurrentWorkspace {}

/// ⌘Q where the window's workspace did not take it (the keyboard in a dialog): ask, then
/// quit, as the workspace does.
pub fn quit_requested(cx: &mut App) {
    let ws = cx.try_global::<CurrentWorkspace>().and_then(|w| w.0.upgrade());
    let window = cx.windows().into_iter().next();
    match (ws, window) {
        (Some(ws), Some(window)) => {
            let _ = window.update(cx, |_, window, cx| ws.update(cx, |ws, cx| ws.request_quit(window, cx)));
        }
        _ => cx.quit(),
    }
}

/// Quit the app as ⌘Q does once answered: from a signal, without asking.
pub fn quit_from_signal(cx: &mut App) {
    match cx.try_global::<CurrentWorkspace>().and_then(|w| w.0.upgrade()) {
        Some(ws) => ws.update(cx, |ws, cx| ws.quit_now(cx)),
        None => cx.quit(),
    }
}

impl Workspace {
    /// What quitting now would lose or stop.
    pub fn quit_info(&self, cx: &App) -> QuitInfo {
        let mut unsaved: Vec<String> = self
            .editors(cx)
            .into_iter()
            .filter(|e| e.read(cx).dirty)
            .map(|e| {
                let e = e.read(cx);
                let root = self.model.read(cx).project_root();
                root.and_then(|r| e.path.strip_prefix(&r).ok().map(|p| p.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| e.name().to_string())
            })
            .collect();
        unsaved.sort();
        unsaved.dedup();
        let mut stops = Vec::new();
        for s in self.runs.read(cx).sessions.iter().filter(|s| s.running()) {
            let what = match (s.place(), s.tests.is_some()) {
                (Place::Build, true) | (Place::Run, true) => "Tests",
                (Place::Debug, _) => "Debugging",
                (Place::Build, false) => "Gradle build",
                (Place::Run, false) => "Running",
            };
            stops.push(format!("{what} {}", s.title));
        }
        if self.runs.read(cx).scala.is_some() {
            stops.push("Compiling the Scala classes".into());
        }
        if let Some(a) = crate::claude_ide::bridge::changes_of(self, cx).read(cx).busy() {
            let name = a.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            stops.push(format!("Claude Code, editing {name}"));
        }
        if let Some((host, _)) = crate::remote::connection(cx)
            && !stops.is_empty()
        {
            stops.push(format!("The connection to {host}, and what the project runs there"));
        }
        QuitInfo { unsaved, stops }
    }

    /// ⌘Q or the window's close button: ask if need be, then quit.
    pub fn request_quit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        let info = self.quit_info(cx);
        if info.is_empty() && !self.model.read(cx).settings.confirm_quit {
            self.quit_now(cx);
            return;
        }
        self.model.update(cx, |m, cx| {
            m.dialog = Some(Dialog::Quit(info));
            cx.emit(ModelEvent::Dialog);
        });
    }

    /// The Quit dialog was answered.
    pub(super) fn on_quit_choice(&mut self, choice: QuitChoice, window: &mut Window, cx: &mut Context<Self>) {
        let dirty: Vec<_> = self.editors(cx).into_iter().filter(|e| e.read(cx).dirty).collect();
        match choice.save {
            Some(true) if !dirty.is_empty() => {
                let saves: Vec<_> = dirty.iter().map(|e| e.update(cx, |e, cx| e.save(cx))).collect();
                cx.spawn_in(window, async move |this, cx| {
                    let mut all = true;
                    for s in saves {
                        all &= s.await;
                    }
                    let _ = this.update(cx, |this, cx| {
                        if all {
                            this.quit_now(cx);
                        } else {
                            this.model.update(cx, |m, cx| {
                                m.message = "A file could not be saved, so SnoutCode did not quit".into();
                                cx.notify();
                            });
                        }
                    });
                })
                .detach();
            }
            Some(false) => {
                for e in &dirty {
                    e.update(cx, |e, cx| e.discard(cx));
                }
                self.quit_now(cx);
            }
            _ => self.quit_now(cx),
        }
    }

    /// Quit without asking: the window goes first, the stopping happens after it.
    pub fn quit_now(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.quitting, true) {
            return;
        }
        let started = Instant::now();
        let real = cx.has_global::<RealQuit>();
        // The window goes before anything else is done.
        self.hidden = true;
        if real {
            cx.hide();
            cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
            eprintln!("[quit] the window went in {} ms", started.elapsed().as_millis());
            // Whatever happens below, the app does not stay.
            std::thread::spawn(|| {
                std::thread::sleep(EXIT_CAP);
                eprintln!("[quit] still here after {} s: exiting", EXIT_CAP.as_secs());
                // SAFETY: ends the process at once; nothing is left to tidy.
                unsafe { libc::_exit(0) };
            });
        }
        // What the user arranged is kept.
        self.save_layout(cx);
        self.model.update(cx, |m, _| m.persist());
        // Claude Code's lock file goes (the server deletes it on its own thread).
        let lock_gone = self.claude.as_mut().map(|b| b.server.stop());
        // The processes the app runs, as the Processes window would list them.
        let known = self.processes.read(cx).known(cx);
        let host = crate::host::host(cx);
        let remote = crate::remote::is_remote(cx);
        let job = cx.background_executor().spawn(async move {
            let snap = crate::processes::scan::Scanner::new(host.clone()).quick_snapshot(&known.cache);
            let targets: Vec<(u32, u64)> = crate::processes::rows(&*host, &snap, &known)
                .into_iter()
                .filter(|r| r.kind != Kind::App && r.kind != Kind::Leftover)
                .filter_map(|r| r.pid.map(|p| (p, r.start)))
                .collect();
            if let Some(gone) = lock_gone {
                let _ = gone.recv_timeout(Duration::from_millis(300));
            }
            if remote || !real {
                // The remote machine's server stops them there, as briefly as it can (and
                // the tests, which have no app to hand them over from, stop them here).
                host.stop_processes(&targets, Duration::from_millis(1500));
            } else if let Err(e) = spawn_reaper(&targets, REAP_GRACE, &[]) {
                eprintln!("[quit] could not hand the processes over: {e}; stopping them here");
                host.stop_processes(&targets, Duration::from_millis(1500));
            }
            targets.len()
        });
        cx.spawn(async move |_, cx| {
            let n = job.await;
            eprintln!("[quit] handed {n} process(es) over; exiting after {} ms", started.elapsed().as_millis());
            if real {
                std::process::exit(0);
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

/// Start the reaper: this program again, without a window, stopping `targets` (pid and
/// start time) within `grace` and deleting `files`. It runs on its own, in a process group
/// of its own, so the app can exit at once.
pub fn spawn_reaper(targets: &[(u32, u64)], grace: Duration, files: &[PathBuf]) -> std::io::Result<()> {
    if targets.is_empty() && files.is_empty() {
        return Ok(());
    }
    use std::os::unix::process::CommandExt as _;
    let exe = std::env::current_exe()?;
    let list = targets.iter().map(|(p, s)| format!("{p}:{s}")).collect::<Vec<_>>().join(",");
    let files = files.iter().map(|f| f.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
    std::process::Command::new(exe)
        .env(REAP_ENV, list)
        .env(REAP_GRACE_ENV, grace.as_millis().to_string())
        .env(REAP_FILES_ENV, files)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

/// The reaper's targets, from [`REAP_ENV`]'s text.
pub fn parse_targets(text: &str) -> Vec<(u32, u64)> {
    text.split(',')
        .filter_map(|t| {
            let (p, s) = t.trim().split_once(':')?;
            Some((p.parse().ok()?, s.parse().ok()?))
        })
        .collect()
}

/// The reaper: stop what the app handed over, then end. Runs when [`REAP_ENV`] is set.
pub fn reap() {
    let targets = parse_targets(&std::env::var(REAP_ENV).unwrap_or_default());
    let grace = std::env::var(REAP_GRACE_ENV)
        .ok()
        .and_then(|g| g.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(REAP_GRACE);
    for f in std::env::var(REAP_FILES_ENV).unwrap_or_default().lines().filter(|l| !l.is_empty()) {
        let _ = std::fs::remove_file(f);
    }
    let started = Instant::now();
    let n = snout_host::local().stop_processes(&targets, grace);
    eprintln!("[reap] stopped {n} process(es) in {} ms", started.elapsed().as_millis());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reaper_stops_what_it_is_handed() {
        assert_eq!(parse_targets("12:100, 13:200,x:1,"), [(12, 100), (13, 200)]);
        let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        let host = snout_host::local();
        let start = host.processes(false).unwrap().into_iter().find(|p| p.pid == pid).unwrap().start;
        let n = host.stop_processes(&[(pid, start)], Duration::from_secs(2));
        assert_eq!(n, 1);
        let status = child.wait().unwrap();
        assert!(!status.success(), "it was stopped by a signal");
    }
}
