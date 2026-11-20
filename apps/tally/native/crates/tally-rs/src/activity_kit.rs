//! The pieces the ledger's pages share, in the premium look (DESIGN.md): an entry's row (the kit's,
//! with a transfer's own badge), a day's header, chips and a sideways row of them, the search field,
//! a centred field for a note, small cards with a figure, and a quiet glyph key that can be off.
//! Used by Entries, Transactions, Search, Insights and the entry editor.

use relay_money::Date;
use relay_money::model::{AccountType, TxType};
use relay_money::money::MoneyFormatter;
use relay_money::views::Tx;

use crate::activity_data::{self, DayGroup};
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, Ctx, PAD, ty};
use crate::theme;
use crate::ui::HScroll;

pub const W: f32 = WIDTH - PAD * 2.0;
/// An entry's row: the kit's.
pub const ROW_H: f32 = kit::ENTRY_H;
/// A day's header over its rows.
pub const DAY_H: f32 = 36.0;
/// A chip's height.
pub const CHIP_H: f32 = 36.0;

/// Is any of this content band on screen? Long lists skip drawing what cannot be seen.
pub fn visible(cx: &Ctx, y: f32, h: f32) -> bool {
    let top = y + cx.c.dy;
    top + h >= -40.0 && top <= cx.c.h + 40.0
}

/// The colour a transfer and an account wear: the blue of the category colours.
pub fn money_hue() -> Rgba {
    theme::category_color(Some(2))
}

/// A badge with a glyph rather than a category's icon: transfers and accounts.
pub fn glyph_badge(cx: &mut Ctx, x: f32, y: f32, d: f32, glyph: &'static str, colour: Rgba) {
    cx.c.rect(x, y, d, d, (d * 0.32).round(), theme::soft_of(colour));
    cx.c.icon(glyph, x + d / 2.0, y + d / 2.0, d * 0.45, colour);
}

/// An account's badge: its kind's glyph.
pub fn account_badge(cx: &mut Ctx, x: f32, y: f32, d: f32, t: AccountType) {
    glyph_badge(cx, x, y, d, activity_data::account_glyph(t), money_hue());
}

/// An entry's amount as a row shows it: spending with a minus, income signed and green, a transfer
/// quieter.
pub fn amount_of(cx: &Ctx, t: &Tx, fmt: &MoneyFormatter) -> (String, Rgba) {
    match t.r#type {
        TxType::Income => (format!("+{}", fmt.format(t.amount)), cx.p.live_text),
        TxType::Transfer => (fmt.format(t.amount), cx.p.ink2),
        TxType::Expense => (format!("−{}", fmt.format(t.amount)), cx.p.ink),
    }
}

/// A signed sum: "+$2,003.00" or "−$70.87".
pub fn signed(v: i64, fmt: &MoneyFormatter) -> String {
    if v > 0 {
        format!("+{}", fmt.format(v))
    } else if v < 0 {
        format!("−{}", fmt.format(-v))
    } else {
        fmt.format(0)
    }
}

/// One entry, as the kit's row draws it: its badge, its name over its category and account, its
/// amount over `when`. A transfer wears the two arrows. Rows off screen only take their tap.
#[allow(clippy::too_many_arguments)]
pub fn entry_row(cx: &mut Ctx, x: f32, y: f32, w: f32, t: &Tx, fmt: &MoneyFormatter, when: &str) -> bool {
    if !visible(cx, y, ROW_H) {
        return cx.tap(x - 8.0, y, w + 16.0, ROW_H);
    }
    let (amount, colour) = amount_of(cx, t, fmt);
    let name = activity_data::entry_name(t);
    let line = activity_data::entry_line(t, None);
    if t.r#type != TxType::Transfer {
        let icon = t.icon.as_deref().unwrap_or("dots");
        return kit::entry_row(cx, x, y, w, icon, theme::category_color(t.color), &name, &line, &amount, colour, when);
    }
    // The kit's row, with the transfer's own badge (the kit's draws category icons only).
    if cx.pressed(x - 8.0, y, w + 16.0, ROW_H) {
        let wash = cx.p.wash;
        cx.c.rect(x - 8.0, y + 2.0, w + 16.0, ROW_H - 4.0, 14.0, wash);
    }
    glyph_badge(cx, x, y + 10.0, 44.0, "transfer", money_hue());
    let at = Text::new(16.0, colour).medium().right();
    let ww = Text::new(13.0, cx.p.ink3).right();
    let aw = cx.c.measure(&amount, at).max(cx.c.measure(when, ww));
    let tx = x + 58.0;
    let room = (w - 58.0 - aw - 12.0).max(40.0);
    cx.c.text_fit(&name, tx, y + 12.0, room, Text::new(16.0, cx.p.ink).medium());
    cx.c.text_fit(&line, tx, y + 35.0, room, Text::new(13.0, cx.p.ink2));
    cx.c.text(&amount, x + w, y + 12.0, at);
    cx.c.text(when, x + w, y + 35.0, ww);
    cx.tap(x - 8.0, y, w + 16.0, ROW_H)
}

