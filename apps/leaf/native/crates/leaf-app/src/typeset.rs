//! Setting a book's blocks in lines: Literata at the reader's size, broken at spaces (no
//! hyphenation, ragged right), each line a few runs of one face. One block at a time, only the
//! blocks near the screen, so a whole book is never laid out at once.
//!
//! Units are the canvas's layout units; widths come from the canvas's own fonts at the size it
//! will draw them, so what is measured is what is drawn.

use crate::draw::{Canvas, Text, WIDTH, Weight};
use crate::epub::{BOLD, Block, ITALIC, Kind, SUP};

/// The reader's type sheet.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Type {
    /// Body size in units.
    pub size: f32,
    /// 0 tight, 1 normal, 2 loose.
    pub spacing: u8,
    /// 0 small, 1 medium, 2 large.
    pub margins: u8,
    /// 0 Paper, 1 Sepia, 2 Dark, 3 OLED.
    pub theme: u8,
}

impl Default for Type {
    fn default() -> Type {
        Type { size: 18.0, spacing: 1, margins: 1, theme: 2 }
    }
}

pub const SIZES: (f32, f32) = (14.0, 28.0);

impl Type {
    pub fn line(&self) -> f32 {
        [1.42, 1.6, 1.8][self.spacing.min(2) as usize]
    }
    pub fn margin(&self) -> f32 {
        [16.0, 24.0, 34.0][self.margins.min(2) as usize]
    }
    /// The text column's width.
    pub fn column(&self) -> f32 {
        WIDTH - 2.0 * self.margin()
    }
    /// What changes the layout (not the theme).
    pub fn key(&self) -> (u32, u8, u8) {
        ((self.size * 10.0).round() as u32, self.spacing, self.margins)
    }
}

/// A run of one face on a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub x: f32,
    pub start: u32,
    pub end: u32,
    pub face: Weight,
    pub size: f32,
    pub style: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Top of the line box, from the block's top.
    pub top: f32,
    pub h: f32,
    pub start: u32,
    pub end: u32,
    pub runs: Vec<Run>,
}

/// A block set in lines.
#[derive(Clone, Debug, Default)]
pub struct Laid {
    /// Its whole height, the space above and below included.
    pub h: f32,
    pub lines: Vec<Line>,
    /// The text column's left edge for this block (quotes and list items are indented).
    pub left: f32,
    /// A picture's box (x, y, w, h).
    pub image: Option<[f32; 4]>,
}

/// How a block's kind is set: size, face, line height multiple, space above, space below, indent.
fn look(kind: &Kind, t: &Type) -> (f32, Weight, f32, f32, f32, f32) {
    let s = t.size;
    let lh = t.line();
    match kind {
        Kind::Heading(1) => (s * 1.45, Weight::SerifBold, 1.25, s * 1.6, s * 0.7, 0.0),
        Kind::Heading(2) => (s * 1.3, Weight::SerifBold, 1.25, s * 1.4, s * 0.6, 0.0),
        Kind::Heading(3) => (s * 1.15, Weight::SerifBold, 1.3, s * 1.2, s * 0.5, 0.0),
        Kind::Heading(_) => (s * 1.0, Weight::SerifBold, 1.35, s * 1.0, s * 0.4, 0.0),
        Kind::Quote => (s * 0.95, Weight::Serif, lh, 0.0, s * 0.85, 20.0),
        Kind::Item(d, _) => (s, Weight::Serif, lh, 0.0, s * 0.45, 22.0 * *d.min(&4) as f32),
        Kind::Pre => (s * 0.78, Weight::Regular, 1.45, 0.0, s * 0.85, 8.0),
        _ => (s, Weight::Serif, lh, 0.0, s * 0.85, 0.0),
    }
}

/// The face a styled part is set in, from the block's face.
fn face_of(base: Weight, style: u8) -> Weight {
    if base != Weight::Serif {
        return base;
    }
    if style & BOLD != 0 {
        Weight::SerifBold
    } else if style & ITALIC != 0 {
        Weight::SerifItalic
    } else {
        Weight::Serif
    }
}

