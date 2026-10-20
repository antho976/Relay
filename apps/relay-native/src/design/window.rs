//! A design in a window of its own, to keep beside the terminals or on another screen. It is
//! the same board as the Design view's, under the compact chrome the Notes window wears. Each
//! window's size is kept, and the windows open when Relay closes open again when it starts
//! (I-15).

use super::board::Board;
use super::doc::Doc;
use super::Ready;
use crate::app::{label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Duration;

/// Where each design window's size is kept, and whether it was open when Relay last closed, by
/// design id (`native.design.windows.<id>`).
const PLACES: &str = "native.design.windows";
/// Design windows remembered; past it, those closed longest ago are forgotten.
const PLACES_KEPT: usize = 32;

/// A design window's size and state, as kept between runs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Place {
    project: i64,
    width: i32,
    height: i32,
    #[serde(default)]
    maximized: bool,
    /// Open when Relay last closed.
    #[serde(default)]
    open: bool,
    /// When it was last opened or closed, in seconds since 1970: the oldest closed go first.
    #[serde(default)]
    at: i64,
}

thread_local! {
    /// Open design windows by design id, with the board each shows.
    static WINDOWS: RefCell<Vec<(i64, gtk::Window, Weak<Board>)>> = const { RefCell::new(Vec::new()) };
    /// Designs whose window is still opening: a second click waits for the first.
    static OPENING: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
    /// What a board popped out while its design's window was still opening (B28): kept, so the
    /// design's queue and history are still there when that window has its board.
    static HANDED: RefCell<Vec<Rc<Doc>>> = const { RefCell::new(Vec::new()) };
    /// The windows' places, as read at the start and kept since (I-15).
    static KEPT: RefCell<BTreeMap<i64, Place>> = const { RefCell::new(BTreeMap::new()) };
    /// Each window's resizes counted, so that its size is kept once it rests.
    static RESIZES: RefCell<BTreeMap<i64, u64>> = const { RefCell::new(BTreeMap::new()) };
}

pub fn open(ui: &Rc<Ui>, project: i64, id: i64) {
    open_doc(ui, project, id, None, None);
}

/// Open design `id` in a window and run `ready` with its board once it has its size, to put
/// back what a board showed before (E-gallery-6).
// native-canvas restores a popped-out board's camera and selection through it.
#[allow(dead_code)]
pub(crate) fn open_with(ui: &Rc<Ui>, project: i64, id: i64, ready: impl FnOnce(&Rc<Board>) + 'static) {
    open_doc(ui, project, id, None, Some(Box::new(ready)));
}

/// The design on a board, popped out into a window: the same design, so its undo history comes
/// along, whether the window opens now or was open already (B28); `ready` then puts back what
/// the board showed (E-gallery-6).
pub fn pop_out(ui: &Rc<Ui>, doc: Rc<Doc>, ready: impl FnOnce(&Rc<Board>) + 'static) {
    let project = doc.design.borrow().project_id;
    open_doc(ui, project, doc.id, Some(doc.share()), Some(Box::new(ready)));
}

fn open_doc(ui: &Rc<Ui>, project: i64, id: i64, doc: Option<Rc<Doc>>, ready: Option<Ready>) {
    let existing = WINDOWS.with(|w| w.borrow().iter().find(|(d, ..)| *d == id).map(|(_, w, b)| (w.clone(), b.clone())));
    if let Some((window, board)) = existing {
        window.present();
        if let Some(board) = board.upgrade() {
            super::when_placed(&board, ready);
        }
        return;
    }
    if OPENING.with(|o| o.borrow().contains(&id)) {
        HANDED.with(|h| h.borrow_mut().extend(doc));
        return;
    }
    OPENING.with(|o| o.borrow_mut().push(id));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let opened = match doc {
            Some(doc) => Ok(doc),
            None => Doc::open(&ui, id).await,
        };
        OPENING.with(|o| o.borrow_mut().retain(|d| *d != id));
        // The window's doc shares the design they hold now.
        HANDED.with(|h| h.borrow_mut().retain(|d| d.id != id));
        let doc = match opened {
            Ok(doc) => doc,
            Err(e) => {
                ui.show_error(&format!("Could not open the design: {e}"));
                return;
            }
        };
        let board = Board::new(&ui, doc, true);
        let place = KEPT.with(|k| k.borrow().get(&id).cloned());
        let (width, height) = place.as_ref().map_or((1360, 880), |p| (p.width.clamp(480, 16384), p.height.clamp(320, 16384)));
        let window = gtk::Window::builder()
            .application(&ui.window.application().expect("the app owns the main window"))
            .title(format!("{} — Relay Design", board.doc.title()))
            .default_width(width)
            .default_height(height)
            .build();
        if place.is_some_and(|p| p.maximized) {
            window.maximize();
        }
        window.add_css_class("design-window");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let chrome = gtk::CenterBox::new();
        chrome.add_css_class("notes-chrome");
        let mark = super::glyphs::image("frame", 14);
        mark.add_css_class("notes-chrome-mark");
        mark.set_valign(gtk::Align::Center);
        chrome.set_start_widget(Some(&mark));
        let heading = label("Relay Design", "notes-heading");
        chrome.set_center_widget(Some(&heading));
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        controls.add_css_class("notes-chrome-controls");
        for (icon, hint) in [("minimize", "Minimize"), ("maximize", "Maximize"), ("close", "Close")] {
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
                        "maximize" if window.is_maximized() => window.unmaximize(),
                        "maximize" => window.maximize(),
                        _ => window.close(),
                    }
                }
            });
            controls.append(&key);
        }
        chrome.set_end_widget(Some(&controls));
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&chrome));
        let titlebar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        titlebar.set_visible(false);
        window.set_titlebar(Some(&titlebar));
        root.append(&handle);
        root.append(&board.root);
        window.set_child(Some(&root));
        super::register(&board);
        let keep = board.clone();
        let weak = Rc::downgrade(&ui);
        window.connect_close_request(move |window| {
            keep.end_edit(true);
            WINDOWS.with(|w| w.borrow_mut().retain(|(d, ..)| *d != id));
            // Closed by the person (or with its design): it does not come back at the start.
            if let Some(ui) = weak.upgrade() {
                keep_place(&ui, id, project, window, false);
            }
            glib::Propagation::Proceed
        });
        for property in ["default-width", "default-height", "maximized"] {
            let weak = Rc::downgrade(&ui);
            window.connect_notify_local(Some(property), move |window, _| resized(&weak, id, project, window));
        }
        WINDOWS.with(|w| w.borrow_mut().push((id, window.clone(), Rc::downgrade(&board))));
        keep_place(&ui, id, project, &window, true);
        window.present();
        board.canvas.grab_focus();
        super::when_placed(&board, ready);
    });
}

