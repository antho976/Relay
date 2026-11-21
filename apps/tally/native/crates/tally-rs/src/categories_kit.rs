//! The category controls the kit lacks, in the premium look: the grid's tile and its dashed New
//! tile, the twelve colour swatches, the icon choice, and the sheet that asks where a deleted
//! category's entries go.

use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, CARD_R, Ctx, PAD, ty};
use crate::theme;
use crate::ui::Scroll;

/// A category's tile in the grid: its badge, its name and this month's money. Returns whether it
/// was tapped.
#[allow(clippy::too_many_arguments)]
pub fn category_tile(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, name: &str, icon: &str, color: i64, figure: &str) -> bool {
    let p = cx.p;
    let tapped = kit::card_button(cx, x, y, w, h);
    kit::badge(cx, x + 14.0, y + 14.0, 38.0, icon, theme::category_color(Some(color)));
    cx.c.text_fit(name, x + 14.0, y + h - 14.0 - 17.0 - 20.0, w - 28.0, Text::new(14.0, p.ink).medium());
    cx.c.text_fit(figure, x + 14.0, y + h - 14.0 - 17.0, w - 28.0, Text::new(13.0, p.ink2));
    tapped
}

/// A grid tile's height.
pub const TILE_H: f32 = 116.0;

/// The dashed tile at the grid's end that adds one: a plus over its word. Returns whether it was
/// tapped.
pub fn new_tile(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str) -> bool {
    let p = cx.p;
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, CARD_R, p.wash);
    }
    let edge = if p.light { p.ink3.a(0.55) } else { p.ink3.a(0.6) };
    dashed_outline(cx, x, y, w, h, CARD_R, edge);
    cx.c.icon("plus", x + w / 2.0, y + h / 2.0 - 10.0, 20.0, p.ink2);
    cx.c.text(words, x + w / 2.0, y + h / 2.0 + 6.0, Text::new(14.0, p.ink2).medium().center());
    cx.tap(x, y, w, h)
}

/// A rounded rectangle's edge in dashes, 1 unit thick.
fn dashed_outline(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
    let dashes = |len: f32| -> Vec<(f32, f32)> {
        let (dash, gap): (f32, f32) = (5.0, 4.0);
        let mut out = Vec::new();
        let mut at = gap / 2.0;
        while at < len {
            out.push((at, dash.min(len - at)));
            at += dash + gap;
        }
        out
    };
    let (iw, ih) = (w - 2.0 * r, h - 2.0 * r);
    for (a, l) in dashes(iw) {
        cx.c.rect(x + r + a, y, l, 1.0, 0.0, c);
        cx.c.rect(x + r + a, y + h - 1.0, l, 1.0, 0.0, c);
    }
    for (a, l) in dashes(ih) {
        cx.c.rect(x, y + r + a, 1.0, l, 0.0, c);
        cx.c.rect(x + w - 1.0, y + r + a, 1.0, l, 0.0, c);
    }
    // Each corner as two short arcs.
    let corners = [(x + w - r, y + r, 0.0), (x + w - r, y + h - r, 0.25), (x + r, y + h - r, 0.5), (x + r, y + r, 0.75)];
    for (ccx, ccy, start) in corners {
        for (a, b) in [(0.02, 0.10), (0.15, 0.23)] {
            cx.c.arc(ccx, ccy, r, 1.0, start + a, start + b, c, true);
        }
    }
}

/// The twelve colours, in the shades of the palette in use, as circles on a card; the picked
/// one wears a ring and a check. Returns the one tapped and the height used.
pub fn swatches(cx: &mut Ctx, x: f32, y: f32, w: f32, selected: i64) -> (Option<i64>, f32) {
    let p = cx.p;
    let colours = theme::category_colors();
    let pad = 16.0;
    let per_row = 6usize;
    let d = 36.0;
    let gap = ((w - pad * 2.0) - d * per_row as f32) / (per_row - 1) as f32;
    let rows = colours.len().div_ceil(per_row);
    let rgap = 14.0;
    let h = pad * 2.0 + rows as f32 * d + (rows - 1) as f32 * rgap;
    kit::card(cx, x, y, w, h);
    let mut out = None;
    for (i, hue) in colours.iter().enumerate() {
        let (sx, sy) = (x + pad + (i % per_row) as f32 * (d + gap), y + pad + (i / per_row) as f32 * (d + rgap));
        let (cxm, cym) = (sx + d / 2.0, sy + d / 2.0);
        if i as i64 == selected {
            cx.c.circle(cxm, cym, d / 2.0 + 3.0, p.ink);
            cx.c.circle(cxm, cym, d / 2.0 + 1.0, p.slab);
            cx.c.circle(cxm, cym, d / 2.0 - 1.0, *hue);
            cx.c.icon("check", cxm, cym, 16.0, p.on_ink);
        } else {
            cx.c.circle(cxm, cym, d / 2.0, *hue);
        }
        if cx.tap(sx - gap / 2.0, sy - rgap / 2.0, d + gap, d + rgap) {
            out = Some(i as i64);
        }
    }
    (out, h)
}

