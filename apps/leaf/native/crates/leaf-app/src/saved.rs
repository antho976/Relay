//! A series' chapters downloaded to this phone (My space › Downloads, a row tapped): each with its
//! picture, its size and Read; a tap reads it, with no network. Select (a long press, or ⋯ →
//! Select; a long press and drag sweeps many, [`crate::sweep`]) picks chapters and the bar at the bottom deletes them from this phone; ⋯ → "Delete read
//! chapters" deletes those read. Deleting never touches the server's copies. Built from what is
//! on disk ([`crate::offline`]), so it works with no server at all.

use std::collections::BTreeSet;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::thumb_key;
use crate::kit;
use crate::logic;
use crate::offline::Manifest;
use crate::theme::{self as t, PAD, ty};

#[derive(Default)]
pub struct State {
    pub menu: bool,
    /// Selecting: the chapters picked. None when not selecting.
    pub select: Option<BTreeSet<i64>>,
    /// Chapters to delete, waiting for the confirm.
    pub confirm: Option<Vec<i64>>,
}

impl State {
    /// Close the menu or the confirm, if one is open; else stop selecting.
    pub fn close_over(&mut self) -> bool {
        if self.menu || self.confirm.is_some() {
            self.menu = false;
            self.confirm = None;
            return true;
        }
        self.select.take().is_some()
    }

    fn covered(&self) -> bool {
        self.menu || self.confirm.is_some()
    }
}

/// A chapter's row.
const ROW_H: f32 = 70.0;
/// The bar at the bottom while selecting, over the gesture bar.
pub const BAR_H: f32 = 74.0;

/// A saved chapter is read: as the phone last knew (the series' list), else as it was saved.
pub fn is_read(app: &App, m: &Manifest) -> bool {
    app.series.get(&m.series).and_then(|l| l.data.as_ref()).and_then(|d| d.chapters.iter().find(|c| c.id == m.chapter.id)).map_or(m.chapter.read, |c| c.read)
}

/// The confirm before deleting `ids`: how many, and the room it frees. Some(true) to delete.
pub fn confirm(cx: &mut kit::Ctx, saver: &crate::offline::Saver, ids: &[i64]) -> Option<bool> {
    let bytes: u64 = ids.iter().filter_map(|id| saver.manifest(*id)).map(|m| m.bytes).sum();
    let title = format!("Delete {}?", logic::chapters_words(ids.len()));
    let body = format!("That frees {} on this phone. Your server keeps its own copies as before; download them again any time.", crate::space::size_words(bytes));
    kit::confirm(cx, &title, &body, "Delete")
}

/// Delete chapters from this phone, and say what it freed.
pub fn delete(app: &mut App, ids: &[i64]) {
    let (n, bytes) = app.saver.delete(ids);
    app.toast(&format!("Deleted {} · {} freed", logic::chapters_words(n), crate::space::size_words(bytes)));
}

/// The bar at the bottom while selecting: fixed chrome, its keys `(words, live, primary)` side by
/// side. Returns the one tapped.
pub fn select_bar(cx: &mut kit::Ctx, keys: &[(&str, bool, bool)]) -> Option<usize> {
    let saved = cx.c.dy;
    cx.c.dy = 0.0;
    let h = BAR_H + cx.c.inset_bottom;
    let y = cx.c.h - h;
    cx.c.rect_grad(-1000.0, y - 20.0, WIDTH + 2000.0, 20.0, 0.0, t::BG.a(0.0), t::BG);
    cx.c.rect(-1000.0, y, WIDTH + 2000.0, h, 0.0, t::BG);
    kit::chrome(cx, -1000.0, y, WIDTH + 2000.0, h);
    let w = WIDTH - 2.0 * PAD;
    let n = keys.len().max(1) as f32;
    let kw = (w - 8.0 * (n - 1.0)) / n;
    let mut hit = None;
    for (i, (words, live, primary)) in keys.iter().enumerate() {
        let (kx, ky) = (PAD + i as f32 * (kw + 8.0), y + 12.0);
        let tapped = if !live {
            cx.c.rect(kx, ky, kw, 50.0, t::KEY_R, t::S1);
            let tt = Text::new(ty::UI, t::INK3).semi().center();
            cx.c.text_fit(words, kx + kw / 2.0, ky + (50.0 - tt.lh()) / 2.0, kw - 12.0, tt);
            false
        } else if *primary {
            kit::primary(cx, kx, ky, kw, 50.0, words, None)
        } else {
            kit::plain(cx, kx, ky, kw, 50.0, words, None)
        };
        if tapped {
            hit = Some(i);
        }
    }
    cx.c.dy = saved;
    hit
}

enum Act {
    Open(i64),
    Toggle(i64),
}

