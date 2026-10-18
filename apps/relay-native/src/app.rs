use crate::client::{Client, Error, Notice};
use crate::terminal::Pane;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use tokio::runtime::Handle;
#[path = "launch.rs"]
mod launch;
#[path = "onboarding.rs"]
mod onboarding;
#[path = "shell.rs"]
mod shell;
#[path = "status.rs"]
mod status;
#[path = "gaming_key.rs"]
mod gaming_key;

pub fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
pub fn rows(v: &Value, key: &str) -> Vec<Value> {
    v[key].as_array().cloned().unwrap_or_default()
}
pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}
pub fn button(text: &str, class: &str) -> gtk::Button {
    let b = gtk::Button::with_label(text);
    if !class.is_empty() {
        b.add_css_class(class);
    }
    if matches!(class, "nav" | "project" | "file") {
        if let Some(l) = b.child().and_downcast::<gtk::Label>() {
            l.set_xalign(0.0);
        }
    }
    b
}
pub fn icon_button(icon: &str, caption: &str) -> gtk::Button {
    let b = gtk::Button::new();
    b.set_child(Some(&crate::icons::image(icon, 16)));
    b.add_css_class("quiet");
    b.add_css_class("icon-key");
    b.set_tooltip_text(Some(caption));
    b.update_property(&[gtk::accessible::Property::Label(caption)]);
    b
}
/// The person's first name, from their account's real name, else their user name.
pub fn first_name() -> String {
    let real = glib::real_name().to_string_lossy().trim().to_string();
    let first = real.split_whitespace().next().filter(|w| !w.eq_ignore_ascii_case("unknown")).map(str::to_string);
    first.unwrap_or_else(|| glib::user_name().to_string_lossy().to_string())
}

pub fn nav_button(caption: &str, icon: &str) -> gtk::Button {
    let b = button("", "nav");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    let image = crate::icons::image(icon, 18);
    row.append(&image);
    row.append(&label(caption, "nav-label"));
    b.set_child(Some(&row));
    b
}
/// A spinner that spins only while it is on screen. A spinning spinner keeps the frame clock at
/// the display rate for as long as it stays realized, hidden or not — a loading state the stack
/// had moved past held Skills and Plugins at 60 frames/s at rest (BASELINE-2026-10-14).
pub fn spinner() -> gtk::Spinner {
    let spinner = gtk::Spinner::new();
    spinner.connect_map(|spinner| spinner.start());
    spinner.connect_unmap(|spinner| spinner.stop());
    spinner
}
pub fn clear(container: &gtk::Box) {
    while let Some(w) = container.first_child() {
        container.remove(&w);
    }
}
/// A popover open somewhere under `root`. A page rebuilt under an open popover closes it
/// under the pointer, so rebuilds wait for this to close.
pub fn open_popover(root: &gtk::Widget) -> Option<gtk::Popover> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(popover) = widget.downcast_ref::<gtk::Popover>().filter(|p| p.is_visible()) {
            return Some(popover.clone());
        }
        if let Some(found) = open_popover(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .child(child)
        .hexpand(true)
        .vexpand(true)
        .build()
}
/// Calls `on_width` with `parent`'s width whenever it changes, after layout: a probe of no height
/// at the top of `parent` reports it. A page sizes its columns from this instead of a fixed
/// request, so it never holds the window wider than the person made it.
pub fn watch_width(parent: &gtk::Box, on_width: impl Fn(i32) + 'static) {
    let probe = gtk::DrawingArea::new();
    probe.set_content_height(0);
    probe.set_hexpand(true);
    probe.set_can_target(false);
    probe.set_focusable(false);
    let seen = Rc::new(Cell::new(-1));
    let on_width = Rc::new(on_width);
    probe.connect_resize(move |_, width, _| {
        if seen.replace(width) == width {
            return;
        }
        // Never resize inside size-allocate: the next frame takes the new requests.
        let on_width = on_width.clone();
        glib::idle_add_local_once(move || on_width(width));
    });
    parent.prepend(&probe);
}
pub fn field(caption: &str, widget: &impl IsA<gtk::Widget>, parent: &gtk::Box) {
    parent.append(&label(caption, "dim"));
    parent.append(widget);
}

#[path = "notification_center.rs"]
mod notification_center;
#[path = "session_context.rs"]
mod session_context;

thread_local! {
    /// Bumped by every `Ui::show_error`, so a newer message is never cleared by an older timer.
    static NOTICE_SERIAL: Cell<u64> = const { Cell::new(0) };
}

/// How long an armed key waits for its second click before it reverts.
const CONFIRM_WINDOW: std::time::Duration = std::time::Duration::from_secs(4);
/// How often the open project is fetched, so a pane's "behind" count sees what landed on GitHub.
const DRIFT_FETCH_SECS: u32 = 300;

/// A destructive key that confirms in place instead of opening a dialog.
///
/// The first click arms it: the key turns red and reads `caption`. A second click runs `run`.
/// Waiting [`CONFIRM_WINDOW`], moving focus elsewhere or pressing Escape reverts it untouched.
/// Use a real dialog only where the action needs more input than a yes.
pub fn confirm_inline(key: &gtk::Button, caption: &str, run: impl Fn(&gtk::Button) + 'static) {
    confirm_inline_if(key, caption, || true, run);
}

