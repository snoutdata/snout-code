//! Lists, trees and tables, in one look: rows with room to breathe, rounded and inset; the
//! selection a clear wash of the accent while the list has the keyboard (a neutral one when
//! it does not) with a ring round the row the keyboard is on; a faint wash under the pointer;
//! no zebra stripes and no column lines, only a faint line under the header, whose columns
//! sort on a click and show their resize handles when the pointer is on them. Secondary
//! columns (folders, dates, SHAs) are dimmed; long paths are shortened in their middle, the
//! full text in the tooltip. A file's state is a small pill.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, CursorStyle, Div, DragMoveEvent, ElementId, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled, TextAlign, Window, canvas, div, point, px,
};

use super::{IconName, icon, middle_ellipsis, tip};
use crate::theme::{Palette, mix};

/// A row of a list or a table.
pub const ROW_H: f32 = 26.;
/// A row of a tree: a little tighter, as trees run long.
pub const TREE_ROW_H: f32 = 24.;
/// A table's header.
pub const HEAD_H: f32 = 26.;
/// The room between two cells of a table.
pub const CELL_GAP: f32 = 12.;
/// The room at a row's two ends.
pub const ROW_PAD: f32 = 8.;
/// The text of a row, and of a header.
pub const ROW_TEXT: f32 = 13.;
pub const HEAD_TEXT: f32 = 12.;

/// How a row stands: selected or not, whether its list has the keyboard, and whether the
/// keyboard is on this row (the lead of a selection of several).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowState {
    pub selected: bool,
    pub focused: bool,
    pub lead: bool,
}

impl RowState {
    /// A row `selected` or not, in a list that has the keyboard (`focused`) or not.
    pub fn new(selected: bool, focused: bool) -> Self {
        Self { selected, focused, lead: selected }
    }

    /// A row of a list that marks where you are (the current repository, the open file)
    /// rather than taking the keyboard: the accent's wash when `selected`, never the ring.
    pub fn current(selected: bool) -> Self {
        Self { selected, focused: true, lead: false }
    }

    /// Whether the keyboard is on this row of a selection of several (the ring goes round it alone).
    pub fn lead(mut self, lead: bool) -> Self {
        self.lead = lead && self.selected;
        self
    }

    /// The row's ground: the accent's wash, a neutral one while the list does not have the
    /// keyboard, or none.
    pub fn ground(self, p: &Palette) -> Option<Hsla> {
        match (self.selected, self.focused) {
            (false, _) => None,
            (true, true) => Some(p.row_selected()),
            (true, false) => Some(p.row_selected_dim()),
        }
    }
}

/// A row of a list, a tree or a table, `h` high: a flex row with its ground, the hover and the
/// keyboard's ring. The caller adds the cells, `on_click` and the context menu.
pub fn list_row_h(id: impl Into<ElementId>, state: RowState, h: f32, p: &Palette) -> Stateful<Div> {
    let hover = p.row_hover();
    let ring = state.selected && state.focused && state.lead;
    div()
        .id(id)
        .flex_none()
        .h(px(h))
        .w_full()
        .px(px(ROW_PAD))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(if ring { p.focus_ring().opacity(0.9) } else { gpui::transparent_black() })
        .whitespace_nowrap()
        .when_some(state.ground(p), |d, bg| d.bg(bg))
        .when(!state.selected, |d| d.hover(move |s| s.bg(hover)))
}

/// A row of a list or a table, [`ROW_H`] high.
pub fn list_row(id: impl Into<ElementId>, state: RowState, p: &Palette) -> Stateful<Div> {
    list_row_h(id, state, ROW_H, p)
}

/// A row of a tree, [`TREE_ROW_H`] high.
pub fn tree_row(id: impl Into<ElementId>, state: RowState, p: &Palette) -> Stateful<Div> {
    list_row_h(id, state, TREE_ROW_H, p)
}

/// Text of a secondary column: a folder, a date, a count. Dimmed, cut with an ellipsis.
pub fn dim(text: impl Into<SharedString>, p: &Palette) -> Div {
    div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(p.fg_dim).child(text.into())
}

/// A file's name after its type's icon, as the file chips show it.
pub fn file_label(name: impl Into<SharedString>, p: &Palette) -> Div {
    let name: SharedString = name.into();
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(7.))
        .child(crate::icons::file_icon(std::path::Path::new(name.as_ref()), 16., p))
        .child(div().min_w_0().overflow_hidden().text_ellipsis().text_color(p.fg).child(name))
}

