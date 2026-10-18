//! The Settings page: a category list beside one scrolling column per category. Every
//! control saves itself a moment after it changes (`Autosave`); the guardrail editor, which
//! edits one of three layers at a time, keeps its own Save.
use super::current;
use crate::app::{clear, label, rows, text, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

/// The widest a category's column grows; past it the page keeps empty space on the right.
const COLUMN_MAX: i32 = 760;
/// The column's own left and right padding (settings.css `.settings-column`).
const COLUMN_PADDING: i32 = 96;
/// What the header caption says while nothing is being saved.
const IDLE: &str = "Changes save automatically";

pub(crate) struct WallpaperDraft {
    state: Rc<RefCell<Value>>,
    saved: Rc<RefCell<Value>>,
    gallery: glib::WeakRef<gtk::Box>,
    look: Rc<Look>,
    /// The saved wallpaper changed while the gallery was hidden: redraw it on the way back.
    stale: Cell<bool>,
}

pub(crate) fn sync_wallpaper(ui: &Rc<Ui>, image: &Value) {
    let draft = ui.wallpaper_draft.borrow();
    let Some(draft) = draft.as_ref() else {
        return;
    };
    if draft.saved.borrow()["wallpaper"] == *image {
        return;
    }
    let dirty = draft.state.borrow()["wallpaper"] != draft.saved.borrow()["wallpaper"];
    draft.saved.borrow_mut()["wallpaper"] = image.clone();
    if !dirty {
        draft.state.borrow_mut()["wallpaper"] = image.clone();
        if let Some(gallery) = draft.gallery.upgrade() {
            // Hidden, the gallery waits for Settings to show again instead of redrawing.
            if gallery.is_mapped() {
                render_wallpapers(ui, &gallery, &draft.state, &draft.look);
            } else {
                draft.stale.set(true);
            }
        }
    }
}

/// What the page's controls held when it was built or last saved: a save sends only what
/// differs, so it neither reverts a value changed elsewhere nor re-clamps one it never showed.
#[derive(PartialEq)]
struct Snapshot {
    settings: BTreeMap<String, Value>,
    /// `sound`, `volume` and `categories.<name>`.
    notifications: BTreeMap<String, Value>,
}

/// One control's current value: `None` when it has nothing to save (a mode with no card
/// chosen, a shortcut that does not parse) or the control is gone.
type Reader = Box<dyn Fn() -> Option<Value>>;

/// Every control the page saves, registered with where it saves to as the control is built.
#[derive(Default)]
struct Fields {
    /// `settings.set` paths.
    settings: Vec<(String, Reader)>,
    /// `notify.settings.set` keys, as in `Snapshot::notifications`.
    notifications: Vec<(String, Reader)>,
}

impl Fields {
    fn snapshot(&self) -> Snapshot {
        let read = |fields: &[(String, Reader)]| {
            fields.iter().filter_map(|(key, read)| Some((key.clone(), read()?))).collect()
        };
        Snapshot { settings: read(&self.settings), notifications: read(&self.notifications) }
    }
}

/// Reads `widget` through `get` while it exists. Weak, so the page's fields do not keep a
/// page that was cleared alive.
fn reader<W: IsA<gtk::Widget>>(widget: &W, get: impl Fn(&W) -> Option<Value> + 'static) -> Reader {
    let weak = widget.downgrade();
    Box::new(move || weak.upgrade().and_then(|widget| get(&widget)))
}

/// Saves what differs from the baseline a moment after the last edit. One save runs at a
/// time; an edit made during it is sent by the save that follows.
struct Autosave {
    ui: Weak<Ui>,
    fields: Rc<RefCell<Fields>>,
    baseline: RefCell<Option<Snapshot>>,
    wallpapers: Rc<RefCell<Value>>,
    saved_wallpapers: Rc<RefCell<Value>>,
    /// Each autosaving category's header caption.
    captions: RefCell<Vec<glib::WeakRef<gtk::Label>>>,
    timer: RefCell<Option<glib::SourceId>>,
    saving: Cell<bool>,
    again: Cell<bool>,
    /// Bumped by every caption change, so a stale "Saved" timer does not clear a newer one.
    shown: Cell<u64>,
}

impl Autosave {
    fn schedule(self: &Rc<Self>, delay: u64) {
        if let Some(timer) = self.timer.take() {
            timer.remove();
        }
        let weak = Rc::downgrade(self);
        let timer = glib::timeout_add_local_once(Duration::from_millis(delay), move || {
            if let Some(save) = weak.upgrade() {
                // It has fired: forget it rather than remove it.
                save.timer.take();
                save.flush();
            }
        });
        *self.timer.borrow_mut() = Some(timer);
    }

    fn pending(&self) -> bool {
        self.saving.get() || self.timer.borrow().is_some()
    }

    fn dirty(&self) -> bool {
        let wallpaper = *self.wallpapers.borrow() != *self.saved_wallpapers.borrow();
        let controls = self.baseline.borrow().as_ref().is_some_and(|b| *b != self.fields.borrow().snapshot());
        wallpaper || controls || self.pending()
    }

    /// The writes that bring the engine up to the page.
    fn changes(&self, now: &Snapshot, wallpapers: &Value) -> Vec<(&'static str, Value)> {
        let before = self.baseline.borrow();
        let mut writes = Vec::new();
        for (path, value) in &now.settings {
            if before.as_ref().and_then(|b| b.settings.get(path)) != Some(value) {
                writes.push(("settings.set", json!({"path":path,"value":value})));
            }
        }
        for field in ["wallpapers", "wallpaper"] {
            if wallpapers[field] != self.saved_wallpapers.borrow()[field] {
                writes.push(("settings.set", json!({"path":format!("appearance.{field}"),"value":wallpapers[field]})));
            }
        }
        let mut notifications = json!({});
        for (key, value) in &now.notifications {
            if before.as_ref().and_then(|b| b.notifications.get(key)) != Some(value) {
                match key.strip_prefix("categories.") {
                    Some(category) => notifications["categories"][category] = value.clone(),
                    None => notifications[key.as_str()] = value.clone(),
                }
            }
        }
        if notifications.as_object().is_some_and(|patch| !patch.is_empty()) {
            writes.push(("notify.settings.set", json!({"patch":notifications})));
        }
        writes
    }

    fn flush(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if self.saving.get() {
            self.again.set(true);
            return;
        }
        let now = self.fields.borrow().snapshot();
        let wallpapers = self.wallpapers.borrow().clone();
        let writes = self.changes(&now, &wallpapers);
        if writes.is_empty() {
            return;
        }
        self.saving.set(true);
        self.caption("Saving…");
        let save = self.clone();
        glib::spawn_future_local(async move {
            let mut error = None;
            for (op, payload) in writes {
                if let Err(e) = ui.call(op, payload).await {
                    error = Some(e.to_string());
                    break;
                }
            }
            save.saving.set(false);
            match error {
                Some(error) => {
                    save.caption("Not saved");
                    ui.show_error(&format!("Settings not saved: {error}"));
                }
                None => {
                    *save.saved_wallpapers.borrow_mut() = wallpapers;
                    *save.baseline.borrow_mut() = Some(now);
                    save.caption("All changes saved");
                    let shown = save.shown.get();
                    let weak = Rc::downgrade(&save);
                    glib::timeout_add_local_once(Duration::from_millis(1600), move || {
                        if let Some(save) = weak.upgrade().filter(|s| s.shown.get() == shown) {
                            save.caption(IDLE);
                        }
                    });
                }
            }
            if save.again.replace(false) {
                save.flush();
            }
        });
    }

    fn caption(&self, words: &str) {
        self.shown.set(self.shown.get() + 1);
        for caption in self.captions.borrow().iter().filter_map(|c| c.upgrade()) {
            caption.set_text(words);
        }
    }
}

/// What the builders register their controls with.
struct Form {
    fields: Rc<RefCell<Fields>>,
    autosave: Rc<Autosave>,
}

impl Form {
    fn setting(&self, path: impl Into<String>, read: Reader) {
        self.fields.borrow_mut().settings.push((path.into(), read));
    }

    fn notification(&self, key: impl Into<String>, read: Reader) {
        self.fields.borrow_mut().notifications.push((key.into(), read));
    }

    /// Saves `delay` ms after the last call: short for a switch, longer for typing.
    fn changed(&self, delay: u64) -> Rc<dyn Fn()> {
        let weak = Rc::downgrade(&self.autosave);
        Rc::new(move || {
            if let Some(save) = weak.upgrade() {
                save.schedule(delay);
            }
        })
    }
}

/// The mounted page. It is built once per connection, not per project: nothing on it is per
/// project, and rebuilding it on a project switch threw unsaved edits away.
struct Mounted {
    page: glib::WeakRef<gtk::Box>,
    autosave: Rc<Autosave>,
    providers: glib::WeakRef<gtk::Box>,
    detected: RefCell<Value>,
    /// The category keys, for [`open_on`].
    nav: glib::WeakRef<gtk::Box>,
}

thread_local! {
    static MOUNTED: RefCell<Option<Rc<Mounted>>> = const { RefCell::new(None) };
    /// The category the page shows next, when a key elsewhere asked for one ([`open_on`]).
    static OPEN_ON: Cell<Option<&'static str>> = const { Cell::new(None) };
}

/// Shows Settings on its `name` category: Threads' notice about Claude Code opens Agents.
pub(crate) fn open_on(ui: &Rc<Ui>, name: &'static str) {
    OPEN_ON.with(|o| o.set(Some(name)));
    ui.navigate("settings");
    // A panel that would not close kept the window where it was: no later visit opens there.
    if ui.page.borrow().as_str() != "settings" {
        OPEN_ON.with(|o| o.set(None));
    }
}

/// Lights `name`'s key in the category list, which shows its page.
fn show_category(nav: &gtk::Box, name: &str) {
    let wanted = format!("settings-category-{name}");
    let mut key = nav.first_child();
    while let Some(widget) = key {
        if widget.widget_name().as_str() == wanted {
            if let Some(toggle) = widget.downcast_ref::<gtk::ToggleButton>() {
                toggle.set_active(true);
            }
            return;
        }
        key = widget.next_sibling();
    }
}

impl Mounted {
    /// Whether a control or the wallpaper draft differs from what was loaded or saved.
    fn dirty(&self) -> bool {
        self.page.upgrade().is_some() && self.autosave.dirty()
    }

    /// Redraws the provider rows when `provider.list` or `provider.refresh` says something new.
    fn show_providers(&self, detected: &Value) {
        let Some(card) = self.providers.upgrade() else {
            return;
        };
        if *self.detected.borrow() == *detected && card.first_child().is_some() {
            return;
        }
        *self.detected.borrow_mut() = detected.clone();
        clear(&card);
        provider_rows(&card, detected);
    }
}

pub async fn refresh(ui: &Rc<Ui>, project: i64) {
    let generation = ui.generation.get();
    let mounted = MOUNTED.with(|m| m.borrow().clone()).filter(|m| m.page.upgrade().is_some());
    let built = ui.page_projects.borrow().get("settings").copied();
    if let Some(mounted) = mounted.filter(|m| built == Some(generation as i64) || (built.is_some() && m.dirty())) {
        // Keep the form, and any edits in it, across project switches and a reconnect; only
        // re-read the providers, which change behind the page (an install, an update).
        ui.page_projects.borrow_mut().insert("settings".into(), generation as i64);
        if let (Some(name), Some(nav)) = (OPEN_ON.with(Cell::take), mounted.nav.upgrade()) {
            show_category(&nav, name);
        }
        if let Some(draft) = ui.wallpaper_draft.borrow().as_ref() {
            if draft.stale.replace(false) {
                if let Some(gallery) = draft.gallery.upgrade() {
                    render_wallpapers(ui, &gallery, &draft.state, &draft.look);
                }
            }
        }
        let detected = ui.call("provider.list", json!({})).await;
        if current(ui, "settings", project, generation) {
            if let Ok(detected) = detected {
                mounted.show_providers(&detected);
            }
        }
        return;
    }
    let result = load(ui).await;
    if !current(ui, "settings", project, generation) {
        return;
    }
    let data = match result {
        Ok(data) => data,
        Err(e) => {
            ui.show_error(&e.to_string());
            return;
        }
    };
    let wallpapers = Rc::new(RefCell::new(
        json!({"wallpapers":data["appearance.wallpapers"],"wallpaper":data["appearance.wallpaper"]}),
    ));
    let saved_wallpapers = Rc::new(RefCell::new(wallpapers.borrow().clone()));
    // Keep the saved baseline null so the first save persists offered presets, but never
    // select a wallpaper or replace an explicitly empty/custom library here.
    wallpapers.borrow_mut()["wallpapers"] =
        crate::wallpaper_rotation::library_or_defaults(&data["appearance.wallpapers"]);
    let page = &ui.pages["settings"];
    clear(page);
    page.add_css_class("settings-page");
    page.set_spacing(0);
    ui.page_projects
        .borrow_mut()
        .insert("settings".into(), generation as i64);
    let fields = Rc::new(RefCell::new(Fields::default()));
    let autosave = Rc::new(Autosave {
        ui: Rc::downgrade(ui),
        fields: fields.clone(),
        baseline: RefCell::new(None),
        wallpapers: wallpapers.clone(),
        saved_wallpapers: saved_wallpapers.clone(),
        captions: RefCell::default(),
        timer: RefCell::new(None),
        saving: Cell::new(false),
        again: Cell::new(false),
        shown: Cell::new(0),
    });
    let form = Form { fields: fields.clone(), autosave: autosave.clone() };
    let (shell, nav, stack) = body(page);
    let providers = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let mounted = Rc::new(Mounted {
        page: page.downgrade(),
        autosave: autosave.clone(),
        providers: providers.downgrade(),
        detected: RefCell::new(data["detected"].clone()),
        nav: nav.downgrade(),
    });
    MOUNTED.with(|m| *m.borrow_mut() = Some(mounted.clone()));
    appearance(ui, &stack, &form, &data, &wallpapers, &saved_wallpapers);
    notifications(ui, &stack, &form, &data);
    agents(ui, &stack, &form, &data, &providers, &mounted);
    safety(ui, &stack);
    android(&stack, &form, &data);
    keyboard(&stack, &form, &data);
    maintenance(ui, &stack);
    let search = search_field(ui);
    wire_search(&search, &shell, &nav, &stack);
    // Appearance shows first unless a key elsewhere asked for another; its page exists only now.
    if let Some(name) = OPEN_ON.with(Cell::take) {
        show_category(&nav, name);
    }
    // Taken from the built controls, so a value a range clamped counts as unchanged.
    *autosave.baseline.borrow_mut() = Some(fields.borrow().snapshot());
}

/// The settings the page shows, read narrowly: wallpaper libraries can be megabytes and only
/// the two the gallery needs are read. All of them are asked for at once.
async fn load(ui: &Ui) -> Result<Value, crate::client::Error> {
    const PATHS: [&str; 11] = [
        "providers",
        "device",
        "keybindings",
        "appearance.mode",
        "appearance.panel_alpha",
        "appearance.wallpaper_dim",
        "appearance.content_contrast",
        "appearance.wallpapers",
        "appearance.wallpaper",
        "appearance.wallpaper_rotation",
        "terminal.font_size",
    ];
    let settings = join_all(PATHS.map(|path| ui.call("settings.get", json!({"path":path}))).into());
    let (settings, notifications, detected) = tokio::join!(
        settings,
        ui.call("notify.settings.get", json!({})),
        ui.call("provider.list", json!({}))
    );
    let mut data = json!({});
    for (path, value) in PATHS.iter().zip(settings) {
        data[*path] = value?["value"].clone();
    }
    data["notifications"] = notifications?;
    data["detected"] = detected?;
    Ok(data)
}

/// Awaits every future together and returns their outputs in order.
async fn join_all<F: std::future::Future>(futures: Vec<F>) -> Vec<F::Output> {
    let mut futures: Vec<_> = futures.into_iter().map(Box::pin).collect();
    let mut outputs: Vec<Option<F::Output>> = futures.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut waiting = false;
        for (future, output) in futures.iter_mut().zip(outputs.iter_mut()) {
            if output.is_none() {
                match future.as_mut().poll(cx) {
                    std::task::Poll::Ready(value) => *output = Some(value),
                    std::task::Poll::Pending => waiting = true,
                }
            }
        }
        if waiting { std::task::Poll::Pending } else { std::task::Poll::Ready(()) }
    })
    .await;
    outputs.into_iter().map(|output| output.expect("every future finished")).collect()
}

