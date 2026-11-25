//! Chapter pictures on the server (`GET /v1/chapters/{id}/thumb`): a 240 × 180 WebP cut from the
//! most colourful 4:3 crop of up to three of the chapter's pages, read from Suwayomi's downloaded
//! files only. A chapter is never fetched from its source for a picture: not downloaded, no
//! picture. Made once and kept forever in `data/thumbs/<chapter>.webp`.
//!
//! The scoring is the phone's (`leaf-app/src/thumb.rs`), ported as it is so both choose alike: a
//! page is scored on a grid of small blocks, colour (saturation) and contrast (spread of
//! lightness) count for a crop, near-white counts hard against it (bubbles, gutters, blank
//! panels) and near-black a little (black gutters, letterboxing).

use std::path::{Path, PathBuf};

use leaf_proto::norm_title;

use crate::autopilot::{Ctx, log, now_ms};
use crate::suwa::{Ch, Manga};

/// A chapter whose picture could not be made is tried again after this long.
const RETRY_AFTER: i64 = 7 * 86_400_000;

/// The picture's size.
pub const THUMB_W: u32 = 240;
pub const THUMB_H: u32 = 180;
/// Pages are looked at this wide.
pub const LOOK_W: u32 = 400;
/// The most pages tried per chapter.
pub const TRIES: usize = 3;
const QUALITY: f32 = 72.0;

/// Side of a scoring block, in pixels of the page as looked at.
const BLOCK: u32 = 4;

/// A crop of a page, in pixels of the page as looked at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub score: f32,
}

/// Running sums over the block grid, so any window's totals cost four lookups.
struct Sums {
    cols: usize,
    rows: usize,
    /// Per measure, (cols + 1) × (rows + 1) prefix sums: saturation, lightness, lightness²,
    /// near-white, near-black.
    t: [Vec<f64>; 5],
}

impl Sums {
    fn new(rgba: &[u8], w: u32, h: u32) -> Sums {
        let cols = (w / BLOCK) as usize;
        let rows = (h / BLOCK) as usize;
        let mut t: [Vec<f64>; 5] = std::array::from_fn(|_| vec![0.0; (cols + 1) * (rows + 1)]);
        let stride = cols + 1;
        for r in 0..rows {
            let mut row = [0.0f64; 5];
            for c in 0..cols {
                // One pixel in the middle of each block stands for it: pages are big and smooth.
                let (px, py) = (c as u32 * BLOCK + BLOCK / 2, r as u32 * BLOCK + BLOCK / 2);
                let i = ((py * w + px) * 4) as usize;
                let (rr, gg, bb) = (rgba[i] as f64 / 255.0, rgba[i + 1] as f64 / 255.0, rgba[i + 2] as f64 / 255.0);
                let max = rr.max(gg).max(bb);
                let min = rr.min(gg).min(bb);
                let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
                let l = 0.2126 * rr + 0.7152 * gg + 0.0722 * bb;
                let white = if l > 0.9 && max - min < 0.1 { 1.0 } else { 0.0 };
                let black = if l < 0.06 { 1.0 } else { 0.0 };
                // Saturation of a nearly black pixel is noise: weighed by how lit it is.
                let v = [sat * max.min(1.0), l, l * l, white, black];
                for k in 0..5 {
                    row[k] += v[k];
                    t[k][(r + 1) * stride + c + 1] = t[k][r * stride + c + 1] + row[k];
                }
            }
        }
        Sums { cols, rows, t }
    }

    fn sum(&self, k: usize, c0: usize, r0: usize, c1: usize, r1: usize) -> f64 {
        let s = self.cols + 1;
        let t = &self.t[k];
        t[r1 * s + c1] - t[r0 * s + c1] - t[r1 * s + c0] + t[r0 * s + c0]
    }

    /// The score of the blocks `c0..c1` across, `r0..r1` down.
    fn score(&self, c0: usize, r0: usize, c1: usize, r1: usize) -> f32 {
        let n = ((c1 - c0) * (r1 - r0)).max(1) as f64;
        let sat = self.sum(0, c0, r0, c1, r1) / n;
        let l = self.sum(1, c0, r0, c1, r1) / n;
        let l2 = self.sum(2, c0, r0, c1, r1) / n;
        let contrast = (l2 - l * l).max(0.0).sqrt();
        let white = self.sum(3, c0, r0, c1, r1) / n;
        let black = self.sum(4, c0, r0, c1, r1) / n;
        (sat * 1.0 + contrast * 0.9 - white * 1.4 - black * 0.5) as f32
    }
}

