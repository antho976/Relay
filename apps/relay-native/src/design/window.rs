//! A design in a window of its own, to keep beside the terminals or on another screen. It is
//! the same board as the Design view's, under the compact chrome the Notes window wears.

use super::board::Board;
use super::doc::Doc;
use crate::app::{label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    /// Open design windows by design id.
    static WINDOWS: RefCell<Vec<(i64, gtk::Window)>> = const { RefCell::new(Vec::new()) };
}

pub fn open(ui: &Rc<Ui>, _project: i64, id: i64) {
    let existing = WINDOWS.with(|w| w.borrow().iter().find(|(d, _)| *d == id).map(|(_, w)| w.clone()));
    if let Some(window) = existing {
        window.present();
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let doc = match Doc::open(&ui, id).await {
            Ok(doc) => doc,
            Err(e) => {
                ui.show_error(&format!("Could not open the design: {e}"));
                return;
            }
        };
        let board = Board::new(&ui, doc, true);
        let window = gtk::Window::builder()
            .application(&ui.window.application().expect("the app owns the main window"))
            .title(format!("{} — Relay Design", board.doc.title()))
            .default_width(1360)
            .default_height(880)
            .build();
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
        window.connect_close_request(move |_| {
            keep.end_edit(true);
            WINDOWS.with(|w| w.borrow_mut().retain(|(d, _)| *d != id));
            glib::Propagation::Proceed
        });
        WINDOWS.with(|w| w.borrow_mut().push((id, window.clone())));
        window.present();
        board.canvas.grab_focus();
    });
}

/// Close a design's window, when the design is deleted.
pub fn close(id: i64) {
    let window = WINDOWS.with(|w| w.borrow().iter().find(|(d, _)| *d == id).map(|(_, w)| w.clone()));
    if let Some(w) = window {
        w.close();
    }
}
