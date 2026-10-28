//! The bell's popover. Notifications are read, cleared and followed in place, under the bell,
//! without replacing the page the person is working on.
use super::session_context::relative_time;
use super::{button, clear, icon_button, label, rows, text, Ui};
use crate::tools::plugins::explain_error as refusal;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

struct Center {
    ui: Weak<Ui>,
    popover: gtk::Popover,
    list: gtk::Box,
    summary: gtk::Label,
    mark_all: gtk::Button,
    /// A notify.list is in flight, and whether a change since means it must run once more.
    loading: Cell<bool>,
    stale: Cell<bool>,
    /// When the popover last closed. A press on the bell that closes an open popover reaches
    /// the bell too; without this it would open the popover again at once.
    closed_at: Cell<i64>,
    /// The rows on the list by notification id, with what each was drawn from: a refresh
    /// during a burst rebuilds only the rows that changed, so the others stay under the pointer.
    rows: RefCell<std::collections::HashMap<i64, (String, gtk::Box)>>,
}

thread_local! {
    static CENTER: RefCell<Option<Rc<Center>>> = const { RefCell::new(None) };
}

fn center() -> Option<Rc<Center>> {
    CENTER.with(|c| c.borrow().clone())
}

pub(super) fn install(ui: &Rc<Ui>, key: &gtk::Button) {
    let popover = gtk::Popover::new();
    popover.add_css_class("notification-center");
    popover.set_widget_name("notification-center");
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.set_parent(key);
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.set_size_request(400, -1);
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    heading.add_css_class("notification-heading");
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 1);
    titles.set_hexpand(true);
    titles.append(&label("Notifications", "notification-title"));
    let summary = label("", "notification-summary");
    titles.append(&summary);
    heading.append(&titles);
    let mark_all = button("Mark all read", "quiet");
    mark_all.set_widget_name("notifications-mark-all");
    mark_all.set_valign(gtk::Align::Center);
    heading.append(&mark_all);
    frame.append(&heading);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("notification-list");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(480)
        .build();
    frame.append(&scroll);
    popover.set_child(Some(&frame));
    let center = Rc::new(Center {
        ui: Rc::downgrade(ui),
        popover,
        list,
        summary,
        mark_all,
        loading: Cell::new(false),
        stale: Cell::new(false),
        closed_at: Cell::new(0),
        rows: RefCell::default(),
    });
    let weak = Rc::downgrade(&center);
    center.popover.connect_closed(move |_| {
        if let Some(center) = weak.upgrade() {
            center.closed_at.set(glib::monotonic_time());
        }
    });
    let weak = Rc::downgrade(&center);
    center.mark_all.connect_clicked(move |key| {
        let Some(center) = weak.upgrade() else { return };
        let Some(ui) = center.ui.upgrade() else { return };
        key.set_sensitive(false);
        // Its notify.changed event reloads the list and the bell.
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("notify.ack_all", json!({})).await {
                ui.show_error(&refusal("notify.ack_all", &e));
                center.load();
            }
        });
    });
    key.connect_clicked(|_| {
        let Some(center) = self::center() else { return };
        if center.popover.is_visible() {
            center.popover.popdown();
        } else if glib::monotonic_time() - center.closed_at.get() > 250_000 {
            center.show();
        }
    });
    CENTER.with(|c| *c.borrow_mut() = Some(center));
}

/// Open the popover, as the bell does (command palette, `ui.page.switch` to notifications).
/// It opens on the next idle, so a palette or sheet that asked for it has closed first.
pub(super) fn open() {
    glib::idle_add_local_once(|| {
        if let Some(center) = center().filter(|c| !c.popover.is_visible()) {
            center.show();
        }
    });
}

/// A `notify.*` event arrived: an open popover reloads; a closed one loads when opened.
pub(super) fn changed() {
    if let Some(center) = center().filter(|c| c.popover.is_visible()) {
        center.load();
    }
}

impl Center {
    fn show(self: &Rc<Self>) {
        if self.list.first_child().is_none() {
            self.placeholder("Loading…", "");
        }
        self.popover.popup();
        self.load();
    }

