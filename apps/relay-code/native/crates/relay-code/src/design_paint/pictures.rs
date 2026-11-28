//! The pictures image nodes show: decoded from the bytes the PC sends (`design.asset.get`, read
//! by `designs.rs`'s fetch thread), shrunk at decode time to what the page is ever drawn at, kept
//! for the session ([`KEEP_BYTES`] at most, the least lately drawn let go first), and sampled by
//! the painter. Everything here runs off the UI thread except [`status`], [`loading`],
//! [`changes`] and [`mark`], which only look in the keep.
//!
//! PNG, JPEG, WebP and a GIF's first frame are decoded; SVG stays a tile. A picture's name is
//! its content's hash on the PC (`design.asset.add`), shared by every design, so it is kept by
//! its name alone.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

/// The largest picture file the PC keeps (`MAX_ASSET_FILE`), so the most the phone reads.
pub const MAX_FILE: usize = 20 << 20;
/// How much of decoded pictures the phone keeps.
pub const KEEP_BYTES: usize = 64 << 20;
/// The most pixels a picture is decoded from: past it (a decompression bomb, a 100 MP scan) it
/// stays a tile.
const MAX_SOURCE_PX: u64 = 100_000_000;
/// The most a picture decoded whole before it is shrunk may be (WebP, GIF and interlaced PNG
/// have no row-by-row reading here).
const MAX_WHOLE_PX: u64 = 24_000_000;
/// The most pixels a picture is kept at, and its longest side.
const MAX_KEEP_PX: f64 = 4_000_000.0;
const MAX_KEEP_SIDE: f64 = 4096.0;

/// Why a picture stays a tile.
#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// SVG, or bytes that are none of the four kinds.
    Unsupported,
    TooBig,
    /// The file did not decode.
    Broken(String),
    /// The PC refused or lost it.
    Refused(String),
    /// The link dropped or timed out: worth asking again later.
    Link(String),
}

// A decoded picture

/// One size of a picture: premultiplied RGBA, row by row.
struct Level {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

/// A decoded picture and its smaller copies (each half the one before), for drawing it small
/// without shimmer.
pub struct Picture {
    pub w: u32,
    pub h: u32,
    levels: Vec<Level>,
}

impl Picture {
    /// From premultiplied RGBA pixels.
    pub fn new(w: u32, h: u32, data: Vec<u8>) -> Picture {
        let mut levels = vec![Level { w, h, data }];
        while let Some(l) = levels.last().filter(|l| l.w.max(l.h) > 16 && l.w > 1 && l.h > 1) {
            levels.push(half(l));
        }
        Picture { w, h, levels }
    }

    pub fn bytes(&self) -> usize {
        self.levels.iter().map(|l| l.data.len()).sum()
    }

    /// The copy to sample when `ratio` of the picture's pixels fall on one drawn pixel.
    pub fn level_for(&self, ratio: f64) -> usize {
        if ratio <= 2.0 {
            return 0;
        }
        (ratio.log2().floor() as usize).min(self.levels.len() - 1)
    }

    /// The colour at (`u`, `v`), 0 to 1 across the picture, in copy `level`: bilinear,
    /// premultiplied, 0 to 1, the edge pixels held past the edge.
    pub fn sample(&self, level: usize, u: f64, v: f64) -> [f32; 4] {
        let l = &self.levels[level.min(self.levels.len() - 1)];
        let x = (u * f64::from(l.w) - 0.5).clamp(0.0, f64::from(l.w - 1));
        let y = (v * f64::from(l.h) - 0.5).clamp(0.0, f64::from(l.h - 1));
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(l.w - 1), (y0 + 1).min(l.h - 1));
        let (fx, fy) = ((x - f64::from(x0)) as f32, (y - f64::from(y0)) as f32);
        let px = |x: u32, y: u32| {
            let i = ((y * l.w + x) * 4) as usize;
            [l.data[i], l.data[i + 1], l.data[i + 2], l.data[i + 3]].map(f32::from)
        };
        let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            (top + (bottom - top) * fy) / 255.0
        })
    }
}

