//! Editor spike: GPUI Kit's `Editor` on a generated Java file, with the hover and
//! diagnostics an IDE needs, to see what the component's public API can carry.
//!
//!   cargo run -p snout-app --example editor_spike [-- --lines 200000]
//!       opens a window: hover a word for its popover; the squiggles are diagnostics.
//!   cargo run -p snout-app --example editor_spike --release -- --measure [--lines N]
//!       renders headlessly with the real text system and prints frame times for
//!       loading, scrolling and typing.
//!
//! Findings (GPUI Kit 0.8.1, GPUI 0.3.10):
//! - Hover popovers: public API. `EditorState::lsp_mut().hover_provider` takes a
//!   `HoverProvider` returning an LSP `Hover`; the Kit draws the popover (Markdown).
//! - Diagnostics: squiggles and the diagnostic popover are public API
//!   (`diagnostics_mut()` takes LSP-shaped diagnostics). Gutter markers are not: the
//!   gutter paints line numbers and fold icons only (`fold_icon_renderer` is called
//!   for foldable lines alone), so markers need a small patch (a per-line gutter
//!   renderer beside the fold one).
//! - Inlay hints: no API. Decorations (`RangeDecoration`, `TextDecoration`) style
//!   existing text, inline tokens replace source bytes, and the inline completion ghost
//!   text is one span at the caret. Virtual text needs a patch to the display map.
//! - Size: release build, Apple silicon, headless with the real text system. 5,000 lines:
//!   open 35 ms, frame 0.2 ms, keystroke (edit plus incremental reparse) 2.5 ms.
//!   200,000 lines (6 MB): open 0.7 s, frame 1.2 ms after jumping anywhere, keystroke
//!   2.6 ms then a 1.2 ms frame. Both stay far inside a 120 Hz frame; the frame cost grows
//!   with the line count (about 1 ms per 200K lines), not with what is on screen.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use gpui::*;
use gpui_component::highlighter::{Diagnostic, DiagnosticSeverity};
use gpui_component::input::{Editor, EditorState, HoverProvider, Position, Rope, RopeExt};

/// About 22 lines of Java per method (31 bytes a line on average); the class closes at the end.
fn java_source(lines: usize) -> String {
    let mut s = String::from(
        "package com.snoutdata.demo;\n\nimport java.util.List;\nimport java.util.Map;\n\n\
         /** A generated class for the editor spike. */\npublic class Spike {\n",
    );
    let mut n = 0;
    while s.len() / 31 < lines.saturating_sub(1) {
        s += &format!(
            "    /**\n     * Method number {n}.\n     * @param items the items\n     * @return the total\n     */\n\
             \x20   public int method{n}(List<String> items, Map<String, Integer> weights) {{\n\
             \x20       int total = 0;\n\
             \x20       for (String item : items) {{\n\
             \x20           Integer w = weights.get(item);\n\
             \x20           if (w == null) {{\n\
             \x20               continue;\n\
             \x20           }}\n\
             \x20           total += w * item.length();\n\
             \x20       }}\n\
             \x20       String label = \"method{n}\";\n\
             \x20       // Comment line for highlighting.\n\
             \x20       if (total > {n}) {{\n\
             \x20           System.out.println(label + \": \" + total);\n\
             \x20       }}\n\
             \x20       return total;\n\
             \x20   }}\n\n"
        );
        n += 1;
    }
    s += "}\n";
    s
}

/// Answers every hover with the word under the pointer, as a language server would.
struct WordHover;

impl HoverProvider for WordHover {
    fn hover(&self, text: &Rope, offset: usize, _: &mut Window, _: &mut App) -> Task<Result<Option<lsp_types::Hover>>> {
        let word: String = text.word_range(offset).map(|r| text.slice(r).to_string()).unwrap_or_default();
        let hover = (!word.trim().is_empty()).then(|| lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: format!("**{word}**\n\n```java\nint {word}(List<String> items)\n```"),
            }),
            range: None,
        });
        Task::ready(Ok(hover))
    }
}

