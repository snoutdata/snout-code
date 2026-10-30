//! The shared components every view builds with, in the house look of Snout Studio: buttons
//! (primary, secondary, danger, ghost) with a leading icon and a key hint, icon buttons with
//! their tooltip, the dialog's header with its spot drawing or icon badge, its body and
//! footer, file chips, badges, callouts and the empty state of a panel.
//!
//! Icons are Lucide's (ISC, bundled by GPUI Kit's assets; see NOTICE), or the window's own
//! line icons of [`crate::glyphs`]. Every colour comes from the palette.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, SharedString, Stateful, StatefulInteractiveElement as _, Styled, Window, div, point, px,
};
use gpui_component::tooltip::Tooltip;
pub use gpui_kit_assets::IconName;
use snout_core::art::Drawing;

use crate::theme::{Palette, mix, palette};

mod list;
mod menu;
pub mod sheet;
pub use list::*;
pub use menu::*;

// ----- Icons ------------------------------------------------------------------------------

/// An icon: one of Lucide's, or one of the window's own line icons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ico {
    Lucide(IconName),
    Line(crate::glyphs::Icon),
}

impl From<IconName> for Ico {
    fn from(name: IconName) -> Self {
        Ico::Lucide(name)
    }
}

impl From<crate::glyphs::Icon> for Ico {
    fn from(icon: crate::glyphs::Icon) -> Self {
        Ico::Line(icon)
    }
}

/// `ico` as an element `size` square, in `color`.
pub fn icon(ico: impl Into<Ico>, size: f32, color: Hsla) -> AnyElement {
    match ico.into() {
        Ico::Lucide(name) => {
            gpui_component::Icon::new(name).size(px(size)).flex_none().text_color(color).into_any_element()
        }
        Ico::Line(g) => crate::glyphs::icon(g, size, color).into_any_element(),
    }
}

/// A Lucide icon for a menu item: `PopupMenuItem::new(..).icon(menu_icon(IconName::Copy))`.
pub fn menu_icon(name: IconName) -> gpui_component::Icon {
    gpui_component::Icon::new(name)
}

fn tip(text: SharedString) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    move |w, cx| Tooltip::new(text.clone()).build(w, cx)
}

// ----- Tones ------------------------------------------------------------------------------

/// What a badge, a callout or a dialog is about, for its colour and its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Info,
    Success,
    Warning,
    Danger,
    Git,
    Claude,
}

impl Tone {
    /// The tone's colour, readable as text on its wash and on cards (WCAG AA).
    pub fn color(self, p: &Palette) -> Hsla {
        match self {
            Tone::Neutral => p.fg_dim,
            Tone::Accent => p.accent,
            Tone::Info => p.info(),
            Tone::Success => p.ok,
            Tone::Warning => p.warn,
            Tone::Danger => p.error,
            Tone::Git => p.branch(),
            Tone::Claude => p.claude_ink(),
        }
    }

    /// The tone's icon.
    pub fn icon(self) -> IconName {
        match self {
            Tone::Neutral | Tone::Info | Tone::Accent => IconName::Info,
            Tone::Success => IconName::CircleCheck,
            Tone::Warning => IconName::TriangleAlert,
            Tone::Danger => IconName::OctagonAlert,
            Tone::Git => IconName::GitBranch,
            Tone::Claude => IconName::Sparkles,
        }
    }

    /// The spot drawing a dialog of this tone shows when it has none of its own.
    pub fn art(self) -> &'static Drawing {
        use snout_core::art::spots;
        match self {
            Tone::Neutral | Tone::Info | Tone::Accent => spots::info(),
            Tone::Success => spots::success(),
            Tone::Warning => spots::warning(),
            Tone::Danger => spots::discard(),
            Tone::Git => spots::new_branch(),
            Tone::Claude => spots::claude_review(),
        }
    }
}

// ----- Key hints --------------------------------------------------------------------------

/// A key hint: `⏎`, `Esc`, `⌘⌫`, in a small keycap of `ink`.
pub fn kbd(keys: impl Into<SharedString>, ink: Hsla) -> Div {
    div()
        .flex_none()
        .h(px(17.))
        .min_w(px(17.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(ink.opacity(0.35))
        .text_color(ink)
        .text_size(px(10.5))
        .line_height(px(15.))
        .font_weight(FontWeight::MEDIUM)
        .child(keys.into())
}

// ----- Buttons ----------------------------------------------------------------------------

/// The icons of the buttons every dialog and sheet has, the same everywhere: Cancel and
/// Close dismiss, Apply keeps the sheet open, OK applies and closes.
pub const CANCEL_ICON: IconName = IconName::X;
pub const CLOSE_ICON: IconName = IconName::X;
pub const APPLY_ICON: IconName = IconName::Check;
pub const OK_ICON: IconName = IconName::CheckCheck;

/// How a [`Button`] stands out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    /// The one main action: the accent, solid.
    Primary,
    /// Everything else: framed, quiet.
    #[default]
    Secondary,
    /// An action that destroys something: solid red, clearly enabled.
    Danger,
    /// No frame until the pointer is on it: toolbars, links.
    Ghost,
}

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A button: its leading icon, its label and a key hint, in one of four [`Variant`]s, with
/// hover, pressed, keyboard focus (when [`Button::focusable`]) and disabled states. Its id
/// is also its debug selector. Every button has an icon: [`Button::new`] takes it, so a
/// button without one does not build.
#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    label: Option<SharedString>,
    icon: Ico,
    kbd: Option<SharedString>,
    variant: Variant,
    disabled: bool,
    compact: bool,
    focusable: bool,
    wide: bool,
    tooltip: Option<SharedString>,
    selector: Option<&'static str>,
    on_click: Option<Handler>,
}

