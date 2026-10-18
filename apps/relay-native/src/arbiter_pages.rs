//! Arbiter in the Threads space, beside Tally (docs/ARBITER.md): its pages (Overview, a strategy,
//! Orders, Settings), the strategy editor and the Connect Coinbase sheet; and what the thread page
//! borrows from them, the Arbiter panel and the proposal card.
//!
//! Everything is read and written through the engine's `arbiter.*` ops; nothing here decides a
//! trade. Amounts arrive as decimal strings and are only ever shown from them: an amount the person
//! types goes back as the text they typed (cleaned of a currency sign), never through a float, so
//! the engine's decimals see what was written. Colour keeps one meaning on every surface: green is
//! a live strategy or money in, amber is paper (and Dev's waiting), red is a loss, a refusal or a
//! halt.
use super::pages::{card, columns, glyph_image, icon_row, meter_in, plural, setting_row, tile, Tone};
use super::{message, page, toast, Action, Page};
use crate::app::{button, clear, confirm_inline, confirm_inline_if, label, rows, text, Ui};
use crate::client::Error;
use crate::panel::Panel;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_money::money::{fraction_digits, Locale, MoneyFormatter};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Arbiter's tabbed pages: (stack name, caption).
pub const PAGES: [(&str, &str); 3] = [("arbiter-home", "Overview"), ("arbiter-orders", "Orders"), ("arbiter-settings", "Settings")];
/// One strategy, opened from a row; it carries no tab of its own.
pub const STRATEGY: &str = "arbiter-strategy";
/// Every Arbiter page, for `money::add_pages`.
pub const ALL: [&str; 4] = ["arbiter-home", "arbiter-orders", "arbiter-settings", STRATEGY];

/// The thread panel's Arbiter views: (key, caption).
pub const PANEL_TABS: [(&str, &str); 3] = [("arbiter-overview", "Overview"), ("arbiter-strategies", "Strategies"), ("arbiter-orders", "Orders")];

/// Bar widths as the exchange names them, with a caption and the short form a sentence uses.
const GRANULARITIES: [(&str, &str, &str); 8] = [
    ("ONE_MINUTE", "1 minute", "1m"),
    ("FIVE_MINUTE", "5 minutes", "5m"),
    ("FIFTEEN_MINUTE", "15 minutes", "15m"),
    ("THIRTY_MINUTE", "30 minutes", "30m"),
    ("ONE_HOUR", "1 hour", "1h"),
    ("TWO_HOUR", "2 hours", "2h"),
    ("SIX_HOUR", "6 hours", "6h"),
    ("ONE_DAY", "1 day", "1d"),
];

/// Who decides: (mode, its name, the key's short caption, what it means in a line).
const MODES: [(&str, &str, &str, &str); 3] = [
    ("rules", "Rules run, AI proposes", "AI proposes", "The rule places orders within the limits. A thread's agent may read, backtest and propose; what it proposes waits for you."),
    ("agent", "AI trades within limits", "AI trades", "A thread's agent may place orders for this strategy itself. Each still passes the limits, and only you can change them."),
    ("ask", "Every order asks", "Every order asks", "Each order, from the rule or an agent, waits on a card for your yes, and expires unanswered."),
];

/// A strategy's limits: (key, caption, unit). Money is the strategy's quote currency.
const LIMITS: [(&str, &str, &str); 5] = [
    ("max_order", "Most per order", "money"),
    ("max_position", "Most held", "money"),
    ("daily_loss", "Daily loss, then halt", "money"),
    ("orders_per_hour", "Orders per hour", "count"),
    ("cooldown_minutes", "Cooldown after a loss", "minutes"),
];

const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

#[derive(Default)]
struct State {
    /// The strategy the Strategy page shows.
    strategy: Cell<Option<i64>>,
    /// The last `arbiter.summary` read: the sidebar lamp, the panel and the cards borrow from it.
    summary: RefCell<Option<Value>>,
    lamp: RefCell<Option<gtk::Box>>,
    reading: Cell<bool>,
    /// Backtests run from the Strategy page, by (strategy, version), and the strategies running
    /// one now: a backtest takes seconds, and the page is rebuilt on every `arbiter.changed`.
    backtests: RefCell<HashMap<(i64, i64), Value>>,
    backtesting: RefCell<HashSet<i64>>,
    cards_pending: Cell<bool>,
}

thread_local! {
    static STATE: State = State::default();
    /// Set while a page's body is rebuilt: a field that loses its focus to the rebuild is not
    /// being left by the person, so it must not save.
    static REBUILDING: Cell<bool> = const { Cell::new(false) };
    /// Proposal cards in conversations, by proposal id, to redraw when one is answered elsewhere
    /// or expires.
    static CARDS: RefCell<Vec<(i64, glib::WeakRef<gtk::Box>)>> = const { RefCell::new(Vec::new()) };
}

// ---- Reading the engine's numbers ----------------------------------------------------------

fn num(v: &Value) -> f64 {
    opt_num(v).unwrap_or(0.0)
}

/// A decimal string ("50.00") or a number; `None` for null.
fn opt_num(v: &Value) -> Option<f64> {
    match v {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// `x` with at most `places` decimals, trailing zeros dropped: 0.01230000 reads "0.0123".
fn trimmed(x: f64, places: usize) -> String {
    let s = format!("{x:.places$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// "$1,234.56" in a currency Tally's formatter knows; "0.0123 BTC" or "12.5 USDC" in any other.
fn money(value: f64, currency: &str) -> String {
    match fraction_digits(currency) {
        Some(digits) => MoneyFormatter::new(currency, Locale::from_env()).format((value * 10f64.powi(digits as i32)).round() as i64),
        None => {
            let places = if value.abs() >= 1000.0 { 2 } else if value.abs() >= 1.0 { 4 } else { 8 };
            format!("{}{} {currency}", if value < 0.0 { "−" } else { "" }, trimmed(value.abs(), places))
        }
    }
}

/// [`money`] with a plus on a gain; a loss already carries its minus.
fn money_signed(value: f64, currency: &str) -> String {
    let shown = money(value, currency);
    if value > 0.0 && shown.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("+{shown}") } else { shown }
}

/// "+4.2%", "−3.0%".
fn pct(v: f64) -> String {
    let s = format!("{:.1}%", v.abs());
    if s == "0.0%" {
        s
    } else if v > 0.0 {
        format!("+{s}")
    } else {
        format!("−{s}")
    }
}

/// A drawdown, a fall: "−4.2%", and "0.0%" when there was none to speak of.
fn drawdown(x: f64) -> String {
    if x.abs() < 0.05 { String::from("0.0%") } else { format!("−{:.1}%", x.abs()) }
}

/// A price in its quote currency, with the decimals a price under 1 needs.
fn price_text(p: f64, quote: &str) -> String {
    if p >= 1.0 { money(p, quote) } else { format!("{} {quote}", trimmed(p, 6)) }
}

/// `ETH-CAD` as ("ETH", "CAD").
fn pair(product: &str) -> (&str, &str) {
    product.split_once('-').unwrap_or((product, ""))
}

/// Green for a gain, red for a loss, nothing for nothing.
fn tone_of(v: f64) -> Option<&'static str> {
    if v >= 0.005 {
        Some("money-in")
    } else if v <= -0.005 {
        Some("money-over")
    } else {
        None
    }
}

fn add_tone(widget: &impl IsA<gtk::Widget>, v: f64) {
    if let Some(class) = tone_of(v) {
        widget.add_css_class(class);
    }
}

/// What a failed call says on the page: the engine's own sentence, never its code. An engine
/// without Arbiter says so plainly.
pub(super) fn said(e: &Error) -> String {
    match e {
        Error::Bus(b) if matches!(b.code.as_str(), "bus.unknown_op" | "bus.not_implemented") => {
            String::from("This engine has no Arbiter yet. Update Relay's engine to use it.")
        }
        Error::Bus(b) => b.message.clone(),
        e => e.to_string(),
    }
}

/// "5m ago", from an engine timestamp.
fn ago(ts: &str) -> String {
    crate::relative::ago(ts, crate::relative::Form::CompactAgo).unwrap_or_default()
}

/// Minutes from now until `ts`, an RFC 3339 time.
fn minutes_until(ts: &str) -> Option<i64> {
    let then = glib::DateTime::from_iso8601(ts, Some(&glib::TimeZone::utc())).ok()?;
    let now = glib::DateTime::now_utc().ok()?;
    Some(then.difference(&now).as_minutes())
}

/// "3 Mar", from Unix seconds.
fn day_of(unix: i64) -> String {
    glib::DateTime::from_unix_local(unix).ok().and_then(|d| d.format("%-d %b").ok()).map(|s| s.to_string()).unwrap_or_default()
}

fn granularity(key: &str) -> (&'static str, &'static str, &'static str) {
    GRANULARITIES.into_iter().find(|(k, _, _)| *k == key).unwrap_or(GRANULARITIES[4])
}

fn mode_name(mode: &str) -> &'static str {
    MODES.iter().find(|m| m.0 == mode).map_or(MODES[0].1, |m| m.1)
}

/// The home currency, from the last summary read.
fn home_currency() -> String {
    cached().and_then(|s| s["home"].as_str().map(str::to_string)).filter(|h| !h.is_empty()).unwrap_or_else(|| String::from("CAD"))
}

/// A decimal the person typed, cleaned for the engine: "1 000,50 $" is refused, "50" and "50.00"
/// pass as written. `None` when it is not a positive number.
fn decimal(typed: &str) -> Option<String> {
    let cleaned: String = typed.trim().chars().filter(|c| !matches!(c, '$' | ' ' | '%')).collect::<String>().replace(',', ".");
    let value: f64 = cleaned.parse().ok()?;
    (value.is_finite() && value > 0.0).then_some(cleaned)
}

// ---- Small widgets -------------------------------------------------------------------------

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

/// A strategy's colour: red halted, green running live, amber running on paper, none stopped.
fn row_tone(r: &Value) -> &'static str {
    match (text(r, "state"), text(r, "venue")) {
        ("halted", _) => "arbiter-halted",
        ("running", "live") => "arbiter-live",
        ("running", _) => "arbiter-paper",
        _ => "",
    }
}

/// Halted, Live or Paper.
fn venue_pill(r: &Value) -> gtk::Label {
    match (text(r, "state"), text(r, "venue")) {
        ("halted", _) => pill("Halted", "arbiter-halted"),
        (_, "live") => pill("Live", "arbiter-live"),
        _ => pill("Paper", "arbiter-paper"),
    }
}

/// Strokes on the 16-unit grid for Arbiter's own marks; the rest are Tally's glyphs.
fn glyph(key: &str) -> Option<&'static str> {
    Some(match key {
        "arbiter" => r##"<path d="M2 12.5l3.5-4 3 2.5L14 4.5" /><path d="M10.5 4.5H14V8" />"##,
        "dip" => r##"<path d="M2 4l3.5 7 3-3.5L14 12" /><circle cx="5.5" cy="11" r=".9" />"##,
        "trend" => r##"<path d="M2 12.5c3 0 4-6 7-6s3 2 5 2" /><path d="M2 10c3 0 5-6.5 12-6.5" opacity=".55" />"##,
        "breakout" => r##"<path d="M2 8.5h8" stroke-dasharray="1.5 1.5" /><path d="M2 12l3-2 2.5 1L10 6l4-3" />"##,
        "halt" => r##"<path d="M5.5 2.5h5l3 3v5l-3 3h-5l-3-3v-5z" /><path d="M5.8 8h4.4" />"##,
        "key" => r##"<circle cx="5" cy="8" r="2.6" /><path d="M7.6 8H14M11.5 8v2.4M13.5 8v1.8" />"##,
        _ => return None,
    })
}

/// An Arbiter glyph, else Tally's.
pub(super) fn icon(key: &str, size: i32) -> gtk::Image {
    match glyph(key) {
        Some(g) => crate::icons::from_geometry(g, size, 1.5),
        None => glyph_image(key, size),
    }
}

/// Tally's neutral tile, with an Arbiter glyph.
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

/// A strategy's mark, from its first sentence: the row carries no template.
fn rule_icon(r: &Value) -> &'static str {
    let first = r["sentences"][0].as_str().unwrap_or("").to_lowercase();
    if first.contains("every") {
        "calendar"
    } else if first.contains("rsi") {
        "dip"
    } else if first.contains("ema") || first.contains("sma") || first.contains("average") {
        "trend"
    } else if first.contains("high") {
        "breakout"
    } else {
        "arbiter"
    }
}

fn side_icon(side: &str) -> &'static str {
    if side == "buy" { "income" } else { "spend" }
}

/// A page's head: its title (a label, or a title with pills), a context line under it, and keys
/// on the right.
fn heading(head: &gtk::Box, title: &gtk::Widget, context: Option<&gtk::Widget>, actions: &[&gtk::Widget]) {
    clear(head);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_hexpand(true);
    words.append(title);
    if let Some(context) = context {
        words.append(context);
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

fn title_label(words: &str) -> gtk::Widget {
    label(words, "money-title").upcast()
}

fn context_label(words: &str) -> gtk::Widget {
    let l = label(words, "money-context");
    l.set_wrap(true);
    l.upcast()
}

/// A wide column and a narrow one beside it, `right` pixels wide.
fn split(parent: &gtk::Box, right: i32) -> (gtk::Box, gtk::Box) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let l = gtk::Box::new(gtk::Orientation::Vertical, 16);
    l.set_hexpand(true);
    let r = gtk::Box::new(gtk::Orientation::Vertical, 16);
    r.set_size_request(right, -1);
    row.append(&l);
    row.append(&r);
    parent.append(&row);
    (l, r)
}

/// A red bar across the page: a halt, and the key that lifts it.
fn banner(parent: &gtk::Box, words: &str, action: Option<(&str, Action)>) {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    bar.add_css_class("arbiter-banner");
    bar.append(&icon("halt", 16));
    let l = label(words, "arbiter-banner-text");
    l.set_wrap(true);
    l.set_hexpand(true);
    bar.append(&l);
    if let Some((caption, run)) = action {
        let key = button(caption, "money-secondary");
        key.set_valign(gtk::Align::Center);
        key.connect_clicked(move |_| run());
        bar.append(&key);
    }
    parent.append(&bar);
}

/// The kill switch's bar, from summary `s`, with Restart.
fn halted_banner(ui: &Rc<Ui>, parent: &gtk::Box, s: &Value) {
    let reason = s["halt_reason"].as_str().unwrap_or("the kill switch is on");
    banner(parent, &format!("Arbiter is halted: {reason}. No strategy places an order until you restart."), Some(("Restart", restart_action(ui, None))));
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

/// A titled block of the thread panel, as Tally's panel draws them.
fn panel_block(body: &gtk::Box, name: &str, aside: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.add_css_class("threads-block");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = label(name, "threads-caption");
    title.set_hexpand(true);
    head.append(&title);
    if !aside.is_empty() {
        head.append(&label(aside, "threads-panel-aside"));
    }
    b.append(&head);
    body.append(&b);
    b
}

/// A line drawn over `points` (`[x, y]`), with a wash under it. `zero` draws the zero line and
/// keeps it in range; `split` marks an x with a dashed line.
fn line_area(points: Vec<(f64, f64)>, height: i32, hex: &'static str, zero: bool, split: Option<f64>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(height);
    area.set_hexpand(true);
    area.set_draw_func(move |_, cr, width, height| {
        let (w, h) = (f64::from(width), f64::from(height));
        let (Some(first), Some(last)) = (points.first(), points.last()) else { return };
        let (x0, x1) = (first.0, last.0);
        let (mut lo, mut hi) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
        if zero {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        if hi - lo < 1e-9 {
            hi = lo + 1.0;
        }
        let pad = 3.0;
        let x = |t: f64| (t - x0) / (x1 - x0).max(1e-9) * w;
        let y = |v: f64| pad + (hi - v) / (hi - lo) * (h - 2.0 * pad);
        if zero {
            let (r, g, b) = rgb("#33302D");
            cr.set_source_rgb(r, g, b);
            cr.set_line_width(1.0);
            cr.move_to(0.0, y(0.0).round() + 0.5);
            cr.line_to(w, y(0.0).round() + 0.5);
            let _ = cr.stroke();
        }
        if let Some(at) = split.filter(|at| *at > x0 && *at < x1) {
            let (r, g, b) = rgb("#8C877F");
            cr.set_source_rgb(r, g, b);
            cr.set_line_width(1.0);
            cr.set_dash(&[3.0, 3.0], 0.0);
            cr.move_to(x(at).round() + 0.5, 0.0);
            cr.line_to(x(at).round() + 0.5, h);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
        }
        let (r, g, b) = rgb(hex);
        let base = if zero { y(0.0) } else { h };
        cr.move_to(x(first.0), base);
        for p in &points {
            cr.line_to(x(p.0), y(p.1));
        }
        cr.line_to(x(last.0), base);
        cr.close_path();
        cr.set_source_rgba(r, g, b, 0.12);
        let _ = cr.fill();
        for (i, p) in points.iter().enumerate() {
            if i == 0 { cr.move_to(x(p.0), y(p.1)) } else { cr.line_to(x(p.0), y(p.1)) }
        }
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(1.75);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        let _ = cr.stroke();
    });
    area
}

fn rgb(hex: &str) -> (f64, f64, f64) {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xEDE9E2);
    (f64::from((v >> 16) & 0xFF) / 255.0, f64::from((v >> 8) & 0xFF) / 255.0, f64::from(v & 0xFF) / 255.0)
}

const GREEN: &str = "#2ec469";
const RED: &str = "#f0786f";

/// `[unix, value]` pairs as an equity line, green when it ended up, red when down.
fn sparkline(points: &Value, height: i32) -> Option<gtk::DrawingArea> {
    let pts: Vec<(f64, f64)> = points.as_array()?.iter().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect();
    if pts.len() < 2 {
        return None;
    }
    let up = pts[pts.len() - 1].1 >= pts[0].1;
    Some(line_area(pts, height, if up { GREEN } else { RED }, false, None))
}

// ---- Keeping a field's typing across a rebuild ---------------------------------------------

type Typing = Option<(String, String, i32)>;

/// The Arbiter field being typed in, if one is: its name, its text and the cursor.
fn typing(ui: &Ui) -> Typing {
    gtk::prelude::GtkWindowExt::focus(&ui.window)
        .and_then(|w| w.ancestor(gtk::Entry::static_type()))
        .and_downcast::<gtk::Entry>()
        .filter(|e| e.widget_name().starts_with("arbiter-"))
        .map(|e| (e.widget_name().to_string(), e.text().to_string(), e.position()))
}

fn wipe(body: &gtk::Box) {
    REBUILDING.with(|r| r.set(true));
    clear(body);
    REBUILDING.with(|r| r.set(false));
}

fn find(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == name {
            return Some(widget);
        }
        if let Some(found) = find(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn restore(body: &gtk::Box, typing: Typing) {
    let Some((name, words, position)) = typing else { return };
    if let Some(entry) = find(body.upcast_ref(), &name).and_downcast::<gtk::Entry>() {
        entry.set_text(&words);
        entry.grab_focus();
        entry.set_position(position);
    }
}

// ---- The summary, the sidebar lamp and the kill switch -------------------------------------

/// The sidebar key's lamp, made once by `money::install`, hidden while nothing runs.
pub(super) fn lamp_widget() -> gtk::Box {
    let l = lamp("");
    l.set_visible(false);
    STATE.with(|s| *s.lamp.borrow_mut() = Some(l.clone()));
    l
}

fn cached() -> Option<Value> {
    STATE.with(|s| s.summary.borrow().clone())
}

/// Keep a summary for the lamp and the cards, and light the lamp: red when anything is halted,
/// green when a live strategy runs, amber when only paper ones do.
fn remember(s: &Value) {
    STATE.with(|st| *st.summary.borrow_mut() = Some(s.clone()));
    let Some(l) = STATE.with(|st| st.lamp.borrow().clone()) else { return };
    let strategies = rows(s, "strategies");
    let tone = if s["halted"] == true || strategies.iter().any(|r| text(r, "state") == "halted") {
        "arbiter-halted"
    } else if strategies.iter().any(|r| row_tone(r) == "arbiter-live") {
        "arbiter-live"
    } else if strategies.iter().any(|r| row_tone(r) == "arbiter-paper") {
        "arbiter-paper"
    } else {
        ""
    };
    for t in ["arbiter-live", "arbiter-paper", "arbiter-halted"] {
        l.remove_css_class(t);
    }
    if !tone.is_empty() {
        l.add_css_class(tone);
    }
    l.set_visible(!tone.is_empty());
    l.set_tooltip_text(Some(match tone {
        "arbiter-halted" => "Arbiter is halted",
        "arbiter-live" => "A live strategy is running",
        _ => "Paper strategies are running",
    }));
}

/// Read the summary for the lamp; one read at a time. An engine without Arbiter stays quiet.
pub(super) fn read_summary(ui: &Rc<Ui>) {
    if STATE.with(|s| s.reading.replace(true)) {
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let read = ui.call("arbiter.summary", json!({})).await;
        STATE.with(|s| s.reading.set(false));
        if let Ok(s) = read {
            remember(&s);
        }
    });
}

/// `arbiter.changed`: the lamp and the conversation's cards; `money` redraws the page showing.
pub(super) fn changed(ui: &Rc<Ui>) {
    read_summary(ui);
    refresh_cards();
}

/// The kill switch, after a yes: cancel every open order and halt every strategy.
pub(super) fn halt_all(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let panel = Panel::new(&ui, "Halt everything?", 440);
        panel.add_css_class("money-sheet");
        let about = label(
            "This cancels every open order and stops every strategy, paper and live, until you restart. What they hold is kept: selling it is a separate step on each strategy.",
            "money-muted",
        );
        about.set_wrap(true);
        panel.body.append(&about);
        if !panel.response("Halt all").await {
            return;
        }
        match ui.call("arbiter.halt", json!({"reason": "you pressed Halt all"})).await {
            Ok(v) => {
                let cancelled = v["cancelled"].as_u64().unwrap_or(0);
                let failed = rows(&v, "failed");
                let mut words = match cancelled {
                    0 => String::from("Halted. There were no open orders."),
                    1 => String::from("Halted. 1 open order cancelled."),
                    n => format!("Halted. {n} open orders cancelled."),
                };
                if !failed.is_empty() {
                    let why: Vec<&str> = failed.iter().filter_map(Value::as_str).collect();
                    words.push_str(&format!(" Not cancelled: {}", why.join("; ")));
                }
                toast(&ui, &words, None);
            }
            Err(e) => ui.show_error(&said(&e)),
        }
        ui.refresh_page();
    });
}

/// The kill switch's key, from summary `s`: off while everything is already halted, saying so.
fn halt_key(ui: &Rc<Ui>, s: &Value) -> gtk::Button {
    let halt = button("Halt all", "money-destructive");
    let halted = s["halted"] == true;
    halt.set_sensitive(!halted);
    halt.set_tooltip_text(Some(if halted { "Already halted. Restart from the banner to run again" } else { "Cancel every open order and stop every strategy until you restart" }));
    let weak = Rc::downgrade(ui);
    halt.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            halt_all(&ui);
        }
    });
    halt
}

