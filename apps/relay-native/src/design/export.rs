//! Pictures of a design: a selection or a page as PNG at 1× or 2×, or as SVG or PDF, drawn by the same painter
//! the canvas uses and rendered by the window's GPU renderer. Ask uses the same picture to show
//! the agent what you mean.

use super::board::{display_name, Board};
use super::paint::{fill_of, grect, rgba, CANVAS};
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_client::design::{edit, Rect};
use std::rc::Rc;

/// Most pixels along a side of an exported picture.
const MAX_PIXELS: f64 = 8192.0;

/// Render `ids` (or the whole page, `page`) at `scale`. The page carries its canvas colour;
/// a selection is drawn on transparency.
pub fn render(b: &Board, ids: &[String], scale: f64, page: bool) -> Option<(gdk::Texture, Rect)> {
    let (node, bounds, scale) = picture(b, ids, scale, page)?;
    let renderer = b.canvas.native()?.renderer()?;
    let viewport = graphene::Rect::new(0.0, 0.0, (bounds.w * scale).ceil() as f32, (bounds.h * scale).ceil() as f32);
    Some((renderer.render_texture(&node, Some(&viewport)), bounds))
}

/// The render node of `ids` (or the page) at `scale`, its page bounds and the scale used.
fn picture(b: &Board, ids: &[String], scale: f64, page: bool) -> Option<(gtk::gsk::RenderNode, Rect, f64)> {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let page_id = b.page.borrow().clone();
    let ids: Vec<String> = if page { scene.children(&page_id).to_vec() } else { edit::outermost(&scene, ids) };
    let bounds = Rect::bounds(ids.iter().filter_map(|id| placed.rects.get(id)))?;
    let bounds = if page { bounds.inflate(32.0) } else { bounds };
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Vector {
    Svg,
    Pdf,
}

/// The selection (or, with none, the page) as an SVG or a PDF: the canvas's own drawing,
/// replayed through cairo, so shapes and text stay vectors.
pub fn export_vector(b: &Board, kind: Vector) {
    let ids = b.selected();
    let page = ids.is_empty();
    let bytes = match vector_bytes(b, &ids, kind) {
        Ok(bytes) => bytes,
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
    let ext = if kind == Vector::Svg { "svg" } else { "pdf" };
    write_file(&b.me(), bytes, &format!("{}.{ext}", name.replace(['/', '\\'], "-")), &format!("Export as {}", ext.to_uppercase()));
}

/// `ids` (or the page, with none) drawn into an SVG or PDF document.
pub fn vector_bytes(b: &Board, ids: &[String], kind: Vector) -> Result<Vec<u8>, String> {
    let page = ids.is_empty();
    let Some((node, bounds, _)) = picture(b, ids, 1.0, page) else {
        return Err("Nothing to export".into());
    };
    let (w, h) = (bounds.w.ceil().max(1.0), bounds.h.ceil().max(1.0));
    let drawn: Result<Vec<u8>, String> = (|| {
        let out: Box<dyn std::any::Any> = match kind {
            Vector::Svg => {
                let mut surface = gtk::cairo::SvgSurface::for_stream(w, h, Vec::<u8>::new()).map_err(|e| e.to_string())?;
                surface.set_document_unit(gtk::cairo::SvgUnit::Px);
                let cr = gtk::cairo::Context::new(&surface).map_err(|e| e.to_string())?;
                node.draw(&cr);
                drop(cr);
                surface.finish_output_stream().map_err(|e| e.error.to_string())?
            }
            Vector::Pdf => {
                let surface = gtk::cairo::PdfSurface::for_stream(w, h, Vec::<u8>::new()).map_err(|e| e.to_string())?;
                let cr = gtk::cairo::Context::new(&surface).map_err(|e| e.to_string())?;
                node.draw(&cr);
                drop(cr);
                surface.finish_output_stream().map_err(|e| e.error.to_string())?
            }
        };
        out.downcast::<Vec<u8>>().map(|v| *v).map_err(|_| "the drawing came back in an unknown shape".to_string())
    })();
    drawn.map_err(|e| format!("Could not export: {e}"))
}

/// Ask where to save `bytes`, then save them.
pub fn write_file(b: &Rc<Board>, bytes: Vec<u8>, file_name: &str, title: &str) {
    let dialog = gtk::FileDialog::builder().title(title).initial_name(file_name).build();
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let weak = Rc::downgrade(b);
    glib::spawn_future_local(async move {
        let Ok(file) = dialog.save_future(parent.as_ref()).await else {
            return;
        };
        let saved = file
            .replace_contents_future(bytes, None, false, gtk::gio::FileCreateFlags::REPLACE_DESTINATION)
            .await
            .map(|_| ())
            .map_err(|(_, e)| e.to_string());
        if let Some(b) = weak.upgrade() {
            match saved {
                Ok(()) => b.toast(&format!("Exported {}", file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
                Err(e) => b.toast(&format!("Could not export: {e}")),
            }
        }
    });
}

pub fn export_selection(b: &Board, page: bool) {
    let ids = if page { Vec::new() } else { b.selected() };
    if !page && ids.is_empty() {
        b.toast("Select what to export first");
        return;
    }
    save(&b.me(), ids, 2.0, page);
}

pub fn export_ids(b: &Rc<Board>, ids: Vec<String>, scale: f64) {
    save(b, ids, scale, false);
}

fn save(b: &Rc<Board>, ids: Vec<String>, scale: f64, page: bool) {
    let Some((texture, _)) = render(b, &ids, scale, page) else {
        b.toast("Nothing to export");
        return;
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
    let file_name = format!("{}{}.png", name.replace(['/', '\\'], "-"), if scale > 1.0 { "@2x" } else { "" });
    let dialog = gtk::FileDialog::builder().title("Export as PNG").initial_name(file_name.as_str()).build();
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let Some(rt) = b.ui.upgrade().map(|ui| ui.rt.clone()) else {
        return;
    };
    let weak = Rc::downgrade(b);
    glib::spawn_future_local(async move {
        let Ok(file) = dialog.save_future(parent.as_ref()).await else {
            return;
        };
        let saved = match rt.spawn_blocking(move || texture.save_to_png_bytes()).await {
            Ok(bytes) => file
                .replace_contents_future(bytes, None, false, gtk::gio::FileCreateFlags::REPLACE_DESTINATION)
                .await
                .map(|_| ())
                .map_err(|(_, e)| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        if let Some(b) = weak.upgrade() {
            match saved {
                Ok(()) => b.toast(&format!("Exported {}", file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
                Err(e) => b.toast(&format!("Could not export: {e}")),
            }
        }
    });
}
