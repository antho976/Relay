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
    note_pictures_move(ui).await?;
    refused_close_keeps_its_place(ui).await?;
    board_scope_follows_the_project(ui).await?;
    palette_is_freed(ui).await?;
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

fn controller<T: IsA<gtk::EventController> + IsA<glib::Object>>(widget: &impl IsA<gtk::Widget>, test: impl Fn(&T) -> bool) -> Option<T> {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items()).filter_map(|i| controllers.item(i)).filter_map(|c| c.downcast::<T>().ok()).find(|c| test(c))
}

/// Pictures in a note move by drag between lines: upward, downward, and onto themselves
/// (nothing changes). Before, the drop went in as text at the character under the pointer,
/// an upward move deleted another picture, and a drop on itself cut the reference in half.
async fn note_pictures_move(ui: &Rc<Ui>) -> Result<(), String> {
    let project = ui.project.get();
    let folder = std::env::temp_dir().join(format!("relay-devspace-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut refs = Vec::new();
    for (name, shade) in [("a.png", 40u8), ("b.png", 200u8)] {
        let path = folder.join(name);
        let pixels: Vec<u8> = (0..160 * 90).flat_map(|_| [shade, 90, 255 - shade, 255]).collect();
        gtk::gdk::MemoryTexture::new(160, 90, gtk::gdk::MemoryFormat::R8g8b8a8, &glib::Bytes::from_owned(pixels), 160 * 4)
            .save_to_png(&path)
            .map_err(|e| e.to_string())?;
        refs.push(format!("![Image {}]({})", refs.len() + 1, path.display()));
    }
    let (a, b) = (refs[0].clone(), refs[1].clone());
    let body = format!("First line of prose\n{a}\nMiddle line of prose\n{b}\nLast line of prose");
    let note = call(ui, "notes.create", json!({"project_id":project,"title":"Pictures move","body":body})).await?;
    let id = note["id"].as_i64().ok_or("note id")?;
    crate::pages::open_note(ui, note);
    let draft = crate::pages::open_draft(id).ok_or("Pictures draft missing")?;
    let layout: gtk::Widget = draft.layout.clone().upcast();
    let view = find(&layout, &|w| w.is::<gtk::TextView>() && w.has_css_class("view") && w.is_mapped())
        .and_downcast::<gtk::TextView>()
        .ok_or("Note text view")?;
    let text = || {
        let buffer = view.buffer();
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true).to_string()
    };
    wait_for(|| text() == body, "Note loaded").await?;
    // The pictures, top to bottom.
    let pictures = || {
        fn collect(widget: &gtk::Widget, found: &mut Vec<gtk::Widget>) {
            if widget.has_css_class("notes-image") && widget.is_mapped() && widget.height() > 0 {
                found.push(widget.clone());
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                collect(&next, found);
                child = next.next_sibling();
            }
        }
        let mut found = Vec::new();
        collect(view.upcast_ref(), &mut found);
        found.sort_by_key(|w| w.compute_point(&view, &gtk::graphene::Point::zero()).map(|p| p.y() as i32).unwrap_or(0));
        found
    };
    wait_for(|| pictures().len() == 2, "Two pictures drawn").await?;
    // Where a point on line `line` is, in the view's own coordinates: its upper part, or the
    // lower part of its range.
    let at_line = |line: i32, lower: bool| -> Result<(f64, f64), String> {
        let iter = view.buffer().iter_at_line(line).ok_or("no such line")?;
        let (top, height) = view.line_yrange(&iter);
        let y = if lower { top + height - 2 } else { top + 2 };
        let (x, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, 8, y);
        Ok((f64::from(x), f64::from(y)))
    };
    let drag = |picture: &gtk::Widget, (x, y): (f64, f64)| -> Result<bool, String> {
        let source = controller::<gtk::DragSource>(picture, |_| true).ok_or("picture has no drag source")?;
        let content = source
            .emit_by_name::<Option<gtk::gdk::ContentProvider>>("prepare", &[&4.0_f64, &4.0_f64])
            .ok_or("the picture refused to start a drag")?;
        let formats = content.formats();
        let kind = formats.types().first().copied().ok_or("drag offers no type")?;
        require(!formats.contains_type(glib::GString::static_type()), "a picture drag must not offer text a text view would take")?;
        let value = content.value(kind).map_err(|e| e.to_string())?;
        let target = controller::<gtk::DropTarget>(&view, |t| t.actions() == gtk::gdk::DragAction::MOVE).ok_or("note has no picture drop")?;
        Ok(target.emit_by_name::<bool>("drop", &[&glib::BoxedValue(value), &x, &y]))
    };
    let settle = || glib::timeout_future(Duration::from_millis(150));

    // B (line 3) upward, onto the first line's upper half: it goes above everything, and A stays.
    require(drag(&pictures()[1], at_line(0, false)?)?, "upward drop refused")?;
    settle().await;
    let expected = format!("{b}\nFirst line of prose\n{a}\nMiddle line of prose\nLast line of prose");
    require(text() == expected, &format!("upward move: {:?}", text()))?;
    wait_for(|| pictures().len() == 2, "Both pictures still drawn").await?;
    // A dropped on itself: nothing changes.
    let own = pictures()[1].clone();
    let centre = own.compute_point(&view, &gtk::graphene::Point::new(own.width() as f32 / 2., own.height() as f32 / 2.)).ok_or("picture point")?;
    require(drag(&own, (f64::from(centre.x()), f64::from(centre.y())))?, "drop on itself refused")?;
    settle().await;
    require(text() == expected, &format!("drop on itself: {:?}", text()))?;
    // A (line 2) downward, onto the lower half of the middle line: between it and the last.
    require(drag(&pictures()[1], at_line(3, true)?)?, "downward drop refused")?;
    settle().await;
    let expected = format!("{b}\nFirst line of prose\nMiddle line of prose\n{a}\nLast line of prose");
    require(text() == expected, &format!("downward move: {:?}", text()))?;
    // One undo puts the move back whole.
    view.buffer().undo();
    settle().await;
    let before = format!("{b}\nFirst line of prose\n{a}\nMiddle line of prose\nLast line of prose");
    require(text() == before, &format!("undo of a move: {:?}", text()))?;
    // Saved, so the run closes clean; what is stored is the moved text, not a damaged one.
    let window = view.root().and_downcast::<gtk::Window>().ok_or("Notes window")?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        window.activate_action("notes.save", None).map_err(|_| "Notes window has no save action")?;
        glib::timeout_future(Duration::from_millis(100)).await;
        if call(ui, "notes.get", json!({"note_id":id})).await?["body"] == before.as_str() {
            break;
        }
        require(std::time::Instant::now() < deadline, "The moved note never saved")?;
    }
    println!("Note pictures move between lines: up, down, onto themselves, and back with one undo");
    Ok(())
}

