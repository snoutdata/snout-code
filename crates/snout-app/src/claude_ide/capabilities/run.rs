//! Running: the configurations, starting and stopping them, what a run printed and how its tests
//! went.

use std::path::{Path, PathBuf};

use gpui::{Context, Entity, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::playwright::report::{State, Test, TestError};
use crate::run::test_tree::TestState;
use crate::run::{Launch, Runs};
use crate::workspace::Workspace;

/// How many lines of a run's output a call returns, unless it asks for fewer.
const MAX_LINES: usize = 2000;
/// How many lines it returns unless asked.
const LINES: usize = 200;
/// How long a run that finds its program first may take to show.
const WAIT_TICK: std::time::Duration = std::time::Duration::from_millis(100);
const WAIT_TICKS: usize = 200;
/// How many of a trace's actions a call returns: the last ones, nearest the failure.
const TRACE_ACTIONS: usize = 100;

pub const ALL: &[Capability] = &[
    Capability {
        name: "listRunConfigurations",
        domain: Domain::Run,
        effect: Effect::Read,
        description: "What the project can run: its main classes, Gradle and Maven tasks, tests and package.json \
                      scripts, and the one selected. Empty until the project's build has been read: try again shortly.",
        input: Input::None,
        run: Run::Own(configurations),
        consent: None,
    },
    Capability {
        name: "listRuns",
        domain: Domain::Run,
        effect: Effect::Read,
        description: "The runs in the Run, Debug and Build windows, with their ids, whether each still runs, and how \
                      its tests went.",
        input: Input::None,
        run: Run::Own(runs),
        consent: None,
    },
    Capability {
        name: "startRun",
        domain: Domain::Run,
        effect: Effect::Change,
        description: "Start a run configuration, by its id or name, in the Run window, where the user sees it. Answers \
                      with the run's id; it does not wait for the run to end. Follow it with listRuns and getRunOutput.",
        input: Input::Own(start_input),
        run: Run::Own(start),
        consent: Some(|a| {
            let how = if a["debug"] == true { "debug" } else { "run" };
            format!("{how} {}", text_of(a, "configuration"))
        }),
    },
    Capability {
        name: "stopRun",
        domain: Domain::Run,
        effect: Effect::Change,
        description: "Stop a run by its id.",
        input: Input::Own(id_input),
        run: Run::Own(stop),
        consent: Some(|a| format!("stop run {}", a["id"])),
    },
    Capability {
        name: "getRunOutput",
        domain: Domain::Run,
        effect: Effect::Read,
        description: "What a run printed: its last lines. By id, or the run in front of the Run window.",
        input: Input::Own(output_input),
        run: Run::Own(output),
        consent: None,
    },
    Capability {
        name: "getTestResults",
        domain: Domain::Run,
        effect: Effect::Read,
        description: "How a run's tests went: the counts, and each failed test with its message and where it is. By id, \
                      or the run in front of the Run window. A failed Playwright test names its trace: \
                      getPlaywrightTrace reads it.",
        input: Input::Own(id_optional),
        run: Run::Own(test_results),
        consent: None,
    },
    Capability {
        name: "runPlaywrightTests",
        domain: Domain::Run,
        effect: Effect::Change,
        description: "Run a Playwright project's tests in the Tests window: all of them, one browser project, a file, or \
                      the test at a line of it. Answers with the run's id; it does not wait for the run to end. Follow \
                      it with listRuns and getTestResults.",
        input: Input::Own(playwright_input),
        run: Run::Own(run_playwright),
        consent: Some(|a| match a["file"].as_str() {
            Some(file) => format!("run the Playwright tests in {file}"),
            None => "run the Playwright tests".to_owned(),
        }),
    },
    Capability {
        name: "getPlaywrightTrace",
        domain: Domain::Run,
        effect: Effect::Read,
        description: "What a Playwright test's trace saw: the browser's actions with their errors, the requests that \
                      failed, and the console's errors. By part of the test's name, a failed one first. By the run's \
                      id, or the run in front.",
        input: Input::Own(trace_input),
        run: Run::Own(trace),
        consent: None,
    },
    Capability {
        name: "recordPlaywrightTest",
        domain: Domain::Run,
        effect: Effect::Change,
        description: "Start Playwright's recorder at the caret of the spec in front, opening its browser for the user \
                      to click through. What they do is written into the spec. Use openFile first to place the caret. \
                      Answers once the recorder runs.",
        input: Input::None,
        run: Run::Own(|ws, _, reply, window, cx| {
            if ws.recording_state().is_some() {
                return reply.error(TOOL_ERROR, "A recording is going: the user stops it above the editor.", None);
            }
            ws.record_at_cursor(window, cx);
            answer_when_started(Workspace::recording_state, "The recorder runs.", reply, cx);
        }),
        consent: Some(|_| "open a browser and record a Playwright test".to_owned()),
    },
    Capability {
        name: "pickPlaywrightLocator",
        domain: Domain::Run,
        effect: Effect::Change,
        description: "Open Playwright's browser for the user to pick an element; its locator goes in at the caret of the \
                      spec in front. Answers once the browser runs.",
        input: Input::None,
        run: Run::Own(|ws, _, reply, window, cx| {
            if ws.picking_state().is_some() {
                return reply.error(TOOL_ERROR, "Picking is on: the user stops it above the editor.", None);
            }
            ws.pick_locator(window, cx);
            answer_when_started(Workspace::picking_state, "The browser runs.", reply, cx);
        }),
        consent: Some(|_| "open a browser to pick a Playwright locator".to_owned()),
    },
];

fn playwright_input() -> Value {
    tools::schema(
        json!({
            "file": { "type": "string", "description": "A spec, absolute. All of them by default" },
            "line": { "type": "integer", "description": "The line of a test or describe in it, from 1" },
            "project": { "type": "string", "description": "A browser project of the configuration" },
        }),
        &[],
    )
}

fn trace_input() -> Value {
    tools::schema(
        json!({
            "test": { "type": "string", "description": "Part of the test's name, in any case" },
            "id": { "type": "integer", "description": "The run's id, from listRuns" },
        }),
        &["test"],
    )
}

/// Answer once what was asked for has started, or with what the window told the user instead.
fn answer_when_started(
    state: fn(&Workspace) -> Option<bool>,
    started: &'static str,
    reply: Reply,
    cx: &mut Context<Workspace>,
) {
    cx.spawn(async move |this, cx| {
        for _ in 0..WAIT_TICKS {
            let Ok(now) = this.read_with(cx, |w, _| state(w)) else {
                return reply.error(TOOL_ERROR, "The window closed.", None);
            };
            match now {
                Some(true) => return reply.ok(tools::text(started)),
                Some(false) => {}
                None => {
                    let why = this.read_with(cx, |w, cx| w.model.read(cx).message.clone()).unwrap_or_default();
                    return reply.error(TOOL_ERROR, &why, None);
                }
            }
            cx.background_executor().timer(WAIT_TICK).await;
        }
        reply.error(TOOL_ERROR, "It did not start. The Output window may say why.", None);
    })
    .detach();
}

/// Answer with the run started after `before`. Node and Playwright are found first, off the UI
/// thread, so it shows a moment later.
fn answer_with_new_run(runs: Entity<Runs>, before: u64, title: String, reply: Reply, cx: &mut Context<Workspace>) {
    cx.spawn(async move |_, cx| {
        for _ in 0..WAIT_TICKS {
            let id = cx.update(|cx| runs.read(cx).sessions.iter().map(|s| s.id).filter(|id| *id > before).max());
            if let Some(id) = id {
                return reply.ok(tools::json_text(&json!({ "id": id, "title": title })));
            }
            cx.background_executor().timer(WAIT_TICK).await;
        }
        reply.error(TOOL_ERROR, &format!("{title} did not start. The Output window may say why."), None);
    })
    .detach();
}

fn run_playwright(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    use crate::playwright::run::Scope;
    let tests = ws.pw_tests.clone();
    let has_tests = tests.read(cx).playwright || tests.read(cx).unit(cx).is_some();
    if a["file"].is_null() && !has_tests {
        return reply.error(TOOL_ERROR, "This project has no Playwright tests.", None);
    }
    let project = a["project"].as_str().map(str::to_owned);
    let scope = match a["file"].as_str() {
        Some(file) => Scope::At {
            file: tools::path_of(file, &crate::host::host(cx).info().home),
            line: a["line"].as_u64().map(|l| l as u32),
            project,
        },
        None => project.map_or(Scope::All, Scope::Project),
    };
    let runs = ws.runs.clone();
    let before = runs.read(cx).sessions.iter().map(|s| s.id).max().unwrap_or(0);
    tests.update(cx, |t, cx| t.start(scope, cx));
    answer_with_new_run(runs, before, "The tests".to_owned(), reply, cx);
}

fn start_input() -> Value {
    tools::schema(
        json!({
            "configuration": { "type": "string", "description": "A configuration's id or name, from listRunConfigurations" },
            "debug": { "type": "boolean", "description": "Run it under the debugger, in the Debug window", "default": false },
        }),
        &["configuration"],
    )
}

fn id_input() -> Value {
    tools::schema(json!({ "id": { "type": "integer", "description": "The run's id, from listRuns" } }), &["id"])
}

fn id_optional() -> Value {
    tools::schema(json!({ "id": { "type": "integer", "description": "The run's id, from listRuns" } }), &[])
}

fn output_input() -> Value {
    tools::schema(
        json!({
            "id": { "type": "integer", "description": "The run's id, from listRuns" },
            "lastLines": { "type": "integer", "description": "How many of the last lines", "default": LINES },
        }),
        &[],
    )
}

fn configurations(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let runs = ws.runs.clone();
    if runs.read(cx).configurations().is_empty() {
        runs.update(cx, |r, cx| r.load_configurations(cx));
    }
    let r = runs.read(cx);
    let selected = r.selected_config().map(|c| c.id);
    let list: Vec<Value> = r
        .configurations()
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "name": c.name,
                "kind": format!("{:?}", c.kind),
                "module": c.module,
                "selected": Some(&c.id) == selected.as_ref(),
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "configurations": list, "loading": r.configs_loading })));
}

