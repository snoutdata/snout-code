//! The editor's API for agents: Agent Skill → UTCP → API.
//!
//! Claude Code's IDE connection ([`super::server`]) is Claude Code's own plumbing: it opens
//! diffs for review and hears the selection, but it offers the model no tool to open a
//! file. What the model asks of the editor goes another way, without an MCP server's tool
//! schemas in every request: a skill (in a plugin the console's `claude` loads for its
//! session only, `--plugin-dir`) tells it how; a UTCP manual (`GET /utcp`) describes the
//! calls; the calls are plain HTTP on the hooks' endpoint (`POST /api/<tool>`), checked
//! against the same token. [`super::capabilities`] says which calls there are; those the IDE
//! connection also has are answered by its code, so the two never differ.
//!
//! The one exception is Codex: its sandbox cuts its commands off from that port by default, but not
//! its MCP client. So it gets one MCP tool, for its session only, that makes the same calls
//! ([`mcp_message`]): one definition costs about what a skill's line does, and the manual stays out of
//! the context until it is asked for.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::capabilities;
use super::tools;

/// The variable naming the file that holds the token's header line (`x-snout-hook: …`),
/// for `curl -H @file`: the token stays off command lines and out of transcripts.
pub const HEADER_ENV: &str = "SNOUT_API_HEADER";

/// The UTCP manual version this API writes.
const UTCP_VERSION: &str = "1.0.1";

/// The folder of the plugin, inside the session's settings folder.
const PLUGIN: &str = "snout-plugin";

/// The skill, as the plugin carries it; `{{API}}` stands for the script's path.
pub const SKILL: &str = include_str!("snoutcode_skill.md");

/// The script the skill runs: `snout-api <tool> [JSON arguments]`, or `snout-api manual`.
/// One command with no variables in it, so Claude Code's permission check can match it
/// (the session's settings allow it); it reads the port and the token's header file from
/// the console's environment.
pub const SCRIPT: &str = include_str!("snout-api.sh");

/// The plugin's folder in the session's settings folder `dir`, for `--plugin-dir`.
pub fn plugin_dir(dir: &Path) -> PathBuf {
    dir.join(PLUGIN)
}

/// The script's path in the session's settings folder `dir`.
pub fn script(dir: &Path) -> PathBuf {
    plugin_dir(dir).join("bin").join("snout-api")
}

/// What a draft's two calls use in the session's folder when an agent's sandbox cuts it off from
/// the port: the change the window lays out, and the message the script leaves.
pub const COMMIT_CHANGE: &str = "commit-change";
pub const COMMIT_MESSAGE: &str = "commit-message";

/// The UTCP manual: each tool with its inputs and the HTTP call that runs it. The port
/// and the token are variables, from the console's environment.
pub fn manual() -> Value {
    let tools: Vec<Value> = capabilities::all()
        .map(|c| {
            let name = c.name;
            json!({
                "name": name,
                "description": c.description,
                "inputs": c.schema(),
                "outputs": { "type": "object", "description": "The tool's answer: JSON, or text when it is not JSON." },
                "tags": c.tags(),
                "tool_call_template": {
                    "call_template_type": "http",
                    "name": "snout-code",
                    "url": format!("http://127.0.0.1:${{{}}}/api/{name}", super::hooks::PORT_ENV),
                    "http_method": "POST",
                    "content_type": "application/json",
                    "auth": {
                        "auth_type": "api_key",
                        "api_key": "${SNOUT_API_TOKEN}",
                        "var_name": super::hooks::TOKEN_HEADER,
                        "location": "header",
                    },
                },
            })
        })
        .collect();
    json!({
        "utcp_version": UTCP_VERSION,
        "manual_version": env!("CARGO_PKG_VERSION"),
        "info": {
            "title": "SnoutCode",
            "description": format!(
                "The SnoutCode window this agent runs in. Each tool is tagged with what it does: read looks, show changes \
                 only what is on screen, change writes or starts something. Arguments go as query parameters or as a JSON \
                 body. The token is the value of the header line in the file ${HEADER_ENV}; with curl: -H @\"${HEADER_ENV}\"."
            ),
        },
        "tools": tools,
    })
}

