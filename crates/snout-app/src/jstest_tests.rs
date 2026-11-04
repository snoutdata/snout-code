//! Vitest and Jest: packages found as suites, run icons in their test files, and runs into
//! the Tests window through our reporters (here driven by stand-ins for the runners, which
//! call the reporters as Vitest 3 and Jest 29 do).

use std::path::{Path, PathBuf};

use crate::jstest::{Runner, Scope, Suite};
use crate::lang::node;
use crate::lang_tests::{editor_of, pump_until};
use crate::model::Action;
use crate::playwright::report::State;
use crate::run::Target;
use crate::tests::{act, open, scratch_dir, with_repo};

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn fixtures() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/jstest")).to_owned()
}

/// A package at `dir` that tests with `runner`, the stand-in installed as its program.
fn suite_package(dir: &Path, runner: Runner) {
    match runner {
        Runner::Vitest => {
            write(dir, "package.json", r#"{ "name": "cart", "devDependencies": { "vitest": "^3.2.0" } }"#);
            write(
                dir,
                "node_modules/vitest/package.json",
                r#"{ "version": "3.2.7", "bin": { "vitest": "./vitest.mjs" } }"#,
            );
            std::fs::copy(fixtures().join("fake-vitest.mjs"), dir.join("node_modules/vitest/vitest.mjs")).unwrap();
            write(
                dir,
                "src/cart.test.ts",
                "import { describe, it, expect } from 'vitest';\n\ndescribe('cart', () => {\n  it('adds items', () => {\n    expect(1).toBe(1);\n  });\n\n  it('totals', () => {\n    expect(40).toBe(42);\n  });\n});\n",
            );
        }
        Runner::Jest => {
            write(dir, "package.json", r#"{ "name": "math", "devDependencies": { "jest": "^29.7.0" } }"#);
            write(dir, "node_modules/jest/package.json", r#"{ "version": "29.7.0", "bin": "./bin/jest.js" }"#);
            std::fs::create_dir_all(dir.join("node_modules/jest/bin")).unwrap();
            std::fs::copy(fixtures().join("fake-jest.js"), dir.join("node_modules/jest/bin/jest.js")).unwrap();
            write(
                dir,
                "math.test.js",
                "describe('math', () => {\n  test('adds', () => {});\n\n  test('divides', () => {});\n});\n",
            );
        }
    }
}

fn args_of(dir: &Path, runner: Runner) -> Vec<String> {
    let name = if runner == Runner::Vitest { "vitest-args.json" } else { "jest-args.json" };
    serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
}

#[test]
fn packages_test_with_the_runner_they_depend_on_or_configure() {
    let host = snout_host::local();
    let repo = scratch_dir("jstest-suites");
    write(&repo, "package.json", r#"{ "name": "shop", "workspaces": ["apps/*"] }"#);
    suite_package(&repo.join("apps/web"), Runner::Vitest);
    suite_package(&repo.join("apps/api"), Runner::Jest);
    write(&repo, "tools/package.json", r#"{ "name": "tools" }"#);
    write(&repo, "tools/vitest.config.ts", "export default {};\n");
    let found = crate::jstest::suites(&*host, &repo);
    let labels: Vec<String> = found.iter().map(|s| s.label(&repo)).collect();
    assert_eq!(labels, ["Jest in apps/api", "Vitest in apps/web", "Vitest in tools"]);
    let file = repo.join("apps/web/src/cart.test.ts");
    assert_eq!(
        crate::jstest::suite_of(&*host, &file, &repo),
        Some(Suite { dir: repo.join("apps/web"), runner: Runner::Vitest })
    );
    assert_eq!(
        crate::jstest::cli(&*host, &repo.join("apps/api"), Runner::Jest),
        Some(repo.join("apps/api/node_modules/jest/bin/jest.js"))
    );
    assert_eq!(crate::jstest::cli(&*host, &repo.join("tools"), Runner::Vitest), None, "not installed");
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn a_test_runs_by_its_file_and_full_name_and_debugs_in_one_process() {
    let host = snout_host::local();
    let dir = scratch_dir("jstest-launch");
    suite_package(&dir, Runner::Jest);
    let n = node::Node { bin: "/n/bin/node".into(), version: "22.12.0".into(), major: 22 };
    let scope = Scope::At { file: dir.join("math.test.js"), path: vec!["math".into(), "adds".into()], group: false };
    let l = crate::jstest::test_launch(&*host, &dir, &dir, &n, Runner::Jest, &scope, true).unwrap();
    assert!(l.report && l.debug_node.is_some());
    let has = |a: &str| l.args.iter().any(|x| x == a);
    assert!(has("--runInBand") && has("--testLocationInResults") && has(r"math\.test\.js"), "{:?}", l.args);
    assert_eq!(&l.args[l.args.len() - 2..], ["-t", "math adds$"]);
    assert_eq!(l.title, "adds");
    suite_package(&dir, Runner::Vitest);
    let all = crate::jstest::test_launch(&*host, &dir, &dir, &n, Runner::Vitest, &Scope::All, false).unwrap();
    assert_eq!(all.args[1..3], ["run".to_owned(), "--reporter=default".to_owned()]);
    assert!(!all.args.iter().any(|a| a == "-t" || a == "--pool=forks"));
    assert_eq!(all.title, "Vitest");
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_vitest_file_gets_icons_and_its_test_runs_into_the_tests_window(cx: &mut gpui::TestAppContext) {
    if node::discover(&*snout_host::local()).is_empty() {
        println!("vitest runs: skipped; needs Node");
        return;
    }
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("jstest-vitest");
    suite_package(&repo, Runner::Vitest);
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
    let found = pump_until(cx, 10, |cx| cx.update(|_, cx| tests.read(cx).root.is_some()));
    assert!(found, "the Tests window has the package's tests");
    let file = repo.join("src/cart.test.ts");
    act(&model, cx, Action::OpenFile(file.clone(), None));
    let editor = editor_of(&ws, &file, cx).expect("the test file opens");
    let found = pump_until(cx, 10, |cx| cx.update(|_, cx| editor.read(cx).runnables.is_some()));
    assert!(found);
    let r = cx.update(|_, cx| editor.read(cx).runnables.clone()).unwrap();
    let lines: Vec<(u32, Vec<String>)> = r
        .items
        .iter()
        .map(|i| match &i.target {
            Target::JsTest { path, runner: Runner::Vitest, .. } => (i.line, path.clone()),
            other => panic!("{other:?}"),
        })
        .collect();
    let p = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(lines, [(2, p(&["cart"])), (3, p(&["cart", "adds items"])), (7, p(&["cart", "totals"]))]);
    let target = r.at_line(8).unwrap().clone();
    cx.update(|_, cx| runs.update(cx, |r, cx| r.run_target(&target, None, false, cx)));
    let done =
        pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).sessions.last().is_some_and(|s| s.ended.is_some())));
    assert!(done, "the run ends");
    cx.update(|_, cx| {
        let s = runs.read(cx).sessions.last().unwrap();
        let run = s.playwright.as_ref().expect("the Tests window follows it");
        assert_eq!(run.runner, Some(Runner::Vitest));
        let c = run.counts();
        assert_eq!((c.total, c.passed, c.failed), (2, 1, 1), "{}", s.console_text());
        let failed = run.failures()[0];
        assert_eq!((failed.path.clone(), failed.line), (p(&["cart", "totals"]), 8));
        assert_eq!(failed.last_try().unwrap().errors[0].headline(), "expected 40 to be 42");
        assert_eq!(run.location(&failed.id), Some((file.clone(), 8)));
        assert!(s.console_text().contains("RUN  v3.2.7"), "Vitest's own output shows: {}", s.console_text());
        let t = tests.read(cx);
        assert_eq!(t.unit(cx), Some(Suite { dir: repo.clone(), runner: Runner::Vitest }));
        assert!(t.unit_run(cx));
    });
    let args = args_of(&repo, Runner::Vitest);
    assert!(args.iter().any(|a| a == "src/cart.test.ts"), "{args:?}");
    assert_eq!(&args[args.len() - 2..], ["-t", "cart totals$"]);
    // Rerun failed: the failed one, by its name.
    cx.update(|_, cx| tests.update(cx, |t, cx| t.start(crate::playwright::run::Scope::LastFailed, cx)));
    let again = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            let r = runs.read(cx);
            r.sessions.len() >= 2 && r.sessions.last().is_some_and(|s| s.ended.is_some())
        })
    });
    assert!(again);
    let args = args_of(&repo, Runner::Vitest);
    assert_eq!(&args[args.len() - 2..], ["-t", "cart totals$"]);
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn a_jest_run_shows_every_test_skipped_ones_too(cx: &mut gpui::TestAppContext) {
    if node::discover(&*snout_host::local()).is_empty() {
        println!("jest runs: skipped; needs Node");
        return;
    }
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("jstest-jest");
    suite_package(&repo, Runner::Jest);
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
    let found = pump_until(cx, 10, |cx| cx.update(|_, cx| tests.read(cx).root.is_some()));
    assert!(found);
    // Run all, from the Tests window.
    cx.update(|_, cx| tests.update(cx, |t, cx| t.start(crate::playwright::run::Scope::All, cx)));
    let done =
        pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).sessions.last().is_some_and(|s| s.ended.is_some())));
    assert!(done);
    cx.update(|_, cx| {
        let s = runs.read(cx).sessions.last().unwrap();
        assert_eq!(s.title, "Jest");
        let run = s.playwright.as_ref().unwrap();
        let states: Vec<(String, State)> =
            run.order.iter().map(|id| (run.tests[id].path.join(" "), run.state_of(id))).collect();
        assert_eq!(
            states,
            [
                ("math adds".to_owned(), State::Passed),
                ("math divides".to_owned(), State::Failed),
                ("later".to_owned(), State::Skipped)
            ]
        );
        let failed = run.failures()[0];
        let e = &failed.last_try().unwrap().errors[0];
        assert_eq!(e.headline(), "Error: expect(received).toBe(expected)");
        assert_eq!(e.location.line, Some(6), "where the failure points");
    });
    let args = args_of(&repo, Runner::Jest);
    assert!(!args.iter().any(|a| a == "-t"), "every test: {args:?}");
    let _ = std::fs::remove_dir_all(repo);
}

