//! Refactoring from the keyboard: Extract Method, Extract Variable, Extract Constant,
//! Extract Field and Inline, through the engine's code actions for the selection. An
//! extraction's edit goes in, then the new name is offered for renaming where it stands.
//! When the engine offers more than one way (this occurrence only, or every one of them),
//! a small menu at the caret asks which. When it offers none, a note at the caret says why.

use std::collections::HashSet;
use std::ops::Range;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use lsp_types::{CodeAction, CodeActionOrCommand, WorkspaceEdit};
use serde_json::json;

use super::EditorPanel;
use crate::lang::edits::{self, Step};
use crate::lang::{HubEvent, text};
use crate::look;
use crate::theme::palette;

/// A refactoring the keyboard asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refactoring {
    ExtractMethod,
    ExtractVariable,
    ExtractConstant,
    ExtractField,
    Inline,
}

impl Refactoring {
    /// What the menus call it.
    pub fn name(self) -> &'static str {
        match self {
            Refactoring::ExtractMethod => "Extract Method",
            Refactoring::ExtractVariable => "Extract Variable",
            Refactoring::ExtractConstant => "Extract Constant",
            Refactoring::ExtractField => "Extract Field",
            Refactoring::Inline => "Inline",
        }
    }

    /// The kind of code action the engine files it under.
    pub fn kind(self) -> &'static str {
        match self {
            Refactoring::ExtractMethod => "refactor.extract.function",
            Refactoring::ExtractVariable => "refactor.extract.variable",
            Refactoring::ExtractConstant => "refactor.extract.constant",
            Refactoring::ExtractField => "refactor.extract.field",
            Refactoring::Inline => "refactor.inline",
        }
    }

    /// It makes something new, which gets a name.
    pub fn extracts(self) -> bool {
        self != Refactoring::Inline
    }

    /// Whether `action` is one of this refactoring's.
    fn offers(self, action: &CodeAction) -> bool {
        let kind = action.kind.as_ref().map(|k| k.as_str()).unwrap_or_default();
        action.disabled.is_none() && (kind == self.kind() || kind.starts_with(&format!("{}.", self.kind())))
    }

    /// What to say when the engine offers nothing for the selection.
    pub fn not_here(self) -> &'static str {
        match self {
            Refactoring::ExtractMethod => "Cannot extract a method here: select whole statements, or an expression",
            Refactoring::ExtractVariable => "Cannot extract a variable here: select an expression",
            Refactoring::ExtractConstant => {
                "Cannot extract a constant here: select an expression of constants, such as a literal"
            }
            Refactoring::ExtractField => "Cannot extract a field here: select an expression",
            Refactoring::Inline => "Nothing to inline here: put the caret on a local variable, a method or a constant",
        }
    }
}

/// The ways the engine offers for one refactoring, for the user to pick from.
pub struct Choices {
    pub refactoring: Refactoring,
    pub items: Vec<CodeAction>,
    pub selected: usize,
    /// Where the selection started, to find the new name near.
    near: usize,
    focus: gpui::FocusHandle,
}

/// How long a note at the caret stays.
const HINT_FOR: Duration = Duration::from_secs(4);

impl EditorPanel {
    /// Ask the engine for `r` on the selection (or at the caret, for Inline).
    pub fn refactor(&mut self, r: Refactoring, window: &mut Window, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone() else { return };
        if self.class_file.is_some() {
            return self.show_hint("A library class is read only", cx);
        }
        let (rope, selection) = {
            let s = self.state.read(cx);
            (s.text().clone(), s.selected_range())
        };
        if r.extracts() && selection.is_empty() {
            return self.show_hint(r.not_here(), cx);
        }
        let params = json!({
            "textDocument": { "uri": link.uri.as_str() },
            "range": text::range_to_lsp(&rope, &selection),
            "context": { "diagnostics": [], "only": [r.kind()], "triggerKind": 1 }
        });
        let call = link.ask(&rope, "textDocument/codeAction", params, cx);
        let revision = self.revision;
        let before = rope.to_string();
        self._refactor = Some(cx.spawn_in(window, async move |this, cx| {
            let answer = call.await;
            let _ = this.update_in(cx, |this, window, cx| {
                // The text changed meanwhile: the actions are for text that is gone.
                if this.revision != revision {
                    return;
                }
                let list: Vec<CodeActionOrCommand> = match answer {
                    Ok(v) => serde_json::from_value(v).unwrap_or_default(),
                    Err(e) => {
                        eprintln!("[java] {}: {e}", r.name());
                        return this.show_hint("The Java engine is not ready yet", cx);
                    }
                };
                let items = distinct(
                    list.into_iter().map(crate::lang::providers::as_action).filter(|a| r.offers(a)).collect(),
                    &before,
                    &this.path,
                );
                match items.len() {
                    0 => this.show_hint(r.not_here(), cx),
                    1 => {
                        let action = items.into_iter().next().expect("one");
                        this.perform_refactoring(r, action, selection.start, window, cx);
                    }
                    _ => {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        this.choices =
                            Some(Choices { refactoring: r, items, selected: 0, near: selection.start, focus });
                        cx.notify();
                    }
                }
            });
        }));
    }

