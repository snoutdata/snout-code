//! Headless tests of the GPUI app: each view renders in a test window (GPUI's
//! `TestAppContext` / `VisualTestContext`) over a real repository, and its actions change
//! the repository and what the views show. Pixels are checked by `tests/screenshots.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use crate::model::{Action, Model};
use crate::panels::Activity;
use crate::settings::Settings;
use crate::workspace::Workspace;
use snout_core::term::EngineKind;

pub fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}\n{}{}",
        dir.display(),
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The modifier of ⌘-click and the like: ⌘ on a Mac, Ctrl elsewhere.
pub fn primary() -> gpui::Modifiers {
    if cfg!(target_os = "macos") { gpui::Modifiers::command() } else { gpui::Modifiers::control() }
}

pub fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("snout-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    snout_host::canonical(&dir).unwrap()
}

/// Copy the folder `from` into `into` (as `cp -R from into/`), on every system.
pub fn copy_tree(from: &Path, into: &Path) {
    let dest = into.join(from.file_name().expect("a folder name"));
    let mut stack = vec![(from.to_path_buf(), dest)];
    while let Some((src, dst)) = stack.pop() {
        std::fs::create_dir_all(&dst).unwrap();
        for e in std::fs::read_dir(&src).unwrap().flatten() {
            let to = dst.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                stack.push((e.path(), to));
            } else {
                std::fs::copy(e.path(), &to).unwrap();
            }
        }
    }
}

/// A stand-in program `name` in `dir`: the `sh` script on Unix (made executable), the batch
/// file `cmd` (`name.cmd`) on Windows.
pub fn fake_program(dir: &Path, name: &str, sh: &str, cmd: &str) -> PathBuf {
    if cfg!(windows) {
        let fake = dir.join(format!("{name}.cmd"));
        std::fs::write(&fake, cmd.replace('\n', "\r\n")).unwrap();
        fake
    } else {
        let fake = dir.join(name);
        std::fs::write(&fake, sh).unwrap();
        Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
        fake
    }
}

/// A Unix tool at `unix` (`/bin/sh`), or on Windows the one Git for Windows carries
/// (`C:\\Program Files\\Git\\usr\\bin\\sh.exe`), as the tests use them.
pub fn unix_tool(unix: &str) -> PathBuf {
    if cfg!(windows) {
        let name = Path::new(unix).file_name().unwrap().to_string_lossy().into_owned();
        PathBuf::from(r"C:\Program Files\Git\usr\bin").join(format!("{name}.exe"))
    } else {
        PathBuf::from(unix)
    }
}

/// A process that waits about 30 seconds and does nothing else, on every system.
pub fn sleeper() -> std::process::Command {
    if cfg!(windows) {
        let mut c = std::process::Command::new("ping.exe");
        c.args(["-n", "30", "127.0.0.1"]).stdout(std::process::Stdio::null());
        c
    } else {
        let mut c = std::process::Command::new("sleep");
        c.arg("30");
        c
    }
}

/// A repository with a little history, a tag, a second branch, and one file in each
/// interesting state: modified, staged, untracked.
pub fn demo_repo(name: &str) -> PathBuf {
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
    let lib = include_str!("../../snout-core/src/status.rs");
    std::fs::write(dir.join("status.rs"), lib).unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    std::fs::write(dir.join("README.md"), "# demo\n").unwrap();
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

/// The whole window over `settings`, as the app opens it (no settings file).
pub fn open(cx: &mut TestAppContext, settings: Settings) -> (Entity<Model>, Entity<Workspace>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(&settings, cx));
    let model = cx.update(|cx| cx.new(|cx| Model::new(settings, None, cx)));
    let mut ws = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
        ws = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    (model, ws.unwrap(), vcx)
}

/// Held by each test that runs the real engine: one at a time, so a test's timeouts do not
/// count the others' Gradle and jdtls.
pub fn engine_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn with_repo(repo: &Path) -> Settings {
    Settings { repos: vec![repo.to_owned()], current: Some(repo.to_owned()), ..Settings::default() }
}

pub fn act(model: &Entity<Model>, cx: &mut VisualTestContext, action: Action) {
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(action, cx)));
    cx.run_until_parked();
}

/// The app's way to its first window waits for nothing slow: the project's repository opens
/// off the UI thread, and the menu bar and the background fetch wait until the window has
/// painted (`after_first_paint`, which the app runs at the frame after the first).
#[gpui::test]
fn the_first_window_opens_before_the_slow_work(cx: &mut TestAppContext) {
    let dir = scratch_dir("startup");
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("README.md"), "# startup\n").unwrap();
    let settings = with_repo(&dir);
    let window = cx.update(|cx| {
        crate::init_look(&settings, cx);
        crate::init_kit(&settings, cx);
        let model = cx.new(|cx| Model::new(settings.clone(), None, cx));
        let window = crate::open_window(model.clone(), None, cx).expect("a window");
        // The window is open and has drawn its first frame; nothing slow has run yet.
        let m = model.read(cx);
        assert_eq!(m.project.as_deref(), Some(dir.as_path()), "the project is known at once");
        assert!(m.repo.is_none(), "its repository opens off the UI thread");
        assert!(m.fetch_every.is_none(), "the background fetch waits for the first paint");
        assert!(cx.get_menus().unwrap_or_default().is_empty(), "the menu bar waits for the first paint");
        window
    });
    cx.run_until_parked();
    let ws = cx.update(|cx| {
        let root = window.downcast::<gpui_component::Root>().expect("the app's root");
        root.read(cx).expect("the window").view().clone().downcast::<Workspace>().expect("the workspace")
    });
    cx.update(|cx| {
        let model = ws.read(cx).model.clone();
        assert!(model.read(cx).repo.is_some(), "the repository has opened");
        assert!(cx.get_menus().unwrap_or_default().is_empty(), "no frame yet, so no menu bar");
        crate::after_first_paint(&ws, cx);
        assert!(!cx.get_menus().unwrap_or_default().is_empty(), "the menu bar is up once painted");
        if std::env::var_os("SNOUT_NO_FETCH").is_none() {
            assert!(model.read(cx).fetch_every.is_some(), "the background fetch starts once painted");
        }
    });
}

#[gpui::test]
fn the_window_opens_on_the_welcome_page_without_a_repository(cx: &mut TestAppContext) {
    let (model, ws, cx) = open(cx, Settings::default());
    cx.update(|_, cx| {
        assert!(model.read(cx).repo.is_none());
        assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Project, "the project comes first");
        assert_eq!(model.read(cx).title(), "SnoutCode");
        assert!(!ws.read(cx).hub.read(cx).paused, "the Java servers may start once the window has painted");
    });
}

#[gpui::test]
fn projects_groups_hold_repositories_and_the_tree_narrows_the_files(cx: &mut TestAppContext) {
    let a = demo_repo("projects-a");
    let b = demo_repo("projects-b");
    let mut settings = with_repo(&a);
    settings.repos.push(b.clone());
    let (model, ws, cx) = open(cx, settings);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));

    act(&model, cx, Action::SaveGroup { rename: None, name: "Work".into(), repo: Some(b.clone()) });
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.group_of(&b).map(|g| g.name.as_str()), Some("Work"));
        assert!(m.group_of(&a).is_none());
    });
    act(&model, cx, Action::SaveGroup { rename: Some("Work".into()), name: "Clients".into(), repo: None });
    act(&model, cx, Action::MoveToGroup(a.clone(), Some("Clients".into())));
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.groups[0].repos.len(), 2));
    act(&model, cx, Action::DeleteGroup("Clients".into()));
    cx.update(|_, cx| assert!(model.read(cx).settings.groups.is_empty()));

    // The tree under the open repository narrows the changed files to a directory.
    act(&model, cx, Action::FilterDir(Some("src".into())));
    cx.update(|_, cx| {
        let files: Vec<String> = model.read(cx).visible_files().iter().map(|f| f.path.clone()).collect();
        assert_eq!(files, ["src/main.rs"]);
    });
    act(&model, cx, Action::FilterDir(None));
    cx.update(|_, cx| assert_eq!(model.read(cx).visible_files().len(), 3));

    // Opening the other repository from the list.
    act(&model, cx, Action::OpenRepo(b.clone()));
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.current.as_deref(), Some(b.as_path())));
    act(&model, cx, Action::RemoveRepo(b.clone()));
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert!(m.repo.is_none() && m.settings.repos == [a.clone()]);
    });
    for dir in [a, b] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[gpui::test]
