//! A settings file in the editor (the user's `settings.json`, a project's
//! `.snout/settings.json`): completion of the keys and their values, a description on hover,
//! and its problems as squiggles, all from the settings' schema (see
//! [`crate::settings_json`]).

use std::rc::Rc;

use anyhow::Result;
use gpui::{App, Context, Entity, Task, Window};
use gpui_component::input::{CompletionProvider, EditorState, HoverProvider, Rope};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit, Diagnostic,
    DiagnosticSeverity, Documentation, Hover, HoverContents, MarkupContent, MarkupKind, TextEdit,
};

use super::EditorPanel;
use crate::lang::text;
use crate::settings_json::{self as sj, Which};

/// The user's settings file, as the app keeps it (none in a test without a store).
pub struct UserSettingsFile(pub std::path::PathBuf);

impl gpui::Global for UserSettingsFile {}

/// The providers of a settings file's editor.
struct SettingsProviders(Which);

fn kit_range(rope: &Rope, r: &std::ops::Range<usize>) -> lsp_types::Range {
    text::range_to_kit(rope, text::range_to_lsp(rope, r))
}

impl HoverProvider for SettingsProviders {
    fn hover(&self, rope: &Rope, offset: usize, _: &mut Window, _: &mut App) -> Task<Result<Option<Hover>>> {
        let text = rope.to_string();
        let hover = sj::hover(self.0, &text, offset).map(|(md, range)| Hover {
            contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: md }),
            range: Some(kit_range(rope, &range)),
        });
        Task::ready(Ok(hover))
    }
}

impl CompletionProvider for SettingsProviders {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        _: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let text = rope.to_string();
        let items = sj::completions(self.0, &text, offset)
            .into_iter()
            .map(|c| {
                let typed = &text[c.range.clone()];
                let filter = if typed.starts_with('"') { format!("\"{}", c.label) } else { c.label.clone() };
                CompletionItem {
                    label: c.label,
                    kind: Some(if c.is_key { CompletionItemKind::PROPERTY } else { CompletionItemKind::VALUE }),
                    detail: Some(c.detail),
                    documentation: c
                        .doc
                        .map(|d| Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: d })),
                    filter_text: Some(filter),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: kit_range(rope, &c.range),
                        new_text: c.insert,
                    })),
                    ..Default::default()
                }
            })
            .collect();
        Task::ready(Ok(CompletionResponse::List(lsp_types::CompletionList { is_incomplete: false, items })))
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        new_text.chars().last().is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '"' | ':'))
    }
}

/// Which settings `path` holds, if it is a settings file.
pub fn settings_file(path: &std::path::Path, cx: &App) -> Option<Which> {
    sj::which(path, cx.try_global::<UserSettingsFile>().map(|f| f.0.as_path()))
}

/// Put the providers on `state`.
fn install(which: Which, state: &Entity<EditorState>, cx: &mut App) {
    let providers = Rc::new(SettingsProviders(which));
    state.update(cx, |s, cx| {
        let lsp = s.lsp_mut();
        lsp.hover_provider = Some(providers.clone());
        lsp.completion_provider = Some(providers);
        lsp.completion_menu.max_width = gpui::px(520.);
        cx.notify();
    });
}

impl EditorPanel {
    /// A settings file: its providers, and its problems now.
    pub(super) fn attach_settings_lang(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(which) = settings_file(&self.path, cx) else { return false };
        install(which, &self.state, cx);
        self.settings_file = Some(which);
        self.check_settings_text(cx);
        true
    }

    /// The settings file's text changed: its problems again.
    pub fn check_settings_text(&mut self, cx: &mut Context<Self>) {
        let Some(which) = self.settings_file else { return };
        let rope = self.state.read(cx).text().clone();
        let text = rope.to_string();
        let list = sj::problems(which, &text)
            .into_iter()
            .map(|p| Diagnostic {
                range: text::range_to_lsp(&rope, &p.range),
                severity: Some(if p.error { DiagnosticSeverity::ERROR } else { DiagnosticSeverity::WARNING }),
                source: Some("SnoutCode settings".into()),
                message: p.message,
                ..Default::default()
            })
            .collect();
        self.set_diagnostics(list, cx);
    }
}
