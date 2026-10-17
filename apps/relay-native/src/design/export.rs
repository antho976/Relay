//! Pictures of a design: a selection or a page as PNG, at 1× or 2×, drawn by the same painter
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
    let node = snap.to_node()?;
    let renderer = b.canvas.native()?.renderer()?;
    let viewport = graphene::Rect::new(0.0, 0.0, (bounds.w * scale).ceil() as f32, (bounds.h * scale).ceil() as f32);
    Some((renderer.render_texture(&node, Some(&viewport)), bounds))
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
