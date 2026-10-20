//! Typing into a text or sticky on the canvas. A native text view sits exactly over the node,
//! in the node's own font, size (times the zoom), line height, tracking, case and colour, so
//! typing looks like the canvas; GTK gives it the caret, selection, input methods and clipboard.
//! The scene previews each keystroke (a hugging text grows as it is typed, its runs follow the
//! words) and one edit is sent when the typing ends.
//!
//! The view's look is a CSS provider made for each edit (I-13), which can say what a text tag
//! cannot: a size in pixels, a line height in pixels as the canvas lays lines, negative
//! tracking, no tabular figures, the caret's and the selection's colours. Tags lay the parts
//! styled on their own (runs) and Title case's capitals over it.

use super::board::Board;
use super::paint::{font_family, font_size, rgba, text_color};
use gtk::prelude::*;
use gtk::{gdk, pango};
use gtk4 as gtk;
use relay_bus::design::{parse_color, strip_controls, title_starts, Node, NodeKind, NodeOp, Sizing, TextAlign, TextCase, TextRun, MAX_TEXT};
use relay_client::design::layout::STICKY_PAD;
use relay_client::design::names::cut_text;
use relay_client::design::{edit, text, Rect, Scene};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

pub struct Edit {
    pub id: String,
    pub view: gtk::TextView,
    /// The view's look, made for this edit (a class of its own) and dropped with it.
    css: gtk::CssProvider,
    class: String,
    /// The letters Title case capitalises, as the canvas does (TM-34).
    upper: gtk::TextTag,
    /// A tag a run, over its characters.
    run_tags: RefCell<Vec<gtk::TextTag>>,
    /// The node as it was when typing began, or as a style key while typing last sent it.
    original: RefCell<Node>,
    /// The words the runs were last carried over to, and the runs as they are now.
    last: RefCell<(String, Vec<TextRun>)>,
    /// Added for this edit: it is sent only if words were typed.
    pub created: bool,
    pub zoom: Cell<f64>,
    /// What the view's look was made from: it is made again only when that changes (E-text-10).
    look: Cell<u64>,
    /// An input method is composing: Ctrl+Enter and Esc are its own (I-14).
    composing: Rc<Cell<bool>>,
    /// The board's window, and its handler that ends the edit when the window is left (TM-75).
    window: RefCell<Option<(glib::WeakRef<gtk::Window>, glib::SignalHandlerId)>>,
}

impl Edit {
    /// The runs carried over to `words`, the editor's words now, from those they were last
    /// carried to: one keystroke, paste or cut at a time, each a single change, which
    /// `text::remap_runs` follows exactly.
    fn carry(&self, words: &str) -> Vec<TextRun> {
        let mut last = self.last.borrow_mut();
        if last.0 != words {
            let runs = text::remap_runs(&last.0, words, &last.1);
            *last = (words.to_string(), runs);
        }
        last.1.clone()
    }
}

