//! Stats (design 42, "Stats & goals", A): this week's reading, the days as bars, and the readings
//! behind it: the streak, your pace, highlights made and reviews done. Time counts only while a
//! book's page moves (`crate::book`), so a book left open doesn't add to it.

use std::collections::HashMap;

use crate::app::App;
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::shelf;
use crate::theme::{self as t, PAD, ty};

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// "38 min more than last week", "12 min less than last week", "the same as last week".
pub fn versus(this: i64, last: i64) -> String {
    let d = (this - last) / 60;
    if d == 0 {
        "the same as last week".into()
    } else if d > 0 {
        format!("{} more than last week", shelf::duration(d * 60))
    } else {
        format!("{} less than last week", shelf::duration(-d * 60))
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let top_h = 64.0;
    let pg = app.page_begin(dt, top_h);
    let now_ms = shelf::now_ms();
    let today = shelf::day_of(now_ms);
    let time = app.shelf.time.clone();
    let week = shelf::week(&time, today);
    let last_week = shelf::week(&time, today - 7);
    let total: i64 = week.iter().sum();
    let last_total: i64 = last_week.iter().sum();
    let (streak, best) = shelf::streak(&time, today);
    let monday = today - shelf::weekday(today);
    let made = app.shelf.highlights.iter().filter(|h| shelf::day_of(h.created_ms) >= monday).count();
    let reviews: i64 = app.shelf.reviews.iter().filter(|(d, _)| **d >= monday).map(|(_, n)| *n).sum();
    let paces: Vec<f32> = app.shelf.books.iter().map(|b| b.cpm).filter(|c| *c > 0.0).collect();
    let pace = if paces.is_empty() { None } else { Some(paces.iter().sum::<f32>() / paces.len() as f32) };
    let mut per_book: HashMap<i64, i64> = HashMap::new();
    for (d, b, s) in &time {
        if *d >= monday {
            *per_book.entry(*b).or_insert(0) += s;
        }
    }
    let most = per_book.iter().max_by_key(|(_, s)| **s).and_then(|(b, s)| app.shelf.book(*b).map(|bk| (bk.title.clone(), *s)));
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top + 6.0;
    // The week's figure.
    let big = if total < 60 { "0 min".to_string() } else { shelf::duration(total) };
    cx.c.text(&big, x0, y, Text::new(40.0, t::INK).brand().line(1.15));
    y += 50.0;
    let sub = if total == 0 && last_total == 0 { "read this week".to_string() } else { format!("read this week, {}", versus(total, last_total)) };
    cx.c.text_fit(&sub, x0, y, w, Text::new(ty::UI, t::INK2));
    y += 40.0;
    // The days.
    let chart_h = 150.0;
    let max = week.iter().copied().max().unwrap_or(0).max(30 * 60);
    let top_label = shelf::duration(max).replace(" min", "m").replace(" h", "h");
    let axis = Text::new(ty::SMALL, t::INK3);
    cx.c.text(&top_label, x0, y - 6.0, axis);
    cx.c.rect(x0 + 34.0, y, w - 34.0, 1.0, 0.0, t::LINE);
    cx.c.rect(x0 + 34.0, y + chart_h, w - 34.0, 1.0, 0.0, t::LINE);
    let bw = (w - 34.0) / 7.0;
    let today_i = shelf::weekday(today) as usize;
    for (i, secs) in week.iter().enumerate() {
        let bx = x0 + 34.0 + i as f32 * bw + bw * 0.18;
        let bh = chart_h * (*secs as f32 / max as f32);
        if *secs > 0 {
            cx.c.rect(bx, y + chart_h - bh, bw * 0.64, bh.max(3.0), 4.0, if i == today_i { t::ACCENT } else { t::S3 });
        } else if i <= today_i {
            // A day not read: a mark on the line, not a gap that looks like missing data.
            cx.c.rect(bx, y + chart_h - 3.0, bw * 0.64, 3.0, 1.5, t::S2);
        }
        let lt = Text::new(ty::SMALL, if i == today_i { t::INK } else { t::INK3 }).center();
        cx.c.text(DAYS[i], bx + bw * 0.32, y + chart_h + 8.0, lt);
    }
    y += chart_h + 44.0;
    // The readings behind it.
    let cells: [(String, String); 4] = [
        (if streak == 1 { "1 day".into() } else { format!("{streak} days") }, if best > streak { format!("streak, best {best}") } else { "reading streak".into() }),
        (pace.map_or("Not yet".to_string(), |p| format!("{} wpm", (p / 5.8).round() as i64)), if pace.is_some() { "your pace in books".into() } else { "pace, once you've read a while".into() }),
        (made.to_string(), "highlights this week".into()),
        (reviews.to_string(), "reviews done this week".into()),
    ];
    let cw = (w - 16.0) / 2.0;
    for (i, (fig, cap)) in cells.iter().enumerate() {
        let (cx_, cy) = (x0 + (i % 2) as f32 * (cw + 16.0), y + (i / 2) as f32 * 72.0);
        cx.c.text(fig, cx_, cy, Text::new(ty::HEADING, t::INK).semi());
        cx.c.text_fit(cap, cx_, cy + 30.0, cw, Text::new(ty::CAPTION, t::INK3));
    }
    y += 2.0 * 72.0 + 8.0;
    match &most {
        Some((title, s)) if *s >= 60 => {
            cx.c.text_fit(&format!("Most read: {title}, {}", shelf::duration(*s)), x0, y, w, Text::new(ty::UI, t::INK2));
            y += 30.0;
        }
        _ if time.is_empty() => {
            y += cx.c.para("Time counts while a book's page moves, so one left open doesn't add to it.", x0, y, w, Text::new(ty::UI, t::INK3).line(1.45), 3);
        }
        _ => {}
    }
    let bottom = y;
    let back = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        kit::top_bar(&mut cx, top + 8.0, "Reading time", &[]).0
    };
    app.page_end(&pg, bottom);
    if back {
        app.pop();
    }
    pg.moving
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weeks_compare_in_words() {
        assert_eq!(versus(3600, 1320), "38 min more than last week");
        assert_eq!(versus(600, 1320), "12 min less than last week");
        assert_eq!(versus(600, 620), "the same as last week");
    }
}
