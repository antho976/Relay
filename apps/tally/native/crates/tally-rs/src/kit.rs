//! The premium look's pieces (DESIGN.md), drawn immediately: cards with hairline edges, picture
//! cards, section headers, entry rows, badges, meters, keys and carousels. Each takes a [`Ctx`]
//! and a place, draws, and says whether it was tapped.
//!
//! Positions are content units: `y` is moved by the canvas's `dy` (a scroller's offset), and
//! hit tests are moved the same way.

use crate::draw::{Canvas, Rgba, Text};
use crate::theme::{self, Palette};
use crate::ui::Ui;

/// The type scale, as the Kotlin app's `Tally.type`.
pub mod ty {
    pub const HERO: f32 = 46.0;
    pub const HEADING: f32 = 28.0;
    pub const FIGURE: f32 = 24.0;
    pub const TITLE: f32 = 17.0;
    pub const SECTION: f32 = 16.0;
    pub const BODY: f32 = 15.0;
    pub const UI: f32 = 14.0;
    pub const CAPTION: f32 = 12.5;
}

pub const PAD: f32 = 16.0;
pub const CARD_R: f32 = 24.0;
/// A picture card's corners: a little rounder than a card's.
pub const PIC_R: f32 = 28.0;
pub const KEY_R: f32 = 16.0;
/// The page's width less its margins.
pub const CW: f32 = crate::draw::WIDTH - PAD * 2.0;

/// What a screen draws with: the canvas, the finger, the palette, the focused field and the
/// platform (for the keyboard). Build one with [`crate::cx!`], which borrows only these fields of
/// the app, so the screen can still reach the store and its own state.
pub struct Ctx<'a> {
    pub c: &'a mut Canvas,
    pub ui: &'a mut Ui,
    pub p: Palette,
    pub focus: &'a mut crate::widgets::Focus,
    pub host: &'a std::sync::Arc<dyn crate::app::Host>,
    /// The page was opened from the drawer (Budgets, Settings, ...): its header shows the drawer's
    /// key. The same page opened from another page (Overview's "All") shows Back.
    pub top_level: bool,
    /// Set to bring the drawer out.
    pub open_drawer: &'a mut bool,
}

/// A [`Ctx`] over an [`crate::app::App`]'s canvas, finger, palette, focus and host.
#[macro_export]
macro_rules! cx {
    ($app:expr) => {{
        // `App::drawer_page`, by field so it can sit beside a borrow of another field.
        let top_level = $app.drawer_depth == Some($app.nav.len().saturating_sub(1))
            && $app.nav.last().is_some_and(|(r, _)| r.is_top_level());
        $crate::kit::Ctx {
            c: &mut $app.canvas,
            ui: &mut $app.ui,
            p: $app.palette,
            focus: &mut $app.focus,
            host: &$app.host,
            top_level,
            open_drawer: &mut $app.drawer_open,
        }
    }};
}

impl Ctx<'_> {
    /// Was this content rectangle tapped this frame?
    pub fn tap(&mut self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let dy = self.c.dy;
        self.ui.tapped(x, y + dy, w, h)
    }

    pub fn pressed(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.ui.pressed(x, y + self.c.dy, w, h)
    }

    pub fn t(&self, size: f32) -> Text {
        Text::new(size, self.p.ink)
    }
}

/// The ground: plain black on Dark, plain white on Light.
pub fn ground(c: &mut Canvas, p: &Palette) {
    let dy = c.dy;
    c.dy = 0.0;
    c.rect(-1000.0, 0.0, crate::draw::WIDTH + 2000.0, c.h, 0.0, p.ground[0]);
    c.dy = dy;
}

/// A card: the palette's card colour with a hairline edge.
pub fn card(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32) {
    let (fill, line) = (cx.p.slab, cx.p.line_subtle);
    cx.c.rect(x, y, w, h, CARD_R, fill);
    cx.c.outline(x, y, w, h, CARD_R, 1.0, line);
}

/// A card that can be tapped: washed while pressed. Returns whether it was tapped.
pub fn card_button(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32) -> bool {
    card(cx, x, y, w, h);
    if cx.pressed(x, y, w, h) {
        let wash = cx.p.wash;
        cx.c.rect(x, y, w, h, CARD_R, wash);
    }
    cx.tap(x, y, w, h)
}

