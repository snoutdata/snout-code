//! Headless tests of the Java features against the real engine: the small Gradle build of
//! `engine/lsp-driver`'s fixtures, opened in a test window. They need the engine
//! distribution (`engine/build/dist`) and a JDK 17 (`SNOUT_TEST_JDK17`, or one installed
//! where JDKs usually are); without them they say so and pass.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};

use crate::editor::EditorPanel;
use crate::lang::java;
use crate::model::Action;
use crate::panels::Activity;
use crate::tests::{act, git, open, scratch_dir, with_repo};
use crate::workspace::Workspace;

/// The JDK 17 the fixture builds with, if this machine has one.
pub(crate) fn jdk17() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("SNOUT_TEST_JDK17").map(PathBuf::from) {
        return java::jdk_at(&*snout_host::local(), &home)
            .or_else(|| java::jdk_at(&*snout_host::local(), &home.join("Contents/Home")))
            .map(|j| j.home);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    java::discover(&*snout_host::local(), &home).into_iter().find(|j| j.major == 17).map(|j| j.home)
}

/// The fixture as a repository: `java-app/project` is the build, `java-app/shared` a source
/// folder outside it.
fn fixture(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/lsp-driver/src/test/fixtures/java-app");
    crate::tests::copy_tree(&from, &dir);
    let repo = dir.join("java-app");
    // The Gradle daemon the sync starts goes away soon after the test.
    std::fs::write(repo.join("project/gradle.properties"), "org.gradle.daemon.idletimeout=60000\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.name=t", "-c", "user.email=t@example.com", "add", "-A"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    repo
}

/// Let real time pass (the server works in other processes) while the app's tasks run.
pub(crate) fn pump_until(
    cx: &mut VisualTestContext,
    secs: u64,
    mut done: impl FnMut(&mut VisualTestContext) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        if done(cx) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    false
}

/// The workspace under test, for the helpers.
pub(crate) struct TestWorkspace(pub Entity<Workspace>);

impl gpui::Global for TestWorkspace {}

pub(crate) fn editor_of(
    ws: &Entity<Workspace>,
    path: &Path,
    cx: &mut VisualTestContext,
) -> Option<Entity<EditorPanel>> {
    cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path))
}

pub(crate) fn text(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).text(cx))
}

/// Bring the editor's tab forward and put the caret at the `nth` match of `find` (plus
/// `offset` bytes).
pub(crate) fn caret_at(
    editor: &Entity<EditorPanel>,
    find: &str,
    nth: usize,
    offset: usize,
    cx: &mut VisualTestContext,
) {
    cx.update(|window, cx| {
        let ws = cx.global::<TestWorkspace>().0.clone();
        let id = gpui_component::dock::PanelId::from(editor.entity_id());
        ws.update(cx, |ws, cx| ws.dock.update(cx, |d, cx| d.select_panel(id, window, cx)));
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let t = editor.read(cx).text(cx);
        let at = t.match_indices(find).nth(nth).map(|(i, _)| i + offset).expect("the text is there");
        editor.update(cx, |e, cx| {
            e.state.update(cx, |s, cx| {
                s.set_selected_range(at..at, cx);
                s.focus(window, cx);
            })
        });
    });
    cx.run_until_parked();
}

