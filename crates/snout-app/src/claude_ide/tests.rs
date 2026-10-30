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
    let ws = ws.unwrap();
    // The lock file is written on a worker of the server's.
    vcx.update(|_, cx| assert!(ws.read(cx).claude.as_ref().is_some_and(|b| b.server.sync_lock())));
    (model, ws, vcx)
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
    pump_until(cx, "the variables", |cx| cx.update(|_, cx| cx.try_global::<IdeEnv>().is_some_and(|e| !e.0.is_empty())));
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
        editor.update(cx, |e, cx| e.save(cx)).detach();
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
    assert_eq!(std::fs::read_to_string(&hello).unwrap(), HELLO, "Claude Code writes the file, not the IDE");
    cx.update(|_, cx| assert!(editor.read(cx).review.is_none() && !editor.read(cx).dirty));
    // Claude Code writes it; the tab already shows it.
    std::fs::write(&hello, HELLO_V2).unwrap();
    pump(cx, 900);
    cx.update(|_, cx| assert!(!editor.read(cx).dirty && !editor.read(cx).conflict));
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
    // Claude Code writes what the answer says: the text as edited.
    std::fs::write(&hello, format!("// typed\n{v3}")).unwrap();
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
    assert!(!fresh.exists(), "Claude Code makes the file");
    std::fs::write(&fresh, "class Fresh {}\n").unwrap();
    pump(cx, 900);
    cx.update(|_, cx| assert!(!fresh_editor.read(cx).dirty));

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
    // It shows: the lines sent light up in the editor, and a note by the console says what went.
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    let (first, last) = (line("public class") + 1, line("public String") + 1);
    cx.update(|_, cx| {
        let note = console.read(cx).note().map(str::to_owned);
        assert_eq!(note, Some(format!("Sent Hello.java lines {first}\u{2013}{last} to Claude")));
        let flash = editor.read(cx).flash.as_ref().expect("the lines sent light up");
        assert_eq!(flash.lines, vec![first - 1..last]);
        assert!(flash.color.is_some(), "in Claude's colour");
    });

    // No selection: the whole file, without lines. Claude Code drops a mention whose lines are
    // null, so they are left out (its schema takes numbers or nothing).
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| {
            s.set_selected_range(0..0, cx);
            s.focus(w, cx);
        });
    });
    cx.simulate_keystrokes("cmd-alt-k");
    pump_until(cx, "the whole file's mention", |_| c.notes("at_mentioned").len() >= 2);
    let whole = c.notes("at_mentioned")[1].clone();
    assert_eq!(whole, json!({ "filePath": hello.to_string_lossy() }), "no null lines");
    cx.update(|_, cx| assert_eq!(console.read(cx).note(), Some("Sent Hello.java to Claude")));

    // From the right click's menu, which has the keyboard while it shows: the editor it
    // opened on.
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| {
            s.set_selected_range(0..4, cx);
            s.focus(w, cx);
        });
        let at = gpui::point(gpui::px(200.), gpui::px(200.));
        editor.update(cx, |e, cx| e.open_context_menu_at(at, true, None, w, cx));
    });
    pump(cx, 50);
    cx.update(|w, cx| w.dispatch_action(Box::new(crate::editor::SendToClaude), cx));
    pump_until(cx, "the menu's mention", |_| c.notes("at_mentioned").len() >= 3);
    assert_eq!(c.notes("at_mentioned")[2]["lineStart"], 0);

    // The server goes with the window; so does its lock file.
    let lock_path = cx.update(|_, cx| ws.read(cx).claude.as_ref().unwrap().server.lock_path().unwrap());
    assert!(lock_path.exists());
    cx.update(|_, cx| ws.update(cx, |ws, _| ws.claude = None));
    let gone = std::time::Instant::now() + Duration::from_secs(5);
    while lock_path.exists() && std::time::Instant::now() < gone {
        std::thread::sleep(Duration::from_millis(10));
    }
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

    // An open file Claude changes: the tab plays the change out with Claude's caret, only
    // the lines that changed going in (never the whole text afresh), and they fade.
    let rev = cx.update(|_, cx| editor.read(cx).state.read(cx).text().to_string());
    std::fs::write(&hello, HELLO_V2).unwrap();
    pump_until(cx, "the change played", |cx| cx.update(|_, cx| editor.read(cx).live.is_some()));
    pump_until(cx, "the reload", |cx| {
        text_of(&editor, cx) == HELLO_V2 && cx.update(|_, cx| editor.read(cx).live.is_none())
    });
    assert_eq!(rev, HELLO);
    let lit: Vec<usize> = cx.update(|_, cx| editor.read(cx).fades.iter().map(|f| f.0).collect());
    let mut sorted = lit.clone();
    sorted.sort();
    assert_eq!(sorted, [2, 3, 7], "the new lines fade, the rest stays as it was");
    let caret = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    assert!((2..=7).contains(&(caret as usize)), "the view follows the typing: line {caret}");
    pump(cx, 1_200);
    std::thread::sleep(Duration::from_millis(1_800));
    pump(cx, 200);
    cx.update(|_, cx| assert!(editor.read(cx).fades.is_empty(), "and fades"));

    // A file no tab shows: it opens, as a preview, on the change.
    std::fs::write(&other, "class Other {\n    int count;\n}\n").unwrap();
    pump_until(cx, "the other file", |cx| editor_of(&ws, &other, cx).is_some());
    let opened = editor_of(&ws, &other, cx).unwrap();
    pump_until(cx, "the other file typed in", |cx| {
        text_of(&opened, cx) == "class Other {\n    int count;\n}\n"
            && cx.update(|_, cx| opened.read(cx).live.is_none())
    });
    let mut lit: Vec<usize> = cx.update(|_, cx| opened.read(cx).fades.iter().map(|f| f.0).collect());
    lit.sort();
    assert_eq!(lit, [0, 1, 2]);
    cx.update(|_, cx| assert!(opened.read(cx).preview));

    // Following off: only a dot on the tab.
    cx.update(|_, cx| model.update(cx, |m, _| m.settings.follow_claude = false));
    std::fs::write(&hello, HELLO).unwrap();
    pump_until(cx, "the reload", |cx| text_of(&editor, cx) == HELLO);
    cx.update(|_, cx| {
        assert!(editor.read(cx).touched, "a dot marks the tab");
        assert!(editor.read(cx).agent_dot(cx).is_some(), "in Claude's colour");
    });
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// The hooks' endpoint of the window and the command `claude`'s settings run for a hook.
fn hook_command(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (u16, String) {
    pump_until(cx, "the hooks' settings", |cx| {
        cx.update(|_, cx| cx.try_global::<super::IdeArgs>().is_some_and(|a| !a.0.is_empty()))
    });
    let (port, args, env) = cx.update(|_, cx| {
        let port = ws.read(cx).claude.as_ref().unwrap().hooks.as_ref().unwrap().port();
        (port, cx.global::<super::IdeArgs>().0.clone(), cx.global::<IdeEnv>().0.clone())
    });
    assert_eq!(args[0], "--settings", "the console's `claude` gets the hooks as extra settings");
    assert!(env.contains(&(super::hooks::PORT_ENV, port.to_string())));
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let cmd = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_owned();
    (port, cmd)
}

/// Run a hook as Claude Code does: its command through the shell, its input on stdin.
fn run_hook(port: u16, cmd: &str, input: Value) {
    use std::io::Write as _;
    let mut child = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .env(super::hooks::PORT_ENV, port.to_string())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
}

fn tool_hook(event: &str, tool: &str, file: &Path) -> Value {
    json!({ "hook_event_name": event, "session_id": "fake-session", "cwd": file.parent().unwrap(), "tool_name": tool,
        "tool_input": { "file_path": file } })
}

fn changes(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Entity<super::panel::ClaudeChanges> {
    cx.update(|_, cx| super::bridge::changes_of(ws.read(cx), cx))
}

fn listed(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<(PathBuf, usize, usize, bool)> {
    let c = changes(ws, cx);
    cx.update(|_, cx| c.read(cx).set.files.iter().map(|f| (f.path.clone(), f.added, f.removed, f.created())).collect())
}

fn settled(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> bool {
    let c = changes(ws, cx);
    cx.update(|_, cx| !c.read(cx).settling())
}

#[gpui::test]
fn claude_edits_reported_by_its_hooks_play_out_and_wait_for_review(cx: &mut TestAppContext) {
    let (repo, lock_dir, _model, ws, mut client, cx) = connected("claude-hooks", cx);
    let (port, cmd) = hook_command(&ws, cx);
    let hello = repo.join("src/demo/Hello.java");
    let fresh = repo.join("src/demo/Fresh.java");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    let editor = editor_of(&ws, &hello, cx).unwrap();

    // An edit Claude accepts on its own (no openDiff): the hook before it, then the write.
    run_hook(port, &cmd, json!({ "hook_event_name": "UserPromptSubmit", "session_id": "fake-session", "prompt": "x" }));
    run_hook(port, &cmd, tool_hook("PreToolUse", "Edit", &hello));
    pump_until(cx, "the editing indicator", |cx| {
        let c = changes(&ws, cx);
        cx.update(|_, cx| c.read(cx).busy().is_some_and(|a| a.path == hello && a.tool == "Edit"))
    });
    cx.update(|_, cx| {
        assert!(editor.read(cx).claude_hold.is_some(), "the tab waits for the report");
        let c = super::bridge::changes_of(ws.read(cx), cx);
        assert!(super::bridge::indicator(c.read(cx), cx).is_some(), "Claude is editing Hello.java");
    });
    std::fs::write(&hello, HELLO_V2).unwrap();
    // The disk check does not take the file meanwhile: the report shows it.
    pump(cx, 900);
    assert_eq!(text_of(&editor, cx), HELLO);
    run_hook(port, &cmd, tool_hook("PostToolUse", "Edit", &hello));
    pump_until(cx, "the live edit", |cx| cx.update(|_, cx| editor.read(cx).live.is_some()));
    cx.update(|_, cx| {
        let e = editor.read(cx);
        let live = e.live.as_ref().unwrap();
        assert_eq!(live.writer.name.as_ref(), "Claude", "Claude's caret, with its name on the flag");
        assert!(e.presence.as_ref().is_some_and(|p| p.left.is_none()), "the caret writes");
        assert!(!e.dirty, "a live edit is not an unsaved change");
        assert!(e.agent_dot(cx).is_some_and(|(_, live)| live), "the tab's dot pulses");
    });
    // The text types itself in: the caret moves on with it, only the lines it writes change.
    let mut seen = Vec::new();
    pump_until(cx, "the end of the edit", |cx| {
        cx.update(|_, cx| {
            let e = editor.read(cx);
            if let Some(p) = &e.presence {
                seen.push(p.at);
            }
            e.live.is_none()
        })
    });
    seen.dedup();
    assert!(seen.len() >= 3, "the caret moved as the text came in: {seen:?}");
    assert_eq!(text_of(&editor, cx), HELLO_V2);
    cx.update(|_, cx| {
        let e = editor.read(cx);
        assert!(!e.dirty, "the tab as the file");
        assert!(!e.fades.is_empty(), "the new lines fade from Claude's colour");
        assert!(e.fades.iter().all(|(l, _, _)| *l < 9), "only the lines written: {:?}", e.fades);
        assert!(e.presence.as_ref().is_some_and(|p| p.left.is_some()), "the caret lingers where it stopped");
    });
    pump(cx, 2_500);
    std::thread::sleep(Duration::from_millis(1_700));
    pump(cx, 200);
    cx.update(|_, cx| {
        let e = editor.read(cx);
        assert!(e.fades.is_empty() && e.presence.is_none(), "faded and gone in a second or two");
    });
    pump_until(cx, "the change listed", |cx| settled(&ws, cx) && !listed(&ws, cx).is_empty());
    assert_eq!(listed(&ws, cx), [(hello.clone(), 3, 1, false)]);
    pump_until(cx, "the gutter's Claude marks", |cx| cx.update(|_, cx| editor.read(cx).claude_hunks.len() == 2));
    cx.update(|_, cx| assert!(editor.read(cx).claude_base.as_deref().is_some_and(|b| b == HELLO)));

    // A file Claude creates: it opens and types itself in.
    run_hook(port, &cmd, tool_hook("PreToolUse", "Write", &fresh));
    std::fs::write(&fresh, "class Fresh {\n    int n;\n}\n").unwrap();
    run_hook(port, &cmd, tool_hook("PostToolUse", "Write", &fresh));
    pump_until(cx, "the new file's tab", |cx| editor_of(&ws, &fresh, cx).is_some());
    let fresh_editor = editor_of(&ws, &fresh, cx).unwrap();
    pump_until(cx, "the new file typed in", |cx| cx.update(|_, cx| fresh_editor.read(cx).live.is_none()));
    assert_eq!(text_of(&fresh_editor, cx), "class Fresh {\n    int n;\n}\n");
    pump_until(cx, "two files listed", |cx| settled(&ws, cx) && listed(&ws, cx).len() == 2);
    assert_eq!(listed(&ws, cx)[1], (fresh.clone(), 3, 0, true));

    // Review: revert one hunk of Hello (the import), keep the other; the file leaves the list.
    cx.update(|w, cx| ws.update(cx, |ws, cx| super::bridge::revert_hunk(ws, &hello, 0, w, cx)));
    let reverted = HELLO_V2.replace("import java.util.Objects;\n\n", "");
    // Written on the host off the UI thread.
    pump_until(cx, "the hunk put back on disk", |_| std::fs::read_to_string(&hello).unwrap() == reverted);
    pump_until(cx, "the tab takes it", |cx| text_of(&editor, cx) == reverted);
    pump_until(cx, "one hunk left", |cx| settled(&ws, cx) && listed(&ws, cx)[0].1 == 1);
    cx.update(|_, cx| assert!(!editor.read(cx).dirty));
    // The editor's own key: keep the change at the caret.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    cx.update(|w, cx| {
        let line = reverted.lines().position(|l| l.contains("requireNonNull")).unwrap();
        editor.update(cx, |e, cx| e.go_to_claude_line(line, w, cx));
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| s.focus(w, cx));
    });
    pump(cx, 200);
    cx.simulate_keystrokes("cmd-alt-y");
    pump_until(cx, "Hello reviewed", |cx| settled(&ws, cx) && listed(&ws, cx).len() == 1);
    pump(cx, 200);
    cx.update(|_, cx| assert!(editor.read(cx).claude_base.is_none(), "no marks left in the gutter"));
    assert_eq!(std::fs::read_to_string(&hello).unwrap(), reverted, "keeping writes nothing");

    // Revert All, by its keys: the file Claude made goes, its tab too.
    cx.simulate_keystrokes("cmd-alt-shift-z");
    pump_until(cx, "the created file gone", |_| !fresh.exists());
    assert!(!fresh.exists(), "a created file reverts to nothing");
    assert!(editor_of(&ws, &fresh, cx).is_none());
    assert!(listed(&ws, cx).is_empty());

    // While the user types, the editor stays put: the change is listed, nothing moves.
    let other = repo.join("src/demo/Other.java");
    focus(&editor, cx);
    cx.simulate_input("x");
    run_hook(port, &cmd, tool_hook("PreToolUse", "Edit", &other));
    std::fs::write(&other, "class Other {\n    int count;\n}\n").unwrap();
    run_hook(port, &cmd, tool_hook("PostToolUse", "Edit", &other));
    pump_until(cx, "Other listed", |cx| settled(&ws, cx) && listed(&ws, cx).len() == 1);
    assert!(editor_of(&ws, &other, cx).is_none(), "no tab opened under the user's typing");
    cx.update(|w, cx| {
        let active = ws.read(cx).active_editor(w, cx).unwrap();
        assert_eq!(active.read(cx).path, hello, "the tab in front stays");
    });

    // A change Claude asks to make (openDiff) waits in the window too, and is answered there.
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| s.set_value(reverted.clone(), w, cx));
        editor.update(cx, |e, cx| e.save(cx)).detach();
    });
    run_hook(port, &cmd, tool_hook("PreToolUse", "Edit", &hello));
    let diff =
        json!({ "old_file_path": hello, "new_file_path": hello, "new_file_contents": HELLO, "tab_name": "tab h" });
    let id = client.start_tool("openDiff", diff);
    pump_until(cx, "the request in the window", |cx| {
        let c = changes(&ws, cx);
        cx.update(|_, cx| c.read(cx).pending.first().is_some_and(|d| d.front && d.path == hello))
    });
    pump_until(cx, "the stream", |cx| !streaming(&editor, cx));
    cx.update(|w, cx| ws.update(cx, |ws, cx| super::bridge::answer_review(ws, "tab h", true, w, cx)));
    assert_eq!(texts(&client.wait(cx, id))[0], "FILE_SAVED");
    // Claude Code writes what was accepted; the hook after it lists the change, played out
    // already (no second animation).
    std::fs::write(&hello, HELLO).unwrap();
    run_hook(port, &cmd, tool_hook("PostToolUse", "Edit", &hello));
    pump_until(cx, "Hello listed again", |cx| settled(&ws, cx) && listed(&ws, cx).iter().any(|f| f.0 == hello));
    cx.update(|_, cx| {
        assert!(editor.read(cx).live.is_none(), "no replay of a change already reviewed in the tab");
        let c = super::bridge::changes_of(ws.read(cx), cx);
        assert!(c.read(cx).pending.is_empty());
    });
    run_hook(port, &cmd, json!({ "hook_event_name": "Stop", "session_id": "fake-session" }));
    // The indicator lingers a moment (on the wall clock) after the last edit.
    std::thread::sleep(Duration::from_millis(1_600));
    pump(cx, 200);
    let c = changes(&ws, cx);
    cx.update(|_, cx| assert!(c.read(cx).activity.is_none(), "the indicator goes once Claude stops"));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// Pump until `done`, for as long as `secs`; the console's text on the way out if not.
