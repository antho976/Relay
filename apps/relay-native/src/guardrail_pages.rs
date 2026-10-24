//! Guardrail surfaces: held actions in words rather than raw JSON, the exception requests a
//! refused agent sends (with the prompt that puts one in front of the user wherever they are),
//! the grants still in force, and the layered settings editor.
use crate::app::{button, clear, label, rows, text, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[path = "guardrail_settings.rs"]
mod settings;
pub use settings::{editor as settings_editor, open_project_guardrails, open_workspace_guardrails};

const TRAY: &str = "guardrail-requests";
/// More than this many prompts at once is a queue, and the Guardrails page is the queue.
const TRAY_LIMIT: usize = 3;
/// How long a prompt takes to slide in, and to slide away once it is answered.
const PROMPT_IN_MS: u32 = 260;
const PROMPT_OUT_MS: u32 = 180;
/// How long a prompt's Approve keys stay off once it has slid fully in, as a browser's
/// permission prompt does: it arrives unannounced, under wherever the pointer was aimed.
const ARM_MS: u64 = 500;
/// The name a prompt takes while it slides away, so it no longer counts as showing.
const LEAVING: &str = "guardrail-leaving";

fn paragraph(value: &str, class: &str) -> gtk::Label {
    let l = label(value, class);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_selectable(true);
    l
}

fn mono(value: &str) -> gtk::Label {
    let l = paragraph(value, "guardrail-subject");
    l.add_css_class("mono");
    l
}

/// The color hidden-character escapes and their card's warning are drawn in: theme.css's @held.
/// Markup, not a style class, so the escape stands out inside a label of ordinary text.
const HIDDEN_COLOR: &str = "#e5382e";

/// A character that changes how text reads without showing itself: bidi embeddings, overrides,
/// isolates and marks, zero-width and other invisible format characters, the separators a label
/// breaks lines on, and every control character but a newline.
fn is_hidden(c: char) -> bool {
    matches!(c,
        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
        | '\u{200B}'..='\u{200D}' | '\u{2060}'..='\u{2064}' | '\u{206A}'..='\u{206F}' | '\u{FEFF}'
        | '\u{00AD}' | '\u{034F}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}' | '\u{180B}'..='\u{180F}'
        | '\u{2028}' | '\u{2029}' | '\u{3164}' | '\u{FFA0}' | '\u{FE00}'..='\u{FE0F}' | '\u{FFF9}'..='\u{FFFB}'
        | '\u{E0000}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}')
        || (c.is_control() && c != '\n')
}

/// Agent-supplied text made safe to read: each hidden character replaced by a visible escape.
#[derive(Debug, PartialEq)]
struct Revealed {
    /// For a plain-text label or a notice: `<U+202E>` in place of the character.
    plain: String,
    /// The same as Pango markup, the escapes highlighted so a literal `<U+202E>` reads apart.
    markup: String,
    /// How many characters were replaced.
    hidden: usize,
}

/// Every agent-supplied string a guardrail card shows passes through here. Pango applies the
/// bidi algorithm to label text and draws zero-width characters as nothing, so a command or a
/// path could otherwise read differently from the bytes being approved.
fn reveal(value: &str) -> Revealed {
    let mut out = Revealed { plain: String::with_capacity(value.len()), markup: String::new(), hidden: 0 };
    let mut run = String::new();
    for c in value.chars() {
        if !is_hidden(c) {
            out.plain.push(c);
            run.push(c);
            continue;
        }
        let escape = format!("<U+{:04X}>", c as u32);
        out.markup.push_str(&glib::markup_escape_text(&run));
        run.clear();
        out.markup.push_str(&format!(
            "<span foreground=\"{HIDDEN_COLOR}\" weight=\"bold\">{}</span>",
            glib::markup_escape_text(&escape)
        ));
        out.plain.push_str(&escape);
        out.hidden += 1;
    }
    out.markup.push_str(&glib::markup_escape_text(&run));
    out
}

/// The line a card shows, hidden until needed, when agent text in it carried hidden characters.
fn hidden_flag() -> gtk::Label {
    let flag = label("", "body");
    flag.set_markup(&format!(
        "<span foreground=\"{HIDDEN_COLOR}\" weight=\"bold\">Contains hidden characters.</span> \
         They are shown below as &lt;U+…&gt;; what was sent may read differently from what it does."
    ));
    flag.set_wrap(true);
    flag.set_visible(false);
    flag
}

/// Put agent-supplied `value` in `target` with its hidden characters escaped, showing the
/// card's `flag` when there were any.
fn set_agent_text(target: &gtk::Label, value: &str, flag: &gtk::Label) {
    let shown = reveal(value);
    target.set_markup(&shown.markup);
    if shown.hidden > 0 {
        flag.set_visible(true);
    }
}

fn agent_paragraph(value: &str, class: &str, flag: &gtk::Label) -> gtk::Label {
    let l = paragraph("", class);
    set_agent_text(&l, value, flag);
    l
}

fn agent_mono(value: &str, flag: &gtk::Label) -> gtk::Label {
    let l = agent_paragraph(value, "guardrail-subject", flag);
    l.add_css_class("mono");
    l
}

/// The most of one agent-supplied string a card puts in a label. Nothing bounds a reason,
/// a value or a held write's text, and a wrapping label of megabytes stalls every relayout.
const SHOWN_CHARS: usize = 4000;

/// `value` cut to [`SHOWN_CHARS`], saying how much was left out.
fn clip(value: &str) -> String {
    match value.char_indices().nth(SHOWN_CHARS) {
        None => value.to_owned(),
        Some((at, _)) => format!("{}… [{} more bytes not shown]", &value[..at], value.len() - at),
    }
}

/// `value` with every long string clipped, for showing a frozen payload.
fn clip_strings(value: &Value) -> Value {
    match value {
        Value::String(s) => Value::String(clip(s)),
        Value::Array(items) => Value::Array(items.iter().map(clip_strings).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), clip_strings(v))).collect()),
        other => other.clone(),
    }
}

