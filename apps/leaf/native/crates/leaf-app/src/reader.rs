//! The reader: every chapter of the series as one strip, full bleed, scrolled with momentum. The
//! end of a chapter flows into the next past a slim heading. A tap shows or hides a small overlay
//! (Back, the series and chapter, a thin progress line, the chapters before and after).
//!
//! The place is a strip item and an offset within it ([`crate::strip`]), so a page above the
//! screen whose height arrives late never moves what is on it. Pages are fetched and decoded on
//! worker threads ([`crate::images`]); each frame says which it wants, nearest first, and which
//! to keep on disk ahead (the rest of this chapter and the whole next one). The place goes to the
//! server as the person reads ([`crate::logic::ProgressGate`]).

use std::collections::HashSet;

use leaf_proto::{Chapter, Progress, SeriesDetail};

use crate::app::App;
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::images::{Images, tile_h, tile_key};
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

pub struct Reader {
    pub series: i64,
    pub title: String,
    pub chapters: Vec<Chapter>,
    /// Page counts as the server gave them, or none while unknown.
    counts: Vec<Option<u32>>,
    counts_asked: HashSet<usize>,
    pub pos: Pos,
    /// Drives the momentum: its offset is reset each frame, the change is the strip's.
    fling: Scroll,
    pub overlay: bool,
    gate: ProgressGate,
    /// The status bar's height (with the bars hidden, the camera cut-out's): headings keep clear of it.
    top_pad: f32,
    /// A PDF from the shelf (its book id), not a series: pages come from the platform's renderer
    /// and the place is kept on the phone, never sent to the server.
    pub local: Option<i64>,
}

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
            pos: Pos { item, off: 0.0 },
            fling: fling(),
            overlay: false,
            gate,
            top_pad: 0.0,
            local: None,
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
            pos: Pos { item, off: 0.0 },
            fling: fling(),
            overlay: false,
            gate: ProgressGate::default(),
            top_pad: 0.0,
            local: Some(b.id),
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

    pub fn page_count(&mut self, chapter: i64, count: Option<u32>) {
        if let Some(i) = self.chapters.iter().position(|c| c.id == chapter) {
            match count {
                Some(n) => {
                    self.counts[i] = Some(n);
                    // Opened at a page before the length was known: kept inside the chapter.
                    if let Item::Page(c, p) = self.pos.item
                        && c == i
                        && p >= n
                    {
                        self.pos = Pos { item: if n > 0 { Item::Page(c, n - 1) } else { Item::Head(c) }, off: 0.0 };
                    }
                }
                // Asked again the next time it comes near.
                None => {
                    self.counts_asked.remove(&i);
                }
            }
        }
    }

    pub fn scroll_by(&mut self, im: &Images, dy: f32, view_h: f32) {
        let v = View { r: self, im };
        let p = strip::scroll(&v, self.pos, dy, view_h);
        self.pos = p;
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
    let (moving, leave) = frame(app, &mut r, dt);
    app.reader = Some(r);
    if leave {
        app.pop();
    }
    moving
}

