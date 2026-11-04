//! Node projects: a `package.json`'s scripts as run configurations, script files run as
//! Current File, and both debugged through js-debug (here its stand-in), attach included.

use std::path::{Path, PathBuf};

use crate::lang::node;
use crate::lang_tests::{editor_of, pump_until};
use crate::model::Action;
use crate::run::config::{Kind, RunConfig};
use crate::run::{Launch, Outcome, Place, Runs, Target};
use crate::tests::{act, open, scratch_dir, with_repo};

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Whether this machine has a Node with npm beside it, as the run tests need.
fn has_node() -> Option<node::Node> {
    let host = snout_host::local();
    let all = node::discover(&*host);
    let n = all.into_iter().next()?;
    crate::lang::typescript::npm_cli(&*host, &n).map(|_| n)
}

/// A Node project: a few scripts, and the programs they run.
fn node_project(name: &str) -> PathBuf {
    let repo = crate::tests::demo_repo(name);
    write(
        &repo,
        "package.json",
        r#"{
  "name": "shop",
  "scripts": {
    "build": "node build.js",
    "dev": "node dev.js --port 5173"
  }
}
"#,
    );
    write(&repo, "build.js", "console.log('built');\n");
    write(&repo, "dev.js", "console.log('dev server ready on', process.argv.slice(2).join(' '));\n");
    repo
}

fn ended(runs: &gpui::Entity<Runs>, cx: &mut gpui::VisualTestContext) -> Option<u64> {
    let found =
        pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).sessions.last().is_some_and(|s| s.ended.is_some())));
    found.then(|| cx.update(|_, cx| runs.read(cx).sessions.last().unwrap().id))
}

