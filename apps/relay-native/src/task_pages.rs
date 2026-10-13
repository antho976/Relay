use super::*;
use crate::app::scrolled;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

/// The task vocabulary, written once: the bus's column, type, priority and size values in the
/// order a picker lists them. The board and the task pages derive every other order from these.
pub use relay_board::COLUMN_TITLES;
pub const COLUMNS: &[&str] = &{
    let mut names = [""; COLUMN_TITLES.len()];
    let mut i = 0;
    while i < names.len() {
        names[i] = COLUMN_TITLES[i].0;
        i += 1;
    }
    names
};
/// Every column but Done, the last, which only approval reaches.
pub const OPEN_COLUMNS: &[&str] = COLUMNS.split_at(COLUMNS.len() - 1).0;
pub const TYPES: [&str; 5] = ["task", "feature", "bug", "chore", "spike"];
/// Least urgent first; `""` first in sizes. One vocabulary, shared with the board's ordering.
pub use relay_board::{PRIORITIES, SIZES};
pub fn chosen(control: &gtk::ComboBoxText) -> String {
    control
        .active_id()
        .map(|s| s.to_string())
        .unwrap_or_default()
}
pub fn buffer_text(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}
pub fn multiline(value: &str, height: i32) -> gtk::TextView {
    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_size_request(-1, height);
    view.buffer().set_text(value);
    view
}

// Editor-local drafts stay alive while project lists refresh. A close request cannot
// drop a changed draft, and controls are locked while its save is in flight.
pub struct Draft {
    pub window: Option<gtk::Window>,
    panel: Option<Rc<crate::panel::Panel>>,
    pub layout: gtk::Box,
    pub form: gtk::Box,
    pub status: gtk::Label,
    pub footer: gtk::Box,
    pub base: Rc<RefCell<Value>>,
    pub busy: Rc<Cell<bool>>,
    unsent_message: Cell<bool>,
    pub snapshot: Rc<dyn Fn() -> Value>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
}
impl Draft {
    pub fn new_note(
        ui: &Rc<Ui>,
        title: &str,
        base: Value,
        snapshot: Rc<dyn Fn() -> Value>,
        form: gtk::Box,
    ) -> Rc<Self> {
        Self::build(ui, title, base, snapshot, form, true)
    }
    fn build(
        ui: &Rc<Ui>,
        title: &str,
        base: Value,
        snapshot: Rc<dyn Fn() -> Value>,
        form: gtk::Box,
        note: bool,
    ) -> Rc<Self> {
        let window: Option<gtk::Window> = None;
        let panel = (!note).then(|| crate::panel::Panel::page(ui, title));
        form.set_valign(gtk::Align::Start);
        let layout = gtk::Box::new(gtk::Orientation::Vertical, 10);
        layout.set_margin_top(16);
        layout.set_margin_bottom(16);
        layout.set_margin_start(16);
        layout.set_margin_end(16);
        let status = paragraph("");
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        layout.append(&scrolled(&form));
        layout.append(&status);
        layout.append(&footer);
        if let Some(window) = &window {
            window.set_child(Some(&layout));
        }
        if let Some(panel) = &panel {
            panel.body.append(&layout);
        }
        let draft = Rc::new(Self {
            window,
            panel,
            layout,
            form,
            status,
            footer,
            base: Rc::new(RefCell::new(base)),
            busy: Rc::new(Cell::new(false)),
            unsent_message: Cell::new(false),
            snapshot,
            on_close: RefCell::new(None),
        });
        if let Some(window) = &draft.window {
            let weak = Rc::downgrade(&draft);
            window.connect_close_request(move |_| {
                if let Some(d) = weak.upgrade() {
                    if !d.can_close() {
                        return glib::Propagation::Stop;
                    }
                    d.cleanup();
                }
                glib::Propagation::Proceed
            });
        }
        if let Some(panel) = &draft.panel {
            let weak = Rc::downgrade(&draft);
            panel.set_guard(move || weak.upgrade().is_none_or(|d| d.can_close()));
            let weak = Rc::downgrade(&draft);
            panel.on_closed(move || {
                if let Some(d) = weak.upgrade() {
                    d.cleanup();
                }
            });
        }
        draft
    }
    fn can_close(&self) -> bool {
        if self.unsent_message.get() {
            self.status
                .set_text("Send or clear your message before closing.");
            return false;
        }
        if self.busy.get() || self.dirty() {
            self.status
                .set_text("Save your changes or choose Discard and close.");
            false
        } else {
            true
        }
    }
    fn cleanup(&self) {
        if let Some(close) = self.on_close.borrow_mut().take() {
            close();
        }
        clear(&self.footer);
        clear(&self.form);
    }
    pub fn close(&self) {
        if !self.can_close() {
            return;
        }
        if let Some(panel) = &self.panel {
            panel.close();
        }
        if let Some(window) = &self.window {
            self.cleanup();
            window.destroy();
        } else if self.panel.is_none() {
            self.cleanup();
        }
    }
    pub fn dirty(&self) -> bool {
        let current = (self.snapshot)();
        current
            .as_object()
            .is_some_and(|m| m.iter().any(|(k, v)| self.base.borrow()[k] != *v))
    }
}

use super::board_view::{caption, hue, label_chip, priority_icon, since, state_badge, status_icon, titled, wrapped};
use relay_board::{activity_sentence, actor_name, markdown_markup, URGENT_FIRST};

// ── Pieces the issue-style pages share ────────────────────────────────────────────────────

/// A Markdown field the way GitHub writes one: Write and Preview tabs, a formatting bar that
/// wraps the selection, and the text under them.
pub struct MarkdownField {
    pub root: gtk::Box,
    pub view: gtk::TextView,
    write: gtk::ToggleButton,
}
impl MarkdownField {
    pub fn new(initial: &str, placeholder: &str, height: i32) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("md-field");
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        bar.add_css_class("md-bar");
        let write = gtk::ToggleButton::with_label("Write");
        let preview = gtk::ToggleButton::with_label("Preview");
        preview.set_group(Some(&write));
        write.set_active(true);
        for tab in [&write, &preview] {
            tab.add_css_class("md-tab");
            bar.append(tab);
        }
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tools.add_css_class("md-tools");
        tools.set_hexpand(true);
        tools.set_halign(gtk::Align::End);
        bar.append(&tools);
        let view = multiline(initial, height);
        view.add_css_class("md-input");
        view.set_top_margin(9);
        view.set_bottom_margin(9);
        view.set_left_margin(11);
        view.set_right_margin(11);
        // A text view has no placeholder of its own: a hint sits over it while it is empty.
        let input = gtk::Overlay::new();
        input.set_child(Some(&view));
        let hint = label(placeholder, "md-placeholder");
        hint.set_halign(gtk::Align::Start);
        hint.set_valign(gtk::Align::Start);
        hint.set_can_target(false);
        hint.set_visible(initial.is_empty());
        input.add_overlay(&hint);
        let shown = hint.downgrade();
        view.buffer().connect_changed(move |buffer| {
            if let Some(hint) = shown.upgrade() {
                hint.set_visible(buffer.char_count() == 0);
            }
        });
        let rendered = label("", "md-preview");
        rendered.set_wrap(true);
        rendered.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        rendered.set_selectable(true);
        rendered.set_valign(gtk::Align::Start);
        rendered.set_size_request(-1, height);
        let stack = gtk::Stack::new();
        stack.set_vhomogeneous(false);
        stack.add_named(&input, Some("write"));
        stack.add_named(&rendered, Some("preview"));
        let (source, pane, keys) = (view.downgrade(), stack.downgrade(), tools.downgrade());
        preview.connect_toggled(move |preview| {
            let (Some(view), Some(stack), Some(tools)) = (source.upgrade(), pane.upgrade(), keys.upgrade()) else { return };
            if preview.is_active() {
                let text = buffer_text(&view.buffer());
                if text.trim().is_empty() {
                    rendered.set_markup("<i>Nothing to preview</i>");
                } else {
                    rendered.set_markup(&markdown_markup(&text));
                }
                stack.set_visible_child_name("preview");
            } else {
                stack.set_visible_child_name("write");
                view.grab_focus();
            }
            tools.set_sensitive(!preview.is_active());
        });
        for (caption, tip, before, after, whole_line) in [
            ("H", "Heading", "### ", "", true),
            ("B", "Bold", "**", "**", false),
            ("I", "Italic", "*", "*", false),
            ("❝", "Quote", "> ", "", true),
            ("<>", "Code", "`", "`", false),
            ("🔗", "Link", "[", "](url)", false),
            ("•", "Bulleted list", "- ", "", true),
            ("1.", "Numbered list", "1. ", "", true),
            ("☐", "Task list", "- [ ] ", "", true),
        ] {
            let key = button(caption, "md-tool");
            key.set_tooltip_text(Some(tip));
            key.set_focusable(false);
            let view = view.downgrade();
            key.connect_clicked(move |_| {
                if let Some(view) = view.upgrade() {
                    wrap_selection(&view, before, after, whole_line);
                }
            });
            tools.append(&key);
        }
        root.append(&bar);
        root.append(&stack);
        Self { root, view, write }
    }
    pub fn text(&self) -> String {
        buffer_text(&self.view.buffer())
    }
    pub fn clear(&self) {
        self.view.buffer().set_text("");
        self.write.set_active(true);
    }
}

