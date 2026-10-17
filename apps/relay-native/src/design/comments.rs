//! Comments: notes pinned to the design, by you or by an agent, that are never part of it.
//! The comment tool (C) pins one where you click; a pin opens its thread, where you reply,
//! resolve it, or hand it to the terminal's agent, which can answer in the thread itself.
//! Pins keep their screen size at every zoom and sit inside the frame they were dropped on,
//! so they move with it.

use super::board::{Board, AGENT, SELECT};
use super::paint::rgba;
use crate::app::label;
use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp, Reply};
use relay_client::design::layout::Placed;
use relay_client::design::Scene;
use serde_json::{json, Map, Value};
use std::rc::Rc;

/// A pin's size on screen.
const PIN: f64 = 26.0;

fn who(author: &str) -> String {
    if author == "user" || author.is_empty() {
        "You".into()
    } else {
        author.to_string()
    }
}

fn now() -> String {
    glib::DateTime::now_utc().ok().and_then(|d| d.format_iso8601().ok()).map(|s| s.to_string()).unwrap_or_default()
}

impl Board {
    /// This page's comments, oldest first, with where each points (page coordinates).
    fn page_comments(&self, scene: &Scene, placed: &Placed, page: &str) -> Vec<(String, (f64, f64), bool, String)> {
        let mut out: Vec<_> = scene
            .nodes()
            .filter(|n| n.kind == NodeKind::Comment && scene.page_of(&n.id) == Some(page))
            .filter(|n| self.extra.show_resolved.get() || n.resolved != Some(true))
            .filter_map(|n| {
                let r = placed.rect(&n.id)?;
                Some((n.id.clone(), (r.x, r.y), n.resolved == Some(true), n.author.clone().unwrap_or_default()))
            })
            .collect();
        out.sort_by_key(|c| natural(&c.0));
        out
    }

    /// Pins over the canvas: a speech mark whose corner touches the place it is about.
    pub(super) fn comment_pins(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed, page: &str) {
        if self.extra.hide_comments.get() {
            return;
        }
        let cam = self.cam.get();
        let open = self.extra.comment_open.borrow().clone();
        for (i, (id, at, resolved, author)) in self.page_comments(scene, placed, page).into_iter().enumerate() {
            let (x, y) = cam.to_screen(at.0, at.1);
            let (x, y) = (x as f32, y as f32);
            let s = PIN as f32;
            let pb = gsk::PathBuilder::new();
            // A rounded square with its bottom-left corner sharp, sitting above-right of the point.
            pb.move_to(x, y);
            pb.line_to(x, y - s / 2.0);
            pb.cubic_to(x, y - s * 0.78, x + s * 0.22, y - s, x + s / 2.0, y - s);
            pb.cubic_to(x + s * 0.78, y - s, x + s, y - s * 0.78, x + s, y - s / 2.0);
            pb.cubic_to(x + s, y - s * 0.22, x + s * 0.78, y, x + s / 2.0, y);
            pb.close();
            let path = pb.to_path();
            let color = if resolved {
                gdk::RGBA::new(0.55, 0.55, 0.55, 1.0)
            } else if author == "user" || author.is_empty() {
                rgba(SELECT)
            } else {
                rgba(AGENT)
            };
            snap.append_outset_shadow(&gsk::RoundedRect::from_rect(graphene::Rect::new(x, y - s, s, s), s / 2.0), &gdk::RGBA::new(0.0, 0.0, 0.0, 0.3), 0.0, 2.0, 0.0, 6.0);
            snap.append_fill(&path, gsk::FillRule::Winding, &color);
            let ring = if open.as_deref() == Some(id.as_str()) { 2.5 } else { 1.5 };
            snap.append_stroke(&path, &gsk::Stroke::new(ring), &gdk::RGBA::WHITE);
            let mark = if resolved { "✓".to_string() } else { (i + 1).to_string() };
            let layout = self.canvas.create_pango_layout(Some(&mark));
            let mut d = pango::FontDescription::new();
            d.set_family("Geist");
            d.set_weight(pango::Weight::Semibold);
            d.set_absolute_size(11.0 * pango::SCALE as f64);
            layout.set_font_description(Some(&d));
            let (lw, lh) = layout.pixel_size();
            snap.save();
            snap.translate(&graphene::Point::new(x + (s - lw as f32) / 2.0, y - s + (s - lh as f32) / 2.0));
            snap.append_layout(&layout, &gdk::RGBA::WHITE);
            snap.restore();
        }
    }

    /// The pin under screen point `(x, y)`.
    pub(super) fn comment_at(&self, x: f64, y: f64) -> Option<String> {
        if self.extra.hide_comments.get() {
            return None;
        }
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let page = self.page.borrow().clone();
        let cam = self.cam.get();
        self.page_comments(&scene, &placed, &page).into_iter().rev().find_map(|(id, at, _, _)| {
            let (px, py) = cam.to_screen(at.0, at.1);
            (x >= px - 2.0 && x <= px + PIN + 2.0 && y >= py - PIN - 2.0 && y <= py + 2.0).then_some(id)
        })
    }

