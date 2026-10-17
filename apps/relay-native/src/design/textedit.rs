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
use relay_bus::design::{parse_color, Node, NodeKind, NodeOp, Sizing, Span, TextAlign};
use relay_client::design::layout::STICKY_PAD;
use relay_client::design::Rect;
use serde_json::{json, Map};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Edit {
    pub id: String,
    pub view: gtk::TextView,
    tag: gtk::TextTag,
    original: Node,
    /// Added for this edit: it is sent only if words were typed.
    pub created: bool,
    pub zoom: Cell<f64>,
    /// A tag per run style in use (the span with no words), over the node's own tag.
    styles: RefCell<Vec<(gtk::TextTag, Span)>>,
    /// Words just typed: where, how many characters, in the style of what was before them.
    typed: RefCell<Option<(i32, i32, Span)>>,
}

impl Board {
    /// A new text at `at` (page coordinates) in `parent`, typed into at once.
    pub(super) fn create_text(self: &Rc<Self>, at: (f64, f64), parent: String) {
        let origin = self.placed().rect(&parent).unwrap_or_default();
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
        tag.set_priority(0);
        let edit = Edit {
            id: n.id.clone(),
            view: view.clone(),
            tag,
            original: n.clone(),
            created,
            zoom: Cell::new(0.0),
            styles: RefCell::default(),
            typed: RefCell::default(),
        };
        *self.edit.borrow_mut() = Some(edit);
        // The words' runs, each in its style.
        if let Some(spans) = n.live_spans() {
            let mut at = 0;
            for span in spans {
                let len = span.text.chars().count() as i32;
                let style = Span { text: String::new(), ..span.clone() };
                if style != Span::default() {
                    if let Some(tag) = self.style_tag(&style) {
                        buffer.apply_tag(&tag, &buffer.iter_at_offset(at), &buffer.iter_at_offset(at + len));
                    }
                }
                at += len;
            }
        }
        let weak = Rc::downgrade(self);
        buffer.connect_insert_text(move |buffer, iter, text| {
            if let Some(b) = weak.upgrade() {
                let at = iter.offset();
                let style = if at > 0 { b.style_at(&buffer.iter_at_offset(at - 1)) } else { b.style_at(iter) };
                if let Some(e) = b.edit.borrow().as_ref() {
                    *e.typed.borrow_mut() = Some((at, text.chars().count() as i32, style));
                }
            }
        });
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
            let (id, typed) = match b.edit.borrow().as_ref() {
                Some(e) => {
                    buffer.apply_tag(&e.tag, &buffer.start_iter(), &buffer.end_iter());
                    (e.id.clone(), e.typed.borrow_mut().take())
                }
                None => return,
            };
            // Typing inside a styled run carries its style on.
            if let Some((at, len, style)) = typed.filter(|t| t.2 != Span::default()) {
                if let Some(tag) = b.style_tag(&style) {
                    buffer.apply_tag(&tag, &buffer.iter_at_offset(at), &buffer.iter_at_offset(at + len));
                }
            }
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
            let shift = mods.contains(gdk::ModifierType::SHIFT_MASK);
            if key == gdk::Key::Escape || (ctrl && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter)) {
                b.end_edit(true);
                return glib::Propagation::Stop;
            }
            if ctrl {
                let field = match key.to_lower() {
                    gdk::Key::b => "weight",
                    gdk::Key::i => "italic",
                    gdk::Key::u => "underline",
                    gdk::Key::x if shift => "strike",
                    gdk::Key::e => "code",
                    gdk::Key::k => {
                        b.ask_link();
                        return glib::Propagation::Stop;
                    }
                    _ => return glib::Propagation::Proceed,
                };
                b.toggle_run(field, None);
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
        edit.tag.set_line_height(n.line_height.unwrap_or(1.3) as f32);
        // GtkTextTag takes no negative tracking (a tight title's -0.6): while typing it reads a
        // touch looser than on the canvas.
        edit.tag.set_letter_spacing((n.letter_spacing.unwrap_or(0.0).max(0.0) * zoom * pango::SCALE as f64) as i32);
        edit.tag.set_foreground_rgba(Some(&rgba(&text_color(&scene, n))));
        edit.tag.set_underline(if n.underline == Some(true) { pango::Underline::Single } else { pango::Underline::None });
        edit.tag.set_strikethrough(n.strike == Some(true));
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
        let spans = runs(&buffer, &edit.styles.borrow());
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
        let spans_before = edit.original.live_spans().map(<[Span]>::to_vec);
        if commit {
            if edit.created {
                if !text.trim().is_empty() {
                    let mut node = current.unwrap_or_else(|| edit.original.clone());
                    node.text = Some(text);
                    node.spans = spans;
                    self.commit(vec![NodeOp::Add { node, index: None }]);
                    self.select(vec![edit.id.clone()]);
                } else {
                    self.select(Vec::new());
                }
            } else if text != before || spans != spans_before {
                let op = if text.trim().is_empty() && edit.original.kind == NodeKind::Text {
                    NodeOp::Delete { id: edit.id.clone() }
                } else {
                    let mut set = Map::new();
                    // A shape whose label was cleared has none.
                    let empty = text.trim().is_empty();
                    set.insert("text".into(), if empty { serde_json::Value::Null } else { json!(text) });
                    set.insert("spans".into(), spans.as_ref().filter(|_| !empty).map_or(serde_json::Value::Null, |s| json!(s)));
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

impl Board {
    /// The tag for a run style, made once per edit and scaled with the zoom.
    fn style_tag(&self, style: &Span) -> Option<gtk::TextTag> {
        let edit = self.edit.borrow();
        let e = edit.as_ref()?;
        if let Some((t, _)) = e.styles.borrow().iter().find(|(_, s)| s == style) {
            return Some(t.clone());
        }
        let tag = e.view.buffer().create_tag(None, &[])?;
        let zoom = self.cam.get().zoom;
        if let Some(w) = style.weight {
            tag.set_weight(w as i32);
        }
        if style.italic == Some(true) {
            tag.set_style(pango::Style::Italic);
        }
        if style.underline == Some(true) || style.link.is_some() {
            tag.set_underline(pango::Underline::Single);
        }
        if style.strike == Some(true) {
            tag.set_strikethrough(true);
        }
        if let Some(c) = style.color.as_deref().or(style.link.as_ref().map(|_| "#4C8DFF")).filter(|c| parse_color(c).is_some()) {
            tag.set_foreground_rgba(Some(&rgba(c)));
        }
        if let Some(size) = style.size {
            tag.set_size_points(size * zoom * 0.75);
        }
        if style.code == Some(true) {
            tag.set_family(Some("Geist Mono"));
            tag.set_background_rgba(Some(&gdk::RGBA::new(0.5, 0.5, 0.5, 0.19)));
        } else if let Some(f) = &style.font {
            tag.set_family(Some(f));
        }
        e.styles.borrow_mut().push((tag.clone(), style.clone()));
        Some(tag)
    }

    /// The run style at `iter`: the style tag on it, or none.
    fn style_at(&self, iter: &gtk::TextIter) -> Span {
        let edit = self.edit.borrow();
        let Some(e) = edit.as_ref() else { return Span::default() };
        let styles = e.styles.borrow();
        iter.tags().iter().find_map(|t| styles.iter().find(|(s, _)| s == t).map(|(_, st)| st.clone())).unwrap_or_default()
    }

    /// Turn `field` on or off across the selected words (a link to `link`, or off).
    pub(super) fn toggle_run(&self, field: &str, link: Option<Option<String>>) {
        let Some((view, buffer)) = self.edit.borrow().as_ref().map(|e| (e.view.clone(), e.view.buffer())) else { return };
        let Some((a, z)) = buffer.selection_bounds() else { return };
        let (start, end) = (a.offset(), z.offset());
        // On when any selected word lacks it.
        let on = (start..end).any(|i| {
            let s = self.style_at(&buffer.iter_at_offset(i));
            match field {
                "weight" => s.weight.is_none_or(|w| w < 600),
                "italic" => s.italic != Some(true),
                "underline" => s.underline != Some(true),
                "strike" => s.strike != Some(true),
                "code" => s.code != Some(true),
                _ => true,
            }
        });
        let styles: Vec<gtk::TextTag> = self.edit.borrow().as_ref().map(|e| e.styles.borrow().iter().map(|(t, _)| t.clone()).collect()).unwrap_or_default();
        let mut i = start;
        while i < end {
            let style = self.style_at(&buffer.iter_at_offset(i));
            let mut j = i + 1;
            while j < end && self.style_at(&buffer.iter_at_offset(j)) == style {
                j += 1;
            }
            let mut next = style.clone();
            match field {
                "weight" => next.weight = on.then_some(700),
                "italic" => next.italic = on.then_some(true),
                "underline" => next.underline = on.then_some(true),
                "strike" => next.strike = on.then_some(true),
                "code" => next.code = on.then_some(true),
                "link" => next.link = link.clone().flatten(),
                _ => {}
            }
            let (ri, rj) = (buffer.iter_at_offset(i), buffer.iter_at_offset(j));
            for t in &styles {
                buffer.remove_tag(t, &ri, &rj);
            }
            if next != Span::default() {
                if let Some(tag) = self.style_tag(&next) {
                    buffer.apply_tag(&tag, &buffer.iter_at_offset(i), &buffer.iter_at_offset(j));
                }
            }
            i = j;
        }
        view.grab_focus();
    }

    /// Ctrl+K while typing: a web address for the selected words.
    fn ask_link(&self) {
        let Some(view) = self.edit.borrow().as_ref().map(|e| e.view.clone()) else { return };
        let buffer = view.buffer();
        let Some((a, _)) = buffer.selection_bounds() else {
            self.toast("Select the words to link first");
            return;
        };
        let current = self.style_at(&a).link;
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(&view);
        let location = view.iter_location(&a);
        let (x, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, location.x(), location.y());
        pop.set_pointing_to(Some(&gdk::Rectangle::new(x, y, 1, location.height())));
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("https://… (empty removes the link)"));
        entry.set_text(current.as_deref().unwrap_or(""));
        entry.set_width_chars(36);
        pop.set_child(Some(&entry));
        let weak = self.me_weak();
        let p = pop.downgrade();
        entry.connect_activate(move |e| {
            let url = e.text().trim().to_string();
            if let Some(b) = weak.upgrade() {
                b.toggle_run("link", Some((!url.is_empty()).then_some(url)));
            }
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
        });
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
        entry.grab_focus();
    }
}

/// The buffer's words as styled runs, or `None` when none has a style of its own.
fn runs(buffer: &gtk::TextBuffer, styles: &[(gtk::TextTag, Span)]) -> Option<Vec<Span>> {
    let mut out: Vec<Span> = Vec::new();
    let mut iter = buffer.start_iter();
    while !iter.is_end() {
        let style = iter.tags().iter().find_map(|t| styles.iter().find(|(s, _)| s == t).map(|(_, st)| st.clone())).unwrap_or_default();
        let ch = iter.char();
        match out.last_mut() {
            Some(last) if Span { text: String::new(), ..last.clone() } == style => last.text.push(ch),
            _ => out.push(Span { text: ch.to_string(), ..style }),
        }
        iter.forward_char();
    }
    out.iter().any(|s| Span { text: String::new(), ..s.clone() } != Span::default()).then_some(out)
}

/// The kinds whose words can be typed on the canvas.
pub fn takes_text(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Sticky) || is_shape(kind)
}

/// Shapes that carry a centred label.
pub fn is_shape(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon)
}