/// [`confirm_inline`], asking only while `needed` holds (a discard with nothing to lose runs at once).
pub fn confirm_inline_if(
    key: &gtk::Button,
    caption: &str,
    needed: impl Fn() -> bool + 'static,
    run: impl Fn(&gtk::Button) + 'static,
) {
    struct Armed {
        label: Option<glib::GString>,
        child: Option<gtk::Widget>,
        tooltip: Option<glib::GString>,
        width: i32,
    }
    let armed: Rc<RefCell<Option<Armed>>> = Rc::default();
    let serial = Rc::new(Cell::new(0_u64));
    let disarm: Rc<dyn Fn(&gtk::Button)> = Rc::new({
        let armed = armed.clone();
        move |key: &gtk::Button| {
            let Some(previous) = armed.borrow_mut().take() else { return };
            match &previous.label {
                Some(text) => key.set_label(text),
                None => key.set_child(previous.child.as_ref()),
            }
            key.set_tooltip_text(previous.tooltip.as_deref());
            key.set_size_request(previous.width, -1);
            key.remove_css_class("confirm-armed");
            key.reset_property(gtk::AccessibleProperty::Label);
            // An icon key names itself through its caption; a text key through its text.
            if previous.label.is_none() {
                if let Some(caption) = previous.tooltip.as_deref() {
                    key.update_property(&[gtk::accessible::Property::Label(caption)]);
                }
            }
        }
    });
    let caption = caption.to_string();
    let reset = disarm.clone();
    let state = armed.clone();
    key.connect_clicked(move |key| {
        serial.set(serial.get().wrapping_add(1));
        if state.borrow().is_some() {
            reset(key);
            run(key);
            return;
        }
        if !needed() {
            run(key);
            return;
        }
        let text = key.label();
        *state.borrow_mut() = Some(Armed {
            child: if text.is_none() { key.child() } else { None },
            label: text,
            tooltip: key.tooltip_text(),
            width: key.size_request().0,
        });
        // Never narrower than the key it replaces, so the row around it does not jump.
        key.set_size_request(key.width().max(key.size_request().0), -1);
        if key.label().is_some() {
            key.set_label(&caption);
        } else {
            let text = label(&caption, "confirm-caption");
            text.set_xalign(0.5);
            key.set_child(Some(&text));
        }
        key.add_css_class("confirm-armed");
        key.set_tooltip_text(Some("Click again to confirm · Esc cancels"));
        key.update_property(&[gtk::accessible::Property::Label(&caption)]);
        let current = serial.get();
        let serial = serial.clone();
        let key = key.downgrade();
        let reset = reset.clone();
        glib::timeout_add_local_once(CONFIRM_WINDOW, move || {
            if let Some(key) = key.upgrade().filter(|_| serial.get() == current) {
                reset(&key);
            }
        });
    });
    let focus = gtk::EventControllerFocus::new();
    let reset = disarm.clone();
    let weak = key.downgrade();
    focus.connect_leave(move |_| {
        if let Some(key) = weak.upgrade() {
            reset(&key);
        }
    });
    key.add_controller(focus);
    let keys = gtk::EventControllerKey::new();
    let weak = key.downgrade();
    keys.connect_key_pressed(move |_, pressed, _, _| {
        if pressed == gtk::gdk::Key::Escape && armed.borrow().is_some() {
            if let Some(key) = weak.upgrade() {
                disarm(&key);
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    key.add_controller(keys);
}

fn track_navigation(stack: &gtk::Stack, key: &gtk::Button, name: &'static str) {
    let weak = key.downgrade();
    stack.connect_visible_child_name_notify(move |s| {
        if let Some(key) = weak.upgrade() {
            if s.visible_child_name().as_deref() == Some(name) {
                key.add_css_class("selected");
            } else {
                key.remove_css_class("selected");
            }
        }
    });
}

/// Whether a `settings.changed` path can change wallpaper rotation: its own keys, anything
/// under them, or a parent that holds them. `appearance.wallpaper`, which each rotation tick
/// writes, is not one (RA-529).
fn rotation_setting(path: &str) -> bool {
    matches!(path, "" | "appearance")
        || ["appearance.wallpaper_rotation", "appearance.wallpapers"]
            .iter()
            .any(|key| path.strip_prefix(key).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
}

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    pub rt: Handle,
    pub path: PathBuf,
    pub client: RefCell<Option<Client>>,
    pub project: Cell<i64>,
    pub generation: Cell<u64>,
    pub sessions: RefCell<Vec<Value>>,
    sidebar_sessions: RefCell<Vec<Value>>,
    pub projects: RefCell<Vec<Value>>,
    pub page: RefCell<String>,
    /// Whether the compositor still shows this window. A terminal that nobody can see does not
    /// need its output: the engine keeps the scrollback ring either way, so detaching costs
    /// nothing and saves the whole per-frame chain — socket read, parse, VTE feed and redraw —
    /// for as long as the window is minimized or covered.
    window_visible: Cell<bool>,
    applying_ui: Cell<bool>,
    navigation_pending: RefCell<Option<(i64, String)>>,
    navigation_sending: Cell<bool>,
    navigation_echoes: RefCell<BTreeSet<uuid::Uuid>>,
    pub content: gtk::Stack,
    pub pages: BTreeMap<String, gtk::Box>,
    pub page_projects: RefCell<BTreeMap<String, i64>>,
    pub notice: gtk::Label,
    status: gtk::Label,
    usage: status::UsageState,
    device_status: gtk::Label,
    pub(super) resource_status: gtk::Box,
    /// The bell, whose tooltip and accessible name carry the unread count its badge shows.
    notifications_key: gtk::Button,
    notification_count: gtk::Label,
    notification_revision: Cell<u64>,
    setup_checked: Cell<bool>,
    status_project: gtk::Label,
    status_branch: gtk::Label,
    pub(crate) sidebar: gtk::Box,
    settings_sidebar: Cell<bool>,
    /// The title bar's slot for the Settings page's search field, shown on Settings only.
    pub(crate) settings_tools: gtk::Box,
    focus_tabs: gtk::Box,
    mode: RefCell<String>,
    /// The wall's mode before Focus took it, which "Back to the wall" returns to.
    wall_mode: RefCell<String>,
    pub overlay: gtk::Overlay,
    /// What the start screen quiets while it shows: the title bar's keys and the status bar's
    /// contents. The bars themselves stay, wearing the start screen's ground.
    pub start_chrome: Vec<gtk::Widget>,
    pub page_overlay: gtk::Overlay,
    pub panels: Rc<RefCell<Vec<Rc<crate::panel::Panel>>>>,
    registry_dirty: Cell<bool>,
    pub(crate) workspaces: RefCell<Vec<Value>>,
    rendered_sessions: RefCell<BTreeMap<String, Value>>,
    /// The last `worktree.drift` and the project it was for: each pane's commits behind base.
    drift: RefCell<(i64, Value)>,
    drift_pending: Cell<bool>,
    drift_dirty: Cell<bool>,
    drift_timer: Cell<bool>,
    restored_project: Cell<i64>,
    /// The project an explicit destination was opened for: its layout restore applies
    /// everything but the saved page.
    explicit_page: Cell<i64>,
    layout_revision: Cell<u64>,
    layout_saves: RefCell<BTreeMap<i64, Value>>,
    layout_saving: Cell<bool>,
    appearance: gtk::CssProvider,
    pub(crate) wallpaper: gtk::Picture,
    pub(crate) wallpaper_dim: gtk::Box,
    font_size: Cell<f64>,
    pub(crate) palette: RefCell<String>,
    pub keybindings: RefCell<Value>,
    shortcut_hints: ShortcutHints,
    pub sound_busy: Cell<bool>,
    projects_box: gtk::Box,
    wall: gtk::Grid,
    wall_right: gtk::Grid,
    wall_split: gtk::Paned,
    wall_stack: gtk::Stack,
    empty_title: gtk::Label,
    panes: RefCell<BTreeMap<String, Rc<Pane>>>,
    ordered: RefCell<Vec<String>>,
    columns: Cell<i32>,
    focused: RefCell<Option<String>>,
    refresh_pending: Cell<bool>,
    refresh_dirty: Cell<bool>,
    page_pending: Cell<bool>,
    page_dirty: Cell<bool>,
    connected: Cell<bool>,
    /// The title bar's "Reconnect to engine" key: shown only while the engine is not connected.
    reconnect: gtk::Button,
    appearance_pending: Cell<bool>,
    launch: gtk::Revealer,
    launch_caption: gtk::Label,
    launch_busy: Cell<bool>,
    launch_box: gtk::Box,
    pub editor: Rc<crate::editor::Editor>,
    pub note_tabs: gtk::Notebook,
    pub notes_window: RefCell<Option<Rc<crate::pages::NotesWindow>>>,
    pub(crate) wallpaper_rotation: RefCell<crate::wallpaper_rotation::Rotation>,
    pub(crate) wallpaper_draft: RefCell<Option<crate::tools::settings::WallpaperDraft>>,
    pub(crate) provider_updates_checked: Cell<bool>,
}

pub fn run(rt: Handle) -> glib::ExitCode {
    let instance = std::env::var("RELAY_INSTANCE").unwrap_or_else(|_| "dev".into());
    if !matches!(instance.as_str(), "dev" | "test" | "stable") {
        eprintln!("RELAY_INSTANCE must be dev, test or stable");
        return glib::ExitCode::FAILURE;
    }
    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        eprintln!("XDG_RUNTIME_DIR is required to locate the Relay engine");
        return glib::ExitCode::FAILURE;
    };
    let path = std::env::var_os("RELAY_NATIVE_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(runtime_dir)
                .join("relay-v4")
                .join(format!("{instance}.sock"))
        });
    let app = gtk::Application::builder()
        .application_id("com.quietsoftware.Relay4")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_icon_theme_name(Some("Adwaita"));
            // The bundled Fira files carry no hinting instructions, and GTK's automatic mode
            // places glyphs unhinted at fractional positions: on a 1x screen the small
            // captions and figures lose their tops. Slight autohinting on whole pixels keeps
            // them crisp, and is indistinguishable at 2x.
            settings.set_gtk_font_rendering(gtk::FontRendering::Manual);
            settings.set_gtk_hint_font_metrics(true);
            settings.set_gtk_xft_antialias(1);
            settings.set_gtk_xft_hinting(1);
            settings.set_gtk_xft_hintstyle(Some("hintslight"));
            settings.set_gtk_xft_rgba(Some("none"));
            // Selectable text (a thread's replies, error messages) is read, not edited: a label
            // reached by Tab would otherwise select all of itself.
            settings.set_gtk_label_select_on_focus(false);
        }
        let provider = gtk::CssProvider::new();
        provider.connect_parsing_error(|_, _, e| tracing::error!("stylesheet: {e}"));
        provider.load_from_string(concat!(
            include_str!("theme.css"),
            include_str!("css/mirror.css"),
            include_str!("css/notes.css"),
            include_str!("css/git_files.css"),
            include_str!("css/board.css"),
            include_str!("css/sessions.css"),
            include_str!("css/workspace.css"),
            include_str!("css/guardrails.css"),
            include_str!("css/usage.css"),
            include_str!("css/resources.css"),
            include_str!("css/tools.css"),
            include_str!("css/money.css"),
            include_str!("css/threads.css"),
            include_str!("css/arbiter.css"),
            include_str!("css/avex.css"),
            include_str!("css/start.css"),
            include_str!("css/settings.css"),
            include_str!("css/launch.css"),
            include_str!("css/gaming.css"),
            include_str!("css/design.css"),
        ));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let ui = Ui::build(app, rt.clone(), path.clone());
        ui.track_window_visibility();
        ui.window.present();
        crate::pages::show_notes(&ui);
        ui.connect();
        crate::smoke::install(&ui);
    });
    app.run_with_args::<&str>(&[])
}

/// The keys whose tooltips name their shortcut, and the palette key's keycap: they follow a
/// rebinding in Settings (`Ui::show_shortcuts`).
struct ShortcutHints {
    palette: gtk::Button,
    keycap: gtk::Label,
    launch: gtk::Button,
    sidebar: gtk::Button,
    settings: gtk::Button,
}

/// The title bar, and the keys on it that the rest of the window wires up.
struct TopBar {
    handle: gtk::WindowHandle,
    left: gtk::Box,
    sidebar_key: gtk::Button,
    /// "/ Settings" after the wordmark, and the Dev/Money switch it stands in for there.
    crumb: gtk::Box,
    switcher: gtk::Box,
    settings_tools: gtk::Box,
    actions: gtk::Box,
    palette_key: gtk::Button,
    palette_keycap: gtk::Label,
    layouts_key: gtk::Button,
    skills_key: gtk::Button,
    plugins_key: gtk::Button,
    notifications_key: gtk::Button,
    notification_count: gtk::Label,
    reconnect: gtk::Button,
    launch_key: gtk::Button,
    launch_caption: gtk::Label,
    money_add: gtk::Button,
}

fn top_bar(window: &gtk::ApplicationWindow) -> TopBar {
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    top.add_css_class("topbar");
    let sidebar_key = icon_button("sidebar-show-symbolic", "Toggle sidebar");
    sidebar_key.set_widget_name("sidebar-toggle");
    let top_left = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    top_left.add_css_class("topbar-left");
    sidebar_key.set_child(Some(&crate::icons::image("sidebar", 14)));
    top_left.append(&sidebar_key);
    // The brand's mark, settled, beside the Sora wordmark.
    let mark = gtk::DrawingArea::new();
    mark.add_css_class("brand-mark");
    mark.set_content_width(22);
    mark.set_content_height(22);
    mark.set_valign(gtk::Align::Center);
    mark.set_draw_func(|_, cr, w, h| crate::start::draw_mark(cr, f64::from(w.min(h)), 1.0));
    top_left.append(&mark);
    let brand = label("relay", "brand");
    brand.set_margin_end(6);
    top_left.append(&brand);
    let crumb = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    crumb.add_css_class("brand-crumb");
    crumb.append(&label("/", "brand-slash"));
    crumb.append(&label("Settings", "brand-page"));
    crumb.set_visible(false);
    top_left.append(&crumb);
    top.append(&top_left);
    // Dev | Threads switches what the page shows, so it sits over the page, past the sidebar's
    // edge: the block before it keeps the sidebar's width (`follow_sidebar`).
    let switcher = crate::money::switcher();
    top.append(&switcher);
    let top_space = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    top_space.set_hexpand(true);
    top.append(&top_space);
    let settings_tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    settings_tools.add_css_class("settings-tools");
    settings_tools.set_visible(false);
    top.append(&settings_tools);
    let top_actions = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    top_actions.add_css_class("topbar-actions");
    top.append(&top_actions);
    // The palette's key reads as a search field: what it does, and its chord as a keycap. Both
    // follow the binding (`Ui::show_shortcuts`).
    let palette_key = button("", "command-search");
    palette_key.set_tooltip_text(Some("Command palette"));
    palette_key.set_widget_name("command-palette");
    let palette_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    palette_row.append(&crate::icons::image("search", 14));
    // Its natural width at full size; a narrower window shortens the words, not the bar's reach
    // (at 300px it alone kept a 1024px window from fitting its window controls), and to one
    // word rather than "Search or r…".
    let palette_text = crate::fit_label::FitLabel::new("Search or run a command", "Search", "command-search-text");
    palette_text.set_hexpand(true);
    palette_row.append(&palette_text);
    let palette_keycap = label("Ctrl K", "keycap");
    palette_row.append(&palette_keycap);
    palette_key.set_child(Some(&palette_row));
    palette_key.set_valign(gtk::Align::Center);
    let layouts_key = icon_button("view-grid-symbolic", "Window presets");
    layouts_key.set_widget_name("window-presets");
    let skills_key = icon_button("skills", "Skills for this project");
    let plugins_key = icon_button("plugins", "Plugins for this project");
    plugins_key.set_widget_name("project-plugins");
    let notifications_key = icon_button("alarm-symbolic", "Notifications");
    let bell = gtk::Overlay::new();
    bell.set_child(Some(&crate::icons::image("bell", 18)));
    let notification_count = label("", "notification-count");
    notification_count.set_halign(gtk::Align::End);
    notification_count.set_valign(gtk::Align::Start);
    notification_count.set_visible(false);
    notification_count.set_can_target(false);
    bell.add_overlay(&notification_count);
    notifications_key.set_child(Some(&bell));
    for (key, icon) in [(&skills_key, "skills"), (&plugins_key, "plugins"), (&layouts_key, "layout")] {
        key.set_child(Some(&crate::icons::image(icon, 18)));
    }
    for key in [
        &palette_key,
        &skills_key,
        &plugins_key,
        &layouts_key,
        &notifications_key,
    ] {
        top_actions.append(key);
    }
    let reconnect = icon_button("view-refresh-symbolic", "Reconnect to engine");
    top_actions.append(&reconnect);
    reconnect.set_visible(false);
    let launch_key = button("New agent", "primary");
    launch_key.set_widget_name("new-session");
    let launch_label = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    launch_label.append(&crate::icons::image_with_stroke("plus", 14, 2.0));
    let launch_caption = label("New agent", "");
    launch_label.append(&launch_caption);
    launch_key.set_child(Some(&launch_label));
    launch_key.set_valign(gtk::Align::Center);
    launch_key.add_css_class("launch-key");
    top_actions.append(&launch_key);
    let money_add = crate::money::add_key();
    top_actions.append(&money_add);
    let window_controls = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    window_controls.add_css_class("window-controls");
    for (icon, caption, size) in [
        ("minimize", "Minimize", 14),
        ("maximize", "Maximize", 12),
        ("close", "Close", 14),
    ] {
        let key = icon_button(icon, caption);
        key.remove_css_class("icon-key");
        key.add_css_class("window-control");
        key.set_margin_top(0);
        key.set_margin_bottom(0);
        key.set_child(Some(&crate::icons::image(icon, size)));
        if icon == "close" {
            key.add_css_class("window-close");
        }
        let window = window.downgrade();
        key.connect_clicked(move |_| {
            if let Some(window) = window.upgrade() {
                match icon {
                    "minimize" => window.minimize(),
                    "maximize" if window.is_maximized() => window.unmaximize(),
                    "maximize" => window.maximize(),
                    _ => window.close(),
                }
            }
        });
        window_controls.append(&key);
    }
    top.append(&window_controls);
    let handle = gtk::WindowHandle::new();
    handle.set_child(Some(&top));
    // The bar lives inside the wallpaper backdrop so panel opacity shows the wallpaper
    // behind it, like the status bar. A hidden titlebar keeps client-side decorations
    // and their resize edges; the WindowHandle still drags and maximizes the window.
    let titlebar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    titlebar.set_visible(false);
    window.set_titlebar(Some(&titlebar));
    TopBar {
        handle,
        left: top_left,
        sidebar_key,
        crumb,
        switcher,
        settings_tools,
        actions: top_actions,
        palette_key,
        palette_keycap,
        layouts_key,
        skills_key,
        plugins_key,
        notifications_key,
        notification_count,
        reconnect,
        launch_key,
        launch_caption,
        money_add,
    }
}

