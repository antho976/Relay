//! The premium look (DESIGN.md, "Premium, dark and light"): a plain black or white ground, warm
//! grey cards with hairline edges, one brass accent, and colour only where it is data.
//!
//! Two palettes, Light and Dark, and "System" that follows the phone. Category colours have a
//! lighter shade for Dark and a deeper one for Light, so marks stay readable on both.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::draw::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub name: &'static str,
    /// The tab bar and an editor's footer: the ground, nearly opaque.
    pub console: Rgba,
    /// Cards.
    pub slab: Rgba,
    /// Keys, chips and the second level inside a card.
    pub card2: Rgba,
    /// Pressed and selected rows.
    pub wash: Rgba,
    pub ink: Rgba,
    pub ink2: Rgba,
    pub ink3: Rgba,
    /// The ground. All three are the same: the look has no gradient behind it.
    pub ground: [Rgba; 3],
    pub light: bool,
    /// Sheets and fields: solid.
    pub raised: Rgba,
    /// A meter's empty track.
    pub track: Rgba,
    /// Card edges and dividers.
    pub line_subtle: Rgba,
    pub live: Rgba,
    pub held: Rgba,
    pub waiting: Rgba,
    pub held_text: Rgba,
    pub live_text: Rgba,
    pub waiting_text: Rgba,
    /// The one accent, brass: what asks for you, today's mark, the focused field.
    pub signal: Rgba,
    /// The primary key: ink filled.
    pub prim: Rgba,
    /// Text on the primary key.
    pub on_ink: Rgba,
}

pub const DARK: Palette = Palette {
    name: "dark",
    console: Rgba::hex(0x0A0A0A),
    slab: Rgba::hex(0x131313),
    card2: Rgba::hex(0x1D1D1D),
    wash: Rgba::argb(0x14FFFFFF),
    ink: Rgba::hex(0xF5F3EF),
    ink2: Rgba::hex(0xA29E97),
    ink3: Rgba::hex(0x8C877F),
    ground: [Rgba::hex(0x000000); 3],
    light: false,
    raised: Rgba::hex(0x161616),
    track: Rgba::argb(0x1CFFFFFF),
    line_subtle: Rgba::argb(0x14FFFFFF),
    live: Rgba::hex(0x7FCB9F),
    held: Rgba::hex(0xF08F7E),
    waiting: Rgba::hex(0xE8C26A),
    held_text: Rgba::hex(0xF08F7E),
    live_text: Rgba::hex(0x7FCB9F),
    waiting_text: Rgba::hex(0xE8C26A),
    signal: Rgba::hex(0xD8B67C),
    prim: Rgba::hex(0xF5F3EF),
    on_ink: Rgba::hex(0x0A0A0A),
};

pub const LIGHT: Palette = Palette {
    name: "light",
    console: Rgba::WHITE,
    slab: Rgba::hex(0xF5F4F1),
    card2: Rgba::hex(0xECEAE5),
    wash: Rgba::argb(0x0F000000),
    ink: Rgba::hex(0x131211),
    ink2: Rgba::hex(0x5E5A54),
    ink3: Rgba::hex(0x6C6760),
    ground: [Rgba::hex(0xFFFFFF); 3],
    light: true,
    raised: Rgba::hex(0xFFFFFF),
    track: Rgba::argb(0x0F000000),
    line_subtle: Rgba::argb(0x0F000000),
    live: Rgba::hex(0x1E7A4B),
    held: Rgba::hex(0xBF4336),
    waiting: Rgba::hex(0x855C00),
    held_text: Rgba::hex(0xBF4336),
    live_text: Rgba::hex(0x1E7A4B),
    waiting_text: Rgba::hex(0x855C00),
    signal: Rgba::hex(0x8A6634),
    prim: Rgba::hex(0x131211),
    on_ink: Rgba::WHITE,
};

/// Words and marks on a picture card: the pictures are light on Light and dark on Dark.
pub mod pic {
    use crate::draw::Rgba;
    pub const INK_ON_LIGHT: Rgba = Rgba::hex(0x141414);
    pub const INK2_ON_LIGHT: Rgba = Rgba::hex(0x4A443C);
    pub const INK_ON_DARK: Rgba = Rgba::hex(0xF5F3EF);
    pub const INK2_ON_DARK: Rgba = Rgba::hex(0xB9B3AA);
}