    /// A burst of notify events (an agent retrying a refused command) shares one list: the one
    /// in flight runs once more when it answers, rather than one request and rebuild per event.
    fn load(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        self.stale.set(true);
        if self.loading.replace(true) {
            return;
        }
        let center = self.clone();
        glib::spawn_future_local(async move {
            while center.stale.replace(false) {
                let result = ui.call("notify.list", json!({"limit": 100})).await;
                if center.stale.get() {
                    continue;
                }
                match result {
                    Ok(data) => {
                        let entries = rows(&data, "notifications");
                        // Unread across all notifications, not only the newest 100 listed: an
                        // older unread one still counts, and Mark all read must still clear it.
                        let unread = data["unread"].as_u64().map_or_else(
                            || entries.iter().filter(|n| n["read"] != true).count(),
                            |n| n as usize,
                        );
                        center.render(&ui, &entries, unread)
                    }
                    Err(e) => center.placeholder("Notifications are unavailable", &refusal("notify.list", &e)),
                }
            }
            center.loading.set(false);
        });
    }

    fn placeholder(&self, title: &str, detail: &str) {
        clear(&self.list);
        self.rows.borrow_mut().clear();
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 4);
        empty.add_css_class("notification-empty");
        let glyph = crate::icons::image_with_stroke("bell", 20, 1.25);
        glyph.add_css_class("empty-glyph");
        empty.append(&glyph);
        let heading = label(title, "notification-empty-title");
        heading.set_xalign(0.5);
        empty.append(&heading);
        if !detail.is_empty() {
            let copy = label(detail, "notification-empty-detail");
            copy.set_xalign(0.5);
            copy.set_wrap(true);
            copy.set_justify(gtk::Justification::Center);
            copy.set_max_width_chars(44);
            empty.append(&copy);
        }
        self.list.append(&empty);
    }

    fn render(self: &Rc<Self>, ui: &Rc<Ui>, entries: &[Value], unread: usize) {
        self.summary.set_text(&match unread {
            0 if entries.is_empty() => String::from("Nothing yet"),
            0 => String::from("All read"),
            1 => String::from("1 unread"),
            n => format!("{n} unread"),
        });
        self.mark_all.set_sensitive(unread > 0);
        if entries.is_empty() {
            self.placeholder(
                "You're all caught up",
                "Finished, blocked and held agents report here.",
            );
            return;
        }
        // A placeholder, or rows without an id, go; rows still listed stay where they are.
        let mut rows = std::mem::take(&mut *self.rows.borrow_mut());
        let mut child = self.list.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if !rows.values().any(|(_, row)| row.upcast_ref::<gtk::Widget>() == &widget) {
                self.list.remove(&widget);
            }
        }
        let projects = ui.projects.borrow();
        let here = ui.project.get();
        let mut kept = std::collections::HashMap::new();
        let mut previous: Option<gtk::Widget> = None;
        for item in entries {
            let project = item["project_id"].as_i64().and_then(|id| {
                projects
                    .iter()
                    .find(|p| p["id"].as_i64() == Some(id))
                    .map(|p| text(p, "name").to_string())
            });
            // Its "5m" too, so a kept row's age does not stand still.
            let when = crate::relative::ago(text(item, "created_at"), crate::relative::Form::Compact).unwrap_or_default();
            let key = format!("{item}|{project:?}|{here}|{when}");
            let id = item["id"].as_i64().unwrap_or(0);
            let row = match rows.remove(&id) {
                Some((drawn, row)) if drawn == key => row,
                old => {
                    if let Some((_, old)) = old {
                        self.list.remove(&old);
                    }
                    self.row(item, project.as_deref())
                }
            };
            if row.parent().is_some() {
                self.list.reorder_child_after(&row, previous.as_ref());
            } else {
                self.list.insert_child_after(&row, previous.as_ref());
            }
            previous = Some(row.clone().upcast());
            kept.insert(id, (key, row));
        }
        for (_, (_, gone)) in rows {
            self.list.remove(&gone);
        }
        *self.rows.borrow_mut() = kept;
    }

    fn row(self: &Rc<Self>, item: &Value, project: Option<&str>) -> gtk::Box {
        let unread = item["read"] != true;
        let category = text(item, "category");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.add_css_class("notification-row");
        if unread {
            row.add_css_class("unread");
        }
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("notification-dot");
        dot.add_css_class(match category {
            "agent_blocked" | "guardrail" => "held",
            "agent_done" | "aide" => "live",
            _ => "neutral",
        });
        dot.set_valign(gtk::Align::Start);
        dot.set_opacity(if unread { 1.0 } else { 0.0 });
        content.append(&dot);
        let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
        copy.set_hexpand(true);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = label(text(item, "title"), "notification-row-title");
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&title);
        // Where, only when it is not here, then when, in the corner: the dot already says what
        // kind, and the title who.
        let when = crate::relative::ago(text(item, "created_at"), crate::relative::Form::Compact).unwrap_or_default();
        let here = self.ui.upgrade().map(|ui| ui.project.get());
        let corner = match project.filter(|p| !p.is_empty() && item["project_id"].as_i64() != here) {
            Some(project) => format!("{project} · {when}"),
            None => when,
        };
        let corner_label = label(&corner, "notification-time");
        corner_label.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        corner_label.set_max_width_chars(22);
        corner_label.set_tooltip_text(Some(&relative_time(text(item, "created_at"))));
        top.append(&corner_label);
        copy.append(&top);
        // An agent's whole final message arrives as the body; the row shows its opening, in
        // plain words, and following the row opens the rest.
        let (gist, cut) = excerpt(text(item, "body"));
        if !gist.is_empty() {
            let body_label = label(&gist, "notification-body");
            body_label.set_wrap(true);
            body_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            body_label.set_lines(2);
            body_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            body_label.set_max_width_chars(46);
            body_label.set_xalign(0.0);
            if cut {
                body_label.set_tooltip_text(Some(&gist));
            }
            copy.append(&body_label);
        }
        content.append(&copy);
        // A notification whose link the UI cannot route, or that has none, offers no "Open"
        // (D121): its row is plain text, and only Mark read acts on it.
        if route(item).is_some() {
            let open = button("", "notification-open");
            open.set_hexpand(true);
            open.set_tooltip_text(Some("Open where this happened"));
            open.set_child(Some(&content));
            row.append(&open);
            let weak = Rc::downgrade(self);
            let target = item.clone();
            open.connect_clicked(move |_| {
                if let Some(center) = weak.upgrade() {
                    center.follow(&target);
                }
            });
        } else {
            // The padding button.notification-open would have given it.
            content.set_hexpand(true);
            content.set_margin_top(8);
            content.set_margin_bottom(8);
            content.set_margin_start(12);
            content.set_margin_end(12);
            row.append(&content);
        }
        if unread {
            let read = icon_button("check", "Mark read");
            read.add_css_class("notification-read");
            read.set_valign(gtk::Align::Center);
            let weak = Rc::downgrade(self);
            let id = item["id"].clone();
            read.connect_clicked(move |key| {
                let Some(center) = weak.upgrade() else { return };
                let Some(ui) = center.ui.upgrade() else { return };
                key.set_sensitive(false);
                let id = id.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = ui.call("notify.ack", json!({"notification_id": id})).await {
                        ui.show_error(&refusal("notify.ack", &e));
                        center.load();
                    }
                });
            });
            row.append(&read);
        }
        row
    }

    /// Mark `item` read and go where it happened: its task, its page or its project's agents.
    fn follow(&self, item: &Value) {
        let Some(ui) = self.ui.upgrade() else { return };
        let Some(page) = route(item) else { return };
        self.popover.popdown();
        if item["read"] != true {
            let ui = ui.clone();
            let id = item["id"].clone();
            glib::spawn_future_local(async move {
                let _ = ui.call("notify.ack", json!({"notification_id": id})).await;
            });
        }
        // Aide speaks for no project: its page keeps what it said.
        if text(item, "category") == "aide" {
            ui.navigate(page);
            return;
        }
        let project = item["project_id"].as_i64().unwrap_or(ui.project.get());
        let link = &item["link"];
        let payload = &link["payload"];
        if project == 0 {
            ui.navigate(page);
            return;
        }
        ui.open_project(project, page);
        if ui.project.get() != project {
            return;
        }
        if text(link, "op") == "task.get" {
            if let Some(id) = payload["task_id"].as_i64() {
                crate::pages::open_task(&ui, id);
            }
        }
        // A session's own notice focuses its terminal, as the tile's focus key does, and
        // keeps it focused when the project's saved layout is restored after the switch.
        if let Some(session) = payload["session"].as_str() {
            ui.focus_session(session);
        }
    }
}

