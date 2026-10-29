//! Fixes for problems, as the hover card, the gutter's bulb, the context actions (⌥↩) and the
//! Problems window offer them: the engine's code actions sorted into quick fixes and the rest,
//! the best fix of a problem, Fix all in file (the same fix on every problem of its kind, or
//! the engine's own fix-all action), and the problem put into words for a coding agent.

use std::collections::HashMap;

use lsp_types::{CodeAction, Diagnostic, DiagnosticSeverity, Position, Range, TextEdit, Uri, WorkspaceEdit};

/// What an engine's code action is to the lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// A fix for a problem (`quickfix`).
    Fix,
    /// The engine's own fix of every problem of the file (`source.fixAll`).
    FixAll,
    /// Anything else: a refactoring, a source action.
    Other,
}

/// What `a` is.
pub fn sort_of(a: &CodeAction) -> Sort {
    let kind = a.kind.as_ref().map(|k| k.as_str()).unwrap_or_default();
    if kind == "source.fixAll" || kind.starts_with("source.fixAll.") {
        Sort::FixAll
    } else if kind == "quickfix"
        || kind.starts_with("quickfix.")
        || (kind.is_empty() && a.diagnostics.as_ref().is_some_and(|d| !d.is_empty()))
    {
        Sort::Fix
    } else {
        Sort::Other
    }
}

/// The actions that can be applied (the engine leaves some in, disabled, to say why not).
pub fn usable(list: Vec<CodeAction>) -> Vec<CodeAction> {
    list.into_iter().filter(|a| a.disabled.is_none()).collect()
}

/// The fix to offer first: the one the engine prefers, else its first.
pub fn best_fix(list: &[CodeAction]) -> Option<&CodeAction> {
    let fixes = || list.iter().filter(|a| a.disabled.is_none() && sort_of(a) == Sort::Fix);
    fixes().find(|a| a.is_preferred == Some(true)).or_else(|| fixes().next())
}

fn key(p: &Position) -> (u32, u32) {
    (p.line, p.character)
}

/// The problems at `at` (a protocol position): those whose range holds it; an empty range
/// counts from its start to `word_end` (the end of the word there).
pub fn at_position(problems: &[Diagnostic], at: Position, word_end: Position) -> Vec<Diagnostic> {
    problems
        .iter()
        .filter(|d| {
            let (s, e, at) = (key(&d.range.start), key(&d.range.end), key(&at));
            if s == e { s <= at && at <= key(&word_end).max(s) } else { s <= at && at < e }
        })
        .cloned()
        .collect()
}

/// The problems that start on `line`.
pub fn on_line(problems: &[Diagnostic], line: u32) -> Vec<Diagnostic> {
    problems.iter().filter(|d| d.range.start.line == line).cloned().collect()
}

/// How bad: 0 an error, 1 a warning, 2 information, 3 a hint.
pub fn rank(d: &Diagnostic) -> u8 {
    match d.severity {
        Some(DiagnosticSeverity::WARNING) => 1,
        Some(DiagnosticSeverity::INFORMATION) => 2,
        Some(DiagnosticSeverity::HINT) => 3,
        _ => 0,
    }
}

/// `problems` worst first, then in the order they come.
pub fn worst_first(mut problems: Vec<Diagnostic>) -> Vec<Diagnostic> {
    problems.sort_by_key(rank);
    problems
}

/// Two problems of the same kind: the same source and code, or (with no code) the same
/// message.
pub fn same_kind(a: &Diagnostic, b: &Diagnostic) -> bool {
    a.source == b.source
        && match (&a.code, &b.code) {
            (Some(x), Some(y)) => x == y,
            (None, None) => a.message == b.message,
            _ => false,
        }
}

/// A fix's title with what changes from problem to problem left out: quoted names and the
/// parts in parentheses (`Import 'LocalDate' (java.time)` is `Import '…' (…)`).
pub fn shape(title: &str) -> String {
    let mut out = String::new();
    let mut close: Option<char> = None;
    for c in title.chars() {
        match close {
            Some(end) if c == end => {
                out.push('…');
                out.push(c);
                close = None;
            }
            Some(_) => {}
            None => {
                out.push(c);
                close = match c {
                    '\'' => Some('\''),
                    '"' => Some('"'),
                    '\u{2018}' => Some('\u{2019}'),
                    '\u{201c}' => Some('\u{201d}'),
                    '(' => Some(')'),
                    _ => None,
                };
            }
        }
    }
    if close.is_some() {
        // An opening mark that never closes was no quote.
        return title.to_owned();
    }
    out
}