impl Button {
    pub fn new(id: impl Into<SharedString>, icon: impl Into<Ico>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: icon.into(),
            kbd: None,
            variant: Variant::Secondary,
            disabled: false,
            compact: false,
            focusable: false,
            wide: false,
            tooltip: None,
            selector: None,
            on_click: None,
        }
    }

    /// The name the tests find the button by (`debug_bounds`).
    pub fn debug_selector(mut self, name: &'static str) -> Self {
        self.selector = Some(name);
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Another icon than the one it was made with.
    pub fn icon(mut self, icon: impl Into<Ico>) -> Self {
        self.icon = icon.into();
        self
    }

    /// The keys that do the same, shown after the label: `⏎`, `Esc`, `⌘⌫`.
    pub fn kbd(mut self, keys: impl Into<SharedString>) -> Self {
        self.kbd = Some(keys.into());
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(Variant::Primary)
    }

    pub fn danger(self) -> Self {
        self.variant(Variant::Danger)
    }

    pub fn ghost(self) -> Self {
        self.variant(Variant::Ghost)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The smaller size, for rows and pane headers.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Reachable with Tab, its ring showing while the keyboard is on it. For dialogs:
    /// a toolbar's buttons leave the keyboard where it is.
    pub fn focusable(mut self) -> Self {
        self.focusable = true;
        self
    }

    /// As wide as its container.
    pub fn wide(mut self) -> Self {
        self.wide = true;
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

/// The colours of a button: ground, under the pointer, pressed, label, frame.
fn button_colors(variant: Variant, disabled: bool, p: &Palette) -> (Hsla, Hsla, Hsla, Hsla, Hsla) {
    let clear = gpui::transparent_black();
    if disabled {
        let ground = if variant == Variant::Ghost { clear } else { p.button };
        let ink = p.fg_dim.opacity(0.75);
        return (ground, ground, ground, ink, if variant == Variant::Ghost { clear } else { p.border });
    }
    match variant {
        Variant::Primary => {
            let b = p.primary();
            (b.fill, b.hover, b.pressed, b.ink, b.fill)
        }
        Variant::Danger => {
            let b = p.danger();
            (b.fill, b.hover, b.pressed, b.ink, b.fill)
        }
        Variant::Secondary => (p.button, p.button_hover, p.active, p.fg, mix(p.fg, 0.14, p.button)),
        Variant::Ghost => (clear, p.button_hover, p.active, p.fg, clear),
    }
}

/// A button in `variant` with its icon, as a `Div` to add the label, a key hint and
/// handlers to: what [`Button`] draws, for code that wants the element itself. `icon_only`
/// makes it square.
#[allow(clippy::too_many_arguments)]
pub fn button_base(
    id: impl Into<SharedString>,
    variant: Variant,
    ico: impl Into<Ico>,
    disabled: bool,
    compact: bool,
    focusable: bool,
    icon_only: bool,
    p: &Palette,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let (ground, hover, pressed, ink, frame) = button_colors(variant, disabled, p);
    let enabled = !disabled;
    let solid = matches!(variant, Variant::Primary | Variant::Danger) && enabled;
    let ring = p.focus_ring();
    let selector = id.clone();
    let (h, pad) = if compact { (24., 8.) } else { (30., 12.) };
    let dark = p.dark;
    let glow = p.glow_of(p.accent);
    let ico = ico.into();
    div()
        .id(ElementId::Name(id))
        .debug_selector(move || selector.to_string())
        .flex_none()
        .h(px(h))
        .when(!icon_only, |d| d.px(px(pad)).min_w(px(if compact { 0. } else { 72. })))
        .when(icon_only, |d| d.w(px(h)))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(7.))
        .border_1()
        .border_color(frame)
        .bg(ground)
        .text_color(ink)
        .text_size(px(if compact { 12. } else { 13. }))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .when(solid, |d| {
            d.shadow(vec![BoxShadow {
                color: ground.opacity(if dark { 0.35 } else { 0.25 }),
                offset: point(px(0.), px(1.)),
                blur_radius: px(3.),
                spread_radius: px(0.),
                inset: false,
            }])
        })
        .when(variant == Variant::Primary && enabled && !glow.is_empty(), |d| d.shadow(glow))
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)).active(move |s| s.bg(pressed)))
        .when(focusable && enabled, |d| {
            d.tab_index(0).focus_visible(move |s| {
                s.border_color(ring).shadow(vec![BoxShadow {
                    color: ring.opacity(0.55),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(2.),
                    inset: false,
                }])
            })
        })
        .child(icon(ico, if compact { 13. } else { 14. }, ink))
}

