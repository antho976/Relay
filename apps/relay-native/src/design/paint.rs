//! Drawing a design's nodes as GSK render nodes, in page coordinates: the canvas scales the
//! result by the camera, and exports and gallery thumbnails reuse it at their own scale. Text
//! layouts, stroke paths and pictures are cached per node, so a frame that only pans rebuilds
//! nothing.

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{parse_color, Blend, Gradient, GradientKind, ImageFit, Node, NodeKind, TextAlign};
use relay_client::design::layout::{self, Measure, Placed, STICKY_PAD};
use relay_client::design::{shapes, Rect, Scene};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Defaults the schema documents: a white frame, a grey shape, a yellow sticky.
pub const FRAME_FILL: &str = "#FFFFFF";
pub const SHAPE_FILL: &str = "#D9D9D9";
pub const STICKY_FILL: &str = "#FFE58A";
pub const STROKE: &str = "#8C8C8C";
/// A page's canvas when it names no fill: a step off the app's ground.
pub const CANVAS: &str = "#1E1D1B";
const DARK_TEXT: &str = "#1A1A1A";
const LIGHT_TEXT: &str = "#EDE9E2";

pub fn rgba(hex: &str) -> gdk::RGBA {
    let [r, g, b, a] = parse_color(hex).unwrap_or([0, 0, 0, 255]);
    gdk::RGBA::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
}

pub fn grect(r: &Rect) -> graphene::Rect {
    graphene::Rect::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
}

/// Relative luminance of a colour, 0 (black) to 1 (white), ignoring alpha.
pub fn luminance(hex: &str) -> f64 {
    let [r, g, b, _] = parse_color(hex).unwrap_or([0, 0, 0, 255]);
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// The fill a node paints, its kind's default when it names none.
pub fn fill_of(n: &Node) -> Option<String> {
    n.fill.clone().or_else(|| {
        match n.kind {
            NodeKind::Frame => Some(FRAME_FILL),
            NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon => Some(SHAPE_FILL),
            NodeKind::Sticky => Some(STICKY_FILL),
            NodeKind::Page => Some(CANVAS),
            _ => None,
        }
        .map(str::to_string)
    })
}

/// A text's colour: its own, or dark on a light ground and light on a dark one, the ground
/// being the nearest container that paints one.
pub fn text_color(scene: &Scene, n: &Node) -> String {
    if let Some(c) = &n.color {
        return c.clone();
    }
    if matches!(n.kind, NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) {
        // A label sits on the shape itself.
        let ground = n.gradient.as_ref().and_then(|g| g.stops.first()).map(|s| s.color.clone()).or_else(|| fill_of(n)).unwrap_or_default();
        return if luminance(&ground) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }.into();
    }
    let ground = std::iter::once(n.id.as_str())
        .chain(scene.ancestors(&n.id))
        .filter_map(|id| scene.node(id))
        .filter(|a| a.id != n.id)
        .find_map(|a| ground_of(a).filter(|f| parse_color(f).is_some_and(|c| c[3] >= 128)))
        .unwrap_or_else(|| CANVAS.into());
    if luminance(&ground) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }.into()
}

/// The colour a node shows behind what is on it: its fill, or the middle of its gradient.
fn ground_of(n: &Node) -> Option<String> {
    match &n.gradient {
        Some(g) if !g.stops.is_empty() => {
            let mut stops = g.stops.clone();
            stops.sort_by(|a, b| a.at.total_cmp(&b.at));
            Some(stops[stops.len() / 2].color.clone())
        }
        _ => fill_of(n),
    }
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
    d.set_family(font_family(n));
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
    wrap.map(|w| (w * 100.0).round() as i64).hash(&mut h);
    h.finish()
}

