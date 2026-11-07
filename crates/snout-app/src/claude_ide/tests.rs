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

/// A stand-in agent in `bin`, running the `sh` script. On Windows a batch file hands it to Git for
/// Windows's shell: the agents npm installs there are batch files too.
fn fake_agent(bin: &Path, sh: &str) -> PathBuf {
    if cfg!(windows) {
        std::fs::write(bin.join("agent.sh"), sh).unwrap();
    }
    let cmd = "@\"C:\\Program Files\\Git\\bin\\bash.exe\" \"%~dp0agent.sh\" %*\n";
    crate::tests::fake_program(bin, "agent", sh, cmd)
}

/// Pump until `done` holds.
fn pump_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let start = Instant::now();
    while !done(cx) {
        // Generous: a loaded run can stretch a second of the window's clock to several real ones.
        assert!(start.elapsed() < Duration::from_secs(60), "timed out waiting for {what}");
        pump(cx, 16);
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Pump until the draft is over; if it never is, say what the agent's terminal shows.
fn pump_until_drafted(
    cx: &mut VisualTestContext,
    model: &Entity<crate::model::Model>,
    console: &Entity<crate::console::ConsolePanel>,
    repo: &Path,
) {
    let start = Instant::now();
    while cx.update(|_, cx| model.read(cx).draft.is_some()) {
        if start.elapsed() > Duration::from_secs(60) {
            let shown = cx.update(|_, cx| console.read(cx).session_text(repo));
            panic!("timed out waiting for the agent's message; its terminal shows: {shown:?}");
        }
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
    // As the system writes it (backslashes on Windows), as the tools answer with it.
    let hello = repo.join("src").join("demo").join("Hello.java");

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
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-enter"));
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
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-enter"));
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
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-alt-k"));
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
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-alt-k"));
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
    // Claude Code runs hooks with `sh`; on Windows with Git for Windows's bash.
    let shell = if cfg!(windows) { r"C:\Program Files\Git\bin\bash.exe" } else { "/bin/sh" };
    let mut child = std::process::Command::new(shell)
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
    // The disk check does not take the file meanwhile: the report shows it. The hold lasts
    // 10 s of real time, which these pumps can take in a loaded run, so it is kept fresh.
    for _ in 0..9 {
        cx.update(|_, cx| editor.update(cx, |e, _| e.claude_hold = Some(Instant::now())));
        pump(cx, 100);
    }
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
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-alt-y"));
    pump_until(cx, "Hello reviewed", |cx| settled(&ws, cx) && listed(&ws, cx).len() == 1);
    pump(cx, 200);
    cx.update(|_, cx| assert!(editor.read(cx).claude_base.is_none(), "no marks left in the gutter"));
    assert_eq!(std::fs::read_to_string(&hello).unwrap(), reverted, "keeping writes nothing");

    // Revert All, by its keys: the file Claude made goes, its tab too.
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-alt-shift-z"));
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
        cx.update(|_, cx| cx.try_global::<super::CodexArgs>().is_some_and(|a| a.0.len() == 18))
    });
    let notify = cx.update(|_, cx| cx.global::<super::CodexArgs>().0.clone());
    assert_eq!(notify[0], "-c");
    let shell = if cfg!(windows) { "notify=[\"curl.exe\"," } else { "notify=[\"/bin/sh\",\"-c\"," };
    assert!(notify[1].starts_with(shell), "{}", notify[1]);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(crate::tests::unix_tool("/bin/cat"))));
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

/// opencode in the console: SnoutCode's plugin reports its prompt and each edit before and
/// after (posted here as the plugin posts them), and the edit plays out in opencode's name
/// and colour, is put down to it line by line, and waits for review as its.
#[gpui::test]
fn opencode_edits_reported_by_the_plugin_play_out_as_its_own(cx: &mut TestAppContext) {
    let repo = demo_repo("opencode-plugin");
    std::fs::create_dir_all(repo.join("src/demo")).unwrap();
    let hello = repo.join("src/demo/Hello.java");
    std::fs::write(&hello, HELLO).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "Hello"]);
    let lock_dir = scratch_dir("opencode-plugin-ide");
    let mut settings = with_repo(&repo);
    settings.console_agent = crate::agents::ConsoleAgent::Opencode;
    let (_model, ws, cx) = open_with_ide(cx, settings, &lock_dir);
    pump_until(cx, "the plugin's variable", |cx| {
        cx.update(|_, cx| cx.try_global::<super::OpencodeEnv>().is_some_and(|e| !e.0.is_empty()))
    });
    // The console's opencode loads the plugin written beside the hooks' settings.
    let (k, v) = cx.update(|_, cx| cx.global::<super::OpencodeEnv>().0[0].clone());
    assert_eq!(k, "OPENCODE_CONFIG_CONTENT");
    let config: Value = serde_json::from_str(&v).unwrap();
    let url = config["plugin"][0].as_str().unwrap();
    // `file:///C:/…` on Windows names `C:/…`; `file:///tmp/…` elsewhere names `/tmp/…`.
    let path = url.strip_prefix(if cfg!(windows) { "file:///" } else { "file://" }).unwrap();
    let plugin = std::fs::read_to_string(path).unwrap();
    assert_eq!(plugin, super::hooks::OPENCODE_PLUGIN);
    // And Codex its hooks.
    let codex = cx.update(|_, cx| cx.global::<super::CodexArgs>().0.clone());
    assert!(codex.iter().any(|a| a.starts_with("hooks.PreToolUse=")), "{codex:?}");

    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(hello.clone(), None, false, w, cx)));
    let editor = editor_of(&ws, &hello, cx).unwrap();
    pump_until(cx, "the file", |cx| text_of(&editor, cx) == HELLO);
    let (port, token) = cx.update(|_, cx| {
        let h = ws.read(cx).claude.as_ref().unwrap().hooks.as_ref().unwrap();
        (h.port(), h.token().to_owned())
    });
    let post = |body: Value| super::hooks::tests::post_to(port, &token, "/agent", &body);
    let base = json!({ "agent": "opencode", "session": "ses_1", "cwd": repo });
    let with = |extra: Value| {
        let mut b = base.clone();
        b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        b
    };
    post(with(json!({ "event": "prompt", "prompt": "Check the name" })));
    let edit = json!({ "filePath": "src/demo/Hello.java", "oldString": "name;", "newString": "name + \"!\";" });
    post(with(json!({ "event": "pre", "tool": "edit", "input": edit })));
    std::fs::write(&hello, HELLO_V2).unwrap();
    post(with(json!({ "event": "post", "tool": "edit", "input": edit })));
    let mut caret = None;
    pump_until(cx, "the edit played and listed", |cx| {
        if caret.is_none() {
            caret = cx.update(|_, cx| editor.read(cx).presence.as_ref().map(|p| p.writer.name.to_string()));
        }
        cx.update(|_, cx| editor.read(cx).live.is_none()) && settled(&ws, cx) && listed(&ws, cx).len() == 1
    });
    assert_eq!(caret.as_deref(), Some("opencode"), "its caret says its name");
    let c = changes(&ws, cx);
    cx.update(|_, cx| {
        let c = c.read(cx);
        let turn = c.set.current().unwrap();
        assert_eq!((turn.agent, turn.prompt.as_deref()), ("opencode", Some("Check the name")));
        assert_eq!(c.set.file(&hello).unwrap().agent, "opencode");
        let owners = c.ledger.owners(&hello).expect("the ledger has the file");
        let edit = c.ledger.edit(owners.runs()[0].1).unwrap();
        assert_eq!(edit.agent.as_str(), "opencode");
        assert_eq!(cx.global::<super::AgentFiles>().0.get(&hello), Some(&"opencode"));
    });
    // Revert All puts back exactly what the plugin's `pre` saw.
    assert_eq!(text_of(&editor, cx), HELLO_V2);
    cx.update(|w, cx| ws.update(cx, |ws, cx| super::bridge::revert_all(ws, w, cx)));
    pump_until(cx, "the revert", |_| std::fs::read_to_string(&hello).unwrap() == HELLO);
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
        assert_eq!(cx.global::<super::AgentSummary>(), &super::AgentSummary { files: 1, agents: vec!["claude"] });
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
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(crate::tests::unix_tool("/usr/bin/tail"))));
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

