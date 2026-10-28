//! The workspace's side of the connection: the server runs while the window does, its
//! tool calls act on the editor tabs, the selection goes to Claude as it changes, and
//! the editor follows the files Claude changes on its own.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{App, Context, Entity, Task, WeakEntity, Window};
use notify::Watcher as _;
use serde_json::{Value, json};

use super::server::{Call, Event, Reply, Server};
use super::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use super::{IdeConfig, IdeEnv, IdeStatus, review};
use crate::editor::{Answer, EditorPanel, Outcome};
use crate::lang::text::to_lsp;
use crate::workspace::Workspace;

/// How often the selection and the watched files are looked at; the selection is sent
/// once it has stayed put for one look.
pub const TICK: Duration = Duration::from_millis(100);

/// More files than this changing at once (a checkout) are not followed.
const FOLLOW_MAX: usize = 8;

struct Watch {
    root: PathBuf,
    _watcher: notify::RecommendedWatcher,
    rx: mpsc::Receiver<PathBuf>,
    ignore: ignore::gitignore::Gitignore,
}

pub struct Bridge {
    pub server: Server,
    clients: BTreeSet<u64>,
    /// The folder the lock file names.
    root: Option<PathBuf>,
    /// The selection last seen, how many looks it has stayed, and the one last sent.
    seen: Option<(PathBuf, Range<usize>)>,
    sent: Option<(PathBuf, Range<usize>)>,
    pub latest: Option<Value>,
    /// The editor of each change proposed, by its tab name.
    reviews: HashMap<String, WeakEntity<EditorPanel>>,
    watch: Option<Watch>,
    /// What files the watcher reported held then, to see what changes next.
    known: HashMap<PathBuf, String>,
    _tasks: Vec<Task<()>>,
    _quit: gpui::Subscription,
}

impl Bridge {
    pub fn clients(&self) -> usize {
        self.clients.len()
    }
}

/// Start the server for this window, if the app asks for one.
pub fn start(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(config) = cx.try_global::<IdeConfig>().cloned() else { return };
    let root = ws.model.read(cx).project_root();
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Event>();
    let folders: Vec<PathBuf> = root.iter().cloned().collect();
    let server = match Server::start(&config.lock_dir, &folders, move |e| {
        let _ = tx.unbounded_send(e);
    }) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("Claude Code integration: could not start the server: {e}");
            return;
        }
    };
    cx.set_global(IdeEnv(server.env()));
    cx.set_global(IdeStatus::default());
    let events = cx.spawn_in(window, async move |this, cx| {
        while let Some(event) = rx.next().await {
            if this.update_in(cx, |ws, window, cx| on_event(ws, event, window, cx)).is_err() {
                return;
            }
        }
    });
    let ticks = cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if this.update_in(cx, tick).is_err() {
                return;
            }
        }
    });
    // The lock file goes when the app quits.
    let quit = cx.on_app_quit(|ws, _| {
        if let Some(b) = ws.claude.as_mut() {
            b.server.stop();
        }
        async {}
    });
    ws.claude = Some(Bridge {
        server,
        clients: BTreeSet::new(),
        root,
        seen: None,
        sent: None,
        latest: None,
        reviews: HashMap::new(),
        watch: None,
        known: HashMap::new(),
        _tasks: vec![events, ticks],
        _quit: quit,
    });
}

fn set_status(ws: &Workspace, cx: &mut App) {
    let clients = ws.claude.as_ref().map_or(0, Bridge::clients);
    cx.set_global(IdeStatus { clients });
}

fn on_event(ws: &mut Workspace, event: Event, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(b) = ws.claude.as_mut() else { return };
    match event {
        Event::Connected(id) => {
            b.clients.insert(id);
            // The new client hears the selection as it is.
            b.sent = None;
            set_status(ws, cx);
        }
        Event::Disconnected(id) => {
            b.clients.remove(&id);
            set_status(ws, cx);
        }
        Event::Call(call) => call_tool(ws, call, window, cx),
    }
    cx.notify();
}

