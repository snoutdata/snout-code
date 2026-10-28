//! Painter-based diff rendering: side-by-side or unified, optionally compacted.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, TextStyle, pos2, vec2};
use snout_core::diff_layout::{DiffLayout, SbsLine, UniLine, gap_label};
use snout_core::{FileDiff, RowKind};

use crate::theme::Palette;

/// A diff prepared for display. Rebuild when the diff or the compact flag changes.
pub struct DiffView {
    pub diff: FileDiff,
    layout: DiffLayout,
}

impl DiffView {
    pub fn new(diff: FileDiff, compact: bool) -> Self {
        let layout = DiffLayout::new(diff.clone(), compact);
        Self { diff, layout }
    }

    pub fn show(&self, ui: &mut egui::Ui, side_by_side: bool) {
        let font = TextStyle::Monospace.resolve(ui.style());
        let char_w = ui.painter().layout_no_wrap("0".into(), font.clone(), Color32::WHITE).size().x;
        let row_h = ui.text_style_height(&TextStyle::Monospace) + 2.0;
        let colors = Colors::new(ui.visuals());
        let p = Paint { font, char_w, colors, digits: self.layout.number_digits };

        let count = if side_by_side { self.layout.sbs.len() } else { self.layout.unified.len() };
        let spacing = ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, row_h - spacing, count, |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for idx in range {
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                if side_by_side {
                    self.paint_sbs(ui, &p, rect, self.layout.sbs[idx]);
                } else {
                    self.paint_unified(ui, &p, rect, self.layout.unified[idx]);
                }
            }
        });
    }

    fn paint_sbs(&self, ui: &egui::Ui, p: &Paint, rect: Rect, line: SbsLine) {
        let row = match line {
            SbsLine::Gap(n) => return p.gap(ui, rect, n),
            SbsLine::Row(i) => &self.diff.rows[i],
        };
        let mid = rect.center().x;
        let left = Rect::from_min_max(rect.min, pos2(mid - 1.0, rect.max.y));
        let right = Rect::from_min_max(pos2(mid + 1.0, rect.min.y), rect.max);
        let (old_bg, new_bg) = match row.kind {
            RowKind::Equal => (None, None),
            RowKind::Delete => (Some(p.colors.del), None),
            RowKind::Insert => (None, Some(p.colors.add)),
            RowKind::Replace => (Some(p.colors.del), Some(p.colors.add)),
        };
        let old_bg = if row.old.is_none() { Some(p.colors.empty) } else { old_bg };
        let new_bg = if row.new.is_none() { Some(p.colors.empty) } else { new_bg };
        p.cell(ui, left, old_bg, &[row.old.as_ref().map(|l| l.0)], "", row.old.as_ref().map(|l| l.1.as_str()));
        p.cell(ui, right, new_bg, &[row.new.as_ref().map(|l| l.0)], "", row.new.as_ref().map(|l| l.1.as_str()));
        ui.painter().vline(mid, rect.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
    }

    fn paint_unified(&self, ui: &egui::Ui, p: &Paint, rect: Rect, line: UniLine) {
        let rows = &self.diff.rows;
        match line {
            UniLine::Gap(n) => p.gap(ui, rect, n),
            UniLine::Both(i) => {
                let r = &rows[i];
                let (o, n) = (r.old.as_ref().unwrap(), r.new.as_ref().unwrap());
                p.cell(ui, rect, None, &[Some(o.0), Some(n.0)], " ", Some(&n.1));
            }
            UniLine::Old(i) => {
                let o = rows[i].old.as_ref().unwrap();
                p.cell(ui, rect, Some(p.colors.del), &[Some(o.0), None], "-", Some(&o.1));
            }
            UniLine::New(i) => {
                let n = rows[i].new.as_ref().unwrap();
                p.cell(ui, rect, Some(p.colors.add), &[None, Some(n.0)], "+", Some(&n.1));
            }
        }
    }
}

struct Colors {
    add: Color32,
    del: Color32,
    empty: Color32,
    gap: Color32,
    text: Color32,
    weak: Color32,
}

impl Colors {
    fn new(v: &egui::Visuals) -> Self {
        let p = Palette::of(v);
        Self {
            add: p.diff_add(),
            del: p.diff_del(),
            empty: v.faint_bg_color,
            gap: v.extreme_bg_color,
            text: v.text_color(),
            weak: v.weak_text_color(),
        }
    }
}

struct Paint {
    font: FontId,
    char_w: f32,
    colors: Colors,
    digits: usize,
}

impl Paint {
    /// A line-number gutter (one column per entry in `numbers`), a marker and the text.
    fn cell(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        bg: Option<Color32>,
        numbers: &[Option<u32>],
        marker: &str,
        text: Option<&str>,
    ) {
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if let Some(bg) = bg {
            painter.rect_filled(rect, 0.0, bg);
        }
        let num_w = (self.digits as f32 + 1.0) * self.char_w;
        let mut x = rect.min.x + 4.0;
        for n in numbers {
            if let Some(n) = n {
                let pos = pos2(x + num_w - self.char_w * 0.5, rect.center().y);
                painter.text(pos, Align2::RIGHT_CENTER, n.to_string(), self.font.clone(), self.colors.weak);
            }
            x += num_w;
        }
        x += self.char_w * 0.5;
        if !marker.is_empty() {
            painter.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, marker, self.font.clone(), self.colors.weak);
            x += self.char_w * 1.5;
        }
        if let Some(text) = text {
            let text = text.replace('\t', "    ");
            painter.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, self.font.clone(), self.colors.text);
        }
    }

    fn gap(&self, ui: &egui::Ui, rect: Rect, n: usize) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, self.colors.gap);
        let label = gap_label(n);
        painter.text(rect.center(), Align2::CENTER_CENTER, label, self.font.clone(), self.colors.weak);
    }
}
