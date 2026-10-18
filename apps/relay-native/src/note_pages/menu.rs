//! The Notes window's "notes.*" actions, its File/Edit/Search/View menu bar and the
//! keyboard shortcuts behind them. Menus show accelerators; the window's capture-phase
//! key controller dispatches them, leaving the text widgets' own Ctrl+Z/X/C/V/A alone.
use super::doc::{self, Doc};
use super::text::Prefix;
use super::*;
use gtk::gdk::{Key, ModifierType};
use gtk::gio;

thread_local! {
    static GROUP: RefCell<Option<gio::SimpleActionGroup>> = const { RefCell::new(None) };
    static LAST_TEXT: RefCell<glib::WeakRef<gtk::Widget>> = RefCell::new(glib::WeakRef::new());
}

type Toggle = (&'static str, fn(&Prefs) -> bool, fn(&mut Prefs, bool));

fn toggles() -> [Toggle; 8] {
    [
        ("autosave", |p| p.autosave, |p, v| p.autosave = v),
        ("wrap", |p| p.wrap, |p, v| p.wrap = v),
        ("line-numbers", |p| p.line_numbers, |p, v| p.line_numbers = v),
        ("current-line", |p| p.current_line, |p, v| p.current_line = v),
        ("toolbar", |p| p.toolbar, |p, v| p.toolbar = v),
        ("toolbar-labels", |p| p.toolbar_labels, |p, v| p.toolbar_labels = v),
        ("monospace", |p| p.monospace, |p, v| p.monospace = v),
        ("markdown", |p| p.markdown, |p, v| p.markdown = v),
    ]
}

/// Actions that need an open note.
const DOC_ACTIONS: &[&str] = &[
    "save", "save-as", "reload", "rename", "pin", "duplicate", "delete", "close-tab", "undo", "redo",
    "cut", "copy", "paste", "select-all", "datetime", "bold", "italic", "h1", "h2", "h3", "bullets",
    "numbers", "checklist", "quote", "code", "code-block", "link", "rule", "find", "replace",
    "find-next", "find-prev", "goto", "close-find", "next-tab", "prev-tab", "image", "to-task",
    "toggle-check", "copy-note",
];

fn group() -> Option<gio::SimpleActionGroup> {
    GROUP.with(|g| g.borrow().clone())
}

pub fn activate(name: &str) {
    if let Some(group) = group() {
        group.activate_action(name, None);
    }
}

pub fn lookup(name: &str) -> Option<gio::SimpleAction> {
    group()?.lookup_action(name).and_downcast::<gio::SimpleAction>()
}

/// Enable note actions for `doc`, and Save only while there is something to save.
pub fn sync(doc: Option<&Rc<Doc>>) {
    for name in DOC_ACTIONS {
        if let Some(action) = lookup(name) {
            action.set_enabled(doc.is_some());
        }
    }
    if let (Some(action), Some(doc)) = (lookup("save"), doc) {
        action.set_enabled(doc.dirty() || doc.deleted.get());
    }
    // File › Pin to top carries a check mark while the note is pinned.
    if let Some(action) = lookup("pin") {
        let pinned = doc.is_some_and(|doc| doc.pin.is_active());
        if action.state().and_then(|s| s.get::<bool>()) != Some(pinned) {
            action.set_state(&pinned.to_variant());
        }
    }
}

/// Push restored preferences into the toggle actions' check marks.
pub fn sync_prefs(prefs: &Prefs, sidebar: bool) {
    for (name, get, _) in toggles() {
        if let Some(action) = lookup(name) {
            action.set_state(&get(prefs).to_variant());
        }
    }
    if let Some(action) = lookup("sort") {
        action.set_state(&(if prefs.sort_title { "title" } else { "modified" }).to_variant());
    }
    if let Some(action) = lookup("sidebar") {
        action.set_state(&sidebar.to_variant());
    }
}

fn item(caption: &str, action: &str, accel: Option<&str>) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(caption), Some(&format!("notes.{action}")));
    if let Some(accel) = accel {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
    item
}

fn targeted(caption: &str, action: &str, target: glib::Variant) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(caption), None);
    item.set_action_and_target_value(Some(&format!("notes.{action}")), Some(&target));
    item
}

/// `targeted`, showing the key that does it on a focused row.
fn targeted_key(caption: &str, action: &str, target: glib::Variant, accel: &str) -> gio::MenuItem {
    let item = targeted(caption, action, target);
    item.set_attribute_value("accel", Some(&accel.to_variant()));
    item
}

fn section(items: &[gio::MenuItem]) -> gio::Menu {
    let menu = gio::Menu::new();
    for item in items {
        menu.append_item(item);
    }
    menu
}

