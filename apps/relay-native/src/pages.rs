use crate::app::{button, clear, field, label, rows, text, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

#[path = "guardrail_pages.rs"]
mod guardrail_pages;
#[path = "note_pages.rs"]
mod note_pages;
#[path = "notes_window.rs"]
mod notes_window;
#[path = "task_pages.rs"]
mod task_pages;
#[path = "board_view.rs"]
mod board_view;
pub use notes_window::{refresh_notes, show_notes, NotesWindow};
pub use note_pages::{close_all_notes, discard_note, notes_prefs, open_draft, set_notes_prefs, shown_project, unsaved_notes, Prefs as NotesPrefs};
pub use guardrail_pages::{guardrail_event, restore_prompts, settings_editor as guardrail_settings};
#[allow(unused_imports)] // entry points for the workspace and project menus
pub use guardrail_pages::{open_project_guardrails, open_workspace_guardrails};
pub fn verify_note_tools() {
    note_pages::verify_tools();
}
/// Open notes list the tasks made from them; a task event refreshes those lists.
pub fn note_tasks_changed(ui: &Rc<Ui>) {
    note_pages::tasks_changed(ui);
}
pub fn open_note(ui: &Rc<Ui>, note: Value) {
    notes_window::show_project(ui, note["project_id"].as_i64().unwrap_or(ui.project.get()));
    note_pages::edit(ui, note);
}
pub fn open_task(ui: &Rc<Ui>, id: i64) {
    task_pages::open(ui, id);
}
pub fn open_module_board(ui: &Rc<Ui>, module: i64) {
    board_view::open_module(ui, module);
}
pub fn new_module(ui: &Rc<Ui>, project: i64) {
    task_pages::compose_module(ui, project);
}
pub fn new_task(ui: &Rc<Ui>, project: i64, column: &str) {
    task_pages::compose(ui, project, column, None);
}

fn paragraph(value: &str) -> gtk::Label {
    let l = label(value, "body");
    l.set_wrap(true);
    l.set_selectable(true);
    l
}

/// How many mailbox messages the page asks for at first, and the most "Show older" reaches
/// (the engine's own ceiling for one page).
const MAILBOX_PAGE: u32 = 200;
const MAILBOX_PAGE_MAX: u32 = 1000;
thread_local! {
    static MAILBOX_LIMIT: std::cell::Cell<u32> = const { std::cell::Cell::new(MAILBOX_PAGE) };
}

pub async fn refresh(ui: &Rc<Ui>, name: &str, project: i64) {
    let (op, key) = match name {
        "board" => ("task.list", "tasks"),
        "mailbox" => ("mailbox.list", "messages"),
        "guardrails" => ("guardrail.holds.list", "holds"),
        "modules" => ("module.list", "modules"),
        _ => return,
    };
    // A page shown for another project starts again from the newest mailbox page.
    if name == "mailbox" && ui.page_projects.borrow().get(name).copied() != Some(project) {
        MAILBOX_LIMIT.with(|limit| limit.set(MAILBOX_PAGE));
    }
    let payload = match name {
        "modules" => json!({"project_id":project,"include_archived":true}),
        // mailbox.list answers a page, newest messages last; "Show older" widens it.
        "mailbox" => json!({"project_id":project,"limit":MAILBOX_LIMIT.with(|limit| limit.get())}),
        // Cards show no body: a summary keeps hundreds of tasks well under the client's line
        // limit. The board reads a body when it searches, peeks at or copies one.
        "board" => json!({"project_id":project,"summary":true}),
        _ => json!({"project_id":project}),
    };
    // The board labels cards with each row's `module_name`, which task.list joins in itself
    // (completed modules included), so the board needs no module.list of its own.
    let result = ui.call(op, payload).await;
    if ui.project.get() != project || *ui.page.borrow() != name {
        return;
    }
    let (data, older) = match result {
        Ok(v) => (rows(&v, key), v.get("next_before").is_some_and(|before| !before.is_null())),
        Err(e) => {
            ui.show_error(&e.to_string());
            return;
        }
    };
    if name == "board" {
        board_view::show(ui, &ui.pages[name], project, data);
        return;
    }
    let page = &ui.pages[name];
    // Keep forms mounted while events update the list below them.
    let owner = ui.page_projects.borrow().get(name).copied();
    if owner != Some(project) {
        clear(page);
        ui.page_projects.borrow_mut().insert(name.into(), project);
        if name == "modules" {
            page.add_css_class("modules-page");
            // The board's header: its tabs, and New module where the board has New task.
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            head.add_css_class("modules-head");
            head.append(&board_switcher(ui, "modules"));
            let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            gap.set_hexpand(true);
            head.append(&gap);
            let add = button("", "primary");
            add.add_css_class("board-add");
            add.set_valign(gtk::Align::Center);
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            content.append(&crate::icons::image("plus", 12));
            content.append(&label("New module", ""));
            add.set_child(Some(&content));
            let weak = Rc::downgrade(ui);
            add.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    task_pages::compose_module(&ui, project);
                }
            });
            head.append(&add);
            page.append(&head);
        }
        if name != "modules" {
        // A page's title and the line under it, as Tally's, Skills' and Settings' pages have them.
        let heading = label(if name == "mailbox" { "Mailbox" } else { "Guardrails" }, "page-title");
        heading.set_xalign(0.0);
        page.append(&heading);
        let lede = paragraph(if name == "mailbox" {
            "Write to one agent in this project, or to every one with *. Messages between agents show here too."
        } else {
            "Answer agents that need an exception, decide which held actions may proceed, and see the exceptions still in force."
        });
        lede.remove_css_class("body");
        lede.add_css_class("page-lede");
        page.append(&lede);
        if name == "mailbox" {
            mail_composer(ui, page, project);
        }
        }
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_vexpand(true);
        page.append(&body);
    }
    let body = page.last_child().unwrap().downcast::<gtk::Box>().unwrap();
    // Guardrails reconciles its cards instead of rebuilding them (`guardrail_pages::page`).
    if name == "guardrails" {
        guardrail_pages::page(ui, &body, project, data).await;
        return;
    }
    clear(&body);
    // Modules draws its own empty state (`board_view::modules`).
    if data.is_empty() && name != "modules" {
        body.append(&paragraph(match name {
            "mailbox" => "No messages in this project.",
            _ => "No modules yet. A module bundles tasks into one release: choose New module to start one.",
        }));
    }
    match name {
        "mailbox" => {
            // Newest first, as the phone lists them; the engine answers oldest first.
            for message in data.iter().rev() {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
                row.add_css_class("record");
                row.append(&label(
                    &format!(
                        "{} → {}{}",
                        who(text(message, "from")),
                        who(text(message, "to")),
                        if message["priority"].as_bool() == Some(true) {
                            " · PRIORITY"
                        } else {
                            ""
                        }
                    ),
                    "dim",
                ));
                row.append(&paragraph(text(message, "text")));
                // "5 min ago", the moment itself on hover: it showed the engine's nanoseconds.
                let sent = text(message, "sent_at");
                let when = label(&crate::relative::ago(sent, crate::relative::Form::Long).unwrap_or_else(|| sent.to_string()), "dim");
                when.set_tooltip_text(Some(sent));
                row.append(&when);
                body.append(&row);
            }
            // Older messages sit below the oldest one shown.
            let widest = MAILBOX_LIMIT.with(|limit| limit.get()) >= MAILBOX_PAGE_MAX;
            if older && !widest {
                let more = button("Show older messages", "secondary");
                let weak = Rc::downgrade(ui);
                more.connect_clicked(move |_| {
                    MAILBOX_LIMIT.with(|limit| limit.set((limit.get() + MAILBOX_PAGE).min(MAILBOX_PAGE_MAX)));
                    if let Some(ui) = weak.upgrade() {
                        ui.refresh_page();
                    }
                });
                body.append(&more);
            } else if older {
                body.append(&label(&format!("Showing the newest {MAILBOX_PAGE_MAX} messages."), "dim"));
            }
        }
        "modules" => board_view::modules(ui, &body, &data),
        _ => {}
    }
}
/// A mail address as the page shows it: your own messages are "You", a broadcast "everyone".
fn who(name: &str) -> &str {
    match name {
        "user" => "You",
        "*" => "everyone",
        name => name,
    }
}

