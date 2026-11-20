//! Insights' charts, drawn on the canvas from boxes, circles and arcs (Kotlin: ui/kit/Charts.kt):
//! this month's running spend against last month's, the month's line against its pace, six months
//! of bars, the days as a heat grid, and a stacked bar of where the money went. Each says what was
//! tapped.

use relay_money::money::MoneyFormatter;

use crate::draw::{Canvas, Rgba, Text};
use crate::kit::{Ctx, ty};

/// The pace card's chart: this period's running spend (brass) over last period's (quiet), day by
/// day for `days` days, on a scale of round amounts (`ticks`, from zero) labelled on the left and
/// the days under it. The picked day is marked on both lines. A tap picks the nearest day.
#[allow(clippy::too_many_arguments)]
pub fn pace_lines(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, this: &[i64], last: &[i64], days: usize, ticks: &[i64], fmt: &MoneyFormatter, picked: Option<usize>) -> Option<usize> {
    let p = cx.p;
    let lt = Text::new(11.0, p.ink3).medium().right();
    let label_w = ticks.iter().map(|t| cx.c.measure(&fmt.format_whole(*t), lt)).fold(0.0, f32::max) + 8.0;
    let (gx, gw) = (x + label_w, w - label_w);
    let gh = h - 20.0;
    let top_value = ticks.last().copied().unwrap_or(1).max(1) as f32;
    let span = (days.max(2) - 1) as f32;
    let px = |i: usize| gx + gw * i as f32 / span;
    let py = |v: i64| y + gh - gh * v as f32 / top_value;
    for t in ticks {
        let ty_ = py(*t);
        cx.c.rect(gx, ty_ - 0.5, gw, 1.0, 0.0, if *t == 0 { p.track } else { p.line_subtle });
        cx.c.text(&fmt.format_whole(*t), gx - 8.0, ty_ - lt.lh() / 2.0, lt);
    }
    // The days under it: every few, so seven or fewer show, and always the last.
    let every = days.max(1).div_ceil(7).max(1);
    let dt = Text::new(11.0, p.ink3).medium().center();
    for d in (0..days).step_by(every) {
        cx.c.text(&(d + 1).to_string(), px(d), y + gh + 6.0, dt);
    }
    let quiet = p.ink3.a(if p.light { 0.7 } else { 0.85 });
    let line = |cx: &mut Ctx, vals: &[i64], width: f32, colour: Rgba| {
        let n = vals.len().min(days);
        for i in 1..n {
            stroke(cx.c, (px(i - 1), py(vals[i - 1])), (px(i), py(vals[i])), width, colour);
        }
    };
    line(cx, last, 2.0, quiet);
    line(cx, this, 2.6, p.signal);
    let shown = picked.filter(|d| *d < this.len().min(days)).or(this.len().min(days).checked_sub(1));
    if let Some(d) = shown {
        if picked.is_some() {
            cx.c.rect(px(d) - 0.75, y, 1.5, gh, 0.75, p.ink3.a(0.5));
            if let Some(v) = last.get(d) {
                cx.c.circle(px(d), py(*v), 4.5, p.slab);
                cx.c.circle(px(d), py(*v), 3.0, quiet);
            }
        }
        cx.c.circle(px(d), py(this[d]), 6.0, p.slab);
        cx.c.circle(px(d), py(this[d]), 4.0, p.signal);
    }
    let tap = cx.ui.tap.map(|(tx, _)| tx);
    if cx.tap(x, y - 8.0, w, h + 8.0) && !this.is_empty() {
        let f = ((tap.unwrap_or(gx) - gx) / gw).clamp(0.0, 1.0);
        return Some(((f * span).round() as usize).min(this.len().min(days) - 1));
    }
    None
}

/// A stroke from one point to another, laid as overlapping dots (the canvas has no paths).
pub fn stroke(c: &mut Canvas, (x1, y1): (f32, f32), (x2, y2): (f32, f32), width: f32, colour: Rgba) {
    let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let steps = (len / (width * 0.45)).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        c.circle(x1 + (x2 - x1) * t, y1 + (y2 - y1) * t, width / 2.0, colour);
    }
}

/// A dashed stroke: `on` units drawn, `off` skipped.
pub fn dashed(c: &mut Canvas, (x1, y1): (f32, f32), (x2, y2): (f32, f32), width: f32, on: f32, off: f32, colour: Rgba) {
    let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    if len <= 0.0 {
        return;
    }
    let (dx, dy) = ((x2 - x1) / len, (y2 - y1) / len);
    let mut at = 0.0;
    while at < len {
        let end = (at + on).min(len);
        stroke(c, (x1 + dx * at, y1 + dy * at), (x1 + dx * end, y1 + dy * end), width, colour);
        at += on + off;
    }
}

