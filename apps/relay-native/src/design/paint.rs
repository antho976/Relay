//! Drawing a design's nodes as GSK render nodes, in page coordinates: the canvas scales the
//! result by the camera, and exports and gallery thumbnails reuse it at their own scale. Text
//! layouts, stroke paths and pictures are cached per node, so a frame that only pans rebuilds
//! nothing.

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{
    parse_color, strip_controls, Blend, Gradient, GradientKind, ImageFit, Node, NodeKind, Shadow, StrokeCap, StrokeJoin, TextAlign, TextRun, MAX_COORD,
    MAX_SHADOW_BLUR,
};
use relay_client::design::layout::{self, Measure, Placed, STICKY_PAD};
use relay_client::design::shapes::{self, ArcEnd, Seg};
use relay_client::design::{bounds, text, Rect, Scene};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

// The colour rules live in relay-client (client-geometry), where the render job's contrast
// check and the phone's port read them too; the names stay here for the canvas's own files.
pub use relay_client::design::color::{fill_of, luminance, text_color, CANVAS, STICKY_FILL, STROKE};
// Kept for the files other streams own (MODEL-SPEC §5 #11), though nothing reads them now.
#[allow(unused_imports)]
pub use relay_client::design::color::{FRAME_FILL, SHAPE_FILL};

/// What the painter draws, as a number saved thumbnails are keyed by: bump it whenever a change
/// to drawing changes what a thumbnail shows, so the gallery draws them again (I-15).
// The gallery keys its thumbnails with it from native-gallery on.
#[allow(dead_code)]
pub const PAINTER_VERSION: u32 = 2;

pub fn rgba(hex: &str) -> gdk::RGBA {
    let [r, g, b, a] = parse_color(hex).unwrap_or([0, 0, 0, 255]);
    gdk::RGBA::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
}

pub fn grect(r: &Rect) -> graphene::Rect {
    graphene::Rect::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
}

/// The GSK blend a node's `blend` names; `None` for normal.
pub fn blend_mode(n: &Node) -> Option<gsk::BlendMode> {
    Some(match n.blend? {
        Blend::Normal => return None,
        Blend::Multiply => gsk::BlendMode::Multiply,
        Blend::Screen => gsk::BlendMode::Screen,
        Blend::Overlay => gsk::BlendMode::Overlay,
        Blend::Darken => gsk::BlendMode::Darken,
        Blend::Lighten => gsk::BlendMode::Lighten,
        Blend::ColorDodge => gsk::BlendMode::ColorDodge,
        Blend::ColorBurn => gsk::BlendMode::ColorBurn,
        Blend::HardLight => gsk::BlendMode::HardLight,
        Blend::SoftLight => gsk::BlendMode::SoftLight,
        Blend::Difference => gsk::BlendMode::Difference,
        Blend::Exclusion => gsk::BlendMode::Exclusion,
        Blend::Hue => gsk::BlendMode::Hue,
        Blend::Saturation => gsk::BlendMode::Saturation,
        Blend::Color => gsk::BlendMode::Color,
        Blend::Luminosity => gsk::BlendMode::Luminosity,
    })
}

fn weight(w: u16) -> pango::Weight {
    match w {
        0..=149 => pango::Weight::Thin,
        150..=249 => pango::Weight::Ultralight,
        250..=349 => pango::Weight::Light,
        350..=449 => pango::Weight::Normal,
        450..=549 => pango::Weight::Medium,
        550..=649 => pango::Weight::Semibold,
        650..=749 => pango::Weight::Bold,
        750..=849 => pango::Weight::Ultrabold,
        _ => pango::Weight::Heavy,
    }
}

pub fn font_size(n: &Node) -> f64 {
    n.font_size.unwrap_or(16.0)
}

pub fn font_family(n: &Node) -> &str {
    n.font.as_deref().unwrap_or("Geist")
}

pub fn font_description(n: &Node) -> pango::FontDescription {
    let mut d = pango::FontDescription::new();
    // A control character stored before the rule (a NUL) would abort GTK's setter (B43).
    d.set_family(&strip_controls(font_family(n), false));
    d.set_absolute_size(font_size(n) * pango::SCALE as f64);
    d.set_weight(weight(n.font_weight.unwrap_or(400)));
    d.set_style(if n.italic == Some(true) { pango::Style::Italic } else { pango::Style::Normal });
    d
}

/// What a text's look depends on, for the layout caches.
fn text_key(n: &Node, wrap: Option<f64>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    n.text.hash(&mut h);
    n.font.hash(&mut h);
    font_size(n).to_bits().hash(&mut h);
    n.font_weight.hash(&mut h);
    n.italic.hash(&mut h);
    n.underline.hash(&mut h);
    n.strike.hash(&mut h);
    n.align.hash(&mut h);
    n.line_height.map(f64::to_bits).hash(&mut h);
    n.letter_spacing.map(f64::to_bits).hash(&mut h);
    n.text_case.hash(&mut h);
    n.max_lines.hash(&mut h);
    for run in n.runs.iter().flatten() {
        (run.start, run.end, &run.color, &run.font, run.font_weight).hash(&mut h);
        (run.italic, run.underline, run.strike).hash(&mut h);
        run.font_size.map(f64::to_bits).hash(&mut h);
        run.rise.map(f64::to_bits).hash(&mut h);
    }
    wrap.map(|w| (w * 100.0).round() as i64).hash(&mut h);
    h.finish()
}

/// The most px a Pango layout holds: its units are 32-bit, 1024 to the px (B111).
const PANGO_MAX_PX: f64 = (i32::MAX / pango::SCALE) as f64 - 1.0;

/// `px` in Pango units, within what they can hold.
fn pango_units(px: f64) -> i32 {
    (px.clamp(0.0, PANGO_MAX_PX) * pango::SCALE as f64) as i32
}

/// A node's words as Pango is given them: `text::shown` (cased), less any control character
/// but tabs and line breaks (B43: a NUL stored before the rule would abort GTK's setter), and
/// for each stored character the byte its drawn letters start at, moved to match.
fn shown_text(n: &Node) -> (String, Vec<usize>) {
    let (text, offsets) = text::shown(n);
    if matches!(strip_controls(&text, true), Cow::Borrowed(_)) {
        return (text, offsets);
    }
    let mut out = String::with_capacity(text.len());
    let mut moved = vec![0; text.len() + 1];
    let mut buf = [0; 4];
    for (i, c) in text.char_indices() {
        moved[i] = out.len();
        if !strip_controls(c.encode_utf8(&mut buf), true).is_empty() {
            out.push(c);
        }
    }
    moved[text.len()] = out.len();
    let offsets = offsets.into_iter().map(|o| moved[o.min(text.len())]).collect();
    (out, offsets)
}

/// What a run of a text sets of its own, as Pango attributes for its stretch (the caller sets
/// where they start and end): colour (its alpha too), family, size with its line's height
/// (`line_height` times it, so a larger run raises its line as CSS does), weight, italic,
/// underline, strike and rise. A colour that does not parse (a `$name` not yet resolved) is left
/// to the node's.
fn run_attrs(n: &Node, run: &TextRun) -> Vec<pango::Attribute> {
    let units = |px: f64| (px * pango::SCALE as f64).round() as i32;
    let mut out: Vec<pango::Attribute> = Vec::new();
    if let Some([r, g, b, a]) = run.color.as_deref().and_then(parse_color) {
        let wide = |c: u8| c as u16 * 257;
        out.push(pango::AttrColor::new_foreground(wide(r), wide(g), wide(b)).into());
        if a < 255 {
            out.push(pango::AttrInt::new_foreground_alpha(wide(a)).into());
        }
    }
    if let Some(font) = &run.font {
        out.push(pango::AttrString::new_family(&strip_controls(font, false)).into());
    }
    if let Some(px) = run.font_size {
        out.push(pango::AttrSize::new_size_absolute(units(px)).into());
        out.push(pango::AttrInt::new_line_height_absolute(units(n.line_height.unwrap_or(1.3) * px)).into());
    }
    if let Some(w) = run.font_weight {
        out.push(pango::AttrInt::new_weight(weight(w)).into());
    }
    if let Some(italic) = run.italic {
        out.push(pango::AttrInt::new_style(if italic { pango::Style::Italic } else { pango::Style::Normal }).into());
    }
    if let Some(u) = run.underline {
        out.push(pango::AttrInt::new_underline(if u { pango::Underline::Single } else { pango::Underline::None }).into());
    }
    if let Some(strike) = run.strike {
        out.push(pango::AttrInt::new_strikethrough(strike).into());
    }
    if let Some(rise) = run.rise {
        out.push(pango::AttrInt::new_rise(units(rise)).into());
    }
    out
}

/// How tall `n`'s words could get at most, one character a line at its largest size: when this
/// passes what Pango holds, its layout is cut there (B111).
fn tallest(n: &Node) -> f64 {
    let chars = n.text.as_deref().map_or(0, |t| t.chars().count()) as f64;
    let size = n.runs.iter().flatten().filter_map(|r| r.font_size).fold(font_size(n), f64::max);
    (chars + 1.0) * size * n.line_height.unwrap_or(1.3).max(2.0)
}

/// A text's height in px from Pango's (`logical`), or, where that overflowed its 32-bit units
/// (B111), its lines at its own line height, within the model's coordinates.
fn text_height(logical: i32, lines: i32, line: f64) -> f64 {
    let h = logical as f64;
    if h > 0.0 && h < PANGO_MAX_PX {
        h
    } else {
        (lines.max(1) as f64 * line).clamp(1.0, MAX_COORD)
    }
}

/// The longest side a raster picture is decoded at: sharp at twice the 960 px a picture is
/// placed at, a fraction of a full-size photo's memory (B33).
const DECODE_EDGE: i32 = 2048;

#[derive(Clone)]
enum Picture {
    Loading,
    Ready(gdk::Texture),
    Vector(gtk::Svg),
    Failed,
}

/// What a picture file decodes to on the blocking thread.
enum Decoded {
    Raster(gdk::Texture),
    Svg(Vec<u8>),
}

/// The size to decode a raster picture whose long side is `long` px at: `DECODE_EDGE` on its long
/// side when it is longer, else `None` for its own size (a small picture is never blown up).
fn decode_edge(long: i32) -> Option<i32> {
    (long > DECODE_EDGE).then_some(DECODE_EDGE)
}

