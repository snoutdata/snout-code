//! The user guide's screenshots: the real app on the demo projects of `SNOUT_DOCS_SHOTS`
//! (a folder holding `shop`, a Spring Boot web app; `ledger`, a multi-module build; and
//! `greeter`, a small Java app; each a Git repository), with the real engine where it has
//! something to say. Only with `SNOUT_DOCS_SHOTS` set; the PNGs go to `SNOUT_SCREENSHOT`.
//! Each part copies the projects it uses into `~/projects` first; `HOME` is a scratch folder,
//! so paths read as `~/projects/shop`.
//!
//! `scripts/docs-screenshots.sh` sets it all up and writes `docs/images`. `SNOUT_DOCS_JDK`
//! names the JDK the builds run on; `SNOUT_DOCS_ONLY` takes only the parts it names
//! (ledger, java, javatools, debug, welcome, wizard, settings, structure, git, blame, claude, palette, remote), separated
//! by commas. `remote` needs Docker and the Linux server builds; `SNOUT_DOCS_SSH_DIR` is a
//! short folder for ssh's control sockets.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AnyWindowHandle, AppContext as _, Entity, HeadlessAppContext, px, size};
use snout_app::editor::EditorPanel;
use snout_app::model::{Action, Model};
use snout_app::panels::Activity;
use snout_app::settings::{Settings, ThemeChoice};
use snout_app::workspace::{Tool, Workspace};

const W: f32 = 1440.;
const H: f32 = 900.;

fn out_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("SNOUT_SCREENSHOT").expect("SNOUT_SCREENSHOT names where the shots go"))
}

fn wanted(part: &str) -> bool {
    std::env::var("SNOUT_DOCS_ONLY").map_or(true, |only| only.split(',').any(|p| p.trim() == part))
}

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(args[0]).args(&args[1..]).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// The home folder the app sees (`HOME`): paths under it read as `~/…`.
fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap()).canonicalize().unwrap()
}

/// A fresh copy of the demo project `name` in `~/projects`, where the shots open it.
fn copy(projects: &Path, _part: &str, name: &str) -> PathBuf {
    let dir = home().join("projects");
    std::fs::create_dir_all(&dir).unwrap();
    let _ = std::fs::remove_dir_all(dir.join(name));
    assert!(Command::new("cp").arg("-R").arg(projects.join(name)).arg(&dir).status().unwrap().success());
    dir.join(name).canonicalize().unwrap()
}

/// A window of the app, headless.
pub struct Win {
    pub cx: HeadlessAppContext,
    pub handle: AnyWindowHandle,
    pub ws: Entity<Workspace>,
    pub model: Entity<Model>,
}

impl Win {
    fn new(settings: Settings, lang: bool) -> Self {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| snout_app::init(&settings, cx));
        if lang {
            cx.update(|cx| cx.set_global(snout_app::lang::hub::LangEnabled(true)));
        }
        let mut win = Self::window(cx, settings);
        win.pump(1, &mut |_| false);
        win
    }

    /// A window in `cx` (which has the app initialised).
    fn window(mut cx: HeadlessAppContext, settings: Settings) -> Self {
        let model = cx.update(|cx| cx.new(|cx| Model::new(settings, None, cx)));
        let mut ws = None;
        let handle = cx
            .open_window(size(px(W), px(H)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        cx.allow_parking();
        Self { cx, handle: handle.into(), ws: ws.unwrap(), model }
    }

    /// Let real time pass (the engine and Git work in other processes) while the app's tasks
    /// run and the window draws, until `done` or `secs` are up.
    fn pump(&mut self, secs: u64, done: &mut dyn FnMut(&mut gpui::App) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            self.cx.advance_clock(Duration::from_millis(200));
            self.cx.run_until_parked();
            let _ = self.cx.update_window(self.handle, |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            });
            if self.cx.update(|cx| done(cx)) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    fn settle(&mut self) {
        self.pump(1, &mut |_| false);
    }

    fn snap(&mut self, name: &str) {
        // The status bar's message, with the home folder as `~`.
        let home = home().to_string_lossy().into_owned();
        let model = self.model.clone();
        self.cx.update(|cx| {
            model.update(cx, |m, cx| {
                if m.message.contains(&home) {
                    m.message = m.message.replace(&home, "~");
                    cx.notify();
                }
            })
        });
        self.settle();
        let image = self.cx.capture_screenshot(self.handle).expect("offscreen rendering must be available");
        let dir = out_dir();
        std::fs::create_dir_all(&dir).unwrap();
        image.save(dir.join(format!("{name}.png"))).unwrap();
        println!("docs shot {name}: ok");
    }

    /// [`Self::snap`] without running the app's tasks first: the window drawn as it is.
    fn draw_and_snap(&mut self, name: &str) {
        let _ = self.cx.update_window(self.handle, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let image = self.cx.capture_screenshot(self.handle).expect("offscreen rendering must be available");
        image.save(out_dir().join(format!("{name}.png"))).unwrap();
        println!("docs shot {name}: ok");
    }

    fn with<R>(&mut self, f: impl FnOnce(&Entity<Workspace>, &mut gpui::Window, &mut gpui::App) -> R) -> R {
        let ws = self.ws.clone();
        self.cx.update_window(self.handle, |_, window, cx| f(&ws, window, cx)).unwrap()
    }

    fn act(&mut self, a: Action) {
        let model = self.model.clone();
        self.cx.update(|cx| model.update(cx, |m, cx| m.handle(a, cx)));
    }

    fn activity(&mut self, a: Activity) {
        self.with(|ws, window, cx| ws.update(cx, |w, cx| w.show_activity(a, window, cx)));
    }

    fn tool(&mut self, t: Tool) {
        self.with(|ws, window, cx| ws.update(cx, |w, cx| w.toggle_tool(t, window, cx)));
    }

    fn open(&mut self, path: &Path, at: Option<(u32, u32)>) -> Entity<EditorPanel> {
        self.act(Action::OpenFile(path.to_owned(), at));
        let p = path.to_owned();
        let ws = self.ws.clone();
        assert!(
            self.pump(30, &mut |cx| {
                ws.read(cx).editors(cx).iter().any(|e| e.read(cx).path == p && !e.read(cx).loading)
            }),
            "{} opens",
            path.display()
        );
        self.editor(path)
    }

    fn editor(&mut self, path: &Path) -> Entity<EditorPanel> {
        let ws = self.ws.clone();
        self.cx.update(|cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path).unwrap())
    }

    fn text(&mut self, e: &Entity<EditorPanel>) -> String {
        self.cx.update(|cx| e.read(cx).text(cx))
    }

    /// The caret at the `nth` match of `find`, plus `offset` bytes, the editor focused.
    fn caret(&mut self, e: &Entity<EditorPanel>, find: &str, offset: usize) -> usize {
        let at = self.text(e).find(find).unwrap_or_else(|| panic!("{find} is in the file")) + offset;
        let e = e.clone();
        self.with(|_, window, cx| {
            e.update(cx, |e, cx| {
                e.state.update(cx, |s, cx| {
                    s.set_selected_range(at..at, cx);
                    s.focus(window, cx);
                })
            })
        });
        self.settle();
        at
    }

    fn insert(&mut self, e: &Entity<EditorPanel>, text: &str) {
        let e = e.clone();
        self.with(|_, window, cx| e.update(cx, |e, cx| e.state.update(cx, |s, cx| s.insert(text, window, cx))));
    }

    fn set_text(&mut self, e: &Entity<EditorPanel>, text: String) {
        let e = e.clone();
        self.with(|_, window, cx| {
            e.update(cx, |e, cx| {
                e.state.update(cx, |s, cx| {
                    let end = s.text().len();
                    s.set_selected_range(0..end, cx);
                    s.replace(&text, window, cx);
                })
            })
        });
        self.settle();
    }

    fn key(&mut self, keys: &'static str) {
        for k in keys.split(' ') {
            let ks = gpui::Keystroke::parse(k).unwrap();
            self.with(|_, window, cx| window.dispatch_keystroke(ks, cx));
            self.cx.run_until_parked();
        }
    }

    /// The pointer resting on the `find` text of the editor.
    fn hover(&mut self, e: &Entity<EditorPanel>, find: &str, offset: usize) {
        let at = self.text(e).find(find).unwrap() + offset;
        let e2 = e.clone();
        let point = self.cx.update(|cx| e2.read(cx).state.read(cx).range_to_bounds(&(at..at + 1)).map(|b| b.center()));
        let point = point.expect("the line is laid out");
        let e3 = e.clone();
        for _ in 0..40 {
            self.with(|_, window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                        position: point,
                        pressed_button: None,
                        modifiers: gpui::Modifiers::none(),
                    }),
                    cx,
                );
            });
            if self.pump(1, &mut |cx| e3.read(cx).hover.card.as_ref().is_some_and(|c| c.ready())) {
                return;
            }
        }
        println!("docs: no hover on {find}");
    }

    fn runs(&mut self) -> Entity<snout_app::run::Runs> {
        let ws = self.ws.clone();
        self.cx.update(|cx| ws.read(cx).runs.clone())
    }

    fn shutdown(mut self) {
        let ws = self.ws.clone();
        let runs = self.runs();
        let ids: Vec<u64> = self.cx.update(|cx| runs.read(cx).sessions.iter().map(|s| s.id).collect());
        for id in ids {
            self.cx.update(|cx| runs.update(cx, |r, cx| r.stop(id, cx)));
        }
        let r = runs.clone();
        self.pump(20, &mut |cx| !r.read(cx).any_running());
        let hub = self.cx.update(|cx| ws.read(cx).hub.clone());
        self.cx.update(|cx| hub.update(cx, |h, _| h.shutdown_all()));
        self.pump(2, &mut |_| false);
    }
}

fn settings(theme: ThemeChoice, repos: Vec<PathBuf>, current: Option<PathBuf>) -> Settings {
    let mut s = Settings { theme, repos, current, ..Settings::default() };
    s.fetch_minutes = 0;
    if let Some(jdk) = std::env::var_os("SNOUT_DOCS_JDK").map(PathBuf::from) {
        for r in s.repos.clone() {
            s.project_jdks.insert(r, jdk.clone());
        }
    }
    s
}

pub fn run(projects: PathBuf) {
    // SAFETY: set before any app starts; the harness runs on this thread.
    unsafe { std::env::set_var("SNOUT_NO_FETCH", "1") };
    let projects = projects.canonicalize().unwrap();
    if wanted("ledger") {
        ledger(&projects);
    }
    if wanted("java") {
        shop(&projects);
    }
    if wanted("javatools") {
        java_tools(&projects);
    }
    if wanted("debug") {
        debug(&projects, ThemeChoice::Light, "light");
        debug(&projects, ThemeChoice::Dark, "dark");
    }
    if wanted("welcome") {
        welcome(&projects, ThemeChoice::Light, "welcome-light");
        welcome(&projects, ThemeChoice::Dark, "welcome-dark");
    }
    if wanted("wizard") {
        wizard();
    }
    if wanted("settings") {
        settings_pages(&projects);
    }
    if wanted("structure") {
        structure(&projects);
    }
    if wanted("git") {
        git(&projects, ThemeChoice::Light, "light");
        git(&projects, ThemeChoice::Dark, "dark");
    }
    if wanted("blame") {
        blame(ThemeChoice::Light, "light");
        blame(ThemeChoice::Dark, "dark");
    }
    if wanted("claude") {
        claude(&projects);
    }
    if wanted("palette") {
        palette(&projects);
    }
    if wanted("remote") {
        remote(&projects);
    }
}

