//! Headless tests of problems and their fixes: the hover card with the best fix, the hover
//! setting, ⌥⇧↩, the gutter's bulb, the context actions with Fix all and Ask the agent to
//! fix, and the Problems window grouped, filtered and with its own fixes. The engine's
//! answers are canned ([`crate::editor::CannedLang`]); `lang_tests` has the real engine.

use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::input::RopeExt as _;
use lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, TextEdit,
};

use crate::editor::{CannedLang, EditorDefaults, EditorPanel, Fixes};
use crate::model::Action;
use crate::settings::{Settings, ShowHover};
use crate::tests::{act, demo_repo, open, scratch_dir, with_repo};
use crate::workspace::Workspace;

/// A class with a type not imported (line 3) and two unused imports (lines 1 and 2).
const TEXT: &str = "package demo;\nimport java.util.List;\nimport java.util.Map;\nclass A {\n    LocalDate when = LocalDate.now();\n}\n";

fn diag(line: u32, a: u32, b: u32, code: i32, severity: DiagnosticSeverity, message: &str) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(line, a), Position::new(line, b)),
        severity: Some(severity),
        code: Some(NumberOrString::Number(code)),
        source: Some("Java".into()),
        message: message.into(),
        ..Default::default()
    }
}

fn problems() -> Vec<Diagnostic> {
    vec![
        diag(1, 7, 21, 268435844, DiagnosticSeverity::WARNING, "The import java.util.List is never used"),
        diag(2, 7, 20, 268435844, DiagnosticSeverity::WARNING, "The import java.util.Map is never used"),
        diag(4, 4, 13, 16777218, DiagnosticSeverity::ERROR, "LocalDate cannot be resolved to a type"),
    ]
}

/// The engine as these tests have it: an import for `LocalDate`, the removal of an unused
/// import, and a refactoring anywhere.
fn canned(path: &Path) -> CannedLang {
    let uri = snout_lang::uri::from_path(path);
    let edit = move |edits: Vec<TextEdit>| crate::lang::fixes::file_edit(&uri, edits);
    let actions = move |_: &Path, range: &Range| {
        let mut out = Vec::new();
        let all = problems();
        for d in all.iter().filter(|d| d.range.start.line == range.start.line) {
            let (title, edits) = if d.code == Some(NumberOrString::Number(16777218)) {
                (
                    "Import 'LocalDate' (java.time)".to_owned(),
                    vec![TextEdit::new(
                        Range::new(Position::new(1, 0), Position::new(1, 0)),
                        "import java.time.LocalDate;\n".into(),
                    )],
                )
            } else {
                let line = d.range.start.line;
                (
                    "Remove unused import".to_owned(),
                    vec![TextEdit::new(Range::new(Position::new(line, 0), Position::new(line + 1, 0)), String::new())],
                )
            };
            out.push(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![d.clone()]),
                edit: Some(edit(edits)),
                is_preferred: Some(true),
                ..Default::default()
            });
        }
        out.push(CodeAction {
            title: "Extract to local variable".into(),
            kind: Some(CodeActionKind::REFACTOR_EXTRACT),
            ..Default::default()
        });
        out
    };
    CannedLang {
        actions: Rc::new(actions),
        hover: Rc::new(|_, word| {
            (word == "now").then(|| "```java\nLocalDate java.time.LocalDate.now()\n```\nThe current date.".into())
        }),
    }
}

/// The class open with its problems, the engine canned.
fn setup<'a>(
    name: &str,
    cx: &'a mut TestAppContext,
) -> (std::path::PathBuf, Entity<Workspace>, Entity<EditorPanel>, &'a mut VisualTestContext) {
    let repo = demo_repo(name);
    let path = repo.join("A.java");
    std::fs::write(&path, TEXT).unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|_, cx| cx.set_global(canned(&path)));
    act(&model, cx, Action::OpenFile(path.clone(), Some((0, 0))));
    let editor = crate::code_tests::editor_for(&ws, &path, cx);
    cx.update(|_, cx| editor.update(cx, |e, cx| e.set_diagnostics(problems(), cx)));
    cx.run_until_parked();
    (repo, ws, editor, cx)
}

fn point_of(editor: &Entity<EditorPanel>, find: &str, cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    cx.update(|_, cx| {
        let s = editor.read(cx).state.read(cx);
        let at = s.text().to_string().find(find).unwrap() + 1;
        s.range_to_bounds(&(at..at + 1)).unwrap().center()
    })
}

