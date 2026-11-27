//! The controls every screen shares, drawn immediately: keys, pills, icon keys, text fields with
//! the real keyboard (Tally RS's), a top bar, a notice with Retry, a confirm dialog and a small
//! menu. Each takes a [`Ctx`] and a place, draws, and says whether it was tapped.
//!
//! Positions are content units: `y` is moved by the canvas's `dy` (a scroller's offset), and hit
//! tests are moved the same way.

use crate::draw::{Canvas, Rgba, Text, WIDTH};
use crate::theme::{self as t, ty};
use crate::ui::Ui;

/// What a screen draws with: the canvas, the finger, the focused field and the platform (for the
/// keyboard). Build one with [`crate::cx!`].
pub struct Ctx<'a> {
    pub c: &'a mut Canvas,
    pub ui: &'a mut Ui,
    pub focus: &'a mut Focus,
    pub host: &'a std::sync::Arc<dyn crate::app::Host>,
}

/// A [`Ctx`] over an [`crate::app::App`]'s canvas, finger, focus and host, borrowing only those
/// fields so the screen can still reach the rest of the app.
#[macro_export]
macro_rules! cx {
    ($app:expr) => {{ $crate::kit::Ctx { c: &mut $app.canvas, ui: &mut $app.ui, focus: &mut $app.focus, host: &$app.host } }};
}

impl Ctx<'_> {
    /// Was this content rectangle tapped this frame?
    pub fn tap(&mut self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let dy = self.c.dy;
        self.ui.tapped(x, y + dy, w, h)
    }

    /// [`Ctx::tap`], with where the tap landed (content units).
    pub fn tap_at(&mut self, x: f32, y: f32, w: f32, h: f32) -> Option<(f32, f32)> {
        let at = self.ui.tap;
        let dy = self.c.dy;
        self.tap(x, y, w, h).then(|| at.map(|(px, py)| (px, py - dy))).flatten()
    }

    pub fn pressed(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.ui.pressed(x, y + self.c.dy, w, h)
    }
}

/// The ground, edge to edge.
pub fn ground(c: &mut Canvas) {
    let dy = c.dy;
    c.dy = 0.0;
    c.rect(-1000.0, 0.0, WIDTH + 2000.0, c.h, 0.0, t::BG);
    c.dy = dy;
}

// Layers

/// Draw what follows on layer `l`, covering `r` (screen units): nothing on a lower layer takes a
/// tap there (ui.rs, "Layers").
pub fn above(cx: &mut Ctx, l: u8, r: [f32; 4]) {
    cx.ui.layer = l;
    cx.ui.occlude(r);
}

// Keys

/// The one filled key of a screen: the accent, its words dark on it.
#[allow(clippy::too_many_arguments)]
pub fn primary(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str, glyph: Option<&'static str>) -> bool {
    let pressed = cx.pressed(x, y, w, h);
    cx.c.rect(x, y, w, h, t::KEY_R, if pressed { t::ACCENT.mix(Rgba::hex(0x000000), 0.12) } else { t::ACCENT });
    key_words(cx, x, y, w, h, words, glyph, t::ON_ACCENT);
    cx.tap(x, y, w, h)
}

/// A quiet key: a surface, ink words.
#[allow(clippy::too_many_arguments)]
pub fn plain(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str, glyph: Option<&'static str>) -> bool {
    let pressed = cx.pressed(x, y, w, h);
    cx.c.rect(x, y, w, h, t::KEY_R, if pressed { t::S3 } else { t::S2 });
    key_words(cx, x, y, w, h, words, glyph, t::INK);
    cx.tap(x, y, w, h)
}

#[allow(clippy::too_many_arguments)]
fn key_words(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str, glyph: Option<&'static str>, ink: Rgba) {
    let tt = Text::new(ty::BODY, ink).semi();
    let tw = cx.c.measure(words, tt) + if glyph.is_some() { 26.0 } else { 0.0 };
    let mut tx = x + (w - tw) / 2.0;
    if let Some(g) = glyph {
        cx.c.icon(g, tx + 9.0, y + h / 2.0, 18.0, ink);
        tx += 26.0;
    }
    cx.c.text(words, tx, y + (h - tt.lh()) / 2.0, tt);
}