fn welcome(projects: &Path, theme: ThemeChoice, name: &str) {
    let repos: Vec<PathBuf> = ["shop", "ledger", "greeter"].iter().map(|p| copy(projects, "welcome", p)).collect();
    let mut s = settings(theme, repos.clone(), None);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    for (p, ago) in repos.iter().zip([300, 7_200, 180_000]) {
        s.opened.insert(p.clone(), now - ago);
    }
    s.recent = Some(repos);
    let mut w = Win::new(s, false);
    w.pump(3, &mut |_| false);
    w.snap(name);
}

fn wizard() {
    for (page, name) in
        [(0u8, "projects-new-java-light"), (1, "projects-new-spring-light"), (2, "projects-new-deps-light")]
    {
        let mut w = Win::new(settings(ThemeChoice::Light, Vec::new(), None), false);
        w.with(|ws, window, cx| {
            ws.update(cx, |ws, cx| ws.open_new_project(window, cx));
            let view = ws.read(cx).new_project.clone().unwrap();
            view.update(cx, |v, cx| {
                v.name.update(cx, |i, cx| i.set_value(if page == 0 { "invoices" } else { "inventory" }, window, cx));
                v.location.update(cx, |i, cx| i.set_value("/Users/alex/projects", window, cx));
                if page > 0 {
                    use snout_app::new_project::initializr::Catalog;
                    let mut c = Catalog::offline(Some("3.5.7".into()));
                    c.offline = false;
                    c.boot = vec![
                        ("4.0.0-RC1".into(), "4.0.0 (RC1)".into()),
                        ("3.5.7".into(), "3.5.7".into()),
                        ("3.4.11".into(), "3.4.11".into()),
                    ];
                    v.catalog = Some(c);
                    v.kind = snout_app::new_project::Kind::SpringBoot;
                    v.picking = page == 2;
                    v.deps = vec!["web".into(), "data-jpa".into(), "validation".into()];
                }
                cx.notify();
            });
        });
        w.pump(2, &mut |_| false);
        w.snap(name);
    }
}

fn settings_pages(projects: &Path) {
    use snout_app::settings_view::Page;
    // The Terminal page is long: its look and preview, then its shell and external terminal.
    for (page, theme, name, scroll) in [
        (Page::Appearance, ThemeChoice::Light, "settings-appearance-light", 0.),
        (Page::Appearance, ThemeChoice::Dark, "settings-appearance-dark", 0.),
        (Page::Keymap, ThemeChoice::Light, "settings-keymap-light", 0.),
        (Page::Editor, ThemeChoice::Light, "settings-editor-light", 0.),
        (Page::Terminal, ThemeChoice::Light, "settings-terminal-light", 235.),
        (Page::Terminal, ThemeChoice::Light, "settings-terminal-shell-light", 2000.),
        (Page::ClaudeCode, ThemeChoice::Light, "settings-agents-light", 0.),
    ] {
        let shop = copy(projects, "settings", "shop");
        let mut w = Win::new(settings(theme, vec![shop.clone()], Some(shop)), false);
        w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.open_settings(window, cx)));
        w.pump(3, &mut |_| false);
        w.with(|ws, window, cx| {
            let sheet = ws.read(cx).settings_sheet.clone().unwrap();
            sheet.update(cx, |s, cx| {
                s.page = page;
                if page == Page::Keymap {
                    // Go to File given Settings' keys too: the page says who has them.
                    s.draft.keymap.insert("snout::GoToFile".into(), vec!["cmd-p".into(), "cmd-,".into()]);
                    s.taken = Some(("snout::GoToFile".into(), "cmd-,".into(), vec!["snout::OpenSettings".into()]));
                    s.key_search.update(cx, |i, cx| i.set_value("go to", window, cx));
                }
                cx.notify();
            });
        });
        w.pump(2, &mut |_| false);
        if scroll > 0. {
            // The wheel over the page.
            w.with(|_, window, cx| {
                let at = gpui::point(gpui::px(900.), gpui::px(500.));
                let modifiers = gpui::Modifiers::default();
                window.dispatch_event(
                    gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                        position: at,
                        pressed_button: None,
                        modifiers,
                    }),
                    cx,
                );
                window.dispatch_event(
                    gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                        position: at,
                        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), gpui::px(-scroll))),
                        modifiers,
                        touch_phase: gpui::TouchPhase::Moved,
                        source: Default::default(),
                    }),
                    cx,
                );
            });
            w.pump(2, &mut |_| false);
        }
        w.snap(name);
    }
    // The project's own settings: the Editor page in the Project scope, then in the User's.
    for (scope, name) in [
        (snout_app::settings_view::Scope::Project, "settings-project-light"),
        (snout_app::settings_view::Scope::User, "settings-overridden-light"),
    ] {
        let shop = copy(projects, "settings", "shop");
        std::fs::create_dir_all(shop.join(".snout")).unwrap();
        std::fs::write(
            shop.join(snout_app::project_settings::FILE),
            "{\n  \"tab_size\": 2,\n  \"soft_wrap\": true,\n  \"spring_profiles\": [\"dev\"]\n}\n",
        )
        .unwrap();
        let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop)), false);
        w.pump(2, &mut |_| false);
        w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.open_settings(window, cx)));
        w.pump(3, &mut |_| false);
        w.with(|ws, window, cx| {
            let sheet = ws.read(cx).settings_sheet.clone().unwrap();
            sheet.update(cx, |s, cx| {
                s.page = Page::Editor;
                s.set_scope(scope, window, cx);
            });
        });
        w.pump(2, &mut |_| false);
        w.snap(name);
    }
    // The project's settings as JSON: its problems, then completion of a key.
    let shop = copy(projects, "settings", "shop");
    std::fs::create_dir_all(shop.join(".snout")).unwrap();
    let file = shop.join(snout_app::project_settings::FILE);
    std::fs::write(
        &file,
        "{\n  \"tab_size\": 2,\n  \"show_whitespace\": false,\n  \"param_hints\": true,\n  \"fetch_minutes\": 15,\n  \"daemon_idle_minutes\": 30,\n  \"jdk\": \"/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home\",\n  \"soft_wrap\": \"yes\",\n  \"spring_profile\": [\"dev\"]\n}\n",
    )
    .unwrap();
    let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop.clone())), false);
    let e = w.open(&file, Some((7, 16)));
    w.pump(4, &mut |_| false);
    w.hover(&e, "\"spring_profile\"", 4);
    w.pump(6, &mut |_| false);
    w.snap("settings-json-problems-light");
    std::fs::write(&file, "{\n  \"tab_size\": 2,\n  \"soft_wrap\": true\n}\n").unwrap();
    let e2 = e.clone();
    w.with(|_, window, cx| e2.update(cx, |e, cx| e.reload(window, cx)));
    w.pump(4, &mut |_| false);
    w.caret(&e, "\"soft_wrap\": true", "\"soft_wrap\": true".len());
    w.insert(&e, ",\n  \"");
    let e3 = e.clone();
    w.with(|_, window, cx| e3.update(cx, |e, cx| e.state.update(cx, |s, cx| s.show_completions(window, cx))));
    let e4 = e.clone();
    let ok = w.pump(60, &mut |cx| !e4.read(cx).state.read(cx).completion_menu_state().items.is_empty());
    println!("docs: settings json completion {ok}");
    w.pump(1, &mut |_| false);
    w.snap("settings-json-completion-light");
    drop(w);
    // Quit SnoutCode?, with a file unsaved and the application running.
    let shop = copy(projects, "settings", "shop");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop.clone())), false);
    let e = w.open(&shop.join("src/main/java/com/example/shop/service/OrderService.java"), Some((22, 0)));
    w.caret(&e, "public", 0);
    w.insert(&e, " ");
    w.with(|ws, window, cx| {
        ws.update(cx, |ws, cx| {
            let mut info = ws.quit_info(cx);
            info.stops.push("Running ShopApplication".into());
            ws.model.update(cx, |m, cx| {
                m.dialog = Some(snout_app::model::Dialog::Quit(info));
                cx.emit(snout_app::model::ModelEvent::Dialog);
            });
            let _ = window;
        })
    });
    w.pump(2, &mut |_| false);
    w.snap("quit-dialog-light");
}

fn git(projects: &Path, theme: ThemeChoice, mode: &str) {
    let shop = copy(projects, "git", "shop");
    let others: Vec<PathBuf> = ["ledger", "greeter"].iter().map(|p| copy(projects, "git", p)).collect();
    let mut repos = vec![shop.clone()];
    repos.extend(others);
    let mut w = Win::new(settings(theme, repos, Some(shop.clone())), false);
    w.activity(Activity::Git);
    w.act(Action::Refresh);
    w.pump(3, &mut |_| false);
    let model = w.model.clone();
    w.cx.update(|cx| {
        model.update(cx, |m, cx| {
            m.click_file("src/main/java/com/example/shop/service/PriceCalculator.java", false, false, cx)
        })
    });
    w.pump(3, &mut |_| false);
    w.snap(&format!("git-view-{mode}"));
    if mode == "light" {
        // A commit: the merge, and a file it brought.
        w.cx.update(|cx| {
            let id = model.read(cx).log.iter().find(|c| c.summary.starts_with("Merge")).unwrap().id.clone();
            model.update(cx, |m, cx| m.handle(Action::ShowCommit(id), cx));
        });
        w.pump(2, &mut |_| false);
        w.cx.update(|cx| {
            model.update(cx, |m, cx| {
                m.select_commit_file("src/test/java/com/example/shop/service/OrderServiceTest.java".into(), cx)
            })
        });
        w.pump(3, &mut |_| false);
        w.snap("git-commit-light");
        w.act(Action::CloseCommit);
        w.pump(1, &mut |_| false);
        // Discard asks first, with the files it would put back.
        let entries = w.cx.update(|cx| {
            model
                .read(cx)
                .status
                .files
                .iter()
                .filter(|f| f.path.ends_with("Service.java") || f.path.ends_with("PriceCalculator.java"))
                .cloned()
                .collect::<Vec<_>>()
        });
        assert!(!entries.is_empty(), "work in progress to discard");
        w.act(Action::AskDiscard(entries));
        w.pump(2, &mut |_| false);
        w.snap("git-discard-light");
        // What Git said when a push was turned back.
        w.cx.update(|cx| {
            model.update(cx, |m, cx| {
                use snout_app::model::{Dialog, ModelEvent};
                m.dialog = Some(Dialog::Error(
                    "To github.com:example/shop.git\n ! [rejected]        main -> main (fetch first)\n\
                     error: failed to push some refs to 'github.com:example/shop.git'\n\
                     hint: Updates were rejected because the remote contains\n\
                     hint: work that you do not have locally. Integrate the\n\
                     hint: remote changes (e.g. 'git pull ...') before pushing\n\
                     hint: again."
                        .into(),
                ));
                cx.emit(ModelEvent::Dialog);
            })
        });
        w.pump(2, &mut |_| false);
        w.snap("git-error-light");
        // A project with nothing to commit.
        drop(w);
        let greeter = home().join("projects/greeter");
        let mut w = Win::new(settings(theme, vec![greeter.clone()], Some(greeter)), false);
        w.activity(Activity::Git);
        w.act(Action::Refresh);
        w.pump(3, &mut |_| false);
        w.snap("git-clean-light");
    }
}