/// A query string's pairs, decoded.
fn query_pairs(query: &str) -> Vec<(String, String)> {
    fn decode(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => out.push(b' '),
                b'%' => match bytes
                    .get(i + 1..i + 3)
                    .and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok())
                {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                },
                b => out.push(b),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (decode(k), decode(v)),
            None => (decode(p), String::new()),
        })
        .collect()
}

/// The arguments of a call to `tool`: the query's parameters, typed as the tool's schema
/// says, then the JSON body's fields over them.
pub fn arguments(tool: &str, query: &str, body: &[u8]) -> Result<Value, String> {
    let schema = capabilities::find(tool).map(|c| c.schema()["properties"].clone()).unwrap_or(Value::Null);
    let mut args = Map::new();
    for (k, v) in query_pairs(query) {
        let typed = match schema[&k]["type"].as_str() {
            Some("integer") => v.parse::<i64>().map(Value::from).map_err(|_| format!("{k} must be an integer"))?,
            Some("boolean") => match v.as_str() {
                "true" | "1" | "" => Value::Bool(true),
                "false" | "0" => Value::Bool(false),
                _ => return Err(format!("{k} must be true or false")),
            },
            _ => Value::String(v),
        };
        args.insert(k, typed);
    }
    if !body.iter().all(u8::is_ascii_whitespace) {
        match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(fields)) => args.extend(fields),
            _ => return Err("the body must be a JSON object".into()),
        }
    }
    Ok(Value::Object(args))
}

/// An HTTP answer: its status line, content type and body.
#[derive(Debug, PartialEq)]
pub struct Answer {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: String,
}

impl Answer {
    pub fn error(status: &'static str, message: impl Into<String>) -> Self {
        Self { status, content_type: "application/json", body: json!({ "error": message.into() }).to_string() }
    }
}

/// The HTTP answer to a tool's JSON-RPC message: its text items (one item as it is, JSON
/// when it is JSON; several as an array), or its error.
pub fn answer(message: &str) -> Answer {
    let Ok(msg) = serde_json::from_str::<Value>(message) else {
        return Answer::error("500 Internal Server Error", "no answer");
    };
    if let Some(e) = msg.get("error") {
        let status = match e["code"].as_i64() {
            Some(tools::INVALID_PARAMS) => "400 Bad Request",
            Some(tools::METHOD_NOT_FOUND) => "404 Not Found",
            _ => "422 Unprocessable Entity",
        };
        let text = [e["message"].as_str(), e["data"].as_str()].into_iter().flatten().collect::<Vec<_>>().join(": ");
        return Answer::error(status, text);
    }
    let items: Vec<&str> =
        msg["result"]["content"].as_array().into_iter().flatten().filter_map(|c| c["text"].as_str()).collect();
    let as_json = |t: &str| serde_json::from_str::<Value>(t).ok();
    match items.as_slice() {
        [one] => match as_json(one) {
            Some(v) => Answer { status: "200 OK", content_type: "application/json", body: v.to_string() },
            None => Answer { status: "200 OK", content_type: "text/plain; charset=utf-8", body: (*one).to_owned() },
        },
        many => {
            let v: Vec<Value> = many.iter().map(|t| as_json(t).unwrap_or_else(|| json!(t))).collect();
            Answer { status: "200 OK", content_type: "application/json", body: Value::from(v).to_string() }
        }
    }
}

/// The name an agent's MCP client knows the one tool by.
pub const MCP_TOOL: &str = "snoutcode";

/// How a shell writes `path`: a Windows path runs under Git for Windows's bash, where a backslash
/// would be an escape.
pub fn shell_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if text.as_bytes().get(1) == Some(&b':') { text.replace('\\', "/") } else { text.into_owned() }
}

/// A description's first sentence.
fn first_sentence(text: &str) -> &str {
    text.find(". ").map_or(text, |at| &text[..=at])
}

