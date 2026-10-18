//! Avex in the Threads space, beside Tally and Arbiter (docs/GYM.md): its pages (Overview,
//! Workouts, Lifts, and one workout or one lift opened from a row), and what the thread page
//! borrows from them, the Avex panel.
//!
//! Avex has no internet, so what shows is the training history the person last exported from it
//! and imported here. Everything is read through the engine's `gym.*` ops and never changed here:
//! the only writes are importing a newer export and forgetting the copy. Weights and volumes
//! arrive in the person's unit, named by each reading's `unit`.
//!
//! A copy exported before this week would read as a week without training. So the Overview, the
//! lifts and the panel read it as of the day it was exported (the ops' `today`), and say how old
//! it is.
use super::pages::{card, card_asking, cell, columns, date, glyph_image, human_date, icon_row, meter_in, plural, row_key, strip, today, word, Tone};
use super::{message, page, toast, Action, Page};
use crate::app::{button, clear, confirm_inline, label, rows, text, Ui};
use crate::client::Error;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Avex's tabbed pages: (stack name, caption).
pub const PAGES: [(&str, &str); 3] = [("avex-home", "Overview"), ("avex-workouts", "Workouts"), ("avex-lifts", "Lifts")];
/// One workout and one lift, opened from a row; they carry no tab of their own.
pub const WORKOUT: &str = "avex-workout";
pub const LIFT: &str = "avex-lift";
/// Every Avex page, for `money::add_pages`.
pub const ALL: [&str; 5] = ["avex-home", "avex-workouts", "avex-lifts", WORKOUT, LIFT];

/// The thread panel's Avex view: one scroll, so no switch shows above it.
pub const PANEL_TABS: [(&str, &str); 1] = [("avex-overview", "Overview")];

/// How many workouts the Workouts page reads at a time.
const PAGE_SIZE: u32 = 60;

/// What "est. max" is, wherever it shows.
const EST_MAX: &str = "Est. max is an estimated one-rep max: weight × (1 + reps ÷ 30), from sets of 1 to 12 reps";

/// Days without a lift, with no change to read, before it counts as slipping.
const DROPPED_DAYS: i64 = 42;

#[derive(Default)]
struct State {
    /// The workout and the lift their pages show.
    workout: Cell<Option<i64>>,
    lift: RefCell<Option<String>>,
    /// How many workouts the Workouts page shows; Show more reads further.
    shown: Cell<u32>,
    /// The workouts either side of the one showing, older and newer, for Alt+← and Alt+→.
    neighbours: Cell<(Option<i64>, Option<i64>)>,
}

thread_local! {
    static STATE: State = State::default();
}

// ---- Reading the engine's numbers ----------------------------------------------------------

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// `102.5`, `100`: a weight or a count without a trailing `.0`.
fn trimmed(x: f64) -> String {
    if (x - x.round()).abs() < 0.05 {
        group(x.round() as i64)
    } else {
        format!("{x:.1}")
    }
}

/// `12,500`.
pub(super) fn group(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("−{out}")
    } else {
        out
    }
}

fn volume(v: &Value, unit: &str) -> String {
    format!("{} {unit}", group(num(v).unwrap_or(0.0).round() as i64))
}

/// `225 × 5`, `× 12` for a bodyweight set.
fn set_words(weight: &Value, reps: &Value, unit: &str) -> String {
    let reps = reps.as_i64().unwrap_or(0);
    match num(weight) {
        Some(w) => format!("{} {unit} × {reps}", trimmed(w)),
        None => format!("× {reps}"),
    }
}

fn minutes(m: i64) -> String {
    if m >= 60 {
        format!("{} h {:02} min", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

/// `+4.9%`, `−3.1%`.
fn pct(c: f64) -> String {
    if c < 0.0 {
        format!("−{:.1}%", -c)
    } else {
        format!("+{c:.1}%")
    }
}

/// `+5%`, `−12%`.
fn whole_pct(c: f64) -> String {
    let r = c.round() as i64;
    if r < 0 {
        format!("−{}%", -r)
    } else {
        format!("+{r}%")
    }
}

/// `+10 lb`, `−2.5 kg`.
fn signed_weight(d: f64, unit: &str) -> String {
    if d < 0.0 {
        format!("−{} {unit}", trimmed(-d))
    } else {
        format!("+{} {unit}", trimmed(d))
    }
}

/// What a failed call says on the page: the engine's own sentence, never its code. An engine
/// without Avex says so plainly.
pub(super) fn said(e: &Error) -> String {
    match e {
        Error::Bus(b) if matches!(b.code.as_str(), "bus.unknown_op" | "bus.not_implemented") => {
            String::from("This engine does not read Avex yet. Update Relay's engine to see your training here.")
        }
        Error::Bus(b) => b.message.clone(),
        e => e.to_string(),
    }
}

// ---- How old the copy is -------------------------------------------------------------------

/// The copy's age, from `gym.summary`'s `imported.exported_at`, the phone's own date.
#[derive(Clone)]
struct Age {
    /// The export's day, `YYYY-MM-DD`.
    day: String,
    /// Whole days from then to today.
    days: i64,
    /// From before this week: what shows was read as of `day`.
    stale: bool,
}

/// The age of a copy, from a summary read as of today; `None` before anything was imported.
fn age_of(s: &Value) -> Option<Age> {
    let day = s["imported"]["exported_at"].as_str()?.get(..10)?.to_string();
    let days = days_between(&day, &today())?.max(0);
    let stale = day.as_str() < text(&s["this_week"], "start");
    Some(Age { day, days, stale })
}

/// The `today` a reading takes: the export's day when the copy is stale, else none.
fn as_of(age: Option<&Age>) -> Value {
    age.filter(|a| a.stale).map_or(Value::Null, |a| json!(a.day))
}

/// The day the readings count back from: the export's when stale, else today.
fn as_of_day(age: Option<&Age>) -> String {
    age.filter(|a| a.stale).map_or_else(today, |a| a.day.clone())
}

/// Calendar days from `from` to `to`, both `YYYY-MM-DD`.
fn days_between(from: &str, to: &str) -> Option<i64> {
    let (a, b) = (date(from)?, date(to)?);
    // Midnight to midnight is 23 or 25 hours across a clock change.
    Some((b.difference(&a).0 as f64 / 86_400_000_000.0).round() as i64)
}

/// `gym.summary`, as of the export's day when the copy is from before this week, and its age.
async fn summary(ui: &Rc<Ui>) -> Result<(Value, Option<Age>), Error> {
    let now = ui.call("gym.summary", json!({})).await?;
    let age = age_of(&now);
    match age.as_ref().filter(|a| a.stale) {
        Some(a) => Ok((ui.call("gym.summary", json!({"today": a.day})).await?, age)),
        None => Ok((now, age)),
    }
}

/// "Thu 8 Oct", never "Today": for a week's name and for words that go into a question.
fn day_words(day: &str) -> String {
    let Some(d) = date(day) else { return day.to_string() };
    let year = glib::DateTime::now_local().map(|n| n.year()).unwrap_or(d.year());
    d.format(if d.year() == year { "%a %-d %b" } else { "%a %-d %b %Y" }).map(|s| s.to_string()).unwrap_or_else(|_| day.to_string())
}

fn week_of(start: &str) -> String {
    format!("Week of {}", day_words(start))
}

/// "Exported Thu 8 Oct · 7 days ago", "Exported today".
fn exported_line(s: &Value, age: Option<&Age>) -> String {
    format!("Exported {}", exported_when(s, age))
}

/// "Thu 8 Oct · 7 days ago", "today", "yesterday".
fn exported_when(s: &Value, age: Option<&Age>) -> String {
    match age {
        Some(a) if a.days <= 1 => human_date(&a.day).to_lowercase(),
        Some(a) => format!("{} · {} days ago", human_date(&a.day), a.days),
        None => text(&s["imported"], "exported_at").to_string(),
    }
}

/// Avex's "2026-10-08 21:14" as "Thu 8 Oct 21:14".
fn exported_at_words(raw: &str) -> String {
    match (raw.get(..10).filter(|d| date(d).is_some()), raw.get(10..)) {
        (Some(day), Some(time)) => format!("{} {}", day_words(day), time.trim()).trim_end().to_string(),
        (Some(day), None) => day_words(day),
        _ => raw.to_string(),
    }
}

// ---- Marks ---------------------------------------------------------------------------------

/// Strokes on the 16-unit grid for Avex's own marks; the rest are Tally's glyphs.
fn glyph(key: &str) -> Option<&'static str> {
    Some(match key {
        "avex" | "lift" => r##"<path d="M5 8h6" /><rect x="2.5" y="5" width="2.5" height="6" rx=".8" /><rect x="11" y="5" width="2.5" height="6" rx=".8" /><path d="M1.5 6.5v3M14.5 6.5v3" />"##,
        "streak" => r##"<path d="M8 14a4 4 0 0 1-4-4c0-3 4-4 3-8 3 1.5 5 4.5 5 8a4 4 0 0 1-4 4z" />"##,
        "record" => r##"<path d="M5 2.5h6v3.5a3 3 0 0 1-6 0z" /><path d="M5 3.5H3a2 2 0 0 0 2 3M11 3.5h2a2 2 0 0 1-2 3M8 9v3M5.5 13.5h5" />"##,
        "clock" => r##"<circle cx="8" cy="8" r="5.5" /><path d="M8 5v3.2l2 1.3" />"##,
        "run" => r##"<circle cx="10" cy="3" r="1.2" /><path d="M6 14l2-4.5 2 1.5 1 3M8 9.5l1-4-3 1-1.5 2.5M9 5.5l2 2.5 2 .5" />"##,
        "target" => r##"<circle cx="8" cy="8" r="5.5" /><circle cx="8" cy="8" r="2.5" /><circle cx="8" cy="8" r=".5" />"##,
        _ => return None,
    })
}

/// An Avex glyph, else Tally's.
pub(super) fn icon(key: &str, size: i32) -> gtk::Image {
    match glyph(key) {
        Some(g) => crate::icons::from_geometry(g, size, 1.5),
        None => glyph_image(key, size),
    }
}

/// Tally's neutral tile, with an Avex glyph.
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

// ---- Layout --------------------------------------------------------------------------------

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

/// A quiet way back above a page's title: "‹ Workouts".
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
    let run = go(ui, to);
    key.connect_clicked(move |_| run());
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

fn import_key(ui: &Rc<Ui>, caption: &str, class: &str) -> gtk::Button {
    let key = button(caption, class);
    key.set_widget_name("avex-import");
    key.set_tooltip_text(Some("Read the Training history file Avex exports (avex_export.json)"));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import(&ui);
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

fn open_workout_action(ui: &Rc<Ui>, id: i64) -> Action {
    let weak = Rc::downgrade(ui);
    Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            open_workout(&ui, id);
        }
    })
}

