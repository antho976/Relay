//! Aide in the Threads space, beside Tally, Arbiter and Avex (docs/AIDE.md): the person's own
//! agent, which wakes by the clock or on their data, runs on the model each job names, speaks
//! first, and keeps what it learns. Its pages: Overview (what waits on the person, what runs, what
//! it said), Jobs and one job with its runs, Memory (facts and living pages) and Settings.
//!
//! Everything is the engine's (`aide.*`); this module reads it and sends the person's changes. A
//! job's channel is a thread of kind `aide`, which the Threads page opens like any other; the
//! `wake` messages in it, Relay's words to the agent, are drawn by [`wake_note`].
use super::pages::{card, cell, columns, glyph_image, plural, setting_row, strip};
use super::{message, page, toast, Action, Page};
use crate::app::{button, clear, confirm_inline, label, rows, text, Ui};
use crate::client::Error;
use crate::panel::Panel;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Aide's tabbed pages: (stack name, caption).
pub const PAGES: [(&str, &str); 4] = [("aide-home", "Overview"), ("aide-jobs", "Jobs"), ("aide-memory", "Memory"), ("aide-settings", "Settings")];
/// One job and its runs, opened from a row; it carries no tab of its own.
pub const JOB: &str = "aide-job";
/// Every Aide page, for `money::add_pages`.
pub const ALL: [&str; 5] = ["aide-home", "aide-jobs", "aide-memory", "aide-settings", JOB];

/// Where Aide reads from: (source, its name, what it holds).
const SOURCES: [(&str, &str, &str); 4] = [
    ("dev", "Relay", "Sessions, tasks, Git and notifications"),
    ("money", "Tally", "The ledger, budgets and investments"),
    ("arbiter", "Arbiter", "Strategies, orders and decisions"),
    ("gym", "Avex", "Training history"),
];

/// A source's privacy tier: (stored, caption).
const TIERS: [(&str, &str); 3] = [("any", "Any model"), ("local_only", "Your own models only"), ("off", "Off")];

/// How loudly a job speaks: (stored, caption, what it means).
const LEVELS: [(&str, &str, &str); 3] = [
    ("interrupt", "Interrupt", "A notification now, on the phone too. Held in quiet hours and past the day's cap."),
    ("brief", "Brief", "Kept for the next brief."),
    ("quiet", "Quiet", "Only on Aide's page."),
];

/// A schedule's shapes: (stored `every`, caption). The first is none.
const EVERY: [(&str, &str); 5] = [("", "Not by the clock"), ("day", "Every day"), ("weekdays", "Weekdays"), ("week", "Every week"), ("month", "Every month")];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// A data rule's kinds: (stored, caption).
const RULES: [(&str, &str); 6] = [
    ("spend", "A Tally entry of at least"),
    ("budget", "A budget past"),
    ("stuck", "An agent waiting on you for"),
    ("notice", "A Relay notification"),
    ("arbiter", "Arbiter halts, or loses its exchange"),
    ("workout", "A new workout from Avex"),
];
/// The notification categories a notice rule may listen for (never Aide's own).
const CATEGORIES: [(&str, &str); 7] = [
    ("agent_blocked", "Agent needs you"),
    ("agent_done", "Agent finished"),
    ("guardrail", "Guardrail needs you"),
    ("integration", "Integrations"),
    ("provider", "Providers"),
    ("disk", "Disk"),
    ("system", "Everything else"),
];

/// What Aide never does, as `relay_aide::policy::REFUSED` says it: the engine refuses every op
/// outside its list, and these are the kinds a person might expect.
const REFUSED: [&str; 6] = [
    "Pay, buy or move money",
    "Place, change or cancel trades",
    "Delete anything of yours",
    "Send mail or messages to anyone but you",
    "Change guardrails, keys, pairing or its own settings",
    "Run commands or write files",
];

/// A job's runs and minutes when it names none, as the engine counts them.
const RUNS_PER_DAY: u32 = 24;
const MINUTES_PER_RUN: u32 = 10;

#[derive(Default)]
struct State {
    /// The job the Job page shows.
    job: Cell<Option<i64>>,
    /// An `aide.changed` is being waited on: a run announces several in a moment.
    pending: Cell<bool>,
}

thread_local! {
    static STATE: State = State::default();
}

// ---- Words ---------------------------------------------------------------------------------

/// What a failed call says on the page: the engine's own sentence, never its code. An engine
/// without Aide says so plainly.
fn said(e: &Error) -> String {
    match e {
        Error::Bus(b) if matches!(b.code.as_str(), "bus.unknown_op" | "bus.not_implemented") => {
            String::from("This engine has no Aide yet. Update Relay's engine to use it.")
        }
        Error::Bus(b) => b.message.clone(),
        e => e.to_string(),
    }
}

/// Where a fact came from, as a person says it.
fn source_word(source: &str) -> String {
    match source {
        "person" => String::from("You"),
        s if s.starts_with("thread:") => String::from("A run"),
        s => SOURCES.iter().find(|(key, _, _)| *key == s).map_or_else(|| s.to_string(), |(_, name, _)| name.to_string()),
    }
}

/// Where a fact came from, inside a sentence: "from you", "from Tally".
fn from_words(source: &str) -> String {
    match source {
        "person" => String::from("you"),
        s if s.starts_with("thread:") => String::from("a run"),
        s => source_word(s),
    }
}

fn host_word(host: &str) -> &'static str {
    match host {
        "claude" => "Claude",
        "pc" => "This PC",
        "server" => "Your server",
        _ => "",
    }
}

fn reason_word(reason: &str) -> &'static str {
    match reason {
        "schedule" => "By the clock",
        "rule" => "A rule",
        "follow_up" => "A follow-up",
        "manual" => "You pressed Run",
        "catch_up" => "Caught up after the PC was off",
        _ => "Woke",
    }
}

fn run_state_word(state: &str) -> &'static str {
    match state {
        "queued" => "Queued",
        "running" => "Running",
        "done" => "Done",
        "nothing" => "Nothing to say",
        "waiting" => "Waits on you",
        "late" => "Ran late",
        "failed" => "Failed",
        "skipped" => "Skipped",
        _ => "",
    }
}

/// A run's pill colour: green done, amber waiting or late, red failed.
fn run_tone(state: &str) -> &'static str {
    match state {
        "done" | "running" => "arbiter-live",
        "waiting" | "late" | "queued" => "arbiter-paper",
        "failed" => "arbiter-halted",
        _ => "",
    }
}

fn job_state_pill(state: &str) -> gtk::Label {
    match state {
        "proposed" => pill("Proposed", "arbiter-paper"),
        "active" => pill("On", "arbiter-live"),
        _ => pill("Paused", ""),
    }
}

fn level_word(level: &str) -> &'static str {
    LEVELS.iter().find(|(key, _, _)| *key == level).map_or("Brief", |(_, caption, _)| caption)
}

/// What became of a message: shown as a notification, held for the brief, or carried by one.
fn message_status(notified: bool, held: bool, briefed: bool) -> &'static str {
    if notified {
        "Notified"
    } else if briefed {
        "In the brief"
    } else if held {
        "Held for the brief"
    } else {
        ""
    }
}

/// "in 9 h", "in 25 min", "in 2 days", "now".
fn until_words(minutes: i64) -> String {
    match minutes {
        m if m <= 0 => String::from("now"),
        m if m < 60 => format!("in {m} min"),
        m if m < 48 * 60 => format!("in {} h", (m + 30) / 60),
        m => format!("in {} days", (m + 720) / 1440),
    }
}

/// "Morning brief · weekdays at 07:00 · in 9 h".
fn next_wake(name: &str, when: Option<&str>, minutes: Option<i64>) -> String {
    let mut parts = vec![name.to_string()];
    parts.extend(when.filter(|w| !w.is_empty()).map(str::to_string));
    parts.extend(minutes.map(until_words));
    parts.join(" · ")
}

/// What wakes a job, in one line: its schedule and its rules, as the engine says them.
fn when_line(j: &Value) -> String {
    let when: Vec<&str> = j["when"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if when.is_empty() {
        String::from("Runs only when you press Run")
    } else {
        let line = when.join(" · ");
        let mut chars = line.chars();
        chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
    }
}

/// "sonnet, then haiku", "Aide's default".
fn model_line(j: &Value) -> String {
    match (j["model"].as_str(), j["fallback"].as_str()) {
        (Some(m), Some(f)) => format!("{m}, then {f}"),
        (Some(m), None) => m.to_string(),
        (None, Some(f)) => format!("Aide's default, then {f}"),
        (None, None) => String::from("Aide's default"),
    }
}

/// Minutes from now until `ts`, an RFC 3339 time.
fn minutes_until(ts: &str) -> Option<i64> {
    let then = glib::DateTime::from_iso8601(ts, Some(&glib::TimeZone::utc())).ok()?;
    let now = glib::DateTime::now_utc().ok()?;
    Some(then.difference(&now).as_minutes())
}

fn ago(ts: &str) -> String {
    crate::relative::ago(ts, crate::relative::Form::Long).unwrap_or_default()
}

/// "Tue 27 Oct 14:02", in the PC's own time.
fn clock_time(ts: &str) -> String {
    glib::DateTime::from_iso8601(ts, Some(&glib::TimeZone::utc()))
        .ok()
        .and_then(|t| t.to_local().ok())
        .and_then(|t| t.format("%a %-d %b %H:%M").ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// How long a run took: "3 min", "under a minute".
fn took(minutes: i64) -> String {
    if minutes < 1 {
        String::from("under a minute")
    } else {
        format!("{minutes} min")
    }
}

fn minutes_between(from: &str, to: &str) -> Option<i64> {
    let a = glib::DateTime::from_iso8601(from, Some(&glib::TimeZone::utc())).ok()?;
    let b = glib::DateTime::from_iso8601(to, Some(&glib::TimeZone::utc())).ok()?;
    Some(b.difference(&a).as_minutes())
}

// ---- What the editors send -----------------------------------------------------------------

/// A schedule as the editor holds it: [`EVERY`]'s index, and its numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Clock {
    every: usize,
    weekday: u8,
    day: u8,
    hour: u8,
    minute: u8,
}

impl Default for Clock {
    fn default() -> Self {
        Clock { every: 0, weekday: 1, day: 1, hour: 7, minute: 0 }
    }
}

/// A stored schedule as the editor shows it; none when absent.
fn clock_of(s: &Value) -> Clock {
    let n = |key: &str, default: u8| s[key].as_u64().map_or(default, |v| v.min(255) as u8);
    let every = EVERY.iter().position(|(key, _)| !key.is_empty() && s["every"].as_str() == Some(*key)).unwrap_or(0);
    Clock { every, weekday: n("weekday", 1).clamp(1, 7), day: n("day", 1).clamp(1, 28), hour: n("hour", 7), minute: n("minute", 0) }
}

/// The schedule a job is sent with: a weekday only for a week, a day only for a month.
fn schedule_value(c: &Clock) -> Option<Value> {
    let every = EVERY.get(c.every)?.0;
    if every.is_empty() {
        return None;
    }
    let mut s = json!({"every": every, "hour": c.hour, "minute": c.minute});
    match every {
        "week" => s["weekday"] = json!(c.weekday),
        "month" => s["day"] = json!(c.day),
        _ => {}
    }
    Some(s)
}

/// A rule row as typed: [`RULES`]' index, the amount, the number and [`CATEGORIES`]' index.
#[derive(Debug, Clone, PartialEq)]
struct RuleDraft {
    kind: usize,
    amount: String,
    number: u32,
    category: usize,
}

/// A stored rule as its row shows it.
fn rule_draft(v: &Value) -> RuleDraft {
    let kind = RULES.iter().position(|(key, _)| v["kind"].as_str() == Some(*key)).unwrap_or(0);
    let amount = v["over"].as_f64().map(|x| if x.fract() == 0.0 { format!("{x:.0}") } else { x.to_string() }).unwrap_or_default();
    let number = v["percent"].as_u64().or_else(|| v["minutes"].as_u64()).map_or(match kind { 1 => 90, _ => 60 }, |n| n as u32);
    let category = CATEGORIES.iter().position(|(key, _)| v["category"].as_str() == Some(*key)).unwrap_or(0);
    RuleDraft { kind, amount, number, category }
}

/// The rule a row sends, or why it cannot be sent.
fn rule_value(r: &RuleDraft) -> Result<Value, String> {
    Ok(match RULES.get(r.kind).map_or("", |(key, _)| *key) {
        "spend" => {
            let cleaned: String = r.amount.trim().chars().filter(|c| !matches!(c, '$' | ' ')).collect::<String>().replace(',', ".");
            match cleaned.parse::<f64>() {
                Ok(over) if over.is_finite() && over > 0.0 => json!({"kind": "spend", "over": over}),
                _ => return Err(String::from("Type the amount a spend rule watches for, like 200.")),
            }
        }
        "budget" => json!({"kind": "budget", "percent": r.number}),
        "stuck" => json!({"kind": "stuck", "minutes": r.number}),
        "notice" => json!({"kind": "notice", "category": CATEGORIES.get(r.category).map_or("agent_blocked", |(key, _)| *key)}),
        "arbiter" => json!({"kind": "arbiter"}),
        _ => json!({"kind": "workout"}),
    })
}

/// A job as the editor holds it, ready to send.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    name: String,
    instructions: String,
    clock: Clock,
    rules: Vec<Value>,
    sources: Vec<&'static str>,
    model: Option<String>,
    fallback: Option<String>,
    /// Kept as it was: the editor does not show it.
    effort: Option<String>,
    level: &'static str,
    runs_per_day: u32,
    minutes_per_run: u32,
}

/// The `JobSpec` a draft is sent as: exactly its fields, the defaults left out.
fn spec_value(d: &Draft) -> Value {
    let mut s = json!({"name": d.name.trim(), "instructions": d.instructions.trim(), "sources": d.sources, "level": d.level});
    if let Some(schedule) = schedule_value(&d.clock) {
        s["schedule"] = schedule;
    }
    if !d.rules.is_empty() {
        s["rules"] = json!(d.rules);
    }
    for (key, value) in [("model", &d.model), ("fallback", &d.fallback), ("effort", &d.effort)] {
        if let Some(v) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            s[key] = json!(v);
        }
    }
    if d.runs_per_day != RUNS_PER_DAY {
        s["runs_per_day"] = json!(d.runs_per_day);
    }
    if d.minutes_per_run != MINUTES_PER_RUN {
        s["minutes_per_run"] = json!(d.minutes_per_run);
    }
    s
}