/// A small outlined pill for a row's action ("Follow"); `on` fills it quietly ("Following").
/// Returns its width and whether it was tapped (the caller may give the whole row the tap).
pub fn pill(cx: &mut Ctx, right: f32, y: f32, words: &str, on: bool) -> (f32, bool) {
    let tt = Text::new(ty::UI, if on { t::INK2 } else { t::INK }).medium();
    let h = 32.0;
    let w = cx.c.measure(words, tt) + 28.0;
    let x = right - w;
    if on {
        cx.c.rect(x, y, w, h, h / 2.0, t::S2);
    } else {
        cx.c.outline(x, y, w, h, h / 2.0, 1.0, t::INK.a(0.28));
    }
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, h / 2.0, t::WASH);
    }
    cx.c.text(words, x + 14.0, y + (h - tt.lh()) / 2.0, tt);
    (w, cx.tap(x - 6.0, y - 6.0, w + 12.0, h + 12.0))
}

/// A glyph in a 44-unit round target.
pub fn icon_key(cx: &mut Ctx, x: f32, y: f32, glyph: &'static str) -> bool {
    icon_key_ink(cx, x, y, glyph, t::INK)
}

pub fn icon_key_ink(cx: &mut Ctx, x: f32, y: f32, glyph: &'static str, ink: Rgba) -> bool {
    let d = 44.0;
    if cx.pressed(x, y, d, d) {
        cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, t::WASH);
    }
    cx.c.icon(glyph, x + d / 2.0, y + d / 2.0, 21.0, ink);
    cx.tap(x, y, d, d)
}

/// A pushed page's top row: Back at the left, a title beside it, and up to two glyph keys at the
/// right. Returns (back, which key) and its height.
pub fn top_bar(cx: &mut Ctx, top: f32, title: &str, keys: &[&'static str]) -> (bool, Option<usize>, f32) {
    let back = icon_key(cx, 6.0, top, "arrow-left");
    let mut hit = None;
    let mut right = WIDTH - 6.0;
    for (i, g) in keys.iter().enumerate().rev() {
        right -= 44.0;
        if icon_key(cx, right, top, g) {
            hit = Some(i);
        }
    }
    let tt = Text::new(ty::SECTION, t::INK).semi();
    cx.c.text_fit(title, 56.0, top + (44.0 - tt.lh()) / 2.0, right - 56.0 - 8.0, tt);
    (back, hit, 44.0)
}

/// A problem in words with a Retry key under it, drawn on the ground (no box). Returns its height
/// and whether Retry was tapped.
pub fn notice(cx: &mut Ctx, x: f32, y: f32, w: f32, title: &str, body: Option<&str>, action: Option<&str>) -> (f32, bool) {
    let mut h = 0.0;
    let tt = Text::new(ty::SECTION, t::INK).semi().center().line(1.35);
    h += cx.c.para(title, x, y, w, tt, 3);
    if let Some(b) = body {
        h += 6.0;
        h += cx.c.para(b, x, y + h, w, Text::new(ty::UI, t::INK2).center().line(1.45), 5);
    }
    let mut tapped = false;
    if let Some(a) = action {
        h += 18.0;
        let tw = cx.c.measure(a, Text::new(ty::BODY, t::INK).semi()) + 40.0;
        tapped = plain(cx, x + (w - tw) / 2.0, y + h, tw, 44.0, a, None);
        h += 44.0;
    }
    (h, tapped)
}

/// A small label on a surface ("MangaDex").
pub fn tag(c: &mut Canvas, x: f32, y: f32, words: &str) -> f32 {
    let tt = Text::new(ty::SMALL, t::INK2).medium();
    let w = c.measure(words, tt) + 12.0;
    c.rect(x, y, w, 20.0, 5.0, t::S2);
    c.text(words, x + 6.0, y + (20.0 - tt.lh()) / 2.0, tt);
    w
}

// Text fields (Tally RS's, with the real keyboard)

/// The keyboard a text field asks for: words, or a web address (no capitals, no suggestions:
/// autocorrect would "fix" a server's name into a word).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kb {
    #[default]
    Text,
    Url,
    /// A secret: no suggestions, shown as dots.
    Secret,
}

