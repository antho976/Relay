//! The controls every page shares, drawn immediately like the rest of the kit: page headers, list
//! rows, toggles, segmented choices, text fields (with the real keyboard), sheets of choices, a
//! calendar, a confirm dialog and the honest empty state.
//!
//! Sheets and dialogs sit over the page. A page that has one open draws its content with the
//! frame's tap held back ([`hold`]), then puts it back ([`release`]) and draws the sheet, so a tap
//! meant for the sheet never lands on what is under it.

use relay_money::Date;

use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{Ctx, KEY_R, PAD, ty};
use crate::ui::Scroll;

/// Which text field has the keyboard, and what it holds while it is being edited.
#[derive(Default, Debug)]
pub struct Focus {
    pub id: Option<&'static str>,
    pub text: String,
    pub numeric: bool,
    /// The focused field's place on its page (content top, height), so the page keeps it in view.
    pub at: Option<(f32, f32)>,
}

impl Focus {
    pub fn is(&self, id: &str) -> bool {
        self.id == Some(id)
    }
}

// Page structure

/// A pushed page's header: the back arrow on the left, the title in the middle, and an optional
/// action on the right: a glyph ("plus") as a key, or words ("Save") as a small filled pill.
/// Returns (back tapped, action tapped) and the height used.
pub fn page_header(cx: &mut Ctx, top: f32, title: &str, action: Option<&str>) -> (bool, bool, f32) {
    let p = cx.p;
    // A page the drawer opens gets the drawer's key; a page opened from another gets Back.
    let back = if cx.top_level {
        crate::kit::menu_key(cx, 10.0, top);
        false
    } else {
        crate::kit::round_key(cx, 10.0, top, "arrow-left")
    };
    let mut acted = false;
    let mut side: f32 = 56.0;
    if let Some(a) = action {
        if crate::glyphs::NAMES.contains(&a) {
            let g: &'static str = crate::glyphs::NAMES.iter().copied().find(|n| *n == a).unwrap_or("plus");
            acted = crate::kit::round_key(cx, WIDTH - 10.0 - 44.0, top, g);
        } else {
            let t = Text::new(ty::UI, p.on_ink).semi();
            let w = cx.c.measure(a, t) + 28.0;
            let x = WIDTH - PAD - w;
            cx.c.rect(x, top + 5.0, w, 34.0, 17.0, p.prim);
            if cx.pressed(x, top, w, 44.0) {
                cx.c.rect(x, top + 5.0, w, 34.0, 17.0, Rgba::hex(0x808080).a(0.25));
            }
            cx.c.text(a, x + 14.0, top + 5.0 + (34.0 - t.lh()) / 2.0, t);
            acted = cx.tap(x - 6.0, top, w + 12.0, 44.0);
            side = side.max(w + PAD + 6.0);
        }
    }
    let t = Text::new(17.0, p.ink).semi().center();
    let room = WIDTH - side * 2.0;
    let shown = cx.c.fit(title, room, t);
    cx.c.text(&shown, WIDTH / 2.0, top + (44.0 - t.lh()) / 2.0, t);
    (back, acted, 44.0 + 14.0)
}

/// Small capitals over a group ("CATEGORY").
pub fn eyebrow(cx: &mut Ctx, x: f32, y: f32, words: &str) -> f32 {
    let p = cx.p;
    cx.c.text(&words.to_uppercase(), x + 2.0, y, Text::new(12.0, p.ink3).semi());
    22.0
}

/// The empty state, drawn with the section's own shape: a glyph, a title and one line of what to
/// do. Returns its height.
pub fn empty(cx: &mut Ctx, x: f32, y: f32, w: f32, glyph: &'static str, title: &str, body: &str) -> f32 {
    let p = cx.p;
    let bt = Text::new(ty::UI, p.ink2).center().line(1.4);
    let bh = cx.c.para_h(body, w - 48.0, bt, 4);
    let h = 24.0 + 48.0 + 12.0 + 22.0 + 6.0 + bh + 24.0;
    crate::kit::card(cx, x, y, w, h);
    cx.c.rect(x + w / 2.0 - 24.0, y + 24.0, 48.0, 48.0, 15.0, p.card2);
    cx.c.icon(glyph, x + w / 2.0, y + 48.0, 22.0, p.ink2);
    cx.c.text(title, x + w / 2.0, y + 24.0 + 48.0 + 12.0, Text::new(ty::TITLE, p.ink).semi().center());
    cx.c.para(body, x + 24.0, y + 24.0 + 48.0 + 12.0 + 28.0, w - 48.0, bt, 4);
    h
}

