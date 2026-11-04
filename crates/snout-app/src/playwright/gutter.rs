//! The run icons of a Playwright spec file: one at each `test(…)` and `test.describe(…)`
//! (with `only`, `skip`, `fixme`, `serial` and the like), found by tree-sitter in the
//! editor's text, so they show before anything is installed; they move when the file is saved.

use std::path::Path;

use crate::run::{FileRunnables, Runnable, Target};

/// What calls the scan marks: `test`, `it`, `describe` and their modifiers; not `test.step`,
/// `test.use`, the hooks or `test.extend`.
fn marked(callee: &str) -> Option<bool> {
    let mut parts = callee.split('.');
    let head = parts.next()?;
    if !matches!(head, "test" | "it" | "describe") {
        return None;
    }
    let mut group = head == "describe";
    for p in parts {
        match p {
            "describe" => group = true,
            "only" | "skip" | "fixme" | "fail" | "slow" | "serial" | "parallel" => {}
            _ => return None,
        }
    }
    Some(group)
}

/// The string a literal holds, quotes and template marks off.
fn literal(node: tree_sitter::Node, text: &str) -> Option<String> {
    match node.kind() {
        "string" | "template_string" => {
            let raw = node.utf8_text(text.as_bytes()).ok()?;
            Some(raw.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_owned())
        }
        _ => None,
    }
}

/// `text` of the file at `path`, parsed.
fn parse(path: &Path, text: &str) -> Option<tree_sitter::Tree> {
    let tsx = path.extension().is_some_and(|e| e == "tsx" || e == "jsx");
    let language: tree_sitter::Language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(text, None)
}

/// The tests and groups in the spec file at `path` holding `text`.
pub fn runnables(path: &Path, text: &str) -> FileRunnables {
    let mut out = FileRunnables::default();
    let Some(tree) = parse(path, text) else { return out };
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression"
            && let Some(callee) = node.child_by_field_name("function")
            && let Ok(name) = callee.utf8_text(text.as_bytes())
            && let Some(group) = marked(&name.split_whitespace().collect::<String>())
            && let Some(args) = node.child_by_field_name("arguments")
            && let Some(title) = args.named_child(0).and_then(|a| literal(a, text))
        {
            let line = node.start_position().row as u32;
            out.items.push(Runnable {
                line,
                target: Target::Playwright { file: path.to_owned(), line: line + 1, title, group },
            });
        }
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    out.items.sort_by_key(|r| r.line);
    out
}

/// The tests and groups in the test file at `path` holding `text`, each with its line (from
/// 0), its `describe`s' titles and its own, and whether it is a group.
pub fn tests_with_paths(path: &Path, text: &str) -> Vec<(u32, Vec<String>, bool)> {
    let mut out = Vec::new();
    let Some(tree) = parse(path, text) else { return out };
    let mut cursor = tree.walk();
    let mut stack: Vec<(tree_sitter::Node, Vec<String>)> = vec![(tree.root_node(), Vec::new())];
    while let Some((node, around)) = stack.pop() {
        let mut inner = around.clone();
        if node.kind() == "call_expression"
            && let Some(callee) = node.child_by_field_name("function")
            && let Ok(name) = callee.utf8_text(text.as_bytes())
            && let Some(group) = marked(&name.split_whitespace().collect::<String>())
            && let Some(args) = node.child_by_field_name("arguments")
            && let Some(title) = args.named_child(0).and_then(|a| literal(a, text))
        {
            let mut titles = around.clone();
            titles.push(title);
            out.push((node.start_position().row as u32, titles.clone(), group));
            if group {
                inner = titles;
            }
        }
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev().map(|c| (c, inner.clone())));
    }
    out.sort_by_key(|(line, _, _)| *line);
    out
}

/// Where the caret is in a spec, for what a recording writes there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Spot {
    /// In the body of a test, a hook or a step: the steps go in as they are, at `indent`.
    Body { indent: String },
    /// Between tests (at the top, or in a `describe`): a new test goes in, at `indent`.
    Outside { indent: String },
    /// The file holds nothing yet: a whole spec goes in.
    Empty,
}

/// Calls whose callback holds steps: tests, hooks and steps.
fn holds_steps(callee: &str) -> bool {
    marked(callee) == Some(false)
        || callee
            .strip_prefix("test.")
            .is_some_and(|m| matches!(m, "beforeEach" | "afterEach" | "beforeAll" | "afterAll" | "step"))
}