/// Picking another agent in the console's menu switches to it: the session in front ends
/// (a note says so) and the chosen agent starts; one that cannot start says why, with the
/// way to fix it (the custom agent with no command: Settings, Agents).
#[gpui::test]
fn picking_an_agent_switches_the_console_to_it(cx: &mut TestAppContext) {
    use crate::agents::ConsoleAgent;
    use crate::console::Status;
    let (repo, lock_dir, model, ws, _client, cx) = connected("console-switch", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    hook_command(&ws, cx);
    cx.update(|_, cx| cx.set_global(super::IdeArgs::default()));
    cx.update(|_, cx| cx.set_global(super::CodexArgs::default()));
    // `tail` stands in for every agent: with no file it reads the terminal, and runs on.
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(crate::tests::unix_tool("/usr/bin/tail"))));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx)));
    let running = |cx: &mut VisualTestContext| {
        let status = cx.update(|_, cx| console.update(cx, |c, cx| c.status(Some(&repo), cx)));
        matches!(status, Status::Connected | Status::NotConnected | Status::Running)
    };
    pump_until(cx, "Claude Code's session", |cx| running(cx));
    let agent_of = |cx: &mut VisualTestContext| cx.update(|_, cx| console.read(cx).session_agent(&repo, cx).id);
    assert_eq!(agent_of(cx), crate::agents::CLAUDE);
    cx.update(|w, cx| console.update(cx, |c, cx| c.switch_agent(ConsoleAgent::Codex, w, cx)));
    pump_until(cx, "Codex's session", |cx| agent_of(cx) == crate::agents::CODEX && running(cx));
    assert_eq!(cx.update(|_, cx| model.read(cx).settings.console_agent), ConsoleAgent::Codex);
    assert_eq!(
        cx.update(|_, cx| console.read(cx).note().map(str::to_owned)).as_deref(),
        Some("Claude Code ended; Continue resumes it")
    );
    // The custom agent with no command does not start: the console says where to give one.
    cx.update(|w, cx| console.update(cx, |c, cx| c.switch_agent(ConsoleAgent::Custom, w, cx)));
    pump(cx, 300);
    let status = cx.update(|_, cx| console.update(cx, |c, cx| c.status(Some(&repo), cx)));
    assert_eq!(status, Status::NotRunning);
    assert!(cx.debug_bounds("console-missing").is_some(), "it says why the agent cannot start");
    assert!(cx.debug_bounds("console-agent-settings").is_some(), "and opens Settings, Agents");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// A start that was overtaken does not count: a slow lookup of the agent picked first (Amp,
/// not installed) must not put its "not installed" over the agent picked after it, nor its
/// session in that one's place.
#[gpui::test]
fn a_start_overtaken_by_another_agent_does_not_count(cx: &mut TestAppContext) {
    use crate::agents::ConsoleAgent;
    let (repo, lock_dir, model, ws, _client, cx) = connected("console-overtaken", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    hook_command(&ws, cx);
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(crate::tests::unix_tool("/usr/bin/tail"))));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx)));
    pump_until(cx, "the first session", |cx| cx.update(|_, cx| console.read(cx).session_text(&repo).is_some()));
    // Back and forth, faster than the sessions can start: the last pick stands, every time.
    for _ in 0..3 {
        for which in [ConsoleAgent::Amp, ConsoleAgent::Codex, ConsoleAgent::Opencode] {
            cx.update(|w, cx| console.update(cx, |c, cx| c.switch_agent(which, w, cx)));
        }
    }
    pump_until(cx, "the last pick's session", |cx| {
        cx.update(|_, cx| {
            console.read(cx).session_agent(&repo, cx).id == crate::agents::OPENCODE
                && console.read(cx).session_text(&repo).is_some()
        })
    });
    pump(cx, 500);
    cx.update(|_, cx| {
        assert_eq!(console.read(cx).session_agent(&repo, cx).id, crate::agents::OPENCODE);
        assert_eq!(crate::agents::console_agent(cx).id, crate::agents::OPENCODE, "Ask to Fix reaches the one showing");
        assert_eq!(console.read(cx).error(), None);
        assert_eq!(model.read(cx).settings.console_agent, ConsoleAgent::Opencode);
    });
    for dir in [repo, lock_dir] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// The console's agents get the arguments they should: opencode and Amp once got `-c ""`.
#[gpui::test]
fn opencode_and_amp_get_their_own_arguments(cx: &mut TestAppContext) {
    use crate::agents::ConsoleAgent;
    let (repo, lock_dir, model, ws, _client, cx) = connected("console-args", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    // A stand-in that prints the arguments it got, then runs on.
    let bin = crate::tests::scratch_dir("console-args-bin");
    let fake = fake_agent(&bin, "#!/bin/sh\necho \"args=[$*]\"\nexec cat\n");
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::ToggleConsole, cx)));
    pump_until(cx, "the first session", |cx| cx.update(|_, cx| console.read(cx).session_text(&repo).is_some()));
    for (which, id) in [(ConsoleAgent::Opencode, crate::agents::OPENCODE), (ConsoleAgent::Amp, crate::agents::AMP)] {
        cx.update(|w, cx| console.update(cx, |c, cx| c.switch_agent(which, w, cx)));
        pump_until(cx, "its session", |cx| {
            cx.update(|_, cx| {
                console.read(cx).session_agent(&repo, cx).id == id
                    && console.read(cx).session_text(&repo).is_some_and(|t| t.contains("args=["))
            })
        });
        let text = cx.update(|_, cx| console.read(cx).session_text(&repo)).unwrap();
        assert!(text.contains("args=[]"), "{id} starts with no arguments of ours: {text}");
        // And Continue's arguments reach it.
        let resume = crate::console::continue_args(&crate::agents::Agent::of(id, &Default::default()));
        cx.update(|w, cx| console.update(cx, |c, cx| c.start(&repo, &resume, w, cx)));
        pump_until(cx, "the resumed session", |cx| {
            cx.update(|_, cx| console.read(cx).session_text(&repo).is_some_and(|t| t.contains("args=[")))
        });
        let text = cx.update(|_, cx| console.read(cx).session_text(&repo)).unwrap();
        assert!(text.contains(&format!("args=[{}]", resume.join(" "))), "{id}: {text}");
    }
    for dir in [repo, lock_dir, bin] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Port 1 stands for a sandbox: the script cannot reach the window, but can reach its folder.
#[gpui::test]
fn an_agent_in_a_sandbox_drafts_through_the_session_folder(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("draft-sandbox", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    hook_command(&ws, cx);
    let bin = crate::tests::scratch_dir("draft-sandbox-bin");
    let fake = fake_agent(
        &bin,
        "#!/bin/sh\necho ready\nwhile read -r line; do\n  api=$(printf '%s' \"$line\" | sed -n 's/.*Run `\\([^ ]*\\) getCommitChange.*/\\1/p')\n  api=${api#\\'}\n  api=${api%\\'}\n  export SNOUT_HOOK_PORT=1\n  if \"$api\" getCommitChange | grep -q 'NOTES.md'; then\n    \"$api\" setCommitMessage <<'MESSAGE'\nChange Hello\n\nIt didn't change before.\nMESSAGE\n  fi\ndone\n",
    );
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::Refresh, cx)));
    pump_until(cx, "the change", |cx| cx.update(|_, cx| model.read(cx).can_draft()));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::DraftMessage, cx)));
    pump_until(cx, "the draft under way", |cx| cx.update(|_, cx| model.read(cx).draft.is_some()));
    let env = cx.update(|_, cx| cx.global::<IdeEnv>().0.clone());
    let header = env.iter().find(|(k, _)| *k == super::api::HEADER_ENV).map(|(_, v)| PathBuf::from(v)).unwrap();
    let dir = header.parent().unwrap().to_owned();
    pump_until(cx, "the change laid out", |_| dir.join(super::api::COMMIT_CHANGE).exists());
    pump_until_drafted(cx, &model, &console, &repo);
    cx.update(|_, cx| assert_eq!(model.read(cx).commit_msg, "Change Hello\n\nIt didn't change before."));
    pump_until(cx, "the draft's files cleared", |_| {
        !dir.join(super::api::COMMIT_CHANGE).exists() && !dir.join(super::api::COMMIT_MESSAGE).exists()
    });
    for dir in [repo, lock_dir, bin] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Draft asks the agent in the console: the console opens, the agent starts, the prompt goes,