fn wait_long(
    cx: &mut VisualTestContext,
    what: &str,
    secs: u64,
    console: &Entity<crate::console::ConsolePanel>,
    root: &Path,
    mut done: impl FnMut(&mut VisualTestContext) -> bool,
) {
    let start = Instant::now();
    while !done(cx) {
        if start.elapsed() > Duration::from_secs(secs) {
            let text = cx.update(|_, cx| console.read(cx).session_text(root)).unwrap_or_default();
            panic!("timed out waiting for {what}; the console shows:\n{text}");
        }
        pump(cx, 50);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The real `claude` in the console, against the window: a change it asks to make (default
/// mode, through openDiff, accepted in the Claude Changes window), then edits it makes on
/// its own (`--permission-mode acceptEdits`), all reported by the hooks; then Revert All.
/// Needs the user's Claude Code login, so it runs only when asked:
/// `SNOUT_E2E_CLAUDE=<path to claude> SNOUT_E2E_LOCK_DIR=<lock folder> cargo test real_claude -- --ignored`.
#[gpui::test]
#[ignore]
fn real_claude_edits_live_and_reverts(cx: &mut TestAppContext) {
    let Ok(claude) = std::env::var("SNOUT_E2E_CLAUDE") else { return };
    let lock_dir = PathBuf::from(std::env::var("SNOUT_E2E_LOCK_DIR").expect("SNOUT_E2E_LOCK_DIR"));
    let repo = demo_repo("claude-e2e");
    std::fs::create_dir_all(repo.join("src/demo")).unwrap();
    let greeter = repo.join("src/demo/Greeter.java");
    let v1 = "package demo;\n\npublic class Greeter {\n    private final String name;\n\n    public Greeter(String name) {\n        this.name = name;\n    }\n\n    public String greet() {\n        return \"Hello, \" + name;\n    }\n}\n";
    std::fs::write(&greeter, v1).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "Greeter"]);
    let (model, ws, cx) = open_with_ide(cx, with_repo(&repo), &lock_dir);
    let (_, _) = hook_command(&ws, cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(greeter.clone(), None, false, w, cx)));
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(PathBuf::from(&claude))));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx)));
    let root = repo.clone();
    let session =
        |cx: &mut VisualTestContext| cx.update(|_, cx| console.read(cx).session_text(&root)).unwrap_or_default();
    // Claude Code asks whether to trust a new folder: yes, for this scratch one.
    let ready = |cx: &mut VisualTestContext, console: &Entity<crate::console::ConsolePanel>, root: &Path| {
        let start = Instant::now();
        loop {
            let text = cx.update(|_, cx| console.read(cx).session_text(root)).unwrap_or_default();
            if text.contains("trust this folder") {
                // The choice starts on "No, exit": down to "Yes", then Enter, once the
                // dialog listens.
                std::thread::sleep(Duration::from_millis(3_000));
                pump(cx, 100);
                cx.update(|_, cx| console.read(cx).send_keys(root, b"\x1b[B"));
                for _ in 0..10 {
                    pump(cx, 100);
                    std::thread::sleep(Duration::from_millis(100));
                }
                cx.update(|_, cx| console.read(cx).send_keys(root, b"\r"));
                pump(cx, 500);
                std::thread::sleep(Duration::from_millis(500));
            }
            if cx.update(|_, cx| cx.global::<IdeStatus>().clients >= 1)
                && !text.contains("trust this folder")
                && text.contains('❯')
            {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(60), "claude did not start:\n{text}");
            pump(cx, 50);
            std::thread::sleep(Duration::from_millis(20));
        }
        pump(cx, 500);
        std::thread::sleep(Duration::from_millis(1_500));
    };
    let prompt = |cx: &mut VisualTestContext, text: &str| {
        cx.update(|_, cx| console.read(cx).type_text(&root, text));
        pump(cx, 300);
        std::thread::sleep(Duration::from_millis(300));
        cx.update(|_, cx| console.read(cx).send_keys(&root, b"\r"));
    };
    let editor = editor_of(&ws, &greeter, cx).unwrap();

    // 1. Manual mode (asks for every edit): Claude asks through openDiff; the window lists
    // the request; it is accepted there.
    pump(cx, 300);
    cx.update(|w, cx| {
        console.update(cx, |c, cx| c.start(&root, &["--permission-mode".into(), "manual".into()], w, cx))
    });
    pump(cx, 500);
    std::thread::sleep(Duration::from_millis(1_000));
    ready(cx, &console, &root);
    prompt(
        cx,
        "In src/demo/Greeter.java add a private int count field after the name field. Change nothing else and run no commands.",
    );
    let c = changes(&ws, cx);
    wait_long(cx, "the openDiff request", 240, &console, &root, |cx| {
        cx.update(|_, cx| c.read(cx).pending.iter().any(|d| d.front && d.path == greeter))
    });
    pump_until(cx, "the stream", |cx| !streaming(&editor, cx));
    let tab = cx.update(|_, cx| c.read(cx).pending[0].tab.clone());
    cx.update(|w, cx| ws.update(cx, |ws, cx| super::bridge::answer_review(ws, &tab, true, w, cx)));
    wait_long(cx, "the field listed", 240, &console, &root, |cx| {
        settled(&ws, cx) && listed(&ws, cx).iter().any(|f| f.0 == greeter)
    });
    let text = std::fs::read_to_string(&greeter).unwrap();
    assert!(text.contains("int count"), "{text}");
    println!("e2e 1 (default mode, openDiff): listed {:?}", listed(&ws, cx));
    cx.update(|_, cx| assert!(c.read(cx).hooks_heard, "the hooks reported the edit"));
    wait_long(cx, "Claude to finish", 240, &console, &root, |cx| cx.update(|_, cx| c.read(cx).busy().is_none()));

    // 2. acceptEdits: Claude edits on its own; the hooks say so; the editor plays it out.
    cx.update(|w, cx| {
        console.update(cx, |c, cx| c.start(&root, &["--permission-mode".into(), "acceptEdits".into()], w, cx))
    });
    pump(cx, 500);
    std::thread::sleep(Duration::from_millis(3_000));
    ready(cx, &console, &root);
    prompt(
        cx,
        "Rename the method greet to sayHello in src/demo/Greeter.java, and create src/demo/Farewell.java with a class Farewell that has a method bye(String name) returning \"Goodbye, \" + name. Run no commands.",
    );
    let farewell = repo.join("src/demo/Farewell.java");
    let mut saw_live = false;
    wait_long(cx, "both edits listed", 300, &console, &root, |cx| {
        saw_live |= cx.update(|_, cx| ws.read(cx).editors(cx).iter().any(|e| e.read(cx).live.is_some()));
        settled(&ws, cx) && {
            let l = listed(&ws, cx);
            l.iter().any(|f| f.0 == farewell && f.3)
                && std::fs::read_to_string(&greeter).unwrap().contains("sayHello")
                && l.iter().any(|f| f.0 == greeter)
        }
    });
    println!("e2e 2 (acceptEdits): listed {:?}, live seen: {saw_live}", listed(&ws, cx));
    assert!(saw_live, "an edit played out live in the editor");
    wait_long(cx, "the greeter's tab to show it", 30, &console, &root, |cx| text_of(&editor, cx).contains("sayHello"));
    wait_long(cx, "Claude to finish", 240, &console, &root, |cx| cx.update(|_, cx| c.read(cx).busy().is_none()));

    // Revert All: the session's changes undone, the new file gone.
    cx.update(|w, cx| ws.update(cx, |ws, cx| super::bridge::revert_all(ws, w, cx)));
    pump(cx, 300);
    assert_eq!(std::fs::read_to_string(&greeter).unwrap(), v1, "back to before Claude's first edit");
    assert!(!farewell.exists());
    println!("e2e: reverted; the console showed:\n{}", session(cx));
    cx.update(|_, cx| console.update(cx, |c, _| c.program = None));
    cx.update(|_, cx| ws.update(cx, |ws, _| ws.claude = None));
    std::thread::sleep(Duration::from_millis(500));
    let _ = std::fs::remove_dir_all(repo);
}

