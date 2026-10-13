//! Native lifecycle regressions. Invoked only by the isolated fixture harness.
use crate::app::Ui;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::json;
use super::util::{click, find, named, require, wait_for};
use std::{rc::Rc, time::Duration};

pub async fn run(ui: &Rc<Ui>) -> Result<(), String> {
    let project = ui.project.get();
    require(project != 0, "Fixture project must be ready")?;
    let note = ui.call("notes.create",json!({"project_id":project,"title":"Plan","body":"Ordinary Plan fixture","pinned":true})).await.map_err(|e|e.to_string())?;
    let id = note["id"].as_i64().ok_or("Missing fixture note id")?;
    crate::pages::open_note(ui, note.clone());
    let owner = ui
        .notes_window
        .borrow()
        .as_ref()
        .ok_or("Notes window missing")?
        .clone();
    wait_for(
        || {
            crate::pages::shown_project() == project
                && named(&owner.window, "notes-library-split").is_some()
        },
        "Notes library loaded",
    )
    .await?;
    // The first load restores the tabs an earlier run left open (a full smoke run's notes page
    // leaves its own note active), and that may take the tab from Plan. Open it again, as a
    // person would from the library, so the checks below look at Plan.
    crate::pages::open_note(ui, note);
    require(
        owner.window.is_decorated()
            && owner.window.is_resizable()
            && owner
                .window
                .titlebar()
                .is_some_and(|bar| bar.is::<gtk::WindowHandle>()),
        "Notes must retain native resize edges around its custom titlebar",
    )?;
    wait_for(
        || owner.window.is_mapped() && owner.window.width() > 120 && owner.window.height() > 80,
        "Notes window allocated",
    )
    .await?;
    let initial_size = (owner.window.width(), owner.window.height());
    println!("NOTES_RESIZE_INITIAL={}x{}", initial_size.0, initial_size.1);
    owner
        .window
        .set_default_size(initial_size.0 - 120, initial_size.1 - 80);
    wait_for(
        || owner.window.width() < initial_size.0 && owner.window.height() < initial_size.1,
        "Notes window shrank",
    )
    .await?;
    owner
        .window
        .set_default_size(initial_size.0, initial_size.1);
    wait_for(
        || owner.window.width() == initial_size.0 && owner.window.height() == initial_size.1,
        "Notes window grew back",
    )
    .await?;
    let draft = crate::pages::open_draft(id).ok_or("Fixture draft missing")?;
    let title = named(&draft.layout, "note-title")
        .ok_or("Plan must have a title editor")?
        .downcast::<gtk::Entry>()
        .map_err(|_| "Note title type")?;
    require(
        title.is_editable(),
        "An existing Plan document must remain an ordinary editable note",
    )?;
    let body = named(&draft.layout, "note-body")
        .ok_or("Note body missing")?
        .downcast::<gtk::TextView>()
        .map_err(|_| "Note body type")?;
    let source = body
        .clone()
        .downcast::<sourceview5::View>()
        .map_err(|_| "Note body must be a GtkSourceView")?;
    require(
        source
            .buffer()
            .downcast::<sourceview5::Buffer>()
            .ok()
            .and_then(|buffer| sourceview5::prelude::BufferExt::language(&buffer))
            .is_some_and(|language| language.id() == "markdown"),
        "Note body must use the Markdown language",
    )?;
    owner
        .window
        .activate_action("notes.find", None)
        .map_err(|_| "Find action missing")?;
    let query = named(&draft.layout, "notes-find-entry")
        .ok_or("Find bar missing")?
        .downcast::<gtk::Entry>()
        .map_err(|_| "Find entry type")?;
    require(query.is_mapped(), "Ctrl+F did not show the find bar")?;
    query.set_text("plan");
    let count = named(&draft.layout, "notes-find-count")
        .ok_or("Find count missing")?
        .downcast::<gtk::Label>()
        .map_err(|_| "Find count type")?;
    wait_for(|| count.text() == "1 of 1", "Find shows a match count").await?;
    owner
        .window
        .activate_action("notes.close-find", None)
        .map_err(|_| "Close-find action missing")?;
    require(!query.is_mapped(), "Escape did not close the find bar")?;
    owner
        .window
        .activate_action("notes.goto", None)
        .map_err(|_| "Go-to-line action missing")?;
    let goto = named(&draft.layout, "notes-goto-entry")
        .ok_or("Go-to-line field missing")?
        .downcast::<gtk::Entry>()
        .map_err(|_| "Go-to-line field type")?;
    goto.set_text("1:10");
    goto.emit_activate();
    require(
        body.buffer().cursor_position() == 9,
        "Go to line 1:10 did not move the cursor",
    )?;
    body.buffer()
        .set_text("Unsaved text survives the window closing.");
    owner.window.close();
    require(
        !owner.window.is_visible(),
        "Close must hide Notes even with a dirty draft",
    )?;
    require(
        crate::pages::open_draft(id).is_some(),
        "Window close discarded a draft",
    )?;
    // Prove a negative: wait past the autosave debounce (1.5 s, note_pages/doc.rs), so a
    // Plan that did autosave would already show it here (RA-721).
    glib::timeout_future(Duration::from_millis(2000)).await;
    let stored = ui
        .call("notes.get", json!({"note_id":id}))
        .await
        .map_err(|e| e.to_string())?;
    require(
        stored["body"] == "Ordinary Plan fixture",
        "Plan was silently autosaved",
    )?;
    crate::pages::show_notes(ui);
    require(
        Rc::ptr_eq(&owner, ui.notes_window.borrow().as_ref().unwrap()),
        "Reopen created a second Notes owner",
    )?;
    require(owner.window.is_visible(), "Reopen did not present Notes")?;
    require(
        body.buffer()
            .text(
                &body.buffer().start_iter(),
                &body.buffer().end_iter(),
                false,
            )
            .as_str()
            == "Unsaved text survives the window closing.",
        "Reopen replaced dirty editor contents",
    )?;
    let split = named(&owner.window, "notes-library-split")
        .unwrap()
        .downcast::<gtk::Paned>()
        .map_err(|_| "Library split type")?;
    split.set_position(331);
    click(&owner.window, "notes-library-toggle")?;
    require(owner.rail_collapsed.get(), "Library did not collapse")?;
    let renders = owner.renders.get();
    crate::pages::refresh_notes(ui);
    wait_for(|| owner.renders.get() > renders, "Notes refresh completed").await?;
    require(
        named(&owner.window, "notes-library-split")
            .is_some_and(|same| same == split.clone().upcast::<gtk::Widget>()),
        "Refresh rebuilt the Notes shell instead of keeping it",
    )?;
    require(
        owner.rail_collapsed.get(),
        "Refresh reset collapsed library",
    )?;
    require(owner.rail_width.get() == 331, "Refresh reset library width")?;
    owner.window.close();
    crate::pages::show_notes(ui);
    require(
        owner.rail_collapsed.get() && owner.rail_width.get() == 331,
        "Reopen reset rail state",
    )?;
    // The rail state is written after a debounce; read until it lands.
    let deadline = std::time::Instant::now() + super::util::WAIT;
    loop {
        let settings = ui
            .call("settings.get", json!({"path":"native.notes.window"}))
            .await
            .map_err(|e| e.to_string())?;
        if settings["value"]["rail_width"] == 331 && settings["value"]["rail_collapsed"] == true {
            break;
        }
        require(std::time::Instant::now() < deadline, "Rail state was not persisted")?;
        glib::timeout_future(Duration::from_millis(50)).await;
    }
    body.buffer().set_text("Ordinary Plan fixture");
    draft.close();
    require(
        crate::pages::open_draft(id).is_none(),
        "Clean tab close retained stale draft",
    )?;
    let renders = owner.renders.get();
    crate::pages::refresh_notes(ui);
    wait_for(|| owner.renders.get() > renders, "Notes refresh after tab close").await?;
    require(
        crate::pages::open_draft(id).is_none(),
        "Refresh reopened a deliberately closed tab",
    )?;
    features(ui, project, &owner).await?;
    let task = ui
        .call(
            "task.create",
            json!({"project_id":project,"title":"Task message close guard"}),
        )
        .await
        .map_err(|e| e.to_string())?;
    let task_id = task["id"].as_i64().ok_or("Task id missing")?;
    ui.navigate("board");
    crate::pages::open_task(ui, task_id);
    wait_for(
        || named(&ui.window, "task-message").is_some(),
        "Task full-page details loaded",
    )
    .await?;
    let panel = ui
        .panels
        .borrow()
        .last()
        .ok_or("Task did not open a page")?
        .clone();
    let message = named(&ui.window, "task-message")
        .unwrap()
        .downcast::<gtk::TextView>()
        .map_err(|_| "Task message type")?;
    message
        .buffer()
        .set_text("Unsent fixture message. Do not transmit.");
    panel.close();
    require(
        ui.panels
            .borrow()
            .iter()
            .any(|candidate| Rc::ptr_eq(candidate, &panel)),
        "Closing task lost unsent message text",
    )?;
    message.buffer().set_text("");
    panel.close();
    require(
        !ui.panels
            .borrow()
            .iter()
            .any(|candidate| Rc::ptr_eq(candidate, &panel)),
        "Cleared message kept task close blocked",
    )?;
    ui.call("task.delete", json!({"task_id":task_id}))
        .await
        .map_err(|e| e.to_string())?;
    ui.call("notes.delete", json!({"note_id":id}))
        .await
        .map_err(|e| e.to_string())?;
    println!("NOTES_TASK_LIFECYCLE_OK: custom titlebar, resize, retained window, sourceview markdown, find count, go to line, dirty reopen, persistent shell, rail persistence, ordinary Plan, task message guard");
    Ok(())
}

