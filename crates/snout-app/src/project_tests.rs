//! Headless tests of the project: the folder opened is the project (Git found above it),
//! and the Project tool window shows the project as its Gradle model has it.

use std::path::Path;
use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use crate::model::Action;
use crate::panels::Activity;
use crate::project_view::{NodeId, RowKind};
use crate::settings::Settings;
use crate::tests::{act, demo_repo, open, with_repo};

include!("../tests/fixtures/project_fixture.rs");

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..4 {
        cx.executor().advance_clock(Duration::from_millis(1600));
        cx.run_until_parked();
    }
}

#[gpui::test]
fn a_folder_inside_a_repository_is_the_project(cx: &mut TestAppContext) {
    let repo = demo_repo("project-folder");
    let sub = repo.join("services/web");
    std::fs::create_dir_all(sub.join("src")).unwrap();
    std::fs::write(sub.join("src/Web.java"), "class Web {}\n").unwrap();
    std::fs::write(sub.join("README.md"), "web\n").unwrap();
    crate::tests::git(&repo, &["add", "-A"]);
    crate::tests::git(&repo, &["commit", "-qm", "Add the web service"]);
    std::fs::write(sub.join("README.md"), "web, changed\n").unwrap();
    let (model, ws, cx) = open(cx, Settings::default());
    act(&model, cx, Action::OpenPath(sub.clone()));
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.project_root().as_deref(), Some(sub.as_path()), "the folder as opened");
        assert_eq!(m.repo_root().as_deref(), Some(repo.as_path()), "Git works on the repository above");
        assert_eq!(m.settings.current.as_deref(), Some(sub.as_path()));
        assert_eq!(m.recent_projects().first().map(|p| p.as_path()), Some(sub.as_path()));
        assert_eq!(m.settings.repos, std::slice::from_ref(&repo), "the repository list keeps repositories");
        assert!(m.title().starts_with("web ("), "{}", m.title());
        assert!(m.status.files.iter().any(|f| f.path == "services/web/README.md"), "Git sees the change");
    });
    // The tree lists the folder, and colours its changed file by Git's paths.
    let tree = cx.update(|_, cx| ws.read(cx).side.read(cx).files.clone());
    let rows = cx.update(|_, cx| tree.update(cx, |t, cx| t.rows(cx)));
    let names: Vec<String> =
        rows.iter().map(|(e, _)| e.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["src", "README.md"]);
    let marks = cx.update(|_, cx| {
        let m = model.read(cx);
        crate::files::git_marks(&m.repo_root().unwrap(), &m.status.files)
    });
    assert!(marks.contains_key(&sub.join("README.md")));
    // Opening the recent project opens the folder again, not the repository's root.
    act(&model, cx, Action::CloseRepo);
    act(&model, cx, Action::OpenRepo(sub.clone()));
    cx.update(|_, cx| assert_eq!(model.read(cx).project_root().as_deref(), Some(sub.as_path())));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_project_view_shows_modules_shared_folders_failed_builds_and_libraries(cx: &mut TestAppContext) {
    let repo = demo_repo("project-view");
    let jars = repo.with_extension("jars");
    let json = write_project_fixture(&repo, &jars);
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    cx.run_until_parked();
    // Until the model is read: the plain tree, and a note.
    assert!(cx.debug_bounds("project-model-note").is_some(), "the note says the model is on its way");
    assert!(cx.debug_bounds("file-app").is_some(), "the plain tree shows meanwhile");
    let (side, store) = cx.update(|_, cx| {
        let side = ws.read(cx).side.clone();
        (side, crate::project_model::store(cx).unwrap())
    });
    let parsed = crate::project_model::parse(&serde_json::from_str(&json).unwrap()).unwrap();
    cx.update(|_, cx| store.update(cx, |s, cx| s.set_model(parsed, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("project-model-note").is_none());
    for row in ["shop", "app", "billing", "core", "legacy", "Gradle Scripts", "External Libraries"] {
        assert!(cx.debug_bounds(Box::leak(format!("project-row-{row}").into_boxed_str())).is_some(), "{row} is drawn");
    }
    // A failed build shows why on a click.
    let legacy = cx.debug_bounds("project-row-legacy").unwrap().center();
    cx.simulate_click(legacy, Modifiers::none());
    assert!(cx.debug_bounds("project-row-Gradle 6.0.1 needs Java 13 or older").is_some(), "the reason shows");
    // The module opens to its source roots; the libraries to packages and classes.
    let project = cx.update(|_, cx| side.read(cx).project.clone());
    cx.update(|_, cx| {
        project.update(cx, |v, cx| {
            v.expanded.insert(NodeId::Module("shop:app".into()));
            v.expanded.insert(NodeId::Dir(repo.join("app/src")));
            v.expanded.insert(NodeId::Dir(repo.join("app/src/main")));
            v.expanded.insert(NodeId::Libraries);
            v.expanded.insert(NodeId::Library("com.google.guava:guava:33.0.0-jre".into()));
            cx.notify();
        })
    });
    cx.run_until_parked();
    let rows = cx.update(|_, cx| project.update(cx, |v, cx| v.rows(cx)));
    assert!(rows.iter().any(|r| matches!(&r.kind, RowKind::Dir { path, root: Some(crate::project_model::RootKind::Main), .. } if path == &repo.join("app/src/main/java"))));
    assert!(rows.iter().any(|r| matches!(&r.kind, RowKind::Package { name, .. } if name == "com.google.common.base")));
    // Locate File opens the nodes down to the file.
    cx.update(|_, cx| project.update(cx, |v, cx| v.reveal(&repo.join("core/src/main/java/core/Money.java"), cx)));
    // The folders are read off the UI thread, each once the one above it is in.
    cx.run_until_parked();
    let rows = cx.update(|_, cx| project.update(cx, |v, cx| v.rows(cx)));
    assert!(rows.iter().any(|r| matches!(&r.kind, RowKind::File { path, .. } if path.ends_with("Money.java"))));
    // A click on a file opens it.
    let at = cx.debug_bounds("project-row-Money.java").expect("the file is drawn").center();
    cx.simulate_click(at, Modifiers::none());
    cx.update(|_, cx| {
        let opened =
            ws.read(cx).editors(cx).iter().any(|e| e.read(cx).path.ends_with("core/src/main/java/core/Money.java"));
        assert!(opened, "the file opens in a tab");
    });
    // A Gradle script changes: the bar offers to sync; Not now puts it away.
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(repo.join("app/build.gradle"), "plugins { id 'java-library' }\n").unwrap();
    cx.update(|_, cx| store.update(cx, |s, cx| s.poll(cx)));
    settle(cx);
    assert!(cx.update(|_, cx| store.read(cx).scripts_changed));
    let dismiss = cx.debug_bounds("gradle-dismiss").expect("the bar shows").center();
    cx.simulate_click(dismiss, Modifiers::none());
    assert!(cx.debug_bounds("gradle-bar").is_none());
    // Files: the plain tree again.
    cx.update(|_, cx| side.update(cx, |s, cx| s.set_plain_files(true, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("project-tree").is_none() && cx.debug_bounds("file-app").is_some());
    cx.update(|_, cx| assert!(model.read(cx).settings.plain_files, "the choice is kept"));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(jars);
}

#[test]
fn the_fixture_is_the_model_the_view_reads() {
    let root = std::env::temp_dir().join(format!("snout-fixture-{}", std::process::id()));
    let json = write_project_fixture(&root, &root.join("jars"));
    let model = crate::project_model::parse(&serde_json::from_str(&json).unwrap()).unwrap();
    assert!(Path::new(&model.libraries[0].file.clone().unwrap()).is_file());
    let _ = std::fs::remove_dir_all(root);
}