/// The indentation of the line `at` is on.
fn indent_at(text: &str, at: usize) -> String {
    let start = text[..at.min(text.len())].rfind('\n').map_or(0, |i| i + 1);
    text[start..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

/// The indentation inside `block` (a `{ … }`): its first statement's, else one step in
/// from the line it opens on.
fn inner_indent(block: tree_sitter::Node, text: &str) -> String {
    match block.named_child(0) {
        Some(first) => indent_at(text, first.start_byte()),
        None => indent_at(text, block.start_byte()) + &indent_unit(text),
    }
}

/// One step of the file's indentation: a tab, or its smallest run of spaces (two in a file
/// without any).
pub fn indent_unit(text: &str) -> String {
    let mut spaces = usize::MAX;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if line.starts_with('\t') {
            return "\t".into();
        }
        let n = line.len() - line.trim_start_matches(' ').len();
        if n > 0 {
            spaces = spaces.min(n);
        }
    }
    " ".repeat(if spaces == usize::MAX { 2 } else { spaces })
}

/// Where the caret at byte `at` is, in `text` of the spec at `path`.
pub fn spot(path: &Path, text: &str, at: usize) -> Spot {
    if text.trim().is_empty() {
        return Spot::Empty;
    }
    let Some(tree) = parse(path, text) else { return Spot::Outside { indent: String::new() } };
    let mut found = Spot::Outside { indent: String::new() };
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    // Outer calls come before inner ones: the last that holds the caret wins.
    while let Some(node) = stack.pop() {
        if node.start_byte() > at || node.end_byte() < at {
            continue;
        }
        if node.kind() == "call_expression"
            && let Some(callee) = node.child_by_field_name("function")
            && let Ok(name) = callee.utf8_text(text.as_bytes())
            && let Some(args) = node.child_by_field_name("arguments")
        {
            let name: String = name.split_whitespace().collect();
            let body = (0..args.named_child_count() as u32)
                .filter_map(|i| args.named_child(i))
                .filter(|a| matches!(a.kind(), "arrow_function" | "function_expression" | "function"))
                .filter_map(|f| f.child_by_field_name("body"))
                .find(|b| b.kind() == "statement_block" && b.start_byte() < at && at < b.end_byte());
            if let Some(body) = body {
                if holds_steps(&name) {
                    found = Spot::Body { indent: inner_indent(body, text) };
                } else if marked(&name) == Some(true) {
                    found = Spot::Outside { indent: inner_indent(body, text) };
                }
            }
        }
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_tests_and_groups_not_steps_or_hooks() {
        let text = r#"import { test, expect } from '@playwright/test';

test.describe('checkout', () => {
  test.beforeEach(async ({ page }) => { await page.goto('/'); });
  test('pays by card', async ({ page }) => {
    await test.step('fill', async () => {});
  });
  test.only(`applies a coupon`, async () => {});
  test.describe.serial("nested", () => {
    test.skip('later', async () => {});
  });
});
test.use({ locale: 'fr' });
"#;
        let r = runnables(Path::new("/p/tests/checkout.spec.ts"), text);
        let got: Vec<(u32, String, bool)> = r
            .items
            .iter()
            .map(|i| match &i.target {
                Target::Playwright { line, title, group, .. } => (*line, title.clone(), *group),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            got,
            [
                (3, "checkout".into(), true),
                (5, "pays by card".into(), false),
                (8, "applies a coupon".into(), false),
                (9, "nested".into(), true),
                (10, "later".into(), false),
            ]
        );
        assert_eq!(r.items[1].line, 4, "the icon's line counts from 0");
    }

    #[test]
    fn the_caret_is_in_a_test_between_tests_or_in_an_empty_file() {
        let text = "import { test } from '@playwright/test';\n\ntest.describe('cart', () => {\n    test('adds', async ({ page }) => {\n        await page.goto('/');\n        \n    });\n    test.beforeEach(async ({ page }) => {});\n    \n});\n\n";
        let path = Path::new("/p/tests/cart.spec.ts");
        let at = |needle: &str, ahead: usize| text.find(needle).unwrap() + ahead;
        // On the blank line after the goto.
        assert_eq!(spot(path, text, at("await page.goto('/');\n", 30)), Spot::Body { indent: "        ".into() });
        // In an empty hook: one step in from its line.
        assert_eq!(spot(path, text, at("{});", 1)), Spot::Body { indent: "        ".into() });
        // In the describe, between its tests.
        assert_eq!(spot(path, text, at("{});\n    \n", 9)), Spot::Outside { indent: "    ".into() });
        // At the end of the file.
        assert_eq!(spot(path, text, text.len()), Spot::Outside { indent: String::new() });
        assert_eq!(spot(path, " \n", 1), Spot::Empty);
    }
}
