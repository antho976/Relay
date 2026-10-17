//! Pictures on the canvas: chosen from disk, dropped from the file manager or pasted. Each
//! goes to the engine's picture store (`design.asset.add`, which copies it, so a design keeps
//! its pictures when the file moves) and becomes an image node at its natural size.

use super::board::Board;
use gtk::prelude::*;
use gtk::{gdk, gio};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp};
use relay_bus::ops::design::AssetAddOut;
use serde_json::{json, Map};
use std::path::PathBuf;
use std::rc::Rc;

/// Pictures land no wider than this; a 4K screenshot would otherwise fill the canvas.
const MAX_SIDE: f64 = 960.0;

fn image_filter() -> gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Pictures (PNG, JPEG, GIF, WebP, SVG)"));
    for mime in ["image/png", "image/jpeg", "image/gif", "image/webp", "image/svg+xml"] {
        filter.add_mime_type(mime);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

pub fn choose_images(b: &Rc<Board>) {
    let dialog = gtk::FileDialog::builder().title("Place pictures").filters(&image_filter()).build();
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let weak = Rc::downgrade(b);
    glib::spawn_future_local(async move {
        let Ok(files) = dialog.open_multiple_future(parent.as_ref()).await else {
            return;
        };
        let paths: Vec<PathBuf> = (0..files.n_items())
            .filter_map(|i| files.item(i).and_downcast::<gio::File>())
            .filter_map(|f| f.path())
            .collect();
        if let Some(b) = weak.upgrade() {
            place_files(&b, paths, None);
        }
    });
}

/// Store each file and place it, side by side from `at` (page coordinates) or the centre of
/// the view.
pub fn place_files(b: &Rc<Board>, paths: Vec<PathBuf>, at: Option<(f64, f64)>) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let weak = Rc::downgrade(b);
    let design = b.doc.id;
    glib::spawn_future_local(async move {
        let mut placed = Vec::new();
        for path in paths {
            match ui.call("design.asset.add", json!({"design_id": design, "path": path.to_string_lossy()})).await {
                Ok(v) => {
                    if let Ok(out) = serde_json::from_value::<AssetAddOut>(v) {
                        placed.push((out, path.file_stem().map(|s| s.to_string_lossy().to_string())));
                    }
                }
                Err(e) => {
                    if let Some(b) = weak.upgrade() {
                        b.toast(&format!("{}: {e}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                    }
                }
            }
        }
        if let Some(b) = weak.upgrade() {
            add_images(&b, placed, at);
        }
    });
}

/// A picture from the clipboard: written to a private file, stored, placed.
pub fn place_texture(b: &Rc<Board>, texture: gdk::Texture, at: Option<(f64, f64)>) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let weak = Rc::downgrade(b);
    let design = b.doc.id;
    let rt = ui.rt.clone();
    glib::spawn_future_local(async move {
        let dir = glib::user_runtime_dir();
        let path = dir.join(format!("relay-design-paste-{}.png", uuid::Uuid::new_v4()));
        let p = path.clone();
        let written = rt
            .spawn_blocking(move || std::fs::write(&p, texture.save_to_png_bytes()))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
        if let Err(e) = written {
            if let Some(b) = weak.upgrade() {
                b.toast(&format!("Could not paste the picture: {e}"));
            }
            return;
        }
        let out = ui.call("design.asset.add", json!({"design_id": design, "path": path.to_string_lossy()})).await;
        let _ = std::fs::remove_file(&path);
        let Some(b) = weak.upgrade() else {
            return;
        };
        match out.map_err(|e| e.to_string()).and_then(|v| serde_json::from_value::<AssetAddOut>(v).map_err(|e| e.to_string())) {
            Ok(out) => add_images(&b, vec![(out, Some("Pasted picture".into()))], at),
            Err(e) => b.toast(&format!("Could not paste the picture: {e}")),
        }
    });
}

fn add_images(b: &Rc<Board>, pictures: Vec<(AssetAddOut, Option<String>)>, at: Option<(f64, f64)>) {
    if pictures.is_empty() {
        return;
    }
    let target = b.paste_target();
    let origin = b.placed().rect(&target).unwrap_or_default();
    let (w, h) = b.size.get();
    let (mut x, y) = at.unwrap_or_else(|| b.to_page(w / 2.0, h / 2.0));
    let mut ops = Vec::new();
    let mut ids = Vec::new();
    for (out, name) in pictures {
        let scale = (MAX_SIDE / out.w.max(out.h).max(1) as f64).min(1.0);
        let (iw, ih) = ((out.w as f64 * scale).round().max(1.0), (out.h as f64 * scale).round().max(1.0));
        let mut n = Node::new(NodeKind::Image);
        n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Image);
        n.parent = Some(target.clone());
        n.name = name;
        n.src = Some(out.src);
        n.w = iw;
        n.h = ih;
        let (cx, cy) = if at.is_some() { (x, y) } else { (x - iw / 2.0, y - ih / 2.0) };
        n.x = (cx - origin.x).round();
        n.y = (cy - origin.y).round();
        x += iw + 24.0;
        ids.push(n.id.clone());
        ops.push(NodeOp::Add { node: n, index: None });
    }
    if b.commit(ops) {
        b.select(ids);
    }
}

/// Choose another picture for an image node, keeping its box.
pub fn replace_image(b: &Rc<Board>, id: &str) {
    let dialog = gtk::FileDialog::builder().title("Replace picture").filters(&image_filter()).build();
    let parent = b.root.root().and_downcast::<gtk::Window>();
    let weak = Rc::downgrade(b);
    let id = id.to_string();
    glib::spawn_future_local(async move {
        let Ok(file) = dialog.open_future(parent.as_ref()).await else {
            return;
        };
        let (Some(path), Some(b)) = (file.path(), weak.upgrade()) else {
            return;
        };
        let Some(ui) = b.ui.upgrade() else {
            return;
        };
        match ui.call("design.asset.add", json!({"design_id": b.doc.id, "path": path.to_string_lossy()})).await {
            Ok(v) => {
                if let Ok(out) = serde_json::from_value::<AssetAddOut>(v) {
                    let mut set = Map::new();
                    set.insert("src".into(), json!(out.src));
                    b.commit(vec![NodeOp::Update { id, set }]);
                }
            }
            Err(e) => b.toast(&e.to_string()),
        }
    });
}
