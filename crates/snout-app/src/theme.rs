//! The look, in one place: the colour themes (Studio, the default, warm and modelled on
//! Snout Studio; Vivid; Deep; and Classic, the earlier neutral greys), each in dark and light,
//! the type, and the editor's syntax colours, laid onto GPUI Kit's theme. Every view takes
//! its colours from [`palette`]; nothing else names a colour.

mod palettes;

use gpui::{App, Global, Hsla, Rgba, Subscription, Window, WindowAppearance};
use gpui_component::{Theme, ThemeMode};

use crate::settings::{ColorTheme, ThemeChoice};

/// Every colour the app uses. Pick one with [`palette`]; nothing else names a colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// The colour theme this palette belongs to.
    pub theme: ColorTheme,
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
    /// Lines a diff added and removed (gutter, counts, row washes): a colour-blind-safe pair.
    pub added: Hsla,
    pub removed: Hsla,
    /// A line the working tree changed, in the gutter.
    pub modified: Hsla,
    /// The title bar's and the status bar's grounds.
    pub title_bg: Hsla,
    pub status_bg: Hsla,
    /// Icon hues: blue, green, orange, red, purple, teal, yellow, pink. Grey when untinted.
    pub hues: [Hsla; 8],
    /// Tool window, activity bar and file icons in colour.
    pub tinted: bool,
    /// Accents give off a soft glow.
    pub glow: bool,
    /// How strongly the accent fills a selection.
    pub sel_mix: f32,
    /// Letters on a coloured tile or badge (a project's badge, a file type's tile).
    pub on_badge: Hsla,
    /// The dimming laid over the window behind a sheet or a drop.
    pub scrim: Hsla,
    /// The Spring accent: a soft leaf green.
    pub spring: Hsla,
    /// Ink line of the drawings.
    pub ill_ink: Hsla,
    pub ill_blob: Hsla,
    pub ill_paper: Hsla,
    pub ill_tint: Hsla,
    /// The drawings' one hue: a softer relative of `ok`.
    pub ill_accent: Hsla,
    pub ill_warm: Hsla,
    pub ill_clay: Hsla,
    syntax: [u32; 8],
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

/// WCAG relative luminance.
pub fn luminance(c: Hsla) -> f32 {
    let c: Rgba = c.into();
    let f = |x: f32| if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
    0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
}

/// WCAG contrast ratio of two opaque colours.
pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

impl Palette {
    /// `theme` in dark or light.
    pub fn new(theme: ColorTheme, dark: bool) -> Self {
        let b = palettes::base(theme, dark);
        let (fg, card, ok, warn, error) = (hex(b.fg), hex(b.card), hex(b.ok), hex(b.warn), hex(b.error));
        let panel = hex(b.panel);
        let hues = b.hues.map_or([hex(b.fg_dim); 8], |h| h.map(hex));
        let classic = theme == ColorTheme::Classic;
        Self {
            dark,
            theme,
            ground: hex(b.ground),
            panel,
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
            added: hex(b.added),
            removed: hex(b.removed),
            modified: hex(b.modified),
            title_bg: hex(b.title),
            status_bg: hex(b.status),
            hues,
            tinted: b.hues.is_some(),
            glow: theme == ColorTheme::Deep,
            sel_mix: b.sel_mix,
            on_badge: hex(0xffffff),
            scrim: if dark { hex(0x000000).opacity(0.5) } else { fg.opacity(0.26) },
            spring: if !classic {
                ok
            } else if dark {
                gpui::hsla(95. / 360., 0.38, 0.56, 1.)
            } else {
                gpui::hsla(95. / 360., 0.5, 0.36, 1.)
            },
            ill_ink: mix(fg, 0.88, panel),
            ill_blob: mix(fg, 0.05, panel),
            ill_paper: mix(fg, 0.06, card),
            ill_tint: mix(fg, 0.20, card),
            // The drawings' one hue: sage, as in Snout Studio's; the accent in Vivid and Deep.
            ill_accent: mix(
                if matches!(theme, ColorTheme::Vivid | ColorTheme::Deep) { hex(b.accent) } else { ok },
                0.70,
                card,
            ),
            ill_warm: mix(warn, 0.65, card),
            ill_clay: mix(mix(error, 0.5, warn), 0.55, card),
            syntax: b.syntax,
        }
    }

