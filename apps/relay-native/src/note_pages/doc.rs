//! One open note: a GtkSourceView Markdown editor, its find/replace bar, go-to-line,
//! status bar and tab, wrapped around the shared `Draft` that owns its dirty state.
use super::glyphs::glyph;
use super::text::{self as tx, Enter, Inline, Prefix};
use super::*;
use crate::editor::find_bar::FindBar;
use sourceview5::prelude::*;

pub struct Doc {
    pub id: i64,
    pub project: i64,
    pub draft: Rc<Draft>,
    pub title: gtk::Entry,
    meta: gtk::Label,
    pub pin: gtk::ToggleButton,
    pub view: sourceview5::View,
    pub buffer: sourceview5::Buffer,
    /// The find and replace bar, which Code's editor shares.
    pub find: Rc<FindBar>,
    notice: gtk::Box,
    notice_text: gtk::Label,
    notice_actions: gtk::Box,
    state: gtk::Label,
    pub position: gtk::MenuButton,
    position_label: gtk::Label,
    goto_entry: gtk::Entry,
    goto_hint: gtk::Label,
    counts: gtk::Label,
    mode: gtk::Button,
    eol: gtk::Label,
    zoom: gtk::Button,
    autosave: gtk::Button,
    pub tab: gtk::Box,
    tab_dot: gtk::Box,
    tab_label: gtk::Label,
    /// The title as stored, which the engine keeps verbatim: the snapshot reports it unchanged
    /// while the field shows the same text, so surrounding whitespace is not an edit.
    stored_title: Rc<RefCell<Value>>,
    pub deleted: Cell<bool>,
    pub conflict: Cell<bool>,
    /// Its project was removed: the tab stays on screen whatever project is shown, and Save
    /// as new note files the text under the project shown.
    pub orphan: Cell<bool>,
    seen_remote: RefCell<String>,
    syncing: Cell<bool>,
    last_dirty: Cell<bool>,
    autosave_timer: RefCell<Option<glib::SourceId>>,
    count_timer: RefCell<Option<glib::SourceId>>,
    state_idle: RefCell<Option<glib::SourceId>>,
    flash_timer: RefCell<Option<glib::SourceId>>,
    /// Checkboxes and pictures drawn over the text.
    pub inline: super::inline::Inline,
    /// The task the last "Create task" made, for its notice's Open key.
    last_task: Cell<i64>,
    /// The tasks made from this note, under its title: the thinking and the work it became.
    tasks: gtk::Box,
    /// Bumped by every `load_tasks`, so a slow answer never overwrites a newer one.
    tasks_generation: Cell<u64>,
    /// The tab was asked to close while a save ran: it closes once that save lands.
    pub close_pending: Cell<bool>,
    /// Made empty by New note (which creates it at once) and stored as made since: closing
    /// its tab deletes it, so an untouched Ctrl+N leaves no "Untitled" note behind.
    pub fresh: Cell<bool>,
}

/// The largest body this client sends. Replies and `notes.changed` events carry the whole
/// body back over a socket that drops lines over 2 MiB, so a larger note could be stored but
/// never read back here.
pub const MAX_BODY: usize = 512 * 1024;

/// Why `body` is refused, when it is over `MAX_BODY`.
pub fn oversize(body: &str) -> Option<String> {
    (body.len() > MAX_BODY).then(|| {
        format!(
            "This text is {} KB; a note holds at most {} KB. Move part of it into another note.",
            body.len() / 1024,
            MAX_BODY / 1024
        )
    })
}

