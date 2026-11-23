//! Library: the followed series as a grid of covers, three across (design 42, "1 · Library", B).
//! Each cover carries how far it is read on its bottom edge; under it, the title and where you
//! are. Read again from the server each time it shows; the copy on disk shows meanwhile, and
//! when the server can't be reached ("Offline").

use leaf_proto::Series;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::images::CoverOf;
use crate::kit;
use crate::logic;
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};

const COLS: usize = 3;
const GAP: f32 = 12.0;
/// The fixed top: the title and its keys.
const TOP: f32 = 64.0;

/// A cover's width in the grid.
pub fn cover_w() -> f32 {
    (WIDTH - PAD * 2.0 - GAP * (COLS as f32 - 1.0)) / COLS as f32
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    if app.fresh && !app.library.loading {
        app.ask(Job::Library);
    }
    let pg = app.page_begin(dt, TOP);
    let now = app.now_ns;
    let list: Vec<Series> = app.library.data.clone().unwrap_or_default();
    let (loading, error, offline) = (app.library.loading, app.library.error.clone(), app.library.offline());
    let mut open: Option<i64> = None;
    let mut find = false;
    let mut retry = false;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - PAD * 2.0;
    let mut y = pg.top + 6.0;
    let cw = cover_w();
    let ch = cw * 10.0 / 7.0;
    if list.is_empty() && app.library.data.is_none() {
        if loading {
            // The first answer is on its way: the grid's shape, quiet.
            for i in 0..6 {
                let (col, row) = (i % COLS, i / COLS);
                let x = x0 + col as f32 * (cw + GAP);
                let cy = y + row as f32 * (ch + 72.0);
                cx.c.rect(x, cy, cw, ch, COVER_R, t::S1);
            }
            y += 2.0 * (ch + 72.0);
        } else if let Some(e) = &error {
            y += 120.0;
            let (h, r) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, e, Some("Check that it's on, and its address in Settings."), Some("Retry"));
            retry = r;
            y += h;
        }
    } else if list.is_empty() {
        // Honest at zero: the shelf's own shape, empty.
        y += 40.0;
        let (sw, sh) = (cw * 0.78, cw * 0.78 * 10.0 / 7.0);
        for i in 0..3 {
            let sx = WIDTH / 2.0 - sw * 1.5 - 10.0 + i as f32 * (sw + 10.0);
            let sy = y + if i == 1 { 0.0 } else { 14.0 };
            cx.c.outline(sx, sy, sw, sh, COVER_R, 1.0, t::INK3.a(0.5));
        }
        y += sh + 40.0;
        cx.c.text("Nothing followed yet", WIDTH / 2.0, y, Text::new(ty::HEADING, t::INK).semi().center());
        y += 32.0;
        y += cx.c.para(
            "Search your server's sources for a series and follow it. New chapters arrive here on their own.",
            x0 + 24.0,
            y,
            w - 48.0,
            Text::new(ty::UI, t::INK2).center().line(1.45),
            4,
        ) + 24.0;
        if kit::primary(&mut cx, x0 + 24.0, y, w - 48.0, 50.0, "Find a series", Some("search")) {
            find = true;
        }
        y += 50.0;
    } else {
        let title_t = Text::new(13.5, t::INK).medium().line(1.3);
        let line_t = Text::new(ty::CAPTION, t::INK3);
        let cell_h = ch + 8.0 + title_t.lh() * 2.0 + 2.0 + line_t.lh() + 18.0;
        let view_top = -cx.c.dy - 40.0;
        let view_bottom = -cx.c.dy + cx.c.h + 40.0;
        // Newest chapter first, as a reading list wants it.
        let mut order: Vec<&Series> = list.iter().collect();
        order.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.title.cmp(&b.title)));
        for (i, s) in order.iter().enumerate() {
            let (col, row) = (i % COLS, i / COLS);
            let x = x0 + col as f32 * (cw + GAP);
            let cy = y + row as f32 * cell_h;
            if cy + cell_h < view_top || cy > view_bottom {
                continue;
            }
            let of = CoverOf::Series(s.id);
            cx.c.rect(x, cy, cw, ch, COVER_R, t::S1);
            if app.images.cover(cx.c, of, now) {
                cx.c.image_cover(&of.key(), x, cy, cw, ch, COVER_R, crate::draw::Rgba::WHITE);
            }
            if cx.pressed(x, cy, cw, cell_h - 10.0) {
                cx.c.rect(x, cy, cw, ch, COVER_R, t::WASH);
            }
            // How far it is read, on the cover's bottom edge.
            let f = logic::read_fraction(s);
            if f > 0.0 {
                let (lx, ly, lw) = (x + 8.0, cy + ch - 9.0, cw - 16.0);
                cx.c.rect(lx, ly, lw, 3.0, 1.5, crate::draw::Rgba::hex(0x000000).a(0.55));
                cx.c.rect(lx, ly, (lw * f).max(3.0), 3.0, 1.5, t::ACCENT);
            }
            if s.note.is_some() {
                cx.c.circle(x + cw - 12.0, cy + 12.0, 6.0, t::BG);
                cx.c.circle(x + cw - 12.0, cy + 12.0, 4.0, t::WARN);
            }
            let ty_ = cy + ch + 8.0;
            cx.c.para(&s.title, x, ty_, cw, title_t, 2);
            let ly = ty_ + title_t.lh() * 2.0 + 2.0;
            match &s.note {
                Some(n) => cx.c.text_fit(n, x, ly, cw, line_t.color(t::WARN)),
                None => {
                    let lines = logic::library_lines(s);
                    let fits = lines.iter().find(|l| cx.c.measure(l, line_t) <= cw).unwrap_or(&lines[lines.len() - 1]).clone();
                    cx.c.text_fit(&fits, x, ly, cw, line_t)
                }
            };
            if cx.tap(x, cy, cw, cell_h - 10.0) {
                open = Some(s.id);
            }
        }
        y += (order.len().div_ceil(COLS)) as f32 * cell_h;
        if let Some(e) = &error
            && !offline
        {
            cx.c.text_fit(e, x0, y, w, Text::new(ty::CAPTION, t::DANGER));
            y += 24.0;
        }
    }
    let bottom = y;
    // The fixed top: the title, Offline when the server can't be reached, Search and Settings.
    let mut search = false;
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
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0 - 44.0, top + 10.0, "search") {
            search = true;
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 10.0, "gear") {
            settings = true;
        }
    }
    app.page_end(&pg, bottom);
    if retry {
        app.images.retry_failed();
        app.ask(Job::Library);
    }
    if let Some(id) = open {
        app.push(Route::Series(id));
    } else if search || find {
        app.push(Route::Search);
    } else if settings {
        app.push(Route::Connect);
    }
    pg.moving || app.library.loading && !crate::wake::has()
}