fn runs(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let list: Vec<Value> = ws
        .runs
        .read(cx)
        .sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "window": format!("{:?}", s.place()),
                "running": s.running(),
                "status": s.status(),
                "tests": s.test_summary(),
                "errors": s.problems.0,
                "warnings": s.problems.1,
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "runs": list })));
}

fn start(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(asked) = need(a, "configuration") else { return missing(reply, "configuration") };
    let debug = a["debug"] == true;
    let runs = ws.runs.clone();
    let all = runs.read(cx).configurations();
    let needle = asked.trim();
    let found =
        all.iter().find(|c| c.id == needle).or_else(|| all.iter().find(|c| c.name.eq_ignore_ascii_case(needle)));
    let Some(config) = found.cloned() else {
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        let list = if names.is_empty() { "none yet".to_owned() } else { names.join(", ") };
        let message = format!("No configuration called {asked}");
        return reply.error(INVALID_PARAMS, &message, Some(format!("The configurations: {list}")));
    };
    let name = config.name.clone();
    let before = runs.read(cx).sessions.iter().map(|s| s.id).max().unwrap_or(0);
    if let Some(id) = runs.update(cx, |r, cx| r.start(Launch::Config { config, debug }, cx)) {
        return reply.ok(tools::json_text(&json!({ "id": id, "title": name })));
    }
    answer_with_new_run(runs, before, name, reply, cx);
}