/// Set one block. `image` is a picture's size in pixels, once known.
pub fn lay(c: &mut Canvas, b: &Block, t: &Type, image: Option<(u32, u32)>) -> Laid {
    let col = t.column();
    match &b.kind {
        Kind::Image(..) => {
            let (w, h) = match image {
                Some((pw, ph)) if pw > 0 && ph > 0 => {
                    // Shown at its own size up to the column, never taller than most of a screen.
                    let mut w = (pw as f32 / 2.2).min(col);
                    let mut h = w * ph as f32 / pw as f32;
                    if h > 560.0 {
                        h = 560.0;
                        w = h * pw as f32 / ph as f32;
                    }
                    (w, h)
                }
                _ => (col, 180.0),
            };
            let gap = t.size * 0.9;
            Laid { h: h + 2.0 * gap, lines: Vec::new(), left: t.margin(), image: Some([(WIDTH - w) / 2.0, gap, w, h]) }
        }
        Kind::Rule => Laid { h: t.size * 2.4, lines: Vec::new(), left: t.margin(), image: None },
        kind => {
            let (size, face, line, above, below, indent) = look(kind, t);
            let left = t.margin() + indent;
            let lines = wrap(c, b, col - indent, size, face, size * line, above);
            let text_h = lines.last().map_or(0.0, |l| l.top + l.h - above);
            Laid { h: above + text_h + below, lines, left, image: None }
        }
    }
}

/// A piece of a word in one style.
struct Part {
    start: u32,
    end: u32,
    style: u8,
    w: f32,
}

fn measure(c: &mut Canvas, s: &str, face: Weight, size: f32) -> f32 {
    c.measure(s, Text::new(size, crate::draw::Rgba::WHITE).face(face))
}

fn part_size(size: f32, style: u8) -> f32 {
    if style & SUP != 0 { size * 0.68 } else { size }
}

/// Break the block's text into lines no wider than `width`.
fn wrap(c: &mut Canvas, b: &Block, width: f32, size: f32, face: Weight, lh: f32, top: f32) -> Vec<Line> {
    let text = b.text.as_str();
    // Style boundaries, in order.
    let mut cuts: Vec<u32> = vec![0, text.len() as u32];
    for s in &b.spans {
        cuts.push(s.start);
        cuts.push(s.end);
    }
    // Words: a run of non-spaces and the spaces after it; a newline ends a line.
    let mut words: Vec<(u32, u32, bool)> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\n' {
            // A break after a dash, as well as at spaces.
            let ch = text[i..].chars().next().unwrap_or(' ');
            i += ch.len_utf8();
            if (ch == '—' || ch == '–' || ch == '/') && i < bytes.len() && bytes[i] != b' ' {
                break;
            }
        }
        let mut hard = false;
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
            if bytes[i] == b'\n' {
                hard = true;
                i += 1;
                break;
            }
            i += 1;
        }
        words.push((start as u32, i as u32, hard));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let parts_of = |c: &mut Canvas, a: u32, z: u32| -> Vec<Part> {
        let mut out = Vec::new();
        let mut s = a;
        while s < z {
            let next = cuts.iter().copied().find(|&k| k > s).unwrap_or(z).min(z);
            let style = b.style_at(s);
            let piece = text[s as usize..next as usize].trim_end_matches('\n');
            let w = measure(c, piece, face_of(face, style), part_size(size, style));
            out.push(Part { start: s, end: next, style, w });
            s = next;
        }
        out
    };
    let mut lines: Vec<Line> = Vec::new();
    let mut cur: Vec<Part> = Vec::new();
    let mut x = 0.0f32;
    let push_line = |lines: &mut Vec<Line>, parts: &mut Vec<Part>| {
        let top = top + lines.len() as f32 * lh;
        let start = parts.first().map_or(0, |p| p.start);
        let end = parts.last().map_or(start, |p| p.end);
        let mut runs: Vec<Run> = Vec::new();
        let mut rx = 0.0;
        for p in parts.drain(..) {
            let f = face_of(face, p.style);
            let sz = part_size(size, p.style);
            match runs.last_mut() {
                Some(r) if r.style == p.style && r.end == p.start => r.end = p.end,
                _ => runs.push(Run { x: rx, start: p.start, end: p.end, face: f, size: sz, style: p.style }),
            }
            rx += p.w;
        }
        lines.push(Line { top, h: lh, start, end, runs });
    };
    for (a, z, hard) in words {
        let parts = parts_of(c, a, z);
        // The word's width without its trailing spaces decides whether it fits.
        let trimmed_end = a + text[a as usize..z as usize].trim_end().len() as u32;
        let ink: f32 = if trimmed_end == z { parts.iter().map(|p| p.w).sum() } else { parts_of(c, a, trimmed_end).iter().map(|p| p.w).sum() };
        if x > 0.0 && x + ink > width {
            push_line(&mut lines, &mut cur);
            x = 0.0;
        }
        if ink > width {
            // One word wider than the column (a web address): broken where it must be.
            for (s, e) in split_long(c, text, a, z, width, size, face, b) {
                let ps = parts_of(c, s, e);
                cur.extend(ps);
                push_line(&mut lines, &mut cur);
            }
            x = 0.0;
            continue;
        }
        x += parts.iter().map(|p| p.w).sum::<f32>();
        cur.extend(parts);
        if hard {
            push_line(&mut lines, &mut cur);
            x = 0.0;
        }
    }
    if !cur.is_empty() {
        push_line(&mut lines, &mut cur);
    }
    lines
}

