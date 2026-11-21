//! The Plan pages' own pieces, in the kit's style (DESIGN.md, "The premium look"): group labels,
//! budget, bill and amount rows, a figures card, the stacked bar, the dashed "New goal" tile, keys
//! over a picture, the editors' fields, choice tiles, swatches, chips, the keypad and the Plan's
//! own Undo toast. Colours come from the palette and `theme::category_color` only.

use relay_money::money::MoneyFormatter;

use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, CARD_R, Ctx, KEY_R, PAD, ty};
use crate::plan_logic::{self as logic, BudgetLine, Tone};
use crate::theme;

pub const W: f32 = WIDTH - PAD * 2.0;

pub fn tone_colour(cx: &Ctx, t: Tone) -> Rgba {
    match t {
        Tone::Quiet => cx.p.ink3,
        Tone::Warn => cx.p.waiting_text,
        Tone::Held => cx.p.held_text,
        Tone::Good => cx.p.live_text,
    }
}

/// A group's label over its card, on the ground ("THIS WEEK"), as Accounts' groups. Returns the
/// height to the card.
pub fn group_label(cx: &mut Ctx, y: f32, words: &str) -> f32 {
    let p = cx.p;
    let t = Text::new(12.0, p.ink3).semi();
    cx.c.text(&words.to_uppercase(), PAD + 2.0, y, t);
    t.lh() + 10.0
}

/// Rows on one card, inset 16 from its edge, a hairline between each two from `text_x` (the
/// rows' text column, from the inset edge). `row` draws row `i` at (x, top, width) and says
/// whether it was tapped. Returns (height, the row tapped).
pub fn row_card(cx: &mut Ctx, y: f32, n: usize, row_h: f32, text_x: f32, mut row: impl FnMut(&mut Ctx, usize, f32, f32, f32) -> bool) -> (f32, Option<usize>) {
    let p = cx.p;
    let h = n as f32 * row_h + 8.0;
    kit::card(cx, PAD, y, W, h);
    let (x, w) = (PAD + 16.0, W - 32.0);
    let mut hit = None;
    let mut ry = y + 4.0;
    for i in 0..n {
        if i > 0 {
            cx.c.rect(x + text_x, ry, w - text_x, 1.0, 0.0, p.line_subtle);
        }
        if row(cx, i, x, ry, w) {
            hit = Some(i);
        }
        ry += row_h;
    }
    (h, hit)
}

/// One budget, compact: its badge, its name and what is left (or over), a thin meter with the
/// pace mark, and what was spent of what under it in small type. [`BUDGET_ROW_H`] tall, its text
/// at [`BUDGET_TEXT_X`]. Returns whether it was tapped.
pub fn budget_line(cx: &mut Ctx, x: f32, y: f32, w: f32, b: &BudgetLine, fmt: &MoneyFormatter) -> bool {
    let p = cx.p;
    let rh = BUDGET_ROW_H;
    if cx.pressed(x - 10.0, y + 1.0, w + 20.0, rh - 2.0) {
        cx.c.rect(x - 10.0, y + 3.0, w + 20.0, rh - 6.0, 14.0, p.wash);
    }
    let colour = theme::category_color(Some(b.color));
    kit::badge(cx, x, y + (rh - 36.0) / 2.0, 36.0, &b.icon, colour);
    let tx = x + BUDGET_TEXT_X;
    let room = w - BUDGET_TEXT_X;
    let ty_ = y + 10.0;
    let r = &b.reading;
    let (note, over) = logic::left_words(r, fmt);
    let nt = Text::new(14.0, if over { p.held_text } else { p.ink }).medium().right();
    let nw = cx.c.measure(&note, nt);
    cx.c.text(&note, x + w, ty_, nt);
    cx.c.text_fit(&b.name, tx, ty_ - 1.0, room - nw - 10.0, Text::new(15.0, p.ink).medium());
    let fill = if over {
        p.held
    } else if r.status == relay_money::pace::PaceStatus::OverPace {
        p.waiting
    } else {
        colour
    };
    kit::meter(cx, tx, ty_ + 23.0, room, r.spent_fraction, Some(r.pace_fraction), fill);
    let of = format!("{} of {}", fmt.format(r.spent), fmt.format_whole(r.budget));
    cx.c.text_fit(&of, tx, ty_ + 33.0, room, Text::new(12.0, p.ink3));
    cx.tap(x - 10.0, y + 1.0, w + 20.0, rh - 2.0)
}