/// "4 min ago" from an RFC 3339 timestamp; empty when it does not parse.
fn ago(ts: &str) -> String {
    crate::relative::ago(ts, crate::relative::Form::Long).unwrap_or_default()
}

fn is_request(hold: &Value) -> bool {
    text(hold, "op") == "guardrail.request"
}

/// A request in the shape `guardrail.requests.list` returns, from either that or a hold row.
fn as_request(item: &Value) -> Value {
    if item.get("kind").is_some() {
        return item.clone();
    }
    let details = &item["details"];
    json!({
        "id": item["id"], "project_id": item["project_id"], "session": item["session"],
        "kind": details["kind"], "value": details["value"], "reason": details["reason"],
        "requested_scope": details["requested_scope"], "state": item["state"],
        "created_at": item["created_at"],
    })
}

fn kind_verb(kind: &str) -> &'static str {
    match kind {
        "command" => "run",
        "path" => "write to",
        _ => "commit past the change caps",
    }
}

fn scope_phrase(scope: &str) -> &'static str {
    if scope == "session" { "for the rest of the session" } else { "once" }
}

/// What a policy is called where a person reads it.
fn policy_title(policy: &str) -> &'static str {
    match policy {
        "exception" => "Exception request",
        "protected_path" => "Protected path",
        "destructive_write" => "Large rewrite",
        "shape_gate" => "File shape check",
        "write_root" => "Write outside the worktree",
        "denied_command" => "Denied command",
        "cap" => "Commit over the size caps",
        _ => "Guardrail hold",
    }
}

/// The policy that actually stopped the action. A hold the user tripped wraps the original.
fn effective(hold: &Value) -> (String, Value) {
    let details = &hold["details"];
    if details["original"].is_object() {
        (text(details, "policy").to_string(), details["original"].clone())
    } else {
        (text(hold, "policy").to_string(), details.clone())
    }
}

fn number(value: &Value) -> String {
    match value.as_f64() {
        Some(n) if n.fract() == 0.0 => format!("{n:.0}"),
        Some(n) => format!("{n:.1}"),
        None => "?".into(),
    }
}

/// One sentence saying why the action is waiting.
fn explain(policy: &str, details: &Value) -> String {
    let path = text(details, "path");
    match policy {
        "destructive_write" if details["reason"] == "comparison_cap" => {
            format!("{path} is too large to compare safely, so Relay cannot tell how much of it would be lost.")
        }
        "destructive_write" => {
            let share = details["removed_pct"]
                .as_f64()
                .map(|pct| format!(" ({pct:.0}% of {} lines)", number(&details["old_lines"])))
                .unwrap_or_default();
            // The engine asks git only when "Allow rewrites git can restore" is on, and the
            // hold does not say whether it asked: claim git cannot help only when it says so.
            let git = match &details["recoverable"] {
                Value::Bool(false) => ", and git cannot restore what is there now",
                _ => "",
            };
            format!(
                "Removes {} lines{share} and adds {}. The limit is {} lines or {}% of a file{git}.",
                number(&details["removed_lines"]), number(&details["added_lines"]),
                number(&details["limit_lines"]), number(&details["limit_pct"]),
            )
        }
        "shape_gate" => format!(
            "{path} must pass the {} check: {}.",
            text(details, "validator").replace('_', " "),
            text(details, "reason"),
        ),
        "protected_path" => format!("{path} is protected by the pattern {}.", details["pattern"]),
        "write_root" => format!("{path} is outside every directory this session may write to."),
        "denied_command" => format!("Runs {}, which is on the denied list.", details["pattern"]),
        "cap" => format!(
            "Changes {} files and {} lines; the caps are {} files and {} lines.",
            number(&details["files"]), number(&details["lines"]),
            number(&details["cap_files"]), number(&details["cap_lines"]),
        ),
        _ => String::new(),
    }
}

/// The thing being acted on, for the meta line.
fn subject(hold: &Value, details: &Value) -> String {
    let op = text(hold, "op");
    if let Some(command) = details["command"].as_str() {
        return command.to_string();
    }
    if let Some(path) = details["path"].as_str() {
        return path.to_string();
    }
    match op {
        "guardrail.gate" | "" => String::new(),
        other => other.to_string(),
    }
}

/// The label/value pairs worth showing under "Details", in reading order.
fn detail_pairs(details: &Value) -> Vec<(String, String)> {
    const NAMES: &[(&str, &str)] = &[
        ("path", "Path"), ("command", "Command"), ("pattern", "Matched rule"), ("validator", "Check"),
        ("reason", "Why"), ("removed_lines", "Lines removed"), ("added_lines", "Lines added"),
        ("old_lines", "Lines in the file"), ("removed_pct", "Share removed (%)"),
        ("limit_lines", "Line limit"), ("limit_pct", "Share limit (%)"), ("files", "Files changed"),
        ("lines", "Lines changed"), ("cap_files", "File cap"), ("cap_lines", "Line cap"),
        ("bytes", "Size (bytes)"),
    ];
    NAMES
        .iter()
        .filter_map(|(key, name)| {
            let value = details.get(*key)?;
            let shown = match value {
                Value::Null => return None,
                Value::String(s) => s.clone(),
                Value::Number(_) => number(value),
                other => other.to_string(),
            };
            Some((name.to_string(), shown))
        })
        .collect()
}

fn details_grid(details: &Value, flag: &gtk::Label) -> Option<gtk::Expander> {
    let pairs = detail_pairs(details);
    if pairs.is_empty() {
        return None;
    }
    let grid = gtk::Grid::builder().column_spacing(16).row_spacing(4).build();
    grid.add_css_class("guardrail-details");
    for (row, (name, value)) in pairs.iter().enumerate() {
        grid.attach(&label(name, "faint"), 0, row as i32, 1, 1);
        let value = agent_paragraph(&clip(value), "body", flag);
        value.set_hexpand(true);
        grid.attach(&value, 1, row as i32, 1, 1);
    }
    let expander = gtk::Expander::new(Some("Details"));
    expander.add_css_class("guardrail-expander");
    expander.set_child(Some(&grid));
    Some(expander)
}

