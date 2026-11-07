//! The harness: what an agent in the console can ask of the app. One table says what there is, and
//! both the UTCP manual ([`super::api::manual`]) and `POST /api/<tool>` come from it, so a tool
//! cannot be offered and not answered, or answered and not offered.
//!
//! To add one: split showing from changing ([`Effect`]), write the description for a model that has
//! never seen this app, add the row to [`ALL`], and add the call to the test in `tests.rs` that
//! plays the agent. The rows' checks are below.

pub mod agents;
pub mod app;
pub mod build;
pub mod code;
pub mod git;
pub mod projects;
pub mod run;
pub mod spring;

use std::collections::{HashSet, VecDeque};

use gpui::{App, Context, Window};
use serde_json::{Value, json};

use super::server::{Call, Reply};
use super::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::model::Dialog;
use crate::panels::Activity;
use crate::workspace::{Tool, Workspace};

/// What a call does. Looking and showing never ask; a change is for the user to allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Reads, and nothing on screen moves.
    Read,
    /// Changes only what shows: a tab, a window. Nothing is written.
    Show,
    /// Writes something, or starts something. The user is asked, unless they allowed this tool for
    /// the session.
    Change,
    /// A change the user has just asked for in the window (Draft's message): it does not ask again.
    Requested,
}

impl Effect {
    pub fn tag(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Show => "show",
            Effect::Change | Effect::Requested => "change",
        }
    }
}

/// The part of the app a tool belongs to, as the manual tags it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    App,
    Project,
    Run,
    Code,
    Build,
    Editor,
    Git,
    Agents,
    Spring,
}

impl Domain {
    pub fn tag(self) -> &'static str {
        match self {
            Domain::App => "app",
            Domain::Project => "project",
            Domain::Run => "run",
            Domain::Code => "code",
            Domain::Build => "build",
            Domain::Agents => "agents",
            Domain::Spring => "spring",
            Domain::Editor => "editor",
            Domain::Git => "git",
        }
    }
}

type Runner = fn(&mut Workspace, &Value, Reply, &mut Window, &mut Context<Workspace>);

/// Who answers a call.
#[derive(Clone, Copy)]
pub enum Run {
    /// The code that answers Claude Code's IDE connection, so the two never differ.
    Connection,
    Own(Runner),
}

/// The arguments a tool takes.
#[derive(Clone, Copy)]
pub enum Input {
    None,
    /// As the IDE connection's tool of that name describes them.
    Connection,
    Own(fn() -> Value),
}

/// One thing an agent can ask of the app.
pub struct Capability {
    pub name: &'static str,
    pub domain: Domain,
    pub effect: Effect,
    /// For a model that has never seen this app.
    pub description: &'static str,
    pub input: Input,
    pub run: Run,
    /// What the card asks, for a person answering in a hurry: "save /p/A.java". Every
    /// [`Effect::Change`] has one.
    pub consent: Option<fn(&Value) -> String>,
}

impl Capability {
    /// The JSON schema of its arguments.
    pub fn schema(&self) -> Value {
        match self.input {
            Input::None => tools::schema(json!({}), &[]),
            Input::Connection => tools::input_of(self.name).unwrap_or_else(|| tools::schema(json!({}), &[])),
            Input::Own(schema) => schema(),
        }
    }

    /// The tags the manual gives it: its domain and its effect.
    pub fn tags(&self) -> [&'static str; 2] {
        [self.domain.tag(), self.effect.tag()]
    }
}

/// All of them, in the order the manual lists them.
pub fn all() -> impl Iterator<Item = &'static Capability> {
    [CORE, app::ALL, projects::ALL, run::ALL, code::ALL, build::ALL, git::ALL, agents::ALL, spring::ALL]
        .into_iter()
        .flatten()
}

