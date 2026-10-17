//! Typing into a text or sticky on the canvas. A native text view sits exactly over the node,
//! in the node's own font, size (times the zoom), line height and colour, so typing looks like
//! the canvas; GTK gives it the caret, selection, input methods and clipboard. The scene
//! previews each keystroke (a hugging text grows as it is typed) and one edit is sent when the
//! typing ends.

use super::board::Board;
use super::paint::{font_description, rgba, text_color};
use gtk::prelude::*;
use gtk::{gdk, pango};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp, Sizing, TextAlign};
use relay_client::design::layout::STICKY_PAD;
use relay_client::design::Rect;
use serde_json::{json, Map};
use std::cell::Cell;
use std::rc::Rc;

pub struct Edit {
    pub id: String,
    pub view: gtk::TextView,
    tag: gtk::TextTag,
    original: Node,
    /// Added for this edit: it is sent only if words were typed.
    pub created: bool,
    pub zoom: Cell<f64>,
}

impl Board {
    /// A new text at `at` (page coordinates) in `parent`, typed into at once.
    pub(super) fn create_text(self: &Rc<Self>, at: (f64, f64), parent: String) {
        let origin = self.origin_of(&parent);
        let mut n = Node::new(NodeKind::Text);
        n.id = self.doc.scene.borrow_mut().fresh_id(NodeKind::Text);
        n.parent = Some(parent);
        n.text = Some(String::new());
        n.x = (at.0 - origin.x).round();
        n.y = (at.1 - origin.y - 10.0).round();
        n.w_sizing = Some(Sizing::Hug);
        let id = n.id.clone();
        self.doc.scene.borrow_mut().insert_preview(n.clone());
        self.invalidate();
        self.select(vec![id.clone()]);
        self.start_edit(n, true);
        if !self.tool_lock.get() {
            self.set_tool(super::board::Tool::Select);
        }
    }

    pub(super) fn begin_edit(self: &Rc<Self>, id: &str, created: bool) {
        let Some(n) = self.doc.scene.borrow().node(id).cloned() else {
            return;
        };
        if n.is_locked() || !takes_text(n.kind) {
            return;
        }
        self.start_edit(n, created);
    }

