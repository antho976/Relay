//! The pictures `video.still` answers with: frames as Remotion drew them (PNG), laid out as one
//! labelled contact sheet when there are several, and the pixels under a text for its contrast.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// A picture in memory: 8-bit RGBA rows, top first.
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Rgba {
    pub fn new(w: u32, h: u32, fill: [u8; 4]) -> Rgba {
        let mut data = Vec::with_capacity(w as usize * h as usize * 4);
        for _ in 0..w as usize * h as usize {
            data.extend_from_slice(&fill);
        }
        Rgba { w, h, data }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = (y as usize * self.w as usize + x as usize) * 4;
        [self.data[at], self.data[at + 1], self.data[at + 2], self.data[at + 3]]
    }

    fn put(&mut self, x: u32, y: u32, c: [u8; 4]) {
        if x < self.w && y < self.h {
            let at = (y as usize * self.w as usize + x as usize) * 4;
            self.data[at..at + 4].copy_from_slice(&c);
        }
    }

    /// `other` copied in with its top left at (`x`, `y`), cut to fit.
    fn blit(&mut self, other: &Rgba, x: u32, y: u32) {
        let w = other.w.min(self.w.saturating_sub(x)) as usize;
        for row in 0..other.h.min(self.h.saturating_sub(y)) {
            let from = row as usize * other.w as usize * 4;
            let to = ((y + row) as usize * self.w as usize + x as usize) * 4;
            self.data[to..to + w * 4].copy_from_slice(&other.data[from..from + w * 4]);
        }
    }
}

/// A PNG as RGBA: 8-bit grey, grey with alpha, RGB or RGBA (what a browser writes).
pub fn decode(path: &Path) -> Result<Rgba, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("the picture is too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h) = (info.width, info.height);
    let px = w as usize * h as usize;
    let data = match info.color_type {
        png::ColorType::Rgba => buf[..px * 4].to_vec(),
        png::ColorType::Rgb => buf[..px * 3].as_chunks::<3>().0.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..px * 2].as_chunks::<2>().0.iter().flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => buf[..px].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("an indexed PNG was not expanded".into()),
    };
    Ok(Rgba { w, h, data })
}

pub fn encode(path: &Path, picture: &Rgba) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), picture.w, picture.h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&picture.data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

const GAP: u32 = 8;
/// The strip under each frame its label is written in.
const LABEL: u32 = 30;
const GROUND: [u8; 4] = [24, 26, 31, 255];
const INK: [u8; 4] = [235, 238, 243, 255];

/// Frames side by side in rows, each with its label under it: as many columns as make the sheet
/// about square, frames in reading order.
pub fn sheet(frames: &[Rgba], labels: &[String]) -> Rgba {
    let (tw, th) = frames.iter().fold((1, 1), |(w, h), f| (w.max(f.w), h.max(f.h)));
    let n = frames.len().max(1) as f64;
    let cols = ((n * (th + LABEL) as f64 / tw as f64).sqrt().ceil() as usize).clamp(1, frames.len().max(1));
    let rows = frames.len().div_ceil(cols).max(1);
    let w = GAP + cols as u32 * (tw + GAP);
    let h = GAP + rows as u32 * (th + LABEL + GAP);
    let mut out = Rgba::new(w, h, GROUND);
    for (i, frame) in frames.iter().enumerate() {
        let (col, row) = ((i % cols) as u32, (i / cols) as u32);
        let x = GAP + col * (tw + GAP) + (tw - frame.w) / 2;
        let y = GAP + row * (th + LABEL + GAP);
        out.blit(frame, x, y);
        if let Some(label) = labels.get(i) {
            write(&mut out, label, GAP + col * (tw + GAP) + 4, y + th + (LABEL - 7 * SCALE) / 2);
        }
    }
    out
}

/// How large a label's 5 × 7 glyphs are drawn.
const SCALE: u32 = 3;

