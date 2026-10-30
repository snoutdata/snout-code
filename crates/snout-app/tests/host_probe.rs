//! Which host calls the UI thread still makes: the real app (real windows, real threads, the
//! Java engine when it is built) on a host slowed down as a remote one is
//! (`snout_host::LatencyHost`), driven through opening a Gradle project, opening, editing and
//! saving files, a change on disk, Find References, and the tabs coming back in a new
//! window. Each host call made on the UI thread is printed; the probe fails if there is one.
//!
//! It opens windows, so it runs only when asked:
//!
//! ```sh
//! SNOUT_HOST_PROBE=1 cargo test -p snout-app --test host_probe
//! ```
//!
//! `SNOUT_HOST_LATENCY_MS` sets the delay (default 200). The app's settings go to a scratch
//! folder.

#[cfg(target_os = "macos")]
fn main() {
    if std::env::var_os("SNOUT_HOST_PROBE").is_none() {
        println!("host probe: skipped; set SNOUT_HOST_PROBE=1 to run it (it opens windows)");
        return;
    }
    probe::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("host probe: skipped on this platform");
}

#[cfg(target_os = "macos")]
mod probe {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use gpui::{AppContext as _, AsyncApp, Bounds, Entity, WindowBounds, WindowOptions, px, size};
    use snout_app::editor::EditorPanel;
    use snout_app::model::{Action, Model};
    use snout_app::settings::{Settings, Store};
    use snout_app::workspace::Workspace;
    use snout_host::LatencyHost;