    /// The default theme, dark.
    pub fn dark() -> Self {
        Self::new(ColorTheme::default(), true)
    }

    /// The default theme, light.
    pub fn light() -> Self {
        Self::new(ColorTheme::default(), false)
    }

    /// Background of a line a diff added or removed.
    pub fn diff_add(&self) -> Hsla {
        mix(self.added, if self.dark { 0.15 } else { 0.14 }, self.panel)
    }

    pub fn diff_del(&self) -> Hsla {
        mix(self.removed, if self.dark { 0.15 } else { 0.14 }, self.panel)
    }

    /// An icon in hue `i` of [`Palette::hues`], or the dim grey when icons are untinted.
    pub fn hue(&self, i: usize) -> Hsla {
        if self.tinted { self.hues[i] } else { self.fg_dim }
    }

    /// The colour a branch takes: a chip, the current branch in the title bar.
    pub fn branch(&self) -> Hsla {
        if self.tinted { self.accent } else { self.ok }
    }

    /// The tab in front: a whisper of the accent in the coloured themes.
    pub fn tab_selected(&self) -> Hsla {
        if self.tinted { mix(self.accent, 0.07, self.panel) } else { self.ground }
    }

    /// Where the title bar's wash starts, on its left: the accent, faintly. `None` in Classic.
    pub fn title_wash(&self) -> Option<Hsla> {
        self.tinted.then(|| mix(self.accent, if self.dark { 0.20 } else { 0.16 }, self.title_bg))
    }

    /// A tile or badge of `color` that [`Palette::on_badge`] letters read on (WCAG AA).
    pub fn badge(&self, color: Hsla) -> Hsla {
        let mut c = color;
        while contrast(self.on_badge, c) < 4.6 {
            c = mix(c, 0.92, hex(0x000000));
        }
        c
    }

    /// A solid tile of `color` with letters on it that read (WCAG AA): `(ground, ink)`. Light
    /// letters on a deepened colour in light, dark letters on a lightened one in dark.
    pub fn tile(&self, color: Hsla) -> (Hsla, Hsla) {
        if !self.dark {
            return (self.badge(color), self.on_badge);
        }
        let mut c = color;
        while contrast(self.panel, c) < 4.6 {
            c = mix(c, 0.92, hex(0xffffff));
        }
        (c, self.panel)
    }

    /// A soft glow of `color` around an element, when the palette glows.
    pub fn glow_of(&self, color: Hsla) -> Vec<gpui::BoxShadow> {
        if !self.glow {
            return vec![];
        }
        vec![gpui::BoxShadow {
            color: color.opacity(if self.dark { 0.55 } else { 0.35 }),
            offset: gpui::point(gpui::px(0.), gpui::px(0.)),
            blur_radius: gpui::px(10.),
            spread_radius: gpui::px(0.),
            inset: false,
        }]
    }

    /// A soft wash of `color` for chips and badges.
    pub fn wash(&self, color: Hsla) -> Hsla {
        mix(color, 0.14, self.panel)
    }

    /// Text of lesser weight, between the text and the dim text, readable on every surface.
    pub fn weak(&self) -> Hsla {
        mix(self.fg, 0.35, self.fg_dim)
    }

    pub fn selection(&self) -> Hsla {
        mix(self.accent, self.sel_mix, self.panel)
    }

