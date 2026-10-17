//! Pictures of a design: a selection or a page as PNG (at 1× or 2×, rendered by the window's
//! GPU renderer), or as SVG or PDF (the same render nodes drawn on a cairo vector surface), all
//! drawn by the painter the canvas uses. Ask uses the PNG to show the agent what you mean.

use super::board::{display_name, Board};
use super::paint::{fill_of, grect, rgba, CANVAS};
use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};
use gtk4 as gtk;
use relay_bus::design::NodeKind;
use relay_client::design::layout::Placed;
use relay_client::design::{edit, shapes, Rect, Scene};
use std::rc::Rc;

/// Most pixels along a side of an exported picture.
const MAX_PIXELS: f64 = 8192.0;

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
}

/// Everything `id` paints, in page coordinates: its box turned as it is turned, a connector's
/// route, and the reach of its shadow, stroke and blur.
fn painted_bounds(scene: &Scene, placed: &Placed, id: &str) -> Option<Rect> {
    let n = scene.node(id)?;
    let r = placed.rect(id)?;
    let base = if n.kind == NodeKind::Line && (n.from.is_some() || n.to.is_some() || n.route.is_some()) {
        let (pts, _) = shapes::line_points(scene, placed, n);
        let x0 = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let y0 = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let x1 = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let y1 = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        if pts.is_empty() { r } else { Rect::new(x0, y0, x1 - x0, y1 - y0) }
    } else {
        shapes::rotated_bounds(&r, shapes::rotation(n))
    };
    let shadow = n.shadow.as_ref().filter(|s| !s.inset).map_or(0.0, |s| s.blur + s.spread.max(0.0) + s.x.abs().max(s.y.abs()));
    let stroke = match n.kind {
        // Strokes and arrowheads reach past a line's points.
        NodeKind::Line | NodeKind::Path => (n.stroke_width.unwrap_or(2.0) * 3.5).max(8.0) / 2.0,
        _ => 0.0,
    };
    let mut out = base.inflate(shadow.max(stroke) + n.blur.unwrap_or(0.0) * 2.0);
    // What overflows an unclipped frame or a group shows, so it is pictured too.
    if n.kind.holds_children() && !(n.kind == NodeKind::Frame && n.clip != Some(false)) {
        for child in scene.children(id) {
            if let Some(c) = painted_bounds(scene, placed, child) {
                out = Rect::bounds([out, c].iter()).unwrap_or(out);
            }
        }
    }
    Some(out)
}

/// The render node of `ids` (or the whole page, `page`) at scale 1, from its painted bounds'
/// corner, and those bounds. The page carries its canvas colour; a selection is drawn on
/// transparency.
fn picture(b: &Board, ids: &[String], page: bool, scale: f64) -> Option<(gsk::RenderNode, Rect, f64)> {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let page_id = b.page.borrow().clone();
    let ids: Vec<String> = if page { scene.children(&page_id).to_vec() } else { edit::outermost(&scene, ids) };
    let shown: Vec<Rect> = ids.iter().filter(|id| scene.node(id).is_some_and(|n| !n.is_hidden())).filter_map(|id| painted_bounds(&scene, &placed, id)).collect();
    let bounds = Rect::bounds(shown.iter())?;
    let bounds = if page { bounds.inflate(32.0) } else { bounds };
    let bounds = Rect::new(bounds.x.floor(), bounds.y.floor(), bounds.w.ceil().max(1.0), bounds.h.ceil().max(1.0));
    let scale = scale.min(MAX_PIXELS / bounds.w.max(bounds.h).max(1.0));
    let snap = gtk::Snapshot::new();
    snap.scale(scale as f32, scale as f32);
    snap.translate(&graphene::Point::new(-bounds.x as f32, -bounds.y as f32));
    if page {
        let fill = scene.node(&page_id).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        snap.append_color(&rgba(&fill), &grect(&bounds));
    }
    for id in &ids {
        b.painter.paint(&snap, &scene, &placed, id, None);
    }
    Some((snap.to_node()?, bounds, scale))
}

/// Render `ids` (or the whole page, `page`) at `scale` as a texture.
pub fn render(b: &Board, ids: &[String], scale: f64, page: bool) -> Option<(gdk::Texture, Rect)> {
    let (node, bounds, scale) = picture(b, ids, page, scale)?;
    let renderer = b.canvas.native()?.renderer()?;
    let viewport = graphene::Rect::new(0.0, 0.0, (bounds.w * scale).ceil() as f32, (bounds.h * scale).ceil() as f32);
    Some((renderer.render_texture(&node, Some(&viewport)), bounds))
}

