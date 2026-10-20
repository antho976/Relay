//! Pictures of a design: a selection or a page as PNG (rendered by the window's GPU renderer),
//! or as SVG or PDF (the same render nodes drawn on a cairo vector surface), all drawn by the
//! painter the canvas uses. Ask sends the agent a PNG of what you mean, and `design.render`
//! draws the agent's pictures, through the same [`picture_of`].
//!
//! Motion leaves the canvas here too (feature-motion §3): a picture at a time of it, a contact
//! sheet of evenly spaced frames, and animations as an APNG or numbered PNG frames. Each is
//! drawn by the same painter posed for its time ([`picture_in`]), over one area that holds
//! every frame ([`posed_area`]), so the frames line up. Every still is drawn at rest, even
//! during Play.

use super::board::{display_name, Board};
use super::paint::{blend_mode, grect, outline, rgba, Painter, Posing};
use gtk::prelude::*;
use gtk::{gdk, gio, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{parse_color, NodeKind, Pose};
use relay_client::design::color::{fill_of, ground_of, luminance, CANVAS};
use relay_client::design::layout::{Measure, Placed};
use relay_client::design::motion::{self, Clocks};
use relay_client::design::{apng, bounds, boxes, edit, names, shapes, Rect, Scene};
use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::{Rc, Weak};
use std::time::Duration;

/// Most pixels along a side of an exported PNG.
const MAX_PIXELS: u32 = 8192;
/// Most points along a side of a PDF page: 200 inches, the most Acrobat opens (B46).
const PDF_MAX_PT: u32 = 14_400;
/// The bytes a file name's stem may take: with "@1.37x" and ".png" it stays within the 255
/// bytes a file name holds (B101).
const STEM_BYTES: usize = 200;
/// How many times 72 ppi a vector file's blurs and soft shadows are pictured at, which cairo can
/// carry only as images: 216 ppi (E-export-5).
const VECTOR_DETAIL: f64 = 3.0;
/// How long an export waits for the pictures it shows to load (E-export-2).
const PICTURE_WAIT: Duration = Duration::from_secs(3);
/// Frames a second an animation is exported at.
pub const ANIMATION_FPS: u16 = 30;
/// A contact sheet's space around and between its cells, and the band under each cell its
/// time is written in, in picture px; a cell's time is Geist Mono at `SHEET_LABEL_PX`.
const SHEET_GUTTER: u32 = 16;
const SHEET_LABEL: u32 = 22;
const SHEET_LABEL_PX: f64 = 12.0;
/// The frames of Ask's contact sheet, and the longest side it is drawn within: a model sees
/// no more (I-8).
pub const ASK_FRAMES: u8 = 6;
pub const ASK_SIDE: u32 = 1568;
/// Frames of an animation being encoded off the main thread at once: the next waits, so a long
/// export never holds hundreds of raw frames.
const ENCODING: usize = 4;
/// How often an animation's progress is said, in frames.
const PROGRESS_EVERY: usize = 15;

/// What a file of the design is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    /// A picture at this scale.
    Png(f64),
    Svg,
    Pdf,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Png(_) => "png",
            Format::Svg => "svg",
            Format::Pdf => "pdf",
        }
    }

    /// The scale `area` is drawn at in this format: a PNG's own within 8192 px a side, an SVG
    /// 1:1 at any size, a PDF 1:1 within 14,400 pt a side (B46, TM-23).
    pub(super) fn scale(self, area: &Rect) -> f64 {
        match self {
            Format::Png(s) => boxes::fit_scale(area, s, MAX_PIXELS, f64::INFINITY),
            Format::Svg => 1.0,
            Format::Pdf => boxes::fit_scale(area, 1.0, PDF_MAX_PT, f64::INFINITY),
        }
    }
}

/// Why there is no picture (E-export-1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Failure {
    /// Nothing chosen shows anything: hidden, clear, or gone.
    Empty,
    /// The canvas has no renderer yet (it is not on screen).
    NoRenderer,
    /// An animation or contact sheet of what does not move.
    Still,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Failure::Empty => "Nothing to export: what is chosen shows nothing",
            Failure::NoRenderer => "The canvas isn't ready to draw yet",
            Failure::Still => "Nothing chosen moves yet: give it motion in the inspector",
        }
    }
}

/// The page area a picture of `ids` covers (none, or every layer the page shows: the page),
/// in page px: `boxes::area`, the rule `design.render` answers with.
pub(crate) fn area_of(painter: &Painter, scene: &Scene, placed: &Placed, page: &str, ids: &[String]) -> Option<Rect> {
    boxes::area(scene, placed, painter, page, ids)
}

/// The render node of `ids` on `page` (none: the whole page) at `scale`, drawn from the corner
/// of its area ([`area_of`]), and that area. `placed` is the page as a still shows it, every
/// scrolling frame at its start. A page, or a choice of every layer it shows, is painted as
/// the canvas paints it, its top-level blends mixing with its fill; with `backdrop` the fill
/// lies under the whole area. A selection is drawn inside its ancestors' opacity, turns and
/// clips, on the nearest ground under it (an ancestor's fill, else the page's) with
/// `backdrop`, else on transparency (TM-26). `None` when nothing in it shows. Always at rest,
/// whatever Play is showing.
pub(crate) fn picture_of(painter: &Painter, scene: &Scene, placed: &Placed, page: &str, ids: &[String], backdrop: bool, scale: f64) -> Option<(gsk::RenderNode, Rect)> {
    let area = area_of(painter, scene, placed, page, ids)?;
    Some((picture_in(painter, scene, placed, page, ids, backdrop, scale, &area, None)?, area))
}

/// [`picture_of`] over a given `area` (one that holds every frame of a sheet or an animation),
/// posed by `posing`, or at rest with none: an export or Ask during Play never shows the frame
/// Play is on.
#[allow(clippy::too_many_arguments)]
pub(crate) fn picture_in(painter: &Painter, scene: &Scene, placed: &Placed, page: &str, ids: &[String], backdrop: bool, scale: f64, area: &Rect, posing: Option<Rc<Posing>>) -> Option<gsk::RenderNode> {
    painter.with_posing(posing, || draw_in(painter, scene, placed, page, ids, backdrop, scale, area))
}

#[allow(clippy::too_many_arguments)]
fn draw_in(painter: &Painter, scene: &Scene, placed: &Placed, page: &str, ids: &[String], backdrop: bool, scale: f64, area: &Rect) -> Option<gsk::RenderNode> {
    let area = *area;
    let snap = gtk::Snapshot::new();
    snap.scale(scale as f32, scale as f32);
    snap.translate(&graphene::Point::new(-area.x as f32, -area.y as f32));
    let on_page: Vec<String> = ids.iter().filter(|id| scene.page_of(id) == Some(page)).cloned().collect();
    let outer = edit::outermost(scene, &on_page);
    if ids.is_empty() || ids.iter().any(|id| id == page) || whole_page(scene, page, &outer) {
        let fill = backdrop.then(|| scene.node(page).and_then(fill_of).unwrap_or_else(|| CANVAS.into()));
        if let Some(fill) = &fill {
            snap.append_color(&rgba(fill), &grect(&area));
        }
        painter.paint_page_on(&snap, scene, placed, page, None, fill.as_deref());
    } else {
        let ground = backdrop.then(|| ground_under(scene, page, &outer));
        let mut acc = gtk::Snapshot::new();
        if let Some(ground) = &ground {
            acc.append_color(&rgba(ground), &grect(&area));
        }
        for id in &outer {
            // A blend mixes with the ground and what is drawn before it; a vector file keeps
            // what it draws as vectors, so it is drawn plain there.
            match scene.node(id).filter(|_| !painter.vector.get()).and_then(blend_mode) {
                Some(mode) => {
                    let below = acc.to_node();
                    acc = gtk::Snapshot::new();
                    acc.push_blend(mode);
                    if let Some(below) = &below {
                        acc.append_node(below);
                    }
                    acc.pop();
                    in_ancestors(painter, &acc, scene, placed, id);
                    acc.pop();
                }
                None => in_ancestors(painter, &acc, scene, placed, id),
            }
        }
        if let Some(node) = acc.to_node() {
            snap.append_node(&node);
        }
    }
    snap.to_node()
}

/// Whether `outer` is every layer `page` shows, each on the page itself: a picture of them is
/// a picture of the page.
fn whole_page(scene: &Scene, page: &str, outer: &[String]) -> bool {
    let shown = shown_children(scene, page);
    !shown.is_empty() && outer.len() == shown.len() && shown.iter().all(|id| outer.contains(id))
}

/// `page`'s top-level layers that show: not hidden, not wholly clear.
fn shown_children(scene: &Scene, page: &str) -> Vec<String> {
    let shows = |id: &&String| scene.node(id).is_some_and(|n| !n.is_hidden() && n.opacity.is_none_or(|o| o > 0.0));
    scene.children(page).iter().filter(shows).cloned().collect()
}

/// The colour a selection lies on: the nearest fill under all of `outer` at least half opaque
/// (a frame's, a shape's), else the page's.
fn ground_under(scene: &Scene, page: &str, outer: &[String]) -> String {
    let shared = |a: &str| outer.iter().all(|id| scene.ancestors(id).contains(&a));
    outer
        .first()
        .map(|id| scene.ancestors(id))
        .unwrap_or_default()
        .into_iter()
        .filter(|a| *a != page && shared(a))
        .filter_map(|a| scene.node(a))
        .find_map(|a| ground_of(a).filter(|g| parse_color(g).is_some_and(|c| c[3] >= 128)))
        .or_else(|| scene.node(page).and_then(fill_of))
        .unwrap_or_else(|| CANVAS.into())
}