/// The search field, in the title bar's slot for it (app.rs shows the slot on Settings only).
fn search_field(ui: &Ui) -> gtk::SearchEntry {
    let search = gtk::SearchEntry::new();
    search.set_widget_name("settings-search");
    search.add_css_class("search");
    search.add_css_class("settings-search");
    search.set_placeholder_text(Some("Search settings"));
    search.set_size_request(260, -1);
    search.set_valign(gtk::Align::Center);
    clear(&ui.settings_tools);
    ui.settings_tools.append(&search);
    search
}

/// The category list beside the stack of category pages.
fn body(page: &gtk::Box) -> (gtk::Box, gtk::Box, gtk::Stack) {
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    shell.add_css_class("settings-body");
    shell.set_hexpand(true);
    shell.set_vexpand(true);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let nav = gtk::Box::new(gtk::Orientation::Vertical, 4);
    nav.add_css_class("settings-nav");
    nav.set_size_request(246, -1);
    let mut first = None::<gtk::ToggleButton>;
    for Category { name, title, icon, nav: hint, .. } in CATEGORIES {
        let key = gtk::ToggleButton::new();
        key.add_css_class("settings-category");
        key.set_widget_name(&format!("settings-category-{name}"));
        if let Some(first) = &first {
            key.set_group(Some(first));
        } else {
            first = Some(key.clone());
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let mark = crate::icons::from_geometry(icon, 16, 1.5);
        mark.add_css_class("settings-category-icon");
        mark.set_valign(gtk::Align::Center);
        row.append(&mark);
        let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
        copy.set_valign(gtk::Align::Center);
        copy.append(&label(title, "settings-nav-title"));
        copy.append(&label(hint, "settings-nav-hint"));
        row.append(&copy);
        key.set_child(Some(&row));
        let target = stack.downgrade();
        key.connect_toggled(move |key| {
            if key.is_active() {
                if let Some(stack) = target.upgrade() {
                    if stack.child_by_name(name).is_some() {
                        stack.set_visible_child_name(name);
                    }
                }
            }
        });
        key.set_active(name == "appearance");
        nav.append(&key);
    }
    shell.append(&nav);
    shell.append(&stack);
    page.append(&shell);
    (shell, nav, stack)
}

/// The appearance modes, their cards' words, in the order the cards show.
const MODES: [(&str, &str, &str); 3] = [
    ("matte", "Matte", "Soft console around darker panes"),
    ("dark", "Warm dark", "Default, easy on long nights"),
    ("oled", "OLED", "True black, the wall disappears"),
];

fn appearance(
    ui: &Rc<Ui>,
    stack: &gtk::Stack,
    form: &Form,
    data: &Value,
    wallpapers: &Rc<RefCell<Value>>,
    saved_wallpapers: &Rc<RefCell<Value>>,
) {
    let page = category(stack, "appearance", Some(form));
    let mode = data["appearance.mode"].as_str().unwrap_or("dark");
    let look = Rc::new(Look {
        mode: RefCell::new(mode.to_string()),
        alpha: Cell::new(data["appearance.panel_alpha"].as_f64().unwrap_or(1.)),
        dim: Cell::new(data["appearance.wallpaper_dim"].as_f64().unwrap_or(0.28)),
        contrast: Cell::new(data["appearance.content_contrast"].as_f64().unwrap_or(0.)),
        stage: RefCell::default(),
    });

    let theme = section(&page, "Theme", None::<&gtk::Widget>);
    let cards = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    cards.set_homogeneous(true);
    let mut first = None::<gtk::ToggleButton>;
    let mut keys = Vec::new();
    let changed = form.changed(120);
    for (id, title, hint) in MODES {
        let key = gtk::ToggleButton::new();
        key.add_css_class("settings-theme");
        key.set_widget_name(&format!("setting:appearance.mode={id}"));
        if let Some(first) = &first {
            key.set_group(Some(first));
        } else {
            first = Some(key.clone());
        }
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let swatch = gtk::DrawingArea::new();
        swatch.set_content_height(78);
        swatch.set_hexpand(true);
        swatch.set_draw_func(move |_, cr, w, h| draw_theme(cr, id, f64::from(w), f64::from(h)));
        card.append(&swatch);
        let words = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        words.add_css_class("settings-theme-words");
        let copy = gtk::Box::new(gtk::Orientation::Vertical, 4);
        copy.set_hexpand(true);
        copy.append(&label(title, "settings-theme-title"));
        let line = label(hint, "settings-theme-hint");
        line.set_wrap(true);
        copy.append(&line);
        words.append(&copy);
        let check = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        check.add_css_class("settings-check");
        check.set_valign(gtk::Align::Center);
        check.set_hexpand(false);
        let tick = crate::icons::image_with_stroke("check", 11, 2.2);
        tick.set_halign(gtk::Align::Center);
        tick.set_valign(gtk::Align::Center);
        tick.set_hexpand(true);
        check.append(&tick);
        check.set_visible(mode == id);
        words.append(&check);
        card.append(&words);
        key.set_child(Some(&card));
        key.set_active(mode == id);
        let shown = look.clone();
        let changed = changed.clone();
        key.connect_toggled(move |key| {
            check.set_visible(key.is_active());
            if key.is_active() {
                *shown.mode.borrow_mut() = id.to_string();
                shown.redraw();
                changed();
            }
        });
        cards.append(&key);
        keys.push((key.downgrade(), id));
    }
    form.setting(
        "appearance.mode",
        Box::new(move || keys.iter().find(|(key, _)| key.upgrade().is_some_and(|key| key.is_active())).map(|(_, id)| json!(id))),
    );
    theme.append(&cards);

    // The gallery draws its own heading: "Clear background" sits on it.
    let gallery = gtk::Box::new(gtk::Orientation::Vertical, 0);
    gallery.add_css_class("settings-section");
    page.append(&gallery);
    let draft_look = look.clone();
    *ui.wallpaper_draft.borrow_mut() = Some(WallpaperDraft {
        state: wallpapers.clone(),
        saved: saved_wallpapers.clone(),
        gallery: gallery.downgrade(),
        look: draft_look,
        stale: Cell::new(false),
    });
    WALLPAPER_SAVE.with(|s| *s.borrow_mut() = Some(form.changed(120)));
    render_wallpapers(ui, &gallery, wallpapers, &look);

    let readability = section(&page, "Readability", None::<&gtk::Widget>);
    let readability = card(&readability);
    for (path, title, hint, default, min, max) in [
        ("appearance.panel_alpha", "Panel opacity", "How solid terminals and the sidebar are over the wallpaper.", 1., 0.72, 1.),
        ("appearance.wallpaper_dim", "Wallpaper dim", "Darkens the image itself.", 0.28, 0., 0.75),
        ("appearance.content_contrast", "Content protection", "Strengthens panels over bright parts of the image so text stays readable.", 0., 0., 1.),
    ] {
        let (control, input) = slider(min, max, data[path].as_f64().unwrap_or(default));
        input.set_widget_name(&format!("setting:{path}"));
        form.setting(path, reader(&input, |input| Some(json!(input.value()))));
        let changed = form.changed(400);
        let shown = look.clone();
        input.connect_value_changed(move |input| {
            let cell = match path {
                "appearance.panel_alpha" => &shown.alpha,
                "appearance.wallpaper_dim" => &shown.dim,
                _ => &shown.contrast,
            };
            cell.set(input.value());
            shown.redraw();
            changed();
        });
        row(&readability, title, hint, Some(&control));
    }
    let rotation = &data["appearance.wallpaper_rotation"];
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    controls.append(&label("Every", "settings-unit"));
    let minutes = stepper(
        "setting:appearance.wallpaper_rotation.interval_minutes",
        rotation["interval_minutes"].as_f64().unwrap_or(15.).clamp(1., 1440.),
        1.,
        1440.,
        1.,
    );
    let interval = minutes.adjustment.clone();
    form.setting(
        "appearance.wallpaper_rotation.interval_minutes",
        reader(&minutes.root, move |_| Some(json!(interval.value().round() as i64))),
    );
    let changed = form.changed(500);
    minutes.adjustment.connect_value_changed(move |_| changed());
    controls.append(&minutes.root);
    controls.append(&label("min", "settings-unit"));
    let enabled = toggle("setting:appearance.wallpaper_rotation.enabled", rotation["enabled"] == true);
    enabled.set_margin_start(6);
    form.setting("appearance.wallpaper_rotation.enabled", reader(&enabled, |input| Some(json!(input.is_active()))));
    let changed = form.changed(120);
    enabled.connect_active_notify(move |_| changed());
    controls.append(&enabled);
    row(&readability, "Rotate wallpapers", "Shuffles through your library. Needs at least two wallpapers.", Some(&controls));
}

thread_local! {
    /// Schedules a save after a wallpaper pick, removal or addition: the gallery redraws
    /// itself from several places that have no `Form` to hand.
    static WALLPAPER_SAVE: RefCell<Option<Rc<dyn Fn()>>> = RefCell::default();
}

fn wallpaper_changed() {
    if let Some(save) = WALLPAPER_SAVE.with(|s| s.borrow().clone()) {
        save();
    }
}

fn notifications(ui: &Rc<Ui>, stack: &gtk::Stack, form: &Form, data: &Value) {
    let page = category(stack, "notifications", Some(form));
    let settings = &data["notifications"];
    let sound = section(&page, "Sound", None::<&gtk::Widget>);
    let alerts = card(&sound);
    const SOUNDS: [(&str, &str); 5] =
        [("off", "Off"), ("chime", "Chime"), ("glass", "Glass"), ("pulse", "Pulse"), ("signal", "Signal")];
    let picker = gtk::DropDown::from_strings(&SOUNDS.map(|(_, name)| name));
    picker.add_css_class("settings-select");
    picker.set_widget_name("notify:sound");
    picker.set_selected(
        SOUNDS.iter().position(|(id, _)| Some(*id) == settings["sound"].as_str()).unwrap_or(1) as u32,
    );
    form.notification(
        "sound",
        reader(&picker, |input| Some(json!(SOUNDS.get(input.selected() as usize).map_or("off", |(id, _)| *id)))),
    );
    let play = gtk::Button::new();
    play.add_css_class("settings-square");
    play.set_child(Some(&crate::icons::from_geometry(PLAY, 12, 1.5)));
    play.set_tooltip_text(Some("Play the alert"));
    play.set_widget_name("notify:preview");
    play.set_sensitive(picker.selected() != 0);
    let alert = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    alert.append(&picker);
    alert.append(&play);
    row(&alerts, "Alert sound", "Plays for the events you leave on below. Every event still shows in the feed.", Some(&alert));
    let (volume_control, volume) = slider(0., 1., settings["volume"].as_f64().unwrap_or(0.7));
    volume.set_widget_name("notify:volume");
    let speaker = crate::icons::from_geometry(SPEAKER, 14, 1.5);
    speaker.add_css_class("settings-unit");
    volume_control.prepend(&speaker);
    form.notification("volume", reader(&volume, |input| Some(json!(input.value()))));
    let changed = form.changed(400);
    volume.connect_value_changed(move |_| changed());
    row(&alerts, "Volume", "", Some(&volume_control));
    let changed = form.changed(120);
    let key = play.downgrade();
    picker.connect_selected_notify(move |picker| {
        if let Some(key) = key.upgrade() {
            key.set_sensitive(picker.selected() != 0);
        }
        changed();
    });
    let weak = Rc::downgrade(ui);
    let (s, v) = (picker.downgrade(), volume.downgrade());
    play.connect_clicked(move |_| {
        if let (Some(ui), Some(s), Some(v)) = (weak.upgrade(), s.upgrade(), v.upgrade()) {
            crate::sounds::play(&ui, SOUNDS[s.selected() as usize].0, v.value());
        }
    });

    for (title, aside, events) in [
        ("Agents", "Always shown · sound when on", &[
            ("agent_blocked", "Agent needs you", "Blocked on a question or waiting for approval."),
            ("agent_done", "Agent finished", "Once per terminal, and again after you type into it."),
            ("guardrail", "Guardrail needs you", "An action held for approval, or an exception request. Refusals are filed quietly."),
        ][..]),
        ("System", "", &[
            ("provider", "Providers", "Claude or Codex limits, outages and sign-in issues."),
            ("integration", "Integrations", "Git, GitHub and plugin events."),
            ("disk", "Disk", "Low space and worktree cleanup."),
            ("system", "Everything else", "Crashed app runs and other engine events."),
        ][..]),
    ] {
        let aside = (!aside.is_empty()).then(|| label(aside, "settings-aside"));
        let group = section(&page, title, aside.as_ref());
        let events_card = card(&group);
        for (category, title, hint) in events {
            let on = toggle(&format!("notify:category:{category}"), settings["categories"][category].as_bool().unwrap_or(true));
            form.notification(format!("categories.{category}"), reader(&on, |input| Some(json!(input.is_active()))));
            let changed = form.changed(120);
            on.connect_active_notify(move |_| changed());
            row(&events_card, title, hint, Some(&on));
        }
    }
}

/// The provider rows go in `providers`, which `mounted` redraws when the providers change.
fn agents(ui: &Rc<Ui>, stack: &gtk::Stack, form: &Form, data: &Value, providers: &gtk::Box, mounted: &Rc<Mounted>) {
    let page = category(stack, "agents", Some(form));
    // Called directly, not through Ui::mutate: its answer is the providers to show.
    let rediscover = gtk::Button::with_label("Refresh");
    rediscover.add_css_class("settings-link");
    rediscover.set_widget_name("provider-refresh");
    let installed = section(&page, "Installed", Some(&rediscover));
    providers.add_css_class("settings-card");
    installed.append(providers);
    provider_rows(providers, &data["detected"]);
    let weak = Rc::downgrade(ui);
    let shown = Rc::downgrade(mounted);
    rediscover.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        key.set_sensitive(false);
        let key = key.clone();
        let shown = shown.clone();
        glib::spawn_future_local(async move {
            match ui.call("provider.refresh", json!({})).await {
                Ok(detected) => {
                    if let Some(mounted) = shown.upgrade() {
                        mounted.show_providers(&detected);
                    }
                }
                Err(error) => ui.show_error(&error.to_string()),
            }
            key.set_sensitive(true);
        });
    });

    let updates = section(&page, "Updates", None::<&gtk::Widget>);
    let updates = card(&updates);
    for (provider, name) in [("claude", "Claude"), ("codex", "Codex")] {
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        let update = gtk::Button::with_label("Update now");
        update.add_css_class("settings-button");
        update.set_widget_name(&format!("provider-update-{provider}"));
        let weak = Rc::downgrade(ui);
        update.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                crate::provider_updates::update(&ui, provider);
            }
        });
        controls.append(&update);
        let automatic = toggle(&format!("setting:providers.{provider}.auto_update"), data["providers"][provider]["auto_update"] == true);
        form.setting(format!("providers.{provider}.auto_update"), reader(&automatic, |input| Some(json!(input.is_active()))));
        let changed = form.changed(120);
        automatic.connect_active_notify(move |_| changed());
        controls.append(&automatic);
        row(&updates, &format!("Update {name} at startup"), "Installs the newest CLI each time Relay starts.", Some(&controls));
    }

    let paths = section(&page, "Executables", Some(&label("Empty means found on PATH", "settings-aside")));
    let paths = card(&paths);
    for (provider, name) in [("claude", "Claude"), ("codex", "Codex")] {
        let path = format!("providers.{provider}.path");
        let input = path_entry(form, &path, data["providers"][provider]["path"].as_str().unwrap_or(""));
        row(&paths, &format!("{name} executable"), "", Some(&input));
    }

    let terminal = section(&page, "Terminal", None::<&gtk::Widget>);
    let terminal = card(&terminal);
    let size = stepper("setting:terminal.font_size", data["terminal.font_size"].as_f64().unwrap_or(9.75).clamp(8., 24.), 8., 24., 0.25);
    let points = size.adjustment.clone();
    form.setting("terminal.font_size", reader(&size.root, move |_| Some(json!(points.value()))));
    let changed = form.changed(500);
    size.adjustment.connect_value_changed(move |_| changed());
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    controls.append(&size.root);
    controls.append(&label("pt", "settings-unit"));
    row(&terminal, "Font size", "The text size in every agent terminal.", Some(&controls));
}