thread_local! {
    /// Edits made so far: each one's CSS class is its own.
    static EDITS: Cell<u64> = const { Cell::new(0) };
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
        let class = format!("design-edit-{}", EDITS.with(|e| e.replace(e.get() + 1)));
        view.add_css_class(&class);
        let css = gtk::CssProvider::new();
        // Above the app's own and its appearance's, so the node's look wins.
        gtk::style_context_add_provider_for_display(&self.canvas.display(), &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
        let buffer = view.buffer();
        buffer.set_text(n.text.as_deref().unwrap_or(""));
        let upper = buffer.create_tag(None, &[]).expect("an anonymous tag");
        upper.set_text_transform(pango::TextTransform::Uppercase);
        let composing = Rc::new(Cell::new(false));
        let edit = Edit {
            id: n.id.clone(),
            view: view.clone(),
            css,
            class,
            upper,
            run_tags: RefCell::default(),
            original: RefCell::new(n.clone()),
            last: RefCell::new((n.text.clone().unwrap_or_default(), n.runs.clone().unwrap_or_default())),
            created,
            zoom: Cell::new(0.0),
            look: Cell::new(0),
            composing: composing.clone(),
            window: RefCell::default(),
        };
        *self.edit.borrow_mut() = Some(edit);
        *self.painter.editing.borrow_mut() = Some(n.id.clone());
        self.style_edit();
        self.overlay.add_overlay(&view);
        // Drawn just above the canvas and under the floating panels (dock, layers, inspector):
        // added last, it painted over the layers panel when the text sat beside it.
        view.insert_after(&self.overlay, Some(&self.canvas));
        self.invalidate();

        // No more than a text holds (B41): what would pass it is cut where it is typed or
        // pasted, without splitting a drawn character, and said.
        let weak = Rc::downgrade(self);
        buffer.connect_insert_text(move |buffer, at, words| {
            let room = MAX_TEXT.saturating_sub(buffer.char_count().max(0) as usize);
            if words.chars().count() <= room {
                return;
            }
            buffer.stop_signal_emission_by_name("insert-text");
            let (kept, _) = cut_text(words, room);
            if !kept.is_empty() {
                buffer.insert(at, &kept);
            }
            if let Some(b) = weak.upgrade() {
                b.toast(&format!("A text holds at most {MAX_TEXT} characters"));
            }
        });
        let weak = Rc::downgrade(self);
        buffer.connect_changed(move |buffer| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            let words = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
            // The runs follow the words they style (drawing 6.7).
            let Some((id, runs)) = b.edit.borrow().as_ref().map(|e| (e.id.clone(), e.carry(&words))) else {
                return;
            };
            if let Some(node) = b.doc.scene.borrow_mut().node_mut(&id) {
                node.text = Some(words);
                node.runs = (!runs.is_empty()).then_some(runs);
            }
            b.tag_edit();
            b.placed.borrow_mut().take();
            b.content.borrow_mut().take();
            b.canvas.queue_draw();
            b.overlay.queue_allocate();
        });
        view.connect_preedit_changed(move |_, preedit| composing.set(!preedit.is_empty()));
        // A middle press pans the canvas; it never pastes the primary selection here (TM-77).
        let middle = gtk::GestureClick::new();
        middle.set_button(gdk::BUTTON_MIDDLE);
        middle.set_propagation_phase(gtk::PropagationPhase::Capture);
        middle.connect_pressed(|g, _, _, _| {
            g.set_state(gtk::EventSequenceState::Claimed);
        });
        view.add_controller(middle);
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, mods| {
            let Some(b) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
            // While an input method composes, its keys are its own (I-14).
            let composing = b.edit.borrow().as_ref().is_some_and(|e| e.composing.get());
            if !composing && (key == gdk::Key::Escape || (ctrl && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter))) {
                b.end_edit(true);
                return glib::Propagation::Stop;
            }
            // Tab ends the typing and gives the keyboard back to the canvas, rather than to the
            // dock's first key (TM-76).
            if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab) {
                b.end_edit(true);
                b.canvas.grab_focus();
                return glib::Propagation::Stop;
            }
            // Bold, italic and underline, as the shortcut sheet says, while typing (TM-79).
            let field = match key {
                gdk::Key::b | gdk::Key::B => "font_weight",
                gdk::Key::i | gdk::Key::I => "italic",
                gdk::Key::u | gdk::Key::U => "underline",
                _ => "",
            };
            if ctrl && !field.is_empty() && !composing {
                b.toggle_typed(field);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        view.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_leave(move |_| {
            // Leaving for a popover of the board's own (a colour, a size) keeps the edit;
            // leaving the window ends it.
            let w = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(b) = w.upgrade() {
                    let still = b.edit.borrow().as_ref().is_some_and(|e| e.view.has_focus());
                    let window = b.root.root().and_downcast::<gtk::Window>();
                    let in_board = window.as_ref().and_then(gtk::prelude::GtkWindowExt::focus).is_some_and(|f| f.is_ancestor(&b.root));
                    let away = window.is_some_and(|w| !w.is_active());
                    if (!still && !in_board) || away {
                        b.end_edit(true);
                    }
                }
            });
        });
        view.add_controller(focus);
        // Another window taken up (a popped-out board's person gone to the main one) ends the
        // typing, so what an agent changes meanwhile shows rather than waiting on it (TM-75).
        if let Some(window) = self.root.root().and_downcast::<gtk::Window>() {
            let weak = Rc::downgrade(self);
            let handler = window.connect_is_active_notify(move |w| {
                if let Some(b) = weak.upgrade().filter(|_| !w.is_active()) {
                    b.end_edit(true);
                }
            });
            if let Some(e) = self.edit.borrow().as_ref() {
                *e.window.borrow_mut() = Some((window.downgrade(), handler));
            }
        }
        view.grab_focus();
        if created {
            buffer.place_cursor(&buffer.end_iter());
        } else {
            buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        }
        self.rebuild_context();
    }

    /// The edited node's text box, in page coordinates: a wrapping text exactly as wide as the
    /// canvas wraps it (TM-32, TM-7), a hugging one with room for the caret past its last
    /// letter, and as tall as the view's own lines, so the last ones are never cut (TM-31).
    pub(super) fn edit_rect(&self) -> Option<Rect> {
        let (id, view) = self.edit.borrow().as_ref().map(|e| (e.id.clone(), e.view.clone()))?;
        let placed = self.placed();
        let r = placed.rect(&id)?;
        let scene = self.doc.scene.borrow();
        let n = scene.node(&id)?;
        let zoom = self.cam.get().zoom;
        let rect = if is_shape(n.kind) {
            super::paint::shape_text_box(&self.painter, n, &r)?
        } else if n.kind == NodeKind::Sticky {
            Rect::new(r.x + STICKY_PAD, r.y + STICKY_PAD, (r.w - 2.0 * STICKY_PAD).max(1.0), (r.h - 2.0 * STICKY_PAD).max(1.0))
        } else {
            match placed.wrap_width(n, &r) {
                Some(w) => Rect::new(r.x, r.y, w, r.h),
                None => Rect::new(r.x, r.y, r.w + 4.0 / zoom, r.h),
            }
        };
        drop(scene);
        let (_, tall, _, _) = view.measure(gtk::Orientation::Vertical, (rect.w * zoom).ceil().max(1.0) as i32);
        Some(Rect { h: rect.h.max(tall as f64 / zoom), ..rect })
    }

    /// Give the text view the node's look at the current zoom: its CSS (I-13), its opacity
    /// through all that holds it (B108), how it aligns and wraps, and its tags. Made again only
    /// when the zoom or the look changed (E-text-10: a style pasted while typing restyles it).
    pub(super) fn style_edit(&self) {
        let edit = self.edit.borrow();
        let Some(edit) = edit.as_ref() else {
            return;
        };
        let zoom = self.cam.get().zoom;
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let Some(n) = scene.node(&edit.id) else {
            return;
        };
        // The canvas draws the words of a turned node, or one in a turned frame (B108,
        // E-text-13): the view, which cannot turn, keeps its caret and selection and shows
        // its letters clear.
        let clear = self.painter.draws_edited(&scene, &edit.id);
        let ink = text_color(&scene, n);
        let opacity = faded(&scene, &edit.id);
        let wraps = n.kind == NodeKind::Sticky || is_shape(n.kind) || placed.rect(&n.id).is_some_and(|r| placed.wrap_width(n, &r).is_some());
        let look = look_key(n, &ink, clear, opacity, wraps);
        if edit.zoom.get() == zoom && edit.look.get() == look {
            return;
        }
        edit.zoom.set(zoom);
        edit.look.set(look);
        edit.css.load_from_string(&edit_css(&edit.class, n, zoom, &ink, clear));
        edit.view.set_opacity(opacity);
        let default = if is_shape(n.kind) { TextAlign::Center } else { TextAlign::Left };
        edit.view.set_justification(match n.align.unwrap_or(default) {
            TextAlign::Center => gtk::Justification::Center,
            TextAlign::Right => gtk::Justification::Right,
            TextAlign::Left => gtk::Justification::Left,
        });
        edit.view.set_wrap_mode(if wraps { gtk::WrapMode::WordChar } else { gtk::WrapMode::None });
        self.tag_edit();
    }

    /// Lay the edit's tags over its words: Title case's capitals at each word's first letter,
    /// as the canvas draws them (TM-34), and a tag a run, in the run's own look.
    fn tag_edit(&self) {
        let edit = self.edit.borrow();
        let Some(edit) = edit.as_ref() else {
            return;
        };
        let scene = self.doc.scene.borrow();
        let Some(n) = scene.node(&edit.id) else {
            return;
        };
        let buffer = edit.view.buffer();
        let (start, end) = buffer.bounds();
        buffer.remove_tag(&edit.upper, &start, &end);
        if n.text_case == Some(TextCase::Title) {
            let words = buffer.text(&start, &end, false);
            for at in title_starts(&words) {
                let from = buffer.iter_at_offset(at as i32);
                let mut to = from;
                to.forward_char();
                buffer.apply_tag(&edit.upper, &from, &to);
            }
        }
        let table = buffer.tag_table();
        for tag in edit.run_tags.borrow_mut().drain(..) {
            table.remove(&tag);
        }
        let zoom = self.cam.get().zoom;
        let clear = self.painter.draws_edited(&scene, &edit.id);
        for run in n.runs.iter().flatten() {
            let Some(tag) = buffer.create_tag(None, &[]) else {
                continue;
            };
            run_tag(&tag, run, zoom, clear);
            // A larger run raises its line as the canvas does: its line height times its size,
            // as a multiple of its font's own line, which is what the tag takes.
            if let Some(px) = run.font_size {
                let mut probe = n.clone();
                probe.font_size = Some(px);
                probe.font = run.font.clone().or_else(|| n.font.clone());
                tag.set_line_height((n.line_height.unwrap_or(1.3) * px / self.painter.natural_line(&probe)) as f32);
            }
            buffer.apply_tag(&tag, &buffer.iter_at_offset(run.start as i32), &buffer.iter_at_offset(run.end as i32));
            edit.run_tags.borrow_mut().push(tag);
        }
    }

    /// Ctrl+B, Ctrl+I or Ctrl+U while typing (TM-79): bold (to 700, or back to 400 from 600 and
    /// over), italic or underline toggled on the selected words as a run, or on the whole text
    /// when nothing is selected, and the words typed so far go with it in one step, so the
    /// style is never sent over older words. The editor stays open.
    fn toggle_typed(&self, field: &str) {
        let Some((id, view, created)) = self.edit.borrow().as_ref().map(|e| (e.id.clone(), e.view.clone(), e.created)) else {
            return;
        };
        let buffer = view.buffer();
        let words = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
        let Some(n) = self.doc.scene.borrow().node(&id).cloned() else {
            return;
        };
        let runs = self.edit.borrow().as_ref().map(|e| e.carry(&words)).unwrap_or_default();
        let mut set = Map::new();
        match buffer.selection_bounds().map(|(a, b)| (a.offset() as u32, b.offset() as u32)).filter(|(a, b)| a < b) {
            Some((from, to)) => {
                let runs = toggled_runs(&n, &runs, from, to, field);
                set.insert("runs".into(), if runs.is_empty() { Value::Null } else { json!(runs) });
            }
            None => {
                set.insert(field.into(), toggled(&n, field));
                if !runs.is_empty() {
                    set.insert("runs".into(), json!(runs));
                }
            }
        }
        if created {
            // Not on the engine yet: styled in place, it goes out with its words.
            let patched = n.patched(&set).ok();
            if let (Some(p), Some(node)) = (patched, self.doc.scene.borrow_mut().node_mut(&id)) {
                *node = p;
            }
        } else {
            if !words.is_empty() {
                set.insert("text".into(), json!(words));
                set.entry("runs").or_insert(Value::Null);
            }
            if !self.commit(vec![NodeOp::Update { id: id.clone(), set }]) {
                return;
            }
        }
        // What was sent (or styled in place) is what typing goes on from: Esc keeps it.
        if let (Some(e), Some(now)) = (self.edit.borrow().as_ref(), self.doc.scene.borrow().node(&id)) {
            if !created {
                let mut kept = e.original.borrow_mut();
                kept.text.clone_from(&now.text);
                kept.runs.clone_from(&now.runs);
            }
            *e.last.borrow_mut() = (now.text.clone().unwrap_or_default(), now.runs.clone().unwrap_or_default());
            e.zoom.set(0.0);
        }
        self.invalidate();
        self.style_edit();
    }

    /// Stop typing: send the text if it changed (`commit`), or drop the edit.
    pub(super) fn end_edit(&self, commit: bool) {
        let Some(edit) = self.edit.borrow_mut().take() else {
            return;
        };
        // Ended by a click elsewhere (another view, a field), the keyboard stays where it went.
        let had_keyboard = edit.view.is_focus();
        *self.painter.editing.borrow_mut() = None;
        let buffer = edit.view.buffer();
        let typed = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
        // Never more than a text holds, whatever reached the buffer (B41).
        let (text, cut) = cut_text(&typed, MAX_TEXT);
        let runs = edit.carry(&text);
        if let Some((window, handler)) = edit.window.borrow_mut().take() {
            if let Some(window) = window.upgrade() {
                window.disconnect(handler);
            }
        }
        gtk::style_context_remove_provider_for_display(&self.canvas.display(), &edit.css);
        self.overlay.remove_overlay(&edit.view);
        // Back to the words it had; the edit below makes the change for real. A style set
        // while typing has been sent already (or, on a new text, rides along with it).
        let original = edit.original.borrow().clone();
        let current = self.doc.scene.borrow().stored(&edit.id).cloned();
        {
            let mut scene = self.doc.scene.borrow_mut();
            if edit.created {
                scene.remove_preview(&edit.id);
            } else if let Some(n) = scene.node_mut(&edit.id) {
                n.text.clone_from(&original.text);
                n.runs.clone_from(&original.runs);
            }
        }
        self.placed.borrow_mut().take();
        self.doc.set_hold(false);
        let before = original.text.clone().unwrap_or_default();
        let runs_value = || if runs.is_empty() { Value::Null } else { json!(runs) };
        let empty = text.trim().is_empty();
        if commit {
            let sent = if edit.created {
                if empty {
                    self.select(Vec::new());
                    true
                } else {
                    // As stored: the new text's colour and style names, not what they resolve to.
                    let mut node = current.unwrap_or_else(|| original.clone());
                    node.text = Some(text.clone());
                    node.runs = (!runs.is_empty()).then(|| runs.clone());
                    let sent = self.commit(vec![NodeOp::Add { node, index: None }]);
                    self.select(vec![edit.id.clone()]);
                    sent
                }
            } else if text != before || original.runs.clone().unwrap_or_default() != runs {
                let ops = if empty && original.kind == NodeKind::Text {
                    // A connector attached to it lets go where it is drawn (TM-60).
                    let placed = self.placed();
                    edit::delete(&self.doc.scene.borrow(), &placed, std::slice::from_ref(&edit.id))
                } else {
                    let mut set = Map::new();
                    // A shape whose label was cleared has none, nor runs.
                    set.insert("text".into(), if empty { Value::Null } else { json!(text) });
                    if original.runs.is_some() || !runs.is_empty() {
                        set.insert("runs".into(), if empty { Value::Null } else { runs_value() });
                    }
                    vec![NodeOp::Update { id: edit.id.clone(), set }]
                };
                self.commit(ops)
            } else {
                true
            };
            // Refused (the node gone meanwhile, the design full): the words are not lost (I-14).
            if !sent && !empty {
                self.canvas.clipboard().set_text(&text);
                self.toast("Couldn't save the text: it's on the clipboard");
            } else if cut {
                self.toast(&format!("A text holds at most {MAX_TEXT} characters: the rest was left out"));
            }
        }
        self.invalidate();
        if had_keyboard {
            self.canvas.grab_focus();
        }
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

/// How opaque a node shows: its own opacity times that of everything it is in (B108).
fn faded(scene: &Scene, id: &str) -> f64 {
    std::iter::once(id).chain(scene.ancestors(id)).filter_map(|a| scene.node(a)).map(|n| n.opacity.unwrap_or(1.0).clamp(0.0, 1.0)).product()
}

/// What the editor's look is made from, beyond the zoom: when it changes, the look is made again.
fn look_key(n: &Node, ink: &str, clear: bool, opacity: f64, wraps: bool) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&n.font, font_size(n).to_bits(), n.font_weight, n.italic, n.underline, n.strike).hash(&mut h);
    (n.line_height.map(f64::to_bits), n.letter_spacing.map(f64::to_bits), n.text_case, n.align).hash(&mut h);
    (ink, clear, opacity.to_bits(), wraps).hash(&mut h);
    h.finish()
}