/// A dialog's or a sheet's button with its icon and label, as an element to add a handler
/// to: what [`Button`] draws.
pub fn footer_button(
    id: impl Into<SharedString>,
    ico: impl Into<Ico>,
    label: impl Into<SharedString>,
    primary: bool,
    enabled: bool,
    p: &Palette,
) -> Stateful<Div> {
    let variant = if primary { Variant::Primary } else { Variant::Secondary };
    button_base(id, variant, ico, !enabled, false, false, false, p).child(label.into())
}

/// The label colour of a button in `variant`, for its icon.
pub fn button_ink(variant: Variant, disabled: bool, p: &Palette) -> Hsla {
    button_colors(variant, disabled, p).3
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let ink = button_ink(self.variant, self.disabled, &p);
        let solid = matches!(self.variant, Variant::Primary | Variant::Danger) && !self.disabled;
        let icon_only = self.label.is_none();
        button_base(self.id, self.variant, self.icon, self.disabled, self.compact, self.focusable, icon_only, &p)
            .when(self.wide, |d| d.w_full())
            .when_some(self.selector, |d, s| d.debug_selector(move || s.to_owned()))
            .when_some(self.tooltip, |d, t| d.tooltip(tip(t)))
            .when_some(self.label, |d, l| d.child(l))
            .when_some(self.kbd, |d, k| {
                let key_ink = if solid { ink.opacity(0.85) } else { p.fg_dim };
                d.child(div().ml(px(2.)).child(kbd(k, key_ink)))
            })
            .when_some(self.on_click.filter(|_| !self.disabled), |d, f| d.on_click(move |e, w, cx| f(e, w, cx)))
    }
}

/// A square icon button with its tooltip: toolbars and pane headers.
#[derive(IntoElement)]
pub struct IconButton {
    id: SharedString,
    icon: Ico,
    tooltip: SharedString,
    size: f32,
    disabled: bool,
    selected: bool,
    tone: Option<Tone>,
    on_click: Option<Handler>,
}

impl IconButton {
    pub fn new(id: impl Into<SharedString>, icon: impl Into<Ico>, tooltip: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            tooltip: tooltip.into(),
            size: 24.,
            disabled: false,
            selected: false,
            tone: None,
            on_click: None,
        }
    }

    /// Its side, in points (24 by default).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// On, like a toggle that is pressed in.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The icon in a tone's colour (the Run button's green, Stop's red).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let enabled = !self.disabled;
        let ink = match (enabled, self.tone) {
            (false, _) => p.fg_dim.opacity(0.5),
            (true, Some(t)) => t.color(&p),
            (true, None) if self.selected => p.fg,
            (true, None) => mix(p.fg, 0.7, p.fg_dim),
        };
        let (hover, pressed) = (p.button_hover, p.active);
        let selector = self.id.clone();
        div()
            .id(ElementId::Name(self.id.clone()))
            .debug_selector(move || selector.to_string())
            .flex_none()
            .size(px(self.size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .when(self.selected, |d| d.bg(p.selection()))
            .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)).active(move |s| s.bg(pressed)))
            .tooltip(tip(self.tooltip))
            .child(icon(self.icon, (self.size * 0.62).round(), ink))
            .when_some(self.on_click.filter(|_| enabled), |d, f| d.on_click(move |e, w, cx| f(e, w, cx)))
    }
}

// ----- Badges, chips and callouts -----------------------------------------------------------

fn pill(tone: Tone, p: &Palette) -> Div {
    let color = tone.color(p);
    div()
        .flex_none()
        .h(px(19.))
        .px(px(7.))
        .flex()
        .items_center()
        .gap(px(4.))
        .rounded(px(10.))
        .border_1()
        .border_color(mix(color, 0.35, p.card))
        .bg(p.wash(color))
        .text_color(color)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
}

/// A small pill of text in a soft wash of its tone: a state, a count, a ref.
pub fn badge(text: impl Into<SharedString>, tone: Tone, p: &Palette) -> Div {
    pill(tone, p).child(text.into())
}

/// A badge with an icon in front.
pub fn badge_with_icon(text: impl Into<SharedString>, ico: impl Into<Ico>, tone: Tone, p: &Palette) -> Div {
    pill(tone, p).child(icon(ico, 11., tone.color(p))).child(text.into())
}

