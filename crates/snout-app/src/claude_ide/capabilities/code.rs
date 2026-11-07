//! The language engines' answers: what no shell gives an agent. Java's engine and TypeScript's know
//! the project as a compiler does: symbols, definitions, references, types, callers, renames.

use std::path::{Path, PathBuf};

use gpui::{Context, Task, Window};
use lsp_types::{Position, Uri};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::workspace::Workspace;

/// How many results a call returns at most.
const MAX_RESULTS: usize = 200;

pub const ALL: &[Capability] = &[
    Capability {
        name: "findSymbol",
        domain: Domain::Code,
        effect: Effect::Read,
        description: "Find classes, methods and fields by name across the project, as the language engine knows them. \
                      Faster and surer than searching the text.",
        input: Input::Own(symbol_input),
        run: Run::Own(find_symbol),
        consent: None,
    },
    Capability {
        name: "getDefinition",
        domain: Domain::Code,
        effect: Effect::Read,
        description: "Where the symbol at a place in a file is defined, into libraries too.",
        input: Input::Own(place_input),
        run: Run::Own(|ws, a, reply, _, cx| locations(ws, a, reply, "textDocument/definition", json!({}), cx)),
        consent: None,
    },
    Capability {
        name: "findReferences",
        domain: Domain::Code,
        effect: Effect::Read,
        description: "Every place the symbol at a place in a file is used, across the project.",
        input: Input::Own(place_input),
        run: Run::Own(|ws, a, reply, _, cx| {
            let context = json!({ "context": { "includeDeclaration": false } });
            locations(ws, a, reply, "textDocument/references", context, cx)
        }),
        consent: None,
    },
    Capability {
        name: "getHover",
        domain: Domain::Code,
        effect: Effect::Read,
        description: "The type and documentation of the symbol at a place in a file.",
        input: Input::Own(place_input),
        run: Run::Own(hover),
        consent: None,
    },
    Capability {
        name: "getHierarchy",
        domain: Domain::Code,
        effect: Effect::Read,
        description: "One level of the hierarchy of the symbol at a place in a file: a type's supertypes or subtypes, \
                      or a method's callers or callees. Call it again on a result to go further.",
        input: Input::Own(hierarchy_input),
        run: Run::Own(hierarchy),
        consent: None,
    },
    Capability {
        name: "getProjectProblems",
        domain: Domain::Code,
        effect: Effect::Read,
        description: "The errors and warnings the language engines report across the project, errors first.",
        input: Input::None,
        run: Run::Own(problems),
        consent: None,
    },
    Capability {
        name: "renameSymbol",
        domain: Domain::Code,
        effect: Effect::Change,
        description: "Rename the symbol at a place in a file, everywhere the language engine knows it is used. The \
                      changed files open with the changes unsaved, for the user to review.",
        input: Input::Own(rename_input),
        run: Run::Own(rename),
        consent: Some(|a| {
            format!("rename the symbol at {}:{} to {}", text_of(a, "filePath"), a["line"], text_of(a, "newName"))
        }),
    },
];

fn place_properties() -> Value {
    json!({
        "filePath": { "type": "string", "description": "The file, absolute" },
        "line": { "type": "integer", "description": "The line, from 1" },
        "text": { "type": "string", "description": "The symbol's name as it stands on that line" },
        "column": { "type": "integer", "description": "The column, from 1, in place of text" },
    })
}

fn place_input() -> Value {
    tools::schema(place_properties(), &["filePath", "line"])
}

fn symbol_input() -> Value {
    tools::schema(json!({ "query": { "type": "string", "description": "All or part of the name" } }), &["query"])
}

fn hierarchy_input() -> Value {
    let mut p = place_properties();
    p["direction"] = json!({
        "type": "string",
        "enum": ["supertypes", "subtypes", "callers", "callees"],
        "description": "Which way to look",
    });
    tools::schema(p, &["filePath", "line", "direction"])
}

fn rename_input() -> Value {
    let mut p = place_properties();
    p["newName"] = json!({ "type": "string", "description": "The new name" });
    tools::schema(p, &["filePath", "line", "newName"])
}

