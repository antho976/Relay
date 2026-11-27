//! Scanlator promo pages: the "join our Discord, read only at …" panels and the recycled promo art
//! a group puts at the start or end of every chapter, recognised by their pictures and skipped.
//!
//! - A page near a chapter's edges (its first [`EDGE_START`] or last [`EDGE_END`]) passing through
//!   leaf-server (`GET /v1/chapters/{id}/pages/{n}`, from a source or from Suwayomi's downloads)
//!   gets a 64-bit difference hash ([`dhash`]: grey, 9 × 8, each pixel against its right
//!   neighbour), kept with its chapter and page. Only bytes already being fetched are hashed: this
//!   never asks a source for anything.
//! - A page is promo when pages within [`NEAR`] bits of it sit near the edges of at least
//!   [`CHAPTERS`] distinct chapters, or it is within [`NEAR`] bits of one the owner hid by hand
//!   (`POST /v1/pages/hide`). A promo page is answered `204` with `x-leaf-skip: 1`, no bytes; once
//!   its hash is known it is not even fetched again.
//! - Plain pages (nearly one colour: a blank, a page of small text on white) have no hash: they
//!   would all look alike, and a blank page is not a scanlator's.

use std::collections::{HashMap, HashSet};

use crate::autopilot::{Ctx, now_ms};

/// Pages from a chapter's start that are looked at.
pub const EDGE_START: u32 = 3;
/// Pages from a chapter's end that are looked at.
pub const EDGE_END: u32 = 4;
/// Hashes this many bits apart or fewer are the same picture (re-encoded, resized, cropped a
/// little).
pub const NEAR: u32 = 6;
/// A picture seen near the edges of this many chapters is a scanlator's, not the story's.
pub const CHAPTERS: usize = 3;
/// The grey levels' spread under which a page is too plain to hash (0 to 255).
const PLAIN: f32 = 4.0;
/// What [`Seen`] keeps for a page hashed and found plain, so it is not decoded again.
const PLAIN_PAGE: u64 = 0;

/// What the phone is told of a page that came blank: its reader offers another source.
pub const BLANK_PAGE: &str = "The source sent this page blank";

/// A picture that is only zeros at its start: what a source (or a download cut short) leaves
/// where the image should be.
pub fn blank(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.iter().take(4096).all(|b| *b == 0)
}

/// Page `n` of a chapter `count` pages long is near its start or its end.
pub fn near_edge(n: u32, count: u32) -> bool {
    n < EDGE_START || n + EDGE_END >= count
}

/// The bits that differ between two hashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// The difference hash of 9 × 8 grey levels (row by row): bit `8y + x` is set when pixel `x` of
/// row `y` is darker than the one to its right. None when the picture is too plain to tell.
pub fn dhash_grey(px: &[u8]) -> Option<u64> {
    if px.len() != 72 {
        return None;
    }
    let mean = px.iter().map(|&v| v as f32).sum::<f32>() / 72.0;
    let spread = (px.iter().map(|&v| (v as f32 - mean).powi(2)).sum::<f32>() / 72.0).sqrt();
    if spread < PLAIN {
        return None;
    }
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            if px[y * 9 + x] < px[y * 9 + x + 1] {
                h |= 1 << (y * 8 + x);
            }
        }
    }
    Some(h)
}

/// A page's difference hash from its bytes (WebP, JPEG or PNG). None when it doesn't decode or is
/// too plain.
pub fn dhash(bytes: &[u8]) -> Option<u64> {
    let mut r = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 << 20);
    r.limits(limits);
    let img = r.decode().ok()?;
    let small = image::imageops::thumbnail(&img.to_luma8(), 9, 8);
    dhash_grey(small.as_raw())
}

/// What leaf-server has seen: the hashes of pages near chapters' edges, and the pages hidden by
/// hand. Kept in `leaf.db` (`page_hash`, `page_hidden`) and in memory, where every page asks it.
#[derive(Default, Debug)]
pub struct Seen {
    pages: HashMap<(i64, u32), u64>,
    hidden: HashMap<(i64, u32), u64>,
}

impl Seen {
    /// The hash kept for a chapter's page, if it was hashed.
    pub fn known(&self, chapter: i64, page: u32) -> Option<u64> {
        self.pages.get(&(chapter, page)).or_else(|| self.hidden.get(&(chapter, page))).copied()
    }

    pub fn record(&mut self, chapter: i64, page: u32, hash: u64) {
        self.pages.insert((chapter, page), hash);
    }

    pub fn hide(&mut self, chapter: i64, page: u32, hash: u64) {
        self.hidden.insert((chapter, page), hash);
    }

