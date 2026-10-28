//! Claude Code beside the repository: finding `claude`, and commit messages drafted by it
//! in print mode (`claude -p`) from the patch the commit would record. A draft runs beside
//! any console session and leaves that alone.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};

/// Inherited variables that would make Claude Code think it runs nested inside another
/// session (the app started from a Claude Code terminal), or misreport the terminal.
pub const ENV_REMOVE: &[&str] =
    &["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_SSE_PORT", "TERM_PROGRAM", "TERM_PROGRAM_VERSION"];

/// Patches longer than this are cut; the stat at their head still names every file.
const MAX_PATCH: usize = 60_000;

/// `claude` on PATH or in the places its installers put it (an app started from Finder
/// does not get the shell's PATH).
pub fn find_claude() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path_dirs = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    let extra = [".local/bin", ".claude/local", ".npm-global/bin", ".bun/bin"]
        .iter()
        .filter_map(|d| home.as_ref().map(|h| h.join(d)))
        .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    path_dirs.into_iter().chain(extra).map(|d| d.join("claude")).find(|p| p.is_file())
}

/// A running draft. Dropping it stops the process.
pub struct Draft {
    child: Arc<Mutex<Child>>,
    rx: mpsc::Receiver<Result<String, String>>,
}

impl Draft {
    /// Start `program -p prompt` in `root` with `patch` on stdin; `wake` runs on a worker
    /// thread when the result is ready for [`Draft::poll`].
    pub fn start(
        program: &Path,
        root: &Path,
        prompt: &str,
        patch: &str,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(program);
        cmd.args(["-p", prompt, "--output-format", "text", "--no-session-persistence", "--strict-mcp-config"])
            // Text in, text out: nothing to read or run.
            .args(["--tools", ""])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for var in ENV_REMOVE {
            cmd.env_remove(var);
        }
        let mut child = cmd.spawn().map_err(|e| format!("Could not start {}: {e}", program.display()))?;
        let (mut stdin, mut stdout, mut stderr) = (
            child.stdin.take().expect("piped"),
            child.stdout.take().expect("piped"),
            child.stderr.take().expect("piped"),
        );
        let child = Arc::new(Mutex::new(child));
        let patch = cut(patch, MAX_PATCH).to_owned();
        let (tx, rx) = mpsc::channel();
        let waiter = child.clone();
        std::thread::spawn(move || {
            let errors = std::thread::spawn(move || {
                let mut s = String::new();
                let _ = stderr.read_to_string(&mut s);
                s
            });
            // A closed pipe here means it already quit; its exit status says why.
            let _ = stdin.write_all(patch.as_bytes());
            drop(stdin);
            let mut out = String::new();
            let _ = stdout.read_to_string(&mut out);
            let status = waiter.lock().unwrap().wait();
            let errors = errors.join().unwrap_or_default();
            let result = match status {
                Ok(s) if s.success() => match clean(&out) {
                    msg if msg.is_empty() => Err("Claude Code returned an empty message".to_owned()),
                    msg => Ok(msg),
                },
                Ok(s) => Err(format!("Claude Code {s}\n{}", first_nonempty(&errors, &out))),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(result);
            wake();
        });
        Ok(Self { child, rx })
    }

    pub fn poll(&self) -> Option<Result<String, String>> {
        self.rx.try_recv().ok()
    }
}

impl Drop for Draft {
    fn drop(&mut self) {
        // Fails harmlessly when it has already exited.
        let _ = self.child.lock().map(|mut c| c.kill());
    }
}

/// The instruction; the patch follows on stdin. `notes` is whatever is in the message box,
/// taken as the author's own words to build on.
pub fn draft_prompt(recent: &[&str], notes: &str, amend: bool) -> String {
    let mut s = String::from(
        "Write the git commit message for the change on stdin (a stat summary, then the patch). \
         Reply with the message only: no preamble, no code fences, no trailers. \
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
    s
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

fn first_nonempty<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.trim().is_empty() { b.trim() } else { a.trim() }
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
        let p = draft_prompt(&["Fix the thing"], "  group repos  ", true);
        assert!(p.contains("- Fix the thing") && p.contains("group repos") && p.contains("amended"));
        assert!(!draft_prompt(&[], "", false).contains("notes"));
    }
}