#[gpui::test]
fn java_files_get_the_language_features_of_the_engine(cx: &mut TestAppContext) {
    let (Some(_), Some(jdk)) = (java::engine_dist(&*snout_host::local()), jdk17()) else {
        println!("java features: skipped; needs engine/build/dist and a JDK 17 (SNOUT_TEST_JDK17)");
        return;
    };
    // The server answers from other processes, through reader threads.
    cx.executor().allow_parking();
    let _engine = crate::tests::engine_lock();
    let repo = fixture("lang-java");
    let build = repo.join("project");
    let mut settings = with_repo(&repo);
    // One engine serves every build of the opened folder.
    settings.project_jdks.insert(repo.clone(), jdk.clone());
    settings.project_jdks.insert(build.clone(), jdk);
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, settings);
    cx.update(|_, cx| cx.set_global(TestWorkspace(ws.clone())));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let app = build.join("app/src/main/java/demo/app/App.java");
    act(&model, cx, Action::OpenFile(app.clone(), None));
    let editor = editor_of(&ws, &app, cx).expect("App.java opens");
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());

    // The engine imports the build, and the file gets its (empty) diagnostics.
    let started = Instant::now();
    let ready = pump_until(cx, 240, |cx| {
        cx.update(|_, cx| hub.read(cx).servers.get(&repo).is_some_and(|s| s.first_diagnostics.is_some()))
    });
    assert!(ready, "the server answers within four minutes");
    println!("java features: first diagnostics after {:.1} s", started.elapsed().as_secs_f32());
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).java_status(cx)).expect("a Java status");
    assert!(status.0.starts_with("Java: "), "{status:?}");
    assert!(cx.update(|_, cx| editor.read(cx).problems.is_empty()), "the fixture compiles");

    // Hover over Lib.hello: the server's description of it.
    cx.update(|_, cx| ws.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    let hello = cx.update(|_, cx| {
        let s = editor.read(cx).state.read(cx);
        let t = s.text().to_string();
        let at = t.find("hello(name)").unwrap() + 2;
        s.range_to_bounds(&(at..at + 1)).map(|b| b.center())
    });
    let hello = hello.expect("the line is laid out");
    let hovered = pump_until(cx, 30, |cx| {
        cx.simulate_mouse_move(hello, None, Modifiers::none());
        cx.update(|_, cx| editor.read(cx).hover.card.as_ref().is_some_and(|c| c.info.is_some()))
    });
    assert!(hovered, "a hover shows");
    let hover = cx.update(|_, cx| editor.read(cx).hover.card.as_ref().unwrap().info.clone().unwrap());
    assert!(hover.contains("hello"), "{hover}");

    // Completion after `Lib.`.
    let original = text(&editor, cx);
    caret_at(&editor, "System.out", 0, 0, cx);
    cx.simulate_input("Lib.");
    let completed = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            editor.read(cx).state.read(cx).completion_menu_state().items.iter().any(|i| i.label.starts_with("hello"))
        })
    });
    assert!(completed, "completion offers Lib.hello");
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.set_value(original.clone(), window, cx)))
    });
    cx.run_until_parked();

    // Accepting a completion of a class not yet imported adds its import.
    caret_at(&editor, "        System.out", 0, 0, cx);
    cx.simulate_input("Object list = new ArrayLis");
    let offered = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            let s = editor.read(cx).state.read(cx);
            s.completion_menu_state().open
                && s.completion_menu_state().items.first().is_some_and(|i| i.label.starts_with("ArrayList"))
        })
    });
    let labels = cx.update(|_, cx| {
        editor
            .read(cx)
            .state
            .read(cx)
            .completion_menu_state()
            .items
            .iter()
            .take(5)
            .map(|i| i.label.clone())
            .collect::<Vec<_>>()
    });
    assert!(offered, "ArrayList is offered first: {labels:?}");
    cx.simulate_keystrokes("enter");
    let imported = pump_until(cx, 30, |cx| text(&editor, cx).contains("import java.util.ArrayList;"));
    assert!(imported, "{}", text(&editor, cx));
    cx.update(|window, cx| {
        editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.set_value(original.clone(), window, cx)))
    });
    cx.run_until_parked();

    // Signature help inside greet(.
    caret_at(&editor, "greet(\"world\")", 0, 6, cx);
    cx.update(|_, cx| editor.update(cx, |e, cx| e.ask_signature(cx)));
    let signed = pump_until(cx, 30, |cx| cx.update(|_, cx| editor.read(cx).signature.is_some()));
    assert!(signed, "signature help for greet");

    // Go to definition (F12) opens Lib.java.
    caret_at(&editor, "Lib.hello", 0, 5, cx);
    cx.simulate_keystrokes("f12");
    let lib = build.join("lib/src/main/java/demo/lib/Lib.java");
    let went = pump_until(cx, 30, |cx| editor_of(&ws, &lib, cx).is_some());
    assert!(went, "F12 opens the definition in its file");

    // Find references (⇧F12) of the Greeting class, from App.java; right after the
    // start the index may still be filling, so ask again until it answers.
    assert!(editor_of(&ws, &lib, cx).is_some());
    let refs = cx.update(|_, cx| ws.read(cx).references.clone());
    let mut found = false;
    for _ in 0..20 {
        caret_at(&editor, "Greeting greet", 0, 2, cx);
        cx.simulate_keystrokes("shift-f12");
        if pump_until(cx, 3, |cx| cx.update(|_, cx| refs.read(cx).len() >= 2)) {
            found = true;
            break;
        }
    }
    assert!(found, "references are listed");

    // An error: a squiggle, a Problems row, and a quick fix.
    let bad = cx.update(|_, cx| editor.read(cx).state.read(cx).text().to_string());
    caret_at(&editor, "    public static void main", 0, 0, cx);
    cx.simulate_input("    int broken = \"text\";\n");
    let flagged = pump_until(cx, 60, |cx| cx.update(|_, cx| !editor.read(cx).problems.is_empty()));
    assert!(flagged, "the type error is reported");
    let problems = cx.update(|_, cx| ws.read(cx).problems.read(cx).counts(cx));
    assert!(problems.0 >= 1, "the Problems tab counts it: {problems:?}");
    let squiggles = cx.update(|_, cx| editor.read(cx).state.read(cx).diagnostics().map_or(0, |d| d.len()));
    assert!(squiggles >= 1);
    caret_at(&editor, "\"text\"", 0, 1, cx);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-."));
    let fixes =
        pump_until(cx, 30, |cx| {
            cx.update(|_, cx| {
                editor.read(cx).hover.actions.as_ref().is_some_and(|l| {
                    l.entries.iter().any(|e| matches!(e, crate::editor::context_actions::Entry::Fix(_)))
                })
            })
        });
    assert!(fixes, "quick fixes are offered");
    cx.simulate_keystrokes("escape");
    // The Problems tab lists it under its file, and a double click goes to it.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_problems(w, cx)));
    cx.run_until_parked();
    caret_at(&editor, "package", 0, 0, cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_problems(w, cx)));
    cx.run_until_parked();
    let row = cx.debug_bounds("problem-row-1").expect("a Problems row");
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
    let line = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    let bad_line = cx.update(|_, cx| editor.read(cx).problems[0].range.start.line);
    assert_eq!(line, bad_line, "the click put the caret on the problem");
    // Put the file back as it was.
    cx.update(|window, cx| editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.set_value(bad.clone(), window, cx))));
    cx.run_until_parked();

    // Rename greet (F2) everywhere.
    caret_at(&editor, "greet(String", 0, 1, cx);
    cx.simulate_keystrokes("f2");
    assert!(cx.update(|_, cx| editor.read(cx).rename.is_some()), "F2 asks for the new name");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-a"));
    cx.simulate_input("welcome");
    cx.simulate_keystrokes("enter");
    let renamed = pump_until(cx, 30, |cx| {
        let t = text(&editor, cx);
        t.contains("welcome(String") && t.contains("welcome(\"world\")")
    });
    assert!(renamed, "{}", text(&editor, cx));

    // Format document and organize imports.
    caret_at(&editor, "private App()", 0, 0, cx);
    cx.simulate_input("     ");
    caret_at(&editor, "import demo.lib.Lib;", 0, 0, cx);
    cx.simulate_input("import java.util.List;\n");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("shift-alt-o"));
    let organized = pump_until(cx, 30, |cx| !text(&editor, cx).contains("java.util.List"));
    assert!(organized, "the unused import goes");
    cx.simulate_keystrokes(&crate::keymap::platform_keys("shift-alt-f"));
    let formatted = pump_until(cx, 30, |cx| text(&editor, cx).contains("\n    private App()"));
    assert!(formatted, "{}", text(&editor, cx));

    // Symbols: the workspace's (⌘T) and the file's (⇧⌘O).
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-t"));
    cx.simulate_input("Greeting");
    let listed = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            ws.read(cx).search.as_ref().is_some_and(|s| s.read(cx).items.iter().any(|i| i.path == "Greeting"))
        })
    });
    assert!(listed, "⌘T finds the class");
    cx.simulate_keystrokes("escape");
    caret_at(&editor, "class App", 0, 0, cx);
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-shift-o"));
    let file_symbols = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            ws.read(cx).search.as_ref().is_some_and(|s| s.read(cx).items.iter().any(|i| i.path.starts_with("main")))
        })
    });
    let seen = cx.update(|_, cx| {
        ws.read(cx)
            .search
            .as_ref()
            .map(|s| (s.read(cx).mode, s.read(cx).items.iter().map(|i| i.path.clone()).collect::<Vec<_>>()))
    });
    assert!(file_symbols, "⇧⌘O lists the file's members: {seen:?}");
    cx.simulate_keystrokes("escape");

    // Into the JDK: String opens as a read only class file tab.
    caret_at(&editor, "String name", 0, 1, cx);
    cx.simulate_keystrokes("f12");
    let class_tab = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| {
            ws.read(cx).editors(cx).iter().any(|e| {
                let e = e.read(cx);
                e.class_file.is_some() && e.text(cx).contains("class String")
            })
        })
    });
    assert!(class_tab, "F12 on a JDK class shows it");

    // Quitting stops the server.
    let pids: Vec<u32> = cx.update(|_, cx| hub.update(cx, |h, _| h.server_pids()));
    cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
    let gone = pids.iter().all(|p| {
        !std::process::Command::new("kill")
            .arg("-0")
            .arg(p.to_string())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    });
    assert!(gone, "the engine stopped");
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}