/// Lift a halt: one strategy's, or the kill switch's.
fn restart_action(ui: &Rc<Ui>, strategy: Option<i64>) -> Action {
    let weak = Rc::downgrade(ui);
    Rc::new(move || {
        let Some(ui) = weak.upgrade() else { return };
        glib::spawn_future_local(async move {
            match ui.call("arbiter.restart", json!({"strategy_id": strategy})).await {
                Ok(_) => toast(&ui, if strategy.is_some() { "Restarted. It runs again from its next bar." } else { "Restarted. Strategies run again from their next bar." }, None),
                Err(e) => ui.show_error(&said(&e)),
            }
            ui.refresh_page();
        });
    })
}

// ---- The pages -----------------------------------------------------------------------------

/// Arbiter's tabs as Dev's segmented control, above each Arbiter page.
pub fn tabs(showing: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("money-tabs");
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("view-tabs");
    tabs.set_halign(gtk::Align::Start);
    for (name, caption) in PAGES {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("{name}-tab"));
        if name == showing {
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
    for (name, title) in ALL.into_iter().zip(["Arbiter", "Orders", "Settings", "Strategy"]) {
        if let Some(page) = page(name) {
            heading(&page.head, &title_label(title), None, &[]);
            message(&page.body, "Reading Arbiter…");
        }
    }
}

/// Open strategy `id` on the Strategy page.
pub(super) fn open_strategy(ui: &Rc<Ui>, id: i64) {
    STATE.with(|s| s.strategy.set(Some(id)));
    if *ui.page.borrow() == STRATEGY {
        ui.refresh_page();
    } else {
        ui.navigate(STRATEGY);
    }
}

pub async fn refresh(ui: &Rc<Ui>, name: &str) {
    let Some(page) = page(name) else { return };
    match name {
        "arbiter-home" => {
            let read = ui.call("arbiter.summary", json!({})).await;
            if *ui.page.borrow() != name {
                return;
            }
            match read {
                Ok(s) => home(ui, &page, &s),
                Err(e) => failed(ui, &page, "Arbiter", &e),
            }
        }
        "arbiter-orders" => {
            let (orders, decisions, proposals) = tokio::join!(
                ui.call("arbiter.order.list", json!({"limit": 100})),
                ui.call("arbiter.decision.list", json!({"limit": 100})),
                ui.call("arbiter.proposal.list", json!({"limit": 50}))
            );
            if *ui.page.borrow() != name {
                return;
            }
            match (orders, decisions) {
                (Ok(o), Ok(d)) => orders_page(&page, &o, &d, proposals.ok().as_ref()),
                (Err(e), _) | (_, Err(e)) => failed(ui, &page, "Orders", &e),
            }
        }
        "arbiter-settings" => {
            let (settings, summary) = tokio::join!(ui.call("arbiter.settings.get", json!({})), ui.call("arbiter.summary", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            if let Ok(s) = &summary {
                remember(s);
            }
            match settings {
                Ok(st) => settings_page(ui, &page, &st, summary.ok().as_ref()),
                Err(e) => failed(ui, &page, "Settings", &e),
            }
        }
        STRATEGY => {
            let Some(id) = STATE.with(|s| s.strategy.get()) else {
                ui.navigate("arbiter-home");
                return;
            };
            let (detail, summary) = tokio::join!(ui.call("arbiter.strategy.get", json!({"id": id})), ui.call("arbiter.summary", json!({})));
            if *ui.page.borrow() != name || STATE.with(|s| s.strategy.get()) != Some(id) {
                return;
            }
            if let Ok(s) = &summary {
                remember(s);
            }
            match detail {
                Ok(d) => strategy_page(ui, &page, &d),
                Err(e) => failed(ui, &page, "Strategy", &e),
            }
        }
        _ => {}
    }
}

fn failed(ui: &Rc<Ui>, page: &Page, name: &str, error: &Error) {
    heading(&page.head, &title_label(name), None, &[]);
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

/// "Coinbase · portfolio 3f2a9c1e… · checked 12s ago", with a lamp: green connected, red when the
/// last check failed, none without a key.
fn connection_line(c: &Value) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("arbiter-connection");
    let exchange = match text(c, "exchange") {
        "" | "coinbase" => "Coinbase",
        other => other,
    };
    let (tone, words) = match text(c, "state") {
        "ok" => {
            let mut parts = vec![exchange.to_string()];
            if let Some(p) = c["permissions"]["portfolio_uuid"].as_str().filter(|p| !p.is_empty()) {
                parts.push(format!("portfolio {}…", &p[..p.len().min(8)]));
            }
            if let Some(when) = c["checked_at"].as_str().map(ago).filter(|w| !w.is_empty()) {
                parts.push(format!("checked {when}"));
            }
            ("arbiter-live", parts.join(" · "))
        }
        "error" => ("arbiter-halted", format!("{exchange} · {}", c["error"].as_str().unwrap_or("the last check failed"))),
        _ => ("", String::from("Paper only · no Coinbase key yet")),
    };
    row.append(&lamp(tone));
    let l = label(&words, "money-context");
    l.set_wrap(true);
    if tone == "arbiter-halted" {
        l.add_css_class("money-over");
    }
    row.append(&l);
    row
}

/// The page's offer to connect a key, saying what works without one.
fn connect_card(ui: &Rc<Ui>, parent: &gtk::Box) {
    let rows = card(parent, "Connect Coinbase", None);
    let b = block(&rows);
    let about = label(
        "Paper trading works without a key: prices are public, and paper fills are simulated on them. A key lets a strategy go live and lets Arbiter read your real balances and fee tier.",
        "money-muted",
    );
    about.set_wrap(true);
    b.append(&about);
    let key = button("Connect Coinbase…", "primary");
    key.set_widget_name("arbiter-connect");
    key.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            connect(&ui);
        }
    });
    b.append(&key);
}

/// Overview: the connection, the halt, then the portfolio and the strategies beside today's
/// limits, what waits for the person, and the fills.
fn home(ui: &Rc<Ui>, page: &Page, s: &Value) {
    remember(s);
    let new_key = button("New strategy", "");
    new_key.set_widget_name("arbiter-new");
    let weak = Rc::downgrade(ui);
    new_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            editor(&ui, None);
        }
    });
    let halt = halt_key(ui, s);
    halt.set_widget_name("arbiter-halt-all");
    heading(&page.head, &title_label("Arbiter"), Some(connection_line(&s["connection"]).upcast_ref()), &[new_key.upcast_ref(), halt.upcast_ref()]);
    wipe(&page.body);
    let body = &page.body;
    if s["halted"] == true {
        halted_banner(ui, body, s);
    }
    if let Some(problem) = s["runner_error"].as_str().filter(|p| !p.is_empty()) {
        let l = label(&format!("The last pass went wrong: {problem}"), "money-problem");
        l.set_wrap(true);
        body.append(&l);
    }
    match text(&s["connection"], "state") {
        "ok" => {}
        "error" => {
            let rows = card(body, "Coinbase", None);
            let b = block(&rows);
            let l = label(&format!("The last check with Coinbase failed: {}. Live strategies place no orders until it passes.", s["connection"]["error"].as_str().unwrap_or("no reason given")), "money-problem");
            l.set_wrap(true);
            b.append(&l);
            let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let check = button("Check again", "money-secondary");
            let weak = Rc::downgrade(ui);
            check.connect_clicked(move |key| {
                if let Some(ui) = weak.upgrade() {
                    check_now(&ui, key);
                }
            });
            keys.append(&check);
            let replace = button("Replace key…", "money-secondary");
            let weak = Rc::downgrade(ui);
            replace.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    connect(&ui);
                }
            });
            keys.append(&replace);
            b.append(&keys);
        }
        _ => connect_card(ui, body),
    }
    let (left, right) = split(body, 360);
    portfolio(&left, s);
    strategies(ui, &left, s);
    limits_today(ui, &right, s);
    waiting(ui, &right, s);
    recent_fills(&right, s);
    let open = rows(s, "open_orders");
    if !open.is_empty() {
        let list = card(&right, "Open orders", None);
        for o in &open {
            list.append(&order_row(o));
        }
    }
}

/// The money: live when a key reads it, else paper; today, since start, cash and fees; equity.
fn portfolio(parent: &gtk::Box, s: &Value) {
    let home = home_currency();
    let live = s["live"].is_object();
    let account = if live { &s["live"] } else { &s["paper"] };
    let rows = card(parent, "Portfolio", None);
    let b = block(&rows);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    top.append(&label(&money(num(&account["value"]), &home), "arbiter-figure"));
    top.append(&if live { pill("Live", "arbiter-live") } else { pill("Paper", "arbiter-paper") });
    b.append(&top);
    let stats = gtk::Grid::new();
    stats.set_column_spacing(18);
    stats.set_row_spacing(4);
    stats.set_column_homogeneous(true);
    let figures = [
        ("Today, after fees", num(&account["pnl_today"]), true),
        ("Since start", num(&account["pnl_total"]), true),
        ("Held as cash", num(&account["cash"]), false),
        ("Fees this month", num(&account["fees_month"]), false),
    ];
    for (i, (caption, value, signed)) in figures.into_iter().enumerate() {
        let cellbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        cellbox.append(&label(caption, "tally-cell-caption"));
        let figure = label(&if signed { money_signed(value, &home) } else { money(value, &home) }, "arbiter-stat");
        if signed {
            add_tone(&figure, value);
        }
        cellbox.append(&figure);
        stats.attach(&cellbox, i as i32, 0, 1, 1);
    }
    b.append(&stats);
    if let Some(line) = sparkline(&account["equity"], 48) {
        line.set_tooltip_text(Some("Value over time"));
        b.append(&line);
    }
    if live {
        let paper = &s["paper"];
        let l = label(&format!("Paper account {} · today {}", money(num(&paper["value"]), &home), money_signed(num(&paper["pnl_today"]), &home)), "money-row-detail");
        b.append(&l);
    }
}

fn strategies(ui: &Rc<Ui>, parent: &gtk::Box, s: &Value) {
    let weak = Rc::downgrade(ui);
    let add: Action = Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            editor(&ui, None);
        }
    });
    let list = card(parent, "Strategies", Some(("New strategy", add)));
    let all = rows(s, "strategies");
    if all.is_empty() {
        list.append(&icon_row(
            &mark("arbiter"),
            "No strategies yet",
            "Start from a template: a weekly buy, buying the dip, a trend or a breakout. A new strategy starts on paper.",
            "",
            "",
        ));
    }
    for r in &all {
        list.append(&strategy_row(ui, r));
    }
}

/// A strategy in one row: its mark, name and pill over who decides and what it is doing, its
/// profit, and Restart when it is halted. The row opens the strategy.
fn strategy_row(ui: &Rc<Ui>, r: &Value) -> gtk::Box {
    let id = r["id"].as_i64().unwrap_or(0);
    let currency = text(r, "currency").to_string();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("tally-row");
    row.add_css_class("tally-entry");
    let main = button("", "money-row-key");
    main.set_widget_name(&format!("arbiter-strategy-{id}"));
    main.set_hexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    inner.append(&mark(rule_icon(r)));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = label(text(r, "name"), "money-row-title");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    top.append(&name);
    top.append(&venue_pill(r));
    words.append(&top);
    let doing = match text(r, "state") {
        "halted" => format!("Halted: {}", r["halt_reason"].as_str().unwrap_or("a limit was reached")),
        "stopped" => format!("Stopped · {}", mode_name(text(r, "mode"))),
        _ => format!("{} · {}", mode_name(text(r, "mode")), text(r, "doing")),
    };
    let d = label(&doing, "money-row-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if text(r, "state") == "halted" {
        d.add_css_class("money-over");
    }
    words.append(&d);
    inner.append(&words);
    let figures = gtk::Box::new(gtk::Orientation::Vertical, 2);
    figures.set_valign(gtk::Align::Center);
    let pnl = num(&r["pnl_realized"]) + num(&r["pnl_open"]);
    let total = label(&money_signed(pnl, &currency), "money-amount");
    total.set_xalign(1.0);
    add_tone(&total, pnl);
    figures.append(&total);
    let today = label(&format!("today {}", money_signed(num(&r["pnl_today"]), &currency)), "money-row-detail");
    today.set_xalign(1.0);
    figures.append(&today);
    inner.append(&figures);
    main.set_child(Some(&inner));
    main.set_tooltip_text(Some(&r["sentences"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n")));
    let weak = Rc::downgrade(ui);
    main.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_strategy(&ui, id);
        }
    });
    row.append(&main);
    if text(r, "state") == "halted" {
        let restart = button("Restart", "money-secondary");
        restart.set_valign(gtk::Align::Center);
        restart.set_margin_end(12);
        let run = restart_action(ui, Some(id));
        restart.connect_clicked(move |_| run());
        row.append(&restart);
    }
    row
}

