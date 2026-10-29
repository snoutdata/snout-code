//! GPUI Kit's editor asks its providers for hover, completion, definitions and code
//! actions; these ask the document's language server, through the hub.

use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Result;
use gpui::{App, Entity, SharedString, Task, WeakEntity, Window};
use gpui_component::input::{
    CodeActionProvider, CompletionProvider, DefinitionProvider, EditorState, Rope, RopeExt as _,
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
    /// The last edit was typed. GPUI Kit asks whether typed text triggers completion,
    /// and inserts an accepted completion without asking: so an edit it did not ask about
    /// is no typing, even when it adds a single character (`ArrayLis` to `ArrayList`).
    pub typed: std::cell::Cell<bool>,
    /// A Spring Boot configuration file (properties or YAML), which the engine serves.
    pub config: bool,
    /// The hover request of the word last asked about.
    pub hovers: HoverCache,
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

    /// What the server says about the symbol at byte `offset`: its markdown and the range
    /// (bytes) it is about. The editor's hover card asks this; GPUI Kit's own hover is off.
    pub fn hover_markdown(&self, text: &Rope, offset: usize, cx: &mut App) -> Task<Option<(String, Range<usize>)>> {
        let call = self.hovers.hover(text, offset, cx, |cx| {
            let call = self.ask(text, "textDocument/hover", self.at(text, offset), cx);
            cx.background_executor().spawn(async move {
                let v = call.await.map_err(|e| e.to_string())?;
                if v.is_null() {
                    return Ok(None);
                }
                let hover: Hover = serde_json::from_value(v).map_err(|e| e.to_string())?;
                Ok((!hover_is_empty(&hover)).then_some(hover))
            })
        });
        let rope = text.clone();
        cx.background_executor().spawn(async move {
            let hover = call.await.ok().flatten()?;
            let range = hover
                .range
                .map(|r| text::range_from_lsp(&rope, r))
                .or_else(|| rope.word_range(offset))
                .unwrap_or(offset..offset);
            Some((hover_markdown(&hover), range))
        })
    }

    fn emit(&self, event: HubEvent, cx: &mut App) {
        if let Some(hub) = self.hub.upgrade() {
            hub.update(cx, |_, cx| cx.emit(event));
        }
    }
}

/// What the server answers about a hover, shared by every ask for the same word.
type HoverAnswer = futures::future::Shared<Task<std::result::Result<Option<Hover>, String>>>;

/// The hover request of the word last asked about.
#[derive(Default)]
pub struct HoverCache(RefCell<Option<(Rope, Range<usize>, HoverAnswer)>>);

impl HoverCache {
    /// GPUI Kit asks again after every pointer move and drops the ask before it, so a
    /// hand that keeps the pointer moving a little cancelled every request a server took
    /// longer than that to answer. Here one request per word (and text) runs to its end
    /// whatever drops it, and every ask for the same word waits for that one; `ask`
    /// starts the request.
    pub fn hover(
        &self,
        text: &Rope,
        offset: usize,
        cx: &mut App,
        ask: impl FnOnce(&mut App) -> Task<std::result::Result<Option<Hover>, String>>,
    ) -> Task<Result<Option<Hover>>> {
        let word = text.word_range(offset).unwrap_or(offset..offset + 1);
        let cached = self.0.borrow().as_ref().and_then(|(rope, range, answer)| {
            (range == &word && rope.len() == text.len() && rope == text).then(|| answer.clone())
        });
        let answer = cached.unwrap_or_else(|| {
            let answer = futures::FutureExt::shared(ask(cx));
            *self.0.borrow_mut() = Some((text.clone(), word, answer.clone()));
            answer
        });
        cx.background_executor().spawn(async move { answer.await.map_err(|e| anyhow::anyhow!(e)) })
    }
}

/// The providers of one Java editor.
pub struct JavaProviders(pub Rc<DocLink>);

impl JavaProviders {
    pub fn new(link: Rc<DocLink>) -> Self {
        Self(link)
    }
}

/// Put the providers on `state`, and the hook that opens definitions in other files.
pub fn install(link: &Rc<DocLink>, state: &Entity<EditorState>, cx: &mut App) {
    let providers = Rc::new(JavaProviders::new(link.clone()));
    let open_link = link.clone();
    state.update(cx, |s, cx| {
        let lsp = s.lsp_mut();
        // The editor's own hover card shows what the server says, and the problems there.
        lsp.hover_provider = None;
        lsp.completion_provider = Some(providers.clone());
        lsp.definition_provider = Some(providers);
        // The editor's own context actions (⌥↩, ⌘.) ask for code actions; GPUI Kit's menu
        // has none to show.
        lsp.code_action_providers.clear();
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

/// A hover's contents as one markdown text.
pub fn hover_markdown(h: &Hover) -> String {
    use lsp_types::{HoverContents, MarkedString};
    let one = |m: &MarkedString| match m {
        MarkedString::String(s) => s.clone(),
        MarkedString::LanguageString(l) => format!("```{}\n{}\n```", l.language, l.value),
    };
    match &h.contents {
        HoverContents::Scalar(m) => one(m),
        HoverContents::Array(a) => a.iter().map(one).collect::<Vec<_>>().join("\n\n"),
        HoverContents::Markup(m) => m.value.clone(),
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
        self.0.typed.set(true);
        // In a configuration file, keys have dashes, and values follow `=` or `: `.
        let config = self.0.config && (new_text.ends_with(['=', '-', ':']) || new_text == " ");
        config || new_text == "." || new_text.chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_')
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
/// item it was, if one of the last list ends there with that text. A typed edit is no
/// completion, however much it looks like one.
pub fn accepted_item(
    link: &DocLink,
    now: &Rope,
    end: usize,
    change: &(Range<usize>, String),
    typed: bool,
) -> Option<CompletionItem> {
    let (_, inserted) = change;
    if typed || inserted.is_empty() {
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