const PRICE_EDIT_FROM: &str = "        if (items >= 3) {\n            return total.multiply(new BigDecimal(\"0.10\")).setScale(2, RoundingMode.HALF_UP);";
const PRICE_EDIT_TO: &str = "        if (items >= bulkItems) {\n            return total.multiply(bulkDiscount).setScale(2, RoundingMode.HALF_UP);";

fn price_v2(v1: &str) -> String {
    v1.replace("        // Ten percent off from three items.\n", "")
        .replace(PRICE_EDIT_FROM, PRICE_EDIT_TO)
        .replace(
            "    private final BigDecimal taxRate;\n",
            "    private final BigDecimal taxRate;\n    private final int bulkItems;\n    private final BigDecimal bulkDiscount;\n",
        )
        .replace(
            "    public PriceCalculator(@Value(\"${shop.tax-rate:0.20}\") BigDecimal taxRate) {\n        this.taxRate = taxRate;\n",
            "    public PriceCalculator(\n            @Value(\"${shop.tax-rate:0.20}\") BigDecimal taxRate,\n            @Value(\"${shop.bulk.items:3}\") int bulkItems,\n            @Value(\"${shop.bulk.discount:0.10}\") BigDecimal bulkDiscount) {\n        this.taxRate = taxRate;\n        this.bulkItems = bulkItems;\n        this.bulkDiscount = bulkDiscount;\n",
        )
}

fn fake_claude(dir: &Path, body: &str) -> PathBuf {
    let fake = dir.parent().unwrap().join("fake-claude");
    std::fs::write(&fake, format!("#!/bin/sh\n{body}\nsleep 60\n")).unwrap();
    Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
    fake
}

/// The console with Claude Code (a stand-in that prints what it would), and its live edits.
fn claude(projects: &Path) {
    use snout_app::claude_ide::changes::Source;
    use snout_app::claude_ide::panel::{Activity as Doing, PendingDiff};
    #[derive(PartialEq, Clone, Copy)]
    enum Step {
        Console,
        Start,
        Typing,
        Fade,
        Card,
        Inline,
    }
    for (step, theme, name) in [
        (Step::Console, ThemeChoice::Light, "claude-console-light"),
        (Step::Start, ThemeChoice::Light, "agents-start-light"),
        (Step::Typing, ThemeChoice::Light, "agents-presence-light"),
        (Step::Typing, ThemeChoice::Dark, "agents-presence-dark"),
        (Step::Fade, ThemeChoice::Light, "claude-changes-light"),
        (Step::Fade, ThemeChoice::Dark, "claude-changes-dark"),
        (Step::Card, ThemeChoice::Light, "agents-review-light"),
        (Step::Inline, ThemeChoice::Light, "claude-inline-light"),
    ] {
        let shop = copy(projects, "claude", "shop");
        // Committed: Claude starts from a clean tree.
        sh(&shop, &["git", "add", "-A"]);
        let ok = Command::new("git")
            .args(["-c", "commit.gpgsign=false", "commit", "-qm", "Answer 404 for an unknown product"])
            .current_dir(&shop)
            .env("GIT_AUTHOR_DATE", "2026-10-19T09:40:00")
            .env("GIT_COMMITTER_DATE", "2026-10-19T09:40:00")
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let file = shop.join("src/main/java/com/example/shop/service/PriceCalculator.java");
        let v1 = std::fs::read_to_string(&file).unwrap();
        let v2 = price_v2(&v1);
        let mut w = Win::new(settings(theme, vec![shop.clone()], Some(shop.clone())), false);
        w.activity(if step == Step::Console { Activity::Git } else { Activity::Project });
        w.act(Action::Refresh);
        let e = w.open(&file, Some((0, 0)));
        let ws = w.ws.clone();
        let console = w.cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
        let script = if step == Step::Console {
            "sleep 1\n\
             printf '\\033]0;Bulk discount settings\\007'\n\
             printf '\\033[1m>\\033[0m Make the bulk discount configurable in application.yml\\n\\n'\n\
             printf '\\033[32m●\\033[0m Read(\\033[36msrc/main/java/com/example/shop/service/PriceCalculator.java\\033[0m)\\n'\n\
             printf '  ⎿  Read 41 lines\\n\\n'\n\
             printf '\\033[32m●\\033[0m Update(\\033[36msrc/main/java/com/example/shop/service/PriceCalculator.java\\033[0m)\\n'\n\
             printf '  ⎿  Updated with 9 additions and 3 removals\\n\\n'\n\
             printf '\\033[32m●\\033[0m Update(\\033[36msrc/main/resources/application.yml\\033[0m)\\n'\n\
             printf '  ⎿  Updated with 3 additions\\n\\n'\n\
             printf '\\033[32m●\\033[0m The bulk discount now reads shop.bulk.items and\\n  shop.bulk.discount, with the old values as defaults.\\n\\n'\n\
             printf '\\033[1m>\\033[0m '"
        } else {
            "sleep 1\n\
             printf '\\033]0;Bulk discount settings\\007'\n\
             printf '\\033[32m●\\033[0m Update(\\033[36msrc/main/java/com/example/shop/service/PriceCalculator.java\\033[0m)\\n'\n\
             printf '  ⎿  Updated with 9 additions and 3 removals\\n\\n'\n\
             printf '\\033[1m>\\033[0m '"
        };
        let fake = fake_claude(&shop, script);
        w.cx.update(|cx| console.update(cx, |c, _| c.program = Some(fake)));
        w.act(Action::ToggleConsole);
        w.cx.update(|cx| cx.set_global(snout_app::claude_ide::IdeStatus { clients: 1 }));
        w.pump(4, &mut |_| false);
        if step == Step::Start {
            // The session ended: the console's illustrated Start.
            let root = shop.clone();
            w.cx.update(|cx| console.update(cx, |c, cx| c.end_session(&root, cx)));
            w.pump(1, &mut |_| false);
            w.snap(name);
            continue;
        }
        if step == Step::Console {
            // What Claude changed, in the Git view beside it.
            std::fs::write(&file, &v2).unwrap();
            let yml = shop.join("src/main/resources/application.yml");
            let y = std::fs::read_to_string(&yml).unwrap();
            std::fs::write(&yml, y + "  bulk:\n    items: 3\n    discount: 0.10\n").unwrap();
            w.act(Action::Refresh);
            w.pump(2, &mut |_| false);
            let model = w.model.clone();
            w.cx.update(|cx| {
                model.update(cx, |m, cx| {
                    m.click_file("src/main/java/com/example/shop/service/PriceCalculator.java", false, false, cx)
                })
            });
            w.pump(3, &mut |_| false);
            w.snap(name);
            continue;
        }
        let changes = w.cx.update(|cx| snout_app::claude_ide::bridge::changes_of(ws.read(cx), cx));
        let yml = shop.join("src/main/resources/application.yml");
        let yml_v1 = std::fs::read_to_string(&yml).unwrap();
        let yml_v2 = yml_v1.clone() + "  bulk:\n    items: 3\n    discount: 0.10\n";
        let test = shop.join("src/test/java/com/example/shop/service/BulkDiscountTest.java");
        let (f2, s2, y2, t2) = (file.clone(), shop.clone(), yml.clone(), test.clone());
        let (v1b, v2b) = (v1.clone(), v2.clone());
        w.cx.update(|cx| {
            changes.update(cx, |c, cx| {
                c.root = Some(s2.clone());
                c.set.new_turn("claude", Some("Make the bulk discount configurable in application.yml".into()));
                c.hooks_heard = true;
                c.set.before_edit(&f2, Some(Arc::new(v1b.clone())));
                c.record(f2.clone(), Some(Arc::new(v2b.clone())), None, "Edit".into(), Source::Hook, "claude", cx);
                let edit = snout_app::claude_ide::ledger::EditRecord::now(
                    "claude",
                    c.set.turn,
                    Some("Make the bulk discount configurable in application.yml".into()),
                    "Edit",
                );
                c.ledger.record(&f2, Some(&v1b), &v2b, edit);
                c.set.before_edit(&t2, None);
                let text = "package com.example.shop.service;\n\nimport static org.junit.jupiter.api.Assertions.assertEquals;\n\nimport java.math.BigDecimal;\nimport org.junit.jupiter.api.Test;\n\nclass BulkDiscountTest {\n\n    @Test\n    void usesTheConfiguredThreshold() {\n        PriceCalculator prices = new PriceCalculator(new BigDecimal(\"0.20\"), 5, new BigDecimal(\"0.15\"));\n        assertEquals(BigDecimal.ZERO, prices.discount(new BigDecimal(\"40.00\"), 4));\n        assertEquals(new BigDecimal(\"7.50\"), prices.discount(new BigDecimal(\"50.00\"), 5));\n    }\n}\n";
                c.record(t2.clone(), Some(Arc::new(text.into())), None, "Write".into(), Source::Hook, "claude", cx);
                let old = Some(Some(Arc::new(yml_v1.clone())));
                c.record(y2.clone(), Some(Arc::new(yml_v2.clone())), old, String::new(), Source::Watcher, "claude", cx);
                c.expanded.insert(f2.clone());
                let tool = if step == Step::Typing { "Edit" } else { "Write" };
                c.activity = Some(Doing { path: f2.clone(), tool: tool.into(), at: Instant::now(), done: false });
                if matches!(step, Step::Fade | Step::Card | Step::Inline) {
                    c.activity = None;
                    c.pending.push(PendingDiff {
                        tab: "✻ [Claude Code] README.md ⧉".into(),
                        path: s2.join("README.md"),
                        added: 4,
                        removed: 1,
                        new_file: false,
                        front: true,
                    });
                }
            })
        });
        w.pump(1, &mut |_| false);
        let (e2, v2c) = (e.clone(), v2.clone());
        w.with(|ws, window, cx| {
            if !matches!(step, Step::Typing | Step::Card) {
                ws.update(cx, |ws, cx| ws.toggle_tool(Tool::ClaudeChanges, window, cx));
            }
            e2.update(cx, |e, cx| match step {
                Step::Typing => {
                    e.animate_edit(
                        &v2c,
                        true,
                        Some(snout_app::editor::Writer {
                            name: "Claude".into(),
                            color: snout_app::theme::palette(cx).claude().mark,
                        }),
                        window,
                        cx,
                    );
                    e.hold_live_at(0.55, window, cx);
                }
                _ => {
                    e.animate_edit(&v2c, false, None, window, cx);
                }
            });
            if matches!(step, Step::Fade | Step::Card | Step::Inline) {
                ws.update(cx, snout_app::claude_ide::bridge::sync_bases);
            }
        });
        w.pump(1, &mut |_| false);
        let (e3, v1c, v2d) = (e.clone(), v1.clone(), v2.clone());
        w.with(|_, window, cx| {
            e3.update(cx, |e, cx| match step {
                Step::Card => {
                    // The card of the constructor's mark.
                    let _ = (&v1c, &v2d);
                    if let Some(run) = e.agent_lane(cx).into_iter().filter(|r| !r.lines.is_empty()).nth(1) {
                        e.agent_card = Some(snout_app::editor::AgentCard {
                            lines: run.lines.clone(),
                            edit: run.edit,
                            at: gpui::point(gpui::px(600.), gpui::px(250.)),
                        });
                    }
                }
                Step::Inline => {
                    let first = e.claude_hunks.first().map_or(0, |h| h.new_lines.start);
                    e.go_to_claude_line(first, window, cx);
                }
                _ => {}
            });
        });
        w.pump(1, &mut |_| false);
        w.snap(name);
    }
}

