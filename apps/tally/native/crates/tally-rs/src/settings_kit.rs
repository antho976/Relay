//! The setup pages' own pieces, in the premium look (PORTING.md, "The premium look"): groups of
//! setting rows on a card with hairlines between them, rows whose line under the title wraps, a
//! quiet glyph tile, a footer bar that stays put, a small note, a status line, and a bank's tile.

use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, Ctx, KEY_R, PAD, ty};

/// The content's width on a pushed page.
pub const W: f32 = WIDTH - PAD * 2.0;

/// The gap between things on a pushed page.
pub const GAP: f32 = 12.0;

/// The space between one group and the eyebrow of the next.
pub const GROUP_GAP: f32 = 26.0;

/// One setting row: a glyph tile, a title (in its own colour), a line under it that wraps, and at
/// its end a value, a status dot, a switch or a chevron.
#[derive(Clone, Copy, Default)]
pub struct Row<'a> {
    pub glyph: Option<(&'static str, Option<Rgba>)>,
    pub title: &'a str,
    pub title_color: Option<Rgba>,
    pub sub: Option<&'a str>,
    pub sub_color: Option<Rgba>,
    pub value: Option<&'a str>,
    pub value_color: Option<Rgba>,
    pub dot: Option<Rgba>,
    pub toggle: Option<bool>,
    pub chevron: bool,
    /// A brass check at its end: the one chosen.
    pub checked: bool,
}

impl<'a> Row<'a> {
    pub fn new(title: &'a str) -> Row<'a> {
        Row { title, ..Row::default() }
    }
    /// A glyph on the quiet tile.
    pub fn icon(mut self, g: &'static str) -> Self {
        self.glyph = Some((g, None));
        self
    }
    /// A glyph in a colour that means something (a state colour), on its own soft tint.
    pub fn glyph(mut self, g: &'static str, tint: Rgba) -> Self {
        self.glyph = Some((g, Some(tint)));
        self
    }
    pub fn line(mut self, s: &'a str) -> Self {
        self.sub = Some(s);
        self
    }
    pub fn line_color(mut self, c: Rgba) -> Self {
        self.sub_color = Some(c);
        self
    }
    pub fn value(mut self, v: &'a str) -> Self {
        self.value = Some(v);
        self
    }
    pub fn value_color(mut self, c: Rgba) -> Self {
        self.value_color = Some(c);
        self
    }
    pub fn dot(mut self, c: Rgba) -> Self {
        self.dot = Some(c);
        self
    }
    pub fn toggle(mut self, on: bool) -> Self {
        self.toggle = Some(on);
        self
    }
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }
    pub fn opens(mut self) -> Self {
        self.chevron = true;
        self
    }
    pub fn color(mut self, c: Rgba) -> Self {
        self.title_color = Some(c);
        self
    }
}

const TILE: f32 = 36.0;
/// Where a row's words start, past its tile.
const LEAD: f32 = 12.0 + TILE + 14.0;
/// The widest a row's value may be.
const VALUE_W: f32 = 170.0;

/// A glyph on the quiet tile (`tint` none), or on a soft tint of a colour that means something.
pub fn glyph_tile(cx: &mut Ctx, x: f32, y: f32, size: f32, glyph: &'static str, tint: Option<Rgba>) {
    let p = cx.p;
    let (bg, fg) = match tint {
        Some(t) => (crate::theme::soft_of(t), t),
        None => (p.card2, p.ink),
    };
    cx.c.rect(x, y, size, size, (size * 0.31).round(), bg);
    cx.c.icon(glyph, x + size / 2.0, y + size / 2.0, size * 0.53, fg);
}

fn trail_w(cx: &mut Ctx, r: &Row) -> f32 {
    let mut w = 0.0;
    if let Some(v) = r.value {
        w += cx.c.measure(v, Text::new(ty::UI, cx.p.ink2).medium()).min(VALUE_W) + 10.0;
    }
    if r.dot.is_some() {
        w += 8.0 + 10.0;
    }
    if r.toggle.is_some() {
        w += 52.0 + 18.0;
    }
    if r.chevron {
        w += 22.0;
    }
    if r.checked {
        w += 28.0;
    }
    w
}

fn text_room(cx: &mut Ctx, w: f32, r: &Row) -> f32 {
    let lead = if r.glyph.is_some() { LEAD } else { 16.0 };
    w - lead - 12.0 - trail_w(cx, r)
}

fn title_t(cx: &Ctx, r: &Row) -> Text {
    Text::new(16.0, r.title_color.unwrap_or(cx.p.ink)).medium().line(1.3)
}

