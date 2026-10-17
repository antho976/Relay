//! Version history: the design as it was. You save a named version when it matters; Relay
//! saves one on its own before an agent starts editing what someone else edited last, and
//! before a restore. A version shows its picture before you bring it back, and bringing one
//! back is itself undoable (the state it replaced is saved first). Also "Copy as HTML and
//! CSS", the engine's `design.code` for the selected frame.

use super::board::Board;
use super::paint::{fill_of, rgba, CANVAS};
use crate::app::{label, rows, text};
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_bus::design::Node;
use relay_client::design::{layout, Camera, Rect, Scene};
use serde_json::json;
use std::rc::Rc;

impl Board {
    pub(super) fn show_history(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let b = self.me();
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.add_css_class("design-history");
        pop.set_parent(&self.canvas);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(self.size.get().0 as i32 / 2, 8, 1, 1)));
        pop.set_position(gtk::PositionType::Bottom);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_size_request(380, -1);
        body.append(&label("Version history", "design-ask-title"));
        let save = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let name = gtk::Entry::new();
        name.set_placeholder_text(Some("Name this version (Enter saves it)"));
        name.set_hexpand(true);
        save.append(&name);
        body.append(&save);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_min_content_height(120);
        scroll.set_max_content_height(460);
        scroll.set_propagate_natural_height(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        body.append(&scroll);
        pop.set_child(Some(&body));
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        let load = {
            let weak = Rc::downgrade(&b);
            let list = list.downgrade();
            let ui = ui.clone();
            Rc::new(move || {
                let (Some(b), Some(list)) = (weak.upgrade(), list.upgrade()) else { return };
                let ui = ui.clone();
                let design = b.doc.id;
                glib::spawn_future_local(async move {
                    let listed = ui.call("design.version.list", json!({"design_id": design})).await;
                    while let Some(c) = list.first_child() {
                        list.remove(&c);
                    }
                    let versions = match listed {
                        Ok(v) => rows(&v, "versions"),
                        Err(e) => {
                            list.append(&label(&format!("Could not list versions: {e}"), "dim"));
                            return;
                        }
                    };
                    if versions.is_empty() {
                        let none = label("No versions yet. Name one above to keep this moment; Relay keeps one by itself before an agent edits and before a restore.", "dim");
                        none.set_wrap(true);
                        none.set_max_width_chars(44);
                        list.append(&none);
                    }
                    for v in versions {
                        list.append(&version_row(&b, &v));
                    }
                });
            })
        };
        let weak = Rc::downgrade(&b);
        let reload = load.clone();
        name.connect_activate(move |e| {
            let (Some(b), Some(ui)) = (weak.upgrade(), weak.upgrade().and_then(|b| b.ui.upgrade())) else { return };
            let label_text = e.text().trim().to_string();
            e.set_text("");
            let design = b.doc.id;
            let reload = reload.clone();
            glib::spawn_future_local(async move {
                let payload = if label_text.is_empty() { json!({"design_id": design}) } else { json!({"design_id": design, "label": label_text}) };
                match ui.call("design.version.save", payload).await {
                    Ok(_) => reload(),
                    Err(err) => b.toast(&format!("Could not save a version: {err}")),
                }
            });
        });
        load();
        pop.popup();
        name.grab_focus();
    }

    /// The selected frame as HTML and CSS, on the clipboard.
    pub(super) fn copy_code(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let target = {
            let scene = self.doc.scene.borrow();
            self.selected().into_iter().next().or_else(|| scene.children(&self.page.borrow()).first().cloned())
        };
        let Some(node) = target else {
            self.toast("Select a frame to copy as code");
            return;
        };
        let design = self.doc.id;
        let weak = self.me_weak();
        glib::spawn_future_local(async move {
            let got = ui.call("design.code", json!({"design_id": design, "node_id": node})).await;
            let Some(b) = weak.upgrade() else { return };
            match got {
                Ok(v) => {
                    let page = format!("<style>\n{}\n</style>\n{}", text(&v, "css"), text(&v, "html"));
                    b.root.clipboard().set_text(&page);
                    b.toast("Copied the HTML and CSS");
                }
                Err(e) => b.toast(&format!("Could not make code: {e}")),
            }
        });
    }
}

