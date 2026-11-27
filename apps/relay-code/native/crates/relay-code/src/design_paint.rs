//! A design's page as the phone draws it (design 44, D11): laid out by the PC's own rules
//! (`relay_client::design::layout::place`, its words measured here with the app's own fonts),
//! then painted on the CPU into a picture the canvas shows, as the Kotlin app's `DesignCanvas.kt`
//! and the PC's `paint.rs` paint it. Both run on `designs.rs`'s worker, never in a frame: a page
//! is read and laid out once when it arrives ([`plan`]), and each picture is painted once for its
//! size ([`board`], [`page`], [`strip`], [`region`]).
//!
//! Painted: frames (fill, gradient, corners, clip, borders on each side, dashes), rects,
//! stickies, ellipses and arcs, polygons and stars, lines, paths and connectors with their
//! arrowheads, icons, words (face, weight, size, colour, alignment, wrapping as laid out, runs'
//! colours and weights, underline, strike, a cut last line), outer and inner shadows, opacity
//! and turns, and pictures: an image node's PNG, JPEG, WebP or GIF (its first frame), fetched
//! from the PC by `designs.rs`'s fetch thread and decoded there ([`pictures`]), drawn into its
//! box by its fit (cover, contain, stretch), corners, opacity, turn and flips, sampled
//! bilinearly from the copy nearest its drawn size. A picture still on its way, an SVG, or one
//! that failed or is too big is a quiet tile. Not painted, each counted for the line under the
//! page ([`Omitted`], [`Plan::omitted_now`]): those tiles, blur, the glow a text, line or icon
//! casts, blend modes, motion (shown at rest), turned words (set level) and sizes inside a text.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use relay_client::design::layout::{self, Measure, Placed, STICKY_PAD};
use relay_client::design::{Node, NodeKind, Rect, Scene, bounds, color, icons, shapes, text as dtext};
use serde_json::{Map, Value};
use zeno::{Cap, Command, Fill, Join, Mask, Stroke, Vector};

pub mod pictures;

use pictures::Picture;

// Fonts

/// The app's faces, read once for the worker (the canvas holds its own for the UI thread).
fn faces() -> &'static [fontdue::Font] {
    static FACES: OnceLock<Vec<fontdue::Font>> = OnceLock::new();
    FACES.get_or_init(|| {
        crate::app::FONTS
            .iter()
            .map(|b| fontdue::Font::from_bytes(*b, fontdue::FontSettings { scale: 48.0, ..Default::default() }).expect("the bundled font is valid"))
            .collect()
    })
}

/// The face a design's font and weight are set in, of the ones the app carries (`draw::Weight`'s
/// order): Geist at 400, 500 and 600, Literata at 400 and 600, Geist Mono.
pub fn face(font: Option<&str>, weight: u16) -> usize {
    let f = font.unwrap_or("").to_ascii_lowercase();
    let mono = ["mono", "code", "courier", "consol", "menlo"].iter().any(|k| f.contains(k));
    let serif = (f.contains("serif") && !f.contains("sans"))
        || ["literata", "georgia", "times", "garamond", "merriweather", "lora", "playfair", "baskerville", "fraunces", "newsreader", "crimson"].iter().any(|k| f.contains(k));
    if mono {
        5
    } else if serif {
        if weight >= 550 { 6 } else { 4 }
    } else if weight >= 550 {
        2
    } else if weight >= 450 {
        1
    } else {
        0
    }
}

/// How a node's words are set.
#[derive(Clone, Copy, Debug)]
pub struct Set {
    pub face: usize,
    pub size: f64,
    /// Line height in px.
    pub line: f64,
    pub spacing: f64,
}

impl Set {
    pub fn of(n: &Node) -> Set {
        let size = n.font_size.unwrap_or(16.0).clamp(1.0, 1000.0);
        Set { face: face(n.font.as_deref(), n.font_weight.unwrap_or(400)), size, line: n.line_height.unwrap_or(1.3).max(0.1) * size, spacing: n.letter_spacing.unwrap_or(0.0) }
    }
}

/// Width of `s` set in `set`, in px: advances and kerning, letter spacing between letters. A
/// character the faces lack is left out, as the canvas leaves it out.
pub fn width(s: &str, set: Set) -> f64 {
    let f = &faces()[set.face];
    let px = set.size as f32;
    let (mut w, mut prev, mut count) = (0.0f64, None::<u16>, 0usize);
    for c in s.chars() {
        let i = f.lookup_glyph_index(c);
        if i == 0 {
            continue;
        }
        if let Some(p) = prev {
            w += f64::from(f.horizontal_kern_indexed(p, i, px).unwrap_or(0.0));
        }
        w += f64::from(f.metrics_indexed(i, px).advance_width) + set.spacing;
        prev = Some(i);
        count += 1;
    }
    if count > 0 { w - set.spacing } else { 0.0 }
}

/// One line of a node's words: a byte range of its drawn text, its width, and whether it was
/// cut (it ends in "…").
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub w: f64,
    pub cut: bool,
}

/// `text` broken into lines as Pango's word-and-character wrapping breaks it: at spaces, a word
/// wider than the line cut between letters; at every line break; at most `max` lines when it
/// wraps, the last cut with "…".
pub fn lines(text: &str, set: Set, wrap: Option<f64>, max: Option<u16>) -> Vec<Line> {
    let mut out = Vec::new();
    let mut from = 0;
    for hard in text.split('\n') {
        let to = from + hard.len();
        breaks(text, from, to, set, wrap, &mut out);
        from = to + 1;
    }
    if let (Some(most), Some(room)) = (max, wrap) {
        let most = usize::from(most.max(1));
        if out.len() > most {
            out.truncate(most);
            let last = out.last_mut().expect("at least one line");
            last.cut = true;
            // Take back letters until the line and its "…" fit.
            let end_room = room - width("…", set);
            let mut end = text[..last.end].trim_end().len();
            while end > last.start && width(text[last.start..end].trim_end(), set) > end_room {
                end = text[..end].char_indices().next_back().map_or(last.start, |(i, _)| i);
            }
            last.end = end;
        }
    }
    for l in &mut out {
        let words = text[l.start..l.end].trim_end();
        l.w = if l.cut { width(&format!("{words}…"), set) } else { width(words, set) };
    }
    out
}

fn breaks(text: &str, from: usize, to: usize, set: Set, wrap: Option<f64>, out: &mut Vec<Line>) {
    let line = |start: usize, end: usize| Line { start, end, w: 0.0, cut: false };
    let Some(room) = wrap else {
        out.push(line(from, to));
        return;
    };
    let room = room + 0.01;
    let (mut start, mut end, mut p) = (from, from, from);
    loop {
        let word_end = text[p..to].find(' ').map_or(to, |k| p + k);
        if end == start {
            if width(&text[start..word_end], set) > room {
                // A word longer than the line: as many of its letters as fit, at least one.
                let cut = longest(text, start, word_end, set, room);
                out.push(line(start, cut));
                (start, end, p) = (cut, cut, cut);
                continue;
            }
            end = word_end;
        } else if width(text[start..word_end].trim_end(), set) <= room {
            end = word_end;
        } else {
            out.push(line(start, end));
            (start, end) = (p, p);
            continue;
        }
        if word_end >= to {
            break;
        }
        p = word_end + 1;
    }
    out.push(line(start, end));
}

/// The end of the longest run of `text[start..end]` that fits `room`, at least one character.
fn longest(text: &str, start: usize, end: usize, set: Set, room: f64) -> usize {
    let ends: Vec<usize> = text[start..end].char_indices().map(|(i, c)| start + i + c.len_utf8()).collect();
    let (mut lo, mut hi) = (0usize, ends.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&text[start..ends[mid]], set) <= room { lo = mid } else { hi = mid - 1 }
    }
    ends.first().map_or(end, |_| ends[lo])
}

/// Measures words for layout as the PC's painter does: the widest line plus a pixel, lines at
/// the node's line height; nothing is 2 px wide. Asked many times a pass, so each answer is kept.
#[derive(Default)]
pub struct Setter {
    sizes: RefCell<HashMap<(String, u64), (f64, f64)>>,
}

impl Measure for Setter {
    fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
        let key = (n.id.clone(), wrap.map_or(u64::MAX, f64::to_bits));
        if let Some(s) = self.sizes.borrow().get(&key) {
            return *s;
        }
        let set = Set::of(n);
        let (text, _) = dtext::shown(n);
        let size = if text.is_empty() {
            (2.0, set.line)
        } else {
            let ls = lines(&text, set, wrap.map(|w| w + set.spacing.max(0.0)), n.max_lines.filter(|_| wrap.is_some()));
            let w = ls.iter().map(|l| l.w).fold(0.0, f64::max).ceil() + 1.0;
            (w, ls.len().max(1) as f64 * set.line)
        };
        self.sizes.borrow_mut().insert(key, size);
        size
    }
}

// The plan

/// A run of a text as a byte range of its drawn text, with its colour and face where it sets one.
pub type Run = (usize, usize, Option<[u8; 4]>, Option<usize>);

/// A node's words set to draw: where, how wide a box they align in, and their lines.
#[derive(Clone, Debug)]
pub struct Words {
    pub x: f64,
    pub y: f64,
    pub box_w: f64,
    pub text: String,
    pub lines: Vec<Line>,
    pub set: Set,
    pub color: [u8; 4],
    /// 0 left, 1 centre, 2 right.
    pub align: u8,
    pub underline: bool,
    pub strike: bool,
    /// Runs as byte ranges of `text`: their colour and face where they set one.
    pub runs: Vec<Run>,
}

/// A top-level frame of a page: an artboard.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub rect: Rect,
}

/// What the phone does not draw as the PC does, counted over a page's shown nodes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Omitted {
    /// Pictures shown as tiles: an SVG, failed, too big, or not fetched.
    pub pictures: usize,
    /// Pictures on their way (tiles until they come).
    pub loading: usize,
    pub blur: usize,
    pub glow: usize,
    pub blend: usize,
    pub moving: usize,
    pub turned_words: usize,
    pub sizes: usize,
    pub unread: usize,
}

impl Omitted {
    /// The line under a page: what it leaves out, or None when it draws it all.
    pub fn caption(&self) -> Option<String> {
        let n = |k: usize, one: &str, many: &str| match k {
            0 => None,
            1 => Some(format!("1 {one}")),
            k => Some(format!("{k} {many}")),
        };
        let parts: Vec<String> = [
            n(self.pictures, "picture (shown as a tile)", "pictures (shown as tiles)"),
            n(self.loading, "picture (still loading)", "pictures (still loading)"),
            n(self.blur, "blur", "blurs"),
            n(self.glow, "glow", "glows"),
            n(self.blend, "blend mode", "blend modes"),
            n(self.moving, "moving layer (at rest)", "moving layers (at rest)"),
            n(self.turned_words, "turned text (set level)", "turned texts (set level)"),
            n(self.sizes, "text with mixed sizes", "texts with mixed sizes"),
            n(self.unread, "layer this app can't read", "layers this app can't read"),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| format!("Not drawn here: {}.", parts.join(", ")))
    }
}

