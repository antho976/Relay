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

    #[cfg(test)]
    fn put(&mut self, x: u32, y: u32, c: [u8; 4]) {
        if x < self.w && y < self.h {
            let at = (y as usize * self.w as usize + x as usize) * 4;
            self.data[at..at + 4].copy_from_slice(&c);
        }
    }

    /// `c` over the pixel at (`x`, `y`) at its alpha.
    pub fn blend(&mut self, x: u32, y: u32, c: [u8; 4]) {
        if x >= self.w || y >= self.h {
            return;
        }
        let at = (y as usize * self.w as usize + x as usize) * 4;
        let a = c[3] as u32;
        for (dst, src) in self.data[at..at + 3].iter_mut().zip(c) {
            *dst = ((src as u32 * a + *dst as u32 * (255 - a)) / 255) as u8;
        }
        self.data[at + 3] = 255;
    }

    /// A rectangle of `c` (at its alpha), cut to fit.
    pub fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, c: [u8; 4]) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                self.blend(xx, yy, c);
            }
        }
    }

    /// `other` copied in with its top left at (`x`, `y`), cut to fit.
    pub fn blit(&mut self, other: &Rgba, x: u32, y: u32) {
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

/// How many columns lay `n` frames of `tw` × `th` out best: the sheet near square, and few cells
/// left empty (an empty cell weighs as much as the sheet being twice as wide as it is tall).
fn columns(n: usize, tw: u32, th: u32) -> usize {
    let cell = (tw + GAP) as f64 / (th + LABEL + GAP) as f64;
    (1..=n.max(1)).min_by(|&a, &b| {
        let score = |cols: usize| {
            let rows = n.div_ceil(cols).max(1);
            let empty = cols * rows - n;
            (cols as f64 * cell / rows as f64).ln().abs() + empty as f64 * std::f64::consts::LN_2
        };
        score(a).total_cmp(&score(b))
    }).unwrap_or(1)
}

/// Frames side by side in rows, each with its label under it, in reading order ([`columns`]).
pub fn sheet(frames: &[Rgba], labels: &[String]) -> Rgba {
    let (tw, th) = frames.iter().fold((1, 1), |(w, h), f| (w.max(f.w), h.max(f.h)));
    let cols = columns(frames.len(), tw, th);
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
    draw_text(out, text, x, y, SCALE, INK);
}

/// `text` in the 5 × 7 font at `scale`, in `ink`: digits, capitals (small letters are drawn as
/// capitals, but `s` and `f`, which labels write as they are) and common marks. Its width.
pub fn draw_text(out: &mut Rgba, text: &str, x: u32, y: u32, scale: u32, ink: [u8; 4]) -> u32 {
    let mut at = x;
    for ch in text.chars() {
        let shape = glyph(ch).or_else(|| glyph(ch.to_ascii_uppercase()));
        if let Some(rows) = shape {
            for (r, bits) in rows.iter().enumerate() {
                for c in 0..5 {
                    if bits & (0b10000 >> c) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                out.blend(at + c * scale + dx, y + r as u32 * scale + dy, ink);
                            }
                        }
                    }
                }
            }
        }
        at += 6 * scale;
    }
    at - x
}

/// How wide `text` is drawn at `scale`.
pub fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 6 * scale
}

