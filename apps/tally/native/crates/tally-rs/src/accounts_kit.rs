//! The pieces Accounts, Investments and Categories share that the kit has no control for yet, in
//! the premium look's style: a second key in a pushed page's header, a group's eyebrow with its
//! total, rows inside a card with hairlines between them, a stack bar, a thin meter, a label and
//! hint for fields, a value chart, a keypad and a confirm dialog with a gentler way out.

use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, CARD_R, Ctx, KEY_R, KeyKind, PAD, ty};

/// The width of a page's column.
pub const W: f32 = WIDTH - PAD * 2.0;

/// A second glyph key in a pushed page's header, left of the one `widgets::page_header` draws.
pub fn header_key(cx: &mut Ctx, top: f32, glyph: &'static str) -> bool {
    kit::round_key(cx, WIDTH - 10.0 - 44.0 - 44.0, top, glyph)
}

/// A group's eyebrow ("EVERYDAY") with an optional figure at its right. Returns the height used.
pub fn group_head(cx: &mut Ctx, y: f32, label: &str, figure: Option<&str>) -> f32 {
    let p = cx.p;
    let t = Text::new(12.0, p.ink3).semi();
    cx.c.text(&label.to_uppercase(), PAD + 2.0, y, t);
    if let Some(f) = figure {
        cx.c.text(f, PAD + W - 2.0, y, Text::new(12.0, p.ink3).medium().right());
    }
    t.lh() + 10.0
}

/// One row's height inside a card.
pub const ROW_H: f32 = 66.0;

/// A row inside a card: a glyph on a quiet 36 square, a title, a line under it, and a figure at
/// the right (with its colour), with an optional second figure under it. Returns whether it was
/// tapped.
#[allow(clippy::too_many_arguments)]
pub fn card_row(cx: &mut Ctx, x: f32, y: f32, w: f32, glyph: &'static str, title: &str, line: &str, figure: Option<(&str, Rgba)>, under: Option<(&str, Rgba)>) -> bool {
    let p = cx.p;
    let h = ROW_H;
    if cx.pressed(x + 4.0, y + 4.0, w - 8.0, h - 8.0) {
        cx.c.rect(x + 4.0, y + 4.0, w - 8.0, h - 8.0, 18.0, p.wash);
    }
    let gx = x + 16.0;
    cx.c.rect(gx, y + (h - 36.0) / 2.0, 36.0, 36.0, 11.0, p.card2);
    cx.c.icon(glyph, gx + 18.0, y + h / 2.0, 18.0, p.ink);
    let tx = gx + 36.0 + 14.0;
    let right = x + w - 16.0;
    let mut fw: f32 = 0.0;
    if let Some((f, colour)) = figure {
        let ft = Text::new(16.0, colour).medium().right();
        fw = cx.c.measure(f, ft) + 12.0;
        let fy = if under.is_some() { y + 13.0 } else { y + (h - ft.lh()) / 2.0 };
        cx.c.text(f, right, fy, ft);
    }
    if let Some((u, colour)) = under {
        let ut = Text::new(13.0, colour).medium().right();
        fw = fw.max(cx.c.measure(u, ut) + 12.0);
        cx.c.text(u, right, y + 36.0, ut);
    }
    let room = (right - tx - fw).max(40.0);
    cx.c.text_fit(title, tx, y + 13.0, room, Text::new(16.0, p.ink).medium());
    cx.c.text_fit(line, tx, y + 36.0, room, Text::new(13.0, p.ink2));
    cx.tap(x, y, w, h)
}

/// The hairline between two rows of a card, from the words' edge to the card's inner edge.
pub fn row_rule(cx: &mut Ctx, x: f32, y: f32, w: f32) {
    let c = cx.p.line_subtle;
    cx.c.rect(x + 16.0 + 36.0 + 14.0, y, w - (16.0 + 36.0 + 14.0) - 16.0, 1.0, 0.0, c);
}

/// A card of rows, each [`ROW_H`] tall with hairlines between. `draw` draws row `i` at its top
/// and says whether it was tapped. Returns the card's height and the row tapped.
pub fn row_card(cx: &mut Ctx, y: f32, n: usize, mut draw: impl FnMut(&mut Ctx, usize, f32) -> bool) -> (f32, Option<usize>) {
    let h = n as f32 * ROW_H + 8.0;
    kit::card(cx, PAD, y, W, h);
    let mut hit = None;
    for i in 0..n {
        let ry = y + 4.0 + i as f32 * ROW_H;
        if i > 0 {
            row_rule(cx, PAD, ry, W);
        }
        if draw(cx, i, ry) {
            hit = Some(i);
        }
    }
    (h, hit)
}