/// One row per provider from a `provider.list` / `provider.refresh` answer.
fn provider_rows(card: &gtk::Box, detected: &Value) {
    let providers = rows(detected, "providers");
    if providers.is_empty() {
        row(card, "No providers found", "Install Claude Code or Codex, then press Refresh.", None::<&gtk::Widget>);
    }
    for provider in providers {
        let id = text(&provider, "provider");
        let installed = provider["installed"].as_bool() == Some(true);
        let mut name = id.to_string();
        if let Some(first) = name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let mut facts = Vec::new();
        if !text(&provider, "version").is_empty() {
            facts.push(text(&provider, "version").to_string());
        }
        if let Some(account) = provider["signed_in_as"].as_str() {
            facts.push(account.to_string());
        }
        facts.push(if provider["guarded"].as_bool() == Some(true) { "Guardrail adapter".into() } else { "No guardrail adapter".into() });
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        line.add_css_class("settings-row");
        let mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        mark.add_css_class("settings-provider-mark");
        mark.set_valign(gtk::Align::Center);
        mark.set_hexpand(false);
        let glyph = crate::icons::image(id, 16);
        glyph.set_hexpand(true);
        glyph.set_halign(gtk::Align::Center);
        mark.append(&glyph);
        line.append(&mark);
        let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
        copy.set_hexpand(true);
        copy.set_valign(gtk::Align::Center);
        copy.append(&label(&name, "settings-row-title"));
        let hint = label(&facts.join(" · "), "settings-row-hint");
        hint.set_wrap(true);
        copy.append(&hint);
        let path = label(text(&provider, "path"), "settings-path-text");
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_selectable(true);
        path.set_visible(!text(&provider, "path").is_empty());
        copy.append(&path);
        line.append(&copy);
        let state = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        state.add_css_class("settings-state");
        state.set_valign(gtk::Align::Center);
        let lamp = label("", "settings-lamp");
        lamp.set_valign(gtk::Align::Center);
        if installed {
            lamp.add_css_class("on");
        }
        state.append(&lamp);
        state.append(&label(if installed { "Installed" } else { "Not installed" }, "settings-state-text"));
        line.append(&state);
        card.append(&line);
    }
}