type Action = (&'static str, fn(&Rc<Ui>, &Rc<Doc>));
/// What to do once a save succeeds (close the tab, for instance).
pub type AfterSave = Option<Box<dyn FnOnce(&Rc<Ui>, &Rc<Doc>)>>;

fn status_button(caption: &str, tip: &str) -> gtk::Button {
    let key = gtk::Button::with_label(caption);
    key.add_css_class("notes-status-item");
    key.set_focus_on_click(false);
    key.set_tooltip_text(Some(tip));
    key
}

fn status_separator() -> gtk::Separator {
    let line = gtk::Separator::new(gtk::Orientation::Vertical);
    line.add_css_class("notes-status-sep");
    line
}

pub fn build(ui: &Rc<Ui>, note: &Value) -> Rc<Doc> {
    let id = note["id"].as_i64().unwrap_or(0);
    let project = note["project_id"].as_i64().unwrap_or(ui.project.get());
    let prefs = super::prefs();
    let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    form.add_css_class("notes-doc");
    form.set_vexpand(true);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    head.add_css_class("notes-doc-head");
    let title = gtk::Entry::builder()
        .text(&*tx::clean(text(note, "title")))
        .placeholder_text("Untitled")
        .hexpand(true)
        .build();
    title.set_widget_name("note-title");
    title.add_css_class("notes-title");
    title.update_property(&[gtk::accessible::Property::Label("Note title")]);
    head.append(&title);
    let meta = label("", "notes-meta");
    meta.set_valign(gtk::Align::Center);
    head.append(&meta);
    let pin = gtk::ToggleButton::new();
    pin.set_child(Some(&glyph("pin", 15)));
    pin.add_css_class("notes-pin");
    pin.set_valign(gtk::Align::Center);
    pin.set_focus_on_click(false);
    pin.set_tooltip_text(Some("Pin this note to the top of the library"));
    pin.set_active(note["pinned"].as_bool().unwrap_or(false));
    head.append(&pin);
    form.append(&head);
    let tasks = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tasks.add_css_class("notes-tasks");
    tasks.set_visible(false);
    form.append(&tasks);

    let notice = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    notice.add_css_class("notes-notice");
    notice.set_visible(false);
    let notice_text = label("", "notes-notice-text");
    notice_text.set_wrap(true);
    notice_text.set_hexpand(true);
    notice_text.set_selectable(true);
    let notice_actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    notice_actions.set_valign(gtk::Align::Center);
    notice.append(&notice_text);
    notice.append(&notice_actions);
    form.append(&notice);

    let buffer = sourceview5::Buffer::new(None);
    if let Some(language) = sourceview5::LanguageManager::default().language("markdown") {
        buffer.set_language(Some(&language));
    }
    if let Some(scheme) = super::scheme() {
        buffer.set_style_scheme(Some(&scheme));
    }
    buffer.set_highlight_matching_brackets(false);
    buffer.begin_irreversible_action();
    buffer.set_text(&tx::clean(text(note, "body")));
    buffer.end_irreversible_action();
    buffer.place_cursor(&buffer.start_iter());
    let view = sourceview5::View::with_buffer(&buffer);
    view.set_widget_name("note-body");
    view.add_css_class("notes-source");
    view.set_auto_indent(true);
    view.set_tab_width(4);
    view.set_insert_spaces_instead_of_tabs(true);
    view.set_smart_home_end(sourceview5::SmartHomeEndType::Before);
    view.set_left_margin(18);
    view.set_right_margin(18);
    view.set_top_margin(14);
    view.set_bottom_margin(28);
    view.set_pixels_below_lines(2);
    view.set_pixels_inside_wrap(1);
    view.upcast_ref::<gtk::Widget>().update_property(&[gtk::accessible::Property::Label("Note text")]);
    view.set_extra_menu(Some(&super::menu::text_menu()));
    let scroll = crate::app::scrolled(&view);
    scroll.add_css_class("notes-scroll");
    form.append(&scroll);

    // Every note's bar starts from the last query any of them searched for.
    let find = FindBar::new(&view, &buffer, "notes", "Find", &super::LAST_QUERY);
    find.bar.add_css_class("notes-find");
    form.append(&find.bar);

    let status = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    status.add_css_class("notes-status");
    let state = label("Saved", "notes-state");
    state.set_width_chars(9);
    status.append(&state);
    status.append(&status_separator());
    let position = gtk::MenuButton::new();
    position.add_css_class("notes-status-menu");
    position.set_direction(gtk::ArrowType::Up);
    position.set_tooltip_text(Some("Go to line (Ctrl+G)"));
    let position_label = label("Ln 1, Col 1", "");
    position.set_child(Some(&position_label));
    status.append(&position);
    status.append(&status_separator());
    let counts = label("", "notes-status-text");
    counts.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.append(&counts);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    status.append(&spacer);
    let mode = status_button("Markdown", "Markdown highlighting");
    mode.set_action_name(Some("notes.markdown"));
    status.append(&mode);
    status.append(&status_separator());
    status.append(&label("UTF-8", "notes-status-text"));
    status.append(&status_separator());
    let eol = label("LF", "notes-status-text");
    status.append(&eol);
    status.append(&status_separator());
    let zoom = status_button("100%", "Reset zoom (Ctrl+0)");
    zoom.set_action_name(Some("notes.zoom-reset"));
    status.append(&zoom);
    status.append(&status_separator());
    let autosave = status_button("Autosave off", "Save automatically after a pause in typing");
    autosave.set_action_name(Some("notes.autosave"));
    status.append(&autosave);
    form.append(&status);

    let goto = gtk::Popover::new();
    goto.add_css_class("notes-goto");
    let goto_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    goto_box.append(&label("Go to line", "notes-goto-title"));
    let goto_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let goto_entry = gtk::Entry::builder()
        .placeholder_text("Line or line:column")
        .width_chars(18)
        .build();
    goto_entry.set_widget_name("notes-goto-entry");
    let goto_key = button("Go", "primary");
    goto_row.append(&goto_entry);
    goto_row.append(&goto_key);
    goto_box.append(&goto_row);
    let goto_hint = label("", "notes-goto-hint");
    goto_box.append(&goto_hint);
    goto.set_child(Some(&goto_box));
    position.set_popover(Some(&goto));

    let tab = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tab.add_css_class("notes-tab");
    let tab_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tab_dot.add_css_class("notes-dirty-dot");
    tab_dot.set_valign(gtk::Align::Center);
    tab_dot.set_visible(false);
    let tab_label = label("", "notes-tab-label");
    tab_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    tab_label.set_max_width_chars(24);
    let tab_close = gtk::Button::new();
    tab_close.set_child(Some(&glyph("close", 12)));
    tab_close.add_css_class("notes-tab-close");
    tab_close.set_focus_on_click(false);
    tab_close.set_tooltip_text(Some("Close (Ctrl+W)"));
    tab.append(&tab_dot);
    tab.append(&tab_label);
    tab.append(&tab_close);

    let stored_title = Rc::new(RefCell::new(note["title"].clone()));
    let snapshot: Rc<dyn Fn() -> Value> = Rc::new({
        let title = title.clone();
        let buffer = buffer.clone();
        let stored_title = stored_title.clone();
        move || {
            let value = title.text().trim().to_string();
            let stored = stored_title.borrow();
            json!({
                "title": if stored.as_str().is_some_and(|s| s.trim() == value) {
                    stored.clone()
                } else if value.is_empty() && stored.is_null() {
                    Value::Null
                } else {
                    json!(value)
                },
                "body": buffer_text(buffer.upcast_ref()),
            })
        }
    });
    let draft = Draft::new_note(ui, "Note", note.clone(), snapshot, form.clone());
    // The note form fills the tab: unwrap Draft's scrolled form and hide its own status
    // and footer, which this editor replaces with a status bar and the File menu.
    draft.layout.remove(&draft.status);
    draft.layout.remove(&draft.footer);
    if let Some(scroll) = draft.layout.first_child().and_downcast::<gtk::ScrolledWindow>() {
        scroll.set_child(gtk::Widget::NONE);
        draft.layout.remove(&scroll);
    }
    draft.layout.append(&form);
    draft.form.set_valign(gtk::Align::Fill);
    draft.layout.set_spacing(0);
    draft.layout.set_margin_top(0);
    draft.layout.set_margin_bottom(0);
    draft.layout.set_margin_start(0);
    draft.layout.set_margin_end(0);
    draft.layout.add_css_class("notes-page-body");
    draft.layout.set_vexpand(true);

    let inline = super::inline::Inline::new(&buffer);
    let doc = Rc::new(Doc {
        id,
        project,
        draft,
        title,
        meta,
        pin,
        view,
        buffer,
        find,
        notice,
        notice_text,
        notice_actions,
        state,
        position,
        position_label,
        goto_entry: goto_entry.clone(),
        goto_hint,
        counts,
        mode,
        eol,
        zoom,
        autosave,
        tab,
        tab_dot,
        tab_label,
        stored_title,
        deleted: Cell::new(false),
        conflict: Cell::new(false),
        orphan: Cell::new(false),
        seen_remote: RefCell::new(String::new()),
        syncing: Cell::new(false),
        last_dirty: Cell::new(false),
        autosave_timer: RefCell::new(None),
        count_timer: RefCell::new(None),
        state_idle: RefCell::new(None),
        flash_timer: RefCell::new(None),
        inline,
        last_task: Cell::new(0),
        tasks,
        tasks_generation: Cell::new(0),
        close_pending: Cell::new(false),
        fresh: Cell::new(false),
    });
    let weak_ui = Rc::downgrade(ui);
    let weak = Rc::downgrade(&doc);
    let with = move |f: fn(&Rc<Ui>, &Rc<Doc>)| {
        let (weak_ui, weak) = (weak_ui.clone(), weak.clone());
        move || {
            if let (Some(ui), Some(doc)) = (weak_ui.upgrade(), weak.upgrade()) {
                f(&ui, &doc)
            }
        }
    };

    let changed = with(|ui, doc| doc.changed(ui));
    doc.buffer.connect_changed(move |_| changed());
    let relayout = with(|_, doc| super::inline::schedule(doc));
    doc.buffer.connect_changed(move |_| relayout());
    for adjustment in [doc.view.vadjustment(), doc.view.hadjustment()].into_iter().flatten() {
        let place = with(|_, doc| super::inline::schedule_place(doc));
        adjustment.connect_changed(move |_| place());
    }
    let (weak_ui, weak) = (Rc::downgrade(ui), Rc::downgrade(&doc));
    doc.view.connect_paste_clipboard(move |view| {
        if let (Some(ui), Some(doc)) = (weak_ui.upgrade(), weak.upgrade()) {
            if super::inline::paste(&ui, &doc) {
                view.stop_signal_emission_by_name("paste-clipboard");
            }
        }
    });
    for cut in [false, true] {
        let weak = Rc::downgrade(&doc);
        let copied = move |view: &sourceview5::View| {
            if weak.upgrade().is_some_and(|doc| doc.copy_with_images(cut)) {
                view.stop_signal_emission_by_name(if cut { "cut-clipboard" } else { "copy-clipboard" });
            }
        };
        if cut {
            doc.view.connect_cut_clipboard(copied);
        } else {
            doc.view.connect_copy_clipboard(copied);
        }
    }
    doc.view.add_controller(super::inline::drop_target(ui, &doc));
    doc.view.add_controller(super::inline::picture_drop(&doc));
    doc.view.add_controller(super::inline::menu_keys(&doc));
    let changed = with(|ui, doc| doc.changed(ui));
    doc.title.connect_changed(move |_| changed());
    let focus_body = with(|_, doc| {
        doc.view.grab_focus();
    });
    doc.title.connect_activate(move |_| focus_body());
    let cursor = with(|_, doc| doc.update_position());
    doc.buffer.connect_cursor_position_notify(move |_| cursor());
    let moved = with(|_, doc| super::inline::cursor_moved(doc));
    doc.buffer.connect_cursor_position_notify(move |_| moved());
    let selection = with(|_, doc| doc.schedule_counts());
    doc.buffer.connect_has_selection_notify(move |_| selection());
    let pinned = with(pin_changed);
    doc.pin.connect_toggled(move |_| pinned());
    // The key and File › Pin to top say whether the note is pinned, whoever pinned it.
    let pin_shown = with(|ui, doc| {
        doc.show_pin();
        if super::current_doc(ui).is_some_and(|current| Rc::ptr_eq(&current, doc)) {
            super::menu::sync(Some(doc));
        }
    });
    doc.pin.connect_toggled(move |_| pin_shown());
    let close = with(request_close);
    tab_close.connect_clicked(move |_| close());
    let middle = gtk::GestureClick::new();
    middle.set_button(2);
    let close = with(request_close);
    middle.connect_released(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        close();
    });
    doc.tab.add_controller(middle);

    let opened = with(|_, doc| {
        doc.goto_entry.set_text("");
        doc.goto_hint_reset();
        doc.goto_entry.grab_focus();
    });
    goto.connect_show(move |_| opened());
    // A refused entry is marked until it is typed over.
    let typed = with(|_, doc| doc.goto_hint_reset());
    goto_entry.connect_changed(move |_| typed());
    let jump = with(|_, doc| doc.goto_from_entry());
    goto_entry.connect_activate(move |_| jump());
    let jump = with(|_, doc| doc.goto_from_entry());
    goto_key.connect_clicked(move |_| jump());

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let enter = Rc::downgrade(&doc);
    keys.connect_key_pressed(move |_, key, _, mods| {
        let plain = (mods & (gtk::gdk::ModifierType::SHIFT_MASK
            | gtk::gdk::ModifierType::CONTROL_MASK
            | gtk::gdk::ModifierType::ALT_MASK))
            .is_empty();
        if plain && matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
            if let Some(doc) = enter.upgrade() {
                if doc.enter() {
                    return glib::Propagation::Stop;
                }
            }
        }
        glib::Propagation::Proceed
    });
    doc.view.add_controller(keys);
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let zoom_ui = Rc::downgrade(ui);
    // Touchpads and high-resolution wheels send many small deltas: zoom per whole notch.
    let travel = Cell::new(0.0);
    wheel.connect_scroll(move |wheel, _, dy| {
        if wheel.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            let pixels = wheel.unit() == gtk::gdk::ScrollUnit::Surface;
            let (steps, rest) = tx::zoom_notches(travel.get(), dy, pixels);
            travel.set(rest);
            if steps != 0 {
                if let Some(ui) = zoom_ui.upgrade() {
                    super::zoom(&ui, steps.signum());
                }
            }
            return glib::Propagation::Stop;
        }
        travel.set(0.0);
        glib::Propagation::Proceed
    });
    doc.view.add_controller(wheel);

    doc.apply_prefs(&prefs);
    doc.show_pin();
    super::inline::schedule(&doc);
    doc.update_meta();
    doc.update_position();
    doc.update_counts();
    doc.refresh_state(ui);
    doc
}

