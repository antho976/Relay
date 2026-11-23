//! Downloaded pages, made smaller. Suwayomi converts pages to WebP at its own quality (about 80;
//! its quality setting is broken in 2.4). Re-encoding at 60 takes a chapter from ~12 MB to ~8 MB
//! with no difference visible on a phone (measured 2026-11-22 on MangaDex's Junk to King ch. 1).
//!
//! It reads Suwayomi's download folder (shared through the compose volume), re-encodes each WebP
//! page once, and keeps the new one only when it is clearly smaller. A page Suwayomi is still
//! writing (changed in the last minute) waits for the next pass.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Keeps the re-encoded page only when it saves at least this share.
const MIN_SAVING: f64 = 0.05;
/// At most this many pages a pass: a big first pass spreads over a few hours.
const PER_PASS: usize = 4000;

pub struct Squeezed {
    pub pages: usize,
    pub before: u64,
    pub after: u64,
}

fn pages_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            pages_under(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("webp")) {
            out.push(p);
        }
    }
}

/// Re-encodes `bytes` (a WebP page) at `quality`; None when it would not be clearly smaller.
pub fn smaller(bytes: &[u8], quality: f32) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let out = webp::Encoder::from_rgba(img.as_raw(), w, h).encode(quality);
    ((out.len() as f64) < bytes.len() as f64 * (1.0 - MIN_SAVING)).then(|| out.to_vec())
}

/// One pass over the download folder; `done` says whether a page was already seen.
pub fn pass(root: &Path, quality: f32, done: &mut dyn FnMut(&str) -> bool, mark: &mut dyn FnMut(&str)) -> Squeezed {
    let mut all = Vec::new();
    pages_under(root, &mut all);
    let fresh = SystemTime::now() - Duration::from_secs(60);
    let mut s = Squeezed { pages: 0, before: 0, after: 0 };
    for p in all {
        if s.pages >= PER_PASS {
            break;
        }
        let key = p.to_string_lossy().to_string();
        if done(&key) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if meta.modified().is_ok_and(|m| m > fresh) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else { continue };
        s.pages += 1;
        s.before += bytes.len() as u64;
        match smaller(&bytes, quality) {
            Some(out) => {
                let tmp = p.with_extension("webp.leaf-tmp");
                if std::fs::write(&tmp, &out).is_ok() && std::fs::rename(&tmp, &p).is_ok() {
                    s.after += out.len() as u64;
                } else {
                    let _ = std::fs::remove_file(&tmp);
                    s.after += bytes.len() as u64;
                }
            }
            None => s.after += bytes.len() as u64,
        }
        mark(&key);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_only_replaced_when_clearly_smaller() {
        // A noisy page encoded at high quality shrinks at 60; the shrunk one does not shrink again.
        let mut rgba = vec![0u8; 64 * 64 * 4];
        let mut x: u32 = 7;
        for px in rgba.chunks_mut(4) {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            px.copy_from_slice(&[(x >> 16) as u8, (x >> 8) as u8, (x >> 24) as u8, 255]);
        }
        let big = webp::Encoder::from_rgba(&rgba, 64, 64).encode(95.0).to_vec();
        let once = smaller(&big, 60.0).expect("q95 to q60 saves");
        assert!(once.len() < big.len());
        assert!(smaller(&once, 60.0).is_none(), "already at 60: left alone");
        assert!(smaller(b"not a picture", 60.0).is_none());
    }
}