/// A note in a wash of its tone with the tone's icon: what to watch out for (warning), what
/// cannot be taken back (danger), what is good to know (info).
pub fn callout(tone: Tone, text: impl Into<SharedString>, p: &Palette) -> Div {
    callout_with(tone, None, text, p)
}

/// A [`callout`] with a title over its text.
pub fn callout_with(tone: Tone, title: Option<SharedString>, text: impl Into<SharedString>, p: &Palette) -> Div {
    let color = tone.color(p);
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(12.))
        .py(px(9.))
        .rounded(px(8.))
        .bg(mix(color, 0.14, p.card))
        .border_1()
        .border_color(mix(color, 0.32, p.card))
        .child(div().pt(px(1.)).child(icon(tone.icon(), 16., color)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .text_color(p.fg)
                .text_size(px(12.5))
                .line_height(px(18.))
                .when_some(title, |d, t| d.child(div().font_weight(FontWeight::SEMIBOLD).child(t)))
                .child(text.into()),
        )
}

/// `path` shortened in its middle to about `max` characters, its file name kept whole while
/// it fits: `src/main/…/internal/InvoiceService.java`. Whole folders go first.
pub fn middle_ellipsis(path: &str, max: usize) -> String {
    let count = path.chars().count();
    if count <= max {
        return path.to_owned();
    }
    let (dir, name) = match path.rfind('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => ("", path),
    };
    let name_len = name.chars().count();
    if dir.is_empty() || name_len + 4 >= max {
        // Only the name: cut its middle.
        let keep = max.saturating_sub(1);
        let head: String = path.chars().take(keep.div_ceil(2)).collect();
        let tail: String = path.chars().skip(count - keep / 2).collect();
        return format!("{head}…{tail}");
    }
    let budget = max - name_len - 2;
    let parts: Vec<&str> = dir.split('/').collect();
    let mut head = Vec::new();
    let mut tail: Vec<&str> = Vec::new();
    let mut used = 0;
    let (mut i, mut j) = (0, parts.len());
    // Alternate: the first folder, then the last, then the next ones in.
    loop {
        let mut took = false;
        if i < j && used + parts[i].len() < budget {
            used += parts[i].len() + 1;
            head.push(parts[i]);
            i += 1;
            took = true;
        }
        if i < j && used + parts[j - 1].len() < budget {
            used += parts[j - 1].len() + 1;
            tail.insert(0, parts[j - 1]);
            j -= 1;
            took = true;
        }
        if !took || i >= j {
            break;
        }
    }
    if i >= j {
        return path.to_owned();
    }
    let mut out = String::new();
    for h in head {
        out.push_str(h);
        out.push('/');
    }
    out.push('…');
    for t in tail {
        out.push('/');
        out.push_str(t);
    }
    out.push('/');
    out.push_str(name);
    out
}

/// A file as a chip: its type's icon, its folder (dimmed, shortened in the middle) and its
/// name, and a badge of its state; the full path in the tooltip.
#[derive(IntoElement)]
pub struct FileChip {
    id: ElementId,
    path: String,
    status: Option<(SharedString, Hsla)>,
    max: usize,
}

impl FileChip {
    /// `path` is the file's, `/`-separated.
    pub fn new(id: impl Into<ElementId>, path: impl Into<String>) -> Self {
        Self { id: id.into(), path: path.into(), status: None, max: 52 }
    }

    /// Its state, as a badge in `color`: Modified, Untracked, Staged.
    pub fn status(mut self, label: impl Into<SharedString>, color: Hsla) -> Self {
        self.status = Some((label.into(), color));
        self
    }

    /// About how many characters the path may take.
    pub fn max_chars(mut self, max: usize) -> Self {
        self.max = max;
        self
    }
}

impl RenderOnce for FileChip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let short = middle_ellipsis(&self.path, self.max);
        let (dir, name) = match short.rfind('/') {
            Some(i) => (short[..=i].to_owned(), short[i + 1..].to_owned()),
            None => (String::new(), short.clone()),
        };
        let full: SharedString = self.path.clone().into();
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(34.))
            .px(px(10.))
            .rounded(px(8.))
            .bg(p.input)
            .border_1()
            .border_color(p.border)
            .tooltip(tip(full))
            .child(crate::icons::file_icon(std::path::Path::new(&self.path), 16., &p))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(12.5))
                    .child(div().flex_none().text_color(p.fg_dim).child(dir))
                    .child(div().flex_none().text_color(p.fg).font_weight(FontWeight::MEDIUM).child(name)),
            )
            .when_some(self.status, |d, (label, color)| {
                d.child(
                    div()
                        .flex_none()
                        .h(px(19.))
                        .px(px(7.))
                        .flex()
                        .items_center()
                        .rounded(px(10.))
                        .bg(p.wash(color))
                        .border_1()
                        .border_color(mix(color, 0.35, p.card))
                        .text_color(color)
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
            })
    }
}

