//! Updates, as the app shows them: a quiet check after launch and every few hours, a download in
//! the background (the status bar's background tasks show it), then a toast with Release Notes,
//! Later and Restart. Later leaves a Restart to update item in the status bar, and a plain quit
//! installs too. `snout-update` does the work; see `docs/updates.md`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::StreamExt as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, Global, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use snout_update::failure::Kind;
use snout_update::flow::{self, Ready};
use snout_update::route::Route;
use snout_update::stage::CodeCheck;
use snout_update::status::{ManualReason, UpdateStatus};
use snout_update::swap::{Mode, Plan, WaiterConfig};

use crate::components::{Button, IconName};
use crate::menus::{CheckForUpdates, ReleaseNotes, RestartToUpdate};
use crate::model::Model;
use crate::theme::palette;

/// Set when a release is built (the packaging scripts): only such a build reads the feed. A build
/// from the source is updated by building the source again.
const OFFICIAL_BUILD: bool = match option_env!("SNOUT_OFFICIAL_BUILD") {
    Some(v) => !v.is_empty(),
    None => false,
};

/// A feed to use instead of the public one: signed all the same. For trying an update out.
pub const FEED_ENV: &str = "SNOUT_UPDATE_FEED";
/// Set, the app looks at no feed by itself.
pub const NO_UPDATE_ENV: &str = "SNOUT_NO_UPDATE";

pub const DOWNLOAD_URL: &str = "https://snoutcode.com/download";

/// How long after launch the leftovers of the last update are tidied: startup is sacred.
const CLEANUP_AFTER: std::time::Duration = std::time::Duration::from_secs(20);

/// The version this program is. A development build can be told otherwise, by a file in its
/// bundle, to try an update between two copies of one build.
pub fn current_version() -> String {
    #[cfg(debug_assertions)]
    if let Some(v) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(2).map(|c| c.join("Resources/fake-version")))
        .and_then(|f| std::fs::read_to_string(f).ok())
    {
        return v.trim().to_owned();
    }
    env!("CARGO_PKG_VERSION").to_owned()
}

/// The modes this program is started in by an update, instead of the app. True when it ran one.
pub fn run_helper_mode() -> bool {
    use snout_update::stage::SELF_CHECK_ENV;
    use snout_update::swap::{WAITER_ENV, wait_out_swap, waiter_main};
    if let Some(answer) = std::env::var_os(SELF_CHECK_ENV) {
        let _ = std::fs::write(answer, current_version());
        return true;
    }
    if let Some(config) = std::env::var_os(WAITER_ENV) {
        waiter_main(std::path::Path::new(&config));
        return true;
    }
    std::env::current_exe().is_ok_and(|exe| wait_out_swap(&snout_update::updates_dir(), &exe))
}

fn test_feed() -> Option<String> {
    std::env::var(FEED_ENV).ok().filter(|f| !f.is_empty())
}

fn official() -> bool {
    OFFICIAL_BUILD || (cfg!(debug_assertions) && test_feed().is_some())
}

/// The route of this program on this machine (it looks at the disk).
fn this_route() -> Route {
    let exe = std::env::current_exe().unwrap_or_default();
    snout_update::route::route(official(), std::env::consts::OS, &exe, snout_update::route::can_write)
}

fn config(route: Route) -> flow::Config {
    flow::Config {
        feed: test_feed().unwrap_or_else(|| flow::FEED.to_owned()),
        public_key: snout_update::manifest::PUBLIC_KEY.to_owned(),
        current: current_version(),
        platform: snout_update::manifest::platform_key(),
        route,
        updates: snout_update::updates_dir(),
        log: snout_update::log_path(),
        // A development build is signed ad hoc, which no other build can match.
        code_check: if cfg!(debug_assertions) { CodeCheck::IntactOnly } else { CodeCheck::Requirement },
    }
}

