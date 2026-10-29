//! Screenshots of the whole window, rendered offscreen with GPUI's real renderer (Metal on
//! macOS) and the real text system, as `egui_kittest` snapshots do for the egui app.
//! Runs on the main thread (`harness = false`). With `SNOUT_SCREENSHOT=<dir>` the PNGs
//! are saved there to look at; either way each must actually show something.

#[cfg(target_os = "macos")]
fn main() {
    // The user guide's shots instead, when asked for (see `screenshots/docs.rs`).
    if let Some(projects) = std::env::var_os("SNOUT_DOCS_SHOTS") {
        docs::run(projects.into());
        return;
    }
    shots::run();
}

#[cfg(target_os = "macos")]
#[path = "screenshots/docs.rs"]
mod docs;

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("screenshots: skipped; GPUI has no headless renderer on this platform here");
}

#[cfg(target_os = "macos")]
mod shots {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;
    use std::time::Duration;

    use gpui::{AppContext as _, HeadlessAppContext, px, size};
    use snout_app::model::Action;
    use snout_app::panels::Activity;
    use snout_app::settings::{ColorTheme, Settings, ThemeChoice};
    use snout_app::workspace::Workspace;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success();
        assert!(ok, "git {args:?}");
    }

    /// The egui app's screenshot fixture, file for file: a little history, a tag, a second
    /// branch, and one file in each interesting state.
    pub fn demo_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snout-app-shot-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let dir = dir.canonicalize().unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "Demo"),
            ("user.email", "demo@example.com"),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
        ] {
            git(&dir, &["config", k, v]);
        }
        git(&dir, &["remote", "add", "origin", "git@github.com:demo/demo.git"]);
        let lib = include_str!("../../snout-core/src/status.rs");
        std::fs::write(dir.join("status.rs"), lib).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        std::fs::write(dir.join("README.md"), "# demo\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "Initial import"]);
        git(&dir, &["tag", "-a", "v0.1", "-m", "First release"]);
        git(&dir, &["branch", "feature/diff-view"]);
        git(&dir, &["commit", "-q", "--allow-empty", "-m", "Second commit"]);
        let edited = lib
            .replace("Human readable state", "Human-readable state")
            .replace("\"Conflict\".into()", "\"Conflicted\".into()")
            .replace("pub fn is_unborn", "/// True before the first commit.\n    pub fn is_unborn");
        std::fs::write(dir.join("status.rs"), edited).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello, snout\");\n    run();\n}\n")
            .unwrap();
        git(&dir, &["add", "src/main.rs"]);
        std::fs::write(dir.join("NOTES.md"), "untracked\n").unwrap();
        dir
    }

    /// The egui app's commit-view fixture: a merge, so the graph has a second lane.
    fn with_merge(dir: &Path) {
        git(dir, &["stash", "-u", "-q"]);
        git(dir, &["switch", "-q", "feature/diff-view"]);
        std::fs::write(dir.join("README.md"), "# demo\n\nNow with a diff view.\n").unwrap();
        git(dir, &["commit", "-qam", "Describe the diff view"]);
        git(dir, &["switch", "-q", "main"]);
        git(dir, &["merge", "-q", "--no-ff", "-m", "Merge feature/diff-view", "feature/diff-view"]);
        git(dir, &["stash", "pop", "-q"]);
    }

    const GREETER_V1: &str = "package demo;\n\nimport java.util.List;\n\n/** Says hello. */\npublic class Greeter {\n    private final String name;\n\n    public Greeter(String name) {\n        this.name = name;\n    }\n\n    // The greeting.\n    public String greet() {\n        return \"Hello, \" + name + \"!\";\n    }\n\n    public List<String> greetAll(List<String> names) {\n        return names.stream().map(n -> \"Hello, \" + n).toList();\n    }\n}\n";
    const GREETER_V2: &str = "package demo;\n\nimport java.util.List;\nimport java.util.Objects;\n\n/** Says hello. */\npublic class Greeter {\n    private final String name;\n\n    public Greeter(String name) {\n        this.name = Objects.requireNonNull(name);\n    }\n\n    // The greeting, with the name.\n    public String greet() {\n        return \"Hello, \" + name + \"!\";\n    }\n\n    @Override\n    public String toString() {\n        return \"Greeter[\" + name + \"]\";\n    }\n}\n";

    pub enum Select {
        File(&'static str),
        /// A commit (by summary) and one of its files, in a repository with a merge.
        Commit(&'static str, &'static str),
        Console,
        /// The console running a program that prints colours, attributes, wide characters
        /// and a link: the same output under each terminal engine.
        TerminalColors,
        /// No repository open: the welcome page.
        Nothing,
        /// A Java file in an editor tab.
        Editor,
        /// The Settings sheet on a page.
        Settings(snout_app::settings_view::Page),
        /// The Settings sheet on a page, scrolled down this far (in points).
        SettingsScrolled(snout_app::settings_view::Page, f32),
        /// The background tasks' popover: the engine build and a Git job running.
        Tasks,
        /// A change Claude Code proposes, under review in its tab: part way through
        /// typing itself in, or all in; Claude connected in the console.
        Review(Option<f32>),
        /// The Project view on the fixture model: modules, a shared folder, a failed build,
        /// the Gradle scripts and the libraries, some of them open.
        ProjectModel,
        /// The Run window on a test run: the tree with a failure and its diff.
        RunTests,
        /// The debugger stopped on a breakpoint: the line in the editor, the Debug window.
        Debug,
        /// The Gradle window's tasks, and a task's run in the Build window.
        Gradle,
        /// Spring, on the engine's `spring-app` fixture with the answers it recorded.
        Spring(SpringShot),
        /// The Keymap page with Go to File moved onto Settings' keys.
        Keymap,
        /// The New Project dialog: 0 a Java project, 1 a Spring Boot one, 2 its dependencies.
        NewProject(u8),
        /// The Processes window: the engine, its language server and Gradle daemons, a run,
        /// a test JVM, a terminal and a leftover.
        Processes,
        /// Remote SSH: 0 Connect to SSH Host with the hosts of an ssh config, 1 the folder
        /// picker of the remote machine, 2 a window on a remote machine whose connection is
        /// coming back (the title bar and the status bar say so).
        Remote(u8),
        /// Claude Code editing on its own, live, with the Claude Changes window open.
        Claude(ClaudeShot),
        /// A dialog over the Git view, by name: `discard`, `discard-many`, `delete-files`,
        /// `delete-branch`, `new-branch`, `clone`, `error`, `info`, `pull`, `checkout`, `about`.
        Dialog(&'static str),
        /// Two tool windows with nothing in them yet: one in the bottom dock, one on the right.
        Tools(snout_app::workspace::Tool, snout_app::workspace::Tool),
        /// The Git view of a repository with nothing to commit.
        Clean,
        /// Find in Files on a word no file holds.
        NoResults,
        /// Connect to SSH Host while it connects.
        Connecting,
        /// The bar offering a JDK the project needs.
        JdkBar,
    }

    #[derive(Clone, Copy, PartialEq)]
    pub enum ClaudeShot {
        /// The edit's removed line struck through, before the typing.
        Strike,
        /// Part way through typing the new text in.
        Typing,
        /// Done: the green band fading, the gutter's Claude marks, a change asking for review.
        Fade,
        /// The file's changes inline, from the window, with the bar to accept or revert them.
        Inline,
    }

    #[derive(Clone, Copy, PartialEq)]
    pub enum SpringShot {
        /// The controller with a bean menu of eighty injection sites, filtered.
        ManySites,
        /// The controller: beans, injections, endpoints, a chooser, `@Value` hints.
        Gutter,
        /// The application class (a run icon beside its bean), and the Beans tab.
        Beans,
        /// The Endpoints tab, and an endpoint's tooltip in the gutter.
        Endpoints,
        /// `application.properties` with its warnings and completion.
        Properties,
    }

    /// Colourful output for the terminal engines' shots: the 16 colours plain and bold, a
    /// 256-colour ramp, a true-colour gradient, the attributes, wide characters and a link.
    const TERMINAL_COLORS: &str = r#"#!/bin/sh
printf '\033]0;Terminal colours\007'
for i in 0 1 2 3 4 5 6 7; do printf '\033[3%sm col%s \033[0m' $i $i; done; printf '\n'
for i in 0 1 2 3 4 5 6 7; do printf '\033[1;3%sm bld%s \033[0m' $i $i; done; printf '\n'
for i in 0 1 2 3 4 5 6 7; do printf '\033[4%sm bg%s  \033[0m' $i $i; done; printf '\n'
i=16; while [ $i -lt 232 ]; do printf '\033[48;5;%sm ' $i; i=$((i+6)); done; printf '\033[0m\n'
i=0; while [ $i -lt 72 ]; do printf '\033[48;2;%s;%s;200m ' $((i*3)) $((255-i*3)); i=$((i+1)); done; printf '\033[0m\n'
printf '\033[1mbold\033[0m \033[2mdim\033[0m \033[3mitalic\033[0m \033[4munderline\033[0m \033[4:3mcurly\033[0m \033[9mstrike\033[0m \033[7minverse\033[0m \033[53moverline\033[0m\n'
printf 'wide: 中文字符 かな 한글  emoji: ✓ ★ ●\n'
printf 'link: \033]8;;https://example.com/docs\033\\the docs\033]8;;\033\\ and https://example.com/plain\n'
printf '\033[32m✓\033[0m Read \033[36msrc/main.rs\033[0m\n> '
sleep 30
"#;

    const MAIN_JAVA: &str = "package demo.app;\n\nimport java.util.List;\n\npublic final class Main {\n    public static void main(String[] args) {\n        List<String> names = List.of(\"ada\", \"grace\");\n        int total = 0;\n        for (int i = 1; i <= 3; i++) {\n            total = add(total, i);\n        }\n        System.out.println(\"total \" + total + \" for \" + names);\n    }\n\n    static int add(int a, int b) {\n        return a + b;\n    }\n}\n";

    fn note(
        runs: &gpui::Entity<snout_app::run::Runs>,
        method: &str,
        params: serde_json::Value,
        cx: &mut HeadlessAppContext,
    ) {
        cx.update(|cx| runs.update(cx, |r, cx| r.engine_notification(method, params, cx)));
    }

    include!("fixtures/project_fixture.rs");

    thread_local! {
        /// The colour theme the next shots take (Studio, the default, unless a shot says).
        static COLORS: std::cell::Cell<ColorTheme> = const { std::cell::Cell::new(ColorTheme::Studio) };
        /// The terminal engine the next shots take.
        static ENGINE: std::cell::Cell<snout_core::term::EngineKind> =
            const { std::cell::Cell::new(snout_core::term::EngineKind::Builtin) };
        /// The next shots' terminals are set up: another typeface and size, a taller line, a
        /// bar cursor, a shell and variables.
        static TERMINAL_SET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// `shot` with the terminals on another engine.
    fn engine_shot(engine: snout_core::term::EngineKind, name: &str, theme: ThemeChoice, select: Option<Select>) {
        ENGINE.set(engine);
        shot(name, theme, Activity::Git, select);
        ENGINE.set(snout_core::term::EngineKind::Builtin);
    }

    /// `shot` in another colour theme.
    fn themed_shot(colors: ColorTheme, name: &str, theme: ThemeChoice, activity: Activity, select: Option<Select>) {
        COLORS.set(colors);
        shot(name, theme, activity, select);
        COLORS.set(ColorTheme::default());
    }

    fn shot(name: &str, theme: ThemeChoice, activity: Activity, select: Option<Select>) {
        // `SNOUT_SHOT_ONLY=<part of a name>` takes only the shots it names.
        if std::env::var("SNOUT_SHOT_ONLY").is_ok_and(|only| !name.contains(&only)) {
            return;
        }
        let repo = demo_repo(name);
        if matches!(select, Some(Select::Commit(..))) {
            with_merge(&repo);
        }
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let current = (!matches!(select, Some(Select::Nothing))).then(|| repo.clone());
        let mut settings = Settings {
            theme,
            colors: COLORS.get(),
            terminal_engine: ENGINE.get(),
            repos: vec![repo.clone()],
            current,
            ..Settings::default()
        };
        if TERMINAL_SET.get() {
            settings.terminal_font = Some("Menlo".into());
            settings.terminal_font_size = Some(14.);
            settings.terminal_line_height = 1.5;
            settings.terminal_cursor = snout_app::settings::TerminalCursor::Bar;
            settings.terminal_shell = "/bin/bash".into();
            settings.terminal_env.insert("JAVA_TOOL_OPTIONS".into(), "-Xmx2g".into());
            settings.terminal_env.insert("SPRING_PROFILES_ACTIVE".into(), "dev".into());
            settings.terminal_copy_on_select = true;
        }
        if matches!(select, Some(Select::Nothing)) {
            // The Welcome page's recent projects, opened a while ago.
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let others =
                ["/Users/demo/dev/billing-service", "/Users/demo/dev/shop-platform", "/Users/demo/work/analytics"];
            let mut recent = vec![repo.clone()];
            recent.extend(others.iter().map(PathBuf::from));
            for (i, p) in recent.iter().enumerate() {
                settings.opened.insert(p.clone(), now - [120, 7_200, 90_000, 864_000][i]);
            }
            settings.recent = Some(recent);
        }
        cx.update(|cx| snout_app::init(&settings, cx));
        let model = cx.update(|cx| {
            let settings = settings.clone();
            cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
        });
        let mut ws = None;
        let handle = cx
            .open_window(size(px(1400.), px(900.)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        let ws = ws.unwrap();
        // The repository opens off the UI thread.
        cx.allow_parking();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_activity(activity, window, cx));
        })
        .unwrap();
        if !matches!(select, Some(Select::Nothing)) {
            cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
            cx.run_until_parked();
        }
        match select {
            Some(Select::File(path)) => cx.update(|cx| model.update(cx, |m, cx| m.click_file(path, false, false, cx))),
            Some(Select::Commit(summary, file)) => {
                cx.update(|cx| {
                    let id = model.read(cx).log.iter().find(|c| c.summary == summary).unwrap().id.clone();
                    model.update(cx, |m, cx| m.handle(Action::ShowCommit(id), cx));
                });
                cx.run_until_parked();
                cx.update(|cx| model.update(cx, |m, cx| m.select_commit_file(file.into(), cx)));
            }
            Some(Select::Console) => {
                let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
                let fake = repo.join("fake-claude");
                let script = "#!/bin/sh\nprintf '\\033]0;Fixing the diff view\\007'\nprintf '\\033[1;32m✓\\033[0m Read \\033[36msrc/main.rs\\033[0m\\n'\nprintf '\\033[7m inverse \\033[0m and \\033[4munderline\\033[0m\\n> '\nsleep 30\n";
                std::fs::write(&fake, script).unwrap();
                Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
                cx.update(|cx| console.update(cx, |c, _| c.program = Some(fake)));
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx)));
            }
            Some(Select::TerminalColors) => {
                let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
                let fake = repo.join("fake-claude");
                std::fs::write(&fake, TERMINAL_COLORS).unwrap();
                Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
                cx.update(|cx| console.update(cx, |c, _| c.program = Some(fake)));
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx)));
            }
            Some(Select::Editor) => {
                // A Java file with history, then edited: lines added, changed and removed.
                std::fs::create_dir_all(repo.join("src/main/java/demo")).unwrap();
                let file = repo.join("src/main/java/demo/Greeter.java");
                std::fs::write(&file, GREETER_V1).unwrap();
                git(&repo, &["add", "src/main/java/demo/Greeter.java"]);
                git(&repo, &["commit", "-qm", "Add the greeter"]);
                std::fs::write(&file, GREETER_V2).unwrap();
                std::fs::write(repo.join("build.gradle.kts"), "plugins {\n    java\n}\n").unwrap();
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
                cx.run_until_parked();
                let open =
                    |cx: &mut HeadlessAppContext, a: Action| cx.update(|cx| model.update(cx, |m, cx| m.handle(a, cx)));
                open(&mut cx, Action::OpenFile(repo.join("build.gradle.kts"), None));
                open(&mut cx, Action::OpenFile(repo.join("src/main.rs"), None));
                open(&mut cx, Action::PreviewFile(repo.join("README.md")));
                open(&mut cx, Action::OpenFile(file, Some((14, 23))));
                // The tabs read their files off the UI thread.
                cx.run_until_parked();
                // An unsaved change in another tab, for its dot.
                cx.update_window(handle.into(), |_, window, cx| {
                    let ws = ws.read(cx);
                    let main = ws.editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("src/main.rs")).unwrap();
                    let state = main.read(cx).state.clone();
                    state.update(cx, |s, cx| s.insert("// edited\n", window, cx));
                })
                .unwrap();
                // What a server would report: an error and a warning, for the inspections
                // widget and the squiggles.
                cx.update_window(handle.into(), |_, _, cx| {
                    let ws = ws.read(cx);
                    let greeter =
                        ws.editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("Greeter.java")).unwrap();
                    let at =
                        |l, a, b| lsp_types::Range::new(lsp_types::Position::new(l, a), lsp_types::Position::new(l, b));
                    let diags = vec![
                        lsp_types::Diagnostic {
                            range: at(10, 20, 27),
                            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                            message: "Objects cannot be resolved".into(),
                            ..Default::default()
                        },
                        lsp_types::Diagnostic {
                            range: at(2, 7, 21),
                            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                            message: "The import java.util.List is never used".into(),
                            ..Default::default()
                        },
                    ];
                    greeter.update(cx, |e, cx| e.set_diagnostics(diags, cx));
                })
                .unwrap();
                // The editor's right-click menu, its Refactor submenu open.
                if name.contains("editor-menu") {
                    cx.update_window(handle.into(), |_, window, cx| {
                        let ws = ws.read(cx);
                        let greeter =
                            ws.editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("Greeter.java")).unwrap();
                        let at = gpui::point(px(620.), px(300.));
                        greeter
                            .update(cx, |e, cx| e.open_context_menu_at(at, true, Some("Greeter".into()), window, cx));
                    })
                    .unwrap();
                }
                // The palette shot: the file split in two, and the command palette over it.
                if name.contains("palette") {
                    cx.update_window(handle.into(), |_, window, cx| {
                        ws.update(cx, |ws, cx| ws.split_right(window, cx));
                    })
                    .unwrap();
                    cx.run_until_parked();
                    cx.update_window(handle.into(), |_, window, cx| {
                        // Drawn once, so the palette finds the editor's own shortcuts.
                        window.draw(cx).clear(cx);
                        ws.update(cx, |ws, cx| ws.open_palette(window, cx));
                    })
                    .unwrap();
                }
                // In the light shot, find and replace in the file is open on a word.
                if theme == ThemeChoice::Light {
                    cx.update_window(handle.into(), |_, _, cx| {
                        let ws = ws.read(cx);
                        let greeter =
                            ws.editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("Greeter.java")).unwrap();
                        let state = greeter.read(cx).state.clone();
                        state.update(cx, |s, cx| {
                            let at = s.text().to_string().find("name;").unwrap();
                            s.set_selected_range(at..at + 4, cx);
                            s.open_search(true, cx);
                        });
                    })
                    .unwrap();
                }
            }
            Some(Select::Settings(page)) | Some(Select::SettingsScrolled(page, _)) => {
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.open_settings(window, cx));
                })
                .unwrap();
                // It comes up once the host's JDKs are found.
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, _, cx| {
                    let sheet = ws.read(cx).settings_sheet.clone().unwrap();
                    sheet.update(cx, |s, cx| {
                        s.page = page;
                        cx.notify();
                    });
                })
                .unwrap();
                if let Some(Select::SettingsScrolled(_, by)) = select {
                    // Drawn once, then the wheel over the page.
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.refresh();
                        window.draw(cx).clear(cx);
                        let at = gpui::point(px(900.), px(500.));
                        window.dispatch_event(
                            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                                position: at,
                                pressed_button: None,
                                modifiers: Default::default(),
                            }),
                            cx,
                        );
                        window.dispatch_event(
                            gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                                position: at,
                                delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-by))),
                                modifiers: Default::default(),
                                touch_phase: gpui::TouchPhase::Moved,
                                source: Default::default(),
                            }),
                            cx,
                        );
                    })
                    .unwrap();
                }
            }
            Some(Select::Keymap) => {
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.open_settings(window, cx));
                })
                .unwrap();
                // It comes up once the host's JDKs are found.
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, window, cx| {
                    let sheet = ws.read(cx).settings_sheet.clone().unwrap();
                    sheet.update(cx, |s, cx| {
                        s.page = snout_app::settings_view::Page::Keymap;
                        s.draft.keymap.insert("snout::GoToFile".into(), vec!["cmd-p".into(), "cmd-,".into()]);
                        s.draft.keymap.insert("snout::ToggleConsole".into(), vec![]);
                        s.taken = Some(("snout::GoToFile".into(), "cmd-,".into(), vec!["snout::OpenSettings".into()]));
                        s.key_search.update(cx, |i, cx| i.set_value("go", window, cx));
                        cx.notify();
                    });
                })
                .unwrap();
            }
            Some(Select::Remote(step)) => {
                use snout_app::remote::{Connection, Recents, Remote};
                let status = match step {
                    2 => snout_remote::Status::Reconnecting { attempt: 1, error: None },
                    _ => snout_remote::Status::Connected,
                };
                let connection = (step > 0).then(|| Connection { name: "devbox".into(), host: None, status });
                let recents = Recents {
                    hosts: vec!["devbox".into()],
                    projects: vec![snout_app::remote::RemoteProject {
                        host: "devbox".into(),
                        path: "/home/dev/demo".into(),
                    }],
                };
                // While it comes back, the engine is still on its way there.
                let tasks = (step == 2)
                    .then(|| snout_app::background::BgTask {
                        title: "Java engine on devbox".into(),
                        stage: "Downloading Java 21 for the engine".into(),
                        percent: Some(42),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect();
                cx.update(|cx| cx.set_global(Remote { recents, connection, tasks, ..Default::default() }));
                match step {
                    0 => {
                        // An ssh config with an Include, as the dialog reads it.
                        let ssh = std::env::temp_dir().join(format!("snout-shot-ssh-{}", std::process::id()));
                        std::fs::create_dir_all(ssh.join("conf.d")).unwrap();
                        std::fs::write(
                            ssh.join("config"),
                            "Host build-arm build-x86\n  User ci\nInclude conf.d/*\nHost *\n  ServerAliveInterval 30\n",
                        )
                        .unwrap();
                        std::fs::write(ssh.join("conf.d/work"), "Host staging gpu-box\n").unwrap();
                        // SAFETY: the harness runs one shot at a time on this thread.
                        unsafe { std::env::set_var(snout_app::remote::SSH_CONFIG_ENV, ssh.join("config")) };
                        cx.update_window(handle.into(), |_, window, cx| {
                            ws.update(cx, |w, cx| w.open_connect(window, cx))
                        })
                        .unwrap();
                        cx.run_until_parked();
                        unsafe { std::env::remove_var(snout_app::remote::SSH_CONFIG_ENV) };
                    }
                    1 => {
                        // The remote machine's folders, here this Mac's.
                        let home = repo.join("remote-home");
                        for d in ["demo", "billing-service", "notes", ".cache", "shop-platform"] {
                            std::fs::create_dir_all(home.join(d)).unwrap();
                        }
                        cx.update_window(handle.into(), |_, window, cx| {
                            let host = snout_app::host::host(cx);
                            let picker = cx.new(|cx| {
                                snout_app::remote::picker::FolderPicker::new(host, "devbox".into(), home, window, cx)
                            });
                            ws.update(cx, |w, cx| {
                                w.remote_picker = Some(picker);
                                cx.notify();
                            });
                        })
                        .unwrap();
                        cx.run_until_parked();
                        cx.update_window(handle.into(), |_, _, cx| {
                            let picker = ws.read(cx).remote_picker.clone().unwrap();
                            picker.update(cx, |p, cx| {
                                p.selected = Some("demo".into());
                                cx.notify();
                            });
                        })
                        .unwrap();
                    }
                    _ => {}
                }
            }
            Some(Select::NewProject(page)) => {
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.open_new_project(window, cx));
                    let view = ws.read(cx).new_project.clone().unwrap();
                    view.update(cx, |v, cx| {
                        v.name.update(cx, |i, cx| i.set_value("inventory-service", window, cx));
                        v.location.update(cx, |i, cx| i.set_value("/Users/demo/Projects", window, cx));
                        if page > 0 {
                            use snout_app::new_project::initializr::Catalog;
                            let mut c = Catalog::offline(Some("4.1.2".into()));
                            c.offline = false;
                            c.boot = vec![
                                ("4.2.0-RC1".into(), "4.2.0 (RC1)".into()),
                                ("4.1.2".into(), "4.1.2".into()),
                                ("4.0.9".into(), "4.0.9".into()),
                            ];
                            v.catalog = Some(c);
                            v.kind = snout_app::new_project::Kind::SpringBoot;
                            v.picking = page == 2;
                            v.deps = vec!["web".into(), "data-jpa".into(), "lombok".into()];
                        }
                        cx.notify();
                    });
                })
                .unwrap();
            }
            Some(Select::Tasks) => {
                cx.update_window(handle.into(), |_, _, cx| {
                    let ws = ws.read(cx);
                    let (hub, status) = (ws.hub.clone(), ws.status.clone());
                    hub.update(cx, |h, _| {
                        h.engine_build = Some(snout_app::background::EngineBuild {
                            stage: "Downloading JDK 21 to build snout-engine".into(),
                            ..Default::default()
                        })
                    });
                    model.update(cx, |m, _| m.job = Some("Fetching".into()));
                    status.update(cx, |s, _| s.tasks_open = true);
                })
                .unwrap();
            }
            Some(Select::Review(held)) => {
                std::fs::create_dir_all(repo.join("src/main/java/demo")).unwrap();
                let file = repo.join("src/main/java/demo/Greeter.java");
                std::fs::write(&file, GREETER_V1).unwrap();
                git(&repo, &["add", "src/main/java/demo/Greeter.java"]);
                git(&repo, &["commit", "-qm", "Add the greeter"]);
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(file.clone(), Some((0, 0))), cx)));
                // Claude Code connected, its console showing.
                let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
                let fake = repo.join("fake-claude");
                let script = "#!/bin/sh\nprintf '\\033]0;Making the greeter null safe\\007'\nprintf '\\033[1;32m●\\033[0m Update(\\033[36msrc/main/java/demo/Greeter.java\\033[0m)\\n'\nprintf '  Waiting for your review in the editor\\n> '\nsleep 30\n";
                std::fs::write(&fake, script).unwrap();
                Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
                cx.update(|cx| console.update(cx, |c, _| c.program = Some(fake)));
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx)));
                cx.update(|cx| cx.set_global(snout_app::claude_ide::IdeStatus { clients: 1 }));
                cx.update_window(handle.into(), |_, window, cx| {
                    let ws = ws.read(cx);
                    let greeter =
                        ws.editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("Greeter.java")).unwrap();
                    greeter.update(cx, |e, cx| {
                        let quiet: snout_app::editor::Answer = Box::new(|_| {});
                        e.propose("✻ [Claude Code] Greeter.java ⧉".into(), GREETER_V2.into(), quiet, window, cx);
                        // A second change waits its turn.
                        let quiet: snout_app::editor::Answer = Box::new(|_| {});
                        let next = GREETER_V2.replace("Says hello.", "Says hello, politely.");
                        e.propose("✻ [Claude Code] Greeter.java (2) ⧉".into(), next, quiet, window, cx);
                        match held {
                            Some(fraction) => e.hold_stream_at(fraction, window, cx),
                            None => e.finish_stream(window, cx),
                        }
                    });
                })
                .unwrap();
            }
            Some(Select::ProjectModel) => {
                use snout_app::project_view::NodeId;
                let jars = repo.with_extension("jars");
                let json = write_project_fixture(&repo, &jars);
                let parsed = snout_app::project_model::parse(&serde_json::from_str(&json).unwrap()).unwrap();
                // Committed, then one file changed: Git's colours as a project usually has them.
                std::fs::write(repo.join(".gitignore"), "build/\n").unwrap();
                git(&repo, &["add", "-A"]);
                git(&repo, &["commit", "-qm", "The shop"]);
                std::fs::write(
                    repo.join("app/src/main/java/shop/Cart.java"),
                    "package shop;\n\npublic class Cart { }\n",
                )
                .unwrap();
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
                let open =
                    |cx: &mut HeadlessAppContext, a: Action| cx.update(|cx| model.update(cx, |m, cx| m.handle(a, cx)));
                open(&mut cx, Action::OpenFile(repo.join("app/src/main/java/shop/App.java"), None));
                cx.update(|cx| {
                    let store = snout_app::project_model::store(cx).unwrap();
                    store.update(cx, |s, cx| s.set_model(parsed, cx));
                    let project = ws.read(cx).side.read(cx).project.clone();
                    project.update(cx, |v, cx| {
                        for id in [
                            NodeId::Module("shop:app".into()),
                            NodeId::Dir(repo.join("app/src")),
                            NodeId::Dir(repo.join("app/src/main")),
                            NodeId::Dir(repo.join("app/src/main/java")),
                            NodeId::Dir(repo.join("app/src/main/java/shop")),
                            NodeId::Shared(repo.join("core")),
                            NodeId::Failed("legacy".into()),
                            NodeId::Libraries,
                            NodeId::Library("com.google.guava:guava:33.0.0-jre".into()),
                            NodeId::Package(
                                "com.google.guava:guava:33.0.0-jre".into(),
                                "com.google.common.base".into(),
                            ),
                        ] {
                            v.expanded.insert(id);
                        }
                        cx.notify();
                    });
                });
            }
            Some(Select::RunTests) => {
                use serde_json::json;
                let runs = cx.update(|cx| ws.read(cx).runs.clone());
                let launch = snout_app::run::Launch::Tests {
                    module: "shop:app".into(),
                    filter: serde_json::Value::Null,
                    label: "CartTest".into(),
                    debug: false,
                    rerun_failed: false,
                };
                let id = cx.update(|cx| runs.update(cx, |r, cx| r.adopt(launch, "t1", cx)));
                for line in ["> Task :app:compileJava", "> Task :app:compileTestJava", "> Task :app:test"] {
                    note(
                        &runs,
                        "snout/taskOutput",
                        json!({ "runId": "t1", "stream": "stdout", "line": line }),
                        &mut cx,
                    );
                }
                let ev = |event: &str, id: &str, parent: Option<&str>, kind: &str, name: &str| {
                    json!({ "runId": "t1", "event": event, "id": id, "parent": parent, "kind": kind, "name": name,
                            "displayName": name, "className": "shop.CartTest" })
                };
                note(&runs, "snout/testEvent", ev("started", "r", None, "suite", "Gradle Test Run :app:test"), &mut cx);
                for (sid, suite, tests) in [
                    (
                        "s1",
                        "CartTest",
                        vec![
                            ("adds an item", "passed", 12),
                            ("totals the prices", "failed", 31),
                            ("empties", "passed", 3),
                            ("applies a coupon", "skipped", 0),
                        ],
                    ),
                    ("s2", "PriceTest", vec![("rounds half up", "passed", 2), ("formats euros", "passed", 5)]),
                ] {
                    note(&runs, "snout/testEvent", ev("started", sid, Some("r"), "suite", suite), &mut cx);
                    for (i, (name, outcome, ms)) in tests.into_iter().enumerate() {
                        let tid = format!("{sid}-{i}");
                        note(&runs, "snout/testEvent", ev("started", &tid, Some(sid), "test", name), &mut cx);
                        let mut done = ev("finished", &tid, Some(sid), "test", name);
                        done["outcome"] = json!(outcome);
                        done["durationMillis"] = json!(ms);
                        if outcome == "failed" {
                            done["message"] = json!("expected: <42.50> but was: <40.00>");
                            done["stackTrace"] = json!(
                                "org.opentest4j.AssertionFailedError: expected: <42.50> but was: <40.00>\n\tat org.junit.jupiter.api.AssertionUtils.fail(AssertionUtils.java:151)\n\tat shop.CartTest.totalsThePrices(CartTest.java:27)\n\tat java.base/java.util.ArrayList.forEach(ArrayList.java:1596)"
                            );
                            note(
                                &runs,
                                "snout/testEvent",
                                json!({ "runId": "t1", "event": "output", "kind": "test", "id": tid, "text": "cart: 2 items\n" }),
                                &mut cx,
                            );
                        }
                        note(&runs, "snout/testEvent", done, &mut cx);
                    }
                    let mut end = ev("finished", sid, Some("r"), "suite", suite);
                    end["outcome"] = json!("passed");
                    note(&runs, "snout/testEvent", end, &mut cx);
                }
                note(
                    &runs,
                    "snout/taskFinished",
                    json!({ "runId": "t1", "outcome": "failed", "millis": 4300 }),
                    &mut cx,
                );
                let _ = id;
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.show_bottom(ws.tabs.run, window, cx));
                })
                .unwrap();
            }
            Some(Select::Debug) => {
                use snout_app::run::debug::{Frame, Var};
                std::fs::create_dir_all(repo.join("app/src/main/java/demo/app")).unwrap();
                let file = repo.join("app/src/main/java/demo/app/Main.java");
                std::fs::write(&file, MAIN_JAVA).unwrap();
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(file.clone(), Some((9, 0))), cx)));
                let runs = cx.update(|cx| ws.read(cx).runs.clone());
                let config = snout_app::run::config::RunConfig {
                    id: "main:app:demo.app.Main".into(),
                    name: "Main".into(),
                    main_class: Some("demo.app.Main".into()),
                    ..Default::default()
                };
                cx.update(|cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, 9, cx)));
                cx.update(|cx| runs.update(cx, |r, cx| r.set_condition(&file, 15, Some("a > 2".into()), cx)));
                let id = cx.update(|cx| {
                    runs.update(cx, |r, cx| r.adopt(snout_app::run::Launch::Config { config, debug: true }, "d1", cx))
                });
                note(
                    &runs,
                    "snout/taskOutput",
                    serde_json::json!({ "runId": "d1", "stream": "stdout",
                    "line": "Listening for transport dt_socket at address: 52311" }),
                    &mut cx,
                );
                // A connection that answers nothing: the debugger is attached and stopped.
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                let (_server, _) = listener.accept().unwrap();
                let (dap, _events) = snout_lang::dap::Dap::over(Box::new(client)).unwrap();
                cx.update(|cx| {
                    runs.update(cx, |r, cx| {
                        let s = r.session_mut(id).unwrap();
                        let d = s.debug.as_mut().unwrap();
                        d.dap = Some(dap);
                        d.paused = true;
                        d.reason = Some("breakpoint".into());
                        d.thread = Some(1);
                        d.threads = vec![(1, "main".into()), (14, "Reference Handler".into())];
                        d.frames = vec![
                            Frame {
                                id: 1,
                                name: "Main.main(String[])".into(),
                                path: Some(file.clone()),
                                source: None,
                                line: 10,
                            },
                            Frame {
                                id: 2,
                                name: "NativeMethodAccessorImpl.invoke0(Method, Object, Object[])".into(),
                                path: None,
                                source: Some("NativeMethodAccessorImpl.java".into()),
                                line: 0,
                            },
                        ];
                        d.scopes = vec![Var { name: "Local".into(), value: String::new(), ty: None, reference: 5 }];
                        d.children.insert(
                            5,
                            vec![
                                Var {
                                    name: "args".into(),
                                    value: "String[0]@12".into(),
                                    ty: Some("String[]".into()),
                                    reference: 0,
                                },
                                Var {
                                    name: "names".into(),
                                    value: "ImmutableCollections$List12@13 size=2".into(),
                                    ty: Some("List<String>".into()),
                                    reference: 6,
                                },
                                Var { name: "total".into(), value: "3".into(), ty: Some("int".into()), reference: 0 },
                                Var { name: "i".into(), value: "3".into(), ty: Some("int".into()), reference: 0 },
                            ],
                        );
                        d.children.insert(
                            6,
                            vec![
                                Var {
                                    name: "[0]".into(),
                                    value: "\"ada\"".into(),
                                    ty: Some("String".into()),
                                    reference: 0,
                                },
                                Var {
                                    name: "[1]".into(),
                                    value: "\"grace\"".into(),
                                    ty: Some("String".into()),
                                    reference: 0,
                                },
                            ],
                        );
                        d.expanded.insert("scope:Local".into());
                        d.expanded.insert("scope:Local/names".into());
                        d.watches.push(snout_app::run::debug::Watch {
                            expr: "total * 2".into(),
                            value: Some(Ok(Var {
                                name: String::new(),
                                value: "6".into(),
                                ty: Some("int".into()),
                                reference: 0,
                            })),
                        });
                        d.evaluated = Some((
                            "add(total, 10)".into(),
                            Ok(Var { name: String::new(), value: "13".into(), ty: None, reference: 0 }),
                        ));
                        r.sync_marks(cx);
                    })
                });
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.show_bottom(ws.tabs.debug, window, cx));
                })
                .unwrap();
                std::mem::forget(_server);
            }
            Some(Select::Spring(kind)) => spring_setup(kind, &repo, &mut cx, handle.into(), &ws, &model),
            Some(Select::Gradle) => {
                use serde_json::json;
                let runs = cx.update(|cx| ws.read(cx).runs.clone());
                let tasks = json!({ "builds": [ { "build": "shop", "rootDir": repo, "projects": [
                    { "project": ":", "name": "shop", "module": "shop", "groups": [
                        { "name": "help", "tasks": [ { "name": "tasks", "path": ":tasks" }, { "name": "projects", "path": ":projects" } ] } ] },
                    { "project": ":app", "name": "app", "module": "shop:app", "groups": [
                        { "name": "application", "tasks": [ { "name": "bootRun", "path": ":app:bootRun", "description": "Runs this project as a Spring Boot application." } ] },
                        { "name": "build", "tasks": [
                            { "name": "assemble", "path": ":app:assemble" }, { "name": "build", "path": ":app:build" },
                            { "name": "classes", "path": ":app:classes", "description": "Assembles main classes." },
                            { "name": "clean", "path": ":app:clean" }, { "name": "jar", "path": ":app:jar" } ] },
                        { "name": "verification", "tasks": [ { "name": "check", "path": ":app:check" }, { "name": "test", "path": ":app:test" } ] },
                        { "name": "other", "tasks": [ { "name": "compileJava", "path": ":app:compileJava" } ] } ] } ] },
                    { "build": "billing", "rootDir": repo.join("billing"), "projects": [] } ] });
                cx.update(|cx| runs.update(cx, |r, _| r.gradle.builds = snout_app::run::parse_gradle_tasks(&tasks)));
                let gradle = cx.update(|cx| ws.read(cx).gradle.clone());
                cx.update(|cx| {
                    gradle.update(cx, |g, cx| {
                        g.reveal("classes", cx);
                        g.reveal("bootRun", cx);
                    })
                });
                let id = cx.update(|cx| {
                    runs.update(cx, |r, cx| {
                        r.adopt(
                            snout_app::run::Launch::Task { build: Some("shop".into()), task: ":app:classes".into() },
                            "g1",
                            cx,
                        )
                    })
                });
                for (task, outcome, ms) in [
                    (":app:compileJava", "success", 2140),
                    (":app:processResources", "up-to-date", 3),
                    (":app:classes", "success", 0),
                ] {
                    note(
                        &runs,
                        "snout/taskProgress",
                        json!({ "runId": "g1", "task": task, "event": "started" }),
                        &mut cx,
                    );
                    note(
                        &runs,
                        "snout/taskOutput",
                        json!({ "runId": "g1", "stream": "stdout", "line": format!("> Task {task}") }),
                        &mut cx,
                    );
                    note(
                        &runs,
                        "snout/taskProgress",
                        json!({ "runId": "g1", "task": task, "event": "finished", "outcome": outcome, "millis": ms }),
                        &mut cx,
                    );
                }
                note(
                    &runs,
                    "snout/taskOutput",
                    json!({ "runId": "g1", "stream": "stdout", "line": "\u{1b}[32;1mBUILD SUCCESSFUL\u{1b}[0m in 3s" }),
                    &mut cx,
                );
                note(
                    &runs,
                    "snout/taskFinished",
                    json!({ "runId": "g1", "outcome": "success", "millis": 3120 }),
                    &mut cx,
                );
                let _ = id;
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| {
                        ws.toggle_tool(snout_app::workspace::Tool::Gradle, window, cx);
                        ws.show_bottom(ws.tabs.build, window, cx);
                    });
                })
                .unwrap();
            }
            Some(Select::Processes) => {
                use snout_app::processes::{Kind, Row};
                let engine_root = repo.clone();
                let row =
                    |pid: u32, depth: usize, kind: Kind, purpose: &str, mb: u64, cpu: f32, up: u64, state: &str| Row {
                        pid: Some(pid),
                        start: 0,
                        depth,
                        kind,
                        purpose: purpose.into(),
                        project: Some("shop".into()),
                        memory: mb << 20,
                        cpu,
                        uptime: up,
                        state: state.into(),
                        busy: matches!(state, "Ready" | "Building" | "Running" | "Debugging" | "Indexing"),
                        idle: state.starts_with("Idle") || state == "Left over",
                    };
                let rows = vec![
                    row(48211, 0, Kind::App, "SnoutCode: the window, the editors and Git", 412, 2.1, 7_440, "Running"),
                    row(
                        48302,
                        1,
                        Kind::Engine { root: engine_root.clone() },
                        "Java engine for shop: reads the Gradle build, runs and tests the code",
                        236,
                        0.4,
                        7_380,
                        "Ready",
                    ),
                    row(
                        48377,
                        2,
                        Kind::LanguageServer { root: engine_root.clone() },
                        "Java language server for shop: completion, errors, navigation",
                        1_310,
                        3.8,
                        7_352,
                        "Ready",
                    ),
                    row(
                        51022,
                        2,
                        Kind::Run { session: Some(1) },
                        "ShopApplication: your program, being debugged",
                        388,
                        12.5,
                        512,
                        "Debugging",
                    ),
                    row(
                        48455,
                        2,
                        Kind::Daemon { version: "8.10.2".into() },
                        "Gradle daemon 8.10.2 for shop: syncs the build, runs tasks and tests",
                        702,
                        86.0,
                        7_300,
                        "Building",
                    ),
                    row(
                        52107,
                        3,
                        Kind::Tests { session: Some(2) },
                        "Tests of OrderServiceTest: the test JVM Gradle started",
                        301,
                        97.2,
                        14,
                        "Running",
                    ),
                    row(
                        49120,
                        2,
                        Kind::Daemon { version: "7.6.4".into() },
                        "Gradle daemon 7.6.4 for shop: syncs the build, runs tasks and tests",
                        455,
                        0.0,
                        5_100,
                        "Idle for 6 min, stops after 15",
                    ),
                    row(48299, 1, Kind::Terminal, "Terminal: your zsh shell in shop", 6, 0.0, 7_420, "Running"),
                    row(
                        39004,
                        0,
                        Kind::Leftover,
                        "Gradle daemon 8.5, left over from an earlier session",
                        520,
                        0.0,
                        93_600,
                        "Left over",
                    ),
                ];
                let processes = cx.update(|cx| ws.read(cx).processes.clone());
                cx.update(|cx| processes.update(cx, |p, cx| p.set_rows(rows, cx)));
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.toggle_tool(snout_app::workspace::Tool::Processes, window, cx));
                })
                .unwrap();
            }
            Some(Select::Claude(which)) => {
                use snout_app::claude_ide::changes::Source;
                use snout_app::claude_ide::panel::{Activity, PendingDiff};
                std::fs::create_dir_all(repo.join("src/main/java/demo")).unwrap();
                let file = repo.join("src/main/java/demo/Greeter.java");
                std::fs::write(&file, GREETER_V1).unwrap();
                std::fs::write(repo.join("build.gradle.kts"), "plugins {\n    java\n}\n").unwrap();
                git(&repo, &["add", "-A"]);
                git(&repo, &["commit", "-qm", "Add the greeter"]);
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(file.clone(), Some((0, 0))), cx)));
                let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
                let fake = repo.join("fake-claude");
                let script = "#!/bin/sh\nprintf '\\033]0;Making the greeter null safe\\007'\nprintf '\\033[1;32m●\\033[0m Update(\\033[36msrc/main/java/demo/Greeter.java\\033[0m)\\n'\nprintf '  Updated src/main/java/demo/Greeter.java with 6 additions and 4 removals\\n> '\nsleep 30\n";
                std::fs::write(&fake, script).unwrap();
                Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
                cx.update(|cx| console.update(cx, |c, _| c.program = Some(fake)));
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx)));
                cx.update(|cx| cx.set_global(snout_app::claude_ide::IdeStatus { clients: 1 }));
                cx.run_until_parked();
                // The session so far: the greeter, a file Claude made, one the watcher saw.
                let changes = cx.update(|cx| snout_app::claude_ide::bridge::changes_of(ws.read(cx), cx));
                let farewell = repo.join("src/main/java/demo/Farewell.java");
                let gradle = repo.join("build.gradle.kts");
                cx.update(|cx| {
                    changes.update(cx, |c, cx| {
                        c.root = Some(repo.clone());
                        c.set.new_turn("claude", Some("Add bulk discounts".into()));
                        c.hooks_heard = true;
                        c.set.before_edit(&file, Some(Arc::new(GREETER_V1.into())));
                        c.record(file.clone(), Some(Arc::new(GREETER_V2.into())), None, "Edit".into(), Source::Hook, "claude", cx);
                        c.set.before_edit(&farewell, None);
                        let text = "package demo;\n\n/** Says goodbye. */\npublic class Farewell {\n    public String bye(String name) {\n        return \"Goodbye, \" + name;\n    }\n}\n";
                        c.record(farewell.clone(), Some(Arc::new(text.into())), None, "Write".into(), Source::Hook, "claude", cx);
                        let old = Some(Some(Arc::new("plugins {\n    java\n}\n".to_owned())));
                        let new = "plugins {\n    java\n    application\n}\n\napplication {\n    mainClass = \"demo.Main\"\n}\n";
                        c.record(gradle.clone(), Some(Arc::new(new.into())), old, String::new(), Source::Watcher, "claude", cx);
                        c.expanded.insert(file.clone());
                        let tool = if which == ClaudeShot::Strike || which == ClaudeShot::Typing { "Edit" } else { "Write" };
                        c.activity =
                            Some(Activity { path: file.clone(), tool: tool.into(), at: std::time::Instant::now(), done: false });
                        if matches!(which, ClaudeShot::Fade | ClaudeShot::Inline) {
                            c.activity = None;
                            c.pending.push(PendingDiff {
                                tab: "✻ [Claude Code] README.md ⧉".into(),
                                path: repo.join("README.md"),
                                added: 4,
                                removed: 1,
                                new_file: false,
                                front: true,
                            });
                        }
                    })
                });
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, window, cx| {
                    if which != ClaudeShot::Typing || ws.read(cx).model.read(cx).settings.theme == ThemeChoice::Light {
                        ws.update(cx, |ws, cx| ws.toggle_tool(snout_app::workspace::Tool::ClaudeChanges, window, cx));
                    }
                    let greeter = ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file).unwrap();
                    match which {
                        ClaudeShot::Strike | ClaudeShot::Typing => {
                            greeter.update(cx, |e, cx| {
                                e.animate_edit(GREETER_V2, true, window, cx);
                                e.hold_live_at(if which == ClaudeShot::Strike { 0. } else { 0.55 }, window, cx);
                            });
                        }
                        ClaudeShot::Fade | ClaudeShot::Inline => {
                            greeter.update(cx, |e, cx| {
                                e.animate_edit(GREETER_V2, false, window, cx);
                            });
                            ws.update(cx, snout_app::claude_ide::bridge::sync_bases);
                        }
                    }
                })
                .unwrap();
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, window, cx| {
                    let greeter = ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file).unwrap();
                    greeter.update(cx, |e, cx| match which {
                        ClaudeShot::Fade => {
                            let hunks = snout_app::claude_ide::review::hunks(GREETER_V1, GREETER_V2);
                            let lines: Vec<std::ops::Range<usize>> =
                                hunks.iter().map(|h| h.new_lines.clone()).filter(|l| !l.is_empty()).collect();
                            e.hold_flash(lines, 8, cx);
                        }
                        ClaudeShot::Inline => {
                            let first = e.claude_hunks.first().map_or(0, |h| h.new_lines.start);
                            e.go_to_claude_line(first, window, cx);
                        }
                        _ => {}
                    });
                })
                .unwrap();
            }
            Some(Select::Dialog(which)) => {
                use snout_app::model::{Dialog, ModelEvent, Mood};
                // A file deep in the tree, for a long path.
                let deep = "src/main/java/com/example/billing/service/internal/InvoiceReconciliationService.java";
                std::fs::create_dir_all(repo.join(deep).parent().unwrap()).unwrap();
                std::fs::write(repo.join(deep), "class InvoiceReconciliationService {}\n").unwrap();
                git(&repo, &["add", deep]);
                git(&repo, &["commit", "-qm", "Add the reconciliation service"]);
                std::fs::write(repo.join(deep), "class InvoiceReconciliationService { int total; }\n").unwrap();
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
                cx.run_until_parked();
                let dialog = cx.update(|cx| {
                    let m = model.read(cx);
                    let entry = |path: &str| m.status.files.iter().find(|f| f.path == path).cloned().unwrap();
                    match which {
                        "discard" => Dialog::ConfirmDiscard(vec![entry(deep)]),
                        "discard-many" => Dialog::ConfirmDiscard(m.status.files.clone()),
                        "delete-files" => Dialog::ConfirmDeleteFiles(vec![entry("NOTES.md")]),
                        "delete-branch" => Dialog::DeleteBranch { name: "feature/diff-view".into(), upstream: None },
                        "new-branch" => Dialog::NewBranch(None),
                        "clone" => Dialog::Clone { parent: "/Users/demo/dev".into() },
                        "error" => Dialog::Error(
                            "To github.com:demo/demo.git\n ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs to 'github.com:demo/demo.git'\nhint: Updates were rejected because the remote contains work that you do not\nhint: have locally.".into(),
                        ),
                        "info" => Dialog::Info {
                            title: "No repositories found".into(),
                            text: "Nothing under /Users/demo/Downloads is a Git repository.".into(),
                            mood: Mood::Empty,
                        },
                        "pull" => Dialog::PullProblem {
                            problem: snout_core::PullProblem::Divergent,
                            details: "hint: You have divergent branches and need to specify how to reconcile them.\nfatal: Need to specify how to reconcile divergent branches.".into(),
                        },
                        "checkout" => Dialog::ConfirmCheckout(snout_core::CheckoutTarget::Branch("feature/diff-view".into())),
                        "about" => Dialog::About,
                        other => panic!("no dialog {other}"),
                    }
                });
                cx.update(|cx| {
                    model.update(cx, |m, cx| {
                        m.dialog = Some(dialog);
                        cx.emit(ModelEvent::Dialog);
                    })
                });
            }
            Some(Select::Tools(bottom, right)) => {
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| {
                        ws.toggle_tool(bottom, window, cx);
                        ws.toggle_tool(right, window, cx);
                    });
                })
                .unwrap();
            }
            Some(Select::Clean) => {
                git(&repo, &["add", "-A"]);
                git(&repo, &["commit", "-qm", "Everything in"]);
                cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
            }
            Some(Select::NoResults) => {
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.open_find_in_files(false, window, cx));
                    let view = ws.read(cx).find_in_files.clone().unwrap();
                    view.update(cx, |v, cx| v.find.update(cx, |i, cx| i.set_value("zebraPattern", window, cx)));
                })
                .unwrap();
            }
            Some(Select::JdkBar) => {
                cx.update_window(handle.into(), |_, _, cx| {
                    let hub = ws.read(cx).hub.clone();
                    hub.update(cx, |h, cx| {
                        h.jdk_prompt = Some(snout_app::lang::hub::JdkPrompt {
                            root: repo.clone(),
                            major: 21,
                            resync: false,
                            progress: None,
                            error: None,
                        });
                        cx.notify();
                    });
                })
                .unwrap();
            }
            Some(Select::Connecting) => {
                cx.update_window(handle.into(), |_, window, cx| {
                    ws.update(cx, |w, cx| w.open_connect(window, cx));
                    let dialog = ws.read(cx).connect_dialog.clone().unwrap();
                    dialog.update(cx, |d, cx| {
                        d.input.update(cx, |i, cx| i.set_value("devbox", window, cx));
                        d.connecting = Some("devbox".into());
                        cx.notify();
                    });
                })
                .unwrap();
            }
            Some(Select::Nothing) | None => {}
        }
        // Let async work (the highlighter, the survey) finish and draw a few frames.
        cx.allow_parking();
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(50));
            cx.advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
        let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
        let first = image.get_pixel(0, 0);
        assert!(image.pixels().any(|p| p != first), "{name}: the window is blank");
        if let Some(dir) = std::env::var_os("SNOUT_SCREENSHOT") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            image.save(dir.join(format!("{name}.png"))).unwrap();
        }
        let _ = std::fs::remove_dir_all(repo);
        println!("screenshot {name}: ok");
    }

    /// Completion items given as they are, for a shot of the completion menu.
    struct FixedCompletions(Vec<lsp_types::CompletionItem>);

    impl gpui_component::input::CompletionProvider for FixedCompletions {
        fn completions(
            &self,
            _: &gpui_component::input::Rope,
            _: usize,
            _: lsp_types::CompletionContext,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) -> gpui::Task<anyhow::Result<lsp_types::CompletionResponse>> {
            gpui::Task::ready(Ok(lsp_types::CompletionResponse::Array(self.0.clone())))
        }

        fn is_completion_trigger(&self, _: usize, _: &str, _: &mut gpui::App) -> bool {
            false
        }
    }

    /// The editor of the file whose path ends with `name`.
    fn editor_named(
        ws: &gpui::Entity<Workspace>,
        name: &str,
        cx: &mut HeadlessAppContext,
    ) -> gpui::Entity<snout_app::editor::EditorPanel> {
        cx.update(|cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path.ends_with(name)).unwrap())
    }

    /// Draw a few frames, so the editors lay out and the gutter knows its lanes.
    fn settle(handle: gpui::AnyWindowHandle, cx: &mut HeadlessAppContext) {
        for _ in 0..3 {
            cx.advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
    }

    /// The middle of the Spring lane on the line holding `find`.
    fn spring_lane(
        editor: &gpui::Entity<snout_app::editor::EditorPanel>,
        find: &str,
        cx: &mut HeadlessAppContext,
    ) -> gpui::Point<gpui::Pixels> {
        cx.update(|cx| {
            let e = editor.read(cx);
            let (a, z) = e.lanes.get().spring();
            let s = e.state.read(cx);
            let off = s.text().to_string().find(find).unwrap();
            let b = s.range_to_bounds(&(off..off)).unwrap();
            gpui::point((a + z) / 2., b.center().y)
        })
    }

    /// The `spring-app` fixture in `repo`, the model fed what the engine answered, and the
    /// shot's file open.
    fn spring_setup(
        kind: SpringShot,
        repo: &Path,
        cx: &mut HeadlessAppContext,
        handle: gpui::AnyWindowHandle,
        ws: &gpui::Entity<Workspace>,
        model: &gpui::Entity<snout_app::model::Model>,
    ) {
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/lsp-driver/src/test/fixtures/spring-app");
        for item in ["build.gradle", "settings.gradle", "src"] {
            assert!(Command::new("cp").arg("-R").arg(from.join(item)).arg(repo).status().unwrap().success());
        }
        git(repo, &["add", "-A"]);
        git(repo, &["commit", "-qm", "The Spring application"]);
        let text = include_str!("fixtures/spring-app.json").replace("{ROOT}", &repo.to_string_lossy());
        let f: serde_json::Value = serde_json::from_str(&text).unwrap();
        let modules = snout_app::spring::parse_beans(&f["beans"]);
        let endpoints = snout_app::spring::parse_endpoints(&f["endpoints"]);
        let spring = cx.update(|cx| ws.read(cx).spring.clone());
        let ends = endpoints.clone();
        cx.update(|cx| spring.update(cx, |s, cx| s.set_model(modules, ends, cx)));
        let open = |cx: &mut HeadlessAppContext, path: PathBuf, at: (u32, u32)| {
            cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(path, Some(at)), cx)))
        };
        let java = repo.join("src/main/java/demo");
        let marks = |name: &str| {
            snout_app::spring::FileSpring::parse(&*snout_app::host::local(), &f["injections"][name], &endpoints)
        };
        let feed = |cx: &mut HeadlessAppContext, name: &str, marks: snout_app::spring::FileSpring| {
            let editor = editor_named(ws, name, cx);
            cx.update(|cx| {
                editor.update(cx, |e, cx| {
                    e.spring = Some(std::rc::Rc::new(marks));
                    cx.notify();
                })
            });
            editor
        };
        match kind {
            SpringShot::Gutter | SpringShot::Endpoints | SpringShot::ManySites => {
                let file = java.join("web/GreetingController.java");
                // An edit since the commit, for the Git marks beside the others.
                let text = std::fs::read_to_string(&file).unwrap();
                std::fs::write(
                    &file,
                    text.replace(
                        "        return body;",
                        "        // Echoed as it came.
        return body;",
                    ),
                )
                .unwrap();
                open(cx, file.clone(), (20, 0));
                let editor = feed(cx, "GreetingController.java", marks("GreetingController"));
                let hints = snout_app::spring::parse_hints(&f["inlayHints"]);
                let diags: Vec<lsp_types::Diagnostic> = serde_json::from_value(f["diagnostics"].clone()).unwrap();
                cx.update(|cx| {
                    editor.update(cx, |e, cx| {
                        e.hints = std::rc::Rc::new(hints);
                        e.set_diagnostics(diags, cx);
                    })
                });
                let runs = cx.update(|cx| ws.read(cx).runs.clone());
                cx.update(|cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, 40, cx)));
                settle(handle, cx);
                if kind == SpringShot::Gutter {
                    let at = spring_lane(&editor, "public GreetingController(", cx);
                    cx.update(|cx| {
                        editor.update(cx, |e, cx| {
                            let line = e.spring.as_ref().unwrap().line(32).unwrap().clone();
                            let items = snout_app::editor::spring_items(&line);
                            e.gutter_menu = Some(snout_app::editor::GutterMenu::Spring { at, items, filter: None });
                            cx.notify();
                        })
                    });
                } else if kind == SpringShot::ManySites {
                    // A bean injected in eighty places: the list is capped, scrolls, and
                    // has a field to narrow it.
                    let at = spring_lane(&editor, "public GreetingController(", cx);
                    cx.update_window(handle, |_, window, cx| {
                        editor.update(cx, |e, cx| {
                            let line = e.spring.as_ref().unwrap().line(32).unwrap().clone();
                            let target = snout_app::editor::spring_items(&line)
                                .into_iter()
                                .find_map(|i| match i {
                                    snout_app::editor::SpringItem::Go { target, .. } => Some(target),
                                    _ => None,
                                })
                                .unwrap();
                            let mut items =
                                vec![snout_app::editor::SpringItem::Heading("Bean 'clock' is injected into".into())];
                            for i in 0..80 {
                                items.push(snout_app::editor::SpringItem::Go {
                                    label: format!("OrderService{i}.clock"),
                                    detail: format!("OrderService{i}.java:{}", 12 + i),
                                    target: target.clone(),
                                });
                            }
                            let filter = cx.new(|cx| gpui_component::input::InputState::new(window, cx));
                            filter.update(cx, |f, cx| f.set_value("service1", window, cx));
                            e.gutter_menu =
                                Some(snout_app::editor::GutterMenu::Spring { at, items, filter: Some(filter) });
                            cx.notify();
                        })
                    })
                    .unwrap();
                } else {
                    let at = spring_lane(&editor, "public String hello", cx);
                    cx.update(|cx| {
                        editor.update(cx, |e, cx| {
                            let tip = e.spring.as_ref().unwrap().line(39).unwrap().tooltip();
                            e.gutter_tip = Some((39, at, tip));
                            cx.notify();
                        })
                    });
                    let win = cx.update(|cx| ws.read(cx).spring_window.clone());
                    cx.update(|cx| win.update(cx, |w, cx| w.set_tab(snout_app::spring::window::Tab::Endpoints, cx)));
                    cx.update_window(handle, |_, window, cx| {
                        ws.update(cx, |ws, cx| ws.toggle_tool(snout_app::workspace::Tool::Spring, window, cx));
                    })
                    .unwrap();
                }
            }
            SpringShot::Beans => {
                open(cx, java.join("config/AppConfig.java"), (0, 0));
                feed(cx, "AppConfig.java", marks("AppConfig"));
                open(cx, java.join("DemoApplication.java"), (8, 0));
                // The application class's bean mark, as the engine marks it.
                let app = r#"{"uri":"","lines":[{"line":8,"injections":[],"beans":[{"name":"demoApplication","type":"demo.DemoApplication","kind":"configuration","stereotype":"org.springframework.boot.autoconfigure.SpringBootApplication","declaringType":"demo.DemoApplication","language":"java","injectedInto":[]}]}]}"#;
                let app: serde_json::Value = serde_json::from_str(app).unwrap();
                feed(
                    cx,
                    "DemoApplication.java",
                    snout_app::spring::FileSpring::parse(&*snout_app::host::local(), &app, &[]),
                );
                let win = cx.update(|cx| ws.read(cx).spring_window.clone());
                cx.update(|cx| {
                    win.update(cx, |w, cx| {
                        w.toggle("m:spring-app:demo.greeting", cx);
                        w.toggle("m:spring-app:demo.config", cx);
                    })
                });
                cx.update_window(handle, |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.toggle_tool(snout_app::workspace::Tool::Spring, window, cx));
                })
                .unwrap();
            }
            SpringShot::Properties => {
                let file = repo.join("src/main/resources/application.properties");
                std::fs::write(&file, std::fs::read_to_string(&file).unwrap() + "demo.").unwrap();
                open(cx, file.clone(), (8, 5));
                let editor = editor_named(ws, "application.properties", cx);
                let diags: Vec<lsp_types::Diagnostic> =
                    serde_json::from_value(f["propertiesDiagnostics"].clone()).unwrap();
                let items: Vec<lsp_types::CompletionItem> =
                    serde_json::from_value(f["completion_props"]["items"].clone()).unwrap();
                cx.update(|cx| editor.update(cx, |e, cx| e.set_diagnostics(diags, cx)));
                settle(handle, cx);
                cx.update_window(handle, |_, window, cx| {
                    editor.update(cx, |e, cx| {
                        e.state.update(cx, |s, cx| {
                            s.lsp_mut().completion_provider = Some(std::rc::Rc::new(FixedCompletions(items)));
                            s.focus(window, cx);
                            s.show_completions(window, cx);
                        })
                    })
                })
                .unwrap();
            }
        }
    }

    /// The Project view of a real folder with a model the engine wrote for it, when
    /// `SNOUT_SHOT_PROJECT` and `SNOUT_SHOT_MODEL` name them; `SNOUT_SHOT_OPEN` lists module
    /// names to open, separated by commas. The folder is only read.
    fn real_project_shot(name: &str, theme: ThemeChoice) {
        let (Some(folder), Some(file)) = (std::env::var_os("SNOUT_SHOT_PROJECT"), std::env::var_os("SNOUT_SHOT_MODEL"))
        else {
            return;
        };
        let folder = PathBuf::from(folder).canonicalize().unwrap();
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        let parsed = snout_app::project_model::parse(&json).expect("a model of format 2");
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let settings = Settings { theme, current: Some(folder.clone()), ..Settings::default() };
        cx.update(|cx| snout_app::init(&settings, cx));
        let model = cx.update(|cx| {
            let settings = settings.clone();
            cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
        });
        let mut ws = None;
        let handle = cx
            .open_window(size(px(1400.), px(900.)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        let ws = ws.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_activity(Activity::Project, window, cx));
        })
        .unwrap();
        let open: Vec<String> = std::env::var("SNOUT_SHOT_OPEN")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        cx.update(|cx| {
            use snout_app::project_view::NodeId;
            let ids: Vec<NodeId> = parsed
                .modules
                .iter()
                .filter(|m| open.contains(&m.name))
                .map(|m| NodeId::Module(m.id.clone()))
                .chain(
                    parsed
                        .shared
                        .iter()
                        .filter(|s| open.contains(&s.name))
                        .map(|s| NodeId::Shared(s.content_root.clone())),
                )
                .chain(
                    parsed.failed.iter().filter(|f| open.contains(&f.build)).map(|f| NodeId::Failed(f.build.clone())),
                )
                .chain(open.iter().filter(|o| *o == "libraries").map(|_| NodeId::Libraries))
                .chain(open.iter().filter(|o| *o == "scripts").map(|_| NodeId::Scripts))
                .collect();
            snout_app::project_model::store(cx).unwrap().update(cx, |s, cx| s.set_model(parsed, cx));
            let project = ws.read(cx).side.read(cx).project.clone();
            project.update(cx, |v, cx| {
                v.expanded.extend(ids);
                cx.notify();
            });
        });
        cx.allow_parking();
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(50));
            cx.advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
        let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
        if let Some(dir) = std::env::var_os("SNOUT_SCREENSHOT") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            image.save(dir.join(format!("{name}.png"))).unwrap();
        }
        println!("screenshot {name}: ok");
    }

    /// The Java tools on the `shop` fixture (`tests/fixtures/shop`), for real: the engine
    /// (`engine/build/dist`, or `SNOUT_ENGINE_DIST`) reads it, and each step is shot as the
    /// user sees it. Only with `SNOUT_SHOT_JAVA=<JDK 17 to 21 home>`.
    fn real_java_shots(theme: ThemeChoice) {
        let Some(jdk) = std::env::var_os("SNOUT_SHOT_JAVA").map(PathBuf::from) else { return };
        let mode = if theme == ThemeChoice::Dark { "dark" } else { "light" };
        let dir = std::env::temp_dir().join(format!("snout-app-shot-java-{mode}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shop");
        assert!(Command::new("cp").arg("-R").arg(&from).arg(&dir).status().unwrap().success());
        let repo = dir.join("shop").canonicalize().unwrap();
        std::fs::write(repo.join("gradle.properties"), "org.gradle.daemon.idletimeout=60000\n").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        let c = ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"];
        git(&repo, &[&c[..], &["add", "-A"]].concat());
        git(&repo, &[&c[..], &["commit", "-qm", "f"]].concat());
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let mut settings =
            Settings { theme, repos: vec![repo.clone()], current: Some(repo.clone()), ..Settings::default() };
        settings.project_jdks.insert(repo.clone(), jdk);
        cx.update(|cx| snout_app::init(&settings, cx));
        cx.update(|cx| cx.set_global(snout_app::lang::hub::LangEnabled(true)));
        let model = cx.update(|cx| {
            let settings = settings.clone();
            cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
        });
        let mut ws = None;
        let handle = cx
            .open_window(size(px(1400.), px(900.)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        let ws = ws.unwrap();
        cx.allow_parking();
        let pump = |cx: &mut HeadlessAppContext, secs: u64, done: &dyn Fn(&mut HeadlessAppContext) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(secs);
            while std::time::Instant::now() < deadline {
                cx.advance_clock(Duration::from_millis(200));
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                })
                .unwrap();
                if done(cx) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            false
        };
        let save = |cx: &mut HeadlessAppContext, name: &str| {
            let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
            if let Some(out) = std::env::var_os("SNOUT_SCREENSHOT") {
                let out = PathBuf::from(out);
                std::fs::create_dir_all(&out).unwrap();
                image.save(out.join(format!("{name}-{mode}.png"))).unwrap();
            }
            println!("screenshot {name}-{mode}: ok");
        };
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_activity(Activity::Project, window, cx));
        })
        .unwrap();
        let file = repo.join("src/main/java/demo/shop/Shop.java");
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(file.clone(), Some((22, 0))), cx)));
        let hub = cx.update(|cx| ws.read(cx).hub.clone());
        let r = repo.clone();
        assert!(
            pump(&mut cx, 300, &|cx| cx.update(|cx| hub
                .read(cx)
                .servers
                .get(&r)
                .is_some_and(|s| s.first_diagnostics.is_some()))),
            "the engine is ready"
        );
        let editor = cx
            .update(|cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file))
            .expect("Shop.java is open");
        let original = cx.update(|cx| editor.read(cx).text(cx));
        // Select `find` (its `nth` match) and run `then` on the editor.
        let on_selection = |cx: &mut HeadlessAppContext,
                            find: &str,
                            nth: usize,
                            then: &dyn Fn(
            &mut snout_app::editor::EditorPanel,
            &mut gpui::Window,
            &mut gpui::Context<snout_app::editor::EditorPanel>,
        )| {
            cx.update_window(handle.into(), |_, window, cx| {
                editor.update(cx, |e, cx| {
                    let t = e.text(cx);
                    let at = t.match_indices(find).nth(nth).map(|(i, _)| i).expect("the text is there");
                    e.state.update(cx, |s, cx| {
                        s.set_selected_range(at..at + find.len(), cx);
                        s.focus(window, cx);
                    });
                    then(e, window, cx);
                })
            })
            .unwrap();
        };
        let put_back = |cx: &mut HeadlessAppContext| {
            cx.update_window(handle.into(), |_, window, cx| {
                editor.update(cx, |e, cx| {
                    e.rename = None;
                    e.choices = None;
                    e.state.update(cx, |s, cx| s.set_value(original.clone(), window, cx));
                })
            })
            .unwrap();
        };
        use snout_app::editor::Refactoring;

        // Parameter name hints after the lines of `main`, whose arguments are literals.
        pump(&mut cx, 60, &|cx| cx.update(|cx| !editor.read(cx).param_hints.is_empty()));
        cx.update_window(handle.into(), |_, window, cx| {
            editor.update(cx, |e, cx| e.go_to(40, 8, window, cx));
        })
        .unwrap();
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-param-hints");

        // The Structure window, the caret in `total()`; then the File Structure popup on
        // `label`, typed into.
        let caret_on = |cx: &mut HeadlessAppContext, line: u32, column: u32| {
            cx.update_window(handle.into(), |_, window, cx| {
                editor.update(cx, |e, cx| e.go_to(line, column, window, cx));
            })
            .unwrap();
        };
        caret_on(&mut cx, 25, 12);
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.toggle_tool(snout_app::workspace::Tool::Structure, window, cx));
        })
        .unwrap();
        let structure = cx.update(|cx| ws.read(cx).structure.clone());
        pump(&mut cx, 30, &|cx| cx.update(|cx| structure.read(cx).current.is_some()));
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-structure");
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.toggle_tool(snout_app::workspace::Tool::Structure, window, cx));
        })
        .unwrap();
        caret_on(&mut cx, 32, 12);
        cx.update_window(handle.into(), |_, window, cx| ws.update(cx, |w, cx| w.open_file_structure(window, cx)))
            .unwrap();
        pump(&mut cx, 30, &|cx| {
            cx.update(|cx| ws.read(cx).search.as_ref().is_some_and(|s| !s.read(cx).items.is_empty()))
        });
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-file-structure");
        cx.update(|cx| {
            ws.update(cx, |w, cx| {
                w.search = None;
                cx.notify();
            })
        });

        // Type Hierarchy of `Shape` (from `List<Shape>`), then Call Hierarchy of `area()`
        // (from `s.area()`) with `total()` opened to its callers.
        use snout_app::lang::hierarchy::Mode;
        let hierarchy = cx.update(|cx| ws.read(cx).hierarchy.clone());
        caret_on(&mut cx, 8, 24);
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_hierarchy(Mode::Subtypes, window, cx))
        })
        .unwrap();
        pump(&mut cx, 30, &|cx| cx.update(|cx| hierarchy.read(cx).visible().len() >= 3));
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-type-hierarchy");
        caret_on(&mut cx, 25, 23);
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_hierarchy(Mode::Callers, window, cx))
        })
        .unwrap();
        pump(&mut cx, 30, &|cx| cx.update(|cx| hierarchy.read(cx).nodes.iter().filter(|n| n.depth == 1).count() >= 2));
        let total = cx.update(|cx| hierarchy.read(cx).nodes.iter().position(|n| n.name.starts_with("total")));
        if let Some(total) = total {
            cx.update(|cx| hierarchy.update(cx, |h, cx| h.expand(total, cx)));
            pump(&mut cx, 20, &|cx| cx.update(|cx| hierarchy.read(cx).nodes.iter().any(|n| n.depth == 2)));
        }
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-call-hierarchy");
        cx.update_window(handle.into(), |_, window, cx| ws.update(cx, |w, cx| w.open_classes(window, cx))).unwrap();
        let search = cx.update(|cx| ws.read(cx).search.clone()).expect("Go to Class");
        cx.update_window(handle.into(), |_, window, cx| search.update(cx, |s, cx| s.set_query("Shap", window, cx)))
            .unwrap();
        pump(&mut cx, 30, &|cx| cx.update(|cx| search.read(cx).items.len() >= 3));
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-go-to-class");
        cx.update(|cx| {
            ws.update(cx, |w, cx| {
                w.search = None;
                cx.notify();
            })
        });
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.toggle_tool(snout_app::workspace::Tool::Hierarchy, window, cx));
        })
        .unwrap();

        // Extract Method on the loop of `total()`: the new method, its name in the field.
        // Right after the start the engine may still be indexing: ask until it answers.
        for _ in 0..10 {
            on_selection(
                &mut cx,
                "for (Shape s : shapes) {\n            sum += s.area();\n        }",
                0,
                &|e, w, cx| e.refactor(Refactoring::ExtractMethod, w, cx),
            );
            if pump(&mut cx, 6, &|cx| cx.update(|cx| editor.read(cx).rename.is_some())) {
                break;
            }
        }
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-extract-method");
        put_back(&mut cx);

        // Extract Variable on an expression used twice: which occurrences to replace.
        cx.update_window(handle.into(), |_, window, cx| {
            let twice = original.replace("int tax = 7;", "int tax = 7 * 6;\n        int fee = 7 * 6;");
            editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.set_value(twice, window, cx)));
        })
        .unwrap();
        pump(&mut cx, 1, &|_| false);
        on_selection(&mut cx, "7 * 6", 0, &|e, w, cx| e.refactor(Refactoring::ExtractVariable, w, cx));
        pump(&mut cx, 30, &|cx| cx.update(|cx| editor.read(cx).choices.is_some()));
        pump(&mut cx, 1, &|_| false);
        save(&mut cx, "java-extract-choices");
        put_back(&mut cx);
        pump(&mut cx, 1, &|_| false);

        cx.update(|cx| hub.update(cx, |h, _| h.shutdown_all()));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The debugger stopped on a breakpoint in the engine's run-app fixture, for real: the
    /// engine (`engine/build/dist`, or `SNOUT_ENGINE_DIST`) runs and debugs it. Only with
    /// `SNOUT_SHOT_DEBUG=<JDK 17 to 21 home>`.
    fn real_debug_shot(name: &str, theme: ThemeChoice) {
        let Some(jdk) = std::env::var_os("SNOUT_SHOT_DEBUG").map(PathBuf::from) else { return };
        let dir = std::env::temp_dir().join(format!("snout-app-shot-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/sync-cli/src/test/fixtures/run-app");
        assert!(Command::new("cp").arg("-R").arg(&from).arg(&dir).status().unwrap().success());
        let repo = dir.join("run-app").canonicalize().unwrap();
        std::fs::write(repo.join("gradle.properties"), "org.gradle.daemon.idletimeout=60000\n").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["-c", "user.name=t", "-c", "user.email=t@example.com", "add", "-A"]);
        git(
            &repo,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "f",
            ],
        );
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let mut settings =
            Settings { theme, repos: vec![repo.clone()], current: Some(repo.clone()), ..Settings::default() };
        settings.project_jdks.insert(repo.clone(), jdk);
        cx.update(|cx| snout_app::init(&settings, cx));
        cx.update(|cx| cx.set_global(snout_app::lang::hub::LangEnabled(true)));
        let model = cx.update(|cx| {
            let settings = settings.clone();
            cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
        });
        let mut ws = None;
        let handle = cx
            .open_window(size(px(1400.), px(900.)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        let ws = ws.unwrap();
        cx.allow_parking();
        let pump = |cx: &mut HeadlessAppContext, secs: u64, done: &dyn Fn(&mut HeadlessAppContext) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(secs);
            while std::time::Instant::now() < deadline {
                cx.advance_clock(Duration::from_millis(200));
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                })
                .unwrap();
                if done(cx) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            false
        };
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_activity(Activity::Project, window, cx));
        })
        .unwrap();
        let runs = cx.update(|cx| ws.read(cx).runs.clone());
        let r2 = runs.clone();
        assert!(
            pump(&mut cx, 600, &|cx| cx.update(|cx| !r2.read(cx).engine_configs.is_empty())),
            "the engine is ready"
        );
        let file = repo.join("app/src/main/java/demo/app/Main.java");
        let line =
            std::fs::read_to_string(&file).unwrap().lines().position(|l| l.contains("total = add(total, i);")).unwrap()
                as u32;
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(file.clone(), Some((line, 0))), cx)));
        cx.update(|cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, line, cx)));
        let main = cx.update(|cx| runs.read(cx).configurations().into_iter().find(|c| c.main_class.is_some()).unwrap());
        cx.update(|cx| {
            runs.update(cx, |r, cx| r.start(snout_app::run::Launch::Config { config: main, debug: true }, cx))
        });
        let stopped = pump(&mut cx, 300, &|cx| {
            cx.update(|cx| {
                let m = snout_app::run::marks(cx).unwrap();
                m.read(cx).exec.is_some() && !m.read(cx).locals.is_empty()
            })
        });
        assert!(stopped, "the breakpoint is hit");
        // One step, so the loop has gone round once.
        let id = cx.update(|cx| runs.read(cx).sessions.last().unwrap().id);
        cx.update(|cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
        pump(&mut cx, 30, &|cx| {
            cx.update(|cx| snout_app::run::marks(cx).unwrap().read(cx).locals.iter().any(|(n, v)| n == "i" && v == "2"))
        });
        cx.update(|cx| runs.update(cx, |r, cx| r.add_watch(id, "total * 10".into(), cx)));
        pump(&mut cx, 3, &|_| false);
        let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
        if let Some(out) = std::env::var_os("SNOUT_SCREENSHOT") {
            let out = PathBuf::from(out);
            std::fs::create_dir_all(&out).unwrap();
            image.save(out.join(format!("{name}.png"))).unwrap();
        }
        cx.update(|cx| runs.update(cx, |r, cx| r.stop(id, cx)));
        pump(&mut cx, 10, &|cx| cx.update(|cx| !runs.read(cx).any_running()));
        let hub = cx.update(|cx| ws.read(cx).hub.clone());
        cx.update(|cx| hub.update(cx, |h, _| h.shutdown_all()));
        let _ = std::fs::remove_dir_all(dir);
        println!("screenshot {name}: ok");
    }

    /// A Spring Boot application of a real project, run and debugged the way a user does it,
    /// for real. Only with `SNOUT_SHOT_BOOT=<the project's folder>` and
    /// `SNOUT_SHOT_BOOT_JDK=<JDK home for its Gradle>`; with `SNOUT_SHOT_BOOT_FIRST=<a folder
    /// above it>`, that folder is the project first (its engine runs, the Project view
    /// opens on it), and the application's folder replaces it as another project would.
    /// `SNOUT_SHOT_BOOT_MAIN` names the main class's file under the project. Then: the
    /// engine of the first project is gone, the run icons and the title bar's configuration
    /// show, the application runs from its `main` method's icon and says so in the Run
    /// window, and under the debugger it stops in its `main` method. Shots of each step.
    fn real_boot_shot(theme: ThemeChoice) {
        let (Some(folder), Some(jdk)) = (std::env::var_os("SNOUT_SHOT_BOOT"), std::env::var_os("SNOUT_SHOT_BOOT_JDK"))
        else {
            return;
        };
        let folder = PathBuf::from(folder).canonicalize().unwrap();
        let first = std::env::var_os("SNOUT_SHOT_BOOT_FIRST").map(|f| PathBuf::from(f).canonicalize().unwrap());
        let main_file = folder.join(
            std::env::var("SNOUT_SHOT_BOOT_MAIN")
                .unwrap_or_else(|_| "src/main/java/com/solarpanda/bamboo/Application.java".into()),
        );
        let jdk = PathBuf::from(jdk);
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let opened = first.clone().unwrap_or_else(|| folder.clone());
        let mut settings = Settings { theme, current: Some(opened.clone()), ..Settings::default() };
        settings.project_jdks.insert(folder.clone(), jdk.clone());
        settings.project_jdks.insert(opened.clone(), jdk);
        cx.update(|cx| snout_app::init(&settings, cx));
        cx.update(|cx| cx.set_global(snout_app::lang::hub::LangEnabled(true)));
        let model = cx.update(|cx| {
            let settings = settings.clone();
            cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
        });
        let mut ws = None;
        let handle = cx
            .open_window(size(px(1400.), px(900.)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        let ws = ws.unwrap();
        cx.allow_parking();
        let pump = |cx: &mut HeadlessAppContext, secs: u64, done: &dyn Fn(&mut HeadlessAppContext) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(secs);
            while std::time::Instant::now() < deadline {
                cx.advance_clock(Duration::from_millis(200));
                cx.run_until_parked();
                cx.update_window(handle.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                })
                .unwrap();
                if done(cx) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            false
        };
        let save = |cx: &mut HeadlessAppContext, name: &str| {
            let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
            if let Some(out) = std::env::var_os("SNOUT_SCREENSHOT") {
                let out = PathBuf::from(out);
                std::fs::create_dir_all(&out).unwrap();
                image.save(out.join(format!("{name}.png"))).unwrap();
            }
            println!("screenshot {name}: ok");
        };
        cx.update_window(handle.into(), |_, window, cx| {
            ws.update(cx, |w, cx| w.show_activity(Activity::Project, window, cx));
        })
        .unwrap();
        let runs = cx.update(|cx| ws.read(cx).runs.clone());
        let hub = cx.update(|cx| ws.read(cx).hub.clone());
        let r2 = runs.clone();
        let ready = move |cx: &mut HeadlessAppContext| cx.update(|cx| !r2.read(cx).engine_configs.is_empty());
        assert!(pump(&mut cx, 900, &ready), "the engine of {} is ready", opened.display());
        if first.is_some() {
            save(&mut cx, "boot-first-dark");
            let before: Vec<u32> = cx.update(|cx| hub.read(cx).server_pids());
            // Another project in its place, as Open does it.
            cx.update(|cx| model.update(cx, |m, cx| m.open_arg(&folder, cx)));
            // A moment in: the views follow the new project at once.
            pump(&mut cx, 8, &|_| false);
            save(&mut cx, "boot-switching-dark");
            let (h2, f2) = (hub.clone(), folder.clone());
            let switched = pump(&mut cx, 900, &|cx| {
                cx.update(|cx| {
                    let h = h2.read(cx);
                    h.servers.len() == 1 && h.servers.contains_key(&f2) && h.client(&f2).is_some()
                }) && ready(cx)
            });
            assert!(switched, "only the new project's engine runs, and it is ready");
            let gone = pump(&mut cx, 30, &|_| before.iter().all(|pid| unsafe { libc::kill(*pid as i32, 0) } != 0));
            assert!(gone, "the first project's engine stopped: {before:?}");
            let r3 = runs.clone();
            let f3 = folder.clone();
            assert!(
                pump(&mut cx, 600, &|cx| cx.update(|cx| {
                    let r = r3.read(cx);
                    !r.gradle.builds.is_empty() && r.gradle.builds.iter().all(|b| b.root_dir.starts_with(&f3))
                })),
                "the Gradle window lists the new project's builds only"
            );
            save(&mut cx, "boot-switched-dark");
        }
        // The main class: run icons on its class and on its main method.
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(main_file.clone(), None), cx)));
        let editor =
            cx.update(|cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == main_file).unwrap());
        let e2 = editor.clone();
        assert!(
            pump(&mut cx, 120, &|cx| cx.update(|cx| e2
                .read(cx)
                .runnables
                .as_ref()
                .is_some_and(|r| r.items.len() >= 2))),
            "run icons on the class and its main method"
        );
        let text = std::fs::read_to_string(&main_file).unwrap();
        let main_line = text.lines().position(|l| l.contains("void main(")).unwrap() as u32;
        let (target, module) = cx.update(|cx| {
            let r = editor.read(cx).runnables.clone().unwrap();
            let item = r.items.iter().find(|i| i.line == main_line).expect("an icon on main").clone();
            (item.target, r.module.clone())
        });
        let selected = cx.update(|cx| runs.read(cx).selected_config()).expect("a configuration in the title bar");
        assert!(selected.spring_boot, "the Spring Boot application is the title bar's: {selected:?}");
        // Run it from the icon on main: the Run window shows the command and its output.
        cx.update(|cx| runs.update(cx, |r, cx| r.run_target(&target, module.as_deref(), false, cx)));
        let id = cx.update(|cx| runs.read(cx).sessions.last().unwrap().id);
        let r4 = runs.clone();
        let output = move |cx: &mut HeadlessAppContext| {
            cx.update(|cx| {
                r4.read(cx).session(id).map(|s| s.lines.iter().map(|l| l.text()).collect::<Vec<_>>().join("\n"))
            })
            .unwrap_or_default()
        };
        let o2 = output.clone();
        let started = pump(&mut cx, 600, &|cx| o2(cx).contains("Starting Application") || o2(cx).contains("Started "));
        assert!(started, "the application starts: {}", output(&mut cx));
        pump(&mut cx, 20, &|_| false);
        let text_now = output(&mut cx);
        for line in text_now.lines().take(5) {
            println!("boot run: {}", line.chars().take(200).collect::<String>());
        }
        assert!(text_now.lines().any(|l| l.contains("bin/java") && l.contains("@")), "the command shows: {text_now}");
        assert!(
            !text_now.contains("ClassNotFoundException") && !text_now.contains("NoClassDefFoundError"),
            "every class is on the classpath: {text_now}"
        );
        save(&mut cx, "boot-run-dark");
        cx.update(|cx| runs.update(cx, |r, cx| r.stop(id, cx)));
        let r5 = runs.clone();
        assert!(pump(&mut cx, 60, &|cx| cx.update(|cx| !r5.read(cx).any_running())), "it stops");
        // Debug it: a breakpoint where main starts the application stops it there.
        let line = text.lines().position(|l| l.contains("SpringApplication.run")).unwrap() as u32;
        cx.update(|cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&main_file, line, cx)));
        cx.update(|cx| runs.update(cx, |r, cx| r.run_target(&target, module.as_deref(), true, cx)));
        let id = cx.update(|cx| runs.read(cx).sessions.last().unwrap().id);
        let f4 = main_file.clone();
        let stopped = pump(&mut cx, 600, &|cx| {
            cx.update(|cx| snout_app::run::marks(cx).unwrap().read(cx).exec == Some((f4.clone(), line)))
        });
        assert!(stopped, "the breakpoint in main is hit");
        pump(&mut cx, 5, &|_| false);
        save(&mut cx, "boot-debug-dark");
        cx.update(|cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&main_file, line, cx)));
        cx.update(|cx| runs.update(cx, |r, cx| r.stop(id, cx)));
        let r6 = runs.clone();
        pump(&mut cx, 60, &|cx| cx.update(|cx| !r6.read(cx).any_running()));
        cx.update(|cx| hub.update(cx, |h, _| h.shutdown_all()));
    }

    /// Spring on a real project, for real: the engine reads it, the window shows its beans
    /// and endpoints, the gutter and the `@Value` hints come from the engine, and
    /// completion in its `application.properties`. Only with `SNOUT_SHOT_SPRING=<project>`
    /// and `SNOUT_SHOT_SPRING_JDK=<JDK home for its Gradle>`; `SNOUT_SHOT_SPRING_FILES`
    /// names, separated by commas, the controller, the `@Value` class and the properties
    /// file under it (defaults fit a project with one of each). The project is only read:
    /// nothing is saved.
    fn real_spring_shot(theme: ThemeChoice) {
        use gpui_component::input::RopeExt as _;
        let (Some(folder), Some(jdk)) =
            (std::env::var_os("SNOUT_SHOT_SPRING"), std::env::var_os("SNOUT_SHOT_SPRING_JDK"))
        else {
            return;
        };
        let folder = PathBuf::from(folder).canonicalize().unwrap();
        let files: Vec<PathBuf> = std::env::var("SNOUT_SHOT_SPRING_FILES")
            .unwrap_or_default()
            .split(',')
            .filter(|f| !f.is_empty())
            .map(|f| folder.join(f))
            .collect();
        assert_eq!(files.len(), 3, "SNOUT_SHOT_SPRING_FILES names the controller, a @Value class and the properties");
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let mut settings =
            Settings { theme, repos: vec![folder.clone()], current: Some(folder.clone()), ..Settings::default() };
        settings.project_jdks.insert(folder.clone(), PathBuf::from(jdk));
        cx.update(|cx| snout_app::init(&settings, cx));
        cx.update(|cx| cx.set_global(snout_app::lang::hub::LangEnabled(true)));
        let model = cx.update(|cx| {
            let settings = settings.clone();
            cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
        });
        let mut ws = None;
        let handle = cx
            .open_window(size(px(1400.), px(900.)), |window, cx| {
                let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
                ws = Some(view.clone());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        let ws = ws.unwrap();
        let any: gpui::AnyWindowHandle = handle.into();
        cx.allow_parking();
        let pump = |cx: &mut HeadlessAppContext, secs: u64, done: &dyn Fn(&mut HeadlessAppContext) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(secs);
            while std::time::Instant::now() < deadline {
                cx.advance_clock(Duration::from_millis(200));
                cx.run_until_parked();
                cx.update_window(any, |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                })
                .unwrap();
                if done(cx) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            false
        };
        let snap = |cx: &mut HeadlessAppContext, name: &str| {
            let image = cx.capture_screenshot(any).expect("offscreen rendering must be available");
            if let Some(out) = std::env::var_os("SNOUT_SCREENSHOT") {
                let out = PathBuf::from(out);
                std::fs::create_dir_all(&out).unwrap();
                image.save(out.join(format!("{name}.png"))).unwrap();
            }
            println!("screenshot {name}: ok");
        };
        cx.update_window(any, |_, window, cx| {
            ws.update(cx, |w, cx| w.show_activity(Activity::Project, window, cx));
        })
        .unwrap();
        let open = |cx: &mut HeadlessAppContext, path: &Path, at: (u32, u32)| {
            cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenFile(path.to_owned(), Some(at)), cx)))
        };
        // The controller: its beans, injections and endpoints in the gutter.
        let controller = &files[0];
        let text = std::fs::read_to_string(controller).unwrap();
        let first = text.lines().position(|l| l.contains("@Autowired")).unwrap_or(0) as u32;
        open(&mut cx, controller, (first, 0));
        let spring = cx.update(|cx| ws.read(cx).spring.clone());
        let started = std::time::Instant::now();
        assert!(pump(&mut cx, 1800, &|cx| cx.update(|cx| spring.read(cx).loaded)), "the Spring model is read");
        let (beans, endpoints) = cx.update(|cx| (spring.read(cx).bean_count(), spring.read(cx).endpoints.len()));
        println!("spring: {beans} beans, {endpoints} endpoints, after {:.0} s", started.elapsed().as_secs_f32());
        let editor = editor_named(&ws, controller.file_name().unwrap().to_str().unwrap(), &mut cx);
        let e2 = editor.clone();
        assert!(
            pump(&mut cx, 120, &|cx| cx.update(|cx| e2.read(cx).spring.as_ref().is_some_and(|s| !s.lines.is_empty()))),
            "the controller's marks"
        );
        let marks = cx.update(|cx| {
            let s = editor.read(cx).spring.clone().unwrap();
            s.lines.iter().map(|l| format!("{}:{:?}", l.line + 1, l.mark().unwrap())).collect::<Vec<_>>()
        });
        println!("spring: {} marks in {}: {}", marks.len(), controller.display(), marks.join(" "));
        // The Beans tab beside it, and a service's tooltip.
        cx.update_window(any, |_, window, cx| {
            ws.update(cx, |ws, cx| ws.toggle_tool(snout_app::workspace::Tool::Spring, window, cx));
        })
        .unwrap();
        pump(&mut cx, 2, &|_| false);
        let at = spring_lane(&editor, "@Autowired", &mut cx);
        cx.update(|cx| {
            editor.update(cx, |e, cx| {
                let line = e.spring.as_ref().unwrap().line(first).cloned();
                if let Some(line) = line {
                    e.gutter_tip = Some((first, at, line.tooltip()));
                }
                cx.notify();
            })
        });
        pump(&mut cx, 1, &|_| false);
        snap(&mut cx, "spring-real-gutter-beans");
        // An injection's mark goes to its bean; the bean's lists where it is injected.
        cx.update(|cx| editor.update(cx, |e, _| e.gutter_tip = None));
        let target = cx.update(|cx| {
            let s = editor.read(cx).spring.clone().unwrap();
            s.line(first).and_then(|l| l.injections.first()).and_then(|i| i.beans.first()).map(|b| b.target())
        });
        if let Some(target) = target {
            cx.update(|cx| spring.update(cx, |s, cx| s.open(target.clone(), cx)));
            let (path, line) = target.file().unwrap();
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            assert!(
                pump(&mut cx, 120, &|cx| cx.update(|cx| {
                    ws.read(cx).editors(cx).iter().any(|e| {
                        e.read(cx).path == path && e.read(cx).spring.as_ref().is_some_and(|s| s.line(line).is_some())
                    })
                })),
                "the bean's file opens with its marks"
            );
            let service = editor_named(&ws, &name, &mut cx);
            let bean = cx.update(|cx| service.read(cx).spring.as_ref().unwrap().line(line).cloned().unwrap());
            println!("spring: {name}:{} {}", line + 1, bean.tooltip().replace('\n', "; "));
            let at = cx.update(|cx| {
                let e = service.read(cx);
                let (a, z) = e.lanes.get().spring();
                let s = e.state.read(cx);
                let off = s.text().line_start_offset(line as usize);
                let b = s.range_to_bounds(&(off..off)).unwrap();
                gpui::point((a + z) / 2., b.center().y)
            });
            cx.update(|cx| {
                service.update(cx, |e, cx| {
                    let items = snout_app::editor::spring_items(&bean);
                    e.gutter_menu = Some(snout_app::editor::GutterMenu::Spring { at, items, filter: None });
                    cx.notify();
                })
            });
            pump(&mut cx, 1, &|_| false);
            snap(&mut cx, "spring-real-bean-menu");
            cx.update(|cx| service.update(cx, |e, _| e.gutter_menu = None));
        }
        // The Endpoints tab, searching.
        let win = cx.update(|cx| ws.read(cx).spring_window.clone());
        cx.update_window(any, |_, window, cx| {
            win.update(cx, |w, cx| w.show_endpoints(Some("logistic".into()), window, cx));
        })
        .unwrap();
        let shown = cx.update(|cx| win.read(cx).endpoint_rows(cx).len());
        println!("spring: {shown} endpoints match 'logistic'");
        pump(&mut cx, 1, &|_| false);
        snap(&mut cx, "spring-real-endpoints");
        // The @Value hints.
        let values = &files[1];
        let text = std::fs::read_to_string(values).unwrap();
        let line = text.lines().position(|l| l.contains("@Value")).unwrap_or(0) as u32;
        open(&mut cx, values, (line, 0));
        let editor = editor_named(&ws, values.file_name().unwrap().to_str().unwrap(), &mut cx);
        let e2 = editor.clone();
        assert!(pump(&mut cx, 120, &|cx| cx.update(|cx| !e2.read(cx).hints.is_empty())), "the @Value hints");
        let hints = cx.update(|cx| editor.read(cx).hints.len());
        println!("spring: {hints} @Value hints in {}", values.display());
        snap(&mut cx, "spring-real-value-hints");
        // Completion in application.properties, typed but never saved.
        let props = &files[2];
        let lines = std::fs::read_to_string(props).unwrap().lines().count() as u32;
        open(&mut cx, props, (lines, 0));
        let editor = editor_named(&ws, props.file_name().unwrap().to_str().unwrap(), &mut cx);
        let e2 = editor.clone();
        pump(&mut cx, 60, &|cx| cx.update(|cx| !e2.read(cx).problems.is_empty()));
        let warnings = cx.update(|cx| editor.read(cx).problems.len());
        println!("spring: {warnings} warnings in {}", props.display());
        cx.update_window(any, |_, window, cx| {
            editor.update(cx, |e, cx| {
                e.state.update(cx, |s, cx| {
                    let end = s.text().len();
                    s.set_selected_range(end..end, cx);
                    s.focus(window, cx);
                    s.insert("\nspring.servlet.multipart.", window, cx);
                })
            })
        })
        .unwrap();
        pump(&mut cx, 3, &|_| false);
        cx.update_window(any, |_, window, cx| {
            editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.show_completions(window, cx)))
        })
        .unwrap();
        let e2 = editor.clone();
        let completed = pump(&mut cx, 60, &|cx| {
            cx.update(|cx| !e2.read(cx).state.read(cx).completion_menu_state().items.is_empty())
        });
        let items = cx.update(|cx| editor.read(cx).state.read(cx).completion_menu_state().items.len());
        println!("spring: completion offers {items} keys ({completed})");
        snap(&mut cx, "spring-real-properties");
        // Nothing is saved: the tab is put back as the file is.
        cx.update_window(any, |_, window, cx| editor.update(cx, |e, cx| e.reload(window, cx))).unwrap();
        let hub = cx.update(|cx| ws.read(cx).hub.clone());
        cx.update(|cx| hub.update(cx, |h, _| h.shutdown_all()));
    }

    /// The splash window on its own.
    /// ssh's question in SnoutCode's dialog (the askpass helper): a password, or a host key.
    fn askpass_shot(name: &str, theme: ThemeChoice, prompt: &'static str) {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let settings = Settings { theme, ..Settings::default() };
        cx.update(|cx| snout_app::init(&settings, cx));
        let handle = cx
            .open_window(size(px(460.), px(if prompt.contains("yes/no") { 270. } else { 210. })), |window, cx| {
                let view = cx.new(|cx| snout_app::remote::askpass::Askpass::new(prompt.into(), window, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        cx.allow_parking();
        for _ in 0..3 {
            cx.advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
        let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
        let first = image.get_pixel(0, 0);
        assert!(image.pixels().any(|p| p != first), "{name}: the dialog is blank");
        if let Some(dir) = std::env::var_os("SNOUT_SCREENSHOT") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            image.save(dir.join(format!("{name}.png"))).unwrap();
        }
        println!("screenshot {name}: ok");
    }

    fn splash_shot(name: &str, theme: ThemeChoice) {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let settings = Settings { theme, ..Settings::default() };
        cx.update(|cx| snout_app::init(&settings, cx));
        let handle = cx
            .open_window(size(px(snout_app::splash::SPLASH_W), px(snout_app::splash::SPLASH_H)), |_, cx| {
                cx.new(|_| snout_app::splash::Splash)
            })
            .unwrap();
        cx.allow_parking();
        for _ in 0..3 {
            cx.advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
        let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
        let first = image.get_pixel(0, 0);
        assert!(image.pixels().any(|p| p != first), "{name}: the splash is blank");
        if let Some(dir) = std::env::var_os("SNOUT_SCREENSHOT") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            image.save(dir.join(format!("{name}.png"))).unwrap();
        }
        println!("screenshot {name}: ok");
    }

    /// The shared components and every spot drawing, on one page.
    fn gallery_shot(name: &str, colors: ColorTheme, theme: ThemeChoice) {
        if std::env::var("SNOUT_SHOT_ONLY").is_ok_and(|only| !name.contains(&only)) {
            return;
        }
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let settings = Settings { theme, colors, ..Settings::default() };
        cx.update(|cx| snout_app::init(&settings, cx));
        let handle = cx
            .open_window(size(px(1500.), px(1060.)), |window, cx| {
                let view = cx.new(|_| snout_app::components::Gallery);
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        cx.allow_parking();
        for _ in 0..3 {
            cx.advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
        let image = cx.capture_screenshot(handle.into()).expect("offscreen rendering must be available");
        let first = image.get_pixel(0, 0);
        assert!(image.pixels().any(|p| p != first), "{name}: the gallery is blank");
        if let Some(dir) = std::env::var_os("SNOUT_SCREENSHOT") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            image.save(dir.join(format!("{name}.png"))).unwrap();
        }
        println!("screenshot {name}: ok");
    }

    pub fn run() {
        splash_shot("splash-dark", ThemeChoice::Dark);
        splash_shot("splash-light", ThemeChoice::Light);
        // The Git view, as the egui app's `renders_repo_and_diff` and
        // `clicking_a_commit_shows_what_it_changed` shots show it.
        shot("git-dark", ThemeChoice::Dark, Activity::Git, Some(Select::File("status.rs")));
        shot("git-light", ThemeChoice::Light, Activity::Git, Some(Select::File("status.rs")));
        shot("commit-dark", ThemeChoice::Dark, Activity::Git, Some(Select::Commit("Initial import", "status.rs")));
        shot("console-light", ThemeChoice::Light, Activity::Git, Some(Select::Console));
        // The same colourful output under each terminal engine this build has.
        for engine in snout_core::term::EngineKind::available() {
            let name = engine.label().to_lowercase().replace('-', "");
            engine_shot(
                engine,
                &format!("terminal-colors-{name}-dark"),
                ThemeChoice::Dark,
                Some(Select::TerminalColors),
            );
            engine_shot(
                engine,
                &format!("terminal-colors-{name}-light"),
                ThemeChoice::Light,
                Some(Select::TerminalColors),
            );
        }
        shot(
            "settings-terminal-light",
            ThemeChoice::Light,
            Activity::Project,
            Some(Select::Settings(snout_app::settings_view::Page::Terminal)),
        );
        shot("welcome-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Nothing));
        shot("git-empty-dark", ThemeChoice::Dark, Activity::Git, Some(Select::Nothing));
        shot("project-dark", ThemeChoice::Dark, Activity::Project, None);
        shot("project-model-dark", ThemeChoice::Dark, Activity::Project, Some(Select::ProjectModel));
        shot("project-model-light", ThemeChoice::Light, Activity::Project, Some(Select::ProjectModel));
        real_project_shot("project-real-dark", ThemeChoice::Dark);
        real_debug_shot("debug-real-dark", ThemeChoice::Dark);
        real_java_shots(ThemeChoice::Dark);
        real_boot_shot(ThemeChoice::Dark);
        shot("editor-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Editor));
        shot("editor-light", ThemeChoice::Light, Activity::Project, Some(Select::Editor));
        shot("palette-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Editor));
        shot("editor-menu-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Editor));
        shot("editor-menu-light", ThemeChoice::Light, Activity::Project, Some(Select::Editor));
        themed_shot(
            ColorTheme::Vivid,
            "vivid-editor-menu-light",
            ThemeChoice::Light,
            Activity::Project,
            Some(Select::Editor),
        );
        use snout_app::settings_view::Page;
        shot("settings-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Settings(Page::Appearance)));
        shot("settings-light", ThemeChoice::Light, Activity::Project, Some(Select::Settings(Page::Editor)));
        shot("keymap-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Keymap));
        shot("keymap-light", ThemeChoice::Light, Activity::Project, Some(Select::Keymap));
        shot("new-project-java-dark", ThemeChoice::Dark, Activity::Project, Some(Select::NewProject(0)));
        shot("new-project-java-light", ThemeChoice::Light, Activity::Project, Some(Select::NewProject(0)));
        shot("new-project-spring-dark", ThemeChoice::Dark, Activity::Project, Some(Select::NewProject(1)));
        shot("new-project-deps-dark", ThemeChoice::Dark, Activity::Project, Some(Select::NewProject(2)));
        shot("remote-connect-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Remote(0)));
        shot("remote-connect-light", ThemeChoice::Light, Activity::Project, Some(Select::Remote(0)));
        shot("remote-picker-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Remote(1)));
        shot("remote-reconnecting-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Remote(2)));
        shot("remote-reconnecting-light", ThemeChoice::Light, Activity::Project, Some(Select::Remote(2)));
        askpass_shot("remote-askpass-dark", ThemeChoice::Dark, "dev@devbox's password:");
        askpass_shot(
            "remote-askpass-hostkey-light",
            ThemeChoice::Light,
            "The authenticity of host 'devbox (10.0.0.5)' can't be established.\nED25519 key fingerprint is SHA256:4y1k8sV0Q9.\nAre you sure you want to continue connecting (yes/no/[fingerprint])?",
        );
        shot("tasks-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Tasks));
        shot("tasks-light", ThemeChoice::Light, Activity::Project, Some(Select::Tasks));
        shot("review-streaming-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Review(Some(0.45))));
        shot("review-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Review(None)));
        shot("review-light", ThemeChoice::Light, Activity::Project, Some(Select::Review(None)));
        shot("claude-live-strike-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Claude(ClaudeShot::Strike)));
        shot("claude-live-typing-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Claude(ClaudeShot::Typing)));
        shot(
            "claude-live-typing-light",
            ThemeChoice::Light,
            Activity::Project,
            Some(Select::Claude(ClaudeShot::Typing)),
        );
        shot("claude-changes-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Claude(ClaudeShot::Fade)));
        shot("claude-changes-light", ThemeChoice::Light, Activity::Project, Some(Select::Claude(ClaudeShot::Fade)));
        shot("claude-inline-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Claude(ClaudeShot::Inline)));
        shot("welcome-light", ThemeChoice::Light, Activity::Project, Some(Select::Nothing));
        shot("run-tests-dark", ThemeChoice::Dark, Activity::Project, Some(Select::RunTests));
        shot("run-tests-light", ThemeChoice::Light, Activity::Project, Some(Select::RunTests));
        shot("debug-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Debug));
        shot("debug-light", ThemeChoice::Light, Activity::Project, Some(Select::Debug));
        shot("gradle-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Gradle));
        shot("processes-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Processes));
        shot("processes-light", ThemeChoice::Light, Activity::Project, Some(Select::Processes));
        shot("settings-gradle-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Settings(Page::Gradle)));
        // Every Settings page, light and dark (`SNOUT_SHOT_ONLY=settings-page-` takes these alone).
        for page in Page::ALL {
            let slug = page.title().to_lowercase().replace(' ', "-");
            for (theme, mode) in [(ThemeChoice::Light, "light"), (ThemeChoice::Dark, "dark")] {
                shot(&format!("settings-page-{slug}-{mode}"), theme, Activity::Project, Some(Select::Settings(page)));
            }
        }
        TERMINAL_SET.set(true);
        shot(
            "settings-page-terminal-set-light",
            ThemeChoice::Light,
            Activity::Project,
            Some(Select::Settings(Page::Terminal)),
        );
        shot(
            "settings-page-terminal-set-preview-light",
            ThemeChoice::Light,
            Activity::Project,
            Some(Select::SettingsScrolled(Page::Terminal, 420.)),
        );
        shot(
            "settings-page-terminal-set-end-dark",
            ThemeChoice::Dark,
            Activity::Project,
            Some(Select::SettingsScrolled(Page::Terminal, 2000.)),
        );
        TERMINAL_SET.set(false);
        for (by, part) in [(560., "middle"), (2000., "end")] {
            for (theme, mode) in [(ThemeChoice::Light, "light"), (ThemeChoice::Dark, "dark")] {
                let name = format!("settings-page-terminal-{part}-{mode}");
                shot(&name, theme, Activity::Project, Some(Select::SettingsScrolled(Page::Terminal, by)));
            }
        }
        // The Terminal page's preview in every colour theme, light and dark: its colours read
        // on each (`SNOUT_SHOT_ONLY=terminal-palette-` takes these alone).
        ENGINE.set(snout_core::term::EngineKind::preferred());
        for colors in ColorTheme::ALL {
            let c = colors.label().to_lowercase();
            for (theme, mode) in [(ThemeChoice::Light, "light"), (ThemeChoice::Dark, "dark")] {
                let name = format!("terminal-palette-{c}-{mode}");
                themed_shot(colors, &name, theme, Activity::Project, Some(Select::Settings(Page::Terminal)));
            }
        }
        ENGINE.set(snout_core::term::EngineKind::Builtin);
        real_spring_shot(ThemeChoice::Dark);
        shot("spring-gutter-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Spring(SpringShot::Gutter)));
        shot("spring-gutter-light", ThemeChoice::Light, Activity::Project, Some(Select::Spring(SpringShot::Gutter)));
        shot("spring-many-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Spring(SpringShot::ManySites)));
        shot("spring-beans-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Spring(SpringShot::Beans)));
        shot(
            "spring-endpoints-dark",
            ThemeChoice::Dark,
            Activity::Project,
            Some(Select::Spring(SpringShot::Endpoints)),
        );
        shot(
            "spring-endpoints-light",
            ThemeChoice::Light,
            Activity::Project,
            Some(Select::Spring(SpringShot::Endpoints)),
        );
        shot(
            "spring-properties-dark",
            ThemeChoice::Dark,
            Activity::Project,
            Some(Select::Spring(SpringShot::Properties)),
        );
        // The dialogs and the empty tool windows (`SNOUT_SHOT_ONLY=ui-` takes these alone).
        for (colors, c) in [(ColorTheme::Studio, "studio"), (ColorTheme::Vivid, "vivid")] {
            gallery_shot(&format!("ui-gallery-{c}-dark"), colors, ThemeChoice::Dark);
            gallery_shot(&format!("ui-gallery-{c}-light"), colors, ThemeChoice::Light);
        }
        use snout_app::workspace::Tool;
        for which in [
            "discard",
            "discard-many",
            "delete-files",
            "delete-branch",
            "new-branch",
            "clone",
            "error",
            "info",
            "pull",
            "checkout",
            "about",
        ] {
            shot(&format!("ui-dialog-{which}-dark"), ThemeChoice::Dark, Activity::Git, Some(Select::Dialog(which)));
            shot(&format!("ui-dialog-{which}-light"), ThemeChoice::Light, Activity::Git, Some(Select::Dialog(which)));
        }
        for (bottom, right, name) in [
            (Tool::Run, Tool::Spring, "run-spring"),
            (Tool::Debug, Tool::Gradle, "debug-gradle"),
            (Tool::Problems, Tool::ClaudeChanges, "problems-claude"),
            (Tool::Processes, Tool::Notifications, "processes-notifications"),
            (Tool::Build, Tool::Spring, "build-spring"),
        ] {
            for (theme, mode) in [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")] {
                shot(&format!("ui-empty-{name}-{mode}"), theme, Activity::Project, Some(Select::Tools(bottom, right)));
            }
        }
        for (theme, mode) in [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")] {
            shot(&format!("ui-empty-clean-{mode}"), theme, Activity::Git, Some(Select::Clean));
            shot(&format!("ui-empty-search-{mode}"), theme, Activity::Project, Some(Select::NoResults));
            shot(&format!("ui-remote-connecting-{mode}"), theme, Activity::Project, Some(Select::Connecting));
            shot(&format!("ui-jdk-bar-{mode}"), theme, Activity::Project, Some(Select::JdkBar));
            shot(&format!("ui-welcome-{mode}"), theme, Activity::Project, Some(Select::Nothing));
            shot(&format!("ui-git-{mode}"), theme, Activity::Git, Some(Select::File("status.rs")));
            themed_shot(
                ColorTheme::Vivid,
                &format!("ui-vivid-dialog-discard-{mode}"),
                theme,
                Activity::Git,
                Some(Select::Dialog("discard")),
            );
            themed_shot(
                ColorTheme::Vivid,
                &format!("ui-vivid-empty-run-spring-{mode}"),
                theme,
                Activity::Project,
                Some(Select::Tools(Tool::Run, Tool::Spring)),
            );
        }
        // The other colour themes, on the editor and the Git view.
        for (colors, name) in
            [(ColorTheme::Vivid, "vivid"), (ColorTheme::Deep, "deep"), (ColorTheme::Classic, "classic")]
        {
            for (theme, mode) in [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")] {
                themed_shot(colors, &format!("{name}-editor-{mode}"), theme, Activity::Project, Some(Select::Editor));
                themed_shot(
                    colors,
                    &format!("{name}-git-{mode}"),
                    theme,
                    Activity::Git,
                    Some(Select::File("status.rs")),
                );
            }
        }
    }
}
