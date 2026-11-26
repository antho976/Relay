//! A cover looked at full screen (the owner, 2026-11-25: "sometimes I like to watch the comic
//! art"): a tap on the series page's cover opens it on black at its own size (never past the
//! screen), and from there two fingers zoom in as far as [`MAX_Z`], a double tap zooms in on that
//! spot or back out, one finger moves it around once it is bigger than the screen, and a pull down
//! at its own size (or ✕, or Back) puts it away.
//!
//! The picture is the cover's own file, decoded at its own size ([`Images::full`]); the small
//! copy stands in until it comes.

use crate::draw::{Rgba, WIDTH};
use crate::images::{CoverOf, Images};
use crate::kit;
use crate::theme as t;

/// How far two fingers zoom in, past the picture's own size on screen.
pub const MAX_Z: f32 = 5.0;
/// A double tap zooms to this.
const TAP_Z: f32 = 2.5;
/// Two taps this close in time are a double tap.
const DOUBLE_NS: u64 = 300_000_000;
/// A pull down this far, at its own size, puts it away (units).
const CLOSE_PULL: f32 = 120.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Zoom {
    pub of: CoverOf,
    /// How far zoomed in: 1 is its own size (or the screen's, if smaller).
    pub z: f32,
    /// Where its middle is, from the screen's middle (units).
    pub off: (f32, f32),
    /// A pull down at its own size, toward putting it away (units).
    pub pull: f32,
    /// A saved picture was removed from Leaf (its trash key): the caller lets it go.
    pub removed: bool,
    last: Option<Fingers>,
    last_tap: Option<(u64, (f32, f32))>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fingers {
    One((f32, f32)),
    Two((f32, f32), (f32, f32)),
}

impl Zoom {
    pub fn new(of: CoverOf) -> Zoom {
        Zoom { of, z: 1.0, off: (0.0, 0.0), pull: 0.0, removed: false, last: None, last_tap: None }
    }

    /// Zoom to `z` keeping the picture's point under `at` (screen units, from the screen's
    /// middle) where it is.
    pub fn zoom_at(&mut self, z: f32, at: (f32, f32)) {
        let z = z.clamp(1.0, MAX_Z);
        let k = z / self.z;
        self.off = (at.0 - (at.0 - self.off.0) * k, at.1 - (at.1 - self.off.1) * k);
        self.z = z;
    }

    /// Kept over the screen: a side bigger than the screen covers it, a smaller one sits in the
    /// middle.
    pub fn clamp(&mut self, base: (f32, f32), screen: (f32, f32)) {
        let (w, h) = (base.0 * self.z, base.1 * self.z);
        let room = |size: f32, screen: f32| ((size - screen) / 2.0).max(0.0);
        let (rx, ry) = (room(w, screen.0), room(h, screen.1));
        self.off = (self.off.0.clamp(-rx, rx), self.off.1.clamp(-ry, ry));
    }
}

/// The picture's size on screen at its own size (units): its pixels as the screen's, never past
/// the screen.
pub fn base_size(px: (u32, u32), u: f32, screen: (f32, f32)) -> (f32, f32) {
    let (w, h) = (px.0 as f32 / u.max(0.01), px.1 as f32 / u.max(0.01));
    let fit = (screen.0 / w).min(screen.1 / h).min(1.0);
    (w * fit, h * fit)
}

/// Draw the viewer over everything; true when it is put away.
pub fn draw(cx: &mut kit::Ctx, im: &mut Images, z: &mut Zoom, now: u64) -> bool {
    cx.c.dy = 0.0;
    cx.ui.cover();
    let screen = (WIDTH, cx.c.h);
    let mid = (WIDTH / 2.0, cx.c.h / 2.0);
    let full = im.full(cx.c, z.of, now);
    let small = im.cover(cx.c, z.of, now);
    let px = full.or_else(|| small.then_some((crate::images::COVER_W, crate::images::COVER_H)));
    let base = px.map(|p| base_size(p, cx.c.u, screen)).unwrap_or((WIDTH * 0.6, WIDTH * 0.85));
    // Fingers: two zoom about their middle, one moves it (or pulls it away at its own size).
    let now_f = match cx.ui.pinch() {
        Some((a, b)) => Some(Fingers::Two(a, b)),
        None => cx.ui.finger().map(Fingers::One),
    };
    let mut close = false;
    match (z.last, now_f) {
        (Some(Fingers::Two(a0, b0)), Some(Fingers::Two(a, b))) => {
            let d0 = ((a0.0 - b0.0).powi(2) + (a0.1 - b0.1).powi(2)).sqrt().max(1.0);
            let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
            let (m0, m) = (((a0.0 + b0.0) / 2.0, (a0.1 + b0.1) / 2.0), ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
            z.zoom_at(z.z * d / d0, (m0.0 - mid.0, m0.1 - mid.1));
            z.off = (z.off.0 + m.0 - m0.0, z.off.1 + m.1 - m0.1);
            z.pull = 0.0;
        }
        (Some(Fingers::One(p0)), Some(Fingers::One(p))) => {
            let (dx, dy) = (p.0 - p0.0, p.1 - p0.1);
            if z.z > 1.01 {
                z.off = (z.off.0 + dx, z.off.1 + dy);
            } else {
                z.pull = (z.pull + dy).max(0.0);
            }
        }
        (Some(_), None) if z.pull > CLOSE_PULL => close = true,
        _ => {}
    }
    if now_f.is_none() {
        z.pull *= 0.7;
        if z.pull < 0.5 {
            z.pull = 0.0;
        }
    }
    z.last = now_f;
    z.clamp(base, screen);
    // A double tap: in on that spot, or back to its own size.
    if let Some(tp) = cx.ui.tap {
        let at = (tp.0 - mid.0, tp.1 - mid.1);
        match z.last_tap {
            Some((t0, p0)) if now.saturating_sub(t0) < DOUBLE_NS && (p0.0 - tp.0).abs() < 40.0 && (p0.1 - tp.1).abs() < 40.0 => {
                if z.z > 1.01 {
                    z.z = 1.0;
                    z.off = (0.0, 0.0);
                } else {
                    z.zoom_at(TAP_Z, at);
                }
                z.clamp(base, screen);
                z.last_tap = None;
            }
            _ => z.last_tap = Some((now, tp)),
        }
    }
    // Black, dimmer as it is pulled away; the picture; ✕.
    let fade = (1.0 - z.pull / (CLOSE_PULL * 2.5)).clamp(0.3, 1.0);
    cx.c.rect(-1000.0, -1000.0, WIDTH + 2000.0, cx.c.h + 2000.0, 0.0, Rgba::hex(0x000000).a(fade));
    let (w, h) = (base.0 * z.z, base.1 * z.z);
    let (x, y) = (mid.0 - w / 2.0 + z.off.0, mid.1 - h / 2.0 + z.off.1 + z.pull);
    if full.is_some() {
        cx.c.image_cover(&z.of.full_key(), x, y, w, h, 0.0, Rgba::WHITE);
    } else if small {
        cx.c.image_cover(&z.of.key(), x, y, w, h, 0.0, Rgba::WHITE);
    } else {
        cx.c.rect(x, y, w, h, 0.0, t::S1);
    }
    if kit::icon_key(cx, WIDTH - 6.0 - 44.0, cx.c.inset_top + 8.0, "close") {
        close = true;
    }
    // A saved picture can be let go from here (the gallery keeps its own copy).
    if matches!(z.of, CoverOf::Panel(_)) && kit::icon_key(cx, 6.0, cx.c.inset_top + 8.0, "trash") {
        z.removed = true;
        close = true;
    }
    cx.ui.tap = None;
    close
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cover_opens_at_its_own_size_never_past_the_screen() {
        // 400 × 600 pixels on a screen of 2.77 pixels a unit: 144 × 217 units, its own size.
        let b = base_size((400, 600), 2.77, (390.0, 844.0));
        assert!((b.0 - 144.4).abs() < 0.5 && (b.1 - 216.6).abs() < 0.5, "{b:?}");
        // 1800 × 2600: bigger than the screen, so it fits it.
        let b = base_size((1800, 2600), 2.77, (390.0, 844.0));
        assert!((b.0 - 390.0).abs() < 0.5 && b.1 <= 844.0, "{b:?}");
    }

    #[test]
    fn zooming_keeps_the_spot_under_the_fingers_and_stays_over_the_screen() {
        let mut z = Zoom::new(CoverOf::Series(1));
        let base = (300.0, 400.0);
        z.zoom_at(2.0, (60.0, -40.0));
        assert_eq!(z.z, 2.0);
        // The point 60 right of the middle is still 60 right of it.
        assert_eq!(z.off, (-60.0, 40.0));
        z.clamp(base, (390.0, 844.0));
        assert_eq!(z.off, (-60.0, 0.0), "600 tall in 844: kept in the middle up and down");
        z.zoom_at(50.0, (0.0, 0.0));
        assert_eq!(z.z, MAX_Z, "no further than five times");
        z.zoom_at(0.2, (0.0, 0.0));
        assert_eq!(z.z, 1.0, "no smaller than its own size");
    }
}