fn version_row(b: &Rc<Board>, v: &serde_json::Value) -> gtk::Widget {
    let id = v["id"].as_i64().unwrap_or_default();
    let item = gtk::Box::new(gtk::Orientation::Vertical, 4);
    item.add_css_class("design-version");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let auto = v["auto"].as_bool().unwrap_or(false);
    let title = text(v, "label");
    let when = crate::relative::ago(text(v, "created_at"), crate::relative::Form::Long).unwrap_or_default();
    let words = gtk::Box::new(gtk::Orientation::Vertical, 0);
    words.set_hexpand(true);
    let name = label(if title.is_empty() { &when } else { title }, if auto { "dim" } else { "design-comment-author" });
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let by = text(v, "created_by");
    let meta = label(&format!("{}{} · {} layers", if title.is_empty() { String::new() } else { format!("{when} · ") }, if by == "user" || by.is_empty() { "you".to_string() } else { by.to_string() }, v["nodes"].as_i64().unwrap_or(0)), "faint");
    meta.set_xalign(0.0);
    words.append(&meta);
    head.append(&words);
    let peek = gtk::Button::with_label("Preview");
    peek.add_css_class("quiet");
    head.append(&peek);
    let restore = gtk::Button::with_label("Restore");
    restore.add_css_class("quiet");
    head.append(&restore);
    let remove = crate::app::icon_button("trash", "Delete this version");
    head.append(&remove);
    item.append(&head);
    let picture = gtk::Picture::new();
    picture.set_size_request(340, 180);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_visible(false);
    item.append(&picture);
    let weak = Rc::downgrade(b);
    let pic = picture.downgrade();
    peek.connect_clicked(move |_| {
        let (Some(b), Some(pic)) = (weak.upgrade(), pic.upgrade()) else { return };
        if pic.is_visible() {
            pic.set_visible(false);
            return;
        }
        let Some(ui) = b.ui.upgrade() else { return };
        let design = b.doc.id;
        glib::spawn_future_local(async move {
            let Ok(got) = ui.call("design.version.get", json!({"design_id": design, "version_id": id})).await else { return };
            let nodes: Vec<Node> = serde_json::from_value(got["nodes"].clone()).unwrap_or_default();
            if let Some(t) = thumbnail(&b, nodes, 680.0, 360.0) {
                pic.set_paintable(Some(&t));
                pic.set_visible(true);
            }
        });
    });
    let weak = Rc::downgrade(b);
    restore.connect_clicked(move |k| {
        let Some(b) = weak.upgrade() else { return };
        let Some(ui) = b.ui.upgrade() else { return };
        let design = b.doc.id;
        let pop = k.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>();
        glib::spawn_future_local(async move {
            match ui.call("design.version.restore", json!({"design_id": design, "version_id": id})).await {
                Ok(_) => {
                    b.doc.reread();
                    b.toast("Restored. The state it replaced is saved as a version, so you can go back.");
                    if let Some(p) = pop {
                        p.popdown();
                    }
                }
                Err(e) => b.toast(&format!("Could not restore: {e}")),
            }
        });
    });
    let weak = Rc::downgrade(b);
    let row = item.downgrade();
    remove.connect_clicked(move |_| {
        let Some(b) = weak.upgrade() else { return };
        let Some(ui) = b.ui.upgrade() else { return };
        let design = b.doc.id;
        let row = row.clone();
        glib::spawn_future_local(async move {
            if ui.call("design.version.delete", json!({"design_id": design, "version_id": id})).await.is_ok() {
                if let Some(r) = row.upgrade() {
                    r.set_visible(false);
                }
            }
        });
    });
    item.upcast()
}

/// A version's first page, drawn by the board's painter.
fn thumbnail(b: &Board, nodes: Vec<Node>, w: f64, h: f64) -> Option<gdk::Texture> {
    let scene = Scene::new(nodes);
    let page = scene.pages().first()?.clone();
    let placed = layout::place(&scene, &page, &*b.painter);
    let bounds = Rect::bounds(scene.children(&page).iter().filter_map(|id| placed.rects.get(id)))?;
    let snap = gtk::Snapshot::new();
    let ground = scene.node(&page).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
    snap.append_color(&rgba(&ground), &graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
    let cam = Camera::fit(&bounds, w, h, 16.0, 4.0);
    snap.scale(cam.zoom as f32, cam.zoom as f32);
    snap.translate(&graphene::Point::new(-cam.x as f32, -cam.y as f32));
    b.painter.paint_page(&snap, &scene, &placed, &page, None);
    let node = snap.to_node()?;
    let renderer = b.canvas.native()?.renderer()?;
    Some(renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w as f32, h as f32))))
}