/// A model picker's choices: `none` first (no model named), then every model `aide.models`
/// lists, then `current` as it is written when the list does not have it (a Claude alias such
/// as `sonnet`, or a server since removed, which the engine says so of when it runs).
fn model_choices(models: &[Value], current: Option<&str>, none: &str) -> Vec<(Option<String>, String)> {
    let mut out = vec![(None, none.to_string())];
    for m in models {
        let id = text(m, "id");
        if id.is_empty() {
            continue;
        }
        // The engine's names say where each runs: "Qwen 3.5 4B (this PC)".
        let mut caption = if text(m, "name").is_empty() { format!("{id} · {}", host_word(text(m, "host"))) } else { text(m, "name").to_string() };
        if m["available"] == false {
            caption.push_str(" · not available");
        }
        out.push((Some(id.to_string()), caption));
    }
    if let Some(c) = current.filter(|c| !c.is_empty() && !out.iter().any(|(id, _)| id.as_deref() == Some(*c))) {
        out.push((Some(c.to_string()), c.to_string()));
    }
    out
}

/// One endpoint as the engine takes it: exactly its fields.
fn endpoint_body(e: &Value) -> Value {
    let mut out = json!({"id": text(e, "id"), "name": text(e, "name"), "url": text(e, "url"), "vision": e["vision"] == true});
    if let Some(m) = e["model"].as_str().filter(|m| !m.trim().is_empty()) {
        out["model"] = json!(m.trim());
    }
    if let Some(c) = e["context"].as_u64() {
        out["context"] = json!(c);
    }
    out
}

/// An endpoint from the server sheet's fields, or why it cannot be sent. The engine checks the
/// rest (the id's letters, the address's shape) and says so in its own words.
fn endpoint_value(id: &str, name: &str, url: &str, model: &str, context: &str, vision: bool) -> Result<Value, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err(String::from("Give the server an id: letters, digits, - and _."));
    }
    if url.trim().is_empty() {
        return Err(String::from("Type its address: http://host:port."));
    }
    let context = match context.trim() {
        "" => Value::Null,
        c => match c.replace([',', ' '], "").parse::<u32>() {
            Ok(n) => json!(n),
            Err(_) => return Err(String::from("Its context is a number of tokens, like 32768.")),
        },
    };
    Ok(endpoint_body(&json!({"id": id, "name": name.trim(), "url": url.trim(), "model": model, "context": context, "vision": vision})))
}

/// The Settings page's choices.
#[derive(Debug, Clone, PartialEq)]
struct SettingsForm {
    model: Option<String>,
    fallback: Option<String>,
    /// Each source's tier, in [`SOURCES`]' order.
    tiers: [&'static str; 4],
    quiet_start: u8,
    quiet_end: u8,
    cap: u32,
}

/// The whole `Settings` object `aide.settings.set` takes (it refuses a field it does not know):
/// `read` as the engine last gave it, with the form's choices and `endpoints` when given.
fn settings_body(read: &Value, form: Option<&SettingsForm>, endpoints: Option<&[Value]>) -> Value {
    let kept = |key: &str| read[key].as_str().filter(|m| !m.is_empty()).map(str::to_string);
    let (model, fallback) = form.map_or_else(|| (kept("model"), kept("fallback")), |f| (f.model.clone(), f.fallback.clone()));
    let mut tiers = serde_json::Map::new();
    for (i, (source, _, _)) in SOURCES.iter().enumerate() {
        let tier = form.map_or_else(|| read["tiers"][source].as_str().unwrap_or("any").to_string(), |f| f.tiers[i].to_string());
        tiers.insert(source.to_string(), json!(tier));
    }
    let hour = |key: &str, default: u64| read[key].as_u64().unwrap_or(default);
    let endpoints: Vec<Value> = match endpoints {
        Some(list) => list.iter().map(endpoint_body).collect(),
        None => rows(read, "endpoints").iter().map(endpoint_body).collect(),
    };
    let mut out = json!({
        "paused": read["paused"] == true,
        "quiet_start": form.map_or(hour("quiet_start", 22), |f| u64::from(f.quiet_start)),
        "quiet_end": form.map_or(hour("quiet_end", 7), |f| u64::from(f.quiet_end)),
        "interrupt_cap": form.map_or(hour("interrupt_cap", 6), |f| u64::from(f.cap)),
        "tiers": tiers,
        "endpoints": endpoints,
    });
    if let Some(m) = model {
        out["model"] = json!(m);
    }
    if let Some(f) = fallback {
        out["fallback"] = json!(f);
    }
    out
}

// ---- Marks and small widgets ---------------------------------------------------------------

/// Strokes on the 16-unit grid for Aide's own marks; the rest are Tally's glyphs.
fn glyph(key: &str) -> Option<&'static str> {
    Some(match key {
        "aide" => r##"<circle cx="8" cy="8" r="5.5" /><path d="M8 4.8l.9 2.3 2.3.9-2.3.9L8 11.2l-.9-2.3-2.3-.9 2.3-.9z" />"##,
        "job" => r##"<circle cx="8" cy="8" r="5.5" /><path d="M8 5v3.2l2 1.3" />"##,
        "run" => r##"<path d="M5.5 3.5v9l7-4.5z" />"##,
        "fact" => r##"<path d="M4.5 2.5h7v11L8 11l-3.5 2.5z" />"##,
        "page" => r##"<path d="M4 2.5h5.5L12 5v8.5H4z" /><path d="M9.5 2.5V5H12M6 8.5h4M6 11h4" />"##,
        "errand" => r##"<rect x="2.5" y="2.5" width="11" height="11" rx="2" /><path d="M5.5 8l1.8 1.8L10.8 6" />"##,
        "message" => r##"<path d="M3 3.5h10a.5.5 0 0 1 .5.5v6.5a.5.5 0 0 1-.5.5H7l-3 2.5V11H3a.5.5 0 0 1-.5-.5V4a.5.5 0 0 1 .5-.5z" />"##,
        "server" => r##"<rect x="2.5" y="3" width="11" height="4" rx="1" /><rect x="2.5" y="9" width="11" height="4" rx="1" /><path d="M5 5h.5M5 11h.5" />"##,
        "never" => r##"<circle cx="8" cy="8" r="5.5" /><path d="M4.1 11.9l7.8-7.8" />"##,
        _ => return None,
    })
}

/// An Aide glyph, else Tally's.
pub(super) fn icon(key: &str, size: i32) -> gtk::Image {
    match glyph(key) {
        Some(g) => crate::icons::from_geometry(g, size, 1.5),
        None => glyph_image(key, size),
    }
}

/// Tally's neutral tile, with an Aide glyph.
fn mark(key: &str) -> gtk::Box {
    let t = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    t.add_css_class("tally-tile");
    t.set_valign(gtk::Align::Center);
    t.set_hexpand(false);
    t.set_size_request(28, 28);
    let image = icon(key, 15);
    image.set_halign(gtk::Align::Center);
    image.set_hexpand(true);
    t.append(&image);
    t
}

fn pill(words: &str, tone: &str) -> gtk::Label {
    let l = label(words, "arbiter-pill");
    if !tone.is_empty() {
        l.add_css_class(tone);
    }
    l.set_valign(gtk::Align::Center);
    l
}

fn lamp(tone: &str) -> gtk::Box {
    let l = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    l.add_css_class("arbiter-lamp");
    if !tone.is_empty() {
        l.add_css_class(tone);
    }
    l.set_valign(gtk::Align::Center);
    l.set_halign(gtk::Align::Center);
    l
}

/// A page's head: its title, a context line under it, and keys on the right.
fn heading(head: &gtk::Box, title: &str, context: &str, actions: &[&gtk::Widget]) {
    clear(head);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_hexpand(true);
    words.append(&label(title, "money-title"));
    if !context.is_empty() {
        let l = label(context, "money-context");
        l.set_wrap(true);
        words.append(&l);
    }
    row.append(&words);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    keys.set_valign(gtk::Align::Center);
    for action in actions {
        keys.append(*action);
    }
    row.append(&keys);
    head.append(&row);
}

/// A quiet way back above a page's title: "‹ Jobs".
fn back_key(ui: &Rc<Ui>, head: &gtk::Box, caption: &str, to: &'static str) {
    let key = button("", "avex-back");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let chevron = crate::icons::image("chevron-left", 12);
    chevron.set_valign(gtk::Align::Center);
    row.append(&chevron);
    row.append(&label(caption, ""));
    key.set_child(Some(&row));
    key.set_halign(gtk::Align::Start);
    let said = format!("Back to {caption}");
    key.set_tooltip_text(Some(&said));
    key.update_property(&[gtk::accessible::Property::Label(&said)]);
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.navigate(to);
        }
    });
    head.prepend(&key);
}

/// A padded block inside a card, for what is not a row.
fn block(rows: &gtk::Box) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
    b.add_css_class("arbiter-block");
    rows.append(&b);
    b
}

fn quiet_row(rows: &gtk::Box, words: &str) {
    let l = label(words, "money-muted");
    l.set_wrap(true);
    l.add_css_class("arbiter-quiet-row");
    rows.append(&l);
}

/// A line across the page: Aide paused, or a job only proposed.
fn notice(words: &str, action: Option<gtk::Button>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("aide-notice");
    let m = icon("aide", 15);
    m.add_css_class("aide-notice-mark");
    m.set_valign(gtk::Align::Center);
    row.append(&m);
    let l = label(words, "aide-notice-text");
    l.set_wrap(true);
    l.set_hexpand(true);
    row.append(&l);
    if let Some(key) = action {
        key.set_valign(gtk::Align::Center);
        row.append(&key);
    }
    row
}

/// A row that asks something of the person: a mark, what it is over a line about it, its keys.
fn ask_row(mark_key: &str, title: &str, detail: &str, keys: &[gtk::Button]) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.add_css_class("aide-row");
    row.append(&mark(mark_key));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let t = label(title, "money-row-title");
    t.set_wrap(true);
    t.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    words.append(&t);
    if !detail.is_empty() {
        let d = label(detail, "money-row-detail");
        d.set_wrap(true);
        d.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        words.append(&d);
    }
    row.append(&words);
    let k = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    k.set_valign(gtk::Align::Center);
    for key in keys {
        key.add_css_class("arbiter-small");
        k.append(key);
    }
    row.append(&k);
    row
}

/// Runs `op` from `key`: the key waits while it runs, the page says what happened, and reads again.
fn act(ui: &Rc<Ui>, key: &gtk::Button, op: &'static str, payload: Value, done: &'static str) {
    key.set_sensitive(false);
    let (ui, key) = (ui.clone(), key.clone());
    glib::spawn_future_local(async move {
        match ui.call(op, payload).await {
            Ok(_) => toast(&ui, done, None),
            Err(e) => {
                key.set_sensitive(true);
                ui.show_error(&said(&e));
            }
        }
        ui.refresh_page();
    });
}

/// A key that runs `op` with `payload`; with `confirm`, only on a second press that says it.
fn op_key(ui: &Rc<Ui>, caption: &str, class: &str, op: &'static str, payload: Value, done: &'static str, confirm: Option<&str>) -> gtk::Button {
    let key = button(caption, class);
    let weak = Rc::downgrade(ui);
    let run = move |key: &gtk::Button| {
        if let Some(ui) = weak.upgrade() {
            act(&ui, key, op, payload.clone(), done);
        }
    };
    match confirm {
        Some(words) => confirm_inline(&key, words, run),
        None => {
            key.connect_clicked(run);
        }
    }
    key
}

