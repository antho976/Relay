//! The reader: every chapter of the series as one strip, full bleed, scrolled with momentum. The
//! end of a chapter flows into the next past a slim heading. A tap anywhere on the strip
//! shows or hides a small overlay (Back, the series and chapter, a thin progress line, the
//! chapters before and after, auto-scroll and the dimmer). A tap does nothing else: no tap zones
//! and no volume keys (the owner, 2026-11-23).
//!
//! Auto-scroll moves the strip at the series' own speed (kept in `config.json`); it waits while
//! the page ahead is not loaded yet and goes on when it is, a tap stops it, and it stops by itself
//! only at the end of the last chapter.
//!
//! The place is a strip item and an offset within it ([`crate::strip`]), so a page above the
//! screen whose height arrives late never moves what is on it. Pages are fetched and decoded on
//! worker threads ([`crate::images`]); each frame says which it wants, nearest first, and which
//! to keep on disk ahead (the rest of this chapter and the whole next one). The place goes to the
//! server as the person reads ([`crate::logic::ProgressGate`]).

use std::collections::{HashMap, HashSet};

use leaf_proto::{Chapter, Progress, SeriesDetail};

use crate::app::App;
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::images::{Images, tile_h};
use crate::kit;
use crate::logic::{self, PAGE_W, ProgressGate, TILE_H};
use crate::net::Job;
use crate::strip::{self, Book, Item, Pos};
use crate::theme::{self as t, PAD, ty};
use crate::ui::Scroll;

/// A chapter's heading in the strip.
const HEAD_H: f32 = 64.0;
/// A chapter whose page count is still on its way.
const LOADING_H: f32 = 420.0;
/// A page not decoded yet stands in at this height (a webtoon strip is usually taller).
const PLACEHOLDER_H: f32 = 560.0;
/// A page that would not load.
const FAILED_H: f32 = 240.0;
const END_H: f32 = 240.0;
/// A chapter whose length did not come is asked again after this long, doubling each time up to
/// [`COUNT_RETRY_MAX_NS`]: never once a frame.
const COUNT_RETRY_NS: u64 = 5_000_000_000;
const COUNT_RETRY_MAX_NS: u64 = 60_000_000_000;

pub struct Reader {
    pub series: i64,
    pub title: String,
    pub chapters: Vec<Chapter>,
    /// Page counts as the server gave them, or none while unknown.
    counts: Vec<Option<u32>>,
    counts_asked: HashSet<usize>,
    /// Chapters whose length did not come: when to ask again, and how many times it failed.
    count_failed: HashMap<usize, (u64, u32)>,
    /// Chapters whose last page's top has been on screen: once its bottom is too, the chapter is
    /// finished. A glance up at the end of the chapter before, from the next one's heading, is not
    /// reading it.
    end_seen: HashSet<usize>,
    pub pos: Pos,
    /// Drives the momentum: its offset is reset each frame, the change is the strip's.
    fling: Scroll,
    pub overlay: bool,
    gate: ProgressGate,
    /// The status bar's height (with the bars hidden, the camera cut-out's): headings keep clear of it.
    top_pad: f32,
    /// `top_pad` was read once: from then on it only shrinks, so the bars showing with the
    /// overlay never push the strip down.
    top_read: bool,
    /// A PDF from the shelf (its book id), not a series: pages come from the platform's renderer
    /// and the place is kept on the phone, never sent to the server.
    pub local: Option<i64>,
    /// Auto-scroll is on, at this speed (1 to [`logic::SPEEDS`]).
    pub auto: bool,
    pub speed: u8,
    /// Auto-scroll is waiting for the page ahead.
    pub waiting: bool,
    /// Opened at a place kept on the phone: how far down its item, once that item's height is
    /// known (a page decoded, a chapter's length come). A move before then lets it go.
    pending_off: Option<f32>,
    /// When the place is kept on the phone ([`crate::places::Gate`]).
    save: crate::places::Gate,
    /// The place last kept on the phone (the overlay shows it).
    pub saved: Option<crate::places::Place>,
    /// A page just hidden by hand, with Undo offered until then: (chapter id, page, until ns).
    pub undo: Option<(i64, u32, u64)>,
    /// The sheet over the strip after a long press on a page: (chapter index, page).
    pub hide_sheet: Option<(usize, u32)>,
}

/// Undo is offered this long after a page is hidden.
pub const UNDO_NS: u64 = 5_000_000_000;

/// How far ahead of the screen's bottom auto-scroll wants pages loaded before it moves on.
const AHEAD: f32 = 0.6;
/// How far past the screen (in screens) a page's tiles go to the atlas ahead of being shown.
const TILE_AHEAD: f32 = 0.5;
/// How far below the screen (in screens) pages are decoded ready: a flick covers two screens in
/// a moment, and a page decoded only once it is reached shows as a dark square first. About
/// eight webtoon pages, some 4 MB each.
const DECODE_AHEAD: f32 = 5.0;

/// The strip as the images know it now.
struct View<'a> {
    r: &'a Reader,
    im: &'a Images,
}

impl Book for View<'_> {
    fn chapters(&self) -> usize {
        self.r.chapters.len()
    }
    fn pages(&self, c: usize) -> Option<u32> {
        self.r.counts.get(c).copied().flatten()
    }
    fn height(&self, item: Item) -> f32 {
        match item {
            // Each heading keeps the status bar's (or the camera cut-out's) height above its words,
            // so a chapter opened at its heading reads clear of them.
            Item::Head(c) => self.r.top_pad + HEAD_H + if self.pages(c).is_none() { LOADING_H } else { 0.0 },
            Item::Page(c, n) => {
                let id = self.r.chapters[c].id;
                // A scanlator's promo page, or one hidden by hand: not in the strip at all.
                if self.im.skipped(id, n) {
                    return 0.0;
                }
                match self.im.size(id, n) {
                    Some((w, h)) if w > 0 => h as f32 * WIDTH / w as f32,
                    _ if self.im.failed(id, n).is_some() => FAILED_H,
                    _ => PLACEHOLDER_H,
                }
            }
            Item::End => END_H,
        }
    }
}

/// Small enough that f32 keeps fractions of a unit; a frame never moves this far.
const FLING_MID: f32 = 4096.0;

