//! Recognising, from git's error message, that a pull, fetch, push or clone failed because
//! Git could not sign in to the remote: a credential helper that cannot run (a repository
//! copied from another computer names a program not installed here), a token the server
//! refuses, no credentials at all, or an SSH key or host key the server or SSH do not accept.

use serde::Serialize;

/// Why Git could not sign in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SignInKind {
    /// The credential helper the configuration names failed to run; `program` is the one
    /// missing, when the message says.
    BrokenHelper { program: Option<String> },
    /// The server refused the user name and token or password Git sent.
    Rejected,
    /// Git had nothing to send: no helper gave a user name, and it may not ask.
    NoCredentials,
    /// The server says the repository is not there, which is also what it says to an
    /// account that may not see it.
    NoAccess,
    /// The SSH server accepted none of the keys offered.
    SshKey,
    /// SSH does not know the server's host key, or it changed.
    HostKey,
}

/// Git could not sign in to the remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignInProblem {
    #[serde(flatten)]
    pub kind: SignInKind,
    /// The server, as `github.com`, when the message names it.
    pub host: Option<String>,
}

impl SignInProblem {
    /// Recognise the problem from git's error message (git runs with `LC_ALL=C`, but what
    /// the remote and the helper say comes as they say it).
    pub fn from_error(message: &str) -> Option<Self> {
        let m = message;
        let has = |needles: &[&str]| needles.iter().any(|n| m.contains(n));
        let refused = has(&[
            "Authentication failed for",
            "Invalid username or token",
            "Invalid username or password",
            "HTTP Basic: Access denied",
            "The requested URL returned error: 401",
            "The requested URL returned error: 403",
        ]);
        let nothing = has(&["could not read Username", "could not read Password", "terminal prompts disabled"]);
        let kind = if (refused || nothing) && helper_failed(m) {
            SignInKind::BrokenHelper { program: missing_program(m) }
        } else if refused {
            SignInKind::Rejected
        } else if nothing {
            SignInKind::NoCredentials
        } else if has(&["Host key verification failed", "REMOTE HOST IDENTIFICATION HAS CHANGED"]) {
            SignInKind::HostKey
        } else if m.contains("Permission denied (publickey") {
            SignInKind::SshKey
        } else if m.contains("Repository not found") || (m.contains("fatal: repository '") && m.contains("' not found"))
        {
            SignInKind::NoAccess
        } else {
            return None;
        };
        Some(Self { kind, host: host_of(m) })
    }
}

/// The helper's own failure, among what git printed: its shell could not find a program, or
/// git could not find a `git-credential-*` command.
fn helper_failed(m: &str) -> bool {
    m.contains("command not found")
        || (m.contains("credential-") && m.contains("is not a git command"))
        || m.lines().any(|l| l.contains("credential") && l.contains("No such file or directory"))
}

/// The program the shell could not find: `gh` in `f get: gh: command not found`, or the
/// helper git could not, `git-credential-manager` in
/// `git: 'credential-manager' is not a git command`.
fn missing_program(m: &str) -> Option<String> {
    for line in m.lines() {
        // zsh puts the name after: `zsh:1: command not found: gh`.
        let name = match (line.find("command not found: "), line.find(": command not found")) {
            (Some(i), _) => Some(line[i + "command not found: ".len()..].trim()),
            // `sh: line 1: gh`, `f get: gh`: the last part before.
            (None, Some(i)) => line[..i].rsplit(':').next().map(str::trim),
            _ => None,
        };
        if let Some(name) = name
            && !name.is_empty()
            && !name.contains(' ')
        {
            return Some(name.to_owned());
        }
        if let Some(i) = line.find("'credential-")
            && line[i..].contains("is not a git command")
        {
            let rest = &line[i + 1..];
            let name = &rest[..rest.find('\'').unwrap_or(rest.len())];
            return Some(format!("git-{name}"));
        }
    }
    None
}