#[gpui::test]
fn a_build_needing_a_missing_jdk_offers_to_download_it(cx: &mut TestAppContext) {
    let repo = fixture("lang-nojdk");
    let empty_home = scratch_dir("lang-nojdk-home");
    // Servers stay off until the hub looks for JDKs in an empty home.
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    // The opened folder's engine, which syncs the build.
    let build = repo.clone();
    cx.update(|_, cx| {
        hub.update(cx, |h, cx| {
            h.home = empty_home.clone();
            h.enabled = true;
            h.ensure_server(&build, cx);
        })
    });
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    cx.run_until_parked();
    let prompt = cx.update(|_, cx| hub.read(cx).jdk_prompt.clone()).expect("the bar asks");
    assert_eq!(prompt.major, 17, "the build declares Java 17");
    assert!(cx.debug_bounds("jdk-bar").is_some(), "the bar shows over the docks");
    let status = cx.update(|_, cx| hub.read(cx).servers.get(&build).map(|s| s.status()));
    assert_eq!(status.as_deref(), Some("Java: Needs JDK 17"));
    // Not now puts it away.
    let dismiss = cx.debug_bounds("jdk-dismiss").expect("a Not now button").center();
    cx.simulate_click(dismiss, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(|_, cx| hub.read(cx).jdk_prompt.is_none()));
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
    let _ = std::fs::remove_dir_all(empty_home);
}