    /// One page's hiding undone, or (None) every one.
    pub fn unhide(&mut self, which: Option<(i64, u32)>) {
        match which {
            Some(k) => {
                self.hidden.remove(&k);
            }
            None => self.hidden.clear(),
        }
    }

    pub fn hidden_count(&self) -> u32 {
        self.hidden.len() as u32
    }

    /// Whether a picture is a scanlator's: like one hidden by hand, or seen near the edges of
    /// [`CHAPTERS`] distinct chapters (its own among them).
    pub fn is_promo(&self, hash: u64) -> bool {
        // 0 stands for a plain page (no hash): never promo, never like another.
        if hash == PLAIN_PAGE {
            return false;
        }
        if self.hidden.values().any(|&h| hamming(h, hash) <= NEAR) {
            return true;
        }
        let mut chapters: HashSet<i64> = HashSet::new();
        for (&(c, _), &h) in &self.pages {
            if h != PLAIN_PAGE && hamming(h, hash) <= NEAR {
                chapters.insert(c);
                if chapters.len() >= CHAPTERS {
                    return true;
                }
            }
        }
        false
    }
}

/// A page as the phone gets it.
pub enum PageOut {
    Bytes(Vec<u8>, String),
    /// A promo page: `204`, `x-leaf-skip: 1`.
    Skip,
}

impl Ctx {
    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.promo.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The hashes and hidden pages kept from before a restart.
    pub fn load_promo(&self) {
        let (pages, hidden) = {
            let st = self.store();
            (st.page_hashes(), st.hidden_pages())
        };
        let mut seen = self.seen();
        for (c, p, h) in pages {
            seen.record(c, p, h);
        }
        for (c, p, h) in hidden {
            seen.hide(c, p, h);
        }
    }

    /// `GET /v1/chapters/{id}/pages/{n}`: the page, or [`PageOut::Skip`] for a promo page when
    /// `skip` (the phone's setting). A page near the chapter's edges is hashed as it passes; one
    /// whose hash is known to be promo is not fetched at all. A page the source sent blank is an
    /// error ([`BLANK_PAGE`]).
    pub fn page_out(&self, chapter: i64, n: u32, skip: bool) -> Result<PageOut, String> {
        let list = self.pages(chapter)?;
        let edge = near_edge(n, list.urls.len() as u32);
        let known = self.seen().known(chapter, n);
        if skip && let Some(h) = known {
            let promo = self.seen().is_promo(h);
            if promo {
                return Ok(PageOut::Skip);
            }
        }
        let (bytes, kind) = self.page(chapter, n as usize)?;
        if blank(&bytes) {
            // A source that sent zeros for a picture (MangaFire's chapter 58 of Return of the
            // Frozen Player, 2026-11-26): an error the phone can act on, not a page it can't
            // decode; a copy downloaded blank goes, so the next ask is the source's again.
            if list.local {
                let _ = self.download(chapter, false);
            }
            return Err(BLANK_PAGE.to_string());
        }
        if edge && known.is_none() {
            match dhash(&bytes) {
                Some(h) => {
                    self.seen().record(chapter, n, h);
                    self.store().add_page_hash(chapter, n, h, &list.source, now_ms());
                    let promo = self.seen().is_promo(h);
                    if skip && promo {
                        return Ok(PageOut::Skip);
                    }
                }
                // Plain: remembered as such, so it is not decoded again.
                None => self.seen().record(chapter, n, PLAIN_PAGE),
            }
        }
        Ok(PageOut::Bytes(bytes, kind))
    }

    /// `POST /v1/pages/hide`: the page's hash (kept, else from one fetch of the page through its
    /// source's budget) joins the pages hidden by hand. Answers how many there are.
    pub fn hide_page(&self, chapter: i64, n: u32) -> Result<u32, String> {
        let known = self.seen().known(chapter, n).filter(|h| *h != PLAIN_PAGE);
        let h = match known {
            Some(h) => h,
            None => {
                let (bytes, _) = self.page(chapter, n as usize)?;
                dhash(&bytes).ok_or("That page is too plain to recognise others by")?
            }
        };
        self.seen().hide(chapter, n, h);
        self.store().hide_page(chapter, n, h, now_ms());
        Ok(self.seen().hidden_count())
    }

    /// `DELETE /v1/pages/hide`: one page's hiding undone, or (None) every one.
    pub fn unhide_page(&self, which: Option<(i64, u32)>) -> u32 {
        self.seen().unhide(which);
        self.store().unhide_page(which);
        self.seen().hidden_count()
    }

