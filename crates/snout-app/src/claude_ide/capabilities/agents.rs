//! The agents' own changes, as the Agent Changes window keeps them, for an agent to read and to settle.

use gpui::{Context, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::bridge;
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, INVALID_PARAMS};
use crate::workspace::Workspace;

pub const ALL: &[Capability] = &[
    Capability {
        name: "listAgentChanges",
        domain: Domain::Agents,
        effect: Effect::Read,
        description: "The files the agents changed in this session that the user has not reviewed yet: which agent, \
                      how many lines, and on which prompt.",
        input: Input::None,
        run: Run::Own(list),
        consent: None,
    },
    Capability {
        name: "acceptAgentChange",
        domain: Domain::Agents,
        effect: Effect::Change,
        description: "Keep an agent's changes to a file, so they no longer wait for review.",
        input: Input::Own(file_input),
        run: Run::Own(|ws, a, reply, window, cx| settle(ws, a, reply, false, window, cx)),
        consent: Some(|a| format!("keep the agent's changes to {}", text_of(a, "filePath"))),
    },
    Capability {
        name: "revertAgentChange",
        domain: Domain::Agents,
        effect: Effect::Change,
        description: "Put a file back as it was before the agent's changes in this session.",
        input: Input::Own(file_input),
        run: Run::Own(|ws, a, reply, window, cx| settle(ws, a, reply, true, window, cx)),
        consent: Some(|a| format!("revert the agent's changes to {}", text_of(a, "filePath"))),
    },
];

fn file_input() -> Value {
    tools::schema(json!({ "filePath": { "type": "string", "description": "The file, absolute" } }), &["filePath"])
}

fn list(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let changes = bridge::changes_of(ws, cx);
    let c = changes.read(cx);
    let files: Vec<Value> = c
        .set
        .files
        .iter()
        .map(|f| {
            let prompt = c.set.turn_of(f.turn).and_then(|t| t.prompt.clone());
            json!({
                "filePath": f.path.to_string_lossy(),
                "agent": f.agent,
                "added": f.added,
                "removed": f.removed,
                "created": f.created(),
                "prompt": prompt,
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "files": files })));
}

fn settle(ws: &mut Workspace, a: &Value, reply: Reply, revert: bool, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(file) = need(a, "filePath") else { return missing(reply, "filePath") };
    let path = tools::path_of(file, &crate::host::host(cx).info().home);
    if bridge::changes_of(ws, cx).read(cx).set.file(&path).is_none() {
        let message = format!("No agent's change waits on {file}. listAgentChanges has them.");
        return reply.error(INVALID_PARAMS, &message, None);
    }
    if revert {
        bridge::revert_file(ws, &path, window, cx);
        reply.ok(tools::text(format!("{file} is back as it was")));
    } else {
        bridge::accept_file(ws, &path, window, cx);
        reply.ok(tools::text(format!("The changes to {file} are kept")));
    }
}