/// The engine's `snout/readiness`, without an engine: the status bar's background task and
/// its list of modules, the open file's module in the inspections widget until it is ready,
/// and `snout/modelChanged` keeping the Project view's model while it reads the new one.
#[gpui::test]
fn readiness_shows_in_the_status_bar_and_the_editor(cx: &mut TestAppContext) {
    use crate::project_model::{Module, ProjectModel, RootKind, SourceRoot};
    use serde_json::json;
    let repo = crate::tests::demo_repo("readiness");
    let src = repo.join("app/src/main/java/demo");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(repo.join("settings.gradle"), "include 'app'\n").unwrap();
    std::fs::write(repo.join("app/build.gradle"), "plugins { id 'java' }\n").unwrap();
    let file = src.join("App.java");
    std::fs::write(&file, "package demo;\n\nclass App {}\n").unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let module = |id: &str, dir: &str| Module {
        id: id.into(),
        name: id.rsplit(':').next().unwrap().into(),
        build: "readiness".into(),
        gradle_path: format!(":{}", id.rsplit(':').next().unwrap()),
        content_root: repo.join(dir),
        roots: vec![SourceRoot { dir: repo.join(dir).join("src/main/java"), kind: RootKind::Main, exists: true }],
        excluded: Vec::new(),
        uses: Vec::new(),
        jdk: None,
        stale: None,
        maven: false,
    };
    let project = ProjectModel {
        root: repo.clone(),
        name: "readiness".into(),
        modules: vec![module("readiness:app", "app"), module("readiness:lib", "lib")],
        ..Default::default()
    };
    let store = cx.update(|_, cx| crate::project_model::store(cx)).unwrap();
    cx.update(|_, cx| store.update(cx, |s, cx| s.set_model(project, cx)));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    cx.update(|_, cx| hub.update(cx, |h, _| h.insert_published(&repo, String::new(), Vec::new())));
    act(&model, cx, Action::OpenFile(file.clone(), Some((0, 0))));
    let editor = editor_of(&ws, &file, cx).unwrap();
    let say = |params: serde_json::Value, cx: &mut VisualTestContext| {
        cx.update(|_, cx| hub.update(cx, |h, cx| h.engine_says(&repo, "snout/readiness", params, cx)));
        cx.run_until_parked();
    };
    say(json!({ "module": "readiness:app", "stage": "compilingScala", "percent": 40, "millis": 900 }), cx);
    say(json!({ "module": "readiness:lib", "stage": "ready", "percent": 100, "millis": 900 }), cx);
    say(
        json!({ "module": null, "stage": "compilingScala", "percent": 70, "modules": 2, "ready": 1, "millis": 900 }),
        cx,
    );
    // The status bar: the project's stage and percent, and each module's in the list.
    let tasks = cx.update(|_, cx| ws.read(cx).status.read(cx).tasks(cx));
    let java = tasks.iter().find(|t| t.title.starts_with("Java: ")).expect("the engine's task");
    assert_eq!(java.compact.as_deref(), Some("Java: 1 of 2 modules ready"));
    assert_eq!(java.stage, "Compiling Scala: 1 of 2 modules ready");
    assert_eq!(java.percent, Some(70));
    assert_eq!(
        java.parts,
        [
            ("readiness:app".to_owned(), "Compiling Scala".to_owned(), Some(40)),
            ("readiness:lib".to_owned(), "Ready".to_owned(), None)
        ]
    );
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).java_status(cx)).unwrap();
    assert_eq!(status.0, "Java: 1 of 2 modules ready");
    let status_bar = cx.update(|_, cx| ws.read(cx).status.clone());
    cx.update(|_, cx| status_bar.update(cx, |s, _| s.tasks_open = true));
    cx.update(|_, cx| ws.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    assert!(cx.debug_bounds("task-parts").is_some(), "the popover lists the modules");
    // The open file's module, in the inspections widget, until it is ready.
    let pending = cx.update(|_, cx| editor.read(cx).module_readiness(cx)).expect("the module is not ready");
    assert_eq!((pending.label(), pending.percent), ("Compiling Scala", Some(40)));
    assert!(cx.debug_bounds("inspections-pending").is_some());
    say(json!({ "module": "readiness:app", "stage": "ready", "percent": 100, "millis": 2000 }), cx);
    say(json!({ "module": null, "stage": "ready", "percent": 100, "modules": 2, "ready": 2, "millis": 2000 }), cx);
    assert!(cx.update(|_, cx| editor.read(cx).module_readiness(cx)).is_none());
    assert!(cx.debug_bounds("inspections-pending").is_none(), "the problems again");
    assert!(cx.update(|_, cx| !ws.read(cx).status.read(cx).tasks(cx).iter().any(|t| t.title.starts_with("Java: "))));
    // A background sync changed the model: the one shown stays until the new one is read.
    cx.update(|_, cx| hub.update(cx, |h, cx| h.engine_says(&repo, "snout/modelChanged", json!({}), cx)));
    cx.run_until_parked();
    assert!(cx.update(|_, cx| store.read(cx).current.is_some()), "the Project view keeps its model");
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn readiness_reads_the_answer_and_the_notifications() {
    use crate::lang::hub::Readiness;
    use serde_json::json;
    let r = Readiness::from_answer(&json!({ "stage": "importing", "percent": 50.4, "modules": [
        { "module": "a", "stage": "ready", "percent": 100 },
        { "module": "core", "stage": "importing", "percent": 20, "shared": true } ] }));
    assert_eq!((r.total, r.ready), (2, 1));
    assert!(r.pending());
    assert_eq!(r.project.as_ref().unwrap().percent, Some(50));
    assert!(r.modules["core"].shared);
    assert_eq!(r.summary(), "1 of 2 modules ready");
    let mut r = Readiness::default();
    assert!(r.apply(&json!({ "module": null, "stage": "syncing", "percent": 0, "modules": 0, "ready": 0 })));
    assert_eq!(r.summary(), "Syncing the build model");
    assert!(!r.apply(&json!({ "module": null, "stage": "syncing", "percent": 0, "modules": 0, "ready": 0 })));
    assert_eq!(crate::lang::java::Requirement { declared: None, gradle_max: Some(19) }.wanted(), 17);
    assert_eq!(crate::lang::java::Requirement { declared: None, gradle_max: Some(16) }.wanted(), 11);
}
