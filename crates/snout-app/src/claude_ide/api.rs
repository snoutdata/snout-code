//! The editor's API for agents: Agent Skill → UTCP → API.
//!
//! Claude Code's IDE connection ([`super::server`]) is Claude Code's own plumbing: it opens
//! diffs for review and hears the selection, but it offers the model no tool to open a
//! file. What the model asks of the editor goes another way, without an MCP server's tool
//! schemas in every request: a skill (in a plugin the console's `claude` loads for its
//! session only, `--plugin-dir`) tells it how; a UTCP manual (`GET /utcp`) describes the
//! calls; the calls are plain HTTP on the hooks' endpoint (`POST /api/<tool>`), checked
//! against the same token. Each call is answered by the same code as the IDE connection's
//! tool of that name, so the two never differ.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::tools;

/// The tools the API offers: the IDE connection's, less those that wait on the user or
/// only make sense to Claude Code itself (diff tabs).
pub const TOOLS: [&str; 8] = [
    "openFile",
    "getCurrentSelection",
    "getLatestSelection",
    "getOpenEditors",
    "getWorkspaceFolders",
    "getDiagnostics",
    "checkDocumentDirty",
    "saveDocument",
];

/// The variable naming the file that holds the token's header line (`x-snout-hook: …`),
/// for `curl -H @file`: the token stays off command lines and out of transcripts.
pub const HEADER_ENV: &str = "SNOUT_API_HEADER";

/// The UTCP manual version this API writes.
const UTCP_VERSION: &str = "1.0.1";

/// The folder of the plugin, inside the session's settings folder.
const PLUGIN: &str = "snout-plugin";

/// The skill, as the plugin carries it; `{{API}}` stands for the script's path.
pub const SKILL: &str = include_str!("snout_editor_skill.md");

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

/// The tool `tools/list` describes by `name`.
fn described(name: &str) -> Option<Value> {
    tools::list()["tools"].as_array()?.iter().find(|t| t["name"] == name).cloned()
}

/// The UTCP manual: each tool with its inputs and the HTTP call that runs it. The port
/// and the token are variables, from the console's environment.
pub fn manual() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .filter_map(|name| {
            let t = described(name)?;
            Some(json!({
                "name": name,
                "description": t["description"],
                "inputs": t["inputSchema"],
                "outputs": { "type": "object", "description": "The tool's answer: JSON, or text when it is not JSON." },
                "tags": ["editor", "snout-code"],
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
            }))
        })
        .collect();
    json!({
        "utcp_version": UTCP_VERSION,
        "manual_version": env!("CARGO_PKG_VERSION"),
        "info": {
            "title": "SnoutCode editor",
            "description": format!(
                "The editor of the SnoutCode window this agent runs in. Arguments go as query parameters or as a JSON \
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
    let schema = described(tool).map(|t| t["inputSchema"]["properties"].clone()).unwrap_or(Value::Null);
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

/// Write the plugin that carries the skill and its script into `dir` on `host`. Returns
/// its folder, for `--plugin-dir`.
pub fn write_plugin(host: &dyn snout_host::Host, dir: &Path) -> std::io::Result<PathBuf> {
    let root = plugin_dir(dir);
    let skill = root.join("skills").join("snout-editor");
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
        "description": "The SnoutCode editor, for the agent in its console.",
    });
    let meta = serde_json::to_string_pretty(&meta).map_err(std::io::Error::other)?;
    host.write_mode(&root.join(".claude-plugin").join("plugin.json"), meta.as_bytes(), Some(0o600))?;
    let text = SKILL.replace("{{API}}", &api.to_string_lossy());
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
        assert_eq!(names, TOOLS, "every tool described, none waiting on the user");
        let open = &m["tools"][0];
        assert_eq!(open["inputs"]["required"], json!(["filePath"]));
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
    fn the_plugin_carries_the_skill() {
        let dir = std::env::temp_dir().join(format!("snout-api-plugin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = write_plugin(&*snout_host::local(), &dir).unwrap();
        let meta: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
        assert_eq!(meta["name"], "snout-code");
        let skill = std::fs::read_to_string(root.join("skills/snout-editor/SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: snout-editor\n"));
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