fn open_lift_action(ui: &Rc<Ui>, name: &str) -> Action {
    let weak = Rc::downgrade(ui);
    let name = name.to_string();
    Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            open_lift(&ui, &name);
        }
    })
}

/// Bars over `values`, the last in Relay's lime and the rest in a quiet ink, with a caption under
/// the first and the last; hovering one says its label and value.
fn bars(values: Vec<f64>, labels: Vec<String>, unit: &'static str, height: i32) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let area = gtk::DrawingArea::new();
    area.set_content_height(height);
    area.set_hexpand(true);
    let drawn = values.clone();
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (f64::from(w), f64::from(h));
        let n = drawn.len().max(1) as f64;
        let top = drawn.iter().copied().fold(0.0, f64::max).max(1.0);
        let gap = 4.0;
        let bw = ((w - gap * (n - 1.0)) / n).max(2.0);
        for (i, v) in drawn.iter().enumerate() {
            let last = i + 1 == drawn.len();
            let (r, g, b) = if last { (0.776, 0.949, 0.306) } else { (0.42, 0.40, 0.38) };
            cr.set_source_rgb(r, g, b);
            let bh = if *v > 0.0 { (v / top * (h - 2.0)).max(2.0) } else { 1.0 };
            let x = i as f64 * (bw + gap);
            let rad = (bw / 2.0).min(2.5);
            let y = h - bh;
            cr.move_to(x, h);
            cr.line_to(x, y + rad);
            cr.arc(x + rad, y + rad, rad, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
            cr.arc(x + bw - rad, y + rad, rad, 1.5 * std::f64::consts::PI, 0.0);
            cr.line_to(x + bw, h);
            cr.close_path();
            let _ = cr.fill();
        }
    });
    area.set_has_tooltip(true);
    let (tips, tip_labels) = (values, labels.clone());
    area.connect_query_tooltip(move |area, x, _, _, tooltip| {
        let n = tips.len();
        if n == 0 {
            return false;
        }
        let i = ((f64::from(x) / f64::from(area.width().max(1))) * n as f64).floor() as usize;
        let Some(v) = tips.get(i.min(n - 1)) else { return false };
        tooltip.set_text(Some(&format!("{}\n{} {unit}", tip_labels[i.min(n - 1)], group(v.round() as i64))));
        true
    });
    column.append(&area);
    if let (Some(first), Some(last)) = (labels.first(), labels.last()) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let a = label(first, "arbiter-range");
        a.set_hexpand(true);
        row.append(&a);
        row.append(&label(last, "arbiter-range"));
        column.append(&row);
    }
    column
}

/// Room on the left of a progress line for its highest and lowest values.
const LINE_AXIS: f64 = 46.0;

/// A line over `points`, its records marked, on a scale that starts near the lowest value so
/// progress shows, the highest and lowest values written at their height; hovering a point says
/// its day and value.
fn progress_line(points: Vec<(String, f64, bool)>, unit: String, height: i32) -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    let area = gtk::DrawingArea::new();
    area.set_content_height(height);
    area.set_hexpand(true);
    overlay.set_child(Some(&area));
    if points.is_empty() {
        return overlay;
    }
    let low = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let high = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let pad = ((high - low) * 0.15).max(1.0);
    let (lo, hi) = (low - pad, high + pad);
    let h = f64::from(height);
    let y_of = move |v: f64| 6.0 + (hi - v) / (hi - lo) * (h - 12.0);
    for value in if high > low { vec![high, low] } else { vec![high] } {
        let tick = label(&trimmed(value), "threads-chart-axis");
        tick.set_halign(gtk::Align::Start);
        tick.set_valign(gtk::Align::Start);
        tick.set_margin_top((y_of(value) - 7.0).max(0.0) as i32);
        tick.set_can_target(false);
        overlay.add_overlay(&tick);
    }
    let drawn = points.clone();
    area.set_draw_func(move |_, cr, w, _| {
        let w = f64::from(w);
        let x_of = |i: usize| if drawn.len() == 1 { (LINE_AXIS + w) / 2.0 } else { LINE_AXIS + 6.0 + i as f64 / (drawn.len() - 1) as f64 * (w - LINE_AXIS - 12.0) };
        // Hairlines at the highest and the lowest, where their values are written.
        cr.set_source_rgba(0.93, 0.91, 0.89, 0.08);
        cr.set_line_width(1.0);
        for v in [high, low] {
            let y = y_of(v).round() + 0.5;
            cr.move_to(LINE_AXIS, y);
            cr.line_to(w, y);
        }
        let _ = cr.stroke();
        cr.set_source_rgb(0.416, 0.624, 0.847);
        cr.set_line_width(2.0);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        for (i, p) in drawn.iter().enumerate() {
            if i == 0 {
                cr.move_to(x_of(i), y_of(p.1));
            } else {
                cr.line_to(x_of(i), y_of(p.1));
            }
        }
        let _ = cr.stroke();
        for (i, p) in drawn.iter().enumerate() {
            if p.2 {
                cr.set_source_rgb(0.776, 0.949, 0.306);
                cr.arc(x_of(i), y_of(p.1), 3.5, 0.0, 2.0 * std::f64::consts::PI);
                let _ = cr.fill();
            } else if drawn.len() <= 40 {
                cr.set_source_rgb(0.416, 0.624, 0.847);
                cr.arc(x_of(i), y_of(p.1), 2.0, 0.0, 2.0 * std::f64::consts::PI);
                let _ = cr.fill();
            }
        }
    });
    area.set_has_tooltip(true);
    area.connect_query_tooltip(move |area, x, _, _, tooltip| {
        let n = points.len();
        if n == 0 {
            return false;
        }
        let w = f64::from(area.width().max(1));
        let span = (w - LINE_AXIS - 12.0).max(1.0);
        let i = if n == 1 { 0 } else { (((f64::from(x) - LINE_AXIS - 6.0) / span) * (n - 1) as f64).round().clamp(0.0, (n - 1) as f64) as usize };
        let (day, v, record) = &points[i];
        let mark = if *record { " · a record" } else { "" };
        tooltip.set_text(Some(&format!("{}\nest. max {} {unit}{mark}", human_date(day), trimmed(*v))));
        true
    });
    overlay
}

// ---- The pages -----------------------------------------------------------------------------

/// Avex's tabs as Dev's segmented control, above each Avex page.
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

/// A title and a first message on every page, before its first read; and Alt+← and Alt+→ on the
/// Workout page.
pub fn install() {
    for (name, title) in ALL.into_iter().zip(["Avex", "Workouts", "Lifts", "Workout", "Lift"]) {
        if let Some(page) = page(name) {
            heading(&page.head, title, "", &[]);
            message(&page.body, "Reading Avex…");
        }
    }
    if let Some(page) = page(WORKOUT) {
        // Global, so it answers wherever the focus is; it does nothing off this page.
        let keys = gtk::ShortcutController::new();
        keys.set_scope(gtk::ShortcutScope::Global);
        for (key, newer) in [(gtk::gdk::Key::Left, false), (gtk::gdk::Key::Right, true)] {
            let action = gtk::CallbackAction::new(move |_, _| {
                let Some(ui) = super::threads::the_ui_pub() else { return glib::Propagation::Proceed };
                if *ui.page.borrow() != WORKOUT {
                    return glib::Propagation::Proceed;
                }
                let (older, later) = STATE.with(|s| s.neighbours.get());
                match if newer { later } else { older } {
                    Some(id) => {
                        open_workout(&ui, id);
                        glib::Propagation::Stop
                    }
                    None => glib::Propagation::Proceed,
                }
            });
            let trigger = gtk::KeyvalTrigger::new(key, gtk::gdk::ModifierType::ALT_MASK);
            keys.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
        }
        page.head.add_controller(keys);
    }
}

