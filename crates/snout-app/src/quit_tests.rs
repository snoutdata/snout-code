//! Headless tests of quitting as the user does it: ⌘Q and the window's close button, through
//! the Quit SnoutCode? dialog and its buttons.

use std::path::{Path, PathBuf};

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};

use crate::code_tests::editor_for;
use crate::model::{Action, Dialog, Model};
use crate::settings::{Settings, Store};
use crate::tests::{act, demo_repo, with_repo};
use crate::workspace::Workspace;

/// The window over `settings`, as the app opens it, saving its settings to `store`.
pub fn open_stored(
    cx: &mut TestAppContext,
    settings: Settings,
    store: Store,
) -> (Entity<Model>, Entity<Workspace>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(&settings, cx));
    let model = cx.update(|cx| cx.new(|cx| Model::new(settings, Some(store), cx)));
    let mut ws = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
        ws = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    (model, ws.unwrap(), vcx)
}

pub fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
    cx.simulate_click(at, Modifiers::none());
}

fn quit_dialog(model: &Entity<Model>, cx: &mut VisualTestContext) -> Option<crate::model::QuitInfo> {
    cx.update(|_, cx| match &model.read(cx).dialog {
        Some(Dialog::Quit(info)) => Some(info.clone()),
        _ => None,
    })
}

fn hidden(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| ws.read(cx).hidden)
}

/// A repository with a Java file open in a tab, changed and not saved.
fn changed_tab(name: &str, model: &Entity<Model>, ws: &Entity<Workspace>, repo: &Path, cx: &mut VisualTestContext) {
    std::fs::write(repo.join(name), "class Q {}\n").unwrap();
    act(model, cx, Action::OpenFile(repo.join(name), Some((0, 0))));
    let editor = editor_for(ws, &repo.join(name), cx);
    cx.update(|w, cx| editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.insert("x", w, cx))));
    cx.run_until_parked();
}

fn settings_file(name: &str) -> PathBuf {
    crate::tests::scratch_dir(name).join("settings.json")
}

#[gpui::test]
fn command_q_asks_then_the_window_goes_at_once_and_dont_ask_again_is_kept(cx: &mut TestAppContext) {
    let repo = demo_repo("quit-cmd-q");
    let file = settings_file("quit-cmd-q-store");
    let (model, ws, cx) = open_stored(cx, with_repo(&repo), Store::at(file.clone()));
    cx.simulate_keystrokes("cmd-q");
    cx.run_until_parked();
    assert_eq!(quit_dialog(&model, cx), Some(Default::default()), "nothing to lose: it asks, as Settings says");
    assert!(cx.debug_bounds("quit-discard").is_none() && cx.debug_bounds("quit-save-all").is_none());
    assert!(!hidden(&ws, cx));
    // Don't ask again, then Quit: the window goes in that click, before anything is stopped.
    click(cx, "quit-dont-ask");
    click(cx, "quit-ok");
    cx.update(|_, cx| assert!(ws.read(cx).quitting));
    assert!(hidden(&ws, cx), "the window went at once");
    cx.run_until_parked();
    // The answer is saved with the settings: the next launch quits without asking.
    let saved = Store::at(file.clone()).load();
    assert!(!saved.confirm_quit, "Don't ask again is kept in {}", file.display());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

#[gpui::test]
fn with_the_question_off_the_close_button_quits_without_a_dialog(cx: &mut TestAppContext) {
    let repo = demo_repo("quit-close-off");
    let file = settings_file("quit-close-off-store");
    let settings = Settings { confirm_quit: false, ..with_repo(&repo) };
    let (model, ws, cx) = open_stored(cx, settings, Store::at(file.clone()));
    assert!(!cx.simulate_close(), "the window stays for the quit to hide it");
    assert!(hidden(&ws, cx), "the window went at once");
    assert_eq!(quit_dialog(&model, cx), None, "nothing asked");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

#[gpui::test]
fn the_close_button_names_unsaved_files_and_offers_save_discard_and_cancel(cx: &mut TestAppContext) {
    let repo = demo_repo("quit-close-unsaved");
    let file = settings_file("quit-close-unsaved-store");
    let settings = Settings { confirm_quit: false, ..with_repo(&repo) };
    let (model, ws, cx) = open_stored(cx, settings, Store::at(file.clone()));
    changed_tab("Q.java", &model, &ws, &repo, cx);
    // The close button: asked even with the question off, as a file would be lost.
    assert!(!cx.simulate_close(), "the window stays while it asks");
    cx.run_until_parked();
    let info = quit_dialog(&model, cx).expect("Quit SnoutCode? shows");
    assert_eq!(info.unsaved, ["Q.java"]);
    for button in ["quit-save-all", "quit-discard", "dialog-cancel"] {
        assert!(cx.debug_bounds(button).is_some(), "{button} is offered");
    }
    assert!(cx.debug_bounds("quit-dont-ask").is_none(), "not when something would be lost");
    // Cancel: the window and the change stay.
    click(cx, "dialog-cancel");
    cx.run_until_parked();
    assert_eq!(quit_dialog(&model, cx), None);
    assert!(!hidden(&ws, cx));
    cx.update(|_, cx| assert!(!ws.read(cx).quitting, "Cancel stays"));
    // Again, and Discard and Quit: the window goes, the file is as it was.
    assert!(!cx.simulate_close());
    cx.run_until_parked();
    click(cx, "quit-discard");
    assert!(hidden(&ws, cx), "the window went at once");
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(repo.join("Q.java")).unwrap(), "class Q {}\n", "discarded");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

#[gpui::test]
fn command_q_with_unsaved_files_saves_them_all_then_goes(cx: &mut TestAppContext) {
    let repo = demo_repo("quit-save-all");
    let file = settings_file("quit-save-all-store");
    let (model, ws, cx) = open_stored(cx, with_repo(&repo), Store::at(file.clone()));
    changed_tab("Q.java", &model, &ws, &repo, cx);
    cx.simulate_keystrokes("cmd-q");
    cx.run_until_parked();
    assert_eq!(quit_dialog(&model, cx).map(|i| i.unsaved), Some(vec!["Q.java".to_string()]));
    click(cx, "quit-save-all");
    for _ in 0..20 {
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        cx.run_until_parked();
    }
    assert_eq!(std::fs::read_to_string(repo.join("Q.java")).unwrap(), "xclass Q {}\n", "saved");
    assert!(hidden(&ws, cx), "then the window went");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}