    const APP: &str = "package demo;\n\npublic class App {\n    public static void main(String[] args) {\n        System.out.println(Greeter.greet(\"probe\"));\n    }\n}\n";
    const GREETER: &str = "package demo;\n\npublic final class Greeter {\n    public static String greet(String name) {\n        return \"Hello, \" + name;\n    }\n}\n";

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success();
        assert!(ok, "git {args:?}");
    }

    /// A small Gradle build in a Git repository.
    fn project(root: &Path) -> PathBuf {
        let dir = root.join("probe-app");
        let src = dir.join("src/main/java/demo");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(dir.join("settings.gradle"), "rootProject.name = 'probe-app'\n").unwrap();
        std::fs::write(
            dir.join("build.gradle"),
            "plugins { id 'application' }\napplication { mainClass = 'demo.App' }\n",
        )
        .unwrap();
        std::fs::write(src.join("App.java"), APP).unwrap();
        std::fs::write(src.join("Greeter.java"), GREETER).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["-c", "user.name=Probe", "-c", "user.email=probe@example.com", "add", "-A"]);
        git(&dir, &["-c", "user.name=Probe", "-c", "user.email=probe@example.com", "commit", "-qm", "Start"]);
        snout_host::canonical(&dir).unwrap()
    }

    struct Probe {
        host: Arc<LatencyHost>,
        counted: u64,
        failed: Vec<String>,
    }

    impl Probe {
        /// The UI thread's calls since the last phase, under `name`.
        fn phase(&mut self, name: &str) {
            let now = self.host.ui_calls();
            let n = now - self.counted;
            self.counted = now;
            println!("host probe: {name}: {n} host call(s) on the UI thread");
            if n > 0 {
                self.failed.push(format!("{name}: {n}"));
            }
        }
    }

    async fn wait(cx: &mut AsyncApp, ms: u64) {
        cx.background_executor().timer(Duration::from_millis(ms)).await;
    }

    /// Wait (up to `secs`) until `f` holds.
    async fn until(cx: &mut AsyncApp, secs: u64, mut f: impl FnMut(&mut gpui::App) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(secs) {
            if cx.update(&mut f) {
                return true;
            }
            wait(cx, 100).await;
        }
        false
    }

    fn editor_of(ws: &Entity<Workspace>, path: &Path, cx: &gpui::App) -> Option<Entity<EditorPanel>> {
        ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path)
    }

    fn open_window(model: Entity<Model>, cx: &mut gpui::App) -> (gpui::AnyWindowHandle, Entity<Workspace>) {
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        let mut ws = None;
        let handle = cx
            .open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                |window, cx| {
                    let view = cx.new(|cx| Workspace::new(model, window, cx));
                    ws = Some(view.clone());
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                },
            )
            .expect("a window");
        (handle.into(), ws.expect("the workspace"))
    }

    static CODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(2);

    pub fn run() {
        let ms = std::env::var(snout_host::LATENCY_ENV).ok().and_then(|v| v.parse().ok()).unwrap_or(200);
        let scratch = tempfile::Builder::new().prefix("snout-probe").tempdir().unwrap();
        let dir = project(&snout_host::canonical(scratch.path()).unwrap());
        let store = Store::in_home(&scratch.path().join("home"));
        gpui_platform::application().with_assets(gpui_kit_assets::AllAssets).run(move |cx: &mut gpui::App| {
            // This thread is the UI thread.
            let host = Arc::new(LatencyHost::new(snout_host::local(), Duration::from_millis(ms)).ui_thread_here());
            cx.set_global(snout_app::host::HostGlobal(host.clone()));
            let settings = Settings { repos: vec![dir.clone()], current: Some(dir.clone()), ..Settings::default() };
            snout_app::init(&settings, cx);
            cx.set_global(snout_app::lang::hub::LangEnabled(true));
            let engine = snout_app::lang::java::engine_dist(&*snout_host::local()).is_some();
            let model = cx.new(|cx| Model::new(settings, Some(store), cx));
            let (_, ws) = open_window(model.clone(), cx);
            let mut probe = Probe { host, counted: 0, failed: Vec::new() };
            cx.spawn(async move |cx| {
                let app = dir.join("src/main/java/demo/App.java");
                let greeter = dir.join("src/main/java/demo/Greeter.java");
                // Opening the project: the window, the Git panel, the tree, the models.
                wait(cx, 4000).await;
                probe.phase("open the project");

                // Open a file, type, save.
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(app.clone(), Some((4, 30))), cx)));
                let ws2 = ws.clone();
                let a = app.clone();
                let loaded =
                    until(cx, 30, move |cx| editor_of(&ws2, &a, cx).is_some_and(|e| !e.read(cx).loading)).await;
                assert!(loaded, "App.java opens");
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(greeter.clone(), None), cx)));
                wait(cx, 2000).await;
                probe.phase("open files");
                let editor = cx.update(|cx| editor_of(&ws, &app, cx)).expect("App.java");
                let save = cx.update(|cx| {
                    let window = cx.windows()[0];
                    window
                        .update(cx, |_, window, cx| {
                            editor.update(cx, |e, cx| {
                                let text = e.text(cx).replace("probe", "probed");
                                e.state.update(cx, |s, cx| s.set_value(text, window, cx));
                                e.save(cx)
                            })
                        })
                        .expect("the window")
                });
                assert!(save.await, "App.java saves");
                assert!(std::fs::read_to_string(&app).unwrap().contains("probed"));
                probe.phase("edit and save");

                // A change on disk: the clean tab takes it.
                wait(cx, 1100).await;
                std::fs::write(&app, APP.replace("probe", "on disk")).unwrap();
                let e = editor.clone();
                let reloaded = until(cx, 20, move |cx| e.read(cx).text(cx).contains("on disk")).await;
                assert!(reloaded, "the tab takes the change on disk");
                probe.phase("a change on disk");

                // Find References, once the engine answers.
                if engine {
                    let hub = cx.update(|cx| ws.read(cx).hub.clone());
                    let d = dir.clone();
                    let ready = until(cx, 300, move |cx| {
                        hub.read(cx).servers.get(&d).is_some_and(|s| s.first_diagnostics.is_some())
                    })
                    .await;
                    probe.phase("the engine starts");
                    if ready {
                        let g = cx.update(|cx| editor_of(&ws, &greeter, cx)).expect("Greeter.java");
                        cx.update(|cx| {
                            let window = cx.windows()[0];
                            let _ = window.update(cx, |_, window, cx| {
                                g.update(cx, |e, cx| {
                                    e.go_to(3, 26, window, cx);
                                    e.find_references(cx);
                                })
                            });
                        });
                        let w = ws.clone();
                        let found = until(cx, 120, move |cx| !w.read(cx).references.read(cx).is_empty()).await;
                        wait(cx, 1500).await;
                        println!("host probe: Find References found {}", if found { "references" } else { "none" });
                        probe.phase("find references");
                    } else {
                        println!("host probe: the engine did not answer; Find References not tried");
                    }
                } else {
                    println!("host probe: no engine built (engine/build/dist); Find References not tried");
                }

                // The tabs come back: a new window restores the saved layout.
                cx.update(|cx| ws.update(cx, |ws, cx| ws.save_layout(cx)));
                let (_, ws2) = cx.update(|cx| open_window(model.clone(), cx));
                let a = app.clone();
                let back = until(cx, 30, move |cx| editor_of(&ws2, &a, cx).is_some_and(|e| !e.read(cx).loading)).await;
                assert!(back, "the tabs come back");
                wait(cx, 1500).await;
                probe.phase("restore the tabs");

                if probe.failed.is_empty() {
                    println!("host probe: no host call on the UI thread");
                } else {
                    println!("host probe: FAILED: {}", probe.failed.join(", "));
                }
                CODE.store(i32::from(!probe.failed.is_empty()), std::sync::atomic::Ordering::SeqCst);
                // Quitting stops the engine (and what it started) first.
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
        std::process::exit(CODE.load(std::sync::atomic::Ordering::SeqCst));
    }
}