/// The formatting bar's keys: wrap the selection (or the cursor) in `before`…`after`, or with
/// `whole_line` prefix every selected line.
fn wrap_selection(view: &gtk::TextView, before: &str, after: &str, whole_line: bool) {
    let buffer = view.buffer();
    let (start, end) = buffer.selection_bounds().unwrap_or_else(|| {
        let at = buffer.iter_at_mark(&buffer.get_insert());
        (at, at)
    });
    buffer.begin_user_action();
    if whole_line {
        for line in (start.line()..=end.line()).rev() {
            if let Some(mut at) = buffer.iter_at_line(line) {
                buffer.insert(&mut at, before);
            }
        }
    } else {
        let (from, to) = (start.offset(), end.offset());
        // The later point first, so the earlier offset still holds.
        buffer.insert(&mut buffer.iter_at_offset(to), after);
        buffer.insert(&mut buffer.iter_at_offset(from), before);
        let shift = before.chars().count() as i32;
        buffer.select_range(&buffer.iter_at_offset(from + shift), &buffer.iter_at_offset(to + shift));
    }
    buffer.end_user_action();
    view.grab_focus();
}

/// A round initial in the name's hue: who wrote a comment or did a step.
fn avatar(name: &str) -> gtk::Label {
    let initial: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
    let face = label(&initial, "avatar");
    face.add_css_class(&format!("avatar-hue-{}", hue(name)));
    face.set_xalign(0.5);
    face.set_valign(gtk::Align::Start);
    face.set_tooltip_text(Some(name));
    face
}

/// A choice's icon, drawn fresh for each place it shows.
type Icon = fn(&str) -> Option<gtk::Widget>;
/// Each pill's value and key, so a pick can light it and put out the rest.
type PillKeys = Rc<RefCell<Vec<(String, glib::WeakRef<gtk::Button>)>>>;
/// Files waiting to go up with a new task: name, MIME type and bytes.
type Queued = Rc<RefCell<Vec<(String, String, Vec<u8>)>>>;
fn column_icon(c: &str) -> Option<gtk::Widget> {
    Some(status_icon(c, 12).upcast())
}
fn priority_mark(p: &str) -> Option<gtk::Widget> {
    Some(priority_icon(p).upcast())
}
fn type_icon(t: &str) -> Option<gtk::Widget> {
    Some(super::task_mark(t, 9).upcast())
}
fn no_icon(_: &str) -> Option<gtk::Widget> {
    None
}

/// A field's choices laid out as pills, the current one lit: one click picks, nothing pops
/// up. `choices` are (value, caption); rows wrap at the sidebar's width.
fn pills(choices: Vec<(String, String)>, current: &str, icon: Icon, pick: impl Fn(&str) + 'static) -> gtk::Box {
    let keys: PillKeys = Rc::default();
    let pick = Rc::new(pick);
    let mut items = Vec::new();
    for (value, caption) in choices {
        let key = button("", "choice-pill");
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let mark = icon(&value);
        let wide = caption.chars().count() + if mark.is_some() { 5 } else { 3 };
        if let Some(mark) = mark {
            content.append(&mark);
        }
        let name = label(&caption, "");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(18);
        content.append(&name);
        key.set_child(Some(&content));
        key.set_tooltip_text(Some(&caption));
        if value == current {
            key.add_css_class("selected");
        }
        keys.borrow_mut().push((value.clone(), key.downgrade()));
        let (keys, pick) = (keys.clone(), pick.clone());
        key.connect_clicked(move |_| {
            for (other, key) in keys.borrow().iter() {
                if let Some(key) = key.upgrade() {
                    if *other == value {
                        key.add_css_class("selected");
                    } else {
                        key.remove_css_class("selected");
                    }
                }
            }
            pick(&value);
        });
        items.push((key.upcast::<gtk::Widget>(), wide));
    }
    let rows = wrapped(items, 34);
    rows.add_css_class("choice-pills");
    rows
}
fn titled_choices(list: &[&str], none: &str) -> Vec<(String, String)> {
    list.iter().map(|v| (v.to_string(), if v.is_empty() { none.to_string() } else { titled(v) })).collect()
}
fn column_choices(list: &[&str]) -> Vec<(String, String)> {
    list.iter().map(|c| (c.to_string(), caption(c).to_string())).collect()
}
fn module_choices(modules: &[Value], current: Option<i64>) -> Vec<(String, String)> {
    let mut names = vec![(String::new(), "None".to_string())];
    for m in modules {
        // A completed module is offered only to the task that already has it.
        if !m["completed_at"].is_null() && m["id"].as_i64() != current {
            continue;
        }
        names.push((m["id"].to_string(), text(m, "name").to_string()));
    }
    if let Some(module) = current.filter(|m| !names.iter().any(|(id, _)| *id == m.to_string())) {
        names.push((module.to_string(), format!("Module #{module}")));
    }
    names
}

/// A sidebar section: a quiet heading over its content.
fn side_section(title: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 7);
    section.add_css_class("issue-side-section");
    if !title.is_empty() {
        section.append(&label(title, "issue-side-title"));
    }
    section
}

/// The page's width: a centred column with the sidebar beside it, or under it when narrow.
/// GitHub keeps an issue to a readable measure instead of stretching it across the window.
fn issue_columns(ui: &Rc<Ui>, panel: &Rc<crate::panel::Panel>, page: &gtk::Box, main: &gtk::Box, side: &gtk::Box) -> gtk::Box {
    const SIDE: i32 = 264;
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 32);
    columns.add_css_class("issue-columns");
    main.set_hexpand(false);
    main.set_valign(gtk::Align::Start);
    side.set_hexpand(false);
    side.set_valign(gtk::Align::Start);
    side.add_css_class("issue-sidebar");
    columns.append(main);
    columns.append(side);
    page.set_halign(gtk::Align::Center);
    // The page panel spans the window less its sidebar; size the column to what is left.
    let fit = {
        let (columns, main, side) = (columns.downgrade(), main.downgrade(), side.downgrade());
        move |width: i32| {
            let (Some(columns), Some(main), Some(side)) = (columns.upgrade(), main.upgrade(), side.upgrade()) else { return };
            let room = (width - 260).max(360);
            if room >= 940 {
                columns.set_orientation(gtk::Orientation::Horizontal);
                main.set_size_request((room - SIDE - 32 - 64).clamp(520, 820), -1);
                side.set_size_request(SIDE, -1);
            } else {
                columns.set_orientation(gtk::Orientation::Vertical);
                main.set_size_request((room - 48).min(820), -1);
                side.set_size_request((room - 48).min(820), -1);
            }
        }
    };
    fit(ui.window.width());
    if let Some(surface) = ui.window.surface() {
        let handler = surface.connect_layout(move |_, width, _| fit(width));
        let handler = RefCell::new(Some(handler));
        panel.on_closed(move || {
            if let Some(handler) = handler.borrow_mut().take() {
                surface.disconnect(handler);
            }
        });
    }
    columns
}
fn project_name(ui: &Ui, project: i64) -> String {
    ui.projects.borrow().iter().find(|p| p["id"].as_i64() == Some(project)).map(|p| text(p, "name").to_string()).unwrap_or_default()
}

// ── Attachments: dropped or pasted, never typed ───────────────────────────────────────────

