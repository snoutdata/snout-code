//! Spring, as the Java engine reads it: the project's beans and endpoints.

use gpui::{Context, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools;
use crate::workspace::Workspace;

/// How many a call returns at most.
const MAX: usize = 300;

pub const ALL: &[Capability] = &[
    Capability {
        name: "listBeans",
        domain: Domain::Spring,
        effect: Effect::Read,
        description: "The Spring beans of the project, as the Java engine reads them: name, type, where it is declared \
                      and its stereotype. Filter by part of a name or type.",
        input: Input::Own(filter_input),
        run: Run::Own(beans),
        consent: None,
    },
    Capability {
        name: "listEndpoints",
        domain: Domain::Spring,
        effect: Effect::Read,
        description: "The HTTP endpoints of the project's Spring code: path, methods and handler. Filter by part of a \
                      path or handler.",
        input: Input::Own(filter_input),
        run: Run::Own(endpoints),
        consent: None,
    },
];

fn filter_input() -> Value {
    tools::schema(json!({ "filter": { "type": "string", "description": "Part of what to find, in any case" } }), &[])
}

fn wanted(a: &Value) -> String {
    a["filter"].as_str().unwrap_or_default().trim().to_lowercase()
}

fn beans(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let filter = wanted(a);
    let spring = ws.spring.read(cx);
    let found: Vec<Value> = spring
        .modules
        .iter()
        .flat_map(|m| m.beans.iter().map(move |b| (m, b)))
        .filter(|(_, b)| {
            filter.is_empty() || b.name.to_lowercase().contains(&filter) || b.ty.to_lowercase().contains(&filter)
        })
        .take(MAX)
        .map(|(m, b)| {
            json!({
                "module": m.module,
                "name": b.name,
                "type": b.ty,
                "stereotype": b.stereotype,
                "declaringType": b.declaring_type,
                "method": b.method,
                "uri": b.uri,
                "active": b.active,
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "loaded": spring.loaded, "count": found.len(), "beans": found })));
}

fn endpoints(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let filter = wanted(a);
    let spring = ws.spring.read(cx);
    let found: Vec<Value> = spring
        .endpoints
        .iter()
        .filter(|e| {
            filter.is_empty()
                || e.path.to_lowercase().contains(&filter)
                || e.handler.class.to_lowercase().contains(&filter)
                || e.handler.method.to_lowercase().contains(&filter)
        })
        .take(MAX)
        .map(|e| {
            json!({
                "module": e.module,
                "path": e.path,
                "methods": e.methods_label(),
                "handler": format!("{}.{}", e.handler.class, e.handler.method),
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "loaded": spring.loaded, "count": found.len(), "endpoints": found })));
}
