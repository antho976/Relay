//! The find and replace bar Notes and Code share, in the KWrite mould: match case, whole words
//! and regular expressions, "3 of 12", Previous and Next (Shift+F3 and F3, which work with the
//! bar closed too, from the last search and its options) and a Replace row Ctrl+H shows.
//! GtkSourceView's search finds; each owner places the bar, names it and binds its own
//! shortcuts to `open`, `step` and `close`.
use crate::app::{button, label};
use gtk4 as gtk;
use sourceview5::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread::LocalKey;

/// The last search a bar ran: its query and its three options, which go together to the next
/// bar that starts from it.
pub struct Memory {
    query: String,
    case: bool,
    word: bool,
    regex: bool,
}
impl Memory {
    pub const fn new() -> Self {
        Self { query: String::new(), case: false, word: false, regex: false }
    }
}

pub struct FindBar {
    pub bar: gtk::Grid,
    pub query: gtk::Entry,
    replacement: gtk::Entry,
    count: gtk::Label,
    replace_row: gtk::Box,
    case: gtk::ToggleButton,
    word: gtk::ToggleButton,
    regex: gtk::ToggleButton,
    view: sourceview5::View,
    buffer: sourceview5::Buffer,
    search: sourceview5::SearchContext,
    /// A result ("Replaced 3") the count shows until the next search.
    note: RefCell<Option<String>>,
    /// The last search, which an empty bar starts from: one for every note, Code's its own.
    memory: &'static LocalKey<RefCell<Memory>>,
}

fn small_key(icon: &str, tip: &str) -> gtk::Button {
    let key = gtk::Button::new();
    key.set_child(Some(&crate::icons::image_with_stroke(icon, 14, 1.5)));
    key.add_css_class("find-key");
    key.set_focus_on_click(false);
    key.set_tooltip_text(Some(tip));
    key.update_property(&[gtk::accessible::Property::Label(tip)]);
    key
}

fn option_key(caption: &str, tip: &str) -> gtk::ToggleButton {
    let key = gtk::ToggleButton::with_label(caption);
    key.add_css_class("find-option");
    key.set_focus_on_click(false);
    key.set_tooltip_text(Some(tip));
    key
}

/// `f` on the bar while it lives, for a signal handler.
fn with(bar: &Rc<FindBar>, f: fn(&FindBar)) -> impl Fn() + 'static {
    let weak = Rc::downgrade(bar);
    move || {
        if let Some(bar) = weak.upgrade() {
            f(&bar);
        }
    }
}