/// Open workout `id` on the Workout page.
pub(super) fn open_workout(ui: &Rc<Ui>, id: i64) {
    STATE.with(|s| s.workout.set(Some(id)));
    if *ui.page.borrow() == WORKOUT {
        ui.refresh_page();
    } else {
        ui.navigate(WORKOUT);
    }
}

/// Open lift `name` on the Lift page.
pub(super) fn open_lift(ui: &Rc<Ui>, name: &str) {
    STATE.with(|s| *s.lift.borrow_mut() = Some(name.to_string()));
    if *ui.page.borrow() == LIFT {
        ui.refresh_page();
    } else {
        ui.navigate(LIFT);
    }
}

pub async fn refresh(ui: &Rc<Ui>, name: &str) {
    let Some(page) = page(name) else { return };
    match name {
        "avex-home" => {
            let read = summary(ui).await;
            if *ui.page.borrow() != name {
                return;
            }
            match read {
                Ok((s, age)) => home(ui, &page, &s, age.as_ref()),
                Err(e) => failed(ui, &page, "Avex", &e),
            }
        }
        "avex-workouts" => {
            let shown = STATE.with(|s| s.shown.get()).max(PAGE_SIZE);
            let (list, summary, cardio) = tokio::join!(
                ui.call("gym.sessions", json!({"limit": shown})),
                ui.call("gym.summary", json!({})),
                ui.call("gym.cardio", json!({"limit": 1000}))
            );
            if *ui.page.borrow() != name {
                return;
            }
            match list {
                Ok(l) => workouts_page(ui, &page, &l, summary.ok().as_ref(), cardio.ok().as_ref()),
                Err(e) => failed(ui, &page, "Workouts", &e),
            }
        }
        "avex-lifts" => {
            // The four weeks each lift is read over end where the copy does.
            let age = ui.call("gym.summary", json!({})).await.ok().as_ref().and_then(age_of);
            let read = ui.call("gym.lifts", json!({"today": as_of(age.as_ref())})).await;
            if *ui.page.borrow() != name {
                return;
            }
            match read {
                Ok(l) => lifts_page(ui, &page, &l, age.as_ref()),
                Err(e) => failed(ui, &page, "Lifts", &e),
            }
        }
        WORKOUT => {
            // A display smoke run opens a workout with RELAY_NATIVE_AVEX=<id>.
            let smoke = std::env::var("RELAY_NATIVE_AVEX").ok().and_then(|v| v.parse().ok());
            let Some(id) = STATE.with(|s| s.workout.get()).or(smoke) else {
                ui.navigate("avex-workouts");
                return;
            };
            let read = ui.call("gym.session.get", json!({"id": id})).await;
            if *ui.page.borrow() != name || STATE.with(|s| s.workout.get()).is_some_and(|w| w != id) {
                return;
            }
            match read {
                Ok(d) => workout_page(ui, &page, &d),
                Err(e) => failed(ui, &page, "Workout", &e),
            }
        }
        LIFT => {
            // A display smoke run opens a lift with RELAY_NATIVE_AVEX=<name>.
            let smoke = std::env::var("RELAY_NATIVE_AVEX").ok();
            let Some(lift) = STATE.with(|s| s.lift.borrow().clone()).or(smoke) else {
                ui.navigate("avex-lifts");
                return;
            };
            let age = ui.call("gym.summary", json!({})).await.ok().as_ref().and_then(age_of);
            let read = ui.call("gym.lift.get", json!({"name": lift, "limit": 400, "today": as_of(age.as_ref())})).await;
            if *ui.page.borrow() != name || STATE.with(|s| s.lift.borrow().as_deref().is_some_and(|l| l != lift)) {
                return;
            }
            match read {
                Ok(d) => lift_page(ui, &page, &d, age.as_ref()),
                Err(e) => failed(ui, &page, "Lift", &e),
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

/// Before anything was imported: what Avex is to Relay, and the three steps to bring it in.
fn empty_state(ui: &Rc<Ui>, body: &gtk::Box) {
    let rows = card(body, "Bring in your training", None);
    let b = block(&rows);
    let about = label(
        "Avex keeps everything on your phone and has no internet, so Relay reads the file it exports. Nothing goes back to the phone, and Avex is never changed from here.",
        "money-muted",
    );
    about.set_wrap(true);
    b.append(&about);
    for (n, step) in [
        "In Avex: Settings, Export, Training history (JSON).",
        "Send avex_export.json to this computer: a cable, a cloud drive or Quick Share.",
        "Import it here. Import again whenever you want Relay to catch up; each import replaces the last.",
    ]
    .iter()
    .enumerate()
    {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("arbiter-steps");
        let number = label(&(n + 1).to_string(), "arbiter-step-number");
        number.set_valign(gtk::Align::Start);
        number.set_xalign(0.5);
        row.append(&number);
        let l = label(step, "arbiter-step");
        l.set_wrap(true);
        l.set_hexpand(true);
        row.append(&l);
        b.append(&row);
    }
    let key = import_key(ui, "Import Avex export…", "primary");
    key.set_halign(gtk::Align::Start);
    b.append(&key);
}

/// "This copy is 7 days old": said above what a stale copy shows, since its week is not this one.
fn stale_notice(age: &Age) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("avex-notice");
    let clock = icon("clock", 15);
    clock.add_css_class("avex-notice-mark");
    clock.set_valign(gtk::Align::Center);
    row.append(&clock);
    let words = label(&format!("This copy is {} old. Workouts since then are on your phone.", plural(age.days.max(1) as usize, "day")), "avex-notice-text");
    words.set_wrap(true);
    words.set_hexpand(true);
    words.set_xalign(0.0);
    row.append(&words);
    row
}

/// Overview: when the copy is from, its week beside the twelve before, then recent workouts and
/// records beside open goals and the lifts done most. A copy from before this week shows the
/// week it was exported in, and says so.
fn home(ui: &Rc<Ui>, page: &Page, s: &Value, age: Option<&Age>) {
    let unit = text(s, "unit").to_string();
    if s["imported"].is_null() {
        heading(&page.head, "Avex", "Your training, read from Avex's export", &[]);
        clear(&page.body);
        empty_state(ui, &page.body);
        return;
    }
    let stale = age.is_some_and(|a| a.stale);
    let refresh = import_key(ui, "Import newer…", if stale { "primary" } else { "" });
    let more = copy_menu(ui, s);
    let mut context = exported_line(s, age);
    if let Some(name) = s["imported"]["user_name"].as_str().filter(|n| !n.is_empty()) {
        context = format!("{name}'s training · {context}");
    }
    heading(&page.head, "Avex", &context, &[refresh.upcast_ref(), more.upcast_ref()]);
    clear(&page.body);
    let body = &page.body;
    if let Some(a) = age.filter(|a| a.stale) {
        body.append(&stale_notice(a));
    }

    let week = &s["this_week"];
    let last = &s["last_week"];
    // Read as of the export, "this week" is the week it was exported in.
    let (caption, earlier_name, before, that, none) = if stale {
        (week_of(text(week, "start")), "The week before", "the week before", "that week", "No workouts that week")
    } else {
        (String::from("This week"), "Last week", "last week", "this week", "No workouts yet this week")
    };
    let target = s["imported"]["days_per_week"].as_i64().unwrap_or(0);
    let (_, cells) = strip(body);
    let sessions = week["sessions"].as_i64().unwrap_or(0);
    let of = if target > 0 { format!(" of {target}") } else { String::new() };
    let earlier = last["sessions"].as_i64().unwrap_or(0);
    cell(&cells, "calendar", &caption, &format!("{sessions}{of}"), &format!("{earlier_name} {earlier}"), None);
    let vol = num(&week["volume"]).unwrap_or(0.0);
    let prior = num(&last["volume"]).unwrap_or(0.0);
    // An empty week says so: a percentage of nothing reads as a collapse.
    let change = if vol <= 0.0 {
        String::from(none)
    } else if prior > 0.0 {
        format!("{} on {before}", whole_pct((vol / prior - 1.0) * 100.0))
    } else {
        format!("Nothing {before}")
    };
    cell(&cells, "chart", "Volume", &volume(&week["volume"], &unit), &change, None);
    if let Some(volume_cell) = cells.last_child() {
        volume_cell.set_tooltip_text(Some("Weight × reps over every working set, as Avex counts it"));
    }
    let streak = s["streak_weeks"].as_i64().unwrap_or(0);
    cell(&cells, "spark", "Streak", &plural(streak.max(0) as usize, "week"), "In a row with a workout", None);
    let since = if stale {
        // Days since, counted from the export, would be wrong by the copy's age: say the day.
        rows(s, "recent").iter().find(|w| w["untracked"] != true).map_or_else(|| String::from("Never"), |w| human_date(text(w, "day")))
    } else {
        match s["days_since_last"].as_i64() {
            Some(0) => String::from("Today"),
            Some(1) => String::from("Yesterday"),
            Some(d) => format!("{d} days ago"),
            None => String::from("Never"),
        }
    };
    let mut sub = format!("{} training {that}", minutes(week["active_minutes"].as_i64().unwrap_or(0)));
    let cardio = week["cardio_minutes"].as_i64().unwrap_or(0);
    if cardio > 0 {
        sub.push_str(&format!(" · {} cardio", minutes(cardio)));
    }
    cell(&cells, "clock", "Last workout", &since, &sub, None);

    // Twelve weeks of volume, the copy's last one last.
    let weeks = rows(s, "weeks");
    if !weeks.is_empty() {
        let rows = card_asking(body, "Volume by week", None, Some((ui, "How has my training volume changed over the last few weeks?")));
        let b = block(&rows);
        let values = weeks.iter().map(|w| num(&w["volume"]).unwrap_or(0.0)).collect();
        let labels = weeks.iter().map(|w| week_of(text(w, "start"))).collect();
        b.append(&bars(values, labels, if unit == "kg" { "kg" } else { "lb" }, 96));
    }

    let pair = columns(body, 2);
    let recent = card(&pair[0], "Recent workouts", Some(("All workouts", go(ui, "avex-workouts"))));
    let list = rows(s, "recent");
    if list.is_empty() {
        quiet_row(&recent, "No workouts in this export.");
    }
    for w in &list {
        recent.append(&workout_row(ui, w, &unit));
    }
    let records = card(&pair[0], "Records", None);
    let list = rows(s, "records");
    if list.is_empty() {
        quiet_row(&records, "No records marked by Avex yet.");
    }
    for r in list.iter().take(5) {
        records.append(&record_row(ui, r, &unit));
    }

    // What the person is working toward comes before what the lifts did.
    let goals = rows(s, "goals");
    if !goals.is_empty() {
        let card = card(&pair[1], "Goals", None);
        for g in &goals {
            card.append(&goal_row(ui, g, &unit));
        }
    }
    let lifts = card_asking(&pair[1], "Lifts", Some(("All lifts", go(ui, "avex-lifts"))), Some((ui, "Which of my lifts are progressing and which have stalled?")));
    let list = rows(s, "lifts");
    if list.is_empty() {
        quiet_row(&lifts, "No lifts with working sets yet.");
    }
    for l in &list {
        lifts.append(&lift_row(ui, l, &unit));
    }
}

/// About this copy and Forget, behind a menu: neither is an everyday key.
fn copy_menu(ui: &Rc<Ui>, s: &Value) -> gtk::MenuButton {
    let key = gtk::MenuButton::new();
    key.set_icon_name("view-more-symbolic");
    key.set_widget_name("avex-more");
    key.set_tooltip_text(Some("About this copy"));
    key.set_valign(gtk::Align::Center);
    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 10);
    list.add_css_class("avex-about");
    list.append(&label("About this copy", "avex-about-title"));
    let facts = gtk::Grid::new();
    facts.set_row_spacing(6);
    facts.set_column_spacing(16);
    let i = &s["imported"];
    let t = &s["totals"];
    let untracked = t["untracked"].as_i64().unwrap_or(0);
    let counted = t["sessions"].as_i64().unwrap_or(0);
    let cardio = t["cardio"].as_i64().unwrap_or(0);
    let mut lines = vec![
        ("Exported", format!("{} · Avex {}", exported_at_words(text(i, "exported_at")), text(i, "app_version"))),
        ("Imported", crate::relative::ago(text(i, "imported_at"), crate::relative::Form::Long).unwrap_or_default()),
        ("File", text(i, "file").to_string()),
        (
            "Workouts",
            if untracked > 0 { format!("{} counted · {} untracked", group(counted), group(untracked)) } else { group(counted) },
        ),
        ("Sets", format!("{} in the counted workouts", group(t["sets"].as_i64().unwrap_or(0)))),
        ("Cardio", format!("{} {} · {}", group(cardio), if cardio == 1 { "entry" } else { "entries" }, minutes(t["cardio_minutes"].as_i64().unwrap_or(0)))),
        ("Units", if text(s, "unit") == "kg" { String::from("Kilograms, as Avex shows them") } else { String::from("Pounds, as Avex shows them") }),
    ];
    if let (Some(first), Some(last)) = (t["first_day"].as_str(), t["last_day"].as_str()) {
        lines.insert(3, ("Spanning", format!("{} to {}", day_words(first), day_words(last))));
    }
    for (row, (name, value)) in lines.into_iter().enumerate() {
        let n = label(name, "avex-fact-name");
        n.set_xalign(0.0);
        n.set_valign(gtk::Align::Start);
        let v = label(&value, "avex-fact");
        v.set_xalign(0.0);
        v.set_wrap(true);
        v.set_max_width_chars(34);
        facts.attach(&n, 0, row as i32, 1, 1);
        facts.attach(&v, 1, row as i32, 1, 1);
    }
    list.append(&facts);
    let note = label("Read only. Nothing here goes back to the phone, and Avex is never changed from here.", "money-row-detail");
    note.set_wrap(true);
    note.set_max_width_chars(44);
    note.set_xalign(0.0);
    list.append(&note);
    let forget = button("Forget the copy…", "money-destructive");
    forget.set_widget_name("avex-reset");
    forget.set_halign(gtk::Align::Start);
    forget.set_tooltip_text(Some("Erase the imported history on this PC. Avex keeps its own."));
    let (weak, pop) = (Rc::downgrade(ui), popover.downgrade());
    confirm_inline(&forget, "Forget it", move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        if let Some(p) = pop.upgrade() {
            p.popdown();
        }
        glib::spawn_future_local(async move {
            match ui.call("gym.reset", json!({})).await {
                Ok(_) => toast(&ui, "Forgotten. Import an export to bring it back.", None),
                Err(e) => ui.show_error(&said(&e)),
            }
            ui.refresh_page();
        });
    });
    list.append(&forget);
    popover.set_child(Some(&list));
    key.set_popover(Some(&popover));
    key
}

/// A workout as a row: its title over the day and lifts, its volume; opens the Workout page.
fn workout_row(ui: &Rc<Ui>, w: &Value, unit: &str) -> gtk::Box {
    let lifts: Vec<&str> = w["lifts"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut detail = vec![human_date(text(w, "day"))];
    if w["minutes"].as_i64().unwrap_or(0) > 0 {
        detail.push(minutes(w["minutes"].as_i64().unwrap_or(0)));
    }
    if !lifts.is_empty() {
        detail.push(lifts.join(", "));
    }
    let mut title = text(w, "title").to_string();
    if w["untracked"] == true {
        title.push_str(" · untracked");
    }
    let prs = w["prs"].as_i64().unwrap_or(0);
    if prs > 0 {
        title.push_str(&format!(" · {}", plural(prs as usize, "record")));
    }
    let line = icon_row(&mark("lift"), &title, &detail.join(" · "), &volume(&w["volume"], unit), if w["untracked"] == true { "money-muted" } else { "" });
    row_key(&line, "Open this workout", open_workout_action(ui, w["id"].as_i64().unwrap_or(0)))
}

/// A cardio or rest-day entry, among the workouts by its day. It opens nothing: this is all of it.
fn cardio_row(c: &Value) -> gtk::Box {
    let kind = text(c, "kind");
    let rest = kind.eq_ignore_ascii_case("rest");
    let title = if rest { String::from("Rest day") } else { word(kind) };
    let mut detail = vec![human_date(text(c, "day"))];
    if let Some(km) = num(&c["distance_km"]).filter(|d| *d > 0.0) {
        let km = format!("{km:.2}");
        detail.push(format!("{} km", km.trim_end_matches('0').trim_end_matches('.')));
    }
    for part in [text(c, "effort"), text(c, "note")] {
        if !part.is_empty() {
            detail.push(part.to_lowercase().replace('_', " "));
        }
    }
    let m = c["minutes"].as_i64().unwrap_or(0);
    let figure = if rest || m <= 0 { String::new() } else { minutes(m) };
    icon_row(&mark(if rest { "clock" } else { "run" }), &title, &detail.join(" · "), &figure, "money-muted")
}

fn record_row(ui: &Rc<Ui>, r: &Value, unit: &str) -> gtk::Box {
    let detail = match num(&r["e1rm"]) {
        Some(e) => format!("{} · est. max {} {unit}", human_date(text(r, "day")), trimmed(e)),
        None => human_date(text(r, "day")),
    };
    let line = icon_row(&mark("record"), text(r, "lift"), &detail, &set_words(&r["weight"], &r["reps"], unit), "money-in");
    row_key(&line, "Open this lift", open_lift_action(ui, text(r, "lift")))
}

/// The four-week change as a small chip: the one place a lift row has colour, green up, amber
/// down, none while it holds.
fn change_chip(change: f64) -> gtk::Label {
    let chip = label(&pct(change), "avex-change");
    if change > 1.0 {
        chip.add_css_class("avex-up");
    } else if change < -1.0 {
        chip.add_css_class("avex-down");
    }
    chip.set_valign(gtk::Align::Center);
    chip
}

/// "est. max 250.8 lb +4.9%": the best estimated max of the last four weeks, and its change on
/// the four before, aligned down a card.
fn max_figure(e1rm: f64, change: Option<f64>, unit: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    b.set_valign(gtk::Align::Center);
    b.append(&label("est. max", "avex-est"));
    let figure = label(&format!("{} {unit}", trimmed(e1rm)), "money-amount");
    figure.set_width_chars(9);
    figure.set_xalign(1.0);
    b.append(&figure);
    // A fixed slot, so the figures line up down a card whether or not a lift has a change.
    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    chip.set_size_request(58, -1);
    // Set, so the chip's own expanding never reaches the row.
    chip.set_hexpand(false);
    if let Some(c) = change {
        let c = change_chip(c);
        c.set_hexpand(true);
        c.set_halign(gtk::Align::End);
        chip.append(&c);
    }
    b.append(&chip);
    b.set_hexpand(false);
    b.set_tooltip_text(Some(&format!("{EST_MAX}. The best of the last four weeks; the change is on the four weeks before.")));
    b
}

/// A lift as a row: how often, when last, and its best set; at the end its estimated max and
/// four-week change.
fn lift_row(ui: &Rc<Ui>, l: &Value, unit: &str) -> gtk::Box {
    let mut detail = vec![plural(l["sessions"].as_i64().unwrap_or(0).max(0) as usize, "workout"), format!("last {}", human_date(text(l, "last_day")))];
    if l["best"].is_object() {
        detail.push(format!("best {}", set_words(&l["best"]["weight"], &l["best"]["reps"], unit)));
    } else if let Some(reps) = l["best_reps"].as_i64() {
        detail.push(format!("best {}", plural(reps.max(0) as usize, "rep")));
    }
    let line = icon_row(&mark("lift"), text(l, "name"), &detail.join(" · "), "", "");
    if let Some(e) = num(&l["recent_e1rm"]) {
        line.append(&max_figure(e, num(&l["change_pct"]), unit));
    }
    row_key(&line, "Open this lift", open_lift_action(ui, text(l, "name")))
}

/// A goal in the person's words. A lift goal reads against that lift's best estimated max: a
/// meter until it is reached.
fn goal_row(ui: &Rc<Ui>, g: &Value, unit: &str) -> gtk::Box {
    let kind = text(g, "kind");
    let key = text(g, "target_key");
    let target = num(&g["target_value"]);
    let best = &g["best"];
    let lift_goal = matches!(kind, "lift_1rm" | "lift_target");
    let name = match (kind, target) {
        (_, Some(v)) if lift_goal => {
            let lift = if best.is_object() { text(best, "lift").to_string() } else { word(key) };
            format!("{lift} to an est. max of {} {unit}", trimmed(v))
        }
        ("consistency", Some(v)) => format!("Train {} days a week", trimmed(v)),
        ("conditioning", Some(v)) => format!("{} min of cardio a week", trimmed(v)),
        ("muscle_volume", Some(v)) => format!("{}: {} sets a week", word(key), trimmed(v)),
        ("bodyweight", Some(v)) => format!("Body weight {} {unit}", trimmed(v)),
        (k, Some(v)) if key.is_empty() => format!("{}: {}", word(k), trimmed(v)),
        (k, Some(v)) => format!("{}: {} · {}", word(k), word(key), trimmed(v)),
        (k, None) => format!("{}: {}", word(k), word(key)),
    };
    let progress = if lift_goal { num(&best["e1rm"]).zip(target).filter(|(_, t)| *t > 0.0) } else { None };
    let mut detail = Vec::new();
    if lift_goal && best.is_object() {
        detail.push(format!("best {} on {}", set_words(&best["weight"], &best["reps"], unit), human_date(text(best, "day"))));
    }
    match text(g, "note") {
        "" if detail.is_empty() => detail.push(format!("Set {}", human_date(text(g, "created_day")))),
        "" => {}
        note => detail.push(note.to_string()),
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.append(&mark("target"));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let n = label(&name, "money-row-title");
    n.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&n);
    let d = label(&detail.join(" · "), "money-row-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    if let Some((e, t)) = progress.filter(|(e, t)| e < t) {
        let meter = meter_in(e / t, None, Tone::Plain, 4);
        // Never narrower than the meter can draw.
        meter.set_size_request(40, -1);
        meter.set_margin_top(4);
        meter.set_tooltip_text(Some(&format!("est. max {} of {} {unit}", trimmed(e), trimmed(t))));
        words.append(&meter);
    }
    row.append(&words);
    if let Some((e, t)) = progress {
        let (figure, class) = if e >= t { (String::from("Reached"), "money-in") } else { (format!("{} of {} {unit}", trimmed(e), trimmed(t)), "") };
        let f = label(&figure, "money-amount");
        if !class.is_empty() {
            f.add_css_class(class);
        }
        f.set_valign(gtk::Align::Center);
        f.set_tooltip_text(Some(&format!("The lift's best est. max against the goal. {EST_MAX}")));
        row.append(&f);
    }
    if lift_goal && best.is_object() {
        return row_key(&row, "Open this lift", open_lift_action(ui, text(best, "lift")));
    }
    row
}

/// A row of the Workouts page: a workout or a cardio entry.
enum Item<'a> {
    Workout(&'a Value),
    Cardio(&'a Value),
}

/// Workouts: every one with the cardio between them, newest first, a page at a time.
fn workouts_page(ui: &Rc<Ui>, page: &Page, l: &Value, summary: Option<&Value>, cardio: Option<&Value>) {
    let unit = summary.map(|s| text(s, "unit").to_string()).unwrap_or_else(|| String::from("lb"));
    let total = l["total"].as_u64().unwrap_or(0);
    let list = rows(l, "sessions");
    let mut entries = cardio.map(|c| rows(c, "entries")).unwrap_or_default();
    // A page that stops short of the first workout stops its cardio there too.
    if (list.len() as u64) < total {
        if let Some(oldest) = list.last() {
            let from = text(oldest, "day").to_string();
            entries.retain(|c| text(c, "day") >= from.as_str());
        }
    }
    let context = match summary.filter(|s| !s["imported"].is_null()) {
        Some(s) => {
            let untracked = s["totals"]["untracked"].as_u64().unwrap_or(0);
            let mut parts = vec![if untracked > 0 { format!("{}, {untracked} untracked", plural(total as usize, "workout")) } else { plural(total as usize, "workout") }];
            let all = s["totals"]["cardio"].as_u64().unwrap_or(0);
            if all > 0 {
                parts.push(format!("{all} cardio {}", if all == 1 { "entry" } else { "entries" }));
            }
            parts.push(exported_line(s, age_of(s).as_ref()));
            parts.join(" · ")
        }
        None => String::from("Nothing imported from Avex yet"),
    };
    heading(&page.head, "Workouts", &context, &[]);
    clear(&page.body);
    if list.is_empty() && entries.is_empty() {
        if summary.is_some_and(|s| s["imported"].is_null()) {
            empty_state(ui, &page.body);
        } else {
            message(&page.body, "No workouts in this export.");
        }
        return;
    }
    // Both lists are newest first: merged by day and time.
    let mut items = Vec::with_capacity(list.len() + entries.len());
    let (mut a, mut b) = (list.iter().peekable(), entries.iter().peekable());
    let when = |v: &Value| (text(v, "day").to_string(), text(v, "time").to_string());
    loop {
        let workout_first = match (a.peek(), b.peek()) {
            (Some(w), Some(c)) => when(w) >= when(c),
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => break,
        };
        if workout_first {
            items.extend(a.next().map(Item::Workout));
        } else {
            items.extend(b.next().map(Item::Cardio));
        }
    }
    // Grouped by month, newest first.
    let mut current = String::new();
    let mut card_rows: Option<gtk::Box> = None;
    for item in &items {
        let day = match item {
            Item::Workout(v) | Item::Cardio(v) => text(v, "day"),
        };
        let month = day.get(..7).unwrap_or("").to_string();
        if month != current || card_rows.is_none() {
            current = month;
            let caption = date(&format!("{current}-01")).and_then(|d| d.format("%B %Y").ok()).map(|s| s.to_string()).unwrap_or_else(|| current.clone());
            card_rows = Some(card(&page.body, &caption, None));
        }
        if let Some(rows) = &card_rows {
            match item {
                Item::Workout(w) => rows.append(&workout_row(ui, w, &unit)),
                Item::Cardio(c) => rows.append(&cardio_row(c)),
            }
        }
    }
    if (list.len() as u64) < total {
        let more = button(&format!("Show {} more", (total - list.len() as u64).min(u64::from(PAGE_SIZE))), "money-secondary");
        more.set_halign(gtk::Align::Center);
        let weak = Rc::downgrade(ui);
        more.connect_clicked(move |_| {
            STATE.with(|s| s.shown.set(s.shown.get().max(PAGE_SIZE) + PAGE_SIZE));
            if let Some(ui) = weak.upgrade() {
                ui.refresh_page();
            }
        });
        page.body.append(&more);
    }
}

/// Earlier and Later: the workouts either side, as Alt+← and Alt+→ also go.
fn step_keys(ui: &Rc<Ui>, older: Option<i64>, newer: Option<i64>) -> gtk::Box {
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    for (chevron, caption, about, id) in [("chevron-left", "Earlier", "The workout before · Alt+←", older), ("chevron-right", "Later", "The workout after · Alt+→", newer)] {
        let key = button("", "money-secondary");
        key.add_css_class("avex-step");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let image = crate::icons::image(chevron, 12);
        image.set_valign(gtk::Align::Center);
        let words = label(caption, "");
        if chevron == "chevron-left" {
            row.append(&image);
            row.append(&words);
        } else {
            row.append(&words);
            row.append(&image);
        }
        key.set_child(Some(&row));
        key.update_property(&[gtk::accessible::Property::Label(caption)]);
        key.set_tooltip_text(Some(if id.is_some() { about } else if chevron == "chevron-left" { "This is the first workout" } else { "This is the newest workout" }));
        key.set_sensitive(id.is_some());
        if let Some(id) = id {
            let run = open_workout_action(ui, id);
            key.connect_clicked(move |_| run());
        }
        keys.append(&key);
    }
    keys
}

/// One workout: its figures, the journal, and every exercise in one card, each with its sets
/// and how it compares with the last time.
fn workout_page(ui: &Rc<Ui>, page: &Page, d: &Value) {
    let unit = text(d, "unit").to_string();
    let row = &d["row"];
    let (older, newer) = (d["older_id"].as_i64(), d["newer_id"].as_i64());
    STATE.with(|s| s.neighbours.set((older, newer)));
    let steps = step_keys(ui, older, newer);
    let mut context = vec![human_date(text(row, "day")), text(row, "time").to_string()];
    if row["untracked"] == true {
        context.push(String::from("untracked: left out of every total"));
    }
    let title = text(row, "title");
    heading(&page.head, title, &context.join(" · "), &[steps.upcast_ref()]);
    back_key(ui, &page.head, "Workouts", "avex-workouts");
    clear(&page.body);
    let body = &page.body;
    let (_, cells) = strip(body);
    cell(&cells, "clock", "Training", &minutes(row["minutes"].as_i64().unwrap_or(0)), "", None);
    cell(&cells, "list", "Sets", &row["sets"].as_i64().unwrap_or(0).to_string(), "", None);
    cell(&cells, "chart", "Volume", &volume(&row["volume"], &unit), "", None);
    let prs = row["prs"].as_i64().unwrap_or(0);
    cell(&cells, "spark", "Records", &prs.to_string(), "", if prs > 0 { Some("money-in") } else { None });
    let journal = text(d, "journal");
    let mood = text(row, "mood");
    if !journal.is_empty() || !mood.is_empty() {
        let rows = card(body, "Journal", None);
        let b = block(&rows);
        if !mood.is_empty() {
            b.append(&label(&format!("Mood: {}", word(mood)), "money-muted"));
        }
        if !journal.is_empty() {
            let l = label(journal, "arbiter-sentence");
            l.set_wrap(true);
            l.set_selectable(true);
            b.append(&l);
        }
    }
    let exercises = rows(d, "exercises");
    if exercises.is_empty() {
        return;
    }
    let next = if title == "Freestyle" { String::from("Plan my next workout from this one") } else { format!("Plan my next {title}") };
    let list = card_asking(body, "Exercises", None, Some((ui, &next)));
    for e in &exercises {
        exercise(ui, &list, e, &unit);
    }
}

/// One exercise in the workout's card: its name as a sub-head, its sets, and a line with how it
/// went against the last time.
fn exercise(ui: &Rc<Ui>, list: &gtk::Box, e: &Value, unit: &str) {
    let skipped = e["skipped"] == true;
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.add_css_class("avex-exercise-head");
    let name = label(text(e, "name"), "avex-exercise-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    head.append(&name);
    if e["was_pr"] == true {
        head.append(&label("record", "avex-record-mark"));
    }
    if skipped {
        head.append(&label("skipped", "money-row-detail"));
    }
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    head.append(&gap);
    if !skipped {
        let history = button("History", "money-text-action");
        history.set_tooltip_text(Some("Every workout with this lift"));
        let run = open_lift_action(ui, text(e, "name"));
        history.connect_clicked(move |_| run());
        head.append(&history);
    }
    list.append(&head);
    if skipped {
        return;
    }
    if let Some(note) = e["note"].as_str().filter(|n| !n.is_empty()) {
        let l = label(note, "money-row-detail");
        l.set_wrap(true);
        l.set_xalign(0.0);
        l.add_css_class("avex-exercise-note");
        list.append(&l);
    }
    // Working sets are numbered on their own; a warm-up shows as W.
    let mut number = 0;
    for s in rows(e, "sets") {
        if text(&s, "kind") != "warmup" {
            number += 1;
        }
        list.append(&set_row(number, &s, unit));
    }
    let mut rest = Vec::new();
    // Sets without weight move none.
    if num(&e["volume"]).is_some_and(|v| v > 0.0) {
        rest.push(volume(&e["volume"], unit));
    }
    if let Some(best) = num(&e["best_e1rm"]) {
        rest.push(format!("est. max {} {unit}", trimmed(best)));
    }
    if let Some(d) = e["difficulty"].as_str().filter(|d| !d.is_empty()) {
        rest.push(format!("felt {}", word(d).to_lowercase()));
    }
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    foot.add_css_class("avex-exercise-foot");
    // How it went against last time leads, a step brighter than the rest.
    if let Some(words) = versus(e, unit) {
        foot.append(&label(&if rest.is_empty() { words } else { format!("{words} · ") }, "avex-versus"));
    }
    if !rest.is_empty() {
        let l = label(&rest.join(" · "), "money-row-detail");
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        foot.append(&l);
    }
    list.append(&foot);
}

/// "vs Thu 1 Oct: +10 lb on the top set", from the exercise's `previous`; nothing the first time.
fn versus(e: &Value, unit: &str) -> Option<String> {
    let before = e["previous"].as_object()?;
    let day = day_words(before.get("day").and_then(Value::as_str).unwrap_or(""));
    // The top set, as the lift's history reads it: the heaviest working set, then the most reps.
    let sets = rows(e, "sets");
    let top = sets
        .iter()
        .filter(|s| text(s, "kind") != "warmup" && s["reps"].as_i64().unwrap_or(0) > 0)
        .max_by(|a, b| {
            let key = |s: &Value| (num(&s["weight"]).unwrap_or(-1.0), s["reps"].as_i64().unwrap_or(0));
            key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let (now_w, now_r) = (num(&top["weight"]), top["reps"].as_i64().unwrap_or(0));
    let (then_w, then_r) = (before.get("top_weight").and_then(Value::as_f64), before.get("top_reps").and_then(Value::as_i64).unwrap_or(0));
    let reps = |d: i64| if d < 0 { format!("−{}", plural((-d) as usize, "rep")) } else { format!("+{}", plural(d as usize, "rep")) };
    let words = match (now_w, then_w) {
        (Some(w), Some(t)) if (w - t).abs() >= 0.05 => format!("{} on the top set", signed_weight(w - t, unit)),
        (Some(w), Some(_)) if now_r != then_r => format!("{} at {} {unit}", reps(now_r - then_r), trimmed(w)),
        (Some(_), Some(_)) => String::from("the same top set"),
        (None, None) if now_r != then_r => format!("{} on the best set", reps(now_r - then_r)),
        (None, None) => String::from("the same reps"),
        (_, Some(t)) => format!("{} {unit} × {then_r} then", trimmed(t)),
        (Some(_), None) => format!("× {then_r} then"),
    };
    Some(format!("vs {day}: {words}"))
}

/// One set: its number among the working sets (W for a warm-up), weight × reps, its marks, and
/// its estimated max.
fn set_row(number: usize, s: &Value, unit: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.add_css_class("avex-set");
    let kind = text(s, "kind");
    let warm = kind == "warmup";
    let n = label(&if warm { String::from("W") } else { number.to_string() }, "arbiter-step-number");
    n.set_xalign(0.5);
    n.set_valign(gtk::Align::Center);
    row.append(&n);
    let words = label(&set_words(&s["weight"], &s["reps"], unit), "arbiter-stat");
    if warm {
        words.add_css_class("money-muted");
    }
    words.set_hexpand(true);
    words.set_xalign(0.0);
    row.append(&words);
    let mut marks = Vec::new();
    if !warm && kind != "work" {
        marks.push(word(kind));
    }
    if let Some(rpe) = num(&s["rpe"]) {
        marks.push(format!("RPE {}", trimmed(rpe)));
    }
    if s["amrap"] == true {
        marks.push(String::from("AMRAP"));
    }
    if s["to_failure"] == true {
        marks.push(String::from("to failure"));
    }
    if s["assisted"] == true {
        marks.push(String::from("assisted"));
    }
    if !marks.is_empty() {
        row.append(&label(&marks.join(" · "), "money-row-detail"));
    }
    if let Some(e) = num(&s["e1rm"]) {
        let l = label(&format!("~{} {unit}", trimmed(e)), "tally-figures");
        l.set_width_chars(10);
        l.set_xalign(1.0);
        if s["best"] == true {
            l.add_css_class("avex-best");
            l.set_tooltip_text(Some(&format!("The best set. {EST_MAX}")));
        } else {
            l.set_tooltip_text(Some(EST_MAX));
        }
        row.append(&l);
    }
    row
}

#[derive(Clone, Copy, PartialEq)]
enum Trend {
    Up,
    Holding,
    Slipping,
}

/// Up or down more than 1% on the four weeks before, else holding; a lift with nothing to compare
/// that has not been done in six weeks is slipping too.
fn trend(l: &Value, as_of: &str) -> Trend {
    match num(&l["change_pct"]) {
        Some(c) if c > 1.0 => Trend::Up,
        Some(c) if c < -1.0 => Trend::Slipping,
        Some(_) => Trend::Holding,
        None if days_between(text(l, "last_day"), as_of).is_some_and(|d| d > DROPPED_DAYS) => Trend::Slipping,
        None => Trend::Holding,
    }
}

/// Lifts: moving up, holding and slipping, each done most first (the moves biggest first).
fn lifts_page(ui: &Rc<Ui>, page: &Page, l: &Value, age: Option<&Age>) {
    let unit = text(l, "unit").to_string();
    let list = rows(l, "lifts");
    let mut context = format!("{} · {EST_MAX}. Each is the best of the last four weeks, against the four before.", plural(list.len(), "lift"));
    if let Some(a) = age.filter(|a| a.stale) {
        context.push_str(&format!(" Counted to {}, your last export.", day_words(&a.day)));
    }
    heading(&page.head, "Lifts", &context, &[]);
    clear(&page.body);
    if list.is_empty() {
        message(&page.body, "No lifts yet. Import Avex's training history on the Overview.");
        return;
    }
    let day = as_of_day(age);
    let change = |l: &Value| num(&l["change_pct"]).unwrap_or(0.0);
    let mut asked = false;
    for (which, title) in [(Trend::Up, "Moving up"), (Trend::Holding, "Holding"), (Trend::Slipping, "Slipping")] {
        let mut group: Vec<&Value> = list.iter().filter(|lift| trend(lift, &day) == which).collect();
        if group.is_empty() {
            continue;
        }
        match which {
            Trend::Up => group.sort_by(|a, b| change(b).total_cmp(&change(a))),
            Trend::Slipping => group.sort_by(|a, b| change(a).total_cmp(&change(b))),
            Trend::Holding => {}
        }
        let ask = (!asked).then_some((ui, "Which of my lifts are progressing and which have stalled?"));
        asked = true;
        let rows = card_asking(&page.body, title, None, ask);
        for lift in group {
            rows.append(&lift_row(ui, lift, &unit));
        }
    }
}

/// The least-squares slope of the estimated max over the eight weeks before the last point, a
/// week at a time; `None` with fewer than three points there.
fn weekly_slope(points: &[(String, f64, bool)]) -> Option<f64> {
    let last = &points.last()?.0;
    let xy: Vec<(f64, f64)> = points.iter().filter_map(|(day, v, _)| Some((-(days_between(day, last)? as f64), *v))).filter(|(x, _)| *x >= -56.0).collect();
    if xy.len() < 3 {
        return None;
    }
    let n = xy.len() as f64;
    let (mx, my) = (xy.iter().map(|p| p.0).sum::<f64>() / n, xy.iter().map(|p| p.1).sum::<f64>() / n);
    let sxx: f64 = xy.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = xy.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    (sxx > 0.0).then(|| sxy / sxx * 7.0)
}

/// What the lift has done, in a sentence or two: "Up 4.9% in four weeks, about +2 lb a week.
/// Last record Mon 5 Oct."
fn trend_sentence(row: &Value, points: &[(String, f64, bool)], unit: &str, as_of: &str) -> String {
    let last_day = text(row, "last_day");
    let first = match num(&row["change_pct"]) {
        Some(c) if c > 1.0 => format!("Up {:.1}% in four weeks", c),
        Some(c) if c < -1.0 => format!("Down {:.1}% in four weeks", -c),
        Some(_) => String::from("Holding steady over four weeks"),
        None if row["best"].is_null() => {
            return match row["best_reps"].as_i64() {
                Some(r) => format!("Done without weight: at best {} in one set.", plural(r.max(0) as usize, "rep")),
                None => String::from("No set to read an estimated max from yet."),
            };
        }
        None if days_between(last_day, as_of).is_some_and(|d| d > 28) => format!("Not done in the last four weeks; last {}", human_date(last_day)),
        None => String::from("Not enough weeks yet to compare"),
    };
    let mut out = first;
    if let Some(w) = weekly_slope(points).map(|w| (w * 2.0).round() / 2.0).filter(|w| *w != 0.0) {
        out.push_str(&format!(", about {} a week", signed_weight(w, unit)));
    }
    out.push('.');
    if let (Some((record, _, _)), Some((newest, _, _))) = (points.iter().rev().find(|p| p.2), points.last()) {
        match days_between(record, newest) {
            Some(gap) if gap > 35 => out.push_str(&format!(" No new record in {} weeks; the last was {}.", gap / 7, human_date(record))),
            _ => out.push_str(&format!(" Last record {}.", human_date(record))),
        }
    }
    out
}

/// One lift: what it has done in words, its best and its four weeks, its estimated max over time,
/// and every session.
fn lift_page(ui: &Rc<Ui>, page: &Page, d: &Value, age: Option<&Age>) {
    let unit = text(d, "unit").to_string();
    let row = &d["row"];
    let name = text(row, "name").to_string();
    heading(
        &page.head,
        &name,
        &format!("{} · {} sets · last {}", plural(row["sessions"].as_i64().unwrap_or(0).max(0) as usize, "workout"), row["sets"].as_i64().unwrap_or(0), human_date(text(row, "last_day"))),
        &[],
    );
    back_key(ui, &page.head, "Lifts", "avex-lifts");
    clear(&page.body);
    let body = &page.body;
    let history = rows(d, "history");
    let mut points: Vec<(String, f64, bool)> = history.iter().filter_map(|p| Some((text(p, "day").to_string(), num(&p["e1rm"])?, p["record"] == true))).collect();
    points.reverse();
    let day = as_of_day(age);
    let words = trend_sentence(row, &points, &unit, &day);
    let sentence = label(&words, "avex-trend");
    sentence.set_wrap(true);
    sentence.set_xalign(0.0);
    body.append(&sentence);

    let (_, cells) = strip(body);
    let best = &row["best"];
    if best.is_object() {
        cell(&cells, "chart", "Best set", &set_words(&best["weight"], &best["reps"], &unit), "", None);
        match num(&best["e1rm"]) {
            Some(e) => cell(&cells, "spark", "Best est. max", &format!("{} {unit}", trimmed(e)), &human_date(text(best, "day")), None),
            None => cell(&cells, "spark", "Best est. max", "—", "No set of 1 to 12 reps", None),
        }
        if let Some(c) = cells.last_child() {
            c.set_tooltip_text(Some(EST_MAX));
        }
        let change = num(&row["change_pct"]);
        let sub = match (change, num(&row["recent_e1rm"])) {
            (Some(c), Some(e)) => format!("From est. max {} {unit} the four weeks before", trimmed(e / (1.0 + c / 100.0))),
            (None, Some(_)) => String::from("Nothing in the four weeks before"),
            _ => String::from("Not done in the last four weeks"),
        };
        cell(&cells, "calendar", "Four-week change", &change.map_or_else(|| String::from("—"), pct), &sub, None);
    } else {
        let reps = row["best_reps"].as_i64().map_or_else(|| String::from("—"), |r| plural(r.max(0) as usize, "rep"));
        cell(&cells, "chart", "Best set", &reps, "Without weight", None);
    }

    if points.len() >= 2 {
        let stalled = trend(row, &day) == Trend::Slipping || words.contains("No new record");
        let question = if stalled { format!("How do I break this plateau on {name}?") } else { format!("How is my {name} progressing, and what should I change?") };
        let rows = card_asking(body, "Estimated max over time", None, Some((ui, &question)));
        let b = block(&rows);
        b.append(&progress_line(points, unit.clone(), 150));
        let l = label("Each workout's best set as an estimated one-rep max; records in lime.", "money-row-detail");
        l.set_wrap(true);
        l.set_tooltip_text(Some(EST_MAX));
        b.append(&l);
    }
    let rows = card(body, "Every workout", None);
    for p in &history {
        let mut detail = vec![format!("{} sets", p["sets"].as_i64().unwrap_or(0))];
        // Sets without weight move none.
        if num(&p["volume"]).is_some_and(|v| v > 0.0) {
            detail.push(volume(&p["volume"], &unit));
        }
        if let Some(e) = num(&p["e1rm"]) {
            detail.push(format!("est. max {} {unit}", trimmed(e)));
        }
        let top = if p["top_weight"].is_null() { plural(p["top_reps"].as_i64().unwrap_or(0).max(0) as usize, "rep") } else { set_words(&p["top_weight"], &p["top_reps"], &unit) };
        let mut title = human_date(text(p, "day"));
        if p["record"] == true {
            title.push_str(" · record");
        }
        let line = icon_row(&mark(if p["record"] == true { "record" } else { "lift" }), &title, &detail.join(" · "), &top, if p["record"] == true { "money-in" } else { "" });
        rows.append(&row_key(&line, "Open this workout", open_workout_action(ui, p["session_id"].as_i64().unwrap_or(0))));
    }
}

/// Choose Avex's export and import it, replacing what is here.
pub(super) fn import(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Avex training history (.json)"));
        filter.add_pattern("*.json");
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder().title("Import Avex's training history").filters(&filters).default_filter(&filter).build();
        let Ok(file) = dialog.open_future(Some(&ui.window)).await else { return };
        let Some(path) = file.path() else {
            ui.show_error("That file is not on this PC's disk. Copy it here first.");
            return;
        };
        match ui.call("gym.import", json!({"path": path.to_string_lossy()})).await {
            // The set count is left out: it counts warm-ups and untracked workouts, which the
            // pages leave out, so it would disagree with them.
            Ok(v) => toast(
                &ui,
                &format!(
                    "Imported {} from Avex, exported {}.",
                    plural(v["sessions"].as_u64().unwrap_or(0) as usize, "workout"),
                    text(&v, "exported_at").get(..10).map_or_else(|| text(&v, "exported_at").to_string(), human_date)
                ),
                None,
            ),
            Err(e) => ui.show_error(&said(&e)),
        }
        ui.refresh_page();
    });
}

// ---- The thread panel ----------------------------------------------------------------------

/// The panel's one reading: the summary, as of the export when the copy is stale, and its age.
pub(super) async fn panel_read(ui: &Rc<Ui>, _tab: &str) -> Result<Value, Error> {
    let (s, age) = summary(ui).await?;
    let age = age.map(|a| json!({"day": a.day, "days": a.days, "stale": a.stale}));
    Ok(json!({"summary": s, "age": age}))
}

pub(super) fn panel_draw(ui: &Rc<Ui>, body: &gtk::Box, _tab: &str, v: &Value) {
    let age = v["age"]["day"].as_str().map(|day| Age { day: day.to_string(), days: v["age"]["days"].as_i64().unwrap_or(0), stale: v["age"]["stale"] == true });
    panel_overview(ui, body, &v["summary"], age.as_ref());
}

/// A titled block of the thread panel, as Tally's panel draws them, with a quiet way into Avex.
fn panel_block(body: &gtk::Box, name: &str, link: Option<(&str, Action)>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.add_css_class("threads-block");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = label(name, "threads-caption");
    title.set_hexpand(true);
    title.set_valign(gtk::Align::Center);
    head.append(&title);
    if let Some((caption, run)) = link {
        let key = button(caption, "money-text-action");
        key.add_css_class("avex-panel-link");
        key.connect_clicked(move |_| run());
        head.append(&key);
    }
    b.append(&head);
    body.append(&b);
    b
}

/// A panel row: a name over a detail, a figure at the end. Clicked, it starts a question about
/// itself in the thread, in the person's words, and leaves it to them to finish.
fn panel_row(ui: &Rc<Ui>, name: &str, detail: &str, figure: &str, figure_class: &str, question: String) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    let n = label(name, "threads-panel-name");
    n.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&n);
    if !detail.is_empty() {
        let d = label(detail, "threads-panel-detail");
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&d);
    }
    row.append(&words);
    if !figure.is_empty() {
        let f = label(figure, "threads-panel-figures");
        if !figure_class.is_empty() {
            f.add_css_class(figure_class);
        }
        f.set_valign(gtk::Align::Center);
        row.append(&f);
    }
    let key = gtk::Button::new();
    key.add_css_class("arbiter-panel-row");
    key.set_child(Some(&row));
    key.set_tooltip_text(Some(&format!("Ask in this thread: \u{201c}{}\u{2026}\u{201d}", question.trim_end_matches([':', ' ']))));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::threads::draft(&ui, &question);
        }
    });
    key
}

