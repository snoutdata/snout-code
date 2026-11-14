//! The Agent View tool window: what an agent chose to show the user, for what no tool covers. It
//! writes markdown, as it does all day, and a fenced block comes back with buttons. It is a renderer,
//! not a chat: an agent cannot see a click, and it lasts as long as the window. The console's
//! scrollback is the record of what the agent did.

use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};
use gpui_component::scroll::ScrollableElement as _;

use crate::components::IconName;
use crate::theme::palette;
use crate::workspace::{Tool, Workspace};

/// How many pushes it keeps.
const KEEP: usize = 50;
/// How much one push may hold, in bytes.
pub const MAX_PUSH: usize = 256 * 1024;

/// A piece of a push, in the order it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Prose(String),
    Code { language: String, text: String },
}

/// `markdown`, cut at its fenced blocks. A fence never closed runs to the end, as it renders.
pub fn blocks(markdown: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut prose = String::new();
    let mut fence: Option<(String, String, String)> = None;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        match &mut fence {
            Some((marks, _, text)) => {
                if ticks >= marks.len() && trimmed.trim_end().chars().all(|c| c == '`') {
                    let (_, language, text) = fence.take().unwrap_or_default();
                    out.push(Block::Code { language, text: text.trim_end_matches('\n').to_owned() });
                } else {
                    text.push_str(line);
                    text.push('\n');
                }
            }
            None if ticks >= 3 => {
                if !prose.trim().is_empty() {
                    out.push(Block::Prose(prose.trim_end().to_owned()));
                }
                prose.clear();
                let language = trimmed[ticks..].split_whitespace().next().unwrap_or_default().to_lowercase();
                fence = Some(("`".repeat(ticks), language, String::new()));
            }
            None => {
                prose.push_str(line);
                prose.push('\n');
            }
        }
    }
    if let Some((_, language, text)) = fence {
        out.push(Block::Code { language, text: text.trim_end_matches('\n').to_owned() });
    }
    if !prose.trim().is_empty() {
        out.push(Block::Prose(prose.trim_end().to_owned()));
    }
    out
}

/// Whether a block in `language` is commands for a shell.
pub fn is_shell(language: &str) -> bool {
    matches!(language, "sh" | "bash" | "shell" | "zsh" | "console" | "powershell" | "ps1" | "cmd" | "bat")
}

/// What was pushed.
#[derive(Clone, Debug)]
pub struct Push {
    pub agent: String,
    pub title: Option<String>,
    pub blocks: Vec<Block>,
}

/// What the agents have shown, oldest first.
#[derive(Default)]
pub struct AgentView {
    pub pushes: Vec<Push>,
}

impl AgentView {
    /// Show `markdown`, after what shows unless `clear`. What the user will see, in the agent's terms.
    pub fn push(
        &mut self,
        agent: String,
        title: Option<String>,
        markdown: &str,
        clear: bool,
        cx: &mut Context<Self>,
    ) -> String {
        if clear {
            self.pushes.clear();
        }
        let blocks = blocks(markdown);
        let code = blocks.iter().filter(|b| matches!(b, Block::Code { .. })).count();
        let shell = blocks.iter().filter(|b| matches!(b, Block::Code { language, .. } if is_shell(language))).count();
        if !blocks.is_empty() {
            self.pushes.push(Push { agent, title, blocks });
        }
        if self.pushes.len() > KEEP {
            self.pushes.remove(0);
        }
        cx.notify();
        let mut told = "Shown in the app's Agent View".to_owned();
        if code > 0 {
            told += &format!(": {code} code block{} the user can copy or insert", if code == 1 { "" } else { "s" });
        }
        if shell > 0 {
            told += &format!(", {shell} of them with a Run in Terminal button");
        }
        told + ". You will not hear of what the user clicks."
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.pushes.clear();
        cx.notify();
    }
}

pub struct AgentViewPanel {
    view: Entity<AgentView>,
    ws: WeakEntity<Workspace>,
    focus: FocusHandle,
}

impl AgentViewPanel {
    pub fn new(view: Entity<AgentView>, ws: WeakEntity<Workspace>, cx: &mut Context<Self>) -> Self {
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        Self { view, ws, focus: cx.focus_handle() }
    }

    /// Put `text` in at the caret of the editor in front.
    fn insert(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.ws.update(cx, |ws, cx| {
            let Some(editor) = ws.active_editor(window, cx) else {
                return ws.model.update(cx, |m, cx| {
                    m.message = ts!("agents-open-a-file-to-insert-into").into();
                    cx.notify();
                });
            };
            editor.update(cx, |e, cx| e.state.update(cx, |s, cx| s.insert(text, window, cx)));
        });
    }

    /// Run `text` in the Terminal: the click is the user's own, and they see it type.
    fn run(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.ws.update(cx, |ws, cx| {
            ws.show_tool(Tool::Terminal, window, cx);
            let Some(terminal) = crate::console::terminal_of(ws, cx) else { return };
            terminal.update(cx, |t, cx| {
                let Some(root) = t.folder(cx) else { return };
                t.ensure_session(&root, window, cx);
                t.run_when_ready(&root, &text, cx);
            });
        });
    }

