//! Notes and recall (design 42, "Notes & recall"): every highlight in one place, by book, with
//! today's review on top; one book's highlights by chapter, exported as Markdown; the review card
//! that brings a highlight back on a spaced schedule; and Find, which searches the library's
//! titles and your notes.

use std::collections::HashMap;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Canvas, Rgba, Text, WIDTH, Weight};
use crate::kit;
use crate::shelf::{self, Again, Book, COLOURS, Highlight};
use crate::theme::{self as t, PAD, ty};

#[derive(Default)]
pub struct State {
    /// A colour to show only (0 to 3), on Notes and a book's notes.
    pub colour: Option<u8>,
    pub chips: crate::ui::HScroll,
}

#[derive(Default)]
pub struct Review {
    pub queue: Vec<i64>,
    pub at: usize,
    pub done: usize,
}

#[derive(Default)]
pub struct Find {
    pub query: String,
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// A day in words from today: "Today", "Yesterday", "Tue", "Nov 3".
pub fn day_words(ms: i64, now: i64) -> String {
    let (d, today) = (shelf::day_of(ms), shelf::day_of(now));
    match today - d {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=6 => WEEKDAYS[shelf::weekday(d) as usize].into(),
        _ => {
            let (y, m, dd) = crate::logic::civil(d);
            let (ty_, ..) = crate::logic::civil(today);
            if y == ty_ { format!("{} {dd}", MONTHS[(m - 1) as usize]) } else { format!("{} {dd}, {y}", MONTHS[(m - 1) as usize]) }
        }
    }
}

/// A day inside a sentence: "today", "yesterday", "tomorrow", "on Thu", "on Nov 13".
pub fn when(ms: i64, now: i64) -> String {
    let (d, today) = (shelf::day_of(ms), shelf::day_of(now));
    match d - today {
        0 => "today".into(),
        -1 => "yesterday".into(),
        1 => "tomorrow".into(),
        2..=6 => format!("on {}", WEEKDAYS[shelf::weekday(d) as usize]),
        _ => format!("on {}", day_words(ms, now)),
    }
}

/// One book's highlights as Markdown, in book order, by chapter: the passage quoted, its colour's
/// meaning and your note under it.
pub fn markdown(b: &Book, hs: &[Highlight]) -> String {
    let mut hs: Vec<&Highlight> = hs.iter().filter(|h| h.book == b.id).collect();
    hs.sort_by_key(|h| (h.from, h.id));
    let mut out = format!("# {}\n", b.title);
    if !b.author.is_empty() {
        out.push_str(&format!("\n*{}*\n", b.author));
    }
    let mut chapter: Option<&str> = None;
    for h in hs {
        if chapter != Some(h.chapter.as_str()) {
            chapter = Some(h.chapter.as_str());
            if !h.chapter.is_empty() {
                out.push_str(&format!("\n## {}\n", h.chapter));
            }
        }
        out.push('\n');
        for line in h.text.lines() {
            out.push_str(&format!("> {line}\n"));
        }
        out.push_str(&format!("\n{}", COLOURS[h.colour.min(3) as usize].0));
        if !h.note.is_empty() {
            out.push_str(&format!(": {}", h.note));
        }
        out.push('\n');
    }
    out
}

/// The colours' counts in a set of highlights.
fn counts(hs: &[&Highlight]) -> [usize; 4] {
    let mut c = [0; 4];
    for h in hs {
        c[h.colour.min(3) as usize] += 1;
    }
    c
}

/// "All", then each colour that has highlights, with its count. Some(the filter) when one is tapped.
fn colour_chips(cx: &mut kit::Ctx, hs: &mut crate::ui::HScroll, y: f32, shown: &[&Highlight], filter: Option<u8>, dt: f32) -> Option<Option<u8>> {
    let n = counts(shown);
    let mut items = vec![(format!("All {}", shown.len()), filter.is_none(), None)];
    let mut which = vec![None];
    for (i, (name, col)) in COLOURS.iter().enumerate() {
        if n[i] > 0 {
            items.push((format!("{name} {}", n[i]), filter == Some(i as u8), Some(Rgba::hex(*col))));
            which.push(Some(i as u8));
        }
    }
    kit::chip_row(cx, hs, y, &items, dt).map(|i| which[i])
}

/// Four short bars, each as long as its colour's share (a book's mix of highlights).
fn mix_bars(c: &mut Canvas, x: f32, y: f32, w: f32, n: [usize; 4]) -> f32 {
    let total: usize = n.iter().sum();
    let mut bx = x;
    for (i, k) in n.iter().enumerate() {
        if *k == 0 {
            continue;
        }
        let bw = (w * *k as f32 / total.max(1) as f32).max(6.0);
        c.rect(bx, y, bw - 3.0, 3.0, 1.5, Rgba::hex(COLOURS[i].1));
        bx += bw;
    }
    bx - x
}

/// Notes: today's review, the colour chips, and one row per book with highlights.
pub fn draw(app: &mut App, dt: f32) -> bool {
    let top_h = 64.0;
    let pg = app.page_begin(dt, top_h);
    let now_ms = shelf::now_ms();
    let now = app.now_ns;
    let filter = app.st.notes.colour;
    let all: Vec<Highlight> = app.shelf.highlights.clone();
    let books: Vec<Book> = app.shelf.books.clone();
    let due = shelf::due_today(&all, now_ms, 10);
    let today = shelf::day_of(now_ms);
    let week = shelf::week(&app.shelf.time, today);
    let (streak, _) = shelf::streak(&app.shelf.time, today);
    let mut open: Option<i64> = None;
    let mut start = false;
    let mut set_filter: Option<Option<u8>> = None;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top + 4.0;
    if all.is_empty() {
        y += 70.0;
        for (i, (_, col)) in COLOURS.iter().enumerate() {
            cx.c.rect(WIDTH / 2.0 - 60.0, y + i as f32 * 14.0, 120.0 - i as f32 * 18.0, 4.0, 2.0, Rgba::hex(*col).a(0.6));
        }
        y += 90.0;
        cx.c.text("No highlights yet", WIDTH / 2.0, y, Text::new(ty::HEADING, t::INK).semi().center());
        y += 32.0;
        y += cx.c.para(
            "Long-press a word as you read, drag to the end of the passage, and pick a colour. Everything you mark collects here, and comes back to you in a daily review.",
            x0 + 20.0,
            y,
            w - 40.0,
            Text::new(ty::UI, t::INK2).center().line(1.45),
            5,
        );
    } else {
        // Today's review.
        let card_h = 158.0;
        cx.c.rect(x0, y, w, card_h, 14.0, t::S1);
        cx.c.text("Today's review", x0 + 16.0, y + 16.0, Text::new(ty::SECTION, t::INK).semi());
        if !due.is_empty() {
            cx.c.text(&format!("{} min", due.len().div_ceil(3).max(1)), x0 + w - 16.0, y + 19.0, Text::new(ty::CAPTION, t::INK3).right());
        }
        let books_due = {
            let mut b: Vec<i64> = all.iter().filter(|h| due.contains(&h.id)).map(|h| h.book).collect();
            b.sort();
            b.dedup();
            b.len()
        };
        let body = if due.is_empty() {
            let next = all.iter().filter(|h| !h.retired && h.id != 0).map(|h| h.due_ms).min();
            match next {
                Some(n) => format!("Nothing due today. The next highlight comes back {}.", when(n, now_ms).replace("today", "later today")),
                None => "Nothing left to review: every highlight is retired.".into(),
            }
        } else {
            format!(
                "{} {} from {} {}, the ones you're closest to forgetting.",
                due.len(),
                if due.len() == 1 { "highlight" } else { "highlights" },
                books_due,
                if books_due == 1 { "book" } else { "books" }
            )
        };
        cx.c.para(&body, x0 + 16.0, y + 44.0, w - 32.0 - if due.is_empty() { 0.0 } else { 110.0 }, Text::new(ty::UI, t::INK2).line(1.4), 3);
        if !due.is_empty() && kit::primary(&mut cx, x0 + w - 16.0 - 96.0, y + 50.0, 96.0, 42.0, "Start", Some("play")) {
            start = true;
        }
        // This week: a segment per day read.
        let sy = y + card_h - 44.0;
        let sw = (w - 32.0 - 6.0 * 6.0) / 7.0;
        for (i, secs) in week.iter().enumerate() {
            let on = *secs >= 60;
            cx.c.rect(x0 + 16.0 + i as f32 * (sw + 6.0), sy, sw, 4.0, 2.0, if on { t::ACCENT } else { t::S3 });
        }
        let words = match streak {
            0 => "No reading streak yet".to_string(),
            1 => "1 day read in a row".to_string(),
            n => format!("{n}-day reading streak"),
        };
        cx.c.text(&words, x0 + 16.0, sy + 14.0, Text::new(ty::CAPTION, t::INK3));
        y += card_h + 18.0;
        // Colours.
        let shown: Vec<&Highlight> = all.iter().collect();
        if let Some(f) = colour_chips(&mut cx, &mut app.st.notes.chips, y, &shown, filter, dt) {
            set_filter = Some(f);
        }
        y += 50.0;
        // Books with highlights, the one marked last first.
        let mut per: HashMap<i64, Vec<&Highlight>> = HashMap::new();
        for h in &all {
            if filter.is_none_or(|c| c == h.colour) {
                per.entry(h.book).or_default().push(h);
            }
        }
        let mut rows: Vec<(&Book, Vec<&Highlight>)> = books.iter().filter_map(|b| per.remove(&b.id).map(|v| (b, v))).collect();
        rows.sort_by_key(|(_, v)| std::cmp::Reverse(v.iter().map(|h| h.created_ms).max().unwrap_or(0)));
        if rows.is_empty() {
            cx.c.text("No highlights in this colour.", x0, y + 8.0, Text::new(ty::UI, t::INK2));
            y += 40.0;
        }
        for (b, hs) in rows {
            let rh = 72.0;
            if cx.pressed(0.0, y, WIDTH, rh) {
                cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
            }
            crate::book::cover(cx.c, &mut app.images, b, x0, y + 8.0, 40.0, 56.0, now);
            let tx = x0 + 54.0;
            cx.c.text_fit(&b.title, tx, y + 14.0, w - 54.0 - 80.0, Text::new(ty::UI, t::INK).semi());
            let bw = mix_bars(cx.c, tx, y + 42.0, 110.0, counts(&hs));
            cx.c.text(&hs.len().to_string(), tx + bw + 6.0, y + 34.0, Text::new(ty::CAPTION, t::INK3));
            let last = hs.iter().map(|h| h.created_ms).max().unwrap_or(0);
            cx.c.text(&day_words(last, now_ms), x0 + w, y + 16.0, Text::new(ty::CAPTION, t::INK3).right());
            if cx.tap(0.0, y, WIDTH, rh) {
                open = Some(b.id);
            }
            y += rh;
        }
    }
    let bottom = y + kit::TABS_H;
    let mut find = false;
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        cx.c.text("Notes", PAD, top + 14.0, Text::new(ty::TITLE, t::INK).brand());
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 10.0, "search") {
            find = true;
        }
    }
    let tab = kit::tabs(&mut cx, 1);
    app.page_end(&pg, bottom);
    if let Some(f) = set_filter {
        app.st.notes.colour = f;
    }
    if start {
        app.push(Route::Review);
    } else if let Some(id) = open {
        app.push(Route::BookNotes(id));
    } else if find {
        app.push(Route::Find);
    } else if let Some(i) = tab {
        app.reset_to([Route::Library, Route::Notes, Route::Stats][i].clone());
    }
    pg.moving
}