/// A raster picture upright (its Exif orientation applied, TM-24) and no longer than
/// `DECODE_EDGE` on its long side; a format the pixbuf loaders lack is read by GDK at its size.
fn decode(path: &std::path::Path) -> Result<gdk::Texture, String> {
    use gtk::gdk_pixbuf::Pixbuf;
    let pixbuf = match Pixbuf::file_info(path).map(|(_, w, h)| decode_edge(w.max(h))) {
        Some(Some(edge)) => Pixbuf::from_file_at_scale(path, edge, edge, true),
        Some(None) => Pixbuf::from_file(path),
        None => return gdk::Texture::from_filename(path).map_err(|e| e.to_string()),
    };
    match pixbuf {
        Ok(p) => Ok(gdk::Texture::for_pixbuf(&p.apply_embedded_orientation().unwrap_or(p))),
        Err(_) => gdk::Texture::from_filename(path).map_err(|e| e.to_string()),
    }
}

/// An SVG picture that reads nothing outside itself: no files or addresses it links to, so a
/// picture an agent added cannot reach the disk or the network when it is drawn.
fn svg(bytes: Vec<u8>) -> gtk::Svg {
    let svg = gtk::Svg::new();
    svg.set_features(gtk::SvgFeatures::ANIMATIONS | gtk::SvgFeatures::SYSTEM_RESOURCES | gtk::SvgFeatures::EXTENSIONS);
    svg.load_from_bytes(&glib::Bytes::from_owned(bytes));
    svg
}

/// The picture an image node shows, if it is one.
fn image_src(n: &Node) -> Option<&str> {
    n.src.as_deref().filter(|_| n.kind == NodeKind::Image)
}

/// Call `f` with `root` and every node inside it, without copying them: a walk with its own
/// stack, so a design nested past any depth cannot overflow ours.
fn each_under<'a>(scene: &'a Scene, root: &'a str, mut f: impl FnMut(&'a Node)) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(n) = scene.node(id) {
            f(n);
        }
        stack.extend(scene.children(id).iter().map(String::as_str));
    }
}

/// Keep the pictures `scene` still shows, on any page, and every one still loading (its load
/// lands in the map when it finishes); a picture that failed goes too with `drop_failed`, so it
/// is tried again the next time it is drawn (B33; the render job's choice).
fn prune_pictures(map: &mut HashMap<String, Picture>, scene: &Scene, drop_failed: bool) {
    let mut used = HashSet::new();
    for page in scene.pages() {
        each_under(scene, page, |n| {
            used.extend(image_src(n));
        });
    }
    map.retain(|src, p| match p {
        Picture::Loading => true,
        Picture::Failed if drop_failed => false,
        _ => used.contains(src.as_str()),
    });
}

/// Caches for drawing one design. One per canvas (and one for the gallery's thumbnails).
pub struct Painter {
    ctx: pango::Context,
    layouts: RefCell<HashMap<String, (u64, pango::Layout)>>,
    sizes: RefCell<HashMap<u64, (f64, f64)>>,
    paths: RefCell<HashMap<String, (u64, gsk::Path)>>,
    /// Icon geometry by name, parsed once.
    icons: RefCell<HashMap<String, Option<gsk::Path>>>,
    pictures: RefCell<HashMap<String, Picture>>,
    pub assets_dir: RefCell<String>,
    /// Called on the main loop when a picture finishes loading.
    loaded: RefCell<Option<Rc<dyn Fn()>>>,
    /// The text being edited in place is not drawn: the editor shows it.
    pub editing: RefCell<Option<String>>,
    /// Set while drawing for an SVG or PDF file: pictures and blends are drawn through cairo as
    /// images of their own, which every viewer shows, where a texture or blend node becomes a
    /// filter some draw white (TM-21). Every decoded pixel of a picture is kept (TM-22).
    pub vector: Cell<bool>,
    me: std::rc::Weak<Painter>,
}

impl Painter {
    pub fn new(widget: &impl IsA<gtk::Widget>) -> Rc<Painter> {
        let ctx = widget.as_ref().create_pango_context();
        Rc::new_cyclic(|me| Painter {
            me: me.clone(),
            ctx,
            layouts: RefCell::default(),
            sizes: RefCell::default(),
            paths: RefCell::default(),
            icons: RefCell::default(),
            pictures: RefCell::default(),
            assets_dir: RefCell::default(),
            loaded: RefCell::default(),
            editing: RefCell::default(),
            vector: Cell::default(),
        })
    }

    pub fn on_loaded(&self, f: impl Fn() + 'static) {
        *self.loaded.borrow_mut() = Some(Rc::new(f));
    }

    /// Drop the layouts, paths and pictures of nodes that are gone, so a long session's caches
    /// stay the size of the design.
    pub fn retain(&self, scene: &Scene) {
        self.layouts.borrow_mut().retain(|id, _| scene.node(id).is_some());
        self.paths.borrow_mut().retain(|id, _| scene.node(id).is_some());
        let mut sizes = self.sizes.borrow_mut();
        if sizes.len() > 20_000 {
            sizes.clear();
        }
        self.retain_pictures(scene, false);
    }

    /// Keep only the pictures `scene` shows and those still loading; with `drop_failed`, one
    /// that failed to load is tried again the next time it is drawn.
    pub fn retain_pictures(&self, scene: &Scene, drop_failed: bool) {
        prune_pictures(&mut self.pictures.borrow_mut(), scene, drop_failed);
    }

    fn new_layout(&self, n: &Node, wrap: Option<f64>) -> pango::Layout {
        let layout = pango::Layout::new(&self.ctx);
        layout.set_font_description(Some(&font_description(n)));
        let (text, offsets) = shown_text(n);
        layout.set_text(&text);
        let attrs = pango::AttrList::new();
        if n.underline == Some(true) {
            attrs.insert(pango::AttrInt::new_underline(pango::Underline::Single));
        }
        if n.strike == Some(true) {
            attrs.insert(pango::AttrInt::new_strikethrough(true));
        }
        if let Some(ls) = n.letter_spacing.filter(|v| *v != 0.0) {
            attrs.insert(pango::AttrInt::new_letter_spacing((ls * pango::SCALE as f64) as i32));
        }
        // A multiple of the font size, as the model says, not of the font's own line.
        let line = n.line_height.unwrap_or(1.3) * font_size(n);
        attrs.insert(pango::AttrInt::new_line_height_absolute((line * pango::SCALE as f64).round() as i32));
        // Each run's own look over its stretch, after the node's, so it wins there.
        for (start, end, run) in text::run_bytes(n, &offsets) {
            for mut attr in run_attrs(n, run) {
                attr.set_start_index(start);
                attr.set_end_index(end);
                attrs.insert(attr);
            }
        }
        layout.set_attributes(Some(&attrs));
        // Words that could pass what Pango holds wrap and stop there instead of overflowing it
        // and drawing their last lines anywhere (B111).
        let tall = tallest(n) >= PANGO_MAX_PX;
        if let Some(w) = wrap.or(tall.then_some(PANGO_MAX_PX)) {
            layout.set_width(pango_units(w));
            layout.set_wrap(pango::WrapMode::WordChar);
        }
        layout.set_alignment(match n.align {
            Some(TextAlign::Center) => pango::Alignment::Center,
            Some(TextAlign::Right) => pango::Alignment::Right,
            _ => pango::Alignment::Left,
        });
        if let (Some(max), Some(_)) = (n.max_lines, wrap) {
            cut_lines(&layout, max);
        }
        if tall && layout.height() < 0 {
            layout.set_height(pango_units(PANGO_MAX_PX));
            layout.set_ellipsize(pango::EllipsizeMode::End);
        }
        layout
    }

    /// Whether any picture `scene` shows is still loading; asking starts the loads.
    // The gallery waits on one page's now (E-export-9); kept for a job that draws every page.
    #[allow(dead_code)]
    pub fn loading(&self, scene: &Scene) -> bool {
        let mut any = false;
        for page in scene.pages() {
            any |= self.loading_under(scene, page);
        }
        any
    }

    /// Whether any picture under `root` (itself included) is still loading; asking starts their
    /// loads. A thumbnail of one page waits on that page's pictures alone.
    pub fn loading_under(&self, scene: &Scene, root: &str) -> bool {
        let mut any = false;
        each_under(scene, root, |n| {
            if let Some(src) = image_src(n) {
                any |= matches!(self.picture(src), Picture::Loading);
            }
        });
        any
    }

    /// Whether the canvas draws `id`'s words while they are typed into, rather than leaving them
    /// to the editor: when it, or anything it is in, is turned (B108, E-text-13). The editor
    /// cannot turn, so it then shows its words clear and the canvas draws them where they are.
    pub fn draws_edited(&self, scene: &Scene, id: &str) -> bool {
        turned_within(scene, id)
    }

    /// The font's own line height for `n`, in px: what a line height of 1 means to Pango.
    pub fn natural_line(&self, n: &Node) -> f64 {
        let metrics = self.ctx.metrics(Some(&font_description(n)), None);
        (metrics.height() as f64 / pango::SCALE as f64).max(1.0)
    }

    /// The node's laid-out text, cached until its words or style change.
    fn layout(&self, n: &Node, wrap: Option<f64>) -> pango::Layout {
        let key = text_key(n, wrap);
        if let Some((k, layout)) = self.layouts.borrow().get(&n.id) {
            if *k == key {
                return layout.clone();
            }
        }
        let layout = self.new_layout(n, wrap);
        self.layouts.borrow_mut().insert(n.id.clone(), (key, layout.clone()));
        layout
    }

    fn picture(&self, src: &str) -> Picture {
        if let Some(p) = self.pictures.borrow().get(src) {
            return p.clone();
        }
        self.pictures.borrow_mut().insert(src.to_string(), Picture::Loading);
        let path = std::path::Path::new(&*self.assets_dir.borrow()).join(src);
        let src = src.to_string();
        let loaded = self.loaded.borrow().clone();
        let painter_ptr = self.me.clone();
        glib::spawn_future_local(async move {
            let is_svg = src.ends_with(".svg");
            let read = gtk::gio::spawn_blocking(move || -> Result<Decoded, String> {
                if is_svg {
                    std::fs::read(&path).map(Decoded::Svg).map_err(|e| e.to_string())
                } else {
                    decode(&path).map(Decoded::Raster)
                }
            })
            .await;
            let picture = match read {
                Ok(Ok(Decoded::Raster(texture))) => Picture::Ready(texture),
                Ok(Ok(Decoded::Svg(bytes))) => Picture::Vector(svg(bytes)),
                _ => Picture::Failed,
            };
            if let Some(painter) = painter_ptr.upgrade() {
                painter.pictures.borrow_mut().insert(src, picture);
            }
            if let Some(f) = loaded {
                f();
            }
        });
        Picture::Loading
    }

