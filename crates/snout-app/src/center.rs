//! The center tabs: the diff of the selected file, the Journal (history with its graph),
//! and the Commit view of a commit picked in the Journal.

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, BorderStyle, Bounds, ClickEvent, Context, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Pixels, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, point, px, quad, uniform_list,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dock::{BasePanel, BasePanelView, Panel, PanelEvent, panel_handle};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{Selectable as _, Sizable as _, Theme};
use snout_core::diff_layout::{DiffLayout, SbsLine, UniLine, gap_label};
use snout_core::graph::GraphRow;
use snout_core::{CheckoutTarget, DiffBase, RowKind};

use crate::model::{Action, Model, StartPoint};
use crate::panels::panel;
use crate::theme::{Palette, palette};
use crate::ui::{change_color, icon, icons, note, pane_title, ref_badge};

pub enum Kind {
    Diff,
    Journal,
    Commit,
}

pub fn build<T>(
    model: &Entity<Model>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Vec<(Kind, Arc<dyn BasePanelView>)> {
    let diff = cx.new(|cx| DiffPanel::new(model.clone(), cx));
    let journal = cx.new(|cx| JournalPanel::new(model.clone(), window, cx));
    let commit = cx.new(|cx| CommitPanel::new(model.clone(), cx));
    vec![(Kind::Journal, panel_handle(journal)), (Kind::Diff, panel_handle(diff)), (Kind::Commit, panel_handle(commit))]
}

fn send(model: &Entity<Model>, action: Action) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let model = model.clone();
    move |_, _, cx| {
        let a = action.clone();
        model.update(cx, |m, cx| m.handle(a, cx))
    }
}

fn mono(cx: &App) -> SharedString {
    Theme::global(cx).mono_font_family.clone()
}

// ----- Diff ----------------------------------------------------------------------------------

/// Path and +/− counts of the diff showing.
fn diff_title(diff: Option<&Result<Rc<DiffLayout>, String>>, p: &Palette, cx: &App) -> AnyElement {
    let Some(Ok(view)) = diff else { return div().into_any_element() };
    let d = &view.diff;
    div()
        .flex()
        .gap(px(8.))
        .min_w_0()
        .overflow_hidden()
        .child(div().font_family(mono(cx)).whitespace_nowrap().child(d.path.clone()))
        .when(!d.binary && !d.too_large, |e| {
            e.child(div().text_color(p.ok).child(format!("+{}", d.additions)))
                .child(div().text_color(p.error).child(format!("−{}", d.deletions)))
        })
        .into_any_element()
}

/// Layout toggles shared by every diff.
fn diff_controls(model: &Entity<Model>, cx: &App) -> AnyElement {
    let s = &model.read(cx).settings;
    let (sbs, compact) = (s.side_by_side, s.compact);
    let set = |side: Option<bool>, compact: Option<bool>| {
        let model = model.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            model.update(cx, |m, cx| m.set_diff_options(side, compact, cx))
        }
    };
    div()
        .flex()
        .gap(px(2.))
        .child(Button::new("sbs").ghost().xsmall().label("Side by Side").selected(sbs).on_click(set(Some(true), None)))
        .child(Button::new("uni").ghost().xsmall().label("Unified").selected(!sbs).on_click(set(Some(false), None)))
        .child(
            Button::new("compact")
                .ghost()
                .xsmall()
                .label("Compact")
                .selected(compact)
                .tooltip("Hide unchanged lines away from changes")
                .on_click(set(None, Some(!compact))),
        )
        .into_any_element()
}