fn sub_t(cx: &Ctx, r: &Row) -> Text {
    Text::new(13.0, r.sub_color.unwrap_or(cx.p.ink2)).line(1.38)
}

/// The height a row takes at width `w`.
pub fn row_h(cx: &mut Ctx, w: f32, r: &Row) -> f32 {
    let room = text_room(cx, w, r);
    let (tt, st) = (title_t(cx, r), sub_t(cx, r));
    let mut h = cx.c.para_h(r.title, room, tt, 2);
    if let Some(s) = r.sub {
        h += 3.0 + cx.c.para_h(s, room, st, 4);
    }
    (h + 24.0).max(crate::widgets::row_h(r.sub.is_some()))
}

/// Draws a row at (`x`, `y`), `w` wide. Returns (its height, whether it was tapped).
pub fn row(cx: &mut Ctx, x: f32, y: f32, w: f32, r: &Row) -> (f32, bool) {
    let p = cx.p;
    let h = row_h(cx, w, r);
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y + 2.0, w, h - 4.0, 14.0, p.wash);
    }
    let mut tx = x + 16.0;
    if let Some((g, tint)) = r.glyph {
        glyph_tile(cx, x + 12.0, y + (h - TILE) / 2.0, TILE, g, tint);
        tx = x + LEAD;
    }
    let room = text_room(cx, w, r);
    let (tt, st) = (title_t(cx, r), sub_t(cx, r));
    let title_h = cx.c.para_h(r.title, room, tt, 2);
    let sub_h = r.sub.map(|s| 3.0 + cx.c.para_h(s, room, st, 4)).unwrap_or(0.0);
    let ty0 = y + (h - title_h - sub_h) / 2.0;
    cx.c.para(r.title, tx, ty0, room, tt, 2);
    if let Some(s) = r.sub {
        cx.c.para(s, tx, ty0 + title_h + 3.0, room, st, 4);
    }
    let mut right = x + w - 12.0;
    if r.chevron {
        cx.c.icon("chevron-right", right - 7.0, y + h / 2.0, 14.0, p.ink3);
        right -= 22.0;
    }
    if r.checked {
        cx.c.icon("check", right - 9.0, y + h / 2.0, 18.0, p.signal);
        right -= 28.0;
    }
    if let Some(on) = r.toggle {
        crate::widgets::switch(cx, right - 52.0, y + (h - 30.0) / 2.0, on);
        right -= 52.0 + 10.0;
    }
    if let Some(d) = r.dot {
        cx.c.circle(right - 4.0, y + h / 2.0, 4.0, d);
        right -= 8.0 + 10.0;
    }
    if let Some(v) = r.value {
        let vt = Text::new(ty::UI, r.value_color.unwrap_or(p.ink2)).medium().right();
        cx.c.text_fit(v, right, y + (h - vt.lh()) / 2.0, VALUE_W, vt);
    }
    (h, cx.tap(x, y, w, h))
}

/// Small capitals over a group: 12 semibold, `ink3`. Returns the height used, the gap under it
/// included.
pub fn eyebrow(cx: &mut Ctx, y: f32, words: &str) -> f32 {
    let t = Text::new(12.0, cx.p.ink3).semi();
    cx.c.text(&words.to_uppercase(), PAD + 2.0, y, t);
    t.lh() + 10.0
}

/// The height of a card of rows.
pub fn card_h(cx: &mut Ctx, rows: &[Row]) -> f32 {
    let inner = W - 8.0;
    rows.iter().map(|r| row_h(cx, inner, r)).sum::<f32>() + 8.0
}

/// Rows on one card with hairlines between them. Returns (height, the row tapped).
pub fn rows_card(cx: &mut Ctx, y: f32, rows: &[Row]) -> (f32, Option<usize>) {
    let h = card_h(cx, rows);
    kit::card(cx, PAD, y, W, h);
    let inner = W - 8.0;
    let mut ry = y + 4.0;
    let mut tapped = None;
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            divider(cx, ry, r.glyph.is_some());
        }
        let (rh, t) = row(cx, PAD + 4.0, ry, inner, r);
        if t {
            tapped = Some(i);
        }
        ry += rh;
    }
    (h, tapped)
}

/// A hairline across a card at `y`, starting under the words when the rows have tiles.
pub fn divider(cx: &mut Ctx, y: f32, after_tile: bool) {
    let x = PAD + 4.0 + if after_tile { LEAD } else { 16.0 };
    let c = cx.p.line_subtle;
    cx.c.rect(x, y, PAD + W - 16.0 - x, 1.0, 0.0, c);
}