// ----- The tools ---------------------------------------------------------------------

fn call_tool(ws: &mut Workspace, call: Call, window: &mut Window, cx: &mut Context<Workspace>) {
    let Call { name, arguments: a, reply, .. } = call;
    match name.as_str() {
        "openFile" => open_file(ws, &a, reply, window, cx),
        "openDiff" => open_diff(ws, &a, reply, window, cx),
        "getCurrentSelection" => {
            let v = match ws.active_editor(window, cx) {
                Some(editor) => {
                    let mut v = selection_of(&editor, cx);
                    v["success"] = json!(true);
                    v
                }
                None => json!({ "success": false, "message": "No active editor found" }),
            };
            reply.ok(tools::json_text(&v));
        }
        "getLatestSelection" => {
            let latest = ws.claude.as_ref().and_then(|b| b.latest.clone()).or_else(|| {
                // Nothing sent yet: the selection of the editor in front, if any.
                ws.active_editor(window, cx).map(|e| selection_of(&e, cx))
            });
            let v = latest.unwrap_or_else(|| json!({ "success": false, "message": "No selection available" }));
            reply.ok(tools::json_text(&v));
        }
        "getOpenEditors" => reply.ok(tools::json_text(&open_editors(ws, window, cx))),
        "getWorkspaceFolders" => {
            let root = ws.model.read(cx).project_root();
            let folders: Vec<Value> = root
                .iter()
                .map(|r| {
                    json!({
                        "name": crate::model::Model::dir_name(r),
                        "uri": tools::file_url(r),
                        "path": r.to_string_lossy(),
                    })
                })
                .collect();
            let root_path = root.map(|r| r.to_string_lossy().into_owned());
            reply.ok(tools::json_text(&json!({ "success": true, "folders": folders, "rootPath": root_path })));
        }
        "getDiagnostics" => {
            let only = a.get("uri").and_then(Value::as_str).map(tools::path_of);
            reply.ok(tools::json_text(&diagnostics(ws, only.as_deref(), cx)));
        }
        "checkDocumentDirty" => {
            let Some(file) = a.get("filePath").and_then(Value::as_str) else {
                return reply.error(INVALID_PARAMS, "Invalid params", Some("Missing filePath parameter".into()));
            };
            let v = match editor_for(ws, &tools::path_of(file), cx) {
                Some(e) => {
                    json!({ "success": true, "filePath": file, "isDirty": e.read(cx).dirty, "isUntitled": false })
                }
                None => json!({ "success": false, "message": format!("Document not open: {file}") }),
            };
            reply.ok(tools::json_text(&v));
        }
        "saveDocument" => {
            let Some(file) = a.get("filePath").and_then(Value::as_str) else {
                return reply.error(INVALID_PARAMS, "Invalid params", Some("Missing filePath parameter".into()));
            };
            let v = match editor_for(ws, &tools::path_of(file), cx) {
                Some(e) => {
                    e.update(cx, |e, cx| e.save(cx));
                    match e.read(cx).dirty {
                        false => json!({
                            "success": true,
                            "filePath": file,
                            "saved": true,
                            "message": "Document saved successfully",
                        }),
                        true => json!({
                            "success": false,
                            "message": format!("Failed to save file: {}", file),
                            "filePath": file,
                        }),
                    }
                }
                None => json!({ "success": false, "message": format!("Document not open: {file}") }),
            };
            reply.ok(tools::json_text(&v));
        }
        "close_tab" => {
            let Some(tab) = a.get("tab_name").and_then(Value::as_str) else {
                return reply.error(
                    INVALID_PARAMS,
                    "Invalid params",
                    Some("Missing required parameter: tab_name".into()),
                );
            };
            close_tab(ws, tab, window, cx);
            reply.ok(tools::text("TAB_CLOSED"));
        }
        "closeAllDiffTabs" => {
            let mut closed = 0;
            for e in ws.editors(cx) {
                if e.read(cx).review_count() > 0 {
                    closed += e.update(cx, |e, cx| e.close_all_reviews(window, cx));
                }
            }
            if let Some(b) = ws.claude.as_mut() {
                b.reviews.clear();
            }
            reply.ok(tools::text(format!("CLOSED_{closed}_DIFF_TABS")));
        }
        other => reply.error(tools::METHOD_NOT_FOUND, &format!("Tool not found: {other}"), None),
    }
}

