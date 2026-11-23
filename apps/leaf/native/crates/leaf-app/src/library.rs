//! Library: books on the phone and the series the server follows, in one grid of covers, three
//! across (design 42, "1 · Library": B's grid under A's hero). The book you read last leads as a
//! "continue" hero; chips filter books from comics when there are both. Each cover carries how far
//! it is read on its bottom edge. Series are read again from the server each time the page shows;
//! the copy on disk shows meanwhile, and when the server can't be reached ("Offline").

use leaf_proto::Series;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH, Weight};
use crate::images::CoverOf;
use crate::kit;
use crate::logic;
use crate::net::Job;
use crate::shelf::{self, Book, ImportState};
use crate::theme::{self as t, COVER_R, PAD, ty};

const COLS: usize = 3;
const GAP: f32 = 12.0;
/// The fixed top: the title and its keys.
const TOP: f32 = 64.0;

#[derive(Default)]
pub struct State {
    /// 0 all, 1 books, 2 comics.
    pub filter: u8,
    pub add_menu: bool,
}

/// A cover's width in the grid.
pub fn cover_w() -> f32 {
    (WIDTH - PAD * 2.0 - GAP * (COLS as f32 - 1.0)) / COLS as f32
}

enum Cell<'a> {
    Book(&'a Book),
    Series(&'a Series),
}

/// How far into a book, in words for under its cover.
pub fn book_line(b: &Book) -> String {
    if b.kind == shelf::Kind::Pdf && b.progress <= 0.0 {
        return if b.pages > 0 { format!("PDF · {} pages", b.pages) } else { "PDF".into() };
    }
    if b.progress >= 0.995 {
        return "Finished".into();
    }
    if b.opened_ms == 0 || b.progress <= 0.0 {
        return if b.author.is_empty() { "Not started".into() } else { b.author.clone() };
    }
    let pct = (b.progress * 100.0).round().max(1.0) as i64;
    match hours_left(b) {
        Some(h) => format!("{pct}% · {h} left"),
        None => format!("{pct}%"),
    }
}

/// Time left in the whole book at the reader's pace, once it has been measured.
fn hours_left(b: &Book) -> Option<String> {
    if b.chars <= 0 || b.cpm <= 0.0 {
        return None;
    }
    let mins = ((1.0 - b.progress) * b.chars as f32 / b.cpm).round() as i64;
    Some(if mins < 60 { format!("{} min", mins.max(1)) } else { format!("{} h {} min", mins / 60, mins % 60) })
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    if app.fresh && !app.library.loading && app.server.is_some() {
        app.ask(Job::Library);
    }
    let pg = app.page_begin(dt, TOP);
    let now = app.now_ns;
    let series: Vec<Series> = app.library.data.clone().unwrap_or_default();
    let books: Vec<Book> = app.shelf.books.clone();
    let (loading, error, offline) = (app.library.loading, app.library.error.clone(), app.library.offline());
    let filter = app.st.library.filter;
    let importing = app.shelf.importing();
    let mut open_series: Option<i64> = None;
    let mut open_book: Option<i64> = None;
    let mut add = false;
    let mut retry = false;
    let mut set_filter = None;
    let mut to_imports = false;
    let mut to_connect = false;
    let mut to_search = false;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - PAD * 2.0;
    let mut y = pg.top + 4.0;
    let cw = cover_w();
    let ch = cw * 10.0 / 7.0;
    if importing > 0 {
        let words = if importing == 1 { "Adding 1 book".to_string() } else { format!("Adding {importing} books") };
        let tt = Text::new(ty::UI, t::INK2);
        cx.c.circle(x0 + 5.0, y + 10.0, 4.0, t::ACCENT);
        cx.c.text(&words, x0 + 18.0, y + (20.0 - tt.lh()) / 2.0, tt);
        if cx.tap(0.0, y - 8.0, WIDTH, 36.0) {
            to_imports = true;
        }
        y += 34.0;
    }
    let empty = books.is_empty() && series.is_empty();
    if empty && app.shelf.loaded && !(loading && app.library.data.is_none()) {
        // Honest at zero: the shelf's own shape, empty.
        y += 40.0;
        let (sw, sh) = (cw * 0.78, cw * 0.78 * 10.0 / 7.0);
        for i in 0..3 {
            let sx = WIDTH / 2.0 - sw * 1.5 - 10.0 + i as f32 * (sw + 10.0);
            let sy = y + if i == 1 { 0.0 } else { 14.0 };
            cx.c.outline(sx, sy, sw, sh, COVER_R, 1.0, t::INK3.a(0.5));
        }
        y += sh + 40.0;
        cx.c.text("No books yet", WIDTH / 2.0, y, Text::new(ty::HEADING, t::INK).semi().center());
        y += 32.0;
        y += cx.c.para("Add EPUB, PDF or text files from your phone. They're copied into Leaf and stay on the phone.", x0 + 24.0, y, w - 48.0, Text::new(ty::UI, t::INK2).center().line(1.45), 4) + 24.0;
        if kit::primary(&mut cx, x0 + 24.0, y, w - 48.0, 50.0, "Add books", Some("plus")) {
            add = true;
        }
        y += 50.0;
        if app.server.is_none() {
            y += 12.0;
            if kit::plain(&mut cx, x0 + 24.0, y, w - 48.0, 46.0, "Connect your comics server", None) {
                to_connect = true;
            }
            y += 46.0;
        } else if let Some(e) = &error {
            y += 20.0;
            cx.c.para(e, x0 + 24.0, y, w - 48.0, Text::new(ty::CAPTION, t::INK3).center(), 3);
            y += 40.0;
        } else if let Some(s) = &app.server {
            // Connected, nothing followed yet: say so, and where comics come from.
            y += 12.0;
            if kit::plain(&mut cx, x0 + 24.0, y, w - 48.0, 46.0, "Find comics to follow", Some("search")) {
                to_search = true;
            }
            y += 46.0 + 16.0;
            let words = format!("Comics come from {}", crate::logic::server_label(&s.base));
            cx.c.para(&words, x0 + 24.0, y, w - 48.0, Text::new(ty::CAPTION, t::INK3).center(), 2);
            y += 24.0;
        }
    } else {
        // The book read last leads.
        let last = books.iter().filter(|b| b.opened_ms > 0 && b.progress < 0.995).max_by_key(|b| b.opened_ms);
        if let Some(b) = last
            && filter != 2
        {
            let hh = 162.0;
            if cx.pressed(x0, y, w, hh) {
                cx.c.rect(x0, y, w, hh, 14.0, t::S2);
            } else {
                cx.c.rect(x0, y, w, hh, 14.0, t::S1);
            }
            let (bw, bh) = (98.0, 140.0);
            crate::book::cover(cx.c, &mut app.images, b, x0 + 11.0, y + 11.0, bw, bh, now);
            let tx = x0 + 11.0 + bw + 16.0;
            let tw = w - (tx - x0) - 14.0;
            cx.c.text("CONTINUE READING", tx, y + 14.0, Text::new(ty::SMALL, t::ACCENT).semi());
            let tt = Text::new(ty::SECTION, t::INK).face(Weight::SerifBold).line(1.25);
            let th = cx.c.para(&b.title, tx, y + 34.0, tw, tt, 2);
            let sub = if b.chapter.is_empty() { b.author.clone() } else { b.chapter.clone() };
            cx.c.text_fit(&sub, tx, y + 38.0 + th, tw, Text::new(ty::CAPTION, t::INK2));
            let ly = y + hh - 58.0;
            cx.c.rect(tx, ly, tw, 3.0, 1.5, t::INK.a(0.12));
            cx.c.rect(tx, ly, (tw * b.progress).max(3.0), 3.0, 1.5, t::ACCENT);
            cx.c.text_fit(&book_line(b), tx, ly + 10.0, tw, Text::new(ty::CAPTION, t::INK3));
            if cx.tap(x0, y, w, hh) {
                open_book = Some(b.id);
            }
            y += hh + 20.0;
        }
        // Chips when there are both kinds.
        if !books.is_empty() && !series.is_empty() {
            let mut cx_ = x0;
            for (i, words) in [format!("All {}", books.len() + series.len()), format!("Books {}", books.len()), format!("Comics {}", series.len())].iter().enumerate() {
                let (cw_, tapped) = kit::chip(&mut cx, cx_, y, words, filter == i as u8, None);
                if tapped {
                    set_filter = Some(i as u8);
                }
                cx_ += cw_ + 8.0;
            }
            y += 48.0;
        }
        // Newest first: a book by when it was last opened or added, a series by its newest chapter.
        let mut cells: Vec<(i64, Cell)> = Vec::new();
        if filter != 2 {
            cells.extend(books.iter().map(|b| (b.opened_ms.max(b.added_ms), Cell::Book(b))));
        }
        if filter != 1 {
            cells.extend(series.iter().map(|s| (s.updated_at, Cell::Series(s))));
        }
        cells.sort_by_key(|a| std::cmp::Reverse(a.0));
        let title_t = Text::new(13.5, t::INK).medium().line(1.3);
        let line_t = Text::new(ty::CAPTION, t::INK3);
        let cell_h = ch + 8.0 + title_t.lh() * 2.0 + 2.0 + line_t.lh() + 18.0;
        let view_top = -cx.c.dy - 40.0;
        let view_bottom = -cx.c.dy + cx.c.h + 40.0;
        if series.is_empty() && loading && app.library.data.is_none() && books.is_empty() {
            for i in 0..6 {
                let (col, row) = (i % COLS, i / COLS);
                cx.c.rect(x0 + col as f32 * (cw + GAP), y + row as f32 * cell_h, cw, ch, COVER_R, t::S1);
            }
        }
        for (i, (_, cell)) in cells.iter().enumerate() {
            let (col, row) = (i % COLS, i / COLS);
            let x = x0 + col as f32 * (cw + GAP);
            let cy = y + row as f32 * cell_h;
            if cy + cell_h < view_top || cy > view_bottom {
                continue;
            }
            let ty_ = cy + ch + 8.0;
            let ly = ty_ + title_t.lh() * 2.0 + 2.0;
            let progress_line = |c: &mut crate::draw::Canvas, f: f32| {
                if f > 0.0 {
                    let (lx, lyy, lw) = (x + 8.0, cy + ch - 9.0, cw - 16.0);
                    c.rect(lx, lyy, lw, 3.0, 1.5, Rgba::hex(0x000000).a(0.55));
                    c.rect(lx, lyy, (lw * f).max(3.0), 3.0, 1.5, t::ACCENT);
                }
            };
            match cell {
                Cell::Book(b) => {
                    crate::book::cover(cx.c, &mut app.images, b, x, cy, cw, ch, now);
                    if cx.pressed(x, cy, cw, cell_h - 10.0) {
                        cx.c.rect(x, cy, cw, ch, COVER_R, t::WASH);
                    }
                    progress_line(cx.c, b.progress);
                    cx.c.para(&b.title, x, ty_, cw, title_t, 2);
                    cx.c.text_fit(&book_line(b), x, ly, cw, line_t);
                    if cx.tap(x, cy, cw, cell_h - 10.0) {
                        open_book = Some(b.id);
                    }
                }
                Cell::Series(s) => {
                    let of = CoverOf::Series(s.id);
                    cx.c.rect(x, cy, cw, ch, COVER_R, t::S1);
                    if app.images.cover(cx.c, of, now) {
                        cx.c.image_cover(&of.key(), x, cy, cw, ch, COVER_R, Rgba::WHITE);
                    }
                    if cx.pressed(x, cy, cw, cell_h - 10.0) {
                        cx.c.rect(x, cy, cw, ch, COVER_R, t::WASH);
                    }
                    progress_line(cx.c, logic::read_fraction(s));
                    if s.note.is_some() {
                        cx.c.circle(x + cw - 12.0, cy + 12.0, 6.0, t::BG);
                        cx.c.circle(x + cw - 12.0, cy + 12.0, 4.0, t::WARN);
                    }
                    cx.c.para(&s.title, x, ty_, cw, title_t, 2);
                    match &s.note {
                        Some(n) => cx.c.text_fit(n, x, ly, cw, line_t.color(t::WARN)),
                        None => {
                            let lines = logic::library_lines(s);
                            let fits = lines.iter().find(|l| cx.c.measure(l, line_t) <= cw).unwrap_or(&lines[lines.len() - 1]).clone();
                            cx.c.text_fit(&fits, x, ly, cw, line_t)
                        }
                    };
                    if cx.tap(x, cy, cw, cell_h - 10.0) {
                        open_series = Some(s.id);
                    }
                }
            }
        }
        y += (cells.len().div_ceil(COLS)) as f32 * cell_h;
        if let Some(e) = &error
            && !offline
            && filter != 1
        {
            cx.c.text_fit(e, x0, y, w, Text::new(ty::CAPTION, t::DANGER));
            y += 24.0;
        }
        if let Some(e) = &app.shelf.error {
            cx.c.text_fit(e, x0, y, w, Text::new(ty::CAPTION, t::DANGER));
            y += 24.0;
        }
    }
    let bottom = y + kit::TABS_H;
    // The fixed top: the title, Offline when the server can't be reached, Find, Add and Settings.
    let mut find = false;
    let mut settings = false;
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + TOP, 0.0, t::BG);
        let tt = Text::new(ty::TITLE, t::INK).brand();
        let tw = cx.c.text("Library", PAD, top + 14.0, tt);
        if offline {
            let words = "Offline";
            let ot = Text::new(ty::CAPTION, t::INK2).medium();
            let ow = cx.c.measure(words, ot) + 18.0;
            let (ox, oy) = (PAD + tw + 10.0, top + 14.0 + (tt.lh() - 22.0) / 2.0);
            cx.c.rect(ox, oy, ow, 22.0, 11.0, t::S2);
            cx.c.text(words, ox + 9.0, oy + (22.0 - ot.lh()) / 2.0, ot);
            if cx.ui.tapped(ox - 6.0, oy - 10.0, ow + 12.0, 42.0) {
                retry = true;
            }
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0 * 3.0, top + 10.0, "search") {
            find = true;
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0 * 2.0, top + 10.0, "plus") {
            app.st.library.add_menu = true;
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 10.0, "gear") {
            settings = true;
        }
    }
    let tab = kit::tabs(&mut cx, 0);
    let mut follow = false;
    if app.st.library.add_menu {
        let top = cx.c.inset_top + TOP - 4.0;
        match kit::menu(&mut cx, top, &["Books from this phone", "Follow a series"]) {
            Some(0) => {
                add = true;
                app.st.library.add_menu = false;
            }
            Some(1) => {
                follow = true;
                app.st.library.add_menu = false;
            }
            Some(_) => app.st.library.add_menu = false,
            None => {}
        }
    }
    app.page_end(&pg, bottom);
    if let Some(f) = set_filter {
        app.st.library.filter = f;
    }
    if retry {
        app.images.retry_failed();
        app.ask(Job::Library);
    }
    if add {
        app.host.pick_books();
    }
    if let Some(id) = open_book {
        app.open_book(id);
    } else if let Some(id) = open_series {
        app.push(Route::Series(id));
    } else if follow {
        app.push(if app.server.is_some() { Route::Search } else { Route::Connect });
    } else if to_search {
        app.push(Route::Search);
    } else if find {
        app.push(Route::Find);
    } else if settings || to_connect {
        app.push(Route::Connect);
    } else if to_imports {
        app.push(Route::Imports);
    } else if let Some(i) = tab {
        app.reset_to([Route::Library, Route::Notes, Route::Stats][i].clone());
    }
    pg.moving || app.library.loading && !crate::wake::has() || app.shelf.busy() && !crate::wake::has()
}

