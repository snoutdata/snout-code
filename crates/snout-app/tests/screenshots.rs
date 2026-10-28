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

    /// A repository with a little history, a branch, a tag and changes in several states.
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
        let lib = include_str!("../../snout-core/src/status.rs");
        std::fs::write(dir.join("status.rs"), lib).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "Initial import"]);
        git(&dir, &["tag", "-a", "v0.1", "-m", "First release"]);
        git(&dir, &["branch", "feature/diff-view"]);
        git(&dir, &["commit", "-q", "--allow-empty", "-m", "Second commit"]);
        std::fs::write(dir.join("status.rs"), lib.replace("Human readable state", "Human-readable state")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello, snout\");\n}\n").unwrap();
        git(&dir, &["add", "src/main.rs"]);
        std::fs::write(dir.join("NOTES.md"), "untracked\n").unwrap();
        dir
    }

    pub enum Select {
        File(&'static str),
        Commit(usize),
        Console,
    }

    fn shot(name: &str, theme: ThemeChoice, activity: Activity, select: Option<Select>) {
        let repo = demo_repo(name);
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::current_platform(true).text_system(),
            Arc::new(gpui_kit_assets::AllAssets),
            gpui_platform::current_headless_renderer,
        );
        let settings =
            Settings { theme, repos: vec![repo.clone()], current: Some(repo.clone()), ..Settings::default() };
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
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
        match select {
            Some(Select::File(path)) => cx.update(|cx| model.update(cx, |m, cx| m.click_file(path, false, false, cx))),
            Some(Select::Commit(back)) => cx.update(|cx| {
                let id = model.read(cx).log[back].id.clone();
                model.update(cx, |m, cx| m.handle(Action::ShowCommit(id), cx))
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
            None => {}
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
        shot("projects-dark", ThemeChoice::Dark, Activity::Projects, None);
        shot("diff-light", ThemeChoice::Light, Activity::Git, Some(Select::File("status.rs")));
        shot("diff-dark", ThemeChoice::Dark, Activity::Git, Some(Select::File("src/main.rs")));
        shot("commit-dark", ThemeChoice::Dark, Activity::Git, Some(Select::Commit(1)));
        shot("console-light", ThemeChoice::Light, Activity::Git, Some(Select::Console));
    }
}