/// A colour as CSS reads it, its alpha included.
fn css_colour(hex: &str) -> String {
    let [r, g, b, a] = parse_color(hex).unwrap_or([0x1A, 0x1A, 0x1A, 255]);
    format!("rgba({r}, {g}, {b}, {:.3})", f64::from(a) / 255.0)
}

/// The CSS that gives the editor of `n` its look at `zoom`, on the view's own `class` (I-13):
/// the font as the canvas sets it; the size, line height and tracking in pixels, as the canvas
/// lays its lines (TM-31), tracking negative too (E-text-1), so an empty text already shows in
/// its own look (E-text-2); no tabular figures from the app's theme (TM-33); upper or lower case
/// as drawn; the words in `ink`, or clear where the canvas draws them (`clear`); the caret in
/// `ink` (E-text-3); selected words in `ink` on the canvas's selection blue (TM-78).
fn edit_css(class: &str, n: &Node, zoom: f64, ink: &str, clear: bool) -> String {
    let size = font_size(n);
    let family = strip_controls(font_family(n), false).replace('\\', "\\\\").replace('"', "\\\"");
    let weight = (n.font_weight.unwrap_or(400).saturating_add(50) / 100 * 100).clamp(100, 900);
    let style = if n.italic == Some(true) { "italic" } else { "normal" };
    let lines = match (n.underline == Some(true), n.strike == Some(true)) {
        (true, true) => "underline line-through",
        (true, false) => "underline",
        (false, true) => "line-through",
        (false, false) => "none",
    };
    let case = match n.text_case {
        Some(TextCase::Upper) => "uppercase",
        Some(TextCase::Lower) => "lowercase",
        _ => "none",
    };
    let ink = css_colour(ink);
    let words = if clear { "transparent".to_string() } else { ink.clone() };
    format!(
        "textview.{class}, textview.{class} text {{ font-family: \"{family}\"; font-size: {:.3}px; font-weight: {weight}; font-style: {style}; \
         line-height: {:.3}px; letter-spacing: {:.3}px; font-feature-settings: normal; text-decoration-line: {lines}; text-transform: {case}; \
         color: {words}; caret-color: {ink}; }}\n\
         textview.{class} text selection {{ color: {words}; background-color: alpha(#4C9DFF, 0.32); }}\n",
        size * zoom,
        n.line_height.unwrap_or(1.3) * size * zoom,
        n.letter_spacing.unwrap_or(0.0) * zoom,
    )
}