    /// Apply `action`'s edit; for an extraction, then offer to rename what it made.
    fn perform_refactoring(
        &mut self,
        r: Refactoring,
        action: CodeAction,
        near: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(link) = self.lang.clone() else { return };
        let before = self.text(cx);
        let (edit, command) = match (action.edit, action.command) {
            (Some(edit), command) => (Some(edit), command),
            // jdtls hands some edits over as a command for the client to apply.
            (None, Some(c)) if c.command == "java.apply.workspaceEdit" => {
                let edit =
                    c.arguments.as_ref().and_then(|a| a.first()).and_then(|v| serde_json::from_value(v.clone()).ok());
                (edit, None)
            }
            (None, command) => (None, command),
        };
        let mut here = true;
        if let Some(edit) = edit {
            match edits_of(&edit, &self.path) {
                // Only this file changes: here, at once, so the new name can be found.
                Some(list) => self.apply_lsp_edits(&list, window, cx),
                None => {
                    here = false;
                    if let Some(hub) = link.hub.upgrade() {
                        hub.update(cx, |_, cx| cx.emit(HubEvent::ApplyEdit { edit, reply: None }));
                    }
                }
            }
        }
        if let Some(command) = command
            && let Some(hub) = link.hub.upgrade()
        {
            let uri = link.uri.clone();
            hub.update(cx, |h, cx| h.execute_command(&uri, command, cx));
        }
        self.state.update(cx, |s, cx| s.focus(window, cx));
        if !(r.extracts() && here) {
            return;
        }
        let after = self.text(cx);
        if let Some(name) = new_name(&before, &after, near) {
            self.state.update(cx, |s, cx| s.set_selected_range(name.start..name.start, cx));
            self.start_rename(window, cx);
        }
    }

    /// The menu's pick: apply it.
    fn choose(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.choices.take() else { return };
        cx.notify();
        let Some(action) = c.items.into_iter().nth(i) else { return };
        self.perform_refactoring(c.refactoring, action, c.near, window, cx);
    }