/// Paint `id` inside its ancestors' effects, outermost first, as the page draws it: each one's
/// opacity, its turn about its centre, and a clipping frame's clip.
fn in_ancestors(painter: &Painter, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, id: &str) {
    let mut pops = Vec::new();
    for a in scene.ancestors(id).into_iter().rev() {
        let (Some(n), Some(r)) = (scene.node(a), placed.rect(a)) else {
            continue;
        };
        if n.kind == NodeKind::Page {
            continue;
        }
        let opacity = n.opacity.unwrap_or(1.0);
        if opacity < 1.0 {
            snap.push_opacity(opacity.max(0.0));
            pops.push(false);
        }
        let turn = shapes::rotation(n);
        if turn != 0.0 {
            snap.save();
            snap.translate(&graphene::Point::new(r.cx() as f32, r.cy() as f32));
            snap.rotate(turn as f32);
            snap.translate(&graphene::Point::new(-r.cx() as f32, -r.cy() as f32));
            pops.push(true);
        }
        if n.kind == NodeKind::Frame && n.clip != Some(false) {
            snap.push_rounded_clip(&outline(n, &r));
            pops.push(false);
        }
    }
    painter.paint(snap, scene, placed, id, None);
    for restore in pops.into_iter().rev() {
        if restore {
            snap.restore();
        } else {
            snap.pop();
        }
    }
}

// ------------------------------------------------------------------ motion

/// The clocks a picture of `page` at a time is posed by, as Play makes them: its movers, and
/// the shown pictures that animate themselves (repainted with every frame). One per picture,
/// sheet or export, so what does not move is painted once for all of its frames.
pub(crate) fn clocks_of(painter: &Painter, scene: &Scene, page: &str) -> Rc<Clocks> {
    let mut clocks = Clocks::new(scene, page);
    let moving = painter.moving_srcs();
    if !moving.is_empty() {
        let mut stack = vec![page.to_string()];
        while let Some(id) = stack.pop() {
            let Some(n) = scene.node(&id).filter(|n| !n.is_hidden()) else {
                continue;
            };
            if n.kind == NodeKind::Image && n.src.as_deref().is_some_and(|s| moving.contains(s)) {
                clocks.touch(scene, &id);
            }
            stack.extend(scene.children(&id).iter().cloned());
        }
    }
    Rc::new(clocks)
}

/// Each mover's pose at `t_ms` on `clocks`, for the painter's `with_posing`.
pub(crate) fn posing_at(scene: &Scene, clocks: &Rc<Clocks>, t_ms: f64) -> Rc<Posing> {
    let mut poses = HashMap::new();
    clocks.poses(scene, t_ms, &mut poses);
    Rc::new(Posing { poses, clocks: clocks.clone() })
}

/// What a picture of `ids` (none: the page) holds.
fn roots(page: &str, ids: &[String]) -> Vec<String> {
    if ids.is_empty() {
        vec![page.to_string()]
    } else {
        ids.to_vec()
    }
}

/// How long the motion of what `ids` (none: the page) hold runs, and whether all of it loops:
/// `(0, false)` when none of it moves.
pub(crate) fn span_of(scene: &Scene, clocks: &Clocks, page: &str, ids: &[String]) -> (f64, bool) {
    clocks.span_of(scene, &roots(page, ids))
}

/// Whether anything shown in `ids` (none: the page) moves by its keys: what the export menus'
/// animation entries wait for. Stops at the first mover.
pub(crate) fn moves(scene: &Scene, page: &str, ids: &[String]) -> bool {
    let roots = roots(page, ids);
    let mut stack: Vec<&str> = roots.iter().map(String::as_str).collect();
    while let Some(id) = stack.pop() {
        let Some(n) = scene.node(id).filter(|n| !n.is_hidden()) else {
            continue;
        };
        if n.motion.is_some() && n.kind != NodeKind::Page {
            return true;
        }
        stack.extend(scene.children(id).iter().map(String::as_str));
    }
    false
}

/// The page area a picture of `ids` (none: the page) covers at each of `times`: its area at
/// rest ([`area_of`]) and wherever its movers are taken then, so every frame of one sheet or
/// animation lines up and nothing moved is cut off. A mover's painted bounds are carried out
/// through itself and its ancestors, innermost first: each ancestor's clip and turn, and the
/// pose of each inside the picture (`motion::posed_bounds`); a selection is drawn inside its
/// ancestors at rest. `None` when nothing in it shows at any of them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn posed_area(measure: &dyn Measure, scene: &Scene, placed: &Placed, page: &str, ids: &[String], clocks: &Clocks, times: &[f64]) -> Option<Rect> {
    let rest = boxes::area(scene, placed, measure, page, ids);
    let whole = ids.is_empty() || ids.iter().any(|id| id == page);
    let on_page: Vec<String> = ids.iter().filter(|id| scene.page_of(id) == Some(page)).cloned().collect();
    let outer: HashSet<String> = edit::outermost(scene, &on_page).into_iter().collect();
    // Each mover in the picture: its bounds at rest, itself and its ancestors below the page,
    // and how many of them are inside the picture (posed).
    let mut reach = Vec::new();
    for (id, _) in clocks.movers() {
        let chain: Vec<&str> = std::iter::once(id).chain(scene.ancestors(id)).filter(|a| *a != page).collect();
        let posed = if whole { Some(chain.len()) } else { chain.iter().position(|a| outer.contains(*a)).map(|at| at + 1) };
        let (Some(posed), Some(b)) = (posed, bounds::painted_bounds(scene, placed, measure, id)) else {
            continue;
        };
        reach.push((b, chain, posed));
    }
    let margin = if whole { boxes::PAGE_MARGIN } else { 0.0 };
    let mut poses = HashMap::new();
    let mut area = rest;
    for t in times {
        clocks.poses(scene, *t, &mut poses);
        for (b, chain, posed) in &reach {
            if let Some(b) = carried(scene, placed, *b, chain, *posed, &poses) {
                let b = b.inflate(margin);
                area = Some(area.map_or(b, |a| a.union(&b)));
            }
        }
    }
    area.map(|a| boxes::whole_px(&a))
}

/// `b`, what `chain[0]` paints, carried out through `chain` (it, then its ancestors): each
/// ancestor's clip and turn, and the pose of the first `posed`. `None` when a clip hides it.
fn carried(scene: &Scene, placed: &Placed, mut b: Rect, chain: &[&str], posed: usize, poses: &HashMap<String, Pose>) -> Option<Rect> {
    for (at, id) in chain.iter().enumerate() {
        let (n, r) = (scene.node(id)?, placed.rect(id)?);
        // Its own clip and turn are in what it paints.
        if at > 0 {
            if n.kind == NodeKind::Frame && n.clip != Some(false) {
                b = meet(&b, &r)?;
            }
            let turn = shapes::rotation(n);
            if turn != 0.0 {
                b = shapes::rotated_bounds_about(&b, turn, r.cx(), r.cy());
            }
        }
        if let Some(pose) = poses.get(*id).filter(|_| at < posed) {
            b = posed_reach(&b, &r, pose, n.blur.unwrap_or(0.0));
        }
    }
    Some(b)
}

/// `b` once moved by `pose`, which turns and scales about its origin in the node's box `r`; a
/// blur past the node's own `blur` widens it.
fn posed_reach(b: &Rect, r: &Rect, pose: &Pose, blur: f64) -> Rect {
    let grow = pose.blur.map_or(0.0, |p| (p - blur.max(0.0)).max(0.0) * 2.0);
    if !pose.moves() {
        return b.inflate(grow);
    }
    // `posed_bounds` takes its origin as fractions of the box it moves: the box's origin, said
    // in `b`'s.
    let (w, h) = (b.w.max(1e-6), b.h.max(1e-6));
    let (ox, oy) = (r.x + pose.origin[0] * r.w, r.y + pose.origin[1] * r.h);
    let about = Pose { origin: [(ox - b.x) / w, (oy - b.y) / h], ..pose.clone() };
    motion::posed_bounds(&Rect::new(b.x, b.y, w, h), &about).inflate(grow)
}

/// Where `a` and `b` overlap, if they do.
fn meet(a: &Rect, b: &Rect) -> Option<Rect> {
    let (x0, y0) = (a.x.max(b.x), a.y.max(b.y));
    let (x1, y1) = (a.right().min(b.right()), a.bottom().min(b.bottom()));
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Where a contact sheet's cells lie: `cols` across and `rows` down in reading order, each
/// `cell` px with a `label` px band under it for its time, `gutter` px around and between.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sheet {
    pub cols: u32,
    pub rows: u32,
    pub cell: (u32, u32),
    pub gutter: u32,
    pub label: u32,
}

impl Sheet {
    /// `n` cells of `cell` px, ⌈√n⌉ across.
    fn new(n: usize, cell: (u32, u32), gutter: u32, label: u32) -> Sheet {
        let n = n.clamp(1, u32::MAX as usize) as u32;
        let cols = f64::from(n).sqrt().ceil() as u32;
        Sheet { cols, rows: n.div_ceil(cols), cell, gutter, label }
    }

    /// The whole sheet, in px.
    pub(crate) fn size(&self) -> (u32, u32) {
        let (cw, ch) = self.cell;
        (self.cols * cw + (self.cols + 1) * self.gutter, self.rows * (ch + self.label) + (self.rows + 1) * self.gutter)
    }

    /// Cell `i`'s top-left corner, in reading order.
    pub(crate) fn origin(&self, i: usize) -> (u32, u32) {
        let (col, row) = (i as u32 % self.cols, i as u32 / self.cols);
        (self.gutter + col * (self.cell.0 + self.gutter), self.gutter + row * (self.cell.1 + self.label + self.gutter))
    }
}