/// Real runs of installed packages, under the real js-debug (downloaded as the app does):
/// `SNOUT_VITEST_DIR` and `SNOUT_JEST_DIR` name packages with the runner installed and a
/// `describe('math')` whose `adds` test sits on line 3 and computes `1 + 2` on line 4 (from
/// 1). Each runs, then debugs that test, stopping on line 4. Run by hand with `--ignored`.
#[gpui::test]
#[ignore]
fn real_vitest_and_jest_tests_run_and_stop_at_breakpoints(cx: &mut gpui::TestAppContext) {
    cx.executor().allow_parking();
    let dirs: Vec<(PathBuf, Runner, &str)> =
        [("SNOUT_VITEST_DIR", Runner::Vitest, "src/math.test.ts"), ("SNOUT_JEST_DIR", Runner::Jest, "math.test.js")]
            .into_iter()
            .filter_map(|(var, r, f)| std::env::var(var).ok().map(|d| (PathBuf::from(d), r, f)))
            .collect();
    assert!(!dirs.is_empty(), "set SNOUT_VITEST_DIR or SNOUT_JEST_DIR");
    let (_, ws, cx) = open(cx, with_repo(&dirs[0].0));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    for (dir, runner, file) in dirs {
        let file = dir.join(file);
        let path = vec!["math".to_owned(), "adds".to_owned()];
        let scope = Scope::At { file: file.clone(), path: path.clone(), group: false };
        cx.update(|_, cx| runs.update(cx, |r, cx| r.start_js_tests(dir.clone(), runner, scope.clone(), false, cx)));
        let done = pump_until(cx, 60, |cx| {
            cx.update(|_, cx| {
                runs.read(cx).sessions.last().is_some_and(|s| s.ended.is_some() && s.playwright.is_some())
            })
        });
        assert!(done);
        cx.update(|_, cx| {
            let s = runs.read(cx).sessions.last().unwrap();
            let run = s.playwright.as_ref().unwrap();
            let adds = run.tests.values().find(|t| t.path == path).unwrap_or_else(|| panic!("{}", s.console_text()));
            assert_eq!(run.state_of(&adds.id), State::Passed, "{}", s.console_text());
            assert_eq!(adds.line, 3);
            println!("{}: ran, {} tests", runner.name(), run.tests.len());
        });
        let started = std::time::Instant::now();
        cx.update(|_, cx| {
            runs.update(cx, |r, cx| {
                r.toggle_breakpoint(&file, 3, cx);
                r.start_js_tests(dir.clone(), runner, scope.clone(), true, cx);
            })
        });
        let paused = pump_until(cx, 120, |cx| {
            cx.update(|_, cx| {
                let r = runs.read(cx);
                r.paused_debug().is_some_and(|id| !r.session(id).unwrap().debug.as_ref().unwrap().locals().is_empty())
            })
        });
        let console = cx.update(|_, cx| runs.read(cx).sessions.last().map(|s| s.console_text()).unwrap_or_default());
        assert!(paused, "{} stops: {console}", runner.name());
        let id = cx.update(|_, cx| runs.read(cx).paused_debug().unwrap());
        cx.update(|_, cx| {
            let d = runs.read(cx).session(id).unwrap().debug.as_ref().unwrap().snapshot();
            assert_eq!(d.exec_point(), Some((file.clone(), 3)));
            println!("{}: stopped after {:?}, {} connections", runner.name(), started.elapsed(), d.targets.len());
        });
        cx.update(|_, cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
        let done =
            pump_until(cx, 60, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
        assert!(done, "{} goes on to the end", runner.name());
        cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, 3, cx)));
    }
}
