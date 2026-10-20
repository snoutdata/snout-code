//! Lanes for the commit graph beside the Journal.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, pos2};
use snout_core::Commit;

use crate::theme::Palette;

/// Horizontal distance between lanes, in points.
pub const LANE_W: f32 = 12.0;

/// One row of the graph. Each edge joins two lanes over half a row: `top` edges run from the
/// top of the row to the commit's height, the others from there to the bottom.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct GraphRow {
    pub lane: usize,
    pub edges: Vec<Edge>,
    /// Lanes in use on this row.
    pub width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub top: bool,
}

/// Lay out `commits` (children before parents, as `git log --date-order` lists them).
pub fn layout(commits: &[Commit]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<&str>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());
    for c in commits {
        let id = c.id.as_str();
        let free = |lanes: &mut Vec<Option<&str>>| match lanes.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                lanes.push(None);
                lanes.len() - 1
            }
        };
        let lane = match lanes.iter().position(|l| *l == Some(id)) {
            Some(i) => i,
            None => free(&mut lanes),
        };
        let mut edges = Vec::new();
        // Lines arriving from above: every branch waiting for this commit converges on it,
        // the rest pass by.
        let mut passing = vec![false; lanes.len()];
        for (i, l) in lanes.iter().enumerate() {
            match l {
                Some(x) if *x == id => edges.push(Edge { from: i, to: lane, top: true }),
                Some(_) => {
                    edges.push(Edge { from: i, to: i, top: true });
                    passing[i] = true;
                }
                None => {}
            }
        }
        for l in lanes.iter_mut().filter(|l| **l == Some(id)) {
            *l = None;
        }
        // Lines leaving downwards, to each parent: the first parent keeps this lane unless
        // another branch is already heading for it.
        for (k, parent) in c.parents.iter().enumerate() {
            let to = match lanes.iter().position(|l| *l == Some(parent.as_str())) {
                Some(j) => j,
                None => {
                    let j = if k == 0 { lane } else { free(&mut lanes) };
                    lanes[j] = Some(parent.as_str());
                    j
                }
            };
            edges.push(Edge { from: lane, to, top: false });
        }
        for (i, pass) in passing.iter().enumerate() {
            if *pass {
                edges.push(Edge { from: i, to: i, top: false });
            }
        }
        while lanes.last() == Some(&None) {
            lanes.pop();
        }
        let width = edges.iter().map(|e| e.from.max(e.to)).chain([lane]).max().unwrap_or(0) + 1;
        rows.push(GraphRow { lane, edges, width });
    }
    rows
}

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
    let lanes = rows.iter().map(|r| r.width).max().unwrap_or(1).min(12);
    lanes as f32 * LANE_W + 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            short_id: id.into(),
            author: String::new(),
            email: String::new(),
            time: 0,
            date: String::new(),
            refs: Vec::new(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            summary: String::new(),
            unpushed: false,
        }
    }

    #[test]
    fn a_branch_and_its_merge_take_a_second_lane() {
        // m merges b into a; both come from root.
        let rows = layout(&[commit("m", &["a", "b"]), commit("a", &["r"]), commit("b", &["r"]), commit("r", &[])]);
        let lanes: Vec<usize> = rows.iter().map(|r| r.lane).collect();
        assert_eq!(lanes, [0, 0, 1, 0]);
        assert!(rows[0].edges.contains(&Edge { from: 0, to: 1, top: false }), "merge forks right");
        assert!(rows[1].edges.contains(&Edge { from: 1, to: 1, top: true }), "b's lane passes a");
        assert!(rows[2].edges.contains(&Edge { from: 1, to: 0, top: false }), "b heads back to the root's lane");
        assert_eq!(rows[3].width, 1);
        assert_eq!(rows.iter().map(|r| r.width).max(), Some(2));
    }

    #[test]
    fn a_straight_history_is_one_lane() {
        let rows = layout(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
        assert!(rows[2].edges.iter().all(|e| e.top), "nothing below the root");
    }
}