/// Wait for the engine: the run configurations, and the project's model.
fn engine_ready(w: &mut Win, root: &Path, secs: u64) {
    let runs = w.runs();
    let started = Instant::now();
    let ok = w.pump(secs, &mut |cx| {
        !runs.read(cx).engine_configs.is_empty()
            && snout_app::project_model::store(cx).is_some_and(|s| s.read(cx).current.is_some())
    });
    assert!(ok, "the engine of {} is ready", root.display());
    println!("docs: engine of {} ready after {:.0} s", root.display(), started.elapsed().as_secs_f32());
}

fn expand(w: &mut Win, ids: Vec<snout_app::project_view::NodeId>) {
    let ws = w.ws.clone();
    w.cx.update(|cx| {
        let project = ws.read(cx).side.read(cx).project.clone();
        project.update(cx, |v, cx| {
            v.expanded.extend(ids);
            cx.notify();
        });
    });
}

/// Put up Project Structure and wait until it has read the model and found the JDKs.
fn open_structure(w: &mut Win) -> Entity<snout_app::structure::ProjectStructureView> {
    w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.open_project_structure(window, cx)));
    let ws = w.ws.clone();
    let view = w.cx.update(|cx| ws.read(cx).project_structure.clone()).expect("Project Structure is up");
    let v = view.clone();
    let ok = w.pump(60, &mut |cx| {
        let v = v.read(cx);
        v.structure().is_some() && v.detected.is_some()
    });
    assert!(ok, "Project Structure reads the model");
    view
}

fn structure_page(
    w: &mut Win,
    view: &Entity<snout_app::structure::ProjectStructureView>,
    f: impl FnOnce(&mut snout_app::structure::ProjectStructureView),
) {
    let v = view.clone();
    w.cx.update(|cx| {
        v.update(cx, |v, cx| {
            f(v);
            cx.notify();
        })
    });
    w.pump(2, &mut |_| false);
}

/// Project Structure on the multi-module build (its pages) and on the Spring app (its
/// dependencies' conflicts, and the facets).
fn structure(projects: &Path) {
    use snout_app::structure::data::{Edge, NodeKind};
    use snout_app::structure::{ModuleSel, Page, Scope};
    let ledger = copy(projects, "structure", "ledger");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![ledger.clone()], Some(ledger.clone())), true);
    w.activity(Activity::Project);
    engine_ready(&mut w, &ledger, 900);
    let view = open_structure(&mut w);
    w.snap("structure-project-light");
    structure_page(&mut w, &view, |v| {
        v.page = Page::Modules;
        v.selected = Some(ModuleSel::Module("ledger:core".into()));
    });
    w.snap("structure-modules-light");
    structure_page(&mut w, &view, |v| {
        v.page = Page::Dependencies;
        v.dep_module = Some("ledger:app".into());
        v.scope = Scope::Runtime;
        v.open.clear();
        let s = v.structure().unwrap();
        let g = s.module("ledger:app").unwrap().graph(Scope::Runtime).unwrap().clone();
        // The projects it declares open, and the library the core brings in.
        for e in &g.roots {
            if let Edge::To { to, .. } = e {
                v.open.insert(vec![*to]);
                if matches!(g.nodes[*to].kind, NodeKind::Project { ref path, .. } if path == ":core") {
                    for d in &g.nodes[*to].deps {
                        if let Edge::To { to: lib, .. } = d {
                            v.open.insert(vec![*to, *lib]);
                        }
                    }
                }
            }
        }
    });
    w.snap("structure-dependencies-light");
    structure_page(&mut w, &view, |v| {
        v.page = Page::Sdks;
        v.selected_jdk = v.detected.as_ref().and_then(|d| d.first()).map(|j| j.home.clone());
    });
    w.snap("structure-sdks-light");
    w.shutdown();

    let shop = copy(projects, "structure", "shop");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop.clone())), true);
    w.activity(Activity::Project);
    engine_ready(&mut w, &shop, 900);
    let ws = w.ws.clone();
    let spring = w.cx.update(|cx| ws.read(cx).spring.clone());
    let ok = w.pump(900, &mut |cx| spring.read(cx).loaded);
    println!("docs: structure spring loaded {ok}");
    let view = open_structure(&mut w);
    structure_page(&mut w, &view, |v| {
        v.page = Page::Dependencies;
        v.scope = Scope::Test;
        v.conflicts_only = true;
    });
    w.snap("structure-conflicts-light");
    structure_page(&mut w, &view, |v| {
        v.conflicts_only = false;
        v.page = Page::Facets;
    });
    w.snap("structure-facets-light");
    w.shutdown();

    // The Modules page in the dark.
    let ledger = copy(projects, "structure", "ledger");
    let mut w = Win::new(settings(ThemeChoice::Dark, vec![ledger.clone()], Some(ledger.clone())), true);
    w.activity(Activity::Project);
    engine_ready(&mut w, &ledger, 900);
    let view = open_structure(&mut w);
    structure_page(&mut w, &view, |v| {
        v.page = Page::Modules;
        v.selected = Some(ModuleSel::Module("ledger:app".into()));
    });
    w.snap("structure-modules-dark");
    w.shutdown();
}

/// The multi-module build: the Project view and the Gradle window.
fn ledger(projects: &Path) {
    use snout_app::project_view::NodeId;
    let ledger = copy(projects, "ledger", "ledger");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![ledger.clone()], Some(ledger.clone())), true);
    w.activity(Activity::Project);
    engine_ready(&mut w, &ledger, 900);
    let modules: Vec<(String, String)> = w.cx.update(|cx| {
        let s = snout_app::project_model::store(cx).unwrap();
        s.read(cx).current.as_ref().unwrap().modules.iter().map(|m| (m.id.clone(), m.name.clone())).collect()
    });
    println!("docs: ledger modules {modules:?}");
    let mut ids: Vec<NodeId> = modules.iter().map(|(id, _)| NodeId::Module(id.clone())).collect();
    let core = ledger.join("core");
    for d in [
        "src",
        "src/main",
        "src/main/java",
        "src/main/java/com",
        "src/main/java/com/example",
        "src/main/java/com/example/ledger",
        "src/main/java/com/example/ledger/core",
    ] {
        ids.push(NodeId::Dir(core.join(d)));
    }
    ids.push(NodeId::Libraries);
    expand(&mut w, ids);
    w.open(&core.join("src/main/java/com/example/ledger/core/Ledger.java"), Some((14, 0)));
    w.pump(5, &mut |_| false);
    w.snap("projects-project-view-light");
    // The Gradle window, with a task run in the Build window.
    let runs = w.runs();
    let r = runs.clone();
    w.pump(120, &mut |cx| !r.read(cx).gradle.builds.is_empty());
    w.tool(Tool::Gradle);
    let ws = w.ws.clone();
    w.cx.update(|cx| {
        let g = ws.read(cx).gradle.clone();
        g.update(cx, |g, cx| {
            g.reveal("run", cx);
        })
    });
    w.cx.update(|cx| {
        runs.update(cx, |r, cx| r.start(snout_app::run::Launch::Task { build: None, task: ":app:run".into() }, cx))
    });
    let r = runs.clone();
    w.pump(300, &mut |cx| r.read(cx).sessions.last().is_some_and(|s| s.ended.is_some()));
    w.pump(2, &mut |_| false);
    w.snap("gradle-window-light");
    w.shutdown();
}