/// A scroller with room both ways, whose changes the strip takes.
fn fling() -> Scroll {
    let mut s = Scroll::default();
    s.off = FLING_MID;
    s.max = 2.0 * FLING_MID;
    s
}

impl Reader {
    /// Open at page `page` of chapter `index` (oldest first) of a series.
    pub fn open(d: &SeriesDetail, index: usize, page: u32) -> Reader {
        let chapters = d.chapters.clone();
        let counts = chapters.iter().map(|c| (c.pages > 0).then_some(c.pages)).collect();
        let mut gate = ProgressGate::default();
        for c in &chapters {
            gate.known(c.id, c.last_page, c.read);
        }
        let index = index.min(chapters.len().saturating_sub(1));
        let item = if page > 0 { Item::Page(index, page) } else { Item::Head(index) };
        Reader {
            series: d.series.id,
            title: d.series.title.clone(),
            chapters,
            counts,
            counts_asked: HashSet::new(),
            count_failed: HashMap::new(),
            end_seen: HashSet::new(),
            pos: Pos { item, off: 0.0 },
            fling: fling(),
            overlay: false,
            gate,
            top_pad: 0.0,
            top_read: false,
            local: None,
            auto: false,
            speed: logic::DEFAULT_SPEED,
            waiting: false,
            pending_off: None,
            save: Default::default(),
            saved: None,
            undo: None,
            hide_sheet: None,
        }
    }

    /// A PDF from the shelf, as one chapter of pages (chapter id `-book`), at its place.
    pub fn open_pdf(b: &crate::shelf::Book) -> Reader {
        let chapter = Chapter {
            id: -b.id,
            number: 1.0,
            name: b.title.clone(),
            group: None,
            pages: b.pages,
            read: false,
            last_page: b.place.block,
            downloaded: true,
            uploaded_at: 0,
            from: None,
        };
        let page = b.place.block.min(b.pages.saturating_sub(1));
        let item = if page > 0 { Item::Page(0, page) } else { Item::Head(0) };
        Reader {
            series: -b.id,
            title: b.title.clone(),
            counts: vec![Some(b.pages)],
            chapters: vec![chapter],
            counts_asked: HashSet::new(),
            count_failed: HashMap::new(),
            end_seen: HashSet::new(),
            pos: Pos { item, off: 0.0 },
            fling: fling(),
            overlay: false,
            gate: ProgressGate::default(),
            top_pad: 0.0,
            top_read: false,
            local: Some(b.id),
            auto: false,
            speed: logic::DEFAULT_SPEED,
            waiting: false,
            pending_off: None,
            save: Default::default(),
            saved: None,
            undo: None,
            hide_sheet: None,
        }
    }

    /// A PDF's place (the page at the top) and how far through.
    pub fn local_place(&self) -> Option<(crate::shelf::Place, f32)> {
        let pages = self.counts.first().copied().flatten()?;
        let page = match self.pos.item {
            Item::Head(_) => 0,
            Item::Page(_, n) => n,
            Item::End => pages.saturating_sub(1),
        };
        Some((crate::shelf::Place { section: 0, block: page, byte: 0 }, (page + 1) as f32 / pages.max(1) as f32))
    }

    /// Open at a place kept on the phone, in chapter `index`: its page (or heading), then its
    /// offset once that page's height is known.
    pub fn restore(&mut self, index: usize, p: crate::places::Place) {
        let item = if p.head { Item::Head(index) } else { Item::Page(index, p.page) };
        self.pos = Pos { item, off: 0.0 };
        self.pending_off = (p.off > 0.0).then_some(p.off);
        self.saved = Some(p);
    }

    /// Where the top of the screen is, as the phone keeps it (a series only; while a restored
    /// offset waits for its page, the place it was opened at).
    pub fn place_now(&self) -> Option<crate::places::Place> {
        if self.local.is_some() || self.chapters.is_empty() {
            return None;
        }
        if self.pending_off.is_some() {
            return self.saved;
        }
        let (c, page, head) = match self.pos.item {
            Item::Head(c) => (c, 0, true),
            Item::Page(c, n) => (c, n, false),
            Item::End => return None,
        };
        let ch = &self.chapters[c];
        Some(crate::places::Place { series: self.series, chapter: ch.id, number: ch.number, page, head, off: self.pos.off.max(0.0), at_ms: crate::shelf::now_ms() })
    }

    /// The place kept now, whatever the gate says (the app goes to the background, the reader
    /// closes).
    pub fn keep_now(&mut self, now_ns: u64) -> Option<crate::places::Place> {
        let p = self.place_now()?;
        if !self.further(&p) {
            return self.saved;
        }
        self.save.force(&p, now_ns);
        self.saved = Some(p);
        Some(p)
    }

    /// Where a place is along the strip: its chapter here, its heading (0) or page (1 + n), and how
    /// far down that. None for a chapter not in this strip.
    fn along(&self, p: &crate::places::Place) -> Option<(usize, u32, f32)> {
        let c = self.chapters.iter().position(|ch| ch.id == p.chapter)?;
        Some((c, if p.head { 0 } else { p.page + 1 }, p.off))
    }

    /// The place only moves down the strip (the owner, 2026-11-24): scrolling back up to look at
    /// something leaves it where reading got to. True when `p` is further than the place kept, or
    /// nothing is kept yet.
    pub fn further(&self, p: &crate::places::Place) -> bool {
        match (self.saved.and_then(|s| self.along(&s)), self.along(p)) {
            (Some(s), Some(n)) => n > s,
            _ => true,
        }
    }

    /// The top of the screen is on a page (or heading) before the place kept.
    fn behind_saved(&self) -> bool {
        let Some((sc, slot, _)) = self.saved.and_then(|s| self.along(&s)) else { return false };
        match self.pos.item {
            Item::Head(c) => (c, 0) < (sc, slot),
            Item::Page(c, n) => (c, n + 1) < (sc, slot),
            Item::End => false,
        }
    }