/// Which text field has the keyboard, and what it holds while it is being edited.
#[derive(Default, Debug)]
pub struct Focus {
    pub id: Option<&'static str>,
    pub text: String,
    pub kind: Kb,
    /// The focused field's place on its page (content top, height), so the page keeps it in view.
    pub at: Option<(f32, f32)>,
    /// The soft keyboard shows (the app sets it each frame), so a tap on a focused field whose
    /// keyboard went away brings it back.
    pub keyboard_up: bool,
    /// The selection in `text`, in characters (start, end; equal for a caret). The keyboard moves
    /// it (Gboard's spacebar slide, its editing keys) and a tap in the field sets it.
    pub sel: Option<(usize, usize)>,
}

impl Focus {
    pub fn is(&self, id: &str) -> bool {
        self.id == Some(id)
    }

    /// The caret and selection, inside the text (after its end when none is known).
    pub fn selection(&self) -> (usize, usize) {
        let n = self.text.chars().count();
        let (a, b) = self.sel.unwrap_or((n, n));
        (a.min(n), b.min(n))
    }
}

/// A boxed text field with a label over it. Returns the height used. While focused, `value`
/// follows what the keyboard holds.
#[allow(clippy::too_many_arguments)]
pub fn field(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, label: &str, placeholder: &str, value: &mut String, kind: Kb) -> f32 {
    let lt = Text::new(ty::CAPTION, t::INK2).medium();
    cx.c.text(label, x + 2.0, y, lt);
    let fy = y + lt.lh() + 6.0;
    let h = 48.0;
    let focused = cx.focus.is(id);
    if focused {
        *value = cx.focus.text.clone();
        cx.focus.at = Some((fy, h));
    }
    cx.c.rect(x, fy, w, h, t::KEY_R, t::S1);
    cx.c.outline(x, fy, w, h, t::KEY_R, 1.0, if focused { t::INK3 } else { t::LINE });
    inline(cx, id, x + 14.0, fy, w - 28.0, h, placeholder, value, kind, Text::new(ty::BODY, t::INK));
    lt.lh() + 6.0 + h
}

/// A field with no box: just its words (or the placeholder), a caret while focused, and the tap
/// that gives it the keyboard. True when the tap focused it.
#[allow(clippy::too_many_arguments)]
pub fn inline(cx: &mut Ctx, id: &'static str, x: f32, y: f32, w: f32, h: f32, placeholder: &str, value: &mut String, kind: Kb, style: Text) -> bool {
    let focused = cx.focus.is(id);
    if focused {
        *value = cx.focus.text.clone();
        cx.focus.at = Some((y, h));
    }
    let shown_value = if kind == Kb::Secret { "•".repeat(value.chars().count()) } else { value.clone() };
    let tt = if value.is_empty() { style.color(t::INK3) } else { style };
    let words = if value.is_empty() { placeholder.to_string() } else { shown_value };
    // Long text shows its end while typing, as a field scrolls.
    let shown = if focused && cx.c.measure(&words, tt) > w - 6.0 { tail(cx.c, &words, w - 6.0, tt) } else { cx.c.fit(&words, w, tt) };
    let tw = cx.c.text(&shown, x, y + (h - tt.lh()) / 2.0, tt);
    if focused {
        let caret = x + if value.is_empty() { 0.0 } else { tw + 1.0 };
        cx.c.rect(caret, y + (h - 22.0) / 2.0, 2.0, 22.0, 1.0, t::ACCENT);
    }
    if cx.tap(x - 14.0, y, w + 28.0, h) {
        if !focused {
            cx.focus.id = Some(id);
            cx.focus.text = value.clone();
            cx.focus.kind = kind;
            cx.host.keyboard(true, value, kind);
            return true;
        }
        if !cx.focus.keyboard_up {
            cx.host.keyboard(true, value, kind);
            return true;
        }
    }
    false
}

