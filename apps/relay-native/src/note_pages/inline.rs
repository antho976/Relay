//! What the Notes editor draws over its Markdown: a real checkbox on every `- [ ]` item and
//! the picture under every line that references an image (`![Image 1](/path.png)`). The text
//! stays plain Markdown, which agents read as written; these are overlays on the text view
//! that follow it, and Plain text mode (View > Markdown highlighting off) turns them off.
//!
//! Pasted, dropped and chosen images are copied under the project's `.relay/notes/images/`
//! (git never sees `.relay/`) and referenced by absolute path, so an agent can open the file.
use super::doc::Doc;
use super::text as tx;
use super::*;
use gtk::gio;
use sourceview5::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The largest picture drawn in a note; the file itself is kept at full size.
const MAX_W: f64 = 440.;
const MAX_H: f64 = 300.;
const CHECK: i32 = 16;
/// Space around a picture in the room kept below its line, beyond its own height.
const ROOM: i32 = 18;

/// A decoded image, the modification time of the file it was read from, and when that was
/// last compared with the disk.
struct Decoded {
    modified: Option<std::time::SystemTime>,
    texture: Option<gtk::gdk::Texture>,
    /// None until the first read lands.
    checked: Option<std::time::Instant>,
    /// A read is on a worker thread; these notes redraw when it lands.
    waiting: Option<Vec<std::rc::Weak<Doc>>>,
}

/// A picture being dragged, as its drag offers it: nothing outside Relay, and no text view,
/// takes this type, so only `picture_drop` handles the drop. The number names the drag.
#[derive(Clone, Copy, Debug, glib::Boxed)]
#[boxed_type(name = "RelayNotePictureDrag")]
pub struct PictureDrag(u64);

/// Where a dragged picture's reference line is: a mark of its own at the line's start, made
/// when the drag began, so a redraw that hands the picture slots out again (`refresh`) can't
/// point the move at another picture.
struct Dragging {
    serial: u64,
    doc: std::rc::Weak<Doc>,
    mark: gtk::TextMark,
}

thread_local! {
    static DRAGGING: RefCell<Option<Dragging>> = const { RefCell::new(None) };
    static DRAG_SERIAL: Cell<u64> = const { Cell::new(0) };
    static TEXTURES: RefCell<HashMap<PathBuf, Decoded>> = RefCell::new(HashMap::new());
    /// Image files being written for a paste or drop, so a second one in the same second is
    /// not given the same name.
    static RESERVED: RefCell<std::collections::HashSet<PathBuf>> = RefCell::default();
}

/// How long a picture is drawn from memory before its file's modification time is looked at
/// again. The overlays are redrawn on every keystroke; the disk is not read on any of them.
const RECHECK: std::time::Duration = std::time::Duration::from_secs(2);

/// The picture `path` holds, as last read. One never read, or not compared with its file for
/// `RECHECK`, is read on a worker thread, and `doc` redraws when that changes what it shows.
fn texture(doc: &Rc<Doc>, path: &Path) -> Option<gtk::gdk::Texture> {
    let (texture, read) = TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.entry(path.to_path_buf()).or_insert_with(|| Decoded {
            modified: None,
            texture: None,
            checked: None,
            waiting: None,
        });
        let read = match &mut entry.waiting {
            Some(waiting) => {
                if !waiting.iter().any(|w| std::ptr::eq(w.as_ptr(), Rc::as_ptr(doc))) {
                    waiting.push(Rc::downgrade(doc));
                }
                None
            }
            None if entry.checked.is_none_or(|checked| checked.elapsed() >= RECHECK) => {
                entry.waiting = Some(vec![Rc::downgrade(doc)]);
                Some(entry.modified.filter(|_| entry.texture.is_some()))
            }
            None => None,
        };
        (entry.texture.clone(), read)
    });
    if let Some(known) = read {
        let path = path.to_path_buf();
        glib::spawn_future_local(async move {
            let file = path.clone();
            let read = gio::spawn_blocking(move || {
                let modified = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
                if modified.is_some() && modified == known {
                    return (modified, None);
                }
                (modified, Some(modified.and_then(|_| decode_fitted(&file))))
            })
            .await
            .unwrap_or((None, Some(None)));
            let (modified, decoded) = read;
            let waiting = TEXTURES.with(|cache| {
                let mut cache = cache.borrow_mut();
                let entry = cache.get_mut(&path)?;
                entry.checked = Some(std::time::Instant::now());
                let waiting = entry.waiting.take();
                entry.modified = modified;
                let decoded = decoded?;
                // Only a picture that changed (or came or went) redraws the notes showing it.
                let same = match (&entry.texture, &decoded) {
                    (Some(a), Some(b)) => a == b,
                    (None, None) => true,
                    _ => false,
                };
                entry.texture = decoded;
                (!same).then_some(waiting).flatten()
            });
            for doc in waiting.into_iter().flatten().filter_map(|doc| doc.upgrade()) {
                schedule(&doc);
            }
        });
    }
    texture
}