    /// A page leaves the strip (hidden by hand, `old_h` units tall until now, or skipped by the
    /// server): when it is the one the screen's top is in, the place moves to what follows it so
    /// that nothing below it moves on screen. Above the screen, nothing moves anyway (the place is
    /// an item and an offset).
    /// Call once the page is skipped ([`Images::skip`]), with its height from before.
    pub fn drop_page(&mut self, im: &Images, chapter: i64, n: u32, old_h: f32, view_h: f32) {
        let Some(c) = self.chapters.iter().position(|ch| ch.id == chapter) else { return };
        if self.pos.item == Item::Page(c, n) {
            self.anchor_past(im, old_h, view_h);
        }
    }

    /// A page's height in the strip now (0 when it is not this series').
    pub fn page_h(&self, im: &Images, chapter: i64, n: u32) -> f32 {
        match self.chapters.iter().position(|ch| ch.id == chapter) {
            Some(c) => View { r: self, im }.height(Item::Page(c, n)),
            None => 0.0,
        }
    }

    /// The place, in a page whose height became 0, moved to the item after it at the same spot on
    /// screen (that item's top stays `old_h - off` down the screen).
    fn anchor_past(&mut self, im: &Images, old_h: f32, view_h: f32) {
        let v = View { r: self, im };
        let Some(next) = strip::next(&v, self.pos.item) else { return };
        let p = strip::normalize(&v, Pos { item: next, off: self.pos.off - old_h });
        self.pos = strip::clamp_bottom(&v, p, view_h);
        self.pending_off = None;
    }

    /// Screenshots: chapters whose length is not listed get `n` pages, so the shot asks the
    /// server for none.
    pub fn shot_counts(&mut self, n: u32) {
        for c in &mut self.counts {
            if c.is_none() {
                *c = Some(n);
            }
        }
    }

    /// The items on screen, each with its top's y.
    pub fn visible_items(&self, im: &Images, view_h: f32) -> Vec<(Item, f32)> {
        strip::visible(&View { r: self, im }, self.pos, view_h, 0.0)
    }

    /// Page `n` is the last of chapter `c` that is shown (those after it are left out).
    fn last_shown(&self, im: &Images, c: usize, n: u32) -> bool {
        let id = self.chapters[c].id;
        self.counts[c].is_some_and(|p| n < p && (n + 1..p).all(|k| im.skipped(id, k)))
    }

    /// Chapter `c`'s page count, once known.
    pub fn count(&self, c: usize) -> Option<u32> {
        self.counts.get(c).copied().flatten()
    }

    /// A chapter's length came (or did not, at `now_ns`).
    pub fn page_count(&mut self, chapter: i64, count: Option<u32>, now_ns: u64) {
        if let Some(i) = self.chapters.iter().position(|c| c.id == chapter) {
            match count {
                Some(n) => {
                    self.counts[i] = Some(n);
                    self.count_failed.remove(&i);
                    // Opened at a page before the length was known: kept inside the chapter.
                    if let Item::Page(c, p) = self.pos.item
                        && c == i
                        && p >= n
                    {
                        self.pos = Pos { item: if n > 0 { Item::Page(c, n - 1) } else { Item::Head(c) }, off: 0.0 };
                    }
                }
                // Asked again a little later, if it is still near then; each failure waits longer.
                None => {
                    let tries = self.count_failed.get(&i).map_or(0, |f| f.1) + 1;
                    let wait = (COUNT_RETRY_NS << (tries - 1).min(4)).min(COUNT_RETRY_MAX_NS);
                    self.count_failed.insert(i, (now_ns + wait, tries));
                }
            }
        }
    }

    pub fn scroll_by(&mut self, im: &Images, dy: f32, view_h: f32) {
        let v = View { r: self, im };
        let p = strip::scroll(&v, self.pos, dy, view_h);
        self.pos = p;
    }

    /// What the reader last told (or holds back to tell) the server of each chapter.
    pub fn places(&self) -> Vec<(i64, Progress)> {
        self.chapters.iter().filter_map(|c| self.gate.place(c.id).map(|p| (c.id, p))).collect()
    }

    /// The report held back, as the reader closes.
    pub fn flush(&mut self, now_ns: u64) -> Option<(i64, Progress)> {
        self.gate.flush(now_ns)
    }