/// Shares of a whole as one bar of hues, with 2-unit seams.
pub fn stack_bar(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, parts: &[(Rgba, i64)]) {
    let track = cx.p.track;
    cx.c.rect(x, y, w, h, h / 2.0, track);
    let total: i64 = parts.iter().map(|(_, v)| (*v).max(0)).sum();
    if total <= 0 {
        return;
    }
    let shown: Vec<(Rgba, f32)> = parts.iter().filter(|(_, v)| *v > 0).map(|(c, v)| (*c, w * *v as f32 / total as f32)).collect();
    let n = shown.len();
    let mut at = x;
    for (i, (colour, sw)) in shown.into_iter().enumerate() {
        let seam = if i + 1 < n { 2.0 } else { 0.0 };
        let sw_ = (sw - seam).max(1.0);
        // Only the bar's two ends are round.
        let r = (h / 2.0).min(sw_ / 2.0);
        if n == 1 {
            cx.c.rect(at, y, sw_, h, r, colour);
        } else if i == 0 {
            cx.c.rect(at, y, sw_, h, r, colour);
            cx.c.rect(at + r, y, sw_ - r, h, 0.0, colour);
        } else if i + 1 == n {
            cx.c.rect(at, y, sw_, h, r, colour);
            cx.c.rect(at, y, sw_ - r, h, 0.0, colour);
        } else {
            cx.c.rect(at, y, sw_, h, 0.0, colour);
        }
        at += sw;
    }
}

/// A thin meter: a track and a fill, `h` tall.
pub fn thin_meter(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, fraction: f32, colour: Rgba) {
    let track = cx.p.track;
    cx.c.rect(x, y, w, h, h / 2.0, track);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        cx.c.rect(x, y, (w * f).max(h), h, h / 2.0, colour);
    }
}

/// A word over a control ("Kind"). Returns the height used.
pub fn label(cx: &mut Ctx, x: f32, y: f32, words: &str) -> f32 {
    let t = Text::new(ty::CAPTION, cx.p.ink2).medium();
    cx.c.text(words, x + 2.0, y, t);
    t.lh() + 4.0
}

/// A line under a field: a hint in ink-3, or a problem in red. Returns the height used.
pub fn hint(cx: &mut Ctx, x: f32, y: f32, w: f32, words: &str, problem: bool) -> f32 {
    let t = Text::new(ty::CAPTION, if problem { cx.p.held_text } else { cx.p.ink3 }).line(1.35);
    6.0 + cx.c.para(words, x + 2.0, y + 6.0, w - 4.0, t, 3)
}

/// A legend dot, a label, a figure and a share: one line of an allocation.
#[allow(clippy::too_many_arguments)]
pub fn share_row(cx: &mut Ctx, x: f32, y: f32, w: f32, label: &str, colour: Rgba, value: &str, share: &str) -> f32 {
    let p = cx.p;
    let h = 32.0;
    cx.c.circle(x + 5.0, y + h / 2.0, 5.0, colour);
    let st = Text::new(13.0, p.ink3).right();
    let sw = cx.c.measure(share, st);
    cx.c.text(share, x + w, y + (h - st.lh()) / 2.0, st);
    let vt = Text::new(15.0, p.ink).medium().right();
    let vw = cx.c.measure(value, vt);
    cx.c.text(value, x + w - sw - 10.0, y + (h - vt.lh()) / 2.0, vt);
    let lt = Text::new(15.0, p.ink2);
    cx.c.text_fit(label, x + 18.0, y + (h - lt.lh()) / 2.0, w - 18.0 - sw - vw - 20.0, lt);
    h
}

/// A card's title in the premium look: 16 semibold at its top left, an optional figure at its
/// right in `colour`. Returns the height used.
pub fn card_title(cx: &mut Ctx, x: f32, y: f32, w: f32, title: &str, figure: Option<(&str, Rgba)>) -> f32 {
    let t = Text::new(16.0, cx.p.ink).semi();
    cx.c.text(title, x + 18.0, y + 18.0, t);
    if let Some((f, c)) = figure {
        cx.c.text(f, x + w - 18.0, y + 18.0, Text::new(15.0, c).semi().right());
    }
    18.0 + t.lh()
}

