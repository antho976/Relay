//! My space (design 42, page 14): what you read, in three tabs.
//!
//! - **Recent** (artboard A): every series and book, last read first, each with the chapter you
//!   are on out of how many, NEW when chapters came since, when you last read it, and Continue or
//!   Start.
//! - **Following**: webtoon's Subscribed list, the owner's reference. Every series in My space with
//!   its bell, which is the follow: on (filled, the accent), new chapters are announced; off, they
//!   are not. "New chapter" on a cover when some came since you read it. Sorted by Recently read,
//!   Recently updated or Recently followed (when it was added: the server keeps no time for the
//!   bell).
//! - **Downloads**: per series, the chapters the server keeps and the ones on this phone, with
//!   the room they take, and removing them.
//!
//! The + menu adds books from the phone or a series from Search; ⋯ holds Highlights and Reading
//! time, which are no longer tabs. While a menu is open the page under it gets no tap (the + menu's
//! tap went through to the cover under it, 2026-11-22).

use std::collections::HashMap;

use leaf_proto::Series;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::home::new_tag;
use crate::images::CoverOf;
use crate::kit;
use crate::logic;
use crate::net::Job;
use crate::shelf::{self, Book};
use crate::theme::{self as t, PAD, ty};

/// The fixed top: the title and its keys, then the tabs.
const TOP: f32 = 104.0;
const TABS: [&str; 3] = ["RECENT", "FOLLOWING", "DOWNLOADS"];

#[derive(Default)]
pub struct State {
    pub tab: u8,
    pub add_menu: bool,
    pub more_menu: bool,
    pub sort_menu: bool,
    /// Where the sort menu drops from (screen units).
    sort_top: f32,
}

impl State {
    /// Close a menu, if one is open.
    pub fn close_over(&mut self) -> bool {
        let open = self.add_menu || self.more_menu || self.sort_menu;
        self.add_menu = false;
        self.more_menu = false;
        self.sort_menu = false;
        open
    }
}

/// "61 MB", "1.3 GB".
pub fn size_words(bytes: u64) -> String {
    let mb = bytes as f64 / 1e6;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0)
    } else if mb >= 1.0 {
        format!("{} MB", mb.round() as u64)
    } else if bytes > 0 {
        "under 1 MB".into()
    } else {
        "0 MB".into()
    }
}

enum Act {
    Series(i64),
    Continue(i64),
    Book(i64),
    Bell(i64, bool),
    Remove(i64),
}