    /// The colours of Claude Code's edits, by what they mark.
    pub fn claude(&self) -> ClaudeColors {
        let mark = if self.dark { hex(CLAUDE_DARK) } else { hex(CLAUDE_LIGHT) };
        ClaudeColors {
            mark,
            mark_wash: self.wash(mark),
            added: self.added,
            added_band: self.added.opacity(if self.dark { 0.22 } else { 0.2 }),
            removed: self.removed,
            removed_band: self.removed.opacity(if self.dark { 0.20 } else { 0.14 }),
            removed_text: mix(self.removed, 0.85, self.fg),
            caret: mark,
            indicator: self.card,
        }
    }

    /// The colour of a note that informs: the cool blue the diffs add with, never a state.
    pub fn info(&self) -> Hsla {
        self.added
    }

    /// A destructive button, solid so it reads as one you can press: its fill, under the
    /// pointer, pressed, and its label (WCAG AA on all three).
    pub fn danger(&self) -> Pressable {
        let fill = self.badge(self.error);
        let black = hex(0x000000);
        Pressable { fill, hover: mix(fill, 0.86, black), pressed: mix(fill, 0.74, black), ink: self.on_badge }
    }

    /// The main button: the accent, deepened under the pointer and when pressed.
    pub fn primary(&self) -> Pressable {
        Pressable {
            fill: self.accent,
            hover: mix(self.accent, 0.88, self.fg),
            pressed: mix(self.accent, 0.78, self.fg),
            ink: self.on_accent,
        }
    }

    /// The ring round a control the keyboard is on.
    pub fn focus_ring(&self) -> Hsla {
        mix(self.accent, 0.75, self.card)
    }

    /// Claude's hue as text: on its wash, on cards and panels (WCAG AA).
    pub fn claude_ink(&self) -> Hsla {
        let mark = self.claude().mark;
        let toward = if self.dark { hex(0xffffff) } else { hex(0x000000) };
        let mut c = mark;
        while contrast(c, self.wash(mark)) < 4.6 || contrast(c, self.card) < 4.6 {
            c = mix(c, 0.94, toward);
        }
        c
    }

    /// The footer strip of a dialog: between the card and the ground.
    pub fn dialog_footer(&self) -> Hsla {
        mix(self.ground, 0.45, self.card)
    }

    /// Lane colours of the commit graph: the drawings' tones, then two quiet extras.
    pub fn lanes(&self) -> [Hsla; 6] {
        if self.tinted {
            let h = self.hues;
            return [h[1], h[2], h[0], h[3], h[4], self.fg_dim];
        }
        let (blue, violet) = if self.dark { (hex(0x7fa6d8), hex(0xb79ad6)) } else { (hex(0x3f6fa8), hex(0x7a55a8)) };
        let strong = |c: Hsla| if self.dark { c } else { mix(c, 0.7, self.fg) };
        [strong(self.ill_accent), strong(self.ill_warm), blue, strong(self.ill_clay), violet, self.fg_dim]
    }
}

/// A filled button's colours: its fill, under the pointer, pressed, and its label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pressable {
    pub fill: Hsla,
    pub hover: Hsla,
    pub pressed: Hsla,
    pub ink: Hsla,
}

/// Claude Code's own hue, for what it changed: a warm clay that no Git state uses.
const CLAUDE_DARK: u32 = 0xd97757;
const CLAUDE_LIGHT: u32 = 0xc15f3c;

/// The tokens of Claude Code's edits, as [`Palette::claude`] gives them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClaudeColors {
    /// The gutter mark of a line Claude changed (until reviewed), the tab's dot, the
    /// Claude Changes window's accents.
    pub mark: Hsla,
    /// A soft wash of `mark` for chips and the floating indicator's badge.
    pub mark_wash: Hsla,
    /// Lines added (counts, the fading band behind new text).
    pub added: Hsla,
    pub added_band: Hsla,
    /// Lines removed (counts, struck text while it goes).
    pub removed: Hsla,
    pub removed_band: Hsla,
    pub removed_text: Hsla,
    /// The point where text is typing itself in.
    pub caret: Hsla,
    /// The floating indicator's ground.
    pub indicator: Hsla,
}