/// A word too wide for a line, cut into pieces that fit.
#[allow(clippy::too_many_arguments)]
fn split_long(c: &mut Canvas, text: &str, a: u32, z: u32, width: f32, size: f32, face: Weight, b: &Block) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut s = a as usize;
    while s < z as usize {
        let mut e = s;
        for (i, ch) in text[s..z as usize].char_indices() {
            let cand = s + i + ch.len_utf8();
            let w = measure(c, &text[s..cand], face_of(face, b.style_at(s as u32)), size);
            if w > width && e > s {
                break;
            }
            e = cand;
        }
        out.push((s as u32, e as u32));
        s = e;
    }
    out
}

impl Laid {
    /// The line at `y` from the block's top (the nearest one).
    pub fn line_at(&self, y: f32) -> Option<&Line> {
        self.lines.iter().find(|l| y < l.top + l.h).or(self.lines.last())
    }

    /// The line holding byte `i`.
    pub fn line_of(&self, i: u32) -> Option<&Line> {
        self.lines.iter().find(|l| i < l.end).or(self.lines.last())
    }

    /// Where byte `i` sits across its line (column units, from `left`).
    pub fn x_of(&self, c: &mut Canvas, text: &str, i: u32) -> f32 {
        let Some(l) = self.line_of(i) else { return 0.0 };
        x_in_line(c, text, l, i)
    }

    /// The byte nearest the point (`x` from the column's left, `y` from the block's top).
    pub fn hit(&self, c: &mut Canvas, text: &str, x: f32, y: f32) -> u32 {
        let Some(l) = self.line_at(y) else { return 0 };
        let mut best = (f32::MAX, l.start);
        let mut i = l.start as usize;
        let end = l.end as usize;
        loop {
            let d = (x_in_line(c, text, l, i as u32) - x).abs();
            if d < best.0 {
                best = (d, i as u32);
            }
            if i >= end {
                break;
            }
            i += text[i..].chars().next().map_or(1, char::len_utf8);
        }
        best.1
    }
}

