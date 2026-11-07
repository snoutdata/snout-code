//! Projects: which there are, opening one and closing it, and the New Project dialog. An agent that
//! made a project from the home folder opens it here.

use gpui::{Context, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, TOOL_ERROR};
use crate::model::Action;
use crate::workspace::Workspace;

pub const ALL: &[Capability] = &[
    Capability {
        name: "listProjects",
        domain: Domain::Project,
        effect: Effect::Read,
        description: "The project open now, and the projects the user opened lately, most recent first.",
        input: Input::None,
        run: Run::Own(list),
        consent: None,
    },
    Capability {
        name: "switchProject",
        domain: Domain::Project,
        effect: Effect::Change,
        description: "Open a folder as the project in this window, in place of the one open: any folder, with Git or \
                      not. The console then shows a new session in that project, and this session stays where it \
                      runs. Use it after making a project, so the user can work in it.",
        input: Input::Own(open_input),
        run: Run::Own(open),
        consent: Some(|a| format!("open the project {}", text_of(a, "path"))),
    },
    Capability {
        name: "closeProject",
        domain: Domain::Project,
        effect: Effect::Change,
        description: "Close the project open in this window. The user is asked about unsaved files.",
        input: Input::None,
        run: Run::Own(close),
        consent: Some(|_| "close the project".to_owned()),
    },
    Capability {
        name: "showNewProject",
        domain: Domain::Project,
        effect: Effect::Show,
        description: "Open the New Project dialog for the user to fill in. It makes nothing by itself.",
        input: Input::None,
        run: Run::Own(show_new),
        consent: None,
    },
];

fn open_input() -> Value {
    tools::schema(json!({ "path": { "type": "string", "description": "The folder to open, absolute" } }), &["path"])
}

fn list(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let m = ws.model.read(cx);
    let open = m.project_root().map(|p| p.to_string_lossy().into_owned());
    let recent: Vec<String> = m.recent_projects().iter().map(|p| p.to_string_lossy().into_owned()).collect();
    reply.ok(tools::json_text(&json!({ "open": open, "recent": recent })));
}

fn open(_: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(asked) = need(a, "path") else { return missing(reply, "path") };
    let host = crate::host::host(cx);
    let path = tools::path_of(asked, &host.info().home);
    let shown = path.clone();
    let there = cx.background_executor().spawn(async move { host.canonicalize(&shown).is_ok() });
    cx.spawn_in(window, async move |ws, cx| {
        if !there.await {
            let message = format!("There is no folder at {}", path.display());
            return reply.error(TOOL_ERROR, &message, None);
        }
        let name = path.display().to_string();
        let _ = ws.update(cx, |ws, cx| ws.send(Action::OpenPath(path), cx));
        reply.ok(tools::text(format!("Opening {name}. getAppState says when it is open.")));
    })
    .detach();
}

fn close(ws: &mut Workspace, _: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    if !ws.model.read(cx).project_open() {
        return reply.error(TOOL_ERROR, "No project is open", None);
    }
    ws.close_project(window, cx);
    reply.ok(tools::text("Closing the project"));
}

fn show_new(ws: &mut Workspace, _: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.open_new_project(window, cx);
    reply.ok(tools::text("The New Project dialog shows"));
}