/// and what the agent hands back through the editor's API lands in the commit box.
#[gpui::test]
fn draft_asks_the_agent_in_the_console_and_takes_its_message(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("draft", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    hook_command(&ws, cx);
    // A stand-in for an agent: it reads the prompt, runs the script it names for the change,
    // and hands back a message that shows it saw the change.
    let bin = crate::tests::scratch_dir("draft-bin");
    let fake = fake_agent(
        &bin,
        "#!/bin/sh\necho ready\nwhile read -r line; do\n  api=$(printf '%s' \"$line\" | sed -n 's/.*Run `\\([^ ]*\\) getCommitChange.*/\\1/p')\n  api=${api#\\'}\n  api=${api%\\'}\n  if \"$api\" getCommitChange | grep -q 'NOTES.md'; then\n    \"$api\" setCommitMessage <<'MESSAGE'\nChange Hello\n\nIt didn't change before.\nMESSAGE\n  fi\ndone\n",
    );
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::Refresh, cx)));
    pump_until(cx, "the change", |cx| cx.update(|_, cx| model.read(cx).can_draft()));
    assert!(!cx.update(|_, cx| model.read(cx).settings.console_open), "the console starts closed");
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::DraftMessage, cx)));
    pump_until(cx, "the draft under way", |cx| cx.update(|_, cx| model.read(cx).draft.is_some()));
    pump_until_drafted(cx, &model, &console, &repo);
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert!(m.settings.console_open, "it opens as the toolbar's button opens it");
        assert_eq!(m.commit_msg, "Change Hello\n\nIt didn't change before.");
        assert_eq!(m.message, "Claude drafted the commit message");
    });
    // Nobody is waiting now, so a late message goes nowhere.
    let late = cx.update(|_, cx| model.update(cx, |m, cx| m.take_draft("Too late", "Claude", cx)));
    assert!(late.is_err());
    cx.update(|_, cx| assert_ne!(model.read(cx).commit_msg, "Too late"));
    for dir in [repo, lock_dir, bin] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Stop takes the prompt back while it waits, and the Draft button returns.
#[gpui::test]
fn picking_another_agent_ends_a_draft_that_waits_on_the_first(cx: &mut TestAppContext) {
    let (_repo, _lock_dir, model, ws, _client, cx) = connected("draft-swap", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    hook_command(&ws, cx);
    // An agent that stays up and never answers.
    let fake = fake_agent(&crate::tests::scratch_dir("draft-swap-bin"), "#!/bin/sh\necho ready\nsleep 60\n");
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(fake.clone())));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::Refresh, cx)));
    pump_until(cx, "the change", |cx| cx.update(|_, cx| model.read(cx).can_draft()));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::DraftMessage, cx)));
    pump_until(cx, "the draft under way", |cx| cx.update(|_, cx| model.read(cx).draft.is_some()));
    // The first agent is up, so its session is what the swap replaces.
    let root = cx.update(|_, cx| model.read(cx).project_root()).unwrap();
    pump_until(cx, "the agent", |cx| {
        cx.update(|_, cx| console.read(cx).session_text(&root).is_some_and(|t| t.contains("ready")))
    });
    let which = crate::agents::ConsoleAgent::ALL.into_iter().find(|a| a.id() == crate::agents::AMP).unwrap();
    cx.update(|w, cx| console.update(cx, |c, cx| c.switch_agent(which, w, cx)));
    pump(cx, 100);
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert!(m.draft.is_none(), "it does not wait on an agent that is gone");
        assert!(m.message.contains("Click Draft to ask Amp"), "{}", m.message);
    });
}

#[gpui::test]
fn stop_takes_back_a_draft_that_is_waiting(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("draft-stop", cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    hook_command(&ws, cx);
    // `tail` prints nothing, so the prompt stays waiting.
    cx.update(|_, cx| console.update(cx, |c, _| c.program = Some(crate::tests::unix_tool("/usr/bin/tail"))));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::Refresh, cx)));
    pump_until(cx, "the change", |cx| cx.update(|_, cx| model.read(cx).can_draft()));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::DraftMessage, cx)));
    pump_until(cx, "the draft", |cx| cx.update(|_, cx| model.read(cx).draft.is_some()));
    pump_until(cx, "the session", |cx| cx.update(|_, cx| console.read(cx).session_text(&repo).is_some()));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::CancelDraft, cx)));
    pump(cx, 100);
    cx.update(|_, cx| {
        assert!(model.read(cx).can_draft(), "the button is back");
        assert_eq!(model.read(cx).message, "Stopped drafting");
    });
    for dir in [repo, lock_dir] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// When the agent is not installed, Draft does not hang: the console says how to install it,
/// and the button comes back.
#[gpui::test]
fn draft_with_no_agent_installed_points_to_the_console(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("draft-missing", cx);
    hook_command(&ws, cx);
    let console = cx.update(|_, cx| crate::console::console_of(ws.read(cx), cx)).unwrap();
    cx.update(|_, cx| console.update(cx, |c, _| c.program = None));
    cx.update(|_, cx| {
        model.update(cx, |m, cx| {
            // A path that is not there, so the agent counts as not installed. Absolute on every
            // system: a relative one is left for the system to find.
            m.settings.claude_path = Some(lock_dir.join("nonexistent").join("claude"));
            m.handle(crate::model::Action::Refresh, cx)
        })
    });
    pump_until(cx, "the change", |cx| cx.update(|_, cx| model.read(cx).can_draft()));
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::DraftMessage, cx)));
    pump_until(cx, "the draft to end", |cx| {
        cx.update(|_, cx| model.read(cx).message.contains("is not installed yet") && model.read(cx).can_draft())
    });
    assert!(cx.debug_bounds("console-install").is_some(), "the console offers to install it");
    for dir in [repo, lock_dir] {
        let _ = std::fs::remove_dir_all(dir);
    }
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