/// The editor tab of `path`, if one is open.
fn editor_for(ws: &Workspace, path: &Path, cx: &App) -> Option<Entity<EditorPanel>> {
    let canonical = path.canonicalize().ok();
    ws.editors(cx).into_iter().find(|e| {
        let p = &e.read(cx).path;
        p == path || canonical.as_deref().is_some_and(|c| c == p)
    })
}

/// The selection of `editor` as the protocol has it.
fn selection_of(editor: &Entity<EditorPanel>, cx: &App) -> Value {
    let e = editor.read(cx);
    let s = e.state.read(cx);
    let rope = s.text();
    let range = s.selected_range();
    let (start, end) = (to_lsp(rope, range.start), to_lsp(rope, range.end));
    json!({
        "text": rope.slice(range.clone()).to_string(),
        "filePath": e.path.to_string_lossy(),
        "fileUrl": tools::file_url(&e.path),
        "selection": {
            "start": { "line": start.line, "character": start.character },
            "end": { "line": end.line, "character": end.character },
            "isEmpty": range.is_empty(),
        },
    })
}

fn open_editors(ws: &Workspace, window: &Window, cx: &App) -> Value {
    let active = ws.active_editor(window, cx).map(|e| e.entity_id());
    let tabs: Vec<Value> = ws
        .editors(cx)
        .into_iter()
        .filter(|e| e.read(cx).class_file.is_none())
        .map(|editor| {
            let is_active = Some(editor.entity_id()) == active;
            let e = editor.read(cx);
            let mut tab = json!({
                "uri": tools::file_url(&e.path),
                "isActive": is_active,
                "isPinned": false,
                "isPreview": e.preview,
                "isDirty": e.dirty,
                "label": e.name().to_string(),
                "groupIndex": 0,
                "viewColumn": 1,
                "isGroupActive": true,
                "fileName": e.path.to_string_lossy(),
                "languageId": tools::language_id(&e.path),
                "lineCount": e.line_count(cx),
                "isUntitled": false,
            });
            if is_active {
                let sel = selection_of(&editor, cx);
                tab["selection"] = json!({
                    "start": sel["selection"]["start"],
                    "end": sel["selection"]["end"],
                    "isReversed": false,
                });
            }
            tab
        })
        .collect();
    json!({ "tabs": tabs })
}

/// The language servers' diagnostics, per file; `only` narrows them to one file.
fn diagnostics(ws: &Workspace, only: Option<&Path>, cx: &App) -> Value {
    use std::str::FromStr as _;
    let hub = ws.hub.read(cx);
    let mut files: Vec<Value> = Vec::new();
    for server in hub.servers.values() {
        for (uri, list) in &server.diagnostics {
            if list.is_empty() {
                continue;
            }
            if let Some(only) = only {
                let path = lsp_types::Uri::from_str(uri).ok().and_then(|u| snout_lang::uri::to_path(&u));
                let same =
                    path.as_deref().is_some_and(|p| p == only || only.canonicalize().ok().is_some_and(|c| c == p));
                if !same {
                    continue;
                }
            }
            files.push(json!({ "uri": uri, "diagnostics": list.iter().map(tools::diagnostic).collect::<Vec<_>>() }));
        }
    }
    Value::Array(files)
}

fn close_tab(ws: &mut Workspace, tab: &str, window: &mut Window, cx: &mut Context<Workspace>) {
    // A change Claude proposed: its review ends (rejected if still waiting); the file's
    // tab stays.
    let reviewed = ws.claude.as_mut().and_then(|b| b.reviews.remove(tab)).and_then(|e| e.upgrade());
    if let Some(editor) = reviewed {
        editor.update(cx, |e, cx| e.close_review(tab, window, cx));
        return;
    }
    // Otherwise only a tab named exactly so, without unsaved changes.
    let named = ws.editors(cx).into_iter().find(|e| {
        let e = e.read(cx);
        (e.name() == tab || e.path == Path::new(tab)) && !e.dirty && e.review_count() == 0
    });
    if let Some(editor) = named {
        ws.dock.update(cx, |d, cx| d.remove_panel(editor, window, cx));
    }
}

