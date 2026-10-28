//! The line commands of the editor, as plain functions over the text and the selection
//! (byte offsets): toggle comment, duplicate, move lines up and down, and the bracket
//! that matches the one at the caret. Each returns the one replacement to make, so the
//! editor applies it as a single step that undo takes back.

use std::ops::Range;

/// Replace `range` with `text`, then select `select` (in the new text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<usize>,
    pub text: String,
    pub select: Range<usize>,
}

/// The byte ranges of the lines, each with its line break.
fn lines(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            out.push(start..i + 1);
            start = i + 1;
        }
    }
    if start < text.len() || out.is_empty() {
        out.push(start..text.len());
    }
    out
}

/// The lines the selection touches (a selection ending at the start of a line leaves that
/// line out), as indexes into `ls`.
fn touched(ls: &[Range<usize>], sel: &Range<usize>) -> Range<usize> {
    let line_of = |at: usize| ls.iter().position(|l| at < l.end || l.end == l.start).unwrap_or(ls.len() - 1);
    let first = line_of(sel.start);
    let mut last = line_of(sel.end);
    if sel.end > sel.start && last > first && ls[last].start == sel.end {
        last -= 1;
    }
    first..last + 1
}

fn content<'a>(text: &'a str, l: &Range<usize>) -> &'a str {
    text[l.clone()].trim_end_matches(['\n', '\r'])
}

/// Comment the selected lines out with `prefix`, or back in when they all are.
pub fn toggle_comment(text: &str, sel: Range<usize>, prefix: &str) -> Edit {
    let ls = lines(text);
    let rows = touched(&ls, &sel);
    let block = ls[rows.start].start..ls[rows.end - 1].end;
    let body: Vec<&str> = rows.clone().map(|r| &text[ls[r].clone()]).collect();
    let filled: Vec<&&str> = body.iter().filter(|l| !l.trim().is_empty()).collect();
    let commented = !filled.is_empty() && filled.iter().all(|l| l.trim_start().starts_with(prefix));
    let indent = filled.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let mut out = String::with_capacity(block.len() + body.len() * (prefix.len() + 1));
    // How far each line's start moved, to carry the caret along.
    let mut shifts: Vec<(usize, isize, usize)> = Vec::new();
    for (i, line) in body.iter().enumerate() {
        let at = ls[rows.start + i].start;
        if line.trim().is_empty() {
            out.push_str(line);
            continue;
        }
        if commented {
            let lead = line.len() - line.trim_start().len();
            let rest = &line[lead + prefix.len()..];
            let space = rest.starts_with(' ') as usize;
            out.push_str(&line[..lead]);
            out.push_str(&rest[space..]);
            shifts.push((at + lead, -((prefix.len() + space) as isize), at));
        } else {
            out.push_str(&line[..indent]);
            out.push_str(prefix);
            out.push(' ');
            out.push_str(&line[indent..]);
            shifts.push((at + indent, (prefix.len() + 1) as isize, at));
        }
    }
    let map = |pos: usize| {
        let mut moved: isize = 0;
        for &(col, by, line_start) in &shifts {
            if pos >= col {
                moved += by;
            } else if pos >= line_start && by < 0 {
                // Inside a removed marker: to where it began.
                moved += (col as isize - pos as isize).max(by);
            }
        }
        (pos as isize + moved).max(block.start as isize) as usize
    };
    let select = if sel.is_empty() {
        let at = map(sel.start);
        at..at
    } else {
        map(sel.start)..map(sel.end)
    };
    Edit { range: block, text: out, select }
}

