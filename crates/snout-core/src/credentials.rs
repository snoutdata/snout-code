//! The credential helpers a repository's Git runs, where each is set, and the programs each
//! calls; and the one deterministic repair: a helper that calls a program by a bare name Git's
//! `PATH` lacks (`gh`, installed in `/opt/homebrew/bin`) is made to call it by its full path,
//! in the file that set it. Credentials themselves stay with the system's own helper; this
//! reads and writes them only through `git credential`.

use std::ops::Range;
use std::path::{Path, PathBuf};

use serde::Serialize;
use snout_host::Host;

use crate::error::{Error, Result};
use crate::git::{run, run_raw, run_with_stdin};

/// Where a configuration entry comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigScope {
    System,
    Global,
    Local,
    Worktree,
    /// `-c` or `GIT_CONFIG_*`: not in a file, so not to be changed here.
    Command,
}

/// One `credential.helper` (or `credential.<url>.helper`) entry, in the order Git reads them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Helper {
    pub scope: ConfigScope,
    /// The file that sets it (absolute), when a file does.
    pub file: Option<PathBuf>,
    /// The key as Git prints it: `credential.helper`, `credential.https://github.com.helper`.
    pub key: String,
    /// The URL part of the key, for a helper that applies to some URLs only.
    pub url: Option<String>,
    pub value: String,
}

impl Helper {
    /// An empty value: the helpers read before it no longer apply.
    pub fn is_reset(&self) -> bool {
        self.value.trim().is_empty()
    }

    /// Whether it applies to a remote at `url`: every helper without a URL, else one whose URL's
    /// scheme and host match (and whose path, if it names one, starts the remote's).
    pub fn applies_to(&self, url: &str) -> bool {
        let Some(pattern) = &self.url else { return true };
        let split = |u: &str| -> Option<(String, String, String)> {
            let (scheme, rest) = u.split_once("://")?;
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = authority.rsplit('@').next().unwrap_or(authority).to_ascii_lowercase();
            Some((scheme.to_ascii_lowercase(), host, path.trim_end_matches('/').to_owned()))
        };
        match (split(pattern), split(url)) {
            (Some((ps, ph, pp)), Some((us, uh, up))) => ps == us && ph == uh && (pp.is_empty() || up.starts_with(&pp)),
            _ => false,
        }
    }

    /// The programs it runs, by name, with where the name is in the value. A shell helper
    /// (`!…`) runs what its commands name (not the shell's own builtins nor its functions); a
    /// helper named by a word runs `git-credential-<word>`; one named by a path runs that.
    pub fn programs(&self) -> Vec<(String, Range<usize>)> {
        let v = &self.value;
        if let Some(script) = v.strip_prefix('!') {
            return commands_in(script).into_iter().map(|(name, r)| (name, r.start + 1..r.end + 1)).collect();
        }
        let first = v.split_whitespace().next().unwrap_or("");
        if first.is_empty() {
            return Vec::new();
        }
        let start = v.find(first).unwrap_or(0);
        if first.contains('/') || first.contains('\\') {
            vec![(first.to_owned(), start..start + first.len())]
        } else {
            vec![(format!("git-credential-{first}"), start..start + first.len())]
        }
    }

    /// The value with the program called `name` (a shell helper's) called by `path` instead.
    pub fn calling(&self, name: &str, path: &Path) -> Option<String> {
        if !self.value.starts_with('!') {
            return None;
        }
        let word = shell_word(path);
        let spans: Vec<Range<usize>> = self.programs().into_iter().filter(|(n, _)| n == name).map(|(_, r)| r).collect();
        if spans.is_empty() {
            return None;
        }
        let mut out = self.value.clone();
        for r in spans.into_iter().rev() {
            out.replace_range(r, &word);
        }
        Some(out)
    }
}

/// `path` as one word of a shell command: as it is when it can be, else in single quotes (a
/// Windows path, whose backslashes the shell would take as escapes, or one with a space).
pub fn shell_word(path: &Path) -> String {
    let shown = path.to_string_lossy();
    if shown.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+:@%".contains(c)) {
        shown.into_owned()
    } else {
        format!("'{}'", shown.replace('\'', "'\\''"))
    }
}

