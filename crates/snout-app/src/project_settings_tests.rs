//! Headless tests of the project's own settings: `.snout/settings.json` over the user's, read
//! again as it changes on disk, its errors keeping the last good values, and the Settings
//! Settings sheet (the user's) beside Project Settings (the project's).

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::model::Model;
use crate::project_settings::{self as ps, Key, ProjectSettings};
use crate::quit_tests::{click, open_stored};
use crate::settings::{Settings, Store};
use crate::settings_view::Scope;
use crate::tests::{demo_repo, with_repo};
use crate::workspace::Workspace;

fn write(repo: &std::path::Path, text: &str) {
    std::fs::create_dir_all(repo.join(".snout")).unwrap();
    std::fs::write(ps::file_of(repo), text).unwrap();
    // A stamp of its own, even on a file system that keeps whole seconds.
    let later = std::time::SystemTime::now() + Duration::from_secs(rand_secs());
    let f = std::fs::File::options().write(true).open(ps::file_of(repo)).unwrap();
    f.set_modified(later).unwrap();
}

fn rand_secs() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

/// The workspace looks at the file again (as its poll does every second).
fn look_again(ws: &Entity<Workspace>, cx: &mut VisualTestContext) {
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.check_project_settings(true, w, cx)));
    cx.run_until_parked();
}

fn tab_and_wrap(model: &Entity<Model>, cx: &mut VisualTestContext) -> (usize, bool) {
    cx.update(|_, cx| {
        let s = &model.read(cx).settings;
        (s.tab_size, s.soft_wrap)
    })
}