/// What an agent reads to find its way: every tool, with no arguments (the short form, the whole of
/// a domain with [`domain_manual`]). It is in the context only once asked for, but it grows with each
/// tool: the full UTCP manual repeats each tool's call, for clients that make the calls themselves.
pub fn index() -> Value {
    let tools: Vec<Value> = capabilities::all()
        .map(|c| {
            json!({
                "name": c.name,
                "domain": c.domain.tag(),
                "effect": c.effect.tag(),
                "description": first_sentence(c.description),
            })
        })
        .collect();
    json!({
        "effects": "read looks; show changes only what is on screen; change writes or starts something, and the user may be asked",
        "next": "Ask for a domain's manual for the arguments of its tools.",
        "tools": tools,
    })
}

/// The tools of `domain` with their arguments. An unknown one says which there are.
pub fn domain_manual(domain: &str) -> Result<Value, String> {
    let tools: Vec<Value> = capabilities::all()
        .filter(|c| c.domain.tag().eq_ignore_ascii_case(domain.trim()))
        .map(|c| {
            json!({
                "name": c.name,
                "effect": c.effect.tag(),
                "description": c.description,
                "inputs": c.schema(),
            })
        })
        .collect();
    if tools.is_empty() {
        let mut known: Vec<&str> = capabilities::all().map(|c| c.domain.tag()).collect();
        known.dedup();
        return Err(format!("No domain called {domain}. The domains: {}", known.join(", ")));
    }
    Ok(json!({ "domain": domain.trim().to_lowercase(), "tools": tools }))
}

/// The one MCP tool, as `tools/list` offers it.
fn mcp_tool() -> Value {
    json!({
        "name": MCP_TOOL,
        "description": "Work the SnoutCode window this session runs in: see the project, what is open and what is on \
                        screen, open files at a line or with text selected, and bring its windows forward. Use it \
                        instead of `code`, `open`, `idea` or any other editor. Call it with tool \"manual\" to list what \
                        it can do, and with tool \"manual\" and arguments {\"domain\": name} for the arguments of a \
                        domain's tools.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tool": { "type": "string", "description": "A tool from the manual, or \"manual\"" },
                "arguments": { "type": "object", "description": "The tool's arguments" },
            },
            "required": ["tool"],
        },
    })
}

/// The answer to a JSON-RPC message on the MCP endpoint; none for a notification. `call` makes a call
/// of the API.
pub fn mcp_message(msg: &Value, call: impl FnOnce(&str, Value) -> Answer) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let result = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match msg["method"].as_str().unwrap_or_default() {
        "initialize" => result(json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": MCP_TOOL, "version": env!("CARGO_PKG_VERSION") },
        })),
        "ping" => result(json!({})),
        "tools/list" => result(json!({ "tools": [mcp_tool()] })),
        "tools/call" if params["name"] == MCP_TOOL => {
            let tool = params["arguments"]["tool"].as_str().unwrap_or_default().to_owned();
            let arguments = match &params["arguments"]["arguments"] {
                Value::Null => json!({}),
                a => a.clone(),
            };
            let answered = if tool == "manual" {
                let domain = arguments["domain"].as_str();
                match domain.map_or_else(|| Ok(index()), domain_manual) {
                    Ok(manual) => Ok(manual.to_string()),
                    Err(e) => Err(e),
                }
            } else if capabilities::find(&tool).is_none() {
                Err(format!("No tool called {tool:?}. Call tool \"manual\" for the list."))
            } else {
                let a = call(&tool, arguments);
                if a.status.starts_with('2') {
                    Ok(a.body)
                } else {
                    let error = serde_json::from_str::<Value>(&a.body).ok();
                    Err(error.and_then(|e| e["error"].as_str().map(str::to_owned)).unwrap_or(a.body))
                }
            };
            let is_error = answered.is_err();
            let text = answered.unwrap_or_else(|e| e);
            result(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        "tools/call" => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": tools::METHOD_NOT_FOUND, "message": "No such tool" } })
        }
        // A client's other asks (resources, prompts): there are none.
        "resources/list" => result(json!({ "resources": [] })),
        "prompts/list" => result(json!({ "prompts": [] })),
        other => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": tools::METHOD_NOT_FOUND, "message": format!("No method {other}") } })
        }
    })
}