/// Pango's weight for a stored one, by the painter's steps.
fn pango_weight(w: u16) -> pango::Weight {
    match w {
        0..=149 => pango::Weight::Thin,
        150..=249 => pango::Weight::Ultralight,
        250..=349 => pango::Weight::Light,
        350..=449 => pango::Weight::Normal,
        450..=549 => pango::Weight::Medium,
        550..=649 => pango::Weight::Semibold,
        650..=749 => pango::Weight::Bold,
        750..=849 => pango::Weight::Ultrabold,
        _ => pango::Weight::Heavy,
    }
}

/// A run's look on its tag: only what the run sets, over the view's own look; its colour left
/// out where the canvas draws the words (`clear`).
fn run_tag(tag: &gtk::TextTag, run: &TextRun, zoom: f64, clear: bool) {
    let mut font = pango::FontDescription::new();
    if let Some(family) = &run.font {
        font.set_family(&strip_controls(family, false));
    }
    if let Some(px) = run.font_size {
        font.set_absolute_size(px * zoom * pango::SCALE as f64);
    }
    if let Some(w) = run.font_weight {
        font.set_weight(pango_weight(w));
    }
    if let Some(italic) = run.italic {
        font.set_style(if italic { pango::Style::Italic } else { pango::Style::Normal });
    }
    tag.set_font_desc(Some(&font));
    match (&run.color, clear) {
        (_, true) => tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.0, 0.0, 0.0, 0.0))),
        (Some(c), false) => tag.set_foreground_rgba(Some(&rgba(c))),
        (None, false) => {}
    }
    if let Some(rise) = run.rise {
        tag.set_rise((rise * zoom * pango::SCALE as f64).round() as i32);
    }
    if let Some(on) = run.underline {
        tag.set_underline(if on { pango::Underline::Single } else { pango::Underline::None });
    }
    if let Some(on) = run.strike {
        tag.set_strikethrough(on);
    }
}