/// Older name for [`card`].
pub fn glass(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32) {
    card(cx, x, y, w, h);
}

/// Older name for [`card_button`].
pub fn glass_button(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32) -> bool {
    card_button(cx, x, y, w, h)
}

// Pictures

/// Draw the picture `name` (its Dark version on Dark) cropped into the box, keeping the part at
/// `focus`. Until it has been decoded the box shows the picture's own ground colour. Returns
/// whether the picture drawn is a dark one (light words go on it).
#[allow(clippy::too_many_arguments)]
pub fn picture(cx: &mut Ctx, name: &'static str, x: f32, y: f32, w: f32, h: f32, radius: f32, focus: (f32, f32)) -> bool {
    let (key, dark) = crate::pictures::themed(name, !cx.p.light);
    let ground = crate::pictures::ground(key).unwrap_or(if dark { Rgba::hex(0x121212) } else { Rgba::hex(0xE9DFD0) });
    cx.c.rect(x, y, w, h, radius, ground);
    if crate::pictures::ensure(cx.c, key) {
        cx.c.image_cover_at(key, x, y, w, h, radius, Rgba::WHITE, focus);
    }
    dark
}

/// What a picture card says, bottom up from its foot.
#[derive(Default, Clone, Copy)]
pub struct PicCard<'a> {
    pub eyebrow: Option<&'a str>,
    pub title: &'a str,
    /// 26 for a full-width card, 17 for a small tile; 34 for a figure as the title.
    pub title_size: f32,
    pub line: Option<&'a str>,
    /// A thin progress bar under the line.
    pub meter: Option<f32>,
    /// A pill button at the foot ("Add money").
    pub button: Option<&'a str>,
}

/// A picture card, Wealthsimple's way: the picture fills the card, its object in the upper part,
/// and the words sit on the picture's plain lower part. Returns (card tapped, button tapped).
#[allow(clippy::too_many_arguments)]
pub fn pic_card(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, pic: &'static str, spec: &PicCard) -> (bool, bool) {
    let small = w < 200.0;
    let r = if small { CARD_R } else { PIC_R };
    let dark = picture(cx, pic, x, y, w, h, r, (0.5, 0.12));
    if dark {
        let line = cx.p.line_subtle;
        cx.c.outline(x, y, w, h, r, 1.0, line);
    }
    let (ink, ink2) = if dark { (theme::pic::INK_ON_DARK, theme::pic::INK2_ON_DARK) } else { (theme::pic::INK_ON_LIGHT, theme::pic::INK2_ON_LIGHT) };
    let pad = if small { 16.0 } else { 22.0 };
    let inner = w - pad * 2.0;
    let title_size = if spec.title_size > 0.0 { spec.title_size } else if small { 17.0 } else { 26.0 };
    let tt = Text::new(title_size, ink).semi().line(1.15);
    let lt = Text::new(if small { 13.0 } else { 15.0 }, ink2).line(1.4);
    let et = Text::new(12.0, ink2).semi();
    // Measure from the foot up.
    let mut stack = 0.0;
    let bh = 42.0;
    if spec.button.is_some() {
        stack += bh + 14.0;
    }
    if spec.meter.is_some() {
        stack += 5.0 + if small { 10.0 } else { 14.0 };
    }
    let line_h = spec.line.map(|l| cx.c.para_h(l, inner, lt, 2)).unwrap_or(0.0);
    stack += line_h + if spec.line.is_some() { 6.0 } else { 0.0 };
    let title_lines = cx.c.wrap(spec.title, inner, tt, 2).len().max(1) as f32;
    stack += tt.lh() * title_lines;
    if spec.eyebrow.is_some() {
        stack += et.lh() + 6.0;
    }
    let mut ty = y + h - pad - stack;
    // A soft shade of the picture's own foot colour under the words, so they read even where the
    // object reaches down behind them.
    let (key, _) = crate::pictures::themed(pic, !cx.p.light);
    let foot = crate::pictures::ground(key).unwrap_or(if dark { Rgba::hex(0x121212) } else { Rgba::hex(0xE9DFD0) });
    let shade_top = (ty - 48.0).max(y);
    cx.c.rect_grad(x, shade_top, w, y + h - shade_top, r, foot.a(0.0), foot.a(if dark { 0.92 } else { 0.85 }));
    if let Some(e) = spec.eyebrow {
        cx.c.text(&e.to_uppercase(), x + pad, ty, et);
        ty += et.lh() + 6.0;
    }
    ty += cx.c.para(spec.title, x + pad, ty, inner, tt, 2);
    if let Some(l) = spec.line {
        ty += 6.0;
        ty += cx.c.para(l, x + pad, ty, inner, lt, 2);
    }
    if let Some(m) = spec.meter {
        ty += if small { 10.0 } else { 14.0 };
        let track = if dark { Rgba::WHITE.a(0.16) } else { Rgba::hex(0x141414).a(0.13) };
        cx.c.rect(x + pad, ty, inner, 5.0, 2.5, track);
        let f = m.clamp(0.0, 1.0);
        if f > 0.0 {
            cx.c.rect(x + pad, ty, (inner * f).max(5.0), 5.0, 2.5, ink);
        }
        ty += 5.0;
    }
    let mut button = false;
    if let Some(b) = spec.button {
        ty += 14.0;
        let (fill, fg) = if dark { (Rgba::WHITE, Rgba::hex(0x141414)) } else { (Rgba::hex(0x141414), Rgba::WHITE) };
        let bt = Text::new(15.0, fg).semi();
        let bw = cx.c.measure(b, bt) + 36.0;
        cx.c.rect(x + pad, ty, bw, bh, bh / 2.0, fill);
        cx.c.text(b, x + pad + 18.0, ty + (bh - bt.lh()) / 2.0, bt);
        button = cx.tap(x + pad - 6.0, ty - 6.0, bw + 12.0, bh + 12.0);
    }
    // No pressed wash: a card that lights up as a finger slides over it reads as a mistake.
    let card = cx.tap(x, y, w, h);
    (card, button)
}

