//! Claude Code's side played by a fake client: a WebSocket with the lock file's token
//! that calls every tool against the real window, accepts and rejects proposed changes
//! through the review's keys, and hears the selection and @-mentions. The lock file goes
//! to a scratch folder, never to `~/.claude`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use serde_json::{Value, json};
use tungstenite::Message;

use super::server::tests::connect;
use super::{IdeConfig, IdeEnv, IdeStatus};
use crate::editor::EditorPanel;
use crate::model::Model;
use crate::settings::Settings;
use crate::tests::{demo_repo, git, scratch_dir, with_repo};
use crate::workspace::Workspace;

const HELLO: &str = "package demo;\n\n/** Says hello. */\npublic class Hello {\n    public String greet(String name) {\n        return \"Hello, \" + name;\n    }\n}\n";
const HELLO_V2: &str = "package demo;\n\nimport java.util.Objects;\n\n/** Says hello. */\npublic class Hello {\n    public String greet(String name) {\n        return \"Hello, \" + Objects.requireNonNull(name) + \"!\";\n    }\n}\n";

/// The window over `settings`, with its Claude Code server's lock file in `lock_dir`.
fn open_with_ide<'a>(
    cx: &'a mut TestAppContext,
    settings: Settings,
    lock_dir: &Path,
) -> (Entity<Model>, Entity<Workspace>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    let lock_dir = lock_dir.to_owned();
    cx.update(|cx| {
        crate::init(&settings, cx);
        cx.set_global(IdeConfig { lock_dir });
    });
    let model = cx.update(|cx| cx.new(|cx| Model::new(settings, None, cx)));
    let mut ws = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
        ws = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    (model, ws.unwrap(), vcx)
}

/// The one lock file in `dir`.
fn lock_in(dir: &Path) -> Value {
    let entry = std::fs::read_dir(dir).unwrap().flatten().find(|e| e.path().extension().is_some_and(|x| x == "lock"));
    serde_json::from_str(&std::fs::read_to_string(entry.expect("a lock file").path()).unwrap()).unwrap()
}

/// Let the window run for `ms` of its clock, giving the real threads (sockets, file
/// events) real time too.
fn pump(cx: &mut VisualTestContext, ms: u64) {
    for _ in 0..ms.div_ceil(16) {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(16));
        std::thread::sleep(Duration::from_millis(1));
    }
    cx.run_until_parked();
}