#[gpui::test]
fn the_projects_file_overrides_the_users_settings_and_is_read_again_as_it_changes(cx: &mut TestAppContext) {
    let repo = demo_repo("project-settings-live");
    write(&repo, r#"{ "tab_size": 2, "soft_wrap": true, "spring_profiles": ["dev"] }"#);
    let file = crate::tests::scratch_dir("project-settings-live-store").join("settings.json");
    let user = Settings { tab_size: 4, ..with_repo(&repo) };
    let (model, ws, cx) = open_stored(cx, user, Store::at(file.clone()));
    assert_eq!(tab_and_wrap(&model, cx), (2, true), "the project's values are in use");
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!((m.user_settings().tab_size, m.user_settings().soft_wrap), (4, false), "the user's are kept");
        assert_eq!(m.spring_mode(), crate::spring::ProfileMode::Chosen(vec!["dev".into()]));
    });
    // Saving the user's settings saves the user's values, not the project's.
    cx.update(|_, cx| model.update(cx, |m, _| m.persist()));
    assert_eq!(Store::at(file.clone()).load().tab_size, 4);
    // The file changes on disk: read again, and what it no longer sets is the user's.
    write(&repo, r#"{ "tab_size": 3 }"#);
    look_again(&ws, cx);
    assert_eq!(tab_and_wrap(&model, cx), (3, false));
    // A mistake in it: the values read last stay, and the error shows.
    write(&repo, r#"{ "tab_size": 5,"#);
    look_again(&ws, cx);
    assert_eq!(tab_and_wrap(&model, cx), (3, false), "the last good values stay");
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert!(
            m.project_settings_error.as_deref().is_some_and(|e| e.contains("EOF")),
            "{:?}",
            m.project_settings_error
        );
        assert!(m.message.contains(ps::FILE), "{}", m.message);
    });
    write(&repo, r#"{ "tab_size": 5 }"#);
    look_again(&ws, cx);
    assert_eq!(tab_and_wrap(&model, cx), (5, false));
    cx.update(|_, cx| assert!(model.read(cx).project_settings_error.is_none()));
    // The file goes: nothing is overridden.
    std::fs::remove_file(ps::file_of(&repo)).unwrap();
    look_again(&ws, cx);
    assert_eq!(tab_and_wrap(&model, cx), (4, false));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

/// Settings shows the user's settings only, saying which ones the project sets for itself;
/// Project Settings edits the project's, each the user's until the project sets its own.
#[gpui::test]
fn settings_are_the_users_and_project_settings_edit_the_projects_and_reset_an_override(cx: &mut TestAppContext) {
    let repo = demo_repo("project-settings-sheet");
    std::fs::write(repo.join("package.json"), "{ \"name\": \"demo\" }\n").unwrap();
    write(&repo, r#"{ "tab_size": 2, "team": "kept" }"#);
    let file = crate::tests::scratch_dir("project-settings-sheet-store").join("settings.json");
    let (model, ws, cx) = open_stored(cx, Settings { tab_size: 4, ..with_repo(&repo) }, Store::at(file.clone()));
    cx.dispatch_action(crate::menus::OpenSettings);
    cx.run_until_parked();
    click(cx, "settings-page-editor");
    // Settings: the user's value, said to be set by the project for itself; no project scope.
    let sheet = cx.update(|_, cx| ws.read(cx).settings_sheet.clone()).expect("the sheet is up");
    cx.update(|_, cx| assert_eq!(sheet.read(cx).shown().tab_size, 4));
    assert!(cx.debug_bounds("settings-overridden-tab_size").is_some());
    assert!(cx.debug_bounds("settings-overridden-soft_wrap").is_none());
    assert!(cx.debug_bounds("settings-scope-project").is_none(), "Settings has no project scope");
    click(cx, "settings-cancel");
    cx.run_until_parked();

    // Project Settings: the project's value, set there; the others are the user's.
    cx.dispatch_action(crate::menus::ProjectStructure);
    cx.run_until_parked();
    let window = cx.update(|_, cx| ws.read(cx).project_structure.clone()).expect("Project Settings is up");
    click(cx, "structure-page-editor");
    let sheet = cx.update(|_, cx| window.read(cx).settings.clone());
    cx.update(|_, cx| assert_eq!((sheet.read(cx).scope, sheet.read(cx).shown().tab_size), (Scope::Project, 2)));
    assert!(cx.debug_bounds("settings-project-set-tab_size").is_some());
    assert!(cx.debug_bounds("settings-inherited-soft_wrap").is_some());
    // Soft wrap is set for the project; its tab size is reset to the user's.
    click(cx, "settings-soft-wrap");
    click(cx, "settings-reset-tab_size");
    cx.update(|_, cx| {
        let p = &sheet.read(cx).project;
        assert_eq!((p.soft_wrap, p.tab_size), (Some(true), None));
    });
    click(cx, "structure-ok");
    cx.run_until_parked();
    assert_eq!(tab_and_wrap(&model, cx), (4, true));

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ps::file_of(&repo)).unwrap()).unwrap();
    assert_eq!(written, serde_json::json!({ "soft_wrap": true, "team": "kept" }), "the file's own keys stay");
    // The user's settings did not take the project's.
    assert!(!Store::at(file.clone()).load().soft_wrap);
    // A command that changes a value the project sets changes it there.
    cx.dispatch_action(crate::menus::ToggleParamHints);
    cx.run_until_parked();
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert!(!m.settings.param_hints && m.project_settings.param_hints.is_none(), "not the project's: the user's");
        assert!(!m.user_settings().param_hints);
    });
    cx.update(|_, cx| {
        model.update(cx, |m, cx| {
            let values = ProjectSettings { soft_wrap: Some(true), param_hints: Some(true), ..Default::default() };
            m.save_project_settings(values, cx)
        })
    });
    cx.run_until_parked();
    cx.dispatch_action(crate::menus::ToggleParamHints);
    cx.run_until_parked();
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.project_settings.param_hints, Some(false), "the project's file takes it");
        assert!(!m.user_settings().param_hints, "the user's stays as it was");
        assert!(Key::ParamHints.is_set(&m.project_settings));
    });
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