// ----- Empty states -----------------------------------------------------------------------

/// What an empty panel shows: its spot drawing, a title, a hint, and what to do about it.
#[derive(IntoElement)]
pub struct EmptyState {
    art: &'static Drawing,
    title: SharedString,
    hint: Option<SharedString>,
    actions: Vec<AnyElement>,
    compact: bool,
    surface: Option<Hsla>,
}

impl EmptyState {
    pub fn new(art: &'static Drawing, title: impl Into<SharedString>) -> Self {
        Self { art, title: title.into(), hint: None, actions: Vec::new(), compact: false, surface: None }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        let hint = hint.into();
        self.hint = (!hint.is_empty()).then_some(hint);
        self
    }

    /// A button under the hint (a [`Button`], usually).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// The main thing to do about it: a normal-sized primary button with its icon. The
    /// first action of an empty state is this one.
    pub fn primary(
        self,
        id: &'static str,
        ico: impl Into<Ico>,
        label: impl Into<SharedString>,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action(Button::new(id, ico).primary().label(label).on_click(f))
    }

    /// Another thing to do about it, beside the primary one: a normal-sized framed button.
    pub fn secondary(
        self,
        id: &'static str,
        ico: impl Into<Ico>,
        label: impl Into<SharedString>,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action(Button::new(id, ico).label(label).on_click(f))
    }

    /// The primary action as [`EmptyState::primary`] makes it, dispatching `action`.
    pub fn primary_action(
        self,
        id: &'static str,
        ico: impl Into<Ico>,
        label: impl Into<SharedString>,
        action: impl gpui::Action + Clone,
    ) -> Self {
        self.primary(id, ico, label, move |_, w, cx| w.dispatch_action(action.boxed_clone(), cx))
    }

    /// What it sits on, for the drawing's blob (the tool windows' ground by default).
    pub fn on(mut self, surface: Hsla) -> Self {
        self.surface = Some(surface);
        self
    }

    /// A smaller drawing, for narrow tool windows and short panes.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let w = if self.compact { 132. } else { 176. };
        let ratio = self.art.size.x / self.art.size.y;
        div()
            .debug_selector(|| "empty-state".into())
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(px(24.))
            .py(px(12.))
            .gap(px(4.))
            .child(div().debug_selector(|| "empty-state-art".into()).child(crate::art::drawing_on(
                self.art,
                w,
                w / ratio,
                self.surface.unwrap_or(p.ground),
            )))
            .child(
                div()
                    .mt(px(6.))
                    .text_color(p.fg)
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_center()
                    .child(self.title),
            )
            .when_some(self.hint, |d, h| {
                d.child(
                    div()
                        .max_w(px(360.))
                        .text_color(p.fg_dim)
                        .text_size(px(12.5))
                        .line_height(px(18.))
                        .text_center()
                        .child(h),
                )
            })
            .when(!self.actions.is_empty(), |d| {
                d.child(div().mt(px(10.)).flex().flex_wrap().justify_center().gap(px(8.)).children(self.actions))
            })
    }
}

// ----- Pages ------------------------------------------------------------------------------

/// The head of a page of a sheet (Settings, Project Structure): its spot drawing, its title
/// and what the page is for, and the page's own controls on the right.
pub fn page_header(
    art: &'static Drawing,
    title: impl Into<SharedString>,
    blurb: impl Into<SharedString>,
    controls: impl IntoElement,
    p: &Palette,
) -> Div {
    div()
        .flex_none()
        .px(px(20.))
        .pt(px(12.))
        .pb(px(6.))
        .flex()
        .items_center()
        .gap(px(14.))
        .child(crate::art::drawing_on(art, 114., 76., p.card))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(p.fg).child(title.into()))
                .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(p.fg_dim).child(blurb.into())),
        )
        .child(div().flex_none().flex().flex_col().items_end().gap(px(6.)).child(controls))
}