fn the_activity_bar_switches_between_the_git_view_and_the_docks(cx: &mut TestAppContext) {
    let (_, ws, cx) = open(cx, Settings::default());
    let left_open = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(gpui_component::dock::DockPlacement::Left))
    };
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Project, "the first launch"));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Project, w, cx)));
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Project));
    assert!(left_open(cx));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Project, w, cx)));
    assert!(!left_open(cx), "the active item closes the dock");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Git));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Git, "Git stays"));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    assert!(left_open(cx));
}

/// What `git show` says the last commit holds.
fn last_commit_files(repo: &Path) -> String {
    let out =
        Command::new("git").arg("-C").arg(repo).args(["show", "--name-only", "--format=%s", "HEAD"]).output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

#[gpui::test]
fn git_panel_branches_check_out_create_and_delete(cx: &mut TestAppContext) {
    use snout_core::{CheckoutTarget, LocalChanges};
    let repo = demo_repo("branches");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Git, w, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let m = model.read(cx);
        let names: Vec<&str> = m.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main") && names.contains(&"feature/diff-view"), "{names:?}");
        assert_eq!(m.tags.len(), 1);
    });

    // With tracked changes, a checkout asks what to do with them first.
    let feature = CheckoutTarget::Branch("feature/diff-view".into());
    act(&model, cx, Action::RequestCheckout(feature.clone()));
    cx.update(|_, cx| assert!(matches!(model.read(cx).dialog, Some(crate::model::Dialog::ConfirmCheckout(_)))));
    act(&model, cx, Action::Checkout(feature, LocalChanges::Stash));
    cx.update(|_, cx| assert_eq!(model.read(cx).status.branch.head.as_deref(), Some("feature/diff-view")));

    act(&model, cx, Action::CreateBranch { name: "topic".into(), start: None, checkout: false });
    cx.update(|_, cx| assert!(model.read(cx).branches.iter().any(|b| b.name == "topic")));
    act(&model, cx, Action::DeleteBranch { name: "topic".into(), force: false, upstream: None });
    cx.update(|_, cx| assert!(!model.read(cx).branches.iter().any(|b| b.name == "topic")));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn git_panel_commits_without_staging_what_is_selected(cx: &mut TestAppContext) {
    let repo = demo_repo("commit");
    let (model, _, cx) = open(cx, with_repo(&repo));
    // Something is staged: Commit records only that.
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_scope(), snout_core::Scope::Staged(1)));
    act(&model, cx, Action::Unstage(vec!["src/main.rs".into()]));
    // Nothing staged and one file selected: Commit adds and records that file alone.
    cx.update(|_, cx| model.update(cx, |m, cx| m.click_file("NOTES.md", false, false, cx)));
    // The diff is read off the UI thread.
    cx.run_until_parked();
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.commit_scope().label(), "1 selected");
        assert!(m.diff.is_some(), "the selected file's diff is ready for its tab");
        assert!(!m.can_commit(), "no message yet");
    });
    cx.update(|_, cx| model.update(cx, |m, _| m.commit_msg = "Add notes".into()));
    act(&model, cx, Action::Commit);
    let last = last_commit_files(&repo);
    assert!(last.contains("NOTES.md") && !last.contains("main.rs"), "{last}");
    // Nothing selected: Commit takes every changed file.
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert!(m.commit_msg.is_empty(), "the message is cleared after committing");
        assert_eq!(m.commit_scope().label(), "all 2 changed");
    });
    cx.update(|_, cx| model.update(cx, |m, _| m.commit_msg = "The rest".into()));
    act(&model, cx, Action::Commit);
    cx.update(|_, cx| assert!(model.read(cx).status.files.is_empty()));
    // Amend fills in the last message.
    cx.update(|_, cx| model.update(cx, |m, cx| m.set_amend(true, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_msg.trim(), "The rest"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn claude_drafts_the_commit_message_from_the_patch(cx: &mut TestAppContext) {
    let repo = demo_repo("draft");
    let bin = scratch_dir("draft-bin");
    // A stand-in for `claude -p`: it reads the patch and answers with a message naming a
    // file in it.
    let fake = fake_program(
        &bin,
        "claude",
        "#!/bin/sh\nif grep -q 'src/main.rs' ; then echo 'Greet snout'; else echo 'Wrong patch'; fi\n",
        "@echo off\nfindstr /c:\"src/main.rs\" >nul && (echo Greet snout) || (echo Wrong patch)\n",
    );
    let (model, _, cx) = open(cx, with_repo(&repo));
    cx.update(|_, cx| model.update(cx, |m, _| m.claude = Some(fake.clone())));
    act(&model, cx, Action::DraftMessage);
    cx.update(|_, cx| assert!(model.read(cx).draft.is_some()));
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        if cx.update(|_, cx| model.read(cx).draft.is_none()) {
            break;
        }
    }
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_msg, "Greet snout"));
    for dir in [repo, bin] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[gpui::test]
fn background_fetch_counts_what_there_is_to_pull(cx: &mut TestAppContext) {
    let work = demo_repo("fetch");
    // A remote with one commit more than the working copy knows about.
    let remote = scratch_dir("fetch-remote").join("remote.git");
    let (w, r) = (work.to_str().unwrap(), remote.to_str().unwrap());
    git(work.parent().unwrap(), &["clone", "-q", "--bare", w, r]);
    git(&work, &["remote", "add", "up", r]);
    git(&work, &["fetch", "-q", "up"]);
    git(&work, &["branch", "-q", "--set-upstream-to=up/main"]);
    let other = scratch_dir("fetch-other");
    git(&other, &["clone", "-q", r, "."]);
    let who = ["-c", "user.name=O", "-c", "user.email=o@example.com"];
    git(&other, &[&who[..], &["commit", "-q", "--allow-empty", "-m", "Upstream"]].concat());
    git(&other, &["push", "-q", "origin", "main"]);

    let (model, _, cx) = open(cx, with_repo(&work));
    cx.update(|_, cx| {
        model.update(cx, |m, cx| {
            m.fetch_every = Some(std::time::Duration::ZERO);
            m.survey(false, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let m = model.read(cx);
        let t = m.tracking.get(&work).copied().flatten().expect("counted");
        assert_eq!((t.ahead, t.behind), (0, 1), "the fetch saw the new upstream commit");
        assert_eq!(m.status.branch.behind, 1, "the open repository was re-read after fetching");
    });
    for dir in [work, remote.parent().unwrap().to_owned(), other] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[gpui::test]
fn the_diff_tab_follows_the_selection_and_its_options(cx: &mut TestAppContext) {
    use snout_core::DiffBase;
    let repo = demo_repo("diff");
    let (model, _, cx) = open(cx, with_repo(&repo));
    cx.update(|_, cx| model.update(cx, |m, cx| m.click_file("status.rs", false, false, cx)));
    cx.run_until_parked();
    let lines = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let m = model.read(cx);
            let d = m.diff.as_ref().unwrap().as_ref().unwrap();
            assert_eq!(d.diff.path, "status.rs");
            d.len(m.settings.side_by_side)
        })
    };
    let compact = lines(cx);
    cx.update(|_, cx| model.update(cx, |m, cx| m.set_diff_options(None, Some(false), cx)));
    cx.run_until_parked();
    assert!(lines(cx) > compact, "the whole file shows without Compact");
    // Comparing HEAD with the index: status.rs is not staged, so nothing differs.
    cx.update(|_, cx| model.update(cx, |m, cx| m.set_diff_base(DiffBase::HeadToIndex, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| assert!(model.read(cx).diff.as_ref().unwrap().as_ref().unwrap().diff.is_identical()));
    // Two files selected: no diff, the tab says how many.
    cx.update(|_, cx| model.update(cx, |m, cx| m.click_file("NOTES.md", false, true, cx)));
    cx.update(|_, cx| assert!(model.read(cx).diff.is_none() && model.read(cx).selected.len() == 2));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_journal_filters_and_opens_commits_in_the_commit_tab(cx: &mut TestAppContext) {
    let repo = demo_repo("journal");
    let (model, _, cx) = open(cx, with_repo(&repo));
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.log.len(), 2);
        assert_eq!(m.graph.len(), 2);
    });
    cx.update(|_, cx| model.update(cx, |m, _| m.journal_filter = "initial".into()));
    cx.update(|_, cx| assert_eq!(model.read(cx).journal_rows(), [1]));
    cx.update(|_, cx| model.update(cx, |m, _| m.journal_filter = "v0.1".into()));
    cx.update(|_, cx| assert_eq!(model.read(cx).journal_rows(), [1]));
    cx.update(|_, cx| model.update(cx, |m, _| m.journal_filter.clear()));

    let first = cx.update(|_, cx| model.read(cx).log[1].id.clone());
    act(&model, cx, Action::ShowCommit(first.clone()));
    cx.update(|_, cx| {
        let m = model.read(cx);
        let view = m.commit_view.as_ref().unwrap();
        assert_eq!(view.id(), Some(first.as_str()));
        let d = view.details.as_ref().unwrap();
        assert_eq!(d.summary(), "Initial import");
        assert!(d.parents.is_empty());
        assert_eq!(view.selected.as_deref(), Some(d.files[0].path.as_str()));
        assert!(view.diff.as_ref().unwrap().is_ok(), "the first file's diff shows");
        assert!(m.commit_scope() != snout_core::Scope::Nothing, "the working tree is untouched");
    });
    cx.update(|_, cx| model.update(cx, |m, cx| m.select_commit_file("status.rs".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let view = model.read(cx).commit_view.as_ref().unwrap();
        assert_eq!(view.diff.as_ref().unwrap().as_ref().unwrap().diff.path, "status.rs");
    });
    act(&model, cx, Action::CloseCommit);
    cx.update(|_, cx| assert!(model.read(cx).commit_view.is_none()));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn claude_console_runs_in_the_repo_and_refreshes_git(cx: &mut TestAppContext) {
    claude_console_on(EngineKind::Builtin, cx);
}

#[cfg(feature = "ghostty")]
#[gpui::test]
fn claude_console_runs_on_ghostty(cx: &mut TestAppContext) {
    claude_console_on(EngineKind::Ghostty, cx);
}

fn claude_console_on(engine: EngineKind, cx: &mut TestAppContext) {
    let name = format!("console-{}", engine.label().to_lowercase());
    let repo = demo_repo(&name);
    let bin = scratch_dir(&format!("{name}-bin"));
    // A stand-in for `claude`: it says where it runs, then makes the file it is told to.
    let fake = fake_program(
        &bin,
        "claude",
        "#!/bin/sh\necho \"ready in $(pwd)\"\nread name\ntouch \"$name.txt\"\necho \"made $name\"\nsleep 30\n",
        "@echo off\necho ready in %CD%\nset /p name=\ntype nul > \"%name%.txt\"\necho made %name%\nping -n 30 127.0.0.1 >nul\n",
    );
    let (model, ws, cx) = open(cx, Settings { terminal_engine: engine, ..with_repo(&repo) });
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).expect("the console is docked");
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    act(&model, cx, Action::ToggleConsole);
    // Real time: the program starts in a pseudo terminal, which takes a while when the
    // machine is busy (the engine tests run beside this one).
    let wait = |cx: &mut VisualTestContext, what: &str| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            cx.executor().advance_clock(std::time::Duration::from_millis(50));
            cx.run_until_parked();
            let text = cx.update(|_, cx| console.read(cx).session_text(&repo)).unwrap_or_default();
            if text.contains(what) {
                return text;
            }
        }
        panic!("{what:?} never appeared");
    };
    // The line may wrap: the program can print before the terminal takes the panel's width.
    let text = wait(cx, "ready in").replace('\n', "");
    assert!(text.contains(repo.file_name().unwrap().to_str().unwrap()), "{text}");
    cx.update(|_, cx| assert_eq!(console.read(cx).session_engine(&repo), Some(engine)));
    cx.simulate_keystrokes("n o t e enter");
    wait(cx, "made note");
    assert!(repo.join("note.txt").exists());
    // Once the agent goes quiet, the Git views read the repository again.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(25));
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        if cx.update(|_, cx| model.read(cx).status.files.iter().any(|f| f.path == "note.txt")) {
            break;
        }
    }
    cx.update(|_, cx| assert!(model.read(cx).status.files.iter().any(|f| f.path == "note.txt")));
    act(&model, cx, Action::ToggleConsole);
    cx.update(|_, cx| assert!(!model.read(cx).settings.console_open));
    for dir in [repo, bin] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[gpui::test]
fn keyboard_shortcuts_reach_the_workspace(cx: &mut TestAppContext) {
    let repo = demo_repo("keys");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-2"));
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Git, "⌘2 is Git"));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-1"));
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Project, "⌘1 is the project"));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-2"));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-a"));
    cx.update(|_, cx| assert_eq!(model.read(cx).selected.len(), 3, "⌘A selects every listed file"));
    let first = cx.update(|_, cx| model.read(cx).log[1].id.clone());
    act(&model, cx, Action::ShowCommit(first));
    cx.simulate_keystrokes("up");
    let top = cx.update(|_, cx| model.read(cx).log[0].id.clone());
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_view.as_ref().unwrap().id(), Some(top.as_str()), "↑ steps up"));
    cx.simulate_keystrokes("up");
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_view.as_ref().unwrap().id(), Some(top.as_str()), "and stops"));
    cx.simulate_keystrokes("down");
    cx.update(|_, cx| {
        assert_ne!(model.read(cx).commit_view.as_ref().unwrap().id(), Some(top.as_str()), "↓ steps down")
    });
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| assert!(model.read(cx).commit_view.is_none(), "Esc leaves the commit view"));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-backspace"));
    cx.update(|_, cx| {
        let dialog = &model.read(cx).dialog;
        assert!(matches!(dialog, Some(crate::model::Dialog::ConfirmDeleteFiles(f)) if f.len() == 3), "{dialog:?}");
    });
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn discarding_takes_a_click_or_cmd_backspace_never_enter(cx: &mut TestAppContext) {
    let repo = demo_repo("discard-keys");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Git, w, cx)));
    cx.run_until_parked();
    let status = |cx: &mut gpui::VisualTestContext| {
        cx.update(|_, cx| model.read(cx).status.files.iter().find(|f| f.path == "status.rs").cloned())
    };
    let entry = status(cx).expect("status.rs is modified");
    act(&model, cx, Action::AskDiscard(vec![entry.clone()]));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|_, cx| assert!(model.read(cx).dialog.is_none(), "Esc cancels"));
    assert!(status(cx).is_some(), "and keeps the change");
    act(&model, cx, Action::AskDiscard(vec![entry]));
    cx.run_until_parked();
    assert!(cx.debug_bounds("danger").is_some(), "the dialog has its Discard button");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|_, cx| assert!(model.read(cx).dialog.is_some(), "⏎ does not discard"));
    assert!(status(cx).is_some(), "and the change is still there");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-backspace" } else { "ctrl-backspace" });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(model.read(cx).dialog.is_none(), "⌘⌫ discards and closes it"));
    assert!(status(cx).is_none(), "the change is gone");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_changed_files_sort_by_the_clicked_column(cx: &mut TestAppContext) {
    use crate::model::SortColumn;
    let repo = demo_repo("sort");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Git, w, cx)));
    let order = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| model.read(cx).visible_files().iter().map(|f| f.path.clone()).collect::<Vec<_>>())
    };
    assert_eq!(order(cx), ["NOTES.md", "src/main.rs", "status.rs"], "by name at first");
    cx.update(|_, cx| model.update(cx, |m, cx| m.sort_by(SortColumn::Name, cx)));
    assert_eq!(order(cx), ["status.rs", "src/main.rs", "NOTES.md"], "a second click flips the order");
    cx.update(|_, cx| model.update(cx, |m, cx| m.sort_by(SortColumn::Directory, cx)));
    assert_eq!(order(cx), ["NOTES.md", "status.rs", "src/main.rs"]);
    cx.update(|_, cx| model.update(cx, |m, cx| m.sort_by(SortColumn::State, cx)));
    cx.run_until_parked();
    let states: Vec<String> =
        cx.update(|_, cx| model.read(cx).visible_files().iter().map(|f| f.state_label()).collect());
    let mut sorted = states.clone();
    sorted.sort();
    assert_eq!(states, sorted);
    // Shift-click ranges follow the order on screen.
    cx.update(|_, cx| model.update(cx, |m, cx| m.sort_by(SortColumn::Directory, cx)));
    cx.update(|_, cx| model.update(cx, |m, cx| m.click_file("NOTES.md", false, false, cx)));
    cx.update(|_, cx| model.update(cx, |m, cx| m.click_file("status.rs", true, false, cx)));
    cx.update(|_, cx| assert_eq!(model.read(cx).selected.len(), 2));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn a_folder_dropped_on_the_window_opens(cx: &mut TestAppContext) {
    use gpui::{ExternalPaths, FileDropEvent, point, px};
    let a = demo_repo("drop-a");
    let b = demo_repo("drop-b");
    let (model, _, cx) = open(cx, with_repo(&a));
    let position = point(px(600.), px(400.));
    let paths = ExternalPaths(vec![b.clone()].into());
    cx.simulate_event(FileDropEvent::Entered { position, paths });
    cx.simulate_event(FileDropEvent::Pending { position });
    cx.simulate_event(FileDropEvent::Submit { position });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.settings.current.as_deref(), Some(b.as_path()));
        assert!(m.settings.repos.contains(&b), "the dropped repository joins the list");
    });
    for dir in [a, b] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[gpui::test]