/// A close the engine refuses (here a reviewer whose builder still works in the worktree a
/// removal would delete) puts the pane back where it was: the same pane with its scrollback,
/// focus included. Before, it came back at the end of the wall, a new pane built from the
/// engine's tail.
async fn refused_close_keeps_its_place(ui: &Rc<Ui>) -> Result<(), String> {
    ui.navigate("agents");
    wait_for(|| ui.smoke_wall().0.len() >= 3, "Fixture agents on the wall").await?;
    let (before, _) = ui.smoke_wall();
    let (name, pane) = before[1].clone();
    ui.smoke_close(&name);
    require(!ui.smoke_wall().0.iter().any(|(n, _)| *n == name), "The pane leaves on the click")?;
    crate::smoke::util::wait_within(Duration::from_secs(30), || ui.notice.text().contains("Could not close"), "The close is refused").await?;
    wait_for(|| ui.smoke_wall().0.iter().any(|(n, _)| *n == name), "The pane comes back").await?;
    glib::timeout_future(Duration::from_millis(500)).await;
    let (after, _) = ui.smoke_wall();
    require(after.iter().position(|(n, _)| *n == name) == Some(1), &format!("back in its place: {after:?}"))?;
    require(after.iter().find(|(n, _)| *n == name).map(|(_, p)| *p) == Some(pane), "the same pane, not one rebuilt from the tail")?;
    println!("A refused close puts the same pane back in its place");
    Ok(())
}

