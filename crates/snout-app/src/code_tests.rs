//! Headless tests of the Code view: the file tree's marks, the editor tabs, the status
//! bar, and the editing, Git and navigation features of the editor.

use std::path::Path;

use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};

use crate::editor::EditorPanel;
use crate::model::Action;
use crate::panels::Activity;
use crate::tests::{act, demo_repo, open, with_repo};
use crate::workspace::Workspace;

pub fn editor_for(ws: &Entity<Workspace>, path: &Path, cx: &mut VisualTestContext) -> Entity<EditorPanel> {
    cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path))
        .expect("a tab for the file")
}

fn tab_names(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| ws.read(cx).editors(cx).iter().map(|e| e.read(cx).name().to_string()).collect())
}

#[test]
fn the_tree_marks_files_and_folders_by_their_git_state() {
    use crate::files::{GitMark, git_marks};
    use snout_core::{Change, FileEntry};
    let root = Path::new("/r");
    let entry = |path: &str, index: Option<Change>, worktree: Option<Change>| FileEntry {
        path: path.into(),
        orig_path: None,
        index,
        worktree,
        conflicted: false,
    };
    let files = [
        entry("src/a/Mod.java", None, Some(Change::Modified)),
        entry("src/a/New.java", Some(Change::Added), None),
        entry("notes.txt", None, Some(Change::Untracked)),
        entry("gone.txt", None, Some(Change::Deleted)),
    ];
    let marks = git_marks(root, &files);
    assert_eq!(marks[&root.join("src/a/Mod.java")], GitMark::Modified);
    assert_eq!(marks[&root.join("src/a/New.java")], GitMark::Added);
    assert_eq!(marks[&root.join("notes.txt")], GitMark::Untracked);
    assert_eq!(marks[&root.join("src")], GitMark::Modified, "a folder takes the strongest state below it");
    assert!(!marks.contains_key(&root.join("gone.txt")), "deleted files are not in the tree");
}

#[gpui::test]
fn a_single_click_previews_and_editing_or_a_double_click_keeps_the_tab(cx: &mut TestAppContext) {
    let repo = demo_repo("preview");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    act(&model, cx, Action::PreviewFile(repo.join("README.md")));
    act(&model, cx, Action::PreviewFile(repo.join("status.rs")));
    assert_eq!(tab_names(&ws, cx), ["status.rs"], "the second preview takes the first one's tab");
    let preview = editor_for(&ws, &repo.join("status.rs"), cx);
    cx.update(|_, cx| assert!(preview.read(cx).preview));
    // Typing keeps it; the next preview gets a tab of its own.
    cx.simulate_keystrokes("x");
    cx.update(|_, cx| assert!(!preview.read(cx).preview && preview.read(cx).dirty));
    act(&model, cx, Action::PreviewFile(repo.join("README.md")));
    assert_eq!(tab_names(&ws, cx), ["status.rs", "README.md"]);
    // A double click on a preview tab keeps it.
    cx.run_until_parked();
    // The Welcome page went with the first file: the README's tab is the second.
    let tab = cx.debug_bounds("editor-tab-1").expect("the README tab is drawn").center();
    cx.simulate_click(tab, Modifiers::none());
    let readme = editor_for(&ws, &repo.join("README.md"), cx);
    cx.update(|_, cx| assert!(readme.read(cx).preview, "one click only selects it"));
    cx.simulate_event(gpui::MouseDownEvent {
        position: tab,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: tab,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    cx.update(|_, cx| assert!(!readme.read(cx).preview, "a double click keeps it"));
    // Opening from go to file (not a preview) keeps a previewed file's tab too.
    act(&model, cx, Action::PreviewFile(repo.join("src/main.rs")));
    act(&model, cx, Action::OpenFile(repo.join("src/main.rs"), None));
    cx.update(|_, cx| assert!(!editor_for_app(&ws, &repo.join("src/main.rs"), cx).read(cx).preview));
    let _ = std::fs::remove_dir_all(repo);
}

fn model_recent(ws: &Entity<Workspace>, cx: &gpui::App) -> Vec<std::path::PathBuf> {
    ws.read(cx).model.read(cx).recent_projects()
}

fn editor_for_app(ws: &Entity<Workspace>, path: &Path, cx: &gpui::App) -> Entity<EditorPanel> {
    ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path).unwrap()
}

#[gpui::test]
fn a_middle_click_closes_a_tab(cx: &mut TestAppContext) {
    let repo = demo_repo("middle");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    act(&model, cx, Action::OpenFile(repo.join("README.md"), None));
    act(&model, cx, Action::OpenFile(repo.join("status.rs"), None));
    cx.run_until_parked();
    let tab = cx.debug_bounds("editor-tab-0").expect("the README tab is drawn").center();
    cx.simulate_mouse_down(tab, MouseButton::Middle, Modifiers::none());
    cx.simulate_mouse_up(tab, MouseButton::Middle, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(tab_names(&ws, cx), ["status.rs"]);
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn a_right_click_in_the_editor_opens_the_apps_menu_and_its_items_reach_the_editor(cx: &mut TestAppContext) {
    let (repo, _, _, editor, cx) = open_text("menu.txt", "alpha beta\n", (0, 0), cx);
    cx.run_until_parked();
    let body = cx.debug_bounds("editor-body").expect("the editor is drawn");
    let at = body.origin + gpui::point(gpui::px(120.), gpui::px(10.));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.update(|_, cx| assert!(editor.read(cx).context.is_open(), "our menu opens, not the system's"));
    // The keyboard is in it: Esc closes it.
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!editor.read(cx).context.is_open(), "Esc closes it"));
    // Select All, picked from it, selects the editor's text.
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes("up enter");
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(!editor.read(cx).context.is_open(), "picking an item closes it");
        let state = editor.read(cx).state.read(cx);
        assert_eq!(state.selected_range(), 0..state.text().len(), "Select All reached the editor");
    });
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_status_bar_follows_the_editor_in_front(cx: &mut TestAppContext) {
    let repo = demo_repo("status-bar");
    std::fs::write(repo.join("App.java"), "class App {\r\n\tint x;\r\n}\r\n").unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    act(&model, cx, Action::OpenFile(repo.join("App.java"), Some((1, 1))));
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).editor_status(cx)).expect("an editor is in front");
    assert_eq!(status.position, "2:2");
    assert_eq!((status.endings, status.language, status.encoding), ("CRLF", "Java", "UTF-8"));
    assert!(status.indent.starts_with("Tab"), "{}", status.indent);
    cx.simulate_keystrokes("shift-right");
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).editor_status(cx)).unwrap();
    assert_eq!(status.selection.as_deref(), Some("1 selected"));
    // The tree shows the file selected.
    let files = cx.update(|_, cx| ws.read(cx).side.read(cx).files.clone());
    cx.update(|_, cx| assert_eq!(files.read(cx).active.as_deref(), Some(repo.join("App.java").as_path())));
    act(&model, cx, Action::OpenFile(repo.join("status.rs"), None));
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).editor_status(cx)).unwrap();
    assert_eq!((status.endings, status.language, status.indent.as_str()), ("LF", "Rust", "4 spaces"));
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn indentation_and_line_endings_are_read_from_the_file() {
    use crate::editor::FileInfo;
    let two = FileInfo::detect("a:\n  b:\n    c: 1\n  d: 2\n");
    assert_eq!((two.tabs, two.width, two.crlf), (false, 2, false));
    let four = FileInfo::detect("class A {\r\n    void f() {\r\n        g();\r\n    }\r\n}\r\n");
    assert_eq!((four.tabs, four.width, four.crlf), (false, 4, true));
    assert!(FileInfo::detect("x\n\ty\n\t\tz\n").tabs);
    assert_eq!(FileInfo::detect("").width, 4);
}