/// Copy the selection after itself (selecting the copy), or the caret's lines below them.
pub fn duplicate(text: &str, sel: Range<usize>) -> Edit {
    if !sel.is_empty() {
        let copy = text[sel.clone()].to_owned();
        let end = sel.end + copy.len();
        return Edit { range: sel.end..sel.end, text: copy, select: sel.end..end };
    }
    let ls = lines(text);
    let rows = touched(&ls, &sel);
    let block = ls[rows.start].start..ls[rows.end - 1].end;
    let mut copy = text[block.clone()].to_owned();
    let at = block.end;
    if !copy.ends_with('\n') {
        // The last line has no break: the copy goes after a new one.
        copy.insert_str(0, newline(text));
    }
    let caret = sel.start + copy.len();
    Edit { range: at..at, text: copy, select: caret..caret }
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Move the selected lines one up or down, the selection with them. `None` at the top or
/// the bottom of the text.
pub fn move_lines(text: &str, sel: Range<usize>, up: bool) -> Option<Edit> {
    let ls = lines(text);
    let rows = touched(&ls, &sel);
    if (up && rows.start == 0) || (!up && rows.end >= ls.len()) {
        return None;
    }
    let nl = newline(text);
    let (above, below) =
        if up { (rows.start - 1..rows.start, rows.clone()) } else { (rows.clone(), rows.end..rows.end + 1) };
    let range = ls[above.start].start..ls[below.end - 1].end;
    let piece = |r: Range<usize>| -> String { r.map(|i| format!("{}{nl}", content(text, &ls[i]))).collect() };
    let mut out = piece(below.clone()) + &piece(above.clone());
    // Keep the text's ending: no break at the end if there was none.
    if !text[range.clone()].ends_with('\n') {
        out.truncate(out.len() - nl.len());
    }
    let other_len: usize = if up {
        content(text, &ls[rows.start - 1]).len() + nl.len()
    } else {
        content(text, &ls[rows.end]).len() + nl.len()
    };
    let shift = |p: usize| if up { p - other_len } else { p + other_len };
    let select = shift(sel.start)..shift(sel.end);
    Some(Edit { range, text: out, select })
}

const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// The bracket at the caret (just before it, else just after) and the one matching it, as
/// byte offsets into `text`, looking no further than `limit` bytes.
pub fn matching_bracket(text: &str, caret: usize, limit: usize) -> Option<(usize, usize)> {
    let before = text[..caret].chars().next_back().map(|c| (caret - c.len_utf8(), c));
    let after = text[caret..].chars().next().map(|c| (caret, c));
    for (at, c) in [before, after].into_iter().flatten() {
        if let Some(&(open, close)) = PAIRS.iter().find(|(o, _)| *o == c) {
            let mut depth = 0usize;
            for (i, ch) in text[at..].char_indices().take_while(|(i, _)| *i < limit) {
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some((at, at + i));
                    }
                }
            }
            return None;
        }
        if let Some(&(open, close)) = PAIRS.iter().find(|(_, c2)| *c2 == c) {
            let mut depth = 0usize;
            for (i, ch) in text[..at + 1].char_indices().rev().take_while(|(i, _)| at - i < limit) {
                if ch == close {
                    depth += 1;
                } else if ch == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i, at));
                    }
                }
            }
            return None;
        }
    }
    None
}

/// How a language writes a line comment.
pub fn comment_prefix(language: &str) -> Option<&'static str> {
    match language {
        "java" | "kotlin" | "groovy" | "rust" | "javascript" | "typescript" | "go" | "c" | "cpp" | "scala" => {
            Some("//")
        }
        "yaml" | "properties" | "python" | "bash" | "toml" | "ruby" => Some("#"),
        "sql" => Some("--"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, e: &Edit) -> String {
        format!("{}{}{}", &text[..e.range.start], e.text, &text[e.range.end..])
    }

    #[test]
    fn comments_toggle_at_the_common_indent() {
        let text = "class A {\n    int x;\n        int y;\n}\n";
        let start = text.find("int x").unwrap();
        let end = text.find("int y").unwrap() + 2;
        let e = toggle_comment(text, start..end, "//");
        let out = apply(text, &e);
        assert_eq!(out, "class A {\n    // int x;\n    //     int y;\n}\n");
        let back = toggle_comment(&out, e.select.clone(), "//");
        assert_eq!(apply(&out, &back), text);
        // A caret stays on its character.
        let at = text.find("x;").unwrap();
        let e = toggle_comment(text, at..at, "//");
        assert_eq!(&apply(text, &e)[e.select.start..e.select.start + 2], "x;");
    }

    #[test]
    fn lines_duplicate_and_move() {
        let text = "a\nbb\nccc";
        let at = text.find("bb").unwrap() + 1;
        let e = duplicate(text, at..at);
        assert_eq!(apply(text, &e), "a\nbb\nbb\nccc");
        assert_eq!(e.select.start, at + 3, "the caret goes to the copy");
        let e = duplicate(text, 8..8);
        assert_eq!(apply(text, &e), "a\nbb\nccc\nccc", "the last line gets a break first");
        let e = duplicate(text, 0..1);
        assert_eq!((apply(text, &e).as_str(), e.select), ("aa\nbb\nccc", 1..2));

        let e = move_lines(text, at..at, true).unwrap();
        let up = apply(text, &e);
        assert_eq!(up, "bb\na\nccc");
        assert_eq!(&up[e.select.start..], "b\na\nccc");
        let e = move_lines(text, at..at, false).unwrap();
        assert_eq!(apply(text, &e), "a\nccc\nbb");
        assert!(move_lines(text, 0..0, true).is_none());
        assert!(move_lines(text, 8..8, false).is_none());
        // Several lines move together.
        let e = move_lines("1\n2\n3\n4\n", 2..5, false).unwrap();
        assert_eq!(apply("1\n2\n3\n4\n", &e), "1\n4\n2\n3\n");
    }

    #[test]
    fn brackets_find_their_match_both_ways() {
        let text = "f(a[1], { b(); })";
        assert_eq!(matching_bracket(text, 2, 1000), Some((1, 16)), "after the opening one");
        assert_eq!(matching_bracket(text, 17, 1000), Some((1, 16)), "after the closing one");
        assert_eq!(matching_bracket(text, 3, 1000), Some((3, 5)), "before one");
        assert_eq!(matching_bracket(text, 6, 1000), Some((3, 5)));
        assert_eq!(matching_bracket("x y", 1, 1000), None);
        assert_eq!(matching_bracket("(((", 1, 1000), None);
    }
}