/// Hues that mean something, kept for screens that name them. On this look they are the category
/// colours' Dark shades; prefer `category_color` and the palette's state colours.
pub mod hue {
    use crate::draw::Rgba;
    pub const VIOLET: Rgba = Rgba::hex(0xB998EC);
    pub const MINT: Rgba = Rgba::hex(0x7FCB9F);
    pub const CORAL: Rgba = Rgba::hex(0xF08F7E);
    pub const SKY: Rgba = Rgba::hex(0x7FAEEB);
    pub const SUN: Rgba = Rgba::hex(0xE8C96A);
    pub const PINK: Rgba = Rgba::hex(0xEE8FAE);
    pub const TEAL: Rgba = Rgba::hex(0x6CCBC8);
    pub const LIME: Rgba = Rgba::hex(0xA9CF7E);
}

static DARK_NOW: AtomicBool = AtomicBool::new(true);

/// Which shades `category_color` and friends hand out: set whenever the palette changes.
pub fn set_dark(dark: bool) {
    DARK_NOW.store(dark, Ordering::Relaxed);
}

fn dark_now() -> bool {
    DARK_NOW.load(Ordering::Relaxed)
}

/// A soft tint of a hue for a badge's or tile's ground.
pub fn soft_of(hue: Rgba) -> Rgba {
    if dark_now() { hue.a(0.19) } else { hue.a(0.14) }
}

/// Words on a soft tint: the ink, which reads on both.
pub fn deep_of(hue: Rgba) -> Rgba {
    if dark_now() { DARK.ink } else { hue.mix(LIGHT.ink, 0.35) }
}

/// The category colours, in the ledger's order (a category stores its index): (Dark, Light).
pub const CATEGORY_PAIRS: [(Rgba, Rgba); 12] = [
    (Rgba::hex(0x86C58A), Rgba::hex(0x3E8A47)), // green: groceries
    (Rgba::hex(0x6CCBC8), Rgba::hex(0x20817E)), // teal: health
    (Rgba::hex(0x7FAEEB), Rgba::hex(0x2F68B5)), // blue: transport
    (Rgba::hex(0xB998EC), Rgba::hex(0x7350B5)), // violet: home
    (Rgba::hex(0xD69BDF), Rgba::hex(0x9C4688)), // orchid: gifts
    (Rgba::hex(0xEE8FAE), Rgba::hex(0xB4456C)), // pink: fun
    (Rgba::hex(0xEFA36B), Rgba::hex(0xB4612A)), // orange: dining
    (Rgba::hex(0xE8C96A), Rgba::hex(0x94740F)), // gold: bills
    (Rgba::hex(0xB5C17A), Rgba::hex(0x6B7A1F)), // olive
    (Rgba::hex(0xD2A982), Rgba::hex(0x875A30)), // tan: coffee
    (Rgba::hex(0xA7B4C8), Rgba::hex(0x56627A)), // slate: other
    (Rgba::hex(0xE69A82), Rgba::hex(0xAD5539)), // brick: shopping
];

/// The category colours for the palette in use.
pub fn category_colors() -> [Rgba; 12] {
    let d = dark_now();
    CATEGORY_PAIRS.map(|(dk, lt)| if d { dk } else { lt })
}

pub fn category_color(index: Option<i64>) -> Rgba {
    let n = CATEGORY_PAIRS.len() as i64;
    let (dk, lt) = CATEGORY_PAIRS[(index.unwrap_or(10).rem_euclid(n)) as usize];
    if dark_now() { dk } else { lt }
}

/// A date badge's hue: on this look a date is not data, so every date wears the same quiet card.
pub fn date_hue(_i: usize) -> Rgba {
    if dark_now() { DARK.ink3 } else { LIGHT.ink3 }
}

/// What Appearance offers, in order.
pub const MODES: [(&str, &str); 3] = [("system", "System"), ("light", "Light"), ("dark", "Dark")];

/// The palette for a stored appearance ("system", "light", "dark"; older names from the sky look
/// map onto these) and whether the phone is on dark.
pub fn for_mode(mode: &str, system_dark: bool) -> Palette {
    match mode {
        "light" | "sky" | "dusk" | "mint" => LIGHT,
        "dark" | "night" => DARK,
        _ => {
            if system_dark {
                DARK
            } else {
                LIGHT
            }
        }
    }
}

/// Kept for callers that name a palette: "light" or "dark", anything else Dark.
pub fn by_name(name: &str) -> Palette {
    if name == "light" { LIGHT } else { DARK }
}

/// Both palettes, Light first.
pub const ALL: [Palette; 2] = [LIGHT, DARK];
