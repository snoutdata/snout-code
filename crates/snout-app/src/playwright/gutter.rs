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

/// The tests and groups in the spec file at `path` holding `text`.
pub fn runnables(path: &Path, text: &str) -> FileRunnables {
    let tsx = path.extension().is_some_and(|e| e == "tsx" || e == "jsx");
    let language: tree_sitter::Language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let mut parser = tree_sitter::Parser::new();
    let mut out = FileRunnables::default();
    if parser.set_language(&language).is_err() {
        return out;
    }
    let Some(tree) = parser.parse(text, None) else { return out };
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
}