/// `ids` (or the page) as an SVG or PDF document: the render node drawn on a vector surface.
pub(super) fn vector(b: &Board, ids: &[String], page: bool, format: Format) -> Result<Vec<u8>, String> {
    let (node, bounds, _) = picture(b, ids, page, 1.0).ok_or("Nothing to export")?;
    let draw = |cr: &cairo::Context| node.draw(cr);
    let bytes = match format {
        Format::Svg => {
            let mut surface = cairo::SvgSurface::for_stream(bounds.w, bounds.h, Vec::<u8>::new()).map_err(|e| e.to_string())?;
            surface.set_document_unit(cairo::SvgUnit::Px);
            draw(&cairo::Context::new(&surface).map_err(|e| e.to_string())?);
            surface.finish_output_stream().map_err(|e| e.to_string())?
        }
        _ => {
            let surface = cairo::PdfSurface::for_stream(bounds.w, bounds.h, Vec::<u8>::new()).map_err(|e| e.to_string())?;
            draw(&cairo::Context::new(&surface).map_err(|e| e.to_string())?);
            surface.finish_output_stream().map_err(|e| e.to_string())?
        }
    };
    bytes.downcast::<Vec<u8>>().map(|b| *b).map_err(|_| "the surface kept its bytes".into())
}

pub fn export_selection(b: &Board, page: bool, format: Format) {
    let ids = if page { Vec::new() } else { b.selected() };
    if !page && ids.is_empty() {
        b.toast("Select what to export first");
        return;
    }
    save(&b.me(), ids, format, page);
}

pub fn export_ids(b: &Rc<Board>, ids: Vec<String>, format: Format) {
    save(b, ids, format, false);
}

fn save(b: &Rc<Board>, ids: Vec<String>, format: Format, page: bool) {
    // PNG is encoded off the main thread; a vector document is drawn here, being quick.
    enum Out {
        Texture(gdk::Texture),
        Bytes(Vec<u8>),
    }
    let out = match format {
        Format::Png(scale) => render(b, &ids, scale, page).map(|(t, _)| Out::Texture(t)).ok_or_else(|| "Nothing to export".to_string()),
        _ => vector(b, &ids, page, format).map(Out::Bytes),
    };
    let out = match out {
        Ok(out) => out,
        Err(e) => {
            b.toast(&e);
            return;
        }
    };
    let name = if page {
        b.page_name(&b.page.borrow())
    } else {
        let scene = b.doc.scene.borrow();
        match ids.as_slice() {
            [one] => scene.node(one).map(display_name).unwrap_or_else(|| "Selection".into()),
            _ => format!("{} selection", b.doc.title()),
        }
    };
    let suffix = if matches!(format, Format::Png(s) if s > 1.0) { "@2x" } else { "" };
    let file_name = format!("{}{}.{}", name.replace(['/', '\\'], "-"), suffix, format.extension());
    let title = format!("Export as {}", format.extension().to_uppercase());
    let dialog = gtk::FileDialog::builder().title(title.as_str()).initial_name(file_name.as_str()).build();
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let Some(rt) = b.ui.upgrade().map(|ui| ui.rt.clone()) else {
        return;
    };
    let weak = Rc::downgrade(b);
    glib::spawn_future_local(async move {
        let Ok(file) = dialog.save_future(parent.as_ref()).await else {
            return;
        };
        let bytes = match out {
            Out::Texture(texture) => rt.spawn_blocking(move || texture.save_to_png_bytes()).await.map_err(|e| e.to_string()),
            Out::Bytes(bytes) => Ok(glib::Bytes::from_owned(bytes)),
        };
        let saved = match bytes {
            Ok(bytes) => file
                .replace_contents_future(bytes, None, false, gtk::gio::FileCreateFlags::REPLACE_DESTINATION)
                .await
                .map(|_| ())
                .map_err(|(_, e)| e.to_string()),
            Err(e) => Err(e),
        };
        if let Some(b) = weak.upgrade() {
            match saved {
                Ok(()) => b.toast(&format!("Exported {}", file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
                Err(e) => b.toast(&format!("Could not export: {e}")),
            }
        }
    });
}
