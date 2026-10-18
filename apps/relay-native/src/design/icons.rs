//! The icon picker: Relay's icon set in a searchable grid. Choosing one places an icon node in
//! the middle of the view (inside the selected frame, as paste does), or swaps the selected
//! icon's.

use super::board::Board;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp, ICONS};
use serde_json::{json, Map};
use std::rc::Rc;

/// An icon of the set as a symbolic picture that takes the widget's text colour.
pub fn image(name: &str, size: i32) -> gtk::Image {
    let d = relay_client::design::icons::path(name).unwrap_or("");
    let doc = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="url(#gpa:foreground)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="{d}"/></svg>"##
    );
    let image = gtk::Image::from_paintable(Some(&gtk::Svg::from_bytes(&glib::Bytes::from_owned(doc.into_bytes()))));
    image.set_pixel_size(size);
    image
}

pub fn picker(b: &Rc<Board>, pop: &gtk::Popover, target: Option<String>) -> gtk::Widget {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.add_css_class("design-icon-picker");
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search icons"));
    body.append(&search);
    let grid = gtk::FlowBox::new();
    grid.set_max_children_per_line(8);
    grid.set_min_children_per_line(8);
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&grid));
    scroll.set_min_content_height(260);
    scroll.set_max_content_height(320);
    scroll.set_propagate_natural_height(true);
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    body.append(&scroll);
    let nothing = crate::app::label("No icon matches", "dim");
    nothing.set_halign(gtk::Align::Center);
    nothing.set_margin_top(12);
    nothing.set_visible(false);
    body.append(&nothing);
    let mut keys = Vec::new();
    for name in ICONS {
        let key = gtk::Button::new();
        key.add_css_class("design-icon-cell");
        key.set_child(Some(&image(name, 20)));
        key.set_tooltip_text(Some(name));
        let weak = Rc::downgrade(b);
        let p = pop.downgrade();
        let target = target.clone();
        key.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let Some(b) = weak.upgrade() {
                choose(&b, name, target.as_deref());
            }
        });
        grid.insert(&key, -1);
        keys.push((name.to_string(), key));
    }
    let keys = Rc::new(keys);
    let k = keys.clone();
    search.connect_search_changed(move |s| {
        let q = s.text().to_lowercase();
        let mut any = false;
        for (name, key) in k.iter() {
            if let Some(cell) = key.parent() {
                cell.set_visible(name.contains(&q));
                any |= name.contains(&q);
            }
        }
        nothing.set_visible(!any);
    });
    // Enter takes the first icon showing.
    search.connect_activate(move |_| {
        if let Some((_, key)) = keys.iter().find(|(_, key)| key.parent().is_some_and(|c| c.is_visible())) {
            key.emit_clicked();
        }
    });
    // Typing searches as soon as the picker opens.
    let s = search.downgrade();
    glib::idle_add_local_once(move || {
        if let Some(s) = s.upgrade() {
            s.grab_focus();
        }
    });
    body.set_size_request(320, -1);
    body.upcast()
}

pub(super) fn choose(b: &Rc<Board>, name: &str, target: Option<&str>) {
    if let Some(id) = target {
        let mut set = Map::new();
        set.insert("icon".into(), json!(name));
        b.commit(vec![NodeOp::Update { id: id.to_string(), set }]);
        return;
    }
    let parent = b.paste_target();
    let origin = b.origin_of(&parent);
    let (w, h) = b.size.get();
    let (cx, cy) = b.to_page(w / 2.0, h / 2.0);
    let mut n = Node::new(NodeKind::Icon);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Icon);
    n.parent = Some(parent);
    n.icon = Some(name.to_string());
    n.name = Some(name.to_string());
    (n.w, n.h) = (24.0, 24.0);
    n.x = (cx - origin.x - 12.0).round();
    n.y = (cy - origin.y - 12.0).round();
    let id = n.id.clone();
    if b.commit(vec![NodeOp::Add { node: n, index: None }]) {
        b.select(vec![id]);
    }
}
