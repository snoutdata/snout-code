//! Headless tests of the Java tools against the real engine, on a small Gradle project
//! (`tests/fixtures/shop`): refactoring, formatting a selection, parameter name hints, the
//! Structure and Hierarchy windows and Go to Class. Like `lang_tests`, they need the engine
//! distribution and a JDK 17; without them they say so and pass.

use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::editor::EditorPanel;
use crate::lang::java;
use crate::lang_tests::{TestWorkspace, caret_at, editor_of, jdk17, pump_until, text};
use crate::model::{Action, Model};
use crate::panels::Activity;
use crate::tests::{act, git, open, scratch_dir, with_repo};
use crate::workspace::Workspace;

/// The shop fixture, copied and committed: the build is the repository.
fn shop(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shop");
    let ok = std::process::Command::new("cp").arg("-R").arg(&from).arg(&dir).status().unwrap().success();
    assert!(ok, "copy the fixture");
    let repo = dir.join("shop");
    std::fs::write(repo.join("gradle.properties"), "org.gradle.daemon.idletimeout=60000\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    let c = ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"];
    git(&repo, &[&c[..], &["add", "-A"]].concat());
    git(&repo, &[&c[..], &["commit", "-qm", "fixture"]].concat());
    repo
}

/// The workspace on a copy of the shop with its engine running and `file` open (under
/// `src/main/java/demo/shop`), once the engine has checked it.
struct Started<'a> {
    model: Entity<Model>,
    ws: Entity<Workspace>,
    cx: &'a mut VisualTestContext,
    repo: PathBuf,
    editor: Entity<EditorPanel>,
}

fn start<'a>(cx: &'a mut TestAppContext, name: &str, file: &str) -> Option<Started<'a>> {
    let (Some(_), Some(jdk)) = (java::engine_dist(&*snout_host::local()), jdk17()) else {
        println!("java tools: skipped; needs engine/build/dist and a JDK 17 (SNOUT_TEST_JDK17)");
        return None;
    };
    cx.executor().allow_parking();
    let repo = shop(name);
    let mut settings = with_repo(&repo);
    settings.project_jdks.insert(repo.clone(), jdk);
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, settings);
    cx.update(|_, cx| cx.set_global(TestWorkspace(ws.clone())));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let path = repo.join("src/main/java/demo/shop").join(file);
    act(&model, cx, Action::OpenFile(path.clone(), None));
    let editor = editor_of(&ws, &path, cx).expect("the file opens");
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    let started = Instant::now();
    let ready = pump_until(cx, 240, |cx| {
        cx.update(|_, cx| hub.read(cx).servers.get(&repo).is_some_and(|s| s.first_diagnostics.is_some()))
    });
    assert!(ready, "the engine answers within four minutes");
    println!("java tools: first diagnostics after {:.1} s", started.elapsed().as_secs_f32());
    Some(Started { model, ws, cx, repo, editor })
}

impl Started<'_> {
    /// Select the `nth` match of `find`.
    fn select(&mut self, find: &str, nth: usize) {
        caret_at(&self.editor, find, nth, 0, self.cx);
        let editor = self.editor.clone();
        self.cx.update(|_, cx| {
            editor.update(cx, |e, cx| {
                e.state.update(cx, |s, cx| {
                    let at = s.cursor();
                    s.set_selected_range(at..at + find.len(), cx);
                })
            })
        });
        self.cx.run_until_parked();
    }

    fn text(&mut self) -> String {
        text(&self.editor, self.cx)
    }

    fn put_back(&mut self, original: &str) {
        let editor = self.editor.clone();
        self.cx.update(|window, cx| {
            editor.update(cx, |e, cx| {
                e.rename = None;
                e.state.update(cx, |s, cx| s.set_value(original.to_owned(), window, cx))
            })
        });
        self.cx.run_until_parked();
    }

    /// Wait for the text to hold `what`.
    fn wait_for_text(&mut self, secs: u64, what: impl Fn(&str) -> bool) -> bool {
        let editor = self.editor.clone();
        pump_until(self.cx, secs, |cx| what(&text(&editor, cx)))
    }

    fn stop(self) {
        let hub = self.cx.update(|_, cx| self.ws.read(cx).hub.clone());
        self.cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
        let _ = std::fs::remove_dir_all(self.repo.parent().unwrap());
    }
}