/// A highlight drawn as a passage: its colour's bar, the words in the reading face, your note.
/// Returns its height.
#[allow(clippy::too_many_arguments)]
fn passage(c: &mut Canvas, h: &Highlight, x: f32, y: f32, w: f32, size: f32, lines: usize, now_ms: i64, footer: bool) -> f32 {
    let tt = Text::new(size, t::INK).face(Weight::Serif).line(1.45);
    let text = h.text.replace('\n', " ");
    let th = c.para(&text, x + 14.0, y, w - 14.0, tt, lines);
    let mut hh = th;
    if !h.note.is_empty() {
        hh += 8.0;
        c.icon("chat", x + 21.0, y + hh + 9.0, 13.0, t::INK3);
        hh += c.para(&h.note, x + 34.0, y + hh, w - 34.0, Text::new(ty::UI, t::INK2).line(1.4), 4);
    }
    if footer {
        hh += 6.0;
        let words = format!("{} · {}", COLOURS[h.colour.min(3) as usize].0, day_words(h.created_ms, now_ms));
        c.text(&words, x + 14.0, y + hh, Text::new(ty::CAPTION, t::INK3));
        hh += 18.0;
    }
    c.rect(x, y + 2.0, 3.0, hh - 4.0, 1.5, Rgba::hex(COLOURS[h.colour.min(3) as usize].1));
    hh
}

