use std::process::Command;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::*;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success();
    assert!(ok, "git {args:?}");
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("snout-gui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A repository with a little history, a tag, a second branch, and one file in each
/// interesting state.
fn demo_repo(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    std::fs::create_dir_all(dir.join("src")).unwrap();
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
    let lib = include_str!("../../../snout-core/src/status.rs");
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
    std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello, snout\");\n    run();\n}\n").unwrap();
    git(&dir, &["add", "src/main.rs"]);
    std::fs::write(dir.join("NOTES.md"), "untracked\n").unwrap();
    dir
}

fn harness(open: Option<PathBuf>) -> Harness<'static, SnoutApp> {
    let mut h = Harness::builder().with_size([1400.0, 900.0]).wgpu().build_eframe(move |cc| SnoutApp::new(cc, open));
    h.run_steps(3);
    h
}

/// Save a PNG of the current frame when `SNOUT_SCREENSHOT=/path/shot.png` is set;
/// `suffix` distinguishes the shots of different tests.
fn screenshot(h: &mut Harness<'_, SnoutApp>, suffix: &str) {
    if let Ok(out) = std::env::var("SNOUT_SCREENSHOT") {
        let path = out.replace(".png", &format!("{suffix}.png"));
        h.render().unwrap().save(path).unwrap();
    }
}

fn head(h: &Harness<'_, SnoutApp>) -> Option<String> {
    h.state().status.branch.head.clone()
}