pub const BUDGET_ROW_H: f32 = 66.0;
pub const BUDGET_TEXT_X: f32 = 48.0;

/// A row with a category badge, a title and a line, and an amount at its end. [`ROW_H`] tall, its
/// text at [`ROW_TEXT_X`].
#[allow(clippy::too_many_arguments)]
pub fn badge_row(cx: &mut Ctx, x: f32, y: f32, w: f32, icon: &str, colour: Rgba, title: &str, sub: &str, amount: &str, amount_colour: Rgba) -> bool {
    let p = cx.p;
    if cx.pressed(x - 10.0, y + 1.0, w + 20.0, ROW_H - 2.0) {
        cx.c.rect(x - 10.0, y + 3.0, w + 20.0, ROW_H - 6.0, 14.0, p.wash);
    }
    kit::badge(cx, x, y + (ROW_H - 40.0) / 2.0, 40.0, icon, colour);
    let at = Text::new(16.0, amount_colour).medium().right();
    let aw = cx.c.measure(amount, at);
    cx.c.text(amount, x + w, y + (ROW_H - at.lh()) / 2.0, at);
    let tx = x + ROW_TEXT_X;
    let room = (w - ROW_TEXT_X - aw - 12.0).max(40.0);
    cx.c.text_fit(title, tx, y + 12.0, room, Text::new(16.0, p.ink).medium());
    cx.c.text_fit(sub, tx, y + 35.0, room, Text::new(13.0, p.ink2));
    cx.tap(x - 10.0, y + 1.0, w + 20.0, ROW_H - 2.0)
}

pub const ROW_H: f32 = 64.0;
pub const ROW_TEXT_X: f32 = 54.0;

/// What a bill row shows.
pub struct BillShow<'a> {
    pub month: String,
    pub day: i8,
    /// The palette's signal for a date that is close (the badge turns brass).
    pub hue: Rgba,
    pub name: &'a str,
    pub note: &'a str,
    pub note_colour: Rgba,
    /// A check before the note: it was paid.
    pub paid: bool,
    pub amount: String,
    pub amount_colour: Rgba,
}

/// One bill row's height inside its group's card.
pub const BILL_ROW_H: f32 = 66.0;

/// A group's bills on one card, a row each with hairlines between. Returns (height, the row
/// tapped).
pub fn bill_card(cx: &mut Ctx, y: f32, rows: &[BillShow]) -> (f32, Option<usize>) {
    row_card(cx, y, rows.len(), BILL_ROW_H, 58.0, |cx, i, x, ry, w| bill_line(cx, x, ry, w, &rows[i]))
}

/// A bill: its date badge, name, line and amount, [`BILL_ROW_H`] tall. Returns whether it was
/// tapped.
fn bill_line(cx: &mut Ctx, x: f32, y: f32, w: f32, b: &BillShow) -> bool {
    let p = cx.p;
    let rh = BILL_ROW_H;
    if cx.pressed(x - 10.0, y + 1.0, w + 20.0, rh - 2.0) {
        cx.c.rect(x - 10.0, y + 3.0, w + 20.0, rh - 6.0, 14.0, p.wash);
    }
    kit::date_badge(cx, x, y + 9.0, &b.month, b.day, b.hue);
    let at = Text::new(16.0, b.amount_colour).medium().right();
    let mut aw = cx.c.measure(&b.amount, at);
    cx.c.text(&b.amount, x + w, y + (rh - at.lh()) / 2.0, at);
    if b.paid {
        cx.c.icon("check", x + w - aw - 14.0, y + rh / 2.0, 14.0, p.live_text);
        aw += 22.0;
    }
    let tx = x + 58.0;
    let room = (w - 58.0 - aw - 12.0).max(40.0);
    cx.c.text_fit(b.name, tx, y + 13.0, room, Text::new(16.0, p.ink).medium());
    cx.c.text_fit(b.note, tx, y + 36.0, room, Text::new(13.0, b.note_colour));
    cx.tap(x - 10.0, y + 1.0, w + 20.0, rh - 2.0)
}

