//! Drawing a design's nodes as GSK render nodes, in page coordinates: the canvas scales the
//! result by the camera, and exports and gallery thumbnails reuse it at their own scale. Text
//! layouts, stroke paths and pictures are cached per node, so a frame that only pans rebuilds
//! nothing.

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{parse_color, Gradient, GradientKind, ImageFit, ListStyle, Node, NodeKind, Span, TextAlign};
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
    n.list.hash(&mut h);
    if let Some(spans) = n.live_spans() {
        for s in spans {
            s.text.hash(&mut h);
            (s.weight, s.italic, s.underline, s.strike, s.code).hash(&mut h);
            (&s.color, &s.font, &s.link).hash(&mut h);
            s.size.map(f64::to_bits).hash(&mut h);
        }
    }
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
        let (text, runs) = rich_text(n);
        layout.set_text(&text);
        let attrs = pango::AttrList::new();
        for (start, end, span) in &runs {
            span_attrs(&attrs, *start as u32, *end as u32, span);
        }
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
        let flip = (n.flip_x == Some(true), n.flip_y == Some(true));
        if flip.0 || flip.1 {
            snap.save();
            snap.translate(&graphene::Point::new(r.cx() as f32, r.cy() as f32));
            snap.scale(if flip.0 { -1.0 } else { 1.0 }, if flip.1 { -1.0 } else { 1.0 });
            snap.translate(&graphene::Point::new(-r.cx() as f32, -r.cy() as f32));
        }
        let filtered = n.filters.as_ref().and_then(relay_client::design::filters::matrix);
        if let Some((m, o)) = &filtered {
            // GSK multiplies a row vector by the matrix: the transpose of `rgb' = m · rgb`.
            let f = |i: usize, j: usize| m[j][i] as f32;
            let matrix = graphene::Matrix::from_float([
                f(0, 0), f(0, 1), f(0, 2), 0.0,
                f(1, 0), f(1, 1), f(1, 2), 0.0,
                f(2, 0), f(2, 1), f(2, 2), 0.0,
                0.0, 0.0, 0.0, 1.0,
            ]);
            snap.push_color_matrix(&matrix, &graphene::Vec4::new(o[0] as f32, o[1] as f32, o[2] as f32, 0.0));
        }
        let blur = n.blur.filter(|b| *b > 0.0);
        if let Some(b) = blur {
            snap.push_blur(b);
        }
        let editing = self.editing.borrow().as_deref() == Some(id);
        match n.kind {
            NodeKind::Page => {}
            NodeKind::Group => {
                for child in scene.children(id) {
                    self.paint(snap, scene, placed, child, view);
                }
            }
            NodeKind::Frame => {
                let rr = outline(n, &r);
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
                self.border(snap, n, &r);
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
                if n.src.is_some() {
                    snap.push_fill(&path, gsk::FillRule::Winding);
                    self.picture_in(snap, n, &r);
                    snap.pop();
                } else if let Some(g) = &n.gradient {
                    snap.push_fill(&path, gsk::FillRule::Winding);
                    gradient(snap, g, &r);
                    snap.pop();
                } else if let Some(fill) = fill_of(n).filter(|f| parse_color(f).is_some_and(|c| c[3] > 0)) {
                    snap.append_fill(&path, gsk::FillRule::Winding, &rgba(&fill));
                }
                if let Some(stroke) = &n.stroke {
                    let w = n.stroke_width.unwrap_or(1.0);
                    if w > 0.0 {
                        snap.append_stroke(&path, &stroke_of(n, w), &rgba(stroke));
                    }
                }
                if !editing {
                    self.shape_text(snap, scene, n, &r);
                }
            }
            NodeKind::Icon => self.icon(snap, scene, n, &r),
            NodeKind::Text => {
                if !editing {
                    if let Some(fill) = &n.fill {
                        snap.append_color(&rgba(fill), &grect(&r));
                    }
                    self.text(snap, scene, n, &r, layout::wrap_width(n, &r));
                }
            }
            NodeKind::Line | NodeKind::Path => self.stroke(snap, scene, placed, n, &r),
            NodeKind::Image => self.image(snap, n, &r),
            NodeKind::Vector => self.vector(snap, n, &r),
            // Pinned over the canvas by the board, never part of what is drawn or exported.
            NodeKind::Comment => {}
        }
        if blur.is_some() {
            snap.pop();
        }
        if filtered.is_some() {
            snap.pop();
        }
        if flip.0 || flip.1 {
            snap.restore();
        }
        if turn != 0.0 {
            snap.restore();
        }
        if opacity < 1.0 {
            snap.pop();
        }
    }

    /// Shadow then fill (a colour or a gradient), inside the rounded outline.
    fn box_paint(&self, snap: &gtk::Snapshot, n: &Node, rr: &gsk::RoundedRect) {
        if let Some(s) = &n.shadow {
            snap.append_outset_shadow(rr, &rgba(&s.color), s.x as f32, s.y as f32, s.spread as f32, s.blur as f32);
        }
        if n.src.is_some() {
            snap.push_rounded_clip(rr);
            let b = rr.bounds();
            self.picture_in(snap, n, &Rect::new(b.x() as f64, b.y() as f64, b.width() as f64, b.height() as f64));
            snap.pop();
        } else if let Some(g) = &n.gradient {
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
            let inset = Rect::new(r.x + w / 2.0, r.y + w / 2.0, (r.w - w).max(0.0), (r.h - w).max(0.0));
            let b = gsk::PathBuilder::new();
            b.add_rounded_rect(&outline(n, &inset));
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
        snap.append_stroke(&path, &stroke_of(n, width), &color);
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
        self.picture_in(snap, n, r);
        snap.pop();
        self.border(snap, n, r);
    }

    /// The node's picture (`src`) over `r` by its `fit` and `crop`; the caller clips. While it
    /// loads, or if it is missing, a quiet tile with a picture glyph.
    fn picture_in(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        let picture = n.src.as_deref().map(|s| self.picture(s)).unwrap_or(Picture::Failed);
        let natural = match &picture {
            Picture::Ready(t) => Some((t.width() as f64, t.height() as f64)),
            Picture::Vector(svg) => Some((svg.intrinsic_width().max(1) as f64, svg.intrinsic_height().max(1) as f64)),
            _ => None,
        };
        let Some((pw, ph)) = natural else {
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
            return;
        };
        // The crop is the part shown: fit it, then draw the whole picture around it.
        let [cx0, cy0, cw, ch] = n.crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let (tw, th) = (pw * cw, ph * ch);
        let (sx, sy) = (r.w / tw.max(1.0), r.h / th.max(1.0));
        let (kx, ky) = match n.fit.unwrap_or(ImageFit::Cover) {
            ImageFit::Stretch => (sx, sy),
            ImageFit::Cover => (sx.max(sy), sx.max(sy)),
            ImageFit::Contain => (sx.min(sy), sx.min(sy)),
        };
        let shown = Rect::new(r.cx() - tw * kx / 2.0, r.cy() - th * ky / 2.0, tw * kx, th * ky);
        let whole = Rect::new(shown.x - pw * cx0 * kx, shown.y - ph * cy0 * ky, pw * kx, ph * ky);
        let cropped = n.crop.is_some();
        if cropped {
            snap.push_clip(&grect(&shown));
        }
        match &picture {
            Picture::Ready(t) => snap.append_scaled_texture(t, gsk::ScalingFilter::Trilinear, &grect(&whole)),
            Picture::Vector(svg) => {
                snap.save();
                snap.translate(&graphene::Point::new(whole.x as f32, whole.y as f32));
                svg.snapshot(snap.upcast_ref::<gdk::Snapshot>(), whole.w, whole.h);
                snap.restore();
            }
            _ => {}
        }
        if cropped {
            snap.pop();
        }
    }

    /// A vector: its path scaled to its box, filled unless open, then stroked.
    fn vector(&self, snap: &gtk::Snapshot, n: &Node, r: &Rect) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        n.d.hash(&mut h);
        n.view.map(|v| (v[0].to_bits(), v[1].to_bits())).hash(&mut h);
        (r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()).hash(&mut h);
        let key = h.finish();
        let cached = self.paths.borrow().get(&n.id).filter(|(k, _)| *k == key).map(|(_, p)| p.clone());
        let path = match cached {
            Some(p) => p,
            None => {
                let p = vector_path(&shapes::vector_segments(n, r));
                self.paths.borrow_mut().insert(n.id.clone(), (key, p.clone()));
                p
            }
        };
        if n.open != Some(true) {
            if n.src.is_some() {
                snap.push_fill(&path, gsk::FillRule::Winding);
                self.picture_in(snap, n, r);
                snap.pop();
            } else if let Some(g) = &n.gradient {
                snap.push_fill(&path, gsk::FillRule::Winding);
                gradient(snap, g, r);
                snap.pop();
            } else if let Some(fill) = n.fill.as_deref().or(Some(SHAPE_FILL)).filter(|f| parse_color(f).is_some_and(|c| c[3] > 0)) {
                snap.append_fill(&path, gsk::FillRule::Winding, &rgba(fill));
            }
        }
        let stroke = n.stroke.as_deref().or(if n.open == Some(true) { Some(STROKE) } else { None });
        if let Some(stroke) = stroke {
            let w = n.stroke_width.unwrap_or(if n.open == Some(true) { 2.0 } else { 1.0 });
            if w > 0.0 {
                snap.append_stroke(&path, &stroke_of(n, w), &rgba(stroke));
            }
        }
    }
}