/// One page of a design, read, laid out and its words set: what every picture of it is painted from.
pub struct Plan {
    /// Unique per plan: pictures painted from it are named by it.
    pub serial: u64,
    pub design: i64,
    pub rev: i64,
    pub page: String,
    pub page_name: String,
    scene: Scene,
    placed: Placed,
    /// The page's canvas colour.
    pub ground: [u8; 4],
    pub boards: Vec<Board>,
    /// Everything on the page as painted; None for an empty page.
    pub bounds: Option<Rect>,
    words: HashMap<String, Words>,
    /// What it leaves out besides pictures ([`Self::omitted_now`] counts those).
    pub omitted: Omitted,
    /// Each distinct picture its shown image nodes name, and the size it is drawn at most.
    pub pictures: Vec<Want>,
    /// Each shown image node's picture (None when it names none).
    images: Vec<Option<String>>,
}

/// A picture a page needs: its `src`, and the size (px) it should cover to look sharp on the
/// largest screen at 2× zoom.
#[derive(Clone, Debug, PartialEq)]
pub struct Want {
    pub src: String,
    pub w: u32,
    pub h: u32,
}

/// Picture pixels per page px for a node in an artboard `root_w` wide: the artboard fitted to a
/// 1440 px screen and zoomed twice, at most 4 and at least 1.
fn picture_density(root_w: f64) -> f64 {
    (2880.0 / root_w.max(1.0)).clamp(1.0, 4.0)
}

impl Plan {
    /// [`Self::omitted`] with the pictures still not drawn as they stand now: tiles (an SVG,
    /// failed, too big, not fetched) and those on their way.
    pub fn omitted_now(&self) -> Omitted {
        let mut o = self.omitted;
        for src in &self.images {
            match src.as_deref().map(pictures::status) {
                Some(pictures::Status::Ready) => {}
                Some(pictures::Status::Loading) => o.loading += 1,
                _ => o.pictures += 1,
            }
        }
        o
    }

    /// Whether none of its pictures is on its way.
    pub fn pictures_settled(&self) -> bool {
        self.pictures.iter().all(|w| pictures::status(&w.src) != pictures::Status::Loading)
    }

    /// Which of its pictures are here, as one number ([`pictures::mark`]).
    pub fn pictures_mark(&self) -> u64 {
        pictures::mark(self.pictures.iter().map(|w| w.src.as_str()))
    }
}

static SERIAL: AtomicU64 = AtomicU64::new(1);

