//! A Java file's symbols as the Structure window and the File Structure popup show them:
//! the engine's `textDocument/documentSymbol` tree, each symbol with its visibility (read
//! off the declaration's modifiers, which the protocol leaves out), and the badge it is
//! drawn with.

use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, FontWeight, Hsla, IntoElement, ParentElement as _, Styled as _, div, px};
use lsp_types::{DocumentSymbol, Range, SymbolKind};
use serde_json::Value;

use crate::look;
use crate::theme::{Palette, mix};

/// Who may see a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Protected,
    Package,
    Private,
}

/// One symbol of a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    /// What the engine says beside the name: a method's or a field's type.
    pub detail: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub is_static: bool,
    /// The whole declaration, and its name (protocol positions).
    pub range: Range,
    pub selection: Range,
    pub children: Vec<Symbol>,
}

/// The symbols of `v` (a `documentSymbol` answer, nested) of the file `text`.
pub fn parse(v: &Value, text: &str) -> Vec<Symbol> {
    let list: Vec<DocumentSymbol> = serde_json::from_value(v.clone()).unwrap_or_default();
    let lines: Vec<usize> =
        std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect::<Vec<usize>>();
    let offset = |p: lsp_types::Position| -> usize {
        let start = lines.get(p.line as usize).copied().unwrap_or(text.len());
        let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
        let mut units = 0u32;
        for (i, c) in text[start..line_end].char_indices() {
            if units >= p.character {
                return start + i;
            }
            units += c.len_utf16() as u32;
        }
        line_end
    };
    fn walk(
        list: Vec<DocumentSymbol>,
        in_interface: bool,
        text: &str,
        offset: &dyn Fn(lsp_types::Position) -> usize,
    ) -> Vec<Symbol> {
        list.into_iter()
            // The package declaration and the imports are not the file's structure.
            .filter(|s| !matches!(s.kind, SymbolKind::PACKAGE | SymbolKind::NAMESPACE | SymbolKind::MODULE))
            .map(|s| {
                let (from, to) = (offset(s.range.start), offset(s.selection_range.start));
                let (mut visibility, is_static) = modifiers(text.get(from.min(to)..to).unwrap_or_default());
                // An interface's members, and an enum's constants, are public.
                if visibility == Visibility::Package && (in_interface || s.kind == SymbolKind::ENUM_MEMBER) {
                    visibility = Visibility::Public;
                }
                let interface = s.kind == SymbolKind::INTERFACE;
                Symbol {
                    name: s.name,
                    detail: s.detail.unwrap_or_default(),
                    kind: s.kind,
                    visibility,
                    is_static,
                    range: s.range,
                    selection: s.selection_range,
                    children: walk(s.children.unwrap_or_default(), interface, text, offset),
                }
            })
            .collect()
    }
    walk(list, false, text, &offset)
}

/// The visibility and `static` of a declaration, from its text before its name (comments,
/// annotations and their arguments are skipped).
pub fn modifiers(head: &str) -> (Visibility, bool) {
    let mut words: Vec<&str> = Vec::new();
    let b = head.as_bytes();
    let mut i = 0;
    let mut depth = 0i32;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let annotation = start > 0 && b[start - 1] == b'@';
                if depth == 0 && !annotation {
                    words.push(&head[start..i]);
                }
            }
            _ => i += 1,
        }
    }
    let visibility = if words.contains(&"public") {
        Visibility::Public
    } else if words.contains(&"protected") {
        Visibility::Protected
    } else if words.contains(&"private") {
        Visibility::Private
    } else {
        Visibility::Package
    };
    (visibility, words.contains(&"static"))
}

/// Whether `kind` is a type: a class, an interface, an enum or a record.
pub fn is_type(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::CLASS | SymbolKind::INTERFACE | SymbolKind::ENUM | SymbolKind::STRUCT)
}

/// The letter and hue (of [`Palette::hues`]) a symbol of `kind` is drawn with.
fn badge_of(kind: SymbolKind) -> (&'static str, usize) {
    match kind {
        SymbolKind::CLASS => ("C", 0),
        SymbolKind::INTERFACE => ("I", 1),
        SymbolKind::ENUM => ("E", 2),
        SymbolKind::STRUCT => ("R", 5),
        SymbolKind::METHOD | SymbolKind::FUNCTION => ("m", 7),
        SymbolKind::CONSTRUCTOR => ("c", 7),
        SymbolKind::FIELD | SymbolKind::PROPERTY | SymbolKind::VARIABLE => ("f", 2),
        SymbolKind::CONSTANT | SymbolKind::ENUM_MEMBER => ("f", 6),
        SymbolKind::PACKAGE | SymbolKind::NAMESPACE | SymbolKind::MODULE => ("p", 4),
        _ => ("•", 4),
    }
}

