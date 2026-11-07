//! Precise attribution: the agents report every file edit to the app, before and after.
//! Claude Code's hooks report them whether it asked first (`openDiff`) or not (edits it
//! accepts on its own never reach the IDE server). A `claude` the console starts gets
//! extra settings (`--settings <file>`, the user's own settings are never touched) whose
//! `PreToolUse` and `PostToolUse` hooks on the file tools, `UserPromptSubmit` and `Stop`
//! post the hook's input to a small HTTP endpoint here: 127.0.0.1 only, on a port the
//! system picks, each request checked against a token the hook sends in a header read from
//! a file only this user can read (so the token never shows on a command line).
//!
//! The other agents report the same way: Codex through hooks of its own, set for the
//! session with `-c` (they post to `/hook/codex`; its `apply_patch` names the files in the
//! patch), opencode and Amp through a small plugin ([`OPENCODE_PLUGIN`], [`AMP_PLUGIN`])
//! that posts to `/agent`.
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
/// The tool that changes files with a patch (Codex's, opencode's and Amp's): the files are
/// named in it.
pub const PATCH_TOOL: &str = "apply_patch";
/// The plugin opencode loads for a session in the console ([`opencode_env`]).
pub const OPENCODE_PLUGIN: &str = include_str!("agent-plugins/opencode.js");
/// The plugin Amp loads from its plugins folder ([`amp_plugin_path`]); it does nothing in
/// an Amp outside the console.
pub const AMP_PLUGIN: &str = include_str!("agent-plugins/amp.js");
/// The plugin's file name in Amp's plugins folder.
pub const AMP_PLUGIN_FILE: &str = "snoutcode.js";
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
    /// An agent finished a turn (Codex's `notify`, Amp's `agent.end`), with the prompt
    /// that started it.
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
    /// The agent that reported it ([`crate::agents::CLAUDE`], [`crate::agents::CODEX`], ...).
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