    /// A free line's or path's stroke as `shapes::path_segments` describes it (hit testing
    /// flattens the same pieces, so a click lands on the curve drawn, B118), in its own box's
    /// coordinates: its points are narrowed to f32 there, not on the page, so a drawing far from
    /// the origin keeps its shape (B110), and moving it reuses the cached path.
    fn path(&self, n: &Node) -> Option<gsk::Path> {
        let points = n.points.as_deref()?;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in points {
            p[0].to_bits().hash(&mut h);
            p[1].to_bits().hash(&mut h);
        }
        (n.kind == NodeKind::Path).hash(&mut h);
        n.smooth.hash(&mut h);
        shapes::closed_shape(n).hash(&mut h);
        let key = h.finish();
        if let Some((k, path)) = self.paths.borrow().get(&n.id) {
            if *k == key {
                return Some(path.clone());
            }
        }
        let path = stroke_path(n)?;
        self.paths.borrow_mut().insert(n.id.clone(), (key, path.clone()));
        Some(path)
    }

    /// Paint `page`'s content: its children, in page coordinates. `view` culls what lies
    /// wholly outside it.
    pub fn paint_page(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, page: &str, view: Option<&Rect>) {
        self.paint_children(snap, scene, placed, page, view, None);
    }

    /// Paint `page`'s content as it shows on `ground`, the page's fill, which the caller paints
    /// under it: a top-level node with a `blend` mixes with the ground and what is under it,
    /// as a frame's children mix with its fill (B29, B30). The ground goes into what this
    /// paints only behind those nodes (their painted bounds), so whatever the caller shows
    /// between the nodes, as the canvas's grid, stays seen; without a ground, or with nothing
    /// blending at the top, this is `paint_page`.
    // The board, the gallery and export switch to it in native-canvas, native-gallery and
    // native-export (MODEL-SPEC §6.4).
    #[allow(dead_code)]
    pub fn paint_page_on(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, page: &str, view: Option<&Rect>, ground: Option<&str>) {
        let fill = ground.filter(|g| parse_color(g).is_some_and(|c| c[3] > 0)).map(rgba);
        let behind = match fill {
            Some(_) => blend_region(scene, placed, self, page),
            None => Vec::new(),
        };
        let (Some(fill), false) = (fill, behind.is_empty()) else {
            return self.paint_children(snap, scene, placed, page, view, None);
        };
        let ground = |s: &gtk::Snapshot| match behind.as_slice() {
            [one] => s.append_color(&fill, &grect(one)),
            many => {
                // One fill of all their boxes, so a see-through ground is not doubled where they overlap.
                let b = gsk::PathBuilder::new();
                for r in many {
                    b.add_rect(&grect(r));
                }
                s.append_fill(&b.to_path(), gsk::FillRule::Winding, &fill);
            }
        };
        self.paint_children(snap, scene, placed, page, view, Some(&ground));
    }

    /// Paint `parent`'s children in order. A child with a `blend` mixes with what is painted
    /// before it in the same parent (and `ground`, the parent's own fill, under them all). For a
    /// vector file (`vector`), what they draw stays vectors, and each blended child is a picture
    /// of the blend over what is under it, cut to where it draws, which every viewer shows.
    fn paint_children(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, parent: &str, view: Option<&Rect>, ground: Option<&dyn Fn(&gtk::Snapshot)>) {
        let kids = scene.children(parent);
        let blends = |id: &String| scene.node(id).is_some_and(|n| !n.is_hidden() && blend_mode(n).is_some());
        if !kids.iter().any(blends) {
            if let Some(ground) = ground {
                ground(snap);
            }
            for child in kids {
                self.paint(snap, scene, placed, child, view);
            }
            return;
        }
        let vector = self.vector.get();
        let mut acc = gtk::Snapshot::new();
        if let Some(ground) = ground {
            ground(&acc);
            if vector {
                ground(snap);
            }
        }
        for child in kids {
            match scene.node(child).filter(|n| !n.is_hidden()).and_then(blend_mode) {
                Some(mode) => {
                    let below = acc.to_node();
                    acc = gtk::Snapshot::new();
                    acc.push_blend(mode);
                    if let Some(below) = &below {
                        acc.append_node(below);
                    }
                    acc.pop();
                    self.paint(&acc, scene, placed, child, view);
                    acc.pop();
                    if vector {
                        let mixed = acc.to_node();
                        acc = gtk::Snapshot::new();
                        if let Some(mixed) = &mixed {
                            acc.append_node(mixed);
                            if let Some(area) = bounds::painted_bounds(scene, placed, self, child) {
                                raster_on_cairo(snap, mixed, &area);
                            }
                        }
                    }
                }
                None if vector => {
                    let one = gtk::Snapshot::new();
                    self.paint(&one, scene, placed, child, view);
                    if let Some(node) = one.to_node() {
                        acc.append_node(&node);
                        snap.append_node(&node);
                    }
                }
                None => self.paint(&acc, scene, placed, child, view),
            }
        }
        if !vector {
            if let Some(node) = acc.to_node() {
                snap.append_node(&node);
            }
        }
    }

    /// Paint one node and what is inside it.
    pub fn paint(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, id: &str, view: Option<&Rect>) {
        let Some(n) = scene.node(id).filter(|n| !n.is_hidden()) else {
            return;
        };
        let Some(r) = placed.rect(id) else {
            return;
        };
        let clips = n.kind == NodeKind::Frame && n.clip != Some(false);
        let turn = shapes::rotation(n);
        // How far what it draws reaches past its box: shadows, ring caps, a line's stroke and
        // heads, blur, and a margin for words that overflow a little.
        let stroke = match n.kind {
            NodeKind::Line | NodeKind::Path => shapes::arrow_size(n).max(n.stroke_width.unwrap_or(2.0) * 2.0),
            _ => n.stroke_width.unwrap_or(2.0),
        };
        let reach = shapes::shadow_reach(n) + shapes::cap_reach(n, &r) + stroke.max(0.0) + n.blur.unwrap_or(0.0).max(0.0) * 2.0 + 16.0;
        // A connector runs wherever its ends are, not inside its stored box.
        let connector = n.kind == NodeKind::Line && (n.from.is_some() || n.to.is_some());
        let culls = (!n.kind.holds_children() || clips) && !connector;
        if culls && view.is_some_and(|v| !v.intersects(&shapes::rotated_bounds(&r, turn).inflate(reach))) {
            return;
        }
        let opacity = n.opacity.unwrap_or(1.0);
        if opacity <= 0.0 {
            return;
        }
        if opacity < 1.0 {
            snap.push_opacity(opacity);
        }
        if turn != 0.0 {
            snap.save();
            snap.translate(&graphene::Point::new(r.cx() as f32, r.cy() as f32));
            snap.rotate(turn as f32);
            snap.translate(&graphene::Point::new(-r.cx() as f32, -r.cy() as f32));
        }
        let blur = n.blur.filter(|b| *b > 0.0);
        if let Some(b) = blur {
            snap.push_blur(b);
        }
        // The words being typed are the editor's, unless it cannot show them where they are drawn.
        let editing = self.editing.borrow().as_deref() == Some(id) && !self.draws_edited(scene, id);
        match n.kind {
            NodeKind::Page => {}
            NodeKind::Group => self.paint_children(snap, scene, placed, id, view, None),
            NodeKind::Frame => {
                let rr = outline(n, &r);
                let shadows = shapes::effective_shadows(n);
                self.shadow(snap, &shadows, &rr);
                if clips {
                    snap.push_rounded_clip(&rr);
                }
                // Its inner shadow lies over its fill and under its children, as in CSS (B93):
                // both are the ground a blended child mixes with.
                let ground = |s: &gtk::Snapshot| {
                    self.fill(s, n, &rr);
                    self.inner_shadow(s, &shadows, &rr);
                };
                self.paint_children(snap, scene, placed, id, view, Some(&ground));
                if clips {
                    snap.pop();
                }
                self.border(snap, n, &r);
            }
            NodeKind::Ellipse if shapes::arc(n).is_some() => {
                self.shape_at(snap, n, &r, arc_path);
                if !editing {
                    self.shape_text(snap, scene, n, &r);
                }
            }
            NodeKind::Rect | NodeKind::Sticky | NodeKind::Ellipse => {
                // A sticky's own shadow, unless it names one, is among its effective shadows.
                let rr = outline(n, &r);
                self.box_paint(snap, n, &rr);
                self.border(snap, n, &r);
                if n.kind == NodeKind::Sticky {
                    if !editing {
                        let inner = Rect::new(r.x + STICKY_PAD, r.y + STICKY_PAD, (r.w - 2.0 * STICKY_PAD).max(1.0), r.h);
                        self.text(snap, scene, n, &inner, layout::wrap_width(n, &r));
                    }
                } else if !editing {
                    self.shape_text(snap, scene, n, &r);
                }
            }
            NodeKind::Polygon => {
                self.shape_at(snap, n, &r, polygon_path);
                if !editing {
                    self.shape_text(snap, scene, n, &r);
                }
            }
            NodeKind::Icon => {
                // Cast outside the flip, so its shadows stay where the light puts them (B120).
                let cast = self.cast(snap, n);
                let flipped = flip_about(snap, n, &r);
                self.icon(snap, scene, n, &r);
                if flipped {
                    snap.restore();
                }
                if cast {
                    snap.pop();
                }
            }
            NodeKind::Text => {
                // Its fill box stays while it is typed into (B94); the editor shows the words.
                if let Some(fill) = &n.fill {
                    snap.append_color(&rgba(fill), &grect(&r));
                }
                if !editing {
                    // Its letters cast its shadows, not its fill box (CSS's text-shadow).
                    let cast = self.cast(snap, n);
                    self.text(snap, scene, n, &r, layout::wrap_width(n, &r));
                    if cast {
                        snap.pop();
                    }
                }
            }
            NodeKind::Line | NodeKind::Path => {
                // Fill, stroke and heads glow together.
                let cast = self.cast(snap, n);
                self.stroke(snap, scene, placed, n, &r);
                if cast {
                    snap.pop();
                }
            }
            NodeKind::Image => self.image(snap, n, &r),
        }
        if blur.is_some() {
            snap.pop();
        }
        if turn != 0.0 {
            snap.restore();
        }
        if opacity < 1.0 {
            snap.pop();
        }
    }

    /// Shadow, fill (a colour or a gradient) and inner shadow, inside the rounded outline.
    fn box_paint(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        let shadows = shapes::effective_shadows(n);
        self.shadow(snap, &shadows, rr);
        self.fill(snap, n, rr);
        self.inner_shadow(snap, &shadows, rr);
    }