/// How much of a body a row keeps; two lines show well under this, and the rest is the link's.
const EXCERPT_CHARS: usize = 240;

/// The opening of `body` as one plain paragraph, and whether anything was left out: markdown
/// marks, list bullets, headings, tables and code blocks dropped, line breaks run together.
fn excerpt(body: &str) -> (String, bool) {
    let mut words = String::new();
    let mut fenced = false;
    let mut dropped = false;
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("```") {
            fenced = !fenced;
            dropped = true;
            continue;
        }
        if fenced || line.starts_with('|') {
            dropped = true;
            continue;
        }
        let line = line.trim_start_matches('#').trim_start_matches('>').trim_start();
        let line = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).unwrap_or(line);
        let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let line = match line[digits..].strip_prefix(". ") {
            Some(rest) if digits > 0 => rest,
            _ => line,
        };
        for word in line.split_whitespace() {
            let word = word.replace("**", "").replace("__", "").replace('`', "");
            if word.is_empty() {
                continue;
            }
            if !words.is_empty() {
                words.push(' ');
            }
            words.push_str(&word);
        }
    }
    match words.char_indices().nth(EXCERPT_CHARS) {
        None => (words, dropped),
        Some((at, _)) => (format!("{}…", words[..at].trim_end()), true),
    }
}

