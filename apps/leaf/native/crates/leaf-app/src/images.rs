//! Covers and pages: fetched from the server on worker threads, kept on disk, decoded off the UI
//! thread, and handed to the canvas's picture atlas.
//!
//! - A cover is cropped to one size ([`COVER_W`] × [`COVER_H`]) and a tiny copy of it is kept as
//!   the series page's banner (drawn stretched, which blurs it into a wash of the cover's colours).
//! - A page is kept on disk as the server sent it (`cache/<chapter>/<n>`), decoded, scaled to at
//!   most [`PAGE_W`] wide and cut into tiles at most [`TILE_H`] tall, which go into the atlas as
//!   they come on screen. Decoded pages are kept only near the reader's place ([`Images::trim`]);
//!   their sizes are kept for good, so the strip's heights never change back.
//!
//! The reader says each frame which pages it wants, nearest first ([`Images::want_pages`]): the
//! queue is replaced by that list, so pages scrolled far past are never fetched for nothing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use crate::draw::Canvas;
use crate::logic::TILE_H;
use crate::net::{Client, Server, write_atomic};

/// Covers are cut to this size: a grid cover is about 310 px wide on a 1080 px phone.
pub const COVER_W: u32 = 336;
pub const COVER_H: u32 = 480;
/// The banner: so small that drawing it across the screen blurs it.
const BANNER: (u32, u32) = (14, 20);
/// A cover as the home page's spotlight and the series page's banner draw it, across and down:
/// cut to this shape, at the cover's own resolution (never more than the screen's width).
const HERO_ASPECT: f32 = 720.0 / 860.0;
/// A cover is never drawn bigger than this times its own pixels: one too small to fill the
/// spotlight at that is shown sharp as a poster over a blurred, darkened copy of itself.
pub const MAX_UPSCALE: f32 = 1.3;
/// The poster is kept at most this wide (it is drawn far narrower than the screen).
const POSTER_MAX_W: u32 = 600;
/// The blurred copy behind a poster: this wide, blurred, then drawn across the whole spotlight.
const BACKDROP_W: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoverOf {
    /// A followed series' cover (`/v1/series/{id}/cover`).
    Series(i64),
    /// A search result's (`/v1/covers/{manga_id}`).
    Manga(i64),
    /// A book's, taken from its file at import (never from the server).
    Book(i64),
    /// A recommendation's, by its AniList id (`/v1/recs/cover/{id}`).
    Rec(i64),
}

impl CoverOf {
    pub fn key(self) -> String {
        match self {
            CoverOf::Series(id) => format!("c:s{id}"),
            CoverOf::Manga(id) => format!("c:m{id}"),
            CoverOf::Book(id) => format!("c:k{id}"),
            CoverOf::Rec(id) => format!("c:r{id}"),
        }
    }
    pub fn banner_key(self) -> String {
        format!("b:{}", self.key())
    }
    /// The large copy: the cover cut to the spotlight's shape, or, when the cover is too small
    /// for that ([`Images::hero_poster`]), a blurred, darkened copy to draw behind its poster.
    pub fn hero_key(self) -> String {
        format!("h:{}", self.key())
    }
    /// The cover itself, at its own size, drawn sharp over [`CoverOf::hero_key`]'s blur.
    pub fn poster_key(self) -> String {
        format!("hp:{}", self.key())
    }
    fn path(self) -> String {
        match self {
            // A title not added yet (a preview, `-manga_id`): its entry's cover.
            CoverOf::Series(id) if id < 0 => format!("/v1/covers/{}", -id),
            CoverOf::Series(id) => format!("/v1/series/{id}/cover"),
            CoverOf::Manga(id) => format!("/v1/covers/{id}"),
            CoverOf::Book(_) => String::new(),
            CoverOf::Rec(id) => format!("/v1/recs/cover/{id}"),
        }
    }
    pub fn file(self, data: &Path) -> PathBuf {
        data.join("covers").join(self.key().replace(':', "-"))
    }
}

/// A collection's picture (`/v1/recs/tagart/{slug}`, 640 × 360 WebP), kept in the atlas this size.
pub const ART_W: u32 = 448;
pub const ART_H: u32 = 252;
/// A tag the server had no picture for is asked again after this long (the files come at deploy).
const ART_NONE_MS: u128 = 24 * 3_600_000;