/// Run `script` through the shell on its own thread, with `env`, while the window runs;
/// whether it succeeded, and its standard output.
fn shell_while_pumping(cx: &mut VisualTestContext, script: &str, env: &[(&'static str, String)]) -> (bool, String) {
    // On Windows, the Git for Windows bash that Claude Code runs its commands with.
    let mut c = std::process::Command::new(crate::tests::unix_tool("/bin/sh"));
    c.arg("-c").arg(script).env_remove(super::hooks::PORT_ENV).env_remove(super::api::HEADER_ENV);
    for (k, v) in env {
        c.env(k, v);
    }
    let child = std::thread::spawn(move || c.output().unwrap());
    pump_until(cx, "the command", |_| child.is_finished());
    let out = child.join().unwrap();
    (out.status.success(), String::from_utf8(out.stdout).unwrap().trim_end().to_owned())
}

/// A call of the API as an agent makes it: real HTTP, with the token from the file the console
/// names. The window keeps running while it waits. The answer's status code and body.
fn agent_calls(cx: &mut VisualTestContext, tool: &str, arguments: Value, with_token: bool) -> (u16, String) {
    let call = agent_starts_call(cx, tool, arguments, with_token);
    agent_gets_answer(cx, call)
}

/// The agent's call on its way, which may wait for the user.
fn agent_starts_call(
    cx: &mut VisualTestContext,
    tool: &str,
    arguments: Value,
    with_token: bool,
) -> std::thread::JoinHandle<String> {
    use std::io::{Read as _, Write as _};
    let env = cx.update(|_, cx| cx.global::<IdeEnv>().0.clone());
    let var = |name: &str| env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()).expect(name);
    let port: u16 = var(super::hooks::PORT_ENV).parse().unwrap();
    let header = std::fs::read_to_string(var(super::api::HEADER_ENV)).unwrap();
    let header = if with_token { header.trim().to_owned() } else { "x-none: none".to_owned() };
    let body = arguments.to_string();
    let request = format!(
        "POST /api/{tool} HTTP/1.1\r\nhost: 127.0.0.1\r\n{header}\r\ncontent-type: application/json\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    std::thread::spawn(move || {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    })
}

/// The status and body of the answer to a call on its way.
fn agent_gets_answer(cx: &mut VisualTestContext, call: std::thread::JoinHandle<String>) -> (u16, String) {
    pump_until(cx, "the app's answer", |_| call.is_finished());
    let out = call.join().unwrap();
    let status = out.split_whitespace().nth(1).and_then(|s| s.parse().ok()).expect("a status");
    (status, out.split_once("\r\n\r\n").map_or(String::new(), |(_, b)| b.to_owned()))
}

/// The harness, as an agent meets it: this test is the agent, speaking HTTP to the real window,
/// and what it asserts is what the window does.
#[gpui::test]
fn an_agent_finds_out_where_it_stands_and_shows_the_user_a_window(cx: &mut TestAppContext) {
    let (repo, lock_dir, _model, ws, _client, cx) = connected("harness", cx);
    hook_command(&ws, cx);

    // Everything the manual offers is really answered.
    use super::capabilities::Effect;
    for c in super::capabilities::all().filter(|c| c.effect != Effect::Change) {
        let (status, body) = agent_calls(cx, c.name, json!({}), true);
        assert!(!matches!(status, 404 | 504), "{}: {status} {body}", c.name);
    }

    // Where it stands: the project and its branch.
    let state = |cx: &mut VisualTestContext| {
        let (status, body) = agent_calls(cx, "getAppState", json!({}), true);
        assert_eq!(status, 200, "{body}");
        serde_json::from_str::<Value>(&body).unwrap()
    };
    let s = state(cx);
    assert_eq!(s["project"]["path"], repo.to_string_lossy().as_ref());
    assert_eq!(s["project"]["opening"], false);
    assert!(s["project"]["git"]["branch"].as_str().unwrap().contains("main"), "{s}");
    assert_eq!(s["version"], env!("CARGO_PKG_VERSION"));

    // A file it opens is among the open files, and is the one in front.
    let hello = repo.join("src/demo/Hello.java");
    let (status, body) = agent_calls(cx, "openFile", json!({ "filePath": hello.to_string_lossy() }), true);
    assert_eq!(status, 200, "{body}");
    let s = state(cx);
    let files = s["openFiles"].as_array().unwrap();
    assert!(files.iter().any(|f| f["path"] == hello.to_string_lossy().as_ref() && f["active"] == true), "{s}");

    // The window comes forward, and asking again does not hide it.
    use crate::workspace::Tool;
    cx.update(|_, cx| assert!(!ws.read(cx).tool_open(Tool::Problems, cx)));
    for _ in 0..2 {
        let (status, body) = agent_calls(cx, "showToolWindow", json!({ "name": "problems" }), true);
        assert_eq!((status, body.as_str()), (200, "The Problems window shows"));
        pump(cx, 50);
        cx.update(|_, cx| assert!(ws.read(cx).tool_open(Tool::Problems, cx)));
    }
    let s = state(cx);
    assert!(s["windowsShowing"].as_array().unwrap().contains(&json!("Problems")), "{s}");
    let (status, body) = agent_calls(cx, "showToolWindow", json!({ "name": "Git" }), true);
    assert_eq!((status, body.as_str()), (200, "The Git window shows"));
    pump(cx, 50);
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, crate::panels::Activity::Git));

    // A window that does not exist: the answer lists those that do, so the agent can try again.
    let (status, body) = agent_calls(cx, "showToolWindow", json!({ "name": "Nope" }), true);
    assert_eq!(status, 400);
    assert!(body.contains("No window called Nope") && body.contains("Agent Changes"), "{body}");
    // A tool that does not exist, and no token.
    assert_eq!(agent_calls(cx, "deleteEverything", json!({}), true).0, 404);
    assert_eq!(agent_calls(cx, "getAppState", json!({}), false).0, 401);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// A change waits for the user: nothing happens until they answer, Don't Allow tells the agent so,
/// Allow is for this one call, and Allow for This Session stops the asking for that tool. Every call
/// is in the Agent Log, with how it ended.
#[gpui::test]
fn a_change_waits_for_the_user_and_the_log_says_how_it_ended(cx: &mut TestAppContext) {
    use super::agent_log::Outcome;
    use crate::model::Dialog;
    let (repo, lock_dir, model, ws, _client, cx) = connected("harness-ask", cx);
    hook_command(&ws, cx);
    let hello = repo.join("src/demo/Hello.java");
    let hello_text = hello.to_string_lossy().into_owned();
    agent_calls(cx, "openFile", json!({ "filePath": hello_text }), true);
    let editor = editor_of(&ws, &hello, cx).expect("the tab");
    let on_disk = || std::fs::read_to_string(&hello).unwrap();
    let before = on_disk();
    let edit = |cx: &mut VisualTestContext, text: &str| {
        cx.update(|w, cx| {
            editor.update(cx, |e, cx| {
                e.state.update(cx, |s, cx| s.set_value(text.to_owned(), w, cx));
                e.dirty = true;
            });
        });
        pump(cx, 50);
    };
    let asking = |cx: &mut VisualTestContext| {
        pump_until(cx, "the card", |cx| {
            cx.update(|_, cx| matches!(model.read(cx).dialog, Some(Dialog::AgentAsks { .. })))
        });
        cx.run_until_parked();
    };
    let save = json!({ "filePath": hello_text });

    // Don't Allow: the file stays, and the agent is told.
    edit(cx, "class Refused {}\n");
    let call = agent_starts_call(cx, "saveDocument", save.clone(), true);
    asking(cx);
    cx.update(|_, cx| match model.read(cx).dialog.as_ref() {
        Some(Dialog::AgentAsks { what, tool, .. }) => {
            assert_eq!((what.as_str(), tool.as_str()), (format!("save {hello_text}").as_str(), "saveDocument"));
        }
        other => panic!("{other:?}"),
    });
    assert_eq!(on_disk(), before, "nothing yet");
    assert!(!call.is_finished(), "the agent waits");
    // The card comes up while the user may be typing: Return does not answer it.
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|_, cx| assert!(matches!(model.read(cx).dialog, Some(Dialog::AgentAsks { .. })), "still asking"));
    assert_eq!(on_disk(), before, "Return allowed nothing");
    crate::quit_tests::click(cx, "agent-refuse");
    let (status, body) = agent_gets_answer(cx, call);
    assert_eq!(status, 422);
    assert!(body.contains("did not allow"), "{body}");
    assert_eq!(on_disk(), before);

    // Allow: this call, and the next asks again.
    let call = agent_starts_call(cx, "saveDocument", save.clone(), true);
    asking(cx);
    crate::quit_tests::click(cx, "agent-once");
    assert_eq!(agent_gets_answer(cx, call).0, 200);
    assert_eq!(on_disk(), "class Refused {}\n");

    // Allow for This Session: this call and the ones after.
    edit(cx, "class Session {}\n");
    let call = agent_starts_call(cx, "saveDocument", save.clone(), true);
    asking(cx);
    crate::quit_tests::click(cx, "agent-session");
    assert_eq!(agent_gets_answer(cx, call).0, 200);
    assert_eq!(on_disk(), "class Session {}\n");
    edit(cx, "class Asked {}\n");
    assert_eq!(agent_calls(cx, "saveDocument", save.clone(), true).0, 200, "no card this time");
    assert_eq!(on_disk(), "class Asked {}\n");

    // Showing a window never asks.
    agent_calls(cx, "showToolWindow", json!({ "name": "Agent Log" }), true);
    pump(cx, 100);
    assert!(cx.debug_bounds("agent-log").is_some(), "the log shows");

    let log = cx.update(|_, cx| ws.read(cx).agent_log.read(cx).entries.iter().cloned().collect::<Vec<_>>());
    let saves: Vec<_> = log.iter().filter(|e| e.tool == "saveDocument").collect();
    assert_eq!(saves.len(), 4);
    assert!(matches!(&saves[0].outcome, Outcome::Failed(why) if why.contains("did not allow")), "{:?}", saves[0]);
    assert!(saves[1..].iter().all(|e| e.outcome == Outcome::Done), "{saves:?}");
    assert!(saves[0].arguments.contains("Hello.java"));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// An agent in the home folder opens the project it made, and runs what is in it: the whole of the
/// flow with no project open. Needs Node for the script; without it, it stops at the project.
#[gpui::test]
fn an_agent_opens_the_project_it_made_and_runs_it(cx: &mut TestAppContext) {
    use crate::model::Dialog;
    let lock_dir = scratch_dir("harness-make-ide");
    let (model, ws, cx) = open_with_ide(cx, Settings::default(), &lock_dir);
    hook_command(&ws, cx);
    let made = scratch_dir("harness-made");
    std::fs::write(
        made.join("package.json"),
        r#"{ "name": "made", "scripts": { "hello": "node -e \"console.log('hello from the agent')\"" } }"#,
    )
    .unwrap();
    let path = made.to_string_lossy().into_owned();

    let (_, body) = agent_calls(cx, "listProjects", json!({}), true);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["open"], Value::Null, "{body}");
    let card = |cx: &mut VisualTestContext| {
        pump_until(cx, "the card", |cx| {
            cx.update(|_, cx| matches!(model.read(cx).dialog, Some(Dialog::AgentAsks { .. })))
        });
        cx.run_until_parked();
    };
    // A folder that is not there: the agent is told, after the user said yes.
    let call = agent_starts_call(cx, "switchProject", json!({ "path": format!("{path}/nope") }), true);
    card(cx);
    crate::quit_tests::click(cx, "agent-once");
    let (status, body) = agent_gets_answer(cx, call);
    assert!(status == 422 && body.contains("no folder"), "{status} {body}");

    let call = agent_starts_call(cx, "switchProject", json!({ "path": path }), true);
    card(cx);
    crate::quit_tests::click(cx, "agent-session");
    assert_eq!(agent_gets_answer(cx, call).0, 200);
    pump_until(cx, "the project", |cx| cx.update(|_, cx| model.read(cx).project_open()));
    let (_, body) = agent_calls(cx, "getAppState", json!({}), true);
    let opened =
        PathBuf::from(serde_json::from_str::<Value>(&body).unwrap()["project"]["path"].as_str().unwrap().to_owned());
    assert_eq!(opened.canonicalize().unwrap(), made.canonicalize().unwrap());

    if crate::node_tests::has_node().is_none() {
        let _ = std::fs::remove_dir_all(made);
        return;
    }
    let configs = |cx: &mut VisualTestContext| {
        let (_, body) = agent_calls(cx, "listRunConfigurations", json!({}), true);
        serde_json::from_str::<Value>(&body).unwrap()
    };
    let mut id = String::new();
    pump_until(cx, "the scripts", |cx| {
        id = configs(cx)["configurations"][0]["id"].as_str().unwrap_or_default().to_owned();
        !id.is_empty()
    });
    let call = agent_starts_call(cx, "startRun", json!({ "configuration": id }), true);
    card(cx);
    crate::quit_tests::click(cx, "agent-session");
    let (status, body) = agent_gets_answer(cx, call);
    assert_eq!(status, 200, "{body}");
    let run = serde_json::from_str::<Value>(&body).unwrap()["id"].as_u64().unwrap();
    pump_until(cx, "the run's output", |cx| {
        let (_, body) = agent_calls(cx, "getRunOutput", json!({ "id": run }), true);
        body.contains("hello from the agent")
    });
    pump_until(cx, "the run's end", |cx| {
        let (_, body) = agent_calls(cx, "listRuns", json!({}), true);
        let list = serde_json::from_str::<Value>(&body).unwrap();
        list["runs"].as_array().unwrap().iter().any(|r| r["id"] == run && r["running"] == false)
    });
    let (_, body) = agent_calls(cx, "getTestResults", json!({ "id": run }), true);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["tests"], Value::Null, "no tests in this run: {body}");
    // No more cards for that tool; a name it does not know gets the list.
    let (status, body) = agent_calls(cx, "startRun", json!({ "configuration": "nope" }), true);
    assert!(status == 400 && body.contains("The configurations"), "{status} {body}");
    let _ = std::fs::remove_dir_all(made);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// The language engine's answers, through the harness, on the real Java engine: what no shell gives