    /// The chapter and page the top of the screen is in.
    fn place(&self, v: &View) -> (usize, u32) {
        match self.pos.item {
            Item::Head(c) => (c, 0),
            Item::Page(c, n) => (c, n),
            Item::End => {
                let c = self.chapters.len().saturating_sub(1);
                (c, v.pages(c).unwrap_or(1).saturating_sub(1))
            }
        }
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let Some(mut r) = app.reader.take() else {
        app.pop();
        return true;
    };
    let (moving, leave, act) = frame(app, &mut r, dt);
    // The phone's bars (the time, the battery) show with the overlay and hide with it (the owner,
    // 2026-11-24).
    app.immersive(!r.overlay);
    let series = (r.local.is_none()).then_some(r.series);
    app.reader = Some(r);
    match act {
        Some(PageAct::Hide(c, n)) => app.hide_page(c, n),
        Some(PageAct::Unhide(c, n)) => app.unhide_page(c, n),
        None => {}
    }
    match leave {
        Leave::Stay => {}
        Leave::Back => {
            app.pop();
        }
        // The end's key says "Back to the series": there, also when the reader was opened from
        // Home or an update.
        Leave::ToSeries => {
            app.pop();
            if let Some(id) = series
                && app.route() != &crate::app::Route::Series(id)
            {
                app.push(crate::app::Route::Series(id));
            }
        }
    }
    moving
}

/// A page hidden by hand, or that undone.
enum PageAct {
    Hide(i64, u32),
    Unhide(i64, u32),
}

/// The Undo toast's box, at the screen's bottom (above the overlay's bar when it shows).
fn undo_box(c: &crate::draw::Canvas, overlay: bool) -> (f32, f32, f32, f32) {
    let w = WIDTH - 2.0 * PAD;
    let bottom = c.h - c.inset_bottom - if overlay { BAR_H } else { 0.0 };
    (PAD, bottom - 16.0 - 48.0, w, 48.0)
}

/// Where a frame of the reader goes next.
enum Leave {
    Stay,
    Back,
    ToSeries,
}

/// One frame of the reader. Returns (still moving, where to go).
fn frame(app: &mut App, r: &mut Reader, dt: f32) -> (bool, Leave, Option<PageAct>) {
    let view_h = app.canvas.h;
    let now = app.now_ns;
    r.top_pad = if r.top_read { r.top_pad.min(app.canvas.inset_top) } else { app.canvas.inset_top };
    r.top_read = true;
    let mut page_act: Option<PageAct> = None;
    // The sheet after a long press holds every tap and drag from the strip under it.
    let sheet_open = r.hide_sheet.is_some();
    let held = if sheet_open {
        app.ui.cover();
        app.ui.tap.take()
    } else {
        None
    };
    // The Undo toast takes its tap before anything under it.
    if let Some((chapter, n, until)) = r.undo {
        if now >= until {
            r.undo = None;
        } else {
            app.wake_at(until);
            let (x, y, w, h) = undo_box(&app.canvas, r.overlay);
            if app.ui.tapped(x, y, w, h) {
                page_act = Some(PageAct::Unhide(chapter, n));
            }
        }
    }
    // The overlay's keys take their taps before the strip does.
    let overlay_tap = r.overlay;
    let (top_bar, bottom_bar) = (app.canvas.inset_top + 60.0, BAR_H + app.canvas.inset_bottom);
    // A drag on the overlay's bars (the dimmer) is theirs, not the strip's; nor is one that starts
    // on the phone's own gesture strips at the top and bottom edges (a swipe home, or down for the
    // shade, never moves the page: the owner, 2026-11-24).
    let edges = (app.canvas.inset_top.max(EDGE_ZONE), view_h - app.canvas.inset_bottom.max(EDGE_ZONE));
    let from_edge = app.ui.drag_from.is_some_and(|(_, y)| y < edges.0 || y > edges.1);
    let on_bars = sheet_open || from_edge || (r.overlay && app.ui.drag_from.is_some_and(|(_, y)| y < top_bar || y > view_h - bottom_bar));
    // Scroll: the finger, then the momentum it left, auto-scroll.
    let before = r.fling.off;
    let coasting = if on_bars { false } else { r.fling.update(&app.ui, [0.0, 0.0, WIDTH, view_h], dt) };
    let mut dy = r.fling.off - before;
    r.fling.off = FLING_MID;
    let mut moving = coasting;
    if r.auto && !sheet_open {
        let v = View { r, im: &app.images };
        match ahead(&v, r.pos, view_h) {
            Ahead::Ready => {
                r.waiting = false;
                if !app.ui.busy() {
                    dy += logic::scroll_speed(r.speed) * dt;
                }
                moving = true;
            }
            Ahead::Loading => r.waiting = true,
            // A page that would not load: stop at it (its Retry is on screen), never skip it.
            Ahead::Failed => {
                r.auto = false;
                r.waiting = false;
            }
        }
    }
    // A page the server skips that the screen's top is in (it was a placeholder until now): the
    // place moves past it, what follows staying put on screen.
    if let Item::Page(c, n) = r.pos.item
        && app.images.skipped(r.chapters[c].id, n)
    {
        let old = if r.pos.off > 0.0 { PLACEHOLDER_H } else { 0.0 };
        r.anchor_past(&app.images, old, view_h);
    }
    // A place kept on the phone: its offset, once its item's height is known; a move first lets
    // it go (the item's top stays where it is).
    if r.pending_off.is_some() && dy != 0.0 {
        r.pending_off = None;
    }
    if let Some(off) = r.pending_off {
        let v = View { r, im: &app.images };
        let known = match r.pos.item {
            Item::Page(c, n) => {
                let id = r.chapters[c].id;
                app.images.size(id, n).is_some() || app.images.failed(id, n).is_some() || app.images.skipped(id, n)
            }
            Item::Head(c) => r.counts[c].is_some(),
            Item::End => true,
        };
        if known {
            let h = v.height(r.pos.item);
            let p = Pos { item: r.pos.item, off: off.min((h - 1.0).max(0.0)) };
            r.pos = strip::clamp_bottom(&v, p, view_h);
            r.pending_off = None;
        }
    }
    if dy != 0.0 {
        let v = View { r, im: &app.images };
        let p = strip::scroll(&v, r.pos, dy, view_h);
        let stuck = p == r.pos;
        r.pos = p;
        if stuck {
            r.fling.stop();
            // The end of the last chapter: nothing more to scroll to.
            if r.auto && dy > 0.0 {
                r.auto = false;
            }
        }
    }
    // Ask for page counts the strip needs (a chapter the server listed without them).
    let near_chapter = {
        let v = View { r, im: &app.images };
        strip::chapter_of(&v, r.pos.item)
    };
    for c in [near_chapter, near_chapter + 1] {
        if r.local.is_none() && c < r.chapters.len() && r.counts[c].is_none() {
            match r.count_failed.get(&c) {
                Some(&(at, _)) if now < at => {
                    app.wake_at(at);
                    continue;
                }
                Some(_) => {
                    r.counts_asked.remove(&c);
                }
                None => {}
            }
            if r.counts_asked.insert(c) {
                app.ask(Job::PageCount(r.chapters[c].id));
            }
        }
    }
    // What to fetch: what is on screen and two screens below first (needed: a failure shows),
    // then one above; then decoded ready, [`DECODE_AHEAD`] screens below; then, only to disk, the
    // rest of this chapter and the whole next one.
    let (shown, wants, keep, firm) = {
        let v = View { r, im: &app.images };
        let shown = strip::visible(&v, r.pos, view_h, 0.0);
        let mut near = strip::visible(&v, r.pos, view_h, view_h * 2.0);
        near.extend(strip::above(&v, r.pos, view_h));
        let mut wants: Vec<(i64, u32, bool)> = Vec::new();
        let mut keep = HashSet::new();
        for (item, _) in &near {
            if let Item::Page(c, n) = *item {
                let id = r.chapters[c].id;
                wants.push((id, n, true));
                keep.insert((id, n));
            }
        }
        let firm = wants.len();
        for (item, _) in strip::visible(&v, r.pos, view_h, view_h * DECODE_AHEAD) {
            if let Item::Page(c, n) = item {
                let id = r.chapters[c].id;
                if keep.insert((id, n)) {
                    wants.push((id, n, true));
                }
            }
        }
        let c = near_chapter;
        for ch in [c, c + 1] {
            if ch < r.chapters.len()
                && let Some(n) = r.counts[ch]
            {
                let id = r.chapters[ch].id;
                for p in 0..n {
                    if !keep.contains(&(id, p)) {
                        wants.push((id, p, false));
                    }
                }
            }
        }
        (shown, wants, keep, firm)
    };
    app.images.want_pages_firm(&wants, firm);
    app.images.trim(&keep);
    // Draw the strip.
    let mut retry = false;
    let mut to_series = false;
    {
        let v_im = &app.images;
        let c = &mut app.canvas;
        let ui = &mut app.ui;
        for (item, y) in &shown {
            let h = View { r, im: v_im }.height(*item);
            match *item {
                Item::Head(ci) => {
                    let ch = &r.chapters[ci];
                    let words = if r.local.is_some() { ch.name.clone() } else { format!("Chapter {}", logic::num(ch.number)) };
                    let tt = Text::new(ty::CAPTION, t::INK3).medium().center();
                    let y = &(y + r.top_pad);
                    let ty_ = y + (HEAD_H - tt.lh()) / 2.0;
                    let tw = c.measure(&words, tt);
                    c.text(&words, WIDTH / 2.0, ty_, tt);
                    let mid = y + HEAD_H / 2.0;
                    c.rect(PAD, mid, WIDTH / 2.0 - tw / 2.0 - 14.0 - PAD, 1.0, 0.0, t::LINE);
                    c.rect(WIDTH / 2.0 + tw / 2.0 + 14.0, mid, WIDTH / 2.0 - tw / 2.0 - 14.0 - PAD, 1.0, 0.0, t::LINE);
                    if r.counts[ci].is_none() {
                        let mid = y + HEAD_H + LOADING_H / 2.0 - 10.0;
                        if r.count_failed.contains_key(&ci) {
                            c.text(&format!("Chapter {} didn't open", logic::num(ch.number)), WIDTH / 2.0, mid - 12.0, Text::new(ty::BODY, t::INK2).center());
                            c.text("Trying again in a moment", WIDTH / 2.0, mid + 14.0, Text::new(ty::CAPTION, t::INK3).center());
                        } else {
                            c.text(&format!("Opening chapter {}", logic::num(ch.number)), WIDTH / 2.0, mid, Text::new(ty::UI, t::INK3).center());
                        }
                    }
                }
                Item::Page(ci, n) => {
                    let id = r.chapters[ci].id;
                    let size = v_im.size(id, n);
                    let mut drawn = false;
                    if let Some((pw, ph)) = size {
                        drawn = true;
                        let scale = WIDTH / pw as f32;
                        for i in 0..ph.div_ceil(TILE_H) as usize {
                            let key = v_im.tile_key(id, n, i);
                            let th = tile_h(ph, i);
                            let top = y + (i as u32 * TILE_H) as f32 * scale;
                            let bottom = y + ((i as u32 * TILE_H) + th) as f32 * scale;
                            // Only tiles on screen or about to be go to the atlas: a webtoon strip's
                            // dozen tiles would otherwise go up at once, and push out the ones shown.
                            if bottom < -view_h * TILE_AHEAD || top > view_h * (1.0 + TILE_AHEAD) {
                                continue;
                            }
                            if !c.has_image(&key)
                                && let Some(img) = v_im.page(id, n)
                                && let Some(px) = img.tiles.get(i)
                            {
                                c.add_image_slot(&key, pw, th, (PAGE_W, TILE_H), px.clone());
                            }
                            if bottom < 0.0 || top > view_h {
                                continue;
                            }
                            if !c.image_tile(&key, 0.0, top, WIDTH, bottom) {
                                c.rect(0.0, top, WIDTH, bottom - top, 0.0, t::S1);
                            }
                        }
                    }
                    if !drawn {
                        c.rect(0.0, *y, WIDTH, h, 0.0, t::S1);
                        let label = Text::new(ty::CAPTION, t::INK3).center();
                        match v_im.failed(id, n) {
                            Some(why) => {
                                c.text(&format!("Page {} didn't load", n + 1), WIDTH / 2.0, y + h / 2.0 - 36.0, Text::new(ty::BODY, t::INK2).center());
                                c.text_fit(why, WIDTH / 2.0, y + h / 2.0 - 10.0, WIDTH - 2.0 * PAD, label);
                                let (kw, kh) = (120.0, 40.0);
                                let (kx, ky) = (WIDTH / 2.0 - kw / 2.0, y + h / 2.0 + 20.0);
                                c.rect(kx, ky, kw, kh, t::KEY_R, t::S2);
                                let kt = Text::new(ty::UI, t::INK).semi().center();
                                c.text("Retry", WIDTH / 2.0, ky + (kh - kt.lh()) / 2.0, kt);
                                if !overlay_tap && ui.tapped(kx, ky, kw, kh) {
                                    retry = true;
                                }
                            }
                            None => {
                                c.text(&format!("{}", n + 1), WIDTH / 2.0, y + 40.0, label);
                            }
                        }
                    }
                }
                Item::End => {
                    let last = r.chapters.last();
                    let words = match last {
                        _ if r.local.is_some() => "The end".to_string(),
                        Some(ch) => format!("That's the latest chapter, {}", logic::num(ch.number)),
                        None => "No chapters yet".into(),
                    };
                    c.text(&words, WIDTH / 2.0, y + 56.0, Text::new(ty::BODY, t::INK).semi().center());
                    let sub = if r.local.is_some() { "Back to your library when you're done." } else { "New ones arrive in your library as your server finds them." };
                    c.text(sub, WIDTH / 2.0, y + 84.0, Text::new(ty::CAPTION, t::INK3).center());
                    let (kw, kh) = (180.0, 44.0);
                    let (kx, ky) = (WIDTH / 2.0 - kw / 2.0, y + 124.0);
                    c.rect(kx, ky, kw, kh, t::KEY_R, t::S2);
                    let kt = Text::new(ty::UI, t::INK).semi().center();
                    c.text(if r.local.is_some() { "Back to your library" } else { "Back to the series" }, WIDTH / 2.0, ky + (kh - kt.lh()) / 2.0, kt);
                    if !overlay_tap && ui.tapped(kx, ky, kw, kh) {
                        to_series = true;
                    }
                }
            }
        }
    }
    // A finger held on a page: the sheet that hides it (and the ones like it).
    if !sheet_open
        && !r.overlay
        && !r.auto
        && r.local.is_none()
        && let Some(((_, py), ns)) = app.ui.resting(now)
        && ns >= crate::series::LONG_PRESS_NS
    {
        for (item, y) in &shown {
            if let Item::Page(c, n) = *item {
                let h = View { r, im: &app.images }.height(*item);
                if py >= *y && py < y + h && app.images.size(r.chapters[c].id, n).is_some() {
                    app.ui.swallow();
                    r.hide_sheet = Some((c, n));
                    r.fling.stop();
                    break;
                }
            }
        }
    }
    // Where the reader is, and what is finished.
    let (ci, page) = {
        let v = View { r, im: &app.images };
        r.place(&v)
    };
    if !app.shot && !r.chapters.is_empty() && r.local.is_none() {
        // Back above the place kept, the server is not told the page either: it keeps the furthest.
        let mut reports = if r.behind_saved() { Vec::new() } else { r.gate.see(r.chapters[ci].id, page, false, now) };
        // A chapter is finished once its last page has been on screen from its top to its bottom.
        // Promo pages left out at its end count as seen: the last page shown is its end.
        for (item, y) in &shown {
            if let Item::Page(c, n) = *item
                && r.last_shown(&app.images, c, n)
            {
                if *y >= -1.0 {
                    r.end_seen.insert(c);
                }
                let h = View { r, im: &app.images }.height(*item);
                let id = r.chapters[c].id;
                if y + h <= view_h + 1.0 && r.end_seen.contains(&c) && (app.images.size(id, n).is_some() || app.images.skipped(id, n)) {
                    reports.extend(r.gate.see(r.chapters[c].id, n, true, now));
                }
            }
        }
        reports.extend(r.gate.tick(now));
        for (chapter, p) in reports {
            if p.done
                && let Some(c) = r.chapters.iter_mut().find(|c| c.id == chapter)
            {
                c.read = true;
            }
            app.report_progress(chapter, p);
        }
        if let Some(at) = r.gate.due_at() {
            app.wake_at(at);
        }
    }
    // The exact place, kept on the phone: on each new page, and every two seconds while the strip
    // moves within one; only further down ([`Reader::further`]).
    if r.pending_off.is_none()
        && let Some(p) = r.place_now()
        && r.further(&p)
    {
        if r.save.due(&p, now) {
            app.places.put(p);
            r.saved = Some(p);
        } else if !r.saved.is_some_and(|s| s.same_spot(&p)) {
            app.wake_at(r.save.next_ns());
        }
    }
    // The dimmer: a black veil over the pages, under the overlay.
    if app.prefs.dim > 0.0 {
        let c = &mut app.canvas;
        c.rect(-1000.0, 0.0, WIDTH + 2000.0, view_h, 0.0, crate::draw::Rgba::hex(0x000000).a(app.prefs.dim.min(crate::app::DIM_MAX)));
    }
    if r.auto && r.waiting && !r.overlay {
        let c = &mut app.canvas;
        let words = "Waiting for the next page";
        let tt = Text::new(ty::CAPTION, t::INK2).medium();
        let w = c.measure(words, tt) + 28.0;
        let y = view_h - c.inset_bottom - 56.0;
        c.rect((WIDTH - w) / 2.0, y, w, 32.0, 16.0, t::GLASS);
        c.text(words, WIDTH / 2.0, y + (32.0 - tt.lh()) / 2.0, tt.center());
    }
    // The overlay, or the tap that brings it.
    let mut back = false;
    let mut jump: Option<usize> = None;
    let mut speed: Option<u8> = None;
    let mut dim: Option<f32> = None;
    if r.overlay {
        // The bookmark: the exact spot the place is kept at, a line across the strip and its tag.
        if let Some(s) = r.saved.filter(|_| r.local.is_none()) {
            let item = r.chapters.iter().position(|c| c.id == s.chapter).map(|i| if s.head { Item::Head(i) } else { Item::Page(i, s.page) });
            if let Some((_, y)) = item.and_then(|it| shown.iter().find(|(x, _)| *x == it)) {
                let line_y = (y + s.off).max(app.canvas.inset_top + 60.0);
                if line_y < view_h - BAR_H - app.canvas.inset_bottom {
                    bookmark_mark(&mut app.canvas, line_y);
                }
            }
        }
        let mut cx = cx!(app);
        let top = cx.c.inset_top;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 60.0, 0.0, t::GLASS);
        if kit::icon_key(&mut cx, 6.0, top + 8.0, "arrow-left") {
            back = true;
        }
        let ch = &r.chapters[ci];
        cx.c.text_fit(&r.title, 56.0, top + 9.0, WIDTH - 56.0 - PAD, Text::new(ty::UI, t::INK).semi());
        let pages = r.counts[ci];
        let sub = match pages {
            Some(n) if r.local.is_some() => format!("Page {} of {n}", (page + 1).min(n)),
            Some(n) => format!("Chapter {} · page {} of {n}", logic::num(ch.number), (page + 1).min(n)),
            None => format!("Chapter {}", logic::num(ch.number)),
        };
        let sub_t = Text::new(ty::CAPTION, t::INK2);
        // Where the place is kept on the phone: "Saved · page 12".
        // In this chapter the place is marked where it is, on the strip (below); in another, said here.
        let saved = r.saved.filter(|_| r.local.is_none()).map(|s| match r.chapters.iter().position(|c| c.id == s.chapter) {
            Some(i) if i == ci => (Some(s), String::new()),
            Some(i) => (None, format!("Saved · ch. {} p. {}", logic::num(r.chapters[i].number), s.page + 1)),
            None => (None, "Saved".to_string()),
        });
        let sub_w = cx.c.measure(&sub, sub_t);
        cx.c.text_fit(&sub, 56.0, top + 31.0, WIDTH - 56.0 - PAD, sub_t);
        if let Some((_, words)) = saved.as_ref().filter(|(_, w)| !w.is_empty()) {
            let sx = 56.0 + sub_w + 8.0;
            if sx < WIDTH - PAD - 60.0 {
                cx.c.text_fit(&format!("· {words}"), sx, top + 31.0, WIDTH - PAD - sx, sub_t.color(t::ACCENT));
            }
        }
        // The bottom: the chapter's progress as a thin line, the chapters around it, auto-scroll,
        // then the dimmer.
        let by = cx.c.h - bottom_bar;
        cx.c.rect(-1000.0, by, WIDTH + 2000.0, bottom_bar, 0.0, t::GLASS);
        let f = pages.map(|n| (page + 1) as f32 / n.max(1) as f32).unwrap_or(0.0);
        cx.c.rect(0.0, by, WIDTH, 2.0, 0.0, t::INK.a(0.12));
        cx.c.rect(0.0, by, WIDTH * f, 2.0, 0.0, t::ACCENT);
        // The saved place, a small mark on the track.
        if let (Some((Some(s), _)), Some(n)) = (&saved, pages) {
            let mx = (WIDTH * (s.page + 1) as f32 / n.max(1) as f32).clamp(2.0, WIDTH - 2.0);
            cx.c.rect(mx - 1.5, by - 3.0, 3.0, 8.0, 1.5, t::INK);
        }
        let ky = by + 10.0;
        if r.local.is_none() {
            // − play +, in the middle.
            let mid = WIDTH / 2.0;
            if kit::icon_key_ink(&mut cx, mid - 22.0 - 48.0, ky, "minus", if r.speed > 1 { t::INK } else { t::INK3 }) && r.speed > 1 {
                speed = Some(r.speed - 1);
            }
            let pressed = cx.pressed(mid - 24.0, ky - 2.0, 48.0, 48.0);
            cx.c.circle(mid, ky + 22.0, 23.0, if r.auto { t::ACCENT } else if pressed { t::S3 } else { t::S2 });
            cx.c.icon(if r.auto { "pause" } else { "play" }, mid + if r.auto { 0.0 } else { 1.5 }, ky + 22.0, 18.0, if r.auto { t::ON_ACCENT } else { t::INK });
            if cx.tap(mid - 24.0, ky - 2.0, 48.0, 48.0) {
                r.auto = !r.auto;
                r.waiting = false;
                if r.auto {
                    r.overlay = false;
                }
            }
            if kit::icon_key_ink(&mut cx, mid + 22.0 + 4.0, ky, "plus", if r.speed < logic::SPEEDS { t::INK } else { t::INK3 }) && r.speed < logic::SPEEDS {
                speed = Some(r.speed + 1);
            }
        }
        // The dimmer: darker to the left, past what the phone's own brightness goes down to.
        let sy = by + 64.0 + 22.0;
        let lt = Text::new(ty::CAPTION, t::INK2);
        cx.c.text("Brightness", PAD, sy - lt.lh() / 2.0, lt);
        let (sx, sw) = (PAD + 92.0, WIDTH - 2.0 * PAD - 92.0 - 10.0);
        let bright = 1.0 - app.prefs.dim / crate::app::DIM_MAX;
        if let Some(v) = kit::slider(&mut cx, sx, sy, sw, bright) {
            dim = Some((1.0 - v) * crate::app::DIM_MAX);
        }
        if ci > 0 {
            let words = format!("Ch. {}", logic::num(r.chapters[ci - 1].number));
            if chapter_key(&mut cx, PAD - 6.0, ky, &words, "chevron-left", false) {
                jump = Some(ci - 1);
            }
        }
        if ci + 1 < r.chapters.len() {
            let words = format!("Ch. {}", logic::num(r.chapters[ci + 1].number));
            if chapter_key(&mut cx, WIDTH - PAD + 6.0, ky, &words, "chevron-right", true) {
                jump = Some(ci + 1);
            }
        }
        // A tap elsewhere on the bars is theirs; any other puts the overlay away (not the touch
        // that stopped a fling: `tapped` knows).
        let on_bar = cx.ui.tapped(-1e5, -1e5, 2e5, top_bar + 1e5) || cx.ui.tapped(-1e5, view_h - bottom_bar, 2e5, 1e5);
        if !on_bar && cx.ui.tapped(-1e5, -1e5, 2e5, 2e5) {
            r.overlay = false;
        }
    } else if app.ui.tap.is_some() && app.ui.tapped(-1e5, -1e5, 2e5, 2e5) {
        if r.auto {
            // A tap stops auto-scroll, and shows the overlay to start it again.
            r.auto = false;
        }
        r.overlay = true;
    }
    if let Some(l) = speed {
        r.speed = l;
        app.set_speed(r.series, l);
        app.toast(&format!("Auto-scroll speed {l} of {}", logic::SPEEDS));
    }
    if let Some(d) = dim {
        app.prefs.dim = if d < 0.02 { 0.0 } else { d };
        app.dim_dirty = true;
    }
    if retry {
        app.images.retry_failed();
    }
    if let Some(c) = jump {
        r.pos = Pos { item: Item::Head(c), off: 0.0 };
        // A fling still coasting would carry the new chapter past its heading.
        r.fling.stop();
        r.overlay = false;
    }
    moving |= app.images.pages_pending() && !crate::wake::has();
    // Undo, for a few seconds after a page was hidden.
    if r.undo.is_some() {
        let (x, y, w, h) = undo_box(&app.canvas, r.overlay);
        let c = &mut app.canvas;
        c.rect(x, y, w, h, 12.0, t::S3);
        let tt = Text::new(ty::UI, t::INK).medium();
        c.text_fit("Page hidden, and ones like it", x + 16.0, y + (h - tt.lh()) / 2.0, w - 100.0, tt);
        let kt = Text::new(ty::UI, t::ACCENT).semi().right();
        c.text("Undo", x + w - 16.0, y + (h - kt.lh()) / 2.0, kt);
    }
    // The sheet after a long press: Hide, or Cancel (a tap outside closes it too).
    if let Some((c, n)) = r.hide_sheet {
        let mut cx = cx!(app);
        kit::release(&mut cx, held);
        let id = r.chapters[c].id;
        let words = format!("Page {} of chapter {}", n + 1, logic::num(r.chapters[c].number));
        match kit::choices(&mut cx, &words, Some("Scanlators' \u{201C}join our Discord\u{201D} pages and the like"), &[("Hide this page and ones like it", false), ("Cancel", false)]) {
            Some(0) => {
                r.hide_sheet = None;
                page_act = Some(PageAct::Hide(id, n));
            }
            Some(_) => r.hide_sheet = None,
            None => {}
        }
    }
    let leave = if to_series {
        Leave::ToSeries
    } else if back {
        Leave::Back
    } else {
        Leave::Stay
    };
    (moving, leave, page_act)
}