/// What a row says under its amount in a list grouped by day: a bill's posting repeats.
pub fn when_in_day(t: &Tx, repeats: &std::collections::HashSet<i64>) -> &'static str {
    if repeats.contains(&t.id) {
        "Repeats"
    } else {
        match t.r#type {
            TxType::Transfer => "Transfer",
            _ => "",
        }
    }
}

/// A day's name for its header: "Today", "Yesterday", "Monday 17", or "Monday 17 Oct" outside
/// today's month.
pub fn day_name(d: Date, today: Date) -> String {
    match (today - d).get_days() {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ if d.year() == today.year() && d.month() == today.month() => d.strftime("%A %-d").to_string(),
        _ if d.year() == today.year() => d.strftime("%A %-d %b").to_string(),
        _ => d.strftime("%a %-d %b %Y").to_string(),
    }
}

/// A short date for a row's right side: "17 Nov", with the year when it is not this one.
pub fn short_day(d: &str, today: Date) -> String {
    match d.parse::<Date>() {
        Ok(d) if d.year() == today.year() => d.strftime("%-d %b").to_string(),
        Ok(d) => d.strftime("%-d %b %Y").to_string(),
        Err(_) => String::new(),
    }
}

/// A day's header: its name on the left and its money on the right.
pub fn day_head(cx: &mut Ctx, y: f32, name: &str, money: &str, colour: Rgba) {
    let p = cx.p;
    let t = Text::new(15.0, p.ink).semi();
    cx.c.text(name, PAD, y + 10.0, t);
    if !money.is_empty() {
        let mt = Text::new(13.0, colour).medium().right();
        cx.c.text(money, PAD + W, y + 10.0 + (t.lh() - mt.lh()) / 2.0, mt);
    }
}

/// Rows' inset inside their card, and the card's margin above the first and under the last.
pub const ROW_IN: f32 = 16.0;
pub const CARD_PAD: f32 = 4.0;

/// The height [`row_card`] takes for `n` rows.
pub fn row_card_h(n: usize) -> f32 {
    CARD_PAD * 2.0 + n as f32 * ROW_H
}

/// Entries on a card, inset, with a hairline between rows. `when` says what goes under each
/// amount. Returns the bottom and the entry tapped.
pub fn row_card<'a>(cx: &mut Ctx, y: f32, rows: impl IntoIterator<Item = &'a Tx>, fmt: &MoneyFormatter, when: impl Fn(&Tx) -> String) -> (f32, Option<i64>) {
    let rows: Vec<&Tx> = rows.into_iter().collect();
    let h = row_card_h(rows.len());
    if visible(cx, y, h) {
        kit::card(cx, PAD, y, W, h);
    }
    let (x, w) = (PAD + ROW_IN, W - ROW_IN * 2.0);
    let mut tapped = None;
    let mut ry = y + CARD_PAD;
    for (i, t) in rows.iter().enumerate() {
        if i > 0 && visible(cx, ry, 1.0) {
            let line = cx.p.line_subtle;
            cx.c.rect(x + 58.0, ry, w - 58.0, 1.0, 0.0, line);
        }
        if entry_row(cx, x, ry, w, t, fmt, &when(t)) {
            tapped = Some(t.id);
        }
        ry += ROW_H;
    }
    (y + h, tapped)
}