fn safety(ui: &Rc<Ui>, stack: &gtk::Stack) {
    let page = category(stack, "safety", None);
    page.append(&crate::pages::guardrail_settings(ui));
}

fn android(stack: &gtk::Stack, form: &Form, data: &Value) {
    let page = category(stack, "android", Some(form));
    let tools = section(&page, "SDK and tools", Some(&label("Empty means discovered", "settings-aside")));
    let tools = card(&tools);
    for (key, title, hint) in [
        ("sdk_path", "Android SDK", "The folder that holds platform-tools and the emulator."),
        ("adb_path", "ADB", "Installs builds and mirrors devices."),
        ("emulator_path", "Emulator", "Starts virtual devices."),
        ("avdmanager_path", "AVD manager", "Creates and removes virtual devices."),
    ] {
        let input = path_entry(form, &format!("device.{key}"), text(&data["device"], key));
        row(&tools, title, hint, Some(&input));
    }
}

fn keyboard(stack: &gtk::Stack, form: &Form, data: &Value) {
    let page = category(stack, "keyboard", Some(form));
    let shortcuts = section(&page, "Shortcuts", Some(&label("Empty turns one off", "settings-aside")));
    let shortcuts = card(&shortcuts);
    for (name, title, fallback) in crate::shortcuts::DEFAULTS {
        let input = gtk::Entry::new();
        input.add_css_class("settings-shortcut");
        input.set_width_chars(14);
        EntryExt::set_alignment(&input, 0.5);
        input.set_placeholder_text(Some("Off"));
        input.set_text(data["keybindings"][name].as_str().unwrap_or(fallback));
        input.set_widget_name(&format!("setting:keybindings.{name}"));
        // A shortcut is saved as typed, empty included: empty turns it off. One that does not
        // parse is held back until it does.
        form.setting(
            format!("keybindings.{name}"),
            reader(&input, |input| {
                let value = input.text();
                let value = value.trim();
                (value.is_empty() || crate::shortcuts::valid(value)).then(|| json!(value))
            }),
        );
        let (_, hint) = row(&shortcuts, title, "", Some(&input));
        let changed = form.changed(700);
        input.connect_changed(move |input| {
            let value = input.text();
            let value = value.trim();
            let valid = value.is_empty() || crate::shortcuts::valid(value);
            if valid {
                input.remove_css_class("invalid");
                hint.set_visible(false);
            } else {
                input.add_css_class("invalid");
                hint.set_text("Use Ctrl+Key notation, like Ctrl+Shift+B.");
                hint.set_visible(true);
            }
            changed();
        });
    }
    let fixed = section(&page, "Built in", None::<&gtk::Widget>);
    let fixed = card(&fixed);
    for (title, keys) in [
        ("Close a sheet or panel", &["Esc"][..]),
        ("Copy in a terminal", &["Ctrl", "Shift", "C"][..]),
        ("Paste in a terminal", &["Ctrl", "Shift", "V"][..]),
    ] {
        let chord = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        for key in keys {
            chord.append(&label(key, "keycap"));
        }
        row(&fixed, title, "", Some(&chord));
    }
}