/// The sheet `n` frames of `area` are laid out on and the scale each is drawn at: `requested`,
/// lowered until the whole sheet, gutters and labels with it, is within `max_side` px a side
/// and `max_area_px` in all. When the gutters and labels would take more than a third of the
/// cells' size (24 frames in 256 px), they are left out (a 2 px gutter between the cells), and
/// the times are only in the answer.
pub(crate) fn sheet_fit(area: &Rect, n: usize, requested: f64, max_side: u32, max_area_px: f64) -> (Sheet, f64) {
    let fits = |s: &Sheet| {
        let (w, h) = s.size();
        w.max(h) <= max_side && f64::from(w) * f64::from(h) <= max_area_px
    };
    let at = |scale: f64, (gutter, label): (u32, u32)| Sheet::new(n, boxes::picture_size(area, scale), gutter, label);
    const TINY: f64 = 1e-9;
    // The largest scale up to `requested` a sheet with `chrome` fits at, if any.
    let best = |chrome: (u32, u32)| -> Option<f64> {
        if !fits(&at(TINY, chrome)) {
            return None;
        }
        if fits(&at(requested, chrome)) {
            return Some(requested);
        }
        // A bisection: the sheet grows with the scale.
        let (mut lo, mut hi) = (TINY, requested);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if fits(&at(mid, chrome)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    };
    let (full, bare) = ((SHEET_GUTTER, SHEET_LABEL), (2, 0));
    match (best(full), best(bare)) {
        (Some(s), Some(b)) if s >= b * 2.0 / 3.0 => (at(s, full), s),
        (_, Some(b)) => (at(b, bare), b),
        (Some(s), None) => (at(s, full), s),
        (None, None) => (at(TINY, (0, 0)), TINY),
    }
}

/// A frame that shows nothing at its time (a fade's start, on transparency) as an empty cell.
pub(crate) fn or_blank(node: Option<gsk::RenderNode>) -> gsk::RenderNode {
    node.unwrap_or_else(|| gsk::ContainerNode::new(&[]).upcast())
}

/// A frame's time as a sheet's label and Ask's prompt say it: whole ms.
pub(crate) fn time_label(t_ms: f64) -> String {
    format!("{} ms", t_ms.round() as i64)
}

/// A contact sheet: each of `cells` (drawn from its own corner) in its place on `sheet`, on
/// `ground`, its time under it in Geist Mono. `widget` lays the words out as the app does.
pub(crate) fn sheet_node(widget: &impl IsA<gtk::Widget>, sheet: &Sheet, cells: &[gsk::RenderNode], times: &[f64], ground: &str) -> Option<gsk::RenderNode> {
    let snap = gtk::Snapshot::new();
    let (w, h) = sheet.size();
    snap.append_color(&rgba(ground), &graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
    let ink = if luminance(ground) > 0.35 { gdk::RGBA::new(0.1, 0.1, 0.1, 0.8) } else { gdk::RGBA::new(0.93, 0.91, 0.89, 0.8) };
    let mut font = pango::FontDescription::new();
    font.set_family("Geist Mono");
    font.set_absolute_size(SHEET_LABEL_PX * f64::from(pango::SCALE));
    let (cw, ch) = sheet.cell;
    for (i, (cell, t)) in cells.iter().zip(times).enumerate() {
        let (x, y) = sheet.origin(i);
        snap.save();
        snap.translate(&graphene::Point::new(x as f32, y as f32));
        let bounds = graphene::Rect::new(0.0, 0.0, cw as f32, ch as f32);
        snap.push_clip(&bounds);
        snap.append_node(cell);
        snap.pop();
        // A hairline round each cell: on the page's own ground, where one ends is not seen.
        if sheet.gutter > 0 {
            let edge = gdk::RGBA::new(ink.red(), ink.green(), ink.blue(), 0.25);
            let outside = graphene::Rect::new(-1.0, -1.0, cw as f32 + 2.0, ch as f32 + 2.0);
            snap.append_border(&gsk::RoundedRect::from_rect(outside, 0.0), &[1.0; 4], &[edge; 4]);
        }
        if sheet.label > 0 {
            let layout = widget.as_ref().create_pango_layout(Some(&time_label(*t)));
            layout.set_font_description(Some(&font));
            let (lw, lh) = layout.pixel_size();
            let (lx, ly) = ((cw as i32 - lw) / 2, ch as i32 + (sheet.label as i32 - lh) / 2);
            snap.translate(&graphene::Point::new(lx.max(0) as f32, ly as f32));
            snap.append_layout(&layout, &ink);
        }
        snap.restore();
    }
    snap.to_node()
}

/// The page's canvas colour, behind a contact sheet's cells.
pub(crate) fn sheet_ground(scene: &Scene, page: &str) -> String {
    scene.node(page).and_then(fill_of).unwrap_or_else(|| CANVAS.into())
}

/// `texture`'s pixels as 8-bit RGBA with straight alpha: every frame of an animation in one
/// format, as APNG needs, whatever the renderer drew in.
fn rgba_of(texture: &gdk::Texture) -> (glib::Bytes, usize) {
    let mut down = gdk::TextureDownloader::new(texture);
    down.set_format(gdk::MemoryFormat::R8g8b8a8);
    down.download_bytes()
}

/// `w`×`h` 8-bit RGBA pixels as a PNG: off the main thread.
fn png_of(w: u32, h: u32, bytes: &glib::Bytes, stride: usize) -> Vec<u8> {
    gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::R8g8b8a8, bytes, stride).save_to_png_bytes().to_vec()
}

/// What exporting `ids` (none: the page) pictures, and whether on its ground by default. A
/// page whose every shown layer is a frame is those frames with no margin, on the page's fill
/// only when the page names one (E-export-4); any other page is its content with 32 px of its
/// fill around it; a selection is on transparency (TM-26).
fn target(scene: &Scene, page: &str, ids: &[String], whole: bool) -> (Vec<String>, bool) {
    if !whole {
        return (ids.to_vec(), false);
    }
    let shown = shown_children(scene, page);
    if !shown.is_empty() && shown.iter().all(|id| scene.node(id).is_some_and(|n| n.kind == NodeKind::Frame)) {
        let filled = scene.node(page).is_some_and(|p| p.fill.is_some());
        (shown, filled)
    } else {
        (Vec::new(), true)
    }
}

/// While set, the painter draws for a vector file (TM-21); it is put back however the drawing
/// ends.
struct ForVector<'a>(&'a Painter);

impl<'a> ForVector<'a> {
    fn new(painter: &'a Painter) -> Self {
        painter.vector.set(true);
        ForVector(painter)
    }
}

impl Drop for ForVector<'_> {
    fn drop(&mut self) {
        self.0.vector.set(false);
    }
}

/// The board's page pictured as a still shows it, every scrolling frame at its start however
/// the canvas is scrolled (feature-layout §2.5): `ids` (none: the page) at `scale`.
fn picture(b: &Board, ids: &[String], backdrop: bool, scale: f64) -> Option<(gsk::RenderNode, Rect)> {
    let placed = b.still_placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    picture_of(&b.painter, &scene, &placed, &page, ids, backdrop, scale)
}

/// The area of `ids` (none: the page) on the board's page as a still shows it: what a picture
/// of them covers, to choose its scale and its name before it is drawn.
pub fn extent(b: &Board, ids: &[String]) -> Option<Rect> {
    let placed = b.still_placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    area_of(&b.painter, &scene, &placed, &page, ids)
}

/// `ids` (none: the page) rendered as a texture at `scale`, which the caller chose for their
/// area ([`extent`]), and that area.
pub fn render(b: &Board, ids: &[String], backdrop: bool, scale: f64) -> Result<(gdk::Texture, Rect), Failure> {
    let renderer = b.canvas.native().and_then(|n| n.renderer()).ok_or(Failure::NoRenderer)?;
    let (node, area) = picture(b, ids, backdrop, scale).ok_or(Failure::Empty)?;
    let (w, h) = boxes::picture_size(&area, scale);
    let viewport = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
    Ok((renderer.render_texture(&node, Some(&viewport)), area))
}

/// `ids` (none: the page) at `at_ms` of the page's motion, as a still shows the page, at
/// `scale` within 8192 px a side: the texture, its area (wherever the movers are then) and the
/// scale drawn at.
pub fn render_at(b: &Board, ids: &[String], backdrop: bool, scale: f64, at_ms: f64) -> Result<(gdk::Texture, Rect, f64), Failure> {
    let renderer = b.canvas.native().and_then(|n| n.renderer()).ok_or(Failure::NoRenderer)?;
    let placed = b.still_placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    let clocks = clocks_of(&b.painter, &scene, &page);
    let area = posed_area(&*b.painter, &scene, &placed, &page, ids, &clocks, &[at_ms]).ok_or(Failure::Empty)?;
    let scale = boxes::fit_scale(&area, scale, MAX_PIXELS, f64::INFINITY);
    let posing = posing_at(&scene, &clocks, at_ms);
    let node = picture_in(&b.painter, &scene, &placed, &page, ids, backdrop, scale, &area, Some(posing));
    // What was kept of the still layers is for these clocks alone.
    b.painter.clear_stills();
    let node = node.ok_or(Failure::Empty)?;
    let (w, h) = boxes::picture_size(&area, scale);
    Ok((renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w as f32, h as f32))), area, scale))
}

/// A contact sheet of `ids` (none: the page): `frames` pictures evenly over the span of what
/// moves in them, from its start to its end, each labelled with its time, `ceil(√frames)`
/// across on the page's canvas colour; drawn at `scale`, lowered until the sheet is within
/// `max_side` px a side and 4 megapixels. The texture and each cell's time, in reading order;
/// `Failure::Still` when nothing in them moves.
pub fn contact_sheet(b: &Board, ids: &[String], backdrop: bool, scale: f64, max_side: u32, frames: u8) -> Result<(gdk::Texture, Vec<f64>), Failure> {
    let renderer = b.canvas.native().and_then(|n| n.renderer()).ok_or(Failure::NoRenderer)?;
    let placed = b.still_placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    let clocks = clocks_of(&b.painter, &scene, &page);
    let (span, _) = span_of(&scene, &clocks, &page, ids);
    if span <= 0.0 {
        return Err(Failure::Still);
    }
    let times = motion::sheet_times(span, frames);
    let area = posed_area(&*b.painter, &scene, &placed, &page, ids, &clocks, &times).ok_or(Failure::Empty)?;
    let (sheet, scale) = sheet_fit(&area, times.len(), scale, max_side, boxes::MAX_AREA_PX);
    let cells: Vec<gsk::RenderNode> = times
        .iter()
        .map(|t| or_blank(picture_in(&b.painter, &scene, &placed, &page, ids, backdrop, scale, &area, Some(posing_at(&scene, &clocks, *t)))))
        .collect();
    b.painter.clear_stills();
    let node = sheet_node(&b.canvas, &sheet, &cells, &times, &sheet_ground(&scene, &page)).ok_or(Failure::Empty)?;
    let (w, h) = sheet.size();
    Ok((renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w as f32, h as f32))), times))
}

/// The scale Ask pictures `area` at, chosen for a model: its long side near 1,568 px, from a
/// quarter to four times its size, within 8,192 px a side and 4 megapixels (I-8, TM-26).
pub fn ask_scale(area: &Rect) -> f64 {
    let want = (1568.0 / area.w.max(area.h).max(1.0)).clamp(0.25, 4.0);
    boxes::fit_scale(area, want, MAX_PIXELS, boxes::MAX_AREA_PX)
}