/// Open `name` (holding `text`) in an editor tab with the caret at `at`.
#[allow(clippy::type_complexity)]
fn open_text<'a>(
    name: &str,
    text: &str,
    at: (u32, u32),
    cx: &'a mut TestAppContext,
) -> (std::path::PathBuf, Entity<crate::model::Model>, Entity<Workspace>, Entity<EditorPanel>, &'a mut VisualTestContext)
{
    let repo = demo_repo(&format!("edit-{}", name.replace('.', "-")));
    std::fs::write(repo.join(name), text).unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    act(&model, cx, Action::OpenFile(repo.join(name), Some(at)));
    let editor = editor_for(&ws, &repo.join(name), cx);
    (repo, model, ws, editor, cx)
}

fn text_of(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).text(cx))
}

#[gpui::test]
fn the_caret_line_is_lit_and_brackets_match(cx: &mut TestAppContext) {
    let text = "class A {\n    void f() { g(1); }\n}\n";
    let (repo, _, _, editor, cx) = open_text("A.java", text, (0, 9), cx);
    cx.update(|_, cx| {
        let theme = gpui_component::Theme::global(cx);
        assert!(theme.highlight_theme.style.editor_active_line.is_some(), "the caret's line has a band");
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(editor.read(cx).bracket_pair, Some((8, text.len() - 2))));
    // Moving onto other brackets frames those instead.
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(1, 17, w, cx)));
    cx.run_until_parked();
    let open = text.find("g(").unwrap() + 1;
    cx.update(|_, cx| assert_eq!(editor.read(cx).bracket_pair, Some((open, open + 2))));
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(1, 5, w, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(editor.read(cx).bracket_pair, None));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn find_replace_and_go_to_line_have_their_keys(cx: &mut TestAppContext) {
    let text = "one\ntwo\nthree\nfour\n";
    let (repo, _, ws, editor, cx) = open_text("notes.txt", text, (0, 0), cx);
    let state = cx.update(|_, cx| editor.read(cx).state.clone());
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-f"));
    cx.update(|_, cx| assert!(state.read(cx).search_session().open, "⌘F finds in the file"));
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-alt-f"));
    cx.update(|_, cx| {
        let s = state.read(cx).search_session();
        assert!(s.open && s.replace_mode, "⌥⌘F replaces");
    });
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-shift-f"));
    cx.update(|_, cx| {
        let find = ws.read(cx).find_in_files.clone().expect("⇧⌘F in the editor is still find in files");
        assert!(!find.read(cx).replace_mode);
    });
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-l"));
    cx.update(|_, cx| assert_eq!(ws.read(cx).search.as_ref().unwrap().read(cx).mode, crate::search::Mode::GoToLine));
    cx.simulate_input("3:2");
    cx.simulate_keystrokes("enter");
    cx.update(|_, cx| {
        assert!(ws.read(cx).search.is_none());
        let pos = state.read(cx).cursor_position();
        assert_eq!((pos.line, pos.character), (2, 1), "⌘L went to line 3, column 2");
    });
    assert_eq!(crate::search::parse_line("12"), Some((11, 0)));
    assert_eq!(crate::search::parse_line(" 4:7 "), Some((3, 6)));
    assert_eq!(crate::search::parse_line("x"), None);
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn line_commands_comment_duplicate_move_and_undo(cx: &mut TestAppContext) {
    let text = "class A {\n    int x;\n    int y;\n}\n";
    let (repo, _, _, editor, cx) = open_text("B.java", text, (1, 6), cx);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-/"));
    assert_eq!(text_of(&editor, cx), "class A {\n    // int x;\n    int y;\n}\n");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-/"));
    assert_eq!(text_of(&editor, cx), text, "⌘/ again uncomments");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-d"));
    assert_eq!(text_of(&editor, cx), "class A {\n    int x;\n    int x;\n    int y;\n}\n");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-z"));
    assert_eq!(text_of(&editor, cx), text, "one undo takes the duplicate back");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(2, 4, w, cx)));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("alt-up"));
    assert_eq!(text_of(&editor, cx), "class A {\n    int y;\n    int x;\n}\n");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("alt-down alt-down"));
    assert_eq!(text_of(&editor, cx), "class A {\n    int x;\n}\n    int y;\n");
    cx.update(|_, cx| assert_eq!(editor.read(cx).state.read(cx).cursor_position().line, 3, "the caret moves along"));
    // ⌥Z wraps long lines, and again does not.
    cx.update(|_, cx| assert!(!editor.read(cx).soft_wrap));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("alt-z"));
    cx.update(|_, cx| assert!(editor.read(cx).soft_wrap));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_font_zooms_for_every_editor_and_is_remembered(cx: &mut TestAppContext) {
    let (repo, model, _, _, cx) = open_text("Z.java", "class Z {}\n", (0, 0), cx);
    let size = |cx: &mut VisualTestContext| cx.update(|_, cx| crate::editor::font_size(cx));
    assert_eq!(size(cx), crate::look::MONO_SIZE);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-= cmd-="));
    assert_eq!(size(cx), crate::look::MONO_SIZE + 2.);
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.editor_font_size, crate::look::MONO_SIZE + 2.));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd--"));
    assert_eq!(size(cx), crate::look::MONO_SIZE + 1.);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-0"));
    assert_eq!(size(cx), crate::look::MONO_SIZE, "⌘0 resets it");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn closing_a_tab_or_the_window_with_changes_asks_first(cx: &mut TestAppContext) {
    let (repo, model, ws, editor, cx) = open_text("C.java", "class C {}\n", (0, 0), cx);
    cx.simulate_keystrokes("x");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-w"));
    assert!(cx.has_pending_prompt(), "⌘W on a changed tab asks");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(tab_names(&ws, cx), ["C.java"], "Cancel keeps the tab");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-w"));
    cx.simulate_prompt_answer("Save");
    cx.run_until_parked();
    assert!(tab_names(&ws, cx).is_empty(), "Save closes it");
    assert_eq!(std::fs::read_to_string(repo.join("C.java")).unwrap(), "xclass C {}\n");

    // A clean tab closes without asking; Don't Save drops the changes.
    act(&model, cx, Action::OpenFile(repo.join("README.md"), Some((0, 0))));
    cx.simulate_keystrokes("y");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-w"));
    cx.simulate_prompt_answer("Don't Save");
    cx.run_until_parked();
    assert!(tab_names(&ws, cx).is_empty());
    assert_eq!(std::fs::read_to_string(repo.join("README.md")).unwrap(), "# demo\n");

    // The window: nothing unsaved closes at once; otherwise it asks.
    act(&model, cx, Action::OpenFile(repo.join("README.md"), Some((0, 0))));
    assert!(cx.update(|w, cx| ws.update(cx, |ws, cx| ws.confirm_unsaved(false, w, cx))));
    cx.simulate_keystrokes("z");
    assert!(!cx.update(|w, cx| ws.update(cx, |ws, cx| ws.confirm_unsaved(false, w, cx))));
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Save");
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(repo.join("README.md")).unwrap(), "z# demo\n");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn a_file_changed_on_disk_reloads_or_warns(cx: &mut TestAppContext) {
    let (repo, _, _, editor, cx) = open_text("D.java", "class D {}\n", (0, 0), cx);
    let file = repo.join("D.java");
    let wait = |cx: &mut VisualTestContext| {
        for _ in 0..5 {
            cx.executor().advance_clock(crate::editor::DISK_POLL);
            cx.run_until_parked();
        }
    };
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&file, "class D { int a; }\n").unwrap();
    wait(cx);
    assert_eq!(text_of(&editor, cx), "class D { int a; }\n", "a clean tab takes the new text");
    cx.update(|_, cx| assert!(!editor.read(cx).dirty));

    cx.simulate_keystrokes("x");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&file, "class D { int b; }\n").unwrap();
    wait(cx);
    cx.update(|_, cx| assert!(editor.read(cx).conflict, "a changed tab warns instead"));
    assert!(text_of(&editor, cx).starts_with('x'), "and keeps its text");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.reload(w, cx)));
    // Read off the UI thread.
    cx.run_until_parked();
    assert_eq!(text_of(&editor, cx), "class D { int b; }\n");
    cx.update(|_, cx| assert!(!editor.read(cx).conflict && !editor.read(cx).dirty));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_gutter_marks_lines_changed_since_the_commit(cx: &mut TestAppContext) {
    use snout_core::gutter::LineChangeKind::*;
    let repo = demo_repo("gutter");
    let body: String = (1..=80).map(|i| format!("line {i}\n")).collect();
    std::fs::write(repo.join("long.txt"), &body).unwrap();
    crate::tests::git(&repo, &["add", "long.txt"]);
    crate::tests::git(&repo, &["commit", "-qm", "Add a long file"]);
    let edited = body.replace("line 3\n", "line three\n").replace("line 6\n", "").replace("line 8\n", "line 8\nnew\n");
    std::fs::write(repo.join("long.txt"), &edited).unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    act(&model, cx, Action::OpenFile(repo.join("long.txt"), Some((0, 0))));
    let editor = editor_for(&ws, &repo.join("long.txt"), cx);
    let settle = |cx: &mut VisualTestContext| {
        for _ in 0..5 {
            cx.executor().advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            cx.update(|w, cx| w.draw(cx).clear(cx));
        }
    };
    settle(cx);
    let changes =
        cx.update(|_, cx| editor.read(cx).changes.iter().map(|c| (c.kind, c.lines.clone())).collect::<Vec<_>>());
    assert_eq!(changes, [(Modified, 2..3), (Deleted, 5..5), (Added, 7..8)]);
    // Each mark is drawn beside its line, left of the code, from the editor's layout.
    let marks = cx.update(|_, cx| editor.read(cx).painted.borrow().clone());
    assert_eq!(marks.iter().map(|m| m.0).collect::<Vec<_>>(), [Modified, Deleted, Added]);
    let line_top = |line: usize, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let s = editor.read(cx).state.read(cx);
            let at = gpui_component::input::RopeExt::line_start_offset(s.text(), line);
            s.range_to_bounds(&(at..at)).unwrap()
        })
    };
    let third = line_top(2, cx);
    assert_eq!(marks[0].1.top(), third.top());
    assert!(marks[0].1.right() < third.left(), "the mark sits left of the code");
    // Typing a line marks it at once (after a short pause).
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes("x enter");
    settle(cx);
    let changes = cx.update(|_, cx| editor.read(cx).changes.iter().map(|c| c.kind).collect::<Vec<_>>());
    assert_eq!(changes, [Added, Modified, Deleted, Added]);
    // Scrolled down, the marks move with the lines.
    cx.update(|_, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| s.set_scroll_offset(gpui::point(gpui::px(0.), gpui::px(-60.)), cx));
    });
    settle(cx);
    let before = marks[0].1.top();
    let after = cx.update(|_, cx| editor.read(cx).painted.borrow().iter().find(|m| m.0 == Modified).map(|m| m.1.top()));
    assert!(after.is_some_and(|y| y < before), "{after:?} should be above {before:?}");
    // Once the change is committed, the gutter is clean.
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-s"));
    crate::tests::git(&repo, &["commit", "-qam", "Edit the long file"]);
    act(&model, cx, Action::Refresh);
    settle(cx);
    cx.update(|_, cx| assert!(editor.read(cx).changes.is_empty()));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_command_palette_lists_every_command_with_its_keys_and_runs_one(cx: &mut TestAppContext) {
    let (repo, _, ws, editor, cx) = open_text("P.java", "class P {}\n", (0, 0), cx);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-shift-p"));
    let palette = cx.update(|_, cx| ws.read(cx).search.clone()).expect("⇧⌘P puts up the palette");
    cx.update(|_, cx| {
        let p = palette.read(cx);
        assert_eq!(p.mode, crate::search::Mode::Commands);
        let recent = model_recent(&ws, cx);
        assert_eq!(p.items.len(), crate::menus::commands(&recent).len(), "every command is listed");
        let keys = |name: &str| p.items.iter().find(|i| i.path == name).and_then(|i| i.keys.clone());
        let shown = |mac: &str| crate::keymap::display(&crate::keymap::platform_keys(mac), cx);
        assert_eq!(keys("Toggle Comment"), Some(shown("cmd-/")), "with the keys it has in the editor");
        assert_eq!(keys("Go to File…"), Some(shown("cmd-p")));
        assert!(keys("Pull").is_none(), "and none where it has none");
    });
    cx.simulate_input("duplicate");
    cx.update(|_, cx| assert_eq!(palette.read(cx).items[0].path, "Duplicate Line"));
    cx.simulate_keystrokes("enter");
    cx.update(|_, cx| assert!(ws.read(cx).search.is_none()));
    assert_eq!(text_of(&editor, cx), "class P {}\nclass P {}\n", "the command ran in the editor");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn split_right_shows_the_file_twice_and_keeps_both_in_step(cx: &mut TestAppContext) {
    let (repo, _, ws, editor, cx) = open_text("S.java", "class S {}\n", (0, 0), cx);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-\\"));
    let editors = cx.update(|_, cx| ws.read(cx).editors(cx));
    assert_eq!(editors.len(), 2);
    let twin = editors.into_iter().find(|e| e.entity_id() != editor.entity_id()).unwrap();
    cx.update(|_, cx| {
        let dock = ws.read(cx).dock.read(cx);
        let tree = dock.layout(gpui_component::dock::DockPlacement::Center).unwrap();
        let a = tree.find_panel_node(crate::tabs::panel_id(&editor));
        let b = tree.find_panel_node(crate::tabs::panel_id(&twin));
        assert!(a.is_some() && b.is_some() && a != b, "the twin has a tab group of its own");
    });
    // The split has the keyboard; what is typed there shows in the first view too.
    cx.simulate_keystrokes("x");
    assert_eq!(text_of(&twin, cx), "xclass S {}\n");
    assert_eq!(text_of(&editor, cx), "xclass S {}\n");
    cx.update(|_, cx| assert!(editor.read(cx).dirty));
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-s"));
    cx.update(|_, cx| assert!(!editor.read(cx).dirty && !twin.read(cx).dirty, "saving one saves both"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_window_opens_on_the_activity_it_showed_last(cx: &mut TestAppContext) {
    let repo = demo_repo("activity");
    let (model, ws, vcx) = open(cx, with_repo(&repo));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let saved = vcx.update(|_, cx| model.read(cx).settings.clone());
    assert_eq!(saved.activity, Some(Activity::Project));
    let model2 = cx.update(|cx| gpui::AppContext::new(cx, |cx| crate::model::Model::new(saved.clone(), None, cx)));
    let mut ws2 = None;
    let (_, vcx2) = cx.add_window_view(|window, cx| {
        let view = gpui::AppContext::new(cx, |cx| Workspace::new(model2.clone(), window, cx));
        ws2 = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx2.run_until_parked();
    vcx2.update(|_, cx| assert_eq!(ws2.unwrap().read(cx).side.read(cx).activity, Activity::Project));
    let _ = std::fs::remove_dir_all(repo);
}

/// A Java file of about 5,000 lines.
fn big_java() -> String {
    let mut s = String::from("package demo;\n\nimport java.util.*;\n\npublic class Big {\n");
    let mut i = 0;
    while s.lines().count() < 5000 {
        s.push_str(&format!(
            "    /** Method {i}. */\n    public int method{i}(List<String> names, int x) {{\n        int total = x * {i};\n        for (String n : names) {{\n            if (n.length() > {m}) {{\n                total += n.hashCode() % 7;\n            }}\n        }}\n        return total + \"{i}\".length();\n    }}\n\n",
            m = i % 9
        ));
        i += 1;
    }
    s.push_str("}\n");
    s
}

#[gpui::test]
fn typing_and_scrolling_a_large_java_file_stay_quick(cx: &mut TestAppContext) {
    use std::time::Instant;
    let text = big_java();
    // Committed, so the gutter compares with it as the text changes.
    let repo = demo_repo("big-java");
    std::fs::write(repo.join("Big.java"), &text).unwrap();
    crate::tests::git(&repo, &["add", "Big.java"]);
    crate::tests::git(&repo, &["commit", "-qm", "A big file"]);
    let (model, ws, cx) = open(cx, with_repo(&repo));
    act(&model, cx, Action::OpenFile(repo.join("Big.java"), Some((2500, 8))));
    let editor = editor_for(&ws, &repo.join("Big.java"), cx);
    cx.update(|w, cx| w.draw(cx).clear(cx));
    // Each step is timed against a frame with nothing new, drawn right next to it: the
    // machine's speed and load (a debug build, a small CI runner, other tests running) then
    // cancel out, and what is left is the work the step adds.
    let mut idle = std::time::Duration::ZERO;
    let mut idle_frame = |cx: &mut VisualTestContext| {
        let started = Instant::now();
        cx.update(|w, cx| {
            w.refresh();
            w.draw(cx).clear(cx)
        });
        idle += started.elapsed();
    };
    // Typing: each keystroke and the frame after it.
    let mut typing = std::time::Duration::ZERO;
    for _ in 0..40 {
        idle_frame(cx);
        let started = Instant::now();
        cx.simulate_keystrokes("a");
        cx.update(|w, cx| w.draw(cx).clear(cx));
        typing += started.elapsed();
    }
    let typing = typing / 40;
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(editor.read(cx).changes.len(), 1, "the gutter marks the typed line"));
    // Scrolling: a page at a time through the file, a frame each.
    let state = cx.update(|_, cx| editor.read(cx).state.clone());
    let mut scrolling = std::time::Duration::ZERO;
    for i in 0..40 {
        idle_frame(cx);
        let started = Instant::now();
        cx.update(|_, cx| {
            state.update(cx, |s, cx| s.set_scroll_offset(gpui::point(gpui::px(0.), gpui::px(-600. * i as f32)), cx))
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        scrolling += started.elapsed();
    }
    let scrolling = scrolling / 40;
    let idle = idle / 80;
    println!("Big.java: {idle:?} a frame, {typing:?} a keystroke, {scrolling:?} a scrolled frame");
    // Both take about 2.5 idle frames in a debug build, on a laptop and on a 2 vCPU runner
    // alike. The bound catches a regression that does whole-file work on every keystroke or
    // frame, not the last millisecond.
    let limit = idle * 6;
    assert!(typing < limit, "typing takes {typing:?} a keystroke, over 6 idle frames ({idle:?} each)");
    assert!(scrolling < limit, "scrolling takes {scrolling:?} a frame, over 6 idle frames ({idle:?} each)");
    assert_eq!(text_of(&editor, cx).len(), text.len() + 40);
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_project_view_shows_only_the_file_tree_under_the_project_name(cx: &mut TestAppContext) {
    let repo = demo_repo("code-side");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("file-README.md").is_some(), "the file tree shows");
    assert!(cx.debug_bounds("repo-list").is_none(), "the repository list does not");
    let side = cx.update(|_, cx| ws.read(cx).side.read(cx).activity);
    assert_eq!(side, Activity::Project);
    // The activity bar: the project first, then Git.
    let (project, git) = (cx.debug_bounds("act-project").unwrap(), cx.debug_bounds("act-git").unwrap());
    assert!(project.top() < git.top(), "the project's item is above Git's");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn file_menu_open_recent_lists_projects_newest_first_and_opens_one(cx: &mut TestAppContext) {
    use crate::menus::{ClearRecent, OpenRecent};
    let a = crate::tests::demo_repo("recent-a");
    let b = crate::tests::demo_repo("recent-b");
    let (model, ws, cx) = open(cx, with_repo(&a));
    act(&model, cx, Action::OpenRepo(b.clone()));
    let recent = cx.update(|_, cx| model.read(cx).recent_projects());
    assert_eq!(recent, [b.clone(), a.clone()], "most recent first");
    // The menu bar's File ▸ Open Recent, and the palette, list the same.
    let bar = cx.update(|_, cx| crate::menus::bar(&model.read(cx).settings, &model.read(cx).recent_projects(), &[]));
    let file = bar.iter().find(|m| m.name == "File").expect("a File menu");
    let names: Vec<String> = file.items.iter().filter_map(menu_item_name).collect();
    let first = ["New File…", "New Project…", "Open Project…", "Open Recent", "Close Project"].map(String::from);
    assert!(names.starts_with(&first), "{names:?}");
    let sub = file
        .items
        .iter()
        .find_map(|i| match i {
            gpui::MenuItem::Submenu(m) if m.name == "Open Recent" => Some(m),
            _ => None,
        })
        .expect("an Open Recent submenu");
    let sub_names: Vec<String> = sub.items.iter().filter_map(menu_item_name).collect();
    let (na, nb) = (crate::model::Model::dir_name(&a), crate::model::Model::dir_name(&b));
    assert_eq!(sub_names, [nb.clone(), na.clone(), "Clear Recent".into()]);
    let palette: Vec<String> = crate::menus::commands(&recent).into_iter().map(|(n, _)| n.to_string()).collect();
    for name in ["Open Project…", "Close Project", "Clear Recent Projects"] {
        assert!(palette.iter().any(|n| n == name), "the palette has {name}");
    }
    assert!(palette.contains(&format!("Open Recent: {na}")));
    // Choosing one opens it.
    cx.update(|w, cx| w.dispatch_action(Box::new(OpenRecent(a.clone())), cx));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(model.read(cx).repo.as_ref().map(|r| r.root().to_owned()), Some(a.clone())));
    // Clear Recent empties the list; the repositories stay in the Git view's list.
    cx.update(|w, cx| w.dispatch_action(Box::new(ClearRecent), cx));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(model.read(cx).recent_projects().is_empty());
        assert_eq!(model.read(cx).settings.repos.len(), 2);
    });
    let _ = ws;
    let _ = std::fs::remove_dir_all(a);
    let _ = std::fs::remove_dir_all(b);
}

fn menu_item_name(item: &gpui::MenuItem) -> Option<String> {
    match item {
        gpui::MenuItem::Action { name, .. } => Some(name.to_string()),
        gpui::MenuItem::Submenu(m) => Some(m.name.to_string()),
        _ => None,
    }
}

#[gpui::test]
fn close_project_closes_its_tabs_and_returns_to_the_welcome_page(cx: &mut TestAppContext) {
    use crate::menus::CloseProject;
    let repo = demo_repo("close-project");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    act(&model, cx, Action::OpenFile(repo.join("README.md"), None));
    assert_eq!(tab_names(&ws, cx), ["README.md"]);
    cx.update(|w, cx| w.dispatch_action(Box::new(CloseProject), cx));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(model.read(cx).repo.is_none(), "no project is open");
        assert_eq!(model.read(cx).title(), "SnoutCode");
        assert_eq!(model.read(cx).recent_projects(), std::slice::from_ref(&repo), "it stays recent");
    });
    assert!(tab_names(&ws, cx).is_empty(), "its tabs closed");
    let welcome = cx.update(|_, cx| {
        let ws = ws.read(cx);
        let id = ws.tabs.welcome;
        ws.dock.read(cx).panel(id).is_some()
    });
    assert!(welcome, "the Welcome page is there");
    assert!(cx.debug_bounds("welcome-recent-0").is_some(), "with the project in its recent list");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn without_the_java_engine_the_status_bar_says_so_quietly_and_explains(cx: &mut TestAppContext) {
    let repo = demo_repo("no-engine");
    std::fs::write(repo.join("build.gradle"), "plugins { id 'java' }\n").unwrap();
    let (_, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let status = cx.update(|_, cx| ws.read(cx).status.clone());
    cx.update(|_, cx| {
        status.update(cx, |s, cx| {
            s.java_missing = true;
            cx.notify();
        })
    });
    cx.run_until_parked();
    let item = cx.debug_bounds("status-java-missing").expect("the item shows for a Gradle project");
    assert!(cx.debug_bounds("java-missing-note").is_none());
    cx.simulate_click(item.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("java-missing-note").is_some(), "a click explains why and how to fix it");
    cx.update(|_, cx| status.update(cx, |s, _| s.java_missing = false));
    cx.update(|_, cx| status.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    assert!(cx.debug_bounds("status-java-missing").is_none(), "with the engine there is nothing to say");
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn a_command_line_path_may_name_a_line_and_column() {
    use crate::model::split_line_suffix;
    assert_eq!(split_line_suffix("/a/B.java:12"), Some(("/a/B.java".into(), (11, 0))));
    assert_eq!(split_line_suffix("/a/B.java:12:5"), Some(("/a/B.java".into(), (11, 4))));
    assert_eq!(split_line_suffix("/a/B.java"), None);
}

/// Providers that count the editor's questions: a hover answers after `HOVER_TAKES`, as
/// a busy server does; a definition at once, in another file.
struct Probe {
    hovers: std::rc::Rc<std::cell::Cell<usize>>,
    definitions: std::rc::Rc<std::cell::Cell<usize>>,
    cache: crate::lang::providers::HoverCache,
}

const HOVER_TAKES: std::time::Duration = std::time::Duration::from_millis(1000);

impl gpui_component::input::HoverProvider for Probe {
    fn hover(
        &self,
        text: &gpui_component::input::Rope,
        offset: usize,
        _: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> gpui::Task<anyhow::Result<Option<lsp_types::Hover>>> {
        let hovers = self.hovers.clone();
        self.cache.hover(text, offset, cx, |cx| {
            hovers.set(hovers.get() + 1);
            let timer = cx.background_executor().timer(HOVER_TAKES);
            cx.background_executor().spawn(async move {
                timer.await;
                Ok(Some(lsp_types::Hover {
                    contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String("probe".into())),
                    range: None,
                }))
            })
        })
    }
}

impl gpui_component::input::DefinitionProvider for Probe {
    fn definitions(
        &self,
        _: &gpui_component::input::Rope,
        _: usize,
        _: &mut gpui::Window,
        _: &mut gpui::App,
    ) -> gpui::Task<anyhow::Result<Vec<lsp_types::LocationLink>>> {
        self.definitions.set(self.definitions.get() + 1);
        let range = lsp_types::Range::default();
        let uri: lsp_types::Uri = "file:///elsewhere/Other.java".parse().unwrap();
        gpui::Task::ready(Ok(vec![lsp_types::LocationLink {
            origin_selection_range: None,
            target_uri: uri,
            target_range: range,
            target_selection_range: range,
        }]))
    }
}

/// Hover and ⌘-click driven by pointer events through the window, as a hand gives them:
/// a click in the text first, a pointer that keeps moving a little over a word while the
/// server is slow, and a ⌘-click while the hover shows.
#[gpui::test]
fn hover_and_command_click_answer_the_pointer(cx: &mut TestAppContext) {
    use std::time::Duration;
    let text = "class A {\n    void greet() { helper(1); }\n}\n";
    let (repo, _, ws, editor, cx) = open_text("A.txt", text, (0, 0), cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let hovers = std::rc::Rc::new(std::cell::Cell::new(0));
    let definitions = std::rc::Rc::new(std::cell::Cell::new(0));
    let shown = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let probe =
        std::rc::Rc::new(Probe { hovers: hovers.clone(), definitions: definitions.clone(), cache: Default::default() });
    cx.update(|_, cx| {
        let state = editor.read(cx).state.clone();
        let shown = shown.clone();
        state.update(cx, |s, _| {
            let lsp = s.lsp_mut();
            lsp.hover_provider = Some(probe.clone());
            lsp.definition_provider = Some(probe.clone());
            lsp.show_document = Some(std::rc::Rc::new(move |params, _, _| {
                shown.borrow_mut().push(params.uri.to_string());
                true
            }));
        });
    });
    cx.run_until_parked();
    let point_at = |cx: &mut VisualTestContext, at: usize| {
        cx.update(|_, cx| editor.read(cx).state.read(cx).range_to_bounds(&(at..at + 1)).map(|b| b.center()).unwrap())
    };
    let helper = text.find("helper").unwrap();
    let a = point_at(cx, helper + 1);
    let b = point_at(cx, helper + 3);
    let greet = point_at(cx, text.find("greet").unwrap());
    cx.simulate_click(greet, Modifiers::none());
    // The pointer comes to rest on `helper` for a moment, long enough to ask, then moves
    // within the word now and then while the answer is on its way.
    let popover =
        |cx: &mut VisualTestContext| cx.update(|_, cx| editor.read(cx).state.read(cx).hover_popover().is_some());
    for (i, at) in [a, b, a, b].into_iter().enumerate() {
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(!popover(cx), "no answer yet after {i} moves");
    }
    cx.executor().advance_clock(HOVER_TAKES);
    cx.run_until_parked();
    assert!(popover(cx), "the hover shows once the server answers");
    assert_eq!(hovers.get(), 1, "one request for the word, however often the pointer moved");

    // ⌘-click on the word while the hover shows: its definition opens.
    cx.simulate_click(b, crate::tests::primary());
    cx.run_until_parked();
    assert_eq!(definitions.get(), 1);
    assert_eq!(*shown.borrow(), ["file:///elsewhere/Other.java"], "the definition's file is shown");
    // A plain click only moves the caret.
    cx.simulate_click(a, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(definitions.get(), 1);
    let _ = std::fs::remove_dir_all(repo);
}

/// Accepting a completion that adds a single character (`ArrayLis` to `ArrayList`) adds
/// the import it carries; typing that character does not.
#[gpui::test]
fn an_accepted_completion_adds_its_import_however_little_it_inserts(cx: &mut TestAppContext) {
    let repo = demo_repo("completion-import");
    std::fs::write(repo.join("settings.gradle"), "rootProject.name = 'demo'\n").unwrap();
    std::fs::write(repo.join("build.gradle"), "plugins { id 'java' }\n").unwrap();
    let file = repo.join("src/Foo.java");
    let text = "class Foo {\n    Object o = new ArrayLis\n}\n";
    std::fs::write(&file, text).unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    act(&model, cx, Action::OpenFile(file.clone(), None));
    let editor = editor_for(&ws, &file, cx);
    let link = cx.update(|_, cx| editor.read(cx).lang.clone()).expect("a Java file of a build has a server link");
    let import = lsp_types::TextEdit {
        range: lsp_types::Range::new(lsp_types::Position::new(0, 0), lsp_types::Position::new(0, 0)),
        new_text: "import java.util.ArrayList;\n\n".into(),
    };
    let item = lsp_types::CompletionItem {
        label: "ArrayList".into(),
        insert_text: Some("ArrayList".into()),
        additional_text_edits: Some(vec![import]),
        ..Default::default()
    };
    let offer = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let rope = editor.read(cx).state.read(cx).text().clone();
            *link.completions.borrow_mut() = (vec![item.clone()], rope);
        })
    };
    let end = text.find("ArrayLis").unwrap() + "ArrayLis".len();
    // Typed: the `t` is only typing.
    offer(cx);
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(1, 27, w, cx)));
    cx.simulate_input("t");
    assert!(!text_of(&editor, cx).contains("import"), "typing adds no import");
    // Accepted from the menu (GPUI Kit's insert): the import comes with it.
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(text_of(&editor, cx), text);
    offer(cx);
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        let item = lsp_types::CompletionItem { insert_text: None, ..item.clone() };
        state.update(cx, |s, cx| s.insert_completion(&item, end - "ArrayLis".len()..end, w, cx));
    });
    cx.run_until_parked();
    let now = text_of(&editor, cx);
    assert!(now.starts_with("import java.util.ArrayList;\n\nclass Foo"), "{now}");
    assert!(now.contains("new ArrayList\n"), "{now}");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_inspections_count_problems_and_step_through_them(cx: &mut TestAppContext) {
    use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
    let text = "class A {\n    int a = x;\n    int b = y;\n    int c = z;\n}\n";
    let (repo, _, _, editor, cx) = open_text("Inspect.java", text, (0, 0), cx);
    let diag = |line, severity| Diagnostic {
        range: Range::new(Position::new(line, 12), Position::new(line, 13)),
        severity: Some(severity),
        message: "x".into(),
        ..Default::default()
    };
    let list = vec![diag(3, DiagnosticSeverity::WARNING), diag(1, DiagnosticSeverity::ERROR)];
    cx.update(|_, cx| editor.update(cx, |e, cx| e.set_diagnostics(list, cx)));
    cx.update(|_, cx| assert_eq!(editor.read(cx).problem_counts(), (1, 1)));
    let line = |cx: &mut VisualTestContext| cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    let step = |cx: &mut VisualTestContext, forward: bool| {
        cx.update(|w, cx| editor.update(cx, |e, cx| e.jump_problem(forward, w, cx)));
        cx.run_until_parked();
    };
    step(cx, true);
    assert_eq!(line(cx), 1, "the first problem after the caret");
    step(cx, true);
    assert_eq!(line(cx), 3);
    step(cx, true);
    assert_eq!(line(cx), 1, "round the file");
    step(cx, false);
    assert_eq!(line(cx), 3, "and back");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_status_bar_shows_where_the_file_sits(cx: &mut TestAppContext) {
    let repo = demo_repo("crumbs");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    act(&model, cx, Action::OpenFile(repo.join("src/main.rs"), None));
    cx.run_until_parked();
    let crumbs = cx.update(|_, cx| editor_for_app(&ws, &repo.join("src/main.rs"), cx).read(cx).crumbs());
    assert_eq!(crumbs[1..], ["src".to_owned(), "main.rs".to_owned()]);
    assert!(cx.debug_bounds("breadcrumbs").is_some(), "the status bar draws the breadcrumbs");
    assert!(cx.debug_bounds("status-position").is_some());
    // The Git view shows the branch there instead.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Git, w, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("breadcrumbs").is_none());
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn quitting_asks_about_unsaved_files_and_what_runs_and_may_skip_the_question(cx: &mut TestAppContext) {
    use crate::model::{Dialog, QuitChoice};
    let (repo, model, ws, editor, cx) = open_text("Q.java", "class Q {}\n", (0, 0), cx);
    // Nothing to lose: the dialog asks, as Settings says, and offers not to ask again.
    cx.dispatch_action(crate::menus::Quit);
    cx.run_until_parked();
    let dialog = cx.update(|_, cx| model.read(cx).dialog.clone());
    assert!(matches!(&dialog, Some(Dialog::Quit(info)) if info.is_empty()), "{dialog:?}");
    cx.update(gpui_component::WindowExt::close_all_dialogs);
    // A changed tab: named, and asked about even with the question switched off.
    cx.update(|w, cx| editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.insert("x", w, cx))));
    cx.update(|_, cx| model.update(cx, |m, _| m.settings.confirm_quit = false));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.request_quit(w, cx)));
    cx.run_until_parked();
    let dialog = cx.update(|_, cx| model.read(cx).dialog.clone());
    match &dialog {
        Some(Dialog::Quit(info)) => assert_eq!(info.unsaved, ["Q.java"]),
        other => panic!("{other:?}"),
    }
    cx.update(gpui_component::WindowExt::close_all_dialogs);
    cx.update(|_, cx| assert!(!ws.read(cx).quitting, "Cancel stays"));
    // Save All and Quit: written, then the app goes.
    act(&model, cx, Action::Quit(QuitChoice { save: Some(true), dont_ask: false }));
    for _ in 0..20 {
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        cx.run_until_parked();
    }
    assert_eq!(std::fs::read_to_string(repo.join("Q.java")).unwrap(), "xclass Q {}\n");
    cx.update(|_, cx| assert!(ws.read(cx).quitting));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn with_nothing_to_lose_and_the_question_off_quitting_goes_at_once(cx: &mut TestAppContext) {
    let (repo, model, ws, _, cx) = open_text("R.java", "class R {}\n", (0, 0), cx);
    act(&model, cx, Action::Quit(crate::model::QuitChoice { save: None, dont_ask: true }));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(!model.read(cx).settings.confirm_quit, "Don't ask again switches the question off");
        assert!(ws.read(cx).quitting);
    });
    let _ = std::fs::remove_dir_all(repo);
}