/// A tag's slug, as the server names its picture: lower-cased, spaces as `-`, nothing else but
/// letters and digits ("Martial Arts": `martial-arts`).
pub fn tag_slug(tag: &str) -> String {
    let mut out = String::new();
    for c in tag.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if (c == ' ' || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// The atlas key of a collection's picture.
pub fn art_key(slug: &str) -> String {
    format!("a:{slug}")
}

/// Whether a collection has its picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Art {
    /// In the atlas: draw it with [`art_key`].
    Ready,
    /// On its way (from disk or the server).
    Coming,
    /// The server has none (404): the collage stands in.
    None,
}

/// A chapter's picture as the server makes it (`/v1/chapters/{id}/thumb`, WebP).
pub const THUMB_W: u32 = 240;
pub const THUMB_H: u32 = 180;

/// The atlas key of a chapter's picture.
pub fn thumb_key(chapter: i64) -> String {
    format!("t:{chapter}")
}

/// Where a chapter's picture is kept, as the server sent it.
pub fn thumb_file(data: &Path, chapter: i64) -> PathBuf {
    data.join("thumbs").join(format!("{chapter}.webp"))
}

/// The atlas key of tile `i` of a page, as cut the `cut`th way ([`Images::tile_key`]).
fn tile_key(chapter: i64, n: u32, i: usize, cut: u32) -> String {
    format!("p{chapter}:{n}:{i}:{cut}")
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Want {
    Cover(CoverOf),
    /// `decode` false: only to have it on disk, for later (the next chapter).
    Page { chapter: i64, n: u32, decode: bool },
    /// A picture inside a book's file (an EPUB's figure), by its atlas key.
    Local { key: String, book: PathBuf, entry: String },
    /// A cover at the spotlight's size.
    Hero(CoverOf),
    /// A chapter's picture, from disk or the server.
    Thumb { chapter: i64 },
    /// A collection's picture, by its tag's slug.
    Art(String),
}

/// A decoded page: its size after scaling, and its rows of tiles (RGBA).
pub struct PageImg {
    pub w: u32,
    pub h: u32,
    pub tiles: Vec<Vec<u8>>,
}

struct Pixels {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

/// A cover for the spotlight: cut to its shape when it is big enough, else a blurred backdrop
/// and the cover itself as a poster.
struct HeroPics {
    main: Pixels,
    poster: Option<Pixels>,
}

/// A page's fetch: decoded (or only kept on disk, None), or a scanlator's promo page the server
/// skips (`x-leaf-skip`): dropped from the strip.
enum Fetched {
    Img(Option<PageImg>),
    Skip,
}

enum Got {
    /// `src`: the cover's own size in pixels, which says whether it can fill the spotlight.
    Cover { of: CoverOf, img: Option<Pixels>, banner: Option<Pixels>, src: Option<(u32, u32)> },
    Page { chapter: i64, n: u32, decode: bool, result: Result<Fetched, String> },
    Local { key: String, book: PathBuf, entry: String, img: Option<Pixels> },
    Hero { of: CoverOf, img: Option<HeroPics> },
    Thumb { chapter: i64, img: Option<Pixels> },
    /// `none`: the server has no picture for the tag (not a failure to reach it).
    Art { slug: String, img: Option<Pixels>, none: bool },
}

struct Queue {
    /// Priority (higher first), then the order asked.
    items: Vec<(i32, u64, Want)>,
    seq: u64,
    /// Workers on a page only kept on disk ahead (not to be shown yet): at most
    /// [`PLAIN_AT_ONCE`], so one is always free for the page coming on screen.
    plain_out: usize,
}

/// How many workers may fetch pages only kept ahead at once: a slow network must not hold every
/// worker while the page you scroll to waits.
const PLAIN_AT_ONCE: usize = WORKERS - 1;

struct Shared {
    q: Mutex<Queue>,
    cv: Condvar,
    server: Mutex<Option<Server>>,
    /// PDFs open in the strip reader: their pages come from the platform's renderer, under the
    /// chapter id `-book`.
    pdfs: Mutex<HashMap<i64, PathBuf>>,
    host: Mutex<Option<std::sync::Arc<dyn crate::app::Host>>>,
    /// The screen's width in pixels, which decides whether a cover fills the spotlight.
    screen_w: std::sync::atomic::AtomicU32,
    /// Blank runs at a page's top and bottom are cut short ([`crate::logic::gutter_cuts`]).
    trim: std::sync::atomic::AtomicBool,
    /// Scanlators' promo pages are shown too (the setting off: `?promo=1`).
    promo: std::sync::atomic::AtomicBool,
    /// Screenshots: pages are made up here, never fetched.
    fake_pages: std::sync::atomic::AtomicBool,
}

/// How many fetches and decodes run at once.
const WORKERS: usize = 3;
/// A cover that failed is asked for again after this long.
const COVER_RETRY_NS: u64 = 30_000_000_000;
/// A chapter's picture the server didn't have (404) is asked for again after this long: it makes
/// one within a minute of being asked, from a chapter it has downloaded.
const THUMB_RETRY_NS: u64 = 300_000_000_000;

pub struct Images {
    shared: Arc<Shared>,
    rx: Receiver<Got>,
    /// Queued or being fetched.
    asked: HashSet<Want>,
    cover_failed: HashMap<CoverOf, u64>,
    /// Every page's size once decoded (scaled), kept for good.
    sizes: HashMap<(i64, u32), (u32, u32)>,
    decoded: HashMap<(i64, u32), PageImg>,
    /// Pages that would not load when they were to be shown: drawn with a Retry.
    failed: HashMap<(i64, u32), String>,
    /// Pages that would not load when only kept ahead on disk: not kept ahead again, but fetched
    /// when they come near, as any other (the server tries another source for them meanwhile).
    ahead_failed: HashSet<(i64, u32)>,
    /// Pages asked to be decoded ahead of need ([`Images::want_pages_firm`]): a failure is not
    /// shown, only tried again once they are needed.
    soft: HashSet<(i64, u32)>,
    last_firm: usize,
    /// How many times the trim setting changed: a page's tiles are keyed by it, so tiles cut the
    /// old way, still in the atlas, are never drawn for a page cut the new way.
    cut: u32,
    stored: HashSet<(i64, u32)>,
    last_wants: Vec<(i64, u32, bool)>,
    /// A reader is open: decoded pages arriving are kept. Closed, they are let go on arrival.
    reading: bool,
    /// Pictures inside books: their sizes once decoded, and the ones that would not.
    local_sizes: HashMap<String, (u32, u32)>,
    local_failed: HashSet<String>,
    hero_failed: HashMap<CoverOf, u64>,
    /// Covers too small to fill the spotlight: their poster's size (pixels).
    posters: HashMap<CoverOf, (u32, u32)>,
    /// Covers' own sizes, as they were decoded.
    cover_src: HashMap<CoverOf, (u32, u32)>,
    /// Pictures the server had none of: when, and whether it said then that it had one.
    thumb_failed: HashMap<i64, (u64, bool)>,
    /// Tags with no picture on the server, and pictures that failed (when, ns).
    art_none: HashSet<String>,
    art_failed: HashMap<String, u64>,
    /// Pages left out of the strip: scanlators' promo pages the server skips, and pages hidden by
    /// hand. Kept on disk beside the page (`<n>.skip`).
    skipped: HashSet<(i64, u32)>,
    data: PathBuf,
}

impl Images {
    pub fn new(data: PathBuf) -> Images {
        let shared = Arc::new(Shared {
            q: Mutex::new(Queue { items: Vec::new(), seq: 0, plain_out: 0 }),
            cv: Condvar::new(),
            server: Mutex::new(None),
            pdfs: Mutex::new(HashMap::new()),
            host: Mutex::new(None),
            screen_w: std::sync::atomic::AtomicU32::new(1080),
            trim: std::sync::atomic::AtomicBool::new(true),
            promo: std::sync::atomic::AtomicBool::new(false),
            fake_pages: std::sync::atomic::AtomicBool::new(false),
        });
        let (tx, rx) = channel();
        let client = Client::new();
        for i in 0..WORKERS {
            let (shared, tx, client, data) = (shared.clone(), tx.clone(), client.clone(), data.clone());
            std::thread::Builder::new().name(format!("leaf-img-{i}")).spawn(move || worker(&shared, &tx, &client, &data)).expect("an image thread starts");
        }
        Images {
            hero_failed: HashMap::new(),
            posters: HashMap::new(),
            cover_src: HashMap::new(),
            thumb_failed: HashMap::new(),
            art_none: HashSet::new(),
            art_failed: HashMap::new(),
            skipped: HashSet::new(),
            data,
            shared,
            rx,
            asked: HashSet::new(),
            cover_failed: HashMap::new(),
            sizes: HashMap::new(),
            decoded: HashMap::new(),
            failed: HashMap::new(),
            ahead_failed: HashSet::new(),
            soft: HashSet::new(),
            last_firm: 0,
            cut: 0,
            stored: HashSet::new(),
            last_wants: Vec::new(),
            reading: false,
            local_sizes: HashMap::new(),
            local_failed: HashSet::new(),
        }
    }

    /// The screen is this many pixels wide (each frame; covers decoded from now on follow it).
    pub fn set_screen_w(&self, px: f32) {
        self.shared.screen_w.store(px.round().max(1.0) as u32, std::sync::atomic::Ordering::Relaxed);
    }

    /// The cover is too small to fill the spotlight: the size of its poster ([`CoverOf::poster_key`],
    /// pixels), drawn sharp over the blurred [`CoverOf::hero_key`]. None when the hero fills it.
    /// Whether the cover can fill the spotlight sharp (no more than [`MAX_UPSCALE`]): None until
    /// its cover or spotlight picture has been decoded once.
    pub fn hero_fits(&self, of: CoverOf) -> Option<bool> {
        let screen_w = self.shared.screen_w.load(std::sync::atomic::Ordering::Relaxed).clamp(320, 1440) as f32;
        if let Some(&(w, h)) = self.cover_src.get(&of) {
            let cw = (w as f32).min(h as f32 * HERO_ASPECT);
            return Some(cw * MAX_UPSCALE >= screen_w);
        }
        if self.posters.contains_key(&of) {
            return Some(false);
        }
        None
    }

    pub fn hero_poster(&self, of: CoverOf) -> Option<(u32, u32)> {
        self.posters.get(&of).copied()
    }

    /// Cut blank gutters from pages decoded from now on. Pages already measured are measured
    /// again, so the strip takes their new heights.
    pub fn set_trim(&self, on: bool) {
        self.shared.trim.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    /// The setting changed while no reader is open: forget the pages' sizes.
    pub fn retrim(&mut self, on: bool) {
        self.set_trim(on);
        self.cut += 1;
        self.sizes.clear();
        self.decoded.clear();
        self.last_wants.clear();
    }

    /// The platform, for PDF pages.
    pub fn set_host(&self, h: std::sync::Arc<dyn crate::app::Host>) {
        *self.shared.host.lock().unwrap_or_else(|e| e.into_inner()) = Some(h);
    }

    /// A PDF's pages are asked for under chapter `-book`.
    pub fn set_pdf(&self, book: i64, path: PathBuf) {
        self.shared.pdfs.lock().unwrap_or_else(|e| e.into_inner()).insert(-book, path);
    }

    /// A picture inside a book: its size once known, and asked for (decoded off the UI thread)
    /// when it is not in the atlas. Draw it with the key when [`Canvas::has_image`].
    pub fn local(&mut self, c: &Canvas, key: &str, book: &Path, entry: &str) -> Option<(u32, u32)> {
        let size = self.local_sizes.get(key).copied();
        if !c.has_image(key) && !c.image_waiting(key) && !self.local_failed.contains(key) {
            self.push(60, Want::Local { key: key.to_string(), book: book.to_path_buf(), entry: entry.to_string() });
        }
        size
    }

    /// A picture inside a book's size, once decoded.
    pub fn local_size(&self, key: &str) -> Option<(u32, u32)> {
        self.local_sizes.get(key).copied()
    }

    /// A picture inside a book that would not decode.
    pub fn local_failed(&self, key: &str) -> bool {
        self.local_failed.contains(key)
    }

    pub fn set_server(&self, s: Option<Server>) {
        *self.shared.server.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }

    fn push(&mut self, prio: i32, want: Want) {
        if !self.asked.insert(want.clone()) {
            return;
        }
        let mut q = self.shared.q.lock().unwrap_or_else(|e| e.into_inner());
        q.seq += 1;
        let seq = q.seq;
        q.items.push((prio, seq, want));
        drop(q);
        self.shared.cv.notify_one();
    }

    /// Make sure a cover is in the atlas or on its way. True when it can be drawn now.
    pub fn cover(&mut self, c: &mut Canvas, of: CoverOf, now_ns: u64) -> bool {
        let key = of.key();
        if c.has_image(&key) {
            return true;
        }
        if c.image_waiting(&key) || self.asked.contains(&Want::Cover(of)) {
            return false;
        }
        if self.cover_failed.get(&of).is_some_and(|t| now_ns.saturating_sub(*t) < COVER_RETRY_NS) {
            return false;
        }
        self.push(100, Want::Cover(of));
        false
    }

    /// Let go of a cover: its copy on disk, its pictures in the atlas, and what was learnt of it
    /// (its id now names another series: [`crate::known`]).
    pub fn forget_cover(&mut self, c: &mut Canvas, of: CoverOf) {
        for key in [of.key(), of.banner_key(), of.hero_key(), of.poster_key()] {
            c.forget_image(&key);
        }
        let _ = std::fs::remove_file(of.file(&self.data));
        self.cover_failed.remove(&of);
        self.hero_failed.remove(&of);
        self.cover_src.remove(&of);
        self.posters.remove(&of);
    }

    /// Make sure a cover's large copy is in the atlas or on its way. True when it can be drawn
    /// now (with [`CoverOf::hero_key`]).
    pub fn hero(&mut self, c: &mut Canvas, of: CoverOf, now_ns: u64) -> bool {
        let key = of.hero_key();
        // A poster's blur is no use without the poster (the atlas may have let one go).
        if c.has_image(&key) && (!self.posters.contains_key(&of) || c.has_image(&of.poster_key())) {
            return true;
        }
        if c.image_waiting(&key) || self.asked.contains(&Want::Hero(of)) {
            return false;
        }
        if self.hero_failed.get(&of).is_some_and(|t| now_ns.saturating_sub(*t) < COVER_RETRY_NS) {
            return false;
        }
        self.push(90, Want::Hero(of));
        false
    }

    /// A collection's picture: in the atlas, on its way (asked for now if need be), or none on the
    /// server. Kept on disk once fetched; a 404 is remembered a day.
    pub fn art(&mut self, c: &Canvas, slug: &str, now_ns: u64) -> Art {
        if slug.is_empty() || self.art_none.contains(slug) {
            return Art::None;
        }
        let key = art_key(slug);
        if c.has_image(&key) {
            return Art::Ready;
        }
        let want = Want::Art(slug.to_string());
        if c.image_waiting(&key) || self.asked.contains(&want) {
            return Art::Coming;
        }
        if self.art_failed.get(slug).is_some_and(|t| now_ns.saturating_sub(*t) < COVER_RETRY_NS) {
            return Art::None;
        }
        self.push(80, want);
        Art::Coming
    }

    /// A chapter's picture can be drawn now (with [`thumb_key`]).
    pub fn has_thumb(&self, c: &Canvas, chapter: i64) -> bool {
        c.has_image(&thumb_key(chapter))
    }

    /// The chapters whose pictures the page shows now, (chapter, the server says it has one):
    /// those not in the atlas are read from disk or asked of the server, and anything queued for
    /// pictures no longer on screen is dropped. One the server had none of is asked again after
    /// a while, or at once when the server says it has one now.
    pub fn want_thumbs(&mut self, c: &Canvas, wants: &[(i64, bool)], now_ns: u64) {
        let fresh: Vec<Want> = wants
            .iter()
            .filter(|(ch, _)| !c.has_image(&thumb_key(*ch)) && !c.image_waiting(&thumb_key(*ch)))
            .filter(|(ch, has)| self.thumb_failed.get(ch).is_none_or(|(t, had)| now_ns.saturating_sub(*t) >= THUMB_RETRY_NS || (*has && !*had)))
            .map(|&(chapter, _)| Want::Thumb { chapter })
            .collect();
        for (ch, has) in wants {
            if let Some(f) = self.thumb_failed.get_mut(ch)
                && *has
            {
                // Asked again now that the server says it has one: a second 404 waits the while.
                f.1 = true;
            }
        }
        let mut q = self.shared.q.lock().unwrap_or_else(|e| e.into_inner());
        let mut dropped = Vec::new();
        q.items.retain(|(_, _, w)| {
            let stale = matches!(w, Want::Thumb { .. }) && !fresh.contains(w);
            if stale {
                dropped.push(w.clone());
            }
            !stale
        });
        for w in dropped {
            self.asked.remove(&w);
        }
        let mut added = false;
        for w in fresh {
            if self.asked.insert(w.clone()) {
                q.seq += 1;
                let seq = q.seq;
                // Under covers and the reader's pages; the topmost first.
                q.items.push((20, seq, w));
                added = true;
            }
        }
        drop(q);
        if added {
            self.shared.cv.notify_all();
        }
    }

    /// The pages the reader wants, nearest first: (chapter, page, decode). Anything else queued
    /// for pages is dropped.
    pub fn want_pages(&mut self, wants: &[(i64, u32, bool)]) {
        self.want_pages_firm(wants, wants.len());
    }

    /// [`Self::want_pages`], where only the first `firm` are needed now (on screen or close to
    /// it): one after those that fails is not shown as failed, only asked again once it is firm
    /// (a page decoded far ahead the server can't send yet may well come when it is reached).
    pub fn want_pages_firm(&mut self, wants: &[(i64, u32, bool)], firm: usize) {
        self.reading = true;
        if wants == self.last_wants.as_slice() && firm == self.last_firm {
            return;
        }
        self.last_wants = wants.to_vec();
        self.last_firm = firm;
        let mut q = self.shared.q.lock().unwrap_or_else(|e| e.into_inner());
        let mut dropped = Vec::new();
        q.items.retain(|(_, _, w)| {
            let page = matches!(w, Want::Page { .. });
            if page {
                dropped.push(w.clone());
            }
            !page
        });
        for w in dropped {
            self.asked.remove(&w);
        }
        let count = wants.len() as i32;
        for (i, &(chapter, n, decode)) in wants.iter().enumerate() {
            let k = (chapter, n);
            let done = if decode { self.decoded.contains_key(&k) } else { self.stored.contains(&k) || self.sizes.contains_key(&k) };
            if done || self.skipped.contains(&k) || self.failed.contains_key(&k) || (i >= firm && self.ahead_failed.contains(&k)) {
                continue;
            }
            if decode && i >= firm {
                self.soft.insert(k);
            } else {
                self.soft.remove(&k);
            }
            let want = Want::Page { chapter, n, decode };
            // Being fetched already (or, for a decode, a plain fetch of it is: the decode waits).
            if self.asked.contains(&want) {
                continue;
            }
            self.asked.insert(want.clone());
            q.seq += 1;
            let seq = q.seq;
            // Decodes before plain fetches, each nearest first.
            let prio = if decode { 50 } else { 0 } + (count - i as i32).min(49);
            q.items.push((prio, seq, want));
        }
        drop(q);
        self.shared.cv.notify_all();
    }

    /// Hand what the workers finished to the canvas. True when something arrived.
    pub fn drain(&mut self, c: &mut Canvas, now_ns: u64) -> bool {
        let mut arrived = false;
        while let Ok(got) = self.rx.try_recv() {
            arrived = true;
            match got {
                Got::Cover { of, img, banner, src } => {
                    self.asked.remove(&Want::Cover(of));
                    if let Some(wh) = src {
                        self.cover_src.insert(of, wh);
                    }
                    match img {
                        Some(p) => {
                            if !c.add_image_slot(&of.key(), p.w, p.h, (COVER_W, COVER_H), p.px) && !c.image_waiting(&of.key()) {
                                self.cover_failed.insert(of, now_ns);
                            }
                            if let Some(b) = banner {
                                c.add_image(&of.banner_key(), b.w, b.h, b.px);
                            }
                        }
                        None => {
                            self.cover_failed.insert(of, now_ns);
                        }
                    }
                }
                Got::Page { chapter, n, decode, result } => {
                    self.asked.remove(&Want::Page { chapter, n, decode });
                    let k = (chapter, n);
                    match result {
                        Ok(Fetched::Img(Some(img))) => {
                            self.sizes.insert(k, (img.w, img.h));
                            self.stored.insert(k);
                            if self.reading {
                                self.decoded.insert(k, img);
                            }
                        }
                        Ok(Fetched::Img(None)) => {
                            self.stored.insert(k);
                        }
                        Ok(Fetched::Skip) => {
                            self.skipped.insert(k);
                        }
                        Err(e) if decode && !self.soft.contains(&k) => {
                            self.failed.insert(k, e);
                        }
                        Err(_) => {
                            self.ahead_failed.insert(k);
                        }
                    }
                    self.soft.remove(&k);
                    // The reader's list may now ask for something it skipped while this was out.
                    self.last_wants.clear();
                }
                Got::Hero { of, img } => {
                    self.asked.remove(&Want::Hero(of));
                    match img {
                        Some(HeroPics { main: p, poster }) => {
                            match poster {
                                Some(q) => {
                                    self.posters.insert(of, (q.w, q.h));
                                    c.add_image(&of.poster_key(), q.w, q.h, q.px);
                                }
                                None => {
                                    self.posters.remove(&of);
                                }
                            }
                            if !c.add_image(&of.hero_key(), p.w, p.h, p.px) && !c.image_waiting(&of.hero_key()) {
                                self.hero_failed.insert(of, now_ns);
                            }
                        }
                        None => {
                            self.hero_failed.insert(of, now_ns);
                        }
                    }
                }
                Got::Art { slug, img, none } => {
                    self.asked.remove(&Want::Art(slug.clone()));
                    match img {
                        Some(p) => {
                            if !c.add_image_slot(&art_key(&slug), p.w, p.h, (ART_W, ART_H), p.px) && !c.image_waiting(&art_key(&slug)) {
                                self.art_failed.insert(slug, now_ns);
                            }
                        }
                        None if none => {
                            self.art_none.insert(slug);
                        }
                        None => {
                            self.art_failed.insert(slug, now_ns);
                        }
                    }
                }
                Got::Thumb { chapter, img } => {
                    self.asked.remove(&Want::Thumb { chapter });
                    match img {
                        Some(p) => {
                            self.thumb_failed.remove(&chapter);
                            c.add_image_slot(&thumb_key(chapter), p.w, p.h, (THUMB_W, THUMB_H), p.px);
                        }
                        None => {
                            let had = self.thumb_failed.get(&chapter).is_some_and(|f| f.1);
                            self.thumb_failed.insert(chapter, (now_ns, had));
                        }
                    }
                }
                Got::Local { key, book, entry, img } => {
                    self.asked.remove(&Want::Local { key: key.clone(), book, entry });
                    match img {
                        Some(p) => {
                            self.local_sizes.insert(key.clone(), (p.w, p.h));
                            c.add_image_slot(&key, p.w, p.h, (crate::logic::PAGE_W, TILE_H), p.px);
                        }
                        None => {
                            self.local_failed.insert(key);
                        }
                    }
                }
            }
        }
        arrived
    }

    /// A page's size, once it has been decoded.
    pub fn size(&self, chapter: i64, n: u32) -> Option<(u32, u32)> {
        self.sizes.get(&(chapter, n)).copied()
    }

    pub fn page(&self, chapter: i64, n: u32) -> Option<&PageImg> {
        self.decoded.get(&(chapter, n))
    }

    /// The page is left out of the strip (a promo page, or hidden by hand).
    pub fn skipped(&self, chapter: i64, n: u32) -> bool {
        self.skipped.contains(&(chapter, n))
    }

    /// Leave a page out from now on (hidden by hand): kept on disk beside it.
    pub fn skip(&mut self, chapter: i64, n: u32) {
        self.skipped.insert((chapter, n));
        self.decoded.remove(&(chapter, n));
        // One empty file, on a tap: written here, so an Undo right after finds it to remove.
        let dir = chapter_dir(&self.data, chapter);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{n}.skip")), b"");
    }

    /// A page hidden by hand is shown again (Undo): fetched anew.
    pub fn unskip(&mut self, chapter: i64, n: u32) {
        self.skipped.remove(&(chapter, n));
        self.last_wants.clear();
        // Removed before the page is asked for again (the fetch reads it).
        let _ = std::fs::remove_file(chapter_dir(&self.data, chapter).join(format!("{n}.skip")));
    }

    /// Every page left out is asked for again (the pages hidden by hand cleared, the setting
    /// turned off): the server says again which to skip.
    pub fn clear_skips(&mut self) {
        self.skipped.clear();
        self.last_wants.clear();
        let data = self.data.clone();
        let _ = std::thread::Builder::new().name("leaf-skip".into()).spawn(move || {
            for d in std::fs::read_dir(data.join("cache")).into_iter().flatten().flatten() {
                for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
                    if f.file_name().to_string_lossy().ends_with(".skip") {
                        let _ = std::fs::remove_file(f.path());
                    }
                }
            }
        });
    }

    /// Scanlators' promo pages are skipped (the setting, on by default) or shown.
    pub fn set_promo(&mut self, skip: bool) {
        let was = !self.shared.promo.swap(!skip, std::sync::atomic::Ordering::Relaxed);
        if was != skip && !skip {
            // Shown again: what was left out is fetched (with `?promo=1`).
            self.skipped.clear();
            self.last_wants.clear();
        }
    }

    /// Screenshots: pages are drawn here (bands of colour) instead of fetched, so a shot of the
    /// reader asks the server for no page.
    pub fn set_fake_pages(&self, on: bool) {
        self.shared.fake_pages.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    /// Tests: a page the server said to skip.
    #[cfg(test)]
    pub fn fake_skip(&mut self, chapter: i64, n: u32) {
        self.skipped.insert((chapter, n));
    }

    /// Why a page could not be had, if it could not.
    pub fn failed(&self, chapter: i64, n: u32) -> Option<&str> {
        self.failed.get(&(chapter, n)).map(String::as_str)
    }

    /// The atlas key of tile `i` of page `n` of a chapter.
    pub fn tile_key(&self, chapter: i64, n: u32, i: usize) -> String {
        tile_key(chapter, n, i, self.cut)
    }

    /// Try the pages that failed again (Retry, or the server came back).
    pub fn retry_failed(&mut self) {
        self.failed.clear();
        self.ahead_failed.clear();
        self.cover_failed.clear();
        self.hero_failed.clear();
        self.thumb_failed.clear();
        self.last_wants.clear();
    }

    /// Let go of decoded pages the reader no longer needs near it.
    pub fn trim(&mut self, keep: &HashSet<(i64, u32)>) {
        self.decoded.retain(|k, _| keep.contains(k));
    }

    /// The reader closed: nothing more is fetched for it (a chapter opened by mistake is not
    /// downloaded behind your back), and its decoded pages are let go. Pages that failed are
    /// tried again when they are next read.
    pub fn leave_pages(&mut self) {
        self.want_pages(&[]);
        self.reading = false;
        self.decoded.clear();
        self.failed.clear();
        self.ahead_failed.clear();
    }

    /// Tests: a page measured as if it had been decoded.
    #[cfg(test)]
    pub fn fake_size(&mut self, chapter: i64, n: u32, w: u32, h: u32) {
        self.sizes.insert((chapter, n), (w, h));
    }

    /// Tests: a page decoded (blank tiles) as if it had come from the server.
    #[cfg(test)]
    pub fn fake_page(&mut self, chapter: i64, n: u32, w: u32, h: u32) {
        let tiles = crate::logic::tile_rows(h).into_iter().map(|(_, th)| vec![0u8; (w * th * 4) as usize]).collect();
        self.sizes.insert((chapter, n), (w, h));
        self.stored.insert((chapter, n));
        self.decoded.insert((chapter, n), PageImg { w, h, tiles });
    }

    /// Something is being fetched or decoded.
    pub fn pending(&self) -> bool {
        !self.asked.is_empty()
    }

    /// Nothing for the reader is being fetched or decoded.
    pub fn pages_pending(&self) -> bool {
        self.asked.iter().any(|w| matches!(w, Want::Page { decode: true, .. }))
    }
}

fn worker(shared: &Shared, tx: &Sender<Got>, client: &Client, data: &Path) {
    loop {
        let want = {
            let mut q = shared.q.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                let full = q.plain_out >= PLAIN_AT_ONCE;
                let plain = |w: &Want| matches!(w, Want::Page { decode: false, .. });
                if let Some(i) = (0..q.items.len()).filter(|&i| !(full && plain(&q.items[i].2))).max_by_key(|&i| (q.items[i].0, std::cmp::Reverse(q.items[i].1))) {
                    let w = q.items.swap_remove(i).2;
                    if plain(&w) {
                        q.plain_out += 1;
                    }
                    break w;
                }
                q = shared.cv.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        let was_plain = matches!(want, Want::Page { decode: false, .. });
        let server = shared.server.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &want {
            Want::Cover(of) => cover(client, server.as_ref(), data, *of),
            Want::Page { chapter, n, decode } if *chapter < 0 => Got::Page { chapter: *chapter, n: *n, decode: *decode, result: pdf_page(shared, *chapter, *n, *decode).map(Fetched::Img) },
            Want::Page { chapter, n, decode } if shared.fake_pages.load(std::sync::atomic::Ordering::Relaxed) => {
                Got::Page { chapter: *chapter, n: *n, decode: *decode, result: Ok(Fetched::Img(Some(made_up_page(*chapter, *n)))) }
            }
            Want::Page { chapter, n, decode } => Got::Page {
                chapter: *chapter,
                n: *n,
                decode: *decode,
                result: page(client, server.as_ref(), data, *chapter, *n, *decode, shared.trim.load(std::sync::atomic::Ordering::Relaxed), shared.promo.load(std::sync::atomic::Ordering::Relaxed)),
            },
            Want::Local { key, book, entry } => Got::Local { key: key.clone(), book: book.clone(), entry: entry.clone(), img: local_picture(book, entry) },
            Want::Hero(of) => Got::Hero { of: *of, img: hero(client, server.as_ref(), data, *of, shared.screen_w.load(std::sync::atomic::Ordering::Relaxed)) },
            Want::Thumb { chapter } => Got::Thumb { chapter: *chapter, img: thumb(client, server.as_ref(), data, *chapter) },
            Want::Art(slug) => art(client, server.as_ref(), data, slug),
        }));
        let got = got.unwrap_or_else(|_| match want {
            Want::Cover(of) => Got::Cover { of, img: None, banner: None, src: None },
            Want::Page { chapter, n, decode } => Got::Page { chapter, n, decode, result: Err("This page didn't decode".into()) },
            Want::Local { key, book, entry } => Got::Local { key, book, entry, img: None },
            Want::Hero(of) => Got::Hero { of, img: None },
            Want::Thumb { chapter } => Got::Thumb { chapter, img: None },
            Want::Art(slug) => Got::Art { slug, img: None, none: false },
        });
        if was_plain {
            shared.q.lock().unwrap_or_else(|e| e.into_inner()).plain_out -= 1;
            shared.cv.notify_all();
        }
        if tx.send(got).is_err() {
            return;
        }
        crate::wake::wake();
    }
}

/// The file's bytes, from disk or (then kept there) the server.
fn fetch(client: &Client, server: Option<&Server>, file: &Path, path: &str) -> Result<Vec<u8>, crate::net::Fail> {
    if let Ok(b) = std::fs::read(file)
        && !b.is_empty()
    {
        return Ok(b);
    }
    let s = server.ok_or(crate::net::Fail::Unreachable)?;
    let b = client.get(s, path)?;
    let _ = write_atomic(file, &b);
    Ok(b)
}

fn cover(client: &Client, server: Option<&Server>, data: &Path, of: CoverOf) -> Got {
    let file = of.file(data);
    // A book's cover is only ever its file's.
    let server = if matches!(of, CoverOf::Book(_)) { None } else { server };
    let Ok(bytes) = fetch(client, server, &file, &of.path()) else { return Got::Cover { of, img: None, banner: None, src: None } };
    let Ok(img) = image::load_from_memory(&bytes) else {
        let _ = std::fs::remove_file(&file);
        return Got::Cover { of, img: None, banner: None, src: None };
    };
    let src = Some((img.width(), img.height()));
    let filled = img.resize_to_fill(COVER_W, COVER_H, image::imageops::FilterType::Triangle).to_rgba8();
    let small = image::imageops::resize(&filled, BANNER.0, BANNER.1, image::imageops::FilterType::Triangle);
    let (w, h) = filled.dimensions();
    Got::Cover {
        of,
        img: Some(Pixels { w, h, px: filled.into_raw() }),
        banner: Some(Pixels { w: BANNER.0, h: BANNER.1, px: small.into_raw() }),
        src,
    }
}

/// A cover for the spotlight on a screen `screen_w` pixels wide. Big enough to fill it within
/// [`MAX_UPSCALE`]: cut to its shape (from the top third, where faces are, when it is taller),
/// at its own resolution or the screen's, whichever is less. Smaller: the cover as a poster at
/// its own size, and a blurred, darkened copy to fill the spotlight behind it.
fn hero(client: &Client, server: Option<&Server>, data: &Path, of: CoverOf, screen_w: u32) -> Option<HeroPics> {
    let file = of.file(data);
    let server = if matches!(of, CoverOf::Book(_)) { None } else { server };
    let bytes = fetch(client, server, &file, &of.path()).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    Some(hero_pics(&img, screen_w))
}

fn hero_pics(img: &image::RgbaImage, screen_w: u32) -> HeroPics {
    use image::imageops::{FilterType, crop_imm, resize};
    let (w0, h0) = (img.width().max(1), img.height().max(1));
    // The part the spotlight's shape keeps, in the cover's own pixels.
    let cw = (w0 as f32).min(h0 as f32 * HERO_ASPECT).floor().max(1.0) as u32;
    let ch = ((cw as f32 / HERO_ASPECT).round() as u32).clamp(1, h0);
    let screen_w = screen_w.clamp(320, 1440);
    if cw as f32 * MAX_UPSCALE >= screen_w as f32 {
        let (x, y) = ((w0 - cw) / 2, ((h0 - ch) as f32 * 0.3) as u32);
        let cut = crop_imm(img, x, y, cw, ch).to_image();
        let out = if cw > screen_w { resize(&cut, screen_w, ((screen_w as f32 / HERO_ASPECT).round() as u32).max(1), FilterType::Triangle) } else { cut };
        let (w, h) = out.dimensions();
        return HeroPics { main: Pixels { w, h, px: out.into_raw() }, poster: None };
    }
    let poster = if w0 > POSTER_MAX_W { resize(img, POSTER_MAX_W, ((h0 as f32 * POSTER_MAX_W as f32 / w0 as f32).round() as u32).max(1), FilterType::Triangle) } else { img.clone() };
    let (pw, ph) = poster.dimensions();
    let (bw, bh) = (BACKDROP_W, ((BACKDROP_W as f32 * h0 as f32 / w0 as f32).round() as u32).max(1));
    let mut back = resize(img, bw, bh, FilterType::Triangle);
    blur(&mut back, 7, 3);
    for p in back.pixels_mut() {
        for c in 0..3 {
            p.0[c] = (p.0[c] as f32 * 0.6) as u8;
        }
        p.0[3] = 255;
    }
    HeroPics { main: Pixels { w: bw, h: bh, px: back.into_raw() }, poster: Some(Pixels { w: pw, h: ph, px: poster.into_raw() }) }
}

/// A box blur of radius `r`, `passes` times across and down (three is close to a Gaussian),
/// edges held.
fn blur(img: &mut image::RgbaImage, r: u32, passes: u32) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let r = r as isize;
    let mut buf: Vec<[f32; 3]> = img.pixels().map(|p| [p.0[0] as f32, p.0[1] as f32, p.0[2] as f32]).collect();
    let mut tmp = buf.clone();
    let n = (2 * r + 1) as f32;
    for _ in 0..passes {
        for (len, count, step, stride) in [(w, h, 1usize, w), (h, w, w, 1usize)] {
            for line in 0..count {
                let at = |i: isize| line * stride + (i.clamp(0, len as isize - 1) as usize) * step;
                let mut sum = [0.0f32; 3];
                for i in -r..=r {
                    let p = buf[at(i)];
                    for c in 0..3 {
                        sum[c] += p[c];
                    }
                }
                for i in 0..len as isize {
                    tmp[at(i)] = [sum[0] / n, sum[1] / n, sum[2] / n];
                    let (add, sub) = (buf[at(i + r + 1)], buf[at(i - r)]);
                    for c in 0..3 {
                        sum[c] += add[c] - sub[c];
                    }
                }
            }
            std::mem::swap(&mut buf, &mut tmp);
        }
    }
    for (p, b) in img.pixels_mut().zip(buf) {
        p.0[0] = b[0].round() as u8;
        p.0[1] = b[1].round() as u8;
        p.0[2] = b[2].round() as u8;
    }
}

/// A collection's picture: from disk (`covers/tag-<slug>.webp`), else the server's, kept. A 404 is
/// marked on disk (`tag-<slug>.none`) and not asked again for a day.
fn art(client: &Client, server: Option<&Server>, data: &Path, slug: &str) -> Got {
    let dir = data.join("covers");
    let (file, none) = (dir.join(format!("tag-{slug}.webp")), dir.join(format!("tag-{slug}.none")));
    let recent_none = std::fs::metadata(&none).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_millis() < ART_NONE_MS);
    let bytes = match std::fs::read(&file) {
        Ok(b) if !b.is_empty() => b,
        _ if recent_none => return Got::Art { slug: slug.to_string(), img: None, none: true },
        _ => {
            let Some(s) = server else { return Got::Art { slug: slug.to_string(), img: None, none: false } };
            match client.get(s, &format!("/v1/recs/tagart/{slug}")) {
                Ok(b) => {
                    let _ = write_atomic(&file, &b);
                    let _ = std::fs::remove_file(&none);
                    b
                }
                Err(crate::net::Fail::Answer(404, _)) => {
                    let _ = write_atomic(&none, b"");
                    return Got::Art { slug: slug.to_string(), img: None, none: true };
                }
                Err(_) => return Got::Art { slug: slug.to_string(), img: None, none: false },
            }
        }
    };
    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i.resize_to_fill(ART_W, ART_H, image::imageops::FilterType::Triangle).to_rgba8(),
        Err(_) => {
            let _ = std::fs::remove_file(&file);
            return Got::Art { slug: slug.to_string(), img: None, none: false };
        }
    };
    Got::Art { slug: slug.to_string(), img: Some(Pixels { w: img.width(), h: img.height(), px: img.into_raw() }), none: false }
}