/// What a fix is called in "Fix all ‘…’ in file": its title, with the names that change from
/// problem to problem as `…` and the parts in parentheses left out.
pub fn fix_all_name(title: &str) -> String {
    let s = shape(title);
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' if chars.peek() == Some(&'…') => {
                chars.next();
                chars.next();
            }
            '\'' | '"' | '\u{2018}' | '\u{201c}' if chars.peek() == Some(&'…') => {
                chars.next();
                chars.next();
                out.push('…');
            }
            c => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The same fix on every problem of a kind in a file.
#[derive(Clone, Debug, PartialEq)]
pub struct FixAll {
    /// "Fix all ‘Remove unused import’ in file".
    pub label: String,
    /// The fix as it was offered for the problem it was asked for, and its shape.
    pub title: String,
    pub shape: String,
    /// Every problem of the kind in the file.
    pub targets: Vec<Diagnostic>,
}

/// The Fix all offers for the fixes `fixes` of the problems `here`, among the file's
/// `problems`: one for each fix whose problem has others of its kind in the file, when its
/// shape tells it from the problem's other fixes.
pub fn fix_all_offers(problems: &[Diagnostic], here: &[Diagnostic], fixes: &[CodeAction]) -> Vec<FixAll> {
    let mut out: Vec<FixAll> = Vec::new();
    let fixes: Vec<&CodeAction> = fixes.iter().filter(|a| a.disabled.is_none() && sort_of(a) == Sort::Fix).collect();
    for fix in &fixes {
        let of = fix.diagnostics.as_ref().and_then(|d| d.first()).or_else(|| here.first());
        let Some(of) = of else { continue };
        let targets: Vec<Diagnostic> = problems.iter().filter(|d| same_kind(d, of)).cloned().collect();
        if targets.len() < 2 {
            continue;
        }
        let shape = shape(&fix.title);
        // Two fixes of one problem alike but for their names (an import from one package
        // or another): which one each problem takes cannot be told.
        let alike = fixes
            .iter()
            .filter(|f| {
                let theirs = f.diagnostics.as_ref().and_then(|d| d.first()).or_else(|| here.first());
                theirs == Some(of) && self::shape(&f.title) == shape
            })
            .count();
        if alike > 1 {
            continue;
        }
        let label = format!("Fix all \u{2018}{}\u{2019} in file", fix_all_name(&fix.title));
        if out.iter().any(|o| o.label == label) {
            continue;
        }
        out.push(FixAll { label, title: fix.title.clone(), shape, targets });
    }
    out
}

/// The fix `all` takes for one of its problems, among the actions the engine offers there:
/// the same title, else the one fix of the same shape.
pub fn pick_for<'a>(all: &FixAll, offered: &'a [CodeAction]) -> Option<&'a CodeAction> {
    let fixes: Vec<&CodeAction> = offered.iter().filter(|a| a.disabled.is_none() && sort_of(a) == Sort::Fix).collect();
    if let Some(same) = fixes.iter().find(|a| a.title == all.title) {
        return Some(same);
    }
    let mut alike = fixes.into_iter().filter(|a| shape(&a.title) == all.shape);
    match (alike.next(), alike.next()) {
        (Some(one), None) => Some(one),
        _ => None,
    }
}

/// The edits of Fix all `all` in the file `uri`, from what the engine offered for each of
/// its problems (`answers`, in the order of its targets), and how many problems they fix.
pub fn fix_all_from(all: &FixAll, uri: &Uri, answers: Vec<Vec<CodeAction>>) -> (Vec<TextEdit>, usize) {
    let per: Vec<Vec<TextEdit>> = answers
        .iter()
        .map(|offered| {
            pick_for(all, offered).and_then(|a| edit_of(a).0).and_then(|e| edits_in(&e, uri)).unwrap_or_default()
        })
        .collect();
    merge(per)
}

/// The edit an action makes, also when the engine hands it over as a command for the
/// client to apply (`java.apply.workspaceEdit`); and the command left to run.
pub fn edit_of(action: &CodeAction) -> (Option<WorkspaceEdit>, Option<lsp_types::Command>) {
    match (&action.edit, &action.command) {
        (Some(edit), command) => (Some(edit.clone()), command.clone()),
        (None, Some(c)) if c.command == "java.apply.workspaceEdit" => {
            let edit =
                c.arguments.as_ref().and_then(|a| a.first()).and_then(|v| serde_json::from_value(v.clone()).ok());
            (edit, None)
        }
        (None, command) => (None, command.clone()),
    }
}