/// The files being brought in: each with what became of it, in words.
pub fn draw_imports(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt, 60.0);
    let imports = app.shelf.imports.clone();
    let left = app.shelf.importing();
    let mut done = false;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top + 4.0;
    let total = imports.len();
    if total > 0 {
        let f = (total - left) as f32 / total as f32;
        cx.c.rect(x0, y, w - 60.0, 3.0, 1.5, t::S3);
        cx.c.rect(x0, y, ((w - 60.0) * f).max(3.0), 3.0, 1.5, t::ACCENT);
        cx.c.text(&format!("{} of {total}", total - left), x0 + w, y - 7.0, Text::new(ty::CAPTION, t::INK3).right());
        y += 24.0;
    } else {
        cx.c.text("Nothing is being added.", x0, y, Text::new(ty::UI, t::INK2));
        y += 30.0;
    }
    for im in &imports {
        let (title, words, ink, glyph): (String, String, Rgba, Option<&'static str>) = match &im.state {
            ImportState::Waiting => (im.name.clone(), "Waiting".into(), t::INK3, None),
            ImportState::Working => (im.name.clone(), "Reading the file".into(), t::INK3, None),
            ImportState::Added(title) => (title.clone(), format!("Added from {}", im.name), t::INK3, Some("check")),
            ImportState::Already(title) => (title.clone(), "Already in your library".into(), t::INK3, Some("check")),
            ImportState::Failed(why) => (im.name.clone(), why.clone(), t::WARN, Some("flag")),
        };
        let rh = 64.0;
        cx.c.rect(x0, y + 8.0, 34.0, 48.0, 4.0, t::S2);
        cx.c.icon("file-text", x0 + 17.0, y + 32.0, 16.0, t::INK3);
        cx.c.text_fit(&title, x0 + 48.0, y + 12.0, w - 48.0 - 30.0, Text::new(ty::UI, t::INK).semi());
        cx.c.para(&words, x0 + 48.0, y + 32.0, w - 48.0 - 30.0, Text::new(ty::CAPTION, ink), 2);
        match glyph {
            Some(g) => cx.c.icon(g, x0 + w - 10.0, y + 30.0, 17.0, if g == "check" { t::ACCENT } else { t::WARN }),
            None => cx.c.arc(x0 + w - 10.0, y + 30.0, 8.0, 2.0, 0.0, 0.7, t::INK3, false),
        }
        y += rh + 6.0;
    }
    y += 10.0;
    if left == 0 && total > 0 {
        if kit::primary(&mut cx, x0, y, w, 48.0, "Done", None) {
            done = true;
        }
        y += 48.0;
    }
    let bottom = y;
    let back = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 60.0, 0.0, t::BG);
        let title = if left > 0 { format!("Adding {} {}", left, if left == 1 { "book" } else { "books" }) } else { "Added to your library".to_string() };
        kit::top_bar(&mut cx, top + 8.0, &title, &[]).0
    };
    app.page_end(&pg, bottom);
    if back || done {
        app.pop();
    }
    pg.moving || left > 0 && !crate::wake::has()
}
