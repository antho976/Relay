//! Asking first in a thread (docs/THREADS.md, phase 4): what the agent proposed
//! (`thread.propose`) as a card in the conversation, with the act named on its key and "Not now".
//!
//! Everything on the card is the engine's reading of the ledger (`thread.proposal.*`): the rows
//! are the real entries, budgets and accounts, drawn as Tally draws them, so a proposal to delete
//! "a duplicate coffee" whose id is the rent shows the rent. The agent's words appear only as its
//! quoted reason. The card's state is the proposal's, read again whenever the thread opens, so an
//! answer stays answered.
//!
//! A row that changed or went since the agent asked says so, and the key waits until the person
//! looks again ("Review again"); approving then binds to that reading (`seen`).

use super::pages::{badge_sized, human_date, icon_row, tile};
use crate::app::{button, clear, label};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_money::copy::plural_as;
use relay_money::money::{Locale, MoneyFormatter};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

thread_local! {
    /// The cards on screen, by proposal id. One proposal asked twice is one card drawn twice.
    static CARDS: RefCell<Vec<(i64, glib::WeakRef<gtk::Box>)>> = const { RefCell::new(Vec::new()) };
    /// What the person read again after a proposal changed: the reading's `seen`, by proposal.
    static REVIEWED: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::new());
    /// A read of the thread's proposals is on its way: cards asking meanwhile share it.
    static READING: Cell<bool> = const { Cell::new(false) };
}

/// The card for proposal `id`, filled once the engine has read it.
pub(super) fn card(id: i64) -> gtk::Box {
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    slot.append(&label("Reading the ledger…", "threads-card-detail"));
    CARDS.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(_, w)| w.upgrade().is_some());
        c.push((id, slot.downgrade()));
    });
    refresh();
    slot
}

/// Read the open thread's proposals again and redraw their cards: on opening, and whenever the
/// ledger moves under a card that waits.
pub(super) fn refresh() {
    if CARDS.with(|c| c.borrow().iter().all(|(_, w)| w.upgrade().is_none())) {
        return;
    }
    if READING.with(|r| r.replace(true)) {
        return;
    }
    glib::idle_add_local_once(|| {
        let (Some(ui), Some(thread)) = (super::threads::the_ui_pub(), super::threads::current()) else {
            READING.with(|r| r.set(false));
            return;
        };
        glib::spawn_future_local(async move {
            let read = ui.call("thread.proposal.list", json!({"thread_id": thread})).await;
            READING.with(|r| r.set(false));
            let Ok(read) = read else { return };
            for p in read["proposals"].as_array().into_iter().flatten() {
                show(p);
            }
        });
    });
}

/// `thread.proposal`: one was made or answered, here or anywhere.
pub(super) fn changed(p: &Value) {
    show(p);
}

fn show(p: &Value) {
    let Some(id) = p["id"].as_i64() else { return };
    let slots: Vec<gtk::Box> = CARDS.with(|c| c.borrow().iter().filter(|(i, _)| *i == id).filter_map(|(_, w)| w.upgrade()).collect());
    for slot in slots {
        fill(&slot, p);
    }
}

/// The op a proposal runs, and how many it holds.
fn act_of(p: &Value) -> (String, usize) {
    let Some((op, list)) = p["action"].as_object().and_then(|a| a.iter().next()) else { return (String::new(), 0) };
    (op.clone(), list.as_array().map_or(0, Vec::len))
}

fn items(p: &Value) -> Vec<Value> {
    p["items"].as_array().cloned().unwrap_or_default()
}