/// The notice under the title bar: the label, which stays the notice's source of truth
/// (`show_error`, smoke checks), and the bar around it, which follows its visibility and
/// carries the dismiss key.
fn notice_bar() -> (gtk::Label, gtk::Box) {
    let notice = label("Connecting to the Relay engine…", "notice");
    notice.set_wrap(true);
    notice.set_selectable(true);
    notice.set_hexpand(true);
    // The label stays the notice's source of truth (`show_error`, smoke checks); the bar
    // around it follows its visibility and carries the dismiss key.
    let notice_bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    notice_bar.add_css_class("notice-bar");
    notice_bar.append(&notice);
    let notice_close = icon_button("close", "Dismiss");
    notice_close.set_widget_name("notice-dismiss");
    notice_close.add_css_class("notice-dismiss");
    notice_close.set_valign(gtk::Align::Start);
    notice_bar.append(&notice_close);
    notice
        .bind_property("visible", &notice_bar, "visible")
        .sync_create()
        .build();
    let weak_notice = notice.downgrade();
    notice_close.connect_clicked(move |_| {
        if let Some(notice) = weak_notice.upgrade() {
            notice.set_visible(false);
        }
    });
    (notice, notice_bar)
}

/// The sidebar's width as the window opens, and how narrow and wide a drag may take it.
const SIDEBAR_WIDTH: i32 = 248;
const SIDEBAR_MIN: i32 = 200;
const SIDEBAR_MAX: i32 = 420;

/// Keeps the title bar's brand block as wide as the sidebar under it, so the Dev | Threads
/// pill after it starts over the page however the sidebar is dragged, and follows the brand
/// when the sidebar is hidden.
fn follow_sidebar(brand: &gtk::Box, split: &gtk::Paned, sidebar: &gtk::Box) {
    let fit = {
        let (brand, split, sidebar) = (brand.downgrade(), split.downgrade(), sidebar.downgrade());
        move || {
            let (Some(brand), Some(split), Some(sidebar)) = (brand.upgrade(), split.upgrade(), sidebar.upgrade()) else {
                return;
            };
            let width = if sidebar.is_visible() { split.position() } else { -1 };
            if brand.width_request() != width {
                brand.set_size_request(width, -1);
            }
        }
    };
    fit();
    let on_drag = fit.clone();
    split.connect_position_notify(move |_| on_drag());
    sidebar.connect_visible_notify(move |_| fit());
}

/// The sidebar: page keys, the workspaces and their projects, and Settings at the foot.
struct Sidebar {
    root: gtk::Box,
    nav: gtk::Box,
    money_nav: gtk::Box,
    /// The workspace heading and project list, which the Money space hides.
    workspaces: [gtk::Widget; 2],
    add_project: gtk::Button,
    projects_box: gtk::Box,
    /// The foot: the usage card (placed once usage exists) above you and Settings.
    footer: gtk::Box,
    settings_key: gtk::Button,
}

fn sidebar() -> Sidebar {
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.set_size_request(SIDEBAR_MIN, -1);
    sidebar.set_hexpand(false);
    sidebar.add_css_class("sidebar");
    let nav = gtk::Box::new(gtk::Orientation::Vertical, 1);
    nav.add_css_class("navigation");
    sidebar.append(&nav);
    let money_nav = crate::money::sidebar_keys();
    sidebar.append(&money_nav);
    let section = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    section.add_css_class("workspace-heading");
    let heading = label("Workspaces", "section-label");
    heading.set_hexpand(true);
    section.append(&heading);
    let add_project = icon_button("plus", "Add workspace");
    section.append(&add_project);
    sidebar.append(&section);
    let projects_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let project_scroll = scrolled(&projects_box);
    project_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sidebar.append(&project_scroll);
    // You, at the foot: your initial and first name, and the gear that opens Settings.
    let settings_key = button("", "nav");
    settings_key.add_css_class("settings-key");
    settings_key.set_tooltip_text(Some("Settings"));
    let who = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let name = crate::app::first_name();
    let initial = label(&name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(), "you-avatar");
    initial.set_xalign(0.5);
    initial.set_valign(gtk::Align::Center);
    who.append(&initial);
    let shown = label(&name, "nav-label");
    shown.set_hexpand(true);
    shown.set_xalign(0.0);
    who.append(&shown);
    who.append(&crate::icons::image("gear", 16));
    settings_key.set_child(Some(&who));
    let sidebar_footer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_footer.add_css_class("sidebar-footer");
    sidebar_footer.append(&settings_key);
    sidebar.append(&sidebar_footer);
    let workspaces = [section.upcast(), project_scroll.upcast()];
    Sidebar { root: sidebar, nav, money_nav, workspaces, add_project, projects_box, footer: sidebar_footer, settings_key }
}

/// The agent wall: the panes' grid and its right column, the empty state, and the focus tabs.
struct Wall {
    agents: gtk::Box,
    focus_tabs: gtk::Box,
    wall: gtk::Grid,
    wall_right: gtk::Grid,
    wall_split: gtk::Paned,
    wall_stack: gtk::Stack,
    empty_title: gtk::Label,
    empty_launch: gtk::Button,
}

fn wall() -> Wall {
    let wall = gtk::Grid::builder()
        .hexpand(true)
        .vexpand(true)
        .column_homogeneous(true)
        .row_homogeneous(true)
        .column_spacing(2)
        .row_spacing(2)
        .build();
    let wall_stack = gtk::Stack::new();
    wall_stack.set_vexpand(true);
    let wall_right = gtk::Grid::builder()
        .hexpand(true)
        .vexpand(true)
        .row_homogeneous(true)
        .row_spacing(2)
        .build();
    let wall_split = gtk::Paned::new(gtk::Orientation::Horizontal);
    wall_split.add_css_class("wall-split");
    wall_split.set_start_child(Some(&wall));
    wall_split.set_end_child(Some(&wall_right));
    wall_split.set_position(600);
    wall_split.set_shrink_start_child(false);
    wall_split.set_shrink_end_child(false);
    wall.set_size_request(280, -1);
    wall_right.set_size_request(280, -1);
    let wall_scroll = scrolled(&wall_split);
    // Files and Git can leave less room than the terminal panes' minimum.
    // Keep their content reachable instead of allocating below that minimum.
    wall_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    wall_scroll.set_overlay_scrolling(false);
    wall_stack.add_named(&wall_scroll, Some("wall"));
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 6);
    empty.add_css_class("wall-empty");
    empty.set_valign(gtk::Align::Center);
    empty.set_halign(gtk::Align::Center);
    let empty_glyph = crate::icons::image_with_stroke("grid", 28, 1.0);
    empty_glyph.add_css_class("empty-glyph");
    empty.append(&empty_glyph);
    let empty_title = label("No agents", "title");
    empty_title.set_halign(gtk::Align::Center);
    empty.append(&empty_title);
    let empty_description = label(
        "Launch a solo agent in its own worktree, or a review group on one branch. Each agent lands here as a tile.",
        "empty-description",
    );
    empty_description.set_wrap(true);
    empty_description.set_max_width_chars(52);
    empty_description.set_justify(gtk::Justification::Center);
    empty.append(&empty_description);
    let empty_launch = button("", "primary");
    let empty_action = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    empty_action.append(&crate::icons::image_with_stroke("plus", 14, 2.0));
    empty_action.append(&label("New agent", ""));
    empty_launch.set_child(Some(&empty_action));
    empty_launch.set_halign(gtk::Align::Center);
    empty.append(&empty_launch);
    // The terminals' plates top up the page stack's @wall; with none, the page does.
    let empty_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty_page.add_css_class("page-layer");
    empty.set_vexpand(true);
    empty_page.append(&empty);
    wall_stack.add_named(&empty_page, Some("empty"));
    wall_stack.set_visible_child_name("empty");
    let agents = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let focus_tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    focus_tabs.add_css_class("focus-tabs");
    agents.append(&focus_tabs);
    let wall_body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wall_body.append(&wall_stack);
    agents.append(&wall_body);
    Wall { agents, focus_tabs, wall, wall_right, wall_split, wall_stack, empty_title, empty_launch }
}