/// Click navigation: ⌘ held over a name the engine knows underlines it at once, with a hand;
/// with the setting on ⌥, ⌥ does it and ⌘ does not; the usages popup lists places and closes
/// on Esc.
#[gpui::test]
fn the_navigation_modifier_underlines_a_link_and_the_popup_lists_places(cx: &mut TestAppContext) {
    use crate::editor::navigation::{NavItem, NavPopup};
    let text = "class A {\n    void greet() { helper(1); }\n}\n";
    let (repo, model, _ws, editor, cx) = open_text("Nav.txt", text, (0, 0), cx);
    let definitions = std::rc::Rc::new(std::cell::Cell::new(0));
    let probe = std::rc::Rc::new(Probe {
        hovers: Default::default(),
        definitions: definitions.clone(),
        cache: Default::default(),
    });
    cx.update(|_, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, _| s.lsp_mut().definition_provider = Some(probe.clone()));
    });
    cx.run_until_parked();
    let helper = text.find("helper").unwrap();
    let at =
        cx.update(|_, cx| editor.read(cx).state.read(cx).range_to_bounds(&(helper + 2..helper + 3)).unwrap().center());
    let link = |cx: &mut VisualTestContext| cx.update(|_, cx| editor.read(cx).nav.link.clone());
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(link(cx), None, "no link without the modifier");
    cx.simulate_mouse_move(at, None, crate::tests::primary());
    cx.run_until_parked();
    assert_eq!(link(cx), Some(helper..helper + 6), "underlined at once");
    assert!(cx.debug_bounds("nav-link").is_some(), "drawn, with the hand");
    // (GPUI Kit asks too, for its own link.)
    let asked = definitions.get();
    assert!(asked >= 1);
    // Let go: the link goes; held again over the same word: known, not asked again.
    cx.simulate_modifiers_change(Modifiers::none());
    cx.run_until_parked();
    assert_eq!(link(cx), None);
    cx.simulate_mouse_move(at, None, crate::tests::primary());
    cx.run_until_parked();
    assert_eq!(link(cx), Some(helper..helper + 6));
    assert_eq!(definitions.get(), asked, "known: not asked again for the word");
    // ⌥ chosen in the settings: ⌘ no longer underlines, ⌥ does.
    cx.update(|_, cx| {
        let mut s = model.read(cx).settings.clone();
        s.click_nav_modifier = crate::settings::NavModifier::Alt;
        cx.set_global(crate::editor::EditorDefaults::of(&s));
    });
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_move(at, None, crate::tests::primary());
    cx.run_until_parked();
    assert_eq!(link(cx), None, "⌘ is not the key any more");
    cx.simulate_mouse_move(at, None, Modifiers::alt());
    cx.run_until_parked();
    assert_eq!(link(cx), Some(helper..helper + 6));
    // The popup of places: drawn, ↓ picks, Esc closes.
    cx.update(|w, cx| {
        editor.update(cx, |e, cx| {
            let focus = cx.focus_handle();
            focus.focus(w, cx);
            let item = |line: u32, preview: &str| NavItem {
                uri: "file:///p/src/A.java".parse().unwrap(),
                at: lsp_types::Position::new(line, 4),
                file: "A.java".into(),
                dir: "src".into(),
                preview: preview.into(),
            };
            e.nav.popup = Some(NavPopup {
                title: "Usages of helper".into(),
                at: gpui::point(gpui::px(200.), gpui::px(200.)),
                items: vec![item(3, "helper(1);"), item(9, "helper(2);")],
                selected: 0,
                focus,
            });
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("nav-popup").is_some() && cx.debug_bounds("nav-item-1").is_some());
    cx.simulate_keystrokes("down");
    assert_eq!(cx.update(|_, cx| editor.read(cx).nav.popup.as_ref().map(|p| p.selected)), Some(1));
    cx.simulate_keystrokes("escape");
    assert!(cx.update(|_, cx| editor.read(cx).nav.popup.is_none()), "Esc closes it");
    let _ = std::fs::remove_dir_all(repo);
}

/// Every test's workspace lives in the one test process, so that process's children are the
/// other tests' too (their `git` commands, terminals and engines): quitting in one test stops
/// what its workspace started, never those.
#[gpui::test]
fn quitting_in_a_test_leaves_the_other_tests_processes_be(cx: &mut TestAppContext) {
    let mut other = crate::tests::sleeper().spawn().unwrap();
    let (repo, model, ws, _, cx) = open_text("Spare.java", "class Spare {}\n", (0, 0), cx);
    act(&model, cx, Action::Quit(crate::model::QuitChoice { save: None, dont_ask: true }));
    cx.run_until_parked();
    cx.update(|_, cx| assert!(ws.read(cx).quitting));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let status = other.try_wait().unwrap();
    let _ = other.kill();
    let _ = other.wait();
    let _ = std::fs::remove_dir_all(repo);
    assert_eq!(status, None, "another test's process was stopped");
}