/// One book's highlights by chapter, filtered by colour; Export shares them as Markdown.
pub fn draw_book(app: &mut App, id: i64, dt: f32) -> bool {
    let top_h = 60.0;
    let pg = app.page_begin(dt, top_h);
    let now_ms = shelf::now_ms();
    let Some(book) = app.shelf.book(id).cloned() else {
        app.page_end(&pg, 0.0);
        app.pop();
        return true;
    };
    let filter = app.st.notes.colour;
    let mut hs: Vec<Highlight> = app.shelf.highlights.iter().filter(|h| h.book == id).cloned().collect();
    hs.sort_by_key(|h| (h.from, h.id));
    let mut open: Option<shelf::Place> = None;
    let mut set_filter: Option<Option<u8>> = None;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top + 6.0;
    if hs.is_empty() {
        y += 60.0;
        cx.c.text("No highlights in this book yet", WIDTH / 2.0, y, Text::new(ty::SECTION, t::INK).semi().center());
        y += 28.0;
        y += cx.c.para("Long-press a word as you read to start one.", x0 + 20.0, y, w - 40.0, Text::new(ty::UI, t::INK2).center(), 3);
    } else {
        let refs: Vec<&Highlight> = hs.iter().collect();
        if let Some(f) = colour_chips(&mut cx, &mut app.st.notes.chips, y, &refs, filter, dt) {
            set_filter = Some(f);
        }
        y += 50.0;
        let mut chapter: Option<String> = None;
        let view_top = -cx.c.dy - 200.0;
        let view_bottom = -cx.c.dy + cx.c.h + 200.0;
        for h in hs.iter().filter(|h| filter.is_none_or(|c| c == h.colour)) {
            if chapter.as_ref() != Some(&h.chapter) {
                chapter = Some(h.chapter.clone());
                y += 6.0;
                let title = if h.chapter.is_empty() { "Untitled" } else { &h.chapter };
                cx.c.text_fit(title, x0, y, w, Text::new(ty::CAPTION, t::INK3).semi());
                y += 26.0;
            }
            // Measured without drawing when far off screen.
            if y > view_bottom || y < view_top - 400.0 {
                let text = h.text.replace('\n', " ");
                let mut hh = cx.c.para_h(&text, w - 14.0, Text::new(16.0, t::INK).face(Weight::Serif).line(1.45), 8);
                if !h.note.is_empty() {
                    hh += 8.0 + cx.c.para_h(&h.note, w - 34.0, Text::new(ty::UI, t::INK2).line(1.4), 4);
                }
                y += hh + 24.0 + 22.0;
                continue;
            }
            let hh = passage(cx.c, h, x0, y, w, 16.0, 8, now_ms, true);
            if cx.pressed(0.0, y - 6.0, WIDTH, hh + 12.0) {
                cx.c.rect(0.0, y - 6.0, WIDTH, hh + 12.0, 0.0, t::WASH);
            }
            if cx.tap(0.0, y - 6.0, WIDTH, hh + 12.0) {
                open = Some(h.from);
            }
            y += hh + 22.0;
        }
    }
    let bottom = y;
    let (back, key) = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        let keys: &[&'static str] = if hs.is_empty() { &[] } else { &["upload"] };
        let (back, key, _) = kit::top_bar(&mut cx, top + 8.0, &book.title, keys);
        (back, key)
    };
    app.page_end(&pg, bottom);
    if let Some(f) = set_filter {
        app.st.notes.colour = f;
    }
    if key == Some(0) {
        let md = markdown(&book, &hs);
        let name = format!("{} notes.md", book.title.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().trim());
        if let Some(words) = app.host.share_file(&name, "text/markdown", md.as_bytes()) {
            app.toast(&words);
        }
    }
    if back {
        app.pop();
    } else if let Some(p) = open {
        open_at(app, id, p);
    }
    pg.moving
}

