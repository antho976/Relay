//! Native device mirror: an engine-owned scrcpy transport, one FFmpeg decode, GTK textures.
//!
//! One [`MirrorView`] per device. It is a self-contained widget tree that lives either in its
//! own window or docked to the right of the wall, and moves between the two without restarting
//! the stream. It is the phone and one rail beside it, nothing else: the rail carries the device
//! switcher (with the status lamp), the hardware keys, Android's navigation triad and the
//! window's own actions. A phone and a virtual device get the same view — AVDs boot headless and
//! this is their screen (see [`open_avd`]).
//!
//! The mirror never waits on silence. The engine reports every state on the stream itself —
//! starting, running (with the real picture size), and on the way out stopped, failed with its
//! code and reason, or lost when the device went away — so the stage always says what is
//! happening and offers the next step. A lost device is watched for, and the mirror resumes on
//! its own when it comes back.
mod decode;
mod glyphs;
mod input;

use crate::app::{label, Ui};
use crate::client::{Client, Error, Notice};
use crate::panel::Dock;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::Duration;

/// Engine → view messages that must never be dropped (pictures travel separately, lossy).
pub(crate) enum Control {
    /// A status object from the engine: `{state, width, height, name?, code?, message?}`.
    Status(Value),
    /// The decoder lost its place; ask the device for a fresh config + key frame.
    Resync,
    /// The video pipeline or the engine connection broke.
    Broken(String),
}

/// Video frames can exceed the control channel's 2 MiB line: a key frame at a high capture
/// rung is a single base64 line.
const MIRROR_MAX_LINE: usize = 32 * 1024 * 1024;
/// The largest capture rung the client asks for. Decode is single-threaded on purpose (see
/// `decode.rs`), and a picture taller than this is more pixels than any dock or window shows.
const MAX_CAPTURE: i32 = 1280;
const DOCK_WIDTH: i32 = 440;
/// What the window spends around the phone: the rail and stage padding across, padding down.
const CHROME_W: i32 = 116;
const CHROME_H: i32 = 60;
const HINT: &str = "Click to tap · drag to swipe · right-click for Back · type to send keys";
/// How long a booting AVD may take before the mirror stops waiting for it.
const BOOT_WAIT: Duration = Duration::from_secs(180);
/// Mirror attempts while a fresh AVD finishes booting (adb lists it before the system is up).
const BOOT_TRIES: u32 = 40;

thread_local! {
    /// Open mirrors, strongly held: a view lives until its window closes or its dock is closed.
    static VIEWS: RefCell<Vec<Rc<MirrorView>>> = const { RefCell::new(Vec::new()) };
    /// Whether the last mirror the user placed was docked; new mirrors open the same way.
    static PREFER_DOCK: Cell<bool> = const { Cell::new(false) };
}

/// Open (or bring forward) the mirror for `device`.
pub fn open(ui: &Rc<Ui>, device: String) {
    let existing = VIEWS.with(|views| views.borrow().iter().find(|view| *view.device.borrow() == device).cloned());
    if let Some(view) = existing {
        view.present();
        return;
    }
    let view = MirrorView::new(ui, device);
    VIEWS.with(|views| views.borrow_mut().push(view.clone()));
    if PREFER_DOCK.with(Cell::get) {
        view.dock();
    } else {
        view.detach();
    }
    view.start();
}

/// End every mirror, docked or in its own window. The main window calls this as it closes:
/// a mirror window is an application window too and would otherwise keep the process, its
/// engine connection and the device's stream alive with nothing behind it.
pub fn close_all() {
    let views = VIEWS.with(|views| views.borrow().clone());
    for view in views {
        view.close();
    }
}

/// Open the next mirror docked beside the wall (the smoke harness screenshots the main window,
/// which a detached mirror is not part of).
pub(crate) fn prefer_dock() {
    PREFER_DOCK.with(|prefer| prefer.set(true));
}

/// Boot the AVD `name` headless and open its mirror: the same view a phone gets. The stage
/// says what is happening while the emulator comes up, then the mirror starts on its serial.
pub fn open_avd(ui: &Rc<Ui>, name: String, cold: bool) {
    let existing = VIEWS.with(|views| views.borrow().iter().find(|view| view.avd.borrow().as_deref() == Some(name.as_str())).cloned());
    if let Some(view) = existing {
        view.present();
        return;
    }
    let view = MirrorView::new(ui, String::new());
    *view.avd.borrow_mut() = Some(name.clone());
    view.set_model(&name);
    VIEWS.with(|views| views.borrow_mut().push(view.clone()));
    if PREFER_DOCK.with(Cell::get) {
        view.dock();
    } else {
        view.detach();
    }
    view.boot(cold);
}

enum Host {
    None,
    Window(gtk::Window),
    Dock(Dock),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Connecting,
    Live,
    Ended,
}

struct MirrorView {
    ui: Weak<Ui>,
    device: RefCell<String>,
    /// The device's own name once known (`device.list` model or the server's handshake).
    model: RefCell<String>,
    /// The AVD this view booted, while it has not yet been seen live.
    avd: RefCell<Option<String>>,
    booting: Cell<bool>,
    boot_tries: Cell<u32>,
    root: gtk::Box,
    lamp: gtk::Box,
    title: gtk::Label,
    meta: gtk::Label,
    picker: gtk::MenuButton,
    picker_list: gtk::Box,
    dock_key: gtk::Button,
    fullscreen_key: gtk::Button,
    close_key: gtk::Button,
    shot_key: gtk::Button,
    display_key: gtk::Button,
    controls: Vec<gtk::Button>,
    aspect: gtk::AspectFrame,
    picture: gtk::Picture,
    card: gtk::Box,
    card_spinner: gtk::Spinner,
    card_title: gtk::Label,
    card_detail: gtk::Label,
    card_code: gtk::Label,
    card_retry: gtk::Button,
    card_other: gtk::Button,
    hint: gtk::Label,
    hint_generation: Cell<u64>,
    phase: Cell<Phase>,
    /// The picture size in device pixels: the last decoded frame, else the engine's status.
    dims: Cell<(i32, i32)>,
    texture: RefCell<Option<gtk::gdk::MemoryTexture>>,
    display_on: Cell<bool>,
    /// The last failure, for "Copy details".
    problem: RefCell<String>,
    input: RefCell<Option<async_channel::Sender<Value>>>,
    task: RefCell<Option<glib::JoinHandle<()>>>,
    generation: Cell<u64>,
    host: RefCell<Host>,
    fullscreen: Cell<bool>,
    pointer: Cell<Option<(f64, f64)>>,
    /// The Android key sent down for each hardware key still held, so its release sends the
    /// same key up whatever the modifiers are by then, and a key never sent down sends nothing.
    held: RefCell<HashMap<u32, u32>>,
}