/// The scenes older screens ask for, mapped onto the pictures.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Scene {
    Mountain,
    Beach,
    Growth,
    Hills,
    City,
    Road,
    Banks,
}

impl Scene {
    pub const ALL: [Scene; 7] = [Scene::Mountain, Scene::Beach, Scene::Growth, Scene::Hills, Scene::City, Scene::Road, Scene::Banks];

    /// The picture this scene shows.
    pub fn key(self) -> &'static str {
        match self {
            Scene::Mountain => "suitcase",
            Scene::Beach => "umbrella",
            Scene::Growth => "chart",
            Scene::Hills => "wallet",
            Scene::City => "house",
            Scene::Road => "carkey",
            Scene::Banks => "bank",
        }
    }

    /// A scene for a goal, from what its name says: see [`crate::pictures::for_goal`].
    pub fn for_goal(name: &str, invest: bool) -> Scene {
        match crate::pictures::for_goal(name, invest) {
            "suitcase" => Scene::Mountain,
            "umbrella" => Scene::Beach,
            "chart" => Scene::Growth,
            "house" => Scene::City,
            "carkey" => Scene::Road,
            _ => Scene::Hills,
        }
    }
}

/// A card with a picture across its top and room for `body_h` units of words under it, on the
/// card colour. Returns the top of the body.
#[allow(clippy::too_many_arguments)]
pub fn postcard(cx: &mut Ctx, x: f32, y: f32, w: f32, scene: Scene, scene_h: f32, body_h: f32, tag: Option<(&'static str, &str)>) -> f32 {
    let h = scene_h + body_h;
    card(cx, x, y, w, h);
    cx.c.push_clip(x, y + cx.c.dy, w, scene_h);
    let dark = picture(cx, scene.key(), x, y, w, scene_h + CARD_R, CARD_R, (0.5, 0.25));
    cx.c.pop_clip();
    if let Some((glyph, words)) = tag {
        let (bg, fg) = if dark { (Rgba::hex(0x000000).a(0.55), Rgba::WHITE) } else { (Rgba::WHITE.a(0.85), Rgba::hex(0x141414)) };
        let t = Text::new(12.0, fg).semi();
        let tw = cx.c.measure(words, t);
        let (cw, ch) = (tw + 20.0 + 17.0, 24.0);
        cx.c.rect(x + 12.0, y + 12.0, cw, ch, ch / 2.0, bg);
        cx.c.icon(glyph, x + 12.0 + 10.0 + 6.0, y + 12.0 + ch / 2.0, 12.0, fg);
        cx.c.text(words, x + 12.0 + 27.0, y + 12.0 + (ch - t.lh()) / 2.0, t);
    }
    y + scene_h
}

/// A half-circle gauge for the month: a quiet track, the share spent in ink, brass past pace.
pub fn gauge(cx: &mut Ctx, center_x: f32, top: f32, w: f32, spent: f32, pace: Option<f32>) {
    let stroke = 14.0;
    let r = w / 2.0;
    let cy = top + r;
    let track = cx.p.track;
    cx.c.arc(center_x, cy, r, stroke, 0.75, 1.25, track, false);
    let f = spent.clamp(0.0, 1.0);
    if f > 0.0 {
        let col = if pace.is_some_and(|p| f > p) { cx.p.held } else { cx.p.signal };
        cx.c.arc(center_x, cy, r, stroke, 0.75, 0.75 + f * 0.5, col, false);
    }
    if let Some(p) = pace {
        let ang = (0.75 + p.clamp(0.0, 1.0) * 0.5) * std::f32::consts::TAU;
        let (inner, outer) = (r - stroke * 1.4, r + stroke * 0.4);
        let ink = cx.p.ink2;
        let mid = (inner + outer) / 2.0;
        let (mx, my) = (center_x + ang.sin() * mid, cy - ang.cos() * mid);
        tick(cx.c, mx, my, outer - inner, 2.5, ang, ink);
    }
}

/// A thin line `len` long through (`x`, `y`), along the angle `ang` from the top.
fn tick(c: &mut Canvas, x: f32, y: f32, len: f32, width: f32, ang: f32, col: Rgba) {
    let steps = (len / 1.5).ceil() as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32 - 0.5;
        c.circle(x + ang.sin() * len * t, y - ang.cos() * len * t, width / 2.0, col);
    }
}

