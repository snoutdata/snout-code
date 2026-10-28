//! The editor spike's claims, headless: a hover provider is asked when the pointer rests
//! on the text, and diagnostics land in the editor's set. See `examples/editor_spike.rs`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;
use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement as _, Render, Styled as _, Task,
    TestAppContext, Window, div, point, px,
};
use gpui_component::highlighter::{Diagnostic, DiagnosticSeverity};
use gpui_component::input::{Editor, EditorState, HoverProvider, Position, Rope, RopeExt};

struct Recorder(Rc<RefCell<Vec<String>>>);

impl HoverProvider for Recorder {
    fn hover(&self, text: &Rope, offset: usize, _: &mut Window, _: &mut App) -> Task<Result<Option<lsp_types::Hover>>> {
        let word = text.word_range(offset).map(|r| text.slice(r).to_string()).unwrap_or_default();
        self.0.borrow_mut().push(word.clone());
        Task::ready(Ok(Some(lsp_types::Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(word)),
            range: None,
        })))
    }
}

struct View {
    editor: Entity<EditorState>,
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Editor::new(&self.editor).h_full())
    }
}

#[gpui::test]
fn java_editor_asks_for_hovers_and_keeps_diagnostics(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let asked = Rc::new(RefCell::new(Vec::new()));
    let source: String = (0..5_000).map(|i| format!("    int value{i} = compute(total, {i});\n")).collect();
    let text = format!("class A {{\n{source}}}\n");
    let provider = Recorder(asked.clone());
    let (view, cx) = cx.add_window_view(|window, cx| {
        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx).language("java");
            state.set_value(text, window, cx);
            state.lsp_mut().hover_provider = Some(Rc::new(provider));
            state.diagnostics_mut().unwrap().push(
                Diagnostic::new(Position::new(1, 8)..Position::new(1, 14), "Cannot resolve symbol")
                    .with_severity(DiagnosticSeverity::Error),
            );
            state
        });
        View { editor }
    });
    cx.run_until_parked();
    let editor = cx.update(|_, cx| view.read(cx).editor.clone());
    assert_eq!(cx.update(|_, cx| editor.read(cx).text().lines_len()), 5_003);
    assert_eq!(cx.update(|_, cx| editor.read(cx).diagnostics().map(|d| d.len())), Some(1));

    // Rest the pointer over the second line, past the gutter, until the provider is asked.
    for x in [120.0, 160.0, 200.0, 240.0] {
        cx.simulate_mouse_move(point(px(x), px(30.0)), None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
    }
    let asked = asked.borrow();
    assert!(!asked.is_empty(), "the hover provider was never asked");
    assert!(asked.iter().any(|w| !w.trim().is_empty()), "asked about {asked:?}");
}