/// The value Ctrl+B, Ctrl+I or Ctrl+U gives a whole text: bold to 700, or back to the default
/// from 600 and over; italic and underline turned on, or off as none.
fn toggled(n: &Node, field: &str) -> Value {
    let on = match field {
        "font_weight" => n.font_weight.unwrap_or(400) >= 600,
        "italic" => n.italic == Some(true),
        _ => n.underline == Some(true),
    };
    match (on, field) {
        (true, _) => Value::Null,
        (false, "font_weight") => json!(700),
        (false, _) => json!(true),
    }
}

/// `runs` with characters `from..to` of `n`'s text toggled as Ctrl+B, Ctrl+I or Ctrl+U toggles
/// a selection (TM-79): on, unless every character of it is on already (by its run or by the
/// text's own style), then off. A run takes the field only where it differs from the text's
/// own, and a stretch left with no style of its own is no run.
fn toggled_runs(n: &Node, runs: &[TextRun], from: u32, to: u32, field: &str) -> Vec<TextRun> {
    let own = |r: &TextRun| match field {
        "font_weight" => r.font_weight.unwrap_or(n.font_weight.unwrap_or(400)) >= 600,
        "italic" => r.italic.or(n.italic).unwrap_or(false),
        _ => r.underline.or(n.underline).unwrap_or(false),
    };
    // Each character by its run, or by the text's own style where no run holds it.
    let base = blank(0, 0);
    let on = !(from..to).all(|at| own(runs.iter().find(|r| r.start <= at && at < r.end).unwrap_or(&base)));
    let text_on = own(&base);
    restyle(runs, from, to, &|r| match field {
        "font_weight" => r.font_weight = (on != text_on).then_some(if on { 700 } else { 400 }),
        "italic" => r.italic = (on != text_on).then_some(on),
        _ => r.underline = (on != text_on).then_some(on),
    })
}

