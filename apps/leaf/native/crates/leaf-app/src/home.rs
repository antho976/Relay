//! Home (design 42, page 15, "Home, webtoon's layout"): the spotlight, a rounded card of big art
//! with the title centred over it, Continue or Start and the bell (Follow), turning by itself
//! every eighteen seconds and sliding gently (it is not dragged: its dots under it choose one);
//! only covers that fill it sharp take a turn. The spotlight leads with belled series that have
//! new chapters, then what was read most recently, books too ([`logic::spotlight`]).
//!
//! Under it: Top picks for you (three across, each with why in the accent), Continue reading
//! (square covers, "1 new chapter" in the accent) and "Because you read X" rails, each under a
//! banner (a still collage of its covers). Nothing in them moves. Top picks show six of the
//! server's at a time, the next six every four hours ([`logic::rotating`]). The server's finished
//! series to binge is not shown (the owner, 2026-11-23), nor a Because rail of one pick, nor its
//! Collections (the owner never used them, 2026-11-24; the server still makes them). Picks are
//! [`crate::recs`]': a tap opens a pick's page (a preview, nothing added), a long press its sheet.
//! Until the server has made them
//! (its first day) the section says so.

use leaf_proto::{Rec, Recs, Series};

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::CoverOf;
use crate::kit::{self, Ctx};
use crate::logic::{self, Spot};
use crate::net::Job;
use crate::shelf::{self, Book};
use crate::theme::{self as t, COVER_R, PAD, ty};
use crate::ui::HScroll;

/// The spotlight rests this long on each (the owner: slow; it is a picture, not a ticker).
pub const TURN_NS: u64 = 18_000_000_000;
/// A turn slides over this long, easing in and out.
const SLIDE_NS: u64 = 1_200_000_000;
/// At most this many things take turns in it.
const SPOTS: usize = 5;
/// How far down the order the spotlight looks for covers big enough to fill it sharp.
const SPOT_CANDIDATES: usize = 15;
/// The spotlight card's height, and its top under the fixed top.
const CARD_H: f32 = 412.0;
/// The fixed top: "Leaf" and the gear.
const TOP_H: f32 = 56.0;
/// The card's corners.
const CARD_R: f32 = 14.0;
/// A book this far through is finished.
const BOOK_DONE: f32 = 0.995;
/// Top picks shown on Home (the rest are a tap away).
const TOP_PICKS: usize = 6;
/// Home shows the next six top picks after this long (the owner, 2026-11-24: "not every minute").
const PICKS_TURN_MS: i64 = 4 * 3_600_000;
/// The banner over a "Because you read" rail.
const BANNER_H: f32 = 76.0;
/// A "Because you read" rail shows only with at least this many picks.
pub const MIN_BECAUSE: usize = 2;

#[derive(Default)]
pub struct State {
    /// Where the spotlight is: a whole number at rest, between two while it turns. Unbounded;
    /// the thing shown is this modulo how many there are.
    pub pos: f32,
    pub target: f32,
    /// Where the turn going on started from, and when.
    from: f32,
    turn_ns: u64,
    /// When the thing showing came to rest, for the next turn.
    pub since_ns: u64,
    pub rail: HScroll,
    /// "Because you read" rails.
    pub because: Vec<HScroll>,
    /// The shot asks for this one in the spotlight.
    pub shot_spot: Option<usize>,
}

impl State {
    /// Slide to `target` (a whole number) from wherever the spotlight is.
    fn turn_to(&mut self, target: f32, now: u64) {
        self.from = self.pos;
        self.target = target;
        self.turn_ns = now;
        self.since_ns = now;
    }
}

#[derive(Clone)]
enum Item {
    Series(Series),
    Book(Book),
}

impl Item {
    fn cover(&self) -> CoverOf {
        match self {
            Item::Series(s) => CoverOf::Series(s.id),
            Item::Book(b) => CoverOf::Book(b.id),
        }
    }
    fn title(&self) -> &str {
        match self {
            Item::Series(s) => &s.title,
            Item::Book(b) => &b.title,
        }
    }
}

/// A NEW tag: dark words on the accent.
pub fn new_tag(c: &mut crate::draw::Canvas, x: f32, y: f32, words: &str) -> f32 {
    let tt = Text::new(10.5, t::ON_ACCENT).semi();
    let w = c.measure(words, tt) + 10.0;
    c.rect(x, y, w, 18.0, 4.0, t::ACCENT);
    c.text(words, x + 5.0, y + (18.0 - tt.lh()) / 2.0, tt);
    w
}