fn stop(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(id) = a["id"].as_u64() else {
        return reply.error(INVALID_PARAMS, "Which run? Missing id", None);
    };
    let runs = ws.runs.clone();
    match runs.read(cx).session(id).map(|s| s.running()) {
        None => reply.error(TOOL_ERROR, &format!("No run {id}"), None),
        Some(false) => reply.ok(tools::text(format!("Run {id} has already ended"))),
        Some(true) => {
            runs.update(cx, |r, cx| r.stop(id, cx));
            reply.ok(tools::text(format!("Stopping run {id}")));
        }
    }
}

/// The session asked for: by `id`, else the one in front of the Run window.
fn session_of<'a>(ws: &Workspace, a: &Value, cx: &'a gpui::App) -> Option<&'a crate::run::Session> {
    let runs = ws.runs.read(cx);
    match a["id"].as_u64() {
        Some(id) => runs.session(id),
        None => runs.shown(crate::run::Place::Run),
    }
}

fn output(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(s) = session_of(ws, a, cx) else {
        return reply.error(TOOL_ERROR, "No such run. listRuns has them.", None);
    };
    let wanted = a["lastLines"].as_u64().map_or(LINES, |n| (n as usize).clamp(1, MAX_LINES));
    let text = s.console_text();
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.len().saturating_sub(wanted);
    reply.ok(tools::json_text(&json!({
        "id": s.id,
        "running": s.running(),
        "status": s.status(),
        "omittedLines": from + s.dropped,
        "output": lines[from..].join("\n"),
    })));
}