impl Doc {
    pub fn body(&self) -> String {
        buffer_text(self.buffer.upcast_ref())
    }

    pub fn dirty(&self) -> bool {
        self.draft.dirty()
    }

    /// The dirty state the last `refresh_state` found, without copying the text again.
    pub fn shown_dirty(&self) -> bool {
        self.last_dirty.get()
    }

    /// The tab and window name: the title, else the first line of text.
    pub fn name(&self) -> String {
        let title = self.title.text();
        if !title.trim().is_empty() {
            return title.trim().to_string();
        }
        let end = self.buffer.iter_at_line(24).unwrap_or_else(|| self.buffer.end_iter());
        tx::display_title("", &self.buffer.text(&self.buffer.start_iter(), &end, false))
    }

    pub fn set_base(&self, note: Value) {
        // Saved or changed elsewhere: no longer the empty note New note made.
        self.fresh.set(false);
        *self.stored_title.borrow_mut() = note["title"].clone();
        *self.draft.base.borrow_mut() = note;
        self.sync_pin();
        self.update_meta();
    }

    /// Replace the text with `note`, without an undo step, keeping the cursor nearby.
    pub fn load(&self, note: Value) {
        let offset = self.buffer.cursor_position();
        self.title.set_text(&tx::clean(text(&note, "title")));
        self.buffer.begin_irreversible_action();
        self.buffer.set_text(&tx::clean(text(&note, "body")));
        self.buffer.end_irreversible_action();
        let at = self.buffer.iter_at_offset(offset.min(self.buffer.char_count()));
        self.buffer.place_cursor(&at);
        self.set_base(note);
        self.deleted.set(false);
        self.conflict.set(false);
        self.seen_remote.borrow_mut().clear();
        self.hide_notice();
    }

    fn sync_pin(&self) {
        let pinned = self.draft.base.borrow()["pinned"].as_bool().unwrap_or(false);
        if self.pin.is_active() != pinned {
            self.syncing.set(true);
            self.pin.set_active(pinned);
            self.syncing.set(false);
        }
    }

    fn show_pin(&self) {
        self.pin.set_tooltip_text(Some(if self.pin.is_active() {
            "Unpin this note"
        } else {
            "Pin this note to the top of the library"
        }));
    }

    pub fn update_meta(&self) {
        let base = self.draft.base.borrow();
        let when = super::fmt_date(text(&base, "updated_at"));
        self.meta.set_text(&if when.is_empty() { String::new() } else { format!("Edited {when}") });
        self.meta.set_tooltip_text(Some(&format!(
            "Created {}\nEdited {}",
            super::fmt_date(text(&base, "created_at")),
            when
        )));
    }