/// An error on every 40th method name and a warning on every 40th `label`.
fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (line, l) in text.lines().enumerate() {
        let line = line as u32;
        if let Some(col) = l.find("public int method")
            && line % 1000 < 25
        {
            let start = col as u32 + 11;
            out.push(
                Diagnostic::new(Position::new(line, start)..Position::new(line, start + 6), "Cannot resolve symbol")
                    .with_severity(DiagnosticSeverity::Error)
                    .with_source("spike"),
            );
        }
        if let Some(col) = l.find("String label")
            && line % 500 < 25
        {
            let start = col as u32 + 7;
            out.push(
                Diagnostic::new(Position::new(line, start)..Position::new(line, start + 5), "Variable is never used")
                    .with_severity(DiagnosticSeverity::Warning),
            );
        }
    }
    out
}

struct Spike {
    editor: Entity<EditorState>,
}

impl Spike {
    fn new(lines: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = java_source(lines);
        let diags = diagnostics(&text);
        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx).language("java");
            state.set_value(text, window, cx);
            state.lsp_mut().hover_provider = Some(Rc::new(WordHover));
            if let Some(set) = state.diagnostics_mut() {
                set.extend(diags);
            }
            state
        });
        Self { editor }
    }
}

impl Render for Spike {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Editor::new(&self.editor).h_full())
    }
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let lines = arg("--lines").and_then(|n| n.parse().ok()).unwrap_or(5_000);
    if std::env::args().any(|a| a == "--measure") {
        return measure(lines);
    }
    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1100.), px(800.)), cx)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| Spike::new(lines, window, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

/// Frame times with the real text system, headless.
fn measure(lines: usize) {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::current_platform(true).text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    cx.update(gpui_component::init);
    let t = Instant::now();
    let mut view = None;
    let handle = cx
        .open_window(size(px(1100.), px(800.)), |window, cx| {
            let spike = cx.new(|cx| Spike::new(lines, window, cx));
            view = Some(spike.clone());
            cx.new(|cx| gpui_component::Root::new(spike, window, cx))
        })
        .unwrap();
    let view = view.unwrap();
    let frame = |cx: &mut HeadlessAppContext| {
        let t = Instant::now();
        cx.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .unwrap();
        t.elapsed()
    };
    let first = frame(&mut cx);
    cx.run_until_parked();
    println!("{lines} lines: open {:?}, first frame {first:?}", t.elapsed());
    if let Some(path) = arg("--shot") {
        for _ in 0..5 {
            cx.advance_clock(std::time::Duration::from_millis(500));
            cx.run_until_parked();
            frame(&mut cx);
        }
        cx.capture_screenshot(handle.into()).unwrap().save(&path).unwrap();
        println!("  saved {path}");
    }

    let editor = cx.update(|cx| view.read(cx).editor.clone());
    let mut times = Vec::new();
    for step in 0..60 {
        cx.update_window(handle.into(), |_, window, cx| {
            editor.update(cx, |e, cx| {
                let line = (step * lines / 60).min(lines - 1) as u32;
                e.set_cursor_position(Position::new(line, 0), window, cx);
            })
        })
        .unwrap();
        times.push(frame(&mut cx));
    }
    report("jump through the file", &times);

    times.clear();
    let mut parse = Vec::new();
    let mut edits = Vec::new();
    cx.update_window(handle.into(), |_, window, cx| {
        editor.update(cx, |e, cx| e.set_cursor_position(Position::new(lines as u32 / 2, 8), window, cx))
    })
    .unwrap();
    for _ in 0..60 {
        let t = Instant::now();
        cx.update_window(handle.into(), |_, window, cx| editor.update(cx, |e, cx| e.insert("x", window, cx))).unwrap();
        edits.push(t.elapsed());
        times.push(frame(&mut cx));
        let t = Instant::now();
        cx.run_until_parked();
        parse.push(t.elapsed());
    }
    report("insert a character (edit and reparse)", &edits);
    report("frame after the keystroke", &times);
    report("background reparse per keystroke", &parse);
}

fn report(what: &str, times: &[std::time::Duration]) {
    let mut ms: Vec<f64> = times.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(f64::total_cmp);
    let p = |q: f64| ms[((ms.len() - 1) as f64 * q) as usize];
    println!("  {what}: median {:.1} ms, p95 {:.1} ms, max {:.1} ms", p(0.5), p(0.95), p(1.0));
}
