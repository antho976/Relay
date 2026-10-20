//! Ask: hand the selection to the terminal's agent, as Claude Design's comments hand an element
//! to Claude. What you type goes to the design's terminal as a prompt naming the design, the
//! page and the selected layers by id, with a picture of them, so the agent can read them with
//! `design.get` and change them with `design.apply` while you watch.
//!
//! When what is asked about moves, the picture is a contact sheet of its motion, and the prompt
//! says so with the frames' times (feature-motion §3).
//!
//! An agent at its prompt (idle) has the prompt pasted in, its line breaks kept inside a
//! bracketed paste, then an Enter of its own (I-4); never over text the person has typed there
//! and not sent (TM-20). One at work (running) gets it as priority mail, which it reads with its
//! next tool call, rather than keystrokes queued behind its turn. One waiting on the person
//! (blocked: a permission prompt or a question) or still starting is never typed into, since a
//! digit of the prompt would answer its dialog (TM-1); Show terminal takes the person there.

use super::board::{display_name, Board};
use super::export;
use crate::app::{label, rows, text};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::ops::design::{AssetAddOut, GET_NODES_MAX};
use relay_client::design::names::node_caption;
use serde_json::{json, Value};
use std::rc::Rc;
use std::time::Duration;

/// How long after an Ask the board says so if the agent has changed nothing (E-ask-7).
const ANSWER_WAIT: Duration = Duration::from_secs(90);
/// How long Ask waits for a woken terminal's agent to come up (I-5).
const WAKE_WAIT: Duration = Duration::from_secs(20);
/// The selected layers the prompt names with their kind and name; the rest by id alone.
const NAMED: usize = 8;

// Ask was board.rs's `kind_caption`'s last caller; it now names what a layer draws
// (`names::node_caption`, E-board-24). The function goes with board.rs's next change: that
// file belongs to another stream while this one is built.
const _: fn(relay_bus::design::NodeKind) -> &'static str = super::board::kind_caption;

/// A session that has an agent to read what is sent to it, as the agent wall counts them.
pub fn live(state: &str) -> bool {
    matches!(state, "spawning" | "running" | "idle" | "blocked")
}

/// Whether a session in `state` may be typed into: its agent is at its prompt or at work, not
/// at a dialog waiting on the person (blocked) or still starting (TM-1).
fn typable(state: &str) -> bool {
    matches!(state, "running" | "idle")
}

/// Whether a session in `state` can be woken to be asked: parked, or kept from the last run.
fn wakeable(state: &str) -> bool {
    matches!(state, "parked" | "restorable")
}

/// What Send does with a terminal in a state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Route {
    /// At its prompt: type the prompt in.
    Type,
    /// At work: priority mail, read with its next tool call (I-9).
    Mail,
    /// At a dialog waiting on the person: say so, and offer mail.
    Blocked,
    /// Still starting: say so.
    Starting,
    /// Gone: choose another terminal.
    Gone,
}

fn route(state: &str) -> Route {
    match state {
        "idle" => Route::Type,
        "running" => Route::Mail,
        "blocked" => Route::Blocked,
        "spawning" => Route::Starting,
        _ => Route::Gone,
    }
}

/// Where a terminal in `state` sorts in the picker: at its prompt first, then at work, then
/// waiting on the person, then starting (E-ask-2).
fn rank(state: &str) -> u8 {
    match state {
        "idle" => 0,
        "running" => 1,
        "blocked" => 2,
        _ => 3,
    }
}

/// The picker's word beside a terminal that cannot be typed into now.
fn state_note(state: &str) -> Option<&'static str> {
    match state {
        "blocked" => Some("waiting on you"),
        "spawning" => Some("starting"),
        _ => None,
    }
}

/// What the picker says of the design's terminal, `session`, gone to `state` (E-ask-1).
fn gone_heading(session: &str, state: &str) -> String {
    match state {
        "parked" | "restorable" => format!("{session} is parked."),
        "created" => format!("{session} has not started."),
        _ => format!("{session}'s terminal has closed."),
    }
}

/// What the board says while Send waits on a terminal it will not type into.
fn hold_note(session: &str, route: Route) -> Option<String> {
    match route {
        Route::Blocked => Some(format!("{session} is waiting on you in its terminal (a permission prompt or a question). Answer it there, then Send.")),
        Route::Starting => Some(format!("{session} is still starting; Send again in a moment.")),
        _ => None,
    }
}

/// `session`'s state in a `session.list` answer; "closed" when it is not listed.
fn state_of(listed: &Value, session: &str) -> String {
    rows(listed, "sessions").iter().find(|s| text(s, "name") == session).map_or("closed", |s| text(s, "state")).to_string()
}

/// `session`'s provider in a `session.list` answer; empty when it is not listed.
fn provider_of(listed: &Value, session: &str) -> String {
    rows(listed, "sessions").iter().find(|s| text(s, "name") == session).map_or("", |s| text(s, "provider")).to_string()
}

/// What Send says when `session`'s open pane shows text typed at its prompt and not sent: Ask
/// would run its prompt into it (TM-20). `None` for an empty prompt, and when no pane is
/// attached, where the paste's brackets keep a leading `/` or `!` from acting as a command.
fn unsent_note(session: &str, provider: &str) -> Option<String> {
    let row = crate::terminal::prompt_row(session)?;
    (!crate::terminal::prompt_line_empty(&row, provider)).then(|| format!("{session}'s prompt has unsent text; send or clear it there first."))
}