/// A tile: a card with a tinted icon badge, a caption, a figure and a line under it.
#[allow(clippy::too_many_arguments)]
pub fn tint_tile(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, caption: &str, figure: &str, sub: &str, hue: Rgba, glyph: &'static str) -> bool {
    let tapped = card_button(cx, x, y, w, h);
    let ix = x + PAD;
    cx.c.rect(ix, y + PAD, 36.0, 36.0, 11.0, theme::soft_of(hue));
    cx.c.icon(glyph, ix + 18.0, y + PAD + 18.0, 18.0, hue);
    let mut ty = y + PAD + 36.0 + 10.0;
    let cap = Text::new(ty::UI, cx.p.ink2).medium();
    ty += cx.c.para(caption, ix, ty, w - PAD * 2.0, cap, 2);
    let fig = Text::new(ty::FIGURE, cx.p.ink).medium();
    cx.c.text_fit(figure, ix, ty + 2.0, w - PAD * 2.0, fig);
    ty += fig.lh() + 6.0;
    let s = Text::new(ty::CAPTION, cx.p.ink3);
    cx.c.para(sub, ix, ty, w - PAD * 2.0, s, 2);
    tapped
}

/// The height a tint tile needs for these words at width `w`.
pub fn tint_tile_h(c: &mut Canvas, w: f32, caption: &str, sub: &str) -> f32 {
    let cap = c.para_h(caption, w - PAD * 2.0, Text::new(ty::UI, Rgba::WHITE).medium(), 2);
    let s = c.para_h(sub, w - PAD * 2.0, Text::new(ty::CAPTION, Rgba::WHITE), 2);
    PAD + 36.0 + 10.0 + cap + Text::new(ty::FIGURE, Rgba::WHITE).lh() + 8.0 + s + PAD
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Quiet,
    Good,
    Warn,
    Bad,
}