/// A card's state as a chip (the rule that held it, or "Exception request"), and its age.
fn head(eyebrow: &str, when: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    chip.add_css_class("guardrail-state");
    chip.set_halign(gtk::Align::Start);
    chip.set_hexpand(true);
    let mark = crate::icons::image("shield", 13);
    mark.add_css_class("guardrail-mark");
    chip.append(&mark);
    let title = label(eyebrow, "guardrail-eyebrow");
    chip.append(&title);
    let holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    holder.set_hexpand(true);
    holder.append(&chip);
    row.append(&holder);
    if !when.is_empty() {
        row.append(&label(when, "faint"));
    }
    row
}

/// A held action, in words: what it was, why it stopped, and what you can do.
pub fn hold_row(ui: &Rc<Ui>, hold: Value) -> gtk::Box {
    let (policy, details) = effective(&hold);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("record");
    card.add_css_class("guardrail-card");
    card.append(&head(policy_title(&policy), &ago(text(&hold, "created_at"))));
    let flag = hidden_flag();
    card.append(&flag);
    let who = text(&hold, "session");
    let what = subject(&hold, &details);
    let heading = label("", "title");
    set_agent_text(
        &heading,
        &if who.is_empty() || text(&hold, "actor") == "user" {
            "Your own action is held".to_string()
        } else {
            format!("{who} is waiting on you")
        },
        &flag,
    );
    card.append(&heading);
    if !what.is_empty() {
        card.append(&agent_mono(&clip(&what), &flag));
    }
    let why = explain(&policy, &details);
    if !why.is_empty() {
        card.append(&agent_paragraph(&clip(&why), "body", &flag));
    }
    if let Some(grid) = details_grid(&details, &flag) {
        card.append(&grid);
    }
    let exact = mono("");
    exact.add_css_class("guardrail-exact");
    exact.set_visible(false);
    card.append(&exact);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    keys.add_css_class("guardrail-keys");
    let inspect = button("Review exact action", "quiet");
    let allow = button("Allow once", "primary");
    allow.set_sensitive(false);
    allow.set_tooltip_text(Some("Review the exact action first"));
    let reject = button("Deny…", "");
    // In the order you take them: read the exact action, which arms Allow once.
    inspect.remove_css_class("quiet");
    keys.append(&inspect);
    keys.append(&allow);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    keys.append(&spacer);
    keys.append(&reject);
    card.append(&keys);
    let id = hold["id"].clone();
    let denial = denial_form(ui, &card, &keys, id.clone(), "Reason (sent to the agent, optional)");
    reject.connect_clicked(move |_| denial());

    let weak = Rc::downgrade(ui);
    let hold_id = id.clone();
    allow.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let key = key.clone();
        let id = hold_id.clone();
        glib::spawn_future_local(async move {
            match ui.call("guardrail.confirm", json!({"hold_id": id})).await {
                Ok(v) => {
                    if v["outcome"]["ok"] == false {
                        let error = &v["outcome"]["error"];
                        ui.show_error(&reveal(&format!("Allowed, but the action then failed: {}", clip(text(error, "message")))).plain);
                    }
                    ui.refresh_page();
                }
                Err(e) => {
                    ui.show_error(&e.to_string());
                    key.set_sensitive(true);
                }
            }
        });
    });
    let weak = Rc::downgrade(ui);
    let allow_key = allow.downgrade();
    inspect.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        if exact.is_visible() {
            exact.set_visible(false);
            key.set_label("Review exact action");
            return;
        }
        key.set_sensitive(false);
        let key = key.clone();
        let exact = exact.clone();
        let flag = flag.clone();
        let allow_key = allow_key.clone();
        let id = id.clone();
        glib::spawn_future_local(async move {
            match ui.call("guardrail.hold.get", json!({"hold_id": id})).await {
                Ok(v) => {
                    let request = &v["request"];
                    let mut shown = format!("{}\n", text(request, "op"));
                    // JSON escapes only quotes, backslashes and C0 controls: a bidi override
                    // in the payload would otherwise reach the label raw.
                    shown.push_str(&serde_json::to_string_pretty(&clip_strings(&request["payload"])).unwrap_or_default());
                    set_agent_text(&exact, &shown, &flag);
                    exact.set_visible(true);
                    key.set_label("Hide exact action");
                    if let Some(allow) = allow_key.upgrade() {
                        allow.set_sensitive(true);
                        allow.set_tooltip_text(None);
                    }
                }
                Err(e) => ui.show_error(&format!("Cannot inspect this hold: {e}")),
            }
            key.set_sensitive(true);
        });
    });
    card
}

/// The hold was answered elsewhere (the phone, the CLI) or is gone: nothing is left to answer.
fn settled(error: &crate::client::Error) -> bool {
    matches!(error, crate::client::Error::Bus(e) if matches!(e.code.as_str(), "guardrail.hold_resolved" | "guardrail.hold_not_found"))
}

