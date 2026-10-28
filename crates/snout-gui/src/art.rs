//! Painting the house drawings of `snout_core::art` with egui, and the buttons built on them.

use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
pub use snout_core::art::{Drawing, Glyph, about, clean, mark, pick, seed, sparkle, welcome};
use snout_core::art::{Fill, Line};

use crate::theme::Palette;

fn at(q: snout_core::art::Pos2) -> Pos2 {
    pos2(q.x, q.y)
}

/// Painting a drawing with egui.
pub trait Paint {
    /// Paint fitted into `rect` (keeping its proportions, centred). `ink` is the line colour;
    /// pass `None` for the palette's.
    fn paint(&self, painter: &Painter, rect: Rect, ink: Option<Color32>);
    /// Allocate `size` in `ui` and paint there.
    fn show(&self, ui: &mut egui::Ui, size: Vec2) -> egui::Response;
}

impl Paint for Drawing {
    fn paint(&self, painter: &Painter, rect: Rect, ink: Option<Color32>) {
        let p = Palette::of(&painter.ctx().global_style().visuals);
        paint_with(self, painter, rect, ink.unwrap_or(p.ill_ink), p);
    }

    fn show(&self, ui: &mut egui::Ui, size: Vec2) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, None);
        }
        resp
    }
}

fn paint_with(d: &Drawing, painter: &Painter, rect: Rect, ink: Color32, p: &Palette) {
    let scale = (rect.width() / d.size.x).min(rect.height() / d.size.y);
    let origin = rect.center() - vec2(d.size.x, d.size.y) * scale / 2.0;
    let at = |q: snout_core::art::Pos2| origin + at(q).to_vec2() * scale;
    let width = (d.line_width * scale).max(1.0);

    for part in &d.parts {
        let color = match part.fill {
            Fill::None => Color32::TRANSPARENT,
            Fill::Blob => p.ill_blob,
            Fill::Paper => p.ill_paper,
            Fill::Tint => p.ill_tint,
            Fill::Accent => p.ill_accent,
            Fill::Clay => p.ill_clay,
            Fill::Ink => ink,
        };
        if part.fill != Fill::None {
            let mut mesh = Mesh::default();
            for (i, &q) in part.tris.iter().enumerate() {
                mesh.colored_vertex(at(q), color);
                mesh.indices.push(i as u32);
            }
            painter.add(Shape::mesh(mesh));
            if part.line == Line::None {
                // The mesh has hard edges; a hairline of the same colour smooths them.
                for (pts, _) in &part.outlines {
                    painter.add(Shape::closed_line(pts.iter().map(|&q| at(q)).collect(), Stroke::new(1.0, color)));
                }
            }
        }
        let stroke = match part.line {
            Line::None => continue,
            Line::Ink => Stroke::new(width, ink),
            Line::Ground => Stroke::new(width * 0.8, ink.gamma_multiply(0.35)),
        };
        for (pts, closed) in &part.outlines {
            let pts: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
            if *closed {
                painter.add(Shape::closed_line(pts, stroke));
            } else {
                // Round caps.
                for end in [pts.first(), pts.last()].into_iter().flatten() {
                    painter.circle_filled(*end, stroke.width / 2.0, stroke.color);
                }
                painter.add(Shape::line(pts, stroke));
            }
        }
    }
}

/// A button with a leading [`Glyph`], drawn in the button's own text colour.
pub fn glyph_button(ui: &mut egui::Ui, glyph: Glyph, text: &str, enabled: bool) -> egui::Response {
    glyph_button_sized(ui, glyph, text, enabled, Vec2::ZERO)
}

/// [`glyph_button`] at least `min_size`, its content centred.
pub fn glyph_button_sized(
    ui: &mut egui::Ui,
    glyph: Glyph,
    text: &str,
    enabled: bool,
    min_size: Vec2,
) -> egui::Response {
    let icon = 16.0;
    let gap = if text.is_empty() { 0.0 } else { 6.0 };
    let pad = vec2(9.0, 5.0);
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let content = vec2(icon + gap + galley.size().x, icon.max(galley.size().y));
    let size = (content + pad * 2.0).max(min_size);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let visuals = if enabled { *ui.style().interact(&resp) } else { ui.visuals().widgets.noninteractive };
        if enabled {
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        let ink = if enabled { visuals.fg_stroke.color } else { Palette::of(ui.visuals()).fg_dim.gamma_multiply(0.7) };
        let left = rect.center().x - content.x / 2.0;
        let icon_rect = Rect::from_min_size(pos2(left, rect.center().y - icon / 2.0), Vec2::splat(icon));
        glyph.drawing().paint(ui.painter(), icon_rect, Some(ink));
        let text_pos = pos2(icon_rect.max.x + gap, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(text_pos, galley, ink);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A quiet pill for pane headers: glyph and label, filled only under the pointer.
pub fn chip_button(ui: &mut egui::Ui, glyph: Glyph, text: &str, enabled: bool) -> egui::Response {
    let p = Palette::of(ui.visuals());
    let (icon, gap, pad) = (14.0, 5.0, vec2(8.0, 3.0));
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = vec2(icon + gap + galley.size().x, icon.max(galley.size().y)) + pad * 2.0;
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let hot = enabled && (resp.hovered() || resp.has_focus());
        if hot {
            ui.painter().rect(
                rect,
                rect.height() / 2.0,
                p.button_hover,
                Stroke::new(1.0, p.border),
                egui::StrokeKind::Inside,
            );
        }
        // Full ink whenever it can be pressed; only a disabled chip looks faded.
        let ink = if enabled { p.fg } else { p.fg_dim.gamma_multiply(0.6) };
        let icon_rect = Rect::from_min_size(pos2(rect.left() + pad.x, rect.center().y - icon / 2.0), Vec2::splat(icon));
        glyph.drawing().paint(ui.painter(), icon_rect, Some(ink));
        let text_pos = pos2(icon_rect.max.x + gap, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(text_pos, galley, ink);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A small frameless icon button, for pane headers.
pub fn icon_button(ui: &mut egui::Ui, glyph: Glyph, enabled: bool) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), sense);
    if ui.is_rect_visible(rect) {
        let p = Palette::of(ui.visuals());
        if enabled && resp.hovered() {
            ui.painter().rect_filled(rect, 5.0, p.button_hover);
        }
        let ink = if enabled { p.fg } else { p.fg_dim.gamma_multiply(0.6) };
        glyph.drawing().paint(ui.painter(), rect.shrink(3.0), Some(ink));
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, format!("{glyph:?}")));
    resp
}