/// A limit and how much of it today used: a meter red when full, amber past two thirds.
fn limit_meter(l: &Value, currency: &str) -> gtk::Box {
    let unit = text(l, "unit");
    let shown = |x: f64| match unit {
        "money" => money(x, currency),
        "percent" => format!("{x:.1}%"),
        _ => format!("{}", x.round() as i64),
    };
    let used = num(&l["used"]);
    let limit = opt_num(&l["limit"]);
    let b = gtk::Box::new(gtk::Orientation::Vertical, 4);
    b.add_css_class("arbiter-limit");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = label(text(l, "label"), "money-row-title");
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    top.append(&name);
    let fraction = limit.filter(|x| *x > 0.0).map_or(0.0, |x| used / x);
    let tone = if fraction >= 1.0 {
        Tone::Over
    } else if fraction > 0.66 {
        Tone::Ahead
    } else {
        Tone::Plain
    };
    let figures = label(&match limit {
        Some(x) => format!("{} of {}", shown(used), shown(x)),
        None => format!("{} · no limit", shown(used)),
    }, "tally-figures");
    if fraction >= 1.0 {
        figures.add_css_class("money-over");
    } else if fraction > 0.66 {
        figures.add_css_class("money-ahead");
    }
    top.append(&figures);
    b.append(&top);
    // No limit, nothing to fill: an empty bar read as a limit not reached.
    if limit.is_some() {
        b.append(&meter_in(fraction, None, tone, 5));
    }
    b
}

fn limits_today(ui: &Rc<Ui>, parent: &gtk::Box, s: &Value) {
    let weak = Rc::downgrade(ui);
    let settings: Action = Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            ui.navigate("arbiter-settings");
        }
    });
    let list = card(parent, "Limits today", Some(("Settings", settings)));
    let limits = rows(s, "limits");
    if limits.is_empty() {
        quiet_row(&list, "No limits across strategies yet. Set them in Settings.");
    }
    let home = home_currency();
    for l in &limits {
        list.append(&limit_meter(l, &home));
    }
}

fn waiting(ui: &Rc<Ui>, parent: &gtk::Box, s: &Value) {
    let pending = rows(s, "pending");
    let title = if pending.is_empty() { String::from("Waiting for you") } else { format!("Waiting for you · {}", pending.len()) };
    let list = card(parent, &title, None);
    if pending.is_empty() {
        quiet_row(&list, "Nothing waits for your approval.");
    }
    for p in &pending {
        list.append(&proposal_row(ui, p));
    }
}

/// A proposal in a list: its title, why, and Review, Approve and Dismiss.
fn proposal_row(ui: &Rc<Ui>, p: &Value) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.add_css_class("tally-row");
    row.add_css_class("arbiter-proposal-row");
    let title = label(text(p, "title"), "money-row-title");
    title.set_wrap(true);
    row.append(&title);
    let detail = label(&proposal_detail(p, true), "money-row-detail");
    detail.set_wrap(true);
    row.append(&detail);
    if let Some(why) = p["why"].as_str().filter(|w| !w.is_empty()) {
        let l = label(why, "arbiter-why");
        l.set_wrap(true);
        row.append(&l);
    }
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    keys.set_margin_top(4);
    let review = button("Review", "money-text-action");
    let approve = button(approve_caption(p), "primary");
    approve.add_css_class("arbiter-small");
    let dismiss = button("Dismiss", "quiet");
    dismiss.add_css_class("arbiter-small");
    keys.append(&review);
    if let Some(words) = expires_words(p) {
        let l = label(&words, "money-row-detail");
        l.set_valign(gtk::Align::Center);
        keys.append(&l);
    }
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    keys.append(&gap);
    keys.append(&dismiss);
    keys.append(&approve);
    row.append(&keys);
    let shown = p.clone();
    let weak = Rc::downgrade(ui);
    review.connect_clicked(move |_| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(panel) = Panel::toggle(&ui, "Review a proposal", 500) else { return };
        panel.add_css_class("money-sheet");
        panel.body.append(&proposal_card(&shown));
        panel.present();
    });
    for (key, yes) in [(approve, true), (dismiss, false)] {
        let (weak, p, keys) = (Rc::downgrade(ui), p.clone(), keys.clone());
        key.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            keys.set_sensitive(false);
            let keys = keys.clone();
            let ui2 = ui.clone();
            resolve(&ui, &p, yes, Box::new(move |result| {
                if let Err(e) = result {
                    keys.set_sensitive(true);
                    ui2.show_error(&e);
                }
                ui2.refresh_page();
            }));
        });
    }
    row
}

/// Answer proposal `p`; `done` hears the proposal as it now is, or the engine's refusal.
fn resolve(ui: &Rc<Ui>, p: &Value, approve: bool, done: Box<dyn Fn(Result<Value, String>)>) {
    let payload = json!({"id": p["id"], "approve": approve, "draft_hash": p["draft_hash"]});
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        done(ui.call("arbiter.proposal.resolve", payload).await.map_err(|e| said(&e)));
    });
}

fn recent_fills(parent: &gtk::Box, s: &Value) {
    let list = card(parent, "Recent fills", None);
    let fills = rows(s, "fills");
    if fills.is_empty() {
        quiet_row(&list, "No fills yet.");
    }
    for f in fills.iter().take(8) {
        list.append(&fill_row(f));
    }
}

/// "Bought 0.0123 ETH at $4,123.00", where and when; a sell shows what it made.
fn fill_row(f: &Value) -> gtk::Box {
    let (base, quote) = pair(text(f, "product"));
    let buy = text(f, "side") == "buy";
    let (size, price) = (num(&f["size"]), num(&f["price"]));
    let title = format!("{} {} {base} at {}", if buy { "Bought" } else { "Sold" }, trimmed(size, 8), price_text(price, quote));
    let detail = [
        f["strategy"].as_str().unwrap_or("By hand").to_string(),
        String::from(if text(f, "venue") == "live" { "Live" } else { "Paper" }),
        ago(text(f, "at")),
    ];
    let (figure, class) = match (buy, opt_num(&f["pnl"])) {
        (false, Some(p)) => (money_signed(p, quote), tone_of(p).unwrap_or("")),
        _ => (money(size * price, quote), ""),
    };
    icon_row(&tile(side_icon(text(f, "side"))), &title, &detail.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · "), &figure, class)
}

/// What an order did or will do: "Bought 0.0121 ETH at $4,123.00", "Buy $50.00 of ETH".
fn order_title(o: &Value) -> String {
    let (base, quote) = pair(text(o, "product"));
    let buy = text(o, "side") == "buy";
    let filled = num(&o["filled_base"]);
    if text(o, "status") == "filled" && filled > 0.0 {
        let at = opt_num(&o["average_price"]).map(|p| format!(" at {}", price_text(p, quote))).unwrap_or_default();
        return format!("{} {} {base}{at}", if buy { "Bought" } else { "Sold" }, trimmed(filled, 8));
    }
    format!("{} {}", if buy { "Buy" } else { "Sell" }, order_amount(o))
}

fn order_amount(o: &Value) -> String {
    let (base, quote) = pair(text(o, "product"));
    match (opt_num(&o["quote_size"]), opt_num(&o["base_size"])) {
        (Some(q), _) => format!("{} of {base}", money(q, quote)),
        (None, Some(b)) => format!("{} {base}", trimmed(b, 8)),
        _ => format!("all the {base}"),
    }
}

fn status_word(status: &str) -> &'static str {
    match status {
        "pending" => "Sending",
        "open" => "Open",
        "filled" => "Filled",
        "cancelled" => "Cancelled",
        "expired" => "Expired",
        "failed" => "Failed",
        _ => "Unknown",
    }
}

fn status_pill(o: &Value) -> gtk::Label {
    let status = text(o, "status");
    let p = pill(status_word(status), if status == "failed" { "arbiter-halted" } else { "" });
    if let Some(e) = o["error"].as_str() {
        p.set_tooltip_text(Some(e));
    }
    p
}

fn source_word(source: &str) -> &'static str {
    match source {
        "rule" => "its rule",
        "agent" => "an agent",
        _ => "you",
    }
}

/// An order as a row: what it does, who asked and where, its status.
fn order_row(o: &Value) -> gtk::Box {
    let mut detail = vec![
        o["strategy"].as_str().unwrap_or("By hand").to_string(),
        format!("by {}", source_word(text(o, "source"))),
        String::from(if text(o, "venue") == "live" { "Live" } else { "Paper" }),
        ago(text(o, "created_at")),
    ];
    if let Some(e) = o["error"].as_str().filter(|e| !e.is_empty()) {
        detail.push(e.to_string());
    }
    let row = icon_row(&tile(side_icon(text(o, "side"))), &order_title(o), &detail.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · "), "", "");
    if let Some(why) = o["why"].as_str().filter(|w| !w.is_empty()) {
        row.set_tooltip_text(Some(why));
    }
    let p = status_pill(o);
    row.append(&p);
    row
}

/// One line of the decision log, with a lamp for what kind of decision it was.
fn decision_row(d: &Value) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("tally-row");
    let tone = match text(d, "kind") {
        "refused" | "halt" | "error" => "arbiter-halted",
        "approved" | "restart" => "arbiter-live",
        "proposal" => "arbiter-paper",
        _ => "",
    };
    let dot = lamp(tone);
    dot.set_valign(gtk::Align::Start);
    dot.set_margin_top(6);
    row.append(&dot);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    let l = label(text(d, "text"), "arbiter-log");
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    if tone == "arbiter-halted" {
        l.add_css_class("money-over");
    }
    words.append(&l);
    words.append(&label(&ago(text(d, "at")), "money-row-detail"));
    row.append(&words);
    row
}

/// Orders: every order with what decided it, beside the decision log and the proposals.
fn orders_page(page: &Page, orders: &Value, decisions: &Value, proposals: Option<&Value>) {
    heading(&page.head, &title_label("Orders"), Some(&context_label("Every order and the decision behind it, newest first. Hover an order for why it was placed.")), &[]);
    wipe(&page.body);
    let (left, right) = split(&page.body, 400);
    let list = card(&left, "Orders", None);
    let all = rows(orders, "orders");
    if all.is_empty() {
        quiet_row(&list, "No orders yet. A strategy's first order shows here, paper or live.");
    }
    for o in &all {
        list.append(&order_row(o));
    }
    if let Some(proposals) = proposals {
        let list = card(&left, "Proposals", None);
        let all = rows(proposals, "proposals");
        if all.is_empty() {
            quiet_row(&list, "No proposals yet. A thread's agent proposes from a conversation.");
        }
        for p in &all {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("tally-row");
            let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
            words.set_hexpand(true);
            let title = label(text(p, "title"), "money-row-title");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            words.append(&title);
            let line = p["outcome"].as_str().filter(|o| !o.is_empty()).map_or_else(|| proposal_detail(p, false), str::to_string);
            let d = label(&format!("{line} · {}", ago(text(p, "created_at"))), "money-row-detail");
            d.set_ellipsize(gtk::pango::EllipsizeMode::End);
            words.append(&d);
            row.append(&words);
            row.append(&proposal_pill(text(p, "status")));
            list.append(&row);
        }
    }
    let log = card(&right, "Decisions", None);
    let all = rows(decisions, "decisions");
    if all.is_empty() {
        quiet_row(&log, "Nothing decided yet.");
    }
    for d in &all {
        log.append(&decision_row(d));
    }
}

// ---- Settings ------------------------------------------------------------------------------

/// Settings: the Coinbase key, the paper account, and the limits across every strategy.
fn settings_page(ui: &Rc<Ui>, page: &Page, st: &Value, s: Option<&Value>) {
    let typing = typing(ui);
    heading(
        &page.head,
        &title_label("Settings"),
        Some(&context_label("Arbiter keeps its book in arbiter.db beside Relay's store, and the Coinbase key in the system keyring.")),
        &[],
    );
    wipe(&page.body);
    let pair = columns(&page.body, 2);
    // An open order holds the key (it is how the order is read back and cancelled) and paper's
    // history: the engine refuses both until a halt cancels it.
    let open = s.map(|s| rows(s, "open_orders")).unwrap_or_default();
    let (live_open, paper_open) = (open.iter().any(|o| text(o, "venue") == "live"), open.iter().any(|o| text(o, "venue") != "live"));
    coinbase_card(ui, &pair[0], s.map_or(&Value::Null, |s| &s["connection"]), live_open);
    paper_card(ui, &pair[0], st, paper_open);
    global_limits(ui, &pair[1], st);
    restore(&page.body, typing);
}

/// The key and what Coinbase says of it; `open` while a live order is open, which Remove waits for.
fn coinbase_card(ui: &Rc<Ui>, column: &gtk::Box, c: &Value, open: bool) {
    let list = card(column, "Coinbase", None);
    let state = text(c, "state");
    match state {
        "ok" | "error" => {
            list.append(&icon_row(&mark("key"), "API key", c["key"].as_str().unwrap_or("Saved in the system keyring"), "", ""));
            let p = &c["permissions"];
            let can = match (p["can_view"] == true, p["can_trade"] == true) {
                (_, true) => "Can view and trade",
                (true, false) => "Can view, cannot trade: strategies cannot go live",
                _ => "Cannot view",
            };
            let transfer = if p["can_transfer"] == true { "can transfer" } else { "cannot transfer" };
            list.append(&icon_row(&tile("insurance"), &format!("{can} · {transfer}"), &portfolio_words(p), "", ""));
            list.append(&icon_row(&tile("fees"), &fee_words(&c["fees"]), "Fees come from your own tier, read from Coinbase", "", ""));
            if state == "error" {
                let l = label(&format!("The last check failed: {}", c["error"].as_str().unwrap_or("no reason given")), "money-problem");
                l.set_wrap(true);
                l.add_css_class("arbiter-quiet-row");
                list.append(&l);
            } else if let Some(when) = c["checked_at"].as_str().map(ago).filter(|w| !w.is_empty()) {
                quiet_row(&list, &format!("Checked {when}."));
            }
            let check = button("Check now", "");
            check.set_widget_name("arbiter-check");
            let weak = Rc::downgrade(ui);
            check.connect_clicked(move |key| {
                if let Some(ui) = weak.upgrade() {
                    check_now(&ui, key);
                }
            });
            setting_row(&list, "gauge", "Check with Coinbase", "Read balances, fees and the key's permissions now.", &check);
            let replace = button("Replace…", "");
            let weak = Rc::downgrade(ui);
            replace.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    connect(&ui);
                }
            });
            setting_row(&list, "card", "Replace the key", "Check a new key and keep it instead of this one.", &replace);
            let remove = button("Remove…", "money-destructive");
            remove.set_widget_name("arbiter-key-remove");
            if open {
                remove.set_sensitive(false);
                remove.set_tooltip_text(Some("A live order is open: halt first, which cancels it"));
            }
            let weak = Rc::downgrade(ui);
            confirm_inline(&remove, "Remove the key", move |key| {
                let Some(ui) = weak.upgrade() else { return };
                key.set_sensitive(false);
                glib::spawn_future_local(async move {
                    match ui.call("arbiter.key.remove", json!({})).await {
                        Ok(_) => toast(&ui, "Removed. Live strategies stopped; paper ones run on.", None),
                        Err(e) => ui.show_error(&said(&e)),
                    }
                    ui.refresh_page();
                });
            });
            setting_row(&list, "trash", "Remove the key", "Forget it from the keyring. Live strategies stop.", &remove);
        }
        _ => {
            list.append(&icon_row(&mark("key"), "No key yet", "Paper trading works without one: prices are public.", "", ""));
            let key = button("Connect…", "primary");
            key.set_widget_name("arbiter-connect");
            let weak = Rc::downgrade(ui);
            key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    connect(&ui);
                }
            });
            setting_row(&list, "card", "Connect Coinbase", "A View and Trade key, never Transfer. It stays in the system keyring.", &key);
        }
    }
}

fn portfolio_words(p: &Value) -> String {
    match p["portfolio_uuid"].as_str().filter(|u| !u.is_empty()) {
        Some(u) => format!("Limited to portfolio {}…{}", &u[..u.len().min(8)], p["portfolio_type"].as_str().map(|t| format!(" ({})", t.to_lowercase())).unwrap_or_default()),
        None => String::from("Reaches the whole account: a portfolio of its own would keep Arbiter to what it may use"),
    }
}

/// "Fee tier Advanced 1 · maker 0.40% · taker 0.60%".
fn fee_words(f: &Value) -> String {
    let rate = |key: &str| opt_num(&f[key]).map_or_else(|| String::from("?"), |r| format!("{:.2}%", r * 100.0));
    let tier = f["tier"].as_str().map_or_else(|| String::from("Entry fee tier"), |t| format!("Fee tier {t}"));
    format!("{tier} · maker {} · taker {}", rate("maker"), rate("taker"))
}

fn check_now(ui: &Rc<Ui>, key: &gtk::Button) {
    key.set_sensitive(false);
    let (ui, key) = (ui.clone(), key.clone());
    glib::spawn_future_local(async move {
        match ui.call("arbiter.refresh", json!({})).await {
            Ok(c) if c["state"] == "ok" => toast(&ui, "Checked with Coinbase.", None),
            Ok(c) => ui.show_error(&format!("Coinbase: {}", c["error"].as_str().unwrap_or("the check failed"))),
            Err(e) => ui.show_error(&said(&e)),
        }
        key.set_sensitive(true);
        ui.refresh_page();
    });
}

/// A field for a settings row: right-aligned, named so a rebuild keeps its typing.
fn field_entry(name: &str, value: &str, width: i32) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.add_css_class("money-field");
    e.set_widget_name(name);
    e.set_width_chars(width);
    gtk::prelude::EntryExt::set_alignment(&e, 1.0);
    e.set_placeholder_text(Some("None"));
    e.set_text(value);
    // A field a save refused is marked until it is typed in again.
    e.connect_changed(|e| e.remove_css_class("error"));
    e
}

/// Enter in `entry` presses `key`, as a click does; not again while its save is on its way.
fn enter_presses(entry: &gtk::Entry, key: &gtk::Button) {
    let key = key.downgrade();
    entry.connect_activate(move |_| {
        if let Some(key) = key.upgrade().filter(|k| k.is_sensitive()) {
            key.emit_clicked();
        }
    });
}

