//! The book reader: an EPUB or text file as one continuous column, scrolled like the comics strip
//! (`crate::strip`, with a section for a chapter and a block for a page). The place is (section,
//! block, offset in the block), so text set again at another size never jumps, and what is above
//! the screen can change height without moving what is on it.
//!
//! The book is read on a worker thread when it opens; blocks are set in lines as they come near
//! the screen ([`crate::typeset`]) and kept. A tap shows the controls (contents, type, search,
//! notes); a long press selects a word and a drag extends it, for a highlight in one of four named
//! colours or a note. A link inside the book opens what it points to in place (a footnote) rather
//! than leaving the page. Reading time counts only while the page moves, and the reader's own
//! speed in this book gives the time left in the chapter.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Canvas, Rgba, Text, WIDTH, Weight};
use crate::epub::{Block, Doc, Kind, LINK, SUP, Target};
use crate::images::Images;
use crate::kit;
use crate::shelf::{self, COLOURS, Highlight, Place};
use crate::strip::{self, Item, Pos};
use crate::theme::{self as t, PAD, ty};
use crate::typeset::{self, Laid, Type};
use crate::ui::Scroll;

/// A long press, in ns.
const LONG_PRESS: u64 = 450_000_000;
const END_H: f32 = 280.0;
/// Characters a minute until the reader's own speed is known (about 200 words).
const DEFAULT_CPM: f32 = 1000.0;
/// A pause longer than this between moves is not counted as reading.
const IDLE_S: f64 = 90.0;

/// A reading theme: ground, ink, quiet ink, links.
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Rgba,
    pub ink: Rgba,
    pub dim: Rgba,
    pub link: Rgba,
    pub dark: bool,
}

pub const THEMES: [(&str, Theme); 4] = [
    ("Paper", Theme { bg: Rgba::hex(0xF4F1EA), ink: Rgba::hex(0x1F1D1A), dim: Rgba::hex(0x6B655C), link: Rgba::hex(0x1E7A4C), dark: false }),
    ("Sepia", Theme { bg: Rgba::hex(0xF0E4CC), ink: Rgba::hex(0x3A2E22), dim: Rgba::hex(0x76664F), link: Rgba::hex(0x2B6B47), dark: false }),
    ("Dark", Theme { bg: Rgba::hex(0x121213), ink: Rgba::hex(0xE4E0D8), dim: Rgba::hex(0x8E8A82), link: Rgba::hex(0x3FDD8C), dark: true }),
    ("OLED", Theme { bg: Rgba::hex(0x000000), ink: Rgba::hex(0xCFCBC3), dim: Rgba::hex(0x7E7A73), link: Rgba::hex(0x3FDD8C), dark: true }),
];

pub fn theme(ty_: &Type) -> Theme {
    THEMES[ty_.theme.min(3) as usize].1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sheet {
    None,
    Type,
    Contents,
    Note,
}

/// A selection in one section, from `a` to `b` (block, byte), `a` first.
#[derive(Clone, Debug)]
pub struct Sel {
    pub s: u32,
    pub a: (u32, u32),
    pub b: (u32, u32),
    /// Where the long press began: the drag extends from it.
    anchor: (u32, u32),
    /// The finger that made it is still down and dragging its end.
    pub selecting: bool,
    /// A handle being dragged: false the start, true the end.
    handle: Option<bool>,
    /// The highlight this is (tapped), not a new selection.
    pub existing: Option<Highlight>,
}

/// Time read and characters passed, counted while the page moves.
#[derive(Default)]
struct ReadClock {
    last_ns: u64,
    last_char: u64,
    secs: f64,
    chars: f64,
}

impl ReadClock {
    /// The reader stopped (closed, or the app went to the background): the gap up to the next
    /// move is not counted, however short.
    fn pause(&mut self) {
        self.last_ns = 0;
    }

    fn moved(&mut self, now: u64, at_char: u64) {
        if self.last_ns > 0 {
            let gap = now.saturating_sub(self.last_ns) as f64 / 1e9;
            if gap <= IDLE_S {
                self.secs += gap;
                if at_char > self.last_char {
                    // Skimming far ahead (a jump) is not reading.
                    let d = (at_char - self.last_char) as f64;
                    if d < 6000.0 {
                        self.chars += d;
                    }
                }
            }
        }
        self.last_ns = now;
        self.last_char = at_char;
    }
}

/// Search inside the book.
#[derive(Default)]
pub struct Found {
    pub query: String,
    pub hits: Vec<Hit>,
    pub searching: bool,
    rx: Option<Receiver<(String, Vec<Hit>)>>,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub s: u32,
    pub b: u32,
    pub byte: u32,
    /// The words around it, and where the match is inside them.
    pub context: String,
    pub at: (usize, usize),
}

pub struct BookReader {
    pub id: i64,
    pub title: String,
    pub path: PathBuf,
    pub doc: Option<Arc<Doc>>,
    rx: Option<Receiver<Result<Doc, String>>>,
    pub error: Option<String>,
    pub pos: Pos,
    /// Where to open once the book is read.
    restore: Option<Place>,
    fling: Scroll,
    cache: HashMap<(u32, u32), Laid>,
    cache_key: (u32, u8, u8, u32),
    pub ty: Type,
    pub chrome: bool,
    pub sheet: Sheet,
    sheet_scroll: Scroll,
    sheet_fresh: bool,
    pub sel: Option<Sel>,
    /// A footnote open in place: its (section, block).
    pub popover: Option<(u32, u32)>,
    /// Where the reader was before a jump (contents, search, a note): one tap back.
    pub back_to: Option<Pos>,
    clock: ReadClock,
    top_pad: f32,
    saved: Option<Place>,
    save_at: u64,
    pub note: String,
    pub found: Found,
    /// The highlight a note is being written for.
    note_for: Option<Highlight>,
    confirm_remove: bool,
    more: bool,
}

/// The book as the strip sees it.
struct View<'a> {
    doc: &'a Doc,
    cache: RefCell<&'a mut HashMap<(u32, u32), Laid>>,
    c: RefCell<&'a mut Canvas>,
    im: &'a Images,
    ty: Type,
    top_pad: f32,
    book: i64,
}

pub fn image_key(book: i64, s: u32, b: u32) -> String {
    format!("i{book}:{s}:{b}")
}

impl View<'_> {
    fn laid(&self, s: u32, b: u32) -> Laid {
        if let Some(l) = self.cache.borrow().get(&(s, b)) {
            return l.clone();
        }
        let Some(block) = self.doc.block(s, b) else { return Laid::default() };
        let image = if matches!(block.kind, Kind::Image(..)) { self.im.local_size(&image_key(self.book, s, b)) } else { None };
        let l = typeset::lay(&mut self.c.borrow_mut(), block, &self.ty, image);
        // A picture's size may still be on its way: set it again then.
        if !matches!(block.kind, Kind::Image(..)) || image.is_some() {
            self.cache.borrow_mut().insert((s, b), l.clone());
        }
        l
    }
}

impl strip::Book for View<'_> {
    fn chapters(&self) -> usize {
        self.doc.sections.len()
    }
    fn pages(&self, c: usize) -> Option<u32> {
        Some(self.doc.sections[c].blocks.len() as u32)
    }
    fn height(&self, item: Item) -> f32 {
        match item {
            Item::Head(0) => self.top_pad + 20.0,
            Item::Head(_) => 56.0,
            Item::Page(s, b) => self.laid(s as u32, b).h,
            Item::End => END_H,
        }
    }
}

/// Small enough that f32 keeps fractions of a unit.
const FLING_MID: f32 = 4096.0;

fn fling() -> Scroll {
    let mut s = Scroll::default();
    s.off = FLING_MID;
    s.max = 2.0 * FLING_MID;
    s
}