    /// The drop shadows under the rounded outline, in order (the last on top), of a node's
    /// effective shadows (`shapes::effective_shadows`: a sticky's own when it names none, none
    /// for one wholly clear, B37).
    fn shadow(&self, snap: &gtk::Snapshot, shadows: &[Shadow], rr: &gsk::RoundedRect) {
        for s in shadows.iter().filter(|s| !s.inset) {
            snap.append_outset_shadow(rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, blur_of(s) as f32);
        }
    }

    /// The inner shadows inside the rounded outline, in order.
    fn inner_shadow(&self, snap: &gtk::Snapshot, shadows: &[Shadow], rr: &gsk::RoundedRect) {
        for s in shadows.iter().filter(|s| s.inset) {
            snap.append_inset_shadow(rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, blur_of(s) as f32);
        }
    }

    /// Push the shadows a text, icon, line or path casts round its letters or strokes (B123):
    /// outer ones, without spread, as `effective_shadows` leaves them for these kinds; a glow is
    /// one with no offset. True when it pushed, so the caller pops once it has drawn.
    fn cast(&self, snap: &gtk::Snapshot, n: &Node) -> bool {
        let shadows: Vec<gsk::Shadow> = shapes::effective_shadows(n)
            .iter()
            .filter(|s| !s.inset)
            .map(|s| gsk::Shadow::new(rgba(&s.color), s.x as f32, s.y as f32, blur_of(s) as f32))
            .collect();
        if shadows.is_empty() {
            return false;
        }
        snap.push_shadow(&shadows);
        true
    }

    /// A polygon or an arc in its box `r`: its outline built in the box's own coordinates (at
    /// 0, 0) and drawn from the box's corner, so its corners are narrowed to f32 after the
    /// corner is taken off (B110).
    fn shape_at(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect, outline: fn(&Node, &Rect) -> gsk::Path) {
        let own = Rect::new(0.0, 0.0, r.w, r.h);
        snap.save();
        snap.translate(&graphene::Point::new(r.x as f32, r.y as f32));
        self.shape(snap, n, &outline(n, &own), &own);
        snap.restore();
    }

    /// A shape drawn from a path (a polygon, an arc): its outer shadows, fill, inner shadows and
    /// stroke, the stroke inside the edge as a box's border is.
    fn shape(&self, snap: &gtk::Snapshot, n: &Node, path: &gsk::Path, r: &Rect) {
        let shadows = shapes::effective_shadows(n);
        for s in shadows.iter().filter(|s| !s.inset) {
            path_shadow(snap, path, s, self.vector.get());
        }
        self.fill_path(snap, n, path, r);
        for s in shadows.iter().filter(|s| s.inset) {
            path_inset_shadow(snap, path, s, r, self.vector.get());
        }
        let w = n.stroke_width.unwrap_or(1.0);
        if w > 0.0 && (n.stroke.is_some() || n.stroke_gradient.is_some()) {
            // Twice as wide, clipped to the inside.
            snap.push_fill(path, gsk::FillRule::Winding);
            self.stroke_with(snap, n, path, &stroke_of(n, w * 2.0), n.stroke.as_deref().map(rgba).as_ref(), Some(r));
            snap.pop();
        }
    }

    /// Stroke `path` with `pen`: in `n`'s `stroke_gradient` laid over `bounds` (the node's box,
    /// else the stroke's own bounds, so a level line keeps a height), else in `color`.
    fn stroke_with(&self, snap: &gtk::Snapshot, n: &Node, path: &gsk::Path, pen: &gsk::Stroke, color: Option<&gdk::RGBA>, bounds: Option<&Rect>) {
        match &n.stroke_gradient {
            Some(g) => {
                let Some(area) = bounds.copied().or_else(|| path.stroke_bounds(pen).map(|b| rect_of(&b))) else {
                    return;
                };
                snap.push_stroke(path, pen);
                gradient(snap, g, &area, self.vector.get());
                snap.pop();
            }
            None => {
                if let Some(c) = color {
                    snap.append_stroke(path, pen, c);
                }
            }
        }
    }

    fn fill_path(&self, snap: &gtk::Snapshot, n: &Node, path: &gsk::Path, r: &Rect) {
        if let Some(g) = &n.gradient {
            snap.push_fill(path, gsk::FillRule::Winding);
            gradient(snap, g, r, self.vector.get());
            snap.pop();
        } else if let Some(fill) = fill_of(n).filter(|f| parse_color(f).is_some_and(|c| c[3] > 0)) {
            snap.append_fill(path, gsk::FillRule::Winding, &rgba(&fill));
        }
    }

    /// The fill alone (a colour or a gradient), inside the rounded outline.
    fn fill(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        if let Some(g) = &n.gradient {
            snap.push_rounded_clip(rr);
            let b = rr.bounds();
            gradient(snap, g, &rect_of(b), self.vector.get());
            snap.pop();
        } else if let Some(fill) = fill_of(n).filter(|f| parse_color(f).is_some_and(|c| c[3] > 0)) {
            snap.push_rounded_clip(rr);
            snap.append_color(&rgba(&fill), rr.bounds());
            snap.pop();
        }
    }

    /// The outline drawn inside the edge; dashed or in a gradient, as a stroke along the edge
    /// inset by half its width.
    fn border(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        if n.stroke.is_none() && n.stroke_gradient.is_none() {
            return;
        }
        let w = n.stroke_width.unwrap_or(1.0);
        if w <= 0.0 {
            return;
        }
        let c = n.stroke.as_deref().map(rgba);
        if n.dash.is_some() || n.stroke_gradient.is_some() {
            // Along the middle of the border, its corners following the fill's.
            let half = (w / 2.0).min(r.w / 2.0).min(r.h / 2.0) as f32;
            let mut inset = outline(n, r);
            inset.shrink(half, half, half, half);
            let b = gsk::PathBuilder::new();
            b.add_rounded_rect(&inset);
            let pen = stroke_of(n, w);
            if n.dash.is_none() {
                // Square corners, as a solid border's.
                pen.set_line_join(gsk::LineJoin::Miter);
            }
            self.stroke_with(snap, n, &b.to_path(), &pen, c.as_ref(), Some(r));
        } else if let Some(c) = c {
            snap.append_border(&outline(n, r), &[w as f32; 4], &[c, c, c, c]);
        }
    }

    fn text(&self, snap: &gtk::Snapshot, scene: &Scene, n: &Node, r: &Rect, wrap: Option<f64>) {
        if n.text.as_deref().is_none_or(str::is_empty) {
            return;
        }
        let layout = self.layout(n, wrap);
        snap.save();
        snap.translate(&graphene::Point::new(r.x as f32, r.y as f32));
        snap.append_layout(&layout, &rgba(&text_color(scene, n)));
        snap.restore();
    }

    /// A shape's label, centred both ways inside it.
    fn shape_text(&self, snap: &gtk::Snapshot, scene: &Scene, n: &Node, r: &Rect) {
        let Some(inner) = shape_text_box(self, n, r) else {
            return;
        };
        let mut centred = n.clone();
        centred.align = Some(n.align.unwrap_or(TextAlign::Center));
        self.text(snap, scene, &centred, &inner, Some(inner.w));
    }

    /// An icon from the set, scaled to its box and stroked in its colour.
    fn icon(&self, snap: &gtk::Snapshot, scene: &Scene, n: &Node, r: &Rect) {
        let name = n.icon.as_deref().unwrap_or("");
        let path = {
            let mut cache = self.icons.borrow_mut();
            if !cache.contains_key(name) {
                let parsed = relay_client::design::icons::path(name).and_then(|d| gsk::Path::parse(d).ok());
                cache.insert(name.to_string(), parsed);
            }
            cache.get(name).cloned().flatten()
        };
        let color = rgba(&n.stroke.clone().unwrap_or_else(|| text_color(scene, n)));
        let Some(path) = path else {
            snap.append_color(&gdk::RGBA::new(0.55, 0.55, 0.55, 0.3), &grect(r));
            return;
        };
        let width = n.stroke_width.unwrap_or(2.0);
        if width <= 0.0 {
            return;
        }
        let s = (r.w.min(r.h) / 24.0).max(0.01);
        snap.save();
        snap.translate(&graphene::Point::new((r.cx() - 12.0 * s) as f32, (r.cy() - 12.0 * s) as f32));
        snap.scale(s as f32, s as f32);
        // Weight and dashes in the icon's own 24-unit grid, so they keep its proportions (B96).
        snap.append_stroke(&path, &stroke_of(n, width), &color);
        snap.restore();
    }

    /// A line or path: its fill (three or more free points), its stroke and its arrowheads
    /// (`shapes::arrowheads`, which hit testing takes too), drawn from its box's corner `r`, where
    /// its points count from. A weight of 0 draws no stroke and no heads (B97); a closed shape
    /// strokes inside its edge as a polygon does (B95).
    fn stroke(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, n: &Node, r: &Rect) {
        // Page points narrowed to f32 once the corner is taken off (B110).
        let local = |p: (f64, f64)| ((p.0 - r.x) as f32, (p.1 - r.y) as f32);
        let path = if shapes::routed(n) {
            let (pts, curved) = shapes::line_points(scene, placed, n);
            if pts.len() < 2 {
                return;
            }
            let b = gsk::PathBuilder::new();
            let (x, y) = local(pts[0]);
            b.move_to(x, y);
            if curved {
                let [(ax, ay), (bx, by), (cx, cy)] = [local(pts[1]), local(pts[2]), local(pts[3])];
                b.cubic_to(ax, ay, bx, by, cx, cy);
            } else {
                for p in &pts[1..] {
                    let (x, y) = local(*p);
                    b.line_to(x, y);
                }
            }
            b.to_path()
        } else {
            let Some(path) = self.path(n) else {
                return;
            };
            path
        };
        snap.save();
        snap.translate(&graphene::Point::new(r.x as f32, r.y as f32));
        if shapes::fillable_line(n) && (n.fill.is_some() || n.gradient.is_some()) {
            // A gradient spans the box, or the points where an agent left the box empty.
            let own = Rect::new(0.0, 0.0, r.w, r.h);
            let area = if r.w > 0.0 && r.h > 0.0 { Some(own) } else { path.bounds().map(|b| rect_of(&b)) };
            self.fill_path(snap, n, &path, &area.unwrap_or(own));
        }
        let width = n.stroke_width.unwrap_or(2.0);
        let color = rgba(n.stroke.as_deref().unwrap_or(STROKE));
        let shown = n.stroke_gradient.is_some() || n.stroke.as_deref().is_none_or(|s| parse_color(s).is_some_and(|c| c[3] > 0));
        if width > 0.0 && shown {
            if shapes::closed_shape(n) {
                snap.push_fill(&path, gsk::FillRule::Winding);
                self.stroke_with(snap, n, &path, &stroke_of(n, width * 2.0), Some(&color), None);
                snap.pop();
            } else {
                let pen = stroke_of(n, width);
                let heads: Vec<gsk::Path> = shapes::arrowheads(scene, placed, n, r)
                    .into_iter()
                    .map(|[tip, a, b]| {
                        let p = gsk::PathBuilder::new();
                        let [(tx, ty), (ax, ay), (bx, by)] = [local(tip), local(a), local(b)];
                        p.move_to(tx, ty);
                        p.line_to(ax, ay);
                        p.line_to(bx, by);
                        p.close();
                        p.to_path()
                    })
                    .collect();
                // One gradient over the line and its heads, so a head carries the colour on.
                let area = path.stroke_bounds(&pen).map(|b| heads.iter().filter_map(gsk::Path::bounds).fold(rect_of(&b), |a, h| a.union(&rect_of(&h))));
                self.stroke_with(snap, n, &path, &pen, Some(&color), area.as_ref());
                for head in &heads {
                    match (&n.stroke_gradient, &area) {
                        (Some(g), Some(area)) => {
                            snap.push_fill(head, gsk::FillRule::Winding);
                            gradient(snap, g, area, self.vector.get());
                            snap.pop();
                        }
                        _ => snap.append_fill(head, gsk::FillRule::Winding, &color),
                    }
                }
            }
        }
        snap.restore();
    }