/// Where a request goes once its picture is ready, by `session`'s `state` then: pasted at its
/// prompt unless text is left there (`unsent`, read only for an agent at its prompt), mail for
/// one gone to work, else held with what to say, as Send's own check routes it (TM-1, TM-20).
fn recheck(session: &str, state: &str, unsent: impl FnOnce() -> Option<String>) -> Result<Route, String> {
    match route(state) {
        Route::Type => unsent().map_or(Ok(Route::Type), Err),
        Route::Mail => Ok(Route::Mail),
        way => Err(hold_note(session, way).unwrap_or_else(|| format!("{session}'s terminal has closed."))),
    }
}

/// The composer's draft once `sent` has gone: emptied while it is still what was sent, kept when
/// the person has typed another since.
fn clear_sent(draft: &mut String, sent: &str) {
    if draft.trim() == sent.trim() {
        draft.clear();
    }
}

/// Show `session`'s terminal from the board, or say why it cannot be (TM-1, E-ask-7).
fn show_terminal(b: &Board, session: &str) {
    if !b.ui.upgrade().is_some_and(|ui| ui.show_session(session)) {
        b.toast(&format!("{session}'s terminal is not on the wall"));
    }
}

/// A terminal's lamp, as the board's terminal key shows it.
fn lamp(state: &str) -> gtk::Box {
    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    lamp.add_css_class("design-lamp");
    lamp.add_css_class(state);
    lamp.set_valign(gtk::Align::Center);
    lamp
}

impl Board {
    pub(super) fn ask_agent(&self, anchor: &gtk::Widget) {
        let b = self.me();
        // The selection toolbar is rebuilt as the design changes, which would take the
        // composer with it: it opens from the top bar's Ask instead.
        let in_bar = self.context.borrow().as_ref().is_some_and(|bar| anchor == bar || anchor.is_ancestor(bar));
        let anchor = if in_bar { self.ask.upcast_ref::<gtk::Widget>() } else { anchor };
        self.end_edit(true);
        let (Some(session), Some(ui)) = (self.doc.session(), self.ui.upgrade()) else {
            pick_terminal(&b, anchor, None);
            return;
        };
        // A terminal that has closed reads nothing: offer the live ones instead. Whether this
        // one can be typed into is asked again at Send.
        let project = self.doc.design.borrow().project_id;
        let weak = Rc::downgrade(&b);
        let anchor = anchor.downgrade();
        glib::spawn_future_local(async move {
            let listed = ui.call("session.list", json!({"project_id": project})).await;
            let (Some(b), Some(anchor)) = (weak.upgrade(), anchor.upgrade()) else {
                return;
            };
            // Unread, the composer opens and Send reads it again.
            let state = listed.as_ref().ok().map(|v| state_of(v, &session));
            if let Some(state) = &state {
                b.set_lamp(state);
            }
            match state {
                Some(state) if !live(&state) => pick_terminal(&b, &anchor, Some((session, state))),
                _ => composer(&b, &anchor, session),
            }
        });
    }
}

/// No terminal yet, or the design's has gone (its name and state): choose a live one, which
/// ties the design to it, or wake a parked one, then ask.
fn pick_terminal(b: &Rc<Board>, anchor: &gtk::Widget, gone: Option<(String, String)>) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.set_parent(anchor);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let heading = match &gone {
        Some((s, state)) => format!("{} Ask which terminal's agent?", gone_heading(s, state)),
        None => "Ask which terminal's agent?".to_string(),
    };
    let heading = label(&heading, "design-menu-heading");
    heading.set_wrap(true);
    heading.set_max_width_chars(36);
    list.append(&heading);
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
        let named = |keep: fn(&str) -> bool| -> Vec<(String, String)> {
            let mut out: Vec<(String, String)> =
                sessions.iter().filter(|s| keep(text(s, "state"))).map(|s| (text(s, "name").to_string(), text(s, "state").to_string())).collect();
            out.sort_by(|a, b| (rank(&a.1), &a.0).cmp(&(rank(&b.1), &b.0)));
            out
        };
        // Every live agent, each with its lamp; one waiting on the person or starting says so,
        // and Send waits for it (E-ask-2, TM-1).
        let (live, parked) = (named(live), named(wakeable));
        if live.is_empty() && parked.is_empty() {
            let none = label("No agent is running in this project. Start one from Agents, then ask again.", "dim");
            none.set_wrap(true);
            none.set_max_width_chars(36);
            list.append(&none);
            return;
        }
        let row = |name: &str, state: &str, on: Rc<Pick>| {
            let row = gtk::Button::new();
            row.add_css_class("design-menu-row");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.append(&lamp(state));
            let caption = label(name, "");
            caption.set_hexpand(true);
            caption.set_xalign(0.0);
            content.append(&caption);
            if let Some(note) = state_note(state) {
                content.append(&label(note, "dim"));
            }
            row.set_child(Some(&content));
            let weak = Rc::downgrade(&b);
            let pp = pop.downgrade();
            let a = anchor.clone();
            row.connect_clicked(move |_| {
                if let Some(p) = pp.upgrade() {
                    p.popdown();
                }
                if let (Some(b), Some(a)) = (weak.upgrade(), a.upgrade()) {
                    on(&b, &a);
                }
            });
            list.append(&row);
        };
        for (name, state) in live {
            let n = name.clone();
            row(&name, &state, Rc::new(move |b, a| {
                b.link_session(Some(n.clone()));
                composer(b, a, n.clone());
            }));
        }
        // A parked terminal is woken, then asked (E-ask-1, I-5).
        if !parked.is_empty() {
            list.append(&label("Wake and ask", "design-menu-heading"));
        }
        for (name, state) in parked {
            let (n, s) = (name.clone(), state.clone());
            row(&name, &state, Rc::new(move |b, a| wake_and_ask(b, a, n.clone(), &s)));
        }
    });
}