/// The most colourful 4:3 crop of a page (RGBA, `w` × `h`), from windows the page's full width
/// and two thirds of it, stepped a quarter of their height down (and a third across). None when
/// the page is too small to cut one from.
pub fn best_crop(rgba: &[u8], w: u32, h: u32) -> Option<Crop> {
    if w < BLOCK * 8 || h < BLOCK * 6 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let s = Sums::new(rgba, w, h);
    let mut best: Option<Crop> = None;
    for frac in [1.0f32, 0.66] {
        let cw = ((s.cols as f32 * frac) as usize).max(4);
        let ch = (cw * 3 / 4).min(s.rows);
        if ch < 3 {
            continue;
        }
        let cw = cw.min(ch * 4 / 3).max(1);
        let step_y = (ch / 4).max(1);
        let step_x = (cw / 3).max(1);
        // Every window whole inside the page: stepped, and the last one flush with the far edge.
        let starts = |len: usize, span: usize, step: usize| -> Vec<usize> {
            let last = len.saturating_sub(span);
            let mut v: Vec<usize> = (0..=last).step_by(step).collect();
            if v.last() != Some(&last) {
                v.push(last);
            }
            v
        };
        for r0 in starts(s.rows, ch, step_y) {
            for c0 in starts(s.cols, cw, step_x) {
                let (c1, r1) = (c0 + cw, r0 + ch);
                let score = s.score(c0, r0, c1, r1);
                if best.is_none_or(|b| score > b.score) {
                    let b = BLOCK as usize;
                    best = Some(Crop { x: (c0 * b) as u32, y: (r0 * b) as u32, w: (cw * b) as u32, h: (ch * b) as u32, score });
                }
            }
        }
    }
    best
}

/// Which pages to try: up to [`TRIES`] distinct ones, chosen by `seed` (the chapter's id, so the
/// same chapter gets the same picture), leaving out the first page and the last two when there
/// are enough (credits, a title card, a "next time").
pub fn pick_pages(count: u32, seed: u64) -> Vec<u32> {
    if count == 0 {
        return Vec::new();
    }
    let (lo, hi) = if count > 5 { (1, count - 2) } else { (0, count) };
    let span = hi - lo;
    let mut out = Vec::new();
    let mut x = seed | 1;
    let mut guard = 0;
    while out.len() < TRIES.min(span as usize) && guard < 64 {
        // xorshift64
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let n = lo + (x % span as u64) as u32;
        if !out.contains(&n) {
            out.push(n);
        }
        guard += 1;
    }
    out
}

/// A folder name compared as Suwayomi writes it: `/`, `:` and the like become `_`, so names are
/// compared once normalised ("Junk to King_ I Am A Scavenger" is "Junk to King: I Am A
/// Scavenger").
fn same_name(dir: &str, want: &str) -> bool {
    let (a, b) = (norm_title(dir), norm_title(want));
    !a.is_empty() && a == b
}

fn dirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// The chapter's folder in Suwayomi's downloads (`root` is its `mangas/`):
/// `<source> (EN)/<series title>/<scanlator>_<chapter name>` (no scanlator: the name alone).
pub fn chapter_dir(root: &Path, source: &str, title: &str, scanlator: Option<&str>, name: &str) -> Option<PathBuf> {
    let want = match scanlator {
        Some(s) if !s.is_empty() => format!("{s}_{name}"),
        _ => name.to_string(),
    };
    let file = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for src in dirs(root) {
        let n = file(&src);
        // "Asura Scans (EN)": the language in brackets is Suwayomi's, not the source's name.
        let bare = match n.rfind(" (") {
            Some(i) if n.ends_with(')') => &n[..i],
            _ => n.as_str(),
        };
        if !same_name(bare, source) {
            continue;
        }
        for series in dirs(&src).into_iter().filter(|d| same_name(&file(d), title)) {
            if let Some(ch) = dirs(&series).into_iter().find(|d| same_name(&file(d), &want)) {
                return Some(ch);
            }
        }
    }
    None
}

/// The page pictures in a chapter's folder, in reading order (`001.webp`, `002.webp`, ...).
pub fn pages_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| ["webp", "jpg", "jpeg", "png"].contains(&x.to_ascii_lowercase().as_str())))
        .collect();
    v.sort();
    v
}

