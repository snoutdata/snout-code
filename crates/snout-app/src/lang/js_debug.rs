//! js-debug, the JavaScript debugger (the standalone DAP server of `microsoft/vscode-js-debug`):
//! downloaded once per host at a pinned version and checked against its checksum, then run on
//! the project's Node. It debugs any Node program. SnoutCode starts the program itself, with
//! the variables js-debug asks for (`runInTerminal`: a `--require` of its bootloader in
//! `NODE_OPTIONS`); each Node process the program starts (Playwright's runner and its
//! workers, a script's children) then comes back as a session of its own (`startDebugging`),
//! on a connection of its own to the same server.

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context as _, bail};
use serde_json::{Value, json};
use snout_host::{Cmd, Host, Process, Stdio};

use crate::run::process::ProcessLaunch;

/// The release SnoutCode uses.
pub const VERSION: &str = "1.140.0";
const SHA256: &str = "27dab92937ec1ab35821ae955aac867544fe06a1b6307229049f2d789af10968";

fn url() -> String {
    format!("https://github.com/microsoft/vscode-js-debug/releases/download/v{VERSION}/js-debug-dap-v{VERSION}.tar.gz")
}

/// Where the release is unpacked on a host: beside the Node SnoutCode downloads.
pub fn dir(info: &snout_host::HostInfo) -> PathBuf {
    super::node::downloads_dir(info).with_file_name("js-debug").join(VERSION)
}

/// The server's script, in the release.
pub fn server_script(info: &snout_host::HostInfo) -> PathBuf {
    dir(info).join("js-debug/src/dapDebugServer.js")
}

/// The server's script, when the release is on `host`.
pub fn installed(host: &dyn Host) -> Option<PathBuf> {
    Some(server_script(host.info())).filter(|s| host.is_file(s))
}

/// Download the release onto `host`, check it, unpack it, and return the server's script.
/// Blocking; run it off the main thread.
pub fn download(host: &dyn Host, progress: &super::java::DownloadProgress) -> anyhow::Result<PathBuf> {
    let dir = dir(host.info());
    let parent = dir.parent().context("no folder for the debugger")?;
    host.create_dir_all(parent)?;
    let part = parent.join(format!(".js-debug-{VERSION}.tar.gz.part"));
    super::java::fetch_file(host, &url(), &part, progress).context("download js-debug")?;
    if super::java::sha256_on(host, &part)? != SHA256 {
        let _ = host.remove_file(&part);
        bail!("the download is damaged (its checksum does not match)");
    }
    let unpack = parent.join(format!(".js-debug-{VERSION}.unpack"));
    let _ = host.remove_dir_all(&unpack);
    host.create_dir_all(&unpack)?;
    let ok = host.run(&Cmd::new("tar").arg("-xzf").arg(&part).arg("-C").arg(&unpack), None)?.status.success();
    let _ = host.remove_file(&part);
    if !ok {
        bail!("the download could not be unpacked");
    }
    let _ = host.remove_dir_all(&dir);
    host.rename(&unpack, &dir)?;
    installed(host).context("the download holds no debug server")
}

/// The port in the server's first line (`Debug server listening at 127.0.0.1:63978`).
pub fn port_of(line: &str) -> Option<u16> {
    line.trim().strip_prefix("Debug server listening at ")?.rsplit(':').next()?.parse().ok()
}

/// A debug server running on a host; it ends when this goes.
pub struct Server {
    process: Mutex<Box<dyn Process>>,
    pub port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Ok(p) = self.process.get_mut() {
            let _ = p.kill();
            let _ = p.wait();
        }
    }
}

/// Start the server `script` on `node`, on a free port of `host`'s loopback. Blocking.
pub fn start(host: &dyn Host, node: &Path, script: &Path) -> anyhow::Result<Server> {
    let cmd = Cmd::new(node)
        .arg(script)
        .args(["0", "127.0.0.1"])
        .cwd(script.parent().unwrap_or(Path::new("/")))
        .stdin(Stdio::Null)
        .stdout(Stdio::Piped)
        .stderr(Stdio::Null);
    let mut process = host.spawn(&cmd).context("start js-debug")?;
    let out = process.take_stdout().context("no output from js-debug")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = BufReader::new(out).lines();
        if let Some(Ok(first)) = lines.next() {
            let _ = tx.send(first);
        }
        // What it says later is of no use; it is read so the server never blocks on it.
        for _ in lines {}
    });
    match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(line) => match port_of(&line) {
            Some(port) => Ok(Server { process: Mutex::new(process), port }),
            None => {
                let _ = process.kill();
                bail!("js-debug said {line:?}")
            }
        },
        Err(_) => {
            let _ = process.kill();
            bail!("js-debug did not start")
        }
    }
}

