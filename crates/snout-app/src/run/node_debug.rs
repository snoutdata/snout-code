//! Debugging a Node program with js-debug: a session whose [`ProcessLaunch`] has a
//! `debug_node` starts js-debug's server on that Node (downloading js-debug the first time),
//! connects, and asks it to launch the program. js-debug answers with `runInTerminal`: the
//! session starts the program as any process run, with js-debug's variables added, so its
//! output, its reporter and Stop work as they do without the debugger. Each Node process the
//! program starts then asks for a session of its own (`startDebugging`), which gets a
//! connection of its own, the breakpoints, and the Debug window when it stops.
//!
//! Nothing here knows about Playwright: any Node program (a script, `npm run`, a test
//! runner) is debugged the same way.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::Context;
use serde_json::{Value, json};
use snout_lang::dap::Dap;

use super::debug::Adapter;
use super::process::ProcessLaunch;
use super::{Ended, Outcome, Runs, Stream};
use crate::lang::js_debug;

const WAIT: Duration = Duration::from_secs(10);

impl Runs {
    /// Start session `id`'s program under js-debug, run on `node`.
    pub(super) fn start_node_debug(&mut self, id: u64, launch: ProcessLaunch, node: PathBuf, cx: &mut Context<Self>) {
        if let Some(d) = self.session_mut(id).and_then(|s| s.debug.as_mut()) {
            d.adapter = Adapter::Node;
            d.attaching = true;
        }
        let host = crate::host::host(cx);
        let fixed = self.js_debug_server.clone();
        cx.spawn(async move |this, cx| {
            let installed = match fixed {
                Some(script) => Some(script),
                None => {
                    let host = host.clone();
                    cx.background_executor().spawn(async move { js_debug::installed(&*host) }).await
                }
            };
            let script = match installed {
                Some(s) => Ok(s),
                None => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(s) = this.session_mut(id) {
                            s.push(
                                Stream::System,
                                &format!("Downloading the JavaScript debugger (js-debug {})", js_debug::VERSION),
                            );
                        }
                        cx.notify();
                    });
                    let host = host.clone();
                    cx.background_executor()
                        .spawn(async move { js_debug::download(&*host, &Default::default()) })
                        .await
                        .map_err(|e| format!("The JavaScript debugger could not be downloaded: {e:#}"))
                }
            };
            let connected = match script {
                Ok(script) => {
                    let host = host.clone();
                    cx.background_executor()
                        .spawn(async move {
                            let server = js_debug::start(&*host, &node, &script)
                                .map_err(|e| format!("The JavaScript debugger did not start: {e:#}"))?;
                            let (dap, events) = Dap::connect(&*host, server.port, WAIT)
                                .map_err(|e| format!("Could not connect the debugger: {e}"))?;
                            Ok((Arc::new(server), dap, events))
                        })
                        .await
                }
                Err(e) => Err(e),
            };
            let (server, dap, events) = match connected {
                Ok(c) => c,
                Err(why) => {
                    let _ = this.update(cx, |this, cx| this.node_debug_failed(id, why, cx));
                    return;
                }
            };
            let ok = this.update(cx, |this, cx| {
                let listen = this.listen(id, dap.clone(), events, cx);
                let Some(d) = this.session_mut(id).and_then(|s| s.debug.as_mut()).filter(|d| !d.ended) else {
                    return false;
                };
                d.attaching = false;
                d.server = Some(server);
                d.add_target(&dap);
                d.events.push(listen);
                cx.notify();
                true
            });
            if !matches!(ok, Ok(true)) {
                dap.close();
                return;
            }
            if let Err(e) = dap.request("initialize", js_debug::initialize_args()).await {
                eprintln!("[debug] initialize: {e}");
            }
            if let Err(e) = dap.request("launch", js_debug::launch_args(&launch)).await {
                let _ = this.update(cx, |this, cx| {
                    this.node_debug_failed(id, format!("The debugger could not start the program: {e}"), cx)
                });
            }
        })
        .detach();
    }

    /// Session `id` could not get its debugger going: it ends, saying why.
    fn node_debug_failed(&mut self, id: u64, why: String, cx: &mut Context<Self>) {
        let Some(s) = self.session_mut(id) else { return };
        if let Some(d) = s.debug.as_mut() {
            d.disconnect();
        }
        if s.ended.is_none() && s.process.is_none() {
            s.push(Stream::Err, &why);
            s.ended = Some(Ended {
                outcome: Outcome::Failed,
                elapsed: s.started.elapsed(),
                exit_code: None,
                error: Some(why),
            });
            if let Some(run) = s.playwright.as_mut() {
                run.finish();
            }
            cx.emit(super::RunsEvent::Ended(id));
        } else {
            s.push(Stream::Err, &why);
        }
        cx.notify();
    }

    /// The debug adapter asks something of SnoutCode, over `from`.
    pub(super) fn adapter_asks(
        &mut self,
        id: u64,
        from: &Dap,
        seq: i64,
        command: &str,
        arguments: Value,
        cx: &mut Context<Self>,
    ) {
        let node = self.session(id).and_then(|s| s.debug.as_ref()).is_some_and(|d| d.adapter == Adapter::Node);
        match command {
            "runInTerminal" if node => self.node_run_in_terminal(id, from, seq, &arguments, cx),
            "startDebugging" if node => self.node_start_debugging(id, from, seq, arguments, cx),
            _ => from.respond(seq, command, Err("not supported by this client".into())),
        }
    }

    /// js-debug is ready for the program: start it, with its variables.
    fn node_run_in_terminal(&mut self, id: u64, from: &Dap, seq: i64, arguments: &Value, cx: &mut Context<Self>) {
        let Some(s) = self.session(id) else { return };
        let super::Launch::Process(launch) = &s.launch else { return };
        if s.process.is_some() || s.ended.is_some() {
            from.respond(seq, "runInTerminal", Err("the program has started already".into()));
            return;
        }
        let launch = js_debug::with_terminal_env(launch, arguments);
        self.spawn_process(id, launch, cx);
        match self.session(id).and_then(|s| s.pid) {
            Some(pid) => from.respond(seq, "runInTerminal", Ok(json!({ "processId": pid }))),
            None => from.respond(seq, "runInTerminal", Err("the program did not start".into())),
        }
    }

    /// A Node process of the program wants debugging: connect to it as a session of its own.
    fn node_start_debugging(&mut self, id: u64, from: &Dap, seq: i64, arguments: Value, cx: &mut Context<Self>) {
        let Some(port) =
            self.session(id).and_then(|s| s.debug.as_ref()).and_then(|d| d.server.as_ref()).map(|s| s.port)
        else {
            from.respond(seq, "startDebugging", Err("the debugger has ended".into()));
            return;
        };
        from.respond(seq, "startDebugging", Ok(Value::Null));
        let request = arguments.get("request").and_then(Value::as_str).unwrap_or("launch").to_owned();
        let configuration = arguments.get("configuration").cloned().unwrap_or_else(|| json!({}));
        let host = crate::host::host(cx);
        let connect = cx.background_executor().spawn(async move { Dap::connect(&*host, port, WAIT) });
        cx.spawn(async move |this, cx| {
            let (dap, events) = match connect.await {
                Ok(c) => c,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(s) = this.session_mut(id) {
                            s.push(Stream::Err, &format!("Could not connect the debugger to a Node process: {e}"));
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let ok = this.update(cx, |this, cx| {
                let listen = this.listen(id, dap.clone(), events, cx);
                let Some(d) = this.session_mut(id).and_then(|s| s.debug.as_mut()).filter(|d| !d.ended) else {
                    return false;
                };
                d.add_target(&dap);
                d.events.push(listen);
                true
            });
            if !matches!(ok, Ok(true)) {
                dap.close();
                return;
            }
            if let Err(e) = dap.request("initialize", js_debug::initialize_args()).await {
                eprintln!("[debug] initialize: {e}");
            }
            if let Err(e) = dap.request(&request, configuration).await {
                eprintln!("[debug] {request}: {e}");
            }
        })
        .detach();
    }
}
