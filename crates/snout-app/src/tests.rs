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
