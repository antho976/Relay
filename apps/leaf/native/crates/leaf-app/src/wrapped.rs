//! Leaf Wrapped (design 45, the owner's pick, 2026-11-25): your month, or your year, as a story of
//! cards you tap through. It is the one place reading numbers show (no stats tab): chapters read,
//! the longest streak, the best day, your series of the month, the new ones, the time, what you
//! caught up on, then a card to share. The numbers are [`crate::wrapped_stats`]'s.
//!
//! Home offers it in a month's first week (the year's in January's first two weeks), and My
//! space's ⋯ opens last month's any time. A tap on the right two thirds goes on, on the left back;
//! a finger held pauses; each card moves on by itself after [`CARD_NS`]; Back, ✕ or the last
//! card's Done puts it away.

use std::collections::HashMap;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::CoverOf;
use crate::kit;
use crate::theme::{self as t, PAD};
use crate::wrapped_stats::{self as ws, Period, Stats};

/// A card stays this long before the next comes.
pub const CARD_NS: u64 = 8_000_000_000;
const WHITE: Rgba = Rgba::WHITE;

/// The cards a period has, in order: those with nothing to say are left out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Card {
    Hello,
    Chapters,
    Streak,
    BestDay,
    Top,
    New,
    Time,
    Caught,
    Share,
}

pub fn cards(p: &Period, s: &Stats) -> Vec<Card> {
    let mut v = vec![Card::Hello];
    if s.chapters > 0 {
        v.push(Card::Chapters);
    }
    if s.days_read > 1 {
        v.push(Card::Streak);
    }
    if p.month.is_some() && s.best_day.as_ref().is_some_and(|b| b.1 > 1) {
        v.push(Card::BestDay);
    }
    if !s.top.is_empty() {
        v.push(Card::Top);
    }
    if !s.new_series.is_empty() || p.month.is_none() {
        v.push(Card::New);
    }
    if p.month.is_some() && s.comic_secs + s.book_secs >= 600 {
        v.push(Card::Time);
    }
    if p.month.is_some() && (!s.caught_up.is_empty() || !s.books_done.is_empty()) {
        v.push(Card::Caught);
    }
    v.push(Card::Share);
    v
}

#[derive(Default)]
pub struct State {
    pub period: Option<Period>,
    pub at: usize,
    /// When the card on screen came (ns).
    pub since: u64,
    /// The time it had been shown when a finger came down on it (paused), ns.
    held: Option<u64>,
    /// Worked out once per period and history.
    stats: Option<(Period, usize, Stats)>,
}

/// The two colours of each card's ground, top to bottom.
fn ground(c: Card) -> (Rgba, Rgba) {
    let (a, b) = match c {
        Card::Hello | Card::Caught => (0x0F3B27, 0x06140E),
        Card::Chapters => (0x1F2A5C, 0x0C1230),
        Card::Streak => (0x4A1F3D, 0x1D0B18),
        Card::BestDay => (0x5A3415, 0x211306),
        Card::Top | Card::Share => (0x151517, 0x0D0D0E),
        Card::New => (0x12343B, 0x06171A),
        Card::Time => (0x2B2140, 0x0F0B18),
    };
    (Rgba::hex(a), Rgba::hex(b))
}

/// The words over a card: big numbers in the display face, the rest plain.
fn big(c: &mut crate::draw::Canvas, s: &str, y: f32, size: f32) -> f32 {
    let tt = Text::new(size, WHITE).brand().line(1.0);
    let lines = c.wrap(s, WIDTH - 2.0 * PAD, tt, 3);
    let mut yy = y;
    for l in &lines {
        c.text(l, PAD, yy, tt);
        yy += tt.lh();
    }
    yy - y
}

fn words(c: &mut crate::draw::Canvas, s: &str, y: f32, size: f32, dim: bool) -> f32 {
    let tt = Text::new(size, if dim { WHITE.a(0.72) } else { WHITE }).semi().line(1.3);
    c.para(s, PAD, y, WIDTH - 2.0 * PAD, tt, 4)
}

#[allow(clippy::too_many_arguments)]
fn cover(cx: &mut kit::Ctx, im: &mut crate::images::Images, id: i64, x: f32, y: f32, w: f32, h: f32, r: f32, now: u64) {
    let of = CoverOf::Series(id);
    cx.c.rect(x, y, w, h, r, WHITE.a(0.08));
    if im.cover(cx.c, of, now) {
        cx.c.image_cover(&of.key(), x, y, w, h, r, WHITE);
    }
}

