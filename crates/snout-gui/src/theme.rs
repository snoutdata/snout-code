//! The look: the SnoutData palettes (Studio's Gold for dark, the website's warm neutrals for
//! light), applied to egui's visuals, plus the illustration tones derived from them.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Theme, ThemePreference, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Match the system",
            Self::Dark => "Gold (dark)",
            Self::Light => "Light",
        }
    }

    fn preference(self) -> ThemePreference {
        match self {
            Self::System => ThemePreference::System,
            Self::Dark => ThemePreference::Dark,
            Self::Light => ThemePreference::Light,
        }
    }
}

/// Every colour the app uses. Pick one with [`Palette::of`]; nothing else names a colour.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// The ground between the panes.
    pub ground: Color32,
    /// The panes.
    pub panel: Color32,
    /// Raised surfaces: dialogs, menus, cards.
    pub card: Color32,
    pub input: Color32,
    pub stripe: Color32,
    pub button: Color32,
    pub button_hover: Color32,
    pub active: Color32,
    pub border: Color32,
    pub fg: Color32,
    pub fg_dim: Color32,
    pub accent: Color32,
    /// Text on an accent fill.
    pub on_accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub error: Color32,
    /// Ink line of the drawings.
    pub ill_ink: Color32,
    pub ill_blob: Color32,
    pub ill_paper: Color32,
    pub ill_tint: Color32,
    /// The drawings' one hue: a softer relative of `ok`.
    pub ill_accent: Color32,
    pub ill_warm: Color32,
    pub ill_clay: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// `a` mixed into `b`, `t` of the way (CSS `color-mix(in srgb, a t, b)`).
pub fn mix(a: Color32, t: f32, b: Color32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 * t + y as f32 * (1.0 - t)).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

impl Palette {
    pub const GOLD: Self = Self::derive(Base {
        ground: hex(0x050403),
        panel: hex(0x0d0c0b),
        card: hex(0x191714),
        input: hex(0x1b1917),
        stripe: hex(0x12110f),
        button: hex(0x1e1b18),
        button_hover: hex(0x2a2622),
        active: hex(0x2a2419),
        border: hex(0x262320),
        fg: hex(0xe9e4dc),
        fg_dim: hex(0x8f887e),
        accent: hex(0xe0b256),
        on_accent: hex(0x1a1408),
        ok: hex(0x6fd08c),
        warn: hex(0xe0b34a),
        error: hex(0xf0705c),
    });

    pub const LIGHT: Self = Self::derive(Base {
        ground: hex(0xeae9e1),
        panel: hex(0xf7f7f4),
        card: hex(0xffffff),
        input: hex(0xffffff),
        stripe: hex(0xf0efe9),
        button: hex(0xecebe4),
        button_hover: hex(0xe3e1d8),
        active: hex(0xe9e7de),
        border: hex(0xe5e3da),
        fg: hex(0x26251e),
        fg_dim: hex(0x76756d),
        accent: hex(0x26251e),
        on_accent: hex(0xf7f7f4),
        ok: hex(0x18794e),
        warn: hex(0xb06d0a),
        error: hex(0xc0392b),
    });

    pub fn of(visuals: &Visuals) -> &'static Self {
        if visuals.dark_mode { &Self::GOLD } else { &Self::LIGHT }
    }

    /// The illustration tones are derived from the base colours, as Studio does, so a
    /// drawing sits right in either palette.
    const fn derive(b: Base) -> Self {
        Self {
            ground: b.ground,
            panel: b.panel,
            card: b.card,
            input: b.input,
            stripe: b.stripe,
            button: b.button,
            button_hover: b.button_hover,
            active: b.active,
            border: b.border,
            fg: b.fg,
            fg_dim: b.fg_dim,
            accent: b.accent,
            on_accent: b.on_accent,
            ok: b.ok,
            warn: b.warn,
            error: b.error,
            ill_ink: const_mix(b.fg, 88, b.panel),
            ill_blob: const_mix(b.fg, 5, b.panel),
            ill_paper: const_mix(b.fg, 6, b.card),
            ill_tint: const_mix(b.fg, 20, b.card),
            ill_accent: const_mix(b.ok, 70, b.card),
            ill_warm: const_mix(b.warn, 65, b.card),
            ill_clay: const_mix(const_mix(b.error, 50, b.warn), 55, b.card),
        }
    }

    /// Background of a line a diff added or removed.
    pub fn diff_add(&self) -> Color32 {
        mix(self.ok, 0.16, self.panel)
    }

    pub fn diff_del(&self) -> Color32 {
        mix(self.error, 0.16, self.panel)
    }

    /// A soft wash of `color` for chips and badges.
    pub fn wash(&self, color: Color32) -> Color32 {
        mix(color, 0.14, self.panel)
    }

    /// Lane colours of the commit graph: the drawings' tones, then two quiet extras.
    pub fn lanes(&self) -> [Color32; 6] {
        let dark = self.panel.r() < 128;
        let (blue, violet) = if dark { (hex(0x7fa6d8), hex(0xb79ad6)) } else { (hex(0x3f6fa8), hex(0x7a55a8)) };
        let strong = |c: Color32| if dark { c } else { mix(c, 0.7, self.fg) };
        [strong(self.ill_accent), strong(self.ill_warm), blue, strong(self.ill_clay), violet, self.fg_dim]
    }
}

