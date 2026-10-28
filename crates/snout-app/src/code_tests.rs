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
    cx.update(|_, cx| ws.read(cx).editors().into_iter().find(|e| e.read(cx).path == path)).expect("a tab for the file")
}

fn tab_names(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| ws.read(cx).editors().iter().map(|e| e.read(cx).name().to_string()).collect())
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
    ws.read(cx).editors().into_iter().find(|e| e.read(cx).path == path).unwrap()
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