fn maintenance(ui: &Rc<Ui>, stack: &gtk::Stack) {
    let page = category(stack, "maintenance", None);
    let now = section(&page, "Backups", None::<&gtk::Widget>);
    let now = card(&now);
    let backup = gtk::Button::with_label("Back up now");
    backup.add_css_class("settings-button");
    backup.set_widget_name("settings-backup");
    let (_, status) = row(&now, "Back up the database", "Relay keeps the five most recent copies.", Some(&backup));
    let recent = gtk::Box::new(gtk::Orientation::Vertical, 0);
    recent.add_css_class("settings-card");
    let count = label("", "settings-aside");
    let history = section(&page, "Recent", Some(&count));
    history.append(&recent);
    list_backups(ui, &recent, &count);
    let weak = Rc::downgrade(ui);
    let (list, count) = (recent.downgrade(), count.downgrade());
    backup.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        key.set_sensitive(false);
        let key = key.clone();
        let status = status.clone();
        let (list, count) = (list.clone(), count.clone());
        glib::spawn_future_local(async move {
            match ui.call("app.backup.now", json!({})).await {
                Ok(value) => {
                    let bytes = value["bytes"].as_u64().unwrap_or(0);
                    status.set_text(&format!("Saved {} to {}", size(bytes), text(&value, "path")));
                    if let (Some(list), Some(count)) = (list.upgrade(), count.upgrade()) {
                        list_backups(&ui, &list, &count);
                    }
                }
                Err(error) => status.set_text(&error.to_string()),
            }
            key.set_sensitive(true);
        });
    });
}

/// Fills `card` with the backups on disk, newest first.
fn list_backups(ui: &Rc<Ui>, card: &gtk::Box, count: &gtk::Label) {
    let weak = Rc::downgrade(ui);
    let (card, count) = (card.downgrade(), count.downgrade());
    glib::spawn_future_local(async move {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let result = ui.call("app.backup.list", json!({})).await;
        let (Some(card), Some(count)) = (card.upgrade(), count.upgrade()) else {
            return;
        };
        clear(&card);
        let mut backups = match result {
            Ok(value) => rows(&value, "backups"),
            Err(error) => {
                row(&card, "Cannot list backups", &error.to_string(), None::<&gtk::Widget>);
                return;
            }
        };
        backups.sort_by(|a, b| text(b, "created_at").cmp(text(a, "created_at")));
        count.set_text(&match backups.len() {
            0 => String::new(),
            1 => "1 copy".into(),
            n => format!("{n} copies"),
        });
        if backups.is_empty() {
            row(&card, "No backups yet", "Back up now to keep the first copy.", None::<&gtk::Widget>);
        }
        for backup in backups {
            let when = crate::relative::ago(text(&backup, "created_at"), crate::relative::Form::Long)
                .unwrap_or_else(|| text(&backup, "created_at").to_string());
            let mut title = when;
            if let Some(first) = title.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            if !text(&backup, "reason").is_empty() {
                title = format!("{title} · {}", text(&backup, "reason"));
            }
            let bytes = label(&size(backup["bytes"].as_u64().unwrap_or(0)), "settings-value");
            let (_, hint) = row(&card, &title, text(&backup, "path"), Some(&bytes));
            hint.add_css_class("settings-path-text");
            hint.set_wrap(false);
            hint.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            hint.set_selectable(true);
        }
    });
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / f64::from(1u32 << 30)),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / f64::from(1u32 << 20)),
        b if b >= 1 << 10 => format!("{:.0} KB", b as f64 / 1024.),
        b => format!("{b} B"),
    }
}

/// Search hides the categories with no match and shows the first that has one.
fn wire_search(search: &gtk::SearchEntry, shell: &gtk::Box, nav: &gtk::Box, stack: &gtk::Stack) {
    let no_results = gtk::Box::new(gtk::Orientation::Vertical, 6);
    no_results.add_css_class("settings-empty");
    no_results.set_hexpand(true);
    no_results.append(&label("No settings match", "settings-row-title"));
    no_results.append(&label("Try another word, like wallpaper or shortcut.", "settings-row-hint"));
    no_results.set_visible(false);
    shell.append(&no_results);
    let mut keys = Vec::new();
    let mut child = nav.first_child();
    for Category { name, .. } in CATEGORIES {
        let Some(key) = child.take().and_downcast::<gtk::ToggleButton>() else {
            break;
        };
        child = key.next_sibling();
        let text = stack
            .child_by_name(name)
            .map(|page| settings_search_text(&page))
            .unwrap_or_default();
        keys.push((name, key, text));
    }
    let weak_stack = stack.downgrade();
    search.connect_search_changed(move |search| {
        let Some(stack) = weak_stack.upgrade() else {
            return;
        };
        let query = search.text().to_lowercase();
        let words: Vec<_> = query.split_whitespace().collect();
        let mut first = None;
        let mut current_matches = false;
        for (name, key, text) in &keys {
            let matches = words.iter().all(|word| text.contains(word));
            key.set_visible(matches);
            if matches {
                first.get_or_insert(key);
                current_matches |= stack.visible_child_name().as_deref() == Some(*name);
            }
        }
        stack.set_visible(first.is_some());
        no_results.set_visible(first.is_none());
        if !current_matches {
            if let Some(first) = first {
                first.set_active(true);
            }
        }
    });
}

fn settings_search_text(widget: &gtk::Widget) -> String {
    let mut result = String::new();
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        result.push_str(&label.text());
    }
    if widget.widget_name().starts_with("setting:") {
        result.push_str(&widget.widget_name());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        result.push(' ');
        result.push_str(&settings_search_text(&widget));
        child = widget.next_sibling();
    }
    result.to_lowercase()
}

/// One settings category: its key in the list and the heading of its page.
struct Category {
    name: &'static str,
    title: &'static str,
    /// The key's glyph, on the 16-unit grid `icons::from_geometry` paints.
    icon: &'static str,
    /// The line under the title in the category list.
    nav: &'static str,
    /// The line under the page's title.
    lede: &'static str,
}