#[allow(clippy::too_many_arguments)]
fn cover(cx: &mut kit::Ctx, im: &mut crate::images::Images, s: &Series, x: f32, y: f32, w: f32, h: f32, r: f32, now: u64) {
    let of = CoverOf::Series(s.id);
    cx.c.rect(x, y, w, h, r, t::S1);
    if im.cover(cx.c, of, now) {
        cx.c.image_cover(&of.key(), x, y, w, h, r, Rgba::WHITE);
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let tab = app.st.space.tab.min(2);
    if app.fresh {
        if !app.library.loading && app.server.is_some() {
            app.ask(Job::Library);
        }
        app.images.scan_cache();
    }
    let series: Vec<Series> = app.library.data.clone().unwrap_or_default();
    let books: Vec<Book> = app.shelf.books.clone();
    // Downloads need every series' chapters; Recent and Following count new ones with them.
    for s in &series {
        if !app.series.contains_key(&s.id) {
            app.ask(Job::Series(s.id));
        }
    }
    let new_of: HashMap<i64, u32> = series.iter().map(|s| (s.id, logic::new_chapters(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())))).collect();
    let now = app.now_ns;
    let now_ms = shelf::now_ms();
    let loading = app.library.loading && app.library.data.is_none();
    let offline = app.library.offline();
    let error = app.library.error.clone().filter(|_| !offline);
    let sort = app.following_sort;
    let covered = app.st.space.add_menu || app.st.space.more_menu || app.st.space.sort_menu;
    let pg = app.page_begin(dt, TOP);
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    let st = &mut app.st.space;
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top;
    let mut act: Option<Act> = None;
    let mut to_search = false;
    let view_top = -cx.c.dy - 140.0;
    let view_bottom = -cx.c.dy + cx.c.h + 140.0;
    match tab {
        0 => {
            let with_new = series.iter().filter(|s| new_of[&s.id] > 0).count();
            let mut words = vec![if series.len() == 1 { "1 series".to_string() } else { format!("{} series", series.len()) }];
            if with_new > 0 {
                words.push(format!("{with_new} with new chapters"));
            }
            if !books.is_empty() {
                words.push(if books.len() == 1 { "1 book".into() } else { format!("{} books", books.len()) });
            }
            y += 12.0;
            cx.c.text_fit(&words.join(" · ").to_uppercase(), x0, y, w, Text::new(ty::SMALL, t::INK3).semi());
            y += 30.0;
            // Last read first; never read after, newest added first.
            let mut rows: Vec<(i64, i64, Result<&Series, &Book>)> = series.iter().map(|s| (s.last_read_at, s.added_at, Ok(s))).collect();
            rows.extend(books.iter().map(|b| (b.opened_ms, b.added_ms, Err(b))));
            rows.sort_by_key(|r| (std::cmp::Reverse(r.0), std::cmp::Reverse(r.1)));
            if rows.is_empty() && !loading {
                y += 40.0;
                let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Nothing here yet", Some("Books you add and series you add from Search show here, last read first."), None);
                y += h + 20.0;
                if kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Find a series", Some("search")) {
                    to_search = true;
                }
                y += 46.0;
            }
            let rh = 104.0;
            for (_, _, r) in rows {
                if y + rh < view_top || y > view_bottom {
                    y += rh;
                    continue;
                }
                if cx.pressed(0.0, y, WIDTH, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                let cw = 96.0;
                let tx = cw + 14.0;
                let (title, line, new, last, key, f, row_act, key_act) = match r {
                    Ok(s) => {
                        cover(&mut cx, &mut app.images, s, 0.0, y, cw, rh, 0.0, now);
                        let line = match (logic::at_chapter(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())), s.latest) {
                            (Some(n), Some(l)) => format!("Chapter {} of {}", logic::num(n), logic::num(l)),
                            (Some(n), None) => format!("Chapter {}", logic::num(n)),
                            (None, _) if s.chapters == 0 => "No chapters yet".into(),
                            (None, _) => "Chapter 1".into(),
                        };
                        let last = if s.last_read_at > 0 { format!("Last read {}", logic::ago(s.last_read_at, now_ms)) } else { "Not started".into() };
                        let key = if s.last_read_at > 0 { "Continue" } else { "Start" };
                        (s.title.as_str(), line, new_of[&s.id], last, key, logic::read_fraction(s), Act::Series(s.id), Act::Continue(s.id))
                    }
                    Err(b) => {
                        crate::book::cover(cx.c, &mut app.images, b, 0.0, y, cw, rh, now);
                        let last = if b.opened_ms > 0 { format!("Last read {}", logic::ago(b.opened_ms, now_ms)) } else { "Not started".into() };
                        let key = if b.progress > 0.0 { "Continue" } else { "Start" };
                        (b.title.as_str(), crate::library::book_line(b), 0, last, key, b.progress, Act::Book(b.id), Act::Book(b.id))
                    }
                };
                if f > 0.0 {
                    cx.c.rect(0.0, y + rh - 3.0, cw * f.min(1.0), 3.0, 0.0, t::ACCENT);
                }
                let kt = Text::new(ty::UI, t::INK).semi();
                let kw = cx.c.measure(key, kt) + 28.0;
                let (kx, ky) = (WIDTH - PAD - kw, y + (rh - 36.0) / 2.0);
                if cx.pressed(kx, ky, kw, 36.0) {
                    cx.c.rect(kx, ky, kw, 36.0, 8.0, t::WASH);
                }
                cx.c.outline(kx, ky, kw, 36.0, 8.0, 1.0, t::INK.a(0.28));
                cx.c.text(key, kx + 14.0, ky + (36.0 - kt.lh()) / 2.0, kt);
                let tw = kx - 12.0 - tx;
                let title_t = Text::new(16.5, t::INK).semi();
                let line_t = Text::new(ty::UI, t::INK);
                let last_t = Text::new(12.5, t::INK3);
                let block = title_t.lh() + 6.0 + 18.0 + 6.0 + last_t.lh();
                let ty0 = y + (rh - block) / 2.0;
                cx.c.text_fit(title, tx, ty0, tw, title_t);
                let ly = ty0 + title_t.lh() + 6.0;
                // NEW beside the chapter, the chapter said shorter when both don't fit.
                let tag = format!("{new} NEW");
                let tag_w = cx.c.measure(&tag, Text::new(10.5, t::ON_ACCENT).semi()) + 18.0;
                let room = if new > 0 { tw - tag_w } else { tw };
                let line = if cx.c.measure(&line, line_t) > room { line.replacen("Chapter ", "Ch. ", 1) } else { line };
                let lw = cx.c.text_fit(&line, tx, ly + (18.0 - line_t.lh()) / 2.0, room, line_t);
                if new > 0 {
                    new_tag(cx.c, tx + lw + 8.0, ly, &tag);
                }
                cx.c.text_fit(&last, tx, ly + 18.0 + 6.0, tw, last_t);
                if cx.tap(kx - 6.0, ky - 8.0, kw + 12.0, 52.0) {
                    act = Some(key_act);
                } else if cx.tap(0.0, y, WIDTH, rh) {
                    act = Some(row_act);
                }
                y += rh;
            }
        }
        1 => {
            y += 10.0;
            let ht = Text::new(ty::UI, t::INK2);
            let head_h = 40.0;
            cx.c.text(&if series.len() == 1 { "1 series".to_string() } else { format!("{} series", series.len()) }, x0, y + (head_h - ht.lh()) / 2.0, ht);
            let sw = logic::FOLLOWING_SORTS[sort as usize];
            let stt = Text::new(ty::UI, t::INK).medium().right();
            let sww = cx.c.measure(sw, stt);
            cx.c.text(sw, WIDTH - PAD - 22.0, y + (head_h - stt.lh()) / 2.0, stt);
            cx.c.icon("chevron-down", WIDTH - PAD - 8.0, y + head_h / 2.0, 15.0, t::INK2);
            if cx.tap(WIDTH - PAD - 30.0 - sww, y, sww + 30.0 + PAD, head_h) {
                st.sort_menu = true;
                st.sort_top = y + head_h + cx.c.dy;
            }
            y += head_h + 4.0;
            let mut list = series.clone();
            logic::sort_following(&mut list, sort);
            if list.is_empty() && !loading {
                y += 40.0;
                let (h, _) = kit::notice(
                    &mut cx,
                    x0 + 16.0,
                    y,
                    w - 32.0,
                    "No series yet",
                    Some("Add a series from Search, then turn on its bell to hear when new chapters come."),
                    None,
                );
                y += h + 20.0;
                if kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Find a series", Some("search")) {
                    to_search = true;
                }
                y += 46.0;
            }
            let rh = 128.0;
            for s in &list {
                if y + rh < view_top || y > view_bottom {
                    y += rh;
                    continue;
                }
                if cx.pressed(0.0, y, WIDTH, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                let (cw, ch) = (120.0, 112.0);
                let cy = y + (rh - ch) / 2.0;
                cover(&mut cx, &mut app.images, s, x0, cy, cw, ch, 6.0, now);
                if new_of[&s.id] > 0 {
                    new_tag(cx.c, x0 + 6.0, cy + 6.0, "New chapter");
                }
                let bx = WIDTH - 6.0 - 44.0;
                let by = y + (rh - 44.0) / 2.0;
                let bell = if s.bell { kit::icon_key_ink(&mut cx, bx, by, "bell-on", t::ACCENT) } else { kit::icon_key_ink(&mut cx, bx, by, "bell", t::INK3) };
                if bell {
                    act = Some(Act::Bell(s.id, !s.bell));
                }
                let tx = x0 + cw + 14.0;
                let tw = bx - 4.0 - tx;
                let title_t = Text::new(16.0, t::INK).semi().line(1.3);
                let lines = cx.c.wrap(&s.title, tw, title_t, 2);
                let line_t = Text::new(12.5, t::INK3);
                let block = lines.len() as f32 * title_t.lh() + 6.0 + line_t.lh();
                let mut ly = y + (rh - block) / 2.0;
                for l in &lines {
                    cx.c.text(l, tx, ly, title_t);
                    ly += title_t.lh();
                }
                let updated = if s.updated_at > 0 { format!("{} Updated", logic::local_date(s.updated_at)) } else { format!("Added {}", logic::local_date(s.added_at)) };
                cx.c.text_fit(&updated, tx, ly + 6.0, tw, line_t);
                if !bell && cx.tap(0.0, y, WIDTH, rh) {
                    act = Some(Act::Series(s.id));
                }
                y += rh;
            }
        }
        _ => {
            let cached = app.images.cached.clone().unwrap_or_default();
            // Per series: the chapters the server keeps, and those on this phone with their bytes.
            let mut rows: Vec<(&Series, usize, usize, u64)> = Vec::new();
            for s in &series {
                let Some(d) = app.series.get(&s.id).and_then(|l| l.data.as_ref()) else { continue };
                let server = d.chapters.iter().filter(|c| c.downloaded).count();
                let phone: Vec<u64> = d.chapters.iter().filter_map(|c| cached.get(&c.id).copied()).collect();
                if server > 0 || !phone.is_empty() {
                    rows.push((s, server, phone.len(), phone.iter().sum()));
                }
            }
            let total: u64 = rows.iter().map(|r| r.3).sum();
            let on_server: usize = rows.iter().map(|r| r.1).sum();
            y += 14.0;
            let ft = Text::new(26.0, t::INK).semi();
            let fw = cx.c.text(&size_words(total), x0, y, ft);
            cx.c.text("on this phone", x0 + fw + 8.0, y + ft.lh() - 20.0, Text::new(ty::UI, t::INK2));
            y += ft.lh() + 4.0;
            cx.c.text_fit(
                &format!("{} kept on your server, ready to read", logic::chapters_words(on_server)),
                x0,
                y,
                w,
                Text::new(12.5, t::INK3),
            );
            y += 30.0;
            if rows.is_empty() && app.images.cached.is_some() && !loading {
                y += 20.0;
                y += cx.c.para(
                    "Nothing downloaded. Download chapters from a series page; your server keeps the one after the chapter you last read.",
                    x0,
                    y,
                    w,
                    Text::new(ty::UI, t::INK2).line(1.45),
                    4,
                );
            }
            let rh = 84.0;
            for (s, server, phone, bytes) in rows {
                if cx.pressed(0.0, y, WIDTH, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                let (cw, ch) = (48.0, 66.0);
                cover(&mut cx, &mut app.images, s, x0, y + (rh - ch) / 2.0, cw, ch, 6.0, now);
                let tx = x0 + cw + 14.0;
                let bx = WIDTH - 6.0 - 44.0;
                let tw = bx - 4.0 - tx;
                let title_t = Text::new(ty::BODY, t::INK).medium();
                let line_t = Text::new(12.5, t::INK3);
                let ty0 = y + (rh - title_t.lh() - 4.0 - line_t.lh()) / 2.0;
                cx.c.text_fit(&s.title, tx, ty0, tw, title_t);
                let mut words = Vec::new();
                if server > 0 {
                    words.push(format!("{server} on your server"));
                }
                if phone > 0 {
                    words.push(format!("{phone} on this phone, {}", size_words(bytes)));
                }
                cx.c.text_fit(&words.join(" · "), tx, ty0 + title_t.lh() + 4.0, tw, line_t);
                if kit::icon_key_ink(&mut cx, bx, y + (rh - 44.0) / 2.0, "trash", t::INK2) {
                    act = Some(Act::Remove(s.id));
                } else if cx.tap(0.0, y, WIDTH, rh) {
                    act = Some(Act::Series(s.id));
                }
                y += rh;
            }
        }
    }
    if let Some(e) = &error {
        y += 12.0;
        y += cx.c.para(e, x0, y, w, Text::new(ty::CAPTION, t::DANGER), 3);
    }
    let bottom = y + kit::TABS_H;
    // The fixed top: the title, + and ⋯, then the tabs.
    let mut set_tab = None;
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + TOP, 0.0, t::BG);
        kit::fixed_top(&mut cx, TOP);
        cx.c.text("My space", PAD, top + 14.0, Text::new(ty::TITLE, t::INK).brand());
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0 * 2.0, top + 10.0, "plus") {
            st.add_menu = true;
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 10.0, "more") {
            st.more_menu = true;
        }
        let ty_ = top + 62.0;
        let tt = Text::new(13.5, t::INK3).medium();
        let mut x = PAD;
        for (i, words) in TABS.iter().enumerate() {
            let on = i as u8 == tab;
            let tw = cx.c.text(words, x, ty_ + (38.0 - tt.lh()) / 2.0, if on { tt.color(t::INK) } else { tt });
            if on {
                cx.c.rect(x, ty_ + 38.0 - 2.0, tw, 2.0, 1.0, t::ACCENT);
            }
            if cx.ui.tapped(x - 10.0, ty_, tw + 20.0, 38.0) {
                set_tab = Some(i as u8);
            }
            x += tw + 24.0;
        }
        cx.c.rect(-1000.0, ty_ + 38.0, WIDTH + 2000.0, 1.0, 0.0, t::LINE);
    }
    let tab_hit = kit::tabs(&mut cx, 2);
    // Over the page: the menus.
    let mut add_books = false;
    let mut to_notes = false;
    let mut to_stats = false;
    let mut new_sort = None;
    if covered {
        kit::release(&mut cx, held);
        let top = cx.c.inset_top + 56.0;
        if st.add_menu {
            match kit::menu(&mut cx, top, &["Books from this phone", "A series from Search"]) {
                Some(0) => {
                    add_books = true;
                    st.add_menu = false;
                }
                Some(1) => {
                    to_search = true;
                    st.add_menu = false;
                }
                Some(_) => st.add_menu = false,
                None => {}
            }
        } else if st.more_menu {
            match kit::menu(&mut cx, top, &["Highlights", "Reading time"]) {
                Some(0) => {
                    to_notes = true;
                    st.more_menu = false;
                }
                Some(1) => {
                    to_stats = true;
                    st.more_menu = false;
                }
                Some(_) => st.more_menu = false,
                None => {}
            }
        } else if st.sort_menu {
            match kit::menu(&mut cx, st.sort_top, &logic::FOLLOWING_SORTS) {
                Some(i) if i < 3 => {
                    new_sort = Some(i as u8);
                    st.sort_menu = false;
                }
                Some(_) => st.sort_menu = false,
                None => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    if let Some(s) = new_sort {
        app.set_following_sort(s);
    }
    if let Some(t_) = set_tab
        && t_ != tab
    {
        app.st.space.tab = t_;
        *app.scroll() = Default::default();
        if t_ == 2 {
            app.images.scan_cache();
        }
    }
    if add_books {
        app.host.pick_books();
    }
    match act {
        Some(Act::Series(id)) => app.push(Route::Series(id)),
        Some(Act::Continue(id)) => app.continue_series(id),
        Some(Act::Book(id)) => app.open_book(id),
        Some(Act::Bell(id, on)) => app.set_bell(id, on),
        Some(Act::Remove(id)) => remove_downloads(app, id),
        None => {
            if to_search {
                app.go_tab(1);
            } else if to_notes {
                app.push(Route::Notes);
            } else if to_stats {
                app.push(Route::Stats);
            } else if let Some(i) = tab_hit {
                app.go_tab(i);
            }
        }
    }
    pg.moving || app.library.loading && !crate::wake::has()
}

/// Remove a series' downloads: its chapters' pages on this phone, and the copies the server keeps.
fn remove_downloads(app: &mut App, id: i64) {
    let Some(d) = app.series.get(&id).and_then(|l| l.data.clone()) else { return };
    let cached = app.images.cached.clone().unwrap_or_default();
    let phone: Vec<i64> = d.chapters.iter().filter(|c| cached.contains_key(&c.id)).map(|c| c.id).collect();
    let server: Vec<i64> = d.chapters.iter().filter(|c| c.downloaded).map(|c| c.id).collect();
    let n = phone.iter().chain(server.iter()).collect::<std::collections::HashSet<_>>().len();
    app.images.remove_chapters(phone);
    app.download(&server, false);
    app.toast(&format!("Removed {} of {}", logic::chapters_words(n), d.series.title));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_shortly() {
        assert_eq!(size_words(0), "0 MB");
        assert_eq!(size_words(400_000), "under 1 MB");
        assert_eq!(size_words(61_200_000), "61 MB");
        assert_eq!(size_words(1_340_000_000), "1.3 GB");
    }
}
