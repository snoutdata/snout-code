//! The editor's side of the language server: the document kept in sync, the problems as
//! squiggles and gutter dots, imports a completion needs, signature help, rename, find
//! references, formatting and organize imports, and class files shown read only.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, Window, canvas, div, point, px, size,
};
use gpui_component::input::{InputEvent, InputState, Rope, RopeExt as _};
use lsp_types::{SignatureHelp, TextEdit, Uri, WorkspaceEdit};
use serde_json::json;

use super::{EditorPanel, font_size, language_for};
use crate::lang::providers::{self, DocLink};
use crate::lang::{HubEvent, text};
use crate::look;
use crate::theme::palette;

impl EditorPanel {
    /// Tell the file's language server about it, if it has one (a Java file of a Gradle
    /// build), and give the editor its providers.
    pub(super) fn attach_lang(&mut self, cx: &mut Context<Self>) {
        self.detach_lang(cx);
        self.problems.clear();
        self.signature = None;
        if self.class_file.is_some() || language_for(&self.path) != "java" {
            return;
        }
        let Some(hub) = crate::lang::hub(cx) else { return };
        let weak = cx.weak_entity();
        let rope = self.state.read(cx).text().clone();
        let path = self.path.clone();
        let Some((uri, root, diags)) = hub.update(cx, |h, cx| h.open_doc(&weak, &path, rope, cx)) else { return };
        let link = Rc::new(DocLink {
            hub: hub.downgrade(),
            uri,
            root,
            editor: cx.entity_id(),
            completions: RefCell::new((Vec::new(), Rope::new())),
            typed: std::cell::Cell::new(false),
        });
        providers::install(&link, &self.state, cx);
        self.lang = Some(link);
        if !diags.is_empty() {
            self.set_diagnostics(diags, cx);
        }
    }

    /// The tab no longer shows the document: the server may forget it.
    pub(super) fn detach_lang(&mut self, cx: &mut App) {
        let Some(link) = self.lang.take() else { return };
        if let Some(hub) = link.hub.upgrade() {
            hub.update(cx, |h, _| h.close_doc(&link.uri, link.editor));
        }
        providers::uninstall(&self.state, cx);
    }

    /// The text changed: send the change, add the imports an accepted completion needs,
    /// and ask for signature help when a call's arguments are being typed.
    pub(super) fn lang_changed(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone() else { return };
        let typed = link.typed.replace(false);
        let rope = self.state.read(cx).text().clone();
        let Some(change) = link.sync(&rope, cx) else { return };
        let end = change.0.start + change.1.len();
        if let Some(item) = providers::accepted_item(&link, &rope, end, &change, typed) {
            link.completions.borrow_mut().0.clear();
            self.add_completion_imports(&link, item, cx);
        }
        if change.1.ends_with('(') || change.1.ends_with(',') {
            self.ask_signature(cx);
        } else if change.1.contains(')') && self.signature.take().is_some() {
            cx.notify();
        }
    }

    /// The edits an accepted completion carries (an import), resolved when the list
    /// left them out.
    fn add_completion_imports(&mut self, link: &Rc<DocLink>, item: lsp_types::CompletionItem, cx: &mut Context<Self>) {
        let uri = link.uri.clone();
        let edits = item.additional_text_edits.clone().unwrap_or_default();
        if !edits.is_empty() {
            emit_edits(link, uri, edits, cx);
            return;
        }
        let rope = self.state.read(cx).text().clone();
        let params = serde_json::to_value(&item).unwrap_or_default();
        let call = link.ask(&rope, "completionItem/resolve", params, cx);
        let link = link.clone();
        cx.spawn(async move |_, cx| {
            let Ok(v) = call.await else { return };
            let Ok(item) = serde_json::from_value::<lsp_types::CompletionItem>(v) else { return };
            let edits = item.additional_text_edits.unwrap_or_default();
            if !edits.is_empty() {
                cx.update(|cx| emit_edits(&link, uri, edits, cx));
            }
        })
        .detach();
    }