/// an agent. Needs the engine distribution and a JDK 17; without them it says so and passes.
#[gpui::test]
fn an_agent_asks_the_java_engine_what_the_code_is(cx: &mut TestAppContext) {
    use crate::lang::java;
    let _engine = crate::tests::engine_lock();
    let (Some(_), Some(jdk)) = (java::engine_dist(&*snout_host::local()), crate::lang_tests::jdk17()) else {
        println!("harness, Java: skipped; needs engine/build/dist and a JDK 17 (SNOUT_TEST_JDK17)");
        return;
    };
    let repo = crate::java_tools_tests::shop("harness-java");
    let lock_dir = scratch_dir("harness-java-ide");
    let mut settings = with_repo(&repo);
    settings.project_jdks.insert(repo.clone(), jdk);
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open_with_ide(cx, settings, &lock_dir);
    cx.update(|_, cx| cx.set_global(crate::lang_tests::TestWorkspace(ws.clone())));
    hook_command(&ws, cx);
    let dir = repo.join("src/main/java/demo/shop");
    let shop = dir.join("Shop.java");
    let shape = dir.join("Shape.java");
    let line_of = |path: &Path, text: &str| -> u64 {
        let body = std::fs::read_to_string(path).unwrap();
        body.lines().position(|l| l.contains(text)).unwrap_or_else(|| panic!("{text} in {}", path.display())) as u64 + 1
    };
    agent_calls(cx, "openFile", json!({ "filePath": shop.to_string_lossy() }), true);
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    let root = repo.clone();
    let start = Instant::now();
    while !cx.update(|_, cx| hub.read(cx).servers.get(&root).is_some_and(|s| s.first_diagnostics.is_some())) {
        assert!(start.elapsed() < Duration::from_secs(240), "the engine answers within four minutes");
        pump(cx, 200);
    }
    let call = |cx: &mut VisualTestContext, tool: &str, a: Value| -> Value {
        let (status, body) = agent_calls(cx, tool, a, true);
        assert_eq!(status, 200, "{tool}: {body}");
        serde_json::from_str(&body).unwrap_or(Value::String(body))
    };

    // By name, across the project. The engine may still be indexing: ask again.
    let start = Instant::now();
    let found = loop {
        let v = call(cx, "findSymbol", json!({ "query": "Circle" }));
        if v["count"].as_u64().unwrap_or(0) > 0 || start.elapsed() > Duration::from_secs(60) {
            break v;
        }
        pump(cx, 500);
    };
    assert!(found["symbols"].to_string().contains("Circle.java"), "{found}");

    let at_call = line_of(&shop, ".area()");
    let there = json!({ "filePath": shop.to_string_lossy(), "line": at_call, "text": "area" });
    let definition = call(cx, "getDefinition", there.clone());
    assert!(definition["places"].to_string().contains("Shape.java"), "{definition}");
    let at_decl = line_of(&shape, "double area");
    let decl = json!({ "filePath": shape.to_string_lossy(), "line": at_decl, "text": "area" });
    let references = call(cx, "findReferences", decl.clone());
    assert!(references["places"].to_string().contains("Shop.java"), "{references}");
    let hover = call(cx, "getHover", there);
    assert!(hover.to_string().contains("area"), "{hover}");

    // The engine knows a type's hierarchy once the file is open; the answer says so until it is.
    let at_type = line_of(&shape, "interface Shape");
    let ty = json!({ "filePath": shape.to_string_lossy(), "line": at_type, "text": "Shape", "direction": "subtypes" });
    let (status, body) = agent_calls(cx, "getHierarchy", ty.clone(), true);
    if status != 200 {
        assert!(body.contains("makeFrontmost"), "{status} {body}");
        call(cx, "openFile", json!({ "filePath": shape.to_string_lossy(), "makeFrontmost": false }));
    }
    let start = Instant::now();
    let subtypes = loop {
        let (status, body) = agent_calls(cx, "getHierarchy", ty.clone(), true);
        if status == 200 && body.contains("Circle") || start.elapsed() > Duration::from_secs(30) {
            break serde_json::from_str::<Value>(&body).unwrap_or(Value::String(body));
        }
        pump(cx, 500);
    };
    assert!(subtypes["items"].to_string().contains("Circle"), "{subtypes}");
    let mut callers = decl.clone();
    callers["direction"] = json!("callers");
    let callers = call(cx, "getHierarchy", callers);
    assert!(callers["items"].to_string().contains("Shop.java"), "{callers}");
    let problems = call(cx, "getProjectProblems", json!({}));
    assert!(problems["total"].is_u64(), "{problems}");

    // What it names wrongly is put right by the answer.
    let (status, body) = agent_calls(
        cx,
        "getDefinition",
        json!({ "filePath": shop.to_string_lossy(), "line": 1, "text": "nope" }),
        true,
    );
    assert!(status == 400 && body.contains("not on line 1"), "{status} {body}");

    // A rename changes files, so it asks; the tabs have it, unsaved.
    let at_sum = line_of(&shop, "double sum");
    let rename = json!({ "filePath": shop.to_string_lossy(), "line": at_sum, "text": "sum", "newName": "runningSum" });
    let asked = agent_starts_call(cx, "renameSymbol", rename, true);
    pump_until(cx, "the card", |cx| {
        cx.update(|_, cx| matches!(model.read(cx).dialog, Some(crate::model::Dialog::AgentAsks { .. })))
    });
    cx.run_until_parked();
    crate::quit_tests::click(cx, "agent-once");
    let (status, body) = agent_gets_answer(cx, asked);
    assert_eq!(status, 200, "{body}");
    let editor = editor_of(&ws, &shop, cx).expect("the tab");
    pump_until(cx, "the rename in the tab", |cx| {
        cx.update(|_, cx| editor.read(cx).state.read(cx).value().to_string().contains("double runningSum"))
    });
    let on_disk = std::fs::read_to_string(&shop).unwrap();
    assert!(on_disk.contains("double sum"), "unsaved, for the user to review");

    // The build's tasks, and running one where the user sees it.
    let start = Instant::now();
    loop {
        let tasks = call(cx, "listBuildTasks", json!({}));
        if tasks["gradle"].to_string().contains(":classes") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(240), "the tasks: {tasks}");
        pump(cx, 500);
    }
    let asked = agent_starts_call(cx, "runBuildTask", json!({ "task": ":classes" }), true);
    pump_until(cx, "the card", |cx| {
        cx.update(|_, cx| matches!(model.read(cx).dialog, Some(crate::model::Dialog::AgentAsks { .. })))
    });
    cx.run_until_parked();
    crate::quit_tests::click(cx, "agent-once");
    let (status, body) = agent_gets_answer(cx, asked);
    assert_eq!(status, 200, "{body}");
    let run = serde_json::from_str::<Value>(&body).unwrap()["id"].as_u64().unwrap();
    let start = Instant::now();
    loop {
        let runs = call(cx, "listRuns", json!({}));
        if runs["runs"].as_array().unwrap().iter().any(|r| r["id"] == run && r["running"] == false) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(240), "the run: {runs}");
        pump(cx, 500);
    }
    let output = call(cx, "getRunOutput", json!({ "id": run }));
    assert!(output["output"].is_string(), "{output}");
    // Spring: this project has none, and the answer says so without failing.
    assert_eq!(call(cx, "listBeans", json!({}))["count"], 0);
    assert_eq!(call(cx, "listEndpoints", json!({ "filter": "x" }))["count"], 0);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// The rest of the app, as an agent asks for it: Git through the window, the settings it may change