/// What the shell runs itself, or names that are not programs.
const SHELL_OWN: &[&str] = &[
    "test", "[", "[[", "echo", "printf", "read", "exit", "return", "local", "export", "set", "unset", "shift", "true",
    "false", "if", "then", "else", "elif", "fi", "for", "do", "done", "while", "until", "case", "esac", "in",
    "function", "eval", "exec", "command", "cd", "builtin", "type", "declare", "typeset", ":", ".", "source", "!",
];

/// The commands a shell script runs, by name and place: the first word of each command, past
/// assignments, at the start, after `;`, `&&`, `||`, `|`, `{`, `(` and `$(`, and after the
/// keywords that start a command. Functions it defines (`f() {…}`) and builtins are left out.
fn commands_in(script: &str) -> Vec<(String, Range<usize>)> {
    let bytes = script.as_bytes();
    let mut words: Vec<(String, Range<usize>, bool)> = Vec::new(); // (word, span, starts a command)
    let mut at_start = true;
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match (quote, c) {
            (Some(b'\''), b'\'') | (Some(b'"'), b'"') => {
                quote = None;
                i += 1;
                continue;
            }
            (Some(b'\''), _) => {
                i += 1;
                continue;
            }
            (None, b'\'') | (None, b'"') => {
                quote = Some(c);
                i += 1;
                continue;
            }
            _ => {}
        }
        if c == b'$' && bytes.get(i + 1) == Some(&b'(') {
            at_start = true;
            i += 2;
            continue;
        }
        if quote.is_some() && c != b'$' && !is_word(c) {
            i += 1;
            continue;
        }
        if matches!(c, b';' | b'&' | b'|' | b'{' | b'(' | b'\n') {
            at_start = true;
            i += 1;
            continue;
        }
        if matches!(c, b'}' | b')') || c.is_ascii_whitespace() || c == b'$' {
            if c == b'$' {
                // A variable: not a command.
                i += 1;
                while i < bytes.len() && (is_word(bytes[i]) || bytes[i] == b'{' || bytes[i] == b'}') {
                    i += 1;
                }
                at_start = false;
                continue;
            }
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (is_word(bytes[i]) || bytes[i] == b'=') {
            i += 1;
        }
        if i == start {
            i += 1;
            continue;
        }
        let word = &script[start..i];
        let assignment = word.contains('=');
        words.push((word.to_owned(), start..i, at_start && !assignment));
        if !assignment {
            at_start = matches!(word, "then" | "else" | "do" | "if" | "while" | "until" | "!" | "exec" | "command");
        }
    }
    // A name followed by `()` is a function being defined; calling it is no program either.
    let defined: Vec<String> = words
        .iter()
        .filter(|(_, r, _)| script[r.end..].trim_start().starts_with("()"))
        .map(|(w, _, _)| w.clone())
        .collect();
    words
        .into_iter()
        .filter(|(w, _, cmd)| *cmd && !SHELL_OWN.contains(&w.as_str()) && !defined.contains(w))
        .map(|(w, r, _)| (w, r))
        .collect()
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'/' | b'+' | b'@' | b'%' | b':' | b',' | b'~')
}

/// The helpers in `git config -z --show-scope --show-origin --get-regexp` output for the
/// repository at `repo`: `scope NUL origin NUL key LF value NUL`, entry after entry.
pub fn parse_helpers(out: &str, repo: &Path) -> Vec<Helper> {
    let fields: Vec<&str> = out.split('\0').collect();
    let mut helpers = Vec::new();
    for entry in fields.chunks(3) {
        let [scope, origin, kv] = entry else { continue };
        let (key, value) = kv.split_once('\n').unwrap_or((kv, ""));
        let scope = match *scope {
            "system" => ConfigScope::System,
            "global" => ConfigScope::Global,
            "local" => ConfigScope::Local,
            "worktree" => ConfigScope::Worktree,
            _ => ConfigScope::Command,
        };
        let file = origin.strip_prefix("file:").map(|f| {
            let f = PathBuf::from(f);
            if f.is_absolute() { f } else { repo.join(f) }
        });
        let url = key
            .strip_prefix("credential.")
            .and_then(|k| k.strip_suffix(".helper"))
            .filter(|u| !u.is_empty())
            .map(str::to_owned);
        helpers.push(Helper { scope, file, key: key.to_owned(), url, value: value.to_owned() });
    }
    helpers
}