    /// Show what the server reports: squiggles (GPUI Kit draws them, and the message on
    /// hover) and a dot in the gutter.
    pub fn set_diagnostics(&mut self, list: Vec<lsp_types::Diagnostic>, cx: &mut Context<Self>) {
        self.problems = list;
        let problems = self.problems.clone();
        self.state.update(cx, |s, cx| {
            let rope = s.text().clone();
            if let Some(set) = s.diagnostics_mut() {
                set.reset(&rope);
                let mut kit: Vec<lsp_types::Diagnostic> = problems
                    .into_iter()
                    .map(|mut d| {
                        d.range = text::range_to_kit(&rope, d.range);
                        d
                    })
                    .collect();
                kit.sort_by_key(|d| (d.range.start.line, d.range.start.character));
                set.extend(kit);
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Apply a server's edits (protocol ranges) to this tab, as undoable edits, keeping
    /// the caret where it was in the text around it.
    pub fn apply_lsp_edits(&mut self, edits: &[TextEdit], window: &mut Window, cx: &mut Context<Self>) {
        let (rope, caret) = {
            let s = self.state.read(cx);
            (s.text().clone(), s.cursor())
        };
        let list = text::edits_from_lsp(&rope, edits);
        if list.is_empty() {
            return;
        }
        let mut new_caret = caret as isize;
        for (range, t) in &list {
            if range.end <= caret {
                new_caret += t.len() as isize - range.len() as isize;
            } else if range.start < caret {
                new_caret = (range.start + t.len()) as isize;
            }
        }
        self.state.update(cx, |s, cx| {
            for (range, t) in list {
                s.set_selected_range(range, cx);
                s.replace(t, window, cx);
            }
            let c = (new_caret.max(0) as usize).min(s.text().len());
            s.set_selected_range(c..c, cx);
        });
    }

    /// Put the caret on a protocol position (UTF-16 columns) and give the editor the keyboard.
    pub fn go_to_lsp(&mut self, at: lsp_types::Position, window: &mut Window, cx: &mut Context<Self>) {
        let pos = text::to_kit(self.state.read(cx).text(), at);
        self.go_to(pos.line, pos.character, window, cx);
    }

    /// ⌘-click: go to the definition of the symbol under the pointer. GPUI Kit does this
    /// only for a symbol the pointer rested on with ⌘ held, and an open hover popover
    /// forgets that symbol on any press outside it (in the capture phase, before the
    /// editor sees the press). So the press is taken here, before either: the caret goes
    /// under the pointer and the editor's own go to definition runs from there.
    pub(super) fn command_click(
        &mut self,
        e: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if e.button != gpui::MouseButton::Left || !e.modifiers.secondary(cx) || e.modifiers.alt {
            return false;
        }
        let state = self.state.clone();
        if !state.update(cx, |s, _| s.lsp_mut().definition_provider.is_some()) {
            return false;
        }
        let Some(at) = offset_at(state.read(cx), e.position) else {
            return false;
        };
        state.update(cx, |s, cx| {
            s.dismiss_lsp_overlays(cx);
            s.set_selected_range(at..at, cx);
            s.focus(window, cx);
        });
        window.dispatch_action(Box::new(gpui_component::input::GoToDefinition), cx);
        true
    }

    /// The word at the caret and the caret as a protocol position.
    fn at_caret(&self, cx: &App) -> (lsp_types::Position, String, Rope) {
        let s = self.state.read(cx);
        let rope = s.text().clone();
        let caret = s.cursor();
        (text::to_lsp(&rope, caret), rope.word_at(caret), rope)
    }

    /// ⇧F12: every use of the symbol at the caret, in the References panel.
    pub fn find_references(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone() else { return };
        let (at, word, rope) = self.at_caret(cx);
        link.sync(&rope, cx);
        if let Some(hub) = link.hub.upgrade() {
            let name = if word.trim().is_empty() { "the symbol".to_owned() } else { word };
            hub.update(cx, |h, cx| h.find_references(link.uri.clone(), at, name, cx));
        }
    }

    /// F2: a field over the caret for the new name of the symbol there.
    pub fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.lang.is_none() {
            return;
        }
        let (_, word, _) = self.at_caret(cx);
        if word.trim().is_empty() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(word.clone()));
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        cx.subscribe_in(&input, window, |this, _, e: &InputEvent, window, cx| match e {
            InputEvent::PressEnter { .. } => this.confirm_rename(window, cx),
            InputEvent::Blur => this.cancel_rename(window, cx),
            _ => {}
        })
        .detach();
        self.rename = Some(input);
        cx.notify();
    }

    pub fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rename.take().is_some() {
            self.state.update(cx, |s, cx| s.focus(window, cx));
            cx.notify();
        }
    }

    /// Rename the symbol at the caret everywhere the server finds it.
    pub fn confirm_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.rename.take() else { return };
        let name = input.read(cx).value().trim().to_owned();
        self.state.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
        let Some(link) = self.lang.clone() else { return };
        let (at, word, rope) = self.at_caret(cx);
        if name.is_empty() || name == word {
            return;
        }
        let params = json!({ "textDocument": { "uri": link.uri.as_str() }, "position": at, "newName": name });
        let call = link.ask(&rope, "textDocument/rename", params, cx);
        cx.spawn(async move |_, cx| match call.await {
            Ok(v) => {
                if let Ok(edit) = serde_json::from_value::<WorkspaceEdit>(v) {
                    cx.update(|cx| emit(&link, HubEvent::ApplyEdit { edit, reply: None }, cx));
                }
            }
            Err(e) => eprintln!("[java] rename: {e}"),
        })
        .detach();
    }

