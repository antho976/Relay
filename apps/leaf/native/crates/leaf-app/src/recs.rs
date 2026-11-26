//! Recommendations (design 42, pages 15 and 17): AniList's reader recommendations for what you
//! read, made once a day on the server (`/v1/recs`) and carried by at least one of your sources.
//! Home shows Top picks, "Because you read X", Collections and one finished series to binge;
//! a series page shows "Readers also like" (`/v1/series/{id}/also`).
//!
//! A pick is not in your space. A tap opens its page as a preview (`/v1/preview`, nothing added:
//! [`App::open_preview`]), with Follow and Download as any series page; reading a chapter,
//! following or downloading is what adds it (the owner, 2026-11-23). A long press opens its sheet
//! (artboard wtH3): why it is there, Follow (added with its bell on), Read chapter 1 (added, then
//! opened), Not interested (hidden for good) and Fewer like a tag. One already yours opens its
//! page. A list of picks (a collection, all Top picks, the ones because you read one) is its own
//! page, [`draw_picks`].

use leaf_proto::Rec;

use crate::app::{App, Route, Then};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::{CoverOf, Images};
use crate::kit::{self, Ctx};
use crate::logic;
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};
use crate::ui::HScroll;

#[derive(Default)]
pub struct State {
    /// A pick's sheet is open.
    pub sheet: Option<Sheet>,
}

pub struct Sheet {
    pub rec: Rec,
    /// "Fewer like" this tag (a collection's own, else the pick's first genre).
    pub tag: Option<String>,
}

/// What a tap on a pick asked for.
pub enum Hit {
    /// A tap: its page (a preview, or the series when it is yours already).
    Open(usize),
    /// A long press: the sheet.
    Hold(usize),
}

/// A pick tapped (its page: the series when it is yours, else a preview) or held (its sheet).
pub fn hit(app: &mut App, rec: &Rec, tag: Option<&str>, held: bool) {
    match rec.in_space {
        _ if held => {
            let tag = tag.map(str::to_string).or_else(|| rec.genres.first().cloned());
            app.st.recs.sheet = Some(Sheet { rec: rec.clone(), tag });
        }
        Some(id) => app.push(Route::Series(id)),
        None => app.open_pick(rec),
    }
}

/// A pick's cover, or its quiet place until it comes.
#[allow(clippy::too_many_arguments)]
pub fn cover(c: &mut crate::draw::Canvas, im: &mut Images, r: &Rec, x: f32, y: f32, w: f32, h: f32, radius: f32, now: u64) {
    let of = CoverOf::Rec(r.anilist_id);
    c.rect(x, y, w, h, radius, t::S1);
    if im.cover(c, of, now) {
        c.image_cover(&of.key(), x, y, w, h, radius, Rgba::WHITE);
    }
}

/// A static collage of picks' covers filling a rounded box (Steam's collection tiles): up to
/// `most` covers side by side, each a slice cut from its middle, on the tone `base` while they
/// come, dimmed to `light` (1 as they are) so words over it read. Nothing moves.
#[allow(clippy::too_many_arguments)]
pub fn collage(c: &mut crate::draw::Canvas, im: &mut Images, recs: &[Rec], x: f32, y: f32, w: f32, h: f32, radius: f32, most: usize, base: Rgba, light: f32, now: u64) {
    c.rect(x, y, w, h, radius, base);
    let n = recs.len().min(most);
    if n == 0 {
        return;
    }
    let cw = w / n as f32;
    for (i, r) in recs.iter().take(n).enumerate() {
        let of = CoverOf::Rec(r.anilist_id);
        if !im.cover(c, of, now) {
            continue;
        }
        // Each slice runs under the next by a corner's width, so the seams are straight and only
        // the box's own corners are round.
        let sx = x + i as f32 * cw;
        let sw = if i + 1 < n { cw + radius } else { x + w - sx };
        let l = (light.clamp(0.0, 1.0) * 255.0) as u32;
        c.image_cover(&of.key(), sx, y, sw, h, radius, Rgba::hex((l << 16) | (l << 8) | l));
    }
}

/// A dark veil up from the bottom of a picture, for words over it.
pub fn scrim(c: &mut crate::draw::Canvas, x: f32, y: f32, w: f32, h: f32, radius: f32) {
    c.rect(x, y, w, h, radius, Rgba::hex(0x000000).a(0.12));
    c.rect_grad(x, y + h * 0.3, w, h * 0.7, radius, Rgba::hex(0x000000).a(0.0), Rgba::hex(0x000000).a(0.85));
}