/// Figures two to a row on one card, hairlines between them: a label, the figure, a line under
/// it. Returns the height used.
pub fn figures(cx: &mut Ctx, y: f32, items: &[(&str, String, String)]) -> f32 {
    let p = cx.p;
    let cw = W / 2.0;
    let inner = cw - 36.0;
    let lt = Text::new(13.0, p.ink2).medium();
    let ft = Text::new(22.0, p.ink).medium();
    let st = Text::new(12.5, p.ink3).line(1.35);
    let rows: Vec<f32> = items
        .chunks(2)
        .map(|row| row.iter().map(|it| 18.0 + lt.lh() + 4.0 + ft.lh() + 4.0 + cx.c.para_h(&it.2, inner, st, 2) + 18.0).fold(0.0, f32::max))
        .collect();
    let h: f32 = rows.iter().sum();
    kit::card(cx, PAD, y, W, h);
    let mut ry = y;
    for (r, row) in items.chunks(2).enumerate() {
        if r > 0 {
            cx.c.rect(PAD + 18.0, ry, W - 36.0, 1.0, 0.0, p.line_subtle);
        }
        for (i, it) in row.iter().enumerate() {
            let x = PAD + i as f32 * cw + 18.0;
            if i > 0 {
                cx.c.rect(PAD + cw, ry + 14.0, 1.0, rows[r] - 28.0, 0.0, p.line_subtle);
            }
            let mut ty_ = ry + 18.0;
            cx.c.text_fit(it.0, x, ty_, inner, lt);
            ty_ += lt.lh() + 4.0;
            cx.c.text_fit(&it.1, x, ty_, inner, ft);
            ty_ += ft.lh() + 4.0;
            cx.c.para(&it.2, x, ty_, inner, st, 2);
        }
        ry += rows[r];
    }
    h
}

/// Parts side by side in one rounded bar, a gap of the ground between them.
pub fn stack_bar(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, parts: &[(Rgba, i64)]) {
    let track = cx.p.track;
    cx.c.rect(x, y, w, h, h / 2.0, track);
    let total: i64 = parts.iter().map(|p| p.1.max(0)).sum::<i64>().max(1);
    let shown: Vec<(Rgba, f32)> = parts.iter().filter(|p| p.1 > 0).map(|p| (p.0, w * p.1 as f32 / total as f32)).collect();
    let n = shown.len();
    let r = h / 2.0;
    let mut at = x;
    for (i, (colour, pw)) in shown.iter().enumerate() {
        let seam = if i + 1 < n { 2.0 } else { 0.0 };
        let sw = (pw - seam).max(1.0);
        let (first, last) = (i == 0, i + 1 == n);
        if first || last {
            // A rounded piece, squared off on the side that meets another.
            cx.c.rect(at, y, sw, h, r.min(sw / 2.0), *colour);
            if !first {
                cx.c.rect(at, y, (sw / 2.0).min(sw), h, 0.0, *colour);
            }
            if !last {
                cx.c.rect(at + sw / 2.0, y, sw / 2.0, h, 0.0, *colour);
            }
        } else {
            cx.c.rect(at, y, sw, h, 0.0, *colour);
        }
        at += pw;
    }
}

