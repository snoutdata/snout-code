//! Precise attribution: Claude Code's hooks report every file edit to the app, whether
//! it asked first (`openDiff`) or not (edits it accepts on its own never reach the IDE
//! server). A `claude` the console starts gets extra settings (`--settings <file>`, the
//! user's own settings are never touched) whose `PreToolUse` and `PostToolUse` hooks on
//! the file tools, `UserPromptSubmit` and `Stop` post the hook's input to a small HTTP
//! endpoint here: 127.0.0.1 only, on a port the system picks, each request checked
//! against a token the hook sends in a header read from a file only this user can read
//! (so the token never shows on a command line).
//!
//! The endpoint answers once it has read the file the tool is about to change (before)
//! or has changed (after), so the snapshot is taken while Claude Code waits: a revert
//! puts back exactly what was there. The hook never decides anything: it says nothing
//! and always succeeds, whatever happens here.

use std::io::{BufRead as _, BufReader, ErrorKind, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use snout_host::HostRef;

use super::lockfile;

/// The header the hook presents the token in.
pub const TOKEN_HEADER: &str = "x-snout-hook";
/// The variable that tells the hook where to post.
pub const PORT_ENV: &str = "SNOUT_HOOK_PORT";
/// The tools that change files.
pub const FILE_TOOLS: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];
/// The largest request read (a `Write` of a big file, with its response).
const MAX_BODY: usize = 64 << 20;
/// The largest file snapshot taken; bigger files are reported without their text.
pub const MAX_SNAPSHOT: u64 = 16 << 20;

/// What a hook says happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HookKind {
    /// A file tool is about to run (the permission prompt, if any, comes after).
    PreTool,
    /// A file tool ran.
    PostTool,
    /// The user sent a prompt: a new turn begins.
    Prompt,
    /// Claude Code finished its answer.
    Stop,
    /// Codex finished a turn (its `notify`), with the prompt that started it.
    TurnComplete,
}

/// The file a tool works on, as it was when the hook ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Snapshot {
    /// Not there (a file `Write` creates).
    Missing,
    Text(Arc<String>),
    /// There but not taken: too big, or not text.
    Unread,
}

