//! The look: the SnoutData palettes (Studio's Gold for dark, the website's warm neutrals for
//! light), carried over from the egui app and laid onto GPUI Kit's theme.

use gpui::{App, Global, Hsla, Rgba, Window, WindowAppearance};
use gpui_component::{Theme, ThemeMode};

use crate::settings::ThemeChoice;

/// Every colour the app uses. Pick one with [`palette`]; nothing else names a colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// The ground between the panes.
    pub ground: Hsla,
    /// The panes.
    pub panel: Hsla,
    /// Raised surfaces: dialogs, menus, cards.
    pub card: Hsla,
    pub input: Hsla,
    pub stripe: Hsla,
    pub button: Hsla,
    pub button_hover: Hsla,
    pub active: Hsla,
    pub border: Hsla,
    pub fg: Hsla,
    pub fg_dim: Hsla,
    pub accent: Hsla,
    /// Text on an accent fill.
    pub on_accent: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub error: Hsla,
    /// The drawings' one hue: a softer relative of `ok`.
    pub ill_accent: Hsla,
    pub ill_warm: Hsla,
    pub ill_clay: Hsla,
}

fn hex(rgb: u32) -> Hsla {
    gpui::rgb(rgb).into()
}

/// `a` mixed into `b`, `t` of the way (CSS `color-mix(in srgb, a t, b)`).
pub fn mix(a: Hsla, t: f32, b: Hsla) -> Hsla {
    let (a, b): (Rgba, Rgba) = (a.into(), b.into());
    let l = |x: f32, y: f32| x * t + y * (1.0 - t);
    Rgba { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: 1.0 }.into()
}

struct Base {
    ground: u32,
    panel: u32,
    card: u32,
    input: u32,
    stripe: u32,
    button: u32,
    button_hover: u32,
    active: u32,
    border: u32,
    fg: u32,
    fg_dim: u32,
    accent: u32,
    on_accent: u32,
    ok: u32,
    warn: u32,
    error: u32,
}

const GOLD: Base = Base {
    ground: 0x050403,
    panel: 0x0d0c0b,
    card: 0x191714,
    input: 0x1b1917,
    stripe: 0x12110f,
    button: 0x1e1b18,
    button_hover: 0x2a2622,
    active: 0x2a2419,
    border: 0x262320,
    fg: 0xe9e4dc,
    fg_dim: 0x8f887e,
    accent: 0xe0b256,
    on_accent: 0x1a1408,
    ok: 0x6fd08c,
    warn: 0xe0b34a,
    error: 0xf0705c,
};

const LIGHT: Base = Base {
    ground: 0xeae9e1,
    panel: 0xf7f7f4,
    card: 0xffffff,
    input: 0xffffff,
    stripe: 0xf0efe9,
    button: 0xecebe4,
    button_hover: 0xe3e1d8,
    active: 0xe9e7de,
    border: 0xe5e3da,
    fg: 0x26251e,
    fg_dim: 0x76756d,
    accent: 0x26251e,
    on_accent: 0xf7f7f4,
    ok: 0x18794e,
    warn: 0xb06d0a,
    error: 0xc0392b,
};

impl Palette {
    fn derive(b: &Base, dark: bool) -> Self {
        let (fg, card, ok, warn, error) = (hex(b.fg), hex(b.card), hex(b.ok), hex(b.warn), hex(b.error));
        Self {
            dark,
            ground: hex(b.ground),
            panel: hex(b.panel),
            card,
            input: hex(b.input),
            stripe: hex(b.stripe),
            button: hex(b.button),
            button_hover: hex(b.button_hover),
            active: hex(b.active),
            border: hex(b.border),
            fg,
            fg_dim: hex(b.fg_dim),
            accent: hex(b.accent),
            on_accent: hex(b.on_accent),
            ok,
            warn,
            error,
            ill_accent: mix(ok, 0.70, card),
            ill_warm: mix(warn, 0.65, card),
            ill_clay: mix(mix(error, 0.5, warn), 0.55, card),
        }
    }

    pub fn gold() -> Self {
        Self::derive(&GOLD, true)
    }