/// A key that opens job `id`'s channel on the Threads page.
fn channel_key(ui: &Rc<Ui>, thread: i64) -> gtk::Button {
    let key = button("Channel", "money-secondary");
    key.set_tooltip_text(Some("Open the thread its runs talk in. You can answer there."));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::threads::open(&ui, thread);
        }
    });
    key
}

fn go(ui: &Rc<Ui>, page: &'static str) -> Action {
    let weak = Rc::downgrade(ui);
    Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            ui.navigate(page);
        }
    })
}

fn picker(captions: &[&str], selected: usize, name: &str) -> gtk::DropDown {
    let d = gtk::DropDown::from_strings(captions);
    d.add_css_class("money-picker");
    d.set_widget_name(name);
    d.set_selected(selected.min(captions.len().saturating_sub(1)) as u32);
    d.set_valign(gtk::Align::Center);
    d
}

fn spin(min: f64, max: f64, value: f64, name: &str) -> gtk::SpinButton {
    let s = gtk::SpinButton::with_range(min, max, 1.0);
    s.set_value(value);
    s.add_css_class("money-field");
    s.set_widget_name(name);
    s.set_valign(gtk::Align::Center);
    s
}

/// [`spin`] that shows two digits: an hour or a minute.
fn clock_spin(max: f64, value: f64, name: &str) -> gtk::SpinButton {
    let s = spin(0.0, max, value, name);
    s.set_width_chars(2);
    s.connect_output(|s| {
        s.set_text(&format!("{:02}", s.value_as_int()));
        glib::Propagation::Stop
    });
    s
}

fn words(text: &str) -> gtk::Label {
    let l = label(text, "arbiter-form-words");
    l.set_valign(gtk::Align::Center);
    l
}

/// A captioned field of a sheet, as the entry sheet draws them.
fn field(body: &gtk::Box, caption: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
    b.append(&label(&caption.to_uppercase(), "money-label"));
    b.append(widget);
    body.append(&b);
    b
}

fn sheet_entry(name: &str, value: &str, placeholder: &str) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.add_css_class("money-field");
    e.set_widget_name(name);
    e.set_text(value);
    e.set_placeholder_text(Some(placeholder));
    e
}

fn problem_label() -> gtk::Label {
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    problem
}

fn show_problem(problem: &gtk::Label, words: &str) {
    problem.set_text(words);
    problem.set_visible(true);
}

/// Relay's words to Aide that started a run, in its channel: quiet, and folded to a few lines.
pub(super) fn wake_note(words: &str) -> gtk::Box {
    let note = gtk::Box::new(gtk::Orientation::Vertical, 4);
    note.add_css_class("aide-wake");
    note.set_halign(gtk::Align::Start);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let m = icon("aide", 13);
    m.set_valign(gtk::Align::Center);
    top.append(&m);
    top.append(&label("Relay woke Aide", "aide-wake-title"));
    note.append(&top);
    let whole = words.trim().to_string();
    let fold = folded(&whole);
    let body = label(fold.as_deref().unwrap_or(&whole), "aide-wake-text");
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    body.set_max_width_chars(80);
    body.set_selectable(true);
    body.set_focusable(false);
    note.append(&body);
    if let Some(fold) = fold {
        let more = button("Show all", "money-text-action");
        more.add_css_class("aide-wake-more");
        more.set_halign(gtk::Align::Start);
        let shown = body.downgrade();
        more.connect_clicked(move |key| {
            let Some(body) = shown.upgrade() else { return };
            let open = body.text() == fold;
            body.set_text(if open { &whole } else { &fold });
            key.set_label(if open { "Show less" } else { "Show all" });
        });
        note.append(&more);
    }
    note
}

/// A wake's first lines, at most [`FOLD_CHARS`] of them; none when that is all of it.
fn folded(words: &str) -> Option<String> {
    let lines: Vec<&str> = words.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut fold = lines.iter().take(FOLD_LINES).copied().collect::<Vec<_>>().join("\n");
    if fold.chars().count() > FOLD_CHARS {
        fold = fold.chars().take(FOLD_CHARS).collect::<String>().trim_end().to_string();
    }
    if fold == lines.join("\n") {
        return None;
    }
    fold = fold.trim_end_matches('.').to_string();
    fold.push('…');
    Some(fold)
}

const FOLD_LINES: usize = 3;
const FOLD_CHARS: usize = 240;

// ---- The pages -----------------------------------------------------------------------------

/// Aide's tabs as Dev's segmented control, above each Aide page. One job shows under Jobs.
pub fn tabs(showing: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("money-tabs");
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("view-tabs");
    tabs.set_halign(gtk::Align::Start);
    for (name, caption) in PAGES {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("{name}-tab"));
        if name == showing || (showing == JOB && name == "aide-jobs") {
            tab.add_css_class("selected");
        }
        tab.connect_clicked(move |_| {
            if let Some(ui) = super::threads::the_ui_pub() {
                ui.navigate(name);
            }
        });
        tabs.append(&tab);
    }
    row.append(&tabs);
    row
}

/// A title and a first message on every page, before its first read.
pub fn install() {
    for (name, title) in ALL.into_iter().zip(["Aide", "Jobs", "Memory", "Settings", "Job"]) {
        if let Some(page) = page(name) {
            heading(&page.head, title, "", &[]);
            message(&page.body, "Reading Aide…");
        }
    }
}

/// Open job `id` on the Job page.
pub(super) fn open_job(ui: &Rc<Ui>, id: i64) {
    STATE.with(|s| s.job.set(Some(id)));
    if *ui.page.borrow() == JOB {
        ui.refresh_page();
    } else {
        ui.navigate(JOB);
    }
}

/// `aide.changed`: several in a moment are read once, on the Aide page showing. Settings change
/// only from their own page, which reads them again when it saves; a read in between would wipe
/// what the person is choosing there.
pub(super) fn changed(ui: &Rc<Ui>) {
    if STATE.with(|s| s.pending.replace(true)) {
        return;
    }
    let weak = Rc::downgrade(ui);
    glib::timeout_add_local_once(Duration::from_millis(300), move || {
        STATE.with(|s| s.pending.set(false));
        let Some(ui) = weak.upgrade() else { return };
        let showing = ui.page.borrow().clone();
        if !showing.starts_with("aide-") || showing == "aide-settings" {
            return;
        }
        // A field being typed in keeps its words: the page reads once it is left.
        if typing_on(&ui, &showing) {
            let weak = Rc::downgrade(&ui);
            glib::timeout_add_local_once(Duration::from_secs(2), move || {
                if let Some(ui) = weak.upgrade() {
                    changed(&ui);
                }
            });
            return;
        }
        ui.refresh_page();
    });
}

/// Whether the focus is in a field on page `name`.
fn typing_on(ui: &Ui, name: &str) -> bool {
    let Some(page) = page(name) else { return false };
    gtk::prelude::GtkWindowExt::focus(&ui.window)
        .is_some_and(|w| (w.is::<gtk::Editable>() || w.is::<gtk::TextView>()) && w.is_ancestor(&page.body))
}