    /// ⇧⌥F: the server's formatting of the whole file.
    pub fn format_document(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone() else { return };
        let rope = self.state.read(cx).text().clone();
        let params = json!({
            "textDocument": { "uri": link.uri.as_str() },
            "options": { "tabSize": self.info.width, "insertSpaces": !self.info.tabs }
        });
        let call = link.ask(&rope, "textDocument/formatting", params, cx);
        let uri = link.uri.clone();
        cx.spawn(async move |_, cx| {
            let Ok(v) = call.await else { return };
            let edits: Vec<TextEdit> = serde_json::from_value(v).unwrap_or_default();
            if !edits.is_empty() {
                cx.update(|cx| emit_edits(&link, uri, edits, cx));
            }
        })
        .detach();
    }

    /// ⇧⌥O: imports sorted, unused ones gone, missing ones added where only one fits.
    pub fn organize_imports(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone() else { return };
        let rope = self.state.read(cx).text().clone();
        let params = json!({
            "textDocument": { "uri": link.uri.as_str() },
            "range": text::range_to_lsp(&rope, &(0..rope.len())),
            "context": { "diagnostics": [], "only": ["source.organizeImports"], "triggerKind": 1 }
        });
        let call = link.ask(&rope, "textDocument/codeAction", params, cx);
        cx.spawn(async move |_, cx| {
            let Ok(v) = call.await else { return };
            let list: Vec<lsp_types::CodeActionOrCommand> = serde_json::from_value(v).unwrap_or_default();
            if let Some(action) = list.into_iter().next().map(providers::as_action) {
                cx.update(|cx| providers::perform(&link, action, cx));
            }
        })
        .detach();
    }

    /// Ask which call the caret is in, and which of its parameters.
    pub fn ask_signature(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone() else { return };
        let (at, _, rope) = self.at_caret(cx);
        let params = json!({ "textDocument": { "uri": link.uri.as_str() }, "position": at });
        let call = link.ask(&rope, "textDocument/signatureHelp", params, cx);
        self.signature_line = Some(at.line);
        self._signature = Some(cx.spawn(async move |this, cx| {
            let help = call.await.ok().and_then(|v| serde_json::from_value::<SignatureHelp>(v).ok());
            let _ = this.update(cx, |this, cx| {
                this.signature = help.filter(|h| !h.signatures.is_empty());
                cx.notify();
            });
        }));
    }