/// Open a book at a place (from a note or the review): straight back to it when it is the book
/// under this page.
pub fn open_at(app: &mut App, id: i64, p: shelf::Place) {
    let under = app.nav.iter().any(|(r, _)| *r == Route::Book) && app.book.as_ref().is_some_and(|b| b.id == id);
    if under {
        while app.route() != &Route::Book {
            if !app.pop() {
                break;
            }
        }
    } else {
        if let Some(b) = app.shelf.book_mut(id) {
            b.place = p;
        }
        app.open_book(id);
    }
    let view_h = app.canvas.h;
    if let Some(r) = &mut app.book
        && r.id == id
    {
        if r.doc.is_some() {
            r.back_to = Some(r.pos);
            r.go(&mut app.canvas, &app.images, p, view_h);
        }
        r.chrome = false;
    }
}

/// The review: one highlight at a time, and when to show it again.
pub fn draw_review(app: &mut App, _dt: f32) -> bool {
    let now_ms = shelf::now_ms();
    let now = app.now_ns;
    if app.fresh {
        app.st.review = Review { queue: shelf::due_today(&app.shelf.highlights, now_ms, 10), at: 0, done: 0 };
    }
    let q = app.st.review.queue.clone();
    let at = app.st.review.at;
    let current = q.get(at).and_then(|id| app.shelf.highlights.iter().find(|h| h.id == *id)).cloned();
    let mut answer: Option<Again> = None;
    let mut open = false;
    let mut close = false;
    let mut cx = cx!(app);
    let top = cx.c.inset_top;
    let x0 = PAD + 4.0;
    let w = WIDTH - 2.0 * x0;
    if kit::icon_key(&mut cx, 6.0, top + 8.0, "close") {
        close = true;
    }
    // Progress: a segment per card.
    let n = q.len().max(1);
    let (px, pw) = (60.0, WIDTH - 60.0 - 70.0);
    let sw = (pw - 4.0 * (n as f32 - 1.0)) / n as f32;
    for i in 0..n {
        cx.c.rect(px + i as f32 * (sw + 4.0), top + 29.0, sw, 3.0, 1.5, if i < at { t::ACCENT } else { t::S3 });
    }
    cx.c.text(&format!("{} of {}", (at + 1).min(q.len().max(1)), q.len()), WIDTH - PAD, top + 22.0, Text::new(ty::CAPTION, t::INK3).right());
    match &current {
        Some(h) => {
            let book = app.shelf.book(h.book).cloned();
            let mut y = top + 80.0;
            if let Some(b) = &book {
                crate::book::cover(cx.c, &mut app.images, b, x0, y, 32.0, 46.0, now);
                cx.c.text_fit(&b.title, x0 + 44.0, y + 4.0, w - 44.0, Text::new(ty::UI, t::INK).semi());
                let sub = if h.chapter.is_empty() { format!("Saved {}", when(h.created_ms, now_ms)) } else { format!("{} · saved {}", h.chapter, when(h.created_ms, now_ms)) };
                cx.c.text_fit(&sub, x0 + 44.0, y + 26.0, w - 44.0, Text::new(ty::CAPTION, t::INK3));
            }
            y += 76.0;
            let tt = Text::new(23.0, t::INK).face(Weight::Serif).line(1.4);
            let text = h.text.replace('\n', " ");
            let th = cx.c.para(&text, x0 + 18.0, y, w - 18.0, tt, 10);
            cx.c.rect(x0, y + 4.0, 3.0, th - 8.0, 1.5, Rgba::hex(COLOURS[h.colour.min(3) as usize].1));
            y += th + 16.0;
            if !h.note.is_empty() {
                cx.c.icon("chat", x0 + 25.0, y + 9.0, 14.0, t::INK3);
                y += cx.c.para(&h.note, x0 + 40.0, y, w - 40.0, Text::new(ty::UI, t::INK2).line(1.4), 4) + 8.0;
            }
            if kit::plain(&mut cx, x0, y + 10.0, 150.0, 42.0, "Open in book", Some("external")) {
                open = true;
            }
            let bottom = cx.c.h - cx.c.inset_bottom;
            let ky = bottom - 16.0 - 54.0;
            cx.c.text("SHOW THIS AGAIN", x0, ky - 26.0, Text::new(ty::SMALL, t::INK3).semi());
            let kw = (w - 3.0 * 8.0) / 4.0;
            for (i, a) in Again::ALL.iter().enumerate() {
                let kx = x0 + i as f32 * (kw + 8.0);
                let (big, small) = a.words();
                let main = *a == Again::Normal;
                let pressed = cx.pressed(kx, ky, kw, 54.0);
                let fill = if main { if pressed { t::ACCENT.mix(Rgba::hex(0), 0.12) } else { t::ACCENT } } else if pressed { t::S3 } else { t::S2 };
                cx.c.rect(kx, ky, kw, 54.0, t::KEY_R, fill);
                let ink = if main { t::ON_ACCENT } else { t::INK };
                cx.c.text(big, kx + kw / 2.0, ky + 9.0, Text::new(ty::UI, ink).semi().center());
                cx.c.text(small, kx + kw / 2.0, ky + 29.0, Text::new(ty::SMALL, if main { t::ON_ACCENT.a(0.75) } else { t::INK3 }).center());
                if cx.tap(kx, ky, kw, 54.0) {
                    answer = Some(*a);
                }
            }
        }
        None => {
            let mid = cx.c.h * 0.38;
            let done = app.st.review.done;
            let title = if done > 0 { "Done for today" } else { "Nothing to review" };
            cx.c.text(title, WIDTH / 2.0, mid, Text::new(ty::HEADING, t::INK).semi().center());
            let next = app.shelf.highlights.iter().filter(|h| !h.retired && h.id != 0).map(|h| h.due_ms).min();
            let body = match (done, next) {
                (0, Some(n)) => format!("The next highlight comes back {}.", when(n, now_ms)),
                (0, None) => "Highlights you make come back here the next day.".to_string(),
                (d, Some(n)) => format!("{d} reviewed. The next one comes back {}.", when(n, now_ms)),
                (d, None) => format!("{d} reviewed."),
            };
            cx.c.para(&body, PAD * 2.0, mid + 34.0, WIDTH - PAD * 4.0, Text::new(ty::UI, t::INK2).center().line(1.45), 3);
            if kit::plain(&mut cx, WIDTH / 2.0 - 80.0, mid + 90.0, 160.0, 44.0, "Back to notes", None) {
                close = true;
            }
        }
    }
    if let (Some(a), Some(h)) = (answer, &current) {
        app.shelf.reviewed(h.id, a, now_ms);
        app.st.review.at += 1;
        app.st.review.done += 1;
    }
    if open && let Some(h) = &current {
        let (id, p) = (h.book, h.from);
        open_at(app, id, p);
        return true;
    }
    if close {
        app.pop();
    }
    false
}