    /// A picture in its rounded box: its drop shadows, the picture (mirrored as it is
    /// flipped), its inner shadows over the picture (B31) and its border. Only the picture
    /// mirrors: its corners, shadows and border stay where they are drawn (B120).
    fn image(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        let rr = outline(n, r);
        let shadows = shapes::effective_shadows(n);
        self.shadow(snap, &shadows, &rr);
        snap.push_rounded_clip(&rr);
        let picture = n.src.as_deref().map(|s| self.picture(s)).unwrap_or(Picture::Failed);
        let natural = match &picture {
            Picture::Ready(t) => Some((t.width() as f64, t.height() as f64)),
            Picture::Vector(svg) => Some((svg.intrinsic_width().max(1) as f64, svg.intrinsic_height().max(1) as f64)),
            _ => None,
        };
        match natural {
            Some((tw, th)) => {
                let (sx, sy) = (r.w / tw.max(1.0), r.h / th.max(1.0));
                let target = match n.fit.unwrap_or(ImageFit::Cover) {
                    ImageFit::Stretch => *r,
                    fit => {
                        let s = if fit == ImageFit::Cover { sx.max(sy) } else { sx.min(sy) };
                        let (w, h) = (tw * s, th * s);
                        Rect::new(r.cx() - w / 2.0, r.cy() - h / 2.0, w, h)
                    }
                };
                // The fit is centred on the box, so it mirrors about the box's centre.
                let flipped = flip_about(snap, n, r);
                match &picture {
                    Picture::Ready(t) if self.vector.get() => texture_on_cairo(snap, t, &target),
                    Picture::Ready(t) => snap.append_scaled_texture(t, gsk::ScalingFilter::Trilinear, &grect(&target)),
                    Picture::Vector(svg) => {
                        snap.save();
                        snap.translate(&graphene::Point::new(target.x as f32, target.y as f32));
                        svg.snapshot(snap.upcast_ref::<gdk::Snapshot>(), target.w, target.h);
                        snap.restore();
                    }
                    _ => {}
                }
                if flipped {
                    snap.restore();
                }
            }
            None => {
                // Loading or missing: a quiet tile with a picture glyph.
                snap.append_color(&gdk::RGBA::new(0.55, 0.55, 0.55, 0.25), &grect(r));
                let s = (r.w.min(r.h) * 0.25).clamp(6.0, 48.0);
                let b = gsk::PathBuilder::new();
                let (cx, cy) = (r.cx() as f32, r.cy() as f32);
                let s = s as f32;
                b.add_rect(&graphene::Rect::new(cx - s, cy - s * 0.75, s * 2.0, s * 1.5));
                b.move_to(cx - s, cy + s * 0.5);
                b.line_to(cx - s * 0.2, cy - s * 0.2);
                b.line_to(cx + s * 0.3, cy + s * 0.3);
                b.line_to(cx + s * 0.6, cy);
                b.line_to(cx + s, cy + s * 0.4);
                let stroke = gsk::Stroke::new((s / 8.0).max(1.0));
                stroke.set_line_join(gsk::LineJoin::Round);
                snap.append_stroke(&b.to_path(), &stroke, &gdk::RGBA::new(0.55, 0.55, 0.55, 0.8));
            }
        }
        snap.pop();
        self.inner_shadow(snap, &shadows, &rr);
        self.border(snap, n, r);
    }
}

/// Show at most `max` lines of `layout` (it has a wrap width), the last one shown ending in "…"
/// when more were cut (W60). Pango reads a negative height per paragraph, which is wrong for
/// words with line breaks, so the height is the bottom of line `max`, found by a first pass, and
/// ellipsizing is set after it (with the default height it would cut every paragraph to a line).
fn cut_lines(layout: &pango::Layout, max: u16) {
    let max = i32::from(max.max(1));
    if layout.line_count() <= max {
        return;
    }
    let mut lines = layout.iter();
    for _ in 1..max {
        if !lines.next_line() {
            return;
        }
    }
    let (_, bottom) = lines.line_yrange();
    layout.set_height(bottom.saturating_add(pango::SCALE / 2));
    layout.set_ellipsize(pango::EllipsizeMode::End);
}

/// Whether `id` or anything it is in is turned (by other than a whole number of turns).
fn turned_within(scene: &Scene, id: &str) -> bool {
    std::iter::once(id).chain(scene.ancestors(id)).filter_map(|a| scene.node(a)).any(|n| shapes::rotation(n).rem_euclid(360.0) != 0.0)
}

/// The painted bounds of `page`'s top-level nodes that blend: where `paint_page_on` lays its
/// ground.
fn blend_region(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str) -> Vec<Rect> {
    scene
        .children(page)
        .iter()
        .filter(|id| scene.node(id).is_some_and(|n| !n.is_hidden() && blend_mode(n).is_some()))
        .filter_map(|id| bounds::painted_bounds(scene, placed, measure, id))
        .collect()
}

/// An image surface holding `texture`'s pixels, at its size.
fn surface_of(texture: &gdk::Texture) -> Option<cairo::ImageSurface> {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, texture.width(), texture.height()).ok()?;
    let stride = surface.stride() as usize;
    // GDK's default download is cairo's ARGB32: premultiplied, in the machine's order.
    texture.download(&mut surface.data().ok()?, stride);
    surface.mark_dirty();
    Some(surface)
}

/// Paint `surface` filling `target` through a cairo node, unrepeated.
fn surface_on_cairo(snap: &gtk::Snapshot, surface: &cairo::ImageSurface, target: &Rect) {
    let (w, h) = (surface.width().max(1) as f64, surface.height().max(1) as f64);
    let cr = snap.append_cairo(&grect(target));
    cr.translate(target.x, target.y);
    cr.scale(target.w / w, target.h / h);
    let pattern = cairo::SurfacePattern::create(surface);
    pattern.set_extend(cairo::Extend::None);
    pattern.set_filter(cairo::Filter::Good);
    if cr.set_source(&pattern).is_ok() {
        let _ = cr.paint();
    }
}

/// A picture drawn through cairo at the size it was decoded, scaled into `target`: for the SVG
/// and PDF files, an image element of its own (TM-21) with every pixel it has (TM-22).
fn texture_on_cairo(snap: &gtk::Snapshot, texture: &gdk::Texture, target: &Rect) {
    match surface_of(texture) {
        Some(surface) => surface_on_cairo(snap, &surface, target),
        None => snap.append_scaled_texture(texture, gsk::ScalingFilter::Trilinear, &grect(target)),
    }
}

/// Draw with `draw`, or for a vector file (`vector`) a picture of what it draws (inside `area`,
/// else all of it): what cairo can carry into an SVG only as a filter on an element, which some
/// viewers draw white (TM-21), as a conic gradient or a mask.
fn pictured(snap: &gtk::Snapshot, vector: bool, area: Option<&Rect>, draw: impl FnOnce(&gtk::Snapshot)) {
    if !vector {
        return draw(snap);
    }
    let one = gtk::Snapshot::new();
    draw(&one);
    if let Some(node) = one.to_node() {
        let area = area.copied().unwrap_or_else(|| rect_of(&node.bounds()));
        raster_on_cairo(snap, &node, &area);
    }
}

/// What `node` shows inside `area`, pictured at twice its size (at most 8192 px a side) and drawn
/// there through cairo: what a vector file cannot carry as itself, a blend.
fn raster_on_cairo(snap: &gtk::Snapshot, node: &gsk::RenderNode, area: &Rect) {
    let b = rect_of(&node.bounds());
    let (x0, y0) = (b.x.max(area.x).floor(), b.y.max(area.y).floor());
    let (x1, y1) = (b.right().min(area.right()).ceil(), b.bottom().min(area.bottom()).ceil());
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let b = Rect::new(x0, y0, x1 - x0, y1 - y0);
    let k = 2.0f64.min(8192.0 / b.w.max(b.h).max(1.0));
    let (w, h) = ((b.w * k).ceil().max(1.0) as i32, (b.h * k).ceil().max(1.0) as i32);
    let Ok(surface) = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h) else {
        return;
    };
    let Ok(cr) = cairo::Context::new(&surface) else {
        return;
    };
    cr.scale(k, k);
    cr.translate(-b.x, -b.y);
    node.draw(&cr);
    drop(cr);
    surface.flush();
    surface_on_cairo(snap, &surface, &Rect::new(b.x, b.y, w as f64 / k, h as f64 / k));
}

/// A shadow's blur as painted: within 0 to 200, as the node's own blur, whatever an older build
/// stored (B72; a blur node's cost grows with its radius).
fn blur_of(s: &Shadow) -> f64 {
    if s.blur.is_finite() {
        s.blur.clamp(0.0, MAX_SHADOW_BLUR)
    } else {
        0.0
    }
}

/// `color` with its alpha taken off, for a shape drawn whole and then made see-through once.
fn opaque(c: &gdk::RGBA) -> gdk::RGBA {
    gdk::RGBA::new(c.red(), c.green(), c.blue(), 1.0)
}

/// Draw `shape` (in `color`, opaque) see-through by `color`'s alpha as one piece, so parts that
/// overlap are not darker.
fn see_through(snap: &gtk::Snapshot, color: &gdk::RGBA, shape: impl FnOnce(&gdk::RGBA)) {
    let alpha = color.alpha();
    if alpha < 1.0 {
        snap.push_opacity(alpha as f64);
    }
    shape(&opaque(color));
    if alpha < 1.0 {
        snap.pop();
    }
}