/// A card of tips about a page: a light bulb and its title, then each tip with its icon.
pub fn tips(title: impl Into<SharedString>, items: &[(IconName, &'static str)], p: &Palette) -> Div {
    let color = p.accent;
    div()
        .mt(px(16.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(10.))
        .bg(mix(color, if p.dark { 0.08 } else { 0.06 }, p.card))
        .border_1()
        .border_color(mix(color, 0.22, p.card))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.fg)
                .child(icon(IconName::Lightbulb, 15., color))
                .child(title.into()),
        )
        .children(items.iter().map(|(ico, text)| {
            div()
                .flex()
                .items_start()
                .gap(px(9.))
                .text_size(px(12.5))
                .line_height(px(18.))
                .text_color(p.fg)
                .child(
                    div()
                        .flex_none()
                        .mt(px(1.))
                        .size(px(20.))
                        .rounded(px(6.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(p.wash(color))
                        .child(icon(*ico, 12., color)),
                )
                .child(div().flex_1().min_w_0().pt(px(1.)).child(*text))
        }))
}

// ----- Dialogs ----------------------------------------------------------------------------

/// A dialog's header: its spot drawing with the tone's badge on its corner, the title, and
/// a line under it saying what the dialog is for.
pub fn dialog_header(
    tone: Tone,
    art: Option<&'static Drawing>,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
    p: &Palette,
) -> Div {
    header(tone, art, None, title.into(), subtitle, p)
}

/// A dialog's header with no drawing: `ico` in a tile of the tone's wash, then the title.
pub fn dialog_header_icon(
    tone: Tone,
    ico: impl Into<Ico>,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
    p: &Palette,
) -> Div {
    header(tone, None, Some(ico.into()), title.into(), subtitle, p)
}

fn header(
    tone: Tone,
    art: Option<&'static Drawing>,
    ico: Option<Ico>,
    title: SharedString,
    subtitle: Option<SharedString>,
    p: &Palette,
) -> Div {
    let color = tone.color(p);
    let ico = ico.unwrap_or(Ico::Lucide(tone.icon()));
    let badge_bg = p.badge(color);
    let picture = match art {
        Some(art) => div()
            .flex_none()
            .relative()
            .w(px(108.))
            .h(px(72.))
            .child(crate::art::drawing_on(art, 108., 72., p.card))
            .when(tone != Tone::Neutral, |d| {
                d.child(
                    div()
                        .absolute()
                        .right(px(2.))
                        .bottom(px(0.))
                        .size(px(26.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(badge_bg)
                        .border_2()
                        .border_color(p.card)
                        .child(icon(tone.icon(), 13., p.on_badge)),
                )
            }),
        None => div()
            .flex_none()
            .size(px(36.))
            .rounded(px(10.))
            .flex()
            .items_center()
            .justify_center()
            .bg(p.wash(color))
            .border_1()
            .border_color(mix(color, 0.3, p.card))
            .child(icon(ico, 18., color)),
    };
    div().flex_none().flex().items_center().gap(px(16.)).px(px(20.)).pt(px(18.)).pb(px(10.)).child(picture).child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(p.fg).child(title))
            .when_some(subtitle, |d, s| {
                d.child(div().text_size(px(13.)).line_height(px(19.)).text_color(p.fg_dim).child(s))
            }),
    )
}

/// A dialog's body: its content in a column with room around it.
pub fn dialog_body() -> Div {
    div().flex().flex_col().gap(px(12.)).px(px(20.)).pt(px(4.)).pb(px(16.))
}

/// A dialog's footer: a status line on the left (or anything else), its buttons on the
/// right, on a strip of its own.
pub fn dialog_footer(status: Option<(String, Hsla)>, buttons: Vec<AnyElement>, p: &Palette) -> Div {
    div()
        .flex_none()
        .min_h(px(56.))
        .px(px(20.))
        .py(px(12.))
        .flex()
        .items_center()
        .gap(px(8.))
        .border_t_1()
        .border_color(p.border)
        .bg(p.dialog_footer())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .children(status.map(|(t, c)| div().text_color(c).child(t))),
        )
        .children(buttons)
}

/// A dialog's card, drawn by the app (the overlays: Connect to SSH Host, New Project,
/// Settings): its header, then the caller's body and [`dialog_footer`].
#[allow(clippy::too_many_arguments)]
pub fn dialog_frame(
    id: &'static str,
    tone: Tone,
    art: Option<&'static Drawing>,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
    w: f32,
    h: Option<f32>,
    cx: &App,
) -> Stateful<Div> {
    let p = palette(cx);
    card(id, w, h, &p).child(dialog_header(tone, art, title, subtitle, &p))
}

/// A dialog's card as [`dialog_frame`] draws it, with an icon in its header in place of a
/// drawing: for tall dialogs (pickers, wizards, Settings).
#[allow(clippy::too_many_arguments)]
pub fn dialog_frame_icon(
    id: &'static str,
    tone: Tone,
    ico: impl Into<Ico>,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
    w: f32,
    h: Option<f32>,
    cx: &App,
) -> Stateful<Div> {
    let p = palette(cx);
    card(id, w, h, &p)
        .child(dialog_header_icon(tone, ico, title, subtitle, &p).pb(px(12.)).border_b_1().border_color(p.border))
}

fn card(id: &'static str, w: f32, h: Option<f32>, p: &Palette) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .occlude()
        .w(px(w))
        .when_some(h, |d, h| d.h(px(h)))
        .flex()
        .flex_col()
        .bg(p.card)
        .border_1()
        .border_color(p.border)
        .rounded(px(12.))
        .shadow_lg()
        .overflow_hidden()
        .text_size(px(crate::look::BODY))
        .text_color(p.fg)
}

/// The label over a field of a form.
pub fn field_label(text: impl Into<SharedString>, p: &Palette) -> Div {
    div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(p.fg_dim).child(text.into())
}

/// A choice in a dialog as a card: an icon in a tile, a title and what it does. The caller
/// adds `on_click`.
pub fn choice_card(
    id: impl Into<ElementId>,
    ico: impl Into<Ico>,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    p: &Palette,
) -> Stateful<Div> {
    let (hover, edge) = (p.button_hover, mix(p.accent, 0.5, p.border));
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(10.))
        .border_1()
        .border_color(p.border)
        .bg(p.card)
        .cursor_pointer()
        .hover(move |s| s.bg(hover).border_color(edge))
        .child(
            div()
                .flex_none()
                .size(px(34.))
                .rounded(px(9.))
                .flex()
                .items_center()
                .justify_center()
                .bg(p.wash(p.accent))
                .child(icon(ico, 17., p.accent)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title.into()))
                .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(p.fg_dim).child(detail.into())),
        )
        .child(icon(IconName::ChevronRight, 15., p.fg_dim))
}