/// An inline "why not" form under `keys`, sending `guardrail.reject`. Returns the opener.
fn denial_form(ui: &Rc<Ui>, card: &gtk::Box, keys: &gtk::Box, id: Value, placeholder: &str) -> impl Fn() + 'static {
    let form = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    form.add_css_class("guardrail-deny");
    let reason = gtk::Entry::builder().placeholder_text(placeholder).hexpand(true).build();
    let send = button("Send denial", "primary");
    let cancel = button("Cancel", "quiet");
    form.append(&reason);
    form.append(&send);
    form.append(&cancel);
    form.set_visible(false);
    card.append(&form);
    let weak_keys = keys.downgrade();
    let weak_form = form.downgrade();
    cancel.connect_clicked(move |_| {
        if let (Some(keys), Some(form)) = (weak_keys.upgrade(), weak_form.upgrade()) {
            form.set_visible(false);
            keys.set_visible(true);
        }
    });
    let weak = Rc::downgrade(ui);
    let entry = reason.downgrade();
    let submit = move |key: &gtk::Button| {
        let (Some(ui), Some(entry)) = (weak.upgrade(), entry.upgrade()) else { return };
        let reason = entry.text().trim().to_string();
        key.set_sensitive(false);
        let key = key.clone();
        let id = id.clone();
        glib::spawn_future_local(async move {
            let reason = (!reason.is_empty()).then_some(reason);
            match ui.call("guardrail.reject", json!({"hold_id": id, "reason": reason})).await {
                Ok(_) => {
                    close_prompt(&ui, id.as_i64().unwrap_or_default());
                    ui.refresh_page();
                }
                Err(e) => {
                    ui.show_error(&e.to_string());
                    if settled(&e) {
                        close_prompt(&ui, id.as_i64().unwrap_or_default());
                        ui.refresh_page();
                    }
                }
            }
            key.set_sensitive(true);
        });
    };
    let send_key = send.clone();
    reason.connect_activate(move |_| send_key.emit_clicked());
    // Escape here takes the denial back, as Cancel does; it goes no further, to a panel the
    // prompt happens to sit over.
    let escape = gtk::EventControllerKey::new();
    let cancel_key = cancel.downgrade();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key != gtk::gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(cancel) = cancel_key.upgrade() {
            cancel.emit_clicked();
        }
        glib::Propagation::Stop
    });
    reason.add_controller(escape);
    send.connect_clicked(submit);
    let keys = keys.downgrade();
    let form = form.downgrade();
    let reason = reason.downgrade();
    move || {
        if let (Some(keys), Some(form), Some(entry)) = (keys.upgrade(), form.upgrade(), reason.upgrade()) {
            keys.set_visible(false);
            form.set_visible(true);
            entry.grab_focus();
        }
    }
}

/// What the notice says once a request is approved: who may now do what, and for how long.
fn approved_notice(request: &Value, scope: &str) -> String {
    let value = text(request, "value");
    let what = match value.char_indices().nth(120) {
        None if value.is_empty() => String::new(),
        None => format!(" “{value}”"),
        Some((at, _)) => format!(" “{}…”", &value[..at]),
    };
    reveal(&format!(
        "Approved: {} may {}{what} {}. The agent has been told to retry.",
        text(request, "session"), kind_verb(text(request, "kind")), scope_phrase(scope),
    ))
    .plain
}

/// An agent's request for an exception, with the three answers. `compact` is the prompt form.
/// Also returns the two Approve keys; a prompt's start insensitive, for `prompt_slot` to arm.
pub fn exception_card(ui: &Rc<Ui>, request: &Value, compact: bool) -> (gtk::Box, [gtk::Button; 2]) {
    let request = as_request(request);
    let id = request["id"].as_i64().unwrap_or_default();
    let card = gtk::Box::new(gtk::Orientation::Vertical, if compact { 6 } else { 8 });
    card.add_css_class("guardrail-card");
    card.add_css_class("guardrail-request");
    if compact {
        card.add_css_class("guardrail-prompt");
    } else {
        card.add_css_class("record");
    }
    card.set_widget_name(&format!("guardrail-request-{id}"));
    // A prompt pops up over any project; say which one it is from when it is not this one.
    let from = request["project_id"].as_i64().filter(|p| compact && *p != ui.project.get()).and_then(|p| {
        ui.projects.borrow().iter().find(|row| row["id"].as_i64() == Some(p)).map(|row| text(row, "name").to_owned())
    });
    let session = text(&request, "session");
    let kind = text(&request, "kind");
    let value = text(&request, "value");
    let asked = text(&request, "requested_scope");
    let flag = hidden_flag();
    let heading = label("", "title");
    set_agent_text(&heading, &format!("{session} asks to {}", kind_verb(kind)), &flag);
    heading.set_xalign(0.0);
    if compact {
        // The prompt floats over your work, so it says it in one line: who wants what, from
        // where and since when. The rule lifted is said on the keys' tooltips.
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let mark = crate::icons::image("shield", 14);
        mark.add_css_class("guardrail-mark");
        row.append(&mark);
        // Wrapped, not cut: the session's name is part of what is being approved.
        heading.set_hexpand(true);
        heading.set_wrap(true);
        heading.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        // A wrapping label asks for its whole text on one line; these keep the prompt narrow.
        heading.set_max_width_chars(34);
        flag.set_max_width_chars(46);
        row.append(&heading);
        let when = crate::relative::ago(text(&request, "created_at"), crate::relative::Form::Compact).unwrap_or_default();
        let corner = match from {
            Some(name) if !name.is_empty() => format!("{name} · {when}"),
            _ => when,
        };
        let corner = label(&corner, "faint");
        corner.set_valign(gtk::Align::Start);
        row.append(&corner);
        card.append(&row);
        card.append(&flag);
    } else {
        let eyebrow = match from {
            Some(name) if !name.is_empty() => format!("Exception request · {name}"),
            _ => "Exception request".to_owned(),
        };
        card.append(&head(&eyebrow, &ago(text(&request, "created_at"))));
        card.append(&flag);
        heading.set_wrap(true);
        card.append(&heading);
    }
    if !value.is_empty() {
        let subject = agent_mono(&clip(value), &flag);
        if compact {
            subject.set_max_width_chars(46);
            // Every byte being approved stays readable, in a box that scrolls past a few lines
            // rather than growing the prompt.
            let scroll = gtk::ScrolledWindow::builder()
                .child(&subject)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_height(true)
                .max_content_height(84)
                .build();
            scroll.add_css_class("guardrail-subject-scroll");
            card.append(&scroll);
        } else {
            card.append(&subject);
        }
    }
    let reason = text(&request, "reason");
    if !reason.is_empty() {
        let why = agent_paragraph(&format!("“{}”", clip(reason)), "body", &flag);
        why.add_css_class("guardrail-reason");
        if compact {
            // The agent's case, not the thing approved: its opening is enough here, and the
            // Guardrails page has the rest.
            why.set_lines(2);
            why.set_max_width_chars(46);
            why.set_ellipsize(gtk::pango::EllipsizeMode::End);
            why.set_selectable(false);
            why.set_tooltip_text(Some(&reveal(&clip(reason)).plain));
        }
        card.append(&why);
    }
    if !compact {
        let lifted = label("", "faint");
        set_agent_text(
            &lifted,
            &format!(
                "Asked for {}. Only this rule is lifted, and only for {session}.",
                if asked == "session" { "the rest of the session" } else { "one use" },
            ),
            &flag,
        );
        card.append(&lifted);
    }
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    keys.add_css_class("guardrail-keys");
    let once = button("Approve once", if asked == "session" { "" } else { "primary" });
    let always = button(if compact { "For the session" } else { "Approve for this session" }, if asked == "session" { "primary" } else { "" });
    let deny = button(if compact { "Deny" } else { "Deny…" }, "quiet");
    if compact {
        let who = reveal(session).plain;
        once.set_tooltip_text(Some(&format!("Lift only this rule, only for {who}, for one use")));
        always.set_tooltip_text(Some(&format!("Lift only this rule, only for {who}, for the rest of its session")));
    }
    for key in [&once, &always, &deny] {
        // A prompt is answered beside a terminal: a click must not pull the keys out of it.
        key.set_focus_on_click(!compact);
        keys.append(key);
    }
    if compact {
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        keys.append(&spacer);
        let later = crate::app::icon_button("close", "Answer later from Guardrails");
        later.set_focus_on_click(false);
        let weak = Rc::downgrade(ui);
        later.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                close_prompt(&ui, id);
            }
        });
        keys.append(&later);
    }
    card.append(&keys);
    let opener = denial_form(ui, &card, &keys, json!(id), "Why not? The agent reads this");
    deny.connect_clicked(move |_| opener());
    for (key, scope) in [(&once, "once"), (&always, "session")] {
        if compact {
            // Off until the prompt has been on screen a moment (prompt_slot): it arrives
            // unannounced, and a click meant for the terminal under it must not approve it.
            key.set_sensitive(false);
        }
        let weak = Rc::downgrade(ui);
        let all = keys.downgrade();
        let notice = approved_notice(&request, scope);
        key.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(keys) = all.upgrade() {
                keys.set_sensitive(false);
            }
            let all = all.clone();
            let notice = notice.clone();
            glib::spawn_future_local(async move {
                match ui.call("guardrail.confirm", json!({"hold_id": id, "scope": scope})).await {
                    Ok(_) => {
                        close_prompt(&ui, id);
                        ui.show_info(&notice);
                        ui.refresh_page();
                    }
                    Err(e) => {
                        ui.show_error(&e.to_string());
                        if settled(&e) {
                            close_prompt(&ui, id);
                            ui.refresh_page();
                        } else if let Some(keys) = all.upgrade() {
                            keys.set_sensitive(true);
                        }
                    }
                }
            });
        });
    }
    (card, [once, always])
}

