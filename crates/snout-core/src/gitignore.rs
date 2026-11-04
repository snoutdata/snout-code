//! Add to .gitignore: the rules a file of the repository can be ignored by (itself, its
//! folder, its extension), written so Git reads them as meant, and appended to the
//! repository's root `.gitignore`.

use crate::{Repo, Result};

/// One way to ignore a file: the rule, and what the menu calls it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgnoreRule {
    /// The line for `.gitignore` (`/src/A.sql`, `/build/`, `*.sql`).
    pub pattern: String,
    /// What it ignores, for the menu (`src/A.sql`, `build/`, `All .sql files`).
    pub label: String,
}

/// `text` with the characters a `.gitignore` line reads as glob or syntax escaped: `*`,
/// `?`, `[` and `\` anywhere, `#` and `!` at its start, a space at its end.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.chars().enumerate() {
        if matches!(c, '*' | '?' | '[' | '\\') || (i == 0 && matches!(c, '#' | '!')) {
            out.push('\\');
        }
        out.push(c);
    }
    if out.ends_with(' ') {
        out.pop();
        out.push_str("\\ ");
    }
    out
}

/// The rules the files at `paths` (relative to the repository's root, `/`-separated) can
/// be ignored by: the files themselves; their folder, when they share one below the root;
/// their extension, when they share one.
pub fn ignore_rules(paths: &[String]) -> Vec<IgnoreRule> {
    if paths.is_empty() {
        return Vec::new();
    }
    let mut rules = Vec::new();
    // The files, anchored at the root so a file of the same name elsewhere stays.
    if let [one] = paths {
        rules.push(IgnoreRule { pattern: format!("/{}", escape(one)), label: one.clone() });
    } else {
        rules.push(IgnoreRule {
            pattern: paths.iter().map(|p| format!("/{}", escape(p))).collect::<Vec<_>>().join("\n"),
            label: format!("These {} files", paths.len()),
        });
    }
    let dir = |p: &str| p.rsplit_once('/').map(|(d, _)| d.to_owned());
    if let Some(d) = dir(&paths[0]).filter(|d| paths.iter().all(|p| dir(p).as_deref() == Some(d))) {
        rules.push(IgnoreRule { pattern: format!("/{}/", escape(&d)), label: format!("{d}/") });
    }
    let ext = |p: &str| {
        let name = p.rsplit('/').next().unwrap_or(p);
        name.rsplit_once('.').filter(|(stem, e)| !stem.is_empty() && !e.is_empty()).map(|(_, e)| e.to_owned())
    };
    if let Some(e) = ext(&paths[0]).filter(|e| paths.iter().all(|p| ext(p).as_deref() == Some(e))) {
        rules.push(IgnoreRule { pattern: format!("*.{}", escape(&e)), label: format!("All .{e} files") });
    }
    rules
}

impl Repo {
    /// Append `pattern` (one rule, or several on their own lines) to the root `.gitignore`,
    /// made if there is none; lines it already has are not added again. How many lines
    /// were added.
    pub fn add_to_gitignore(&self, pattern: &str) -> Result<usize> {
        let file = self.root().join(".gitignore");
        let host = self.host();
        let text = match host.read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let have: std::collections::HashSet<&str> = text.lines().map(str::trim_end).collect();
        let new: Vec<&str> = pattern.lines().filter(|l| !l.is_empty() && !have.contains(l)).collect();
        if new.is_empty() {
            return Ok(0);
        }
        let mut add = String::new();
        if !text.is_empty() && !text.ends_with('\n') {
            add.push('\n');
        }
        for line in &new {
            add.push_str(line);
            add.push('\n');
        }
        host.write(&file, format!("{text}{add}").as_bytes())?;
        Ok(new.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_its_folder_and_its_extension() {
        let r = ignore_rules(&["db/2026-11-04-SMS.sql".into()]);
        let pats: Vec<&str> = r.iter().map(|r| r.pattern.as_str()).collect();
        assert_eq!(pats, ["/db/2026-11-04-SMS.sql", "/db/", "*.sql"]);
        assert_eq!(r[2].label, "All .sql files");
        // At the root: no folder; no extension for a dotfile or a name without one.
        let r = ignore_rules(&[".env".into()]);
        assert_eq!(r.iter().map(|r| r.pattern.as_str()).collect::<Vec<_>>(), ["/.env"]);
        let r = ignore_rules(&["Dockerfile".into()]);
        assert_eq!(r.len(), 1);
        // Several: the files, then what they share.
        let r = ignore_rules(&["a/x.log".into(), "a/y.log".into()]);
        assert_eq!(r[0].pattern, "/a/x.log\n/a/y.log");
        assert_eq!(r[0].label, "These 2 files");
        assert_eq!((r[1].pattern.as_str(), r[2].pattern.as_str()), ("/a/", "*.log"));
        let r = ignore_rules(&["a/x.log".into(), "b/y.txt".into()]);
        assert_eq!(r.len(), 1, "nothing shared");
    }

    #[test]
    fn names_git_would_read_as_patterns_are_escaped() {
        assert_eq!(ignore_rules(&["#notes".into()])[0].pattern, "/\\#notes");
        assert_eq!(ignore_rules(&["!x".into()])[0].pattern, "/\\!x");
        assert_eq!(ignore_rules(&["a[1]*.txt".into()])[0].pattern, "/a\\[1]\\*.txt");
        assert_eq!(ignore_rules(&["trailing ".into()])[0].pattern, "/trailing\\ ");
    }

    #[test]
    fn rules_are_appended_once_and_git_ignores_the_file() {
        use crate::git::test_run as run;
        let dir = std::env::temp_dir().join(format!("snout-test-gitignore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("db")).unwrap();
        run(&dir, &["init", "-q", "-b", "main"]).unwrap();
        std::fs::write(dir.join(".gitignore"), "target").unwrap();
        std::fs::write(dir.join("db/a.sql"), "select 1;").unwrap();
        let repo = Repo::open(&snout_host::local(), &dir).unwrap();
        assert!(repo.status().unwrap().files.iter().any(|f| f.path == "db/a.sql"));
        let rule = &ignore_rules(&["db/a.sql".into()])[2];
        assert_eq!(repo.add_to_gitignore(&rule.pattern).unwrap(), 1);
        assert_eq!(repo.add_to_gitignore(&rule.pattern).unwrap(), 0, "once");
        assert_eq!(std::fs::read_to_string(dir.join(".gitignore")).unwrap(), "target\n*.sql\n");
        assert!(!repo.status().unwrap().files.iter().any(|f| f.path == "db/a.sql"), "Git ignores it");
        let _ = std::fs::remove_dir_all(dir);
    }
}