/// Codex in the console (a stand-in that echoes, in place of `codex`): the console starts
/// it with its `notify` set for the session, a Return begins its turn, the files that
/// change while it works are put down to it, and its notify names the turn's prompt.
#[gpui::test]
fn codex_in_the_console_gets_its_turns_and_its_edits(cx: &mut TestAppContext) {
    let repo = demo_repo("codex-console");
    std::fs::create_dir_all(repo.join("src/demo")).unwrap();
    let other = repo.join("src/demo/Other.java");
    std::fs::write(&other, "class Other {}\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "Other"]);
    let lock_dir = scratch_dir("codex-console-ide");
    let mut settings = with_repo(&repo);
    settings.console_agent = crate::agents::ConsoleAgent::Codex;
    let (model, ws, cx) = open_with_ide(cx, settings, &lock_dir);
    pump_until(cx, "Codex's notify", |cx| {
        cx.update(|_, cx| cx.try_global::<super::CodexArgs>().is_some_and(|a| a.0.len() == 2))
    });
    let notify = cx.update(|_, cx| cx.global::<super::CodexArgs>().0.clone());
    assert_eq!(notify[0], "-c");
    assert!(notify[1].starts_with("notify=[\"/bin/sh\",\"-c\","), "{}", notify[1]);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some("/bin/cat".into())));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx)));
    pump_until(cx, "the session", |cx| cx.update(|_, cx| console.read(cx).session_text(&repo).is_some()));
    cx.update(|_, cx| {
        let agent = console.read(cx).session_agent(&repo, cx);
        assert_eq!((agent.id, agent.name.as_ref()), (crate::agents::CODEX, "Codex"));
    });
    pump_until(cx, "the console's agent", |cx| {
        cx.update(|_, cx| crate::agents::console_agent(cx).id == crate::agents::CODEX)
    });
    // The watcher starts with the first look at the project.
    pump(cx, 300);
    std::thread::sleep(Duration::from_millis(300));

    // Return: Codex's turn begins.
    cx.update(|w, cx| {
        let focus = gpui::Focusable::focus_handle(console.read(cx), cx);
        w.focus(&focus, cx);
    });
    cx.simulate_keystrokes("x enter");
    pump_until(cx, "the turn", |cx| {
        cx.update(|_, cx| cx.try_global::<crate::agents::ConsoleTurn>().is_some_and(|t| t.active))
    });
    let c = changes(&ws, cx);
    pump_until(cx, "a turn of Codex's", |cx| {
        cx.update(|_, cx| c.read(cx).set.current().is_some_and(|t| t.agent == crate::agents::CODEX))
    });
    std::fs::write(&other, "class Other {\n    int count;\n}\n").unwrap();
    pump_until(cx, "the change put down to Codex", |cx| {
        settled(&ws, cx) && cx.update(|_, cx| c.read(cx).set.file(&other).is_some_and(|f| f.agent == "codex"))
    });
    // Codex's notify at the end of the turn: the prompt.
    let port = cx.update(|_, cx| ws.read(cx).claude.as_ref().unwrap().hooks.as_ref().unwrap().port());
    let notify: Vec<String> = serde_json::from_str(notify[1].strip_prefix("notify=").unwrap()).unwrap();
    let payload = json!({ "type": "agent-turn-complete", "turn-id": "1", "input-messages": ["Count the others"] });
    let out = std::process::Command::new(&notify[0])
        .args(&notify[1..])
        .arg(payload.to_string())
        .env(super::hooks::PORT_ENV, port.to_string())
        .output()
        .unwrap();
    assert!(out.status.success());
    pump_until(cx, "the prompt", |cx| {
        cx.update(|_, cx| c.read(cx).set.current().is_some_and(|t| t.prompt.as_deref() == Some("Count the others")))
    });
    cx.update(|_, cx| console.update(cx, |c, _| c.program = None));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// Claude writes far down a file while the editor does not follow: the view stays where