/// A table's cell: `width` wide, or the rest of the row when `None`, its content in the middle.
pub fn cell(width: Option<f32>) -> Div {
    let d = div().h_full().min_w_0().flex().items_center().overflow_hidden().whitespace_nowrap();
    match width {
        Some(w) => d.flex_none().w(px(w)),
        None => d.flex_1(),
    }
}

/// A table's body row: the cells, laid out by `widths`.
pub fn table_row(
    id: impl Into<ElementId>,
    state: RowState,
    widths: &[Option<f32>],
    cells: Vec<AnyElement>,
    p: &Palette,
) -> Stateful<Div> {
    list_row(id, state, p).gap(px(CELL_GAP)).children(widths.iter().zip(cells).map(|(w, c)| cell(*w).child(c)))
}

// ----- Paths shortened in their middle ----------------------------------------------------

/// `text` (a path, `/`-separated) in `color`, shortened in its middle to the width it gets:
/// `scala/bamboo-web/…/solarpanda/bamboo`. The full text shows in the tooltip.
pub fn path_text(id: impl Into<ElementId>, text: impl Into<SharedString>, color: Hsla) -> Stateful<Div> {
    let text: SharedString = text.into();
    let full = text.clone();
    div().id(id).flex_1().min_w_0().h_full().when(!full.is_empty(), |d| d.tooltip(tip(full))).child(
        canvas(
            move |bounds, window, _| {
                let style = window.text_style();
                let size = style.font_size.to_pixels(window.rem_size());
                let line_h = style.line_height_in_pixels(window.rem_size());
                let shape = |s: String, window: &mut Window| {
                    let mut run = style.to_run(s.len());
                    run.color = color;
                    window.text_system().shape_line(s.into(), size, &[run], None)
                };
                let width = bounds.size.width;
                let whole = shape(text.to_string(), window);
                if whole.width <= width {
                    return (whole, line_h);
                }
                // The longest middle cut that fits.
                let (mut lo, mut hi) = (1usize, text.chars().count());
                let mut best = shape(middle_ellipsis(&text, 1), window);
                while lo < hi {
                    let mid = (lo + hi).div_ceil(2);
                    let line = shape(middle_ellipsis(&text, mid), window);
                    if line.width <= width {
                        best = line;
                        lo = mid;
                    } else {
                        hi = mid - 1;
                    }
                }
                (best, line_h)
            },
            move |bounds, (line, line_h), window, cx| {
                let y = bounds.origin.y + (bounds.size.height - line_h) / 2.;
                let _ = line.paint(point(bounds.origin.x, y), line_h, TextAlign::Left, None, window, cx);
            },
        )
        .size_full(),
    )
}

// ----- State pills ------------------------------------------------------------------------

/// A file's state in Git, as a pill: its own colour (a colour-blind-safe set: the blue of
/// lines added, the orange of lines removed, the ochre of a warning, a violet, the red
/// of errors with its icon, the green of what is done), its word always beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatePill {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
    Staged,
    PartlyStaged,
}

impl StatePill {
    pub const ALL: [StatePill; 10] = [
        Self::Modified,
        Self::Added,
        Self::Deleted,
        Self::Renamed,
        Self::Copied,
        Self::TypeChanged,
        Self::Untracked,
        Self::Conflicted,
        Self::Staged,
        Self::PartlyStaged,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Modified => "Modified",
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::TypeChanged => "Type changed",
            Self::Untracked => "Untracked",
            Self::Conflicted => "Conflicted",
            Self::Staged => "Staged",
            Self::PartlyStaged => "Partly staged",
        }
    }

    /// The pill's hue (its text is [`Palette::pill_ink`] of it).
    pub fn color(self, p: &Palette) -> Hsla {
        match self {
            Self::Modified | Self::TypeChanged => p.warn,
            Self::Added => p.added,
            Self::Deleted => p.removed,
            Self::Renamed | Self::Copied => {
                if p.tinted {
                    p.hues[4]
                } else {
                    mix(p.accent, 0.5, p.fg_dim)
                }
            }
            Self::Untracked => p.fg_dim,
            Self::Conflicted => p.error,
            Self::Staged | Self::PartlyStaged => p.ok,
        }
    }

    fn icon(self) -> Option<IconName> {
        match self {
            Self::Conflicted => Some(IconName::TriangleAlert),
            Self::Staged => Some(IconName::Check),
            Self::PartlyStaged => Some(IconName::CircleDashed),
            _ => None,
        }
    }
}