/// The files a patch names (`*** Add File:`, `*** Update File:`, `*** Delete File:` and
/// the new name of `*** Move to:`), made absolute against `cwd`, each once.
pub fn patch_paths(patch: &str, cwd: Option<&Path>) -> Vec<PathBuf> {
    const MARKS: [&str; 4] = ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"];
    let mut out: Vec<PathBuf> = Vec::new();
    for line in patch.lines() {
        let line = line.trim();
        let Some(raw) = MARKS.iter().find_map(|m| line.strip_prefix(m)).map(str::trim).filter(|r| !r.is_empty()) else {
            continue;
        };
        let path = absolute(Path::new(raw), cwd);
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

fn absolute(path: &Path, cwd: Option<&Path>) -> PathBuf {
    match cwd {
        Some(cwd) if path.is_relative() => cwd.join(path),
        _ => path.to_owned(),
    }
}

/// Read a hook's JSON input into an event (without the file).
pub fn parse(body: &Value) -> Option<HookEvent> {
    parse_hook(body, crate::agents::CLAUDE).into_iter().next()
}

/// Read the JSON input of a hook of `agent` (Claude Code's, or Codex's, which has the same
/// shape) into its events, without the files: one per file a tool changes (a patch can
/// change several), one for a prompt or the end of an answer.
pub fn parse_hook(body: &Value, agent: &'static str) -> Vec<HookEvent> {
    let s = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let kind = match body.get("hook_event_name").and_then(Value::as_str) {
        Some("PreToolUse") => HookKind::PreTool,
        Some("PostToolUse") => HookKind::PostTool,
        Some("UserPromptSubmit") => HookKind::Prompt,
        Some("Stop") => HookKind::Stop,
        _ => return Vec::new(),
    };
    let tool = s("tool_name");
    let input = body.get("tool_input").cloned().unwrap_or(Value::Null);
    let cwd = body.get("cwd").and_then(Value::as_str).map(PathBuf::from);
    let paths: Vec<Option<PathBuf>> = match kind {
        HookKind::PreTool | HookKind::PostTool if tool == PATCH_TOOL => {
            let patch = input.get("command").or_else(|| input.get("patch")).and_then(Value::as_str);
            patch_paths(patch.unwrap_or_default(), cwd.as_deref()).into_iter().map(Some).collect()
        }
        HookKind::PreTool | HookKind::PostTool => {
            tool_path(&tool, &input, cwd.as_deref()).into_iter().map(Some).collect()
        }
        _ => vec![None],
    };
    let prompt = body.get("prompt").and_then(Value::as_str).and_then(first_line);
    let session = s("session_id");
    paths
        .into_iter()
        .map(|path| HookEvent {
            kind: kind.clone(),
            agent,
            session: session.clone(),
            prompt: prompt.clone(),
            tool: tool.clone(),
            path,
            input: input.clone(),
            file: None,
        })
        .collect()
}

/// The agent a plugin's report names, as the app knows it.
fn plugin_agent(name: &str) -> Option<&'static str> {
    match name {
        crate::agents::OPENCODE => Some(crate::agents::OPENCODE),
        crate::agents::AMP => Some(crate::agents::AMP),
        _ => None,
    }
}

/// The files a tool of a plugin's agent changes, from its input: the files the plugin
/// worked out itself (Amp's own reading of its tool call), else the tool's `filePath` or
/// `path`, or the files its patch names.
fn plugin_paths(tool: &str, input: &Value, files: Option<&Value>, cwd: Option<&Path>) -> Vec<PathBuf> {
    if let Some(files) = files.and_then(Value::as_array) {
        let mut out: Vec<PathBuf> = Vec::new();
        for f in files.iter().filter_map(Value::as_str).filter(|f| !f.is_empty()) {
            let path = absolute(Path::new(f), cwd);
            if !out.contains(&path) {
                out.push(path);
            }
        }
        return out;
    }
    if matches!(tool, PATCH_TOOL | "patch") {
        let patch = input.get("patchText").or_else(|| input.get("patch")).and_then(Value::as_str);
        return patch_paths(patch.unwrap_or_default(), cwd);
    }
    match tool {
        "edit" | "write" | "multiedit" | "edit_file" | "create_file" | "delete_file" => input
            .get("filePath")
            .or_else(|| input.get("path"))
            .and_then(Value::as_str)
            .map(|p| absolute(Path::new(p), cwd))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// Read the report of SnoutCode's plugin in an agent (opencode, Amp) into its events,
/// without the files:
///
/// ```json
/// { "agent": "opencode", "event": "pre", "session": "ses_1", "tool": "edit",
///   "input": { "filePath": "/p/A.java", ... }, "cwd": "/p" }
/// ```
///
/// `event` is `pre` or `post` (a tool, with `tool`, `input` and maybe `files`), `prompt`
/// (with `prompt`), `stop`, or `end` (a turn ended, with its `prompt`).
pub fn parse_agent(body: &Value) -> Vec<HookEvent> {
    let Some(agent) = body.get("agent").and_then(Value::as_str).and_then(plugin_agent) else { return Vec::new() };
    let s = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let kind = match body.get("event").and_then(Value::as_str) {
        Some("pre") => HookKind::PreTool,
        Some("post") => HookKind::PostTool,
        Some("prompt") => HookKind::Prompt,
        Some("stop") => HookKind::Stop,
        Some("end") => HookKind::TurnComplete,
        _ => return Vec::new(),
    };
    let tool = s("tool");
    let input = body.get("input").cloned().unwrap_or(Value::Null);
    let cwd = body.get("cwd").and_then(Value::as_str).filter(|c| !c.is_empty()).map(PathBuf::from);
    let paths: Vec<Option<PathBuf>> = match kind {
        HookKind::PreTool | HookKind::PostTool => {
            plugin_paths(&tool, &input, body.get("files"), cwd.as_deref()).into_iter().map(Some).collect()
        }
        _ => vec![None],
    };
    let prompt = body.get("prompt").and_then(Value::as_str).and_then(first_line);
    let session = s("session");
    paths
        .into_iter()
        .map(|path| HookEvent {
            kind: kind.clone(),
            agent,
            session: session.clone(),
            prompt: prompt.clone(),
            tool: tool.clone(),
            path,
            input: input.clone(),
            file: None,
        })
        .collect()
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
/// last argument) with the token from `header` to `port`. The user's own config is never
/// touched. On a Unix host through `sh` (the port from the environment); on Windows (a
/// `header` with a drive) Windows's own `curl.exe` directly, the JSON the value of its last
/// option.
pub fn codex_args(header: &Path, port: u16) -> Vec<String> {
    let windows = matches!(header.components().next(), Some(std::path::Component::Prefix(_)));
    let notify = if windows {
        let url = format!("http://127.0.0.1:{port}/codex");
        let header = format!("@{}", header.display());
        serde_json::to_string(&[
            "curl.exe",
            "-s",
            "-m",
            "5",
            "-o",
            "NUL",
            "-X",
            "POST",
            "-H",
            header.as_str(),
            "-H",
            "content-type: application/json",
            "--url",
            url.as_str(),
            "--data-binary",
        ])
    } else {
        let header = header.to_string_lossy().replace('\'', "'\\''");
        let script = format!(
            "[ -n \"${PORT_ENV}\" ] && curl -s -m 5 -o /dev/null -X POST -H @'{header}' \
             -H 'content-type: application/json' --data-binary \"$1\" \"http://127.0.0.1:${PORT_ENV}/codex\"; exit 0"
        );
        serde_json::to_string(&["/bin/sh", "-c", script.as_str(), "snout-notify"])
    };
    let mut args = vec!["-c".into(), format!("notify={}", notify.unwrap_or_default())];
    args.extend(codex_hook_args(windows));
    args
}

/// The command each of Codex's hooks runs: post its input to `/hook/codex`, quietly, and
/// succeed whatever happens. It names no path or port of its own (both come from the
/// session's variables), so it reads the same in every window: Codex asks the user to
/// trust a hook once, by its text, and then runs it in every session.
pub fn codex_hook_command(windows: bool) -> String {
    if windows {
        format!(
            "curl.exe -s -m 5 -o NUL -X POST -H @\"%{h}%\" -H \"content-type: application/json\" \
             --data-binary @- \"http://127.0.0.1:%{PORT_ENV}%/hook/codex\" & exit /b 0",
            h = super::api::HEADER_ENV
        )
    } else {
        format!(
            "/bin/sh -c '[ -n \"${PORT_ENV}\" ] && [ -n \"${h}\" ] && curl -s -m 5 -o /dev/null -X POST \
             -H @\"${h}\" -H \"content-type: application/json\" --data-binary @- \
             \"http://127.0.0.1:${PORT_ENV}/hook/codex\"; exit 0'",
            h = super::api::HEADER_ENV
        )
    }
}

/// The `-c` options that set Codex's hooks for the session: `PreToolUse` and `PostToolUse`
/// on `apply_patch` (every file edit), `UserPromptSubmit` and `Stop`. The values are TOML;
/// a JSON string is a TOML string.
pub fn codex_hook_args(windows: bool) -> Vec<String> {
    let command = serde_json::to_string(&codex_hook_command(windows)).unwrap_or_default();
    let handler = format!("{{type = \"command\", command = {command}, timeout = 10}}");
    let on_patch = format!("[{{matcher = \"^{PATCH_TOOL}$\", hooks = [{handler}]}}]");
    let always = format!("[{{hooks = [{handler}]}}]");
    [("PreToolUse", &on_patch), ("PostToolUse", &on_patch), ("UserPromptSubmit", &always), ("Stop", &always)]
        .into_iter()
        .flat_map(|(event, value)| ["-c".to_owned(), format!("hooks.{event}={value}")])
        .collect()
}

/// The variable that makes an `opencode` started in the console load SnoutCode's plugin
/// `plugin` for the session, beside the user's own (opencode merges its plugin lists).
pub fn opencode_env(plugin: &Path) -> (&'static str, String) {
    let url = if matches!(plugin.components().next(), Some(std::path::Component::Prefix(_))) {
        format!("file:///{}", plugin.to_string_lossy().replace('\\', "/"))
    } else {
        format!("file://{}", plugin.to_string_lossy())
    };
    ("OPENCODE_CONFIG_CONTENT", json!({ "plugin": [url] }).to_string())
}

/// Where Amp looks for the plugins of this user on `host`:
/// `$XDG_CONFIG_HOME/amp/plugins`, else `~/.config/amp/plugins` (on Windows too).
pub fn amp_plugin_dir(host: &dyn snout_host::Host) -> Option<PathBuf> {
    let info = host.info();
    let config = info
        .var("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| (!info.home.as_os_str().is_empty()).then(|| info.home.join(".config")))?;
    Some(config.join("amp").join("plugins"))
}

/// Put SnoutCode's plugin into Amp's plugins folder on `host`, unless it is there as it
/// is. It does nothing in an Amp started outside SnoutCode's console. Returns its path.
pub fn install_amp_plugin(host: &dyn snout_host::Host) -> std::io::Result<PathBuf> {
    let dir = amp_plugin_dir(host).ok_or_else(|| std::io::Error::other("no home folder"))?;
    let file = dir.join(AMP_PLUGIN_FILE);
    if host.read(&file).is_ok_and(|t| t == AMP_PLUGIN.as_bytes()) {
        return Ok(file);
    }
    host.create_dir_all_mode(&dir, None)?;
    host.write_mode(&file, AMP_PLUGIN.as_bytes(), Some(0o644))?;
    Ok(file)
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

/// The settings `claude` gets with `--settings`: the hooks, and leave to run the editor's
/// API script `api` (the skill's calls) without asking.
pub fn settings(header: &Path, api: &Path) -> Value {
    let hook = json!([{ "type": "command", "command": command(header), "timeout": 10 }]);
    let on_tools = json!([{ "matcher": FILE_TOOLS.join("|"), "hooks": hook }]);
    let always = json!([{ "hooks": hook }]);
    json!({
        "hooks": {
            "PreToolUse": on_tools,
            "PostToolUse": on_tools,
            "UserPromptSubmit": always,
            "Stop": always,
        },
        "permissions": { "allow": [format!("Bash({}:*)", api.to_string_lossy())] },
    })
}

/// Write the settings file, the header file and the editor's plugin ([`super::api`]) into
/// `dir` on `host`, readable by this user only. Returns the settings file.
pub fn write_settings(host: &dyn snout_host::Host, dir: &Path, token: &str) -> std::io::Result<PathBuf> {
    host.create_dir_all_mode(dir, Some(0o700))?;
    let header = dir.join("hook-header");
    host.write_mode(&header, format!("{TOKEN_HEADER}: {token}\n").as_bytes(), Some(0o600))?;
    super::api::write_plugin(host, dir)?;
    host.write_mode(&opencode_plugin(dir), OPENCODE_PLUGIN.as_bytes(), Some(0o600))?;
    let file = dir.join("claude-settings.json");
    let text =
        serde_json::to_string_pretty(&settings(&header, &super::api::script(dir))).map_err(std::io::Error::other)?;
    host.write_mode(&file, text.as_bytes(), Some(0o600))?;
    Ok(file)
}

/// SnoutCode's opencode plugin in the folder `dir` of [`write_settings`].
pub fn opencode_plugin(dir: &Path) -> PathBuf {
    dir.join("snoutcode-opencode.js")
}

type Emit = Arc<dyn Fn(HookEvent) + Send + Sync>;
type Ask = Arc<dyn Fn(super::server::Call) + Send + Sync>;

/// How long an API call waits for the editor's answer.
const API_WAIT: Duration = Duration::from_secs(30);

/// The endpoint. Dropping it stops it.
pub struct HookServer {
    port: u16,
    token: String,
    stop: Arc<AtomicBool>,
}

impl HookServer {
    /// Listen on 127.0.0.1; `emit` hears each hook, with the file read on `host`, and
    /// `ask` each call of the editor's API ([`super::api`]), to answer through its reply.
    pub fn start(
        host: HostRef,
        emit: impl Fn(HookEvent) + Send + Sync + 'static,
        ask: impl Fn(super::server::Call) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let token = lockfile::new_token()?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let (s, t, emit, ask): (_, _, Emit, Ask) = (stop.clone(), token.clone(), Arc::new(emit), Arc::new(ask));
        std::thread::Builder::new().name("claude-hooks".into()).spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (h, t, e, a) = (host.clone(), t.clone(), emit.clone(), ask.clone());
                        let _ = std::thread::Builder::new()
                            .name("claude-hook".into())
                            .spawn(move || serve(stream, &*h, &t, &e, &a));
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

fn respond_with(stream: &mut TcpStream, a: &super::api::Answer) {
    let _ = write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        a.status,
        a.content_type,
        a.body.len(),
        a.body
    );
    let _ = stream.flush();
}

/// A call of the editor's API: `GET /utcp` (the manual) or `POST /api/<tool>`.
fn serve_api(stream: &mut TcpStream, req: &Request, ask: &Ask) {
    use super::api::{self, Answer};
    let (path, query) = req.path.split_once('?').unwrap_or((req.path.as_str(), ""));
    if path == "/utcp" {
        let body = serde_json::to_string_pretty(&api::manual()).unwrap_or_default();
        return respond_with(stream, &Answer { status: "200 OK", content_type: "application/json", body });
    }
    let Some(name) = path.strip_prefix("/api/").filter(|n| super::capabilities::find(n).is_some()) else {
        return respond_with(stream, &Answer::error("404 Not Found", format!("no such call: {path}")));
    };
    if req.method != "POST" {
        return respond_with(stream, &Answer::error("405 Method Not Allowed", "use POST"));
    }
    let arguments = match api::arguments(name, query, &req.body) {
        Ok(a) => a,
        Err(e) => return respond_with(stream, &Answer::error("400 Bad Request", e)),
    };
    let (reply, answer) = super::server::Reply::channel();
    ask(super::server::Call { client: super::server::API_CLIENT, name: name.to_owned(), arguments, reply });
    let a = match answer.recv_timeout(API_WAIT) {
        Ok(message) => api::answer(&message),
        Err(_) => Answer::error("504 Gateway Timeout", "the editor did not answer"),
    };
    respond_with(stream, &a);
}

fn serve(mut stream: TcpStream, host: &dyn snout_host::Host, token: &str, emit: &Emit, ask: &Ask) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(req) = stream.try_clone().and_then(read_request) else {
        return respond(&mut stream, "400 Bad Request");
    };
    if !req.header(TOKEN_HEADER).is_some_and(|t| same(t, token)) {
        return respond(&mut stream, "401 Unauthorized");
    }
    if req.path == "/utcp" || req.path.starts_with("/utcp?") || req.path.starts_with("/api/") {
        return serve_api(&mut stream, &req, ask);
    }
    if req.method != "POST" || !matches!(req.path.as_str(), "/hook" | "/hook/codex" | "/codex" | "/agent") {
        return respond(&mut stream, "404 Not Found");
    }
    let Some(body) = serde_json::from_slice::<Value>(&req.body).ok() else {
        return respond(&mut stream, "204 No Content");
    };
    let events = match req.path.as_str() {
        "/codex" => parse_codex(&body).into_iter().collect(),
        "/hook/codex" => parse_hook(&body, crate::agents::CODEX),
        "/agent" => parse_agent(&body),
        _ => parse_hook(&body, crate::agents::CLAUDE),
    };
    for mut event in events {
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
        snout_host::canonical(&dir).unwrap()
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
        let v = settings(Path::new("/tmp/x y/hook-header"), Path::new("/tmp/x y/snout-plugin/bin/snout-api"));
        assert_eq!(v["permissions"]["allow"], json!(["Bash(/tmp/x y/snout-plugin/bin/snout-api:*)"]));
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
        let server =
            HookServer::start(snout_host::local(), move |e| tx.lock().unwrap().send(e).unwrap(), |_| {}).unwrap();
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.join("conf/hook-header")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let cmd = v["hooks"]["PostToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_owned();
        // Claude Code runs hooks with `sh`; on Windows with Git for Windows's bash.
        let shell = if cfg!(windows) { r"C:\Program Files\Git\bin\bash.exe" } else { "/bin/sh" };
        if !Path::new(shell).exists() {
            return;
        }
        let post_input = json!({ "hook_event_name": "PostToolUse", "session_id": "s", "tool_name": "Edit",
            "tool_input": { "file_path": file } });
        let run = |env: Option<u16>| {
            let mut c = std::process::Command::new(shell);
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
        let server =
            HookServer::start(snout_host::local(), move |e| tx.lock().unwrap().send(e).unwrap(), |_| {}).unwrap();
        write_settings(&*snout_host::local(), &dir, server.token()).unwrap();
        let args = codex_args(&dir.join("hook-header"), server.port());
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

    /// Post `body` to `path` of the endpoint, presenting `token`.
    pub fn post_to(port: u16, token: &str, path: &str, body: &Value) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let body = body.to_string();
        write!(
            s,
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{TOKEN_HEADER}: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }

    /// A Codex `PreToolUse` hook's input for an `apply_patch`, shaped as Codex's schema
    /// (`pre-tool-use.command.input`) has it: the patch is the tool input's `command`.
    const CODEX_PATCH: &str = include_str!("../../tests/fixtures/agents/codex-hook-apply-patch.json");

    #[test]
    fn a_patch_names_its_files() {
        let patch = "*** Begin Patch\n*** Add File: a/New.java\n+x\n*** Update File: /abs/B.java\n@@\n-a\n+b\n\
                     *** Delete File: c.txt\n*** Update File: d.md\n*** Move to: e/d.md\n*** Update File: a/New.java\n*** End Patch";
        assert_eq!(
            patch_paths(patch, Some(Path::new("/p"))),
            ["/p/a/New.java", "/abs/B.java", "/p/c.txt", "/p/d.md", "/p/e/d.md"].map(PathBuf::from).to_vec(),
            "relative to the folder, each once, the new name of a move too"
        );
        assert!(patch_paths("no patch here", None).is_empty());
    }

    #[test]
    fn codex_hooks_report_each_file_of_a_patch() {
        let v: Value = serde_json::from_str(CODEX_PATCH).unwrap();
        let events = parse_hook(&v, crate::agents::CODEX);
        let paths: Vec<_> = events.iter().filter_map(|e| e.path.clone()).collect();
        assert_eq!(
            paths,
            [
                "/home/alex/projects/shop/src/main/java/shop/OrderService.java",
                "/home/alex/projects/shop/src/test/java/shop/OrderServiceTest.java",
                "/home/alex/projects/shop/README.md",
                "/home/alex/projects/shop/docs/README.md",
            ]
            .map(PathBuf::from)
        );
        assert!(events.iter().all(|e| e.kind == HookKind::PreTool && e.agent == crate::agents::CODEX));
        assert_eq!(events[0].tool, "apply_patch");
        let prompt = json!({ "hook_event_name": "UserPromptSubmit", "session_id": "s", "prompt": "Add a test" });
        let e = parse_hook(&prompt, crate::agents::CODEX);
        assert_eq!((e[0].kind.clone(), e[0].prompt.as_deref()), (HookKind::Prompt, Some("Add a test")));
        let shell = json!({ "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": { "command": "ls" } });
        assert!(parse_hook(&shell, crate::agents::CODEX).is_empty(), "the watcher sees what a shell command changes");
    }

    #[test]
    fn codex_gets_its_hooks_as_toml_with_one_text_for_every_window() {
        let args = codex_hook_args(false);
        assert_eq!(args.len(), 8);
        let command = codex_hook_command(false);
        assert!(!command.contains("127.0.0.1:1") && !command.contains("/tmp"), "no port or path of its own: {command}");
        for pair in args.chunks(2) {
            assert_eq!(pair[0], "-c");
            let (key, value) = pair[1].split_once('=').unwrap();
            let event = key.strip_prefix("hooks.").unwrap();
            // Codex reads `-c key=value` with the value as TOML.
            let table: toml::Table = toml::from_str(&format!("v = {value}")).unwrap();
            let group = &table["v"].as_array().unwrap()[0];
            let handler = &group["hooks"].as_array().unwrap()[0];
            assert_eq!(handler["type"].as_str(), Some("command"));
            assert_eq!(handler["command"].as_str(), Some(command.as_str()));
            assert_eq!(handler["timeout"].as_integer(), Some(10));
            let matcher = group.get("matcher").and_then(|m| m.as_str());
            match event {
                "PreToolUse" | "PostToolUse" => assert_eq!(matcher, Some("^apply_patch$")),
                "UserPromptSubmit" | "Stop" => assert_eq!(matcher, None),
                other => panic!("unexpected hook {other}"),
            }
        }
        let all = codex_args(Path::new("/tmp/x/hook-header"), 4100);
        assert!(all[1].starts_with("notify="), "notify stays, for a Codex whose hooks are off");
        assert_eq!(&all[2..], &args[..]);
    }

    #[test]
    fn the_codex_hook_reaches_the_endpoint_as_codex_runs_it() {
        if !Path::new("/bin/sh").exists() {
            return;
        }
        let dir = scratch("codex-hook");
        let file = dir.join("A.java");
        std::fs::write(&file, "class A {}\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let server =
            HookServer::start(snout_host::local(), move |e| tx.lock().unwrap().send(e).unwrap(), |_| {}).unwrap();
        write_settings(&*snout_host::local(), &dir.join("conf"), server.token()).unwrap();
        let input = json!({ "hook_event_name": "PreToolUse", "session_id": "s", "cwd": dir, "tool_name": "apply_patch",
            "tool_input": { "command": "*** Begin Patch\n*** Update File: A.java\n@@\n-class A {}\n+class A { int x; }\n*** End Patch" } });
        let run = |port: Option<u16>| {
            // Codex runs a hook with the user's shell, `-lc`; the command starts its own sh.
            let mut c = std::process::Command::new("/bin/sh");
            c.arg("-c").arg(codex_hook_command(false)).stdin(std::process::Stdio::piped());
            c.env(super::super::api::HEADER_ENV, dir.join("conf/hook-header"));
            match port {
                Some(port) => c.env(PORT_ENV, port.to_string()),
                None => c.env_remove(PORT_ENV),
            };
            let mut child = c.spawn().unwrap();
            let _ = child.stdin.take().unwrap().write_all(input.to_string().as_bytes());
            child.wait_with_output().unwrap()
        };
        let out = run(Some(server.port()));
        assert!(out.status.success() && out.stdout.is_empty(), "silent and successful");
        let e = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((e.kind, e.agent, e.path), (HookKind::PreTool, crate::agents::CODEX, Some(file.clone())));
        assert_eq!(e.file, Some(Snapshot::Text(Arc::new("class A {}\n".into()))));
        assert!(run(None).status.success(), "outside SnoutCode's console it does nothing, and succeeds");
        drop(server);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_plugins_reports_name_their_files() {
        let edit = json!({ "agent": "opencode", "event": "pre", "session": "ses_1", "tool": "edit", "cwd": "/p",
            "input": { "filePath": "/p/A.java", "oldString": "a", "newString": "b" } });
        let e = parse_agent(&edit);
        assert_eq!(
            (e[0].kind.clone(), e[0].agent, e[0].path.clone()),
            (HookKind::PreTool, crate::agents::OPENCODE, Some(PathBuf::from("/p/A.java")))
        );
        let patch = json!({ "agent": "opencode", "event": "post", "tool": "apply_patch", "cwd": "/p",
            "input": { "patchText": "*** Begin Patch\n*** Add File: B.java\n+x\n*** Delete File: C.java\n*** End Patch" } });
        let paths: Vec<_> = parse_agent(&patch).into_iter().filter_map(|e| e.path).collect();
        assert_eq!(paths, [PathBuf::from("/p/B.java"), PathBuf::from("/p/C.java")]);
        let amp = json!({ "agent": "amp", "event": "post", "tool": "Bash", "input": { "cmd": "echo x > out.txt" },
            "files": ["/p/out.txt", "/p/out.txt"] });
        let e = parse_agent(&amp);
        assert_eq!(e.len(), 1, "the files Amp worked out, each once");
        assert_eq!((e[0].agent, e[0].path.clone()), (crate::agents::AMP, Some(PathBuf::from("/p/out.txt"))));
        let end = parse_agent(&json!({ "agent": "amp", "event": "end", "prompt": "Rename greet" }));
        assert_eq!((end[0].kind.clone(), end[0].prompt.as_deref()), (HookKind::TurnComplete, Some("Rename greet")));
        assert!(parse_agent(&json!({ "agent": "claude", "event": "stop" })).is_empty(), "only the plugins' agents");
        assert!(
            parse_agent(
                &json!({ "agent": "opencode", "event": "pre", "tool": "read", "input": { "filePath": "/p/A" } })
            )
            .is_empty()
        );
    }

    /// Run `driver` (an ES module) with Node, the plugin `plugin` beside it, in `dir`; `None`
    /// without Node.
    fn node(dir: &Path, plugin: &str, driver: &str, env: &[(&str, String)]) -> Option<std::process::Output> {
        std::fs::write(dir.join("plugin.mjs"), plugin).unwrap();
        std::fs::write(dir.join("driver.mjs"), driver).unwrap();
        let mut c = std::process::Command::new("node");
        c.arg(dir.join("driver.mjs")).current_dir(dir).env_remove(PORT_ENV).env_remove(super::super::api::HEADER_ENV);
        c.envs(env.iter().map(|(k, v)| (k, v)));
        c.output().ok()
    }

    /// Run SnoutCode's plugin in a stand-in for the agent that calls its hooks as the agent
    /// does, changing the file between the tool's before and after; return what the endpoint
    /// heard, or `None` without Node.
    fn run_plugin(name: &str, plugin: &str, driver: &str) -> Option<(Vec<HookEvent>, PathBuf)> {
        let dir = scratch(name);
        let file = dir.join("A.java");
        std::fs::write(&file, "class A {}\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let server =
            HookServer::start(snout_host::local(), move |e| tx.lock().unwrap().send(e).unwrap(), |_| {}).unwrap();
        write_settings(&*snout_host::local(), &dir.join("conf"), server.token()).unwrap();
        let driver = driver
            .replace("FILE", &serde_json::to_string(&file).unwrap())
            .replace("DIR", &serde_json::to_string(&dir).unwrap());
        let env = [
            (PORT_ENV, server.port().to_string()),
            (super::super::api::HEADER_ENV, dir.join("conf/hook-header").to_string_lossy().into_owned()),
        ];
        let out = node(&dir, plugin, &driver, &env)?;
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let mut events = Vec::new();
        while let Ok(e) = rx.recv_timeout(Duration::from_millis(500)) {
            events.push(e);
        }
        // Outside SnoutCode's console the plugin does nothing.
        let quiet = node(&dir, plugin, &driver, &[])?;
        assert!(quiet.status.success(), "{}", String::from_utf8_lossy(&quiet.stderr));
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "nothing heard without the port");
        drop(server);
        Some((events, file))
    }

    #[test]
    fn the_opencode_plugin_reports_each_edit_before_and_after() {
        let driver = r#"
            import { writeFileSync } from "node:fs"
            const { SnoutCode } = await import("./plugin.mjs")
            const hooks = await SnoutCode({ directory: DIR })
            if (!process.env.SNOUT_HOOK_PORT) { if (Object.keys(hooks).length) process.exit(4); process.exit(0) }
            await hooks["chat.message"]({ sessionID: "ses_1" }, { message: {}, parts: [{ type: "text", text: "Add a field" }] })
            await hooks["tool.execute.before"]({ tool: "read", sessionID: "ses_1", callID: "c0" }, { args: { filePath: FILE } })
            await hooks["tool.execute.before"]({ tool: "edit", sessionID: "ses_1", callID: "c1" },
                { args: { filePath: FILE, oldString: "{}", newString: "{ int x; }" } })
            writeFileSync(FILE, "class A { int x; }\n")
            await hooks["tool.execute.after"]({ tool: "edit", sessionID: "ses_1", callID: "c1", args: { filePath: FILE } },
                { title: "", output: "", metadata: {} })
            await hooks.event({ event: { type: "session.idle", properties: { sessionID: "ses_1" } } })
        "#;
        let Some((events, file)) = run_plugin("opencode-plugin", OPENCODE_PLUGIN, driver) else { return };
        let kinds: Vec<_> = events.iter().map(|e| e.kind.clone()).collect();
        assert_eq!(
            kinds,
            [HookKind::Prompt, HookKind::PreTool, HookKind::PostTool, HookKind::Stop],
            "reads are not edits"
        );
        assert!(events.iter().all(|e| e.agent == crate::agents::OPENCODE && e.session == "ses_1"));
        assert_eq!(events[0].prompt.as_deref(), Some("Add a field"));
        assert_eq!(events[1].path.as_deref(), Some(file.as_path()));
        assert_eq!(events[1].file, Some(Snapshot::Text(Arc::new("class A {}\n".into()))), "before the edit");
        assert_eq!(events[2].file, Some(Snapshot::Text(Arc::new("class A { int x; }\n".into()))), "after it");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn the_amp_plugin_reports_each_edit_and_lets_the_tool_run() {
        let driver = r#"
            import { writeFileSync } from "node:fs"
            const mod = await import("./plugin.mjs")
            const on = {}
            const amp = {
                on: (name, f) => { on[name] = f },
                helpers: {
                    filesModifiedByToolCall: (e) => e.tool === "edit_file" ? [{ fsPath: e.input.path }] : null,
                    filePathFromURI: (uri) => uri.fsPath,
                },
            }
            mod.default(amp)
            if (!process.env.SNOUT_HOOK_PORT) { if (Object.keys(on).length) process.exit(4); process.exit(0) }
            const thread = { id: "T-1" }
            const read = await on["tool.call"]({ toolUseID: "t0", tool: "Read", input: { path: FILE }, thread })
            const call = await on["tool.call"]({ toolUseID: "t1", tool: "edit_file", input: { path: FILE }, thread })
            if (read?.action !== "allow" || call?.action !== "allow") process.exit(3)
            writeFileSync(FILE, "class A { int x; }\n")
            await on["tool.result"]({ toolUseID: "t1", tool: "edit_file", input: { path: FILE }, status: "done", thread })
            await on["agent.end"]({ thread, message: "Add a field", id: 2, status: "done", messages: [] })
        "#;
        let Some((events, file)) = run_plugin("amp-plugin", AMP_PLUGIN, driver) else { return };
        let kinds: Vec<_> = events.iter().map(|e| e.kind.clone()).collect();
        assert_eq!(kinds, [HookKind::PreTool, HookKind::PostTool, HookKind::TurnComplete]);
        assert!(events.iter().all(|e| e.agent == crate::agents::AMP && e.session == "T-1"));
        assert_eq!(events[0].file, Some(Snapshot::Text(Arc::new("class A {}\n".into()))));
        assert_eq!(events[1].path.as_deref(), Some(file.as_path()));
        assert_eq!(events[1].file, Some(Snapshot::Text(Arc::new("class A { int x; }\n".into()))));
        assert_eq!(events[2].prompt.as_deref(), Some("Add a field"));
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn opencode_loads_the_plugin_from_a_file_url() {
        let (k, v) = opencode_env(Path::new("/tmp/snout x/snoutcode-opencode.js"));
        assert_eq!(k, "OPENCODE_CONFIG_CONTENT");
        assert_eq!(v, r#"{"plugin":["file:///tmp/snout x/snoutcode-opencode.js"]}"#);
    }
}