/// the user is, and a chip at the editor's edge says where Claude is.
#[gpui::test]
fn an_edit_out_of_sight_shows_a_chip_at_the_edge(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("claude-chip", cx);
    let (port, cmd) = hook_command(&ws, cx);
    cx.update(|_, cx| model.update(cx, |m, _| m.settings.follow_claude = false));
    let long = repo.join("src/demo/Long.java");
    let body: String = (0..300).map(|i| format!("    int f{i} = {i};\n")).collect();
    let v1 = format!("class Long {{\n{body}}}\n");
    std::fs::write(&long, &v1).unwrap();
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(long.clone(), None, false, w, cx)));
    let editor = editor_of(&ws, &long, cx).unwrap();
    pump_until(cx, "the file", |cx| text_of(&editor, cx) == v1);
    pump(cx, 300);
    run_hook(port, &cmd, tool_hook("PreToolUse", "Edit", &long));
    let v2 = v1.replace("    int f250 = 250;\n", "    int f250 = 250;\n    int added = 1;\n");
    std::fs::write(&long, &v2).unwrap();
    run_hook(port, &cmd, tool_hook("PostToolUse", "Edit", &long));
    pump_until(cx, "the chip", |cx| cx.update(|_, cx| editor.update(cx, |e, cx| e.presence_chip(cx).is_some())));
    let line = cx.update(|_, cx| editor.read(cx).presence_line(cx)).unwrap();
    assert!(line >= 250, "Claude writes at line {line}");
    let caret = cx.update(|_, cx| editor.read(cx).state.read(cx).cursor_position().line);
    assert!(caret < 5, "the user's caret stays where it was: {caret}");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// What an agent wrote stays put down to it: the ledger, the gutter's lane, the tint until