/// The diff itself, or a note saying why there is none.
fn diff_body(
    id: &'static str,
    diff: Option<&Result<Rc<DiffLayout>, String>>,
    (left, right): (&str, &str),
    side_by_side: bool,
    empty: &str,
    cx: &App,
) -> AnyElement {
    let p = palette(cx);
    let view = match diff {
        None => return note("", empty.to_owned(), cx).into_any_element(),
        Some(Err(e)) => return note("", e.clone(), cx).into_any_element(),
        Some(Ok(view)) => view.clone(),
    };
    let d = &view.diff;
    if d.binary {
        return note("", "Binary file differs", cx).into_any_element();
    }
    if d.too_large {
        return note("", "File too large to display", cx).into_any_element();
    }
    if d.is_identical() {
        return note("", format!("No differences between {left} and {right}"), cx).into_any_element();
    }
    let font = mono(cx);
    let count = view.len(side_by_side);
    let digits = view.number_digits;
    let list = uniform_list(id, count, move |range, _, _| {
        range.map(|i| diff_line(&view, i, side_by_side, digits, &p)).collect()
    })
    .flex_1()
    .font_family(font)
    .text_size(px(12.5));
    div()
        .size_full()
        .flex()
        .flex_col()
        .when(side_by_side, |e| {
            e.child(
                div()
                    .flex()
                    .text_color(p.fg_dim)
                    .text_size(px(12.))
                    .child(div().flex_1().px(px(6.)).child(left.to_owned()))
                    .child(div().flex_1().px(px(6.)).child(right.to_owned())),
            )
        })
        .child(list)
        .into_any_element()
}

fn number(n: Option<u32>, digits: usize, p: &Palette) -> gpui::Div {
    div()
        .flex_none()
        .w(px((digits as f32 + 1.0) * 7.6))
        .text_right()
        .pr(px(6.))
        .text_color(p.fg_dim)
        .child(n.map(|n| n.to_string()).unwrap_or_default())
}

fn cell(
    bg: Option<Hsla>,
    numbers: &[Option<u32>],
    marker: &str,
    text: Option<&str>,
    digits: usize,
    p: &Palette,
) -> gpui::Div {
    let mut row = div().flex().flex_1().min_w_0().h_full().overflow_hidden().when_some(bg, |d, bg| d.bg(bg));
    for n in numbers {
        row = row.child(number(*n, digits, p));
    }
    if !marker.is_empty() {
        row = row.child(div().w(px(12.)).text_color(p.fg_dim).child(marker.to_owned()));
    }
    row.child(div().whitespace_nowrap().child(text.unwrap_or_default().replace('\t', "    ")))
}

fn diff_line(view: &DiffLayout, i: usize, side_by_side: bool, digits: usize, p: &Palette) -> AnyElement {
    let rows = &view.diff.rows;
    let gap = |n: usize| {
        div()
            .h(px(20.))
            .w_full()
            .flex()
            .justify_center()
            .bg(p.input)
            .text_color(p.fg_dim)
            .child(gap_label(n))
            .into_any_element()
    };
    let line = div().h(px(20.)).w_full().flex();
    if side_by_side {
        let row = match view.sbs[i] {
            SbsLine::Gap(n) => return gap(n),
            SbsLine::Row(i) => &rows[i],
        };
        let (old_bg, new_bg) = match row.kind {
            RowKind::Equal => (None, None),
            RowKind::Delete => (Some(p.diff_del()), None),
            RowKind::Insert => (None, Some(p.diff_add())),
            RowKind::Replace => (Some(p.diff_del()), Some(p.diff_add())),
        };
        let old_bg = if row.old.is_none() { Some(p.stripe) } else { old_bg };
        let new_bg = if row.new.is_none() { Some(p.stripe) } else { new_bg };
        line.child(cell(
            old_bg,
            &[row.old.as_ref().map(|l| l.0)],
            "",
            row.old.as_ref().map(|l| l.1.as_str()),
            digits,
            p,
        ))
        .child(div().w(px(1.)).h_full().bg(p.border))
        .child(cell(new_bg, &[row.new.as_ref().map(|l| l.0)], "", row.new.as_ref().map(|l| l.1.as_str()), digits, p))
        .into_any_element()
    } else {
        match view.unified[i] {
            UniLine::Gap(n) => gap(n),
            UniLine::Both(i) => {
                let r = &rows[i];
                let (o, n) = (r.old.as_ref().unwrap(), r.new.as_ref().unwrap());
                line.child(cell(None, &[Some(o.0), Some(n.0)], " ", Some(&n.1), digits, p)).into_any_element()
            }
            UniLine::Old(i) => {
                let o = rows[i].old.as_ref().unwrap();
                line.child(cell(Some(p.diff_del()), &[Some(o.0), None], "-", Some(&o.1), digits, p)).into_any_element()
            }
            UniLine::New(i) => {
                let n = rows[i].new.as_ref().unwrap();
                line.child(cell(Some(p.diff_add()), &[None, Some(n.0)], "+", Some(&n.1), digits, p)).into_any_element()
            }
        }
    }
}