fn open_file(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(file) = a.get("filePath").and_then(Value::as_str) else {
        return reply.error(INVALID_PARAMS, "Invalid params", Some("Missing filePath parameter".into()));
    };
    let path = tools::path_of(file);
    if !path.is_file() {
        return reply.error(TOOL_ERROR, "File operation error", Some(format!("File not found: {}", path.display())));
    }
    let flag = |k: &str| a.get(k).and_then(Value::as_bool);
    let preview = flag("preview").unwrap_or(false);
    let front = flag("makeFrontmost").unwrap_or(true);
    let editor = ws.reveal_file(path.clone(), preview, front, window, cx);
    let mut message = format!("Opened file: {}", path.display());
    let text = editor.read(cx).text(cx);
    let line_start = |line: usize| text.split_inclusive('\n').take(line).map(str::len).sum::<usize>();
    let line_end = |at: usize| text[at..].find('\n').map_or(text.len(), |i| at + i);
    let mut select: Option<Range<usize>> = None;
    let line = |k: &str| a.get(k).and_then(Value::as_u64).map(|n| n as usize);
    if line("startLine").is_some() || line("endLine").is_some() {
        let start = line("startLine").unwrap_or(1).max(1);
        let end = line("endLine").unwrap_or(start).max(start);
        let from = line_start(start - 1).min(text.len());
        select = Some(from..line_end(line_start(end - 1).min(text.len())));
        message = format!("Opened file and selected lines {start} to {end}");
    }
    let start_text = a.get("startText").and_then(Value::as_str).filter(|s| !s.is_empty());
    let end_text = a.get("endText").and_then(Value::as_str).filter(|s| !s.is_empty());
    if let Some(st) = start_text {
        match text.find(st) {
            Some(at) => {
                let mut end = at + st.len();
                message = format!("Opened file and selected text \"{st}\"");
                if let Some(et) = end_text {
                    match text[at..].find(et) {
                        Some(i) => {
                            end = at + i + et.len();
                            if flag("selectToEndOfLine").unwrap_or(false) {
                                end = line_end(end);
                            }
                            message = format!("Opened file and selected text from \"{st}\" to \"{et}\"");
                        }
                        None => {
                            message = format!("Opened file and positioned at \"{st}\" (end text \"{et}\" not found)");
                        }
                    }
                }
                select = Some(at..end);
            }
            None => message = format!("Opened file, but text \"{st}\" not found"),
        }
    }
    if let Some(range) = select {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| s.set_selected_range(range, cx));
    }
    if front {
        reply.ok(tools::text(message));
    } else {
        let e = editor.read(cx);
        reply.ok(tools::json_text(&json!({
            "success": true,
            "filePath": path.to_string_lossy(),
            "languageId": tools::language_id(&path),
            "lineCount": e.line_count(cx),
        })));
    }
}

fn open_diff(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let mut args = Vec::new();
    for key in ["old_file_path", "new_file_path", "new_file_contents", "tab_name"] {
        match a.get(key).and_then(Value::as_str) {
            Some(v) => args.push(v.to_owned()),
            None => {
                return reply.error(
                    INVALID_PARAMS,
                    "Invalid params",
                    Some(format!("Missing required parameter: {key}")),
                );
            }
        }
    }
    let [_, new_path, contents, tab]: [String; 4] = args.try_into().expect("four arguments");
    let path = tools::path_of(&new_path);
    let editor = ws.reveal_file(path, false, true, window, cx);
    let name = tab.clone();
    let answer: Answer = Box::new(move |outcome| match outcome {
        Outcome::Accepted(text) => reply.ok(tools::texts(["FILE_SAVED".to_owned(), text])),
        Outcome::Rejected => reply.ok(tools::texts(["DIFF_REJECTED".to_owned(), name])),
    });
    editor.update(cx, |e, cx| e.propose(tab.clone(), contents, answer, window, cx));
    if let Some(b) = ws.claude.as_mut() {
        b.reviews.retain(|_, e| e.upgrade().is_some_and(|e| e.read(cx).review_count() > 0));
        b.reviews.insert(tab, editor.downgrade());
    }
}