/// A cover or, until it comes, its quiet place.
#[allow(clippy::too_many_arguments)]
fn cover(cx: &mut Ctx, im: &mut crate::images::Images, it: &Item, x: f32, y: f32, w: f32, h: f32, r: f32, now: u64) {
    match it {
        Item::Book(b) => crate::book::cover(cx.c, im, b, x, y, w, h, now),
        Item::Series(s) => {
            let of = CoverOf::Series(s.id);
            cx.c.rect(x, y, w, h, r, t::S1);
            if im.cover(cx.c, of, now) {
                cx.c.image_cover(&of.key(), x, y, w, h, r, Rgba::WHITE);
            }
        }
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    if app.fresh {
        if !app.library.loading && app.server.is_some() {
            app.ask(Job::Library);
        }
        app.st.home.since_ns = app.now_ns;
        app.want_recs(false);
        app.check_update();
        app.want_lists();
    }
    let now = app.now_ns;
    let now_ms = shelf::now_ms();
    let all_series: Vec<Series> = app.library.data.clone().unwrap_or_default();
    // Dropped series stay out of the spotlight and Continue reading, and so does one added and
    // left alone: nothing read, no bell, nothing downloaded (`logic::in_space`).
    let downloaded = app.saver.series_ids();
    let series: Vec<Series> = all_series.iter().filter(|s| logic::shelf_of(s) != logic::Shelf::Dropped && logic::in_space(s, downloaded.contains(&s.id))).cloned().collect();
    let books: Vec<Book> = app.shelf.books.clone();
    let detail = |id: i64| app.series.get(&id).and_then(|l| l.data.as_ref());
    let new_of = |s: &Series| logic::new_chapters(s, detail(s.id));
    // The spotlight: belled news, then reading; when nothing is read yet, what was added last.
    let mut items: Vec<Item> = Vec::new();
    let mut spots: Vec<Spot> = Vec::new();
    for s in &series {
        spots.push(Spot { rings: s.bell && new_of(s) > 0, updated_at: s.updated_at, read_at: s.last_read_at, done: logic::finished(s, detail(s.id)) });
        items.push(Item::Series(s.clone()));
    }
    for b in &books {
        spots.push(Spot { rings: false, updated_at: 0, read_at: b.opened_ms, done: b.progress >= BOOK_DONE });
        items.push(Item::Book(b.clone()));
    }
    let mut order = logic::spotlight(&spots, SPOT_CANDIDATES);
    // Room left: what was added or updated last takes it, so the spotlight still turns.
    {
        let mut added: Vec<(i64, usize)> = items
            .iter()
            .enumerate()
            .map(|(i, it)| match it {
                Item::Series(s) => (s.added_at.max(s.updated_at), i),
                Item::Book(b) => (b.added_ms, i),
            })
            .collect();
        added.sort_by_key(|a| std::cmp::Reverse(a.0));
        // Finished things last of all, most recently read first.
        let mut done: Vec<usize> = (0..spots.len()).filter(|&i| spots[i].done && spots[i].read_at > 0).collect();
        done.sort_by_key(|&i| std::cmp::Reverse(spots[i].read_at));
        let fill: Vec<usize> = added.into_iter().map(|(_, i)| i).filter(|&i| !spots[i].done).chain(done).collect();
        for i in fill {
            if order.len() >= SPOT_CANDIDATES {
                break;
            }
            if !order.contains(&i) {
                order.push(i);
            }
        }
    }
    // Full-bleed only (the owner's call, 2026-11-22): a cover too small to fill the spotlight sharp
    // is skipped, not shown blurred or as a poster. Covers not decoded yet are asked for (small
    // ones, cheap); until any is known to fit, the order as it stands is shown.
    let now_c = app.now_ns;
    let mut fits: Vec<usize> = Vec::new();
    for &i in &order {
        let of = items[i].cover();
        match app.images.hero_fits(of) {
            Some(true) => fits.push(i),
            Some(false) => {}
            None => {
                app.images.cover(&mut app.canvas, of, now_c);
            }
        }
        if fits.len() >= SPOTS {
            break;
        }
    }
    if !fits.is_empty() {
        order = fits;
    } else {
        order.truncate(SPOTS);
    }
    let hero: Vec<Item> = order.iter().map(|&i| items[i].clone()).collect();
    // What is being read, last read first (a belled series' new chapter as of when it came), then
    // finished things while there are fewer than fifteen ([`logic::continue_order`]).
    let mut reading: Vec<logic::Reading> = Vec::new();
    let mut readable: Vec<Item> = Vec::new();
    for s in &series {
        let d = detail(s.id);
        let ring = s.bell && new_of(s) > 0;
        reading.push(logic::Reading { started: s.last_read_at > 0, done: logic::finished(s, d), read_at: s.last_read_at, news_at: if ring { s.updated_at } else { 0 } });
        readable.push(Item::Series(s.clone()));
    }
    for b in &books {
        reading.push(logic::Reading { started: b.opened_ms > 0, done: b.progress >= BOOK_DONE, read_at: b.opened_ms, news_at: 0 });
        readable.push(Item::Book(b.clone()));
    }
    // (finished, the thing)
    let rail: Vec<(bool, Item)> = logic::continue_order(&reading).into_iter().map(|(i, done)| (done, readable[i].clone())).collect();
    // The chapters of what the spotlight and the rail show, so Continue opens the reader and new
    // chapters are counted: asked once each, from the copy on disk first.
    let want: Vec<i64> = hero.iter().chain(rail.iter().map(|r| &r.1)).filter_map(|it| if let Item::Series(s) = it { Some(s.id) } else { None }).collect();
    for id in want {
        if !app.series.contains_key(&id) {
            app.ask(Job::Series(id));
        }
    }
    let news: Vec<u32> = hero.iter().map(|it| if let Item::Series(s) = it { logic::new_chapters(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())) } else { 0 }).collect();
    let continues: Vec<Option<logic::Continue>> = hero
        .iter()
        .map(|it| if let Item::Series(s) = it { app.series.get(&s.id).and_then(|l| l.data.as_ref()).and_then(|d| crate::places::resume(&app.places, d)) } else { None })
        .collect();
    let rail_new: Vec<u32> = rail.iter().map(|(_, it)| if let Item::Series(s) = it { logic::new_chapters(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())) } else { 0 }).collect();
    let (loading, offline) = (app.library.loading && app.library.data.is_none(), app.library.offline());
    let error = app.library.error.clone().filter(|_| !offline);
    let shot = app.shot;
    // Picks: shown once comics are part of the app (a server, and something in My space or picks).
    let recs: Option<Recs> = app.recs.data.clone();
    let picks_shown = app.server.is_some() && (!all_series.is_empty() || recs.as_ref().is_some_and(|r| !r.top.is_empty()));
    let recs_loading = app.recs.loading && recs.is_none();
    let recs_error = app.recs.error.clone().filter(|_| recs.is_none());
    let covered = app.st.recs.sheet.is_some();
    // Wrapped, in a month's first week (the year's in January's first two): a card, first until
    // it is opened, then at the end.
    let mut wrapped: Vec<(crate::wrapped_stats::Period, Option<crate::wrapped_stats::Stats>, bool)> = Vec::new();
    if app.server.is_some() && !all_series.is_empty() {
        let offered = crate::wrapped_stats::offered(crate::shelf::day_of(crate::shelf::now_ms()));
        if !offered.is_empty() {
            app.want_history();
        }
        for p in offered {
            let s = app.wrapped_summary(&p);
            let seen = app.wrapped_seen.iter().any(|k| *k == p.key());
            wrapped.push((p, s, seen));
        }
    }
    let mut open_wrapped: Option<crate::wrapped_stats::Period> = None;

    let pg = app.page_begin(dt, TOP_H);
    let st = &mut app.st.home;
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    let top = cx.c.inset_top;
    let n = hero.len();
    let mut open_series: Option<i64> = None;
    let mut continue_series: Option<i64> = None;
    let mut bell: Option<(i64, bool)> = None;
    let mut open_book: Option<i64> = None;
    let mut book_notes: Option<i64> = None;
    let mut to_space = false;
    let mut add_books = false;
    let mut to_search = false;
    let mut to_connect = false;
    let mut to_picks: Option<String> = None;
    let mut rec_hit: Option<(crate::recs::Hit, Vec<Rec>, Option<String>)> = None;
    let mut retry_recs = false;
    let mut moving = false;
    let mut wake: Option<u64> = None;
    let (card_x, card_w) = (PAD, WIDTH - 2.0 * PAD);
    let card_y = top + TOP_H + 4.0;
    let card_bottom = card_y + CARD_H;
    let mut y;
    if n > 0 {
        // Turning: by itself, or to a dot tapped; a slide that eases in and out.
        if let Some(i) = st.shot_spot.take() {
            st.pos = i as f32;
            st.turn_to(i as f32, now);
        }
        let resting = (st.pos - st.target).abs() < 0.001;
        if n > 1 && !shot && resting && now.saturating_sub(st.since_ns) >= TURN_NS {
            st.turn_to(st.target.round() + 1.0, now);
        }
        if (st.pos - st.target).abs() >= 0.001 {
            let f = (now.saturating_sub(st.turn_ns) as f32 / SLIDE_NS as f32).clamp(0.0, 1.0);
            let e = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
            st.pos = st.from + (st.target - st.from) * e;
            if f >= 1.0 {
                st.pos = st.target;
                st.since_ns = now;
            } else {
                moving = true;
            }
        } else {
            st.pos = st.target;
        }
        if n == 1 {
            st.pos = 0.0;
            st.target = 0.0;
        }
        let first = st.pos.floor();
        // The pictures stay inside the card; the next slides in from its right edge.
        cx.c.push_clip(card_x, card_y + cx.c.dy, card_w, CARD_H);
        for k in [first, first + 1.0] {
            let off = (k - st.pos) * card_w;
            if off.abs() >= card_w - 0.5 {
                continue;
            }
            let i = (k as i64).rem_euclid(n as i64) as usize;
            let it = &hero[i];
            let of = it.cover();
            let has_hero = match it {
                Item::Book(b) => b.has_cover && app.images.hero(cx.c, of, now),
                Item::Series(_) => app.images.hero(cx.c, of, now),
            };
            let x = card_x + off;
            if has_hero && app.images.hero_poster(of).is_some() {
                // Not one that fills it: its own colours, blurred (shown only until a sharp one is known).
                cx.c.image_cover(&of.hero_key(), x, card_y, card_w, CARD_H, CARD_R, Rgba::WHITE);
            } else if has_hero {
                cx.c.image_cover_at(&of.hero_key(), x, card_y, card_w, CARD_H, CARD_R, Rgba::WHITE, (0.5, 0.3));
            } else if app.images.cover(cx.c, of, now) && cx.c.has_image(&of.banner_key()) {
                cx.c.image_cover(&of.banner_key(), x, card_y, card_w, CARD_H, CARD_R, Rgba::WHITE.a(0.7));
            } else {
                let tone = match it {
                    Item::Book(b) => book_tone(b),
                    Item::Series(_) => t::S1,
                };
                cx.c.rect(x, card_y, card_w, CARD_H, CARD_R, tone);
            }
            // Dark under the words, so they read over any picture.
            cx.c.rect_grad(x, card_y + CARD_H * 0.42, card_w, CARD_H * 0.58, CARD_R, t::BG.a(0.0), t::BG.a(0.9));
        }
        cx.c.pop_clip();
        for k in [first, first + 1.0] {
            let off = (k - st.pos) * card_w;
            if off.abs() >= card_w - 0.5 {
                continue;
            }
            let i = (k as i64).rem_euclid(n as i64) as usize;
            let it = &hero[i];
            let inner = 16.0;
            let x = card_x + inner + off;
            let w = card_w - 2.0 * inner;
            let mid = card_x + card_w / 2.0 + off;
            let keys_y = card_bottom - inner - 44.0;
            let line_t = Text::new(13.0, t::INK.a(0.8)).center();
            let line_y = keys_y - 16.0 - line_t.lh();
            let title_t = Text::new(25.0, t::INK).brand().line(1.12).center();
            let lines = cx.c.wrap(it.title(), w, title_t, 2);
            let title_y = line_y - 6.0 - lines.len() as f32 * title_t.lh();
            let (news_words, line, key, sub, sub_glyph, sub_on): (Option<String>, String, String, &str, &'static str, bool) = match it {
                Item::Series(s) => {
                    let new = news[i];
                    let news_words = (new > 0).then(|| if new == 1 { "1 new chapter".to_string() } else { format!("{new} new chapters") });
                    let mut line: Vec<String> = Vec::new();
                    line.extend(logic::status_word(&s.status));
                    line.push(logic::chapters_words(s.chapters as usize));
                    if s.last_read_at > 0 {
                        line.push(format!("read {}", logic::ago(s.last_read_at, now_ms)));
                    }
                    let key = match &continues[i] {
                        Some(c) => c.label.clone(),
                        None => match logic::at_chapter(s, None) {
                            Some(n) => format!("Continue ch. {}", logic::num(n)),
                            None => "Start reading".into(),
                        },
                    };
                    let (sub, glyph) = if s.bell { ("Following", "bell-on") } else { ("Follow", "bell") };
                    (news_words, line.join(" · "), key, sub, glyph, s.bell)
                }
                Item::Book(b) => {
                    let mut line: Vec<String> = Vec::new();
                    if !b.author.is_empty() {
                        line.push(b.author.clone());
                    }
                    line.push(crate::library::book_line(b));
                    line.dedup();
                    let key = if b.progress > 0.0 { "Continue" } else { "Start reading" };
                    (None, line.join(" · "), key.into(), "Notes", "notes", false)
                }
            };
            if let Some(nw) = &news_words {
                cx.c.text_fit(&nw.to_uppercase(), mid, title_y - 20.0, w, Text::new(11.5, t::ACCENT).semi().center());
            }
            for (j, l) in lines.iter().enumerate() {
                cx.c.text(l, mid, title_y + j as f32 * title_t.lh(), title_t);
            }
            cx.c.text_fit(&line, mid, line_y, w, line_t);
            // The white key, then the bell (or a book's Notes) outlined, side by side.
            let kw = (w - 10.0) / 2.0;
            let kt = Text::new(ty::BODY, t::BG).semi().center();
            let pressed = cx.pressed(x, keys_y, kw, 44.0);
            cx.c.rect(x, keys_y, kw, 44.0, t::KEY_R, if pressed { t::INK.mix(t::BG, 0.15) } else { t::INK });
            cx.c.text_fit(&key, x + kw / 2.0, keys_y + (44.0 - kt.lh()) / 2.0, kw - 16.0, kt);
            let dx = x + kw + 10.0;
            let dt_ = Text::new(ty::BODY, t::INK).medium();
            if cx.pressed(dx, keys_y, kw, 44.0) {
                cx.c.rect(dx, keys_y, kw, 44.0, t::KEY_R, t::WASH);
            }
            cx.c.rect(dx, keys_y, kw, 44.0, t::KEY_R, t::BG.a(0.35));
            cx.c.outline(dx, keys_y, kw, 44.0, t::KEY_R, 1.0, t::INK.a(0.3));
            let sw = cx.c.measure(sub, dt_) + 24.0;
            let sx = dx + (kw - sw) / 2.0;
            cx.c.icon(sub_glyph, sx + 8.0, keys_y + 22.0, 15.0, if sub_on { t::ACCENT } else { t::INK });
            cx.c.text(sub, sx + 24.0, keys_y + (44.0 - dt_.lh()) / 2.0, dt_);
            if off.abs() < 1.0 && !covered {
                let main = cx.tap(x, keys_y, kw, 44.0);
                let second = cx.tap(dx, keys_y, kw, 44.0);
                let picture = !main && !second && cx.tap(card_x, card_y, card_w, keys_y - card_y - 4.0);
                match it {
                    Item::Series(s) if main => continue_series = Some(s.id),
                    Item::Series(s) if second => bell = Some((s.id, !s.bell)),
                    Item::Series(s) if picture => open_series = Some(s.id),
                    Item::Book(b) if main || picture => open_book = Some(b.id),
                    Item::Book(b) if second => book_notes = Some(b.id),
                    _ => {}
                }
            }
        }
        // Which one, of how many: dots under the card.
        y = card_bottom + 14.0;
        if n > 1 {
            let cur = (st.pos.round() as i64).rem_euclid(n as i64) as usize;
            let total = 16.0 + (n - 1) as f32 * 4.0 + (n - 1) as f32 * 6.0;
            let mut dx = WIDTH / 2.0 - total / 2.0;
            let left = dx;
            let mut centres = vec![0.0; n];
            for (i, c) in centres.iter_mut().enumerate() {
                if i == cur {
                    cx.c.rect(dx, y, 16.0, 4.0, 2.0, t::INK);
                    *c = dx + 8.0;
                    dx += 16.0;
                } else {
                    cx.c.circle(dx + 2.0, y + 2.0, 2.0, t::INK.a(0.4));
                    *c = dx + 2.0;
                    dx += 4.0;
                }
                dx += 6.0;
            }
            // A dot is too small to aim at: a tap along them picks the nearest.
            let (sx, sw) = (left - 16.0, dx - left + 32.0);
            if !covered
                && let Some((tx, _)) = cx.ui.tap
                && cx.tap(sx, y - 18.0, sw, 40.0)
                && let Some(j) = (0..n).min_by(|&a, &b| (centres[a] - tx).abs().total_cmp(&(centres[b] - tx).abs()))
                && j != cur
            {
                let to = st.pos.round() + j as f32 - cur as f32;
                st.turn_to(to, now);
                moving = true;
            }
            if !shot && (st.pos - st.target).abs() < 0.001 {
                wake = Some(st.since_ns + TURN_NS);
            }
        }
        y += 4.0 + 28.0;
    } else if loading {
        cx.c.rect(card_x, card_y, card_w, CARD_H, CARD_R, t::S1);
        y = card_bottom + 32.0;
    } else {
        // Honest at zero: nothing to read yet, and where things come from.
        y = top + 140.0;
        let x0 = PAD + 24.0;
        let w = WIDTH - 2.0 * x0;
        let (sw, sh) = (78.0, 112.0);
        for i in 0..3 {
            let sx = WIDTH / 2.0 - sw * 1.5 - 10.0 + i as f32 * (sw + 10.0);
            cx.c.outline(sx, y + if i == 1 { 0.0 } else { 14.0 }, sw, sh, COVER_R, 1.0, t::INK3.a(0.5));
        }
        y += sh + 40.0;
        cx.c.text("Nothing to read yet", WIDTH / 2.0, y, Text::new(ty::HEADING, t::INK).semi().center());
        y += 32.0;
        y += cx.c.para("Add EPUB, PDF or text files from this phone, or comics from your server's sources.", x0, y, w, Text::new(ty::UI, t::INK2).center().line(1.45), 4) + 24.0;
        if kit::primary(&mut cx, x0, y, w, 50.0, "Add books", Some("plus")) {
            add_books = true;
        }
        y += 62.0;
        if app.server.is_some() {
            if kit::plain(&mut cx, x0, y, w, 46.0, "Find comics", Some("search")) {
                to_search = true;
            }
        } else if kit::plain(&mut cx, x0, y, w, 46.0, "Connect your comics server", None) {
            to_connect = true;
        }
        y += 46.0;
        if let Some(e) = &error {
            y += 16.0;
            y += cx.c.para(e, x0, y, w, Text::new(ty::CAPTION, t::INK3).center(), 3);
        }
        y += 32.0;
    }
    let head_t = Text::new(18.0, t::INK).semi();
    // A section's heading, with a chevron when it opens a page of its own. True when tapped.
    let heading = |cx: &mut Ctx, y: f32, words: &str, more: bool| -> (f32, bool) {
        let lines = cx.c.wrap(words, WIDTH - 2.0 * PAD - if more { 28.0 } else { 0.0 }, head_t.line(1.25), 2);
        let lh = head_t.line(1.25).lh();
        for (j, l) in lines.iter().enumerate() {
            cx.c.text(l, PAD, y + j as f32 * lh, head_t.line(1.25));
        }
        let h = lines.len() as f32 * lh;
        if more {
            cx.c.icon("chevron-right", WIDTH - PAD - 8.0, y + lh / 2.0, 18.0, t::INK2);
        }
        let tapped = more && cx.tap(0.0, y - 10.0, WIDTH, h + 20.0);
        (h, tapped)
    };
    for (p, s, _) in wrapped.iter().filter(|w| !w.2) {
        let (h, tap) = wrapped_card(&mut cx, &mut app.images, p, s.as_ref(), y, now, covered);
        if tap {
            open_wrapped = Some(*p);
        }
        y += h + 32.0;
    }
    // Top picks for you.
    if picks_shown {
        let top_recs: Vec<Rec> = recs.as_ref().map(|r| logic::rotating(&r.top, TOP_PICKS, crate::shelf::now_ms(), PICKS_TURN_MS)).unwrap_or_default();
        let more = recs.as_ref().is_some_and(|r| r.top.len() > TOP_PICKS);
        let (h, tapped) = heading(&mut cx, y, "Top picks for you", more);
        if tapped {
            to_picks = Some("top".into());
        }
        y += h + 14.0;
        if !top_recs.is_empty() {
            let (h, hit) = crate::recs::grid(&mut cx, &mut app.images, &top_recs, y, now, covered, true);
            if let Some(hit) = hit {
                rec_hit = Some((hit, top_recs.clone(), None));
            }
            y += h + 32.0;
        } else if recs_loading {
            let cw = (WIDTH - 2.0 * PAD - 20.0) / 3.0;
            for i in 0..3 {
                cx.c.rect(PAD + i as f32 * (cw + 10.0), y, cw, (cw * 1.3).round(), COVER_R, t::S1);
            }
            y += (cw * 1.3).round() + 43.0 + 32.0;
        } else {
            let made = recs.as_ref().is_some_and(|r| r.made_at > 0);
            let words = match (&recs_error, made) {
                (Some(e), _) => format!("Picks didn't load. {e}"),
                (None, false) => "Picks are being made from what you read; back in a few hours.".to_string(),
                (None, true) => "No picks for now. They come from series you've read a few chapters of, and only ones your sources carry.".to_string(),
            };
            let pt = Text::new(ty::UI, t::INK2).line(1.45);
            y += cx.c.para(&words, PAD, y, WIDTH - 2.0 * PAD, pt, 4);
            if recs_error.is_some() {
                y += 12.0;
                let rw = cx.c.measure("Retry", Text::new(ty::BODY, t::INK).semi()) + 40.0;
                if kit::plain(&mut cx, PAD, y, rw, 40.0, "Retry", None) {
                    retry_recs = true;
                }
                y += 40.0;
            }
            y += 32.0;
        }
    }
    if !rail.is_empty() {
        let (h, tapped) = heading(&mut cx, y, "Continue reading", true);
        if tapped {
            to_space = true;
        }
        y += h + 14.0;
        let side = 128.0;
        let title_t = Text::new(13.5, t::INK).semi();
        let line_t = Text::new(12.5, t::INK3);
        let cell_h = side + 8.0 + title_t.lh() + 3.0 + line_t.lh();
        let area = [0.0, y + cx.c.dy, WIDTH, cell_h];
        cx.ui.claim_h(area);
        moving |= st.rail.update(cx.ui, area, dt);
        let mut x = PAD - st.rail.off;
        for (j, (done, it)) in rail.iter().enumerate() {
            if x + side > -10.0 && x < WIDTH + 10.0 {
                cover(&mut cx, &mut app.images, it, x, y, side, side, COVER_R, now);
                if cx.pressed(x, y, side, cell_h) {
                    cx.c.rect(x, y, side, side, COVER_R, t::WASH);
                }
                let (f, words, new) = match it {
                    // Finished: "Read", dimmed, and no bar.
                    _ if *done => (0.0, "Read".to_string(), false),
                    Item::Series(s) => {
                        let new = rail_new[j];
                        let d = app.series.get(&s.id).and_then(|l| l.data.as_ref());
                        let words = if new > 0 {
                            if new == 1 { "1 new chapter".to_string() } else { format!("{new} new chapters") }
                        } else {
                            let at = logic::at_chapter(s, d).map_or("Ch. 1".into(), |n| format!("Ch. {}", logic::num(n)));
                            format!("{at} · {} to go", logic::to_go(s, d))
                        };
                        (logic::read_fraction(s), words, new > 0)
                    }
                    Item::Book(b) => (b.progress, crate::library::book_line(b), false),
                };
                if new {
                    new_tag(cx.c, x + 6.0, y + 6.0, "NEW CHAPTER");
                }
                // How far: the accent along the cover's foot.
                if f > 0.0 {
                    cx.c.rect(x + 1.0, y + side - 4.0, side - 2.0, 3.0, 1.5, Rgba::hex(0x000000).a(0.55));
                    cx.c.rect(x + 1.0, y + side - 4.0, ((side - 2.0) * f).max(3.0), 3.0, 1.5, t::ACCENT);
                }
                cx.c.text_fit(it.title(), x, y + side + 8.0, side, title_t);
                let words_t = if new {
                    line_t.color(t::ACCENT)
                } else if *done {
                    line_t.color(t::INK3.a(0.6))
                } else {
                    line_t
                };
                cx.c.text_fit(&words, x, y + side + 8.0 + title_t.lh() + 3.0, side, words_t);
                if !covered && cx.tap(x, y, side, cell_h) {
                    match it {
                        Item::Series(s) => open_series = Some(s.id),
                        Item::Book(b) => open_book = Some(b.id),
                    }
                }
            }
            x += side + 10.0;
        }
        st.rail.max = x + st.rail.off - 10.0 + PAD - WIDTH;
        y += cell_h + 32.0;
    }
    // Your lists put on Home, each a rail of its own (the owner, 2026-11-26).
    let mut list_act: Option<crate::lists::RailAct> = None;
    let pinned: Vec<leaf_proto::List> = app.lists.data.iter().flatten().filter(|l| l.home).cloned().collect();
    for l in &pinned {
        let on: Vec<(Series, String)> = crate::lists::series_of(l, &all_series)
            .into_iter()
            .map(|s| (s.clone(), crate::lists::where_words(s, app.series.get(&s.id).and_then(|d| d.data.as_ref()))))
            .collect();
        let hs = app.st.lists.rails.entry(l.id).or_default();
        let (h, m) = crate::lists::rail(&mut cx, &mut app.images, hs, l, &on, y, dt, now, covered, &mut list_act);
        moving |= m;
        y += h + 32.0;
    }
    if picks_shown && let Some(r) = &recs {
        // Because you read X: a rail each.
        // One pick is not a rail (the owner, 2026-11-23): fewer than two, and it is left out.
        let because: Vec<&leaf_proto::Because> = r.because.iter().filter(|b| b.recs.len() >= MIN_BECAUSE).collect();
        st.because.resize(because.len(), HScroll::default());
        for (k, b) in because.iter().enumerate() {
            // Its header: a banner, a still collage of the rail's covers, with the words over it.
            let (bx, bw, bh) = (PAD, WIDTH - 2.0 * PAD, BANNER_H);
            crate::recs::collage(cx.c, &mut app.images, &b.recs, bx, y, bw, bh, 12.0, 5, crate::recs::tone(&b.title).mix(t::BG, 0.4), 0.42, now);
            crate::recs::scrim(cx.c, bx, y, bw, bh, 12.0);
            let more = b.recs.len() > 3;
            let small = Text::new(ty::SMALL, Rgba::WHITE.a(0.78)).semi();
            let big = Text::new(17.0, Rgba::WHITE).semi();
            let tw = bw - 28.0 - if more { 24.0 } else { 0.0 };
            let ty0 = y + bh - 14.0 - big.lh();
            cx.c.text(if b.finished { "BECAUSE YOU FINISHED" } else { "BECAUSE YOU READ" }, bx + 14.0, ty0 - small.lh() - 2.0, small);
            cx.c.text_fit(&b.title, bx + 14.0, ty0, tw, big);
            if more {
                cx.c.icon("chevron-right", bx + bw - 18.0, ty0 + big.lh() / 2.0, 18.0, Rgba::WHITE);
                if cx.pressed(bx, y, bw, bh) {
                    cx.c.rect(bx, y, bw, bh, 12.0, t::WASH);
                }
                if !covered && cx.tap(bx, y, bw, bh) {
                    to_picks = Some(format!("because:{}", b.series));
                }
            }
            y += bh + 14.0;
            let (h, hit, m) = crate::recs::rail(&mut cx, &mut app.images, &mut st.because[k], &b.recs, y, dt, now, covered);
            moving |= m;
            if let Some(hit) = hit {
                rec_hit = Some((hit, b.recs.clone(), None));
            }
            y += h + 32.0;
        }
    }
    for (p, s, _) in wrapped.iter().filter(|w| w.2) {
        let (h, tap) = wrapped_card(&mut cx, &mut app.images, p, s.as_ref(), y, now, covered);
        if tap {
            open_wrapped = Some(*p);
        }
        y += h + 32.0;
    }
    if n > 0
        && let Some(e) = &error
    {
        y += cx.c.para(e, PAD, y, WIDTH - 2.0 * PAD, Text::new(ty::CAPTION, t::DANGER), 3) + 8.0;
    }
    let bottom = y + kit::TABS_H;
    // The fixed top: the name and the gear, on the ground.
    let mut settings = false;
    let mut retry = false;
    {
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + TOP_H, 0.0, t::BG);
        kit::fixed_top(&mut cx, TOP_H);
        let tt = Text::new(22.0, t::INK).brand();
        let tw = cx.c.text("Leaf", PAD + 4.0, top + 6.0 + (44.0 - tt.lh()) / 2.0, tt);
        if offline {
            let words = "Offline";
            let ot = Text::new(ty::CAPTION, t::INK).medium();
            let ow = cx.c.measure(words, ot) + 18.0;
            let (ox, oy) = (PAD + 4.0 + tw + 10.0, top + 6.0 + 11.0);
            cx.c.rect(ox, oy, ow, 22.0, 11.0, t::S3.a(0.85));
            cx.c.text(words, ox + 9.0, oy + (22.0 - ot.lh()) / 2.0, ot);
            if cx.ui.tapped(ox - 6.0, oy - 10.0, ow + 12.0, 42.0) {
                retry = true;
            }
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 6.0, "gear") {
            settings = true;
        }
    }
    let tab = kit::tabs(&mut cx, 0);
    let mut sheet_act = None;
    if covered {
        kit::release(&mut cx, held);
        if let Some(s) = &app.st.recs.sheet {
            sheet_act = crate::recs::sheet(&mut cx, &mut app.images, s, now);
        }
    }
    app.page_end(&pg, bottom);
    if let Some(at) = wake {
        app.wake_at(at);
    }
    if retry {
        app.images.retry_failed();
        app.ask(Job::Library);
    }
    if retry_recs {
        app.want_recs(true);
    }
    if add_books {
        app.host.pick_books();
    }
    if let Some(a) = sheet_act {
        crate::recs::act(app, a);
    } else if covered {
        // The sheet holds every tap.
    } else if let Some(a) = list_act {
        crate::lists::rail_act(app, a);
    } else if let Some((h, list, tag)) = rec_hit {
        crate::recs::on_hit(app, h, &list, tag.as_deref());
    } else if let Some(p) = open_wrapped {
        crate::wrapped::open(app, p);
    } else if let Some(of) = to_picks {
        app.push(Route::Picks(of));
    } else if let Some(id) = continue_series {
        app.continue_series(id);
    } else if let Some((id, on)) = bell {
        app.set_bell(id, on);
    } else if let Some(id) = open_series {
        app.push(Route::Series(id));
    } else if let Some(id) = open_book {
        app.open_book(id);
    } else if let Some(id) = book_notes {
        app.push(Route::BookNotes(id));
    } else if settings || to_connect {
        app.push(Route::Connect);
    } else if to_search {
        app.go_tab(1);
    } else if to_space {
        app.st.space.tab = crate::space::RECENT;
        app.go_tab(2);
    } else if let Some(i) = tab {
        app.go_tab(i);
    }
    pg.moving || moving || (app.library.loading || app.recs.loading) && !crate::wake::has()
}