/// The end of `s` that fits `max_w`, led by an ellipsis.
fn tail(c: &mut Canvas, s: &str, max_w: f32, tt: Text) -> String {
    let chars: Vec<char> = s.chars().collect();
    for start in 0..chars.len() {
        let cand: String = std::iter::once('…').chain(chars[start..].iter().copied()).collect();
        if c.measure(&cand, tt) <= max_w {
            return cand;
        }
    }
    String::new()
}

// Over the page

/// Take this frame's tap away from what is under a dialog or menu. Call before drawing the page.
pub fn hold(cx: &mut Ctx) -> Option<(f32, f32)> {
    cx.ui.cover();
    cx.ui.tap.take()
}

/// Give the held tap back for the dialog. Call after drawing the page.
pub fn release(cx: &mut Ctx, held: Option<(f32, f32)>) {
    cx.ui.tap = held;
}

/// A dialog that asks before something that cannot be undone, naming it. Some(true) for the act,
/// Some(false) for Cancel or a tap outside.
pub fn confirm(cx: &mut Ctx, title: &str, body: &str, act: &str) -> Option<bool> {
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    cx.ui.cover();
    let h_screen = cx.c.h;
    above(cx, crate::ui::layer::DIALOG, [-1000.0, 0.0, WIDTH + 2000.0, h_screen]);
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, h_screen, 0.0, t::SCRIM);
    let w = WIDTH - 48.0;
    let bt = Text::new(ty::UI, t::INK2).line(1.45);
    let bh = cx.c.para_h(body, w - 40.0, bt, 6);
    let h = 22.0 + 24.0 + 10.0 + bh + 22.0 + 44.0 + 20.0;
    let (x, y) = (24.0, (h_screen - h) / 2.0);
    cx.c.rect(x, y, w, h, 16.0, t::S1);
    cx.c.text_fit(title, x + 20.0, y + 22.0, w - 40.0, Text::new(ty::SECTION, t::INK).semi());
    cx.c.para(body, x + 20.0, y + 56.0, w - 40.0, bt, 6);
    let by = y + h - 20.0 - 44.0;
    let bw = (w - 40.0 - 10.0) / 2.0;
    let mut out = None;
    if plain(cx, x + 20.0, by, bw, 44.0, "Cancel", None) {
        out = Some(false);
    }
    let kx = x + 20.0 + bw + 10.0;
    let pressed = cx.pressed(kx, by, bw, 44.0);
    cx.c.rect(kx, by, bw, 44.0, t::KEY_R, if pressed { t::DANGER.mix(Rgba::hex(0x000000), 0.15) } else { t::DANGER });
    let tt = Text::new(ty::BODY, Rgba::hex(0x2A0B09)).semi().center();
    cx.c.text(act, kx + bw / 2.0, by + (44.0 - tt.lh()) / 2.0, tt);
    if cx.tap(kx, by, bw, 44.0) {
        out = Some(true);
    }
    if out.is_none() && cx.ui.tap_here().is_some_and(|(tx, ty_)| !(tx >= x && tx < x + w && ty_ >= y && ty_ < y + h)) {
        cx.ui.tap = None;
        out = Some(false);
    }
    cx.c.dy = saved_dy;
    out
}

