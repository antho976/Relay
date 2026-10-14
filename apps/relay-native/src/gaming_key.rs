//! The status bar's gaming key: a switch for gaming mode, a choice to let it turn itself on while
//! a game runs, and what it is doing now. The engine does the capping (`relay_core::gaming`);
//! this reads `app.status.gaming` on connect and `gaming.changed` after, and writes
//! `settings: performance.gaming.*`.
use super::*;

struct Key {
    button: gtk::MenuButton,
    caption: gtk::Label,
    what: gtk::Label,
    switch: gtk::Switch,
    auto: gtk::CheckButton,
    state: gtk::Label,
    /// Set while `render` moves the controls, so their handlers do not write the value back.
    rendering: Cell<bool>,
}

thread_local! {
    static KEY: RefCell<Option<Rc<Key>>> = const { RefCell::new(None) };
}

fn key() -> Option<Rc<Key>> {
    KEY.with(|k| k.borrow().clone())
}

/// The key for the status bar, built once.
pub(super) fn build() -> gtk::MenuButton {
    let button = gtk::MenuButton::new();
    button.set_widget_name("status-gaming");
    button.add_css_class("quiet");
    // Flat like the bar's other keys: a menu button keeps its frame on an inner `button`.
    button.set_has_frame(false);
    button.set_tooltip_text(Some("Gaming mode"));
    button.update_property(&[gtk::accessible::Property::Label("Gaming mode")]);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&crate::icons::image("gamepad", 12));
    let caption = label("Gaming", "mono");
    caption.set_visible(false);
    content.append(&caption);
    button.set_child(Some(&content));

    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.add_css_class("gaming-sheet");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let title = label("Gaming mode", "gaming-title");
    title.set_hexpand(true);
    let switch = gtk::Switch::new();
    switch.set_valign(gtk::Align::Center);
    switch.update_property(&[gtk::accessible::Property::Label("Gaming mode")]);
    head.append(&title);
    head.append(&switch);
    body.append(&head);
    let what = label("", "gaming-note");
    what.set_wrap(true);
    what.set_max_width_chars(42);
    body.append(&what);
    let auto = gtk::CheckButton::with_label("Turn on by itself while a game runs");
    body.append(&auto);
    let state = label("", "gaming-state");
    state.set_wrap(true);
    state.set_max_width_chars(42);
    body.append(&state);
    let popover = gtk::Popover::new();
    popover.add_css_class("gaming-popover");
    popover.set_child(Some(&body));
    button.set_popover(Some(&popover));

    let key = Rc::new(Key { button: button.clone(), caption, what, switch, auto, state, rendering: Cell::new(false) });
    KEY.with(|k| *k.borrow_mut() = Some(key));
    render(&json!({"threads": 2}));
    button
}

/// Writes a change of the switch or the choice to the engine's settings.
pub(super) fn wire(ui: &Rc<Ui>) {
    let Some(key) = key() else { return };
    let weak = Rc::downgrade(ui);
    let k = key.clone();
    key.switch.connect_state_set(move |_, on| {
        if !k.rendering.get() {
            if let Some(ui) = weak.upgrade() {
                ui.set_gaming("performance.gaming.on", on);
            }
        }
        glib::Propagation::Proceed
    });
    let weak = Rc::downgrade(ui);
    let k = key.clone();
    key.auto.connect_toggled(move |check| {
        if !k.rendering.get() {
            if let Some(ui) = weak.upgrade() {
                ui.set_gaming("performance.gaming.auto", check.is_active());
            }
        }
    });
}

/// Show a `GamingStatus` (`app.status.gaming`, or a `gaming.changed` payload).
pub(super) fn render(status: &Value) {
    let Some(key) = key() else { return };
    let supported = status["supported"].as_bool().unwrap_or(false);
    let on = status["on"].as_bool().unwrap_or(false);
    let auto = status["auto"].as_bool().unwrap_or(false);
    let active = status["active"].as_bool().unwrap_or(false);
    let threads = status["threads"].as_u64().unwrap_or(2);
    key.rendering.set(true);
    key.switch.set_active(on);
    key.switch.set_state(on);
    key.auto.set_active(auto);
    key.switch.set_sensitive(supported);
    key.auto.set_sensitive(supported);
    key.rendering.set(false);
    key.caption.set_visible(active);
    if active {
        key.button.add_css_class("gaming-active");
    } else {
        key.button.remove_css_class("gaming-active");
    }
    let plural = if threads == 1 { "" } else { "s" };
    key.what.set_text(&format!("While it is on, agents and everything they run, builds included, share {threads} CPU thread{plural} at the lowest priority, so your game keeps the machine. Relay itself stays fast."));
    let text = match (supported, active, status["game"].as_str()) {
        (false, _, _) => "Not available here: gaming mode needs a systemd user session.".to_string(),
        (true, true, Some(game)) if !on => format!("On while {game} runs. Agents are capped at {threads} thread{plural}."),
        (true, true, _) => format!("On. Agents are capped at {threads} thread{plural}."),
        (true, false, _) if auto => "Off. Watching for a game to start.".to_string(),
        (true, false, _) => "Off.".to_string(),
    };
    key.state.set_text(&text);
    key.button.set_tooltip_text(Some(if active { "Gaming mode is on" } else { "Gaming mode" }));
}

impl Ui {
    /// Read gaming mode's state (on connect; events keep it current after).
    pub(super) fn refresh_gaming(self: &Rc<Self>) {
        if self.client.borrow().is_none() {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            if let Ok(status) = ui.call("app.status", json!({})).await {
                render(&status["gaming"]);
            }
        });
    }

    fn set_gaming(self: &Rc<Self>, path: &'static str, value: bool) {
        let ui = self.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("settings.set", json!({"path": path, "value": value})).await {
                ui.show_error(&e.to_string());
                ui.refresh_gaming();
            }
        });
    }
}