/// The day groups: a header with the day's net on the ground, then that day's entries on a card.
/// Returns the bottom and the entry tapped.
pub fn days(cx: &mut Ctx, mut y: f32, days: &[DayGroup], today: Date, fmt: &MoneyFormatter, repeats: &std::collections::HashSet<i64>) -> (f32, Option<i64>) {
    let mut tapped = None;
    for d in days {
        if visible(cx, y, DAY_H) {
            let net = d.income - d.spent;
            let words = if d.spent == 0 && d.income == 0 { String::new() } else { signed(net, fmt) };
            let ink3 = cx.p.ink3;
            day_head(cx, y, &day_name(d.date, today), &words, ink3);
        }
        let (b, t) = row_card(cx, y + DAY_H, &d.rows, fmt, |t| when_in_day(t, repeats).to_string());
        if t.is_some() {
            tapped = t;
        }
        y = b + 14.0;
    }
    (y, tapped)
}

/// A hairline row of a details card: a label on the left, the value on the right (with a
/// category's badge before it, and a line under it), and a chevron when it opens something.
/// Returns (height, tapped).
#[allow(clippy::too_many_arguments)]
pub fn detail_row(cx: &mut Ctx, y: f32, first: bool, label: &str, value: &str, sub: Option<&str>, badge: Option<(&str, Rgba)>, opens: bool) -> (f32, bool) {
    let p = cx.p;
    let (x, w) = (PAD + ROW_IN, W - ROW_IN * 2.0);
    let h = if sub.is_some() { 68.0 } else { 56.0 };
    if !first {
        cx.c.rect(x, y, w, 1.0, 0.0, p.line_subtle);
    }
    if opens && cx.pressed(PAD, y, W, h) {
        cx.c.rect(PAD + 6.0, y + 2.0, W - 12.0, h - 4.0, 14.0, p.wash);
    }
    let lt = Text::new(14.0, p.ink2);
    let lw = cx.c.measure(label, lt);
    let vt = Text::new(15.0, p.ink).medium().right();
    let st = Text::new(13.0, p.ink3).right();
    let mut right = x + w;
    if opens {
        cx.c.icon("chevron-right", right - 6.0, y + h / 2.0, 13.0, p.ink3);
        right -= 22.0;
    }
    let block = vt.lh() + sub.map_or(0.0, |_| 2.0 + st.lh());
    let vy = y + (h - block) / 2.0;
    cx.c.text(label, x, y + (h - lt.lh()) / 2.0, lt);
    let room = right - x - lw - 16.0 - if badge.is_some() { 32.0 } else { 0.0 };
    let shown = cx.c.fit(value, room, vt);
    let vw = cx.c.measure(&shown, vt);
    cx.c.text(&shown, right, vy, vt);
    if let Some((icon, colour)) = badge {
        kit::badge(cx, right - vw - 32.0, vy + (vt.lh() - 24.0) / 2.0, 24.0, icon, colour);
    }
    if let Some(s) = sub {
        cx.c.text_fit(s, right, vy + vt.lh() + 2.0, right - x - lw - 16.0, st);
    }
    (h, opens && cx.tap(PAD, y, W, h))
}

// Chips

/// How wide a chip with these words is.
pub fn chip_w(cx: &mut Ctx, words: &str) -> f32 {
    cx.c.measure(words, Text::new(ty::UI, cx.p.ink).medium()) + 32.0
}

/// A chip, as the kit's pills: the chosen one filled with ink, the rest outlined. Returns whether
/// it was tapped.
pub fn chip(cx: &mut Ctx, x: f32, y: f32, words: &str, on: bool) -> bool {
    let p = cx.p;
    let w = chip_w(cx, words);
    let h = CHIP_H;
    if on {
        cx.c.rect(x, y, w, h, h / 2.0, p.prim);
    } else {
        cx.c.outline(x, y, w, h, h / 2.0, 1.0, edge(cx));
        if cx.pressed(x, y, w, h) {
            cx.c.rect(x, y, w, h, h / 2.0, p.wash);
        }
    }
    let t = Text::new(ty::UI, if on { p.on_ink } else { p.ink }).medium();
    cx.c.text(words, x + 16.0, y + (h - t.lh()) / 2.0, t);
    cx.tap(x, y, w, h)
}