/// The selected changed file's diff.
pub struct DiffPanel {
    model: Entity<Model>,
    focus: FocusHandle,
}

impl DiffPanel {
    fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, focus: cx.focus_handle() }
    }
}

panel!(DiffPanel, "Changes", |_, _| "Changes".into(), true);

impl Render for DiffPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        let base = m.settings.diff_base;
        let empty = if m.selected.len() > 1 {
            format!("{} files selected", m.selected.len())
        } else {
            "Select a file to see its changes".to_owned()
        };
        let model = self.model.clone();
        let compare =
            Button::new("compare").ghost().xsmall().label(base.label()).tooltip("What to compare").dropdown_menu(
                move |mut menu, _, _| {
                    for b in DiffBase::ALL {
                        let model = model.clone();
                        menu = menu.item(
                            PopupMenuItem::new(b.label())
                                .checked(b == base)
                                .on_click(move |_, _, cx| model.update(cx, |m, cx| m.set_diff_base(b, cx))),
                        );
                    }
                    menu
                },
            );
        div()
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(diff_title(m.diff.as_ref(), &p, cx))
                    .child(div().flex_1())
                    .child(compare)
                    .child(diff_controls(&self.model, cx)),
            )
            .child(div().flex_1().min_h_0().child(diff_body(
                "diff-lines",
                m.diff.as_ref(),
                base.sides(),
                m.settings.side_by_side,
                &empty,
                cx,
            )))
    }
}

// ----- Journal -------------------------------------------------------------------------------

const LANE_W: f32 = 12.0;
const ROW_H: f32 = 26.0;

fn lane_x(bounds: Bounds<Pixels>, lane: usize) -> Pixels {
    bounds.left() + px(LANE_W * 0.5 + 4.0 + lane as f32 * LANE_W)
}