/// Close a design's window, when the design is deleted: what is being typed there is kept,
/// not committed into it (TM-50).
pub fn close(id: i64) {
    let found = WINDOWS.with(|w| w.borrow().iter().find(|(d, ..)| *d == id).map(|(_, w, b)| (w.clone(), b.clone())));
    let Some((window, board)) = found else {
        return;
    };
    if let Some(ui) = board.upgrade().and_then(|b| b.ui.upgrade()) {
        if super::rescue(&ui, id) {
            ui.show_info("The design was deleted. What you were typing is on the clipboard.");
        }
    }
    window.close();
}

/// Every design window closed, each board's words in progress committed first: the main window
/// is closing (TM-39). They stay kept as open, so they come back at the next start (I-15).
pub(crate) fn close_all() {
    let windows: Vec<_> = WINDOWS.with(|w| w.borrow_mut().drain(..).collect());
    for (_, window, board) in windows {
        if let Some(b) = board.upgrade() {
            b.end_edit(true);
        }
        window.destroy();
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Keep `place` for design `id`: whether that changes what is kept (its time aside), and the
/// ids to forget to stay within [`PLACES_KEPT`], those closed longest ago first, never an open
/// one.
fn remember(kept: &mut BTreeMap<i64, Place>, id: i64, place: Place) -> (bool, Vec<i64>) {
    let changed = kept.get(&id).is_none_or(|was| Place { at: place.at, ..was.clone() } != place);
    kept.insert(id, place);
    let mut closed: Vec<(i64, i64)> = kept.iter().filter(|(_, p)| !p.open).map(|(id, p)| (p.at, *id)).collect();
    closed.sort_unstable();
    let forget: Vec<i64> = closed.into_iter().take(kept.len().saturating_sub(PLACES_KEPT)).map(|(_, id)| id).collect();
    for id in &forget {
        kept.remove(id);
    }
    (changed, forget)
}

/// The places `settings.get` answers, by design id; one that does not read is left out.
fn read_places(value: &Value) -> BTreeMap<i64, Place> {
    let Some(map) = value.as_object() else {
        return BTreeMap::new();
    };
    map.iter().filter_map(|(id, place)| Some((id.parse().ok()?, serde_json::from_value(place.clone()).ok()?))).collect()
}

/// Keep `window`'s size and whether it is `open`, for design `id`.
fn keep_place(ui: &Rc<Ui>, id: i64, project: i64, window: &gtk::Window, open: bool) {
    let place = Place { project, width: window.default_width(), height: window.default_height(), maximized: window.is_maximized(), open, at: now() };
    let (changed, forget) = KEPT.with(|k| remember(&mut k.borrow_mut(), id, place.clone()));
    if !changed && forget.is_empty() {
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if changed {
            let _ = ui.call("settings.set", json!({"path": format!("{PLACES}.{id}"), "value": place})).await;
        }
        for gone in forget {
            let _ = ui.call("settings.reset", json!({"path": format!("{PLACES}.{gone}")})).await;
        }
    });
}

/// A window was resized or (un)maximized: its size is kept once it has rested.
fn resized(ui: &Weak<Ui>, id: i64, project: i64, window: &gtk::Window) {
    let count = RESIZES.with(|r| {
        let mut r = r.borrow_mut();
        let n = r.entry(id).or_default();
        *n += 1;
        *n
    });
    let (ui, window) = (ui.clone(), window.downgrade());
    glib::timeout_add_local_once(Duration::from_millis(600), move || {
        if RESIZES.with(|r| r.borrow().get(&id) != Some(&count)) {
            return;
        }
        let (Some(ui), Some(window)) = (ui.upgrade(), window.upgrade()) else {
            return;
        };
        // Still open: the close keeps the size of one closing meanwhile.
        if WINDOWS.with(|w| w.borrow().iter().any(|(d, w, _)| *d == id && *w == window)) {
            keep_place(&ui, id, project, &window, true);
        }
    });
}

/// Forget design `id`'s window.
fn forget(ui: &Rc<Ui>, id: i64) {
    KEPT.with(|k| k.borrow_mut().remove(&id));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let _ = ui.call("settings.reset", json!({"path": format!("{PLACES}.{id}")})).await;
    });
}

