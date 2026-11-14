//! The Agent Log tool window: every call an agent made of the app, what it asked for and how it
//! ended, newest first. The session's only: the console's scrollback is the agent's record, this
//! is the app's.

use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use gpui_component::scroll::ScrollableElement as _;
use serde_json::Value;

use super::capabilities::{Capability, Effect};
use crate::theme::palette;

/// How many calls it keeps.
const KEEP: usize = 500;
/// How much of a call's arguments or answer it keeps.
const SUMMARY: usize = 200;

/// How a call ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It has not answered yet: at work, or waiting for the user.
    Waiting,
    Done,
    Failed(String),
}

/// One call.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    /// When, in seconds since the epoch.
    pub at: u64,
    pub agent: String,
    pub tool: &'static str,
    pub effect: Effect,
    /// Its arguments, shortened.
    pub arguments: String,
    pub outcome: Outcome,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `text`, at most [`SUMMARY`] characters of it on one line.
fn shortened(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= SUMMARY {
        return line;
    }
    let cut: String = line.chars().take(SUMMARY).collect();
    format!("{cut}\u{2026}")
}

/// The calls, newest last.
#[derive(Default)]
pub struct AgentLog {
    pub entries: std::collections::VecDeque<Entry>,
    next: u64,
}

impl AgentLog {
    /// Note a call as it starts. Returns its id, for [`Self::end`].
    pub fn begin(&mut self, agent: String, c: &Capability, arguments: &Value, cx: &mut Context<Self>) -> u64 {
        self.next += 1;
        let arguments = match arguments.as_object() {
            Some(a) if a.is_empty() => String::new(),
            _ => shortened(&arguments.to_string()),
        };
        self.entries.push_back(Entry {
            id: self.next,
            at: now(),
            agent,
            tool: c.name,
            effect: c.effect,
            arguments,
            outcome: Outcome::Waiting,
        });
        if self.entries.len() > KEEP {
            self.entries.pop_front();
        }
        cx.notify();
        self.next
    }

    /// Note how the call `id` ended: `error` if it failed.
    pub fn end(&mut self, id: u64, error: Option<String>, cx: &mut Context<Self>) {
        if let Some(e) = self.entries.iter_mut().rev().find(|e| e.id == id) {
            e.outcome = error.map_or(Outcome::Done, |text| Outcome::Failed(shortened(&text)));
            cx.notify();
        }
    }
}

pub struct AgentLogPanel {
    log: Entity<AgentLog>,
    focus: FocusHandle,
}

impl AgentLogPanel {
    pub fn new(log: Entity<AgentLog>, cx: &mut Context<Self>) -> Self {
        cx.observe(&log, |_, _, cx| cx.notify()).detach();
        Self { log, focus: cx.focus_handle() }
    }
}

crate::panels::panel!(AgentLogPanel, "Agent Log", |_, _| ts!("cmd-agent-log").into(), true);

fn row(e: &Entry, now: u64, cx: &App) -> gpui::AnyElement {
    let p = palette(cx);
    let (mark, tone) = match &e.outcome {
        Outcome::Waiting => (ts!("agents-waiting").to_owned(), p.fg_dim),
        Outcome::Done => (ts!("agents-done").to_owned(), p.fg_dim),
        Outcome::Failed(why) => (why.clone(), p.warn),
    };
    let effect = match e.effect {
        Effect::Read => "read",
        Effect::Show => "show",
        Effect::Change | Effect::Requested => "change",
    };
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .px(px(10.))
        .py(px(6.))
        .border_b_1()
        .border_color(p.border)
        .child(
            div()
                .flex()
                .gap(px(8.))
                .items_baseline()
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(crate::look::text(e.tool)))
                .child(div().text_color(p.fg_dim).child(crate::look::text(effect)))
                .child(div().flex_1())
                .child(div().text_color(p.fg_dim).child(crate::look::text(format!(
                    "{}, {}",
                    e.agent,
                    crate::welcome::ago(e.at, now)
                )))),
        )
        .when_some_args(&e.arguments, &p)
        .child(div().text_color(tone).child(crate::look::text(mark)))
        .into_any_element()
}

/// A call's arguments, when it had any.
trait WhenArguments {
    fn when_some_args(self, arguments: &str, p: &crate::theme::Palette) -> Self;
}

impl WhenArguments for gpui::Div {
    fn when_some_args(self, arguments: &str, p: &crate::theme::Palette) -> Self {
        if arguments.is_empty() {
            return self;
        }
        self.child(div().text_color(p.fg_dim).child(crate::look::text(arguments.to_owned())))
    }
}

impl Render for AgentLogPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let log = self.log.read(cx);
        let base = div()
            .id("agent-log")
            .debug_selector(|| "agent-log".into())
            .size_full()
            .bg(p.ground)
            .text_size(px(crate::look::BODY))
            .text_color(p.fg);
        if log.entries.is_empty() {
            return base.child(
                crate::components::EmptyState::new(snout_core::art::spots::quiet_bell(), ts!("agents-no-calls-yet"))
                    .hint(ts!("agents-what-an-agent-in-the-console-asks-of"))
                    .compact(),
            );
        }
        let at = now();
        let rows: Vec<gpui::AnyElement> = log.entries.iter().rev().map(|e| row(e, at, cx)).collect();
        base.child(div().size_full().overflow_y_scrollbar().flex().flex_col().children(rows))
    }
}