/// A chapter's picture: from disk, else the server's (`/v1/chapters/{id}/thumb`, about 10 KB of
/// WebP, kept on disk for good). None while the server has none (404): the row shows its cloud.
/// No page is ever fetched for a picture.
fn thumb(client: &Client, server: Option<&Server>, data: &Path, chapter: i64) -> Option<Pixels> {
    let bytes = fetch(client, server, &thumb_file(data, chapter), &format!("/v1/chapters/{chapter}/thumb")).ok()?;
    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i.to_rgba8(),
        Err(_) => {
            let _ = std::fs::remove_file(thumb_file(data, chapter));
            return None;
        }
    };
    Some(Pixels { w: img.width(), h: img.height(), px: img.into_raw() })
}

/// The page cache's chapters: (chapter, bytes, when last used in ms).
fn cache_dirs(data: &Path) -> Vec<(i64, u64, i64)> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(data.join("cache")) else { return out };
    for d in dirs.flatten() {
        let Some(chapter) = d.file_name().to_str().and_then(|n| n.parse::<i64>().ok()) else { continue };
        let path = d.path();
        let mut bytes = 0u64;
        let mut newest = 0i64;
        for f in std::fs::read_dir(&path).into_iter().flatten().flatten() {
            if let Ok(m) = f.metadata() {
                bytes += m.len();
                let at = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
                newest = newest.max(at);
            }
        }
        let used = std::fs::read_to_string(path.join(".used")).ok().and_then(|t| t.trim().parse::<i64>().ok()).unwrap_or(newest);
        out.push((chapter, bytes, used));
    }
    out
}