/// The picture in `file`, decoded no larger than twice the size it is drawn at, sharp on a
/// 2× screen: a 4K screenshot is 33 MB as a texture, and at most 440 × 300 of it shows.
/// Runs on a worker thread.
fn decode_fitted(file: &Path) -> Option<gtk::gdk::Texture> {
    use gtk::gdk_pixbuf::Pixbuf;
    let (most_w, most_h) = ((MAX_W * 2.) as i32, (MAX_H * 2.) as i32);
    match Pixbuf::file_info(file) {
        Some((_, w, h)) if w > most_w || h > most_h => {
            let pixbuf = Pixbuf::from_file_at_scale(file, most_w, most_h, true).ok()?;
            let format = if pixbuf.has_alpha() { gtk::gdk::MemoryFormat::R8g8b8a8 } else { gtk::gdk::MemoryFormat::R8g8b8 };
            let bytes = pixbuf.read_pixel_bytes();
            Some(gtk::gdk::MemoryTexture::new(pixbuf.width(), pixbuf.height(), format, &bytes, pixbuf.rowstride() as usize).upcast())
        }
        _ => gtk::gdk::Texture::from_filename(file).ok(),
    }
}

/// Drop the pictures no open note draws any more, once the current main-loop pass is done.
fn prune_soon() {
    thread_local!(static PRUNING: Cell<bool> = const { Cell::new(false) });
    if PRUNING.with(|p| p.replace(true)) {
        return;
    }
    glib::idle_add_local_once(|| {
        PRUNING.with(|p| p.set(false));
        let drawn: std::collections::HashSet<PathBuf> = super::DOCS.with(|docs| {
            docs.borrow()
                .values()
                .flat_map(|doc| {
                    doc.inline.pictures.borrow().iter().filter(|s| s.frame.is_visible()).map(|s| s.path.borrow().clone()).collect::<Vec<_>>()
                })
                .collect()
        });
        TEXTURES.with(|cache| cache.borrow_mut().retain(|path, decoded| decoded.waiting.is_some() || drawn.contains(path)));
    });
}

/// The size a picture is drawn at: its own, scaled down to fit `MAX_W` × `MAX_H`.
fn fitted(texture: &gtk::gdk::Texture) -> (i32, i32) {
    let (w, h) = (f64::from(texture.width().max(1)), f64::from(texture.height().max(1)));
    let scale = (MAX_W / w).min(MAX_H / h).min(1.);
    ((w * scale).round().max(1.) as i32, (h * scale).round().max(1.) as i32)
}

struct Check {
    button: gtk::CheckButton,
    /// The `[` of the item it stands on.
    mark: RefCell<Option<gtk::TextMark>>,
    syncing: Cell<bool>,
}

struct Shown {
    frame: gtk::Box,
    picture: gtk::Picture,
    path: RefCell<PathBuf>,
    /// The start of the line the reference sits on.
    mark: RefCell<Option<gtk::TextMark>>,
}

pub struct Inline {
    checks: RefCell<Vec<Rc<Check>>>,
    pictures: RefCell<Vec<Rc<Shown>>>,
    idle: RefCell<Option<glib::SourceId>>,
    place_idle: RefCell<Option<glib::SourceId>>,
    /// Paints an item's `- [ ]` fully transparent, under its checkbox (the paper is translucent,
    /// so its colour would show). Never `invisible`: GTK leaves invisible text out of what the
    /// buffer reads back, so a save would drop it.
    marker: gtk::TextTag,
    /// The line the cursor is on, shown as plain Markdown so it edits as typed.
    editing: Cell<i32>,
    /// A ticked item's text.
    done: gtk::TextTag,
    /// The reference line under a picture, quieter than prose.
    reference: gtk::TextTag,
    /// Room below a reference line for its picture, by height.
    rooms: RefCell<HashMap<i32, gtk::TextTag>>,
    /// Pasted or dropped pictures still being written: the note's tab waits for them to close.
    pub writing: Cell<u32>,
}

impl Inline {
    pub fn new(buffer: &sourceview5::Buffer) -> Self {
        let marker = buffer.create_tag(None, &[("foreground-rgba", &gtk::gdk::RGBA::TRANSPARENT)]).unwrap();
        let done = buffer
            .create_tag(None, &[("strikethrough", &true), ("foreground-rgba", &gtk::gdk::RGBA::new(0.6, 0.6, 0.6, 0.75))])
            .unwrap();
        let reference = buffer
            .create_tag(None, &[("foreground-rgba", &gtk::gdk::RGBA::new(0.47, 0.47, 0.48, 1.)), ("scale", &0.85_f64)])
            .unwrap();
        // Highlighting creates its tags as it meets new syntax, each above the last: keep ours
        // on top, or a list marker's colour shows through the hidden `- [ ]`.
        let ours = [marker.downgrade(), done.downgrade(), reference.downgrade()];
        buffer.tag_table().connect_tag_added(move |table, added| {
            let top = table.size() - 1;
            if ours.iter().any(|tag| tag.upgrade().as_ref() == Some(added)) {
                return;
            }
            for tag in ours.iter().filter_map(|tag| tag.upgrade()) {
                tag.set_priority(top);
            }
        });
        Self {
            checks: RefCell::new(Vec::new()),
            pictures: RefCell::new(Vec::new()),
            idle: RefCell::new(None),
            place_idle: RefCell::new(None),
            marker,
            editing: Cell::new(-1),
            done,
            reference,
            rooms: RefCell::new(HashMap::new()),
            writing: Cell::new(0),
        }
    }

    pub fn stop(&self) {
        for timer in [&self.idle, &self.place_idle] {
            if let Some(timer) = timer.borrow_mut().take() {
                timer.remove();
            }
        }
        // Its pictures go from memory once no other open note draws them.
        for shown in self.pictures.borrow().iter() {
            shown.frame.set_visible(false);
        }
        prune_soon();
    }
}

