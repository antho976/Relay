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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoverOf {
    /// A followed series' cover (`/v1/series/{id}/cover`).
    Series(i64),
    /// A search result's (`/v1/covers/{manga_id}`).
    Manga(i64),
    /// A book's, taken from its file at import (never from the server).
    Book(i64),
}

impl CoverOf {
    pub fn key(self) -> String {
        match self {
            CoverOf::Series(id) => format!("c:s{id}"),
            CoverOf::Manga(id) => format!("c:m{id}"),
            CoverOf::Book(id) => format!("c:k{id}"),
        }
    }
    pub fn banner_key(self) -> String {
        format!("b:{}", self.key())
    }
    fn path(self) -> String {
        match self {
            CoverOf::Series(id) => format!("/v1/series/{id}/cover"),
            CoverOf::Manga(id) => format!("/v1/covers/{id}"),
            CoverOf::Book(_) => String::new(),
        }
    }
    pub fn file(self, data: &Path) -> PathBuf {
        data.join("covers").join(self.key().replace(':', "-"))
    }
}

/// The atlas key of tile `i` of a page.
pub fn tile_key(chapter: i64, n: u32, i: usize) -> String {
    format!("p{chapter}:{n}:{i}")
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Want {
    Cover(CoverOf),
    /// `decode` false: only to have it on disk, for later (the next chapter).
    Page { chapter: i64, n: u32, decode: bool },
    /// A picture inside a book's file (an EPUB's figure), by its atlas key.
    Local { key: String, book: PathBuf, entry: String },
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

enum Got {
    Cover { of: CoverOf, img: Option<Pixels>, banner: Option<Pixels> },
    Page { chapter: i64, n: u32, decode: bool, result: Result<Option<PageImg>, String> },
    Local { key: String, book: PathBuf, entry: String, img: Option<Pixels> },
}

struct Queue {
    /// Priority (higher first), then the order asked.
    items: Vec<(i32, u64, Want)>,
    seq: u64,
}

struct Shared {
    q: Mutex<Queue>,
    cv: Condvar,
    server: Mutex<Option<Server>>,
    /// PDFs open in the strip reader: their pages come from the platform's renderer, under the
    /// chapter id `-book`.
    pdfs: Mutex<HashMap<i64, PathBuf>>,
    host: Mutex<Option<std::sync::Arc<dyn crate::app::Host>>>,
}

/// How many fetches and decodes run at once.
const WORKERS: usize = 3;
/// A cover that failed is asked for again after this long.
const COVER_RETRY_NS: u64 = 30_000_000_000;

pub struct Images {
    shared: Arc<Shared>,
    rx: Receiver<Got>,
    /// Queued or being fetched.
    asked: HashSet<Want>,
    cover_failed: HashMap<CoverOf, u64>,
    /// Every page's size once decoded (scaled), kept for good.
    sizes: HashMap<(i64, u32), (u32, u32)>,
    decoded: HashMap<(i64, u32), PageImg>,
    failed: HashMap<(i64, u32), String>,
    stored: HashSet<(i64, u32)>,
    last_wants: Vec<(i64, u32, bool)>,
    /// Pictures inside books: their sizes once decoded, and the ones that would not.
    local_sizes: HashMap<String, (u32, u32)>,
    local_failed: HashSet<String>,
}

impl Images {
    pub fn new(data: PathBuf) -> Images {
        let shared = Arc::new(Shared {
            q: Mutex::new(Queue { items: Vec::new(), seq: 0 }),
            cv: Condvar::new(),
            server: Mutex::new(None),
            pdfs: Mutex::new(HashMap::new()),
            host: Mutex::new(None),
        });
        let (tx, rx) = channel();
        let client = Client::new();
        for i in 0..WORKERS {
            let (shared, tx, client, data) = (shared.clone(), tx.clone(), client.clone(), data.clone());
            std::thread::Builder::new().name(format!("leaf-img-{i}")).spawn(move || worker(&shared, &tx, &client, &data)).expect("an image thread starts");
        }
        Images {
            shared,
            rx,
            asked: HashSet::new(),
            cover_failed: HashMap::new(),
            sizes: HashMap::new(),
            decoded: HashMap::new(),
            failed: HashMap::new(),
            stored: HashSet::new(),
            last_wants: Vec::new(),
            local_sizes: HashMap::new(),
            local_failed: HashSet::new(),
        }
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

    /// The pages the reader wants, nearest first: (chapter, page, decode). Anything else queued
    /// for pages is dropped.
    pub fn want_pages(&mut self, wants: &[(i64, u32, bool)]) {
        if wants == self.last_wants.as_slice() {
            return;
        }
        self.last_wants = wants.to_vec();
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
            if done || self.failed.contains_key(&k) {
                continue;
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
                Got::Cover { of, img, banner } => {
                    self.asked.remove(&Want::Cover(of));
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
                        Ok(Some(img)) => {
                            self.sizes.insert(k, (img.w, img.h));
                            self.stored.insert(k);
                            self.decoded.insert(k, img);
                        }
                        Ok(None) => {
                            self.stored.insert(k);
                        }
                        Err(e) => {
                            self.failed.insert(k, e);
                        }
                    }
                    // The reader's list may now ask for something it skipped while this was out.
                    self.last_wants.clear();
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

    /// Why a page could not be had, if it could not.
    pub fn failed(&self, chapter: i64, n: u32) -> Option<&str> {
        self.failed.get(&(chapter, n)).map(String::as_str)
    }

    /// Try the pages that failed again (Retry, or the server came back).
    pub fn retry_failed(&mut self) {
        self.failed.clear();
        self.cover_failed.clear();
        self.last_wants.clear();
    }

    /// Let go of decoded pages the reader no longer needs near it.
    pub fn trim(&mut self, keep: &HashSet<(i64, u32)>) {
        self.decoded.retain(|k, _| keep.contains(k));
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
                if let Some(i) = (0..q.items.len()).max_by_key(|&i| (q.items[i].0, std::cmp::Reverse(q.items[i].1))) {
                    break q.items.swap_remove(i).2;
                }
                q = shared.cv.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        let server = shared.server.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &want {
            Want::Cover(of) => cover(client, server.as_ref(), data, *of),
            Want::Page { chapter, n, decode } if *chapter < 0 => Got::Page { chapter: *chapter, n: *n, decode: *decode, result: pdf_page(shared, *chapter, *n, *decode) },
            Want::Page { chapter, n, decode } => Got::Page { chapter: *chapter, n: *n, decode: *decode, result: page(client, server.as_ref(), data, *chapter, *n, *decode) },
            Want::Local { key, book, entry } => Got::Local { key: key.clone(), book: book.clone(), entry: entry.clone(), img: local_picture(book, entry) },
        }));
        let got = got.unwrap_or_else(|_| match want {
            Want::Cover(of) => Got::Cover { of, img: None, banner: None },
            Want::Page { chapter, n, decode } => Got::Page { chapter, n, decode, result: Err("This page didn't decode".into()) },
            Want::Local { key, book, entry } => Got::Local { key, book, entry, img: None },
        });
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
    let Ok(bytes) = fetch(client, server, &file, &of.path()) else { return Got::Cover { of, img: None, banner: None } };
    let Ok(img) = image::load_from_memory(&bytes) else {
        let _ = std::fs::remove_file(&file);
        return Got::Cover { of, img: None, banner: None };
    };
    let filled = img.resize_to_fill(COVER_W, COVER_H, image::imageops::FilterType::Triangle).to_rgba8();
    let small = image::imageops::resize(&filled, BANNER.0, BANNER.1, image::imageops::FilterType::Triangle);
    let (w, h) = filled.dimensions();
    Got::Cover {
        of,
        img: Some(Pixels { w, h, px: filled.into_raw() }),
        banner: Some(Pixels { w: BANNER.0, h: BANNER.1, px: small.into_raw() }),
    }
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

fn page(client: &Client, server: Option<&Server>, data: &Path, chapter: i64, n: u32, decode: bool) -> Result<Option<PageImg>, String> {
    let dir = chapter_dir(data, chapter);
    let file = dir.join(n.to_string());
    if !decode && file.is_file() {
        return Ok(None);
    }
    let fresh = !file.is_file();
    let bytes = fetch(client, server, &file, &format!("/v1/chapters/{chapter}/pages/{n}")).map_err(|e| match e {
        crate::net::Fail::Unreachable => "Can't reach your server for this page".to_string(),
        e => e.words("your server"),
    })?;
    if fresh && !dir.join(".used").is_file() {
        touch_chapter(data, chapter);
    }
    if !decode {
        return Ok(None);
    }
    decode_page(&bytes).map(Some).inspect_err(|_| {
        // A broken copy on disk is fetched again next time.
        let _ = std::fs::remove_file(&file);
    })
}

/// A page's bytes decoded, scaled down to at most [`PAGE_W`] wide, and cut into tiles.
pub fn decode_page(bytes: &[u8]) -> Result<PageImg, String> {
    let img = image::load_from_memory(bytes).map_err(|_| "This page didn't decode".to_string())?;
    let (w0, h0) = (img.width(), img.height());
    let (w, h) = crate::logic::scaled_size(w0, h0);
    let rgba = if (w, h) == (w0, h0) { img.to_rgba8() } else { image::imageops::resize(&img.to_rgba8(), w, h, image::imageops::FilterType::Triangle) };
    let raw = rgba.into_raw();
    let row = (w * 4) as usize;
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
        let Ok(dirs) = std::fs::read_dir(&root) else { return };
        let mut cached = Vec::new();
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
            cached.push(crate::logic::CachedChapter { chapter, bytes, used_ms: used });
        }
        for chapter in crate::logic::prune_choice(&cached, now_ms(), crate::logic::CACHE_CAP, &keep) {
            let _ = std::fs::remove_dir_all(root.join(chapter.to_string()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tall_page_decodes_into_tiles() {
        // A 1000 × 2600 PNG: scaled to 800 × 2080, three tiles.
        let img = image::RgbaImage::from_pixel(1000, 2600, image::Rgba([10, 200, 30, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        let p = decode_page(&bytes).unwrap();
        assert_eq!((p.w, p.h), (800, 2080));
        assert_eq!(p.tiles.len(), 3);
        assert_eq!(p.tiles[0].len(), 800 * 1020 * 4);
        assert_eq!(p.tiles[2].len(), 800 * 40 * 4);
        assert_eq!((tile_h(2080, 0), tile_h(2080, 2)), (1020, 40));
        assert!(decode_page(b"not an image").is_err());
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