/// Write the plugin that carries the skill and its script into `dir` on `host`. Returns
/// its folder, for `--plugin-dir`.
pub fn write_plugin(host: &dyn snout_host::Host, dir: &Path) -> std::io::Result<PathBuf> {
    let root = plugin_dir(dir);
    let skill = root.join("skills").join("snoutcode");
    let api = script(dir);
    host.create_dir_all_mode(&root.join(".claude-plugin"), Some(0o700))?;
    host.create_dir_all_mode(&skill, Some(0o700))?;
    if let Some(bin) = api.parent() {
        host.create_dir_all_mode(bin, Some(0o700))?;
    }
    host.write_mode(&api, SCRIPT.as_bytes(), Some(0o700))?;
    let meta = json!({
        "name": "snout-code",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "The SnoutCode window, for the agent in its console.",
    });
    let meta = serde_json::to_string_pretty(&meta).map_err(std::io::Error::other)?;
    host.write_mode(&root.join(".claude-plugin").join("plugin.json"), meta.as_bytes(), Some(0o600))?;
    let text = SKILL.replace("{{API}}", &shell_path(&api));
    host.write_mode(&skill.join("SKILL.md"), text.as_bytes(), Some(0o600))?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manual_offers_each_tool_as_an_http_call_with_the_token_as_a_variable() {
        let m = manual();
        assert_eq!(m["utcp_version"], UTCP_VERSION);
        let names: Vec<&str> = m["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, capabilities::all().map(|c| c.name).collect::<Vec<_>>(), "every tool described");
        let open = m["tools"].as_array().unwrap().iter().find(|t| t["name"] == "openFile").unwrap();
        assert_eq!(open["inputs"]["required"], json!(["filePath"]));
        assert_eq!(open["tags"], json!(["editor", "show"]));
        let call = &open["tool_call_template"];
        assert_eq!(call["url"], "http://127.0.0.1:${SNOUT_HOOK_PORT}/api/openFile");
        assert_eq!(call["auth"]["var_name"], "x-snout-hook");
        assert!(!m.to_string().contains("token\":\""), "no token written into the manual");
    }

    #[test]
    fn arguments_come_from_the_query_typed_by_the_schema_and_from_the_body() {
        let a = arguments("openFile", "filePath=%2Fp%2FA%20B.java&startLine=12&makeFrontmost=false", b"").unwrap();
        assert_eq!(a, json!({ "filePath": "/p/A B.java", "startLine": 12, "makeFrontmost": false }));
        let b = arguments("openFile", "startLine=3", br#"{"filePath":"/p/A.java","startLine":7}"#).unwrap();
        assert_eq!(b, json!({ "filePath": "/p/A.java", "startLine": 7 }), "the body wins");
        assert!(arguments("openFile", "startLine=x", b"").is_err());
        assert!(arguments("openFile", "", b"[1]").is_err());
        assert_eq!(arguments("getOpenEditors", "", b"  \n").unwrap(), json!({}));
        assert_eq!(query_pairs("a=%zz&b=50%")[0].1, "%zz");
        assert_eq!(query_pairs("b=50%")[0].1, "50%");
    }

    #[test]
    fn answers_are_the_tools_json_or_text_and_errors_have_a_status() {
        let ok = answer(&json!({ "id": 1, "result": tools::text("Opened file: /p/A.java") }).to_string());
        assert_eq!((ok.status, ok.content_type), ("200 OK", "text/plain; charset=utf-8"));
        assert_eq!(ok.body, "Opened file: /p/A.java");
        let j = answer(&json!({ "id": 1, "result": tools::json_text(&json!({ "tabs": [] })) }).to_string());
        assert_eq!((j.content_type, j.body.as_str()), ("application/json", r#"{"tabs":[]}"#));
        let missing = json!({ "id": 1, "error": { "code": tools::TOOL_ERROR, "message": "File operation error",
            "data": "File not found: /p/nope" } });
        let e = answer(&missing.to_string());
        assert_eq!(e.status, "422 Unprocessable Entity");
        assert!(e.body.contains("File not found: /p/nope"));
        let bad = json!({ "id": 1, "error": { "code": tools::INVALID_PARAMS, "message": "Invalid params" } });
        assert_eq!(answer(&bad.to_string()).status, "400 Bad Request");
    }

    #[test]
    fn an_mcp_client_gets_one_tool_that_makes_the_calls() {
        let no_call = |_: &str, _: Value| -> Answer { panic!("no call of the API") };
        let init = mcp_message(
            &json!({ "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }),
            no_call,
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["id"], 1);
        assert_eq!(mcp_message(&json!({ "method": "notifications/initialized" }), no_call), None);
        let list = mcp_message(&json!({ "id": 2, "method": "tools/list" }), no_call).unwrap();
        let listed = list["result"]["tools"].as_array().unwrap();
        assert_eq!(listed.len(), 1, "one definition, whatever the number of tools");
        assert_eq!(listed[0]["name"], MCP_TOOL);
        assert!(listed[0].to_string().len() < 900, "about what a skill's line costs: {}", listed[0].to_string().len());

        let called = |tool: &str, arguments: Value, a: Answer| {
            let msg = json!({ "id": 3, "method": "tools/call", "params": { "name": MCP_TOOL, "arguments": { "tool": tool, "arguments": arguments } } });
            mcp_message(&msg, |name, args| {
                assert_eq!((name, &args), (tool, &json!({ "filePath": "/p/A.java" })));
                a
            })
            .unwrap()["result"]
                .clone()
        };
        let ok = Answer { status: "200 OK", content_type: "text/plain", body: "Opened".into() };
        let r = called("openFile", json!({ "filePath": "/p/A.java" }), ok);
        assert_eq!((r["content"][0]["text"].as_str(), r["isError"].as_bool()), (Some("Opened"), Some(false)));
        let refused = Answer::error("422 Unprocessable Entity", "File not found");
        let r = called("openFile", json!({ "filePath": "/p/A.java" }), refused);
        assert_eq!((r["content"][0]["text"].as_str(), r["isError"].as_bool()), (Some("File not found"), Some(true)));

        // The manual, a domain of it, and names that are not there, none of them a call of the API.
        let manual = |arguments: Value| {
            let msg = json!({ "id": 4, "method": "tools/call", "params": { "name": MCP_TOOL, "arguments": { "tool": "manual", "arguments": arguments } } });
            mcp_message(&msg, no_call).unwrap()["result"].clone()
        };
        assert!(manual(json!({}))["content"][0]["text"].as_str().unwrap().contains("getAppState"));
        let domain = manual(json!({ "domain": "Editor" }));
        assert!(domain["content"][0]["text"].as_str().unwrap().contains("startLine"), "with the arguments");
        let none = manual(json!({ "domain": "nope" }));
        assert_eq!(none["isError"], true);
        assert!(none["content"][0]["text"].as_str().unwrap().contains("editor"), "says which there are");
        let msg = json!({ "id": 5, "method": "tools/call", "params": { "name": MCP_TOOL, "arguments": { "tool": "deleteEverything" } } });
        assert_eq!(mcp_message(&msg, no_call).unwrap()["result"]["isError"], true);
        let msg = json!({ "id": 6, "method": "tools/call", "params": { "name": "other" } });
        assert!(mcp_message(&msg, no_call).unwrap()["error"].is_object());
    }

    #[test]
    fn the_short_manual_lists_every_tool_without_its_arguments() {
        let i = index();
        assert_eq!(i["tools"].as_array().unwrap().len(), capabilities::all().count());
        assert!(!i.to_string().contains("startLine"));
        assert!(i.to_string().len() < manual().to_string().len() / 3, "well under the whole manual");
    }

    #[test]
    fn a_windows_path_is_written_for_its_bash() {
        assert_eq!(shell_path(Path::new(r"C:\Users\me\snout-api")), "C:/Users/me/snout-api");
        assert_eq!(shell_path(Path::new("/home/me/snout-api")), "/home/me/snout-api");
    }

    #[test]
    fn the_plugin_carries_the_skill() {
        let dir = std::env::temp_dir().join(format!("snout-api-plugin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = write_plugin(&*snout_host::local(), &dir).unwrap();
        let meta: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
        assert_eq!(meta["name"], "snout-code");
        let skill = std::fs::read_to_string(root.join("skills/snoutcode/SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: snoutcode\n"));
        let api = script(&dir).to_string_lossy().into_owned();
        assert!(skill.contains(&format!("{api} openFile")) && !skill.contains("{{API}}"), "the script by its path");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(std::fs::metadata(&api).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