/// Find the items and pictures again after the text changed, once per main-loop pass.
pub fn schedule(doc: &Rc<Doc>) {
    if doc.inline.idle.borrow().is_some() {
        return;
    }
    let weak = Rc::downgrade(doc);
    *doc.inline.idle.borrow_mut() = Some(glib::idle_add_local_once(move || {
        if let Some(doc) = weak.upgrade() {
            doc.inline.idle.borrow_mut().take();
            refresh(&doc);
        }
    }));
}

/// The cursor moved: when it changed lines, the line it left gets its checkbox back and the
/// line it reached shows its Markdown.
pub fn cursor_moved(doc: &Rc<Doc>) {
    let line = doc.buffer.iter_at_mark(&doc.buffer.get_insert()).line();
    if line != doc.inline.editing.get() && super::prefs().markdown {
        doc.inline.editing.set(line);
        schedule(doc);
    }
}

/// Move the overlays to where their text is now: the view was resized, zoomed or re-wrapped.
pub fn schedule_place(doc: &Rc<Doc>) {
    if doc.inline.place_idle.borrow().is_some() {
        return;
    }
    let weak = Rc::downgrade(doc);
    *doc.inline.place_idle.borrow_mut() = Some(glib::idle_add_local_once(move || {
        if let Some(doc) = weak.upgrade() {
            doc.inline.place_idle.borrow_mut().take();
            place(&doc);
        }
    }));
}

fn take_mark(buffer: &sourceview5::Buffer, slot: &RefCell<Option<gtk::TextMark>>) {
    if let Some(mark) = slot.borrow_mut().take() {
        if !mark.is_deleted() {
            buffer.delete_mark(&mark);
        }
    }
}

fn refresh(doc: &Rc<Doc>) {
    let inline = &doc.inline;
    let buffer = &doc.buffer;
    let (start, end) = buffer.bounds();
    buffer.remove_tag(&inline.marker, &start, &end);
    buffer.remove_tag(&inline.done, &start, &end);
    buffer.remove_tag(&inline.reference, &start, &end);
    for room in inline.rooms.borrow().values() {
        buffer.remove_tag(room, &start, &end);
    }
    let mut checks = 0;
    let mut pictures = 0;
    if super::prefs().markdown {
        // Highlighting creates its tags as it meets new syntax: stay above them.
        let top = buffer.tag_table().size() - 1;
        for tag in [&inline.marker, &inline.done, &inline.reference] {
            tag.set_priority(top);
        }
        let body = doc.body();
        let editing = buffer.iter_at_mark(&buffer.get_insert()).line();
        inline.editing.set(editing);
        let mut fenced = false;
        for (number, line) in body.split('\n').enumerate() {
            if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            let Some(at) = buffer.iter_at_line(number as i32) else { break };
            // The line being edited shows its Markdown as typed, so the box never hides text
            // under the cursor; the text does not move when it comes back.
            let item = tx::check_item(line).filter(|_| number as i32 != editing);
            if let Some((column, ticked)) = item {
                let mut first = at;
                first.set_line_offset(column as i32 - 2);
                let mut bracket = at;
                bracket.set_line_offset(column as i32);
                let mut text = at;
                text.set_line_offset(column as i32 + 4);
                let mut end = at;
                if !end.ends_line() {
                    end.forward_to_line_end();
                }
                buffer.apply_tag(&inline.marker, &first, &text);
                if ticked {
                    buffer.apply_tag(&inline.done, &text, &end);
                }
                let check = check_slot(doc, checks);
                take_mark(buffer, &check.mark);
                *check.mark.borrow_mut() = Some(buffer.create_mark(None, &bracket, true));
                check.syncing.set(true);
                check.button.set_active(ticked);
                check.syncing.set(false);
                // A screen reader names the box by its item.
                let item = line.get(column + 4..).unwrap_or("").trim();
                check.button.update_property(&[gtk::accessible::Property::Label(if item.is_empty() { "Checklist item" } else { item })]);
                check.button.set_visible(true);
                checks += 1;
            }
            if let Some((_, path, from, to)) = tx::image_ref(line) {
                let path = PathBuf::from(&path);
                let Some(picture) = texture(doc, &path) else { continue };
                let (w, h) = fitted(&picture);
                let shown = picture_slot(doc, pictures);
                shown.picture.set_paintable(Some(&picture));
                shown.picture.set_size_request(w, h);
                shown.picture.set_alternative_text(Some(&path.to_string_lossy()));
                shown.frame.update_property(&[gtk::accessible::Property::Label(&format!(
                    "Picture {}. Menu key on its line for its actions",
                    path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
                ))]);
                shown.frame.set_tooltip_text(Some(&format!(
                    "{}\nDouble-click to copy the image · drag to move it · right-click for more",
                    path.display()
                )));
                *shown.path.borrow_mut() = path;
                take_mark(buffer, &shown.mark);
                *shown.mark.borrow_mut() = Some(buffer.create_mark(None, &at, true));
                let mut a = at;
                a.set_line_offset(from as i32);
                let mut b = at;
                b.set_line_offset(to as i32);
                buffer.apply_tag(&inline.reference, &a, &b);
                let mut end = at;
                if !end.ends_line() {
                    end.forward_to_line_end();
                }
                let room = room(buffer, inline, h + ROOM);
                buffer.apply_tag(&room, &at, &end);
                shown.frame.set_visible(true);
                pictures += 1;
            }
        }
    }
    for check in inline.checks.borrow().iter().skip(checks) {
        check.button.set_visible(false);
        take_mark(buffer, &check.mark);
    }
    let mut dropped = false;
    for shown in inline.pictures.borrow().iter().skip(pictures) {
        dropped |= shown.frame.is_visible();
        shown.frame.set_visible(false);
        shown.picture.set_paintable(gtk::gdk::Paintable::NONE);
        take_mark(buffer, &shown.mark);
    }
    if dropped {
        prune_soon();
    }
    place(doc);
}