fn day_words(d: i64) -> String {
    const WD: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    let (_, m, dd) = ws::date_of_day(d);
    format!("{} {} {dd}", WD[crate::rhythm::weekday(d) as usize], ws::MONTHS[(m - 1) as usize])
}

fn short_date(d: i64) -> String {
    let (_, m, dd) = ws::date_of_day(d);
    format!("{} {dd}", &ws::MONTHS[(m - 1) as usize][..3])
}

/// "9:40 in the morning", "1:15 at night".
fn time_words(ms: i64, offset_s: i64) -> String {
    let m = (ms.div_euclid(1000) + offset_s).div_euclid(60).rem_euclid(1440);
    let (h, mm) = (m / 60, m % 60);
    let part = match h {
        5..=11 => "in the morning",
        12..=17 => "in the afternoon",
        18..=21 => "in the evening",
        _ => "at night",
    };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("{h12}:{mm:02} {part}")
}

/// The text a Share sends: the period's readings in a few lines.
pub fn share_words(p: &Period, s: &Stats, title: &dyn Fn(i64) -> String) -> String {
    let what = if p.month.is_some() { format!("My {} in Leaf", p.name()) } else { format!("My {} in Leaf", p.year) };
    let mut lines = vec![what, format!("{} chapters · {} reading", s.chapters, ws::hours_words(s.comic_secs + s.book_secs))];
    if s.streak.0 > 1 {
        lines.push(format!("Longest streak: {} days", s.streak.0));
    }
    if !s.new_series.is_empty() {
        lines.push(format!("New series: {}", s.new_series.len()));
    }
    if let Some((id, n, ..)) = s.top.first() {
        lines.push(format!("Top series: {} ({n} chapters)", title(*id)));
    }
    lines.join("\n")
}

/// Open Wrapped for a period.
pub fn open(app: &mut App, p: Period) {
    app.st.wrapped = State { period: Some(p), since: app.now_ns, ..Default::default() };
    app.want_history();
    app.wrapped_seen(&p.key());
    app.push(crate::app::Route::Wrapped(p.key()));
}