// ----- The gallery ------------------------------------------------------------------------

/// Every component and spot drawing on one page, for the screenshots.
pub struct Gallery;

impl gpui::Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let row = || div().flex().items_center().gap(px(10.));
        let arts = snout_core::art::spots::all();
        div()
            .size_full()
            .bg(p.panel)
            .text_color(p.fg)
            .font_family(crate::look::ui_font(cx))
            .text_size(px(crate::look::BODY))
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                row()
                    .child(Button::new("g-primary", IconName::Check).primary().label("Commit").kbd("⏎"))
                    .child(Button::new("g-secondary", CANCEL_ICON).label("Cancel").kbd("Esc"))
                    .child(
                        Button::new("g-danger", IconName::Trash)
                            .danger()
                            .label("Discard")
                            .kbd(crate::keymap::shown_keys("⌘⌫")),
                    )
                    .child(Button::new("g-ghost", IconName::RefreshCw).ghost().label("Refresh"))
                    .child(Button::new("g-disabled", IconName::Plus).primary().label("Create").disabled(true))
                    .child(Button::new("g-compact", IconName::Copy).compact().label("Copy"))
                    .child(IconButton::new("g-icon", IconName::Settings, "Settings"))
                    .child(IconButton::new("g-icon-on", IconName::Search, "Search").selected(true))
                    .child(IconButton::new("g-icon-run", IconName::Play, "Run").tone(Tone::Success)),
            )
            .child(
                row()
                    .child(badge("Modified", Tone::Warning, &p))
                    .child(badge("Staged", Tone::Success, &p))
                    .child(badge("main", Tone::Git, &p))
                    .child(badge_with_icon("Claude", IconName::Sparkles, Tone::Claude, &p))
                    .child(badge("3 errors", Tone::Danger, &p))
                    .child(badge("Info", Tone::Info, &p))
                    .child(
                        div().w(px(420.)).child(
                            FileChip::new(
                                "g-chip",
                                "src/main/java/com/example/billing/service/internal/InvoiceReconciliationService.java",
                            )
                            .status("Modified", p.warn),
                        ),
                    ),
            )
            .child(gallery_table(cx))
            .child(
                row()
                    .child(div().w(px(380.)).child(callout(Tone::Danger, "This cannot be undone.", &p)))
                    .child(div().w(px(380.)).child(callout(Tone::Warning, "This leaves HEAD detached.", &p)))
                    .child(div().w(px(380.)).child(callout(
                        Tone::Info,
                        "SnoutCode installs its server on the host.",
                        &p,
                    ))),
            )
            .child(div().flex().flex_wrap().gap(px(8.)).children(arts.into_iter().map(|(name, art)| {
                div()
                    .w(px(140.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .rounded(px(8.))
                    .bg(p.card)
                    .border_1()
                    .border_color(p.border)
                    .p(px(4.))
                    .child(crate::art::drawing(art, 132., 88., None))
                    .child(div().text_size(px(11.)).text_color(p.fg_dim).child(name))
            })))
    }
}