impl BookReader {
    /// Open a book (its text is read on a thread of its own), at its saved place.
    pub fn open(book: &shelf::Book, data: &std::path::Path, ty_: Type, doc: Option<Arc<Doc>>) -> BookReader {
        let path = book.path(data);
        let rx = if doc.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let p = path.clone();
            let _ = std::thread::Builder::new().name("leaf-book".into()).spawn(move || {
                let got = std::panic::catch_unwind(|| crate::epub::open(&p)).unwrap_or_else(|_| Err("Leaf couldn't read this book".into()));
                let _ = tx.send(got);
                crate::wake::wake();
            });
            Some(rx)
        } else {
            None
        };
        BookReader {
            id: book.id,
            title: book.title.clone(),
            path,
            doc,
            rx,
            error: None,
            pos: Pos { item: Item::Head(0), off: 0.0 },
            restore: Some(book.place),
            fling: fling(),
            cache: HashMap::new(),
            cache_key: (0, 0, 0, 0),
            ty: ty_,
            chrome: false,
            sheet: Sheet::None,
            sheet_scroll: Scroll::default(),
            sheet_fresh: false,
            sel: None,
            popover: None,
            back_to: None,
            clock: ReadClock::default(),
            top_pad: 0.0,
            saved: Some(book.place),
            save_at: 0,
            note: String::new(),
            found: Found::default(),
            note_for: None,
            confirm_remove: false,
            more: false,
        }
    }

    fn view<'a>(&'a mut self, c: &'a mut Canvas, im: &'a Images) -> Option<View<'a>> {
        let doc = self.doc.as_deref()?;
        Some(View { doc, cache: RefCell::new(&mut self.cache), c: RefCell::new(c), im, ty: self.ty, top_pad: self.top_pad, book: self.id })
    }

    /// The mouse wheel or a screenshot's scroll.
    pub fn scroll_by(&mut self, c: &mut Canvas, im: &Images, dy: f32, view_h: f32) {
        let pos = self.pos;
        if let Some(v) = self.view(c, im) {
            let p = strip::scroll(&v, pos, dy, view_h);
            self.pos = p;
        }
    }

    /// The place the top of the screen shows.
    pub fn place(&mut self, c: &mut Canvas, im: &Images) -> Place {
        let pos = self.pos;
        let Some(v) = self.view(c, im) else { return self.saved.unwrap_or_default() };
        match pos.item {
            Item::Head(s) => Place { section: s as u32, block: 0, byte: 0 },
            Item::Page(s, b) => {
                let l = v.laid(s as u32, b);
                let byte = l.line_at(pos.off).map_or(0, |line| line.start);
                Place { section: s as u32, block: b, byte }
            }
            Item::End => {
                let s = v.doc.sections.len().saturating_sub(1);
                Place { section: s as u32, block: v.doc.sections[s].blocks.len().saturating_sub(1) as u32, byte: 0 }
            }
        }
    }

    /// Go to a place: its line at the top of the screen.
    pub fn go(&mut self, c: &mut Canvas, im: &Images, p: Place, view_h: f32) {
        let Some(v) = self.view(c, im) else { return };
        let s = (p.section as usize).min(v.doc.sections.len().saturating_sub(1));
        let blocks = v.doc.sections[s].blocks.len() as u32;
        if blocks == 0 {
            self.pos = Pos { item: Item::Head(s), off: 0.0 };
            return;
        }
        let b = p.block.min(blocks - 1);
        let l = v.laid(s as u32, b);
        let off = if p.byte == 0 { 0.0 } else { l.line_of(p.byte).map_or(0.0, |line| line.top) };
        // A place at a section's first block shows the gap above it too.
        let start = if b == 0 && p.byte == 0 { Pos { item: Item::Head(s), off: 0.0 } } else { Pos { item: Item::Page(s, b), off } };
        let pos = strip::scroll(&v, start, 0.0, view_h);
        self.pos = pos;
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let Some(mut r) = app.book.take() else {
        app.pop();
        return true;
    };
    let (moving, leave) = frame(app, &mut r, dt);
    app.book = Some(r);
    if leave {
        app.pop();
    }
    moving
}

/// One frame of the reader. Returns (still moving, leave).
fn frame(app: &mut App, r: &mut BookReader, dt: f32) -> (bool, bool) {
    let view_h = app.canvas.h;
    let now = app.now_ns;
    r.top_pad = app.canvas.inset_top;
    let th = theme(&r.ty);
    app.canvas.rect(-1000.0, 0.0, WIDTH + 2000.0, view_h, 0.0, th.bg);
    // The book arrives.
    if let Some(rx) = &r.rx
        && let Ok(got) = rx.try_recv()
    {
        r.rx = None;
        match got {
            Ok(d) => {
                let d = Arc::new(d);
                app.docs.insert(r.id, d.clone());
                r.doc = Some(d);
            }
            Err(e) => r.error = Some(e),
        }
    }
    let Some(doc) = r.doc.clone() else {
        return waiting(app, r, th);
    };
    // Type set again when the sheet or the window changed it, keeping the place.
    let key = (r.ty.key().0, r.ty.key().1, r.ty.key().2, (app.canvas.u * 1000.0) as u32);
    if key != r.cache_key {
        let first = r.cache_key == (0, 0, 0, 0);
        let keep = if first { None } else { Some(r.place(&mut app.canvas, &app.images)) };
        r.cache.clear();
        r.cache_key = key;
        if let Some(p) = r.restore.take().or(keep) {
            r.go(&mut app.canvas, &app.images, p, view_h);
        }
    }
    let covered = r.sheet != Sheet::None || r.popover.is_some() || r.more || r.confirm_remove;
    // Touch: a long press selects, a drag from a handle moves it, anything else scrolls.
    let mut scrolling_allowed = !covered;
    if !covered {
        if let Some(sel) = &r.sel
            && sel.selecting
        {
            scrolling_allowed = false;
        }
        if let Some((p, held)) = app.ui.resting(now)
            && held >= LONG_PRESS
            && r.sel.as_ref().is_none_or(|s| !s.selecting)
        {
            app.ui.swallow();
            if let Some((s, b, byte)) = hit_point(app, r, &doc, p.0, p.1, view_h) {
                let text = &doc.sections[s as usize].blocks[b as usize].text;
                let (a, z) = typeset::word_at(text, byte);
                r.sel = Some(Sel { s, a: (b, a), b: (b, z), anchor: (b, a), selecting: true, handle: None, existing: None });
                r.chrome = false;
            }
            scrolling_allowed = false;
        }
        // Extending: the finger's place is one end, the press's word the other.
        let finger = app.ui.finger();
        let handles = r.sel.as_ref().map(|s| (s.selecting, s.handle));
        if let Some((selecting, handle)) = handles {
            let mut grab = handle;
            if !selecting
                && grab.is_none()
                && let Some(from) = app.ui.drag_from
                && finger.is_some()
            {
                grab = handle_at(app, r, &doc, from, view_h);
                if let Some(sel) = &mut r.sel {
                    sel.handle = grab;
                }
            }
            if (selecting || grab.is_some())
                && let Some(f) = finger
            {
                scrolling_allowed = false;
                if let Some((s, b, byte)) = hit_point(app, r, &doc, f.0, f.1, view_h)
                    && let Some(sel) = &mut r.sel
                    && s == sel.s
                {
                    let here = (b, byte);
                    if selecting {
                        let text = &doc.sections[s as usize].blocks[b as usize].text;
                        let (wa, wz) = typeset::word_at(text, byte);
                        if here < sel.anchor {
                            sel.a = (b, wa);
                            sel.b = (sel.anchor.0, typeset::word_at(&doc.sections[s as usize].blocks[sel.anchor.0 as usize].text, sel.anchor.1).1);
                        } else {
                            sel.a = sel.anchor;
                            sel.b = (b, wz);
                        }
                    } else if grab == Some(true) {
                        sel.b = here.max(sel.a);
                    } else if grab == Some(false) {
                        sel.a = here.min(sel.b);
                    }
                    sel.existing = None;
                }
            }
            if finger.is_none()
                && let Some(sel) = &mut r.sel
            {
                sel.selecting = false;
                sel.handle = None;
            }
        }
    }
    // Scroll: the finger, then the momentum it left.
    let mut moving = false;
    if scrolling_allowed {
        let before = r.fling.off;
        moving = r.fling.update(&app.ui, [0.0, 0.0, WIDTH, view_h], dt);
        let dy = r.fling.off - before;
        r.fling.off = FLING_MID;
        if dy != 0.0 {
            let pos = r.pos;
            if let Some(v) = r.view(&mut app.canvas, &app.images) {
                let p = strip::scroll(&v, pos, dy, view_h);
                if p == pos {
                    r.fling.stop();
                }
                r.pos = p;
            }
        }
    } else {
        r.fling.stop();
        r.fling.off = FLING_MID;
    }
    // What shows.
    let pos = r.pos;
    let shown = {
        let v = r.view(&mut app.canvas, &app.images).expect("the book is open");
        strip::visible(&v, pos, view_h, 0.0)
    };
    let place = r.place(&mut app.canvas, &app.images);
    let at_char = doc.char_at(place.section, place.block, place.byte);
    // Reading time and speed, only while the page moves.
    if Some(place) != r.saved || moving {
        if !app.shot {
            r.clock.moved(now, at_char);
        }
        if now >= r.save_at {
            save(app, r, &doc, place);
            r.save_at = now + 3_000_000_000;
        } else {
            app.wake_at(r.save_at);
        }
    }
    if r.clock.secs >= 30.0 {
        flush_time(app, r);
    }
    // Pictures near the screen are asked for.
    for (item, _) in &shown {
        if let Item::Page(s, b) = *item
            && let Some(Block { kind: Kind::Image(entry, _), .. }) = doc.block(s as u32, b)
        {
            let key = image_key(r.id, s as u32, b);
            app.images.local(&app.canvas, &key, &r.path, entry);
        }
    }
    // The page.
    let hls: Vec<Highlight> = app.shelf.highlights.iter().filter(|h| h.book == r.id).cloned().collect();
    let mut tapped_end = false;
    for (item, y) in &shown {
        match *item {
            Item::Head(s) if s > 0 => {
                let mid = y + 28.0;
                app.canvas.rect(WIDTH / 2.0 - 16.0, mid, 32.0, 1.5, 0.75, th.dim.a(0.5));
            }
            Item::Page(s, b) => {
                let k = (s as u32, b);
                // Set now if it was not (a picture still on its way is set again each frame).
                let tmp = if r.cache.contains_key(&k) { None } else { r.view(&mut app.canvas, &app.images).map(|v| v.laid(k.0, k.1)) };
                let Some(laid) = tmp.as_ref().or_else(|| r.cache.get(&k)) else { continue };
                let block = &doc.sections[s].blocks[b as usize];
                draw_block(app, r, block, laid, k.0, b, *y, th, &hls);
            }
            Item::End => {
                let c = &mut app.canvas;
                c.text("The end", WIDTH / 2.0, y + 70.0, Text::new(ty::HEADING, th.ink).face(Weight::SerifBold).center());
                c.text_fit(&r.title, WIDTH / 2.0, y + 104.0, WIDTH - 2.0 * PAD, Text::new(ty::UI, th.dim).center());
                let (kw, kh) = (200.0, 44.0);
                let (kx, ky) = (WIDTH / 2.0 - kw / 2.0, y + 144.0);
                c.rect(kx, ky, kw, kh, t::KEY_R, th.ink.a(0.08));
                let kt = Text::new(ty::UI, th.ink).semi().center();
                c.text("Back to your library", WIDTH / 2.0, ky + (kh - kt.lh()) / 2.0, kt);
                if !covered && r.sel.is_none() && app.ui.tapped(kx, ky, kw, kh) {
                    tapped_end = true;
                }
            }
            _ => {}
        }
    }
    // The quiet lines: the chapter at the top, time left and how far at the bottom.
    let (chapter, ch_start, ch_end) = doc.chapter_span(place.section, place.block);
    let cpm = app.shelf.book(r.id).map_or(0.0, |b| b.cpm);
    let left_words = time_left(ch_end.saturating_sub(at_char), if cpm > 0.0 { cpm } else { DEFAULT_CPM });
    let pct = if doc.chars > 0 { (at_char as f32 / doc.chars as f32 * 100.0).round() as u32 } else { 0 };
    let _ = ch_start;
    if !r.chrome {
        let c = &mut app.canvas;
        let top = c.inset_top;
        // Solid behind the words (and the status bar), fading just under them.
        c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 26.0, 0.0, th.bg);
        c.rect_grad(-1000.0, top + 26.0, WIDTH + 2000.0, 18.0, 0.0, th.bg, th.bg.a(0.0));
        if !chapter.is_empty() && !matches!(r.pos.item, Item::Head(0)) || r.pos.off > 40.0 {
            c.text_fit(&chapter, WIDTH / 2.0, top + 6.0, WIDTH - 80.0, Text::new(ty::SMALL, th.dim).center());
        }
        let bh = 30.0 + c.inset_bottom;
        let by = c.h - bh;
        c.rect_grad(-1000.0, by - 18.0, WIDTH + 2000.0, 18.0, 0.0, th.bg.a(0.0), th.bg);
        c.rect(-1000.0, by, WIDTH + 2000.0, bh, 0.0, th.bg);
        let tt = Text::new(ty::SMALL, th.dim);
        c.text(&left_words, PAD + 4.0, by + 6.0, tt);
        c.text(&format!("{pct}%"), WIDTH - PAD - 4.0, by + 6.0, tt.right());
    }
    // One tap back to where a jump came from.
    let mut back_jump = false;
    if let Some(bt) = r.back_to {
        let c = &mut app.canvas;
        let words = "Back to where you were";
        let kt = Text::new(ty::UI, t::INK).medium();
        let w = c.measure(words, kt) + 44.0;
        let (x, y) = ((WIDTH - w) / 2.0, c.h - c.inset_bottom - 92.0);
        c.shadow(x, y, w, 40.0, 20.0, 12.0, Rgba::hex(0x000000).a(0.4));
        c.rect(x, y, w, 40.0, 20.0, t::S3);
        c.icon("undo", x + 20.0, y + 20.0, 15.0, t::INK);
        c.text(words, x + 34.0, y + (40.0 - kt.lh()) / 2.0, kt);
        if !covered && app.ui.tapped(x, y, w, 40.0) {
            back_jump = true;
            r.pos = bt;
        }
        // Far from both, it goes.
        if block_distance(bt.item, r.pos.item) > 400 {
            r.back_to = None;
        }
    }
    if back_jump {
        r.back_to = None;
    }
    // The selection bar (before the page hears the tap: one on the bar is the bar's).
    if !covered && r.sel.as_ref().is_some_and(|s| !s.selecting) {
        selection_bar(app, r, &doc, &shown);
    }
    // Taps on the page: a link, a highlight, or the controls.
    let mut leave = tapped_end;
    if !covered
        && r.sel.as_ref().is_none_or(|s| !s.selecting)
        && let Some((tx, ty_)) = app.ui.tap
    {
        let on_chrome = r.chrome && (ty_ < app.canvas.inset_top + 60.0 || ty_ > app.canvas.h - app.canvas.inset_bottom - 110.0);
        if !on_chrome {
            app.ui.tap = None;
            let hit = hit_point(app, r, &doc, tx, ty_, view_h);
            let mut handled = false;
            if r.sel.is_some() {
                r.sel = None;
                handled = true;
            } else if let Some((s, b, byte)) = hit {
                let block = &doc.sections[s as usize].blocks[b as usize];
                if let Some(l) = block.links.iter().find(|l| byte + 1 >= l.start && byte <= l.end)
                    && let Target::Inside(ts, tb) = l.target
                {
                    r.popover = Some((ts, tb));
                    handled = true;
                } else if let Some(h) = hls.iter().find(|h| inside(h, s, b, byte)) {
                    r.sel = Some(Sel { s, a: (h.from.block, h.from.byte), b: (h.to.block, h.to.byte), anchor: (h.from.block, h.from.byte), selecting: false, handle: None, existing: Some(h.clone()) });
                    handled = true;
                }
            }
            if !handled {
                r.chrome = !r.chrome;
            }
        }
    }
    // The controls.
    if r.chrome && r.sheet == Sheet::None && r.popover.is_none() {
        leave |= chrome(app, r, &doc, &chapter, &left_words, pct, ch_end, at_char);
    }
    if let Some((s, b)) = r.popover {
        popover(app, r, &doc, s, b, view_h);
    }
    match r.sheet {
        Sheet::Type => type_sheet(app, r),
        Sheet::Contents => contents_sheet(app, r, &doc, place, view_h),
        Sheet::Note => note_sheet(app, r),
        Sheet::None => {}
    }
    if r.confirm_remove {
        let mut cx = cx!(app);
        match kit::confirm(&mut cx, "Remove this book?", &format!("{} and its highlights leave Leaf. The file goes too.", r.title), "Remove") {
            Some(true) => {
                r.confirm_remove = false;
                let id = r.id;
                app.shelf.remove_book(id);
                app.docs.remove(&id);
                leave = true;
            }
            Some(false) => r.confirm_remove = false,
            None => {}
        }
    }
    moving |= app.ui.busy();
    (moving, leave)
}