fn x_in_line(c: &mut Canvas, text: &str, l: &Line, i: u32) -> f32 {
    for r in &l.runs {
        if i <= r.end {
            let i = i.max(r.start).min(r.end);
            return r.x + measure(c, &text[r.start as usize..i as usize], r.face, r.size);
        }
    }
    l.runs.last().map_or(0.0, |r| r.x + measure(c, &text[r.start as usize..r.end as usize], r.face, r.size))
}

/// The word around byte `i`: (start, end).
pub fn word_at(text: &str, i: u32) -> (u32, u32) {
    let i = (i as usize).min(text.len());
    let is_word = |c: char| c.is_alphanumeric() || c == '\'' || c == '’' || c == '-';
    let mut a = i;
    while a > 0 {
        let prev = text[..a].chars().next_back().unwrap_or(' ');
        if !is_word(prev) {
            break;
        }
        a -= prev.len_utf8();
    }
    let mut z = i;
    while z < text.len() {
        let next = text[z..].chars().next().unwrap_or(' ');
        if !is_word(next) {
            break;
        }
        z += next.len_utf8();
    }
    if z == a && z < text.len() {
        z += text[z..].chars().next().map_or(1, char::len_utf8);
    }
    (a as u32, z as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epub::Span;

    fn block(text: &str, spans: Vec<Span>) -> Block {
        Block { kind: Kind::Para, text: text.into(), spans, links: Vec::new(), at: 0 }
    }

    fn canvas() -> Canvas {
        let mut c = Canvas::new(crate::app::FONTS);
        c.begin(1080.0, 2340.0, 0.0, 0.0);
        c
    }

    #[test]
    fn a_paragraph_wraps_within_the_column_and_covers_every_byte() {
        let mut c = canvas();
        let t = Type::default();
        let text = "Think of the last time you guessed how often something happens. You probably did not count. You noticed how quickly examples came to mind and took that speed as the answer.";
        let b = block(text, vec![Span { start: 6, end: 8, style: ITALIC }]);
        let l = lay(&mut c, &b, &t, None);
        assert!(l.lines.len() >= 4, "{} lines", l.lines.len());
        for w in l.lines.windows(2) {
            assert_eq!(w[0].end, w[1].start, "lines meet");
        }
        assert_eq!(l.lines[0].start, 0);
        assert_eq!(l.lines.last().unwrap().end, text.len() as u32);
        for line in &l.lines {
            let width = x_in_line(&mut c, text, line, text[..line.end as usize].trim_end().len() as u32);
            assert!(width <= t.column() + 0.5, "{width} > {}", t.column());
        }
        assert!(l.lines[0].runs.iter().any(|r| r.face == Weight::SerifItalic));
        // A bigger size makes more lines.
        let big = lay(&mut c, &b, &Type { size: 26.0, ..t }, None);
        assert!(big.lines.len() > l.lines.len());
        assert!(big.h > l.h);
    }

    #[test]
    fn hits_land_on_the_byte_under_the_finger() {
        let mut c = canvas();
        let t = Type::default();
        let text = "Ease of recall is a signal about your memory, not about the world.";
        let b = block(text, Vec::new());
        let l = lay(&mut c, &b, &t, None);
        let i = text.find("recall").unwrap() as u32;
        let x = l.x_of(&mut c, text, i);
        let y = l.lines[0].top + 4.0;
        assert_eq!(l.hit(&mut c, text, x + 1.0, y), i);
        assert_eq!(word_at(text, i + 2), (i, i + 6));
        let m = text.find("memory").unwrap() as u32;
        assert_eq!(word_at(text, m), (m, m + 6));
    }

    #[test]
    fn a_long_address_breaks_and_newlines_end_lines() {
        let mut c = canvas();
        let t = Type::default();
        let text = format!("See https://example.com/{} now\nand then", "a".repeat(120));
        let l = lay(&mut c, &block(&text, Vec::new()), &t, None);
        assert!(l.lines.len() >= 4);
        let last = l.lines.last().unwrap();
        assert_eq!(&text[last.start as usize..last.end as usize], "and then");
    }
}
