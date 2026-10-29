//! The colour themes, each a dark and a light set of base colours and the code's tones.
//! Everything else in [`super::Palette`] is derived from these. The tests in `theme.rs`
//! hold every text colour to WCAG AA on the surfaces it is drawn on.
//!
//! Lines added and removed are a blue and an orange (or brick) that differ along the
//! blue/yellow axis the common colour blindnesses keep; errors and warnings differ in
//! lightness as well as hue, and carry their own icons.

use crate::settings::ColorTheme;

pub(super) struct Base {
    /// The chrome: title bar, stripes, tool window headers.
    pub ground: u32,
    /// The panes and the editor.
    pub panel: u32,
    pub card: u32,
    pub input: u32,
    pub stripe: u32,
    pub button: u32,
    pub button_hover: u32,
    pub active: u32,
    pub border: u32,
    pub fg: u32,
    pub fg_dim: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub ok: u32,
    pub warn: u32,
    pub error: u32,
    pub added: u32,
    pub removed: u32,
    /// A line the working tree changed, in the gutter.
    pub modified: u32,
    pub title: u32,
    pub status: u32,
    /// Icon hues: blue, green, orange, red, purple, teal, yellow, pink. `None`: icons stay grey.
    pub hues: Option<[u32; 8]>,
    /// How strongly the accent fills a selection.
    pub sel_mix: f32,
    /// keyword, type, function, string, number, comment, attribute, property.
    pub syntax: [u32; 8],
}

/// Studio, after dark: Snout Studio's warm dark. Walnut grounds, the editor a step darker
/// than the chrome, an amber accent; Studio's gold, teal and clay for the code.
const STUDIO_DARK: Base = Base {
    ground: 0x27221c,
    panel: 0x1f1b16,
    card: 0x2b251d,
    input: 0x191510,
    stripe: 0x231e18,
    button: 0x352e24,
    button_hover: 0x3d3529,
    active: 0x46331d,
    border: 0x3a3228,
    fg: 0xe6dccb,
    fg_dim: 0xb0a38c,
    accent: 0xe08c3e,
    on_accent: 0x1f1206,
    ok: 0x9ad17e,
    warn: 0xe0b34a,
    error: 0xf47c7c,
    added: 0x6fb3d6,
    removed: 0xe57d62,
    modified: 0xe0b34a,
    title: 0x2a241e,
    status: 0x27221c,
    hues: Some([0x86a8c8, 0x9ad17e, 0xe08c3e, 0xe57d62, 0xc6a0d0, 0x7fb9a6, 0xe0b34a, 0xe79aa8]),
    sel_mix: 0.22,
    syntax: [0xe0b256, 0x7fb9a6, 0x9fb7d8, 0xcf9472, 0xb8c98a, 0xa09482, 0xe79aa8, 0xc6a8d6],
};

/// Studio: Snout Studio's warm light. Cream paper, a terracotta accent, sage, ochre and clay.
const STUDIO_LIGHT: Base = Base {
    ground: 0xf1ebdd,
    panel: 0xf9f5ec,
    card: 0xfdfaf3,
    input: 0xfffcf6,
    stripe: 0xf4eee2,
    button: 0xe7ddc7,
    button_hover: 0xddd0b4,
    active: 0xf1e2c4,
    border: 0xe2d7c0,
    fg: 0x43382a,
    fg_dim: 0x655743,
    accent: 0xa14913,
    on_accent: 0xffffff,
    ok: 0x3c6c32,
    warn: 0x855a00,
    error: 0x951d37,
    added: 0x1d6a91,
    removed: 0xae3a26,
    modified: 0xb07a12,
    title: 0xece4d4,
    status: 0xf1ebdd,
    hues: Some([0x3f6f9e, 0x4b7f3a, 0xbf5a1a, 0xb5432f, 0x8a5a96, 0x2f7f72, 0x96690a, 0xb04f6c]),
    sel_mix: 0.16,
    syntax: [0x845500, 0x2d6b5f, 0x34598a, 0x9a4a24, 0x46702a, 0x6e624e, 0xa3385f, 0x6e4a8a],
};

/// Vivid: neutral greys, an electric violet accent, colour everywhere it carries meaning.
const VIVID_DARK: Base = Base {
    ground: 0x2b2d30,
    panel: 0x1e1f22,
    card: 0x2f3135,
    input: 0x1a1b1e,
    stripe: 0x26282b,
    button: 0x393b40,
    button_hover: 0x3f4248,
    active: 0x3d2f6e,
    border: 0x3c3f45,
    fg: 0xe8eaee,
    fg_dim: 0xabafb8,
    accent: 0xb39bff,
    on_accent: 0x170b3d,
    ok: 0x5fd08a,
    warn: 0xf5b83d,
    error: 0xff8a92,
    added: 0x3fb5f0,
    removed: 0xff9147,
    modified: 0xb39bff,
    title: 0x2b2d30,
    status: 0x2b2d30,
    hues: Some([0x5ea8ff, 0x5fd08a, 0xff9d4d, 0xff8a92, 0xb48cff, 0x3fd0c9, 0xf5c84b, 0xff7ac0]),
    sel_mix: 0.22,
    syntax: [0xff7ab8, 0x6fd6f7, 0x7fb4ff, 0xb5e48c, 0xc4a7ff, 0x959ba6, 0xffb05c, 0xf0a7e4],
};