/// A chip's outline: the kit's pill edge.
pub fn edge(cx: &Ctx) -> Rgba {
    if cx.p.light { Rgba::hex(0x000000).a(0.12) } else { Rgba::WHITE.a(0.16) }
}

/// Chips in a row that scrolls sideways. Returns the chip tapped and whether the row moves.
pub fn chip_row(cx: &mut Ctx, hs: &mut HScroll, y: f32, labels: &[&str], selected: Option<usize>, dt: f32) -> (Option<usize>, bool) {
    let gap = 8.0;
    let widths: Vec<f32> = labels.iter().map(|l| chip_w(cx, l)).collect();
    let content = widths.iter().sum::<f32>() + gap * labels.len().saturating_sub(1) as f32;
    let (x0, moving) = kit::carousel(cx, hs, y, CHIP_H, content, dt);
    let mut x = x0;
    let mut out = None;
    for (i, l) in labels.iter().enumerate() {
        if x < WIDTH && x + widths[i] > 0.0 && chip(cx, x, y, l, selected == Some(i)) {
            out = Some(i);
        }
        x += widths[i] + gap;
    }
    kit::carousel_end(cx);
    (out, moving)
}

/// Chips that share the page's width between them, each sized to its words plus an even share of
/// what is left, so all of them show with nothing to scroll. Returns the chip tapped.
pub fn chips_fit(cx: &mut Ctx, y: f32, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let p = cx.p;
    let gap = 8.0;
    let n = labels.len().max(1) as f32;
    let t = Text::new(ty::UI, p.ink).medium();
    let words: Vec<f32> = labels.iter().map(|l| cx.c.measure(l, t)).collect();
    let spare = ((W - gap * (n - 1.0) - words.iter().sum::<f32>()) / n).max(12.0);
    let mut x = PAD;
    let mut out = None;
    for (i, l) in labels.iter().enumerate() {
        let w = words[i] + spare;
        let on = selected == Some(i);
        let h = CHIP_H;
        if on {
            cx.c.rect(x, y, w, h, h / 2.0, p.prim);
        } else {
            cx.c.outline(x, y, w, h, h / 2.0, 1.0, edge(cx));
            if cx.pressed(x, y, w, h) {
                cx.c.rect(x, y, w, h, h / 2.0, p.wash);
            }
        }
        let tt = Text::new(ty::UI, if on { p.on_ink } else { p.ink }).medium().center();
        cx.c.text_fit(l, x + w / 2.0, y + (h - tt.lh()) / 2.0, w - 8.0, tt);
        if cx.tap(x, y, w, h) {
            out = Some(i);
        }
        x += w + gap;
    }
    out
}

/// What leads a tall chip's words.
#[derive(Clone, Copy)]
pub enum Lead<'a> {
    /// A category's icon in its colour.
    Category(&'a str, Rgba),
    /// A glyph in ink.
    Glyph(&'static str),
}

/// A taller chip on the key colour, for what an entry is made of: a category (tinted and outlined
/// in its colour when chosen), an account, a day. `w` none sizes it to its words. Returns
/// (width, tapped).
#[allow(clippy::too_many_arguments)]
pub fn tag_chip(cx: &mut Ctx, x: f32, y: f32, w: Option<f32>, h: f32, lead: Lead, words: &str, on: bool) -> (f32, bool) {
    let p = cx.p;
    let t = Text::new(ty::UI, p.ink).medium();
    let lead_w = match lead {
        Lead::Category(..) => 26.0 + 8.0,
        Lead::Glyph(_) => 16.0 + 8.0,
    };
    let w = w.unwrap_or_else(|| 12.0 + lead_w + cx.c.measure(words, t) + 14.0);
    let r = 14.0;
    match lead {
        Lead::Category(_, colour) if on => {
            cx.c.rect(x, y, w, h, r, theme::soft_of(colour));
            cx.c.outline(x, y, w, h, r, 1.5, colour);
        }
        _ => cx.c.rect(x, y, w, h, r, p.card2),
    }
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, r, p.wash);
    }
    let mut tx = x + 12.0;
    match lead {
        Lead::Category(icon, colour) => {
            if !on {
                cx.c.rect(tx, y + (h - 26.0) / 2.0, 26.0, 26.0, 8.0, theme::soft_of(colour));
            }
            cx.c.category_icon(icon, tx + 13.0, y + h / 2.0, 15.0, colour);
            tx += 26.0 + 8.0;
        }
        Lead::Glyph(g) => {
            cx.c.icon(g, tx + 8.0, y + h / 2.0, 16.0, p.ink2);
            tx += 16.0 + 8.0;
        }
    }
    cx.c.text_fit(words, tx, y + (h - t.lh()) / 2.0, x + w - 12.0 - tx, t);
    (w, cx.tap(x, y, w, h))
}