impl FindBar {
    /// A hidden bar over `view`'s `buffer`. `name` prefixes the query field's and the count's
    /// widget names (`notes-find-entry`, `notes-find-count`).
    pub fn new(
        view: &sourceview5::View,
        buffer: &sourceview5::Buffer,
        name: &str,
        placeholder: &str,
        memory: &'static LocalKey<RefCell<Memory>>,
    ) -> Rc<Self> {
        let settings = sourceview5::SearchSettings::new();
        settings.set_wrap_around(true);
        let search = sourceview5::SearchContext::new(buffer, Some(&settings));
        search.set_highlight(false);
        let bar = gtk::Grid::new();
        bar.set_column_spacing(4);
        bar.set_row_spacing(5);
        bar.set_visible(false);
        let query = gtk::Entry::builder().placeholder_text(placeholder).hexpand(true).build();
        query.add_css_class("find-entry");
        query.set_widget_name(&format!("{name}-find-entry"));
        let count = label("", "find-count");
        count.set_width_chars(11);
        count.set_widget_name(&format!("{name}-find-count"));
        count.set_xalign(1.);
        let previous = small_key("chevron-up", "Previous match (Shift+F3)");
        let next = small_key("chevron-down", "Next match (F3)");
        let case = option_key("Aa", "Match case");
        let word = option_key("W", "Whole words only");
        word.add_css_class("find-word");
        let regex = option_key(".*", "Regular expression");
        let close = small_key("close", "Close (Esc)");
        bar.attach(&query, 0, 0, 1, 1);
        bar.attach(&count, 1, 0, 1, 1);
        bar.attach(&previous, 2, 0, 1, 1);
        bar.attach(&next, 3, 0, 1, 1);
        bar.attach(&case, 4, 0, 1, 1);
        bar.attach(&word, 5, 0, 1, 1);
        bar.attach(&regex, 6, 0, 1, 1);
        bar.attach(&close, 7, 0, 1, 1);
        let replacement = gtk::Entry::builder()
            .placeholder_text("Replace with")
            .hexpand(true)
            .build();
        replacement.add_css_class("find-entry");
        let replace_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let replace_one = button("Replace", "find-button");
        let replace_all = button("Replace all", "find-button");
        replace_one.set_focus_on_click(false);
        replace_all.set_focus_on_click(false);
        replace_row.append(&replace_one);
        replace_row.append(&replace_all);
        bar.attach(&replacement, 0, 1, 1, 1);
        bar.attach(&replace_row, 1, 1, 7, 1);

        let find = Rc::new(Self {
            bar,
            query,
            replacement,
            count,
            replace_row,
            case: case.clone(),
            word: word.clone(),
            regex: regex.clone(),
            view: view.clone(),
            buffer: buffer.clone(),
            search,
            note: RefCell::new(None),
            memory,
        });
        let incremental = with(&find, Self::incremental);
        find.query.connect_changed(move |_| incremental());
        let forward = with(&find, |find| find.step(false));
        find.query.connect_activate(move |_| forward());
        let keys = gtk::EventControllerKey::new();
        let backward = with(&find, |find| find.step(true));
        keys.connect_key_pressed(move |_, key, _, mods| {
            if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter)
                && mods.contains(gtk::gdk::ModifierType::SHIFT_MASK)
            {
                backward();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        find.query.add_controller(keys);
        let step = with(&find, |find| find.step(true));
        previous.connect_clicked(move |_| step());
        let step = with(&find, |find| find.step(false));
        next.connect_clicked(move |_| step());
        let shut = with(&find, Self::close);
        close.connect_clicked(move |_| shut());
        for (key, apply) in [
            (&case, (|s: &sourceview5::SearchSettings, on| s.set_case_sensitive(on)) as fn(&sourceview5::SearchSettings, bool)),
            (&word, |s, on| s.set_at_word_boundaries(on)),
            (&regex, |s, on| s.set_regex_enabled(on)),
        ] {
            let again = with(&find, Self::incremental);
            let settings = settings.clone();
            key.connect_toggled(move |key| {
                apply(&settings, key.is_active());
                again();
            });
        }
        let one = with(&find, Self::replace_one);
        replace_one.connect_clicked(move |_| one());
        let one = with(&find, Self::replace_one);
        find.replacement.connect_activate(move |_| one());
        let all = with(&find, Self::replace_all);
        replace_all.connect_clicked(move |_| all());
        let count = with(&find, Self::update_count);
        find.search.connect_occurrences_count_notify(move |_| count());
        let count = with(&find, Self::update_count);
        find.search.connect_regex_error_notify(move |_| count());
        // "3 of 12" follows the caret while the bar shows.
        let count = with(&find, |find| {
            if find.visible() {
                find.update_count();
            }
        });
        find.buffer.connect_cursor_position_notify(move |_| count());
        find
    }

    /// Shows the bar (with the Replace row for Ctrl+H), seeded from a one-line selection or the
    /// last query.
    pub fn open(&self, replace: bool) {
        let query = &self.query;
        if let Some((start, end)) = self.buffer.selection_bounds() {
            let selected = self.buffer.text(&start, &end, false);
            if !selected.contains('\n') && selected.chars().count() <= 200 {
                query.set_text(&selected);
            }
        } else if query.text().is_empty() {
            self.recall();
        }
        self.replacement.set_visible(replace);
        self.replace_row.set_visible(replace);
        self.bar.set_visible(true);
        self.search.set_highlight(true);
        self.sync_query();
        if replace && !query.text().is_empty() {
            self.replacement.grab_focus();
        } else {
            query.grab_focus();
            query.select_region(0, -1);
        }
        self.update_count();
    }

    /// Hides the bar and its highlights, and gives the text back the keyboard.
    pub fn close(&self) {
        self.hide();
        self.view.grab_focus();
    }

    /// `close` without moving focus: for a view switched away from under the bar.
    pub fn hide(&self) {
        self.bar.set_visible(false);
        self.search.set_highlight(false);
    }

    pub fn visible(&self) -> bool {
        self.bar.is_visible()
    }

    /// Take up the last search: its options first, so the query is searched with them.
    fn recall(&self) {
        let (query, case, word, regex) = self.memory.with(|m| {
            let m = m.borrow();
            (m.query.clone(), m.case, m.word, m.regex)
        });
        for (key, on) in [(&self.case, case), (&self.word, word), (&self.regex, regex)] {
            if key.is_active() != on {
                key.set_active(on);
            }
        }
        self.query.set_text(&query);
    }

    fn sync_query(&self) {
        self.note.borrow_mut().take();
        let value = self.query.text();
        let settings = self.search.settings();
        if value.is_empty() {
            settings.set_search_text(None);
        } else {
            settings.set_search_text(Some(&value));
            self.memory.with(|m| {
                let mut m = m.borrow_mut();
                value.as_str().clone_into(&mut m.query);
                (m.case, m.word, m.regex) = (self.case.is_active(), self.word.is_active(), self.regex.is_active());
            });
        }
    }

    fn incremental(&self) {
        self.sync_query();
        if !self.query.text().is_empty() {
            let from = self
                .buffer
                .selection_bounds()
                .map(|(start, _)| start)
                .unwrap_or_else(|| self.buffer.iter_at_mark(&self.buffer.get_insert()));
            self.select_match(self.search.forward(&from));
        }
        self.update_count();
    }

    /// F3 / Shift+F3 and the bar's arrows. Works with the bar closed, using the last query.
    pub fn step(&self, backward: bool) {
        // Taken first: filling in the last query selects its first match (incremental), and
        // stepping on from that selection would skip it.
        let (start, end) = self.buffer.selection_bounds().unwrap_or_else(|| {
            let at = self.buffer.iter_at_mark(&self.buffer.get_insert());
            (at, at)
        });
        if self.query.text().is_empty() {
            self.recall();
        }
        self.sync_query();
        if self.query.text().is_empty() {
            self.open(false);
            return;
        }
        let found = if backward { self.search.backward(&start) } else { self.search.forward(&end) };
        self.select_match(found);
        self.update_count();
    }

    fn select_match(&self, found: Option<(gtk::TextIter, gtk::TextIter, bool)>) {
        let Some((start, end, _)) = found else {
            return;
        };
        self.buffer.select_range(&start, &end);
        self.view.scroll_to_mark(&self.buffer.get_insert(), 0.15, false, 0.0, 0.0);
    }

    fn update_count(&self) {
        let query = &self.query;
        let count = &self.count;
        if let Some(note) = self.note.borrow().as_deref() {
            count.set_text(note);
            return;
        }
        query.remove_css_class("error");
        if query.text().is_empty() {
            count.set_text("");
            return;
        }
        if self.search.regex_error().is_some() {
            count.set_text("Invalid pattern");
            query.add_css_class("error");
            return;
        }
        let total = self.search.occurrences_count();
        if total < 0 {
            count.set_text("…");
        } else if total == 0 {
            count.set_text("No results");
            query.add_css_class("error");
        } else {
            let current = self
                .buffer
                .selection_bounds()
                .map(|(start, end)| self.search.occurrence_position(&start, &end))
                .unwrap_or(0);
            count.set_text(&if current > 0 {
                format!("{current} of {total}")
            } else {
                format!("{total} found")
            });
        }
    }

    /// A read-only text (Code's comparisons, a file still loading) is searched, never replaced.
    fn replace_one(&self) {
        if !self.view.is_editable() {
            return;
        }
        self.sync_query();
        if let Some((mut start, mut end)) = self.buffer.selection_bounds() {
            if self.search.occurrence_position(&start, &end) > 0 {
                if let Err(error) = self.search.replace(&mut start, &mut end, &self.replacement.text()) {
                    self.count.set_text(&error.to_string());
                    return;
                }
                self.buffer.place_cursor(&end);
            }
        }
        self.step(false);
    }

    fn replace_all(&self) {
        if !self.view.is_editable() || self.query.text().is_empty() {
            return;
        }
        self.sync_query();
        // Native search keeps undo grouping and UTF-8 text positions.
        let note = match crate::editor::replace_all(&self.search, &self.replacement.text()) {
            Ok(count) => format!("Replaced {count}"),
            Err(error) => error.to_string(),
        };
        *self.note.borrow_mut() = Some(note);
        self.update_count();
    }
}