fn menu(sections: &[gio::Menu]) -> gio::Menu {
    let menu = gio::Menu::new();
    for part in sections {
        menu.append_section(None, part);
    }
    menu
}

pub fn headings_menu() -> gio::Menu {
    section(&[
        item("Heading 1", "h1", None),
        item("Heading 2", "h2", None),
        item("Heading 3", "h3", None),
    ])
}

pub fn sort_menu() -> gio::Menu {
    let sort = section(&[
        targeted("Date modified", "sort", "modified".to_variant()),
        targeted("Title", "sort", "title".to_variant()),
    ]);
    let menu = gio::Menu::new();
    menu.append_section(Some("Sort by"), &sort);
    menu
}

fn format_menu() -> gio::Menu {
    menu(&[
        section(&[
            item("Bold", "bold", Some("<Control>b")),
            item("Italic", "italic", Some("<Control>i")),
            item("Inline code", "code", None),
            item("Link", "link", Some("<Control>k")),
        ]),
        headings_menu(),
        section(&[
            item("Bulleted list", "bullets", None),
            item("Numbered list", "numbers", None),
            item("Checklist", "checklist", None),
            item("Quote", "quote", None),
        ]),
        section(&[item("Code block", "code-block", None), item("Horizontal rule", "rule", None)]),
    ])
}

fn model() -> gio::Menu {
    let bar = gio::Menu::new();
    bar.append_submenu(
        Some("_File"),
        &menu(&[
            section(&[
                item("New note", "new", Some("<Control>n")),
                item("Open…", "open", Some("<Control>o")),
            ]),
            section(&[
                item("Save", "save", Some("<Control>s")),
                item("Save as new note", "save-as", Some("<Control><Shift>s")),
                item("Reload", "reload", None),
            ]),
            section(&[
                item("Rename…", "rename", Some("F2")),
                item("Pin to top", "pin", None),
                item("Duplicate", "duplicate", None),
                item("Copy note text", "copy-note", None),
                item("Create task from note", "to-task", Some("<Control><Shift>t")),
            ]),
            section(&[item("Delete…", "delete", None)]),
            section(&[item("Save automatically", "autosave", None)]),
            section(&[
                item("Close tab", "close-tab", Some("<Control>w")),
                item("Close window", "close-window", None),
            ]),
        ]),
    );
    let edit = menu(&[
        section(&[
            item("Undo", "undo", Some("<Control>z")),
            item("Redo", "redo", Some("<Control><Shift>z")),
        ]),
        section(&[
            item("Cut", "cut", Some("<Control>x")),
            item("Copy", "copy", Some("<Control>c")),
            item("Paste", "paste", Some("<Control>v")),
            item("Select all", "select-all", Some("<Control>a")),
        ]),
        section(&[
            item("Insert image…", "image", None),
            item("Insert date and time", "datetime", Some("F5")),
        ]),
        section(&[item("Tick or untick item", "toggle-check", Some("<Control>Return"))]),
    ]);
    edit.append_submenu(Some("Format"), &format_menu());
    bar.append_submenu(Some("_Edit"), &edit);
    bar.append_submenu(
        Some("_Search"),
        &menu(&[
            section(&[
                item("Find…", "find", Some("<Control>f")),
                item("Replace…", "replace", Some("<Control>h")),
            ]),
            section(&[
                item("Find next", "find-next", Some("F3")),
                item("Find previous", "find-prev", Some("<Shift>F3")),
            ]),
            section(&[item("Go to line…", "goto", Some("<Control>g"))]),
        ]),
    );
    let view = menu(&[
        section(&[
            item("Toolbar", "toolbar", None),
            item("Toolbar labels", "toolbar-labels", None),
            item("Customize toolbar…", "customize-toolbar", None),
            item("Library", "sidebar", Some("F9")),
        ]),
        section(&[
            item("Line numbers", "line-numbers", None),
            item("Highlight current line", "current-line", None),
            item("Word wrap", "wrap", Some("<Alt>z")),
            item("Monospace font", "monospace", None),
            item("Markdown highlighting", "markdown", None),
        ]),
        section(&[
            item("Zoom in", "zoom-in", Some("<Control>plus")),
            item("Zoom out", "zoom-out", Some("<Control>minus")),
            item("Reset zoom", "zoom-reset", Some("<Control>0")),
        ]),
        section(&[
            item("Next tab", "next-tab", Some("<Control>Tab")),
            item("Previous tab", "prev-tab", Some("<Control><Shift>Tab")),
        ]),
    ]);
    view.append_section(None, &sort_menu());
    bar.append_submenu(Some("_View"), &view);
    bar
}

