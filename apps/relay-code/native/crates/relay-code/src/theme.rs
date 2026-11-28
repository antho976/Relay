//! Relay Code's look (design 44, D13): Claude's app in the house style. A warm near-black ground,
//! off-white ink, one coral accent for send and the one primary key, state colours only for state.
//! Every colour on screen comes from here.

use crate::draw::Rgba;

pub const BG: Rgba = Rgba::hex(0x1F1E1D);
/// The drawer and sheets.
pub const PANEL: Rgba = Rgba::hex(0x181716);
/// Surfaces you can tap, from the quietest up.
pub const S1: Rgba = Rgba::hex(0x2A2927);
pub const S2: Rgba = Rgba::hex(0x353431);
pub const S3: Rgba = Rgba::hex(0x403F3B);
/// Code blocks.
pub const CODE: Rgba = Rgba::hex(0x141312);
pub const LINE: Rgba = Rgba::argb(0x17FFFFFF);
pub const INK: Rgba = Rgba::hex(0xF5F4EF);
pub const INK2: Rgba = Rgba::hex(0xB5B0A6);
pub const INK3: Rgba = Rgba::hex(0x9E998F);
/// The one accent: what asks for you (send, the primary key, the caret).
pub const ACCENT: Rgba = Rgba::hex(0xD97757);
pub const ON_ACCENT: Rgba = Rgba::hex(0x1F1E1C);
pub const ACCENT_DIM: Rgba = Rgba::argb(0x2BD97757);
pub const OK: Rgba = Rgba::hex(0x8CC08F);
pub const WARN: Rgba = Rgba::hex(0xE5B567);
pub const DANGER: Rgba = Rgba::hex(0xEC8574);
pub const GLASS: Rgba = Rgba::argb(0xF2181716);
pub const SCRIM: Rgba = Rgba::argb(0x8C000000);
/// A press over anything.
pub const WASH: Rgba = Rgba::argb(0x14FFFFFF);

/// Agents' badges: a colour each, picked from the name, as the PC and the old app do.
pub const BADGES: [Rgba; 6] = [Rgba::hex(0x6FC2B4), Rgba::hex(0xB9A3FF), Rgba::hex(0xE5B567), Rgba::hex(0xEC8FA8), Rgba::hex(0x7FA8E8), Rgba::hex(0x9CCB7A)];

/// The type scale, in layout units (1/390 of the phone's width).
pub mod ty {
    pub const TITLE: f32 = 24.0;
    pub const SECTION: f32 = 17.0;
    pub const BODY: f32 = 15.0;
    /// The agent's words, in the serif.
    pub const READ: f32 = 16.0;
    pub const UI: f32 = 14.0;
    pub const CAPTION: f32 = 13.0;
    pub const SMALL: f32 = 12.0;
    pub const MONO: f32 = 13.0;
}

/// The page's side margin.
pub const PAD: f32 = 16.0;
/// Keys' corners.
pub const KEY_R: f32 = 10.0;