/// The act in words, now and done ("Delete 2 entries", "Deleted 2 entries"), and whether it
/// removes something, which its key says in the destructive style.
fn act(p: &Value) -> (String, String, bool) {
    let (op, n) = act_of(p);
    let n64 = n as i64;
    let items = items(p);
    let entries = plural_as(n64, "entry", "entries");
    match op.as_str() {
        "money.tx.delete" => (format!("Delete {entries}"), format!("Deleted {entries}"), true),
        "money.invest.delete" => {
            let what = plural_as(n64, "activity", "activities");
            (format!("Delete {what}"), format!("Deleted {what}"), true)
        }
        "money.tx.update" => {
            // Only the category moves: a move, as the person would call it.
            let moves = items.iter().all(|i| {
                let (was, now) = (&i["entry"], &i["entry_after"]);
                !now.is_object() || ["type", "amount", "date", "account_id", "to_account_id", "note"].iter().all(|k| was[k] == now[k])
            });
            if moves { (format!("Move {entries}"), format!("Moved {entries}"), false) } else { (format!("Change {entries}"), format!("Changed {entries}"), false) }
        }
        "money.budget.set" => {
            let budgets = plural_as(n64, "budget", "budgets");
            if items.iter().all(|i| i["budget_after"].is_null()) {
                (format!("Remove {budgets}"), format!("Removed {budgets}"), true)
            } else {
                (format!("Set {budgets}"), format!("Set {budgets}"), false)
            }
        }
        "money.account.add" => {
            let accounts = plural_as(n64, "account", "accounts");
            (format!("Add {accounts}"), format!("Added {accounts}"), false)
        }
        "money.account.update" => {
            let accounts = plural_as(n64, "account", "accounts");
            if items.iter().all(|i| i["account_after"]["archived"] == true && i["account"]["archived"] == false) {
                (format!("Archive {accounts}"), format!("Archived {accounts}"), true)
            } else {
                (format!("Change {accounts}"), format!("Changed {accounts}"), false)
            }
        }
        _ => (String::from("Approve"), String::from("Done"), false),
    }
}

fn pill(words: &str, tone: &str) -> gtk::Label {
    let l = label(words, "arbiter-pill");
    if !tone.is_empty() {
        l.add_css_class(tone);
    }
    l.set_valign(gtk::Align::Start);
    l
}