fn card_shows(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| editor.read(cx).hover.card.as_ref().is_some_and(|c| c.ready()))
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn text(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).text(cx))
}

#[gpui::test]
fn hovering_a_problem_shows_its_best_fix_after_the_delay_and_the_button_applies_it(cx: &mut TestAppContext) {
    let (repo, _, editor, cx) = setup("hover-fix", cx);
    let at = point_of(&editor, "LocalDate when", cx);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait(cx, 200);
    assert!(!card_shows(&editor, cx), "not before the delay");
    wait(cx, 400);
    assert!(card_shows(&editor, cx), "the card shows once the pointer rests");
    cx.update(|_, cx| {
        let s = editor.read(cx).state.read(cx);
        assert!(s.diagnostic_popover().is_none(), "GPUI Kit's own popover gives way to the card");
        let card = editor.read(cx).hover.card.as_ref().unwrap();
        assert_eq!(card.problems[0].message, "LocalDate cannot be resolved to a type");
        let Fixes::Found(list) = &card.fixes else { panic!("the fixes are found") };
        assert_eq!(list[0].title, "Import 'LocalDate' (java.time)");
    });
    let button = cx.debug_bounds("hover-fix").expect("the best fix is a button");
    let more = cx.debug_bounds("hover-more").expect("More actions… follows it");
    assert!(more.left() > button.right());
    cx.simulate_mouse_move(button.center(), None, Modifiers::none());
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(text(&editor, cx).contains("import java.time.LocalDate;\n"), "the import is added");
    assert!(!card_shows(&editor, cx), "the card goes");

    // Hovering a symbol with no problem: what the engine says it is.
    let now = point_of(&editor, "now()", cx);
    cx.simulate_mouse_move(now, None, Modifiers::none());
    wait(cx, 600);
    cx.update(|_, cx| {
        let card = editor.read(cx).hover.card.as_ref().expect("a card for the symbol");
        assert!(card.problems.is_empty());
        assert!(card.info.as_deref().unwrap().contains("The current date."));
    });
    // Moving away puts it away, after a moment.
    let body = cx.debug_bounds("editor-body").unwrap();
    cx.simulate_mouse_move(body.bottom_right() - gpui::point(gpui::px(40.), gpui::px(40.)), None, Modifiers::none());
    wait(cx, 400);
    assert!(!card_shows(&editor, cx));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_problem_mark_in_the_gutter_shows_the_card_too(cx: &mut TestAppContext) {
    let (repo, _, editor, cx) = setup("hover-gutter", cx);
    let (lane, y) = cx.update(|_, cx| {
        let e = editor.read(cx);
        let s = e.state.read(cx);
        let at = s.text().line_start_offset(4);
        let b = s.range_to_bounds(&(at..at)).unwrap();
        let (a, z) = e.lanes.get().problems();
        ((a + z) / 2., b.center().y)
    });
    cx.simulate_mouse_move(gpui::point(lane, y), None, Modifiers::none());
    wait(cx, 600);
    cx.update(|_, cx| {
        let card = editor.read(cx).hover.card.as_ref().expect("a card beside the mark");
        assert_eq!(card.spot, crate::editor::Spot::Line(4));
        assert_eq!(card.problems.len(), 1);
    });
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn with_hover_only_while_command_is_held_a_rest_shows_nothing_and_command_shows_it(cx: &mut TestAppContext) {
    let (repo, _, editor, cx) = setup("hover-command", cx);
    cx.update(|_, cx| {
        let s = Settings { show_hover: ShowHover::WhileCommandHeld, ..Settings::default() };
        cx.set_global(EditorDefaults::of(&s));
    });
    let at = point_of(&editor, "LocalDate when", cx);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait(cx, 1500);
    assert!(!card_shows(&editor, cx), "a rest alone shows no card");
    cx.simulate_mouse_move(at, None, Modifiers::command());
    wait(cx, 100);
    assert!(card_shows(&editor, cx), "⌘ shows it at once");
    // The delay of the other mode is a setting too.
    cx.update(|_, cx| {
        let s = Settings { hover_delay_ms: 1200, ..Settings::default() };
        cx.set_global(EditorDefaults::of(&s));
        editor.update(cx, |e, cx| e.hide_card(cx));
    });
    let now = point_of(&editor, "now()", cx);
    cx.simulate_mouse_move(now, None, Modifiers::none());
    wait(cx, 800);
    assert!(!card_shows(&editor, cx), "not before its 1200 ms");
    wait(cx, 600);
    assert!(card_shows(&editor, cx));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn alt_shift_enter_applies_the_best_fix_at_the_caret_and_the_bulb_shows_on_its_line(cx: &mut TestAppContext) {
    let (repo, _, editor, cx) = setup("top-fix", cx);
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(4, 6, w, cx)));
    wait(cx, 400);
    cx.update(|_, cx| {
        let bulb = editor.read(cx).hover.bulb.expect("a bulb on the caret's line");
        assert_eq!((bulb.line, bulb.error), (4, true));
    });
    assert!(cx.update(|_, cx| editor.read(cx).hover.bulb_painted.get().is_some()), "the bulb is drawn");
    // A line with no problem has none.
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(0, 2, w, cx)));
    wait(cx, 400);
    assert!(cx.update(|_, cx| editor.read(cx).hover.bulb.is_none()));
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(4, 6, w, cx)));
    wait(cx, 400);
    // A click on the bulb lists the line's actions.
    let bulb = cx.update(|_, cx| editor.read(cx).hover.bulb_painted.get().unwrap());
    cx.simulate_click(bulb.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(|_, cx| editor.read(cx).hover.actions.is_some()), "the bulb opens the actions");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| editor.read(cx).hover.actions.is_none()));
    cx.simulate_keystrokes("alt-shift-enter");
    cx.run_until_parked();
    assert!(text(&editor, cx).contains("import java.time.LocalDate;\n"), "⌥⇧↩ imports it");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_context_actions_offer_fix_all_in_file_and_apply_it(cx: &mut TestAppContext) {
    let (repo, _, editor, cx) = setup("fix-all", cx);
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(1, 10, w, cx)));
    cx.run_until_parked();
    cx.simulate_keystrokes("alt-enter");
    cx.run_until_parked();
    let labels: Vec<String> = cx.update(|_, cx| {
        let list = editor.read(cx).hover.actions.as_ref().expect("⌥↩ lists the actions");
        list.entries.iter().map(|e| e.label("Claude")).collect()
    });
    assert_eq!(
        labels,
        [
            "Remove unused import",
            "Fix all \u{2018}Remove unused import\u{2019} in file",
            "Ask Claude to fix",
            "Extract to local variable"
        ]
    );
    assert!(cx.debug_bounds("context-actions-row-1").is_some(), "the list is drawn");
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    let after = text(&editor, cx);
    assert!(!after.contains("import java.util.List;") && !after.contains("import java.util.Map;"), "{after}");
    assert!(after.contains("LocalDate when"), "the other problem stays");
    // ⌘. lists them too.
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(2, 8, w, cx)));
    cx.simulate_keystrokes("cmd-.");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| editor.read(cx).hover.actions.is_some()));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn ask_to_fix_types_the_file_lines_and_problem_into_the_agents_prompt(cx: &mut TestAppContext) {
    let (repo, ws, editor, cx) = setup("ask-fix", cx);
    let bin = scratch_dir("ask-fix-bin");
    // A stand-in for the agent: it waits, and the terminal echoes what is typed at it.
    let fake = bin.join("claude");
    std::fs::write(&fake, "#!/bin/sh\necho ready\nsleep 30\n").unwrap();
    Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).expect("the console is docked");
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    let wait_for = |cx: &mut VisualTestContext, what: &str| {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            cx.executor().advance_clock(Duration::from_millis(50));
            cx.run_until_parked();
            let text = cx.update(|_, cx| console.read(cx).session_text(&repo)).unwrap_or_default();
            if text.replace('\n', "").contains(what) {
                return true;
            }
        }
        false
    };
    // The context actions' Ask Claude to fix on the error.
    cx.update(|w, cx| editor.update(cx, |e, cx| e.go_to(4, 6, w, cx)));
    cx.run_until_parked();
    cx.simulate_keystrokes("alt-enter");
    cx.run_until_parked();
    let ask = cx.update(|_, cx| {
        let list = editor.read(cx).hover.actions.as_ref().unwrap();
        list.entries.iter().position(|e| matches!(e, crate::editor::context_actions::Entry::Ask(_))).unwrap()
    });
    cx.update(|w, cx| editor.update(cx, |e, cx| e.pick_action(ask, w, cx)));
    let ok = wait_for(cx, "ready");
    let shown =
        cx.update(|_, cx| (console.read(cx).session_text(&repo), ws.read(cx).model.read(cx).settings.console_open));
    assert!(ok, "the agent runs in the console: {shown:?}");
    assert!(
        wait_for(cx, "@A.java#L5 Fix this error: LocalDate cannot be resolved to a type"),
        "the mention and the problem are typed into its prompt"
    );
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(bin);
}