/// The palette in use, and what chose it.
pub struct Current {
    pub palette: Palette,
    pub theme: ColorTheme,
    pub choice: ThemeChoice,
}

impl Global for Current {}

pub fn palette(cx: &App) -> Palette {
    cx.try_global::<Current>().map_or_else(Palette::dark, |c| c.palette)
}

fn is_dark_appearance(a: WindowAppearance) -> bool {
    matches!(a, WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

/// Whether `choice` means dark right now: the system's appearance, as `window` shows it
/// when there is one, for [`ThemeChoice::System`].
pub fn is_dark(choice: ThemeChoice, window: Option<&Window>, cx: &App) -> bool {
    match choice {
        ThemeChoice::Dark => true,
        ThemeChoice::Light => false,
        ThemeChoice::System => is_dark_appearance(window.map_or_else(|| cx.window_appearance(), Window::appearance)),
    }
}

/// Take the typefaces and sizes the settings choose.
pub fn set_fonts(settings: &crate::settings::Settings, cx: &mut App) {
    let size = settings.ui_font_size.clamp(10., 20.);
    cx.set_global(crate::look::Fonts {
        ui: settings.ui_font.clone().into(),
        ui_size: size,
        mono: settings.editor_font.clone().into(),
    });
}

/// Keep `window`'s app on the system's light or dark while the settings say System: when
/// the Mac switches, the palette switches with it, live.
pub fn follow_system(window: &Window) -> Subscription {
    window.observe_window_appearance(|window, cx| system_changed(window.appearance(), Some(window), cx))
}

/// The Mac turned to `appearance`: with the theme on System, the palette follows.
pub fn system_changed(appearance: WindowAppearance, window: Option<&mut Window>, cx: &mut App) {
    if let Some(c) = cx.try_global::<Current>()
        && c.choice == ThemeChoice::System
    {
        let theme = c.theme;
        lay(theme, ThemeChoice::System, is_dark_appearance(appearance), window, cx);
    }
}

/// Follow `theme` in the light or dark `choice` says: pick the palette and lay it onto GPUI
/// Kit's theme. A forced Light or Dark also sets the windows' own frames to match; System
/// lets them follow the Mac again.
pub fn apply(theme: ColorTheme, choice: ThemeChoice, window: Option<&mut Window>, cx: &mut App) {
    let forced = match choice {
        ThemeChoice::System => None,
        ThemeChoice::Dark => Some(WindowAppearance::Dark),
        ThemeChoice::Light => Some(WindowAppearance::Light),
    };
    if cx.try_global::<Current>().map(|c| c.choice) != Some(choice) {
        cx.set_window_appearance(forced);
    }
    let dark = is_dark(choice, window.as_deref(), cx);
    lay(theme, choice, dark, window, cx);
}

fn lay(theme: ColorTheme, choice: ThemeChoice, dark: bool, window: Option<&mut Window>, cx: &mut App) {
    let p = Palette::new(theme, dark);
    cx.set_global(Current { palette: p, theme, choice });
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, window, cx);
    let (ui, mono, size) = (crate::look::ui_font(cx), crate::look::mono_font(cx), crate::look::ui_size(cx));
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
        t.title_bar = p.title_bg;
        t.title_bar_border = p.border;
        t.status_bar = p.status_bg;
        t.status_bar_border = p.border;
        t.link = p.accent;
        t.window_border = p.border;
        t.font_family = ui;
        t.mono_font_family = mono;
        t.font_size = gpui::px(size);
        t.radius = gpui::px(5.);
        t.radius_lg = gpui::px(8.);
        t.highlight_theme = std::sync::Arc::new(syntax(&p));
    });
    cx.refresh_windows();
}