/// A small label for a state ("1 day ahead of your money"), centred on `cx_mid`.
pub fn state_pill(cx: &mut Ctx, cx_mid: f32, y: f32, words: &str, tone: Tone) -> f32 {
    let fg = match tone {
        Tone::Quiet => cx.p.ink2,
        Tone::Good => cx.p.live_text,
        Tone::Warn => cx.p.waiting_text,
        Tone::Bad => cx.p.held_text,
    };
    let bg = if tone == Tone::Quiet { cx.p.card2 } else { fg.a(if cx.p.light { 0.1 } else { 0.16 }) };
    let t = Text::new(13.0, fg).semi();
    let tw = cx.c.measure(words, t);
    let (w, h) = (tw + 28.0, 28.0);
    cx.c.rect(cx_mid - w / 2.0, y, w, h, h / 2.0, bg);
    cx.c.text(words, cx_mid, y + (h - t.lh()) / 2.0, t.center());
    h
}

/// A meter: a thin track and a fill, with a mark where an even pace would be.
#[allow(clippy::too_many_arguments)]
pub fn meter(cx: &mut Ctx, x: f32, y: f32, w: f32, fraction: f32, pace: Option<f32>, colour: Rgba) {
    let h = 6.0;
    let track = cx.p.track;
    cx.c.rect(x, y, w, h, h / 2.0, track);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        cx.c.rect(x, y, (w * f).max(h), h, h / 2.0, colour);
    }
    if let Some(p) = pace {
        let ink = cx.p.ink2;
        cx.c.rect(x + w * p.clamp(0.0, 1.0) - 1.0, y - 3.0, 2.0, h + 6.0, 1.0, ink);
    }
}

/// A panel: a card with a title, an optional subtitle and an action on the right.
/// Draws the card `h` tall; returns (top of its content, whether the action was tapped).
#[allow(clippy::too_many_arguments)]
pub fn panel(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, title: &str, subtitle: Option<&str>, action: Option<&str>) -> (f32, bool) {
    card(cx, x, y, w, h);
    let t = Text::new(ty::UI, cx.p.ink2).medium();
    cx.c.text(title, x + 20.0, y + 18.0, t);
    let mut tapped = false;
    if let Some(a) = action {
        let at = Text::new(ty::UI, cx.p.ink2).medium().right();
        let aw = cx.c.measure(a, at);
        cx.c.text(a, x + w - 20.0, y + 18.0, at);
        tapped = cx.tap(x + w - 20.0 - aw - 12.0, y, aw + 32.0, 48.0);
    }
    let mut top = y + 18.0 + t.lh();
    if let Some(s) = subtitle {
        let st = Text::new(20.0, cx.p.ink).semi();
        cx.c.text(s, x + 20.0, top + 2.0, st);
        top += st.lh() + 2.0;
    }
    (top + 8.0, tapped)
}

/// Kinds of key: the one filled action, or a plain one.
#[derive(Clone, Copy, PartialEq)]
pub enum KeyKind {
    Primary,
    Plain,
}

/// A key with words (and an optional glyph before them). Returns whether it was tapped.
#[allow(clippy::too_many_arguments)]
pub fn key(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, words: &str, glyph: Option<&'static str>, kind: KeyKind) -> bool {
    let (bg, fg) = match kind {
        KeyKind::Primary => (cx.p.prim, cx.p.on_ink),
        KeyKind::Plain => (cx.p.card2, cx.p.ink),
    };
    let pressed = cx.pressed(x, y, w, h);
    let r = if h >= 50.0 { 18.0 } else { KEY_R };
    cx.c.rect(x, y, w, h, r, bg);
    if pressed {
        cx.c.rect(x, y, w, h, r, if kind == KeyKind::Primary { Rgba::hex(0x808080).a(0.25) } else { cx.p.wash });
    }
    let t = Text::new(if h >= 50.0 { 16.0 } else { ty::BODY }, fg).semi();
    let tw = cx.c.measure(words, t) + if glyph.is_some() { 26.0 } else { 0.0 };
    let mut tx = x + (w - tw) / 2.0;
    if let Some(g) = glyph {
        cx.c.icon(g, tx + 9.0, y + h / 2.0, 18.0, fg);
        tx += 26.0;
    }
    cx.c.text(words, tx, y + (h - t.lh()) / 2.0, t);
    cx.tap(x, y, w, h)
}