/// What the client tells js-debug it can do.
pub fn initialize_args() -> Value {
    json!({
        "clientID": "snoutcode", "clientName": "SnoutCode", "adapterID": "pwa-node",
        "linesStartAt1": true, "columnsStartAt1": true, "pathFormat": "path",
        "supportsVariableType": true, "supportsVariablePaging": false,
        "supportsRunInTerminalRequest": true, "supportsStartDebuggingRequest": true,
        "supportsProgressReporting": false, "supportsInvalidatedEvent": false,
    })
}

/// The `launch` that debugs `launch`: js-debug asks the client to start it (`runInTerminal`)
/// with its variables, and follows every Node process it starts.
pub fn launch_args(launch: &ProcessLaunch) -> Value {
    let env: serde_json::Map<String, Value> =
        launch.env.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
    json!({
        "type": "pwa-node",
        "request": "launch",
        "name": launch.title,
        "runtimeExecutable": launch.program,
        "runtimeArgs": launch.args,
        "cwd": launch.cwd,
        "env": env,
        "console": "integratedTerminal",
        "autoAttachChildProcesses": true,
        "skipFiles": ["<node_internals>/**"],
    })
}

/// `launch` as `runInTerminal` asks it started: its variables, with js-debug's on top
/// (a variable it sets to null is left as the launch has it).
pub fn with_terminal_env(launch: &ProcessLaunch, arguments: &Value) -> ProcessLaunch {
    let mut out = launch.clone();
    for (k, v) in arguments.get("env").and_then(Value::as_object).into_iter().flatten() {
        let Some(v) = v.as_str() else { continue };
        out.env.retain(|(name, _)| name != k);
        out.env.push((k.clone(), v.to_owned()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Place;

    #[test]
    fn the_server_says_its_port_and_runs_get_its_variables() {
        assert_eq!(port_of("Debug server listening at 127.0.0.1:63978\n"), Some(63978));
        assert_eq!(port_of("Debug server listening at ::1:9229"), Some(9229));
        assert_eq!(port_of("something else"), None);
        let launch = ProcessLaunch {
            title: "math.spec.ts".into(),
            program: "/n/bin/node".into(),
            args: vec!["cli.js".into(), "test".into()],
            cwd: "/qa".into(),
            env: vec![("FORCE_COLOR".into(), "1".into()), ("NODE_OPTIONS".into(), "--max-old-space-size=4096".into())],
            place: Place::Debug,
            report: true,
            debug_node: Some("/n/bin/node".into()),
        };
        let args = launch_args(&launch);
        assert_eq!(args["runtimeExecutable"], "/n/bin/node");
        assert_eq!(args["runtimeArgs"], json!(["cli.js", "test"]));
        assert_eq!(args["env"]["FORCE_COLOR"], "1");
        assert_eq!(args["console"], "integratedTerminal");
        let asked = json!({ "env": {
            "NODE_OPTIONS": "--max-old-space-size=4096 --require /js/bootloader.js",
            "VSCODE_INSPECTOR_OPTIONS": ":::{}",
            "GONE": null,
        }});
        let run = with_terminal_env(&launch, &asked);
        let get = |k: &str| run.env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("NODE_OPTIONS"), Some("--max-old-space-size=4096 --require /js/bootloader.js"));
        assert_eq!(get("VSCODE_INSPECTOR_OPTIONS"), Some(":::{}"));
        assert_eq!(get("FORCE_COLOR"), Some("1"));
        assert_eq!(get("GONE"), None);
        assert_eq!(run.env.iter().filter(|(n, _)| n == "NODE_OPTIONS").count(), 1);
        assert_eq!((run.program, run.args), (launch.program, launch.args));
    }
}
