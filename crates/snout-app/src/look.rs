//! The Git view's pieces, in the window's design language: its typefaces and sizes, the
//! pane frame, the separators, and its buttons (glyph, chip, icon, toggle and primary),
//! quiet until the pointer is on them, with line icons. The layout keeps the egui app's
//! measures.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Div, ElementId, Entity, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::tooltip::Tooltip;
use snout_core::art::Glyph;

use crate::art::outline_glyph;
use crate::theme::{Palette, mix, palette};

/// The interface's typeface (Inter, bundled) and the editor's (Hack).
pub const FONT: &str = "Inter";
pub const MONO: &str = "Hack";
/// Text sizes: body and buttons, monospace, small.
pub const BODY: f32 = 13.0;
pub const MONO_SIZE: f32 = 13.0;
pub const SMALL: f32 = 9.0;
/// The typefaces in use, as the settings choose them.
pub struct Fonts {
    pub ui: SharedString,
    pub ui_size: f32,
    pub mono: SharedString,
}

impl gpui::Global for Fonts {}

/// The interface's typeface now.
pub fn ui_font(cx: &App) -> SharedString {
    cx.try_global::<Fonts>().map_or_else(|| FONT.into(), |f| f.ui.clone())
}

/// The interface's text size now.
pub fn ui_size(cx: &App) -> f32 {
    cx.try_global::<Fonts>().map_or(BODY, |f| f.ui_size)
}

/// The editors' typeface now.
pub fn mono_font(cx: &App) -> SharedString {
    cx.try_global::<Fonts>().map_or_else(|| MONO.into(), |f| f.mono.clone())
}

/// The height egui gives a widget row.
pub const INTERACT: f32 = 20.0;
/// A line of body text.
pub const LINE: f32 = 16.0;

/// A pane: lifted off the ground with a hairline and rounded corners (egui's `theme::pane`).
///
/// It fills its slot (a flex column) less a 3-point margin all round, as egui's does.
pub fn pane(p: &Palette) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .m(px(3.))
        .px(px(10.))
        .py(px(8.))
        .bg(p.panel)
        .border_1()
        .border_color(p.border)
        .rounded(px(6.))
        .flex()
        .flex_col()
        .overflow_hidden()
}

/// egui's horizontal separator under a pane's title row: a hairline with room on both sides.
pub fn separator(p: &Palette) -> Div {
    div().flex_none().h(px(16.)).flex().flex_col().justify_center().child(div().h(px(1.)).bg(p.border))
}

/// egui's vertical separator, between groups of toolbar buttons.
pub fn vseparator(p: &Palette) -> Div {
    div().flex_none().px(px(3.)).h(px(22.)).flex().items_center().child(div().w(px(1.)).h(px(20.)).bg(p.border))
}

/// A pane's title.
pub fn title(text: impl Into<SharedString>) -> Div {
    div().flex_none().whitespace_nowrap().child(text.into())
}

/// Weak (dimmed) text.
pub fn weak(text: impl Into<SharedString>, p: &Palette) -> Div {
    div().whitespace_nowrap().text_color(p.weak()).child(text.into())
}

fn tip(text: SharedString) -> impl Fn(&mut gpui::Window, &mut App) -> gpui::AnyView {
    move |w, cx| Tooltip::new(text.clone()).build(w, cx)
}

/// A toolbar button: a line icon and its label, quiet until the pointer is on it.
pub fn glyph_button(
    id: impl Into<ElementId>,
    g: Glyph,
    text: impl Into<SharedString>,
    enabled: bool,
    p: &Palette,
) -> Stateful<Div> {
    let text: SharedString = text.into();
    let ink = if enabled { p.fg } else { p.fg_dim.opacity(0.6) };
    let icon_ink = if enabled { mix(p.fg, 0.8, p.ground) } else { ink };
    let hover_bg = p.button_hover;
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(if text.is_empty() { 0. } else { 6. }))
        .h(px(26.))
        .px(px(8.))
        .rounded(px(5.))
        .text_color(ink)
        .whitespace_nowrap()
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover_bg)))
        .child(outline_glyph(g, 15., icon_ink))
        .when(!text.is_empty(), |d| d.child(text))
}

/// A quiet pill for pane headers: glyph and label, filled only under the pointer.
pub fn chip_button(
    id: impl Into<ElementId>,
    g: Glyph,
    text: impl Into<SharedString>,
    enabled: bool,
    p: &Palette,
) -> Stateful<Div> {
    let ink = if enabled { p.fg } else { p.fg_dim.opacity(0.6) };
    let (hover_bg, border) = (p.button_hover, p.border);
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(5.))
        .h(px(22.))
        .px(px(8.))
        .rounded(px(11.))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_color(ink)
        .whitespace_nowrap()
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover_bg).border_color(border)))
        .child(outline_glyph(g, 14., ink))
        .child(text.into())
}