/// The widget the Edit menu acts on: the text field that last had focus, else the note.
fn text_target(ui: &Rc<Ui>) -> Option<gtk::Widget> {
    let recent = LAST_TEXT.with(|w| w.borrow().upgrade()).filter(|w| w.is_mapped());
    recent.or_else(|| super::current_doc(ui).map(|doc| doc.view.clone().upcast()))
}

fn edit_action(ui: &Rc<Ui>, name: &str) {
    if let Some(widget) = text_target(ui) {
        widget.grab_focus();
        let _ = widget.activate_action(name, None);
    }
}

fn with_doc(ui: &Rc<Ui>, f: impl FnOnce(&Rc<Ui>, &Rc<Doc>)) {
    if let Some(doc) = super::current_doc(ui) {
        f(ui, &doc);
    }
}

/// Install the action group, the shortcut controller and focus tracking on `window`, and
/// return its menu bar.
pub fn install(ui: &Rc<Ui>, window: &gtk::Window) -> gtk::PopoverMenuBar {
    let group = gio::SimpleActionGroup::new();
    let weak = Rc::downgrade(ui);
    let add = |name: &str, f: fn(&Rc<Ui>)| {
        let action = gio::SimpleAction::new(name, None);
        let weak = weak.clone();
        action.connect_activate(move |_, _| {
            if let Some(ui) = weak.upgrade() {
                f(&ui)
            }
        });
        group.add_action(&action);
    };
    add("new", |ui| super::new_note(ui, None, String::new()));
    add("open", super::focus_library);
    add("save", |ui| with_doc(ui, |ui, d| doc::save(ui, d, None)));
    add("save-as", |ui| with_doc(ui, doc::save_as));
    add("reload", |ui| with_doc(ui, |ui, d| doc::reload(ui, d, true)));
    add("rename", |ui| {
        with_doc(ui, |_, d| {
            d.title.grab_focus();
            d.title.select_region(0, -1);
        })
    });
    add("duplicate", |ui| with_doc(ui, |ui, d| super::new_note(ui, super::text::copy_title(&d.title.text()), d.body())));
    add("delete", |ui| with_doc(ui, |ui, d| super::confirm_delete(ui, d.id, d.name())));
    add("close-tab", |ui| with_doc(ui, doc::request_close));
    add("close-window", |ui| {
        if let Some(window) = super::window_of(ui) {
            window.close();
        }
    });
    add("undo", |ui| edit_action(ui, "text.undo"));
    add("redo", |ui| edit_action(ui, "text.redo"));
    add("cut", |ui| edit_action(ui, "clipboard.cut"));
    add("copy", |ui| edit_action(ui, "clipboard.copy"));
    add("paste", |ui| edit_action(ui, "clipboard.paste"));
    add("select-all", |ui| edit_action(ui, "selection.select-all"));
    add("datetime", |ui| {
        with_doc(ui, |_, d| {
            if let Ok(stamp) = glib::DateTime::now_local().and_then(|now| now.format("%Y-%m-%d %H:%M")) {
                d.insert_text(&stamp);
            }
        })
    });
    add("bold", |ui| with_doc(ui, |_, d| d.wrap_inline("**", "**", "bold text")));
    add("italic", |ui| with_doc(ui, |_, d| d.wrap_inline("*", "*", "italic text")));
    add("code", |ui| with_doc(ui, |_, d| d.wrap_inline("`", "`", "code")));
    add("link", |ui| with_doc(ui, |_, d| d.insert_link()));
    add("h1", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Heading(1))));
    add("h2", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Heading(2))));
    add("h3", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Heading(3))));
    add("bullets", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Bullet)));
    add("numbers", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Number)));
    add("checklist", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Check)));
    add("quote", |ui| with_doc(ui, |_, d| d.toggle_prefix(Prefix::Quote)));
    add("code-block", |ui| with_doc(ui, |_, d| d.insert_block(true)));
    add("rule", |ui| with_doc(ui, |_, d| d.insert_block(false)));
    add("find", |ui| with_doc(ui, |_, d| d.open_find(false)));
    add("replace", |ui| with_doc(ui, |_, d| d.open_find(true)));
    add("find-next", |ui| with_doc(ui, |_, d| d.find_step(false)));
    add("find-prev", |ui| with_doc(ui, |_, d| d.find_step(true)));
    add("close-find", |ui| with_doc(ui, |_, d| d.close_find()));
    add("goto", |ui| with_doc(ui, |_, d| d.position.popup()));
    add("image", |ui| with_doc(ui, super::inline::choose_image));
    add("to-task", |ui| with_doc(ui, doc::to_task));
    add("toggle-check", |ui| with_doc(ui, |_, d| d.toggle_check_at_cursor()));
    add("copy-note", |ui| {
        with_doc(ui, |_, d| {
            d.view.clipboard().set_text(&d.body());
            d.flash("Note text copied");
        })
    });
    add("customize-toolbar", |ui| super::customize_toolbar(ui, None));
    add("zoom-in", |ui| super::zoom(ui, 1));
    add("zoom-out", |ui| super::zoom(ui, -1));
    add("zoom-reset", |ui| super::zoom(ui, 0));
    add("next-tab", |ui| super::cycle_tab(ui, 1));
    add("prev-tab", |ui| super::cycle_tab(ui, -1));

    let prefs = super::prefs();
    for (name, get, set) in toggles() {
        let action = gio::SimpleAction::new_stateful(name, None, &get(&prefs).to_variant());
        let weak = weak.clone();
        action.connect_activate(move |action, _| {
            let next = !action.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
            action.set_state(&next.to_variant());
            if let Some(ui) = weak.upgrade() {
                let mut prefs = super::prefs();
                set(&mut prefs, next);
                super::set_prefs(&ui, prefs);
            }
        });
        group.add_action(&action);
    }
    // Stateful, so the menu shows a check mark on a pinned note; `sync` keeps it with the note.
    let pin = gio::SimpleAction::new_stateful("pin", None, &false.to_variant());
    let target = weak.clone();
    pin.connect_activate(move |_, _| {
        if let Some(ui) = target.upgrade() {
            with_doc(&ui, |_, d| super::toggle_pin(d));
        }
    });
    group.add_action(&pin);
    let sidebar = gio::SimpleAction::new_stateful("sidebar", None, &true.to_variant());
    let target = weak.clone();
    sidebar.connect_activate(move |action, _| {
        let next = !action.state().and_then(|s| s.get::<bool>()).unwrap_or(true);
        if let Some(ui) = target.upgrade() {
            super::set_sidebar(&ui, next);
        }
    });
    group.add_action(&sidebar);
    let sort = gio::SimpleAction::new_stateful(
        "sort",
        Some(glib::VariantTy::STRING),
        &(if prefs.sort_title { "title" } else { "modified" }).to_variant(),
    );
    let target = weak.clone();
    sort.connect_activate(move |action, value| {
        let Some(value) = value else { return };
        action.set_state(value);
        if let Some(ui) = target.upgrade() {
            let mut prefs = super::prefs();
            prefs.sort_title = value.str() == Some("title");
            super::set_prefs(&ui, prefs);
        }
    });
    group.add_action(&sort);
    let row = |name: &str, f: fn(&Rc<Ui>, i64)| {
        let action = gio::SimpleAction::new(name, Some(glib::VariantTy::INT64));
        let weak = weak.clone();
        action.connect_activate(move |_, value| {
            if let (Some(ui), Some(id)) = (weak.upgrade(), value.and_then(|v| v.get::<i64>())) {
                f(&ui, id)
            }
        });
        group.add_action(&action);
    };
    row("row-open", |ui, id| super::open_id(ui, id, false));
    row("row-rename", |ui, id| super::open_id(ui, id, true));
    row("row-pin", super::pin_id);
    row("row-duplicate", super::duplicate_id);
    row("row-delete", super::delete_id);
    row("row-task", super::task_id);
    row("row-copy", super::copy_id);

    window.insert_action_group("notes", Some(&group));
    GROUP.with(|g| *g.borrow_mut() = Some(group));

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let target = weak.clone();
    keys.connect_key_pressed(move |_, key, keycode, mods| match target.upgrade() {
        Some(ui) => shortcut(&ui, crate::shortcuts::latin(key, keycode), mods),
        None => glib::Propagation::Proceed,
    });
    window.add_controller(keys);
    window.connect_focus_widget_notify(|window| {
        if let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) {
            if focus.is::<gtk::Text>() || focus.is::<gtk::TextView>() {
                LAST_TEXT.with(|w| w.borrow().set(Some(&focus)));
            }
        }
    });
    gtk::PopoverMenuBar::from_model(Some(&model()))
}