impl MirrorView {
    fn new(ui: &Rc<Ui>, device: String) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("mirror");
        root.set_hexpand(true);
        root.set_vexpand(true);

        // -- device switcher: the rail's head, with the status lamp on it -----------------
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.add_css_class("lamp");
        lamp.add_css_class("mirror-lamp");
        lamp.set_halign(gtk::Align::End);
        lamp.set_valign(gtk::Align::Start);
        lamp.set_can_target(false);
        let face = gtk::Overlay::new();
        face.set_child(Some(&glyphs::image("device", 16)));
        face.add_overlay(&lamp);
        let picker = gtk::MenuButton::new();
        picker.set_child(Some(&face));
        picker.add_css_class("mirror-picker");
        picker.set_tooltip_text(Some(&device));
        let title = label(&device, "mirror-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let meta = label(&device, "mirror-meta");
        meta.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let sheet = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sheet.add_css_class("mirror-sheet");
        let heading = gtk::Box::new(gtk::Orientation::Vertical, 0);
        heading.add_css_class("mirror-sheet-head");
        heading.append(&title);
        heading.append(&meta);
        sheet.append(&heading);
        let picker_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        picker_list.add_css_class("mirror-devices");
        sheet.append(&picker_list);
        let tips = label(HINT, "mirror-meta");
        tips.set_wrap(true);
        tips.set_max_width_chars(34);
        tips.add_css_class("mirror-sheet-foot");
        sheet.append(&tips);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&sheet));
        popover.set_has_arrow(false);
        popover.set_position(gtk::PositionType::Left);
        picker.set_popover(Some(&popover));

        // -- stage: the phone and its rail ---------------------------------------------------
        let stage = gtk::Grid::new();
        stage.add_css_class("mirror-stage");
        stage.set_hexpand(true);
        stage.set_vexpand(true);
        stage.set_column_spacing(14);
        let picture = gtk::Picture::new();
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        picture.set_focusable(true);
        picture.add_css_class("mirror-picture");
        picture.update_property(&[gtk::accessible::Property::Label("Device screen"), gtk::accessible::Property::Description(HINT)]);
        let screen = gtk::Overlay::new();
        screen.add_css_class("mirror-screen");
        screen.set_overflow(gtk::Overflow::Hidden);
        screen.set_child(Some(&picture));
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class("mirror-card");
        card.set_halign(gtk::Align::Fill);
        card.set_valign(gtk::Align::Center);
        let card_spinner = gtk::Spinner::new();
        card_spinner.set_halign(gtk::Align::Center);
        card_spinner.add_css_class("mirror-spinner");
        let card_title = label("", "mirror-card-title");
        card_title.set_wrap(true);
        card_title.set_justify(gtk::Justification::Center);
        card_title.set_xalign(0.5);
        let card_detail = label("", "mirror-card-detail");
        card_detail.set_wrap(true);
        card_detail.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        card_detail.set_justify(gtk::Justification::Center);
        card_detail.set_xalign(0.5);
        card_detail.set_selectable(true);
        card_detail.set_lines(8);
        card_detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let card_code = label("", "mirror-card-code");
        card_code.set_xalign(0.5);
        card_code.set_selectable(true);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_halign(gtk::Align::Center);
        actions.add_css_class("mirror-card-actions");
        let card_retry = gtk::Button::new();
        let retry_face = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        retry_face.append(&glyphs::image("retry", 14));
        retry_face.append(&gtk::Label::new(Some("Retry")));
        card_retry.set_child(Some(&retry_face));
        card_retry.add_css_class("primary");
        let card_other = gtk::Button::with_label("Copy details");
        card_other.add_css_class("quiet");
        actions.append(&card_retry);
        actions.append(&card_other);
        for widget in [card_spinner.upcast_ref::<gtk::Widget>(), card_title.upcast_ref(), card_detail.upcast_ref(), card_code.upcast_ref(), actions.upcast_ref()] {
            card.append(widget);
        }
        screen.add_overlay(&card);
        // Short confirmations ("Saved …", "Copied …") float over the foot of the screen.
        let hint = label("", "mirror-toast");
        hint.set_halign(gtk::Align::Center);
        hint.set_valign(gtk::Align::End);
        hint.set_can_target(false);
        hint.set_visible(false);
        screen.add_overlay(&hint);

        let aspect = gtk::AspectFrame::new(0.5, 0.5, 9.0 / 19.5, false);
        aspect.set_child(Some(&screen));
        aspect.set_hexpand(true);
        aspect.set_vexpand(true);
        aspect.add_css_class("mirror-device");
        stage.attach(&aspect, 0, 0, 1, 1);

        // The rail: switcher, hardware keys, navigation, device panels; window actions at the foot.
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rail.add_css_class("mirror-rail");
        rail.set_vexpand(true);
        let mut controls = Vec::new();
        let display_key = glyphs::key("screen-off", "Turn the device screen off (mirroring continues)");
        let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
        head.add_css_class("mirror-rail-group");
        head.append(&picker);
        rail.append(&head);
        for group in input::RAIL {
            let block = gtk::Box::new(gtk::Orientation::Vertical, 2);
            block.add_css_class("mirror-rail-group");
            for (glyph, caption, kind) in group {
                let key = glyphs::key(glyph, caption);
                key.set_widget_name(kind);
                block.append(&key);
                controls.push(key);
            }
            rail.append(&block);
        }
        let block = gtk::Box::new(gtk::Orientation::Vertical, 2);
        block.add_css_class("mirror-rail-group");
        block.append(&display_key);
        rail.append(&block);
        controls.push(display_key.clone());
        let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        spacer.set_vexpand(true);
        rail.append(&spacer);
        let shot_key = glyphs::key("screenshot", "Save a screenshot");
        let dock_key = glyphs::key("dock", "Dock beside the wall");
        let fullscreen_key = glyphs::key("fullscreen", "Full screen (F11)");
        let close_key = glyphs::key("close", "Close mirror");
        let foot = gtk::Box::new(gtk::Orientation::Vertical, 2);
        foot.add_css_class("mirror-rail-group");
        foot.add_css_class("mirror-rail-foot");
        for key in [&shot_key, &dock_key, &fullscreen_key, &close_key] {
            foot.append(key);
        }
        rail.append(&foot);
        stage.attach(&rail, 1, 0, 1, 1);
        root.append(&stage);

        let view = Rc::new(Self {
            ui: Rc::downgrade(ui),
            device: RefCell::new(device),
            model: RefCell::default(),
            avd: RefCell::default(),
            booting: Cell::new(false),
            boot_tries: Cell::new(0),
            root,
            lamp,
            title,
            meta,
            picker,
            picker_list,
            dock_key,
            fullscreen_key,
            close_key,
            shot_key,
            display_key,
            controls,
            aspect,
            picture,
            card,
            card_spinner,
            card_title,
            card_detail,
            card_code,
            card_retry,
            card_other,
            hint,
            hint_generation: Cell::new(0),
            phase: Cell::new(Phase::Connecting),
            dims: Cell::new((0, 0)),
            texture: RefCell::default(),
            display_on: Cell::new(true),
            problem: RefCell::default(),
            input: RefCell::default(),
            task: RefCell::default(),
            generation: Cell::new(0),
            host: RefCell::new(Host::None),
            fullscreen: Cell::new(false),
            pointer: Cell::new(None),
            held: RefCell::default(),
        });
        view.wire();
        view
    }

    /// Connect every control. Closures hold the view weakly; the registry owns it.
    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let with = move |f: fn(&Rc<MirrorView>)| {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    f(&view);
                }
            }
        };
        let action = with(|view| view.save_screenshot());
        self.shot_key.connect_clicked(move |_| action());
        let action = with(|view| if matches!(*view.host.borrow(), Host::Dock(_)) { view.detach() } else { view.dock() });
        self.dock_key.connect_clicked(move |_| action());
        let action = with(|view| view.toggle_fullscreen());
        self.fullscreen_key.connect_clicked(move |_| action());
        let action = with(|view| view.close());
        self.close_key.connect_clicked(move |_| action());
        let action = with(|view| view.retry());
        self.card_retry.connect_clicked(move |_| action());
        let weak = Rc::downgrade(self);
        self.card_other.connect_clicked(move |button| {
            if let Some(view) = weak.upgrade() {
                if view.card_other.label().as_deref() == Some("Choose device") {
                    view.picker.popup();
                } else {
                    button.clipboard().set_text(&view.problem.borrow());
                    view.flash("Copied the error details.");
                }
            }
        });
        let weak = Rc::downgrade(self);
        if let Some(popover) = self.picker.popover() {
            popover.connect_show(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.fill_picker();
                }
            });
        }
        for key in &self.controls {
            let weak = Rc::downgrade(self);
            key.connect_clicked(move |key| {
                let Some(view) = weak.upgrade() else { return };
                if key == &view.display_key {
                    let on = !view.display_on.get();
                    view.send(input::display_power(on));
                    view.display_on.set(on);
                    if on {
                        glyphs::rekey(key, "screen-off", "Turn the device screen off (mirroring continues)");
                    } else {
                        glyphs::rekey(key, "screen-on", "Turn the device screen back on");
                    }
                } else {
                    view.send(input::button(key.widget_name().as_str()));
                }
            });
        }
        self.wire_pointer();
        self.wire_keys();
    }

    fn wire_pointer(self: &Rc<Self>) {
        // Primary button: a finger. Down, coalesced moves, up — a drag that leaves the picture
        // keeps tracking along its edge, and a cancelled gesture still lifts the finger.
        let drag = gtk::GestureDrag::new();
        drag.set_button(gtk::gdk::BUTTON_PRIMARY);
        let last = Rc::new(Cell::new(None::<(i32, i32)>));
        let weak = Rc::downgrade(self);
        let held = last.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let Some(view) = weak.upgrade() else { return };
            view.picture.grab_focus();
            if let Some(point) = view.map(x, y, false) {
                held.set(Some(point));
                view.send(input::touch(input::DOWN, point, view.dims.get()));
            }
        });
        let weak = Rc::downgrade(self);
        let held = last.clone();
        drag.connect_drag_update(move |gesture, dx, dy| {
            let Some(view) = weak.upgrade() else { return };
            let (Some((sx, sy)), Some(previous)) = (gesture.start_point(), held.get()) else { return };
            if let Some(point) = view.map(sx + dx, sy + dy, true) {
                if point != previous {
                    held.set(Some(point));
                    view.send(input::touch(input::MOVE, point, view.dims.get()));
                }
            }
        });
        let weak = Rc::downgrade(self);
        let held = last.clone();
        drag.connect_drag_end(move |gesture, dx, dy| {
            let Some(view) = weak.upgrade() else { return };
            let Some(previous) = held.take() else { return };
            let point = gesture.start_point().and_then(|(sx, sy)| view.map(sx + dx, sy + dy, true)).unwrap_or(previous);
            view.send(input::touch(input::UP, point, view.dims.get()));
        });
        let weak = Rc::downgrade(self);
        drag.connect_cancel(move |_, _| {
            if let (Some(view), Some(point)) = (weak.upgrade(), last.take()) {
                view.send(input::touch(input::UP, point, view.dims.get()));
            }
        });
        self.picture.add_controller(drag);

        // Secondary = Back, middle = Home: the two things a desktop mouse reaches for most.
        let click = gtk::GestureClick::new();
        click.set_button(0);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, _, _, _| {
            let Some(view) = weak.upgrade() else { return };
            match gesture.current_button() {
                gtk::gdk::BUTTON_SECONDARY => view.send(input::button("back")),
                gtk::gdk::BUTTON_MIDDLE => view.send(input::button("home")),
                _ => {}
            }
        });
        self.picture.add_controller(click);

        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(self);
        motion.connect_motion(move |_, x, y| {
            if let Some(view) = weak.upgrade() {
                view.pointer.set(Some((x, y)));
            }
        });
        self.picture.add_controller(motion);

        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        let weak = Rc::downgrade(self);
        scroll.connect_scroll(move |controller, dx, dy| {
            let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
            let (w, h) = view.dims.get();
            let at = view.pointer.get().and_then(|(x, y)| view.map(x, y, false)).unwrap_or((w / 2, h / 2));
            let pixels = controller.unit() == gtk::gdk::ScrollUnit::Surface;
            let (hscroll, vscroll) = input::scroll_amount(dx, dy, pixels);
            if hscroll != 0.0 || vscroll != 0.0 {
                view.send(input::scroll(at, (w, h), hscroll, vscroll));
            }
            glib::Propagation::Stop
        });
        self.picture.add_controller(scroll);
    }

    fn wire_keys(self: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, hardware, modifiers| {
            let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
            if key == gtk::gdk::Key::F11 || (key == gtk::gdk::Key::Escape && view.fullscreen.get()) {
                view.toggle_fullscreen();
                return glib::Propagation::Stop;
            }
            if view.phase.get() != Phase::Live {
                return glib::Propagation::Proceed;
            }
            let ctrl = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let alt = modifiers.contains(gtk::gdk::ModifierType::ALT_MASK);
            if ctrl && !alt && matches!(key, gtk::gdk::Key::v | gtk::gdk::Key::V) {
                // Paste the desktop clipboard into the focused field on the device: one
                // clipboard message instead of a keystroke per character.
                let clipboard = view.picture.clipboard();
                let weak = Rc::downgrade(&view);
                glib::spawn_future_local(async move {
                    if let Ok(Some(text)) = clipboard.read_text_future().await {
                        if let Some(view) = weak.upgrade() {
                            // relay_core::mirror::SET_CLIPBOARD_MAX_LENGTH: the engine refuses a
                            // longer one, and input is sent unanswered, so the paste would
                            // otherwise just not happen.
                            const PASTE_MAX: usize = (1 << 18) - 14;
                            if text.len() > PASTE_MAX {
                                view.flash(&format!("Too long to paste: {} KB, the most is 255 KB.", text.len() / 1024));
                            } else {
                                view.send(input::paste(text.as_str()));
                            }
                        }
                    }
                });
                return glib::Propagation::Stop;
            }
            let meta = input::meta(modifiers);
            if let Some(code) = input::keycode(key).or_else(|| (ctrl || alt).then(|| input::chord_keycode(key)).flatten()) {
                view.send(input::key(input::DOWN, code, meta));
                view.held.borrow_mut().insert(hardware, code);
                return glib::Propagation::Stop;
            }
            if !ctrl && !alt {
                if let Some(c) = key.to_unicode().filter(|c| !c.is_control()) {
                    view.send(input::text(c));
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(self);
        keys.connect_key_released(move |_, _, hardware, modifiers| {
            let Some(view) = weak.upgrade() else { return };
            let Some(code) = view.held.borrow_mut().remove(&hardware) else { return };
            view.send(input::key(input::UP, code, input::meta(modifiers)));
        });
        self.root.add_controller(keys);
    }

    fn map(&self, x: f64, y: f64, clamp: bool) -> Option<(i32, i32)> {
        let area = (self.picture.width() as f64, self.picture.height() as f64);
        if clamp {
            input::map_point_clamped(area, self.dims.get(), x, y)
        } else {
            input::map_point(area, self.dims.get(), x, y)
        }
    }

    /// Queue one input event. Only a live mirror takes input; the queue is unbounded and the
    /// sender collapses moves, so no burst of motion can end the mirror.
    fn send(&self, event: Value) {
        if self.phase.get() != Phase::Live {
            return;
        }
        if let Some(input) = self.input.borrow().as_ref() {
            let _ = input.try_send(event);
        }
    }

    // ------------------------------------------------------------------ hosts

    fn present(&self) {
        match &*self.host.borrow() {
            Host::Window(window) => window.present(),
            Host::Dock(_) => {
                self.picture.grab_focus();
            }
            Host::None => {}
        }
    }

    /// Take the view out of its current host without stopping anything.
    fn release(&self) {
        match self.host.replace(Host::None) {
            Host::Window(window) => {
                window.set_child(None::<&gtk::Widget>);
                // `destroy`, not `close`: close-request is what ends the mirror.
                window.destroy();
                // A destroyed window never reports leaving full screen; the view leaves it here.
                self.fullscreen.set(false);
                self.root.remove_css_class("fullscreen");
                glyphs::rekey(&self.fullscreen_key, "fullscreen", "Full screen (F11)");
            }
            Host::Dock(dock) => {
                dock.body.remove(&self.root);
                dock.close();
            }
            Host::None => {}
        }
    }

    fn detach(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        self.release();
        let (width, height) = self.window_size(&ui);
        let window = gtk::Window::builder()
            .application(&ui.window.application().unwrap())
            .transient_for(&ui.window)
            .title(self.window_title())
            .default_width(width)
            .default_height(height)
            .build();
        window.add_css_class("mirror-window");
        window.set_child(Some(&self.root));
        let weak = Rc::downgrade(self);
        window.connect_close_request(move |_| {
            if let Some(view) = weak.upgrade() {
                // The window is closing itself; forget it so `close` does not destroy it again.
                *view.host.borrow_mut() = Host::None;
                view.close();
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(self);
        window.connect_fullscreened_notify(move |window| {
            if let Some(view) = weak.upgrade() {
                view.fullscreen.set(window.is_fullscreen());
                if window.is_fullscreen() {
                    glyphs::rekey(&view.fullscreen_key, "exit-fullscreen", "Leave full screen (F11)");
                    view.root.add_css_class("fullscreen");
                } else {
                    glyphs::rekey(&view.fullscreen_key, "fullscreen", "Full screen (F11)");
                    view.root.remove_css_class("fullscreen");
                }
            }
        });
        glyphs::rekey(&self.dock_key, "dock", "Dock beside the wall");
        self.close_key.set_visible(false);
        self.root.remove_css_class("docked");
        *self.host.borrow_mut() = Host::Window(window.clone());
        PREFER_DOCK.with(|prefer| prefer.set(false));
        window.present();
        self.picture.grab_focus();
    }

    fn dock(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        // The dock takes the right edge; the Devices panel the mirror was opened from would sit
        // on top of or under it, so it steps aside. Only that one: a run's log panel closed
        // here could not be reopened, and a panel with unsaved changes stays.
        let panels = ui.panels.borrow().clone();
        for panel in panels.iter().filter(|panel| std::iter::successors(panel.body.parent(), |widget| widget.parent()).any(|widget| widget.has_css_class("device-panel"))) {
            panel.close();
        }
        // One dock: a mirror already docked moves out to its own window.
        let docked = VIEWS.with(|views| {
            views.borrow().iter().filter(|view| !Rc::ptr_eq(view, self) && matches!(*view.host.borrow(), Host::Dock(_))).cloned().collect::<Vec<_>>()
        });
        for view in docked {
            view.detach();
        }
        if self.fullscreen.get() {
            self.toggle_fullscreen();
        }
        self.release();
        let dock = Dock::new(&ui, DOCK_WIDTH);
        dock.body.append(&self.root);
        glyphs::rekey(&self.dock_key, "undock", "Open in its own window");
        self.close_key.set_visible(true);
        self.root.add_css_class("docked");
        *self.host.borrow_mut() = Host::Dock(dock);
        PREFER_DOCK.with(|prefer| prefer.set(true));
        self.picture.grab_focus();
    }

    fn toggle_fullscreen(self: &Rc<Self>) {
        if matches!(*self.host.borrow(), Host::Dock(_)) {
            self.detach();
        }
        if let Host::Window(window) = &*self.host.borrow() {
            if window.is_fullscreen() {
                window.unfullscreen();
            } else {
                window.fullscreen();
            }
        }
    }

    /// End the mirror and drop the view. Dropping the session drops its engine connection,
    /// and the engine stops a mirror whose connection is gone.
    fn close(&self) {
        self.generation.set(self.generation.get() + 1);
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.input.borrow_mut().take();
        self.held.borrow_mut().clear();
        self.release();
        // By identity: a view still booting its AVD has no serial yet.
        VIEWS.with(|views| views.borrow_mut().retain(|view| !std::ptr::eq(Rc::as_ptr(view), self)));
    }

    fn window_title(&self) -> String {
        let model = self.model.borrow();
        if model.is_empty() { format!("Device · {}", self.device.borrow()) } else { format!("{model} · Relay") }
    }

    /// The main window's monitor, in logical pixels.
    fn screen(ui: &Ui) -> (i32, i32) {
        let monitor = ui.window.surface().and_then(|surface| WidgetExt::display(&ui.window).monitor_at_surface(&surface));
        monitor.map(|monitor| (monitor.geometry().width(), monitor.geometry().height())).unwrap_or((1920, 1080))
    }

    /// The tallest a mirror window asks to be.
    fn max_height(ui: &Ui) -> i32 {
        (Self::screen(ui).1 * 85 / 100).clamp(560, 960)
    }

    /// A phone-shaped window: tall, sized to the monitor, wide enough for the key rail.
    fn window_size(&self, ui: &Ui) -> (i32, i32) {
        let height = Self::max_height(ui);
        let (w, h) = match self.dims.get() {
            (w, h) if w > 0 && h > 0 => (w as f64, h as f64),
            _ => (9.0, 19.5),
        };
        let phone = ((height - CHROME_H) as f64 * w / h) as i32;
        (phone + CHROME_W, height)
    }

    /// The capture rung to ask for: the monitor's height in device pixels, capped.
    fn capture_size(&self, ui: &Ui) -> i32 {
        let monitor = ui.window.surface().and_then(|surface| WidgetExt::display(&ui.window).monitor_at_surface(&surface));
        monitor.map(|monitor| monitor.geometry().height() * monitor.scale_factor()).unwrap_or(1024).clamp(1024, MAX_CAPTURE)
    }

    // ------------------------------------------------------------------ session

    /// (Re)start the mirror on the current device. Any running session is dropped first.
    fn start(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        let (input, inputs) = async_channel::unbounded();
        *self.input.borrow_mut() = Some(input);
        self.held.borrow_mut().clear();
        self.display_on.set(true);
        glyphs::rekey(&self.display_key, "screen-off", "Turn the device screen off (mirroring continues)");
        if self.booting.get() {
            self.connecting("Waiting for Android to finish starting…");
        } else {
            self.connecting("Asking Relay to start the mirror…");
        }
        let device = self.device.borrow().clone();
        let max_size = self.capture_size(&ui);
        let task = glib::spawn_future_local(session(Rc::downgrade(self), generation, device, max_size, ui.rt.clone(), ui.path.clone(), inputs));
        *self.task.borrow_mut() = Some(task);
        // The model name, for the switcher and the cards, ahead of the handshake.
        if self.model.borrow().is_empty() {
            let weak = Rc::downgrade(self);
            glib::spawn_future_local(async move {
                let Some(ui) = weak.upgrade().and_then(|view| view.ui.upgrade()) else { return };
                if let Ok(list) = ui.call("device.list", json!({})).await {
                    if let Some(view) = weak.upgrade() {
                        let device = view.device.borrow().clone();
                        if let Some(model) = list["devices"].as_array().into_iter().flatten().find(|d| d["serial"] == device.as_str()).and_then(|d| d["model"].as_str()) {
                            if view.model.borrow().is_empty() {
                                view.set_model(model);
                            }
                        }
                    }
                }
            });
        }
    }

    /// Boot this view's AVD headless, wait for adb to list it, then start the mirror on it.
    fn boot(self: &Rc<Self>, cold: bool) {
        let (Some(ui), Some(name)) = (self.ui.upgrade(), self.avd.borrow().clone()) else { return };
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.booting.set(true);
        self.boot_tries.set(0);
        self.connecting(if cold { "Cold-booting the virtual device…" } else { "Booting the virtual device…" });
        self.card_title.set_text(&format!("Starting {}", self.name()));
        let weak = Rc::downgrade(self);
        let task = glib::spawn_future_local(async move {
            let alive = || weak.upgrade().filter(|view| view.current(generation));
            if let Err(error) = ui.call("avd.boot", json!({"name":name,"cold":cold})).await {
                if let Some(view) = alive() {
                    view.booting.set(false);
                    view.ended(&format!("{} didn't boot", view.name()), &error.to_string(), None, Some("Copy details"));
                }
                return;
            }
            let found = glib::future_with_timeout(BOOT_WAIT, wait_for_avd(&alive, &name, &ui.rt, &ui.path)).await;
            let Some(view) = alive() else { return };
            match found {
                Ok(Ok(serial)) => {
                    *view.device.borrow_mut() = serial;
                    drop(view);
                    // adb lists an emulator a moment before it accepts a push.
                    glib::timeout_future(Duration::from_millis(800)).await;
                    if let Some(view) = alive() {
                        view.start();
                    }
                }
                Ok(Err(reason)) => {
                    view.booting.set(false);
                    view.ended(&format!("{} didn't boot", view.name()), &reason, None, Some("Copy details"));
                }
                Err(_) => {
                    view.booting.set(false);
                    view.ended(&format!("{} is taking too long to boot", view.name()), "adb never listed the emulator. Check the AVD in Android Studio's Device Manager, or retry with a cold boot.", None, Some("Copy details"));
                }
            }
        });
        *self.task.borrow_mut() = Some(task);
    }

    /// Retry: a view that never reached its AVD boots it again; anything else restarts the mirror.
    fn retry(self: &Rc<Self>) {
        if self.device.borrow().is_empty() && self.avd.borrow().is_some() {
            self.boot(false);
        } else {
            self.start();
        }
    }

    fn current(&self, generation: u64) -> bool {
        self.generation.get() == generation
    }

    fn switch_device(self: &Rc<Self>, serial: String) {
        if *self.device.borrow() == serial {
            return;
        }
        let taken = VIEWS.with(|views| views.borrow().iter().find(|view| *view.device.borrow() == serial).cloned());
        if let Some(other) = taken {
            other.present();
            return;
        }
        *self.device.borrow_mut() = serial;
        self.avd.borrow_mut().take();
        self.booting.set(false);
        self.model.borrow_mut().clear();
        self.texture.borrow_mut().take();
        self.picture.set_paintable(None::<&gtk::gdk::Paintable>);
        self.dims.set((0, 0));
        self.title.set_text(&self.device.borrow());
        self.update_meta();
        self.start();
    }

    fn set_model(&self, model: &str) {
        let model = model.replace('_', " ");
        *self.model.borrow_mut() = model.clone();
        self.title.set_text(&model);
        if let Host::Window(window) = &*self.host.borrow() {
            window.set_title(Some(&self.window_title()));
        }
        if self.phase.get() == Phase::Connecting {
            self.card_title.set_text(&format!("Connecting to {model}"));
        }
    }

    fn name(&self) -> String {
        let model = self.model.borrow();
        if model.is_empty() { self.device.borrow().clone() } else { model.clone() }
    }

    fn update_meta(&self) {
        let (w, h) = self.dims.get();
        let device = self.device.borrow();
        let device = if device.is_empty() { "virtual device".to_string() } else { device.clone() };
        let text = match self.phase.get() {
            Phase::Live if w > 0 => format!("{device} · {w}×{h}"),
            Phase::Connecting if self.booting.get() => format!("{device} · booting"),
            Phase::Connecting => format!("{device} · connecting"),
            Phase::Ended => format!("{device} · not mirroring"),
            Phase::Live => device.clone(),
        };
        self.meta.set_text(&text);
        self.picker.set_tooltip_text(Some(&format!("{}\n{text}\nSwitch device", self.name())));
    }

    fn set_lamp(&self, state: &str) {
        for class in ["live", "held", "waiting"] {
            self.lamp.remove_css_class(class);
        }
        if !state.is_empty() {
            self.lamp.add_css_class(state);
        }
    }

    fn set_controls(&self, live: bool) {
        for key in &self.controls {
            key.set_sensitive(live);
        }
        self.shot_key.set_sensitive(self.texture.borrow().is_some());
    }

    fn connecting(&self, stage: &str) {
        self.phase.set(Phase::Connecting);
        self.set_lamp("waiting");
        self.set_controls(false);
        self.picture.add_css_class("dimmed");
        self.card.set_visible(true);
        self.card.remove_css_class("problem");
        self.card_spinner.set_visible(true);
        self.card_spinner.start();
        self.card_title.set_text(&format!("Connecting to {}", self.name()));
        self.card_detail.set_text(stage);
        self.card_code.set_visible(false);
        self.card_retry.set_visible(false);
        self.card_other.set_visible(false);
        self.update_meta();
    }

    fn live(&self) {
        if self.phase.get() == Phase::Live {
            return;
        }
        self.phase.set(Phase::Live);
        self.booting.set(false);
        self.set_lamp("live");
        self.card_spinner.stop();
        self.card.set_visible(false);
        self.picture.remove_css_class("dimmed");
        self.set_controls(true);
        self.update_meta();
    }

    /// A terminal card: what happened, why, and the next step.
    fn ended(&self, title: &str, detail: &str, code: Option<&str>, other: Option<&str>) {
        self.phase.set(Phase::Ended);
        self.input.borrow_mut().take();
        self.set_lamp("held");
        self.set_controls(false);
        self.picture.add_css_class("dimmed");
        self.card.set_visible(true);
        self.card.add_css_class("problem");
        self.card_spinner.stop();
        self.card_spinner.set_visible(false);
        self.card_title.set_text(title);
        self.card_detail.set_text(detail);
        self.card_code.set_text(code.unwrap_or(""));
        self.card_code.set_visible(code.is_some());
        self.card_retry.set_visible(true);
        self.card_other.set_visible(other.is_some());
        if let Some(other) = other {
            self.card_other.set_label(other);
        }
        *self.problem.borrow_mut() = match code {
            Some(code) => format!("{title}\n{detail}\n{code}"),
            None => format!("{title}\n{detail}"),
        };
        self.update_meta();
    }

    fn apply_status(&self, status: &Value) {
        if let Some(name) = status["name"].as_str().filter(|name| !name.is_empty()) {
            if self.model.borrow().is_empty() {
                self.set_model(name);
            }
        }
        let size = (status["width"].as_i64().unwrap_or(0) as i32, status["height"].as_i64().unwrap_or(0) as i32);
        if self.texture.borrow().is_none() && size.0 > 0 && size.1 > 0 {
            self.set_dims(size);
        }
        match status["state"].as_str() {
            Some("starting") if self.phase.get() == Phase::Connecting => {
                self.card_detail.set_text("Starting the screen server on the device…");
            }
            Some("running") if self.phase.get() == Phase::Connecting => {
                self.card_detail.set_text("Waiting for the first picture…");
            }
            _ => {}
        }
    }

    fn set_dims(&self, (w, h): (i32, i32)) {
        if self.dims.get() == (w, h) {
            return;
        }
        let (ow, oh) = self.dims.replace((w, h));
        self.aspect.set_ratio(w as f32 / h as f32);
        self.update_meta();
        // The device rotated: turn the window with it, so a landscape app is not a thin strip
        // in a tall window. The phone's width becomes its height, within what a new window may
        // take, and the width follows the picture; the chrome around it stays put.
        let rotated = ow > 0 && oh > 0 && (ow > oh) != (w > h);
        if let (Host::Window(window), Some(ui)) = (&*self.host.borrow(), self.ui.upgrade()) {
            if rotated && !window.is_fullscreen() && !window.is_maximized() {
                let ratio = w as f64 / h as f64;
                let widest = Self::screen(&ui).0 * 9 / 10 - CHROME_W;
                let mut phone_h = (window.width() - CHROME_W).clamp(120, Self::max_height(&ui) - CHROME_H);
                if (phone_h as f64 * ratio) as i32 > widest {
                    phone_h = (widest as f64 / ratio) as i32;
                }
                let phone_w = (phone_h as f64 * ratio) as i32;
                window.set_default_size(phone_w + CHROME_W, phone_h + CHROME_H);
            }
        }
    }

    fn paint(&self, frame: decode::Pixels) {
        let bytes = glib::Bytes::from_owned(frame.rgb);
        let texture = gtk::gdk::MemoryTexture::new(frame.width, frame.height, gtk::gdk::MemoryFormat::R8g8b8, &bytes, frame.width as usize * 3);
        self.picture.set_paintable(Some(&texture));
        *self.texture.borrow_mut() = Some(texture);
        self.set_dims((frame.width, frame.height));
        if self.phase.get() == Phase::Connecting {
            self.live();
        }
    }

    /// Show `message` over the foot of the screen for a few seconds.
    fn flash(self: &Rc<Self>, message: &str) {
        let generation = self.hint_generation.get() + 1;
        self.hint_generation.set(generation);
        self.hint.set_text(message);
        self.hint.set_visible(true);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_secs(4), move || {
            if let Some(view) = weak.upgrade().filter(|view| view.hint_generation.get() == generation) {
                view.hint.set_visible(false);
            }
        });
    }

    fn save_screenshot(self: &Rc<Self>) {
        let Some(texture) = self.texture.borrow().clone() else { return };
        let stamp = glib::DateTime::now_local().ok().and_then(|now| now.format("%Y%m%d-%H%M%S").ok()).map(|s| s.to_string()).unwrap_or_default();
        let name = format!("{}-{stamp}.png", self.name().replace(' ', "-"));
        let chooser = gtk::FileDialog::builder().title("Save device screenshot").initial_name(name.as_str()).build();
        let parent = self.root.root().and_downcast::<gtk::Window>();
        let Some(rt) = self.ui.upgrade().map(|ui| ui.rt.clone()) else { return };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(file) = chooser.save_future(parent.as_ref()).await else { return };
            // Encode on the blocking pool and write through GIO, so a slow disk never stalls
            // the main loop and a location with no local path still saves.
            let saved = match rt.spawn_blocking(move || texture.save_to_png_bytes()).await {
                Ok(bytes) => file.replace_contents_future(bytes, None, false, gtk::gio::FileCreateFlags::REPLACE_DESTINATION).await.map(|_| ()).map_err(|(_, error)| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            let Some(view) = weak.upgrade() else { return };
            match saved {
                Ok(()) => view.flash(&format!("Saved {}", file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
                Err(error) => view.flash(&format!("Could not save the screenshot: {error}")),
            }
        });
    }

    /// Fill the device picker from `device.list` each time it opens.
    fn fill_picker(self: &Rc<Self>) {
        crate::app::clear(&self.picker_list);
        self.picker_list.append(&label("Loading devices…", "mirror-meta"));
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Some(ui) = weak.upgrade().and_then(|view| view.ui.upgrade()) else { return };
            let listed = ui.call("device.list", json!({})).await;
            let Some(view) = weak.upgrade() else { return };
            crate::app::clear(&view.picker_list);
            let devices = match listed {
                Ok(value) => value["devices"].as_array().cloned().unwrap_or_default(),
                Err(error) => {
                    view.picker_list.append(&label(&error.to_string(), "mirror-meta"));
                    return;
                }
            };
            if devices.is_empty() {
                view.picker_list.append(&label("No devices. Connect a phone with USB debugging on.", "mirror-meta"));
            }
            for device in devices {
                let serial = device["serial"].as_str().unwrap_or("").to_string();
                let state = device["state"].as_str().unwrap_or("");
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let current = *view.device.borrow() == serial;
                let mark = glyphs::image("check", 14);
                mark.set_opacity(if current { 1.0 } else { 0.0 });
                row.append(&mark);
                let names = gtk::Box::new(gtk::Orientation::Vertical, 0);
                names.append(&label(&device["model"].as_str().unwrap_or(&serial).replace('_', " "), "mirror-device-name"));
                let detail = if state == "device" { serial.clone() } else { format!("{serial} · {state}") };
                names.append(&label(&detail, "mirror-meta"));
                row.append(&names);
                let button = gtk::Button::new();
                button.set_child(Some(&row));
                button.add_css_class("quiet");
                button.add_css_class("mirror-device-row");
                button.set_sensitive(state == "device");
                let weak = Rc::downgrade(&view);
                button.connect_clicked(move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.picker.popdown();
                        view.switch_device(serial.clone());
                    }
                });
                view.picker_list.append(&button);
            }
        });
    }
}

/// How a session ended, as the stage explains it.
enum End {
    /// A terminal status object from the engine.
    Status(Value),
    /// The local pipeline or connection broke.
    Broken(String),
    /// The engine refused or could not be reached.
    Request(Error),
}

/// One mirror session: connect, start, stream, then explain the end. Runs on the GTK thread;
/// FFmpeg and the socket reads run on the tokio runtime.
async fn session(
    weak: Weak<MirrorView>,
    generation: u64,
    device: String,
    max_size: i32,
    rt: tokio::runtime::Handle,
    path: PathBuf,
    inputs: async_channel::Receiver<Value>,
) {
    let alive = || weak.upgrade().filter(|view| view.current(generation));
    let end = stream(&alive, &device, max_size, &rt, &path, inputs).await;
    let Some(view) = alive() else { return };
    // A fresh emulator is listed before Android is up, so its first attempts fail; keep the
    // booting card and try again rather than showing each of those as an error.
    if view.booting.get() && !matches!(end, End::Request(Error::Disconnected)) && view.boot_tries.get() < BOOT_TRIES {
        view.boot_tries.set(view.boot_tries.get() + 1);
        drop(view);
        glib::timeout_future(Duration::from_secs(3)).await;
        if let Some(view) = alive() {
            view.start();
        }
        return;
    }
    if let Some(view) = alive() {
        view.booting.set(false);
    }
    let name = view.name();
    let lost = match end {
        End::Status(status) => {
            let code = status["code"].as_str().unwrap_or("");
            let message = status["message"].as_str().unwrap_or("");
            match status["state"].as_str() {
                Some("lost") => {
                    view.ended(&format!("{name} disconnected"), "The mirror resumes on its own when it reconnects. Check the cable, or that wireless debugging is still on.", None, Some("Choose device"));
                    true
                }
                Some("stopped") => {
                    view.ended("Mirror stopped", "Something outside this window ended the mirror.", None, None);
                    false
                }
                _ => {
                    view.ended(failure_title(code), message, Some(code), Some("Copy details"));
                    false
                }
            }
        }
        End::Broken(message) => {
            let title = if message.starts_with("FFmpeg") { "FFmpeg is needed to show the screen" } else { "The mirror stopped" };
            view.ended(title, &message, None, Some("Copy details"));
            false
        }
        End::Request(Error::Bus(error)) => {
            let detail = match &error.hint {
                Some(hint) => format!("{} {}", sentence(&error.message), sentence(hint)),
                None => sentence(&error.message),
            };
            let lost = error.code == "device.none";
            let title = if lost { format!("{name} isn't connected") } else { failure_title(&error.code).to_string() };
            view.ended(&title, &detail, Some(&error.code), Some(if lost { "Choose device" } else { "Copy details" }));
            lost
        }
        End::Request(error) => {
            view.ended("Can't reach Relay's engine", &error.to_string(), None, Some("Copy details"));
            false
        }
    };
    drop(view);
    if lost && wait_for_device(&alive, &device, &rt, &path).await {
        // adb lists a device a moment before it accepts a push.
        glib::timeout_future(Duration::from_millis(800)).await;
        if let Some(view) = alive() {
            view.start();
        }
    }
}

async fn stream(
    alive: &impl Fn() -> Option<Rc<MirrorView>>,
    device: &str,
    max_size: i32,
    rt: &tokio::runtime::Handle,
    path: &std::path::Path,
    inputs: async_channel::Receiver<Value>,
) -> End {
    let (client, notices) = match Client::connect_with_limit(rt, path.to_path_buf(), MIRROR_MAX_LINE).await {
        Ok(connection) => connection,
        Err(error) => return End::Request(error),
    };
    let started = match client.request(rt, "device.mirror.start", json!({"device":device,"max_size":max_size})).await {
        Ok(started) => started,
        Err(error) => return End::Request(error),
    };
    let mirror_id = started["mirror_id"].as_i64().unwrap_or(0);
    if let Some(view) = alive() {
        view.apply_status(&json!({"state":"starting","width":started["width"],"height":started["height"]}));
    } else {
        return End::Broken(String::new());
    }

    // One slot: a picture waiting for the paint is replaced by a newer one, never queued behind.
    let (frames, pictures) = async_channel::bounded::<decode::Pixels>(1);
    let (control, controls) = async_channel::unbounded::<Control>();
    let worker = rt.spawn(decode::run(notices, frames, pictures.clone(), control));
    struct AbortOnDrop(tokio::task::AbortHandle);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _worker = AbortOnDrop(worker.abort_handle());
    // Resync requests share the input path, so they queue behind nothing but input.
    let (resync, resyncs) = async_channel::unbounded::<()>();

    let paint = async {
        while let Ok(picture) = pictures.recv().await {
            match alive() {
                Some(view) => view.paint(picture),
                None => break,
            }
        }
        std::future::pending::<()>().await;
    };
    let watch = async {
        while let Ok(message) = controls.recv().await {
            match message {
                Control::Status(status) => {
                    if matches!(status["state"].as_str(), Some("stopped" | "failed" | "lost")) {
                        return End::Status(status);
                    }
                    match alive() {
                        Some(view) => view.apply_status(&status),
                        None => return End::Broken(String::new()),
                    }
                }
                Control::Resync => {
                    let _ = resync.try_send(());
                }
                Control::Broken(message) => return End::Broken(message),
            }
        }
        End::Broken("The video pipeline stopped.".into())
    };
    let send = async {
        loop {
            let batch = tokio::select! {
                event = inputs.recv() => match event {
                    Ok(event) => {
                        let mut batch = vec![event];
                        while let Ok(more) = inputs.try_recv() {
                            batch.push(more);
                        }
                        input::coalesce(batch)
                    }
                    Err(_) => break,
                },
                Ok(()) = resyncs.recv() => vec![input::reset_video()],
            };
            // Written in order without waiting for each answer: the engine answers a connection's
            // input in the order it arrives, so a move no longer waits out the previous one's
            // round trip — and the GTK loop that resumes this future — before it leaves.
            for event in batch {
                if client.send_unanswered("device.mirror.input", json!({"mirror_id":mirror_id,"event":event})).await.is_err() {
                    return;
                }
            }
        }
        std::future::pending::<()>().await;
    };
    tokio::select! {
        end = watch => end,
        _ = paint => End::Broken(String::new()),
        _ = send => End::Broken("Relay's engine connection closed.".into()),
    }
}

/// Wait, event-driven, for `device` to come back: subscribe to `device.changed` on a connection
/// of our own, take a device-watch lease, and check `device.list` on each change.
async fn wait_for_device(
    alive: &impl Fn() -> Option<Rc<MirrorView>>,
    device: &str,
    rt: &tokio::runtime::Handle,
    path: &std::path::Path,
) -> bool {
    let Ok((client, notices)) = Client::connect(rt, path.to_path_buf()).await else { return false };
    if client.request(rt, "bus.subscribe", json!({"events":["device.changed"]})).await.is_err()
        || client.request(rt, "device.watch", json!({"on":true})).await.is_err()
    {
        return false;
    }
    loop {
        if alive().is_none() {
            return false;
        }
        if let Ok(list) = client.request(rt, "device.list", json!({})).await {
            let ready = list["devices"].as_array().into_iter().flatten().any(|d| d["serial"] == device && d["state"] == "device");
            if ready {
                return true;
            }
        }
        loop {
            match notices.recv().await {
                Ok(Notice::Event(event)) if event.ev == "device.changed" => break,
                Ok(Notice::Disconnected(_)) | Err(_) => return false,
                Ok(_) => {}
            }
        }
    }
}

/// Wait, event-driven, for the AVD `name` to show up in adb, and return its serial. `avd.boot`
/// reports a failed launch on `avd.changed`; the caller bounds the wait.
async fn wait_for_avd(
    alive: &impl Fn() -> Option<Rc<MirrorView>>,
    name: &str,
    rt: &tokio::runtime::Handle,
    path: &std::path::Path,
) -> Result<String, String> {
    let gone = || "The mirror was closed.".to_string();
    let (client, notices) = Client::connect(rt, path.to_path_buf()).await.map_err(|error| error.to_string())?;
    client.request(rt, "bus.subscribe", json!({"events":["device.changed","avd.changed"]})).await.map_err(|error| error.to_string())?;
    client.request(rt, "device.watch", json!({"on":true})).await.map_err(|error| error.to_string())?;
    loop {
        if alive().is_none() {
            return Err(gone());
        }
        if let Ok(list) = client.request(rt, "avd.list", json!({})).await {
            let serial = list["avds"].as_array().into_iter().flatten().find(|avd| avd["name"] == name).and_then(|avd| avd["running_serial"].as_str());
            if let Some(serial) = serial {
                return Ok(serial.to_string());
            }
        }
        loop {
            match notices.recv().await {
                Ok(Notice::Event(event)) if event.ev == "avd.changed" && event.payload["name"] == name && event.payload["state"] == "failed" => {
                    return Err("The Android emulator could not be launched. Check the SDK path in Settings.".into());
                }
                Ok(Notice::Event(event)) if event.ev == "device.changed" => break,
                Ok(Notice::Disconnected(error)) => return Err(error.to_string()),
                Err(_) => return Err(gone()),
                Ok(_) => {}
            }
        }
    }
}

fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut out = text.to_string();
    if let Some(first) = out.get(..1) {
        out.replace_range(..1, &first.to_uppercase());
    }
    if !out.is_empty() && !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// The card title for an engine failure code: what broke, in the user's terms.
fn failure_title(code: &str) -> &'static str {
    match code {
        "device.mirror_push_failed" => "Couldn't copy the screen server to the device",
        "device.mirror_forward_failed" => "adb couldn't open a tunnel to the device",
        "device.mirror_spawn_failed" | "device.mirror_ended" => "The screen server didn't start",
        "device.mirror_stream_failed" => "The screen stream stopped",
        "device.mirror_server_missing" => "This Relay install is missing its screen server",
        "device.not_ready" => "The device isn't ready — allow USB debugging on it",
        "device.size_failed" | "device.size_timeout" => "The device didn't report its screen size",
        "device.adb_timeout" | "device.adb_failed" | "device.adb_missing" => "adb isn't answering",
        _ => "The mirror stopped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_codes_have_specific_titles_and_messages_read_as_sentences() {
        assert_eq!(failure_title("device.mirror_push_failed"), "Couldn't copy the screen server to the device");
        assert_eq!(failure_title("device.something_new"), "The mirror stopped");
        assert_eq!(sentence("device relay-phone is not connected"), "Device relay-phone is not connected.");
        assert_eq!(sentence("Already fine."), "Already fine.");
        assert_eq!(sentence(""), "");
    }
}