/// Pump until `done` holds.
fn pump_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let start = Instant::now();
    while !done(cx) {
        assert!(start.elapsed() < Duration::from_secs(15), "timed out waiting for {what}");
        pump(cx, 16);
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Claude Code's side: a socket on its own thread, so the window keeps running while
/// a request waits for its answer.
struct Client {
    cmds: mpsc::Sender<String>,
    answers: mpsc::Receiver<Value>,
    got: HashMap<u64, Value>,
    notes: Arc<Mutex<Vec<Value>>>,
    next: u64,
}

impl Client {
    fn new(port: u16, token: &str) -> Self {
        let mut ws = connect(port, Some(token)).expect("the token lets the client in");
        ws.get_ref().set_read_timeout(Some(Duration::from_millis(10))).unwrap();
        let (cmds, cmd_rx) = mpsc::channel::<String>();
        let (ans_tx, answers) = mpsc::channel();
        let notes = Arc::new(Mutex::new(Vec::new()));
        let n = notes.clone();
        std::thread::spawn(move || {
            loop {
                loop {
                    match cmd_rx.try_recv() {
                        Ok(text) => ws.send(Message::text(text)).unwrap(),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => return,
                    }
                }
                match ws.read() {
                    Ok(Message::Text(t)) => {
                        let v: Value = serde_json::from_str(t.as_str()).unwrap();
                        if v.get("id").is_some() {
                            let _ = ans_tx.send(v);
                        } else {
                            n.lock().unwrap().push(v);
                        }
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e))
                        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                    Err(_) => return,
                }
            }
        });
        Self { cmds, answers, got: HashMap::new(), notes, next: 1 }
    }

    fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.cmds.send(msg.to_string()).unwrap();
        id
    }

    fn answered(&mut self, id: u64) -> Option<Value> {
        while let Ok(v) = self.answers.try_recv() {
            self.got.insert(v["id"].as_u64().unwrap(), v);
        }
        self.got.remove(&id)
    }

    fn wait(&mut self, cx: &mut VisualTestContext, id: u64) -> Value {
        let start = Instant::now();
        loop {
            if let Some(v) = self.answered(id) {
                return v;
            }
            assert!(start.elapsed() < Duration::from_secs(15), "no answer to request {id}");
            pump(cx, 16);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn call(&mut self, cx: &mut VisualTestContext, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        self.wait(cx, id)
    }

    fn start_tool(&mut self, name: &str, arguments: Value) -> u64 {
        self.send("tools/call", json!({ "name": name, "arguments": arguments }))
    }

    /// A tool's answer: its text items.
    fn tool(&mut self, cx: &mut VisualTestContext, name: &str, arguments: Value) -> Vec<String> {
        let id = self.start_tool(name, arguments);
        texts(&self.wait(cx, id))
    }

    /// A tool's answer read as JSON.
    fn tool_json(&mut self, cx: &mut VisualTestContext, name: &str, arguments: Value) -> Value {
        let texts = self.tool(cx, name, arguments);
        serde_json::from_str(&texts[0]).unwrap_or_else(|e| panic!("{name}: {e}: {texts:?}"))
    }

    fn notes(&self, method: &str) -> Vec<Value> {
        self.notes.lock().unwrap().iter().filter(|n| n["method"] == method).map(|n| n["params"].clone()).collect()
    }
}

fn texts(answer: &Value) -> Vec<String> {
    let content = answer["result"]["content"].as_array().unwrap_or_else(|| panic!("a result: {answer}"));
    content.iter().map(|c| c["text"].as_str().unwrap().to_owned()).collect()
}

fn editor_of(ws: &Entity<Workspace>, path: &Path, cx: &mut VisualTestContext) -> Option<Entity<EditorPanel>> {
    cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path))
}

fn text_of(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).text(cx))
}

fn review_count(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| editor.read(cx).review_count())
}

fn streaming(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| editor.read(cx).review.as_ref().is_some_and(|r| r.streaming()))
}

fn focus(editor: &Entity<EditorPanel>, cx: &mut VisualTestContext) {
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| s.focus(w, cx));
    });
}

/// A repository with a Java file, and the window over it with a client connected.
type Connected<'a> = (PathBuf, PathBuf, Entity<Model>, Entity<Workspace>, Client, &'a mut VisualTestContext);

fn connected<'a>(name: &str, cx: &'a mut TestAppContext) -> Connected<'a> {
    let repo = demo_repo(name);
    std::fs::create_dir_all(repo.join("src/demo")).unwrap();
    std::fs::write(repo.join("src/demo/Hello.java"), HELLO).unwrap();
    std::fs::write(repo.join("src/demo/Other.java"), "class Other {}\n").unwrap();
    git(&repo, &["add", "src/demo"]);
    git(&repo, &["commit", "-qm", "Add hello"]);
    let lock_dir = scratch_dir(&format!("{name}-ide"));
    let (model, ws, cx) = open_with_ide(cx, with_repo(&repo), &lock_dir);
    let lock = lock_in(&lock_dir);
    let port = cx.update(|_, cx| ws.read(cx).claude.as_ref().unwrap().server.port());
    let client = Client::new(port, lock["authToken"].as_str().unwrap());
    // The console runs something harmless in place of `claude`.
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some("/bin/cat".into())));
    pump_until(cx, "the connection", |cx| cx.update(|_, cx| cx.global::<IdeStatus>().clients == 1));
    (repo, lock_dir, model, ws, client, cx)
}