/// A PDF's page, drawn by the platform at the strip's width (none on the desktop).
fn pdf_page(shared: &Shared, chapter: i64, n: u32, decode: bool) -> Result<Option<PageImg>, String> {
    if !decode {
        return Ok(None);
    }
    let path = shared.pdfs.lock().unwrap_or_else(|e| e.into_inner()).get(&chapter).cloned().ok_or("This PDF isn't open")?;
    let host = shared.host.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("PDFs open on the phone")?;
    let (w, h, px) = host.pdf_page(&path, n, crate::logic::PAGE_W).ok_or("This page didn't draw")?;
    if w == 0 || h == 0 || px.len() != (w * h * 4) as usize {
        return Err("This page didn't draw".into());
    }
    let row = (w * 4) as usize;
    let tiles = crate::logic::tile_rows(h).into_iter().map(|(y, th)| px[y as usize * row..(y + th) as usize * row].to_vec()).collect();
    Ok(Some(PageImg { w, h, tiles }))
}

/// A picture inside a book, scaled to fit one tile's room.
fn local_picture(book: &Path, entry: &str) -> Option<Pixels> {
    let bytes = crate::epub::read_entry(book, entry)?;
    let img = image::load_from_memory(&bytes).ok()?;
    let (w0, h0) = (img.width().max(1), img.height().max(1));
    let scale = (crate::logic::PAGE_W as f32 / w0 as f32).min(TILE_H as f32 / h0 as f32).min(1.0);
    let (w, h) = (((w0 as f32 * scale) as u32).max(1), ((h0 as f32 * scale) as u32).max(1));
    let rgba = if (w, h) == (w0, h0) { img.to_rgba8() } else { image::imageops::resize(&img.to_rgba8(), w, h, image::imageops::FilterType::Triangle) };
    Some(Pixels { w, h, px: rgba.into_raw() })
}