/// A group of rows: an eyebrow over a card. Returns (height used, the row tapped).
pub fn group(cx: &mut Ctx, y: f32, label: Option<&str>, rows: &[Row]) -> (f32, Option<usize>) {
    let mut at = y;
    if let Some(l) = label {
        at += eyebrow(cx, at, l);
    }
    let (h, tapped) = rows_card(cx, at, rows);
    (at + h - y, tapped)
}

/// A wrapped line of words across the page. Returns its height.
pub fn words(cx: &mut Ctx, y: f32, text: &str, style: Text) -> f32 {
    cx.c.para(text, PAD, y, W, style, 12)
}

/// A small note in `ink3` under what it explains, with an optional glyph before it. Returns its
/// height.
pub fn note(cx: &mut Ctx, y: f32, glyph: Option<&'static str>, text: &str) -> f32 {
    let p = cx.p;
    let t = Text::new(13.0, p.ink3).line(1.45);
    let x = PAD + 2.0 + if glyph.is_some() { 22.0 } else { 0.0 };
    let room = PAD + W - 2.0 - x;
    if let Some(g) = glyph {
        cx.c.icon(g, PAD + 2.0 + 7.0, y + t.lh() / 2.0, 14.0, p.ink3);
    }
    cx.c.para(text, x, y, room, t, 8)
}

/// How a status line reads.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    /// Something under way.
    Working,
    Good,
    Problem,
}

/// What the page is doing, what went wrong, or what it did: a card with a state dot (or a
/// turning mark while working) and the words. Returns (height, whether it was tapped).
pub fn status(cx: &mut Ctx, y: f32, tone: Tone, text: &str) -> (f32, bool) {
    let p = cx.p;
    let ink = match tone {
        Tone::Working => p.ink,
        Tone::Good => p.ink,
        Tone::Problem => p.held_text,
    };
    let t = Text::new(ty::UI, ink).line(1.42);
    let room = W - 18.0 - 22.0 - 18.0;
    let h = cx.c.para_h(text, room, t, 6) + 30.0;
    kit::card(cx, PAD, y, W, h);
    let my = y + 15.0 + t.lh() / 2.0;
    match tone {
        Tone::Working => cx.c.icon("sync", PAD + 18.0 + 7.0, my, 15.0, p.signal),
        Tone::Good => cx.c.circle(PAD + 18.0 + 5.0, my, 4.5, p.live),
        Tone::Problem => cx.c.circle(PAD + 18.0 + 5.0, my, 4.5, p.held),
    }
    cx.c.para(text, PAD + 18.0 + 22.0, y + 15.0, room, t, 6);
    (h, cx.tap(PAD, y, W, h))
}

/// A bank's tile: the bank glyph on the quiet tile, as every bank wears it (no logos, no brand
/// colours).
pub fn bank_mark(cx: &mut Ctx, x: f32, y: f32, size: f32, _name: &str) {
    glyph_tile(cx, x, y, size, "bank", None);
}

/// A key across the page, ink or plain, dimmed and untappable when not `enabled`.
#[allow(clippy::too_many_arguments)]
pub fn wide_key(cx: &mut Ctx, x: f32, y: f32, w: f32, words: &str, glyph: Option<&'static str>, primary: bool, enabled: bool) -> bool {
    tall_key(cx, x, y, w, 52.0, words, glyph, primary, enabled)
}

/// [`wide_key`] at any height.
#[allow(clippy::too_many_arguments)]
pub fn tall_key(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str, glyph: Option<&'static str>, primary: bool, enabled: bool) -> bool {
    let kind = if primary { kit::KeyKind::Primary } else { kit::KeyKind::Plain };
    if enabled {
        return kit::key(cx, x, y, w, h, words, glyph, kind);
    }
    // Drawn as it would be, under a veil of the ground, and deaf to taps.
    let held = cx.ui.tap.take();
    kit::key(cx, x, y, w, h, words, glyph, kind);
    cx.ui.tap = held;
    let veil = cx.p.ground[1].a(0.5);
    cx.c.rect(x, y, w, h, if h >= 50.0 { 18.0 } else { KEY_R }, veil);
    false
}