fn fill(slot: &gtk::Box, p: &Value) {
    clear(slot);
    let id = p["id"].as_i64().unwrap_or(0);
    let status = p["status"].as_str().unwrap_or("");
    let pending = status == "pending";
    let (now_words, done_words, destructive) = act(p);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("threads-card");
    card.add_css_class("arbiter-proposal");
    if pending {
        card.add_css_class("arbiter-asking");
    }
    if status == "undone" || status == "declined" {
        card.add_css_class("threads-card-undone");
    }
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = label(if status == "approved" { &done_words } else { &now_words }, "threads-card-title");
    title.set_wrap(true);
    title.set_hexpand(true);
    top.append(&title);
    top.append(&match status {
        "pending" => pill("Needs your approval", "arbiter-paper"),
        "running" => pill("Applying…", "arbiter-paper"),
        "approved" => pill("Done", "arbiter-live"),
        "failed" => pill("Failed", "arbiter-halted"),
        "undone" => pill("Undone", ""),
        _ => pill("Not done", ""),
    });
    card.append(&top);

    let (op, _) = act_of(p);
    let fmt = MoneyFormatter::new(p["currency"].as_str().filter(|c| !c.is_empty()).unwrap_or("CAD"), Locale::from_env());
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 2);
    rows.add_css_class("threads-ask-rows");
    for item in items(p) {
        rows.append(&row(&op, &item, &fmt));
        // Only a card that waits says what moved since the agent asked: an answered one shows
        // the ledger as it was when it was answered.
        let note = match item["state"].as_str().filter(|_| pending) {
            Some("changed") => Some(("Changed since Claude asked", "threads-ask-changed")),
            Some("gone") => Some(("Already gone", "threads-ask-gone")),
            _ => None,
        };
        if let Some((words, class)) = note {
            let l = label(words, class);
            l.set_margin_start(40);
            rows.append(&l);
        }
    }
    card.append(&rows);
    if let Some(why) = p["why"].as_str().map(str::trim).filter(|w| !w.is_empty()) {
        let l = label(&format!("\u{201c}{why}\u{201d} \u{2014} Claude"), "threads-ask-why");
        l.set_wrap(true);
        l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        card.append(&l);
    }

    let problem = label("", "threads-error");
    problem.set_wrap(true);
    problem.set_visible(false);
    match status {
        "pending" => {
            let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let primary = button(&now_words, if destructive { "money-destructive" } else { "primary" });
            primary.set_widget_name(&format!("thread-approve-{id}"));
            let not_now = button("Not now", "quiet");
            keys.append(&primary);
            keys.append(&not_now);
            let seen = p["seen"].as_str().unwrap_or("").to_string();
            let gone = items(p).iter().any(|i| i["state"] == "gone");
            let reviewed = REVIEWED.with(|r| r.borrow().get(&id) == Some(&seen));
            if gone {
                primary.set_sensitive(false);
                let l = label("Some of this is no longer in the ledger: choose Not now, and ask again.", "threads-card-detail");
                l.set_wrap(true);
                card.append(&keys);
                card.append(&l);
            } else if p["stale"] == true && !reviewed {
                primary.set_sensitive(false);
                let again = button("Review again", "quiet");
                keys.append(&again);
                card.append(&keys);
                let s = slot.downgrade();
                again.connect_clicked(move |key| {
                    let Some(ui) = super::threads::the_ui_pub() else { return };
                    key.set_sensitive(false);
                    let s = s.clone();
                    glib::spawn_future_local(async move {
                        // What the person looks at now is what they approve.
                        if let Ok(fresh) = ui.call("thread.proposal.get", json!({"id": id})).await {
                            REVIEWED.with(|r| r.borrow_mut().insert(id, fresh["seen"].as_str().unwrap_or("").to_string()));
                            if let Some(slot) = s.upgrade() {
                                fill(&slot, &fresh);
                            }
                        }
                    });
                });
            } else {
                card.append(&keys);
            }
            for (key, approve) in [(primary, true), (not_now, false)] {
                let (keys, problem, seen) = (keys.clone(), problem.clone(), seen.clone());
                key.connect_clicked(move |_| {
                    let payload = if approve { json!({"id": id, "approve": true, "seen": seen}) } else { json!({"id": id, "approve": false}) };
                    answer(&keys, &problem, "thread.proposal.resolve", payload);
                });
            }
        }
        "approved" => {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let done = label(p["outcome"].as_str().unwrap_or("Done."), "threads-card-detail");
            done.set_hexpand(true);
            done.set_wrap(true);
            line.append(&done);
            if p["undoable"] == true {
                let undo = button("Undo", "quiet");
                undo.add_css_class("threads-undo");
                line.append(&undo);
                let (line_, problem_) = (line.clone(), problem.clone());
                undo.connect_clicked(move |_| answer(&line_, &problem_, "thread.proposal.undo", json!({"id": id})));
            }
            card.append(&line);
        }
        "failed" => {
            let l = label(&format!("Could not be done, so nothing changed: {}", p["outcome"].as_str().unwrap_or("no reason given")), "threads-error");
            l.set_wrap(true);
            card.append(&l);
        }
        "undone" => card.append(&label("Put back as it was.", "threads-card-detail")),
        "running" => card.append(&label("Applying it now.", "threads-card-detail")),
        _ => card.append(&label("Not done: nothing changed.", "threads-card-detail")),
    }
    card.append(&problem);
    slot.append(&card);
}

/// Send the person's answer; the card redraws from what the engine says came of it. A refusal
/// says why; one because the proposal moved (it changed since they read it, it went, it was
/// answered elsewhere) reads it again, and the card redrawn says what moved.
fn answer(keys: &gtk::Box, problem: &gtk::Label, op: &'static str, payload: Value) {
    let Some(ui) = super::threads::the_ui_pub() else { return };
    keys.set_sensitive(false);
    let (keys, problem) = (keys.clone(), problem.clone());
    glib::spawn_future_local(async move {
        match ui.call(op, payload).await {
            Ok(view) => show(&view),
            Err(e) => {
                let (code, said) = match &e {
                    crate::client::Error::Bus(b) => (b.code.clone(), b.message.clone()),
                    other => (String::new(), other.to_string()),
                };
                problem.set_text(&said);
                problem.set_visible(true);
                keys.set_sensitive(true);
                if matches!(code.as_str(), "thread.proposal_changed" | "thread.proposal_gone" | "thread.proposal_answered") {
                    refresh();
                }
            }
        }
    });
}

fn n(v: &Value) -> i64 {
    v.as_i64().unwrap_or(0)
}