/// The chapter's picture as WebP, from up to [`TRIES`] of its pages; None when no page could
/// be read or cut from.
pub fn make(dir: &Path, seed: u64) -> Option<Vec<u8>> {
    let pages = pages_in(dir);
    let mut best: Option<(f32, image::RgbaImage)> = None;
    for i in pick_pages(pages.len() as u32, seed) {
        let Ok(img) = image::open(&pages[i as usize]) else { continue };
        let (w, h) = (img.width(), img.height());
        if w == 0 || h == 0 {
            continue;
        }
        let lh = ((h as u64 * LOOK_W as u64) / w as u64).clamp(1, 65_535) as u32;
        let small = image::imageops::resize(&img.to_rgba8(), LOOK_W, lh, image::imageops::FilterType::Triangle);
        drop(img);
        let Some(c) = best_crop(small.as_raw(), LOOK_W, lh) else { continue };
        if best.as_ref().is_none_or(|(s, _)| c.score > *s) {
            best = Some((c.score, image::imageops::crop_imm(&small, c.x, c.y, c.w, c.h).to_image()));
        }
    }
    let (_, crop) = best?;
    let out = image::imageops::resize(&crop, THUMB_W, THUMB_H, image::imageops::FilterType::Lanczos3);
    Some(webp::Encoder::from_rgba(out.as_raw(), THUMB_W, THUMB_H).encode(QUALITY).to_vec())
}

impl Ctx {
    fn thumb_path(&self, chapter_id: i64) -> PathBuf {
        self.data.join("thumbs").join(format!("{chapter_id}.webp"))
    }

    /// The pictures already made, read once at start.
    pub fn load_thumbs(&self) {
        let ids: Vec<i64> = std::fs::read_dir(self.data.join("thumbs"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".webp")?.parse().ok())
            .collect();
        self.thumbs.lock().unwrap_or_else(|p| p.into_inner()).extend(ids);
    }

    /// `GET /v1/chapters/{id}/thumb`: the picture, or None at once when there is none yet (one
    /// is then made within a minute if the chapter is downloaded).
    pub fn thumb(&self, chapter_id: i64) -> Option<Vec<u8>> {
        let made = self.thumbs.lock().unwrap_or_else(|p| p.into_inner()).contains(&chapter_id);
        if made && let Ok(b) = std::fs::read(self.thumb_path(chapter_id)) {
            return Some(b);
        }
        if self.downloads.is_some() {
            let mut wanted = self.thumbs_wanted.lock().unwrap_or_else(|p| p.into_inner());
            if wanted.len() < 200 && !wanted.contains(&chapter_id) {
                wanted.push(chapter_id);
            }
            drop(wanted);
            self.wake();
        }
        None
    }

    fn tried_lately(&self, chapter_id: i64, now: i64) -> bool {
        let at = self.store().get(&format!("thumb_no:{chapter_id}"));
        at.and_then(|v| v.parse::<i64>().ok()).is_some_and(|at| now - at < RETRY_AFTER)
    }

    /// Cuts the chapter's picture from its downloaded pages (slow: pictures are decoded; never
    /// under the store's lock). True when one was made.
    fn make_thumb(&self, m: &Manga, c: &Ch) -> bool {
        let Some(root) = &self.downloads else { return false };
        let picture = chapter_dir(root, &m.source_name, &m.title, c.group.as_deref(), &c.name).and_then(|dir| make(&dir, c.id as u64));
        let Some(bytes) = picture else {
            self.store().set(&format!("thumb_no:{}", c.id), &now_ms().to_string());
            return false;
        };
        let path = self.thumb_path(c.id);
        let _ = std::fs::create_dir_all(self.data.join("thumbs"));
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
        self.thumbs.lock().unwrap_or_else(|p| p.into_inner()).insert(c.id);
        true
    }

    fn wants_thumb(&self, c: &Ch, now: i64) -> bool {
        if !c.downloaded {
            return false;
        }
        let made = self.thumbs.lock().unwrap_or_else(|p| p.into_inner()).contains(&c.id);
        !made && !self.tried_lately(c.id, now)
    }

    /// Pictures for downloaded chapters that have none, `most` at most; answers how many made.
    pub fn thumbs_pass(&self, most: usize) -> usize {
        if self.downloads.is_none() {
            return 0;
        }
        let now = now_ms();
        let works = self.store().works();
        let mut made = 0;
        for w in works {
            let links = self.store().links(w.id);
            for id in std::iter::once(w.manga_id).chain(links) {
                let Ok(m) = self.suwa.manga(id) else { continue };
                for c in m.chapters.iter().filter(|c| self.wants_thumb(c, now)) {
                    if made >= most {
                        return made;
                    }
                    if self.make_thumb(&m, c) {
                        made += 1;
                    }
                }
            }
        }
        made
    }