/// and those it may not, telling the user something, and its own changes.
#[gpui::test]
fn an_agent_works_git_settings_and_its_own_changes(cx: &mut TestAppContext) {
    use crate::model::Dialog;
    let (repo, lock_dir, model, ws, _client, cx) = connected("harness-rest", cx);
    let (port, hook_cmd) = hook_command(&ws, cx);
    let call = |cx: &mut VisualTestContext, tool: &str, a: Value| -> Value {
        let (status, body) = agent_calls(cx, tool, a, true);
        assert_eq!(status, 200, "{tool}: {body}");
        serde_json::from_str(&body).unwrap_or(Value::String(body))
    };
    // Each change is allowed once here: the card's own test is above.
    let allowed = |cx: &mut VisualTestContext, tool: &str, a: Value| -> (u16, String) {
        let asked = agent_starts_call(cx, tool, a, true);
        pump_until(cx, "the card", |cx| {
            cx.update(|_, cx| matches!(model.read(cx).dialog, Some(Dialog::AgentAsks { .. })))
        });
        cx.run_until_parked();
        crate::quit_tests::click(cx, "agent-once");
        agent_gets_answer(cx, asked)
    };

    // A change on disk, shown, staged and committed through the window.
    std::fs::write(repo.join("note.txt"), "a note\n").unwrap();
    cx.update(|_, cx| model.update(cx, |m, cx| m.handle(crate::model::Action::Refresh, cx)));
    pump_until(cx, "the change", |cx| {
        cx.update(|_, cx| model.read(cx).project_status().files.iter().any(|f| f.path == "note.txt"))
    });
    assert_eq!(call(cx, "showChanges", json!({})), "The Git view shows");
    pump(cx, 50);
    cx.update(|_, cx| assert_eq!(ws.read(cx).side.read(cx).activity, crate::panels::Activity::Git));
    assert_eq!(call(cx, "showDiff", json!({ "path": "note.txt" })), "The diff of note.txt shows");
    let (status, body) = agent_calls(cx, "showDiff", json!({ "path": "nope.txt" }), true);
    assert!(status == 400 && body.contains("no changes"), "{status} {body}");
    let (status, body) = allowed(cx, "stageFiles", json!({ "paths": ["note.txt"] }));
    assert_eq!(status, 200, "{body}");
    pump_until(cx, "the file staged", |cx| {
        cx.update(|_, cx| model.read(cx).project_status().files.iter().any(|f| f.path == "note.txt" && f.has_staged()))
    });
    let (status, body) = allowed(cx, "commitChanges", json!({ "message": "Add a note" }));
    assert_eq!(status, 200, "{body}");
    let log = |repo: &Path| {
        let out =
            std::process::Command::new("git").args(["log", "-1", "--format=%s"]).current_dir(repo).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    pump_until(cx, "the commit", |_| log(&repo) == "Add a note");

    // Settings: what it may change takes effect; what would run something is out of reach.
    let before = call(cx, "getSettings", json!({}));
    assert!(before.get("tab_size").is_some(), "{before}");
    assert!(before.get("claude_path").is_none() && before.get("terminal_env").is_none(), "{before}");
    let (status, body) = allowed(cx, "setSetting", json!({ "key": "tab_size", "value": "7" }));
    assert_eq!(status, 200, "{body}");
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.tab_size, 7));
    for key in ["claude_path", "terminal_shell", "custom_agent_command", "terminal_env"] {
        let (status, body) = allowed(cx, "setSetting", json!({ "key": key, "value": "/tmp/evil" }));
        assert!(status == 400 && body.contains("not a setting an agent may change"), "{key}: {status} {body}");
    }
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.custom_agent_command, ""));
    let (status, body) = allowed(cx, "setSetting", json!({ "key": "tab_size", "value": "many" }));
    assert!(status == 400 && body.contains("not a value"), "{status} {body}");
    cx.update(|_, cx| assert_eq!(model.read(cx).settings.tab_size, 7));

    // Telling the user something.
    assert_eq!(call(cx, "showNotification", json!({ "message": "The build is done" })), "The user was told");
    cx.update(|_, cx| {
        let text = ws.read(cx).notifications.read(cx).items(cx).join("\n");
        assert!(text.contains("The build is done"), "{text}");
    });
    let commands = call(cx, "getShortcuts", json!({}));
    assert!(commands["commands"].as_array().unwrap().iter().any(|c| c["command"] == "Show Processes"), "{commands}");
    assert!(call(cx, "listProcesses", json!({}))["processes"].is_array());

    // Its own changes: reported by its hooks, listed, and put back.
    let other = repo.join("src/demo/Other.java");
    let input = |event: &str| {
        json!({ "hook_event_name": event, "session_id": "s", "cwd": repo, "tool_name": "Edit",
                "tool_input": { "file_path": other.to_string_lossy() }, "tool_response": {} })
    };
    let original = std::fs::read_to_string(&other).unwrap();
    run_hook(port, &hook_cmd, input("PreToolUse"));
    std::fs::write(&other, "class Other { int edited; }\n").unwrap();
    run_hook(port, &hook_cmd, input("PostToolUse"));
    pump_until(cx, "the change in the list", |cx| {
        call(cx, "listAgentChanges", json!({}))["files"].to_string().contains("Other.java")
    });
    let path = other.to_string_lossy().into_owned();
    let (status, body) = allowed(cx, "revertAgentChange", json!({ "filePath": path }));
    assert_eq!(status, 200, "{body}");
    pump_until(cx, "the file back", |_| std::fs::read_to_string(&other).unwrap() == original);
    let (status, body) = allowed(cx, "revertAgentChange", json!({ "filePath": path }));
    assert!(status == 400 && body.contains("No agent's change"), "{status} {body}");

    for dir in [repo, lock_dir] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// What an agent shows the user: the view comes forward once, later pushes leave the user where they