/// A 44-unit key with only a glyph: back, menu, search, settings. No fill; a soft wash pressed.
pub fn round_key(cx: &mut Ctx, x: f32, y: f32, glyph: &'static str) -> bool {
    let d = 44.0;
    if cx.pressed(x, y, d, d) {
        let wash = cx.p.wash;
        cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, wash);
    }
    let ink = cx.p.ink;
    cx.c.icon(glyph, x + d / 2.0, y + d / 2.0, 22.0, ink);
    cx.tap(x, y, d, d)
}

/// A category's icon on a rounded square of its colour's tint: entries, budgets, categories.
pub fn badge(cx: &mut Ctx, x: f32, y: f32, d: f32, icon: &str, colour: Rgba) {
    cx.c.rect(x, y, d, d, (d * 0.32).round(), theme::soft_of(colour));
    cx.c.category_icon(icon, x + d / 2.0, y + d / 2.0, d * 0.5, colour);
}

/// A date as a badge: the month's three letters over the day, on a quiet square. A brass one
/// (`hue` the palette's signal) marks the date that is close.
pub fn date_badge(cx: &mut Ctx, x: f32, y: f32, month: &str, day: i8, hue: Rgba) {
    let soon = hue.0 == cx.p.signal.0;
    let bg = if soon { cx.p.signal.a(0.16) } else { cx.p.card2 };
    cx.c.rect(x, y, 44.0, 48.0, 13.0, bg);
    let mt = Text::new(10.0, if soon { cx.p.signal } else { cx.p.ink3 }).semi().center();
    cx.c.text(&month.to_uppercase(), x + 22.0, y + 7.0, mt);
    cx.c.text(&day.to_string(), x + 22.0, y + 19.0, Text::new(18.0, cx.p.ink).semi().center().line(1.2));
}

/// The donut of where the money went: `parts` as (colour, amount), ring segments with hairline gaps.
pub fn donut(cx: &mut Ctx, center_x: f32, center_y: f32, size: f32, thickness: f32, parts: &[(Rgba, i64)]) {
    let total: i64 = parts.iter().map(|(_, v)| *v).sum();
    let track = cx.p.track;
    cx.c.arc(center_x, center_y, size / 2.0, thickness, 0.0, 0.99999, track, true);
    if total <= 0 {
        return;
    }
    let gap = 0.004;
    let mut at = 0.0f32;
    for (colour, v) in parts {
        let share = *v as f32 / total as f32;
        if share <= 0.0 {
            continue;
        }
        let end = at + share;
        let (a, b) = if share > gap * 2.0 { (at + gap / 2.0, end - gap / 2.0) } else { (at, end) };
        cx.c.arc(center_x, center_y, size / 2.0, thickness, a, b, *colour, true);
        at = end;
    }
}

/// The drawer's key (three lines): tapping it brings the drawer out. For pages that draw their own
/// top, such as Search.
pub fn menu_key(cx: &mut Ctx, x: f32, y: f32) {
    if round_key(cx, x, y, "menu") {
        *cx.open_drawer = true;
    }
}

/// The app's icon (icon E, fanned cards), its Dark or Light version, `d` units square.
pub fn app_icon(cx: &mut Ctx, x: f32, y: f32, d: f32) {
    let key = if cx.p.light { "icon" } else { "icon_d" };
    if crate::pictures::ensure(cx.c, key) {
        cx.c.image_cover(key, x, y, d, d, d * 0.238, Rgba::WHITE);
    } else {
        let fill = if cx.p.light { Rgba::hex(0xF6F3EE) } else { Rgba::hex(0x141414) };
        cx.c.rect(x, y, d, d, d * 0.238, fill);
    }
}

// The premium look's own pieces

/// A section's header: its title on the left, an optional action ("See all") on the right.
/// Returns (height used, action tapped).
pub fn section(cx: &mut Ctx, y: f32, title: &str, action: Option<&str>) -> (f32, bool) {
    let t = Text::new(20.0, cx.p.ink).semi();
    cx.c.text(title, PAD, y, t);
    let mut tapped = false;
    if let Some(a) = action {
        let at = Text::new(ty::UI, cx.p.ink2).medium().right();
        let aw = cx.c.measure(a, at);
        cx.c.text(a, crate::draw::WIDTH - PAD, y + (t.lh() - at.lh()) / 2.0, at);
        tapped = cx.tap(crate::draw::WIDTH - PAD - aw - 16.0, y - 8.0, aw + 32.0, t.lh() + 16.0);
    }
    (t.lh(), tapped)
}

