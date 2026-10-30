//! Sheets that resize: Settings, Project Structure and New Project sit over the window as
//! cards, and their edges and corners take the pointer to make them bigger or smaller. The
//! card stays centred, so an edge follows the pointer and its opposite moves the other way.
//! Each sheet's size is kept in the user's settings (`dialog_sizes`, by the sheet's key) and
//! held within the window, so a size kept on a big screen fits a small one. The handles are
//! the app's own elements, not the platform's, so they work the same on every system.

use gpui::{
    App, AppContext as _, Context, CursorStyle, Div, DragMoveEvent, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, Size, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::model::Model;

/// What a resizable sheet is called in the settings, the size it opens at the first time,
/// and the least it may be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetSpec {
    pub key: &'static str,
    pub default: (f32, f32),
    pub min: (f32, f32),
}

/// Settings: the page tree stays 220 points wide; a page's rows need some 560 beside it.
pub const SETTINGS: SheetSpec = SheetSpec { key: "settings", default: (860., 660.), min: (780., 480.) };

/// Project Structure: the page tree, and the modules' and dependencies' tables beside it.
pub const PROJECT_STRUCTURE: SheetSpec =
    SheetSpec { key: "project-structure", default: (1000., 660.), min: (820., 480.) };

/// New Project: the kinds on the left, the form beside them.
pub const NEW_PROJECT: SheetSpec = SheetSpec { key: "new-project", default: (860., 640.), min: (760., 540.) };

/// How far a sheet keeps from the window's edges.
pub const MARGIN: f32 = 24.;

/// The size `want` held between the sheet's least and the window's `viewport` (less the
/// margins). A window smaller than the sheet's least gets the window's size.
pub fn clamp(spec: &SheetSpec, want: (f32, f32), viewport: Size<Pixels>) -> (f32, f32) {
    let max = (f32::from(viewport.width) - 2. * MARGIN, f32::from(viewport.height) - 2. * MARGIN);
    let fit = |v: f32, min: f32, max: f32| v.max(min.min(max)).min(max.max(1.));
    (fit(want.0, spec.min.0, max.0), fit(want.1, spec.min.1, max.1))
}

/// The size the sheet has now: the one kept, or its default, held within the window.
pub fn size_of(spec: &SheetSpec, model: &Entity<Model>, window: &Window, cx: &App) -> (f32, f32) {
    let kept = model.read(cx).settings.dialog_sizes.get(spec.key).map(|[w, h]| (*w, *h));
    clamp(spec, kept.unwrap_or(spec.default), window.viewport_size())
}

/// A handle being dragged: the sheet's key, and which ways it sizes the sheet.
#[derive(Clone, Debug)]
pub struct SheetDrag {
    pub key: &'static str,
    pub width: bool,
    pub height: bool,
}

struct NoGhost;

impl Render for NoGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Keep the sizes on disk once a handle is let go: for the backdrop under a sheet, where
/// the pointer may be when it is.
pub fn persist_on_drop(model: Entity<Model>) -> impl Fn(&SheetDrag, &mut Window, &mut App) + 'static {
    move |_, _, cx| model.update(cx, |m, _| m.persist())
}

/// `content` (a sheet that fills what it is given) at its kept size, with handles on its
/// edges and corners.
pub fn resizable(
    spec: SheetSpec,
    content: impl IntoElement,
    model: &Entity<Model>,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let (w, h) = size_of(&spec, model, window, cx);
    let (sizing, dropping) = (model.clone(), model.clone());
    let key = spec.key;
    let handle = |id: &'static str, width: bool, height: bool, cursor: CursorStyle| {
        div()
            .id(id)
            .debug_selector(move || format!("sheet-resize-{id}"))
            .absolute()
            .cursor(cursor)
            .occlude()
            .on_drag(SheetDrag { key, width, height }, |_, _, _, cx| cx.new(|_| NoGhost))
            .on_click(|_, _, cx| cx.stop_propagation())
    };
    // The edges reach a little out of the card, and the corners a little more, so they are
    // easy to take.
    let (edge, corner) = (px(8.), px(14.));
    div()
        .id(gpui::ElementId::Name(format!("sheet-{key}").into()))
        .debug_selector(move || format!("sheet-{key}"))
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .flex()
        .flex_col()
        .child(content)
        .on_drag_move(move |e: &DragMoveEvent<SheetDrag>, window, cx| {
            let d = e.drag(cx).clone();
            if d.key != key {
                return;
            }
            let c = e.bounds.center();
            let at = e.event.position;
            let (mut w, mut h) = size_of(&spec, &sizing, window, cx);
            if d.width {
                w = 2. * f32::from((at.x - c.x).abs());
            }
            if d.height {
                h = 2. * f32::from((at.y - c.y).abs());
            }
            let (w, h) = clamp(&spec, (w.round(), h.round()), window.viewport_size());
            sizing.update(cx, |m, _| {
                m.settings.dialog_sizes.insert(key.to_owned(), [w, h]);
            });
            window.refresh();
        })
        .on_drop(persist_on_drop(dropping))
        .child(handle("n", false, true, CursorStyle::ResizeUpDown).top(-edge / 2.).left(corner).right(corner).h(edge))
        .child(
            handle("s", false, true, CursorStyle::ResizeUpDown).bottom(-edge / 2.).left(corner).right(corner).h(edge),
        )
        .child(
            handle("w", true, false, CursorStyle::ResizeLeftRight).left(-edge / 2.).top(corner).bottom(corner).w(edge),
        )
        .child(
            handle("e", true, false, CursorStyle::ResizeLeftRight).right(-edge / 2.).top(corner).bottom(corner).w(edge),
        )
        .child(
            handle("nw", true, true, CursorStyle::ResizeUpLeftDownRight).top(-edge / 2.).left(-edge / 2.).size(corner),
        )
        .child(
            handle("ne", true, true, CursorStyle::ResizeUpRightDownLeft).top(-edge / 2.).right(-edge / 2.).size(corner),
        )
        .child(
            handle("sw", true, true, CursorStyle::ResizeUpRightDownLeft)
                .bottom(-edge / 2.)
                .left(-edge / 2.)
                .size(corner),
        )
        .child(
            handle("se", true, true, CursorStyle::ResizeUpLeftDownRight)
                .bottom(-edge / 2.)
                .right(-edge / 2.)
                .size(corner),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_keeps_its_least_and_the_window_holds_it() {
        let big = gpui::size(px(2000.), px(1400.));
        assert_eq!(clamp(&SETTINGS, (500., 300.), big), SETTINGS.min, "never under its least");
        assert_eq!(clamp(&SETTINGS, (1200., 900.), big), (1200., 900.));
        let small = gpui::size(px(1000.), px(700.));
        assert_eq!(clamp(&SETTINGS, (1800., 1200.), small), (952., 652.), "held within the window");
        let tiny = gpui::size(px(600.), px(400.));
        assert_eq!(clamp(&SETTINGS, SETTINGS.default, tiny), (552., 352.), "a tiny window wins over the least");
    }
}