fn room(buffer: &sourceview5::Buffer, inline: &Inline, height: i32) -> gtk::TextTag {
    let mut rooms = inline.rooms.borrow_mut();
    let tag = rooms
        .entry(height)
        .or_insert_with(|| buffer.create_tag(None, &[("pixels-below-lines", &height)]).unwrap())
        .clone();
    tag.set_priority(buffer.tag_table().size() - 1);
    tag
}

/// Put every overlay over its text. Positions are buffer coordinates, so they scroll with it.
fn place(doc: &Rc<Doc>) {
    let view = &doc.view;
    let buffer = &doc.buffer;
    for check in doc.inline.checks.borrow().iter() {
        let Some(mark) = check.mark.borrow().clone().filter(|m| !m.is_deleted()) else { continue };
        // Centred over the item's `[ ]`.
        let at = buffer.iter_at_mark(&mark);
        let open = view.iter_location(&at);
        let mut close = at;
        close.forward_chars(2);
        let close = view.iter_location(&close);
        let x = (open.x() + close.x() + close.width() - CHECK) / 2;
        view.move_overlay(&check.button, x, open.y() + (open.height() - CHECK) / 2);
    }
    for shown in doc.inline.pictures.borrow().iter() {
        let Some(mark) = shown.mark.borrow().clone().filter(|m| !m.is_deleted()) else { continue };
        let at = buffer.iter_at_mark(&mark);
        // The paragraph's range includes the room below it, which the picture fills.
        let (top, height) = view.line_yrange(&at);
        let (_, h) = shown.picture.size_request();
        view.move_overlay(&shown.frame, view.iter_location(&at).x(), top + height - ROOM + 6 - h);
    }
}

fn check_slot(doc: &Rc<Doc>, index: usize) -> Rc<Check> {
    if let Some(check) = doc.inline.checks.borrow().get(index) {
        return check.clone();
    }
    let button = gtk::CheckButton::new();
    button.add_css_class("notes-check");
    button.set_focus_on_click(false);
    button.set_can_focus(false);
    button.set_size_request(CHECK, CHECK);
    button.set_cursor_from_name(Some("pointer"));
    button.set_tooltip_text(Some("Tick or untick (Ctrl+Enter on the line)"));
    let check = Rc::new(Check { button, mark: RefCell::new(None), syncing: Cell::new(false) });
    doc.view.add_overlay(&check.button, 0, 0);
    let (weak, slot) = (Rc::downgrade(doc), Rc::downgrade(&check));
    check.button.connect_toggled(move |_| {
        let (Some(doc), Some(check)) = (weak.upgrade(), slot.upgrade()) else { return };
        if check.syncing.get() {
            return;
        }
        let mark = check.mark.borrow().clone().filter(|m| !m.is_deleted());
        if let Some(mark) = mark {
            doc.toggle_check_line(doc.buffer.iter_at_mark(&mark).line());
        }
    });
    doc.inline.checks.borrow_mut().push(check.clone());
    check
}

fn picture_slot(doc: &Rc<Doc>, index: usize) -> Rc<Shown> {
    if let Some(shown) = doc.inline.pictures.borrow().get(index) {
        return shown.clone();
    }
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("notes-image");
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    frame.append(&picture);
    frame.set_cursor_from_name(Some("grab"));
    let shown = Rc::new(Shown {
        frame,
        picture,
        path: RefCell::new(PathBuf::new()),
        mark: RefCell::new(None),
    });
    doc.view.add_overlay(&shown.frame, 0, 0);
    let (weak, slot) = (Rc::downgrade(doc), Rc::downgrade(&shown));

    let clicks = gtk::GestureClick::new();
    clicks.set_button(0);
    clicks.connect_pressed(move |gesture, presses, x, y| {
        let (Some(doc), Some(shown)) = (weak.upgrade(), slot.upgrade()) else { return };
        let path = shown.path.borrow().clone();
        if gesture.current_button() == 3 {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            image_menu(&doc, &shown, x, y);
        } else if presses == 2 {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            copy_images(&doc.view.clipboard(), reference_of(&doc, &shown).as_deref(), &[path]);
            doc.flash("Image copied");
        }
    });
    shown.frame.add_controller(clicks);

    // Dragging a picture moves its reference line between the lines it is dropped at
    // (`picture_drop`), in this note or another open one.
    let drag = gtk::DragSource::new();
    drag.set_actions(gtk::gdk::DragAction::MOVE);
    let (weak, slot) = (Rc::downgrade(doc), Rc::downgrade(&shown));
    drag.connect_prepare(move |drag, _, _| {
        let (doc, shown) = (weak.upgrade()?, slot.upgrade()?);
        let mark = shown.mark.borrow().clone().filter(|m| !m.is_deleted())?;
        if let Some(paintable) = shown.picture.paintable() {
            drag.set_icon(Some(&paintable), 12, 12);
        }
        let serial = DRAG_SERIAL.with(|s| {
            s.set(s.get() + 1);
            s.get()
        });
        // Right gravity: text put in at the line's start goes before the mark, which stays
        // on the picture's own line.
        let own = doc.buffer.create_mark(None, &doc.buffer.iter_at_mark(&mark), false);
        end_drag();
        DRAGGING.with(|d| *d.borrow_mut() = Some(Dragging { serial, doc: Rc::downgrade(&doc), mark: own }));
        Some(gtk::gdk::ContentProvider::for_value(&PictureDrag(serial).to_value()))
    });
    drag.connect_drag_end(|_, _, _| end_drag());
    shown.frame.add_controller(drag);
    doc.inline.pictures.borrow_mut().push(shown.clone());
    shown
}

