//! What `snout-core` says, in the language in use. That crate has no interface and keeps its
//! words in English; those the window shows are said again here.

use snout_core::{Change, DiffBase, FileEntry, Scope};

/// The two versions a diff compares, as its menu names them.
pub fn diff_base(base: DiffBase) -> &'static str {
    match base {
        DiffBase::HeadToWorktree => ts!("core-head-to-worktree"),
        DiffBase::IndexToWorktree => ts!("core-index-to-worktree"),
        DiffBase::HeadToIndex => ts!("core-head-to-index"),
    }
}

/// The two versions a diff compares, as its panes name them.
pub fn diff_sides(base: DiffBase) -> (&'static str, &'static str) {
    match base {
        DiffBase::HeadToWorktree => ("HEAD", ts!("git-working-tree")),
        DiffBase::IndexToWorktree => (ts!("core-index"), ts!("git-working-tree")),
        DiffBase::HeadToIndex => ("HEAD", ts!("core-index")),
    }
}

/// Beside the Commit button.
pub fn scope_label(scope: &Scope) -> String {
    match scope {
        Scope::Staged(n) => t!("core-scope-staged", n = ((*n) as u64)),
        Scope::Files { paths, all: true } => t!("core-scope-all-changed", n = ((paths.len()) as u64)),
        Scope::Files { paths, all: false } => t!("core-scope-selected", n = ((paths.len()) as u64)),
        Scope::MessageOnly => t!("core-scope-message-only"),
        Scope::Nothing => t!("core-scope-nothing"),
    }
}

/// In a sentence: "Commits …".
pub fn scope_what(scope: &Scope) -> String {
    match scope {
        Scope::Staged(_) => t!("core-what-staged"),
        Scope::Files { paths, all: true } if paths.len() == 1 => t!("core-what-changed-file"),
        Scope::Files { paths, all: true } => t!("core-what-all-changed", n = paths.len().to_string()),
        Scope::Files { paths, .. } if paths.len() == 1 => t!("core-what-selected-file"),
        Scope::Files { paths, .. } => t!("core-what-selected", n = paths.len().to_string()),
        Scope::MessageOnly => t!("core-what-message"),
        Scope::Nothing => t!("core-what-nothing"),
    }
}

fn change(change: Change) -> &'static str {
    match change {
        Change::Modified => ts!("state-modified"),
        Change::TypeChanged => ts!("state-type-changed"),
        Change::Added => ts!("state-added"),
        Change::Deleted => ts!("state-deleted"),
        Change::Renamed => ts!("state-renamed"),
        Change::Copied => ts!("state-copied"),
        Change::Untracked => ts!("state-untracked"),
        Change::Conflicted => ts!("state-conflicted"),
    }
}

/// A file's state, in words.
pub fn file_state(file: &FileEntry) -> String {
    if file.conflicted {
        return ts!("state-conflicted").to_owned();
    }
    match (file.index, file.worktree) {
        (_, Some(Change::Untracked)) => ts!("state-untracked").to_owned(),
        (Some(i), None) => t!("core-state-staged", state = change(i)),
        (None, Some(w)) => change(w).to_owned(),
        (Some(i), Some(_)) => t!("core-state-partly-staged", state = change(i)),
        (None, None) => ts!("core-state-unchanged").to_owned(),
    }
}

/// How long ago `then` was: "just now", "5 minutes ago", "yesterday", "3 years ago".
pub fn ago(then: i64, now: i64) -> String {
    let s = (now - then).max(0);
    let (m, h, d) = (s / 60, s / 3600, s / 86_400);
    match () {
        _ if s < 60 => ts!("welcome-just-now").to_owned(),
        _ if m < 60 => t!("welcome-minutes-ago", count = m),
        _ if h < 24 => t!("welcome-hours-ago", count = h),
        _ if d < 2 => ts!("welcome-yesterday").to_owned(),
        _ if d < 14 => t!("welcome-days-ago", count = d),
        _ if d < 60 => t!("welcome-weeks-ago", count = (d / 7)),
        _ if d < 365 => t!("welcome-months-ago", count = (d / 30)),
        _ => t!("core-years-ago", count = (d / 365)),
    }
}

/// Why a branch or group name will not do.
pub fn name_problem(problem: &'static str) -> &'static str {
    match problem {
        "A branch name cannot start with a dash." => ts!("core-branch-dash"),
        "A branch name cannot contain “..” or “@{”." => ts!("core-branch-dots"),
        "A branch name cannot contain ~ ^ : ? * [ or \\." => ts!("core-branch-chars"),
        "A branch name cannot start with “/” or end with “/”, “.” or “.lock”." => {
            ts!("core-branch-ends")
        }
        "There is already a group with this name" => ts!("core-group-taken"),
        other => other,
    }
}

/// A terminal engine's name.
pub fn engine_label(engine: snout_core::term::EngineKind) -> &'static str {
    match engine {
        snout_core::term::EngineKind::Builtin => ts!("core-engine-built-in"),
        other => other.label(),
    }
}

/// Why this build cannot run a terminal engine.
pub fn engine_unavailable(engine: snout_core::term::EngineKind) -> Option<&'static str> {
    engine.unavailable().map(|_| ts!("core-ghostty-unavailable"))
}

/// The line that stands for unchanged lines left out of a diff.
pub fn gap_label(n: usize) -> String {
    t!("core-gap-unchanged", count = n)
}

/// The updater's sentence for the toast.
pub fn update_text(status: &snout_update::status::UpdateStatus) -> String {
    use snout_update::status::{ManualReason, UpdateStatus as S};
    match status {
        S::Checking => t!("core-update-checking"),
        S::Available { version, .. } => t!("core-update-found", version = version.to_string()),
        S::Downloading { preparing: true, .. } => t!("core-update-preparing-now"),
        S::Downloading { .. } => t!("core-update-downloading-now"),
        S::Downloaded { version, .. } => t!("core-update-ready", version = version.to_string()),
        S::NotAvailable { from_source: true, .. } => t!("core-update-from-source"),
        S::NotAvailable { version, .. } => t!("core-update-up-to-date", version = version.to_string()),
        S::Manual { version, reason: ManualReason::PackageManager } => {
            t!("core-update-available", version = version.to_string())
        }
        S::Manual { version, reason: ManualReason::CannotReplace } => {
            t!("core-update-cannot-replace", version = version.to_string())
        }
        S::Manual { version, reason: ManualReason::DidNotLand } => {
            t!("core-update-did-not-land", version = version.to_string())
        }
        S::Error(failure) => update_failure(failure.kind),
    }
}

/// One sentence on what went wrong with an update, and whose problem it is.
fn update_failure(kind: snout_update::failure::Kind) -> String {
    use snout_update::failure::Kind as K;
    match kind {
        K::Offline => t!("core-update-offline"),
        K::FeedUnavailable => t!("core-update-feed-down"),
        K::RateLimited => t!("core-update-rate-limited"),
        K::NoRelease => t!("core-update-no-release"),
        K::Signature => t!("core-update-signature"),
        K::Disk => t!("core-update-disk"),
        K::Unknown => t!("core-update-unknown"),
    }
}

/// An update on its way, for the status bar: its stage and how far it is.
pub fn update_task(status: &snout_update::status::UpdateStatus) -> Option<(String, Option<u32>)> {
    use snout_update::status::UpdateStatus as S;
    let (_, percent) = status.task()?;
    let stage = match status {
        S::Downloading { preparing: true, .. } => t!("core-update-preparing"),
        _ => t!("core-update-downloading"),
    };
    Some((stage, percent))
}