/// For screenshots: a reader set up as a person would leave it.
impl BookReader {
    /// Go to `frac` of the way through the book (by its characters).
    pub fn shot_at(&mut self, c: &mut Canvas, im: &Images, frac: f32, view_h: f32) {
        let Some(doc) = self.doc.clone() else { return };
        let target = (doc.chars as f32 * frac) as u64;
        for (s, sec) in doc.sections.iter().enumerate() {
            for (b, bl) in sec.blocks.iter().enumerate() {
                if bl.at + bl.chars() > target && !bl.text.is_empty() {
                    self.go(c, im, Place { section: s as u32, block: b as u32, byte: 0 }, view_h);
                    return;
                }
            }
        }
    }

    /// Select the first sentence of the first long paragraph on screen.
    pub fn shot_select(&mut self, c: &mut Canvas, im: &Images, view_h: f32) {
        let Some(doc) = self.doc.clone() else { return };
        let pos = self.pos;
        let shown = match self.view(c, im) {
            Some(v) => strip::visible(&v, pos, view_h, 0.0),
            None => return,
        };
        for (item, y) in shown {
            if let Item::Page(s, b) = item
                && y > 160.0
                && let Some(bl) = doc.block(s as u32, b)
                && bl.kind == Kind::Para
                && bl.text.len() > 80
            {
                let from = bl.text.char_indices().map(|(i, _)| i).find(|i| *i >= 40).unwrap_or(bl.text.len());
                let end = bl.text[from..].find(". ").map_or(bl.text.floor_char_boundary(120), |i| from + i + 1);
                self.sel = Some(Sel { s: s as u32, a: (b, 0), b: (b, end as u32), anchor: (b, 0), selecting: false, handle: None, existing: None });
                return;
            }
        }
    }