#[gpui::test]
fn extracts_and_inlines_with_the_keyboard(cx: &mut TestAppContext) {
    let _engine = crate::tests::engine_lock();
    let Some(mut t) = start(cx, "java-refactor", "Shop.java") else { return };
    let original = t.text();
    let editor = t.editor.clone();
    let renaming = |cx: &mut VisualTestContext| cx.update(|_, cx| editor.read(cx).rename.is_some());

    // Nothing selected: Extract Method says what it needs, at the caret.
    caret_at(&editor, "double sum", 0, 0, t.cx);
    t.cx.simulate_keystrokes("cmd-alt-m");
    let hint = t.cx.update(|_, cx| editor.read(cx).hint.clone());
    assert!(hint.as_deref().is_some_and(|h| h.contains("select")), "{hint:?}");
    assert!(t.cx.debug_bounds("editor-hint").is_some(), "the note shows at the caret");

    // Extract Method (⌥⌘M) on the loop: a new method, and its name offered for renaming.
    // The engine may still be indexing right after the start: ask again until it answers.
    let mut extracted = false;
    for _ in 0..10 {
        t.select("for (Shape s : shapes) {\n            sum += s.area();\n        }", 0);
        t.cx.simulate_keystrokes("cmd-alt-m");
        if pump_until(t.cx, 6, |cx| renaming(cx)) {
            extracted = true;
            break;
        }
    }
    assert!(extracted, "Extract Method asks for the new name: {}", t.text());
    let made = t.text();
    assert!(made.contains("private double extracted(") || made.contains("private static double"), "{made}");
    // The name is selected in the field: type another and ↩ renames it everywhere.
    t.cx.simulate_input("sumOfAreas");
    t.cx.simulate_keystrokes("enter");
    assert!(
        t.wait_for_text(30, |s| s.contains("double sumOfAreas(") && s.contains("sum = sumOfAreas(")),
        "{}",
        t.text()
    );
    t.put_back(&original);

    // Extract Variable (⌥⌘V) on an expression used once: no question, a local, its name.
    t.select("shop.total() * 1.25", 0);
    t.cx.simulate_keystrokes("cmd-alt-v");
    assert!(pump_until(t.cx, 30, |cx| renaming(cx)), "Extract Variable asks for the name");
    assert!(t.text().contains("double "), "{}", t.text());
    t.cx.simulate_keystrokes("escape");
    t.put_back(&original);

    // An expression used twice: a menu asks which to replace.
    let twice = original.replace("int tax = 7;", "int tax = 7 * 6;\n        int fee = 7 * 6;");
    t.put_back(&twice);
    t.select("7 * 6", 0);
    t.cx.simulate_keystrokes("cmd-alt-v");
    let asked = pump_until(t.cx, 30, |cx| cx.update(|_, cx| editor.read(cx).choices.is_some()));
    assert!(asked, "two ways: this occurrence or both");
    assert!(t.cx.debug_bounds("refactor-choices").is_some());
    let titles: Vec<String> =
        t.cx.update(|_, cx| editor.read(cx).choices.as_ref().unwrap().items.iter().map(|a| a.title.clone()).collect());
    assert!(titles.iter().any(|x| x.contains("all occurrences")), "{titles:?}");
    let all = titles.iter().position(|x| x.contains("all occurrences")).unwrap();
    for _ in 0..all {
        t.cx.simulate_keystrokes("down");
    }
    t.cx.simulate_keystrokes("enter");
    assert!(pump_until(t.cx, 10, |cx| renaming(cx)), "the pick is applied and named");
    let both = t.text();
    t.cx.simulate_keystrokes("escape");
    assert_eq!(both.matches("7 * 6").count(), 1, "{both}");
    t.put_back(&original);

    // Extract Constant (⌥⌘C) on a literal: a static final field.
    t.select("1.25", 0);
    t.cx.simulate_keystrokes("cmd-alt-c");
    assert!(pump_until(t.cx, 30, |cx| renaming(cx)), "Extract Constant asks for the name");
    assert!(t.text().contains("static final double"), "{}", t.text());
    t.cx.simulate_keystrokes("escape");
    t.put_back(&original);

    // Extract Field (⌥⇧⌘F) on an expression of a method: a field.
    t.select("name + \": \" + count", 0);
    t.cx.simulate_keystrokes("cmd-alt-shift-f");
    let field = pump_until(t.cx, 30, |cx| renaming(cx));
    assert!(field, "Extract Field asks for the name: {}", t.text());
    assert!(t.text().contains("private static String"), "{}", t.text());
    t.cx.simulate_keystrokes("escape");
    t.put_back(&original);

    // Inline (⌥⌘N) on a local variable: its value in its place.
    caret_at(&editor, "tax = 7", 0, 1, t.cx);
    t.cx.simulate_keystrokes("cmd-alt-n");
    assert!(t.wait_for_text(30, |s| !s.contains("int tax") && s.contains("* 1.25 + 7")), "{}", t.text());
    assert!(!renaming(t.cx), "inlining names nothing");
    t.put_back(&original);

    // The commands are in the palette and the menus.
    let names: Vec<String> = crate::menus::commands(&[]).into_iter().map(|(n, _)| n.to_string()).collect();
    for name in ["Extract Method", "Extract Variable", "Extract Constant", "Extract Field", "Inline"] {
        assert!(names.iter().any(|n| n == name), "{name} in the palette");
    }
    let _ = &t.model;
    t.stop();
}