/// The folder a chapter's pages are kept in.
pub fn chapter_dir(data: &Path, chapter: i64) -> PathBuf {
    data.join("cache").join(chapter.to_string())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Mark a chapter used now, for the cache's pruning.
pub fn touch_chapter(data: &Path, chapter: i64) {
    let dir = chapter_dir(data, chapter);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(".used"), now_ms().to_string());
}

/// A page: from disk, else the server (then kept on disk). A page the server skips (a scanlator's
/// promo page, `x-leaf-skip`) is marked on disk (`<n>.skip`) and not asked for again, unless
/// promo pages are to be shown (`promo`: asked with `?promo=1`, the mark ignored and dropped).
#[allow(clippy::too_many_arguments)]
fn page(client: &Client, server: Option<&Server>, data: &Path, chapter: i64, n: u32, decode: bool, trim: bool, promo: bool) -> Result<Fetched, String> {
    let dir = chapter_dir(data, chapter);
    let file = dir.join(n.to_string());
    let mark = dir.join(format!("{n}.skip"));
    if !promo && mark.is_file() && !file.is_file() {
        return Ok(Fetched::Skip);
    }
    // Downloaded to this phone: read from there, with no network at all.
    let saved = crate::offline::chapter_dir(data, chapter);
    if let Ok(b) = std::fs::read(saved.join(n.to_string()))
        && !b.is_empty()
    {
        return if decode { decode_page(&b, trim).map(|p| Fetched::Img(Some(p))) } else { Ok(Fetched::Img(None)) };
    }
    if !promo && saved.join(format!("{n}.skip")).is_file() {
        return Ok(Fetched::Skip);
    }
    if !decode && file.is_file() {
        return Ok(Fetched::Img(None));
    }
    let fresh = !file.is_file();
    let words = |e: crate::net::Fail| match e {
        crate::net::Fail::Unreachable => "Can't reach your server for this page".to_string(),
        e => e.words("your server"),
    };
    let bytes = match std::fs::read(&file).ok().filter(|b| !b.is_empty()) {
        Some(b) => b,
        None => {
            let s = server.ok_or_else(|| words(crate::net::Fail::Unreachable))?;
            let path = format!("/v1/chapters/{chapter}/pages/{n}{}", if promo { "?promo=1" } else { "" });
            match client.get_page(s, &path).map_err(words)? {
                Some(b) => {
                    let _ = write_atomic(&file, &b);
                    if promo {
                        let _ = std::fs::remove_file(&mark);
                    }
                    b
                }
                None => {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(&mark, b"");
                    if !dir.join(".used").is_file() {
                        touch_chapter(data, chapter);
                    }
                    return Ok(Fetched::Skip);
                }
            }
        }
    };
    if fresh && !dir.join(".used").is_file() {
        touch_chapter(data, chapter);
    }
    if !decode {
        return Ok(Fetched::Img(None));
    }
    decode_page(&bytes, trim).map(|p| Fetched::Img(Some(p))).inspect_err(|_| {
        // A broken copy on disk is fetched again next time.
        let _ = std::fs::remove_file(&file);
    })
}