/// `state` as a small pill.
pub fn state_pill(state: StatePill, p: &Palette) -> Div {
    let color = state.color(p);
    let ink = p.pill_ink(color);
    div()
        .flex_none()
        .h(px(18.))
        .px(px(7.))
        .flex()
        .items_center()
        .gap(px(3.))
        .rounded(px(9.))
        .bg(p.wash(color))
        .border_1()
        .border_color(mix(color, 0.28, p.panel))
        .text_color(ink)
        .text_size(px(10.5))
        .line_height(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .when_some(state.icon(), |d, i| d.child(icon(i, 10., ink)))
        .child(state.label())
}

/// Several pills in a row.
pub fn state_pills(states: &[StatePill], p: &Palette) -> Div {
    div().flex().items_center().gap(px(4.)).children(states.iter().map(|s| state_pill(*s, p)))
}

// ----- Tables -----------------------------------------------------------------------------

/// A table's column: its title, its width (`None`: the rest of the row), and how narrow a
/// drag may make it.
#[derive(Clone, Debug)]
pub struct Column {
    pub title: SharedString,
    pub width: Option<f32>,
    pub min: f32,
    pub right: bool,
}

impl Column {
    pub fn new(title: impl Into<SharedString>, width: Option<f32>) -> Self {
        Self { title: title.into(), width, min: 48., right: false }
    }

    /// Its title and cells on the right (numbers).
    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }

    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }
}

/// A table's columns, as the user has sized them. Keep it in the view (an entity, so the
/// header's handles can change it) and lay the rows out by [`TableColumns::widths`].
pub struct TableColumns {
    columns: Vec<Column>,
}

impl TableColumns {
    pub fn new(columns: Vec<Column>, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { columns })
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Every column's width now (`None`: the rest of the row).
    pub fn widths(&self) -> Vec<Option<f32>> {
        self.columns.iter().map(|c| c.width).collect()
    }

    /// Set a fixed column's width, kept above its minimum.
    pub fn set_width(&mut self, i: usize, w: f32) {
        if let Some(c) = self.columns.get_mut(i)
            && c.width.is_some()
        {
            c.width = Some(w.max(c.min).round());
        }
    }

    /// Set a column's width from its data, whatever the user did (the Journal's graph follows
    /// its lanes).
    pub fn fit(&mut self, i: usize, w: f32) {
        if let Some(c) = self.columns.get_mut(i) {
            c.width = Some(w);
        }
    }
}

/// What a resize handle drags: the table and the edge.
#[derive(Clone, Debug)]
struct ColumnDrag {
    table: SharedString,
    index: usize,
}

struct NoGhost;

