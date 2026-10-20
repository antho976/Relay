//! Design: a project's design canvases, beside Agents in the project's views. The gallery
//! lists the project's designs (each optionally a terminal's); opening one puts it on the
//! board, an infinite canvas of pages, frames, text, shapes, stickies, pen strokes and
//! pictures that everything on it can be clicked, edited and moved on. Agents make and change
//! designs through the same `design.*` ops, and an open board shows their changes as they land.
//! A design can also open in a window of its own.

mod ask;
mod assets;
mod board;
mod canvas;
mod context;
mod doc;
mod drive;
mod export;
mod gallery;
mod glyphs;
mod icons;
mod inspector;
mod layers;
mod motion_panel;
mod paint;
mod pictures;
mod render;
mod shape_panels;
mod textedit;
mod tools;
mod widgets;
mod window;

use crate::app::Ui;
use board::Board;
use doc::Doc;
use gallery::{Filter, Gallery};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// The Design view of the main window: the gallery, or one design on the board.
pub struct View {
    pub root: gtk::Stack,
    gallery: Rc<Gallery>,
    host: gtk::Box,
    board: RefCell<Option<Rc<Board>>>,
}

thread_local! {
    static VIEW: RefCell<Option<Rc<View>>> = const { RefCell::new(None) };
    /// Every open board, the main view's and the windows', for routing `design.changed`.
    static BOARDS: RefCell<Vec<Weak<Board>>> = const { RefCell::new(Vec::new()) };
}

fn view() -> Option<Rc<View>> {
    VIEW.with(|v| v.borrow().clone())
}

/// Build the view and mount it beside the agent wall.
pub fn install(ui: &Rc<Ui>) {
    let gallery = Gallery::new(ui);
    let root = gtk::Stack::new();
    root.add_css_class("design-view");
    root.set_hhomogeneous(false);
    root.set_vhomogeneous(false);
    root.set_transition_type(gtk::StackTransitionType::Crossfade);
    root.set_transition_duration(120);
    root.add_named(&gallery.root, Some("gallery"));
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.set_hexpand(true);
    host.set_vexpand(true);
    root.add_named(&host, Some("board"));
    root.set_visible_child_name("gallery");
    let view = Rc::new(View { root, gallery, host, board: RefCell::default() });
    let weak = Rc::downgrade(ui);
    *view.gallery.open.borrow_mut() = Some(Rc::new(move |id| {
        if let Some(ui) = weak.upgrade() {
            open(&ui, id);
        }
    }));
    ui.editor.mount_design(&view.root);
    VIEW.with(|v| *v.borrow_mut() = Some(view));
    let weak = Rc::downgrade(ui);
    ui.editor.connect_view_changed(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.editor.design_visible() {
                shown(&ui);
            }
        }
    });
}

/// The view came on screen, or its project changed: read the project's designs.
fn shown(ui: &Rc<Ui>) {
    let Some(view) = view() else {
        return;
    };
    let project = ui.project.get();
    if view.gallery.project.get() != project {
        view.gallery.project.set(project);
        *view.gallery.filter.borrow_mut() = Filter::All;
        view.gallery.reset();
        close_board(&view);
    }
    view.gallery.refresh();
    let board = view.board.borrow().clone();
    if let Some(b) = board {
        b.canvas.grab_focus();
    }
}

fn close_board(view: &View) {
    if let Some(b) = view.board.borrow_mut().take() {
        b.end_edit(true);
        view.host.remove(&b.root);
    }
    view.root.set_visible_child_name("gallery");
}

/// Show the Design view of the current project, filtered to `terminal`'s designs.
pub fn show(ui: &Rc<Ui>, terminal: Option<String>) {
    ui.navigate("design");
    if let Some(view) = view() {
        close_board(&view);
        *view.gallery.filter.borrow_mut() = match terminal {
            Some(t) => Filter::Terminal(t),
            None => Filter::All,
        };
        view.gallery.refresh();
    }
}

/// Open design `id` on the main view's board.
pub fn open(ui: &Rc<Ui>, id: i64) {
    let Some(view) = view() else {
        return;
    };
    if view.board.borrow().as_ref().is_some_and(|b| b.doc.id == id) {
        view.root.set_visible_child_name("board");
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let doc = match Doc::open(&ui, id).await {
            Ok(doc) => doc,
            Err(e) => {
                view.gallery.toast(&format!("Could not open the design: {e}"), None);
                return;
            }
        };
        close_board(&view);
        let b = Board::new(&ui, doc, false);
        let weak = Rc::downgrade(&view);
        *b.back.borrow_mut() = Some(Rc::new(move || {
            if let Some(view) = weak.upgrade() {
                close_board(&view);
                view.gallery.refresh();
            }
        }));
        view.host.append(&b.root);
        register(&b);
        *view.board.borrow_mut() = Some(b.clone());
        view.root.set_visible_child_name("board");
        b.canvas.grab_focus();
    });
}