/// A small frameless icon button, for pane headers.
pub fn icon_button(
    id: impl Into<ElementId>,
    g: Glyph,
    tooltip: &'static str,
    enabled: bool,
    p: &Palette,
) -> Stateful<Div> {
    let ink = if enabled { p.fg } else { p.fg_dim.opacity(0.6) };
    let hover_bg = p.button_hover;
    div()
        .id(id)
        .flex_none()
        .size(px(22.))
        .p(px(3.))
        .rounded(px(5.))
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover_bg)))
        .tooltip(tip(crate::keymap::shown(tooltip).into()))
        .child(outline_glyph(g, 16., ink))
}

/// egui's selectable label with its icon: filled with the selection colour when on.
pub fn toggle(
    id: impl Into<ElementId>,
    ico: impl Into<crate::components::Ico>,
    text: impl Into<SharedString>,
    on: bool,
    p: &Palette,
) -> Stateful<Div> {
    let hover_bg = p.button_hover;
    let ink = if on { p.fg } else { mix(p.fg, 0.7, p.fg_dim) };
    div()
        .id(id)
        .flex_none()
        .h(px(INTERACT + 2.))
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded(px(6.))
        .whitespace_nowrap()
        .cursor_pointer()
        .when(on, |d| d.bg(p.selection()))
        .when(!on, |d| d.hover(move |s| s.bg(hover_bg)))
        .child(crate::components::icon(ico, 13., ink))
        .child(text.into())
}

/// A small framed button with its icon, for bars and rows: the shared button's look,
/// compact.
pub fn small_button(
    id: impl Into<ElementId>,
    ico: impl Into<crate::components::Ico>,
    text: impl Into<SharedString>,
    p: &Palette,
) -> Stateful<Div> {
    let (hover_bg, pressed) = (p.button_hover, p.active);
    div()
        .id(id)
        .flex_none()
        .px(px(8.))
        .h(px(22.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded(px(6.))
        .border_1()
        .border_color(mix(p.fg, 0.14, p.button))
        .bg(p.button)
        .text_size(px(12.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .active(move |s| s.bg(pressed))
        .whitespace_nowrap()
        .child(crate::components::icon(ico, 12., p.fg))
        .child(text.into())
}

/// The primary action of a pane, with its icon: the accent, solid, grey while it cannot be
/// pressed.
pub fn primary_button(
    id: impl Into<ElementId>,
    icon: crate::components::IconName,
    text: &'static str,
    enabled: bool,
    p: &Palette,
) -> Stateful<Div> {
    let b = p.primary();
    let (fill, ink) = if enabled { (b.fill, b.ink) } else { (p.button, p.fg_dim) };
    let (hover, pressed) = (b.hover, b.pressed);
    div()
        .id(id)
        .flex_none()
        .min_w(px(88.))
        .h(px(28.))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(7.))
        .bg(fill)
        .font_weight(gpui::FontWeight::MEDIUM)
        .when(enabled, |d| d.shadow(p.glow_of(p.accent)))
        .text_color(ink)
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)).active(move |s| s.bg(pressed)))
        .child(crate::components::icon(icon, 14., ink))
        .child(text)
}

/// A text field drawn as egui draws one: the input colour, a hairline, rounded corners,
/// and an optional magnifier in front.
pub fn field(state: &Entity<InputState>, search: bool, width: Option<f32>, p: &Palette) -> Div {
    div()
        .flex_none()
        .when_some(width, |d, w| d.w(px(w)))
        .when(width.is_none(), |d| d.w_full())
        .h(px(INTERACT + 2.))
        .px(px(6.))
        .flex()
        .items_center()
        .gap(px(5.))
        .bg(p.input)
        .border_1()
        .border_color(mix(p.fg, 0.08, p.button))
        .rounded(px(6.))
        .when(search, |d| d.child(magnifier(p.weak())))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(state).appearance(false).px(px(0.)).text_size(px(BODY)).h(px(INTERACT))),
        )
}

/// A row of `(text, colour)` chips: small rounded labels in a soft wash of their colour.
pub fn chip(text: impl Into<SharedString>, color: Hsla, p: &Palette) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .h(px(17.))
        .flex()
        .items_center()
        .rounded(px(9.))
        .border_1()
        .border_color(mix(color, 0.4, p.panel))
        .bg(p.wash(color))
        .text_color(color)
        .text_size(px(11.5))
        .whitespace_nowrap()
        .child(text.into())
}

/// A ref beside a commit, as egui draws it: small text in a wash of its colour.
pub fn ref_badge(r: &str, p: &Palette) -> Div {
    let (text, color) = if let Some(branch) = r.strip_prefix("HEAD -> ") {
        (branch, p.branch())
    } else if let Some(tag) = r.strip_prefix("tag: ") {
        (tag, p.warn)
    } else if r == "HEAD" {
        (r, p.branch())
    } else {
        (r, p.fg_dim)
    };
    div()
        .flex_none()
        .px(px(6.))
        .h(px(18.))
        .flex()
        .items_center()
        .rounded(px(8.))
        .border_1()
        .border_color(mix(color, 0.45, p.panel))
        .bg(p.wash(color))
        .text_color(color)
        .text_size(px(SMALL))
        .whitespace_nowrap()
        .child(text.to_owned())
}