/// Paint one row of the graph. `filled` paints the commit solid (not pushed yet); `head`
/// adds a ring.
fn paint_graph(bounds: Bounds<Pixels>, row: &GraphRow, filled: bool, head: bool, p: &Palette, window: &mut Window) {
    let colors = p.lanes();
    let color = |lane: usize| colors[lane % colors.len()];
    let (top, mid, bottom) = (bounds.top(), bounds.center().y, bounds.bottom());
    for e in &row.edges {
        let (y0, y1) = if e.top { (top, mid) } else { (mid, bottom) };
        let (a, b) = (point(lane_x(bounds, e.from), y0), point(lane_x(bounds, e.to), y1));
        // A line keeps the colour of the lane it belongs to: the branch it comes from when
        // it converges, the branch it goes to when it forks.
        let c = color(if e.top { e.from } else { e.to });
        let mut path = PathBuilder::stroke(px(1.6));
        path.move_to(a);
        if a.x == b.x {
            path.line_to(b);
        } else {
            let m = point((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            path.curve_to(m, point(a.x, m.y));
            path.curve_to(b, point(b.x, m.y));
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, c);
        }
    }
    let c = point(lane_x(bounds, row.lane), mid);
    let lane = color(row.lane);
    let dot = |r: f32| Bounds::new(point(c.x - px(r), c.y - px(r)), gpui::size(px(2. * r), px(2. * r)));
    if head {
        window.paint_quad(quad(dot(6.5), px(6.5), gpui::transparent_black(), px(1.5), lane, BorderStyle::Solid));
    }
    window.paint_quad(quad(dot(4.0), px(4.0), if filled { lane } else { p.panel }, px(1.8), lane, BorderStyle::Solid));
}

pub struct JournalPanel {
    model: Entity<Model>,
    filter: Entity<InputState>,
    focus: FocusHandle,
}

impl JournalPanel {
    fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Message, author, SHA or ref"));
        cx.subscribe(&filter, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                this.model.update(cx, |m, cx| {
                    m.journal_filter = text;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, filter, focus: cx.focus_handle() }
    }
}

panel!(JournalPanel, "Journal", |_, _| "Journal".into(), true);

impl Render for JournalPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        if m.repo.is_none() {
            return div().size_full().bg(p.panel).child(note("", "No repository open", cx));
        }
        if m.log.is_empty() {
            return div().size_full().bg(p.panel).child(note(
                "No commits yet",
                "Stage some files and make the first one",
                cx,
            ));
        }
        let rows = m.journal_rows();
        // Lanes only mean something over the whole history, so a filter hides them.
        let show_graph = rows.len() == m.log.len();
        let graph_w = if show_graph { snout_core::graph::lane_count(&m.graph) as f32 * LANE_W + 8.0 } else { 14.0 };
        let log = Rc::new(m.log.clone());
        let graph = Rc::new(m.graph.clone());
        let shown = m.commit_view.as_ref().and_then(|v| v.id()).map(str::to_owned);
        let hint = m.commit_view.is_none();
        let model = self.model.clone();
        let mono = mono(cx);
        let list = uniform_list("journal", rows.len(), move |range, _, _| {
            range
                .map(|k| {
                    let i = rows[k];
                    let c = &log[i];
                    let is_head = c.refs.iter().any(|r| r == "HEAD" || r.starts_with("HEAD -> "));
                    let selected = shown.as_deref() == Some(c.id.as_str());
                    let (row, unpushed) = (graph.get(i).cloned().unwrap_or_default(), c.unpushed);
                    let (id, summary, short) = (c.id.clone(), c.summary.clone(), c.short_id.clone());
                    let model = model.clone();
                    div()
                        .id(("commit", k))
                        .h(px(ROW_H))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .when(k % 2 == 1, |d| d.bg(p.stripe))
                        .when(selected, |d| d.bg(p.selection()))
                        .hover(|s| s.bg(p.button_hover))
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    if show_graph {
                                        paint_graph(bounds, &row, unpushed, is_head, &p, window);
                                    } else if unpushed {
                                        let c = bounds.center();
                                        let b = Bounds::new(
                                            point(c.x - px(3.5), c.y - px(3.5)),
                                            gpui::size(px(7.), px(7.)),
                                        );
                                        window.paint_quad(quad(
                                            b,
                                            px(3.5),
                                            p.ill_accent,
                                            px(0.),
                                            p.ill_accent,
                                            BorderStyle::Solid,
                                        ));
                                    }
                                },
                            )
                            .flex_none()
                            .w(px(graph_w))
                            .h_full(),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .overflow_hidden()
                                .children(c.refs.iter().map(|r| ref_badge(r, &p)))
                                .child(
                                    div()
                                        .whitespace_nowrap()
                                        .when(c.unpushed, |d| d.font_weight(FontWeight::SEMIBOLD))
                                        .child(c.summary.clone()),
                                ),
                        )
                        .child(
                            div().flex_none().w(px(150.)).overflow_hidden().whitespace_nowrap().child(c.author.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(130.))
                                .text_color(p.fg_dim)
                                .whitespace_nowrap()
                                .child(c.date.clone()),
                        )
                        .child(div().flex_none().w(px(70.)).font_family(mono.clone()).child(c.short_id.clone()))
                        .on_click({
                            let (model, id) = (model.clone(), id.clone());
                            move |_, _, cx| {
                                if !selected {
                                    let a = Action::ShowCommit(id.clone());
                                    model.update(cx, |m, cx| m.handle(a, cx));
                                }
                            }
                        })
                        .context_menu(move |menu, _, _| {
                            let start = StartPoint { label: format!("{short} {summary}"), rev: id.clone() };
                            menu.item(
                                PopupMenuItem::new("Show changes")
                                    .on_click(send(&model, Action::ShowCommit(id.clone()))),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Check out this commit (detached)").on_click(send(
                                    &model,
                                    Action::RequestCheckout(CheckoutTarget::Commit(id.clone())),
                                )),
                            )
                            .item(
                                PopupMenuItem::new("Create branch here…")
                                    .on_click(send(&model, Action::ShowNewBranch(Some(start)))),
                            )
                            .separator()
                            .item(PopupMenuItem::new("Copy SHA").on_click(send(&model, Action::Copy(id.clone()))))
                            .item(
                                PopupMenuItem::new("Copy message")
                                    .on_click(send(&model, Action::Copy(summary.clone()))),
                            )
                        })
                })
                .collect()
        })
        .flex_1();
        div()
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().w(px(260.)).child(Input::new(&self.filter).small()))
                    .when(hint, |d| d.child(div().text_color(p.fg_dim).child("Click a commit to see what it changed"))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(div().w(px(graph_w)))
                    .child(div().flex_1().child("Message"))
                    .child(div().w(px(150.)).child("Author"))
                    .child(div().w(px(130.)).child("Date"))
                    .child(div().w(px(70.)).child("SHA")),
            )
            .child(list)
    }
}