    /// Typing, Replace all and undo emit 'changed' once per edit, and working out the state
    /// copies the whole note, so that happens once per main-loop pass instead.
    fn changed(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.schedule_counts();
        if self.state_idle.borrow().is_some() {
            return;
        }
        let (weak_ui, weak) = (Rc::downgrade(ui), Rc::downgrade(self));
        *self.state_idle.borrow_mut() = Some(glib::idle_add_local_once(move || {
            let (Some(ui), Some(doc)) = (weak_ui.upgrade(), weak.upgrade()) else { return };
            doc.state_idle.borrow_mut().take();
            let dirty = doc.refresh_state(&ui);
            doc.arm_autosave(&ui, dirty);
        }));
    }

    /// Show the save state everywhere it appears; returns whether the note is dirty.
    pub fn refresh_state(&self, ui: &Rc<Ui>) -> bool {
        let dirty = self.dirty();
        let (caption, class) = if self.draft.busy.get() {
            ("Saving…", "saving")
        } else if self.orphan.get() {
            ("Project removed", "warning")
        } else if self.deleted.get() {
            ("Deleted elsewhere", "warning")
        } else if self.conflict.get() {
            ("Not saved", "warning")
        } else if dirty {
            ("Modified", "modified")
        } else {
            ("Saved", "saved")
        };
        self.state.set_text(caption);
        for name in ["saving", "warning", "modified", "saved"] {
            if name == class {
                self.state.add_css_class(name);
            } else {
                self.state.remove_css_class(name);
            }
        }
        self.tab_dot.set_visible(dirty);
        let name = self.name();
        if self.tab_label.text() != name {
            self.tab_label.set_text(&name);
            // An ellipsizing label asks for no width at all; a notebook tab then shows only "…".
            self.tab_label.set_width_chars(name.chars().count().clamp(4, 24) as i32);
            self.tab.set_tooltip_text(Some(&format!("{} · {name}", super::project_name(ui, self.project))));
        }
        if self.last_dirty.replace(dirty) != dirty {
            super::dirty_changed(ui, self.id, dirty);
        }
        super::update_chrome(ui);
        dirty
    }

    pub fn schedule_autosave(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.arm_autosave(ui, self.dirty());
    }