/// Project Settings is there for a project, not for a plain folder: its command is off (and so
/// its menu item), and the project's own row offers it first in its right-click menu.
#[gpui::test]
fn project_settings_are_offered_for_a_project_and_not_for_a_plain_folder(cx: &mut TestAppContext) {
    let folder = demo_repo("project-settings-plain");
    let (_, ws, cx) =
        open_stored(cx, with_repo(&folder), Store::at(crate::tests::scratch_dir("plain-store").join("s.json")));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(crate::panels::Activity::Project, w, cx)));
    cx.run_until_parked();
    let available =
        |cx: &mut VisualTestContext| cx.update(|w, cx| w.is_action_available(&crate::menus::ProjectStructure, cx));
    assert!(!available(cx), "a plain folder has no Project Settings");
    cx.dispatch_action(crate::menus::ProjectStructure);
    cx.run_until_parked();
    assert!(cx.update(|_, cx| ws.read(cx).project_structure.is_none()));

    // The folder becomes a project (a package.json): Project Settings is there, and first in
    // the project row's menu.
    std::fs::write(folder.join("package.json"), "{ \"name\": \"demo\" }\n").unwrap();
    cx.update(|_, cx| ws.update(cx, |ws, cx| ws.forget_project_kind(cx)));
    cx.run_until_parked();
    assert!(available(cx), "a project has Project Settings");
    let row = cx.debug_bounds("file-root").expect("the project's folder is drawn").center();
    cx.simulate_mouse_down(row, gpui::MouseButton::Right, gpui::Modifiers::none());
    cx.simulate_mouse_up(row, gpui::MouseButton::Right, gpui::Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| ws.read(cx).project_structure.is_some()), "the row's menu opens Project Settings");
    let _ = std::fs::remove_dir_all(folder);
}

/// The Download JDK dialog offers each version the catalogue has, the newest long-term
/// support one first, and each vendor of the version picked.
#[gpui::test]
fn the_download_jdk_dialog_picks_a_version_and_a_vendor(cx: &mut TestAppContext) {
    use crate::lang::java::JdkPackage;
    let repo = demo_repo("project-settings-jdk-picker");
    std::fs::write(repo.join("package.json"), "{ \"name\": \"demo\" }\n").unwrap();
    let (_, ws, cx) =
        open_stored(cx, with_repo(&repo), Store::at(crate::tests::scratch_dir("picker-store").join("s.json")));
    cx.run_until_parked();
    let pkg = |vendor: &str, major: u32, lts: bool| JdkPackage {
        vendor: vendor.into(),
        major,
        version: format!("{major}.0.1+1"),
        id: format!("{vendor}-{major}"),
        file: format!("{vendor}-{major}.tar.gz"),
        lts,
    };
    cx.dispatch_action(crate::menus::ProjectStructure);
    cx.run_until_parked();
    let view = cx.update(|_, cx| ws.read(cx).project_structure.clone()).expect("Project Settings is up");
    cx.update(|_, cx| {
        view.update(cx, |v, cx| {
            v.page = crate::structure::Page::Sdks;
            v.jdk_catalog = Some(Ok(vec![
                pkg("temurin", 25, true),
                pkg("temurin", 24, false),
                pkg("corretto", 21, true),
                pkg("zulu", 21, true),
                pkg("zulu", 11, true),
            ]));
            v.picker = Some(Default::default());
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("structure-jdk-picker").is_some(), "the dialog shows");
    let picked = |cx: &mut VisualTestContext| cx.update(|_, cx| view.read(cx).picked().map(|p| p.id));
    assert_eq!(picked(cx).as_deref(), Some("temurin-25"), "the newest long-term support version, its first vendor");
    cx.update(|_, cx| view.update(cx, |v, _| v.picker.as_mut().unwrap().major = Some(11)));
    assert_eq!(picked(cx).as_deref(), Some("zulu-11"), "the only vendor of 11");
    cx.update(|_, cx| {
        view.update(cx, |v, _| {
            let picker = v.picker.as_mut().unwrap();
            picker.major = Some(21);
            picker.vendor = Some("zulu".into());
        })
    });
    assert_eq!(picked(cx).as_deref(), Some("zulu-21"));
    let _ = std::fs::remove_dir_all(repo);
}