/// The place an agent names, in the engine's terms: `text` on a line (from 1), else a column (from 1).
pub fn position_in(file: &str, line: u64, text: Option<&str>, column: Option<u64>) -> Result<Position, String> {
    let index = line.checked_sub(1).ok_or("Lines count from 1")? as usize;
    let Some(row) = file.lines().nth(index) else {
        return Err(format!("The file has {} lines, not {line}", file.lines().count()));
    };
    let start = match (text, column) {
        (Some(t), _) => {
            let at = row.find(t).ok_or_else(|| format!("{t:?} is not on line {line}: {}", row.trim()))?;
            row[..at].encode_utf16().count() as u32
        }
        (None, Some(c)) => c.saturating_sub(1) as u32,
        (None, None) => return Err("Say where on the line: text or column".to_owned()),
    };
    Ok(Position { line: index as u32, character: start })
}

/// A place in a file, as the engine knows it.
struct Spot {
    uri: Uri,
    root: PathBuf,
    at: Position,
}

/// Find the place the arguments name: the file's text comes from its tab, else from the host, off the
/// UI thread. Answers with what is wrong, for the agent to put right.
fn spot(ws: &Workspace, a: &Value, cx: &mut Context<Workspace>) -> Task<Result<Spot, String>> {
    let Some(file) = need(a, "filePath") else { return Task::ready(Err("Missing filePath".to_owned())) };
    let Some(line) = a["line"].as_u64() else { return Task::ready(Err("Missing line".to_owned())) };
    let host = crate::host::host(cx);
    let path = tools::path_of(file, &host.info().home);
    let Ok(uri) = tools::file_url(&path).parse::<Uri>() else {
        return Task::ready(Err(format!("Not a path: {file}")));
    };
    let hub = ws.hub.read(cx);
    let root = hub.root_of(&uri).or_else(|| hub.engine_root_for(&uri));
    let Some(root) = root else {
        return Task::ready(Err(format!(
            "No language engine knows {file}. Is it in the open project? getAppState says what is open."
        )));
    };
    let open = ws
        .editors(cx)
        .into_iter()
        .find(|e| e.read(cx).path == path)
        .map(|e| e.read(cx).state.read(cx).value().to_string());
    let (text, column) = (a["text"].as_str().map(str::to_owned), a["column"].as_u64());
    cx.background_executor().spawn(async move {
        let body = match open {
            Some(body) => body,
            None => read(&*host, &path)?,
        };
        Ok(Spot { uri, root, at: position_in(&body, line, text.as_deref(), column)? })
    })
}