    fn arm_autosave(self: &Rc<Self>, ui: &Rc<Ui>, dirty: bool) {
        if let Some(timer) = self.autosave_timer.borrow_mut().take() {
            timer.remove();
        }
        if !super::prefs().autosave || self.deleted.get() || self.conflict.get() || !dirty {
            return;
        }
        let (weak_ui, weak) = (Rc::downgrade(ui), Rc::downgrade(self));
        *self.autosave_timer.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(1500),
            move || {
                let (Some(ui), Some(doc)) = (weak_ui.upgrade(), weak.upgrade()) else { return };
                doc.autosave_timer.borrow_mut().take();
                if doc.draft.busy.get() {
                    doc.schedule_autosave(&ui);
                } else {
                    save(&ui, &doc, None);
                }
            },
        ));
    }

    pub fn stop_timers(&self) {
        for timer in [&self.autosave_timer, &self.count_timer, &self.state_idle, &self.flash_timer] {
            if let Some(timer) = timer.borrow_mut().take() {
                timer.remove();
            }
        }
        self.inline.stop();
    }

    /// Say `message` in the status bar for a moment, where the counts are.
    pub fn flash(self: &Rc<Self>, message: &str) {
        self.counts.set_text(message);
        if let Some(timer) = self.flash_timer.borrow_mut().take() {
            timer.remove();
        }
        let weak = Rc::downgrade(self);
        *self.flash_timer.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(1800),
            move || {
                if let Some(doc) = weak.upgrade() {
                    doc.flash_timer.borrow_mut().take();
                    doc.update_counts();
                }
            },
        ));
    }

    /// Copy or cut a selection of pictures alone: the files and, for one, the picture travel
    /// with the text. A selection with words in it copies as text (`tx::only_images`). Returns
    /// whether it handled the copy.
    fn copy_with_images(&self, cut: bool) -> bool {
        let Some((start, end)) = self.buffer.selection_bounds() else { return false };
        let selected = self.buffer.text(&start, &end, false).to_string();
        if !tx::only_images(&selected) {
            return false;
        }
        let images = super::inline::referenced(&selected);
        if images.is_empty() {
            return false;
        }
        super::inline::copy_images(&self.view.clipboard(), Some(&selected), &images);
        if cut {
            self.buffer.begin_user_action();
            self.buffer.delete_selection(true, true);
            self.buffer.end_user_action();
        }
        true
    }

    /// Tick or untick the checklist item on `line`, as one undo step.
    pub fn toggle_check_line(&self, line: i32) -> bool {
        let buffer = &self.buffer;
        let Some(start) = buffer.iter_at_line(line) else { return false };
        let mut end = start;
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        let Some((column, ticked)) = tx::check_item(&buffer.text(&start, &end, true)) else { return false };
        let cursor = buffer.cursor_position();
        let mut from = start;
        from.set_line_offset(column as i32 + 1);
        let mut to = from;
        to.forward_char();
        buffer.begin_user_action();
        buffer.delete(&mut from, &mut to);
        buffer.insert(&mut from, if ticked { " " } else { "x" });
        buffer.end_user_action();
        buffer.place_cursor(&buffer.iter_at_offset(cursor));
        true
    }

    /// Ctrl+Enter: tick the item under the cursor, or make the line one.
    pub fn toggle_check_at_cursor(&self) {
        let line = self.buffer.iter_at_mark(&self.buffer.get_insert()).line();
        if !self.toggle_check_line(line) {
            self.toggle_prefix(Prefix::Check);
        }
    }

    pub fn apply_prefs(&self, prefs: &super::Prefs) {
        self.view.set_wrap_mode(if prefs.wrap { gtk::WrapMode::WordChar } else { gtk::WrapMode::None });
        self.view.set_show_line_numbers(prefs.line_numbers);
        self.view.set_highlight_current_line(prefs.current_line);
        self.buffer.set_highlight_syntax(prefs.markdown);
        self.mode.set_label(if prefs.markdown { "Markdown" } else { "Plain text" });
        self.zoom.set_label(&format!("{}%", prefs.zoom));
        self.autosave.set_label(if prefs.autosave { "Autosave on" } else { "Autosave off" });
        if prefs.autosave {
            self.autosave.add_css_class("on");
        } else {
            self.autosave.remove_css_class("on");
        }
    }

    pub fn show_notice(self: &Rc<Self>, ui: &Rc<Ui>, message: &str, actions: Vec<Action>) {
        self.notice.remove_css_class("info");
        self.notice_text.set_text(message);
        while let Some(child) = self.notice_actions.first_child() {
            self.notice_actions.remove(&child);
        }
        for (caption, act) in actions {
            let key = button(caption, "notes-notice-button");
            key.set_focus_on_click(false);
            let (weak_ui, weak) = (Rc::downgrade(ui), Rc::downgrade(self));
            key.connect_clicked(move |_| {
                if let (Some(ui), Some(doc)) = (weak_ui.upgrade(), weak.upgrade()) {
                    act(&ui, &doc)
                }
            });
            self.notice_actions.append(&key);
        }
        let dismiss = gtk::Button::new();
        dismiss.set_child(Some(&glyph("close", 12)));
        dismiss.add_css_class("notes-notice-close");
        dismiss.set_tooltip_text(Some("Dismiss"));
        let weak = Rc::downgrade(self);
        dismiss.connect_clicked(move |_| {
            if let Some(doc) = weak.upgrade() {
                doc.hide_notice()
            }
        });
        self.notice_actions.append(&dismiss);
        self.notice.set_visible(true);
    }

    pub fn hide_notice(&self) {
        self.notice.set_visible(false);
    }

    // ---- statistics and position -------------------------------------------------

    pub fn update_position(&self) {
        let at = self.buffer.iter_at_mark(&self.buffer.get_insert());
        self.position_label.set_text(&format!(
            "Ln {}, Col {}",
            at.line() + 1,
            self.view.visual_column(&at) + 1
        ));
    }

    fn schedule_counts(self: &Rc<Self>) {
        if self.count_timer.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        *self.count_timer.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(120),
            move || {
                if let Some(doc) = weak.upgrade() {
                    doc.count_timer.borrow_mut().take();
                    doc.update_counts();
                }
            },
        ));
    }

    fn update_counts(&self) {
        if let Some((start, end)) = self.buffer.selection_bounds() {
            let selected = self.buffer.text(&start, &end, false);
            let (lines, words, chars) = tx::stats(&selected);
            self.counts.set_text(&format!(
                "{} selected · {} · {}",
                tx::thousands(chars),
                tx::counted(words, "word", "words"),
                tx::counted(lines, "line", "lines")
            ));
        } else {
            let body = self.body();
            let (lines, words, chars) = tx::stats(&body);
            self.counts.set_text(&format!(
                "{} · {} · {}",
                tx::counted(lines, "line", "lines"),
                tx::counted(words, "word", "words"),
                tx::counted(chars, "character", "characters")
            ));
            self.eol.set_text(if body.contains("\r\n") { "CRLF" } else { "LF" });
        }
    }

    pub fn goto(&self, line: i32, column: Option<i32>) {
        let line = line.clamp(1, self.buffer.line_count().max(1));
        let Some(mut at) = self.buffer.iter_at_line(line - 1) else { return };
        if let Some(column) = column {
            let mut end = at;
            if !end.ends_line() {
                end.forward_to_line_end();
            }
            at.set_line_offset((column - 1).clamp(0, end.line_offset()));
        }
        self.buffer.place_cursor(&at);
        self.view.grab_focus();
        let view = self.view.clone();
        let mark = self.buffer.get_insert();
        glib::idle_add_local_once(move || view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.35));
        self.update_position();
    }

    fn goto_from_entry(&self) {
        let entry = &self.goto_entry;
        match tx::parse_goto(&entry.text()) {
            Some((line, column)) => {
                if let Some(popover) = self.position.popover() {
                    popover.popdown();
                }
                self.goto(line, column);
            }
            None => {
                entry.add_css_class("error");
                self.goto_hint.set_text("Type a line number, or line:column");
            }
        }
    }

    /// The go-to field's hint as it opens: the lines there are to go to.
    fn goto_hint_reset(&self) {
        self.goto_entry.remove_css_class("error");
        self.goto_hint.set_text(&match self.buffer.line_count() {
            1 => String::from("Line 1, column optional"),
            lines => format!("Line 1–{lines}, column optional"),
        });
    }

    // ---- find and replace (`crate::editor::find_bar`, shared with Code) -------------

    pub fn open_find(&self, replace: bool) {
        self.find.open(replace);
    }

    pub fn close_find(&self) {
        self.find.close();
    }

    pub fn find_visible(&self) -> bool {
        self.find.visible()
    }

    /// F3 / Shift+F3. Works with the bar closed, using the last query.
    pub fn find_step(&self, backward: bool) {
        self.find.step(backward);
    }

    // ---- Markdown editing ---------------------------------------------------------

    /// Continue or end a Markdown list on Enter. Returns whether the key was handled.
    fn enter(&self) -> bool {
        let buffer = &self.buffer;
        if buffer.has_selection() {
            return false;
        }
        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        let mut start = cursor;
        start.set_line_offset(0);
        let mut end = cursor;
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        let before = buffer.text(&start, &cursor, false);
        let after = buffer.text(&cursor, &end, false);
        match tx::list_enter(&before, &after) {
            None => false,
            Some(Enter::End) => {
                buffer.begin_user_action();
                buffer.delete(&mut start, &mut end);
                buffer.end_user_action();
                true
            }
            Some(Enter::Continue(prefix)) => {
                let mut at = cursor;
                buffer.begin_user_action();
                buffer.insert(&mut at, &format!("\n{prefix}"));
                buffer.end_user_action();
                self.view.scroll_mark_onscreen(&buffer.get_insert());
                true
            }
        }
    }

    fn select_offsets(&self, start: i32, end: i32) {
        self.buffer.select_range(&self.buffer.iter_at_offset(start), &self.buffer.iter_at_offset(end));
    }

    /// Wrap the selection in `before`/`after`, or unwrap it when it already is.
    pub fn wrap_inline(&self, before: &str, after: &str, placeholder: &str) {
        let buffer = &self.buffer;
        let (b, a) = (before.chars().count() as i32, after.chars().count() as i32);
        buffer.begin_user_action();
        if let Some((mut start, mut end)) = buffer.selection_bounds() {
            let selected = buffer.text(&start, &end, false).to_string();
            let length = selected.chars().count() as i32;
            let at = start.offset();
            let mut line_start = start;
            line_start.set_line_offset(0);
            let mut line_end = end;
            if !line_end.ends_line() {
                line_end.forward_to_line_end();
            }
            let ahead = buffer.text(&line_start, &start, false);
            let behind = buffer.text(&end, &line_end, false);
            match tx::inline_toggle(&ahead, &selected, &behind, before, after) {
                Inline::Outside => {
                    let mut outer_start = buffer.iter_at_offset(at - b);
                    let mut outer_end = buffer.iter_at_offset(end.offset() + a);
                    buffer.delete(&mut outer_start, &mut outer_end);
                    buffer.insert(&mut buffer.iter_at_offset(at - b), &selected);
                    self.select_offsets(at - b, at - b + length);
                }
                Inline::Inside => {
                    let inner: String = selected.chars().skip(b as usize).take((length - b - a) as usize).collect();
                    buffer.delete(&mut start, &mut end);
                    buffer.insert(&mut buffer.iter_at_offset(at), &inner);
                    self.select_offsets(at, at + length - b - a);
                }
                Inline::Wrap => {
                    buffer.delete(&mut start, &mut end);
                    buffer.insert(&mut buffer.iter_at_offset(at), &format!("{before}{selected}{after}"));
                    self.select_offsets(at + b, at + b + length);
                }
            }
        } else {
            let at = buffer.cursor_position();
            buffer.insert(&mut buffer.iter_at_offset(at), &format!("{before}{placeholder}{after}"));
            self.select_offsets(at + b, at + b + placeholder.chars().count() as i32);
        }
        buffer.end_user_action();
        self.view.grab_focus();
    }

    pub fn insert_link(&self) {
        let buffer = &self.buffer;
        buffer.begin_user_action();
        if let Some((mut start, mut end)) = buffer.selection_bounds() {
            let selected = buffer.text(&start, &end, false).to_string();
            let at = start.offset();
            buffer.delete(&mut start, &mut end);
            buffer.insert(&mut buffer.iter_at_offset(at), &format!("[{selected}](url)"));
            let url = at + selected.chars().count() as i32 + 3;
            self.select_offsets(url, url + 3);
        } else {
            let at = buffer.cursor_position();
            buffer.insert(&mut buffer.iter_at_offset(at), "[link text](url)");
            self.select_offsets(at + 1, at + 10);
        }
        buffer.end_user_action();
        self.view.grab_focus();
    }

    /// Headings, lists, checklists and quotes on every line the selection touches.
    pub fn toggle_prefix(&self, prefix: Prefix) {
        let buffer = &self.buffer;
        let selected = buffer.has_selection();
        let (from, to) = buffer.selection_bounds().unwrap_or_else(|| {
            let at = buffer.iter_at_mark(&buffer.get_insert());
            (at, at)
        });
        let mut start = from;
        start.set_line_offset(0);
        let mut end = to;
        if selected && end.starts_line() && end.offset() > from.offset() {
            end.backward_char();
        }
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        let original = buffer.text(&start, &end, false).to_string();
        let lines: Vec<&str> = original.split('\n').collect();
        let replaced = tx::toggle_lines(&lines, prefix).join("\n");
        if replaced == original {
            return;
        }
        let at = start.offset();
        buffer.begin_user_action();
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut buffer.iter_at_offset(at), &replaced);
        buffer.end_user_action();
        let length = replaced.chars().count() as i32;
        if selected {
            self.select_offsets(at, at + length);
        } else {
            buffer.place_cursor(&buffer.iter_at_offset(at + length));
        }
        self.view.grab_focus();
    }

    /// Insert a block on lines of its own: a fenced code block or a horizontal rule.
    pub fn insert_block(&self, code: bool) {
        let buffer = &self.buffer;
        buffer.begin_user_action();
        let (mut start, mut end) = buffer.selection_bounds().unwrap_or_else(|| {
            let at = buffer.iter_at_mark(&buffer.get_insert());
            (at, at)
        });
        let selected = buffer.text(&start, &end, false).to_string();
        let lead = if start.starts_line() { "" } else { "\n" };
        let at = start.offset();
        buffer.delete(&mut start, &mut end);
        if code {
            let block = format!("{lead}```\n{selected}\n```\n");
            buffer.insert(&mut buffer.iter_at_offset(at), &block);
            let inner = at + lead.len() as i32 + 4;
            self.select_offsets(inner, inner + selected.chars().count() as i32);
        } else {
            buffer.insert(&mut buffer.iter_at_offset(at), &format!("{lead}\n---\n\n"));
            buffer.place_cursor(&buffer.iter_at_offset(at + lead.len() as i32 + 6));
        }
        buffer.end_user_action();
        self.view.grab_focus();
    }

    pub fn insert_text(&self, value: &str) {
        self.buffer.begin_user_action();
        self.buffer.delete_selection(true, true);
        self.buffer.insert_at_cursor(value);
        self.buffer.end_user_action();
        self.view.grab_focus();
    }
}