// ----- Commit view ---------------------------------------------------------------------------

pub struct CommitPanel {
    model: Entity<Model>,
    focus: FocusHandle,
}

impl CommitPanel {
    fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, focus: cx.focus_handle() }
    }
}

panel!(
    CommitPanel,
    "Commit",
    |s, cx| match s.model.read(cx).commit_view.as_ref().and_then(|v| v.details.as_ref().ok()) {
        Some(d) => format!("Commit {}", d.short_id).into(),
        None => "Commit".into(),
    },
    true
);

impl Render for CommitPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let model = self.model.clone();
        let m = self.model.read(cx);
        let Some(view) = &m.commit_view else {
            return div().size_full().bg(p.panel).child(note(
                "",
                "Click a commit in the Journal to see what it changed",
                cx,
            ));
        };
        let back = Button::new("back")
            .ghost()
            .small()
            .label("Working tree")
            .tooltip("Back to the working tree")
            .on_click(send(&model, Action::CloseCommit));
        let d = match &view.details {
            Ok(d) => d.clone(),
            Err(e) => {
                return div().size_full().bg(p.panel).flex().flex_col().child(back).child(note("", e.clone(), cx));
            }
        };
        let root = m.repo.as_ref().map(|r| r.root().to_owned());
        let selected = view.selected.clone();
        let files = div()
            .id("commit-files")
            .size_full()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(2.))
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(pane_title("Files"))
                    .child(div().text_color(p.fg_dim).child(format!("{} changed", d.files.len()))),
            )
            .when(d.files.is_empty(), |e| e.child(note("", "This commit changes no files", cx)))
            .children(d.files.iter().enumerate().map(|(i, f)| {
                let dir = match &f.orig_path {
                    Some(orig) => format!("{}  (from {orig})", f.directory()),
                    None => f.directory().to_owned(),
                };
                let path = f.path.clone();
                let is_sel = selected.as_ref() == Some(&f.path);
                let (model, root) = (model.clone(), root.clone());
                div()
                    .id(("cfile", i))
                    .flex()
                    .gap(px(8.))
                    .h(px(24.))
                    .items_center()
                    .px(px(6.))
                    .rounded(px(4.))
                    .when(is_sel, |e| e.bg(p.selection()))
                    .hover(|s| s.bg(p.button_hover))
                    .child(div().w(px(180.)).overflow_hidden().whitespace_nowrap().child(f.file_name().to_owned()))
                    .child(div().w(px(90.)).text_color(change_color(f.change, &p)).child(f.change.label()))
                    .child(
                        div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_color(p.fg_dim).child(dir),
                    )
                    .on_click({
                        let (model, path) = (model.clone(), path.clone());
                        move |_, _, cx| model.update(cx, |m, cx| m.select_commit_file(path.clone(), cx))
                    })
                    .context_menu(move |menu, _, _| {
                        let menu = match &root {
                            Some(root) => menu.item(
                                PopupMenuItem::new("Reveal in Finder")
                                    .on_click(send(&model, Action::Reveal(root.join(&path)))),
                            ),
                            None => menu,
                        };
                        menu.item(PopupMenuItem::new("Copy path").on_click(send(&model, Action::Copy(path.clone()))))
                    })
            }));

        let key = |k: &str| div().w(px(90.)).flex_none().text_color(p.fg_dim).child(k.to_owned());
        let fact = |k: &str, v: AnyElement| div().flex().gap(px(10.)).child(key(k)).child(v);
        let differs = (&d.committer, &d.committer_email) != (&d.author, &d.email) || d.committer_date != d.date;
        let parents: Vec<AnyElement> = d
            .parents
            .iter()
            .enumerate()
            .map(|(i, parent)| {
                let summary = m.log.iter().find(|c| &c.id == parent).map(|c| c.summary.clone()).unwrap_or_default();
                Button::new(("parent", i))
                    .link()
                    .label(parent[..parent.len().min(7)].to_owned())
                    .tooltip(summary)
                    .on_click(send(&model, Action::ShowCommit(parent.clone())))
                    .into_any_element()
            })
            .collect();
        let details = div()
            .id("commit-details")
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(8.))
            .overflow_y_scrollbar()
            .child(pane_title("Commit"))
            .when(!d.refs.is_empty(), |e| {
                e.child(div().flex().flex_wrap().gap(px(4.)).children(d.refs.iter().map(|r| ref_badge(r, &p))))
            })
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(d.summary().to_owned()))
            .when(!d.body().is_empty(), |e| e.child(div().child(d.body().to_owned())))
            .child(fact("Author", div().child(format!("{} <{}>", d.author, d.email)).into_any_element()))
            .child(fact("Date", div().child(d.date.clone()).into_any_element()))
            .when(differs, |e| {
                e.child(fact(
                    "Committer",
                    div().child(format!("{} <{}>", d.committer, d.committer_email)).into_any_element(),
                ))
                .child(fact("Committed", div().child(d.committer_date.clone()).into_any_element()))
            })
            .child(fact(
                "SHA",
                div()
                    .flex()
                    .flex_wrap()
                    .min_w_0()
                    .gap(px(6.))
                    .items_center()
                    .child(div().font_family(mono(cx)).text_size(px(11.)).child(d.id.clone()))
                    .child(
                        Button::new("copy-sha")
                            .xsmall()
                            .label("Copy")
                            .on_click(send(&model, Action::Copy(d.id.clone()))),
                    )
                    .into_any_element(),
            ))
            .child(fact(
                if d.parents.len() > 1 { "Parents" } else { "Parent" },
                if d.parents.is_empty() {
                    div().text_color(p.fg_dim).child("none, this is the first commit").into_any_element()
                } else {
                    div().flex().flex_col().children(parents).into_any_element()
                },
            ));

        let empty = if d.files.is_empty() { "This commit changes no files" } else { "Select a file" };
        let parent = if d.parents.is_empty() { "(nothing)" } else { "Parent" };
        let diff = div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(pane_title("Changes"))
                    .child(diff_title(view.diff.as_ref(), &p, cx))
                    .child(div().flex_1())
                    .child(diff_controls(&model, cx)),
            )
            .child(div().flex_1().min_h_0().child(diff_body(
                "commit-diff",
                view.diff.as_ref(),
                (parent, "This commit"),
                m.settings.side_by_side,
                empty,
                cx,
            )));

        div()
            .size_full()
            .bg(p.panel)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .p(px(6.))
                    .child(back)
                    .child(icon(icons::COMMIT).small())
                    .child(div().font_family(mono(cx)).font_weight(FontWeight::SEMIBOLD).child(d.short_id.clone()))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(d.summary().to_owned()),
                    ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable("commit-view")
                        .child(
                            resizable_panel().child(
                                h_resizable("commit-top")
                                    .child(resizable_panel().child(files))
                                    .child(resizable_panel().size(px(420.)).child(details)),
                            ),
                        )
                        .child(resizable_panel().size(px(380.)).child(diff)),
                ),
            )
    }
}