fn glyph(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '#' => [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
        '\'' => [0b00100, 0b00100, 0b01000, 0, 0, 0, 0],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '&' => [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
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

/// The contrast of text drawn in `ink` at `coverage` (its alpha and opacity together) against
/// what is under it, in the box (`x`, `y`, `w`, `h`) of a picture drawn at `scale`. The ground is
/// the median, by luminance, of the pixels clearly not the ink's colour; when few pixels differ
/// from the ink, the ground is near the ink's own colour, which is the fault itself (glyphs cover
/// well under half a line's box, so the median of every pixel is then the ground's). The words
/// are judged as drawn: the ink over that ground at their coverage, so dimmed text reads dim.
/// `None` for a box outside the picture.
pub fn text_contrast(picture: &Rgba, scale: f64, rect: [f64; 4], ink: [u8; 3], coverage: f64) -> Option<f64> {
    let [x, y, w, h] = rect.map(|v| v * scale);
    let (x0, y0) = (x.max(0.0) as u32, y.max(0.0) as u32);
    let (x1, y1) = (((x + w).ceil() as u32).min(picture.w), ((y + h).ceil() as u32).min(picture.h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut ground: Vec<(f64, [u8; 3])> = Vec::new();
    let mut every: Vec<(f64, [u8; 3])> = Vec::new();
    // Every pixel in a small box, every few in a large one: about 4,000 at most.
    let step = (((x1 - x0) as f64 * (y1 - y0) as f64 / 4000.0).sqrt().floor() as u32).max(1);
    for py in (y0..y1).step_by(step as usize) {
        for px in (x0..x1).step_by(step as usize) {
            let p = picture.pixel(px, py);
            let rgb = [p[0], p[1], p[2]];
            let at = (luminance(rgb), rgb);
            every.push(at);
            let distance = rgb.iter().zip(ink).map(|(&a, b)| (a as i32 - b as i32).abs()).max().unwrap_or(0);
            if distance > 48 {
                ground.push(at);
            }
        }
    }
    if ground.len() * 5 < every.len() {
        ground = every;
    }
    ground.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (ground_l, ground_rgb) = ground[ground.len() / 2];
    let c = coverage.clamp(0.0, 1.0);
    let drawn = [0, 1, 2].map(|i| (ink[i] as f64 * c + ground_rgb[i] as f64 * (1.0 - c)).round() as u8);
    Some(ratio(luminance(drawn), ground_l))
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
    fn a_sheet_leaves_no_cell_empty_when_a_grid_fits() {
        // Vertical frames at the sheet's size (270 × 480).
        let fit = |n| { let c = columns(n, 270, 480); (c, n.div_ceil(c)) };
        assert_eq!(fit(6), (3, 2));
        assert_eq!(fit(8), (4, 2));
        assert_eq!(fit(12), (4, 3));
        assert_eq!(fit(2), (2, 1));
        // Wide frames stack.
        assert_eq!(columns(6, 480, 270), 2);
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
        let c = text_contrast(&picture, 1.0, [0.0, 0.0, 40.0, 20.0], [0, 0, 0], 1.0).unwrap();
        assert!((c - 21.0).abs() < 0.01, "{c}");
        // Light grey ink on a mid grey: poor.
        let grey = Rgba::new(20, 20, [119, 119, 119, 255]);
        let c = text_contrast(&grey, 1.0, [0.0, 0.0, 20.0, 20.0], [180, 180, 180], 1.0).unwrap();
        assert!(c < 3.0, "{c}");
        // Ink all but the ground's own colour: no pixel stands apart, and that is the fault.
        let mut dim = Rgba::new(20, 20, [40, 40, 40, 255]);
        for x in 5..9 {
            dim.put(x, 8, [70, 70, 70, 255]);
        }
        let c = text_contrast(&dim, 1.0, [0.0, 0.0, 20.0, 20.0], [70, 70, 70], 1.0).unwrap();
        assert!(c < 1.6, "{c}");
        // A box outside the picture says nothing.
        assert_eq!(text_contrast(&dim, 1.0, [40.0, 40.0, 10.0, 10.0], [0, 0, 0], 1.0), None);
        // At half scale the box is found in the smaller picture.
        assert!(text_contrast(&picture, 0.5, [0.0, 0.0, 80.0, 40.0], [0, 0, 0], 1.0).is_some());
        // Black words at a third of their opacity on white are a light grey: judged as drawn.
        let dimmed = text_contrast(&picture, 1.0, [0.0, 0.0, 40.0, 20.0], [0, 0, 0], 0.33).unwrap();
        assert!(dimmed < 3.0, "{dimmed}");
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
