//! The tools this IDE offers Claude Code, as `tools/list` describes them, and the shapes
//! of their results. Names, arguments and results follow the published integrations, so
//! Claude Code reads them as it reads theirs.

use serde_json::{Value, json};

/// Every tool `tools/call` answers. `close_tab` is called by Claude Code but not listed,
/// as the other integrations do.
pub const NAMES: [&str; 11] = [
    "openFile",
    "openDiff",
    "getCurrentSelection",
    "getLatestSelection",
    "getOpenEditors",
    "getWorkspaceFolders",
    "getDiagnostics",
    "checkDocumentDirty",
    "saveDocument",
    "closeAllDiffTabs",
    "close_tab",
];

/// JSON-RPC error codes.
pub const INVALID_PARAMS: i64 = -32602;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const TOOL_ERROR: i64 = -32000;

fn schema(properties: Value, required: &[&str]) -> Value {
    let mut s = json!({
        "type": "object",
        "additionalProperties": false,
        "$schema": "http://json-schema.org/draft-07/schema#",
    });
    if properties.as_object().is_some_and(|p| !p.is_empty()) {
        s["properties"] = properties;
    }
    if !required.is_empty() {
        s["required"] = json!(required);
    }
    s
}

fn tool(name: &str, description: &str, input: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input })
}

/// What `tools/list` answers.
pub fn list() -> Value {
    let none = || schema(json!({}), &[]);
    let file_path = |what: &str| json!({ "filePath": { "type": "string", "description": what } });
    json!({ "tools": [
        tool(
            "openFile",
            "Open a file in the editor and optionally select a range of text",
            schema(
                json!({
                    "filePath": { "type": "string", "description": "Path to the file to open" },
                    "preview": { "type": "boolean", "description": "Whether to open the file in preview mode", "default": false },
                    "startLine": { "type": "integer", "description": "Optional: Line number to start selection" },
                    "endLine": { "type": "integer", "description": "Optional: Line number to end selection" },
                    "startText": { "type": "string", "description": "Text pattern to find the start of the selection range. Selects from the beginning of this match." },
                    "endText": { "type": "string", "description": "Text pattern to find the end of the selection range. Selects up to the end of this match. If not provided, only the startText match will be selected." },
                    "selectToEndOfLine": { "type": "boolean", "description": "If true, selection will extend to the end of the line containing the endText match.", "default": false },
                    "makeFrontmost": { "type": "boolean", "description": "Whether to make the file the active editor tab. If false, the file will be opened in the background without changing focus.", "default": true },
                }),
                &["filePath"],
            ),
        ),
        tool(
            "openDiff",
            "Open a diff view comparing old file content with new file content",
            schema(
                json!({
                    "old_file_path": { "type": "string", "description": "Path to the old file to compare" },
                    "new_file_path": { "type": "string", "description": "Path to the new file to compare" },
                    "new_file_contents": { "type": "string", "description": "Contents for the new file version" },
                    "tab_name": { "type": "string", "description": "Name for the diff tab/view" },
                }),
                &["old_file_path", "new_file_path", "new_file_contents", "tab_name"],
            ),
        ),
        tool("getCurrentSelection", "Get the current text selection in the editor", none()),
        tool("getLatestSelection", "Get the most recent text selection (even if not in the active editor)", none()),
        tool("getOpenEditors", "Get list of currently open files", none()),
        tool("getWorkspaceFolders", "Get all workspace folders currently open in the IDE", none()),
        tool(
            "getDiagnostics",
            "Get language diagnostics (errors, warnings) from the editor",
            schema(
                json!({ "uri": { "type": "string", "description": "Optional file URI to get diagnostics for. If not provided, gets diagnostics for all open files." } }),
                &[],
            ),
        ),
        tool("checkDocumentDirty", "Check if a document has unsaved changes (is dirty)", schema(file_path("Path to the file to check"), &["filePath"])),
        tool("saveDocument", "Save a document with unsaved changes", schema(file_path("Path to the file to save"), &["filePath"])),
        tool("closeAllDiffTabs", "Close all diff tabs in the editor", none()),
    ] })
}

/// A tool result of one text item.
pub fn text(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }] })
}