#[gpui::test]
fn the_problems_window_groups_by_file_filters_and_toggles_severities(cx: &mut TestAppContext) {
    use crate::lang::problems::Row;
    let (repo, ws, editor, cx) = setup("problems-window", cx);
    let path = repo.join("A.java");
    let other = repo.join("B.java");
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    let uri = |p: &Path| snout_lang::uri::from_path(p).as_str().to_owned();
    let mut many: Vec<Diagnostic> = (0..260)
        .map(|i| diag(i, 0, 4, 536871097, DiagnosticSeverity::WARNING, &format!("Unused local variable v{i}")))
        .collect();
    many.push(diag(3, 0, 4, 1, DiagnosticSeverity::INFORMATION, "Can be final"));
    cx.update(|_, cx| {
        hub.update(cx, |h, cx| {
            h.insert_published(&repo, uri(&path), problems());
            h.insert_published(&repo, uri(&other), many);
            cx.notify();
        })
    });
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_problems(w, cx)));
    cx.run_until_parked();
    let problems = cx.update(|_, cx| ws.read(cx).problems.clone());
    cx.update(|_, cx| {
        let pp = problems.read(cx);
        assert_eq!(pp.groups.len(), 2);
        assert_eq!(pp.groups[0].name.as_ref(), "A.java", "the file with an error first");
        assert_eq!(pp.groups[0].counts, [1, 2, 0]);
        assert_eq!(pp.groups[1].counts, [0, 260, 1]);
        assert_eq!(pp.rows.len(), 2 + 3 + 261);
    });
    assert!(cx.debug_bounds("problems-errors").is_some(), "the severity toggles are drawn");
    // Warnings off: only the error and the information are left.
    cx.update(|_, cx| problems.update(cx, |p, cx| p.toggle_severity(1, cx)));
    assert_eq!(cx.update(|_, cx| problems.read(cx).rows.len()), 4);
    cx.update(|_, cx| problems.update(cx, |p, cx| p.toggle_severity(1, cx)));
    // The filter narrows by message (and file).
    cx.update(|w, cx| {
        let f = problems.read(cx).filter.clone();
        f.update(cx, |f, cx| f.set_value("v25", w, cx));
    });
    cx.run_until_parked();
    let rows = cx.update(|_, cx| problems.read(cx).rows.as_ref().clone());
    assert_eq!(rows.len(), 1 + 11, "v25 and v250 to v259: {rows:?}");
    cx.update(|w, cx| problems.read(cx).filter.clone().update(cx, |f, cx| f.set_value("", w, cx)));
    cx.run_until_parked();
    // A group collapses.
    cx.update(|_, cx| problems.update(cx, |p, cx| p.toggle_group(1, cx)));
    assert_eq!(cx.update(|_, cx| problems.read(cx).rows.len()), 2 + 3);
    // A double click goes to a problem.
    cx.run_until_parked();
    let row = cx.debug_bounds("problem-row-1").expect("a problem row");
    let error = cx_problem(&problems, "LocalDate", cx);
    assert_eq!(cx.update(|_, cx| problems.read(cx).rows[1]), Row::Problem(error), "the error first");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.update(|_, cx| assert_eq!(problems.read(cx).selected, Some(Row::Problem(error))));
    cx.simulate_event(gpui::MouseDownEvent {
        position: row.center(),
        button: gpui::MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: row.center(),
        button: gpui::MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    cx.run_until_parked();
    let caret = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position());
    assert_eq!(caret.line, 4, "the caret is on the problem");
    // A right click: its fixes, Fix all and Ask.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_problems(w, cx)));
    cx.run_until_parked();
    let row = cx.debug_bounds("problem-row-2").expect("the first warning's row");
    cx.simulate_mouse_down(row.center(), gpui::MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(row.center(), gpui::MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let labels: Vec<String> = cx.update(|_, cx| {
        let list = problems.read(cx).actions.as_ref().expect("the actions of the problem");
        list.entries.iter().map(|e| e.label("Claude")).collect()
    });
    assert!(labels.iter().any(|l| l.starts_with("Fix all")), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Ask Claude to fix"), "{labels:?}");
    let _ = std::fs::remove_dir_all(repo);
}

fn cx_problem(problems: &Entity<crate::lang::panels::ProblemsPanel>, text: &str, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| problems.read(cx).problems.iter().position(|p| p.text.contains(text)).unwrap())
}