const CORE: &[Capability] = &[
    Capability {
        name: "getAppState",
        domain: Domain::App,
        effect: Effect::Read,
        description: "Where this session stands in the SnoutCode window: the project (none is open when the session \
                      runs in the home folder), its Git branch, the open files, the runs, the problems, the windows \
                      showing and the connection. Call it first when you need to know any of them.",
        input: Input::None,
        run: Run::Own(app_state),
        consent: None,
    },
    Capability {
        name: "showToolWindow",
        domain: Domain::App,
        effect: Effect::Show,
        description: "Bring one of SnoutCode's windows to the front for the user to look at, by name. It stays if it \
                      already shows. It starts nothing and changes nothing.",
        input: Input::Own(show_tool_window_input),
        run: Run::Own(show_tool_window),
        consent: None,
    },
    Capability {
        name: "openFile",
        domain: Domain::Editor,
        effect: Effect::Show,
        description: "Open a file in the editor, and select lines or text in it.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "getCurrentSelection",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The text selected in the editor in front, and where it is.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "getLatestSelection",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The last text selected in any tab, even if another tab is in front now.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "getOpenEditors",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The open tabs: path, language, whether each has unsaved changes, and the selection of the one \
                      in front.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "getWorkspaceFolders",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The project's folders.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "getDiagnostics",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The errors and warnings the language engines report for a file, or for every file they have \
                      looked at.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "checkDocumentDirty",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "Whether a file open in the editor has unsaved changes.",
        input: Input::Connection,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "saveDocument",
        domain: Domain::Editor,
        effect: Effect::Change,
        description: "Save a file open in the editor, with the user's unsaved changes in it.",
        input: Input::Connection,
        run: Run::Connection,
        consent: Some(|a| format!("save {}", text_of(a, "filePath"))),
    },
    Capability {
        name: "getCommitChange",
        domain: Domain::Git,
        effect: Effect::Read,
        description: "While SnoutCode waits for a drafted commit message: the change the commit would record, and how \
                      to write its message.",
        input: Input::None,
        run: Run::Connection,
        consent: None,
    },
    Capability {
        name: "setCommitMessage",
        domain: Domain::Git,
        effect: Effect::Requested,
        description: "Put the drafted commit message in SnoutCode's commit box, for the user to read and commit.",
        input: Input::Own(commit_message_input),
        run: Run::Connection,
        consent: None,
    },
];

/// The capability called `name`.
pub fn find(name: &str) -> Option<&'static Capability> {
    all().find(|c| c.name == name)
}

/// The required argument `name` of a call.
fn need<'a>(a: &'a Value, name: &str) -> Option<&'a str> {
    a.get(name).and_then(Value::as_str).filter(|v| !v.trim().is_empty())
}

/// Answer a call that lacks the argument `name`.
fn missing(reply: Reply, name: &str) {
    reply.error(INVALID_PARAMS, &format!("Missing {name}"), None);
}

/// The argument `name` of a call, as text, for a card.
fn text_of<'a>(a: &'a Value, name: &str) -> &'a str {
    a.get(name).and_then(Value::as_str).unwrap_or("?")
}

/// A call waiting for the user's answer.
struct Ask {
    capability: &'static Capability,
    call: Call,
}

/// What the window remembers of the agents' calls: what the user allowed, and what waits for them.
#[derive(Default)]
pub struct Harness {
    /// The tools the user allowed for the session.
    allowed: HashSet<&'static str>,
    /// The calls waiting, the one the card shows first.
    asks: VecDeque<Ask>,
    /// The card shows.
    asking: bool,
}

impl Harness {
    /// Nothing waits for the user.
    pub fn is_idle(&self) -> bool {
        self.asks.is_empty() && !self.asking
    }
}

/// What the user answered on the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskAnswer {
    Once,
    Session,
}

/// The answer to a call the user turned down, or closed the card on.
const REFUSED: &str = "The user did not allow this. Do not try it again unless they ask you to.";

/// Answer a call of the API. The server let through only names in [`ALL`].
pub fn run(ws: &mut Workspace, call: Call, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(capability) = find(&call.name) else {
        let message = format!("Tool not found: {}", call.name);
        return call.reply.error(tools::METHOD_NOT_FOUND, &message, None);
    };
    let agent = crate::agents::console_agent(cx).short.to_string();
    let id = ws.agent_log.update(cx, |log, cx| log.begin(agent, capability, &call.arguments, cx));
    let answered = ws.claude.as_ref().map(|b| b.events.clone());
    let Call { client, name, arguments, reply } = call;
    let reply = reply.observed(move |message| {
        let error = message.get("error").map(|e| {
            let text = [e["message"].as_str(), e["data"].as_str()].into_iter().flatten();
            text.collect::<Vec<_>>().join(": ")
        });
        if let Some(events) = answered {
            let _ = events.unbounded_send(super::server::Event::Answered { id, error });
        }
    });
    let call = Call { client, name, arguments, reply };
    if capability.effect == Effect::Change && !ws.harness.allowed.contains(capability.name) {
        ws.harness.asks.push_back(Ask { capability, call });
        return next_ask(ws, cx);
    }
    answer(capability, ws, call, window, cx);
}

fn answer(c: &Capability, ws: &mut Workspace, call: Call, window: &mut Window, cx: &mut Context<Workspace>) {
    match c.run {
        Run::Own(runner) => runner(ws, &call.arguments, call.reply, window, cx),
        Run::Connection => super::bridge::call_tool(ws, call, window, cx),
    }
}