    fn code(
        &self,
        id: String,
        language: &str,
        text: &str,
        p: &crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (copied, inserted, ran) = (text.to_owned(), text.to_owned(), text.to_owned());
        let selector = |what: &str| format!("agent-view-{what}-{id}");
        let shell = is_shell(language);
        div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(p.border)
            .rounded(px(6.))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .px(px(8.))
                    .py(px(4.))
                    .bg(p.panel)
                    .child(div().flex_1().min_w_0().text_color(p.fg_dim).child(crate::look::text(language.to_owned())))
                    .child({
                        let s = selector("copy");
                        crate::look::small_button(s.clone(), IconName::Copy, ts!("cmd-copy"), p)
                            .debug_selector(move || s.clone())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copied.clone()));
                            }))
                    })
                    .child({
                        let s = selector("insert");
                        crate::look::small_button(s.clone(), IconName::Plus, ts!("agents-insert"), p)
                            .debug_selector(move || s.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.insert(inserted.clone(), w, cx)))
                    })
                    .children(shell.then(|| {
                        let s = selector("run");
                        crate::look::small_button(s.clone(), IconName::Play, ts!("agents-run-in-terminal"), p)
                            .debug_selector(move || s.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.run(ran.clone(), w, cx)))
                    })),
            )
            .child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .font_family(crate::look::mono_font(cx))
                    .text_size(px(12.))
                    .child(crate::look::text(text.to_owned())),
            )
            .into_any_element()
    }
}

crate::panels::panel!(AgentViewPanel, "Agent View", |_, _| ts!("stripes-agent-view").into(), true);

impl Render for AgentViewPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let pushes = self.view.read(cx).pushes.clone();
        let base = div()
            .id("agent-view")
            .debug_selector(|| "agent-view".into())
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_size(px(crate::look::BODY))
            .text_color(p.fg);
        if pushes.is_empty() {
            return base.child(
                crate::components::EmptyState::new(
                    snout_core::art::spots::quiet_bell(),
                    ts!("agents-nothing-shown-yet"),
                )
                .hint(ts!("agents-what-an-agent-in-the-console-chooses-to"))
                .compact(),
            );
        }
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (i, push) in pushes.iter().enumerate() {
            let head = match &push.title {
                Some(title) => t!("agents-from", title = title.to_string(), agent = (push.agent).to_string()),
                None => t!("agents-from-2", agent = (push.agent).to_string()),
            };
            rows.push(div().text_color(p.fg_dim).text_size(px(11.5)).child(crate::look::text(head)).into_any_element());
            for (j, block) in push.blocks.iter().enumerate() {
                rows.push(match block {
                    Block::Prose(text) => {
                        gpui_component::text::TextView::markdown(("agent-view-text", i * 1000 + j), text.clone())
                            .selectable(true)
                            .into_any_element()
                    }
                    Block::Code { language, text } => self.code(format!("{i}-{j}"), language, text, &p, cx),
                });
            }
        }
        let view = self.view.clone();
        base.child(
            div().flex().justify_end().px(px(8.)).py(px(4.)).border_b_1().border_color(p.border).child(
                crate::look::small_button("agent-view-clear", IconName::Trash, ts!("tabs-clear"), &p)
                    .debug_selector(|| "agent-view-clear".into())
                    .on_click(move |_, _, cx: &mut App| view.update(cx, |v, cx| v.clear(cx))),
            ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(div().flex().flex_col().gap(px(10.)).p(px(12.)).children(rows)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_push_is_cut_at_its_fences_in_the_order_it_was_written() {
        let md = "First\n\n```sql\nselect 1\n```\nThen\n\n```sh\necho hi\n\necho there\n```\nLast\n";
        assert_eq!(
            blocks(md),
            [
                Block::Prose("First".into()),
                Block::Code { language: "sql".into(), text: "select 1".into() },
                Block::Prose("Then".into()),
                Block::Code { language: "sh".into(), text: "echo hi\n\necho there".into() },
                Block::Prose("Last".into()),
            ]
        );
    }

    #[test]
    fn a_fence_never_closed_runs_to_the_end_and_a_longer_one_holds_a_shorter() {
        assert_eq!(blocks("```\nopen"), [Block::Code { language: String::new(), text: "open".into() }]);
        assert_eq!(
            blocks("````md\n```\ninner\n```\n````"),
            [Block::Code { language: "md".into(), text: "```\ninner\n```".into() }]
        );
        assert!(blocks("  \n\n").is_empty());
    }

    #[test]
    fn only_a_shells_block_offers_to_run() {
        assert!(is_shell("bash") && is_shell("powershell"));
        assert!(!is_shell("sql") && !is_shell("") && !is_shell("java"));
    }
}