/// Every page but the agent wall, added to `content`. Notes has a box but no place in it.
fn pages(content: &gtk::Stack) -> BTreeMap<String, gtk::Box> {
    let mut pages = BTreeMap::new();
    for name in [
        "board",
        "modules",
        "mailbox",
        "guardrails",
        "notes",
        "settings",
        "skills",
        "plugins",
        "devices",
    ] {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        page.add_css_class("page");
        page.set_vexpand(true);
        if name == "notes" {
            // The Notes workspace lives in its own retained native window.
        } else if name == "board" {
            content.add_named(&page, Some(name));
        } else {
            let scroller = scrolled(&page);
            if name == "settings" {
                // Its category list stands in the sidebar's place and paints as the sidebar;
                // only the column beside it is the page (css/settings.css).
                scroller.add_css_class("split-page");
            }
            content.add_named(&scroller, Some(name));
        }
        pages.insert(name.into(), page);
    }
    pages
}

/// The New agents sheet over `body`, and the scrim that closes it.
struct LaunchSheet {
    overlay: gtk::Overlay,
    launch: gtk::Revealer,
    launch_box: gtk::Box,
    scrim: gtk::Button,
}

fn launch_sheet(body: &gtk::Box) -> LaunchSheet {
    let launch_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    launch_box.add_css_class("launch");
    launch_box.set_size_request(780, -1);
    let launch = gtk::Revealer::new();
    // Its fixed-width allocation remains above the page when collapsed.
    launch.set_can_target(false);
    launch.set_transition_duration(0);
    launch.set_hexpand(false);
    launch.set_child(Some(&launch_box));
    launch.set_halign(gtk::Align::End);
    launch.set_size_request(780, -1);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(body));
    let scrim = gtk::Button::new();
    scrim.add_css_class("launch-scrim");
    // Clicked, never a Tab stop: Space on it would close the sheet.
    scrim.set_focusable(false);
    scrim.set_visible(false);
    scrim.update_property(&[gtk::accessible::Property::Label("Close New agents")]);
    overlay.add_overlay(&scrim);
    overlay.add_overlay(&launch);
    let weak_body = body.downgrade();
    let weak_scrim = scrim.downgrade();
    launch.connect_child_revealed_notify(move |launch| {
        launch.set_can_target(launch.reveals_child());
        if let Some(scrim) = weak_scrim.upgrade() {
            scrim.set_visible(launch.reveals_child());
        }
        if let Some(body) = weak_body.upgrade() {
            body.set_sensitive(!launch.reveals_child());
        }
    });
    LaunchSheet { overlay, launch, launch_box, scrim }
}

/// The status bar along the bottom.
struct StatusBar {
    root: gtk::Box,
    status_project: gtk::Label,
    status_branch: gtk::Label,
    status: gtk::Label,
    devices_key: gtk::Button,
    device_status: gtk::Label,
    resources_key: gtk::Button,
    resource_status: gtk::Box,
}

/// The bar, and the usage state behind the sidebar's card (placed by `Ui::new`).
fn status_bar() -> (StatusBar, status::UsageState) {
    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bottom.add_css_class("statusbar");
    let status_project = label("No project", "status-project");
    status_project.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status_project.set_max_width_chars(24);
    let status_branch = label("", "mono");
    status_branch.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status_branch.set_max_width_chars(18);
    bottom.append(&status_project);
    bottom.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    // The branch, when there is a project to have one: no bare branch mark beside "No project".
    let branch_cell = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    branch_cell.append(&crate::icons::image("branch", 11));
    branch_cell.append(&status_branch);
    branch_cell.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    branch_cell.set_visible(false);
    bottom.append(&branch_cell);
    status_branch.connect_label_notify(move |l| branch_cell.set_visible(!l.label().is_empty()));
    bottom.append(&label("linux · local", "status-platform"));
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bottom.append(&spacer);
    let status = label("Engine disconnected", "");
    bottom.append(&status);
    let devices_key = button("", "quiet");
    devices_key.set_widget_name("status-devices");
    let device_status = label("No device", "mono");
    let device_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    device_content.append(&crate::icons::image("device", 12));
    device_content.append(&device_status);
    devices_key.set_child(Some(&device_content));
    let resources_key = icon_button("cpu", "Resources");
    resources_key.set_widget_name("status-resources");
    let resource_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    resource_content.append(&crate::icons::image("cpu", 12));
    let resource_status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    resource_content.append(&resource_status);
    resources_key.set_child(Some(&resource_content));
    let usage = status::UsageState::new();
    bottom.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    bottom.append(&gaming_key::build());
    bottom.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    bottom.append(&devices_key);
    bottom.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    bottom.append(&resources_key);
    let bar = StatusBar {
        root: bottom,
        status_project,
        status_branch,
        status,
        devices_key,
        device_status,
        resources_key,
        resource_status,
    };
    (bar, usage)
}

/// Puts `outer` over the wallpaper and its dim layer, as the window's content.
fn backdrop(window: &gtk::ApplicationWindow, outer: &gtk::Box) -> (gtk::Picture, gtk::Box) {
    let wallpaper = gtk::Picture::new();
    wallpaper.set_can_shrink(true);
    wallpaper.set_content_fit(gtk::ContentFit::Cover);
    let backdrop = gtk::Overlay::new();
    backdrop.set_child(Some(&wallpaper));
    let wallpaper_dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wallpaper_dim.add_css_class("wallpaper-dim");
    wallpaper_dim.set_can_target(false);
    backdrop.add_overlay(&wallpaper_dim);
    backdrop.add_overlay(outer);
    outer.set_hexpand(true);
    outer.set_vexpand(true);
    window.set_child(Some(&backdrop));
    (wallpaper, wallpaper_dim)
}

impl Ui {
    fn build(app: &gtk::Application, rt: Handle, path: PathBuf) -> Rc<Self> {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Relay")
            .default_width(1440)
            .default_height(900)
            .build();
        crate::fonts::install(&window);
        crate::icons::install_app_icon(&window);
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let top = top_bar(&window);
        outer.append(&top.handle);
        let (notice, notice_bar) = notice_bar();
        outer.append(&notice_bar);
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        let side = sidebar();
        // The sidebar and the page share a split the person can drag, from 200 to 420 pixels.
        let split = gtk::Paned::new(gtk::Orientation::Horizontal);
        split.add_css_class("sidebar-split");
        split.set_hexpand(true);
        split.set_start_child(Some(&side.root));
        split.set_resize_start_child(false);
        split.set_shrink_start_child(false);
        split.set_position(SIDEBAR_WIDTH);
        // The width the person chose, which a window too narrow for it squeezes and a wider one
        // gives back; GtkPaned alone kept the squeezed width (a window opened small, or made
        // narrower and wide again, left the sidebar at its 220px minimum for good).
        let chosen = Rc::new(Cell::new(SIDEBAR_WIDTH));
        split.connect_position_notify({
            let chosen = chosen.clone();
            move |split| {
                if split.position() > SIDEBAR_MAX {
                    split.set_position(SIDEBAR_MAX);
                } else if split.position() < split.max_position() {
                    chosen.set(split.position());
                }
            }
        });
        split.connect_max_position_notify(move |split| {
            let want = chosen.get().min(split.max_position());
            if split.position() < want {
                // Not during the allocation that changed the room.
                let split = split.downgrade();
                glib::idle_add_local_once(move || {
                    if let Some(split) = split.upgrade() {
                        split.set_position(want);
                    }
                });
            }
        });
        body.append(&split);
        follow_sidebar(&top.left, &split, &side.root);
        let content = gtk::Stack::new();
        content.add_css_class("page-stack");
        content.set_hhomogeneous(false);
        content.set_vhomogeneous(false);
        content.set_hexpand(true);
        content.set_vexpand(true);
        let page_overlay = gtk::Overlay::new();
        page_overlay.set_child(Some(&content));
        split.set_end_child(Some(&page_overlay));
        split.set_resize_end_child(true);
        split.set_shrink_end_child(false);
        let wall = wall();
        let editor = crate::editor::Editor::new();
        editor.mount_agents(&wall.agents);
        // Panel opacity reaches the terminals only if the stacks between them and the
        // wallpaper stop repainting @wall on top of each other (see css/usage.css).
        wall.agents.add_css_class("agents-surface");
        if let Some(host) = wall.agents.parent() {
            host.add_css_class("agents-host");
        }
        content.add_named(&editor.root, Some("agents"));
        let pages = pages(&content);
        crate::money::add_pages(&content);
        let sheet = launch_sheet(&body);
        let panel_host = gtk::Overlay::new();
        panel_host.set_child(Some(&sheet.overlay));
        outer.append(&panel_host);
        let (bar, usage) = status_bar();
        outer.append(&bar.root);
        side.footer.prepend(usage.card());
        let (wallpaper, wallpaper_dim) = backdrop(&window, &outer);
        let ui = Rc::new(Self {
            window,
            rt,
            path,
            client: RefCell::new(None),
            project: Cell::new(0),
            generation: Cell::new(0),
            sessions: RefCell::default(),
            sidebar_sessions: RefCell::default(),
            projects: RefCell::default(),
            page: RefCell::new("agents".into()),
            window_visible: Cell::new(true),
            applying_ui: Cell::new(false),
            navigation_pending: RefCell::default(),
            navigation_sending: Cell::new(false),
            navigation_echoes: RefCell::default(),
            content,
            pages,
            page_projects: RefCell::default(),
            notice,
            status: bar.status.clone(),
            usage,
            device_status: bar.device_status.clone(),
            resource_status: bar.resource_status.clone(),
            notifications_key: top.notifications_key.clone(),
            notification_count: top.notification_count.clone(),
            notification_revision: Cell::new(0),
            setup_checked: Cell::new(false),
            status_project: bar.status_project.clone(),
            status_branch: bar.status_branch.clone(),
            sidebar: side.root.clone(),
            settings_sidebar: Cell::new(true),
            settings_tools: top.settings_tools.clone(),
            focus_tabs: wall.focus_tabs,
            mode: RefCell::new("grid".into()),
            wall_mode: RefCell::new("grid".into()),
            overlay: panel_host,
            start_chrome: [top.left.clone().upcast(), top.switcher.clone().upcast(), top.settings_tools.clone().upcast(), top.actions.clone().upcast()]
                .into_iter()
                .chain(bar.root.observe_children().into_iter().filter_map(|c| c.ok()?.downcast::<gtk::Widget>().ok()))
                .collect(),
            page_overlay,
            panels: Rc::default(),
            registry_dirty: Cell::new(true),
            workspaces: RefCell::default(),
            rendered_sessions: RefCell::default(),
            drift: RefCell::new((0, Value::Null)),
            drift_pending: Cell::new(false),
            drift_dirty: Cell::new(false),
            drift_timer: Cell::new(false),
            restored_project: Cell::new(0),
            explicit_page: Cell::new(0),
            layout_revision: Cell::new(0),
            layout_saves: RefCell::default(),
            layout_saving: Cell::new(false),
            appearance: gtk::CssProvider::new(),
            wallpaper,
            wallpaper_dim,
            font_size: Cell::new(9.75),
            palette: RefCell::new("matte".into()),
            keybindings: RefCell::new(json!({})),
            shortcut_hints: ShortcutHints {
                palette: top.palette_key.clone(),
                keycap: top.palette_keycap.clone(),
                launch: top.launch_key.clone(),
                sidebar: top.sidebar_key.clone(),
                settings: side.settings_key.clone(),
            },
            sound_busy: Cell::new(false),
            projects_box: side.projects_box.clone(),
            wall: wall.wall,
            wall_right: wall.wall_right,
            wall_split: wall.wall_split,
            wall_stack: wall.wall_stack,
            empty_title: wall.empty_title,
            panes: RefCell::default(),
            ordered: RefCell::default(),
            columns: Cell::new(2),
            focused: RefCell::new(None),
            refresh_pending: Cell::new(false),
            refresh_dirty: Cell::new(false),
            page_pending: Cell::new(false),
            page_dirty: Cell::new(false),
            connected: Cell::new(false),
            reconnect: top.reconnect.clone(),
            appearance_pending: Cell::new(false),
            launch: sheet.launch.clone(),
            launch_caption: top.launch_caption.clone(),
            launch_busy: Cell::new(false),
            launch_box: sheet.launch_box.clone(),
            editor,
            note_tabs: gtk::Notebook::new(),
            notes_window: RefCell::default(),
            wallpaper_rotation: RefCell::default(),
            wallpaper_draft: RefCell::default(),
            provider_updates_checked: Cell::new(false),
        });
        ui.wire_navigation(&side, &top);
        ui.wire_settings_return(&top, &bar.root);
        ui.show_shortcuts();
        ui.wire_keys(&top, &bar, &wall.empty_launch, &side.add_project, &sheet.scrim);
        ui.wire_window();
        let mut dev_only: Vec<gtk::Widget> = vec![side.nav.clone().upcast(), top.layouts_key.clone().upcast(), top.launch_key.clone().upcast()];
        dev_only.extend(side.workspaces.iter().cloned());
        crate::money::install(&ui, &side.money_nav, &top.money_add, dev_only);
        crate::design::install(&ui);
        ui
    }