/// One frame of the reader. Returns (still moving, leave).
fn frame(app: &mut App, r: &mut Reader, dt: f32) -> (bool, bool) {
    let view_h = app.canvas.h;
    let now = app.now_ns;
    r.top_pad = app.canvas.inset_top;
    // The overlay's keys take their taps before the strip does.
    let overlay_tap = r.overlay;
    // Scroll: the finger, then the momentum it left.
    let before = r.fling.off;
    let coasting = r.fling.update(&app.ui, [0.0, 0.0, WIDTH, view_h], dt);
    let dy = r.fling.off - before;
    r.fling.off = FLING_MID;
    let mut moving = coasting;
    if dy != 0.0 {
        let v = View { r, im: &app.images };
        let p = strip::scroll(&v, r.pos, dy, view_h);
        let stuck = p == r.pos;
        r.pos = p;
        if stuck {
            r.fling.stop();
        }
    }
    // Ask for page counts the strip needs (a chapter the server listed without them).
    let near_chapter = {
        let v = View { r, im: &app.images };
        strip::chapter_of(&v, r.pos.item)
    };
    for c in [near_chapter, near_chapter + 1] {
        if r.local.is_none() && c < r.chapters.len() && r.counts[c].is_none() && r.counts_asked.insert(c) {
            app.ask(Job::PageCount(r.chapters[c].id));
        }
    }
    // What to fetch: what is on screen and two screens below first, then one above; then, only
    // to disk, the rest of this chapter and the whole next one.
    let (shown, wants, keep) = {
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
        (shown, wants, keep)
    };
    app.images.want_pages(&wants);
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
                        c.text(&format!("Opening chapter {}", logic::num(ch.number)), WIDTH / 2.0, y + HEAD_H + LOADING_H / 2.0 - 10.0, Text::new(ty::UI, t::INK3).center());
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
                            let key = tile_key(id, n, i);
                            let th = tile_h(ph, i);
                            if !c.has_image(&key)
                                && let Some(img) = v_im.page(id, n)
                                && let Some(px) = img.tiles.get(i)
                            {
                                c.add_image_slot(&key, pw, th, (PAGE_W, TILE_H), px.clone());
                            }
                            let top = y + (i as u32 * TILE_H) as f32 * scale;
                            let bottom = y + ((i as u32 * TILE_H) + th) as f32 * scale;
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
    // Where the reader is, and what is finished.
    let (ci, page) = {
        let v = View { r, im: &app.images };
        r.place(&v)
    };
    if !app.shot && !r.chapters.is_empty() && r.local.is_none() {
        let mut reports = r.gate.see(r.chapters[ci].id, page, false, now);
        // A chapter whose last page's bottom is on screen is finished.
        for (item, y) in &shown {
            if let Item::Page(c, n) = *item
                && r.counts[c].is_some_and(|p| n + 1 == p)
            {
                let h = View { r, im: &app.images }.height(*item);
                if y + h <= view_h + 1.0 && app.images.size(r.chapters[c].id, n).is_some() {
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
            app.jobs.send(app.server.as_ref(), Job::Progress(chapter, p));
        }
        if let Some(at) = r.gate.due_at() {
            app.wake_at(at);
        }
    }
    // The overlay, or the tap that brings it.
    let mut back = false;
    let mut jump: Option<usize> = None;
    if r.overlay {
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
        cx.c.text_fit(&sub, 56.0, top + 31.0, WIDTH - 56.0 - PAD, Text::new(ty::CAPTION, t::INK2));
        // The bottom: the chapter's progress as a thin line, the chapters around it.
        let bar_h = 64.0 + cx.c.inset_bottom;
        let by = cx.c.h - bar_h;
        cx.c.rect(-1000.0, by, WIDTH + 2000.0, bar_h, 0.0, t::GLASS);
        let f = pages.map(|n| (page + 1) as f32 / n.max(1) as f32).unwrap_or(0.0);
        cx.c.rect(0.0, by, WIDTH, 2.0, 0.0, t::INK.a(0.12));
        cx.c.rect(0.0, by, WIDTH * f, 2.0, 0.0, t::ACCENT);
        let ky = by + 10.0;
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
        // Any other tap puts it away (not the touch that stopped a fling: `tapped` knows).
        if cx.ui.tapped(-1e5, -1e5, 2e5, 2e5) {
            r.overlay = false;
        }
    } else if app.ui.tapped(-1e5, -1e5, 2e5, 2e5) {
        r.overlay = true;
    }
    if retry {
        app.images.retry_failed();
    }
    if let Some(c) = jump {
        r.pos = Pos { item: Item::Head(c), off: 0.0 };
        r.fling.stop();
        r.overlay = false;
    }
    moving |= app.images.pages_pending() && !crate::wake::has();
    (moving, back || to_series)
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