fn read(host: &dyn snout_host::Host, path: &Path) -> Result<String, String> {
    let bytes = host.read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// An engine's request about a file that failed: what to do about it. TypeScript's engine knows only
/// the files open in a tab.
fn engine_failed(e: impl std::fmt::Display) -> String {
    format!("The language engine did not answer: {e}. It may still be starting; or open the file first with openFile.")
}

/// A location as the agent reads it: a path, and a line and column from 1.
fn place_of(uri: &str, range: &Value, home: &Path) -> Value {
    json!({
        "filePath": tools::path_of(uri, home).to_string_lossy(),
        "line": range["start"]["line"].as_u64().unwrap_or(0) + 1,
        "column": range["start"]["character"].as_u64().unwrap_or(0) + 1,
    })
}

/// The places in an engine's answer: one location or many, links too.
pub fn places_in(v: &Value, home: &Path) -> Vec<Value> {
    let one = |l: &Value| {
        let uri = l["uri"].as_str().or_else(|| l["targetUri"].as_str())?;
        let range = if l["range"].is_null() { &l["targetSelectionRange"] } else { &l["range"] };
        Some(place_of(uri, range, home))
    };
    match v {
        Value::Array(list) => list.iter().filter_map(one).take(MAX_RESULTS).collect(),
        Value::Null => Vec::new(),
        single => one(single).into_iter().collect(),
    }
}

fn locations(
    ws: &mut Workspace,
    a: &Value,
    reply: Reply,
    method: &'static str,
    extra: Value,
    cx: &mut Context<Workspace>,
) {
    let found = spot(ws, a, cx);
    let hub = ws.hub.clone();
    let home = crate::host::host(cx).info().home.clone();
    cx.spawn(async move |_, cx| {
        let spot = match found.await {
            Ok(s) => s,
            Err(e) => return reply.error(INVALID_PARAMS, &e, None),
        };
        let mut params = json!({ "textDocument": { "uri": spot.uri.as_str() }, "position": spot.at });
        if let (Some(p), Some(x)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(x.clone());
        }
        let call = cx.update(|cx| hub.read(cx).request_doc(&spot.uri, &spot.root, method, params));
        match call.await {
            Ok(v) => {
                let places = places_in(&v, &home);
                reply.ok(tools::json_text(&json!({ "count": places.len(), "places": places })));
            }
            Err(e) => reply.error(TOOL_ERROR, &engine_failed(e), None),
        }
    })
    .detach();
}

fn hover(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let found = spot(ws, a, cx);
    let hub = ws.hub.clone();
    cx.spawn(async move |_, cx| {
        let spot = match found.await {
            Ok(s) => s,
            Err(e) => return reply.error(INVALID_PARAMS, &e, None),
        };
        let params = json!({ "textDocument": { "uri": spot.uri.as_str() }, "position": spot.at });
        let call = cx.update(|cx| hub.read(cx).request_doc(&spot.uri, &spot.root, "textDocument/hover", params));
        match call.await {
            Ok(Value::Null) => reply.ok(tools::text("The engine has nothing to say about that place")),
            Ok(v) => match serde_json::from_value::<lsp_types::Hover>(v) {
                Ok(h) => reply.ok(tools::text(crate::lang::providers::hover_markdown(&h))),
                Err(e) => reply.error(TOOL_ERROR, &engine_failed(e), None),
            },
            Err(e) => reply.error(TOOL_ERROR, &engine_failed(e), None),
        }
    })
    .detach();
}

fn hierarchy(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let (prepare, next, field) = match a["direction"].as_str() {
        Some("supertypes") => ("textDocument/prepareTypeHierarchy", "typeHierarchy/supertypes", None),
        Some("subtypes") => ("textDocument/prepareTypeHierarchy", "typeHierarchy/subtypes", None),
        Some("callers") => ("textDocument/prepareCallHierarchy", "callHierarchy/incomingCalls", Some("from")),
        Some("callees") => ("textDocument/prepareCallHierarchy", "callHierarchy/outgoingCalls", Some("to")),
        _ => return missing(reply, "direction: supertypes, subtypes, callers or callees"),
    };
    let found = spot(ws, a, cx);
    let hub = ws.hub.clone();
    let home = crate::host::host(cx).info().home.clone();
    cx.spawn(async move |_, cx| {
        let spot = match found.await {
            Ok(s) => s,
            Err(e) => return reply.error(INVALID_PARAMS, &e, None),
        };
        let params = json!({ "textDocument": { "uri": spot.uri.as_str() }, "position": spot.at });
        let call = cx.update(|cx| hub.read(cx).request_doc(&spot.uri, &spot.root, prepare, params));
        let items = match call.await {
            Ok(v) => v,
            Err(e) => return reply.error(TOOL_ERROR, &engine_failed(e), None),
        };
        let Some(item) = items.as_array().and_then(|i| i.first()).cloned() else {
            // The engine knows some files only once they are open.
            let message = "The engine finds no type or method at that place. If the file is not open, open it with \
                           openFile, with makeFrontmost false, and ask again.";
            return reply.error(TOOL_ERROR, message, None);
        };
        let call = cx.update(|cx| hub.read(cx).request_doc(&spot.uri, &spot.root, next, json!({ "item": item })));
        match call.await {
            Ok(v) => {
                let items: Vec<Value> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(MAX_RESULTS)
                    .map(|entry| {
                        let i = field.map_or(entry, |f| &entry[f]);
                        let uri = i["uri"].as_str().unwrap_or_default();
                        let mut place = place_of(uri, &i["selectionRange"], &home);
                        place["name"] = i["name"].clone();
                        place["detail"] = i["detail"].clone();
                        place
                    })
                    .collect();
                reply.ok(tools::json_text(&json!({ "count": items.len(), "items": items })));
            }
            Err(e) => reply.error(TOOL_ERROR, &engine_failed(e), None),
        }
    })
    .detach();
}

fn find_symbol(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(query) = need(a, "query") else { return missing(reply, "query") };
    let Some(project) = ws.model.read(cx).project_root() else {
        return reply.error(TOOL_ERROR, "No project is open", None);
    };
    let hub = ws.hub.read(cx);
    let Some(root) = hub.engine_root(&project) else {
        return reply.error(TOOL_ERROR, "The language engine is not running for this project yet", None);
    };
    let call = hub.request(&root, "workspace/symbol", json!({ "query": query }));
    let home = crate::host::host(cx).info().home.clone();
    cx.background_executor()
        .spawn(async move {
            match call.await {
                Ok(v) => {
                    let symbols: Vec<Value> = v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .take(MAX_RESULTS)
                        .map(|s| {
                            let l = &s["location"];
                            let mut place = place_of(l["uri"].as_str().unwrap_or_default(), &l["range"], &home);
                            place["name"] = s["name"].clone();
                            place["container"] = s["containerName"].clone();
                            place
                        })
                        .collect();
                    reply.ok(tools::json_text(&json!({ "count": symbols.len(), "symbols": symbols })));
                }
                Err(e) => reply.error(TOOL_ERROR, &engine_failed(e), None),
            }
        })
        .detach();
}

fn problems(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    use lsp_types::DiagnosticSeverity as S;
    let home = crate::host::host(cx).info().home.clone();
    let mut all = ws.hub.read(cx).all_problems();
    all.sort_by_key(|(_, d)| match d.severity {
        Some(S::ERROR) => 0,
        Some(S::WARNING) => 1,
        _ => 2,
    });
    let total = all.len();
    let list: Vec<Value> = all
        .iter()
        .take(MAX_RESULTS)
        .map(|(uri, d)| {
            json!({
                "filePath": tools::path_of(uri, &home).to_string_lossy(),
                "line": d.range.start.line + 1,
                "column": d.range.start.character + 1,
                "severity": match d.severity {
                    Some(S::ERROR) => "error",
                    Some(S::WARNING) => "warning",
                    _ => "info",
                },
                "message": d.message,
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "total": total, "problems": list })));
}

fn rename(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(name) = need(a, "newName").map(str::to_owned) else { return missing(reply, "newName") };
    let found = spot(ws, a, cx);
    let hub = ws.hub.clone();
    cx.spawn_in(window, async move |ws, cx| {
        let spot = match found.await {
            Ok(s) => s,
            Err(e) => return reply.error(INVALID_PARAMS, &e, None),
        };
        let params = json!({ "textDocument": { "uri": spot.uri.as_str() }, "position": spot.at, "newName": name });
        let Ok(call) =
            cx.update(|_, cx| hub.read(cx).request_doc(&spot.uri, &spot.root, "textDocument/rename", params))
        else {
            return;
        };
        let edit = match call.await.map(serde_json::from_value::<Option<lsp_types::WorkspaceEdit>>) {
            Ok(Ok(Some(edit))) => edit,
            Ok(Ok(None)) => return reply.error(TOOL_ERROR, "The engine cannot rename what is at that place", None),
            Ok(Err(e)) => return reply.error(TOOL_ERROR, &engine_failed(e), None),
            Err(e) => return reply.error(TOOL_ERROR, &engine_failed(e), None),
        };
        let applying = ws.update_in(cx, |ws, window, cx| ws.apply_edit(edit, window, cx));
        match applying {
            Ok(task) => {
                task.await;
                reply.ok(tools::text(format!("Renamed to {name}. The changed files are open, unsaved.")));
            }
            Err(e) => reply.error(TOOL_ERROR, &e.to_string(), None),
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_found_by_its_text_or_its_column() {
        let file = "class A {\n    int \u{1F600} count;\n}\n";
        assert_eq!(position_in(file, 1, Some("A"), None).unwrap(), Position { line: 0, character: 6 });
        // The engine counts UTF-16 units: the emoji is two.
        assert_eq!(position_in(file, 2, Some("count"), None).unwrap(), Position { line: 1, character: 11 });
        assert_eq!(position_in(file, 2, None, Some(5)).unwrap(), Position { line: 1, character: 4 });
        assert!(position_in(file, 9, Some("A"), None).unwrap_err().contains("3 lines"));
        assert!(position_in(file, 1, Some("nope"), None).unwrap_err().contains("not on line 1"));
        assert!(position_in(file, 1, None, None).is_err());
        assert!(position_in(file, 0, Some("A"), None).is_err());
    }

    #[test]
    fn an_engines_places_read_as_paths_and_lines_from_one() {
        let home = Path::new("/home/me");
        let range = json!({ "start": { "line": 4, "character": 2 }, "end": { "line": 4, "character": 5 } });
        let one = json!({ "uri": "file:///p/A.java", "range": range });
        assert_eq!(places_in(&one, home), [json!({ "filePath": "/p/A.java", "line": 5, "column": 3 })]);
        let link = json!([{ "targetUri": "file:///p/B.java", "targetSelectionRange": range, "targetRange": range }]);
        assert_eq!(places_in(&link, home), [json!({ "filePath": "/p/B.java", "line": 5, "column": 3 })]);
        assert!(places_in(&Value::Null, home).is_empty());
    }
}
