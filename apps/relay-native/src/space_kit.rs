//! The pieces every page of the Threads space draws the same way: Tally's pages, Avex's, the
//! thread page and its panel. What a failed call says, the key that tries again, and the panel's
//! titled blocks. A page's own look stays in its own file; one fix here reaches them all.
//!
//! The numbers around them read by `relay_client::figures`, one rule each.
use super::message;
use crate::app::{button, label, Ui};
use crate::client::Error;
use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

/// The codes an engine without an op answers with: an older engine, or one built without it.
fn missing_op(code: &str) -> bool {
    matches!(code, "bus.unknown_op" | "bus.not_implemented")
}

/// What a failed call says on a page: the engine's own sentence, never its code ("Invalid
/// money.invalid: …"). An engine without the op says `missing`, which names what to update.
pub(super) fn said(e: &Error, missing: &str) -> String {
    match e {
        Error::Bus(b) if missing_op(&b.code) => missing.to_string(),
        Error::Bus(b) => b.message.clone(),
        e => e.to_string(),
    }
}

/// "Try again" under a failed read: `again` reads it again.
pub(super) fn retry_key(ui: &Rc<Ui>, again: fn(&Rc<Ui>)) -> gtk::Button {
    let key = button("Try again", "money-secondary");
    key.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            again(&ui);
        }
    });
    key
}

/// A page whose read failed: why, in `body`, and the key that reads the page again.
pub(super) fn failed(ui: &Rc<Ui>, body: &gtk::Box, why: &str) {
    message(body, why);
    body.append(&retry_key(ui, |ui| ui.refresh_page()));
}

/// A titled block of the thread panel: its caption, an optional count on the right, and its rows
/// under it. Tally's, Avex's and Arbiter's panels draw theirs alike.
pub(super) fn panel_block(body: &gtk::Box, name: &str, aside: &str) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    block.add_css_class("threads-block");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = label(name, "threads-caption");
    title.set_hexpand(true);
    title.set_valign(gtk::Align::Center);
    head.append(&title);
    if !aside.is_empty() {
        head.append(&label(aside, "threads-panel-aside"));
    }
    block.append(&head);
    body.append(&block);
    block
}

/// A quiet key at the end of a [`panel_block`]'s head into the page that has all of it.
pub(super) fn block_link(block: &gtk::Box, caption: &str, run: impl Fn() + 'static) {
    let Some(head) = block.first_child().and_downcast::<gtk::Box>() else { return };
    let key = button(caption, "money-text-action");
    key.add_css_class("threads-panel-link");
    key.connect_clicked(move |_| run());
    head.append(&key);
}