/// What `mailbox.send`'s `delivery` means for the person who sent it to `recipients` agents;
/// an error when nobody got it.
fn delivery(value: &str, recipients: usize) -> Result<&'static str, &'static str> {
    match value {
        "session_parked" if recipients > 1 => Ok("Sent · the agents are parked and read it when they wake"),
        "session_parked" => Ok("Sent · the agent is parked and reads it when it wakes"),
        "not_running" => Ok("Sent · no recipient is running yet"),
        "no_recipients" => Err("No agent to deliver to"),
        _ => Ok("Sent"),
    }
}

pub fn workspace_picker(ui: &Rc<Ui>, project: i64, destination: &str) -> gtk::MenuButton {
    let picker = gtk::MenuButton::new();
    picker.add_css_class("workspace-picker");
    picker.set_valign(gtk::Align::Center);
    let projects = ui.projects.borrow();
    let spaces = ui.workspaces.borrow();
    let current = projects.iter().find(|p| p["id"].as_i64() == Some(project));
    let workspace = current.and_then(|p| spaces.iter().find(|w| w["id"] == p["workspace_id"]));
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    let caption = label(
        &format!(
            "{} / {}",
            workspace.map(|w| text(w, "name")).unwrap_or("Workspace"),
            current.map(|p| text(p, "name")).unwrap_or("Project")
        ),
        "workspace-picker-label",
    );
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    caption.set_max_width_chars(30);
    content.append(&caption);
    content.append(&crate::icons::image("chevron-down", 12));
    picker.set_child(Some(&content));
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_size_request(240, -1);
    for space in spaces.iter() {
        let owned: Vec<_> = projects
            .iter()
            .filter(|p| p["workspace_id"] == space["id"])
            .collect();
        if owned.is_empty() {
            continue;
        }
        menu.append(&label(text(space, "name"), "section-label"));
        for item in owned {
            let key = button(text(item, "name"), "workspace-project");
            let id = item["id"].as_i64().unwrap_or(0);
            if id == project {
                key.add_css_class("selected");
            }
            let weak = Rc::downgrade(ui);
            let pop = popover.downgrade();
            let destination = destination.to_string();
            key.connect_clicked(move |_| {
                if let Some(pop) = pop.upgrade() {
                    pop.popdown();
                }
                if let Some(ui) = weak.upgrade() {
                    if destination == "notes" {
                        notes_window::show_project(&ui, id);
                    } else if ui.project.get() != id {
                        ui.open_project(id, &destination);
                    }
                }
            });
            menu.append(&key);
        }
    }
    let scroll = crate::app::scrolled(&menu);
    scroll.set_max_content_height(480);
    scroll.set_propagate_natural_height(true);
    popover.set_child(Some(&scroll));
    picker.set_popover(Some(&popover));
    picker
}