/// "Yours": a check on a pick already in your space.
fn yours(c: &mut crate::draw::Canvas, x: f32, y: f32) {
    let tt = Text::new(10.5, t::INK).semi();
    let w = c.measure("Yours", tt) + 26.0;
    c.rect(x, y, w, 20.0, 5.0, Rgba::hex(0x000000).a(0.72));
    c.icon("check", x + 10.0, y + 10.0, 11.0, t::ACCENT);
    c.text("Yours", x + 18.0, y + (20.0 - tt.lh()) / 2.0, tt);
}

/// A long press resting on this content rectangle: true once (the touch is taken).
fn held_on(cx: &mut Ctx, now: u64, x: f32, y: f32, w: f32, h: f32) -> bool {
    let dy = cx.c.dy;
    let on = cx.ui.resting(now).is_some_and(|((px, py), ns)| ns >= crate::series::LONG_PRESS_NS && px >= x && px < x + w && py >= y + dy && py < y + h + dy);
    if on {
        cx.ui.swallow();
    }
    on
}

/// The grid's cells: three across.
pub const COLS: usize = 3;
const GAP: f32 = 10.0;

/// The height of a grid of `n` picks.
pub fn grid_h(n: usize) -> f32 {
    let (_, _, cell) = cell_size();
    let rows = n.div_ceil(COLS);
    rows as f32 * cell + rows.saturating_sub(1) as f32 * 16.0
}

/// (cover width, cover height, cell height).
fn cell_size() -> (f32, f32, f32) {
    let cw = (WIDTH - 2.0 * PAD - GAP * (COLS - 1) as f32) / COLS as f32;
    let ch = (cw * 1.3).round();
    (cw, ch, ch + 8.0 + 18.0 + 17.0)
}

/// Picks three across from `y`: the cover, the title, and why in the accent (`why`; else its
/// genres and length, for a list where every pick has the same why), "Yours" on one already in
/// your space. `still`: a sheet is over the page (no long press under it).
#[allow(clippy::too_many_arguments)]
pub fn grid(cx: &mut Ctx, im: &mut Images, recs: &[Rec], y: f32, now: u64, still: bool, why: bool) -> (f32, Option<Hit>) {
    let (cw, ch, cell) = cell_size();
    let view_top = -cx.c.dy - 40.0;
    let view_bottom = -cx.c.dy + cx.c.h + 40.0;
    let mut hit = None;
    for (i, r) in recs.iter().enumerate() {
        let (col, row) = (i % COLS, i / COLS);
        let x = PAD + col as f32 * (cw + GAP);
        let cy = y + row as f32 * (cell + 16.0);
        if cy + cell < view_top || cy > view_bottom {
            continue;
        }
        cover(cx.c, im, r, x, cy, cw, ch, COVER_R, now);
        if cx.pressed(x, cy, cw, cell) {
            cx.c.rect(x, cy, cw, ch, COVER_R, t::WASH);
        }
        if r.in_space.is_some() {
            yours(cx.c, x + 6.0, cy + 6.0);
        }
        cx.c.text_fit(&r.title, x, cy + ch + 8.0, cw, Text::new(13.0, t::INK).semi());
        if why {
            cx.c.text_fit(&r.reason, x, cy + ch + 8.0 + 18.0, cw, Text::new(12.0, t::ACCENT));
        } else {
            cx.c.text_fit(&facts(r), x, cy + ch + 8.0 + 18.0, cw, Text::new(12.0, t::INK3));
        }
        if !still && held_on(cx, now, x, cy, cw, cell) {
            hit = Some(Hit::Hold(i));
        } else if cx.tap(x, cy, cw, cell) {
            hit = Some(Hit::Open(i));
        }
    }
    (grid_h(recs.len()), hit)
}

/// The cover of a rail's pick.
pub const RAIL_W: f32 = 116.0;
pub const RAIL_H: f32 = 156.0;