/// The Spring Boot shop with the engine: the editor's Java features, Gradle, runs, tests,
/// Spring, search and the processes.
fn shop(projects: &Path) {
    use snout_app::project_view::NodeId;
    let shop = copy(projects, "java", "shop");
    let pkg = shop.join("src/main/java/com/example/shop");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop.clone())), true);
    w.activity(Activity::Project);
    let order_service = pkg.join("service/OrderService.java");
    let e = w.open(&order_service, Some((24, 0)));
    engine_ready(&mut w, &shop, 900);
    let ws = w.ws.clone();
    let hub = w.cx.update(|cx| ws.read(cx).hub.clone());
    let s2 = shop.clone();
    let ok = w.pump(600, &mut |cx| hub.read(cx).servers.get(&s2).is_some_and(|s| s.first_diagnostics.is_some()));
    println!("docs: shop diagnostics {ok}");
    let mut ids = vec![NodeId::Root];
    for d in [
        "src",
        "src/main",
        "src/main/java",
        "src/main/java/com",
        "src/main/java/com/example",
        "src/main/java/com/example/shop",
        "src/main/java/com/example/shop/service",
        "src/main/java/com/example/shop/web",
    ] {
        ids.push(NodeId::Dir(shop.join(d)));
    }
    let root_module: Option<String> = w.cx.update(|cx| {
        snout_app::project_model::store(cx)
            .unwrap()
            .read(cx)
            .current
            .as_ref()
            .unwrap()
            .modules
            .first()
            .map(|m| m.id.clone())
    });
    if let Some(m) = root_module.clone() {
        ids.push(NodeId::Module(m));
    }
    expand(&mut w, ids);
    w.pump(10, &mut |_| false);
    let original = w.text(&e);

    // Completion: the members of the injected service.
    w.caret(&e, "        this.prices = prices;\n", "        this.prices = prices;\n".len());
    w.insert(&e, "        productService.\n");
    w.caret(&e, "productService.\n", "productService.".len());
    w.pump(2, &mut |_| false);
    let e2 = e.clone();
    w.with(|_, window, cx| e2.update(cx, |e, cx| e.state.update(cx, |s, cx| s.show_completions(window, cx))));
    let e2 = e.clone();
    let ok = w.pump(60, &mut |cx| !e2.read(cx).state.read(cx).completion_menu_state().items.is_empty());
    println!("docs: completion {ok}");
    w.pump(2, &mut |_| false);
    w.snap("java-completion-light");
    w.key("escape");
    w.set_text(&e, original.clone());
    w.pump(3, &mut |_| false);

    // Hover over a method.
    w.hover(&e, "withTax(net", 2);
    w.pump(1, &mut |_| false);
    w.snap("java-hover-light");
    w.with(|_, window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: gpui::point(px(W - 20.), px(H - 10.)),
                pressed_button: None,
                modifiers: gpui::Modifiers::none(),
            }),
            cx,
        );
    });
    w.caret(&e, "package", 0);
    w.pump(1, &mut |_| false);

    // Errors and their quick fix: a type not imported, used on two lines.
    w.caret(&e, "        int items = 0;\n", 0);
    w.insert(&e, "        LocalDate placed = LocalDate.now();\n        LocalDate due = placed.plusDays(3);\n");
    let e2 = e.clone();
    let ok = w.pump(120, &mut |cx| e2.read(cx).problems.len() >= 3);
    println!(
        "docs: problems {ok} {:?}",
        w.cx.update(|cx| e.read(cx).problems.iter().map(|d| d.message.clone()).collect::<Vec<_>>())
    );
    w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.show_problems(window, cx)));
    w.pump(2, &mut |_| false);
    w.snap("java-problems-light");
    // The pointer resting on the error: the card with its best fix.
    w.hover(&e, "LocalDate placed", 3);
    let e2 = e.clone();
    let ok = w.pump(60, &mut |cx| {
        e2.read(cx).hover.card.as_ref().is_some_and(|c| matches!(c.fixes, snout_app::editor::Fixes::Found(_)))
    });
    println!("docs: problem hover {ok}");
    w.pump(1, &mut |_| false);
    w.snap("java-problem-hover-light");
    let e2 = e.clone();
    w.with(|_, _, cx| e2.update(cx, |e, cx| e.hide_card(cx)));
    w.with(|_, window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: gpui::point(px(W - 20.), px(H - 10.)),
                pressed_button: None,
                modifiers: gpui::Modifiers::none(),
            }),
            cx,
        );
    });
    // The caret on it: the bulb, and ⌥↩ lists the context actions.
    w.caret(&e, "LocalDate placed", 2);
    let e2 = e.clone();
    let ok = w.pump(30, &mut |cx| e2.read(cx).hover.bulb.is_some());
    println!("docs: bulb {ok}");
    w.key("alt-enter");
    let e2 = e.clone();
    let ok = w.pump(60, &mut |cx| e2.read(cx).hover.actions.as_ref().is_some_and(|l| !l.loading));
    let labels: Vec<String> = w.cx.update(|cx| {
        e.read(cx)
            .hover
            .actions
            .as_ref()
            .map(|l| l.entries.iter().map(|x| x.label("Claude")).collect())
            .unwrap_or_default()
    });
    println!("docs: quick fixes {ok} {labels:?}");
    w.pump(1, &mut |_| false);
    w.snap("java-quick-fix-light");
    w.key("escape");
    w.set_text(&e, original.clone());
    let e2 = e.clone();
    w.pump(60, &mut |cx| e2.read(cx).problems.is_empty());

    w.tool(Tool::Problems);
    w.pump(1, &mut |_| false);

    // Rename.
    w.caret(&e, "place(String", 2);
    let e2 = e.clone();
    w.with(|_, window, cx| e2.update(cx, |e, cx| e.start_rename(window, cx)));
    w.pump(1, &mut |_| false);
    let e2 = e.clone();
    w.with(|_, window, cx| {
        e2.update(cx, |e, cx| {
            if let Some(input) = e.rename.clone() {
                input.update(cx, |i, cx| i.set_value("placeOrder", window, cx));
            }
        })
    });
    w.pump(1, &mut |_| false);
    w.snap("java-rename-light");
    let e2 = e.clone();
    w.with(|_, window, cx| e2.update(cx, |e, cx| e.cancel_rename(window, cx)));

    // Find usages of the product service.
    let refs = w.cx.update(|cx| ws.read(cx).references.clone());
    for _ in 0..20 {
        w.caret(&e, "ProductService productService", 3);
        let e2 = e.clone();
        w.cx.update(|cx| e2.update(cx, |e, cx| e.find_references(cx)));
        let r = refs.clone();
        if w.pump(4, &mut |cx| r.read(cx).len() >= 2) {
            break;
        }
    }
    w.pump(2, &mut |_| false);
    w.snap("java-references-light");

    // Tests: all of them, one failing.
    let runs = w.runs();
    let tests = w.cx.update(|cx| {
        runs.read(cx).configurations().into_iter().find(|c| c.kind == snout_app::run::config::Kind::Tests)
    });
    println!("docs: test configuration {:?}", tests.as_ref().map(|c| (&c.name, &c.module)));
    let module = tests.as_ref().map(|c| c.module.clone()).or(root_module.clone()).unwrap();
    w.cx.update(|cx| {
        runs.update(cx, |r, cx| {
            r.start(
                snout_app::run::Launch::Tests {
                    module: module.clone(),
                    filter: serde_json::Value::Null,
                    label: "All tests in shop".into(),
                    debug: false,
                    rerun_failed: false,
                },
                cx,
            )
        })
    });
    let r = runs.clone();
    let ok = w.pump(600, &mut |cx| r.read(cx).sessions.last().is_some_and(|s| s.ended.is_some()));
    println!("docs: tests ended {ok}");
    let test_file = shop.join("src/test/java/com/example/shop/service/OrderServiceTest.java");
    w.open(&test_file, Some((24, 0)));
    w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.show_bottom(ws.tabs.run, window, cx)));
    w.pump(4, &mut |_| false);
    w.snap("run-tests-light");

    // The application, from the title bar's configuration.
    let app = pkg.join("ShopApplication.java");
    w.open(&app, Some((8, 0)));
    let selected = w.cx.update(|cx| runs.read(cx).selected_config());
    println!("docs: title bar configuration {:?}", selected.as_ref().map(|c| &c.name));
    if let Some(config) = selected {
        w.cx.update(|cx| runs.update(cx, |r, cx| r.start(snout_app::run::Launch::Config { config, debug: false }, cx)));
        let r = runs.clone();
        let ok = w.pump(300, &mut |cx| {
            r.read(cx)
                .sessions
                .last()
                .is_some_and(|s| s.lines.iter().any(|l| l.text().contains("Started ShopApplication")))
        });
        println!("docs: application started {ok}");
        w.pump(3, &mut |_| false);
        w.snap("run-window-light");
        // The processes: the engine, its language server, Gradle, the application.
        let p = w.cx.update(|cx| ws.read(cx).processes.clone());
        w.cx.update(|cx| p.update(cx, |p, cx| p.look(cx)));
        w.tool(Tool::Processes);
        w.pump(6, &mut |_| false);
        w.snap("processes-light");
        w.tool(Tool::Processes);
        let id = w.cx.update(|cx| runs.read(cx).sessions.last().unwrap().id);
        w.cx.update(|cx| runs.update(cx, |r, cx| r.stop(id, cx)));
        let r = runs.clone();
        w.pump(30, &mut |cx| !r.read(cx).any_running());
    }

    // Gradle: the shop's tasks, a build run in the Build window.
    w.tool(Tool::Gradle);
    w.cx.update(|cx| {
        let g = ws.read(cx).gradle.clone();
        g.update(cx, |g, cx| {
            g.reveal("bootRun", cx);
            g.reveal("classes", cx);
        })
    });
    w.cx.update(|cx| {
        runs.update(cx, |r, cx| r.start(snout_app::run::Launch::Task { build: None, task: ":classes".into() }, cx))
    });
    let r = runs.clone();
    w.pump(300, &mut |cx| r.read(cx).sessions.last().is_some_and(|s| s.ended.is_some()));
    w.pump(2, &mut |_| false);
    w.snap("gradle-shop-light");
    w.tool(Tool::Gradle);
    w.tool(Tool::Build);

    // Spring: the gutter, the beans, the endpoints.
    let spring = w.cx.update(|cx| ws.read(cx).spring.clone());
    let s = spring.clone();
    let ok = w.pump(900, &mut |cx| s.read(cx).loaded);
    println!("docs: spring loaded {ok}");
    let controller = pkg.join("web/OrderController.java");
    let ce = w.open(&controller, Some((20, 0)));
    let c2 = ce.clone();
    w.pump(120, &mut |cx| c2.read(cx).spring.as_ref().is_some_and(|s| !s.lines.is_empty()));
    w.tool(Tool::Spring);
    let win = w.cx.update(|cx| ws.read(cx).spring_window.clone());
    w.cx.update(|cx| win.update(cx, |w, cx| w.set_tab(snout_app::spring::window::Tab::Beans, cx)));
    // Every package open.
    let keys: Vec<String> = w.cx.update(|cx| {
        win.read(cx)
            .bean_rows(cx)
            .into_iter()
            .filter_map(|r| match r {
                snout_app::spring::window::BeanRow::Group { key, open: false, .. } => Some(key),
                _ => None,
            })
            .collect()
    });
    for k in keys {
        w.cx.update(|cx| win.update(cx, |w, cx| w.toggle(&k, cx)));
    }
    w.pump(2, &mut |_| false);
    w.snap("spring-beans-light");
    w.with(|_, window, cx| win.update(cx, |w, cx| w.show_endpoints(None, window, cx)));
    w.pump(2, &mut |_| false);
    w.snap("spring-endpoints-light");
    w.tool(Tool::Spring);
    // The injection gutter: the service's constructor, and the beans it takes.
    let oe = w.open(&order_service, Some((16, 0)));
    let o2 = oe.clone();
    w.pump(120, &mut |cx| o2.read(cx).spring.as_ref().is_some_and(|s| !s.lines.is_empty()));
    w.pump(2, &mut |_| false);
    let text = w.text(&oe);
    let line = text.lines().position(|l| l.contains("public OrderService(")).unwrap() as u32;
    let o3 = oe.clone();
    let at = w.cx.update(|cx| {
        let e = o3.read(cx);
        let (a, z) = e.lanes.get().spring();
        let s = e.state.read(cx);
        let off = text.find("public OrderService(").unwrap();
        let b = s.range_to_bounds(&(off..off)).unwrap();
        gpui::point((a + z) / 2., b.center().y)
    });
    let o4 = oe.clone();
    w.cx.update(|cx| {
        o4.update(cx, |e, cx| {
            if let Some(l) = e.spring.as_ref().and_then(|s| s.line(line)).cloned() {
                let items = snout_app::editor::spring_items(&l);
                e.gutter_menu = Some(snout_app::editor::GutterMenu::Spring { at, items, filter: None });
            } else {
                println!("docs: no Spring marks on line {}", line + 1);
            }
            cx.notify();
        })
    });
    w.pump(1, &mut |_| false);
    w.snap("spring-injections-light");
    w.cx.update(|cx| o4.update(cx, |e, _| e.gutter_menu = None));

    // Profiles: sample data only in dev, and the model evaluated under prod.
    let resources = shop.join("src/main/resources");
    std::fs::write(resources.join("application-dev.yml"), "shop:\n  tax-rate: 0.00\n").unwrap();
    std::fs::write(resources.join("application-prod.yml"), "server:\n  port: 80\n").unwrap();
    let sample = pkg.join("service/SampleData.java");
    std::fs::write(
        &sample,
        "package com.example.shop.service;\n\nimport org.springframework.context.annotation.Profile;\nimport org.springframework.stereotype.Component;\n\n/** Products to try the shop with, in development. */\n@Component\n@Profile(\"dev\")\nclass SampleData {\n}\n",
    )
    .unwrap();
    let se = w.open(&sample, Some((8, 0)));
    let s3 = se.clone();
    w.pump(120, &mut |cx| s3.read(cx).spring.as_ref().is_some_and(|s| !s.lines.is_empty()));
    let prod = snout_app::spring::ProfileMode::Chosen(vec!["prod".into()]);
    w.cx.update(|cx| spring.update(cx, |s, cx| s.set_profile_mode(prod, cx)));
    let s4 = spring.clone();
    let ok = w.pump(60, &mut |cx| s4.read(cx).profile_info.inactive_beans() > 0);
    println!("docs: profiles evaluated {ok}");
    w.tool(Tool::Spring);
    let keys: Vec<String> = w.cx.update(|cx| {
        win.read(cx)
            .bean_rows(cx)
            .into_iter()
            .filter_map(|r| match r {
                snout_app::spring::window::BeanRow::Group { key, open: false, .. } => Some(key),
                _ => None,
            })
            .collect()
    });
    for k in keys {
        w.cx.update(|cx| win.update(cx, |w, cx| w.toggle(&k, cx)));
    }
    w.cx.update(|cx| win.update(cx, |w, cx| w.set_tab(snout_app::spring::window::Tab::Beans, cx)));
    w.pump(3, &mut |_| false);
    w.snap("spring-profiles-light");
    w.cx.update(|cx| spring.update(cx, |s, cx| s.set_profile_mode(snout_app::spring::ProfileMode::Any, cx)));
    w.tool(Tool::Spring);
    let s5 = se.clone();
    w.with(|ws, window, cx| {
        let dock = ws.read(cx).dock.clone();
        dock.update(cx, |d, cx| d.remove_panel(s5, window, cx));
    });
    for f in [resources.join("application-dev.yml"), resources.join("application-prod.yml"), sample.clone()] {
        let _ = std::fs::remove_file(f);
    }

    // application.yml: completion of Spring's keys.
    let yml = shop.join("src/main/resources/application.yml");
    let ye = w.open(&yml, Some((3, 0)));
    w.pump(10, &mut |_| false);
    w.caret(&ye, "    default-property-inclusion: non_null\n", "    default-property-inclusion: non_null\n".len());
    w.insert(&ye, "  mvc:\n    \n");
    w.caret(&ye, "  mvc:\n    \n", "  mvc:\n    ".len());
    let y2 = ye.clone();
    w.with(|_, window, cx| y2.update(cx, |e, cx| e.state.update(cx, |s, cx| s.show_completions(window, cx))));
    let y3 = ye.clone();
    let ok = w.pump(60, &mut |cx| !y3.read(cx).state.read(cx).completion_menu_state().items.is_empty());
    println!("docs: yml completion {ok}");
    w.pump(1, &mut |_| false);
    w.snap("spring-yml-completion-light");
    w.key("escape");
    let y4 = ye.clone();
    w.with(|_, window, cx| y4.update(cx, |e, cx| e.reload(window, cx)));

    // Search Everywhere, and the command palette.
    w.open(&order_service, None);
    w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.search_everywhere(window, cx)));
    w.pump(1, &mut |_| false);
    w.with(|ws, window, cx| {
        if let Some(s) = ws.read(cx).search.clone() {
            s.update(cx, |s, cx| s.set_query("Order", window, cx));
        }
    });
    w.pump(10, &mut |_| false);
    w.snap("editor-search-everywhere-light");
    w.key("escape");
    w.shutdown();
}

