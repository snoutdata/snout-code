//! The Processes tool window: every process SnoutCode runs, as a tree under SnoutCode itself,
//! each line saying what the process is for, with its pid, memory, CPU, age and state, and
//! Stop and Restart where they make sense. The foot has the totals and Stop Idle Processes.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, uniform_list,
};
use gpui_component::tooltip::Tooltip;

use super::{Act, Kind, Processes, Row, bytes, duration, totals};
use crate::glyphs::{Icon, icon};
use crate::theme::palette;

pub struct ProcessesWindow {
    pub processes: Entity<Processes>,
    focus: FocusHandle,
}

impl ProcessesWindow {
    pub fn new(processes: Entity<Processes>, cx: &mut Context<Self>) -> Self {
        cx.observe(&processes, |_, _, cx| cx.notify()).detach();
        Self { processes, focus: cx.focus_handle() }
    }
}

crate::panels::panel!(ProcessesWindow, "Processes", |_, _| "Processes".into(), true);

fn glyph(kind: &Kind) -> Icon {
    match kind {
        Kind::App => Icon::Code,
        Kind::Engine { .. } => Icon::Gear,
        Kind::LanguageServer { .. } => Icon::Jdk,
        Kind::Daemon { .. } => Icon::Build,
        Kind::Run { .. } => Icon::Play,
        Kind::Tests { .. } => Icon::Check,
        Kind::GradleChild { .. } => Icon::Hammer,
        Kind::Terminal => Icon::Terminal,
        Kind::Claude => Icon::Spark,
        Kind::Other => Icon::Tools,
        Kind::Leftover => Icon::Warning,
    }
}

/// The columns, right of the process's description.
const PID_W: f32 = 56.;
const MEMORY_W: f32 = 70.;
const CPU_W: f32 = 56.;
const UPTIME_W: f32 = 76.;
const STATE_W: f32 = 200.;
const ACTIONS_W: f32 = 52.;