/// A value as a field shows it: a decimal string as written, a number, or nothing.
fn shown_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// The paper account; `open` while a paper order is open, which Start over must wait for.
fn paper_card(ui: &Rc<Ui>, column: &gtk::Box, st: &Value, open: bool) {
    let list = card(column, "Paper account", None);
    let home = field_entry("arbiter-home-currency", text(st, "home"), 8);
    home.set_max_length(5);
    home.set_placeholder_text(Some("CAD"));
    // An empty unit slot, so its edge meets the amount's under it.
    setting_row(&list, "coins", "Home currency", "Totals are shown in it. Each strategy trades in its product's own currency.", &with_unit(&home, ""));
    let cash = field_entry("arbiter-paper-cash", &shown_value(&st["paper_cash"]), 8);
    cash.update_property(&[gtk::accessible::Property::Label("Paper starts with")]);
    let currency = if text(st, "home").is_empty() { "CAD" } else { text(st, "home") };
    setting_row(&list, "cash", "Paper starts with", "What the paper account holds when it starts, in the home currency.", &with_unit(&cash, currency));
    let save = button("Save", "");
    save.set_widget_name("arbiter-settings-save");
    save.set_halign(gtk::Align::End);
    save.add_css_class("tally-save");
    list.append(&save);
    enter_presses(&home, &save);
    enter_presses(&cash, &save);
    let weak = Rc::downgrade(ui);
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let code = home.text().trim().to_uppercase();
        if !(3..=5).contains(&code.len()) || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            home.add_css_class("error");
            toast(&ui, "A currency is a code such as CAD or USDC.", None);
            return;
        }
        let Some(start) = decimal(&cash.text()) else {
            cash.add_css_class("error");
            toast(&ui, "Type what paper starts with, like 1000.", None);
            return;
        };
        key.set_sensitive(false);
        let key = key.clone();
        glib::spawn_future_local(async move {
            match ui.call("arbiter.settings.set", json!({"home": code, "paper_cash": start})).await {
                Ok(_) => toast(&ui, "Saved.", None),
                Err(e) => ui.show_error(&said(&e)),
            }
            key.set_sensitive(true);
            ui.refresh_page();
        });
    });
    let reset = button("Start over…", "money-destructive");
    reset.set_widget_name("arbiter-paper-reset");
    if open {
        reset.set_sensitive(false);
        reset.set_tooltip_text(Some("A paper order is open: halt first"));
    }
    let weak = Rc::downgrade(ui);
    confirm_inline(&reset, "Erase paper history", move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        glib::spawn_future_local(async move {
            match ui.call("arbiter.settings.set", json!({"reset_paper": true})).await {
                Ok(_) => toast(&ui, "Paper starts over with its starting cash.", None),
                Err(e) => ui.show_error(&said(&e)),
            }
            ui.refresh_page();
        });
    });
    setting_row(&list, "trash", "Start paper over", "Erases every paper order, fill and record, and starts again with the cash above. Live history is kept.", &reset);
}

/// The limits across every strategy: (key, caption, about, unit, required).
const GLOBAL: [(&str, &str, &str, &str, bool); 6] = [
    ("max_exposure", "Most held, all strategies", "At cost, in the home currency.", "money", false),
    ("daily_loss", "Daily loss, then halt all", "A loss today across strategies that halts every one of them.", "money", false),
    ("orders_per_hour", "Orders per hour", "Across every strategy.", "count", false),
    ("approval_minutes", "Approvals wait", "Minutes a card waits for your yes before it expires.", "count", true),
    ("price_band_pct", "Limit price band", "How far from the best price a limit order may sit, percent.", "percent", true),
    ("slippage_pct", "Slippage assumed", "Added to market orders in backtests and paper fills, percent.", "percent", true),
];

/// A field with its unit after it ("1000 CAD", "10 min", "2 %"), in a slot of one width so the
/// fields of a card keep one edge.
fn with_unit(entry: &gtk::Entry, unit: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let suffix = label(unit, "money-suffix");
    suffix.set_valign(gtk::Align::Center);
    suffix.set_xalign(0.0);
    suffix.set_size_request(48, -1);
    // As wide as it asks and no wider, whatever the description beside it wraps to.
    entry.set_max_width_chars(entry.width_chars());
    entry.set_hexpand(false);
    row.append(entry);
    row.append(&suffix);
    row
}

fn global_limits(ui: &Rc<Ui>, column: &gtk::Box, st: &Value) {
    let list = card(column, "Limits across every strategy", None);
    let g = st["global"].clone();
    let home = text(st, "home");
    let home = if home.is_empty() { "CAD" } else { home };
    let mut fields = Vec::new();
    for (key, caption, about, unit, required) in GLOBAL {
        let e = field_entry(&format!("arbiter-global-{key}"), &shown_value(&g[key]), 7);
        e.update_property(&[gtk::accessible::Property::Label(caption)]);
        if required {
            e.set_placeholder_text(Some(""));
        }
        let words = match (unit, key) {
            ("money", _) => home,
            (_, "orders_per_hour") => "an hour",
            (_, "approval_minutes") => "min",
            _ => "%",
        };
        setting_row(&list, match key {
            "max_exposure" => "coins",
            "daily_loss" => "gauge",
            "approval_minutes" => "calendar",
            _ => "list",
        }, caption, about, &with_unit(&e, words));
        fields.push(e);
    }
    let save = button("Save limits", "");
    save.set_widget_name("arbiter-global-save");
    save.set_halign(gtk::Align::End);
    save.add_css_class("tally-save");
    list.append(&save);
    for e in &fields {
        enter_presses(e, &save);
    }
    let weak = Rc::downgrade(ui);
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let mut out = g.as_object().cloned().unwrap_or_default();
        for ((name, caption, _, unit, required), entry) in GLOBAL.iter().zip(&fields) {
            let typed = entry.text().trim().to_string();
            if typed.is_empty() {
                if *required {
                    entry.add_css_class("error");
                    toast(&ui, &format!("{caption} needs a value."), None);
                    return;
                }
                out.insert(name.to_string(), Value::Null);
                continue;
            }
            let value = match *unit {
                "count" => typed.parse::<u32>().ok().filter(|n| *n > 0).map(Value::from),
                _ => decimal(&typed).map(Value::from),
            };
            let Some(value) = value else {
                entry.add_css_class("error");
                toast(&ui, &format!("{caption}: type a number above zero, or leave it empty for no limit."), None);
                return;
            };
            out.insert(name.to_string(), value);
        }
        key.set_sensitive(false);
        let key = key.clone();
        glib::spawn_future_local(async move {
            match ui.call("arbiter.limits.set", json!({"global": Value::Object(out)})).await {
                Ok(_) => toast(&ui, "Saved. Every strategy's next order meets these limits.", None),
                Err(e) => ui.show_error(&said(&e)),
            }
            key.set_sensitive(true);
            ui.refresh_page();
        });
    });
}

// ---- A strategy ----------------------------------------------------------------------------

/// A strategy's limit fields, each a row of `parent`: its entry and the unit beside it.
struct LimitField {
    key: &'static str,
    caption: &'static str,
    unit: &'static str,
    entry: gtk::Entry,
    suffix: gtk::Label,
}

fn limit_fields(parent: &gtk::Box, limits: &Value, currency: &str, prefix: &str) -> Vec<LimitField> {
    LIMITS
        .iter()
        .map(|(key, caption, unit)| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("tally-row");
            row.add_css_class("arbiter-limit-row");
            let name = label(caption, "money-row-title");
            name.set_hexpand(true);
            row.append(&name);
            let suffix = label(match *unit {
                "money" => currency,
                "count" => "an hour",
                _ => "min",
            }, "money-suffix");
            suffix.set_valign(gtk::Align::Center);
            let entry = field_entry(&format!("{prefix}{key}"), &shown_value(&limits[*key]), 8);
            entry.set_valign(gtk::Align::Center);
            entry.update_property(&[gtk::accessible::Property::Label(caption)]);
            if *unit == "money" {
                row.append(&suffix);
                row.append(&entry);
            } else {
                row.append(&entry);
                row.append(&suffix);
            }
            parent.append(&row);
            LimitField { key, caption, unit, entry, suffix }
        })
        .collect()
}

/// The limits as typed, for `arbiter.limits.set` or a draft; an empty field is no limit.
fn read_limits(fields: &[LimitField]) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    for f in fields {
        let typed = f.entry.text().trim().to_string();
        if typed.is_empty() {
            continue;
        }
        let value = match f.unit {
            "money" => decimal(&typed).map(Value::from),
            _ => typed.parse::<u32>().ok().filter(|n| *n > 0).map(Value::from),
        };
        let Some(value) = value else {
            return Err(format!("{}: type a number above zero, like 50, or leave it empty for no limit.", f.caption));
        };
        out.insert(f.key.to_string(), value);
    }
    Ok(Value::Object(out))
}

/// A strategy: its rule, its backtest beside its paper and live records, its trades on a chart;
/// who decides, its limits, its decisions and its orders.
fn strategy_page(ui: &Rc<Ui>, page: &Page, d: &Value) {
    let typing = typing(ui);
    let r = &d["row"];
    let id = r["id"].as_i64().unwrap_or(0);
    let version = r["version"].as_i64().unwrap_or(1);
    let (state, venue) = (text(r, "state"), text(r, "venue"));
    let product = text(r, "product");
    let currency = match text(r, "currency") {
        "" => pair(product).1.to_string(),
        c => c.to_string(),
    };
    let (_, bars_caption, bars_short) = granularity(text(r, "granularity"));

    // The head: back to the overview, the name and its pills, and the keys.
    clear(&page.head);
    let back = button("", "money-text-action");
    let back_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    back_row.append(&crate::icons::image("chevron-left", 12));
    back_row.append(&label("Arbiter", ""));
    back.set_child(Some(&back_row));
    back.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    back.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.navigate("arbiter-home");
        }
    });
    let title = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    title.append(&label(text(r, "name"), "money-title"));
    title.append(&venue_pill(r));
    if state == "stopped" {
        title.append(&pill("Stopped", ""));
    }
    let mut keys: Vec<gtk::Widget> = Vec::new();
    if state == "halted" {
        let restart = button("Restart", "primary");
        let run = restart_action(ui, Some(id));
        restart.connect_clicked(move |_| run());
        keys.push(restart.upcast());
    } else {
        let running = state == "running";
        let go = button(if running { "Stop" } else { "Start" }, if running { "" } else { "primary" });
        go.set_widget_name("arbiter-run");
        let weak = Rc::downgrade(ui);
        let run = move |key: &gtk::Button| {
            if let Some(ui) = weak.upgrade() {
                set_strategy(&ui, key, json!({"id": id, "running": !running}));
            }
        };
        if running {
            go.set_tooltip_text(Some("Stop placing orders; what it holds is kept"));
            go.connect_clicked(run);
        } else if venue == "live" {
            // Starting is what places real orders: a second click says so first.
            go.set_tooltip_text(Some("Run the rule from its next bar, with real money on Coinbase"));
            confirm_inline(&go, "Start with real money", run);
        } else {
            go.set_tooltip_text(Some("Run the rule from its next bar"));
            go.connect_clicked(run);
        }
        keys.push(go.upcast());
    }
    let backtest = button("Backtest", "");
    backtest.set_widget_name("arbiter-backtest");
    let run = backtest_action(ui, id, version);
    backtest.connect_clicked(move |_| run());
    if STATE.with(|s| s.backtesting.borrow().contains(&id)) {
        backtest.set_sensitive(false);
        backtest.set_tooltip_text(Some("Backtesting…"));
    }
    keys.push(backtest.upcast());
    let edit = button("Edit", "");
    edit.set_widget_name("arbiter-edit");
    let (weak, shown) = (Rc::downgrade(ui), d.clone());
    edit.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            editor(&ui, Some(shown.clone()));
        }
    });
    keys.push(edit.upcast());
    if venue == "live" {
        let paper = button("Back to paper", "");
        paper.set_widget_name("arbiter-paper");
        let weak = Rc::downgrade(ui);
        paper.connect_clicked(move |key| {
            if let Some(ui) = weak.upgrade() {
                set_strategy(&ui, key, json!({"id": id, "venue": "paper"}));
            }
        });
        keys.push(paper.upcast());
    } else {
        let live = button("Go live…", "");
        live.set_widget_name("arbiter-go-live");
        let (weak, shown) = (Rc::downgrade(ui), d.clone());
        live.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                go_live(&ui, &shown);
            }
        });
        keys.push(live.upcast());
    }
    keys.push(more_menu(ui, d).upcast());
    let key_refs: Vec<&gtk::Widget> = keys.iter().collect();
    let context = context_label(&format!("Version {version} · {bars_short} bars · {product}"));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.append(&back);
    column.append(&title);
    heading(&page.head, column.upcast_ref(), Some(&context), &key_refs);

    wipe(&page.body);
    let body = &page.body;
    // The kill switch holds every strategy, whatever its own state says.
    if let Some(s) = cached().filter(|s| s["halted"] == true) {
        halted_banner(ui, body, &s);
    }
    if state == "halted" {
        let reason = r["halt_reason"].as_str().unwrap_or("a limit was reached");
        banner(body, &format!("Halted: {reason}. It places no order until you restart it."), Some(("Restart", restart_action(ui, Some(id)))));
    }
    let (left, right) = split(body, 380);

    // The rule, in sentences, and the move it needs to break even.
    let rule = card(&left, "The rule", None);
    let b = block(&rule);
    for s in r["sentences"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let l = label(s, "arbiter-sentence");
        l.set_wrap(true);
        b.append(&l);
    }
    let note = label(
        &format!("A round trip costs about {:.2}% at your fee tier, so this rule needs a move larger than that to make money.", num(&d["break_even_pct"])),
        "money-row-detail",
    );
    note.set_wrap(true);
    note.set_margin_top(4);
    b.append(&note);

    results_card(ui, &left, d, id, version, bars_caption, &currency);
    left.append(&super::chart::price_card(super::chart::PriceSpec {
        title: format!("{product}, {bars_short} bars, with this strategy's trades"),
        query: json!({"product": product, "granularity": text(r, "granularity"), "bars": 300, "strategy_id": id}),
        cutoff: None,
    }));

    who_decides(ui, &right, id, text(r, "mode"));
    let list = card(&right, "Limits", None);
    let fields = Rc::new(limit_fields(&list, &d["limits"], &currency, "arbiter-limit-"));
    let saved = Rc::new(RefCell::new(read_limits(&fields).unwrap_or(Value::Null)));
    for f in fields.iter() {
        let commit = {
            let (weak, fields, saved) = (Rc::downgrade(ui), fields.clone(), saved.clone());
            move || {
                let Some(ui) = weak.upgrade() else { return };
                if REBUILDING.with(|r| r.get()) {
                    return;
                }
                let limits = match read_limits(&fields) {
                    Ok(l) => l,
                    Err(e) => {
                        toast(&ui, &e, None);
                        return;
                    }
                };
                if *saved.borrow() == limits {
                    return;
                }
                *saved.borrow_mut() = limits.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = ui.call("arbiter.limits.set", json!({"strategy_id": id, "limits": limits})).await {
                        ui.show_error(&said(&e));
                    }
                    ui.refresh_page();
                });
            }
        };
        let commit = Rc::new(commit);
        let on_enter = commit.clone();
        f.entry.connect_activate(move |_| on_enter());
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(move |_| commit());
        f.entry.add_controller(focus);
    }
    quiet_row(&list, "Changes save as you leave a field. An empty field is no limit; AI trading needs the first three.");

    let log = card(&right, "Decisions", None);
    let decisions = rows(d, "decisions");
    if decisions.is_empty() {
        quiet_row(&log, "Nothing decided yet.");
    }
    for x in decisions.iter().take(12) {
        log.append(&decision_row(x));
    }
    let list = card(&right, "Orders", None);
    let orders = rows(d, "orders");
    if orders.is_empty() {
        quiet_row(&list, "No orders yet.");
    }
    for o in orders.iter().take(10) {
        list.append(&order_row(o));
    }
    restore(body, typing);
}

/// `arbiter.strategy.set` from a key: the engine's refusal is shown as it says it.
fn set_strategy(ui: &Rc<Ui>, key: &gtk::Button, payload: Value) {
    key.set_sensitive(false);
    let (ui, key) = (ui.clone(), key.clone());
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call("arbiter.strategy.set", payload).await {
            ui.show_error(&said(&e));
            key.set_sensitive(true);
        }
        ui.refresh_page();
    });
}

/// Sell everything and Delete, behind a menu: neither is an everyday key.
fn more_menu(ui: &Rc<Ui>, d: &Value) -> gtk::MenuButton {
    let r = &d["row"];
    let id = r["id"].as_i64().unwrap_or(0);
    let key = gtk::MenuButton::new();
    key.set_icon_name("view-more-symbolic");
    key.set_tooltip_text(Some("More"));
    key.set_valign(gtk::Align::Center);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.add_css_class("threads-menu");
    let popover = gtk::Popover::new();
    let flatten = button("Sell everything…", "nav");
    flatten.set_widget_name("arbiter-flatten");
    let held = num(&r["held_base"]);
    flatten.set_sensitive(held > 0.0);
    if held <= 0.0 {
        flatten.set_tooltip_text(Some("It holds nothing"));
    }
    let (weak, shown, pop) = (Rc::downgrade(ui), d.clone(), popover.downgrade());
    flatten.connect_clicked(move |_| {
        if let Some(p) = pop.upgrade() {
            p.popdown();
        }
        if let Some(ui) = weak.upgrade() {
            sell_everything(&ui, &shown);
        }
    });
    list.append(&flatten);
    let delete = button("Delete strategy", "money-destructive");
    delete.set_widget_name("arbiter-delete");
    // What the engine would refuse, said on the key instead.
    let open = rows(d, "orders").iter().any(|o| matches!(text(o, "status"), "pending" | "open"));
    let blocked = if text(r, "state") == "running" {
        Some("Stop it first")
    } else if held > 0.0 {
        Some("Sell what it holds first")
    } else if open {
        Some("Halt it first: an order is open")
    } else {
        None
    };
    delete.set_sensitive(blocked.is_none());
    delete.set_tooltip_text(blocked);
    let (weak, pop) = (Rc::downgrade(ui), popover.downgrade());
    confirm_inline(&delete, "Delete for good", move |_| {
        if let Some(p) = pop.upgrade() {
            p.popdown();
        }
        let Some(ui) = weak.upgrade() else { return };
        glib::spawn_future_local(async move {
            match ui.call("arbiter.strategy.delete", json!({"id": id})).await {
                Ok(_) => {
                    STATE.with(|s| s.strategy.set(None));
                    ui.navigate("arbiter-home");
                    toast(&ui, "Deleted. Its history stays in the decision log.", None);
                }
                Err(e) => ui.show_error(&said(&e)),
            }
        });
    });
    list.append(&delete);
    popover.set_child(Some(&list));
    key.set_popover(Some(&popover));
    key
}