/// A symbol's badge: its kind's letter in a wash of its colour.
pub fn badge(kind: SymbolKind, p: &Palette) -> AnyElement {
    let (letter, hue) = badge_of(kind);
    let color = if p.tinted { p.hues[hue] } else { p.accent };
    div()
        .flex_none()
        .size(px(15.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(mix(color, if p.dark { 0.3 } else { 0.22 }, p.panel))
        .border_1()
        .border_color(mix(color, 0.45, p.panel))
        .text_color(color)
        .font_family(look::MONO)
        .font_weight(FontWeight::BOLD)
        .text_size(px(9.5))
        .line_height(px(15.))
        .child(letter)
        .into_any_element()
}

/// A member's visibility, as a small mark after its badge: a lock for private, a filled
/// dot for public, a half one for protected, a ring for package private.
pub fn visibility_mark(v: Visibility, p: &Palette) -> AnyElement {
    let dot = |fill: Option<Hsla>, ring: Hsla| {
        div().size(px(7.)).rounded_full().border_1().border_color(ring).when_some(fill, |d, f| d.bg(f))
    };
    let mark = match v {
        Visibility::Private => crate::glyphs::icon(crate::glyphs::Icon::Lock, 10., p.error).into_any_element(),
        Visibility::Public => dot(Some(p.ok), p.ok).into_any_element(),
        Visibility::Protected => dot(Some(mix(p.warn, 0.5, p.panel)), p.warn).into_any_element(),
        Visibility::Package => dot(None, p.fg_dim).into_any_element(),
    };
    div().flex_none().w(px(10.)).flex().items_center().justify_center().child(mark).into_any_element()
}

/// What a visibility is called, for tooltips and tests.
pub fn visibility_name(v: Visibility) -> &'static str {
    match v {
        Visibility::Public => "public",
        Visibility::Protected => "protected",
        Visibility::Package => "package private",
        Visibility::Private => "private",
    }
}

/// How a tree of symbols is ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    /// As in the file.
    #[default]
    Position,
    /// By name, types first.
    Name,
}

/// A row of a symbol tree: the symbol, how deep it is, and where in the tree (indexes from
/// the top, in the file's order).
#[derive(Clone, Debug)]
pub struct Row {
    pub path: Vec<usize>,
    pub depth: usize,
    pub symbol: Symbol,
    pub has_children: bool,
    pub open: bool,
}