/// A run with no style of its own, over `start..end`.
fn blank(start: u32, end: u32) -> TextRun {
    TextRun { start, end, color: None, font: None, font_size: None, font_weight: None, italic: None, underline: None, strike: None, rise: None }
}

/// `runs` with characters `from..to` restyled by `apply`, each stretch from the look it had (its
/// run's, or none): split where a run or the stretch starts or ends, then joined again where
/// neighbours look the same, a stretch with no style left out.
fn restyle(runs: &[TextRun], from: u32, to: u32, apply: &dyn Fn(&mut TextRun)) -> Vec<TextRun> {
    let mut cuts: Vec<u32> = runs.iter().flat_map(|r| [r.start, r.end]).chain([from, to]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let look = |r: &TextRun| TextRun { start: 0, end: 0, ..r.clone() };
    let mut out: Vec<TextRun> = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let mut piece = runs.iter().find(|r| r.start <= a && b <= r.end).cloned().unwrap_or_else(|| blank(a, b));
        (piece.start, piece.end) = (a, b);
        if from <= a && b <= to {
            apply(&mut piece);
        }
        if look(&piece) == blank(0, 0) {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end == a && look(last) == look(&piece) => last.end = b,
            _ => out.push(piece),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(start: u32, end: u32) -> TextRun {
        blank(start, end)
    }

    fn bold(start: u32, end: u32) -> TextRun {
        TextRun { font_weight: Some(700), ..blank(start, end) }
    }

    #[test]
    fn the_editor_looks_as_the_canvas_draws() {
        let mut n = Node::new(NodeKind::Text);
        n.font = Some("Geist \"Mono\"".into());
        n.font_size = Some(16.0);
        n.line_height = Some(1.5);
        n.letter_spacing = Some(-0.6);
        n.font_weight = Some(640);
        n.underline = Some(true);
        n.strike = Some(true);
        n.text_case = Some(TextCase::Upper);
        let css = edit_css("design-edit-7", &n, 2.0, "#1A1A1A80", false);
        // Pixels, as the canvas lays them, at the zoom: 16 × 2 and 1.5 × 16 × 2 (TM-31).
        assert!(css.contains("font-size: 32.000px"), "{css}");
        assert!(css.contains("line-height: 48.000px"), "{css}");
        // Tracking may be negative (E-text-1), and no tabular figures (TM-33).
        assert!(css.contains("letter-spacing: -1.200px"), "{css}");
        assert!(css.contains("font-feature-settings: normal"), "{css}");
        assert!(css.contains("font-family: \"Geist \\\"Mono\\\"\""), "{css}");
        assert!(css.contains("font-weight: 600"), "{css}");
        assert!(css.contains("text-decoration-line: underline line-through"), "{css}");
        assert!(css.contains("text-transform: uppercase"), "{css}");
        // The words, the caret and the selected words in the text's colour (E-text-3, TM-78).
        assert!(css.contains("color: rgba(26, 26, 26, 0.502); caret-color: rgba(26, 26, 26, 0.502)"), "{css}");
        assert!(css.contains("textview.design-edit-7 text selection { color: rgba(26, 26, 26, 0.502)"), "{css}");
        // Where the canvas draws the words (a turned node), the view's are clear, its caret not.
        let clear = edit_css("design-edit-7", &n, 1.0, "#FFFFFF", true);
        assert!(clear.contains("color: transparent; caret-color: rgba(255, 255, 255, 1.000)"), "{clear}");
        // Title case is the tags' (the canvas's word starts), not CSS's capitalize.
        n.text_case = Some(TextCase::Title);
        assert!(edit_css("c", &n, 1.0, "#000", false).contains("text-transform: none"));
    }

    #[test]
    fn a_shortcut_toggles_the_whole_text() {
        let mut n = Node::new(NodeKind::Text);
        assert_eq!(toggled(&n, "font_weight"), json!(700));
        n.font_weight = Some(600);
        assert_eq!(toggled(&n, "font_weight"), Value::Null);
        assert_eq!(toggled(&n, "italic"), json!(true));
        n.underline = Some(true);
        assert_eq!(toggled(&n, "underline"), Value::Null);
    }

    #[test]
    fn a_shortcut_toggles_the_selected_words_as_a_run() {
        let n = Node::new(NodeKind::Text);
        // "hello world": bold on "world".
        assert_eq!(toggled_runs(&n, &[], 6, 11, "font_weight"), vec![bold(6, 11)]);
        // Again on the same words: off, and no run is left.
        assert_eq!(toggled_runs(&n, &[bold(6, 11)], 6, 11, "font_weight"), Vec::<TextRun>::new());
        // Over a stretch half bold: all bold, joined in one run.
        assert_eq!(toggled_runs(&n, &[bold(6, 11)], 0, 8, "font_weight"), vec![bold(0, 11)]);
        // Off in the middle of a bold run: it splits round the words.
        assert_eq!(toggled_runs(&n, &[bold(0, 11)], 3, 5, "font_weight"), vec![bold(0, 3), bold(5, 11)]);
        // A run's other looks stay where the toggle goes.
        let grey = TextRun { color: Some("#888888".into()), ..run(0, 4) };
        assert_eq!(
            toggled_runs(&n, std::slice::from_ref(&grey), 2, 6, "italic"),
            vec![grey.clone().with_end(2), TextRun { italic: Some(true), ..grey.clone().with_range(2, 4) }, TextRun { italic: Some(true), ..run(4, 6) }]
        );
        // On a bold text, off is a run of the default weight; on again, nothing of its own.
        let mut heavy = Node::new(NodeKind::Text);
        heavy.font_weight = Some(700);
        let off = toggled_runs(&heavy, &[], 0, 5, "font_weight");
        assert_eq!(off, vec![TextRun { font_weight: Some(400), ..run(0, 5) }]);
        assert_eq!(toggled_runs(&heavy, &off, 0, 5, "font_weight"), Vec::<TextRun>::new());
    }

    trait Range {
        fn with_end(self, end: u32) -> Self;
        fn with_range(self, start: u32, end: u32) -> Self;
    }

    impl Range for TextRun {
        fn with_end(self, end: u32) -> Self {
            TextRun { end, ..self }
        }
        fn with_range(self, start: u32, end: u32) -> Self {
            TextRun { start, end, ..self }
        }
    }
}
