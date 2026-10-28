//! Running, testing and debugging, headless: the run windows over sessions fed the engine's
//! notifications, and (with the engine distribution and a JDK 17, as `lang_tests` needs)
//! the engine's `run-app` fixture run, tested and debugged end to end in a test window.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use serde_json::json;

use crate::lang::java;
use crate::model::Action;
use crate::panels::Activity;
use crate::run::config::Kind;
use crate::run::test_tree::TestState;
use crate::run::{Launch, Outcome, Place, Runs, Target};
use crate::tests::{act, demo_repo, git, open, scratch_dir, with_repo};
use crate::workspace::Workspace;

fn runs_of(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Entity<Runs> {
    cx.update(|_, cx| ws.read(cx).runs.clone())
}

#[gpui::test]
fn sessions_take_the_engines_notifications(cx: &mut TestAppContext) {
    let repo = demo_repo("run-sessions");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = runs_of(&ws, cx);
    let launch = Launch::Tests {
        module: "app".into(),
        filter: json!({ "kind": "class", "class": "demo.CalcTest" }),
        label: "CalcTest".into(),
        debug: false,
        rerun_failed: false,
    };
    let id = cx.update(|_, cx| runs.update(cx, |r, cx| r.adopt(launch, "run-1", cx)));
    let note = |method: &str, params: serde_json::Value, cx: &mut VisualTestContext| {
        cx.update(|_, cx| runs.update(cx, |r, cx| r.engine_notification(method, params, cx)));
    };
    note(
        "snout/taskOutput",
        json!({ "runId": "run-1", "stream": "stdout", "line": "\u{1b}[32mstarting\u{1b}[0m" }),
        cx,
    );
    note("snout/taskProgress", json!({ "runId": "run-1", "task": ":app:test", "event": "started" }), cx);
    for (event, id, parent, kind, name) in [
        ("started", "1", None, "suite", "Gradle Test Run :app:test"),
        ("started", "2", Some("1"), "suite", "CalcTest"),
        ("started", "3", Some("2"), "test", "adds()"),
    ] {
        note(
            "snout/testEvent",
            json!({ "runId": "run-1", "event": event, "id": id, "parent": parent, "kind": kind,
            "name": name, "displayName": name, "className": "demo.CalcTest" }),
            cx,
        );
    }
    note(
        "snout/testEvent",
        json!({ "runId": "run-1", "event": "finished", "id": "3", "parent": "2", "kind": "test",
        "name": "adds()", "outcome": "failed", "durationMillis": 4, "message": "expected: <2> but was: <3>",
        "stackTrace": "org.opentest4j.AssertionFailedError: x\n\tat demo.CalcTest.adds(CalcTest.java:9)" }),
        cx,
    );
    note(
        "snout/taskFinished",
        json!({ "runId": "run-1", "outcome": "failed", "millis": 1200,
        "tests": { "passed": 0, "failed": 1, "skipped": 0 } }),
        cx,
    );
    cx.run_until_parked();
    cx.update(|_, cx| {
        let r = runs.read(cx);
        let s = r.session(id).unwrap();
        assert_eq!(s.lines[1].text(), "starting");
        assert_eq!(s.tasks[0].path, ":app:test");
        let tree = s.tests.as_ref().unwrap();
        assert_eq!(tree.counts(), (0, 1, 0, 0));
        assert_eq!(s.selected_test.as_deref(), Some("3"), "the failure is picked");
        assert_eq!(s.ended.as_ref().unwrap().outcome, Outcome::Failed);
        assert_eq!(s.test_summary().as_deref(), Some("1 failed, 0 passed, 1 test"));
        assert!(!r.any_running());
    });
    // The Run window shows the tree, the failure with its diff, and Rerun Failed.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_bottom(ws.tabs.run, w, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("test-tree").is_some(), "the test tree shows");
    assert!(cx.debug_bounds("test-diff").is_some(), "the assertion's diff shows");
    assert!(cx.debug_bounds("run-rerun-failed").is_some());
    // Build problems go to Problems.
    note(
        "snout/buildProblems",
        json!({ "runId": "run-1", "problems": [
        { "file": repo.join("src/main.rs"), "line": 2, "column": 5, "severity": "error", "message": "boom" }] }),
        cx,
    );
    cx.update(|_, cx| {
        let all = ws.read(cx).hub.read(cx).all_problems();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1.message, "boom");
        assert_eq!(all[0].1.range.start.line, 1);
    });
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn breakpoints_are_kept_and_drawn_and_the_gutter_offers_runs(cx: &mut TestAppContext) {
    let repo = demo_repo("run-breakpoints");
    std::fs::create_dir_all(repo.join("src/main/java/demo")).unwrap();
    let file = repo.join("src/main/java/demo/App.java");
    std::fs::write(
        &file,
        "package demo;\n\npublic class App {\n    public static void main(String[] args) {\n        int x = 1;\n        System.out.println(x);\n    }\n}\n",
    )
    .unwrap();
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let runs = runs_of(&ws, cx);
    act(&model, cx, Action::OpenFile(file.clone(), Some((4, 0))));
    cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, 4, cx)));
    cx.update(|_, cx| runs.update(cx, |r, cx| r.set_condition(&file, 5, Some("x > 0".into()), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let marks = crate::run::marks(cx).unwrap();
        let m = marks.read(cx);
        assert_eq!(m.breakpoints[&file], [(4, None), (5, Some("x > 0".into()))]);
        assert_eq!(runs.read(cx).breakpoints_in(&file).len(), 2);
    });
    // Clicking the breakpoint lane over the line numbers clears it.
    cx.run_until_parked();
    let editor = cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file).unwrap());
    let at = cx.update(|_, cx| {
        let e = editor.read(cx);
        let lanes = e.lanes.get();
        let s = e.state.read(cx);
        let t = s.text().to_string();
        let off = t.find("int x").unwrap();
        let b = s.range_to_bounds(&(off..off)).unwrap();
        gpui::point(lanes.bounds.left() + gpui::px(20.), b.center().y)
    });
    cx.simulate_click(at, gpui::Modifiers::none());
    cx.update(|_, cx| assert_eq!(runs.read(cx).breakpoints_in(&file).len(), 1, "the click cleared line 5's"));
    // The file's main class is found for the run icon, with no engine.
    let mains = crate::run::main_classes(&file, &std::fs::read_to_string(&file).unwrap());
    assert_eq!(mains, [(3, "demo.App".to_owned())]);
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_welcome_page_lists_recent_projects_and_goes_with_the_first_file(cx: &mut TestAppContext) {
    let a = demo_repo("welcome-a");
    let b = demo_repo("welcome-b");
    let mut settings = crate::settings::Settings { recent: Some(vec![a.clone(), b.clone()]), ..Default::default() };
    settings.opened.insert(a.clone(), 0);
    let (model, ws, cx) = open(cx, settings);
    cx.run_until_parked();
    assert!(cx.debug_bounds("welcome-open").is_some());
    assert!(cx.debug_bounds("welcome-new").is_some());
    assert!(cx.debug_bounds("welcome-recent-1").is_some(), "both projects are listed");
    let welcome = cx.update(|_, cx| ws.read(cx).welcome.clone());
    let search = cx.update(|_, cx| welcome.read(cx).search.clone());
    let name = crate::model::Model::dir_name(&b);
    cx.update(|w, cx| search.update(cx, |s, cx| s.set_value(name.clone(), w, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(welcome.read(cx).shown(cx), std::slice::from_ref(&b)));
    // Open a project and a file: the Welcome tab goes.
    act(&model, cx, Action::OpenRepo(a.clone()));
    act(&model, cx, Action::OpenFile(a.join("README.md"), None));
    cx.update(|_, cx| {
        let ws = ws.read(cx);
        assert!(ws.dock.read(cx).panel(ws.tabs.welcome).is_none(), "the Welcome tab closed");
    });
    let _ = std::fs::remove_dir_all(a);
    let _ = std::fs::remove_dir_all(b);
}

#[gpui::test]
fn a_build_that_needs_a_jdk_offers_it_once(cx: &mut TestAppContext) {
    let repo = demo_repo("needs-jdk");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let model = serde_json::json!({ "modelVersion": 2, "rootDir": repo, "failedBuilds": [
        { "build": "legacy", "rootDir": repo.join("legacy"), "reason": "Needs Java 8", "needsJdk": 8 } ] });
    let parsed = crate::project_model::parse(&model).unwrap();
    assert_eq!(parsed.failed[0].needs_jdk, Some(8));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    cx.update(|_, cx| hub.update(cx, |h, cx| h.offer_jdk(&repo, 8, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let p = hub.read(cx).jdk_prompt.clone().expect("the bar offers JDK 8");
        assert_eq!((p.major, p.resync), (8, true));
    });
    assert!(cx.debug_bounds("jdk-bar").is_some(), "the bar shows");
    cx.update(|_, cx| hub.update(cx, |h, cx| h.dismiss_jdk(cx)));
    cx.update(|_, cx| hub.update(cx, |h, cx| h.offer_jdk(&repo, 8, cx)));
    cx.update(|_, cx| assert!(hub.read(cx).jdk_prompt.is_none(), "not again once turned down"));
    let _ = std::fs::remove_dir_all(repo);
}

// ----- The real engine -------------------------------------------------------------------------

/// A JDK from 17 to 21 for the fixture's Gradle: `SNOUT_TEST_JDK`, one installed where JDKs
/// usually are, or the one snout.sh downloaded to build the engine.
fn test_jdk() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("SNOUT_TEST_JDK").map(PathBuf::from) {
        return java::jdk_at(&home).or_else(|| java::jdk_at(&home.join("Contents/Home"))).map(|j| j.home);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let fits = |j: &java::Jdk| (17..=21).contains(&j.major);
    if let Some(j) = java::discover(&home).into_iter().find(fits) {
        return Some(j.home);
    }
    let built = home.join("Library/Caches/SnoutCode/build-jdk");
    std::fs::read_dir(built)
        .ok()?
        .flatten()
        .filter_map(|e| java::jdk_at(&e.path().join("Contents/Home")).or_else(|| java::jdk_at(&e.path())))
        .find(fits)
        .map(|j| j.home)
}

/// The engine's run-app fixture (`demo.app.Main`, a `Greeter` and its test) as a repository.
fn run_app(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/sync-cli/src/test/fixtures/run-app");
    let ok = std::process::Command::new("cp").arg("-R").arg(&from).arg(&dir).status().unwrap().success();
    assert!(ok, "copy the fixture");
    let repo = dir.join("run-app");
    std::fs::write(repo.join("gradle.properties"), "org.gradle.daemon.idletimeout=60000\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.name=t", "-c", "user.email=t@example.com", "add", "-A"]);
    git(
        &repo,
        &["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "commit", "-qm", "f"],
    );
    repo
}

fn pump_until(cx: &mut VisualTestContext, secs: u64, mut done: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
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

fn output(runs: &Entity<Runs>, id: u64, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        runs.read(cx)
            .session(id)
            .map(|s| s.lines.iter().map(|l| l.text()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    })
}

fn finished(runs: &Entity<Runs>, id: u64, cx: &mut VisualTestContext) -> Option<Outcome> {
    cx.update(|_, cx| runs.read(cx).session(id).and_then(|s| s.ended.as_ref().map(|e| e.outcome)))
}

#[gpui::test]
fn a_fixture_app_runs_tests_and_debugs_in_the_app(cx: &mut TestAppContext) {
    let runs_java = |d: PathBuf| {
        let ok = std::process::Command::new(d.join("runtime/bin/java")).arg("-version").output();
        ok.is_ok_and(|o| o.status.success()).then_some(d)
    };
    let (Some(_), Some(jdk)) = (java::engine_dist().and_then(runs_java), test_jdk()) else {
        println!("run and debug: skipped; needs an engine distribution whose Java runs, and a JDK 17 to 21");
        return;
    };
    cx.executor().allow_parking();
    let _engine = crate::tests::engine_lock();
    let repo = run_app("run-debug");
    let mut settings = with_repo(&repo);
    settings.project_jdks.insert(repo.clone(), jdk);
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, settings);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let runs = runs_of(&ws, cx);
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());

    // The engine imports the build: the configurations and the tasks come.
    let ready = pump_until(cx, 600, |cx| {
        cx.update(|_, cx| {
            let r = runs.read(cx);
            !r.engine_configs.is_empty() && !r.gradle.builds.is_empty()
        })
    });
    assert!(ready, "the configurations and the tasks within ten minutes");
    let configs = cx.update(|_, cx| runs.read(cx).configurations());
    let main = configs.iter().find(|c| c.kind == Kind::Main).cloned().expect("a main configuration");
    assert_eq!(main.main_class.as_deref(), Some("demo.app.Main"));
    assert!(configs.iter().any(|c| c.kind == Kind::Gradle && c.task.as_deref() == Some(":app:run")), "{configs:?}");
    assert!(configs.iter().any(|c| c.kind == Kind::Tests), "{configs:?}");
    let tasks = cx.update(|_, cx| crate::run::gradle_window::task_rows(ws.read(cx).gradle.read(cx), cx));
    assert!(tasks.iter().any(|t| t == "run-app"), "{tasks:?}");
    cx.update(|_, cx| ws.update(cx, |ws, cx| ws.gradle.update(cx, |g, cx| g.reveal("hello", cx))));
    let tasks = cx.update(|_, cx| crate::run::gradle_window::task_rows(ws.read(cx).gradle.read(cx), cx));
    assert!(tasks.iter().any(|t| t == ":app:hello"), "{tasks:?}");

    // A Gradle task, as the Gradle window's double click runs it: the Build window.
    let build = cx
        .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Task { build: None, task: ":app:hello".into() }, cx)))
        .unwrap();
    assert!(pump_until(cx, 300, |cx| finished(&runs, build, cx).is_some()), "the task ends");
    assert_eq!(finished(&runs, build, cx), Some(Outcome::Success), "{}", output(&runs, build, cx));
    assert!(output(&runs, build, cx).contains("hello from the hello task"));
    cx.update(|_, cx| {
        let s = runs.read(cx).session(build).unwrap();
        assert_eq!(s.place(), Place::Build);
        assert!(
            s.tasks.iter().any(|t| t.path == ":app:hello" && t.outcome.as_deref() == Some("success")),
            "{:?}",
            s.tasks
        );
    });

    // The main class runs.
    let run = cx
        .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Config { config: main.clone(), debug: false }, cx)))
        .unwrap();
    assert!(pump_until(cx, 300, |cx| finished(&runs, run, cx).is_some()), "the program ends");
    assert!(output(&runs, run, cx).contains("total 6"), "{}", output(&runs, run, cx));

    // The test class runs from its gutter icon.
    let test = repo.join("app/src/test/java/demo/app/GreeterTest.java");
    act(&model, cx, Action::OpenFile(test.clone(), None));
    let editor = cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == test).unwrap());
    assert!(
        pump_until(cx, 120, |cx| cx.update(|_, cx| editor
            .read(cx)
            .runnables
            .as_ref()
            .is_some_and(|r| r.module.is_some()))),
        "the engine finds the file's tests"
    );
    let (target, module) = cx.update(|_, cx| {
        let r = editor.read(cx).runnables.clone().unwrap();
        let class = r.items.iter().find(|i| matches!(i.target, Target::TestClass { .. })).unwrap().target.clone();
        (class, r.module.clone())
    });
    cx.update(|_, cx| runs.update(cx, |r, cx| r.run_target(&target, module.as_deref(), false, cx)));
    let tests = cx.update(|_, cx| runs.read(cx).sessions.last().unwrap().id);
    assert!(pump_until(cx, 300, |cx| finished(&runs, tests, cx).is_some()), "the tests end");
    cx.update(|_, cx| {
        let s = runs.read(cx).session(tests).unwrap();
        let tree = s.tests.as_ref().unwrap();
        assert_eq!(tree.counts().0, 1, "one test passed: {:?}", tree);
        assert!(tree.nodes.values().any(|n| n.name.contains("greets") && n.state == TestState::Passed));
    });

    // Debug: a breakpoint in the loop stops it, the variables show, a step moves on.
    let file = repo.join("app/src/main/java/demo/app/Main.java");
    act(&model, cx, Action::OpenFile(file.clone(), None));
    let line =
        std::fs::read_to_string(&file).unwrap().lines().position(|l| l.contains("total = add(total, i);")).unwrap()
            as u32;
    cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, line, cx)));
    let debug = cx
        .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Config { config: main.clone(), debug: true }, cx)))
        .unwrap();
    let paused = pump_until(cx, 300, |cx| {
        cx.update(|_, cx| {
            crate::run::marks(cx).unwrap().read(cx).exec.is_some()
                && !crate::run::marks(cx).unwrap().read(cx).locals.is_empty()
        })
    });
    assert!(paused, "the breakpoint is hit: {}", output(&runs, debug, cx));
    cx.update(|_, cx| {
        let m = crate::run::marks(cx).unwrap();
        let m = m.read(cx);
        assert_eq!(m.exec, Some((file.clone(), line)));
        assert!(m.locals.contains(&("i".into(), "1".into())), "{:?}", m.locals);
        assert!(m.locals.contains(&("total".into(), "0".into())), "{:?}", m.locals);
    });
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_bottom(ws.tabs.debug, w, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("debug-frame-0").is_some(), "the frames show");
    assert!(cx.debug_bounds("debug-var-1").is_some(), "the variables show");
    cx.update(|_, cx| runs.update(cx, |r, cx| r.step(debug, "next", cx)));
    let stepped = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| crate::run::marks(cx).unwrap().read(cx).exec.as_ref().is_some_and(|(_, l)| *l != line))
    });
    assert!(stepped, "a step moves to the next line");
    cx.update(|_, cx| runs.update(cx, |r, cx| r.evaluate(debug, "total + 100".into(), cx)));
    let evaluated = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| runs.read(cx).session(debug).and_then(|s| s.debug.as_ref()?.evaluated.clone()).is_some())
    });
    assert!(evaluated);
    let value =
        cx.update(|_, cx| runs.read(cx).session(debug).unwrap().debug.as_ref().unwrap().evaluated.clone().unwrap());
    assert_eq!(value.1.map(|v| v.value), Ok("101".to_owned()));
    // Hot swap: add() changes and is saved while the program is stopped; jdtls builds it and
    // the debugger reloads it, so the loop's remaining calls run the new code.
    let main_editor = cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file).unwrap());
    cx.update(|w, cx| {
        main_editor.update(cx, |e, cx| {
            let text = e.text(cx).replace("return a + b;", "return a + b * 10;");
            e.state.update(cx, |s, cx| s.set_value(text, w, cx));
            e.save(cx);
        })
    });
    let reloaded = pump_until(cx, 120, |cx| {
        cx.update(|_, cx| {
            runs.read(cx)
                .session(debug)
                .and_then(|s| s.debug.as_ref()?.note.clone())
                .is_some_and(|n| n.starts_with("Reloaded"))
        })
    });
    assert!(reloaded, "the change is reloaded: {}", output(&runs, debug, cx));
    // Off with the breakpoint, and on to the end: 1 + 2 * 10 + 3 * 10.
    cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, line, cx)));
    cx.update(|_, cx| runs.update(cx, |r, cx| r.step(debug, "continue", cx)));
    assert!(pump_until(cx, 120, |cx| finished(&runs, debug, cx).is_some()), "the program ends");
    assert!(output(&runs, debug, cx).contains("total 51"), "{}", output(&runs, debug, cx));

    cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}