/// A drop shadow under the shape `path`: the shape moved by the offset, grown by a positive
/// spread or shrunk by a negative one, blurred as a box's shadow is (TM-25: the same radius
/// `append_outset_shadow` takes). Pictured for a vector file (`vector`): it may take a mask.
fn path_shadow(snap: &gtk::Snapshot, path: &gsk::Path, s: &Shadow, vector: bool) {
    pictured(snap, vector, None, |snap| path_shadow_on(snap, path, s));
}

fn path_shadow_on(snap: &gtk::Snapshot, path: &gsk::Path, s: &Shadow) {
    let c = rgba(&s.color);
    let blur = blur_of(s);
    snap.save();
    snap.translate(&graphene::Point::new(s.x as f32, s.y as f32));
    if blur > 0.0 {
        snap.push_blur(blur);
    }
    see_through(snap, &c, |c| {
        if s.spread < 0.0 {
            // The shape less a band along its edge.
            snap.push_mask(gsk::MaskMode::InvertedAlpha);
            snap.append_stroke(path, &gsk::Stroke::new((-s.spread * 2.0) as f32), c);
            snap.pop();
            snap.append_fill(path, gsk::FillRule::Winding, c);
            snap.pop();
        } else {
            snap.append_fill(path, gsk::FillRule::Winding, c);
            if s.spread > 0.0 {
                snap.append_stroke(path, &gsk::Stroke::new((s.spread * 2.0) as f32), c);
            }
        }
    });
    if blur > 0.0 {
        snap.pop();
    }
    snap.restore();
}

/// An inner shadow inside the shape `path` (its box `r`): everything outside the shape, moved by
/// the offset, the shape grown inwards by a positive spread (outwards by a negative one),
/// blurred, and kept inside the shape, as CSS's inset box-shadow. Pictured for a vector file
/// (`vector`): it takes a mask.
fn path_inset_shadow(snap: &gtk::Snapshot, path: &gsk::Path, s: &Shadow, r: &Rect, vector: bool) {
    pictured(snap, vector, None, |snap| path_inset_shadow_on(snap, path, s, r));
}

fn path_inset_shadow_on(snap: &gtk::Snapshot, path: &gsk::Path, s: &Shadow, r: &Rect) {
    let c = rgba(&s.color);
    let blur = blur_of(s);
    // Far enough round the shape that its edge never shows inside it.
    let around = r.inflate(s.x.abs().max(s.y.abs()) + blur * 2.0 + s.spread.abs() + 1.0);
    snap.push_fill(path, gsk::FillRule::Winding);
    snap.save();
    snap.translate(&graphene::Point::new(s.x as f32, s.y as f32));
    if blur > 0.0 {
        snap.push_blur(blur);
    }
    see_through(snap, &c, |c| {
        snap.push_mask(gsk::MaskMode::InvertedAlpha);
        snap.append_fill(path, gsk::FillRule::Winding, c);
        if s.spread < 0.0 {
            snap.append_stroke(path, &gsk::Stroke::new((-s.spread * 2.0) as f32), c);
        }
        snap.pop();
        snap.append_color(c, &grect(&around));
        snap.pop();
        if s.spread > 0.0 {
            snap.append_stroke(path, &gsk::Stroke::new((s.spread * 2.0) as f32), c);
        }
    });
    if blur > 0.0 {
        snap.pop();
    }
    snap.restore();
    snap.pop();
}

/// A node's outline: an ellipse, or its box with its corner radii (`radii`, else `radius`; a
/// sticky's 6 by default).
pub fn outline(n: &Node, r: &Rect) -> gsk::RoundedRect {
    let cap = |v: f64| v.min(r.w / 2.0).min(r.h / 2.0).max(0.0) as f32;
    if n.kind == NodeKind::Ellipse {
        let half = graphene::Size::new(r.w as f32 / 2.0, r.h as f32 / 2.0);
        return gsk::RoundedRect::new(grect(r), half, half, half, half);
    }
    match n.radii {
        Some([tl, tr, br, bl]) => {
            let c = |v: f64| graphene::Size::new(cap(v), cap(v));
            gsk::RoundedRect::new(grect(r), c(tl), c(tr), c(br), c(bl))
        }
        None => rounded(r, n.radius.unwrap_or(shapes::default_radius(n.kind))),
    }
}

fn polygon_path(n: &Node, r: &Rect) -> gsk::Path {
    let b = gsk::PathBuilder::new();
    let pts = shapes::node_polygon(n, r);
    if let Some((x, y)) = pts.first() {
        b.move_to(*x as f32, *y as f32);
        for (x, y) in &pts[1..] {
            b.line_to(*x as f32, *y as f32);
        }
        b.close();
    }
    b.to_path()
}

/// An ellipse's arc: a pie slice, or with a hole a ring segment, as a closed path. A ring
/// segment's ends are shaped by its `arc.cap` (`shapes::arc_ends`): round or square ends are part
/// of the outline, so the ring stays one fill that takes gradients, shadows and borders.
pub fn arc_path(n: &Node, r: &Rect) -> gsk::Path {
    let b = gsk::PathBuilder::new();
    let Some((start, sweep, inner)) = shapes::arc(n) else {
        b.add_rounded_rect(&outline(n, r));
        return b.to_path();
    };
    if inner > 0.0 && sweep.abs() >= 360.0 {
        // A whole ring: the hole is a second outline, cut out by the even-odd rule.
        return ring_path(r, inner);
    }
    let (cx, cy, rx, ry) = (r.cx(), r.cy(), r.w / 2.0, r.h / 2.0);
    let at = |deg: f64, k: f64| {
        let a = deg.to_radians();
        ((cx + rx * k * a.sin()) as f32, (cy - ry * k * a.cos()) as f32)
    };
    // In arcs of at most 90 degrees, so each is drawn true.
    let steps = (sweep.abs() / 90.0).ceil().max(1.0) as usize;
    let along = |b: &gsk::PathBuilder, from: f64, to: f64, k: f64| {
        let (rxk, ryk) = ((rx * k) as f32, (ry * k) as f32);
        for i in 1..=steps {
            let d = from + (to - from) * i as f64 / steps as f64;
            let (x, y) = at(d, k);
            b.svg_arc_to(rxk, ryk, 0.0, false, to > from, x, y);
        }
    };
    let (x, y) = at(start, 1.0);
    b.move_to(x, y);
    along(&b, start, start + sweep, 1.0);
    if inner > 0.0 {
        match shapes::arc_ends(n, r) {
            Some((cap, [first, last])) => {
                cap_to(&b, cap, &last, true, sweep > 0.0);
                along(&b, start + sweep, start, inner);
                cap_to(&b, cap, &first, false, sweep > 0.0);
            }
            None => {
                let (x, y) = at(start + sweep, inner);
                b.line_to(x, y);
                along(&b, start + sweep, start, inner);
            }
        }
    } else if sweep.abs() < 360.0 {
        b.line_to(cx as f32, cy as f32);
    }
    b.close();
    b.to_path()
}

/// Across one capped end of a ring segment, from the outer edge to the inner one (`inward`) or
/// back: half a circle bulging past the end for `round`, turning the way the ring runs
/// (`clockwise` for a positive sweep); out and back by half the ring's thickness for `square`.
fn cap_to(b: &gsk::PathBuilder, cap: StrokeCap, end: &ArcEnd, inward: bool, clockwise: bool) {
    let (from, to) = if inward { (end.outer, end.inner) } else { (end.inner, end.outer) };
    let c = end.cap_radius();
    match cap {
        StrokeCap::Round => b.svg_arc_to(c as f32, c as f32, 0.0, false, clockwise, to.0 as f32, to.1 as f32),
        StrokeCap::Square => {
            let (ox, oy) = (end.out.0 * c, end.out.1 * c);
            b.line_to((from.0 + ox) as f32, (from.1 + oy) as f32);
            b.line_to((to.0 + ox) as f32, (to.1 + oy) as f32);
            b.line_to(to.0 as f32, to.1 as f32);
        }
        StrokeCap::Butt => b.line_to(to.0 as f32, to.1 as f32),
    }
}

/// A free line's or path's stroke from its points as they are stored, relative to its box's
/// corner: `shapes::path_segments` from (0, 0).
fn stroke_path(n: &Node) -> Option<gsk::Path> {
    let (start, segs) = shapes::path_segments(n, (0.0, 0.0))?;
    Some(build_path(start, &segs, shapes::closed_shape(n)))
}

/// A path from `start` through `segs` (`shapes::path_segments`), closed for a closed shape.
fn build_path(start: (f64, f64), segs: &[Seg], closed: bool) -> gsk::Path {
    let b = gsk::PathBuilder::new();
    let f = |p: (f64, f64)| (p.0 as f32, p.1 as f32);
    let (x, y) = f(start);
    b.move_to(x, y);
    for seg in segs {
        match *seg {
            Seg::Line(to) => {
                let (x, y) = f(to);
                b.line_to(x, y);
            }
            Seg::Quad(c, to) => {
                let [(cx, cy), (x, y)] = [f(c), f(to)];
                b.quad_to(cx, cy, x, y);
            }
            Seg::Cubic(c1, c2, to) => {
                let [(ax, ay), (bx, by), (x, y)] = [f(c1), f(c2), f(to)];
                b.cubic_to(ax, ay, bx, by, x, y);
            }
        }
    }
    if closed {
        b.close();
    }
    b.to_path()
}

fn rect_of(b: &graphene::Rect) -> Rect {
    Rect::new(b.x() as f64, b.y() as f64, b.width() as f64, b.height() as f64)
}

/// A whole ring: the ellipse less its hole, as two outlines.
fn ring_path(r: &Rect, inner: f64) -> gsk::Path {
    let b = gsk::PathBuilder::new();
    let (cx, cy, rx, ry) = (r.cx() as f32, r.cy() as f32, (r.w / 2.0) as f32, (r.h / 2.0) as f32);
    let k = inner as f32;
    for (sx, sy, cw) in [(rx, ry, true), (rx * k, ry * k, false)] {
        b.move_to(cx, cy - sy);
        b.svg_arc_to(sx, sy, 0.0, false, cw, cx, cy + sy);
        b.svg_arc_to(sx, sy, 0.0, false, cw, cx, cy - sy);
        b.close();
    }
    b.to_path()
}

/// Mirror the snapshot about `r`'s centre as `n` is flipped; true when it saved, so the caller
/// restores.
fn flip_about(snap: &gtk::Snapshot, n: &Node, r: &Rect) -> bool {
    let (fx, fy) = (n.flip_x == Some(true), n.flip_y == Some(true));
    if !fx && !fy {
        return false;
    }
    snap.save();
    snap.translate(&graphene::Point::new(r.cx() as f32, r.cy() as f32));
    snap.scale(if fx { -1.0 } else { 1.0 }, if fy { -1.0 } else { 1.0 });
    snap.translate(&graphene::Point::new(-r.cx() as f32, -r.cy() as f32));
    true
}