// Rows

/// A row inside a glass card: a glyph tile, a title, an optional line under it, an optional
/// trailing word, and a chevron when it opens something. 56 tall (64 with a line). Returns
/// whether it was tapped.
#[allow(clippy::too_many_arguments)]
pub fn row(cx: &mut Ctx, x: f32, y: f32, w: f32, glyph: Option<(&'static str, Rgba)>, title: &str, sub: Option<&str>, trailing: Option<&str>, chevron: bool) -> bool {
    let p = cx.p;
    let h = row_h(sub.is_some());
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, 14.0, p.wash);
    }
    let mut tx = x + 12.0;
    if let Some((g, tint)) = glyph {
        // A quiet square unless the glyph's colour means something.
        let quiet = tint.0 == p.ink.0 || tint.0 == p.ink2.0 || tint.0 == p.signal.0;
        let (bg, fg) = if quiet { (p.card2, p.ink) } else { (crate::theme::soft_of(tint), tint) };
        cx.c.rect(tx, y + (h - 36.0) / 2.0, 36.0, 36.0, 11.0, bg);
        cx.c.icon(g, tx + 18.0, y + h / 2.0, 19.0, fg);
        tx += 36.0 + 14.0;
    }
    let mut right = x + w - 12.0;
    if chevron {
        cx.c.icon("chevron-right", right - 7.0, y + h / 2.0, 14.0, p.ink3);
        right -= 22.0;
    }
    if let Some(t) = trailing {
        let tt = Text::new(ty::UI, p.ink2).medium().right();
        let tw = cx.c.measure(t, tt).min(150.0);
        cx.c.text_fit(t, right, y + (h - tt.lh()) / 2.0, 150.0, tt);
        right -= tw + 10.0;
    }
    let room = right - tx;
    let title_t = Text::new(16.0, p.ink).medium();
    match sub {
        Some(s) => {
            cx.c.text_fit(title, tx, y + 11.0, room, title_t);
            cx.c.text_fit(s, tx, y + 34.0, room, Text::new(13.0, p.ink2));
        }
        None => {
            cx.c.text_fit(title, tx, y + (h - title_t.lh()) / 2.0, room, title_t);
        }
    }
    cx.tap(x, y, w, h)
}

pub fn row_h(with_sub: bool) -> f32 {
    if with_sub { 66.0 } else { 58.0 }
}

/// A hairline between rows of a card.
pub fn divider(cx: &mut Ctx, x: f32, y: f32, w: f32) {
    let c = cx.p.line_subtle;
    cx.c.rect(x + 62.0, y, w - 62.0, 1.0, 0.0, c);
}

/// A switch, `on` filled. 52 × 30.
pub fn switch(cx: &mut Ctx, x: f32, y: f32, on: bool) {
    let p = cx.p;
    let (w, h) = (52.0, 30.0);
    cx.c.rect(x, y, w, h, h / 2.0, if on { p.prim } else { p.card2 });
    let kx = if on { x + w - h + 3.0 } else { x + 3.0 };
    let knob = if on { p.on_ink } else if p.light { Rgba::WHITE } else { p.ink3 };
    cx.c.circle(kx + (h - 6.0) / 2.0, y + h / 2.0, (h - 6.0) / 2.0, knob);
}

