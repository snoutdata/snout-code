//! SnoutCode as the IDE Claude Code connects to. Claude Code's IDE integration is a
//! WebSocket server it finds through a lock file, speaking JSON-RPC with MCP's methods;
//! it calls the IDE's tools (open a file, propose a diff and wait for the answer, read the
//! selection and the diagnostics) and hears the selection change.
//!
//! The protocol is not formally documented and may change, so it is kept here: the lock
//! file ([`lockfile`]), the server and its messages ([`server`]), the tools' shapes
//! ([`tools`]) and the text side of a review ([`review`]), none of which knows the app.
//! Claude Code's hooks report every edit it makes, asked first or not ([`hooks`]); what
//! it changed waits for review in the session's change set ([`changes`]) and the Claude
//! Changes tool window ([`panel`]). What the model itself asks of the editor goes through
//! a skill, a UTCP manual and plain HTTP instead ([`api`]), from the table of what an agent can
//! ask of the app ([`capabilities`]). [`bridge`] connects them to the workspace.

pub mod agent_log;
pub mod agent_view;
pub mod api;
pub mod bridge;
pub mod capabilities;
pub mod changes;
pub mod hooks;
pub mod ledger;
pub mod lockfile;
pub mod panel;
pub mod review;
pub mod server;
pub mod tools;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use gpui::{Global, actions};

// The agents' changes: the next and previous one (in the file, then in the next file
// changed), keep or revert the one at the caret, keep or revert all of them; show the Agent
// Changes window; show who wrote which lines.
actions!(
    claude,
    [
        NextClaudeChange,
        PrevClaudeChange,
        AcceptClaudeHunk,
        RevertClaudeHunk,
        AcceptAllClaude,
        RevertAllClaude,
        ShowAgentChanges,
        AnnotateWithAgents
    ]
);

/// The files with agents' changes nobody has reviewed yet, and the agent of each: the
/// Project tree's dots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentFiles(pub std::collections::HashMap<PathBuf, &'static str>);

impl Global for AgentFiles {}

/// What the status bar says of the agents' changes: how many files wait for review, and
/// the agents that changed them, the one that changed the most first (Claude Code first of
/// equals).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentSummary {
    pub files: usize,
    pub agents: Vec<&'static str>,
}

impl Global for AgentSummary {}

impl AgentSummary {
    /// The summary of `files`, each with the agent that changed it.
    pub fn of<'a>(files: impl IntoIterator<Item = &'a &'static str>) -> Self {
        let mut count: Vec<(&'static str, usize)> = Vec::new();
        let mut total = 0;
        for a in files {
            total += 1;
            match count.iter_mut().find(|(b, _)| b == a) {
                Some((_, n)) => *n += 1,
                None => count.push((a, 1)),
            }
        }
        count.sort_by_key(|(a, n)| (std::cmp::Reverse(*n), *a != crate::agents::CLAUDE, *a));
        Self { files: total, agents: count.into_iter().map(|(a, _)| a).collect() }
    }

    /// What the status bar says: "Claude changed 3 files", "Claude and opencode changed
    /// 4 files", "3 agents changed 5 files"; `short` names an agent as its caret does.
    pub fn text(&self, short: impl Fn(&'static str) -> String) -> String {
        let who = match self.agents.as_slice() {
            [] => t!("agents-all"),
            [a] => short(a),
            [a, b] => t!("agents-two", a = short(a), b = short(b)),
            more => t!("agents-n", n = more.len().to_string()),
        };
        t!("agents-changed-files", who = who, count = self.files)
    }
}

/// Where the window's server writes its lock file. Set by the app; tests set a scratch
/// folder, and without it no server starts.
#[derive(Clone, Debug)]
pub struct IdeConfig {
    pub lock_dir: PathBuf,
}

impl Global for IdeConfig {}

/// The variables a `claude` started in the console gets, so it connects.
#[derive(Clone, Debug, Default)]
pub struct IdeEnv(pub Vec<(&'static str, String)>);

impl Global for IdeEnv {}

/// The arguments a `claude` started in the console gets: the hook settings.
#[derive(Clone, Debug, Default)]
pub struct IdeArgs(pub Vec<String>);

impl Global for IdeArgs {}

/// The arguments a `codex` started in the console gets: its hooks, which report each edit
/// and prompt to the hooks' endpoint, and its `notify` program, which tells it when a turn
/// ends.
#[derive(Clone, Debug, Default)]
pub struct CodexArgs(pub Vec<String>);

impl Global for CodexArgs {}

/// The variables an `opencode` started in the console gets besides the hooks' port: the one
/// that loads SnoutCode's plugin for the session.
#[derive(Clone, Debug, Default)]
pub struct OpencodeEnv(pub Vec<(&'static str, String)>);

impl Global for OpencodeEnv {}

/// How many Claude Code sessions are connected, for the console's header.
#[derive(Clone, Copy, Debug, Default)]
pub struct IdeStatus {
    pub clients: usize,
}

impl Global for IdeStatus {}