/// What the check on its thread says.
enum Msg {
    Status(UpdateStatus),
    Done { route: Route, ready: Option<Ready> },
}

pub struct Updater {
    pub status: Option<UpdateStatus>,
    /// The user asked: every result is shown. A check of its own only shows what needs acting on.
    pub manual: bool,
    /// Later: a ready update waits in the status bar.
    pub dismissed: bool,
    /// The version announced and dismissed, so a later check does not announce it again.
    announced: Option<String>,
    /// This launch is the first on a new version, and that is still being said.
    pub updated_to: Option<String>,
    /// What a restart or a quit puts in place.
    pub ready: Option<Ready>,
    route: Option<Route>,
    running: bool,
    /// The app is quitting to restart on the new version.
    restart: bool,
    /// The app is quitting because it was told to (a signal, the session ending): an install now
    /// could be cut off.
    skip_install: bool,
    cancel: Arc<AtomicBool>,
}

struct UpdaterGlobal(Entity<Updater>);

impl Global for UpdaterGlobal {}

/// The app's updater.
pub fn updater(cx: &mut App) -> Entity<Updater> {
    if let Some(u) = cx.try_global::<UpdaterGlobal>() {
        return u.0.clone();
    }
    let u = cx.new(|_| Updater::new());
    cx.set_global(UpdaterGlobal(u.clone()));
    u
}

fn read_updater(cx: &App) -> Option<&Updater> {
    cx.try_global::<UpdaterGlobal>().map(|u| u.0.read(cx))
}