/// File > Create task from note: the selection, else the whole note, becomes a Backlog task.
pub fn to_task(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    if doc.orphan.get() {
        doc.show_notice(ui, "This note's project was removed, so it has no board to add a task to.", Vec::new());
        return;
    }
    let name = doc.name();
    let selected = doc
        .buffer
        .selection_bounds()
        .map(|(start, end)| doc.buffer.text(&start, &end, false).to_string())
        .filter(|text| !text.trim().is_empty());
    let (title, text) = match selected {
        Some(text) => (tx::task_title(&text).unwrap_or_else(|| name.clone()), text),
        None => (name.clone(), doc.body()),
    };
    let text = text.trim();
    let source = format!("From the note “{name}”.");
    let body = if text.is_empty() { source } else { format!("{text}\n\n{source}") };
    let mut payload = json!({"project_id":doc.project,"title":title,"body":body,"column":"backlog"});
    // The task remembers its note, which lists it under its title from then on.
    if doc.id > 0 {
        payload["note_id"] = json!(doc.id);
    }
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        match ui.call("task.create", payload).await {
            Ok(task) => {
                let id = task["id"].as_i64().unwrap_or(0);
                doc.last_task.set(id);
                doc.show_notice(
                    &ui,
                    &format!("Created task #{id} “{title}” in Backlog."),
                    vec![("Open task", |ui, doc| super::super::task_pages::open(ui, doc.last_task.get()))],
                );
                doc.notice.add_css_class("info");
                load_tasks(&ui, &doc);
            }
            Err(error) => doc.show_notice(&ui, &format!("Could not create a task: {}", readable(&error)), Vec::new()),
        }
    });
}

/// List the tasks made from this note under its title, each a key that opens the task. The
/// strip stays hidden while there are none, or when the engine cannot say.
pub fn load_tasks(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    if doc.id <= 0 || doc.orphan.get() {
        doc.tasks.set_visible(false);
        return;
    }
    let generation = doc.tasks_generation.get() + 1;
    doc.tasks_generation.set(generation);
    let payload = json!({"project_id":doc.project,"note_id":doc.id,"summary":true});
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        let found = ui.call("task.list", payload).await;
        if doc.tasks_generation.get() != generation {
            return;
        }
        let tasks = match found {
            Ok(value) => rows(&value, "tasks"),
            Err(error) => {
                tracing::debug!(%error, "the note's tasks could not be listed");
                Vec::new()
            }
        };
        clear(&doc.tasks);
        doc.tasks.set_visible(!tasks.is_empty());
        if tasks.is_empty() {
            return;
        }
        let caption = label(if tasks.len() == 1 { "Task" } else { "Tasks" }, "notes-tasks-caption");
        caption.set_valign(gtk::Align::Center);
        doc.tasks.append(&caption);
        let chips = gtk::FlowBox::new();
        chips.add_css_class("notes-task-chips");
        chips.set_selection_mode(gtk::SelectionMode::None);
        chips.set_hexpand(true);
        chips.set_max_children_per_line(12);
        chips.set_column_spacing(4);
        chips.set_row_spacing(4);
        for task in &tasks {
            chips.insert(&task_chip(&ui, task), -1);
        }
        doc.tasks.append(&chips);
    });
}