/// Sell what a strategy holds at the market price, after a yes that says how much and where.
fn sell_everything(ui: &Rc<Ui>, d: &Value) {
    let r = d["row"].clone();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let (base, _) = pair(text(&r, "product"));
        let panel = Panel::new(&ui, "Sell everything?", 440);
        panel.add_css_class("money-sheet");
        let where_ = if text(&r, "venue") == "live" { "on Coinbase, with real money" } else { "on paper" };
        let about = label(
            &format!("Sells the {} {base} {} holds at the market price, {where_}. It works while halted, and does not restart anything.", trimmed(num(&r["held_base"]), 8), text(&r, "name")),
            "money-muted",
        );
        about.set_wrap(true);
        panel.body.append(&about);
        if !panel.response("Sell everything").await {
            return;
        }
        match ui.call("arbiter.flatten", json!({"strategy_id": r["id"]})).await {
            Ok(o) => toast(&ui, &format!("{}: {}.", order_title(&o), status_word(text(&o, "status")).to_lowercase()), None),
            Err(e) => ui.show_error(&said(&e)),
        }
        ui.refresh_page();
    });
}

/// Who decides: the three modes as a segmented control, what the chosen one means, and the
/// engine's refusal (AI trading without limits) under it.
fn who_decides(ui: &Rc<Ui>, parent: &gtk::Box, id: i64, mode: &str) {
    let list = card(parent, "Who decides", None);
    let b = block(&list);
    let segments = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    segments.add_css_class("money-segments");
    segments.set_homogeneous(true);
    let mut group: Option<gtk::ToggleButton> = None;
    let mut toggles = Vec::new();
    for (value, name, short, _) in MODES {
        let key = gtk::ToggleButton::with_label(short);
        key.set_widget_name(&format!("arbiter-mode-{value}"));
        key.set_tooltip_text(Some(name));
        key.set_group(group.as_ref());
        if group.is_none() {
            group = Some(key.clone());
        }
        key.set_active(value == mode);
        segments.append(&key);
        toggles.push((value, key));
    }
    b.append(&segments);
    let current = MODES.iter().find(|m| m.0 == mode).unwrap_or(&MODES[0]);
    let about = label(&format!("{}: {}", current.1, current.3), "money-row-detail");
    about.set_wrap(true);
    b.append(&about);
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    b.append(&problem);
    let toggles = Rc::new(toggles);
    let mode = mode.to_string();
    for (value, key) in toggles.iter() {
        let (weak, toggles, problem, mode, value) = (Rc::downgrade(ui), toggles.clone(), problem.clone(), mode.clone(), *value);
        key.connect_toggled(move |key| {
            if !key.is_active() || value == mode {
                return;
            }
            let Some(ui) = weak.upgrade() else { return };
            let (toggles, problem, mode) = (toggles.clone(), problem.clone(), mode.clone());
            glib::spawn_future_local(async move {
                match ui.call("arbiter.strategy.set", json!({"id": id, "mode": value})).await {
                    Ok(_) => ui.refresh_page(),
                    Err(e) => {
                        problem.set_text(&said(&e));
                        problem.set_visible(true);
                        if let Some((_, previous)) = toggles.iter().find(|(v, _)| *v == mode) {
                            previous.set_active(true);
                        }
                    }
                }
            });
        });
    }
}

/// Run a backtest of the strategy's saved rule over 180 days; the page shows a spinner meanwhile.
fn backtest_action(ui: &Rc<Ui>, id: i64, version: i64) -> Action {
    let weak = Rc::downgrade(ui);
    Rc::new(move || {
        let Some(ui) = weak.upgrade() else { return };
        if !STATE.with(|s| s.backtesting.borrow_mut().insert(id)) {
            return;
        }
        ui.refresh_page();
        glib::spawn_future_local(async move {
            let read = ui.call("arbiter.backtest", json!({"strategy_id": id, "days": 180})).await;
            STATE.with(|s| s.backtesting.borrow_mut().remove(&id));
            match read {
                Ok(report) => STATE.with(|s| {
                    s.backtests.borrow_mut().insert((id, version), report);
                }),
                Err(e) => ui.show_error(&said(&e)),
            }
            ui.refresh_page();
        });
    })
}

/// The backtest beside paper and live, its warnings, the count of versions tried, and the
/// backtest's profit over the range.
fn results_card(ui: &Rc<Ui>, parent: &gtk::Box, d: &Value, id: i64, version: i64, bars: &str, currency: &str) {
    let running = STATE.with(|s| s.backtesting.borrow().contains(&id));
    let list = card(parent, "Backtest, paper, live", if running { None } else { Some(("Run backtest", backtest_action(ui, id, version))) });
    let b = block(&list);
    if running {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&crate::app::spinner());
        row.append(&label(&format!("Backtesting 180 days of {bars} bars on the exchange's candles…"), "money-muted"));
        b.append(&row);
    }
    let report = STATE.with(|s| s.backtests.borrow().get(&(id, version)).cloned()).or_else(|| d["backtest"].is_object().then(|| d["backtest"].clone()));
    b.append(&results_table(report.as_ref(), &d["paper"], &d["live"]));
    let Some(report) = report else {
        if !running {
            let l = label(
                "No backtest of this version yet. A backtest fills each signal at the next bar's open, at your fee tier with slippage, and judges the rule on the last third of the range, which it was not tuned on.",
                "money-row-detail",
            );
            l.set_wrap(true);
            b.append(&l);
        }
        return;
    };
    for w in report["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let l = label(w, "arbiter-warning");
        l.set_wrap(true);
        b.append(&l);
    }
    let tried = d["variants_tried"].as_u64().unwrap_or(0);
    if tried > 1 {
        let l = label(
            &format!("{tried} versions of this rule have been backtested. The more versions tried, the likelier the best one only fits the past."),
            "arbiter-warning",
        );
        l.set_wrap(true);
        b.append(&l);
    }
    let points: Vec<(f64, f64)> = report["equity"].as_array().into_iter().flatten().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect();
    if points.len() > 1 {
        let up = points[points.len() - 1].1 >= 0.0;
        let split = report["judged"]["start"].as_f64();
        let curve = line_area(points.clone(), 110, if up { GREEN } else { RED }, true, split);
        let (lo, hi) = points.iter().fold((0.0_f64, 0.0_f64), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
        curve.set_tooltip_text(Some(&format!("Profit after fees, from {} to {}", money_signed(lo, currency), money_signed(hi, currency))));
        curve.set_margin_top(6);
        b.append(&curve);
        let l = label("Profit after fees over the range. The dashed line is where judging starts.", "money-row-detail");
        l.set_wrap(true);
        b.append(&l);
    }
}

/// One row of the results table: each column's words, and the number that colours them.
type Cells = [(String, Option<f64>); 5];

/// Tuned on, judged on, paper, live and buy and hold, side by side.
fn results_table(report: Option<&Value>, paper: &Value, live: &Value) -> gtk::Grid {
    let grid = gtk::Grid::new();
    grid.add_css_class("arbiter-table");
    grid.set_column_spacing(16);
    grid.set_row_spacing(6);
    let put = |col: i32, row: i32, words: &str, class: &str, tone: Option<f64>| {
        let l = label(words, class);
        if col > 0 {
            l.set_xalign(1.0);
            l.set_hexpand(true);
        }
        if let Some(v) = tone {
            add_tone(&l, v);
        }
        grid.attach(&l, col, row, 1, 1);
    };
    let range = |s: &Value| match (s["start"].as_i64(), s["end"].as_i64()) {
        (Some(a), Some(b)) => format!("{}–{}", day_of(a), day_of(b)),
        _ => String::from("—"),
    };
    let days = |r: &Value| match r["days"].as_f64() {
        Some(d) if d > 0.0 => format!("{d:.0} days"),
        _ => String::from("not yet"),
    };
    let dash = || String::from("—");
    for (col, name) in ["Tuned on", "Judged on", "Paper", "Live", "Buy and hold"].into_iter().enumerate() {
        put(col as i32 + 1, 0, name, "arbiter-th", None);
    }
    let (tuned, judged, whole) = match report {
        Some(r) => (&r["tuned"], &r["judged"], &r["whole"]),
        None => (&Value::Null, &Value::Null, &Value::Null),
    };
    let has = report.is_some();
    let ranges = [if has { range(tuned) } else { dash() }, if has { range(judged) } else { dash() }, days(paper), days(live), if has { range(whole) } else { dash() }];
    for (col, words) in ranges.iter().enumerate() {
        put(col as i32 + 1, 1, words, "arbiter-range", None);
    }
    let ret = |v: Option<f64>| (v.map_or_else(dash, pct), v);
    let record_ret = |r: &Value| if r["trades"].as_u64().unwrap_or(0) > 0 { r["return_pct"].as_f64() } else { None };
    let rows: [(&str, Cells); 5] = [
        ("Return after fees", [
            ret(tuned["return_pct"].as_f64()),
            ret(judged["return_pct"].as_f64()),
            ret(record_ret(paper)),
            ret(record_ret(live)),
            ret(whole["buy_hold_pct"].as_f64()),
        ]),
        ("Worst drawdown", [
            (tuned["max_drawdown_pct"].as_f64().map_or_else(dash, drawdown), None),
            (judged["max_drawdown_pct"].as_f64().map_or_else(dash, drawdown), None),
            (dash(), None),
            (dash(), None),
            (dash(), None),
        ]),
        ("Trades", [
            (tuned["trades"].as_u64().map_or_else(dash, |n| n.to_string()), None),
            (judged["trades"].as_u64().map_or_else(dash, |n| n.to_string()), None),
            (paper["trades"].as_u64().unwrap_or(0).to_string(), None),
            (live["trades"].as_u64().unwrap_or(0).to_string(), None),
            (if has { String::from("1") } else { dash() }, None),
        ]),
        ("Won", [
            (tuned["won_pct"].as_f64().map_or_else(dash, |x| format!("{x:.0}%")), None),
            (judged["won_pct"].as_f64().map_or_else(dash, |x| format!("{x:.0}%")), None),
            (paper["won_pct"].as_f64().map_or_else(dash, |x| format!("{x:.0}%")), None),
            (live["won_pct"].as_f64().map_or_else(dash, |x| format!("{x:.0}%")), None),
            (dash(), None),
        ]),
        ("Time in the market", [
            (tuned["time_in_market_pct"].as_f64().map_or_else(dash, |x| format!("{x:.0}%")), None),
            (judged["time_in_market_pct"].as_f64().map_or_else(dash, |x| format!("{x:.0}%")), None),
            (dash(), None),
            (dash(), None),
            (if has { String::from("100%") } else { dash() }, None),
        ]),
    ];
    for (i, (name, cells)) in rows.iter().enumerate() {
        let row = i as i32 + 2;
        put(0, row, name, "arbiter-rowname", None);
        for (col, (words, tone)) in cells.iter().enumerate() {
            put(col as i32 + 1, row, words, "arbiter-td", *tone);
        }
    }
    grid
}

/// Going live: the paper record and the limits in one confirmation. Without a key that can
/// trade, the Connect Coinbase sheet instead.
fn go_live(ui: &Rc<Ui>, d: &Value) {
    let can_trade = cached().is_some_and(|s| s["connection"]["state"] == "ok" && s["connection"]["permissions"]["can_trade"] == true);
    if !can_trade {
        connect(ui);
        return;
    }
    let d = d.clone();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let r = &d["row"];
        let name = text(r, "name");
        let currency = match text(r, "currency") {
            "" => pair(text(r, "product")).1.to_string(),
            c => c.to_string(),
        };
        let panel = Panel::new(&ui, &format!("Take {name} live?"), 480);
        panel.add_css_class("money-sheet");
        let when = match text(r, "state") {
            "running" => String::from("From its next bar"),
            "halted" => String::from("Once you restart it"),
            _ => String::from("Once you start it"),
        };
        let about = label(
            &format!("{when}, {name} places real orders on Coinbase with real money, within the limits below. Its paper record so far:"),
            "money-muted",
        );
        about.set_wrap(true);
        panel.body.append(&about);
        let paper = &d["paper"];
        let record = gtk::Grid::new();
        record.add_css_class("arbiter-record");
        record.set_column_spacing(18);
        record.set_row_spacing(2);
        let figures = [
            ("On paper", paper["days"].as_f64().map_or_else(|| String::from("not yet"), |x| format!("{x:.0} days")), None),
            ("Trades", paper["trades"].as_u64().unwrap_or(0).to_string(), None),
            ("Won", paper["won_pct"].as_f64().map_or_else(|| String::from("—"), |x| format!("{x:.0}%")), None),
            ("After fees", money_signed(num(&paper["pnl"]), &currency), Some(num(&paper["pnl"]))),
        ];
        for (i, (caption, value, tone)) in figures.into_iter().enumerate() {
            record.attach(&label(caption, "tally-cell-caption"), i as i32, 0, 1, 1);
            let f = label(&value, "arbiter-stat");
            if let Some(v) = tone {
                add_tone(&f, v);
            }
            record.attach(&f, i as i32, 1, 1, 1);
        }
        panel.body.append(&record);
        let limits = &d["limits"];
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        list.add_css_class("arbiter-golive-limits");
        let mut missing = Vec::new();
        for (key, caption, unit) in LIMITS {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let n = label(caption, "money-row-detail");
            n.set_hexpand(true);
            row.append(&n);
            let value = match (unit, opt_num(&limits[key])) {
                (_, None) => {
                    if matches!(key, "max_order" | "max_position" | "daily_loss") {
                        missing.push(caption.to_lowercase());
                    }
                    String::from("No limit")
                }
                ("money", Some(x)) => money(x, &currency),
                ("count", Some(x)) => format!("{x:.0} an hour"),
                (_, Some(x)) => format!("{x:.0} min"),
            };
            row.append(&label(&value, "tally-figures"));
            list.append(&row);
        }
        panel.body.append(&list);
        // The engine refuses a strategy that trades real money on its own with no limit per
        // order: say so here, with nothing to press but Close.
        if opt_num(&limits["max_order"]).is_none() && text(r, "mode") != "ask" {
            let l = label("Give it a limit per order first, under Limits on its page, or let every order ask.", "money-problem");
            l.set_wrap(true);
            panel.body.append(&l);
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            actions.set_halign(gtk::Align::End);
            let close = button("Close", "primary");
            let sheet = Rc::downgrade(&panel);
            close.connect_clicked(move |_| {
                if let Some(sheet) = sheet.upgrade() {
                    sheet.close();
                }
            });
            actions.append(&close);
            panel.body.append(&actions);
            panel.compact(true, 280);
            panel.present();
            close.grab_focus();
            return;
        }
        if !missing.is_empty() {
            let l = label(&format!("Without a limit on {}, nothing but its rule bounds what it spends.", missing.join(", ")), "arbiter-warning");
            l.set_wrap(true);
            panel.body.append(&l);
        }
        if !panel.response("Go live").await {
            return;
        }
        if let Err(e) = ui.call("arbiter.strategy.set", json!({"id": r["id"], "venue": "live"})).await {
            ui.show_error(&said(&e));
        }
        ui.refresh_page();
    });
}

// ---- The strategy editor -------------------------------------------------------------------

/// When a strategy buys, as the editor can change it: one of the templates' shapes, or an entry
/// written elsewhere (by an agent's proposal) that the editor keeps as it is.
#[derive(Clone)]
enum Seed {
    Schedule { every: String, weekday: u32, day: u32, hour: u32 },
    Rsi { period: u32, below: f64 },
    Cross { fast: u32, slow: u32 },
    Breakout { bars: u32 },
    Other { entry: Value, sentence: String },
}

/// A template: its name, a line about it, and its exits (take profit, stop loss, trailing).
const TEMPLATES: [(&str, &str, [&str; 3]); 4] = [
    ("Weekly buy", "A fixed amount every week, whatever the price", ["", "", ""]),
    ("Buy the dip (RSI)", "Buys when RSI(14) falls below 30; sells at +4% or −3%", ["4", "3", ""]),
    ("Trend (moving averages)", "Buys when EMA(12) crosses above EMA(26); sells when it crosses back", ["", "5", ""]),
    ("Breakout", "Buys when the price crosses above its 20-bar high; trails 5%", ["", "", "5"]),
];

fn template_seed(i: usize) -> Seed {
    match i {
        0 => Seed::Schedule { every: String::from("week"), weekday: 1, day: 1, hour: 9 },
        1 => Seed::Rsi { period: 14, below: 30.0 },
        2 => Seed::Cross { fast: 12, slow: 26 },
        _ => Seed::Breakout { bars: 20 },
    }
}

fn u(v: &Value, default: u32) -> u32 {
    v.as_u64().map_or(default, |n| n as u32)
}