impl Updater {
    fn new() -> Self {
        Self {
            status: None,
            manual: false,
            dismissed: false,
            announced: None,
            updated_to: None,
            ready: None,
            route: None,
            running: false,
            restart: false,
            skip_install: false,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Look at the feed: `manual` when the user asked.
    pub fn check(&mut self, manual: bool, cx: &mut Context<Self>) {
        if manual {
            self.manual = true;
            self.dismissed = false;
            self.announced = None;
        } else if !self.running {
            self.manual = false;
        }
        // Only one version is made ready at a time: the check after the restart offers a newer one.
        if let Some(ready) = &self.ready {
            if manual {
                self.status =
                    Some(UpdateStatus::Downloaded { version: ready.version.clone(), notes: ready.notes.clone() });
            }
            cx.notify();
            return;
        }
        // One check at a time: what runs goes on, and now reports to whoever asked.
        if self.running {
            cx.notify();
            return;
        }
        self.running = true;
        if manual {
            self.status = Some(UpdateStatus::Checking);
        }
        let cancel = self.cancel.clone();
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        std::thread::spawn(move || {
            let route = this_route();
            let cfg = config(route.clone());
            let ready = flow::run(&cfg, &cancel, &mut |s| {
                let _ = tx.unbounded_send(Msg::Status(s));
            });
            let _ = tx.unbounded_send(Msg::Done { route, ready });
        });
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await {
                if this.update(cx, |this, cx| this.receive(msg, cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn receive(&mut self, msg: Msg, cx: &mut Context<Self>) {
        match msg {
            Msg::Status(status) => self.set_status(status),
            Msg::Done { route, ready } => {
                self.running = false;
                self.route = Some(route);
                self.ready = ready;
                #[cfg(debug_assertions)]
                self.test_act(cx);
            }
        }
        cx.notify();
    }

    /// A development build trying a whole update, with nobody to click: once one is ready,
    /// `SNOUT_UPDATE_TEST_ACT` restarts (`restart`) or quits (`quit`).
    #[cfg(debug_assertions)]
    fn test_act(&self, cx: &mut Context<Self>) {
        let Some(act) = std::env::var("SNOUT_UPDATE_TEST_ACT").ok().filter(|_| self.ready.is_some()) else { return };
        cx.defer(move |cx| {
            let ws = cx.try_global::<crate::workspace::quit::CurrentWorkspace>().and_then(|w| w.0.upgrade());
            if let Some(ws) = ws {
                ws.update(cx, |ws, cx| if act == "restart" { ws.restart_to_update(cx) } else { ws.quit_now(cx) });
            }
        });
    }

    /// What the check found. A newly ready update surfaces its toast again after an earlier Later.
    pub fn set_status(&mut self, status: UpdateStatus) {
        if matches!(status, UpdateStatus::Downloaded { .. }) {
            self.dismissed = false;
        }
        self.status = Some(status);
    }

    /// The toast's X, or Later.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        match &self.status {
            // A ready update is not dismissable, only put off.
            Some(UpdateStatus::Downloaded { .. }) => self.dismissed = true,
            Some(UpdateStatus::Manual { version, .. }) => {
                self.announced = Some(version.clone());
                self.status = None;
                self.manual = false;
            }
            _ => {
                self.status = None;
                self.manual = false;
            }
        }
        self.updated_to = None;
        cx.notify();
    }

    /// What the toast shows, if anything: what needs acting on, and every result of a check the
    /// user asked for, but never the download, whose progress is in the status bar.
    pub fn toast_status(&self) -> Option<&UpdateStatus> {
        let status = self.status.as_ref()?;
        match status {
            UpdateStatus::Downloaded { .. } => (!self.dismissed).then_some(status),
            UpdateStatus::Manual { version, .. } => (self.announced.as_ref() != Some(version)).then_some(status),
            UpdateStatus::Downloading { .. } => None,
            _ => self.manual.then_some(status),
        }
    }

    /// The Restart to update item shows.
    pub fn restart_waiting(&self) -> bool {
        self.dismissed && matches!(self.status, Some(UpdateStatus::Downloaded { .. }))
    }

    /// What a quit starts, in the words of the swap. None when nothing is to be installed.
    fn waiter(&self) -> Option<Waiter> {
        if self.skip_install {
            return None;
        }
        let ready = self.ready.as_ref()?;
        let plan = match self.route.as_ref()? {
            Route::MacBundle { app } => {
                let p = snout_update::stage::bundle_paths(app);
                Plan::Bundle { install: p.install, next: p.next, old: p.old }
            }
            Route::WindowsInstaller { folder, exe } => {
                Plan::Installer { installer: ready.path.clone(), folder: folder.clone(), exe: exe.clone() }
            }
            _ => return None,
        };
        Some(Waiter {
            config: WaiterConfig {
                app_pid: std::process::id(),
                mode: if self.restart { Mode::Restart } else { Mode::Quit },
                version: ready.version.clone(),
                plan,
                updates: snout_update::updates_dir(),
                log: snout_update::log_path(),
                direct_launch: test_feed().is_some(),
            },
        })
    }
}

/// The process that puts the new version in place once the app is gone.
pub struct Waiter {
    config: WaiterConfig,
}

impl Waiter {
    /// Start it. Runs off the UI thread as the app quits, after the processes to stop have been
    /// listed, so it is not one of them. A failure leaves the install as it is, and a later launch
    /// offers the update again.
    pub fn start(self) {
        let log = |m: &str| snout_update::log_to(&self.config.log, m);
        // An all-users install asks for an administrator: never after someone has simply quit.
        if let (Mode::Quit, Plan::Installer { folder, .. }) = (self.config.mode, &self.config.plan)
            && !snout_update::route::can_write(folder)
        {
            log("an all-users install waits for Restart");
            return;
        }
        snout_update::record::note_attempt(&self.config.updates, &self.config.version);
        let program = match waiter_program(&self.config) {
            Ok(p) => p,
            Err(e) => return log(&format!("the waiter could not be set up: {e}")),
        };
        log(&format!("{:?}: the waiter puts v{} in place", self.config.mode, self.config.version));
        if let Err(e) = snout_update::swap::start_waiter(&program, &self.config) {
            log(&format!("the waiter did not start: {e}"));
        }
    }
}

/// The program the waiter runs as. On a Mac this one: a running program's folder can be moved. On
/// Windows the copy made when the update became ready, since the installer replaces this one.
fn waiter_program(cfg: &WaiterConfig) -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    if !matches!(cfg.plan, Plan::Installer { .. }) {
        return Ok(exe);
    }
    let copy = snout_update::swap::waiter_path(&cfg.updates);
    if copy.exists() { Ok(copy) } else { snout_update::swap::make_waiter_copy(&cfg.updates, &exe) }
}

/// What to start as the app quits. The download in flight stops here too.
pub fn waiter_at_quit(cx: &App) -> Option<Waiter> {
    let u = read_updater(cx)?;
    u.cancel.store(true, Ordering::Relaxed);
    u.waiter()
}

/// The quit about to happen comes back on the new version, or does not.
pub fn set_restart(restart: bool, cx: &mut App) {
    updater(cx).update(cx, |u, _| u.restart = restart);
}

/// The quit about to happen comes from outside (a signal, the session ending): it installs nothing.
pub fn skip_install(cx: &mut App) {
    updater(cx).update(cx, |u, _| u.skip_install = true);
}

/// A background task for the status bar, while an update is on its way.
pub fn task(cx: &App) -> Option<crate::background::BgTask> {
    let (stage, percent) = read_updater(cx)?.status.as_ref()?.task()?;
    Some(crate::background::BgTask { title: "SnoutCode update".into(), stage, percent, ..Default::default() })
}

/// The page of a version's release notes.
pub fn notes_url(version: &str) -> String {
    format!("{}/releases/tag/v{version}", crate::menus::SOURCE_URL)
}

/// The app's actions, from anywhere.
pub fn app_actions(cx: &mut App) {
    cx.on_action(|_: &CheckForUpdates, cx| updater(cx).update(cx, |u, cx| u.check(true, cx)));
    cx.on_action(|_: &ReleaseNotes, cx| cx.open_url(&notes_url(&current_version())));
    cx.on_action(|_: &RestartToUpdate, cx| {
        let ws = cx.try_global::<crate::workspace::quit::CurrentWorkspace>().and_then(|w| w.0.upgrade());
        if let Some(ws) = ws {
            ws.update(cx, |ws, cx| ws.restart_to_update(cx));
        }
    });
}

/// The work of a real app's first frame: say so if this launch is on a new version, tidy up after
/// the last update, and start looking at the feed.
pub fn start(model: &Entity<Model>, cx: &mut App) {
    // Only the real app: the tests' windows have no business with the user's update folder.
    if !cx.has_global::<crate::workspace::quit::RealQuit>() {
        return;
    }
    let updater = updater(cx);
    note_version(model, &updater, cx);
    if std::env::var_os(NO_UPDATE_ENV).is_some() {
        return;
    }
    let route_app = |route: Route| match route {
        Route::MacBundle { app } => Some(app),
        _ => None,
    };
    cx.background_executor()
        .spawn({
            let timer = cx.background_executor().timer(CLEANUP_AFTER);
            async move {
                timer.await;
                let dir = snout_update::updates_dir();
                if let Some(r) = snout_update::record::settle(&dir, &current_version()) {
                    let message = format!(
                        "v{} was swapped in {} time(s), but this is still v{}",
                        r.version,
                        r.attempts,
                        current_version()
                    );
                    snout_update::log_to(&snout_update::log_path(), &message);
                }
                snout_update::stage::cleanup(&dir, route_app(this_route()).as_deref(), &current_version());
            }
        })
        .detach();
    let model = model.clone();
    cx.spawn(async move |cx| {
        let mut wait = snout_update::FIRST_CHECK_AFTER;
        loop {
            cx.background_executor().timer(wait).await;
            wait = snout_update::RECHECK_EVERY;
            cx.update(|cx| {
                if official() && model.read(cx).settings.update_check {
                    updater.update(cx, |u, cx| u.check(false, cx));
                }
            });
        }
    })
    .detach();
}

/// What a launch learns from the version it last ran as: a fresh install records it silently, and
/// only a change of version is announced.
fn note_version(model: &Entity<Model>, updater: &Entity<Updater>, cx: &mut App) {
    let current = current_version();
    let last = model.read(cx).settings.last_version.clone();
    if last.as_deref() == Some(current.as_str()) {
        return;
    }
    if last.is_some_and(|l| snout_update::version::is_newer(&current, &l)) {
        updater.update(cx, |u, cx| {
            u.updated_to = Some(current.clone());
            cx.notify();
        });
    }
    model.update(cx, |m, _| {
        m.settings.last_version = Some(current);
        m.persist();
    });
}

/// The toast: one card at the window's bottom right, above the status bar.
pub struct UpdateToast {
    updater: Entity<Updater>,
    _watch: Subscription,
}

impl UpdateToast {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let updater = updater(cx);
        let watch = cx.observe(&updater, |_, _, cx| cx.notify());
        Self { updater, _watch: watch }
    }
}

fn icon_of(status: &UpdateStatus) -> IconName {
    match status {
        UpdateStatus::Checking => IconName::RefreshCw,
        UpdateStatus::Available { .. } | UpdateStatus::Downloading { .. } => IconName::CloudDownload,
        UpdateStatus::Downloaded { .. } => IconName::Sparkles,
        UpdateStatus::NotAvailable { from_source: true, .. } => IconName::Info,
        UpdateStatus::NotAvailable { .. } => IconName::CircleCheck,
        UpdateStatus::Manual { .. } => IconName::Download,
        UpdateStatus::Error(f) => match f.kind {
            Kind::Offline => IconName::WifiOff,
            Kind::FeedUnavailable | Kind::RateLimited => IconName::Cloud,
            Kind::Signature => IconName::ShieldAlert,
            _ => IconName::TriangleAlert,
        },
    }
}

impl Render for UpdateToast {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let u = self.updater.read(cx);
        let status = u.toast_status().cloned();
        let updated_to = u.updated_to.clone();
        let (icon, text, tip): (IconName, String, Option<SharedString>) = match (&status, &updated_to) {
            (Some(s), _) => {
                let tip = match s {
                    UpdateStatus::Error(f) => f.detail.clone().map(SharedString::from),
                    _ => None,
                };
                (icon_of(s), s.text(), tip)
            }
            (None, Some(version)) => (IconName::CircleCheck, format!("Updated to v{version}"), None),
            (None, None) => return div().into_any_element(),
        };
        let updater = self.updater.clone();
        let dismiss = {
            let updater = updater.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| updater.update(cx, |u, cx| u.dismiss(cx))
        };
        let notes = |version: String| {
            Button::new("update-notes", IconName::BookOpen)
                .compact()
                .ghost()
                .label("Release Notes")
                .debug_selector("update-notes")
                .on_click(move |_, _, cx| cx.open_url(&notes_url(&version)))
        };
        let close = Button::new("update-close", IconName::X)
            .compact()
            .ghost()
            .tooltip("Dismiss")
            .debug_selector("update-close")
            .on_click(dismiss.clone());
        let actions = match &status {
            Some(UpdateStatus::Downloaded { version, .. }) => div()
                .child(notes(version.clone()))
                .child(
                    Button::new("update-later", IconName::X)
                        .compact()
                        .ghost()
                        .label("Later")
                        .debug_selector("update-later")
                        .on_click(dismiss),
                )
                .child(
                    Button::new("update-restart", IconName::RotateCw)
                        .compact()
                        .primary()
                        .label("Restart")
                        .debug_selector("update-restart")
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(RestartToUpdate), cx)),
                ),
            Some(UpdateStatus::Manual { version, reason }) => {
                let reason = *reason;
                div()
                    .when(reason != ManualReason::DidNotLand, |d| d.child(notes(version.clone())))
                    .child(
                        Button::new("update-download", IconName::Download)
                            .compact()
                            .primary()
                            .label("Download")
                            .debug_selector("update-download")
                            .on_click(|_, _, cx| cx.open_url(DOWNLOAD_URL)),
                    )
                    .child(close)
            }
            None => div().when_some(updated_to, |d, v| d.child(notes(v))).child(close),
            Some(_) => div().child(close),
        };
        div()
            .id("update-toast")
            .debug_selector(|| "update-toast".into())
            .absolute()
            .right(px(12.))
            .bottom(px(34.))
            .max_w(px(620.))
            .px(px(12.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .occlude()
            .text_size(px(12.5))
            .text_color(p.fg)
            .child(crate::components::icon(icon, 15., p.accent))
            .child(
                div()
                    .id("update-toast-text")
                    .debug_selector(|| "update-toast-text".into())
                    .flex_1()
                    .min_w_0()
                    .when_some(tip, |d, tip| {
                        d.tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    })
                    .child(text),
            )
            .child(actions.flex_none().flex().items_center().gap(px(4.)))
            .into_any_element()
    }
}

/// The status bar's Restart to update, once Later put a ready update off.
pub fn status_item(cx: &App) -> Option<gpui::AnyElement> {
    if !read_updater(cx)?.restart_waiting() {
        return None;
    }
    let p = palette(cx);
    let hover = p.button_hover;
    Some(
        div()
            .id("status-update")
            .debug_selector(|| "status-update".into())
            .flex_none()
            .h(px(20.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(4.))
            .whitespace_nowrap()
            .cursor_pointer()
            .text_color(p.accent)
            .hover(move |s| s.bg(hover))
            .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Restart to install the update").build(w, cx))
            .child(crate::components::icon(IconName::RotateCw, 12., p.accent))
            .child("Restart to update")
            .on_click(|_, window, cx| window.dispatch_action(Box::new(RestartToUpdate), cx))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext};
    use snout_update::failure::describe;

    use super::*;
    use crate::quit_tests::{click, open_stored};
    use crate::settings::{Settings, Store};

    fn ready() -> UpdateStatus {
        UpdateStatus::Downloaded { version: "1.0.1".into(), notes: None }
    }

    fn show(cx: &mut VisualTestContext, manual: bool, status: UpdateStatus) {
        cx.update(|_, cx| {
            updater(cx).update(cx, |u, cx| {
                u.manual = manual;
                u.set_status(status);
                cx.notify();
            })
        });
        cx.run_until_parked();
    }

    fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
        cx.debug_bounds(selector).is_some()
    }

    fn open<'a>(name: &str, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
        let file = crate::tests::scratch_dir(name).join("settings.json");
        let (_, _, vcx) = open_stored(cx, Settings::default(), Store::at(file));
        vcx
    }