impl Render for NoGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// What a click on a column's title does: sort by column `i`.
pub type SortHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A table's header: each column's title (medium weight, dimmed), the arrow of the column it
/// sorts by (`sort`: the column and whether it goes up), a click on a title to sort by it
/// (`on_sort`, when the table sorts), a faint line under it, and, when the pointer is on the
/// header, the handles between the columns that resize them.
pub fn table_header(
    id: impl Into<SharedString>,
    columns: &Entity<TableColumns>,
    sort: Option<(usize, bool)>,
    on_sort: Option<SortHandler>,
    cx: &App,
) -> Stateful<Div> {
    let p = crate::theme::palette(cx);
    let id: SharedString = id.into();
    let cols = columns.read(cx).columns.clone();
    let count = cols.len();
    let group: SharedString = format!("{id}-head").into();
    let handle_color = mix(p.fg, 0.28, p.panel);
    let mut head = div()
        .id(ElementId::Name(id.clone()))
        .group(group.clone())
        .flex_none()
        .h(px(HEAD_H))
        .w_full()
        .px(px(ROW_PAD + 1.))
        .flex()
        .items_center()
        .gap(px(CELL_GAP))
        .border_b_1()
        .border_color(mix(p.border, 0.7, p.panel))
        .text_size(px(HEAD_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.fg_dim)
        .whitespace_nowrap();
    for (i, col) in cols.iter().enumerate() {
        let sorted = sort.filter(|(c, _)| *c == i).map(|(_, up)| up);
        let title = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .min_w_0()
            .overflow_hidden()
            .when(col.right, |d| d.ml_auto())
            .when(sorted.is_some(), |d| d.text_color(p.fg))
            .child(col.title.clone())
            .when_some(sorted, |d, up| {
                d.child(icon(if up { IconName::ArrowUp } else { IconName::ArrowDown }, 12., p.fg_dim))
            });
        let mut c = div()
            .id(("head", i))
            .debug_selector({
                let id = id.clone();
                move || format!("{id}-sort-{i}")
            })
            .relative()
            .h_full()
            .min_w_0()
            .flex()
            .items_center()
            .child(title);
        c = match col.width {
            Some(w) => c.flex_none().w(px(w)),
            None => c.flex_1(),
        };
        if let Some(f) = on_sort.clone() {
            c = c.cursor_pointer().on_click(move |_, window, cx| f(i, window, cx));
        }
        // The handle on the column's right edge: it sizes this column when it is fixed, or
        // the next one (from its left) when this one takes the rest of the row.
        let resizable = i + 1 < count && (col.width.is_some() || cols[i + 1].width.is_some());
        if resizable {
            let drag = ColumnDrag { table: id.clone(), index: i };
            let (table, columns) = (id.clone(), columns.clone());
            let next_w = cols.get(i + 1).and_then(|c| c.width);
            let fixed = col.width.is_some();
            c = c.on_drag_move(move |e: &DragMoveEvent<ColumnDrag>, window, cx| {
                let d = e.drag(cx);
                if d.table != table || d.index != i {
                    return;
                }
                let x = e.event.position.x;
                columns.update(cx, |t, _| {
                    if fixed {
                        t.set_width(i, f32::from(x - e.bounds.left()));
                    } else if let Some(next) = next_w {
                        let right = f32::from(e.bounds.right()) + CELL_GAP + next;
                        t.set_width(i + 1, right - f32::from(x) - CELL_GAP);
                    }
                });
                window.refresh();
            });
            c = c.child(
                div()
                    .id(("resize", i))
                    .debug_selector({
                        let id = id.clone();
                        move || format!("{id}-resize-{i}")
                    })
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(-(CELL_GAP / 2. + 4.)))
                    .w(px(8.))
                    .flex()
                    .justify_center()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .on_drag(drag, |_, _, _, cx| cx.new(|_| NoGhost))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(div().w(px(1.)).my(px(6.)).h_full().group_hover(group.clone(), move |s| s.bg(handle_color))),
            );
        }
        head = head.child(c);
    }
    head
}

/// The strip of a table's header, for tables that lay out their own titles: the height,
/// the type, the faint line under it. Its titles go in with [`head_title`].
pub fn header_bar(p: &Palette) -> Div {
    div()
        .flex_none()
        .h(px(HEAD_H))
        .px(px(ROW_PAD + 1.))
        .flex()
        .items_center()
        .gap(px(CELL_GAP))
        .border_b_1()
        .border_color(mix(p.border, 0.7, p.panel))
        .text_size(px(HEAD_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.fg_dim)
        .whitespace_nowrap()
}

/// A column's title in a [`header_bar`], with the arrow when the table sorts by it (`Some(up)`).
pub fn head_title(text: impl Into<SharedString>, sorted: Option<bool>, p: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .min_w_0()
        .overflow_hidden()
        .when(sorted.is_some(), |d| d.text_color(p.fg))
        .child(text.into())
        .when_some(sorted, |d, up| {
            d.child(icon(if up { IconName::ArrowUp } else { IconName::ArrowDown }, 12., p.fg_dim))
        })
}

/// A table: its header over its body (a `uniform_list` of [`table_row`]s, usually).
pub fn table(header: impl IntoElement, body: impl IntoElement) -> Div {
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(header)
        .child(div().flex_1().min_h_0().pt(px(3.)).flex().flex_col().child(body))
}

// ----- Section headers -------------------------------------------------------------------

/// The title of a group of rows in a list (Local Branches, Tags): small, medium weight,
/// dimmed.
pub fn section_title(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .text_size(px(HEAD_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.fg_dim)
        .whitespace_nowrap()
        .child(text.into())
}