/// A row with a switch at its end; a tap anywhere on it flips `on`. Returns whether it flipped.
pub fn toggle_row(cx: &mut Ctx, x: f32, y: f32, w: f32, glyph: Option<(&'static str, Rgba)>, title: &str, sub: Option<&str>, on: &mut bool) -> bool {
    let h = row_h(sub.is_some());
    // The switch is drawn first and the row's words stop short of it.
    switch(cx, x + w - 12.0 - 52.0, y + (h - 30.0) / 2.0, *on);
    let tapped = row(cx, x, y, w - 64.0, glyph, title, sub, None, false) || cx.tap(x + w - 64.0, y, 64.0, h);
    if tapped {
        *on = !*on;
    }
    tapped
}

/// Choices side by side in a frosted track, the chosen one in ink. Returns a newly chosen index.
pub fn segmented(cx: &mut Ctx, x: f32, y: f32, w: f32, labels: &[&str], selected: usize) -> Option<usize> {
    let p = cx.p;
    let h = 44.0;
    cx.c.rect(x, y, w, h, 14.0, p.slab);
    let tw = (w - 8.0) / labels.len().max(1) as f32;
    let mut out = None;
    for (i, label) in labels.iter().enumerate() {
        let sx = x + 4.0 + i as f32 * tw;
        let on = i == selected;
        if on {
            if p.light {
                cx.c.shadow(sx, y + 5.0, tw, h - 8.0, 11.0, 4.0, Rgba::hex(0x000000).a(0.08));
                cx.c.rect(sx, y + 4.0, tw, h - 8.0, 11.0, Rgba::WHITE);
            } else {
                cx.c.rect(sx, y + 4.0, tw, h - 8.0, 11.0, p.card2.mix(Rgba::WHITE, 0.04));
            }
        }
        let t = Text::new(ty::UI, if on { p.ink } else { p.ink2 }).center();
        let t = if on { t.semi() } else { t.medium() };
        cx.c.text_fit(label, sx + tw / 2.0, y + (h - t.lh()) / 2.0, tw - 8.0, t);
        if cx.tap(sx, y, tw, h) && !on {
            out = Some(i);
        }
    }
    out
}

/// Small rounded choices that wrap ("Today", "Yesterday", "Another day"). Returns a tapped index
/// and the height used.
pub fn pills(cx: &mut Ctx, x: f32, y: f32, w: f32, labels: &[&str], selected: Option<usize>) -> (Option<usize>, f32) {
    let p = cx.p;
    let (h, gap) = (36.0, 8.0);
    let (mut px, mut py) = (x, y);
    let mut out = None;
    for (i, label) in labels.iter().enumerate() {
        let on = selected == Some(i);
        let t = Text::new(ty::UI, if on { p.on_ink } else { p.ink }).medium();
        let pw = cx.c.measure(label, t) + 32.0;
        if px + pw > x + w && px > x {
            px = x;
            py += h + gap;
        }
        if on {
            cx.c.rect(px, py, pw, h, h / 2.0, p.prim);
        } else {
            cx.c.outline(px, py, pw, h, h / 2.0, 1.0, if p.light { Rgba::hex(0x000000).a(0.12) } else { Rgba::WHITE.a(0.16) });
        }
        cx.c.text(label, px + 16.0, py + (h - t.lh()) / 2.0, t);
        if cx.tap(px, py, pw, h) {
            out = Some(i);
        }
        px += pw + gap;
    }
    (out, py + h - y)
}

// Text fields

/// A boxed text field with a label over it. `numeric` asks the keyboard for digits. Returns the
/// height used. While focused, `value` follows what the keyboard holds.
#[allow(clippy::too_many_arguments)]
pub fn field(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, label: &str, placeholder: &str, value: &mut String, numeric: bool) -> f32 {
    let p = cx.p;
    let lt = Text::new(ty::CAPTION, p.ink2).medium();
    cx.c.text(label, x + 2.0, y, lt);
    let fy = y + lt.lh() + 4.0;
    let h = 48.0;
    let focused = cx.focus.is(id);
    if focused {
        *value = cx.focus.text.clone();
        cx.focus.at = Some((fy, h));
    }
    cx.c.rect(x, fy, w, h, 14.0, p.slab);
    if focused {
        // A quiet edge while typing: the owner found the brass one odd.
        cx.c.outline(x, fy, w, h, 14.0, 1.5, p.ink3);
    } else {
        let l = p.line_subtle;
        cx.c.outline(x, fy, w, h, 14.0, 1.0, l);
    }
    inline(cx, id, x + 14.0, fy, w - 28.0, h, placeholder, value, numeric, Text::new(ty::BODY, p.ink));
    lt.lh() + 4.0 + h
}

/// A field with no box: just its words (or the placeholder), a caret while focused, and the tap
/// that gives it the keyboard. For rows that carry their own frame, like the entry's note.
#[allow(clippy::too_many_arguments)]
pub fn inline(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, h: f32, placeholder: &str, value: &mut String, numeric: bool, style: Text) -> bool {
    let p = cx.p;
    let focused = cx.focus.is(id);
    if focused {
        *value = cx.focus.text.clone();
        cx.focus.at = Some((y, h));
    }
    let t = if value.is_empty() { style.color(p.ink3) } else { style };
    let words = if value.is_empty() { placeholder } else { value.as_str() };
    // Long text shows its end while typing, as a field scrolls.
    let shown = if focused && cx.c.measure(words, t) > w - 6.0 { tail(cx, words, w - 6.0, t) } else { cx.c.fit(words, w, t) };
    let tw = cx.c.text(&shown, x, y + (h - t.lh()) / 2.0, t);
    if focused {
        let cx_ = x + if value.is_empty() { 0.0 } else { tw + 1.0 };
        cx.c.rect(cx_, y + (h - 22.0) / 2.0, 2.0, 22.0, 1.0, p.signal);
    }
    if cx.tap(x - 14.0, y, w + 28.0, h) && !focused {
        cx.focus.id = Some(id);
        cx.focus.text = value.clone();
        cx.focus.numeric = numeric;
        cx.host.keyboard(true, value, numeric);
        return true;
    }
    false
}

/// The end of `s` that fits `max_w`, led by an ellipsis.
fn tail(cx: &mut Ctx, s: &str, max_w: f32, t: Text) -> String {
    let chars: Vec<char> = s.chars().collect();
    for start in 0..chars.len() {
        let cand: String = std::iter::once('…').chain(chars[start..].iter().copied()).collect();
        if cx.c.measure(&cand, t) <= max_w {
            return cand;
        }
    }
    String::new()
}

/// An amount typed in a field, in minor units: "12.5" is 1250 with two decimals.
pub fn parse_minor(text: &str, fraction_digits: u32) -> Option<i64> {
    let cleaned: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect::<String>().replace(',', ".");
    if cleaned.is_empty() {
        return None;
    }
    relay_money::money::parse_amount(&cleaned, fraction_digits)
}

/// Minor units as a field holds them while editing: "12.50", no symbol, no grouping.
pub fn minor_text(minor: i64, fraction_digits: u32) -> String {
    if fraction_digits == 0 {
        return minor.to_string();
    }
    let scale = relay_money::money::pow10(fraction_digits);
    format!("{}.{:0width$}", minor / scale, (minor % scale).abs(), width = fraction_digits as usize)
}

// Over the page

/// Take this frame's tap away from what is under a sheet. Call before drawing the page.
pub fn hold(cx: &mut Ctx) -> Option<(f32, f32)> {
    cx.ui.tap.take()
}

/// Give the held tap back for the sheet. Call after drawing the page, before the sheet.
pub fn release(cx: &mut Ctx, held: Option<(f32, f32)>) {
    cx.ui.tap = held;
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SheetOut {
    Nothing,
    Pick(usize),
    Close,
}

/// The dark veil behind a sheet or dialog, over the whole screen. True when it was tapped.
fn scrim(cx: &mut Ctx) -> bool {
    let h = cx.c.h;
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, h, 0.0, Rgba::hex(0x000000).a(if cx.p.light { 0.4 } else { 0.7 }));
    cx.ui.tap.is_some_and(|_| true)
}

/// A sheet of choices from the bottom: a title, then one row per item (a title and an optional
/// line), the selected one checked. It scrolls when the list is long. Taps on the veil close it.
pub fn sheet_list(cx: &mut Ctx, title: &str, items: &[(String, Option<String>)], selected: Option<usize>, scroll: &mut Scroll, dt: f32) -> SheetOut {
    let p = cx.p;
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    let any_tap = scrim(cx);
    let screen_h = cx.c.h;
    let bottom_inset = cx.c.inset_bottom;
    let rh = 56.0;
    let list_h = items.iter().map(|(_, s)| if s.is_some() { 64.0 } else { rh }).sum::<f32>();
    let head = 64.0;
    let max_h = screen_h * 0.72;
    let h = (head + list_h + 16.0 + bottom_inset).min(max_h);
    let top = screen_h - h;
    cx.c.rect(0.0, top, WIDTH, h + 40.0, 28.0, p.raised);
    cx.c.rect(WIDTH / 2.0 - 18.0, top + 8.0, 36.0, 4.0, 2.0, p.track.mix(p.ink, 0.2));
    cx.c.text(title, PAD + 4.0, top + 22.0, Text::new(ty::TITLE, p.ink).semi());
    let view = [0.0, top + head, WIDTH, h - head];
    scroll.update(cx.ui, view, dt);
    scroll.max = list_h + 16.0 + bottom_inset - (h - head);
    cx.c.push_clip(0.0, top + head, WIDTH, h - head);
    cx.c.dy = -scroll.off;
    let mut y = top + head;
    let mut out = SheetOut::Nothing;
    for (i, (name, sub)) in items.iter().enumerate() {
        let on = selected == Some(i);
        let rh_i = if sub.is_some() { 64.0 } else { rh };
        let x = PAD - 4.0;
        let w = WIDTH - 2.0 * (PAD - 4.0);
        if cx.pressed(x, y, w, rh_i) {
            cx.c.rect(x, y, w, rh_i, 14.0, p.wash);
        }
        let room = w - 24.0 - if on { 30.0 } else { 0.0 };
        let tt = Text::new(ty::BODY, p.ink);
        let tt = if on { tt.semi() } else { tt.medium() };
        match sub {
            Some(s) => {
                cx.c.text_fit(name, x + 12.0, y + 11.0, room, tt);
                cx.c.text_fit(s, x + 12.0, y + 33.0, room, Text::new(ty::CAPTION, p.ink3));
            }
            None => {
                cx.c.text_fit(name, x + 12.0, y + (rh_i - tt.lh()) / 2.0, room, tt);
            }
        }
        if on {
            cx.c.icon("check", x + w - 22.0, y + rh_i / 2.0, 18.0, p.signal);
        }
        if cx.tap(x, y, w, rh_i) && y + rh_i - scroll.off > top + head {
            out = SheetOut::Pick(i);
        }
        y += rh_i;
    }
    cx.c.dy = 0.0;
    cx.c.pop_clip();
    if out == SheetOut::Nothing && any_tap && cx.ui.tapped(-1000.0, 0.0, WIDTH + 2000.0, top) {
        out = SheetOut::Close;
    }
    cx.c.dy = saved_dy;
    out
}

/// A calendar sheet: one month at a time, the chosen day in ink, arrows for the months around it.
/// `month` is any day of the month shown. Returns the day picked.
pub fn sheet_date(cx: &mut Ctx, title: &str, chosen: Date, month: &mut Date) -> (SheetOut, Option<Date>) {
    use jiff::ToSpan;
    let p = cx.p;
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    let any_tap = scrim(cx);
    let screen_h = cx.c.h;
    let h = 64.0 + 44.0 + 28.0 + 6.0 * 46.0 + 16.0 + cx.c.inset_bottom;
    let top = screen_h - h;
    cx.c.rect(0.0, top, WIDTH, h + 40.0, 28.0, p.raised);
    cx.c.text(title, PAD + 4.0, top + 22.0, Text::new(ty::TITLE, p.ink).semi());
    let first = month.first_of_month();
    let y = top + 64.0;
    cx.c.text(&first.strftime("%B %Y").to_string(), WIDTH / 2.0, y + 10.0, Text::new(ty::BODY, p.ink).semi().center());
    if crate::kit::round_key(cx, PAD, y, "chevron-left") {
        *month = first.checked_sub(1.month()).unwrap_or(first);
    }
    if crate::kit::round_key(cx, WIDTH - PAD - 44.0, y, "chevron-right") {
        *month = first.checked_add(1.month()).unwrap_or(first);
    }
    let gy = y + 52.0;
    let cw = (WIDTH - PAD * 2.0) / 7.0;
    for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
        cx.c.text(d, PAD + cw * i as f32 + cw / 2.0, gy, Text::new(ty::CAPTION, p.ink3).semi().center());
    }
    let lead = first.weekday().to_sunday_zero_offset() as i64;
    let days = first.days_in_month() as i64;
    let today = crate::model::today();
    let mut picked = None;
    for d in 0..days {
        let cell = lead + d;
        let (cxp, cyp) = (PAD + cw * (cell % 7) as f32, gy + 26.0 + 46.0 * (cell / 7) as f32);
        let date = first.checked_add(d.days()).unwrap_or(first);
        let on = date == chosen;
        if on {
            cx.c.circle(cxp + cw / 2.0, cyp + 20.0, 19.0, p.ink);
        } else if date == today {
            let c = p.signal;
            cx.c.outline(cxp + cw / 2.0 - 19.0, cyp + 1.0, 38.0, 38.0, 19.0, 1.5, c);
        }
        let t = Text::new(ty::BODY, if on { p.on_ink } else { p.ink }).center();
        cx.c.text(&(d + 1).to_string(), cxp + cw / 2.0, cyp + 20.0 - t.lh() / 2.0, if on { t.semi() } else { t });
        if cx.tap(cxp, cyp, cw, 40.0) {
            picked = Some(date);
        }
    }
    let mut out = if picked.is_some() { SheetOut::Pick(0) } else { SheetOut::Nothing };
    if out == SheetOut::Nothing && any_tap && cx.ui.tapped(-1000.0, 0.0, WIDTH + 2000.0, top) {
        out = SheetOut::Close;
    }
    cx.c.dy = saved_dy;
    (out, picked)
}