#[test]
fn a_node_project_is_a_package_json_without_playwright_or_a_java_build() {
    let host = snout_host::local();
    let dir = scratch_dir("node-kind");
    assert!(!crate::run::node_run::is_node_project(&*host, &dir));
    write(&dir, "package.json", r#"{ "name": "x" }"#);
    assert!(crate::run::node_run::is_node_project(&*host, &dir));
    assert!(!crate::run::node_run::has_dependencies(&*host, &dir));
    assert_eq!(crate::playwright::setup::check(&*host, &dir), None, "nothing to install");
    write(&dir, "package.json", r#"{ "name": "x", "dependencies": { "express": "^5.0.0" } }"#);
    assert!(
        matches!(crate::playwright::setup::check(&*host, &dir), Some(crate::playwright::setup::Need::Packages { .. })),
        "its packages want installing"
    );
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    assert_eq!(crate::playwright::setup::check(&*host, &dir), None);
    let summary = crate::playwright::setup::summary(&*host, &dir, Some(&dir)).unwrap();
    assert!(summary.starts_with("npm · "), "{summary}");
    write(&dir, "build.gradle.kts", "");
    assert!(!crate::run::node_run::is_node_project(&*host, &dir), "a Java build with a package.json beside");
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_node_projects_scripts_are_configurations_and_dev_runs_first(cx: &mut gpui::TestAppContext) {
    if has_node().is_none() {
        println!("node runs: skipped; needs Node and npm");
        return;
    }
    cx.executor().allow_parking();
    let repo = node_project("node-scripts");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let listed = pump_until(cx, 10, |cx| cx.update(|_, cx| runs.read(cx).node_configs.len() == 2));
    assert!(listed, "the scripts are configurations");
    let selected = cx.update(|_, cx| runs.read(cx).selected_config()).unwrap();
    assert_eq!((selected.kind, selected.name.as_str()), (Kind::Npm, "npm run dev"), "dev before build");
    cx.update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Config { config: selected, debug: false }, cx)));
    let id = ended(&runs, cx).expect("the script ends");
    cx.update(|_, cx| {
        let s = runs.read(cx).session(id).unwrap();
        assert_eq!(s.place(), Place::Run);
        assert_eq!(s.ended.as_ref().unwrap().outcome, Outcome::Success, "{}", s.console_text());
        assert!(s.console_text().contains("dev server ready on --port 5173"), "{}", s.console_text());
    });
    // Run while it runs is Rerun: the session knows its configuration.
    cx.update(|_, cx| assert_eq!(runs.read(cx).session(id).unwrap().config.as_deref(), Some("npm:dev")));
    // The script's arguments go after `--`.
    let build = RunConfig {
        args: vec!["--watch".into()],
        ..crate::run::node_run::configs(&*snout_host::local(), &repo)[0].clone()
    };
    let n = has_node().unwrap();
    let launch = crate::run::node_run::config_launch(&*snout_host::local(), &repo, &n, &build, false).unwrap();
    assert_eq!(&launch.args[1..], ["run", "build", "--", "--watch"]);
    assert_eq!(launch.title, "npm run build");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn package_json_gets_a_run_icon_beside_each_script(cx: &mut gpui::TestAppContext) {
    cx.executor().allow_parking();
    let repo = node_project("node-icons");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let file = repo.join("package.json");
    act(&model, cx, Action::OpenFile(file.clone(), None));
    let editor = editor_of(&ws, &file, cx).expect("package.json opens");
    let found = pump_until(cx, 10, |cx| cx.update(|_, cx| editor.read(cx).runnables.is_some()));
    assert!(found);
    let r = cx.update(|_, cx| editor.read(cx).runnables.clone()).unwrap();
    let items: Vec<(u32, Target)> = r.items.iter().map(|i| (i.line, i.target.clone())).collect();
    assert_eq!(items, [(3, Target::NpmScript { name: "build".into() }), (4, Target::NpmScript { name: "dev".into() })]);
    assert_eq!(r.at_line(4).map(Target::menu_label).as_deref(), Some("dev"), "the editor's Run is the script's");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn current_file_runs_a_script_on_the_projects_node(cx: &mut gpui::TestAppContext) {
    let Some(n) = has_node() else {
        println!("node runs: skipped; needs Node and npm");
        return;
    };
    cx.executor().allow_parking();
    let repo = node_project("node-file");
    write(&repo, "tools/hello.mjs", "console.log('hello from', process.argv[2]);\n");
    write(&repo, "tools/sum.ts", "const n: number = 2;\nconsole.log('sum is', n * 3);\n");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let file = repo.join("tools/hello.mjs");
    act(&model, cx, Action::OpenFile(file.clone(), None));
    let editor = editor_of(&ws, &file, cx).expect("the script opens");
    let found = pump_until(cx, 10, |cx| cx.update(|_, cx| editor.read(cx).runnables.is_some()));
    assert!(found);
    let r = cx.update(|_, cx| editor.read(cx).runnables.clone()).unwrap();
    assert!(r.items.is_empty(), "no icon in the gutter");
    assert_eq!(r.at_line(0), Some(&Target::Script { file: file.clone() }), "Run runs the file");
    // With arguments, as its configuration has them.
    let mut config = crate::run::node_run::file_config(&repo, &file);
    config.args = vec!["SnoutCode".into()];
    cx.update(|_, cx| runs.update(cx, |r, cx| r.start(Launch::Config { config, debug: false }, cx)));
    let id = ended(&runs, cx).expect("the script ends");
    cx.update(|_, cx| {
        let s = runs.read(cx).session(id).unwrap();
        assert_eq!(s.title, "hello.mjs");
        assert!(s.console_text().contains("hello from SnoutCode"), "{}", s.console_text());
    });
    // TypeScript: Node strips the types itself from 22.6, the flag before 22.18.
    let ts = repo.join("tools/sum.ts");
    let launch = crate::run::node_run::file_launch(&*snout_host::local(), &repo, &n, &ts, &[], &[], None, Vec::new());
    match launch {
        Ok(_) => {
            cx.update(|_, cx| runs.update(cx, |r, cx| r.run_node_file(&ts, false, cx)));
            let found = pump_until(cx, 30, |cx| {
                cx.update(|_, cx| {
                    runs.read(cx).sessions.last().is_some_and(|s| s.title == "sum.ts" && s.ended.is_some())
                })
            });
            assert!(found, "the TypeScript file runs");
            let text = cx.update(|_, cx| runs.read(cx).sessions.last().unwrap().console_text());
            assert!(text.contains("sum is 6"), "{text}");
        }
        Err(why) => assert!(why.contains("tsx"), "{why}"),
    }
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn typescript_runs_through_the_projects_tsx_when_it_has_it() {
    let host = snout_host::local();
    let dir = scratch_dir("node-tsx");
    write(&dir, "package.json", r#"{ "devDependencies": { "tsx": "^4.19.0" } }"#);
    write(&dir, "node_modules/tsx/package.json", r#"{ "version": "4.19.2", "bin": { "tsx": "./dist/cli.mjs" } }"#);
    write(&dir, "node_modules/tsx/dist/cli.mjs", "");
    let old = node::Node { bin: "/n/18/bin/node".into(), version: "18.20.0".into(), major: 18 };
    let file = dir.join("src/server.ts");
    let l = crate::run::node_run::file_launch(
        &*host,
        &dir,
        &old,
        &file,
        &["--inspect".into()],
        &["-v".into()],
        None,
        Vec::new(),
    )
    .unwrap();
    let tsx = dir.join("node_modules").join("tsx").join("dist/cli.mjs").display().to_string();
    assert_eq!(l.args, [tsx, "--inspect".into(), file.display().to_string(), "-v".into()]);
    assert_eq!(l.program, old.bin);
    std::fs::remove_dir_all(dir.join("node_modules")).unwrap();
    let why = crate::run::node_run::file_launch(&*host, &dir, &old, &file, &[], &[], None, Vec::new()).unwrap_err();
    assert!(why.contains("npm add -D tsx") && why.contains("Node 18.20.0"), "{why}");
    let new = node::Node { bin: "/n/22/bin/node".into(), version: "22.12.0".into(), major: 22 };
    let l = crate::run::node_run::file_launch(&*host, &dir, &new, &file, &[], &[], None, Vec::new()).unwrap();
    assert_eq!(l.args, ["--experimental-strip-types".to_owned(), file.display().to_string()]);
    let _ = std::fs::remove_dir_all(dir);
}

/// The fake js-debug server, in a folder of its own.
fn fake_js_debug(name: &str) -> PathBuf {
    let server = scratch_dir(&format!("{name}-js-debug"));
    let fixtures = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/playwright"));
    std::fs::copy(fixtures.join("fake-js-debug.js"), server.join("dapDebugServer.js")).unwrap();
    server
}

/// Wait until a session stopped with its variables in.
fn paused(runs: &gpui::Entity<Runs>, cx: &mut gpui::VisualTestContext) -> Option<u64> {
    let found = pump_until(cx, 30, |cx| {
        cx.update(|_, cx| {
            let r = runs.read(cx);
            r.paused_debug().is_some_and(|id| !r.session(id).unwrap().debug.as_ref().unwrap().locals().is_empty())
        })
    });
    found.then(|| cx.update(|_, cx| runs.read(cx).paused_debug().unwrap()))
}

#[gpui::test]
fn debugging_a_script_stops_at_its_breakpoint(cx: &mut gpui::TestAppContext) {
    if has_node().is_none() {
        println!("node debugging: skipped; needs Node");
        return;
    }
    cx.executor().allow_parking();
    let repo = node_project("node-debug");
    // The program waits for its debugger to let go, as a real one does at a breakpoint.
    write(
        &repo,
        "tools/job.js",
        "const fs = require('fs');\nconst total = 2 * 3;\nconst hold = process.env.FAKE_DEBUG_HOLD;\n\
         if (hold) {\n  fs.writeFileSync(__dirname + '/debugged.json', JSON.stringify({ NODE_OPTIONS: process.env.NODE_OPTIONS }));\n\
         \x20 const a = new Int32Array(new SharedArrayBuffer(4));\n  while (!fs.existsSync(hold)) Atomics.wait(a, 0, 0, 50);\n}\n\
         console.log('job done', total);\n",
    );
    let server = fake_js_debug("node-debug");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let file = repo.join("tools/job.js");
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.js_debug_server = Some(server.join("dapDebugServer.js"));
            r.toggle_breakpoint(&file, 1, cx);
            r.run_target(&Target::Script { file: file.clone() }, None, true, cx);
        })
    });
    let log = || std::fs::read_to_string(server.join("js-debug.log")).unwrap_or_default();
    let id = paused(&runs, cx).unwrap_or_else(|| panic!("it stops at the breakpoint: {}", log()));
    cx.update(|_, cx| {
        let s = runs.read(cx).session(id).unwrap();
        assert_eq!((s.place(), s.title.as_str()), (Place::Debug, "job.js"));
        let d = s.debug.as_ref().unwrap();
        assert_eq!(d.exec_point(), Some((file.clone(), 1)));
        assert!(!d.attached);
    });
    let started: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join("tools/debugged.json")).unwrap()).unwrap();
    assert!(started["NODE_OPTIONS"].as_str().unwrap().contains("bootloader.js"), "js-debug's variables: {started}");
    assert!(log().contains(r#""runtimeArgs":[""#), "{}", log());
    cx.update(|_, cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
    let done = pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
    assert!(done, "the program ends once the debugger lets go: {}", log());
    let text = cx.update(|_, cx| runs.read(cx).session(id).unwrap().console_text());
    assert!(text.contains("job done 6"), "{text}");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(server);
}

#[gpui::test]
fn attaching_stops_at_a_breakpoint_and_stop_leaves_the_program_running(cx: &mut gpui::TestAppContext) {
    if has_node().is_none() {
        println!("node debugging: skipped; needs Node");
        return;
    }
    cx.executor().allow_parking();
    let repo = node_project("node-attach");
    write(&repo, "server.js", "const port = 3000;\n");
    let server = fake_js_debug("node-attach");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let file = repo.join("server.js");
    let attach = RunConfig {
        id: "user:attach".into(),
        kind: Kind::Node,
        name: "Attach to server".into(),
        port: Some(9229),
        user: true,
        ..Default::default()
    };
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.js_debug_server = Some(server.join("dapDebugServer.js"));
            r.set_user_configurations(vec![attach.clone()], Some(attach.id.clone()), cx);
            r.toggle_breakpoint(&file, 0, cx);
            // Run on an attach configuration debugs too.
            r.start(Launch::Config { config: attach, debug: false }, cx);
        })
    });
    let log = || std::fs::read_to_string(server.join("js-debug.log")).unwrap_or_default();
    let id = paused(&runs, cx).unwrap_or_else(|| panic!("it stops at the breakpoint: {}", log()));
    cx.update(|_, cx| {
        let s = runs.read(cx).session(id).unwrap();
        assert_eq!(s.title, "Attach to server");
        assert!(s.process.is_none(), "nothing started");
        assert!(s.console_text().contains("Attaching to the Node listening on port 9229"), "{}", s.console_text());
        assert!(s.debug.as_ref().unwrap().attached);
    });
    let said = log();
    assert!(
        said.contains(r#""role":"launch","type":"request","command":"attach""#)
            && said.contains(r#""address":"127.0.0.1","port":9229"#),
        "{said}"
    );
    cx.update(|_, cx| runs.update(cx, |r, cx| r.stop(id, cx)));
    let done = pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
    assert!(done, "Stop lets go");
    let said = log();
    assert!(said.contains(r#""command":"disconnect","arguments":{"terminateDebuggee":false}"#), "{said}");
    assert!(!said.contains(r#""terminateDebuggee":true"#), "the program goes on: {said}");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(server);
}

/// A real debug under js-debug (downloaded as the app does, unless `SNOUT_JS_DEBUG` names a
/// server script): an npm script, whose Node starts under a shell under npm, stops at a
/// breakpoint in the program; a TypeScript file, run with Node's type stripping, stops at
/// a breakpoint on its TypeScript line; an attach to a Node started with `--inspect` stops,
/// and Stop leaves that Node running. Run by hand with `--ignored`.
#[gpui::test]
#[ignore]
fn a_real_debug_stops_in_an_npm_script_a_typescript_file_and_an_attach(cx: &mut gpui::TestAppContext) {
    let n = has_node().expect("Node with npm");
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("node-real-debug");
    write(&repo, "package.json", r#"{ "name": "real", "scripts": { "start": "node src/main.js" } }"#);
    write(
        &repo,
        "src/main.js",
        "function triple(n) {\n  const out = n * 3;\n  return out;\n}\nconsole.log('tripled', triple(2));\n",
    );
    write(
        &repo,
        "src/sum.ts",
        "function add(a: number, b: number): number {\n  const total: number = a + b;\n  return total;\n}\nconsole.log('sum', add(2, 4));\n",
    );
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    if let Ok(script) = std::env::var("SNOUT_JS_DEBUG") {
        cx.update(|_, cx| runs.update(cx, |r, _| r.js_debug_server = Some(script.into())));
    }
    let real_paused = |runs: &gpui::Entity<Runs>, cx: &mut gpui::VisualTestContext| {
        let found = pump_until(cx, 120, |cx| {
            cx.update(|_, cx| {
                let r = runs.read(cx);
                r.paused_debug().is_some_and(|id| !r.session(id).unwrap().debug.as_ref().unwrap().locals().is_empty())
            })
        });
        found.then(|| cx.update(|_, cx| runs.read(cx).paused_debug().unwrap()))
    };
    let main = repo.join("src/main.js");
    let listed = pump_until(cx, 10, |cx| cx.update(|_, cx| !runs.read(cx).node_configs.is_empty()));
    assert!(listed);
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.toggle_breakpoint(&main, 2, cx);
            r.run_npm_script("start", true, cx);
        })
    });
    let started = std::time::Instant::now();
    let console = |runs: &gpui::Entity<Runs>, cx: &mut gpui::VisualTestContext| {
        cx.update(|_, cx| runs.read(cx).sessions.last().map(|s| s.console_text()).unwrap_or_default())
    };
    let Some(id) = real_paused(&runs, cx) else { panic!("the npm script stops: {}", console(&runs, cx)) };
    cx.update(|_, cx| {
        let d = runs.read(cx).session(id).unwrap().debug.as_ref().unwrap().snapshot();
        assert_eq!(d.exec_point(), Some((main.clone(), 2)));
        assert!(d.locals().iter().any(|(k, v)| k == "out" && v == "6"), "{:?}", d.locals());
        println!("npm script: stopped after {:?}, {} connections", started.elapsed(), d.targets.len());
    });
    cx.update(|_, cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
    let done = pump_until(cx, 60, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
    assert!(done, "{}", console(&runs, cx));
    let text = cx.update(|_, cx| runs.read(cx).session(id).unwrap().console_text());
    assert!(text.contains("tripled 6"), "{text}");
    // TypeScript, its breakpoint on the TypeScript line.
    let sum = repo.join("src/sum.ts");
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.toggle_breakpoint(&sum, 2, cx);
            r.run_node_file(&sum, true, cx);
        })
    });
    let Some(id) = real_paused(&runs, cx) else { panic!("the TypeScript file stops: {}", console(&runs, cx)) };
    cx.update(|_, cx| {
        let d = runs.read(cx).session(id).unwrap().debug.as_ref().unwrap().snapshot();
        assert_eq!(d.exec_point(), Some((sum.clone(), 2)));
        assert!(d.locals().iter().any(|(k, v)| k == "total" && v == "6"), "{:?}", d.locals());
    });
    cx.update(|_, cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
    let done = pump_until(cx, 60, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
    assert!(done);
    let text = cx.update(|_, cx| runs.read(cx).session(id).unwrap().console_text());
    assert!(text.contains("sum 6"), "{text}");
    // Attach to a Node started elsewhere with --inspect; Stop leaves it running.
    let tick = repo.join("src/tick.js");
    write(&repo, "src/tick.js", "let count = 0;\nfunction tick() {\n  count += 1;\n}\nsetInterval(tick, 100);\n");
    let mut program = std::process::Command::new(&n.bin)
        .arg("--inspect=127.0.0.1:9339")
        .arg(&tick)
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let attach = RunConfig {
        id: "user:attach".into(),
        kind: Kind::Node,
        name: "Attach".into(),
        port: Some(9339),
        user: true,
        ..Default::default()
    };
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.toggle_breakpoint(&tick, 2, cx);
            r.start(Launch::Config { config: attach, debug: true }, cx);
        })
    });
    let Some(id) = real_paused(&runs, cx) else { panic!("the attach stops: {}", console(&runs, cx)) };
    cx.update(|_, cx| {
        let d = runs.read(cx).session(id).unwrap().debug.as_ref().unwrap().snapshot();
        assert_eq!(d.exec_point(), Some((tick.clone(), 2)));
    });
    cx.update(|_, cx| runs.update(cx, |r, cx| r.stop(id, cx)));
    let done = pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
    assert!(done);
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert!(matches!(program.try_wait(), Ok(None)), "the program goes on after Stop");
    let _ = program.kill();
    println!("node {}: all three stopped and ran on", n.version);
    let _ = std::fs::remove_dir_all(repo);
}
