//! A spec tab while a recording writes into it, or locators picked go to its caret: the bar
//! that says so, with Stop, and the writes themselves (the app's, not the user's typing).

use std::ops::Range;

use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};

use super::EditorPanel;
use crate::look;
use crate::theme::{mix, palette};

/// What the tab's bar says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordBar {
    /// A recording writes its steps here; `lost` once they were edited meanwhile (it no
    /// longer writes).
    Recording { steps: usize, lost: bool },
    /// Locators picked in the browser go to the caret.
    Picking { picked: usize },
}

impl EditorPanel {
    /// Put `text` in place of `range`, as the app's change; the caret goes after it.
    /// Returns where `text` now is.
    pub fn write_range(
        &mut self,
        range: Range<usize>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Range<usize> {
        let len = self.state.read(cx).text().len();
        let range = range.start.min(len)..range.end.min(len);
        let start = range.start;
        self.writing += 1;
        self.state.update(cx, |s, cx| {
            s.set_selected_range(range, cx);
            s.replace(text.to_owned(), window, cx);
            let end = (start + text.len()).min(s.text().len());
            s.set_selected_range(end..end, cx);
        });
        start..start + text.len()
    }

    /// The caret's place in the text, and the text before it on its line.
    pub fn caret_and_before(&self, cx: &App) -> (usize, String) {
        let s = self.state.read(cx);
        let caret = s.cursor();
        let text = s.text().to_string();
        let start = text[..caret.min(text.len())].rfind('\n').map_or(0, |i| i + 1);
        (caret, text[start..caret.min(text.len())].to_owned())
    }

    pub fn set_record_bar(&mut self, bar: Option<RecordBar>, cx: &mut Context<Self>) {
        if self.recording != bar {
            self.recording = bar;
            cx.notify();
        }
    }

    /// The bar above the text while a recording or a pick goes.
    pub(super) fn record_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let bar = self.recording.as_ref()?;
        let p = palette(cx);
        let red = p.error;
        let text = match bar {
            RecordBar::Recording { lost: true, .. } => {
                ts!("editor-recording-but-the-recorded-steps-were-edited-here").to_owned()
            }
            RecordBar::Recording { steps, .. } => {
                let n = match steps {
                    0 => ts!("editor-no-steps-yet").to_owned(),
                    1 => ts!("editor-1-step").to_owned(),
                    n => t!("editor-steps", n = n.to_string()),
                };
                t!("editor-recording-into-this-file-act-in-the-browser", n = n.to_string())
            }
            RecordBar::Picking { picked: 0 } => {
                ts!("editor-picking-locators-click-an-element-in-the-browser").to_owned()
            }
            RecordBar::Picking { picked } => {
                t!("editor-picking-locators-picked-click-another-or-stop", picked = picked.to_string())
            }
        };
        Some(
            div()
                .id("record-bar")
                .debug_selector(|| "record-bar".into())
                .flex_none()
                .mx(px(8.))
                .mb(px(4.))
                .px(px(10.))
                .py(px(5.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(6.))
                .bg(p.wash(red))
                .border_1()
                .border_color(mix(red, 0.4, p.panel))
                .child(div().flex_none().size(px(9.)).rounded_full().bg(red))
                .child(div().flex_1().min_w_0().child(crate::look::text(text)))
                .child(
                    look::small_button("record-stop", crate::components::IconName::Square, ts!("cmd-stop"), &p)
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::menus::StopRecording), cx)),
                ),
        )
    }
}
