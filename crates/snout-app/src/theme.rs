//! The look, in one place: a neutral dark palette (near black greys, one blue accent,
//! hairline separators) and its light twin, the type, and the editor's syntax colours, laid
//! onto GPUI Kit's theme. Every view takes its colours from [`palette`].

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
    /// Ink line of the drawings.
    pub ill_ink: Hsla,
    pub ill_blob: Hsla,
    pub ill_paper: Hsla,
    pub ill_tint: Hsla,
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

/// Dark: the chrome (title bar, stripes, tool windows) a step lighter than the editor.
const DARK: Base = Base {
    ground: 0x2a2c30,
    panel: 0x1f2023,
    card: 0x2d2f34,
    input: 0x1f2023,
    stripe: 0x25272b,
    button: 0x35373c,
    button_hover: 0x3a3d43,
    active: 0x2b3f63,
    border: 0x393b41,
    fg: 0xdcdfe4,
    fg_dim: 0x8b8f97,
    accent: 0x3b78e7,
    on_accent: 0xffffff,
    ok: 0x5cb46a,
    warn: 0xdba43f,
    error: 0xe0605a,
};

/// Light: white editor, a pale grey chrome.
const LIGHT: Base = Base {
    ground: 0xf3f4f6,
    panel: 0xffffff,
    card: 0xffffff,
    input: 0xffffff,
    stripe: 0xf6f7f9,
    button: 0xeceef1,
    button_hover: 0xe3e5e9,
    active: 0xd6e2fa,
    border: 0xdcdee3,
    fg: 0x1e1f23,
    fg_dim: 0x676b74,
    accent: 0x3266d4,
    on_accent: 0xffffff,
    ok: 0x2d8a42,
    warn: 0xa66b00,
    error: 0xcc3a32,
};

impl Palette {
    fn derive(b: &Base, dark: bool) -> Self {
        let (fg, card, ok, warn, error) = (hex(b.fg), hex(b.card), hex(b.ok), hex(b.warn), hex(b.error));
        let panel = hex(b.panel);
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
            ill_ink: mix(fg, 0.88, panel),
            ill_blob: mix(fg, 0.05, panel),
            ill_paper: mix(fg, 0.06, card),
            ill_tint: mix(fg, 0.20, card),
            ill_accent: mix(ok, 0.70, card),
            ill_warm: mix(warn, 0.65, card),
            ill_clay: mix(mix(error, 0.5, warn), 0.55, card),
        }
    }

    pub fn dark() -> Self {
        Self::derive(&DARK, true)
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

    /// Text of lesser weight: the text colour at 60 %, as egui draws weak text.
    pub fn weak(&self) -> Hsla {
        self.fg.opacity(0.6)
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
    cx.try_global::<Current>().map_or_else(Palette::dark, |c| c.0)
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
    let p = if dark { Palette::dark() } else { Palette::light() };
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
        t.tab_active = p.ground;
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
        t.link = p.accent;
        t.window_border = p.border;
        t.font_family = crate::look::FONT.into();
        t.mono_font_family = crate::look::MONO.into();
        t.font_size = gpui::px(crate::look::BODY);
        t.radius = gpui::px(5.);
        t.radius_lg = gpui::px(8.);
        t.highlight_theme = std::sync::Arc::new(syntax(&p));
    });
}

/// The editor's colours: the syntax in muted tones that sit on the neutral greys, and its
/// gutter and current line.
pub fn syntax(p: &Palette) -> gpui_component::highlighter::HighlightTheme {
    let hex = |c: Hsla| {
        let c: Rgba = c.into();
        let b = |x: f32| (x.clamp(0., 1.) * 255.).round() as u8;
        format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
    };
    let s = syntax_colors(p);
    let style = |c: Hsla| serde_json::json!({ "color": hex(c) });
    let italic = |c: Hsla| serde_json::json!({ "color": hex(c), "font_style": "italic" });
    let syntax = serde_json::json!({
        "keyword": style(s.keyword),
        "type": style(s.ty),
        "constructor": style(s.ty),
        "enum": style(s.ty),
        "function": style(s.function),
        "string": style(s.string),
        "string.escape": style(s.number),
        "string.special": style(s.string),
        "string.regex": style(s.string),
        "text.literal": style(s.string),
        "number": style(s.number),
        "boolean": style(s.number),
        "constant": style(s.number),
        "comment": italic(s.comment),
        "comment_doc": italic(s.comment),
        "attribute": style(s.attribute),
        "tag": style(s.keyword),
        "label": style(s.attribute),
        "property": style(s.property),
        "variable": style(p.fg),
        "variable.special": style(s.keyword),
        "operator": style(s.punctuation),
        "punctuation": style(s.punctuation),
        "punctuation.bracket": style(s.punctuation),
        "punctuation.delimiter": style(s.punctuation),
        "title": { "color": hex(s.keyword), "font_weight": 600 },
        "emphasis": { "font_style": "italic" },
        "emphasis.strong": { "font_weight": 700 },
        "link_text": style(s.ty),
        "link_uri": italic(s.ty),
        "embedded": style(p.fg),
    });
    let mut style_json = serde_json::json!({
        "editor.background": hex(p.panel),
        "editor.foreground": hex(p.fg),
        "editor.line_number": hex(s.gutter),
        "editor.active_line_number": hex(p.fg),
        "editor.active_line.background": hex(active_line(p)),
        "editor.invisible": hex(mix(p.fg, 0.25, p.panel)),
        "editor.gutter.background": hex(p.panel),
        "error": hex(p.error),
        "warning": hex(p.warn),
    });
    style_json["syntax"] = syntax;
    let json = serde_json::json!({
        "name": if p.dark { "SnoutCode Dark" } else { "SnoutCode Light" },
        "appearance": if p.dark { "dark" } else { "light" },
        "style": style_json,
    });
    serde_json::from_value(json).expect("the syntax theme is well formed")
}

/// The tones of the code, picked to sit with the palette.
pub struct SyntaxColors {
    pub keyword: Hsla,
    pub ty: Hsla,
    pub function: Hsla,
    pub string: Hsla,
    pub number: Hsla,
    pub comment: Hsla,
    pub attribute: Hsla,
    pub property: Hsla,
    pub punctuation: Hsla,
    pub gutter: Hsla,
}

pub fn syntax_colors(p: &Palette) -> SyntaxColors {
    if p.dark {
        SyntaxColors {
            keyword: hex(0xd29b74),
            ty: hex(0xc9cdd4),
            function: hex(0x62aaee),
            string: hex(0x78b383),
            number: hex(0x44b0bd),
            comment: hex(0x7c8089),
            attribute: hex(0xbcb46e),
            property: hex(0xc58bc0),
            punctuation: mix(p.fg, 0.75, p.panel),
            gutter: mix(p.fg, 0.32, p.panel),
        }
    } else {
        SyntaxColors {
            keyword: hex(0x1d3fa6),
            ty: hex(0x23252b),
            function: hex(0x0d6680),
            string: hex(0x247a35),
            number: hex(0x2556d8),
            comment: hex(0x8a8d93),
            attribute: hex(0x8f7d12),
            property: hex(0x7d2391),
            punctuation: mix(p.fg, 0.75, p.panel),
            gutter: mix(p.fg, 0.38, p.panel),
        }
    }
}

/// The band behind the caret's line in the editor.
pub fn active_line(p: &Palette) -> Hsla {
    mix(p.fg, if p.dark { 0.05 } else { 0.045 }, p.panel)
}