/// are, its code has buttons, and a shell block runs in the Terminal only when clicked.
#[gpui::test]
fn an_agent_shows_the_user_something_and_the_user_acts_on_it(cx: &mut TestAppContext) {
    use crate::workspace::Tool;
    let (repo, lock_dir, _model, ws, _client, cx) = connected("harness-view", cx);
    hook_command(&ws, cx);
    let hello = repo.join("src/demo/Hello.java");
    agent_calls(cx, "openFile", json!({ "filePath": hello.to_string_lossy() }), true);
    pump(cx, 50);
    assert!(cx.debug_bounds("agent-view-clear").is_none(), "no view until an agent draws");

    let md = "# A plan\n\nFirst this.\n\n```sh\necho hello-from-the-view > ran.txt\n```\n\n```java\nint added;\n```\n";
    let (status, body) = agent_calls(cx, "showInApp", json!({ "markdown": md, "title": "The plan" }), true);
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains("2 code blocks") && body.contains("1 of them with a Run") && body.contains("not hear"),
        "{body}"
    );
    pump(cx, 50);
    cx.update(|_, cx| assert!(ws.read(cx).tool_open(Tool::AgentView, cx), "the first push brings it forward"));
    assert!(cx.debug_bounds("agent-view-run-0-1").is_some(), "the shell block can run");
    assert!(cx.debug_bounds("agent-view-run-0-2").is_none(), "the Java block cannot");
    assert!(!repo.join("ran.txt").exists(), "nothing runs without a click");

    // Copy, and Insert at the caret of the editor in front.
    let (copy, insert) = ("agent-view-copy-0-2", "agent-view-insert-0-2");
    crate::quit_tests::click(cx, copy);
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text()));
    assert_eq!(copied.as_deref(), Some("int added;"));
    crate::quit_tests::click(cx, insert);
    cx.run_until_parked();
    let editor = editor_of(&ws, &hello, cx).expect("the tab");
    cx.update(|_, cx| assert!(editor.read(cx).state.read(cx).value().contains("int added;")));

    // A later push adds and does not move the user: another window in the same dock stays in front.
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_tool(Tool::Notifications, w, cx)));
    pump(cx, 50);
    let (status, body) = agent_calls(cx, "showInApp", json!({ "markdown": "More." }), true);
    assert_eq!(status, 200, "{body}");
    assert!(!body.contains("code block"), "{body}");
    pump(cx, 50);
    cx.update(|_, cx| {
        assert!(ws.read(cx).tool_open(Tool::Notifications, cx), "it does not take the user away");
        assert_eq!(ws.read(cx).agent_view.read(cx).pushes.len(), 2, "it appends");
    });

    // clear starts afresh, and it survives moving to another dock.
    let (status, _) = agent_calls(cx, "showInApp", json!({ "markdown": "Only this.", "clear": true }), true);
    assert_eq!(status, 200);
    cx.update(|_, cx| assert_eq!(ws.read(cx).agent_view.read(cx).pushes.len(), 1));

    // Run in Terminal: the click runs it, in the project.
    agent_calls(cx, "showInApp", json!({ "markdown": "```sh\necho ran > ran.txt\n```", "clear": true }), true);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_tool(Tool::AgentView, w, cx)));
    pump(cx, 50);
    if !cfg!(windows) {
        crate::quit_tests::click(cx, "agent-view-run-0-0");
        pump_until(cx, "the command to run", |_| repo.join("ran.txt").exists());
        cx.update(|_, cx| assert!(ws.read(cx).tool_open(Tool::Terminal, cx), "the Terminal shows it"));
    }
    for dir in [repo, lock_dir] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// The real agents, signed in, each asked in plain words to do what only the harness can: bring a
