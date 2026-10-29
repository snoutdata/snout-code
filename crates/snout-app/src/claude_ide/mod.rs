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
//! Changes tool window ([`panel`]). [`bridge`] connects them to the workspace.

pub mod bridge;
pub mod changes;
pub mod hooks;
pub mod lockfile;
pub mod panel;
pub mod review;
pub mod server;
pub mod tools;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use gpui::{Global, actions};

// Claude's changes: the next and previous one (in the file, then in the next file it
// changed), keep or revert the one at the caret, keep or revert all of them.
actions!(
    claude,
    [NextClaudeChange, PrevClaudeChange, AcceptClaudeHunk, RevertClaudeHunk, AcceptAllClaude, RevertAllClaude]
);

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

/// How many Claude Code sessions are connected, for the console's header.
#[derive(Clone, Copy, Debug, Default)]
pub struct IdeStatus {
    pub clients: usize,
}

impl Global for IdeStatus {}