/// What an export of the page draws, as the Export keys do, and whether on its ground: the
/// smoke harness's file steps.
pub(super) fn page_target(b: &Board) -> (Vec<String>, bool) {
    let scene = b.doc.scene.borrow();
    target(&scene, &b.page.borrow(), &[], true)
}

/// `ids` (or the page, `page`) as an SVG or PDF document, as the export menu draws it by
/// default: the smoke harness's `svg` and `pdf` steps.
pub(super) fn vector(b: &Board, ids: &[String], page: bool, format: Format) -> Result<Vec<u8>, String> {
    let (shown, backdrop) = {
        let scene = b.doc.scene.borrow();
        target(&scene, &b.page.borrow(), ids, page)
    };
    let title = file_title(b, ids, page);
    vector_of(b, &shown, backdrop, format, &title).map(|(bytes, _)| bytes)
}

/// `ids` (none: the page) as an SVG or PDF document titled `title`, and the scale it is drawn
/// at.
fn vector_of(b: &Board, ids: &[String], backdrop: bool, format: Format, title: &str) -> Result<(Vec<u8>, f64), String> {
    let area = extent(b, ids).ok_or(Failure::Empty.message())?;
    let scale = format.scale(&area);
    let (node, area) = {
        let _vector = ForVector::new(&b.painter);
        picture(b, ids, backdrop, scale).ok_or(Failure::Empty.message())?
    };
    document(&node, area.w * scale, area.h * scale, format, title).map(|bytes| (bytes, scale))
}

/// `node`, drawn from the origin, as an SVG or PDF document `w` by `h` px (or pt) titled
/// `title`. Its surface is as large as what is drawn, whatever the scale (TM-23).
fn document(node: &gsk::RenderNode, w: f64, h: f64, format: Format, title: &str) -> Result<Vec<u8>, String> {
    let draw = |surface: &cairo::Surface| draw_on(surface, node);
    let bytes = match format {
        Format::Svg => {
            let mut surface = cairo::SvgSurface::for_stream(w, h, Vec::<u8>::new()).map_err(|e| e.to_string())?;
            surface.set_document_unit(cairo::SvgUnit::Px);
            draw(&surface)?;
            surface.finish_output_stream().map_err(|e| e.to_string())?
        }
        _ => {
            let surface = cairo::PdfSurface::for_stream(w, h, Vec::<u8>::new()).map_err(|e| e.to_string())?;
            // The file says what it is and what made it (E-export-7).
            let _ = surface.set_metadata(cairo::PdfMetadata::Title, title);
            let _ = surface.set_metadata(cairo::PdfMetadata::Creator, "Relay");
            if let Some(now) = glib::DateTime::now_local().ok().and_then(|t| t.format("%Y-%m-%dT%H:%M:%S%:z").ok()) {
                let _ = surface.set_metadata(cairo::PdfMetadata::CreateDate, &now);
            }
            draw(&surface)?;
            surface.finish_output_stream().map_err(|e| e.to_string())?
        }
    };
    bytes.downcast::<Vec<u8>>().map(|b| *b).map_err(|_| "the surface kept its bytes".into())
}

/// Draw `node` on a vector `surface` as cairo would, except its blurs and soft shadows, which
/// a vector file carries only as images and GSK pictures at 72 ppi: those are pictured at
/// `VECTOR_DETAIL` times that, each an image of its own (E-export-5).
fn draw_on(surface: &cairo::Surface, node: &gsk::RenderNode) -> Result<(), String> {
    let cr = cairo::Context::new(surface).map_err(|e| e.to_string())?;
    draw_vector(&cr, node);
    Ok(())
}

/// `draw_on`'s walk: into containers, 2D transforms, clips, opacity and debug nodes, so a blur
/// or shadow inside them is found; anything else drawn by GSK.
fn draw_vector(cr: &cairo::Context, node: &gsk::RenderNode) {
    use gsk::TransformCategory as T;
    if let Some(c) = node.downcast_ref::<gsk::ContainerNode>() {
        for i in 0..c.n_children() {
            draw_vector(cr, &c.child(i));
        }
    } else if let Some(t) = node.downcast_ref::<gsk::TransformNode>() {
        let transform = t.transform();
        if !matches!(transform.category(), T::_2d | T::_2dAffine | T::_2dTranslate | T::Identity) {
            return node.draw(cr);
        }
        let (xx, yx, xy, yy, dx, dy) = transform.to_2d();
        let _ = cr.save();
        cr.transform(cairo::Matrix::new(xx.into(), yx.into(), xy.into(), yy.into(), dx.into(), dy.into()));
        draw_vector(cr, &t.child());
        let _ = cr.restore();
    } else if let Some(c) = node.downcast_ref::<gsk::ClipNode>() {
        let r = c.clip();
        let _ = cr.save();
        cr.rectangle(r.x().into(), r.y().into(), r.width().into(), r.height().into());
        cr.clip();
        draw_vector(cr, &c.child());
        let _ = cr.restore();
    } else if let Some(c) = node.downcast_ref::<gsk::RoundedClipNode>() {
        let _ = cr.save();
        rounded_path(cr, &c.clip());
        cr.clip();
        draw_vector(cr, &c.child());
        let _ = cr.restore();
    } else if let Some(o) = node.downcast_ref::<gsk::OpacityNode>() {
        cr.push_group();
        draw_vector(cr, &o.child());
        if cr.pop_group_to_source().is_ok() {
            let _ = cr.paint_with_alpha(o.opacity().into());
        }
    } else if let Some(d) = node.downcast_ref::<gsk::DebugNode>() {
        draw_vector(cr, &d.child());
    } else if soft(node) {
        raster(cr, node);
    } else {
        node.draw(cr);
    }
}

/// Whether `node` is a blur or a blurred shadow: what cairo carries as an image.
fn soft(node: &gsk::RenderNode) -> bool {
    node.downcast_ref::<gsk::BlurNode>().is_some_and(|b| b.radius() > 0.0)
        || node.downcast_ref::<gsk::OutsetShadowNode>().is_some_and(|s| s.blur_radius() > 0.0)
        || node.downcast_ref::<gsk::InsetShadowNode>().is_some_and(|s| s.blur_radius() > 0.0)
}

/// `node` pictured at `VECTOR_DETAIL` times the context's scale (at most 8192 px a side) and
/// painted where it lies, unrepeated, so a vector file holds it as a plain image (TM-21).
fn raster(cr: &cairo::Context, node: &gsk::RenderNode) {
    let b = node.bounds();
    let (x, y, w, h) = (f64::from(b.x()), f64::from(b.y()), f64::from(b.width()), f64::from(b.height()));
    let m = cr.matrix();
    let zoom = (m.xx() * m.yy() - m.xy() * m.yx()).abs().sqrt();
    let k = (VECTOR_DETAIL * zoom).min(8192.0 / w.max(h).max(1.0));
    if !(k > 0.0 && w > 0.0 && h > 0.0) {
        return node.draw(cr);
    }
    let Ok(surface) = cairo::ImageSurface::create(cairo::Format::ARgb32, (w * k).ceil() as i32, (h * k).ceil() as i32) else {
        return node.draw(cr);
    };
    if let Ok(image) = cairo::Context::new(&surface) {
        image.scale(k, k);
        image.translate(-x, -y);
        node.draw(&image);
    }
    surface.flush();
    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(1.0 / k, 1.0 / k);
    let pattern = cairo::SurfacePattern::create(&surface);
    pattern.set_extend(cairo::Extend::None);
    pattern.set_filter(cairo::Filter::Good);
    if cr.set_source(&pattern).is_ok() {
        let _ = cr.paint();
    }
    let _ = cr.restore();
}

/// A rounded rectangle's outline as the context's path: each corner an elliptical arc.
fn rounded_path(cr: &cairo::Context, rr: &gsk::RoundedRect) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let b = rr.bounds();
    let (x, y, w, h) = (f64::from(b.x()), f64::from(b.y()), f64::from(b.width()), f64::from(b.height()));
    let c = rr.corner().map(|s| (f64::from(s.width()), f64::from(s.height())));
    let arc = |cx: f64, cy: f64, (rx, ry): (f64, f64), from: f64| {
        if rx <= 0.0 || ry <= 0.0 {
            return cr.line_to(cx, cy);
        }
        let _ = cr.save();
        cr.translate(cx, cy);
        cr.scale(rx, ry);
        cr.arc(0.0, 0.0, 1.0, from, from + FRAC_PI_2);
        let _ = cr.restore();
    };
    cr.new_sub_path();
    arc(x + c[0].0, y + c[0].1, c[0], PI);
    arc(x + w - c[1].0, y + c[1].1, c[1], -FRAC_PI_2);
    arc(x + w - c[2].0, y + h - c[2].1, c[2], 0.0);
    arc(x + c[3].0, y + h - c[3].1, c[3], FRAC_PI_2);
    cr.close_path();
}

/// The "@2x" a PNG's file name carries for the scale it is drawn at, which a large picture
/// lowers (B100): none at 1×, else the scale to two places without trailing zeros.
fn png_suffix(scale: f64) -> String {
    if (scale - 1.0).abs() < 1e-6 {
        return String::new();
    }
    let s = format!("{scale:.2}");
    format!("@{}x", s.trim_end_matches('0').trim_end_matches('.'))
}

/// A file's name: `name` as a stem that fits a file system (B42, B101), the scale's suffix
/// and the format's extension.
fn file_name(name: &str, suffix: &str, format: Format) -> String {
    format!("{}{}.{}", names::file_stem(name, STEM_BYTES), suffix, format.extension())
}

/// What an export is called: the page's name, the one layer's, or the design's selection.
fn file_title(b: &Board, ids: &[String], page: bool) -> String {
    if page {
        return b.page_name(&b.page.borrow());
    }
    let scene = b.doc.scene.borrow();
    match ids {
        [one] => scene.node(one).map(display_name).unwrap_or_else(|| "Selection".into()),
        _ => format!("{} selection", b.doc.title()),
    }
}

pub fn export_selection(b: &Board, page: bool, format: Format) {
    // The words being typed are committed first, so they are in the picture (TM-30).
    b.end_edit(true);
    let ids = if page { Vec::new() } else { b.selected() };
    if !page && ids.is_empty() {
        b.toast("Select what to export first");
        return;
    }
    save(&b.me(), ids, format, page);
}