/// The Menu key (or Shift+F10) on a picture's reference line opens that picture's menu, which
/// is otherwise a right-click away: the picture itself takes no keyboard focus.
pub fn menu_keys(doc: &Rc<Doc>) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(doc);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let menu = key == gtk::gdk::Key::Menu || (key == gtk::gdk::Key::F10 && modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK));
        let Some(doc) = weak.upgrade().filter(|_| menu) else { return glib::Propagation::Proceed };
        let line = doc.buffer.iter_at_mark(&doc.buffer.get_insert()).line();
        let shown = doc.inline.pictures.borrow().iter().find(|s| {
            s.frame.is_visible() && s.mark.borrow().as_ref().is_some_and(|m| !m.is_deleted() && doc.buffer.iter_at_mark(m).line() == line)
        }).cloned();
        match shown {
            Some(shown) => {
                let (w, h) = (shown.frame.width(), shown.frame.height());
                image_menu(&doc, &shown, f64::from(w / 2), f64::from(h / 2));
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    });
    keys
}

/// The reference line a picture is drawn for, which a copy of it pastes back as in a note: a
/// paste this window copied itself goes in as text (`paste`), and a bare path is no picture.
fn reference_of(doc: &Doc, shown: &Shown) -> Option<String> {
    let mark = shown.mark.borrow().clone().filter(|m| !m.is_deleted())?;
    let line = line_text(&doc.buffer, doc.buffer.iter_at_mark(&mark).line());
    tx::image_ref(&line).map(|(alt, path, ..)| tx::image_markdown(&alt, &path))
}

/// Forget the picture drag in progress, and its mark.
fn end_drag() {
    if let Some(drag) = DRAGGING.with(|d| d.borrow_mut().take()) {
        if let (Some(doc), false) = (drag.doc.upgrade(), drag.mark.is_deleted()) {
            doc.buffer.delete_mark(&drag.mark);
        }
    }
}

/// A dragged picture dropped on a note's text: its reference line moves to the line boundary
/// nearest the pointer, as one undo step in its own note. A drop on the picture's own line,
/// or on the boundary just under it, changes nothing.
pub fn picture_drop(doc: &Rc<Doc>) -> gtk::DropTarget {
    let target = gtk::DropTarget::new(PictureDrag::static_type(), gtk::gdk::DragAction::MOVE);
    let weak = Rc::downgrade(doc);
    target.connect_drop(move |_, value, x, y| {
        let (Some(doc), Ok(PictureDrag(serial))) = (weak.upgrade(), value.get::<PictureDrag>()) else { return false };
        let Some((from, mark)) = DRAGGING.with(|d| {
            d.borrow().as_ref().filter(|d| d.serial == serial && !d.mark.is_deleted()).and_then(|d| Some((d.doc.upgrade()?, d.mark.clone())))
        }) else {
            return false;
        };
        // By height alone: a drop in the margin, or past a line's end, still has a line.
        let (_, by) = doc.view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let (over, top) = doc.view.line_at_y(by);
        // The boundary above the line under the pointer, or below it from the line's lower
        // half (a picture's line includes the room it is drawn in).
        let (_, height) = doc.view.line_yrange(&over);
        let below = by > top + height / 2;
        let source = from.buffer.iter_at_mark(&mark).line();
        let target_line = over.line() + i32::from(below);
        if Rc::ptr_eq(&from, &doc) && (target_line == source || target_line == source + 1) {
            end_drag();
            return true;
        }
        let line = line_text(&from.buffer, source);
        let buffer = &doc.buffer;
        buffer.begin_user_action();
        match buffer.iter_at_line(target_line).filter(|_| target_line < buffer.line_count()) {
            Some(mut at) => buffer.insert(&mut at, &format!("{line}\n")),
            None => {
                // Below the last line: a line of its own at the end.
                let mut end = buffer.end_iter();
                let lead = if end.starts_line() { "" } else { "\n" };
                buffer.insert(&mut end, &format!("{lead}{line}"));
            }
        }
        if !Rc::ptr_eq(&from, &doc) {
            buffer.end_user_action();
            from.buffer.begin_user_action();
        }
        remove_line(&from, from.buffer.iter_at_mark(&mark).line(), false);
        from.buffer.end_user_action();
        end_drag();
        true
    });
    target
}

fn line_text(buffer: &sourceview5::Buffer, line: i32) -> String {
    let Some(start) = buffer.iter_at_line(line) else { return String::new() };
    let mut end = start;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    buffer.text(&start, &end, true).to_string()
}