    /// The comment tool's click: a new thread there, written before it is added.
    pub(super) fn new_comment(&self, p: (f64, f64), parent: String) {
        let origin = if parent == *self.page.borrow() { (0.0, 0.0) } else { self.placed().rect(&parent).map_or((0.0, 0.0), |r| (r.x, r.y)) };
        let (sx, sy) = self.cam.get().to_screen(p.0, p.1);
        let b = self.me();
        thread(&b, None, (sx, sy), move |b, text| {
            let mut n = Node::new(NodeKind::Comment);
            n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Comment);
            n.parent = Some(parent.clone());
            n.x = (p.0 - origin.0).round();
            n.y = (p.1 - origin.1).round();
            n.text = Some(text);
            n.author = Some("user".into());
            let id = n.id.clone();
            b.commit(vec![NodeOp::Add { node: n, index: None }]);
            Some(id)
        });
    }

    /// Open a pin's thread.
    pub(super) fn open_comment(&self, id: &str) {
        let placed = self.placed();
        let Some(r) = placed.rect(id) else { return };
        let (sx, sy) = self.cam.get().to_screen(r.x, r.y);
        let b = self.me();
        let target = id.to_string();
        thread(&b, Some(id.to_string()), (sx, sy), move |b, text| {
            let replies = {
                let scene = b.doc.scene.borrow();
                let mut replies = scene.node(&target)?.replies.clone().unwrap_or_default();
                replies.push(Reply { author: "user".into(), text, at: now() });
                replies
            };
            let mut set = Map::new();
            set.insert("replies".into(), json!(replies));
            b.commit(vec![NodeOp::Update { id: target.clone(), set }]);
            Some(target.clone())
        });
    }
}

/// Number-aware order for comment ids (comment-2 before comment-10).
fn natural(id: &str) -> (String, u64) {
    let digits: String = id.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
    (id[..id.len() - digits.len()].to_string(), digits.parse().unwrap_or(0))
}