    /// The sidebar's page keys and Settings, and the title bar's notifications, Skills and
    /// Plugins keys.
    fn wire_navigation(self: &Rc<Self>, side: &Sidebar, top: &TopBar) {
        let ui = self;
        let nav = &side.nav;
        for (name, caption, icon) in [
            ("board", "Board", "sidebar"),
            ("skills", "Skills", "skills"),
            ("plugins", "Plugins", "dashboard"),
            ("notes", "Notes", "brief"),
        ] {
            let b = nav_button(caption, icon);
            b.set_widget_name(&format!("nav-{name}"));
            match name {
                "skills" => b.set_tooltip_text(Some("Skills: agent instructions, on or off per project")),
                "plugins" => b.set_tooltip_text(Some("Plugins: skills, rules and MCP tools for a project")),
                _ => {}
            }
            track_navigation(&ui.content, &b, name);
            nav.append(&b);
            let weak = Rc::downgrade(ui);
            b.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.toggle_page(name);
                }
            });
        }
        let weak = Rc::downgrade(ui);
        side.settings_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.toggle_page("settings");
            }
        });
        // Notifications open in place under the bell, never as a page of their own.
        notification_center::install(ui, &top.notifications_key);
        let weak = Rc::downgrade(ui);
        top.skills_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.project_skills();
            }
        });
        let weak = Rc::downgrade(ui);
        top.plugins_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.project_plugins();
            }
        });
    }

    /// Settings takes the whole window: it hides the sidebar, the title bar's actions and the
    /// status bar, and turns the sidebar key into a back key to the page it was opened from.
    fn wire_settings_return(self: &Rc<Self>, top: &TopBar, bottom: &gtk::Box) {
        let ui = self;
        let (back_key, top_actions) = (top.sidebar_key.clone(), top.actions.clone());
        let (crumb, switcher, settings_tools) = (top.crumb.clone(), top.switcher.clone(), top.settings_tools.clone());
        let (skills_key, plugins_key, bottom) = (top.skills_key.clone(), top.plugins_key.clone(), bottom.clone());
        // The page Settings was opened from, which its back key returns to.
        let settings_return = Rc::new(RefCell::new(String::from("agents")));
        let return_state = settings_return.clone();
        let weak = Rc::downgrade(ui);
        let mut_previous = Rc::new(RefCell::new(String::from("agents")));
        ui.content.connect_visible_child_name_notify(move |stack| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let page = stack.visible_child_name().unwrap_or_default();
            let settings = page == "settings";
            if settings && mut_previous.borrow().as_str() != "settings" {
                ui.settings_sidebar.set(ui.sidebar.is_visible());
                *return_state.borrow_mut() = mut_previous.borrow().clone();
                ui.sidebar.set_visible(false);
            } else if !settings && mut_previous.borrow().as_str() == "settings" {
                ui.sidebar.set_visible(ui.settings_sidebar.get());
            }
            crumb.set_visible(settings);
            switcher.set_visible(!settings);
            settings_tools.set_visible(settings);
            back_key.set_child(Some(&crate::icons::image(
                if settings { "chevron-left" } else { "sidebar" },
                14,
            )));
            back_key.set_tooltip_text(Some(&if settings {
                String::from("Back from settings")
            } else {
                ui.shortcut_tip("Toggle sidebar", "sidebar")
            }));
            top_actions.set_visible(!settings);
            bottom.set_visible(!settings);
            skills_key.set_visible(page == "agents");
            plugins_key.set_visible(page == "agents");
            *mut_previous.borrow_mut() = page.into();
        });
        let weak = Rc::downgrade(ui);
        top.sidebar_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui.page.borrow().as_str() == "settings" {
                    let page = settings_return.borrow().clone();
                    ui.navigate(&page);
                } else {
                    ui.sidebar.set_visible(!ui.sidebar.is_visible());
                    ui.save_layout();
                }
            }
        });
    }

    /// The title bar's and status bar's remaining keys, the empty wall's launch key, Add
    /// workspace and the launch scrim.
    fn wire_keys(
        self: &Rc<Self>,
        top: &TopBar,
        bar: &StatusBar,
        empty_launch: &gtk::Button,
        add_project: &gtk::Button,
        scrim: &gtk::Button,
    ) {
        let ui = self;
        gaming_key::wire(ui);
        let weak = Rc::downgrade(ui);
        top.palette_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.command_palette();
            }
        });
        let weak = Rc::downgrade(ui);
        top.layouts_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.layout_menu();
            }
        });
        let weak = Rc::downgrade(ui);
        bar.devices_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                crate::tools::devices::open(&ui);
            }
        });
        let weak = Rc::downgrade(ui);
        bar.resources_key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.resources();
            }
        });
        for b in [&top.launch_key, empty_launch] {
            let weak = Rc::downgrade(ui);
            b.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    if ui.launch.reveals_child() && !ui.launch_busy.get() {
                        ui.launch.set_reveal_child(false);
                    } else {
                        ui.show_launch(None);
                    }
                }
            });
        }
        let weak = Rc::downgrade(ui);
        top.reconnect.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                // Reconnecting recreates every pane, the launch's placeholders included.
                if ui.launch_busy.get() {
                    ui.show_error("Wait for agent launch to finish before reconnecting.");
                    return;
                }
                ui.connect();
            }
        });
        let weak = Rc::downgrade(ui);
        add_project.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.open_repository();
            }
        });
        let weak = Rc::downgrade(ui);
        scrim.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if !ui.launch_busy.get() {
                    ui.launch.set_reveal_child(false);
                }
            }
        });
    }

    /// Window-wide wiring: the Code view, usage polling, shortcuts, the appearance sheet,
    /// Escape, pane sizes on every layout, and the close policy.
    fn wire_window(self: &Rc<Self>) {
        let ui = self;
        let weak = Rc::downgrade(ui);
        ui.editor.connect_view_changed(move || {
            if let Some(ui) = weak.upgrade() {
                ui.update_attachments();
            }
        });
        ui.install_usage();
        ui.install_shortcuts();
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &ui.appearance,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(ui);
        keys.connect_key_pressed(move |_, key, _, _| {
            if let Some(ui) = weak.upgrade() {
                if key == gtk::gdk::Key::Escape {
                    let panel = ui.panels.borrow().last().cloned();
                    if let Some(panel) = panel {
                        panel.close();
                        return glib::Propagation::Stop;
                    }
                }
                if key == gtk::gdk::Key::Escape
                    && ui.launch.reveals_child()
                    && !ui.launch_busy.get()
                {
                    ui.launch.set_reveal_child(false);
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        ui.window.add_controller(keys);
        let weak = Rc::downgrade(ui);
        ui.window.connect_realize(move |w| {
            if let Some(surface) = w.surface() {
                let weak = weak.clone();
                surface.connect_layout(move |_, _, _| {
                    if let Some(ui) = weak.upgrade() {
                        let width = ui.wall_split.width();
                        if width >= 560 && ui.wall_right.is_visible() {
                            let position = ui.wall_split.position().clamp(280, width - 280);
                            if position != ui.wall_split.position() {
                                ui.wall_split.set_position(position);
                            }
                        }
                        for p in ui.panes.borrow().values() {
                            p.schedule_resize();
                        }
                    }
                });
            }
        });
        let owned = ui.clone();
        ui.window.connect_close_request(move |_| owned.close_request());
    }
    /// The main window's close: refused while a launch runs or any work is unsaved, otherwise
    /// every panel, note, mirror and pane is closed before the window goes.
    fn close_request(&self) -> glib::Propagation {
        if self.launch_busy.get() {
            self.show_error("Wait for agent launch to finish before closing.");
            return glib::Propagation::Stop;
        }
        if crate::pages::unsaved_notes() {
            self.show_error("Save or discard note changes before closing.");
            return glib::Propagation::Stop;
        }
        if self.editor.is_dirty() {
            self.show_error("Save or discard your editor changes before closing.");
            return glib::Propagation::Stop;
        }
        let panels = self.panels.borrow().clone();
        if panels.iter().any(|panel| !panel.can_close()) {
            self.show_error("Save or discard panel changes before closing.");
            return glib::Propagation::Stop;
        }
        for panel in panels.iter().rev() {
            panel.close();
        }
        crate::pages::close_all_notes();
        if let Some(notes) = self.notes_window.borrow_mut().take() {
            notes.window.destroy();
        }
        crate::mirror::close_all();
        self.generation.set(self.generation.get() + 1);
        self.set_connected(false);
        self.client.borrow_mut().take();
        for pane in self.panes.borrow().values() {
            pane.stop();
        }
        self.panes.borrow_mut().clear();
        glib::Propagation::Proceed
    }
    /// Show `message` in the banner under the title bar. It has a dismiss key and clears itself
    /// once there was time to read it, unless the pointer rests on it. While the engine is not
    /// connected it describes that state, so it stays until the connection comes back.
    pub fn show_error(&self, message: &str) {
        self.show_notice(message, (6 + message.chars().count() / 20).min(20));
    }
    /// `show_error` for good news (an approval, a commit, a push): the same banner, its rule not
    /// the red of a refusal.
    pub fn show_info(&self, message: &str) {
        self.show_notice(message, (6 + message.chars().count() / 20).min(20));
        if let Some(bar) = self.notice.parent() {
            bar.add_css_class("info");
        }
    }
    /// `show_error` with its own reading time, in seconds.
    fn show_notice(&self, message: &str, seconds: usize) {
        self.notice.set_text(message);
        self.notice.set_visible(true);
        if let Some(bar) = self.notice.parent() {
            bar.remove_css_class("info");
        }
        let serial = NOTICE_SERIAL.with(|s| {
            s.set(s.get().wrapping_add(1));
            s.get()
        });
        if !self.connected.get() {
            return;
        }
        let mut remaining = seconds;
        let notice = self.notice.downgrade();
        glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
            let Some(notice) = notice.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if NOTICE_SERIAL.with(Cell::get) != serial || !notice.is_visible() {
                return glib::ControlFlow::Break;
            }
            let held = notice.parent().is_some_and(|bar| {
                bar.state_flags().contains(gtk::StateFlags::PRELIGHT)
            }) || notice.has_focus();
            if !held {
                remaining = remaining.saturating_sub(1);
            }
            if remaining > 0 {
                return glib::ControlFlow::Continue;
            }
            notice.set_visible(false);
            glib::ControlFlow::Break
        });
    }
    pub async fn call(&self, op: &str, payload: Value) -> Result<Value, Error> {
        let client = self.client.borrow().clone().ok_or(Error::Disconnected)?;
        if crate::client::is_lifecycle_request(op) {
            return Client::lifecycle_request(&self.rt, self.path.clone(), op, payload).await;
        }
        client.request(&self.rt, op, payload).await
    }
    pub fn mutate(self: &Rc<Self>, op: &'static str, payload: Value, key: &gtk::Button) {
        key.set_sensitive(false);
        let key = key.clone();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            match ui.call(op, payload).await {
                Ok(_) => ui.refresh(),
                Err(e) => ui.show_error(&e.to_string()),
            }
            key.set_sensitive(true);
            ui.refresh_page();
        });
    }
    /// Whether the engine answers: requests fail at once while it does not.
    pub fn is_connected(&self) -> bool {
        self.connected.get()
    }
    fn set_connected(&self, connected: bool) {
        self.connected.set(connected);
        self.reconnect.set_visible(!connected);
    }
    fn connect(self: &Rc<Self>) {
        self.navigation_echoes.borrow_mut().clear();
        self.registry_dirty.set(true);
        self.rendered_sessions.borrow_mut().clear();
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        self.client.borrow_mut().take();
        self.set_connected(false);
        for p in self.panes.borrow().values() {
            p.stop();
        }
        self.panes.borrow_mut().clear();
        self.ordered.borrow_mut().clear();
        // The first refresh after connecting re-applies the saved pane order and focus, and no
        // save writes the cleared wall over them before it has.
        self.restored_project.set(0);
        // A launch form built for the old connection would no longer submit (RA-470).
        self.launch.set_reveal_child(false);
        self.forget_launch();
        self.layout();
        self.show_error("Connecting to the Relay engine…");
        let ui = self.clone();
        glib::spawn_future_local(async move {
            match Client::connect(&ui.rt, ui.path.clone()).await {
                Ok((client, notices)) => {
                    if generation != ui.generation.get() {
                        return;
                    }
                    *ui.client.borrow_mut() = Some(client);
                    if let Err(e)=ui.call("bus.subscribe",json!({"events":["project.changed","project.deleted","workspace.changed","workspace.deleted","session.changed","task.changed","task.deleted","mailbox.new","mailbox.changed","guardrail.held","guardrail.resolved","guardrail.grant_used","overlap.changed","notes.changed","notes.deleted","file.changed","git.changed","worktree.changed","module.changed","module.deleted","skill.changed","skill.deleted","plugin.changed","settings.changed","provider.update.changed","notify.new","notify.changed","device.changed","device.lease.acquired","device.lease.released","run.changed","run.crash","device.signing.changed","avd.changed","layout.changed","ui.changed","ui.toast","usage.changed","integration.changed","integration.result","money.changed","arbiter.changed","gym.changed","thread.changed","thread.message","thread.delta","thread.local","thread.proposal","gaming.changed","design.changed","design.deleted"]})).await {
                        if generation == ui.generation.get() {
                            ui.show_error(&e.to_string());
                        }
                        return;
                    }
                    // A newer attempt started while this one subscribed: it owns the UI now.
                    if generation != ui.generation.get() {
                        return;
                    }
                    ui.set_connected(true);
                    crate::provider_updates::startup(&ui);
                    ui.status.set_text("");
                    ui.refresh_status();
                    ui.refresh_notification_count();
                    crate::pages::restore_prompts(&ui);
                    ui.notice.set_visible(false);
                    ui.refresh();
                    ui.load_appearance();
                    crate::wallpaper_rotation::refresh(&ui);
                    ui.watch_drift();
                    ui.load_keybindings();
                    crate::money::startup(&ui);
                    if *ui.page.borrow() == "devices" {
                        // Its own future, as navigate() does: awaited here, more than 64 events
                        // ahead of its reply would fill the notice channel before anything
                        // drains it, and the reply would never be read (RA-554).
                        let ui = ui.clone();
                        glib::spawn_future_local(async move {
                            let _ = ui.call("device.watch", json!({"on":true})).await;
                        });
                    }
                    let weak = Rc::downgrade(&ui);
                    drop(ui);
                    while let Ok(notice) = notices.recv().await {
                        let Some(ui) = weak.upgrade() else {
                            break;
                        };
                        if ui.generation.get() != generation {
                            break;
                        }
                        match notice {
                            Notice::Event(e) => {
                                // Local navigation already updated the widgets. Consume its
                                // echo before project filtering, including a project just left.
                                if e.ev == "ui.changed"
                                    && e.cause.is_some_and(|id| {
                                        ui.navigation_echoes.borrow_mut().remove(&id)
                                    })
                                {
                                    continue;
                                }
                                // An event too large to read: whatever it changed, re-read it.
                                if e.ev == crate::client::DROPPED_EVENT {
                                    tracing::warn!(ev = text(&e.payload, "ev"), "dropped an oversized engine event");
                                    crate::pages::refresh_notes(&ui);
                                    ui.refresh();
                                    continue;
                                }
                                // Each refreshes only what it changed: a usage event spends
                                // no adb call (RA-511).
                                match e.ev.as_str() {
                                    // Only the usage meters read it; no page does.
                                    "usage.changed" => {
                                        ui.refresh_usage(false);
                                        continue;
                                    }
                                    "device.changed" | "run.changed" => ui.refresh_devices(),
                                    // The status bar's key is all that shows it.
                                    "gaming.changed" => {
                                        gaming_key::render(&e.payload);
                                        continue;
                                    }
                                    _ => {}
                                }
                                if e.ev == "notify.new" {
                                    crate::sounds::notify(&ui, &e.payload);
                                }
                                if e.ev.starts_with("notify.") {
                                    ui.refresh_notification_count();
                                    notification_center::changed();
                                }
                                if e.ev.starts_with("guardrail.") {
                                    crate::pages::guardrail_event(&ui, &e.ev, &e.payload);
                                }
                                if e.ev == "money.changed" {
                                    crate::money::changed(&ui);
                                    continue;
                                }
                                if e.ev == "arbiter.changed" {
                                    crate::money::arbiter_changed(&ui);
                                    continue;
                                }
                                if e.ev == "gym.changed" {
                                    crate::money::gym_changed(&ui);
                                    continue;
                                }
                                if e.ev.starts_with("thread.") {
                                    crate::money::thread_event(&ui, &e.ev, &e.payload);
                                    continue;
                                }
                                if e.ev == "provider.update.changed" {
                                    crate::provider_updates::event(&ui, &e.payload);
                                    continue;
                                }
                                // A note lists the tasks made from it, whatever project it shows.
                                if matches!(e.ev.as_str(), "task.changed" | "task.deleted") {
                                    crate::pages::note_tasks_changed(&ui);
                                }
                                // Notes retains its own project when the main window switches.
                                if matches!(e.ev.as_str(), "notes.changed" | "notes.deleted") {
                                    crate::pages::refresh_notes(&ui);
                                    continue;
                                }
                                // So does a design in a window of its own.
                                if e.ev.starts_with("design.") {
                                    crate::design::event(&ui, &e.ev, &e.payload);
                                    continue;
                                }
                                if e.project_id.is_some_and(|id| id != ui.project.get())
                                    && !e.ev.starts_with("project.")
                                    && !e.ev.starts_with("session.")
                                {
                                    continue;
                                }
                                let reads = {
                                    // Each page is re-read only for the events that change
                                    // what it shows: a usage, notification or device event
                                    // re-read (and the board redrew) for nothing. Money's
                                    // pages read only the ledger, which says so with
                                    // money.changed; the wall and Code refresh on their own.
                                    let family = e.ev.split('.').next().unwrap_or("");
                                    match ui.page.borrow().as_str() {
                                        // Cards name their module; a module counts its tasks.
                                        "board" | "modules" => matches!(family, "task" | "module"),
                                        "mailbox" => family == "mailbox",
                                        // Requests, held actions and grants, and overlaps.
                                        "guardrails" => matches!(family, "guardrail" | "overlap"),
                                        // A finished provider update says so with notify.new,
                                        // which repaints the provider cards.
                                        "settings" => family == "notify",
                                        "skills" => family == "skill",
                                        // A plugin's skills are listed with it.
                                        "plugins" => matches!(family, "plugin" | "skill"),
                                        "devices" => matches!(family, "device" | "run" | "avd" | "mirror"),
                                        _ => false,
                                    }
                                };
                                if e.ev.starts_with("project.") || e.ev.starts_with("workspace.") {
                                    ui.registry_dirty.set(true);
                                    ui.refresh();
                                } else if e.ev.starts_with("session.") {
                                    // `session.changed` carries the whole row: apply it in
                                    // place instead of re-listing every session.
                                    if !ui.apply_session_event(&e.ev, &e.payload) {
                                        ui.refresh();
                                    }
                                } else if matches!(
                                    e.ev.as_str(),
                                    "file.changed"
                                        | "git.changed"
                                        | "worktree.changed"
                                        | "integration.changed"
                                        | "integration.result"
                                ) {
                                    ui.editor.invalidate(&ui, Some((&e.ev, &e.payload)));
                                    if e.ev != "file.changed" {
                                        ui.refresh_drift();
                                    }
                                } else if e.ev == "layout.changed"
                                    && e.payload["action"] == "applied"
                                {
                                    ui.apply_layout(&e.payload["state"]);
                                } else if e.ev == "ui.toast" {
                                    // Say who sent it: it shares the banner with Relay's own errors.
                                    let message = match &e.actor {
                                        relay_bus::Actor::Agent(_) => format!("{}: {}", e.actor, text(&e.payload, "text")),
                                        _ => text(&e.payload, "text").to_string(),
                                    };
                                    let ttl = e.payload["ttl_ms"].as_u64().unwrap_or(4000);
                                    ui.show_notice(&message, ttl.div_ceil(1000).clamp(2, 60) as usize);
                                } else if e.ev == "ui.changed" {
                                    ui.apply_ui_event(&e.payload);
                                } else if e.ev == "settings.changed" {
                                    let path = text(&e.payload, "path");
                                    if path.starts_with("keybindings") || path.is_empty() {
                                        ui.load_keybindings();
                                    }
                                    if path.starts_with("usage") || path.is_empty() {
                                        ui.reload_usage_prefs();
                                    }
                                    // Only these read the wallpaper library, which can be
                                    // megabytes: not every appearance field (RA-529).
                                    if rotation_setting(path) {
                                        crate::wallpaper_rotation::refresh(&ui);
                                    }
                                    if path.starts_with("appearance.")
                                        || path == "terminal.font_size"
                                        || path.is_empty()
                                    {
                                        // One Save writes each changed field on its own; reload
                                        // the appearance once for the lot.
                                        if !ui.appearance_pending.replace(true) {
                                            let weak = Rc::downgrade(&ui);
                                            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                                                if let Some(ui) = weak.upgrade() {
                                                    ui.appearance_pending.set(false);
                                                    ui.load_appearance();
                                                }
                                            });
                                        }
                                    }
                                } else if reads {
                                    ui.refresh_page();
                                }
                            }
                            Notice::Disconnected(e) => {
                                ui.navigation_echoes.borrow_mut().clear();
                                ui.set_connected(false);
                                ui.client.borrow_mut().take();
                                ui.status.set_text(
                                    "Engine disconnected · sessions remain owned by the engine",
                                );
                                ui.show_error(&e.to_string());
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) if generation == ui.generation.get() => ui.show_error(&format!(
                    "{e}. Start relay serve for this instance, then reconnect."
                )),
                Err(_) => {}
            }
        });
    }
    pub fn dismiss_panels(&self) -> bool {
        let panels = self.panels.borrow().clone();
        if panels.iter().any(|panel| !panel.can_close()) {
            self.show_error("Save or discard your changes before leaving this page.");
            return false;
        }
        for panel in panels.iter().rev() {
            panel.close();
        }
        true
    }
    pub fn toggle_page(self: &Rc<Self>, page: &str) {
        let active = *self.page.borrow() == page;
        self.navigate(if active { "agents" } else { page });
    }
    pub fn navigate(self: &Rc<Self>, page: &str) {
        if page == "notes" {
            crate::pages::show_notes(self);
            return;
        }
        if page == "notifications" {
            // A restored layout or an engine echo must not pop it open by itself.
            if !self.applying_ui.get() {
                notification_center::open();
            }
            return;
        }
        let files = page == "code";
        let design = page == "design";
        let page = if files || design { "agents" } else { page };
        // A restored layout or an engine echo does not take the window out of Money.
        if self.applying_ui.get() && crate::money::active() && !crate::money::is_page(page) {
            return;
        }
        if *self.page.borrow() != page && !self.dismiss_panels() {
            return;
        }
        if self.content.child_by_name(page).is_none() {
            return;
        }
        if !self.applying_ui.get()
            && self.connected.get()
            && matches!(
                page,
                "agents"
                    | "code"
                    | "board"
                    | "modules"
                    | "skills"
                    | "plugins"
                    | "settings"
            )
        {
            self.sync_navigation(page);
        }
        let previous = self.page.borrow().clone();
        if (previous == "devices") != (page == "devices") && self.connected.get() {
            let ui = self.clone();
            let on = page == "devices";
            glib::spawn_future_local(async move {
                if let Err(e) = ui.call("device.watch", json!({"on":on})).await {
                    ui.show_error(&e.to_string());
                }
            });
        }
        *self.page.borrow_mut() = page.into();
        self.content.set_visible_child_name(page);
        crate::money::follow(self, page, self.applying_ui.get());
        if page == "agents" {
            self.editor.prepare_project(self);
            if files {
                self.editor.show_files();
            } else if design {
                self.editor.show_design();
            } else {
                self.editor.show_agents();
            }
        }
        self.save_layout();
        self.layout();
        self.refresh_page();
    }
    pub fn refresh(self: &Rc<Self>) {
        self.refresh_dirty.set(true);
        if self.refresh_pending.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            while ui.refresh_dirty.replace(false) {
                let generation = ui.generation.get();
                let layout_revision = ui.layout_revision.get();
                let mut layout_project = ui.project.get();
                // The sidebar order before this read, to find a removed project's neighbour.
                let mut order = None;
                if ui.registry_dirty.replace(false) {
                    let (projects, workspaces) = tokio::join!(
                        ui.call("project.list", json!({})),
                        ui.call("workspace.list", json!({}))
                    );
                    if generation != ui.generation.get() {
                        continue;
                    }
                    match (projects, workspaces) {
                        (Ok(p), Ok(w)) => {
                            order = Some(ui.project_order());
                            *ui.projects.borrow_mut() = rows(&p, "projects");
                            *ui.workspaces.borrow_mut() = rows(&w, "workspaces");
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            // The registry is still unread: the next refresh must fetch it.
                            ui.registry_dirty.set(true);
                            ui.show_error(&e.to_string());
                            break;
                        }
                    }
                }
                if !ui
                    .projects
                    .borrow()
                    .iter()
                    .any(|p| p["id"].as_i64() == Some(ui.project.get()))
                {
                    if ui.project.get() != 0 {
                        // Removed elsewhere: the same move a removal made here makes.
                        if !ui.leave_removed_project(&order.unwrap_or_else(|| ui.project_order())) {
                            break;
                        }
                    } else {
                        // Nothing chosen yet: start on the first project.
                        let first = ui.projects.borrow().first().and_then(|p| p["id"].as_i64());
                        if let Some(first) = first {
                            ui.project.set(first);
                            // Chosen by this iteration, not switched under it: restore (and so
                            // allow saves to) this project's layout now, not on a later refresh.
                            layout_project = first;
                            ui.editor.reset();
                            ui.editor.prepare_project(&ui);
                        }
                    }
                }
                ui.render_projects();
                if !ui.setup_checked.replace(true) && ui.projects.borrow().is_empty() {
                    ui.open_repository();
                }
                let project = ui.project.get();
                if project == 0 {
                    ui.sessions.borrow_mut().clear();
                    ui.reconcile();
                    continue;
                }
                match ui
                    .call("session.list", json!({"include_closed":false}))
                    .await
                {
                    Ok(v) if ui.project.get() == project && generation == ui.generation.get() => {
                        let all = rows(&v, "sessions");
                        // A launch keeps its placeholders, and adopts its own new sessions into
                        // their places (as `apply_session_event` does while it runs).
                        let launching = ui.launch_busy.get();
                        let previous = std::mem::take(&mut *ui.sessions.borrow_mut());
                        let known = |s: &Value| previous.iter().any(|p| text(p, "name") == text(s, "name"));
                        let mut sessions: Vec<Value> = all
                            .iter()
                            .filter(|s| s["project_id"].as_i64() == Some(project))
                            .filter(|s| !launching || text(s, "state") != "created" || known(s))
                            .cloned()
                            .collect();
                        if launching {
                            sessions.extend(previous.iter().filter(|s| s["placeholder"] == true).cloned());
                        }
                        *ui.sessions.borrow_mut() = sessions;
                        *ui.sidebar_sessions.borrow_mut() = all;
                        ui.render_projects();
                        ui.reconcile();
                    }
                    Ok(_) => {}
                    Err(e) => ui.show_error(&e.to_string()),
                }
                ui.restore_layout(layout_project, layout_revision).await;
                crate::pages::refresh_notes(&ui);
                if ui.page.borrow().as_str() == "board" {
                    ui.refresh_page();
                }
            }
            ui.refresh_pending.set(false);
        });
    }
    fn reconcile(self: &Rc<Self>) {
        // A pane closed optimistically stays closed while its close is in flight.
        self.sessions.borrow_mut().retain(|s| !shell::is_closing(text(s, "name")));
        self.render_status_counts();
        let sessions = self.sessions.borrow().clone();
        self.launch_caption.set_text("New agent");
        let available: Vec<String> = sessions.iter().map(|s| text(s, "name").into()).collect();
        let mut names = self.ordered.borrow().clone();
        names.retain(|n| available.contains(n));
        for n in available {
            if !names.contains(&n) {
                names.push(n);
            }
        }
        let old: Vec<String> = self
            .panes
            .borrow()
            .keys()
            .filter(|n| !names.contains(n))
            .cloned()
            .collect();
        for n in old {
            self.rendered_sessions.borrow_mut().remove(&n);
            if let Some(p) = self.panes.borrow_mut().remove(&n) {
                p.stop();
                if let Some(grid) = p.root.parent().and_downcast::<gtk::Grid>() {
                    grid.remove(&p.root);
                }
            }
        }
        let mut created = false;
        for s in &sessions {
            let name = text(s, "name");
            let new_pane = !self.panes.borrow().contains_key(name);
            created |= new_pane;
            if new_pane {
                let pane = Pane::new(name, self.path.clone(), self.rt.clone());
                self.install_pane_controls(&pane, name);
                pane.apply_appearance(&self.palette.borrow(), self.font_size.get());
                self.panes.borrow_mut().insert(name.into(), pane);
            }
            let pane = self.panes.borrow().get(name).cloned().unwrap();
            let signature = json!({"state":s["state"], "role":s["role"], "provider":s["provider"], "branch":s["branch"], "worktree":s["worktree"], "intent":s["intent"], "pair_with":s["pair_with"]});
            // Render signatures describe widgets, not just sessions. A recreated pane has
            // an empty header and visible slate even when the session itself is unchanged.
            if new_pane || self.rendered_sessions.borrow().get(name) != Some(&signature) {
                pane.update_session(s);
                self.session_actions(&pane, s);
                self.rendered_sessions
                    .borrow_mut()
                    .insert(name.into(), signature);
            }
        }
        // Only layout() puts a pane in a grid. A new pane whose name is already in `ordered` (a
        // launch adopting its placeholder's slot, a restored layout) needs it as much as a new
        // order does, or it stays off the wall until something else lays out.
        if *self.ordered.borrow() != names || created {
            *self.ordered.borrow_mut() = names;
            self.layout();
        } else {
            self.update_attachments();
        }
        let name = self
            .projects
            .borrow()
            .iter()
            .find(|p| p["id"].as_i64() == Some(self.project.get()))
            .map(|p| text(p, "name").to_string());
        self.status_project.set_text(name.as_deref().unwrap_or("No project"));
        self.empty_title.set_text(&match &name {
            Some(name) => format!("No agents in {name}"),
            None => String::from("No project yet"),
        });
        self.status_branch.set_text(
            self.projects
                .borrow()
                .iter()
                .find(|p| p["id"].as_i64() == Some(self.project.get()))
                .map(|p| text(p, "base_branch"))
                .unwrap_or(""),
        );

        self.wall_stack
            .set_visible_child_name(if sessions.is_empty() { "empty" } else { "wall" });
        self.apply_drift();
    }
    /// Fetch now and every few minutes, so "behind" counts what has landed on GitHub too.
    /// One timer for the window's life; a fetch while disconnected just fails quietly.
    fn watch_drift(self: &Rc<Self>) {
        self.fetch_for_drift();
        if self.drift_timer.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_seconds_local(DRIFT_FETCH_SECS, move || {
            let Some(ui) = weak.upgrade() else { return glib::ControlFlow::Break };
            if ui.connected.get() {
                ui.fetch_for_drift();
            }
            glib::ControlFlow::Continue
        });
    }
    fn fetch_for_drift(self: &Rc<Self>) {
        let project = self.project.get();
        if project == 0 {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            // Offline or no remote: the counts stay against the refs already fetched.
            let _ = ui.call("git.fetch", json!({"project_id": project})).await;
            ui.refresh_drift();
        });
    }
    /// Re-read `worktree.drift` for the current project; calls while one is out coalesce.
    fn refresh_drift(self: &Rc<Self>) {
        self.drift_dirty.set(true);
        if self.drift_pending.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            while ui.drift_dirty.replace(false) {
                let project = ui.project.get();
                if project == 0 {
                    break;
                }
                // Not every project has a base with commits yet; such a pane shows no count.
                let drift = ui.call("worktree.drift", json!({"project_id": project})).await.unwrap_or(Value::Null);
                if ui.project.get() == project {
                    *ui.drift.borrow_mut() = (project, drift);
                    ui.apply_drift();
                }
            }
            ui.drift_pending.set(false);
        });
    }
    fn apply_drift(self: &Rc<Self>) {
        let project = self.project.get();
        if self.drift.borrow().0 != project {
            self.refresh_drift();
            return;
        }
        let drift = self.drift.borrow();
        let base = text(&drift.1, "base");
        let rows = rows(&drift.1, "worktrees");
        for s in self.sessions.borrow().iter() {
            let Some(pane) = self.panes.borrow().get(text(s, "name")).cloned() else { continue };
            let behind = rows.iter().find(|w| text(w, "path") == text(s, "worktree")).and_then(|w| w["behind"].as_i64());
            pane.set_behind(behind, base);
        }
    }
    /// Follow the compositor's own view of whether this window is on screen.
    ///
    /// `SUSPENDED` is the compositor saying the surface is not visible — minimized, fully
    /// covered, or on another workspace — and `MINIMIZED` covers the backends that do not send
    /// it. Either way the terminals detach, and re-attach from their last sequence with bounded
    /// catch-up when the window comes back, which is the same path a reconnect already takes.
    /// The surface only exists once the window is realized, so this is installed from there.
    pub fn track_window_visibility(self: &Rc<Self>) {
        let ui = self.clone();
        self.window.connect_realize(move |window| {
            let Some(toplevel) = window
                .surface()
                .and_then(|surface| surface.downcast::<gtk::gdk::Toplevel>().ok())
            else {
                return;
            };
            let hidden = |state: gtk::gdk::ToplevelState| {
                state.contains(gtk::gdk::ToplevelState::MINIMIZED)
                    || state.contains(gtk::gdk::ToplevelState::SUSPENDED)
            };
            ui.window_visible.set(!hidden(toplevel.state()));
            ui.update_attachments();
            let weak = Rc::downgrade(&ui);
            toplevel.connect_state_notify(move |toplevel| {
                let Some(ui) = weak.upgrade() else { return };
                let visible = !hidden(toplevel.state());
                if ui.window_visible.replace(visible) != visible {
                    ui.update_attachments();
                }
            });
        });
    }
    fn update_attachments(&self) {
        let focus = self.focused.borrow();
        let mode = self.mode.borrow();
        for s in self.sessions.borrow().iter() {
            if let Some(p) = self.panes.borrow().get(text(s, "name")) {
                let active = (self.window_visible.get()
                    && *self.page.borrow() == "agents"
                    && self.editor.agents_visible()
                    && (mode.as_str() != "focus"
                        || focus.as_ref().is_none_or(|n| n == text(s, "name"))))
                    && !matches!(
                        text(s, "state"),
                        "created" | "parked" | "restorable" | "exited"
                    );
                p.set_active(active);
            }
        }
    }
    pub fn verify_shell(self: &Rc<Self>) {
        let panes = self.panes.borrow().clone();
        for session in self.sessions.borrow().iter() {
            panes[text(session, "name")].verify_session_rendered(session);
        }
        let mode = self.mode.borrow().clone();
        for mode in ["grid", "focus", "review", "mosaic"] {
            self.set_mode(mode);
            for (name, pane) in &panes {
                assert!(
                    Rc::ptr_eq(pane, &self.panes.borrow()[name]),
                    "Layout replaced a terminal"
                );
            }
        }
        self.set_mode(&mode);
        println!("Shell layouts verified: grid, focus, review, mosaic, retained terminals");
    }
    pub fn verify_launch(&self) {
        assert!(self.launch.reveals_child());
        assert!(
            self.launch.width() == 780 && self.launch_box.width() == 779,
            "Launch dimensions: sheet={}, form={}",
            self.launch.width(),
            self.launch_box.width()
        );
        assert!(
            !self.sidebar.is_sensitive(),
            "The launch sheet must block background actions"
        );
    }
    pub fn verify_terminal_input(&self) {
        use vte4::prelude::TerminalExt;
        for (name, pane) in self.panes.borrow().iter() {
            pane.terminal
                .paste_text(&format!("native-paste-check-{name}\n"));
        }
    }
    /// Every live pane is on screen and shows `marker`. A parked, exited, stopped or not started
    /// session's pane shows its slate over a detached terminal, so it is not read.
    pub fn terminal_contents_contain(&self, marker: &str) -> bool {
        use vte4::prelude::TerminalExt;
        let panes = self.panes.borrow();
        let live: Vec<_> = self.sessions.borrow().iter()
            .filter(|s| matches!(text(s, "state"), "spawning" | "running" | "idle" | "blocked"))
            .map(|s| panes.get(text(s, "name")).cloned())
            .collect();
        !live.is_empty() && live.iter().all(|pane| {
            let Some(pane) = pane else { return false };
            let (_, row) = pane.terminal.cursor_position();
            let (text, _) = pane.terminal.text_range_format(vte4::Format::Text, 0, 0, row, 500);
            pane.terminal.is_mapped() && text.is_some_and(|text| text.contains(marker))
        })
    }
    /// Every session has a pane and every pane is attached: what [`Ui::verify_burst`] asserts
    /// before it feeds input.
    pub(crate) fn burst_ready(&self) -> bool {
        let panes = self.panes.borrow();
        !panes.is_empty() && panes.len() == self.sessions.borrow().len() && panes.values().all(|pane| pane.is_ready())
    }
    pub fn verify_burst(&self, check: bool) {
        use vte4::prelude::TerminalExt;
        let rendered = self.panes.borrow().len();
        assert!(rendered > 0 && rendered == self.sessions.borrow().len(), "{rendered} panes");
        if !check {
            println!("Burst layout: {}", self.mode.borrow());
        }
        for (name, pane) in self.panes.borrow().iter() {
            if check {
                let (_, row) = pane.terminal.cursor_position();
                let (tail, _) = pane.terminal.text_range_format(
                    vte4::Format::Text,
                    (row - 10).max(0),
                    0,
                    row,
                    200,
                );
                assert!(
                    tail.unwrap_or_default()
                        .contains(&format!("BURST-END-{name}")),
                    "No native tail marker for {name}"
                );
            } else {
                pane.verify_ready();
                pane.terminal.paste_text("native-burst\n");
            }
        }
        if check {
            println!("BURST_RENDERED={rendered}");
        }
    }
    pub fn refresh_page(self: &Rc<Self>) {
        self.page_dirty.set(true);
        if self.page_pending.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            while ui.page_dirty.replace(false) {
                let page = ui.page.borrow().clone();
                let project = ui.project.get();
                if matches!(page.as_str(), "agents" | "code") {
                    continue;
                }
                if crate::money::is_page(&page) {
                    crate::money::refresh(&ui, &page).await;
                    continue;
                }
                if matches!(
                    page.as_str(),
                    "settings" | "skills" | "plugins" | "devices"
                ) {
                    crate::tools::refresh(&ui, &page, project).await;
                } else if project != 0 {
                    crate::pages::refresh(&ui, &page, project).await;
                }
            }
            ui.page_pending.set(false);
        });
    }
    pub fn open_repository(self: &Rc<Self>) {
        self.open_repository_in(None);
    }
    fn open_repository_in(self: &Rc<Self>, workspace: Option<Value>) {
        if self.editor.is_dirty() || !self.dismiss_panels() {
            self.show_error("Save or discard your changes before adding a project.");
            return;
        }
        onboarding::open(self, workspace);
    }
}

#[cfg(test)]
mod tests {
    use super::rotation_setting;

    #[test]
    fn only_rotation_keys_refresh_rotation() {
        for path in ["", "appearance", "appearance.wallpaper_rotation", "appearance.wallpaper_rotation.minutes", "appearance.wallpapers"] {
            assert!(rotation_setting(path), "{path}");
        }
        for path in ["appearance.wallpaper", "appearance.wallpaper_opacity", "appearance.wallpapers_x", "appearance.accent", "terminal.font_size"] {
            assert!(!rotation_setting(path), "{path}");
        }
    }
}