/// The row context menu for note `id`.
pub fn row_menu(id: i64, pinned: bool) -> gio::Menu {
    let target = id.to_variant();
    menu(&[
        section(&[
            targeted("Open", "row-open", target.clone()),
            targeted_key("Rename…", "row-rename", target.clone(), "F2"),
        ]),
        section(&[
            targeted(if pinned { "Unpin" } else { "Pin to top" }, "row-pin", target.clone()),
            targeted("Duplicate", "row-duplicate", target.clone()),
            targeted("Copy note text", "row-copy", target.clone()),
            targeted("Create task from note", "row-task", target.clone()),
        ]),
        section(&[targeted_key("Delete…", "row-delete", target, "Delete")]),
    ])
}

/// What the editor's right-click menu adds below GTK's own Cut / Copy / Paste.
pub fn text_menu() -> gio::Menu {
    let format = section(&[
        item("Bold", "bold", Some("<Control>b")),
        item("Italic", "italic", Some("<Control>i")),
        item("Inline code", "code", None),
        item("Link", "link", Some("<Control>k")),
    ]);
    let lines = section(&[
        item("Checklist", "checklist", None),
        item("Bulleted list", "bullets", None),
        item("Numbered list", "numbers", None),
        item("Quote", "quote", None),
    ]);
    let format_menu = gio::Menu::new();
    format_menu.append_section(None, &format);
    format_menu.append_section(None, &headings_menu());
    format_menu.append_section(None, &lines);
    let top = section(&[
        item("Tick or untick item", "toggle-check", Some("<Control>Return")),
        item("Insert image…", "image", None),
    ]);
    top.append_submenu(Some("Format"), &format_menu);
    menu(&[top, section(&[item("Create task from selection", "to-task", Some("<Control><Shift>t"))])])
}