/// A row of picks that scrolls sideways, from `y`: covers with their titles. Returns its height,
/// what was tapped, and whether it is still moving.
#[allow(clippy::too_many_arguments)]
pub fn rail(cx: &mut Ctx, im: &mut Images, hs: &mut HScroll, recs: &[Rec], y: f32, dt: f32, now: u64, still: bool) -> (f32, Option<Hit>, bool) {
    let title_t = Text::new(13.0, t::INK).semi();
    let cell_h = RAIL_H + 8.0 + title_t.lh();
    let area = [0.0, y + cx.c.dy, WIDTH, cell_h];
    cx.ui.claim_h(area);
    let moving = hs.update(cx.ui, area, dt);
    let mut x = PAD - hs.off;
    let mut hit = None;
    for (i, r) in recs.iter().enumerate() {
        if x + RAIL_W > -10.0 && x < WIDTH + 10.0 {
            cover(cx.c, im, r, x, y, RAIL_W, RAIL_H, COVER_R, now);
            if cx.pressed(x, y, RAIL_W, cell_h) {
                cx.c.rect(x, y, RAIL_W, RAIL_H, COVER_R, t::WASH);
            }
            if r.in_space.is_some() {
                yours(cx.c, x + 6.0, y + 6.0);
            }
            cx.c.text_fit(&r.title, x, y + RAIL_H + 8.0, RAIL_W, title_t);
            if !still && held_on(cx, now, x, y, RAIL_W, cell_h) {
                hit = Some(Hit::Hold(i));
            } else if cx.tap(x, y, RAIL_W, cell_h) {
                hit = Some(Hit::Open(i));
            }
        }
        x += RAIL_W + 10.0;
    }
    hs.max = x + hs.off - 10.0 + PAD - WIDTH;
    (cell_h, hit, moving)
}

/// A collection's tile colour: deep tones, one per tag (categories may carry colour).
pub fn tone(tag: &str) -> Rgba {
    const TONES: [u32; 6] = [0x1E6B47, 0x46289A, 0x8A3320, 0x1F4F7E, 0x6E5317, 0x6A2453];
    let hash = tag.bytes().fold(7u32, |h, x| h.wrapping_mul(31).wrapping_add(x as u32));
    Rgba::hex(TONES[(hash % TONES.len() as u32) as usize])
}

/// A pick's line: its first genres and its length ("Action · Murim · 125 chapters").
pub fn facts(r: &Rec) -> String {
    let mut line: Vec<String> = r.genres.iter().take(2).cloned().collect();
    if let Some(n) = r.chapters.filter(|n| *n > 0) {
        line.push(logic::chapters_words(n as usize));
    }
    if r.status == "completed" {
        line.push("finished".into());
    }
    line.join(" · ")
}

/// What a pick's sheet asked for.
pub enum Act {
    /// Added, its bell on.
    Follow,
    Read,
    Hide,
    Fewer(String),
    Open(i64),
    Close,
}