/// The credential helpers Git reads in `repo`, in its order.
pub fn helpers(host: &dyn Host, repo: &Path) -> Result<Vec<Helper>> {
    let out = run_raw(
        host,
        repo,
        &["config", "-z", "--show-scope", "--show-origin", "--get-regexp", r"^credential\..*helper$"],
        None,
    )?;
    // Exit status 1: none set.
    Ok(parse_helpers(&String::from_utf8_lossy(&out.stdout), repo))
}

/// The helpers that answer for a remote at `url`: those that apply to it, from the last empty
/// value on (an empty value drops the ones before it).
pub fn in_effect<'a>(helpers: &'a [Helper], url: &str) -> Vec<&'a Helper> {
    let applying: Vec<&Helper> = helpers.iter().filter(|h| h.applies_to(url)).collect();
    let from = applying.iter().rposition(|h| h.is_reset()).map_or(0, |i| i + 1);
    applying[from..].to_vec()
}

/// Make `helper` say `value` instead, in the file that set it.
pub fn set_helper(host: &dyn Host, repo: &Path, helper: &Helper, value: &str) -> Result<()> {
    let Some(file) = &helper.file else {
        return Err(Error::Git {
            args: "config".into(),
            message: "the helper is set on the command line, not in a file".into(),
        });
    };
    let file = file.to_string_lossy();
    run(host, repo, &["config", "--file", &file, "--fixed-value", "--replace-all", &helper.key, value, &helper.value])?;
    Ok(())
}

/// The URL of the remote `name` in `repo`.
pub fn remote_url(host: &dyn Host, repo: &Path, name: &str) -> Option<String> {
    let out = run(host, repo, &["remote", "get-url", name]).ok()?;
    let url = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!url.is_empty()).then_some(url)
}

/// The request `git credential` reads for `url`, with `username` and `password` when given.
fn request(url: &str, username: Option<&str>, password: Option<&str>) -> String {
    let mut s = format!("url={url}\n");
    if let Some(u) = username {
        s.push_str(&format!("username={u}\n"));
    }
    if let Some(p) = password {
        s.push_str(&format!("password={p}\n"));
    }
    s.push('\n');
    s
}