/// A made-up page for screenshots: soft bands of colour, a panel's frame, by chapter and page.
fn made_up_page(chapter: i64, n: u32) -> PageImg {
    let (w, h) = (crate::logic::PAGE_W, crate::logic::PAGE_W * 3 / 2);
    let seed = (chapter.unsigned_abs() as u32).wrapping_mul(31).wrapping_add(n * 17);
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let band = (y * 5 / h + seed) % 5;
        let base = [[38u8, 52, 74], [92, 64, 88], [40, 86, 92], [120, 96, 70], [60, 70, 110]][band as usize];
        let edge = x < 24 || x >= w - 24 || (y % (h / 3)) < 12;
        let shade = (x as f32 / w as f32 * 30.0) as u8;
        if edge { image::Rgba([235, 232, 226, 255]) } else { image::Rgba([base[0].saturating_add(shade), base[1], base[2].saturating_add(shade / 2), 255]) }
    });
    let raw = img.into_raw();
    let row = (w * 4) as usize;
    let tiles = crate::logic::tile_rows(h).into_iter().map(|(y, th)| raw[y as usize * row..(y + th) as usize * row].to_vec()).collect();
    PageImg { w, h, tiles }
}

/// The most one page's decode may allocate: a 1600 × 30000 webtoon strip fits, a malformed
/// header asking for gigabytes does not.
const PAGE_MAX_ALLOC: u64 = 256 << 20;
/// A page with more pixels than this decodes alone: three of them at once could take the phone's
/// memory.
const BIG_PAGE_PX: u64 = 12_000_000;
static BIG_DECODE: Mutex<()> = Mutex::new(());