/// A small filled pill ("Sign in"), its right edge at `right`, centred on `mid`. Returns whether
/// it was tapped.
pub fn pill(cx: &mut Ctx, right: f32, mid: f32, words: &str) -> bool {
    let p = cx.p;
    let t = Text::new(13.0, p.on_ink).semi();
    let (w, h) = (cx.c.measure(words, t) + 28.0, 32.0);
    let (x, y) = (right - w, mid - h / 2.0);
    cx.c.rect(x, y, w, h, h / 2.0, p.prim);
    if cx.pressed(x - 6.0, y - 6.0, w + 12.0, h + 12.0) {
        cx.c.rect(x, y, w, h, h / 2.0, p.ink2.a(0.3));
    }
    cx.c.text(words, x + 14.0, y + (h - t.lh()) / 2.0, t);
    cx.tap(x - 6.0, y - 6.0, w + 12.0, h + 12.0)
}

/// The width [`pill`] takes.
pub fn pill_w(cx: &mut Ctx, words: &str) -> f32 {
    cx.c.measure(words, Text::new(13.0, cx.p.on_ink).semi()) + 28.0
}

/// The fixed bar at the bottom of a pushed page that carries its act, `h` tall over the bottom
/// inset. Returns its top in screen units.
pub fn footer_top(cx: &Ctx, h: f32) -> f32 {
    cx.c.h - cx.c.inset_bottom - h - 28.0
}

/// The height a footer of one row of keys takes.
pub const FOOTER_KEYS: f32 = 52.0;

/// Takes this frame's tap if it landed on the footer, so the page under it never gets it.
pub fn footer_hold(cx: &mut Ctx, top: f32) -> Option<(f32, f32)> {
    let hit = cx.ui.tap.is_some_and(|(_, y)| y >= top);
    if hit { cx.ui.tap.take() } else { None }
}

/// Draws the footer's ground from `top` (screen units; the canvas must not be scrolled): the
/// ground colour, nearly opaque, with a hairline over it.
pub fn footer_ground(cx: &mut Ctx, top: f32) {
    let h = cx.c.h - top + 40.0;
    let (console, line) = (cx.p.console, cx.p.line_subtle);
    cx.c.rect(-1000.0, top, WIDTH + 2000.0, h, 0.0, console);
    cx.c.rect(-1000.0, top, WIDTH + 2000.0, 1.0, 0.0, line);
}

/// A field's label over what it labels: an eyebrow. Returns the height used, the gap under it
/// included.
pub fn label(cx: &mut Ctx, y: f32, words: &str) -> f32 {
    eyebrow(cx, y, words)
}

/// A caption under a group: a hint or the outcome of an act. Returns its height.
pub fn caption(cx: &mut Ctx, y: f32, words: &str, color: Rgba) -> f32 {
    cx.c.para(words, PAD + 2.0, y, W - 4.0, Text::new(13.0, color).line(1.45), 8)
}

/// What draws before a pick row's words, given its left edge and its middle.
pub type Lead<'a> = Option<&'a dyn Fn(&mut Ctx, f32, f32)>;

/// A row to choose, inside a card: a title, an optional line under it, an optional leading mark,
/// and a check in brass when chosen. Returns (height, tapped).
#[allow(clippy::too_many_arguments)]
pub fn pick_row(cx: &mut Ctx, x: f32, y: f32, w: f32, title: &str, sub: Option<&str>, lead: Lead, chosen: bool) -> (f32, bool) {
    let p = cx.p;
    let tt = Text::new(16.0, p.ink).medium();
    let st = Text::new(13.0, p.ink2);
    let h = crate::widgets::row_h(sub.is_some());
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y + 2.0, w, h - 4.0, 14.0, p.wash);
    }
    let mut tx = x + 16.0;
    if let Some(f) = lead {
        f(cx, x + 12.0, y + h / 2.0);
        tx = x + LEAD;
    }
    let room = x + w - 16.0 - if chosen { 28.0 } else { 0.0 } - tx;
    let top = y + (h - tt.lh() - sub.map(|_| st.lh() + 3.0).unwrap_or(0.0)) / 2.0;
    cx.c.text_fit(title, tx, top, room, tt);
    if let Some(s) = sub {
        cx.c.text_fit(s, tx, top + tt.lh() + 3.0, room, st);
    }
    if chosen {
        cx.c.icon("check", x + w - 16.0 - 9.0, y + h / 2.0, 18.0, p.signal);
    }
    (h, cx.tap(x, y, w, h))
}

/// The app's icon, `d` across, the face of Tally (Settings' header, About).
pub fn coin(cx: &mut Ctx, x: f32, y: f32, d: f32) {
    crate::kit::app_icon(cx, x, y, d);
}