/// A file or image arriving by drop or paste.
enum Incoming {
    Path(String),
    Image { name: String, bytes: Vec<u8> },
}
fn mime_of(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt" | "md" | "log") => "text/plain",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
fn texture_png(texture: &gtk::gdk::Texture) -> Vec<u8> {
    texture.save_to_png_bytes().to_vec()
}
/// Files and images dropped on `widget`, or pasted with Ctrl+V anywhere in it, go to `take`.
/// A paste of plain text is left to the text field it lands in.
fn accept_attachments(widget: &impl IsA<gtk::Widget>, take: impl Fn(Vec<Incoming>) + 'static) {
    use gtk::gdk;
    let take = Rc::new(take);
    let drop = gtk::DropTarget::new(glib::Type::INVALID, gdk::DragAction::COPY);
    drop.set_types(&[gdk::FileList::static_type(), gdk::Texture::static_type()]);
    let target = widget.as_ref().downgrade();
    drop.connect_enter(move |_, _, _| {
        if let Some(w) = target.upgrade() {
            w.add_css_class("drop-attach");
        }
        gdk::DragAction::COPY
    });
    let target = widget.as_ref().downgrade();
    drop.connect_leave(move |_| {
        if let Some(w) = target.upgrade() {
            w.remove_css_class("drop-attach");
        }
    });
    let (sink, target) = (take.clone(), widget.as_ref().downgrade());
    drop.connect_drop(move |_, value, _, _| {
        if let Some(w) = target.upgrade() {
            w.remove_css_class("drop-attach");
        }
        if let Ok(files) = value.get::<gdk::FileList>() {
            let paths: Vec<Incoming> = files.files().iter().filter_map(|f| f.path()).map(|p| Incoming::Path(p.to_string_lossy().into_owned())).collect();
            if !paths.is_empty() {
                sink(paths);
                return true;
            }
        }
        if let Ok(texture) = value.get::<gdk::Texture>() {
            sink(vec![Incoming::Image { name: "dropped.png".into(), bytes: texture_png(&texture) }]);
            return true;
        }
        false
    });
    widget.add_controller(drop);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let source = widget.as_ref().downgrade();
    keys.connect_key_pressed(move |_, key, _, mods| {
        if !(matches!(key, gdk::Key::v | gdk::Key::V) && mods.contains(gdk::ModifierType::CONTROL_MASK)) {
            return glib::Propagation::Proceed;
        }
        let Some(widget) = source.upgrade() else { return glib::Propagation::Proceed };
        let clipboard = widget.clipboard();
        let formats = clipboard.formats();
        let sink = take.clone();
        if formats.contains_type(gdk::FileList::static_type()) && !formats.contain_mime_type("text/plain;charset=utf-8") {
            glib::spawn_future_local(async move {
                if let Ok(value) = clipboard.read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT).await {
                    if let Ok(files) = value.get::<gdk::FileList>() {
                        sink(files.files().iter().filter_map(|f| f.path()).map(|p| Incoming::Path(p.to_string_lossy().into_owned())).collect());
                    }
                }
            });
            return glib::Propagation::Stop;
        }
        if formats.contains_type(gdk::Texture::static_type()) {
            glib::spawn_future_local(async move {
                if let Ok(Some(texture)) = clipboard.read_texture_future().await {
                    sink(vec![Incoming::Image { name: "pasted.png".into(), bytes: texture_png(&texture) }]);
                }
            });
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    widget.add_controller(keys);
}
/// A thumbnail for an image (or a named chip for any other file), with a remove key.
fn attachment_tile(name: &str, mime: &str, image: Option<gtk::gdk::Texture>, path: Option<&str>, remove: impl Fn() + 'static) -> gtk::Overlay {
    let tile = gtk::Overlay::new();
    tile.add_css_class("attachment-tile");
    tile.set_tooltip_text(Some(name));
    // A gtk::Image scales its picture to one fixed size; a Picture asks for the image's own
    // width and would stretch the page to it.
    let face: gtk::Widget = match (mime.starts_with("image/"), image, path) {
        (true, Some(texture), _) => gtk::Image::from_paintable(Some(&texture)).upcast(),
        (true, None, Some(path)) => gtk::Image::from_file(path).upcast(),
        _ => {
            let chip = gtk::Box::new(gtk::Orientation::Vertical, 4);
            chip.set_valign(gtk::Align::Center);
            chip.append(&crate::icons::image("file", 18));
            let caption = label(name, "attachment-name");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            caption.set_max_width_chars(12);
            caption.set_xalign(0.5);
            chip.append(&caption);
            chip.upcast()
        }
    };
    if let Some(image) = face.downcast_ref::<gtk::Image>() {
        image.set_pixel_size(84);
    }
    face.set_size_request(104, 88);
    tile.set_halign(gtk::Align::Start);
    tile.set_child(Some(&face));
    let close = crate::app::icon_button("close", "Remove");
    close.add_css_class("attachment-remove");
    close.set_halign(gtk::Align::End);
    close.set_valign(gtk::Align::Start);
    close.connect_clicked(move |_| remove());
    tile.add_overlay(&close);
    tile
}

// ── New task ──────────────────────────────────────────────────────────────────────────────

/// GitHub's new-issue page: a title and a Markdown description, images dropped or pasted
/// onto it, and the task's fields as pills beside it. Create opens the new task (or, with
/// Create more, clears for the next). From a module's board it starts in that module.
pub fn compose(ui: &Rc<Ui>, project: i64, column: &str, module: Option<i64>) {
    let name = project_name(ui, project);
    let panel = crate::panel::Panel::page(ui, &if name.is_empty() { "New task".into() } else { format!("New task in {name}") });
    panel.add_css_class("issue-page");
    panel.hide_scrollbar();
    let page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    page.add_css_class("issue-compose");
    let main = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = label("", "issue-field-caption");
    heading.set_markup("Add a title <span foreground=\"#e5382e\">*</span>");
    main.append(&heading);
    let title = gtk::Entry::builder().placeholder_text("Title").build();
    title.set_widget_name("compose-title");
    title.add_css_class("issue-title-entry");
    main.append(&title);
    main.append(&label("Add a description", "issue-field-caption"));
    let description = MarkdownField::new("", "Type your description here…  Paste or drop images to attach them.", 220);
    description.view.set_widget_name("compose-body");
    main.append(&description.root);
    // Images and files waiting to go up with the task.
    let queued: Queued = Rc::default();
    let tray = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    tray.add_css_class("attachment-tray");
    tray.set_visible(false);
    main.append(&tray);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("issue-compose-footer");
    let more = gtk::CheckButton::with_label("Create more");
    more.set_tooltip_text(Some("Stay here after creating, ready for the next task"));
    footer.append(&more);
    let status = paragraph("");
    status.set_hexpand(true);
    status.set_xalign(1.);
    footer.append(&status);
    let cancel = button("Cancel", "quiet");
    footer.append(&cancel);
    let create = button("Create task", "primary");
    create.set_widget_name("compose-create");
    create.set_tooltip_text(Some("Create the task (Ctrl+Enter)"));
    footer.append(&create);
    main.append(&footer);

    // The sidebar's picks, by payload key.
    let picks: Rc<RefCell<BTreeMap<&'static str, String>>> = Rc::new(RefCell::new(BTreeMap::from([
        ("column", column.to_string()),
        ("type", "task".into()),
        ("priority", "medium".into()),
        ("size", String::new()),
        ("module_id", module.map(|m| m.to_string()).unwrap_or_default()),
    ])));
    let choose_into = |key: &'static str| {
        let picks = picks.clone();
        move |value: &str| {
            picks.borrow_mut().insert(key, value.to_string());
        }
    };
    let side = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for (title_text, control) in [
        ("Status", pills(column_choices(OPEN_COLUMNS), column, column_icon, choose_into("column"))),
        ("Priority", pills(titled_choices(&URGENT_FIRST, ""), "medium", priority_mark, choose_into("priority"))),
        ("Size · how much work", pills(titled_choices(&SIZES, "None"), "", no_icon, choose_into("size"))),
        ("Type", pills(titled_choices(&TYPES, ""), "task", type_icon, choose_into("type"))),
    ] {
        let section = side_section(title_text);
        section.append(&control);
        side.append(&section);
    }
    let modules = side_section("Module");
    let module_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    module_slot.append(&label("Loading…", "issue-side-empty"));
    modules.append(&module_slot);
    side.append(&modules);
    let tags = side_section("Labels");
    let labels = gtk::Entry::builder().placeholder_text("ui, perf").build();
    tags.append(&labels);
    side.append(&tags);

    page.append(&issue_columns(ui, &panel, &page, &main, &side));
    panel.body.append(&page);

    let refill = {
        let (tray, queued) = (tray.downgrade(), queued.clone());
        Rc::new(move || {
            let Some(tray) = tray.upgrade() else { return };
            clear(&tray);
            for (index, (name, mime, bytes)) in queued.borrow().iter().enumerate() {
                let image = mime.starts_with("image/").then(|| gtk::gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()).flatten();
                let (queued, tray_weak) = (queued.clone(), tray.downgrade());
                let tile = attachment_tile(name, mime, image, None, move || {
                    if index < queued.borrow().len() {
                        queued.borrow_mut().remove(index);
                    }
                    // Redrawn on the next turn: this key's own tile goes with it.
                    if let Some(tray) = tray_weak.upgrade() {
                        tray.activate_action("compose.refill", None).ok();
                    }
                });
                tray.append(&tile);
            }
            tray.set_visible(!queued.borrow().is_empty());
        })
    };
    let group = gtk::gio::SimpleActionGroup::new();
    let redraw = gtk::gio::SimpleAction::new("refill", None);
    let again = refill.clone();
    redraw.connect_activate(move |_, _| {
        let again = again.clone();
        glib::idle_add_local_once(move || again());
    });
    group.add_action(&redraw);
    page.insert_action_group("compose", Some(&group));
    let (into, again, note) = (queued.clone(), refill.clone(), status.downgrade());
    accept_attachments(&page, move |incoming| {
        for item in incoming {
            match item {
                Incoming::Path(path) => match std::fs::read(&path) {
                    Ok(bytes) => {
                        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "attachment".into());
                        let mime = mime_of(&name).to_string();
                        into.borrow_mut().push((name, mime, bytes));
                    }
                    Err(e) => {
                        if let Some(note) = note.upgrade() {
                            note.set_text(&format!("Could not read {path}: {e}"));
                        }
                    }
                },
                Incoming::Image { name, bytes } => into.borrow_mut().push((name, "image/png".into(), bytes)),
            }
        }
        again();
    });

    let weak = Rc::downgrade(ui);
    let slot = module_slot.downgrade();
    let module_pick = choose_into("module_id");
    glib::spawn_future_local(async move {
        let Some(ui) = weak.upgrade() else { return };
        let available = ui.call("module.list", json!({"project_id":project})).await;
        if let Some(slot) = slot.upgrade() {
            clear(&slot);
            let modules = available.map(|a| rows(&a, "modules")).unwrap_or_default();
            slot.append(&pills(module_choices(&modules, module), &module.map(|m| m.to_string()).unwrap_or_default(), no_icon, module_pick));
        }
    });

    let permit_close = Rc::new(Cell::new(false));
    let busy = Rc::new(Cell::new(false));
    let empty = {
        let (title, body, queued) = (title.downgrade(), description.view.buffer(), queued.clone());
        move || title.upgrade().is_none_or(|t| t.text().trim().is_empty()) && buffer_text(&body).trim().is_empty() && queued.borrow().is_empty()
    };
    let empty = Rc::new(empty);
    let (guard_status, permit, working, is_empty) = (status.downgrade(), permit_close.clone(), busy.clone(), empty.clone());
    panel.set_guard(move || {
        if working.get() {
            return false;
        }
        if permit.get() || is_empty() {
            return true;
        }
        if let Some(status) = guard_status.upgrade() {
            status.set_text("Create the task, or Cancel to discard it.");
        }
        false
    });
    let (p, permit, working, is_empty) = (Rc::downgrade(&panel), permit_close.clone(), busy.clone(), empty.clone());
    crate::app::confirm_inline_if(&cancel, "Discard draft", move || !is_empty(), move |_| {
        if !working.get() {
            permit.set(true);
            if let Some(p) = p.upgrade() {
                p.close();
            }
        }
    });
    let weak = Rc::downgrade(ui);
    let p = Rc::downgrade(&panel);
    let body = description.view.downgrade();
    let page_weak = page.downgrade();
    let title_key = title.clone();
    create.connect_clicked(move |_| {
        let (Some(ui), Some(body), Some(page)) = (weak.upgrade(), body.upgrade(), page_weak.upgrade()) else { return };
        let name = title_key.text().trim().to_string();
        if name.is_empty() {
            status.set_text("A task needs a title.");
            title_key.grab_focus();
            return;
        }
        if busy.replace(true) {
            return;
        }
        let picked = picks.borrow().clone();
        let pick = |key: &str| picked.get(key).cloned().unwrap_or_default();
        use base64::Engine;
        let attachments: Vec<Value> = queued.borrow().iter().map(|(name, mime, bytes)| json!({"name":name,"mime":mime,"bytes_b64":base64::engine::general_purpose::STANDARD.encode(bytes)})).collect();
        let payload = json!({"project_id": project, "title": name, "body": buffer_text(&body.buffer()), "type": pick("type"), "priority": pick("priority"), "size": if pick("size").is_empty() {Value::Null} else {json!(pick("size"))}, "column": pick("column"), "module_id": pick("module_id").parse::<i64>().ok(), "labels": labels.text().split(',').map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>(), "attachments": attachments});
        page.set_sensitive(false);
        status.set_text("Creating…");
        let (status, p, busy, permit, title, more, queued, refill) = (status.clone(), p.clone(), busy.clone(), permit_close.clone(), title_key.clone(), more.clone(), queued.clone(), refill.clone());
        glib::spawn_future_local(async move {
            let result = ui.call("task.create", payload).await;
            busy.set(false);
            page.set_sensitive(true);
            match result {
                Ok(task) if more.is_active() => {
                    title.set_text("");
                    body.buffer().set_text("");
                    queued.borrow_mut().clear();
                    refill();
                    status.set_text(&format!("Created #{}", task["id"]));
                    title.grab_focus();
                    ui.refresh_page();
                }
                Ok(task) => {
                    permit.set(true);
                    if let Some(p) = p.upgrade() {
                        p.close();
                    }
                    ui.refresh_page();
                    if let Some(id) = task["id"].as_i64() {
                        open(&ui, id);
                    }
                }
                Err(e) => status.set_text(&e.to_string()),
            }
        });
    });
    let submit = create.downgrade();
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, mods| {
        if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) && mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if let Some(submit) = submit.upgrade() {
                submit.emit_clicked();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    page.add_controller(keys);
    let submit = create.downgrade();
    title.connect_activate(move |_| {
        if let Some(submit) = submit.upgrade() {
            submit.emit_clicked();
        }
    });
    panel.present();
    title.grab_focus();
}