/// `l` at half its size, each pixel the mean of four.
fn half(l: &Level) -> Level {
    let (w, h) = ((l.w / 2).max(1), (l.h / 2).max(1));
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x * 2, y * 2);
            let (sx1, sy1) = ((sx + 1).min(l.w - 1), (sy + 1).min(l.h - 1));
            for c in 0..4 {
                let at = |x: u32, y: u32| u32::from(l.data[((y * l.w + x) * 4 + c) as usize]);
                data.push(((at(sx, sy) + at(sx1, sy) + at(sx, sy1) + at(sx1, sy1) + 2) / 4) as u8);
            }
        }
    }
    Level { w, h, data }
}

// Base64

/// Standard base64 (RFC 4648, `+` and `/`), padded or not; line breaks and spaces skipped.
/// None for any other character.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const BAD: u8 = 255;
    static TABLE: LazyLock<[u8; 256]> = LazyLock::new(|| {
        let mut t = [BAD; 256];
        for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".iter().enumerate() {
            t[*c as usize] = i as u8;
        }
        t
    });
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut ended = false;
    for &c in s.as_bytes() {
        match c {
            b'=' => ended = true,
            b'\n' | b'\r' | b' ' | b'\t' => {}
            _ if ended => return None,
            _ => {
                let v = TABLE[c as usize];
                if v == BAD {
                    return None;
                }
                acc = (acc << 6) | u32::from(v);
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push((acc >> bits) as u8);
                    acc &= (1 << bits) - 1;
                }
            }
        }
    }
    Some(out)
}

// Decoding

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Other,
}

/// What the bytes are, by their first bytes (the name's extension is only what it claims).
pub fn sniff(b: &[u8]) -> Kind {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Kind::Png
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Kind::Jpeg
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Kind::Gif
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Kind::Webp
    } else {
        Kind::Other
    }
}

/// Whether a picture of this name may be drawn at all: not an SVG (those stay tiles, so their
/// bytes are not fetched).
pub fn fetchable(src: &str) -> bool {
    !src.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("svg"))
}

/// The size a `sw`×`sh` picture is kept at so it still covers `want` (px; (0, 0) for its own
/// size): never larger than it is, nor past [`MAX_KEEP_PX`] or [`MAX_KEEP_SIDE`].
pub fn keep_size(sw: u32, sh: u32, want: (u32, u32)) -> (u32, u32) {
    let (fw, fh) = (f64::from(sw.max(1)), f64::from(sh.max(1)));
    let mut s: f64 = if want.0 == 0 && want.1 == 0 { 1.0 } else { (f64::from(want.0) / fw).max(f64::from(want.1) / fh) };
    s = s.min(1.0).min((MAX_KEEP_PX / (fw * fh)).sqrt()).min(MAX_KEEP_SIDE / fw.max(fh));
    (((fw * s).round() as u32).max(1), ((fh * s).round() as u32).max(1))
}

