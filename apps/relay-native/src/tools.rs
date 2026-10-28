//! Auxiliary native workspaces. Forms stay mounted across engine events.
use crate::app::{button, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::Value;
use std::rc::Rc;

#[path = "tools_devices.rs"]
pub(crate) mod devices;
#[path = "tools_market.rs"]
mod market;
#[path = "tools_plugins.rs"]
pub(crate) mod plugins;
#[path = "tools_settings.rs"]
pub(crate) mod settings;
#[path = "tools_skills.rs"]
mod skills;

pub(super) fn paragraph(value: &str) -> gtk::Label {
    let l = label(value, "body");
    l.set_wrap(true);
    l.set_selectable(true);
    l
}

pub(super) fn section(parent: &gtk::Box, title: &str) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 8);
    block.add_css_class("record");
    block.append(&label(title, "title"));
    parent.append(&block);
    block
}

pub(super) fn action(
    ui: &Rc<Ui>,
    parent: &gtk::Box,
    title: &str,
    op: &'static str,
    payload: Value,
) {
    let key = button(title, "quiet");
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |b| {
        if let Some(ui) = weak.upgrade() {
            ui.mutate(op, payload.clone(), b);
        }
    });
    parent.append(&key);
}

pub(super) fn current(ui: &Ui, name: &str, project: i64, generation: u64) -> bool {
    ui.generation.get() == generation && ui.project.get() == project && *ui.page.borrow() == name
}

pub async fn refresh(ui: &Rc<Ui>, name: &str, project: i64) {
    match name {
        "settings" => return settings::refresh(ui, project).await,
        "skills" => return skills::refresh(ui, project).await,
        "plugins" => return plugins::refresh(ui, project).await,
        "devices" => devices::refresh(ui, project).await,
        _ => {}
    }
}