#[test]
fn renders_repo_and_diff() {
    let dir = demo_repo("diff");
    let mut h = harness(Some(dir.clone()));
    assert_eq!(h.state().status.files.len(), 3);
    assert_eq!(h.state().tags.len(), 1);

    h.get_by_label("status.rs").click();
    h.run_steps(3);
    assert!(matches!(h.state().diff, Some(Ok(_))), "diff for the selected file");
    screenshot(&mut h, "");

    h.state_mut().settings.side_by_side = false;
    h.run_steps(3);
    screenshot(&mut h, "-unified");

    let ctx = h.ctx.clone();
    h.state_mut().handle(Action::SetTheme(ThemeChoice::Light), &ctx);
    h.run_steps(3);
    assert!(!h.ctx.global_style().visuals.dark_mode);
    screenshot(&mut h, "-light");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clicking_a_commit_shows_what_it_changed() {
    let dir = demo_repo("commit-view");
    // A merge, so the graph has a second lane.
    git(&dir, &["stash", "-u", "-q"]);
    git(&dir, &["switch", "-q", "feature/diff-view"]);
    std::fs::write(dir.join("README.md"), "# demo\n\nNow with a diff view.\n").unwrap();
    git(&dir, &["commit", "-qam", "Describe the diff view"]);
    git(&dir, &["switch", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "Merge feature/diff-view", "feature/diff-view"]);
    git(&dir, &["stash", "pop", "-q"]);
    let mut h = harness(Some(dir.clone()));
    assert_eq!(h.state().graph.iter().map(|r| r.width).max(), Some(2));

    h.get_by_label("Initial import").click();
    h.run_steps(3);
    let view = h.state().commit_view.as_ref().expect("a click opens the commit");
    let details = view.details.as_ref().unwrap();
    assert_eq!(details.summary(), "Initial import");
    assert_eq!(details.files.len(), 3);
    assert_eq!(view.selected.as_deref(), Some("README.md"), "the first file is picked");
    assert!(matches!(view.diff, Some(Ok(_))));
    h.get_by_label("status.rs").click();
    h.run_steps(3);
    let view = h.state().commit_view.as_ref().unwrap();
    assert_eq!(view.diff.as_ref().unwrap().as_ref().unwrap().diff.path, "status.rs");
    screenshot(&mut h, "-commit");

    // A click moves to another commit, on its text or on the graph beside it.
    h.get_by_label("Describe the diff view").click();
    h.run_steps(3);
    let details = h.state().commit_view.as_ref().unwrap().details.as_ref().unwrap();
    assert_eq!(details.files[0].path, "README.md");
    // The graph column ends where the Message column's header starts.
    let x = h.get_by_label("Message").rect().min.x - 10.0;
    let at = egui::pos2(x, h.get_by_label("Initial import").rect().center().y);
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.run_steps(3);
    let details = h.state().commit_view.as_ref().unwrap().details.as_ref().unwrap();
    assert_eq!(details.summary(), "Initial import", "a click on the graph column");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().commit_view.is_none());
    screenshot(&mut h, "-graph");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn directory_tree_narrows_the_file_list() {
    let dir = demo_repo("tree");
    let mut h = harness(Some(dir.clone()));
    assert_eq!(h.state().dirs, ["src"]);
    h.get_by_label("📁 src").click();
    h.run_steps(2);
    assert_eq!(h.state().dir_filter.as_deref(), Some("src"));
    h.get_by_label("main.rs");
    assert!(h.query_by_label("status.rs").is_none(), "files outside src/ are hidden");
    screenshot(&mut h, "-tree");

    h.get_by_label("in src/  ✕").click();
    h.run_steps(2);
    assert_eq!(h.state().dir_filter, None);
    h.get_by_label("status.rs");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A clone whose remote was then replaced by an unrelated history.
fn clone_of_replaced_remote(name: &str) -> (PathBuf, PathBuf) {
    let root = scratch_dir(name);
    let remote = root.join("remote.git");
    git(&root, &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
    for (dir, file) in [("old", "old.txt"), ("new", "new.txt")] {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        git(&d, &["init", "-q", "-b", "main"]);
        git(&d, &["config", "user.name", "Demo"]);
        git(&d, &["config", "user.email", "demo@example.com"]);
        git(&d, &["config", "commit.gpgsign", "false"]);
        std::fs::write(d.join(file), "x\n").unwrap();
        git(&d, &["add", "-A"]);
        git(&d, &["commit", "-qm", file]);
        if dir == "old" {
            git(&d, &["push", "-q", remote.to_str().unwrap(), "main"]);
            git(&root, &["clone", "-q", remote.to_str().unwrap(), "local"]);
        } else {
            git(&d, &["push", "-q", "--force", remote.to_str().unwrap(), "main"]);
        }
    }
    let local = root.join("local");
    git(&local, &["config", "commit.gpgsign", "false"]);
    git(&local, &["fetch", "-q"]);
    (root, local)
}

fn wait_for_job(h: &mut Harness<'_, SnoutApp>) {
    wait_until(h, "the job to finish", |app| app.job.is_none());
    h.run_steps(2);
}

#[test]
fn pull_from_a_replaced_remote_offers_a_reset() {
    let (root, local) = clone_of_replaced_remote("replaced");
    let mut h = harness(Some(local.clone()));
    let ctx = h.ctx.clone();
    h.state_mut().handle(Action::Pull(PullMode::Merge), &ctx);
    wait_for_job(&mut h);
    assert!(matches!(h.state().dialog, Some(Dialog::PullProblem { problem: PullProblem::Unrelated, .. })));
    screenshot(&mut h, "-unrelated");

    h.get_by_label("Reset to origin/main").click();
    h.run_steps(3);
    assert!(matches!(&h.state().dialog, Some(Dialog::Info { title, .. }) if title == "main now matches origin/main"));
    assert!(local.join("new.txt").exists() && !local.join("old.txt").exists());
    assert!(h.state().branches.iter().any(|b| b.name.starts_with("backup/main-")), "backup branch listed");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn welcome_screen_without_repo() {
    let mut h = harness(None);
    assert!(h.state().repo.is_none());
    h.get_by_label("Open local repository…   ⌘O");
    h.get_by_label("Find repositories in folder…");
    screenshot(&mut h, "-welcome");
}

#[test]
fn checkout_picker_switches_branch_and_detaches_on_tag() {
    let dir = demo_repo("picker");
    let mut h = harness(Some(dir.clone()));
    assert_eq!(head(&h).as_deref(), Some("main"));

    // ⇧⌘O opens the picker; typing filters it.
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::O);
    h.run_steps(2);
    let Some(Dialog::Checkout(picker)) = &mut h.state_mut().dialog else { panic!("picker open") };
    picker.query = "feat".into();
    h.run_steps(2);
    screenshot(&mut h, "-picker");

    // The working tree is dirty, but nothing conflicts with the target: keep works.
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert!(h.state().dialog.is_none(), "picker closed");
    assert_eq!(head(&h).as_deref(), Some("feature/diff-view"));
    assert_eq!(h.state().status.files.len(), 3, "local changes carried over");

    // Tags check out detached, after asking about the local changes.
    let ctx = h.ctx.clone();
    h.state_mut().handle(Action::RequestCheckout(CheckoutTarget::Tag("v0.1".into())), &ctx);
    h.run_steps(2);
    assert!(matches!(h.state().dialog, Some(Dialog::ConfirmCheckout(_))));
    screenshot(&mut h, "-confirm");
    h.get_by_label("Stash, check out, re-apply").click();
    h.run_steps(3);
    assert_eq!(head(&h), None, "detached at tag");
    assert!(h.state().head_label().starts_with("detached at "));
    assert_eq!(h.state().status.files.len(), 3, "stash re-applied");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_plain_folder_offers_init_or_search() {
    let parent = scratch_dir("plain");
    let inner = demo_repo("plain-inner");
    let nested = parent.join("inner");
    std::fs::rename(&inner, &nested).unwrap();

    let mut h = harness(None);
    let ctx = h.ctx.clone();
    h.state_mut().handle(Action::OpenPath(parent.clone()), &ctx);
    h.run_steps(2);
    assert!(matches!(h.state().dialog, Some(Dialog::InitRepo(_))));
    screenshot(&mut h, "-init");

    // "Find repositories inside" runs on a worker thread.
    h.get_by_label("Find repositories inside").click();
    for _ in 0..50 {
        h.run_steps(1);
        if h.state().job.is_none() && h.state().repo.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let root = h.state().repo.as_ref().map(|r| r.root().canonicalize().unwrap());
    assert_eq!(root, Some(nested.canonicalize().unwrap()), "found and opened the nested repo");

    // Initializing makes the folder itself a repository.
    let fresh = scratch_dir("fresh");
    h.state_mut().handle(Action::InitRepo(fresh.clone()), &ctx);
    h.run_steps(2);
    let root = h.state().repo.as_ref().map(|r| r.root().canonicalize().unwrap());
    assert_eq!(root, Some(fresh.canonicalize().unwrap()));
    assert!(h.state().log.is_empty());

    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&fresh);
}

fn wait_until(h: &mut Harness<'_, SnoutApp>, what: &str, done: impl Fn(&SnoutApp) -> bool) {
    for _ in 0..200 {
        h.run_steps(1);
        if done(h.state()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("timed out waiting for {what}");
}

fn console_text(app: &SnoutApp) -> String {
    let root = app.repo.as_ref().unwrap().root();
    app.console.session_text(root).unwrap_or_default()
}

#[test]
fn claude_console_runs_in_the_repo_and_refreshes_git() {
    let dir = demo_repo("console");
    let bin = scratch_dir("console-bin").join("claude");
    // A stand-in for `claude`: draws a box, then creates the files it is asked for.
    std::fs::write(
        &bin,
        "#!/bin/sh\nprintf '\\033[38;5;208m╭──────────────╮\\n│ ✻ fake agent │\\n╰──────────────╯\\033[0m\\n'\n\
         printf 'cwd: %s\\n' \"$(basename \"$PWD\")\"\n\
         while read -r name; do echo \"⏺ writing $name\"; echo hi > \"$name\"; done\n",
    )
    .unwrap();
    Command::new("chmod").arg("+x").arg(&bin).status().unwrap();

    let mut h = harness(Some(dir.clone()));
    h.state_mut().console.program = Some(bin.clone());
    h.key_press_modifiers(egui::Modifiers::COMMAND, Key::J);
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    wait_until(&mut h, "the agent to start", |app| console_text(app).contains(&format!("cwd: {name}")));

    let files_before = h.state().status.files.len();
    h.event(egui::Event::Text("AGENT.md".into()));
    h.key_press(Key::Enter);
    wait_until(&mut h, "the agent to write a file", |app| console_text(app).contains("⏺ writing AGENT.md"));
    wait_until(&mut h, "the git view to refresh", |app| app.status.files.len() == files_before + 1);
    screenshot(&mut h, "-console");
    let ctx = h.ctx.clone();
    h.state_mut().handle(Action::SetTheme(ThemeChoice::Light), &ctx);
    h.run_steps(2);
    screenshot(&mut h, "-console-light");
    h.state_mut().handle(Action::SetTheme(ThemeChoice::Dark), &ctx);

    // ⌘J hides the console; the session keeps running.
    h.key_press_modifiers(egui::Modifiers::COMMAND, Key::J);
    h.run_steps(2);
    assert!(!h.state().settings.console_open);
    assert!(console_text(h.state()).contains("AGENT.md"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(bin.parent().unwrap());
}

/// Renders the real Claude Code (no prompt is sent). Opt-in:
/// `SNOUT_REAL_CLAUDE=1 SNOUT_SCREENSHOT=/tmp/shot.png cargo test real_claude -- --ignored`
#[test]
#[ignore]
fn real_claude_renders() {
    if std::env::var_os("SNOUT_REAL_CLAUDE").is_none() {
        return;
    }
    let dir = demo_repo("real-claude");
    let mut h = harness(Some(dir.clone()));
    let ctx = h.ctx.clone();
    h.state_mut().handle(Action::ToggleConsole, &ctx);
    std::thread::sleep(std::time::Duration::from_secs(4));
    for _ in 0..30 {
        h.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("{}", console_text(h.state()));
    screenshot(&mut h, "-real-claude");
    h.state_mut().console.stop(&dir.canonicalize().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every non-ASCII character in UI code must exist in the bundled fonts, otherwise
/// it renders as a missing-glyph box.
#[test]
fn ui_glyphs_exist_in_fonts() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    let sources = [
        include_str!("mod.rs"),
        include_str!("dialogs.rs"),
        include_str!("console.rs"),
        include_str!("../terminal.rs"),
        // What Claude Code draws.
        "╭─╮│╰╯ ✻✶✳✢·⏺⎿❯›↓↑←→⏵⏸✓✗✔✘•▐▛▜▌▝▘█░▒▓…⧉",
        include_str!("../diff_view.rs"),
        include_str!("../art.rs"),
        include_str!("../graph.rs"),
        include_str!("../theme.rs"),
        include_str!("../../../snout-core/src/diff.rs"),
    ];
    let mut missing = BTreeSet::new();
    for line in sources.iter().flat_map(|s| s.lines()) {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for c in line.chars().filter(|c| !c.is_ascii()) {
            for font in [egui::FontId::proportional(14.0), egui::FontId::monospace(14.0)] {
                if !ctx.fonts_mut(|f| f.has_glyph(&font, c)) {
                    missing.insert(format!("{c} U+{:04X} ({:?})", c as u32, font.family));
                }
            }
        }
    }
    assert!(missing.is_empty(), "glyphs missing from fonts: {missing:#?}");
}

#[test]
fn every_dialog_renders_in_both_themes() {
    let dir = demo_repo("dialogs");
    let mut h = harness(Some(dir.clone()));
    let ctx = h.ctx.clone();
    let entries = h.state().status.files.clone();
    let make = |name: &str| match name {
        "error" => Dialog::Error(
            "fatal: unable to access 'https://example.com/repo.git/': Could not resolve host: example.com".into(),
        ),
        "info" => Dialog::Info {
            title: "main now matches origin/main".into(),
            text: "Your previous commits are kept on the branch backup/main-1.".into(),
            mood: Mood::Done,
        },
        "discard" => Dialog::ConfirmDiscard(entries.clone()),
        "confirm" => Dialog::ConfirmCheckout(CheckoutTarget::Tag("v0.1".into())),
        "picker" => Dialog::Checkout(CheckoutPicker::default()),
        "branch" => Dialog::NewBranch { name: "feature/graph".into(), start: None, checkout: true },
        "delete" => Dialog::DeleteBranch {
            name: "feature/diff-view".into(),
            force: false,
            upstream: Some(RemoteRef { remote: "origin".into(), branch: "feature/diff-view".into() }),
            with_upstream: true,
        },
        "delete-remote" => {
            Dialog::DeleteRemoteBranch(RemoteRef { remote: "origin".into(), branch: "feature/diff-view".into() })
        }
        "delete-files" => Dialog::ConfirmDeleteFiles(entries.clone()),
        "divergent" => Dialog::PullProblem {
            problem: PullProblem::Divergent,
            details: "hint: You have divergent branches and need to specify how to reconcile them.".into(),
        },
        "init" => Dialog::InitRepo(PathBuf::from("/Users/demo/projects")),
        _ => {
            Dialog::Clone { url: "https://github.com/snout-dev/snout-git.git".into(), parent: "/Users/demo/dev".into() }
        }
    };
    for theme in [ThemeChoice::Dark, ThemeChoice::Light] {
        h.state_mut().handle(Action::SetTheme(theme), &ctx);
        for name in [
            "error",
            "info",
            "discard",
            "confirm",
            "picker",
            "branch",
            "delete",
            "delete-remote",
            "delete-files",
            "divergent",
            "init",
            "clone",
        ] {
            h.state_mut().dialog = Some(make(name));
            h.run_steps(3);
            assert!(h.state().dialog.is_some(), "{name} stays open");
            screenshot(&mut h, &format!("-dlg-{name}-{theme:?}"));
        }
    }

    // Esc closes any dialog.
    h.state_mut().dialog = Some(make("init"));
    h.run_steps(2);
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().dialog.is_none(), "Esc closes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deletes_branches_locally_and_on_the_remote() {
    let local = demo_repo("delete");
    let remote = local.with_extension("remote.git");
    let _ = std::fs::remove_dir_all(&remote);
    git(&local, &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
    git(&local, &["remote", "set-url", "origin", remote.to_str().unwrap()]);
    git(&local, &["push", "-q", "-u", "origin", "main", "feature/diff-view", "main:topic"]);
    let mut h = harness(Some(local.clone()));
    let ctx = h.ctx.clone();
    let branch = |h: &Harness<'_, SnoutApp>, name: &str| h.state().branches.iter().find(|b| b.name == name).cloned();

    // Hovering a branch shows its trash can.
    h.get_all_by_label("feature/diff-view").next().unwrap().hover();
    h.run_steps(2);
    screenshot(&mut h, "-branch-hover");
    h.get_by_label("Trash");

    // A local branch with an upstream offers to delete that too.
    let feature = branch(&h, "feature/diff-view").expect("local branch");
    h.state_mut().handle(Action::AskDeleteBranch(feature), &ctx);
    h.run_steps(2);
    let Some(Dialog::DeleteBranch { upstream: Some(up), with_upstream, .. }) = &mut h.state_mut().dialog else {
        panic!("delete dialog offering the upstream")
    };
    assert_eq!(up.label(), "origin/feature/diff-view");
    *with_upstream = true;
    h.run_steps(2);
    h.get_by_label("Delete branch").click();
    h.run_steps(2);
    wait_for_job(&mut h);
    assert!(branch(&h, "feature/diff-view").is_none(), "local branch gone");
    assert!(branch(&h, "origin/feature/diff-view").is_none(), "remote branch gone");

    // A remote branch on its own.
    let topic = branch(&h, "origin/topic").expect("remote branch");
    h.state_mut().handle(Action::AskDeleteBranch(topic), &ctx);
    h.run_steps(2);
    h.get_by_label("Delete from origin").click();
    h.run_steps(2);
    wait_for_job(&mut h);
    assert!(branch(&h, "origin/topic").is_none());
    let out = Command::new("git").arg("-C").arg(&remote).args(["branch", "--list"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "* main");

    // Delete asks before touching files, and only offers what is on disk.
    let files = h.state().status.files.clone();
    h.state_mut().handle(Action::AskDeleteFiles(files), &ctx);
    h.run_steps(2);
    let Some(Dialog::ConfirmDeleteFiles(entries)) = &h.state().dialog else { panic!("confirmation") };
    assert_eq!(entries.len(), 3);
    h.get_by_label("Cancel").click();
    h.run_steps(2);
    assert!(h.state().dialog.is_none() && local.join("NOTES.md").exists());

    let _ = std::fs::remove_dir_all(&local);
    let _ = std::fs::remove_dir_all(&remote);
}
