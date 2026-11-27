//! What screens draw with: rounded boxes, soft shadows, arcs, text, icons and pictures, in layout
//! units. A unit is 1/390 of a phone's width (the design canvas's artboards are 390 wide), so a
//! layout written once fits every phone; a wide desktop window keeps phone proportions.
//!
//! Copied from Tally RS (`apps/tally/native/crates/tally-rs/src/draw.rs`). Leaf adds reserved
//! picture slots ([`Canvas::add_image_slot`]), so a reader's page tiles and the covers each keep to
//! one slot size and an evicted slot always fits the next of its kind, and hard-edged tiles
//! ([`Canvas::image_tile`]) that meet without a seam.
//!
//! Everything pushed here becomes one [`Inst`]; nothing touches the GPU until
//! [`crate::gfx::Gfx::render`]. Clip regions split the list into [`Batch`]es.

use std::collections::HashMap;

use crate::gfx::{self, AtlasUpload, Batch, Globals, Inst};
use crate::glyphs;

/// Width of every layout, in units.
pub const WIDTH: f32 = 390.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn hex(rgb: u32) -> Rgba {
        Rgba([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255])
    }
    /// From Android's 0xAARRGGBB.
    pub const fn argb(c: u32) -> Rgba {
        Rgba([(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8])
    }
    pub const WHITE: Rgba = Rgba::hex(0xFFFFFF);
    pub const CLEAR: Rgba = Rgba([0, 0, 0, 0]);

    /// The same colour at `a` opacity (0 to 1) of what it had.
    pub fn a(self, a: f32) -> Rgba {
        let [r, g, b, alpha] = self.0;
        Rgba([r, g, b, (alpha as f32 * a.clamp(0.0, 1.0)).round() as u8])
    }

    pub fn mix(self, other: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba([m(self.0[0], other.0[0]), m(self.0[1], other.0[1]), m(self.0[2], other.0[2]), m(self.0[3], other.0[3])])
    }

    pub fn rgb_f32(self) -> [f32; 3] {
        [self.0[0] as f32 / 255.0, self.0[1] as f32 / 255.0, self.0[2] as f32 / 255.0]
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// The faces: Geist at three weights, Sora for the wordmark, and Literata (a reading serif) for
/// books: regular, italic and semibold.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Weight {
    Regular = 0,
    Medium = 1,
    Semi = 2,
    Brand = 3,
    Serif = 4,
    SerifItalic = 5,
    SerifBold = 6,
}

/// How many faces the canvas holds ([`Weight`]).
pub const FACES: usize = 7;

/// How a run of text is set. Text is placed by the top of its line box (`size` × `line`).
#[derive(Clone, Copy, Debug)]
pub struct Text {
    pub size: f32,
    pub weight: Weight,
    pub color: Rgba,
    pub align: Align,
    /// Line height as a multiple of the size.
    pub line: f32,
}

impl Text {
    pub fn new(size: f32, color: Rgba) -> Text {
        Text { size, weight: Weight::Regular, color, align: Align::Left, line: 1.3 }
    }
    pub fn medium(mut self) -> Text {
        self.weight = Weight::Medium;
        self
    }
    pub fn semi(mut self) -> Text {
        self.weight = Weight::Semi;
        self
    }
    pub fn brand(mut self) -> Text {
        self.weight = Weight::Brand;
        self
    }
    pub fn face(mut self, w: Weight) -> Text {
        self.weight = w;
        self
    }
    pub fn center(mut self) -> Text {
        self.align = Align::Center;
        self
    }
    pub fn right(mut self) -> Text {
        self.align = Align::Right;
        self
    }
    pub fn color(mut self, c: Rgba) -> Text {
        self.color = c;
        self
    }
    pub fn line(mut self, l: f32) -> Text {
        self.line = l;
        self
    }
    /// Height of one line, in units.
    pub fn lh(&self) -> f32 {
        self.size * self.line
    }
}

#[derive(Clone, Copy)]
struct Glyph {
    uv: [f32; 4],
    w: f32,
    h: f32,
    left: f32,
    /// Distance from the baseline down to the glyph's bottom edge (negative above it).
    bottom: f32,
    advance: f32,
}

#[derive(Clone, Copy)]
struct Picture {
    uv: [f32; 4],
    w: u32,
    h: u32,
    /// The part of the atlas it holds (x, y, w, h): at least the picture, kept whole for the
    /// next picture when this one is evicted.
    slot: [u32; 4],
    /// The last frame that drew it.
    used: u64,
}

/// The picture atlas's room. Pictures go on shelves; when there is no room, the picture drawn
/// longest ago, and not lately, gives up its slot (nearly every picture is the same size, so a
/// slot fits the next one). A picture drawn this frame or the last is never evicted, so the set a
/// page shows stays put however many others come and go.
struct ImageAtlas {
    size: u32,
    /// Next free place: x, y, and the height of the shelf being filled.
    shelf: (u32, u32, u32),
    /// Slots given up by evicted pictures.
    free: Vec<[u32; 4]>,
    pics: HashMap<String, Picture>,
    /// The frame being drawn (counted by [`Canvas::begin`]).
    frame: u64,
}

impl ImageAtlas {
    fn new(size: u32) -> ImageAtlas {
        ImageAtlas { size, shelf: (1, 1, 0), free: Vec::new(), pics: HashMap::new(), frame: 0 }
    }

    fn reset(&mut self) {
        self.shelf = (1, 1, 0);
        self.free.clear();
        self.pics.clear();
    }

    fn shelf_place(&mut self, w: u32, h: u32) -> Option<[u32; 4]> {
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > self.size {
            x = 1;
            y += row + 1;
            row = 0;
        }
        if x + w + 1 > self.size || y + h + 1 > self.size {
            return None;
        }
        self.shelf = (x + w + 1, y, row.max(h));
        Some([x, y, w, h])
    }

    /// Room for a `w`×`h` picture under `key`, evicting only pictures last drawn before frame
    /// `keep_from`. None when everything that could make room is still being drawn.
    #[cfg_attr(not(test), allow(dead_code))]
    fn place(&mut self, key: &str, w: u32, h: u32, keep_from: u64) -> Option<[u32; 4]> {
        self.place_in(key, w, h, (w, h), keep_from)
    }

    /// As [`Self::place`], in a slot at least `slot` big: pictures of one kind reserve one slot
    /// size, so the slot one gives up always fits the next.
    fn place_in(&mut self, key: &str, w: u32, h: u32, slot: (u32, u32), keep_from: u64) -> Option<[u32; 4]> {
        let (sw, sh) = (slot.0.max(w), slot.1.max(h));
        let fits = |s: &[u32; 4]| s[2] >= sw && s[3] >= sh;
        let slot = if let Some(i) = (0..self.free.len()).filter(|&i| fits(&self.free[i])).min_by_key(|&i| self.free[i][2] * self.free[i][3]) {
            Some(self.free.swap_remove(i))
        } else if let Some(s) = self.shelf_place(sw, sh) {
            Some(s)
        } else if let Some(old) = self
            .pics
            .iter()
            .filter(|(_, p)| p.used < keep_from && fits(&p.slot))
            .min_by_key(|(_, p)| (p.used, p.slot[2] * p.slot[3]))
            .map(|(k, _)| k.clone())
        {
            self.pics.remove(&old).map(|p| p.slot)
        } else if self.pics.values().all(|p| p.used < keep_from) {
            // Nothing on screen, yet no single slot fits (an odd size): start the atlas over.
            self.reset();
            self.shelf_place(sw, sh)
        } else {
            None
        }?;
        let s = self.size as f32;
        let (x, y) = (slot[0], slot[1]);
        let uv = [(x as f32 + 0.5) / s, (y as f32 + 0.5) / s, (x + w) as f32 / s - 0.5 / s, (y + h) as f32 / s - 0.5 / s];
        self.pics.insert(key.to_string(), Picture { uv, w, h, slot, used: self.frame });
        Some([x, y, w, h])
    }
}

/// An RGBA picture at half its width and height, each pixel the average of the four it stands for.
fn half_size(w: u32, h: u32, pixels: &[u8]) -> (u32, u32, Vec<u8>) {
    let (hw, hh) = (w / 2, h / 2);
    let row = (w * 4) as usize;
    let mut out = Vec::with_capacity((hw * hh * 4) as usize);
    for y in 0..hh as usize {
        let top = &pixels[2 * y * row..];
        let bottom = &pixels[(2 * y + 1) * row..];
        for x in 0..hw as usize {
            let i = 8 * x;
            for c in 0..4 {
                let sum = top[i + c] as u32 + top[i + 4 + c] as u32 + bottom[i + c] as u32 + bottom[i + 4 + c] as u32;
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    (hw, hh, out)
}

/// Past this shelf, the glyph atlas starts over at the next frame's start: a frame's new glyphs
/// fit in the quarter left.
const GLYPHS_START_OVER: u32 = gfx::ATLAS_SIZE * 3 / 4;

/// A decoded picture waiting for room: its key, size, the slot it reserves, and its pixels.
type Waiting = (String, u32, u32, (u32, u32), Vec<u8>);

/// How many decoded pictures may wait for room in the atlas.
const MAX_WAITING: usize = 6;

pub struct Canvas {
    pub insts: Vec<Inst>,
    pub batches: Vec<Batch>,
    pub uploads: Vec<AtlasUpload>,
    pub image_uploads: Vec<AtlasUpload>,
    images: ImageAtlas,
    /// Decoded pictures with no room yet: placed at the end of a frame that leaves some.
    image_waiting: Vec<Waiting>,
    /// A waiting picture was placed after the frame was drawn, or the glyph atlas started over
    /// mid-frame: draw another.
    image_late: bool,
    fonts: [fontdue::Font; FACES],
    glyphs: HashMap<(u8, u16, u16), Glyph>,
    icons: HashMap<(&'static str, u16), Glyph>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    clips: Vec<[f32; 4]>,
    /// Target size in pixels.
    pub px_w: f32,
    pub px_h: f32,
    /// Pixels per unit.
    pub u: f32,
    /// Layout height in units (the width is always [`WIDTH`]).
    pub h: f32,
    /// Space at the top and bottom that the system draws over (status bar, gesture bar), in units.
    pub inset_top: f32,
    pub inset_bottom: f32,
    /// Added to every y drawn, in units: a scrolled region sets it.
    pub dy: f32,
    /// Multiplies every alpha. Screens fade by setting it.
    pub fade: f32,
    pub globals: Globals,
}

impl Canvas {
    pub fn new(fonts: [&[u8]; FACES]) -> Canvas {
        let load = |bytes: &[u8]| {
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings { scale: 48.0, ..Default::default() })
                .expect("the bundled font is valid")
        };
        Canvas {
            insts: Vec::with_capacity(4096),
            batches: Vec::with_capacity(16),
            uploads: Vec::new(),
            image_uploads: Vec::new(),
            images: ImageAtlas::new(gfx::IMAGES_SIZE),
            image_waiting: Vec::new(),
            image_late: false,
            fonts: fonts.map(load),
            glyphs: HashMap::with_capacity(512),
            icons: HashMap::with_capacity(64),
            shelf_x: 1,
            shelf_y: 1,
            shelf_h: 0,
            clips: Vec::new(),
            px_w: 1.0,
            px_h: 1.0,
            u: 1.0,
            h: 844.0,
            inset_top: 0.0,
            inset_bottom: 0.0,
            dy: 0.0,
            fade: 1.0,
            globals: Globals { screen: [1.0, 1.0], _pad: [0.0; 2] },
        }
    }

    /// Start a frame for a target of this many pixels. Insets are in pixels.
    pub fn begin(&mut self, px_w: f32, px_h: f32, inset_top: f32, inset_bottom: f32) {
        self.insts.clear();
        self.batches.clear();
        self.batches.push(Batch { start: 0, end: 0, clip: None });
        self.clips.clear();
        self.px_w = px_w;
        self.px_h = px_h;
        self.u = (px_w / WIDTH).min(px_h / 700.0).max(0.1);
        self.h = px_h / self.u;
        self.inset_top = inset_top / self.u;
        self.inset_bottom = inset_bottom / self.u;
        self.dy = 0.0;
        self.fade = 1.0;
        self.globals.screen = [px_w, px_h];
        self.images.frame += 1;
        // A glyph atlas filling up starts over here, between frames, rather than in the middle of
        // one: there, what the frame had already queued would point at slots being reused.
        if self.shelf_y > GLYPHS_START_OVER {
            self.reset_glyphs();
        }
    }

    /// Forget every glyph and icon in the atlas; they are rasterised again as they are asked for.
    fn reset_glyphs(&mut self) {
        self.glyphs.clear();
        self.icons.clear();
        (self.shelf_x, self.shelf_y, self.shelf_h) = (1, 1, 0);
    }

    /// Close the last batch. Call once the frame is drawn.
    pub fn finish(&mut self) {
        let n = self.insts.len() as u32;
        if let Some(b) = self.batches.last_mut() {
            b.end = n;
        }
        // Pictures waiting for room take the slots of pictures this frame did not draw.
        if !self.image_waiting.is_empty() {
            let frame = self.images.frame;
            for (key, w, h, slot, pixels) in std::mem::take(&mut self.image_waiting) {
                match self.images.place_in(&key, w, h, slot, frame) {
                    Some([x, y, w, h]) => {
                        self.image_uploads.push(AtlasUpload { x, y, w, h, pixels });
                        self.image_late = true;
                    }
                    None => self.image_waiting.push((key, w, h, slot, pixels)),
                }
            }
        }
    }

    /// True once after a picture became drawable when the frame was already drawn, or after the
    /// glyph atlas had to start over in the middle of a frame (text queued before it would show
    /// garbled): the host should draw another frame.
    pub fn take_late_pictures(&mut self) -> bool {
        std::mem::take(&mut self.image_late)
    }

    /// Left edge of the layout column, in pixels: centred when the target is wider.
    fn x0(&self) -> f32 {
        ((self.px_w - WIDTH * self.u) * 0.5).max(0.0)
    }

    /// Draw only inside this rectangle (units, not moved by `dy`) until [`Self::pop_clip`].
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let u = self.u;
        let mut r = [self.x0() + x * u, y * u, w * u, h * u];
        if let Some(p) = self.clips.last() {
            let (x0, y0) = (r[0].max(p[0]), r[1].max(p[1]));
            let (x1, y1) = ((r[0] + r[2]).min(p[0] + p[2]), (r[1] + r[3]).min(p[1] + p[3]));
            r = [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)];
        }
        self.clips.push(r);
        self.new_batch();
    }

    pub fn pop_clip(&mut self) {
        self.clips.pop();
        self.new_batch();
    }

    fn new_batch(&mut self) {
        let n = self.insts.len() as u32;
        if let Some(b) = self.batches.last_mut() {
            b.end = n;
        }
        let clip = self.clips.last().map(|r| {
            let x = r[0].max(0.0).floor() as u32;
            let y = r[1].max(0.0).floor() as u32;
            [x, y, (r[2] + r[0] - x as f32).ceil().max(0.0) as u32, (r[3] + r[1] - y as f32).ceil().max(0.0) as u32]
        });
        self.batches.push(Batch { start: n, end: n, clip });
    }

    fn col(&self, c: Rgba) -> [u8; 4] {
        if self.fade >= 1.0 { c.0 } else { c.a(self.fade).0 }
    }

    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, x: f32, y: f32, w: f32, h: f32, c0: Rgba, c1: Rgba, p: [f32; 4], e: [f32; 4]) {
        let u = self.u;
        let ox = self.x0();
        let kind = p[3] as u32 & 15;
        let plain = kind == 0 && p[0] == 0.0 && p[1] == 0.0 && p[2] == 0.0;
        self.insts.push(Inst {
            rect: [ox + x * u, (y + self.dy) * u, w * u, h * u],
            uv: [0.0; 4],
            c0: self.col(c0),
            c1: self.col(c1),
            p: [p[0] * u, p[1] * u, p[2] * u, p[3] + if plain { gfx::FLAG_PLAIN } else { 0.0 }],
            e,
        });
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, c: Rgba) {
        self.push(x, y, w, h, c, c, [radius, 0.0, 0.0, gfx::KIND_BOX], [0.0; 4]);
    }

    /// Filled, shading from `top` to `bottom`.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_grad(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, top: Rgba, bottom: Rgba) {
        self.push(x, y, w, h, top, bottom, [radius, 0.0, 0.0, gfx::KIND_BOX], [0.0; 4]);
    }

    /// Filled, shading from `left` to `right`.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_hgrad(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, left: Rgba, right: Rgba) {
        self.push(x, y, w, h, left, right, [radius, 0.0, 0.0, gfx::KIND_BOX + gfx::FLAG_HGRAD], [0.0; 4]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, thickness: f32, c: Rgba) {
        self.push(x, y, w, h, c, c, [radius, thickness, 0.0, gfx::KIND_BOX], [0.0; 4]);
    }

    /// A soft shadow under a rounded rectangle, reaching `blur` units past it. Draw it first.
    #[allow(clippy::too_many_arguments)]
    pub fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, blur: f32, c: Rgba) {
        self.push(x, y, w, h, c, c, [radius, 0.0, blur, gfx::KIND_BOX], [0.0; 4]);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        self.rect(cx - r, cy - r, r * 2.0, r * 2.0, r, c);
    }

    /// A stroke along a circle from `from` to `to`, as fractions of a turn clockwise from the top.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, thickness: f32, from: f32, to: f32, c: Rgba, butt: bool) {
        let tau = std::f32::consts::TAU;
        let flags = gfx::KIND_ARC + if butt { gfx::FLAG_BUTT } else { 0.0 };
        self.push(cx - r, cy - r, r * 2.0, r * 2.0, c, c, [0.0, thickness, 0.0, flags], [0.0, from * tau, to * tau, 0.0]);
    }

    // Pictures

    /// Put a picture (RGBA, row by row) in the atlas under `key`. False when there is no room
    /// yet: it waits, and takes the slot of a picture no longer drawn at the end of a frame
    /// ([`Self::take_late_pictures`] then says so). Pictures drawn this frame or the last are
    /// never evicted for it.
    pub fn add_image(&mut self, key: &str, w: u32, h: u32, pixels: Vec<u8>) -> bool {
        self.add_image_slot(key, w, h, (w, h), pixels)
    }

    /// [`Self::add_image`] in a slot at least `slot` big (pixels, at full size): a reader's page
    /// tiles all reserve one tile's room, covers one cover's.
    pub fn add_image_slot(&mut self, key: &str, w: u32, h: u32, slot: (u32, u32), pixels: Vec<u8>) -> bool {
        if self.images.pics.contains_key(key) {
            return true;
        }
        let (sw, sh) = (slot.0.max(w), slot.1.max(h));
        if w == 0 || h == 0 || sw + 2 > self.images.size || sh + 2 > self.images.size || pixels.len() != (w * h * 4) as usize {
            return false;
        }
        // An atlas smaller than the full one (a GPU that stops at 2048) holds only four
        // full-size pictures, fewer than Goals shows at once: there each is kept at half size,
        // so four times as many fit. They are drawn at the same size on screen (by their place
        // in the atlas, not their pixels), only a little softer.
        let half = self.images.size < gfx::IMAGES_SIZE && w >= 2 && h >= 2;
        let (w, h, pixels) = if half { half_size(w, h, &pixels) } else { (w, h, pixels) };
        let slot = if half { (sw / 2, sh / 2) } else { (sw, sh) };
        let keep_from = self.images.frame.saturating_sub(1);
        if let Some([x, y, w, h]) = self.images.place_in(key, w, h, slot, keep_from) {
            self.image_waiting.retain(|(k, ..)| k != key);
            self.image_uploads.push(AtlasUpload { x, y, w, h, pixels });
            return true;
        }
        self.image_waiting.retain(|(k, ..)| k != key);
        if self.image_waiting.len() >= MAX_WAITING {
            self.image_waiting.remove(0);
        }
        self.image_waiting.push((key.to_string(), w, h, slot, pixels));
        false
    }

    /// Let a picture go now (its slot is free for the next one): it no longer stands for what it
    /// showed.
    pub fn forget_image(&mut self, key: &str) {
        if let Some(p) = self.images.pics.remove(key) {
            self.images.free.push(p.slot);
        }
        self.image_waiting.retain(|(k, ..)| k != key);
    }

    pub fn has_image(&self, key: &str) -> bool {
        self.images.pics.contains_key(key)
    }

    /// Decoded and waiting for room in the atlas (asking for it again would decode it again).
    pub fn image_waiting(&self, key: &str) -> bool {
        self.image_waiting.iter().any(|(k, ..)| k == key)
    }

    /// The GPU's picture atlas is `size` pixels a side (smaller than [`gfx::IMAGES_SIZE`] on
    /// some phones): pictures are placed in that much room from now on. Set once the GPU is up,
    /// before the first frame; a change empties the atlas, and pictures come back as they are
    /// asked for.
    pub fn set_images_size(&mut self, size: u32) {
        if size != self.images.size {
            let frame = self.images.frame;
            self.images = ImageAtlas::new(size);
            self.images.frame = frame;
            self.image_waiting.clear();
            self.image_uploads.clear();
        }
    }

    /// Empty the picture atlas; pictures are added again as they are asked for.
    pub fn clear_images(&mut self) {
        self.images.reset();
        self.image_waiting.clear();
    }

    /// A picture about to be drawn: its place, marked as drawn this frame.
    fn picture(&mut self, key: &str) -> Option<Picture> {
        let frame = self.images.frame;
        let p = self.images.pics.get_mut(key)?;
        p.used = frame;
        Some(*p)
    }

    /// Draw the picture `key` cropped to fill a rounded rectangle, as `ContentScale.Crop` does.
    #[allow(clippy::too_many_arguments)]
    pub fn image_cover(&mut self, key: &str, x: f32, y: f32, w: f32, h: f32, radius: f32, tint: Rgba) -> bool {
        self.image_cover_at(key, x, y, w, h, radius, tint, (0.5, 0.5))
    }

    /// Draw only `part` of the picture (left, top, right, bottom as fractions of it) stretched
    /// into the box: a close-up, such as the coin in a small circle.
    #[allow(clippy::too_many_arguments)]
    pub fn image_part(&mut self, key: &str, x: f32, y: f32, w: f32, h: f32, radius: f32, tint: Rgba, part: [f32; 4]) -> bool {
        let Some(p) = self.picture(key) else { return false };
        let [u0, v0, u1, v1] = p.uv;
        let (du, dv) = (u1 - u0, v1 - v0);
        let uv = [u0 + du * part[0], v0 + dv * part[1], u0 + du * part[2], v0 + dv * part[3]];
        let u = self.u;
        let col = self.col(tint);
        self.insts.push(Inst {
            rect: [self.x0() + x * u, (y + self.dy) * u, w * u, h * u],
            uv,
            c0: col,
            c1: col,
            p: [radius * u, 0.0, 0.0, gfx::KIND_IMAGE],
            e: [0.0; 4],
        });
        true
    }

    /// Draw the whole picture `key` stretched over the band `x`..`x + w` across and `top`..`bottom`
    /// down (units), its edges on whole pixels and hard (no smoothing): tiles of one page drawn
    /// edge to edge meet without a seam.
    pub fn image_tile(&mut self, key: &str, x: f32, top: f32, w: f32, bottom: f32) -> bool {
        let Some(p) = self.picture(key) else { return false };
        let u = self.u;
        let x0 = (self.x0() + x * u).round();
        let x1 = (self.x0() + (x + w) * u).round();
        let y0 = ((top + self.dy) * u).round();
        let y1 = ((bottom + self.dy) * u).round();
        if y1 <= y0 || x1 <= x0 {
            return true;
        }
        let col = self.col(Rgba::WHITE);
        self.insts.push(Inst {
            rect: [x0, y0, x1 - x0, y1 - y0],
            uv: p.uv,
            c0: col,
            c1: col,
            p: [0.0, 0.0, 0.0, gfx::KIND_IMAGE + gfx::FLAG_HARD],
            e: [0.0; 4],
        });
        true
    }

    /// As [`Canvas::image_cover`], keeping the part of the picture at `focus` (0 to 1 across and
    /// down) when it is cropped: (0.5, 0.1) keeps the top, where a studio picture's object sits.
    #[allow(clippy::too_many_arguments)]
    pub fn image_cover_at(&mut self, key: &str, x: f32, y: f32, w: f32, h: f32, radius: f32, tint: Rgba, focus: (f32, f32)) -> bool {
        let Some(p) = self.picture(key) else { return false };
        let [u0, v0, u1, v1] = p.uv;
        let (iw, ih) = (p.w as f32, p.h as f32);
        let (box_a, img_a) = (w / h.max(0.01), iw / ih);
        let (mut a, mut b, mut c, mut d) = (u0, v0, u1, v1);
        if img_a > box_a {
            // Wider than the box: keep the middle across.
            let keep = (u1 - u0) * box_a / img_a;
            a = u0 + (u1 - u0 - keep) * focus.0.clamp(0.0, 1.0);
            c = a + keep;
        } else {
            let keep = (v1 - v0) * img_a / box_a;
            b = v0 + (v1 - v0 - keep) * focus.1.clamp(0.0, 1.0);
            d = b + keep;
        }
        let u = self.u;
        let col = self.col(tint);
        self.insts.push(Inst {
            rect: [self.x0() + x * u, (y + self.dy) * u, w * u, h * u],
            uv: [a, b, c, d],
            c0: col,
            c1: col,
            p: [radius * u, 0.0, 0.0, gfx::KIND_IMAGE],
            e: [0.0; 4],
        });
        true
    }

    // Text

    fn atlas_put(&mut self, w: u32, h: u32, pixels: Vec<u8>) -> [f32; 4] {
        if w == 0 || h == 0 {
            return [0.0; 4];
        }
        if self.shelf_x + w + 1 > gfx::ATLAS_SIZE {
            self.shelf_x = 1;
            self.shelf_y += self.shelf_h + 1;
            self.shelf_h = 0;
        }
        if self.shelf_y + h + 1 > gfx::ATLAS_SIZE {
            // Full within one frame (rare: `begin` starts over early): start over. What this frame
            // queued before now points at reused slots, so one more frame draws it all again.
            self.reset_glyphs();
            self.image_late = true;
        }
        let (x, y) = (self.shelf_x, self.shelf_y);
        self.shelf_x += w + 1;
        self.shelf_h = self.shelf_h.max(h);
        self.uploads.push(AtlasUpload { x, y, w, h, pixels });
        let s = gfx::ATLAS_SIZE as f32;
        [x as f32 / s, y as f32 / s, (x + w) as f32 / s, (y + h) as f32 / s]
    }

    fn glyph(&mut self, font: usize, index: u16, px: u16) -> Glyph {
        if let Some(g) = self.glyphs.get(&(font as u8, index, px)) {
            return *g;
        }
        let (metrics, pixels) = self.fonts[font].rasterize_indexed(index, px as f32);
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        let uv = self.atlas_put(w, h, pixels);
        let g = Glyph {
            uv,
            w: w as f32,
            h: h as f32,
            left: metrics.xmin as f32,
            bottom: metrics.ymin as f32,
            advance: metrics.advance_width,
        };
        self.glyphs.insert((font as u8, index, px), g);
        g
    }

    fn px_size(&self, size: f32) -> u16 {
        (size * self.u).round().clamp(4.0, 400.0) as u16
    }

    /// Width of `s` set in `style`, in units.
    pub fn measure(&mut self, s: &str, style: Text) -> f32 {
        let font = &self.fonts[style.weight as usize];
        let px = self.px_size(style.size) as f32;
        let mut width = 0f32;
        let mut prev: Option<u16> = None;
        for ch in s.chars() {
            let index = font.lookup_glyph_index(ch);
            // A character the faces don't have (CJK in an author's name) is left out, not boxed.
            if index == 0 {
                continue;
            }
            if let Some(p) = prev {
                width += font.horizontal_kern_indexed(p, index, px).unwrap_or(0.0);
            }
            width += font.metrics_indexed(index, px).advance_width;
            prev = Some(index);
        }
        width / self.u
    }

    /// Draw one line of `s` with its line box's top at `top`, from `x` (or centred on it, or
    /// ending at it). Returns the width drawn, in units.
    pub fn text(&mut self, s: &str, x: f32, top: f32, style: Text) -> f32 {
        let width = self.measure(s, style);
        let font = style.weight as usize;
        let px = self.px_size(style.size);
        let u = self.u;
        let start = match style.align {
            Align::Left => x,
            Align::Center => x - width * 0.5,
            Align::Right => x - width,
        };
        let (asc, desc) = self.fonts[font]
            .horizontal_line_metrics(px as f32)
            .map(|m| (m.ascent, m.descent))
            .unwrap_or((px as f32 * 0.9, -(px as f32) * 0.25));
        let line_px = style.lh() * u;
        let baseline = ((top + self.dy) * u + (line_px - (asc - desc)) * 0.5 + asc).round();
        let mut pen = (self.x0() + start * u).round();
        let colour = self.col(style.color);
        let mut prev: Option<u16> = None;
        for ch in s.chars() {
            let index = self.fonts[font].lookup_glyph_index(ch);
            if index == 0 {
                continue;
            }
            if let Some(p) = prev {
                pen += self.fonts[font].horizontal_kern_indexed(p, index, px as f32).unwrap_or(0.0);
            }
            let g = self.glyph(font, index, px);
            if g.w > 0.0 {
                self.insts.push(Inst {
                    rect: [pen + g.left, baseline - (g.bottom + g.h), g.w, g.h],
                    uv: g.uv,
                    c0: colour,
                    c1: colour,
                    p: [0.0, 0.0, 0.0, gfx::KIND_GLYPH],
                    e: [0.0; 4],
                });
            }
            pen += g.advance;
            prev = Some(index);
        }
        width
    }

    /// The faces draw every character of `c` (Geist has no CJK, for one).
    pub fn can_show(&self, c: char) -> bool {
        c.is_whitespace() || self.fonts[0].lookup_glyph_index(c) != 0
    }

    /// `s` cut to fit `max_w`, with an ellipsis when it had to be cut.
    pub fn fit(&mut self, s: &str, max_w: f32, style: Text) -> String {
        if self.measure(s, style) <= max_w {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let cand: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "…";
            if self.measure(&cand, style) <= max_w { lo = mid } else { hi = mid - 1 }
        }
        chars[..lo].iter().collect::<String>().trim_end().to_string() + "…"
    }

    /// One line, cut with an ellipsis to `max_w`. Returns the width drawn.
    pub fn text_fit(&mut self, s: &str, x: f32, top: f32, max_w: f32, style: Text) -> f32 {
        let line = self.fit(s, max_w, style);
        self.text(&line, x, top, style)
    }

    /// `s` broken into lines no wider than `max_w`, at most `max_lines` (the last one cut).
    pub fn wrap(&mut self, s: &str, max_w: f32, style: Text, max_lines: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in s.split(' ') {
            let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if line.is_empty() || self.measure(&cand, style) <= max_w {
                line = cand;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if lines.len() > max_lines.max(1) {
            let rest = lines[max_lines - 1..].join(" ");
            lines.truncate(max_lines - 1);
            let last = self.fit(&rest, max_w, style);
            lines.push(last);
        }
        lines
    }

    /// A wrapped paragraph from `top`. Returns its height.
    pub fn para(&mut self, s: &str, x: f32, top: f32, max_w: f32, style: Text, max_lines: usize) -> f32 {
        let lines = self.wrap(s, max_w, style, max_lines);
        let ax = match style.align {
            Align::Left => x,
            Align::Center => x + max_w * 0.5,
            Align::Right => x + max_w,
        };
        for (i, l) in lines.iter().enumerate() {
            self.text(l, ax, top + i as f32 * style.lh(), style);
        }
        lines.len() as f32 * style.lh()
    }

    /// Height a paragraph would take, without drawing it.
    pub fn para_h(&mut self, s: &str, max_w: f32, style: Text, max_lines: usize) -> f32 {
        self.wrap(s, max_w, style, max_lines).len() as f32 * style.lh()
    }

    // Icons

    /// One of Relay's glyphs, `size` units square, centred on (`cx`, `cy`). The line is about
    /// 1.3 px as drawn, like the other apps: a larger icon gets finer lines, not heavier ones.
    pub fn icon(&mut self, name: &'static str, cx: f32, cy: f32, size: f32, c: Rgba) {
        let px = self.px_size(size);
        let g = match self.icons.get(&(name, px)) {
            Some(g) => *g,
            None => {
                let pixels = raster_icon(name, px as u32, size);
                let uv = self.atlas_put(px as u32, px as u32, pixels);
                let g = Glyph { uv, w: px as f32, h: px as f32, left: 0.0, bottom: 0.0, advance: 0.0 };
                self.icons.insert((name, px), g);
                g
            }
        };
        let u = self.u;
        let x = (self.x0() + (cx - size * 0.5) * u).round();
        let y = ((cy - size * 0.5 + self.dy) * u).round();
        let colour = self.col(c);
        self.insts.push(Inst { rect: [x, y, g.w, g.h], uv: g.uv, c0: colour, c1: colour, p: [0.0, 0.0, 0.0, gfx::KIND_GLYPH], e: [0.0; 4] });
    }
}

/// SVG path data as zeno commands, arcs turned into curves. zeno parses path strings itself, but
/// stops early on some compact arcs.
fn path_commands(d: &str) -> Vec<zeno::Command> {
    use svgtypes::SimplePathSegment as S;
    use zeno::{Command as C, Vector};
    let v = |x: f64, y: f64| Vector::new(x as f32, y as f32);
    svgtypes::SimplifyingPathParser::from(d)
        .filter_map(Result::ok)
        .map(|s| match s {
            S::MoveTo { x, y } => C::MoveTo(v(x, y)),
            S::LineTo { x, y } => C::LineTo(v(x, y)),
            S::CurveTo { x1, y1, x2, y2, x, y } => C::CurveTo(v(x1, y1), v(x2, y2), v(x, y)),
            S::Quadratic { x1, y1, x, y } => C::QuadTo(v(x1, y1), v(x, y)),
            S::ClosePath => C::Close,
        })
        .collect()
}

/// Leaf's own glyphs beside Relay's generated set (`glyphs.rs`, which is regenerated, not edited):
/// the bell filled, for a series whose bell is on, the info key (a ringed i) and no connection (a
/// cloud struck through).
fn own_glyph(name: &str) -> Option<&'static glyphs::Glyph> {
    use glyphs::{Glyph, Shape};
    match name {
        "bell-on" => Some(&Glyph {
            scale: 1.0,
            shapes: &[
                Shape { d: "M6.85 14a1.33 1.33 0 0 0 2.3 0", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M2.17 10.22A.67.67 0 0 0 2.67 11.33h10.67a.67.67 0 0 0 .49-1.12C12.94 9.3 12 8.33 12 5.33a4 4 0 0 0-8 0c0 3-.94 3.97-1.83 4.89z", fill: true, stroke: true, rotate: 0.0 },
            ],
        }),
        "info" => Some(&Glyph {
            scale: 1.0,
            shapes: &[
                Shape { d: "M8 1.67a6.33 6.33 0 1 0 0 12.66a6.33 6.33 0 1 0 0-12.66z", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M8 7.33v3.67", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M8 4.4a.8.8 0 1 0 0 1.6a.8.8 0 1 0 0-1.6z", fill: true, stroke: false, rotate: 0.0 },
            ],
        }),
        // A ribbon bookmark, filled (on the reader's bookmark tag).
        // A filled star: a source's rating.
        "star" => Some(&Glyph {
            scale: 1.0,
            shapes: &[Shape { d: "M8.00 1.60L9.76 6.17L14.66 6.44L10.85 9.53L12.11 14.26L8.00 11.60L3.89 14.26L5.15 9.53L1.34 6.44L6.24 6.17z", fill: true, stroke: true, rotate: 0.0 }],
        }),
        "bookmark" => Some(&Glyph {
            scale: 1.0,
            shapes: &[Shape { d: "M4 1.75h8a.75.75 0 0 1 .75.75v11.5l-4.75-3-4.75 3V2.5A.75.75 0 0 1 4 1.75z", fill: true, stroke: true, rotate: 0.0 }],
        }),
        // No connection: the cloud, broken where a slash crosses it.
        "cloud-off" => Some(&Glyph {
            scale: 1.0,
            shapes: &[
                Shape { d: "M1.75 1.75l12.5 12.5", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M3.9 3.9A4.6 4.6 0 0 0 6 12.5h5.6a3 3 0 0 0 .85-.12", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M14.3 10.9A3 3 0 0 0 11.6 6.6h-1.15A4.6 4.6 0 0 0 6.7 3.45", fill: false, stroke: true, rotate: 0.0 },
            ],
        }),
        // Settings' rows: an open book (Reading), the server's two units, a backup box.
        "book-open" => Some(&Glyph {
            scale: 1.0,
            shapes: &[
                Shape { d: "M8 4.25C6.75 3.25 4.75 2.75 1.75 2.75v9.75c3 0 5 .5 6.25 1.5 1.25-1 3.25-1.5 6.25-1.5V2.75c-3 0-5 .5-6.25 1.5z", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M8 4.25V14", fill: false, stroke: true, rotate: 0.0 },
            ],
        }),
        "server" => Some(&Glyph {
            scale: 1.0,
            shapes: &[
                Shape { d: "M3.25 2.25h9.5a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-9.5a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1z", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M3.25 8.75h9.5a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-9.5a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1z", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M5 4.75h.5M5 11.25h.5", fill: false, stroke: true, rotate: 0.0 },
            ],
        }),
        "archive" => Some(&Glyph {
            scale: 1.0,
            shapes: &[
                Shape { d: "M1.75 2.75h12.5v3H1.75z", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M2.75 5.75v7.5h10.5v-7.5", fill: false, stroke: true, rotate: 0.0 },
                Shape { d: "M6.25 8.5h3.5", fill: false, stroke: true, rotate: 0.0 },
            ],
        }),
        _ => None,
    }
}

/// A glyph as an alpha mask `px` pixels square.
fn raster_icon(name: &str, px: u32, size_units: f32) -> Vec<u8> {
    use zeno::{Angle, Cap, Fill, Join, Mask, Stroke, Transform};
    let g = own_glyph(name).unwrap_or_else(|| glyphs::get(name));
    let n = (px * px) as usize;
    let mut out = vec![0u8; n];
    let s = px as f32 / 16.0 * g.scale;
    // As the Kotlin app: about 1.3 px of line at any size, never heavier than 1.5 grid units.
    let stroke_units = (20.8 / size_units.max(1.0)).min(1.5) / g.scale;
    for shape in g.shapes {
        let mut t = Transform::scale(s, s);
        if shape.rotate != 0.0 {
            let pivot = 8.0 / g.scale;
            t = Transform::rotation_about((pivot, pivot), Angle::from_degrees(shape.rotate)).then(&t);
        }
        for (enabled, fill) in [(shape.fill, true), (shape.stroke, false)] {
            if !enabled {
                continue;
            }
            let mut buf = vec![0u8; n];
            let cmds = path_commands(shape.d);
            let mut m = Mask::new(&cmds);
            m.size(px, px).transform(Some(t));
            if fill {
                m.style(Fill::NonZero);
            } else {
                let mut st = Stroke::new(stroke_units);
                st.cap(Cap::Round).join(Join::Round);
                m.style(st);
            }
            m.render_into(&mut buf, None);
            for (o, b) in out.iter_mut().zip(buf) {
                *o = (*o).max(b);
            }
        }
    }
    out
}

/// Ease-out: fast start, gentle landing. `t` in 0..1.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::{Canvas, ImageAtlas, Rgba, Text, GLYPHS_START_OVER};
    use crate::gfx::ATLAS_SIZE;

    #[test]
    fn a_filling_glyph_atlas_starts_over_between_frames() {
        let mut c = Canvas::new(crate::app::FONTS);
        c.begin(1080.0, 2340.0, 0.0, 0.0);
        c.text("Tally", 0.0, 0.0, Text::new(16.0, Rgba::WHITE));
        assert!(!c.glyphs.is_empty());
        c.shelf_y = GLYPHS_START_OVER + 1;
        c.begin(1080.0, 2340.0, 0.0, 0.0);
        assert!(c.glyphs.is_empty(), "started over at the frame's start");
        assert_eq!(c.shelf_y, 1);
        assert!(!c.take_late_pictures(), "nothing drawn garbled, so no frame more");
    }

    #[test]
    fn a_glyph_atlas_full_mid_frame_asks_for_another_frame() {
        let mut c = Canvas::new(crate::app::FONTS);
        c.begin(1080.0, 2340.0, 0.0, 0.0);
        c.shelf_y = ATLAS_SIZE - 4;
        c.text("Tally", 0.0, 0.0, Text::new(16.0, Rgba::WHITE));
        assert!(c.shelf_y < 100, "started over");
        assert!(c.take_late_pictures(), "text queued before it is drawn again");
    }

    /// The studio pictures' size: twelve fit the 3072 atlas.
    const PW: u32 = 720;
    const PH: u32 = 823;

    fn key(i: usize) -> String {
        format!("p{i}")
    }

    #[test]
    fn a_smaller_atlas_keeps_pictures_at_half_size_inside_its_bounds() {
        // A GLES 3.0 phone that stops at 2048: at full size only four would fit, fewer than
        // Goals shows. At half size twenty do; the rest wait their turn.
        let mut c = Canvas::new(crate::app::FONTS);
        c.set_images_size(2048);
        let px = vec![200u8; (PW * PH * 4) as usize];
        let placed = (0..24).filter(|i| c.add_image(&key(*i), PW, PH, px.clone())).count();
        assert_eq!(placed, 20);
        assert!(c.image_uploads.iter().all(|u| u.x + u.w <= 2048 && u.y + u.h <= 2048));
        assert!(c.image_uploads.iter().all(|u| (u.w, u.h) == (PW / 2, PH / 2) && u.pixels.len() == (u.w * u.h * 4) as usize));
        assert!(c.image_uploads[0].pixels.iter().all(|&b| b == 200), "averaged, not darkened");
        assert!(c.image_waiting(&key(20)));
        // The full atlas keeps them whole.
        let mut full = Canvas::new(crate::app::FONTS);
        assert!(full.add_image("p", PW, PH, px));
        assert_eq!((full.image_uploads[0].w, full.image_uploads[0].h), (PW, PH));
    }

    #[test]
    fn a_full_atlas_evicts_the_picture_drawn_longest_ago() {
        let mut a = ImageAtlas::new(3072);
        a.frame = 1;
        let mut n = 0;
        while a.place(&key(n), PW, PH, 0).is_some() {
            n += 1;
        }
        assert_eq!(n, 12);
        // Frame 10 draws every picture but p3 (last drawn in frame 5).
        a.frame = 10;
        for (k, p) in a.pics.iter_mut() {
            p.used = if k == "p3" { 5 } else { 10 };
        }
        let slot3 = a.pics["p3"].slot;
        assert!(a.place("new", PW, PH, 9).is_some());
        assert!(!a.pics.contains_key("p3"));
        assert_eq!(a.pics["new"].slot, slot3);
        assert_eq!(a.pics.len(), 12);
    }

    #[test]
    fn pictures_on_screen_are_never_evicted() {
        let mut a = ImageAtlas::new(3072);
        a.frame = 1;
        for i in 0..12 {
            assert!(a.place(&key(i), PW, PH, 0).is_some());
        }
        // A thirteenth while all twelve are drawn each frame: it waits; nothing is cleared.
        for frame in 2..50 {
            a.frame = frame;
            for p in a.pics.values_mut() {
                p.used = frame;
            }
            assert!(a.place("p12", PW, PH, frame - 1).is_none());
            assert_eq!(a.pics.len(), 12);
        }
    }

    #[test]
    fn a_short_tile_reserves_a_whole_tile_slot() {
        // A page's last tile is short; its slot is a full tile's, so the next full tile that
        // evicts it fits there.
        let mut a = ImageAtlas::new(3072);
        a.frame = 1;
        let [_, _, w, h] = a.place_in("short", 800, 120, (800, 1020), 0).unwrap();
        assert_eq!((w, h), (800, 120));
        assert_eq!(a.pics["short"].slot[3], 1020);
        let mut n = 0;
        while a.place_in(&format!("t{n}"), 800, 1020, (800, 1020), 0).is_some() {
            n += 1;
        }
        assert_eq!(n, 8, "nine tile slots in all");
        a.frame = 5;
        for p in a.pics.values_mut() {
            p.used = 5;
        }
        a.pics.get_mut("short").unwrap().used = 1;
        assert!(a.place_in("full", 800, 1020, (800, 1020), 4).is_some(), "the short tile's slot takes a full one");
    }

    #[test]
    fn an_odd_size_starts_over_only_when_nothing_is_on_screen() {
        let mut a = ImageAtlas::new(3072);
        a.frame = 1;
        for i in 0..12 {
            a.place(&key(i), PW, PH, 0).unwrap();
        }
        a.place("small", 336, 336, 0).unwrap();
        // Fill the rest of the shelves so a wide picture has nowhere to go.
        let mut i = 100;
        while a.shelf_place(336, 336).is_some() {
            i += 1;
        }
        assert!(i > 100);
        a.frame = 5;
        a.pics.get_mut("p0").unwrap().used = 5;
        assert!(a.place("wide", 960, 552, 4).is_none(), "p0 is on screen");
        a.pics.get_mut("p0").unwrap().used = 1;
        let [x, y, w, h] = a.place("wide", 960, 552, 4).unwrap();
        assert_eq!((x, y, w, h), (1, 1, 960, 552));
        assert_eq!(a.pics.len(), 1);
    }
}