pub async fn refresh(ui: &Rc<Ui>, name: &str) {
    let Some(page) = page(name) else { return };
    match name {
        "aide-home" => {
            let (summary, errands, jobs, facts, messages, templates) = tokio::join!(
                ui.call("aide.summary", json!({})),
                ui.call("aide.errand.list", json!({})),
                ui.call("aide.job.list", json!({})),
                ui.call("aide.fact.list", json!({})),
                ui.call("aide.message.list", json!({"limit": 12})),
                ui.call("aide.job.templates", json!({}))
            );
            if *ui.page.borrow() != name {
                return;
            }
            match summary {
                Ok(s) => {
                    let list = |v: Result<Value, Error>, key: &str| v.ok().map(|v| rows(&v, key)).unwrap_or_default();
                    let read = Home {
                        errands: list(errands, "errands"),
                        jobs: list(jobs, "jobs"),
                        facts: list(facts, "facts"),
                        messages: list(messages, "messages"),
                        templates: list(templates, "templates"),
                    };
                    home(ui, &page, &s, &read);
                }
                Err(e) => failed(ui, &page, "Aide", &e),
            }
        }
        "aide-jobs" => {
            let (jobs, runs, templates) =
                tokio::join!(ui.call("aide.job.list", json!({})), ui.call("aide.run.list", json!({"limit": 200})), ui.call("aide.job.templates", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match jobs {
                Ok(j) => jobs_page(ui, &page, &rows(&j, "jobs"), &runs.ok().map(|r| rows(&r, "runs")).unwrap_or_default(), &templates.ok().map(|t| rows(&t, "templates")).unwrap_or_default()),
                Err(e) => failed(ui, &page, "Jobs", &e),
            }
        }
        JOB => {
            let Some(id) = STATE.with(|s| s.job.get()) else {
                ui.navigate("aide-jobs");
                return;
            };
            let read = ui.call("aide.job.get", json!({"id": id})).await;
            if *ui.page.borrow() != name || STATE.with(|s| s.job.get()) != Some(id) {
                return;
            }
            match read {
                Ok(d) => job_page(ui, &page, &d),
                Err(e) => failed(ui, &page, "Job", &e),
            }
        }
        "aide-memory" => {
            let (facts, pages) = tokio::join!(ui.call("aide.fact.list", json!({})), ui.call("aide.page.list", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match facts {
                Ok(f) => memory_page(ui, &page, &rows(&f, "facts"), pages.ok().map(|p| rows(&p, "pages")).as_deref()),
                Err(e) => failed(ui, &page, "Memory", &e),
            }
        }
        "aide-settings" => {
            let (settings, models) = tokio::join!(ui.call("aide.settings.get", json!({})), ui.call("aide.models", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match settings {
                Ok(s) => settings_page(ui, &page, &s, models.map(|m| rows(&m, "models")).map_err(|e| said(&e))),
                Err(e) => failed(ui, &page, "Settings", &e),
            }
        }
        _ => {}
    }
}

fn failed(ui: &Rc<Ui>, page: &Page, name: &str, error: &Error) {
    heading(&page.head, name, "", &[]);
    message(&page.body, &said(error));
    let retry = button("Try again", "money-secondary");
    retry.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    retry.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.refresh_page();
        }
    });
    page.body.append(&retry);
}

// ---- Overview ------------------------------------------------------------------------------

/// What the Overview reads beside `aide.summary`.
struct Home {
    errands: Vec<Value>,
    jobs: Vec<Value>,
    facts: Vec<Value>,
    messages: Vec<Value>,
    templates: Vec<Value>,
}

/// Pause all, or Resume: the whole settings object again, with `paused` turned over.
fn pause_key(ui: &Rc<Ui>, paused: bool) -> gtk::Button {
    let key = button(if paused { "Resume" } else { "Pause all" }, if paused { "primary" } else { "money-secondary" });
    key.set_widget_name("aide-pause");
    key.set_tooltip_text(Some(if paused { "Let jobs run and Aide speak again" } else { "Nothing runs and nothing is said until you resume" }));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let key = key.clone();
        glib::spawn_future_local(async move {
            let set = match ui.call("aide.settings.get", json!({})).await {
                Ok(read) => {
                    let mut body = settings_body(&read, None, None);
                    body["paused"] = json!(!paused);
                    ui.call("aide.settings.set", body).await
                }
                Err(e) => Err(e),
            };
            match set {
                Ok(_) => toast(&ui, if paused { "Aide is running again." } else { "Paused. Nothing runs until you resume." }, None),
                Err(e) => {
                    key.set_sensitive(true);
                    ui.show_error(&said(&e));
                }
            }
            ui.refresh_page();
        });
    });
    key
}

/// The next wake, from the summary and the job it names.
fn next_line(s: &Value, jobs: &[Value]) -> Option<String> {
    let name = s["next_job"].as_str()?;
    let at = s["next_at"].as_str()?;
    let when = jobs.iter().find(|j| text(j, "name") == name && j["schedule"].is_object()).and_then(|j| j["when"][0].as_str());
    Some(next_wake(name, when, minutes_until(at)))
}

/// Overview: what waits on the person first, then what runs, what Aide said and what it is on.
fn home(ui: &Rc<Ui>, page: &Page, s: &Value, read: &Home) {
    let paused = s["paused"] == true;
    let next = next_line(s, &read.jobs);
    let context = match (&next, read.jobs.is_empty()) {
        _ if paused => String::from("Paused: nothing runs and nothing is said until you resume."),
        (Some(n), _) => format!("Next: {n}"),
        (None, true) => String::from("Your own agent: it works while you are away, speaks first when something needs you, and asks before anything that matters."),
        (None, false) => String::from("No job wakes by the clock now."),
    };
    let pause = pause_key(ui, paused);
    heading(&page.head, "Aide", &context, &[pause.upcast_ref()]);
    clear(&page.body);
    let body = &page.body;
    if paused {
        body.append(&notice("Aide is paused. Jobs do not run, rules do not wake them, and nothing is said until you resume.", None));
    }

    let errands: Vec<&Value> = read.errands.iter().filter(|e| text(e, "state") == "waiting" || e["proposed_done"] == true).collect();
    let proposed: Vec<&Value> = read.jobs.iter().filter(|j| text(j, "state") == "proposed").collect();
    let unconfirmed: Vec<&Value> = read.facts.iter().filter(|f| f["confirmed"] != true).collect();
    let running = rows(s, "running");

    let (_, cells) = strip(body);
    let (figure, sub) = match (&next, s["next_at"].as_str().and_then(minutes_until)) {
        _ if paused => (String::from("Paused"), String::from("Until you resume")),
        (Some(_), Some(m)) => (until_words(m), s["next_job"].as_str().unwrap_or("").to_string()),
        _ => (String::from("None"), String::from("Nothing wakes by the clock")),
    };
    cell(&cells, "calendar", "Next wake", &figure, &sub, None);
    let queued = running.iter().filter(|r| text(r, "state") == "queued").count();
    cell(&cells, "spark", "Running now", &running.len().to_string(), &if queued > 0 { format!("{queued} waiting for a seat") } else if running.is_empty() { String::from("Nothing running") } else { String::from("Stop any of them below") }, None);
    let waits = errands.len() + proposed.len() + unconfirmed.len();
    let mut parts = Vec::new();
    for (n, one) in [(errands.len(), "errand"), (proposed.len(), "job"), (unconfirmed.len(), "fact")] {
        if n > 0 {
            parts.push(plural(n, one));
        }
    }
    cell(&cells, "list", "Waiting on you", &waits.to_string(), &if parts.is_empty() { String::from("Nothing waits") } else { parts.join(" · ") }, None);
    let (today, cap) = (s["interrupts_today"].as_u64().unwrap_or(0), s["interrupt_cap"].as_u64().unwrap_or(0));
    let held = s["held"].as_u64().unwrap_or(0);
    cell(&cells, "gauge", "Interrupts today", &format!("{today} of {cap}"), &if held > 0 { format!("{held} held for the brief") } else { String::from("None held for the brief") }, if cap > 0 && today >= cap { Some("money-over") } else { None });

    // No jobs yet: the ones Aide starts with come first.
    if read.jobs.is_empty() && !read.templates.is_empty() {
        templates_card(ui, body, &read.templates, &read.jobs, "Start with a job");
    }

    let pair = columns(body, 2);
    let title = if waits > 0 { format!("Waiting on you · {waits}") } else { String::from("Waiting on you") };
    let list = card(&pair[0], &title, None);
    if waits == 0 {
        quiet_row(&list, "Nothing waits on you.");
    }
    for e in errands {
        let id = e["id"].as_i64().unwrap_or(0);
        let mut detail = vec![if e["proposed_done"] == true { String::from("Aide thinks it is done") } else { String::from("Waits on you") }];
        if !text(e, "detail").is_empty() {
            detail.push(text(e, "detail").to_string());
        }
        let close = op_key(ui, "Close", "primary", "aide.errand.update", json!({"id": id, "state": "done"}), "Closed.", None);
        let (caption, tip) = if e["proposed_done"] == true { ("Reopen", "It is not done: Aide keeps at it") } else { ("Hand back", "Aide takes it up again") };
        let reopen = op_key(ui, caption, "quiet", "aide.errand.update", json!({"id": id, "state": "open"}), "Back on Aide's list.", None);
        reopen.set_tooltip_text(Some(tip));
        list.append(&ask_row("errand", text(e, "title"), &detail.join(" · "), &[reopen, close]));
    }
    for j in proposed {
        let id = j["id"].as_i64().unwrap_or(0);
        let look = button("Look", "money-text-action");
        let weak = Rc::downgrade(ui);
        look.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                open_job(&ui, id);
            }
        });
        let remove = op_key(ui, "Remove", "quiet", "aide.job.delete", json!({"id": id}), "Removed.", Some("Remove it"));
        let on = op_key(ui, "Turn on", "primary", "aide.job.set", json!({"id": id, "state": "active"}), "Turned on.", None);
        list.append(&ask_row("job", &format!("Aide proposes a job: {}", text(j, "name")), &when_line(j), &[look, remove, on]));
    }
    for f in unconfirmed {
        let id = f["id"].as_i64().unwrap_or(0);
        let detail = format!("Learned from {} · {}", from_words(text(f, "source")), ago(text(f, "learned_at")));
        let forget = op_key(ui, "Forget", "quiet", "aide.fact.forget", json!({"id": id}), "Forgotten.", None);
        let confirm = op_key(ui, "Confirm", "primary", "aide.fact.update", json!({"id": id, "confirmed": true}), "Confirmed. Runs are given it from now on.", None);
        list.append(&ask_row("fact", text(f, "text"), &detail, &[forget, confirm]));
    }

    let list = card(&pair[0], "Running now", Some(("All jobs", go(ui, "aide-jobs"))));
    if running.is_empty() {
        quiet_row(&list, "Nothing is running.");
    }
    for r in &running {
        list.append(&running_row(ui, r));
    }

    let list = card(&pair[1], "What Aide said", None);
    if read.messages.is_empty() {
        quiet_row(&list, "Nothing yet. What it says shows here, and as a notification when it interrupts.");
    }
    for m in &read.messages {
        list.append(&message_row(m));
    }

    let open: Vec<&Value> = read.errands.iter().filter(|e| text(e, "state") == "open" && e["proposed_done"] != true).collect();
    if !open.is_empty() {
        let list = card(&pair[1], "On its list", None);
        for e in open {
            let id = e["id"].as_i64().unwrap_or(0);
            let mut detail = text(e, "detail").to_string();
            if let Some(m) = e["due_at"].as_str().and_then(minutes_until) {
                let due = if m < 0 { String::from("late") } else { format!("due {}", until_words(m)) };
                detail = if detail.is_empty() { due } else { format!("{due} · {detail}") };
            }
            let close = op_key(ui, "Close", "quiet", "aide.errand.update", json!({"id": id, "state": "done"}), "Closed.", None);
            list.append(&ask_row("errand", text(e, "title"), &detail, &[close]));
        }
    }
}

fn running_row(ui: &Rc<Ui>, r: &Value) -> gtk::Box {
    let id = r["id"].as_i64().unwrap_or(0);
    let mut detail = vec![run_state_word(text(r, "state")).to_string()];
    let host = host_word(text(r, "host"));
    if !host.is_empty() {
        detail.push(host.to_string());
    }
    detail.push(format!("started {}", ago(text(r, "started_at"))));
    if !text(r, "trigger").is_empty() {
        detail.push(text(r, "trigger").to_string());
    }
    let mut keys = Vec::new();
    if let Some(t) = r["thread_id"].as_i64() {
        keys.push(channel_key(ui, t));
    }
    keys.push(op_key(ui, "Stop", "money-destructive", "aide.run.stop", json!({"id": id}), "Stopped.", None));
    ask_row("run", text(r, "job_name"), &detail.join(" · "), &keys)
}

/// A message: its title and what became of it, a few lines of it, its level and when.
fn message_row(m: &Value) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 3);
    row.add_css_class("tally-row");
    row.add_css_class("aide-message");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let t = label(text(m, "title"), "money-row-title");
    t.set_wrap(true);
    t.set_hexpand(true);
    top.append(&t);
    let status = message_status(m["notified"] == true, m["held"] == true, m["briefed_at"].is_string());
    if !status.is_empty() {
        top.append(&pill(status, if m["notified"] == true { "arbiter-live" } else { "" }));
    }
    row.append(&top);
    let body = text(m, "body").trim();
    if !body.is_empty() {
        let b = label(body, "aide-message-body");
        b.set_wrap(true);
        b.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        b.set_lines(3);
        b.set_ellipsize(gtk::pango::EllipsizeMode::End);
        b.set_tooltip_text(Some(body));
        row.append(&b);
    }
    row.append(&label(&format!("{} · {}", level_word(text(m, "level")), ago(text(m, "created_at"))), "money-row-detail"));
    row
}

/// The jobs Aide starts with that are not here yet: each with Change first and Add.
fn templates_card(ui: &Rc<Ui>, parent: &gtk::Box, templates: &[Value], jobs: &[Value], title: &str) {
    let left: Vec<&Value> = templates.iter().filter(|t| !jobs.iter().any(|j| text(j, "name") == text(&t["spec"], "name"))).collect();
    if left.is_empty() {
        return;
    }
    let list = card(parent, title, None);
    for t in left {
        let spec = t["spec"].clone();
        let change = button("Change first", "quiet");
        change.set_tooltip_text(Some("Open it in the editor before it is saved"));
        let (weak, seed) = (Rc::downgrade(ui), spec.clone());
        change.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                editor(&ui, None, &seed);
            }
        });
        let add = op_key(ui, "Add", "primary", "aide.job.save", json!({"spec": spec, "active": true}), "Added and turned on.", None);
        add.set_widget_name(&format!("aide-template-{}", text(t, "id")));
        let detail = format!("{} {}", text(t, "why"), when_of_spec(&t["spec"]));
        list.append(&ask_row("job", text(&t["spec"], "name"), detail.trim(), &[change, add]));
    }
}

/// "Weekdays at 07:00.", for a template the engine has not described: its schedule only.
fn when_of_spec(spec: &Value) -> String {
    let s = &spec["schedule"];
    let Some(every) = s["every"].as_str() else { return String::new() };
    let at = format!("{:02}:{:02}", s["hour"].as_u64().unwrap_or(0), s["minute"].as_u64().unwrap_or(0));
    match every {
        "day" => format!("Every day at {at}."),
        "weekdays" => format!("Weekdays at {at}."),
        "week" => format!("Every {} at {at}.", WEEKDAYS[(s["weekday"].as_u64().unwrap_or(1).clamp(1, 7) - 1) as usize]),
        "month" => format!("On day {} of every month at {at}.", s["day"].as_u64().unwrap_or(1)),
        _ => String::new(),
    }
}

// ---- Jobs ----------------------------------------------------------------------------------

fn new_key(ui: &Rc<Ui>) -> gtk::Button {
    let key = button("New job", "primary");
    key.set_widget_name("aide-new-job");
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            editor(&ui, None, &Value::Null);
        }
    });
    key
}

/// The keys a job carries wherever it shows: Run now, Pause or Turn on, Edit, and Remove when
/// `removable`.
fn job_keys(ui: &Rc<Ui>, j: &Value, removable: bool) -> Vec<gtk::Button> {
    let id = j["id"].as_i64().unwrap_or(0);
    let state = text(j, "state");
    let mut keys = Vec::new();
    if state != "proposed" {
        let run = op_key(ui, "Run now", "money-secondary", "aide.run.now", json!({"job_id": id}), "Running. Its channel shows it as it goes.", None);
        run.set_widget_name(&format!("aide-run-{id}"));
        keys.push(run);
    }
    keys.push(if state == "active" {
        op_key(ui, "Pause", "money-secondary", "aide.job.set", json!({"id": id, "state": "paused"}), "Paused.", None)
    } else {
        op_key(ui, "Turn on", "primary", "aide.job.set", json!({"id": id, "state": "active"}), "Turned on.", None)
    });
    let edit = button("Edit", "money-secondary");
    if !removable {
        edit.set_widget_name("aide-edit");
    }
    let (weak, job) = (Rc::downgrade(ui), j.clone());
    edit.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            editor(&ui, job["id"].as_i64(), &job);
        }
    });
    keys.push(edit);
    if removable {
        keys.push(op_key(ui, "Remove", "money-destructive", "aide.job.delete", json!({"id": id}), "Removed, with its runs.", Some("Remove it")));
    }
    keys
}

fn jobs_page(ui: &Rc<Ui>, page: &Page, jobs: &[Value], runs: &[Value], templates: &[Value]) {
    let new = new_key(ui);
    heading(&page.head, "Jobs", "What Aide does, and what wakes it. Each job reads only what you allow, on the model it names.", &[new.upcast_ref()]);
    clear(&page.body);
    let list = card(&page.body, "Jobs", None);
    if jobs.is_empty() {
        quiet_row(&list, "No jobs yet. A job is a few plain sentences and what wakes it: a time of day, or something in your data. Start from one below, or write your own.");
    }
    for j in jobs {
        let id = j["id"].as_i64();
        let last = runs.iter().find(|r| r["job_id"].as_i64() == id);
        list.append(&job_row(ui, j, last));
    }
    templates_card(ui, &page.body, templates, jobs, "Jobs Aide starts with");
}