/// A GSK path through svgpath segments.
pub fn vector_path(segs: &[relay_bus::svgpath::Seg]) -> gsk::Path {
    use relay_bus::svgpath::Seg;
    let b = gsk::PathBuilder::new();
    for s in segs {
        match *s {
            Seg::Move([x, y]) => b.move_to(x as f32, y as f32),
            Seg::Line([x, y]) => b.line_to(x as f32, y as f32),
            Seg::Cubic(a, c, e) => b.cubic_to(a[0] as f32, a[1] as f32, c[0] as f32, c[1] as f32, e[0] as f32, e[1] as f32),
            Seg::Close => b.close(),
        }
    }
    b.to_path()
}

/// What a text lays out: its words with each list line's marker before it, and its live spans
/// as byte ranges of that.
pub fn rich_text(n: &Node) -> (String, Vec<(usize, usize, Span)>) {
    let text = n.text.as_deref().unwrap_or("");
    let plain = [Span { text: text.to_string(), ..Span::default() }];
    let spans = n.live_spans().unwrap_or(&plain);
    let styled = n.live_spans().is_some();
    let mut out = String::with_capacity(text.len() + 16);
    let mut runs = Vec::new();
    let mut line = 0;
    let mut line_start = true;
    for span in spans {
        let start = out.len();
        let mut rest = span.text.as_str();
        while !rest.is_empty() {
            if line_start {
                line += 1;
                if let Some(list) = n.list {
                    let (mark, skip) = match list {
                        ListStyle::Bullet => ("•  ".to_string(), 0),
                        ListStyle::Number => (format!("{line}.  "), 0),
                        ListStyle::Check if rest.starts_with("[x] ") || rest.starts_with("[X] ") => ("☑  ".to_string(), 4),
                        ListStyle::Check if rest.starts_with("[ ] ") => ("☐  ".to_string(), 4),
                        ListStyle::Check => ("☐  ".to_string(), 0),
                    };
                    out.push_str(&mark);
                    rest = &rest[skip..];
                }
            }
            let cut = rest.find('\n').map_or(rest.len(), |i| i + 1);
            out.push_str(&rest[..cut]);
            line_start = rest[..cut].ends_with('\n');
            rest = &rest[cut..];
        }
        if styled && out.len() > start {
            runs.push((start, out.len(), span.clone()));
        }
    }
    (out, runs)
}

