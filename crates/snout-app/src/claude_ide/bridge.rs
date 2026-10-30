//! The workspace's side of the connection: the server runs while the window does, its
//! tool calls act on the editor tabs, the selection goes to Claude as it changes, and
//! the editor follows the files Claude changes on its own: its hooks say which, before
//! and after each edit, and the file watcher stands in for a `claude` started elsewhere.
//! What Claude changed waits in the Claude Changes window until it is accepted or
//! reverted.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Styled as _, Task, WeakEntity, Window, div, px,
};
use serde_json::{Value, json};

use super::changes::{Restore, Source};
use super::hooks::{self, HookEvent, HookKind, HookServer, Snapshot};
use super::panel::{Activity, ClaudeChanges, PendingDiff, Updated};
use super::server::{Call, Event, Reply, Server};
use super::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use super::{CodexArgs, IdeArgs, IdeConfig, IdeEnv, IdeStatus, review};
use crate::agents::{self, ConsoleTurn};
use crate::editor::{Answer, ClaudeRequest, EditorPanel, Outcome};
use crate::lang::text::to_lsp;
use crate::theme::palette;
use crate::workspace::Workspace;

/// How often the selection and the watched files are looked at; the selection is sent
/// once it has stayed put for one look.
pub const TICK: Duration = Duration::from_millis(100);

/// More files than this changing at once (a checkout) are not followed.
const FOLLOW_MAX: usize = 8;

/// The user typed in an editor this recently: the editor stays where it is.
const TYPING: Duration = Duration::from_millis(1_500);
/// A followed file stays in front at least this long before the next one takes its place.
const DWELL: Duration = Duration::from_millis(700);
/// The hooks reported a file this recently: the watcher leaves it to them.
const HOOKED: Duration = Duration::from_secs(5);

/// The watch on the project, set up off the UI thread.
struct Watch {
    root: PathBuf,
    /// Once it is set up: the watcher, and what the project's `.gitignore` leaves out.
    on: Option<(snout_host::Watcher, ignore::gitignore::Gitignore)>,
    _setup: Task<()>,
    /// Reading the files that changed.
    reading: Option<Task<()>>,
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
    /// The hooks' endpoint, and the folder of the settings `claude` gets.
    pub hooks: Option<HookServer>,
    /// The port of the host that leads to the hooks' endpoint.
    hooks_forward: Option<snout_host::Forward>,
    settings_dir: Option<PathBuf>,
    /// The files the hooks reported, and when; the files the app wrote itself (a revert).
    hooked: HashMap<PathBuf, Instant>,
    own_writes: HashMap<PathBuf, Instant>,
    /// Each file's text before the edit under way (the hooks' `PreToolUse`).
    pre: HashMap<PathBuf, Snapshot>,
    /// The file last brought forward to follow Claude, and when; the one waiting its turn.
    last_reveal: Option<Instant>,
    queued: Option<PathBuf>,
    /// The console's turn last seen begin.
    turn_seen: Option<Instant>,
    /// Since when the ledger waits to be kept.
    ledger_since: Option<Instant>,
    _tasks: Vec<Task<()>>,
    _quit: gpui::Subscription,
    _changes: gpui::Subscription,
}

impl Drop for Bridge {
    fn drop(&mut self) {
        // The settings `claude` was given go with the window.
        if let Some(dir) = self.settings_dir.take() {
            let host = self.server.host();
            std::thread::spawn(move || {
                let _ = host.remove_dir_all(&dir);
            });
        }
    }
}

impl Bridge {
    pub fn clients(&self) -> usize {
        self.clients.len()
    }

    /// End Claude Code's connection to the window now, before its host closes. What is left
    /// to do on the host (wait for the lock file to go, remove the hooks' settings, give up
    /// the hooks' port) comes back, to run off the UI thread.
    pub fn shut_down(mut self) -> impl FnOnce() + Send + 'static {
        let gone = self.server.stop();
        let host = self.server.host();
        let (dir, forward) = (self.settings_dir.take(), self.hooks_forward.take());
        drop(self);
        move || {
            let _ = gone.recv_timeout(Duration::from_secs(5));
            if let Some(dir) = dir {
                let _ = host.remove_dir_all(&dir);
            }
            drop(forward);
        }
    }
}