/// The overlay's bottom bar, above the gesture bar: the chapter keys and auto-scroll, then the
/// dimmer.
const BAR_H: f32 = 64.0 + 48.0 + 14.0;
/// A drag that starts this close to the top or bottom edge (or within the phone's bars, if
/// taller) is the phone's gesture, not a scroll of the strip.
const EDGE_ZONE: f32 = 28.0;

/// What auto-scroll finds from the screen's top to a little past its bottom.
#[derive(Debug, PartialEq)]
enum Ahead {
    /// Every page there is loaded and every chapter knows its length.
    Ready,
    Loading,
    /// A page there would not load.
    Failed,
}

/// The bookmark's mark at `y`: a line across in the accent, and a tag hanging from it at the
/// left, "BOOKMARK" (flat where it meets the line, rounded below).
fn bookmark_mark(c: &mut crate::draw::Canvas, y: f32) {
    c.rect(0.0, y - 1.0, WIDTH, 2.0, 0.0, t::ACCENT);
    let tt = Text::new(11.0, t::ON_ACCENT).semi();
    let words = "BOOKMARK";
    let (x, top, h) = (PAD, y, 24.0);
    let w = c.measure(words, tt) + 32.0;
    c.rect(x, top, w, h, 8.0, t::ACCENT);
    c.rect(x, top, w, h / 2.0, 0.0, t::ACCENT);
    c.icon("bookmark", x + 14.0, top + h / 2.0, 12.0, t::ON_ACCENT);
    c.text(words, x + 24.0, top + (h - tt.lh()) / 2.0, tt);
}