impl Render for ProcessesWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let procs = self.processes.read(cx);
        let base = div()
            .id("processes")
            .debug_selector(|| "processes".into())
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_size(px(crate::look::BODY))
            .text_color(p.fg);
        if procs.rows.is_empty() {
            return base.child(
                crate::components::EmptyState::new(snout_core::art::spots::no_processes(), "Looking at the processes")
                    .hint("SnoutCode's own processes show here: the Java engine, Gradle daemons, runs and terminals.")
                    .compact(),
            );
        }
        let rows: Rc<Vec<Row>> = Rc::new(procs.rows.clone());
        let (count, memory, cpu) = totals(&rows);
        let idle = rows.iter().filter(|r| r.idle && r.pid.is_some()).count();
        let notice = procs.notice.clone();
        let hover = p.button_hover;
        let cell = |w: f32| div().w(px(w)).flex_none().overflow_hidden().whitespace_nowrap();
        let header = div()
            .flex_none()
            .h(px(24.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(8.))
            .border_b_1()
            .border_color(p.border)
            .text_size(px(11.5))
            .text_color(p.fg_dim)
            .child(div().flex_1().min_w_0().child("Process"))
            .child(cell(PID_W).text_right().child("PID"))
            .child(cell(MEMORY_W).text_right().child("Memory"))
            .child(cell(CPU_W).text_right().child("CPU"))
            .child(cell(UPTIME_W).text_right().child("Uptime"))
            .child(cell(STATE_W).pl(px(12.)).child("State"))
            .child(div().w(px(ACTIONS_W)).flex_none());
        let entity = self.processes.clone();
        let n = rows.len();
        let list = uniform_list("processes-rows", n, move |range, _, _| {
            range
                .map(|i| {
                    let r = rows[i].clone();
                    // The role in the text colour, what it does after the colon dimmed.
                    let head = r.purpose.find(": ").unwrap_or(r.purpose.len());
                    let text = gpui::StyledText::new(r.purpose.clone()).with_highlights([(
                        head..r.purpose.len(),
                        gpui::HighlightStyle { color: Some(p.fg_dim), ..Default::default() },
                    )]);
                    let tip: SharedString = match &r.project {
                        Some(project) => format!("{}\nProject: {project}", r.purpose).into(),
                        None => r.purpose.clone().into(),
                    };
                    let ink = match &r.kind {
                        Kind::Leftover => p.warn,
                        Kind::Engine { .. } | Kind::LanguageServer { .. } | Kind::Daemon { .. } => p.accent,
                        Kind::Run { .. } | Kind::Tests { .. } => p.ok,
                        _ => p.fg_dim,
                    };
                    let state_ink = if r.state.starts_with("Failed") || r.kind == Kind::Leftover {
                        p.warn
                    } else if r.busy {
                        p.accent
                    } else {
                        p.fg_dim
                    };
                    let num = |text: String| cell(0.).text_right().text_color(p.fg_dim).child(text);
                    let live = r.pid.is_some();
                    let mut actions = div().w(px(ACTIONS_W)).flex_none().flex().justify_end().gap(px(2.));
                    for act in r.actions() {
                        let (g, tip, ink, name) = match act {
                            Act::Restart => (Icon::Rerun, "Restart", p.fg_dim, "restart"),
                            Act::Stop => (Icon::Stop, "Stop", p.error, "stop"),
                        };
                        let entity = entity.clone();
                        let row = r.clone();
                        actions = actions.child(
                            div()
                                .id(SharedString::from(format!("process-{name}-{i}")))
                                .debug_selector(move || format!("process-{name}-{i}"))
                                .size(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .tooltip(move |w, cx| Tooltip::new(tip).build(w, cx))
                                .child(icon(g, 12., ink))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    entity.update(cx, |e, cx| e.act(&row, act, cx));
                                }),
                        );
                    }
                    div()
                        .id(("process-row", i))
                        .debug_selector(move || format!("process-row-{i}"))
                        .w_full()
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .whitespace_nowrap()
                        .hover(move |s| s.bg(hover))
                        .child(
                            div()
                                .id(("process-what", i))
                                .flex_1()
                                .min_w_0()
                                .pl(px(16. * r.depth as f32))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                                .child(div().flex_none().child(icon(glyph(&r.kind), 14., ink)))
                                .child(div().min_w_0().overflow_hidden().text_ellipsis().child(text)),
                        )
                        .child(num(r.pid.map(|p| p.to_string()).unwrap_or_default()).w(px(PID_W)))
                        .child(num(if live { bytes(r.memory) } else { String::new() }).w(px(MEMORY_W)))
                        .child(num(if live { format!("{:.1}%", r.cpu) } else { String::new() }).w(px(CPU_W)))
                        .child(num(if live { duration(r.uptime) } else { String::new() }).w(px(UPTIME_W)))
                        .child(
                            cell(STATE_W)
                                .pl(px(12.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_color(state_ink)
                                .child(
                                    div()
                                        .flex_none()
                                        .size(px(6.))
                                        .rounded(px(3.))
                                        .when(r.busy, |d| d.bg(p.accent))
                                        .when(!r.busy, |d| d.border_1().border_color(p.fg_dim)),
                                )
                                .child(div().min_w_0().overflow_hidden().text_ellipsis().child(r.state.clone())),
                        )
                        .child(actions)
                })
                .collect()
        })
        .flex_1()
        .min_h_0()
        .py(px(2.));
        let summary = format!(
            "{count} {}, {} of memory, {:.0}% CPU",
            if count == 1 { "process" } else { "processes" },
            bytes(memory),
            cpu
        );
        let processes = self.processes.clone();
        let foot = div()
            .flex_none()
            .h(px(30.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(12.))
            .border_t_1()
            .border_color(p.border)
            .text_size(px(12.))
            .text_color(p.fg_dim)
            .child(div().debug_selector(|| "processes-totals".into()).child(summary))
            .when_some(notice, |d, n| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_color(p.fg)
                        .child(crate::components::icon(crate::components::IconName::Info, 13., p.info()))
                        .child(n),
                )
            })
            .child(div().flex_1())
            .child(
                crate::look::small_button("processes-stop-idle", "Stop Idle Processes", &p)
                    .debug_selector(|| "processes-stop-idle".into())
                    .px(px(8.))
                    .h(px(22.))
                    .text_color(if idle > 0 { p.fg } else { p.fg_dim })
                    .font_weight(FontWeight::MEDIUM)
                    .tooltip(|w, cx| {
                        Tooltip::new(
                            "Stops the idle Gradle daemons, what an earlier session left running, and the engines of \
                             projects that are not open",
                        )
                        .build(w, cx)
                    })
                    .on_click(move |_, _, cx| {
                        processes.update(cx, |p, cx| {
                            p.stop_idle(cx);
                        });
                    }),
            );
        base.child(header).child(list).child(foot)
    }
}