pub fn draw(app: &mut App, series: i64, dt: f32) -> bool {
    if app.fresh {
        app.st.saved = State::default();
    }
    let list: Vec<Manifest> = app.saver.saved_of(series);
    let reads: Vec<bool> = list.iter().map(|m| is_read(app, m)).collect();
    let title = app.series_title(series);
    let covered = app.st.saved.covered();
    let selecting = app.st.saved.select.clone();
    let now = app.now_ns;
    let top_h = 60.0;
    let pg = app.page_begin(dt, top_h);
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    let (x0, w) = (PAD, WIDTH - 2.0 * PAD);
    let mut y = pg.top + 8.0;
    let bytes: u64 = list.iter().map(|m| m.bytes).sum();
    let ht = Text::new(ty::UI, t::INK2);
    cx.c.text_fit(&format!("{} · {} on this phone", logic::chapters_words(list.len()), crate::space::size_words(bytes)), x0, y, w, ht);
    y += ht.lh() + 14.0;
    if list.is_empty() && app.saver.scanned() {
        y += 30.0;
        let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Nothing of it on this phone", Some("Download chapters from the series page to read them offline."), None);
        y += h;
    }
    let mut act: Option<Act> = None;
    let mut rows: Vec<crate::sweep::Row> = Vec::with_capacity(list.len());
    let mut thumbs: Vec<(i64, bool)> = Vec::new();
    let (view_top, view_bottom) = (-cx.c.dy - 80.0, -cx.c.dy + cx.c.h + 80.0);
    // Newest first, as the series page lists them.
    for (m, read) in list.iter().zip(reads.iter()).rev() {
        let h = ROW_H;
        rows.push(crate::sweep::Row { id: m.chapter.id, y, h });
        if y + h < view_top || y > view_bottom {
            y += h;
            continue;
        }
        let c = &m.chapter;
        let picked = selecting.as_ref().is_some_and(|s| s.contains(&c.id));
        if picked {
            cx.c.rect(0.0, y, WIDTH, h, 0.0, t::ACCENT_DIM);
        }
        if cx.pressed(0.0, y, WIDTH, h) {
            cx.c.rect(0.0, y, WIDTH, h, 0.0, t::WASH);
        }
        let (pw, ph) = crate::series::PIC;
        let (px, py) = (x0, y + (h - ph) / 2.0);
        let has_pic = app.thumbs.contains(&c.id);
        if has_pic {
            thumbs.push((c.id, true));
        }
        if !(has_pic && cx.c.image_cover(&thumb_key(c.id), px, py, pw, ph, 6.0, Rgba::WHITE.a(if *read { 0.45 } else { 1.0 }))) {
            cx.c.rect(px, py, pw, ph, 6.0, t::S2);
        }
        let tx = px + pw + 14.0;
        let mut key_w = 24.0;
        let mut key_tap = false;
        if selecting.is_some() {
            let (cxx, cyy) = (WIDTH - PAD - 11.0, y + h / 2.0);
            if picked {
                cx.c.circle(cxx, cyy, 11.0, t::ACCENT);
                cx.c.icon("check", cxx, cyy, 13.0, t::ON_ACCENT);
            } else {
                cx.c.outline(cxx - 11.0, cyy - 11.0, 22.0, 22.0, 11.0, 1.5, t::INK3);
            }
        } else {
            let (kw, tapped) = kit::pill(&mut cx, WIDTH - PAD, y + (h - 32.0) / 2.0, "Read", *read);
            key_w = kw;
            key_tap = tapped;
        }
        let tw = WIDTH - PAD - key_w - 12.0 - tx;
        let title_t = Text::new(14.5, if *read { t::INK3 } else { t::INK }).semi();
        let line_t = Text::new(12.5, t::INK3);
        let ty0 = y + (h - title_t.lh() - 3.0 - line_t.lh()) / 2.0;
        let words = match logic::chapter_title(&c.name, c.number) {
            Some(n) => format!("Ch. {} · {n}", logic::num(c.number)),
            None => format!("Chapter {}", logic::num(c.number)),
        };
        cx.c.text_fit(&words, tx, ty0, tw, title_t);
        let size = crate::space::size_words(m.bytes);
        let line = if *read {
            format!("Read · {size}")
        } else if c.last_page > 0 {
            format!("page {} of {} · {size}", c.last_page + 1, m.pages.max(c.last_page + 1))
        } else {
            format!("{} pages · {size}", m.pages)
        };
        cx.c.text_fit(&line, tx, ty0 + title_t.lh() + 3.0, tw, line_t);
        if !covered && (key_tap || cx.tap(0.0, y, WIDTH, h)) {
            act = Some(if selecting.is_some() { Act::Toggle(c.id) } else { Act::Open(c.id) });
        }
        y += h;
    }
    app.images.want_thumbs(cx.c, &thumbs, now);
    let mut bottom = y + 24.0;
    // Selecting: Delete at the bottom, fixed.
    let mut bar_hit = None;
    if let Some(sel) = &selecting {
        bottom += BAR_H;
        let words = if sel.is_empty() { "Delete".to_string() } else { format!("Delete {}", sel.len()) };
        bar_hit = select_bar(&mut cx, &[(words.as_str(), !sel.is_empty(), true)]);
    }
    // The fixed top: Back, the title, ⋯ (selecting: close, how many, All).
    let mut back = false;
    let mut more = false;
    let mut stop = false;
    let mut all = false;
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        kit::fixed_top(&mut cx, top_h);
        match &selecting {
            Some(sel) => {
                stop = kit::icon_key(&mut cx, 6.0, top + 8.0, "close");
                let tt = Text::new(ty::SECTION, t::INK).semi();
                let words = if sel.is_empty() { "Tap chapters to select".to_string() } else { format!("{} selected", sel.len()) };
                cx.c.text_fit(&words, 56.0, top + 8.0 + (44.0 - tt.lh()) / 2.0, WIDTH - 56.0 - 90.0, tt);
                all = text_key(&mut cx, top + 8.0, "All");
            }
            None => {
                let (b, key, _) = kit::top_bar(&mut cx, top + 8.0, &title, &["more"]);
                back = b;
                more = key == Some(0);
            }
        }
    }
    let st = &mut app.st.saved;
    let mut confirmed: Option<Vec<i64>> = None;
    let mut delete_read = false;
    if covered {
        kit::release(&mut cx, held);
        if st.menu {
            let top = cx.c.inset_top + 56.0;
            match kit::menu(&mut cx, top, &["Select", "Delete read chapters"]) {
                Some(0) => {
                    st.menu = false;
                    st.select = Some(BTreeSet::new());
                }
                Some(1) => {
                    st.menu = false;
                    delete_read = true;
                }
                Some(_) => st.menu = false,
                None => {}
            }
        } else if let Some(ids) = st.confirm.clone() {
            match confirm(&mut cx, &app.saver, &ids) {
                Some(true) => {
                    st.confirm = None;
                    confirmed = Some(ids);
                }
                Some(false) => st.confirm = None,
                None => {}
            }
        }
    } else if more {
        st.menu = true;
    } else if stop {
        st.select = None;
    } else if all {
        st.select = Some(list.iter().map(|m| m.chapter.id).collect());
    } else if bar_hit == Some(0)
        && let Some(sel) = &st.select
    {
        st.confirm = Some(sel.iter().copied().collect());
    }
    if delete_read {
        let ids: Vec<i64> = list.iter().zip(reads.iter()).filter(|(_, r)| **r).map(|(m, _)| m.chapter.id).collect();
        if ids.is_empty() {
            app.toast("None of these is read yet");
        } else {
            app.st.saved.confirm = Some(ids);
        }
    }
    app.page_end(&pg, bottom);
    // A long press and drag selects (or lets go of) every row it passes.
    let band = (app.canvas.inset_top + top_h, app.canvas.h - app.canvas.inset_bottom - BAR_H);
    let swept = app.sweep_rows(rows, band, covered, dt, |st| &mut st.saved.select);
    if let Some(ids) = confirmed {
        delete(app, &ids);
        app.st.saved.select = None;
    }
    match act {
        Some(_) if covered => {}
        Some(Act::Open(id)) => open(app, series, id),
        Some(Act::Toggle(id)) => {
            if let Some(sel) = &mut app.st.saved.select
                && !sel.remove(&id)
            {
                sel.insert(id);
            }
        }
        None => {
            if back && !covered {
                app.pop();
            }
        }
    }
    pg.moving || swept
}