/// An amount with its cents set smaller and raised, as a price is: "$1,284" then ".50".
/// Returns its width.
pub fn money(cx: &mut Ctx, s: &str, x: f32, top: f32, size: f32, colour: Rgba, cents: Rgba) -> f32 {
    let main = Text::new(size, colour).medium().line(1.15);
    let (whole, frac) = match s.rfind('.') {
        Some(i) if s.len() - i <= 3 && s[i + 1..].chars().all(|c| c.is_ascii_digit()) => (&s[..i], &s[i..]),
        _ => (s, ""),
    };
    let w1 = cx.c.measure(whole, main);
    cx.c.text(whole, x, top, main);
    if frac.is_empty() {
        return w1;
    }
    let small = Text::new((size * 0.55).round(), cents).medium().line(1.15);
    cx.c.text(frac, x + w1 + 1.0, top + size * 0.08, small);
    w1 + 1.0 + cx.c.measure(frac, small)
}

/// The width [`money`] would take.
pub fn money_w(c: &mut Canvas, s: &str, size: f32) -> f32 {
    let main = Text::new(size, Rgba::WHITE).medium();
    match s.rfind('.') {
        Some(i) if s.len() - i <= 3 => c.measure(&s[..i], main) + 1.0 + c.measure(&s[i..], Text::new((size * 0.55).round(), Rgba::WHITE).medium()),
        _ => c.measure(s, main),
    }
}

/// An entry's row: the category badge, what and where, the amount and when. 64 tall.
pub const ENTRY_H: f32 = 64.0;

#[allow(clippy::too_many_arguments)]
pub fn entry_row(cx: &mut Ctx, x: f32, y: f32, w: f32, icon: &str, colour: Rgba, title: &str, sub: &str, amount: &str, amount_colour: Rgba, when: &str) -> bool {
    if cx.pressed(x - 8.0, y, w + 16.0, ENTRY_H) {
        let wash = cx.p.wash;
        cx.c.rect(x - 8.0, y + 2.0, w + 16.0, ENTRY_H - 4.0, 14.0, wash);
    }
    badge(cx, x, y + 10.0, 44.0, icon, colour);
    let at = Text::new(16.0, amount_colour).medium().right();
    let ww = Text::new(13.0, cx.p.ink3).right();
    let aw = cx.c.measure(amount, at).max(cx.c.measure(when, ww));
    let tx = x + 58.0;
    let room = (w - 58.0 - aw - 12.0).max(40.0);
    cx.c.text_fit(title, tx, y + 12.0, room, Text::new(16.0, cx.p.ink).medium());
    cx.c.text_fit(sub, tx, y + 35.0, room, Text::new(13.0, cx.p.ink2));
    cx.c.text(amount, x + w, y + 12.0, at);
    cx.c.text(when, x + w, y + 35.0, ww);
    cx.tap(x - 8.0, y, w + 16.0, ENTRY_H)
}

/// A row of cards that scrolls sideways (goal cards, chips). Call before drawing the cards: it
/// moves `hs` with a sideways drag that began in the row and clips to it; draw each card at
/// `x0 + i * (item_w + gap)`, then call [`carousel_end`]. Returns `(x0, moving)`.
#[allow(clippy::too_many_arguments)]
pub fn carousel(cx: &mut Ctx, hs: &mut crate::ui::HScroll, y: f32, h: f32, content_w: f32, dt: f32) -> (f32, bool) {
    let screen_y = y + cx.c.dy;
    hs.max = content_w - CW;
    cx.ui.claim_h([0.0, screen_y, crate::draw::WIDTH, h]);
    let moving = hs.update(cx.ui, [0.0, screen_y, crate::draw::WIDTH, h], dt);
    cx.c.push_clip(0.0, screen_y, crate::draw::WIDTH, h);
    (PAD - hs.off, moving)
}

pub fn carousel_end(cx: &mut Ctx) {
    cx.c.pop_clip();
}
