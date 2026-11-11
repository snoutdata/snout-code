//! The `*` of Find in Files. Code is full of a `*` that means itself (`import java.util.*`,
//! `/**`, `a * b`), so only one that touches a letter, a digit or `_` is a wildcard
//! (`get*Name`, `Token*`): it stands for whatever is on the line there. A match that is the
//! text as typed ranks before one the wildcard widened.

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Which characters of `text` are wildcards.
fn wild(text: &str) -> Vec<bool> {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len())
        .map(|i| chars[i] == '*' && (is_word(i.checked_sub(1).map(|j| chars[j])) || is_word(chars.get(i + 1).copied())))
        .collect()
}

/// The regular expression a plain (not a regular expression) text makes when it holds a
/// wildcard `*`, the rest of it taken as written. None when it holds none.
pub fn wildcard_regex(text: &str) -> Option<String> {
    let wild = wild(text);
    if !wild.contains(&true) {
        return None;
    }
    let mut out = String::new();
    let mut run = String::new();
    for (c, w) in text.chars().zip(wild) {
        if w {
            out.push_str(&regex_escape(&std::mem::take(&mut run)));
            if !out.ends_with("[^\\n]*?") {
                out.push_str("[^\\n]*?");
            }
        } else {
            run.push(c);
        }
    }
    out.push_str(&regex_escape(&run));
    Some(out)
}

/// The runs of `text` between its wildcards, each written as is: every match holds them all.
pub fn literal_runs(text: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    for (c, w) in text.chars().zip(wild(text)) {
        if w {
            out.push(String::new());
        } else {
            out.last_mut().expect("never empty").push(c);
        }
    }
    out.retain(|r| !r.is_empty());
    out
}

/// `s` with the characters a regular expression gives a meaning escaped.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