/// The text edits `edit` makes to the file `uri`, when it changes that file and nothing else.
#[allow(clippy::mutable_key_type)] // lsp-types keys its edits by URI
pub fn edits_in(edit: &WorkspaceEdit, uri: &Uri) -> Option<Vec<TextEdit>> {
    let path = snout_lang::uri::to_path(uri);
    let mut out = Vec::new();
    for step in super::edits::steps(edit.clone()) {
        match step {
            super::edits::Step::Edit(p, list) if Some(&p) == path.as_ref() => out.extend(list),
            _ => return None,
        }
    }
    // A URI that is no file path: the plain changes, when they are of it alone.
    if path.is_none() {
        let changes = edit.changes.as_ref()?;
        if changes.len() != 1 {
            return None;
        }
        return changes.get(uri).cloned();
    }
    Some(out)
}

fn overlaps(a: &Range, b: &Range) -> bool {
    let (a0, a1, b0, b1) = (key(&a.start), key(&a.end), key(&b.start), key(&b.end));
    // Two insertions at one place do not overlap; an insertion inside a replacement does.
    a0 < b1 && b0 < a1 || (a0 == a1 && b0 < a0 && a0 < b1) || (b0 == b1 && a0 < b0 && b0 < a1)
}

/// The edits of several fixes of one file (each computed on the same text) as one: an edit
/// two fixes both make is made once; a fix whose edits clash with those of a fix before it
/// is left out. Also how many fixes went in.
pub fn merge(fixes: Vec<Vec<TextEdit>>) -> (Vec<TextEdit>, usize) {
    let mut kept: Vec<TextEdit> = Vec::new();
    let mut taken = 0;
    for edits in fixes {
        if edits.is_empty() {
            continue;
        }
        let new: Vec<&TextEdit> = edits.iter().filter(|e| !kept.contains(e)).collect();
        if new.iter().any(|e| kept.iter().any(|k| overlaps(&k.range, &e.range))) {
            continue;
        }
        kept.extend(new.into_iter().cloned());
        taken += 1;
    }
    (kept, taken)
}

/// One workspace edit of `edits` to `uri`.
#[allow(clippy::mutable_key_type)] // lsp-types keys its edits by URI
pub fn file_edit(uri: &Uri, edits: Vec<TextEdit>) -> WorkspaceEdit {
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    WorkspaceEdit { changes: Some(changes), ..Default::default() }
}

/// What a problem is called: error, warning, note.
pub fn severity_word(d: &Diagnostic) -> &'static str {
    match rank(d) {
        0 => "error",
        1 => "warning",
        _ => "problem",
    }
}

/// The lines of a problem (0-based, inclusive).
pub fn lines_of(d: &Diagnostic) -> (u32, u32) {
    let end = if d.range.end.character == 0 && d.range.end.line > d.range.start.line {
        d.range.end.line - 1
    } else {
        d.range.end.line
    };
    (d.range.start.line, end.max(d.range.start.line))
}