/// How a stroke's ends are drawn: its `stroke_cap`, else round, except dashes, which end flat so
/// their gaps stay open at any weight, and dots (zero-length dashes), which need round ends to show.
fn cap_of(n: &Node) -> gsk::LineCap {
    let dots = n.dash.as_ref().is_some_and(|d| d.iter().step_by(2).any(|v| *v == 0.0));
    match n.stroke_cap {
        Some(StrokeCap::Butt) => gsk::LineCap::Butt,
        Some(StrokeCap::Round) => gsk::LineCap::Round,
        Some(StrokeCap::Square) => gsk::LineCap::Square,
        None if n.dash.is_some() && !dots => gsk::LineCap::Butt,
        None => gsk::LineCap::Round,
    }
}

/// How a stroke turns its corners: its `stroke_join`, else round. The miter limit stays GSK's
/// (and CSS's) 4.
fn join_of(n: &Node) -> gsk::LineJoin {
    match n.stroke_join {
        Some(StrokeJoin::Miter) => gsk::LineJoin::Miter,
        Some(StrokeJoin::Bevel) => gsk::LineJoin::Bevel,
        Some(StrokeJoin::Round) | None => gsk::LineJoin::Round,
    }
}

/// The pen for a node's stroke: its weight, ends (`cap_of`), corners (`join_of`) and dashes.
fn stroke_of(n: &Node, width: f64) -> gsk::Stroke {
    let stroke = gsk::Stroke::new(width as f32);
    stroke.set_line_join(join_of(n));
    stroke.set_line_cap(cap_of(n));
    if let Some(dash) = &n.dash {
        stroke.set_dash(&dash.iter().map(|v| *v as f32).collect::<Vec<_>>());
    }
    stroke
}

/// Paint `g` over `r` (the caller clips it to the shape). A conic gradient is pictured for a
/// vector file (`vector`).
fn gradient(snap: &gtk::Snapshot, g: &Gradient, r: &Rect, vector: bool) {
    let mut stops: Vec<gsk::ColorStop> = g.stops.iter().map(|s| gsk::ColorStop::new(s.at as f32, rgba(&s.color))).collect();
    stops.sort_by(|a, b| a.offset().total_cmp(&b.offset()));
    let centre = graphene::Point::new(r.cx() as f32, r.cy() as f32);
    match g.kind {
        GradientKind::Linear => {
            // CSS reading: 0 points up, 90 right, 180 (the default) down.
            let a = g.angle.unwrap_or(180.0).to_radians();
            let (dx, dy) = (a.sin(), -a.cos());
            let len = (r.w * a.sin()).abs() + (r.h * a.cos()).abs();
            let (cx, cy) = (r.cx(), r.cy());
            let start = graphene::Point::new((cx - dx * len / 2.0) as f32, (cy - dy * len / 2.0) as f32);
            let end = graphene::Point::new((cx + dx * len / 2.0) as f32, (cy + dy * len / 2.0) as f32);
            snap.append_linear_gradient(&grect(r), &start, &end, &stops);
        }
        GradientKind::Radial => {
            snap.append_radial_gradient(&grect(r), &centre, (r.w / 2.0).max(0.5) as f32, (r.h / 2.0).max(0.5) as f32, 0.0, 1.0, &stops);
        }
        // Round the centre, clockwise from `angle` (0, the default, at the top), as CSS's
        // `conic-gradient(from …)` and GSK's rotation read it.
        GradientKind::Conic => pictured(snap, vector, Some(r), |snap| snap.append_conic_gradient(&grect(r), &centre, g.angle.unwrap_or(0.0) as f32, &stops)),
    }
}

/// Where a shape's label sits: `shapes::label_box` (inside its `padding`, else 12 px in from its
/// sides, as tall as its words, centred), the box bounds count (B32). While it is typed into
/// with no words yet, the box one line would take, for the editor.
pub fn shape_text_box(p: &Painter, n: &Node, r: &Rect) -> Option<Rect> {
    if n.text.as_deref().is_none_or(str::is_empty) {
        if p.editing.borrow().as_deref() != Some(n.id.as_str()) {
            return None;
        }
        let mut probe = n.clone();
        probe.text = Some(" ".into());
        return shapes::label_box(&probe, r, p);
    }
    shapes::label_box(n, r, p)
}

pub fn rounded(r: &Rect, radius: f64) -> gsk::RoundedRect {
    let radius = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0) as f32;
    gsk::RoundedRect::from_rect(grect(r), radius)
}