// ── New module ────────────────────────────────────────────────────────────────────────────

/// A module is a version bundle: a name, a priority and the tasks it ships. The page reads
/// like New task, and Create opens the module's own board.
pub fn compose_module(ui: &Rc<Ui>, project: i64) {
    let panel = crate::panel::Panel::page(ui, "New module");
    panel.add_css_class("issue-page");
    panel.hide_scrollbar();
    let page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    page.add_css_class("issue-compose");
    let main = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = label("", "issue-field-caption");
    heading.set_markup("Name <span foreground=\"#e5382e\">*</span>");
    main.append(&heading);
    let name = gtk::Entry::builder().placeholder_text("e.g. v1.4 — Gallery and profile").build();
    name.add_css_class("issue-title-entry");
    main.append(&name);
    main.append(&label("Tasks in this module", "issue-field-caption"));
    main.append(&label("Tick the tasks it ships. A task belongs to one module; ticking one in another moves it here.", "issue-side-empty"));
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("module-pick-list");
    list.append(&label("Loading…", "issue-side-empty"));
    main.append(&list);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("issue-compose-footer");
    let status = paragraph("");
    status.set_hexpand(true);
    status.set_xalign(1.);
    footer.append(&status);
    let cancel = button("Cancel", "quiet");
    footer.append(&cancel);
    let create = button("Create module", "primary");
    footer.append(&create);
    main.append(&footer);
    let priority = Rc::new(RefCell::new("medium".to_string()));
    let side = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let section = side_section("Priority");
    let chosen = priority.clone();
    section.append(&pills(titled_choices(&URGENT_FIRST, ""), "medium", priority_mark, move |p| *chosen.borrow_mut() = p.to_string()));
    side.append(&section);
    let about = side_section("About modules");
    let note = label("A module groups tasks into one release. It gets its own board with progress and patch notes, and its tasks still show on the main board with the module's name.", "issue-side-empty");
    note.set_wrap(true);
    note.set_max_width_chars(32);
    about.append(&note);
    side.append(&about);
    page.append(&issue_columns(ui, &panel, &page, &main, &side));
    panel.body.append(&page);

    let picked: Rc<RefCell<BTreeSet<i64>>> = Rc::default();
    let weak = Rc::downgrade(ui);
    let (rows_box, chosen) = (list.downgrade(), picked.clone());
    glib::spawn_future_local(async move {
        let Some(ui) = weak.upgrade() else { return };
        let tasks = ui.call("task.list", json!({"project_id":project,"summary":true})).await;
        let Some(list) = rows_box.upgrade() else { return };
        clear(&list);
        let tasks: Vec<Value> = tasks.map(|t| rows(&t, "tasks")).unwrap_or_default().into_iter().filter(|t| text(t, "column") != "done").collect();
        if tasks.is_empty() {
            list.append(&label("No open tasks yet: add them from the module's board.", "issue-side-empty"));
        }
        for task in tasks {
            let id = task["id"].as_i64().unwrap_or(0);
            let row = gtk::CheckButton::new();
            row.add_css_class("module-pick");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.append(&status_icon(text(&task, "column"), 12));
            let title = label(text(&task, "title"), "");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_hexpand(true);
            content.append(&title);
            if let Some(other) = task["module_name"].as_str() {
                content.append(&label(other, "module-card-meta"));
            }
            content.append(&label(&format!("#{id}"), "card-id"));
            row.set_child(Some(&content));
            let chosen = chosen.clone();
            row.connect_toggled(move |row| {
                if row.is_active() {
                    chosen.borrow_mut().insert(id);
                } else {
                    chosen.borrow_mut().remove(&id);
                }
            });
            list.append(&row);
        }
    });
    let p = Rc::downgrade(&panel);
    cancel.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.close();
        }
    });
    let (weak, p, field) = (Rc::downgrade(ui), Rc::downgrade(&panel), name.downgrade());
    create.connect_clicked(move |key| {
        let (Some(ui), Some(field)) = (weak.upgrade(), field.upgrade()) else { return };
        let value = field.text().trim().to_string();
        if value.is_empty() {
            status.set_text("A module needs a name.");
            field.grab_focus();
            return;
        }
        key.set_sensitive(false);
        status.set_text("Creating…");
        let (tasks, priority, p, status, key) = (picked.borrow().clone(), priority.borrow().clone(), p.clone(), status.clone(), key.clone());
        glib::spawn_future_local(async move {
            match ui.call("module.create", json!({"project_id":project,"name":value,"priority":priority})).await {
                Ok(module) => {
                    let id = module["id"].as_i64().unwrap_or(0);
                    for task in tasks {
                        if let Err(e) = ui.call("task.update", json!({"task_id":task,"module_id":id})).await {
                            ui.show_error(&e.to_string());
                        }
                    }
                    if let Some(p) = p.upgrade() {
                        p.close();
                    }
                    super::board_view::open_module(&ui, id);
                }
                Err(e) => {
                    status.set_text(&e.to_string());
                    key.set_sensitive(true);
                }
            }
        });
    });
    let submit = create.downgrade();
    name.connect_activate(move |_| {
        if let Some(submit) = submit.upgrade() {
            submit.emit_clicked();
        }
    });
    panel.present();
    name.grab_focus();
}

// ── A task, as an issue ───────────────────────────────────────────────────────────────────

thread_local! {
    /// Bumped by every open: only the newest open still in flight may present its page.
    static OPEN_SERIAL: Cell<u64> = const { Cell::new(0) };
    /// Task pages on screen, so opening one again shows it instead of stacking a second.
    static OPEN_DETAILS: RefCell<Vec<(i64, std::rc::Weak<Detail>)>> = const { RefCell::new(Vec::new()) };
}
fn open_detail(id: i64) -> Option<Rc<Detail>> {
    OPEN_DETAILS.with(|open| {
        open.borrow_mut().retain(|(_, d)| d.strong_count() > 0);
        open.borrow().iter().find(|(task, _)| *task == id).and_then(|(_, d)| d.upgrade())
    })
}