    pub fn light() -> Self {
        Self::derive(&LIGHT, false)
    }

    /// Background of a line a diff added or removed.
    pub fn diff_add(&self) -> Hsla {
        mix(self.ok, 0.16, self.panel)
    }

    pub fn diff_del(&self) -> Hsla {
        mix(self.error, 0.16, self.panel)
    }

    /// A soft wash of `color` for chips and badges.
    pub fn wash(&self, color: Hsla) -> Hsla {
        mix(color, 0.14, self.panel)
    }

    pub fn selection(&self) -> Hsla {
        mix(self.accent, if self.dark { 0.22 } else { 0.12 }, self.panel)
    }

    /// Lane colours of the commit graph: the drawings' tones, then two quiet extras.
    pub fn lanes(&self) -> [Hsla; 6] {
        let (blue, violet) = if self.dark { (hex(0x7fa6d8), hex(0xb79ad6)) } else { (hex(0x3f6fa8), hex(0x7a55a8)) };
        let strong = |c: Hsla| if self.dark { c } else { mix(c, 0.7, self.fg) };
        [strong(self.ill_accent), strong(self.ill_warm), blue, strong(self.ill_clay), violet, self.fg_dim]
    }
}

/// The palette in use.
pub struct Current(pub Palette);

impl Global for Current {}

pub fn palette(cx: &App) -> Palette {
    cx.try_global::<Current>().map_or_else(Palette::gold, |c| c.0)
}

/// Whether `choice` means dark right now.
pub fn is_dark(choice: ThemeChoice, cx: &App) -> bool {
    match choice {
        ThemeChoice::Dark => true,
        ThemeChoice::Light => false,
        ThemeChoice::System => matches!(cx.window_appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark),
    }
}

/// Follow `choice`: pick the palette and lay it onto GPUI Kit's theme.
pub fn apply(choice: ThemeChoice, window: Option<&mut Window>, cx: &mut App) {
    let dark = is_dark(choice, cx);
    let p = if dark { Palette::gold() } else { Palette::light() };
    cx.set_global(Current(p));
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, window, cx);
    Theme::update(cx, |t| {
        t.background = p.panel;
        t.foreground = p.fg;
        t.border = p.border;
        t.muted = p.stripe;
        t.muted_foreground = p.fg_dim;
        t.accent = p.button_hover;
        t.accent_foreground = p.fg;
        t.primary = p.accent;
        t.primary_hover = mix(p.accent, 0.88, p.fg);
        t.primary_active = mix(p.accent, 0.8, p.fg);
        t.primary_foreground = p.on_accent;
        t.secondary = p.button;
        t.secondary_hover = p.button_hover;
        t.secondary_active = p.active;
        t.secondary_foreground = p.fg;
        t.danger = p.error;
        t.success = p.ok;
        t.warning = p.warn;
        t.input = p.border;
        t.ring = p.accent;
        t.caret = p.fg;
        t.selection = p.selection();
        t.popover = p.card;
        t.popover_foreground = p.fg;
        t.colors.list = p.panel;
        t.list_hover = p.button_hover;
        t.list_active = p.selection();
        t.list_active_border = p.accent;
        t.list_even = p.stripe;
        t.list_head = p.panel;
        t.table = p.panel;
        t.table_even = p.stripe;
        t.table_hover = p.button_hover;
        t.table_active = p.selection();
        t.table_head = p.panel;
        t.tab = p.ground;
        t.tab_active = p.panel;
        t.tab_active_foreground = p.fg;
        t.tab_bar = p.ground;
        t.tab_foreground = p.fg_dim;
        t.sidebar = p.ground;
        t.sidebar_foreground = p.fg;
        t.sidebar_border = p.border;
        t.title_bar = p.ground;
        t.title_bar_border = p.border;
        t.status_bar = p.ground;
        t.status_bar_border = p.border;
        t.link = if dark { p.accent } else { p.fg };
        t.window_border = p.border;
        t.font_size = gpui::px(14.);
        t.radius = gpui::px(6.);
        t.radius_lg = gpui::px(12.);
    });
}