/// A page's bytes decoded, scaled down to at most [`PAGE_W`] wide, its blank gutters cut short
/// when `trim` ([`crate::logic::gutter_cuts`]), and cut into tiles.
pub fn decode_page(bytes: &[u8], trim: bool) -> Result<PageImg, String> {
    let bad = |_| "This page didn't decode".to_string();
    let reader = || -> std::io::Result<image::ImageReader<std::io::Cursor<&[u8]>>> {
        let mut r = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(PAGE_MAX_ALLOC);
        r.limits(limits);
        Ok(r)
    };
    let (w0, h0) = reader().map_err(bad)?.into_dimensions().map_err(|_| "This page didn't decode".to_string())?;
    let _alone = (w0 as u64 * h0 as u64 > BIG_PAGE_PX).then(|| BIG_DECODE.lock().unwrap_or_else(|e| e.into_inner()));
    let img = reader().map_err(bad)?.decode().map_err(|_| "This page didn't decode".to_string())?;
    let (w, h) = crate::logic::scaled_size(w0, h0);
    // Scaled straight from the decoded picture: no full-size RGBA copy first.
    let rgba = if (w, h) == (w0, h0) { img.into_rgba8() } else { image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle) };
    let raw = rgba.into_raw();
    let row = (w * 4) as usize;
    let (cut_top, cut_bottom) = if trim { crate::logic::gutter_cuts(&raw, w, h) } else { (0, 0) };
    let h = h - cut_top - cut_bottom;
    let raw = &raw[cut_top as usize * row..(cut_top + h) as usize * row];
    let tiles = crate::logic::tile_rows(h).into_iter().map(|(y, th)| raw[y as usize * row..(y + th) as usize * row].to_vec()).collect();
    Ok(PageImg { w, h, tiles })
}

/// Tile `i`'s height in a page `h` tall.
pub fn tile_h(h: u32, i: usize) -> u32 {
    TILE_H.min(h.saturating_sub(i as u32 * TILE_H))
}