const VIVID_LIGHT: Base = Base {
    ground: 0xf2f3f6,
    panel: 0xffffff,
    card: 0xffffff,
    input: 0xffffff,
    stripe: 0xf7f8fa,
    button: 0xe9eaf0,
    button_hover: 0xe0e2ea,
    active: 0xe6defd,
    border: 0xd9dbe2,
    fg: 0x1b1c21,
    fg_dim: 0x575b65,
    accent: 0x6a3fe0,
    on_accent: 0xffffff,
    ok: 0x18733f,
    warn: 0x875600,
    error: 0x941844,
    added: 0x0a66a3,
    removed: 0xa84400,
    modified: 0x6a3fe0,
    title: 0xf2f3f6,
    status: 0xf2f3f6,
    hues: Some([0x2f6fe0, 0x1f9a52, 0xcf650a, 0xd8344a, 0x7b4fe0, 0x0f968f, 0xae8200, 0xd13d8c]),
    sel_mix: 0.14,
    syntax: [0xb8166a, 0x0a6d8f, 0x2554c7, 0x2f7a12, 0x6a3fe0, 0x666b74, 0xa04e00, 0x8a2fa6],
};

/// Deep: midnight indigo grounds, an aqua accent that glows, neon code.
const DEEP_DARK: Base = Base {
    ground: 0x181a3f,
    panel: 0x10112b,
    card: 0x1f2150,
    input: 0x0c0d24,
    stripe: 0x15173a,
    button: 0x262a63,
    button_hover: 0x2d3170,
    active: 0x173f5a,
    border: 0x2b2e66,
    fg: 0xe9ebff,
    fg_dim: 0xa2a7dc,
    accent: 0x2ee8d6,
    on_accent: 0x032421,
    ok: 0x6ff0a0,
    warn: 0xffcb5c,
    error: 0xff7a9f,
    added: 0x4fc3ff,
    removed: 0xff9a3d,
    modified: 0xb18cff,
    title: 0x13143a,
    status: 0x13143a,
    hues: Some([0x6aa8ff, 0x6ff0a0, 0xffa95a, 0xff5f8f, 0xb18cff, 0x2ee8d6, 0xffd760, 0xff79d0]),
    sel_mix: 0.22,
    syntax: [0xff6ac8, 0x7af0ff, 0x8fb0ff, 0xb9f27c, 0xffb86c, 0x7f86c4, 0xffd96a, 0xc7a8ff],
};

const DEEP_LIGHT: Base = Base {
    ground: 0xe2e2f8,
    panel: 0xf6f6ff,
    card: 0xffffff,
    input: 0xffffff,
    stripe: 0xeeeefc,
    button: 0xdcddf6,
    button_hover: 0xd2d4f2,
    active: 0xc9eeec,
    border: 0xcacbec,
    fg: 0x15163a,
    fg_dim: 0x51548a,
    accent: 0x006e76,
    on_accent: 0xffffff,
    ok: 0x136c36,
    warn: 0x875800,
    error: 0x990d3d,
    added: 0x0a64a8,
    removed: 0xa14300,
    modified: 0x7442d6,
    title: 0xdedff7,
    status: 0xdedff7,
    hues: Some([0x3157d6, 0x178a48, 0xc65e0a, 0xd02060, 0x7442d6, 0x008c8f, 0x9c7700, 0xc4309a]),
    sel_mix: 0.15,
    syntax: [0xb0127a, 0x0b6880, 0x2f4fd0, 0x3a7612, 0xa04a00, 0x5f6396, 0x8a5e00, 0x6a2fc0],
};

/// Classic, dark: the earlier neutral greys and blue accent, the chrome a step lighter than
/// the editor. Raised to WCAG AA where it fell short, and its diff colours made safe.
const CLASSIC_DARK: Base = Base {
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
    fg_dim: 0xb0b4ba,
    accent: 0x6199f5,
    on_accent: 0x0d1b33,
    ok: 0x5cb46a,
    warn: 0xdba43f,
    error: 0xed716a,
    added: 0x4a9fe0,
    removed: 0xe08a3c,
    modified: 0x6199f5,
    title: 0x2a2c30,
    status: 0x2a2c30,
    hues: None,
    sel_mix: 0.22,
    syntax: [0xd29b74, 0xc9cdd4, 0x62aaee, 0x78b383, 0x44b0bd, 0x9094a0, 0xbcb46e, 0xc58bc0],
};

/// Classic, light: white editor, a pale grey chrome.
const CLASSIC_LIGHT: Base = Base {
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
    fg_dim: 0x5d6169,
    accent: 0x3062cc,
    on_accent: 0xffffff,
    ok: 0x24773a,
    warn: 0x8f5c00,
    error: 0x932a24,
    added: 0x1e6bb1,
    removed: 0xa54e00,
    modified: 0xa66b00,
    title: 0xf3f4f6,
    status: 0xf3f4f6,
    hues: None,
    sel_mix: 0.12,
    syntax: [0x1d3fa6, 0x23252b, 0x0d6680, 0x247a35, 0x2556d8, 0x6a6d75, 0x7a6a0c, 0x7d2391],
};

pub(super) fn base(theme: ColorTheme, dark: bool) -> &'static Base {
    match (theme, dark) {
        (ColorTheme::Studio, true) => &STUDIO_DARK,
        (ColorTheme::Studio, false) => &STUDIO_LIGHT,
        (ColorTheme::Vivid, true) => &VIVID_DARK,
        (ColorTheme::Vivid, false) => &VIVID_LIGHT,
        (ColorTheme::Deep, true) => &DEEP_DARK,
        (ColorTheme::Deep, false) => &DEEP_LIGHT,
        (ColorTheme::Classic, true) => &CLASSIC_DARK,
        (ColorTheme::Classic, false) => &CLASSIC_LIGHT,
    }
}