/// the change is reviewed, the dots on the tab and the file, the status bar's note, the
/// turn and its prompt in the Agent Changes window; accepting keeps the record.
#[gpui::test]
fn the_lines_an_agent_wrote_keep_its_name(cx: &mut TestAppContext) {
    let (repo, lock_dir, _model, ws, _client, cx) = connected("claude-attribution", cx);
    let (port, cmd) = hook_command(&ws, cx);
    let hello = repo.join("src/demo/Hello.java");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    let editor = editor_of(&ws, &hello, cx).unwrap();
    pump_until(cx, "the file", |cx| text_of(&editor, cx) == HELLO);
    run_hook(
        port,
        &cmd,
        json!({ "hook_event_name": "UserPromptSubmit", "prompt": "Check the name\nand greet louder" }),
    );
    run_hook(port, &cmd, tool_hook("PreToolUse", "Edit", &hello));
    std::fs::write(&hello, HELLO_V2).unwrap();
    run_hook(port, &cmd, tool_hook("PostToolUse", "Edit", &hello));
    pump_until(cx, "the edit played and listed", |cx| {
        cx.update(|_, cx| editor.read(cx).live.is_none()) && settled(&ws, cx) && listed(&ws, cx).len() == 1
    });
    pump(cx, 300);
    let c = changes(&ws, cx);
    cx.update(|_, cx| {
        let c = c.read(cx);
        let owners = c.ledger.owners(&hello).expect("the ledger has the file");
        let runs = owners.runs();
        assert_eq!(runs.iter().map(|r| r.0.clone()).collect::<Vec<_>>(), [2..4, 7..8], "{runs:?}");
        let edit = c.ledger.edit(runs[0].1).unwrap();
        assert_eq!((edit.agent.as_str(), edit.prompt.as_deref()), ("claude", Some("Check the name")));
        assert_eq!(c.set.current().unwrap().prompt.as_deref(), Some("Check the name"));
        // The dots and the note.
        assert_eq!(cx.global::<super::AgentFiles>().0.get(&hello), Some(&"claude"));
        assert_eq!(cx.global::<super::AgentSummary>(), &super::AgentSummary { files: 1, agent: Some("claude") });
    });
    cx.update(|_, cx| {
        let e = editor.read(cx);
        let lane = e.agent_lane(cx);
        assert!(lane.iter().any(|r| r.lines == (2..4) && r.unreviewed && r.edit.is_some()), "{lane:?}");
        assert!(!e.agent_tint(cx).is_empty(), "tinted until reviewed");
        assert!(e.agent_dot(cx).is_some_and(|(_, live)| !live), "the tab's dot");
    });
    // The user types above: Claude's lines move down with the text, and stay Claude's.
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| {
            s.set_selected_range(0..0, cx);
            s.replace("// top\n", w, cx);
        });
    });
    pump_until(cx, "the lines moved", |cx| {
        cx.update(|_, cx| {
            editor.read(cx).agent_lines.as_ref().is_some_and(|o| o.runs().first().is_some_and(|r| r.0 == (3..5)))
        })
    });
    pump(cx, 300);
    cx.update(|_, cx| {
        let runs = c.read(cx).ledger.owners(&hello).unwrap().runs();
        assert_eq!(runs[0].0, 3..5, "the ledger took the editor's lines: {runs:?}");
    });
    cx.update(|w, cx| {
        let state = editor.read(cx).state.clone();
        state.update(cx, |s, cx| {
            s.set_selected_range(0..7, cx);
            s.replace("", w, cx);
        });
        editor.update(cx, |e, cx| e.save(cx)).detach();
    });
    pump(cx, 500);
    // Accept the file: the tint goes, the record stays (Annotate with Agents still says so).
    cx.update(|w, cx| ws.update(cx, |ws, cx| super::bridge::accept_file(ws, &hello, w, cx)));
    pump_until(cx, "reviewed", |cx| cx.update(|_, cx| editor.read(cx).claude_base.is_none()));
    pump(cx, 300);
    cx.update(|_, cx| {
        let e = editor.read(cx);
        assert!(e.agent_tint(cx).is_empty(), "no tint once reviewed");
        let lane = e.agent_lane(cx);
        assert!(!lane.is_empty() && lane.iter().all(|r| !r.unreviewed), "the marks stay, paler: {lane:?}");
        assert!(cx.global::<super::AgentFiles>().0.is_empty() && cx.global::<super::AgentSummary>().files == 0);
    });
    // Annotate with Agents opens the annotation column: Claude's uncommitted lines say Claude.
    cx.update(|_, cx| editor.update(cx, |e, cx| e.toggle_annotate(cx)));
    pump_until(cx, "the blame", |cx| cx.update(|_, cx| editor.read(cx).blame.data.is_some()));
    pump(cx, 300);
    let notes = cx.update(|_, cx| editor.read(cx).annotation_labels(cx));
    assert!(notes.iter().any(|(_, l)| l.contains("Claude \u{b7} ")), "{notes:?}");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// The console's one header row says which agent runs and how it is; End leaves it ended
