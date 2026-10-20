//! Pictures of a design: a selection or a page as PNG (rendered by the window's GPU renderer),
//! or as SVG or PDF (the same render nodes drawn on a cairo vector surface), all drawn by the
//! painter the canvas uses. Ask sends the agent a PNG of what you mean, and `design.render`
//! draws the agent's pictures, through the same [`picture_of`].

use super::board::{display_name, Board};
use super::paint::{blend_mode, grect, outline, rgba, Painter};
use gtk::prelude::*;
use gtk::{gdk, gio, graphene, gsk};
use gtk4 as gtk;
use relay_bus::design::{parse_color, NodeKind};
use relay_client::design::color::{fill_of, ground_of, CANVAS};
use relay_client::design::layout::Placed;
use relay_client::design::{boxes, edit, names, shapes, Rect, Scene};
use std::rc::Rc;
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
    fn scale(self, area: &Rect) -> f64 {
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
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Failure::Empty => "Nothing to export: what is chosen shows nothing",
            Failure::NoRenderer => "The canvas isn't ready to draw yet",
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
/// `backdrop`, else on transparency (TM-26). `None` when nothing in it shows.
pub(crate) fn picture_of(painter: &Painter, scene: &Scene, placed: &Placed, page: &str, ids: &[String], backdrop: bool, scale: f64) -> Option<(gsk::RenderNode, Rect)> {
    let area = area_of(painter, scene, placed, page, ids)?;
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
    Some((snap.to_node()?, area))
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

/// The scale Ask pictures `area` at, chosen for a model: its long side near 1,568 px, from a
/// quarter to four times its size, within 8,192 px a side and 4 megapixels (I-8, TM-26).
pub fn ask_scale(area: &Rect) -> f64 {
    let want = (1568.0 / area.w.max(area.h).max(1.0)).clamp(0.25, 4.0);
    boxes::fit_scale(area, want, MAX_PIXELS, boxes::MAX_AREA_PX)
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
