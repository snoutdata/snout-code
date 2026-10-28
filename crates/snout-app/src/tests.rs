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

pub fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success();
    assert!(ok, "git {args:?}");
}

pub fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("snout-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
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

pub fn with_repo(repo: &Path) -> Settings {
    Settings { repos: vec![repo.to_owned()], current: Some(repo.to_owned()), ..Settings::default() }
}

pub fn act(model: &Entity<Model>, cx: &mut VisualTestContext, action: Action) {
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(action, cx)));
    cx.run_until_parked();
}

#[gpui::test]
fn the_window_opens_on_the_welcome_page_without_a_repository(cx: &mut TestAppContext) {
    let (model, ws, cx) = open(cx, Settings::default());
    cx.update(|_, cx| {
        assert!(model.read(cx).repo.is_none());
        assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Projects);
        assert_eq!(model.read(cx).title(), "SnoutCode");
    });
}

#[gpui::test]
fn projects_groups_hold_repositories_and_the_tree_narrows_the_files(cx: &mut TestAppContext) {
    let a = demo_repo("projects-a");
    let b = demo_repo("projects-b");
    let mut settings = with_repo(&a);
    settings.repos.push(b.clone());
    let (model, ws, cx) = open(cx, settings);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Projects, w, cx)));

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
fn the_activity_bar_switches_and_closes_the_left_dock(cx: &mut TestAppContext) {
    let (_, ws, cx) = open(cx, Settings::default());
    let left_open = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(gpui_component::dock::DockPlacement::Left))
    };
    assert!(left_open(cx));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    assert!(left_open(cx));
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Git));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    assert!(!left_open(cx), "the active item closes the dock");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Projects, w, cx)));
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
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_msg.trim(), "The rest"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn claude_drafts_the_commit_message_from_the_patch(cx: &mut TestAppContext) {
    let repo = demo_repo("draft");
    let bin = scratch_dir("draft-bin");
    // A stand-in for `claude -p`: it reads the patch and answers with a message naming a
    // file in it.
    let fake = bin.join("claude");
    let script = "#!/bin/sh\nif grep -q 'src/main.rs' ; then echo 'Greet snout'; else echo 'Wrong patch'; fi\n";
    std::fs::write(&fake, script).unwrap();
    Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
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
    let repo = demo_repo("console");
    let bin = scratch_dir("console-bin");
    // A stand-in for `claude`: it says where it runs, then makes the file it is told to.
    let fake = bin.join("claude");
    let script = "#!/bin/sh\necho \"ready in $(pwd)\"\nread name\ntouch \"$name.txt\"\necho \"made $name\"\nsleep 30\n";
    std::fs::write(&fake, script).unwrap();
    Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).expect("the console is docked");
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    act(&model, cx, Action::ToggleConsole);
    let wait = |cx: &mut VisualTestContext, what: &str| {
        for _ in 0..300 {
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
    let text = wait(cx, "ready in");
    assert!(text.contains(repo.file_name().unwrap().to_str().unwrap()), "{text}");
    cx.simulate_keystrokes("n o t e enter");
    wait(cx, "made note");
    assert!(repo.join("note.txt").exists());
    // Once the agent goes quiet, the Git views read the repository again.
    for _ in 0..60 {
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
    cx.simulate_keystrokes("cmd-0");
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, Activity::Git));
    cx.simulate_keystrokes("cmd-a");
    cx.update(|_, cx| assert_eq!(model.read(cx).selected.len(), 3, "⌘A selects every listed file"));
    let first = cx.update(|_, cx| model.read(cx).log[1].id.clone());
    act(&model, cx, Action::ShowCommit(first));
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| assert!(model.read(cx).commit_view.is_none(), "Esc leaves the commit view"));
    cx.simulate_keystrokes("cmd-backspace");
    cx.update(|_, cx| {
        let dialog = &model.read(cx).dialog;
        assert!(matches!(dialog, Some(crate::model::Dialog::ConfirmDeleteFiles(f)) if f.len() == 3), "{dialog:?}");
    });
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