    /// A dot beside each line with an error or a warning.
    pub(super) fn problem_marks(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let (state, problems) = (self.state.clone(), self.problems.clone());
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                if problems.is_empty() {
                    return;
                }
                let s = state.read(cx);
                let (Some(rows), Some(line_height)) = (s.visible_row_range(), s.line_height()) else { return };
                let rope = s.text();
                let map = s.display_map();
                let first = map.display_row_to_buffer_line(rows.start);
                let last = map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start));
                let mut worst: std::collections::BTreeMap<u32, bool> = Default::default();
                for d in &problems {
                    let error = d.severity.is_none_or(|s| s == lsp_types::DiagnosticSeverity::ERROR);
                    let warn = d.severity == Some(lsp_types::DiagnosticSeverity::WARNING);
                    if error || warn {
                        let e = worst.entry(d.range.start.line).or_insert(false);
                        *e |= error;
                    }
                }
                for (line, error) in worst {
                    let line = line as usize;
                    if line < first || line > last || line >= rope.lines_len() {
                        continue;
                    }
                    let at = rope.line_start_offset(line);
                    let Some(b) = s.range_to_bounds(&(at..at)) else { continue };
                    let r = px(3.5);
                    let center = point(bounds.left() + px(7.), b.top() + line_height / 2.);
                    let dot = gpui::Bounds::new(point(center.x - r, center.y - r), size(r * 2., r * 2.));
                    window.paint_quad(gpui::fill(dot, if error { p.error } else { p.warn }).corner_radii(r));
                }
            },
        )
        .absolute()
        .inset_0()
    }

    /// Where the caret is drawn, in window coordinates.
    fn caret_bounds(&self, cx: &App) -> Option<gpui::Bounds<gpui::Pixels>> {
        let s = self.state.read(cx);
        let c = s.cursor();
        s.range_to_bounds(&(c..c))
    }

    /// The signature of the call being typed, its current parameter in bold, over the caret.
    pub(super) fn signature_popup(&self, cx: &App) -> Option<AnyElement> {
        let help = self.signature.as_ref()?;
        let b = self.caret_bounds(cx)?;
        let p = palette(cx);
        let active = help.active_signature.unwrap_or(0) as usize;
        let sig = help.signatures.get(active).or(help.signatures.first())?;
        let param = sig.active_parameter.or(help.active_parameter).unwrap_or(0) as usize;
        let label = sig.label.clone();
        // The active parameter's span of the label.
        let span = sig.parameters.as_ref().and_then(|ps| ps.get(param)).and_then(|pi| match &pi.label {
            lsp_types::ParameterLabel::Simple(s) => label.find(s.as_str()).map(|i| i..i + s.len()),
            lsp_types::ParameterLabel::LabelOffsets([a, b]) => {
                let units: Vec<u16> = label.encode_utf16().collect();
                let to_byte = |u: u32| String::from_utf16_lossy(&units[..(u as usize).min(units.len())]).len();
                Some(to_byte(*a)..to_byte(*b))
            }
        });
        let parts: Vec<AnyElement> = match span.filter(|s| s.end <= label.len() && label.is_char_boundary(s.start)) {
            Some(s) => vec![
                div().child(label[..s.start].to_owned()).into_any_element(),
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.accent)
                    .child(label[s.clone()].to_owned())
                    .into_any_element(),
                div().child(label[s.end..].to_owned()).into_any_element(),
            ],
            None => vec![div().child(label).into_any_element()],
        };
        let popup = div()
            .id("signature-help")
            .debug_selector(|| "signature-help".into())
            .mb(px(4.))
            .px(px(8.))
            .py(px(4.))
            .flex()
            .rounded(px(6.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_md()
            .font_family(look::MONO)
            .text_size(px(font_size(cx) - 1.))
            .text_color(p.fg)
            .children(parts);
        Some(
            gpui::deferred(gpui::anchored().position(b.origin).anchor(gpui::Anchor::BottomLeft).child(popup))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// The field for the new name, under the caret.
    pub(super) fn rename_box(&self, cx: &App) -> Option<AnyElement> {
        let input = self.rename.as_ref()?;
        let b = self.caret_bounds(cx)?;
        let p = palette(cx);
        let field = div()
            .id("rename-box")
            .key_context("RenameBox")
            .debug_selector(|| "rename-box".into())
            .mt(px(4.))
            .w(px(280.))
            .p(px(6.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .text_size(px(12.))
            .child(div().text_color(p.weak()).child("Rename to (Enter renames everywhere)"))
            .child(look::field(input, false, None, &p));
        Some(
            gpui::deferred(gpui::anchored().position(b.bottom_left()).child(field)).with_priority(1).into_any_element(),
        )
    }

    /// Show a class file of a library (its source when a source jar has it) read only.
    pub fn show_class_file(
        &mut self,
        uri: Uri,
        root: PathBuf,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.detach_lang(cx);
        self.path = PathBuf::from(class_file_name(&uri));
        self.error = None;
        self.root = None;
        self.class_file = Some((uri.clone(), root.clone()));
        self.saved = text.clone();
        self.dirty = false;
        self.preview = false;
        self.state.update(cx, |s, cx| {
            s.set_highlighter("java", cx);
            s.set_value(text, window, cx);
            s.set_readonly(true, cx);
        });
        if let Some(hub) = crate::lang::hub(cx) {
            let weak = cx.weak_entity();
            let rope = self.state.read(cx).text().clone();
            hub.update(cx, |h, _| h.open_virtual(&weak, &root, &uri, rope));
            let link = Rc::new(DocLink {
                hub: hub.downgrade(),
                uri,
                root,
                editor: cx.entity_id(),
                completions: RefCell::new((Vec::new(), Rope::new())),
                typed: std::cell::Cell::new(false),
            });
            providers::install(&link, &self.state, cx);
            self.lang = Some(link);
        }
        cx.notify();
    }
}

/// "String.class" for `jdt://contents/rt.jar/java.lang/String.class?=…`.
pub fn class_file_name(uri: &Uri) -> String {
    let s = uri.as_str();
    let path = s.split('?').next().unwrap_or(s);
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = name.replace("%24", "$");
    if name.is_empty() { "Class.class".into() } else { name }
}

/// Whether `path` is a class file tab's made-up path.
pub fn is_class_file_path(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "class") && !path.is_absolute()
}

/// The text offset of the character under `point` (window coordinates), from the
/// editor's last layout: the visible lines are searched, a character at a time.
pub fn offset_at(state: &gpui_component::input::EditorState, point: gpui::Point<gpui::Pixels>) -> Option<usize> {
    let rows = state.visible_row_range()?;
    let text = state.text();
    let map = state.display_map();
    let first = map.display_row_to_buffer_line(rows.start);
    let last = (map.display_row_to_buffer_line(rows.end.saturating_sub(1).max(rows.start)) + 1).min(text.lines_len());
    for line in first..last {
        let (start, end) = (text.line_start_offset(line), text.line_end_offset(line));
        let mut at = start;
        for c in text.slice(start..end).chars() {
            let next = at + c.len_utf8();
            if state.range_to_bounds(&(at..next)).is_some_and(|b| b.contains(&point)) {
                return Some(at);
            }
            at = next;
        }
    }
    None
}

fn emit(link: &DocLink, event: HubEvent, cx: &mut App) {
    if let Some(hub) = link.hub.upgrade() {
        hub.update(cx, |_, cx| cx.emit(event));
    }
}

/// Ask the workspace to apply `edits` to the document `uri`.
#[allow(clippy::mutable_key_type)] // lsp-types keys its edits by URI
fn emit_edits(link: &DocLink, uri: Uri, edits: Vec<TextEdit>, cx: &mut App) {
    let mut changes = std::collections::HashMap::new();
    changes.insert(uri, edits);
    let edit = WorkspaceEdit { changes: Some(changes), ..Default::default() };
    emit(link, HubEvent::ApplyEdit { edit, reply: None }, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr as _;

    #[test]
    fn names_class_files_after_their_class() {
        let uri = Uri::from_str("jdt://contents/rt.jar/java.lang/String.class?=demo/%5C/jdk").unwrap();
        assert_eq!(class_file_name(&uri), "String.class");
        assert!(is_class_file_path(Path::new("String.class")));
    }
}