/// A small menu dropped from a key at the top right: one row per item. Some(i) for a row,
/// Some(usize::MAX) for a tap outside (it closes).
pub fn menu(cx: &mut Ctx, top: f32, items: &[&str]) -> Option<usize> {
    let saved_dy = cx.c.dy;
    cx.c.dy = 0.0;
    cx.ui.cover();
    // A tap anywhere is the menu's: on a row, or outside to close it.
    let screen = cx.c.h;
    above(cx, crate::ui::layer::POPUP, [-1000.0, 0.0, WIDTH + 2000.0, screen]);
    let w = 200.0;
    let rh = 48.0;
    let h = rh * items.len() as f32 + 8.0;
    let x = WIDTH - 12.0 - w;
    cx.c.shadow(x, top, w, h, 12.0, 18.0, Rgba::hex(0x000000).a(0.5));
    cx.c.rect(x, top, w, h, 12.0, t::S2);
    let mut out = None;
    for (i, item) in items.iter().enumerate() {
        let y = top + 4.0 + rh * i as f32;
        if cx.pressed(x, y, w, rh) {
            cx.c.rect(x + 4.0, y, w - 8.0, rh, 8.0, t::WASH);
        }
        let tt = Text::new(ty::BODY, t::INK);
        cx.c.text(item, x + 16.0, y + (rh - tt.lh()) / 2.0, tt);
        if cx.tap(x, y, w, rh) {
            out = Some(i);
        }
    }
    if out.is_none() && cx.ui.tap_here().is_some() {
        cx.ui.tap = None;
        out = Some(usize::MAX);
    }
    cx.c.dy = saved_dy;
    out
}

// Choosing

/// Choices side by side on a quiet track, the one on raised (Spacing: Tight, Normal, Loose).
/// Returns the one tapped.
pub fn segmented(cx: &mut Ctx, x: f32, y: f32, w: f32, items: &[&str], on: usize) -> Option<usize> {
    segmented_on(cx, x, y, w, items, on, t::S1)
}

/// [`segmented`] on a `track` of its own colour: S2 on a sheet, which is S1 itself.
pub fn segmented_on(cx: &mut Ctx, x: f32, y: f32, w: f32, items: &[&str], on: usize, track: Rgba) -> Option<usize> {
    let h = 36.0;
    cx.c.rect(x, y, w, h, 9.0, track);
    let iw = w / items.len().max(1) as f32;
    let mut hit = None;
    for (i, item) in items.iter().enumerate() {
        let ix = x + iw * i as f32;
        if i == on {
            cx.c.rect(ix + 3.0, y + 3.0, iw - 6.0, h - 6.0, 7.0, t::S3);
        } else if cx.pressed(ix, y, iw, h) {
            cx.c.rect(ix + 3.0, y + 3.0, iw - 6.0, h - 6.0, 7.0, t::WASH);
        }
        let tt = Text::new(ty::UI, if i == on { t::INK } else { t::INK2 }).medium().center();
        cx.c.text_fit(item, ix + iw / 2.0, y + (h - tt.lh()) / 2.0, iw - 8.0, tt);
        // 44 units tall to the finger, though 36 to the eye.
        if cx.tap(ix, y - 4.0, iw, h + 8.0) {
            hit = Some(i);
        }
    }
    hit
}

/// A filter chip ("All 47", "Key idea 21"), with a colour dot when it stands for one. Returns its
/// width and whether it was tapped.
pub fn chip(cx: &mut Ctx, x: f32, y: f32, words: &str, on: bool, dot: Option<Rgba>) -> (f32, bool) {
    let h = 32.0;
    let tt = Text::new(ty::UI, if on { t::BG } else { t::INK2 }).medium();
    let lead = if dot.is_some() { 18.0 } else { 0.0 };
    let w = cx.c.measure(words, tt) + 24.0 + lead;
    cx.c.rect(x, y, w, h, 8.0, if on { t::INK } else { t::S2 });
    if !on && cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, 8.0, t::WASH);
    }
    if let Some(d) = dot {
        cx.c.circle(x + 16.0, y + h / 2.0, 4.0, d);
    }
    cx.c.text(words, x + 12.0 + lead, y + (h - tt.lh()) / 2.0, tt);
    (w, cx.tap(x, y - 6.0, w, h + 12.0))
}