/// A real project, when `SNOUT_TEST_PROJECT` names it (with `SNOUT_TEST_PROJECT_JDK` for its
/// Gradle, and `SNOUT_TEST_CLASS` a test class in it, by file name): its tasks list, its
/// `classes` task runs, its Spring Boot configuration resolves (a dry run: nothing starts)
/// and the test class runs from its gutter icon. The project is only read.
#[gpui::test]
fn a_real_project_lists_runs_and_tests(cx: &mut TestAppContext) {
    let (Some(project), Some(_)) = (std::env::var_os("SNOUT_TEST_PROJECT").map(PathBuf::from), java::engine_dist())
    else {
        println!("real project: skipped; set SNOUT_TEST_PROJECT");
        return;
    };
    let project = project.canonicalize().unwrap();
    cx.executor().allow_parking();
    let _engine = crate::tests::engine_lock();
    let mut settings = crate::settings::Settings::default();
    if let Some(jdk) = std::env::var_os("SNOUT_TEST_PROJECT_JDK") {
        settings.project_jdks.insert(project.clone(), PathBuf::from(jdk));
    }
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, settings);
    cx.update(|_, cx| model.update(cx, |m, cx| m.open_arg(&project, cx)));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let runs = runs_of(&ws, cx);
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    let started = Instant::now();
    let ready = pump_until(cx, 1500, |cx| {
        cx.update(|_, cx| {
            let r = runs.read(cx);
            !r.engine_configs.is_empty() && !r.gradle.builds.is_empty()
        })
    });
    assert!(ready, "the configurations and the tasks come");
    println!("real project: ready after {:.0} s", started.elapsed().as_secs_f32());
    let builds = cx.update(|_, cx| runs.read(cx).gradle.builds.clone());
    let all: Vec<String> = builds
        .iter()
        .flat_map(|b| b.projects.iter())
        .flat_map(|p| p.groups.iter().flat_map(|(_, t)| t.iter().map(|t| t.path.clone())))
        .collect();
    println!("real project: {} tasks, e.g. {:?}", all.len(), all.iter().take(8).collect::<Vec<_>>());
    let classes =
        all.iter().find(|t| t.ends_with(":classes") && t.matches(':').count() == 1).cloned().expect("a :classes task");
    let configs = cx.update(|_, cx| runs.read(cx).configurations());
    for c in &configs {
        println!("real project: configuration {} ({:?}, {})", c.name, c.kind, c.group());
    }
    let boot =
        configs.iter().find(|c| c.spring_boot && c.kind == Kind::Main).cloned().expect("a Spring Boot configuration");
    let dry = cx
        .update(|_, cx| runs.read(cx).request("snout/run", json!({ "config": boot.to_engine(), "dryRun": true }), cx));
    let resolved = futures::executor::block_on(dry).expect("the dry run resolves");
    println!(
        "real project: {} resolves to {} on {} classpath entries",
        boot.name,
        resolved["mainClass"],
        resolved["classpath"].as_array().map_or(0, Vec::len)
    );

    let build = cx
        .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Task { build: None, task: classes.clone() }, cx)))
        .unwrap();
    assert!(pump_until(cx, 900, |cx| finished(&runs, build, cx).is_some()), "{classes} ends");
    assert_eq!(finished(&runs, build, cx), Some(Outcome::Success), "{}", output(&runs, build, cx));
    println!("real project: {classes} succeeded");

    if let Some(class) = std::env::var_os("SNOUT_TEST_CLASS") {
        let class = class.to_string_lossy().into_owned();
        let file = ignore::WalkBuilder::new(&project)
            .build()
            .flatten()
            .map(|e| e.into_path())
            .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == class))
            .expect("the test class's file");
        act(&model, cx, Action::OpenFile(file.clone(), None));
        let editor = cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file).unwrap());
        assert!(pump_until(cx, 120, |cx| cx.update(|_, cx| editor
            .read(cx)
            .runnables
            .as_ref()
            .is_some_and(|r| r.module.is_some() && !r.items.is_empty()))));
        let (target, module) = cx.update(|_, cx| {
            let r = editor.read(cx).runnables.clone().unwrap();
            (
                r.items.iter().find(|i| matches!(i.target, Target::TestClass { .. })).unwrap().target.clone(),
                r.module.clone(),
            )
        });
        cx.update(|_, cx| runs.update(cx, |r, cx| r.run_target(&target, module.as_deref(), false, cx)));
        let tests = cx.update(|_, cx| runs.read(cx).sessions.last().unwrap().id);
        assert!(pump_until(cx, 900, |cx| finished(&runs, tests, cx).is_some()), "the tests end");
        let summary = cx.update(|_, cx| runs.read(cx).session(tests).unwrap().test_summary());
        println!("real project: {class}: {summary:?}");
        let counts = cx.update(|_, cx| runs.read(cx).session(tests).unwrap().tests.as_ref().unwrap().counts());
        assert!(counts.0 > 0 && counts.1 == 0, "{counts:?}\n{}", output(&runs, tests, cx));
    }
    cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
}