/// A `design.get {design_id, page}` answer read and laid out. Slow for a big page: call it off
/// the UI thread.
pub fn plan(design: i64, mut v: Value) -> Result<Plan, String> {
    let Value::Array(list) = v.get_mut("nodes").map(Value::take).unwrap_or_default() else {
        return Err("The PC sent no layers for this page".into());
    };
    let mut nodes = Vec::with_capacity(list.len());
    let mut unread = 0;
    for x in list {
        match Node::read(x) {
            Ok(n) => nodes.push(n),
            Err(_) => unread += 1,
        }
    }
    let (page, page_name) = nodes.iter().find(|n| n.kind == NodeKind::Page).map(|n| (n.id.clone(), n.name.clone().unwrap_or_default())).ok_or("This page is gone")?;
    let mut looks = Map::new();
    for key in ["colors", "styles"] {
        if let Some(x) = v.get(key).filter(|x| x.is_object()) {
            looks.insert(key.into(), x.clone());
        }
    }
    let mut scene = Scene::new(nodes);
    if let Ok(styles) = serde_json::from_value(Value::Object(looks)) {
        scene.set_styles(styles);
    }
    let setter = Setter::default();
    let placed = layout::place(&scene, &page, &setter);
    let mut words = HashMap::new();
    let mut omitted = Omitted { unread, ..Omitted::default() };
    let (mut images, mut wants) = (Vec::new(), Vec::<Want>::new());
    // Every shown node of the page, with whether it sits in a turn and its artboard's width.
    let mut todo: Vec<(String, bool, f64)> = scene.children(&page).iter().map(|c| (c.clone(), false, placed.rect(c).map_or(1.0, |r| r.w))).collect();
    while let Some((id, turned, root_w)) = todo.pop() {
        let Some(n) = scene.node(&id) else { continue };
        if n.is_hidden() || n.opacity.is_some_and(|o| o <= 0.0) {
            continue;
        }
        let turned = turned || shapes::rotation(n) != 0.0;
        count(n, turned, &mut omitted);
        if n.kind == NodeKind::Image {
            let src = n.src.clone().filter(|s| !s.is_empty());
            if let (Some(s), Some(r)) = (&src, placed.rect(&id)) {
                let k = picture_density(root_w);
                let (w, h) = ((r.w * k).ceil().clamp(1.0, 8192.0) as u32, (r.h * k).ceil().clamp(1.0, 8192.0) as u32);
                match wants.iter_mut().find(|x| x.src == *s) {
                    Some(x) => (x.w, x.h) = (x.w.max(w), x.h.max(h)),
                    None => wants.push(Want { src: s.clone(), w, h }),
                }
            }
            images.push(src);
        }
        if let Some(r) = placed.rect(&id)
            && let Some(w) = set_words(&scene, &placed, &setter, n, &r)
        {
            words.insert(id.clone(), w);
        }
        todo.extend(scene.children(&id).iter().map(|c| (c.clone(), turned, root_w)));
    }
    let boards = scene
        .children(&page)
        .iter()
        .filter_map(|id| {
            let n = scene.node(id).filter(|n| n.kind == NodeKind::Frame && !n.is_hidden())?;
            let rect = placed.rect(id).filter(|r| r.w > 0.0 && r.h > 0.0)?;
            Some(Board { id: id.clone(), name: n.name.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Frame".into()), rect })
        })
        .collect();
    let bounds = bounds::painted_bounds(&scene, &placed, &setter, &page).map(|b| if b.w > 0.0 && b.h > 0.0 { b } else { b.inflate(1.0) });
    let ground = scene.node(&page).and_then(color::ground_of).and_then(|c| rgba(&c)).filter(|c| c[3] > 0).unwrap_or([0x1E, 0x1D, 0x1B, 255]);
    let rev = v.pointer("/design/rev").and_then(Value::as_i64).unwrap_or(0);
    Ok(Plan {
        serial: SERIAL.fetch_add(1, Ordering::Relaxed),
        design,
        rev,
        page,
        page_name,
        scene,
        placed,
        ground: [ground[0], ground[1], ground[2], 255],
        boards,
        bounds,
        words,
        omitted,
        pictures: wants,
        images,
    })
}

fn count(n: &Node, turned: bool, o: &mut Omitted) {
    if n.blur.is_some_and(|b| b > 0.0) || n.backdrop_blur.is_some_and(|b| b > 0.0) {
        o.blur += 1;
    }
    if matches!(n.kind, NodeKind::Text | NodeKind::Line | NodeKind::Path | NodeKind::Icon) && !shapes::effective_shadows(n).is_empty() {
        o.glow += 1;
    }
    if n.blend.as_ref().is_some_and(|b| format!("{b:?}") != "Normal") {
        o.blend += 1;
    }
    if n.motion.is_some() {
        o.moving += 1;
    }
    let has_words = n.text.as_deref().is_some_and(|t| !t.is_empty()) && matches!(n.kind, NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon);
    if has_words && turned {
        o.turned_words += 1;
    }
    if has_words && n.runs.iter().flatten().any(|r| r.font_size.is_some()) {
        o.sizes += 1;
    }
}

/// Where and how a text's, sticky's or shape label's words are drawn, at the widths layout gave.
fn set_words(scene: &Scene, placed: &Placed, measure: &Setter, n: &Node, r: &Rect) -> Option<Words> {
    if n.text.as_deref().is_none_or(str::is_empty) {
        return None;
    }
    let label = matches!(n.kind, NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon);
    let (x, y, wrap) = match n.kind {
        NodeKind::Text => (r.x, r.y, placed.wrap_width(n, r)),
        NodeKind::Sticky => (r.x + STICKY_PAD, r.y + STICKY_PAD, placed.wrap_width(n, r)),
        _ if label => {
            let b = shapes::label_box(n, r, measure)?;
            (b.x, b.y, Some(b.w))
        }
        _ => return None,
    };
    let set = Set::of(n);
    let (text, offsets) = dtext::shown(n);
    let ls = lines(&text, set, wrap.map(|w| w + set.spacing.max(0.0)), n.max_lines.filter(|_| wrap.is_some()));
    let widest = ls.iter().map(|l| l.w).fold(0.0, f64::max);
    let align = match n.align.as_ref().map(|a| format!("{a:?}")).as_deref() {
        Some("Center") => 1,
        Some("Right") => 2,
        Some(_) => 0,
        None if label => 1,
        None => 0,
    };
    let base = n.font_weight.unwrap_or(400);
    let runs = dtext::run_bytes(n, &offsets)
        .into_iter()
        .map(|(s, e, run)| {
            let restyled = run.font.is_some() || run.font_weight.is_some();
            let f = restyled.then(|| face(run.font.as_deref().or(n.font.as_deref()), run.font_weight.unwrap_or(base)));
            (s as usize, e as usize, run.color.as_deref().and_then(rgba), f)
        })
        .collect();
    Some(Words {
        x,
        y,
        box_w: wrap.unwrap_or(widest),
        text,
        lines: ls,
        set,
        color: rgba(&color::text_color(scene, n)).unwrap_or([0, 0, 0, 255]),
        align,
        underline: n.underline == Some(true),
        strike: n.strike == Some(true),
        runs,
    })
}

/// `#RGB`, `#RRGGBB` or `#RRGGBBAA`.
pub fn rgba(text: &str) -> Option<[u8; 4]> {
    let hex = text.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let nib = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok().map(|n| n * 17);
    match hex.len() {
        3 => Some([nib(0)?, nib(1)?, nib(2)?, 255]),
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

// Geometry helpers (pure, tested)

/// The scale that fits `w`×`h` inside `box_w`×`box_h`, at most `most`.
pub fn fit(w: f64, h: f64, box_w: f64, box_h: f64, most: f64) -> f64 {
    if w <= 0.0 || h <= 0.0 {
        return 1.0;
    }
    (box_w / w).min(box_h / h).clamp(1e-4, most)
}

/// How a picture fills its image node's box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FitMode {
    Cover,
    Contain,
    Stretch,
}

/// An image node's `fit`: cover when it sets none, as the PC draws it.
pub fn fit_mode(n: &Node) -> FitMode {
    match n.fit.as_ref().map(|f| format!("{f:?}")).as_deref() {
        Some("Contain") => FitMode::Contain,
        Some("Stretch") => FitMode::Stretch,
        _ => FitMode::Cover,
    }
}

/// Where a `pw`×`ph` picture is drawn for the box `r`, as the PC's painter places it: cover
/// fills the box, centred, what runs over cut by the box; contain shows it whole, centred;
/// stretch is the box.
pub fn fit_box(fit: FitMode, pw: f64, ph: f64, r: &Rect) -> Rect {
    if fit == FitMode::Stretch || pw <= 0.0 || ph <= 0.0 {
        return *r;
    }
    let (sx, sy) = (r.w / pw, r.h / ph);
    let s = if fit == FitMode::Cover { sx.max(sy) } else { sx.min(sy) };
    let (w, h) = (pw * s, ph * s);
    Rect::new(r.cx() - w / 2.0, r.cy() - h / 2.0, w, h)
}

/// Whether an artboard takes the full width: landscape, or wider than a tablet.
pub fn is_wide(w: f64, h: f64) -> bool {
    w >= h * 1.2 || w > 1000.0
}

/// Where each artboard's picture goes in a column `width` wide, from `y` 0: phone-shaped ones
/// two across in order, wide ones the full width, each keeping its shape (a very tall one is
/// fitted into at most `tallest` column widths). `label` is the room under each for its name.
/// Returns each picture's box (x, y, w, h) and the total height.
pub fn arrange(sizes: &[(f64, f64)], width: f32, gap: f32, label: f32, tallest: f32) -> (Vec<[f32; 4]>, f32) {
    let col = (width - gap) / 2.0;
    let mut out = Vec::with_capacity(sizes.len());
    let (mut y, mut row_h, mut left_taken) = (0.0f32, 0.0f32, false);
    for &(w, h) in sizes {
        let (w, h) = (w.max(1.0) as f32, h.max(1.0) as f32);
        if is_wide(w.into(), h.into()) {
            if left_taken {
                y += row_h + label + gap;
                (row_h, left_taken) = (0.0, false);
            }
            let bh = (width * h / w).min(width * 1.4);
            let bw = (bh * w / h).min(width);
            out.push([(width - bw) / 2.0, y, bw, bh]);
            y += bh + label + gap;
            continue;
        }
        let bh = (col * h / w).min(col * tallest);
        let bw = (bh * w / h).min(col);
        let x = if left_taken { col + gap } else { 0.0 } + (col - bw) / 2.0;
        out.push([x, y, bw, bh]);
        row_h = row_h.max(bh);
        if left_taken {
            y += row_h + label + gap;
            (row_h, left_taken) = (0.0, false);
        } else {
            left_taken = true;
        }
    }
    if left_taken {
        y += row_h + label + gap;
    }
    (out, (y - gap).max(0.0))
}

/// A thumbnail's filmstrip: the first artboards side by side at one scale, as tall as the strip
/// less `pad` above and below (narrowed so the first fits across), centred when they are fewer
/// than the strip is wide; the last one shown may run off its end. The scale and each shown
/// artboard's top-left.
pub fn strip_layout(sizes: &[(f64, f64)], w: f64, h: f64, pad: f64, gap: f64) -> (f64, Vec<(f64, f64)>) {
    if sizes.is_empty() {
        return (1.0, Vec::new());
    }
    let tallest = sizes.iter().take(4).map(|s| s.1).fold(1.0, f64::max);
    let mut scale = (h - 2.0 * pad).max(1.0) / tallest;
    scale = scale.min((w - 2.0 * pad).max(1.0) / sizes[0].0.max(1.0));
    let mut out = Vec::new();
    let mut x = pad;
    for &(bw, _) in sizes {
        if x >= w - pad {
            break;
        }
        out.push((x, pad));
        x += bw * scale + gap;
    }
    let used = x - gap - pad;
    if used < w - 2.0 * pad {
        let shift = (w - 2.0 * pad - used) / 2.0;
        for p in &mut out {
            p.0 += shift;
        }
    }
    (scale, out)
}

// Pictures

/// An RGBA picture, row by row, opaque.
pub struct Pix {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Pix {
    pub fn new(w: u32, h: u32, ground: [u8; 4]) -> Pix {
        let n = (w as usize) * (h as usize);
        let mut data = Vec::with_capacity(n * 4);
        for _ in 0..n {
            data.extend_from_slice(&[ground[0], ground[1], ground[2], 255]);
        }
        Pix { w, h, data }
    }
}

/// One artboard, `w`×`h` pixels (its shape).
pub fn board(plan: &Plan, id: &str, w: u32, h: u32) -> Pix {
    let mut pix = Pix::new(w, h, plan.ground);
    if let Some(b) = plan.boards.iter().find(|b| b.id == id) {
        paint(plan, std::slice::from_ref(&b.id), b.rect, &mut pix, [0.0, 0.0, f64::from(w), f64::from(h)]);
    }
    pix
}

/// The whole page fitted and centred in `w`×`h` pixels.
pub fn page(plan: &Plan, w: u32, h: u32) -> Pix {
    let mut pix = Pix::new(w, h, plan.ground);
    if let Some(b) = plan.bounds {
        let b = b.inflate(b.w.max(b.h) * 0.02);
        let s = fit(b.w, b.h, f64::from(w), f64::from(h), 64.0);
        let (rw, rh) = (f64::from(w) / s, f64::from(h) / s);
        let region = Rect::new(b.cx() - rw / 2.0, b.cy() - rh / 2.0, rw, rh);
        let roots = plan.scene.children(&plan.page).to_vec();
        paint(plan, &roots, region, &mut pix, [0.0, 0.0, f64::from(w), f64::from(h)]);
    }
    pix
}

/// Part of the page (`region`, page px) in `w`×`h` pixels: one artboard's (`root`) or everything.
pub fn region(plan: &Plan, root: Option<&str>, region: Rect, w: u32, h: u32) -> Pix {
    let mut pix = Pix::new(w, h, plan.ground);
    let roots = match root {
        Some(r) => vec![r.to_string()],
        None => plan.scene.children(&plan.page).to_vec(),
    };
    paint(plan, &roots, region, &mut pix, [0.0, 0.0, f64::from(w), f64::from(h)]);
    pix
}

/// A thumbnail `w`×`h`: the first artboards in a filmstrip ([`strip_layout`]), or the whole page
/// fitted when it has none.
pub fn strip(plan: &Plan, w: u32, h: u32, pad: f64, gap: f64) -> Pix {
    if plan.boards.is_empty() {
        return page(plan, w, h);
    }
    let mut pix = Pix::new(w, h, plan.ground);
    let sizes: Vec<(f64, f64)> = plan.boards.iter().map(|b| (b.rect.w, b.rect.h)).collect();
    let (s, at) = strip_layout(&sizes, f64::from(w), f64::from(h), pad, gap);
    for (b, (x, y)) in plan.boards.iter().zip(at) {
        paint(plan, std::slice::from_ref(&b.id), b.rect, &mut pix, [x, y, b.rect.w * s, b.rect.h * s]);
    }
    pix
}

/// Paint `roots` (and what they hold) of `region` (page px) into `dst` (x, y, w, h in pixels).
fn paint(plan: &Plan, roots: &[String], region: Rect, pix: &mut Pix, dst: [f64; 4]) {
    if region.w <= 0.0 || region.h <= 0.0 || dst[2] < 1.0 || dst[3] < 1.0 {
        return;
    }
    let s = dst[2] / region.w;
    let af = Af { a: s, b: 0.0, c: 0.0, d: s, e: dst[0] - region.x * s, f: dst[1] - region.y * s };
    let whole = IRect { x: 0, y: 0, w: pix.w as i32, h: pix.h as i32 };
    let r = IRect { x: dst[0].round() as i32, y: dst[1].round() as i32, w: (dst[0] + dst[2]).round() as i32 - dst[0].round() as i32, h: (dst[1] + dst[3]).round() as i32 - dst[1].round() as i32 }.meet(&whole);
    let mut p = Painter { plan, pix, clips: vec![Clip { r, m: None }], glyphs: HashMap::new(), icons: HashMap::new() };
    p.fill_rect(r, &Paint::Solid(plan.ground), 1.0);
    for id in roots {
        p.node(id, af, 1.0, 0);
    }
}

// The painter

/// Page px to picture px: a scale, a turn, a move.
#[derive(Clone, Copy, Debug)]
struct Af {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Af {
    fn apply(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
    /// This after `o`: a point goes through `o` first.
    fn then(&self, o: &Af) -> Af {
        let s = self;
        Af { a: s.a * o.a + s.c * o.b, b: s.b * o.a + s.d * o.b, c: s.a * o.c + s.c * o.d, d: s.b * o.c + s.d * o.d, e: s.a * o.e + s.c * o.f + s.e, f: s.b * o.e + s.d * o.f + s.f }
    }
    /// Turned `deg` clockwise about the page point (`cx`, `cy`) first (`shapes::rotate`).
    fn turned_about(&self, cx: f64, cy: f64, deg: f64) -> Af {
        let (sn, cs) = deg.to_radians().sin_cos();
        let r = Af { a: cs, b: sn, c: -sn, d: cs, e: cx - cx * cs + cy * sn, f: cy - cx * sn - cy * cs };
        self.then(&r)
    }
    fn scale(&self) -> f64 {
        self.a.hypot(self.b)
    }
    fn turned(&self) -> bool {
        self.b.abs() > 1e-9 || self.c.abs() > 1e-9
    }
    fn invert(&self) -> Af {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-12 {
            return *self;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Af { a, b, c, d, e: -(a * self.e + c * self.f), f: -(b * self.e + d * self.f) }
    }
}

/// A pixel rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
struct IRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl IRect {
    fn meet(&self, o: &IRect) -> IRect {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = ((self.x + self.w).min(o.x + o.w), (self.y + self.h).min(o.y + o.h));
        IRect { x: x0, y: y0, w: (x1 - x0).max(0), h: (y1 - y0).max(0) }
    }
    fn empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
    fn grow(&self, by: i32) -> IRect {
        IRect { x: self.x - by, y: self.y - by, w: self.w + 2 * by, h: self.h + 2 * by }
    }
    fn len(&self) -> usize {
        (self.w.max(0) as usize) * (self.h.max(0) as usize)
    }
}

/// A path in picture px, built from page points through an [`Af`], with its bounds.
struct Pen {
    af: Af,
    cmds: Vec<Command>,
    lo: (f64, f64),
    hi: (f64, f64),
}

const KAPPA: f64 = 0.552_284_75;

impl Pen {
    fn new(af: Af) -> Pen {
        Pen { af, cmds: Vec::new(), lo: (f64::MAX, f64::MAX), hi: (f64::MIN, f64::MIN) }
    }
    fn pt(&mut self, p: (f64, f64)) -> Vector {
        let (x, y) = self.af.apply(p);
        self.lo = (self.lo.0.min(x), self.lo.1.min(y));
        self.hi = (self.hi.0.max(x), self.hi.1.max(y));
        Vector::new(x as f32, y as f32)
    }
    fn move_to(&mut self, p: (f64, f64)) {
        let v = self.pt(p);
        self.cmds.push(Command::MoveTo(v));
    }
    fn line_to(&mut self, p: (f64, f64)) {
        let v = self.pt(p);
        self.cmds.push(Command::LineTo(v));
    }
    fn quad_to(&mut self, c: (f64, f64), p: (f64, f64)) {
        let (c, p) = (self.pt(c), self.pt(p));
        self.cmds.push(Command::QuadTo(c, p));
    }
    fn cubic_to(&mut self, c1: (f64, f64), c2: (f64, f64), p: (f64, f64)) {
        let (c1, c2, p) = (self.pt(c1), self.pt(c2), self.pt(p));
        self.cmds.push(Command::CurveTo(c1, c2, p));
    }
    fn close(&mut self) {
        self.cmds.push(Command::Close);
    }
    /// A box with its corners rounded by `radii` (top-left, clockwise).
    fn rrect(&mut self, r: &Rect, [tl, tr, br, bl]: [f64; 4]) {
        let (x, y, rt, bt) = (r.x, r.y, r.right(), r.bottom());
        let k = 1.0 - KAPPA;
        self.move_to((x + tl, y));
        self.line_to((rt - tr, y));
        if tr > 0.0 {
            self.cubic_to((rt - tr * k, y), (rt, y + tr * k), (rt, y + tr));
        }
        self.line_to((rt, bt - br));
        if br > 0.0 {
            self.cubic_to((rt, bt - br * k), (rt - br * k, bt), (rt - br, bt));
        }
        self.line_to((x + bl, bt));
        if bl > 0.0 {
            self.cubic_to((x + bl * k, bt), (x, bt - bl * k), (x, bt - bl));
        }
        self.line_to((x, y + tl));
        if tl > 0.0 {
            self.cubic_to((x, y + tl * k), (x + tl * k, y), (x + tl, y));
        }
        self.close();
    }
    fn ellipse(&mut self, r: &Rect) {
        let (cx, cy, rx, ry) = (r.cx(), r.cy(), r.w / 2.0, r.h / 2.0);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        self.move_to((cx, cy - ry));
        self.cubic_to((cx + kx, cy - ry), (cx + rx, cy - ky), (cx + rx, cy));
        self.cubic_to((cx + rx, cy + ky), (cx + kx, cy + ry), (cx, cy + ry));
        self.cubic_to((cx - kx, cy + ry), (cx - rx, cy + ky), (cx - rx, cy));
        self.cubic_to((cx - rx, cy - ky), (cx - kx, cy - ry), (cx, cy - ry));
        self.close();
    }
    fn poly(&mut self, pts: &[(f64, f64)], closed: bool) {
        for (i, p) in pts.iter().enumerate() {
            if i == 0 { self.move_to(*p) } else { self.line_to(*p) }
        }
        if closed {
            self.close();
        }
    }
    fn bbox(&self, grow: f64) -> IRect {
        if self.cmds.is_empty() || self.lo.0 > self.hi.0 {
            return IRect { x: 0, y: 0, w: 0, h: 0 };
        }
        let clampi = |v: f64| v.clamp(-1e7, 1e7) as i32;
        let (x0, y0) = (clampi((self.lo.0 - grow).floor()), clampi((self.lo.1 - grow).floor()));
        let (x1, y1) = (clampi((self.hi.0 + grow).ceil()), clampi((self.hi.1 + grow).ceil()));
        IRect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
}

/// The paint a fill or stroke is laid in.
enum Paint {
    Solid([u8; 4]),
    Grad(Box<Grad>),
    Pic(Box<PicPaint>),
}

/// A picture laid over the box `t` (page px).
struct PicPaint {
    pic: std::sync::Arc<Picture>,
    level: usize,
    /// Picture px back to page px.
    inv: Af,
    t: Rect,
}

impl PicPaint {
    /// Its colour at picture pixel (`x`, `y`), not premultiplied, as `fill_cov` lays colours.
    fn at(&self, x: f64, y: f64) -> [f32; 4] {
        let (px, py) = self.inv.apply((x, y));
        let c = self.pic.sample(self.level, (px - self.t.x) / self.t.w.max(1e-9), (py - self.t.y) / self.t.h.max(1e-9));
        if c[3] <= 0.0 {
            return [0.0; 4];
        }
        [(c[0] / c[3]).min(1.0), (c[1] / c[3]).min(1.0), (c[2] / c[3]).min(1.0), c[3]]
    }
}

struct Grad {
    /// 0 linear, 1 radial, 2 conic.
    kind: u8,
    /// Picture px back to page px.
    inv: Af,
    p0: (f64, f64),
    v: (f64, f64),
    c: (f64, f64),
    rx: f64,
    ry: f64,
    start: f64,
    stops: Vec<(f64, [f32; 4])>,
}

impl Grad {
    fn at(&self, x: f64, y: f64) -> [f32; 4] {
        let (px, py) = self.inv.apply((x, y));
        let t = match self.kind {
            0 => {
                let len2 = (self.v.0 * self.v.0 + self.v.1 * self.v.1).max(1e-9);
                ((px - self.p0.0) * self.v.0 + (py - self.p0.1) * self.v.1) / len2
            }
            1 => ((px - self.c.0) / self.rx.max(1e-9)).hypot((py - self.c.1) / self.ry.max(1e-9)),
            _ => ((px - self.c.0).atan2(-(py - self.c.1)).to_degrees() - self.start).rem_euclid(360.0) / 360.0,
        };
        let s = &self.stops;
        if t <= s[0].0 {
            return s[0].1;
        }
        for w in s.windows(2) {
            if t <= w[1].0 {
                let k = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-9)) as f32;
                return std::array::from_fn(|i| w[0].1[i] + (w[1].1[i] - w[0].1[i]) * k);
            }
        }
        s[s.len() - 1].1
    }
}

impl Paint {
    fn at(&self, x: f64, y: f64) -> [f32; 4] {
        match self {
            Paint::Solid(c) => [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, f32::from(c[3]) / 255.0],
            Paint::Grad(g) => g.at(x, y),
            Paint::Pic(p) => p.at(x, y),
        }
    }
}

/// Where drawing may reach: a pixel box, and within it a coverage when its edge is not that box.
struct Clip {
    r: IRect,
    m: Option<Vec<u8>>,
}

/// A piece of an icon's path, on its 24-unit grid.
#[derive(Clone, Copy)]
enum ISeg {
    M(f64, f64),
    L(f64, f64),
    C(f64, f64, f64, f64, f64, f64),
    Q(f64, f64, f64, f64),
    Z,
}

fn icon_segs(d: &str) -> Vec<ISeg> {
    use svgtypes::SimplePathSegment as S;
    svgtypes::SimplifyingPathParser::from(d)
        .filter_map(Result::ok)
        .map(|s| match s {
            S::MoveTo { x, y } => ISeg::M(x, y),
            S::LineTo { x, y } => ISeg::L(x, y),
            S::CurveTo { x1, y1, x2, y2, x, y } => ISeg::C(x1, y1, x2, y2, x, y),
            S::Quadratic { x1, y1, x, y } => ISeg::Q(x1, y1, x, y),
            S::ClosePath => ISeg::Z,
        })
        .collect()
}

type GlyphKey = (usize, u16, u32);

struct Painter<'a> {
    plan: &'a Plan,
    pix: &'a mut Pix,
    clips: Vec<Clip>,
    glyphs: HashMap<GlyphKey, (fontdue::Metrics, Vec<u8>)>,
    icons: HashMap<String, Vec<ISeg>>,
}

fn render_mask(cmds: &[Command], style: zeno::Style, r: IRect) -> Vec<u8> {
    let mut buf = vec![0u8; r.len()];
    if r.empty() || cmds.is_empty() {
        return buf;
    }
    Mask::new(cmds).style(style).size(r.w as u32, r.h as u32).offset(Vector::new(-r.x as f32, -r.y as f32)).render_into(&mut buf, None);
    buf
}

/// Three box blurs, about a Gaussian of `sigma` px.
fn blur(m: &mut [u8], w: usize, h: usize, sigma: f64) {
    if sigma < 0.4 || w == 0 || h == 0 {
        return;
    }
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let mi = ((12.0 * sigma * sigma - n * (wl * wl) as f64 - 4.0 * n * wl as f64 - 3.0 * n) / (-4.0 * wl as f64 - 4.0)).round() as i64;
    let mut tmp = vec![0u8; m.len()];
    for i in 0..3 {
        let size = if i < mi { wl } else { wu };
        let r = ((size - 1) / 2).max(0) as usize;
        if r == 0 {
            continue;
        }
        box_pass(m, &mut tmp, w, h, r, true);
        box_pass(&tmp, m, w, h, r, false);
    }
}

fn box_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, across: bool) {
    let (lines, len, step, stride) = if across { (h, w, 1, w) } else { (w, h, w, 1) };
    let div = (2 * r + 1) as u32;
    for l in 0..lines {
        let base = l * stride;
        let at = |i: usize| u32::from(src[base + i * step]);
        let mut sum: u32 = (0..=r.min(len - 1)).map(at).sum();
        for i in 0..len {
            dst[base + i * step] = ((sum + div / 2) / div) as u8;
            if i + r + 1 < len {
                sum += at(i + r + 1);
            }
            if i >= r {
                sum -= at(i - r);
            }
        }
    }
}

fn parse_paint(n: &Node, r: &Rect, af: Af, stroke: bool) -> Option<Paint> {
    let g = if stroke { n.stroke_gradient.as_ref() } else { n.gradient.as_ref() };
    if let Some(g) = g {
        let mut stops: Vec<(f64, [f32; 4])> = g.stops.iter().filter_map(|s| Some((s.at.clamp(0.0, 1.0), rgba(&s.color)?.map(|c| f32::from(c) / 255.0)))).collect();
        if !stops.is_empty() {
            stops.sort_by(|a, b| a.0.total_cmp(&b.0));
            let kind = match format!("{:?}", g.kind).as_str() {
                "Radial" => 1,
                "Conic" => 2,
                _ => 0,
            };
            let [cxs, cys] = g.center.unwrap_or([0.5, 0.5]);
            let c = (r.x + cxs * r.w, r.y + cys * r.h);
            let k = g.radius.unwrap_or(1.0);
            let deg = g.angle.unwrap_or(if kind == 0 { 180.0 } else { 0.0 });
            let (sn, cs) = deg.to_radians().sin_cos();
            let len = (r.w * sn).abs() + (r.h * cs).abs();
            let (dx, dy) = (sn * len, -cs * len);
            return Some(Paint::Grad(Box::new(Grad {
                kind,
                inv: af.invert(),
                p0: (r.cx() - dx / 2.0, r.cy() - dy / 2.0),
                v: (dx, dy),
                c,
                rx: r.w / 2.0 * k,
                ry: r.h / 2.0 * k,
                start: deg,
                stops,
            })));
        }
    }
    let c = if stroke { n.stroke.clone() } else { color::fill_of(n) };
    c.as_deref().and_then(rgba).filter(|c| c[3] > 0).map(Paint::Solid)
}

impl Painter<'_> {
    fn clip(&self) -> &Clip {
        self.clips.last().expect("the picture's own clip")
    }

    /// Lay `paint` through coverage `cov` (over `r`), times `alpha`, inside the clip.
    fn fill_cov(&mut self, r: IRect, cov: &[u8], paint: &Paint, alpha: f32) {
        if alpha <= 0.0 || cov.len() < r.len() {
            return;
        }
        let clip = self.clips.last().expect("the picture's own clip");
        let area = r.meet(&clip.r);
        if area.empty() {
            return;
        }
        let pw = self.pix.w as usize;
        let solid = match paint {
            Paint::Solid(c) => (c[3] > 0).then(|| paint.at(0.0, 0.0)),
            Paint::Grad(_) | Paint::Pic(_) => None,
        };
        if matches!(paint, Paint::Solid(_)) && solid.is_none() {
            return;
        }
        for y in area.y..area.y + area.h {
            let crow = ((y - r.y) * r.w) as usize;
            let mrow = ((y - clip.r.y) * clip.r.w) as usize;
            for x in area.x..area.x + area.w {
                let mut c = f32::from(cov[crow + (x - r.x) as usize]);
                if c == 0.0 {
                    continue;
                }
                if let Some(m) = &clip.m {
                    c *= f32::from(m[mrow + (x - clip.r.x) as usize]) / 255.0;
                    if c == 0.0 {
                        continue;
                    }
                }
                let col = solid.unwrap_or_else(|| paint.at(f64::from(x) + 0.5, f64::from(y) + 0.5));
                let a = c / 255.0 * alpha * col[3];
                if a <= 0.0 {
                    continue;
                }
                let i = (y as usize * pw + x as usize) * 4;
                for (d, c) in self.pix.data[i..i + 3].iter_mut().zip(col) {
                    let v = f32::from(*d);
                    *d = (v + (c * 255.0 - v) * a).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    fn fill_rect(&mut self, r: IRect, paint: &Paint, alpha: f32) {
        let cov = vec![255u8; r.len()];
        self.fill_cov(r, &cov, paint, alpha);
    }

    fn fill_pen(&mut self, pen: &Pen, rule: Fill, paint: &Paint, alpha: f32) {
        let r = pen.bbox(1.0).meet(&self.clip().r);
        if r.empty() {
            return;
        }
        let m = render_mask(&pen.cmds, rule.into(), r);
        self.fill_cov(r, &m, paint, alpha);
    }

    fn push_clip(&mut self, pen: &Pen, plain: bool) {
        let top = self.clip();
        let top_r = top.r;
        if plain && top.m.is_none() {
            let r = IRect { x: pen.lo.0.round() as i32, y: pen.lo.1.round() as i32, w: (pen.hi.0.round() - pen.lo.0.round()) as i32, h: (pen.hi.1.round() - pen.lo.1.round()) as i32 };
            self.clips.push(Clip { r: r.meet(&top_r), m: None });
            return;
        }
        let r = pen.bbox(1.0).meet(&top_r);
        let mut m = render_mask(&pen.cmds, Fill::NonZero.into(), r);
        if let Some(tm) = &top.m {
            for y in 0..r.h {
                for x in 0..r.w {
                    let t = tm[((r.y + y - top_r.y) * top_r.w + (r.x + x - top_r.x)) as usize];
                    let i = (y * r.w + x) as usize;
                    m[i] = ((u32::from(m[i]) * u32::from(t) + 127) / 255) as u8;
                }
            }
        }
        self.clips.push(Clip { r, m: Some(m) });
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    /// A node and what it holds.
    fn node(&mut self, id: &str, af: Af, alpha: f32, depth: u32) {
        let plan = self.plan;
        if depth > 120 {
            return;
        }
        let Some(n) = plan.scene.node(id) else { return };
        let Some(r) = plan.placed.rect(id) else { return };
        let op = n.opacity.unwrap_or(1.0);
        if n.is_hidden() || op <= 0.0 {
            return;
        }
        let alpha = alpha * op.min(1.0) as f32;
        let turn = shapes::rotation(n);
        let af = if turn != 0.0 { af.turned_about(r.cx(), r.cy(), turn) } else { af };
        if self.culled(n, &r, af) {
            return;
        }
        match n.kind {
            NodeKind::Group => {
                for k in plan.scene.children(id) {
                    self.node(k, af, alpha, depth + 1);
                }
            }
            NodeKind::Frame => {
                let pen = self.outline(n, &r, af, 0.0, 0.0, 0.0);
                self.shadows(n, &r, af, alpha, &pen, false);
                let clips = n.clip != Some(false);
                if clips {
                    let plain = !af.turned() && shapes::corner_radii(n, &r) == [0.0; 4];
                    self.push_clip(&pen, plain);
                }
                if let Some(p) = parse_paint(n, &r, af, false) {
                    self.fill_pen(&pen, Fill::NonZero, &p, alpha);
                }
                self.shadows(n, &r, af, alpha, &pen, true);
                for k in plan.scene.children(id) {
                    self.node(k, af, alpha, depth + 1);
                }
                if clips {
                    self.pop_clip();
                }
                self.border(n, &r, af, alpha);
            }
            NodeKind::Ellipse if shapes::arc(n).is_some() => {
                let pen = self.arc_pen(n, &r, af);
                self.shape(n, &r, af, alpha, &pen, Fill::EvenOdd);
                if let Some((cap, ends)) = shapes::arc_ends(n, &r)
                    && format!("{cap:?}") == "Round"
                    && let Some(p) = parse_paint(n, &r, af, false)
                {
                    for e in ends {
                        let (m, cr) = (e.mid(), e.cap_radius());
                        let mut cp = Pen::new(af);
                        cp.ellipse(&Rect::new(m.0 - cr, m.1 - cr, 2.0 * cr, 2.0 * cr));
                        self.fill_pen(&cp, Fill::NonZero, &p, alpha);
                    }
                }
                self.words(id, af, alpha);
            }
            NodeKind::Rect | NodeKind::Sticky | NodeKind::Ellipse => {
                let pen = self.outline(n, &r, af, 0.0, 0.0, 0.0);
                self.shadows(n, &r, af, alpha, &pen, false);
                if let Some(p) = parse_paint(n, &r, af, false) {
                    self.fill_pen(&pen, Fill::NonZero, &p, alpha);
                }
                self.shadows(n, &r, af, alpha, &pen, true);
                self.border(n, &r, af, alpha);
                self.words(id, af, alpha);
            }
            NodeKind::Polygon => {
                let mut pen = Pen::new(af);
                pen.poly(&shapes::node_polygon(n, &r), true);
                self.shape(n, &r, af, alpha, &pen, Fill::NonZero);
                self.words(id, af, alpha);
            }
            NodeKind::Text => {
                if let Some(c) = n.fill.as_deref().and_then(rgba) {
                    let mut pen = Pen::new(af);
                    pen.rrect(&r, [0.0; 4]);
                    self.fill_pen(&pen, Fill::NonZero, &Paint::Solid(c), alpha);
                }
                self.words(id, af, alpha);
            }
            NodeKind::Line | NodeKind::Path => self.line(n, &r, af, alpha),
            NodeKind::Image => {
                let pen = self.outline(n, &r, af, 0.0, 0.0, 0.0);
                self.shadows(n, &r, af, alpha, &pen, false);
                self.push_clip(&pen, false);
                match n.src.as_deref().and_then(pictures::get) {
                    Some(pic) => self.picture(n, &r, af, alpha, pic),
                    None => self.placeholder(&r, af, alpha),
                }
                self.pop_clip();
                self.shadows(n, &r, af, alpha, &pen, true);
                self.border(n, &r, af, alpha);
            }
            NodeKind::Icon => self.icon(n, &r, af, alpha),
            NodeKind::Page => {}
        }
    }

    /// Whether what `n` draws (a box-shaped node's box, its words and shadow included) misses
    /// the clip; lines, groups and unclipped frames are never skipped (what they hold may show).
    fn culled(&self, n: &Node, r: &Rect, af: Af) -> bool {
        let own = match n.kind {
            NodeKind::Frame if n.clip != Some(false) => *r,
            NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Sticky | NodeKind::Image | NodeKind::Icon | NodeKind::Text => match self.plan.words.get(&n.id) {
                Some(w) => r.union(&Rect::new(w.x, w.y, w.box_w.max(1.0), w.lines.len() as f64 * w.set.line)),
                None => *r,
            },
            _ => return false,
        };
        let reach = shapes::shadow_reach(n) + n.stroke_width.unwrap_or(2.0).max(0.0) + 2.0;
        let mut pen = Pen::new(af);
        pen.rrect(&own.inflate(reach), [0.0; 4]);
        pen.bbox(1.0).meet(&self.clip().r).empty()
    }

    /// `n`'s outline (its rounded box, or its ellipse), grown by `grow` and moved.
    fn outline(&self, n: &Node, r: &Rect, af: Af, grow: f64, dx: f64, dy: f64) -> Pen {
        let mut pen = Pen::new(af);
        let g = Rect::new(r.x - grow + dx, r.y - grow + dy, (r.w + 2.0 * grow).max(0.0), (r.h + 2.0 * grow).max(0.0));
        if n.kind == NodeKind::Ellipse {
            pen.ellipse(&g);
        } else {
            let radii = shapes::corner_radii(n, r).map(|c| if c > 0.0 { (c + grow).max(0.0) } else { 0.0 });
            pen.rrect(&g, radii);
        }
        pen
    }

    /// An ellipse's arc: a pie slice, a ring or a ring's segment.
    fn arc_pen(&self, n: &Node, r: &Rect, af: Af) -> Pen {
        let mut pen = Pen::new(af);
        let Some((start, sweep, inner)) = shapes::arc(n) else { return pen };
        let (cx, cy, rx, ry) = (r.cx(), r.cy(), r.w / 2.0, r.h / 2.0);
        let at = |deg: f64, k: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            (cx + rx * k * s, cy - ry * k * c)
        };
        let steps = ((sweep.abs() / 4.0).ceil() as usize).clamp(8, 180);
        if sweep.abs() >= 360.0 {
            for (k, dir) in [(1.0, 1.0), (inner, -1.0)] {
                for i in 0..=steps {
                    let p = at(start + dir * 360.0 * i as f64 / steps as f64, k);
                    if i == 0 { pen.move_to(p) } else { pen.line_to(p) }
                }
                pen.close();
            }
            return pen;
        }
        for i in 0..=steps {
            let p = at(start + sweep * i as f64 / steps as f64, 1.0);
            if i == 0 { pen.move_to(p) } else { pen.line_to(p) }
        }
        if inner > 0.0 {
            for i in (0..=steps).rev() {
                pen.line_to(at(start + sweep * i as f64 / steps as f64, inner));
            }
        } else {
            pen.line_to((cx, cy));
        }
        pen.close();
        pen
    }

    /// A polygon or an arc: shadows, fill, inner shadows, and its stroke inside the edge.
    fn shape(&mut self, n: &Node, r: &Rect, af: Af, alpha: f32, pen: &Pen, rule: Fill) {
        self.path_shadows(n, af, alpha, pen, rule, false);
        if let Some(p) = parse_paint(n, r, af, false) {
            self.fill_pen(pen, rule, &p, alpha);
        }
        self.path_shadows(n, af, alpha, pen, rule, true);
        let w = n.stroke_width.unwrap_or(1.0);
        if w > 0.0
            && (n.stroke.is_some() || n.stroke_gradient.is_some())
            && let Some(p) = parse_paint(n, r, af, true)
        {
            let px = (2.0 * w * af.scale()).max(1.0);
            let area = pen.bbox(px + 2.0).meet(&self.clip().r);
            if area.empty() {
                return;
            }
            let mut st = Stroke::new(px as f32);
            st.join(join_of(n));
            let mut m = render_mask(&pen.cmds, st.into(), area);
            let inside = render_mask(&pen.cmds, rule.into(), area);
            for (a, b) in m.iter_mut().zip(inside) {
                *a = ((u32::from(*a) * u32::from(b) + 127) / 255) as u8;
            }
            self.fill_cov(area, &m, &p, alpha);
        }
    }

    /// A box's (frame, rect, sticky, ellipse, image) outer or inner shadows.
    fn shadows(&mut self, n: &Node, r: &Rect, af: Af, alpha: f32, own: &Pen, inset: bool) {
        for s in shapes::effective_shadows(n).into_iter().filter(|s| s.inset == inset) {
            let Some(col) = rgba(&s.color).filter(|c| c[3] > 0) else { continue };
            let blur_px = s.blur.clamp(0.0, 200.0) * af.scale();
            let pad = (blur_px * 1.6).ceil() as i32 + 2;
            if inset {
                let hole = self.outline(n, r, af, -s.spread, s.x, s.y);
                let area = own.bbox(1.0).grow(pad).meet(&self.clip().r.grow(pad));
                let mut m = render_mask(&hole.cmds, Fill::NonZero.into(), area);
                for v in &mut m {
                    *v = 255 - *v;
                }
                blur(&mut m, area.w as usize, area.h as usize, blur_px / 2.0);
                let inside = render_mask(&own.cmds, Fill::NonZero.into(), area);
                for (a, b) in m.iter_mut().zip(inside) {
                    *a = ((u32::from(*a) * u32::from(b) + 127) / 255) as u8;
                }
                self.fill_cov(area, &m, &Paint::Solid(col), alpha);
            } else {
                let cast = self.outline(n, r, af, s.spread, s.x, s.y);
                let area = cast.bbox(1.0).grow(pad).meet(&self.clip().r.grow(pad));
                if area.empty() {
                    continue;
                }
                let mut m = render_mask(&cast.cmds, Fill::NonZero.into(), area);
                blur(&mut m, area.w as usize, area.h as usize, blur_px / 2.0);
                // Outside the box only, as GSK's outset shadow.
                let inside = render_mask(&own.cmds, Fill::NonZero.into(), area);
                for (a, b) in m.iter_mut().zip(inside) {
                    *a = ((u32::from(*a) * u32::from(255 - b) + 127) / 255) as u8;
                }
                self.fill_cov(area, &m, &Paint::Solid(col), alpha);
            }
        }
    }

    /// A drawn path's shadows (a polygon's, an arc's): moved, blurred, no spread.
    fn path_shadows(&mut self, n: &Node, af: Af, alpha: f32, own: &Pen, rule: Fill, inset: bool) {
        for s in shapes::effective_shadows(n).into_iter().filter(|s| s.inset == inset) {
            let Some(col) = rgba(&s.color).filter(|c| c[3] > 0) else { continue };
            let blur_px = s.blur.clamp(0.0, 200.0) * af.scale();
            let pad = (blur_px * 1.6).ceil() as i32 + 2;
            let (dx, dy) = (s.x * af.a + s.y * af.c, s.x * af.b + s.y * af.d);
            let moved: Vec<Command> = own.cmds.iter().map(|c| shift(c, dx as f32, dy as f32)).collect();
            let mut mp = Pen::new(af);
            mp.cmds = moved;
            mp.lo = (own.lo.0 + dx, own.lo.1 + dy);
            mp.hi = (own.hi.0 + dx, own.hi.1 + dy);
            if inset {
                let area = own.bbox(1.0).grow(pad).meet(&self.clip().r.grow(pad));
                let mut m = render_mask(&mp.cmds, rule.into(), area);
                for v in &mut m {
                    *v = 255 - *v;
                }
                blur(&mut m, area.w as usize, area.h as usize, blur_px / 2.0);
                let inside = render_mask(&own.cmds, rule.into(), area);
                for (a, b) in m.iter_mut().zip(inside) {
                    *a = ((u32::from(*a) * u32::from(b) + 127) / 255) as u8;
                }
                self.fill_cov(area, &m, &Paint::Solid(col), alpha);
            } else {
                let area = mp.bbox(1.0).grow(pad).meet(&self.clip().r.grow(pad));
                let mut m = render_mask(&mp.cmds, rule.into(), area);
                blur(&mut m, area.w as usize, area.h as usize, blur_px / 2.0);
                self.fill_cov(area, &m, &Paint::Solid(col), alpha);
            }
        }
    }

    /// The border inside the edge: each side its own width (`stroke_widths`), or dashes along it.
    fn border(&mut self, n: &Node, r: &Rect, af: Af, alpha: f32) {
        if n.stroke.is_none() && n.stroke_gradient.is_none() {
            return;
        }
        let Some(paint) = parse_paint(n, r, af, true) else { return };
        let w = n.stroke_width.unwrap_or(1.0).clamp(0.0, 1000.0);
        let sides = n.stroke_widths.unwrap_or([w; 4]).map(|v| if v.is_finite() { v.clamp(0.0, 1000.0) } else { 0.0 });
        if sides.iter().all(|v| *v <= 0.0) {
            return;
        }
        let outer = self.outline(n, r, af, 0.0, 0.0, 0.0);
        let area = outer.bbox(1.0).meet(&self.clip().r);
        if area.empty() {
            return;
        }
        if let Some(dash) = n.dash.as_ref().filter(|_| n.stroke_widths.is_none()) {
            let half = (w / 2.0).min(r.w / 2.0).min(r.h / 2.0);
            let pen = self.outline(n, r, af, -half, 0.0, 0.0);
            let s = af.scale();
            let dashes: Vec<f32> = dash.iter().map(|d| (d * s) as f32).collect();
            let mut st = Stroke::new((w * s).max(1.0) as f32);
            st.dash(&dashes, 0.0).cap(if dash.contains(&0.0) { Cap::Round } else { Cap::Butt });
            let m = render_mask(&pen.cmds, st.into(), area);
            self.fill_cov(area, &m, &paint, alpha);
            return;
        }
        // Hairlines stay a pixel wide at any scale.
        let hair = 1.0 / af.scale().max(1e-6);
        let [t, rt, b, l] = sides.map(|v| if v > 0.0 { v.max(hair) } else { 0.0 });
        let inner_r = Rect::new(r.x + l, r.y + t, (r.w - l - rt).max(0.0), (r.h - t - b).max(0.0));
        let mut inner = Pen::new(af);
        if inner_r.w > 0.0 && inner_r.h > 0.0 {
            if n.kind == NodeKind::Ellipse {
                inner.ellipse(&inner_r);
            } else {
                let [a, bb, c, d] = shapes::corner_radii(n, r);
                inner.rrect(&inner_r, [(a - t.max(l)).max(0.0), (bb - t.max(rt)).max(0.0), (c - b.max(rt)).max(0.0), (d - b.max(l)).max(0.0)]);
            }
        }
        let mut m = render_mask(&outer.cmds, Fill::NonZero.into(), area);
        let hole = render_mask(&inner.cmds, Fill::NonZero.into(), area);
        for (a, h) in m.iter_mut().zip(hole) {
            *a = ((u32::from(*a) * u32::from(255 - h) + 127) / 255) as u8;
        }
        self.fill_cov(area, &m, &paint, alpha);
    }

    /// A line or path: its fill (three or more free points), its stroke and its arrowheads.
    fn line(&mut self, n: &Node, r: &Rect, af: Af, alpha: f32) {
        let plan = self.plan;
        let mut pen = Pen::new(af);
        if shapes::routed(n) {
            let (p, curved) = shapes::line_points(&plan.scene, &plan.placed, n);
            if p.len() < 2 {
                return;
            }
            pen.move_to(p[0]);
            if curved && p.len() >= 4 {
                pen.cubic_to(p[1], p[2], p[3]);
            } else {
                for q in &p[1..] {
                    pen.line_to(*q);
                }
            }
        } else {
            let Some((start, segs)) = shapes::path_segments(n, (r.x, r.y)) else { return };
            pen.move_to(start);
            for s in segs {
                match s {
                    shapes::Seg::Line(p) => pen.line_to(p),
                    shapes::Seg::Quad(c, p) => pen.quad_to(c, p),
                    shapes::Seg::Cubic(c1, c2, p) => pen.cubic_to(c1, c2, p),
                }
            }
            if shapes::closed_shape(n) {
                pen.close();
            }
        }
        if shapes::fillable_line(n)
            && (n.fill.is_some() || n.gradient.is_some())
            && let Some(p) = parse_paint(n, r, af, false)
        {
            self.fill_pen(&pen, Fill::NonZero, &p, alpha);
        }
        let w = n.stroke_width.unwrap_or(2.0);
        let stroke_col = n.stroke.clone().unwrap_or_else(|| color::STROKE.to_string());
        let shown = n.stroke_gradient.is_some() || rgba(&stroke_col).is_some_and(|c| c[3] > 0);
        if w <= 0.0 || !shown {
            return;
        }
        let paint = if n.stroke_gradient.is_some() { parse_paint(n, r, af, true) } else { rgba(&stroke_col).map(Paint::Solid) };
        let Some(paint) = paint else { return };
        let s = af.scale();
        let px = (w * s).max(0.75);
        if shapes::closed_shape(n) {
            let area = pen.bbox(2.0 * px + 2.0).meet(&self.clip().r);
            let mut st = Stroke::new((2.0 * px) as f32);
            st.join(join_of(n));
            let mut m = render_mask(&pen.cmds, st.into(), area);
            let inside = render_mask(&pen.cmds, Fill::NonZero.into(), area);
            for (a, b) in m.iter_mut().zip(inside) {
                *a = ((u32::from(*a) * u32::from(b) + 127) / 255) as u8;
            }
            self.fill_cov(area, &m, &paint, alpha);
            return;
        }
        let dashes: Vec<f32> = n.dash.iter().flatten().map(|d| (d * s) as f32).collect();
        let cap = match n.stroke_cap.as_ref().map(|c| format!("{c:?}")).as_deref() {
            Some("Butt") => Cap::Butt,
            Some("Square") => Cap::Square,
            Some(_) => Cap::Round,
            None if !dashes.is_empty() && !dashes.contains(&0.0) => Cap::Butt,
            None => Cap::Round,
        };
        let mut st = Stroke::new(px as f32);
        st.cap(cap).join(join_of(n));
        if !dashes.is_empty() && dashes.iter().any(|d| *d > 0.0) {
            st.dash(&dashes, 0.0);
        }
        let area = pen.bbox(px + 2.0).meet(&self.clip().r);
        if !area.empty() {
            let m = render_mask(&pen.cmds, st.into(), area);
            self.fill_cov(area, &m, &paint, alpha);
        }
        for head in shapes::arrowheads(&plan.scene, &plan.placed, n, r) {
            let mut hp = Pen::new(af);
            hp.poly(&head, true);
            self.fill_pen(&hp, Fill::NonZero, &paint, alpha);
        }
    }

    /// An icon from the set, or its own outline, scaled to its box's shorter side and stroked.
    fn icon(&mut self, n: &Node, r: &Rect, af: Af, alpha: f32) {
        let data = n.icon_path.clone().or_else(|| n.icon.as_deref().and_then(icons::path).map(str::to_string));
        let Some(data) = data else {
            let mut pen = Pen::new(af);
            pen.rrect(r, [0.0; 4]);
            self.fill_pen(&pen, Fill::NonZero, &Paint::Solid([140, 140, 140, 77]), alpha);
            return;
        };
        let w = n.stroke_width.unwrap_or(2.0);
        if w <= 0.0 {
            return;
        }
        let col = rgba(&n.stroke.clone().unwrap_or_else(|| color::text_color(&self.plan.scene, n))).unwrap_or([0, 0, 0, 255]);
        let segs = self.icons.entry(data.clone()).or_insert_with(|| icon_segs(&data)).clone();
        let s = (r.w.min(r.h) / 24.0).max(0.01);
        let (ox, oy) = (r.cx() - 12.0 * s, r.cy() - 12.0 * s);
        let (fx, fy) = (n.flip_x == Some(true), n.flip_y == Some(true));
        let at = |x: f64, y: f64| (ox + if fx { 24.0 - x } else { x } * s, oy + if fy { 24.0 - y } else { y } * s);
        let mut pen = Pen::new(af);
        for seg in segs {
            match seg {
                ISeg::M(x, y) => pen.move_to(at(x, y)),
                ISeg::L(x, y) => pen.line_to(at(x, y)),
                ISeg::C(a, b, c, d, x, y) => pen.cubic_to(at(a, b), at(c, d), at(x, y)),
                ISeg::Q(a, b, x, y) => pen.quad_to(at(a, b), at(x, y)),
                ISeg::Z => pen.close(),
            }
        }
        let px = (w * s * af.scale()).max(0.75);
        let mut st = Stroke::new(px as f32);
        st.cap(Cap::Round).join(Join::Round);
        let area = pen.bbox(px + 2.0).meet(&self.clip().r);
        if !area.empty() {
            let m = render_mask(&pen.cmds, st.into(), area);
            self.fill_cov(area, &m, &Paint::Solid(col), alpha);
        }
    }

    /// A picture in its node's box `r`, inside the box's rounded clip: placed by its fit and
    /// mirrored about the box's centre as the node is flipped (only the picture mirrors, as on
    /// the PC: corners, shadows and border stay).
    fn picture(&mut self, n: &Node, r: &Rect, af: Af, alpha: f32, pic: std::sync::Arc<Picture>) {
        let t = fit_box(fit_mode(n), f64::from(pic.w), f64::from(pic.h), r);
        let (fx, fy) = (n.flip_x == Some(true), n.flip_y == Some(true));
        let af = if fx || fy {
            let (sx, sy) = (if fx { -1.0 } else { 1.0 }, if fy { -1.0 } else { 1.0 });
            af.then(&Af { a: sx, b: 0.0, c: 0.0, d: sy, e: if fx { 2.0 * r.cx() } else { 0.0 }, f: if fy { 2.0 * r.cy() } else { 0.0 } })
        } else {
            af
        };
        let level = pic.level_for(f64::from(pic.w) / (t.w * af.scale()).max(1e-6));
        let mut pen = Pen::new(af);
        pen.rrect(&t, [0.0; 4]);
        let paint = Paint::Pic(Box::new(PicPaint { pic, level, inv: af.invert(), t }));
        self.fill_pen(&pen, Fill::NonZero, &paint, alpha);
    }

    /// A picture the phone cannot show (yet): a quiet tile with a picture glyph.
    fn placeholder(&mut self, r: &Rect, af: Af, alpha: f32) {
        let mut pen = Pen::new(af);
        pen.rrect(r, [0.0; 4]);
        self.fill_pen(&pen, Fill::NonZero, &Paint::Solid([140, 140, 140, 64]), alpha);
        let Some(data) = icons::path("image") else { return };
        let size = (r.w.min(r.h) * 0.25).clamp(6.0, 48.0);
        let s = size / 24.0;
        let (ox, oy) = (r.cx() - 12.0 * s, r.cy() - 12.0 * s);
        let mut pen = Pen::new(af);
        for seg in icon_segs(data) {
            let at = |x: f64, y: f64| (ox + x * s, oy + y * s);
            match seg {
                ISeg::M(x, y) => pen.move_to(at(x, y)),
                ISeg::L(x, y) => pen.line_to(at(x, y)),
                ISeg::C(a, b, c, d, x, y) => pen.cubic_to(at(a, b), at(c, d), at(x, y)),
                ISeg::Q(a, b, x, y) => pen.quad_to(at(a, b), at(x, y)),
                ISeg::Z => pen.close(),
            }
        }
        let px = (1.5 * s * af.scale()).max(0.75);
        let mut st = Stroke::new(px as f32);
        st.cap(Cap::Round).join(Join::Round);
        let area = pen.bbox(px + 2.0).meet(&self.clip().r);
        let m = render_mask(&pen.cmds, st.into(), area);
        self.fill_cov(area, &m, &Paint::Solid([140, 140, 140, 150]), alpha);
    }

    /// A node's words where the plan set them. Under a turn, each line starts where the turn
    /// puts it but is set level.
    fn words(&mut self, id: &str, af: Af, alpha: f32) {
        let Some(w) = self.plan.words.get(id) else { return };
        let s = af.scale();
        let set = w.set;
        let px = (set.size * s) as f32;
        if px < 0.6 {
            return;
        }
        let px_key = (px * 4.0).round() as u32;
        let faces = faces();
        for (li, line) in w.lines.iter().enumerate() {
            let top = w.y + li as f64 * set.line;
            let lx = match w.align {
                1 => w.x + (w.box_w - line.w) / 2.0,
                2 => w.x + w.box_w - line.w,
                _ => w.x,
            };
            let (asc, desc) = faces[set.face].horizontal_line_metrics(set.size as f32).map_or((set.size * 0.9, -set.size * 0.25), |m| (f64::from(m.ascent), f64::from(m.descent)));
            let baseline = top + (set.line - (asc - desc)) / 2.0 + asc;
            let words = w.text[line.start..line.end].trim_end();
            let ell = if line.cut { "…" } else { "" };
            let mut pen_x = lx;
            let mut prev: Option<(usize, u16)> = None;
            for (bi, c) in words.char_indices().map(|(i, c)| (line.start + i, c)).chain(ell.chars().map(|c| (usize::MAX, c))) {
                let run = w.runs.iter().find(|(a, b, _, _)| bi >= *a && bi < *b);
                let fi = run.and_then(|r| r.3).unwrap_or(set.face);
                let col = run.and_then(|r| r.2).unwrap_or(w.color);
                let f = &faces[fi];
                let gi = f.lookup_glyph_index(c);
                if gi == 0 {
                    continue;
                }
                if let Some((pf, pg)) = prev
                    && pf == fi
                {
                    pen_x += f64::from(f.horizontal_kern_indexed(pg, gi, set.size as f32).unwrap_or(0.0));
                }
                let (gx, gy) = af.apply((pen_x, baseline));
                let (m, bmp) = self.glyphs.entry((fi, gi, px_key)).or_insert_with(|| f.rasterize_indexed(gi, px));
                if m.width > 0 && m.height > 0 {
                    let r = IRect { x: (gx + f64::from(m.xmin)).round() as i32, y: (gy - f64::from(m.ymin) - m.height as f64).round() as i32, w: m.width as i32, h: m.height as i32 };
                    let bmp = std::mem::take(bmp);
                    self.fill_cov(r, &bmp, &Paint::Solid(col), alpha);
                    if let Some(e) = self.glyphs.get_mut(&(fi, gi, px_key)) {
                        e.1 = bmp;
                    }
                }
                pen_x += f64::from(f.metrics_indexed(gi, set.size as f32).advance_width) + set.spacing;
                prev = Some((fi, gi));
            }
            let thick = (set.size * 0.07).max(1.0 / s.max(1e-6));
            for (on, at) in [(w.underline, baseline + set.size * 0.12), (w.strike, baseline - set.size * 0.28)] {
                if on {
                    let mut pen = Pen::new(af);
                    pen.rrect(&Rect::new(lx, at - thick / 2.0, line.w, thick), [0.0; 4]);
                    self.fill_pen(&pen, Fill::NonZero, &Paint::Solid(w.color), alpha);
                }
            }
        }
    }
}

fn join_of(n: &Node) -> Join {
    match n.stroke_join.as_ref().map(|j| format!("{j:?}")).as_deref() {
        Some("Miter") => Join::Miter,
        Some("Bevel") => Join::Bevel,
        _ => Join::Round,
    }
}

fn shift(c: &Command, dx: f32, dy: f32) -> Command {
    let m = |v: &Vector| Vector::new(v.x + dx, v.y + dy);
    match c {
        Command::MoveTo(p) => Command::MoveTo(m(p)),
        Command::LineTo(p) => Command::LineTo(m(p)),
        Command::CurveTo(a, b, p) => Command::CurveTo(m(a), m(b), m(p)),
        Command::QuadTo(a, p) => Command::QuadTo(m(a), m(p)),
        Command::Close => Command::Close,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(size: f64) -> Set {
        Set { face: 0, size, line: size * 1.3, spacing: 0.0 }
    }

    #[test]
    fn faces_follow_the_font_and_weight() {
        assert_eq!(face(None, 400), 0);
        assert_eq!(face(Some("Inter"), 500), 1);
        assert_eq!(face(Some("Geist"), 700), 2);
        assert_eq!(face(Some("Geist Mono"), 700), 5);
        assert_eq!(face(Some("Literata"), 400), 4);
        assert_eq!(face(Some("Source Serif 4"), 600), 6);
        assert_eq!(face(Some("Open Sans"), 400), 0);
    }

    #[test]
    fn words_wrap_within_their_width() {
        let text = "Designs open on artboards and the repo is one tap away";
        let one = lines(text, set(16.0), None, None);
        assert_eq!(one.len(), 1);
        let wrapped = lines(text, set(16.0), Some(120.0), None);
        assert!(wrapped.len() >= 3, "{wrapped:?}");
        assert!(wrapped.iter().all(|l| l.w <= 120.5), "{wrapped:?}");
        // Every word is still there, in order.
        let back: Vec<&str> = wrapped.iter().map(|l| text[l.start..l.end].trim()).collect();
        assert_eq!(back.join(" "), text);
        // A word longer than the line is cut between letters.
        let long = lines("abcdefghijklmnopqrstuvwxyz", set(16.0), Some(40.0), None);
        assert!(long.len() > 3 && long.iter().all(|l| l.w <= 40.5 && l.end > l.start), "{long:?}");
        // Line breaks are kept, an empty line too.
        assert_eq!(lines("a\n\nb", set(16.0), None, None).len(), 3);
    }

    #[test]
    fn the_last_line_is_cut_at_max_lines() {
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let ls = lines(text, set(16.0), Some(90.0), Some(2));
        assert_eq!(ls.len(), 2);
        assert!(ls[1].cut && ls[1].w <= 90.5, "{ls:?}");
        assert!(!ls[0].cut);
    }

    #[test]
    fn setter_measures_as_the_pc_does() {
        let setter = Setter::default();
        let mut n = Node::read(json!({"id": "t", "kind": "text", "parent": "f", "text": "Hello"})).unwrap();
        let (w, h) = setter.measure(&n, None);
        assert!(w > 20.0 && w < 60.0, "{w}");
        assert!((h - 16.0 * 1.3).abs() < 1e-9);
        n.text = Some(String::new());
        n.id = "e".into();
        assert_eq!(setter.measure(&n, None), (2.0, 16.0 * 1.3));
    }

    #[test]
    fn fit_keeps_the_shape_inside_the_box() {
        assert_eq!(fit(390.0, 844.0, 195.0, 1000.0, 4.0), 0.5);
        assert_eq!(fit(100.0, 100.0, 1000.0, 1000.0, 4.0), 4.0);
        assert!((fit(1200.0, 600.0, 300.0, 300.0, 4.0) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn artboards_go_two_across_and_wide_ones_full_width() {
        let phone = (390.0, 844.0);
        let wide = (1440.0, 900.0);
        let (at, h) = arrange(&[phone, phone, phone, wide, phone], 358.0, 10.0, 30.0, 2.6);
        assert_eq!(at.len(), 5);
        // The first two share a row, side by side.
        assert_eq!(at[0][1], at[1][1]);
        assert!(at[1][0] > at[0][0] + at[0][2]);
        // The third starts the next row on the left; the wide one gets its own row under it.
        assert!(at[2][1] > at[0][1] && at[2][0] < 10.0);
        assert!(at[3][1] > at[2][1] + at[2][3]);
        assert!((at[3][2] - 358.0).abs() < 1e-3);
        // Shapes are kept.
        for (r, (w, hh)) in at.iter().zip([phone, phone, phone, wide, phone]) {
            assert!((r[2] / r[3] - (w / hh) as f32).abs() < 1e-3);
        }
        assert!(h >= at[4][1] + at[4][3]);
        // A very tall one is held to the cap.
        let (tall, _) = arrange(&[(100.0, 2000.0)], 358.0, 10.0, 30.0, 2.6);
        assert!(tall[0][3] <= 174.0 * 2.6 + 1e-3);
    }

    #[test]
    fn the_strip_shows_the_first_artboards() {
        let (s, at) = strip_layout(&[(390.0, 844.0), (390.0, 844.0), (390.0, 844.0)], 358.0, 150.0, 10.0, 8.0);
        assert!((s - 130.0 / 844.0).abs() < 1e-9);
        assert_eq!(at.len(), 3);
        // Centred: as much room on the left as on the right.
        let right = 358.0 - (at[2].0 + 390.0 * s);
        assert!((at[0].0 - right).abs() < 1e-6);
        // A wide first artboard is narrowed to fit across.
        let (s2, at2) = strip_layout(&[(4000.0, 800.0)], 358.0, 150.0, 10.0, 8.0);
        assert!(4000.0 * s2 <= 338.0 + 1e-9 && at2.len() == 1);
        assert!(strip_layout(&[], 10.0, 10.0, 1.0, 1.0).1.is_empty());
    }

    #[test]
    fn colours_read_every_form() {
        assert_eq!(rgba("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(rgba("#D97757"), Some([0xD9, 0x77, 0x57, 255]));
        assert_eq!(rgba("#00000000"), Some([0, 0, 0, 0]));
        assert_eq!(rgba("D97757"), None);
    }

    pub(crate) fn sample_page() -> Value {
        crate::designs::sample_page()
    }

    #[test]
    fn a_page_is_planned_and_painted() {
        // Its picture never fetched (the shots' sample one may be kept by another test).
        let mut v = sample_page();
        for n in v["nodes"].as_array_mut().unwrap() {
            if n["kind"] == "image" {
                n["src"] = json!("never-fetched.png");
            }
        }
        let p = plan(7, v).unwrap();
        assert_eq!(p.boards.len(), 3);
        assert_eq!(p.boards[0].name, "Home");
        assert!(p.bounds.is_some());
        assert_eq!(p.omitted.pictures, 0);
        // Two image nodes show the one picture.
        assert_eq!(p.omitted_now().pictures, 2);
        assert_eq!(p.pictures.len(), 1);
        // 358 × 150 in a 390-wide artboard, at four picture px a page px (the 400 × 220 one in
        // the 1200-wide artboard needs less).
        assert_eq!((p.pictures[0].w, p.pictures[0].h), (1432, 600));
        assert!(p.omitted_now().caption().unwrap().starts_with("Not drawn here: 2 pictures (shown as tiles)"));
        pictures::loading("never-fetched.png");
        assert_eq!(p.omitted_now().caption().unwrap(), "Not drawn here: 2 pictures (still loading).");
        assert!(!p.pictures_settled());
        pictures::forget("never-fetched.png");
        // A board: its own fill where nothing else is, the accent where its key is.
        let b = &p.boards[0];
        let pix = board(&p, &b.id, 195, 422);
        assert_eq!((pix.w, pix.h, pix.data.len()), (195, 422, 195 * 422 * 4));
        let at = |x: u32, y: u32| &pix.data[((y * pix.w + x) * 4) as usize..((y * pix.w + x) * 4 + 3) as usize];
        assert_eq!(at(2, 300), &[0x1F, 0x1E, 0x1D]);
        let accent = (0..pix.h).flat_map(|y| (0..pix.w).map(move |x| (x, y))).any(|(x, y)| at(x, y) == [0xD9, 0x77, 0x57]);
        assert!(accent, "the primary key is painted in the accent");
        // Words are painted: light pixels on the dark board.
        let light = (0..pix.h).flat_map(|y| (0..pix.w).map(move |x| (x, y))).filter(|&(x, y)| at(x, y)[0] > 200 && at(x, y)[2] > 200).count();
        assert!(light > 50, "{light}");
        let s = strip(&p, 358, 150, 10.0, 8.0);
        assert_eq!(s.data.len(), 358 * 150 * 4);
        let whole = page(&p, 300, 200);
        assert_eq!(whole.data.len(), 300 * 200 * 4);
    }

    #[test]
    fn pictures_cover_contain_and_stretch_their_box() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        // A square picture: cover is as wide as the box and runs over above and below.
        let c = fit_box(FitMode::Cover, 40.0, 40.0, &r);
        assert_eq!(c, Rect::new(10.0, -5.0, 100.0, 100.0));
        // Contain is as tall as the box, centred across.
        let k = fit_box(FitMode::Contain, 40.0, 40.0, &r);
        assert_eq!(k, Rect::new(35.0, 20.0, 50.0, 50.0));
        assert_eq!(fit_box(FitMode::Stretch, 40.0, 40.0, &r), r);
        let n = Node::read(json!({"id": "i", "kind": "image", "parent": "f", "src": "a.png"})).unwrap();
        assert_eq!(fit_mode(&n), FitMode::Cover);
        let n = Node::read(json!({"id": "i", "kind": "image", "parent": "f", "src": "a.png", "fit": "contain"})).unwrap();
        assert_eq!(fit_mode(&n), FitMode::Contain);
    }

    /// A page of one 100×100 artboard (ground #102030) holding `image`.
    fn picture_page(image: Value) -> Plan {
        plan(9, json!({"design": {"id": 9, "rev": 1}, "nodes": [
            {"id": "pg", "kind": "page", "fill": "#000000"},
            {"id": "f", "kind": "frame", "parent": "pg", "x": 0, "y": 0, "w": 100, "h": 100, "fill": "#102030"},
            image,
        ]}))
        .unwrap()
    }

    fn px(p: &Pix, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * p.w + x) * 4) as usize;
        [p.data[i], p.data[i + 1], p.data[i + 2]]
    }

    #[test]
    fn a_picture_is_painted_in_its_rounded_box() {
        // Left half red, right half blue.
        let two = |w: u32, h: u32| Picture::new(w, h, (0..h).flat_map(|_| (0..w).flat_map(move |x| if x < w / 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] })).collect());
        pictures::put("paint-test-a.png", Ok(two(8, 8)));
        let p = picture_page(json!({"id": "i", "kind": "image", "parent": "f", "x": 0, "y": 0, "w": 100, "h": 100, "src": "paint-test-a.png", "radius": 30}));
        assert_eq!(p.omitted_now().pictures, 0);
        assert!(p.omitted_now().caption().is_none());
        let pix = board(&p, "f", 100, 100);
        // The corner is cut round: the artboard's ground shows there.
        assert_eq!(px(&pix, 1, 1), [0x10, 0x20, 0x30]);
        // The picture inside.
        assert_eq!(px(&pix, 20, 50), [255, 0, 0]);
        assert_eq!(px(&pix, 80, 50), [0, 0, 255]);
        // Flipped across, the halves trade places; the corners stay cut.
        let f = picture_page(json!({"id": "i", "kind": "image", "parent": "f", "x": 0, "y": 0, "w": 100, "h": 100, "src": "paint-test-a.png", "radius": 30, "flip_x": true}));
        let pix = board(&f, "f", 100, 100);
        assert_eq!(px(&pix, 20, 50), [0, 0, 255]);
        assert_eq!(px(&pix, 1, 98), [0x10, 0x20, 0x30]);
        // Contain: a wide picture in a square box leaves the ground above and below; at half
        // opacity it is half laid over the ground.
        pictures::put("paint-test-b.png", Ok(two(40, 20)));
        let c = picture_page(json!({"id": "i", "kind": "image", "parent": "f", "x": 0, "y": 0, "w": 100, "h": 100, "src": "paint-test-b.png", "fit": "contain", "opacity": 0.5}));
        let pix = board(&c, "f", 100, 100);
        assert_eq!(px(&pix, 50, 5), [0x10, 0x20, 0x30]);
        let mid = px(&pix, 20, 50);
        assert!((i32::from(mid[0]) - (255 + 0x10) / 2).abs() <= 2 && (i32::from(mid[2]) - 0x30 / 2).abs() <= 2, "{mid:?}");
        // Drawn small, from a smaller copy, still the picture's colours.
        let small = board(&p, "f", 10, 10);
        assert!(px(&small, 2, 5)[0] > 200 && px(&small, 7, 5)[2] > 200);
    }

    #[test]
    fn a_page_without_layers_is_said() {
        assert!(plan(1, json!({"nodes": []})).is_err());
        assert!(plan(1, json!({})).is_err());
    }

    #[test]
    fn blur_spreads_and_keeps_the_total_about() {
        let (w, h) = (40usize, 40usize);
        let mut m = vec![0u8; w * h];
        for y in 15..25 {
            for x in 15..25 {
                m[y * w + x] = 255;
            }
        }
        let before: u32 = m.iter().map(|v| u32::from(*v)).sum();
        blur(&mut m, w, h, 3.0);
        let after: u32 = m.iter().map(|v| u32::from(*v)).sum();
        assert!(m[10 * w + 20] > 0 && m[20 * w + 20] < 255);
        assert!((f64::from(after) / f64::from(before) - 1.0).abs() < 0.05);
    }

    /// Paints a real `design.get` answer to PNGs to look at: `DESIGN_JSON=page.json
    /// DESIGN_PNG=out cargo test -p relay-code design_paint::tests::paint_a_real_page -- --ignored`.
    #[test]
    #[ignore]
    #[cfg(not(target_os = "android"))]
    fn paint_a_real_page() {
        let (Ok(src), Ok(out)) = (std::env::var("DESIGN_JSON"), std::env::var("DESIGN_PNG")) else { return };
        // `sample` paints the shots' sample page.
        let v: Value = if src == "sample" { sample_page() } else { serde_json::from_str(&std::fs::read_to_string(&src).unwrap()).unwrap() };
        let v = v.get("result").cloned().unwrap_or(v);
        let t = std::time::Instant::now();
        let p = plan(1, v).unwrap();
        eprintln!("planned in {:?}: {} boards, {:?}", t.elapsed(), p.boards.len(), p.omitted.caption());
        let save = |name: &str, pix: &Pix| {
            let file = std::fs::File::create(format!("{out}-{name}.png")).unwrap();
            let mut e = png::Encoder::new(std::io::BufWriter::new(file), pix.w, pix.h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&pix.data).unwrap();
        };
        let t = std::time::Instant::now();
        save("strip", &strip(&p, 990, 415, 28.0, 22.0));
        save("page", &page(&p, 1600, 1000));
        for (i, b) in p.boards.iter().enumerate().take(6) {
            let s = fit(b.rect.w, b.rect.h, 1000.0, 2000.0, 4.0);
            save(&format!("board{i}"), &board(&p, &b.id, (b.rect.w * s) as u32, (b.rect.h * s) as u32));
        }
        eprintln!("painted in {:?}", t.elapsed());
    }
}