/// A rounded box drawn in dashes, as a place for something not made yet.
pub fn dashed_box(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, r: f32, colour: Rgba) {
    let (dash, gap, t) = (7.0, 6.0, 1.5);
    // Where the dashes of a straight run of `len` start, centred on it.
    let starts = |len: f32| {
        let n = ((len + gap) / (dash + gap)).floor().max(1.0) as usize;
        let lead = (len - (n as f32 * (dash + gap) - gap)) / 2.0;
        (0..n).map(move |i| lead + i as f32 * (dash + gap))
    };
    let (hx, hw) = (x + r, w - 2.0 * r);
    let (vy, vh) = (y + r, h - 2.0 * r);
    for at in starts(hw) {
        cx.c.rect(hx + at, y, dash, t, 0.0, colour);
        cx.c.rect(hx + at, y + h - t, dash, t, 0.0, colour);
    }
    for at in starts(vh) {
        cx.c.rect(x, vy + at, t, dash, 0.0, colour);
        cx.c.rect(x + w - t, vy + at, t, dash, 0.0, colour);
    }
    // The corners: two dashes of arc each.
    let corners = [(x + w - r, y + r, 0.0), (x + w - r, y + h - r, 0.25), (x + r, y + h - r, 0.5), (x + r, y + r, 0.75)];
    for (ccx, ccy, from) in corners {
        cx.c.arc(ccx, ccy, r - t / 2.0, t, from + 0.03, from + 0.1, colour, true);
        cx.c.arc(ccx, ccy, r - t / 2.0, t, from + 0.15, from + 0.22, colour, true);
    }
}

/// The dashed tile that adds a goal: a plus over its words. Returns whether it was tapped.
pub fn new_tile(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str) -> bool {
    let p = cx.p;
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, CARD_R, p.wash);
    }
    let edge = if p.light { p.ink3.a(0.55) } else { p.ink3.a(0.6) };
    dashed_box(cx, x, y, w, h, CARD_R, edge);
    let mid = y + h / 2.0;
    cx.c.icon("plus", x + w / 2.0, mid - 14.0, 22.0, p.ink2);
    cx.c.text(words, x + w / 2.0, mid + 6.0, Text::new(14.0, p.ink2).medium().center());
    cx.tap(x, y, w, h)
}

/// A round key over a picture: a soft disc of the ground under the glyph, so it reads on any
/// part of the picture. Returns whether it was tapped.
pub fn over_key(cx: &mut Ctx, x: f32, y: f32, glyph: &'static str) -> bool {
    let p = cx.p;
    let d = 40.0;
    let disc = p.ground[0].a(if p.light { 0.72 } else { 0.5 });
    cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, disc);
    if cx.pressed(x - 2.0, y - 2.0, d + 4.0, d + 4.0) {
        cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, p.wash);
    }
    cx.c.icon(glyph, x + d / 2.0, y + d / 2.0, 20.0, p.ink);
    cx.tap(x - 2.0, y - 2.0, d + 4.0, d + 4.0)
}

/// A key across the page for an act that removes something: quiet, its words in the held colour.
pub fn remove_key(cx: &mut Ctx, y: f32, words: &str) -> bool {
    let p = cx.p;
    let h = 52.0;
    cx.c.rect(PAD, y, W, h, 18.0, p.card2);
    if cx.pressed(PAD, y, W, h) {
        cx.c.rect(PAD, y, W, h, 18.0, p.wash);
    }
    let t = Text::new(16.0, p.held_text).semi().center();
    cx.c.text(words, WIDTH / 2.0, y + (h - t.lh()) / 2.0, t);
    cx.tap(PAD, y, W, h)
}

/// A label over a control ("Name"). Returns its height.
pub fn label(cx: &mut Ctx, y: f32, words: &str) -> f32 {
    let p = cx.p;
    cx.c.text(words, PAD + 2.0, y, Text::new(13.0, p.ink2).medium());
    24.0
}