fn job_row(ui: &Rc<Ui>, j: &Value, last: Option<&Value>) -> gtk::Box {
    let id = j["id"].as_i64().unwrap_or(0);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("tally-row");
    row.add_css_class("tally-entry");
    let main = button("", "money-row-key");
    main.set_widget_name(&format!("aide-job-{id}"));
    main.set_hexpand(true);
    main.set_tooltip_text(Some("Open this job and its runs"));
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    inner.append(&mark("job"));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = label(text(j, "name"), "money-row-title");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    top.append(&name);
    top.append(&job_state_pill(text(j, "state")));
    words.append(&top);
    let when = label(&when_line(j), "money-row-detail");
    when.set_wrap(true);
    words.append(&when);
    let mut line = vec![model_line(j)];
    match last {
        Some(r) => line.push(format!("Last run: {} · {}", run_state_word(text(r, "state")).to_lowercase(), ago(text(r, "started_at")))),
        None => line.push(String::from("Not run yet")),
    }
    let d = label(&line.join(" · "), "money-row-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    inner.append(&words);
    main.set_child(Some(&inner));
    let weak = Rc::downgrade(ui);
    main.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_job(&ui, id);
        }
    });
    row.append(&main);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    keys.set_valign(gtk::Align::Center);
    keys.set_margin_end(12);
    for key in job_keys(ui, j, true) {
        key.add_css_class("arbiter-small");
        keys.append(&key);
    }
    row.append(&keys);
    row
}

/// One job: what it does and what it may read, then its last runs.
fn job_page(ui: &Rc<Ui>, page: &Page, d: &Value) {
    let j = &d["job"];
    let mut keys: Vec<gtk::Widget> = job_keys(ui, j, false).into_iter().map(|k| k.upcast()).collect();
    if let Some(t) = j["thread_id"].as_i64() {
        let key = channel_key(ui, t);
        key.set_widget_name("aide-channel");
        keys.push(key.upcast());
    }
    heading(&page.head, text(j, "name"), &when_line(j), &keys.iter().collect::<Vec<_>>());
    back_key(ui, &page.head, "Jobs", "aide-jobs");
    clear(&page.body);
    let body = &page.body;
    if text(j, "state") == "proposed" {
        let id = j["id"].as_i64().unwrap_or(0);
        let on = op_key(ui, "Turn on", "primary", "aide.job.set", json!({"id": id, "state": "active"}), "Turned on.", None);
        body.append(&notice("Aide proposed this job. It does nothing until you turn it on.", Some(on)));
    }

    let about = card(body, "What it does", None);
    let b = block(&about);
    let instructions = label(text(j, "instructions"), "aide-instructions");
    instructions.set_wrap(true);
    instructions.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    instructions.set_selectable(true);
    instructions.set_focusable(false);
    b.append(&instructions);
    let facts = gtk::Grid::new();
    facts.set_row_spacing(6);
    facts.set_column_spacing(16);
    facts.set_margin_top(6);
    let sources: Vec<String> = j["sources"].as_array().into_iter().flatten().filter_map(Value::as_str).map(source_word).collect();
    let runs_a_day = j["runs_per_day"].as_u64().unwrap_or(u64::from(RUNS_PER_DAY));
    let minutes = j["minutes_per_run"].as_u64().unwrap_or(u64::from(MINUTES_PER_RUN));
    let mut lines = vec![
        ("Wakes", when_line(j)),
        ("Reads", if sources.is_empty() { String::from("Only what its rules read") } else { sources.join(", ") }),
        ("Model", model_line(j)),
        ("Speaks", format!("{}: {}", level_word(text(j, "level")), LEVELS.iter().find(|l| l.0 == text(j, "level")).map_or("", |l| l.2))),
        ("Limits", format!("{runs_a_day} runs a day · {minutes} min a run")),
    ];
    if let Some(m) = j["next_due_at"].as_str().and_then(minutes_until) {
        lines.push(("Next wake", format!("{} · {}", until_words(m), clock_time(text(j, "next_due_at")))));
    }
    for (row, (name, value)) in lines.into_iter().enumerate() {
        let n = label(name, "aide-fact-name");
        n.set_valign(gtk::Align::Start);
        let v = label(&value, "aide-fact");
        v.set_wrap(true);
        v.set_hexpand(true);
        facts.attach(&n, 0, row as i32, 1, 1);
        facts.attach(&v, 1, row as i32, 1, 1);
    }
    b.append(&facts);

    let runs = rows(d, "runs");
    let list = card(body, "Runs", None);
    if runs.is_empty() {
        quiet_row(&list, "It has not run yet.");
    }
    for r in &runs {
        list.append(&run_row(ui, r));
    }
}

/// A run: why it woke, how it went and on what, what it read, its note and its times.
fn run_row(ui: &Rc<Ui>, r: &Value) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.add_css_class("tally-row");
    row.add_css_class("aide-run");
    let state = text(r, "state");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    top.append(&label(reason_word(text(r, "reason")), "money-row-title"));
    top.append(&pill(run_state_word(state), run_tone(state)));
    let host = host_word(text(r, "host"));
    if !host.is_empty() {
        top.append(&pill(host, ""));
    }
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    top.append(&gap);
    top.append(&label(&ago(text(r, "started_at")), "money-row-detail"));
    row.append(&top);
    let trigger = text(r, "trigger");
    if !trigger.is_empty() {
        let t = label(trigger, "money-row-detail");
        t.set_wrap(true);
        t.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        row.append(&t);
    }
    let reads = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    if let Some(model) = r["model"].as_str() {
        reads.append(&label(&format!("On {model}"), "money-row-detail"));
    }
    for s in r["sources"].as_array().into_iter().flatten() {
        let ok = s["ok"] == true;
        let l = label(&format!("{} {}", source_word(text(s, "source")), if ok { "✓" } else { "✗" }), "aide-read");
        l.add_css_class(if ok { "ok" } else { "failed" });
        l.set_tooltip_text(Some(text(s, "detail")));
        reads.append(&l);
    }
    if reads.first_child().is_some() {
        row.append(&reads);
    }
    if let Some(note) = r["note"].as_str().filter(|n| !n.trim().is_empty()) {
        let n = label(note, "arbiter-why");
        n.set_wrap(true);
        n.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        row.append(&n);
    }
    let mut times = format!("Started {}", clock_time(text(r, "started_at")));
    match (r["ended_at"].as_str(), r["deadline_at"].as_str()) {
        (Some(end), _) => {
            if let Some(m) = minutes_between(text(r, "started_at"), end) {
                times.push_str(&format!(" · took {}", took(m)));
            }
        }
        (None, Some(deadline)) => times.push_str(&format!(" · stopped by {} if still going", clock_time(deadline).rsplit(' ').next().unwrap_or(""))),
        _ => {}
    }
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let t = label(&times, "money-row-detail");
    t.set_hexpand(true);
    t.set_valign(gtk::Align::Center);
    foot.append(&t);
    if let Some(thread) = r["thread_id"].as_i64() {
        let key = channel_key(ui, thread);
        key.add_css_class("arbiter-small");
        foot.append(&key);
    }
    if matches!(state, "queued" | "running") {
        let stop = op_key(ui, "Stop", "money-destructive", "aide.run.stop", json!({"id": r["id"]}), "Stopped.", None);
        stop.add_css_class("arbiter-small");
        foot.append(&stop);
    }
    row.append(&foot);
    row
}

// ---- The job editor ------------------------------------------------------------------------

/// A rule row's fields.
struct RuleRow {
    row: gtk::Box,
    kind: gtk::DropDown,
    amount: gtk::Entry,
    number: gtk::SpinButton,
    category: gtk::DropDown,
}

impl RuleRow {
    fn draft(&self) -> RuleDraft {
        RuleDraft { kind: self.kind.selected() as usize, amount: self.amount.text().to_string(), number: self.number.value_as_int().max(0) as u32, category: self.category.selected() as usize }
    }
}