fn test_results(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(s) = session_of(ws, a, cx) else {
        return reply.error(TOOL_ERROR, "No such run. listRuns has them.", None);
    };
    if let Some(run) = s.playwright.as_ref() {
        let c = run.counts();
        let errors: Vec<String> = run.errors.iter().map(TestError::text).collect();
        let failures: Vec<Value> = run
            .order
            .iter()
            .filter_map(|id| run.tests.get(id))
            .filter(|t| t.state(run.over) == State::Failed)
            .map(|t| {
                let last = t.last_try();
                let (file, line) = run.location(&t.id).unwrap_or_else(|| (t.file.clone(), t.line));
                json!({
                    "title": t.path.join(" > "),
                    "project": t.project,
                    "file": file,
                    "line": line,
                    "errors": last.map(|l| l.errors.iter().map(TestError::text).collect::<Vec<_>>()),
                    "trace": last.and_then(|l| l.trace()),
                })
            })
            .collect();
        return reply.ok(tools::json_text(&json!({
            "id": s.id,
            "running": s.running(),
            "passed": c.passed,
            "failed": c.failed,
            "flaky": c.flaky,
            "skipped": c.skipped,
            "stillRunning": c.running,
            "status": run.status,
            "errors": errors,
            "failures": failures,
        })));
    }
    let Some(tree) = s.tests.as_ref() else {
        return reply.ok(tools::json_text(&json!({ "id": s.id, "running": s.running(), "tests": Value::Null })));
    };
    let (passed, failed, skipped, running) = tree.counts();
    let failures: Vec<Value> = tree
        .nodes
        .values()
        .filter(|n| n.state == TestState::Failed && !n.suite)
        .map(|n| {
            json!({
                "name": n.name,
                "class": n.class_name,
                "message": n.message,
                "stack": n.stack.as_deref().map(|t| t.lines().take(30).collect::<Vec<_>>().join("\n")),
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({
        "id": s.id,
        "running": s.running(),
        "passed": passed,
        "failed": failed,
        "skipped": skipped,
        "stillRunning": running,
        "failures": failures,
    })));
}

/// The test whose name holds `wanted`, a failed one first, and the path of its trace.
fn trace_of(run: &crate::playwright::report::Run, wanted: &str) -> Option<(String, Option<PathBuf>)> {
    let wanted = wanted.to_lowercase();
    let mut found: Vec<&Test> = run
        .order
        .iter()
        .filter_map(|id| run.tests.get(id))
        .filter(|t| t.path.join(" > ").to_lowercase().contains(&wanted))
        .collect();
    found.sort_by_key(|t| t.state(run.over) != State::Failed);
    let t = found.first()?;
    Some((t.path.join(" > "), t.last_try().and_then(|l| l.trace()).map(Path::to_path_buf)))
}

fn trace(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(wanted) = need(a, "test") else { return missing(reply, "test") };
    let Some(s) = session_of(ws, a, cx) else {
        return reply.error(TOOL_ERROR, "No such run. listRuns has them.", None);
    };
    let Some(run) = s.playwright.as_ref() else {
        return reply.error(TOOL_ERROR, "That run is not a Playwright run.", None);
    };
    let Some((title, path)) = trace_of(run, wanted) else {
        return reply.error(TOOL_ERROR, &format!("No test called {wanted} in that run."), None);
    };
    let Some(path) = path else {
        let why = format!("{title} has no trace: the project's configuration turns it on with `trace`.");
        return reply.error(TOOL_ERROR, &why, None);
    };
    // Off the UI thread, from the machine the project is on.
    let host = crate::host::host(cx);
    let read = cx.background_executor().spawn(async move {
        let bytes = host.read(&path)?;
        crate::playwright::trace::of_bytes(bytes)
    });
    cx.spawn(async move |_, _| match read.await {
        Err(e) => reply.error(TOOL_ERROR, &format!("Its trace could not be read: {e}"), None),
        Ok(t) => {
            let from = t.actions.len().saturating_sub(TRACE_ACTIONS);
            let actions: Vec<Value> =
                t.actions[from..].iter().map(|a| json!({ "title": a.title, "error": a.error })).collect();
            let failed: Vec<Value> = t
                .failed_requests
                .iter()
                .map(|r| json!({ "method": r.method, "url": r.url, "status": r.status }))
                .collect();
            reply.ok(tools::json_text(&json!({
                "test": title,
                "omittedActions": from,
                "actions": actions,
                "failedRequests": failed,
                "consoleErrors": t.console_errors,
            })));
        }
    })
    .detach();
}