/// Once Relay is connected, read the kept places and open again the windows that were open when
/// it last closed, of designs still listed; forget those of designs gone (I-15). Not in a smoke
/// capture, which shows only what it set up.
pub(super) fn restore(ui: &Rc<Ui>) {
    if std::env::var_os("RELAY_NATIVE_SCREENSHOT").is_some() {
        return;
    }
    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        // The first connection, waited for two minutes at most.
        let mut waits = 0;
        let ui = loop {
            let ui = weak.upgrade()?;
            if ui.is_connected() {
                break ui;
            }
            drop(ui);
            waits += 1;
            if waits > 480 {
                return None;
            }
            glib::timeout_future(Duration::from_millis(250)).await;
        };
        let read = read_places(&ui.call("settings.get", json!({"path": PLACES})).await.ok()?["value"]);
        // What this run kept already (a window opened meanwhile) stays.
        KEPT.with(|k| {
            let mut k = k.borrow_mut();
            for (id, place) in &read {
                k.entry(*id).or_insert_with(|| place.clone());
            }
        });
        let mut projects: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for (id, place) in read.iter().filter(|(_, p)| p.open) {
            projects.entry(place.project).or_default().push(*id);
        }
        for (project, ids) in projects {
            let listed = match ui.call("design.list", json!({"project_id": project})).await {
                Ok(listed) => listed,
                // A project gone takes its designs' windows with it; anything else, try next time.
                Err(crate::client::Error::Bus(_)) => Value::Null,
                Err(_) => continue,
            };
            let live: HashSet<i64> = listed["designs"].as_array().into_iter().flatten().filter_map(|d| d["id"].as_i64()).collect();
            for id in ids {
                match live.contains(&id) {
                    true => open(&ui, project, id),
                    false => forget(&ui, id),
                }
            }
        }
        Some(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(open: bool, at: i64) -> Place {
        Place { project: 1, width: 1200, height: 800, maximized: false, open, at }
    }

    #[test]
    fn a_window_is_kept_by_design_and_the_longest_closed_go_first() {
        let mut kept = BTreeMap::new();
        assert_eq!(remember(&mut kept, 7, place(true, 10)), (true, vec![]), "a new window is written");
        assert_eq!(remember(&mut kept, 7, place(true, 20)), (false, vec![]), "only its time moved: nothing to write");
        let mut wider = place(true, 30);
        wider.width = 1400;
        assert_eq!(remember(&mut kept, 7, wider), (true, vec![]));
        assert!(remember(&mut kept, 7, place(false, 40)).0, "closing is written");
        for id in 100..100 + PLACES_KEPT as i64 - 1 {
            remember(&mut kept, id, place(id % 2 == 0, id));
        }
        assert_eq!(kept.len(), PLACES_KEPT);
        // One more: the closed one with the oldest time goes, never an open one.
        let (_, forget) = remember(&mut kept, 999, place(true, 999));
        assert_eq!(forget, vec![7]);
        assert!(!kept.contains_key(&7) && kept.contains_key(&999) && kept.len() == PLACES_KEPT);
        let (_, forget) = remember(&mut kept, 1000, place(true, 1000));
        assert_eq!(forget, vec![101], "then the next closed, by its time");
    }

    #[test]
    fn places_read_back_as_settings_keep_them() {
        let value = json!({
            "12": {"project": 3, "width": 900, "height": 700, "maximized": true, "open": true, "at": 5},
            "13": {"project": 3, "width": 900, "height": 700},
            "x": {"project": 3, "width": 900, "height": 700},
            "14": {"width": "wide"},
        });
        let read = read_places(&value);
        assert_eq!(read.keys().copied().collect::<Vec<_>>(), vec![12, 13], "an id or a place that does not read is left out");
        assert_eq!(read[&12], Place { project: 3, width: 900, height: 700, maximized: true, open: true, at: 5 });
        assert!(!read[&13].open && !read[&13].maximized, "absent flags read false");
        assert!(read_places(&Value::Null).is_empty());
    }
}