/// A centred drawing above a short message: how an empty pane says what goes there. With
/// no title, the message is the title.
pub fn art_note(
    drawing: &'static snout_core::art::Drawing,
    title: impl Into<SharedString>,
    text: impl Into<SharedString>,
    cx: &App,
) -> AnyElement {
    let p = palette(cx);
    let (title, text): (SharedString, SharedString) = (title.into(), text.into());
    let state = if title.is_empty() {
        crate::components::EmptyState::new(drawing, text)
    } else {
        crate::components::EmptyState::new(drawing, title).hint(text)
    };
    state.on(p.panel).compact().into_any_element()
}

/// A dimmed message centred in what is left of a pane.
pub fn centered_note(text: impl Into<SharedString>, p: &Palette) -> Div {
    div().size_full().flex().items_center().justify_center().child(weak(text, p))
}

/// A drawn button that opens a GPUI Kit menu: `Trigger(button).dropdown_menu(...)`.
pub struct Trigger(pub Stateful<Div>);

impl gpui::Styled for Trigger {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        self.0.style()
    }
}

impl gpui::InteractiveElement for Trigger {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.0.interactivity()
    }
}

impl gpui_component::Selectable for Trigger {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl IntoElement for Trigger {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.0
    }
}

impl gpui_component::menu::DropdownMenu for Trigger {}

/// A filled triangle, as egui marks a collapsing header: pointing right when closed, down
/// when open.
pub fn disclosure(open: bool, color: Hsla) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let c = b.center();
            let r = px(3.5);
            let mut path = gpui::PathBuilder::fill();
            if open {
                path.move_to(gpui::point(c.x - r, c.y - r * 0.6));
                path.line_to(gpui::point(c.x + r, c.y - r * 0.6));
                path.line_to(gpui::point(c.x, c.y + r * 0.8));
            } else {
                path.move_to(gpui::point(c.x - r * 0.6, c.y - r));
                path.line_to(gpui::point(c.x + r * 0.8, c.y));
                path.line_to(gpui::point(c.x - r * 0.6, c.y + r));
            }
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .flex_none()
    .size(px(12.))
}

/// The open-or-closed chevron of a tree row: › closed, ⌄ open.
pub fn chevron(open: bool, color: Hsla) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let c = b.center();
            let (a, h) = (px(3.5), px(1.75));
            let pts = if open {
                [gpui::point(c.x - a, c.y - h), gpui::point(c.x, c.y + h), gpui::point(c.x + a, c.y - h)]
            } else {
                [gpui::point(c.x - h, c.y - a), gpui::point(c.x + h, c.y), gpui::point(c.x - h, c.y + a)]
            };
            let mut path = gpui::PathBuilder::stroke(px(1.6));
            path.move_to(pts[0]);
            path.line_to(pts[1]);
            path.line_to(pts[2]);
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .flex_none()
    .w(px(14.))
    .h_full()
}

/// egui's checkbox: a small framed box, ticked when on, and its label.
pub fn checkbox(id: impl Into<ElementId>, label: &'static str, on: bool, enabled: bool, p: &Palette) -> Stateful<Div> {
    let ink = if enabled { p.fg } else { p.weak() };
    let tick = p.fg;
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(ink)
        .when(enabled, |d| d.cursor_pointer())
        .child(
            div().size(px(14.)).rounded(px(3.)).bg(p.button).border_1().border_color(mix(p.fg, 0.08, p.button)).when(
                on,
                |d| {
                    d.child(
                        gpui::canvas(
                            |_, _, _| {},
                            move |b, _, window, _| {
                                let o = b.origin;
                                let mut path = gpui::PathBuilder::stroke(px(1.6));
                                path.move_to(gpui::point(o.x + px(2.5), o.y + px(6.5)));
                                path.line_to(gpui::point(o.x + px(5.5), o.y + px(9.5)));
                                path.line_to(gpui::point(o.x + px(10.), o.y + px(3.)));
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, tick);
                                }
                            },
                        )
                        .size_full(),
                    )
                },
            ),
        )
        .child(label)
}

/// The magnifier in front of a search field, in one colour like egui's.
fn magnifier(color: Hsla) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let o = b.origin;
            let at = |x: f32, y: f32| gpui::point(o.x + px(x), o.y + px(y));
            let mut ring = gpui::PathBuilder::stroke(px(1.5));
            ring.move_to(at(10.5, 6.));
            ring.arc_to(gpui::point(px(4.5), px(4.5)), px(0.), false, true, at(1.5, 6.));
            ring.arc_to(gpui::point(px(4.5), px(4.5)), px(0.), false, true, at(10.5, 6.));
            let mut handle = gpui::PathBuilder::stroke(px(1.8));
            handle.move_to(at(9.2, 9.2));
            handle.line_to(at(13., 13.));
            for path in [ring, handle] {
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .flex_none()
    .size(px(14.))
}