/// The thread popover at screen point `at`: the comment and its replies, then a field to
/// write in. `send` adds what was written (a new comment, or a reply) and names the comment.
fn thread(b: &Rc<Board>, id: Option<String>, at: (f64, f64), send: impl Fn(&Rc<Board>, String) -> Option<String> + 'static) {
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.add_css_class("design-comment-pop");
    pop.set_parent(&b.canvas);
    pop.set_position(gtk::PositionType::Right);
    pop.set_pointing_to(Some(&gdk::Rectangle::new((at.0 + PIN) as i32, (at.1 - PIN / 2.0) as i32, 1, 1)));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_size_request(320, -1);
    let node = id.as_deref().and_then(|i| b.doc.scene.borrow().node(i).cloned());
    if let Some(n) = &node {
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let title = label("Comment", "design-ask-title");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        head.append(&title);
        let resolved = n.resolved == Some(true);
        let resolve = gtk::ToggleButton::new();
        resolve.add_css_class("design-context-key");
        resolve.set_child(Some(&super::glyphs::image("resolve", 14)));
        resolve.set_tooltip_text(Some(if resolved { "Reopen" } else { "Resolve" }));
        resolve.set_active(resolved);
        let weak = Rc::downgrade(b);
        let cid = n.id.clone();
        let p = pop.downgrade();
        resolve.connect_toggled(move |k| {
            if let Some(b) = weak.upgrade() {
                let mut set = Map::new();
                set.insert("resolved".into(), if k.is_active() { json!(true) } else { Value::Null });
                b.commit(vec![NodeOp::Update { id: cid.clone(), set }]);
                if k.is_active() {
                    if let Some(p) = p.upgrade() {
                        p.popdown();
                    }
                }
            }
        });
        head.append(&resolve);
        let remove = gtk::Button::new();
        remove.add_css_class("design-context-key");
        remove.set_child(Some(&crate::icons::image("trash", 14)));
        remove.set_tooltip_text(Some("Delete the thread"));
        let weak = Rc::downgrade(b);
        let cid = n.id.clone();
        let p = pop.downgrade();
        remove.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.commit(vec![NodeOp::Delete { id: cid.clone() }]);
            }
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
        });
        head.append(&remove);
        body.append(&head);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let entry = |author: &str, when: &str, text: &str| {
            let item = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            top.append(&label(&who(author), "design-comment-author"));
            if let Some(w) = crate::relative::ago(when, crate::relative::Form::Long) {
                top.append(&label(&w, "faint"));
            }
            item.append(&top);
            let words = label(text, "design-comment-text");
            words.set_wrap(true);
            words.set_xalign(0.0);
            words.set_selectable(true);
            words.set_max_width_chars(40);
            item.append(&words);
            item
        };
        list.append(&entry(n.author.as_deref().unwrap_or("user"), "", n.text.as_deref().unwrap_or("")));
        for r in n.replies.iter().flatten() {
            list.append(&entry(&r.author, &r.at, &r.text));
        }
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_max_content_height(320);
        scroll.set_propagate_natural_height(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        body.append(&scroll);
    } else {
        body.append(&label("New comment", "design-ask-title"));
    }
    let view = gtk::TextView::new();
    view.add_css_class("design-ask-text");
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_size_request(-1, 56);
    view.set_accepts_tab(false);
    let frame = gtk::ScrolledWindow::new();
    frame.set_child(Some(&view));
    frame.set_max_content_height(160);
    frame.set_propagate_natural_height(true);
    frame.add_css_class("design-ask-field");
    body.append(&frame);
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let ask = gtk::Button::new();
    ask.add_css_class("quiet");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&super::glyphs::image("spark", 14));
    row.append(&label("Send to agent", ""));
    ask.set_child(Some(&row));
    ask.set_tooltip_text(Some("Post it and ask the terminal's agent to act on the thread and reply in it"));
    ask.set_hexpand(true);
    ask.set_halign(gtk::Align::Start);
    foot.append(&ask);
    let post = gtk::Button::new();
    post.add_css_class("primary");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&label(if node.is_some() { "Reply" } else { "Post" }, ""));
    row.append(&label("Ctrl Enter", "design-keycap-dark"));
    post.set_child(Some(&row));
    foot.append(&post);
    body.append(&foot);
    pop.set_child(Some(&body));
    *b.extra.comment_open.borrow_mut() = id.clone();
    let weak = Rc::downgrade(b);
    pop.connect_closed(move |p| {
        if let Some(b) = weak.upgrade() {
            b.extra.comment_open.borrow_mut().take();
            b.canvas.queue_draw();
            b.canvas.grab_focus();
        }
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    let send = Rc::new(send);
    let go = {
        let weak = Rc::downgrade(b);
        let view = view.downgrade();
        let p = pop.downgrade();
        Rc::new(move |agent: bool| {
            let (Some(b), Some(view)) = (weak.upgrade(), view.upgrade()) else {
                return;
            };
            let buffer = view.buffer();
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
            let target = if text.is_empty() { id.clone() } else { send(&b, text) };
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let (true, Some(target)) = (agent, target) {
                send_to_agent(&b, &target);
            }
        })
    };
    let g = go.clone();
    post.connect_clicked(move |_| g(false));
    let g = go.clone();
    ask.connect_clicked(move |_| g(true));
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, mods| {
        if mods.contains(gdk::ModifierType::CONTROL_MASK) && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
            go(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    view.add_controller(keys);
    b.canvas.queue_draw();
    pop.popup();
    view.grab_focus();
}

/// Hand a thread to the design's terminal: the agent reads it, changes what it is about, and
/// answers in it.
fn send_to_agent(b: &Rc<Board>, comment: &str) {
    let Some(session) = b.doc.session() else {
        b.toast("Tie this design to a terminal first (the key beside the title), then send again");
        return;
    };
    let (text, about) = {
        let scene = b.doc.scene.borrow();
        let Some(n) = scene.node(comment) else { return };
        let mut words = n.text.clone().unwrap_or_default();
        for r in n.replies.iter().flatten() {
            words.push_str(&format!(" | {}: {}", who(&r.author), r.text));
        }
        // What the pin sits on: the deepest node under it.
        let placed = b.placed();
        let about = placed.rect(comment).and_then(|r| {
            let page = b.page.borrow().clone();
            relay_client::design::hit::chain(&scene, &placed, &page, r.x, r.y, 2.0).last().cloned()
        });
        (words, about)
    };
    if let Some(a) = &about {
        b.select(vec![a.clone()]);
    }
    let request = format!(
        "Comment thread {comment} says: \"{}\". Do what it asks, then reply in the thread: design.apply update {comment} with replies (append {{\"text\": \"what you did\"}}, keeping the earlier ones) — or set resolved: true when it is done.",
        text.replace('"', "'")
    );
    super::ask::send_request(b, session, request, true);
}

#[cfg(test)]
mod tests {
    #[test]
    fn comment_ids_sort_by_their_number() {
        let mut ids = vec!["comment-10", "comment-2", "comment-1"];
        ids.sort_by_key(|i| super::natural(i));
        assert_eq!(ids, ["comment-1", "comment-2", "comment-10"]);
        assert_eq!(super::who("user"), "You");
        assert_eq!(super::who("keen-raven"), "keen-raven");
    }
}
