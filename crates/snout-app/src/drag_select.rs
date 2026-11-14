//! Selecting text with the mouse in a panel of its own (a console, a diff): the drag goes on
//! when the mouse leaves the panel, and at or past an edge the panel scrolls that way, as in
//! the editor, so text out of view can be selected too.

use gpui::{
    Context, DispatchPhase, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle,
    Styled as _, Task, WeakEntity, canvas, px,
};

/// An element that hands `on_move` every move of the mouse in the window while the left button
/// is down, and calls `on_up` when it is released, wherever the mouse is. The view decides
/// whether a selection is under way.
pub fn follow_mouse<V: 'static>(
    view: WeakEntity<V>,
    on_move: impl Fn(&mut V, Point<Pixels>, &mut Context<V>) + Clone + 'static,
    on_up: impl Fn(&mut V, &mut Context<V>) + Clone + 'static,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            let (moved, up, released) = (view.clone(), on_up.clone(), on_move.clone());
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let _ = moved.update(cx, |this, cx| {
                    if e.pressed_button == Some(MouseButton::Left) {
                        released(this, e.position, cx);
                    } else {
                        up(this, cx);
                    }
                });
            });
            let (view, on_up) = (view.clone(), on_up.clone());
            window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left {
                    let _ = view.update(cx, |this, cx| on_up(this, cx));
                }
            });
        },
    )
    .absolute()
    .size_0()
}

/// Whether `at` is at or past an edge of `handle`'s area that it can still scroll towards
/// (sideways only when `sideways`); `apply` scrolls a step, longer the further out it is.
pub fn edge_scroll(handle: &ScrollHandle, at: Point<Pixels>, sideways: bool, apply: bool) -> bool {
    let (b, offset, max) = (handle.bounds(), handle.offset(), handle.max_offset());
    let margin = px(12.);
    let step = |out: Pixels| (out * 0.5).clamp(px(4.), px(60.));
    let mut o = offset;
    if sideways {
        if at.x < b.left() + margin {
            o.x = (o.x + step(b.left() + margin - at.x)).min(px(0.));
        } else if at.x > b.right() - margin {
            o.x = (o.x - step(at.x - (b.right() - margin))).max(-max.x);
        }
    }
    if at.y < b.top() + margin {
        o.y = (o.y + step(b.top() + margin - at.y)).min(px(0.));
    } else if at.y > b.bottom() - margin {
        o.y = (o.y - step(at.y - (b.bottom() - margin))).max(-max.y);
    }
    if o == offset {
        return false;
    }
    if apply {
        handle.set_offset(o);
    }
    true
}

/// Call `tick` about sixty times a second until it says to stop: it scrolls a step and moves
/// the selection along. Dropping the task stops it too.
pub fn autoscroll<V: 'static>(
    cx: &mut Context<V>,
    tick: impl Fn(&mut V, &mut Context<V>) -> bool + 'static,
) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            let going = this.update(cx, |this, cx| tick(this, cx)).unwrap_or(false);
            if !going {
                break;
            }
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
        }
    })
}