/// Everything a task page draws, fetched together.
struct Loaded {
    task: Value,
    tasks: Result<Vec<Value>, String>,
    modules: Vec<Value>,
    activity: Result<Value, String>,
}
async fn load(ui: &Rc<Ui>, id: i64, project: i64) -> Result<Loaded, String> {
    // The task almost always belongs to the project on screen, so its lists are asked for
    // alongside it; only a task from another project waits for a second pair. Completed
    // modules too: a task keeps its module after the module is completed.
    let (task, mut modules, mut tasks, activity) = tokio::join!(
        ui.call("task.get", json!({"task_id":id})),
        ui.call("module.list", json!({"project_id":project,"include_archived":true})),
        ui.call("task.list", json!({"project_id":project,"summary":true})),
        ui.call("task.activity", json!({"task_id":id,"limit":100}))
    );
    let task = task.map_err(|e| e.to_string())?;
    let owner = task["project_id"].as_i64().unwrap_or(0);
    if owner != project {
        (modules, tasks) = tokio::join!(
            ui.call("module.list", json!({"project_id":owner,"include_archived":true})),
            ui.call("task.list", json!({"project_id":owner,"summary":true}))
        );
    }
    let modules = modules.map_err(|e| format!("Could not load modules: {e}"))?;
    Ok(Loaded {
        task,
        tasks: tasks.map(|t| rows(&t, "tasks")).map_err(|e| e.to_string()),
        modules: rows(&modules, "modules"),
        activity: activity.map_err(|e| e.to_string()),
    })
}

pub fn open(ui: &Rc<Ui>, id: i64) {
    if let Some(d) = open_detail(id) {
        return d.panel.present();
    }
    let serial = OPEN_SERIAL.with(|s| { s.set(s.get() + 1); s.get() });
    let (project, page) = (ui.project.get(), ui.page.borrow().clone());
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let loaded = load(&ui, id, project).await;
        // A later open, or a move to another page or project, makes this one stale: presenting
        // it now would cover (and disable) whatever the user went on to.
        let current = OPEN_SERIAL.with(|s| s.get()) == serial;
        if !current || ui.project.get() != project || *ui.page.borrow() != page {
            return;
        }
        if let Some(d) = open_detail(id) {
            return d.panel.present();
        }
        match loaded {
            Ok(loaded) => Detail::show(&ui, id, loaded),
            Err(e) => ui.show_error(&e),
        }
    });
}

/// A task's page in GitHub's issue layout: title and status, the description, sub-tasks and a
/// timeline of everything that happened to it, a comment box, and a sidebar of its fields that
/// apply as they are picked.
pub struct Detail {
    ui: std::rc::Weak<Ui>,
    id: i64,
    project: Cell<i64>,
    panel: Rc<crate::panel::Panel>,
    header: gtk::Box,
    main: gtk::Box,
    side: gtk::Box,
    actions: gtk::Box,
    status: gtk::Label,
    comment: MarkdownField,
    recipient: gtk::ComboBoxText,
    task: RefCell<Value>,
    busy: Cell<bool>,
    /// An inline title or description editor holding text that differs from the task.
    editing: RefCell<Vec<Box<dyn Fn() -> bool>>>,
    /// Older history and messages fetched with "Load older", kept across redraws.
    history: RefCell<Vec<Value>>,
    messages: RefCell<Vec<Value>>,
    comments: RefCell<Vec<Value>>,
    cursors: Cell<(Option<i64>, Option<i64>)>,
}

impl Detail {
    fn show(ui: &Rc<Ui>, id: i64, loaded: Loaded) {
        let project = loaded.task["project_id"].as_i64().unwrap_or(0);
        let name = project_name(ui, project);
        let panel = crate::panel::Panel::page(ui, &if name.is_empty() { format!("Task #{id}") } else { format!("{name} #{id}") });
        panel.add_css_class("issue-page");
        panel.hide_scrollbar();
        let page = gtk::Box::new(gtk::Orientation::Vertical, 14);
        page.add_css_class("issue-detail");
        let header = gtk::Box::new(gtk::Orientation::Vertical, 8);
        header.add_css_class("issue-header");
        page.append(&header);
        let status = paragraph("");
        status.add_css_class("issue-status");
        status.set_visible(false);
        page.append(&status);
        let main = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        main.append(&content);
        let composer = gtk::Box::new(gtk::Orientation::Vertical, 8);
        composer.add_css_class("issue-composer");
        composer.append(&label("Add a comment", "issue-composer-title"));
        let comment = MarkdownField::new("", "Use Markdown to format your comment", 110);
        comment.view.set_widget_name("task-message");
        composer.append(&comment.root);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let recipient = gtk::ComboBoxText::new();
        recipient.set_tooltip_text(Some("Also send the comment to an agent's mailbox, linked to this task"));
        row.append(&recipient);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_hexpand(true);
        actions.set_halign(gtk::Align::End);
        row.append(&actions);
        composer.append(&row);
        main.append(&composer);
        let side = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.append(&issue_columns(ui, &panel, &page, &main, &side));
        panel.body.append(&page);
        let refresh = crate::app::icon_button("refresh", "Reload this task");
        panel.header_action(&refresh);

        let detail = Rc::new(Self {
            ui: Rc::downgrade(ui),
            id,
            project: Cell::new(project),
            panel: panel.clone(),
            header,
            main: content,
            side,
            actions,
            status,
            comment,
            recipient,
            task: RefCell::new(Value::Null),
            busy: Cell::new(false),
            editing: RefCell::new(Vec::new()),
            history: RefCell::new(Vec::new()),
            messages: RefCell::new(Vec::new()),
            comments: RefCell::new(Vec::new()),
            cursors: Cell::new((None, None)),
        });
        let weak = Rc::downgrade(&detail);
        refresh.connect_clicked(move |_| {
            if let Some(d) = weak.upgrade() {
                d.reload();
            }
        });
        let weak = Rc::downgrade(&detail);
        panel.set_guard(move || {
            let Some(d) = weak.upgrade() else { return true };
            if d.busy.get() {
                return false;
            }
            if !d.comment.text().trim().is_empty() {
                d.say("Post or clear your comment before leaving.");
                return false;
            }
            if d.editing.borrow().iter().any(|changed| changed()) {
                d.say("Save or cancel your edit before leaving.");
                return false;
            }
            true
        });
        OPEN_DETAILS.with(|open| open.borrow_mut().push((id, Rc::downgrade(&detail))));
        let weak = Rc::downgrade(&detail);
        panel.on_closed(move || OPEN_DETAILS.with(|open| open.borrow_mut().retain(|(_, d)| !d.ptr_eq(&weak))));
        // The page owns the Detail: its widgets hold only weak references back.
        let owner = detail.clone();
        panel.on_closed(move || {
            owner.editing.borrow_mut().clear();
        });
        unsafe { panel.body.set_data("relay-task-detail", detail.clone()) };
        let weak = Rc::downgrade(&detail);
        accept_attachments(&panel.body, move |incoming| {
            if let Some(d) = weak.upgrade() {
                d.attach(incoming);
            }
        });
        detail.render(loaded);
        panel.present();
    }

