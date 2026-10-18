//! Full access in a thread (docs/THREADS.md, "Full access"): turning it on, which asks first, and
//! what its agent then asks to do, each on a card that waits for the person's answer.
//!
//! The cards sit in a tray over the open thread's message box, oldest first. They are the
//! engine's reading (`thread.ask.list`), read when the thread shows and again on `thread.ask` and
//! `thread.changed`, so an answer given on the phone clears them here too. What the agent asked
//! to run shows exactly: the command or the path in Geist Mono, a hidden character spelled out,
//! an edit's text before and after. An answered or cancelled ask leaves the tray; what came of it
//! shows in the conversation as the tool's answer.

use crate::app::{button, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The tallest the tray grows over the message box before it scrolls.
const TRAY_MAX: i32 = 320;
/// The most of a file's text a card shows, before and after an edit or at a write's start.
const SHOWN_LINES: usize = 6;
const SHOWN_CHARS: usize = 600;
/// How many of a multiple edit's changes a card shows.
const SHOWN_EDITS: usize = 2;
/// How long a new card's keys stay off, as a held action's prompt's do: it arrives unannounced,
/// under wherever the pointer was aimed.
const ARM: std::time::Duration = std::time::Duration::from_millis(500);
/// What full access gives, in the confirmation and on the model picker's key.
pub(super) const FULL_ABOUT: &str = "Its agent gets a shell, your files, the web and all of Relay on this PC. It asks you before each thing it does that changes anything, and runs what you allow as you.";

/// The tray: its scroller, the box of cards in it, the cards by ask id, and the newest read's
/// serial (only it draws).
#[derive(Default)]
struct Tray {
    scroll: Option<gtk::ScrolledWindow>,
    list: Option<gtk::Box>,
    cards: HashMap<i64, gtk::Box>,
    serial: u64,
}

thread_local! {
    static TRAY: RefCell<Tray> = RefCell::default();
}

/// Turning full access on asks first, in the window: what the agent gets, and that each change
/// waits for the person. True once they give it.
pub(super) async fn confirm(ui: &Rc<Ui>) -> bool {
    let panel = crate::panel::Panel::new(ui, "Give this thread full access?", 440);
    panel.add_css_class("money-sheet");
    let about = label(FULL_ABOUT, "money-muted");
    about.set_wrap(true);
    panel.body.append(&about);
    // `Panel::response`'s keys, in this question's words.
    let (send, recv) = async_channel::bounded(1);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    keys.set_halign(gtk::Align::End);
    let not_now = button("Not now", "quiet");
    let give = button("Give full access", "primary");
    give.set_widget_name("panel-accept");
    keys.append(&not_now);
    keys.append(&give);
    panel.body.append(&keys);
    let weak = Rc::downgrade(&panel);
    not_now.connect_clicked(move |_| {
        if let Some(panel) = weak.upgrade() {
            panel.close();
        }
    });
    let given = send.clone();
    give.connect_clicked(move |_| {
        let _ = given.try_send(true);
    });
    panel.on_closed(move || {
        let _ = send.try_send(false);
    });
    panel.compact(true, 280);
    panel.present();
    // A yes or no starts on the key that changes nothing.
    not_now.grab_focus();
    let given = recv.recv().await.unwrap_or(false);
    panel.close();
    given
}

/// What a refusal says, in the engine's words.
pub(super) fn said(e: &crate::client::Error) -> String {
    match e {
        crate::client::Error::Bus(b) => b.message.clone(),
        other => other.to_string(),
    }
}

/// The tray, built once by the page and put over the message box: hidden while nothing waits.
pub(super) fn tray() -> gtk::ScrolledWindow {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    list.add_css_class("threads-asks");
    let scroll = gtk::ScrolledWindow::builder().child(&list).propagate_natural_height(true).max_content_height(TRAY_MAX).build();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_visible(false);
    TRAY.with(|t| {
        let mut t = t.borrow_mut();
        t.scroll = Some(scroll.clone());
        t.list = Some(list);
    });
    scroll
}

fn parts() -> Option<(gtk::ScrolledWindow, gtk::Box)> {
    TRAY.with(|t| {
        let t = t.borrow();
        t.scroll.clone().zip(t.list.clone())
    })
}

fn bump() -> u64 {
    TRAY.with(|t| {
        let mut t = t.borrow_mut();
        t.serial += 1;
        t.serial
    })
}

/// The open thread as the engine just said it (`thread.get`, `thread.changed`): one with full
/// access, or with asks still waiting, reads them; any other has none.
pub(super) fn shown(thread: &Value) {
    let Some(id) = thread["id"].as_i64() else { return };
    if super::threads::current() != Some(id) {
        return;
    }
    if thread["access"] == "full" || thread["asking"].as_u64().is_some_and(|n| n > 0) {
        refresh();
    } else {
        forget();
    }
}

/// Read the open thread's waiting asks and draw them; with no thread open the tray empties.
fn refresh() {
    let serial = bump();
    let (Some(ui), Some(thread)) = (super::threads::the_ui_pub(), super::threads::current()) else {
        draw(&[]);
        return;
    };
    glib::spawn_future_local(async move {
        let read = ui.call("thread.ask.list", json!({"thread_id": thread, "pending": true})).await;
        // Another thread showing, or a newer read sent, since this one left: it is not theirs.
        if TRAY.with(|t| t.borrow().serial) != serial || super::threads::current() != Some(thread) {
            return;
        }
        // Unread, the tray stays as it was until the next event reads it again.
        if let Ok(read) = read {
            let asks: &[Value] = read["asks"].as_array().map_or(&[], Vec::as_slice);
            draw(asks);
        }
    });
}

/// Another thread shows, or a new one: the tray empties, and a read still on its way for the
/// last one draws nothing.
pub(super) fn forget() {
    bump();
    draw(&[]);
}

/// `thread.ask`: an ask made, answered or cancelled, in any thread. One that no longer waits
/// leaves the tray at once; the open thread's are read again.
pub(super) fn changed(ask: &Value) {
    if ask["status"] != "pending" {
        if let Some(id) = ask["id"].as_i64() {
            drop_card(id);
        }
    }
    if ask["thread_id"].as_i64().is_some_and(|t| super::threads::current() == Some(t)) {
        refresh();
    }
}

/// Show the `asks` that still wait, oldest first, a card each. A card already showing stays as
/// it is, so a key just pressed stays pressed; one no longer listed goes.
fn draw(asks: &[Value]) {
    let Some((_, list)) = parts() else { return };
    let mut wanted: Vec<gtk::Box> = Vec::new();
    for ask in asks.iter().filter(|a| a["status"] == "pending") {
        let Some(id) = ask["id"].as_i64() else { continue };
        let kept = TRAY.with(|t| t.borrow().cards.get(&id).cloned());
        let card = kept.unwrap_or_else(|| {
            let card = ask_card(id, ask);
            TRAY.with(|t| t.borrow_mut().cards.insert(id, card.clone()));
            card
        });
        wanted.push(card);
    }
    TRAY.with(|t| t.borrow_mut().cards.retain(|_, card| wanted.contains(card)));
    let mut previous: Option<gtk::Widget> = None;
    for card in &wanted {
        let w = card.upcast_ref::<gtk::Widget>();
        if w.parent().as_ref() == Some(list.upcast_ref()) {
            if w.prev_sibling() != previous {
                list.reorder_child_after(w, previous.as_ref());
            }
        } else {
            list.insert_child_after(w, previous.as_ref());
        }
        previous = Some(w.clone());
    }
    while let Some(extra) = previous.as_ref().map_or_else(|| list.first_child(), |w| w.next_sibling()) {
        list.remove(&extra);
    }
    settle();
}

/// Take ask `id`'s card out of the tray.
fn drop_card(id: i64) {
    let Some(card) = TRAY.with(|t| t.borrow_mut().cards.remove(&id)) else { return };
    if let Some(list) = card.parent().and_downcast::<gtk::Box>() {
        list.remove(&card);
    }
    settle();
}

/// The tray shows while a card is in it, as tall as its cards up to [`TRAY_MAX`].
fn settle() {
    let Some((scroll, list)) = parts() else { return };
    let any = list.first_child().is_some();
    scroll.set_visible(any);
    if any {
        fit();
    }
}

/// Size the tray to its cards at the width they have, once laid out: a scroller measures what it
/// holds at no particular width, so a wrapped line would count as one and the keys under it fall
/// out of view (as `Panel::fit` says). The page calls it again when its width changes.
pub(super) fn fit() {
    glib::idle_add_local_once(|| {
        let Some((scroll, list)) = parts().filter(|(scroll, _)| scroll.is_visible()) else { return };
        let width = scroll.parent().map_or(0, |p| p.width());
        if width <= 0 {
            return;
        }
        let (_, natural, _, _) = list.measure(gtk::Orientation::Vertical, width);
        scroll.set_min_content_height(natural.min(TRAY_MAX));
    });
}

/// What the agent asks to do: what, exactly, and why; for an edit or a write the text; then the
/// person's three answers.
fn ask_card(id: i64, ask: &Value) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    // A proposal's asking card (`threads_ask.rs`): the same amber edge and pill while it waits.
    for class in ["threads-card", "arbiter-proposal", "arbiter-asking", "threads-ask-card"] {
        card.add_css_class(class);
    }
    card.set_widget_name(&format!("thread-ask-{id}"));
    // Said once, under the title, when anything the agent wrote carries a hidden character.
    let flag = words("Contains hidden characters, shown as <U+…>: what it does may read differently from this.", "threads-ask-hidden");
    flag.set_visible(false);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = words(&spelled(ask["title"].as_str().unwrap_or("Use a tool"), &flag), "threads-card-title");
    title.set_hexpand(true);
    top.append(&title);
    let pill = label("Needs your approval", "arbiter-pill");
    pill.add_css_class("arbiter-paper");
    pill.set_valign(gtk::Align::Start);
    top.append(&pill);
    card.append(&top);
    card.append(&flag);
    if let Some(detail) = ask["detail"].as_str().filter(|d| !d.trim().is_empty()) {
        card.append(&words(&spelled(detail, &flag), "threads-ask-detail"));
    }
    if let Some(reason) = ask["reason"].as_str().map(str::trim).filter(|r| !r.is_empty()) {
        card.append(&words(&spelled(reason, &flag), "threads-ask-reason"));
    }
    let (blocks, more) = texts(ask);
    for (caption, text) in blocks {
        let block = gtk::Box::new(gtk::Orientation::Vertical, 4);
        if !caption.is_empty() {
            block.append(&label(caption, "threads-ask-caption"));
        }
        let body = if text.is_empty() { String::from("Empty") } else { spelled(&cut(&text), &flag) };
        block.append(&words(&body, "threads-code"));
        card.append(&block);
    }
    if more > 0 {
        card.append(&label(&format!("And {more} more {} in this file", if more == 1 { "change" } else { "changes" }), "threads-ask-reason"));
    }

    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    keys.set_halign(gtk::Align::End);
    let deny = button("Deny", "");
    deny.set_widget_name(&format!("thread-ask-deny-{id}"));
    let always = button(&for_thread(ask), "");
    always.set_widget_name(&format!("thread-ask-thread-{id}"));
    let once = button("Allow once", "primary");
    once.set_widget_name(&format!("thread-ask-allow-{id}"));
    let problem = words("", "threads-error");
    problem.set_visible(false);
    for (key, allow, scope) in [(&deny, false, None), (&always, true, Some("thread")), (&once, true, Some("once"))] {
        keys.append(key);
        let (weak_keys, weak_problem) = (keys.downgrade(), problem.downgrade());
        key.connect_clicked(move |_| {
            if let (Some(keys), Some(problem)) = (weak_keys.upgrade(), weak_problem.upgrade()) {
                answer(id, allow, scope, &keys, &problem);
            }
        });
    }
    card.append(&keys);
    card.append(&problem);
    keys.set_sensitive(false);
    let armed = keys.downgrade();
    glib::timeout_add_local_once(ARM, move || {
        if let Some(keys) = armed.upgrade() {
            keys.set_sensitive(true);
        }
    });
    card
}

