//! A chapter's picture: a 4:3 crop of one of its own pages, the most colourful of a few random
//! pages, so the series page shows what happens in each chapter rather than a speech bubble or a
//! white gutter (design 42, "Series page"). Made once per chapter on an image worker and kept on
//! disk (`thumbs/<chapter>.jpg`); this module is the choosing, which is pure and tested.
//!
//! A page is scored on a grid of small blocks: colour (saturation) and contrast (spread of
//! lightness) count for a crop, near-white counts hard against it (bubbles, gutters, blank
//! panels) and near-black a little (black gutters, letterboxing).

/// The picture's size on disk and in the atlas.
pub const THUMB_W: u32 = 240;
pub const THUMB_H: u32 = 180;
/// Pages are looked at this wide.
pub const LOOK_W: u32 = 400;
/// The most pages tried per chapter.
pub const TRIES: usize = 4;

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

/// Which pages to try: up to [`TRIES`] distinct ones, chosen at random by `seed`, leaving out the
/// first page and the last two when there are enough (credits, a title card, a "next time").
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

    #[test]
    fn the_crop_finds_the_coloured_panel_not_the_white_page() {
        // A white page 400 × 1600 with a red and blue panel from 900 to 1200.
        let px = page(400, 1600, |x, y| if (900..1200).contains(&y) { if (x / 20 + y / 20) % 2 == 0 { [200, 30, 40] } else { [30, 60, 190] } } else { [250, 250, 250] });
        let c = best_crop(&px, 400, 1600).unwrap();
        assert!(c.y + c.h / 2 >= 900 && c.y + c.h / 2 <= 1200, "crop centred on the panel: {c:?}");
        // 4:3, within the page.
        assert!((c.w as f32 / c.h as f32 - 4.0 / 3.0).abs() < 0.1, "{c:?}");
        assert!(c.x + c.w <= 400 && c.y + c.h <= 1600);
    }

    #[test]
    fn a_speech_bubble_scores_below_a_coloured_scene() {
        // A bubble: white with a few dark words. A scene: colour and shade.
        let bubble = page(400, 300, |x, y| if y % 40 < 6 && x % 30 < 20 { [20, 20, 20] } else { [255, 255, 255] });
        let scene = page(400, 300, |x, y| [(x / 2) as u8, (y / 2) as u8, 160]);
        let a = best_crop(&bubble, 400, 300).unwrap().score;
        let b = best_crop(&scene, 400, 300).unwrap().score;
        assert!(b > a + 0.3, "scene {b} vs bubble {a}");
        // Black letterboxing is weighed against too.
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
        assert_eq!(pick_pages(3, 7).len(), 3);
        assert!(pick_pages(0, 7).is_empty());
        assert_ne!(pick_pages(40, 1), pick_pages(40, 2));
    }
}