/// A new design in `project`, tied to `session`, opened on the board.
pub fn create(ui: &Weak<Ui>, project: i64, session: Option<String>) {
    let Some(ui) = ui.upgrade() else {
        return;
    };
    glib::spawn_future_local(async move {
        // The first name no design has: "Untitled design", then "Untitled design 2" and on.
        let taken = view().map(|v| v.gallery.titles()).unwrap_or_default();
        let title = (1..)
            .map(|n| if n == 1 { "Untitled design".to_string() } else { format!("Untitled design {n}") })
            .find(|t| !taken.contains(t))
            .unwrap_or_default();
        match ui.call("design.create", json!({"project_id": project, "title": title, "session": session})).await {
            Ok(v) => {
                if let Some(id) = v["design"]["id"].as_i64() {
                    if ui.project.get() != project {
                        ui.open_project(project, "design");
                    } else if !ui.editor.design_visible() {
                        ui.navigate("design");
                    }
                    open(&ui, id);
                }
            }
            Err(e) => ui.show_error(&format!("Could not start a design: {e}")),
        }
    });
}

pub(crate) fn register(b: &Rc<Board>) {
    BOARDS.with(|list| {
        let mut list = list.borrow_mut();
        list.retain(|w| w.strong_count() > 0);
        list.push(Rc::downgrade(b));
    });
}

/// `design.changed` / `design.deleted`, whatever project the main window shows: a design in a
/// window keeps its own.
pub fn event(ui: &Rc<Ui>, ev: &str, payload: &Value) {
    let id = payload["id"].as_i64().unwrap_or(0);
    let boards: Vec<Rc<Board>> = BOARDS.with(|l| l.borrow().iter().filter_map(Weak::upgrade).collect());
    for b in boards.iter().filter(|b| b.doc.id == id) {
        if ev == "design.deleted" {
            continue;
        }
        b.doc.event(payload);
    }
    if ev == "design.deleted" || payload["deleted_at"].is_string() {
        window::close(id);
        if let Some(view) = view() {
            let open = view.board.borrow().as_ref().filter(|b| b.doc.id == id).map(|b| b.doc.title());
            if let Some(title) = open {
                close_board(&view);
                // Deleted elsewhere (another window, an agent): say so, and offer it back while
                // it is only in Deleted.
                let gallery = Rc::downgrade(&view.gallery);
                let restore: Option<Rc<dyn Fn()>> = (ev != "design.deleted").then(|| {
                    Rc::new(move || {
                        if let Some(g) = gallery.upgrade() {
                            g.restore(id);
                        }
                    }) as Rc<dyn Fn()>
                });
                view.gallery.toast_with(&format!("“{title}” was deleted"), "Restore", restore);
            }
        }
    }
    if let Some(view) = view() {
        if payload["project_id"].as_i64() == Some(view.gallery.project.get()) && ui.editor.design_visible() {
            view.gallery.refresh();
        }
    }
}

/// Something about a design changed from the board: the gallery reads again when it shows.
pub(crate) fn gallery_changed(_ui: &Rc<Ui>) {
    if let Some(view) = view() {
        view.gallery.refresh();
    }
}

fn duplicate_design(b: &Board) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let id = b.doc.id;
    glib::spawn_future_local(async move {
        match ui.call("design.duplicate", json!({"design_id": id})).await {
            Ok(v) => {
                if let Some(new) = v["id"].as_i64() {
                    open(&ui, new);
                }
            }
            Err(e) => ui.show_error(&e.to_string()),
        }
    });
}

fn delete_design(b: &Board) {
    let id = b.doc.id;
    let title = b.doc.title();
    if b.windowed {
        if let Some(ui) = b.ui.upgrade() {
            glib::spawn_future_local(async move {
                // The window closes as the design goes; the main window says where it went.
                match ui.call("design.delete", json!({"design_id": id})).await {
                    Ok(_) => ui.show_info(&format!("Moved “{title}” to Deleted designs")),
                    Err(e) => ui.show_error(&e.to_string()),
                }
            });
        }
        return;
    }
    if let Some(view) = view() {
        close_board(&view);
        view.gallery.delete(id, &title);
    }
}

/// For the smoke capture: open the Design view on the first design, or the gallery.
pub fn smoke_open(ui: &Rc<Ui>) {
    ui.navigate("design");
    let ui = ui.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
        let first = view().and_then(|v| v.gallery_first());
        if let Some(id) = first.filter(|_| std::env::var("RELAY_SMOKE_DESIGN_BOARD").is_ok()) {
            open(&ui, id);
            // A scripted run of gestures and commands (drive.rs), checked by the harness.
            if let Ok(script) = std::env::var("RELAY_SMOKE_DESIGN_SCRIPT") {
                glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                    if let Some(b) = view().and_then(|v| v.board.borrow().clone()) {
                        drive::run(b, script);
                    }
                });
                return;
            }
            // A layer to show selected, with its toolbar and the inspector.
            if let Ok(node) = std::env::var("RELAY_SMOKE_DESIGN_SELECT") {
                glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                    if let Some(b) = view().and_then(|v| v.board.borrow().clone()) {
                        b.select(vec![node]);
                    }
                });
            }
        }
    });
}

impl View {
    fn gallery_first(&self) -> Option<i64> {
        self.gallery.first()
    }
}