/// Checkboxes and pictures over the text, ticking from the box, the toolbar's names and
/// hidden keys, and Create task from note.
async fn features(ui: &Rc<Ui>, project: i64, owner: &Rc<crate::pages::NotesWindow>) -> Result<(), String> {
    let folder = std::env::temp_dir().join(format!("relay-notes-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let image = folder.join("swatch.png");
    let pixels: Vec<u8> = (0..120 * 80).flat_map(|i| [40, (i % 120 * 2) as u8, 200, 255]).collect();
    gtk::gdk::MemoryTexture::new(120, 80, gtk::gdk::MemoryFormat::R8g8b8a8, &glib::Bytes::from_owned(pixels), 120 * 4)
        .save_to_png(&image)
        .map_err(|e| e.to_string())?;
    let body = format!("# Groceries\n- [ ] milk\n- [x] eggs\n\n![Image 1]({})\n\nAfter the picture.", image.display());
    let note = ui
        .call("notes.create", json!({"project_id":project,"title":"Checklist","body":body}))
        .await
        .map_err(|e| e.to_string())?;
    let id = note["id"].as_i64().ok_or("Missing checklist note id")?;
    crate::pages::open_note(ui, note);
    let draft = crate::pages::open_draft(id).ok_or("Checklist draft missing")?;
    let layout: gtk::Widget = draft.layout.clone().upcast();
    let boxes = || {
        let mut found = Vec::new();
        collect(&layout, &|w| w.has_css_class("notes-check") && w.is_visible(), &mut found);
        found
    };
    wait_for(|| boxes().len() == 2, "Two checklist boxes drawn").await?;
    let picture = || find(&layout, &|w| w.has_css_class("notes-image") && w.is_mapped());
    wait_for(|| picture().is_some_and(|p| p.width() == 120 && p.height() >= 80), "Picture drawn at its size").await?;
    let first = boxes()[0].clone().downcast::<gtk::CheckButton>().map_err(|_| "Checkbox type")?;
    require(!first.is_active(), "milk starts unticked")?;
    first.set_active(true);
    let body = named(&draft.layout, "note-body").ok_or("Note body missing")?.downcast::<gtk::TextView>().map_err(|_| "Body type")?;
    let text = || {
        let buffer = body.buffer();
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
    };
    wait_for(|| text().contains("- [x] milk"), "Ticking the box ticks the Markdown").await?;
    // Ctrl+Enter's action on the line under the cursor unticks it again.
    let buffer = body.buffer();
    buffer.place_cursor(&buffer.iter_at_line(1).ok_or("Line 2")?);
    owner.window.activate_action("notes.toggle-check", None).map_err(|_| "toggle-check action missing")?;
    require(text().contains("- [ ] milk"), "Toggle check did not untick the line")?;
    // Names under the toolbar's icons (formatting keys keep to theirs), and a hidden key.
    let mut prefs = crate::pages::notes_prefs();
    prefs.toolbar_labels = true;
    prefs.toolbar_hidden = 0b111 << 4;
    crate::pages::set_notes_prefs(ui, prefs);
    let root: gtk::Widget = owner.window.clone().upcast();
    let caption = |name: &str| {
        let name = name.to_string();
        find(&root, &move |w| w.downcast_ref::<gtk::Label>().is_some_and(|l| l.has_css_class("notes-tool-caption") && l.text() == name))
    };
    require(caption("Undo").is_some_and(|l| l.is_mapped()), "Toolbar labels did not show")?;
    require(caption("Bold").is_some_and(|l| !l.is_mapped()), "A formatting key shows its name")?;
    require(caption("Paste").is_some_and(|l| !l.is_mapped()), "Hidden Paste key still shows")?;
    if owner.rail_collapsed.get() {
        owner.window.activate_action("notes.sidebar", None).map_err(|_| "sidebar action missing")?;
    }
    glib::timeout_future(Duration::from_millis(250)).await;
    let capture = |suffix: &str| -> Result<(), String> {
        let Ok(path) = std::env::var("RELAY_NATIVE_SCREENSHOT") else { return Ok(()) };
        let path = path.replace(".png", &format!("-{suffix}.png"));
        let paintable = gtk::WidgetPaintable::new(Some(&owner.window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, owner.window.width() as f64, owner.window.height() as f64);
        let node = snapshot.to_node().ok_or("Notes snapshot")?;
        owner.window.renderer().ok_or("Notes renderer")?.render_texture(&node, None).save_to_png(&path).map_err(|e| e.to_string())?;
        println!("NOTES_{}_SCREENSHOT={path}", suffix.to_uppercase());
        Ok(())
    };
    capture("features")?;
    // Right-clicking the toolbar (or View > Customize toolbar) opens its settings.
    owner.window.activate_action("notes.customize-toolbar", None).map_err(|_| "customize action missing")?;
    let customize = find(&root, &|w| w.has_css_class("notes-customize")).ok_or("Toolbar settings missing")?;
    wait_for(|| customize.is_mapped(), "Toolbar settings shown").await?;
    customize.downcast_ref::<gtk::Popover>().ok_or("Toolbar settings type")?.popdown();
    crate::pages::set_notes_prefs(ui, crate::pages::NotesPrefs::default());
    // The whole note becomes a Backlog task.
    owner.window.activate_action("notes.to-task", None).map_err(|_| "to-task action missing")?;
    let mut created = None;
    let deadline = std::time::Instant::now() + super::util::WAIT;
    while created.is_none() {
        let tasks = ui.call("task.list", json!({"project_id":project})).await.map_err(|e| e.to_string())?;
        created = crate::app::rows(&tasks, "tasks").into_iter().find(|t| t["title"] == "Checklist");
        require(std::time::Instant::now() < deadline, "Create task from note made no task")?;
        glib::timeout_future(Duration::from_millis(50)).await;
    }
    let task = created.unwrap();
    require(task["body"].as_str().is_some_and(|b| b.contains("- [ ] milk") && b.contains("From the note")), "Task body is not the note")?;
    require(task["note_id"].as_i64() == Some(id), "The task does not remember its note")?;
    // The note lists the task it became, under its title.
    let chip = || find(&root, &|w| w.has_css_class("notes-task-chip"));
    wait_for(|| chip().is_some_and(|c| c.is_mapped()), "The note lists its task").await?;
    glib::timeout_future(Duration::from_millis(150)).await;
    capture("task")?;
    ui.call("task.delete", json!({"task_id":task["id"]})).await.map_err(|e| e.to_string())?;
    crate::pages::discard_note(ui, id);
    ui.call("notes.delete", json!({"note_id":id})).await.map_err(|e| e.to_string())?;
    std::fs::remove_dir_all(&folder).ok();
    println!("NOTES_FEATURES_OK: checklist boxes, tick from box and Ctrl+Enter, picture overlay, toolbar labels and hidden keys, note to task and back");
    Ok(())
}

fn collect(root: &gtk::Widget, test: &impl Fn(&gtk::Widget) -> bool, found: &mut Vec<gtk::Widget>) {
    if test(root) {
        found.push(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        collect(&widget, test, found);
        child = widget.next_sibling();
    }
}