/// The page following `item` opens, or None when its link names nothing the UI can route
/// (or it has no link), in which case it shows no "Open" affordance (D121).
fn route(item: &Value) -> Option<&str> {
    let link = &item["link"];
    let payload = &link["payload"];
    Some(match text(link, "op") {
        "ui.page.switch" => payload["page"]
            .as_str()
            .filter(|p| *p != "notifications")
            .unwrap_or("agents"),
        "task.get" => "board",
        // A hold or exception request waits on its session's tile (D121), which the focus
        // in follow() brings forward; the Guardrails page would leave that focus on a hidden wall.
        "session.get" | "guardrail.confirm" => "agents",
        // Aide's notifications carry no link: its Overview holds the message and what waits.
        _ if text(item, "category") == "aide" => "aide-home",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_excerpt_reads_as_plain_words() {
        let body = "The queue is set up:\n\n1. **Gemma v1** is running; `23` passed.\n- Next: distill\n\n| a | b |\n|---|---|\n```\ncode\n```";
        assert_eq!(excerpt(body), ("The queue is set up: Gemma v1 is running; 23 passed. Next: distill".into(), true));
        assert_eq!(excerpt("  Short.  "), ("Short.".into(), false));
        assert_eq!(excerpt(""), (String::new(), false));
        assert_eq!(excerpt("2026 was a year"), ("2026 was a year".into(), false));
    }

    #[test]
    fn an_aide_notice_opens_its_overview() {
        assert_eq!(route(&json!({"category": "aide", "link": null})), Some("aide-home"));
        assert_eq!(route(&json!({"category": "system", "link": null})), None);
    }

    #[test]
    fn a_long_body_is_cut_at_a_char_boundary() {
        let (gist, cut) = excerpt(&"é".repeat(500));
        assert!(cut);
        assert_eq!(gist.chars().count(), EXCERPT_CHARS + 1);
        assert!(gist.ends_with('…'));
    }
}