/// Find: titles, authors and your notes, as you type.
pub fn draw_find(app: &mut App, dt: f32) -> bool {
    if app.fresh && app.st.find.query.is_empty() {
        app.focus.id = Some("find");
        app.focus.text.clear();
        app.focus.kind = kit::Kb::Text;
        app.host.keyboard(true, "", kit::Kb::Text);
    }
    let top_h = 64.0;
    let pg = app.page_begin(dt, top_h);
    let now = app.now_ns;
    let now_ms = shelf::now_ms();
    let q = if app.focus.is("find") { app.focus.text.clone() } else { app.st.find.query.clone() };
    let ql = q.trim().to_lowercase();
    let books: Vec<Book> = app.shelf.books.iter().filter(|b| !ql.is_empty() && (b.title.to_lowercase().contains(&ql) || b.author.to_lowercase().contains(&ql))).take(20).cloned().collect();
    let series: Vec<leaf_proto::Series> = app.library.data.iter().flatten().filter(|s| !ql.is_empty() && s.title.to_lowercase().contains(&ql)).take(20).cloned().collect();
    let notes: Vec<Highlight> = app.shelf.highlights.iter().filter(|h| ql.chars().count() >= 2 && (h.text.to_lowercase().contains(&ql) || h.note.to_lowercase().contains(&ql))).take(30).cloned().collect();
    let titles: HashMap<i64, String> = app.shelf.books.iter().map(|b| (b.id, b.title.clone())).collect();
    let mut open_book: Option<i64> = None;
    let mut open_series: Option<i64> = None;
    let mut open_note: Option<(i64, shelf::Place)> = None;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top + 6.0;
    let head = |c: &mut Canvas, y: f32, words: &str, n: usize| {
        c.text(words, x0, y, Text::new(ty::CAPTION, t::INK3).semi());
        c.text(&n.to_string(), x0 + w, y, Text::new(ty::CAPTION, t::INK3).right());
    };
    if ql.is_empty() {
        cx.c.text("Search your books' titles and authors, and your notes.", x0, y + 4.0, Text::new(ty::UI, t::INK3));
        y += 30.0;
    } else if books.is_empty() && series.is_empty() && notes.is_empty() {
        cx.c.text(&format!("Nothing matches “{}”", q.trim()), x0, y + 4.0, Text::new(ty::UI, t::INK2));
        y += 30.0;
    }
    if !books.is_empty() {
        head(cx.c, y, "Books", books.len());
        y += 26.0;
        for b in &books {
            if cx.pressed(0.0, y, WIDTH, 64.0) {
                cx.c.rect(0.0, y, WIDTH, 64.0, 0.0, t::WASH);
            }
            crate::book::cover(cx.c, &mut app.images, b, x0, y + 6.0, 36.0, 52.0, now);
            cx.c.text_fit(&b.title, x0 + 50.0, y + 13.0, w - 50.0, Text::new(ty::UI, t::INK).semi());
            cx.c.text_fit(&crate::library::book_line(b), x0 + 50.0, y + 35.0, w - 50.0, Text::new(ty::CAPTION, t::INK3));
            if cx.tap(0.0, y, WIDTH, 64.0) {
                open_book = Some(b.id);
            }
            y += 64.0;
        }
        y += 14.0;
    }
    if !series.is_empty() {
        head(cx.c, y, "Comics", series.len());
        y += 26.0;
        for s in &series {
            if cx.pressed(0.0, y, WIDTH, 48.0) {
                cx.c.rect(0.0, y, WIDTH, 48.0, 0.0, t::WASH);
            }
            cx.c.text_fit(&s.title, x0, y + 8.0, w, Text::new(ty::UI, t::INK).semi());
            cx.c.text_fit(&crate::logic::library_line(s), x0, y + 28.0, w, Text::new(ty::CAPTION, t::INK3));
            if cx.tap(0.0, y, WIDTH, 48.0) {
                open_series = Some(s.id);
            }
            y += 52.0;
        }
        y += 14.0;
    }
    if !notes.is_empty() {
        head(cx.c, y, "Your notes", notes.len());
        y += 28.0;
        for h in &notes {
            let hh = passage(cx.c, h, x0, y, w, 15.0, 3, now_ms, false);
            let title = titles.get(&h.book).cloned().unwrap_or_default();
            cx.c.text_fit(&title, x0 + 14.0, y + hh + 4.0, w - 14.0, Text::new(ty::CAPTION, t::INK3));
            let total = hh + 24.0;
            if cx.tap(0.0, y - 4.0, WIDTH, total + 8.0) {
                open_note = Some((h.book, h.from));
            }
            y += total + 16.0;
        }
    }
    let bottom = y;
    let back = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        let back = kit::icon_key(&mut cx, 6.0, top + 10.0, "arrow-left");
        let fx = 56.0;
        let fw = WIDTH - fx - PAD;
        cx.c.rect(fx, top + 10.0, fw, 44.0, t::KEY_R, t::S1);
        cx.c.icon("search", fx + 20.0, top + 32.0, 16.0, t::INK3);
        let mut v = app.st.find.query.clone();
        kit::inline(&mut cx, "find", fx + 40.0, top + 10.0, fw - 54.0, 44.0, "Titles, authors, notes", &mut v, kit::Kb::Text, Text::new(ty::BODY, t::INK));
        app.st.find.query = v;
        back
    };
    app.page_end(&pg, bottom);
    if back {
        app.pop();
    } else if let Some(id) = open_book {
        app.open_book(id);
    } else if let Some(id) = open_series {
        app.push(Route::Series(id));
    } else if let Some((id, p)) = open_note {
        open_at(app, id, p);
    }
    pg.moving
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_books_notes_export_as_markdown_in_book_order() {
        let b = Book {
            id: 1,
            kind: shelf::Kind::Epub,
            title: "Meditations".into(),
            author: "Marcus Aurelius".into(),
            file: String::new(),
            added_ms: 0,
            opened_ms: 0,
            place: shelf::Place::default(),
            progress: 0.0,
            cpm: 0.0,
            pages: 0,
            has_cover: false,
            chapter: String::new(),
            chars: 0,
        };
        let h = |id: i64, block: u32, chapter: &str, text: &str, colour: u8, note: &str| Highlight {
            id,
            book: 1,
            from: shelf::Place { section: 0, block, byte: 0 },
            to: shelf::Place { section: 0, block, byte: 4 },
            text: text.into(),
            colour,
            note: note.into(),
            chapter: chapter.into(),
            created_ms: 0,
            due_ms: 0,
            interval_d: 0,
            retired: false,
        };
        let hs = vec![h(2, 9, "Book II", "Begin the morning.", 1, ""), h(1, 3, "Book I", "From my grandfather.", 0, "Gentleness"), h(3, 1, "Other", "Not this book", 0, "")];
        let mut hs = hs;
        hs[2].book = 2;
        let md = markdown(&b, &hs);
        assert_eq!(md, "# Meditations\n\n*Marcus Aurelius*\n\n## Book I\n\n> From my grandfather.\n\nKey idea: Gentleness\n\n## Book II\n\n> Begin the morning.\n\nQuestion\n");
    }

    #[test]
    fn days_read_from_today() {
        let now = 1_800_000_000_000;
        assert_eq!(day_words(now, now), "Today");
        assert_eq!(day_words(now - shelf::DAY_MS, now), "Yesterday");
        assert!(WEEKDAYS.contains(&day_words(now - 3 * shelf::DAY_MS, now).as_str()));
        assert!(day_words(now - 40 * shelf::DAY_MS, now).contains(' '));
    }
}