fn signed(fmt: &MoneyFormatter, tx: &Value) -> String {
    let amount = fmt.format(n(&tx["amount"]));
    match tx["type"].as_str() {
        Some("EXPENSE") => format!("\u{2212}{amount}"),
        Some("INCOME") => format!("+{amount}"),
        _ => amount,
    }
}

fn kind_word(kind: &str) -> &'static str {
    match kind {
        "INCOME" => "Income",
        "TRANSFER" => "Transfer",
        _ => "Expense",
    }
}

/// One thing the proposal acts on, as Tally draws it.
fn row(op: &str, item: &Value, fmt: &MoneyFormatter) -> gtk::Box {
    match op {
        "money.tx.delete" | "money.tx.update" => entry_row(item, fmt),
        "money.invest.delete" => activity_row(&item["activity"]),
        "money.budget.set" => {
            let name = item["category"].as_str().unwrap_or("Monthly budget");
            let money = |v: &Value| v.as_i64().map_or_else(|| String::from("none"), |a| fmt.format(a));
            let detail = match item["budget_after"].as_i64() {
                Some(_) => format!("{} \u{2192} {} a month", money(&item["budget"]), money(&item["budget_after"])),
                None => format!("{} \u{2192} removed", money(&item["budget"])),
            };
            let figure = item["budget_after"].as_i64().map(|a| fmt.format(a)).unwrap_or_default();
            icon_row(&tile("chart"), name, &detail, &figure, "")
        }
        "money.account.add" | "money.account.update" => account_row(item, fmt),
        _ => icon_row(&tile("dots"), "Something Relay cannot draw", "", "", ""),
    }
}

fn entry_row(item: &Value, fmt: &MoneyFormatter) -> gtk::Box {
    let tx = &item["entry"];
    if !tx.is_object() {
        return icon_row(&tile("trash"), "An entry no longer in the ledger", "", "", "");
    }
    let kind = tx["type"].as_str().unwrap_or("EXPENSE");
    let transfer = kind == "TRANSFER";
    let lead = badge_sized(if transfer { "repeat" } else { tx["icon"].as_str().unwrap_or("dot") }, if transfer { 10 } else { tx["color"].as_i64().unwrap_or(10) }, 28);
    let category = tx["category"].as_str().map_or_else(|| kind_word(kind).to_string(), str::to_string);
    let note = tx["note"].as_str().unwrap_or("").trim();
    let what = if note.is_empty() { category.clone() } else { note.to_string() };
    let after = &item["entry_after"];
    let mut detail = Vec::new();
    let mut figure = signed(fmt, tx);
    if after.is_object() {
        // What the change does, before → after, in place of what it leaves alone.
        let category_after = after["category"].as_str().map_or_else(|| kind_word(after["type"].as_str().unwrap_or(kind)).to_string(), str::to_string);
        detail.push(if category_after != category { format!("{category} \u{2192} {category_after}") } else { category.clone() });
        detail.push(if after["account"] != tx["account"] { format!("{} \u{2192} {}", tx["account"].as_str().unwrap_or(""), after["account"].as_str().unwrap_or("")) } else { tx["account"].as_str().unwrap_or("").to_string() });
        let (date, date_after) = (tx["date"].as_str().unwrap_or(""), after["date"].as_str().unwrap_or(""));
        detail.push(if date_after != date { format!("{} \u{2192} {}", human_date(date), human_date(date_after)) } else { human_date(date) });
        if after["note"] != tx["note"] {
            detail.push(format!("note \u{201c}{}\u{201d}", after["note"].as_str().unwrap_or("")));
        }
        if after["amount"] != tx["amount"] || after["type"] != tx["type"] {
            figure = format!("{figure} \u{2192} {}", signed(fmt, after));
        }
    } else {
        if !note.is_empty() {
            detail.push(category);
        }
        detail.push(if transfer { format!("{} \u{2192} {}", tx["account"].as_str().unwrap_or(""), tx["to_account"].as_str().unwrap_or("")) } else { tx["account"].as_str().unwrap_or("").to_string() });
        detail.push(human_date(tx["date"].as_str().unwrap_or("")));
    }
    icon_row(&lead, &what, &detail.join(" \u{b7} "), &figure, if kind == "INCOME" { "money-in" } else { "" })
}