/// The editor's fields.
struct Form {
    name: gtk::Entry,
    instructions: gtk::TextView,
    every: gtk::DropDown,
    weekday: gtk::DropDown,
    day: gtk::SpinButton,
    hour: gtk::SpinButton,
    minute: gtk::SpinButton,
    rules: RefCell<Vec<RuleRow>>,
    sources: Vec<(&'static str, gtk::CheckButton)>,
    model: gtk::DropDown,
    models: Vec<(Option<String>, String)>,
    fallback: gtk::DropDown,
    fallbacks: Vec<(Option<String>, String)>,
    levels: Vec<(&'static str, gtk::ToggleButton)>,
    runs: gtk::SpinButton,
    minutes: gtk::SpinButton,
    effort: Option<String>,
}

impl Form {
    fn rule_drafts(&self) -> Vec<RuleDraft> {
        self.rules.borrow().iter().map(RuleRow::draft).collect()
    }

    /// What the fields say, or the first that cannot be sent.
    fn draft(&self) -> Result<Draft, String> {
        let buffer = self.instructions.buffer();
        let rules = self.rule_drafts().iter().map(rule_value).collect::<Result<Vec<_>, _>>()?;
        Ok(Draft {
            name: self.name.text().to_string(),
            instructions: buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string(),
            clock: Clock {
                every: self.every.selected() as usize,
                weekday: self.weekday.selected() as u8 + 1,
                day: self.day.value_as_int() as u8,
                hour: self.hour.value_as_int() as u8,
                minute: self.minute.value_as_int() as u8,
            },
            rules,
            sources: self.sources.iter().filter(|(_, c)| c.is_active()).map(|(s, _)| *s).collect(),
            model: self.models.get(self.model.selected() as usize).and_then(|m| m.0.clone()),
            fallback: self.fallbacks.get(self.fallback.selected() as usize).and_then(|m| m.0.clone()),
            effort: self.effort.clone(),
            level: self.levels.iter().find(|(_, k)| k.is_active()).map_or("brief", |(l, _)| *l),
            runs_per_day: self.runs.value_as_int().max(1) as u32,
            minutes_per_run: self.minutes.value_as_int().max(1) as u32,
        })
    }

    /// Everything typed, to tell an unsaved draft from the one the sheet opened with.
    fn snapshot(&self) -> String {
        let rules: Vec<String> = self.rule_drafts().iter().map(|r| format!("{r:?}")).collect();
        match self.draft() {
            Ok(d) => format!("{d:?}{rules:?}"),
            Err(_) => format!("{rules:?}{}", self.name.text()),
        }
    }
}

/// The job editor: a new job when `id` is none, filled from `seed` (a job, a template's spec, or
/// nothing). Saving is `aide.job.save`; a refusal shows the engine's sentence above the keys.
pub(super) fn editor(ui: &Rc<Ui>, id: Option<i64>, seed: &Value) {
    let title = if id.is_some() { "Edit job" } else { "New job" };
    let Some(panel) = Panel::toggle(ui, title, 600) else { return };
    panel.add_css_class("money-sheet");
    panel.add_css_class("aide-sheet");
    let loading = label("Reading the models…", "money-muted");
    panel.body.append(&loading);
    panel.present();
    let (ui, seed) = (ui.clone(), seed.clone());
    glib::spawn_future_local(async move {
        let models = ui.call("aide.models", json!({})).await.map(|m| rows(&m, "models"));
        panel.body.remove(&loading);
        let note = models.as_ref().err().map(said);
        form(&ui, &panel, id, &seed, &models.unwrap_or_default(), note);
    });
}

fn rule_row(form: &Rc<Form>, list: &gtk::Box, draft: &RuleDraft) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("arbiter-form-line");
    row.add_css_class("aide-rule");
    let captions: Vec<&str> = RULES.iter().map(|(_, c)| *c).collect();
    let kind = picker(&captions, draft.kind, "aide-rule-kind");
    kind.set_hexpand(true);
    let amount = sheet_entry("aide-rule-amount", &draft.amount, "200");
    amount.set_width_chars(7);
    let number = spin(1.0, 10_080.0, f64::from(draft.number.max(1)), "aide-rule-number");
    let categories: Vec<&str> = CATEGORIES.iter().map(|(_, c)| *c).collect();
    let category = picker(&categories, draft.category, "aide-rule-category");
    let unit = words("");
    let remove = crate::app::icon_button("close", "Remove this rule");
    for w in [kind.upcast_ref::<gtk::Widget>(), amount.upcast_ref(), number.upcast_ref(), category.upcast_ref(), unit.upcast_ref(), remove.upcast_ref()] {
        row.append(w);
    }
    let show = {
        let (amount, number, category, unit) = (amount.clone(), number.clone(), category.clone(), unit.clone());
        move |kind: u32| {
            let key = RULES.get(kind as usize).map_or("", |(k, _)| *k);
            amount.set_visible(key == "spend");
            category.set_visible(key == "notice");
            number.set_visible(matches!(key, "budget" | "stuck"));
            match key {
                "budget" => number.set_range(1.0, 500.0),
                "stuck" => number.set_range(1.0, 10_080.0),
                _ => {}
            }
            unit.set_text(match key {
                "spend" => "or more",
                "budget" => "% of its month",
                "stuck" => "minutes",
                _ => "",
            });
            unit.set_visible(!unit.text().is_empty());
        }
    };
    show(kind.selected());
    // The number keeps what was typed, within the new kind's range.
    number.set_value(f64::from(draft.number.max(1)));
    kind.connect_selected_notify(move |k| show(k.selected()));
    list.append(&row);
    let shown = row.clone();
    let weak = Rc::downgrade(form);
    let parent = list.downgrade();
    remove.connect_clicked(move |_| {
        let (Some(form), Some(parent)) = (weak.upgrade(), parent.upgrade()) else { return };
        parent.remove(&shown);
        form.rules.borrow_mut().retain(|r| r.row != shown);
    });
    form.rules.borrow_mut().push(RuleRow { row, kind, amount, number, category });
}

fn form(ui: &Rc<Ui>, panel: &Rc<Panel>, id: Option<i64>, seed: &Value, models: &[Value], models_note: Option<String>) {
    let body = panel.body.clone();
    let name = sheet_entry("aide-edit-name", text(seed, "name"), "Morning brief");
    field(&body, "Name", &name);

    let instructions = gtk::TextView::new();
    instructions.set_wrap_mode(gtk::WrapMode::WordChar);
    instructions.set_accepts_tab(false);
    instructions.add_css_class("aide-instructions-field");
    instructions.set_widget_name("aide-edit-instructions");
    instructions.buffer().set_text(text(seed, "instructions"));
    instructions.update_property(&[gtk::accessible::Property::Label("What it does")]);
    let scroll = gtk::ScrolledWindow::builder().child(&instructions).min_content_height(110).max_content_height(260).propagate_natural_height(true).build();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.add_css_class("arbiter-secret-frame");
    let about = field(&body, "What it does", &scroll);
    about.append(&label("Plain sentences, as you would ask a person: what to look at, and what to tell you.", "money-row-detail"));

    // When it wakes by the clock.
    let clock = clock_of(&seed["schedule"]);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("arbiter-form-line");
    let every = picker(&EVERY.map(|e| e.1), clock.every, "aide-edit-every");
    let weekday = picker(&WEEKDAYS, usize::from(clock.weekday.clamp(1, 7) - 1), "aide-edit-weekday");
    let on_day = words("on day");
    let day = spin(1.0, 28.0, f64::from(clock.day), "aide-edit-day");
    let at = words("at");
    let hour = clock_spin(23.0, f64::from(clock.hour), "aide-edit-hour");
    let colon = words(":");
    let minute = clock_spin(59.0, f64::from(clock.minute), "aide-edit-minute");
    for w in [every.upcast_ref::<gtk::Widget>(), weekday.upcast_ref(), on_day.upcast_ref(), day.upcast_ref(), at.upcast_ref(), hour.upcast_ref(), colon.upcast_ref(), minute.upcast_ref()] {
        line.append(w);
    }
    let show = {
        let (weekday, on_day, day, at, hour, colon, minute) = (weekday.clone(), on_day.clone(), day.clone(), at.clone(), hour.clone(), colon.clone(), minute.clone());
        move |i: u32| {
            let key = EVERY.get(i as usize).map_or("", |e| e.0);
            weekday.set_visible(key == "week");
            on_day.set_visible(key == "month");
            day.set_visible(key == "month");
            for w in [at.upcast_ref::<gtk::Widget>(), hour.upcast_ref(), colon.upcast_ref(), minute.upcast_ref()] {
                w.set_visible(!key.is_empty());
            }
        }
    };
    show(every.selected());
    every.connect_selected_notify(move |k| show(k.selected()));
    let when = field(&body, "When it wakes", &line);
    when.append(&label("In this PC's time. A schedule that comes round while the PC is off runs when it is back, if not too late.", "money-row-detail"));

    // What wakes it in the data.
    let rules_list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let add_rule = button("Add a rule", "money-text-action");
    add_rule.set_halign(gtk::Align::Start);
    add_rule.set_widget_name("aide-edit-add-rule");
    let rules_block = gtk::Box::new(gtk::Orientation::Vertical, 6);
    rules_block.append(&rules_list);
    rules_block.append(&add_rule);
    let rules_field = field(&body, "What wakes it in your data", &rules_block);
    let rules_about = label("Relay checks a rule itself when the data changes, without a model, and wakes the job once for each thing it finds.", "money-row-detail");
    rules_about.set_wrap(true);
    rules_field.append(&rules_about);

    // What it reads.
    let reads = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let have: Vec<&str> = seed["sources"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let sources: Vec<(&'static str, gtk::CheckButton)> = SOURCES
        .iter()
        .map(|(key, name, about)| {
            let c = gtk::CheckButton::with_label(name);
            c.set_widget_name(&format!("aide-edit-source-{key}"));
            c.set_tooltip_text(Some(about));
            c.set_active(have.contains(key));
            reads.append(&c);
            (*key, c)
        })
        .collect();
    let reads_field = field(&body, "What it may read", &reads);
    reads_field.append(&label("A rule's own data is read too. Settings decide which models may read each.", "money-row-detail"));

    // Its model.
    let model_list = model_choices(models, seed["model"].as_str(), "Aide's default");
    let fallback_list = model_choices(models, seed["fallback"].as_str(), "None");
    let pick_of = |list: &[(Option<String>, String)], current: Option<&str>| list.iter().position(|(id, _)| id.as_deref() == current).unwrap_or(0);
    let model = picker(&model_list.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), pick_of(&model_list, seed["model"].as_str()), "aide-edit-model");
    model.set_hexpand(true);
    let fallback = picker(&fallback_list.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), pick_of(&fallback_list, seed["fallback"].as_str()), "aide-edit-fallback");
    fallback.set_hexpand(true);
    // One above the other: a server's name makes a wide caption.
    field(&body, "Model", &model);
    field(&body, "If it cannot run", &fallback);
    if let Some(note) = models_note {
        let l = label(&format!("Could not list the models: {note}"), "money-row-detail");
        l.set_wrap(true);
        body.append(&l);
    }

    // How loudly it speaks.
    let segments = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    segments.add_css_class("money-segments");
    segments.set_homogeneous(true);
    let level_about = label("", "money-row-detail");
    level_about.set_wrap(true);
    let current = seed["level"].as_str().unwrap_or("brief");
    let mut levels: Vec<(&'static str, gtk::ToggleButton)> = Vec::new();
    for (key, caption, about) in LEVELS {
        let t = gtk::ToggleButton::with_label(caption);
        t.set_tooltip_text(Some(about));
        t.set_widget_name(&format!("aide-edit-level-{key}"));
        if let Some((_, first)) = levels.first() {
            t.set_group(Some(first));
        }
        let shown = level_about.downgrade();
        t.connect_toggled(move |t| {
            if let Some(l) = shown.upgrade().filter(|_| t.is_active()) {
                l.set_text(about);
            }
        });
        segments.append(&t);
        levels.push((key, t));
    }
    if let Some((_, t)) = levels.iter().find(|(k, _)| *k == current).or(levels.get(1)) {
        t.set_active(true);
    }
    let speaks = field(&body, "How it speaks", &segments);
    speaks.append(&level_about);

    // Its limits.
    let runs = spin(1.0, 288.0, j_u(&seed["runs_per_day"], RUNS_PER_DAY), "aide-edit-runs");
    let minutes = spin(1.0, 120.0, j_u(&seed["minutes_per_run"], MINUTES_PER_RUN), "aide-edit-minutes");
    let pair_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    pair_row.set_homogeneous(true);
    let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
    field(&left, "Runs a day, at most", &runs);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let stops = field(&right, "Minutes a run", &minutes);
    stops.append(&label("Past it, the run is stopped.", "money-row-detail"));
    pair_row.append(&left);
    pair_row.append(&right);
    body.append(&pair_row);

    let form = Rc::new(Form {
        name: name.clone(),
        instructions,
        every,
        weekday,
        day,
        hour,
        minute,
        rules: RefCell::default(),
        sources,
        model,
        models: model_list,
        fallback,
        fallbacks: fallback_list,
        levels,
        runs,
        minutes,
        effort: seed["effort"].as_str().map(str::to_string),
    });
    for rule in seed["rules"].as_array().into_iter().flatten() {
        rule_row(&form, &rules_list, &rule_draft(rule));
    }
    let (weak, list) = (Rc::downgrade(&form), rules_list.downgrade());
    add_rule.connect_clicked(move |_| {
        if let (Some(form), Some(list)) = (weak.upgrade(), list.upgrade()) {
            rule_row(&form, &list, &RuleDraft { kind: 1, amount: String::new(), number: 90, category: 0 });
        }
    });

    let problem = problem_label();
    body.append(&problem);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let cancel = button("Cancel", "quiet");
    let save = button(if id.is_some() { "Save" } else { "Save and turn on" }, "primary");
    save.add_css_class("money-hero-action");
    save.set_widget_name("aide-edit-save");
    save.set_hexpand(true);
    actions.append(&cancel);
    actions.append(&save);
    body.append(&actions);

    // Unsaved while it differs from how the sheet opened: Escape, the scrim or × leave it open.
    let opened = form.snapshot();
    let changed = {
        let form = Rc::downgrade(&form);
        Rc::new(move || form.upgrade().is_some_and(|f| f.snapshot() != opened))
    };
    let permit = Rc::new(Cell::new(false));
    let (shown, allowed, unsaved) = (problem.downgrade(), permit.clone(), changed.clone());
    panel.set_guard(move || {
        if allowed.get() || !unsaved() {
            return true;
        }
        if let Some(problem) = shown.upgrade() {
            show_problem(&problem, "Save the job, or Cancel to discard it.");
        }
        false
    });
    let (sheet, allowed, unsaved) = (Rc::downgrade(panel), permit.clone(), changed.clone());
    crate::app::confirm_inline_if(&cancel, "Discard", move || unsaved(), move |_| {
        allowed.set(true);
        if let Some(sheet) = sheet.upgrade() {
            sheet.close();
        }
    });

    let (weak, sheet) = (Rc::downgrade(ui), Rc::downgrade(panel));
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let draft = match form.draft() {
            Ok(d) => d,
            Err(words) => return show_problem(&problem, &words),
        };
        let mut payload = json!({"spec": spec_value(&draft)});
        match id {
            Some(id) => payload["id"] = json!(id),
            None => payload["active"] = json!(true),
        }
        key.set_sensitive(false);
        problem.set_visible(false);
        let (key, problem, sheet, permit) = (key.clone(), problem.clone(), sheet.clone(), permit.clone());
        glib::spawn_future_local(async move {
            match ui.call("aide.job.save", payload).await {
                Ok(saved) => {
                    permit.set(true);
                    if let Some(sheet) = sheet.upgrade() {
                        sheet.close();
                    }
                    toast(&ui, if id.is_some() { "Saved." } else { "Saved and turned on." }, None);
                    match saved["id"].as_i64() {
                        Some(new) if id.is_none() => open_job(&ui, new),
                        _ => ui.refresh_page(),
                    }
                }
                Err(e) => {
                    key.set_sensitive(true);
                    show_problem(&problem, &said(&e));
                }
            }
        });
    });
    name.grab_focus();
}

fn j_u(v: &Value, default: u32) -> f64 {
    v.as_u64().map_or(f64::from(default), |n| n as f64)
}

// ---- Memory --------------------------------------------------------------------------------

fn memory_page(ui: &Rc<Ui>, page: &Page, facts: &[Value], pages: Option<&[Value]>) {
    heading(
        &page.head,
        "Memory",
        "What Aide has learned and keeps, and the pages it keeps current. What it guesses waits for you to confirm before any run is given it.",
        &[],
    );
    clear(&page.body);
    let pair = columns(&page.body, 2);
    let list = card(&pair[0], "What Aide knows", None);
    // Something to tell it, first.
    let add = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    add.add_css_class("tally-row");
    let entry = sheet_entry("aide-fact-new", "", "Something Aide should know: I'm paid on the 15th");
    entry.set_hexpand(true);
    let keep = button("Keep", "money-secondary");
    keep.set_widget_name("aide-fact-keep");
    add.append(&entry);
    add.append(&keep);
    list.append(&add);
    let (weak, typed) = (Rc::downgrade(ui), entry.clone());
    keep.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let words = typed.text().trim().to_string();
        if words.is_empty() {
            typed.grab_focus();
            return;
        }
        act(&ui, key, "aide.fact.add", json!({"text": words, "source": "person"}), "Kept. Runs are given it from now on.");
    });
    let key = keep.downgrade();
    entry.connect_activate(move |_| {
        if let Some(key) = key.upgrade().filter(|k| k.is_sensitive()) {
            key.emit_clicked();
        }
    });
    if facts.is_empty() {
        quiet_row(&list, "Nothing yet. Aide adds what it learns as it runs, and asks you to confirm it.");
    }
    for f in facts {
        list.append(&fact_row(ui, f));
    }
    // Every fact from one source, at once.
    let mut by_source: Vec<(String, usize)> = Vec::new();
    for f in facts {
        let s = text(f, "source").to_string();
        match by_source.iter_mut().find(|(seen, _)| *seen == s) {
            Some((_, n)) => *n += 1,
            None => by_source.push((s, 1)),
        }
    }
    if by_source.len() > 1 || by_source.first().is_some_and(|(_, n)| *n > 1) {
        let forget = card(&pair[0], "Forget by where it came from", None);
        for (source, n) in by_source {
            let key = op_key(ui, "Forget all", "money-destructive", "aide.fact.forget", json!({"source": source}), "Forgotten.", Some("Forget them"));
            setting_row(&forget, "trash", &format!("Everything from {}", from_words(&source)), &plural(n, "fact"), &key);
        }
    }

    let list = card(&pair[1], "Living pages", None);
    match pages {
        None => quiet_row(&list, "Its pages could not be read."),
        Some([]) => quiet_row(&list, "None yet. A living page answers one question, and each run of its job writes it again from current data."),
        Some(pages) => {
            for p in pages {
                list.append(&page_row(ui, p));
            }
        }
    }
}