/// Delete line `line` with its line break, as one undo step when `step` (otherwise inside
/// the caller's).
fn remove_line(doc: &Doc, line: i32, step: bool) {
    let buffer = &doc.buffer;
    let Some(mut start) = buffer.iter_at_line(line) else { return };
    let mut end = start;
    if !end.forward_line() {
        end = buffer.end_iter();
        // The last line: take the break before it instead.
        if start.line() > 0 {
            start.backward_char();
        }
    }
    if step {
        buffer.begin_user_action();
    }
    buffer.delete(&mut start, &mut end);
    if step {
        buffer.end_user_action();
    }
}

fn image_menu(doc: &Rc<Doc>, shown: &Rc<Shown>, x: f64, y: f64) {
    let actions = gio::SimpleActionGroup::new();
    let path = shown.path.borrow().clone();
    let add = |name: &str, f: Box<dyn Fn()>| {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| f());
        actions.add_action(&action);
    };
    let (weak, file, reference) = (Rc::downgrade(doc), path.clone(), reference_of(doc, shown));
    add(
        "copy",
        Box::new(move || {
            if let Some(doc) = weak.upgrade() {
                copy_images(&doc.view.clipboard(), reference.as_deref(), std::slice::from_ref(&file));
                doc.flash("Image copied");
            }
        }),
    );
    let (weak, file) = (Rc::downgrade(doc), path.clone());
    add(
        "copy-path",
        Box::new(move || {
            if let Some(doc) = weak.upgrade() {
                doc.view.clipboard().set_text(&file.to_string_lossy());
                doc.flash("Path copied");
            }
        }),
    );
    let (weak, file) = (Rc::downgrade(doc), path.clone());
    add(
        "open",
        Box::new(move || {
            let window = weak.upgrade().and_then(|d| d.view.root()).and_downcast::<gtk::Window>();
            let failed = launch_failed(&weak, "Could not open the image");
            gtk::FileLauncher::new(Some(&gio::File::for_path(&file))).launch(window.as_ref(), gio::Cancellable::NONE, failed);
        }),
    );
    let (weak, file) = (Rc::downgrade(doc), path.clone());
    add(
        "folder",
        Box::new(move || {
            let window = weak.upgrade().and_then(|d| d.view.root()).and_downcast::<gtk::Window>();
            let failed = launch_failed(&weak, "Could not show the folder");
            gtk::FileLauncher::new(Some(&gio::File::for_path(&file)))
                .open_containing_folder(window.as_ref(), gio::Cancellable::NONE, failed);
        }),
    );
    let (weak, slot) = (Rc::downgrade(doc), Rc::downgrade(shown));
    add(
        "remove",
        Box::new(move || {
            let (Some(doc), Some(shown)) = (weak.upgrade(), slot.upgrade()) else { return };
            let mark = shown.mark.borrow().clone().filter(|m| !m.is_deleted());
            if let Some(mark) = mark {
                remove_line(&doc, doc.buffer.iter_at_mark(&mark).line(), true);
            }
        }),
    );
    let model = gio::Menu::new();
    let first = gio::Menu::new();
    first.append(Some("Copy image"), Some("image.copy"));
    first.append(Some("Copy file path"), Some("image.copy-path"));
    model.append_section(None, &first);
    let second = gio::Menu::new();
    second.append(Some("Open in image viewer"), Some("image.open"));
    second.append(Some("Show in folder"), Some("image.folder"));
    model.append_section(None, &second);
    let third = gio::Menu::new();
    third.append(Some("Remove from note"), Some("image.remove"));
    model.append_section(None, &third);
    let menu = gtk::PopoverMenu::from_model(Some(&model));
    menu.set_has_arrow(false);
    menu.set_halign(gtk::Align::Start);
    menu.set_parent(&shown.frame);
    menu.insert_action_group("image", Some(&actions));
    menu.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    menu.connect_closed(|menu| {
        let menu = menu.clone();
        // After the chosen item's action has run.
        glib::idle_add_local_once(move || menu.unparent());
    });
    menu.popup();
}

/// Put `paths` on the clipboard as files, as the picture itself when there is one, and as
/// `text` (or the paths) for anything that only takes text.
/// The picture of a single path is decoded at full size on a worker thread and joins the
/// copy when it is ready, if nothing was copied since.
pub fn copy_images(clipboard: &gtk::gdk::Clipboard, text: Option<&str>, paths: &[PathBuf]) {
    let fallback = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
    let text = text.unwrap_or(&fallback).to_string();
    let files: Vec<gio::File> = paths.iter().map(gio::File::for_path).collect();
    let content = move |picture: Option<gtk::gdk::Texture>| {
        let mut providers: Vec<gtk::gdk::ContentProvider> = picture.iter().map(|p| gtk::gdk::ContentProvider::for_value(&p.to_value())).collect();
        providers.push(gtk::gdk::ContentProvider::for_value(&gtk::gdk::FileList::from_array(&files).to_value()));
        providers.push(gtk::gdk::ContentProvider::for_value(&text.to_value()));
        gtk::gdk::ContentProvider::new_union(&providers)
    };
    let first = content(None);
    clipboard.set_content(Some(&first)).ok();
    let [only] = paths else { return };
    let (clipboard, file) = (clipboard.clone(), only.clone());
    glib::spawn_future_local(async move {
        let Ok(Some(picture)) = gio::spawn_blocking(move || gtk::gdk::Texture::from_filename(&file).ok()).await else { return };
        if clipboard.content().as_ref() == Some(&first) {
            clipboard.set_content(Some(&content(Some(picture)))).ok();
        }
    });
}

