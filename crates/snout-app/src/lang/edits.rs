//! A server's `WorkspaceEdit` applied: to the editor tab that has a file open (so it can be
//! undone there), else to the file on disk; and files created, renamed or deleted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use lsp_types::{DocumentChangeOperation, DocumentChanges, OneOf, ResourceOp, TextEdit, WorkspaceEdit};

/// What the edit does, file by file, in order.
#[derive(Debug, PartialEq)]
pub enum Step {
    Edit(PathBuf, Vec<TextEdit>),
    Create(PathBuf),
    Rename(PathBuf, PathBuf),
    Delete(PathBuf),
}

/// The steps of `edit`, for files only (other URIs are skipped).
pub fn steps(edit: WorkspaceEdit) -> Vec<Step> {
    let path = |u: &lsp_types::Uri| snout_lang::uri::to_path(u);
    let mut out = Vec::new();
    match edit.document_changes {
        Some(DocumentChanges::Edits(edits)) => {
            for e in edits {
                if let Some(p) = path(&e.text_document.uri) {
                    out.push(Step::Edit(p, e.edits.into_iter().map(text_edit).collect()));
                }
            }
        }
        Some(DocumentChanges::Operations(ops)) => {
            for op in ops {
                match op {
                    DocumentChangeOperation::Edit(e) => {
                        if let Some(p) = path(&e.text_document.uri) {
                            out.push(Step::Edit(p, e.edits.into_iter().map(text_edit).collect()));
                        }
                    }
                    DocumentChangeOperation::Op(ResourceOp::Create(c)) => out.extend(path(&c.uri).map(Step::Create)),
                    DocumentChangeOperation::Op(ResourceOp::Rename(r)) => {
                        if let (Some(a), Some(b)) = (path(&r.old_uri), path(&r.new_uri)) {
                            out.push(Step::Rename(a, b));
                        }
                    }
                    DocumentChangeOperation::Op(ResourceOp::Delete(d)) => out.extend(path(&d.uri).map(Step::Delete)),
                }
            }
        }
        None => {
            let sorted: BTreeMap<String, (lsp_types::Uri, Vec<TextEdit>)> =
                edit.changes.unwrap_or_default().into_iter().map(|(u, e)| (u.as_str().to_owned(), (u, e))).collect();
            for (_, (uri, edits)) in sorted {
                if let Some(p) = path(&uri) {
                    out.push(Step::Edit(p, edits));
                }
            }
        }
    }
    out
}

fn text_edit(e: OneOf<TextEdit, lsp_types::AnnotatedTextEdit>) -> TextEdit {
    match e {
        OneOf::Left(e) => e,
        OneOf::Right(a) => a.text_edit,
    }
}

/// Apply `edits` to a file on `host` no editor has open.
pub fn edit_file(host: &dyn snout_host::Host, path: &std::path::Path, edits: &[TextEdit]) -> std::io::Result<()> {
    let text = host.read_to_string(path)?;
    host.write(path, super::text::apply_to_string(&text, edits).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_both_shapes_of_workspace_edit() {
        let edit: WorkspaceEdit = serde_json::from_value(json!({
            "documentChanges": [
                { "textDocument": { "uri": "file:///w/A.java", "version": 3 },
                  "edits": [{ "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } }, "newText": "B" }] },
                { "kind": "rename", "oldUri": "file:///w/A.java", "newUri": "file:///w/B.java" }
            ]
        }))
        .unwrap();
        let s = steps(edit);
        assert!(matches!(&s[0], Step::Edit(p, e) if p == &PathBuf::from("/w/A.java") && e[0].new_text == "B"));
        assert_eq!(s[1], Step::Rename("/w/A.java".into(), "/w/B.java".into()));
        let edit: WorkspaceEdit = serde_json::from_value(json!({ "changes": { "file:///w/C.java": [] } })).unwrap();
        assert_eq!(steps(edit), [Step::Edit("/w/C.java".into(), Vec::new())]);
    }
}