fn fact_row(ui: &Rc<Ui>, f: &Value) -> gtk::Box {
    let id = f["id"].as_i64().unwrap_or(0);
    let (confirmed, pinned) = (f["confirmed"] == true, f["pinned"] == true);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.add_css_class("aide-row");
    row.append(&mark("fact"));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let shown = label(text(f, "text"), "money-row-title");
    shown.set_wrap(true);
    shown.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    words.append(&shown);
    let editing = sheet_entry(&format!("aide-fact-{id}"), text(f, "text"), "");
    editing.set_visible(false);
    words.append(&editing);
    let mut detail = Vec::new();
    if pinned {
        detail.push(String::from("Pinned"));
    }
    detail.push(source_word(text(f, "source")));
    detail.push(String::from(if confirmed { "Confirmed" } else { "Waits for you to confirm" }));
    detail.push(ago(text(f, "learned_at")));
    words.append(&label(&detail.join(" · "), "money-row-detail"));
    row.append(&words);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    keys.set_valign(gtk::Align::Center);
    if !confirmed {
        keys.append(&op_key(ui, "Confirm", "primary", "aide.fact.update", json!({"id": id, "confirmed": true}), "Confirmed.", None));
    }
    let pin = op_key(ui, if pinned { "Unpin" } else { "Pin" }, "quiet", "aide.fact.update", json!({"id": id, "pinned": !pinned}), if pinned { "Unpinned." } else { "Pinned: every run is given it." }, None);
    pin.set_tooltip_text(Some(if pinned { "Given to a run only when it is relevant" } else { "Give it to every run" }));
    keys.append(&pin);
    let edit = button("Edit", "quiet");
    let save = button("Save", "primary");
    save.set_visible(false);
    keys.append(&edit);
    keys.append(&save);
    keys.append(&op_key(ui, "Forget", "money-destructive", "aide.fact.forget", json!({"id": id}), "Forgotten.", Some("Forget it")));
    let mut child = keys.first_child();
    while let Some(w) = child {
        w.add_css_class("arbiter-small");
        child = w.next_sibling();
    }
    row.append(&keys);
    let (s, e, k) = (shown.downgrade(), editing.downgrade(), save.downgrade());
    edit.connect_clicked(move |edit| {
        let (Some(shown), Some(editing), Some(save)) = (s.upgrade(), e.upgrade(), k.upgrade()) else { return };
        shown.set_visible(false);
        editing.set_visible(true);
        editing.grab_focus();
        edit.set_visible(false);
        save.set_visible(true);
    });
    let (weak, e) = (Rc::downgrade(ui), editing.downgrade());
    save.connect_clicked(move |key| {
        let (Some(ui), Some(editing)) = (weak.upgrade(), e.upgrade()) else { return };
        act(&ui, key, "aide.fact.update", json!({"id": id, "text": editing.text().trim()}), "Saved.");
    });
    let key = save.downgrade();
    editing.connect_activate(move |_| {
        if let Some(key) = key.upgrade() {
            key.emit_clicked();
        }
    });
    row
}

fn page_row(ui: &Rc<Ui>, p: &Value) -> gtk::Box {
    let id = p["id"].as_i64().unwrap_or(0);
    let detail = format!("{} · written {}", text(p, "question"), ago(text(p, "updated_at")));
    let line = super::pages::icon_row(&mark("page"), text(p, "title"), &detail, "", "");
    let weak = Rc::downgrade(ui);
    let run: Action = Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            read_page(&ui, id);
        }
    });
    super::pages::row_key(&line, "Read this page", run)
}

/// A living page, read whole in a sheet: its question, when it was written, its Markdown, Remove.
fn read_page(ui: &Rc<Ui>, id: i64) {
    let Some(panel) = Panel::toggle(ui, "Living page", 640) else { return };
    panel.add_css_class("money-sheet");
    panel.add_css_class("aide-sheet");
    let loading = label("Reading…", "money-muted");
    panel.body.append(&loading);
    panel.present();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let read = ui.call("aide.page.get", json!({"id": id})).await;
        panel.body.remove(&loading);
        let p = match read {
            Ok(p) => p,
            Err(e) => {
                let l = label(&said(&e), "money-problem");
                l.set_wrap(true);
                panel.body.append(&l);
                return;
            }
        };
        let title = label(text(&p, "title"), "money-title");
        title.set_wrap(true);
        panel.body.append(&title);
        let q = label(&format!("{} · written {}", text(&p, "question"), ago(text(&p, "updated_at"))), "money-context");
        q.set_wrap(true);
        panel.body.append(&q);
        let page = gtk::Box::new(gtk::Orientation::Vertical, 10);
        page.add_css_class("aide-page-body");
        if text(&p, "body").trim().is_empty() {
            page.append(&label("Not written yet: its job's next run writes it.", "money-muted"));
        } else {
            super::threads::draw_reply(&page, text(&p, "body"));
        }
        panel.body.append(&page);
        let remove = button("Remove page", "money-destructive");
        remove.set_halign(gtk::Align::Start);
        let (weak, sheet) = (Rc::downgrade(&ui), Rc::downgrade(&panel));
        confirm_inline(&remove, "Remove it", move |key| {
            let Some(ui) = weak.upgrade() else { return };
            key.set_sensitive(false);
            let sheet = sheet.clone();
            glib::spawn_future_local(async move {
                match ui.call("aide.page.delete", json!({"id": id})).await {
                    Ok(_) => {
                        if let Some(sheet) = sheet.upgrade() {
                            sheet.close();
                        }
                        toast(&ui, "Removed.", None);
                    }
                    Err(e) => ui.show_error(&said(&e)),
                }
                ui.refresh_page();
            });
        });
        panel.body.append(&remove);
    });
}

// ---- Settings ------------------------------------------------------------------------------

fn settings_page(ui: &Rc<Ui>, page: &Page, s: &Value, models: Result<Vec<Value>, String>) {
    let save = button("Save", "primary");
    save.set_widget_name("aide-settings-save");
    heading(&page.head, "Settings", "Which models Aide runs on, what each may read, and when it may interrupt you.", &[save.upcast_ref()]);
    clear(&page.body);
    let problem = problem_label();
    page.body.append(&problem);
    let listed = models.as_deref().unwrap_or(&[]);
    let pair = columns(&page.body, 2);

    let list = card(&pair[0], "Models", None);
    let model_list = model_choices(listed, s["model"].as_str(), "Claude's default");
    let fallback_list = model_choices(listed, s["fallback"].as_str(), "None");
    let at = |l: &[(Option<String>, String)], c: Option<&str>| l.iter().position(|(id, _)| id.as_deref() == c).unwrap_or(0);
    let model = picker(&model_list.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), at(&model_list, s["model"].as_str()), "aide-settings-model");
    setting_row(&list, "spark", "Default model", "A job that names no model runs on this.", &model);
    let fallback = picker(&fallback_list.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), at(&fallback_list, s["fallback"].as_str()), "aide-settings-fallback");
    setting_row(&list, "repeat", "If it cannot run", "Tried when the default is not loaded, not answering, or not allowed to read a job's data.", &fallback);
    if let Err(e) = &models {
        quiet_row(&list, &format!("Could not list the models: {e}"));
    }

    let list = card(&pair[0], "What each model may read", None);
    let b = block(&list);
    let about = label(
        "\u{201c}Your own models only\u{201d} means only a model on this PC or on your own server reads it: it never goes to Claude, and a job that would need Claude for it does not run. Off means no model reads it.",
        "money-muted",
    );
    about.set_wrap(true);
    b.append(&about);
    let tier_captions: Vec<&str> = TIERS.iter().map(|t| t.1).collect();
    let tiers: Vec<gtk::DropDown> = SOURCES
        .iter()
        .map(|(key, name, about)| {
            let current = s["tiers"][key].as_str().unwrap_or("any");
            let d = picker(&tier_captions, TIERS.iter().position(|t| t.0 == current).unwrap_or(0), &format!("aide-tier-{key}"));
            setting_row(&list, "insurance", name, about, &d);
            d
        })
        .collect();

    let list = card(&pair[0], "Quiet hours and interrupts", None);
    let quiet_start = clock_spin(23.0, j_u(&s["quiet_start"], 22), "aide-quiet-start");
    let quiet_end = clock_spin(23.0, j_u(&s["quiet_end"], 7), "aide-quiet-end");
    let hours = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    hours.append(&quiet_start);
    hours.append(&words(":00 to"));
    hours.append(&quiet_end);
    hours.append(&words(":00"));
    setting_row(&list, "calendar", "Quiet hours", "Interrupts wait for the brief. The same hour twice means none.", &hours);
    let cap = spin(0.0, 100.0, j_u(&s["interrupt_cap"], 6), "aide-interrupt-cap");
    setting_row(&list, "gauge", "Interrupts a day", "Past this, what Aide has to say waits for the brief.", &cap);

    // Your own servers, each with whether it answered just now.
    let servers = &rows(s, "endpoints");
    let weak = Rc::downgrade(ui);
    let add: Action = Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            server_sheet(&ui, None);
        }
    });
    let list = card(&pair[1], "Your servers", Some(("Add a server", add)));
    if servers.is_empty() {
        quiet_row(&list, "None yet. A server is anything that answers Anthropic's Messages API on your own machine: llama.cpp's server, vLLM, or LiteLLM in front of one.");
    }
    for e in servers {
        list.append(&server_row(ui, e, listed));
    }

    let list = card(&pair[1], "What Aide never does", None);
    for line in REFUSED {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("tally-row");
        row.append(&mark("never"));
        let l = label(line, "money-row-title");
        l.set_wrap(true);
        l.set_hexpand(true);
        row.append(&l);
        list.append(&row);
    }
    quiet_row(&list, "Relay refuses these to it whatever a job says. It asks you before a new job, and a fact it guesses waits for you.");

    let (weak, read) = (Rc::downgrade(ui), s.clone());
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let mut chosen = ["any"; 4];
        for (i, d) in tiers.iter().enumerate() {
            chosen[i] = TIERS.get(d.selected() as usize).map_or("any", |t| t.0);
        }
        let form = SettingsForm {
            model: model_list.get(model.selected() as usize).and_then(|m| m.0.clone()),
            fallback: fallback_list.get(fallback.selected() as usize).and_then(|m| m.0.clone()),
            tiers: chosen,
            quiet_start: quiet_start.value_as_int() as u8,
            quiet_end: quiet_end.value_as_int() as u8,
            cap: cap.value_as_int().max(0) as u32,
        };
        let body = settings_body(&read, Some(&form), None);
        key.set_sensitive(false);
        problem.set_visible(false);
        let (key, problem) = (key.clone(), problem.clone());
        glib::spawn_future_local(async move {
            match ui.call("aide.settings.set", body).await {
                Ok(_) => {
                    toast(&ui, "Saved.", None);
                    ui.refresh_page();
                }
                Err(e) => {
                    key.set_sensitive(true);
                    show_problem(&problem, &said(&e));
                }
            }
        });
    });
}

/// A server: whether it answered, its name, where it is and what it runs; Edit and Remove.
fn server_row(ui: &Rc<Ui>, e: &Value, models: &[Value]) -> gtk::Box {
    let id = text(e, "id").to_string();
    let found = models.iter().find(|m| text(m, "id") == id && text(m, "host") == "server");
    let (tone, tip) = match found {
        Some(m) if m["available"] == true => ("arbiter-live", String::from("It answered just now")),
        Some(m) => ("arbiter-halted", m["note"].as_str().map_or_else(|| String::from("It did not answer"), str::to_string)),
        None => ("", String::from("Not asked yet")),
    };
    let dot = lamp(tone);
    dot.set_tooltip_text(Some(&tip));
    let mut detail = vec![id.clone(), text(e, "url").to_string()];
    if let Some(m) = e["model"].as_str() {
        detail.push(m.to_string());
    }
    detail.push(format!("{} tokens", super::avex::group(e["context"].as_i64().unwrap_or(32_768))));
    let edit = button("Edit", "quiet");
    let (weak, shown) = (Rc::downgrade(ui), e.clone());
    edit.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            server_sheet(&ui, Some(&shown));
        }
    });
    let remove = button("Remove", "money-destructive");
    let weak = Rc::downgrade(ui);
    confirm_inline(&remove, "Remove it", move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let id = id.clone();
        glib::spawn_future_local(async move {
            let result = match ui.call("aide.settings.get", json!({})).await {
                Ok(read) => {
                    let kept: Vec<Value> = rows(&read, "endpoints").into_iter().filter(|e| text(e, "id") != id).collect();
                    ui.call("aide.settings.set", settings_body(&read, None, Some(&kept))).await
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(_) => toast(&ui, "Removed. A job that named it runs on its fallback.", None),
                Err(e) => ui.show_error(&said(&e)),
            }
            ui.refresh_page();
        });
    });
    let name = if text(e, "name").is_empty() { text(e, "id") } else { text(e, "name") };
    let row = ask_row("server", name, &detail.join(" · "), &[edit, remove]);
    row.insert_child_after(&dot, None::<&gtk::Widget>);
    row
}

