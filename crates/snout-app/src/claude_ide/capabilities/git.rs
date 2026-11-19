//! Git, on screen, and the changes that go through the app. An agent's sandbox may stop its own `git`
//! writing to `.git`; these calls do not depend on it.

use gpui::{Context, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::model::Action;
use crate::panels::Activity;
use crate::workspace::Workspace;

/// The first line of a commit message, for the card.
fn subject(a: &Value) -> String {
    text_of(a, "message").lines().next().unwrap_or_default().to_owned()
}

/// The paths of a call, for the card.
fn paths_of(a: &Value) -> Vec<String> {
    a["paths"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}

pub const ALL: &[Capability] = &[
    Capability {
        name: "showChanges",
        domain: Domain::Git,
        effect: Effect::Show,
        description: "Show the Git view, with the project's changes, for the user to look at.",
        input: Input::None,
        run: Run::Own(|ws, _, reply, window, cx| {
            ws.show_activity(Activity::Git, window, cx);
            reply.ok(tools::text("The Git view shows"));
        }),
        consent: None,
    },
    Capability {
        name: "showDiff",
        domain: Domain::Git,
        effect: Effect::Show,
        description: "Show a changed file's diff in the Git view, by its path from the repository's root.",
        input: Input::Own(path_input),
        run: Run::Own(show_diff),
        consent: None,
    },
    Capability {
        name: "showCommit",
        domain: Domain::Git,
        effect: Effect::Show,
        description: "Show a commit in the Git view, by its hash.",
        input: Input::Own(commit_input),
        run: Run::Own(show_commit),
        consent: None,
    },
    Capability {
        name: "showBranches",
        domain: Domain::Git,
        effect: Effect::Show,
        description: "Open the Check Out dialog, listing the branches, for the user to pick one. It checks out nothing \
                      by itself.",
        input: Input::None,
        run: Run::Own(|ws, _, reply, _, cx| {
            ws.send(Action::ShowCheckout, cx);
            reply.ok(tools::text("The Check Out dialog shows"));
        }),
        consent: None,
    },
    Capability {
        name: "stageFiles",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Stage files for the next commit, by their paths from the repository's root.",
        input: Input::Own(paths_input),
        run: Run::Own(|ws, a, reply, _, cx| paths_action(ws, a, reply, Action::Stage, "Staged", cx)),
        consent: Some(|a| format!("stage {}", paths_of(a).join(", "))),
    },
    Capability {
        name: "unstageFiles",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Take files out of the next commit, leaving their changes, by their paths from the repository's \
                      root.",
        input: Input::Own(paths_input),
        run: Run::Own(|ws, a, reply, _, cx| paths_action(ws, a, reply, Action::Unstage, "Unstaged", cx)),
        consent: Some(|a| format!("unstage {}", paths_of(a).join(", "))),
    },
    Capability {
        name: "commitChanges",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Commit what is staged, with this message, through the Git view. The user sees the message before \
                      allowing it.",
        input: Input::Own(message_input),
        run: Run::Own(commit),
        consent: Some(|a| format!("commit \"{}\"", subject(a))),
    },
    Capability {
        name: "pushCommits",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Push the current branch to its upstream, with the user's Git credentials.",
        input: Input::None,
        run: Run::Own(|ws, _, reply, _, cx| repo_action(ws, reply, Action::Push, "Pushing", cx)),
        consent: Some(|_| "push the current branch".to_owned()),
    },
    Capability {
        name: "fetchRemotes",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Fetch from the repository's remotes. It changes no branch or file.",
        input: Input::None,
        run: Run::Own(|ws, _, reply, _, cx| repo_action(ws, reply, Action::Fetch, "Fetching", cx)),
        consent: Some(|_| "fetch from the remotes".to_owned()),
    },
    Capability {
        name: "getGitCredentials",
        domain: Domain::Git,
        effect: Effect::Read,
        description: "How Git signs in to a repository's remote: the remote, the credential helpers in effect for it \
                      (each with the file that sets it), the programs each calls and whether Git finds them or where \
                      they are installed instead, and the repair SnoutCode can make (`repair`). Call it first when a \
                      pull, fetch or push could not sign in. No token is ever read or shown.",
        input: Input::Own(repo_input),
        run: Run::Own(credentials),
        consent: None,
    },
    Capability {
        name: "fixGitCredentialHelper",
        domain: Domain::Git,
        effect: Effect::Change,
        description: "Make the repair getGitCredentials names: the helper calls the program Git cannot find by its \
                      full path, in the file that set the helper, and nothing else changes.",
        input: Input::Own(repo_input),
        run: Run::Own(fix_credentials),
        consent: Some(|a| match a.get("path").and_then(Value::as_str) {
            Some(p) => format!("repair the Git credential helper of {p}"),
            None => "repair the Git credential helper of the repository".to_owned(),
        }),
    },
];

fn repo_input() -> Value {
    tools::schema(
        json!({ "path": { "type": "string", "description": "The repository, absolute; the Git view's when left out" } }),
        &[],
    )
}

/// The repository a call names, else the one the Git view shows, else the project.
fn repo_of(ws: &Workspace, a: &Value, cx: &gpui::App) -> Option<std::path::PathBuf> {
    let home = crate::host::host(cx).info().home.clone();
    a.get("path").and_then(Value::as_str).map(|p| tools::path_of(p, &home)).or_else(|| {
        let m = ws.model.read(cx);
        m.repo_root().or_else(|| m.project_root())
    })
}

fn credentials(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(repo) = repo_of(ws, a, cx) else { return missing(reply, "path") };
    let (host, extra) = (crate::host::host(cx), crate::credentials::ProgramDirs::of(cx));
    cx.background_executor()
        .spawn(async move {
            let report = crate::credentials::diagnose(&*host, &repo, &extra);
            let repair = report.repair();
            reply.ok(tools::json_text(&json!({ "report": report, "repair": repair })));
        })
        .detach();
}

fn fix_credentials(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(repo) = repo_of(ws, a, cx) else { return missing(reply, "path") };
    let (host, extra) = (crate::host::host(cx), crate::credentials::ProgramDirs::of(cx));
    cx.background_executor()
        .spawn(async move {
            let Some(repair) = crate::credentials::diagnose(&*host, &repo, &extra).repair() else {
                return reply.error(TOOL_ERROR, "There is no helper here that a full path repairs.", None);
            };
            match crate::credentials::apply(&*host, &repair) {
                Ok(()) => reply.ok(tools::text(format!(
                    "The helper in {} now calls {}. Try the pull, fetch or push again.",
                    repair.helper.file.as_ref().map(|f| f.display().to_string()).unwrap_or_default(),
                    repair.path.display()
                ))),
                Err(e) => reply.error(TOOL_ERROR, &e, None),
            }
        })
        .detach();
}

fn path_input() -> Value {
    tools::schema(json!({ "path": { "type": "string", "description": "From the repository's root" } }), &["path"])
}

fn commit_input() -> Value {
    tools::schema(json!({ "hash": { "type": "string", "description": "The commit's hash" } }), &["hash"])
}

fn message_input() -> Value {
    tools::schema(json!({ "message": { "type": "string", "description": "The commit message" } }), &["message"])
}

fn paths_input() -> Value {
    tools::schema(
        json!({ "paths": { "type": "array", "items": { "type": "string" }, "description": "From the repository's root" } }),
        &["paths"],
    )
}

fn no_repository(ws: &Workspace, reply: Reply, cx: &gpui::App) -> Option<Reply> {
    if ws.model.read(cx).repo_root().is_some() {
        return Some(reply);
    }
    reply.error(TOOL_ERROR, "The project has no Git repository", None);
    None
}

fn show_diff(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(path) = need(a, "path").map(str::to_owned) else { return missing(reply, "path") };
    let Some(reply) = no_repository(ws, reply, cx) else { return };
    let known = ws.model.read(cx).project_status().files.iter().any(|f| f.path == path);
    if !known {
        let message = format!("{path} has no changes. getAppState says how many files have them.");
        return reply.error(INVALID_PARAMS, &message, None);
    }
    ws.show_activity(Activity::Git, window, cx);
    ws.model.update(cx, |m, cx| m.click_file(&path, false, false, cx));
    reply.ok(tools::text(format!("The diff of {path} shows")));
}

fn show_commit(ws: &mut Workspace, a: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(hash) = need(a, "hash").map(str::to_owned) else { return missing(reply, "hash") };
    let Some(reply) = no_repository(ws, reply, cx) else { return };
    ws.show_activity(Activity::Git, window, cx);
    ws.send(Action::ShowCommit(hash.clone()), cx);
    reply.ok(tools::text(format!("Commit {hash} shows")));
}

fn paths_action(
    ws: &mut Workspace,
    a: &Value,
    reply: Reply,
    action: fn(Vec<String>) -> Action,
    done: &str,
    cx: &mut Context<Workspace>,
) {
    let paths = paths_of(a);
    if paths.is_empty() {
        return missing(reply, "paths");
    }
    let Some(reply) = no_repository(ws, reply, cx) else { return };
    let text = format!("{done} {}", paths.join(", "));
    ws.send(action(paths), cx);
    reply.ok(tools::text(text));
}

fn repo_action(ws: &mut Workspace, reply: Reply, action: Action, doing: &str, cx: &mut Context<Workspace>) {
    let Some(reply) = no_repository(ws, reply, cx) else { return };
    ws.send(action, cx);
    reply.ok(tools::text(format!(
        "{doing}. In getAppState, project.git.running names it while it runs, and project.git.lastFailure says \
         what Git said if it failed (signIn: why Git could not sign in to the remote)."
    )));
}

fn commit(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let Some(message) = need(a, "message").map(str::to_owned) else { return missing(reply, "message") };
    let Some(reply) = no_repository(ws, reply, cx) else { return };
    let ready = ws.model.update(cx, |m, cx| {
        m.commit_msg = message;
        cx.notify();
        m.can_commit()
    });
    if !ready {
        let text = "Nothing can be committed: stage files first with stageFiles. The message is in the commit box.";
        return reply.error(TOOL_ERROR, text, None);
    }
    ws.send(Action::Commit, cx);
    reply.ok(tools::text("Committing. The Git view shows how it goes."));
}