/// The rows of `symbols`: those whose name matches `filter` (and the symbols around them),
/// ordered by `sort`, below the open ones (`closed` holds the paths folded; a filter opens
/// everything).
pub fn rows(symbols: &[Symbol], filter: &str, sort: Sort, closed: &dyn Fn(&[usize]) -> bool) -> Vec<Row> {
    use fuzzy_matcher::FuzzyMatcher as _;
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().ignore_case();
    let filter = filter.trim();
    let hit = |s: &Symbol| filter.is_empty() || matcher.fuzzy_match(&s.name, filter).is_some();
    fn keeps(s: &Symbol, hit: &dyn Fn(&Symbol) -> bool) -> bool {
        hit(s) || s.children.iter().any(|c| keeps(c, hit))
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        list: &[Symbol],
        path: &mut Vec<usize>,
        filter: bool,
        sort: Sort,
        hit: &dyn Fn(&Symbol) -> bool,
        closed: &dyn Fn(&[usize]) -> bool,
        out: &mut Vec<Row>,
    ) {
        let mut order: Vec<usize> = (0..list.len()).filter(|&i| keeps(&list[i], hit)).collect();
        if sort == Sort::Name {
            order.sort_by_key(|&i| (!is_type(list[i].kind), list[i].name.to_lowercase()));
        }
        for i in order {
            let s = &list[i];
            path.push(i);
            let open = filter || !closed(path);
            out.push(Row {
                path: path.clone(),
                depth: path.len() - 1,
                symbol: Symbol { children: Vec::new(), ..s.clone() },
                has_children: !s.children.is_empty(),
                open,
            });
            if open {
                walk(&s.children, path, filter, sort, hit, closed, out);
            }
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(symbols, &mut Vec::new(), !filter.is_empty(), sort, &hit, closed, &mut out);
    out
}

/// The path of the innermost symbol whose declaration holds `at`.
pub fn path_at(symbols: &[Symbol], at: lsp_types::Position) -> Option<Vec<usize>> {
    let key = |p: lsp_types::Position| (p.line, p.character);
    let (i, s) =
        symbols.iter().enumerate().find(|(_, s)| key(s.range.start) <= key(at) && key(at) <= key(s.range.end))?;
    let mut path = vec![i];
    if let Some(inner) = path_at(&s.children, at) {
        path.extend(inner);
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEXT: &str = "package p;\n\n/** Doc. */\n@Deprecated\npublic class A {\n    private static final int N = 1;\n    @SuppressWarnings(\"public\")\n    protected void run() {}\n    int count;\n    interface I { void go(); }\n}\n";

    fn pos(line: u32, character: u32) -> serde_json::Value {
        json!({ "line": line, "character": character })
    }

    fn sym(
        name: &str,
        kind: u32,
        range: (u32, u32, u32, u32),
        sel: (u32, u32),
        children: serde_json::Value,
    ) -> serde_json::Value {
        json!({ "name": name, "kind": kind,
            "range": { "start": pos(range.0, range.1), "end": pos(range.2, range.3) },
            "selectionRange": { "start": pos(sel.0, sel.1), "end": pos(sel.0, sel.1 + name.len() as u32) },
            "children": children })
    }

    fn symbols() -> Vec<Symbol> {
        let v = json!([sym(
            "A",
            5,
            (2, 0, 10, 1),
            (4, 13),
            json!([
                sym("N", 14, (5, 4, 5, 35), (5, 29), json!([])),
                sym("run()", 6, (6, 4, 7, 27), (7, 19), json!([])),
                sym("count", 8, (8, 4, 8, 14), (8, 8), json!([])),
                sym("I", 11, (9, 4, 9, 30), (9, 14), json!([sym("go()", 6, (9, 18, 9, 28), (9, 23), json!([]))])),
            ])
        )]);
        parse(&v, TEXT)
    }

    #[test]
    fn reads_visibility_off_the_declaration() {
        let s = symbols();
        let a = &s[0];
        assert_eq!(a.visibility, Visibility::Public, "past the comment and the annotation");
        let v: Vec<(Visibility, bool)> = a.children.iter().map(|c| (c.visibility, c.is_static)).collect();
        assert_eq!(
            v,
            [
                (Visibility::Private, true),
                // "public" in an annotation's argument does not count.
                (Visibility::Protected, false),
                (Visibility::Package, false),
                (Visibility::Package, false)
            ]
        );
        assert_eq!(a.children[3].children[0].visibility, Visibility::Public, "an interface's method");
    }

    #[test]
    fn rows_filter_sort_fold_and_follow_the_caret() {
        let s = symbols();
        let names = |rows: &[Row]| rows.iter().map(|r| r.symbol.name.clone()).collect::<Vec<_>>();
        let all = rows(&s, "", Sort::Position, &|_| false);
        assert_eq!(names(&all), ["A", "N", "run()", "count", "I", "go()"]);
        assert_eq!(all[5].depth, 2);
        let by_name = rows(&s, "", Sort::Name, &|_| false);
        assert_eq!(names(&by_name), ["A", "I", "go()", "count", "N", "run()"]);
        let folded = rows(&s, "", Sort::Position, &|p| p == [0, 3]);
        assert_eq!(names(&folded), ["A", "N", "run()", "count", "I"]);
        assert!(!folded[4].open && folded[4].has_children);
        // A filter keeps the symbols around a match, and opens them.
        let found = rows(&s, "go", Sort::Position, &|_| true);
        assert_eq!(names(&found), ["A", "I", "go()"]);
        assert_eq!(path_at(&s, lsp_types::Position::new(7, 10)), Some(vec![0, 1]));
        assert_eq!(path_at(&s, lsp_types::Position::new(9, 20)), Some(vec![0, 3, 0]));
        assert_eq!(path_at(&s, lsp_types::Position::new(0, 0)), None);
    }
}