/// The server the message names: from a URL (`https://github.com/...`,
/// `for 'https://github.com'`), or SSH's `git@github.com: Permission denied`.
fn host_of(m: &str) -> Option<String> {
    for scheme in ["https://", "http://", "ssh://"] {
        if let Some(i) = m.find(scheme) {
            let rest = &m[i + scheme.len()..];
            let end = rest.find(['/', '\'', '"', ' ', '\n', ':']).unwrap_or(rest.len());
            let authority = &rest[..end];
            let host = authority.rsplit('@').next().unwrap_or(authority);
            if !host.is_empty() {
                return Some(host.to_owned());
            }
        }
    }
    for line in m.lines() {
        let line = line.trim();
        if let Some(i) = line.find(": Permission denied") {
            let who = &line[..i];
            if let Some((_, host)) = who.rsplit_once('@') {
                return Some(host.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_helper_copied_from_another_computer() {
        let m = "`git pull` failed: f() { test \"$1\" = get && echo username=demo && echo \
                 \"password=$(gh auth token -h github.com -u demo)\"; }; f get: gh: command not found\n\
                 remote: Invalid username or token. Password authentication is not supported for Git operations.\n\
                 fatal: Authentication failed for 'https://github.com/demo/demo.git/'";
        let p = SignInProblem::from_error(m).unwrap();
        assert_eq!(p.kind, SignInKind::BrokenHelper { program: Some("gh".into()) });
        assert_eq!(p.host.as_deref(), Some("github.com"));

        let zsh = "zsh:1: command not found: gh\nfatal: could not read Username for 'https://github.com': terminal prompts disabled";
        let p = SignInProblem::from_error(zsh).unwrap();
        assert_eq!(p.kind, SignInKind::BrokenHelper { program: Some("gh".into()) });
    }

    #[test]
    fn a_helper_git_cannot_find() {
        let m = "git: 'credential-manager' is not a git command. See 'git --help'.\n\
                 fatal: could not read Username for 'https://dev.azure.com': terminal prompts disabled";
        let p = SignInProblem::from_error(m).unwrap();
        assert_eq!(p.kind, SignInKind::BrokenHelper { program: Some("git-credential-manager".into()) });
        assert_eq!(p.host.as_deref(), Some("dev.azure.com"));
    }

    #[test]
    fn refused_missing_and_no_access() {
        let rejected =
            "remote: Invalid username or token.\nfatal: Authentication failed for 'https://git@gitlab.com/a/b.git/'";
        let p = SignInProblem::from_error(rejected).unwrap();
        assert_eq!((p.kind, p.host.as_deref()), (SignInKind::Rejected, Some("gitlab.com")));

        let none = "fatal: could not read Username for 'https://github.com': terminal prompts disabled";
        assert_eq!(SignInProblem::from_error(none).unwrap().kind, SignInKind::NoCredentials);

        let gone = "remote: Repository not found.\nfatal: repository 'https://github.com/a/b.git/' not found";
        assert_eq!(SignInProblem::from_error(gone).unwrap().kind, SignInKind::NoAccess);
    }

    #[test]
    fn ssh_keys() {
        let key = "git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.";
        let p = SignInProblem::from_error(key).unwrap();
        assert_eq!((p.kind, p.host.as_deref()), (SignInKind::SshKey, Some("github.com")));

        let host = "Host key verification failed.\nfatal: Could not read from remote repository.";
        assert_eq!(SignInProblem::from_error(host).unwrap().kind, SignInKind::HostKey);
    }

    #[test]
    fn other_failures_are_not_sign_in_problems() {
        for m in [
            "fatal: refusing to merge unrelated histories",
            " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs",
            "fatal: unable to access 'https://github.com/a/b.git/': Could not resolve host: github.com",
            "CONFLICT (content): Merge conflict in a.txt",
        ] {
            assert_eq!(SignInProblem::from_error(m), None, "{m}");
        }
    }
}