/// window forward. Spends a little of each agent's budget, so it runs only when asked for:
/// `SNOUT_REAL_AGENTS=codex,amp,claude`, or `opencode` too where it is installed.
#[gpui::test]
fn real_agents_work_the_window_through_the_harness(cx: &mut TestAppContext) {
    let Ok(agents) = std::env::var("SNOUT_REAL_AGENTS") else { return };
    use crate::workspace::Tool;
    let (repo, lock_dir, _model, ws, _client, cx) = connected("harness-real", cx);
    hook_command(&ws, cx);
    let (env, claude, codex, opencode) = cx.update(|_, cx| {
        (
            cx.global::<IdeEnv>().0.clone(),
            cx.global::<super::IdeArgs>().0.clone(),
            cx.global::<super::CodexArgs>().0.clone(),
            cx.global::<super::OpencodeEnv>().0.clone(),
        )
    });
    let targets =
        [(Tool::Problems, "Problems"), (Tool::Output, "Output"), (Tool::Terminal, "Terminal"), (Tool::Build, "Build")];
    for (agent, (tool, name)) in agents.split(',').zip(targets) {
        let prompt = format!(
            "You are in SnoutCode's console. Use SnoutCode's tools to bring its {name} window to the front. Do nothing \
             else, change no files, then say done."
        );
        let mut c = match agent {
            "codex" => {
                let mut c = std::process::Command::new("codex");
                c.arg("exec").args(&codex).args(["--skip-git-repo-check", prompt.as_str()]);
                c
            }
            "amp" => {
                let mut c = std::process::Command::new("amp");
                c.args(["-x", prompt.as_str()]).env("PLUGINS", "all");
                c
            }
            "claude" => {
                let mut c = std::process::Command::new("claude");
                c.args(&claude).args(["-p", prompt.as_str()]);
                c
            }
            "opencode" => {
                let mut c = std::process::Command::new("opencode");
                c.args(["run", prompt.as_str()]);
                c
            }
            other => panic!("no agent called {other}"),
        };
        c.current_dir(&repo).stdin(std::process::Stdio::null());
        for (k, v) in env.iter().chain(opencode.iter().filter(|_| agent == "opencode")) {
            c.env(k, v);
        }
        let run = std::thread::spawn(move || c.output().unwrap());
        let start = Instant::now();
        while !run.is_finished() && start.elapsed() < Duration::from_secs(280) {
            pump(cx, 200);
        }
        assert!(run.is_finished(), "{agent} did not finish");
        let out = run.join().unwrap();
        let said = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        pump(cx, 100);
        let open = cx.update(|_, cx| ws.read(cx).tool_open(tool, cx));
        eprintln!(
            "--- {agent} ({name}: {open})\n{}",
            said.chars().rev().take(900).collect::<String>().chars().rev().collect::<String>()
        );
        assert!(open, "{agent} did not bring {name} forward: {said}");
    }
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// With no project open the harness still answers, and says so: the agent in the home folder knows
/// it has a project to make.
#[gpui::test]
fn an_agent_with_no_project_is_told_so(cx: &mut TestAppContext) {
    let lock_dir = scratch_dir("harness-no-project-ide");
    let (_model, ws, cx) = open_with_ide(cx, Settings::default(), &lock_dir);
    hook_command(&ws, cx);
    let (status, body) = agent_calls(cx, "getAppState", json!({}), true);
    assert_eq!(status, 200, "{body}");
    let s: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(s["project"], Value::Null, "{s}");
    let home = cx.update(|_, cx| crate::host::host(cx).info().home.clone());
    assert_eq!(s["homeFolder"], home.to_string_lossy().as_ref());
    assert_eq!(s["openFiles"], json!([]));
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// The model asks the editor through the skill the console's `claude` loads: the skill's
/// own commands (its script, which the session's settings let run) open a file and select
/// in it, read the selection back, and fetch the UTCP manual; a UTCP client's call with
/// query parameters works too; without the token nothing happens.
#[gpui::test]
fn the_skill_opens_files_in_the_editor_over_the_api(cx: &mut TestAppContext) {
    let (repo, lock_dir, _model, ws, _client, cx) = connected("claude-api", cx);
    let (port, _) = hook_command(&ws, cx);
    let (args, env) = cx.update(|_, cx| (cx.global::<super::IdeArgs>().0.clone(), cx.global::<IdeEnv>().0.clone()));
    let plugin = PathBuf::from(&args[args.iter().position(|a| a == "--plugin-dir").expect("the plugin") + 1]);
    let skill = std::fs::read_to_string(plugin.join("skills/snoutcode/SKILL.md")).unwrap();
    assert!(env.iter().any(|(k, _)| *k == super::api::HEADER_ENV), "the console gets the token's header file");
    let api = super::api::shell_path(&plugin.join("bin/snout-api"));
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    assert_eq!(settings["permissions"]["allow"], json!([format!("Bash({api}:*)")]), "it runs without asking");
    let blocks: Vec<&str> = skill.split("```sh\n").skip(1).map(|b| b.split("```").next().unwrap().trim_end()).collect();

    // The skill's own command, with the path and line it shows swapped for ours.
    let hello = repo.join("src/demo/Hello.java");
    let open = blocks[0]
        .replace("/abs/path/File.java", &super::api::shell_path(&hello))
        .replace("\"startLine\":42", "\"startText\":\"class Hello\"");
    assert!(open.starts_with(&format!("{api} openFile '")), "{open}");
    assert_eq!(shell_while_pumping(cx, &open, &env), (true, "Opened file and selected text \"class Hello\"".into()));
    assert!(editor_of(&ws, &hello, cx).is_some(), "a tab for the file");
    let (ok, sel) = shell_while_pumping(cx, &format!("{api} getCurrentSelection"), &env);
    assert!(ok);
    assert_eq!(serde_json::from_str::<Value>(&sel).unwrap()["text"], "class Hello");

    // Lines, as query parameters: how a UTCP client sends them.
    let other = repo.join("src/demo/Other.java");
    let q = format!(
        "curl -s -X POST -H @\"$SNOUT_API_HEADER\" \"http://127.0.0.1:$SNOUT_HOOK_PORT/api/openFile?filePath={}&startLine=1\"",
        super::api::shell_path(&other)
    );
    assert_eq!(shell_while_pumping(cx, &q, &env), (true, "Opened file and selected lines 1 to 1".into()));

    // A file that is not there fails the script; the manual; outside the console, and
    // without the token, nothing happens.
    let missing = open.replace(&super::api::shell_path(&hello), &super::api::shell_path(&repo.join("nope.txt")));
    let (ok, out) = shell_while_pumping(cx, &missing, &env);
    assert!(!ok && out.contains("File not found"), "{out}");
    let (ok, manual) = shell_while_pumping(cx, blocks[1], &env);
    assert!(ok);
    let manual: Value = serde_json::from_str(&manual).unwrap();
    assert!(manual["tools"].as_array().unwrap().iter().any(|t| t["name"] == "openFile"));
    let (ok, editor) = shell_while_pumping(cx, &format!("{api} manual editor"), &env);
    assert!(ok && editor.contains("startLine"), "a domain's arguments: {editor}");
    assert!(!shell_while_pumping(cx, &open, &[]).0, "no editor outside the console");
    let anon = format!("curl -s -o /dev/null -w '%{{http_code}}' -X POST http://127.0.0.1:{port}/api/getOpenEditors");
    assert_eq!(shell_while_pumping(cx, &anon, &[]), (true, "401".into()));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}

/// A pull while Claude Code is connected is not Claude's doing: the files it brings (now
/// as the last commit has them) are not recorded as Claude's changes, no tab opens on
/// them, and the window stays where it was. A file Claude changes afterwards still is.
#[gpui::test]
fn a_pull_is_not_put_down_to_claude(cx: &mut TestAppContext) {
    let (repo, lock_dir, model, ws, _client, cx) = connected("claude-pull", cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(crate::panels::Activity::Git, w, cx)));
    // The watcher starts with the first look at the project.
    pump(cx, 300);
    std::thread::sleep(Duration::from_millis(300));
    let other = repo.join("src/demo/Other.java");
    let tabs = |cx: &mut VisualTestContext| cx.update(|_, cx| ws.read(cx).editors(cx).len());
    let before = tabs(cx);

    // What a pull leaves: the file rewritten, and HEAD moved to the commit that has it
    // (made here by Git outside the app, as a `git pull` in a terminal would).
    std::fs::write(&other, "class Other { int pulled; }\n").unwrap();
    git(&repo, &["commit", "-qam", "Upstream change"]);
    pump(cx, 1_500);
    std::thread::sleep(Duration::from_millis(500));
    pump(cx, 500);
    assert!(listed(&ws, cx).is_empty(), "nothing waits for review: {:?}", listed(&ws, cx));
    assert_eq!(tabs(cx), before, "no tab opened on the pulled file");
    cx.update(|_, cx| {
        assert_eq!(ws.read(cx).side.read(cx).activity, crate::panels::Activity::Git, "the Git view stays");
    });

    // Claude (connected, its edits seen only by the watcher) changes it afterwards: that is
    // Claude's, against the text the pull brought.
    std::fs::write(&other, "class Other { int pulled; int claude; }\n").unwrap();
    pump_until(cx, "Claude's change", |cx| !listed(&ws, cx).is_empty());
    assert_eq!(listed(&ws, cx)[0].0, other);
    let _ = model;
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(lock_dir);
}