/// The image files a selection references, for Copy and Cut.
pub fn referenced(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(tx::image_ref)
        .map(|(_, path, ..)| PathBuf::from(path))
        .filter(|path| path.is_file())
        .collect()
}

// ---- adding images -----------------------------------------------------------------

fn images_dir(ui: &Rc<Ui>, project: i64) -> PathBuf {
    let root = ui
        .projects
        .borrow()
        .iter()
        .find(|p| p["id"].as_i64() == Some(project))
        .and_then(|p| p["path"].as_str().map(PathBuf::from))
        .filter(|path| path.is_dir());
    match root {
        Some(root) => root.join(".relay/notes/images"),
        None => glib::user_data_dir().join("relay-v4/note-images"),
    }
}

/// A new file name in `dir` for note `id`, ending in `extension`.
fn fresh_name(dir: &Path, id: i64, extension: &str) -> PathBuf {
    let stamp = glib::DateTime::now_local()
        .and_then(|now| now.format("%Y%m%d-%H%M%S"))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "image".into());
    let mut n = 0;
    loop {
        let suffix = if n == 0 { String::new() } else { format!("-{n}") };
        let path = dir.join(format!("note-{id}-{stamp}{suffix}.{extension}"));
        if !path.exists() && RESERVED.with(|reserved| reserved.borrow_mut().insert(path.clone())) {
            return path;
        }
        n += 1;
    }
}

/// The names `fresh_name` handed out, written now (or failed): free to be checked on disk.
fn release(paths: &[PathBuf]) {
    RESERVED.with(|reserved| {
        let mut reserved = reserved.borrow_mut();
        for path in paths {
            reserved.remove(path);
        }
    });
}

/// Where a paste or drop puts what it brings: the selection goes at once, as a text paste's
/// does, and the references go where it was when they are ready, wherever the cursor is then.
fn landing(doc: &Doc) -> gtk::TextMark {
    let buffer = &doc.buffer;
    buffer.delete_selection(true, true);
    buffer.create_mark(None, &buffer.iter_at_mark(&buffer.get_insert()), false)
}

/// A picture or file write for `doc` started (`true`) or ended. A tab asked to close while
/// one ran closes when the last ends.
fn writing(ui: &Rc<Ui>, doc: &Rc<Doc>, started: bool) {
    let count = &doc.inline.writing;
    count.set(if started { count.get() + 1 } else { count.get().saturating_sub(1) });
    if !started && count.get() == 0 && doc.close_pending.get() && !doc.draft.busy.get() {
        doc.close_pending.set(false);
        super::doc::request_close(ui, doc);
    }
}

/// Insert references to `paths` at `at`, each on a line of its own.
fn insert_refs(doc: &Doc, paths: &[PathBuf], at: &gtk::TextMark) {
    let buffer = &doc.buffer;
    let mut number = tx::next_image_number(&doc.body());
    let lines: Vec<String> = paths
        .iter()
        .map(|path| {
            let line = tx::image_markdown(&format!("Image {number}"), &path.to_string_lossy());
            number += 1;
            line
        })
        .collect();
    let mut iter = buffer.iter_at_mark(at);
    let lead = if iter.starts_line() { "" } else { "\n" };
    let tail = if iter.ends_line() { "\n" } else { "\n\n" };
    buffer.begin_user_action();
    buffer.insert(&mut iter, &format!("{lead}{}{tail}", lines.join("\n")));
    buffer.end_user_action();
    // The note the person is in shows it; one in a background tab is not brought forward.
    if doc.view.has_focus() {
        doc.view.scroll_to_mark(at, 0.1, false, 0., 0.);
    }
    buffer.delete_mark(at);
}

/// What a launch that failed says in the note's status bar; one dismissed (an app chooser
/// closed) says nothing.
fn launch_failed(doc: &std::rc::Weak<Doc>, what: &'static str) -> impl FnOnce(Result<(), glib::Error>) + 'static {
    let doc = doc.clone();
    move |result| match (result, doc.upgrade()) {
        (Err(error), Some(doc)) if !error.matches(gtk::DialogError::Dismissed) => doc.flash(&format!("{what}: {}", error.message())),
        _ => {}
    }
}

/// Keep a pasted or dropped picture as a PNG beside the note and reference it. Encoding a
/// 4K screenshot takes a few hundred milliseconds: it is written on a worker thread, and the
/// reference goes in at the cursor when it is on disk.
fn store_texture(ui: &Rc<Ui>, doc: &Rc<Doc>, picture: &gtk::gdk::Texture) {
    let dir = images_dir(ui, doc.project);
    let path = fresh_name(&dir, doc.id, "png");
    let at = landing(doc);
    writing(ui, doc, true);
    let (ui, doc, picture, target) = (ui.clone(), doc.clone(), picture.clone(), path.clone());
    glib::spawn_future_local(async move {
        let saved = gio::spawn_blocking(move || {
            std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|_| picture.save_to_png(&target).map_err(|e| e.to_string()))
        })
        .await
        .unwrap_or_else(|_| Err("the image could not be written".into()));
        release(std::slice::from_ref(&path));
        match saved {
            Ok(()) => insert_refs(&doc, &[path], &at),
            Err(error) => {
                doc.buffer.delete_mark(&at);
                doc.show_notice(&ui, &format!("Could not keep the image: {error}"), Vec::new());
            }
        }
        writing(&ui, &doc, false);
    });
}