#[gpui::test]
fn formats_the_selection_or_else_the_file(cx: &mut TestAppContext) {
    let _engine = crate::tests::engine_lock();
    let Some(mut t) = start(cx, "java-format", "Shop.java") else { return };
    let original = t.text();
    let messy = original
        .replace("        double sum = 0;", "double   sum =  0;")
        .replace("        int tax = 7;", "int   tax=7;");
    t.put_back(&messy);

    // ⌥⌘L with a line selected: that line only.
    let mut formatted = false;
    for _ in 0..10 {
        t.select("double   sum =  0;", 0);
        t.cx.simulate_keystrokes("cmd-alt-l");
        if t.wait_for_text(5, |s| s.contains("        double sum = 0;")) {
            formatted = true;
            break;
        }
    }
    assert!(formatted, "the selected line is formatted: {}", t.text());
    assert!(t.text().contains("int   tax=7;"), "the rest is left as it was");

    // Nothing selected: the whole file.
    let editor = t.editor.clone();
    caret_at(&editor, "package", 0, 0, t.cx);
    t.cx.simulate_keystrokes("cmd-alt-l");
    assert!(t.wait_for_text(30, |s| s.contains("        int tax = 7;")), "{}", t.text());
    let names: Vec<String> = crate::menus::commands(&[]).into_iter().map(|(n, _)| n.to_string()).collect();
    assert!(names.iter().any(|n| n == "Format Selection"));
    t.stop();
}

/// The parameter hints of the editor, as (name, argument).
fn param_hints(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> Vec<(String, String)> {
    cx.update(|_, cx| {
        let e = editor.read(cx);
        let text = e.text(cx);
        e.param_hints
            .iter()
            .map(|h| {
                let start = text[..h.at].rfind('\n').map_or(0, |i| i + 1);
                let end = text[h.at..].find('\n').map_or(text.len(), |i| h.at + i);
                (h.name.clone(), crate::editor::argument_at(&text[start..end], h.at - start))
            })
            .collect()
    })
}

#[gpui::test]
fn parameter_names_of_literals_show_and_follow_the_text(cx: &mut TestAppContext) {
    let _engine = crate::tests::engine_lock();
    let Some(mut t) = start(cx, "java-hints", "Shop.java") else { return };
    let editor = t.editor.clone();
    let expected: Vec<(String, String)> = [
        ("owner", "\"Ada\""),
        ("radius", "2.0"),
        ("side", "3.0"),
        ("name", "\"Total\""),
        ("count", "42"),
        ("loud", "true"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let e2 = editor.clone();
    let shown = pump_until(t.cx, 60, |cx| param_hints(&e2, cx) == expected);
    assert!(shown, "the hints of the literal arguments: {:?}", param_hints(&editor, t.cx));

    // Typing above them: they move with their arguments at once, before the engine is asked
    // again (nothing blinks), and stay once it answers.
    caret_at(&editor, "package", 0, 0, t.cx);
    t.cx.simulate_input("// A note.\n");
    assert_eq!(param_hints(&editor, t.cx), expected, "moved with the text");
    let e3 = editor.clone();
    pump_until(t.cx, 3, |_| false);
    assert!(pump_until(t.cx, 30, |cx| param_hints(&e3, cx) == expected), "{:?}", param_hints(&editor, t.cx));
    // Typing before an argument moves the hint with it; typing over one drops its hint until
    // the engine answers again.
    caret_at(&editor, "42", 0, 0, t.cx);
    t.cx.simulate_input("4");
    assert!(param_hints(&editor, t.cx).iter().any(|(n, a)| n == "count" && a == "442"));
    t.select("442", 0);
    t.cx.simulate_input("9");
    assert_eq!(param_hints(&editor, t.cx).len(), expected.len() - 1);
    let e4 = editor.clone();
    assert!(pump_until(t.cx, 30, |cx| param_hints(&e4, cx).iter().any(|(n, a)| n == "count" && a == "9")));

    // The setting off (View ▸ Parameter Name Hints): they go, and stay gone while typing.
    t.cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::ToggleParamHints), cx));
    t.cx.run_until_parked();
    assert!(t.cx.update(|_, cx| !t.model.read(cx).settings.param_hints));
    assert!(param_hints(&editor, t.cx).is_empty());
    t.cx.simulate_input("2");
    pump_until(t.cx, 3, |_| false);
    assert!(param_hints(&editor, t.cx).is_empty());
    // And on again.
    t.cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::ToggleParamHints), cx));
    let e5 = editor.clone();
    assert!(pump_until(t.cx, 30, |cx| !param_hints(&e5, cx).is_empty()));
    t.stop();
}