/// Decode `bytes` (any of the four kinds) to a picture that covers `want` px.
pub fn decode(bytes: &[u8], want: (u32, u32)) -> Result<Picture, Why> {
    let broken = |e: &dyn std::fmt::Display| Why::Broken(e.to_string());
    match sniff(bytes) {
        Kind::Png => png_decode(bytes, want),
        Kind::Jpeg => jpeg_decode(bytes, want),
        Kind::Webp => {
            let mut d = image_webp::WebPDecoder::new(Cursor::new(bytes)).map_err(|e| broken(&e))?;
            let (w, h) = d.dimensions();
            whole_size(w, h)?;
            let n = d.output_buffer_size().ok_or(Why::TooBig)?;
            let mut buf = vec![0u8; n];
            d.read_image(&mut buf).map_err(|e| broken(&e))?;
            let ch = if d.has_alpha() { 4 } else { 3 };
            let mut sh = Shrink::new(w, h, want);
            let mut row = Vec::with_capacity(w as usize * 4);
            for r in buf.chunks_exact(w as usize * ch) {
                to_rgba(r, ch, &mut row);
                sh.push(&row);
            }
            Ok(sh.finish())
        }
        Kind::Gif => {
            let mut o = gif::DecodeOptions::new();
            o.set_color_output(gif::ColorOutput::RGBA);
            o.set_memory_limit(gif::MemoryLimit::Bytes(std::num::NonZeroU64::new(MAX_WHOLE_PX * 4).expect("not zero")));
            let mut d = o.read_info(Cursor::new(bytes)).map_err(|e| broken(&e))?;
            let (w, h) = (u32::from(d.width()), u32::from(d.height()));
            whole_size(w, h)?;
            let f = d.read_next_frame().map_err(|e| broken(&e))?.ok_or_else(|| Why::Broken("no frames".into()))?;
            // The first frame on the screen it is part of, clear around it.
            let mut canvas = vec![0u8; (w * h * 4) as usize];
            let (fl, ft, fw, fh) = (u32::from(f.left), u32::from(f.top), u32::from(f.width), u32::from(f.height));
            for y in 0..fh.min(h.saturating_sub(ft)) {
                let n = fw.min(w.saturating_sub(fl)) as usize * 4;
                let s = (y * fw * 4) as usize;
                let d0 = (((ft + y) * w + fl) * 4) as usize;
                if let Some(src) = f.buffer.get(s..s + n) {
                    canvas[d0..d0 + n].copy_from_slice(src);
                }
            }
            let mut sh = Shrink::new(w, h, want);
            for r in canvas.chunks_exact(w as usize * 4) {
                sh.push(r);
            }
            Ok(sh.finish())
        }
        Kind::Other => Err(Why::Unsupported),
    }
}

fn whole_size(w: u32, h: u32) -> Result<(), Why> {
    if w == 0 || h == 0 {
        return Err(Why::Broken("no pixels".into()));
    }
    if u64::from(w) * u64::from(h) > MAX_WHOLE_PX { Err(Why::TooBig) } else { Ok(()) }
}

fn png_decode(bytes: &[u8], want: (u32, u32)) -> Result<Picture, Why> {
    let broken = |e: png::DecodingError| Why::Broken(e.to_string());
    let mut d = png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: 96 << 20 });
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().map_err(broken)?;
    let (w, h) = (r.info().width, r.info().height);
    if w == 0 || h == 0 {
        return Err(Why::Broken("no pixels".into()));
    }
    if u64::from(w) * u64::from(h) > MAX_SOURCE_PX {
        return Err(Why::TooBig);
    }
    let ch = match r.output_color_type().0 {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(Why::Broken("palette not expanded".into())),
    };
    let mut sh = Shrink::new(w, h, want);
    let mut row = Vec::with_capacity(w as usize * 4);
    if r.info().interlaced {
        whole_size(w, h)?;
        let mut buf = vec![0u8; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).map_err(broken)?;
        for line in buf.chunks_exact(info.line_size).take(h as usize) {
            to_rgba(&line[..w as usize * ch], ch, &mut row);
            sh.push(&row);
        }
    } else {
        // Row by row into the shrinker: a big PNG never sits whole in memory.
        while let Some(line) = r.next_row().map_err(broken)? {
            to_rgba(&line.data()[..w as usize * ch], ch, &mut row);
            sh.push(&row);
        }
    }
    Ok(sh.finish())
}