pub fn export_ids(b: &Rc<Board>, ids: Vec<String>, format: Format) {
    b.end_edit(true);
    save(b, ids, format, false);
}

/// Export `ids` (or the page, `page`). What it covers is found first, so the dialog opens only
/// for something to export and offers a name with the scale it will be drawn at; it is drawn
/// once a file is chosen, from the design as it is then (B82). A PNG is encoded off the main
/// thread; a vector document is drawn here, being quick.
fn save(b: &Rc<Board>, ids: Vec<String>, format: Format, page: bool) {
    let (shown, backdrop) = {
        let scene = b.doc.scene.borrow();
        target(&scene, &b.page.borrow(), &ids, page)
    };
    let Some(area) = extent(b, &shown) else {
        b.toast(Failure::Empty.message());
        return;
    };
    if matches!(format, Format::Png(_)) && b.canvas.native().and_then(|n| n.renderer()).is_none() {
        b.toast(Failure::NoRenderer.message());
        return;
    }
    let name = file_title(b, &ids, page);
    let suffix = match format {
        Format::Png(_) => png_suffix(format.scale(&area)),
        _ => String::new(),
    };
    let initial = file_name(&name, &suffix, format);
    let title = format!("Export as {}", format.extension().to_uppercase());
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let Some(rt) = b.ui.upgrade().map(|ui| ui.rt.clone()) else {
        return;
    };
    let weak = Rc::downgrade(b);
    glib::spawn_future_local(async move {
        let Some((file, clear)) = choose_file(parent, &title, &initial, !backdrop).await else {
            return;
        };
        let Some(b) = weak.upgrade() else {
            return;
        };
        // An agent may have changed the design while the dialog was open.
        let (shown, _) = {
            let scene = b.doc.scene.borrow();
            target(&scene, &b.page.borrow(), &ids, page)
        };
        let unpictured = wait_for_pictures(&b, &shown).await;
        let mut notes = Vec::new();
        if unpictured > 0 {
            notes.push(format!("{unpictured} picture{} could not be drawn", if unpictured == 1 { "" } else { "s" }));
        }
        let bytes = match format {
            Format::Png(asked) => {
                let drawn = extent(&b, &shown)
                    .map(|area| format.scale(&area))
                    .ok_or(Failure::Empty)
                    .and_then(|scale| render(&b, &shown, !clear, scale).map(|(texture, _)| (texture, scale)));
                match drawn {
                    Ok((texture, scale)) => {
                        if scale < asked - 1e-6 {
                            notes.push(format!("drawn at {scale:.2}×, as a picture is at most {MAX_PIXELS} px a side"));
                        }
                        rt.spawn_blocking(move || texture.save_to_png_bytes()).await.map_err(|e| e.to_string())
                    }
                    Err(e) => Err(e.message().to_string()),
                }
            }
            _ => vector_of(&b, &shown, !clear, format, &name).map(|(bytes, scale)| {
                if scale < 1.0 {
                    notes.push("scaled to fit a 200 in page".into());
                }
                glib::Bytes::from_owned(bytes)
            }),
        };
        drop(b);
        let saved = match bytes {
            Ok(bytes) => file
                .replace_contents_future(bytes, None, false, gio::FileCreateFlags::REPLACE_DESTINATION)
                .await
                .map(|_| ())
                .map_err(|(_, e)| e.to_string()),
            Err(e) => Err(e),
        };
        if let Some(b) = weak.upgrade() {
            match saved {
                Ok(()) => {
                    let named = file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let notes = if notes.is_empty() { String::new() } else { format!("; {}", notes.join("; ")) };
                    b.toast(&format!("Exported {named}{notes}"));
                }
                Err(e) => b.toast(&format!("Could not export: {e}")),
            }
        }
    });
}

/// Ask where to save `name`, with a "Transparent background" choice set to `clear` (TM-26,
/// E-export-4): the file and the choice, or `None` when cancelled. GtkFileDialog takes no
/// choices, so this is the file chooser that carries them to the desktop's portal.
#[allow(deprecated)]
async fn choose_file(parent: Option<gtk::Window>, title: &str, name: &str, clear: bool) -> Option<(gio::File, bool)> {
    let chooser = gtk::FileChooserNative::new(Some(title), parent.as_ref(), gtk::FileChooserAction::Save, Some("Export"), Some("Cancel"));
    chooser.set_modal(true);
    chooser.set_current_name(name);
    chooser.add_choice("transparent", "Transparent background", &[]);
    chooser.set_choice("transparent", if clear { "true" } else { "false" });
    let (tx, rx) = async_channel::bounded(1);
    chooser.connect_response(move |_, response| {
        let _ = tx.try_send(response);
    });
    chooser.show();
    let response = rx.recv().await.ok()?;
    if response != gtk::ResponseType::Accept {
        return None;
    }
    let clear = chooser.choice("transparent").is_some_and(|c| c == "true");
    Some((chooser.file()?, clear))
}

// ------------------------------------------------------------------ animation

thread_local! {
    /// An animation is being drawn: one at a time, as each frame takes the main loop's idle turns.
    static ANIMATING: Cell<bool> = const { Cell::new(false) };
}

/// Clears [`ANIMATING`] however the drawing ends.
struct Animating;

impl Animating {
    fn start() -> Option<Animating> {
        (!ANIMATING.with(|a| a.replace(true))).then_some(Animating)
    }
}

impl Drop for Animating {
    fn drop(&mut self) {
        ANIMATING.with(|a| a.set(false));
    }
}

/// An animation drawn: how many frames, whether its motion loops, its size in px, and how many
/// pictures were drawn as placeholders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Drawn {
    pub count: usize,
    pub looped: bool,
    pub size: (u32, u32),
    pub unpictured: usize,
}

/// The frames an animation of what `ids` (none: the page) hold has at `fps`, and whether it
/// loops: refused when nothing there moves or past 600 frames.
fn frames_planned(b: &Board, ids: &[String], fps: u16) -> Result<(Vec<f64>, bool), String> {
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    let clocks = clocks_of(&b.painter, &scene, &page);
    let (span, looped) = span_of(&scene, &clocks, &page, ids);
    frame_plan(span, looped, fps)
}

/// The frame times of a motion `span_ms` long at `fps`, refused when it is still or too long.
fn frame_plan(span_ms: f64, looped: bool, fps: u16) -> Result<(Vec<f64>, bool), String> {
    if span_ms <= 0.0 {
        return Err(Failure::Still.message().into());
    }
    let times = motion::frame_times(span_ms, looped, fps);
    if times.len() > apng::MAX_FRAMES {
        let most = apng::MAX_FRAMES as f64 / f64::from(fps.max(1));
        return Err(format!("Animations export up to {} frames ({most:.0} s at {fps} fps); this one has {}", apng::MAX_FRAMES, times.len()));
    }
    Ok((times, looped))
}

/// Draw the motion of `ids` (none: the page) as PNG frames at `fps`, at 1× within 8192 px a
/// side and 4 megapixels, from the design as it is when called. One frame is drawn per idle
/// turn of the main loop, so input and the canvas's own frames go between them, and each is
/// encoded off the main thread, a few at a time. `progress` hears each frame drawn, of all.
pub(super) async fn draw_animation(b: &Rc<Board>, ids: Vec<String>, backdrop: bool, fps: u16, progress: impl Fn(&Board, usize, usize)) -> Result<(Vec<Vec<u8>>, Drawn), String> {
    let Some(_running) = Animating::start() else {
        return Err("An animation is already being exported; wait for it to finish".into());
    };
    let rt = b.ui.upgrade().map(|ui| ui.rt.clone()).ok_or("the app is closing")?;
    let unpictured = wait_for_pictures(b, &ids).await;
    // The design as it is now: an agent's change while the frames are drawn waits for the next
    // export, so the frames agree.
    let (scene, placed, page) = (b.doc.scene.borrow().clone(), b.still_placed(), b.page.borrow().clone());
    let clocks = clocks_of(&b.painter, &scene, &page);
    let (span, looped) = span_of(&scene, &clocks, &page, &ids);
    let (times, looped) = frame_plan(span, looped, fps)?;
    let area = posed_area(&*b.painter, &scene, &placed, &page, &ids, &clocks, &times).ok_or(Failure::Empty.message())?;
    let scale = boxes::fit_scale(&area, 1.0, MAX_PIXELS, boxes::MAX_AREA_PX);
    let (w, h) = boxes::picture_size(&area, scale);
    let viewport = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
    let weak: Weak<Board> = Rc::downgrade(b);
    let mut encoding = VecDeque::new();
    let mut frames = Vec::with_capacity(times.len());
    for (i, t) in times.iter().enumerate() {
        glib::timeout_future_with_priority(glib::Priority::DEFAULT_IDLE, Duration::ZERO).await;
        let b = weak.upgrade().ok_or("the board closed")?;
        let renderer = b.canvas.native().and_then(|n| n.renderer()).ok_or(Failure::NoRenderer.message())?;
        let node = or_blank(picture_in(&b.painter, &scene, &placed, &page, &ids, backdrop, scale, &area, Some(posing_at(&scene, &clocks, *t))));
        let texture = renderer.render_texture(&node, Some(&viewport));
        let (fw, fh) = (texture.width() as u32, texture.height() as u32);
        let (bytes, stride) = rgba_of(&texture);
        progress(&b, i + 1, times.len());
        encoding.push_back(rt.spawn_blocking(move || png_of(fw, fh, &bytes, stride)));
        if encoding.len() >= ENCODING {
            if let Some(oldest) = encoding.pop_front() {
                frames.push(oldest.await.map_err(|e| e.to_string())?);
            }
        }
    }
    if let Some(b) = weak.upgrade() {
        b.painter.clear_stills();
    }
    for left in encoding {
        frames.push(left.await.map_err(|e| e.to_string())?);
    }
    Ok((frames, Drawn { count: times.len(), looped, size: (w, h), unpictured }))
}

/// Toast an animation's progress on `b` every few frames.
fn say_progress(b: &Board, done: usize, of: usize) {
    if done == 1 || done.is_multiple_of(PROGRESS_EVERY) || done == of {
        b.toast(&format!("Drawing the animation: frame {done} of {of}…"));
    }
}