/// Copy image files into the note's folder and reference the copies; other files are
/// referenced where they are, as links. The copies are made on a worker thread.
fn store_files(ui: &Rc<Ui>, doc: &Rc<Doc>, files: &[PathBuf]) {
    let dir = images_dir(ui, doc.project);
    let copies: Vec<(PathBuf, PathBuf)> = files
        .iter()
        .filter(|file| is_image(&file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()))
        .map(|file| {
            let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            (file.clone(), fresh_name(&dir, doc.id, &extension))
        })
        .collect();
    let others: Vec<String> = files
        .iter()
        .filter(|f| !is_image(&f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()))
        .map(|f| f.to_string_lossy().into_owned())
        .collect();
    let at = landing(doc);
    writing(ui, doc, true);
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        let names: Vec<PathBuf> = copies.iter().map(|(_, copy)| copy.clone()).collect();
        let made = gio::spawn_blocking(move || {
            let mut kept = Vec::new();
            for (file, copy) in copies {
                match std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(&file, &copy)) {
                    Ok(_) => kept.push(copy),
                    Err(error) => return Err(format!("Could not keep {}: {error}", file.display())),
                }
            }
            Ok(kept)
        })
        .await
        .unwrap_or_else(|_| Err("Could not keep the images".into()));
        release(&names);
        match made {
            Ok(kept) if !kept.is_empty() => insert_refs(&doc, &kept, &at),
            Ok(_) => doc.buffer.delete_mark(&at),
            Err(error) => {
                doc.buffer.delete_mark(&at);
                doc.show_notice(&ui, &error, Vec::new());
            }
        }
        if !others.is_empty() {
            doc.insert_text(&others.join("\n"));
        }
        writing(&ui, &doc, false);
    });
}

fn is_image(extension: &str) -> bool {
    crate::editor::image_preview::image_extension(extension)
}

/// Edit > Insert image… and the toolbar's Image key.
pub fn choose_image(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Images"));
    filter.add_mime_type("image/*");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title("Insert image")
        .modal(true)
        .filters(&filters)
        .default_filter(&filter)
        .build();
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        let window = super::window_of(&ui);
        let Ok(chosen) = dialog.open_multiple_future(window.as_ref()).await else { return };
        let paths: Vec<PathBuf> = (0..chosen.n_items())
            .filter_map(|i| chosen.item(i).and_downcast::<gio::File>())
            .filter_map(|f| f.path())
            .collect();
        store_files(&ui, &doc, &paths);
    });
}

/// Paste an image or image files from the clipboard. Returns whether it took the paste;
/// text (and anything this window copied itself) pastes as text.
pub fn paste(ui: &Rc<Ui>, doc: &Rc<Doc>) -> bool {
    let clipboard = doc.view.clipboard();
    if clipboard.is_local() {
        return false;
    }
    let formats = clipboard.formats();
    let files = formats.contains_type(gtk::gdk::FileList::static_type());
    let picture = formats.contains_type(gtk::gdk::Texture::static_type());
    let text = formats.contains_type(glib::GString::static_type()) || formats.contain_mime_type("text/plain");
    if files {
        let (ui, doc, clipboard) = (ui.clone(), doc.clone(), clipboard.clone());
        glib::spawn_future_local(async move {
            let Ok(value) = clipboard.read_value_future(gtk::gdk::FileList::static_type(), glib::Priority::DEFAULT).await else {
                return;
            };
            let Ok(list) = value.get::<gtk::gdk::FileList>() else { return };
            let paths: Vec<PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
            let images = paths
                .iter()
                .any(|p| is_image(&p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()));
            if images {
                store_files(&ui, &doc, &paths);
            } else if let Ok(Some(text)) = clipboard.read_text_future().await {
                doc.insert_text(&text);
            }
        });
        return true;
    }
    if picture && !text {
        let (ui, doc, clipboard) = (ui.clone(), doc.clone(), clipboard.clone());
        glib::spawn_future_local(async move {
            match clipboard.read_texture_future().await {
                Ok(Some(picture)) => store_texture(&ui, &doc, &picture),
                Ok(None) => {}
                Err(error) => doc.show_notice(&ui, &format!("Could not paste the image: {error}"), Vec::new()),
            }
        });
        return true;
    }
    false
}

/// Files or a picture dropped on the editor.
pub fn drop_target(ui: &Rc<Ui>, doc: &Rc<Doc>) -> gtk::DropTarget {
    let target = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
    target.set_types(&[gtk::gdk::FileList::static_type(), gtk::gdk::Texture::static_type()]);
    let (weak_ui, weak) = (Rc::downgrade(ui), Rc::downgrade(doc));
    target.connect_drop(move |_, value, x, y| {
        let (Some(ui), Some(doc)) = (weak_ui.upgrade(), weak.upgrade()) else { return false };
        let (bx, by) = doc.view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        if let Some(at) = doc.view.iter_at_location(bx, by) {
            doc.buffer.place_cursor(&at);
        }
        if let Ok(list) = value.get::<gtk::gdk::FileList>() {
            let paths: Vec<PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
            store_files(&ui, &doc, &paths);
            return true;
        }
        if let Ok(picture) = value.get::<gtk::gdk::Texture>() {
            store_texture(&ui, &doc, &picture);
            return true;
        }
        false
    });
    target
}