    #[gpui::test]
    fn a_ready_update_shows_restart_and_later_moves_it_to_the_status_bar(cx: &mut TestAppContext) {
        let cx = open("update-ready", cx);
        assert!(!drawn(cx, "update-toast"), "nothing to say yet");
        show(cx, false, ready());
        assert!(drawn(cx, "update-restart") && drawn(cx, "update-later") && drawn(cx, "update-notes"));
        assert!(!drawn(cx, "status-update"), "the toast says it for now");
        click(cx, "update-later");
        cx.run_until_parked();
        assert!(!drawn(cx, "update-toast"), "Later put it off");
        assert!(drawn(cx, "status-update"), "and it waits in the status bar");
    }

    #[gpui::test]
    fn a_check_of_its_own_only_shows_what_needs_acting_on(cx: &mut TestAppContext) {
        let cx = open("update-quiet", cx);
        for quiet in [
            UpdateStatus::Checking,
            UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: false },
            UpdateStatus::Available { version: "1.0.1".into(), notes: None },
            UpdateStatus::Downloading { percent: Some(10), preparing: false },
            UpdateStatus::Error(describe("curl: (6) Could not resolve host", None)),
        ] {
            show(cx, false, quiet.clone());
            assert!(!drawn(cx, "update-toast"), "{quiet:?} is silent");
        }
    }

    #[gpui::test]
    fn a_check_the_user_asked_for_answers_and_goes_when_dismissed(cx: &mut TestAppContext) {
        let cx = open("update-manual", cx);
        for answer in [
            UpdateStatus::Checking,
            UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: false },
            UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: true },
            UpdateStatus::Error(describe("HTTP 504", Some(504))),
        ] {
            show(cx, true, answer.clone());
            assert!(drawn(cx, "update-toast"), "{answer:?} is shown");
            click(cx, "update-close");
            cx.run_until_parked();
            assert!(!drawn(cx, "update-toast"), "{answer:?} is dismissed");
        }
        // The download's progress is the status bar's, even when the user asked.
        show(cx, true, UpdateStatus::Downloading { percent: Some(40), preparing: false });
        assert!(!drawn(cx, "update-toast"));
        assert!(drawn(cx, "status-tasks"), "it shows among the background tasks");
    }

    #[gpui::test]
    fn the_status_bar_shows_how_far_the_download_is(cx: &mut TestAppContext) {
        let cx = open("update-task", cx);
        show(cx, false, UpdateStatus::Downloading { percent: Some(40), preparing: false });
        let got = cx.update(|_, cx| task(cx)).unwrap();
        assert_eq!((got.stage.as_str(), got.percent), ("Downloading update", Some(40)));
        show(cx, false, UpdateStatus::Downloading { percent: None, preparing: true });
        assert_eq!(cx.update(|_, cx| task_stage(cx)).as_deref(), Some("Preparing update"));
        show(cx, false, ready());
        assert!(cx.update(|_, cx| task(cx)).is_none(), "ready is not work");
    }

    fn task_stage(cx: &App) -> Option<String> {
        task(cx).map(|t| t.stage)
    }

    #[gpui::test]
    fn a_version_only_announced_offers_the_download_once(cx: &mut TestAppContext) {
        let cx = open("update-announce", cx);
        let linux = UpdateStatus::Manual { version: "1.0.1".into(), reason: ManualReason::PackageManager };
        show(cx, false, linux.clone());
        assert!(drawn(cx, "update-download"), "it needs acting on, so a quiet check shows it");
        click(cx, "update-close");
        cx.run_until_parked();
        assert!(!drawn(cx, "update-toast"));
        show(cx, false, linux);
        assert!(!drawn(cx, "update-toast"), "the next quiet check does not say it again");
        show(cx, false, UpdateStatus::Manual { version: "1.0.2".into(), reason: ManualReason::PackageManager });
        assert!(drawn(cx, "update-download"), "a newer one is news");
    }

    #[gpui::test]
    fn a_new_version_is_said_once_and_a_first_install_says_nothing(cx: &mut TestAppContext) {
        let file = crate::tests::scratch_dir("update-version").join("settings.json");
        let settings = Settings { last_version: Some("0.9.0".into()), ..Default::default() };
        let (model, _, cx) = open_stored(cx, settings, Store::at(file));
        let u = cx.update(|_, cx| updater(cx));
        cx.update(|_, cx| note_version(&model, &u, cx));
        cx.run_until_parked();
        assert!(drawn(cx, "update-notes"), "Updated to, with its notes");
        assert_eq!(cx.update(|_, cx| model.read(cx).settings.last_version.clone()), Some(current_version()));
        click(cx, "update-close");
        cx.run_until_parked();
        assert!(!drawn(cx, "update-toast"));
        // The same version again; then a fresh install.
        cx.update(|_, cx| note_version(&model, &u, cx));
        cx.run_until_parked();
        assert!(!drawn(cx, "update-toast"));
        cx.update(|_, cx| model.update(cx, |m, _| m.settings.last_version = None));
        cx.update(|_, cx| note_version(&model, &u, cx));
        cx.run_until_parked();
        assert!(!drawn(cx, "update-toast"), "a fresh install records it silently");
        assert_eq!(cx.update(|_, cx| model.read(cx).settings.last_version.clone()), Some(current_version()));
    }

    #[gpui::test]
    fn restart_asks_first_when_work_would_be_lost_and_a_later_plain_quit_does_not_restart(cx: &mut TestAppContext) {
        let file = crate::tests::scratch_dir("update-restart").join("settings.json");
        let repo = crate::tests::demo_repo("update-restart-repo");
        let (model, ws, cx) = open_stored(cx, crate::tests::with_repo(&repo), Store::at(file));
        std::fs::write(repo.join("U.java"), "class U {}\n").unwrap();
        crate::tests::act(&model, cx, crate::model::Action::OpenFile(repo.join("U.java"), Some((0, 0))));
        let editor = crate::code_tests::editor_for(&ws, &repo.join("U.java"), cx);
        cx.update(|w, cx| editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.insert("x", w, cx))));
        cx.run_until_parked();
        show(cx, false, ready());
        click(cx, "update-restart");
        cx.run_until_parked();
        let asked = cx.update(|_, cx| matches!(model.read(cx).dialog, Some(crate::model::Dialog::Quit(_))));
        assert!(asked, "unsaved work goes through the Quit dialog");
        assert!(!cx.update(|_, cx| ws.read(cx).hidden), "and nothing is lost meanwhile");
        let restart = |cx: &mut VisualTestContext| cx.update(|_, cx| updater(cx).read(cx).restart);
        assert!(restart(cx));
        // That dialog is cancelled, and later the user simply quits.
        cx.update(|window, cx| ws.update(cx, |ws, cx| ws.request_quit(window, cx)));
        assert!(!restart(cx), "the plain quit does not come back");
    }

    #[gpui::test]
    fn restart_with_nothing_to_lose_quits_without_asking(cx: &mut TestAppContext) {
        let file = crate::tests::scratch_dir("update-restart-clean").join("settings.json");
        let (model, ws, cx) = open_stored(cx, Settings::default(), Store::at(file));
        show(cx, false, ready());
        click(cx, "update-restart");
        cx.run_until_parked();
        assert!(cx.update(|_, cx| model.read(cx).dialog.is_none()), "Confirm before quitting is not for this");
        assert!(cx.update(|_, cx| ws.read(cx).hidden), "the window went");
    }

    #[test]
    fn only_a_ready_update_on_a_route_that_installs_starts_a_waiter() {
        let mut u = Updater::new();
        assert!(u.waiter().is_none());
        u.ready = Some(Ready { version: "1.0.1".into(), notes: None, path: "/u/x".into() });
        u.route = Some(Route::Manual(ManualReason::PackageManager));
        assert!(u.waiter().is_none(), "the package manager owns it");
        u.route = Some(Route::MacBundle { app: "/Applications/SnoutCode.app".into() });
        let w = u.waiter().unwrap().config;
        assert_eq!(w.mode, Mode::Quit, "a plain quit installs");
        assert_eq!((w.version.as_str(), w.app_pid), ("1.0.1", std::process::id()));
        u.restart = true;
        assert_eq!(u.waiter().unwrap().config.mode, Mode::Restart);
        u.skip_install = true;
        assert!(u.waiter().is_none(), "never as the session ends");
    }

    #[test]
    fn the_notes_are_on_the_release_page() {
        assert_eq!(notes_url("1.0.1"), "https://github.com/snoutdata/snout-code/releases/tag/v1.0.1");
    }
}