/// What a coding agent is asked, after the mention of the file and lines.
pub fn ask_text(d: &Diagnostic) -> String {
    let message = d.message.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("Fix this {}: {message}", severity_word(d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{CodeActionKind, NumberOrString};

    fn diag(line: u32, a: u32, b: u32, code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(line, a), Position::new(line, b)),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(code.into())),
            source: Some("Java".into()),
            message: message.into(),
            ..Default::default()
        }
    }

    fn fix(title: &str, d: &Diagnostic) -> CodeAction {
        CodeAction {
            title: title.into(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![d.clone()]),
            ..Default::default()
        }
    }

    #[test]
    fn sorts_actions_and_finds_the_best_fix() {
        let d = diag(3, 0, 4, "1", "x");
        let refactor =
            CodeAction { title: "Extract".into(), kind: Some("refactor.extract".into()), ..Default::default() };
        let all = CodeAction { title: "Fix all".into(), kind: Some("source.fixAll.java".into()), ..Default::default() };
        let mut preferred = fix("B", &d);
        preferred.is_preferred = Some(true);
        let list = vec![refactor.clone(), fix("A", &d), preferred, all.clone()];
        assert_eq!(sort_of(&refactor), Sort::Other);
        assert_eq!(sort_of(&all), Sort::FixAll);
        assert_eq!(best_fix(&list).unwrap().title, "B", "the engine's preferred fix first");
        assert_eq!(best_fix(&list[..2]).unwrap().title, "A");
        assert!(best_fix(&[refactor]).is_none());
    }

    #[test]
    fn shapes_titles_and_names_fix_all() {
        assert_eq!(shape("Import 'LocalDate' (java.time)"), "Import '…' (…)");
        assert_eq!(shape("Remove unused import"), "Remove unused import");
        assert_eq!(shape("it's open"), "it's open");
        assert_eq!(fix_all_name("Import 'LocalDate' (java.time)"), "Import …");
        assert_eq!(fix_all_name("Replace with RoundingMode.HALF_EVEN"), "Replace with RoundingMode.HALF_EVEN");
    }

    #[test]
    fn offers_fix_all_for_a_fix_whose_problem_has_others_of_its_kind() {
        let a = diag(2, 0, 20, "268435844", "The import java.util.List is never used");
        let b = diag(3, 0, 20, "268435844", "The import java.util.Map is never used");
        let other = diag(9, 4, 8, "16777218", "LocalDate cannot be resolved to a type");
        let problems = vec![a.clone(), b.clone(), other.clone()];
        let offers = fix_all_offers(&problems, std::slice::from_ref(&a), &[fix("Remove unused import", &a)]);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].label, "Fix all \u{2018}Remove unused import\u{2019} in file");
        assert_eq!(offers[0].targets, vec![a.clone(), b.clone()]);
        // A problem alone of its kind: nothing to fix all.
        assert!(
            fix_all_offers(&problems, std::slice::from_ref(&other), &[fix("Import 'LocalDate' (java.time)", &other)])
                .is_empty()
        );
        // Two fixes alike but for the name: which each problem takes is not known.
        let c = diag(10, 4, 8, "16777218", "List cannot be resolved to a type");
        let problems = vec![other.clone(), c.clone()];
        let ambiguous = [fix("Import 'List' (java.util)", &c), fix("Import 'List' (java.awt)", &c)];
        assert!(fix_all_offers(&problems, std::slice::from_ref(&c), &ambiguous).is_empty());
        let one = [fix("Import 'LocalDate' (java.time)", &other)];
        let offers = fix_all_offers(&problems, std::slice::from_ref(&other), &one);
        assert_eq!(offers[0].label, "Fix all \u{2018}Import …\u{2019} in file");
        // Each problem takes the one fix of that shape.
        let there = [fix("Import 'List' (java.util)", &c)];
        assert_eq!(pick_for(&offers[0], &there).unwrap().title, "Import 'List' (java.util)");
        assert!(pick_for(&offers[0], &ambiguous).is_none());
    }

    #[test]
    fn finds_the_problems_at_a_place() {
        let a = diag(1, 4, 9, "1", "a");
        let empty = diag(2, 6, 6, "2", "b");
        let all = vec![a.clone(), empty.clone()];
        assert_eq!(at_position(&all, Position::new(1, 4), Position::new(1, 9)), vec![a.clone()]);
        assert!(at_position(&all, Position::new(1, 9), Position::new(1, 9)).is_empty(), "the end is outside");
        assert_eq!(at_position(&all, Position::new(2, 8), Position::new(2, 10)), vec![empty.clone()]);
        assert_eq!(on_line(&all, 2), vec![empty]);
        assert_eq!(lines_of(&a), (1, 1));
        assert_eq!(ask_text(&a), "Fix this warning: a");
    }

    #[test]
    fn merges_the_edits_of_several_fixes() {
        let at = |l: u32, a: u32, b: u32, t: &str| TextEdit {
            range: Range::new(Position::new(l, a), Position::new(l, b)),
            new_text: t.into(),
        };
        let import = at(1, 0, 0, "import x.Q;\n");
        let (edits, n) = merge(vec![
            vec![at(5, 2, 6, "a"), import.clone()],
            vec![at(7, 2, 6, "b"), import.clone()],
            // Clashes with the first.
            vec![at(5, 4, 8, "c")],
            vec![],
        ]);
        assert_eq!(n, 2);
        assert_eq!(edits, vec![at(5, 2, 6, "a"), import, at(7, 2, 6, "b")], "the shared import once");
        // Two insertions at one place both go in.
        let (edits, n) = merge(vec![vec![at(1, 0, 0, "x")], vec![at(1, 0, 0, "y")]]);
        assert_eq!((edits.len(), n), (2, 2));
    }
}