const CATEGORIES: [Category; 7] = [
    Category {
        name: "appearance",
        title: "Appearance",
        icon: r##"<circle cx="8" cy="8" r="5.75" /><path d="M8 2.25a5.75 5.75 0 0 0 0 11.5z" fill="currentColor" stroke="none" />"##,
        nav: "Theme, opacity, wallpaper",
        lede: "Theme, wallpaper, and how much of it shows behind your terminals.",
    },
    Category {
        name: "notifications",
        title: "Notifications",
        icon: r##"<path d="M4 11.5V7a4 4 0 0 1 8 0v4.5l1 1H3z" /><path d="M6.75 14h2.5" />"##,
        nav: "Sounds and categories",
        lede: "Let the important agent moments reach you and keep routine activity quiet.",
    },
    Category {
        name: "agents",
        title: "Agents",
        icon: r##"<rect x="2" y="3" width="12" height="10" rx="2" /><path d="M5 6.5l2 1.5-2 1.5M8.5 10h2.5" />"##,
        nav: "Providers and updates",
        lede: "The agent CLIs on this machine, how they update, and their terminal.",
    },
    Category {
        name: "safety",
        title: "Guardrails",
        icon: r##"<path d="M8 2l5 2v3.5c0 3.3-2 5.5-5 6.8-3-1.3-5-3.5-5-6.8V4z" />"##,
        nav: "Caps and protected paths",
        lede: "Hard limits the engine enforces on every agent: set them for everything, one workspace or one project.",
    },
    Category {
        name: "android",
        title: "Android",
        icon: r##"<rect x="4.25" y="1.75" width="7.5" height="12.5" rx="1.75" /><path d="M7.25 12h1.5" />"##,
        nav: "SDK and device tools",
        lede: "Relay finds the standard SDK locations on its own. Set a path only for an unusual install.",
    },
    Category {
        name: "keyboard",
        title: "Keyboard",
        icon: r##"<rect x="1.75" y="4" width="12.5" height="8" rx="1.75" /><path d="M4.5 6.75h.01M7 6.75h.01M9.5 6.75h.01M12 6.75h.01M5.5 9.5h5" />"##,
        nav: "Global shortcuts",
        lede: "Shortcuts that work anywhere in Relay.",
    },
    Category {
        name: "maintenance",
        title: "Storage",
        icon: r##"<path d="M3 4a5 2 0 1 0 10 0a5 2 0 1 0 -10 0" /><path d="M3 4v8c0 1.1 2.2 2 5 2s5-.9 5-2V4M3 8c0 1.1 2.2 2 5 2s5-.9 5-2" />"##,
        nav: "Backups",
        lede: "Keep a copy of Relay's database before big workflow changes.",
    },
];

const PLAY: &str = r##"<path d="M5 3.5l7.5 4.5L5 12.5z" fill="currentColor" />"##;
const SPEAKER: &str = r##"<path d="M2.5 6h2.5l3.5-3v10L5 10H2.5z" />"##;
const EXPAND: &str = r##"<path d="M9.5 2.5h4v4M13.5 2.5L9.25 6.75M6.5 13.5h-4v-4M2.5 13.5l4.25-4.25" />"##;

/// The page of category `name`, which must be one of [`CATEGORIES`], added to `stack`: its
/// title, its line and, for a category that saves on its own, the save caption.
fn category(stack: &gtk::Stack, name: &str, autosave: Option<&Form>) -> gtk::Box {
    let Category { title, lede, .. } = CATEGORIES.iter().find(|c| c.name == name).expect("a listed settings category");
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.add_css_class("settings-column");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    head.add_css_class("settings-head");
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 8);
    copy.set_hexpand(true);
    copy.append(&label(title, "settings-title"));
    let line = label(lede, "settings-lede");
    line.set_wrap(true);
    copy.append(&line);
    head.append(&copy);
    if let Some(form) = autosave {
        let caption = label(IDLE, "settings-caption");
        caption.set_valign(gtk::Align::End);
        caption.set_xalign(1.0);
        form.autosave.captions.borrow_mut().push(caption.downgrade());
        head.append(&caption);
    }
    column.append(&head);
    let scroll = crate::app::scrolled(&column);
    // External, not Never: the margin that caps the column follows the page's width, and Never
    // counted it in the page's minimum, so the window narrowed only a step at a time.
    scroll.set_policy(gtk::PolicyType::External, gtk::PolicyType::Automatic);
    cap_width(&column, &scroll);
    stack.add_titled(&scroll, Some(name), title);
    column
}

/// Lets `column` fill `scroll` up to [`COLUMN_MAX`] px of content. GTK has no max-width, so a
/// right margin takes up whatever the visible width exceeds it by, following every resize.
fn cap_width(column: &gtk::Box, scroll: &gtk::ScrolledWindow) {
    let target = column.downgrade();
    let fit = move |adjustment: &gtk::Adjustment| {
        if let Some(column) = target.upgrade() {
            let spare = (adjustment.page_size() as i32 - COLUMN_PADDING - COLUMN_MAX).max(0);
            if column.margin_end() != spare {
                column.set_margin_end(spare);
            }
        }
    };
    let adjustment = scroll.hadjustment();
    fit(&adjustment);
    adjustment.connect_page_size_notify(fit);
}

/// A titled group: the heading, with `aside` on its right, over whatever the caller adds.
pub(crate) fn section(parent: &gtk::Box, title: &str, aside: Option<&impl IsA<gtk::Widget>>) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 12);
    block.add_css_class("settings-section");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    head.add_css_class("settings-section-head");
    let name = label(title, "settings-section-title");
    name.set_hexpand(true);
    name.set_valign(gtk::Align::Center);
    head.append(&name);
    if let Some(aside) = aside {
        aside.set_valign(gtk::Align::Center);
        head.append(aside);
    }
    block.append(&head);
    parent.append(&block);
    block
}

/// The rounded card a section's rows sit in.
pub(crate) fn card(section: &gtk::Box) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("settings-card");
    section.append(&card);
    card
}

/// A row: title over a hint on the left, `control` on the right. The hint label is returned
/// (hidden when empty) for a row that reports into it.
pub(crate) fn row(card: &gtk::Box, title: &str, hint: &str, control: Option<&impl IsA<gtk::Widget>>) -> (gtk::Box, gtk::Label) {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    line.add_css_class("settings-row");
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 4);
    copy.set_hexpand(true);
    copy.set_valign(gtk::Align::Center);
    copy.append(&label(title, "settings-row-title"));
    let note = label(hint, "settings-row-hint");
    note.set_wrap(true);
    note.set_visible(!hint.is_empty());
    copy.append(&note);
    line.append(&copy);
    if let Some(control) = control {
        control.set_valign(gtk::Align::Center);
        line.append(control);
    }
    card.append(&line);
    (line, note)
}

fn toggle(name: &str, active: bool) -> gtk::Switch {
    let switch = gtk::Switch::new();
    switch.add_css_class("settings-switch");
    switch.set_widget_name(name);
    switch.set_active(active);
    switch.set_valign(gtk::Align::Center);
    switch
}

/// A slider with its value as a percentage beside it.
fn slider(min: f64, max: f64, value: f64) -> (gtk::Box, gtk::Scale) {
    let control = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    control.add_css_class("settings-slider");
    let input = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, 0.01);
    input.set_value(value);
    input.set_draw_value(false);
    input.set_size_request(200, -1);
    input.set_valign(gtk::Align::Center);
    let shown = label(&format!("{:.0}%", input.value() * 100.), "settings-value");
    shown.set_width_chars(4);
    shown.set_xalign(1.0);
    control.append(&input);
    control.append(&shown);
    input.connect_value_changed(move |input| shown.set_text(&format!("{:.0}%", input.value() * 100.)));
    (control, input)
}

/// A number with − and + keys either side, typed into as well: `adjustment` holds its value.
struct Stepper {
    root: gtk::Box,
    adjustment: gtk::Adjustment,
}