/// A line under a control: a hint, or a problem.
pub fn hint(cx: &mut Ctx, y: f32, words: &str, problem: bool) -> f32 {
    let p = cx.p;
    let t = Text::new(ty::CAPTION, if problem { p.held_text } else { p.ink3 });
    cx.c.para(words, PAD + 2.0, y + 6.0, W - 4.0, t, 2) + 10.0
}

/// A field on the card colour with a hairline edge, brass while it has the keyboard, and an
/// optional glyph before what is typed. Returns its height (52).
#[allow(clippy::too_many_arguments)]
pub fn field(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, glyph: Option<&'static str>, placeholder: &str, value: &mut String, numeric: bool, error: bool) -> f32 {
    field_kb(cx, id, x, y, w, glyph, placeholder, value, crate::widgets::Kb::numeric(numeric), error)
}

/// [`field`] with the keyboard named: a Balance goal's target takes a minus sign.
#[allow(clippy::too_many_arguments)]
pub fn field_kb(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, glyph: Option<&'static str>, placeholder: &str, value: &mut String, kind: crate::widgets::Kb, error: bool) -> f32 {
    let p = cx.p;
    let h = 52.0;
    let focused = cx.focus.is(id);
    cx.c.rect(x, y, w, h, KEY_R, p.slab);
    if focused {
        cx.c.outline(x, y, w, h, KEY_R, 1.5, p.signal);
    } else if error {
        cx.c.outline(x, y, w, h, KEY_R, 1.5, p.held_text);
    } else {
        cx.c.outline(x, y, w, h, KEY_R, 1.0, p.line_subtle);
    }
    let mut tx = x + 16.0;
    if let Some(g) = glyph {
        cx.c.icon(g, tx + 8.0, y + h / 2.0, 16.0, p.ink3);
        tx += 28.0;
    }
    crate::widgets::inline_kb(cx, id, tx, y, x + w - 16.0 - tx, h, placeholder, value, kind, Text::new(16.0, p.ink));
    h
}

/// A field that opens a picker: what is picked (or what to pick) and a chevron. 52 tall.
pub fn pick_field(cx: &mut Ctx, x: f32, y: f32, w: f32, text: &str, filled: bool, glyph: Option<&'static str>) -> bool {
    let p = cx.p;
    let h = 52.0;
    cx.c.rect(x, y, w, h, KEY_R, p.slab);
    cx.c.outline(x, y, w, h, KEY_R, 1.0, p.line_subtle);
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, KEY_R, p.wash);
    }
    let mut tx = x + 16.0;
    if let Some(g) = glyph {
        cx.c.icon(g, tx + 8.0, y + h / 2.0, 16.0, p.ink3);
        tx += 28.0;
    }
    let t = Text::new(16.0, if filled { p.ink } else { p.ink3 });
    cx.c.text_fit(text, tx, y + (h - t.lh()) / 2.0, x + w - 40.0 - tx, t);
    cx.c.icon("chevron-down", x + w - 22.0, y + h / 2.0, 14.0, p.ink3);
    cx.tap(x, y, w, h)
}

/// A choice as a tile: a glyph on a quiet square, a title, a hint; an ink edge and a check when
/// picked.
#[allow(clippy::too_many_arguments)]
pub fn choice_tile(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, title: &str, hint: &str, glyph: &'static str, on: bool) -> bool {
    let p = cx.p;
    kit::card(cx, x, y, w, h);
    if on {
        cx.c.outline(x, y, w, h, CARD_R, 2.0, p.ink);
        cx.c.circle(x + w - 12.0 - 10.0, y + 12.0 + 10.0, 10.0, p.ink);
        cx.c.icon("check", x + w - 22.0, y + 22.0, 12.0, p.on_ink);
    }
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, CARD_R, p.wash);
    }
    let (ix, mut iy) = (x + 14.0, y + 14.0);
    cx.c.rect(ix, iy, 38.0, 38.0, 12.0, p.card2);
    cx.c.icon(glyph, ix + 19.0, iy + 19.0, 18.0, p.ink);
    iy += 38.0 + 10.0;
    let tt = Text::new(15.0, p.ink).semi().line(1.3);
    iy += cx.c.para(title, ix, iy, w - 28.0, tt, 2) + 4.0;
    cx.c.para(hint, ix, iy, w - 28.0, Text::new(12.0, p.ink3).line(1.35), 3);
    cx.tap(x, y, w, h)
}

