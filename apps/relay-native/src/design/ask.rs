//! Ask: hand the selection to the terminal's agent, as Claude Design's comments hand an element
//! to Claude. What you type goes into the design's terminal as a prompt naming the design, the
//! page and the selected layers by id, with a picture of them, so the agent can read them with
//! `design.get` and change them with `design.apply` while you watch.

use super::board::{display_name, kind_caption, Board};
use crate::app::{label, rows, text};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::ops::design::AssetAddOut;
use serde_json::json;
use std::rc::Rc;

impl Board {
    pub(super) fn ask_agent(&self, anchor: &gtk::Widget) {
        let b = self.me();
        // The selection toolbar is rebuilt as the design changes, which would take the
        // composer with it: it opens from the top bar's Ask instead.
        let in_bar = self.context.borrow().as_ref().is_some_and(|bar| anchor == bar || anchor.is_ancestor(bar));
        let anchor = if in_bar { self.ask.upcast_ref::<gtk::Widget>() } else { anchor };
        self.end_edit(true);
        match self.doc.session() {
            Some(session) => composer(&b, anchor, session),
            None => pick_terminal(&b, anchor),
        }
    }
}

/// No terminal yet: choose one, which ties the design to it, then ask.
fn pick_terminal(b: &Rc<Board>, anchor: &gtk::Widget) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.set_parent(anchor);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.append(&label("Ask which terminal's agent?", "design-menu-heading"));
    pop.set_child(Some(&list));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
    let project = b.doc.design.borrow().project_id;
    let weak = Rc::downgrade(b);
    let p = pop.downgrade();
    let anchor = anchor.downgrade();
    glib::spawn_future_local(async move {
        let sessions = ui.call("session.list", json!({"project_id": project})).await.map(|v| rows(&v, "sessions")).unwrap_or_default();
        let (Some(b), Some(pop)) = (weak.upgrade(), p.upgrade()) else {
            return;
        };
        let live: Vec<String> = sessions
            .iter()
            .filter(|s| matches!(text(s, "state"), "running" | "starting" | "created"))
            .map(|s| text(s, "name").to_string())
            .collect();
        if live.is_empty() {
            let none = label("No agent is running in this project. Start one from Agents, then ask again.", "dim");
            none.set_wrap(true);
            none.set_max_width_chars(36);
            list.append(&none);
            return;
        }
        for name in live {
            let row = gtk::Button::with_label(&name);
            row.add_css_class("design-menu-row");
            let weak = Rc::downgrade(&b);
            let pp = pop.downgrade();
            let a = anchor.clone();
            row.connect_clicked(move |_| {
                if let Some(p) = pp.upgrade() {
                    p.popdown();
                }
                if let (Some(b), Some(a)) = (weak.upgrade(), a.upgrade()) {
                    b.link_session(Some(name.clone()));
                    composer(&b, &a, name.clone());
                }
            });
            list.append(&row);
        }
    });
}

fn composer(b: &Rc<Board>, anchor: &gtk::Widget, session: String) {
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.add_css_class("design-ask-pop");
    pop.set_parent(anchor);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_size_request(360, -1);
    body.append(&label(&format!("Ask {session}"), "design-ask-title"));
    let ids = b.selected();
    let about = {
        let scene = b.doc.scene.borrow();
        match ids.as_slice() {
            [] => format!("About {} on {}", b.doc.title(), b.page_name(&b.page.borrow())),
            [one] => format!("About {}", scene.node(one).map(|n| format!("{} “{}”", kind_caption(n.kind).to_lowercase(), display_name(n))).unwrap_or_default()),
            many => format!("About {} selected layers", many.len()),
        }
    };
    body.append(&label(&about, "dim"));
    let view = gtk::TextView::new();
    view.add_css_class("design-ask-text");
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_size_request(-1, 76);
    view.set_accepts_tab(false);
    let frame = gtk::ScrolledWindow::new();
    frame.set_child(Some(&view));
    frame.set_max_content_height(200);
    frame.set_propagate_natural_height(true);
    frame.add_css_class("design-ask-field");
    body.append(&frame);
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let picture = gtk::CheckButton::with_label("Include a picture");
    picture.set_active(true);
    picture.set_tooltip_text(Some("Attach a PNG of the selection (or the page) so the agent sees what you see"));
    picture.set_hexpand(true);
    foot.append(&picture);
    let send = gtk::Button::new();
    send.add_css_class("primary");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&label("Send", ""));
    row.append(&label("Ctrl Enter", "design-keycap-dark"));
    send.set_child(Some(&row));
    foot.append(&send);
    body.append(&foot);
    pop.set_child(Some(&body));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    let go = {
        let weak = Rc::downgrade(b);
        let view = view.downgrade();
        let picture = picture.downgrade();
        let p = pop.downgrade();
        let session = session.clone();
        Rc::new(move || {
            let (Some(b), Some(view), Some(picture)) = (weak.upgrade(), view.upgrade(), picture.upgrade()) else {
                return;
            };
            let buffer = view.buffer();
            let request = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
            if request.is_empty() {
                view.grab_focus();
                return;
            }
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            send_request(&b, session.clone(), request, picture.is_active());
        })
    };
    let g = go.clone();
    send.connect_clicked(move |_| g());
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, mods| {
        if mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) && matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
            go();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    view.add_controller(keys);
    pop.popup();
    view.grab_focus();
}