/// The Java tools on the shop: parameter name hints, the Structure window, the call
/// hierarchy, Go to Class and Extract Method.
fn java_tools(projects: &Path) {
    use snout_app::editor::Refactoring;
    use snout_app::lang::hierarchy::Mode;
    let shop = copy(projects, "javatools", "shop");
    let pkg = shop.join("src/main/java/com/example/shop");
    // A rule with two kinds, for Go to Implementations.
    std::fs::write(
        pkg.join("service/PriceRule.java"),
        "package com.example.shop.service;\n\nimport java.math.BigDecimal;\n\n/** A change to a price. */\npublic interface PriceRule {\n    BigDecimal apply(BigDecimal price);\n}\n",
    )
    .unwrap();
    std::fs::write(
        pkg.join("service/HalfPrice.java"),
        "package com.example.shop.service;\n\nimport java.math.BigDecimal;\n\npublic class HalfPrice implements PriceRule {\n    @Override\n    public BigDecimal apply(BigDecimal price) {\n        return price.divide(BigDecimal.valueOf(2));\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        pkg.join("service/RoundUp.java"),
        "package com.example.shop.service;\n\nimport java.math.BigDecimal;\nimport java.math.RoundingMode;\n\npublic class RoundUp implements PriceRule {\n    @Override\n    public BigDecimal apply(BigDecimal price) {\n        return price.setScale(0, RoundingMode.UP);\n    }\n}\n",
    )
    .unwrap();
    let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop.clone())), true);
    w.activity(Activity::Project);
    let products = pkg.join("repository/ProductRepository.java");
    let e = w.open(&products, Some((18, 0)));
    engine_ready(&mut w, &shop, 900);
    let ws = w.ws.clone();
    let hub = w.cx.update(|cx| ws.read(cx).hub.clone());
    let s2 = shop.clone();
    let ok = w.pump(600, &mut |cx| hub.read(cx).servers.get(&s2).is_some_and(|s| s.first_diagnostics.is_some()));
    println!("docs: javatools diagnostics {ok}");

    // Parameter name hints: the products made from literals.
    let e2 = e.clone();
    let ok = w.pump(60, &mut |cx| !e2.read(cx).param_hints.is_empty());
    println!("docs: parameter hints {ok}");
    w.caret(&e, "    public List<Product> findAll", 0);
    w.pump(2, &mut |_| false);
    w.snap("java-param-hints-light");

    // The Structure window beside the order service, the caret in `place`.
    let orders = pkg.join("service/OrderService.java");
    let oe = w.open(&orders, Some((24, 0)));
    w.caret(&oe, "        int items = 0;", 8);
    w.tool(Tool::Structure);
    let structure = w.cx.update(|cx| ws.read(cx).structure.clone());
    let ok = w.pump(30, &mut |cx| structure.read(cx).current.is_some());
    println!("docs: structure {ok}");
    w.pump(2, &mut |_| false);
    w.snap("java-structure-light");
    w.tool(Tool::Structure);

    // The callers of `withTax`, and the callers of `place`.
    let calc = pkg.join("service/PriceCalculator.java");
    let ce = w.open(&calc, Some((20, 0)));
    w.caret(&ce, "withTax(BigDecimal net)", 2);
    w.with(|ws, window, cx| ws.update(cx, |w, cx| w.show_hierarchy(Mode::Callers, window, cx)));
    let hierarchy = w.cx.update(|cx| ws.read(cx).hierarchy.clone());
    let h = hierarchy.clone();
    let ok = w.pump(60, &mut |cx| h.read(cx).nodes.iter().any(|n| n.depth == 1));
    println!("docs: call hierarchy {ok}");
    let place = w.cx.update(|cx| hierarchy.read(cx).nodes.iter().position(|n| n.name.starts_with("place")));
    if let Some(place) = place {
        w.cx.update(|cx| hierarchy.update(cx, |h, cx| h.expand(place, cx)));
        let h = hierarchy.clone();
        w.pump(30, &mut |cx| h.read(cx).nodes.iter().any(|n| n.depth == 2));
    }
    w.pump(2, &mut |_| false);
    w.snap("java-call-hierarchy-light");
    w.tool(Tool::Hierarchy);

    // ⌘ held over a call: the name is a link. ⌘-click on the declaration: its usages.
    let oe2 = w.open(&orders, Some((30, 0)));
    let call = w.text(&oe2).find("prices.withTax").unwrap() + 9;
    let o3 = oe2.clone();
    let at =
        w.cx.update(|cx| o3.read(cx).state.read(cx).range_to_bounds(&(call..call + 1)).map(|b| b.center())).unwrap();
    for _ in 0..20 {
        w.with(|_, window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                    position: at,
                    pressed_button: None,
                    modifiers: gpui::Modifiers::command(),
                }),
                cx,
            );
        });
        let o4 = oe2.clone();
        if w.pump(1, &mut |cx| o4.read(cx).nav.link.is_some()) {
            break;
        }
    }
    w.draw_and_snap("java-nav-link-light");
    let ce = w.open(&calc, Some((25, 0)));
    w.caret(&ce, "withTax(BigDecimal net)", 2);
    let c2 = ce.clone();
    w.with(|_, window, cx| c2.update(cx, |e, cx| e.go_to_declaration(None, window, cx)));
    let c3 = ce.clone();
    let ok = w.pump(60, &mut |cx| c3.read(cx).nav.popup.is_some());
    println!("docs: usages popup {ok}");
    w.pump(1, &mut |_| false);
    w.snap("java-usages-light");
    // ⌥⌘B on the rule: its two implementations (the new files opened once, so the engine
    // has read them).
    w.open(&pkg.join("service/HalfPrice.java"), Some((0, 0)));
    w.open(&pkg.join("service/RoundUp.java"), Some((0, 0)));
    w.pump(5, &mut |_| false);
    let re = w.open(&pkg.join("service/PriceRule.java"), Some((5, 0)));
    w.caret(&re, "PriceRule {", 2);
    let r2 = re.clone();
    w.with(|_, window, cx| r2.update(cx, |e, cx| e.go_to_implementations(None, window, cx)));
    let r3 = re.clone();
    w.pump(60, &mut |cx| r3.read(cx).nav.popup.is_some() || r3.read(cx).hint.is_some());
    let ok = w.cx.update(|cx| re.read(cx).nav.popup.is_some());
    println!("docs: implementations popup {ok}");
    w.pump(1, &mut |_| false);
    w.snap("java-implementations-light");
    for e in [ce.clone(), re] {
        w.cx.update(|cx| {
            e.update(cx, |e, cx| {
                e.nav.popup = None;
                cx.notify();
            })
        });
    }
    w.pump(1, &mut |_| false);

    // Go to Class: the shop's classes and the libraries' by a part of the name.
    w.with(|ws, window, cx| ws.update(cx, |w, cx| w.open_classes(window, cx)));
    let search = w.cx.update(|cx| ws.read(cx).search.clone()).expect("Go to Class");
    let s3 = search.clone();
    w.with(|_, window, cx| s3.update(cx, |s, cx| s.set_query("Order", window, cx)));
    let s4 = search.clone();
    let ok = w.pump(30, &mut |cx| s4.read(cx).items.len() >= 4);
    println!("docs: go to class {ok}");
    w.pump(2, &mut |_| false);
    w.snap("java-go-to-class-light");
    w.key("escape");
    w.cx.update(|cx| {
        ws.update(cx, |w, cx| {
            w.search = None;
            cx.notify();
        })
    });

    // Extract Method on the discount's sum: the new method, its name ready to rename. The
    // engine may still be indexing: ask until it answers.
    let original = w.text(&ce);
    let find = "total.multiply(new BigDecimal(\"0.10\")).setScale(2, RoundingMode.HALF_UP)";
    let mut ok = false;
    for _ in 0..10 {
        let at = w.text(&ce).find(find).expect("the discount is there");
        let c2 = ce.clone();
        w.with(|_, window, cx| {
            c2.update(cx, |e, cx| {
                e.state.update(cx, |s, cx| {
                    s.set_selected_range(at..at + find.len(), cx);
                    s.focus(window, cx);
                });
                e.refactor(Refactoring::ExtractMethod, window, cx);
            })
        });
        let c3 = ce.clone();
        if w.pump(6, &mut |cx| c3.read(cx).rename.is_some()) {
            ok = true;
            break;
        }
    }
    println!("docs: extract method {ok}");
    w.pump(2, &mut |_| false);
    w.snap("java-extract-method-light");
    let c4 = ce.clone();
    w.with(|_, window, cx| c4.update(cx, |e, cx| e.cancel_rename(window, cx)));
    w.set_text(&ce, original);

    // The editor's right-click menu, on the price calculator.
    w.caret(&ce, "withTax(BigDecimal net)", 2);
    w.pump(2, &mut |_| false);
    let c5 = ce.clone();
    w.with(|_, window, cx| {
        c5.update(cx, |e, cx| {
            e.open_context_menu_at(gpui::point(gpui::px(760.), gpui::px(330.)), true, None, window, cx)
        })
    });
    w.pump(1, &mut |_| false);
    w.snap("editor-context-menu-light");
    w.key("escape");
    w.shutdown();
}

