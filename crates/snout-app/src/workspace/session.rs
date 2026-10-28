//! The saved dock layout made safe to load: what an earlier build wrote, a panel this build
//! no longer has, a file deleted since, a tab of another project. Pure functions over the
//! layout's JSON (GPUI Kit's `DockAreaState`), so they can be tested without a window.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The names GPUI Kit gives its containers in a saved layout.
const TABS: &str = "TabPanel";
const STACK: &str = "StackPanel";

/// What may stay in a layout.
pub struct Keep<'a> {
    /// The panels this build makes, by the name each saves under.
    pub panels: &'a HashSet<String>,
    /// The editor tabs' panel name.
    pub editor: &'a str,
    /// The open project: editor tabs of files outside it go. None: no editor tab stays.
    pub project: Option<&'a Path>,
}

impl Keep<'_> {
    fn editor_stays(&self, info: &Value) -> bool {
        let Some(project) = self.project else { return false };
        let Some(path) = info.get("path").and_then(Value::as_str).map(Path::new) else { return false };
        // A library class shown from a build of the project.
        if info.get("classFile").is_some() {
            return info.get("root").and_then(Value::as_str).is_some_and(|r| {
                let r = Path::new(r);
                r.starts_with(project) || project.starts_with(r)
            });
        }
        path.is_file() && path.starts_with(project)
    }
}

/// The layout `state` with only what `keep` lets through; each dock's tree and the center
/// stay well formed (an emptied group goes, the tab that was in front stays in front).
pub fn sanitize(mut state: Value, keep: &Keep) -> Value {
    let mut seen = HashSet::new();
    if let Some(center) = state.get_mut("center")
        && prune(center, keep, &mut seen)
    {
        *center = empty_tabs();
    }
    for dock in ["left_dock", "right_dock", "bottom_dock"] {
        if let Some(panel) = state.get_mut(dock).and_then(|d| d.get_mut("panel"))
            && prune(panel, keep, &mut seen)
        {
            *panel = empty_tabs();
        }
    }
    state
}

/// An empty tab group.
pub fn empty_tabs() -> Value {
    json!({ "panel_name": TABS, "children": [], "info": { "tabs": { "active_index": 0 } } })
}

