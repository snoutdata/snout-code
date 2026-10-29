//! SnoutCode's language server client, for any language: JSON-RPC over a child process's
//! stdio, the capabilities SnoutCode's editor offers, progress tracking, URIs and the
//! incremental text sync; and a Debug Adapter Protocol client. What is particular to one
//! server (how to start it, its settings) lives with the app.

pub mod client;
pub mod dap;
pub mod progress;
pub mod rpc;
pub mod text;
pub mod uri;

pub use client::{Client, Messages, ServerMessage, block_on};
pub use lsp_types;
pub use rpc::ResponseError;

use serde_json::{Value, json};

/// What SnoutCode's editor supports, for `initialize`. Completion items come as plain text
/// (no snippets); code actions come with their edits (no resolve round trip); positions
/// are UTF-16, the protocol's default.
pub fn client_capabilities() -> Value {
    json!({
        "workspace": {
            "applyEdit": true,
            "workspaceEdit": {
                "documentChanges": true,
                "resourceOperations": ["create", "rename", "delete"]
            },
            "configuration": true,
            "workspaceFolders": true,
            "didChangeWatchedFiles": { "dynamicRegistration": false },
            "symbol": { "symbolKind": { "valueSet": (1..=26).collect::<Vec<u32>>() } },
            "executeCommand": {}
        },
        "textDocument": {
            "synchronization": { "didSave": true, "willSave": false, "dynamicRegistration": false },
            "publishDiagnostics": { "relatedInformation": true, "tagSupport": { "valueSet": [1, 2] } },
            "completion": {
                "completionItem": {
                    "snippetSupport": false,
                    "documentationFormat": ["markdown", "plaintext"],
                    "resolveSupport": { "properties": ["documentation", "detail", "additionalTextEdits"] }
                },
                "contextSupport": true
            },
            "hover": { "contentFormat": ["markdown", "plaintext"] },
            "signatureHelp": {
                "signatureInformation": {
                    "documentationFormat": ["markdown", "plaintext"],
                    "parameterInformation": { "labelOffsetSupport": true },
                    "activeParameterSupport": true
                }
            },
            "definition": { "linkSupport": true },
            "references": {},
            "documentSymbol": { "hierarchicalDocumentSymbolSupport": true },
            "typeHierarchy": {},
            "callHierarchy": {},
            "formatting": {},
            "rangeFormatting": {},
            "rename": { "prepareSupport": true },
            "codeAction": {
                "codeActionLiteralSupport": {
                    "codeActionKind": { "valueSet": [
                        "", "quickfix", "refactor", "refactor.extract", "refactor.inline",
                        "refactor.rewrite", "source", "source.organizeImports"
                    ] }
                },
                "isPreferredSupport": true
            }
        },
        "window": { "workDoneProgress": true, "showDocument": { "support": true } }
    })
}