/// The fixture's tests run as a Gradle task (the Build window), as a test configuration, and
/// under the debugger (stopping in the code they test), as a Gradle task and from the
/// gutter; with `java8`, the build targets Java 8 and runs on the JDK given, and its main
/// class is debugged too.
fn tests_run_and_debug(cx: &mut TestAppContext, jdk: PathBuf, java8: bool) {
    cx.executor().allow_parking();
    let _engine = crate::tests::engine_lock();
    let repo = run_app(if java8 { "run-tests-8" } else { "run-tests" });
    if java8 {
        let build = repo.join("app/build.gradle");
        let text = std::fs::read_to_string(&build).unwrap().replace("VERSION_17", "VERSION_1_8");
        std::fs::write(&build, text).unwrap();
        // `readAllBytes` came with Java 9.
        let main = repo.join("app/src/main/java/demo/app/Main.java");
        let text = std::fs::read_to_string(&main)
            .unwrap()
            .replace("new String(in.readAllBytes(), StandardCharsets.UTF_8).trim()", "\"present\"")
            .replace(
                "System.out.println(\"total \" + total);",
                "System.out.println(\"total \" + total + \" on \" + System.getProperty(\"java.version\"));",
            );
        std::fs::write(&main, text).unwrap();
        git(&repo, &["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-qam", "Java 8"]);
    }
    let mut settings = with_repo(&repo);
    settings.project_jdks.insert(repo.clone(), jdk);
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, settings);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let runs = runs_of(&ws, cx);
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    let ready = pump_until(cx, 600, |cx| {
        cx.update(|_, cx| {
            let r = runs.read(cx);
            !r.engine_configs.is_empty() && !r.gradle.builds.is_empty()
        })
    });
    assert!(ready, "the configurations and the tasks within ten minutes");
    let configs = cx.update(|_, cx| runs.read(cx).configurations());
    let counts = |id: u64, cx: &mut VisualTestContext| {
        cx.update(|_, cx| runs.read(cx).session(id).and_then(|s| s.tests.as_ref().map(|t| t.counts())))
    };

    // `:app:test` from the Gradle window: two tests pass, one fails on purpose.
    let task = cx
        .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Task { build: None, task: ":app:test".into() }, cx)))
        .unwrap();
    assert!(pump_until(cx, 300, |cx| finished(&runs, task, cx).is_some()), "the test task ends");
    assert_eq!(finished(&runs, task, cx), Some(Outcome::Failed), "{}", output(&runs, task, cx));
    let (passed, failed) = {
        let c = counts(task, cx).unwrap_or_else(|| panic!("a test tree: {}", output(&runs, task, cx)));
        (c.0, c.1)
    };
    assert_eq!((passed, failed), (2, 1), "{}", output(&runs, task, cx));

    // The build's own test configuration runs the same.
    let tests = configs.iter().find(|c| c.kind == Kind::Tests).cloned().expect("a test configuration");
    let run = cx
        .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Config { config: tests.clone(), debug: false }, cx)))
        .unwrap();
    assert!(pump_until(cx, 300, |cx| finished(&runs, run, cx).is_some()), "the configuration ends");
    assert_eq!(counts(run, cx).map(|c| (c.0, c.1)), Some((2, 1)), "{}", output(&runs, run, cx));

    // Debugging the tests stops in the code they call, as a Gradle test task (`:app:test`
    // under the debugger) and from the test class's gutter icon.
    let greeter = repo.join("app/src/main/java/demo/app/Greeter.java");
    let line = std::fs::read_to_string(&greeter).unwrap().lines().position(|l| l.contains("return \"hello \"")).unwrap()
        as u32;
    cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&greeter, line, cx)));
    let gradle_test = configs
        .iter()
        .find(|c| c.kind == Kind::Gradle && c.task.as_deref().is_some_and(|t| t.ends_with(":test")))
        .cloned()
        .unwrap_or_else(|| crate::run::config::RunConfig {
            id: "user:test".into(),
            kind: Kind::Gradle,
            name: "test".into(),
            task: Some(":app:test".into()),
            ..Default::default()
        });
    let test = repo.join("app/src/test/java/demo/app/GreeterTest.java");
    act(&model, cx, Action::OpenFile(test.clone(), None));
    let editor = cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == test).unwrap());
    assert!(
        pump_until(cx, 120, |cx| cx.update(|_, cx| editor
            .read(cx)
            .runnables
            .as_ref()
            .is_some_and(|r| r.module.is_some()))),
        "the engine finds the file's tests"
    );
    let (target, module) = cx.update(|_, cx| {
        let r = editor.read(cx).runnables.clone().unwrap();
        let class = r.items.iter().find(|i| matches!(i.target, Target::TestClass { .. })).unwrap().target.clone();
        (class, r.module.clone())
    });
    for how in ["gradle task", "gutter"] {
        let debug = if how == "gradle task" {
            cx.update(|_, cx| {
                runs.update(cx, |r, cx| r.start(Launch::Config { config: gradle_test.clone(), debug: true }, cx))
            })
            .unwrap()
        } else {
            cx.update(|_, cx| runs.update(cx, |r, cx| r.run_target(&target, module.as_deref(), true, cx)));
            cx.update(|_, cx| runs.read(cx).sessions.last().unwrap().id)
        };
        let paused = pump_until(cx, 300, |cx| {
            cx.update(|_, cx| crate::run::marks(cx).unwrap().read(cx).exec.as_ref().is_some_and(|(f, _)| *f == greeter))
        });
        assert!(paused, "debugging the tests ({how}) stops in Greeter: {}", output(&runs, debug, cx));
        cx.update(|_, cx| {
            let m = crate::run::marks(cx).unwrap();
            assert_eq!(m.read(cx).exec, Some((greeter.clone(), line)), "{how}");
        });
        cx.update(|_, cx| runs.update(cx, |r, cx| r.step(debug, "continue", cx)));
        assert!(pump_until(cx, 300, |cx| finished(&runs, debug, cx).is_some()), "the {how} tests end");
    }
    cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&greeter, line, cx)));

    if java8 {
        // The main class, debugged on Java 8: the breakpoint stops it and the values show.
        let main = configs.iter().find(|c| c.kind == Kind::Main).cloned().expect("a main configuration");
        let file = repo.join("app/src/main/java/demo/app/Main.java");
        let line =
            std::fs::read_to_string(&file).unwrap().lines().position(|l| l.contains("total = add(total, i);")).unwrap()
                as u32;
        cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, line, cx)));
        let debug = cx
            .update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Config { config: main, debug: true }, cx)))
            .unwrap();
        let paused = pump_until(cx, 300, |cx| {
            cx.update(|_, cx| {
                let m = crate::run::marks(cx).unwrap();
                m.read(cx).exec.as_ref().is_some_and(|(f, _)| *f == file) && !m.read(cx).locals.is_empty()
            })
        });
        assert!(paused, "the breakpoint is hit on Java 8: {}", output(&runs, debug, cx));
        cx.update(|_, cx| {
            let m = crate::run::marks(cx).unwrap();
            assert!(m.read(cx).locals.contains(&("i".into(), "1".into())), "{:?}", m.read(cx).locals);
        });
        cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, line, cx)));
        cx.update(|_, cx| runs.update(cx, |r, cx| r.step(debug, "continue", cx)));
        assert!(pump_until(cx, 120, |cx| finished(&runs, debug, cx).is_some()), "the program ends");
        let out = output(&runs, debug, cx);
        assert!(out.contains("total 6 on 1.8.0"), "the program ran on Java 8: {out}");
    }

    cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}

#[gpui::test]
fn the_fixtures_tests_run_and_debug_as_gradle_tasks(cx: &mut TestAppContext) {
    let (Some(_), Some(jdk)) = (java::engine_dist(), test_jdk()) else {
        println!("tests run and debug: skipped; needs an engine distribution and a JDK 17 to 21");
        return;
    };
    tests_run_and_debug(cx, jdk, false);
}

/// With `SNOUT_TEST_JDK8` naming a JDK 8: the fixture as a Java 8 build, on Java 8.
#[gpui::test]
fn a_java_8_build_runs_and_debugs(cx: &mut TestAppContext) {
    let jdk8 = std::env::var_os("SNOUT_TEST_JDK8").map(PathBuf::from).and_then(|h| {
        java::jdk_at(&h).or_else(|| java::jdk_at(&h.join("Contents/Home"))).filter(|j| j.major == 8).map(|j| j.home)
    });
    let (Some(_), Some(jdk)) = (java::engine_dist(), jdk8) else {
        println!("Java 8: skipped; set SNOUT_TEST_JDK8 to a JDK 8");
        return;
    };
    tests_run_and_debug(cx, jdk, true);
}