/// A tool result of several text items.
pub fn texts<S: Into<String>>(items: impl IntoIterator<Item = S>) -> Value {
    let content: Vec<Value> = items.into_iter().map(|t| json!({ "type": "text", "text": t.into() })).collect();
    json!({ "content": content })
}

/// A tool result whose text is `value` as JSON.
pub fn json_text(value: &Value) -> Value {
    text(value.to_string())
}

/// The protocol name of a diagnostic's severity.
pub fn severity_name(s: Option<lsp_types::DiagnosticSeverity>) -> &'static str {
    use lsp_types::DiagnosticSeverity as S;
    match s {
        Some(S::WARNING) => "Warning",
        Some(S::INFORMATION) => "Info",
        Some(S::HINT) => "Hint",
        _ => "Error",
    }
}

/// One diagnostic as `getDiagnostics` lists it.
pub fn diagnostic(d: &lsp_types::Diagnostic) -> Value {
    let pos = |p: lsp_types::Position| json!({ "line": p.line, "character": p.character });
    let mut v = json!({
        "message": d.message,
        "severity": severity_name(d.severity),
        "range": { "start": pos(d.range.start), "end": pos(d.range.end) },
    });
    if let Some(source) = &d.source {
        v["source"] = json!(source);
    }
    if let Some(code) = &d.code {
        v["code"] = json!(match code {
            lsp_types::NumberOrString::Number(n) => n.to_string(),
            lsp_types::NumberOrString::String(s) => s.clone(),
        });
    }
    v
}

/// A `file://` URI for `path`.
pub fn file_url(path: &std::path::Path) -> String {
    snout_lang::uri::from_path(path).as_str().to_owned()
}

/// The path a `file://` URI (or a bare path, `~/` in `home`) names.
pub fn path_of(uri_or_path: &str, home: &std::path::Path) -> std::path::PathBuf {
    use std::str::FromStr as _;
    if uri_or_path.starts_with("file://")
        && let Ok(uri) = lsp_types::Uri::from_str(uri_or_path)
        && let Some(path) = snout_lang::uri::to_path(&uri)
    {
        return path;
    }
    expand_tilde(uri_or_path, home)
}

/// `~/x` as the home folder's `x` (the home on the host Claude Code runs on).
pub fn expand_tilde(path: &str, home: &std::path::Path) -> std::path::PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) if !home.as_os_str().is_empty() => home.join(rest),
        _ => path.into(),
    }
}

/// The language id of a file, as the protocol names them.
pub fn language_id(path: &std::path::Path) -> &'static str {
    match crate::editor::language_for(path) {
        "text" => "plaintext",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_tool_has_a_schema_and_close_tab_is_unlisted() {
        let list = list();
        let names: Vec<&str> = list["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), NAMES.len() - 1);
        assert!(!names.contains(&"close_tab"));
        for t in list["tools"].as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object");
            assert!(!t["description"].as_str().unwrap().is_empty());
        }
        let diff = &list["tools"][1];
        assert_eq!(diff["inputSchema"]["required"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn diagnostics_read_as_the_protocol_has_them() {
        let d = lsp_types::Diagnostic {
            range: lsp_types::Range::new(lsp_types::Position::new(3, 4), lsp_types::Position::new(3, 9)),
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            code: Some(lsp_types::NumberOrString::Number(16)),
            source: Some("Java".into()),
            message: "Unused".into(),
            ..Default::default()
        };
        assert_eq!(
            diagnostic(&d),
            json!({
                "message": "Unused",
                "severity": "Warning",
                "range": { "start": { "line": 3, "character": 4 }, "end": { "line": 3, "character": 9 } },
                "source": "Java",
                "code": "16",
            })
        );
    }

    #[test]
    fn uris_and_paths_go_both_ways() {
        let p = std::path::Path::new("/tmp/a b/C.java");
        let url = file_url(p);
        assert!(url.starts_with("file:///tmp/a"));
        let home = std::path::Path::new("/home/me");
        assert_eq!(path_of(&url, home), p);
        assert_eq!(path_of("/x/y.rs", home), std::path::Path::new("/x/y.rs"));
        assert_eq!(path_of("~/y.rs", home), std::path::Path::new("/home/me/y.rs"));
    }
}
