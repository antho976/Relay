//! Pictures for agents: an agent cannot see the canvas, so `design.render` asks this window for
//! a PNG of a frame or page (`design.render_requested`). The board showing that design draws it
//! with its own painter; with none open, the design is read and drawn here off-screen. The PNG
//! goes in Relay's cache and `design.render.done` hands its path back to the agent.

use super::board::Board;
use super::doc::Doc;
use super::paint::{fill_of, grect, rgba, Painter, CANVAS};
use crate::app::Ui;
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_bus::design::NodeKind;
use relay_client::design::{layout, Rect, Scene};
use serde_json::{json, Value};
use std::rc::Rc;

/// Most pixels along a side.
const MAX_PIXELS: f64 = 4096.0;

/// Draw `node` (a page, or anything on one) of `scene` at `scale`.
fn draw(scene: &Scene, painter: &Painter, widget: &impl IsA<gtk::Widget>, node: &str, scale: f64) -> Result<gdk::Texture, String> {
    let n = scene.node(node).ok_or_else(|| format!("no node {node:?} in the design"))?;
    let page = if n.kind == NodeKind::Page { node.to_string() } else { scene.page_of(node).unwrap_or_default().to_string() };
    let placed = layout::place(scene, &page, painter);
    let (ids, bounds, ground) = if n.kind == NodeKind::Page {
        let ids = scene.children(&page).to_vec();
        let bounds = Rect::bounds(ids.iter().filter_map(|id| placed.rects.get(id))).ok_or("the page is empty")?.inflate(32.0);
        (ids, bounds, Some(scene.node(&page).and_then(fill_of).unwrap_or_else(|| CANVAS.into())))
    } else {
        let r = placed.rect(node).ok_or("the node has no place on its page")?;
        (vec![node.to_string()], r, None)
    };
    let scale = scale.min(MAX_PIXELS / bounds.w.max(bounds.h).max(1.0));
    let snap = gtk::Snapshot::new();
    snap.scale(scale as f32, scale as f32);
    snap.translate(&graphene::Point::new(-bounds.x as f32, -bounds.y as f32));
    if let Some(g) = ground {
        snap.append_color(&rgba(&g), &grect(&bounds));
    }
    for id in &ids {
        painter.paint(&snap, scene, &placed, id, None);
    }
    let rendered = snap.to_node().ok_or("nothing to draw")?;
    let renderer = widget.as_ref().native().and_then(|n| n.renderer()).ok_or("the window has no renderer yet")?;
    let viewport = graphene::Rect::new(0.0, 0.0, (bounds.w * scale).ceil() as f32, (bounds.h * scale).ceil() as f32);
    Ok(renderer.render_texture(&rendered, Some(&viewport)))
}

/// Answer one `design.render_requested`.
pub fn requested(ui: &Rc<Ui>, payload: &Value) {
    let request = payload["request_id"].as_str().unwrap_or_default().to_string();
    let design = payload["design_id"].as_i64().unwrap_or(0);
    let node = payload["node_id"].as_str().map(str::to_string);
    let scale = payload["scale"].as_f64().unwrap_or(2.0).clamp(0.25, 4.0);
    if request.is_empty() || design == 0 {
        return;
    }
    let board: Option<Rc<Board>> = super::open_boards().into_iter().find(|b| b.doc.id == design);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let result: Result<gdk::Texture, String> = async {
            match &board {
                Some(b) => {
                    let scene = b.doc.scene.borrow();
                    let node = node.clone().or_else(|| scene.pages().first().cloned()).ok_or("the design has no page")?;
                    draw(&scene, &b.painter, &b.canvas, &node, scale)
                }
                None => {
                    let doc = Doc::open(&ui, design).await?;
                    let painter = Painter::new(&ui.window);
                    *painter.assets_dir.borrow_mut() = doc.assets_dir.borrow().clone();
                    let node = node.clone().or_else(|| doc.scene.borrow().pages().first().cloned()).ok_or("the design has no page")?;
                    // A first pass starts the pictures loading; the second draws them.
                    let _ = draw(&doc.scene.borrow(), &painter, &ui.window, &node, scale);
                    glib::timeout_future(std::time::Duration::from_millis(700)).await;
                    let scene = doc.scene.borrow();
                    draw(&scene, &painter, &ui.window, &node, scale)
                }
            }
        }
        .await;
        let answer = match result {
            Ok(texture) => {
                let dir = glib::user_cache_dir().join("relay").join("design-renders");
                let path = dir.join(format!("{request}.png"));
                let p = path.clone();
                let bytes = texture.save_to_png_bytes();
                let written = ui.rt.spawn_blocking(move || std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&p, &bytes))).await;
                match written {
                    Ok(Ok(())) => json!({"request_id": request, "path": path.to_string_lossy()}),
                    Ok(Err(e)) => json!({"request_id": request, "error": e.to_string()}),
                    Err(e) => json!({"request_id": request, "error": e.to_string()}),
                }
            }
            Err(e) => json!({"request_id": request, "error": e}),
        };
        let _ = ui.call("design.render.done", answer).await;
    });
}
