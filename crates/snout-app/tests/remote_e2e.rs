//! Remote SSH end to end, in the real app: an Ubuntu container with sshd, git, no JDK and a
//! small Gradle project (`crates/snout-server/tests/docker`), reached with the system's ssh, a
//! scratch key and a scratch ssh config (`SNOUT_SSH_CONFIG`); the app's settings in a scratch
//! folder. The app is driven as a user would: Connect to SSH Host, the folder picker, the
//! project opens over the remote host, Java diagnostics and completion from the engine there,
//! its main class and its tests run, the debugger stops at a breakpoint, a terminal runs there;
//! Claude Code there finds the window (the lock file in that machine's `~/.claude/ide`, the IDE
//! server reached through a port of that machine) and its hooks report an edit, which the
//! window plays out and reverts; then the ssh processes on this Mac are killed and the app
//! reconnects, the engine still the same process and the hooks' port still leading back;
//! Close Remote Connection brings the window back to this Mac.
//!
//! It opens windows and needs Docker and the network, so it runs only when asked:
//!
//! ```sh
//! SNOUT_REMOTE_E2E=1 cargo test -p snout-app --test remote_e2e
//! ```

#[cfg(target_os = "macos")]
fn main() {
    // ssh runs this program for its questions (the app's askpass is its own program): none
    // is expected here.
    if std::env::var_os(snout_remote::ASKPASS_ENV).is_some() {
        eprintln!("remote e2e: ssh asked: {:?}", std::env::args().nth(1));
        std::process::exit(1);
    }
    if std::env::var_os("SNOUT_REMOTE_E2E").is_none() {
        println!("remote e2e: skipped; set SNOUT_REMOTE_E2E=1 to run it (Docker, the network, windows)");
        return;
    }
    e2e::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("remote e2e: skipped on this platform");
}

#[cfg(target_os = "macos")]
mod e2e {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::time::{Duration, Instant};

    use gpui::{AppContext as _, AsyncApp, Entity};
    use snout_app::editor::EditorPanel;
    use snout_app::model::{Action, Model};
    use snout_app::run::Launch;
    use snout_app::settings::{Settings, Store};
    use snout_app::workspace::Workspace;
    use snout_remote::Status;

    const IMAGE: &str = "snout-remote-test:2";
    const PROJECT: &str = "/home/dev/demo";
    static CODE: AtomicI32 = AtomicI32::new(2);