fn jpeg_decode(bytes: &[u8], want: (u32, u32)) -> Result<Picture, Why> {
    let broken = |e: jpeg_decoder::Error| Why::Broken(e.to_string());
    let mut d = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    d.read_info().map_err(broken)?;
    let info = d.info().ok_or_else(|| Why::Broken("no header".into()))?;
    let (sw, sh_) = (u32::from(info.width), u32::from(info.height));
    if sw == 0 || sh_ == 0 {
        return Err(Why::Broken("no pixels".into()));
    }
    if u64::from(sw) * u64::from(sh_) > MAX_SOURCE_PX {
        return Err(Why::TooBig);
    }
    // Decoded straight at 1/2, 1/4 or 1/8 when that still covers what is kept.
    let (kw, kh) = keep_size(sw, sh_, want);
    let (w, h) = d.scale(kw.min(65535) as u16, kh.min(65535) as u16).map_err(broken)?;
    let (w, h) = (u32::from(w), u32::from(h));
    let pixels = d.decode().map_err(broken)?;
    let info = d.info().ok_or_else(|| Why::Broken("no header".into()))?;
    let ch = info.pixel_format.pixel_bytes();
    if pixels.len() < (w * h) as usize * ch {
        return Err(Why::Broken("short picture".into()));
    }
    let mut sh = Shrink::new(w, h, (kw, kh));
    let mut row = Vec::with_capacity(w as usize * 4);
    for r in pixels.chunks_exact(w as usize * ch).take(h as usize) {
        match info.pixel_format {
            jpeg_decoder::PixelFormat::L8 => to_rgba(r, 1, &mut row),
            jpeg_decoder::PixelFormat::RGB24 => to_rgba(r, 3, &mut row),
            jpeg_decoder::PixelFormat::L16 => {
                let grey: Vec<u8> = r.as_chunks::<2>().0.iter().map(|p| p[0]).collect();
                to_rgba(&grey, 1, &mut row);
            }
            jpeg_decoder::PixelFormat::CMYK32 => {
                row.clear();
                for p in r.as_chunks::<4>().0 {
                    // Stored inverted, as Adobe writes it (what the image crate assumes).
                    let k = u32::from(p[3]);
                    row.extend_from_slice(&[(u32::from(p[0]) * k / 255) as u8, (u32::from(p[1]) * k / 255) as u8, (u32::from(p[2]) * k / 255) as u8, 255]);
                }
            }
        }
        sh.push(&row);
    }
    Ok(sh.finish())
}

/// A row of `ch`-channel pixels (grey, grey and alpha, RGB, RGBA) as straight RGBA.
fn to_rgba(src: &[u8], ch: usize, out: &mut Vec<u8>) {
    out.clear();
    match ch {
        1 => src.iter().for_each(|&g| out.extend_from_slice(&[g, g, g, 255])),
        2 => src.as_chunks::<2>().0.iter().for_each(|p| out.extend_from_slice(&[p[0], p[0], p[0], p[1]])),
        3 => src.as_chunks::<3>().0.iter().for_each(|p| out.extend_from_slice(&[p[0], p[1], p[2], 255])),
        _ => out.extend_from_slice(src),
    }
}

/// Shrinks a picture fed row by row (straight RGBA) to its kept size ([`keep_size`]), each kept
/// pixel the mean of the area it covers, premultiplied. Holds two rows of sums, never the whole
/// source.
struct Shrink {
    sw: u32,
    sh: u32,
    w: u32,
    h: u32,
    /// Per source column: the kept column it falls in, its share there and in the next.
    cols: Vec<(u32, f32, f32)>,
    ky: f64,
    y: u32,
    /// The kept row `at` and the one after, summed so far.
    at: u32,
    acc: [Vec<f32>; 2],
    line: Vec<f32>,
    out: Vec<u8>,
}

impl Shrink {
    fn new(sw: u32, sh: u32, want: (u32, u32)) -> Shrink {
        let (w, h) = keep_size(sw, sh, want);
        let kx = f64::from(sw) / f64::from(w);
        let cols = (0..sw)
            .map(|x| {
                let j = ((f64::from(x) / kx).floor() as u32).min(w - 1);
                let end = f64::from(j + 1) * kx;
                if f64::from(x + 1) <= end + 1e-9 || j + 1 >= w { (j, 1.0, 0.0) } else { (j, (end - f64::from(x)) as f32, (f64::from(x + 1) - end) as f32) }
            })
            .collect();
        Shrink {
            sw,
            sh,
            w,
            h,
            cols,
            ky: f64::from(sh) / f64::from(h),
            y: 0,
            at: 0,
            acc: [vec![0.0; (w * 4) as usize], vec![0.0; (w * 4) as usize]],
            line: vec![0.0; (w * 4) as usize],
            out: Vec::with_capacity((w * h * 4) as usize),
        }
    }