/// Read a saved chapter: in the series' list when it is at hand (online), else among the saved
/// ones alone.
pub fn open(app: &mut App, series: i64, chapter: i64) {
    let d = app.detail_for(series).filter(|d| d.chapters.iter().any(|c| c.id == chapter));
    let d = d.or_else(|| crate::offline::saved_detail(None, &app.saver.saved_of(series)));
    let Some(d) = d else { return };
    let Some(i) = d.chapters.iter().position(|c| c.id == chapter) else { return };
    let c = &d.chapters[i];
    let page = if c.read { 0 } else { logic::resume_page(c) };
    app.open_chapter(&d, i, page);
}

/// A word as a key at the top right ("All"). True when tapped.
pub fn text_key(cx: &mut kit::Ctx, y: f32, words: &str) -> bool {
    let rt = Text::new(ty::UI, t::INK).semi().right();
    let rw = cx.c.measure(words, rt);
    if cx.pressed(WIDTH - PAD - rw - 12.0, y + 2.0, rw + 24.0, 40.0) {
        cx.c.rect(WIDTH - PAD - rw - 12.0, y + 2.0, rw + 24.0, 40.0, 8.0, t::WASH);
    }
    cx.c.text(words, WIDTH - PAD, y + (44.0 - rt.lh()) / 2.0, rt);
    cx.ui.tapped(WIDTH - PAD - rw - 14.0, y - 2.0, rw + 14.0 + PAD, 48.0)
}