/// Pango attributes for one styled run.
fn span_attrs(attrs: &pango::AttrList, start: u32, end: u32, s: &Span) {
    let put = |mut a: pango::Attribute| {
        a.set_start_index(start);
        a.set_end_index(end);
        attrs.insert(a);
    };
    if let Some(w) = s.weight {
        put(pango::AttrInt::new_weight(weight(w)).upcast());
    }
    if s.italic == Some(true) {
        put(pango::AttrInt::new_style(pango::Style::Italic).upcast());
    }
    if s.underline == Some(true) || s.link.is_some() {
        put(pango::AttrInt::new_underline(pango::Underline::Single).upcast());
    }
    if s.strike == Some(true) {
        put(pango::AttrInt::new_strikethrough(true).upcast());
    }
    let color = s.color.as_deref().or(s.link.as_ref().map(|_| LINK));
    if let Some([r, g, b, a]) = color.and_then(parse_color) {
        let c = |v: u8| v as u16 * 257;
        put(pango::AttrColor::new_foreground(c(r), c(g), c(b)).upcast());
        if a < 255 {
            put(pango::AttrInt::new_foreground_alpha(c(a)).upcast());
        }
    }
    if let Some(size) = s.size {
        put(pango::AttrSize::new_size_absolute((size * pango::SCALE as f64) as i32).upcast());
    }
    if s.code == Some(true) {
        put(pango::AttrString::new_family("Geist Mono, monospace").upcast());
        put(pango::AttrColor::new_background(0x8080, 0x8080, 0x8080).upcast());
        put(pango::AttrInt::new_background_alpha(0x3000).upcast());
    } else if let Some(font) = &s.font {
        put(pango::AttrString::new_family(font).upcast());
    }
}

/// A link's words, when they name no colour.
const LINK: &str = "#4C8DFF";

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

/// The pen for a node's stroke: round ends and joins, and its dash pattern if it has one.
fn stroke_of(n: &Node, width: f64) -> gsk::Stroke {
    let stroke = gsk::Stroke::new(width as f32);
    stroke.set_line_cap(gsk::LineCap::Round);
    stroke.set_line_join(gsk::LineJoin::Round);
    if let Some(dash) = &n.dash {
        stroke.set_dash(&dash.iter().map(|v| *v as f32).collect::<Vec<_>>());
    }
    stroke
}

/// Paint `g` over `r` (the caller clips it to the shape).
fn gradient(snap: &gtk::Snapshot, g: &Gradient, r: &Rect) {
    let mut stops: Vec<gsk::ColorStop> = g.stops.iter().map(|s| gsk::ColorStop::new(s.at as f32, rgba(&s.color))).collect();
    stops.sort_by(|a, b| a.offset().total_cmp(&b.offset()));
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