    fn push(&mut self, row: &[u8]) {
        if self.y >= self.sh || row.len() < self.sw as usize * 4 {
            return;
        }
        self.line.iter_mut().for_each(|v| *v = 0.0);
        for (x, &(j, a, b)) in self.cols.iter().enumerate() {
            let p = &row[x * 4..x * 4 + 4];
            let al = f32::from(p[3]) / 255.0;
            let px = [f32::from(p[0]) * al, f32::from(p[1]) * al, f32::from(p[2]) * al, f32::from(p[3])];
            let i = j as usize * 4;
            for (l, v) in self.line[i..i + 4].iter_mut().zip(px) {
                *l += v * a;
            }
            if b > 0.0 {
                for (l, v) in self.line[i + 4..i + 8].iter_mut().zip(px) {
                    *l += v * b;
                }
            }
        }
        let y = self.y;
        let j = ((f64::from(y) / self.ky).floor() as u32).min(self.h - 1);
        let end = f64::from(j + 1) * self.ky;
        let (a, b) = if f64::from(y + 1) <= end + 1e-9 || j + 1 >= self.h { (1.0, 0.0) } else { ((end - f64::from(y)) as f32, (f64::from(y + 1) - end) as f32) };
        while self.at < j {
            self.emit();
        }
        for (s, l) in self.acc[0].iter_mut().zip(&self.line) {
            *s += l * a;
        }
        if b > 0.0 {
            for (s, l) in self.acc[1].iter_mut().zip(&self.line) {
                *s += l * b;
            }
        }
        self.y += 1;
    }

    /// Write kept row `at` and move on to the next.
    fn emit(&mut self) {
        let area = (f64::from(self.sw) / f64::from(self.w) * self.ky) as f32;
        self.out.extend(self.acc[0].iter().map(|v| (v / area).round().clamp(0.0, 255.0) as u8));
        self.acc.swap(0, 1);
        self.acc[1].iter_mut().for_each(|v| *v = 0.0);
        self.at += 1;
    }

    fn finish(mut self) -> Picture {
        while self.at < self.h {
            self.emit();
        }
        Picture::new(self.w, self.h, self.out)
    }
}

// The keep

/// Where a picture stands on the phone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    /// Not asked for (or let go).
    Absent,
    Loading,
    Ready,
    /// It stays a tile for this session.
    Failed,
}

enum Slot {
    Loading,
    Ready { pic: Arc<Picture>, serial: u64, used: u64 },
    Failed,
}

#[derive(Default)]
struct Keep {
    slots: HashMap<String, Slot>,
    bytes: usize,
    tick: u64,
}

static KEEP: LazyLock<Mutex<Keep>> = LazyLock::new(|| Mutex::new(Keep::default()));
/// Bumped whenever a picture comes, fails or is let go: what [`mark`]s are read again on.
static CHANGES: AtomicU64 = AtomicU64::new(1);
static SERIAL: AtomicU64 = AtomicU64::new(1);

fn keep() -> std::sync::MutexGuard<'static, Keep> {
    KEEP.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn changes() -> u64 {
    CHANGES.load(Ordering::Relaxed)
}

pub fn status(src: &str) -> Status {
    match keep().slots.get(src) {
        None => Status::Absent,
        Some(Slot::Loading) => Status::Loading,
        Some(Slot::Ready { .. }) => Status::Ready,
        Some(Slot::Failed) => Status::Failed,
    }
}