/// A wrapped line of a card: the mouse selects and copies it, Tab passes it by.
fn words(text: &str, class: &str) -> gtk::Label {
    let l = label(text, class);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_selectable(true);
    // After `selectable`, which makes it a Tab stop.
    l.set_focusable(false);
    l
}

/// The text an edit puts in place of the old, or a write puts in its file: (caption, text) each,
/// and how many more changes a multiple edit makes than those shown.
fn texts(ask: &Value) -> (Vec<(&'static str, String)>, usize) {
    let input = &ask["input"];
    let text = |v: &Value| v.as_str().unwrap_or("").to_string();
    match ask["tool"].as_str().unwrap_or("") {
        "Edit" => (vec![("Before", text(&input["old_string"])), ("After", text(&input["new_string"]))], 0),
        "MultiEdit" => {
            let edits: &[Value] = input["edits"].as_array().map_or(&[], Vec::as_slice);
            let pairs = edits.iter().take(SHOWN_EDITS).flat_map(|e| [("Before", text(&e["old_string"])), ("After", text(&e["new_string"]))]).collect();
            (pairs, edits.len().saturating_sub(SHOWN_EDITS))
        }
        "Write" => (vec![("", text(&input["content"]))], 0),
        _ => (Vec::new(), 0),
    }
}

/// The key that allows this tool for the rest of the thread, saying what it allows.
fn for_thread(ask: &Value) -> String {
    if let Some(op) = ask["op"].as_str().filter(|op| !op.is_empty()) {
        return format!("Allow {op} in this thread");
    }
    match ask["tool"].as_str() {
        Some("Bash") => String::from("Allow commands in this thread"),
        _ => String::from("Allow for this thread"),
    }
}

/// Send the person's answer to ask `id`; its card goes once the engine has it. A refusal says why
/// on the card; one because the ask was answered already (on the phone) or cancelled reads the
/// tray again instead.
fn answer(id: i64, allow: bool, scope: Option<&'static str>, keys: &gtk::Box, problem: &gtk::Label) {
    let Some(ui) = super::threads::the_ui_pub() else { return };
    keys.set_sensitive(false);
    let mut payload = json!({"id": id, "allow": allow});
    if let Some(scope) = scope {
        payload["scope"] = json!(scope);
    }
    let (keys, problem) = (keys.downgrade(), problem.downgrade());
    glib::spawn_future_local(async move {
        let refused = match ui.call("thread.ask.answer", payload).await {
            Ok(_) => {
                drop_card(id);
                return;
            }
            Err(e) => e,
        };
        if let Some(keys) = keys.upgrade() {
            keys.set_sensitive(true);
        }
        if matches!(&refused, crate::client::Error::Bus(b) if matches!(b.code.as_str(), "thread.ask_answered" | "thread.ask_not_found")) {
            refresh();
            return;
        }
        if let Some(problem) = problem.upgrade() {
            problem.set_text(&said(&refused));
            problem.set_visible(true);
        }
    });
}

/// `text` as a card shows a file's: its first [`SHOWN_LINES`] lines and [`SHOWN_CHARS`]
/// characters, and how many lines were left out.
fn cut(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = lines.iter().take(SHOWN_LINES).copied().collect::<Vec<_>>().join("\n");
    if let Some((at, _)) = out.char_indices().nth(SHOWN_CHARS) {
        out.truncate(at);
        out.push('…');
    }
    if lines.len() > SHOWN_LINES {
        let more = lines.len() - SHOWN_LINES;
        out.push_str(&format!("\n… {more} more {}", if more == 1 { "line" } else { "lines" }));
    }
    out
}

/// Agent-supplied `text` as the card shows it, its hidden characters spelled out, raising the
/// card's `flag` when there were any: Pango applies a bidi override and draws a zero-width
/// character as nothing, so the command read could otherwise differ from the one allowed.
fn spelled(text: &str, flag: &gtk::Label) -> String {
    let (text, hidden) = reveal(text);
    if hidden > 0 {
        flag.set_visible(true);
    }
    text
}

/// `value` with each hidden character replaced by its code (`<U+202E>`), and how many there were.
fn reveal(value: &str) -> (String, usize) {
    let mut out = String::with_capacity(value.len());
    let mut hidden = 0;
    for c in value.chars() {
        if is_hidden(c) {
            out.push_str(&format!("<U+{:04X}>", c as u32));
            hidden += 1;
        } else {
            out.push(c);
        }
    }
    (out, hidden)
}

/// A character that changes how text reads without showing itself, as a held action's card
/// finds them (`guardrail_pages.rs`): bidi embeddings, overrides, isolates and marks, zero-width
/// and other invisible format characters, the separators a label breaks lines on, and every
/// control character but a newline and a tab, which a file's text is full of.
fn is_hidden(c: char) -> bool {
    matches!(c,
        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
        | '\u{200B}'..='\u{200D}' | '\u{2060}'..='\u{2064}' | '\u{206A}'..='\u{206F}' | '\u{FEFF}'
        | '\u{00AD}' | '\u{034F}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}' | '\u{180B}'..='\u{180F}'
        | '\u{2028}' | '\u{2029}' | '\u{3164}' | '\u{FFA0}' | '\u{FE00}'..='\u{FE0F}' | '\u{FFF9}'..='\u{FFFB}'
        | '\u{E0000}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}')
        || (c.is_control() && c != '\n' && c != '\t')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_character_is_spelled_out() {
        assert_eq!(reveal("cat notes\u{202E}txt.sh"), (String::from("cat notes<U+202E>txt.sh"), 1));
        assert_eq!(reveal("make\tall\n"), (String::from("make\tall\n"), 0));
    }

    #[test]
    fn long_text_keeps_its_first_lines_and_says_how_many_more() {
        let text = (1..=9).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
        assert_eq!(cut(&text), "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\n… 3 more lines");
        assert_eq!(cut("short"), "short");
    }

    #[test]
    fn the_thread_key_says_what_it_allows() {
        assert_eq!(for_thread(&json!({"tool": "Bash"})), "Allow commands in this thread");
        assert_eq!(for_thread(&json!({"tool": "mcp__relay__task_create", "op": "task.create"})), "Allow task.create in this thread");
        assert_eq!(for_thread(&json!({"tool": "Edit", "op": null})), "Allow for this thread");
    }

    #[test]
    fn an_edit_shows_its_text_before_and_after() {
        let edit = json!({"tool": "Edit", "input": {"file_path": "/a", "old_string": "x = 1", "new_string": "x = 2"}});
        assert_eq!(texts(&edit), (vec![("Before", String::from("x = 1")), ("After", String::from("x = 2"))], 0));
        let edits: Vec<Value> = (0..4).map(|n| json!({"old_string": n.to_string(), "new_string": (n + 1).to_string()})).collect();
        let multi = json!({"tool": "MultiEdit", "input": {"file_path": "/a", "edits": edits}});
        let (blocks, more) = texts(&multi);
        assert_eq!((blocks.len(), more), (4, 2));
        assert!(texts(&json!({"tool": "Bash", "input": {"command": "ls"}})).0.is_empty());
    }
}