impl Measure for Painter {
    fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
        let key = text_key(n, wrap);
        if let Some(size) = self.sizes.borrow().get(&key) {
            return *size;
        }
        let layout = self.new_layout(n, wrap);
        if n.text.as_deref().is_none_or(str::is_empty) {
            layout.set_text(" ");
        }
        let (_, logical) = layout.pixel_extents();
        let empty = n.text.as_deref().is_none_or(str::is_empty);
        let w = if empty { 2.0 } else { logical.width().max(1) as f64 + 1.0 };
        let size = (w, text_height(logical.height(), layout.line_count(), n.line_height.unwrap_or(1.3) * font_size(n)));
        self.sizes.borrow_mut().insert(key, size);
        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, parent: Option<&str>, kind: NodeKind, rect: [f64; 4]) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        [n.x, n.y, n.w, n.h] = rect;
        n
    }

    fn image(id: &str, parent: &str, src: &str) -> Node {
        let mut n = node(id, Some(parent), NodeKind::Image, [0.0, 0.0, 40.0, 30.0]);
        n.src = Some(src.into());
        n
    }

    #[test]
    fn pictures_leave_with_their_nodes_and_stay_while_they_load() {
        // Two pages, a picture on each (one inside a frame), and pictures no node shows.
        let scene = Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            node("p2", None, NodeKind::Page, [0.0; 4]),
            node("f", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 200.0]),
            image("a", "f", "shown.png"),
            image("b", "p2", "failed.png"),
        ]);
        let fresh = || {
            HashMap::from([
                ("shown.png".to_string(), Picture::Failed),
                ("failed.png".to_string(), Picture::Failed),
                ("gone.png".to_string(), Picture::Failed),
                ("deleted-while-loading.png".to_string(), Picture::Loading),
            ])
        };
        let mut map = fresh();
        prune_pictures(&mut map, &scene, false);
        let mut kept: Vec<&str> = map.keys().map(String::as_str).collect();
        kept.sort();
        assert_eq!(kept, ["deleted-while-loading.png", "failed.png", "shown.png"]);
        // The render job tries a failed picture again; a load in flight still lands.
        let mut map = fresh();
        prune_pictures(&mut map, &scene, true);
        assert_eq!(map.keys().collect::<Vec<_>>(), ["deleted-while-loading.png"]);
    }

    #[test]
    fn a_large_picture_decodes_smaller_and_a_small_one_at_its_size() {
        assert_eq!(decode_edge(6000), Some(DECODE_EDGE));
        assert_eq!(decode_edge(DECODE_EDGE + 1), Some(DECODE_EDGE));
        assert_eq!(decode_edge(DECODE_EDGE), None);
        assert_eq!(decode_edge(640), None);
    }

    /// Words 8 px a character on 20 px lines, wrapped at `wrap`.
    struct Mono;

    impl Measure for Mono {
        fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
            let chars = n.text.as_deref().map_or(0, |t| t.chars().count()) as f64;
            let line = wrap.map_or(f64::INFINITY, |w| (w / 8.0).floor().max(1.0));
            ((chars * 8.0).min(wrap.unwrap_or(f64::INFINITY)), (chars / line).ceil().max(1.0) * 20.0)
        }
    }

    fn pt(x: f32, y: f32) -> graphene::Point {
        graphene::Point::new(x, y)
    }

    #[test]
    fn caps_and_joins_follow_the_fields_and_keep_todays_rule_without_them() {
        let mut n = node("l", Some("p1"), NodeKind::Line, [0.0; 4]);
        assert_eq!((cap_of(&n), join_of(&n)), (gsk::LineCap::Round, gsk::LineJoin::Round));
        n.dash = Some(vec![6.0, 4.0]);
        assert_eq!(cap_of(&n), gsk::LineCap::Butt, "dashes end flat so their gaps stay open");
        n.dash = Some(vec![0.0, 4.0]);
        assert_eq!(cap_of(&n), gsk::LineCap::Round, "dots need round ends to show");
        n.stroke_cap = Some(StrokeCap::Square);
        n.stroke_join = Some(StrokeJoin::Miter);
        assert_eq!((cap_of(&n), join_of(&n)), (gsk::LineCap::Square, gsk::LineJoin::Miter));
        n.dash = Some(vec![6.0, 4.0]);
        n.stroke_cap = Some(StrokeCap::Round);
        n.stroke_join = Some(StrokeJoin::Bevel);
        assert_eq!((cap_of(&n), join_of(&n)), (gsk::LineCap::Round, gsk::LineJoin::Bevel));
    }

    #[test]
    fn a_ring_segment_ends_as_its_cap_says() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut n = node("e", Some("p1"), NodeKind::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        let ring = |cap| relay_bus::design::Arc { start: 0.0, sweep: 90.0, inner: 0.6, cap };
        let filled = |n: &Node, x: f32, y: f32| arc_path(n, &r).in_fill(&pt(x, y), gsk::FillRule::Winding);
        // The end at 90 degrees runs from (100, 50) to (80, 50); the start at the top from
        // (50, 0) to (50, 20). Just past each end:
        n.arc = Some(ring(None));
        assert!(filled(&n, 90.0, 45.0) && !filled(&n, 90.0, 57.0) && !filled(&n, 43.0, 10.0));
        n.arc = Some(ring(Some(StrokeCap::Butt)));
        assert!(!filled(&n, 90.0, 57.0));
        n.arc = Some(ring(Some(StrokeCap::Round)));
        assert!(filled(&n, 90.0, 57.0) && filled(&n, 43.0, 10.0));
        assert!(!filled(&n, 41.0, 1.5), "a round end leaves the square's corner out");
        n.arc = Some(ring(Some(StrokeCap::Square)));
        assert!(filled(&n, 90.0, 57.0) && filled(&n, 41.0, 1.5));
        // Running the other way, the caps turn with it.
        n.arc = Some(relay_bus::design::Arc { start: 90.0, sweep: -90.0, inner: 0.6, cap: Some(StrokeCap::Round) });
        assert!(filled(&n, 90.0, 57.0) && filled(&n, 43.0, 10.0));
    }

    #[test]
    fn a_pen_path_is_drawn_on_the_curve_hit_testing_takes() {
        // B118: through the midpoints (it passes (43.75, 62.5)), so it never reaches its middle
        // point, and hit testing takes the same curve.
        let mut n = node("a", Some("p1"), NodeKind::Path, [0.0, 0.0, 100.0, 100.0]);
        n.points = Some(vec![[0.0, 0.0], [50.0, 100.0], [100.0, 0.0]]);
        n.stroke_width = Some(2.0);
        let path = stroke_path(&n).unwrap();
        assert!(path.closest_point(&pt(43.75, 62.5), 1.0).is_some());
        assert!(path.closest_point(&pt(50.0, 99.0), 1.0).is_none());
        let scene = Scene::new(vec![node("p1", None, NodeKind::Page, [0.0; 4]), n.clone()]);
        let placed = layout::place(&scene, "p1", &Mono);
        let r = placed.rect("a").unwrap();
        assert!(relay_client::design::hit::on_shape(&scene, &placed, &n, &r, 43.75, 62.5, 1.0));
    }

    #[test]
    fn a_drawing_far_out_keeps_its_shape() {
        // B110: at 3e7 px f32 steps by 2, so narrowed on the page these points would fold.
        let x = 30_000_000.0;
        assert_eq!((x + 1.0) as f32, x as f32);
        let mut n = node("l", Some("p1"), NodeKind::Line, [x, x, 2.0, 1.0]);
        n.points = Some(vec![[0.0, 0.0], [1.0, 0.0], [2.0, 1.0]]);
        let b = stroke_path(&n).unwrap().bounds().unwrap();
        assert_eq!((b.x(), b.y(), b.width(), b.height()), (0.0, 0.0, 2.0, 1.0));
    }

    #[test]
    fn a_stored_shadow_blur_is_painted_within_the_cap() {
        // B72: a blur node's cost grows with its radius; older builds stored up to 10000.
        let shadow = |blur: f64| Shadow { x: 0.0, y: 0.0, blur, spread: 0.0, color: "#000".into(), inset: false };
        assert_eq!(blur_of(&shadow(10_000.0)), MAX_SHADOW_BLUR);
        assert_eq!(blur_of(&shadow(12.0)), 12.0);
        assert_eq!(blur_of(&shadow(-3.0)), 0.0);
        assert_eq!(blur_of(&shadow(f64::NAN)), 0.0);
    }

    fn run(start: u32, end: u32) -> TextRun {
        TextRun { start, end, color: None, font: None, font_size: None, font_weight: None, italic: None, underline: None, strike: None, rise: None }
    }

    #[test]
    fn a_stored_control_character_never_reaches_pango_and_runs_keep_their_letters() {
        // B43: "a", NUL, "bc", with a run over "bc" (characters 2 to 4 of the stored text).
        let mut n = node("t", Some("p1"), NodeKind::Text, [0.0; 4]);
        n.text = Some("a\u{0}bc\u{7}\nd".into());
        n.runs = Some(vec![TextRun { color: Some("#FF0000".into()), ..run(2, 4) }]);
        let (shown, offsets) = shown_text(&n);
        assert_eq!(shown, "abc\nd");
        let bytes = text::run_bytes(&n, &offsets);
        assert_eq!(bytes.iter().map(|(s, e, _)| (*s, *e)).collect::<Vec<_>>(), [(1, 3)]);
        assert_eq!(&shown[1..3], "bc");
        // Clean words pass through as they are.
        n.text = Some("Hello\tthere".into());
        assert_eq!(shown_text(&n).0, "Hello\tthere");
    }

    #[test]
    fn a_run_sets_only_what_it_names() {
        let n = node("t", Some("p1"), NodeKind::Text, [0.0; 4]);
        let types = |r: &TextRun| run_attrs(&n, r).iter().map(|a| a.type_()).collect::<Vec<_>>();
        let see_through = TextRun { color: Some("#FF000080".into()), ..run(0, 1) };
        assert_eq!(types(&see_through), [pango::AttrType::Foreground, pango::AttrType::ForegroundAlpha]);
        let attrs = run_attrs(&n, &see_through);
        let red = attrs[0].downcast_ref::<pango::AttrColor>().unwrap().color();
        assert_eq!((red.red(), red.green(), red.blue()), (65535, 0, 0));
        assert_eq!(attrs[1].downcast_ref::<pango::AttrInt>().unwrap().value(), 128 * 257);
        assert_eq!(types(&TextRun { color: Some("#00FF00".into()), ..run(0, 1) }), [pango::AttrType::Foreground]);
        assert!(types(&TextRun { color: Some("$accent".into()), ..run(0, 1) }).is_empty(), "an unresolved name keeps the node's colour");
        // A larger run raises its line: its size and its line height, at the node's multiple.
        let mut tall = node("t", Some("p1"), NodeKind::Text, [0.0; 4]);
        tall.line_height = Some(1.5);
        let attrs = run_attrs(&tall, &TextRun { font_size: Some(24.0), ..run(0, 1) });
        assert_eq!(attrs[0].downcast_ref::<pango::AttrSize>().unwrap().size(), 24 * pango::SCALE);
        assert_eq!(attrs[1].downcast_ref::<pango::AttrInt>().unwrap().value(), 36 * pango::SCALE);
        let styled = TextRun { font: Some("Geist\u{0}Mono".into()), font_weight: Some(700), italic: Some(true), underline: Some(true), strike: Some(false), rise: Some(-4.0), ..run(0, 1) };
        let attrs = run_attrs(&n, &styled);
        assert_eq!(attrs.iter().map(|a| a.type_()).collect::<Vec<_>>(), [
            pango::AttrType::Family,
            pango::AttrType::Weight,
            pango::AttrType::Style,
            pango::AttrType::Underline,
            pango::AttrType::Strikethrough,
            pango::AttrType::Rise
        ]);
        assert_eq!(attrs[0].downcast_ref::<pango::AttrString>().unwrap().value(), "GeistMono");
        assert_eq!(attrs[5].downcast_ref::<pango::AttrInt>().unwrap().value(), -4 * pango::SCALE);
    }

    #[test]
    fn words_taller_than_pango_holds_measure_by_their_lines() {
        // B111: 5,000 characters at 1,000 px, one a line, pass Pango's 32-bit units.
        let mut n = node("t", Some("p1"), NodeKind::Text, [0.0; 4]);
        n.text = Some("a".repeat(5_000));
        n.font_size = Some(1_000.0);
        assert!(tallest(&n) >= PANGO_MAX_PX);
        n.text = Some("A settings screen".into());
        n.font_size = Some(16.0);
        assert!(tallest(&n) < PANGO_MAX_PX);
        assert_eq!(text_height(62, 3, 20.8), 62.0);
        assert_eq!(text_height(-1_234, 500, 1_300.0), 650_000.0);
        assert_eq!(text_height(i32::MAX, 2_000_000, 1_300.0), MAX_COORD);
        assert_eq!(pango_units(1e9), (PANGO_MAX_PX * pango::SCALE as f64) as i32);
        assert_eq!(pango_units(100.0), 100 * pango::SCALE);
    }

    #[test]
    fn the_canvas_draws_words_typed_into_a_turned_node() {
        let mut frame = node("f", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 200.0]);
        let mut words = node("t", Some("f"), NodeKind::Text, [10.0, 10.0, 80.0, 20.0]);
        let scene = |f: &Node, t: &Node| Scene::new(vec![node("p1", None, NodeKind::Page, [0.0; 4]), f.clone(), t.clone()]);
        assert!(!turned_within(&scene(&frame, &words), "t"));
        words.rotation = Some(30.0);
        assert!(turned_within(&scene(&frame, &words), "t"));
        words.rotation = Some(360.0);
        assert!(!turned_within(&scene(&frame, &words), "t"));
        frame.rotation = Some(-15.0);
        assert!(turned_within(&scene(&frame, &words), "t"), "E-text-13: a turned frame turns its words");
    }

    #[test]
    fn a_page_lays_its_ground_behind_its_blending_nodes_alone() {
        let mut lone = node("lone", Some("p1"), NodeKind::Rect, [10.0, 10.0, 50.0, 40.0]);
        lone.blend = Some(Blend::Multiply);
        let mut hidden = node("hidden", Some("p1"), NodeKind::Rect, [300.0, 0.0, 50.0, 50.0]);
        hidden.blend = Some(Blend::Screen);
        hidden.hidden = Some(true);
        let mut normal = node("normal", Some("p1"), NodeKind::Rect, [100.0, 0.0, 50.0, 50.0]);
        normal.blend = Some(Blend::Normal);
        let inside = |id: &str| {
            let mut n = node(id, Some("f"), NodeKind::Rect, [0.0, 0.0, 20.0, 20.0]);
            n.blend = Some(Blend::Difference);
            n
        };
        let scene = Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            node("under", Some("p1"), NodeKind::Rect, [0.0, 0.0, 400.0, 400.0]),
            lone,
            hidden,
            normal,
            node("f", Some("p1"), NodeKind::Frame, [500.0, 0.0, 100.0, 100.0]),
            inside("in-frame"),
        ]);
        let placed = layout::place(&scene, "p1", &Mono);
        // A frame's children blend with its own fill; only the page's own blending nodes count.
        assert_eq!(blend_region(&scene, &placed, &Mono, "p1"), [Rect::new(10.0, 10.0, 50.0, 40.0)]);
    }

    #[test]
    fn a_picture_reaches_cairo_with_its_own_pixels() {
        // TM-21: a vector export draws the decoded pixels, in cairo's order, at their size.
        let pixels = [0, 0, 255, 255, 255, 0, 0, 255, 0, 128, 0, 128];
        let texture = gdk::MemoryTexture::new(3, 1, gdk::MemoryFormat::B8g8r8a8Premultiplied, &glib::Bytes::from(&pixels), 12);
        let mut surface = surface_of(texture.upcast_ref()).unwrap();
        assert_eq!((surface.width(), surface.height()), (3, 1));
        assert_eq!(&surface.data().unwrap()[..12], &pixels);
    }

    #[test]
    fn a_walk_under_a_node_takes_its_subtree_alone() {
        let scene = Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            node("f", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 200.0]),
            node("g", Some("f"), NodeKind::Group, [0.0; 4]),
            image("a", "g", "a.png"),
            image("b", "p1", "b.png"),
        ]);
        let mut seen = Vec::new();
        each_under(&scene, "f", |n| seen.push(n.id.as_str()));
        seen.sort();
        assert_eq!(seen, ["a", "f", "g"]);
    }
}