/// The APNG of what `ids` (none: the page) hold, at `fps`: it plays forever when its motion
/// loops, else once, ending on its last frame. The bytes and the frame count.
pub(super) async fn animation_bytes(b: &Rc<Board>, ids: Vec<String>, backdrop: bool, fps: u16) -> Result<(Vec<u8>, Drawn), String> {
    let rt = b.ui.upgrade().map(|ui| ui.rt.clone()).ok_or("the app is closing")?;
    let (frames, drawn) = draw_animation(b, ids, backdrop, fps, say_progress).await?;
    let plays = if drawn.looped { 0 } else { 1 };
    let bytes = rt.spawn_blocking(move || apng::assemble(&frames, fps, plays)).await.map_err(|e| e.to_string())??;
    Ok((bytes, drawn))
}

/// "Export … as animation (APNG)": the selection's motion (or the page's, `page`) as one
/// animated PNG at 30 fps. Whether it moves and how many frames it has are checked before the
/// dialog; it is drawn once a file is chosen, from the design as it is then (B82), without
/// stopping the canvas.
pub fn export_animation(b: &Board, page: bool) {
    let Some(Chosen { b, ids, shown, backdrop }) = animation_target(b, page) else {
        return;
    };
    let name = file_title(&b, &ids, page);
    let initial = file_name(&name, "", Format::Png(1.0));
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let weak = Rc::downgrade(&b);
    glib::MainContext::default().spawn_local_with_priority(glib::Priority::DEFAULT_IDLE, async move {
        let Some((file, clear)) = choose_file(parent, "Export animation (APNG)", &initial, !backdrop).await else {
            return;
        };
        let Some(b) = weak.upgrade() else {
            return;
        };
        let started = std::time::Instant::now();
        let made = animation_bytes(&b, shown, !clear, ANIMATION_FPS).await;
        drop(b);
        let saved = match made {
            Ok((bytes, drawn)) => file
                .replace_contents_future(bytes, None, false, gio::FileCreateFlags::REPLACE_DESTINATION)
                .await
                .map(|_| drawn)
                .map_err(|(_, e)| e.to_string()),
            Err(e) => Err(e),
        };
        if let Some(b) = weak.upgrade() {
            match saved {
                Ok(drawn) => {
                    let named = file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    b.toast(&format!("Exported {named}: {}{}", animation_note(&drawn, ANIMATION_FPS), pictures_note(drawn.unpictured)));
                    tracing::debug!(ms = started.elapsed().as_millis() as u64, "animation exported");
                }
                Err(e) => b.toast(&format!("Could not export the animation: {e}")),
            }
        }
    });
}

/// "Export … as PNG frames": the motion as numbered PNGs (`name-0000.png`, …) at 30 fps in a
/// folder chosen for them.
pub fn export_frames(b: &Board, page: bool) {
    let Some(Chosen { b, ids, shown, backdrop }) = animation_target(b, page) else {
        return;
    };
    let stem = names::file_stem(&file_title(&b, &ids, page), STEM_BYTES);
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let weak = Rc::downgrade(&b);
    glib::MainContext::default().spawn_local_with_priority(glib::Priority::DEFAULT_IDLE, async move {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Export frames into a folder");
        let Ok(folder) = dialog.select_folder_future(parent.as_ref()).await else {
            return;
        };
        let (Some(b), Some(dir)) = (weak.upgrade(), folder.path()) else {
            return;
        };
        let written = write_frames(&b, shown, backdrop, ANIMATION_FPS, dir, stem).await;
        match written {
            Ok((count, drawn)) => b.toast(&format!("Exported {count} frames{}", pictures_note(drawn.unpictured))),
            Err(e) => b.toast(&format!("Could not export the frames: {e}")),
        }
    });
}