/// The tabs along the bottom: Home, Search, My space. Returns the one tapped. Drawn over the
/// page (call last).
pub fn tabs(cx: &mut Ctx, on: usize) -> Option<usize> {
    let saved = cx.c.dy;
    cx.c.dy = 0.0;
    let h = TABS_H + cx.c.inset_bottom;
    let y = cx.c.h - h;
    cx.c.rect(-1000.0, y, WIDTH + 2000.0, h, 0.0, t::S1);
    let items: [(&str, &'static str); 3] = [("Home", "home"), ("Search", "search"), ("My space", "modules")];
    let iw = WIDTH / items.len() as f32;
    let mut hit = None;
    for (i, (words, glyph)) in items.iter().enumerate() {
        let x = iw * i as f32;
        let ink = if i == on { t::INK } else { t::INK3 };
        if i == on {
            cx.c.rect(x + iw / 2.0 - 24.0, y + 8.0, 48.0, 26.0, 13.0, t::ACCENT_DIM);
        }
        cx.c.icon(glyph, x + iw / 2.0, y + 21.0, 17.0, if i == on { t::ACCENT } else { ink });
        cx.c.text(words, x + iw / 2.0, y + 37.0, Text::new(ty::SMALL, ink).medium().center());
        if cx.tap(x, y, iw, TABS_H) {
            hit = Some(i);
        }
    }
    cx.c.dy = saved;
    hit.filter(|i| *i != on)
}

/// The bottom tabs' height, above the gesture bar.
pub const TABS_H: f32 = 60.0;

/// A sheet over the bottom of the page, `h` tall (the keyboard and gesture bar added below it):
/// the veil, the panel and its grip. Returns the panel's top and whether a tap fell outside it.
pub fn sheet(cx: &mut Ctx, h: f32) -> (f32, bool) {
    cx.c.dy = 0.0;
    cx.ui.cover();
    let screen = cx.c.h;
    // The veil covers everything: no tap reaches what is under a sheet.
    above(cx, crate::ui::layer::SHEET, [-1000.0, 0.0, WIDTH + 2000.0, screen]);
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, screen, 0.0, t::SCRIM);
    let full = h + cx.c.inset_bottom;
    let top = (screen - full).max(cx.c.inset_top + 24.0);
    cx.c.rect(0.0, top, WIDTH, screen - top + 40.0, 18.0, t::S1);
    cx.c.rect(WIDTH / 2.0 - 18.0, top + 8.0, 36.0, 4.0, 2.0, t::S3);
    let outside = cx.ui.tap_here().is_some_and(|(_, y)| y < top);
    if outside {
        cx.ui.tap = None;
    }
    (top, outside)
}

/// A short message at the bottom of the screen ("Copied").
pub fn toast(c: &mut Canvas, words: &str, bottom: f32) {
    let saved = c.dy;
    c.dy = 0.0;
    let tt = Text::new(ty::UI, t::INK).medium();
    let w = (c.measure(words, tt) + 32.0).min(WIDTH - 32.0);
    let (x, y) = ((WIDTH - w) / 2.0, bottom - 44.0 - 16.0);
    c.rect(x, y, w, 44.0, 12.0, t::S3);
    c.text_fit(words, x + 16.0, y + (44.0 - tt.lh()) / 2.0, w - 32.0, tt);
    c.dy = saved;
}

/// A row of filter chips that scrolls sideways when they don't fit. Each item is (words, on,
/// colour dot). Returns the one tapped.
pub fn chip_row(cx: &mut Ctx, hs: &mut crate::ui::HScroll, y: f32, items: &[(String, bool, Option<Rgba>)], dt: f32) -> Option<usize> {
    let h = 32.0;
    let screen_y = y + cx.c.dy;
    let area = [0.0, screen_y - 6.0, WIDTH, h + 12.0];
    cx.ui.claim_h(area);
    hs.update(cx.ui, area, dt);
    cx.c.push_clip(0.0, screen_y - 8.0, WIDTH, h + 16.0);
    let mut x = t::PAD - hs.off;
    let mut hit = None;
    for (i, (words, on, dot)) in items.iter().enumerate() {
        let (w, tapped) = chip(cx, x, y, words, *on, *dot);
        if tapped {
            hit = Some(i);
        }
        x += w + 8.0;
    }
    cx.c.pop_clip();
    hs.max = x + hs.off - 8.0 + t::PAD - WIDTH;
    hit
}