/// The month as a running line: spent by the end of each day so far over `days`, a dashed line
/// to the budget, a soft wash under the line, and the picked day marked. A tap picks the nearest
/// day drawn.
#[allow(clippy::too_many_arguments)]
pub fn spend_line(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, cumulative: &[i64], days: i64, budget: Option<i64>, picked: Option<usize>) -> Option<usize> {
    let p = cx.p;
    let top_value = cumulative.iter().copied().max().unwrap_or(0).max(budget.unwrap_or(0)).max(1) as f32 * 1.08;
    let base = y + h;
    let span = (days - 1).max(1) as f32;
    let px = |i: usize| x + w * i as f32 / span;
    let py = |v: i64| base - h * v as f32 / top_value;
    cx.c.rect(x, base - 0.5, w, 1.0, 0.0, p.track);
    if let Some(b) = budget {
        cx.c.rect(x, py(b) - 0.5, w, 1.0, 0.0, p.ink3.a(0.35));
        dashed(cx.c, (x, base), (x + w, py(b)), 1.6, 5.0, 5.0, p.ink3);
    }
    let over = budget.is_some_and(|b| cumulative.last().is_some_and(|v| *v > b));
    let ink = if over { p.held } else { p.signal };
    if !cumulative.is_empty() {
        // The wash under the line: thin columns fading to nothing at the axis.
        let last_x = px(cumulative.len() - 1);
        let mut cx_ = x;
        while cx_ < last_x {
            let f = (cx_ - x) / w * span;
            let i = f.floor() as usize;
            let t = f - i as f32;
            let a = cumulative[i.min(cumulative.len() - 1)] as f32;
            let b = cumulative[(i + 1).min(cumulative.len() - 1)] as f32;
            let v = a + (b - a) * t;
            let top = base - h * v / top_value;
            let cw = 2.0f32.min(last_x - cx_);
            cx.c.rect_grad(cx_, top, cw, base - top, 0.0, ink.a(0.24), ink.a(0.0));
            cx_ += 2.0;
        }
        for i in 1..cumulative.len() {
            stroke(cx.c, (px(i - 1), py(cumulative[i - 1])), (px(i), py(cumulative[i])), 2.6, ink);
        }
        let end = cumulative.len() - 1;
        let shown = picked.filter(|d| *d < cumulative.len()).unwrap_or(end);
        let (dx, dy) = (px(shown), py(cumulative[shown]));
        if picked.is_some() {
            cx.c.rect(dx - 0.75, y, 1.5, h, 0.75, p.ink3.a(0.5));
        }
        cx.c.circle(dx, dy, 6.0, p.raised);
        cx.c.circle(dx, dy, 4.0, ink);
    }
    let tap = cx.ui.tap.map(|(tx, ty_)| (tx, ty_ - cx.c.dy));
    if cx.tap(x - 8.0, y - 8.0, w + 16.0, h + 16.0) && !cumulative.is_empty() {
        let (tx, _) = tap.unwrap_or_default();
        let f = ((tx - x) / w).clamp(0.0, 1.0);
        return Some(((f * span).round() as usize).min(cumulative.len() - 1));
    }
    None
}

/// Months side by side: what went out (in ink) beside what came in (green), the highlighted
/// month full strength, the month's short name under each. A tap picks a month.
#[allow(clippy::too_many_arguments)]
pub fn trend_bars(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, spent: &[i64], income: &[i64], labels: &[String], highlight: usize) -> Option<usize> {
    let p = cx.p;
    let n = spent.len().max(1);
    let top_value = spent.iter().chain(income).copied().max().unwrap_or(0).max(1) as f32;
    let slot = w / n as f32;
    let bar = (slot * 0.34).min(22.0);
    let base = y + h;
    cx.c.rect(x, base - 0.5, w, 1.0, 0.0, p.track);
    let mut out = None;
    for (i, &out_i) in spent.iter().enumerate() {
        let on = i == highlight;
        let mid = x + slot * i as f32 + slot / 2.0;
        let group = bar + 3.0 + bar * 0.6;
        let bx = mid - group / 2.0;
        if on {
            cx.c.rect(x + slot * i as f32 + 2.0, y - 6.0, slot - 4.0, h + 32.0, 12.0, p.wash);
        }
        let sh = (h * out_i as f32 / top_value).max(if out_i > 0 { 4.0 } else { 0.0 });
        let ih = (h * income.get(i).copied().unwrap_or(0) as f32 / top_value).max(if income.get(i).copied().unwrap_or(0) > 0 { 4.0 } else { 0.0 });
        if sh > 0.0 {
            cx.c.rect(bx, base - sh, bar, sh, 4.0, if on { p.ink } else { p.ink.a(0.28) });
        }
        if ih > 0.0 {
            cx.c.rect(bx + bar + 3.0, base - ih, bar * 0.6, ih, 3.0, if on { p.live } else { p.live.a(0.55) });
        }
        let t = Text::new(ty::CAPTION, if on { p.ink } else { p.ink3 }).center();
        let t = if on { t.semi() } else { t.medium() };
        if let Some(l) = labels.get(i) {
            cx.c.text(l, mid, base + 8.0, t);
        }
        if cx.tap(x + slot * i as f32, y - 6.0, slot, h + 32.0) {
            out = Some(i);
        }
    }
    out
}

