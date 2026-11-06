//! Coding agents beside the repository: finding `claude`, and what an agent is told when it
//! drafts a commit message from the patch the commit would record.

use std::path::PathBuf;

use snout_host::Host;

/// Inherited variables that would make Claude Code think it runs nested inside another
/// session (the app started from a Claude Code terminal), or misreport the terminal.
pub const ENV_REMOVE: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_CODE_CHILD_SESSION",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
];

/// Patches longer than this are cut; the stat at their head still names every file.
const MAX_PATCH: usize = 60_000;

/// `claude` on the host's PATH or in the places its installers put it (an app started from
/// Finder does not get the shell's PATH).
pub fn find_claude(host: &dyn Host) -> Option<PathBuf> {
    let home = &host.info().home;
    let extra: Vec<PathBuf> = [".local/bin", ".claude/local", ".npm-global/bin", ".bun/bin"]
        .iter()
        .filter(|_| !home.as_os_str().is_empty())
        .map(|d| home.join(d))
        .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from))
        // Windows: npm's global folder.
        .chain(host.info().var("APPDATA").map(|d| PathBuf::from(d).join("npm")))
        .collect();
    snout_host::which(host, &["claude"], &extra)
}

/// How to write the message for the change `patch` (cut to fit), and the change: what an agent
/// gets when it asks. `notes` is whatever is in the message box, taken as the author's own
/// words to build on.
pub fn draft_brief(recent: &[&str], notes: &str, amend: bool, patch: &str) -> String {
    let mut s = String::from(
        "Write the git commit message for the change below (a stat summary, then the patch). \
         No code fences, no trailers. \
         The first line is a summary under 72 characters in the imperative mood. If the change \
         needs more, add a blank line and a short body that says what changed and why, wrapped \
         at 72 columns.",
    );
    if amend {
        s += " The commit is being amended, so the patch is everything the amended commit will hold.";
    }
    if !recent.is_empty() {
        s += "\n\nMatch the style of the repository's recent subjects:\n";
        for subject in recent {
            s += &format!("- {subject}\n");
        }
    }
    if !notes.trim().is_empty() {
        s += &format!("\n\nThe author's notes, to build on and keep the intent of:\n{}\n", notes.trim());
    }
    s += "\n\nThe change:\n";
    s += cut(patch, MAX_PATCH);
    s
}

/// A message as the agent gave it, ready for the message box.
pub fn clean_message(reply: &str) -> String {
    clean(reply)
}

/// The reply without code fences or surrounding blank lines.
fn clean(reply: &str) -> String {
    let text = reply.trim();
    let text = match text.strip_prefix("```") {
        Some(rest) => rest.split_once('\n').map_or("", |(_, body)| body).trim_end().trim_end_matches("```"),
        None => text,
    };
    text.trim().to_owned()
}

fn cut(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_replies() {
        assert_eq!(clean("\n\nAdd groups\n\nBody.\n"), "Add groups\n\nBody.");
        assert_eq!(clean("```text\nAdd groups\n```\n"), "Add groups");
        assert_eq!(cut("héllo", 2), "h");
    }

    #[test]
    fn prompt_carries_style_and_notes() {
        let p = draft_brief(&["Fix the thing"], "  group repos  ", true, "diff --git a b");
        assert!(p.contains("- Fix the thing") && p.contains("group repos") && p.contains("amended"));
        assert!(p.ends_with("The change:\ndiff --git a b"));
        assert!(!draft_brief(&[], "", false, "").contains("notes"));
        assert!(draft_brief(&[], "", false, &"x".repeat(MAX_PATCH + 10)).len() < MAX_PATCH + 1_000, "cut to fit");
    }
}