/// One task made from the note: its state, number and title; a click opens it.
fn task_chip(ui: &Rc<Ui>, task: &Value) -> gtk::Button {
    let id = task["id"].as_i64().unwrap_or(0);
    let column = text(task, "column");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    lamp.add_css_class("notes-task-lamp");
    lamp.add_css_class(column);
    lamp.set_valign(gtk::Align::Center);
    content.append(&lamp);
    content.append(&label(&format!("#{id}"), "notes-task-number"));
    let title = label(text(task, "title"), "notes-task-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(36);
    content.append(&title);
    content.append(&label(relay_board::column_title(column), "notes-task-column"));
    let chip = gtk::Button::new();
    chip.add_css_class("notes-task-chip");
    if column == "done" {
        chip.add_css_class("done");
    }
    chip.set_child(Some(&content));
    chip.set_focus_on_click(false);
    chip.set_tooltip_text(Some("Open the task"));
    let weak = Rc::downgrade(ui);
    chip.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            // The task opens on the main window's board, which comes forward for it.
            ui.window.present();
            super::super::task_pages::open(&ui, id);
        }
    });
    chip
}

fn pin_changed(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    if doc.syncing.get() {
        return;
    }
    doc.fresh.set(false);
    let pinned = doc.pin.is_active();
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        match ui.call("notes.pin", json!({"note_id":doc.id,"pinned":pinned})).await {
            Ok(note) => {
                let mut base = doc.draft.base.borrow_mut();
                base["pinned"] = json!(pinned);
                if note["updated_at"].is_string() {
                    base["updated_at"] = note["updated_at"].clone();
                }
            }
            Err(error) => {
                doc.sync_pin();
                doc.show_notice(&ui, &format!("Could not {}: {}", if pinned { "pin" } else { "unpin" }, readable(&error)), Vec::new());
            }
        }
        doc.update_meta();
        super::refresh_notes(&ui);
    });
}

fn bus_code(error: &crate::client::Error) -> &str {
    match error {
        crate::client::Error::Bus(error) => error.code.as_str(),
        _ => "",
    }
}

/// Save through the engine's conflict check. Unlike Draft's own Save it never locks the
/// form, so autosave cannot take the keyboard focus away mid-sentence.
pub fn save(ui: &Rc<Ui>, doc: &Rc<Doc>, then: AfterSave) {
    if doc.deleted.get() {
        save_as(ui, doc);
        return;
    }
    if doc.draft.busy.get() {
        return;
    }
    if let Some(timer) = doc.autosave_timer.borrow_mut().take() {
        timer.remove();
    }
    if !doc.dirty() {
        if let Some(then) = then {
            then(ui, doc);
        }
        return;
    }
    let next = (doc.draft.snapshot)();
    if let Some(message) = oversize(text(&next, "body")) {
        doc.show_notice(ui, &format!("Not saved: {message}"), Vec::new());
        return;
    }
    doc.draft.busy.set(true);
    doc.refresh_state(ui);
    let base = doc.draft.base.borrow().clone();
    let mut payload = json!({
        "note_id": doc.id,
        "body": next["body"],
        "expected": {"title": base["title"], "body": base["body"]},
    });
    if !next["title"].is_null() {
        payload["title"] = next["title"].clone();
    }
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        let result = ui.call("notes.update", payload).await;
        doc.draft.busy.set(false);
        // A close asked for meanwhile waits for this save: done once it lands, and dropped
        // with the notice when it does not.
        let close = doc.close_pending.take();
        match result {
            Ok(note) => {
                let forced = !next["title"].is_null() && note["title"] != next["title"];
                if forced {
                    doc.title.set_text(&tx::clean(text(&note, "title")));
                }
                doc.conflict.set(false);
                doc.seen_remote.borrow_mut().clear();
                doc.set_base(note);
                doc.hide_notice();
                doc.refresh_state(&ui);
                if forced {
                    doc.show_notice(&ui, "This note's title is fixed, so it was kept.", Vec::new());
                }
                if let Some(then) = then {
                    then(&ui, &doc);
                }
                if close && ui.note_tabs.page_num(&doc.draft.layout).is_some() {
                    close_saved(&ui, &doc);
                }
            }
            Err(error) if bus_code(&error) == "notes.edit_conflict" => {
                doc.conflict.set(true);
                doc.show_notice(
                    &ui,
                    "Not saved: this note changed somewhere else since you opened it. Your text is untouched.",
                    vec![
                        ("Save as new note", |ui, doc| save_as(ui, doc)),
                        ("Reload theirs", |ui, doc| reload(ui, doc, true)),
                    ],
                );
            }
            Err(error) if bus_code(&error) == "notes.not_found" => {
                doc.deleted.set(true);
                deleted_notice(&ui, &doc);
            }
            Err(error) => doc.show_notice(
                &ui,
                &format!("Not saved: {}", readable(&error)),
                vec![("Try again", |ui, doc| save(ui, doc, None))],
            ),
        }
        doc.refresh_state(&ui);
    });
}

pub fn deleted_notice(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    doc.show_notice(
        ui,
        if doc.orphan.get() {
            "This note's project was removed. Your text is still here; saving keeps it as a new note in the project shown."
        } else {
            "This note was deleted somewhere else. Your text is still here; saving keeps it as a new note."
        },
        vec![
            ("Save as new note", |ui, doc| save_as(ui, doc)),
            ("Close", |ui, doc| request_close(ui, doc)),
        ],
    );
}

/// Create a new note from the current text and put it in this tab's place.
pub fn save_as(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    if doc.draft.busy.get() {
        return;
    }
    let snapshot = (doc.draft.snapshot)();
    if let Some(message) = oversize(text(&snapshot, "body")) {
        doc.show_notice(ui, &format!("Not saved: {message}"), Vec::new());
        return;
    }
    // A removed project takes no notes: file the text under the project on screen.
    let project = if doc.orphan.get() { super::current_project(ui) } else { doc.project };
    if project == 0 {
        doc.show_notice(ui, "Pick a project in the toolbar to save this text into.", Vec::new());
        return;
    }
    doc.draft.busy.set(true);
    doc.refresh_state(ui);
    // An unchanged title is marked as the copy's; a deleted note's text keeps its own.
    let typed = text(&snapshot, "title");
    let title = if !doc.deleted.get() && snapshot["title"] == doc.draft.base.borrow()["title"] {
        tx::copy_title(typed)
    } else {
        Some(typed.trim().to_string()).filter(|t| !t.is_empty())
    };
    let mut payload = json!({"project_id":project,"body":snapshot["body"],"pinned":false});
    if let Some(title) = title {
        payload["title"] = json!(title);
    }
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        let result = ui.call("notes.create", payload).await;
        doc.draft.busy.set(false);
        match result {
            Ok(note) => {
                let position = ui.note_tabs.page_num(&doc.draft.layout);
                let close = doc.close_pending.take();
                // The editor stayed open during the round trip: keep what was typed meanwhile
                // (over the stored copy, so it shows as modified) and the cursor with it.
                let current = (doc.draft.snapshot)();
                let cursor = doc.buffer.cursor_position();
                discard_close(&ui, &doc);
                let fresh = super::open_doc(&ui, note, position);
                if current["title"] != snapshot["title"] {
                    fresh.title.set_text(text(&current, "title"));
                }
                if current["body"] != snapshot["body"] {
                    fresh.buffer.begin_irreversible_action();
                    fresh.buffer.set_text(text(&current, "body"));
                    fresh.buffer.end_irreversible_action();
                }
                fresh.buffer.place_cursor(&fresh.buffer.iter_at_offset(cursor.min(fresh.buffer.char_count())));
                fresh.view.grab_focus();
                super::refresh_notes(&ui);
                // Closed while it was saved: the copy's tab closes in its place.
                if close {
                    request_close(&ui, &fresh);
                }
            }
            Err(error) => {
                doc.close_pending.set(false);
                doc.show_notice(&ui, &format!("Could not save a copy: {}", readable(&error)), Vec::new());
                doc.refresh_state(&ui);
            }
        }
    });
}