fn stepper(name: &str, value: f64, min: f64, max: f64, step: f64) -> Stepper {
    let adjustment = gtk::Adjustment::new(value, min, max, step, step * 4., 0.);
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("settings-stepper");
    root.set_widget_name(name);
    let less = gtk::Button::new();
    less.set_child(Some(&crate::icons::image("minus", 12)));
    less.set_tooltip_text(Some("Less"));
    let shown = gtk::Entry::new();
    shown.set_width_chars(if step < 1. { 5 } else { 4 });
    shown.set_max_width_chars(6);
    EntryExt::set_alignment(&shown, 0.5);
    let more = gtk::Button::new();
    more.set_child(Some(&crate::icons::image("plus", 12)));
    more.set_tooltip_text(Some("More"));
    let number = |value: f64| {
        let text = format!("{value:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let show = {
        let (shown, less, more) = (shown.downgrade(), less.downgrade(), more.downgrade());
        move |adjustment: &gtk::Adjustment| {
            if let (Some(shown), Some(less), Some(more)) = (shown.upgrade(), less.upgrade(), more.upgrade()) {
                shown.set_text(&number(adjustment.value()));
                less.set_sensitive(adjustment.value() > adjustment.lower());
                more.set_sensitive(adjustment.value() < adjustment.upper());
            }
        }
    };
    show(&adjustment);
    adjustment.connect_value_changed(show.clone());
    let a = adjustment.clone();
    less.connect_clicked(move |_| a.set_value(a.value() - a.step_increment()));
    let a = adjustment.clone();
    more.connect_clicked(move |_| a.set_value(a.value() + a.step_increment()));
    // Typed: taken on Enter or on leaving the field, rounded to the step and clamped.
    let commit = {
        let a = adjustment.clone();
        let show = show.clone();
        move |entry: &gtk::Entry| {
            if let Ok(typed) = entry.text().trim().parse::<f64>() {
                let step = a.step_increment();
                a.set_value(((typed / step).round() * step).clamp(a.lower(), a.upper()));
            }
            show(&a);
        }
    };
    let typed = commit.clone();
    shown.connect_activate(move |entry| typed(entry));
    let focus = gtk::EventControllerFocus::new();
    let entry = shown.downgrade();
    focus.connect_leave(move |_| {
        if let Some(entry) = entry.upgrade() {
            commit(&entry);
        }
    });
    shown.add_controller(focus);
    root.append(&less);
    root.append(&shown);
    root.append(&more);
    Stepper { root, adjustment }
}

/// An override path: empty saves null, which means "discover it".
fn path_entry(form: &Form, path: &str, value: &str) -> gtk::Entry {
    let input = gtk::Entry::builder().text(value).placeholder_text("Automatic").width_chars(26).build();
    input.add_css_class("settings-path");
    input.set_widget_name(&format!("setting:{path}"));
    form.setting(path, reader(&input, |input| {
        let value = input.text();
        let value = value.trim();
        Some(if value.is_empty() { Value::Null } else { json!(value) })
    }));
    // A half-typed path is not worth saving: wait for a pause, Enter or leaving the field.
    let changed = form.changed(1500);
    input.connect_changed(move |_| changed());
    let now = form.changed(0);
    input.connect_activate({
        let now = now.clone();
        move |_| now()
    });
    let focus = gtk::EventControllerFocus::new();
    focus.connect_leave(move |_| now());
    input.add_controller(focus);
    input
}

/// What the wallpaper stage draws over the image: the chosen mode's panels at the chosen
/// opacity, dim and protection, redrawn as the controls move, before anything is saved.
struct Look {
    mode: RefCell<String>,
    alpha: Cell<f64>,
    dim: Cell<f64>,
    contrast: Cell<f64>,
    stage: RefCell<glib::WeakRef<gtk::DrawingArea>>,
}

impl Look {
    fn redraw(&self) {
        if let Some(stage) = self.stage.borrow().upgrade() {
            stage.queue_draw();
        }
    }

    /// The panels' opacity as shell.rs `load_appearance` works it out for @slab.
    fn panel_alpha(&self) -> f64 {
        let alpha = self.alpha.get().clamp(0.5, 1.);
        (alpha + (1. - alpha) * self.contrast.get().clamp(0., 1.) + 0.04).min(1.)
    }
}

fn rgb(hex: &str) -> (f64, f64, f64) {
    let channel = |i: usize| f64::from(u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0)) / 255.;
    (channel(1), channel(3), channel(5))
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.);
    cr.arc(x + w - r, y + h - r, r, 0., FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 3. * FRAC_PI_2);
    cr.close_path();
}

/// A theme card's miniature: the mode's console around a sidebar and a pane.
fn draw_theme(cr: &gtk::cairo::Context, mode: &str, w: f64, h: f64) {
    let colors = crate::fonts::palette(mode);
    // Matte and Warm dark: their raised console around ground-toned panes. OLED: true black
    // around barely lifted ones.
    let (frame, pane, line) = match mode {
        "oled" => (rgb(colors[0]), rgb(colors[2]), rgb(colors[9])),
        _ => (rgb(colors[3]), rgb(colors[1]), rgb(colors[9])),
    };
    rounded(cr, 0., 0., w, h, 9.);
    cr.set_source_rgb(frame.0, frame.1, frame.2);
    let _ = cr.fill();
    let pad = 12.;
    let side = (w * 0.24).min(76.);
    for (x, width) in [(pad, side), (pad * 1.75 + side, w - pad * 2.75 - side)] {
        let fill = if mode == "oled" && x > pad { rgb("#000000") } else { pane };
        rounded(cr, x + 0.5, pad + 0.5, width - 1., h - pad * 2. - 1., 5.);
        cr.set_source_rgb(fill.0, fill.1, fill.2);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(line.0 + 0.1, line.1 + 0.1, line.2 + 0.1, 1.);
        cr.set_line_width(1.);
        let _ = cr.stroke();
    }
}

/// The stage's mock workspace: a sidebar column and four panes, the first with a few lines.
fn draw_stage(cr: &gtk::cairo::Context, look: &Look, w: f64, h: f64) {
    cr.set_source_rgba(0., 0., 0., look.dim.get().clamp(0., 0.85));
    let _ = cr.paint();
    let colors = crate::fonts::palette(&look.mode.borrow());
    let (panel, ink, edge) = (rgb(colors[2]), rgb(colors[5]), rgb(colors[9]));
    let alpha = look.panel_alpha();
    let pad = 24.;
    let gap = 12.;
    let side = ((w - pad * 2.) * 0.22).max(60.);
    let right = w - pad * 2. - side - gap;
    let column = (right - gap) / 2.;
    let top = (h - pad * 2. - gap) * 0.62;
    let bottom = h - pad * 2. - gap - top;
    let panes = [
        (pad, pad, side, h - pad * 2.),
        (pad + side + gap, pad, column, top),
        (pad + side + gap * 2. + column, pad, column, top),
        (pad + side + gap, pad + top + gap, column, bottom),
        (pad + side + gap * 2. + column, pad + top + gap, column, bottom),
    ];
    for (x, y, pw, ph) in panes {
        rounded(cr, x + 0.5, y + 0.5, pw - 1., ph - 1., 9.);
        cr.set_source_rgba(panel.0, panel.1, panel.2, alpha);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(edge.0 + 0.06, edge.1 + 0.06, edge.2 + 0.06, 0.7);
        cr.set_line_width(1.);
        let _ = cr.stroke();
    }
    let (x, y) = (pad + side + gap + 18., pad + 18.);
    for (index, (width, opacity)) in [(0.36, 0.85), (0.72, 0.4), (0.58, 0.4)].into_iter().enumerate() {
        rounded(cr, x, y + index as f64 * 15., (column - 36.) * width, 5., 2.5);
        cr.set_source_rgba(ink.0, ink.1, ink.2, opacity);
        let _ = cr.fill();
    }
}