/// What choosing a terminal in the picker does, with the board and the picker's anchor.
type Pick = dyn Fn(&Rc<Board>, &gtk::Widget);

/// Wake `session` (parked, or kept from the last run), wait up to 20 s for its agent to be up,
/// then tie the design to it and open the composer.
fn wake_and_ask(b: &Rc<Board>, anchor: &gtk::Widget, session: String, state: &str) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let op = if state == "restorable" { "session.resume" } else { "session.wake" };
    b.toast(&format!("Waking {session}…"));
    let project = b.doc.design.borrow().project_id;
    let weak = Rc::downgrade(b);
    let anchor = anchor.downgrade();
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call(op, json!({"session": session})).await {
            if let Some(b) = weak.upgrade() {
                b.toast(&format!("Could not wake {session}: {e}"));
            }
            return;
        }
        let started = std::time::Instant::now();
        let up = loop {
            if let Ok(listed) = ui.call("session.list", json!({"project_id": project})).await {
                let state = state_of(&listed, &session);
                if typable(&state) || started.elapsed() >= WAKE_WAIT {
                    break typable(&state);
                }
            } else if started.elapsed() >= WAKE_WAIT {
                break false;
            }
            glib::timeout_future(Duration::from_millis(500)).await;
        };
        let (Some(b), Some(anchor)) = (weak.upgrade(), anchor.upgrade()) else {
            return;
        };
        if up {
            b.link_session(Some(session.clone()));
            composer(&b, &anchor, session);
        } else {
            b.toast(&format!("{session} is not up yet; ask again once its terminal shows it ready"));
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
            [] => format!("About the whole of {} in {}", b.page_name(&b.page.borrow()), b.doc.title()),
            // What the layer draws, as the layers list calls it: a vector shape is no "line".
            [one] => format!("About {}", scene.node(one).map(|n| format!("{} “{}”", node_caption(n).to_lowercase(), display_name(n))).unwrap_or_default()),
            many => format!("About {} selected layers", many.len()),
        }
    };
    body.append(&label(&about, "dim"));
    skill_note(b, &body);
    let view = gtk::TextView::new();
    view.add_css_class("design-ask-text");
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_size_request(-1, 76);
    view.set_accepts_tab(false);
    // What was typed and not sent waits for the next Ask.
    view.buffer().set_text(&b.ask_draft.borrow());
    let weak = Rc::downgrade(b);
    view.buffer().connect_changed(move |buffer| {
        if let Some(b) = weak.upgrade() {
            *b.ask_draft.borrow_mut() = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
        }
    });
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
    // Why Send is waiting, and mail for an agent at a dialog (TM-1, I-9).
    let hold = label("", "dim");
    hold.set_wrap(true);
    hold.set_max_width_chars(44);
    hold.set_xalign(0.0);
    hold.set_visible(false);
    body.append(&hold);
    let held = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    held.set_halign(gtk::Align::End);
    held.set_visible(false);
    let show = gtk::Button::with_label("Show terminal");
    show.set_tooltip_text(Some(&format!("Open {session}'s terminal to answer it or clear its prompt")));
    held.append(&show);
    let mail = gtk::Button::with_label("Send as mail instead");
    mail.set_tooltip_text(Some("The agent reads it with its next tool call, once its terminal is answered"));
    held.append(&mail);
    body.append(&held);
    {
        let (weak, p, session) = (Rc::downgrade(b), pop.downgrade(), session.clone());
        show.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let Some(b) = weak.upgrade() {
                show_terminal(&b, &session);
            }
        });
    }
    pop.set_child(Some(&body));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    // What was typed, or nothing yet to send.
    let typed = {
        let view = view.downgrade();
        move || {
            let view = view.upgrade()?;
            let buffer = view.buffer();
            let request = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
            if request.is_empty() {
                view.grab_focus();
                return None;
            }
            Some(request)
        }
    };
    let typed = Rc::new(typed);
    {
        let (weak, picture, p, session, typed) = (Rc::downgrade(b), picture.downgrade(), pop.downgrade(), session.clone(), typed.clone());
        mail.connect_clicked(move |_| {
            let (Some(b), Some(picture), Some(request)) = (weak.upgrade(), picture.upgrade(), typed()) else {
                return;
            };
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            send_request(&b, session.clone(), request, picture.is_active(), Route::Mail);
        });
    }
    // Send reads the terminal's state again: it may have stopped at a dialog since the
    // composer opened (TM-1).
    let go = {
        let weak = Rc::downgrade(b);
        let picture = picture.downgrade();
        let (p, send, hold, held) = (pop.downgrade(), send.downgrade(), hold.downgrade(), held.downgrade());
        let session = session.clone();
        Rc::new(move || {
            let (Some(b), Some(request), Some(ui)) = (weak.upgrade(), typed(), weak.upgrade().and_then(|b| b.ui.upgrade())) else {
                return;
            };
            let Some(send) = send.upgrade().filter(|s| s.is_sensitive()) else {
                return;
            };
            send.set_sensitive(false);
            let project = b.doc.design.borrow().project_id;
            let (weak, picture, p, hold, held, session) = (Rc::downgrade(&b), picture.clone(), p.clone(), hold.clone(), held.clone(), session.clone());
            glib::spawn_future_local(async move {
                let listed = ui.call("session.list", json!({"project_id": project})).await;
                send.set_sensitive(true);
                let (Some(b), Some(picture), Some(pop), Some(hold), Some(held)) = (weak.upgrade(), picture.upgrade(), p.upgrade(), hold.upgrade(), held.upgrade()) else {
                    return;
                };
                let (state, provider) = match listed {
                    Ok(v) => (state_of(&v, &session), provider_of(&v, &session)),
                    Err(e) => {
                        hold.set_text(&format!("Could not check {session}'s terminal: {e}. Send again in a moment."));
                        hold.set_visible(true);
                        return;
                    }
                };
                b.set_lamp(&state);
                let way = route(&state);
                // Typed text left at its prompt holds an Ask as a dialog does; mail goes around
                // both, and Show terminal takes the person to them (TM-1, TM-20).
                let note = hold_note(&session, way).or_else(|| (way == Route::Type).then(|| unsent_note(&session, &provider)).flatten());
                if let Some(note) = note {
                    hold.set_text(&note);
                    hold.set_visible(true);
                    held.set_visible(way != Route::Starting);
                    return;
                }
                let anchor = pop.parent();
                pop.popdown();
                match way {
                    Route::Gone => {
                        if let Some(anchor) = anchor {
                            pick_terminal(&b, &anchor, Some((session, state)));
                        }
                    }
                    _ => send_request(&b, session, request, picture.is_active(), way),
                }
            });
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

/// When the Design plugin is off for the design's project, say so in the composer `body`, with
/// a key that turns it on: its agents then lack the skill for the design tools (I-5).
fn skill_note(b: &Rc<Board>, body: &gtk::Box) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let project = b.doc.design.borrow().project_id;
    let body = body.downgrade();
    let weak = Rc::downgrade(b);
    glib::spawn_future_local(async move {
        let Ok(listed) = ui.call("plugin.list", json!({"project_id": project})).await else {
            return;
        };
        let off = rows(&listed, "plugins").iter().any(|p| {
            text(p, "id") == "design" && !p["enabled_in"].as_array().is_some_and(|on| on.iter().any(|id| id.as_i64() == Some(project)))
        });
        let Some(body) = body.upgrade().filter(|_| off) else {
            return;
        };
        let note = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let words = label("The Design skill is off for this project. The agent won't know the design tools.", "dim");
        words.set_wrap(true);
        words.set_max_width_chars(36);
        words.set_xalign(0.0);
        words.set_hexpand(true);
        note.append(&words);
        let on = gtk::Button::with_label("Turn on");
        on.set_valign(gtk::Align::Center);
        note.append(&on);
        // Under the "About" line.
        body.insert_child_after(&note, body.first_child().and_then(|t| t.next_sibling()).as_ref());
        let n = note.downgrade();
        on.connect_clicked(move |key| {
            key.set_sensitive(false);
            let (ui, n, weak, key) = (ui.clone(), n.clone(), weak.clone(), key.downgrade());
            glib::spawn_future_local(async move {
                let done = ui.call("plugin.enable", json!({"plugin_id": "design", "project_id": project, "enabled": true})).await;
                match done {
                    Ok(_) => {
                        if let Some(n) = n.upgrade() {
                            n.set_visible(false);
                        }
                        if let Some(b) = weak.upgrade() {
                            b.toast("The Design skill is on: agents started in this project from now get it");
                        }
                    }
                    Err(e) => {
                        if let Some(key) = key.upgrade() {
                            key.set_sensitive(true);
                        }
                        if let Some(b) = weak.upgrade() {
                            b.toast(&format!("Could not turn on the Design skill: {e}"));
                        }
                    }
                }
            });
        });
    });
}

/// What Ask tells the agent.
pub struct Prompt<'a> {
    pub title: &'a str,
    pub design: i64,
    pub page: &'a str,
    pub page_name: &'a str,
    /// The selected layers, outermost, in paint order: id, kind as the schema names it, name.
    pub selected: &'a [(String, String, String)],
    /// The stored picture's path; `Err` with why one asked for could not be attached; `Ok(None)`
    /// when none was asked for.
    pub picture: Result<Option<&'a str>, &'a str>,
    /// What is asked about moves: the times of the contact sheet's frames when the picture is
    /// one, none when there is no sheet. `None` when it is still.
    pub motion: Option<&'a [f64]>,
    pub request: &'a str,
}

/// A name or title as the prompt quotes it: one line, without control characters (a newline
/// would send the prompt early, an escape act as a key), its spaces collapsed and its double
/// quotes made single (B76, E-ask-5).
fn clean(s: &str) -> String {
    let plain: String = s.chars().map(|c| if c.is_control() { ' ' } else if c == '"' { '\'' } else { c }).collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The request as it goes to the agent: its lines kept (it is pasted, so a line break does not
/// send it, I-4), blank lines run together into one, tabs as spaces, every other control
/// character dropped (E-ask-5).
fn clean_request(s: &str) -> String {
    let line = |l: &str| {
        let plain: String = l.chars().filter_map(|c| if c == '\t' { Some(' ') } else if c.is_control() { None } else { Some(c) }).collect();
        plain.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let mut out: Vec<String> = Vec::new();
    for l in s.lines().map(line) {
        if !(l.is_empty() && out.last().is_none_or(String::is_empty)) {
            out.push(l);
        }
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

/// The prompt the agent reads: one line but for the request's own, naming the page and every
/// selected layer by id (kinds as the schema names them), and the read that fetches just them
/// (B76, TM-4, I-1).
pub fn prompt(p: &Prompt) -> String {
    let (design, page) = (p.design, p.page);
    let mut out = format!("[Relay design] \"{}\" (design_id {design}, page \"{}\" ({page})).", clean(p.title), clean(p.page_name));
    let picture = match p.picture {
        Ok(Some(path)) => Some(format!("\"{path}\"")),
        _ => None,
    };
    let ids: Vec<&str> = p.selected.iter().map(|(id, _, _)| id.as_str()).take(GET_NODES_MAX).collect();
    if p.selected.is_empty() {
        match &picture {
            Some(path) => out.push_str(&format!(" Nothing selected; picture of the whole page {page}: {path}.")),
            None => out.push_str(&format!(" Nothing selected: about the whole page {page}.")),
        }
    } else {
        let named: Vec<String> = p.selected.iter().take(NAMED).map(|(id, kind, name)| format!("{id} ({kind} \"{}\")", clean(name))).collect();
        out.push_str(&format!(" Selected: {}", named.join(", ")));
        if ids.len() > NAMED {
            out.push_str(&format!(", also {}", ids[NAMED..].join(", ")));
        }
        if p.selected.len() > ids.len() {
            out.push_str(&format!(" and {} more", p.selected.len() - ids.len()));
        }
        out.push('.');
        if let Some(path) = &picture {
            out.push_str(&format!(" Picture of it: {path}."));
        }
    }
    if let Err(why) = p.picture {
        out.push_str(&format!(" (A picture was requested but could not be attached: {}.)", clean(why)));
    }
    if let Some(times) = p.motion {
        out.push_str(&motion_note(times, picture.is_some()));
    }
    let read = if ids.is_empty() || p.selected.len() > ids.len() {
        format!("{{\"design_id\":{design},\"page\":\"{page}\"}}")
    } else {
        format!("{{\"design_id\":{design},\"nodes\":{}}}", serde_json::to_string(&ids).unwrap_or_default())
    };
    out.push_str(&format!(" Request: {} — read it with design.get {read} and change it with design.apply.", clean_request(p.request)));
    out
}

/// What the prompt says of motion: the contact sheet's frames, or that it moves and how to see
/// it when there is no sheet.
fn motion_note(times: &[f64], sheet: bool) -> String {
    let words: Vec<String> = times.iter().map(|t| format!("{}", t.round() as i64)).collect();
    match words.as_slice() {
        [many @ .., last] if sheet && !many.is_empty() => {
            format!(" It moves: the picture is a contact sheet of {} frames at {} and {last} ms, left to right then down.", times.len(), many.join(", "))
        }
        _ => " It moves: design.render with \"frames\" shows its motion as a contact sheet, \"at_ms\" one moment of it.".to_string(),
    }
}

/// Ask the design's terminal at once, with a picture: the smoke harness's `ask:` step. It
/// checks the terminal as Send does, and says on the board why it holds.
pub(super) fn ask_now(b: &Rc<Board>, request: &str) {
    let (Some(session), Some(ui)) = (b.doc.session(), b.ui.upgrade()) else {
        return;
    };
    let project = b.doc.design.borrow().project_id;
    let (weak, request) = (Rc::downgrade(b), request.to_string());
    glib::spawn_future_local(async move {
        let listed = ui.call("session.list", json!({"project_id": project})).await;
        let state = listed.as_ref().map(|v| state_of(v, &session));
        let Some(b) = weak.upgrade() else {
            return;
        };
        // For the harness's log, as its other steps print theirs.
        println!("DESIGN_ASK {session} {}", state.as_deref().unwrap_or("unread"));
        let unsent = listed.as_ref().ok().and_then(|v| unsent_note(&session, &provider_of(v, &session)));
        match (state.as_deref().map(route), unsent) {
            (Ok(Route::Type), Some(note)) => b.toast(&note),
            (Ok(way @ (Route::Type | Route::Mail)), _) => send_request(&b, session, request, true, way),
            (Ok(way), _) => b.toast(&hold_note(&session, way).unwrap_or_else(|| format!("{session}'s terminal has closed"))),
            (Err(e), _) => b.toast(&format!("Could not check {session}'s terminal: {e}")),
        }
    });
}

/// Send `request` about the selection to `session` the `way` its state allows (pasted at its
/// prompt, or mail for one at work), with a picture when asked. Every delivery is here. The
/// request stays in the composer's draft until it has gone (TM-19); a picture asked for and
/// not attached is said, to the agent and on the board (B77); and if the agent has changed
/// nothing 90 s later, the board says so (E-ask-7).
fn send_request(b: &Rc<Board>, session: String, request: String, with_picture: bool, way: Route) {
    let Some(ui) = b.ui.upgrade() else {
        return;
    };
    let ids = b.selected();
    let moving = {
        let scene = b.doc.scene.borrow();
        export::moves(&scene, &b.page.borrow(), &ids)
    };
    // On its real ground, sized for a model (TM-26, I-8): a contact sheet of six frames when it
    // moves. The encode and the write are off the main thread.
    let rendered = with_picture.then(|| {
        let drawn = if moving {
            export::contact_sheet(b, &ids, true, 4.0, export::ASK_SIDE, export::ASK_FRAMES).map(|(texture, times)| (texture, Some(times)))
        } else {
            export::extent(b, &ids).ok_or(export::Failure::Empty).and_then(|area| export::render(b, &ids, true, export::ask_scale(&area))).map(|(texture, _)| (texture, None))
        };
        drawn.map_err(|e| match e {
            export::Failure::Empty => "nothing chosen shows anything to picture",
            export::Failure::NoRenderer => "the canvas isn't ready to draw yet",
            export::Failure::Still => "nothing chosen moves",
        })
    });
    let sheet_times = rendered.as_ref().and_then(|r| r.as_ref().ok()).and_then(|(_, times)| times.clone());
    let selected: Vec<(String, String, String)> = {
        let scene = b.doc.scene.borrow();
        ids.iter().filter_map(|id| scene.node(id).map(|n| (id.clone(), n.kind.name().to_string(), display_name(n)))).collect()
    };
    let title = b.doc.title();
    let design = b.doc.id;
    let page = b.page.borrow().clone();
    let page_name = b.page_name(&page);
    let (project, rev) = {
        let d = b.doc.design.borrow();
        (d.project_id, d.rev)
    };
    let weak = Rc::downgrade(b);
    let rt = ui.rt.clone();
    glib::spawn_future_local(async move {
        let picture: Result<Option<String>, String> = match rendered {
            None => Ok(None),
            Some(Err(why)) => Err(why.to_string()),
            Some(Ok((texture, _))) => {
                let path = glib::user_runtime_dir().join(format!("relay-design-ask-{}.png", uuid::Uuid::new_v4()));
                let p = path.clone();
                let written = rt.spawn_blocking(move || std::fs::write(&p, texture.save_to_png_bytes())).await;
                let stored = match written {
                    Ok(Ok(())) => match ui.call("design.asset.add", json!({"design_id": design, "path": path.to_string_lossy()})).await {
                        Ok(v) => serde_json::from_value::<AssetAddOut>(v).map(|o| Some(o.path)).map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    },
                    Ok(Err(e)) => Err(format!("could not write it: {e}")),
                    Err(e) => Err(format!("could not write it: {e}")),
                };
                let _ = std::fs::remove_file(&path);
                stored
            }
        };
        let message = prompt(&Prompt {
            title: &title,
            design,
            page: &page,
            page_name: &page_name,
            selected: &selected,
            picture: picture.as_ref().map(Option::as_deref).map_err(String::as_str),
            // The sheet's times once it is attached; else only that it moves.
            motion: moving.then(|| if matches!(picture, Ok(Some(_))) { sheet_times.as_deref().unwrap_or_default() } else { &[][..] }),
            request: &request,
        });
        // The terminal may have moved on while the picture was drawn and stored: read it again
        // just before typing, and go as Send's check would, mail or held (TM-1, TM-20).
        let checked = match way {
            Route::Mail => Ok((Route::Mail, None)),
            _ => match ui.call("session.list", json!({"project_id": project})).await {
                Ok(v) => {
                    let state = state_of(&v, &session);
                    recheck(&session, &state, || unsent_note(&session, &provider_of(&v, &session))).map(|way| (way, Some(state)))
                }
                Err(e) => Err(format!("Could not check {session}'s terminal: {e}.")),
            },
        };
        let way = match checked {
            Ok((way, state)) => {
                if let (Some(b), Some(state)) = (weak.upgrade(), state) {
                    b.set_lamp(&state);
                }
                way
            }
            Err(note) => {
                if let Some(b) = weak.upgrade() {
                    b.toast(&format!("{note} Your request is kept for the next Ask."));
                }
                return;
            }
        };
        // Ok(true): sent whole; Ok(false): typed, but its Enter did not go.
        let sent: Result<bool, String> = match way {
            Route::Mail => {
                let mail = json!({"project_id": project, "to": session, "text": message, "priority": true});
                ui.call("mailbox.send", mail).await.map(|_| true).map_err(|e| e.to_string())
            }
            // One paste, bracketed when the agent's terminal asked for it: its line breaks stay
            // text, and nothing in it reads as a key or a command (I-4, TM-20).
            _ => match ui.call("session.input", json!({"session": session, "data": message, "paste": true})).await {
                Ok(_) => {
                    // Enter on its own, after the paste has landed, to send it.
                    glib::timeout_future(Duration::from_millis(60)).await;
                    Ok(ui.call("session.input", json!({"session": session, "data": "\r"})).await.is_ok())
                }
                Err(e) => Err(e.to_string()),
            },
        };
        let Some(b) = weak.upgrade() else {
            return;
        };
        let without = picture.as_ref().err().map(|why| format!(" without a picture: {why}")).unwrap_or_default();
        match sent {
            Err(e) => return b.toast(&format!("Could not reach {session}: {e}. Your request is kept for the next Ask.")),
            Ok(false) => {
                // Sending it again would type it twice.
                clear_sent(&mut b.ask_draft.borrow_mut(), &request);
                return b.toast(&format!("{session} has the request typed but not sent: press Enter in its terminal"));
            }
            Ok(true) => {
                clear_sent(&mut b.ask_draft.borrow_mut(), &request);
                b.toast(&match way {
                    Route::Mail => format!("{session} is busy; it gets this with its next tool call{without}"),
                    _ => format!("Sent to {session}{without}. Its changes show here as it makes them."),
                });
            }
        }
        drop(b);
        glib::timeout_future(ANSWER_WAIT).await;
        if let Some(b) = weak.upgrade() {
            let changed = {
                let d = b.doc.design.borrow();
                answered(rev, d.rev, &d.updated_by, &session)
            };
            if !changed {
                no_change_yet(&b, &session);
            }
        }
    });
}

/// "No change from X yet" on the board's status line, with a key beside it that opens X's
/// terminal for as long as the status says so (E-ask-7). The line has no key of its own for
/// this, so the key is put after the status and taken out when the status changes.
fn no_change_yet(b: &Rc<Board>, session: &str) {
    const NAME: &str = "design-ask-open-terminal";
    let Some(line) = b.status.parent().and_downcast::<gtk::Box>() else {
        return;
    };
    // An earlier Ask's key, if its words are still up.
    let mut child = line.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        if c.widget_name() == NAME {
            line.remove(&c);
        }
    }
    b.status.set_text(&format!("No change from {session} yet"));
    let key = gtk::Button::with_label(&format!("Open {session}'s terminal"));
    key.add_css_class("design-agent-undo");
    key.set_widget_name(NAME);
    line.insert_child_after(&key, Some(&b.status));
    let (weak, name) = (Rc::downgrade(b), session.to_string());
    key.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            show_terminal(&b, &name);
        }
    });
    let handler: Rc<std::cell::Cell<Option<glib::SignalHandlerId>>> = Rc::default();
    let (h, k, l) = (handler.clone(), key.downgrade(), line.downgrade());
    handler.set(Some(b.status.connect_label_notify(move |status| {
        if let (Some(k), Some(l)) = (k.upgrade(), l.upgrade()) {
            l.remove(&k);
        }
        if let Some(id) = h.take() {
            status.disconnect(id);
        }
    })));
}

/// Whether the agent `session` has changed the design since it was asked at `asked` (its rev
/// then): it moved on, and `session` was the last to change it.
fn answered(asked: i64, rev: i64, updated_by: &str, session: &str) -> bool {
    rev > asked && updated_by == session
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask<'a>(selected: &'a [(String, String, String)], picture: Result<Option<&'a str>, &'a str>, request: &'a str) -> String {
        prompt(&Prompt { title: "Settings", design: 12, page: "page-7", page_name: "Flows", selected, picture, motion: None, request })
    }

    fn layer(id: &str, kind: &str, name: &str) -> (String, String, String) {
        (id.into(), kind.into(), name.into())
    }

    #[test]
    fn the_prompt_names_everything_by_id_on_one_line() {
        let selected = [layer("cta", "frame", "Save \"now\"")];
        let p = ask(&selected, Ok(Some("/data/design-assets/ab.png")), "Make it\nbigger");
        // I-4: only the request's own line breaks; it is pasted, so they do not send it.
        assert_eq!(p.matches('\n').count(), 1, "{p}");
        assert!(p.contains("design_id 12"));
        // TM-4: the page by name and by the id design.get takes; the read is of the selection.
        assert!(p.contains("page \"Flows\" (page-7)"), "{p}");
        assert!(p.contains("cta (frame \"Save 'now'\")"));
        // E-ask-4: the path is quoted, clear of the full stop.
        assert!(p.contains("Picture of it: \"/data/design-assets/ab.png\"."), "{p}");
        // E-ask-5 and I-4: its lines are kept, as lines.
        assert!(p.contains("Request: Make it\nbigger —"), "{p}");
        assert!(p.contains("design.get {\"design_id\":12,\"nodes\":[\"cta\"]}"), "{p}");
    }

    #[test]
    fn the_prompt_carries_no_keys_and_names_every_layer() {
        // B76: control bytes in a title, page or layer name would be keys in the terminal.
        let selected: Vec<_> = (0..10).map(|i| layer(&format!("n{i}"), "rect", &format!("Box\t{i}\u{1b}[A"))).collect();
        let p = prompt(&Prompt {
            title: "Set\ntings\u{1b}",
            design: 3,
            page: "p1",
            page_name: "P \"1\"\r",
            selected: &selected,
            picture: Ok(None),
            motion: None,
            request: "one\u{3}\n\n\ttwo\u{9b}",
        });
        assert!(!p.chars().any(|c| c.is_control() && c != '\n'), "{p:?}");
        assert!(p.contains("\"Set tings\"") && p.contains("page \"P '1'\" (p1)"), "{p}");
        assert!((0..10).all(|i| p.contains(&format!("n{i}"))), "every id: {p}");
        assert!(p.contains("n0 (rect \"Box 0 [A\")") && p.contains(", also n8, n9."), "{p}");
        // Blank lines run together; only line breaks survive of the controls.
        assert!(p.contains("Request: one\n\ntwo —"), "{p}");
        assert_eq!(clean_request("\n\n a \n\n\n\tb\u{1b}\n\n"), "a\n\nb");
        assert!(p.contains("\"nodes\":[\"n0\",\"n1\",\"n2\",\"n3\",\"n4\",\"n5\",\"n6\",\"n7\",\"n8\",\"n9\"]"));
    }

    #[test]
    fn the_prompt_says_when_it_is_about_the_whole_page() {
        // E-ask-6 and I-1: nothing selected is the page, read a page at a time.
        let p = ask(&[], Ok(Some("/a/p.png")), "Tidy");
        assert!(p.contains("Nothing selected; picture of the whole page page-7: \"/a/p.png\"."), "{p}");
        assert!(p.contains("design.get {\"design_id\":12,\"page\":\"page-7\"}"), "{p}");
        assert!(ask(&[], Ok(None), "Tidy").contains("Nothing selected: about the whole page page-7."));
    }

    #[test]
    fn the_prompt_says_when_its_picture_is_missing() {
        // B77: a picture asked for and not attached is said, not left out.
        let selected = [layer("cta", "frame", "Save")];
        let p = ask(&selected, Err("nothing chosen shows anything to picture"), "Fix it");
        assert!(p.contains("(A picture was requested but could not be attached: nothing chosen shows anything to picture.)"), "{p}");
        assert!(!p.contains("Picture of it"));
        assert!(!ask(&selected, Ok(None), "Fix it").contains("picture"));
    }

    #[test]
    fn the_prompt_says_when_it_moves() {
        // feature-motion §3: Ask attaches a contact sheet of what moves, and says how to read it.
        let selected = [layer("hero", "frame", "Hero")];
        let times = [0.0, 600.0, 1200.0, 1800.0, 2400.0, 3000.0];
        let with = |picture, motion| prompt(&Prompt { title: "Settings", design: 12, page: "page-7", page_name: "Flows", selected: &selected, picture, motion, request: "Slower" });
        let p = with(Ok(Some("/a/sheet.png")), Some(&times[..]));
        assert!(p.contains("Picture of it: \"/a/sheet.png\"."), "{p}");
        assert!(p.contains(" It moves: the picture is a contact sheet of 6 frames at 0, 600, 1200, 1800, 2400 and 3000 ms, left to right then down."), "{p}");
        // The note comes before the request, which stays last.
        assert!(p.find("It moves").unwrap() < p.find("Request: Slower").unwrap(), "{p}");
        // Odd times are said to the ms.
        let thirds = [0.0, 333.333_333, 666.666_667, 1000.0];
        assert!(with(Ok(Some("/a/s.png")), Some(&thirds[..])).contains("4 frames at 0, 333, 667 and 1000 ms"));
        // Moving with no sheet (none asked for, or it could not be drawn): how to see it.
        for missing in [Ok(None), Err("the canvas isn't ready to draw yet")] {
            let p = with(missing, Some(&[][..]));
            assert!(p.contains("It moves: design.render with \"frames\""), "{p}");
            assert!(!p.contains("contact sheet of"), "{p}");
        }
        // Still: not a word of motion.
        assert!(!with(Ok(Some("/a/p.png")), None).contains("moves"));
        assert!(!p.chars().any(char::is_control));
    }

    #[test]
    fn only_an_agent_at_its_prompt_or_at_work_is_typed_to() {
        // TM-1: a digit of the prompt answers a permission dialog.
        assert!(typable("idle") && typable("running"));
        assert!(!typable("blocked") && !typable("spawning") && !typable("parked") && !typable("closed"));
        assert_eq!(route("idle"), Route::Type);
        assert_eq!(route("running"), Route::Mail, "I-9: mail, read with its next tool call");
        assert_eq!(route("blocked"), Route::Blocked);
        assert_eq!(route("spawning"), Route::Starting);
        assert_eq!(route("exited"), Route::Gone);
        assert!(hold_note("ada", Route::Blocked).unwrap().contains("waiting on you"));
        assert!(hold_note("ada", Route::Type).is_none());
    }

    #[test]
    fn the_picker_says_where_each_terminal_is() {
        // E-ask-1 and E-ask-2.
        let mut states = vec!["blocked", "spawning", "running", "idle"];
        states.sort_by_key(|s| rank(s));
        assert_eq!(states, ["idle", "running", "blocked", "spawning"]);
        assert_eq!(gone_heading("ada", "parked"), "ada is parked.");
        assert_eq!(gone_heading("ada", "created"), "ada has not started.");
        assert_eq!(gone_heading("ada", "exited"), "ada's terminal has closed.");
        assert!(wakeable("parked") && wakeable("restorable") && !wakeable("exited"));
        assert_eq!(state_note("blocked"), Some("waiting on you"));
        let listed = json!({"sessions": [{"name": "ada", "state": "idle"}]});
        assert_eq!((state_of(&listed, "ada"), state_of(&listed, "bo")), ("idle".to_string(), "closed".to_string()));
    }

    #[test]
    fn an_ask_is_answered_by_the_agents_own_change() {
        // E-ask-7.
        assert!(answered(4, 5, "ada", "ada"));
        assert!(!answered(4, 4, "ada", "ada"));
        assert!(!answered(4, 6, "user", "ada"));
    }

    /// The check just before typing routes as Send's own: an agent gone to work gets mail, one
    /// at a dialog, starting or gone holds it, and text left at the prompt holds it too.
    #[test]
    fn the_last_check_routes_as_the_first() {
        let none = || None;
        assert_eq!(recheck("dev", "idle", none), Ok(Route::Type));
        assert_eq!(recheck("dev", "running", || panic!("no prompt to read for mail")), Ok(Route::Mail));
        assert!(recheck("dev", "blocked", none).is_err_and(|n| n.contains("waiting on you")));
        assert!(recheck("dev", "spawning", none).is_err_and(|n| n.contains("still starting")));
        assert_eq!(recheck("dev", "closed", none), Err("dev's terminal has closed.".into()));
        assert_eq!(recheck("dev", "idle", || Some("left".into())), Err("left".into()));
    }

    /// A finished Ask empties the draft only while it still holds what was sent.
    #[test]
    fn a_finished_ask_keeps_a_draft_typed_since() {
        let mut draft = "Make it bigger \n".to_string();
        clear_sent(&mut draft, "Make it bigger");
        assert_eq!(draft, "");
        let mut draft = "And bluer".to_string();
        clear_sent(&mut draft, "Make it bigger");
        assert_eq!(draft, "And bluer");
    }
}