    /// Put the menu away, and the keyboard back in the text.
    pub fn close_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choices.take().is_some() {
            self.state.update(cx, |s, cx| s.focus(window, cx));
            cx.notify();
        }
    }

    fn choices_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.choices.as_mut() else { return };
        let n = c.items.len().max(1);
        match e.keystroke.key.as_str() {
            "up" => c.selected = (c.selected + n - 1) % n,
            "down" => c.selected = (c.selected + 1) % n,
            "enter" => {
                let i = c.selected;
                return self.choose(i, window, cx);
            }
            "escape" => return self.close_choices(window, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// A note at the caret, for a few seconds: why a command did nothing.
    pub fn show_hint(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.hint = Some(text.into());
        self._hint = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HINT_FOR).await;
            let _ = this.update(cx, |this, cx| {
                this.hint = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// The note at the caret, under it.
    pub(super) fn hint_popup(&self, cx: &App) -> Option<AnyElement> {
        let text = self.hint.clone()?;
        let b = self.caret_bounds(cx)?;
        let p = palette(cx);
        let note = crate::components::callout(crate::components::Tone::Warning, text, &p)
            .id("editor-hint")
            .debug_selector(|| "editor-hint".into())
            .mt(px(4.))
            .max_w(px(460.))
            .px(px(10.))
            .py(px(6.))
            .shadow_md()
            .font_family(look::ui_font(cx));
        Some(gpui::deferred(gpui::anchored().position(b.bottom_left()).child(note)).with_priority(1).into_any_element())
    }

    /// The menu of ways, under the caret.
    pub(super) fn choices_popup(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = self.choices.as_ref()?;
        let b = self.caret_bounds(cx)?;
        let p = palette(cx);
        let rows = c.items.iter().enumerate().map(|(i, a)| {
            let selected = i == c.selected;
            div()
                .id(("refactor-choice", i))
                .debug_selector(move || format!("refactor-choice-{i}"))
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .cursor_pointer()
                .when(selected, |d| d.bg(p.row_selected()))
                .when(!selected, |d| d.hover(|s| s.bg(p.row_hover())))
                .child(choice_label(&a.title, c.items.len()))
                .on_click(cx.listener(move |this, _, window, cx| this.choose(i, window, cx)))
        });
        let menu = div()
            .id("refactor-choices")
            .debug_selector(|| "refactor-choices".into())
            .track_focus(&c.focus)
            .on_key_down(cx.listener(Self::choices_key))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_choices(window, cx)))
            .mt(px(4.))
            .min_w(px(260.))
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .text_color(p.fg)
            .child(
                div()
                    .px(px(8.))
                    .pb(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(p.weak())
                    .child(crate::components::icon(crate::components::IconName::WandSparkles, 13., p.fg_dim))
                    .child(c.refactoring.name()),
            )
            .children(rows);
        Some(
            gpui::deferred(gpui::anchored().position(b.bottom_left()).snap_to_window_with_margin(px(8.)).child(menu))
                .with_priority(1)
                .into_any_element(),
        )
    }
}

/// What the menu calls a way: which occurrences an extraction replaces, when the engine
/// offers both.
fn choice_label(title: &str, ways: usize) -> String {
    const ALL: &str = " (replace all occurrences)";
    match title.strip_suffix(ALL) {
        Some(_) => "Replace all occurrences".into(),
        None if ways == 2 && title.starts_with("Extract to") => "Replace this occurrence only".into(),
        None => title.to_owned(),
    }
}

/// The edits `edit` makes to `path`, when it changes that file and nothing else.
fn edits_of(edit: &WorkspaceEdit, path: &std::path::Path) -> Option<Vec<lsp_types::TextEdit>> {
    let mut out = Vec::new();
    for step in edits::steps(edit.clone()) {
        match step {
            Step::Edit(p, list) if p == path => out.extend(list),
            _ => return None,
        }
    }
    Some(out)
}

/// `items` less those that would leave the file as one before them does (the engine offers
/// "every occurrence" beside "this one" even where there is only one).
fn distinct(items: Vec<CodeAction>, before: &str, path: &std::path::Path) -> Vec<CodeAction> {
    let mut seen: Vec<String> = Vec::new();
    items
        .into_iter()
        .filter(|a| {
            let Some(list) = a.edit.as_ref().and_then(|e| edits_of(e, path)) else { return true };
            let after = text::apply_to_string(before, &list);
            if seen.contains(&after) {
                return false;
            }
            seen.push(after);
            true
        })
        .collect()
}

/// The identifiers of Java source `s`, outside comments and literals.
fn identifiers(s: &str) -> Vec<Range<usize>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c >= 0x80 {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] >= 0x80) {
                i += 1;
            }
            out.push(start..i);
        } else {
            i += 1;
        }
    }
    out.retain(|r| s.is_char_boundary(r.start) && s.is_char_boundary(r.end.min(s.len())));
    out
}

/// Where the name a refactoring made stands in `after`: an identifier `before` did not
/// have, the occurrence nearest `near`.
pub fn new_name(before: &str, after: &str, near: usize) -> Option<Range<usize>> {
    let old: HashSet<&str> = identifiers(before).into_iter().map(|r| &before[r]).collect();
    identifiers(after)
        .into_iter()
        .filter(|r| r.end <= after.len() && !old.contains(&after[r.clone()]))
        .min_by_key(|r| r.start.abs_diff(near))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_name_an_extraction_made() {
        let before = "class A {\n    int f() {\n        return 2 * 21; // the answer\n    }\n}\n";
        let after =
            "class A {\n    int f() {\n        int answer = 2 * 21;\n        return answer; // the answer\n    }\n}\n";
        let at = new_name(before, after, before.find("2 * 21").unwrap()).unwrap();
        assert_eq!(&after[at.clone()], "answer");
        // The occurrence nearest where the selection was.
        assert_eq!(at.start, after.find("answer").unwrap());
        let far = new_name(before, after, after.find("return").unwrap()).unwrap();
        assert_eq!(far.start, after.find("return answer").unwrap() + 7);
        // Words in comments and strings are not names.
        assert_eq!(identifiers("a /* b */ \"c\" 'd' // e\nf"), [0..1, 23..24]);
        assert!(new_name("int a;", "int a;", 0).is_none());
        assert_eq!(choice_label("Extract to local variable (replace all occurrences)", 2), "Replace all occurrences");
        assert_eq!(choice_label("Extract to local variable", 2), "Replace this occurrence only");
    }

    #[test]
    fn knows_its_code_action_kinds() {
        let action = |kind: &str| CodeAction {
            title: "t".into(),
            kind: Some(lsp_types::CodeActionKind::from(kind.to_owned())),
            ..Default::default()
        };
        assert!(Refactoring::ExtractMethod.offers(&action("refactor.extract.function")));
        assert!(!Refactoring::ExtractMethod.offers(&action("refactor.extract.functional")));
        assert!(Refactoring::Inline.offers(&action("refactor.inline")));
        assert!(!Refactoring::ExtractField.offers(&action("refactor.extract.variable")));
    }
}