/// The motion of `ids` (none: the page) as `stem-0000.png`, … in `dir`, written off the main
/// thread: how many, and what was drawn.
pub(super) async fn write_frames(b: &Rc<Board>, ids: Vec<String>, backdrop: bool, fps: u16, dir: std::path::PathBuf, stem: String) -> Result<(usize, Drawn), String> {
    let rt = b.ui.upgrade().map(|ui| ui.rt.clone()).ok_or("the app is closing")?;
    let (frames, drawn) = draw_animation(b, ids, backdrop, fps, say_progress).await?;
    let count = frames.len();
    rt.spawn_blocking(move || {
        frames.iter().enumerate().try_for_each(|(i, png)| std::fs::write(dir.join(frame_name(&stem, i)), png).map_err(|e| e.to_string()))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok((count, drawn))
}

/// Frame `i`'s file: four digits, so the files sort in order up to 600 frames and more.
fn frame_name(stem: &str, i: usize) -> String {
    format!("{stem}-{i:04}.png")
}

/// What an animation export covers: the board, the selection, what is drawn (none: the page)
/// and whether on its ground by default.
struct Chosen {
    b: Rc<Board>,
    ids: Vec<String>,
    shown: Vec<String>,
    backdrop: bool,
}

/// What an animation export covers, checked before its dialog; `None`, said in a toast, when
/// nothing is chosen, it does not move, or it is too long.
fn animation_target(b: &Board, page: bool) -> Option<Chosen> {
    b.end_edit(true);
    let ids = if page { Vec::new() } else { b.selected() };
    if !page && ids.is_empty() {
        b.toast("Select what to export first");
        return None;
    }
    let (shown, backdrop) = {
        let scene = b.doc.scene.borrow();
        target(&scene, &b.page.borrow(), &ids, page)
    };
    if let Err(e) = frames_planned(b, &shown, ANIMATION_FPS) {
        b.toast(&e);
        return None;
    }
    if b.canvas.native().and_then(|n| n.renderer()).is_none() {
        b.toast(Failure::NoRenderer.message());
        return None;
    }
    Some(Chosen { b: b.me(), ids, shown, backdrop })
}

/// "90 frames, 3.0 s, looping" for a toast.
fn animation_note(drawn: &Drawn, fps: u16) -> String {
    let n = drawn.count;
    let secs = n as f64 / f64::from(fps.max(1));
    format!("{n} frames, {secs:.1} s, {}", if drawn.looped { "looping" } else { "playing once" })
}

/// "; 2 pictures could not be drawn", or nothing.
fn pictures_note(unpictured: usize) -> String {
    match unpictured {
        0 => String::new(),
        n => format!("; {n} picture{} could not be drawn", if n == 1 { "" } else { "s" }),
    }
}

/// Wait, up to 3 s, for the pictures `ids` (none: the page) show to load, and count those that
/// will be drawn as placeholders: still loading, or with no file to read (E-export-2).
pub(super) async fn wait_for_pictures(b: &Board, ids: &[String]) -> usize {
    let roots = |b: &Board| if ids.is_empty() { vec![b.page.borrow().clone()] } else { ids.to_vec() };
    let started = std::time::Instant::now();
    loop {
        let loading = {
            let scene = b.doc.scene.borrow();
            roots(b).iter().any(|r| b.painter.loading_under(&scene, r))
        };
        if !loading || started.elapsed() >= PICTURE_WAIT {
            break;
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
    let scene = b.doc.scene.borrow();
    let dir = std::path::PathBuf::from(&*b.painter.assets_dir.borrow());
    let unread = |id: &str, src: Option<&str>| src.is_none_or(|s| s.is_empty() || !dir.join(s).is_file()) || b.painter.loading_under(&scene, id);
    roots(b)
        .iter()
        .flat_map(|r| scene.subtree(r))
        .filter(|id| scene.node(id).is_some_and(|n| n.kind == NodeKind::Image && !n.is_hidden() && unread(id, n.src.as_deref())))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::design::Node;

    fn node(id: &str, parent: Option<&str>, kind: NodeKind, rect: [f64; 4]) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        [n.x, n.y, n.w, n.h] = rect;
        n
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_png_is_named_for_the_scale_it_is_drawn_at() {
        // B100: a page past 4096 px is drawn below 2× and was still named @2x.
        assert_eq!(png_suffix(2.0), "@2x");
        assert_eq!(png_suffix(1.0), "");
        assert_eq!(png_suffix(1.3653), "@1.37x");
        assert_eq!(png_suffix(0.5), "@0.5x");
        assert_eq!(png_suffix(Format::Png(2.0).scale(&Rect::new(0.0, 0.0, 6000.0, 100.0))), "@1.37x");
    }

    #[test]
    fn a_file_name_fits_the_file_system() {
        // B101 and B42: a long CJK name, a name of slashes and breaks, and blank or hidden ones.
        let name = file_name(&"字".repeat(200), "@1.37x", Format::Png(2.0));
        assert!(name.len() <= 255 && name.ends_with("@1.37x.png"), "{name}");
        assert_eq!(file_name("a/b\\c\nd", "", Format::Svg), "a-b-c-d.svg");
        assert_eq!(file_name(" \u{200B} ", "", Format::Pdf), "Design.pdf");
        assert_eq!(file_name(".hidden", "@2x", Format::Png(2.0)), "hidden@2x.png");
    }

    #[test]
    fn each_format_has_its_own_largest_side() {
        // B46 and TM-23: vector files were drawn at the PNG's cap and shrank into a corner.
        let wide = Rect::new(0.0, 0.0, 20_000.0, 3000.0);
        assert_eq!(Format::Svg.scale(&wide), 1.0);
        let pdf = Format::Pdf.scale(&wide);
        assert!((pdf - 0.72).abs() < 1e-6 && boxes::picture_size(&wide, pdf).0 <= PDF_MAX_PT, "{pdf}");
        let png = Format::Png(2.0).scale(&wide);
        assert!(boxes::picture_size(&wide, png).0 <= MAX_PIXELS && png > 0.4, "{png}");
        assert_eq!(Format::Png(2.0).scale(&Rect::new(0.0, 0.0, 300.0, 200.0)), 2.0);
    }

    #[test]
    fn a_vector_file_is_as_large_as_what_it_draws() {
        // TM-23: a 10,000 px frame makes an SVG 10,000 px wide.
        let area = Rect::new(0.0, 0.0, 10_000.0, 2000.0);
        let node: gsk::RenderNode = gsk::ColorNode::new(&gdk::RGBA::RED, &grect(&area)).upcast();
        let svg = String::from_utf8(document(&node, area.w, area.h, Format::Svg, "Wide").unwrap()).unwrap();
        assert!(svg.contains("width=\"10000px\""), "{}", &svg[..300.min(svg.len())]);
        // Twice as wide, a PDF page is 14,400 pt and the drawing, scaled with it, fills it.
        let big = Rect::new(0.0, 0.0, 20_000.0, 2000.0);
        let scale = Format::Pdf.scale(&big);
        let drawn = Rect::new(0.0, 0.0, big.w * scale, big.h * scale);
        let node: gsk::RenderNode = gsk::ColorNode::new(&gdk::RGBA::RED, &grect(&drawn)).upcast();
        let pdf = document(&node, drawn.w, drawn.h, Format::Pdf, "Wide page").unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let rec = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None).unwrap();
        draw_on(&rec, &node).unwrap();
        assert_eq!(rec.ink_extents(), (0.0, 0.0, 14_400.0, 1440.0), "what is drawn fills the page, with no empty band");
        // The page's size and E-export-7's title, where poppler is at hand to read them.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch");
        let path = dir.join(format!("export-test-{}.pdf", std::process::id()));
        std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &pdf)).unwrap();
        let info = std::process::Command::new("pdfinfo").arg(&path).output();
        let _ = std::fs::remove_file(&path);
        if let Some(info) = info.ok().filter(|o| o.status.success()) {
            let info = String::from_utf8_lossy(&info.stdout);
            assert!(info.contains("14400 x 1440 pts") && info.contains("Wide page") && info.contains("Relay"), "{info}");
        }
    }

    #[test]
    fn a_vector_files_blurs_are_sharp() {
        // E-export-5: GSK pictures a blur or a soft shadow at 72 ppi on a vector surface.
        let area = Rect::new(20.0, 20.0, 100.0, 100.0);
        let card: gsk::RenderNode = gsk::ColorNode::new(&gdk::RGBA::RED, &grect(&area)).upcast();
        let blurred: gsk::RenderNode = gsk::BlurNode::new(&card, 10.0).upcast();
        let shadow: gsk::RenderNode =
            gsk::OutsetShadowNode::new(&gsk::RoundedRect::from_rect(grect(&area), 8.0), &gdk::RGBA::BLACK, 0.0, 4.0, 0.0, 12.0).upcast();
        let turned = gsk::TransformNode::new(gsk::ContainerNode::new(&[shadow, card.clone()]), Some(&gsk::Transform::new().rotate(10.0)));
        let node: gsk::RenderNode = gsk::ContainerNode::new(&[blurred.clone(), gsk::OpacityNode::new(turned, 0.5).upcast()]).upcast();
        let svg = String::from_utf8(document(&node, 300.0, 300.0, Format::Svg, "Soft").unwrap()).unwrap();
        let widths: Vec<f64> = svg.split("<image ").skip(1).filter_map(|i| i.split("width=\"").nth(1)?.split('"').next()?.parse().ok()).collect();
        assert_eq!(widths.len(), 2, "the blur and the shadow, each an image: {widths:?}");
        let blur_w = f64::from(blurred.bounds().width());
        assert!(widths.iter().all(|w| *w >= 3.0 * 100.0) && widths.contains(&(3.0 * blur_w).ceil()), "{widths:?}");
    }

    #[test]
    fn a_page_of_artboards_exports_snug() {
        // E-export-4: frames alone have no margin, and no ground unless the page names a fill.
        let mut scene = Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            node("a", Some("p1"), NodeKind::Frame, [0.0, 0.0, 100.0, 100.0]),
            node("b", Some("p1"), NodeKind::Frame, [200.0, 0.0, 100.0, 100.0]),
        ]);
        assert_eq!(target(&scene, "p1", &[], true), (ids(&["a", "b"]), false));
        scene.node_mut("p1").unwrap().fill = Some("#FFFFFF".into());
        assert!(target(&scene, "p1", &[], true).1);
        assert!(whole_page(&scene, "p1", &ids(&["a", "b"])), "every frame is the page");
        assert!(!whole_page(&scene, "p1", &ids(&["a"])));
        // Loose content keeps the page's margin and ground; a selection is on transparency.
        let mut ghost = node("ghost", Some("p1"), NodeKind::Rect, [900.0, 0.0, 10.0, 10.0]);
        ghost.opacity = Some(0.0);
        let loose = Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            node("a", Some("p1"), NodeKind::Frame, [0.0, 0.0, 100.0, 100.0]),
            node("s", Some("p1"), NodeKind::Sticky, [200.0, 0.0, 100.0, 100.0]),
            ghost,
        ]);
        assert_eq!(target(&loose, "p1", &[], true), (Vec::new(), true));
        assert_eq!(target(&loose, "p1", &ids(&["s"]), false), (ids(&["s"]), false));
        assert_eq!(shown_children(&loose, "p1"), ids(&["a", "s"]), "a clear layer is not shown (E-export-3)");
    }

    #[test]
    fn a_selection_lies_on_the_ground_it_is_drawn_on() {
        // TM-26: the nearest fill under the whole selection at least half opaque, else the page's.
        let mut card = node("card", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 200.0]);
        card.fill = Some("#FFFFFF".into());
        let mut veil = node("veil", Some("card"), NodeKind::Frame, [0.0, 0.0, 100.0, 100.0]);
        veil.fill = Some("#00000010".into());
        let scene = Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            card,
            veil,
            node("t", Some("veil"), NodeKind::Text, [10.0, 10.0, 50.0, 20.0]),
            node("loose", Some("p1"), NodeKind::Rect, [300.0, 0.0, 10.0, 10.0]),
        ]);
        assert_eq!(ground_under(&scene, "p1", &ids(&["t"])), "#FFFFFF");
        assert_eq!(ground_under(&scene, "p1", &ids(&["loose"])), CANVAS);
        assert_eq!(ground_under(&scene, "p1", &ids(&["t", "loose"])), CANVAS, "only what lies under them all");
    }

    /// Words measured as a 10 px square: these tests draw no words.
    struct Square;

    impl Measure for Square {
        fn measure(&self, _: &relay_bus::design::Node, _: Option<f64>) -> (f64, f64) {
            (10.0, 10.0)
        }
    }

    fn moving(mut n: Node, keys: serde_json::Value) -> Node {
        n.motion = Some(serde_json::from_value(serde_json::json!({ "keys": keys })).unwrap());
        n
    }

    #[test]
    fn a_contact_sheet_reads_left_to_right_then_down() {
        // feature-motion §3: ceil(√n) across, a 16 px gutter, a band for each time.
        let sheet = Sheet::new(6, (100, 50), SHEET_GUTTER, SHEET_LABEL);
        assert_eq!((sheet.cols, sheet.rows), (3, 2));
        assert_eq!(sheet.size(), (3 * 100 + 4 * 16, 2 * (50 + 22) + 3 * 16));
        assert_eq!(sheet.origin(0), (16, 16));
        assert_eq!(sheet.origin(2), (16 + 2 * 116, 16));
        assert_eq!(sheet.origin(4), (16 + 116, 16 + 50 + 22 + 16));
        for (n, cols, rows) in [(2, 2, 1), (4, 2, 2), (5, 3, 2), (10, 4, 3), (24, 5, 5)] {
            let s = Sheet::new(n, (10, 10), 0, 0);
            assert_eq!((s.cols, s.rows), (cols, rows), "{n} frames");
            // The last cell is on the sheet.
            let (x, y) = s.origin(n - 1);
            assert!(x + 10 <= s.size().0 && y + 10 <= s.size().1);
        }
        assert_eq!(time_label(1199.6), "1200 ms");
    }

    #[test]
    fn a_contact_sheet_fits_its_limits_whole() {
        let fits = |s: &Sheet, side: u32, px: f64| {
            let (w, h) = s.size();
            w.max(h) <= side && f64::from(w) * f64::from(h) <= px
        };
        // Small enough: the scale asked for.
        let card = Rect::new(0.0, 0.0, 120.0, 80.0);
        assert_eq!(sheet_fit(&card, 6, 1.0, 1568, boxes::MAX_AREA_PX), (Sheet::new(6, (120, 80), SHEET_GUTTER, SHEET_LABEL), 1.0));
        // A page: lowered until the sheet, gutters and labels with it, fits, and no further.
        let page = Rect::new(-32.0, -32.0, 1504.0, 1064.0);
        for (n, side) in [(6usize, 1568u32), (24, 2560), (2, 64), (12, 1568)] {
            let (sheet, scale) = sheet_fit(&page, n, 2.0, side, boxes::MAX_AREA_PX);
            assert!(scale > 0.0 && scale < 2.0 && fits(&sheet, side, boxes::MAX_AREA_PX), "{n} at {side}: {sheet:?}");
            let bigger = Sheet::new(n, boxes::picture_size(&page, scale * 1.01), sheet.gutter, sheet.label);
            assert!(!fits(&bigger, side, boxes::MAX_AREA_PX), "{n} at {side}: {scale} could be larger");
        }
        // 4 megapixels bounds it too.
        let (sheet, _) = sheet_fit(&page, 6, 2.0, 8192, boxes::MAX_AREA_PX);
        assert!(fits(&sheet, 8192, boxes::MAX_AREA_PX) && sheet.label == SHEET_LABEL);
        // 24 frames in 256 px: the gutters and labels would leave cells 10 px tall, so they go.
        let (small, _) = sheet_fit(&page, 24, 1.0, 256, boxes::MAX_AREA_PX);
        assert!(fits(&small, 256, boxes::MAX_AREA_PX) && small.label == 0 && small.cell.0 >= 45, "{small:?}");
        let (tiny, scale) = sheet_fit(&page, 24, 1.0, 64, boxes::MAX_AREA_PX);
        assert!(fits(&tiny, 64, boxes::MAX_AREA_PX) && tiny.label == 0 && scale > 0.0, "{tiny:?}");
    }

    #[test]
    fn an_animation_has_a_frame_every_thirtieth_of_a_second() {
        // A loop leaves out its end, which is the next loop's start; a one-shot ends on its end.
        let (times, looped) = frame_plan(3000.0, true, 30).unwrap();
        assert!(looped && times.len() == 90 && times[1] > 33.3 && times[1] < 33.4);
        let (times, looped) = frame_plan(3000.0, false, 30).unwrap();
        assert!(!looped && times.len() == 91 && times.last() == Some(&3000.0));
        assert_eq!(frame_plan(0.0, false, 30), Err(Failure::Still.message().to_string()));
        // 20 s at 30 fps is the most; one frame more is refused before any is drawn.
        assert_eq!(frame_plan(20_000.0, true, 30).map(|(t, _)| t.len()), Ok(600));
        let refused = frame_plan(20_100.0, true, 30).unwrap_err();
        assert!(refused.contains("up to 600 frames (20 s at 30 fps)") && refused.contains("603"), "{refused}");
        assert_eq!(frame_name("Hero card", 7), "Hero card-0007.png");
    }

    #[test]
    fn a_moved_layer_is_inside_the_picture() {
        // The area holds every frame: a layer sliding 300 px right widens it; inside a clipping
        // frame, only as far as the frame.
        let slide = serde_json::json!([{"at_ms": 0, "dx": 0}, {"at_ms": 1000, "dx": 300}]);
        let scene = |clip: bool| {
            let mut card = node("card", Some("p"), NodeKind::Frame, [0.0, 0.0, 200.0, 100.0]);
            card.clip = Some(clip);
            Scene::new(vec![node("p", None, NodeKind::Page, [0.0; 4]), card, moving(node("dot", Some("card"), NodeKind::Rect, [10.0, 10.0, 20.0, 20.0]), slide.clone())])
        };
        for clip in [false, true] {
            let scene = scene(clip);
            let placed = relay_client::design::layout::place(&scene, "p", &Square);
            let clocks = Clocks::new(&scene, "p");
            let rest = boxes::area(&scene, &placed, &Square, "p", &[]).unwrap();
            assert_eq!(posed_area(&Square, &scene, &placed, "p", &[], &clocks, &[0.0]), Some(rest), "at its start it is at rest");
            let wide = posed_area(&Square, &scene, &placed, "p", &[], &clocks, &[0.0, 500.0, 1000.0]).unwrap();
            if clip {
                assert_eq!(wide, rest, "the frame clips it");
            } else {
                // 10 + 300 + 20, and the page's 32 px margin.
                assert_eq!(wide.right(), 330.0 + boxes::PAGE_MARGIN, "{wide:?}");
                assert_eq!((wide.x, wide.y, wide.bottom()), (rest.x, rest.y, rest.bottom()));
            }
            // A selection of the dot alone: no margin; its sibling's picture does not move.
            let dot = posed_area(&Square, &scene, &placed, "p", &ids(&["dot"]), &clocks, &[1000.0]).unwrap();
            assert_eq!(dot, if clip { Rect::new(10.0, 10.0, 20.0, 20.0) } else { Rect::new(10.0, 10.0, 320.0, 20.0) });
        }
        // A moving frame carries what it holds; a selection inside it is drawn at rest within it.
        let mut scene = scene(false);
        *scene.node_mut("card").unwrap() = moving(scene.node("card").unwrap().clone(), serde_json::json!([{"at_ms": 0, "dy": 0}, {"at_ms": 400, "dy": 50}]));
        let placed = relay_client::design::layout::place(&scene, "p", &Square);
        let clocks = Clocks::new(&scene, "p");
        let all = posed_area(&Square, &scene, &placed, "p", &[], &clocks, &[400.0]).unwrap();
        assert_eq!(all.bottom(), 100.0 + 50.0 + boxes::PAGE_MARGIN, "{all:?}");
        let dot = posed_area(&Square, &scene, &placed, "p", &ids(&["dot"]), &clocks, &[400.0]).unwrap();
        assert_eq!(dot.y, 10.0, "the frame's own move is outside a selection inside it");
    }

    #[test]
    fn a_pose_moves_what_is_painted_about_its_own_box() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let pose = |f: fn(&mut Pose)| {
            let mut p = Pose::default();
            f(&mut p);
            p
        };
        // A shadow reaches 10 px past the box; doubled about the box's corner, so does it.
        let painted = Rect::new(-10.0, -10.0, 120.0, 120.0);
        let double = pose(|p| {
            p.scale_x = 2.0;
            p.scale_y = 2.0;
            p.origin = [0.0, 0.0];
        });
        assert_eq!(posed_reach(&painted, &r, &double, 0.0), Rect::new(-20.0, -20.0, 240.0, 240.0));
        // A quarter turn about the centre keeps a square; a move and a blur past its own widen it.
        let turned = posed_reach(&r, &r, &pose(|p| p.rotate = 90.0), 0.0);
        assert!((turned.x).abs() < 1e-9 && (turned.w - 100.0).abs() < 1e-9, "{turned:?}");
        let moved = posed_reach(&r, &r, &pose(|p| {
            p.dx = 5.0;
            p.blur = Some(4.0);
        }), 1.0);
        assert_eq!(moved, Rect::new(5.0 - 6.0, -6.0, 112.0, 112.0));
        // A line has no height: still moved, never lost.
        let flat = posed_reach(&Rect::new(0.0, 50.0, 100.0, 0.0), &r, &pose(|p| p.dy = 10.0), 0.0);
        assert!((flat.y - 60.0).abs() < 1e-6 && flat.w == 100.0, "{flat:?}");
        assert_eq!(meet(&Rect::new(0.0, 0.0, 10.0, 10.0), &Rect::new(5.0, 5.0, 10.0, 10.0)), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(meet(&Rect::new(0.0, 0.0, 10.0, 10.0), &Rect::new(20.0, 0.0, 10.0, 10.0)), None);
    }

    #[test]
    fn what_moves_is_found_without_the_hidden() {
        let slide = serde_json::json!([{"at_ms": 0, "dx": 0}, {"at_ms": 500, "dx": 30}]);
        let mut veil = node("veil", Some("p"), NodeKind::Frame, [0.0, 0.0, 50.0, 50.0]);
        veil.hidden = Some(true);
        let scene = Scene::new(vec![
            node("p", None, NodeKind::Page, [0.0; 4]),
            node("still", Some("p"), NodeKind::Rect, [0.0, 0.0, 10.0, 10.0]),
            veil,
            moving(node("ghost", Some("veil"), NodeKind::Rect, [0.0, 0.0, 10.0, 10.0]), slide.clone()),
        ]);
        assert!(!moves(&scene, "p", &[]), "a hidden frame's mover does not show");
        let mut shown = scene.clone();
        shown.node_mut("veil").unwrap().hidden = None;
        assert!(moves(&shown, "p", &[]) && moves(&shown, "p", &ids(&["veil"])));
        assert!(!moves(&shown, "p", &ids(&["still"])));
    }

    /// An animated export's frames are 8-bit RGBA PNGs of one size, so they make an APNG whose
    /// chunks frame as the format says: acTL, then an fcTL and image data per frame, sequence
    /// numbers in order, every CRC right. Its first frame is an ordinary PNG GDK decodes.
    #[test]
    fn an_animation_is_an_apng() {
        let frame = |rgba: [u8; 4]| {
            let pixels: Vec<u8> = (0..16 * 8).flat_map(|_| rgba).collect();
            png_of(16, 8, &glib::Bytes::from_owned(pixels), 16 * 4)
        };
        let frames = vec![frame([255, 0, 0, 255]), frame([0, 255, 0, 128]), frame([0, 0, 255, 0])];
        let png = apng::assemble(&frames, 30, 0).unwrap();
        let mut at = 8;
        let mut kinds = Vec::new();
        let mut sequence = Vec::new();
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind = std::str::from_utf8(&png[at + 4..at + 8]).unwrap().to_string();
            let crc = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
            assert_eq!(crc, crc32(&png[at + 4..at + 8 + len]), "{kind}'s CRC");
            let data = &png[at + 8..at + 8 + len];
            match kind.as_str() {
                "IHDR" => assert_eq!((&data[..8], data[8], data[9]), (&[0, 0, 0, 16, 0, 0, 0, 8][..], 8, 6), "16×8, 8-bit RGBA"),
                "acTL" => assert_eq!(data, [0, 0, 0, 3, 0, 0, 0, 0], "3 frames, played forever"),
                "fcTL" => {
                    sequence.push(u32::from_be_bytes(data[..4].try_into().unwrap()));
                    assert_eq!(&data[20..24], [0, 1, 0, 30], "1/30 s each");
                }
                "fdAT" => sequence.push(u32::from_be_bytes(data[..4].try_into().unwrap())),
                _ => {}
            }
            kinds.push(kind);
            at += 12 + len;
        }
        assert_eq!(png[..8], [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n']);
        let order: Vec<&str> = kinds.iter().map(String::as_str).filter(|k| ["IHDR", "acTL", "fcTL", "IDAT", "fdAT", "IEND"].contains(k)).collect();
        assert_eq!(order.first(), Some(&"IHDR"));
        assert_eq!(order.last(), Some(&"IEND"));
        assert_eq!(kinds.iter().filter(|k| *k == "fcTL").count(), 3);
        assert!(order.iter().position(|k| *k == "acTL") < order.iter().position(|k| *k == "IDAT"));
        assert_eq!(sequence, (0..sequence.len() as u32).collect::<Vec<_>>(), "fcTL and fdAT counted together, without gaps");
        // A reader that knows no animation shows the first frame.
        let first = gdk::Texture::from_bytes(&glib::Bytes::from_owned(png)).unwrap();
        assert_eq!((first.width(), first.height()), (16, 8));
        let (pixels, _) = rgba_of(&first);
        assert_eq!(&pixels[..4], [255, 0, 0, 255]);
    }

    /// CRC-32 as PNG reckons it, to check chunks against.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for b in bytes {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    #[test]
    fn ask_pictures_are_sized_for_a_model() {
        // I-8: an icon is drawn up to four times its size, a wide page brought down.
        assert_eq!(ask_scale(&Rect::new(0.0, 0.0, 24.0, 24.0)), 4.0);
        assert!((ask_scale(&Rect::new(0.0, 0.0, 784.0, 400.0)) - 2.0).abs() < 1e-9);
        let page = Rect::new(0.0, 0.0, 12_544.0, 3000.0);
        assert!((ask_scale(&page) - 0.25).abs() < 1e-9);
        let (w, h) = boxes::picture_size(&page, ask_scale(&page));
        assert!(f64::from(w) * f64::from(h) <= boxes::MAX_AREA_PX);
    }
}