/// The editor's reading of a saved rule's entry, and whether its exit is the trend's cross back.
fn seed_of(rule: &Value, sentence: &str) -> (Seed, bool) {
    let e = &rule["entry"];
    if e["kind"] == "schedule" {
        let seed = Seed::Schedule { every: text(e, "every").to_string(), weekday: u(&e["weekday"], 1), day: u(&e["day"], 1), hour: u(&e["hour"], 9) };
        return (seed, false);
    }
    let w = &e["when"];
    if e["kind"] == "signal" && w["kind"] == "compare" {
        let (l, op, r) = (&w["left"], text(w, "op"), &w["right"]);
        match (text(l, "kind"), op, text(r, "kind")) {
            ("rsi", "below", "number") => return (Seed::Rsi { period: u(&l["period"], 14), below: r["value"].as_f64().unwrap_or(30.0) }, false),
            ("ema", "crosses_above", "ema") => {
                let (fast, slow) = (u(&l["period"], 12), u(&r["period"], 26));
                return (Seed::Cross { fast, slow }, rule["exit"]["when"] == cross(fast, slow, "crosses_below"));
            }
            ("price", "crosses_above", "high") => return (Seed::Breakout { bars: u(&r["bars"], 20) }, false),
            _ => {}
        }
    }
    (Seed::Other { entry: e.clone(), sentence: sentence.to_string() }, false)
}

fn cross(fast: u32, slow: u32, op: &str) -> Value {
    json!({"kind": "compare", "left": {"kind": "ema", "period": fast}, "op": op, "right": {"kind": "ema", "period": slow}})
}

/// The entry's own fields, as built for its shape.
enum Params {
    Schedule { every: gtk::DropDown, weekday: gtk::DropDown, day: gtk::SpinButton, hour: gtk::SpinButton },
    Rsi { period: gtk::SpinButton, below: gtk::SpinButton },
    Cross { fast: gtk::SpinButton, slow: gtk::SpinButton, exit: gtk::CheckButton },
    Breakout { bars: gtk::SpinButton },
    Other { entry: Value },
}

fn spin(min: f64, max: f64, value: f64, name: &str) -> gtk::SpinButton {
    let s = gtk::SpinButton::with_range(min, max, 1.0);
    s.set_value(value);
    s.add_css_class("money-field");
    s.set_widget_name(name);
    s.set_valign(gtk::Align::Center);
    s
}

fn words(text: &str) -> gtk::Label {
    let l = label(text, "arbiter-form-words");
    l.set_valign(gtk::Align::Center);
    l
}

/// Build the entry's fields in `slot` for `seed`, as a sentence with its numbers in it.
fn build_params(slot: &gtk::Box, seed: &Seed, exit_cross: bool) -> Params {
    clear(slot);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("arbiter-form-line");
    slot.append(&line);
    match seed {
        Seed::Schedule { every, weekday, day, hour } => {
            let every_key = gtk::DropDown::from_strings(&["Every day", "Every week", "Every month"]);
            every_key.add_css_class("money-picker");
            every_key.set_selected(match every.as_str() {
                "day" => 0,
                "month" => 2,
                _ => 1,
            });
            let weekday_key = gtk::DropDown::from_strings(&WEEKDAYS);
            weekday_key.add_css_class("money-picker");
            weekday_key.set_selected((*weekday).clamp(1, 7) - 1);
            let on = words("on day");
            let day_key = spin(1.0, 28.0, f64::from(*day), "arbiter-edit-day");
            let hour_key = spin(0.0, 23.0, f64::from(*hour), "arbiter-edit-hour");
            line.append(&every_key);
            line.append(&weekday_key);
            line.append(&on);
            line.append(&day_key);
            line.append(&words("at"));
            line.append(&hour_key);
            line.append(&words(":00 UTC"));
            let show = {
                let (weekday_key, on, day_key) = (weekday_key.clone(), on.clone(), day_key.clone());
                move |every: u32| {
                    weekday_key.set_visible(every == 1);
                    on.set_visible(every == 2);
                    day_key.set_visible(every == 2);
                }
            };
            show(every_key.selected());
            every_key.connect_selected_notify(move |key| show(key.selected()));
            let note = label("Hours are UTC, as the exchange counts them.", "money-row-detail");
            slot.append(&note);
            Params::Schedule { every: every_key, weekday: weekday_key, day: day_key, hour: hour_key }
        }
        Seed::Rsi { period, below } => {
            let period_key = spin(2.0, 100.0, f64::from(*period), "arbiter-edit-rsi");
            let below_key = spin(1.0, 99.0, *below, "arbiter-edit-below");
            line.append(&words("Buy when RSI("));
            line.append(&period_key);
            line.append(&words(") falls below"));
            line.append(&below_key);
            Params::Rsi { period: period_key, below: below_key }
        }
        Seed::Cross { fast, slow } => {
            let fast_key = spin(2.0, 200.0, f64::from(*fast), "arbiter-edit-fast");
            let slow_key = spin(3.0, 400.0, f64::from(*slow), "arbiter-edit-slow");
            line.append(&words("Buy when EMA("));
            line.append(&fast_key);
            line.append(&words(") crosses above EMA("));
            line.append(&slow_key);
            line.append(&words(")"));
            let exit = gtk::CheckButton::with_label("Sell when it crosses back below");
            exit.set_active(exit_cross);
            slot.append(&exit);
            Params::Cross { fast: fast_key, slow: slow_key, exit }
        }
        Seed::Breakout { bars } => {
            let bars_key = spin(2.0, 200.0, f64::from(*bars), "arbiter-edit-bars");
            line.append(&words("Buy when the price crosses above its highest high of the last"));
            line.append(&bars_key);
            line.append(&words("bars"));
            Params::Breakout { bars: bars_key }
        }
        Seed::Other { entry, sentence } => {
            let l = label(&format!("Kept as it is, written outside the templates: {sentence}"), "money-row-detail");
            l.set_wrap(true);
            line.append(&l);
            Params::Other { entry: entry.clone() }
        }
    }
}

fn entry_json(p: &Params) -> Value {
    match p {
        Params::Schedule { every, weekday, day, hour } => match every.selected() {
            0 => json!({"kind": "schedule", "every": "day", "hour": hour.value_as_int()}),
            2 => json!({"kind": "schedule", "every": "month", "hour": hour.value_as_int(), "day": day.value_as_int()}),
            _ => json!({"kind": "schedule", "every": "week", "hour": hour.value_as_int(), "weekday": weekday.selected() + 1}),
        },
        Params::Rsi { period, below } => json!({"kind": "signal", "when": {
            "kind": "compare", "left": {"kind": "rsi", "period": period.value_as_int()}, "op": "below", "right": {"kind": "number", "value": below.value()}
        }}),
        Params::Cross { fast, slow, .. } => json!({"kind": "signal", "when": cross(fast.value_as_int() as u32, slow.value_as_int() as u32, "crosses_above")}),
        Params::Breakout { bars } => json!({"kind": "signal", "when": {
            "kind": "compare", "left": {"kind": "price"}, "op": "crosses_above", "right": {"kind": "high", "bars": bars.value_as_int()}
        }}),
        Params::Other { entry } => entry.clone(),
    }
}

/// A captioned field of the sheet, as the entry sheet draws them.
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

/// The strategy editor: templates first for a new one, then its name, product, bars, when it
/// buys and sells, and its limits. Saving makes a new version; the page then shows its rule in
/// the engine's own sentences.
pub(super) fn editor(ui: &Rc<Ui>, detail: Option<Value>) {
    let title = if detail.is_some() { "Edit strategy" } else { "New strategy" };
    let Some(panel) = Panel::toggle(ui, title, 560) else { return };
    panel.add_css_class("money-sheet");
    panel.add_css_class("arbiter-sheet");
    let loading = label("Reading the exchange's products…", "money-muted");
    panel.body.append(&loading);
    panel.present();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let home = home_currency();
        let quoted = ui.call("arbiter.products", json!({"quote": home})).await;
        // A home currency nothing is quoted in (no CAD pairs on this account) shows them all.
        let read = match quoted {
            Ok(v) if !rows(&v, "products").is_empty() => Ok(v),
            Ok(_) => ui.call("arbiter.products", json!({})).await,
            Err(e) => Err(e),
        };
        panel.body.remove(&loading);
        match read {
            Ok(v) => form(&ui, &panel, rows(&v, "products"), detail),
            Err(e) => {
                let l = label(&said(&e), "money-problem");
                l.set_wrap(true);
                panel.body.append(&l);
            }
        }
    });
}

fn form(ui: &Rc<Ui>, panel: &Rc<Panel>, products: Vec<Value>, detail: Option<Value>) {
    let body = panel.body.clone();
    let row = detail.as_ref().map_or(Value::Null, |d| d["row"].clone());
    let rule = detail.as_ref().map_or(Value::Null, |d| d["rule"].clone());
    let id = row["id"].as_i64();
    let mut products: Vec<Value> = products.into_iter().filter(|p| p["tradable"] != false || p["id"] == row["product"]).collect();
    products.sort_by(|a, b| text(a, "id").cmp(text(b, "id")));
    if products.is_empty() {
        let l = label("The exchange listed nothing to trade. Try again in a moment.", "money-muted");
        l.set_wrap(true);
        body.append(&l);
        return;
    }
    let products = Rc::new(products);
    let home = home_currency();

    // Templates first, for a new strategy.
    let template_keys: Rc<RefCell<Vec<gtk::Button>>> = Rc::default();
    let templates = gtk::Grid::new();
    templates.set_row_spacing(8);
    templates.set_column_spacing(8);
    templates.set_column_homogeneous(true);
    if id.is_none() {
        for (i, (name, about, _)) in TEMPLATES.iter().enumerate() {
            let key = gtk::Button::new();
            key.add_css_class("threads-suggestion");
            key.add_css_class("arbiter-template");
            key.set_widget_name(&format!("arbiter-template-{i}"));
            let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
            words.append(&label(name, "threads-suggestion-ask"));
            let a = label(about, "threads-suggestion-about");
            a.set_wrap(true);
            words.append(&a);
            key.set_child(Some(&words));
            templates.attach(&key, (i % 2) as i32, (i / 2) as i32, 1, 1);
            template_keys.borrow_mut().push(key);
        }
        field(&body, "Start from", &templates);
    }

    let name = sheet_entry("arbiter-edit-name", text(&row, "name"), "Weekly ETH");
    field(&body, "Name", &name);

    // The product (searchable) beside the bars.
    let names: Vec<String> = products
        .iter()
        .map(|p| match opt_num(&p["price"]) {
            Some(x) => format!("{}   {}", text(p, "id"), price_text(x, text(p, "quote"))),
            None => text(p, "id").to_string(),
        })
        .collect();
    let list = gtk::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>());
    let product = gtk::DropDown::new(Some(list), Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
    product.set_enable_search(true);
    product.set_search_match_mode(gtk::StringFilterMatchMode::Substring);
    product.add_css_class("money-picker");
    product.set_widget_name("arbiter-edit-product");
    product.set_hexpand(true);
    let wanted = match text(&row, "product") {
        "" => format!("ETH-{home}"),
        p => p.to_string(),
    };
    let index = products.iter().position(|p| text(p, "id") == wanted).or_else(|| products.iter().position(|p| text(p, "id") == format!("BTC-{home}"))).unwrap_or(0);
    product.set_selected(index as u32);
    let bars = gtk::DropDown::from_strings(&GRANULARITIES.iter().map(|g| g.1).collect::<Vec<_>>());
    bars.add_css_class("money-picker");
    bars.set_widget_name("arbiter-edit-bars-width");
    let current = text(&row, "granularity");
    bars.set_selected(GRANULARITIES.iter().position(|g| g.0 == current).unwrap_or(4) as u32);
    let pair_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
    left.set_hexpand(true);
    field(&left, "Product", &product);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
    field(&right, "Bars", &bars);
    pair_row.append(&left);
    pair_row.append(&right);
    body.append(&pair_row);

    // When it buys.
    let sentence = row["sentences"][0].as_str().unwrap_or("").to_string();
    let (seed, exit_cross) = if rule.is_object() { seed_of(&rule, &sentence) } else { (template_seed(0), false) };
    let params_slot = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let params = Rc::new(RefCell::new(build_params(&params_slot, &seed, exit_cross)));
    field(&body, "When it buys", &params_slot);
    // An exit condition the editor did not build (an agent's) is kept unless a template replaces it.
    let kept_when = Rc::new(RefCell::new(if matches!(seed, Seed::Cross { .. }) { None } else { Some(rule["exit"]["when"].clone()).filter(|w| !w.is_null()) }));

    // How much each time, and how.
    let quote_of = {
        let products = products.clone();
        move |index: u32| products.get(index as usize).map(|p| text(p, "quote").to_string()).unwrap_or_default()
    };
    let amount = sheet_entry("arbiter-edit-amount", &shown_value(&rule["buy"]), "50");
    amount.set_hexpand(true);
    let amount_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    amount_row.append(&amount);
    let amount_suffix = label(&quote_of(product.selected()), "money-suffix");
    amount_suffix.set_valign(gtk::Align::Center);
    amount_row.append(&amount_suffix);
    let segments = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    segments.add_css_class("money-segments");
    segments.set_homogeneous(true);
    let market = gtk::ToggleButton::with_label("Market");
    market.set_tooltip_text(Some("Fills at once at the best price; pays the taker fee"));
    let limit = gtk::ToggleButton::with_label("Limit");
    limit.set_tooltip_text(Some("A post-only order at the best price; pays the lower maker fee, and may not fill"));
    limit.set_group(Some(&market));
    if text(&rule, "pricing") == "limit" {
        limit.set_active(true);
    } else {
        market.set_active(true);
    }
    segments.append(&market);
    segments.append(&limit);
    let pair_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
    left.set_hexpand(true);
    field(&left, "Amount per buy", &amount_row);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
    field(&right, "Order", &segments);
    pair_row.append(&left);
    pair_row.append(&right);
    body.append(&pair_row);

    // When it sells.
    let exit = &rule["exit"];
    let exits = gtk::Grid::new();
    exits.set_row_spacing(8);
    exits.set_column_spacing(12);
    exits.set_column_homogeneous(true);
    let exit_field = |col: i32, row_at: i32, caption: &str, name: &str, value: String, suffix: &str| {
        let b = gtk::Box::new(gtk::Orientation::Vertical, 4);
        b.append(&label(caption, "money-row-detail"));
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let e = sheet_entry(name, &value, "None");
        e.set_hexpand(true);
        e.set_width_chars(6);
        line.append(&e);
        line.append(&label(suffix, "money-suffix"));
        b.append(&line);
        exits.attach(&b, col, row_at, 1, 1);
        e
    };
    let take_profit = exit_field(0, 0, "Take profit at", "arbiter-edit-tp", shown_value(&exit["take_profit_pct"]), "%");
    let stop_loss = exit_field(1, 0, "Stop loss at", "arbiter-edit-sl", shown_value(&exit["stop_loss_pct"]), "%");
    let trailing = exit_field(0, 1, "Trailing stop", "arbiter-edit-trail", shown_value(&exit["trailing_pct"]), "%");
    let max_bars = exit_field(1, 1, "Sell after", "arbiter-edit-max-bars", shown_value(&exit["max_bars"]), "bars");
    field(&body, "When it sells", &exits);

    // Its limits.
    let limits_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    limits_box.add_css_class("arbiter-sheet-limits");
    let defaults = json!({"max_order": "100", "max_position": "500", "daily_loss": "50", "orders_per_hour": 6, "cooldown_minutes": 60});
    let limits_value = detail.as_ref().map_or(defaults, |d| d["limits"].clone());
    let limit_entries = Rc::new(limit_fields(&limits_box, &limits_value, &quote_of(product.selected()), "arbiter-edit-limit-"));
    field(&body, "Limits", &limits_box);

    {
        let (amount_suffix, limit_entries, quote_of) = (amount_suffix.clone(), limit_entries.clone(), quote_of.clone());
        product.connect_selected_notify(move |key| {
            let quote = quote_of(key.selected());
            amount_suffix.set_text(&quote);
            for f in limit_entries.iter().filter(|f| f.unit == "money") {
                f.suffix.set_text(&quote);
            }
        });
    }

    // A template fills everything it decides.
    for (i, key) in template_keys.borrow().iter().enumerate() {
        let (keys, params, slot, kept_when) = (template_keys.clone(), params.clone(), params_slot.clone(), kept_when.clone());
        let (name, amount, take_profit, stop_loss, trailing, max_bars, market) =
            (name.clone(), amount.clone(), take_profit.clone(), stop_loss.clone(), trailing.clone(), max_bars.clone(), market.clone());
        key.connect_clicked(move |_| {
            for (j, k) in keys.borrow().iter().enumerate() {
                if j == i {
                    k.add_css_class("selected");
                } else {
                    k.remove_css_class("selected");
                }
            }
            let (template, _, [tp, sl, trail]) = TEMPLATES[i];
            let typed = name.text();
            if typed.is_empty() || TEMPLATES.iter().any(|t| t.0 == typed.as_str()) {
                name.set_text(template);
            }
            *params.borrow_mut() = build_params(&slot, &template_seed(i), i == 2);
            *kept_when.borrow_mut() = None;
            if amount.text().is_empty() {
                amount.set_text("50");
            }
            take_profit.set_text(tp);
            stop_loss.set_text(sl);
            trailing.set_text(trail);
            max_bars.set_text("");
            market.set_active(true);
        });
    }

    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);
    if text(&row, "venue") == "live" && text(&row, "state") == "running" {
        let l = label("Saving changes what it trades with real money from its next bar.", "arbiter-warning");
        l.set_wrap(true);
        body.append(&l);
    }
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let cancel = button("Cancel", "quiet");
    let save = button(if id.is_some() { "Save a new version" } else { "Save on paper" }, "primary");
    save.add_css_class("money-hero-action");
    save.set_widget_name("arbiter-edit-save");
    save.set_hexpand(true);
    actions.append(&cancel);
    actions.append(&save);
    body.append(&actions);
    // Enter in a field saves, as the key does.
    for e in [&name, &amount, &take_profit, &stop_loss, &trailing, &max_bars] {
        enter_presses(e, &save);
    }
    for f in limit_entries.iter() {
        enter_presses(&f.entry, &save);
    }
    if id.is_none() {
        if let Some(first) = template_keys.borrow().first() {
            first.emit_clicked();
        }
    }

    // What the form says, field by field: the draft is unsaved while it differs from how the
    // sheet opened, and Escape, the scrim or × then leave it open.
    let snapshot = {
        let (name, product, bars, params, kept_when, amount, limit) = (name.clone(), product.clone(), bars.clone(), params.clone(), kept_when.clone(), amount.clone(), limit.clone());
        let (exits, limit_entries) = ([take_profit.clone(), stop_loss.clone(), trailing.clone(), max_bars.clone()], limit_entries.clone());
        move || {
            let crossing = matches!(&*params.borrow(), Params::Cross { exit, .. } if exit.is_active());
            json!([
                name.text().to_string(), product.selected(), bars.selected(), entry_json(&params.borrow()), crossing, kept_when.borrow().clone(),
                amount.text().to_string(), limit.is_active(),
                exits.iter().map(|e| e.text().to_string()).collect::<Vec<_>>(),
                limit_entries.iter().map(|f| f.entry.text().to_string()).collect::<Vec<_>>(),
            ])
        }
    };
    let opened = snapshot();
    let changed = Rc::new(move || snapshot() != opened);
    let permit = Rc::new(Cell::new(false));
    let (shown, allowed, unsaved) = (problem.downgrade(), permit.clone(), changed.clone());
    panel.set_guard(move || {
        if allowed.get() || !unsaved() {
            return true;
        }
        if let Some(problem) = shown.upgrade() {
            problem.set_text(if id.is_some() { "Save a new version, or Cancel to discard the changes." } else { "Save it on paper, or Cancel to discard it." });
            problem.set_visible(true);
        }
        false
    });
    let (sheet, allowed, unsaved) = (Rc::downgrade(panel), permit.clone(), changed.clone());
    confirm_inline_if(&cancel, if id.is_some() { "Discard changes" } else { "Discard draft" }, move || unsaved(), move |_| {
        allowed.set(true);
        if let Some(sheet) = sheet.upgrade() {
            sheet.close();
        }
    });

    let weak = Rc::downgrade(ui);
    let sheet = Rc::downgrade(panel);
    let first = name.clone();
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        // Saved unchanged, a strategy would only gain a version that changes nothing.
        if id.is_some() && !changed() {
            permit.set(true);
            if let Some(sheet) = sheet.upgrade() {
                sheet.close();
            }
            toast(&ui, "Nothing changed, so no new version.", None);
            return;
        }
        let refuse = |words: &str| {
            problem.set_text(words);
            problem.set_visible(true);
        };
        let title = name.text().trim().to_string();
        if title.is_empty() {
            refuse("Give the strategy a name.");
            return;
        }
        let Some(product) = products.get(product.selected() as usize).map(|p| text(p, "id").to_string()) else {
            refuse("Pick a product.");
            return;
        };
        let Some(buy) = decimal(&amount.text()) else {
            refuse("Type the amount to buy each time, like 50.");
            return;
        };
        let mut exit = serde_json::Map::new();
        for (key, caption, entry) in [("take_profit_pct", "Take profit", &take_profit), ("stop_loss_pct", "Stop loss", &stop_loss), ("trailing_pct", "Trailing stop", &trailing)] {
            let typed = entry.text();
            if typed.trim().is_empty() {
                continue;
            }
            let Some(value) = decimal(&typed) else {
                refuse(&format!("{caption}: type a percent above zero, like 4, or leave it empty."));
                return;
            };
            exit.insert(key.to_string(), Value::from(value));
        }
        let typed = max_bars.text();
        if !typed.trim().is_empty() {
            let Some(n) = typed.trim().parse::<u32>().ok().filter(|n| *n > 0) else {
                refuse("Sell after: a whole number of bars, or leave it empty.");
                return;
            };
            exit.insert(String::from("max_bars"), Value::from(n));
        }
        let entry = entry_json(&params.borrow());
        let when = match &*params.borrow() {
            Params::Cross { fast, slow, exit } if exit.is_active() => Some(cross(fast.value_as_int() as u32, slow.value_as_int() as u32, "crosses_below")),
            Params::Cross { .. } => None,
            _ => kept_when.borrow().clone(),
        };
        if let Some(when) = when {
            exit.insert(String::from("when"), when);
        }
        let limits = match read_limits(&limit_entries) {
            Ok(l) => l,
            Err(e) => {
                refuse(&e);
                return;
            }
        };
        let granularity = GRANULARITIES[bars.selected() as usize % GRANULARITIES.len()].0;
        let draft = json!({
            "name": title,
            "product": product,
            "granularity": granularity,
            "rule": {"entry": entry, "buy": buy, "pricing": if limit.is_active() { "limit" } else { "market" }, "exit": Value::Object(exit)},
            "limits": limits,
        });
        let mut payload = json!({"draft": draft});
        if let Some(id) = id {
            payload["id"] = json!(id);
        }
        problem.set_visible(false);
        key.set_sensitive(false);
        let (key, problem, sheet, permit) = (key.clone(), problem.clone(), sheet.clone(), permit.clone());
        glib::spawn_future_local(async move {
            match ui.call("arbiter.strategy.save", payload).await {
                Ok(d) => {
                    permit.set(true);
                    if let Some(sheet) = sheet.upgrade() {
                        sheet.close();
                    }
                    if let Some(id) = d["row"]["id"].as_i64() {
                        open_strategy(&ui, id);
                    }
                }
                Err(e) => {
                    problem.set_text(&said(&e));
                    problem.set_visible(true);
                    key.set_sensitive(true);
                }
            }
        });
    });
    first.grab_focus();
}