/// The height [`heat_grid`] takes for a period of `days` days starting `lead` cells in.
pub fn heat_grid_h(w: f32, days: usize, lead: usize) -> f32 {
    let cell = (w - 6.0 * 6.0) / 7.0;
    let rows = (lead + days).div_ceil(7);
    20.0 + rows as f32 * (cell * 0.82 + 6.0)
}

/// The days as a grid, a week to a row: brighter is more spent, days still ahead faint, the
/// picked day ringed. Returns the day tapped.
#[allow(clippy::too_many_arguments)]
pub fn heat_grid(cx: &mut Ctx, x: f32, y: f32, w: f32, heat: &[u8], lead: usize, elapsed: usize, weekdays: &[&str; 7], day_numbers: &[i8], picked: Option<usize>) -> Option<usize> {
    let p = cx.p;
    let gap = 6.0;
    let cell = (w - gap * 6.0) / 7.0;
    let ch = cell * 0.82;
    for (i, d) in weekdays.iter().enumerate() {
        cx.c.text(d, x + i as f32 * (cell + gap) + cell / 2.0, y, Text::new(ty::CAPTION, p.ink3).semi().center());
    }
    let gy = y + 20.0;
    let mut out = None;
    for (day, step) in heat.iter().enumerate() {
        let k = lead + day;
        let (cxp, cyp) = (x + (k % 7) as f32 * (cell + gap), gy + (k / 7) as f32 * (ch + gap));
        let ahead = day >= elapsed;
        let fill = if ahead {
            p.track.a(0.5)
        } else if *step == 0 {
            p.track
        } else {
            p.signal.a(0.16 + 0.2 * *step as f32)
        };
        cx.c.rect(cxp, cyp, cell, ch, 10.0, fill);
        let on = picked == Some(day);
        if on {
            cx.c.outline(cxp, cyp, cell, ch, 10.0, 1.8, p.ink);
        }
        let ink = if on && *step < 3 {
            p.ink
        } else if ahead {
            p.ink3.a(0.5)
        } else if *step >= 3 {
            if p.light { Rgba::WHITE } else { p.on_ink }
        } else {
            p.ink2
        };
        let t = Text::new(ty::CAPTION, ink).center();
        let t = if on || *step >= 3 { t.semi() } else { t.medium() };
        if let Some(n) = day_numbers.get(day) {
            cx.c.text(&n.to_string(), cxp + cell / 2.0, cyp + (ch - t.lh()) / 2.0, t);
        }
        if !ahead && cx.tap(cxp, cyp, cell, ch) {
            out = Some(day);
        }
    }
    out
}

/// One bar split by share, in each part's colour, on the track.
pub fn stack_bar(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, parts: &[(Rgba, i64)]) {
    let track = cx.p.track;
    cx.c.rect(x, y, w, h, h / 2.0, track);
    let total: i64 = parts.iter().map(|(_, v)| (*v).max(0)).sum();
    if total <= 0 {
        return;
    }
    cx.c.push_clip(x, y + cx.c.dy, w, h);
    let mut at = x;
    for (i, (c, v)) in parts.iter().enumerate() {
        let pw = w * (*v).max(0) as f32 / total as f32;
        if pw <= 0.0 {
            continue;
        }
        // Each part a pill, overlapping the next a little so no seam shows the ground.
        let last = i + 1 == parts.len();
        cx.c.rect(at - if i == 0 { 0.0 } else { h }, y, pw + if i == 0 { 0.0 } else { h } + if last { 0.0 } else { 1.0 }, h, h / 2.0, *c);
        at += pw;
    }
    cx.c.pop_clip();
}
