//! Headless tests of the project's own settings: `.snout/settings.json` over the user's, read
//! again as it changes on disk, its errors keeping the last good values, and the Settings
//! sheet's User and Project scopes.

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

#[gpui::test]
fn the_settings_sheet_shows_and_edits_the_projects_scope_and_resets_an_override(cx: &mut TestAppContext) {
    let repo = demo_repo("project-settings-sheet");
    write(&repo, r#"{ "tab_size": 2, "team": "kept" }"#);
    let file = crate::tests::scratch_dir("project-settings-sheet-store").join("settings.json");
    let (model, ws, cx) = open_stored(cx, Settings { tab_size: 4, ..with_repo(&repo) }, Store::at(file.clone()));
    cx.dispatch_action(crate::menus::OpenSettings);
    cx.run_until_parked();
    click(cx, "settings-page-editor");
    // The user's scope: the user's value, said to be overridden by the project.
    let sheet = cx.update(|_, cx| ws.read(cx).settings_sheet.clone()).expect("the sheet is up");
    cx.update(|_, cx| assert_eq!(sheet.read(cx).shown().tab_size, 4));
    assert!(cx.debug_bounds("settings-overridden-tab_size").is_some());
    assert!(cx.debug_bounds("settings-overridden-soft_wrap").is_none());
    // The project's: its value, set there; the others are the user's.
    click(cx, "settings-scope-project");
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
    // A page a project cannot set says so.
    click(cx, "settings-page-appearance");
    assert!(cx.debug_bounds("settings-show-user").is_some());
    click(cx, "settings-ok");
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