pub fn task_mark(kind: &str, size: i32) -> gtk::DrawingArea {
    let mark = gtk::DrawingArea::new();
    mark.set_content_width(size);
    mark.set_content_height(size);
    mark.set_valign(gtk::Align::Center);
    let kind = kind.to_string();
    mark.set_draw_func(move |widget, cr, width, height| {
        let Some(color) = widget.style_context().lookup_color("secondary") else {
            return;
        };
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            color.alpha() as f64,
        );
        let w = width as f64;
        let h = height as f64;
        cr.set_line_width(1.);
        match kind.as_str() {
            "feature" => {
                cr.rectangle(0., 0., w, h);
                let _ = cr.fill();
            }
            "chore" => {
                for y in [1., 4., 7.] {
                    cr.rectangle(0., y, w, 1.);
                }
                let _ = cr.fill();
            }
            "spike" => {
                let _ = cr.save();
                cr.translate(w / 2., h / 2.);
                cr.rotate(std::f64::consts::FRAC_PI_4);
                cr.rectangle(-w / 2. + 1.5, -h / 2. + 1.5, w - 3., h - 3.);
                let _ = cr.stroke();
                let _ = cr.restore();
            }
            _ => {
                cr.rectangle(0.5, 0.5, w - 1., h - 1.);
                let _ = cr.stroke();
                if kind == "bug" {
                    cr.rectangle(3., 3., w - 6., h - 6.);
                    let _ = cr.fill();
                }
            }
        }
    });
    mark
}