/// The command palette, over the shop's editor.
fn palette(projects: &Path) {
    let shop = copy(projects, "palette", "shop");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![shop.clone()], Some(shop.clone())), false);
    w.activity(Activity::Project);
    w.open(&shop.join("src/main/java/com/example/shop/service/OrderService.java"), Some((22, 0)));
    w.pump(2, &mut |_| false);
    w.with(|ws, window, cx| {
        // Drawn once, so the palette finds the editor's own shortcuts.
        window.draw(cx).clear(cx);
        ws.update(cx, |ws, cx| ws.open_palette(window, cx));
    });
    w.pump(1, &mut |_| false);
    w.with(|ws, window, cx| {
        if let Some(s) = ws.read(cx).search.clone() {
            s.update(cx, |s, cx| s.set_query("go to", window, cx));
        }
    });
    w.pump(3, &mut |_| false);
    w.snap("editor-command-palette-light");
    // Replace in Files: a class renamed in the Java files, its test left out.
    w.with(|ws, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.search = None;
            ws.open_find_in_files(true, window, cx);
        });
        let view = ws.read(cx).find_in_files.clone().unwrap();
        view.update(cx, |v, cx| {
            v.find.update(cx, |i, cx| i.set_value("ProductNotFound", window, cx));
            v.replace.update(cx, |i, cx| i.set_value("ProductMissing", window, cx));
            v.mask.update(cx, |i, cx| i.set_value("*.java", window, cx));
            v.toggle("word", cx);
        });
    });
    let ws = w.ws.clone();
    w.pump(10, &mut |cx| ws.read(cx).find_in_files.as_ref().is_some_and(|v| v.read(cx).match_count() > 3));
    w.with(|ws, _, cx| {
        let view = ws.read(cx).find_in_files.clone().unwrap();
        view.update(cx, |v, cx| {
            if let Some(fi) = v.found.files.iter().position(|f| f.path.ends_with("Test.java")) {
                v.toggle_file(fi, cx);
            }
            v.selected = 1;
        });
    });
    w.pump(1, &mut |_| false);
    w.snap("editor-replace-in-files-light");
}

/// The debugger, stopped at a breakpoint in the greeter's loop.
fn debug(projects: &Path, theme: ThemeChoice, mode: &str) {
    let greeter = copy(projects, &format!("debug-{mode}"), "greeter");
    let mut w = Win::new(settings(theme, vec![greeter.clone()], Some(greeter.clone())), true);
    w.activity(Activity::Project);
    let main = greeter.join("src/main/java/demo/Main.java");
    w.open(&main, Some((9, 0)));
    engine_ready(&mut w, &greeter, 900);
    let text = std::fs::read_to_string(&main).unwrap();
    let line = text.lines().position(|l| l.contains("total = add(total, i);")).unwrap() as u32;
    let runs = w.runs();
    w.cx.update(|cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&main, line, cx)));
    let config = w.cx.update(|cx| runs.read(cx).configurations().into_iter().find(|c| c.main_class.is_some()).unwrap());
    w.cx.update(|cx| runs.update(cx, |r, cx| r.select_config(Some(config.id.clone()), cx)));
    w.cx.update(|cx| runs.update(cx, |r, cx| r.start(snout_app::run::Launch::Config { config, debug: true }, cx)));
    let stopped = w.pump(300, &mut |cx| {
        let m = snout_app::run::marks(cx).unwrap();
        m.read(cx).exec.is_some() && !m.read(cx).locals.is_empty()
    });
    assert!(stopped, "the breakpoint is hit");
    let id = w.cx.update(|cx| runs.read(cx).sessions.last().unwrap().id);
    w.cx.update(|cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
    w.pump(30, &mut |cx| snout_app::run::marks(cx).unwrap().read(cx).locals.iter().any(|(n, v)| n == "i" && v == "2"));
    w.cx.update(|cx| runs.update(cx, |r, cx| r.add_watch(id, "total * 10".into(), cx)));
    w.cx.update(|cx| runs.update(cx, |r, cx| r.add_watch(id, "names.get(i - 1)".into(), cx)));
    w.pump(4, &mut |_| false);
    w.snap(&format!("debug-paused-{mode}"));
    if mode == "light" {
        // Evaluate Expression, on the expression of a watch.
        w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.open_evaluate(window, cx)));
        w.with(|ws, window, cx| {
            let d = ws.read(cx).evaluate_dialog.clone().unwrap();
            d.update(cx, |d, cx| {
                d.input.update(cx, |i, cx| i.set_value("total * 10", window, cx));
                d.evaluate(cx);
            });
        });
        w.pump(10, &mut |cx| {
            let r = runs.read(cx);
            r.session(id).and_then(|s| s.debug.as_ref()).is_some_and(|d| d.evaluated.is_some())
        });
        w.snap("evaluate-light");
        w.with(|ws, _, cx| ws.update(cx, |ws, _| ws.evaluate_dialog = None));
        // View Breakpoints: one with a condition, one disabled.
        let greeter_java = greeter.join("src/main/java/demo/Greeter.java");
        let print = text.lines().position(|l| l.contains("System.out.println")).unwrap() as u32;
        w.cx.update(|cx| {
            runs.update(cx, |r, cx| {
                r.set_condition(&main, line, Some("i == 3".into()), cx);
                r.toggle_breakpoint(&main, print, cx);
                r.set_breakpoint_enabled(&main, print, false, cx);
                r.toggle_breakpoint(&greeter_java, 9, cx);
            })
        });
        w.with(|ws, window, cx| ws.update(cx, |ws, cx| ws.open_breakpoints(window, cx)));
        w.pump(2, &mut |_| false);
        w.snap("breakpoints-light");
    }
    w.shutdown();
}