#[derive(Clone)]
enum Picture {
    Loading,
    Ready(gdk::Texture),
    Vector(gtk::Svg),
    Failed,
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
        })
    }

    pub fn on_loaded(&self, f: impl Fn() + 'static) {
        *self.loaded.borrow_mut() = Some(Rc::new(f));
    }

    /// Drop the layouts and paths of nodes that are gone, so a long session's caches stay the
    /// size of the design.
    pub fn retain(&self, scene: &Scene) {
        self.layouts.borrow_mut().retain(|id, _| scene.node(id).is_some());
        self.paths.borrow_mut().retain(|id, _| scene.node(id).is_some());
        let mut sizes = self.sizes.borrow_mut();
        if sizes.len() > 20_000 {
            sizes.clear();
        }
    }

    fn new_layout(&self, n: &Node, wrap: Option<f64>) -> pango::Layout {
        let layout = pango::Layout::new(&self.ctx);
        layout.set_font_description(Some(&font_description(n)));
        let text = n.text.as_deref().unwrap_or("");
        match n.text_case {
            Some(case) => layout.set_text(&case.apply(text)),
            None => layout.set_text(text),
        }
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
        layout.set_attributes(Some(&attrs));
        if let Some(w) = wrap {
            layout.set_width((w * pango::SCALE as f64) as i32);
            layout.set_wrap(pango::WrapMode::WordChar);
        }
        layout.set_alignment(match n.align {
            Some(TextAlign::Center) => pango::Alignment::Center,
            Some(TextAlign::Right) => pango::Alignment::Right,
            _ => pango::Alignment::Left,
        });
        layout
    }

    /// Whether any picture `scene` shows is still loading; asking starts the loads.
    pub fn loading(&self, scene: &Scene) -> bool {
        let mut any = false;
        for n in scene.to_nodes() {
            if let Some(src) = n.src.as_deref().filter(|_| n.kind == NodeKind::Image) {
                any |= matches!(self.picture(src), Picture::Loading);
            }
        }
        any
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
            let read = gtk::gio::spawn_blocking(move || -> Result<(Option<gdk::Texture>, Option<Vec<u8>>), String> {
                if is_svg {
                    std::fs::read(&path).map(|b| (None, Some(b))).map_err(|e| e.to_string())
                } else {
                    gdk::Texture::from_filename(&path).map(|t| (Some(t), None)).map_err(|e| e.to_string())
                }
            })
            .await;
            let picture = match read {
                Ok(Ok((Some(texture), _))) => Picture::Ready(texture),
                Ok(Ok((None, Some(bytes)))) => Picture::Vector(gtk::Svg::from_bytes(&glib::Bytes::from_owned(bytes))),
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

    /// A stroke's path: smoothed through the midpoints for a pen path, straight for a line.
    fn path(&self, n: &Node, origin: (f64, f64)) -> Option<gsk::Path> {
        let points = n.points.as_deref()?;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in points {
            p[0].to_bits().hash(&mut h);
            p[1].to_bits().hash(&mut h);
        }
        origin.0.to_bits().hash(&mut h);
        origin.1.to_bits().hash(&mut h);
        (n.kind == NodeKind::Path).hash(&mut h);
        n.smooth.hash(&mut h);
        n.closed.hash(&mut h);
        let key = h.finish();
        if let Some((k, path)) = self.paths.borrow().get(&n.id) {
            if *k == key {
                return Some(path.clone());
            }
        }
        let b = gsk::PathBuilder::new();
        let at = |p: &[f64; 2]| ((origin.0 + p[0]) as f32, (origin.1 + p[1]) as f32);
        let (x0, y0) = at(points.first()?);
        b.move_to(x0, y0);
        let closed = n.closed == Some(true) && points.len() > 2;
        if points.len() == 1 {
            b.line_to(x0 + 0.01, y0);
        } else if n.kind == NodeKind::Line && n.smooth == Some(true) && points.len() > 2 {
            let pts: Vec<(f64, f64)> = points.iter().map(|p| (origin.0 + p[0], origin.1 + p[1])).collect();
            for [c1, c2, to] in shapes::smooth_segments(&pts, closed) {
                b.cubic_to(c1.0 as f32, c1.1 as f32, c2.0 as f32, c2.1 as f32, to.0 as f32, to.1 as f32);
            }
        } else if n.kind == NodeKind::Path && points.len() > 2 {
            for w in points.windows(2).skip(1) {
                let (cx, cy) = at(&w[0]);
                let (nx, ny) = at(&w[1]);
                b.quad_to(cx, cy, (cx + nx) / 2.0, (cy + ny) / 2.0);
            }
            let (lx, ly) = at(points.last()?);
            b.line_to(lx, ly);
        } else {
            for p in &points[1..] {
                let (x, y) = at(p);
                b.line_to(x, y);
            }
        }
        if closed {
            b.close();
        }
        let path = b.to_path();
        self.paths.borrow_mut().insert(n.id.clone(), (key, path.clone()));
        Some(path)
    }

    /// Paint `page`'s content: its children, in page coordinates. `view` culls what lies
    /// wholly outside it.
    pub fn paint_page(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, page: &str, view: Option<&Rect>) {
        self.paint_children(snap, scene, placed, page, view, None);
    }

    /// Paint `parent`'s children in order. A child with a `blend` mixes with what is painted
    /// before it in the same parent (and `ground`, the parent's own fill, under them all).
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
        let mut acc = gtk::Snapshot::new();
        if let Some(ground) = ground {
            ground(&acc);
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
                }
                None => self.paint(&acc, scene, placed, child, view),
            }
        }
        if let Some(node) = acc.to_node() {
            snap.append_node(&node);
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
        let reach = n.shadow.as_ref().map_or(0.0, |s| s.blur * 2.0 + s.spread.abs() + s.x.abs().max(s.y.abs()))
            + n.stroke_width.unwrap_or(2.0)
            + n.blur.unwrap_or(0.0) * 2.0
            + 16.0;
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
        let editing = self.editing.borrow().as_deref() == Some(id);
        match n.kind {
            NodeKind::Page => {}
            NodeKind::Group => self.paint_children(snap, scene, placed, id, view, None),
            NodeKind::Frame => {
                let rr = outline(n, &r);
                self.shadow(snap, n, &rr);
                if clips {
                    snap.push_rounded_clip(&rr);
                }
                self.paint_children(snap, scene, placed, id, view, Some(&|s: &gtk::Snapshot| self.fill(s, n, &rr)));
                if clips {
                    snap.pop();
                }
                self.inner_shadow(snap, n, &rr);
                self.border(snap, n, &r);
            }
            NodeKind::Ellipse if shapes::arc(n).is_some() => {
                let path = arc_path(n, &r);
                self.shape(snap, n, &path, &r);
                if !editing {
                    self.shape_text(snap, scene, n, &r);
                }
            }
            NodeKind::Rect | NodeKind::Sticky | NodeKind::Ellipse => {
                let rr = outline(n, &r);
                if n.kind == NodeKind::Sticky && n.shadow.is_none() {
                    snap.append_outset_shadow(&rr, &gdk::RGBA::new(0.0, 0.0, 0.0, 0.22), 0.0, 3.0, 0.0, 10.0);
                }
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
                let path = polygon_path(n, &r);
                self.shape(snap, n, &path, &r);
                if !editing {
                    self.shape_text(snap, scene, n, &r);
                }
            }
            NodeKind::Icon => {
                let flipped = flip_about(snap, n, &r);
                self.icon(snap, scene, n, &r);
                if flipped {
                    snap.restore();
                }
            }
            NodeKind::Text => {
                if !editing {
                    if let Some(fill) = &n.fill {
                        snap.append_color(&rgba(fill), &grect(&r));
                    }
                    self.text(snap, scene, n, &r, layout::wrap_width(n, &r));
                }
            }
            NodeKind::Line | NodeKind::Path => self.stroke(snap, scene, placed, n, &r),
            NodeKind::Image => {
                let flipped = flip_about(snap, n, &r);
                self.image(snap, n, &r);
                if flipped {
                    snap.restore();
                }
            }
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
        self.shadow(snap, n, rr);
        self.fill(snap, n, rr);
        self.inner_shadow(snap, n, rr);
    }

    /// A drop shadow under the rounded outline.
    fn shadow(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        if let Some(s) = n.shadow.as_ref().filter(|s| !s.inset) {
            snap.append_outset_shadow(rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, s.blur as f32);
        }
    }

    /// An inner shadow inside the rounded outline.
    fn inner_shadow(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        if let Some(s) = n.shadow.as_ref().filter(|s| s.inset) {
            snap.append_inset_shadow(rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, s.blur as f32);
        }
    }

    /// A shape drawn from a path (a polygon, an arc, a closed line): its shadow, fill and
    /// stroke, the stroke inside the edge as a box's border is.
    fn shape(&self, snap: &gtk::Snapshot, n: &Node, path: &gsk::Path, r: &Rect) {
        if let Some(s) = &n.shadow {
            let c = rgba(&s.color);
            if s.inset {
                // Inside: the outline's ring, offset and blurred, clipped to the shape.
                snap.push_fill(path, gsk::FillRule::Winding);
                self.fill_path(snap, n, path, r);
                snap.save();
                snap.translate(&graphene::Point::new(s.x as f32, s.y as f32));
                if s.blur > 0.0 {
                    snap.push_blur(s.blur / 2.0);
                }
                snap.append_stroke(path, &gsk::Stroke::new((s.blur.max(1.0) + s.spread.max(0.0) * 2.0) as f32), &c);
                if s.blur > 0.0 {
                    snap.pop();
                }
                snap.restore();
                snap.pop();
            } else {
                snap.save();
                snap.translate(&graphene::Point::new(s.x as f32, s.y as f32));
                if s.blur > 0.0 {
                    snap.push_blur(s.blur / 2.0);
                }
                snap.append_fill(path, gsk::FillRule::Winding, &c);
                if s.spread > 0.0 {
                    snap.append_stroke(path, &gsk::Stroke::new((s.spread * 2.0) as f32), &c);
                }
                if s.blur > 0.0 {
                    snap.pop();
                }
                snap.restore();
                self.fill_path(snap, n, path, r);
            }
        } else {
            self.fill_path(snap, n, path, r);
        }
        if let Some(stroke) = &n.stroke {
            let w = n.stroke_width.unwrap_or(1.0);
            if w > 0.0 {
                snap.push_fill(path, gsk::FillRule::Winding);
                snap.append_stroke(path, &stroke_of(n, w * 2.0), &rgba(stroke));
                snap.pop();
            }
        }
    }

    fn fill_path(&self, snap: &gtk::Snapshot, n: &Node, path: &gsk::Path, r: &Rect) {
        if let Some(g) = &n.gradient {
            snap.push_fill(path, gsk::FillRule::Winding);
            gradient(snap, g, r);
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
            gradient(snap, g, &Rect::new(b.x() as f64, b.y() as f64, b.width() as f64, b.height() as f64));
            snap.pop();
        } else if let Some(fill) = fill_of(n).filter(|f| parse_color(f).is_some_and(|c| c[3] > 0)) {
            snap.push_rounded_clip(rr);
            snap.append_color(&rgba(&fill), rr.bounds());
            snap.pop();
        }
    }

    /// The outline drawn inside the edge; dashed, as a stroke along the edge inset by half its
    /// width.
    fn border(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        let Some(stroke) = &n.stroke else {
            return;
        };
        let w = n.stroke_width.unwrap_or(1.0);
        if w <= 0.0 {
            return;
        }
        let c = rgba(stroke);
        if n.dash.is_some() {
            // Along the middle of the border, its corners following the fill's.
            let half = (w / 2.0).min(r.w / 2.0).min(r.h / 2.0) as f32;
            let mut inset = outline(n, r);
            inset.shrink(half, half, half, half);
            let b = gsk::PathBuilder::new();
            b.add_rounded_rect(&inset);
            snap.append_stroke(&b.to_path(), &stroke_of(n, w), &c);
        } else {
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
        let s = (r.w.min(r.h) / 24.0).max(0.01);
        snap.save();
        snap.translate(&graphene::Point::new((r.cx() - 12.0 * s) as f32, (r.cy() - 12.0 * s) as f32));
        snap.scale(s as f32, s as f32);
        let stroke = gsk::Stroke::new(n.stroke_width.unwrap_or(2.0) as f32);
        stroke.set_line_cap(gsk::LineCap::Round);
        stroke.set_line_join(gsk::LineJoin::Round);
        snap.append_stroke(&path, &stroke, &color);
        snap.restore();
    }

    fn stroke(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, n: &Node, r: &Rect) {
        let width = n.stroke_width.unwrap_or(2.0).max(0.5);
        let color = rgba(n.stroke.as_deref().unwrap_or(STROKE));
        let routed = n.kind == NodeKind::Line && (n.from.is_some() || n.to.is_some() || n.route.is_some());
        // The two segments an arrowhead sits on: from where to the tip, at each end.
        let (path, ends) = if routed {
            let (pts, curved) = shapes::line_points(scene, placed, n);
            if pts.len() < 2 {
                return;
            }
            let b = gsk::PathBuilder::new();
            b.move_to(pts[0].0 as f32, pts[0].1 as f32);
            if curved {
                b.cubic_to(pts[1].0 as f32, pts[1].1 as f32, pts[2].0 as f32, pts[2].1 as f32, pts[3].0 as f32, pts[3].1 as f32);
            } else {
                for p in &pts[1..] {
                    b.line_to(p.0 as f32, p.1 as f32);
                }
            }
            let last = pts.len() - 1;
            (b.to_path(), Some(((pts[last - 1], pts[last]), (pts[1], pts[0]))))
        } else {
            let Some(path) = self.path(n, (r.x, r.y)) else {
                return;
            };
            let ends = n.points.as_deref().filter(|p| p.len() >= 2).map(|p| {
                let at = |q: &[f64; 2]| (r.x + q[0], r.y + q[1]);
                ((at(&p[p.len() - 2]), at(&p[p.len() - 1])), (at(&p[1]), at(&p[0])))
            });
            (path, ends)
        };
        let points = n.points.as_deref().map_or(0, <[_]>::len);
        if !routed && points > 2 && (n.fill.is_some() || n.gradient.is_some()) {
            self.fill_path(snap, n, &path, r);
        }
        if n.stroke.as_deref().is_none_or(|s| parse_color(s).is_some_and(|c| c[3] > 0)) {
            snap.append_stroke(&path, &stroke_of(n, width), &color);
        }
        if n.closed == Some(true) && !routed {
            return;
        }
        let Some((end, start)) = ends else {
            return;
        };
        let mut heads = Vec::new();
        if n.arrow_end == Some(true) {
            heads.push(end);
        }
        if n.arrow_start == Some(true) {
            heads.push(start);
        }
        for (from, tip) in heads {
            let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                continue;
            }
            let (ux, uy) = (dx / len, dy / len);
            let size = (width * 3.5).max(8.0);
            let back = (tip.0 - ux * size, tip.1 - uy * size);
            let (px, py) = (-uy * size * 0.55, ux * size * 0.55);
            let b = gsk::PathBuilder::new();
            b.move_to(tip.0 as f32, tip.1 as f32);
            b.line_to((back.0 + px) as f32, (back.1 + py) as f32);
            b.line_to((back.0 - px) as f32, (back.1 - py) as f32);
            b.close();
            snap.append_fill(&b.to_path(), gsk::FillRule::Winding, &color);
        }
    }

    fn image(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        let rr = outline(n, r);
        if let Some(s) = &n.shadow {
            snap.append_outset_shadow(&rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, s.blur as f32);
        }
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
                match &picture {
                    Picture::Ready(t) => snap.append_scaled_texture(t, gsk::ScalingFilter::Trilinear, &grect(&target)),
                    Picture::Vector(svg) => {
                        snap.save();
                        snap.translate(&graphene::Point::new(target.x as f32, target.y as f32));
                        svg.snapshot(snap.upcast_ref::<gdk::Snapshot>(), target.w, target.h);
                        snap.restore();
                    }
                    _ => {}
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
        self.border(snap, n, r);
    }
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
        None => rounded(r, n.radius.unwrap_or(if n.kind == NodeKind::Sticky { 6.0 } else { 0.0 })),
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

/// An ellipse's arc: a pie slice, or with a hole a ring segment, as a closed path.
pub fn arc_path(n: &Node, r: &Rect) -> gsk::Path {
    let b = gsk::PathBuilder::new();
    let Some((start, sweep, inner)) = shapes::arc(n) else {
        b.add_rounded_rect(&outline(n, r));
        return b.to_path();
    };
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
        let (x, y) = at(start + sweep, inner);
        b.line_to(x, y);
        along(&b, start + sweep, start, inner);
    } else if sweep.abs() < 360.0 {
        b.line_to(cx as f32, cy as f32);
    }
    b.close();
    if inner > 0.0 && sweep.abs() >= 360.0 {
        // A whole ring: the hole is a second outline, cut out by the even-odd rule.
        return ring_path(r, inner);
    }
    b.to_path()
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

/// The pen for a node's stroke: round joins, and its dash pattern if it has one. Dashes end
/// square so their gaps stay open at any weight; a pattern with zero-length dashes is dots,
/// which need round ends to show.
fn stroke_of(n: &Node, width: f64) -> gsk::Stroke {
    let stroke = gsk::Stroke::new(width as f32);
    stroke.set_line_join(gsk::LineJoin::Round);
    match &n.dash {
        Some(dash) => {
            let dots = dash.iter().step_by(2).any(|v| *v == 0.0);
            stroke.set_line_cap(if dots { gsk::LineCap::Round } else { gsk::LineCap::Butt });
            stroke.set_dash(&dash.iter().map(|v| *v as f32).collect::<Vec<_>>());
        }
        None => stroke.set_line_cap(gsk::LineCap::Round),
    }
    stroke
}

/// Paint `g` over `r` (the caller clips it to the shape).
fn gradient(snap: &gtk::Snapshot, g: &Gradient, r: &Rect) {
    let mut stops: Vec<gsk::ColorStop> = g.stops.iter().map(|s| gsk::ColorStop::new(s.at as f32, rgba(&s.color))).collect();
    stops.sort_by(|a, b| a.offset().total_cmp(&b.offset()));
    match g.kind {
        // A conic gradient paints as linear until native-paint draws it round its centre.
        GradientKind::Linear | GradientKind::Conic => {
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
            let centre = graphene::Point::new(r.cx() as f32, r.cy() as f32);
            snap.append_radial_gradient(&grect(r), &centre, (r.w / 2.0).max(0.5) as f32, (r.h / 2.0).max(0.5) as f32, 0.0, 1.0, &stops);
        }
    }
}

/// Where a shape's label sits: centred, 12 px in from its sides, as tall as its words.
pub fn shape_text_box(p: &Painter, n: &Node, r: &Rect) -> Option<Rect> {
    if n.text.as_deref().is_none_or(str::is_empty) && p.editing.borrow().as_deref() != Some(n.id.as_str()) {
        return None;
    }
    let w = (r.w - 24.0).max(1.0);
    let mut probe = n.clone();
    if probe.text.as_deref().is_none_or(str::is_empty) {
        probe.text = Some(" ".into());
    }
    let (_, h) = p.measure(&probe, Some(w));
    Some(Rect::new(r.x + 12.0, r.cy() - h / 2.0, w, h))
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
        let size = (w, logical.height().max(1) as f64);
        self.sizes.borrow_mut().insert(key, size);
        size
    }
}