/// Show the card for the next call waiting, if nothing else is in the way. A card closed with no
/// answer turns the call down.
pub fn next_ask(ws: &mut Workspace, cx: &mut Context<Workspace>) {
    let showing = matches!(ws.model.read(cx).dialog, Some(Dialog::AgentAsks { .. }));
    if ws.harness.asking && !showing {
        ws.harness.asking = false;
        if let Some(ask) = ws.harness.asks.pop_front() {
            ask.call.reply.error(TOOL_ERROR, REFUSED, None);
        }
    }
    if ws.harness.asking || ws.model.read(cx).dialog.is_some() {
        return;
    }
    let Some(ask) = ws.harness.asks.front() else { return };
    let what = ask.capability.consent.map_or_else(|| ask.capability.name.to_owned(), |c| c(&ask.call.arguments));
    let agent = crate::agents::console_agent(cx).name.to_string();
    let tool = ask.capability.name.to_owned();
    ws.harness.asking = true;
    ws.model.update(cx, |m, cx| {
        m.dialog = Some(Dialog::AgentAsks { agent, what, tool });
        cx.emit(crate::model::ModelEvent::Dialog);
        cx.notify();
    });
}

/// The user answered the card: the call goes ahead.
pub fn asked(ws: &mut Workspace, how: AskAnswer, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.harness.asking = false;
    if let Some(Ask { capability, call }) = ws.harness.asks.pop_front() {
        if how == AskAnswer::Session {
            ws.harness.allowed.insert(capability.name);
        }
        answer(capability, ws, call, window, cx);
    }
    // Calls of a tool just allowed need no card of their own.
    let mut ahead = Vec::new();
    while let Some(at) = ws.harness.asks.iter().position(|a| ws.harness.allowed.contains(a.capability.name)) {
        if let Some(ask) = ws.harness.asks.remove(at) {
            ahead.push(ask);
        }
    }
    for Ask { capability, call } in ahead {
        answer(capability, ws, call, window, cx);
    }
    next_ask(ws, cx);
}

fn commit_message_input() -> Value {
    tools::schema(json!({ "message": { "type": "string", "description": "The commit message" } }), &["message"])
}

// ----- Where the session stands --------------------------------------------------------

fn app_state(ws: &mut Workspace, _: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let state = app_state_of(ws, window, cx);
    reply.ok(tools::json_text(&state));
}

fn app_state_of(ws: &Workspace, window: &Window, cx: &App) -> Value {
    let host = crate::host::host(cx);
    let m = ws.model.read(cx);
    let project = m.project_root().map(|root| {
        let git = m.branch_summary().map(|branch| {
            json!({
                "branch": branch,
                "changedFiles": m.project_status().files.len(),
            })
        });
        json!({
            "path": root.to_string_lossy(),
            "name": crate::model::Model::dir_name(&root),
            // Remembered from last time, and still being read.
            "opening": !m.project_open(),
            "git": git,
            "gitTrouble": m.no_git.as_ref().and_then(|n| n.trouble.clone()),
        })
    });
    let active = ws.active_editor(window, cx).map(|e| e.entity_id());
    let files: Vec<Value> = ws
        .editors(cx)
        .into_iter()
        .map(|editor| {
            let is_active = Some(editor.entity_id()) == active;
            let e = editor.read(cx);
            json!({ "path": e.path.to_string_lossy(), "active": is_active, "unsaved": e.dirty })
        })
        .collect();
    let runs: Vec<Value> = ws
        .runs
        .read(cx)
        .sessions
        .iter()
        .map(|s| {
            json!({
                "title": s.title,
                "window": format!("{:?}", s.place()),
                "running": s.running(),
            })
        })
        .collect();
    let (mut errors, mut warnings) = (0, 0);
    for (_, d) in ws.hub.read(cx).all_problems() {
        match d.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => errors += 1,
            Some(lsp_types::DiagnosticSeverity::WARNING) => warnings += 1,
            _ => {}
        }
    }
    let showing: Vec<&str> = WINDOWS.iter().filter(|(_, target)| target.shows(ws, cx)).map(|(name, _)| *name).collect();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "project": project,
        "homeFolder": host.info().home.to_string_lossy(),
        "connection": crate::remote::connection(cx).map(|(name, _)| name),
        "agent": crate::agents::console_agent(cx).name.to_string(),
        "openFiles": files,
        "runs": runs,
        "problems": { "errors": errors, "warnings": warnings },
        "windowsShowing": showing,
    })
}

// ----- The windows ---------------------------------------------------------------------

/// A window `showToolWindow` can bring forward.
#[derive(Clone, Copy)]
enum Target {
    Side(Activity),
    Tool(Tool),
}

impl Target {
    fn shows(self, ws: &Workspace, cx: &App) -> bool {
        match self {
            Target::Side(activity) => ws.side.read(cx).activity == activity,
            Target::Tool(tool) => ws.tool_open(tool, cx),
        }
    }

