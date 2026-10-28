//! The house drawings of `snout_core::art` (the egui app's own icons and spot drawings),
//! painted with GPUI.

use gpui::{
    App, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Styled as _, Window, canvas, point, px, quad, size,
    transparent_black,
};
use snout_core::art::{Drawing, Fill, Glyph, Line, Pos2};

use crate::theme::{Palette, palette};

/// Paint `d` fitted into `bounds` (keeping its proportions, centred), its line in `ink`.
pub fn paint(d: &Drawing, bounds: Bounds<Pixels>, ink: Hsla, p: &Palette, window: &mut Window) {
    paint_with(d, bounds, ink, p, false, window)
}

/// Paint `d` as [`paint`] does; `outline` leaves out the coloured fills, for an icon drawn
/// in one ink (the activity bar's).
fn paint_with(d: &Drawing, bounds: Bounds<Pixels>, ink: Hsla, p: &Palette, outline: bool, window: &mut Window) {
    let scale = (f32::from(bounds.size.width) / d.size.x).min(f32::from(bounds.size.height) / d.size.y);
    let c = bounds.center();
    let origin = point(c.x - px(d.size.x * scale / 2.0), c.y - px(d.size.y * scale / 2.0));
    let at = |q: Pos2| point(origin.x + px(q.x * scale), origin.y + px(q.y * scale));
    let width = (d.line_width * scale).max(1.0);

    for part in &d.parts {
        let color = match part.fill {
            Fill::None => transparent_black(),
            Fill::Blob => p.ill_blob,
            Fill::Paper => p.ill_paper,
            Fill::Tint => p.ill_tint,
            Fill::Accent => p.ill_accent,
            Fill::Clay => p.ill_clay,
            Fill::Ink => ink,
        };
        if part.fill != Fill::None && !(outline && part.fill != Fill::Ink) {
            for (pts, _) in &part.outlines {
                let mut path = PathBuilder::fill();
                for (i, &q) in pts.iter().enumerate() {
                    if i == 0 { path.move_to(at(q)) } else { path.line_to(at(q)) }
                }
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        }
        let (w, color) = match part.line {
            Line::None => continue,
            Line::Ink => (width, ink),
            Line::Ground => (width * 0.8, ink.opacity(0.35)),
        };
        for (pts, closed) in &part.outlines {
            let mut path = PathBuilder::stroke(px(w));
            for (i, &q) in pts.iter().enumerate() {
                if i == 0 { path.move_to(at(q)) } else { path.line_to(at(q)) }
            }
            if *closed {
                path.close();
            } else {
                // Round caps.
                for end in [pts.first(), pts.last()].into_iter().flatten() {
                    let e = at(*end);
                    let r = px(w / 2.0);
                    let b = Bounds::new(point(e.x - r, e.y - r), size(r * 2.0, r * 2.0));
                    window.paint_quad(quad(b, r, color, px(0.), transparent_black(), Default::default()));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        }
    }
}

/// When the sparks started twinkling.
static EPOCH: std::sync::LazyLock<std::time::Instant> = std::sync::LazyLock::new(std::time::Instant::now);

/// Paint `d`'s twinkling sparks, `secs` into their twinkle (still under reduced motion).
fn paint_sparks(d: &Drawing, bounds: Bounds<Pixels>, ink: Hsla, secs: Option<f32>, window: &mut Window) {
    let scale = (f32::from(bounds.size.width) / d.size.x).min(f32::from(bounds.size.height) / d.size.y);
    let c = bounds.center();
    let origin = point(c.x - px(d.size.x * scale / 2.0), c.y - px(d.size.y * scale / 2.0));
    for (i, (at, s)) in d.sparks.iter().enumerate() {
        let (k, turn) = secs.map_or((1.0, 0.0), |t| snout_core::art::twinkle(i, t));
        let pts = snout_core::art::spark_outline(at.x, at.y, s * k, turn);
        let mut path = PathBuilder::fill();
        for (j, q) in pts.iter().enumerate() {
            let q = point(origin.x + px(q.x * scale), origin.y + px(q.y * scale));
            if j == 0 { path.move_to(q) } else { path.line_to(q) }
        }
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, ink);
        }
    }
}

/// A drawing as an element of `w` by `h`, its line in `ink` (the palette's when `None`).
/// Its sparks twinkle, unless the system asks for reduced motion.
pub fn drawing(d: &'static Drawing, w: f32, h: f32, ink: Option<Hsla>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx: &mut App| {
            let p = palette(cx);
            let ink = ink.unwrap_or(p.ill_ink);
            paint(d, bounds, ink, &p, window);
            if !d.sparks.is_empty() {
                let still = cx.reduce_motion();
                paint_sparks(d, bounds, ink, (!still).then(|| EPOCH.elapsed().as_secs_f32()), window);
                if !still {
                    window.request_animation_frame();
                }
            }
        },
    )
    .flex_none()
    .w(px(w))
    .h(px(h))
}

/// An icon, `size` square, in `ink`.
pub fn glyph(g: Glyph, size: f32, ink: Hsla) -> impl IntoElement {
    drawing(g.drawing(), size, size, Some(ink))
}

/// An icon, `size` square, its lines alone in `ink`: no coloured fills.
pub fn outline_glyph(g: Glyph, size: f32, ink: Hsla) -> impl IntoElement {
    let d = g.drawing();
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx: &mut App| {
            let p = palette(cx);
            paint_with(d, bounds, ink, &p, true, window);
        },
    )
    .flex_none()
    .w(px(size))
    .h(px(size))
}