/// The account the helpers give for `url`, without asking anyone: `None` when none gives one.
/// The secret is read and dropped, never returned.
pub fn account_for(host: &dyn Host, repo: &Path, url: &str) -> Option<String> {
    let out = run_raw(host, repo, &["credential", "fill"], Some(request(url, None, None).as_bytes())).ok()?;
    if !out.success {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let has_secret = text.lines().any(|l| l.starts_with("password=") && l.len() > "password=".len());
    let user = text.lines().find_map(|l| l.strip_prefix("username=")).map(str::to_owned);
    has_secret.then_some(user).flatten()
}

/// Hand `username` and `token` for `url` to the helpers that store (the system's keychain
/// helper), as Git does after a sign in that worked.
pub fn save(host: &dyn Host, repo: &Path, url: &str, username: &str, token: &str) -> Result<()> {
    run_with_stdin(host, repo, &["credential", "approve"], Some(request(url, Some(username), Some(token)).as_bytes()))?;
    Ok(())
}

/// Tell the helpers to forget what they stored for `url` and `username`.
pub fn forget(host: &dyn Host, repo: &Path, url: &str, username: &str) -> Result<()> {
    run_with_stdin(host, repo, &["credential", "reject"], Some(request(url, Some(username), None).as_bytes()))?;
    Ok(())
}

/// The helper that stores credentials in the system's keychain on `os`, as Git names it.
pub fn system_store(os: &str) -> &'static str {
    match os {
        "macos" => "osxkeychain",
        "windows" => "manager",
        _ => "libsecret",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GH: &str = r#"!f() { test "$1" = get && echo username=sarvidamed && echo "password=$(gh auth token -h github.com -u sarvidamed)"; }; f"#;

    fn helper(value: &str) -> Helper {
        Helper {
            scope: ConfigScope::Local,
            file: Some("/r/.git/config".into()),
            key: "credential.https://github.com.helper".into(),
            url: Some("https://github.com".into()),
            value: value.into(),
        }
    }

    #[test]
    fn a_shell_helper_names_the_programs_it_calls() {
        let names: Vec<String> = helper(GH).programs().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["gh"]);
        let names = |v: &str| helper(v).programs().into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names("!gh auth git-credential"), ["gh"]);
        assert_eq!(names("!/opt/homebrew/bin/gh auth git-credential"), ["/opt/homebrew/bin/gh"]);
        assert_eq!(names("!FOO=1 op read x | tr -d '\\n'"), ["op", "tr"]);
        assert_eq!(names("osxkeychain"), ["git-credential-osxkeychain"]);
        assert_eq!(names("/usr/local/bin/git-credential-manager"), ["/usr/local/bin/git-credential-manager"]);
        assert!(names("").is_empty());
    }

    #[test]
    fn the_program_is_called_by_its_full_path_and_nothing_else_changes() {
        let fixed = helper(GH).calling("gh", Path::new("/opt/homebrew/bin/gh")).unwrap();
        assert_eq!(fixed, GH.replace("$(gh ", "$(/opt/homebrew/bin/gh "));
        let spaced = helper("!gh auth git-credential").calling("gh", Path::new("/Apps/My Tools/gh")).unwrap();
        assert_eq!(spaced, "!'/Apps/My Tools/gh' auth git-credential");
        // A Windows path: its backslashes would be escapes to the shell Git runs the helper in.
        assert_eq!(
            shell_word(Path::new(r"C:\Program Files\GitHub CLI\gh.exe")),
            r"'C:\Program Files\GitHub CLI\gh.exe'"
        );
        assert_eq!(shell_word(Path::new("/opt/homebrew/bin/gh")), "/opt/homebrew/bin/gh");
        assert_eq!(helper("osxkeychain").calling("git-credential-osxkeychain", Path::new("/x")), None);
        assert_eq!(helper(GH).calling("op", Path::new("/x")), None);
    }

    #[test]
    fn git_configs_listing_reads_as_helpers_with_their_files() {
        let out = "global\0file:/Users/j/.gitconfig\0credential.helper\nosxkeychain\0\
                   local\0file:.git/config\0credential.https://github.com.helper\n\0\
                   local\0file:.git/config\0credential.https://github.com.helper\n!gh auth git-credential\0";
        let h = parse_helpers(out, Path::new("/r"));
        assert_eq!(h.len(), 3);
        assert_eq!((h[0].scope, h[0].url.as_deref(), h[0].value.as_str()), (ConfigScope::Global, None, "osxkeychain"));
        assert_eq!(h[1].file.as_deref(), Some(Path::new("/r/.git/config")));
        assert!(h[1].is_reset());
        assert_eq!(h[2].url.as_deref(), Some("https://github.com"));
        // The empty value drops the global helper for github.com, not for another server.
        let on_github: Vec<&str> =
            in_effect(&h, "https://github.com/joel/sarvida.git").iter().map(|h| h.value.as_str()).collect();
        assert_eq!(on_github, ["!gh auth git-credential"]);
        let elsewhere: Vec<&str> = in_effect(&h, "https://gitlab.com/x.git").iter().map(|h| h.value.as_str()).collect();
        assert_eq!(elsewhere, ["osxkeychain"]);
    }

    #[test]
    fn a_repositorys_helper_is_read_and_repaired_in_its_own_file() {
        let dir = std::env::temp_dir().join(format!("snout-credentials-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let host = snout_host::local();
        run(&*host, &dir, &["init", "-q"]).unwrap();
        let key = "credential.https://github.com.helper";
        run(&*host, &dir, &["config", "--local", "--add", key, ""]).unwrap();
        run(&*host, &dir, &["config", "--local", "--add", key, GH]).unwrap();
        let all = helpers(&*host, &dir).unwrap();
        let ours: Vec<&Helper> = all.iter().filter(|h| h.scope == ConfigScope::Local).collect();
        assert_eq!(ours.len(), 2);
        let gh = ours[1];
        assert_eq!(gh.value, GH);
        assert_eq!(gh.file.as_deref().map(|f| f.ends_with(".git/config")), Some(true));
        let fixed = gh.calling("gh", Path::new("/opt/homebrew/bin/gh")).unwrap();
        set_helper(&*host, &dir, gh, &fixed).unwrap();
        let after: Vec<String> = helpers(&*host, &dir)
            .unwrap()
            .into_iter()
            .filter(|h| h.scope == ConfigScope::Local)
            .map(|h| h.value)
            .collect();
        assert_eq!(after, ["".to_owned(), fixed], "the reset stays, the helper calls gh by its path");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