pub fn draw(app: &mut App, key: &str) -> bool {
    let Some(p) = Period::parse(key) else {
        app.pop();
        return true;
    };
    let now = app.now_ns;
    if app.st.wrapped.period != Some(p) {
        app.st.wrapped = State { period: Some(p), since: now, ..Default::default() };
        app.want_history();
    }
    if app.history.data.is_none() && app.history.error.is_none() {
        app.want_history();
    }
    // The series' names and whether they are caught up come from the library.
    if app.library.data.is_none() && !app.library.loading && app.server.is_some() {
        app.library.loading = true;
        app.ask(crate::net::Job::Library);
    }
    let offset = crate::shelf::zone_offset();
    // The numbers, once the history is here (worked out again only when it changes).
    let hist_len = app.history.data.as_ref().map_or(0, |h| h.len()) * 1000 + app.library.data.as_ref().map_or(0, |l| l.len());
    if app.history.data.is_some() && !app.st.wrapped.stats.as_ref().is_some_and(|(q, n, _)| *q == p && *n == hist_len) {
        let s = app.wrapped_stats(&p);
        app.st.wrapped.stats = Some((p, hist_len, s));
    }
    let stats = app.st.wrapped.stats.as_ref().map(|(_, _, s)| s.clone());
    let titles: HashMap<i64, String> = app.library.data.iter().flatten().map(|s| (s.id, s.title.clone())).collect();
    let title = |id: i64| titles.get(&id).cloned().unwrap_or_else(|| "A series".into());
    let list = stats.as_ref().map(|s| cards(&p, s)).unwrap_or_else(|| vec![Card::Hello]);
    let st = &mut app.st.wrapped;
    // Kept as asked until the cards are known (the numbers may still be coming).
    if stats.is_some() {
        st.at = st.at.min(list.len() - 1);
    }
    let card = list[st.at.min(list.len() - 1)];
    // The clock: paused while a finger is down, on by itself after CARD_NS.
    let down = app.ui.finger().is_some();
    match (down, st.held) {
        (true, None) => st.held = Some(now.saturating_sub(st.since)),
        (false, Some(shown)) => {
            st.since = now.saturating_sub(shown);
            st.held = None;
        }
        _ => {}
    }
    let shown = st.held.unwrap_or_else(|| now.saturating_sub(st.since));
    let last = st.at + 1 >= list.len();
    let mut go: Option<isize> = None;
    if !last && shown >= CARD_NS && stats.is_some() && !app.shot {
        go = Some(1);
    }
    let (n_cards, at) = (list.len(), st.at.min(list.len() - 1));
    let mut close = false;
    let mut share = false;
    {
        let mut cx = cx!(app);
        cx.c.dy = 0.0;
        cx.ui.cover();
        let h = cx.c.h;
        let (top, bottom) = ground(card);
        cx.c.rect_grad(-1000.0, -10.0, WIDTH + 2000.0, h + 20.0, 0.0, top, bottom);
        let y0 = cx.c.inset_top + 96.0;
        match (&stats, card) {
            (None, _) => {
                big(cx.c, &format!("Your {} in Leaf", p.name()), y0 + 60.0, 44.0);
                let words_ = if app.history.error.is_some() { "Your server didn't answer. Wrapped needs it once to count." } else { "Counting your chapters" };
                words(cx.c, words_, y0 + 200.0, 17.0, true);
            }
            (Some(s), c) => draw_card(&mut cx, &mut app.images, &p, s, c, y0, &title, offset, now, &mut share, &mut close, last),
        }
        // The story's chrome: a segment per card, the name, ✕; and where this is from.
        let top_y = cx.c.inset_top + 10.0;
        let seg_w = (WIDTH - 2.0 * 16.0 - (n_cards as f32 - 1.0) * 4.0) / n_cards as f32;
        for k in 0..n_cards {
            let sx = 16.0 + k as f32 * (seg_w + 4.0);
            cx.c.rect(sx, top_y, seg_w, 3.0, 1.5, WHITE.a(0.25));
            let f = if k < at { 1.0 } else if k == at { if last { 1.0 } else { (shown as f32 / CARD_NS as f32).min(1.0) } } else { 0.0 };
            if f > 0.0 {
                cx.c.rect(sx, top_y, seg_w * f, 3.0, 1.5, WHITE);
            }
        }
        let wt = Text::new(14.0, WHITE).semi();
        let lw = cx.c.text("Leaf", PAD, top_y + 30.0 - wt.lh() / 2.0, wt.color(t::ACCENT));
        cx.c.text("Wrapped", PAD + lw + 6.0, top_y + 30.0 - wt.lh() / 2.0, wt);
        if kit::icon_key_ink(&mut cx, WIDTH - 6.0 - 44.0, top_y + 8.0, "close", WHITE) {
            close = true;
        }
        let foot = match p.month {
            Some(_) => format!("Leaf · {} {}", p.name(), p.year),
            None => format!("Leaf · {}", p.year),
        };
        cx.c.text(&foot, PAD, h - cx.c.inset_bottom - 36.0, Text::new(12.0, WHITE.a(0.56)).semi());
        // A tap: back on the left third, on elsewhere (the keys above took theirs).
        if !close && !share {
            if cx.ui.tapped(0.0, top_y + 40.0, WIDTH / 3.0, h) {
                go = Some(-1);
            } else if cx.ui.tapped(WIDTH / 3.0, top_y + 40.0, WIDTH, h) && !last {
                go = Some(1);
            }
        }
        cx.ui.tap = None;
    }
    if let Some(d) = go {
        let st = &mut app.st.wrapped;
        st.at = (st.at as isize + d).clamp(0, n_cards as isize - 1) as usize;
        st.since = now;
        st.held = None;
    }
    if share && let Some(s) = &stats {
        let text = share_words(&p, s, &title);
        if let Some(said) = app.host.share_text(&text) {
            app.toast(&said);
        }
    }
    if close {
        app.pop();
        return true;
    }
    if !last {
        app.wake_at(now + 33_000_000);
    }
    !last
}