/// Remote SSH against a Linux container (`crates/snout-server/tests/docker`) as `devbox`.
fn remote(projects: &Path) {
    use snout_app::remote::{Connection, Recents, Remote, RemoteProject};
    let ssh_dir = PathBuf::from(std::env::var_os("SNOUT_DOCS_SSH_DIR").expect("SNOUT_DOCS_SSH_DIR: a short folder"));
    let _ = std::fs::remove_dir_all(&ssh_dir);
    std::fs::create_dir_all(&ssh_dir).unwrap();
    let key = ssh_dir.join("key");
    let here = Path::new(".");
    sh(here, &["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "docs", "-f", key.to_str().unwrap()]);
    let public = std::fs::read_to_string(ssh_dir.join("key.pub")).unwrap();
    let docker = Path::new(env!("CARGO_MANIFEST_DIR")).join("../snout-server/tests/docker");
    sh(here, &["docker", "build", "-q", "-t", "snout-remote-test:2", docker.to_str().unwrap()]);
    let name = format!("snout-docs-{}", std::process::id());
    struct Container(String);
    impl Drop for Container {
        fn drop(&mut self) {
            let _ = Command::new("docker").args(["unpause", &self.0]).output();
            let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
        }
    }
    let container = Container(name.clone());
    sh(
        here,
        &[
            "docker",
            "run",
            "-d",
            "--rm",
            "--name",
            &name,
            "--hostname",
            "devbox",
            "-p",
            "127.0.0.1::22",
            "-e",
            &format!("AUTHORIZED_KEY={}", public.trim()),
            "snout-remote-test:2",
        ],
    );
    // The demo projects in the home folder there.
    sh(here, &["docker", "exec", &name, "sh", "-c", "rm -rf /home/dev/demo && mkdir -p /home/dev/projects"]);
    for p in ["shop", "ledger", "greeter"] {
        sh(here, &["docker", "cp", projects.join(p).to_str().unwrap(), &format!("{name}:/home/dev/projects/{p}")]);
    }
    sh(here, &["docker", "exec", &name, "chown", "-R", "dev:dev", "/home/dev/projects"]);
    let port = sh(here, &["docker", "port", &name, "22"]);
    let port = port.lines().next().unwrap().rsplit(':').next().unwrap().to_owned();
    let config = ssh_dir.join("config");
    std::fs::write(
        &config,
        format!(
            "Host devbox\n  HostName 127.0.0.1\n  Port {port}\n  User dev\n  IdentityFile {}\n  IdentitiesOnly yes\n  \
             IdentityAgent none\n  StrictHostKeyChecking no\n  UserKnownHostsFile /dev/null\n  LogLevel ERROR\n\
             Host build-arm build-x86\n  User ci\nHost staging\n  User deploy\n",
            key.display()
        ),
    )
    .unwrap();
    // SAFETY: set before the app starts its threads.
    unsafe {
        std::env::set_var(snout_app::remote::SSH_CONFIG_ENV, &config);
        std::env::set_var("SNOUT_SSH_CONTROL_DIR", ssh_dir.join("c"));
    }
    let start = Instant::now();
    while !Command::new("ssh").arg("-F").arg(&config).args(["devbox", "true"]).status().is_ok_and(|s| s.success()) {
        assert!(start.elapsed() < Duration::from_secs(30), "sshd did not come up");
        std::thread::sleep(Duration::from_millis(300));
    }

    let greeter = copy(projects, "remote", "greeter");
    let mut w = Win::new(settings(ThemeChoice::Light, vec![greeter.clone()], Some(greeter.clone())), false);
    w.cx.update(|cx| snout_app::remote::init(None, cx));
    w.cx.update(|cx| {
        cx.global_mut::<Remote>().recents = Recents {
            hosts: vec!["devbox".into()],
            projects: vec![RemoteProject { host: "devbox".into(), path: "/home/dev/projects/ledger".into() }],
        }
    });
    // The Connect dialog, devbox typed.
    w.with(|ws, window, cx| ws.update(cx, |w, cx| w.open_connect(window, cx)));
    w.pump(2, &mut |_| false);
    w.with(|ws, window, cx| {
        if let Some(d) = ws.read(cx).connect_dialog.clone() {
            d.update(cx, |d, cx| d.input.update(cx, |i, cx| i.set_value("devbox", window, cx)));
        }
    });
    w.pump(1, &mut |_| false);
    w.snap("remote-connect-light");
    w.with(|ws, _, cx| {
        ws.update(cx, |w, cx| {
            w.connect_dialog = None;
            cx.notify();
        })
    });
    // Connect for real; the folder picker on the projects there.
    type Connected = Option<Result<Arc<snout_remote::RemoteHost>, String>>;
    let got: Rc<RefCell<Connected>> = Rc::default();
    let g2 = got.clone();
    w.cx.update(|cx| snout_app::remote::connect("devbox".into(), move |r, _| *g2.borrow_mut() = Some(r), cx));
    let g3 = got.clone();
    assert!(w.pump(240, &mut |_| g3.borrow().is_some()), "connected");
    let host = got.borrow_mut().take().unwrap().expect("connected to devbox");
    let h2: snout_host::HostRef = host.clone();
    w.with(|ws, window, cx| {
        let picker = cx.new(|cx| {
            snout_app::remote::picker::FolderPicker::new(h2, "devbox".into(), "/home/dev/projects".into(), window, cx)
        });
        ws.update(cx, |w, cx| {
            w.remote_picker = Some(picker);
            cx.notify();
        });
    });
    let ws = w.ws.clone();
    w.pump(30, &mut |cx| {
        ws.read(cx).remote_picker.as_ref().is_some_and(|p| p.read(cx).folders.as_ref().is_some_and(|f| !f.is_empty()))
    });
    w.with(|ws, _, cx| {
        let p = ws.read(cx).remote_picker.clone().unwrap();
        p.update(cx, |p, cx| {
            p.selected = Some("shop".into());
            cx.notify();
        });
    });
    w.pump(1, &mut |_| false);
    w.snap("remote-picker-light");
    w.with(|ws, _, cx| {
        ws.update(cx, |w, cx| {
            w.remote_picker = None;
            cx.notify();
        })
    });
    // The window on the remote project, as Open makes it.
    let folder = PathBuf::from("/home/dev/projects/shop");
    let Win { mut cx, .. } = w;
    let h3: snout_host::HostRef = host.clone();
    cx.update(|cx| {
        cx.set_global(snout_app::host::HostGlobal(h3));
        cx.global_mut::<Remote>().connection = Some(Connection {
            name: "devbox".into(),
            host: Some(host.clone()),
            status: snout_remote::Status::Connected,
        });
    });
    let mut s = settings(ThemeChoice::Light, vec![folder.clone()], Some(folder.clone()));
    s.project_jdks.clear();
    // The engine there, on the Java it put there.
    cx.update(|cx| cx.set_global(snout_app::lang::hub::LangEnabled(true)));
    let mut w = Win::window(cx, s);
    w.activity(Activity::Project);
    w.act(Action::Refresh);
    engine_ready(&mut w, &folder, 900);
    expand(
        &mut w,
        vec![
            snout_app::project_view::NodeId::Root,
            snout_app::project_view::NodeId::Dir(folder.join("src")),
            snout_app::project_view::NodeId::Dir(folder.join("src/main")),
            snout_app::project_view::NodeId::Dir(folder.join("src/main/java")),
            snout_app::project_view::NodeId::Dir(folder.join("src/main/java/com")),
            snout_app::project_view::NodeId::Dir(folder.join("src/main/java/com/example")),
            snout_app::project_view::NodeId::Dir(folder.join("src/main/java/com/example/shop")),
            snout_app::project_view::NodeId::Dir(folder.join("src/main/java/com/example/shop/web")),
        ],
    );
    w.open(&folder.join("src/main/java/com/example/shop/web/ProductController.java"), Some((20, 0)));
    w.pump(5, &mut |_| false);
    w.snap("remote-project-light");
    w.activity(Activity::Git);
    w.pump(5, &mut |_| false);
    w.snap("remote-git-light");
    w.activity(Activity::Project);
    w.pump(3, &mut |_| false);
    // The window while the connection comes back: the machine gone quiet (the container
    // paused) and the connection dropped under the window. The harness runs the app's
    // background work on this thread, where it would wait for the connection; so the window
    // is drawn as it is now, the status bar told the host's state as the app's poll tells
    // it, and the app runs again once the machine is back.
    sh(here, &["docker", "pause", &name]);
    host.drop_connection();
    let status = host.status();
    assert!(matches!(status, snout_remote::Status::Reconnecting { .. }), "reconnecting: {status:?}");
    w.cx.update(|cx| {
        if let Some(c) = cx.global_mut::<Remote>().connection.as_mut() {
            c.status = status;
        }
        cx.refresh_windows();
    });
    w.draw_and_snap("remote-reconnecting-light");
    sh(here, &["docker", "unpause", &name]);
    let h5 = host.clone();
    assert!(w.pump(150, &mut |_| h5.status() == snout_remote::Status::Connected), "connected again");
    w.pump(3, &mut |_| false);
    w.shutdown();
    host.close();
    drop(container);
    let _ = std::fs::remove_dir_all(&ssh_dir);
}

// ----- Annotate with Git Blame -------------------------------------------------------------

const STOCK: &str = "src/main/java/com/example/inventory/Stock.java";

const STOCK_V1: &str = "package com.example.inventory;

public class Inventory {
    private int count;

    public void add(int n) {
        count += n;
    }

    public int count() {
        return count;
    }
}
";

const STOCK_V2: &str = "package com.example.inventory;

import java.util.Objects;

public class Stock {
    private int count;
    private final String sku;

    public Stock(String sku) {
        this.sku = Objects.requireNonNull(sku);
    }

    public void add(int n) {
        count += n;
    }

    public int count() {
        return count;
    }
}
";

const STOCK_V3_REMOVE: &str = "
    public boolean remove(int n) {
        if (n > count) {
            return false;
        }
        count -= n;
        return true;
    }
";

const STOCK_V5_TOSTRING: &str = "
    @Override
    public String toString() {
        return sku + \": \" + count + \" in stock\";
    }
";

/// The inventory demo: one Java file four people wrote over five years (Maya Chen started
/// it as Inventory.java; Tomás Rivera renamed it and keyed it by SKU; Priya Natarajan added
/// `remove`; Sam Okafor refused negative amounts; Maya came back for `toString`).
fn inventory() -> PathBuf {
    let dir = home().join("projects/inventory");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src/main/java/com/example/inventory")).unwrap();
    let old = "src/main/java/com/example/inventory/Inventory.java";
    let commit = |who: (&str, &str), date: &str, args: &[&str]| {
        let ok = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_AUTHOR_NAME", who.0)
            .env("GIT_AUTHOR_EMAIL", who.1)
            .env("GIT_COMMITTER_NAME", who.0)
            .env("GIT_COMMITTER_EMAIL", who.1)
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .status()
            .unwrap()
            .success();
        assert!(ok, "{args:?}");
    };
    let maya = ("Maya Chen", "maya@example.com");
    let tomas = ("Tom\u{e1}s Rivera", "tomas@example.com");
    let priya = ("Priya Natarajan", "priya@example.com");
    let sam = ("Sam Okafor", "sam@example.com");
    commit(maya, "2021-03-02T10:00:00", &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join(old), STOCK_V1).unwrap();
    commit(maya, "2021-03-02T10:00:00", &["add", "-A"]);
    commit(
        maya,
        "2021-03-02T10:00:00",
        &[
            "commit",
            "-qm",
            "Count what is in stock\n\nA first inventory: one count, a way to add to it and to read it.",
        ],
    );
    commit(tomas, "2023-06-14T15:30:00", &["mv", old, STOCK]);
    std::fs::write(dir.join(STOCK), STOCK_V1.replace("class Inventory", "class Stock")).unwrap();
    commit(tomas, "2023-06-14T15:30:00", &["commit", "-qam", "Rename Inventory to Stock"]);
    std::fs::write(dir.join(STOCK), STOCK_V2).unwrap();
    commit(
        tomas,
        "2023-06-15T11:05:00",
        &["commit", "-qam", "Key stock by SKU\n\nEach stock item now knows its SKU; a null one is refused at once."],
    );
    let v3 = STOCK_V2
        .replace("        return count;\n    }\n", &format!("        return count;\n    }}\n{STOCK_V3_REMOVE}"));
    std::fs::write(dir.join(STOCK), &v3).unwrap();
    commit(
        priya,
        "2025-01-20T09:12:00",
        &[
            "commit",
            "-qam",
            "Take items out of stock\n\nremove() says whether there was enough to take, and leaves the count alone when not.",
        ],
    );
    let v4 = v3.replace(
        "    public void add(int n) {\n",
        "    public void add(int n) {\n        if (n < 0) {\n            throw new IllegalArgumentException(\"Negative amount: \" + n);\n        }\n",
    );
    std::fs::write(dir.join(STOCK), &v4).unwrap();
    commit(
        sam,
        "2026-09-30T17:45:00",
        &[
            "commit",
            "-qam",
            "Refuse negative amounts in add\n\nA negative amount was taken as a removal that never checked the count.\nNow it is an error, and remove() is the way to take items out.",
        ],
    );
    let v5 = v4.trim_end().strip_suffix('}').unwrap().to_owned() + STOCK_V5_TOSTRING + "}\n";
    std::fs::write(dir.join(STOCK), &v5).unwrap();
    commit(maya, "2026-10-26T14:20:00", &["commit", "-qam", "Describe stock in the logs"]);
    dir
}

/// The pointer resting on the annotation column's line `line`, until its card shows.
fn hover_column(w: &mut Win, e: &Entity<EditorPanel>, line: usize) {
    let e2 = e.clone();
    let at = w
        .cx
        .update(|cx| e2.read(cx).blame.painted.borrow().rows.iter().find(|(l, _)| *l == line).map(|(_, b)| b.center()));
    let at = at.expect("the line is drawn");
    for _ in 0..30 {
        w.with(|_, window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                    position: at,
                    pressed_button: None,
                    modifiers: gpui::Modifiers::none(),
                }),
                cx,
            );
        });
        let e3 = e.clone();
        if w.pump(1, &mut |cx| e3.read(cx).blame.card.is_some()) {
            return;
        }
    }
    println!("docs: no blame card on line {line}");
}

fn blame(theme: ThemeChoice, mode: &str) {
    let repo = inventory();
    let file = repo.join(STOCK);
    let mut s = settings(theme, vec![repo.clone()], Some(repo.clone()));
    s.blame_current_line = true;
    s.hover_delay_ms = 200;
    let mut w = Win::new(s, false);
    w.activity(Activity::Project);
    let e = w.open(&file, Some((19, 8)));
    let e2 = e.clone();
    w.with(|_, _, cx| e2.update(cx, |e, cx| e.toggle_blame(cx)));
    let e3 = e.clone();
    w.pump(20, &mut |cx| e3.read(cx).blame.data.is_some() && !e3.read(cx).blame.map.is_empty());
    w.settle();
    w.snap(&format!("blame-column-{mode}"));
    if mode != "light" {
        return;
    }
    // A line's card.
    hover_column(&mut w, &e, 24);
    w.settle();
    w.snap("blame-card-light");
    // A click on a line: its commit lit.
    let e2 = e.clone();
    w.cx.update(|cx| {
        e2.update(cx, |e, cx| {
            e.blame.card = None;
            e.blame.selected = e.blame_commit(24).map(|c| c.id.clone());
            cx.notify();
        })
    });
    w.settle();
    w.snap("blame-highlight-light");
    // Annotate Previous Revision: the file before Priya's commit.
    let e2 = e.clone();
    let priya = w.cx.update(|cx| e2.read(cx).blame_commit(24).cloned()).unwrap();
    w.with(|_, window, cx| {
        e2.update(cx, |e, cx| {
            e.blame.selected = None;
            e.annotate_previous(&priya, window, cx)
        })
    });
    let e3 = e.clone();
    w.pump(20, &mut |cx| e3.read(cx).blame.revisions.last().is_some_and(|r| r.data.is_some()));
    w.settle();
    w.snap("blame-previous-light");
    let e2 = e.clone();
    w.cx.update(|cx| e2.update(cx, |e, cx| e.revision_back(0, cx)));
    // The column closed: the author hints, and the caret line's blame.
    w.cx.update(|cx| e2.update(cx, |e, cx| e.set_annotations(false, cx)));
    w.with(|_, window, cx| {
        e2.update(cx, |e, cx| {
            e.go_to(14, 12, window, cx);
        })
    });
    let e3 = e.clone();
    w.pump(5, &mut |cx| e3.read(cx).blame.line == Some(14));
    w.settle();
    w.snap("blame-hints-light");
    // An agent's lines, not committed yet, and one of yours: the agent in its colour.
    let text = w.text(&e);
    let agent_text = text.replace(
        "        count -= n;\n",
        "        count -= n;\n        if (count == 0) {\n            log.info(\"{} is out of stock\", sku);\n        }\n",
    );
    let mine = agent_text.replace("    private int count;\n", "    private int count; // never negative\n");
    w.set_text(&e, mine.clone());
    let e2 = e.clone();
    w.cx.update(|cx| {
        e2.update(cx, |e, cx| {
            use snout_app::claude_ide::ledger::{EditRecord, Owners};
            let first = mine.lines().position(|l| l.contains("if (count == 0)")).unwrap();
            let mut lines = vec![None; mine.lines().count() + 1];
            for l in &mut lines[first..first + 3] {
                *l = Some(0);
            }
            e.agent_edits = Arc::new(vec![EditRecord::now(
                snout_app::agents::CLAUDE,
                1,
                Some("Log when an item runs out".into()),
                "Edit",
            )]);
            e.agent_lines = Some(Owners { text: Arc::new(mine.clone()), lines: Arc::new(lines) });
            e.set_annotations(true, cx);
        })
    });
    w.pump(2, &mut |_| false);
    w.with(|_, window, cx| e2.update(cx, |e, cx| e.go_to(27, 0, window, cx)));
    w.settle();
    w.snap("blame-agents-light");
}
