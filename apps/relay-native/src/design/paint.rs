//! Drawing a design's nodes as GSK render nodes, in page coordinates: the canvas scales the
//! result by the camera, and exports and gallery thumbnails reuse it at their own scale. Text
//! layouts, stroke paths and pictures are cached per node, so a frame that only pans rebuilds
//! nothing.

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{parse_color, ImageFit, Node, NodeKind, TextAlign};
use relay_client::design::layout::{self, Measure, Placed, STICKY_PAD};
use relay_client::design::{Rect, Scene};
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
            NodeKind::Rect | NodeKind::Ellipse => Some(SHAPE_FILL),
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
    if n.kind == NodeKind::Sticky {
        return if luminance(&fill_of(n).unwrap_or_default()) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }.into();
    }
    let ground = std::iter::once(n.id.as_str())
        .chain(scene.ancestors(&n.id))
        .filter_map(|id| scene.node(id))
        .filter(|a| a.id != n.id)
        .find_map(|a| fill_of(a).filter(|f| parse_color(f).is_some_and(|c| c[3] >= 128)))
        .unwrap_or_else(|| CANVAS.into());
    if luminance(&ground) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }.into()
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
        layout.set_text(text);
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
        attrs.insert(pango::AttrFloat::new_line_height(n.line_height.unwrap_or(1.3)));
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
        if points.len() == 1 {
            b.line_to(x0 + 0.01, y0);
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
        let path = b.to_path();
        self.paths.borrow_mut().insert(n.id.clone(), (key, path.clone()));
        Some(path)
    }

    /// Paint `page`'s content: its children, in page coordinates. `view` culls what lies
    /// wholly outside it.
    pub fn paint_page(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, page: &str, view: Option<&Rect>) {
        for child in scene.children(page) {
            self.paint(snap, scene, placed, child, view);
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
        let reach = n.shadow.as_ref().map_or(0.0, |s| s.blur * 2.0 + s.spread.abs() + s.x.abs().max(s.y.abs()))
            + n.stroke_width.unwrap_or(2.0)
            + 16.0;
        let culls = !n.kind.holds_children() || clips;
        if culls && view.is_some_and(|v| !v.intersects(&r.inflate(reach))) {
            return;
        }
        let opacity = n.opacity.unwrap_or(1.0);
        if opacity <= 0.0 {
            return;
        }
        if opacity < 1.0 {
            snap.push_opacity(opacity);
        }
        match n.kind {
            NodeKind::Page => {}
            NodeKind::Group => {
                for child in scene.children(id) {
                    self.paint(snap, scene, placed, child, view);
                }
            }
            NodeKind::Frame => {
                let rr = rounded(&r, n.radius.unwrap_or(0.0));
                self.box_paint(snap, n, &rr);
                if clips {
                    snap.push_rounded_clip(&rr);
                }
                for child in scene.children(id) {
                    self.paint(snap, scene, placed, child, view);
                }
                if clips {
                    snap.pop();
                }
                self.border(snap, n, &rr);
            }
            NodeKind::Rect | NodeKind::Sticky => {
                let radius = n.radius.unwrap_or(if n.kind == NodeKind::Sticky { 6.0 } else { 0.0 });
                let rr = rounded(&r, radius);
                if n.kind == NodeKind::Sticky && n.shadow.is_none() {
                    snap.append_outset_shadow(&rr, &gdk::RGBA::new(0.0, 0.0, 0.0, 0.22), 0.0, 3.0, 0.0, 10.0);
                }
                self.box_paint(snap, n, &rr);
                self.border(snap, n, &rr);
                if n.kind == NodeKind::Sticky && self.editing.borrow().as_deref() != Some(id) {
                    let inner = Rect::new(r.x + STICKY_PAD, r.y + STICKY_PAD, (r.w - 2.0 * STICKY_PAD).max(1.0), r.h);
                    self.text(snap, scene, n, &inner, layout::wrap_width(n, &r));
                }
            }
            NodeKind::Ellipse => {
                let rr = gsk::RoundedRect::new(
                    grect(&r),
                    graphene::Size::new(r.w as f32 / 2.0, r.h as f32 / 2.0),
                    graphene::Size::new(r.w as f32 / 2.0, r.h as f32 / 2.0),
                    graphene::Size::new(r.w as f32 / 2.0, r.h as f32 / 2.0),
                    graphene::Size::new(r.w as f32 / 2.0, r.h as f32 / 2.0),
                );
                self.box_paint(snap, n, &rr);
                self.border(snap, n, &rr);
            }
            NodeKind::Text => {
                if self.editing.borrow().as_deref() != Some(id) {
                    if let Some(fill) = &n.fill {
                        snap.append_color(&rgba(fill), &grect(&r));
                    }
                    self.text(snap, scene, n, &r, layout::wrap_width(n, &r));
                }
            }
            NodeKind::Line | NodeKind::Path => self.stroke(snap, n, &r),
            NodeKind::Image => self.image(snap, n, &r),
        }
        if opacity < 1.0 {
            snap.pop();
        }
    }

    /// Shadow then fill, inside the rounded outline.
    fn box_paint(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        if let Some(s) = &n.shadow {
            snap.append_outset_shadow(rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, s.blur as f32);
        }
        if let Some(fill) = fill_of(n).filter(|f| parse_color(f).is_some_and(|c| c[3] > 0)) {
            snap.push_rounded_clip(rr);
            snap.append_color(&rgba(&fill), rr.bounds());
            snap.pop();
        }
    }

    fn border(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        if let Some(stroke) = &n.stroke {
            let w = n.stroke_width.unwrap_or(1.0) as f32;
            if w > 0.0 {
                let c = rgba(stroke);
                snap.append_border(rr, &[w; 4], &[c, c, c, c]);
            }
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

    fn stroke(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        let Some(path) = self.path(n, (r.x, r.y)) else {
            return;
        };
        let width = n.stroke_width.unwrap_or(2.0).max(0.5);
        let stroke = gsk::Stroke::new(width as f32);
        stroke.set_line_cap(gsk::LineCap::Round);
        stroke.set_line_join(gsk::LineJoin::Round);
        let color = rgba(n.stroke.as_deref().unwrap_or(STROKE));
        snap.append_stroke(&path, &stroke, &color);
        let Some(points) = n.points.as_deref().filter(|p| p.len() >= 2) else {
            return;
        };
        let at = |p: &[f64; 2]| (r.x + p[0], r.y + p[1]);
        let mut heads = Vec::new();
        if n.arrow_end == Some(true) {
            heads.push((at(&points[points.len() - 2]), at(&points[points.len() - 1])));
        }
        if n.arrow_start == Some(true) {
            heads.push((at(&points[1]), at(&points[0])));
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
        let rr = rounded(r, n.radius.unwrap_or(0.0));
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
        self.border(snap, n, &rr);
    }
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