/// A table of changed files for the gallery: the header, a selection with the keyboard on
/// it, one without, the pointer's wash, every state's pill.
fn gallery_table(cx: &mut gpui::Context<Gallery>) -> impl IntoElement {
    let p = palette(cx);
    let columns = TableColumns::new(
        vec![
            Column::new("Name", Some(220.)),
            Column::new("State", Some(190.)),
            Column::new("Relative Directory", None),
        ],
        cx,
    );
    let widths = columns.read(cx).widths();
    let files: [(&str, &[StatePill], &str, RowState); 7] = [
        (
            "Application.java",
            &[StatePill::Modified],
            "scala/bamboo-web/src/main/java/com/solarpanda/bamboo",
            RowState::new(true, true),
        ),
        ("pom.xml", &[StatePill::Added, StatePill::Staged], "scala/bamboo-web", RowState::default()),
        (
            "LegacyMapper.java",
            &[StatePill::Deleted],
            "scala/bamboo-core/src/main/java/com/solarpanda/legacy",
            RowState::default(),
        ),
        ("Invoice.java", &[StatePill::Renamed, StatePill::PartlyStaged], "billing/src/main/java", RowState::default()),
        ("NOTES.md", &[StatePill::Untracked], "", RowState::default()),
        ("build.gradle.kts", &[StatePill::Conflicted], "", RowState::default()),
        ("Gradle.java", &[StatePill::Modified], "src/main/java/demo", RowState::new(true, false)),
    ];
    let rows = files.iter().enumerate().map(|(i, (name, states, dir, state))| {
        let name_cell = div()
            .flex()
            .items_center()
            .gap(px(7.))
            .child(crate::icons::file_icon(std::path::Path::new(name), 16., &p))
            .child(div().text_color(p.fg).child(*name));
        table_row(
            ("g-row", i),
            *state,
            &widths,
            vec![
                name_cell.into_any_element(),
                state_pills(states, &p).into_any_element(),
                path_text(("g-dir", i), *dir, p.fg_dim).into_any_element(),
            ],
            &p,
        )
        .into_any_element()
    });
    div()
        .w(px(700.))
        .h(px(250.))
        .flex()
        .flex_col()
        .p(px(8.))
        .rounded(px(8.))
        .bg(p.panel)
        .border_1()
        .border_color(p.border)
        .child(table(
            table_header("g-table", &columns, Some((0, true)), None, cx),
            div().flex().flex_col().children(rows),
        ))
}

#[cfg(test)]
mod tests {
    use super::middle_ellipsis;

    /// Every `.rs` file under `dir`, with its text.
    fn sources(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }

    /// The app's own buttons ([`super::Button`], [`super::footer_button`], `look`'s) take
    /// their icon to be built at all. GPUI Kit's buttons do not: each one the app makes must
    /// set one before its menu or its handler. And no dialog or sheet spells its Cancel,
    /// Apply or OK out as a bare label, outside a button.
    #[test]
    fn every_button_has_an_icon() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&src, &mut files);
        let mut bare = Vec::new();
        for (path, text) in &files {
            let mut from = 0;
            while let Some(at) = text[from..].find("Button::new(").map(|i| i + from) {
                from = at + 1;
                if text[..at].ends_with("Icon") || text[..at].ends_with('"') {
                    continue;
                }
                // The app's Button::new takes the id and the icon; GPUI Kit's the id alone.
                let rest = &text[at + "Button::new(".len()..];
                let (mut depth, mut args, mut close) = (0, 1, rest.len());
                for (i, c) in rest.char_indices() {
                    match c {
                        '(' | '[' | '{' => depth += 1,
                        ')' | ']' | '}' if depth == 0 => {
                            close = i;
                            break;
                        }
                        ')' | ']' | '}' => depth -= 1,
                        ',' if depth == 0 => args += 1,
                        _ => {}
                    }
                }
                if args > 1 {
                    continue;
                }
                let chain = &rest[close..];
                let end = ["dropdown_menu(", ".on_click(", ";"]
                    .iter()
                    .filter_map(|m| chain.find(m))
                    .min()
                    .unwrap_or(chain.len());
                if !chain[..end].contains(".icon(") {
                    let line = text[..at].lines().count();
                    bare.push(format!("{}:{line}", path.display()));
                }
            }
            for label in ["Cancel", "Apply", "OK"] {
                if text.contains(&format!(".child(\"{label}\")")) {
                    bare.push(format!("{}: a bare \"{label}\"", path.display()));
                }
            }
        }
        assert!(bare.is_empty(), "buttons without an icon: {bare:#?}");
    }

    #[test]
    fn a_long_path_keeps_its_ends_and_its_name() {
        let path = "src/main/java/com/example/billing/service/internal/InvoiceReconciliationService.java";
        let short = middle_ellipsis(path, 60);
        assert!(short.starts_with("src/"), "{short}");
        assert!(short.ends_with("/internal/InvoiceReconciliationService.java"), "{short}");
        assert!(short.contains('…'), "{short}");
        assert!(short.chars().count() <= 60, "{short}");
        assert_eq!(middle_ellipsis("src/main.rs", 60), "src/main.rs");
        let name = middle_ellipsis("AVeryLongFileNameThatGoesOnAndOnForeverAndEver.java", 20);
        assert_eq!(name.chars().count(), 20, "{name}");
        assert!(name.starts_with("AVery") && name.ends_with(".java"), "{name}");
    }
}
