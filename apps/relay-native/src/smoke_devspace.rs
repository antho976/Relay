//! Dev-space regressions from the 2026-10-24 survey (native-dev.md), against the fixture.
use crate::app::Ui;
use crate::smoke::util::{click, find, named, require, wait_for};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::rc::Rc;
use std::time::Duration;

async fn call(ui: &Rc<Ui>, op: &str, payload: Value) -> Result<Value, String> {
    ui.call(op, payload).await.map_err(|e| format!("{op}: {e}"))
}

pub(crate) async fn run(ui: &Rc<Ui>) -> Result<(), String> {
    task_page_keeps_typing(ui).await?;
    Ok(())
}

fn entry(ui: &Ui, name: &str) -> Result<gtk::Entry, String> {
    named(&ui.window, name).and_downcast::<gtk::Entry>().ok_or(format!("{name} is not drawn"))
}
fn view(ui: &Ui, name: &str) -> Result<gtk::TextView, String> {
    named(&ui.window, name).and_downcast::<gtk::TextView>().ok_or(format!("{name} is not drawn"))
}
fn typed(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
}
fn has_class(ui: &Ui, class: &str) -> bool {
    find(ui.window.upcast_ref(), &|w| w.has_css_class(class) && w.is_mapped()).is_some()
}

/// A title and a description being typed survive the task moving (here, by another client)
/// and an attachment arriving: before, either rebuilt the part and threw the text away.
async fn task_page_keeps_typing(ui: &Rc<Ui>) -> Result<(), String> {
    let project = ui.project.get();
    let task = call(ui, "task.create", json!({"project_id":project,"title":"Typing survives","body":"Saved body.","column":"ready"})).await?;
    let id = task["id"].as_i64().ok_or("task id")?;
    crate::pages::open_task(ui, id);
    wait_for(|| named(&ui.window, "task-title-edit").is_some_and(|w| w.is_mapped()), "Task page drawn").await?;
    click(&ui.window, "task-title-edit")?;
    entry(ui, "task-title")?.set_text("A title being typed");
    click(&ui.window, "task-body-edit")?;
    view(ui, "task-body")?.buffer().set_text("A description being typed");

    call(ui, "task.move", json!({"task_id":id,"column":"active"})).await?;
    wait_for(|| has_class(ui, "status-pill-active"), "The page follows the move").await?;
    call(ui, "task.attach", json!({"task_id":id,"name":"notes.txt","mime":"text/plain","bytes_b64":"aGVsbG8K"})).await?;
    wait_for(|| has_class(ui, "attachment-tile"), "The attachment shows on the page").await?;
    glib::timeout_future(Duration::from_millis(400)).await;
    require(entry(ui, "task-title")?.text() == "A title being typed", "The title being typed was rebuilt away")?;
    require(typed(&view(ui, "task-body")?) == "A description being typed", "The description being typed was rebuilt away")?;

    // A page with an edit open refuses to close; cancelling both edits catches up and closes.
    let panel = ui.panels.borrow().last().cloned().ok_or("Task page is not a panel")?;
    panel.close();
    require(!ui.panels.borrow().is_empty(), "A page with typed edits closed")?;
    for cancel in ["Cancel", "Cancel"] {
        let key = find(ui.window.upcast_ref(), &|w| {
            w.downcast_ref::<gtk::Button>().is_some_and(|b| b.label().as_deref() == Some(cancel) && b.is_mapped())
        })
        .and_downcast::<gtk::Button>()
        .ok_or("Cancel key")?;
        key.emit_clicked();
    }
    panel.close();
    require(ui.panels.borrow().is_empty(), "The page closes once its edits are cancelled")?;
    println!("Task page keeps a title and description being typed across a move and an attachment");
    Ok(())
}