/// (an illustrated Start, no new session on its own) until Start.
#[gpui::test]
fn the_console_says_its_agent_and_state_and_stays_ended(cx: &mut TestAppContext) {
    use crate::console::Status;
    let (repo, lock_dir, model, ws, _client, cx) = connected("console-header", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    // `tail -f /dev/null` stands in for `claude`, started as New would start it (the
    // console still hidden, so it starts nothing by itself).
    hook_command(&ws, cx);
    cx.update(|_, cx| cx.set_global(super::IdeArgs::default()));
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some("/usr/bin/tail".into())));
    let args = ["-f".to_owned(), "/dev/null".to_owned()];
    // Under the whole suite's load a process can fail to start now and then: a few tries.
    let mut status = Status::NotRunning;
    for _ in 0..5 {
        cx.update(|w, cx| console.update(cx, |c, cx| c.start(&repo, &args, w, cx)));
        pump_until(cx, "the session", |cx| cx.update(|_, cx| console.read(cx).session_text(&repo).is_some()));
        pump(cx, 200);
        status = cx.update(|_, cx| console.update(cx, |c, cx| c.status(Some(&repo), cx)));
        if status != Status::Exited {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    assert_eq!(status, Status::Connected, "Claude Code runs and sees the editor");
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx)));
    pump(cx, 200);
    // End: no new session starts by itself; Start does.
    cx.update(|w, cx| {
        let focus = gpui::Focusable::focus_handle(console.read(cx), cx);
        w.focus(&focus, cx);
    });
    assert!(cx.debug_bounds("console-header").is_some() && cx.debug_bounds("console-agent").is_some());
    console.update_in(cx, |c, _, cx| c.end_session(&repo, cx));
    pump(cx, 500);
    let status = cx.update(|_, cx| console.update(cx, |c, cx| c.status(Some(&repo), cx)));
    assert_eq!(status, Status::NotRunning, "it stays ended");
    assert!(cx.debug_bounds("console-start").is_some(), "the empty state's Start button");
    cx.update(|w, cx| console.update(cx, |c, cx| c.start(&repo, &[], w, cx)));
    pump_until(cx, "the new session", |cx| cx.update(|_, cx| console.read(cx).session_text(&repo).is_some()));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// The last editor tab closes like any other, and the empty editor shows in its place:
/// the ways to a file, and the file just closed among the recent ones.
#[gpui::test]
fn the_last_tab_closes_and_the_empty_editor_shows(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("last-tab", cx);
    let hello = repo.join("src/demo/Hello.java");
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    let editor = editor_of(&ws, &hello, cx).unwrap();
    pump_until(cx, "the file", |cx| text_of(&editor, cx) == HELLO);
    assert_eq!(cx.update(|_, cx| ws.read(cx).editors(cx).len()), 1);
    assert!(cx.debug_bounds("close-tab-0").is_some(), "the only tab has its close button");
    let id = editor.entity_id().as_u64();
    cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::CloseEditorTab(id)), cx));
    pump(cx, 300);
    assert!(cx.update(|_, cx| ws.read(cx).editors(cx).is_empty()), "the last tab closed");
    assert!(cx.debug_bounds("empty-editor").is_some(), "the empty editor in its place");
    assert!(cx.debug_bounds("empty-go-to-file").is_some() && cx.debug_bounds("empty-new-file").is_some());
    assert!(cx.debug_bounds("empty-recent-0").is_some(), "the file closed is a recent one");
    cx.update(|_, cx| assert_eq!(model.read(cx).recent_files.first(), Some(&hello)));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}