// ----- The selection and @-mentions -------------------------------------------------

fn tick(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    track_selection(ws, window, cx);
    follow_watched(ws, window, cx);
    // The lock file names the project open now.
    let root = ws.model.read(cx).project_root();
    if let Some(b) = ws.claude.as_mut()
        && b.root != root
    {
        b.root = root.clone();
        b.server.set_folders(&root.iter().cloned().collect::<Vec<_>>());
    }
}

fn track_selection(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let editor = ws.active_editor(window, cx).filter(|e| e.read(cx).class_file.is_none());
    let key = editor.as_ref().map(|e| {
        let e = e.read(cx);
        (e.path.clone(), e.state.read(cx).selected_range())
    });
    let Some(b) = ws.claude.as_mut() else { return };
    if key != b.seen {
        // Still moving: wait for it to settle.
        b.seen = key;
        return;
    }
    if key == b.sent {
        return;
    }
    b.sent = key;
    let Some(editor) = editor else { return };
    let selection = selection_of(&editor, cx);
    b.latest = Some(selection.clone());
    if !b.clients.is_empty() {
        b.server.notify("selection_changed", selection);
    }
}

/// ⌥⌘K: tell Claude Code about the selection (or the file) in the editor in front, and
/// bring its console forward. Without a connection, the mention is typed into the
/// console instead.
pub fn send_selection(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(editor) = ws.active_editor(window, cx) else { return };
    let (path, lines) = {
        let e = editor.read(cx);
        let s = e.state.read(cx);
        let rope = s.text();
        let range = s.selected_range();
        let lines = (!range.is_empty()).then(|| {
            let (a, b) = (to_lsp(rope, range.start), to_lsp(rope, range.end));
            // A selection that ends at the start of a line does not take that line.
            let end = if b.character == 0 && b.line > a.line { b.line - 1 } else { b.line };
            (a.line, end)
        });
        (e.path.clone(), lines)
    };
    let connected = ws.claude.as_ref().is_some_and(|b| b.clients() > 0);
    if connected && let Some(b) = ws.claude.as_ref() {
        b.server.notify(
            "at_mentioned",
            json!({
                "filePath": path.to_string_lossy(),
                "lineStart": lines.map(|l| l.0),
                "lineEnd": lines.map(|l| l.1),
            }),
        );
    }
    show_console(ws, window, cx);
    if !connected {
        let root = ws.model.read(cx).project_root();
        let rel = root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(&path).to_string_lossy().into_owned();
        let mention = match lines {
            Some((a, b)) if a == b => format!("@{rel}#L{} ", a + 1),
            Some((a, b)) => format!("@{rel}#L{}-{} ", a + 1, b + 1),
            None => format!("@{rel} "),
        };
        if let (Some(console), Some(root)) = (crate::console::console_of(ws, cx), root) {
            console.update(cx, |c, _| c.type_text(&root, &mention));
        }
    }
}

/// Show the Claude Code console and give it the keyboard.
fn show_console(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    if !ws.model.read(cx).settings.console_open {
        ws.model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx));
    }
    if let Some(console) = crate::console::console_of(ws, cx) {
        let id = crate::tabs::panel_id(&console);
        ws.dock.update(cx, |d, cx| d.select_panel(id, window, cx));
        let focus = gpui::Focusable::focus_handle(console.read(cx), cx);
        window.focus(&focus, cx);
    }
}

// ----- Following Claude's edits -----------------------------------------------------