// ---- Connect Coinbase ----------------------------------------------------------------------

/// The Connect Coinbase sheet: how to make the key, the key, and what Coinbase said of it.
pub(super) fn connect(ui: &Rc<Ui>) {
    let Some(panel) = Panel::toggle(ui, "Connect Coinbase", 560) else { return };
    panel.add_css_class("money-sheet");
    panel.add_css_class("arbiter-sheet");
    let body = panel.body.clone();
    let about = label(
        "Arbiter signs each request with this key on this PC. The key stays in the system keyring; no server of Relay's exists to hold it.",
        "money-muted",
    );
    about.set_wrap(true);
    body.append(&about);
    let steps = gtk::Box::new(gtk::Orientation::Vertical, 10);
    steps.add_css_class("arbiter-steps");
    for (n, step) in [
        "Create an API key in the Coinbase Developer Platform (portal.cdp.coinbase.com), under API keys.",
        "Pick ECDSA as its signature algorithm. Allow View and Trade, and leave Transfer off: Arbiter refuses a key that can move money out.",
        "Choose a portfolio that holds only what Arbiter may use, and add this PC's IP address to the key's allowlist.",
    ]
    .iter()
    .enumerate()
    {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let number = label(&(n + 1).to_string(), "arbiter-step-number");
        number.set_valign(gtk::Align::Start);
        number.set_xalign(0.5);
        row.append(&number);
        let l = label(step, "arbiter-step");
        l.set_wrap(true);
        l.set_hexpand(true);
        row.append(&l);
        steps.append(&row);
    }
    body.append(&steps);
    let key_name = sheet_entry("arbiter-key-name", "", "organizations/…/apiKeys/…");
    key_name.add_css_class("arbiter-mono");
    field(&body, "Key name", &key_name);
    let secret = gtk::TextView::new();
    secret.set_monospace(true);
    secret.set_wrap_mode(gtk::WrapMode::Char);
    secret.set_accepts_tab(false);
    secret.add_css_class("arbiter-secret");
    secret.set_widget_name("arbiter-key-secret");
    secret.update_property(&[gtk::accessible::Property::Label("Private key")]);
    let secret_scroll = gtk::ScrolledWindow::builder().child(&secret).min_content_height(120).max_content_height(200).propagate_natural_height(true).build();
    secret_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    secret_scroll.add_css_class("arbiter-secret-frame");
    let secret_block = field(&body, "Private key", &secret_scroll);
    secret_block.append(&label("The whole PEM, from -----BEGIN EC PRIVATE KEY----- to its END line.", "money-row-detail"));
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);
    let results = gtk::Box::new(gtk::Orientation::Vertical, 6);
    results.add_css_class("arbiter-results");
    results.set_visible(false);
    body.append(&results);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let cancel = button("Cancel", "quiet");
    let save = button("Check and save to keyring", "primary");
    save.add_css_class("money-hero-action");
    save.set_widget_name("arbiter-key-save");
    save.set_hexpand(true);
    actions.append(&cancel);
    actions.append(&save);
    body.append(&actions);
    // Enter in the name goes on to the key itself, which takes Enter as a new line.
    let next = secret.downgrade();
    key_name.connect_activate(move |_| {
        if let Some(secret) = next.upgrade() {
            secret.grab_focus();
        }
    });
    // What was pasted and not yet checked is not thrown away by Escape, the scrim or ×.
    let saved = Rc::new(Cell::new(false));
    let unsaved = {
        let (name, buffer, saved) = (key_name.downgrade(), secret.buffer(), saved.clone());
        Rc::new(move || !saved.get() && (name.upgrade().is_some_and(|n| !n.text().trim().is_empty()) || buffer.char_count() > 0))
    };
    let permit = Rc::new(Cell::new(false));
    let (shown, allowed, pending) = (problem.downgrade(), permit.clone(), unsaved.clone());
    panel.set_guard(move || {
        if allowed.get() || !pending() {
            return true;
        }
        if let Some(problem) = shown.upgrade() {
            problem.set_text("Check and save the key, or Cancel to discard it.");
            problem.set_visible(true);
        }
        false
    });
    let sheet = Rc::downgrade(&panel);
    confirm_inline_if(&cancel, "Discard draft", move || unsaved(), move |_| {
        permit.set(true);
        if let Some(sheet) = sheet.upgrade() {
            sheet.close();
        }
    });
    let weak = Rc::downgrade(ui);
    let first = key_name.clone();
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let name = key_name.text().trim().to_string();
        let buffer = secret.buffer();
        let pem = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
        if name.is_empty() || pem.is_empty() {
            problem.set_text("Paste both the key's name and its private key.");
            problem.set_visible(true);
            return;
        }
        problem.set_visible(false);
        key.set_sensitive(false);
        key.set_label("Checking with Coinbase…");
        let (key, problem, results, cancel, secret, key_name, saved) = (key.clone(), problem.clone(), results.clone(), cancel.clone(), secret.clone(), key_name.clone(), saved.clone());
        glib::spawn_future_local(async move {
            match ui.call("arbiter.key.set", json!({"key_name": name, "private_key": pem})).await {
                Ok(c) => {
                    saved.set(true);
                    // The key is in the keyring now; it need not stay in a text field.
                    secret.buffer().set_text("");
                    secret.set_sensitive(false);
                    key_name.set_sensitive(false);
                    show_checked(&results, &c);
                    key.set_visible(false);
                    cancel.set_label("Done");
                    cancel.add_css_class("primary");
                    ui.refresh_page();
                }
                Err(e) => {
                    problem.set_text(&said(&e));
                    problem.set_visible(true);
                    key.set_sensitive(true);
                    key.set_label("Check and save to keyring");
                }
            }
        });
    });
    panel.present();
    first.grab_focus();
}

/// "Checked with Coinbase": the portfolio, what the key may do, the fee tier, and the allowlist.
fn show_checked(results: &gtk::Box, c: &Value) {
    clear(results);
    results.append(&label("CHECKED WITH COINBASE", "money-label"));
    let p = &c["permissions"];
    let line = |tone: &str, words: &str| {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let dot = lamp(tone);
        dot.set_valign(gtk::Align::Start);
        dot.set_margin_top(6);
        row.append(&dot);
        let l = label(words, "arbiter-result");
        l.set_wrap(true);
        l.set_hexpand(true);
        row.append(&l);
        results.append(&row);
    };
    let scoped = p["portfolio_uuid"].as_str().is_some_and(|u| !u.is_empty());
    line(if scoped { "arbiter-live" } else { "arbiter-paper" }, &portfolio_words(p));
    match (p["can_view"] == true, p["can_trade"] == true) {
        (_, true) => line("arbiter-live", "Can view and trade"),
        (true, false) => line("arbiter-paper", "Can view, cannot trade: strategies can read the account but not go live"),
        _ => line("arbiter-halted", "Cannot view the account"),
    }
    if p["can_transfer"] == true {
        line("arbiter-halted", "Can transfer money out");
    } else {
        line("arbiter-live", "Cannot transfer money out");
    }
    line("arbiter-live", &fee_words(&c["fees"]));
    line("arbiter-paper", "Coinbase does not say whether the key is limited to this PC's IP address. If you skipped that step, add it to the key in the Developer Platform.");
    if text(c, "state") == "error" {
        line("arbiter-halted", c["error"].as_str().unwrap_or("The check failed"));
    }
    results.set_visible(true);
}

// ---- The thread panel ----------------------------------------------------------------------

/// What a panel tab reads.
pub(super) async fn panel_read(ui: &Rc<Ui>, tab: &str) -> Result<Value, Error> {
    if tab == "arbiter-orders" {
        ui.call("arbiter.order.list", json!({"limit": 40})).await
    } else {
        ui.call("arbiter.summary", json!({})).await
    }
}

pub(super) fn panel_draw(ui: &Rc<Ui>, body: &gtk::Box, tab: &str, v: &Value) {
    match tab {
        "arbiter-orders" => panel_orders(body, v),
        "arbiter-strategies" => {
            remember(v);
            panel_strategies(ui, body, v);
        }
        _ => {
            remember(v);
            panel_overview(ui, body, v);
        }
    }
}

/// The panel's Overview: the money, today's limits, the strategies with their lamps, what
/// waits, and prices; Open Arbiter and the kill switch at its foot.
fn panel_overview(ui: &Rc<Ui>, body: &gtk::Box, s: &Value) {
    let home = home_currency();
    panel_halted(ui, body, s);
    let live = s["live"].is_object();
    let account = if live { &s["live"] } else { &s["paper"] };
    let head = gtk::Box::new(gtk::Orientation::Vertical, 6);
    head.add_css_class("threads-summary");
    let when = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let c = &s["connection"];
    when.append(&lamp(match text(c, "state") {
        "ok" => "arbiter-live",
        "error" => "arbiter-halted",
        _ => "",
    }));
    let eyebrow = match text(c, "state") {
        "ok" => format!("{} · Coinbase", if live { "Live" } else { "Paper" }),
        "error" => String::from("Coinbase check failed"),
        _ => String::from("Paper · no Coinbase key"),
    };
    when.append(&label(&eyebrow, "threads-panel-eyebrow"));
    head.append(&when);
    head.append(&label(&money(num(&account["value"]), &home), "threads-panel-figure"));
    let today = num(&account["pnl_today"]);
    let sub = label(&format!("{} today, after fees", money_signed(today, &home)), "money-muted");
    add_tone(&sub, today);
    head.append(&sub);
    if let Some(line) = sparkline(&account["equity"], 36) {
        head.append(&line);
    }
    body.append(&head);

    let limits = rows(s, "limits");
    if !limits.is_empty() {
        let b = panel_block(body, "Limits today", "");
        for l in &limits {
            let m = limit_meter(l, &home);
            m.remove_css_class("arbiter-limit");
            m.add_css_class("threads-budget");
            b.append(&m);
        }
    }

    let strategies = rows(s, "strategies");
    let b = panel_block(body, "Strategies", "");
    if strategies.is_empty() {
        let l = label("No strategies yet. Ask for one here, or start from a template.", "money-muted");
        l.set_wrap(true);
        b.append(&l);
    }
    for r in &strategies {
        b.append(&panel_strategy_row(ui, r));
    }

    let pending = rows(s, "pending");
    if !pending.is_empty() {
        let b = panel_block(body, "Waiting for you", &pending.len().to_string());
        for p in &pending {
            b.append(&proposal_card(p));
        }
    }

    let prices = rows(s, "prices");
    if !prices.is_empty() {
        let b = panel_block(body, "Prices", "");
        for p in &prices {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("threads-entry");
            let product = text(p, "product");
            let name = label(product, "threads-panel-name");
            name.set_hexpand(true);
            row.append(&name);
            row.append(&label(&price_text(num(&p["price"]), pair(product).1), "threads-panel-figures"));
            if let Some(change) = opt_num(&p["change_24h"]) {
                let c = label(&pct(change), "threads-panel-figures");
                c.set_width_chars(7);
                c.set_xalign(1.0);
                add_tone(&c, change);
                row.append(&c);
            }
            b.append(&row);
        }
    }

    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    foot.set_margin_top(4);
    let open = button("Open Arbiter", "");
    let weak = Rc::downgrade(ui);
    open.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.navigate("arbiter-home");
        }
    });
    foot.append(&open);
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    foot.append(&gap);
    foot.append(&halt_key(ui, s));
    body.append(&foot);
}

/// A strategy in the panel: its lamp, name over what it is doing, its profit. Opens its page.
fn panel_strategy_row(ui: &Rc<Ui>, r: &Value) -> gtk::Button {
    let id = r["id"].as_i64().unwrap_or(0);
    let key = button("", "arbiter-panel-row");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.append(&lamp(row_tone(r)));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    let name = label(text(r, "name"), "threads-panel-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let doing = match text(r, "state") {
        "halted" => format!("Halted: {}", r["halt_reason"].as_str().unwrap_or("a limit was reached")),
        "stopped" => String::from("Stopped"),
        _ => text(r, "doing").to_string(),
    };
    let d = label(&doing, "threads-panel-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    row.append(&words);
    let pnl = num(&r["pnl_realized"]) + num(&r["pnl_open"]);
    let f = label(&money_signed(pnl, text(r, "currency")), "threads-panel-figures");
    add_tone(&f, pnl);
    f.set_valign(gtk::Align::Center);
    row.append(&f);
    key.set_child(Some(&row));
    key.set_tooltip_text(Some(&format!("{} · {}", if text(r, "venue") == "live" { "Live" } else { "Paper" }, mode_name(text(r, "mode")))));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_strategy(&ui, id);
        }
    });
    key
}