    /// Uploads what was dropped or pasted, then redraws.
    fn attach(self: &Rc<Self>, incoming: Vec<Incoming>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let detail = self.clone();
        let id = self.id;
        self.say("Attaching…");
        glib::spawn_future_local(async move {
            use base64::Engine;
            let mut failed = None;
            for item in incoming {
                let payload = match item {
                    Incoming::Path(path) => {
                        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "attachment".into());
                        json!({"task_id":id,"path":path,"name":name,"mime":mime_of(&name)})
                    }
                    Incoming::Image { name, bytes } => json!({"task_id":id,"name":name,"mime":"image/png","bytes_b64":base64::engine::general_purpose::STANDARD.encode(bytes)}),
                };
                if let Err(e) = ui.call("task.attach", payload).await {
                    failed = Some(e.to_string());
                }
            }
            detail.say(failed.as_deref().unwrap_or(""));
            detail.reload();
            ui.refresh_page();
        });
    }

    fn say(&self, message: &str) {
        self.status.set_text(message);
        self.status.set_visible(!message.is_empty());
    }

    fn reload(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let detail = self.clone();
        glib::spawn_future_local(async move {
            match load(&ui, detail.id, detail.project.get()).await {
                Ok(loaded) => detail.render(loaded),
                Err(e) => detail.say(&e),
            }
        });
    }

    /// Runs one task op from this page, then redraws it from the engine's state.
    fn act(self: &Rc<Self>, op: &'static str, payload: Value) {
        self.act_then(op, payload, |_| {});
    }
    fn act_then(self: &Rc<Self>, op: &'static str, payload: Value, after: impl FnOnce(&Rc<Self>) + 'static) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.busy.replace(true) {
            return;
        }
        self.say("Saving…");
        let detail = self.clone();
        glib::spawn_future_local(async move {
            let result = ui.call(op, payload).await;
            detail.busy.set(false);
            match result {
                Ok(_) => {
                    detail.say("");
                    after(&detail);
                    ui.refresh_page();
                }
                Err(crate::client::Error::Bus(e)) if e.code.ends_with(".edit_conflict") => {
                    detail.say("This task changed elsewhere, so the page reloaded with the latest version. Make the change again.");
                }
                Err(e) => {
                    detail.say(&e.to_string());
                    return;
                }
            }
            if detail.panel.body.root().is_some() {
                detail.reload();
            }
        });
    }
    /// `task.update` of one field, checked against the value this page shows.
    fn update(self: &Rc<Self>, field: &str, value: Value) {
        let before = self.task.borrow()[field].clone();
        if before == value {
            return;
        }
        let mut payload = json!({"task_id": self.id, "expected": {field: before}});
        payload[field] = value;
        self.act("task.update", payload);
    }

    fn render(self: &Rc<Self>, loaded: Loaded) {
        let Loaded { task, tasks, modules, activity } = loaded;
        *self.task.borrow_mut() = task.clone();
        self.editing.borrow_mut().clear();
        match &activity {
            Ok(data) => {
                *self.history.borrow_mut() = rows(data, "history");
                *self.messages.borrow_mut() = rows(data, "messages");
                *self.comments.borrow_mut() = rows(data, "comments");
                self.cursors.set((data["next_audit"].as_i64(), data["next_message"].as_i64()));
            }
            Err(e) => self.say(&format!("Could not load the timeline: {e}")),
        }
        let (tasks, tasks_error) = match tasks {
            Ok(tasks) => (tasks, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        if let Some(e) = &tasks_error {
            self.say(&format!("Could not load the project's tasks: {e}. Sub-task titles are unavailable."));
        }
        self.render_header(&task, &tasks);
        clear(&self.main);
        self.main.append(&self.description(&task));
        if let Some(children) = self.subtasks(&task, &tasks) {
            self.main.append(&children);
        }
        self.main.append(&self.timeline());
        self.render_actions(&task);
        self.render_side(&task, &modules);
    }

    fn render_header(self: &Rc<Self>, task: &Value, tasks: &[Value]) {
        clear(&self.header);
        let id = self.id;
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let title = label(text(task, "title"), "issue-title");
        title.set_wrap(true);
        title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_selectable(true);
        title.set_hexpand(false);
        line.append(&title);
        line.append(&label(&format!("#{id}"), "issue-number"));
        let edit = crate::app::icon_button("edit", "Edit the title");
        edit.set_widget_name("task-title-edit");
        edit.set_valign(gtk::Align::Center);
        line.append(&edit);
        self.header.append(&line);
        // The title editor: hidden until the pencil, as GitHub's is.
        let editor = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let entry = gtk::Entry::builder().text(text(task, "title")).hexpand(true).build();
        entry.set_widget_name("task-title");
        entry.add_css_class("issue-title-entry");
        editor.append(&entry);
        let save = button("Save", "primary");
        save.set_widget_name("draft-save");
        let cancel = button("Cancel", "quiet");
        editor.append(&save);
        editor.append(&cancel);
        editor.set_visible(false);
        self.header.append(&editor);
        let (shown, pane, field) = (line.downgrade(), editor.downgrade(), entry.downgrade());
        edit.connect_clicked(move |_| {
            if let (Some(line), Some(editor), Some(entry)) = (shown.upgrade(), pane.upgrade(), field.upgrade()) {
                line.set_visible(false);
                editor.set_visible(true);
                entry.grab_focus();
            }
        });
        let original = text(task, "title").to_string();
        let (shown, pane, field, was) = (line.downgrade(), editor.downgrade(), entry.downgrade(), original.clone());
        cancel.connect_clicked(move |_| {
            if let (Some(line), Some(editor), Some(entry)) = (shown.upgrade(), pane.upgrade(), field.upgrade()) {
                entry.set_text(&was);
                editor.set_visible(false);
                line.set_visible(true);
            }
        });
        let (weak, field) = (Rc::downgrade(self), entry.downgrade());
        save.connect_clicked(move |_| {
            let (Some(d), Some(entry)) = (weak.upgrade(), field.upgrade()) else { return };
            let next = entry.text().trim().to_string();
            if next.is_empty() {
                d.say("A task needs a title.");
                return;
            }
            d.update("title", json!(next));
        });
        let submit = save.downgrade();
        entry.connect_activate(move |_| {
            if let Some(save) = submit.upgrade() {
                save.emit_clicked();
            }
        });
        let field = entry.downgrade();
        self.editing.borrow_mut().push(Box::new(move || field.upgrade().is_some_and(|e| e.text().trim() != original)));

        let meta = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        meta.add_css_class("issue-meta");
        let column = text(task, "column");
        let pill = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        pill.add_css_class("status-pill");
        pill.add_css_class(&format!("status-pill-{column}"));
        pill.append(&status_icon(column, 13));
        pill.append(&label(caption(column), "status-pill-name"));
        meta.append(&pill);
        if let Some(badge) = state_badge(text(task, "state")) {
            meta.append(&badge);
        }
        if let Some(parent) = task["parent_id"].as_i64() {
            let title = tasks.iter().find(|t| t["id"].as_i64() == Some(parent)).map(|t| text(t, "title").to_string());
            let key = button("", "issue-pill");
            let caption = label(&format!("Parent: {}", title.unwrap_or_else(|| format!("#{parent}"))), "");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
            caption.set_max_width_chars(36);
            key.set_child(Some(&caption));
            let weak = self.ui.clone();
            key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    open(&ui, parent);
                }
            });
            meta.append(&key);
        }
        let opened = label(&format!("opened {} · updated {}", since(text(task, "created_at")), since(text(task, "updated_at"))), "issue-opened");
        opened.set_tooltip_text(Some(text(task, "created_at")));
        meta.append(&opened);
        self.header.append(&meta);
    }

    /// The first comment of the thread: the description, with its own Edit.
    fn description(self: &Rc<Self>, task: &Value) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("issue-comment");
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("issue-comment-head");
        let creator = self.history.borrow().iter().find(|row| text(row, "op") == "task.create").map(|row| actor_name(text(row, "actor")));
        let who = label("", "issue-comment-who");
        who.set_markup(&match &creator {
            Some(name) => format!("<b>{}</b> opened this {}", glib::markup_escape_text(name), glib::markup_escape_text(&since(text(task, "created_at")))),
            None => format!("Opened {}", glib::markup_escape_text(&since(text(task, "created_at")))),
        });
        who.set_hexpand(true);
        head.append(&who);
        let edit = button("Edit", "quiet");
        edit.set_widget_name("task-body-edit");
        head.append(&edit);
        card.append(&head);
        let body = text(task, "body").to_string();
        let shown = label("", "issue-comment-body");
        if body.trim().is_empty() {
            shown.set_markup("<i>No description provided.</i>");
            shown.add_css_class("dim");
        } else {
            shown.set_markup(&markdown_markup(&body));
        }
        shown.set_wrap(true);
        shown.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        shown.set_selectable(true);
        card.append(&shown);
        let editor = gtk::Box::new(gtk::Orientation::Vertical, 8);
        editor.add_css_class("issue-comment-body");
        let field = MarkdownField::new(&body, "Describe the outcome and the context an agent needs", 200);
        field.view.set_widget_name("task-body");
        editor.append(&field.root);
        let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        keys.set_halign(gtk::Align::End);
        let cancel = button("Cancel", "quiet");
        let save = button("Update description", "primary");
        keys.append(&cancel);
        keys.append(&save);
        editor.append(&keys);
        editor.set_visible(false);
        card.append(&editor);
        let files = rows(task, "attachments");
        let tray = gtk::FlowBox::new();
        tray.add_css_class("attachment-tray");
        tray.set_selection_mode(gtk::SelectionMode::None);
        tray.set_max_children_per_line(8);
        tray.set_column_spacing(8);
        tray.set_row_spacing(8);
        tray.set_halign(gtk::Align::Start);
        for file in &files {
            let attachment = file["id"].clone();
            let weak = Rc::downgrade(self);
            let tile = attachment_tile(text(file, "name"), text(file, "mime"), None, Some(text(file, "path")), move || {
                if let Some(d) = weak.upgrade() {
                    d.act("task.detach", json!({"task_id":d.id,"attachment_id":attachment}));
                }
            });
            tray.insert(&tile, -1);
        }
        if !files.is_empty() {
            card.append(&tray);
        }
        let hint = label("Paste or drop images and files anywhere on this page to attach them", "attachment-hint");
        card.append(&hint);
        let (display, pane, key) = (shown.downgrade(), editor.downgrade(), edit.downgrade());
        edit.connect_clicked(move |_| {
            if let (Some(display), Some(editor), Some(key)) = (display.upgrade(), pane.upgrade(), key.upgrade()) {
                display.set_visible(false);
                key.set_visible(false);
                editor.set_visible(true);
            }
        });
        let view = field.view.downgrade();
        let (display, pane, key, was) = (shown.downgrade(), editor.downgrade(), edit.downgrade(), body.clone());
        cancel.connect_clicked(move |_| {
            if let (Some(display), Some(editor), Some(key), Some(view)) = (display.upgrade(), pane.upgrade(), key.upgrade(), view.upgrade()) {
                view.buffer().set_text(&was);
                editor.set_visible(false);
                display.set_visible(true);
                key.set_visible(true);
            }
        });
        let (weak, view) = (Rc::downgrade(self), field.view.downgrade());
        save.connect_clicked(move |_| {
            if let (Some(d), Some(view)) = (weak.upgrade(), view.upgrade()) {
                d.update("body", json!(buffer_text(&view.buffer())));
            }
        });
        let view = field.view.downgrade();
        self.editing.borrow_mut().push(Box::new(move || view.upgrade().is_some_and(|v| buffer_text(&v.buffer()) != body)));
        card
    }

    fn subtasks(self: &Rc<Self>, task: &Value, tasks: &[Value]) -> Option<gtk::Box> {
        let children: Vec<i64> = rows(task, "children").iter().filter_map(Value::as_i64).collect();
        let can_add = task["depth"].as_i64().unwrap_or(0) < 2;
        if children.is_empty() && !can_add {
            return None;
        }
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("issue-comment");
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("issue-comment-head");
        head.append(&label("Sub-tasks", "issue-section-title"));
        // Without any yet, only a key that opens the field, as GitHub's Create sub-issue.
        let opener = (children.is_empty()).then(|| {
            let key = button("", "issue-close");
            key.set_child(Some(&icon_row("plus", "Create sub-task")));
            key.set_halign(gtk::Align::Start);
            key
        });
        let finished = children.iter().filter(|c| tasks.iter().any(|t| t["id"].as_i64() == Some(**c) && text(t, "column") == "done")).count();
        if !children.is_empty() {
            head.append(&label(&format!("{finished} / {}", children.len()), "lane-count"));
            let meter = gtk::ProgressBar::new();
            meter.set_fraction(finished as f64 / children.len() as f64);
            meter.set_valign(gtk::Align::Center);
            meter.set_hexpand(true);
            meter.add_css_class("issue-progress");
            head.append(&meter);
        }
        card.append(&head);
        for child in children {
            let row = button("", "issue-link");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            match tasks.iter().find(|t| t["id"].as_i64() == Some(child)) {
                Some(t) => {
                    content.append(&status_icon(text(t, "column"), 13));
                    let name = label(text(t, "title"), "");
                    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    name.set_hexpand(true);
                    content.append(&name);
                    content.append(&label(&format!("#{child}"), "card-id"));
                }
                None => content.append(&label(&format!("#{child}"), "")),
            }
            row.set_child(Some(&content));
            let weak = self.ui.clone();
            row.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    open(&ui, child);
                }
            });
            card.append(&row);
        }
        if can_add {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("issue-comment-body");
            let entry = gtk::Entry::builder().placeholder_text("Add a sub-task title").hexpand(true).build();
            let add = button("Create sub-task", "quiet");
            row.append(&entry);
            row.append(&add);
            let (weak, field) = (Rc::downgrade(self), entry.downgrade());
            let project = task["project_id"].clone();
            add.connect_clicked(move |_| {
                let (Some(d), Some(entry)) = (weak.upgrade(), field.upgrade()) else { return };
                let title = entry.text().trim().to_string();
                if !title.is_empty() {
                    d.act("task.create", json!({"project_id":project,"parent_id":d.id,"title":title,"column":"backlog"}));
                }
            });
            let submit = add.downgrade();
            entry.connect_activate(move |_| {
                if let Some(add) = submit.upgrade() {
                    add.emit_clicked();
                }
            });
            card.append(&row);
        }
        if let Some(opener) = opener {
            card.set_visible(false);
            let shell = gtk::Box::new(gtk::Orientation::Vertical, 8);
            let shown = card.downgrade();
            opener.connect_clicked(move |key| {
                key.set_visible(false);
                if let Some(card) = shown.upgrade() {
                    card.set_visible(true);
                }
            });
            shell.append(&opener);
            shell.append(&card);
            return Some(shell);
        }
        Some(card)
    }

    /// Every step, message and comment, oldest first, on one rail.
    fn timeline(self: &Rc<Self>) -> gtk::Box {
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rail.add_css_class("issue-timeline");
        let (older_history, older_messages) = self.cursors.get();
        if older_history.is_some() || older_messages.is_some() {
            let more = button("Load older activity", "quiet");
            more.set_halign(gtk::Align::Start);
            let weak = Rc::downgrade(self);
            more.connect_clicked(move |key| {
                if let Some(d) = weak.upgrade() {
                    key.set_sensitive(false);
                    d.load_older();
                }
            });
            rail.append(&more);
        }
        // (time, kind, row): kinds sort a comment after a step made in the same instant.
        let mut items: Vec<(String, u8, Value)> = Vec::new();
        items.extend(self.history.borrow().iter().map(|r| (text(r, "ts").to_string(), 0, r.clone())));
        items.extend(self.messages.borrow().iter().map(|r| (text(r, "sent_at").to_string(), 1, r.clone())));
        items.extend(self.comments.borrow().iter().map(|r| (text(r, "created_at").to_string(), 2, r.clone())));
        items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (at, kind, row) in items {
            match kind {
                0 => {
                    let Some(sentence) = activity_sentence(&row) else { continue };
                    let op = text(&row, "op");
                    let icon: gtk::Widget = match op {
                        "task.move" => status_icon(row["payload"]["column"].as_str().unwrap_or("backlog"), 14).upcast(),
                        "task.approve" => status_icon("done", 14).upcast(),
                        "task.create" => crate::icons::image("plus", 12).upcast(),
                        "task.dispatch" => crate::icons::image("user", 12).upcast(),
                        "task.link_commit" => crate::icons::image("commit", 12).upcast(),
                        "task.parent.set" | "task.relate" | "task.unrelate" => crate::icons::image("branch", 12).upcast(),
                        "task.delete" => crate::icons::image("trash", 12).upcast(),
                        "task.restore" | "task.unapprove" => crate::icons::image("undo", 12).upcast(),
                        _ => crate::icons::image("edit", 12).upcast(),
                    };
                    let event = label("", "issue-event-text");
                    event.set_markup(&format!(
                        "<b>{}</b> {} <span alpha=\"60%\">{}</span>",
                        glib::markup_escape_text(&actor_name(text(&row, "actor"))),
                        glib::markup_escape_text(&sentence),
                        glib::markup_escape_text(&since(&at))
                    ));
                    event.set_wrap(true);
                    event.set_tooltip_text(Some(&at));
                    rail.append(&timeline_row(icon, &event, op == "task.approve"));
                }
                1 => {
                    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    let event = label("", "issue-event-text");
                    event.set_markup(&format!(
                        "<b>{}</b> messaged <b>{}</b> <span alpha=\"60%\">{}</span>",
                        glib::markup_escape_text(&actor_name(text(&row, "from"))),
                        glib::markup_escape_text(text(&row, "to")),
                        glib::markup_escape_text(&since(&at))
                    ));
                    event.set_wrap(true);
                    body.append(&event);
                    let said = label(text(&row, "text"), "issue-message");
                    said.set_wrap(true);
                    said.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    said.set_selectable(true);
                    said.set_lines(6);
                    said.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    body.append(&said);
                    rail.append(&timeline_row(crate::icons::image("send", 12).upcast(), &body, false));
                }
                _ => {
                    let who = actor_name(text(&row, "author"));
                    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    card.add_css_class("issue-comment");
                    card.set_hexpand(true);
                    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    head.add_css_class("issue-comment-head");
                    let line = label("", "issue-comment-who");
                    line.set_markup(&format!(
                        "<b>{}</b> commented <span alpha=\"60%\">{}</span>",
                        glib::markup_escape_text(&who),
                        glib::markup_escape_text(&since(&at))
                    ));
                    line.set_tooltip_text(Some(&at));
                    head.append(&line);
                    card.append(&head);
                    let body = label("", "issue-comment-body");
                    body.set_markup(&markdown_markup(text(&row, "body")));
                    body.set_wrap(true);
                    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    body.set_selectable(true);
                    card.append(&body);
                    rail.append(&timeline_row(avatar(&who).upcast(), &card, false));
                }
            }
        }
        rail
    }

    fn load_older(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let (history, messages) = self.cursors.get();
        // A cursor of 0 asks a stream that is already complete for nothing.
        let payload = json!({"task_id":self.id,"before_audit":history.unwrap_or(0),"before_message":messages.unwrap_or(0),"limit":100});
        let detail = self.clone();
        glib::spawn_future_local(async move {
            match ui.call("task.activity", payload).await {
                Ok(data) => {
                    detail.history.borrow_mut().extend(rows(&data, "history"));
                    detail.messages.borrow_mut().extend(rows(&data, "messages"));
                    detail.cursors.set((
                        history.and(data["next_audit"].as_i64()),
                        messages.and(data["next_message"].as_i64()),
                    ));
                    // Only the timeline changes: the rest of the page, and any edit in it, stays.
                    if let Some(old) = detail.main.last_child() {
                        detail.main.remove(&old);
                    }
                    detail.main.append(&detail.timeline());
                }
                Err(e) => detail.say(&e.to_string()),
            }
        });
    }

    /// The comment box's keys: close as completed (approve) or reopen, and Comment.
    fn render_actions(self: &Rc<Self>, task: &Value) {
        clear(&self.actions);
        let project = self.project.get();
        let chosen_before = chosen(&self.recipient);
        self.recipient.remove_all();
        self.recipient.append(Some(""), "Comment only");
        if let Some(ui) = self.ui.upgrade() {
            for session in ui.sessions.borrow().iter().filter(|s| s["project_id"] == project && text(s, "state") != "closed") {
                self.recipient.append(Some(text(session, "name")), &format!("Also message {}", text(session, "name")));
            }
        }
        if !self.recipient.set_active_id(Some(&chosen_before)) {
            self.recipient.set_active(Some(0));
        }
        let done = text(task, "column") == "done";
        let close = button("", "issue-close");
        close.set_child(Some(&{
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            content.append(&status_icon(if done { "in_review" } else { "done" }, 13));
            content.append(&label(if done { "Reopen task" } else { "Close as completed" }, ""));
            content
        }));
        close.set_tooltip_text(Some(if done {
            "Move it back to In review"
        } else {
            "Approve it into Done: links the last agent's branch tip, or the project HEAD"
        }));
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            let Some(d) = weak.upgrade() else { return };
            let (op, payload) = if done {
                ("task.move", json!({"task_id":d.id,"column":"in_review"}))
            } else {
                ("task.approve", json!({"task_id":d.id}))
            };
            // A drafted comment goes first, as GitHub's "Close with comment".
            if d.comment.text().trim().is_empty() {
                d.act(op, payload);
            } else {
                d.post_comment(move |d| d.act(op, payload));
            }
        });
        self.actions.append(&close);
        let send = button("Comment", "primary");
        send.set_widget_name("task-comment");
        let weak = Rc::downgrade(self);
        send.connect_clicked(move |_| {
            if let Some(d) = weak.upgrade() {
                d.post_comment(|d| d.reload());
            }
        });
        self.actions.append(&send);
    }

    fn post_comment(self: &Rc<Self>, then: impl FnOnce(&Rc<Self>) + 'static) {
        let Some(ui) = self.ui.upgrade() else { return };
        let body = self.comment.text();
        if body.trim().is_empty() {
            self.say("Write a comment first.");
            self.comment.view.grab_focus();
            return;
        }
        if self.busy.replace(true) {
            return;
        }
        let to = chosen(&self.recipient);
        self.comment.root.set_sensitive(false);
        self.say("Posting…");
        let detail = self.clone();
        glib::spawn_future_local(async move {
            let posted = ui.call("task.comment", json!({"task_id":detail.id,"body":body})).await;
            let sent = match (&posted, to.is_empty()) {
                (Ok(_), false) => ui.call("mailbox.send", json!({"project_id":detail.project.get(),"to":to,"text":body,"re_task":detail.id})).await.map(|_| ()),
                _ => Ok(()),
            };
            detail.busy.set(false);
            detail.comment.root.set_sensitive(true);
            match (posted, sent) {
                (Ok(_), Ok(())) => {
                    detail.comment.clear();
                    detail.say("");
                    then(&detail);
                }
                (Ok(_), Err(e)) => {
                    detail.comment.clear();
                    detail.say(&format!("Commented, but the message to {to} failed: {e}"));
                    detail.reload();
                }
                (Err(e), _) => detail.say(&e.to_string()),
            }
        });
    }

    /// The fields, as pills that apply on a click: status, priority, size, type and module,
    /// then labels and who works on it.
    fn render_side(self: &Rc<Self>, task: &Value, modules: &[Value]) {
        clear(&self.side);
        let id = self.id;
        let project = self.project.get();
        let Some(ui) = self.ui.upgrade() else { return };

        let column = text(task, "column").to_string();
        let weak = Rc::downgrade(self);
        let from = column.clone();
        let status = pills(column_choices(COLUMNS), &column, column_icon, move |to| {
            let Some(d) = weak.upgrade() else { return };
            if to == from {
                return;
            }
            if to == "done" {
                d.act("task.approve", json!({"task_id":id}));
            } else {
                d.act("task.move", json!({"task_id":id,"column":to}));
            }
        });
        let weak = Rc::downgrade(self);
        let priority = pills(titled_choices(&URGENT_FIRST, ""), text(task, "priority"), priority_mark, move |p| {
            if let Some(d) = weak.upgrade() {
                d.update("priority", json!(p));
            }
        });
        let weak = Rc::downgrade(self);
        let size = pills(titled_choices(&SIZES, "None"), text(task, "size"), no_icon, move |s| {
            if let Some(d) = weak.upgrade() {
                d.update("size", if s.is_empty() { Value::Null } else { json!(s) });
            }
        });
        let weak = Rc::downgrade(self);
        let kind = pills(titled_choices(&TYPES, ""), text(task, "type"), type_icon, move |t| {
            if let Some(d) = weak.upgrade() {
                d.update("type", json!(t));
            }
        });
        let current = task["module_id"].as_i64();
        let weak = Rc::downgrade(self);
        let module = pills(module_choices(modules, current), &current.map(|m| m.to_string()).unwrap_or_default(), no_icon, move |m| {
            if let Some(d) = weak.upgrade() {
                d.update("module_id", m.parse::<i64>().map(Value::from).unwrap_or(Value::Null));
            }
        });
        for (title, control) in [("Status", status), ("Priority", priority), ("Size · how much work", size), ("Type", kind), ("Module", module)] {
            let section = side_section(title);
            section.append(&control);
            self.side.append(&section);
        }

        // Labels: removable chips and a field to add one.
        let tags = side_section("Labels");
        let mut chips: Vec<(gtk::Widget, usize)> = Vec::new();
        for tag in rows(task, "labels").iter().filter_map(|v| v.as_str().map(str::to_string)) {
            let wide = tag.chars().count() + 6;
            let key = button("", "issue-label");
            let content = label_chip(&tag);
            content.append(&crate::icons::image("close", 9));
            key.set_child(Some(&content));
            key.set_tooltip_text(Some(&format!("Remove {tag}")));
            let weak = Rc::downgrade(self);
            key.connect_clicked(move |_| {
                if let Some(d) = weak.upgrade() {
                    d.act("task.label.remove", json!({"task_id":id,"label":tag}));
                }
            });
            chips.push((key.upcast(), wide));
        }
        if !chips.is_empty() {
            tags.append(&wrapped(chips, 34));
        }
        let entry = gtk::Entry::builder().placeholder_text("Add a label").build();
        entry.add_css_class("issue-side-entry");
        let weak = Rc::downgrade(self);
        entry.connect_activate(move |entry| {
            let tag = entry.text().trim().to_string();
            if let (Some(d), false) = (weak.upgrade(), tag.is_empty()) {
                d.act("task.label.add", json!({"task_id":id,"label":tag}));
            }
        });
        tags.append(&entry);
        self.side.append(&tags);

        // The note it was made from: the thinking before the work, one click away.
        if let Some(note_id) = task["note_id"].as_i64() {
            let origin = side_section("From note");
            let key = button("", "issue-side-key");
            let row = icon_row("notes", "Open the note");
            key.set_child(Some(&row));
            key.set_halign(gtk::Align::Start);
            let caption = row.last_child().and_downcast::<gtk::Label>();
            if let Some(caption) = &caption {
                caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
                caption.set_max_width_chars(30);
            }
            let weak = Rc::downgrade(&ui);
            let named = caption.map(|c| c.downgrade());
            glib::spawn_future_local(async move {
                let Some(ui) = weak.upgrade() else { return };
                let title = match ui.call("notes.get", json!({"note_id":note_id})).await {
                    Ok(note) => text(&note, "title").trim().to_string(),
                    Err(_) => return,
                };
                if let Some(caption) = named.and_then(|c| c.upgrade()) {
                    caption.set_text(if title.is_empty() { "Untitled" } else { &title });
                }
            });
            let weak = Rc::downgrade(&ui);
            key.connect_clicked(move |_| {
                let Some(ui) = weak.upgrade() else { return };
                glib::spawn_future_local(async move {
                    match ui.call("notes.get", json!({"note_id":note_id})).await {
                        Ok(note) => crate::pages::open_note(&ui, note),
                        Err(error) => ui.show_error(&format!("Could not open the note: {error}")),
                    }
                });
            });
            origin.append(&key);
            self.side.append(&origin);
        }

        // Assignees: the agents it went to, and sending it to a live one.
        let people = side_section("Assignees");
        let agents: Vec<String> = rows(task, "sessions").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        if agents.is_empty() {
            people.append(&label("No agent yet", "issue-side-empty"));
        }
        for agent in &agents {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&avatar(agent));
            row.append(&label(agent, "issue-side-value"));
            people.append(&row);
        }
        let live: Vec<(String, String)> = ui
            .sessions
            .borrow()
            .iter()
            .filter(|s| text(s, "state") != "closed" && s["project_id"] == project && !agents.iter().any(|a| a == text(s, "name")))
            .map(|s| (text(s, "name").to_string(), text(s, "name").to_string()))
            .collect();
        if !live.is_empty() {
            people.append(&label("Send to a live agent", "issue-side-empty"));
            let weak = Rc::downgrade(self);
            people.append(&pills(live, "", no_icon, move |session| {
                if let Some(d) = weak.upgrade() {
                    d.act("task.dispatch", json!({"task_id":id,"session":session}));
                }
            }));
        }
        let launch = button("", "issue-side-key");
        launch.set_child(Some(&icon_row("plus", "Start new agents on it")));
        launch.set_halign(gtk::Align::Start);
        let weak = Rc::downgrade(self);
        launch.connect_clicked(move |_| {
            let Some(d) = weak.upgrade() else { return };
            if !d.panel.can_close() {
                return;
            }
            if let Some(ui) = d.ui.upgrade() {
                d.panel.close();
                ui.show_launch(Some(id));
            }
        });
        people.append(&launch);
        self.side.append(&people);

        let danger = side_section("");
        let delete = button("", "issue-delete");
        delete.set_child(Some(&icon_row("trash", "Delete task")));
        delete.set_halign(gtk::Align::Start);
        delete.set_tooltip_text(Some("Ctrl+Z on the board restores it"));
        let weak = Rc::downgrade(self);
        crate::app::confirm_inline(&delete, "Delete this task?", move |_| {
            if let Some(d) = weak.upgrade() {
                d.editing.borrow_mut().clear();
                d.act_then("task.delete", json!({"task_id":id}), |d| {
                    d.busy.set(false);
                    d.panel.close();
                });
            }
        });
        danger.append(&delete);
        self.side.append(&danger);
    }
}

fn icon_row(icon: &str, caption: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&crate::icons::image(icon, 12));
    row.append(&label(caption, ""));
    row
}

/// One step on the timeline: its badge on the rail, then what happened.
fn timeline_row(icon: gtk::Widget, content: &impl IsA<gtk::Widget>, closed: bool) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("issue-event");
    let rail = gtk::Overlay::new();
    rail.add_css_class("issue-rail");
    let line = gtk::Box::new(gtk::Orientation::Vertical, 0);
    line.add_css_class("issue-rail-line");
    line.set_halign(gtk::Align::Center);
    line.set_vexpand(true);
    rail.set_child(Some(&line));
    let badge = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    badge.add_css_class("issue-badge");
    if closed {
        badge.add_css_class("issue-badge-closed");
    }
    badge.set_halign(gtk::Align::Center);
    badge.set_valign(gtk::Align::Start);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon.set_hexpand(true);
    badge.append(&icon);
    rail.add_overlay(&badge);
    rail.set_size_request(32, -1);
    row.append(&rail);
    content.set_hexpand(true);
    content.set_valign(gtk::Align::Center);
    row.append(content);
    row
}
