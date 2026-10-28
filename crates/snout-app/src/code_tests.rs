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
    let tab = cx.debug_bounds("editor-tab-2").expect("the README tab is drawn").center();
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
    let tab = cx.debug_bounds("editor-tab-1").expect("the README tab is drawn").center();
    cx.simulate_mouse_down(tab, MouseButton::Middle, Modifiers::none());
    cx.simulate_mouse_up(tab, MouseButton::Middle, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(tab_names(&ws, cx), ["status.rs"]);
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_status_bar_follows_the_editor_in_front(cx: &mut TestAppContext) {
    let repo = demo_repo("status-bar");
    std::fs::write(repo.join("App.java"), "class App {\r\n\tint x;\r\n}\r\n").unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Projects, w, cx)));
    act(&model, cx, Action::OpenFile(repo.join("App.java"), Some((1, 1))));
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).editor_status(cx)).expect("an editor is in front");
    assert_eq!(status.position, "Ln 2, Col 2");
    assert_eq!((status.endings, status.language, status.encoding), ("CRLF", "Java", "UTF-8"));
    assert!(status.indent.starts_with("Tab Size"), "{}", status.indent);
    cx.simulate_keystrokes("shift-right");
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).editor_status(cx)).unwrap();
    assert_eq!(status.selection.as_deref(), Some("1 selected"));
    // The tree shows the file selected.
    let files = cx.update(|_, cx| ws.read(cx).side.read(cx).files.clone());
    cx.update(|_, cx| assert_eq!(files.read(cx).active.as_deref(), Some(repo.join("App.java").as_path())));
    act(&model, cx, Action::OpenFile(repo.join("status.rs"), None));
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).editor_status(cx)).unwrap();
    assert_eq!((status.endings, status.language, status.indent.as_str()), ("LF", "Rust", "Spaces: 4"));
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
    cx.simulate_keystrokes("cmd-f");
    cx.update(|_, cx| assert!(state.read(cx).search_session().open, "⌘F finds in the file"));
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes("cmd-alt-f");
    cx.update(|_, cx| {
        let s = state.read(cx).search_session();
        assert!(s.open && s.replace_mode, "⌥⌘F replaces");
    });
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes("cmd-shift-f");
    cx.update(|_, cx| {
        let search = ws.read(cx).search.clone().expect("⇧⌘F in the editor is still find in files");
        assert_eq!(search.read(cx).mode, crate::search::Mode::FindInFiles);
    });
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes("cmd-l");
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
    cx.simulate_keystrokes("cmd-/");
    assert_eq!(text_of(&editor, cx), "class A {\n    // int x;\n    int y;\n}\n");
    cx.simulate_keystrokes("cmd-/");
    assert_eq!(text_of(&editor, cx), text, "⌘/ again uncomments");
    cx.simulate_keystrokes("cmd-d");
    assert_eq!(text_of(&editor, cx), "class A {\n    int x;\n    int x;\n    int y;\n}\n");
    cx.simulate_keystrokes("cmd-z");
    assert_eq!(text_of(&editor, cx), text, "one undo takes the duplicate back");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(2, 4, w, cx)));
    cx.simulate_keystrokes("alt-up");
    assert_eq!(text_of(&editor, cx), "class A {\n    int y;\n    int x;\n}\n");
    cx.simulate_keystrokes("alt-down alt-down");
    assert_eq!(text_of(&editor, cx), "class A {\n    int x;\n}\n    int y;\n");
    cx.update(|_, cx| assert_eq!(editor.read(cx).state.read(cx).cursor_position().line, 3, "the caret moves along"));
    // ⌥Z wraps long lines, and again does not.
    cx.update(|_, cx| assert!(!editor.read(cx).soft_wrap));
    cx.simulate_keystrokes("alt-z");
    cx.update(|_, cx| assert!(editor.read(cx).soft_wrap));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_font_zooms_for_every_editor_and_is_remembered(cx: &mut TestAppContext) {
    let (repo, model, _, _, cx) = open_text("Z.java", "class Z {}\n", (0, 0), cx);
    let size = |cx: &mut VisualTestContext| cx.update(|_, cx| crate::editor::font_size(cx));
    assert_eq!(size(cx), crate::look::MONO_SIZE);
    cx.simulate_keystrokes("cmd-= cmd-=");
    assert_eq!(size(cx), crate::look::MONO_SIZE + 2.);
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.editor_font_size, crate::look::MONO_SIZE + 2.));
    cx.simulate_keystrokes("cmd--");
    assert_eq!(size(cx), crate::look::MONO_SIZE + 1.);
    cx.simulate_keystrokes("cmd-alt-0");
    assert_eq!(size(cx), crate::look::MONO_SIZE, "⌥⌘0 (⌘0 is the Git view) resets it");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn closing_a_tab_or_the_window_with_changes_asks_first(cx: &mut TestAppContext) {
    let (repo, model, ws, editor, cx) = open_text("C.java", "class C {}\n", (0, 0), cx);
    cx.simulate_keystrokes("x");
    cx.simulate_keystrokes("cmd-w");
    assert!(cx.has_pending_prompt(), "⌘W on a changed tab asks");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(tab_names(&ws, cx), ["C.java"], "Cancel keeps the tab");
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 0, w, cx)));
    cx.simulate_keystrokes("cmd-w");
    cx.simulate_prompt_answer("Save");
    cx.run_until_parked();
    assert!(tab_names(&ws, cx).is_empty(), "Save closes it");
    assert_eq!(std::fs::read_to_string(repo.join("C.java")).unwrap(), "xclass C {}\n");

    // A clean tab closes without asking; Don't Save drops the changes.
    act(&model, cx, Action::OpenFile(repo.join("README.md"), Some((0, 0))));
    cx.simulate_keystrokes("y");
    cx.simulate_keystrokes("cmd-w");
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
    cx.simulate_keystrokes("cmd-s");
    crate::tests::git(&repo, &["commit", "-qam", "Edit the long file"]);
    act(&model, cx, Action::Refresh);
    settle(cx);
    cx.update(|_, cx| assert!(editor.read(cx).changes.is_empty()));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_command_palette_lists_every_command_with_its_keys_and_runs_one(cx: &mut TestAppContext) {
    let (repo, _, ws, editor, cx) = open_text("P.java", "class P {}\n", (0, 0), cx);
    cx.simulate_keystrokes("cmd-shift-p");
    let palette = cx.update(|_, cx| ws.read(cx).search.clone()).expect("⇧⌘P puts up the palette");
    cx.update(|_, cx| {
        let p = palette.read(cx);
        assert_eq!(p.mode, crate::search::Mode::Commands);
        assert_eq!(p.items.len(), crate::menus::commands().len(), "every command is listed");
        let keys = |name: &str| p.items.iter().find(|i| i.path == name).and_then(|i| i.keys.clone());
        assert_eq!(keys("Toggle Comment").as_deref(), Some("⌘/"), "with the keys it has in the editor");
        assert_eq!(keys("Go to File…").as_deref(), Some("⌘P"));
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
    cx.simulate_keystrokes("cmd-\\");
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
    cx.simulate_keystrokes("cmd-s");
    cx.update(|_, cx| assert!(!editor.read(cx).dirty && !twin.read(cx).dirty, "saving one saves both"));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_window_opens_on_the_activity_it_showed_last(cx: &mut TestAppContext) {
    let repo = demo_repo("activity");
    let (model, ws, vcx) = open(cx, with_repo(&repo));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Projects, w, cx)));
    let saved = vcx.update(|_, cx| model.read(cx).settings.clone());
    assert_eq!(saved.activity, Some(Activity::Projects));
    let model2 = cx.update(|cx| gpui::AppContext::new(cx, |cx| crate::model::Model::new(saved.clone(), None, cx)));
    let mut ws2 = None;
    let (_, vcx2) = cx.add_window_view(|window, cx| {
        let view = gpui::AppContext::new(cx, |cx| Workspace::new(model2.clone(), window, cx));
        ws2 = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx2.run_until_parked();
    vcx2.update(|_, cx| assert_eq!(ws2.unwrap().read(cx).side.read(cx).activity, Activity::Projects));
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
    // A frame with nothing new, for comparison.
    let started = Instant::now();
    for _ in 0..40 {
        cx.update(|w, cx| {
            w.refresh();
            w.draw(cx).clear(cx)
        });
    }
    let idle = started.elapsed() / 40;
    // Typing: each keystroke and the frame after it.
    let started = Instant::now();
    for _ in 0..40 {
        cx.simulate_keystrokes("a");
        cx.update(|w, cx| w.draw(cx).clear(cx));
    }
    let typing = started.elapsed() / 40;
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(editor.read(cx).changes.len(), 1, "the gutter marks the typed line"));
    // Scrolling: a page at a time through the file, a frame each.
    let state = cx.update(|_, cx| editor.read(cx).state.clone());
    let started = Instant::now();
    for i in 0..40 {
        cx.update(|_, cx| {
            state.update(cx, |s, cx| s.set_scroll_offset(gpui::point(gpui::px(0.), gpui::px(-600. * i as f32)), cx))
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
    }
    let scrolling = started.elapsed() / 40;
    println!("Big.java: {idle:?} a frame, {typing:?} a keystroke, {scrolling:?} a scrolled frame");
    // Debug builds are several times slower than release; these bounds catch a regression
    // that does whole-file work on every keystroke or frame, not the last millisecond.
    let limit = std::time::Duration::from_millis(if cfg!(debug_assertions) { 120 } else { 16 });
    assert!(typing < limit, "typing takes {typing:?} a keystroke");
    assert!(scrolling < limit, "scrolling takes {scrolling:?} a frame");
    assert_eq!(text_of(&editor, cx).len(), text.len() + 40);
    let _ = std::fs::remove_dir_all(repo);
}