/// The prompt the agent reads: one line, naming everything by id.
pub fn prompt(title: &str, design: i64, page: &str, selected: &[(String, String, String)], picture: Option<&str>, request: &str) -> String {
    let mut out = format!("[Relay design] \"{title}\" (design_id {design}, page \"{page}\").");
    if !selected.is_empty() {
        let names: Vec<String> = selected
            .iter()
            .take(8)
            .map(|(id, kind, name)| format!("{id} ({kind} \"{}\")", name.replace('"', "'")))
            .collect();
        out.push_str(&format!(" Selected: {}", names.join(", ")));
        if selected.len() > 8 {
            out.push_str(&format!(" and {} more", selected.len() - 8));
        }
        out.push('.');
    }
    if let Some(p) = picture {
        out.push_str(&format!(" Picture of it: {p}."));
    }
    let request: String = request.split_whitespace().collect::<Vec<_>>().join(" ");
    out.push_str(&format!(" Request: {request} — read the design with design.get {{\"design_id\":{design}}} and change it with design.apply."));
    out
}

/// Ask the design's terminal at once, with a picture: the smoke harness's `ask:` step.
pub(super) fn ask_now(b: &Rc<Board>, request: &str) {
    if let Some(session) = b.doc.session() {
        send_request(b, session, request.to_string(), true);
    }
}

fn send_request(b: &Rc<Board>, session: String, request: String, with_picture: bool) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let ids = b.selected();
    let rendered = with_picture.then(|| super::export::render(b, &ids, 1.0, ids.is_empty())).flatten();
    let selected: Vec<(String, String, String)> = {
        let scene = b.doc.scene.borrow();
        ids.iter()
            .filter_map(|id| scene.node(id).map(|n| (id.clone(), kind_caption(n.kind).to_lowercase(), display_name(n))))
            .collect()
    };
    let title = b.doc.title();
    let design = b.doc.id;
    let page = b.page_name(&b.page.borrow());
    let weak = Rc::downgrade(b);
    let rt = ui.rt.clone();
    glib::spawn_future_local(async move {
        let mut picture = None;
        if let Some((texture, _)) = rendered {
            let path = glib::user_runtime_dir().join(format!("relay-design-ask-{}.png", uuid::Uuid::new_v4()));
            let p = path.clone();
            let written = rt.spawn_blocking(move || std::fs::write(&p, texture.save_to_png_bytes())).await;
            if matches!(written, Ok(Ok(()))) {
                if let Ok(v) = ui.call("design.asset.add", json!({"design_id": design, "path": path.to_string_lossy()})).await {
                    picture = serde_json::from_value::<AssetAddOut>(v).ok().map(|o| o.path);
                }
            }
            let _ = std::fs::remove_file(&path);
        }
        let message = prompt(&title, design, &page, &selected, picture.as_deref(), &request);
        let typed = ui.call("session.input", json!({"session": session, "data": message})).await;
        let result = match typed {
            Ok(_) => {
                // Enter on its own, so a terminal program reads the prompt as typed, not pasted.
                glib::timeout_future(std::time::Duration::from_millis(60)).await;
                ui.call("session.input", json!({"session": session, "data": "\r"})).await.map(|_| ())
            }
            Err(e) => Err(e),
        };
        if let Some(b) = weak.upgrade() {
            match result {
                Ok(()) => b.toast(&format!("Sent to {session}. Its changes show here as it makes them.")),
                Err(e) => b.toast(&format!("Could not reach {session}: {e}")),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::prompt;

    #[test]
    fn the_prompt_names_everything_by_id_on_one_line() {
        let p = prompt(
            "Settings",
            12,
            "Page 1",
            &[("cta".into(), "frame".into(), "Save \"now\"".into())],
            Some("/data/design-assets/ab.png"),
            "Make it\nbigger",
        );
        assert!(!p.contains('\n'));
        assert!(p.contains("design_id 12"));
        assert!(p.contains("cta (frame \"Save 'now'\")"));
        assert!(p.contains("Picture of it: /data/design-assets/ab.png."));
        assert!(p.contains("Request: Make it bigger"));
        assert!(p.contains("design.get {\"design_id\":12}"));
    }
}