#[gpui::test]
fn claude_code_finds_the_window_and_calls_every_tool(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, mut c, cx) = connected("claude-tools", cx);
    let hello = repo.join("src/demo/Hello.java");

    // Discovery: the lock file, and the variables the console gives `claude`.
    let lock = lock_in(&lock_dir);
    let port = cx.update(|_, cx| ws.read(cx).claude.as_ref().unwrap().server.port());
    assert_eq!(lock["pid"], std::process::id());
    assert_eq!(lock["ideName"], "SnoutCode");
    assert_eq!(lock["transport"], "ws");
    assert_eq!(lock["workspaceFolders"], json!([repo.to_string_lossy()]));
    let token = lock["authToken"].as_str().unwrap().to_owned();
    assert_eq!(token.len(), 32);
    let env = cx.update(|_, cx| cx.global::<IdeEnv>().0.clone());
    assert!(env.contains(&("CLAUDE_CODE_SSE_PORT", port.to_string())));
    assert!(env.contains(&("ENABLE_IDE_INTEGRATION", "true".into())));
    assert!(connect(port, None).is_err(), "no token, no connection");
    assert!(connect(port, Some(&"0".repeat(32))).is_err(), "nor with another token");

    // MCP's handshake.
    let init = c.call(cx, "initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {} }));
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    let list = c.call(cx, "tools/list", json!({}));
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for name in ["openFile", "openDiff", "getDiagnostics", "getCurrentSelection", "closeAllDiffTabs"] {
        assert!(names.contains(&name), "{name} is listed");
    }

    // Workspace folders.
    let folders = c.tool_json(cx, "getWorkspaceFolders", json!({}));
    assert_eq!(folders["success"], true);
    assert_eq!(folders["rootPath"], repo.to_string_lossy().as_ref());
    assert_eq!(folders["folders"][0]["path"], repo.to_string_lossy().as_ref());

    // openFile selects what it is asked to; the keyboard stays where it was.
    let opened = c.tool(cx, "openFile", json!({ "filePath": hello, "startText": "class Hello" }));
    assert_eq!(opened, ["Opened file and selected text \"class Hello\""]);
    let editor = editor_of(&ws, &hello, cx).expect("a tab for the file");
    let lines = c.tool(cx, "openFile", json!({ "filePath": hello, "startLine": 5, "endLine": 6 }));
    assert_eq!(lines, ["Opened file and selected lines 5 to 6"]);
    let quiet =
        c.tool_json(cx, "openFile", json!({ "filePath": hello, "makeFrontmost": false, "startText": "class Hello" }));
    assert_eq!(quiet["languageId"], "java");
    assert_eq!(quiet["lineCount"], 9);
    let missing =
        c.call(cx, "tools/call", json!({ "name": "openFile", "arguments": { "filePath": repo.join("nope.txt") } }));
    assert_eq!(missing["error"]["code"], -32000);

    // The selection, asked for and sent as it settles.
    let sel = c.tool_json(cx, "getCurrentSelection", json!({}));
    assert_eq!(sel["success"], true);
    assert_eq!(sel["text"], "class Hello");
    assert_eq!(sel["filePath"], hello.to_string_lossy().as_ref());
    assert_eq!(sel["selection"]["start"], json!({ "line": 3, "character": 7 }));
    assert_eq!(sel["selection"]["isEmpty"], false);
    pump_until(cx, "selection_changed", |_| !c.notes("selection_changed").is_empty());
    assert_eq!(c.notes("selection_changed").last().unwrap()["text"], "class Hello");
    let latest = c.tool_json(cx, "getLatestSelection", json!({}));
    assert_eq!(latest["text"], "class Hello");

    // Open editors.
    let tabs = c.tool_json(cx, "getOpenEditors", json!({}));
    let tab = tabs["tabs"].as_array().unwrap().iter().find(|t| t["label"] == "Hello.java").unwrap().clone();
    assert_eq!(tab["isActive"], true);
    assert_eq!(tab["languageId"], "java");
    assert_eq!(tab["isDirty"], false);
    assert_eq!(tab["selection"]["start"], json!({ "line": 3, "character": 7 }));

    // Dirty documents, and saving them.
    let clean = c.tool_json(cx, "checkDocumentDirty", json!({ "filePath": hello }));
    assert_eq!(
        clean,
        json!({ "success": true, "filePath": hello.to_string_lossy(), "isDirty": false, "isUntitled": false })
    );
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| {
            s.set_selected_range(0..0, cx);
            s.insert("// note\n", w, cx);
        });
    });
    assert_eq!(c.tool_json(cx, "checkDocumentDirty", json!({ "filePath": hello }))["isDirty"], true);
    let not_open = c.tool_json(cx, "checkDocumentDirty", json!({ "filePath": repo.join("README.md") }));
    assert_eq!(not_open["success"], false);
    let saved = c.tool_json(cx, "saveDocument", json!({ "filePath": hello }));
    assert_eq!(saved["saved"], true);
    assert!(std::fs::read_to_string(&hello).unwrap().starts_with("// note\n"));
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| s.set_value(HELLO, w, cx));
        editor.update(cx, |e, cx| e.save(cx));
    });

    // Diagnostics from the language servers, Java's included.
    let uri = crate::claude_ide::tools::file_url(&hello);
    let diag = lsp_types::Diagnostic {
        range: lsp_types::Range::new(lsp_types::Position::new(5, 8), lsp_types::Position::new(5, 14)),
        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
        source: Some("Java".into()),
        message: "Type mismatch".into(),
        ..Default::default()
    };
    cx.update(|_, cx| {
        let hub = ws.read(cx).hub.clone();
        hub.update(cx, |h, _| h.insert_published(&repo, uri.clone(), vec![diag]));
    });
    let all = c.tool_json(cx, "getDiagnostics", json!({}));
    assert_eq!(all[0]["uri"], uri);
    assert_eq!(all[0]["diagnostics"][0]["severity"], "Error");
    assert_eq!(all[0]["diagnostics"][0]["message"], "Type mismatch");
    assert_eq!(all[0]["diagnostics"][0]["range"]["start"], json!({ "line": 5, "character": 8 }));
    assert_eq!(c.tool_json(cx, "getDiagnostics", json!({ "uri": uri })).as_array().unwrap().len(), 1);
    let other = crate::claude_ide::tools::file_url(&repo.join("README.md"));
    assert_eq!(c.tool_json(cx, "getDiagnostics", json!({ "uri": other })), json!([]));

    // openDiff waits for the user: the change streams into the tab, ⌘↩ accepts it.
    let tab1 = "✻ [Claude Code] Hello.java (1) ⧉";
    let diff = |tab: &str, file: &Path, text: &str| json!({ "old_file_path": file, "new_file_path": file, "new_file_contents": text, "tab_name": tab });
    let id1 = c.start_tool("openDiff", diff(tab1, &hello, HELLO_V2));
    pump_until(cx, "the review", |cx| review_count(&editor, cx) == 1);
    assert!(c.answered(id1).is_none(), "openDiff waits for the user");
    assert!(streaming(&editor, cx), "the change types itself in");
    pump_until(cx, "the stream", |cx| !streaming(&editor, cx));
    assert_eq!(text_of(&editor, cx), HELLO_V2);
    let marks = cx.update(|_, cx| editor.read(cx).review.as_ref().unwrap().marks.clone());
    assert_eq!(crate::claude_ide::review::counts(&marks), (3, 1), "{marks:?}");
    focus(&editor, cx);
    cx.simulate_keystrokes("cmd-enter");
    let answer = c.wait(cx, id1);
    assert_eq!(texts(&answer), ["FILE_SAVED", HELLO_V2]);
    assert_eq!(std::fs::read_to_string(&hello).unwrap(), HELLO_V2, "accepting writes the file");
    cx.update(|_, cx| assert!(editor.read(cx).review.is_none() && !editor.read(cx).dirty));
    assert_eq!(c.tool(cx, "close_tab", json!({ "tab_name": tab1 })), ["TAB_CLOSED"]);
    assert!(editor_of(&ws, &hello, cx).is_some(), "the file's own tab stays");

    // Two at once queue up; what the user types during the stream is kept; Esc rejects.
    let v3 = HELLO_V2.replace("Says hello.", "Says hello, politely.");
    let id2 = c.start_tool("openDiff", diff("tab 2", &hello, &HELLO_V2.replace("\"!\"", "\".\"")));
    let id3 = c.start_tool("openDiff", diff("tab 3", &hello, &v3));
    pump_until(cx, "both proposals", |cx| review_count(&editor, cx) == 2);
    focus(&editor, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(texts(&c.wait(cx, id2)), ["DIFF_REJECTED", "tab 2"]);
    pump_until(cx, "the next review", |cx| streaming(&editor, cx));
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| {
            s.set_selected_range(0..0, cx);
            s.insert("// typed\n", w, cx);
        });
    });
    pump_until(cx, "the stream", |cx| !streaming(&editor, cx));
    assert_eq!(text_of(&editor, cx), format!("// typed\n{v3}"), "the typing is kept, the change put in");
    focus(&editor, cx);
    cx.simulate_keystrokes("cmd-enter");
    assert_eq!(texts(&c.wait(cx, id3)), ["FILE_SAVED".to_owned(), format!("// typed\n{v3}")]);
    assert_eq!(std::fs::read_to_string(&hello).unwrap(), format!("// typed\n{v3}"), "the edited text is saved");
    let accepted = text_of(&editor, cx);

    // Claude Code closing a diff still waiting rejects it and restores the text.
    let id4 = c.start_tool("openDiff", diff("tab 4", &hello, "class Hello {}\n"));
    pump_until(cx, "the review", |cx| review_count(&editor, cx) == 1);
    assert_eq!(c.tool(cx, "close_tab", json!({ "tab_name": "tab 4" })), ["TAB_CLOSED"]);
    assert_eq!(texts(&c.wait(cx, id4)), ["DIFF_REJECTED", "tab 4"]);
    assert_eq!(text_of(&editor, cx), accepted);
    cx.update(|_, cx| assert!(!editor.read(cx).dirty, "back as it was"));

    // closeAllDiffTabs rejects every waiting change, in every tab; a new file goes.
    let main = repo.join("src/main.rs");
    let fresh = repo.join("src/demo/Fresh.java");
    let id5 = c.start_tool("openDiff", diff("tab 5", &hello, "class Hello { int x; }\n"));
    let id6 = c.start_tool("openDiff", diff("tab 6", &main, "fn main() {}\n"));
    let id7 = c.start_tool("openDiff", diff("tab 7", &fresh, "class Fresh {}\n"));
    pump_until(cx, "three reviews", |cx| {
        [&hello, &main, &fresh].iter().all(|p| editor_of(&ws, p, cx).is_some_and(|e| review_count(&e, cx) == 1))
    });
    assert_eq!(c.tool(cx, "closeAllDiffTabs", json!({})), ["CLOSED_3_DIFF_TABS"]);
    for id in [id5, id6, id7] {
        assert_eq!(texts(&c.wait(cx, id))[0], "DIFF_REJECTED");
    }
    assert_eq!(text_of(&editor, cx), accepted);
    pump(cx, 50);
    assert!(editor_of(&ws, &fresh, cx).is_none(), "a rejected new file leaves no tab");
    assert!(!fresh.exists());

    // An accepted new file is written.
    let id8 = c.start_tool("openDiff", diff("tab 8", &fresh, "class Fresh {}\n"));
    pump_until(cx, "the new file's review", |cx| editor_of(&ws, &fresh, cx).is_some_and(|e| review_count(&e, cx) == 1));
    let fresh_editor = editor_of(&ws, &fresh, cx).unwrap();
    cx.update(|w, cx| fresh_editor.update(cx, |e, cx| e.accept_review(w, cx)));
    assert_eq!(texts(&c.wait(cx, id8))[0], "FILE_SAVED");
    assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "class Fresh {}\n");

    // ⌥⌘K sends the selection's lines as an @-mention and brings the console forward.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        let text = state.read(cx).text().to_string();
        let a = text.find("public class").unwrap();
        let b = text.find("    public String").unwrap();
        state.update(cx, |s, cx| {
            s.set_selected_range(a..b + 4, cx);
            s.focus(w, cx);
        });
    });
    cx.simulate_keystrokes("cmd-alt-k");
    cx.update(|_, cx| assert!(model.read(cx).settings.console_open, "the console comes forward"));
    pump_until(cx, "at_mentioned", |_| !c.notes("at_mentioned").is_empty());
    let mention = c.notes("at_mentioned")[0].clone();
    assert_eq!(mention["filePath"], hello.to_string_lossy().as_ref());
    let line = |s: &str| accepted.lines().position(|l| l.contains(s)).unwrap();
    assert_eq!(mention["lineStart"], line("public class"));
    assert_eq!(mention["lineEnd"], line("public String"));

    // The server goes with the window; so does its lock file.
    let lock_path = cx.update(|_, cx| ws.read(cx).claude.as_ref().unwrap().server.lock_path().unwrap().to_owned());
    assert!(lock_path.exists());
    cx.update(|_, cx| ws.update(cx, |ws, _| ws.claude = None));
    assert!(!lock_path.exists());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