/// A group of category icons on a card, each on a quiet key; the picked one on its colour's
/// tint with a ring of the colour. Returns the key tapped and the height used.
pub fn icon_choices<'a>(cx: &mut Ctx, x: f32, y: f32, w: f32, keys: &[&'a str], color: i64, selected: &str) -> (Option<&'a str>, f32) {
    let p = cx.p;
    let pad = 14.0;
    let per_row = 6usize;
    let gap = 8.0;
    let d = ((w - pad * 2.0) - gap * (per_row - 1) as f32) / per_row as f32;
    let rows = keys.len().div_ceil(per_row).max(1);
    let h = pad * 2.0 + rows as f32 * d + (rows - 1) as f32 * gap;
    kit::card(cx, x, y, w, h);
    let hue = theme::category_color(Some(color));
    let mut out = None;
    for (i, key) in keys.iter().enumerate() {
        let (sx, sy) = (x + pad + (i % per_row) as f32 * (d + gap), y + pad + (i / per_row) as f32 * (d + gap));
        let on = *key == selected;
        if on {
            cx.c.rect(sx, sy, d, d, 14.0, theme::soft_of(hue));
            cx.c.outline(sx, sy, d, d, 14.0, 1.5, hue);
        } else {
            cx.c.rect(sx, sy, d, d, 14.0, p.card2);
        }
        if cx.pressed(sx, sy, d, d) {
            cx.c.rect(sx, sy, d, d, 14.0, p.wash);
        }
        cx.c.category_icon(key, sx + d / 2.0, sy + d / 2.0, 22.0, if on { hue } else { p.ink2 });
        if cx.tap(sx, sy, d, d) {
            out = Some(*key);
        }
    }
    (out, h)
}

/// One row of the move sheet: a category to move to (with its badge), or the choice to leave them.
pub struct MoveRow<'a> {
    pub title: &'a str,
    pub sub: &'a str,
    pub badge: Option<(&'a str, i64)>,
}

/// The sheet that asks where a deleted category's entries and bills go: a heading, the line
/// that says what goes for good, then a row per choice. It scrolls when the list is long; taps on
/// the veil close it. Returns the row picked, or `Some(None)` for closed.
pub fn move_sheet(cx: &mut Ctx, heading: &str, note: &str, rows: &[MoveRow], scroll: &mut Scroll, dt: f32) -> Option<Option<usize>> {
    let p = cx.p;
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    let any_tap = cx.ui.tap.is_some();
    let screen_h = cx.c.h;
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, screen_h, 0.0, Rgba::hex(0x000000).a(if p.light { 0.4 } else { 0.7 }));
    let nt = Text::new(ty::CAPTION, p.ink3).line(1.4);
    let note_h = cx.c.para_h(note, WIDTH - PAD * 2.0 - 8.0, nt, 3);
    let head = 22.0 + 24.0 + 6.0 + note_h + 10.0;
    let rh = 60.0;
    let list_h = rows.len() as f32 * rh;
    let h = (head + list_h + 16.0 + cx.c.inset_bottom).min(screen_h * 0.75);
    let top = screen_h - h;
    cx.c.rect(0.0, top, WIDTH, h + 40.0, 28.0, p.raised);
    cx.c.rect(WIDTH / 2.0 - 18.0, top + 8.0, 36.0, 4.0, 2.0, p.track.mix(p.ink, 0.2));
    cx.c.text_fit(heading, PAD + 4.0, top + 22.0, WIDTH - PAD * 2.0 - 8.0, Text::new(ty::TITLE, p.ink).semi());
    cx.c.para(note, PAD + 4.0, top + 22.0 + 24.0 + 6.0, WIDTH - PAD * 2.0 - 8.0, nt, 3);
    let view = [0.0, top + head, WIDTH, h - head];
    scroll.update(cx.ui, view, dt);
    scroll.max = list_h + 16.0 + cx.c.inset_bottom - (h - head);
    // A row scrolled up under the heading is hidden there: a tap on the heading is not for it.
    let in_list = cx.ui.tap.is_some_and(|(_, ty_)| ty_ >= top + head);
    cx.c.push_clip(0.0, top + head, WIDTH, h - head);
    cx.c.dy = -scroll.off;
    let mut out = None;
    let mut y = top + head;
    for (i, r) in rows.iter().enumerate() {
        let (x, w) = (PAD - 4.0, WIDTH - 2.0 * (PAD - 4.0));
        if cx.pressed(x, y, w, rh) {
            cx.c.rect(x, y, w, rh, 14.0, p.wash);
        }
        let mut tx = x + 12.0;
        if let Some((icon, color)) = r.badge {
            crate::kit::badge(cx, tx, y + (rh - 32.0) / 2.0, 32.0, icon, theme::category_color(Some(color)));
            tx += 32.0 + 12.0;
        }
        let room = x + w - 12.0 - tx;
        cx.c.text_fit(r.title, tx, y + 10.0, room, Text::new(ty::BODY, p.ink).medium());
        cx.c.text_fit(r.sub, tx, y + 32.0, room, Text::new(ty::CAPTION, p.ink3));
        if in_list && cx.tap(x, y, w, rh) {
            out = Some(Some(i));
        }
        y += rh;
    }
    cx.c.dy = 0.0;
    cx.c.pop_clip();
    if out.is_none() && any_tap && cx.ui.tapped(-1000.0, 0.0, WIDTH + 2000.0, top) {
        out = Some(None);
    }
    cx.ui.tap = None;
    cx.c.dy = saved_dy;
    out
}
