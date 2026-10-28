//! Lanes for the commit graph beside the Journal.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, pos2};

use crate::theme::Palette;

/// Horizontal distance between lanes, in points.
pub const LANE_W: f32 = 12.0;

pub use snout_core::graph::{GraphRow, layout};

fn lane_x(rect: Rect, lane: usize) -> f32 {
    rect.left() + LANE_W * 0.5 + 4.0 + lane as f32 * LANE_W
}

/// Paint one row of the graph into `rect` (the cell). `filled` paints the commit solid (it
/// is not pushed yet); `head` adds a ring.
pub fn paint(ui: &egui::Ui, rect: Rect, row: &GraphRow, filled: bool, head: bool) {
    let p = Palette::of(ui.visuals());
    let colors = p.lanes();
    let color = |lane: usize| colors[lane % colors.len()];
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let (top, mid, bottom) = (rect.top(), rect.center().y, rect.bottom());
    for e in &row.edges {
        let (y0, y1) = if e.top { (top, mid) } else { (mid, bottom) };
        let (a, b) = (pos2(lane_x(rect, e.from), y0), pos2(lane_x(rect, e.to), y1));
        // A line keeps the colour of the lane it belongs to: the branch it comes from when it
        // converges, the branch it goes to when it forks.
        let stroke = Stroke::new(1.6, color(if e.top { e.from } else { e.to }));
        if a.x == b.x {
            painter.line_segment([a, b], stroke);
        } else {
            let k = (b.y - a.y) * 0.6;
            let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                [a, Pos2::new(a.x, a.y + k), Pos2::new(b.x, b.y - k), b],
                false,
                Color32::TRANSPARENT,
                stroke,
            );
            painter.add(Shape::CubicBezier(curve));
        }
    }
    let c = pos2(lane_x(rect, row.lane), mid);
    let lane = color(row.lane);
    if head {
        painter.circle_stroke(c, 6.5, Stroke::new(1.5, lane));
    }
    painter.circle(c, 4.0, if filled { lane } else { p.panel }, Stroke::new(1.8, lane));
}

pub fn width(rows: &[GraphRow]) -> f32 {
    snout_core::graph::lane_count(rows) as f32 * LANE_W + 8.0
}
