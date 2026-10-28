//! GPUI Kit's editor asks its providers for hover, completion, definitions and code
//! actions; these ask the document's language server, through the hub.

use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Result;
use gpui::{App, Entity, SharedString, Task, WeakEntity, Window};
use gpui_component::input::{
    CodeActionProvider, CompletionProvider, DefinitionProvider, EditorState, HoverProvider, Rope, RopeExt as _,
};
use lsp_types::{
    CodeAction, CodeActionOrCommand, CompletionContext, CompletionItem, CompletionResponse, CompletionTextEdit,
    GotoDefinitionResponse, Hover, LocationLink, Uri,
};
use serde_json::{Value, json};

use super::hub::{HubEvent, LangHub};
use super::text;

/// An editor's document as a language server knows it.
pub struct DocLink {
    pub hub: WeakEntity<LangHub>,
    pub uri: Uri,
    pub root: PathBuf,
    /// The editor tab this link belongs to.
    pub editor: gpui::EntityId,
    /// The last completion list, as the server sent it, and the text it was for: to find
    /// the item a completion inserted, and add the imports it needs.
    pub completions: RefCell<(Vec<CompletionItem>, Rope)>,
}

impl DocLink {
    /// Bring the server up to date with `text`; the change, if there was one.
    pub fn sync(&self, text: &Rope, cx: &mut App) -> Option<(Range<usize>, String)> {
        self.hub.upgrade()?.update(cx, |h, _| h.sync(&self.uri, text))
    }

    /// Ask the document's server (after sending what changed).
    pub fn ask(&self, text: &Rope, method: &str, params: Value, cx: &mut App) -> Task<Result<Value>> {
        let Some(hub) = self.hub.upgrade() else { return Task::ready(Err(anyhow::anyhow!("no hub"))) };
        hub.update(cx, |h, _| h.sync(&self.uri, text));
        let call = hub.read(cx).request(&self.root, method, params);
        cx.background_executor().spawn(call)
    }

    /// `textDocument` and `position` for byte `offset`.
    pub fn at(&self, text: &Rope, offset: usize) -> Value {
        json!({ "textDocument": { "uri": self.uri.as_str() }, "position": text::to_lsp(text, offset) })
    }

    fn emit(&self, event: HubEvent, cx: &mut App) {
        if let Some(hub) = self.hub.upgrade() {
            hub.update(cx, |_, cx| cx.emit(event));
        }
    }
}

/// The providers of one Java editor.
pub struct JavaProviders(pub Rc<DocLink>);

/// Put the providers on `state`, and the hook that opens definitions in other files.
pub fn install(link: &Rc<DocLink>, state: &Entity<EditorState>, cx: &mut App) {
    let providers = Rc::new(JavaProviders(link.clone()));
    let open_link = link.clone();
    state.update(cx, |s, cx| {
        let lsp = s.lsp_mut();
        lsp.hover_provider = Some(providers.clone());
        lsp.completion_provider = Some(providers.clone());
        lsp.definition_provider = Some(providers.clone());
        lsp.code_action_providers = vec![providers];
        lsp.completion_menu.max_width = gpui::px(520.);
        lsp.show_document = Some(Rc::new(move |params, _, cx| {
            if params.uri == open_link.uri {
                return false;
            }
            let at = params.selection.map_or_else(Default::default, |r| r.start);
            let event = HubEvent::Open { root: open_link.root.clone(), uri: params.uri.clone(), at };
            open_link.emit(event, cx);
            true
        }));
        cx.notify();
    });
}

/// Take the providers off again (the tab moved on to a file of no server).
pub fn uninstall(state: &Entity<EditorState>, cx: &mut App) {
    state.update(cx, |s, _| {
        let lsp = s.lsp_mut();
        lsp.hover_provider = None;
        lsp.completion_provider = None;
        lsp.definition_provider = None;
        lsp.code_action_providers.clear();
        lsp.show_document = None;
    });
}

impl HoverProvider for JavaProviders {
    fn hover(&self, text: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Option<Hover>>> {
        let call = self.0.ask(text, "textDocument/hover", self.0.at(text, offset), cx);
        let rope = text.clone();
        cx.background_executor().spawn(async move {
            let v = call.await?;
            if v.is_null() {
                return Ok(None);
            }
            let mut hover: Hover = serde_json::from_value(v)?;
            if hover_is_empty(&hover) {
                return Ok(None);
            }
            hover.range = hover.range.map(|r| text::range_to_kit(&rope, r));
            Ok(Some(hover))
        })
    }
}

fn hover_is_empty(h: &Hover) -> bool {
    use lsp_types::{HoverContents, MarkedString};
    let blank = |m: &MarkedString| match m {
        MarkedString::String(s) => s.trim().is_empty(),
        MarkedString::LanguageString(l) => l.value.trim().is_empty(),
    };
    match &h.contents {
        HoverContents::Scalar(m) => blank(m),
        HoverContents::Array(a) => a.iter().all(blank),
        HoverContents::Markup(m) => m.value.trim().is_empty(),
    }
}

impl CompletionProvider for JavaProviders {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let mut params = self.0.at(text, offset);
        params["context"] = serde_json::to_value(&trigger).unwrap_or_default();
        let call = self.0.ask(text, "textDocument/completion", params, cx);
        let (rope, link) = (text.clone(), self.0.clone());
        cx.spawn(async move |_| {
            let v = call.await?;
            let response: CompletionResponse =
                if v.is_null() { CompletionResponse::Array(Vec::new()) } else { serde_json::from_value(v)? };
            let (items, incomplete) = match response {
                CompletionResponse::Array(items) => (items, false),
                CompletionResponse::List(list) => (list.items, list.is_incomplete),
            };
            *link.completions.borrow_mut() = (items.clone(), rope.clone());
            // GPUI Kit's columns count characters; the server's, UTF-16 units.
            let items = items
                .into_iter()
                .map(|mut item| {
                    if let Some(CompletionTextEdit::Edit(edit)) = item.text_edit.as_mut() {
                        edit.range = text::range_to_kit(&rope, edit.range);
                    }
                    item
                })
                .collect();
            Ok(CompletionResponse::List(lsp_types::CompletionList { is_incomplete: incomplete, items }))
        })
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        new_text == "." || new_text.chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_')
    }
}