/// The panel: one scroll. The copy's week against the twelve before, the last four workouts, the
/// lifts moving most, and the newest records.
fn panel_overview(ui: &Rc<Ui>, body: &gtk::Box, s: &Value, age: Option<&Age>) {
    if s["imported"].is_null() {
        let note = label("Nothing from Avex yet. Export Training history in Avex (Settings, Export) and import it here.", "money-muted");
        note.set_wrap(true);
        body.append(&note);
        let key = import_key(ui, "Import Avex export…", "");
        key.set_halign(gtk::Align::Start);
        body.append(&key);
        return;
    }
    let unit = text(s, "unit").to_string();
    let week = &s["this_week"];
    let stale = age.is_some_and(|a| a.stale);
    let head = gtk::Box::new(gtk::Orientation::Vertical, 6);
    head.add_css_class("threads-summary");
    let when = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    when.append(&icon("avex", 13));
    let line = if stale { week_of(text(week, "start")) } else { format!("This week · exported {}", exported_when(s, age)) };
    let eyebrow = label(&line, "threads-panel-eyebrow");
    eyebrow.set_hexpand(true);
    eyebrow.set_ellipsize(gtk::pango::EllipsizeMode::End);
    when.append(&eyebrow);
    head.append(&when);
    let target = s["imported"]["days_per_week"].as_i64().unwrap_or(0);
    let sessions = week["sessions"].as_i64().unwrap_or(0);
    let of = if target > 0 { format!(" of {target}") } else { String::new() };
    head.append(&label(&format!("{sessions}{of} {}", if sessions == 1 && target == 0 { "workout" } else { "workouts" }), "threads-panel-figure"));
    let mut figures = vec![volume(&week["volume"], &unit), format!("{} in a row", plural(s["streak_weeks"].as_i64().unwrap_or(0).max(0) as usize, "week"))];
    let cardio = week["cardio_minutes"].as_i64().unwrap_or(0);
    if cardio > 0 {
        figures.push(format!("{} cardio", minutes(cardio)));
    }
    head.append(&label(&figures.join(" · "), "money-muted"));
    // A stale copy says how old it is, with the way to a newer one beside it.
    if let Some(a) = age.filter(|a| a.stale) {
        let notice = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        notice.add_css_class("avex-panel-notice");
        let clock = icon("clock", 13);
        clock.add_css_class("avex-notice-mark");
        notice.append(&clock);
        let words = label(&format!("Your last export, {} old", plural(a.days.max(1) as usize, "day")), "threads-panel-detail");
        words.set_hexpand(true);
        words.set_ellipsize(gtk::pango::EllipsizeMode::End);
        notice.append(&words);
        let key = import_key(ui, "Import newer…", "money-text-action");
        key.add_css_class("avex-panel-link");
        notice.append(&key);
        head.append(&notice);
    }
    let weeks = rows(s, "weeks");
    if !weeks.is_empty() {
        let values = weeks.iter().map(|w| num(&w["volume"]).unwrap_or(0.0)).collect();
        let labels = weeks.iter().map(|w| week_of(text(w, "start"))).collect();
        let chart = bars(values, labels, if unit == "kg" { "kg" } else { "lb" }, 56);
        chart.set_margin_top(6);
        head.append(&chart);
    }
    body.append(&head);

    let recent = panel_block(body, "Recent", Some(("All workouts", go(ui, "avex-workouts"))));
    for w in rows(s, "recent").iter().take(4) {
        let lifts: Vec<&str> = w["lifts"].as_array().into_iter().flatten().filter_map(Value::as_str).take(3).collect();
        let detail = format!("{} · {}", human_date(text(w, "day")), lifts.join(", "));
        let question = format!("About {} on {}: ", text(w, "title"), day_words(text(w, "day")));
        recent.append(&panel_row(ui, text(w, "title"), &detail, &volume(&w["volume"], &unit), "", question));
    }

    // The lifts that moved most either way, up or down more than 1% on the four weeks before.
    let mut moving: Vec<Value> = rows(s, "lifts").into_iter().filter(|l| num(&l["change_pct"]).is_some_and(|c| c.abs() > 1.0)).collect();
    moving.sort_by(|a, b| num(&b["change_pct"]).unwrap_or(0.0).abs().total_cmp(&num(&a["change_pct"]).unwrap_or(0.0).abs()));
    if !moving.is_empty() {
        let list = panel_block(body, "Lifts moving", Some(("All lifts", go(ui, "avex-lifts"))));
        for l in moving.iter().take(4) {
            let c = num(&l["change_pct"]).unwrap_or(0.0);
            let detail = num(&l["recent_e1rm"]).map(|e| format!("est. max {} {unit}", trimmed(e))).unwrap_or_default();
            let question = format!("About my {}: ", text(l, "name"));
            list.append(&panel_row(ui, text(l, "name"), &detail, &pct(c), if c > 0.0 { "money-in" } else { "money-ahead" }, question));
        }
    }

    let records = rows(s, "records");
    if !records.is_empty() {
        let list = panel_block(body, "Records", None);
        for r in records.iter().take(3) {
            let question = format!("About my {} record on {}: ", text(r, "lift"), day_words(text(r, "day")));
            list.append(&panel_row(ui, text(r, "lift"), &human_date(text(r, "day")), &set_words(&r["weight"], &r["reps"], &unit), "money-in", question));
        }
    }
}