/// A value over time as a brass line on a soft brass fill, with three amounts up the left and a
/// month's initial under each point. `labels` has one entry per value.
#[allow(clippy::too_many_arguments)]
pub fn value_chart(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, values: &[i64], labels: &[String], tick_text: impl Fn(i64) -> String) {
    let p = cx.p;
    if values.is_empty() {
        return;
    }
    let lo = *values.iter().min().unwrap_or(&0);
    let hi = *values.iter().max().unwrap_or(&0);
    let (lo, hi, step) = nice_range(lo, hi);
    let lt = Text::new(11.0, p.ink3);
    let ticks: Vec<i64> = (0..=2).map(|i| lo + step * i).collect();
    let axis_w = ticks.iter().map(|t| cx.c.measure(&tick_text(*t), lt)).fold(0.0, f32::max) + 10.0;
    let base_h = 20.0;
    let (cx0, cw, top, ch) = (x + axis_w, w - axis_w, y + 6.0, h - base_h - 6.0);
    let span = (hi - lo).max(1) as f32;
    let at = |v: i64| top + ch - ch * (v - lo) as f32 / span;
    for t in &ticks {
        let ty_ = at(*t);
        cx.c.rect(cx0, ty_, cw, 1.0, 0.0, p.line_subtle);
        cx.c.text(&tick_text(*t), x, ty_ - lt.lh() / 2.0, lt);
    }
    let n = values.len();
    let px = |i: usize| if n == 1 { cx0 + cw / 2.0 } else { cx0 + 4.0 + (cw - 8.0) * i as f32 / (n - 1) as f32 };
    // The fill: thin columns from the line down, fading with depth.
    let base = top + ch;
    let (x_first, x_last) = (px(0), px(n - 1));
    let mut col = x_first;
    while col < x_last {
        let f = (col - x_first) / (x_last - x_first).max(1.0) * (n - 1) as f32;
        let i = (f.floor() as usize).min(n.saturating_sub(2));
        let t = f - i as f32;
        let v = if n == 1 { values[0] as f32 } else { values[i] as f32 + (values[i + 1] - values[i]) as f32 * t };
        let yy = top + ch - ch * (v - lo as f32) / span;
        let depth = ((yy - top) / ch).clamp(0.0, 1.0);
        let wcol = (x_last - col).min(1.5);
        cx.c.rect_grad(col, yy, wcol, base - yy, 0.0, p.signal.a(0.34 - 0.2 * depth), p.signal.a(0.06));
        col += 1.5;
    }
    // The line, stamped along each segment.
    for i in 0..n.saturating_sub(1) {
        let (x1, y1, x2, y2) = (px(i), at(values[i]), px(i + 1), at(values[i + 1]));
        let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        let steps = (len / 0.7).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            cx.c.circle(x1 + (x2 - x1) * t, y1 + (y2 - y1) * t, 1.1, p.signal);
        }
    }
    cx.c.circle(px(n - 1), at(values[n - 1]), 3.5, p.signal);
    let mt = Text::new(10.5, p.ink3).center();
    for (i, l) in labels.iter().enumerate().take(n) {
        cx.c.text(l, px(i), base + 6.0, mt);
    }
}

/// A floor, a ceiling and a step for three gridlines that hold `lo..=hi` at round amounts.
pub fn nice_range(lo: i64, hi: i64) -> (i64, i64, i64) {
    let span = (hi - lo).max((hi.abs() / 10).max(100));
    let mut mag = 10f64.powf((span as f64 / 2.0).log10().floor()).max(1.0);
    loop {
        for m in [1.0, 2.0, 2.5, 5.0] {
            let step = ((m * mag).round() as i64).max(1);
            if step * 2 < span {
                continue;
            }
            let floor = lo.div_euclid(step) * step;
            if floor + 2 * step >= hi {
                return (floor, floor + 2 * step, step);
            }
        }
        mag *= 10.0;
    }
}

/// The keypad: digits, the decimal point and a backspace, three to a row, on quiet keys.
/// Returns the key tapped.
pub fn keypad(cx: &mut Ctx, top: f32, decimal: bool) -> Option<&'static str> {
    let p = cx.p;
    let (kw, kh, gap) = ((W - 16.0) / 3.0, KEYPAD_KEY, 8.0);
    let keys = ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", "⌫"];
    let mut out = None;
    for (i, k) in keys.iter().enumerate() {
        let (x, y) = (PAD + (i % 3) as f32 * (kw + gap), top + (i / 3) as f32 * (kh + gap));
        if *k == "." && !decimal {
            continue;
        }
        cx.c.rect(x, y, kw, kh, KEY_R, p.card2);
        if cx.pressed(x, y, kw, kh) {
            cx.c.rect(x, y, kw, kh, KEY_R, p.wash);
        }
        if *k == "⌫" {
            cx.c.icon("arrow-left", x + kw / 2.0, y + kh / 2.0, 20.0, p.ink);
        } else {
            let t = Text::new(22.0, p.ink).medium().center();
            cx.c.text(k, x + kw / 2.0, y + (kh - t.lh()) / 2.0, t);
        }
        if cx.tap(x, y, kw, kh) {
            out = Some(*k);
        }
    }
    out
}