/// Add a server, or change one: its id, name, address, model name and context.
fn server_sheet(ui: &Rc<Ui>, editing: Option<&Value>) {
    let Some(panel) = Panel::toggle(ui, if editing.is_some() { "Edit a server" } else { "Add a server" }, 480) else { return };
    panel.add_css_class("money-sheet");
    panel.add_css_class("aide-sheet");
    let body = panel.body.clone();
    let e = editing.cloned().unwrap_or(Value::Null);
    let about = label("Anything that answers Anthropic's Messages API: llama.cpp's server, vLLM, or LiteLLM in front of one. Aide's runs on it never go to Claude.", "money-muted");
    about.set_wrap(true);
    body.append(&about);
    let id = sheet_entry("aide-server-id", text(&e, "id"), "farm-qwen");
    let id_field = field(&body, "Id", &id);
    id_field.append(&label("What a job names: letters, digits, - and _.", "money-row-detail"));
    let name = sheet_entry("aide-server-name", text(&e, "name"), "Qwen on the farm");
    field(&body, "Name", &name);
    let url = sheet_entry("aide-server-url", text(&e, "url"), "http://10.0.0.5:8080");
    field(&body, "Address", &url);
    let model = sheet_entry("aide-server-model", text(&e, "model"), "The id, when empty");
    field(&body, "Model name", &model);
    let context = sheet_entry("aide-server-context", &e["context"].as_u64().map(|c| c.to_string()).unwrap_or_default(), "32768");
    field(&body, "Context, in tokens", &context);
    let vision = gtk::CheckButton::with_label("It reads pictures");
    vision.set_active(e["vision"] == true);
    body.append(&vision);
    let problem = problem_label();
    body.append(&problem);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let cancel = button("Cancel", "quiet");
    let save = button("Save", "primary");
    save.set_widget_name("aide-server-save");
    save.set_hexpand(true);
    actions.append(&cancel);
    actions.append(&save);
    body.append(&actions);
    let sheet = Rc::downgrade(&panel);
    cancel.connect_clicked(move |_| {
        if let Some(sheet) = sheet.upgrade() {
            sheet.close();
        }
    });
    let was = editing.map(|e| text(e, "id").to_string());
    let (weak, sheet) = (Rc::downgrade(ui), Rc::downgrade(&panel));
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let server = match endpoint_value(&id.text(), &name.text(), &url.text(), &model.text(), &context.text(), vision.is_active()) {
            Ok(v) => v,
            Err(words) => return show_problem(&problem, &words),
        };
        key.set_sensitive(false);
        problem.set_visible(false);
        let (key, problem, sheet, was) = (key.clone(), problem.clone(), sheet.clone(), was.clone());
        glib::spawn_future_local(async move {
            let result = match ui.call("aide.settings.get", json!({})).await {
                Ok(read) => {
                    let mut list = rows(&read, "endpoints");
                    match list.iter().position(|e| Some(text(e, "id")) == was.as_deref()) {
                        Some(i) => list[i] = server,
                        None => list.push(server),
                    }
                    ui.call("aide.settings.set", settings_body(&read, None, Some(&list))).await
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(_) => {
                    if let Some(sheet) = sheet.upgrade() {
                        sheet.close();
                    }
                    toast(&ui, "Saved. Its dot says whether it answers.", None);
                    ui.refresh_page();
                }
                Err(e) => {
                    key.set_sensitive(true);
                    show_problem(&problem, &said(&e));
                }
            }
        });
    });
    panel.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wake_reads_as_a_few_words() {
        assert_eq!(until_words(-3), "now");
        assert_eq!(until_words(25), "in 25 min");
        assert_eq!(until_words(60), "in 1 h");
        assert_eq!(until_words(9 * 60 + 10), "in 9 h");
        assert_eq!(until_words(3 * 1440), "in 3 days");
        assert_eq!(next_wake("Morning brief", Some("weekdays at 07:00"), Some(540)), "Morning brief · weekdays at 07:00 · in 9 h");
        assert_eq!(next_wake("Budget", None, None), "Budget");
    }

    #[test]
    fn a_schedule_sends_only_its_own_fields() {
        let week = Clock { every: 3, weekday: 5, day: 12, hour: 18, minute: 30 };
        assert_eq!(schedule_value(&week), Some(json!({"every": "week", "hour": 18, "minute": 30, "weekday": 5})));
        let month = Clock { every: 4, ..week };
        assert_eq!(schedule_value(&month), Some(json!({"every": "month", "hour": 18, "minute": 30, "day": 12})));
        assert_eq!(schedule_value(&Clock { every: 2, ..week }), Some(json!({"every": "weekdays", "hour": 18, "minute": 30})));
        assert_eq!(schedule_value(&Clock::default()), None);
        // A week's schedule keeps no day of the month: the editor shows the 1st again.
        assert_eq!(clock_of(&schedule_value(&week).unwrap()), Clock { day: 1, ..week });
        assert_eq!(clock_of(&schedule_value(&month).unwrap()), Clock { weekday: 1, ..month });
        assert_eq!(clock_of(&Value::Null), Clock::default());
    }

    #[test]
    fn a_rule_row_reads_back_what_it_sent() {
        for rule in [
            json!({"kind": "spend", "over": 200.0}),
            json!({"kind": "budget", "percent": 90}),
            json!({"kind": "stuck", "minutes": 45}),
            json!({"kind": "notice", "category": "guardrail"}),
            json!({"kind": "arbiter"}),
            json!({"kind": "workout"}),
        ] {
            assert_eq!(rule_value(&rule_draft(&rule)).unwrap(), rule);
        }
        assert_eq!(rule_value(&RuleDraft { kind: 0, amount: "1 250,50 $".into(), number: 0, category: 0 }).unwrap(), json!({"kind": "spend", "over": 1250.5}));
        assert!(rule_value(&RuleDraft { kind: 0, amount: "lots".into(), number: 0, category: 0 }).is_err());
        assert!(rule_value(&RuleDraft { kind: 0, amount: "0".into(), number: 0, category: 0 }).is_err());
    }

    fn draft() -> Draft {
        Draft {
            name: " Morning brief ".into(),
            instructions: "Tell me what happened overnight. ".into(),
            clock: Clock { every: 2, ..Clock::default() },
            rules: vec![json!({"kind": "budget", "percent": 90})],
            sources: vec!["dev", "money"],
            model: Some("sonnet".into()),
            fallback: Some("  ".into()),
            effort: None,
            level: "interrupt",
            runs_per_day: RUNS_PER_DAY,
            minutes_per_run: 20,
        }
    }

    #[test]
    fn a_draft_is_sent_as_a_job_spec_without_defaults() {
        let spec = spec_value(&draft());
        assert_eq!(
            spec,
            json!({
                "name": "Morning brief", "instructions": "Tell me what happened overnight.",
                "schedule": {"every": "weekdays", "hour": 7, "minute": 0},
                "rules": [{"kind": "budget", "percent": 90}], "sources": ["dev", "money"],
                "model": "sonnet", "level": "interrupt", "minutes_per_run": 20,
            })
        );
        let bare = spec_value(&Draft { clock: Clock::default(), rules: vec![], model: None, minutes_per_run: MINUTES_PER_RUN, ..draft() });
        let mut keys: Vec<&String> = bare.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["instructions", "level", "name", "sources"]);
    }

    #[test]
    fn model_choices_start_with_none_and_keep_one_not_listed() {
        let models = [json!({"id": "sonnet", "name": "Sonnet", "host": "claude", "available": true}), json!({"id": "farm", "name": "", "host": "server", "available": false})];
        let c = model_choices(&models, Some("qwen-9b"), "Aide's default");
        assert_eq!(c[0], (None, String::from("Aide's default")));
        assert_eq!(c[1], (Some("sonnet".into()), String::from("Sonnet")));
        assert_eq!(c[2], (Some("farm".into()), String::from("farm · Your server · not available")));
        assert_eq!(c[3], (Some("qwen-9b".into()), String::from("qwen-9b")));
        assert_eq!(model_choices(&models, Some("sonnet"), "None").len(), 3);
    }

    #[test]
    fn settings_are_sent_whole_with_exactly_their_fields() {
        let read = json!({
            "paused": true, "model": "sonnet", "quiet_start": 23, "quiet_end": 6, "interrupt_cap": 4,
            "tiers": {"dev": "any", "money": "local_only", "arbiter": "off", "gym": "any"},
            "endpoints": [{"id": "farm", "name": "Farm", "url": "http://10.0.0.5:8080", "context": 65536, "vision": false, "seen": "x"}],
            "something_new": 1,
        });
        let kept = settings_body(&read, None, None);
        assert_eq!(
            kept,
            json!({
                "paused": true, "model": "sonnet", "quiet_start": 23, "quiet_end": 6, "interrupt_cap": 4,
                "tiers": {"dev": "any", "money": "local_only", "arbiter": "off", "gym": "any"},
                "endpoints": [{"id": "farm", "name": "Farm", "url": "http://10.0.0.5:8080", "context": 65536, "vision": false}],
            })
        );
        let form = SettingsForm { model: None, fallback: Some("qwen".into()), tiers: ["off", "any", "any", "local_only"], quiet_start: 22, quiet_end: 22, cap: 0 };
        let chosen = settings_body(&read, Some(&form), Some(&[]));
        assert!(chosen.get("model").is_none());
        assert_eq!(chosen["fallback"], "qwen");
        assert_eq!(chosen["paused"], true, "the form never changes paused");
        assert_eq!(chosen["tiers"]["gym"], "local_only");
        assert_eq!(chosen["endpoints"], json!([]));
        assert_eq!((chosen["quiet_start"].clone(), chosen["interrupt_cap"].clone()), (json!(22), json!(0)));
        // An engine that has never saved them reads its own defaults back.
        let fresh = settings_body(&json!({}), None, None);
        assert_eq!((fresh["quiet_start"].clone(), fresh["quiet_end"].clone(), fresh["interrupt_cap"].clone()), (json!(22), json!(7), json!(6)));
        assert_eq!(fresh["tiers"]["money"], "any");
    }

    #[test]
    fn a_server_sheet_says_what_is_missing() {
        assert_eq!(
            endpoint_value(" farm ", "", "http://h:8080", "", "32,768", true).unwrap(),
            json!({"id": "farm", "name": "", "url": "http://h:8080", "vision": true, "context": 32768})
        );
        assert_eq!(endpoint_value("farm", "Farm", "http://h:1", "qwen", "", false).unwrap()["model"], "qwen");
        assert!(endpoint_value("", "", "http://h:1", "", "", false).is_err());
        assert!(endpoint_value("farm", "", " ", "", "", false).is_err());
        assert!(endpoint_value("farm", "", "http://h:1", "", "big", false).is_err());
    }

    #[test]
    fn a_wake_folds_to_its_first_lines() {
        assert_eq!(folded("[Aide · run 1]\n\nYour job:\nTell me.\n\nMore\nAnd more"), Some(String::from("[Aide · run 1]\nYour job:\nTell me…")));
        assert_eq!(folded("Short.\nTwo lines."), None);
        let long = "x".repeat(300);
        assert_eq!(folded(&long).map(|f| f.chars().count()), Some(FOLD_CHARS + 1));
    }

    #[test]
    fn words_for_sources_runs_and_messages() {
        assert_eq!(source_word("money"), "Tally");
        assert_eq!(source_word("person"), "You");
        assert_eq!(source_word("thread:12"), "A run");
        assert_eq!(source_word("elsewhere"), "elsewhere");
        assert_eq!(from_words("person"), "you");
        assert_eq!(from_words("gym"), "Avex");
        assert_eq!(host_word("pc"), "This PC");
        assert_eq!(message_status(true, false, false), "Notified");
        assert_eq!(message_status(false, true, false), "Held for the brief");
        assert_eq!(message_status(false, true, true), "In the brief");
        assert_eq!(message_status(false, false, false), "");
        assert_eq!(when_line(&json!({"when": ["weekdays at 07:00", "a budget past 90% of its month"]})), "Weekdays at 07:00 · a budget past 90% of its month");
        assert_eq!(when_line(&json!({"when": []})), "Runs only when you press Run");
        assert_eq!(model_line(&json!({"fallback": "haiku"})), "Aide's default, then haiku");
        assert_eq!(when_of_spec(&json!({"schedule": {"every": "week", "hour": 17, "minute": 0, "weekday": 5}})), "Every Friday at 17:00.");
    }
}
