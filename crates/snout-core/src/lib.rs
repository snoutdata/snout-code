//! Git operations for snout.
//!
//! Everything here shells out to the `git` CLI so that the user's own configuration
//! (credential helpers, SSH agent, hooks, `pull.rebase`, ...) applies unchanged.
//!
//! The API is deliberately front-end agnostic: all results are plain `serde`-serializable
//! values, so the same calls can back the desktop GUI and an MCP server.

mod branches;
mod checkout;
pub mod claude;
mod diff;
pub mod diff_layout;
mod discover;
mod error;
mod git;
mod github;
pub mod graph;
pub mod groups;
mod log;
mod pull;
mod repo;
mod scope;
mod status;
pub mod survey;
mod tags;

pub use scope::{Scope, branch_name_problem, on_disk, repo_dir_name, stage_paths, unstage_paths};

pub use branches::{Branch, BranchKind};
pub use checkout::{CheckoutOutcome, CheckoutTarget, LocalChanges};
pub use diff::{DiffBase, DiffRow, FileDiff, RowKind};
pub use discover::find_repos;
pub use error::{Error, Result};
pub use github::github_web_url;
pub use log::{Commit, CommitDetails, CommitFile};
pub use pull::{PullMode, PullProblem};
pub use repo::{Repo, Tracking, can_trash};
pub use status::{BranchInfo, Change, FileEntry, Status};
pub use tags::Tag;