/// `text` in a 5 × 7 pixel font: the digits and what a frame's label holds (`1.5s · f45`).
fn write(out: &mut Rgba, text: &str, x: u32, y: u32) {
    let mut at = x;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (r, bits) in rows.iter().enumerate() {
                for c in 0..5 {
                    if bits & (0b10000 >> c) != 0 {
                        for dy in 0..SCALE {
                            for dx in 0..SCALE {
                                out.put(at + c * SCALE + dx, y + r as u32 * SCALE + dy, INK);
                            }
                        }
                    }
                }
            }
        }
        at += 6 * SCALE;
    }
}

fn glyph(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '·' => [0, 0, 0, 0b01100, 0b01100, 0, 0],
        's' => [0, 0, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
        'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        _ => return None,
    })
}

/// WCAG relative luminance of an sRGB colour.
pub fn luminance(c: [u8; 3]) -> f64 {
    let ch = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])
}

/// The WCAG contrast ratio of two luminances, 1 to 21.
pub fn ratio(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

/// The contrast of text drawn in `ink` against what is under it, in the box (`x`, `y`, `w`, `h`)
/// of a picture drawn at `scale`: the pixels clearly not the text's colour are the ground, and
/// the ground's median luminance is held against the ink's. When few pixels differ from the ink,
/// the ground is near the ink's own colour, which is the fault itself: glyphs cover well under
/// half a line's box, so the median of every pixel is then the ground's. `None` for a box outside
/// the picture.
pub fn text_contrast(picture: &Rgba, scale: f64, rect: [f64; 4], ink: [u8; 3]) -> Option<f64> {
    let [x, y, w, h] = rect.map(|v| v * scale);
    let (x0, y0) = (x.max(0.0) as u32, y.max(0.0) as u32);
    let (x1, y1) = (((x + w).ceil() as u32).min(picture.w), ((y + h).ceil() as u32).min(picture.h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let ink_l = luminance(ink);
    let mut ground = Vec::new();
    let mut every = Vec::new();
    // Every pixel in a small box, every few in a large one: about 4,000 at most.
    let step = (((x1 - x0) as f64 * (y1 - y0) as f64 / 4000.0).sqrt().floor() as u32).max(1);
    for py in (y0..y1).step_by(step as usize) {
        for px in (x0..x1).step_by(step as usize) {
            let p = picture.pixel(px, py);
            every.push(luminance([p[0], p[1], p[2]]));
            let distance = p.iter().zip(ink).map(|(&a, b)| (a as i32 - b as i32).abs()).max().unwrap_or(0);
            if distance > 48 {
                ground.push(luminance([p[0], p[1], p[2]]));
            }
        }
    }
    if ground.len() * 5 < every.len() {
        ground = every;
    }
    ground.sort_by(f64::total_cmp);
    Some(ratio(ink_l, ground[ground.len() / 2]))
}

/// A CSS colour as the browser's computed style writes it (`rgb(1, 2, 3)`, `rgba(1, 2, 3, 0.5)`,
/// or `color(srgb 0.1 0.2 0.3)`): its RGB and alpha.
pub fn css_color(text: &str) -> Option<([u8; 3], f64)> {
    let text = text.trim();
    if let Some(body) = text.strip_prefix("color(srgb").and_then(|t| t.strip_suffix(')')) {
        let (rgb, alpha) = body.split_once('/').map_or((body, None), |(c, a)| (c, Some(a)));
        let v: Vec<f64> = rgb.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        if v.len() != 3 {
            return None;
        }
        let a = alpha.and_then(|a| a.trim().parse().ok()).unwrap_or(1.0);
        return Some(([0, 1, 2].map(|i| (v[i] * 255.0).round().clamp(0.0, 255.0) as u8), a));
    }
    let open = text.find('(')?;
    let body = text[open + 1..].strip_suffix(')')?;
    let v: Vec<f64> = body.split([',', ' ', '/']).filter(|s| !s.is_empty()).filter_map(|n| n.parse().ok()).collect();
    if v.len() < 3 {
        return None;
    }
    Some(([0, 1, 2].map(|i| v[i].round().clamp(0.0, 255.0) as u8), v.get(3).copied().unwrap_or(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_lays_frames_out_in_rows_with_room_for_labels() {
        let frames: Vec<Rgba> = (0..6).map(|_| Rgba::new(90, 160, [255, 0, 0, 255])).collect();
        let labels: Vec<String> = (0..6).map(|i| format!("{i}s · f{}", i * 30)).collect();
        let out = sheet(&frames, &labels);
        // Tall frames: more columns than rows.
        let cols = (out.w - GAP) / (90 + GAP);
        let rows = (out.h - GAP) / (160 + LABEL + GAP);
        assert!(cols as usize * rows as usize >= 6);
        assert!(cols >= rows, "{cols} × {rows}");
        // The first frame is where it should be, and its label is ink under it.
        assert_eq!(out.pixel(GAP + 1, GAP + 1), [255, 0, 0, 255]);
        let label_row = GAP + 160;
        assert!((label_row..label_row + LABEL).any(|y| (GAP..GAP + 90).any(|x| out.pixel(x, y) == INK)));
    }

    #[test]
    fn a_png_reads_back_as_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.png");
        let mut picture = Rgba::new(3, 2, [10, 20, 30, 255]);
        picture.put(2, 1, [200, 100, 50, 128]);
        encode(&path, &picture).unwrap();
        assert_eq!(decode(&path).unwrap(), picture);
    }

    #[test]
    fn contrast_is_read_from_the_ground_around_the_words() {
        // White ground with black "text" pixels in a quarter of the box.
        let mut picture = Rgba::new(40, 20, [255, 255, 255, 255]);
        for x in 10..20 {
            for y in 5..15 {
                picture.put(x, y, [0, 0, 0, 255]);
            }
        }
        let c = text_contrast(&picture, 1.0, [0.0, 0.0, 40.0, 20.0], [0, 0, 0]).unwrap();
        assert!((c - 21.0).abs() < 0.01, "{c}");
        // Light grey ink on a mid grey: poor.
        let grey = Rgba::new(20, 20, [119, 119, 119, 255]);
        let c = text_contrast(&grey, 1.0, [0.0, 0.0, 20.0, 20.0], [180, 180, 180]).unwrap();
        assert!(c < 3.0, "{c}");
        // Ink all but the ground's own colour: no pixel stands apart, and that is the fault.
        let mut dim = Rgba::new(20, 20, [40, 40, 40, 255]);
        for x in 5..9 {
            dim.put(x, 8, [70, 70, 70, 255]);
        }
        let c = text_contrast(&dim, 1.0, [0.0, 0.0, 20.0, 20.0], [70, 70, 70]).unwrap();
        assert!(c < 1.6, "{c}");
        // A box outside the picture says nothing.
        assert_eq!(text_contrast(&dim, 1.0, [40.0, 40.0, 10.0, 10.0], [0, 0, 0]), None);
        // At half scale the box is found in the smaller picture.
        assert!(text_contrast(&picture, 0.5, [0.0, 0.0, 80.0, 40.0], [0, 0, 0]).is_some());
    }

    #[test]
    fn css_colours_read_as_computed_styles_write_them() {
        assert_eq!(css_color("rgb(245, 247, 250)"), Some(([245, 247, 250], 1.0)));
        assert_eq!(css_color("rgba(0, 0, 0, 0.5)"), Some(([0, 0, 0], 0.5)));
        assert_eq!(css_color("rgb(10 20 30 / 0.25)"), Some(([10, 20, 30], 0.25)));
        assert_eq!(css_color("color(srgb 1 0.5 0)"), Some(([255, 128, 0], 1.0)));
        assert_eq!(css_color("transparent"), None);
    }
}