// Fields

/// The search field: the glyph, the words, and a clear key once something is typed. A firmer
/// quiet edge while it has the keyboard. Returns its height.
pub fn search_box(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, placeholder: &str, value: &mut String) -> f32 {
    let p = cx.p;
    let h = 44.0;
    let r = 14.0;
    let focused = cx.focus.is(id);
    cx.c.rect(x, y, w, h, r, p.slab);
    if focused {
        cx.c.outline(x, y, w, h, r, 1.5, p.ink3);
    } else {
        let l = p.line_subtle;
        cx.c.outline(x, y, w, h, r, 1.0, l);
    }
    cx.c.icon("search", x + 20.0, y + h / 2.0, 17.0, p.ink2);
    // The clear key takes its tap before the field, which would take the whole row.
    let showing = if focused { !cx.focus.text.is_empty() } else { !value.is_empty() };
    let mut room = w - 40.0 - 12.0;
    if showing {
        room -= 32.0;
        let kx = x + w - 40.0;
        if cx.pressed(kx, y, 40.0, h) {
            cx.c.circle(kx + 20.0, y + h / 2.0, 15.0, p.wash);
        }
        cx.c.circle(kx + 20.0, y + h / 2.0, 9.0, p.ink3);
        cx.c.icon("close", kx + 20.0, y + h / 2.0, 10.0, p.slab);
        if cx.tap(kx, y, 40.0, h) {
            value.clear();
            if focused {
                cx.focus.text.clear();
                cx.host.keyboard(true, "", false);
            }
        }
    }
    // The placeholder sits clear of the caret, which waits at the field's start.
    let blank = if focused { cx.focus.text.is_empty() } else { value.is_empty() };
    if blank && focused {
        cx.c.text_fit(placeholder, x + 46.0, y + (h - Text::new(ty::BODY, p.ink3).lh()) / 2.0, room - 6.0, Text::new(ty::BODY, p.ink3));
    }
    let shown = if blank && focused { "" } else { placeholder };
    crate::widgets::inline(cx, id, x + 40.0, y, room, h, shown, value, false, Text::new(ty::BODY, p.ink));
    h
}

/// Put the keyboard on a field from the page itself, as a field asks for focus on opening.
pub fn focus_field(cx: &mut Ctx, id: &'static str, value: &str) {
    if !cx.focus.is(id) {
        cx.focus.id = Some(id);
        cx.focus.text = value.to_string();
        cx.focus.numeric = false;
        cx.host.keyboard(true, value, false);
    }
}

/// Take the keyboard away from whatever field has it.
pub fn drop_focus(cx: &mut Ctx) {
    if cx.focus.id.take().is_some() {
        cx.focus.text.clear();
        cx.host.keyboard(false, "", false);
    }
}