/// The deep tone a drawn book cover has, for a spotlight with no picture.
fn book_tone(b: &Book) -> Rgba {
    const TONES: [u32; 6] = [0x24405E, 0x5C2E2E, 0x24493F, 0x463A63, 0x5E4724, 0x343E4C];
    let hash = b.title.bytes().fold(7u32, |h, x| h.wrapping_mul(31).wrapping_add(x as u32));
    Rgba::hex(TONES[(hash % TONES.len() as u32) as usize])
}

/// Home's Wrapped card: the month's (or year's) name, a line of its numbers, Open, and a fan of its
/// top series' covers. Returns its height and whether it was tapped.
fn wrapped_card(cx: &mut Ctx, im: &mut crate::images::Images, p: &crate::wrapped_stats::Period, s: Option<&crate::wrapped_stats::Stats>, y: f32, now: u64, covered: bool) -> (f32, bool) {
    let (x, w, h) = (PAD, WIDTH - 2.0 * PAD, 196.0);
    cx.c.rect_grad(x, y, w, h, 18.0, Rgba::hex(0x0F3B27), Rgba::hex(0x1F2A5C));
    if let Some(s) = s {
        for (k, (id, ..)) in s.top.iter().take(4).enumerate() {
            let of = CoverOf::Series(*id);
            let (cx_, cy) = (x + w - 150.0 + k as f32 * 28.0, y + 28.0 + (k % 2) as f32 * 30.0);
            cx.c.rect(cx_, cy, 64.0, 92.0, 8.0, Rgba::WHITE.a(0.08));
            if im.cover(cx.c, of, now) {
                cx.c.image_cover(&of.key(), cx_, cy, 64.0, 92.0, 8.0, Rgba::WHITE);
            }
        }
    }
    cx.c.text("WRAPPED", x + 18.0, y + 22.0, Text::new(11.0, t::ACCENT).semi());
    let tw = 170.0;
    let title = format!("Your {} in Leaf", p.name());
    let tt = Text::new(22.0, Rgba::WHITE).semi().line(1.15);
    let mut ty_ = y + 44.0;
    for l in cx.c.wrap(&title, tw, tt, 2) {
        cx.c.text(&l, x + 18.0, ty_, tt);
        ty_ += tt.lh();
    }
    let line = match s {
        Some(s) if s.chapters > 0 => {
            let mut parts = vec![format!("{} chapters", crate::wrapped::group(s.chapters))];
            if s.streak.0 > 1 {
                parts.push(format!("a {}-day streak", s.streak.0));
            }
            if !s.new_series.is_empty() {
                parts.push(format!("{} new series", s.new_series.len()));
            }
            let last = parts.pop().unwrap_or_default();
            if parts.is_empty() { format!("{last}.") } else { format!("{} and {last}.", parts.join(", ")) }
        }
        Some(_) => "A quiet one. Open it anyway.".to_string(),
        None => "Counting your chapters".to_string(),
    };
    cx.c.para(&line, x + 18.0, ty_ + 6.0, tw, Text::new(13.0, Rgba::WHITE.a(0.72)).line(1.4), 3);
    let kt = Text::new(14.0, Rgba::hex(0x0D0D0E)).semi();
    let kw = cx.c.measure("Open", kt) + 32.0;
    cx.c.rect(x + 18.0, y + h - 52.0, kw, 36.0, 18.0, Rgba::WHITE);
    cx.c.text("Open", x + 18.0 + 16.0, y + h - 52.0 + (36.0 - kt.lh()) / 2.0, kt);
    let tapped = !covered && cx.tap(x, y, w, h);
    (h, tapped)
}