fn the_dock_layout_is_saved_and_restored(cx: &mut TestAppContext) {
    use gpui_component::dock::DockPlacement;
    let repo = demo_repo("layout");
    let (model, ws, vcx) = open(cx, with_repo(&repo));
    vcx.update(|w, cx| {
        ws.update(cx, |ws, cx| {
            ws.dock.update(cx, |d, cx| d.set_dock_size(DockPlacement::Left, gpui::px(410.), w, cx));
            ws.set_activity(Activity::Git, w, cx);
        })
    });
    // The save waits for the layout to settle.
    vcx.executor().advance_clock(std::time::Duration::from_secs(1));
    vcx.run_until_parked();
    let saved = vcx.update(|_, cx| model.read(cx).settings.clone());
    assert!(saved.dock.is_some(), "the layout was saved");

    // A new window over the saved settings has the same layout and every panel.
    let model2 = cx.update(|cx| cx.new(|cx| Model::new(saved.clone(), None, cx)));
    let mut ws2 = None;
    let (_, vcx2) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model2.clone(), window, cx));
        ws2 = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx2.run_until_parked();
    let ws2 = ws2.unwrap();
    vcx2.update(|_, cx| {
        let ws = ws2.read(cx);
        let d = ws.dock.read(cx);
        assert_eq!(d.dock_size(DockPlacement::Left), Some(gpui::px(410.)));
        let ids = [Some(ws.tabs.welcome), ws.tabs.console];
        assert!(ids.into_iter().flatten().all(|id| d.panel(id).is_some()), "every panel is placed");
        assert!(crate::console::console_of(ws, cx).is_some());
    });

    // A layout this build cannot read falls back to the usual one.
    let mut broken = saved;
    broken.dock = Some(serde_json::json!({ "version": 99, "center": "nonsense" }));
    let model3 = cx.update(|cx| cx.new(|cx| Model::new(broken, None, cx)));
    let mut ws3 = None;
    let (_, vcx3) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model3.clone(), window, cx));
        ws3 = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx3.run_until_parked();
    let ws3 = ws3.unwrap();
    vcx3.update(|_, cx| {
        assert_eq!(ws3.read(cx).dock.read(cx).dock_size(DockPlacement::Left), Some(gpui::px(320.)));
    });
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_git_view_sorts_switches_tabs_and_resizes_with_the_pointer(cx: &mut TestAppContext) {
    use gpui::{Modifiers, MouseButton, point, px};
    let repo = demo_repo("git-view");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Git, w, cx)));
    cx.run_until_parked();
    let git = cx.update(|_, cx| ws.read(cx).git.clone());
    // A click on a column header sorts by it; a second click flips the order.
    let state = cx.debug_bounds("files-head-sort-1").expect("the State header is drawn").center();
    cx.simulate_click(state, Modifiers::none());
    cx.update(|_, cx| assert_eq!(model.read(cx).sort, (crate::model::SortColumn::State, true)));
    cx.simulate_click(state, Modifiers::none());
    cx.update(|_, cx| assert_eq!(model.read(cx).sort, (crate::model::SortColumn::State, false)));
    // The handle between two columns' titles drags the first one wider.
    let name_w = cx.update(|_, cx| git.read(cx).file_column_widths(cx)[0].unwrap());
    let handle = cx.debug_bounds("files-head-resize-0").expect("the Name column's handle is drawn").center();
    cx.simulate_mouse_down(handle, MouseButton::Left, Modifiers::none());
    for dx in [10., 30., 40.] {
        cx.simulate_mouse_move(point(handle.x + px(dx), handle.y), MouseButton::Left, Modifiers::none());
    }
    cx.simulate_mouse_up(point(handle.x + px(40.), handle.y), MouseButton::Left, Modifiers::none());
    let wider = cx.update(|_, cx| git.read(cx).file_column_widths(cx)[0].unwrap());
    assert!((wider - name_w - 40.).abs() <= 8., "the Name column follows the pointer: {name_w} to {wider}");
    // The bottom pane's tabs.
    let output = cx.debug_bounds("tab-output").expect("the Output tab is drawn").center();
    cx.simulate_click(output, Modifiers::none());
    cx.update(|_, cx| assert!(git.read(cx).shows_output()));
    // Dragging the edge between the files and the commit box widens the box.
    let before = cx.update(|_, cx| git.read(cx).commit_width());
    let edge = cx.debug_bounds("split-Commit").expect("the edge is drawn").center();
    cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
    let to = point(edge.x - px(40.), edge.y);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.update(|_, cx| assert_eq!(git.read(cx).commit_width(), before + 40.));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn files_open_in_editor_tabs_with_their_grammar_and_save(cx: &mut TestAppContext) {
    use crate::editor::language_for;
    use gpui_component::highlighter::LanguageRegistry;
    let repo = demo_repo("editor");
    let files = [
        ("src/App.java", "class App {}\n", "java"),
        ("build.gradle.kts", "plugins { java }\n", "kotlin"),
        ("build.gradle", "apply plugin: 'java'\n", "groovy"),
        ("application.yml", "server:\n  port: 8080\n", "yaml"),
        ("gradle.properties", "org.gradle.jvmargs=-Xmx2g\n", "properties"),
        ("pom.xml", "<project>\n  <artifactId>demo</artifactId>\n</project>\n", "html"),
    ];
    for (name, text, _) in files {
        std::fs::write(repo.join(name), text).unwrap();
    }
    let (model, ws, cx) = open(cx, with_repo(&repo));
    for (name, text, language) in files {
        assert_eq!(language_for(&repo.join(name)), language);
        assert!(LanguageRegistry::singleton().language(language).is_some(), "{language} is registered");
        act(&model, cx, Action::OpenFile(repo.join(name), None));
        let editor =
            cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == repo.join(name)));
        let editor = editor.expect("the file has a tab");
        cx.update(|_, cx| assert_eq!(editor.read(cx).text(cx), text));
    }
    cx.update(|_, cx| {
        assert_eq!(ws.read(cx).editors(cx).len(), files.len());
        assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Project, "the docks come forward");
    });
    // Opening a file again brings its tab forward instead of a second one.
    act(&model, cx, Action::OpenFile(repo.join("build.gradle"), Some((0, 6))));
    cx.update(|_, cx| assert_eq!(ws.read(cx).editors(cx).len(), files.len()));
    // Type, then ⌘S saves.
    act(&model, cx, Action::OpenFile(repo.join("src/App.java"), Some((0, 0))));
    let editor = cx.update(|_, cx| ws.read(cx).editors(cx)[0].clone());
    cx.simulate_keystrokes("x");
    cx.update(|_, cx| assert!(editor.read(cx).dirty, "typing marks the tab"));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-s"));
    assert_eq!(std::fs::read_to_string(repo.join("src/App.java")).unwrap(), "xclass App {}\n");
    cx.update(|_, cx| assert!(!editor.read(cx).dirty));
    // The tabs are part of the saved layout and come back with their files.
    cx.update(|_, cx| ws.update(cx, |ws, cx| ws.save_layout(cx)));
    let layout = cx.update(|_, cx| model.read(cx).settings.dock.clone()).unwrap();
    assert!(layout.to_string().contains("application.yml"), "{layout}");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_file_tree_lists_the_repository_follows_the_disk_and_opens_files(cx: &mut TestAppContext) {
    use gpui::Modifiers;
    let repo = demo_repo("tree");
    std::fs::write(repo.join(".gitignore"), "build/\n").unwrap();
    std::fs::create_dir_all(repo.join("build")).unwrap();
    std::fs::write(repo.join("build/out.class"), "x").unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    cx.run_until_parked();
    let tree = cx.update(|_, cx| ws.read(cx).side.read(cx).files.clone());
    let names = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            tree.update(cx, |t, cx| t.rows(cx))
                .into_iter()
                .map(|(e, depth)| format!("{}{}", "  ".repeat(depth), e.path.file_name().unwrap().to_string_lossy()))
                .collect::<Vec<_>>()
        })
    };
    let top = names(cx);
    assert_eq!(top[..2], ["build", "src"], "folders first: {top:?}");
    assert!(top.contains(&"status.rs".to_owned()) && top.contains(&".gitignore".to_owned()), "{top:?}");
    assert!(!top.iter().any(|n| n == ".git"), ".git stays out: {top:?}");
    let ignored = cx.update(|_, cx| tree.update(cx, |t, cx| t.rows(cx)));
    let build = ignored.iter().find(|(e, _)| e.path.ends_with("build")).expect("ignored folders are listed");
    assert!(build.0.ignored, "and marked, to be dimmed");
    assert!(ignored.iter().filter(|(e, _)| !e.path.ends_with("build")).all(|(e, _)| !e.ignored));
    // A click on a folder opens it; one on a file opens an editor tab.
    let src = cx.debug_bounds("file-src").expect("the src row is drawn").center();
    cx.simulate_click(src, Modifiers::none());
    assert!(names(cx).contains(&"  main.rs".to_owned()));
    let main = cx.debug_bounds("file-main.rs").expect("the main.rs row is drawn").center();
    cx.simulate_click(main, Modifiers::none());
    cx.update(|_, cx| assert!(ws.read(cx).editors(cx).iter().any(|e| e.read(cx).path == repo.join("src/main.rs"))));
    // A file made elsewhere shows up once the watcher reports it.
    std::fs::write(repo.join("src/lib.rs"), "").unwrap();
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(30));
        cx.executor().advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        if names(cx).contains(&"  lib.rs".to_owned()) {
            break;
        }
    }
    assert!(names(cx).contains(&"  lib.rs".to_owned()), "{:?}", names(cx));
    let _ = model;
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn search_lists_matches_fuzzily_and_finds_text() {
    use crate::search::{go_to_file, list_files};
    let find_in_files = |host: &dyn snout_host::Host, root: &std::path::Path, files: &[String], text: &str| {
        host.find_text(root, files, text, crate::search::LIMIT).unwrap()
    };
    let repo = demo_repo("search-fn");
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    std::fs::create_dir_all(repo.join("target")).unwrap();
    std::fs::write(repo.join("target/main.rs"), "hello\n").unwrap();
    let host = snout_host::local();
    let files = list_files(&*host, &repo);
    assert!(files.contains(&"src/main.rs".to_owned()) && !files.iter().any(|f| f.starts_with("target/")), "{files:?}");
    assert!(!files.iter().any(|f| f.starts_with(".git/")));
    assert_eq!(go_to_file(&files, "mainrs")[0].0, "src/main.rs");
    assert_eq!(go_to_file(&files, "stat")[0].0, "status.rs");
    let hits = find_in_files(&*host, &repo, &files, "Hello");
    assert!(hits.is_empty(), "with a capital, case matters");
    let hits = find_in_files(&*host, &repo, &files, "hello, snout");
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].path.as_str(), hits[0].line, hits[0].column), ("src/main.rs", 1, 14));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn go_to_file_and_find_in_files_open_the_file(cx: &mut TestAppContext) {
    let repo = demo_repo("search-ui");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-p"));
    cx.update(|_, cx| assert!(ws.read(cx).search.is_some(), "⌘P puts up the box"));
    let search = cx.update(|_, cx| ws.read(cx).search.clone().unwrap());
    cx.simulate_input("mainrs");
    cx.update(|_, cx| assert_eq!(search.read(cx).items[0].path, "src/main.rs", "typing narrows the files"));
    cx.simulate_keystrokes("enter");
    cx.update(|_, cx| {
        let ws = ws.read(cx);
        assert!(ws.search.is_none(), "↩ closes it");
        assert!(ws.editors(cx).iter().any(|e| e.read(cx).path == repo.join("src/main.rs")));
    });
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-shift-f"));
    let find = cx.update(|_, cx| ws.read(cx).find_in_files.clone()).expect("⇧⌘F puts up Find in Files");
    cx.simulate_input("e");
    let settle = |cx: &mut VisualTestContext| {
        for _ in 0..20 {
            cx.executor().advance_clock(std::time::Duration::from_millis(100));
            cx.run_until_parked();
        }
    };
    settle(cx);
    let n = cx.update(|_, cx| find.read(cx).match_count());
    assert!(n > 2, "many lines hold an e");
    cx.simulate_keystrokes("down down up");
    cx.update(|_, cx| assert_eq!(find.read(cx).selected, 2, "the arrows pick (the first match is picked at first)"));
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| assert!(ws.read(cx).find_in_files.is_none(), "Esc closes it"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn replace_in_files_writes_ticked_matches_updates_open_editors_and_undoes(cx: &mut TestAppContext) {
    use crate::find_in_files::{Row, Scope};
    let repo = demo_repo("replace-ui");
    std::fs::write(repo.join("src/a.txt"), "total = total + 1\n").unwrap();
    std::fs::write(repo.join("src/b.txt"), "the total\n").unwrap();
    std::fs::write(repo.join("c.md"), "total\n").unwrap();
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let settle = |cx: &mut VisualTestContext| {
        for _ in 0..30 {
            cx.executor().advance_clock(std::time::Duration::from_millis(100));
            cx.run_until_parked();
        }
    };
    // b.txt is open, with an edit of its own the search must see.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(repo.join("src/b.txt"), None, false, w, cx)));
    settle(cx);
    let b =
        cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path.ends_with("b.txt")).unwrap());
    cx.update(|w, cx| b.update(cx, |e, cx| e.sync_from("the total, total\n", w, cx)));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_find_in_files(true, w, cx)));
    let find = cx.update(|_, cx| ws.read(cx).find_in_files.clone()).expect("⇧⌘R puts up Replace in Files");
    cx.update(|_, cx| assert!(find.read(cx).replace_mode));
    cx.update(|w, cx| {
        find.update(cx, |v, cx| {
            v.find.update(cx, |i, cx| i.set_value("total", w, cx));
            v.replace.update(cx, |i, cx| i.set_value("sum", w, cx));
            v.mask.update(cx, |i, cx| i.set_value("*.txt", w, cx));
            v.set_scope(Scope::Directory, cx);
        })
    });
    settle(cx);
    cx.update(|_, cx| {
        let v = find.read(cx);
        let paths: Vec<&str> = v.found.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/a.txt", "src/b.txt"], "the mask and the folder of the file in front");
        assert_eq!(v.match_count(), 4, "the open editor's text, not the file's");
    });
    // Leave out the second match of a.txt.
    cx.update(|_, cx| find.update(cx, |v, cx| v.toggle_hit(0, 1, cx)));
    cx.update(|_, cx| assert_eq!(find.read(cx).rows[2], Row::Hit(0, 1)));
    cx.update(|_, cx| find.update(cx, |v, cx| v.replace_all(cx)));
    settle(cx);
    assert_eq!(std::fs::read_to_string(repo.join("src/a.txt")).unwrap(), "sum = total + 1\n");
    assert_eq!(std::fs::read_to_string(repo.join("c.md")).unwrap(), "total\n", "outside the mask");
    cx.update(|_, cx| {
        assert_eq!(b.read(cx).text(cx), "the sum, sum\n", "the open editor follows at once");
        assert!(b.read(cx).dirty, "an editor with changes of its own is left unsaved");
        assert!(ws.read(cx).replace_undo.is_some());
    });
    assert_eq!(std::fs::read_to_string(repo.join("src/b.txt")).unwrap(), "the total\n");
    // One undo puts both back.
    cx.dispatch_action(crate::menus::UndoReplaceInFiles);
    settle(cx);
    assert_eq!(std::fs::read_to_string(repo.join("src/a.txt")).unwrap(), "total = total + 1\n");
    cx.update(|_, cx| assert_eq!(b.read(cx).text(cx), "the total, total\n"));
    // Replace Selected: only the match picked.
    cx.update(|_, cx| {
        find.update(cx, |v, cx| {
            v.selected = v.rows.iter().position(|r| *r == Row::Hit(1, 1)).unwrap();
            v.replace_selected(cx);
        })
    });
    settle(cx);
    cx.update(|_, cx| assert_eq!(b.read(cx).text(cx), "the total, sum\n"));
    assert_eq!(std::fs::read_to_string(repo.join("src/a.txt")).unwrap(), "total = total + 1\n");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_terminal_tab_runs_a_shell_in_the_repository(cx: &mut TestAppContext) {
    terminal_tab_on(EngineKind::Builtin, cx);
}