    fn workspace_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
    }

    fn sh(cmd: &mut Command) -> String {
        let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
        assert!(out.status.success(), "{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn say(what: &str) {
        println!("remote e2e: {what}");
    }

    async fn wait(cx: &mut AsyncApp, ms: u64) {
        cx.background_executor().timer(Duration::from_millis(ms)).await;
    }

    /// Wait (up to `secs`) until `f` holds; fail with `what` otherwise.
    async fn until(cx: &mut AsyncApp, secs: u64, what: &str, mut f: impl FnMut(&mut gpui::App) -> bool) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(secs) {
            if cx.update(&mut f) {
                say(&format!("{what} ({:.1} s)", start.elapsed().as_secs_f32()));
                return;
            }
            wait(cx, 200).await;
        }
        fail(&format!("timed out: {what}"));
    }

    /// The container, removed however the run ends.
    static CONTAINER: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    fn fail(why: &str) -> ! {
        println!("remote e2e: FAILED: {why}");
        if let Some(name) = CONTAINER.get() {
            let _ = Command::new("docker").args(["rm", "-f", name]).output();
        }
        std::process::exit(1);
    }

    /// The workspace of the window open now (the last one).
    fn workspace(cx: &mut gpui::App) -> Option<Entity<Workspace>> {
        let window = *cx.windows().last()?;
        window
            .update(cx, |_, window, _| window.root::<gpui_component::Root>())
            .ok()
            .flatten()
            .flatten()
            .and_then(|root| root.read(cx).view().clone().downcast::<Workspace>().ok())
    }

    fn editor_of(ws: &Entity<Workspace>, path: &Path, cx: &gpui::App) -> Option<Entity<EditorPanel>> {
        ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path)
    }

    fn in_window(cx: &mut gpui::App, f: impl FnOnce(&mut gpui::Window, &mut gpui::App)) {
        let window = *cx.windows().last().expect("a window");
        let _ = window.update(cx, |_, window, cx| f(window, cx));
    }

    fn output(ws: &Entity<Workspace>, id: u64, cx: &gpui::App) -> String {
        let runs = ws.read(cx).runs.read(cx);
        runs.session(id).map(|s| s.lines.iter().map(|l| l.text()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
    }

    fn ended(ws: &Entity<Workspace>, id: u64, cx: &gpui::App) -> bool {
        ws.read(cx).runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())
    }

    /// The engine's processes on the remote machine.
    fn engine_pids(cx: &gpui::App) -> Vec<u32> {
        let host = snout_app::host::host(cx);
        host.processes(false)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.cmd.iter().any(|a| a.contains("SnoutEngine")) && p.cmd.iter().any(|a| a == "lsp"))
            .map(|p| p.pid)
            .collect()
    }

    struct Container(String);

    impl Drop for Container {
        fn drop(&mut self) {
            let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
        }
    }

    pub fn run() {
        let scratch = tempfile::Builder::new().prefix("snt").tempdir().unwrap();
        let dir = scratch.path().canonicalize().unwrap();
        let key = dir.join("key");
        sh(Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", "snout-e2e", "-f"]).arg(&key));
        let public = std::fs::read_to_string(dir.join("key.pub")).unwrap();
        sh(Command::new("docker")
            .args(["build", "-q", "-t", IMAGE])
            .arg(workspace_dir().join("crates/snout-server/tests/docker")));
        let name = format!("snout-e2e-{}", std::process::id());
        let container = Container(name.clone());
        let _ = CONTAINER.set(name.clone());
        sh(Command::new("docker")
            .args(["run", "-d", "--rm", "--name", &name, "-p", "127.0.0.1::22", "-e"])
            .arg(format!("AUTHORIZED_KEY={}", public.trim()))
            .arg(IMAGE));
        let port = sh(Command::new("docker").args(["port", &name, "22"]));
        let port = port.lines().next().unwrap().rsplit(':').next().unwrap().to_owned();
        let config = dir.join("ssh_config");
        std::fs::write(
            &config,
            format!(
                "Host devbox\n  HostName 127.0.0.1\n  Port {port}\n  User dev\n  IdentityFile {}\n  \
                 IdentitiesOnly yes\n  IdentityAgent none\n  StrictHostKeyChecking no\n  \
                 UserKnownHostsFile /dev/null\n  LogLevel ERROR\n",
                key.display()
            ),
        )
        .unwrap();
        let control = dir.join("c");
        // SAFETY: set before the app and its threads start.
        unsafe {
            std::env::set_var(snout_app::remote::SSH_CONFIG_ENV, &config);
            std::env::set_var("SNOUT_SSH_CONTROL_DIR", &control);
            std::env::set_var("SNOUT_NO_FETCH", "1");
        }
        let start = Instant::now();
        while !Command::new("ssh").arg("-F").arg(&config).args(["devbox", "true"]).status().is_ok_and(|s| s.success()) {
            assert!(start.elapsed() < Duration::from_secs(30), "sshd did not come up");
            std::thread::sleep(Duration::from_millis(200));
        }
        let store = Store::in_home(&dir.join("home"));
        let ide_dir = dir.join("ide");

        gpui_platform::application().with_assets(gpui_kit_assets::AllAssets).run(move |cx: &mut gpui::App| {
            cx.set_global(snout_app::host::HostGlobal(snout_host::local()));
            let settings = Settings::default();
            snout_app::init(&settings, cx);
            cx.set_global(snout_app::lang::hub::LangEnabled(true));
            // Claude Code's server, as the app runs it (its lock files on this Mac in scratch).
            cx.set_global(snout_app::claude_ide::IdeConfig { lock_dir: ide_dir.clone() });
            snout_app::remote::init(Some(store.clone()), cx);
            let model = cx.new(|cx| Model::new(store.load(), Some(store.clone()), cx));
            snout_app::open_window(model, None, cx).expect("a window");
            let control = control.clone();
            cx.spawn(async move |cx| {
                script(cx, &control).await;
                CODE.store(0, Ordering::SeqCst);
                // Quitting may end the process here: the container goes first.
                if let Some(name) = CONTAINER.get() {
                    let _ = Command::new("docker").args(["rm", "-f", name]).output();
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
        drop(container);
        std::process::exit(CODE.load(Ordering::SeqCst));
    }

    async fn script(cx: &mut AsyncApp, control: &Path) {
        wait(cx, 1500).await;
        let project = PathBuf::from(PROJECT);
        let app = project.join("src/main/java/demo/App.java");

        // File ▸ Connect to SSH Host…, devbox.
        cx.update(|cx| in_window(cx, |w, cx| w.dispatch_action(Box::new(snout_app::menus::ConnectSsh), cx)));
        until(cx, 10, "the Connect dialog shows", |cx| {
            workspace(cx).is_some_and(|ws| ws.read(cx).connect_dialog.is_some())
        })
        .await;
        cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let dialog = ws.read(cx).connect_dialog.clone().unwrap();
            in_window(cx, |w, cx| {
                dialog.update(cx, |d, cx| {
                    d.input.update(cx, |i, cx| i.set_value("devbox", w, cx));
                    d.connect(cx);
                })
            });
        });
        until(cx, 240, "connected; the folder picker shows", |cx| {
            workspace(cx).is_some_and(|ws| ws.read(cx).remote_picker.is_some())
        })
        .await;
        // The picker browses the remote machine: its home holds the project.
        until(cx, 30, "the picker lists the remote home", |cx| {
            let ws = workspace(cx).unwrap();
            let picker = ws.read(cx).remote_picker.clone().unwrap();
            picker.read(cx).folders.as_ref().is_some_and(|f| f.iter().any(|n| n == "demo"))
        })
        .await;
        cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let picker = ws.read(cx).remote_picker.clone().unwrap();
            picker.update(cx, |p, cx| p.open(PathBuf::from(PROJECT), cx));
        });
        until(cx, 60, "the window opens again on the remote project", |cx| {
            workspace(cx)
                .is_some_and(|ws| ws.read(cx).model.read(cx).project_root().as_deref() == Some(Path::new(PROJECT)))
        })
        .await;
        cx.update(|cx| {
            let host = snout_app::host::host(cx);
            assert_eq!(host.info().os, "linux", "the workspace's host is the remote machine");
            assert_eq!(snout_app::remote::connection(cx).map(|c| c.0).as_deref(), Some("devbox"));
        });

        // A Java file: the engine (installed there, on the Java it downloads) answers.
        cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let model = ws.read(cx).model.clone();
            model.update(cx, |m, cx| m.handle(Action::OpenFile(app.clone(), None), cx));
        });
        let a = app.clone();
        until(cx, 30, "App.java opens", move |cx| {
            workspace(cx).and_then(|ws| editor_of(&ws, &a, cx)).is_some_and(|e| !e.read(cx).loading)
        })
        .await;
        let p = project.clone();
        until(cx, 900, "the engine on the remote answers (first diagnostics)", move |cx| {
            let ws = workspace(cx).unwrap();
            ws.read(cx).hub.read(cx).servers.get(&p).is_some_and(|s| s.first_diagnostics.is_some())
        })
        .await;

        // Diagnostics: a type error is reported, and goes when it is fixed.
        let editor = cx.update(|cx| editor_of(&workspace(cx).unwrap(), &app, cx).unwrap());
        let original = cx.update(|cx| editor.read(cx).text(cx));
        // Typed, as the editor sends its changes to the engine.
        let at = original.find("int total = 0;").unwrap() + "int total = ".len();
        let edit = |cx: &mut AsyncApp, range: std::ops::Range<usize>, text: &'static str| {
            let editor = editor.clone();
            cx.update(|cx| {
                in_window(cx, |w, cx| {
                    editor.update(cx, |e, cx| {
                        e.state.update(cx, |s, cx| {
                            s.set_selected_range(range, cx);
                            s.replace(text, w, cx);
                        })
                    })
                })
            });
        };
        edit(cx, at..at + 1, "\"zero\"");
        let e = editor.clone();
        until(cx, 120, "the type error is reported", move |cx| !e.read(cx).problems.is_empty()).await;
        let message = cx.update(|cx| editor.read(cx).problems[0].message.clone());
        say(&format!("diagnostic: {message}"));
        edit(cx, at..at + 6, "0");
        let fixed = cx.update(|cx| editor.read(cx).text(cx));
        assert_eq!(fixed, original, "the text is as it was");
        let e = editor.clone();
        until(cx, 120, "the error goes once fixed", move |cx| e.read(cx).problems.is_empty()).await;

        // Completion after `Greeter.`.
        let line = original.lines().position(|l| l.contains("Greeter.greet(")).unwrap() as u32;
        let character = original.lines().nth(line as usize).unwrap().find("Greeter.").unwrap() as u32 + 8;
        let ask = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let uri = snout_lang::uri::from_path(&app);
            let params = serde_json::json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": line, "character": character },
            });
            ws.read(cx).hub.read(cx).request(&project, "textDocument/completion", params)
        });
        let answer = ask.await.unwrap_or_else(|e| fail(&format!("completion: {e}")));
        let text = answer.to_string();
        if !text.contains("greet") || !text.contains("add") {
            fail(&format!("completion lacks greet and add: {}", &text[..text.len().min(400)]));
        }
        say("completion offers greet and add");

        // The configurations come: run the main class, then the tests.
        until(cx, 600, "the run configurations come", |cx| {
            let ws = workspace(cx).unwrap();
            !ws.read(cx).runs.read(cx).engine_configs.is_empty()
        })
        .await;
        let configs = cx.update(|cx| workspace(cx).unwrap().read(cx).runs.read(cx).configurations());
        let main = configs
            .iter()
            .find(|c| c.kind == snout_app::run::config::Kind::Main)
            .cloned()
            .unwrap_or_else(|| fail(&format!("no main configuration: {configs:?}")));
        let run_id = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let runs = ws.read(cx).runs.clone();
            runs.update(cx, |r, cx| r.start(Launch::Config { config: main.clone(), debug: false }, cx))
        });
        let run_id = run_id.unwrap_or_else(|| fail("the main class did not start"));
        until(cx, 600, "the main class runs to its end", move |cx| ended(&workspace(cx).unwrap(), run_id, cx)).await;
        let out = cx.update(|cx| output(&workspace(cx).unwrap(), run_id, cx));
        if !out.contains("Hello, remote total 6") {
            fail(&format!("the main class printed: {out}"));
        }
        say("the main class printed: Hello, remote total 6");

        let tests = configs
            .iter()
            .find(|c| c.kind == snout_app::run::config::Kind::Tests)
            .cloned()
            .unwrap_or_else(|| fail(&format!("no test configuration: {configs:?}")));
        let test_id = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let runs = ws.read(cx).runs.clone();
            runs.update(cx, |r, cx| r.start(Launch::Config { config: tests.clone(), debug: false }, cx))
        });
        let test_id = test_id.unwrap_or_else(|| fail("the tests did not start"));
        until(cx, 600, "the tests run to their end", move |cx| ended(&workspace(cx).unwrap(), test_id, cx)).await;
        let counts = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let runs = ws.read(cx).runs.read(cx);
            runs.session(test_id).and_then(|s| s.tests.as_ref().map(|t| t.counts()))
        });
        match counts {
            Some((passed, failed, ..)) if passed == 2 && failed == 0 => say("2 tests passed"),
            other => {
                fail(&format!("the tests: {other:?}\n{}", cx.update(|cx| output(&workspace(cx).unwrap(), test_id, cx))))
            }
        }

        // The debugger, through a tunnel to the remote JVM, stops at a breakpoint.
        let bp = original.lines().position(|l| l.contains("total = Greeter.add(total, i);")).unwrap() as u32;
        let debug_id = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let runs = ws.read(cx).runs.clone();
            runs.update(cx, |r, cx| {
                r.toggle_breakpoint(&app, bp, cx);
                r.start(Launch::Config { config: main.clone(), debug: true }, cx)
            })
        });
        let debug_id = debug_id.unwrap_or_else(|| fail("the debugger did not start"));
        until(cx, 600, "the debugger stops at the breakpoint", |cx| {
            snout_app::run::marks(cx).is_some_and(|m| m.read(cx).exec.is_some() && !m.read(cx).locals.is_empty())
        })
        .await;
        cx.update(|cx| {
            let m = snout_app::run::marks(cx).unwrap();
            let m = m.read(cx);
            say(&format!("stopped at {:?}, locals {:?}", m.exec, m.locals));
            if m.exec.as_ref().map(|(_, l)| *l) != Some(bp) || !m.locals.contains(&("i".into(), "1".into())) {
                fail("not at the breakpoint with i = 1");
            }
        });
        cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let runs = ws.read(cx).runs.clone();
            runs.update(cx, |r, cx| {
                r.toggle_breakpoint(&app, bp, cx);
                r.step(debug_id, "continue", cx);
            })
        });
        until(cx, 120, "the debugged program ends", move |cx| ended(&workspace(cx).unwrap(), debug_id, cx)).await;

        // A terminal on the remote machine.
        cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            in_window(cx, |w, cx| {
                ws.update(cx, |ws, cx| {
                    ws.show_activity(snout_app::panels::Activity::Project, w, cx);
                    ws.show_bottom(ws.tabs.terminal, w, cx);
                })
            });
        });
        let terminal = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let ws = ws.read(cx);
            let view = ws.dock.read(cx).panel(ws.tabs.terminal).expect("the terminal").clone();
            view.view().downcast::<snout_app::console::ConsolePanel>().unwrap()
        });
        // As the tab does when it shows (the window may not be drawn when it is behind others).
        cx.update(|cx| {
            if terminal.read(cx).session_text(&project).is_none() {
                in_window(cx, |w, cx| terminal.update(cx, |t, cx| t.start(&project, &[], w, cx)));
            }
        });
        let start = Instant::now();
        loop {
            let (text, error) = cx
                .update(|cx| (terminal.read(cx).session_text(&project), terminal.read(cx).error().map(str::to_owned)));
            if text.as_deref().is_some_and(|s| s.contains('$')) {
                say("the terminal's shell starts");
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                fail(&format!("the terminal's shell did not start: {text:?}, {error:?}"));
            }
            wait(cx, 200).await;
        }
        cx.update(|cx| terminal.read(cx).type_keys(&project, b"echo ON-$(uname -s)-$(pwd)\r"));
        let (t, p) = (terminal.clone(), project.clone());
        until(cx, 30, "the terminal runs on the remote machine", move |cx| {
            t.read(cx).session_text(&p).is_some_and(|s| s.contains("ON-Linux-/home/dev/demo"))
        })
        .await;

        let _ = claude_code_on_the_remote(cx, "Greeter.java").await;

        // The ssh processes on this Mac die (the remote machine none the wiser): the app
        // reconnects, the engine and the terminal live on.
        let before = cx.update(|cx| engine_pids(cx));
        say(&format!("the engine's pid there: {before:?}"));
        if before.is_empty() {
            fail("no engine process on the remote");
        }
        let remote = cx.update(|cx| {
            cx.global::<snout_app::remote::Remote>().connection.as_ref().and_then(|c| c.host.clone()).unwrap()
        });
        let connections = remote.connections();
        // The ssh processes here: the shared connection and its sessions.
        let pattern = control.to_string_lossy().into_owned();
        let here = Command::new("pgrep").arg("-fl").arg(&pattern).output().map(|o| o.stdout).unwrap_or_default();
        say(&format!("ssh here: {}", String::from_utf8_lossy(&here).trim().replace('\n', " | ")));
        let _ = Command::new("pkill").arg("-f").arg(&pattern).status();
        say("killed the ssh processes on this Mac");
        let r = remote.clone();
        until(cx, 150, "the app reconnects", move |cx| {
            r.connections() > connections
                && r.status() == Status::Connected
                && snout_app::remote::connection(cx).is_some_and(|(_, s)| s == Status::Connected)
        })
        .await;
        let after = cx.update(|cx| engine_pids(cx));
        if after != before {
            fail(&format!("the engine changed: {before:?} then {after:?}"));
        }
        say("the engine is the same process");
        let ask = cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            let uri = snout_lang::uri::from_path(&app);
            let params = serde_json::json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": line, "character": character },
            });
            ws.read(cx).hub.read(cx).request(&project, "textDocument/completion", params)
        });
        let again = ask.await.unwrap_or_else(|e| fail(&format!("completion after the reconnect: {e}")));
        if !again.to_string().contains("greet") {
            fail("completion after the reconnect lacks greet");
        }
        say("the engine answers after the reconnect");
        cx.update(|cx| terminal.read(cx).type_keys(&project, b"echo AFTER-$((6*7))\r"));
        let (t, p) = (terminal.clone(), project.clone());
        until(cx, 30, "the terminal answers after the reconnect", move |cx| {
            t.read(cx).session_text(&p).is_some_and(|s| s.contains("AFTER-42"))
        })
        .await;

        // The hooks' port on the remote machine still leads back to the window.
        let ports = claude_code_on_the_remote(cx, "App.java").await;

        // File ▸ Close Remote Connection: back on this Mac.
        cx.update(|cx| in_window(cx, |w, cx| w.dispatch_action(Box::new(snout_app::menus::CloseRemote), cx)));
        until(cx, 60, "the window is back on this Mac", |cx| {
            snout_app::remote::connection(cx).is_none()
                && snout_app::host::host(cx).info().os == std::env::consts::OS
                && workspace(cx).is_some_and(|ws| ws.read(cx).model.read(cx).project_root().is_none())
        })
        .await;
        // Claude Code's lock file and ports on the remote machine went with the connection.
        let check = format!(
            "ls $HOME/.claude/ide/ | grep -c lock; for p in {} {}; do nc -z 127.0.0.1 $p && echo open; done; true",
            ports.0, ports.1
        );
        let start = Instant::now();
        loop {
            let left = in_container(&check, &[], "");
            if left.trim() == "0" {
                break;
            }
            if start.elapsed() > Duration::from_secs(20) {
                fail(&format!("Claude Code's lock file or ports are left on the remote machine: {left}"));
            }
            wait(cx, 200).await;
        }
        say("Claude Code's lock file and ports went with the connection");
        let recent = cx.update(|cx| snout_app::remote::recents(cx));
        if !recent.projects.iter().any(|p| p.host == "devbox" && p.path == Path::new(PROJECT)) {
            fail(&format!("the remote project is not among the recent ones: {recent:?}"));
        }
        say("PASSED");
    }

    /// Run `cmd` in the container as the user `dev`, `input` on its stdin.
    fn in_container(cmd: &str, env: &[(&str, String)], input: &str) -> String {
        use std::io::Write as _;
        let name = CONTAINER.get().cloned().unwrap_or_default();
        let mut c = Command::new("docker");
        c.args(["exec", "-i", "-u", "dev"]);
        for (k, v) in env {
            c.arg("-e").arg(format!("{k}={v}"));
        }
        let mut child = c
            .args([&name, "sh", "-c", cmd])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        if !out.status.success() {
            fail(&format!("in the container, {cmd}: {}", out.status));
        }
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Claude Code on the remote machine, faked: it finds the window by the lock file there,
    /// calls a tool over the port of that machine that leads to the IDE server, and its hooks
    /// report an edit of `file` (a file of the demo's sources), which the window plays out
    /// and lists; Revert All puts the file back on the remote machine.
    async fn claude_code_on_the_remote(cx: &mut AsyncApp, file: &str) -> (String, String) {
        use snout_app::claude_ide::{IdeArgs, IdeEnv, IdeStatus, bridge};
        let project = PathBuf::from(PROJECT);
        let path = project.join("src/main/java/demo").join(file);
        until(cx, 60, "the console's `claude` has its variables and the hooks' settings", |cx| {
            cx.try_global::<IdeEnv>().is_some_and(|e| e.0.iter().any(|(k, _)| *k == "SNOUT_HOOK_PORT"))
                && cx.try_global::<IdeArgs>().is_some_and(|a| !a.0.is_empty())
        })
        .await;
        let (env, args) = cx.update(|cx| (cx.global::<IdeEnv>().0.clone(), cx.global::<IdeArgs>().0.clone()));
        let port = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone()).unwrap();
        let (sse, hook) = (port("CLAUDE_CODE_SSE_PORT"), port("SNOUT_HOOK_PORT"));
        say(&format!("on the remote, the IDE server is at {sse}, the hooks at {hook}"));

        // The lock file is on the remote machine, naming the port there and the remote folder.
        let lock = in_container(&format!("cat $HOME/.claude/ide/{sse}.lock"), &[], "");
        let lock: serde_json::Value = serde_json::from_str(&lock).unwrap_or_else(|e| fail(&format!("{e}: {lock}")));
        if lock["workspaceFolders"] != serde_json::json!([PROJECT]) || lock["ideName"] != "SnoutCode" {
            fail(&format!("the lock file: {lock}"));
        }
        let token = lock["authToken"].as_str().unwrap().to_owned();

        // A client on the remote machine's loopback (a connection made there, as `claude` makes
        // it), on a thread of its own: the window answers on this one.
        let remote = cx.update(|cx| snout_app::host::host(cx));
        let sse_port: u16 = sse.parse().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            use tungstenite::client::IntoClientRequest as _;
            let stream = match remote.connect_tcp(sse_port, Duration::from_secs(10)) {
                Ok(s) => s,
                Err(e) => return tx.send(format!("no connection: {e}")).unwrap_or(()),
            };
            let mut req = format!("ws://127.0.0.1:{sse_port}").into_client_request().unwrap();
            req.headers_mut().insert("x-claude-code-ide-authorization", token.parse().unwrap());
            let mut ws = match tungstenite::client(req, stream) {
                Ok((ws, _)) => ws,
                Err(e) => return tx.send(format!("no handshake: {e}")).unwrap_or(()),
            };
            let mut call = |id: u64, method: &str, params: serde_json::Value| -> serde_json::Value {
                let msg = serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
                ws.send(tungstenite::Message::text(msg.to_string())).unwrap();
                loop {
                    let Ok(tungstenite::Message::Text(t)) = ws.read() else { continue };
                    let v: serde_json::Value = serde_json::from_str(t.as_str()).unwrap();
                    if v.get("id") == Some(&serde_json::json!(id)) {
                        return v;
                    }
                }
            };
            call(1, "initialize", serde_json::json!({ "protocolVersion": "2025-03-26", "capabilities": {} }));
            let folders = call(2, "tools/call", serde_json::json!({ "name": "getWorkspaceFolders", "arguments": {} }));
            let _ = tx.send(folders["result"]["content"][0]["text"].as_str().unwrap_or_default().to_owned());
            // Stays connected, as `claude` does, until the window goes.
            while ws.read().is_ok() {}
        });
        let folders = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let f = folders.clone();
        until(cx, 30, "Claude Code on the remote machine is answered", move |_| {
            if let Ok(text) = rx.try_recv() {
                *f.lock().unwrap() = Some(text);
            }
            f.lock().unwrap().is_some()
        })
        .await;
        let folders = folders.lock().unwrap().clone().unwrap();
        if !folders.contains(PROJECT) {
            fail(&format!("getWorkspaceFolders: {folders}"));
        }
        until(cx, 10, "the window counts the connection", |cx| cx.global::<IdeStatus>().clients >= 1).await;

        // The hooks, as `claude` runs them there: the command its settings name.
        let settings = in_container(&format!("cat '{}'", args[1]), &[], "");
        let settings: serde_json::Value = serde_json::from_str(&settings).unwrap();
        let hook_cmd = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_owned();
        let hook_env = [("SNOUT_HOOK_PORT", hook.clone())];
        let before = in_container(&format!("cat '{}'", path.display()), &[], "");
        let input = |event: &str| {
            serde_json::json!({ "hook_event_name": event, "session_id": "e2e", "cwd": PROJECT, "tool_name": "Edit",
                "tool_input": { "file_path": path, "old_string": "a", "new_string": "b" } })
            .to_string()
        };
        in_container(&hook_cmd, &hook_env, &input("PreToolUse"));
        let edited = format!("// Edited by Claude Code on the remote machine.\n{before}");
        in_container(&format!("cat > '{}'", path.display()), &[], &edited);
        in_container(&hook_cmd, &hook_env, &input("PostToolUse"));
        let (p, e) = (path.clone(), edited.clone());
        until(cx, 30, "the edit is listed and plays out in its tab", move |cx| {
            let ws = workspace(cx).unwrap();
            let listed = bridge::changes_of(ws.read(cx), cx).read(cx).set.files.iter().any(|f| f.path == p);
            listed && editor_of(&ws, &p, cx).is_some_and(|ed| ed.read(cx).live.is_none() && ed.read(cx).text(cx) == e)
        })
        .await;

        // Revert All puts the file back, on the remote machine.
        cx.update(|cx| {
            let ws = workspace(cx).unwrap();
            in_window(cx, |w, cx| ws.update(cx, |ws, cx| bridge::revert_all(ws, w, cx)));
        });
        let start = Instant::now();
        while in_container(&format!("cat '{}'", path.display()), &[], "") != before {
            if start.elapsed() > Duration::from_secs(30) {
                fail("Revert All did not put the file back");
            }
            wait(cx, 200).await;
        }
        say(&format!("Claude Code on the remote machine: found the window, its edit of {file} shown and reverted"));
        (sse, hook)
    }
}