fn activity_row(a: &Value) -> gtk::Box {
    if !a.is_object() {
        return icon_row(&tile("trash"), "An activity no longer in the ledger", "", "", "");
    }
    let currency = a["currency"].as_str().unwrap_or("CAD");
    let fmt = MoneyFormatter::new(currency, Locale::from_env());
    let kind = a["type"].as_str().unwrap_or("").replace('_', " ").to_lowercase();
    let title = match a["symbol"].as_str().filter(|s| !s.is_empty()) {
        Some(symbol) => format!("{symbol}, {kind}"),
        None => kind,
    };
    let detail = [a["account"].as_str().unwrap_or("").to_string(), human_date(a["date"].as_str().unwrap_or(""))].join(" \u{b7} ");
    icon_row(&tile("invest"), &title, &detail, &fmt.format(n(&a["amount"])), "")
}

fn account_row(item: &Value, fmt: &MoneyFormatter) -> gtk::Box {
    let (was, now) = (&item["account"], &item["account_after"]);
    let kind = |a: &Value| a["type"].as_str().unwrap_or("").to_lowercase();
    if !was.is_object() {
        // One to add: what it will be.
        let mut detail = vec![kind(now)];
        if let Some(r) = now["registration"].as_str() {
            detail.push(r.to_string());
        }
        if let Some(at) = now["institution"].as_str().filter(|i| !i.is_empty()) {
            detail.push(format!("at {at}"));
        }
        detail.push(format!("opening {}", fmt.format(n(&now["balance"]))));
        return icon_row(&tile("bank"), now["name"].as_str().unwrap_or("A new account"), &detail.join(" \u{b7} "), "New", "");
    }
    let name = was["name"].as_str().unwrap_or("");
    let title = match now["name"].as_str().filter(|n| *n != name) {
        Some(renamed) => format!("{name} \u{2192} {renamed}"),
        None => name.to_string(),
    };
    let mut detail = vec![kind(was)];
    if now["archived"] != was["archived"] {
        detail.push(String::from(if now["archived"] == true { "archive it" } else { "bring it back" }));
    }
    if now["registration"] != was["registration"] {
        detail.push(format!("registration {}", now["registration"].as_str().unwrap_or("none")));
    }
    if now["institution"] != was["institution"] {
        detail.push(format!("held at {}", now["institution"].as_str().unwrap_or("")));
    }
    icon_row(&tile("bank"), &title, &detail.join(" \u{b7} "), &fmt.format(n(&was["balance"])), "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_names_the_act_and_its_count() {
        let delete = json!({"action": {"money.tx.delete": [{"id": 1}, {"id": 2}]}, "items": []});
        assert_eq!(act(&delete), ("Delete 2 entries".into(), "Deleted 2 entries".into(), true));
        let entry = json!({"type": "EXPENSE", "amount": 1200, "date": "2026-10-03", "account_id": 1, "to_account_id": null, "note": "Pho", "category_id": 4});
        let moved = json!({"type": "EXPENSE", "amount": 1200, "date": "2026-10-03", "account_id": 1, "to_account_id": null, "note": "Pho", "category_id": 9});
        let fixed = json!({"type": "EXPENSE", "amount": 2100, "date": "2026-10-03", "account_id": 1, "to_account_id": null, "note": "Pho", "category_id": 4});
        let update = |after: &Value| json!({"action": {"money.tx.update": [{"id": 1}]}, "items": [{"entry": entry, "entry_after": after}]});
        assert_eq!(act(&update(&moved)).0, "Move 1 entry");
        assert_eq!(act(&update(&fixed)).0, "Change 1 entry");
        let budgets = json!({"action": {"money.budget.set": [{"amount": 0}, {"amount": 0}, {"amount": 0}]}, "items": [{"budget_after": null}, {"budget_after": null}, {"budget_after": null}]});
        assert_eq!(act(&budgets), ("Remove 3 budgets".into(), "Removed 3 budgets".into(), true));
        let archive = json!({"action": {"money.account.update": [{"id": 3}]}, "items": [{"account": {"archived": false}, "account_after": {"archived": true}}]});
        assert_eq!(act(&archive), ("Archive 1 account".into(), "Archived 1 account".into(), true));
    }
}