/// The panel's bar for the kill switch, with Restart, above either summary tab.
fn panel_halted(ui: &Rc<Ui>, body: &gtk::Box, s: &Value) {
    if s["halted"] == true {
        banner(body, &format!("Halted: {}", s["halt_reason"].as_str().unwrap_or("the kill switch is on")), Some(("Restart", restart_action(ui, None))));
    }
}

/// The panel's Strategies: each with its pill, its rule in sentences and its record.
fn panel_strategies(ui: &Rc<Ui>, body: &gtk::Box, s: &Value) {
    panel_halted(ui, body, s);
    let strategies = rows(s, "strategies");
    if strategies.is_empty() {
        let l = label("No strategies yet. Ask for one in this thread, or start from a template.", "money-muted");
        l.set_wrap(true);
        body.append(&l);
        let add = button("New strategy", "");
        add.set_halign(gtk::Align::Start);
        let weak = Rc::downgrade(ui);
        add.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                editor(&ui, None);
            }
        });
        body.append(&add);
        return;
    }
    for r in &strategies {
        let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
        b.add_css_class("threads-stat");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        top.append(&lamp(row_tone(r)));
        let name = label(text(r, "name"), "threads-panel-name");
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&name);
        top.append(&venue_pill(r));
        b.append(&top);
        for s in r["sentences"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let l = label(s, "threads-panel-detail");
            l.set_wrap(true);
            b.append(&l);
        }
        let currency = text(r, "currency");
        let pnl = num(&r["pnl_realized"]) + num(&r["pnl_open"]);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let f = label(&format!("{} · {}", money_signed(pnl, currency), plural(r["trades"].as_u64().unwrap_or(0) as usize, "trade")), "threads-panel-figures");
        add_tone(&f, pnl);
        f.set_hexpand(true);
        line.append(&f);
        let open = button("Open", "money-text-action");
        let (weak, id) = (Rc::downgrade(ui), r["id"].as_i64().unwrap_or(0));
        open.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                open_strategy(&ui, id);
            }
        });
        line.append(&open);
        b.append(&line);
        body.append(&b);
    }
}

fn panel_orders(body: &gtk::Box, v: &Value) {
    let orders = rows(v, "orders");
    if orders.is_empty() {
        body.append(&label("No orders yet.", "money-muted"));
        return;
    }
    for o in &orders {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("threads-entry");
        row.append(&tile(side_icon(text(o, "side"))));
        let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
        words.set_hexpand(true);
        let title = label(&order_title(o), "threads-panel-name");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&title);
        let detail = [o["strategy"].as_str().unwrap_or("By hand").to_string(), String::from(if text(o, "venue") == "live" { "Live" } else { "Paper" }), ago(text(o, "created_at"))];
        let d = label(&detail.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · "), "threads-panel-detail");
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&d);
        row.append(&words);
        row.append(&status_pill(o));
        if let Some(why) = o["why"].as_str().filter(|w| !w.is_empty()) {
            row.set_tooltip_text(Some(why));
        }
        body.append(&row);
    }
}

// ---- Cards in the conversation -------------------------------------------------------------

/// What an Arbiter tool call did, in words, for the quiet line under a reply.
pub(super) fn tool_caption(op: &str) -> Option<&'static str> {
    Some(match op {
        "arbiter.summary" => "Read Arbiter",
        "arbiter.strategy.get" => "Read a strategy",
        "arbiter.order.list" => "Read orders",
        "arbiter.decision.list" => "Read the decision log",
        "arbiter.products" => "Looked up products",
        "arbiter.series" => "Read prices",
        "arbiter.backtest" => "Ran a backtest",
        "arbiter.proposal.list" => "Read proposals",
        "arbiter.settings.get" => "Read Arbiter's settings",
        "arbiter.propose" => "Proposed a change",
        "arbiter.order.place" => "Asked for an order",
        "arbiter.halt" => "Halted Arbiter",
        _ => return None,
    })
}

/// Whether an Arbiter tool call asked to change something.
pub(super) fn tool_writes(op: &str) -> bool {
    matches!(op, "arbiter.propose" | "arbiter.order.place" | "arbiter.halt")
}

/// The card a tool's answer becomes: a proposal waiting for the person, an order placed, or a
/// refusal. `None` leaves the quiet line as it is.
pub(super) fn tool_card(op: &str, v: &Value) -> Option<gtk::Widget> {
    match op {
        "arbiter.propose" if v["id"].is_i64() => Some(proposal_card(v).upcast()),
        "arbiter.order.place" => match text(v, "outcome") {
            "proposed" if v["proposal"].is_object() => Some(proposal_card(&v["proposal"]).upcast()),
            "placed" if v["order"].is_object() => Some(order_line(&v["order"]).upcast()),
            "refused" => Some(refusal_line(v["refusal"]["message"].as_str().unwrap_or("The limits refused it.")).upcast()),
            _ => None,
        },
        _ if v["proposal"].is_object() => Some(proposal_card(&v["proposal"]).upcast()),
        _ => None,
    }
}

/// A refused tool call's text, as Relay's MCP server words it (`{"error": "Rejected
/// arbiter.over_max_order: That order…"}`), down to the engine's sentence.
pub(super) fn refusal_words(raw: &str) -> String {
    let said = serde_json::from_str::<Value>(raw).ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or_else(|| raw.trim().to_string());
    match said.split_once(": ") {
        Some((head, message)) if head.contains('.') && head.split(' ').count() <= 2 => message.to_string(),
        _ => said,
    }
}

/// An agent's tool call the engine refused, said in the engine's words.
pub(super) fn refusal_line(message: &str) -> gtk::Label {
    let l = label(&format!("Refused: {message}"), "threads-error");
    l.set_wrap(true);
    l.set_max_width_chars(68);
    l
}

/// An order an agent placed: "Bought $50.00 of ETH on paper", with its status.
fn order_line(o: &Value) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("threads-card");
    card.append(&tile(side_icon(text(o, "side"))));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    let buy = text(o, "side") == "buy";
    let verb = match text(o, "status") {
        "filled" => if buy { "Bought" } else { "Sold" },
        "failed" | "cancelled" | "expired" => if buy { "Tried to buy" } else { "Tried to sell" },
        _ => if buy { "Buying" } else { "Selling" },
    };
    let place = if text(o, "venue") == "live" { "on Coinbase" } else { "on paper" };
    let title = label(&format!("{verb} {} {place}", order_amount(o)), "threads-card-title");
    title.set_wrap(true);
    words.append(&title);
    let mut detail = vec![o["strategy"].as_str().unwrap_or("By hand").to_string()];
    if let Some(p) = opt_num(&o["average_price"]) {
        detail.push(format!("at {}", price_text(p, pair(text(o, "product")).1)));
    }
    if let Some(e) = o["error"].as_str() {
        detail.push(e.to_string());
    }
    let d = label(&detail.join(" · "), "threads-card-detail");
    d.set_wrap(true);
    words.append(&d);
    card.append(&words);
    card.append(&status_pill(o));
    card
}

/// A proposal's status as a pill: amber while it waits, then what became of it.
fn proposal_pill(status: &str) -> gtk::Label {
    match status {
        "pending" => pill("Needs your approval", "arbiter-paper"),
        "approved" => pill("Approved", "arbiter-live"),
        "failed" => pill("Failed", "arbiter-halted"),
        "expired" => pill("Expired", ""),
        _ => pill("Dismissed", ""),
    }
}

/// The strategy a proposal is about, as the last summary read it.
fn strategy_row_of(p: &Value) -> Option<Value> {
    let id = p["strategy_id"].as_i64()?;
    cached().and_then(|s| rows(&s, "strategies").into_iter().find(|r| r["id"].as_i64() == Some(id)))
}

/// "Buy the dip · version 4 → 5 · stays on paper". `pending` says the version it would make;
/// once answered the strategy has moved on, so only its name.
fn proposal_detail(p: &Value, pending: bool) -> String {
    let strategy = p["strategy"].as_str().unwrap_or("A strategy");
    match text(p, "kind") {
        "order" => {
            let venue = if p["venue"] == "live" { "live, real money" } else { "on paper" };
            format!("{strategy} · {venue} · {}", if p["limit"] == true { "limit order" } else { "market order" })
        }
        "new_strategy" => format!("New strategy · {} · starts stopped, on paper", p["draft"]["product"].as_str().unwrap_or("")),
        _ => match strategy_row_of(p).filter(|_| pending) {
            Some(r) => {
                let v = r["version"].as_i64().unwrap_or(1);
                let venue = if text(&r, "venue") == "live" { "stays live" } else { "stays on paper" };
                format!("{strategy} · version {v} → {} · {venue}", v + 1)
            }
            None => strategy.to_string(),
        },
    }
}

/// The approve key's caption: an order says it is one.
fn approve_caption(p: &Value) -> &'static str {
    if text(p, "kind") == "order" { "Approve order" } else { "Approve" }
}

/// "expires in 8 min", while a card still waits for its answer.
fn expires_words(p: &Value) -> Option<String> {
    let left = p["expires_at"].as_str().and_then(minutes_until).filter(|m| *m >= 0)?;
    Some(if left < 1 { String::from("expires in under a minute") } else { format!("expires in {left} min") })
}

/// What approving does, under the keys.
fn proposal_footnote(p: &Value) -> String {
    match text(p, "kind") {
        "order" => String::from("Approving passes it through the limits again, then places it."),
        "new_strategy" => String::from("Approving saves it exactly as shown. It starts stopped, on paper."),
        _ => match strategy_row_of(p) {
            Some(r) => format!("Approving runs version {} exactly as shown.", r["version"].as_i64().unwrap_or(1) + 1),
            None => String::from("Approving runs the new version exactly as shown."),
        },
    }
}

/// A proposal as a card in the conversation: what it is, why, its changes, and Approve and
/// Dismiss while it waits. It redraws itself when it is answered anywhere.
pub(super) fn proposal_card(p: &Value) -> gtk::Box {
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    fill_card(&slot, p);
    if let Some(id) = p["id"].as_i64() {
        CARDS.with(|c| {
            let mut c = c.borrow_mut();
            c.retain(|(_, w)| w.upgrade().is_some());
            c.push((id, slot.downgrade()));
        });
        // A card drawn from an old message shows the proposal as it is now, not as it was.
        if text(p, "status") == "pending" {
            refresh_cards();
        }
    }
    slot
}

fn fill_card(slot: &gtk::Box, p: &Value) {
    clear(slot);
    let status = text(p, "status");
    let pending = status == "pending";
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("threads-card");
    card.add_css_class("arbiter-proposal");
    if pending {
        card.add_css_class("arbiter-asking");
    }
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = label(text(p, "title"), "threads-card-title");
    title.set_wrap(true);
    title.set_hexpand(true);
    top.append(&title);
    let status_pill = proposal_pill(status);
    status_pill.set_valign(gtk::Align::Start);
    top.append(&status_pill);
    card.append(&top);
    let detail = label(&proposal_detail(p, pending), "threads-card-detail");
    detail.set_wrap(true);
    card.append(&detail);
    if let Some(why) = p["why"].as_str().filter(|w| !w.is_empty()) {
        let l = label(why, "arbiter-why");
        l.set_wrap(true);
        card.append(&l);
    }
    let changes: Vec<&str> = p["changes"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !changes.is_empty() {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.add_css_class("arbiter-changes");
        for c in changes {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let marker = label("•", "threads-marker");
            marker.set_valign(gtk::Align::Start);
            row.append(&marker);
            let l = label(c, "arbiter-change");
            l.set_wrap(true);
            l.set_hexpand(true);
            row.append(&l);
            list.append(&row);
        }
        card.append(&list);
    }
    if pending {
        let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let approve = button(approve_caption(p), "primary");
        approve.set_widget_name(&format!("arbiter-approve-{}", p["id"]));
        let dismiss = button("Dismiss", "quiet");
        keys.append(&approve);
        keys.append(&dismiss);
        if let Some(words) = expires_words(p) {
            let l = label(&words, "threads-card-detail");
            l.set_valign(gtk::Align::Center);
            keys.append(&l);
        }
        card.append(&keys);
        let foot = label(&proposal_footnote(p), "threads-card-detail");
        foot.set_wrap(true);
        card.append(&foot);
        let problem = label("", "threads-error");
        problem.set_wrap(true);
        problem.set_visible(false);
        card.append(&problem);
        for (key, yes) in [(approve, true), (dismiss, false)] {
            let (slot, p, keys, problem) = (slot.downgrade(), p.clone(), keys.clone(), problem.clone());
            key.connect_clicked(move |_| {
                let Some(ui) = super::threads::the_ui_pub() else { return };
                keys.set_sensitive(false);
                let (slot, keys, problem) = (slot.clone(), keys.clone(), problem.clone());
                resolve(&ui, &p, yes, Box::new(move |result| match result {
                    Ok(view) => {
                        if let Some(slot) = slot.upgrade() {
                            fill_card(&slot, &view);
                        }
                    }
                    Err(e) => {
                        problem.set_text(&e);
                        problem.set_visible(true);
                        keys.set_sensitive(true);
                    }
                }));
            });
        }
    } else {
        let outcome = p["outcome"].as_str().filter(|o| !o.is_empty());
        let (words, class) = match status {
            "approved" => (outcome.map_or_else(|| String::from("Approved."), |o| format!("Approved. {o}")), "threads-card-detail"),
            "failed" => (format!("Could not be done: {}", outcome.unwrap_or("no reason given")), "threads-error"),
            "expired" => (String::from("Expired before an answer."), "threads-card-detail"),
            _ => (String::from("Dismissed."), "threads-card-detail"),
        };
        let l = label(&words, class);
        l.set_wrap(true);
        card.append(&l);
    }
    slot.append(&card);
}

/// Redraw every proposal card on screen from one `arbiter.proposal.list` read; reads asked for
/// in the same moment share it.
pub(super) fn refresh_cards() {
    if CARDS.with(|c| c.borrow().iter().all(|(_, w)| w.upgrade().is_none())) {
        return;
    }
    if STATE.with(|s| s.cards_pending.replace(true)) {
        return;
    }
    glib::idle_add_local_once(|| {
        let Some(ui) = super::threads::the_ui_pub() else {
            STATE.with(|s| s.cards_pending.set(false));
            return;
        };
        glib::spawn_future_local(async move {
            let read = ui.call("arbiter.proposal.list", json!({"limit": 200})).await;
            STATE.with(|s| s.cards_pending.set(false));
            let Ok(read) = read else { return };
            let by_id: HashMap<i64, Value> = rows(&read, "proposals").into_iter().filter_map(|p| Some((p["id"].as_i64()?, p))).collect();
            let cards: Vec<(i64, gtk::Box)> = CARDS.with(|c| {
                let mut c = c.borrow_mut();
                c.retain(|(_, w)| w.upgrade().is_some());
                c.iter().filter_map(|(id, w)| Some((*id, w.upgrade()?))).collect()
            });
            for (id, slot) in cards {
                if let Some(p) = by_id.get(&id) {
                    fill_card(&slot, p);
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_amounts_go_back_as_written() {
        assert_eq!(decimal("50").as_deref(), Some("50"));
        assert_eq!(decimal(" $50.00 ").as_deref(), Some("50.00"));
        assert_eq!(decimal("4,5").as_deref(), Some("4.5"));
        assert_eq!(decimal("4%").as_deref(), Some("4"));
        assert_eq!(decimal("0"), None);
        assert_eq!(decimal("-3"), None);
        assert_eq!(decimal("lots"), None);
    }

    #[test]
    fn amounts_and_percents_read_signed() {
        assert!(money_signed(1.2, "CAD").starts_with('+'));
        assert!(money_signed(-1.2, "CAD").starts_with('−'));
        assert!(!money_signed(0.001, "CAD").starts_with('+'));
        assert_eq!(money(0.0123, "BTC"), "0.0123 BTC");
        assert_eq!(pct(4.24), "+4.2%");
        assert_eq!(pct(-3.0), "−3.0%");
        assert_eq!(pct(0.01), "0.0%");
        assert_eq!(drawdown(4.24), "−4.2%");
        assert_eq!(drawdown(0.04), "0.0%");
        assert_eq!(pair("ETH-CAD"), ("ETH", "CAD"));
    }

    #[test]
    fn a_refusal_reads_as_the_engine_said_it() {
        let raw = "{\n  \"error\": \"Rejected arbiter.over_max_order: That order is over the most per order.\"\n}";
        assert_eq!(refusal_words(raw), "That order is over the most per order.");
        assert_eq!(refusal_words("Something odd: happened here"), "Something odd: happened here");
    }

    #[test]
    fn the_editor_reads_back_what_its_templates_write() {
        let rsi = json!({"entry": {"kind": "signal", "when": {"kind": "compare", "left": {"kind": "rsi", "period": 14}, "op": "below", "right": {"kind": "number", "value": 30}}}, "buy": "50"});
        assert!(matches!(seed_of(&rsi, "").0, Seed::Rsi { period: 14, below } if below == 30.0));
        let trend = json!({"entry": {"kind": "signal", "when": cross(12, 26, "crosses_above")}, "exit": {"when": cross(12, 26, "crosses_below")}});
        assert!(matches!(seed_of(&trend, ""), (Seed::Cross { fast: 12, slow: 26 }, true)));
        let weekly = json!({"entry": {"kind": "schedule", "every": "week", "hour": 9, "weekday": 1}});
        assert!(matches!(seed_of(&weekly, "").0, Seed::Schedule { weekday: 1, hour: 9, .. }));
        let breakout = json!({"entry": {"kind": "signal", "when": {"kind": "compare", "left": {"kind": "price"}, "op": "crosses_above", "right": {"kind": "high", "bars": 20}}}});
        assert!(matches!(seed_of(&breakout, "").0, Seed::Breakout { bars: 20 }));
        // An agent's nested rule is kept, not rewritten into a template.
        let nested = json!({"entry": {"kind": "signal", "when": {"kind": "all", "of": []}}});
        assert!(matches!(seed_of(&nested, "Buys when …").0, Seed::Other { .. }));
    }
}