pub fn choice_tile_h(cx: &mut Ctx, w: f32, title: &str, hint: &str) -> f32 {
    let t = cx.c.para_h(title, w - 28.0, Text::new(15.0, Rgba::WHITE).semi().line(1.3), 2);
    let h = cx.c.para_h(hint, w - 28.0, Text::new(12.0, Rgba::WHITE).line(1.35), 3);
    14.0 + 38.0 + 10.0 + t + 4.0 + h + 16.0
}

/// The twelve category hues as circles on a card; the picked one wears an ink ring and a check.
/// Returns (height, picked index).
pub fn swatches(cx: &mut Ctx, y: f32, selected: i64) -> (f32, Option<i64>) {
    let p = cx.p;
    let (d, gap) = (36.0, 10.0);
    let per_row = ((W - 28.0 + gap) / (d + gap)).floor() as usize;
    let rows = theme::CATEGORY_PAIRS.len().div_ceil(per_row);
    let h = 16.0 * 2.0 + rows as f32 * d + (rows as f32 - 1.0) * gap;
    kit::card(cx, PAD, y, W, h);
    let lead = (W - (per_row as f32 * (d + gap) - gap)) / 2.0;
    let mut out = None;
    for (i, colour) in theme::category_colors().iter().enumerate() {
        let (sx, sy) = (PAD + lead + (i % per_row) as f32 * (d + gap), y + 16.0 + (i / per_row) as f32 * (d + gap));
        let on = selected == i as i64;
        if on {
            cx.c.circle(sx + d / 2.0, sy + d / 2.0, d / 2.0, p.ink);
            cx.c.circle(sx + d / 2.0, sy + d / 2.0, d / 2.0 - 2.5, p.slab);
            cx.c.circle(sx + d / 2.0, sy + d / 2.0, d / 2.0 - 5.0, *colour);
            cx.c.icon("check", sx + d / 2.0, sy + d / 2.0, 14.0, p.slab);
        } else {
            cx.c.circle(sx + d / 2.0, sy + d / 2.0, d / 2.0, *colour);
        }
        if cx.tap(sx - 4.0, sy - 4.0, d + 8.0, d + 8.0) {
            out = Some(i as i64);
        }
    }
    (h, out)
}

/// A chip with an optional glyph on the second card colour: the budget editor's quick fills.
pub fn chip(cx: &mut Ctx, x: f32, y: f32, words: &str, glyph: Option<&'static str>) -> (f32, bool) {
    let p = cx.p;
    let t = Text::new(ty::UI, p.ink).medium();
    let tw = cx.c.measure(words, t);
    let w = tw + 28.0 + if glyph.is_some() { 20.0 } else { 0.0 };
    let h = 36.0;
    cx.c.rect(x, y, w, h, h / 2.0, p.card2);
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, h / 2.0, p.wash);
    }
    let mut tx = x + 14.0;
    if let Some(g) = glyph {
        cx.c.icon(g, tx + 7.0, y + h / 2.0, 13.0, p.signal);
        tx += 20.0;
    }
    cx.c.text(words, tx, y + (h - t.lh()) / 2.0, t);
    (w, cx.tap(x, y, w, h))
}