/// The pick's sheet (artboard wtH3), over the page. The tap was held from the page.
pub fn sheet(cx: &mut Ctx, im: &mut Images, s: &Sheet, now: u64) -> Option<Act> {
    let r = &s.rec;
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let why_t = Text::new(ty::UI, t::INK2).line(1.45);
    let why_h = cx.c.para_h(&r.reason, w - 28.0, why_t, 4);
    let mut rows: Vec<(&'static str, String, Act)> = Vec::new();
    match r.in_space {
        Some(id) => rows.push(("chevron-right", "Open in My space".into(), Act::Open(id))),
        None => {
            rows.push(("bell", "Follow".into(), Act::Follow));
            rows.push(("file-text", "Read chapter 1".into(), Act::Read));
        }
    }
    rows.push(("close", "Not interested".into(), Act::Hide));
    if let Some(tag) = &s.tag {
        rows.push(("minus", format!("Fewer like {tag}"), Act::Fewer(tag.clone())));
    }
    let rh = 52.0;
    let head = 26.0 + 108.0 + 16.0;
    let box_h = 14.0 + 16.0 + 6.0 + why_h + 14.0;
    let (top, outside) = kit::sheet(cx, head + box_h + 12.0 + rh * rows.len() as f32 + 8.0);
    if outside {
        return Some(Act::Close);
    }
    // The cover, the title, its facts.
    let (pw, ph) = (78.0, 108.0);
    let py = top + 26.0;
    cover(cx.c, im, r, x, py, pw, ph, 6.0, now);
    let tx = x + pw + 16.0;
    let tw = WIDTH - PAD - tx;
    let tt = Text::new(ty::SECTION, t::INK).semi().line(1.25);
    let lines = cx.c.wrap(&r.title, tw, tt, 2);
    let mut ty_ = py + 4.0;
    for l in &lines {
        cx.c.text(l, tx, ty_, tt);
        ty_ += tt.lh();
    }
    let ft = Text::new(ty::CAPTION, t::INK2);
    let f = facts(r);
    if !f.is_empty() {
        cx.c.text_fit(&f, tx, ty_ + 4.0, tw, ft);
        ty_ += 4.0 + ft.lh();
    }
    let on = match r.hits.len() {
        0 => "On none of your sources now".to_string(),
        1 => "On 1 of your sources".to_string(),
        n => format!("On {n} of your sources"),
    };
    cx.c.text_fit(&on, tx, ty_ + 4.0, tw, ft.color(t::INK3));
    // Why: the one panel, because it is what the sheet is for.
    let by = top + head;
    cx.c.rect(x, by, w, box_h, 12.0, t::S2);
    cx.c.text("WHY THIS PICK", x + 14.0, by + 14.0, Text::new(ty::SMALL, t::INK3).semi());
    cx.c.para(&r.reason, x + 14.0, by + 14.0 + 16.0 + 6.0, w - 28.0, why_t, 4);
    let mut out = None;
    let mut y = by + box_h + 12.0;
    for (glyph, words, act) in rows {
        if cx.pressed(0.0, y, WIDTH, rh) {
            cx.c.rect(8.0, y, WIDTH - 16.0, rh, 10.0, t::WASH);
        }
        cx.c.icon(glyph, x + 9.0, y + rh / 2.0, 18.0, t::INK2);
        let tt = Text::new(ty::BODY, t::INK);
        cx.c.text_fit(&words, x + 36.0, y + (rh - tt.lh()) / 2.0, w - 36.0, tt);
        if cx.tap(0.0, y, WIDTH, rh) {
            out = Some(act);
        }
        y += rh;
    }
    cx.ui.tap = None;
    out
}

/// Do what a pick's sheet asked.
pub fn act(app: &mut App, a: Act) {
    let Some(s) = app.st.recs.sheet.take() else { return };
    let r = s.rec;
    match a {
        Act::Close => {}
        Act::Open(id) => app.push(Route::Series(id)),
        Act::Follow => app.adopt_rec(&r, Then::Follow),
        // Its first chapter, whatever it is numbered.
        Act::Read => app.adopt_rec(&r, Then::Read(f64::NEG_INFINITY, 0)),
        Act::Hide => {
            app.drop_rec(r.anilist_id);
            app.ask(Job::HideRec(r.anilist_id));
            app.toast(&format!("{} won't be suggested again", r.title));
        }
        Act::Fewer(tag) => {
            app.ask(Job::RecTag(tag.clone(), true));
            if let Some(d) = &mut app.recs.data {
                d.collections.retain(|c| c.tag != tag);
            }
        }
    }
}

/// The picks a list page shows, and its title.
pub fn picks_of(app: &App, of: &str) -> (String, Vec<Rec>, Option<String>) {
    let d = app.recs.data.as_ref();
    if let Some(tag) = of.strip_prefix("tag:") {
        let recs = d.and_then(|d| d.collections.iter().find(|c| c.tag == tag)).map(|c| c.recs.clone()).unwrap_or_default();
        return (tag.to_string(), recs, Some(tag.to_string()));
    }
    if let Some(id) = of.strip_prefix("because:").and_then(|v| v.parse::<i64>().ok()) {
        let b = d.and_then(|d| d.because.iter().find(|b| b.series == id));
        let words = |b: &leaf_proto::Because| format!("Because you {} {}", if b.finished { "finished" } else { "read" }, b.title);
        return (b.map_or("Because you read".into(), words), b.map(|b| b.recs.clone()).unwrap_or_default(), None);
    }
    if of == "binge" {
        return ("Finished, to binge".into(), d.and_then(|d| d.binge.clone()).into_iter().collect(), None);
    }
    ("Top picks for you".into(), d.map(|d| d.top.clone()).unwrap_or_default(), None)
}

/// A list of picks: a collection, every Top pick, the ones because you read one.
pub fn draw_picks(app: &mut App, of: &str, dt: f32) -> bool {
    if app.fresh {
        app.want_recs(false);
    }
    let (title, recs, tag) = picks_of(app, of);
    let covered = app.st.recs.sheet.is_some();
    let now = app.now_ns;
    let pg = app.page_begin(dt, 56.0);
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    let mut y = pg.top + 8.0;
    let mut picked: Option<(usize, bool)> = None;
    if recs.is_empty() {
        y += 40.0;
        let (h, _) = kit::notice(&mut cx, PAD + 16.0, y, WIDTH - 2.0 * PAD - 32.0, "Nothing here now", Some("Picks you added or hid leave this list. New ones come with the next day's picks."), None);
        y += h;
    }
    let rh = 112.0;
    let view_top = -cx.c.dy - 120.0;
    let view_bottom = -cx.c.dy + cx.c.h + 120.0;
    for (i, r) in recs.iter().enumerate() {
        if y + rh < view_top || y > view_bottom {
            y += rh;
            continue;
        }
        if cx.pressed(0.0, y, WIDTH, rh) {
            cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
        }
        let (cw, ch) = (72.0, 96.0);
        cover(cx.c, &mut app.images, r, PAD, y + (rh - ch) / 2.0, cw, ch, 6.0, now);
        let tx = PAD + cw + 14.0;
        // Yours already: a check. Else the row opens its page.
        let kw = 24.0;
        if r.in_space.is_some() {
            cx.c.icon("check", WIDTH - PAD - 10.0, y + rh / 2.0, 16.0, t::ACCENT);
        } else {
            cx.c.icon("chevron-right", WIDTH - PAD - 10.0, y + rh / 2.0, 16.0, t::INK3);
        }
        let tw = WIDTH - PAD - kw - 12.0 - tx;
        let tt = Text::new(ty::BODY, t::INK).semi();
        let rt = Text::new(12.5, t::ACCENT);
        let ft = Text::new(12.5, t::INK3);
        let block = tt.lh() + 4.0 + rt.lh() + 4.0 + ft.lh();
        let ty0 = y + (rh - block) / 2.0;
        cx.c.text_fit(&r.title, tx, ty0, tw, tt);
        cx.c.text_fit(&r.reason, tx, ty0 + tt.lh() + 4.0, tw, rt);
        cx.c.text_fit(&facts(r), tx, ty0 + tt.lh() + 4.0 + rt.lh() + 4.0, tw, ft);
        if !covered && held_on(&mut cx, now, 0.0, y, WIDTH, rh) {
            picked = Some((i, true));
        } else if cx.tap(0.0, y, WIDTH, rh) {
            picked = Some((i, false));
        }
        y += rh;
    }
    let bottom = y;
    // The fixed top: Back and the list's name.
    let back = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 56.0, 0.0, t::BG);
        kit::fixed_top(&mut cx, 56.0);
        kit::top_bar(&mut cx, top + 6.0, &title, &[]).0
    };
    let mut sheet_act = None;
    if covered {
        kit::release(&mut cx, held);
        if let Some(s) = &app.st.recs.sheet {
            sheet_act = sheet(&mut cx, &mut app.images, s, now);
        }
    }
    app.page_end(&pg, bottom);
    if let Some(a) = sheet_act {
        act(app, a);
    } else if let Some((i, held)) = picked.filter(|_| !covered) {
        hit(app, &recs[i], tag.as_deref(), held);
    } else if back && !covered {
        app.pop();
    }
    pg.moving
}

/// Act on a [`Hit`] in `recs`.
pub fn on_hit(app: &mut App, h: Hit, recs: &[Rec], tag: Option<&str>) {
    match h {
        Hit::Open(i) => {
            if let Some(r) = recs.get(i) {
                hit(app, r, tag, false);
            }
        }
        Hit::Hold(i) => {
            if let Some(r) = recs.get(i) {
                hit(app, r, tag, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picks_line_says_its_genres_and_length() {
        let r = Rec {
            anilist_id: 1,
            title: "Myst, Might, Mayhem".into(),
            reason: "More like Nano Machine".into(),
            genres: vec!["Action".into(), "Murim".into(), "Drama".into()],
            chapters: Some(125),
            status: "completed".into(),
            hits: vec![],
            cover: "/v1/recs/cover/1".into(),
            in_space: None,
            description: None,
        };
        assert_eq!(facts(&r), "Action · Murim · 125 chapters · finished");
        assert_eq!(grid_h(0), 0.0);
        assert_eq!(grid_h(3), cell_size().2);
        assert!(grid_h(4) > 2.0 * cell_size().2);
        assert_ne!(tone("Murim"), tone("Regression"));
    }
}
