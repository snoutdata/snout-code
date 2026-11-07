//! What is left of the app: the processes it runs, the settings an agent may change, the keys, and
//! telling the user something.

use gpui::{Context, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::workspace::Workspace;

/// The settings an agent may read and change. A list, not a list of exceptions: what names a program,
/// a shell or a variable would let an agent run anything outside its sandbox the next time the app
/// starts one, or read a secret, and a setting added later stays out until someone decides otherwise.
const SETTINGS: &[&str] = &[
    "theme",
    "side_by_side",
    "compact",
    "ui_font",
    "ui_font_size",
    "editor_font",
    "editor_font_size",
    "tab_size",
    "soft_wrap",
    "show_whitespace",
    "param_hints",
    "show_hover",
    "hover_delay_ms",
    "blame_author_hints",
    "blame_current_line",
    "fetch_minutes",
    "follow_claude",
    "plain_files",
    "daemon_idle_minutes",
    "terminal_font",
    "terminal_font_size",
    "terminal_line_height",
    "terminal_cursor",
    "terminal_cursor_blink",
    "terminal_scrollback",
    "terminal_copy_on_select",
];

pub const ALL: &[Capability] = &[
    Capability {
        name: "showInApp",
        domain: Domain::App,
        effect: Effect::Show,
        description: "Show the user something in SnoutCode's Agent View window, for what no other tool covers: a plan, a \
                      report, a comparison. Write markdown. A fenced code block gets Copy and Insert buttons, and a sh or \
                      bash block a Run in Terminal button. This is for showing the user, not for talking to the app, and \
                      you will not hear of what they click. A push goes after the last; clear starts afresh.",
        input: Input::Own(show_input),
        run: Run::Own(show_in_app),
        consent: None,
    },
    Capability {
        name: "listProcesses",
        domain: Domain::App,
        effect: Effect::Read,
        description: "The processes SnoutCode runs: the language engines, build daemons, runs and terminals, with what \
                      each is for, its memory and CPU, and whether it can be stopped.",
        input: Input::None,
        run: Run::Own(processes),
        consent: None,
    },
    Capability {
        name: "stopProcess",
        domain: Domain::App,
        effect: Effect::Change,
        description: "Stop one of SnoutCode's processes by its pid, from listProcesses.",
        input: Input::Own(pid_input),
        run: Run::Own(stop_process),
        consent: Some(|a| format!("stop SnoutCode's process {}", a["pid"])),
    },
    Capability {
        name: "getSettings",
        domain: Domain::App,
        effect: Effect::Read,
        description: "SnoutCode's appearance and editor settings an agent may change. Not the programs, shells or \
                      variables it runs things with.",
        input: Input::None,
        run: Run::Own(settings),
        consent: None,
    },
    Capability {
        name: "setSetting",
        domain: Domain::App,
        effect: Effect::Change,
        description: "Change one of the settings getSettings lists.",
        input: Input::Own(set_input),
        run: Run::Own(set_setting),
        consent: Some(|a| format!("set {} to {}", text_of(a, "key"), a["value"])),
    },
    Capability {
        name: "getShortcuts",
        domain: Domain::App,
        effect: Effect::Read,
        description: "SnoutCode's commands, each with its keyboard shortcut as bound now, for telling the user how to \
                      do something.",
        input: Input::None,
        run: Run::Own(shortcuts),
        consent: None,
    },
    Capability {
        name: "showNotification",
        domain: Domain::App,
        effect: Effect::Show,
        description: "Tell the user something in the status bar and the Notifications window, for when they may not be \
                      looking at the console: a long job is done, say.",
        input: Input::Own(notify_input),
        run: Run::Own(notify),
        consent: None,
    },
];

fn show_input() -> Value {
    tools::schema(
        json!({
            "markdown": { "type": "string", "description": "What to show" },
            "title": { "type": "string", "description": "A few words above it" },
            "clear": { "type": "boolean", "description": "Take away what shows first", "default": false },
        }),
        &["markdown"],
    )
}

fn show_in_app(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    use crate::claude_ide::agent_view::MAX_PUSH;
    let Some(markdown) = need(a, "markdown") else { return missing(reply, "markdown") };
    if markdown.len() > MAX_PUSH {
        let message = format!("That is {} bytes; one push holds {MAX_PUSH}.", markdown.len());
        return reply.error(INVALID_PARAMS, &message, None);
    }
    let agent = crate::agents::console_agent(cx).short.to_string();
    let title = a["title"].as_str().map(str::to_owned);
    let first = ws.agent_view.read(cx).pushes.is_empty();
    let told = ws.agent_view.update(cx, |v, cx| v.push(agent, title, markdown, a["clear"] == true, cx));
    // It comes forward once, for the first push. After that a push never takes the user away from
    // what they are looking at; the status bar says it came.
    if first {
        ws.show_tool(crate::workspace::Tool::AgentView, window, cx);
    } else {
        ws.model.update(cx, |m, cx| {
            m.message = "An agent added to the Agent View".into();
            cx.notify();
        });
    }
    reply.ok(tools::text(told));
}

fn pid_input() -> Value {
    tools::schema(json!({ "pid": { "type": "integer", "description": "The process's id" } }), &["pid"])
}

fn set_input() -> Value {
    tools::schema(
        json!({
            "key": { "type": "string", "enum": SETTINGS, "description": "The setting" },
            "value": { "type": "string", "description": "Its new value, as getSettings shows it: text, a number or true or false" },
        }),
        &["key", "value"],
    )
}

fn notify_input() -> Value {
    tools::schema(json!({ "message": { "type": "string", "description": "What to tell the user" } }), &["message"])
}

fn processes(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let list: Vec<Value> = ws
        .processes
        .read(cx)
        .rows
        .iter()
        .map(|r| {
            json!({
                "pid": r.pid,
                "purpose": r.purpose,
                "project": r.project,
                "memoryBytes": r.memory,
                "cpuPercent": r.cpu,
                "uptimeSeconds": r.uptime,
                "state": r.state,
                "busy": r.busy,
                "canStop": r.actions().contains(&crate::processes::Act::Stop),
            })
        })
        .collect();
    reply.ok(tools::json_text(&json!({ "processes": list })));
}

fn stop_process(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(pid) = a["pid"].as_u64() else { return missing(reply, "pid") };
    let processes = ws.processes.clone();
    let row = processes.read(cx).rows.iter().find(|r| r.pid.map(u64::from) == Some(pid)).cloned();
    let Some(row) = row else {
        return reply.error(INVALID_PARAMS, &format!("SnoutCode runs no process {pid}"), None);
    };
    if !row.actions().contains(&crate::processes::Act::Stop) {
        return reply.error(TOOL_ERROR, &format!("{} cannot be stopped from here", row.purpose), None);
    }
    processes.update(cx, |p, cx| p.act(&row, crate::processes::Act::Stop, cx));
    reply.ok(tools::text(format!("Stopping {}", row.purpose)));
}

fn settings(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let all = serde_json::to_value(ws.model.read(cx).user_settings()).unwrap_or_default();
    let shown: serde_json::Map<String, Value> =
        SETTINGS.iter().map(|k| ((*k).to_owned(), all.get(*k).cloned().unwrap_or(Value::Null))).collect();
    reply.ok(tools::json_text(&Value::Object(shown)));
}

/// `value`, as the setting's current value says it should be: an agent's MCP or query string gives
/// text for a number or a flag.
fn typed(now: &Value, value: &Value) -> Value {
    let Some(text) = value.as_str() else { return value.clone() };
    match now {
        Value::Bool(_) => text.parse().map_or_else(|_| value.clone(), Value::Bool),
        Value::Number(_) => serde_json::from_str(text).unwrap_or_else(|_| value.clone()),
        _ => value.clone(),
    }
}

fn set_setting(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(key) = need(a, "key").map(str::to_owned) else { return missing(reply, "key") };
    if !SETTINGS.contains(&key.as_str()) {
        let message = format!("{key} is not a setting an agent may change. getSettings lists those it may.");
        return reply.error(INVALID_PARAMS, &message, None);
    }
    let Some(value) = a.get("value") else { return missing(reply, "value") };
    let before = ws.model.read(cx).settings.clone();
    let answer = ws.model.update(cx, |m, cx| {
        let mut all = serde_json::to_value(m.user_settings()).map_err(|e| e.to_string())?;
        let new = typed(&all[&key], value);
        all[&key] = new;
        let changed = serde_json::from_value(all).map_err(|e| format!("{value} is not a value of {key}: {e}"))?;
        m.set_user_settings(changed);
        m.persist();
        cx.notify();
        Ok::<(), String>(())
    });
    match answer {
        Ok(()) => {
            ws.follow_settings(before, window, cx);
            reply.ok(tools::text(format!("{key} is now {value}")));
        }
        Err(e) => reply.error(INVALID_PARAMS, &e, None),
    }
}

fn shortcuts(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let recent = ws.model.read(cx).recent_projects();
    let list: Vec<Value> = crate::menus::commands(&recent[..0])
        .iter()
        .map(|(name, action)| json!({ "command": name.to_string(), "keys": crate::keymap::shortcut(&**action, cx) }))
        .collect();
    reply.ok(tools::json_text(&json!({ "commands": list })));
}

fn notify(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(message) = need(a, "message").map(str::to_owned) else { return missing(reply, "message") };
    let agent = crate::agents::console_agent(cx).short.to_string();
    ws.model.update(cx, |m, cx| {
        let text = format!("{agent}: {message}");
        m.log_output(text.clone(), true);
        m.message = text;
        cx.notify();
    });
    reply.ok(tools::text("The user was told"));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What runs a program or holds a secret stays out of an agent's reach.
    #[test]
    fn an_agent_cannot_reach_what_would_run_something() {
        for key in SETTINGS {
            let k = key.to_lowercase();
            for word in ["path", "shell", "env", "command", "keymap", "update", "external", "sdk", "jdk"] {
                assert!(!k.contains(word), "{key} may run something");
            }
        }
        let defaults = serde_json::to_value(crate::settings::Settings::default()).unwrap();
        for key in SETTINGS {
            assert!(defaults.get(*key).is_some(), "{key} is a setting");
        }
    }

    #[test]
    fn a_value_takes_the_settings_type() {
        assert_eq!(typed(&json!(true), &json!("false")), json!(false));
        assert_eq!(typed(&json!(4), &json!("2")), json!(2));
        assert_eq!(typed(&json!("a"), &json!("b")), json!("b"));
        assert_eq!(typed(&json!(4), &json!(2)), json!(2));
    }
}