fn board_switcher(ui: &Rc<Ui>, selected: &str) -> gtk::Box {
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    tabs.add_css_class("board-switcher");
    tabs.set_valign(gtk::Align::Center);
    for (name, caption) in [("board", "Board"), ("modules", "Modules")] {
        let key = button(caption, "quiet");
        if name == selected {
            key.add_css_class("selected");
        }
        let weak = Rc::downgrade(ui);
        key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                // Either tab leaves a module's board for the whole project.
                board_view::leave_module();
                if name == "board" && ui.page.borrow().as_str() == "board" {
                    ui.refresh_page();
                } else {
                    ui.navigate(name);
                }
            }
        });
        tabs.append(&key);
    }
    tabs
}

fn widgets(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut result = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        result.extend(widgets(&widget));
        result.push(widget);
    }
    result
}

fn mail_composer(ui: &Rc<Ui>, page: &gtk::Box, project: i64) {
    let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let to = gtk::Entry::builder()
        .placeholder_text("Session name, or * for everyone")
        .build();
    let message = gtk::TextView::new();
    message.set_wrap_mode(gtk::WrapMode::WordChar);
    message.set_size_request(-1, 90);
    // Who there is to write to, a click away: everyone, or one of the project's agents.
    let to_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    to.set_hexpand(true);
    to_row.append(&to);
    let pick = gtk::MenuButton::new();
    pick.set_child(Some(&crate::icons::image("chevron-down", 12)));
    pick.set_tooltip_text(Some("Pick who to write to"));
    pick.add_css_class("mail-pick");
    let choices = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let popover = gtk::Popover::new();
    popover.set_child(Some(&choices));
    pick.set_popover(Some(&popover));
    // Listed as the popup opens, so an agent started since the page was built is there too.
    let weak = Rc::downgrade(ui);
    let (entry, shown) = (to.clone(), popover.downgrade());
    pick.set_create_popup_func(move |_| {
        let Some(ui) = weak.upgrade() else { return };
        clear(&choices);
        let mut names = vec![(String::from("*"), String::from("Everyone"))];
        names.extend(
            ui.sessions
                .borrow()
                .iter()
                .filter(|s| s["project_id"].as_i64() == Some(project) && text(s, "state") != "closed")
                .map(|s| (text(s, "name").to_string(), format!("{} · {} {}", text(s, "name"), text(s, "provider"), text(s, "role")))),
        );
        for (name, caption) in names {
            let choice = button(&caption, "nav");
            choice.set_halign(gtk::Align::Fill);
            let (to, popover) = (entry.clone(), shown.clone());
            choice.connect_clicked(move |_| {
                to.set_text(&name);
                if let Some(popover) = popover.upgrade() {
                    popover.popdown();
                }
            });
            choices.append(&choice);
        }
    });
    to_row.append(&pick);
    field("To", &to_row, &form);
    field("Message", &message, &form);
    let priority =
        gtk::CheckButton::with_label("Priority: ask this agent to read it at the next safe point");
    form.append(&priority);
    let send = button("Send message", "primary");
    form.append(&send);
    page.append(&form);
    let weak = Rc::downgrade(ui);
    send.connect_clicked(move |b| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let recipient = to.text().trim().to_string();
        let buffer = message.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if recipient.is_empty() || text.trim().is_empty() {
            ui.show_error("Enter a recipient and a message.");
            return;
        }
        let priority_control = priority.clone();
        let recipient_control = to.clone();
        let priority = priority.is_active();
        if priority && recipient == "*" {
            ui.show_error("Priority mail needs one session name.");
            return;
        }
        b.set_sensitive(false);
        let b = b.clone();
        glib::spawn_future_local(async move {
            match ui
                .call(
                    "mailbox.send",
                    json!({"project_id":project,"to":recipient,"text":text,"priority":priority}),
                )
                .await
            {
                Ok(v) => {
                    if buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), false)
                        .as_str()
                        == text
                        && recipient_control.text().trim() == recipient
                        && priority_control.is_active() == priority
                    {
                        buffer.set_text("");
                        // As on the phone: the next message is ordinary unless asked again.
                        priority_control.set_active(false);
                    }
                    match delivery(crate::app::text(&v, "delivery"), rows(&v, "recipients").len()) {
                        Ok(info) => ui.show_info(info),
                        Err(error) => ui.show_error(error),
                    }
                    ui.refresh_page();
                }
                Err(e) => ui.show_error(&e.to_string()),
            }
            b.set_sensitive(true);
        });
    });
}