fn tray(ui: &Rc<Ui>) -> gtk::Box {
    let mut child = ui.overlay.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == TRAY {
            if let Ok(tray) = widget.downcast::<gtk::Box>() {
                // Overlays stack in the order they were added, and a panel opened since sits
                // over the tray, frame and scrim. Raise it, unless a denial is being typed in it.
                let typing = tray.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(&tray));
                if tray.next_sibling().is_some() && !typing {
                    ui.overlay.remove_overlay(&tray);
                    ui.overlay.add_overlay(&tray);
                }
                return tray;
            }
            break;
        }
        child = widget.next_sibling();
    }
    // No spacing: each card carries its own gap, so the gap closes with it as it slides away.
    let tray = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tray.set_widget_name(TRAY);
    tray.add_css_class("guardrail-tray");
    tray.set_halign(gtk::Align::End);
    tray.set_valign(gtk::Align::End);
    tray.set_margin_end(16);
    tray.set_margin_bottom(40);
    tray.set_size_request(360, -1);
    // The prompt waits beside whatever you are doing; its keys do not take keyboard focus on
    // a click (exception_card), and only the Deny form's reason entry asks for it.
    ui.overlay.add_overlay(&tray);
    tray
}

fn prompt_ids(tray: &gtk::Box) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut child = tray.first_child();
    while let Some(widget) = child {
        if let Some(id) = widget.widget_name().strip_prefix("guardrail-request-").and_then(|id| id.parse().ok()) {
            ids.push(id);
        }
        child = widget.next_sibling();
    }
    ids
}

fn update_overflow(ui: &Rc<Ui>, tray: &gtk::Box, hidden: usize) {
    let mut child = tray.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.widget_name() == "guardrail-more" {
            tray.remove(&widget);
        }
    }
    if hidden == 0 {
        return;
    }
    let more = button(&format!("{hidden} more waiting · open Guardrails"), "quiet");
    more.set_focus_on_click(false);
    more.set_widget_name("guardrail-more");
    more.add_css_class("guardrail-more");
    let weak = Rc::downgrade(ui);
    more.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            // Prompts come from every project, but the Guardrails page lists one: open the
            // project whose request has waited longest.
            let oldest = WAITING.with(|w| w.borrow().first().and_then(|r| r["project_id"].as_i64()));
            match oldest {
                Some(project) if project != ui.project.get() => ui.open_project(project, "guardrails"),
                _ => ui.navigate("guardrails"),
            }
        }
    });
    tray.append(&more);
}

thread_local! {
    /// Requests that arrived while the tray was full, oldest first.
    static WAITING: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static DISMISSED: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
}