#[allow(clippy::too_many_arguments)]
fn draw_card(cx: &mut kit::Ctx, im: &mut crate::images::Images, p: &Period, s: &Stats, card: Card, y0: f32, title: &dyn Fn(i64) -> String, offset: i64, now: u64, share: &mut bool, close: &mut bool, last: bool) {
    let name = p.name();
    let h = cx.c.h;
    match card {
        Card::Hello => {
            let quiet = s.chapters == 0 && s.book_secs < 600;
            let head = match (p.month, quiet) {
                (Some(_), false) => format!("Your {name} in Leaf"),
                (Some(_), true) => format!("A quiet {name}"),
                (None, _) => format!("{}, read.", p.year),
            };
            let mut y = y0 + 60.0;
            y += big(cx.c, &head, y, if p.month.is_some() { 48.0 } else { 60.0 }) + 24.0;
            let sub = match (p.month, quiet) {
                (_, true) => "Hardly a chapter this time. The ones waiting will keep.".to_string(),
                (Some(_), false) => format!("{} days. {} with reading.", p.days(), s.days_read),
                (None, false) => format!("Your year in Leaf: {} chapters, {} series begun.", group(s.chapters), s.new_series.len()),
            };
            words(cx.c, &sub, y, 18.0, true);
            let ids: Vec<i64> = s.top.iter().take(5).map(|t| t.0).collect();
            let (cw, ch) = (120.0, 170.0);
            for (k, id) in ids.iter().enumerate() {
                let x = 24.0 + k as f32 * 58.0;
                cover(cx, im, *id, x, h * 0.56 + (k % 2) as f32 * 26.0, cw, ch, 12.0, now);
            }
        }
        Card::Chapters => {
            let mut y = y0;
            y += words(cx.c, "You read", y, 20.0, true) + 4.0;
            y += big(cx.c, &group(s.chapters), y, 112.0) + 4.0;
            y += words(cx.c, if p.month.is_some() { "chapters" } else { "chapters this year" }, y, 26.0, false) + 8.0;
            let per = s.chapters as f32 / p.days().max(1) as f32;
            let mut line = format!("{per:.1} a day.");
            if let Some(v) = ws::versus_words(s.chapters, s.before, &p.before().name()) {
                line = format!("{line} {v}.");
            }
            words(cx.c, &line, y, 16.0, true);
            // The days as bars (the months, for a year), the best in the accent.
            let vals: Vec<usize> = if p.month.is_some() { s.per_day.clone() } else { s.per_month.to_vec() };
            let base = h * 0.76;
            let n = vals.len().max(1) as f32;
            let gap = if p.month.is_some() { 2.5 } else { 4.6 };
            let bw = (WIDTH - 2.0 * PAD - (n - 1.0) * gap) / n;
            let mx = vals.iter().copied().max().unwrap_or(1).max(1);
            let best = vals.iter().enumerate().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0))).map(|(i, _)| i);
            for (k, v) in vals.iter().enumerate() {
                let bh = (190.0 * *v as f32 / mx as f32).max(3.0);
                let ink = if Some(k) == best && *v > 0 { t::ACCENT } else if *v > 0 { WHITE.a(0.35) } else { WHITE.a(0.1) };
                cx.c.rect(PAD + k as f32 * (bw + gap), base - bh, bw, bh, 2.0, ink);
            }
            let lt = Text::new(11.0, WHITE.a(0.56)).medium();
            if p.month.is_some() {
                cx.c.text(&short_date(p.from), PAD, base + 8.0, lt);
                cx.c.text(&short_date(p.to - 1), WIDTH - PAD, base + 8.0, lt.right());
                if let Some(i) = best.filter(|i| vals[*i] > 0) {
                    words(cx.c, &format!("{} stands out.", day_words(p.from + i as i64)), base + 40.0, 15.0, true);
                }
            } else {
                for k in 0..12 {
                    cx.c.text(&ws::MONTHS[k][..1], PAD + k as f32 * (bw + gap) + bw / 2.0, base + 8.0, lt.center());
                }
                if let Some(i) = best.filter(|i| vals[*i] > 0) {
                    words(cx.c, &format!("Your biggest month was {}.", ws::MONTHS[i]), base + 40.0, 15.0, true);
                }
            }
        }
        Card::Streak => {
            let mut y = y0;
            y += words(cx.c, "Longest streak", y, 20.0, true) + 4.0;
            let (n, from) = s.streak;
            y += big(cx.c, &format!("{n} day{}", if n == 1 { "" } else { "s" }), y, 92.0) + 8.0;
            y += words(cx.c, &format!("{} to {}, not a day missed.", short_date(from), short_date(from + n as i64 - 1)), y, 16.0, true) + 28.0;
            if p.month.is_some() {
                // The month as a calendar, the streak's days in the accent.
                let cell = 40.0;
                let cg = 6.0;
                let ox = (WIDTH - 7.0 * cell - 6.0 * cg) / 2.0;
                for (k, d) in ["M", "T", "W", "T", "F", "S", "S"].iter().enumerate() {
                    cx.c.text(d, ox + k as f32 * (cell + cg) + cell / 2.0, y, Text::new(11.0, WHITE.a(0.56)).semi().center());
                }
                y += 22.0;
                let lead = crate::rhythm::weekday(p.from) as usize;
                for i in 0..p.days() {
                    let d = p.from + i as i64;
                    let (r, c) = ((i + lead) / 7, (i + lead) % 7);
                    let (x, yy) = (ox + c as f32 * (cell + cg), y + r as f32 * (cell + cg));
                    let read = s.per_day.get(i).is_some_and(|n| *n > 0);
                    let on = d >= from && d < from + n as i64;
                    cx.c.rect(x, yy, cell, cell, 8.0, if on { t::ACCENT } else if read { WHITE.a(0.2) } else { WHITE.a(0.06) });
                    cx.c.text(&(i + 1).to_string(), x + 6.0, yy + 5.0, Text::new(11.0, if on { t::ON_ACCENT } else { WHITE.a(if read { 0.72 } else { 0.56 }) }).semi());
                }
                let rows = (p.days() + lead).div_ceil(7);
                y += rows as f32 * (cell + cg) + 16.0;
                let best = if n >= s.best_streak && n > 1 { format!("Your best ever is {}, now matched.", s.best_streak) } else { format!("Your best ever is {} days.", s.best_streak) };
                words(cx.c, &format!("{} of {} days with reading. {best}", s.days_read, p.days()), y, 15.0, true);
            } else {
                // The year as a strip per month: a square per day.
                let (cell, cg) = (8.0, 2.4);
                for m in 0..12u32 {
                    let mp = Period::month(p.year, m + 1);
                    let yy = y + m as f32 * 24.0;
                    cx.c.text(&ws::MONTHS[m as usize][..1], PAD, yy, Text::new(11.0, WHITE.a(0.56)).semi());
                    for i in 0..mp.days() {
                        let d = mp.from + i as i64;
                        let n_d = s.per_day.get((d - p.from) as usize).copied().unwrap_or(0);
                        let on = d >= from && d < from + n as i64;
                        let ink = if on { t::ACCENT } else if n_d > 0 { WHITE.a(0.2 + 0.5 * (n_d.min(20) as f32 / 20.0)) } else { WHITE.a(0.06) };
                        cx.c.rect(PAD + 18.0 + i as f32 * (cell + cg), yy, cell, cell + 6.0, 2.0, ink);
                    }
                }
                y += 12.0 * 24.0 + 16.0;
                words(cx.c, &format!("{} days with reading this year.", s.days_read), y, 15.0, true);
            }
        }
        Card::BestDay => {
            let Some((d, n, first, last_at, top)) = &s.best_day else { return };
            let mut y = y0;
            y += words(cx.c, "Most in one day", y, 20.0, true) + 4.0;
            y += big(cx.c, &n.to_string(), y, 136.0) + 8.0;
            y += words(cx.c, &format!("chapters on {}", day_words(*d)), y, 22.0, false) + 8.0;
            y += words(cx.c, &format!("From {} to {}.", time_words(*first, offset), time_words(*last_at, offset)), y, 15.0, true) + 30.0;
            let most = top.first().map_or(1, |x| x.1).max(1);
            for (k, (id, c)) in top.iter().enumerate() {
                let yy = y + k as f32 * 82.0;
                cover(cx, im, *id, PAD, yy, 52.0, 70.0, 8.0, now);
                cx.c.text_fit(&title(*id), PAD + 66.0, yy + 12.0, WIDTH - PAD - 66.0 - PAD - 90.0, Text::new(17.0, WHITE).semi());
                cx.c.text(&format!("{c} chapter{}", if *c == 1 { "" } else { "s" }), PAD + 66.0, yy + 38.0, Text::new(14.0, WHITE.a(0.72)));
                let bw = 80.0 * *c as f32 / most as f32;
                cx.c.rect(WIDTH - PAD - bw, yy + 30.0, bw, 8.0, 4.0, if k == 0 { t::ACCENT } else { WHITE.a(0.35) });
            }
        }
        Card::Top => {
            let Some((id, n, secs, from_one)) = s.top.first().copied() else { return };
            let of = CoverOf::Series(id);
            if cx.c.has_image(&of.banner_key()) {
                cx.c.image_cover(&of.banner_key(), -20.0, -20.0, WIDTH + 40.0, h * 0.6, 0.0, WHITE.a(0.5));
            }
            cx.c.rect_grad(-1000.0, h * 0.3, WIDTH + 2000.0, h * 0.25, 0.0, Rgba::hex(0x0D0D0E).a(0.0), Rgba::hex(0x0D0D0E));
            cx.c.rect(-1000.0, h * 0.55, WIDTH + 2000.0, h * 0.5, 0.0, Rgba::hex(0x0D0D0E));
            let lead = if p.month.is_some() { "Your series of the month" } else { "Your top five" };
            let mut y = y0 - 30.0;
            words(cx.c, lead, y, 18.0, true);
            y += 40.0;
            let (cw, ch) = (170.0, 240.0);
            cover(cx, im, id, (WIDTH - cw) / 2.0, y, cw, ch, 14.0, now);
            y += ch + 20.0;
            let tt = Text::new(26.0, WHITE).brand().center().line(1.1);
            for l in cx.c.wrap(&title(id), WIDTH - 2.0 * PAD, tt, 2) {
                cx.c.text(&l, WIDTH / 2.0, y, tt);
                y += tt.lh();
            }
            y += 8.0;
            let mut line = format!("{n} chapters · about {}", ws::hours_words(secs));
            if from_one {
                line.push_str(" · from chapter 1");
            }
            cx.c.text_fit(&line, WIDTH / 2.0, y, WIDTH - 2.0 * PAD, Text::new(15.0, WHITE.a(0.72)).center());
            y += 40.0;
            let room = ((h - cx.c.inset_bottom - 70.0 - y) / 56.0).floor().max(0.0) as usize;
            for (k, (id, n, ..)) in s.top.iter().skip(1).take(room.min(4)).enumerate() {
                let yy = y + k as f32 * 56.0;
                cx.c.text(&(k + 2).to_string(), PAD, yy + 10.0, Text::new(20.0, WHITE.a(0.56)).brand());
                cover(cx, im, *id, PAD + 32.0, yy, 34.0, 46.0, 6.0, now);
                cx.c.text_fit(&title(*id), PAD + 78.0, yy + 12.0, WIDTH - 2.0 * PAD - 78.0 - 50.0, Text::new(16.0, WHITE).semi());
                cx.c.text(&n.to_string(), WIDTH - PAD, yy + 13.0, Text::new(15.0, WHITE.a(0.72)).semi().right());
            }
        }
        Card::New => {
            let mut y = y0;
            if p.month.is_some() {
                y += words(cx.c, "New series started", y, 20.0, true) + 4.0;
                y += big(cx.c, &s.new_series.len().to_string(), y, 136.0) + 8.0;
                y += words(cx.c, "series you'd never opened before", y, 22.0, false) + 24.0;
                let per_row = 4;
                for (k, id) in s.new_series.iter().take(8).enumerate() {
                    let (r, c) = (k / per_row, k % per_row);
                    cover(cx, im, *id, PAD + c as f32 * 88.0, y + r as f32 * 116.0, 76.0, 104.0, 8.0, now);
                }
                y += s.new_series.len().min(8).div_ceil(per_row) as f32 * 116.0 + 12.0;
                if let Some((id, n)) = s.took {
                    words(cx.c, &format!("The one that took: {}, {n} chapters in its first two days.", title(id)), y, 15.0, true);
                }
            } else {
                y += words(cx.c, "The year in series", y, 20.0, true) + 16.0;
                for (v, l) in [(s.new_series.len(), "new series started"), (s.caught_up.len(), "caught up")] {
                    y += big(cx.c, &v.to_string(), y, 76.0);
                    y += words(cx.c, l, y + 4.0, 18.0, true) + 30.0;
                }
                if let Some((id, at)) = s.first {
                    let d = (at.div_euclid(1000) + offset).div_euclid(86_400);
                    y += words(cx.c, &format!("First of the year: {}, {} at {}.", title(id), short_date(d), time_words(at, offset)), y, 16.0, false) + 14.0;
                }
                if let Some((id, n)) = s.binge.filter(|b| b.1 > 1) {
                    words(cx.c, &format!("Biggest binge: {}, {n} chapters in two days.", title(id)), y, 16.0, false);
                }
            }
        }
        Card::Time => {
            let mut y = y0;
            let total = s.comic_secs + s.book_secs;
            y += words(cx.c, "Time reading", y, 20.0, true) + 4.0;
            y += big(cx.c, &format!("About {}", ws::hours_words(total)), y, 64.0) + 10.0;
            if let Some(w) = ws::hour_words(&s.hours) {
                words(cx.c, &w, y, 22.0, false);
            }
            let base = h * 0.74;
            let mx = s.hours.iter().copied().max().unwrap_or(1).max(1);
            let (hw, hg) = ((WIDTH - 2.0 * PAD - 23.0 * 2.3) / 24.0, 2.3);
            let top3: Vec<usize> = {
                let mut v: Vec<usize> = (0..24).collect();
                v.sort_by(|a, b| s.hours[*b].cmp(&s.hours[*a]));
                v.into_iter().take(3).collect()
            };
            for (k, v) in s.hours.iter().enumerate() {
                let bh = (200.0 * *v as f32 / mx as f32).max(3.0);
                let ink = if top3.contains(&k) && *v > 0 { t::ACCENT } else if *v > 0 { WHITE.a(0.35) } else { WHITE.a(0.1) };
                cx.c.rect(PAD + k as f32 * (hw + hg), base - bh, hw, bh, 3.0, ink);
            }
            let lt = Text::new(11.0, WHITE.a(0.56)).medium();
            for (k, l) in [(0, "12 am"), (6, "6 am"), (12, "noon"), (18, "6 pm")] {
                cx.c.text(l, PAD + k as f32 * (hw + hg), base + 8.0, lt);
            }
            let split = match (s.comic_secs > 0, s.book_secs > 0) {
                (true, true) => format!("Comics about {}, books {}.", ws::hours_words(s.comic_secs), ws::hours_words(s.book_secs)),
                (true, false) => "All of it comics.".to_string(),
                _ => "All of it books.".to_string(),
            };
            words(cx.c, &split, base + 40.0, 15.0, true);
        }
        Card::Caught => {
            let mut y = y0;
            let n = s.caught_up.len();
            if n > 0 {
                y += words(cx.c, "Caught up on", y, 20.0, true) + 4.0;
                y += big(cx.c, &format!("{n} series"), y, 80.0) + 6.0;
                y += words(cx.c, "every chapter there is, read", y, 20.0, false) + 24.0;
                for (k, id) in s.caught_up.iter().take(5).enumerate() {
                    let (x, yy) = (PAD + k as f32 * 62.0, y + (k % 2) as f32 * 18.0);
                    cover(cx, im, *id, x, yy, 96.0, 134.0, 10.0, now);
                    cx.c.circle(x + 83.0, yy + 5.0, 13.0, t::ACCENT);
                    cx.c.icon("check", x + 83.0, yy + 5.0, 15.0, t::ON_ACCENT);
                }
                y += 134.0 + 18.0 + 30.0;
            }
            let books = &s.books_done;
            if !books.is_empty() {
                let pages: u32 = books.iter().map(|b| b.1).sum();
                let lead = if n > 0 { "And " } else { "" };
                let what = if books.len() == 1 { "1 book".to_string() } else { format!("{} books", books.len()) };
                let tail = if pages > 0 { format!(": {pages} pages.") } else { ".".to_string() };
                y += words(cx.c, &format!("{lead}{what} finished{tail}"), y, 20.0, false) + 6.0;
                let names: Vec<&str> = books.iter().take(3).map(|b| b.0.as_str()).collect();
                words(cx.c, &format!("{}.", names.join(", ")), y, 15.0, true);
            }
        }
        Card::Share => {
            // The summary, as it is shared, and the keys.
            let (x, y, w, ph) = (PAD, y0 - 20.0, WIDTH - 2.0 * PAD, 480.0_f32.min(h - y0 - 140.0));
            cx.c.rect_grad(x, y, w, ph, 22.0, Rgba::hex(0x0F3B27), Rgba::hex(0x1F2A5C));
            for (k, (id, ..)) in s.top.iter().take(5).enumerate() {
                cover(cx, im, *id, x + 18.0 + k as f32 * 62.0, y + 22.0, 54.0, 76.0, 8.0, now);
            }
            let head = if p.month.is_some() { format!("{} {}", p.name(), p.year).to_uppercase() } else { p.year.to_string() };
            cx.c.text(&head, x + 20.0, y + 116.0, Text::new(11.0, t::ACCENT).semi());
            cx.c.text(&format!("My {} in Leaf", if p.month.is_some() { "month" } else { "year" }), x + 20.0, y + 134.0, Text::new(26.0, WHITE).brand());
            let best_month = s.per_month.iter().enumerate().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0))).map(|(i, _)| ws::MONTHS[i][..3].to_string()).unwrap_or_default();
            let cells: Vec<(String, &str)> = vec![
                (group(s.chapters), "chapters"),
                (ws::hours_words(s.comic_secs + s.book_secs).split(' ').take(2).collect::<Vec<_>>().join(" "), "reading"),
                (format!("{} days", s.streak.0), "longest streak"),
                if p.month.is_some() { (s.best_day.as_ref().map_or(0, |b| b.1).to_string(), "best day") } else { (best_month, "best month") },
                (s.new_series.len().to_string(), "new series"),
                (s.caught_up.len().to_string(), "caught up"),
            ];
            for (k, (v, l)) in cells.iter().enumerate() {
                let (r, c) = (k / 2, k % 2);
                let (cx_, cy) = (x + 20.0 + c as f32 * 160.0, y + 186.0 + r as f32 * 72.0);
                if cy + 50.0 > y + ph - 40.0 {
                    break;
                }
                cx.c.text(v, cx_, cy, Text::new(28.0, WHITE).brand());
                cx.c.text(l, cx_, cy + 36.0, Text::new(13.0, WHITE.a(0.72)).medium());
            }
            if let Some((id, ..)) = s.top.first() {
                cx.c.text_fit(&format!("Top series: {}", title(*id)), x + 20.0, y + ph - 34.0, w - 40.0, Text::new(14.0, WHITE).semi());
            }
            let ky = h - cx.c.inset_bottom - 112.0;
            let kw = (WIDTH - 2.0 * PAD - 10.0) / 2.0;
            cx.c.rect(PAD, ky, kw, 48.0, 24.0, WHITE);
            cx.c.text("Share", PAD + kw / 2.0, ky + (48.0 - Text::new(15.0, WHITE).lh()) / 2.0, Text::new(15.0, Rgba::hex(0x0D0D0E)).semi().center());
            if cx.ui.tapped(PAD, ky, kw, 48.0) {
                *share = true;
            }
            cx.c.rect(PAD + kw + 10.0, ky, kw, 48.0, 24.0, WHITE.a(0.12));
            cx.c.text("Done", PAD + kw + 10.0 + kw / 2.0, ky + (48.0 - Text::new(15.0, WHITE).lh()) / 2.0, Text::new(15.0, WHITE).semi().center());
            if cx.ui.tapped(PAD + kw + 10.0, ky, kw, 48.0) && last {
                *close = true;
            }
        }
    }
}

/// "4,612".
pub fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_month_shows_only_the_cards_it_has_numbers_for() {
        let p = Period::month(2026, 11);
        let quiet = Stats { per_day: vec![0; 30], ..Default::default() };
        assert_eq!(cards(&p, &quiet), vec![Card::Hello, Card::Share], "a quiet month: a short story");
        let busy = Stats { chapters: 40, days_read: 9, best_day: Some((0, 12, 0, 0, vec![])), top: vec![(1, 40, 3600, false)], new_series: vec![1], comic_secs: 3600, caught_up: vec![1], per_day: vec![0; 30], ..Default::default() };
        assert_eq!(cards(&p, &busy).len(), 9);
        assert_eq!(group(4612), "4,612");
        assert_eq!(group(412), "412");
        assert_eq!(time_words(9 * 3_600_000 + 40 * 60_000, 0), "9:40 in the morning");
        assert_eq!(time_words(3_600_000 + 15 * 60_000, 0), "1:15 at night");
    }
}