#[cfg(feature = "ghostty")]
#[gpui::test]
fn the_terminal_tab_runs_on_ghostty(cx: &mut TestAppContext) {
    terminal_tab_on(EngineKind::Ghostty, cx);
}

fn terminal_tab_on(engine: EngineKind, cx: &mut TestAppContext) {
    use gpui_component::dock::DockPlacement;
    let repo = demo_repo(&format!("terminal-{}", engine.label().to_lowercase()));
    // The shell and a variable from the settings; text copied as soon as it is selected.
    let mut settings = Settings { terminal_engine: engine, ..with_repo(&repo) };
    // On Windows, Git for Windows's bash.
    settings.terminal_shell =
        if cfg!(windows) { r"C:\Program Files\Git\bin\bash.exe".into() } else { "/bin/sh".into() };
    settings.terminal_shell_args = Vec::new();
    settings.terminal_env.insert("SNOUT_TERM_VAR".into(), "from-the-settings".into());
    // The prompt the test waits for, as root too.
    settings.terminal_env.insert("PS1".into(), "$ ".into());
    settings.terminal_copy_on_select = true;
    let (_, ws, cx) = open(cx, settings);
    let terminal = cx.update(|_, cx| {
        let ws = ws.read(cx);
        let view = ws.dock.read(cx).panel(ws.tabs.terminal).expect("the terminal is docked").clone();
        view.view().downcast::<crate::console::ConsolePanel>().unwrap()
    });
    cx.update(|_, cx| assert_eq!(terminal.read(cx).kind, crate::console::Kind::Shell));
    // Show the docks, open the bottom dock and bring the terminal forward.
    cx.update(|w, cx| {
        ws.update(cx, |ws, cx| {
            ws.show_activity(Activity::Project, w, cx);
            let id = ws.tabs.terminal;
            ws.dock.update(cx, |d, cx| {
                if !d.is_dock_open(DockPlacement::Bottom) {
                    d.toggle_dock(DockPlacement::Bottom, w, cx);
                }
                d.select_panel(id, w, cx);
            });
        })
    });
    // Real time: the program starts in a pseudo terminal, which takes a while when the
    // machine is busy (the engine tests run beside this one).
    let wait = |cx: &mut VisualTestContext, what: &str| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            cx.executor().advance_clock(std::time::Duration::from_millis(50));
            cx.run_until_parked();
            if cx.update(|_, cx| terminal.read(cx).session_text(&repo)).unwrap_or_default().contains(what) {
                return;
            }
        }
        panic!("{what:?} never appeared: {:?}", cx.update(|_, cx| terminal.read(cx).session_text(&repo)));
    };
    wait(cx, "$");
    cx.update(|_, cx| assert_eq!(terminal.read(cx).session_engine(&repo), Some(engine)));
    cx.simulate_keystrokes("p w d enter");
    wait(cx, repo.file_name().unwrap().to_str().unwrap());
    cx.update(|_, cx| terminal.read(cx).type_keys(&repo, b"echo $SNOUT_TERM_VAR\r"));
    wait(cx, "\nfrom-the-settings");
    // A drag over the first row selects it, and the selection is on the clipboard at once.
    let grid = cx.debug_bounds("terminal-grid").expect("the terminal is drawn");
    let (from, to) = (
        grid.origin + gpui::point(gpui::px(8.), gpui::px(8.)),
        grid.origin + gpui::point(gpui::px(200.), gpui::px(8.)),
    );
    cx.simulate_mouse_down(from, gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(to, Some(gpui::MouseButton::Left), gpui::Modifiers::none());
    cx.simulate_mouse_up(to, gpui::MouseButton::Left, gpui::Modifiers::none());
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text())).unwrap_or_default();
    assert!(!copied.trim().is_empty(), "copy on select put the selection on the clipboard");
    // A click lets go of the selection (⌘F would look for it).
    cx.simulate_click(from, gpui::Modifiers::none());
    // A colourful line, a link, and ⌘F finding text in the scrollback.
    let typed =
        "printf '\\033[32mgreen\\033[0m https://example.com\\n'; for i in $(seq 1 60); do echo filler $i; done\r";
    cx.update(|_, cx| terminal.read(cx).type_keys(&repo, typed.as_bytes()));
    wait(cx, "filler 60");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-f" } else { "ctrl-shift-f" });
    assert!(cx.debug_bounds("terminal-find").is_some(), "⌘F opens the find bar");
    cx.simulate_input("green");
    cx.run_until_parked();
    cx.update(|_, cx| {
        let (found, current) = terminal.read(cx).find_state().expect("the find bar is open");
        assert!(found >= 1 && current.is_some(), "{found} matches, current {current:?}");
    });
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| assert!(terminal.read(cx).find_state().is_none(), "Esc closes the find bar"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_title_bar_shows_the_project_and_its_branches(cx: &mut TestAppContext) {
    let repo = demo_repo("title-bar");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.run_until_parked();
    assert!(cx.debug_bounds("title-project").is_some(), "the project widget is drawn");
    let branch = cx.debug_bounds("title-branch").expect("the branch widget is drawn").center();
    cx.simulate_click(branch, gpui::Modifiers::none());
    cx.run_until_parked();
    let popup = cx.update(|_, cx| crate::workspace::branch_popup(ws.read(cx))).expect("the popup opens");
    cx.update(|_, cx| {
        let (local, remote) = popup.read(cx).branches(cx);
        assert_eq!(local[0], ("main".to_owned(), true), "the current branch comes first");
        assert!(local.iter().any(|(b, _)| b == "feature/diff-view"));
        assert!(remote.is_empty());
    });
    cx.simulate_input("diff");
    cx.update(|_, cx| {
        let (local, _) = popup.read(cx).branches(cx);
        assert_eq!(local, [("feature/diff-view".to_owned(), false)], "the search narrows the list");
    });
    let row = cx.debug_bounds("branch-local-0").expect("the branch row is drawn").center();
    cx.simulate_click(row, gpui::Modifiers::none());
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(crate::workspace::branch_popup(ws.read(cx)).is_none(), "a choice closes the popup");
        let m = model.read(cx);
        let asked = matches!(&m.dialog, Some(crate::model::Dialog::ConfirmCheckout(snout_core::CheckoutTarget::Branch(b))) if b == "feature/diff-view");
        assert!(asked || m.head_label() == "feature/diff-view", "the branch is checked out (or asked about)");
    });
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_stripes_open_and_close_the_tool_windows(cx: &mut TestAppContext) {
    use crate::workspace::Tool;
    use gpui_component::dock::DockPlacement;
    let repo = demo_repo("stripes");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    cx.run_until_parked();
    let open = |cx: &mut VisualTestContext, t: Tool| cx.update(|_, cx| ws.read(cx).tool_open(t, cx));
    let dock_open =
        |cx: &mut VisualTestContext, place| cx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(place));
    assert!(!dock_open(cx, DockPlacement::Right), "the right tool windows start hidden");
    let click = |cx: &mut VisualTestContext, id: &'static str| {
        let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} is drawn")).center();
        cx.simulate_click(at, gpui::Modifiers::none());
        cx.run_until_parked();
    };
    click(cx, "stripe-terminal");
    assert!(open(cx, Tool::Terminal), "the terminal shows");
    assert!(!open(cx, Tool::Output));
    click(cx, "stripe-output");
    assert!(open(cx, Tool::Output) && !open(cx, Tool::Terminal), "Output takes its place");
    assert!(cx.debug_bounds("tool-header").is_some(), "a tool window has a header, not a tab bar");
    click(cx, "stripe-output");
    assert!(!dock_open(cx, DockPlacement::Bottom), "a second click hides it");
    click(cx, "stripe-gradle");
    assert!(open(cx, Tool::Gradle) && dock_open(cx, DockPlacement::Right));
    click(cx, "stripe-notifications");
    assert!(open(cx, Tool::Notifications) && !open(cx, Tool::Gradle));
    click(cx, "tool-hide");
    assert!(!dock_open(cx, DockPlacement::Right), "the header's hide button hides the window");
    // From the Git view, a bottom tool window brings the project back.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    click(cx, "stripe-problems");
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Project));
    assert!(open(cx, Tool::Problems));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_settings_sheet_edits_a_copy_until_apply_or_ok(cx: &mut TestAppContext) {
    use crate::settings::ThemeChoice;
    let repo = demo_repo("settings-sheet");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.dispatch_action(crate::menus::OpenSettings);
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings").is_some(), "⌘, opens the sheet");
    let click = |cx: &mut VisualTestContext, id: &'static str| {
        let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} is drawn")).center();
        cx.simulate_click(at, gpui::Modifiers::none());
        cx.run_until_parked();
    };
    click(cx, "settings-theme-light");
    cx.update(|_, cx| assert_ne!(model.read(cx).settings.theme, ThemeChoice::Light, "only the copy changes"));
    click(cx, "settings-apply");
    cx.update(|_, cx| {
        assert_eq!(model.read(cx).settings.theme, ThemeChoice::Light, "Apply puts it in place");
        assert!(!crate::theme::palette(cx).dark, "and the window follows");
        assert!(ws.read(cx).settings_sheet.is_some(), "Apply leaves the sheet open");
        assert_eq!(crate::theme::palette(cx).theme, crate::settings::ColorTheme::Studio, "Studio by default");
    });
    click(cx, "settings-colors-classic");
    click(cx, "settings-apply");
    cx.update(|_, cx| {
        assert_eq!(model.read(cx).settings.colors, crate::settings::ColorTheme::Classic);
        assert_eq!(crate::theme::palette(cx), crate::theme::Palette::new(crate::settings::ColorTheme::Classic, false));
    });
    click(cx, "settings-page-editor");
    click(cx, "settings-tab-size-up");
    click(cx, "settings-soft-wrap");
    click(cx, "settings-cancel");
    cx.update(|_, cx| {
        let s = &model.read(cx).settings;
        assert_eq!((s.tab_size, s.soft_wrap), (4, false), "Cancel drops the copy");
        assert!(ws.read(cx).settings_sheet.is_none());
    });
    cx.dispatch_action(crate::menus::OpenSettings);
    cx.run_until_parked();
    click(cx, "settings-page-editor");
    click(cx, "settings-tab-size-down");
    click(cx, "settings-ok");
    cx.update(|_, cx| {
        assert_eq!(model.read(cx).settings.tab_size, 3, "OK puts it in place");
        assert!(ws.read(cx).settings_sheet.is_none(), "and closes the sheet");
    });
    // Gradle: how long an idle daemon lives.
    let idle = cx.update(|_, cx| model.read(cx).settings.daemon_idle_minutes);
    cx.dispatch_action(crate::menus::OpenSettings);
    cx.run_until_parked();
    click(cx, "settings-page-gradle");
    click(cx, "settings-daemon-idle-up");
    click(cx, "settings-ok");
    cx.update(|_, cx| assert!(model.read(cx).settings.daemon_idle_minutes > idle, "OK keeps the daemons' idle time"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn background_tasks_show_in_the_status_bar_and_its_popover(cx: &mut TestAppContext) {
    let repo = demo_repo("bg-tasks");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.run_until_parked();
    assert!(cx.debug_bounds("status-tasks").is_none(), "nothing runs");
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    cx.update(|_, cx| {
        hub.update(cx, |h, cx| {
            h.engine_build = Some(crate::background::EngineBuild::parse("stage Building snout-engine\n"));
            cx.notify();
        });
        model.update(cx, |m, cx| {
            m.job = Some("Fetching".into());
            cx.notify();
        });
    });
    cx.run_until_parked();
    let tasks = cx.update(|_, cx| ws.read(cx).status.read(cx).tasks(cx));
    assert_eq!(tasks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Building snout-engine", "Git"]);
    let item = cx.debug_bounds("status-tasks").expect("the status bar shows them").center();
    cx.simulate_click(item, gpui::Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tasks-popover").is_some(), "a click lists them");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn a_java_engine_that_stopped_says_it_starts_again_then_offers_restart(cx: &mut TestAppContext) {
    let repo = demo_repo("recovering");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    cx.update(|_, cx| {
        hub.update(cx, |h, cx| {
            h.insert_published(&repo, "file:///x".into(), Vec::new());
            let s = h.servers.get_mut(&repo).unwrap();
            s.stage = crate::lang::hub::Stage::Restarting;
            s.recovering = Some(2);
            cx.notify();
        });
    });
    cx.run_until_parked();
    let tasks = cx.update(|_, cx| ws.read(cx).status.read(cx).tasks(cx));
    let java = tasks.iter().find(|t| t.title.starts_with("Java")).expect("the restart shows as a task");
    assert!(java.stage.starts_with("The Java engine stopped unexpectedly and is starting again (attempt 2 of 5)"));
    assert_eq!(java.compact.as_deref(), Some("Java stopped, starting again"));
    // Given up on: the status bar says so, with Restart.
    cx.update(|_, cx| {
        hub.update(cx, |h, cx| {
            let s = h.servers.get_mut(&repo).unwrap();
            s.stage = crate::lang::hub::Stage::Failed("The Java engine keeps stopping".into());
            s.recovering = None;
            s.jdk = Some(crate::lang::java::Jdk { home: repo.clone(), version: "21".into(), major: 21 });
            cx.notify();
        });
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("status-java-restart").is_some(), "Restart is offered");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_keymap_page_rebinds_a_command_by_its_keys(cx: &mut TestAppContext) {
    let repo = demo_repo("keymap");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.dispatch_action(crate::menus::OpenSettings);
    cx.run_until_parked();
    let click = |cx: &mut VisualTestContext, id: &str| {
        let id: &'static str = Box::leak(id.to_owned().into_boxed_str());
        let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} is drawn")).center();
        cx.simulate_click(at, gpui::Modifiers::none());
        cx.run_until_parked();
    };
    click(cx, "settings-page-keymap");
    let sheet = cx.update(|_, cx| ws.read(cx).settings_sheet.clone().unwrap());
    let ix = cx.update(|_, cx| {
        crate::keymap::entries(&Default::default(), cx).iter().position(|e| e.action == "snout::GoToFile").unwrap()
    });
    // + waits for keys; ⌘K is taken for Go to File, not run.
    click(cx, &format!("keymap-add-{ix}"));
    assert!(cx.debug_bounds("keymap-recording").is_some(), "the row waits for the keys");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-k"));
    cx.update(|_, cx| {
        let s = sheet.read(cx);
        assert!(s.recording.is_none());
        let k = crate::keymap::platform_keys;
        assert_eq!(s.draft.keymap.get("snout::GoToFile"), Some(&vec![k("cmd-p"), k("cmd-k")]));
    });
    // × takes ⌘P away; OK puts it in force at once and saves it.
    click(cx, &format!("keymap-remove-{ix}-0"));
    click(cx, "settings-ok");
    cx.update(|_, cx| {
        let k = crate::keymap::platform_keys("cmd-k");
        assert_eq!(model.read(cx).settings.keymap.get("snout::GoToFile"), Some(&vec![k]));
        assert!(ws.read(cx).search.is_none());
    });
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-p"));
    cx.update(|_, cx| assert!(ws.read(cx).search.is_none(), "⌘P no longer goes to a file"));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-k"));
    cx.update(|_, cx| assert!(ws.read(cx).search.is_some(), "⌘K does"));
    let _ = std::fs::remove_dir_all(repo);
}

/// Pump the test window until `done`, for work on the background executor.
fn pump(cx: &mut VisualTestContext, secs: u64, mut done: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        if done(cx) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

#[gpui::test]
fn the_new_project_dialog_makes_java_and_spring_boot_projects_and_opens_them(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let repo = demo_repo("new-project-from");
    let place = scratch_dir("new-project-into");
    // start.spring.io stands in as an address nothing answers: the offline template.
    cx.update(|cx| cx.set_global(crate::new_project::ServiceGlobal("http://127.0.0.1:9".into())));
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let click = |cx: &mut VisualTestContext, id: &str| {
        let id: &'static str = Box::leak(id.to_owned().into_boxed_str());
        let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} is drawn")).center();
        cx.simulate_click(at, gpui::Modifiers::none());
        cx.run_until_parked();
    };
    let fill = |cx: &mut VisualTestContext, name: &str| {
        let (name, place) = (name.to_owned(), place.display().to_string());
        cx.update(|w, cx| {
            let view = ws.read(cx).new_project.clone().unwrap();
            let (n, l) = (view.read(cx).name.clone(), view.read(cx).location.clone());
            n.update(cx, |i, cx| i.set_value(name, w, cx));
            l.update(cx, |i, cx| i.set_value(place, w, cx));
        });
        cx.run_until_parked();
    };

    // A plain Java project, Groovy DSL, Java 17.
    cx.dispatch_action(crate::menus::NewProject);
    cx.run_until_parked();
    assert!(cx.debug_bounds("new-project").is_some(), "File ▸ New Project… puts up the dialog");
    fill(cx, "hello-app");
    click(cx, "new-project-groovy");
    cx.update(|_, cx| {
        let view = ws.read(cx).new_project.clone().unwrap();
        view.update(cx, |v, _| v.java = 17);
        let b = view.read(cx).basics(cx);
        assert_eq!((b.artifact.as_str(), b.package.as_str()), ("hello-app", "com.example.helloapp"));
        assert_eq!(view.read(cx).problem(cx), None);
    });
    click(cx, "new-project-create");
    let dir = place.join("hello-app");
    assert!(
        pump(cx, 30, |cx| cx.update(|_, cx| model.read(cx).project_root().as_deref() == Some(dir.as_path()))),
        "the new project opens"
    );
    assert!(dir.join("build.gradle").is_file() && dir.join(".git").exists());
    assert!(std::fs::read_to_string(dir.join("build.gradle")).unwrap().contains("JavaLanguageVersion.of(17)"));
    assert!(dir.join("src/main/java/com/example/helloapp/Main.java").is_file());
    cx.update(|_, cx| assert!(ws.read(cx).new_project.is_none(), "the dialog goes"));
    // The name is taken now: the dialog says so and Create waits.
    cx.dispatch_action(crate::menus::NewProject);
    cx.run_until_parked();
    fill(cx, "hello-app");
    cx.update(|_, cx| {
        let view = ws.read(cx).new_project.clone().unwrap();
        assert!(view.read(cx).problem(cx).unwrap().contains("not empty"));
    });

    // Spring Boot without start.spring.io: the known dependencies, and a project made here.
    click(cx, "new-project-kind-spring-boot");
    assert!(
        pump(cx, 30, |cx| cx.update(|_, cx| ws.read(cx).new_project.as_ref().unwrap().read(cx).catalog.is_some())),
        "the dependencies come (the offline list)"
    );
    cx.update(|_, cx| {
        let v = ws.read(cx).new_project.clone().unwrap();
        assert!(v.read(cx).catalog_error.as_ref().unwrap().contains("could not be reached"));
    });
    fill(cx, "shop");
    click(cx, "new-project-next");
    click(cx, "new-project-dep-web");
    click(cx, "new-project-dep-data-jpa");
    click(cx, "new-project-create");
    let shop = place.join("shop");
    assert!(
        pump(cx, 30, |cx| cx.update(|_, cx| model.read(cx).project_root().as_deref() == Some(shop.as_path()))),
        "the Spring Boot project opens"
    );
    let build = std::fs::read_to_string(shop.join("build.gradle.kts")).unwrap();
    assert!(build.contains("spring-boot-starter-webmvc") && build.contains("spring-boot-starter-data-jpa"), "{build}");
    assert!(shop.join("src/main/java/com/example/shop/ShopApplication.java").is_file());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(place);
}
