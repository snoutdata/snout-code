//! Headless tests of settings as text: Open Settings (JSON) for the user and the project, the
//! editor's completion and problems from the schema, and a file saved from the editor taken
//! at once, or, broken, leaving the settings in use as they were.

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::code_tests::editor_for;
use crate::editor::EditorPanel;
use crate::model::Model;
use crate::quit_tests::open_stored;
use crate::settings::{Settings, Store};
use crate::settings_json::Which;
use crate::tests::{demo_repo, scratch_dir, with_repo};

/// Put `text` in the tab and save it, with a stamp of its own.
fn save_text(editor: &Entity<EditorPanel>, text: &str, cx: &mut VisualTestContext) {
    // Put in place as typing does (a programmatic set says nothing of the change).
    cx.update(|w, cx| {
        editor.update(cx, |e, cx| {
            e.state.update(cx, |s, cx| s.set_value(text.to_owned(), w, cx));
            e.check_settings_text(cx);
        })
    });
    let save = cx.update(|_, cx| editor.update(cx, |e, cx| e.save(cx)));
    cx.run_until_parked();
    drop(save);
    let path = cx.update(|_, cx| editor.read(cx).path.clone());
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let later =
        std::time::SystemTime::now() + Duration::from_secs(N.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(later).unwrap();
}

fn problems(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> Vec<(String, bool)> {
    cx.update(|_, cx| {
        editor
            .read(cx)
            .problems
            .iter()
            .map(|d| (d.message.clone(), d.severity == Some(lsp_types::DiagnosticSeverity::ERROR)))
            .collect()
    })
}

fn completion_labels(editor: &Entity<EditorPanel>, offset: usize, cx: &mut VisualTestContext) -> Vec<String> {
    let task = cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        let provider = state.update(cx, |s, _| s.lsp_mut().completion_provider.clone()).expect("a completion provider");
        let rope = state.read(cx).text().clone();
        let ctx = lsp_types::CompletionContext {
            trigger_kind: lsp_types::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        };
        provider.completions(&rope, offset, ctx, w, cx)
    });
    let response = futures::executor::block_on(task).unwrap();
    match response {
        lsp_types::CompletionResponse::List(l) => l.items.into_iter().map(|i| i.label).collect(),
        lsp_types::CompletionResponse::Array(a) => a.into_iter().map(|i| i.label).collect(),
    }
}

fn tab_size(model: &Entity<Model>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| model.read(cx).settings.tab_size)
}

#[gpui::test]
fn the_projects_settings_open_as_json_with_completion_and_problems(cx: &mut TestAppContext) {
    let repo = demo_repo("settings-json-project");
    let file = scratch_dir("settings-json-project-store").join("settings.json");
    let (model, ws, cx) = open_stored(cx, with_repo(&repo), Store::at(file.clone()));
    cx.dispatch_action(crate::menus::OpenProjectSettingsJson);
    cx.run_until_parked();
    let path = repo.join(crate::project_settings::FILE);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\n}\n", "made, with nothing in it yet");
    let editor = editor_for(&ws, &path, cx);
    cx.update(|_, cx| assert_eq!(editor.read(cx).settings_file, Some(Which::Project)));
    // The keys a project may set are offered.
    let text = "{\n  \"tab_size\": 2,\n  \"so\n}\n";
    cx.update(|w, cx| editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.set_value(text, w, cx))));
    let labels = completion_labels(&editor, text.find("\"so").unwrap() + 3, cx);
    assert!(labels.contains(&"soft_wrap".to_owned()) && labels.contains(&"jdk".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"tab_size".to_owned()) && !labels.contains(&"theme".to_owned()), "{labels:?}");
    // Its problems: a wrong type and an unknown key.
    save_text(&editor, "{ \"tab_size\": \"two\", \"tabsize\": 2 }\n", cx);
    let found = problems(&editor, cx);
    assert!(found.iter().any(|(m, e)| *e && m.contains("tab_size") && m.contains("from 1 to 8")), "{found:?}");
    assert!(found.iter().any(|(m, e)| !e && m.contains("Unknown setting `tabsize`")), "{found:?}");
    // Saved valid, it is taken; saved broken, the values in use stay.
    save_text(&editor, "{ \"tab_size\": 2 }\n", cx);
    assert!(problems(&editor, cx).is_empty());
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.check_project_settings(true, w, cx)));
    cx.run_until_parked();
    assert_eq!(tab_size(&model, cx), 2);
    save_text(&editor, "{ \"tab_size\": 3,\n", cx);
    assert!(problems(&editor, cx).iter().any(|(_, e)| *e), "the broken JSON is marked");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.check_project_settings(true, w, cx)));
    cx.run_until_parked();
    assert_eq!(tab_size(&model, cx), 2, "the last good value stays");
    cx.update(|_, cx| assert!(model.read(cx).project_settings_error.is_some()));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

#[gpui::test]
fn the_users_settings_open_as_json_and_a_saved_edit_is_taken_or_kept_out(cx: &mut TestAppContext) {
    let repo = demo_repo("settings-json-user");
    let file = scratch_dir("settings-json-user-store").join("settings.json");
    let (model, ws, cx) = open_stored(cx, Settings { tab_size: 4, ..with_repo(&repo) }, Store::at(file.clone()));
    cx.dispatch_action(crate::menus::OpenSettingsJson);
    cx.run_until_parked();
    assert!(file.exists(), "written for the tab to show");
    let editor = editor_for(&ws, &file, cx);
    cx.update(|_, cx| assert_eq!(editor.read(cx).settings_file, Some(Which::User)));
    let text = cx.update(|_, cx| editor.read(cx).text(cx));
    assert!(problems(&editor, cx).is_empty(), "what the app writes is all known: {:?}", problems(&editor, cx));
    // The theme's values are offered after its key.
    let at = text.find("\"theme\": ").unwrap() + "\"theme\": ".len();
    let labels = completion_labels(&editor, at, cx);
    assert!(labels.contains(&"\"Dark\"".to_owned()), "{labels:?}");
    // An edit saved from the tab is taken at once.
    let edited = text.replace("\"tab_size\": 4", "\"tab_size\": 6");
    save_text(&editor, &edited, cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.check_user_settings(w, cx)));
    assert_eq!(tab_size(&model, cx), 6);
    // Broken: the settings in use stay, the error shows, and the file is not written over.
    let broken = edited.replace("\"tab_size\": 6,", "\"tab_size\": 7");
    save_text(&editor, &broken, cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.check_user_settings(w, cx)));
    assert_eq!(tab_size(&model, cx), 6, "the settings in use stay");
    cx.update(|_, cx| {
        model.update(cx, |m, _| {
            assert!(m.user_settings_error.is_some());
            m.settings.soft_wrap = !m.settings.soft_wrap;
            m.persist();
        })
    });
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken, "the hand edit is not written over");
    // On the way out it is kept aside and the settings in use are written.
    cx.update(|_, cx| model.update(cx, |m, _| m.persist_at_quit()));
    assert_eq!(std::fs::read_to_string(file.with_extension("json.unreadable")).unwrap(), broken);
    assert_eq!(Store::at(file.clone()).load().tab_size, 6);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}