fn focus_in(ui: &Rc<Ui>, doc: &Doc) -> bool {
    super::window_of(ui)
        .and_then(|window| gtk::prelude::GtkWindowExt::focus(&window))
        .is_some_and(|focus| focus.is_ancestor(&doc.draft.layout))
}

fn shortcut(ui: &Rc<Ui>, key: Key, mods: ModifierType) -> glib::Propagation {
    let mask = ModifierType::CONTROL_MASK
        | ModifierType::SHIFT_MASK
        | ModifierType::ALT_MASK
        | ModifierType::SUPER_MASK
        | ModifierType::META_MASK;
    let mods = mods & mask;
    let ctrl = ModifierType::CONTROL_MASK;
    let shift = ModifierType::SHIFT_MASK;
    // On a library row, F2 and Delete are that note's, as its menu says, not the open tab's.
    if mods.is_empty() && matches!(key, Key::F2 | Key::Delete | Key::KP_Delete) {
        if let (Some(id), Some(group)) = (super::focused_row(ui), group()) {
            group.activate_action(if key == Key::F2 { "row-rename" } else { "row-delete" }, Some(&id.to_variant()));
            return glib::Propagation::Stop;
        }
    }
    let doc = super::current_doc(ui);
    let in_body = doc.as_ref().is_some_and(|d| d.view.has_focus());
    let body = |name: &'static str| if in_body { name } else { "" };
    let lower = key.to_lower();
    let name = if mods == ctrl {
        match lower {
            Key::n => "new",
            Key::o => "open",
            Key::s => "save",
            Key::w => "close-tab",
            Key::f => "find",
            Key::h | Key::r => "replace",
            Key::g => "goto",
            Key::b => body("bold"),
            Key::i => body("italic"),
            Key::k => body("link"),
            Key::plus | Key::equal | Key::KP_Add => "zoom-in",
            Key::minus | Key::KP_Subtract => "zoom-out",
            Key::_0 | Key::KP_0 => "zoom-reset",
            Key::Tab | Key::KP_Tab | Key::Page_Down => "next-tab",
            Key::Page_Up => "prev-tab",
            Key::Return | Key::KP_Enter => body("toggle-check"),
            _ => "",
        }
    } else if mods == ctrl | shift {
        match lower {
            Key::s => "save-as",
            Key::t => "to-task",
            Key::Tab | Key::ISO_Left_Tab => "prev-tab",
            Key::plus | Key::equal => "zoom-in",
            _ => "",
        }
    } else if mods.is_empty() {
        match key {
            Key::F2 => "rename",
            Key::F3 => "find-next",
            Key::F5 => "datetime",
            Key::F9 => "sidebar",
            Key::Escape if doc.as_ref().is_some_and(|d| d.find_visible() && focus_in(ui, d)) => "close-find",
            _ => "",
        }
    } else if mods == shift {
        match key {
            Key::F3 => "find-prev",
            _ => "",
        }
    } else if mods == ModifierType::ALT_MASK {
        match lower {
            Key::z => "wrap",
            _ => "",
        }
    } else {
        ""
    };
    if name.is_empty() {
        return glib::Propagation::Proceed;
    }
    activate(name);
    glib::Propagation::Stop
}