fn shows_text(ui: &Ui, wanted: &str) -> bool {
    find(ui.window.upcast_ref(), &|w| w.is_mapped() && w.downcast_ref::<gtk::Label>().is_some_and(|l| l.text() == wanted)).is_some()
}

/// A project switched to from a module's board shows that project's whole board. Before, the
/// old module's scope stayed: every lane empty, the counts 0, until another event came.
async fn board_scope_follows_the_project(ui: &Rc<Ui>) -> Result<(), String> {
    let project = ui.project.get();
    let workspace = ui.workspaces.borrow().first().cloned().ok_or("Fixture workspace")?;
    let root = std::path::PathBuf::from(workspace["path"].as_str().ok_or("workspace path")?).join("devspace-board-scope");
    require(root.starts_with(std::env::temp_dir()), "Fixture repository must be temporary")?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    for args in [vec!["init", "-q", "-b", "main"], vec!["-c", "user.name=Fixture", "-c", "user.email=fixture@relay.test", "commit", "--allow-empty", "-qm", "Fixture"]] {
        let done = std::process::Command::new("git").arg("-C").arg(&root).args(args).output().map_err(|e| e.to_string())?;
        require(done.status.success(), "fixture git")?;
    }
    let other = call(ui, "project.add", json!({"workspace_id":workspace["id"],"path":root,"name":"Board scope fixture"})).await?;
    let other = other["id"].as_i64().ok_or("project id")?;
    call(ui, "task.create", json!({"project_id":other,"title":"A task of the other project","column":"ready"})).await?;
    let module = call(ui, "module.create", json!({"project_id":project,"name":"Scope fixture module","priority":"medium"})).await?;
    let module = module["id"].as_i64().ok_or("module id")?;
    call(ui, "task.create", json!({"project_id":project,"title":"A task in the module","column":"ready","module_id":module})).await?;
    ui.refresh();
    wait_for(|| ui.projects.borrow().iter().any(|p| p["id"].as_i64() == Some(other)), "second project listed").await?;
    ui.navigate("board");
    crate::pages::open_module_board(ui, module);
    wait_for(|| shows_text(ui, "Scope fixture module") && shows_text(ui, "A task in the module"), "The module's board").await?;
    // The creates' events have all landed: the switch is the only thing that redraws.
    glib::timeout_future(Duration::from_millis(1000)).await;
    ui.open_project(other, "board");
    wait_for(|| shows_text(ui, "A task of the other project"), "The other project's board, all of it").await?;
    require(!shows_text(ui, "Scope fixture module"), "The old module's banner is gone")?;
    ui.open_project(project, "agents");
    wait_for(|| ui.project.get() == project, "back on the fixture project").await?;
    println!("Switching project from a module's board shows the new project's whole board");
    Ok(())
}

/// A command palette closed is gone, list and rows: before, the list's own filter, header
/// and activate handlers held the palette that holds the list, so every Ctrl+K kept its rows.
async fn palette_is_freed(ui: &Rc<Ui>) -> Result<(), String> {
    ui.navigate("agents");
    let mut freed = Vec::new();
    for _ in 0..3 {
        click(&ui.window, "command-palette")?;
        wait_for(|| find(ui.window.upcast_ref(), &|w| w.has_css_class("palette-list") && w.is_mapped()).is_some(), "palette open").await?;
        let list = find(ui.window.upcast_ref(), &|w| w.has_css_class("palette-list")).ok_or("palette list")?;
        freed.push(list.downgrade());
        drop(list);
        let panel = ui.panels.borrow().last().cloned().ok_or("palette is not a panel")?;
        panel.close();
        drop(panel);
        wait_for(|| ui.panels.borrow().is_empty(), "palette closed").await?;
    }
    wait_for(|| freed.iter().all(|list| list.upgrade().is_none()), "every closed palette's list freed").await?;
    println!("A closed command palette frees its list");
    Ok(())
}