/// Start the server for this window, if the app asks for one. On a remote machine the lock
/// file goes into that machine's `~/.claude/ide`, and the server and the hooks' endpoint
/// are reached through ports of that machine forwarded back here.
pub fn start(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(config) = cx.try_global::<IdeConfig>().cloned() else { return };
    let host = crate::host::host(cx);
    let lock_dir =
        if crate::remote::is_remote(cx) { super::lockfile::default_dir(&*host) } else { Some(config.lock_dir) };
    let Some(lock_dir) = lock_dir else { return };
    let root = ws.model.read(cx).project_root();
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Event>();
    let folders: Vec<PathBuf> = root.iter().cloned().collect();
    let server = match Server::start(host.clone(), &lock_dir, &folders, move |e| {
        let _ = tx.unbounded_send(e);
    }) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("Claude Code integration: could not start the server: {e}");
            return;
        }
    };
    // Claude Code's hooks report every edit to the endpoint; `claude` gets their settings.
    let (htx, mut hrx) = futures::channel::mpsc::unbounded::<HookEvent>();
    let hooks = match HookServer::start(host.clone(), move |e| {
        let _ = htx.unbounded_send(e);
    }) {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("Claude Code integration: could not start the hooks' endpoint: {e}");
            None
        }
    };
    let settings_dir = hooks
        .as_ref()
        .map(|h| host.info().temp_dir.join(format!("snout-code-claude-{}-{}", host.info().pid, h.port())));
    // What a `claude` started in the console needs (the ports on the host, the hooks'
    // settings written there) is made off the UI thread; it gets them once they are there.
    let (ready_tx, ready_rx) = futures::channel::oneshot::channel::<u16>();
    server.when_ready(move |port| {
        let _ = ready_tx.send(port);
    });
    let hooks_ready = match (&hooks, &settings_dir) {
        (Some(h), Some(dir)) => {
            let (host, dir, token, app_port) = (host.clone(), dir.clone(), h.token().to_owned(), h.port());
            Some(cx.background_spawn(async move {
                let forward = host.forward_to_app(app_port)?;
                let file = hooks::write_settings(&*host, &dir, &token)?;
                std::io::Result::Ok((forward, file))
            }))
        }
        _ => None,
    };
    cx.spawn(async move |ws, cx| {
        let Ok(ide_port) = ready_rx.await else { return };
        let mut env = Server::env(ide_port);
        let mut args = Vec::new();
        let mut codex = Vec::new();
        if let Some(task) = hooks_ready {
            match task.await {
                Ok((forward, file)) => {
                    env.push((hooks::PORT_ENV, forward.port.to_string()));
                    args = vec!["--settings".into(), file.to_string_lossy().into_owned()];
                    if let Some(dir) = file.parent() {
                        codex = hooks::codex_args(&dir.join("hook-header"), forward.port);
                    }
                    let _ = ws.update(cx, |ws, _| {
                        if let Some(b) = ws.claude.as_mut() {
                            b.hooks_forward = Some(forward);
                        }
                    });
                }
                Err(e) => eprintln!("Claude Code integration: could not set up the hooks: {e}"),
            }
        }
        cx.update(|cx| {
            cx.set_global(IdeEnv(env));
            cx.set_global(IdeArgs(args));
            cx.set_global(CodexArgs(codex));
        });
    })
    .detach();
    // Until then, a `claude` started in the console runs without them (not with those of
    // the window before, on another host).
    cx.set_global(IdeEnv::default());
    cx.set_global(IdeArgs::default());
    cx.set_global(CodexArgs::default());
    cx.set_global(IdeStatus::default());
    let hook_events = cx.spawn_in(window, async move |this, cx| {
        while let Some(event) = hrx.next().await {
            if this.update_in(cx, |ws, window, cx| on_hook(ws, event, window, cx)).is_err() {
                return;
            }
        }
    });
    let changes = changes_of(ws, cx);
    changes.update(cx, |c, _| c.root = root.clone());
    if let Some(root) = &root {
        restore_ledger(ws, root, cx);
    }
    let changes_sub = cx.subscribe(&changes, |ws, _, _: &Updated, cx| sync_bases(ws, cx));
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
    // The lock file goes when the app quits (the quit waits a moment for it).
    let quit = cx.on_app_quit(|ws, cx| {
        let gone = ws.claude.as_mut().map(|b| b.server.stop());
        cx.background_executor().spawn(async move {
            if let Some(gone) = gone {
                let _ = gone.recv_timeout(Duration::from_secs(2));
            }
        })
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
        hooks,
        hooks_forward: None,
        settings_dir,
        hooked: HashMap::new(),
        own_writes: HashMap::new(),
        pre: HashMap::new(),
        last_reveal: None,
        queued: None,
        turn_seen: None,
        ledger_since: None,
        _tasks: vec![events, ticks, hook_events],
        _quit: quit,
        _changes: changes_sub,
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
            let only =
                a.get("uri").and_then(Value::as_str).map(|u| tools::path_of(u, &crate::host::host(cx).info().home));
            // The file's name as the host resolves it, asked off the UI thread.
            let host = crate::host::host(cx);
            let canonical = cx.background_executor().spawn({
                let only = only.clone();
                async move { only.and_then(|p| host.canonicalize(&p).ok()) }
            });
            cx.spawn(async move |ws, cx| {
                let canonical = canonical.await;
                let v = ws.update(cx, |ws, cx| diagnostics(ws, only.as_deref(), canonical.as_deref(), cx));
                reply.ok(tools::json_text(&v.unwrap_or_default()));
            })
            .detach();
        }
        "checkDocumentDirty" => {
            let Some(file) = a.get("filePath").and_then(Value::as_str) else {
                return reply.error(INVALID_PARAMS, "Invalid params", Some("Missing filePath parameter".into()));
            };
            let file = file.to_owned();
            let found = editor_for(ws, &tools::path_of(&file, &crate::host::host(cx).info().home), cx);
            cx.spawn(async move |_, cx| {
                let found = found.await;
                let v = cx.update(|cx| match found {
                    Some(e) => {
                        json!({ "success": true, "filePath": file, "isDirty": e.read(cx).dirty, "isUntitled": false })
                    }
                    None => json!({ "success": false, "message": format!("Document not open: {file}") }),
                });
                reply.ok(tools::json_text(&v));
            })
            .detach();
        }
        "saveDocument" => {
            let Some(file) = a.get("filePath").and_then(Value::as_str) else {
                return reply.error(INVALID_PARAMS, "Invalid params", Some("Missing filePath parameter".into()));
            };
            let file = file.to_owned();
            let found = editor_for(ws, &tools::path_of(&file, &crate::host::host(cx).info().home), cx);
            // Found and written off the UI thread; the answer goes when it is.
            cx.spawn(async move |_, cx| {
                let Some(e) = found.await else {
                    let v = json!({ "success": false, "message": format!("Document not open: {file}") });
                    return reply.ok(tools::json_text(&v));
                };
                let saved = cx.update(|cx| e.update(cx, |e, cx| e.save(cx)));
                let v = if saved.await {
                    json!({
                        "success": true,
                        "filePath": file,
                        "saved": true,
                        "message": "Document saved successfully",
                    })
                } else {
                    json!({
                        "success": false,
                        "message": format!("Failed to save file: {}", file),
                        "filePath": file,
                    })
                };
                reply.ok(tools::json_text(&v));
            })
            .detach();
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

/// The editor tab of `path`, if one is open: by its name, else by the name it resolves to
/// on the host (asked off the UI thread).
fn editor_for(ws: &Workspace, path: &Path, cx: &mut Context<Workspace>) -> Task<Option<Entity<EditorPanel>>> {
    if let Some(e) = ws.editors(cx).into_iter().find(|e| e.read(cx).path == path) {
        return Task::ready(Some(e));
    }
    let (host, path) = (crate::host::host(cx), path.to_owned());
    let canonical = cx.background_executor().spawn(async move { host.canonicalize(&path).ok() });
    cx.spawn(async move |ws, cx| {
        let canonical = canonical.await?;
        ws.update(cx, |ws, cx| ws.editors(cx).into_iter().find(|e| e.read(cx).path == canonical)).ok().flatten()
    })
}

/// The editor tab open on `path`, by the name it goes by (the hooks report names as the
/// host resolves them, as the tabs have them).
fn open_editor(ws: &Workspace, path: &Path, cx: &App) -> Option<Entity<EditorPanel>> {
    ws.editors(cx).into_iter().find(|e| e.read(cx).path == path)
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
fn diagnostics(ws: &Workspace, only: Option<&Path>, canonical: Option<&Path>, cx: &App) -> Value {
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
                let same = path.as_deref().is_some_and(|p| p == only || canonical == Some(p));
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

fn open_file(_: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(file) = a.get("filePath").and_then(Value::as_str) else {
        return reply.error(INVALID_PARAMS, "Invalid params", Some("Missing filePath parameter".into()));
    };
    let path = tools::path_of(file, &crate::host::host(cx).info().home);
    // Whether it is there is asked of the host off the UI thread; the tab answers once it
    // shows the file.
    let host = crate::host::host(cx);
    let there = cx.background_executor().spawn({
        let path = path.clone();
        async move { host.is_file(&path) }
    });
    let a = a.clone();
    cx.spawn_in(window, async move |ws, cx| {
        if !there.await {
            return reply.error(
                TOOL_ERROR,
                "File operation error",
                Some(format!("File not found: {}", path.display())),
            );
        }
        let _ = ws.update_in(cx, |ws, window, cx| {
            let flag = |k: &str| a.get(k).and_then(Value::as_bool);
            let preview = flag("preview").unwrap_or(false);
            let front = flag("makeFrontmost").unwrap_or(true);
            let editor = ws.reveal_file(path.clone(), preview, front, window, cx);
            editor.update(cx, |e, cx| {
                e.after_load(move |e, _, cx| select_and_answer(e, &path, &a, front, reply, cx), window, cx)
            });
        });
    })
    .detach();
}

/// openFile's selection, in the tab that shows the file, and its answer.
fn select_and_answer(e: &mut EditorPanel, path: &Path, a: &Value, front: bool, reply: Reply, cx: &mut App) {
    let flag = |k: &str| a.get(k).and_then(Value::as_bool);
    let mut message = format!("Opened file: {}", path.display());
    let text = e.text(cx);
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
        let state = e.state.clone();
        state.update(cx, |s, cx| s.set_selected_range(range, cx));
    }
    if front {
        reply.ok(tools::text(message));
    } else {
        reply.ok(tools::json_text(&json!({
            "success": true,
            "filePath": path.to_string_lossy(),
            "languageId": tools::language_id(path),
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
    let path = tools::path_of(&new_path, &crate::host::host(cx).info().home);
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
    track_console_turn(ws, cx);
    // The tabs' note of who writes now, and whether they follow.
    let settings = &ws.model.read(cx).settings;
    let watching = watched_agent(ws, cx).map(|id| agents::Agent::of(id, settings));
    let follow = settings.follow_claude;
    if cx.try_global::<agents::Watching>().is_none_or(|w| w.0 != watching) {
        cx.set_global(agents::Watching(watching));
    }
    if cx.try_global::<agents::Following>().is_none_or(|f| f.0 != follow) {
        cx.set_global(agents::Following(follow));
    }
    track_selection(ws, window, cx);
    follow_watched(ws, window, cx);
    // The lock file names the project open now.
    let root = ws.model.read(cx).project_root();
    if let Some(b) = ws.claude.as_mut()
        && b.root != root
    {
        b.root = root.clone();
        b.server.set_folders(&root.iter().cloned().collect::<Vec<_>>());
        changes_of(ws, cx).update(cx, |c, _| {
            c.root = root.clone();
            c.ledger = Default::default();
        });
        if let Some(root) = &root {
            restore_ledger(ws, root, cx);
        }
    }
    track_reviews(ws, cx);
    sync_bases(ws, cx);
    keep_ledger(ws, cx);
    reveal_queued(ws, window, cx);
    // Claude went quiet: the indicator goes.
    changes_of(ws, cx).update(cx, |c, cx| {
        if c.activity
            .as_ref()
            .is_some_and(|a| a.at.elapsed() >= super::panel::ACTIVE || (a.done && a.at.elapsed() >= LINGER))
        {
            c.activity = None;
            cx.notify();
        }
    });
    if let Some(b) = ws.claude.as_mut() {
        b.hooked.retain(|_, t| t.elapsed() < HOOKED);
        b.own_writes.retain(|_, t| t.elapsed() < HOOKED);
    }
}

/// A turn of an agent the console runs begins (Return, and it went to work): a new turn in
/// the change set, unless the agent's own hooks start turns themselves (Claude Code's).
fn track_console_turn(ws: &mut Workspace, cx: &mut Context<Workspace>) {
    let Some(turn) = cx.try_global::<ConsoleTurn>().cloned() else { return };
    let Some(b) = ws.claude.as_mut() else { return };
    let started = turn.since.filter(|_| turn.active);
    if started == b.turn_seen {
        return;
    }
    b.turn_seen = started;
    let (Some(_), Some(agent)) = (started, turn.agent) else { return };
    let hooked = agent == agents::CLAUDE && changes_of(ws, cx).read(cx).hooks_heard;
    if !hooked {
        changes_of(ws, cx).update(cx, |c, cx| {
            c.set.new_turn(agent, None);
            cx.notify();
        });
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

/// The `at_mentioned` notification for `path`, its lines `lines` (0-based, inclusive) or
/// the whole file. Claude Code takes the lines as numbers or not at all: a `null` makes it
/// drop the whole notification, so a whole file leaves them out.
pub fn mention_params(path: &Path, lines: Option<(u32, u32)>) -> Value {
    let mut v = json!({ "filePath": path.to_string_lossy() });
    if let Some((a, b)) = lines {
        v["lineStart"] = json!(a);
        v["lineEnd"] = json!(b);
    }
    v
}

/// The mention as Claude Code types it into its prompt: `@path#L3-5 ` (1-based), the path
/// from the project.
pub fn mention_text(rel: &str, lines: Option<(u32, u32)>) -> String {
    match lines {
        Some((a, b)) if a == b => format!("@{rel}#L{} ", a + 1),
        Some((a, b)) => format!("@{rel}#L{}-{} ", a + 1, b + 1),
        None => format!("@{rel} "),
    }
}

/// What the note by the console says was sent.
pub fn sent_note(name: &str, lines: Option<(u32, u32)>, agent: &str) -> String {
    match lines {
        Some((a, b)) if a == b => format!("Sent {name} line {} to {agent}", a + 1),
        Some((a, b)) => format!("Sent {name} lines {}\u{2013}{} to {agent}", a + 1, b + 1),
        None => format!("Sent {name} to {agent}"),
    }
}

/// ⌥⌘K: tell the agent about the selection (or the file) in the editor in front, and bring
/// its console forward. Claude Code connected to the window hears it as an `at_mentioned`
/// and puts the mention in its prompt; any other agent (or a `claude` not connected) gets
/// the mention typed into the console. The lines sent light up for a moment, and a note by
/// the console says what went.
pub fn send_selection(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(editor) = ws.active_editor(window, cx).filter(|e| e.read(cx).class_file.is_none()) else { return };
    let (path, name, lines) = {
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
        (e.path.clone(), e.name().to_string(), lines)
    };
    let connected = ws.claude.as_ref().is_some_and(|b| b.clients() > 0);
    if connected && let Some(b) = ws.claude.as_ref() {
        b.server.notify("at_mentioned", mention_params(&path, lines));
    }
    show_console(ws, window, cx);
    let console = crate::console::console_of(ws, cx);
    let root = ws.model.read(cx).project_root();
    if !connected {
        let rel = root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(&path).to_string_lossy().into_owned();
        if let (Some(console), Some(root)) = (console.as_ref(), root.as_ref()) {
            console.update(cx, |c, cx| c.type_when_ready(root, &mention_text(&rel, lines), cx));
        }
    }
    let shown: Range<usize> = match lines {
        Some((a, b)) => a as usize..b as usize + 1,
        None => 0..editor.read(cx).line_count(cx),
    };
    let color = palette(cx).claude().mark;
    editor.update(cx, |e, cx| e.light_in(std::iter::once(shown).collect(), color, window, cx));
    if let Some(console) = console {
        let note = sent_note(&name, lines, "Claude");
        console.update(cx, |c, cx| c.show_note(note, cx));
    }
}

/// How long after the `at_mentioned` the words of a request are typed, so they follow the
/// mention in Claude Code's prompt.
const AFTER_MENTION: std::time::Duration = std::time::Duration::from_millis(250);

/// The prompt typed for an agent that hears no `at_mentioned`: the mention of the file and
/// lines, then `text`.
pub fn ask_prompt(rel: &str, lines: (u32, u32), text: &str) -> String {
    format!("{}{text}", mention_text(rel, Some(lines)))
}

/// Ask to Fix (the hover card, the context actions, the Problems window): put the file's
/// `lines` and `text` (what is wrong) in the prompt of the console's agent, and bring the
/// console forward; the user reads it and presses Return. Claude Code connected to the window
/// hears the lines as an `at_mentioned`, the words typed after it; any other agent (Codex, a
/// command of your own, a `claude` not connected) gets the mention typed with them.
pub fn ask_to_fix(
    ws: &mut Workspace,
    path: PathBuf,
    lines: (u32, u32),
    text: &str,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let agent = agents::console_agent(cx);
    let connected = agent.id == agents::CLAUDE && ws.claude.as_ref().is_some_and(|b| b.clients() > 0);
    if connected && let Some(b) = ws.claude.as_ref() {
        b.server.notify("at_mentioned", mention_params(&path, Some(lines)));
    }
    show_console(ws, window, cx);
    let console = crate::console::console_of(ws, cx);
    let root = ws.model.read(cx).project_root();
    let rel = root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(&path).to_string_lossy().into_owned();
    if let (Some(console), Some(root)) = (console.as_ref(), root.clone()) {
        if connected {
            let (console, words) = (console.downgrade(), text.to_owned());
            cx.spawn(async move |_, cx| {
                cx.background_executor().timer(AFTER_MENTION).await;
                let _ = console.update(cx, |c, cx| c.type_when_ready(&root, &words, cx));
            })
            .detach();
        } else {
            console.update(cx, |c, cx| c.type_when_ready(&root, &ask_prompt(&rel, lines, text), cx));
        }
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(editor) = ws.editors(cx).into_iter().find(|e| e.read(cx).path == path) {
        let color = agent.color(&palette(cx));
        let shown: Range<usize> = lines.0 as usize..lines.1 as usize + 1;
        editor.update(cx, |e, cx| e.light_in(std::iter::once(shown).collect(), color, window, cx));
    }
    if let Some(console) = console {
        let note = match lines {
            (a, b) if a == b => format!("Asked {} to fix {name} line {}", agent.short, a + 1),
            (a, b) => format!("Asked {} to fix {name} lines {}\u{2013}{}", agent.short, a + 1, b + 1),
        };
        console.update(cx, |c, cx| c.show_note(note, cx));
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

/// The agent a change on disk is put down to now, if any: the agent whose turn runs in the
/// console (or ended a moment ago), else Claude Code while it is connected; never while a
/// Git job of the app's own is changing files.
pub fn watched_agent(ws: &Workspace, cx: &App) -> Option<&'static str> {
    if ws.model.read(cx).job.is_some() {
        return None;
    }
    let turn = cx.try_global::<ConsoleTurn>();
    let console = turn.filter(|t| t.at_work()).and_then(|t| t.agent);
    let claude = ws.claude.as_ref().is_some_and(|b| b.clients() > 0).then_some(agents::CLAUDE);
    console.or(claude)
}

/// An open tab took its file's new text from disk: follow it there, or mark the tab.
pub fn followed_reload(
    ws: &mut Workspace,
    editor: &Entity<EditorPanel>,
    before: &str,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(agent) = watched_agent(ws, cx) else { return };
    let path = editor.read(cx).path.clone();
    if left_to_hooks(ws, &path) {
        return;
    }
    // The file's text (the tab may still be playing the change out).
    let after = editor.read(cx).saved_text().to_owned();
    let lines = changed_lines(before, &after);
    if lines.is_empty() {
        return;
    }
    // Claude Code without the hooks (started elsewhere), or an agent only watched: the
    // change waits for review too, and the lines are the agent's.
    note_edit(ws, &path, Some(before), &after, agent, "", cx);
    let base = Some(Some(Arc::new(before.to_owned())));
    changes_of(ws, cx).update(cx, |c, cx| {
        c.record(path, Some(Arc::new(after.clone())), base, String::new(), Source::Watcher, agent, cx)
    });
    // The tab plays the change out itself, in the agent's colour.
    if ws.model.read(cx).settings.follow_claude {
        let (path, preview) = (editor.read(cx).path.clone(), editor.read(cx).preview);
        ws.reveal_file(path, preview, true, window, cx);
    } else {
        editor.update(cx, |e, cx| {
            e.touched = true;
            cx.notify();
        });
    }
}

/// Watch `root` on the workspace's host, off the UI thread.
fn watch(root: &Path, cx: &mut Context<Workspace>) -> Watch {
    let host = crate::host::host(cx);
    let setup = cx.background_executor().spawn({
        let root = root.to_owned();
        async move {
            let watcher = host.watch(&root).ok()?;
            let mut ignore = ignore::gitignore::GitignoreBuilder::new(&root);
            for line in host.read_to_string(&root.join(".gitignore")).unwrap_or_default().lines() {
                let _ = ignore.add_line(None, line);
            }
            Some((watcher, ignore.build().unwrap_or_else(|_| ignore::gitignore::Gitignore::empty())))
        }
    });
    let root_buf = root.to_owned();
    let task = cx.spawn(async move |ws, cx| {
        let on = setup.await;
        let _ = ws.update(cx, |ws, _| {
            if let Some(w) = ws.claude.as_mut().and_then(|b| b.watch.as_mut()).filter(|w| w.root == root_buf) {
                w.on = on;
            }
        });
    });
    Watch { root: root.to_owned(), on: None, _setup: task, reading: None }
}

/// Files changed on disk that no tab shows: follow Claude into them. They are read off the
/// UI thread; what they held before comes from the last read, else from the last commit.
fn follow_watched(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let root = ws.model.read(cx).project_root();
    let Some(b) = ws.claude.as_mut() else { return };
    if b.watch.as_ref().map(|w| &w.root) != root.as_ref() {
        b.watch = None;
        b.known.clear();
        if let Some(root) = &root {
            b.watch = Some(watch(root, cx));
        }
    }
    let Some(b) = ws.claude.as_mut() else { return };
    let Some(w) = b.watch.as_mut() else { return };
    if w.reading.is_some() {
        return;
    }
    let Some((watcher, ignore)) = w.on.as_ref() else { return };
    let mut changed: Vec<PathBuf> = Vec::new();
    for event in watcher.events.try_iter().filter(|e| e.kind != snout_host::WatchKind::Access) {
        for path in event.paths {
            if !changed.contains(&path) {
                changed.push(path);
            }
        }
    }
    let root = w.root.clone();
    changed.retain(|p| {
        p.starts_with(&root)
            && !p.strip_prefix(&root).is_ok_and(|r| r.components().any(|c| c.as_os_str() == ".git"))
            && !ignore.matched_path_or_any_parents(p, false).is_ignore()
    });
    if changed.is_empty() {
        return;
    }
    let unknown: Vec<bool> = changed.iter().map(|p| !b.known.contains_key(p)).collect();
    let host = crate::host::host(cx);
    let read = cx.background_executor().spawn({
        let root = root.clone();
        async move {
            changed
                .into_iter()
                .zip(unknown)
                .filter(|(p, _)| host.is_file(p))
                .filter_map(|(path, unknown)| {
                    let text = host.read_to_string(&path).ok()?;
                    let head = unknown
                        .then(|| {
                            let rel = path.strip_prefix(&root).ok()?.to_string_lossy().replace('\\', "/");
                            snout_core::gutter::head_text(&*host, &root, &rel)
                        })
                        .flatten();
                    Some((path, text, head))
                })
                .collect::<Vec<_>>()
        }
    });
    w.reading = Some(cx.spawn_in(window, async move |ws, cx| {
        let files = read.await;
        let _ = ws.update_in(cx, |ws, window, cx| {
            if let Some(w) = ws.claude.as_mut().and_then(|b| b.watch.as_mut()) {
                w.reading = None;
            }
            follow_changed(ws, &root, files, window, cx);
        });
    }));
}

/// Follow Claude into the files read, `(path, text, text at the last commit)`.
fn follow_changed(
    ws: &mut Workspace,
    root: &Path,
    files: Vec<(PathBuf, String, Option<String>)>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if ws.model.read(cx).project_root().as_deref() != Some(root) {
        return;
    }
    let agent = watched_agent(ws, cx);
    let connected = agent.is_some();
    let follow = connected && ws.model.read(cx).settings.follow_claude && files.len() <= FOLLOW_MAX;
    let open: Vec<PathBuf> = ws.editors(cx).into_iter().map(|e| e.read(cx).path.clone()).collect();
    for (path, text, head) in files {
        let hooked = left_to_hooks(ws, &path);
        let Some(b) = ws.claude.as_mut() else { return };
        if b.known.len() > 512 {
            b.known.clear();
        }
        let before = b.known.insert(path.clone(), text.clone());
        // Open tabs follow through their own reload; the hooks show their own files.
        if hooked || open.contains(&path) {
            continue;
        }
        let before = before.or(head).unwrap_or_default();
        if before == text {
            continue;
        }
        if let Some(agent) = agent {
            note_edit(ws, &path, Some(&before), &text, agent, "", cx);
        }
        if connected {
            let base = Some(Some(Arc::new(before.clone())));
            let now = Some(Arc::new(text.clone()));
            changes_of(ws, cx).update(cx, |c, cx| {
                c.record(path.clone(), now, base, String::new(), Source::Watcher, agent.unwrap_or(agents::CLAUDE), cx)
            });
        }
        if !follow {
            continue;
        }
        let writer = agent.map(|id| writer_of(ws, id, cx));
        let editor = ws.reveal_file(path, true, true, window, cx);
        editor.update(cx, |e, cx| e.animate_from(&before, &text, writer, window, cx));
    }
}

// ----- Claude's edits, as its hooks report them -------------------------------------

/// An agent changed `path` from `before` to `after`: the ledger puts the lines it changed
/// down to it, with the turn and its prompt.
fn note_edit(
    ws: &Workspace,
    path: &Path,
    before: Option<&str>,
    after: &str,
    agent: &'static str,
    tool: &str,
    cx: &mut Context<Workspace>,
) {
    changes_of(ws, cx).update(cx, |c, _| {
        let (turn, prompt) = c
            .set
            .turns
            .iter()
            .rev()
            .find(|t| t.agent == agent)
            .map_or((c.set.turn, None), |t| (t.id, t.prompt.clone()));
        let edit = super::ledger::EditRecord::now(agent, turn, prompt, tool);
        c.ledger.record(path, before, after, edit);
        c.ledger_changed = true;
    });
}

/// An editor's lines moved with what was typed there: the ledger takes them.
pub fn take_lines(ws: &mut Workspace, editor: &Entity<EditorPanel>, cx: &mut Context<Workspace>) {
    let (path, owners) = {
        let e = editor.read(cx);
        (e.path.clone(), e.agent_lines.clone())
    };
    let Some(owners) = owners else { return };
    changes_of(ws, cx).update(cx, |c, _| {
        c.ledger.take(&path, owners);
        c.ledger_changed = true;
    });
}

/// How long the ledger waits after a change before it is kept.
const KEEP_AFTER: Duration = Duration::from_secs(2);

/// Keep the ledger of the project in the app's folder (off the UI thread), a moment after
/// it changed.
fn keep_ledger(ws: &mut Workspace, cx: &mut Context<Workspace>) {
    let Some(dir) = ws.model.read(cx).store_dir() else { return };
    let Some(root) = ws.model.read(cx).project_root() else { return };
    let changes = changes_of(ws, cx);
    let Some(b) = ws.claude.as_mut() else { return };
    if !changes.read(cx).ledger_changed {
        b.ledger_since = None;
        return;
    }
    let since = *b.ledger_since.get_or_insert_with(Instant::now);
    if since.elapsed() < KEEP_AFTER {
        return;
    }
    b.ledger_since = None;
    let saved = changes.update(cx, |c, _| {
        c.ledger_changed = false;
        c.ledger.to_saved(&root)
    });
    let file = super::ledger::saved_path(&dir, &root);
    cx.background_executor()
        .spawn(async move {
            let Ok(text) = serde_json::to_string(&saved) else { return };
            if let Some(parent) = file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&file, text);
        })
        .detach();
}

/// Take back the project's ledger kept by an earlier run: a file's lines only if the file
/// on the host still holds the text they were recorded for.
fn restore_ledger(ws: &mut Workspace, root: &Path, cx: &mut Context<Workspace>) {
    let Some(dir) = ws.model.read(cx).store_dir() else { return };
    let file = super::ledger::saved_path(&dir, root);
    let host = crate::host::host(cx);
    let read = cx.background_executor().spawn(async move {
        let saved: super::ledger::Saved = serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()?;
        Some(super::ledger::Ledger::restore(saved, |p| host.read_to_string(p).ok()))
    });
    let root = root.to_owned();
    cx.spawn(async move |ws, cx| {
        let Some(ledger) = read.await else { return };
        let _ = ws.update(cx, |ws, cx| {
            if ws.model.read(cx).project_root().as_deref() != Some(root.as_path()) {
                return;
            }
            changes_of(ws, cx).update(cx, |c, cx| {
                if c.ledger.edits.is_empty() {
                    c.ledger = ledger;
                    cx.notify();
                }
            });
            sync_bases(ws, cx);
        });
    })
    .detach();
}

/// The indicator stays this long after the tool ran.
const LINGER: Duration = Duration::from_millis(1_500);

/// The window's change set.
pub fn changes_of(ws: &Workspace, cx: &App) -> Entity<ClaudeChanges> {
    ws.claude_changes.read(cx).changes.clone()
}

/// The hooks (or the app itself) have this file: the watcher stays out of it.
fn left_to_hooks(ws: &Workspace, path: &Path) -> bool {
    ws.claude.as_ref().is_some_and(|b| {
        b.hooked.get(path).is_some_and(|t| t.elapsed() < HOOKED)
            || b.own_writes.get(path).is_some_and(|t| t.elapsed() < HOOKED)
    })
}

/// The user is typing in an editor that has the keyboard.
fn user_typing(ws: &Workspace, window: &Window, cx: &App) -> bool {
    ws.editors(cx).iter().any(|e| {
        let e = e.read(cx);
        e.edited_at.is_some_and(|t| t.elapsed() < TYPING) && e.state.read(cx).focus_handle(cx).is_focused(window)
    })
}

pub fn on_hook(ws: &mut Workspace, e: HookEvent, window: &mut Window, cx: &mut Context<Workspace>) {
    let changes = changes_of(ws, cx);
    match e.kind {
        HookKind::TurnComplete => changes.update(cx, |c, cx| {
            c.set.turn_ended(e.agent, e.prompt.clone());
            cx.emit(Updated);
            cx.notify();
        }),
        HookKind::Prompt => changes.update(cx, |c, cx| {
            c.set.new_turn(e.agent, e.prompt.clone());
            c.activity = None;
            cx.notify();
        }),
        HookKind::Stop => changes.update(cx, |c, cx| {
            if let Some(a) = c.activity.as_mut() {
                a.done = true;
            }
            cx.notify();
        }),
        HookKind::PreTool => {
            let Some(path) = e.path else { return };
            let snap = e.file.clone().unwrap_or(Snapshot::Unread);
            changes.update(cx, |c, cx| {
                c.hooks_heard = true;
                match &snap {
                    Snapshot::Text(t) => c.set.before_edit(&path, Some(t.clone())),
                    Snapshot::Missing => c.set.before_edit(&path, None),
                    Snapshot::Unread => {}
                }
                c.activity =
                    Some(Activity { path: path.clone(), tool: e.tool.clone(), at: Instant::now(), done: false });
                cx.notify();
            });
            // An open tab waits for the report rather than reloading on its own.
            if let Some(editor) = open_editor(ws, &path, cx) {
                editor.update(cx, |e, _| e.claude_hold = Some(Instant::now()));
            }
            if let Some(b) = ws.claude.as_mut() {
                b.hooked.insert(path.clone(), Instant::now());
                b.pre.insert(path, snap);
            }
        }
        HookKind::PostTool => {
            let Some(path) = e.path else { return };
            let after = e.file.clone().unwrap_or(Snapshot::Unread);
            let before = ws.claude.as_mut().and_then(|b| {
                b.hooked.insert(path.clone(), Instant::now());
                b.pre.remove(&path)
            });
            // The ledger: these lines are the agent's now.
            if let Some(text) = after.text() {
                let before = before.as_ref().and_then(Snapshot::text).map(|t| t.as_str().to_owned());
                note_edit(ws, &path, before.as_deref(), text, e.agent, &e.tool, cx);
            }
            changes.update(cx, |c, cx| {
                c.hooks_heard = true;
                c.activity =
                    Some(Activity { path: path.clone(), tool: e.tool.clone(), at: Instant::now(), done: true });
                let now = match &after {
                    Snapshot::Text(t) => Some(Some(t.clone())),
                    Snapshot::Missing => Some(None),
                    Snapshot::Unread => None,
                };
                if let Some(now) = now {
                    c.record(path.clone(), now, None, e.tool.clone(), Source::Hook, e.agent, cx);
                }
                cx.notify();
            });
            show_edit(ws, &path, before.as_ref().and_then(Snapshot::text).cloned(), after.text().cloned(), window, cx);
        }
    }
}

/// Bring Claude's edit of `path` into view and play it out, or mark the tab when the
/// editor should stay where it is.
fn show_edit(
    ws: &mut Workspace,
    path: &Path,
    before: Option<Arc<String>>,
    after: Option<Arc<String>>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let open = open_editor(ws, path, cx);
    let Some(after) = after else {
        if let Some(editor) = open {
            editor.update(cx, |e, _| e.claude_hold = None);
        }
        return;
    };
    let follow = ws.model.read(cx).settings.follow_claude;
    let typing = user_typing(ws, window, cx);
    let active = ws.active_editor(window, cx).is_some_and(|e| e.read(cx).path == path);
    let recent = ws.claude.as_ref().and_then(|b| b.last_reveal).is_some_and(|t| t.elapsed() < DWELL);
    // In front now (it is, or it may come forward), or later (another file just did).
    let front = follow && !typing && (active || !recent);
    if follow
        && !typing
        && !front
        && let Some(b) = ws.claude.as_mut()
    {
        b.queued = Some(path.to_owned());
    }
    match open {
        Some(editor) => {
            if front && !active {
                let preview = editor.read(cx).preview;
                ws.reveal_file(path.to_owned(), preview, true, window, cx);
                note_reveal(ws);
            }
            let visible = front || active;
            let writer = Some(writer_of(ws, agents::CLAUDE, cx));
            let shown = editor.update(cx, |e, cx| e.animate_edit(&after, visible, writer, window, cx));
            if shown && !visible {
                editor.update(cx, |e, cx| {
                    e.touched = true;
                    cx.notify();
                });
            }
        }
        None if front => {
            let editor = ws.reveal_file(path.to_owned(), true, true, window, cx);
            note_reveal(ws);
            let before = before.map(|b| b.as_str().to_owned()).unwrap_or_default();
            let writer = Some(writer_of(ws, agents::CLAUDE, cx));
            editor.update(cx, |e, cx| e.animate_from(&before, &after, writer, window, cx));
        }
        None => {}
    }
}

/// How `agent` writes in a tab: its name on the caret's flag, its colour.
fn writer_of(ws: &Workspace, agent: &str, cx: &App) -> crate::editor::Writer {
    crate::editor::Writer::of(&agents::Agent::of(agent, &ws.model.read(cx).settings), &palette(cx))
}

fn note_reveal(ws: &mut Workspace) {
    if let Some(b) = ws.claude.as_mut() {
        b.last_reveal = Some(Instant::now());
    }
}

/// A file that changed while another was in front: its turn, once that one has been seen.
fn reveal_queued(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(b) = ws.claude.as_ref() else { return };
    if b.queued.is_none() || b.last_reveal.is_some_and(|t| t.elapsed() < DWELL) || user_typing(ws, window, cx) {
        return;
    }
    let Some(path) = ws.claude.as_mut().and_then(|b| b.queued.take()) else { return };
    let busy = changes_of(ws, cx).read(cx).busy().map(|a| a.path.clone());
    // Claude moved on to another file meanwhile: that one shows itself.
    if busy.as_ref().is_some_and(|p| *p != path) && open_editor(ws, busy.as_deref().unwrap_or(&path), cx).is_some() {
        return;
    }
    let lines: Vec<Range<usize>> = open_editor(ws, &path, cx)
        .map(|e| e.read(cx).claude_hunks.iter().map(|h| h.new_lines.clone()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default();
    let editor = ws.reveal_file(path, true, true, window, cx);
    note_reveal(ws);
    editor.update(cx, |e, cx| {
        e.touched = false;
        e.after_load(
            move |e, window, cx| {
                let color = e.agent_color_of(lines.first().map_or(0, |l| l.start), cx);
                e.light_in(lines, color, window, cx)
            },
            window,
            cx,
        )
    });
}

/// One shared empty text: the snapshot of every file Claude made.
fn empty() -> Arc<String> {
    static EMPTY: OnceLock<Arc<String>> = OnceLock::new();
    EMPTY.get_or_init(|| Arc::new(String::new())).clone()
}

/// Every open tab marks the changes waiting for review in its file.
pub fn sync_bases(ws: &mut Workspace, cx: &mut Context<Workspace>) {
    let changes = changes_of(ws, cx);
    let settings = ws.model.read(cx).settings.clone();
    for editor in ws.editors(cx) {
        let file = changes.read(cx).set.file(&editor.read(cx).path).cloned();
        let base = file.as_ref().map(|f| f.base.clone().unwrap_or_else(empty));
        let agent = file.map(|f| agents::Agent::of(f.agent, &settings));
        let (owners, edits) = {
            let c = changes.read(cx);
            (c.ledger.files.get(&editor.read(cx).path).cloned(), c.ledger.edits.clone())
        };
        editor.update(cx, |e, cx| {
            if e.claude_agent != agent {
                e.claude_agent = agent;
                cx.notify();
            }
            let same = match (&e.agent_lines, &owners) {
                (Some(a), Some(b)) => Arc::ptr_eq(&a.lines, &b.lines) || a.text.as_str() != b.text.as_str(),
                (None, None) => true,
                _ => false,
            };
            if !same || !Arc::ptr_eq(&e.agent_edits, &edits) {
                // The ledger's record, unless the tab has moved on from the text it is for.
                if owners.as_ref().is_none_or(|o| o.text.as_str() == e.text(cx)) {
                    e.agent_lines = owners;
                }
                e.agent_edits = edits;
                cx.notify();
            }
            e.set_claude_base(base, cx)
        });
    }
    // The Project tree's dots and the status bar's note.
    let files: std::collections::HashMap<PathBuf, &'static str> =
        changes.read(cx).set.files.iter().map(|f| (f.path.clone(), f.agent)).collect();
    let mut count: std::collections::HashMap<&'static str, usize> = Default::default();
    for a in files.values() {
        *count.entry(a).or_default() += 1;
    }
    let summary = super::AgentSummary {
        files: files.len(),
        agent: count.into_iter().max_by_key(|(a, n)| (*n, *a == agents::CLAUDE)).map(|(a, _)| a),
    };
    if cx.try_global::<super::AgentFiles>().is_none_or(|f| f.0 != files) {
        cx.set_global(super::AgentFiles(files));
    }
    if cx.try_global::<super::AgentSummary>() != Some(&summary) {
        cx.set_global(summary);
    }
}

/// The changes Claude asks to make (`openDiff`), as the Claude Changes window lists them.
fn track_reviews(ws: &mut Workspace, cx: &mut Context<Workspace>) {
    let Some(b) = ws.claude.as_mut() else { return };
    b.reviews.retain(|_, e| e.upgrade().is_some());
    let mut pending: Vec<PendingDiff> = Vec::new();
    for (tab, editor) in &b.reviews {
        let Some(editor) = editor.upgrade() else { continue };
        let e = editor.read(cx);
        if let Some(r) = e.review.as_ref().filter(|r| r.tab_name == *tab) {
            let (added, removed) = review::counts(&r.marks);
            let path = e.path.clone();
            pending.push(PendingDiff { tab: tab.clone(), path, added, removed, new_file: r.new_file, front: true });
        } else if e.queue.iter().any(|p| p.tab_name == *tab) {
            let path = e.path.clone();
            pending.push(PendingDiff { tab: tab.clone(), path, added: 0, removed: 0, new_file: false, front: false });
        }
    }
    pending.sort_by(|a, b| (!a.front, &a.path, &a.tab).cmp(&(!b.front, &b.path, &b.tab)));
    let changes = changes_of(ws, cx);
    if changes.read(cx).pending != pending {
        changes.update(cx, |c, cx| {
            c.pending = pending;
            cx.notify();
        });
    }
}

// ----- Reviewing Claude's changes: from the window, the tab's bar and the keys ---------

/// Write what a revert puts back (on the host, off the UI thread), and show it in the
/// file's tab.
fn restore(ws: &mut Workspace, r: Restore, window: &mut Window, cx: &mut Context<Workspace>) {
    if let Some(b) = ws.claude.as_mut() {
        b.own_writes.insert(r.path.clone(), Instant::now());
        if let Some(text) = &r.text {
            b.known.insert(r.path.clone(), text.clone());
        } else {
            b.known.remove(&r.path);
        }
    }
    changes_of(ws, cx).update(cx, |c, _| {
        match &r.text {
            Some(text) => c.ledger.sync(&r.path, text),
            None => {
                c.ledger.files.remove(&r.path);
            }
        }
        c.ledger_changed = true;
    });
    let host = crate::host::host(cx);
    let write = cx.background_executor().spawn({
        let (path, text) = (r.path.clone(), r.text.clone());
        async move {
            match &text {
                Some(text) => {
                    if let Some(dir) = path.parent() {
                        let _ = host.create_dir_all(dir);
                    }
                    host.write(&path, text.as_bytes())
                }
                None => match host.remove_file(&path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                },
            }
        }
    });
    cx.spawn_in(window, async move |ws, cx| {
        if let Err(e) = write.await {
            eprintln!("Claude Code integration: could not revert {}: {e}", r.path.display());
            return;
        }
        let _ = ws.update_in(cx, |ws, window, cx| {
            if let Some(b) = ws.claude.as_mut() {
                b.own_writes.insert(r.path.clone(), Instant::now());
            }
            let editor = open_editor(ws, &r.path, cx);
            match (&r.text, editor) {
                (Some(text), Some(editor)) => {
                    editor.update(cx, |e, cx| {
                        e.animate_edit(text, false, None, window, cx);
                    });
                }
                (None, Some(editor)) => {
                    editor.update(cx, |e, cx| e.discard(cx));
                    ws.dock.update(cx, |d, cx| d.remove_panel(editor, window, cx));
                }
                (_, None) => {}
            }
            ws.model.update(cx, |m, cx| {
                if m.job.is_none() {
                    m.refresh(cx);
                }
            });
        });
    })
    .detach();
}

pub fn accept_file(ws: &mut Workspace, path: &Path, _: &mut Window, cx: &mut Context<Workspace>) {
    changes_of(ws, cx).update(cx, |c, cx| {
        c.set.accept_file(path);
        cx.emit(Updated);
        cx.notify();
    });
}

pub fn revert_file(ws: &mut Workspace, path: &Path, window: &mut Window, cx: &mut Context<Workspace>) {
    let r = changes_of(ws, cx).update(cx, |c, cx| {
        let r = c.set.revert_file(path);
        cx.emit(Updated);
        cx.notify();
        r
    });
    if let Some(r) = r {
        restore(ws, r, window, cx);
    }
}

/// Keep every change of turn `id`.
pub fn accept_turn(ws: &mut Workspace, id: u32, window: &mut Window, cx: &mut Context<Workspace>) {
    for path in changes_of(ws, cx).read(cx).set.files_of_turn(id) {
        accept_file(ws, &path, window, cx);
    }
}

/// Put back every file turn `id` changed.
pub fn revert_turn(ws: &mut Workspace, id: u32, window: &mut Window, cx: &mut Context<Workspace>) {
    for path in changes_of(ws, cx).read(cx).set.files_of_turn(id) {
        revert_file(ws, &path, window, cx);
    }
}

pub fn accept_all(ws: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>) {
    changes_of(ws, cx).update(cx, |c, cx| {
        c.set.accept_all();
        cx.emit(Updated);
        cx.notify();
    });
}

pub fn revert_all(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let all = changes_of(ws, cx).update(cx, |c, cx| {
        let all = c.set.revert_all();
        cx.emit(Updated);
        cx.notify();
        all
    });
    for r in all {
        restore(ws, r, window, cx);
    }
}

pub fn accept_hunk(ws: &mut Workspace, path: &Path, ix: usize, _: &mut Window, cx: &mut Context<Workspace>) {
    changes_of(ws, cx).update(cx, |c, cx| {
        if c.set.accept_hunk(path, ix).is_some() {
            c.rediff(path, cx);
        }
    });
}

pub fn revert_hunk(ws: &mut Workspace, path: &Path, ix: usize, window: &mut Window, cx: &mut Context<Workspace>) {
    let r = changes_of(ws, cx).update(cx, |c, cx| {
        let r = c.set.revert_hunk(path, ix);
        if r.is_some() {
            c.rediff(path, cx);
        }
        r
    });
    if let Some(r) = r {
        restore(ws, r, window, cx);
    }
}

/// Open `path` on Claude's changes, shown inline: at `line`, or at the first.
pub fn open_change(
    ws: &mut Workspace,
    path: &Path,
    line: Option<usize>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if open_editor(ws, path, cx).is_none() {
        // Not open: whether it is still there is asked of the host off the UI thread.
        let host = crate::host::host(cx);
        let there = cx.background_executor().spawn({
            let path = path.to_owned();
            async move { host.is_file(&path) }
        });
        let path = path.to_owned();
        cx.spawn_in(window, async move |ws, cx| {
            if there.await {
                let _ = ws.update_in(cx, |ws, window, cx| show_change(ws, &path, line, window, cx));
            }
        })
        .detach();
        return;
    }
    show_change(ws, path, line, window, cx);
}

fn show_change(ws: &mut Workspace, path: &Path, line: Option<usize>, window: &mut Window, cx: &mut Context<Workspace>) {
    let first = changes_of(ws, cx).read(cx).set.file(path).and_then(|f| f.hunks.first().map(|h| h.new_lines.start));
    let editor = ws.reveal_file(path.to_owned(), false, true, window, cx);
    sync_bases(ws, cx);
    if let Some(line) = line.or(first) {
        editor.update(cx, |e, cx| e.after_load(move |e, window, cx| e.go_to_claude_line(line, window, cx), window, cx));
    }
    editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.focus(window, cx)));
}

/// Accept or reject the change Claude asks to make in the tab `tab`.
pub fn answer_review(ws: &mut Workspace, tab: &str, accept: bool, window: &mut Window, cx: &mut Context<Workspace>) {
    let editor = ws.claude.as_ref().and_then(|b| b.reviews.get(tab)).and_then(|e| e.upgrade());
    let Some(editor) = editor else { return };
    editor.update(cx, |e, cx| {
        let front = e.review.as_ref().is_some_and(|r| r.tab_name == tab);
        match (front, accept) {
            (true, true) => e.accept_review(window, cx),
            (true, false) => e.reject_review(window, cx),
            (false, false) => {
                e.close_review(tab, window, cx);
            }
            (false, true) => {}
        }
    });
    track_reviews(ws, cx);
}

/// What an editor asks of Claude's changes.
pub fn editor_request(
    ws: &mut Workspace,
    editor: &Entity<EditorPanel>,
    request: &ClaudeRequest,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let path = editor.read(cx).path.clone();
    let changes = changes_of(ws, cx);
    let hunk = |lines: &Range<usize>, cx: &App| {
        changes.read(cx).set.file(&path).and_then(|f| f.hunks.iter().position(|h| h.new_lines == *lines))
    };
    match request {
        ClaudeRequest::AskToFix { lines, text } => ask_to_fix(ws, path.clone(), *lines, text, window, cx),
        ClaudeRequest::AcceptHunk(lines) => {
            if let Some(ix) = hunk(lines, cx) {
                accept_hunk(ws, &path, ix, window, cx);
            }
        }
        ClaudeRequest::RevertHunk(lines) => {
            if let Some(ix) = hunk(lines, cx) {
                revert_hunk(ws, &path, ix, window, cx);
            }
        }
        ClaudeRequest::AcceptFile => accept_file(ws, &path, window, cx),
        ClaudeRequest::RevertFile => revert_file(ws, &path, window, cx),
        ClaudeRequest::NextFile | ClaudeRequest::PrevFile => {
            let files: Vec<PathBuf> = changes.read(cx).set.files.iter().map(|f| f.path.clone()).collect();
            if files.is_empty() {
                return;
            }
            let at = files.iter().position(|p| *p == path);
            let forward = *request == ClaudeRequest::NextFile;
            let next = match (at, forward) {
                (Some(i), true) => (i + 1) % files.len(),
                (Some(i), false) => (i + files.len() - 1) % files.len(),
                (None, true) => 0,
                (None, false) => files.len() - 1,
            };
            let target = files[next].clone();
            let line = changes.read(cx).set.file(&target).and_then(|f| {
                let h = if forward { f.hunks.first() } else { f.hunks.last() };
                h.map(|h| h.new_lines.start)
            });
            open_change(ws, &target, line, window, cx);
        }
    }
}

/// A dot that pulses while Claude works.
pub fn pulse_dot(color: gpui::Hsla, id: &'static str) -> AnyElement {
    use gpui::{Animation, AnimationExt as _, pulsating_between};
    div()
        .flex_none()
        .size(px(8.))
        .rounded_full()
        .bg(color)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1_100))
                .repeat()
                .with_easing(pulsating_between(0.35, 1.))
                .with_max_fps(24.),
            |d, t| d.opacity(t),
        )
        .into_any_element()
}

/// The floating note of what Claude is doing: the file it edits and the tool, or the
/// change waiting for review.
pub fn indicator(changes: &ClaudeChanges, cx: &App) -> Option<(gpui::Stateful<gpui::Div>, PathBuf)> {
    let p = palette(cx);
    let c = p.claude();
    let (what, path, tool) = if let Some(d) = changes.pending.iter().find(|d| d.front) {
        ("Claude is waiting for your review", d.path.clone(), None)
    } else {
        let a = changes
            .activity
            .as_ref()
            .filter(|a| (!a.done && a.at.elapsed() < super::panel::ACTIVE) || (a.done && a.at.elapsed() < LINGER))?;
        ("Claude is editing", a.path.clone(), Some(a.tool.clone()))
    };
    let (name, _) = changes.display(&path);
    let el = div()
        .id("claude-indicator")
        .debug_selector(|| "claude-indicator".into())
        .flex_none()
        .h(px(30.))
        .pl(px(10.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(8.))
        .bg(c.indicator)
        .border_1()
        .border_color(crate::theme::mix(c.mark, 0.45, p.border))
        .shadow_md()
        .text_size(px(12.5))
        .text_color(p.fg)
        .font_family(crate::look::ui_font(cx))
        .cursor_pointer()
        .child(pulse_dot(c.mark, "claude-indicator-dot"))
        .child(div().text_color(p.fg_dim).whitespace_nowrap().child(what))
        .child(crate::icons::file_icon(&path, 13., &p))
        .child(div().max_w(px(220.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(name))
        .when_some(tool, |d, tool| d.child(crate::look::chip(tool, c.mark, &p)));
    Some((el, path))
}