    /// Open the first footnote link after the place in a popover; false when the book has none.
    pub fn shot_popover(&mut self, c: &mut Canvas, im: &Images, view_h: f32) -> bool {
        let Some(doc) = self.doc.clone() else { return false };
        for (s, sec) in doc.sections.iter().enumerate() {
            for (b, bl) in sec.blocks.iter().enumerate() {
                for l in &bl.links {
                    if let Target::Inside(ts, tb) = l.target
                        && (ts, tb) != (s as u32, b as u32)
                        && bl.text[l.start as usize..l.end as usize].chars().count() <= 4
                    {
                        self.go(c, im, Place { section: s as u32, block: b as u32, byte: 0 }, view_h);
                        self.popover = Some((ts, tb));
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn open_sheet(&mut self, which: &str) {
        self.sheet = match which {
            "type" => Sheet::Type,
            "contents" => {
                self.sheet_fresh = true;
                Sheet::Contents
            }
            _ => Sheet::None,
        };
    }
}

impl BookReader {
    /// The note being written, as it stands (Back keeps it, as Save does).
    pub fn take_note(&mut self) -> Option<Highlight> {
        if self.sheet != Sheet::Note {
            return None;
        }
        let mut h = self.note_for.take()?;
        h.note = self.note.trim().to_string();
        Some(h)
    }

    /// Back closes what is over the page first. True when something closed.
    pub fn close_over(&mut self, app_type: &mut Type) -> bool {
        if self.sheet == Sheet::Type {
            *app_type = self.ty;
        }
        if self.sheet != Sheet::None || self.popover.is_some() || self.sel.is_some() || self.more || self.confirm_remove {
            self.sheet = Sheet::None;
            self.popover = None;
            self.sel = None;
            self.more = false;
            self.confirm_remove = false;
            return true;
        }
        if self.chrome {
            self.chrome = false;
            return true;
        }
        false
    }
}

/// Opening, or why it didn't.
fn waiting(app: &mut App, r: &mut BookReader, th: Theme) -> (bool, bool) {
    let mut cx = cx!(app);
    let top = cx.c.inset_top;
    let back = kit::icon_key_ink(&mut cx, 6.0, top + 8.0, "arrow-left", th.ink);
    let mid = cx.c.h * 0.4;
    match &r.error {
        Some(e) => {
            cx.c.text("This book didn't open", WIDTH / 2.0, mid, Text::new(ty::SECTION, th.ink).semi().center());
            cx.c.para(e, PAD * 2.0, mid + 30.0, WIDTH - PAD * 4.0, Text::new(ty::UI, th.dim).center().line(1.45), 4);
        }
        None => {
            cx.c.text_fit(&format!("Opening {}", r.title), WIDTH / 2.0, mid, WIDTH - 2.0 * PAD, Text::new(ty::UI, th.dim).center());
        }
    }
    (r.rx.is_some() && !crate::wake::has(), back)
}

/// Save the place (and what the library shows of it).
fn save(app: &mut App, r: &mut BookReader, doc: &Doc, place: Place) {
    r.saved = Some(place);
    if app.shot {
        return;
    }
    let at = doc.char_at(place.section, place.block, place.byte);
    let progress = if doc.chars > 0 { at as f32 / doc.chars as f32 } else { 0.0 };
    let (chapter, ..) = doc.chapter_span(place.section, place.block);
    let now = shelf::now_ms();
    if let Some(b) = app.shelf.book_mut(r.id) {
        b.place = place;
        b.progress = progress;
        b.opened_ms = now;
        b.chapter = chapter.clone();
        b.chars = doc.chars as i64;
    }
    app.shelf.send(shelf::Op::Place { id: r.id, place, progress, opened_ms: now, chapter, chars: doc.chars as i64 });
}

/// Hand the time read so far to the shelf, and learn the reader's speed from it.
pub fn flush_time(app: &mut App, r: &mut BookReader) {
    let secs = r.clock.secs;
    let chars = r.clock.chars;
    r.clock.secs = 0.0;
    r.clock.chars = 0.0;
    if secs < 1.0 || app.shot {
        return;
    }
    let now = shelf::now_ms();
    app.shelf.add_time(r.id, shelf::day_of(now), secs.round() as i64);
    if secs >= 20.0 && chars > 0.0 {
        let sample = (chars / secs * 60.0) as f32;
        // Plausible reading, not skimming or a page left open.
        if (150.0..5000.0).contains(&sample)
            && let Some(b) = app.shelf.book_mut(r.id)
        {
            b.cpm = if b.cpm <= 0.0 { sample } else { b.cpm * 0.8 + sample * 0.2 };
            let cpm = b.cpm;
            app.shelf.send(shelf::Op::Speed { id: r.id, cpm });
        }
    }
}

/// The book closes: its place and time are kept.
pub fn close(app: &mut App, r: &mut BookReader) {
    if let Some(doc) = r.doc.clone() {
        let place = r.place(&mut app.canvas, &app.images);
        save(app, r, &doc, place);
    }
    flush_time(app, r);
    // Time away (in another app, the phone asleep) is not reading: the next move starts afresh.
    r.clock.pause();
}

pub fn time_left(chars: u64, cpm: f32) -> String {
    let mins = (chars as f32 / cpm.max(1.0)).round() as i64;
    if chars == 0 {
        "End of the chapter".into()
    } else if mins < 1 {
        "Under a minute left in chapter".into()
    } else if mins < 60 {
        format!("{mins} min left in chapter")
    } else {
        format!("{} h {} min left in chapter", mins / 60, mins % 60)
    }
}

fn block_distance(a: Item, b: Item) -> u32 {
    let k = |i: Item| match i {
        Item::Head(s) => (s as u32, 0),
        Item::Page(s, b) => (s as u32, b),
        Item::End => (u32::MAX, 0),
    };
    let (a, b) = (k(a), k(b));
    if a.0 != b.0 { u32::MAX } else { a.1.abs_diff(b.1) }
}

fn inside(h: &Highlight, s: u32, b: u32, byte: u32) -> bool {
    h.from.section == s && (h.from.block, h.from.byte) <= (b, byte) && (b, byte) < (h.to.block, h.to.byte)
}

/// The (section, block, byte) under a point on screen.
fn hit_point(app: &mut App, r: &mut BookReader, doc: &Doc, x: f32, y: f32, view_h: f32) -> Option<(u32, u32, u32)> {
    let pos = r.pos;
    let shown = {
        let v = r.view(&mut app.canvas, &app.images)?;
        strip::visible(&v, pos, view_h, 0.0)
    };
    // The block under the point, else the nearest text block above it.
    let mut best: Option<(usize, u32, f32)> = None;
    for (item, iy) in &shown {
        if let Item::Page(s, b) = *item
            && *iy <= y
        {
            let block = &doc.sections[s].blocks[b as usize];
            if !block.text.is_empty() {
                best = Some((s, b, *iy));
            }
        }
    }
    let (s, b, iy) = best?;
    let laid = r.view(&mut app.canvas, &app.images)?.laid(s as u32, b);
    let text = &doc.sections[s].blocks[b as usize].text;
    let byte = laid.hit(&mut app.canvas, text, x - laid.left, y - iy);
    Some((s as u32, b, byte))
}

/// Which handle of the selection is near a point: Some(false) the start, Some(true) the end.
fn handle_at(app: &mut App, r: &mut BookReader, doc: &Doc, p: (f32, f32), view_h: f32) -> Option<bool> {
    let sel = r.sel.clone()?;
    let ends = [(false, sel.a), (true, sel.b)];
    for (which, (b, byte)) in ends {
        if let Some((x, y)) = screen_of(app, r, doc, sel.s, b, byte, view_h)
            && (p.0 - x).abs() < 28.0
            && (p.1 - y).abs() < 36.0
        {
            return Some(which);
        }
    }
    None
}

/// Where a byte is on screen: its x, and the middle of its line.
fn screen_of(app: &mut App, r: &mut BookReader, doc: &Doc, s: u32, b: u32, byte: u32, view_h: f32) -> Option<(f32, f32)> {
    let pos = r.pos;
    let shown = {
        let v = r.view(&mut app.canvas, &app.images)?;
        strip::visible(&v, pos, view_h, 0.0)
    };
    let (_, iy) = shown.iter().find(|(i, _)| *i == Item::Page(s as usize, b))?;
    let laid = r.view(&mut app.canvas, &app.images)?.laid(s, b);
    let text = &doc.sections[s as usize].blocks[b as usize].text;
    let line = laid.line_of(byte)?;
    let x = laid.left + laid.x_of(&mut app.canvas, text, byte);
    Some((x, iy + line.top + line.h / 2.0))
}

/// Draw one block at `y`: its highlights and the selection behind, then its words.
#[allow(clippy::too_many_arguments)]
fn draw_block(app: &mut App, r: &BookReader, block: &Block, laid: &Laid, s: u32, b: u32, y: f32, th: Theme, hls: &[Highlight]) {
    let c = &mut app.canvas;
    let left = laid.left;
    match &block.kind {
        Kind::Image(_, alt) => {
            if let Some([x, iy, w, h]) = laid.image {
                let key = image_key(r.id, s, b);
                if !c.image_cover(&key, x, y + iy, w, h, 4.0, Rgba::WHITE) {
                    c.rect(x, y + iy, w, h, 4.0, th.ink.a(0.06));
                    if app.images.local_failed(&key) && !alt.is_empty() {
                        c.text_fit(alt, WIDTH / 2.0, y + iy + h / 2.0 - 8.0, w - 24.0, Text::new(ty::CAPTION, th.dim).center());
                    }
                }
            }
            return;
        }
        Kind::Rule => {
            c.rect(WIDTH / 2.0 - 24.0, y + laid.h / 2.0, 48.0, 1.5, 0.75, th.dim.a(0.6));
            return;
        }
        Kind::Quote => {
            if let (Some(f), Some(l)) = (laid.lines.first(), laid.lines.last()) {
                c.rect(left - 14.0, y + f.top + 4.0, 2.5, l.top + l.h - f.top - 8.0, 1.25, th.dim.a(0.45));
            }
        }
        Kind::Item(_, mark) => {
            if let Some(f) = laid.lines.first() {
                let run = f.runs.first();
                let size = run.map_or(r.ty.size, |r| r.size);
                c.text(mark, left - 8.0, y + f.top, Text::new(size, th.dim).face(Weight::Serif).right().line(f.h / size));
            }
        }
        _ => {}
    }
    // Highlights, then the selection, behind the words.
    let mut ranges: Vec<(u32, u32, Rgba)> = Vec::new();
    for h in hls {
        if h.from.section != s || (h.from.block, h.from.byte) > (b, u32::MAX) || (h.to.block, h.to.byte) <= (b, 0) {
            continue;
        }
        if h.to.block < b || h.from.block > b {
            continue;
        }
        let a = if h.from.block == b { h.from.byte } else { 0 };
        let z = if h.to.block == b { h.to.byte } else { block.text.len() as u32 };
        let col = Rgba::hex(COLOURS[h.colour.min(3) as usize].1);
        ranges.push((a, z, col.a(if th.dark { 0.30 } else { 0.42 })));
    }
    if let Some(sel) = &r.sel
        && sel.s == s
        && sel.a.0 <= b
        && b <= sel.b.0
    {
        let a = if sel.a.0 == b { sel.a.1 } else { 0 };
        let z = if sel.b.0 == b { sel.b.1 } else { block.text.len() as u32 };
        ranges.push((a, z, t::ACCENT.a(if th.dark { 0.32 } else { 0.38 })));
    }
    for (a, z, col) in ranges {
        // A stored highlight's bytes are trusted no further than the text's own characters: a
        // book read differently since must not panic the page, every time it is drawn.
        let (a, z) = (block.text.floor_char_boundary(a as usize) as u32, block.text.floor_char_boundary(z as usize) as u32);
        for l in &laid.lines {
            let (la, lz) = (a.max(l.start), z.min(l.end));
            if la >= lz {
                continue;
            }
            let trimmed = l.start + block.text[l.start as usize..lz as usize].trim_end().len() as u32;
            let lz = lz.min(trimmed.max(la));
            let x0 = left + x_at(c, &block.text, l, la);
            let x1 = left + x_at(c, &block.text, l, lz);
            if x1 > x0 {
                c.rect(x0 - 1.0, y + l.top + 1.0, x1 - x0 + 2.0, l.h - 2.0, 3.0, col);
            }
        }
    }
    let base = if block.kind == Kind::Quote { th.ink.mix(th.bg, 0.12) } else { th.ink };
    for l in &laid.lines {
        for run in &l.runs {
            let piece = &block.text[run.start as usize..run.end as usize];
            let ink = if run.style & LINK != 0 { th.link } else { base };
            let raise = if run.style & SUP != 0 { -run.size * 0.35 } else { 0.0 };
            let tt = Text::new(run.size, ink).face(run.face).line(l.h / run.size);
            c.text(piece.trim_end_matches('\n'), left + run.x, y + l.top + raise, tt);
        }
    }
    // The selection's handles.
    if let Some(sel) = &r.sel
        && sel.s == s
        && sel.existing.is_none()
    {
        for (end, (hb, hbyte)) in [(false, sel.a), (true, sel.b)] {
            if hb != b {
                continue;
            }
            if let Some(l) = laid.line_of(if end { hbyte.saturating_sub(1).max(laid.lines.first().map_or(0, |l| l.start)) } else { hbyte }) {
                let x = left + x_at(c, &block.text, l, hbyte.min(l.end));
                c.rect(x - 1.0, y + l.top + 2.0, 2.0, l.h - 4.0, 1.0, t::ACCENT);
                let cy = if end { y + l.top + l.h + 2.0 } else { y + l.top - 2.0 };
                c.circle(x, cy, 6.0, t::ACCENT);
            }
        }
    }
}

fn x_at(c: &mut Canvas, text: &str, l: &typeset::Line, i: u32) -> f32 {
    let laid = Laid { h: 0.0, lines: vec![l.clone()], left: 0.0, image: None };
    laid.x_of(c, text, i)
}

/// The text a selection covers, its blocks joined.
fn sel_text(doc: &Doc, sel: &Sel) -> String {
    let sec = &doc.sections[sel.s as usize];
    let mut out = Vec::new();
    for b in sel.a.0..=sel.b.0 {
        let Some(block) = sec.blocks.get(b as usize) else { break };
        let a = if b == sel.a.0 { sel.a.1 } else { 0 } as usize;
        let z = if b == sel.b.0 { sel.b.1 as usize } else { block.text.len() };
        if let Some(t) = block.text.get(a.min(z)..z) {
            out.push(t.trim().to_string());
        }
    }
    out.join("\n")
}

/// The bar over a selection: four colours, Note, Copy (and Remove for a highlight).
fn selection_bar(app: &mut App, r: &mut BookReader, doc: &Doc, shown: &[(Item, f32)]) {
    let Some(sel) = r.sel.clone() else { return };
    let view_h = app.canvas.h;
    // Above the selection when there is room, else under it.
    let top_y = shown.iter().find(|(i, _)| *i == Item::Page(sel.s as usize, sel.a.0)).map(|(_, y)| *y);
    let bottom_y = shown.iter().find(|(i, _)| *i == Item::Page(sel.s as usize, sel.b.0)).map(|(_, y)| *y);
    let sel_top = match (top_y, r.cache.get(&(sel.s, sel.a.0))) {
        (Some(y), Some(l)) => y + l.line_of(sel.a.1).map_or(0.0, |l| l.top),
        _ => view_h * 0.3,
    };
    let sel_bottom = match (bottom_y, r.cache.get(&(sel.s, sel.b.0))) {
        (Some(y), Some(l)) => y + l.line_of(sel.b.1.saturating_sub(1)).map_or(0.0, |l| l.top + l.h),
        _ => view_h * 0.5,
    };
    let (w, h) = (WIDTH - 2.0 * PAD, 104.0);
    let x = PAD;
    let above = sel_top - h - 14.0;
    let y = if above > app.canvas.inset_top + 8.0 { above } else { (sel_bottom + 18.0).min(view_h - app.canvas.inset_bottom - h - 8.0) };
    let mut cx = cx!(app);
    cx.c.shadow(x, y, w, h, 14.0, 18.0, Rgba::hex(0x000000).a(0.5));
    cx.c.rect(x, y, w, h, 14.0, t::S2);
    // The colours, each with its name.
    let on = sel.existing.as_ref().map(|h| h.colour);
    let mut pick: Option<u8> = None;
    let mut cxx = x + 8.0;
    for (i, (name, col)) in COLOURS.iter().enumerate() {
        let tt = Text::new(ty::CAPTION, t::INK).medium();
        let cw = cx.c.measure(name, tt) + 30.0;
        let (ky, kh) = (y + 8.0, 34.0);
        if on == Some(i as u8) {
            cx.c.rect(cxx, ky, cw, kh, 8.0, t::S3);
        } else if cx.pressed(cxx, ky, cw, kh) {
            cx.c.rect(cxx, ky, cw, kh, 8.0, t::WASH);
        }
        cx.c.circle(cxx + 13.0, ky + kh / 2.0, 5.0, Rgba::hex(*col));
        cx.c.text(name, cxx + 23.0, ky + (kh - tt.lh()) / 2.0, tt);
        if cx.tap(cxx, ky, cw, kh) {
            pick = Some(i as u8);
        }
        cxx += cw + 2.0;
    }
    cx.c.rect(x + 12.0, y + 50.0, w - 24.0, 1.0, 0.0, t::LINE);
    let actions: Vec<(&str, &'static str)> = if sel.existing.is_some() { vec![("Note", "edit"), ("Copy", "copy"), ("Remove", "trash")] } else { vec![("Note", "edit"), ("Copy", "copy")] };
    let aw = (w - 16.0) / actions.len() as f32;
    let mut act = None;
    for (i, (words, glyph)) in actions.iter().enumerate() {
        let ax = x + 8.0 + aw * i as f32;
        let ay = y + 54.0;
        if cx.pressed(ax, ay, aw, 44.0) {
            cx.c.rect(ax, ay, aw, 44.0, 8.0, t::WASH);
        }
        let tt = Text::new(ty::UI, t::INK).medium();
        let tw = cx.c.measure(words, tt) + 24.0;
        let tx = ax + (aw - tw) / 2.0;
        cx.c.icon(glyph, tx + 8.0, ay + 22.0, 15.0, if *words == "Remove" { t::DANGER } else { t::INK });
        cx.c.text(words, tx + 24.0, ay + (44.0 - tt.lh()) / 2.0, tt.color(if *words == "Remove" { t::DANGER } else { t::INK }));
        if cx.tap(ax, ay, aw, 44.0) {
            act = Some(*words);
        }
    }
    // A tap on the bar never reaches the page.
    if cx.ui.tap.is_some_and(|(tx, ty_)| tx >= x && tx < x + w && ty_ >= y && ty_ < y + h) {
        cx.ui.tap = None;
    }
    let text = sel_text(doc, &sel);
    let now = shelf::now_ms();
    let make = |colour: u8| {
        let (chapter, ..) = doc.chapter_span(sel.s, sel.a.0);
        Highlight {
            id: 0,
            book: r.id,
            from: Place { section: sel.s, block: sel.a.0, byte: sel.a.1 },
            to: Place { section: sel.s, block: sel.b.0, byte: sel.b.1 },
            text: text.clone(),
            colour,
            note: String::new(),
            chapter,
            created_ms: now,
            due_ms: shelf::first_due(now),
            interval_d: 0,
            retired: false,
        }
    };
    if let Some(colour) = pick {
        match &sel.existing {
            Some(h) => {
                let mut h = h.clone();
                h.colour = colour;
                app.shelf.update_highlight(h);
            }
            None => app.shelf.add_highlight(make(colour)),
        }
        r.sel = None;
    }
    match act {
        Some("Note") => {
            let h = match &sel.existing {
                Some(h) => h.clone(),
                None => {
                    let h = make(0);
                    app.shelf.add_highlight(h.clone());
                    h
                }
            };
            r.note = h.note.clone();
            r.note_for = Some(h);
            r.sheet = Sheet::Note;
            r.sel = None;
        }
        Some("Copy") => {
            app.host.copy_text(&text);
            app.toast("Copied");
            r.sel = None;
        }
        Some("Remove") => {
            if let Some(h) = &sel.existing {
                app.shelf.remove_highlight(h);
            }
            r.sel = None;
        }
        _ => {}
    }
}

/// The controls over the page: back, the title, search and contents at the top; how far, and
/// Contents, Type and Notes at the bottom.
#[allow(clippy::too_many_arguments)]
fn chrome(app: &mut App, r: &mut BookReader, doc: &Doc, chapter: &str, left_words: &str, pct: u32, _ch_end: u64, at_char: u64) -> bool {
    let th = theme(&r.ty);
    let bar = th.bg.mix(th.ink, 0.06);
    let mut cx = cx!(app);
    // While the menu or the dialog is open, the keys under it take no tap: it is the menu's.
    let held = if r.more || r.confirm_remove { kit::hold(&mut cx) } else { None };
    let top = cx.c.inset_top;
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 60.0, 0.0, bar);
    let back = kit::icon_key_ink(&mut cx, 6.0, top + 8.0, "arrow-left", th.ink);
    let search = kit::icon_key_ink(&mut cx, WIDTH - 6.0 - 88.0, top + 8.0, "search", th.ink);
    let more = kit::icon_key_ink(&mut cx, WIDTH - 6.0 - 44.0, top + 8.0, "more", th.ink);
    cx.c.text_fit(&r.title, 56.0, top + 9.0, WIDTH - 56.0 - 100.0, Text::new(ty::UI, th.ink).semi());
    cx.c.text_fit(chapter, 56.0, top + 31.0, WIDTH - 56.0 - 100.0, Text::new(ty::CAPTION, th.dim));
    let bh = 108.0 + cx.c.inset_bottom;
    let by = cx.c.h - bh;
    cx.c.rect(-1000.0, by, WIDTH + 2000.0, bh, 0.0, bar);
    let f = if doc.chars > 0 { at_char as f32 / doc.chars as f32 } else { 0.0 };
    cx.c.text(&format!("{pct}% of the book"), PAD, by + 12.0, Text::new(ty::CAPTION, th.ink).medium());
    cx.c.text(left_words, WIDTH - PAD, by + 12.0, Text::new(ty::CAPTION, th.dim).right());
    cx.c.rect(PAD, by + 38.0, WIDTH - 2.0 * PAD, 3.0, 1.5, th.ink.a(0.12));
    cx.c.rect(PAD, by + 38.0, ((WIDTH - 2.0 * PAD) * f).max(3.0), 3.0, 1.5, t::ACCENT);
    let keys: [(&str, Option<&'static str>); 3] = [("Contents", Some("list")), ("Type", None), ("Notes", Some("notes"))];
    let kw = (WIDTH - 2.0 * PAD) / keys.len() as f32;
    let mut hit = None;
    for (i, (words, glyph)) in keys.iter().enumerate() {
        let kx = PAD + kw * i as f32;
        let ky = by + 50.0;
        if cx.pressed(kx, ky, kw, 50.0) {
            cx.c.rect(kx, ky, kw, 50.0, 10.0, th.ink.a(0.08));
        }
        match glyph {
            Some(g) => cx.c.icon(g, kx + kw / 2.0, ky + 16.0, 18.0, th.ink),
            None => {
                cx.c.text("Aa", kx + kw / 2.0, ky + 5.0, Text::new(16.0, th.ink).face(Weight::SerifBold).center());
            }
        }
        cx.c.text(words, kx + kw / 2.0, ky + 30.0, Text::new(ty::SMALL, th.dim).center());
        if cx.tap(kx, ky, kw, 50.0) {
            hit = Some(i);
        }
    }
    kit::release(&mut cx, held);
    if r.more {
        match kit::menu(&mut cx, top + 56.0, &["Remove from library"]) {
            Some(0) => {
                r.more = false;
                r.confirm_remove = true;
            }
            Some(_) => r.more = false,
            None => {}
        }
    }
    match hit {
        Some(0) => {
            r.sheet = Sheet::Contents;
            r.sheet_scroll = Scroll::default();
            r.sheet_fresh = true;
            r.chrome = false;
        }
        Some(1) => {
            r.sheet = Sheet::Type;
            r.chrome = false;
        }
        Some(2) => {
            let id = r.id;
            app.push(Route::BookNotes(id));
        }
        _ => {}
    }
    if search {
        app.push(Route::InSearch);
    }
    if more {
        r.more = true;
    }
    back
}

/// A footnote (any link inside the book) opened in place, over the page.
fn popover(app: &mut App, r: &mut BookReader, doc: &Doc, s: u32, b: u32, view_h: f32) {
    let Some(block) = doc.block(s, b) else {
        r.popover = None;
        return;
    };
    // The note, and the paragraphs after it that belong to it (up to the next heading or note).
    let mut text = block.text.clone();
    for nb in b + 1..b + 3 {
        match doc.block(s, nb) {
            Some(n) if n.kind == Kind::Para && text.chars().count() < 280 && !n.links.iter().any(|l| l.start == 0) => {
                text.push_str("\n\n");
                text.push_str(&n.text);
            }
            _ => break,
        }
    }
    let (chapter, ..) = doc.chapter_span(s, b);
    let mut cx = cx!(app);
    cx.ui.cover();
    cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, view_h, 0.0, t::SCRIM.a(0.35));
    let w = WIDTH - 2.0 * PAD;
    let bt = Text::new(ty::UI, t::INK).line(1.5);
    let body_h = cx.c.para_h(&text.replace("\n\n", " "), w - 32.0, bt, 12);
    let h = 16.0 + 20.0 + 10.0 + body_h + 12.0 + 44.0 + 8.0;
    let (x, y) = (PAD, (view_h * 0.42 - h / 2.0).max(cx.c.inset_top + 16.0));
    cx.c.shadow(x, y, w, h, 14.0, 18.0, Rgba::hex(0x000000).a(0.5));
    cx.c.rect(x, y, w, h, 14.0, t::S2);
    let label = Text::new(ty::CAPTION, t::ACCENT).semi();
    let lw = cx.c.text("Note", x + 16.0, y + 16.0, label);
    if !chapter.is_empty() {
        cx.c.text_fit(&format!("· {chapter}"), x + 16.0 + lw + 6.0, y + 16.0, w - 90.0 - lw, Text::new(ty::CAPTION, t::INK3));
    }
    let close = kit::icon_key(&mut cx, x + w - 48.0, y + 4.0, "close");
    cx.c.para(&text.replace("\n\n", " "), x + 16.0, y + 46.0, w - 32.0, bt, 12);
    // "Go there": a quiet key, a step lighter than the card.
    let (gx, gy, gw, gh) = (x + 16.0, y + h - 52.0, 116.0, 40.0);
    cx.c.rect(gx, gy, gw, gh, t::KEY_R, if cx.pressed(gx, gy, gw, gh) { t::S3.mix(t::INK, 0.08) } else { t::S3 });
    let gt = Text::new(ty::UI, t::INK).semi().center();
    cx.c.text("Go there", gx + gw / 2.0, gy + (gh - gt.lh()) / 2.0, gt);
    let go = cx.tap(gx, gy, gw, gh);
    let outside = cx.ui.tap.take().is_some_and(|(tx, ty_)| !(tx >= x && tx < x + w && ty_ >= y && ty_ < y + h));
    if go {
        r.back_to = Some(r.pos);
        r.popover = None;
        r.go(&mut app.canvas, &app.images, Place { section: s, block: b, byte: 0 }, view_h);
    } else if close || outside {
        r.popover = None;
    }
}

/// The type sheet: size, spacing, margins and theme, previewed live on the page above it.
fn type_sheet(app: &mut App, r: &mut BookReader) {
    let mut cx = cx!(app);
    let (top, outside) = kit::sheet(&mut cx, 330.0);
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let mut y = top + 28.0;
    // Size: a track; a tap or a drag along it sets it.
    let (lo, hi) = crate::typeset::SIZES;
    cx.c.text("A", x, y + 4.0, Text::new(14.0, t::INK2).face(Weight::Serif));
    cx.c.text("A", x + w - 46.0, y - 2.0, Text::new(24.0, t::INK2).face(Weight::Serif));
    cx.c.text(&format!("{}", r.ty.size.round() as i32), x + w, y + 4.0, Text::new(ty::UI, t::INK).medium().right());
    let (tx, tw) = (x + 24.0, w - 24.0 - 56.0);
    let ty0 = y + 14.0;
    let f = (r.ty.size - lo) / (hi - lo);
    cx.c.rect(tx, ty0 - 1.5, tw, 3.0, 1.5, t::S3);
    cx.c.rect(tx, ty0 - 1.5, tw * f, 3.0, 1.5, t::ACCENT);
    cx.c.circle(tx + tw * f, ty0, 9.0, t::INK);
    let finger = cx.ui.finger().filter(|_| cx.ui.drag_from.is_some_and(|(fx, fy)| fx > tx - 20.0 && fx < tx + tw + 20.0 && (fy - ty0).abs() < 24.0));
    let tap = cx.ui.tap.filter(|(fx, fy)| *fx > tx - 20.0 && *fx < tx + tw + 20.0 && (fy - ty0).abs() < 24.0);
    if let Some((fx, _)) = finger.or(tap) {
        let v = lo + ((fx - tx) / tw).clamp(0.0, 1.0) * (hi - lo);
        r.ty.size = v.round();
        cx.ui.tap = None;
    }
    y += 50.0;
    let label = Text::new(ty::UI, t::INK2);
    let seg_x = x + 96.0;
    cx.c.text("Spacing", x, y + 9.0, label);
    if let Some(i) = kit::segmented(&mut cx, seg_x, y, w - 96.0, &["Tight", "Normal", "Loose"], r.ty.spacing as usize) {
        r.ty.spacing = i as u8;
    }
    y += 50.0;
    cx.c.text("Margins", x, y + 9.0, label);
    if let Some(i) = kit::segmented(&mut cx, seg_x, y, w - 96.0, &["S", "M", "L"], r.ty.margins as usize) {
        r.ty.margins = i as u8;
    }
    y += 58.0;
    // Themes: a swatch of each, its words in its own ink.
    let sw = (w - 3.0 * 12.0) / 4.0;
    for (i, (name, th)) in THEMES.iter().enumerate() {
        let sx = x + (sw + 12.0) * i as f32;
        cx.c.rect(sx, y, sw, 44.0, 10.0, th.bg);
        if i as u8 == r.ty.theme {
            cx.c.outline(sx - 2.0, y - 2.0, sw + 4.0, 48.0, 12.0, 2.0, t::ACCENT);
        } else {
            cx.c.outline(sx, y, sw, 44.0, 10.0, 1.0, t::LINE);
        }
        cx.c.text("Aa", sx + sw / 2.0, y + 11.0, Text::new(15.0, th.ink).face(Weight::SerifBold).center());
        cx.c.text(name, sx + sw / 2.0, y + 52.0, Text::new(ty::SMALL, if i as u8 == r.ty.theme { t::INK } else { t::INK2 }).center());
        if cx.tap(sx, y, sw, 70.0) {
            r.ty.theme = i as u8;
        }
    }
    cx.ui.tap = None;
    if outside {
        r.sheet = Sheet::None;
        if let Ok(j) = serde_json::to_string(&r.ty) {
            app.shelf.set_setting("type", j);
        }
        app.reading_type = r.ty;
    }
}

/// The contents: every entry, the one you are in marked; a tap goes there.
fn contents_sheet(app: &mut App, r: &mut BookReader, doc: &Doc, place: Place, view_h: f32) {
    let current = doc.toc_index(place.section, place.block);
    let dt = 1.0 / 60.0;
    let mut cx = cx!(app);
    let h = (view_h * 0.82).min(view_h - cx.c.inset_top - 24.0);
    let (top, outside) = kit::sheet(&mut cx, h);
    cx.c.text("Contents", PAD + 4.0, top + 24.0, Text::new(ty::HEADING, t::INK).semi());
    cx.c.text(&format!("{} entries", doc.toc.len()), WIDTH - PAD - 4.0, top + 30.0, Text::new(ty::CAPTION, t::INK3).right());
    let list_top = top + 64.0;
    let bottom = cx.c.h - cx.c.inset_bottom;
    let row = 46.0;
    // Its own scroller, for the rows only; it opens on the entry you are in.
    r.sheet_scroll.max = doc.toc.len() as f32 * row + 16.0 - (bottom - list_top);
    if r.sheet_fresh {
        r.sheet_fresh = false;
        r.sheet_scroll.off = (current.unwrap_or(0) as f32 * row - (bottom - list_top) * 0.35).clamp(0.0, r.sheet_scroll.max.max(0.0));
    }
    r.sheet_scroll.update(cx.ui, [0.0, list_top, WIDTH, bottom - list_top], dt);
    cx.c.push_clip(0.0, list_top, WIDTH, bottom - list_top);
    let off = r.sheet_scroll.off;
    let mut jump = None;
    if doc.toc.is_empty() {
        cx.c.text("This book has no contents.", PAD + 4.0, list_top + 8.0, Text::new(ty::UI, t::INK2));
    }
    for (i, e) in doc.toc.iter().enumerate() {
        let y = list_top + i as f32 * row - off;
        if y + row < list_top || y > bottom {
            continue;
        }
        let on = current == Some(i);
        if on {
            cx.c.rect(0.0, y, WIDTH, row, 0.0, t::S2);
        } else if cx.ui.pressed(0.0, y, WIDTH, row) {
            cx.c.rect(0.0, y, WIDTH, row, 0.0, t::WASH);
        }
        let indent = PAD + 4.0 + 16.0 * (e.level.saturating_sub(1).min(3)) as f32;
        let tt = Text::new(ty::UI, if on { t::ACCENT } else if e.level > 1 { t::INK2 } else { t::INK }).medium();
        cx.c.text_fit(&e.title, indent, y + (row - tt.lh()) / 2.0, WIDTH - indent - PAD - 70.0, tt);
        // Minutes in it, at your pace.
        let start = doc.char_at(e.section, e.block, 0);
        let end = doc.toc.get(i + 1).map_or(doc.chars, |n| doc.char_at(n.section, n.block, 0));
        let cpm = app.shelf.book(r.id).map_or(0.0, |b| b.cpm);
        let mins = ((end.saturating_sub(start)) as f32 / if cpm > 0.0 { cpm } else { DEFAULT_CPM }).round().max(1.0) as i64;
        cx.c.text(&format!("{mins} min"), WIDTH - PAD - 4.0, y + (row - 18.0) / 2.0, Text::new(ty::CAPTION, t::INK3).right());
        if cx.ui.tapped(0.0, y.max(list_top), WIDTH, row) {
            jump = Some(Place { section: e.section, block: e.block, byte: 0 });
        }
    }
    cx.c.pop_clip();
    cx.ui.tap = None;
    if let Some(p) = jump {
        r.back_to = Some(r.pos);
        r.sheet = Sheet::None;
        r.go(&mut app.canvas, &app.images, p, view_h);
    } else if outside {
        r.sheet = Sheet::None;
    }
}

/// A note on a highlight: the words it is on, and what you think.
fn note_sheet(app: &mut App, r: &mut BookReader) {
    let Some(h) = r.note_for.clone() else {
        r.sheet = Sheet::None;
        return;
    };
    let mut cx = cx!(app);
    let (top, outside) = kit::sheet(&mut cx, 236.0);
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let col = Rgba::hex(COLOURS[h.colour.min(3) as usize].1);
    cx.c.rect(x, top + 26.0, 3.0, 40.0, 1.5, col);
    cx.c.para(&h.text, x + 14.0, top + 24.0, w - 14.0, Text::new(ty::UI, t::INK2).face(Weight::Serif).line(1.45), 2);
    let fh = kit::field(&mut cx, "note", x, top + 80.0, w, "Your note", "What this makes you think", &mut r.note, kit::Kb::Text);
    let save = kit::primary(&mut cx, x, top + 80.0 + fh + 14.0, w, 46.0, "Save note", None);
    cx.ui.tap = None;
    if save || outside {
        if cx.focus.id.is_some() {
            cx.focus.id = None;
            cx.host.keyboard(false, "", kit::Kb::Text);
        }
        let mut h = h;
        h.note = r.note.trim().to_string();
        app.shelf.update_highlight(h);
        r.note_for = None;
        r.sheet = Sheet::None;
    }
}

// Search inside the book

/// Every place the words appear (case aside), with the words around each, at most 300.
pub fn search(doc: &Doc, q: &str) -> Vec<Hit> {
    let q = q.trim();
    let mut out = Vec::new();
    if q.chars().count() < 2 {
        return out;
    }
    let qb = q.as_bytes();
    for (s, sec) in doc.sections.iter().enumerate() {
        for (b, block) in sec.blocks.iter().enumerate() {
            let tb = block.text.as_bytes();
            if tb.len() < qb.len() {
                continue;
            }
            let mut i = 0;
            while i + qb.len() <= tb.len() {
                if tb[i..i + qb.len()].eq_ignore_ascii_case(qb) && block.text.is_char_boundary(i) {
                    let (context, at) = context(&block.text, i, i + qb.len());
                    out.push(Hit { s: s as u32, b: b as u32, byte: i as u32, context, at });
                    if out.len() >= 300 {
                        return out;
                    }
                    i += qb.len();
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}

/// About 40 characters before the match and 70 after, cut at words, with "…" where cut.
fn context(text: &str, a: usize, z: usize) -> (String, (usize, usize)) {
    let mut start = a;
    let mut n = 0;
    while start > 0 && n < 40 {
        start -= text[..start].chars().next_back().map_or(1, char::len_utf8);
        n += 1;
    }
    if start > 0
        && let Some(sp) = text[start..a].find(' ')
    {
        start += sp + 1;
    }
    let mut end = z;
    n = 0;
    while end < text.len() && n < 70 {
        end += text[end..].chars().next().map_or(1, char::len_utf8);
        n += 1;
    }
    if end < text.len()
        && let Some(sp) = text[z..end].rfind(' ')
    {
        end = z + sp;
    }
    let lead = if start > 0 { "…" } else { "" };
    let tail = if end < text.len() { "…" } else { "" };
    let s = format!("{lead}{}{tail}", &text[start..end]).replace('\n', " ");
    let at = (lead.len() + a - start, lead.len() + z - start);
    (s, at)
}

/// Search inside the open book: hits grouped by chapter, each with its words around it.
pub fn draw_search(app: &mut App, dt: f32) -> bool {
    let Some(mut r) = app.book.take() else {
        app.pop();
        return true;
    };
    let out = search_page(app, &mut r, dt);
    app.book = Some(r);
    out
}

fn search_page(app: &mut App, r: &mut BookReader, dt: f32) -> bool {
    let Some(doc) = r.doc.clone() else {
        app.pop();
        return true;
    };
    if let Some(rx) = &r.found.rx
        && let Ok((q, hits)) = rx.try_recv()
    {
        if q == r.found.query {
            r.found.hits = hits;
            r.found.searching = false;
        }
        r.found.rx = None;
    }
    if app.fresh && r.found.query.is_empty() {
        app.focus.id = Some("insearch");
        app.focus.text.clear();
        app.focus.kind = kit::Kb::Text;
        app.host.keyboard(true, "", kit::Kb::Text);
    }
    let run = app.typed_done == Some("insearch");
    let top_h = 64.0;
    let pg = app.page_begin(dt, top_h);
    let mut go: Option<Hit> = None;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top + 8.0;
    if r.found.searching {
        cx.c.text("Searching…", x0, y, Text::new(ty::UI, t::INK3));
        y += 30.0;
    } else if !r.found.query.is_empty() && r.found.hits.is_empty() && r.found.rx.is_none() {
        cx.c.text(&format!("No “{}” in this book", r.found.query), x0, y, Text::new(ty::UI, t::INK2));
        y += 30.0;
    } else if !r.found.hits.is_empty() {
        let n = r.found.hits.len();
        cx.c.text(&if n >= 300 { "300 or more places".to_string() } else if n == 1 { "1 place".into() } else { format!("{n} places") }, x0, y, Text::new(ty::CAPTION, t::INK3));
        y += 26.0;
    }
    let mut last_chapter: Option<String> = None;
    let view_top = -cx.c.dy - 80.0;
    let view_bottom = -cx.c.dy + cx.c.h + 80.0;
    let bt = Text::new(15.5, t::INK).face(Weight::Serif).line(1.45);
    for hit in &r.found.hits {
        let (chapter, ..) = doc.chapter_span(hit.s, hit.b);
        if last_chapter.as_ref() != Some(&chapter) {
            y += 8.0;
            let title = if chapter.is_empty() { "Untitled".to_string() } else { chapter.clone() };
            if y > view_top && y < view_bottom {
                cx.c.text_fit(&title, x0, y, w, Text::new(ty::CAPTION, t::INK3).semi());
            }
            y += 26.0;
            last_chapter = Some(chapter);
        }
        let lines_h = 2.0 * bt.lh();
        if y + lines_h > view_top && y < view_bottom {
            if cx.pressed(0.0, y - 6.0, WIDTH, lines_h + 12.0) {
                cx.c.rect(0.0, y - 6.0, WIDTH, lines_h + 12.0, 0.0, t::WASH);
            }
            // The match in the accent: before, the match, after, wrapped as one.
            hit_lines(cx.c, &hit.context, hit.at, x0, y, w, bt);
            if cx.tap(0.0, y - 6.0, WIDTH, lines_h + 12.0) {
                go = Some(hit.clone());
            }
        }
        y += lines_h + 16.0;
    }
    let bottom = y;
    // The fixed top: Back and the field.
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        let back = kit::icon_key(&mut cx, 6.0, top + 10.0, "arrow-left");
        let fx = 56.0;
        let fw = WIDTH - fx - PAD;
        cx.c.rect(fx, top + 10.0, fw, 44.0, t::KEY_R, t::S1);
        cx.c.icon("search", fx + 20.0, top + 32.0, 16.0, t::INK3);
        let mut q = r.found.query.clone();
        kit::inline(&mut cx, "insearch", fx + 40.0, top + 10.0, fw - 54.0, 44.0, "Search in this book", &mut q, kit::Kb::Text, Text::new(ty::BODY, t::INK));
        r.found.query = q;
        if back {
            app.page_end(&pg, bottom);
            app.pop();
            return true;
        }
    }
    app.page_end(&pg, bottom);
    if run && !r.found.query.trim().is_empty() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (d, q) = (doc.clone(), r.found.query.clone());
        r.found.searching = true;
        r.found.hits.clear();
        r.found.rx = Some(rx);
        let _ = std::thread::Builder::new().name("leaf-search".into()).spawn(move || {
            let hits = search(&d, &q);
            let _ = tx.send((q, hits));
            crate::wake::wake();
        });
    }
    if let Some(h) = go {
        let view_h = app.canvas.h;
        r.back_to = Some(r.pos);
        r.go(&mut app.canvas, &app.images, Place { section: h.s, block: h.b, byte: h.byte }, view_h);
        r.chrome = false;
        app.pop();
    }
    pg.moving || r.found.searching && !crate::wake::has()
}

/// A hit's words over two lines at most, the match in the accent.
fn hit_lines(c: &mut Canvas, s: &str, at: (usize, usize), x: f32, y: f32, w: f32, tt: Text) {
    let mut line_x = x;
    let mut line_y = y;
    let mut lines = 1;
    let mut pos = 0;
    for word in s.split_inclusive(' ') {
        let (a, z) = (pos, pos + word.len());
        pos = z;
        let ww = c.measure(word.trim_end(), tt);
        if line_x > x && line_x + ww > x + w {
            lines += 1;
            if lines > 2 {
                return;
            }
            line_x = x;
            line_y += tt.lh();
        }
        let hot = a < at.1 && z > at.0;
        let style = if hot { tt.color(t::ACCENT).face(Weight::SerifBold) } else { tt };
        c.text(word, line_x, line_y, style);
        line_x += c.measure(word, style);
    }
}

/// The reader's type, as kept.
pub fn saved_type(app: &App) -> Type {
    app.shelf.setting("type").and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
}

/// Draw a book's cover in a box: its picture, or one drawn from its title in type.
#[allow(clippy::too_many_arguments)]
pub fn cover(c: &mut Canvas, im: &mut Images, b: &shelf::Book, x: f32, y: f32, w: f32, h: f32, now: u64) {
    let of = crate::images::CoverOf::Book(b.id);
    if b.has_cover && im.cover(c, of, now) {
        c.image_cover(&of.key(), x, y, w, h, t::COVER_R, Rgba::WHITE);
        return;
    }
    // A drawn cover: one of a few deep tones by the title, the title set in the reading face.
    const TONES: [u32; 6] = [0x24405E, 0x5C2E2E, 0x24493F, 0x463A63, 0x5E4724, 0x343E4C];
    let hash = b.title.bytes().fold(7u32, |h, x| h.wrapping_mul(31).wrapping_add(x as u32));
    let tone = Rgba::hex(TONES[(hash % TONES.len() as u32) as usize]);
    c.rect(x, y, w, h, t::COVER_R, tone);
    let k = w / 110.0;
    let pad = 10.0 * k;
    c.rect(x + pad, y + pad, 18.0 * k, 2.0 * k, 1.0, Rgba::WHITE.a(0.55));
    let tt = Text::new(15.0 * k, Rgba::hex(0xF4F1EA)).face(Weight::SerifBold).line(1.18);
    c.para(&b.title, x + pad, y + pad + 10.0 * k, w - 2.0 * pad, tt, 4);
    // The kind at the bottom right, the author beside it.
    let mut room = w - 2.0 * pad;
    if b.kind != shelf::Kind::Epub {
        let tag = if b.kind == shelf::Kind::Pdf { "PDF" } else { "TXT" };
        let tt = Text::new(9.5 * k, Rgba::hex(0xF4F1EA).a(0.8)).medium();
        room -= c.text(tag, x + w - pad, y + h - pad - 14.0 * k, tt.right()) + 6.0 * k;
    }
    if !b.author.is_empty() {
        c.text_fit(&b.author, x + pad, y + h - pad - 14.0 * k, room, Text::new(10.0 * k, Rgba::hex(0xF4F1EA).a(0.75)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_every_place_with_its_words_around() {
        let d = crate::epub::parse_txt("CHAPTER ONE\n\nThe anchor held. An Anchor again.\n\nNothing here.", "t");
        let hits = search(&d, "anchor");
        assert_eq!(hits.len(), 2);
        assert_eq!((hits[0].b, hits[0].byte), (1, 4));
        let h = &hits[1];
        assert_eq!(&h.context[h.at.0..h.at.1], "Anchor");
        assert!(search(&d, "a").is_empty(), "one letter is not a search");
    }

    #[test]
    fn time_left_reads_in_words() {
        assert_eq!(time_left(0, 1000.0), "End of the chapter");
        assert_eq!(time_left(400, 1000.0), "Under a minute left in chapter");
        assert_eq!(time_left(9_000, 1000.0), "9 min left in chapter");
        assert_eq!(time_left(70_000, 1000.0), "1 h 10 min left in chapter");
    }

    #[test]
    fn reading_time_counts_only_while_the_page_moves() {
        let mut c = ReadClock::default();
        c.moved(1_000_000_000, 0);
        c.moved(31_000_000_000, 500);
        assert!((c.secs - 30.0).abs() < 1e-6);
        assert_eq!(c.chars, 500.0);
        // Left open for ten minutes: none of it counts.
        c.moved(631_000_000_000, 600);
        assert!((c.secs - 30.0).abs() < 1e-6);
        // Eighty seconds in another app, then a scroll: not reading either.
        c.pause();
        c.moved(711_000_000_000, 700);
        assert!((c.secs - 30.0).abs() < 1e-6, "{}", c.secs);
        c.moved(712_000_000_000, 750);
        assert!((c.secs - 31.0).abs() < 1e-6, "and reading again counts");
    }
}
