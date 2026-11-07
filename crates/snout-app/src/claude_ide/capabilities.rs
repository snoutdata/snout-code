//! The harness: what an agent in the console can ask of the app. One table says what there is, and
//! both the UTCP manual ([`super::api::manual`]) and `POST /api/<tool>` come from it, so a tool
//! cannot be offered and not answered, or answered and not offered.
//!
//! To add one: split showing from changing ([`Effect`]), write the description for a model that has
//! never seen this app, add the row to [`ALL`], and add the call to the test in `tests.rs` that
//! plays the agent. The rows' checks are below.

use gpui::{App, Context, Window};
use serde_json::{Value, json};

use super::server::{Call, Reply};
use super::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::panels::Activity;
use crate::workspace::{Tool, Workspace};

/// What a call does. Looking and showing never ask; a change is for the user to allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Reads, and nothing on screen moves.
    Read,
    /// Changes only what shows: a tab, a window. Nothing is written.
    Show,
    /// Writes something, or starts something.
    Change,
}

impl Effect {
    pub fn tag(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Show => "show",
            Effect::Change => "change",
        }
    }
}

/// The part of the app a tool belongs to, as the manual tags it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    App,
    Editor,
    Git,
}

impl Domain {
    pub fn tag(self) -> &'static str {
        match self {
            Domain::App => "app",
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
pub const ALL: &[Capability] = &[
    Capability {
        name: "getAppState",
        domain: Domain::App,
        effect: Effect::Read,
        description: "Where this session stands in the SnoutCode window: the project (none is open when the session \
                      runs in the home folder), its Git branch, the open files, the runs, the problems, the windows \
                      showing and the connection. Call it first when you need to know any of them.",
        input: Input::None,
        run: Run::Own(app_state),
    },
    Capability {
        name: "showToolWindow",
        domain: Domain::App,
        effect: Effect::Show,
        description: "Bring one of SnoutCode's windows to the front for the user to look at, by name. It stays if it \
                      already shows. It starts nothing and changes nothing.",
        input: Input::Own(show_tool_window_input),
        run: Run::Own(show_tool_window),
    },
    Capability {
        name: "openFile",
        domain: Domain::Editor,
        effect: Effect::Show,
        description: "Open a file in the editor, and select lines or text in it.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "getCurrentSelection",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The text selected in the editor in front, and where it is.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "getLatestSelection",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The last text selected in any tab, even if another tab is in front now.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "getOpenEditors",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The open tabs: path, language, whether each has unsaved changes, and the selection of the one \
                      in front.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "getWorkspaceFolders",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The project's folders.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "getDiagnostics",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "The errors and warnings the language engines report for a file, or for every file they have \
                      looked at.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "checkDocumentDirty",
        domain: Domain::Editor,
        effect: Effect::Read,
        description: "Whether a file open in the editor has unsaved changes.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "saveDocument",
        domain: Domain::Editor,
        effect: Effect::Change,
        description: "Save a file open in the editor, with the user's unsaved changes in it.",
        input: Input::Connection,
        run: Run::Connection,
    },
    Capability {
        name: "getCommitChange",
        domain: Domain::Git,
        effect: Effect::Read,
        description: "While SnoutCode waits for a drafted commit message: the change the commit would record, and how \
                      to write its message.",
        input: Input::None,
        run: Run::Connection,
    },
    Capability {
        name: "setCommitMessage",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Put the drafted commit message in SnoutCode's commit box, for the user to read and commit.",
        input: Input::Own(commit_message_input),
        run: Run::Connection,
    },
];

/// The capability called `name`.
pub fn find(name: &str) -> Option<&'static Capability> {
    ALL.iter().find(|c| c.name == name)
}

/// Answer a call of the API. The server let through only names in [`ALL`].
pub fn run(ws: &mut Workspace, call: Call, window: &mut Window, cx: &mut Context<Workspace>) {
    match find(&call.name).map(|c| c.run) {
        Some(Run::Own(runner)) => runner(ws, &call.arguments, call.reply, window, cx),
        Some(Run::Connection) => super::bridge::call_tool(ws, call, window, cx),
        None => {
            let message = format!("Tool not found: {}", call.name);
            call.reply.error(tools::METHOD_NOT_FOUND, &message, None);
        }
    }
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
        let mut names: Vec<&str> = ALL.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len(), "a name twice");
        for c in ALL {
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
        for c in ALL {
            let starts = |words: &[&str]| words.iter().any(|w| c.name.starts_with(w));
            match c.effect {
                Effect::Read => assert!(starts(&["get", "list", "find", "check"]), "{} reads", c.name),
                Effect::Show => assert!(starts(&["show", "open"]), "{} shows", c.name),
                Effect::Change => {
                    assert!(!starts(&["get", "list", "find", "check", "show", "open"]), "{} changes", c.name);
                }
            }
        }
    }

    #[test]
    fn a_tool_of_the_connection_is_described_by_it() {
        for c in ALL.iter().filter(|c| matches!(c.input, Input::Connection)) {
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