/// The picture `src`, if it is here (marked as drawn now).
pub fn get(src: &str) -> Option<Arc<Picture>> {
    let mut k = keep();
    k.tick += 1;
    let tick = k.tick;
    match k.slots.get_mut(src) {
        Some(Slot::Ready { pic, used, .. }) => {
            *used = tick;
            Some(pic.clone())
        }
        _ => None,
    }
}

/// `src` is being fetched.
pub fn loading(src: &str) {
    let mut k = keep();
    if !k.slots.contains_key(src) {
        k.slots.insert(src.to_string(), Slot::Loading);
        CHANGES.fetch_add(1, Ordering::Relaxed);
    }
}

/// A fetch that never ran (its thread ended first): `src` may be asked for again.
pub fn forget(src: &str) {
    let mut k = keep();
    if matches!(k.slots.get(src), Some(Slot::Loading)) {
        k.slots.remove(src);
        CHANGES.fetch_add(1, Ordering::Relaxed);
    }
}

/// What came of fetching `src`. A link that dropped leaves it absent, to be asked for again.
pub fn put(src: &str, r: Result<Picture, Why>) {
    let mut k = keep();
    let k = &mut *k;
    if let Some(Slot::Ready { pic, .. }) = k.slots.remove(src) {
        k.bytes -= pic.bytes();
    }
    match r {
        Ok(pic) => {
            k.tick += 1;
            k.bytes += pic.bytes();
            k.slots.insert(src.to_string(), Slot::Ready { pic: Arc::new(pic), serial: SERIAL.fetch_add(1, Ordering::Relaxed), used: k.tick });
            // The least lately drawn go first; the one just come stays.
            while k.bytes > KEEP_BYTES {
                let oldest = k.slots.iter().filter(|(s, _)| *s != src).filter_map(|(s, v)| if let Slot::Ready { used, .. } = v { Some((*used, s.clone())) } else { None }).min();
                let Some((_, s)) = oldest else { break };
                if let Some(Slot::Ready { pic, .. }) = k.slots.remove(&s) {
                    k.bytes -= pic.bytes();
                }
            }
        }
        Err(Why::Link(_)) => {}
        Err(_) => {
            k.slots.insert(src.to_string(), Slot::Failed);
        }
    }
    CHANGES.fetch_add(1, Ordering::Relaxed);
}