fn ahead(v: &View, pos: Pos, view_h: f32) -> Ahead {
    let mut out = Ahead::Ready;
    for (item, _) in strip::visible(v, pos, view_h, view_h * AHEAD) {
        match item {
            // A chapter whose length would not come: stop at it, as at a page that would not load.
            Item::Head(c) if v.pages(c).is_none() && v.r.count_failed.contains_key(&c) => return Ahead::Failed,
            Item::Head(c) if v.pages(c).is_none() => out = Ahead::Loading,
            Item::Page(c, n) => {
                let id = v.r.chapters[c].id;
                if v.im.size(id, n).is_some() || v.im.skipped(id, n) {
                    // Measured: it has loaded once (a later fetch failing is the drawing's business).
                } else if v.im.failed(id, n).is_some() {
                    return Ahead::Failed;
                } else {
                    out = Ahead::Loading;
                }
            }
            _ => {}
        }
    }
    out
}

/// A key for the chapter before or after: its glyph and its number. `right` anchors it at `x`'s
/// right instead of its left.
fn chapter_key(cx: &mut kit::Ctx, x: f32, y: f32, words: &str, glyph: &'static str, right: bool) -> bool {
    let tt = Text::new(ty::UI, t::INK).medium();
    let w = cx.c.measure(words, tt) + 44.0;
    let h = 44.0;
    let kx = if right { x - w } else { x };
    if cx.pressed(kx, y, w, h) {
        cx.c.rect(kx, y, w, h, t::KEY_R, t::WASH);
    }
    if right {
        cx.c.text(words, kx + 12.0, y + (h - tt.lh()) / 2.0, tt);
        cx.c.icon(glyph, kx + w - 18.0, y + h / 2.0, 16.0, t::INK);
    } else {
        cx.c.icon(glyph, kx + 18.0, y + h / 2.0, 16.0, t::INK);
        cx.c.text(words, kx + 32.0, y + (h - tt.lh()) / 2.0, tt);
    }
    cx.tap(kx, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(chapters: usize, pages: u32) -> SeriesDetail {
        let c = |i: usize| Chapter { id: 100 + i as i64, number: i as f64 + 1.0, name: String::new(), group: None, pages, read: false, last_page: 0, downloaded: true, uploaded_at: 0, from: None };
        SeriesDetail {
            series: leaf_proto::Series {
                id: 1,
                title: "T".into(),
                author: None,
                source: "S".into(),
                status: "ongoing".into(),
                chapters: chapters as u32,
                read_upto: None,
                unread: chapters as u32,
                latest: Some(chapters as f64),
                gaps: vec![],
                updated_at: 0,
                note: None,
                bell: false,
                added_at: 0,
                last_read_at: 0,
                shelf: None,
            },
            description: None,
            chapters: (0..chapters).map(c).collect(),
        }
    }

    #[test]
    fn auto_scroll_waits_for_the_page_ahead() {
        let dir = std::env::temp_dir().join(format!("leaf-ahead-{}", std::process::id()));
        let mut im = Images::new(dir.clone());
        let r = Reader::open(&detail(2, 3), 0, 0);
        // Nothing measured yet: wait.
        assert_eq!(ahead(&View { r: &r, im: &im }, r.pos, 800.0), Ahead::Loading);
        // The first two pages fill the screen and 0.6 of one below it: ready, though the third
        // is still on its way.
        im.fake_size(100, 0, 800, 1600);
        im.fake_size(100, 1, 800, 1600);
        assert_eq!(ahead(&View { r: &r, im: &im }, r.pos, 800.0), Ahead::Ready);
        // Further down, the third is needed.
        let p = Pos { item: Item::Page(0, 1), off: 300.0 };
        assert_eq!(ahead(&View { r: &r, im: &im }, p, 800.0), Ahead::Loading);
        im.fake_size(100, 2, 800, 1600);
        // The next chapter's heading is there, and it knows its length (3): its first page isn't.
        let p = Pos { item: Item::Page(0, 2), off: 500.0 };
        assert_eq!(ahead(&View { r: &r, im: &im }, p, 800.0), Ahead::Loading);
        let _ = std::fs::remove_dir_all(dir);
    }
}