/// Put one request in front of the user, without taking focus from a terminal.
fn show_prompt(ui: &Rc<Ui>, request: &Value) {
    let id = request["id"].as_i64().unwrap_or_default();
    if DISMISSED.with(|d| d.borrow().contains(&id)) {
        return;
    }
    let tray = tray(ui);
    if prompt_ids(&tray).contains(&id) {
        return;
    }
    if prompt_ids(&tray).len() >= TRAY_LIMIT {
        WAITING.with(|w| {
            let mut waiting = w.borrow_mut();
            if !waiting.iter().any(|r| r["id"] == request["id"]) {
                waiting.push(request.clone());
            }
        });
    } else {
        // A request shown in the tray is no longer waiting, whichever path brought it here.
        WAITING.with(|w| w.borrow_mut().retain(|r| r["id"] != request["id"]));
        // The overflow line is re-added below, so it always ends the tray.
        let (card, approve) = exception_card(ui, request, true);
        tray.append(&prompt_slot(&card, approve, id));
    }
    let hidden = WAITING.with(|w| w.borrow().len());
    update_overflow(ui, &tray, hidden);
}

/// A prompt card inside a revealer: it slides up and fades in on the next frame, rather than
/// appearing all at once over whatever you are doing. Its `approve` keys come on [`ARM_MS`]
/// after it has finished sliding in.
fn prompt_slot(card: &gtk::Box, approve: [gtk::Button; 2], id: i64) -> gtk::Revealer {
    let slot = gtk::Revealer::new();
    slot.set_widget_name(&format!("guardrail-request-{id}"));
    slot.set_transition_type(gtk::RevealerTransitionType::SlideUp);
    slot.set_transition_duration(PROMPT_IN_MS);
    card.add_css_class("leaving");
    slot.set_child(Some(card));
    let approve = approve.map(|key| key.downgrade());
    let armed = Cell::new(false);
    // A revealer clips its child, shadow included. Clip only while sliding.
    slot.connect_child_revealed_notify(move |slot| {
        if !slot.is_child_revealed() {
            return;
        }
        slot.set_overflow(gtk::Overflow::Visible);
        if armed.replace(true) {
            return;
        }
        let approve = approve.clone();
        let slot = slot.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(ARM_MS), move || {
            // Answered or put off in the meantime: it is sliding away, and stays off.
            if slot.upgrade().is_none_or(|slot| slot.widget_name() == LEAVING) {
                return;
            }
            for key in approve.iter().filter_map(|key| key.upgrade()) {
                key.set_sensitive(true);
            }
        });
    });
    // Started from a frame callback so the hidden state is drawn once and the fade has
    // something to fade from.
    slot.add_tick_callback(|slot, _| {
        if slot.widget_name() == LEAVING {
            return glib::ControlFlow::Break;
        }
        slot.set_reveal_child(true);
        if let Some(card) = slot.child() {
            card.remove_css_class("leaving");
        }
        glib::ControlFlow::Break
    });
    slot
}

/// Slide a prompt away, then remove it. Without animations (or off screen) the revealer
/// reports itself hidden at once, and it is removed at once.
fn retire(tray: &gtk::Box, slot: &gtk::Widget) {
    let Some(revealer) = slot.downcast_ref::<gtk::Revealer>() else {
        tray.remove(slot);
        return;
    };
    revealer.set_widget_name(LEAVING);
    if !revealer.reveals_child() && !revealer.is_child_revealed() {
        // Answered before its first frame: there is nothing to slide away.
        tray.remove(revealer);
        return;
    }
    revealer.set_can_target(false);
    revealer.set_overflow(gtk::Overflow::Hidden);
    if let Some(card) = revealer.child() {
        card.add_css_class("leaving");
    }
    let weak = tray.downgrade();
    revealer.connect_child_revealed_notify(move |revealer| {
        if revealer.is_child_revealed() {
            return;
        }
        if let Some(tray) = weak.upgrade() {
            if revealer.parent().as_ref() == Some(tray.upcast_ref::<gtk::Widget>()) {
                tray.remove(revealer);
            }
        }
    });
    revealer.set_transition_duration(PROMPT_OUT_MS);
    revealer.set_reveal_child(false);
}

/// Take a request's prompt down (answered, expired, or put off), and let a waiting one in.
fn close_prompt(ui: &Rc<Ui>, id: i64) {
    WAITING.with(|w| w.borrow_mut().retain(|r| r["id"].as_i64() != Some(id)));
    // Recorded even when it is not showing: a `guardrail.request.get` still in flight for it
    // must not put it back up.
    DISMISSED.with(|d| {
        let mut dismissed = d.borrow_mut();
        if !dismissed.contains(&id) {
            dismissed.push(id);
        }
    });
    let tray = tray(ui);
    let mut child = tray.first_child();
    let mut removed = false;
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.widget_name() == format!("guardrail-request-{id}") {
            retire(&tray, &widget);
            removed = true;
        }
    }
    if removed {
        // One mutable borrow, bound before show_prompt borrows WAITING again: a shared borrow
        // held across `then` makes the borrow_mut panic.
        let next = WAITING.with(|w| {
            let mut waiting = w.borrow_mut();
            (!waiting.is_empty()).then(|| waiting.remove(0))
        });
        if let Some(next) = next {
            show_prompt(ui, &next);
        }
    }
    let hidden = WAITING.with(|w| w.borrow().len());
    update_overflow(ui, &tray, hidden);
}

/// Every guardrail event passes through here before the page refresh, whatever page or
/// project is showing: a request for an exception has to reach the user wherever they are.
pub fn guardrail_event(ui: &Rc<Ui>, ev: &str, payload: &Value) {
    match ev {
        "guardrail.held" if payload["policy"] == "exception" => {
            let Some(id) = payload["request_id"].as_i64().or(payload["hold_id"].as_i64()) else { return };
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                if let Ok(request) = ui.call("guardrail.request.get", json!({"request_id": id})).await {
                    if request["state"] == "open" {
                        show_prompt(&ui, &request);
                    }
                }
            });
        }
        "guardrail.resolved" => {
            if let Some(id) = payload["hold_id"].as_i64() {
                close_prompt(ui, id);
            } else if payload["state"] == "expired" {
                // A closed session takes its open requests with it.
                restore_prompts(ui);
            }
        }
        _ => {}
    }
}