/// Which of `srcs` are here, as one number: it changes when one comes or is let go, so a
/// picture painted with them is named by it and painted again when it changes. 0 for none.
pub fn mark<'a>(srcs: impl IntoIterator<Item = &'a str>) -> u64 {
    let k = keep();
    srcs.into_iter()
        .filter_map(|s| if let Some(Slot::Ready { serial, .. }) = k.slots.get(s) { Some(*serial) } else { None })
        .fold(0u64, |m, s| m.wrapping_add(s.wrapping_mul(0x9E37_79B9_7F4A_7C15)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes_what_attach_encodes() {
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(base64_decode("Zm9v\nYmE").unwrap(), b"fooba");
        assert!(base64_decode("Zm9v*").is_none());
        assert!(base64_decode("Zg==Zg").is_none());
        let all: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        assert_eq!(base64_decode(&crate::attach::base64(&all)).unwrap(), all);
    }

    #[test]
    fn kinds_are_told_by_their_bytes() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nxxxx"), Kind::Png);
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Kind::Jpeg);
        assert_eq!(sniff(b"GIF89a.."), Kind::Gif);
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Kind::Webp);
        assert_eq!(sniff(b"<svg xmlns"), Kind::Other);
        assert!(matches!(decode(b"<svg/>", (10, 10)), Err(Why::Unsupported)));
        assert!(!fetchable("ab12.svg") && fetchable("ab12.jpg"));
    }

    #[test]
    fn pictures_are_kept_only_as_big_as_they_are_drawn() {
        assert_eq!(keep_size(4000, 3000, (400, 100)), (400, 300));
        assert_eq!(keep_size(100, 50, (400, 400)), (100, 50));
        assert_eq!(keep_size(100, 50, (0, 0)), (100, 50));
        let (w, h) = keep_size(8000, 8000, (8000, 8000));
        assert!(f64::from(w) * f64::from(h) <= MAX_KEEP_PX + 1e4 && w == h);
    }

    pub(crate) fn png_bytes(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let px: Vec<u8> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).flat_map(|(x, y)| f(x, y)).collect();
        e.write_header().unwrap().write_image_data(&px).unwrap();
        out
    }

    #[test]
    fn a_png_decodes_and_shrinks_by_area() {
        // Left half red, right half blue, 40×20: shrunk to 4×2 each side keeps its colour.
        let b = png_bytes(40, 20, |x, _| if x < 20 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        let p = decode(&b, (4, 2)).unwrap();
        assert_eq!((p.w, p.h), (4, 2));
        assert_eq!(p.sample(0, 0.1, 0.5), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(p.sample(0, 0.9, 0.5), [0.0, 0.0, 1.0, 1.0]);
        // Whole, when nothing smaller is asked.
        let p = decode(&b, (0, 0)).unwrap();
        assert_eq!((p.w, p.h), (40, 20));
        // Half transparent white is kept premultiplied.
        let t = decode(&png_bytes(2, 2, |_, _| [255, 255, 255, 128]), (0, 0)).unwrap();
        let c = t.sample(0, 0.5, 0.5);
        assert!((c[3] - 128.0 / 255.0).abs() < 1e-3 && (c[0] - c[3]).abs() < 0.01, "{c:?}");
        // An odd shrink keeps the mean.
        let g = decode(&png_bytes(7, 5, |x, y| if (x + y) % 2 == 0 { [200, 200, 200, 255] } else { [0, 0, 0, 255] }), (3, 2)).unwrap();
        let mid = g.sample(0, 0.5, 0.5)[0];
        assert!((mid - 100.0 / 255.0).abs() < 0.12, "{mid}");
    }

    #[test]
    fn the_sample_jpeg_decodes_at_a_smaller_scale() {
        let b = crate::designs::SAMPLE_PICTURE;
        assert_eq!(sniff(b), Kind::Jpeg);
        let full = decode(b, (0, 0)).unwrap();
        let small = decode(b, (full.w / 4, full.h / 4)).unwrap();
        assert_eq!((small.w, small.h), keep_size(full.w, full.h, (full.w / 4, full.h / 4)));
        assert!(small.levels.len() > 1 && small.levels.last().unwrap().w <= 16);
        // A cut file decodes to what it has, or fails; it never panics.
        let _ = decode(&b[..b.len() / 3], (0, 0));
    }

    #[test]
    fn the_keep_lets_the_least_lately_drawn_go() {
        let big = |v: u8| Picture::new(2048, 2048, vec![v; 2048 * 2048 * 4]);
        let names: Vec<String> = (0..6).map(|i| format!("keep-test-{i}.png")).collect();
        for n in &names {
            loading(n);
            assert_eq!(status(n), Status::Loading);
            put(n, Ok(big(1)));
            // The first is drawn every time, so it is never the one let go.
            assert!(get(&names[0]).is_some());
        }
        assert!(keep().bytes <= KEEP_BYTES);
        assert_eq!(status(&names[0]), Status::Ready);
        assert_eq!(status(&names[5]), Status::Ready);
        assert_eq!(status(&names[1]), Status::Absent);
        let m = mark(names.iter().map(String::as_str));
        put("keep-test-broken.png", Err(Why::Broken("x".into())));
        assert_eq!(status("keep-test-broken.png"), Status::Failed);
        assert_eq!(mark(names.iter().map(String::as_str)), m);
        loading("keep-test-link.png");
        put("keep-test-link.png", Err(Why::Link("down".into())));
        assert_eq!(status("keep-test-link.png"), Status::Absent);
        for n in &names {
            put(n, Err(Why::Unsupported));
        }
    }
}