fn render_wallpapers(ui: &Rc<Ui>, block: &gtk::Box, state: &Rc<RefCell<Value>>, look: &Rc<Look>) {
    // Native selection follows the canonical image. Legacy wallpaper_id/preview are unused.
    clear(block);
    let library = state.borrow()["wallpapers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let active = state.borrow()["wallpaper"].clone();
    let current_image = active.as_str().unwrap_or("");
    let current = library
        .iter()
        .find(|item| item["image"].as_str() == Some(current_image));
    let texture = match current {
        Some(item) => item_texture(item),
        None => wallpaper_texture(current_image),
    };
    let name = current
        .and_then(|item| item["name"].as_str())
        .unwrap_or(if texture.is_some() { "Wallpaper" } else { "No wallpaper" });
    // Forget images no longer in the library.
    TEXTURES.with(|t| {
        t.borrow_mut().retain(|id, _| library.iter().any(|item| item["id"].as_str() == Some(id.as_str())))
    });

    let clear_key = gtk::Button::with_label("Clear background");
    clear_key.add_css_class("settings-link");
    clear_key.set_widget_name("settings-wallpaper-clear");
    clear_key.set_visible(active.is_string());
    let section = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    head.add_css_class("settings-section-head");
    let title = label("Wallpaper", "settings-section-title");
    title.set_hexpand(true);
    head.append(&title);
    head.append(&clear_key);
    section.append(&head);
    block.append(&section);
    let weak = Rc::downgrade(ui);
    let target = block.downgrade();
    let (staged, shown) = (state.clone(), look.clone());
    clear_key.connect_clicked(move |_| {
        let (Some(ui), Some(block)) = (weak.upgrade(), target.upgrade()) else {
            return;
        };
        staged.borrow_mut()["wallpaper"] = Value::Null;
        render_wallpapers(&ui, &block, &staged, &shown);
        wallpaper_changed();
    });

    // The stage: the image, the look over it, and its name and Full preview on top.
    let stage = gtk::Overlay::new();
    stage.add_css_class("settings-stage");
    stage.set_overflow(gtk::Overflow::Hidden);
    stage.set_widget_name("settings-wallpaper-preview");
    let ground = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ground.add_css_class("settings-stage-ground");
    ground.set_size_request(-1, 260);
    stage.set_child(Some(&ground));
    if let Some(texture) = &texture {
        let picture = gtk::Picture::for_paintable(texture);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        stage.add_overlay(&picture);
    }
    let area = gtk::DrawingArea::new();
    area.set_can_target(false);
    let drawn = look.clone();
    area.set_draw_func(move |_, cr, w, h| draw_stage(cr, &drawn, f64::from(w), f64::from(h)));
    *look.stage.borrow_mut() = area.downgrade();
    stage.add_overlay(&area);
    let caption = label(&format!("{name} · live preview"), "settings-chip");
    caption.set_halign(gtk::Align::Start);
    caption.set_valign(gtk::Align::End);
    stage.add_overlay(&caption);
    if let Some(texture) = texture {
        let open = gtk::Button::new();
        open.add_css_class("settings-chip-key");
        open.set_widget_name("settings-wallpaper-open");
        let words = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        words.append(&crate::icons::from_geometry(EXPAND, 12, 1.5));
        words.append(&label("Full preview", ""));
        open.set_child(Some(&words));
        open.set_halign(gtk::Align::End);
        open.set_valign(gtk::Align::End);
        let weak = Rc::downgrade(ui);
        let name = name.to_string();
        open.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let window = gtk::Window::builder()
                .title(&name)
                .transient_for(&ui.window)
                .modal(true)
                .default_width((ui.window.width() - 80).clamp(480, 1200))
                .default_height((ui.window.height() - 80).clamp(320, 800))
                .build();
            let picture = gtk::Picture::for_paintable(&texture);
            picture.set_content_fit(gtk::ContentFit::Contain);
            picture.set_can_shrink(true);
            window.set_child(Some(&picture));
            window.present();
        });
        stage.add_overlay(&open);
    }
    section.append(&stage);

    let gallery = gtk::FlowBox::new();
    gallery.set_selection_mode(gtk::SelectionMode::None);
    gallery.set_homogeneous(true);
    gallery.set_min_children_per_line(2);
    gallery.set_max_children_per_line(4);
    gallery.set_column_spacing(12);
    gallery.set_row_spacing(14);
    gallery.add_css_class("settings-wallpapers");
    for (index, item) in library.iter().enumerate() {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("settings-wallpaper");
        let frame = gtk::Overlay::new();
        let pick = gtk::Button::new();
        pick.add_css_class("settings-thumb");
        pick.set_overflow(gtk::Overflow::Hidden);
        pick.set_widget_name(&format!("settings-wallpaper-pick-{index}"));
        pick.set_tooltip_text(Some(text(item, "name")));
        let selected = item["image"] == active;
        if selected {
            pick.add_css_class("active");
        }
        // The picture is an overlay on a fixed ground: an overlay is not measured, so the
        // image's own size never widens the card past its share of the row.
        let face = gtk::Overlay::new();
        let ground = gtk::Box::new(gtk::Orientation::Vertical, 0);
        ground.set_size_request(120, 90);
        face.set_child(Some(&ground));
        match item_texture(item) {
            Some(texture) => {
                let picture = gtk::Picture::for_paintable(&texture);
                picture.set_content_fit(gtk::ContentFit::Cover);
                picture.set_can_shrink(true);
                face.add_overlay(&picture);
            }
            None => {
                let missing = label("No preview", "settings-thumb-missing");
                missing.set_halign(gtk::Align::Center);
                face.add_overlay(&missing);
            }
        }
        pick.set_child(Some(&face));
        frame.set_child(Some(&pick));
        let weak = Rc::downgrade(ui);
        let target = block.downgrade();
        let (staged, shown) = (state.clone(), look.clone());
        let image = item["image"].clone();
        pick.connect_clicked(move |_| {
            let (Some(ui), Some(block)) = (weak.upgrade(), target.upgrade()) else {
                return;
            };
            staged.borrow_mut()["wallpaper"] = image.clone();
            render_wallpapers(&ui, &block, &staged, &shown);
            wallpaper_changed();
        });
        if !selected {
            let remove = gtk::Button::new();
            remove.add_css_class("settings-thumb-remove");
            remove.set_child(Some(&crate::icons::image_with_stroke("close", 10, 2.)));
            remove.set_tooltip_text(Some("Remove from the library"));
            remove.set_widget_name(&format!("settings-wallpaper-remove-{index}"));
            remove.set_halign(gtk::Align::End);
            remove.set_valign(gtk::Align::Start);
            let weak = Rc::downgrade(ui);
            let target = block.downgrade();
            let (staged, shown) = (state.clone(), look.clone());
            remove.connect_clicked(move |_| {
                let (Some(ui), Some(block)) = (weak.upgrade(), target.upgrade()) else {
                    return;
                };
                let mut value = staged.borrow_mut();
                if let Some(library) = value["wallpapers"].as_array_mut().filter(|l| index < l.len()) {
                    library.remove(index);
                }
                drop(value);
                render_wallpapers(&ui, &block, &staged, &shown);
                wallpaper_changed();
            });
            frame.add_overlay(&remove);
        }
        card.append(&frame);
        let name = label(text(item, "name"), "settings-thumb-name");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        card.append(&name);
        gallery.insert(&card, -1);
    }
    let add = gtk::Button::new();
    add.add_css_class("settings-thumb-add");
    add.set_widget_name("settings-wallpaper-add");
    let words = gtk::Box::new(gtk::Orientation::Vertical, 6);
    words.set_valign(gtk::Align::Center);
    let plus = crate::icons::image("plus", 14);
    words.append(&plus);
    let caption = label("Add wallpaper", "");
    caption.set_xalign(0.5);
    words.append(&caption);
    add.set_child(Some(&words));
    add.set_size_request(120, 92);
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tile.append(&add);
    gallery.insert(&tile, -1);
    section.append(&gallery);
    let weak = Rc::downgrade(ui);
    let target = block.downgrade();
    let (staged, shown) = (state.clone(), look.clone());
    add.connect_clicked(move |key| {
        let (Some(ui), Some(block)) = (weak.upgrade(), target.upgrade()) else {
            return;
        };
        let key = key.clone();
        key.set_sensitive(false);
        let (staged, shown) = (staged.clone(), shown.clone());
        glib::spawn_future_local(async move {
            let result = async {
                let dialog = gtk::FileDialog::builder().title("Choose wallpaper").build();
                // Closing the chooser is not an error.
                let file = match dialog.open_future(Some(&ui.window)).await {
                    Ok(file) => file,
                    Err(e) if e.matches(gtk::DialogError::Dismissed) || e.matches(gtk::DialogError::Cancelled) => return Ok(false),
                    Err(e) => return Err(e.to_string()),
                };
                let path = file.path().ok_or("Choose a local image")?;
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let bytes = ui.rt.spawn_blocking(move || {
                    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                    if meta.len() > 32 * 1024 * 1024 {
                        return Err("Choose an image smaller than 32 MB".to_string());
                    }
                    let image = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 1280, 900, true).map_err(|e| e.to_string())?;
                    image.save_to_bufferv("jpeg", &[("quality", "80")]).map_err(|e| e.to_string())
                }).await.map_err(|e| e.to_string())??;
                use base64::Engine;
                let image = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
                let mut library = staged.borrow()["wallpapers"].as_array().cloned().unwrap_or_default();
                // A legacy `preview` was a second copy of `image` and counted twice against the cap.
                for item in &mut library {
                    if let Some(item) = item.as_object_mut() {
                        item.remove("preview");
                    }
                }
                library.push(json!({"id":uuid::Uuid::new_v4().to_string(),"name":name,"image":image}));
                if serde_json::to_vec(&library).unwrap_or_default().len() > 1500000 {
                    return Err("Wallpaper library is full. Remove an image before adding another.".into());
                }
                staged.borrow_mut()["wallpapers"] = json!(library);
                staged.borrow_mut()["wallpaper"] = json!(image);
                Ok::<bool, String>(true)
            }.await;
            match result {
                Err(e) => ui.show_error(&e),
                Ok(true) => {
                    render_wallpapers(&ui, &block, &staged, &shown);
                    wallpaper_changed();
                }
                Ok(false) => {}
            }
            key.set_sensitive(true);
        });
    });
}

thread_local! {
    /// Decoded library images by id: a gallery redraw (a pick, a removal, a new saved
    /// wallpaper) otherwise decodes every full-size image again on the UI thread.
    static TEXTURES: RefCell<std::collections::HashMap<String, gtk::gdk::Texture>> = RefCell::default();
}

fn item_texture(item: &Value) -> Option<gtk::gdk::Texture> {
    let image = item["image"].as_str()?;
    let Some(id) = item["id"].as_str() else {
        return wallpaper_texture(image);
    };
    if let Some(texture) = TEXTURES.with(|t| t.borrow().get(id).cloned()) {
        return Some(texture);
    }
    let texture = wallpaper_texture(image)?;
    TEXTURES.with(|t| t.borrow_mut().insert(id.to_string(), texture.clone()));
    Some(texture)
}

fn wallpaper_texture(image: &str) -> Option<gtk::gdk::Texture> {
    use base64::Engine;
    let (_, encoded) = image.split_once(',')?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    gtk::gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)).ok()
}