/// The lines of `after` that differ from `before`: the new ones, or the line where
/// others went.
fn changed_lines(before: &str, after: &str) -> Vec<Range<usize>> {
    let count = after.split_inclusive('\n').count().max(1);
    review::hunks(before, after)
        .into_iter()
        .map(|h| {
            if h.new_lines.is_empty() {
                let at = h.new_lines.start.min(count - 1);
                at..at + 1
            } else {
                h.new_lines
            }
        })
        .collect()
}

/// Whether to follow a change on disk now: Claude is connected, and no Git job of the
/// app's own is changing files.
fn following(ws: &Workspace, cx: &App) -> bool {
    ws.claude.as_ref().is_some_and(|b| b.clients() > 0) && ws.model.read(cx).job.is_none()
}

/// An open tab took its file's new text from disk: follow it there, or mark the tab.
pub fn followed_reload(
    ws: &mut Workspace,
    editor: &Entity<EditorPanel>,
    before: &str,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if !following(ws, cx) {
        return;
    }
    let after = editor.read(cx).text(cx);
    let lines = changed_lines(before, &after);
    if lines.is_empty() {
        return;
    }
    if ws.model.read(cx).settings.follow_claude {
        let (path, preview) = (editor.read(cx).path.clone(), editor.read(cx).preview);
        ws.reveal_file(path, preview, true, window, cx);
        editor.update(cx, |e, cx| e.flash(lines, window, cx));
    } else {
        editor.update(cx, |e, cx| {
            e.touched = true;
            cx.notify();
        });
    }
}

fn watch(root: &Path) -> Option<Watch> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && !matches!(event.kind, notify::EventKind::Access(_))
        {
            for path in event.paths {
                let _ = tx.send(path);
            }
        }
    })
    .ok()?;
    watcher.watch(root, notify::RecursiveMode::Recursive).ok()?;
    let (ignore, _) = ignore::gitignore::Gitignore::new(root.join(".gitignore"));
    Some(Watch { root: root.to_owned(), _watcher: watcher, rx, ignore })
}

/// Files changed on disk that no tab shows: follow Claude into them.
fn follow_watched(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let root = ws.model.read(cx).project_root();
    let Some(b) = ws.claude.as_mut() else { return };
    if b.watch.as_ref().map(|w| &w.root) != root.as_ref() {
        b.watch = root.as_deref().and_then(watch);
        b.known.clear();
    }
    let Some(w) = b.watch.as_ref() else { return };
    let mut changed: Vec<PathBuf> = Vec::new();
    while let Ok(path) = w.rx.try_recv() {
        if !changed.contains(&path) {
            changed.push(path);
        }
    }
    let root = w.root.clone();
    let wanted = |p: &Path| {
        p.starts_with(&root)
            && !p.strip_prefix(&root).is_ok_and(|r| r.components().any(|c| c.as_os_str() == ".git"))
            && !w.ignore.matched_path_or_any_parents(p, false).is_ignore()
            && p.is_file()
    };
    changed.retain(|p| wanted(p));
    if changed.is_empty() {
        return;
    }
    let follow = following(ws, cx) && ws.model.read(cx).settings.follow_claude && changed.len() <= FOLLOW_MAX;
    let open: Vec<PathBuf> = ws.editors(cx).into_iter().map(|e| e.read(cx).path.clone()).collect();
    for path in changed {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Some(b) = ws.claude.as_mut() else { return };
        if b.known.len() > 512 {
            b.known.clear();
        }
        let before = b.known.insert(path.clone(), text.clone());
        // Open tabs follow through their own reload.
        if !follow || open.contains(&path) {
            continue;
        }
        let before = before.or_else(|| {
            let rel = path.strip_prefix(&root).ok()?.to_string_lossy().replace('\\', "/");
            snout_core::gutter::head_text(&root, &rel)
        });
        let before = before.unwrap_or_default();
        if before == text {
            continue;
        }
        let lines = changed_lines(&before, &text);
        let editor = ws.reveal_file(path, true, true, window, cx);
        editor.update(cx, |e, cx| e.flash(lines, window, cx));
    }
}