/// Bring back prompts for requests that are still open — after a reconnect, or when a
/// session closed and the set may have shrunk.
pub fn restore_prompts(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let Ok(open) = ui.call("guardrail.requests.list", json!({"state": "open"})).await else { return };
        let open = rows(&open, "requests");
        // Stale waiting entries go first, so closing a stale prompt cannot promote one.
        WAITING.with(|w| w.borrow_mut().retain(|r| open.iter().any(|o| o["id"] == r["id"])));
        let tray = tray(&ui);
        for id in prompt_ids(&tray) {
            if !open.iter().any(|r| r["id"].as_i64() == Some(id)) {
                close_prompt(&ui, id);
            }
        }
        // Oldest first: the one that has waited longest is answered first.
        for request in open.iter().rev() {
            show_prompt(&ui, request);
        }
    });
}

fn section(body: &gtk::Box, title: &str, hint: &str) {
    let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
    head.add_css_class("guardrail-section");
    head.append(&label(title, "section-label"));
    if !hint.is_empty() {
        let hint = paragraph(hint, "faint");
        hint.set_selectable(false);
        head.append(&hint);
    }
    body.append(&head);
}

thread_local! {
    /// The page's cards by request or hold, kept across refreshes: a half-typed denial reason,
    /// a reviewed exact action or an open expander survives every event that does not resolve
    /// that card.
    static CARDS: RefCell<(i64, Vec<(String, gtk::Box)>)> = const { RefCell::new((0, Vec::new())) };
    /// What the page last drew; an event that changes none of it leaves the page alone.
    static DRAWN: Cell<u64> = const { Cell::new(0) };
}

/// How long the page ignores the pointer after cards move under it, so a click aimed at one
/// card cannot land on the Approve key of the card that slid into its place.
const SETTLE_MS: u64 = 500;

/// The Guardrails page body: requests first, then held actions, then the grants in force.
pub async fn page(ui: &Rc<Ui>, body: &gtk::Box, project: i64, holds: Vec<Value>) {
    // Everything is fetched before the page is touched, so it is never drawn half-way.
    let (active, overlaps) = tokio::join!(
        ui.call("guardrail.requests.list", json!({"project_id": project, "state": "active"})),
        ui.call("overlap.list", json!({"project_id": project}))
    );
    if ui.project.get() != project || *ui.page.borrow() != "guardrails" {
        return;
    }
    let drawn = {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        project.hash(&mut hash);
        Value::Array(holds.clone()).to_string().hash(&mut hash);
        for result in [&active, &overlaps] {
            match result {
                Ok(value) => value.to_string().hash(&mut hash),
                Err(error) => error.to_string().hash(&mut hash),
            }
        }
        hash.finish()
    };
    if body.first_child().is_some() && DRAWN.get() == drawn {
        return;
    }
    DRAWN.set(drawn);
    let focus = body.root().and_then(|root| root.focus()).filter(|focus| focus.is_ancestor(body));
    let (old_project, old) = CARDS.take();
    let old = if old_project == project { old } else { Vec::new() };
    clear(body);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("guardrail-actions");
    let edit = button("Guardrail settings for this project", "");
    let weak = Rc::downgrade(ui);
    edit.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_project_guardrails(&ui, project);
        }
    });
    actions.append(&edit);
    let workspace = ui.projects.borrow().iter()
        .find(|p| p["id"].as_i64() == Some(project))
        .and_then(|p| p["workspace_id"].as_i64());
    if let Some(workspace) = workspace {
        let edit = button("Workspace guardrails", "quiet");
        let weak = Rc::downgrade(ui);
        edit.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                open_workspace_guardrails(&ui, workspace);
            }
        });
        actions.append(&edit);
    }
    body.append(&actions);
    if holds.is_empty() {
        body.append(&paragraph("Nothing is waiting for you: no exception requests and no held actions.", "body"));
    }

    // Oldest first, so a new arrival lands at the bottom instead of pushing every card down.
    let (mut requests, mut held): (Vec<Value>, Vec<Value>) = holds.into_iter().partition(is_request);
    requests.sort_by_key(|r| as_request(r)["id"].as_i64());
    held.sort_by_key(|h| h["id"].as_i64());
    let mut cards: Vec<(String, gtk::Box)> = Vec::new();
    let mut place = |key: String, build: &dyn Fn() -> gtk::Box| {
        let card = old.iter().find(|(k, _)| *k == key).map(|(_, card)| card.clone()).unwrap_or_else(build);
        body.append(&card);
        cards.push((key, card));
    };
    if !requests.is_empty() {
        section(body, "Exception requests", "Agents that cannot progress without passing a guardrail. Approving lifts only the named rule, for that session.");
        for request in &requests {
            let key = format!("request-{}", as_request(request)["id"]);
            place(key, &|| exception_card(ui, request, false).0);
        }
    }
    if !held.is_empty() {
        section(body, "Held actions", "Paused until you decide. Review the exact action before allowing it.");
        for hold in &held {
            place(format!("hold-{}", hold["id"]), &|| hold_row(ui, hold.clone()));
        }
    }
    match active {
        Ok(active) => {
            let active = rows(&active, "requests");
            if !active.is_empty() {
                section(body, "Active exceptions", "Approved exceptions still in force. Revoke one to put the rule back immediately.");
                for grant in active {
                    body.append(&grant_row(ui, &grant));
                }
            }
        }
        Err(e) => ui.show_error(&format!("Exceptions: {e}")),
    }
    if let Ok(overlaps) = overlaps {
        let overlaps = rows(&overlaps, "overlaps");
        if !overlaps.is_empty() {
            section(body, "Shared file activity", "Files that more than one agent is changing.");
        }
        for overlap in overlaps {
            let flag = hidden_flag();
            body.append(&agent_paragraph(&format!("{}\n{}", text(&overlap, "path"), clip(text(&overlap, "note"))), "body", &flag));
            body.append(&flag);
        }
    }

    // A card that kept its place but moved (one above it resolved or arrived): hold the
    // pointer off for a moment.
    let kept: Vec<&String> = old.iter().map(|(k, _)| k).filter(|k| cards.iter().any(|(c, _)| c == *k)).collect();
    let moved = kept.iter().any(|k| {
        old.iter().position(|(o, _)| o == *k) != cards.iter().position(|(c, _)| c == *k)
    });
    CARDS.set((project, cards));
    if let Some(focus) = focus.filter(|focus| focus.root().is_some()) {
        match focus.downcast_ref::<gtk::Text>() {
            Some(text) => text.grab_focus_without_selecting(),
            None => focus.grab_focus(),
        };
    }
    if moved {
        body.set_can_target(false);
        let body = body.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(SETTLE_MS), move || {
            if let Some(body) = body.upgrade() {
                body.set_can_target(true);
            }
        });
    }
}