struct Base {
    ground: Color32,
    panel: Color32,
    card: Color32,
    input: Color32,
    stripe: Color32,
    button: Color32,
    button_hover: Color32,
    active: Color32,
    border: Color32,
    fg: Color32,
    fg_dim: Color32,
    accent: Color32,
    on_accent: Color32,
    ok: Color32,
    warn: Color32,
    error: Color32,
}

const fn const_mix(a: Color32, pct: u32, b: Color32) -> Color32 {
    const fn l(x: u8, y: u8, pct: u32) -> u8 {
        ((x as u32 * pct + y as u32 * (100 - pct) + 50) / 100) as u8
    }
    Color32::from_rgb(l(a.r(), b.r(), pct), l(a.g(), b.g(), pct), l(a.b(), b.b(), pct))
}

/// Install both palettes and follow `choice`.
pub fn apply(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.set_visuals_of(Theme::Dark, visuals(&Palette::GOLD, true));
    ctx.set_visuals_of(Theme::Light, visuals(&Palette::LIGHT, false));
    ctx.set_theme(choice.preference());
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.window_margin = egui::Margin::same(18);
        style.spacing.interact_size.y = 22.0;
    });
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let radius = CornerRadius::same(6);

    v.panel_fill = p.panel;
    v.window_fill = p.card;
    v.extreme_bg_color = p.input;
    v.faint_bg_color = p.stripe;
    v.code_bg_color = p.input;
    v.text_edit_bg_color = Some(p.input);
    v.hyperlink_color = if dark { p.accent } else { p.fg };
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.error;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow.color = Color32::from_black_alpha(if dark { 150 } else { 40 });
    v.popup_shadow.color = Color32::from_black_alpha(if dark { 120 } else { 28 });
    v.selection.bg_fill = mix(p.accent, if dark { 0.22 } else { 0.12 }, p.panel);
    v.selection.stroke = Stroke::new(1.0, p.fg);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.fg);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = p.button;
    w.inactive.weak_bg_fill = p.button;
    w.inactive.bg_stroke = Stroke::new(1.0, mix(p.fg, 0.08, p.button));
    w.inactive.fg_stroke = Stroke::new(1.0, p.fg);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = p.button_hover;
    w.hovered.weak_bg_fill = p.button_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, mix(p.fg, 0.25, p.button_hover));
    w.hovered.fg_stroke = Stroke::new(1.5, p.fg);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.active;
    w.active.weak_bg_fill = p.active;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.fg);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = p.active;
    w.open.weak_bg_fill = p.active;
    w.open.bg_stroke = Stroke::new(1.0, p.border);
    w.open.fg_stroke = Stroke::new(1.0, p.fg);
    w.open.corner_radius = radius;
    v
}

/// The frame of a pane: lifted off the ground with a hairline and rounded corners.
pub fn pane(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .outer_margin(egui::Margin::same(3))
}

/// The primary action of a pane or dialog: an accent pill.
pub fn primary_button(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let p = Palette::of(ui.visuals());
    pill_button(ui, enabled, text, p.accent, p.on_accent)
}

/// A destructive action: a red pill.
pub fn danger_button(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let p = Palette::of(ui.visuals());
    let ink = if ui.visuals().dark_mode { hex(0x1c0d0a) } else { Color32::WHITE };
    pill_button(ui, enabled, text, p.error, ink)
}

/// A secondary action beside one of the above.
pub fn plain_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).corner_radius(CornerRadius::same(255)).min_size(egui::vec2(84.0, 28.0)))
}

fn pill_button(ui: &mut egui::Ui, enabled: bool, text: &str, fill: Color32, ink: Color32) -> egui::Response {
    let p = Palette::of(ui.visuals());
    let (fill, color) = if enabled { (fill, ink) } else { (p.button, p.fg_dim) };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).color(color).strong())
            .fill(fill)
            .corner_radius(CornerRadius::same(255))
            .min_size(egui::vec2(88.0, 28.0)),
    )
}
