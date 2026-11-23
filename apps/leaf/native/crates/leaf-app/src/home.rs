//! Home (design 42, page 12, artboard A): a full-bleed spotlight over a cover that turns by
//! itself every eighteen seconds, sliding gently (it is not dragged: its dots choose one), then
//! Continue reading, a rail of covers with how far each is read, then Recently updated. A cover
//! too small to fill the spotlight sharp is a poster over a blurred copy of itself. The spotlight leads with belled series that have new
//! chapters, then what was read most recently, books too ([`logic::spotlight`]).

use leaf_proto::Series;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::{CoverOf, MAX_UPSCALE};
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
/// The spotlight's height under the status bar.
const HERO: f32 = 436.0;

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
    }
    let now = app.now_ns;
    let now_ms = shelf::now_ms();
    let series: Vec<Series> = app.library.data.clone().unwrap_or_default();
    let books: Vec<Book> = app.shelf.books.clone();
    let detail = |id: i64| app.series.get(&id).and_then(|l| l.data.as_ref());
    let new_of = |s: &Series| logic::new_chapters(s, detail(s.id));
    // The spotlight: belled news, then reading; when nothing is read yet, what was added last.
    let mut items: Vec<Item> = Vec::new();
    let mut spots: Vec<Spot> = Vec::new();
    for s in &series {
        spots.push(Spot { rings: s.bell && new_of(s) > 0, updated_at: s.updated_at, read_at: s.last_read_at });
        items.push(Item::Series(s.clone()));
    }
    for b in &books {
        spots.push(Spot { rings: false, updated_at: 0, read_at: b.opened_ms });
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
        for (_, i) in added {
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
    // What is being read: series part way, books part way, last read first.
    let mut rail: Vec<(i64, Item)> = series.iter().filter(|s| s.last_read_at > 0 && s.unread > 0).map(|s| (s.last_read_at, Item::Series(s.clone()))).collect();
    rail.extend(books.iter().filter(|b| b.opened_ms > 0 && b.progress < 0.995).map(|b| (b.opened_ms, Item::Book(b.clone()))));
    rail.sort_by_key(|a| std::cmp::Reverse(a.0));
    let mut updated: Vec<&Series> = series.iter().filter(|s| s.updated_at > 0).collect();
    updated.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
    updated.truncate(8);
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
        .map(|it| if let Item::Series(s) = it { app.series.get(&s.id).and_then(|l| l.data.as_ref()).and_then(|d| logic::continue_point(&d.chapters)) } else { None })
        .collect();
    let rail_new: Vec<u32> = rail.iter().map(|(_, it)| if let Item::Series(s) = it { logic::new_chapters(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())) } else { 0 }).collect();
    let updated_new: Vec<u32> = updated.iter().map(|s| logic::new_chapters(s, app.series.get(&s.id).and_then(|l| l.data.as_ref()))).collect();
    let (loading, offline) = (app.library.loading && app.library.data.is_none(), app.library.offline());
    let error = app.library.error.clone().filter(|_| !offline);
    let shot = app.shot;

    let pg = app.page_begin(dt, 0.0);
    let st = &mut app.st.home;
    let mut cx = cx!(app);
    let top = cx.c.inset_top;
    let n = hero.len();
    let mut open_series: Option<i64> = None;
    let mut continue_series: Option<i64> = None;
    let mut open_book: Option<i64> = None;
    let mut book_notes: Option<i64> = None;
    let mut to_space = false;
    let mut add_books = false;
    let mut to_search = false;
    let mut to_connect = false;
    let mut moving = false;
    let mut wake: Option<u64> = None;
    let hero_h = top + HERO;
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
        // The pictures stop at the spotlight's edge: a backdrop drawn larger must not show as a
        // band under the fade.
        cx.c.push_clip(0.0, cx.c.dy, WIDTH, hero_h);
        for k in [first, first + 1.0] {
            let off = (k - st.pos) * WIDTH;
            if off.abs() >= WIDTH - 0.5 {
                continue;
            }
            let i = (k as i64).rem_euclid(n as i64) as usize;
            let it = &hero[i];
            let of = it.cover();
            let has_hero = match it {
                Item::Book(b) => b.has_cover && app.images.hero(cx.c, of, now),
                Item::Series(_) => app.images.hero(cx.c, of, now),
            };
            if has_hero && app.images.hero_poster(of).is_some() {
                cx.c.image_cover(&of.hero_key(), off - 16.0, -16.0, WIDTH + 32.0, hero_h + 16.0, 0.0, Rgba::WHITE);
            } else if has_hero {
                cx.c.image_cover(&of.hero_key(), off, 0.0, WIDTH, hero_h, 0.0, Rgba::WHITE);
            } else if app.images.cover(cx.c, of, now) && cx.c.has_image(&of.banner_key()) {
                cx.c.image_cover(&of.banner_key(), off - 20.0, -20.0, WIDTH + 40.0, hero_h + 40.0, 0.0, Rgba::WHITE.a(0.7));
            } else {
                let tone = match it {
                    Item::Book(b) => book_tone(b),
                    Item::Series(_) => t::S1,
                };
                cx.c.rect(off, 0.0, WIDTH, hero_h, 0.0, tone);
            }
        }
        cx.c.pop_clip();
        // Dark at the top for the status bar and the name, and a long fade into the ground.
        cx.c.rect_grad(-1000.0, 0.0, WIDTH + 2000.0, top + 90.0, 0.0, t::BG.a(0.55), t::BG.a(0.0));
        cx.c.rect_grad(-1000.0, hero_h * 0.3, WIDTH + 2000.0, hero_h * 0.7 + 1.0, 0.0, t::BG.a(0.0), t::BG);
        for k in [first, first + 1.0] {
            let off = (k - st.pos) * WIDTH;
            if off.abs() >= WIDTH - 0.5 {
                continue;
            }
            let i = (k as i64).rem_euclid(n as i64) as usize;
            let it = &hero[i];
            let x = PAD + 4.0 + off;
            let w = WIDTH - 2.0 * (PAD + 4.0);
            let keys_y = hero_h - 70.0;
            let line_t = Text::new(13.0, t::INK.a(0.78));
            let line_y = keys_y - 18.0 - line_t.lh();
            let title_t = Text::new(30.0, t::INK).brand().line(1.12);
            let lines = cx.c.wrap(it.title(), w, title_t, 2);
            let title_y = line_y - 8.0 - lines.len() as f32 * title_t.lh();
            // A cover too small to fill the spotlight: itself, sharp, at the upper right, never
            // drawn past [`MAX_UPSCALE`] its own pixels, above the words.
            let of = it.cover();
            if let Some((pw, ph)) = app.images.hero_poster(of)
                && cx.c.has_image(&of.hero_key())
                && cx.c.has_image(&of.poster_key())
            {
                let (pw, ph) = (pw as f32, ph as f32);
                let top_y = top + 64.0;
                let room_h = (title_y - 22.0 - 18.0 - top_y).max(60.0);
                let pw_u = (room_h * pw / ph).min(pw * MAX_UPSCALE / cx.c.u).min(WIDTH * 0.5);
                let ph_u = pw_u * ph / pw;
                let px = WIDTH - PAD - 4.0 - pw_u + off;
                let py = top_y + (room_h - ph_u) * 0.35;
                cx.c.shadow(px, py + 8.0, pw_u, ph_u, COVER_R, 26.0, Rgba::hex(0x000000).a(0.6));
                cx.c.image_cover(&of.poster_key(), px, py, pw_u, ph_u, COVER_R, Rgba::WHITE);
                cx.c.outline(px, py, pw_u, ph_u, COVER_R, 1.0, t::INK.a(0.1));
            }
            let (label, label_ink, line, key, sub): (String, Rgba, String, String, &str) = match it {
                Item::Series(s) => {
                    let new = news[i];
                    let label = if new > 0 {
                        (if new == 1 { "1 new chapter".to_string() } else { format!("{new} new chapters") }, t::ACCENT)
                    } else if s.last_read_at > 0 {
                        (format!("Last read {}", logic::ago(s.last_read_at, now_ms)), t::INK)
                    } else {
                        (format!("Added {}", logic::ago(s.added_at, now_ms)), t::INK)
                    };
                    let mut line: Vec<String> = Vec::new();
                    line.extend(logic::status_word(&s.status));
                    line.push(logic::chapters_words(s.chapters as usize));
                    let key = match &continues[i] {
                        Some(c) => c.label.clone(),
                        None => match logic::at_chapter(s, None) {
                            Some(n) => format!("Continue ch. {}", logic::num(n)),
                            None => "Start reading".into(),
                        },
                    };
                    (label.0, label.1, line.join(" · "), key, "Details")
                }
                Item::Book(b) => {
                    let label = if b.opened_ms > 0 { format!("Last read {}", logic::ago(b.opened_ms, now_ms)) } else { format!("Added {}", logic::ago(b.added_ms, now_ms)) };
                    let mut line: Vec<String> = Vec::new();
                    if !b.author.is_empty() {
                        line.push(b.author.clone());
                    }
                    line.push(crate::library::book_line(b));
                    line.dedup();
                    let key = if b.progress > 0.0 { "Continue" } else { "Start reading" };
                    (label, t::INK, line.join(" · "), key.into(), "Notes")
                }
            };
            cx.c.text_fit(&label.to_uppercase(), x, title_y - 22.0, w, Text::new(11.5, label_ink).semi());
            for (j, l) in lines.iter().enumerate() {
                cx.c.text(l, x, title_y + j as f32 * title_t.lh(), title_t);
            }
            cx.c.text_fit(&line, x, line_y, w, line_t);
            // The white key, then Details outlined.
            let kt = Text::new(ty::BODY, t::BG).semi();
            let kw = cx.c.measure(&key, kt) + 26.0 + 36.0;
            let pressed = cx.pressed(x, keys_y, kw, 44.0);
            cx.c.rect(x, keys_y, kw, 44.0, t::KEY_R, if pressed { t::INK.mix(t::BG, 0.15) } else { t::INK });
            cx.c.icon("play", x + 18.0 + 8.0, keys_y + 22.0, 16.0, t::BG);
            cx.c.text(&key, x + 18.0 + 24.0, keys_y + (44.0 - kt.lh()) / 2.0, kt);
            let dt_ = Text::new(ty::BODY, t::INK).medium();
            let dx = x + kw + 10.0;
            let dw = cx.c.measure(sub, dt_) + 36.0;
            if cx.pressed(dx, keys_y, dw, 44.0) {
                cx.c.rect(dx, keys_y, dw, 44.0, t::KEY_R, t::WASH);
            }
            cx.c.outline(dx, keys_y, dw, 44.0, t::KEY_R, 1.0, t::INK.a(0.3));
            cx.c.text(sub, dx + 18.0, keys_y + (44.0 - dt_.lh()) / 2.0, dt_);
            if off.abs() < 1.0 {
                let main = cx.tap(x, keys_y, kw, 44.0);
                let details = cx.tap(dx, keys_y, dw, 44.0);
                let picture = !main && !details && cx.tap(0.0, top + 56.0, WIDTH, keys_y - top - 60.0);
                match it {
                    Item::Series(s) if main => continue_series = Some(s.id),
                    Item::Series(s) if details || picture => open_series = Some(s.id),
                    Item::Book(b) if main || picture => open_book = Some(b.id),
                    Item::Book(b) if details => book_notes = Some(b.id),
                    _ => {}
                }
            }
        }
        // Which one, of how many.
        if n > 1 {
            let cur = (st.pos.round() as i64).rem_euclid(n as i64) as usize;
            let mut dx = WIDTH - PAD - 4.0;
            let dy_ = hero_h - 70.0 + 20.0;
            let right = dx;
            let mut centres = vec![0.0; n];
            for i in (0..n).rev() {
                if i == cur {
                    dx -= 16.0;
                    cx.c.rect(dx, dy_, 16.0, 4.0, 2.0, t::INK);
                    centres[i] = dx + 8.0;
                } else {
                    dx -= 4.0;
                    cx.c.circle(dx + 2.0, dy_ + 2.0, 2.0, t::INK.a(0.4));
                    centres[i] = dx + 2.0;
                }
                dx -= 6.0;
            }
            // A dot is too small to aim at: a tap along them picks the nearest.
            let (sx, sw) = (dx - 10.0, right - dx + 20.0);
            if let Some((tx, _)) = cx.ui.tap
                && cx.tap(sx, dy_ - 20.0, sw, 44.0)
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
        y = hero_h + 24.0;
    } else if loading {
        cx.c.rect(0.0, 0.0, WIDTH, hero_h, 0.0, t::S1);
        cx.c.rect_grad(-1000.0, hero_h * 0.36, WIDTH + 2000.0, hero_h * 0.64 + 1.0, 0.0, t::BG.a(0.0), t::BG);
        y = hero_h + 24.0;
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
    }
    if !rail.is_empty() {
        let tt = Text::new(18.0, t::INK).semi();
        cx.c.text("Continue reading", PAD, y, tt);
        cx.c.icon("chevron-right", WIDTH - PAD - 8.0, y + tt.lh() / 2.0, 18.0, t::INK2);
        if cx.tap(0.0, y - 10.0, WIDTH, tt.lh() + 20.0) {
            to_space = true;
        }
        y += tt.lh() + 14.0;
        let (cw, ch) = (116.0, 164.0);
        let title_t = Text::new(13.5, t::INK).semi();
        let line_t = Text::new(12.5, t::INK3);
        let cell_h = ch + 8.0 + title_t.lh() + 2.0 + line_t.lh();
        let area = [0.0, y + cx.c.dy, WIDTH, cell_h];
        cx.ui.claim_h(area);
        moving |= st.rail.update(cx.ui, area, dt);
        let mut x = PAD - st.rail.off;
        for (j, (_, it)) in rail.iter().enumerate() {
            if x + cw > -10.0 && x < WIDTH + 10.0 {
                cover(&mut cx, &mut app.images, it, x, y, cw, ch, COVER_R, now);
                if cx.pressed(x, y, cw, cell_h) {
                    cx.c.rect(x, y, cw, ch, COVER_R, t::WASH);
                }
                let (f, words, new) = match it {
                    Item::Series(s) => {
                        let at = logic::at_chapter(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())).map_or("Ch. 1".into(), |n| format!("Ch. {}", logic::num(n)));
                        let new = rail_new[j];
                        let words = if new > 0 { format!("{at} · ") } else { format!("{at} · {} unread", s.unread) };
                        (logic::read_fraction(s), words, (new > 0).then(|| format!("{new} new")))
                    }
                    Item::Book(b) => (b.progress, crate::library::book_line(b), None),
                };
                if new.is_some() {
                    new_tag(cx.c, x + 6.0, y + 6.0, "NEW");
                }
                // How far: the accent along the cover's foot.
                if f > 0.0 {
                    cx.c.rect(x + 1.0, y + ch - 4.0, cw - 2.0, 3.0, 1.5, Rgba::hex(0x000000).a(0.55));
                    cx.c.rect(x + 1.0, y + ch - 4.0, ((cw - 2.0) * f).max(3.0), 3.0, 1.5, t::ACCENT);
                }
                cx.c.text_fit(it.title(), x, y + ch + 8.0, cw, title_t);
                let ly = y + ch + 8.0 + title_t.lh() + 2.0;
                let lw = cx.c.text_fit(&words, x, ly, cw, line_t);
                if let Some(nw) = &new {
                    cx.c.text_fit(nw, x + lw, ly, cw - lw, line_t.color(t::ACCENT));
                }
                if cx.tap(x, y, cw, cell_h) {
                    match it {
                        Item::Series(s) => open_series = Some(s.id),
                        Item::Book(b) => open_book = Some(b.id),
                    }
                }
            }
            x += cw + 12.0;
        }
        st.rail.max = x + st.rail.off - 12.0 + PAD - WIDTH;
        y += cell_h + 32.0;
    }
    if !updated.is_empty() {
        let tt = Text::new(18.0, t::INK).semi();
        cx.c.text("Recently updated", PAD, y, tt);
        y += tt.lh() + 8.0;
        let rh = 80.0;
        for (j, s) in updated.iter().enumerate() {
            if cx.pressed(0.0, y, WIDTH, rh) {
                cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
            }
            let (cw, ch) = (48.0, 66.0);
            cover(&mut cx, &mut app.images, &Item::Series((*s).clone()), PAD, y + (rh - ch) / 2.0, cw, ch, 6.0, now);
            let tx = PAD + cw + 14.0;
            let when = logic::ago(s.updated_at, now_ms);
            let wt = Text::new(12.5, t::INK3).right();
            let ww = cx.c.measure(&when, wt);
            let title_t = Text::new(ty::BODY, t::INK).medium();
            let line_t = Text::new(12.5, t::INK3);
            let block = title_t.lh() + 4.0 + 18.0;
            let ty0 = y + (rh - block) / 2.0;
            cx.c.text_fit(&s.title, tx, ty0, WIDTH - PAD - ww - 12.0 - tx, title_t);
            cx.c.text(&when, WIDTH - PAD, ty0 + 2.0, wt);
            let new = updated_new[j];
            let lw = cx.c.text_fit(&logic::updated_words(s, new), tx, ty0 + title_t.lh() + 4.0 + (18.0 - line_t.lh()) / 2.0, WIDTH - PAD - tx - 50.0, line_t);
            if new > 0 {
                new_tag(cx.c, tx + lw + 8.0, ty0 + title_t.lh() + 4.0, "NEW");
            }
            if cx.tap(0.0, y, WIDTH, rh) {
                open_series = Some(s.id);
            }
            y += rh;
        }
        y += 8.0;
    }
    if n > 0
        && let Some(e) = &error
    {
        y += cx.c.para(e, PAD, y, WIDTH - 2.0 * PAD, Text::new(ty::CAPTION, t::DANGER), 3) + 8.0;
    }
    let bottom = y + kit::TABS_H;
    // The fixed top: the name over the spotlight, a band of ground once the page scrolls.
    let mut settings = false;
    let mut retry = false;
    {
        let scrolled = (-cx.c.dy).clamp(0.0, 60.0) / 60.0;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 56.0, 0.0, t::BG.a(scrolled));
        kit::fixed_top(&mut cx, 56.0);
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
        cx.c.circle(WIDTH - 6.0 - 22.0, top + 28.0, 18.0, t::BG.a(0.32 * (1.0 - scrolled)));
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 6.0, "gear") {
            settings = true;
        }
    }
    let tab = kit::tabs(&mut cx, 0);
    app.page_end(&pg, bottom);
    if let Some(at) = wake {
        app.wake_at(at);
    }
    if retry {
        app.images.retry_failed();
        app.ask(Job::Library);
    }
    if add_books {
        app.host.pick_books();
    }
    if let Some(id) = continue_series {
        app.continue_series(id);
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
        app.st.space.tab = 0;
        app.go_tab(2);
    } else if let Some(i) = tab {
        app.go_tab(i);
    }
    pg.moving || moving || app.library.loading && !crate::wake::has()
}

/// The deep tone a drawn book cover has, for a spotlight with no picture.
fn book_tone(b: &Book) -> Rgba {
    const TONES: [u32; 6] = [0x24405E, 0x5C2E2E, 0x24493F, 0x463A63, 0x5E4724, 0x343E4C];
    let hash = b.title.bytes().fold(7u32, |h, x| h.wrapping_mul(31).wrapping_add(x as u32));
    Rgba::hex(TONES[(hash % TONES.len() as u32) as usize])
}