pub const KEYPAD_KEY: f32 = 52.0;
/// The keypad's height.
pub const KEYPAD_H: f32 = 4.0 * KEYPAD_KEY + 3.0 * 8.0;

/// What a [`confirm`] dialog was answered with.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Answer {
    Act,
    Alternate,
    Cancel,
}

/// A dialog that asks before something that cannot be undone, naming it, with an optional gentler
/// way out above the two keys ("Archive"). The act is the red key.
pub fn confirm(cx: &mut Ctx, title: &str, body: &str, act: &str, alternate: Option<&str>) -> Option<Answer> {
    let p = cx.p;
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    let any_tap = cx.ui.tap.is_some();
    let screen_h = cx.c.h;
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, screen_h, 0.0, Rgba::hex(0x000000).a(if p.light { 0.4 } else { 0.7 }));
    let w = WIDTH - 48.0;
    let bt = Text::new(ty::UI, p.ink2).line(1.4);
    let alt_h = if alternate.is_some() { 48.0 + 10.0 } else { 0.0 };
    let chrome = 20.0 + 24.0 + 8.0 + 20.0 + alt_h + 48.0 + 20.0;
    // As many lines as the screen holds around the keys, so the words near the end (what can't
    // be undone, what Archive does) are cut only on a screen too short for them.
    let lines = (((screen_h - 48.0 - chrome) / bt.lh()).floor() as usize).max(4);
    let bh = cx.c.para_h(body, w - 40.0, bt, lines);
    let h = chrome + bh;
    let (x, y) = (24.0, (screen_h - h) / 2.0);
    cx.c.rect(x, y, w, h, CARD_R, p.raised);
    cx.c.outline(x, y, w, h, CARD_R, 1.0, p.line_subtle);
    // Fitted to the card: a long account name would otherwise run off its edge.
    cx.c.text_fit(title, x + 20.0, y + 20.0, w - 40.0, Text::new(ty::TITLE, p.ink).semi());
    cx.c.para(body, x + 20.0, y + 52.0, w - 40.0, bt, lines);
    let mut by = y + h - 20.0 - 48.0 - alt_h;
    let mut out = None;
    if let Some(a) = alternate {
        if kit::key(cx, x + 20.0, by, w - 40.0, 48.0, a, None, KeyKind::Plain) {
            out = Some(Answer::Alternate);
        }
        by += 58.0;
    }
    let bw = (w - 40.0 - 10.0) / 2.0;
    if kit::key(cx, x + 20.0, by, bw, 48.0, "Cancel", None, KeyKind::Plain) {
        out = Some(Answer::Cancel);
    }
    let kx = x + 20.0 + bw + 10.0;
    cx.c.rect(kx, by, bw, 48.0, KEY_R, p.held);
    if cx.pressed(kx, by, bw, 48.0) {
        cx.c.rect(kx, by, bw, 48.0, KEY_R, p.wash);
    }
    let t = Text::new(ty::BODY, p.on_ink).semi().center();
    cx.c.text(act, kx + bw / 2.0, by + (48.0 - t.lh()) / 2.0, t);
    if cx.tap(kx, by, bw, 48.0) {
        out = Some(Answer::Act);
    }
    if out.is_none() && any_tap && cx.ui.tap.is_some_and(|(tx, ty_)| !(tx >= x && tx < x + w && ty_ >= y && ty_ < y + h)) {
        out = Some(Answer::Cancel);
    }
    cx.ui.tap = None;
    cx.c.dy = saved_dy;
    out
}

/// A row that does one thing, as words in a card: "Delete account" in red, "See its entries".
/// Returns whether it was tapped. 56 tall.
pub fn action_row(cx: &mut Ctx, y: f32, glyph: &'static str, words: &str, danger: bool) -> bool {
    let p = cx.p;
    let h = 56.0;
    let tapped = kit::card_button(cx, PAD, y, W, h);
    let c = if danger { p.held_text } else { p.ink };
    cx.c.icon(glyph, PAD + 18.0 + 9.0, y + h / 2.0, 18.0, c);
    let t = Text::new(15.0, c).medium();
    cx.c.text(words, PAD + 18.0 + 30.0, y + (h - t.lh()) / 2.0, t);
    tapped
}

pub const ACTION_H: f32 = 56.0;
