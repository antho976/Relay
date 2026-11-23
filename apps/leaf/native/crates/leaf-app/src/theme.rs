//! Leaf's look (Relay design 42, "Leaf · reading app"): near-black, one green accent, and the
//! covers bring the colour. One palette; every colour on screen comes from here.

use crate::draw::Rgba;

/// The ground.
pub const BG: Rgba = Rgba::hex(0x0D0D0E);
/// Surfaces, from the quietest up: a field, a key, a pressed row.
pub const S1: Rgba = Rgba::hex(0x161618);
pub const S2: Rgba = Rgba::hex(0x1E1E21);
pub const S3: Rgba = Rgba::hex(0x2A2A2E);
/// Hairlines that carry meaning (a field's edge, a divider in a sheet).
pub const LINE: Rgba = Rgba::argb(0x14FFFFFF);
pub const INK: Rgba = Rgba::hex(0xEEECE7);
pub const INK2: Rgba = Rgba::hex(0xA8A49C);
pub const INK3: Rgba = Rgba::hex(0x8E8A82);
/// The one accent: what asks for you (the primary key, progress, the caret).
pub const ACCENT: Rgba = Rgba::hex(0x3FDD8C);
/// Words on the accent.
pub const ON_ACCENT: Rgba = Rgba::hex(0x062112);
/// The accent as a wash behind a selected thing.
pub const ACCENT_DIM: Rgba = Rgba::argb(0x243FDD8C);
pub const WARN: Rgba = Rgba::hex(0xF2B250);
pub const DANGER: Rgba = Rgba::hex(0xF2706A);
/// What floats over a page (the reader's bars, a menu): nearly opaque.
pub const GLASS: Rgba = Rgba::argb(0xE6161618);
/// The veil behind a dialog.
pub const SCRIM: Rgba = Rgba::argb(0xA6000000);
/// A press over anything.
pub const WASH: Rgba = Rgba::argb(0x14FFFFFF);

/// The type scale, in layout units (1/390 of the phone's width).
pub mod ty {
    /// Page titles, in Sora.
    pub const TITLE: f32 = 26.0;
    pub const HEADING: f32 = 20.0;
    pub const SECTION: f32 = 17.0;
    pub const BODY: f32 = 15.0;
    pub const UI: f32 = 14.0;
    pub const CAPTION: f32 = 12.5;
    pub const SMALL: f32 = 11.5;
}

/// The page's side margin.
pub const PAD: f32 = 16.0;
/// Keys' corners.
pub const KEY_R: f32 = 10.0;
/// Covers' corners.
pub const COVER_R: f32 = 8.0;