    fn start_edit(self: &Rc<Self>, n: Node, created: bool) {
        self.end_edit(true);
        self.doc.set_hold(true);
        let view = gtk::TextView::new();
        view.add_css_class("design-text-edit");
        view.set_accepts_tab(false);
        view.set_left_margin(0);
        view.set_right_margin(0);
        view.set_top_margin(0);
        view.set_bottom_margin(0);
        view.set_pixels_above_lines(0);
        view.set_pixels_below_lines(0);
        view.set_pixels_inside_wrap(0);
        let buffer = view.buffer();
        buffer.set_text(n.text.as_deref().unwrap_or(""));
        let tag = buffer.create_tag(None, &[]).expect("an anonymous tag");
        let edit = Edit {
            id: n.id.clone(),
            view: view.clone(),
            tag,
            original: n.clone(),
            created,
            zoom: Cell::new(0.0),
        };
        *self.edit.borrow_mut() = Some(edit);
        *self.painter.editing.borrow_mut() = Some(n.id.clone());
        self.style_edit();
        self.overlay.add_overlay(&view);
        self.invalidate();

        let weak = Rc::downgrade(self);
        buffer.connect_changed(move |buffer| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
            let id = match b.edit.borrow().as_ref() {
                Some(e) => {
                    buffer.apply_tag(&e.tag, &buffer.start_iter(), &buffer.end_iter());
                    e.id.clone()
                }
                None => return,
            };
            if let Some(node) = b.doc.scene.borrow_mut().node_mut(&id) {
                node.text = Some(text);
            }
            b.placed.borrow_mut().take();
            b.content.borrow_mut().take();
            b.canvas.queue_draw();
            b.overlay.queue_allocate();
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, mods| {
            let Some(b) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
            if key == gdk::Key::Escape || (ctrl && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter)) {
                b.end_edit(true);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        view.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_leave(move |_| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            // Leaving for a popover of the board's own (a colour, a size) keeps the edit.
            let w = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(b) = w.upgrade() {
                    let still = b.edit.borrow().as_ref().is_some_and(|e| e.view.has_focus());
                    let in_board = b
                        .root
                        .root()
                        .and_then(|r| gtk::prelude::GtkWindowExt::focus(r.downcast_ref::<gtk::Window>()?))
                        .is_some_and(|f| f.is_ancestor(&b.root));
                    if !still && !in_board {
                        b.end_edit(true);
                    }
                }
            });
            let _ = b;
        });
        view.add_controller(focus);
        buffer.apply_tag(&self.edit.borrow().as_ref().expect("edit").tag, &buffer.start_iter(), &buffer.end_iter());
        view.grab_focus();
        if created {
            buffer.place_cursor(&buffer.end_iter());
        } else {
            buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        }
        self.rebuild_context();
    }

    /// The edited node's text box, in page coordinates.
    pub(super) fn edit_rect(&self) -> Option<Rect> {
        let id = self.edit.borrow().as_ref()?.id.clone();
        let placed = self.placed();
        let r = placed.rect(&id)?;
        let scene = self.doc.scene.borrow();
        let n = scene.node(&id)?;
        if is_shape(n.kind) {
            return super::paint::shape_text_box(&self.painter, n, &r);
        }
        Some(if n.kind == NodeKind::Sticky {
            Rect::new(r.x + STICKY_PAD, r.y + STICKY_PAD, (r.w - 2.0 * STICKY_PAD).max(1.0), (r.h - 2.0 * STICKY_PAD).max(1.0))
        } else {
            Rect::new(r.x, r.y, r.w + 4.0 / self.cam.get().zoom, r.h)
        })
    }

    /// Give the text view the node's look at the current zoom.
    pub(super) fn style_edit(&self) {
        let edit = self.edit.borrow();
        let Some(edit) = edit.as_ref() else {
            return;
        };
        let zoom = self.cam.get().zoom;
        if edit.zoom.get() == zoom {
            return;
        }
        edit.zoom.set(zoom);
        let scene = self.doc.scene.borrow();
        let Some(n) = scene.node(&edit.id) else {
            return;
        };
        let mut scaled = n.clone();
        scaled.font_size = Some(super::paint::font_size(n) * zoom);
        let desc = font_description(&scaled);
        edit.tag.set_font_desc(Some(&desc));
        // The tag takes a multiple of the font's own line; the model's is of the font size.
        let line = n.line_height.unwrap_or(1.3) * super::paint::font_size(n);
        edit.tag.set_line_height((line / self.painter.natural_line(n)) as f32);
        // GtkTextTag takes no negative tracking (a tight title's -0.6): while typing it reads a
        // touch looser than on the canvas.
        edit.tag.set_letter_spacing((n.letter_spacing.unwrap_or(0.0).max(0.0) * zoom * pango::SCALE as f64) as i32);
        edit.tag.set_foreground_rgba(Some(&rgba(&text_color(&scene, n))));
        edit.tag.set_underline(if n.underline == Some(true) { pango::Underline::Single } else { pango::Underline::None });
        edit.tag.set_strikethrough(n.strike == Some(true));
        edit.tag.set_text_transform(match n.text_case {
            Some(relay_bus::design::TextCase::Upper) => pango::TextTransform::Uppercase,
            Some(relay_bus::design::TextCase::Lower) => pango::TextTransform::Lowercase,
            Some(relay_bus::design::TextCase::Title) => pango::TextTransform::Capitalize,
            None => pango::TextTransform::None,
        });
        let default = if is_shape(n.kind) { TextAlign::Center } else { TextAlign::Left };
        edit.view.set_justification(match n.align.unwrap_or(default) {
            TextAlign::Center => gtk::Justification::Center,
            TextAlign::Right => gtk::Justification::Right,
            TextAlign::Left => gtk::Justification::Left,
        });
        let wraps = n.kind == NodeKind::Sticky || is_shape(n.kind) || relay_client::design::layout::w_sizing(n) != Sizing::Hug;
        edit.view.set_wrap_mode(if wraps { gtk::WrapMode::WordChar } else { gtk::WrapMode::None });
    }

    /// Stop typing: send the text if it changed (`commit`), or drop the edit.
    pub(super) fn end_edit(&self, commit: bool) {
        let Some(edit) = self.edit.borrow_mut().take() else {
            return;
        };
        *self.painter.editing.borrow_mut() = None;
        let buffer = edit.view.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
        self.overlay.remove_overlay(&edit.view);
        // Back to the words it had; the edit below makes the change for real. A style set
        // while typing has been sent already (or, on a new text, rides along with it).
        let current = self.doc.scene.borrow().node(&edit.id).cloned();
        {
            let mut scene = self.doc.scene.borrow_mut();
            if edit.created {
                scene.remove_preview(&edit.id);
            } else if let Some(n) = scene.node_mut(&edit.id) {
                n.text = edit.original.text.clone();
            }
        }
        self.doc.set_hold(false);
        let before = edit.original.text.clone().unwrap_or_default();
        if commit {
            if edit.created {
                if !text.trim().is_empty() {
                    let mut node = current.unwrap_or_else(|| edit.original.clone());
                    node.text = Some(text);
                    self.commit(vec![NodeOp::Add { node, index: None }]);
                    self.select(vec![edit.id.clone()]);
                } else {
                    self.select(Vec::new());
                }
            } else if text != before {
                let op = if text.trim().is_empty() && edit.original.kind == NodeKind::Text {
                    NodeOp::Delete { id: edit.id.clone() }
                } else {
                    let mut set = Map::new();
                    // A shape whose label was cleared has none.
                    let empty = text.trim().is_empty();
                    set.insert("text".into(), if empty { serde_json::Value::Null } else { json!(text) });
                    NodeOp::Update { id: edit.id.clone(), set }
                };
                self.commit(vec![op]);
            }
        }
        self.invalidate();
        self.canvas.grab_focus();
        self.rebuild_context();
    }
}

/// The kinds whose words can be typed on the canvas.
pub fn takes_text(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Sticky) || is_shape(kind)
}

/// Shapes that carry a centred label.
pub fn is_shape(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon)
}