#[gpui::test]
fn the_structure_window_and_popup_show_the_files_symbols(cx: &mut TestAppContext) {
    use crate::lang::symbols::{Sort, Visibility};
    let _engine = crate::tests::engine_lock();
    let Some(t) = start(cx, "java-structure", "Shop.java") else { return };
    let (ws, editor, cx) = (t.ws.clone(), t.editor.clone(), &mut *t.cx);
    let structure = cx.update(|_, cx| ws.read(cx).structure.clone());
    caret_at(&editor, "sum += s.area()", 0, 0, cx);
    cx.simulate_keystrokes("cmd-7");
    let open = cx.update(|_, cx| ws.read(cx).tool_open(crate::workspace::Tool::Structure, cx));
    assert!(open, "⌘7 opens the Structure window");
    let names = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| structure.read(cx).rows(cx).iter().map(|r| r.symbol.name.clone()).collect::<Vec<_>>())
    };
    let s2 = structure.clone();
    let listed = pump_until(cx, 60, |cx| cx.update(|_, cx| s2.read(cx).rows(cx).len() >= 9));
    assert!(listed, "the symbols are listed: {:?}", names(cx));
    let rows = cx.update(|_, cx| structure.read(cx).rows(cx));
    let row = |name: &str| {
        rows.iter().find(|r| r.symbol.name.starts_with(name)).unwrap_or_else(|| panic!("{name}: {rows:?}"))
    };
    assert_eq!(row("Shop").depth, 0);
    assert_eq!(row("shapes").symbol.visibility, Visibility::Private);
    assert_eq!(row("owner").symbol.visibility, Visibility::Protected);
    assert_eq!(row("visits").symbol.visibility, Visibility::Package);
    assert_eq!(row("add").symbol.visibility, Visibility::Public);
    let label = row("label");
    assert!(label.symbol.visibility == Visibility::Private && label.symbol.is_static);
    assert_eq!(row("total").depth, 1);
    assert!(cx.debug_bounds("structure-row-0").is_some(), "the tree shows");
    // The caret is in total(): its row is picked out, and follows the caret.
    let current = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let s = structure.read(cx);
            let path = s.current.clone()?;
            s.rows(cx).into_iter().find(|r| r.path == path).map(|r| r.symbol.name)
        })
    };
    assert!(current(cx).is_some_and(|n| n.starts_with("total")), "{:?}", current(cx));
    caret_at(&editor, "shop.add(new Circle", 0, 0, cx);
    assert!(current(cx).is_some_and(|n| n.starts_with("main")), "{:?}", current(cx));
    // Sorted by name, types first; then back in the file's order.
    cx.update(|_, cx| structure.update(cx, |s, cx| s.set_sort(Sort::Name, cx)));
    let sorted = names(cx);
    assert!(sorted[1].starts_with("add") && sorted.last().unwrap().starts_with("visits"), "{sorted:?}");
    cx.update(|_, cx| structure.update(cx, |s, cx| s.set_sort(Sort::Position, cx)));
    // The filter keeps what matches and what holds it.
    let filter = cx.update(|_, cx| structure.read(cx).filter.clone());
    cx.update(|w, cx| filter.update(cx, |f, cx| f.set_value("tot", w, cx)));
    cx.run_until_parked();
    let found = names(cx);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[1].starts_with("total"));
    // A click goes there.
    let at = cx.debug_bounds("structure-row-1").expect("the row").center();
    cx.simulate_click(at, gpui::Modifiers::none());
    cx.run_until_parked();
    let line = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    assert_eq!(line, 22, "on public double total()");
    cx.update(|w, cx| filter.update(cx, |f, cx| f.set_value("", w, cx)));

    // ⌘F12: the same tree over the editor, on the symbol the caret is in; ↩ goes there.
    caret_at(&editor, "String text", 0, 0, cx);
    cx.simulate_keystrokes("cmd-f12");
    let popup = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            ws.read(cx).search.as_ref().is_some_and(|s| {
                let s = s.read(cx);
                s.mode == crate::search::Mode::Structure && s.items.len() >= 9
            })
        })
    });
    assert!(popup, "⌘F12 lists the file's symbols");
    let picked = cx.update(|_, cx| {
        let s = ws.read(cx).search.clone().unwrap();
        let s = s.read(cx);
        (s.items[s.selected].path.clone(), s.items.iter().map(|i| i.depth).max())
    });
    assert!(picked.0.starts_with("label"), "the caret's method is picked: {picked:?}");
    assert_eq!(picked.1, Some(1), "nested");
    cx.simulate_input("main");
    let narrowed = pump_until(cx, 10, |cx| {
        cx.update(|_, cx| ws.read(cx).search.as_ref().is_some_and(|s| s.read(cx).items.len() == 2))
    });
    assert!(narrowed);
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    let line = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    assert_eq!(line, 35, "on main");
    t.stop();
}