/// Trim the page cache on a thread of its own: chapters read over a week ago, then the oldest
/// until it fits ([`crate::logic::prune_choice`]). Chapters in `keep` stay.
pub fn prune_cache(data: PathBuf, keep: Vec<i64>) {
    let _ = std::thread::Builder::new().name("leaf-prune".into()).spawn(move || {
        let root = data.join("cache");
        let cached: Vec<crate::logic::CachedChapter> = cache_dirs(&data).into_iter().map(|(chapter, bytes, used_ms)| crate::logic::CachedChapter { chapter, bytes, used_ms }).collect();
        for chapter in crate::logic::prune_choice(&cached, now_ms(), crate::logic::CACHE_CAP, &keep) {
            let _ = std::fs::remove_dir_all(root.join(chapter.to_string()));
        }
        // Pictures the phone once cut from pages itself (`<chapter>.jpg`, before the server made
        // them): nothing reads them now.
        for e in std::fs::read_dir(data.join("thumbs")).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().ends_with(".jpg") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tags_picture_is_named_by_its_slug() {
        assert_eq!(tag_slug("Martial Arts"), "martial-arts");
        assert_eq!(tag_slug("Post-Apocalyptic"), "post-apocalyptic");
        assert_eq!(tag_slug(" Boys' Love "), "boys-love");
        assert_eq!(tag_slug("Murim"), "murim");
        assert_eq!(tag_slug("Age  Regression"), "age-regression");
    }

    #[test]
    fn a_small_cover_is_a_poster_and_a_big_one_fills_without_upscaling() {
        // 400 px wide on a 1080 px screen would be stretched 2.7 times: a poster at its own size
        // and a small blurred, darkened copy.
        let small = image::RgbaImage::from_fn(400, 600, |x, _| image::Rgba([if x < 200 { 250 } else { 0 }, 100, 50, 255]));
        let h = hero_pics(&small, 1080);
        let p = h.poster.as_ref().expect("a poster");
        assert_eq!((p.w, p.h), (400, 600));
        assert_eq!(h.main.w, BACKDROP_W);
        let row = (h.main.h / 2 * h.main.w * 4) as usize;
        let mid = &h.main.px[row + (h.main.w / 2 * 4) as usize..][..3];
        assert!(mid[0] > 40 && mid[0] < 110, "the halves blur together and darken: {mid:?}");
        // 900 px wide fills a 1080 px screen at 1.2 times: cut to shape at its own size.
        let big = image::RgbaImage::from_pixel(900, 1400, image::Rgba([1, 2, 3, 255]));
        let h = hero_pics(&big, 1080);
        assert!(h.poster.is_none());
        assert_eq!(h.main.w, 900);
        assert_eq!(h.main.h, (900.0 / HERO_ASPECT).round() as u32);
        // Bigger than the screen: brought down to its width, never up.
        let h = hero_pics(&image::RgbaImage::from_pixel(2000, 3000, image::Rgba([1, 2, 3, 255])), 1080);
        assert_eq!(h.main.w, 1080);
    }

    #[test]
    fn a_tall_page_decodes_into_tiles() {
        // A 1000 × 2600 PNG: scaled to 800 × 2080, three tiles.
        let img = image::RgbaImage::from_pixel(1000, 2600, image::Rgba([10, 200, 30, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        let p = decode_page(&bytes, true).unwrap();
        assert_eq!((p.w, p.h), (800, 2080));
        assert_eq!(p.tiles.len(), 3);
        assert_eq!(p.tiles[0].len(), 800 * 1020 * 4);
        assert_eq!(p.tiles[2].len(), 800 * 40 * 4);
        assert_eq!((tile_h(2080, 0), tile_h(2080, 2)), (1020, 40));
        assert!(decode_page(b"not an image", true).is_err());
        // A PNG header claiming 60000 × 60000 is refused before anything that size is made.
        let mut huge = Vec::new();
        image::RgbaImage::new(1, 1).write_to(&mut std::io::Cursor::new(&mut huge), image::ImageFormat::Png).unwrap();
        huge[16..20].copy_from_slice(&60_000u32.to_be_bytes());
        huge[20..24].copy_from_slice(&60_000u32.to_be_bytes());
        // The header's checksum made right again, so it is the size that is refused.
        let crc = huge[12..29].iter().fold(!0u32, |mut c, &b| {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
            c
        });
        huge[29..33].copy_from_slice(&(!crc).to_be_bytes());
        assert_eq!(image::ImageReader::new(std::io::Cursor::new(&huge)).with_guessed_format().unwrap().into_dimensions().unwrap(), (60_000, 60_000));
        assert!(decode_page(&huge, true).is_err());
    }

    #[test]
    fn a_page_decodes_without_its_long_white_top() {
        // 600 × 500: 120 white rows over the art. Trimmed: 16 of them stay.
        let img = image::RgbaImage::from_fn(600, 500, |_, y| if y < 120 { image::Rgba([255, 255, 255, 255]) } else { image::Rgba([90, 30, 160, 255]) });
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        let p = decode_page(&bytes, true).unwrap();
        assert_eq!((p.w, p.h), (600, 396));
        assert_eq!(&p.tiles[0][..4], &[255, 255, 255, 255]);
        assert_eq!(&p.tiles[0][16 * 600 * 4..16 * 600 * 4 + 4], &[90, 30, 160, 255]);
        assert_eq!(decode_page(&bytes, false).unwrap().h, 500, "the setting off keeps the page whole");
    }

    #[test]
    fn queued_pages_go_when_the_reader_moves_or_closes() {
        // A server that takes requests and never answers: the workers hold the first three, the
        // rest stay queued, so what is dropped from the queue can be seen.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let dir = std::env::temp_dir().join(format!("leaf-images-hold-{}", std::process::id()));
        let mut im = Images::new(dir.clone());
        im.set_server(Some(Server { base, token: None, alt: None }));
        let queued = |im: &Images| -> Vec<Want> { im.shared.q.lock().unwrap().items.iter().map(|i| i.2.clone()).collect() };
        let old: Vec<(i64, u32, bool)> = (0..8).map(|n| (1, n, true)).collect();
        im.want_pages(&old);
        assert!(queued(&im).iter().any(|w| matches!(w, Want::Page { chapter: 1, .. })), "some of chapter 1 waits its turn");
        im.want_pages(&[(5, 0, true), (5, 1, true), (5, 2, true), (5, 3, true)]);
        assert!(queued(&im).iter().all(|w| !matches!(w, Want::Page { chapter: 1, .. })), "the reader moved: chapter 1 is dropped");
        im.leave_pages();
        assert!(queued(&im).iter().all(|w| !matches!(w, Want::Page { .. })), "the reader closed: nothing is fetched for it");
        drop(listener);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pages_only_kept_ahead_never_hold_every_worker() {
        // A server that takes requests and never answers.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let dir = std::env::temp_dir().join(format!("leaf-images-plain-{}", std::process::id()));
        let mut im = Images::new(dir.clone());
        im.set_server(Some(Server { base, token: None, alt: None }));
        let queued = |im: &Images| -> Vec<Want> { im.shared.q.lock().unwrap().items.iter().map(|i| i.2.clone()).collect() };
        let plain: Vec<(i64, u32, bool)> = (0..6).map(|n| (2, n, false)).collect();
        im.want_pages(&plain);
        let settled = |im: &Images, left: usize| (0..200).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(5));
            queued(im).len() == left
        });
        assert!(settled(&im, 6 - PLAIN_AT_ONCE), "only {PLAIN_AT_ONCE} are taken: {:?}", queued(&im));
        // The page coming on screen goes at once, on the worker kept free.
        let mut wants = vec![(1, 0, true)];
        wants.extend(plain);
        im.want_pages(&wants);
        assert!(settled(&im, 6 - PLAIN_AT_ONCE), "the page shown is taken: {:?}", queued(&im));
        assert!(!queued(&im).contains(&Want::Page { chapter: 1, n: 0, decode: true }));
        im.leave_pages();
        drop(listener);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_queue_follows_the_reader() {
        let dir = std::env::temp_dir().join(format!("leaf-images-test-{}", std::process::id()));
        let mut im = Images::new(dir.clone());
        // No server: every fetch fails fast. What matters here is what is asked.
        im.want_pages(&[(1, 0, true), (1, 1, true), (2, 0, false)]);
        assert!(im.pending());
        // The reader moved on: what is still queued for the old place is dropped.
        im.want_pages(&[(5, 0, true)]);
        let queued: Vec<Want> = im.shared.q.lock().unwrap().items.iter().map(|i| i.2.clone()).collect();
        assert!(queued.iter().all(|w| !matches!(w, Want::Page { chapter: 1 | 2, .. })));
        // Everything answers (failed, with no server) and nothing is left waiting.
        let mut c = Canvas::new(crate::app::FONTS);
        for _ in 0..500 {
            im.drain(&mut c, 0);
            if !im.pending() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!im.pending());
        assert!(im.failed(5, 0).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}