/// A dialog that asks before something that cannot be undone, naming it. Returns Some(true) for
/// the act, Some(false) for Cancel or the veil.
pub fn confirm(cx: &mut Ctx, title: &str, body: &str, act: &str, destructive: bool) -> Option<bool> {
    let p = cx.p;
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    let any_tap = scrim(cx);
    let w = WIDTH - 48.0;
    let bt = Text::new(ty::UI, p.ink2).line(1.4);
    let bh = cx.c.para_h(body, w - 40.0, bt, 6);
    let h = 20.0 + 24.0 + 8.0 + bh + 20.0 + 48.0 + 20.0;
    let (x, y) = (24.0, (cx.c.h - h) / 2.0);
    cx.c.rect(x, y, w, h, 24.0, p.raised);
    cx.c.text(title, x + 20.0, y + 20.0, Text::new(ty::TITLE, p.ink).semi());
    cx.c.para(body, x + 20.0, y + 52.0, w - 40.0, bt, 6);
    let by = y + h - 20.0 - 48.0;
    let bw = (w - 40.0 - 10.0) / 2.0;
    let mut out = None;
    if crate::kit::key(cx, x + 20.0, by, bw, 48.0, "Cancel", None, crate::kit::KeyKind::Plain) {
        out = Some(false);
    }
    // A destructive act is the red key; anything else is the ink one.
    let kx = x + 20.0 + bw + 10.0;
    if destructive {
        let red = p.held;
        cx.c.rect(kx, by, bw, 48.0, KEY_R, red);
        let t = Text::new(ty::BODY, Rgba::WHITE).semi().center();
        cx.c.text(act, kx + bw / 2.0, by + (48.0 - t.lh()) / 2.0, t);
        if cx.tap(kx, by, bw, 48.0) {
            out = Some(true);
        }
    } else if crate::kit::key(cx, kx, by, bw, 48.0, act, None, crate::kit::KeyKind::Primary) {
        out = Some(true);
    }
    if out.is_none() && any_tap && cx.ui.tap.is_some_and(|(tx, ty_)| !(tx >= x && tx < x + w && ty_ >= y && ty_ < y + h)) {
        cx.ui.tap = None;
        out = Some(false);
    }
    cx.c.dy = saved_dy;
    out
}