    /// The pictures the phone asked for before they were made (a few each minute).
    pub fn wanted_thumbs(&self) {
        let now = now_ms();
        let ids: Vec<i64> = {
            let mut w = self.thumbs_wanted.lock().unwrap_or_else(|p| p.into_inner());
            let n = w.len().min(20);
            w.drain(..n).collect()
        };
        for id in ids {
            let Ok(info) = self.suwa.chapter_info(id) else { continue };
            if !info.downloaded {
                continue;
            }
            let Ok(m) = self.suwa.manga(info.manga_id) else { continue };
            if let Some(c) = m.chapters.iter().find(|c| c.id == id)
                && self.wants_thumb(c, now)
                && !self.make_thumb(&m, c)
            {
                log(&format!("chapter {id}: no picture could be cut from its pages"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = f(x, y);
                px.extend_from_slice(&[r, g, b, 255]);
            }
        }
        px
    }

    // The phone's tests, kept the same: the two must choose alike.
    #[test]
    fn the_crop_finds_the_coloured_panel_not_the_white_page() {
        let px = page(400, 1600, |x, y| if (900..1200).contains(&y) { if (x / 20 + y / 20) % 2 == 0 { [200, 30, 40] } else { [30, 60, 190] } } else { [250, 250, 250] });
        let c = best_crop(&px, 400, 1600).unwrap();
        assert!(c.y + c.h / 2 >= 900 && c.y + c.h / 2 <= 1200, "crop centred on the panel: {c:?}");
        assert!((c.w as f32 / c.h as f32 - 4.0 / 3.0).abs() < 0.1, "{c:?}");
        assert!(c.x + c.w <= 400 && c.y + c.h <= 1600);
    }

    #[test]
    fn a_speech_bubble_scores_below_a_coloured_scene() {
        let bubble = page(400, 300, |x, y| if y % 40 < 6 && x % 30 < 20 { [20, 20, 20] } else { [255, 255, 255] });
        let scene = page(400, 300, |x, y| [(x / 2) as u8, (y / 2) as u8, 160]);
        let a = best_crop(&bubble, 400, 300).unwrap().score;
        let b = best_crop(&scene, 400, 300).unwrap().score;
        assert!(b > a + 0.3, "scene {b} vs bubble {a}");
        let black = page(400, 300, |_, _| [0, 0, 0]);
        assert!(best_crop(&black, 400, 300).unwrap().score < b);
    }

    #[test]
    fn too_small_a_page_gives_no_crop() {
        assert_eq!(best_crop(&page(10, 10, |_, _| [9, 9, 9]), 10, 10), None);
        assert_eq!(best_crop(&[0u8; 16], 400, 300), None);
    }

    #[test]
    fn pages_are_picked_from_the_middle_without_repeats() {
        let p = pick_pages(40, 12345);
        assert_eq!(p.len(), TRIES);
        assert!(p.iter().all(|n| (1..38).contains(n)));
        let mut d = p.clone();
        d.sort_unstable();
        d.dedup();
        assert_eq!(d.len(), p.len());
        assert_eq!(pick_pages(2, 7).len(), 2);
        assert!(pick_pages(0, 7).is_empty());
        assert_eq!(pick_pages(40, 9), pick_pages(40, 9), "the same chapter, the same pages");
    }

    #[test]
    fn a_downloaded_chapter_is_found_and_cut_from_its_colourful_page() {
        let root = std::env::temp_dir().join(format!("leaf-thumbs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ch = root.join("MangaDex (EN)").join("Junk to King_ I Am A Scavenger").join("Drake Scans_Ch.12");
        std::fs::create_dir_all(&ch).unwrap();
        std::fs::create_dir_all(root.join("MangaDex (EN)").join("Junk to King_ I Am A Scavenger").join("Drake Scans_Ch.1")).unwrap();
        // Six white pages but one: whichever three are tried, white loses to colour when present.
        for i in 1..=6u32 {
            let px = if i == 3 { page(200, 300, |x, y| [(x + y) as u8, 40, 200]) } else { page(200, 300, |_, _| [255, 255, 255]) };
            let webp = webp::Encoder::from_rgba(&px, 200, 300).encode(90.0).to_vec();
            std::fs::write(ch.join(format!("{i:03}.webp")), webp).unwrap();
        }
        std::fs::write(ch.join("ComicInfo.xml"), "<x/>").unwrap();
        let found = chapter_dir(&root, "MangaDex", "Junk to King: I Am A Scavenger", Some("Drake Scans"), "Ch.12").expect("the folder");
        assert_eq!(found, ch);
        assert!(chapter_dir(&root, "Asura Scans", "Junk to King: I Am A Scavenger", Some("Drake Scans"), "Ch.12").is_none(), "another source's folder is not it");
        assert!(chapter_dir(&root, "MangaDex", "Junk to King: I Am A Scavenger", None, "Ch.12").is_none());
        assert_eq!(pages_in(&ch).len(), 6, "only pictures");
        let w = make(&ch, 3).expect("a picture");
        let img = image::load_from_memory(&w).unwrap();
        assert_eq!((img.width(), img.height()), (THUMB_W, THUMB_H));
        let _ = std::fs::remove_dir_all(&root);
    }
}
