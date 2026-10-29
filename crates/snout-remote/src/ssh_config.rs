//! The hosts an ssh config file names, for the app's list to pick from: its `Host` lines
//! (patterns with wildcards or negations left out) and those of the files it `Include`s.

use std::path::{Path, PathBuf};

/// The user's ssh config: `~/.ssh/config`.
pub fn user_config() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| Path::new(&h).join(".ssh/config"))
}

/// The hosts `config` names, in the order they come, each once.
pub fn hosts(config: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let ssh_dir = config.parent().map(Path::to_path_buf).unwrap_or_default();
    read(config, &ssh_dir, &mut out, 0);
    out
}

fn read(file: &Path, ssh_dir: &Path, out: &mut Vec<String>, depth: u32) {
    if depth > 8 {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, rest) = match line.find(|c: char| c.is_whitespace() || c == '=') {
            Some(at) => (&line[..at], line[at..].trim_start_matches(|c: char| c.is_whitespace() || c == '=')),
            None => (line, ""),
        };
        let words = split_words(rest);
        if key.eq_ignore_ascii_case("host") {
            for w in words {
                if !w.contains(['*', '?', '!']) && !out.contains(&w) {
                    out.push(w);
                }
            }
        } else if key.eq_ignore_ascii_case("include") {
            for w in words {
                let w = match w.strip_prefix("~/") {
                    Some(rest) => std::env::var_os("HOME").map(|h| Path::new(&h).join(rest)).unwrap_or_default(),
                    None => PathBuf::from(w),
                };
                // Relative names are under ~/.ssh.
                let pattern = if w.is_absolute() { w } else { ssh_dir.join(w) };
                for f in glob(&pattern) {
                    read(&f, ssh_dir, out, depth + 1);
                }
            }
        }
    }
}

/// The words of a config line; a word may be in double quotes.
fn split_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The files a pattern names; `*` and `?` match within one path component.
fn glob(pattern: &Path) -> Vec<PathBuf> {
    let mut found = vec![PathBuf::from("/")];
    for part in pattern.components().skip(1) {
        let part = part.as_os_str().to_string_lossy().into_owned();
        let mut next = Vec::new();
        for base in &found {
            if part.contains(['*', '?']) {
                let Ok(entries) = std::fs::read_dir(base) else { continue };
                let mut names: Vec<String> =
                    entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
                names.sort();
                next.extend(names.into_iter().filter(|n| wildcard(&part, n)).map(|n| base.join(n)));
            } else {
                next.push(base.join(&part));
            }
        }
        found = next;
    }
    found.into_iter().filter(|p| p.is_file()).collect()
}

fn wildcard(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], n) || (!n.is_empty() && go(p, &n[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &n[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &n[1..]),
            _ => false,
        }
    }
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    // Hidden files only by name.
    !(name.starts_with('.') && !pattern.starts_with('.')) && go(&p, &n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_hosts_and_follows_includes() {
        let dir = std::env::temp_dir().join(format!("snout-ssh-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("conf.d")).unwrap();
        std::fs::write(
            dir.join("config"),
            "# mine\nHost devbox build-*\n  HostName devbox.example.com\nInclude conf.d/*.conf\n\
             Host *\n  ServerAliveInterval 30\nhost=\"quoted box\" !nope\nInclude missing/*\n",
        )
        .unwrap();
        std::fs::write(dir.join("conf.d/work.conf"), "Host work-a work-b\n  User me\nHost devbox\n").unwrap();
        std::fs::write(dir.join("conf.d/.hidden.conf"), "Host secret\n").unwrap();
        std::fs::write(dir.join("conf.d/notes.txt"), "Host notes\n").unwrap();
        assert_eq!(hosts(&dir.join("config")), ["devbox", "work-a", "work-b", "quoted box"]);
        assert!(hosts(&dir.join("absent")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