impl Snapshot {
    pub fn text(&self) -> Option<&Arc<String>> {
        match self {
            Snapshot::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// One hook call.
#[derive(Clone, Debug)]
pub struct HookEvent {
    pub kind: HookKind,
    /// The agent that reported it ([`crate::agents::CLAUDE`], [`crate::agents::CODEX`]).
    pub agent: &'static str,
    pub session: String,
    /// The prompt the user sent (`UserPromptSubmit`, Codex's turn), its first line kept.
    pub prompt: Option<String>,
    /// The tool, for the tool hooks.
    pub tool: String,
    /// The file the tool works on.
    pub path: Option<PathBuf>,
    /// The tool's input as Claude Code gave it.
    pub input: Value,
    /// The file as it was when the hook ran (tool hooks only).
    pub file: Option<Snapshot>,
}

/// The file a tool's input names, made absolute against `cwd`.
pub fn tool_path(tool: &str, input: &Value, cwd: Option<&Path>) -> Option<PathBuf> {
    if !FILE_TOOLS.contains(&tool) {
        return None;
    }
    let key = if tool == "NotebookEdit" { "notebook_path" } else { "file_path" };
    let raw = input.get(key).or_else(|| input.get("file_path")).and_then(Value::as_str)?;
    let path = PathBuf::from(raw);
    Some(match cwd {
        Some(cwd) if path.is_relative() => cwd.join(path),
        _ => path,
    })
}

/// Read a hook's JSON input into an event (without the file).
pub fn parse(body: &Value) -> Option<HookEvent> {
    let s = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let kind = match body.get("hook_event_name").and_then(Value::as_str)? {
        "PreToolUse" => HookKind::PreTool,
        "PostToolUse" => HookKind::PostTool,
        "UserPromptSubmit" => HookKind::Prompt,
        "Stop" => HookKind::Stop,
        _ => return None,
    };
    let tool = s("tool_name");
    let input = body.get("tool_input").cloned().unwrap_or(Value::Null);
    let cwd = body.get("cwd").and_then(Value::as_str).map(PathBuf::from);
    let path = match kind {
        HookKind::PreTool | HookKind::PostTool => Some(tool_path(&tool, &input, cwd.as_deref())?),
        _ => None,
    };
    let prompt = body.get("prompt").and_then(Value::as_str).and_then(first_line);
    Some(HookEvent {
        kind,
        agent: crate::agents::CLAUDE,
        session: s("session_id"),
        prompt,
        tool,
        path,
        input,
        file: None,
    })
}

/// The first line of a prompt that says something, trimmed and cut to a tooltip's length.
pub fn first_line(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    // The files mentioned first (`@src/A.java#L3-5 rename it`) say less than the words after.
    let words: Vec<&str> = line.split_whitespace().skip_while(|w| w.starts_with('@')).collect();
    let rest = words.join(" ");
    let line = if rest.is_empty() { line } else { rest.as_str() };
    let mut out: String = line.chars().take(160).collect();
    if line.chars().count() > 160 {
        out.push('\u{2026}');
    }
    Some(out)
}

/// Read Codex's `notify` payload (the JSON it passes its notify program) into an event:
/// only `agent-turn-complete`, with the turn's prompt (the last of its input messages).
pub fn parse_codex(body: &Value) -> Option<HookEvent> {
    if body.get("type").and_then(Value::as_str)? != "agent-turn-complete" {
        return None;
    }
    let s = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let prompt = body
        .get("input-messages")
        .and_then(Value::as_array)
        .and_then(|m| m.iter().rev().find_map(Value::as_str))
        .and_then(first_line);
    let session = Some(s("thread-id")).filter(|t| !t.is_empty()).unwrap_or_else(|| s("turn-id"));
    Some(HookEvent {
        kind: HookKind::TurnComplete,
        agent: crate::agents::CODEX,
        session,
        prompt,
        tool: String::new(),
        path: None,
        input: body.clone(),
        file: None,
    })
}

/// The arguments that make a `codex` started in the console tell the endpoint when a turn
/// ends: its `notify` program set for this session only (`-c`), posting Codex's JSON (its
/// last argument) with the token from `header`. The user's own config is never touched.
pub fn codex_args(header: &Path) -> Vec<String> {
    let header = header.to_string_lossy().replace('\'', "'\\''");
    let script = format!(
        "[ -n \"${PORT_ENV}\" ] && curl -s -m 5 -o /dev/null -X POST -H @'{header}' \
         -H 'content-type: application/json' --data-binary \"$1\" \"http://127.0.0.1:${PORT_ENV}/codex\"; exit 0"
    );
    let notify = serde_json::to_string(&["/bin/sh", "-c", script.as_str(), "snout-notify"]).unwrap_or_default();
    vec!["-c".into(), format!("notify={notify}")]
}

/// Take `path` as it is now on `host`.
pub fn snapshot(host: &dyn snout_host::Host, path: &Path) -> Snapshot {
    match host.metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => Snapshot::Missing,
        Err(_) => Snapshot::Unread,
        Ok(m) if m.len > MAX_SNAPSHOT || !m.is_file() => Snapshot::Unread,
        Ok(_) => match host.read(path).map(String::from_utf8) {
            Ok(Ok(text)) => Snapshot::Text(Arc::new(text)),
            Err(e) if e.kind() == ErrorKind::NotFound => Snapshot::Missing,
            _ => Snapshot::Unread,
        },
    }
}

/// `path` as the host resolves it (the names the editor tabs go by), its folder resolved
/// for a file not there yet; as given when neither is.
pub fn resolved(host: &dyn snout_host::Host, path: &Path) -> PathBuf {
    if let Ok(p) = host.canonicalize(path) {
        return p;
    }
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => host.canonicalize(dir).map_or_else(|_| path.to_owned(), |d| d.join(name)),
        _ => path.to_owned(),
    }
}

/// The command each hook runs: post its input, quietly, and succeed whatever happens. The
/// token comes from `header` (a file holding the header line).
pub fn command(header: &Path) -> String {
    let header = header.to_string_lossy().replace('\'', "'\\''");
    format!(
        "[ -n \"${PORT_ENV}\" ] && curl -s -m 5 -o /dev/null -X POST -H @'{header}' \
         -H 'content-type: application/json' --data-binary @- \"http://127.0.0.1:${PORT_ENV}/hook\"; exit 0"
    )
}

/// The settings `claude` gets with `--settings`: the hooks, nothing else.
pub fn settings(header: &Path) -> Value {
    let hook = json!([{ "type": "command", "command": command(header), "timeout": 10 }]);
    let on_tools = json!([{ "matcher": FILE_TOOLS.join("|"), "hooks": hook }]);
    let always = json!([{ "hooks": hook }]);
    json!({ "hooks": {
        "PreToolUse": on_tools,
        "PostToolUse": on_tools,
        "UserPromptSubmit": always,
        "Stop": always,
    } })
}

/// Write the settings file and the header file into `dir` on `host`, readable by this
/// user only. Returns the settings file.
pub fn write_settings(host: &dyn snout_host::Host, dir: &Path, token: &str) -> std::io::Result<PathBuf> {
    host.create_dir_all_mode(dir, Some(0o700))?;
    let header = dir.join("hook-header");
    host.write_mode(&header, format!("{TOKEN_HEADER}: {token}\n").as_bytes(), Some(0o600))?;
    let file = dir.join("claude-settings.json");
    let text = serde_json::to_string_pretty(&settings(&header)).map_err(std::io::Error::other)?;
    host.write_mode(&file, text.as_bytes(), Some(0o600))?;
    Ok(file)
}

type Emit = Arc<dyn Fn(HookEvent) + Send + Sync>;

/// The endpoint. Dropping it stops it.
pub struct HookServer {
    port: u16,
    token: String,
    stop: Arc<AtomicBool>,
}

impl HookServer {
    /// Listen on 127.0.0.1; `emit` hears each hook, with the file read on `host`.
    pub fn start(host: HostRef, emit: impl Fn(HookEvent) + Send + Sync + 'static) -> std::io::Result<Self> {
        let token = lockfile::new_token()?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let (s, t, emit): (_, _, Emit) = (stop.clone(), token.clone(), Arc::new(emit));
        std::thread::Builder::new().name("claude-hooks".into()).spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (h, t, e) = (host.clone(), t.clone(), emit.clone());
                        let _ = std::thread::Builder::new()
                            .name("claude-hook".into())
                            .spawn(move || serve(stream, &*h, &t, &e));
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        })?;
        Ok(Self { port, token, stop })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

impl Drop for HookServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Equal strings, compared in time that does not depend on where they differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A request: its method, path, headers (names lowercased) and body.
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

/// Read one HTTP/1.1 request.
pub fn read_request(stream: impl std::io::Read) -> std::io::Result<Request> {
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or_default().to_owned(), parts.next().unwrap_or_default().to_owned());
    let mut headers = Vec::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            break;
        }
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        if headers.len() > 64 {
            return Err(std::io::Error::other("too many headers"));
        }
        if let Some((n, v)) = l.split_once(':') {
            headers.push((n.trim().to_ascii_lowercase(), v.trim().to_owned()));
        }
    }
    let len: usize = headers.iter().find(|(n, _)| n == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    if len > MAX_BODY {
        return Err(std::io::Error::other("too large"));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    Ok(Request { method, path, headers, body })
}

fn respond(stream: &mut TcpStream, status: &str) {
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let _ = stream.flush();
}

fn serve(mut stream: TcpStream, host: &dyn snout_host::Host, token: &str, emit: &Emit) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(req) = stream.try_clone().and_then(read_request) else {
        return respond(&mut stream, "400 Bad Request");
    };
    if !req.header(TOKEN_HEADER).is_some_and(|t| same(t, token)) {
        return respond(&mut stream, "401 Unauthorized");
    }
    if req.method != "POST" || (req.path != "/hook" && req.path != "/codex") {
        return respond(&mut stream, "404 Not Found");
    }
    let body = serde_json::from_slice::<Value>(&req.body).ok();
    let event = match req.path.as_str() {
        "/codex" => body.as_ref().and_then(parse_codex),
        _ => body.as_ref().and_then(parse),
    };
    if let Some(mut event) = event {
        if let Some(path) = event.path.take() {
            let path = resolved(host, &path);
            event.file = Some(snapshot(host, &path));
            event.path = Some(path);
        }
        emit(event);
    }
    respond(&mut stream, "204 No Content");
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::{Mutex, mpsc};

    /// Post `body` as the hook command would, presenting `token`.
    pub fn post(port: u16, token: &str, body: &Value) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let body = body.to_string();
        write!(
            s,
            "POST /hook HTTP/1.1\r\nHost: 127.0.0.1\r\n{TOKEN_HEADER}: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snout-hooks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn hook_input_names_the_file_of_each_tool() {
        let edit = json!({ "hook_event_name": "PreToolUse", "session_id": "s1", "cwd": "/p", "tool_name": "Edit",
            "tool_input": { "file_path": "src/A.java", "old_string": "a", "new_string": "b" } });
        let e = parse(&edit).unwrap();
        assert_eq!((e.kind, e.session.as_str(), e.tool.as_str()), (HookKind::PreTool, "s1", "Edit"));
        assert_eq!(e.path, Some(PathBuf::from("/p/src/A.java")), "relative to the session's folder");
        let nb = json!({ "hook_event_name": "PostToolUse", "tool_name": "NotebookEdit",
            "tool_input": { "notebook_path": "/p/n.ipynb" } });
        assert_eq!(parse(&nb).unwrap().path, Some(PathBuf::from("/p/n.ipynb")));
        let bash = json!({ "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": { "command": "ls" } });
        assert!(parse(&bash).is_none(), "only the file tools");
        let stop = json!({ "hook_event_name": "Stop", "session_id": "s1" });
        assert_eq!(parse(&stop).unwrap().kind, HookKind::Stop);
        assert!(parse(&json!({ "hook_event_name": "Notification" })).is_none());
    }

    #[test]
    fn the_settings_hook_the_file_tools_and_keep_the_token_off_the_command_line() {
        let v = settings(Path::new("/tmp/x y/hook-header"));
        let pre = &v["hooks"]["PreToolUse"][0];
        assert_eq!(pre["matcher"], "Edit|MultiEdit|Write|NotebookEdit");
        let cmd = pre["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("-H @'/tmp/x y/hook-header'"), "{cmd}");
        assert!(cmd.ends_with("exit 0"), "the hook never blocks Claude Code: {cmd}");
        assert!(v["hooks"]["UserPromptSubmit"][0].get("matcher").is_none());
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], cmd);
    }

    #[test]
    fn the_endpoint_takes_hooks_with_the_token_and_snapshots_the_file() {
        let dir = scratch("endpoint");
        let file = dir.join("A.java");
        std::fs::write(&file, "class A {}\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let server = HookServer::start(snout_host::local(), move |e| tx.lock().unwrap().send(e).unwrap()).unwrap();
        let pre = json!({ "hook_event_name": "PreToolUse", "session_id": "s", "tool_name": "Write",
            "tool_input": { "file_path": file, "content": "class A { int x; }\n" } });
        assert!(post(server.port(), "wrong", &pre).starts_with("HTTP/1.1 401"));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err(), "nothing heard without the token");
        assert!(post(server.port(), server.token(), &pre).starts_with("HTTP/1.1 204"));
        let e = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(e.kind, HookKind::PreTool);
        assert_eq!(e.file, Some(Snapshot::Text(Arc::new("class A {}\n".into()))), "read before answering");
        let fresh = json!({ "hook_event_name": "PreToolUse", "tool_name": "Write",
            "tool_input": { "file_path": dir.join("New.java") } });
        post(server.port(), server.token(), &fresh);
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().file, Some(Snapshot::Missing));

        // The hook command itself, as Claude Code runs it: through the shell, with curl.
        let settings = write_settings(&*snout_host::local(), &dir.join("conf"), server.token()).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(std::fs::metadata(dir.join("conf/hook-header")).unwrap().permissions().mode() & 0o777, 0o600);
        let cmd = v["hooks"]["PostToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_owned();
        let post_input = json!({ "hook_event_name": "PostToolUse", "session_id": "s", "tool_name": "Edit",
            "tool_input": { "file_path": file } });
        let run = |env: Option<u16>| {
            let mut c = std::process::Command::new("/bin/sh");
            c.arg("-c").arg(&cmd).stdin(std::process::Stdio::piped());
            match env {
                Some(port) => c.env(PORT_ENV, port.to_string()),
                None => c.env_remove(PORT_ENV),
            };
            let mut child = c.spawn().unwrap();
            // Without the app the command may end before it reads its input.
            let _ = child.stdin.take().unwrap().write_all(post_input.to_string().as_bytes());
            child.wait_with_output().unwrap()
        };
        let out = run(Some(server.port()));
        assert!(out.status.success() && out.stdout.is_empty(), "silent and successful");
        let e = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((e.kind, e.path), (HookKind::PostTool, Some(file.clone())));
        // Without the app (no port, or nobody listening) the hook still succeeds.
        assert!(run(None).status.success());
        drop(server);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Codex's `notify` payload, as recorded from Codex 0.5x (`agent-turn-complete`).
    const CODEX_NOTIFY: &str = include_str!("../../tests/fixtures/agents/codex-notify.json");

    #[test]
    fn codex_tells_the_end_of_its_turn_and_its_prompt() {
        let v: Value = serde_json::from_str(CODEX_NOTIFY).unwrap();
        let e = parse_codex(&v).unwrap();
        assert_eq!((e.kind, e.agent), (HookKind::TurnComplete, crate::agents::CODEX));
        assert_eq!(
            e.prompt.as_deref(),
            Some("Add input validation to OrderService.place: reject a null order and an empty list of items.")
        );
        assert!(parse_codex(&json!({ "type": "approval-requested" })).is_none());
        // The prompt of a Claude turn too, from UserPromptSubmit.
        let p = parse(&json!({ "hook_event_name": "UserPromptSubmit", "prompt": "\n  Rename greet\nplease" })).unwrap();
        assert_eq!(p.prompt.as_deref(), Some("Rename greet"));
        assert_eq!(first_line("@src/A.java#L3-5 make it faster").as_deref(), Some("make it faster"));
    }

    #[test]
    fn codex_notify_reaches_the_endpoint_as_codex_runs_it() {
        let dir = scratch("codex");
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let server = HookServer::start(snout_host::local(), move |e| tx.lock().unwrap().send(e).unwrap()).unwrap();
        write_settings(&*snout_host::local(), &dir, server.token()).unwrap();
        let args = codex_args(&dir.join("hook-header"));
        assert_eq!(args[0], "-c");
        // Codex reads `-c notify=<TOML array>`; a JSON array of strings is one.
        let notify: Vec<String> = serde_json::from_str(args[1].strip_prefix("notify=").unwrap()).unwrap();
        // Codex runs the program with its JSON as the last argument.
        let out = std::process::Command::new(&notify[0])
            .args(&notify[1..])
            .arg(CODEX_NOTIFY)
            .env(PORT_ENV, server.port().to_string())
            .output()
            .unwrap();
        assert!(out.status.success() && out.stdout.is_empty(), "silent and successful");
        let e = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(e.kind, HookKind::TurnComplete);
        assert!(e.prompt.unwrap().starts_with("Add input validation"));
        drop(server);
        let _ = std::fs::remove_dir_all(dir);
    }
}