/// A field with no box whose words sit centred on `mid`: the note under an entry's amount. A brass
/// caret while it has the keyboard. Returns whether this tap gave it the keyboard.
#[allow(clippy::too_many_arguments)]
pub fn centred_field(cx: &mut Ctx, id: &'static str, mid: f32, y: f32, w: f32, h: f32, placeholder: &str, value: &mut String, style: Text) -> bool {
    let p = cx.p;
    let focused = cx.focus.is(id);
    if focused {
        *value = cx.focus.text.clone();
        cx.focus.at = Some((y, h));
    }
    let t = if value.is_empty() { style.color(p.ink3) } else { style };
    let words = if value.is_empty() { placeholder } else { value.as_str() };
    let caret = if focused { 6.0 } else { 0.0 };
    let shown = cx.c.fit(words, w - caret, t);
    let tw = cx.c.measure(&shown, t);
    let x = mid - (tw + caret) / 2.0;
    cx.c.text(&shown, x, y + (h - t.lh()) / 2.0, t);
    if focused {
        let cx_ = if value.is_empty() { x - 4.0 } else { x + tw + 3.0 };
        cx.c.rect(cx_, y + (h - 20.0) / 2.0, 2.0, 20.0, 1.0, p.signal);
    }
    if cx.tap(mid - w / 2.0, y, w, h) && !focused {
        cx.focus.id = Some(id);
        cx.focus.text = value.clone();
        cx.focus.numeric = false;
        cx.host.keyboard(true, value, false);
        return true;
    }
    false
}

// Keys and cards

/// A 44-unit glyph key with no fill that reads as off (and takes no tap) when `enabled` is false.
pub fn glyph_key(cx: &mut Ctx, x: f32, y: f32, glyph: &'static str, enabled: bool) -> bool {
    let d = 44.0;
    if enabled && cx.pressed(x, y, d, d) {
        let wash = cx.p.wash;
        cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, wash);
    }
    let ink = if enabled { cx.p.ink } else { cx.p.ink3.a(0.45) };
    cx.c.icon(glyph, x + d / 2.0, y + d / 2.0, 20.0, ink);
    cx.tap(x, y, d, d) && enabled
}

/// A card's own title, small and quiet at its top left ("Where it went"). Returns its height.
pub fn card_title(cx: &mut Ctx, x: f32, y: f32, w: f32, title: &str) -> f32 {
    let t = Text::new(13.0, cx.p.ink2).medium();
    cx.c.text_fit(title, x, y, w, t);
    t.lh()
}

/// A small card's reading: a caption, a figure and an optional line, with the figure in an
/// optional colour. (caption, figure, line, figure colour)
pub type Stat<'a> = (&'a str, String, Option<String>, Option<Rgba>);

const STAT_PAD: f32 = 18.0;

fn stat_h(cx: &mut Ctx, w: f32, line: Option<&str>) -> f32 {
    let lh = line.map(|l| cx.c.para_h(l, w - STAT_PAD * 2.0, Text::new(ty::CAPTION, cx.p.ink3).line(1.3), 2) + 4.0).unwrap_or(0.0);
    STAT_PAD + Text::new(13.0, cx.p.ink2).lh() + 4.0 + Text::new(22.0, cx.p.ink).medium().lh() + lh + STAT_PAD
}

/// Readings two to a row, each on a small card. Returns the bottom.
pub fn stat_grid(cx: &mut Ctx, mut y: f32, tiles: &[Stat]) -> f32 {
    let gap = 12.0;
    let tw = (W - gap) / 2.0;
    for row in tiles.chunks(2) {
        let th = row.iter().map(|t| stat_h(cx, tw, t.2.as_deref())).fold(0.0, f32::max);
        for (i, t) in row.iter().enumerate() {
            let x = PAD + i as f32 * (tw + gap);
            if !visible(cx, y, th) {
                continue;
            }
            kit::card(cx, x, y, tw, th);
            let p = cx.p;
            let mut ty_ = y + STAT_PAD;
            ty_ += card_title(cx, x + STAT_PAD, ty_, tw - STAT_PAD * 2.0, t.0) + 4.0;
            let fig = Text::new(22.0, t.3.unwrap_or(p.ink)).medium();
            cx.c.text_fit(&t.1, x + STAT_PAD, ty_, tw - STAT_PAD * 2.0, fig);
            ty_ += fig.lh() + 4.0;
            if let Some(l) = &t.2 {
                cx.c.para(l, x + STAT_PAD, ty_, tw - STAT_PAD * 2.0, Text::new(ty::CAPTION, p.ink3).line(1.3), 2);
            }
        }
        y += th + gap;
    }
    y - gap
}