/// One approved exception, with Revoke.
fn grant_row(ui: &Rc<Ui>, grant: &Value) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("record");
    row.add_css_class("guardrail-grant");
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 4);
    copy.set_hexpand(true);
    let scope = text(grant, "scope");
    let flag = hidden_flag();
    let who = label("", "body");
    set_agent_text(&who, &format!("{} may {}", text(grant, "session"), kind_verb(text(grant, "kind"))), &flag);
    copy.append(&who);
    if !text(grant, "value").is_empty() {
        copy.append(&agent_mono(&clip(text(grant, "value")), &flag));
    }
    copy.append(&flag);
    let uses = grant["uses"].as_u64().unwrap_or(0);
    let lasting = if scope == "session" {
        match uses {
            0 => "For this session · not used yet".to_string(),
            1 => "For this session · used once".to_string(),
            n => format!("For this session · used {n} times"),
        }
    } else {
        "Once · not used yet".to_string()
    };
    let approved = ago(text(grant, "resolved_at"));
    copy.append(&label(
        &if approved.is_empty() { lasting } else { format!("{lasting} · approved {approved}") },
        "faint",
    ));
    row.append(&copy);
    let revoke = button("Revoke", "");
    revoke.set_valign(gtk::Align::Center);
    let weak = Rc::downgrade(ui);
    let id = grant["id"].clone();
    revoke.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let key = key.clone();
        let id = id.clone();
        glib::spawn_future_local(async move {
            match ui.call("guardrail.grant.revoke", json!({"request_id": id})).await {
                Ok(_) => ui.refresh_page(),
                Err(e) => {
                    ui.show_error(&e.to_string());
                    key.set_sensitive(true);
                }
            }
        });
    });
    row.append(&revoke);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through_and_is_markup_escaped() {
        let shown = reveal("rm -rf build/ && echo <done>\nnext line");
        assert_eq!(shown.hidden, 0);
        assert_eq!(shown.plain, "rm -rf build/ && echo <done>\nnext line");
        assert_eq!(shown.markup, "rm -rf build/ &amp;&amp; echo &lt;done&gt;\nnext line");
    }

    #[test]
    fn bidi_controls_become_visible_escapes() {
        let shown = reveal("cp a\u{202E}b c");
        assert_eq!(shown.hidden, 1);
        assert_eq!(shown.plain, "cp a<U+202E>b c");
        assert!(shown.markup.contains("&lt;U+202E&gt;</span>"), "{}", shown.markup);
        assert!(shown.markup.starts_with("cp a<span ") && shown.markup.ends_with("</span>b c"));
    }

    #[test]
    fn every_listed_character_is_caught() {
        let listed = [
            '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
            '\u{2069}', '\u{200E}', '\u{200F}', '\u{061C}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}',
            '\u{FEFF}', '\t', '\r', '\0', '\u{7F}', '\u{85}', '\u{E0041}',
        ];
        for c in listed {
            let shown = reveal(&format!("a{c}b"));
            assert_eq!(shown.hidden, 1, "{:04X}", c as u32);
            assert_eq!(shown.plain, format!("a<U+{:04X}>b", c as u32));
        }
        assert_eq!(reveal("x\u{E0041}").plain, "x<U+E0041>");
    }

    #[test]
    fn ordinary_unicode_is_left_alone() {
        let shown = reveal("café/日本語/עברית “quoted” — ok\n");
        assert_eq!(shown.hidden, 0);
        assert_eq!(shown.plain, "café/日本語/עברית “quoted” — ok\n");
    }

    #[test]
    fn a_typed_escape_is_not_counted_or_highlighted() {
        let shown = reveal("<U+202E>");
        assert_eq!(shown.hidden, 0);
        assert_eq!(shown.markup, "&lt;U+202E&gt;");
    }

    #[test]
    fn notices_escape_hidden_characters() {
        let hold = json!({
            "op": "guardrail.request", "session": "agent\u{200B}one",
            "details": {"kind": "command", "value": "git push \u{2066}--force\u{2069}"},
        });
        let notice = approved_notice(&as_request(&hold), "once");
        assert_eq!(notice, "Approved: agent<U+200B>one may run “git push <U+2066>--force<U+2069>” once. The agent has been told to retry.");
    }

    #[test]
    fn notice_names_the_session_and_scope() {
        let request = json!({"session": "worker", "kind": "path", "value": "/etc/hosts"});
        assert_eq!(
            approved_notice(&request, "session"),
            "Approved: worker may write to “/etc/hosts” for the rest of the session. The agent has been told to retry."
        );
        let caps = json!({"session": "worker", "kind": "cap", "value": ""});
        assert_eq!(approved_notice(&caps, "once"), "Approved: worker may commit past the change caps once. The agent has been told to retry.");
        let long = json!({"session": "w", "kind": "command", "value": "x".repeat(500)});
        assert!(approved_notice(&long, "once").contains(&format!("“{}…”", "x".repeat(120))));
    }
}
