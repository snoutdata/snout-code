//! Screenshots of the whole window, rendered offscreen with GPUI's real renderer (Metal on
//! macOS) and the real text system, as `egui_kittest` snapshots do for the egui app.
//! Runs on the main thread (`harness = false`). With `SNOUT_SCREENSHOT=<dir>` the PNGs
//! are saved there to look at; either way each must actually show something.

#[cfg(target_os = "macos")]
fn main() {
    shots::run();
}

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
    use snout_app::settings::{Settings, ThemeChoice};
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
        /// No repository open: the welcome page.
        Nothing,
        /// A Java file in an editor tab.
        Editor,
    }

    fn shot(name: &str, theme: ThemeChoice, activity: Activity, select: Option<Select>) {
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
        let settings = Settings { theme, repos: vec![repo.clone()], current, ..Settings::default() };
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
            ws.update(cx, |w, cx| w.show_activity(activity, window, cx));
        })
        .unwrap();
        if !matches!(select, Some(Select::Nothing)) {
            cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
        }
        match select {
            Some(Select::File(path)) => cx.update(|cx| model.update(cx, |m, cx| m.click_file(path, false, false, cx))),
            Some(Select::Commit(summary, file)) => cx.update(|cx| {
                let id = model.read(cx).log.iter().find(|c| c.summary == summary).unwrap().id.clone();
                model.update(cx, |m, cx| {
                    m.handle(Action::ShowCommit(id), cx);
                    m.select_commit_file(file.into(), cx);
                })
            }),
            Some(Select::Console) => {
                let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
                let fake = repo.join("fake-claude");
                let script = "#!/bin/sh\nprintf '\\033]0;Fixing the diff view\\007'\nprintf '\\033[1;32m✓\\033[0m Read \\033[36msrc/main.rs\\033[0m\\n'\nprintf '\\033[7m inverse \\033[0m and \\033[4munderline\\033[0m\\n> '\nsleep 30\n";
                std::fs::write(&fake, script).unwrap();
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
                let open =
                    |cx: &mut HeadlessAppContext, a: Action| cx.update(|cx| model.update(cx, |m, cx| m.handle(a, cx)));
                open(&mut cx, Action::OpenFile(repo.join("build.gradle.kts"), None));
                open(&mut cx, Action::OpenFile(repo.join("src/main.rs"), None));
                open(&mut cx, Action::PreviewFile(repo.join("README.md")));
                open(&mut cx, Action::OpenFile(file, Some((14, 23))));
                // An unsaved change in another tab, for its dot.
                cx.update_window(handle.into(), |_, window, cx| {
                    let ws = ws.read(cx);
                    let main = ws.editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("src/main.rs")).unwrap();
                    let state = main.read(cx).state.clone();
                    state.update(cx, |s, cx| s.insert("// edited\n", window, cx));
                })
                .unwrap();
                // The palette shot: the file split in two, and the command palette over it.
                if name.contains("palette") {
                    cx.update_window(handle.into(), |_, window, cx| {
                        ws.update(cx, |ws, cx| ws.split_right(window, cx));
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

    pub fn run() {
        // The Git view, as the egui app's `renders_repo_and_diff` and
        // `clicking_a_commit_shows_what_it_changed` shots show it.
        shot("git-dark", ThemeChoice::Dark, Activity::Git, Some(Select::File("status.rs")));
        shot("git-light", ThemeChoice::Light, Activity::Git, Some(Select::File("status.rs")));
        shot("commit-dark", ThemeChoice::Dark, Activity::Git, Some(Select::Commit("Initial import", "status.rs")));
        shot("console-light", ThemeChoice::Light, Activity::Git, Some(Select::Console));
        shot("welcome-dark", ThemeChoice::Dark, Activity::Git, Some(Select::Nothing));
        shot("project-dark", ThemeChoice::Dark, Activity::Project, None);
        shot("editor-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Editor));
        shot("editor-light", ThemeChoice::Light, Activity::Project, Some(Select::Editor));
        shot("palette-dark", ThemeChoice::Dark, Activity::Project, Some(Select::Editor));
    }
}