#[gpui::test]
fn the_editor_follows_the_files_claude_changes(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("claude-follow", cx);
    let hello = repo.join("src/demo/Hello.java");
    let other = repo.join("src/demo/Other.java");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    let editor = editor_of(&ws, &hello, cx).unwrap();
    // The watcher starts with the first look at the project.
    pump(cx, 300);
    std::thread::sleep(Duration::from_millis(300));

    // An open file Claude changes: the tab takes the text, scrolls there, lights it.
    std::fs::write(&hello, HELLO_V2).unwrap();
    pump_until(cx, "the reload", |cx| text_of(&editor, cx) == HELLO_V2);
    let lit = cx.update(|_, cx| editor.read(cx).flash.as_ref().map(|f| f.lines.clone())).expect("the change lights up");
    assert_eq!(lit.len(), 2, "{lit:?}");
    assert_eq!(lit.iter().map(|l| l.len()).sum::<usize>(), 3, "the new lines: {lit:?}");
    assert_eq!(lit[1], 7..8);
    let caret = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    assert_eq!(caret as usize, lit[0].start, "the view goes to the change");
    pump(cx, 2_500);
    cx.update(|_, cx| assert!(editor.read(cx).flash.is_none(), "and fades"));

    // A file no tab shows: it opens, as a preview, on the change.
    std::fs::write(&other, "class Other {\n    int count;\n}\n").unwrap();
    pump_until(cx, "the other file", |cx| editor_of(&ws, &other, cx).is_some());
    let opened = editor_of(&ws, &other, cx).unwrap();
    let lit = cx.update(|_, cx| opened.read(cx).flash.as_ref().map(|f| f.lines.clone()));
    let lit = lit.expect("the change lights up");
    assert_eq!((lit.len(), lit[0].clone()), (1, 0..3));
    cx.update(|_, cx| assert!(opened.read(cx).preview));

    // Following off: only a dot on the tab.
    cx.update(|_, cx| model.update(cx, |m, _| m.settings.follow_claude = false));
    std::fs::write(&hello, HELLO).unwrap();
    pump_until(cx, "the reload", |cx| text_of(&editor, cx) == HELLO);
    cx.update(|_, cx| {
        assert!(editor.read(cx).touched, "a dot marks the tab");
        assert!(editor.read(cx).flash.is_none());
    });
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}