    fn show(self, ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
        match self {
            Target::Side(activity) => ws.show_activity(activity, window, cx),
            Target::Tool(tool) => ws.show_tool(tool, window, cx),
        }
    }
}

/// The windows, by the names the user sees.
const WINDOWS: &[(&str, Target)] = &[
    ("Project", Target::Side(Activity::Project)),
    ("Git", Target::Side(Activity::Git)),
    ("Run", Target::Tool(Tool::Run)),
    ("Debug", Target::Tool(Tool::Debug)),
    ("Build", Target::Tool(Tool::Build)),
    ("Output", Target::Tool(Tool::Output)),
    ("Terminal", Target::Tool(Tool::Terminal)),
    ("Problems", Target::Tool(Tool::Problems)),
    ("Console", Target::Tool(Tool::Claude)),
    ("Gradle", Target::Tool(Tool::Gradle)),
    ("Maven", Target::Tool(Tool::Maven)),
    ("Spring", Target::Tool(Tool::Spring)),
    ("Tests", Target::Tool(Tool::Tests)),
    ("Agent Changes", Target::Tool(Tool::ClaudeChanges)),
    ("Structure", Target::Tool(Tool::Structure)),
    ("Hierarchy", Target::Tool(Tool::Hierarchy)),
    ("Notifications", Target::Tool(Tool::Notifications)),
    ("Processes", Target::Tool(Tool::Processes)),
    ("Agent Log", Target::Tool(Tool::AgentLog)),
    ("Agent View", Target::Tool(Tool::AgentView)),
];

fn window_names() -> Vec<&'static str> {
    WINDOWS.iter().map(|(name, _)| *name).collect()
}

fn show_tool_window_input() -> Value {
    tools::schema(
        json!({ "name": { "type": "string", "enum": window_names(), "description": "The window to show" } }),
        &["name"],
    )
}

fn show_tool_window(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let names = || format!("The windows: {}", window_names().join(", "));
    let Some(asked) = a.get("name").and_then(Value::as_str) else {
        return reply.error(INVALID_PARAMS, "Which window? Missing name", Some(names()));
    };
    let Some((name, target)) = WINDOWS.iter().find(|(name, _)| name.eq_ignore_ascii_case(asked.trim())) else {
        return reply.error(INVALID_PARAMS, &format!("No window called {asked}"), Some(names()));
    };
    target.show(ws, window, cx);
    if target.shows(ws, cx) {
        reply.ok(tools::text(format!("The {name} window shows")));
    } else {
        // A Gradle window with no Gradle in the project, say.
        reply.error(TOOL_ERROR, &format!("The {name} window is not available in this project"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_capability_is_fully_described_and_named_once() {
        let mut names: Vec<&str> = all().map(|c| c.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "a name twice");
        for c in all() {
            assert!(c.description.ends_with('.'), "{}: a whole sentence", c.name);
            assert!(!c.description.contains('\u{2014}'), "{}: no dashes in what the model reads", c.name);
            let s = c.schema();
            assert_eq!(s["type"], "object", "{}", c.name);
            assert_eq!(s["additionalProperties"], false, "{}", c.name);
            for field in s["properties"].as_object().into_iter().flatten() {
                assert!(field.1["type"].is_string(), "{}: {} has a type", c.name, field.0);
            }
        }
    }

    /// Showing and changing are separate tools: the name says which, so asking to look can never
    /// also write, and a change can never pass for a look.
    #[test]
    fn a_tools_name_says_what_it_does() {
        for c in all() {
            let starts = |words: &[&str]| words.iter().any(|w| c.name.starts_with(w));
            match c.effect {
                Effect::Read => assert!(starts(&["get", "list", "find", "check"]), "{} reads", c.name),
                Effect::Show => assert!(starts(&["show", "open"]), "{} shows", c.name),
                Effect::Change | Effect::Requested => {
                    assert!(!starts(&["get", "list", "find", "check", "show", "open"]), "{} changes", c.name);
                }
            }
        }
    }

    #[test]
    fn what_changes_something_says_so_on_the_card() {
        for c in all() {
            assert_eq!(c.effect == Effect::Change, c.consent.is_some(), "{}", c.name);
        }
        let save = find("saveDocument").unwrap().consent.unwrap();
        assert_eq!(save(&json!({ "filePath": "/p/A.java" })), "save /p/A.java");
    }

    #[test]
    fn a_tool_of_the_connection_is_described_by_it() {
        for c in all().filter(|c| matches!(c.input, Input::Connection)) {
            assert!(tools::input_of(c.name).is_some(), "{} is not among the connection's tools", c.name);
        }
    }

    #[test]
    fn the_windows_have_one_name_each() {
        let mut names = window_names();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), WINDOWS.len());
    }
}