    pub fn hidden_count(&self) -> u32 {
        self.seen().hidden_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_of_zeros_is_blank_and_a_real_one_is_not() {
        assert!(blank(&vec![0u8; 404_770]));
        assert!(!blank(b"RIFF\x00\x00\x00\x00WEBP"));
        assert!(!blank(&[]));
    }

    /// A comic-ish page: bands and blocks of colour, `seed` choosing which.
    fn page(w: u32, h: u32, seed: u32) -> image::RgbImage {
        image::RgbImage::from_fn(w, h, |x, y| {
            let bx = (x * 7 / w + seed) % 5;
            let by = (y * 9 / h + seed * 3) % 4;
            let v = ((bx * 53 + by * 41 + seed * 17) % 256) as u8;
            image::Rgb([v, v.wrapping_mul(3), 255 - v])
        })
    }

    fn encode(img: &image::RgbImage, fmt: image::ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), fmt).unwrap();
        out
    }

    #[test]
    fn the_same_picture_hashes_alike_however_it_was_saved_and_another_does_not() {
        let a = page(720, 1100, 1);
        let png = dhash(&encode(&a, image::ImageFormat::Png)).expect("a hash");
        // Smaller, and as a JPEG: the same picture.
        let small = image::imageops::resize(&a, 540, 825, image::imageops::FilterType::Triangle);
        let jpeg = dhash(&encode(&small, image::ImageFormat::Jpeg)).expect("a hash");
        assert!(hamming(png, jpeg) <= NEAR, "{} bits apart", hamming(png, jpeg));
        // Another page is far from it.
        let b = dhash(&encode(&page(720, 1100, 2), image::ImageFormat::Png)).expect("a hash");
        assert!(hamming(png, b) > NEAR, "{} bits apart", hamming(png, b));
        // A blank page, or one nearly so, has none.
        let blank = image::RgbImage::from_pixel(720, 1100, image::Rgb([255, 255, 255]));
        assert_eq!(dhash(&encode(&blank, image::ImageFormat::Png)), None);
        assert_eq!(dhash(b"not a picture"), None);
        assert_eq!(hamming(0b1011, 0b0010), 2);
    }

    #[test]
    fn a_picture_near_the_edges_of_three_chapters_is_promo() {
        let mut s = Seen::default();
        let promo = 0xF0F0_1234_ABCD_0F0Fu64;
        // Two chapters end with it (one twice): not yet.
        s.record(1, 20, promo);
        s.record(1, 21, promo ^ 0b1);
        s.record(2, 30, promo ^ 0b110);
        assert!(!s.is_promo(promo));
        // A third, re-encoded a little: now it is, and so is the next one like it.
        s.record(3, 0, promo ^ 0b1_0000_0001);
        assert!(s.is_promo(promo));
        assert!(s.is_promo(promo ^ 0b11));
        // A page of the story is not, nor one seven bits away.
        assert!(!s.is_promo(!promo));
        assert!(!s.is_promo(promo ^ 0x7F));
        // Plain pages, near the edges of every chapter, are never promo.
        for c in 10..20 {
            s.record(c, 0, PLAIN_PAGE);
        }
        assert!(!s.is_promo(PLAIN_PAGE));
        assert!(!s.is_promo(0b11), "nor what lies near them");
        assert!(near_edge(0, 40) && near_edge(2, 40) && !near_edge(3, 40));
        assert!(near_edge(36, 40) && near_edge(39, 40) && !near_edge(35, 40));
    }

    #[test]
    fn a_page_hidden_by_hand_hides_ones_like_it_until_undone() {
        let mut s = Seen::default();
        let art = 0x1234_5678_9ABC_DEF0u64;
        assert!(!s.is_promo(art));
        s.hide(7, 1, art);
        assert_eq!(s.hidden_count(), 1);
        assert!(s.is_promo(art ^ 0b10_0000), "one like it");
        assert_eq!(s.known(7, 1), Some(art), "its hash is kept");
        s.unhide(Some((7, 1)));
        assert!(!s.is_promo(art));
        s.hide(7, 1, art);
        s.hide(8, 2, !art);
        s.unhide(None);
        assert_eq!(s.hidden_count(), 0);
    }

    #[test]
    fn hashes_and_hidden_pages_are_kept_in_the_store() {
        let st = crate::store::Store::in_memory();
        st.add_page_hash(5, 39, u64::MAX, "Drake Scans", 1);
        st.add_page_hash(5, 39, 42, "Drake Scans", 2);
        assert_eq!(st.page_hashes(), vec![(5, 39, 42)]);
        st.hide_page(5, 39, u64::MAX - 1, 3);
        assert_eq!(st.hidden_pages(), vec![(5, 39, u64::MAX - 1)], "a hash over i64's range comes back whole");
        st.unhide_page(Some((5, 39)));
        assert!(st.hidden_pages().is_empty());
    }
}