/// Fetch the stored note and replace the text, asking first when there are changes.
pub fn reload(ui: &Rc<Ui>, doc: &Rc<Doc>, ask: bool) {
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        if ask && doc.dirty() {
            let dialog = gtk::AlertDialog::builder()
                .message(format!("Reload “{}”?", doc.name()))
                .detail("Your unsaved changes to this note will be lost.")
                .buttons(["Cancel", "Reload"])
                .cancel_button(0)
                .default_button(0)
                .modal(true)
                .build();
            if dialog.choose_future(super::window_of(&ui).as_ref()).await.ok() != Some(1) {
                return;
            }
        }
        match ui.call("notes.get", json!({"note_id":doc.id})).await {
            Ok(note) => {
                doc.load(note);
                doc.refresh_state(&ui);
            }
            Err(error) if bus_code(&error) == "notes.not_found" => {
                doc.deleted.set(true);
                deleted_notice(&ui, &doc);
                doc.refresh_state(&ui);
            }
            Err(error) => doc.show_notice(&ui, &format!("Could not reload: {}", readable(&error)), Vec::new()),
        }
    });
}

/// Close without saving: the draft's base takes the current text, so Draft lets go.
pub fn discard_close(_ui: &Rc<Ui>, doc: &Rc<Doc>) {
    if doc.draft.busy.get() {
        return;
    }
    let snapshot = (doc.draft.snapshot)();
    {
        let mut base = doc.draft.base.borrow_mut();
        if let Some(fields) = snapshot.as_object() {
            for (key, value) in fields {
                base[key] = value.clone();
            }
        }
    }
    doc.draft.close();
}

/// Close after a save, unless text was typed while it was in flight. Draft's own refusal
/// goes to a status label this page took off screen, so the notice bar says it instead.
fn close_saved(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    if doc.draft.busy.get() || doc.dirty() {
        doc.show_notice(
            ui,
            "Not closed: this note changed while it was being saved.",
            vec![
                ("Save and close", |ui, doc| save(ui, doc, Some(Box::new(close_saved)))),
                ("Discard and close", discard_close),
            ],
        );
        return;
    }
    doc.draft.close();
}

/// Ctrl+W, the tab's close button and middle-click.
pub fn request_close(ui: &Rc<Ui>, doc: &Rc<Doc>) {
    // Common with autosave on: the tab closes once the save in flight lands.
    if doc.draft.busy.get() {
        doc.close_pending.set(true);
        doc.flash("Closing once saved");
        return;
    }
    // A pasted picture still being written goes in first, so it is not lost with the tab.
    if doc.inline.writing.get() > 0 {
        doc.close_pending.set(true);
        doc.flash("Closing once the picture is kept");
        return;
    }
    if !doc.dirty() {
        doc.draft.close();
        return;
    }
    if super::prefs().autosave && !doc.deleted.get() && !doc.conflict.get() {
        save(ui, doc, Some(Box::new(close_saved)));
        return;
    }
    let (ui, doc) = (ui.clone(), doc.clone());
    glib::spawn_future_local(async move {
        let dialog = gtk::AlertDialog::builder()
            .message(format!("Save changes to “{}” before closing?", doc.name()))
            .detail("If you don't save, your changes will be lost.")
            .buttons(["Cancel", "Don't Save", "Save"])
            .cancel_button(0)
            .default_button(2)
            .modal(true)
            .build();
        match dialog.choose_future(super::window_of(&ui).as_ref()).await {
            Ok(1) => discard_close(&ui, &doc),
            Ok(2) => {
                if doc.deleted.get() {
                    save_as(&ui, &doc);
                } else {
                    save(&ui, &doc, Some(Box::new(close_saved)));
                }
            }
            _ => {}
        }
    });
}

/// A refresh brought this note's stored copy (`None`: it is gone).
pub fn reconcile(ui: &Rc<Ui>, doc: &Rc<Doc>, latest: Option<&Value>) {
    if doc.draft.busy.get() {
        return;
    }
    let Some(note) = latest else {
        if !doc.deleted.replace(true) {
            deleted_notice(ui, doc);
            doc.refresh_state(ui);
        }
        return;
    };
    if doc.deleted.replace(false) {
        doc.hide_notice();
    }
    // An older copy than the one the tab holds (a read that crossed a save) says nothing new:
    // loaded, it would put back old text and wipe undo. Stamps are one width and sort in time.
    let stamp_of = |value: &Value| value["updated_at"].as_str().unwrap_or("").to_string();
    let held = stamp_of(&doc.draft.base.borrow());
    if !held.is_empty() && stamp_of(note) < held {
        return;
    }
    let same = {
        let base = doc.draft.base.borrow();
        note["title"] == base["title"] && note["body"] == base["body"]
    };
    if same {
        {
            let mut base = doc.draft.base.borrow_mut();
            base["pinned"] = note["pinned"].clone();
            base["updated_at"] = note["updated_at"].clone();
        }
        doc.sync_pin();
        doc.update_meta();
        doc.refresh_state(ui);
        return;
    }
    if !doc.dirty() {
        doc.load(note.clone());
        doc.refresh_state(ui);
        return;
    }
    // The text on screen is what is stored: this tab's own save, whose answer was lost to a
    // disconnect. It is adopted as saved, not reported as a change from somewhere else.
    let shown = (doc.draft.snapshot)();
    if note["title"] == shown["title"] && note["body"] == shown["body"] {
        *doc.stored_title.borrow_mut() = note["title"].clone();
        *doc.draft.base.borrow_mut() = note.clone();
        doc.sync_pin();
        doc.update_meta();
        doc.refresh_state(ui);
        return;
    }
    let stamp = text(note, "updated_at").to_string();
    if *doc.seen_remote.borrow() != stamp {
        *doc.seen_remote.borrow_mut() = stamp;
        doc.show_notice(
            ui,
            "This note changed somewhere else while you were editing. Your text is untouched.",
            vec![
                ("Reload theirs", |ui, doc| reload(ui, doc, true)),
                ("Save as new note", |ui, doc| save_as(ui, doc)),
            ],
        );
    }
}