/// The editor's colours: the theme's tones for the code, and its gutter and current line.
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
        "name": format!("SnoutCode {} {}", p.theme.label(), if p.dark { "Dark" } else { "Light" }),
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
    let c = p.syntax.map(hex);
    SyntaxColors {
        keyword: c[0],
        ty: c[1],
        function: c[2],
        string: c[3],
        number: c[4],
        comment: c[5],
        attribute: c[6],
        property: c[7],
        punctuation: mix(p.fg, 0.78, p.panel),
        gutter: readable(p.fg_dim, p.panel),
    }
}

/// `color` moved toward the text colour of `bg` until it reads on it (WCAG AA): the line
/// numbers, as quiet as they may be.
fn readable(color: Hsla, bg: Hsla) -> Hsla {
    let toward = if luminance(bg) < 0.2 { hex(0xffffff) } else { hex(0x000000) };
    let mut c = mix(color, 0.8, bg);
    while contrast(c, bg) < 4.6 {
        c = mix(c, 0.94, toward);
    }
    c
}

/// The band behind the caret's line in the editor.
pub fn active_line(p: &Palette) -> Hsla {
    mix(p.fg, if p.dark { 0.05 } else { 0.045 }, p.panel)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every theme in light and dark.
    fn all() -> impl Iterator<Item = Palette> {
        ColorTheme::ALL.into_iter().flat_map(|t| [Palette::new(t, true), Palette::new(t, false)])
    }

    fn name(p: &Palette) -> String {
        format!("{} {}", p.theme.label(), if p.dark { "dark" } else { "light" })
    }

    #[test]
    fn text_meets_wcag_aa_on_every_surface_it_is_drawn_on() {
        const AA: f32 = 4.5;
        let mut bad = Vec::new();
        let mut need = |what: &str, fg: Hsla, bg: Hsla, min: f32, p: &Palette| {
            let c = contrast(fg, bg);
            if c < min {
                bad.push(format!("{}: {what} {c:.2} < {min}", name(p)));
            }
        };
        for p in all() {
            let surfaces = [
                ("panel", p.panel),
                ("ground", p.ground),
                ("card", p.card),
                ("input", p.input),
                ("stripe", p.stripe),
                ("title bar", p.title_bg),
                ("status bar", p.status_bg),
            ];
            for (s, bg) in surfaces {
                for (t, fg) in [("fg", p.fg), ("fg_dim", p.fg_dim), ("weak", p.weak())] {
                    need(&format!("{t} on {s}"), fg, bg, AA, &p);
                }
                for (t, fg) in [
                    ("accent", p.accent),
                    ("ok", p.ok),
                    ("warn", p.warn),
                    ("error", p.error),
                    ("added", p.added),
                    ("removed", p.removed),
                ] {
                    need(&format!("{t} on {s}"), fg, bg, AA, &p);
                }
            }
            for (s, bg) in [
                ("button", p.button),
                ("button hover", p.button_hover),
                ("selection", p.selection()),
                ("active", p.active),
                ("added line", p.diff_add()),
                ("removed line", p.diff_del()),
                ("current line", active_line(&p)),
                ("tab in front", p.tab_selected()),
            ] {
                need(&format!("fg on {s}"), p.fg, bg, AA, &p);
                need(&format!("fg_dim on {s}"), p.fg_dim, bg, AA, &p);
            }
            for (t, c) in
                [("accent", p.accent), ("ok", p.ok), ("warn", p.warn), ("error", p.error), ("branch", p.branch())]
            {
                need(&format!("{t} chip"), c, p.wash(c), AA, &p);
            }
            need("on_accent on accent", p.on_accent, p.accent, AA, &p);
            for (what, b) in [("primary", p.primary()), ("danger", p.danger())] {
                for (state, bg) in [("", b.fill), (" hover", b.hover), (" pressed", b.pressed)] {
                    need(&format!("{what} button{state}"), b.ink, bg, AA, &p);
                }
            }
            for (t, c) in [("info", p.info()), ("Claude", p.claude_ink())] {
                need(&format!("{t} chip"), c, p.wash(c), AA, &p);
            }
            let footer = p.dialog_footer();
            need("fg_dim on a dialog's footer", p.fg_dim, footer, AA, &p);
            need("error on a dialog's footer", p.error, footer, AA, &p);
            for (t, c) in [("accent", p.accent), ("ok", p.ok), ("warn", p.warn), ("error", p.error), ("info", p.info())]
            {
                need(&format!("{t} callout text"), p.fg, mix(c, 0.14, p.card), AA, &p);
                need(&format!("{t} callout icon"), c, mix(c, 0.14, p.card), 3.0, &p);
            }
            need("the focus ring", p.focus_ring(), p.card, 3.0, &p);
            need("Spring's hints", p.spring, p.panel, if p.theme == ColorTheme::Classic { 3.0 } else { AA }, &p);
            let s = syntax_colors(&p);
            for (t, c) in [
                ("keyword", s.keyword),
                ("type", s.ty),
                ("function", s.function),
                ("string", s.string),
                ("number", s.number),
                ("comment", s.comment),
                ("attribute", s.attribute),
                ("property", s.property),
                ("punctuation", s.punctuation),
            ] {
                need(&format!("{t} on the editor"), c, p.panel, AA, &p);
                need(&format!("{t} on the current line"), c, active_line(&p), AA, &p);
            }
            need("a line number", s.gutter, p.panel, AA, &p);
            for c in p.hues {
                let (tile, ink) = p.tile(c);
                need("a tile's letter", ink, tile, AA, &p);
                need("a badge's letter", p.on_badge, p.badge(c), AA, &p);
                // Icons: 3:1, as WCAG asks of graphics.
                need("an icon on the ground", c, p.ground, 3.0, &p);
                need("an icon on the panel", c, p.panel, 3.0, &p);
            }
            let claude = p.claude();
            need("Claude's mark", claude.mark, p.panel, 3.0, &p);
            need("removed text while it goes", claude.removed_text, p.panel, AA, &p);
        }
        assert!(bad.is_empty(), "below WCAG AA:\n{}", bad.join("\n"));
    }

    /// Added and removed are a blue and an orange: they stay apart along the blue/yellow axis
    /// that protanopia and deuteranopia keep. Errors and warnings differ in lightness too.
    #[test]
    fn diff_and_problem_colours_are_colour_blind_safe() {
        for p in all() {
            let hue = |c: Hsla| c.h * 360.;
            assert!((180. ..=230.).contains(&hue(p.added)), "{}: added is not blue: {}", name(&p), hue(p.added));
            assert!(
                hue(p.removed) <= 40. || hue(p.removed) >= 350.,
                "{}: removed is not orange: {}",
                name(&p),
                hue(p.removed)
            );
            assert!(contrast(p.error, p.warn) >= 1.25, "{}: error and warning too alike in lightness", name(&p));
        }
    }

    #[gpui::test]
    fn on_system_the_palette_follows_the_mac_live(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            apply(ColorTheme::Studio, ThemeChoice::System, None, cx);
            system_changed(WindowAppearance::Dark, None, cx);
            assert!(palette(cx).dark, "the Mac turned dark");
            assert_eq!(palette(cx), Palette::new(ColorTheme::Studio, true));
            system_changed(WindowAppearance::Light, None, cx);
            assert!(!palette(cx).dark, "and light again");
            apply(ColorTheme::Deep, ThemeChoice::Dark, None, cx);
            system_changed(WindowAppearance::Light, None, cx);
            assert_eq!(palette(cx), Palette::new(ColorTheme::Deep, true), "Dark in the settings stays dark");
        });
    }

    #[test]
    fn studio_is_the_default_and_is_warm() {
        let p = Palette::new(ColorTheme::default(), false);
        assert_eq!(p.theme, ColorTheme::Studio);
        let ground: Rgba = p.panel.into();
        assert!(ground.r > ground.b, "warm paper, not a cool grey");
    }
}
