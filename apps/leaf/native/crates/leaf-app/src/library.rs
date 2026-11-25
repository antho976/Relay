//! Books brought in: the words a book is shown with across the app (how far into it, the time
//! left at your pace), and the page that follows files as they are imported. The library page
//! itself became Home and My space (design 42, pages 12 and 14).

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit;
use crate::shelf::{self, Book, ImportState};
use crate::theme::{self as t, PAD, ty};

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
pub fn hours_left(b: &Book) -> Option<String> {
    if b.chars <= 0 || b.cpm <= 0.0 {
        return None;
    }
    let mins = ((1.0 - b.progress) * b.chars as f32 / b.cpm).round() as i64;
    Some(if mins < 60 { format!("{} min", mins.max(1)) } else { format!("{} h {} min", mins / 60, mins % 60) })
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
        kit::fixed_top(&mut cx, 60.0);
        let title = if left > 0 { format!("Adding {} {}", left, if left == 1 { "book" } else { "books" }) } else { "Added to your library".to_string() };
        kit::top_bar(&mut cx, top + 8.0, &title, &[]).0
    };
    app.page_end(&pg, bottom);
    if back || done {
        app.pop();
    }
    pg.moving || left > 0 && !crate::wake::has()
}