/// The digits keypad at the foot of the page, on the ground with a hairline over it. `top` is
/// its top. Returns the key pressed.
pub fn keypad(cx: &mut Ctx, top: f32, h: f32, decimal: bool) -> Option<char> {
    let p = cx.p;
    cx.c.rect(0.0, top, WIDTH, h + 30.0, 0.0, p.ground[0]);
    cx.c.rect(0.0, top, WIDTH, 1.0, 0.0, p.line_subtle);
    let (gap, kh) = (6.0, 46.0);
    let kw = (W - gap * 2.0) / 3.0;
    let keys = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '.', '0', '<'];
    let mut out = None;
    for (i, k) in keys.iter().enumerate() {
        if *k == '.' && !decimal {
            continue;
        }
        let (x, y) = (PAD + (i % 3) as f32 * (kw + gap), top + 14.0 + (i / 3) as f32 * (kh + gap));
        cx.c.rect(x, y, kw, kh, KEY_R, p.card2);
        if cx.pressed(x, y, kw, kh) {
            cx.c.rect(x, y, kw, kh, KEY_R, p.wash);
        }
        if *k == '<' {
            cx.c.icon("arrow-left", x + kw / 2.0, y + kh / 2.0, 20.0, p.ink);
        } else {
            let t = Text::new(22.0, p.ink).medium().center();
            cx.c.text(&k.to_string(), x + kw / 2.0, y + (kh - t.lh()) / 2.0, t);
        }
        if cx.tap(x, y, kw, kh) {
            out = Some(*k);
        }
    }
    out
}

/// The keypad's height above the system bar.
pub const KEYPAD_H: f32 = 14.0 + 4.0 * 52.0 + 8.0;

/// A key with a glyph and words, filled or plain, dimmed while it cannot act.
#[allow(clippy::too_many_arguments)]
pub fn action_key(cx: &mut Ctx, x: f32, y: f32, w: f32, words: &str, glyph: &'static str, primary: bool, enabled: bool) -> bool {
    let p = cx.p;
    let h = 52.0;
    let (bg, fg) = if primary { (p.prim, p.on_ink) } else { (p.card2, p.ink) };
    let dim = if enabled { 1.0 } else { 0.4 };
    cx.c.rect(x, y, w, h, 18.0, bg.a(dim));
    if enabled && cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, 18.0, p.wash);
    }
    let t = Text::new(16.0, fg.a(if enabled { 1.0 } else { 0.7 })).semi();
    let tw = cx.c.measure(words, t) + 26.0;
    let tx = x + w / 2.0 - tw / 2.0;
    cx.c.icon(glyph, tx + 9.0, y + h / 2.0, 18.0, t.color);
    cx.c.text(words, tx + 26.0, y + (h - t.lh()) / 2.0, t);
    cx.tap(x, y, w, h) && enabled
}

/// The Plan's Undo: a pill in ink above `bottom`, words and Undo. Returns whether Undo was tapped.
pub fn undo_toast(cx: &mut Ctx, bottom: f32, words: &str) -> bool {
    let p = cx.p;
    let (x, w, h) = (16.0, WIDTH - 32.0, 48.0);
    let y = bottom - h - 12.0;
    let saved = cx.c.dy;
    cx.c.dy = 0.0;
    cx.c.rect(x, y, w, h, 16.0, p.prim);
    let tt = Text::new(ty::UI, p.on_ink).medium();
    cx.c.text_fit(words, x + 16.0, y + (h - tt.lh()) / 2.0, w - 32.0 - 60.0, tt);
    let ut = Text::new(ty::UI, p.on_ink).semi().right();
    let uw = cx.c.text("Undo", x + w - 16.0, y + (h - ut.lh()) / 2.0, ut);
    cx.c.rect(x + w - 16.0 - uw, y + (h + ut.lh()) / 2.0, uw, 1.0, 0.0, p.on_ink.a(0.6));
    let tapped = cx.ui.tapped(x + w - 90.0, y, 90.0, h);
    // A tap on the rest of the pill goes nowhere under it.
    let _ = cx.ui.tapped(x, y, w, h);
    cx.c.dy = saved;
    tapped
}