/// The text a completion item puts in.
pub fn inserted_text(item: &CompletionItem) -> &str {
    match &item.text_edit {
        Some(CompletionTextEdit::Edit(e)) => &e.new_text,
        Some(CompletionTextEdit::InsertAndReplace(e)) => &e.new_text,
        None => item.insert_text.as_deref().unwrap_or(&item.label),
    }
}

/// After an edit put `inserted` in the text, ending at byte `end` of `now`: the completion
/// item it was, if one of the last list ends there with that text. Typing one character
/// is taken for typing.
pub fn accepted_item(
    link: &DocLink,
    now: &Rope,
    end: usize,
    change: &(Range<usize>, String),
) -> Option<CompletionItem> {
    let (removed, inserted) = change;
    if inserted.chars().count() < 2 && removed.is_empty() {
        return None;
    }
    let list = link.completions.borrow();
    list.0
        .iter()
        .filter(|item| {
            let t = inserted_text(item);
            !t.is_empty() && end >= t.len() && now.slice(end - t.len()..end) == t && t.ends_with(inserted.as_str())
        })
        .max_by_key(|item| inserted_text(item).len())
        .cloned()
}

impl DefinitionProvider for JavaProviders {
    fn definitions(&self, text: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Vec<LocationLink>>> {
        let call = self.0.ask(text, "textDocument/definition", self.0.at(text, offset), cx);
        let (rope, uri) = (text.clone(), self.0.uri.clone());
        let word = text.word_range(offset).map(|r| text::range_to_kit(&rope, text::range_to_lsp(&rope, &r)));
        cx.background_executor().spawn(async move {
            let v = call.await?;
            if v.is_null() {
                return Ok(Vec::new());
            }
            let links: Vec<LocationLink> = match serde_json::from_value::<GotoDefinitionResponse>(v)? {
                GotoDefinitionResponse::Scalar(l) => vec![link_of(l)],
                GotoDefinitionResponse::Array(ls) => ls.into_iter().map(link_of).collect(),
                GotoDefinitionResponse::Link(ls) => ls,
            };
            // Ranges in this document are GPUI Kit's to follow, so they count characters.
            Ok(links
                .into_iter()
                .map(|mut l| {
                    if l.target_uri == uri {
                        l.target_range = text::range_to_kit(&rope, l.target_range);
                        l.target_selection_range = text::range_to_kit(&rope, l.target_selection_range);
                    }
                    l.origin_selection_range = l.origin_selection_range.map(|r| text::range_to_kit(&rope, r)).or(word);
                    l
                })
                .collect())
        })
    }
}

fn link_of(l: lsp_types::Location) -> LocationLink {
    LocationLink {
        origin_selection_range: None,
        target_uri: l.uri,
        target_range: l.range,
        target_selection_range: l.range,
    }
}

impl CodeActionProvider for JavaProviders {
    fn id(&self) -> SharedString {
        "java".into()
    }

    fn code_actions(
        &self,
        state: Entity<EditorState>,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<CodeAction>>> {
        let rope = state.read(cx).text().clone();
        let lsp_range = text::range_to_lsp(&rope, &range);
        let diagnostics: Vec<lsp_types::Diagnostic> = self
            .0
            .hub
            .upgrade()
            .map(|h| h.read(cx).diagnostics_of(&self.0.uri))
            .unwrap_or_default()
            .into_iter()
            .filter(|d| d.range.start.line <= lsp_range.end.line && d.range.end.line >= lsp_range.start.line)
            .collect();
        let params = json!({
            "textDocument": { "uri": self.0.uri.as_str() },
            "range": lsp_range,
            "context": { "diagnostics": diagnostics, "triggerKind": 1 }
        });
        let call = self.0.ask(&rope, "textDocument/codeAction", params, cx);
        cx.background_executor().spawn(async move {
            let v = call.await?;
            let list: Vec<CodeActionOrCommand> = if v.is_null() { Vec::new() } else { serde_json::from_value(v)? };
            Ok(list.into_iter().map(as_action).collect())
        })
    }

    fn perform_code_action(
        &self,
        _: Entity<EditorState>,
        action: CodeAction,
        _: bool,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        perform(&self.0, action, cx);
        Task::ready(Ok(()))
    }
}

/// A command as a code action, so the menu lists both alike.
pub fn as_action(c: CodeActionOrCommand) -> CodeAction {
    match c {
        CodeActionOrCommand::CodeAction(a) => a,
        CodeActionOrCommand::Command(cmd) => {
            CodeAction { title: cmd.title.clone(), command: Some(cmd), ..Default::default() }
        }
    }
}

/// Apply a code action's edit, then run its command.
pub fn perform(link: &DocLink, action: CodeAction, cx: &mut App) {
    if let Some(edit) = action.edit {
        link.emit(HubEvent::ApplyEdit { edit, reply: None }, cx);
    }
    if let Some(command) = action.command
        && let Some(hub) = link.hub.upgrade()
    {
        let uri = link.uri.clone();
        hub.update(cx, |h, cx| h.execute_command(&uri, command, cx));
    }
}
