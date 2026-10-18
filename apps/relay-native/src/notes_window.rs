use super::*;
use std::cell::Cell;

pub struct NotesWindow {
    pub window: gtk::Window,
    pub project: Cell<i64>,
    pub rail_width: Cell<i32>,
    pub rail_collapsed: Cell<bool>,
    /// The open note's name, centred in the titlebar.
    pub heading: gtk::Label,
    /// Completed library renders; the shell persists, so tests wait on this.
    pub renders: Cell<u64>,
    loading: Cell<bool>,
    /// Settings were read; until then a persist would overwrite them with defaults.
    loaded: Cell<bool>,
    persist_wanted: Cell<bool>,
    pending: Cell<bool>,
    restored: Cell<bool>,
    persist_timer: RefCell<Option<glib::SourceId>>,
}
impl NotesWindow {
    fn new(ui: &Rc<Ui>) -> Rc<Self> {
        let window = gtk::Window::builder()
            .application(&ui.window.application().unwrap())
            .title("Notes · Relay")
            .default_width(1080)
            .default_height(760)
            .build();
        window.add_css_class("notes-window");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // A compact titlebar in the KWrite mould: menu bar, document name, window keys.
        let chrome = gtk::CenterBox::new();
        chrome.add_css_class("notes-chrome");
        let start = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let mark = crate::icons::image("notes", 14);
        mark.add_css_class("notes-chrome-mark");
        mark.set_valign(gtk::Align::Center);
        start.append(&mark);
        start.append(&super::note_pages::window_menu(ui, &window));
        chrome.set_start_widget(Some(&start));
        let heading = label("Notes", "notes-heading");
        heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
        heading.set_max_width_chars(48);
        chrome.set_center_widget(Some(&heading));
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        controls.add_css_class("notes-chrome-controls");
        for (icon, hint) in [
            ("minimize", "Minimize"),
            ("maximize", "Maximize"),
            ("close", "Close Notes"),
        ] {
            let key = crate::app::icon_button(icon, hint);
            key.set_focus_on_click(false);
            if icon == "close" {
                key.add_css_class("notes-window-close");
            }
            let target = window.downgrade();
            key.connect_clicked(move |_| {
                if let Some(window) = target.upgrade() {
                    match icon {
                        "minimize" => window.minimize(),
                        "maximize" => {
                            if window.is_maximized() {
                                window.unmaximize();
                            } else {
                                window.maximize();
                            }
                        }
                        _ => window.close(),
                    }
                }
            });
            controls.append(&key);
        }
        chrome.set_end_widget(Some(&controls));
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&chrome));
        // The titlebar lives inside the wallpaper backdrop, as the main window's top bar, so
        // panel opacity shows the wallpaper behind it too. A hidden titlebar keeps GTK's
        // resize edges; the WindowHandle still drags and maximizes the window.
        let titlebar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        titlebar.set_visible(false);
        window.set_titlebar(Some(&titlebar));
        root.append(&handle);
        let content = &ui.pages["notes"];
        content.set_vexpand(true);
        content.append(&label("Opening Notes…", "notes-window-loading"));
        root.append(content);
        // The main window's wallpaper and dim layer, mirrored: panel opacity shows the same
        // picture here, never the desktop (the window itself paints @wall_solid underneath).
        let wallpaper = gtk::Picture::new();
        wallpaper.set_can_shrink(true);
        wallpaper.set_content_fit(gtk::ContentFit::Cover);
        ui.wallpaper.bind_property("paintable", &wallpaper, "paintable").sync_create().build();
        let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
        dim.add_css_class("wallpaper-dim");
        dim.set_can_target(false);
        ui.wallpaper_dim.bind_property("opacity", &dim, "opacity").sync_create().build();
        let backdrop = gtk::Overlay::new();
        backdrop.add_css_class("notes-backdrop");
        backdrop.set_child(Some(&wallpaper));
        backdrop.add_overlay(&dim);
        backdrop.add_overlay(&root);
        root.set_hexpand(true);
        root.set_vexpand(true);
        window.set_child(Some(&backdrop));
        let owned = Rc::new(Self {
            window,
            project: Cell::new(0),
            rail_width: Cell::new(270),
            rail_collapsed: Cell::new(false),
            heading,
            renders: Cell::new(0),
            loading: Cell::new(false),
            loaded: Cell::new(false),
            persist_wanted: Cell::new(false),
            pending: Cell::new(false),
            restored: Cell::new(false),
            persist_timer: RefCell::new(None),
        });
        let weak = Rc::downgrade(ui);
        owned.window.connect_close_request(move |window| {
            window.set_visible(false);
            if let Some(ui) = weak.upgrade() {
                super::note_pages::flush(&ui);
                if let Some(owner) = ui.notes_window.borrow().as_ref() {
                    owner.persist(&ui);
                }
            }
            glib::Propagation::Stop
        });
        owned
    }
    pub fn persist(&self, ui: &Rc<Ui>) {
        if !self.loaded.get() {
            self.persist_wanted.set(true);
            return;
        }
        if let Some(timer) = self.persist_timer.borrow_mut().take() {
            timer.remove();
        }
        let weak = Rc::downgrade(ui);
        *self.persist_timer.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(300),
            move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let value = {
                    let owner = ui.notes_window.borrow();
                    let Some(owner) = owner.as_ref() else {
                        return;
                    };
                    owner.persist_timer.borrow_mut().take();
                    let mut value = json!({"rail_width":owner.rail_width.get(),"rail_collapsed":owner.rail_collapsed.get()});
                    if let (Some(value), Value::Object(state)) = (value.as_object_mut(), super::note_pages::saved_state()) {
                        value.extend(state);
                    }
                    value
                };
                glib::spawn_future_local(async move {
                    let _ = ui
                        .call(
                            "settings.set",
                            json!({"path":"native.notes.window","value":value}),
                        )
                        .await;
                });
            },
        ));
    }
}
pub fn show_notes(ui: &Rc<Ui>) {
    show_project(ui, ui.project.get());
}
pub fn show_project(ui: &Rc<Ui>, project: i64) {
    if ui.notes_window.borrow().is_none() {
        let window = NotesWindow::new(ui);
        *ui.notes_window.borrow_mut() = Some(window);
    }
    let window = ui.notes_window.borrow().as_ref().unwrap().clone();
    if project != 0 {
        window.project.set(project);
    }
    window.window.present();
    refresh_notes(ui);
}
pub fn refresh_notes(ui: &Rc<Ui>) {
    let Some(window) = ui.notes_window.borrow().clone() else {
        return;
    };
    // Nobody reads a hidden library, and every main-window refresh and notes event lands
    // here; showing the window (show_project) refreshes it.
    if !window.window.is_visible() {
        return;
    }
    if window.project.get() == 0 {
        window.project.set(ui.project.get());
    }
    // With no project the settings are still read, so preferences persist, and the window
    // says why it is empty.
    window.pending.set(true);
    if window.loading.replace(true) {
        return;
    }
    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        let Some(ui) = weak.upgrade() else {
            window.loading.set(false);
            return;
        };
        if !window.restored.get() {
            match ui
                .call("settings.get", json!({"path":"native.notes.window"}))
                .await
            {
                Ok(value) => {
                    if let Some(width) = value["value"]["rail_width"].as_i64() {
                        window.rail_width.set(width.clamp(180, 600) as i32);
                    }
                    window
                        .rail_collapsed
                        .set(value["value"]["rail_collapsed"].as_bool().unwrap_or(false));
                    super::note_pages::restore_state(&ui, &value["value"]);
                    window.restored.set(true);
                    window.loaded.set(true);
                    if window.persist_wanted.replace(false) {
                        window.persist(&ui);
                    }
                }
                // The first render persists the window, so it waits for the saved settings:
                // rendering on defaults would overwrite them. The next refresh reads again.
                Err(error) => {
                    window.pending.set(false);
                    window.loading.set(false);
                    super::note_pages::load_error(&ui, &format!("Could not open Notes: {}", crate::editor::readable(&error)));
                    return;
                }
            }
        }
        while window.pending.replace(false) {
            let project = window.project.get();
            if project == 0 {
                break;
            }
            // Previews only: whole bodies made every refresh read and rebuild all the text in
            // the project, and past 2 MiB the reply no longer fit the client's line limit.
            match ui.call("notes.list", json!({"project_id":project,"summary":true})).await {
                Ok(value) if project == window.project.get() => {
                    let notes = rows(&value, "notes");
                    // What the render hands to an editor is read whole first.
                    for id in super::note_pages::needs_full(project, &notes) {
                        let _ = super::note_pages::fetch_full(&ui, id).await;
                    }
                    if project != window.project.get() {
                        window.pending.set(true);
                        continue;
                    }
                    super::note_pages::workspace(&ui, "notes", project, &notes);
                    super::note_pages::fill(&ui);
                }
                Ok(_) => window.pending.set(true),
                // The project was removed. Show the main window's project if that one still
                // exists, else an empty library; never ask for the removed one again.
                Err(crate::client::Error::Bus(error)) if error.code == "project.not_found" => {
                    if project == window.project.get() {
                        let fallback = ui.project.get();
                        let alive = fallback != project
                            && ui.projects.borrow().iter().any(|p| p["id"].as_i64() == Some(fallback));
                        window.project.set(if alive { fallback } else { 0 });
                        super::note_pages::project_gone(&ui, project);
                    }
                    window.pending.set(true);
                }
                Err(error) => super::note_pages::load_error(&ui, &format!("Could not list the notes: {}", crate::editor::readable(&error))),
            }
        }
        if window.project.get() == 0 {
            no_project(&ui);
        }
        window.loading.set(false);
    });
}

/// A fresh install, or the last project was removed: nothing to load notes from. Only the
/// loading line is replaced; a library already on screen stays.
fn no_project(ui: &Rc<Ui>) {
    let waiting = ui.pages["notes"]
        .first_child()
        .and_downcast::<gtk::Label>()
        .filter(|label| label.has_css_class("notes-window-loading"));
    if let Some(waiting) = waiting {
        waiting.set_text("No project yet. Add one from the main window to keep notes in it.");
    }
}