/// Prune `node` in place; true when nothing is left of it.
fn prune(node: &mut Value, keep: &Keep, seen: &mut HashSet<String>) -> bool {
    let name = node.get("panel_name").and_then(Value::as_str).unwrap_or_default().to_owned();
    let info = node.get("info").cloned().unwrap_or(Value::Null);
    if let Some(stack) = info.get("stack") {
        let children = take_children(node);
        let sizes: Vec<Value> = stack.get("sizes").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut kept = Vec::new();
        let mut kept_sizes = Vec::new();
        for (i, mut child) in children.into_iter().enumerate() {
            if !prune(&mut child, keep, seen) {
                kept.push(child);
                kept_sizes.push(sizes.get(i).cloned().unwrap_or(json!(0.0)));
            }
        }
        let empty = kept.is_empty();
        node["children"] = Value::Array(kept);
        node["info"]["stack"]["sizes"] = Value::Array(kept_sizes);
        return empty;
    }
    if let Some(tabs) = info.get("tabs") {
        let active = tabs.get("active_index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let children = take_children(node);
        let mut kept = Vec::new();
        let mut front = None;
        for (i, mut child) in children.into_iter().enumerate() {
            if !prune(&mut child, keep, seen) {
                if i <= active {
                    front = Some(kept.len());
                }
                kept.push(child);
            }
        }
        let empty = kept.is_empty();
        node["children"] = Value::Array(kept);
        node["info"] = json!({ "tabs": { "active_index": front.unwrap_or(0) } });
        return empty;
    }
    // A leaf: an empty container written by an older dock, or a panel.
    if name == TABS || name == STACK {
        return true;
    }
    if name == keep.editor {
        return !keep.editor_stays(info.get("panel").unwrap_or(&Value::Null));
    }
    // A panel this build lacks, or one placed twice, goes (it would show as a broken tab).
    !keep.panels.contains(&name) || !seen.insert(name)
}

fn take_children(node: &mut Value) -> Vec<Value> {
    match node.get_mut("children").map(Value::take) {
        Some(Value::Array(children)) => children,
        _ => Vec::new(),
    }
}

/// The files of the editor tabs anywhere in a saved layout, of whatever shape, in order.
pub fn editor_files(state: &Value, editor: &str) -> Vec<PathBuf> {
    fn walk(v: &Value, editor: &str, out: &mut Vec<PathBuf>) {
        match v {
            Value::Object(map) => {
                if map.get("panel_name").and_then(Value::as_str) == Some(editor)
                    && let Some(path) = map.get("info").and_then(|i| i.get("panel")).and_then(|p| p.get("path"))
                    && let Some(path) = path.as_str().map(PathBuf::from)
                    && !out.contains(&path)
                {
                    out.push(path);
                }
                map.values().for_each(|v| walk(v, editor, out));
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, editor, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(state, editor, &mut out);
    out
}

/// The layout `full` with its center replaced by `center`.
pub fn with_center(mut full: Value, center: Value) -> Value {
    full["center"] = center;
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(name: &str, info: Value) -> Value {
        json!({ "panel_name": name, "children": [], "info": { "panel": info } })
    }

    fn tabs(children: Vec<Value>, active: usize) -> Value {
        json!({ "panel_name": "TabPanel", "children": children, "info": { "tabs": { "active_index": active } } })
    }

    #[test]
    fn unknown_panels_missing_files_and_other_projects_tabs_go() {
        let dir = std::env::temp_dir().join(format!("snout-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("A.java");
        std::fs::write(&a, "class A {}").unwrap();
        let gone = dir.join("Gone.java");
        let center = json!({ "panel_name": "StackPanel", "children": [
            tabs(vec![
                leaf("Editor", json!({ "path": gone })),
                leaf("Welcome", Value::Null),
                leaf("Editor", json!({ "path": a, "line": 3 })),
                leaf("Editor", json!({ "path": "/elsewhere/B.java" })),
            ], 2),
            tabs(vec![leaf("PanelFromTheFuture", Value::Null)], 0),
        ], "info": { "stack": { "sizes": [400.0, 300.0], "axis": 0 } } });
        let state = json!({ "version": 2, "center": center, "bottom_dock": {
            "panel": tabs(vec![leaf("Output", Value::Null), leaf("Output", Value::Null)], 1),
            "placement": "bottom", "size": 200.0, "open": true } });
        let panels: HashSet<String> = ["Welcome", "Output"].into_iter().map(String::from).collect();
        let keep = Keep { panels: &panels, editor: "Editor", project: Some(&dir) };
        let out = sanitize(state.clone(), &keep);
        let group = &out["center"]["children"];
        assert_eq!(group.as_array().unwrap().len(), 1, "the emptied group goes");
        assert_eq!(out["center"]["info"]["stack"]["sizes"], json!([400.0]));
        let kept: Vec<&str> =
            group[0]["children"].as_array().unwrap().iter().map(|c| c["panel_name"].as_str().unwrap()).collect();
        assert_eq!(kept, ["Welcome", "Editor"]);
        assert_eq!(group[0]["info"]["tabs"]["active_index"], 1, "the tab in front stays in front");
        assert_eq!(out["bottom_dock"]["panel"]["children"].as_array().unwrap().len(), 1, "no panel twice");

        // Without a project no editor tab stays; the files can still be read out.
        let keep = Keep { panels: &panels, editor: "Editor", project: None };
        let out = sanitize(state.clone(), &keep);
        assert!(!out.to_string().contains("A.java"));
        assert_eq!(editor_files(&state, "Editor").len(), 3);
        let _ = std::fs::remove_dir_all(dir);
    }
}
