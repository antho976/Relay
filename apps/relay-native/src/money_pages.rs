//! Money's pages: Home, Transactions, Plan and Data, drawn after Tally's screens. Investments
//! has its own module (`money_invest.rs`) built from the parts here.
use super::{formatter, message, page, remember_currency, Action, Page};
use crate::app::{button, clear, confirm_inline, label, rows, text, Ui};
use crate::client::Error;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_money::copy;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Tally's twelve category hues (antho976/Tally, `ui/theme/Color.kt`), by stored colour index.
pub const HUES: [&str; 12] = [
    "#7FB27A", "#4FA9A0", "#6A9FD8", "#8C87D9", "#C27BC0", "#D9768E", "#E08A5F", "#D9A441", "#A3A84E", "#C2A585",
    "#8D99A6", "#B8664F",
];

/// Strokes on the 16-unit grid for Money's own glyphs: its page keys and the category icon
/// keys of `relay_money::model::ICON_KEYS` that have an obvious drawing. The rest take a dot.
fn glyph(key: &str) -> Option<&'static str> {
    Some(match key {
        "home" | "apartment" | "hotel" => r##"<path d="M2.5 7.5L8 3l5.5 4.5M4 6.5V13h8V6.5" /><path d="M6.5 13V9.5h3V13" />"##,
        "list" => r##"<path d="M5.5 4h8M5.5 8h8M5.5 12h8" /><circle cx="2.8" cy="4" r=".6" /><circle cx="2.8" cy="8" r=".6" /><circle cx="2.8" cy="12" r=".6" />"##,
        "plan" => r##"<path d="M8 2.5a5.5 5.5 0 1 0 5.5 5.5H8z" /><path d="M10 1.8a5 5 0 0 1 4.2 4.2H10z" />"##,
        "data" => r##"<ellipse cx="8" cy="4" rx="5" ry="1.8" /><path d="M3 4v8c0 1 2.2 1.8 5 1.8s5-.8 5-1.8V4M3 8c0 1 2.2 1.8 5 1.8S13 9 13 8" />"##,
        "coins" | "savings" | "cash" | "invest" | "crypto" => {
            r##"<ellipse cx="6.5" cy="5" rx="4" ry="1.7" /><path d="M2.5 5v3c0 .9 1.8 1.7 4 1.7M2.5 8v3c0 .9 1.8 1.7 4 1.7" /><ellipse cx="10" cy="9.3" rx="3.5" ry="1.5" /><path d="M6.5 9.3v2.9c0 .8 1.6 1.5 3.5 1.5s3.5-.7 3.5-1.5V9.3" />"##
        }
        "cart" | "store" => r##"<path d="M2 3h2l1.5 7h7L14 5H4.5" /><circle cx="6.5" cy="12.8" r="1" /><circle cx="11.5" cy="12.8" r="1" />"##,
        "dining" | "lunch" | "kitchen" => r##"<path d="M5 2v12M3.5 2v3.5a1.5 1.5 0 0 0 3 0V2M11.5 14V2c-1.6.8-2.5 2.6-2.5 5h2.5" />"##,
        "coffee" | "tea" => r##"<path d="M3 6h8v4a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3z" /><path d="M11 7h1a1.5 1.5 0 0 1 0 3h-1M5.5 2.5V4M8.5 2.5V4" />"##,
        "transport" | "car" | "taxi" | "ev" | "carrepair" | "parking" => {
            r##"<path d="M3 11V8l1.5-3.5h7L13 8v3z" /><path d="M3 11v1.5M13 11v1.5M3 8h10" /><circle cx="5.5" cy="9.5" r=".5" /><circle cx="10.5" cy="9.5" r=".5" />"##
        }
        "train" | "subway" => r##"<rect x="4" y="2" width="8" height="9.5" rx="2" /><path d="M4 7h8M6 14l1-2.5M10 14l-1-2.5" />"##,
        "bike" | "scooter" => r##"<circle cx="4" cy="10.5" r="2.5" /><circle cx="12" cy="10.5" r="2.5" /><path d="M4 10.5l2.5-5H10l2 5M6.5 5.5L8.5 10.5h-4" />"##,
        "fuel" => r##"<path d="M3 14V3.5a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1V14M2 14h9M3 7h7M10 6l2 1.5V12a1 1 0 0 0 2 0V6.5L12.5 5" />"##,
        "bolt" => r##"<path d="M9 2L4 9h4l-1 5 5-7H8z" />"##,
        "wifi" | "online" | "cloud" => r##"<path d="M2 6.5a9 9 0 0 1 12 0M4 9a5.5 5.5 0 0 1 8 0M6 11.3a2.5 2.5 0 0 1 4 0" /><circle cx="8" cy="13.2" r=".6" />"##,
        "phone" => r##"<rect x="4.5" y="2" width="7" height="12" rx="1.2" /><path d="M7 12h2" />"##,
        "bag" | "clothes" | "sale" => r##"<path d="M3 5.5h10l-1 8.5H4z" /><path d="M5.5 5.5V5a2.5 2.5 0 0 1 5 0v.5" />"##,
        "health" | "medical" | "hospital" | "wellness" | "therapy" => {
            r##"<path d="M8 13.5S2.5 10.2 2.5 6.3A2.8 2.8 0 0 1 8 5a2.8 2.8 0 0 1 5.5 1.3C13.5 10.2 8 13.5 8 13.5z" />"##
        }
        "medication" | "pharmacy" => r##"<rect x="2" y="5.5" width="12" height="5" rx="2.5" /><path d="M8 5.5v5" />"##,
        "ticket" | "movie" | "theatre" | "party" | "music" | "games" => {
            r##"<path d="M2.5 5h11v2a1 1 0 0 0 0 2v2h-11V9a1 1 0 0 0 0-2z" /><path d="M10 5.5v1M10 7.5v1M10 9.5v1" />"##
        }
        "repeat" => r##"<path d="M3 7V6a2 2 0 0 1 2-2h7.5M10.5 2l2 2-2 2M13 9v1a2 2 0 0 1-2 2H3.5M5.5 14l-2-2 2-2" />"##,
        "flight" | "luggage" | "beach" => r##"<path d="M14 2L2 6.5l4.5 2 2 4.5z" /><path d="M6.5 8.5L14 2" />"##,
        "gift" | "birthday" => {
            r##"<path d="M2.5 6h11v3h-11zM3.5 9v5h9V9M8 6v8" /><path d="M8 6C6.5 6 5 5.5 5 4.2S6.8 2.8 8 6c1.2-3.2 3-2.6 3-1.8S9.5 6 8 6" />"##
        }
        "dots" => {
            r##"<circle cx="3.5" cy="8" r=".9" fill="currentColor" stroke="none" /><circle cx="8" cy="8" r=".9" fill="currentColor" stroke="none" /><circle cx="12.5" cy="8" r=".9" fill="currentColor" stroke="none" />"##
        }
        "work" | "business" => r##"<path d="M2.5 5.5h11v7.5h-11z" /><path d="M6 5.5V4h4v1.5M2.5 9h11" />"##,
        "spark" | "star" | "idea" => r##"<path d="M8 2l1.5 4.5L14 8l-4.5 1.5L8 14l-1.5-4.5L2 8l4.5-1.5z" />"##,
        "refund" => r##"<path d="M5 4L2.5 6.5 5 9" /><path d="M2.5 6.5H10a3.5 3.5 0 0 1 0 7H6" />"##,
        "bank" | "loan" | "tax" | "legal" => r##"<path d="M2 6l6-3.5L14 6zM3.5 6v6M6.5 6v6M9.5 6v6M12.5 6v6M2 13.5h12" />"##,
        "card" | "wallet" | "atm" | "fees" => r##"<rect x="2" y="4" width="12" height="8.5" rx="1" /><path d="M2 7h12M4.5 10.5h2" />"##,
        "school" | "book" => {
            r##"<path d="M2.5 3.5H7a1 1 0 0 1 1 1V13a1.5 1.5 0 0 0-1.5-1.5h-4zM13.5 3.5H9a1 1 0 0 0-1 1V13a1.5 1.5 0 0 1 1.5-1.5h4z" />"##
        }
        "tv" | "computer" => r##"<rect x="2" y="3" width="12" height="8" rx="1" /><path d="M5.5 14h5M8 11v3" />"##,
        "water" => r##"<path d="M8 2.5S4 7 4 9.8a4 4 0 0 0 8 0C12 7 8 2.5 8 2.5z" />"##,
        "heat" => r##"<path d="M8 14a4 4 0 0 1-4-4c0-3 4-4 3-8 3 1.5 5 4.5 5 8a4 4 0 0 1-4 4z" />"##,
        "pets" => {
            r##"<ellipse cx="8" cy="11" rx="3" ry="2.5" /><circle cx="4" cy="7" r="1.2" /><circle cx="6.5" cy="4.3" r="1.2" /><circle cx="9.5" cy="4.3" r="1.2" /><circle cx="12" cy="7" r="1.2" />"##
        }
        "mail" | "delivery" => r##"<rect x="2" y="3.5" width="12" height="9" rx="1" /><path d="M2.5 4.5L8 9l5.5-4.5" />"##,
        "family" | "child" | "baby" | "elderly" => {
            r##"<circle cx="5.5" cy="5" r="2" /><circle cx="11" cy="6" r="1.5" /><path d="M2 13.5a3.5 3.5 0 0 1 7 0M8.5 13.5a2.5 2.5 0 0 1 5 0" />"##
        }
        "charity" | "church" => r##"<path d="M8 13.5S3 10.5 3 7a2.5 2.5 0 0 1 5-1 2.5 2.5 0 0 1 5 1c0 3.5-5 6.5-5 6.5z" /><path d="M2 13.5h12" />"##,
        "insurance" => r##"<path d="M8 2l5 2v3.5c0 3.3-2 5.5-5 6.8-3-1.3-5-3.5-5-6.8V4z" />"##,
        "sport" | "soccer" | "hiking" | "outdoors" | "pool" => r##"<circle cx="8" cy="8" r="5.5" /><path d="M8 5.5l2.4 1.7-.9 2.8h-3l-.9-2.8z" />"##,
        "haircut" | "beauty" => r##"<circle cx="4.5" cy="11.5" r="2" /><circle cx="11.5" cy="11.5" r="2" /><path d="M6 10L12 2.5M10 10L4 2.5" />"##,
        // The Tally pages' own marks: what a figure is, what a row does.
        "calendar" => r##"<rect x="2.5" y="3.5" width="11" height="10" rx="1.5" /><path d="M2.5 6.5h11M5.5 2v3M10.5 2v3" />"##,
        "chart" => r##"<path d="M2.5 13.5h11" /><path d="M3.5 11l3-3.5 2.5 2 4-5" /><path d="M10.5 4.5h2.5V7" />"##,
        "income" | "download" => r##"<path d="M8 2.5v7M5 6.5l3 3 3-3" /><path d="M2.5 10.5v2a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-2" />"##,
        "spend" | "upload" => r##"<path d="M8 9.5v-7M5 5.5l3-3 3 3" /><path d="M2.5 10.5v2a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-2" />"##,
        "gauge" => r##"<path d="M2.5 11a5.5 5.5 0 1 1 11 0" /><path d="M8 11l2.5-3" />"##,
        "trash" => r##"<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 9h5.6l.7-9" />"##,
        "plus" => r##"<path d="M8 3v10M3 8h10" />"##,
        _ => return None,
    })
}

/// A Money glyph, or a dot for an icon key with no drawing.
pub fn glyph_image(key: &str, size: i32) -> gtk::Image {
    crate::icons::from_geometry(
        glyph(key).unwrap_or(r##"<circle cx="8" cy="8" r="3.2" fill="currentColor" stroke="none" />"##),
        size,
        1.5,
    )
}

/// A category's glyph in its hue, on a wash of that hue.
pub fn badge(icon: &str, color: i64) -> gtk::Box {
    badge_sized(icon, color, 34)
}

/// [`badge`] at `size` pixels: 34 on the pages' rows, 28 in compact rows, 24 in the panel.
pub fn badge_sized(icon: &str, color: i64, size: i32) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tile.add_css_class("money-badge");
    tile.add_css_class(&format!("hue-{}", color.rem_euclid(HUES.len() as i64)));
    if size < 34 {
        tile.add_css_class("money-badge-small");
    }
    tile.set_valign(gtk::Align::Center);
    tile.set_halign(gtk::Align::Start);
    tile.set_hexpand(false);
    tile.set_size_request(size, size);
    let image = glyph_image(icon, if size < 30 { 15 } else { 18 });
    image.set_halign(gtk::Align::Center);
    image.set_valign(gtk::Align::Center);
    image.set_hexpand(true);
    tile.append(&image);
    tile
}

/// What a failed read says on the page. An engine without the ledger yet says so plainly.
pub fn unavailable(error: &Error) -> String {
    match error {
        Error::Bus(e) if matches!(e.code.as_str(), "bus.unknown_op" | "bus.not_implemented") => {
            format!("This engine has no money ledger yet ({}). Update Relay's engine to use Money.", e.message)
        }
        e => e.to_string(),
    }
}

pub(super) fn n(v: &Value, key: &str) -> i64 {
    v[key].as_i64().unwrap_or(0)
}

pub(super) fn date(value: &str) -> Option<glib::DateTime> {
    let mut parts = value.split('-').map(|p| p.parse::<i32>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    glib::DateTime::from_local(y, m, d, 0, 0, 0.0).ok()
}

fn ymd(day: &glib::DateTime) -> String {
    day.format("%Y-%m-%d").map(|s| s.to_string()).unwrap_or_default()
}

pub fn today() -> String {
    glib::DateTime::now_local().map(|now| ymd(&now)).unwrap_or_default()
}

/// "Today", "Yesterday", "Tue 6 Oct"; another year adds it.
pub fn human_date(value: &str) -> String {
    let now = glib::DateTime::now_local().ok();
    if value == today() {
        return String::from("Today");
    }
    if now.as_ref().and_then(|n| n.add_days(-1).ok()).is_some_and(|y| ymd(&y) == value) {
        return String::from("Yesterday");
    }
    let Some(day) = date(value) else { return value.to_string() };
    let same_year = now.is_some_and(|n| n.year() == day.year());
    day.format(if same_year { "%a %-d %b" } else { "%a %-d %b %Y" }).map(|s| s.to_string()).unwrap_or_else(|_| value.to_string())
}

/// "October", or "15 Oct to 14 Nov" for a period that does not start on the 1st.
pub(crate) fn period_name(period: &Value) -> String {
    let (Some(start), Some(end)) = (date(text(period, "start")), date(text(period, "end_exclusive"))) else {
        return String::from("This period");
    };
    let last = end.add_days(-1).unwrap_or(end);
    let fmt = |d: &glib::DateTime, f: &str| d.format(f).map(|s| s.to_string()).unwrap_or_default();
    if start.day_of_month() == 1 && last.month() == start.month() {
        let year = glib::DateTime::now_local().map(|n| n.year()).unwrap_or(start.year());
        fmt(&start, if start.year() == year { "%B" } else { "%B %Y" })
    } else {
        format!("{} to {}", fmt(&start, "%-d %b"), fmt(&last, "%-d %b"))
    }
}

/// An enum name as a word: `CHEQUING` reads "Chequing".
pub(super) fn word(value: &str) -> String {
    let lower = value.replace('_', " ").to_lowercase();
    let mut chars = lower.chars();
    chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

pub(super) fn plural(n: usize, one: &str) -> String {
    copy::plural(n as i64, one)
}

/// How a meter's fill reads: Dev's ink, a category's hue, ahead of pace (amber, Dev's waiting),
/// or over budget (red).
#[derive(Clone, Copy)]
pub enum Tone {
    Plain,
    Hue(i64),
    Ahead,
    Over,
}

/// A budget's tone from its pace status, in its category's hue when it has one.
pub fn tone(status: &str, hue: Option<i64>) -> Tone {
    match status {
        "OVER_BUDGET" => Tone::Over,
        "OVER_PACE" => Tone::Ahead,
        _ => hue.map_or(Tone::Plain, Tone::Hue),
    }
}

pub(super) fn rgb(hex: &str) -> (f64, f64, f64) {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xEDE9E2);
    (f64::from((v >> 16) & 0xFF) / 255.0, f64::from((v >> 8) & 0xFF) / 255.0, f64::from(v & 0xFF) / 255.0)
}

/// [`meter`] filled in `tone`.
pub fn meter_in(fraction: f64, tick: Option<f64>, tone: Tone, height: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(height + 6);
    area.set_hexpand(true);
    area.add_css_class("money-meter");
    let fill = match tone {
        Tone::Plain => rgb("#EDE9E2"),
        Tone::Hue(h) => rgb(HUES[h.rem_euclid(HUES.len() as i64) as usize]),
        Tone::Ahead => rgb("#f0a828"),
        Tone::Over => rgb("#f0786f"),
    };
    area.set_draw_func(move |_, cr, width, _| {
        let (w, h, top) = (width as f64, height as f64, 3.0);
        let rounded = |x: f64, w: f64| {
            let r = (h / 2.0).min(w / 2.0);
            cr.new_sub_path();
            cr.arc(x + w - r, top + r, r, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
            cr.arc(x + r, top + r, r, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
            cr.close_path();
        };
        let (r, g, b) = rgb("#2A2826");
        cr.set_source_rgb(r, g, b);
        rounded(0.0, w);
        let _ = cr.fill();
        let filled = (fraction.clamp(0.0, 1.0) * w).max(if fraction > 0.0 { h } else { 0.0 });
        if filled > 0.0 {
            cr.set_source_rgb(fill.0, fill.1, fill.2);
            rounded(0.0, filled);
            let _ = cr.fill();
        }
        if let Some(tick) = tick.filter(|t| (0.0..=1.0).contains(t)) {
            let x = (tick * w).clamp(1.0, w - 1.0);
            let (r, g, b) = rgb("#8C877F");
            cr.set_source_rgb(r, g, b);
            cr.set_line_width(2.0);
            cr.move_to(x, 0.0);
            cr.line_to(x, h + 6.0);
            let _ = cr.stroke();
        }
    });
    area
}

/// A bar cut into `parts`, each a hue (an index into [`HUES`]) and a share of 0 to 1, in order and
/// filling the width between them, 2px apart. A part too small to see keeps a 2px sliver. Its
/// accessible label lists the shares; a caller that knows their names says them instead.
pub(crate) fn allocation_bar(parts: &[(i64, f64)], height: i32) -> gtk::DrawingArea {
    const GAP: f64 = 2.0;
    const SLIVER: f64 = 2.0;
    let area = gtk::DrawingArea::new();
    area.set_content_height(height);
    area.set_hexpand(true);
    area.add_css_class("money-meter");
    area.set_accessible_role(gtk::AccessibleRole::Img);
    let parts: Vec<(i64, f64)> = parts.iter().copied().filter(|(_, share)| *share > 0.0).collect();
    let spoken: Vec<String> = parts.iter().map(|(_, share)| format!("{:.0} percent", share * 100.0)).collect();
    area.update_property(&[gtk::accessible::Property::Label(&format!("Shares: {}", spoken.join(", ")))]);
    area.set_draw_func(move |_, cr, width, _| {
        let (w, h) = (width as f64, height as f64);
        let rounded = |x: f64, w: f64| {
            let r = 4.0_f64.min(h / 2.0).min(w / 2.0);
            cr.new_sub_path();
            cr.arc(x + w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
            cr.arc(x + w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
            cr.arc(x + r, h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
            cr.arc(x + r, r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
            cr.close_path();
        };
        let total: f64 = parts.iter().map(|(_, share)| share).sum();
        if parts.is_empty() || total <= 0.0 {
            let (r, g, b) = rgb("#2A2826");
            cr.set_source_rgb(r, g, b);
            rounded(0.0, w);
            let _ = cr.fill();
            return;
        }
        // Slivers take their 2px first; the parts that can be seen share what is left.
        let room = (w - GAP * (parts.len() - 1) as f64).max(0.0);
        let thin = |share: f64| share / total * room < SLIVER;
        let slivers = parts.iter().filter(|(_, share)| thin(*share)).count() as f64;
        let seen: f64 = parts.iter().filter(|(_, share)| !thin(*share)).map(|(_, share)| share).sum();
        let spare = (room - SLIVER * slivers).max(0.0);
        let mut x = 0.0;
        for (hue, share) in &parts {
            let part = if thin(*share) || seen <= 0.0 { SLIVER } else { share / seen * spare };
            let (r, g, b) = rgb(HUES[hue.rem_euclid(HUES.len() as i64) as usize]);
            cr.set_source_rgb(r, g, b);
            rounded(x, part);
            let _ = cr.fill();
            x += part + GAP;
        }
    });
    area
}

/// A legend's dot in a hue (an index into [`HUES`]).
pub(crate) fn dot(hue: i64) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(8);
    area.set_content_height(8);
    area.set_valign(gtk::Align::Center);
    // The words beside it say what it marks.
    area.set_accessible_role(gtk::AccessibleRole::Presentation);
    let (r, g, b) = rgb(HUES[hue.rem_euclid(HUES.len() as i64) as usize]);
    area.set_draw_func(move |_, cr, width, height| {
        cr.set_source_rgb(r, g, b);
        cr.arc(width as f64 / 2.0, height as f64 / 2.0, 4.0, 0.0, 2.0 * std::f64::consts::PI);
        let _ = cr.fill();
    });
    area
}

/// "+$1,284" or "−$96": a strip's signed figure, in whole units like the rest of it.
pub(super) fn signed_whole(fmt: &relay_money::money::MoneyFormatter, minor: i64) -> String {
    if minor > 0 { format!("+{}", fmt.format_whole(minor)) } else { fmt.format_whole(minor) }
}

/// The figure's class for a pace status: red over budget, amber ahead of pace.
pub fn tone_class(status: &str) -> Option<&'static str> {
    match status {
        "OVER_BUDGET" => Some("money-over"),
        "OVER_PACE" => Some("money-ahead"),
        _ => None,
    }
}

/// The page title, its context line under it.
fn title(head: &gtk::Box, name: &str, context: &str) {
    clear(head);
    head.append(&label(name, "money-title"));
    if !context.is_empty() {
        let line = label(context, "money-context");
        line.set_wrap(true);
        head.append(&line);
    }
}

/// A card: its title, an optional text action on the right, and the box its rows go in.
pub(super) fn card(parent: &gtk::Box, name: &str, action: Option<(&str, Action)>) -> gtk::Box {
    card_asking(parent, name, action, None)
}

/// [`card`] with an Ask about this key in its head, when `ask` is given: it starts a thread with
/// the card's question, in the person's words.
pub(super) fn card_asking(parent: &gtk::Box, name: &str, action: Option<(&str, Action)>, ask: Option<(&Rc<Ui>, &str)>) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("tally-card");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.add_css_class("tally-card-head");
    let caption = label(name, "tally-card-title");
    caption.set_hexpand(true);
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    head.append(&caption);
    if let Some((ui, question)) = ask {
        head.append(&ask_key(ui, question));
    }
    if let Some((text, run)) = action {
        let key = button(text, "money-text-action");
        key.connect_clicked(move |_| run());
        head.append(&key);
    }
    card.append(&head);
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rows.add_css_class("tally-rows");
    card.append(&rows);
    parent.append(&card);
    rows
}

/// A quiet key that opens a thread asking `question`: the bridge from a page to the agent.
fn ask_key(ui: &Rc<Ui>, question: &str) -> gtk::Button {
    let key = button("", "money-text-action");
    key.add_css_class("money-ask");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    let mark = crate::icons::image("claude", 13);
    mark.set_valign(gtk::Align::Center);
    row.append(&mark);
    row.append(&label("Ask about this", ""));
    key.set_child(Some(&row));
    let said = format!("Ask in a thread: \u{201c}{question}\u{201d}");
    key.set_tooltip_text(Some(&said));
    key.update_property(&[gtk::accessible::Property::Label(&said)]);
    let weak = Rc::downgrade(ui);
    let question = question.to_string();
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::threads::ask(&ui, &question);
        }
    });
    key
}

/// How a widget follows the page's width (see [`fit`]): Tally's pages shrink with the window, so
/// their rows of columns and figures stack instead (DESIGN.md, Layout, records the widths).
#[derive(Clone, Copy)]
pub(super) enum Flex {
    /// A row of columns ([`columns`]): stacked below [`STACK_COLUMNS`].
    Columns,
    /// A strip's figures ([`strip`]): stacked below [`STACK_STRIP`], their rules hidden.
    Strip,
    /// A row of keys and fields: stacked below [`STACK_STRIP`].
    Controls,
    /// A table's less needed column: hidden below [`WIDE`].
    Wide,
}

/// The page widths, in pixels of the content area, where Tally's layout changes.
const STACK_COLUMNS: i32 = 980;
const STACK_STRIP: i32 = 760;
const WIDE: i32 = 900;
/// The widest a Tally page's column of content gets: wider, figures drift far from their names.
const MEASURE: i32 = 1200;
/// `.money-page`'s side padding, both sides.
const PAGE_PADDING: i32 = 64;

thread_local! {
    /// The content area's width as last measured; 0 before the first measure, laid out wide.
    static WIDTH: Cell<i32> = const { Cell::new(0) };
    /// The widgets that follow it. Each read builds new ones; the dropped ones fall out.
    static FLEXING: RefCell<Vec<(glib::WeakRef<gtk::Widget>, Flex)>> = const { RefCell::new(Vec::new()) };
}

fn apply(widget: &gtk::Widget, how: Flex, width: i32) {
    let under = |limit: i32| width > 0 && width < limit;
    let orientation = |stacked: bool| if stacked { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal };
    match how {
        Flex::Wide => widget.set_visible(!under(WIDE)),
        Flex::Columns | Flex::Controls => {
            let Some(row) = widget.downcast_ref::<gtk::Box>() else { return };
            let columns = matches!(how, Flex::Columns);
            let stacked = under(if columns { STACK_COLUMNS } else { STACK_STRIP });
            row.set_orientation(orientation(stacked));
            if columns {
                // Stacked, equal heights would leave holes under the shorter cards.
                row.set_homogeneous(!stacked);
            }
        }
        Flex::Strip => {
            let Some(cells) = widget.downcast_ref::<gtk::Box>() else { return };
            let stacked = under(STACK_STRIP);
            cells.set_orientation(orientation(stacked));
            if stacked {
                cells.add_css_class("narrow");
            } else {
                cells.remove_css_class("narrow");
            }
            let mut child = cells.first_child();
            while let Some(part) = child {
                if part.has_css_class("tally-rule") {
                    part.set_visible(!stacked);
                }
                child = part.next_sibling();
            }
        }
    }
}

/// Makes `widget` follow the page's width, from now on.
pub(super) fn flex(widget: &impl IsA<gtk::Widget>, how: Flex) {
    let widget = widget.upcast_ref::<gtk::Widget>();
    apply(widget, how, WIDTH.with(|w| w.get()));
    FLEXING.with(|f| {
        let mut f = f.borrow_mut();
        f.retain(|(w, _)| w.upgrade().is_some());
        f.push((widget.downgrade(), how));
    });
}

/// The content area is `width` wide: `column` (a page's `.money-page`) is centred in at most
/// [`MEASURE`] pixels, and what no longer fits side by side stacks. `money::add_pages` measures.
pub(super) fn fit(column: &gtk::Box, width: i32) {
    let side = ((width - PAGE_PADDING - MEASURE) / 2).max(0);
    column.set_margin_start(side);
    column.set_margin_end(side);
    if WIDTH.with(|w| w.replace(width)) == width {
        return;
    }
    let flexing: Vec<(gtk::Widget, Flex)> = FLEXING.with(|f| {
        let mut f = f.borrow_mut();
        f.retain(|(w, _)| w.upgrade().is_some());
        f.iter().filter_map(|(w, how)| w.upgrade().map(|w| (w, *how))).collect()
    });
    for (widget, how) in &flexing {
        apply(widget, *how, width);
    }
}

/// `n` columns of equal width under `parent`, side by side; stacked on a narrow page.
pub(super) fn columns(parent: &gtk::Box, n: usize) -> Vec<gtk::Box> {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.set_homogeneous(true);
    let columns = (0..n)
        .map(|_| {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 16);
            row.append(&column);
            column
        })
        .collect();
    parent.append(&row);
    flex(&row, Flex::Columns);
    columns
}

/// A summary strip: one card holding a row of figures (see [`cell`]), and the box under them.
pub(super) fn strip(parent: &gtk::Box) -> (gtk::Box, gtk::Box) {
    let strip = gtk::Box::new(gtk::Orientation::Vertical, 14);
    strip.add_css_class("tally-strip");
    let cells = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    cells.add_css_class("tally-cells");
    strip.append(&cells);
    parent.append(&strip);
    flex(&cells, Flex::Strip);
    (strip, cells)
}

/// One figure of a strip: an icon and what it is, the figure, and a line under it.
pub(super) fn cell(cells: &gtk::Box, icon: &str, caption: &str, figure: &str, sub: &str, class: Option<&str>) {
    if cells.first_child().is_some() {
        let rule = gtk::Separator::new(gtk::Orientation::Vertical);
        rule.add_css_class("tally-rule");
        // A strip already stacked keeps its rules hidden (see `apply`).
        rule.set_visible(!cells.has_css_class("narrow"));
        cells.append(&rule);
    }
    let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
    cell.add_css_class("tally-cell");
    cell.set_hexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let image = glyph_image(icon, 14);
    image.add_css_class("tally-cell-icon");
    top.append(&image);
    top.append(&label(caption, "tally-cell-caption"));
    cell.append(&top);
    let f = label(figure, "tally-cell-figure");
    if let Some(class) = class {
        f.add_css_class(class);
    }
    cell.append(&f);
    if !sub.is_empty() {
        let line = label(sub, "tally-cell-sub");
        line.set_wrap(true);
        line.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        cell.append(&line);
    }
    cells.append(&cell);
}

/// A neutral icon tile, for rows that are not a category.
pub fn tile(icon: &str) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tile.add_css_class("tally-tile");
    tile.set_valign(gtk::Align::Center);
    // The glyph centres by expanding; the tile itself must not take the row's spare width.
    tile.set_hexpand(false);
    tile.set_size_request(28, 28);
    let image = glyph_image(icon, 15);
    image.set_halign(gtk::Align::Center);
    image.set_hexpand(true);
    tile.append(&image);
    tile
}

/// A row of a card: a leading tile, a title over a detail, and a figure at the end.
pub(super) fn icon_row(lead: &gtk::Box, title: &str, detail: &str, figure: &str, figure_class: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.append(lead);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let name = label(title, "money-row-title");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    if !detail.is_empty() {
        let d = label(detail, "money-row-detail");
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&d);
    }
    row.append(&words);
    if !figure.is_empty() {
        let f = label(figure, "money-amount");
        if !figure_class.is_empty() {
            f.add_css_class(figure_class);
        }
        f.set_valign(gtk::Align::Center);
        row.append(&f);
    }
    row
}

/// An account type's mark.
fn account_icon(kind: &str) -> &'static str {
    match kind {
        "CASH" => "cash",
        "SAVINGS" => "savings",
        "CREDIT" => "card",
        "INVESTMENT" => "chart",
        _ => "bank",
    }
}

/// A budget in one line: its badge and name, the meter with today's pace, spent of budget, and
/// what is left (or how far over).
fn budget_line(b: &Value, pace_fraction: f64) -> gtk::Box {
    let fmt = formatter();
    let (spent, budget) = (n(b, "spent"), n(b, "budget"));
    let status = text(b, "status");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.append(&badge_sized(text(b, "icon"), n(b, "color"), 28));
    let name = label(text(b, "name"), "money-row-title");
    name.set_width_chars(11);
    name.set_max_width_chars(11);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&name);
    let bar = meter_in(spent as f64 / budget.max(1) as f64, Some(pace_fraction), tone(status, b["color"].as_i64()), 6);
    bar.set_valign(gtk::Align::Center);
    bar.set_tooltip_text(Some("The tick is where an even spend would be today"));
    row.append(&bar);
    let figures = label(&format!("{} / {}", fmt.format_whole(spent), fmt.format_whole(budget)), "tally-figures");
    figures.set_width_chars(13);
    figures.set_xalign(1.0);
    row.append(&figures);
    let left = budget - spent;
    let rest = label(&if left < 0 { format!("{} over", fmt.format_whole(-left)) } else { format!("{} left", fmt.format_whole(left)) }, "tally-left");
    rest.set_width_chars(11);
    rest.set_xalign(1.0);
    if let Some(class) = tone_class(status) {
        rest.add_css_class(class);
    }
    row.append(&rest);
    row
}

/// A card's `line` (a `tally-row`) as a key the width of the card, which runs `run`.
pub(super) fn row_key(line: &gtk::Box, tooltip: &str, run: Action) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("tally-row");
    row.add_css_class("tally-entry");
    let key = button("", "money-row-key");
    key.add_css_class("tally-line-key");
    key.set_hexpand(true);
    line.remove_css_class("tally-row");
    key.set_child(Some(line));
    key.set_tooltip_text(Some(tooltip));
    key.connect_clicked(move |_| run());
    row.append(&key);
    row
}

/// [`budget_line`] as a key: it opens Entries on that category, this period.
fn budget_key(ui: &Rc<Ui>, b: &Value, pace_fraction: f64) -> gtk::Box {
    let weak = Rc::downgrade(ui);
    let category = b["category_id"].as_i64();
    let run: Action = Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            super::set_filters(None, category);
            super::set_period_offset(0);
            ui.navigate("money-transactions");
        }
    });
    row_key(&budget_line(b, pace_fraction), &format!("See this period's {} entries", text(b, "name")), run)
}

fn go(ui: &Rc<Ui>, page: &'static str) -> Action {
    let weak = Rc::downgrade(ui);
    Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            if page == "money-transactions" {
                super::set_period_offset(0);
                super::set_filters(None, None);
            }
            ui.navigate(page);
        }
    })
}

/// Deletes `tx`, with Undo on a toast rather than a confirmation first (Tally's rule). Undo is
/// `money.tx.restore`, which brings back the same row, its sync id included.
pub fn delete(ui: &Rc<Ui>, tx: Value) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match ui.call("money.tx.delete", json!({"id":tx["id"]})).await {
            Ok(_) => {
                let what = match text(&tx, "category") {
                    "" if text(&tx, "type") == "TRANSFER" => String::from("Transfer"),
                    "" => String::from("Entry"),
                    name => name.to_string(),
                };
                let weak = Rc::downgrade(&ui);
                let id = tx["id"].clone();
                let undo: Action = Rc::new(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let id = id.clone();
                    glib::spawn_future_local(async move {
                        if let Err(e) = ui.call("money.tx.restore", json!({"id":id})).await {
                            ui.show_error(&e.to_string());
                        }
                        ui.refresh_page();
                    });
                });
                super::toast(&ui, &format!("{what} {} deleted", formatter().format(n(&tx, "amount"))), Some(undo));
                ui.refresh_page();
            }
            Err(e) => ui.show_error(&e.to_string()),
        }
    });
}

/// One entry as a row: its category badge, what it was (the note, else the category) over
/// where it sits, and the amount; `dated` adds the day. Clicking it edits; the trash key (shown on
/// hover, where `deletable`) deletes with Undo.
fn tx_row(ui: &Rc<Ui>, tx: &Value, deletable: bool, dated: bool) -> gtk::Box {
    let fmt = formatter();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("tally-row");
    row.add_css_class("tally-entry");
    let main = button("", "money-row-key");
    main.set_hexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let kind = text(tx, "type");
    let transfer = kind == "TRANSFER";
    inner.append(&badge_sized(if transfer { "repeat" } else { text(tx, "icon") }, if transfer { 10 } else { n(tx, "color") }, 28));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let category = match text(tx, "category") {
        "" if transfer => String::from("Transfer"),
        "" => word(kind),
        name => name.to_string(),
    };
    let note = text(tx, "note").trim();
    let (what, mut detail) = if note.is_empty() { (category.clone(), Vec::new()) } else { (note.to_string(), vec![category]) };
    if transfer {
        detail.push(format!("{} → {}", text(tx, "account"), text(tx, "to_account")));
    } else {
        detail.push(text(tx, "account").to_string());
    }
    if dated {
        detail.push(human_date(text(tx, "date")));
    }
    let name = label(&what, "money-row-title");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let d = label(&detail.join(" · "), "money-row-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    inner.append(&words);
    let amount = n(tx, "amount");
    let figure = match kind {
        "INCOME" => label(&fmt.format_signed(amount), "money-amount"),
        "EXPENSE" => label(&format!("−{}", fmt.format(amount)), "money-amount"),
        _ => label(&fmt.format(amount), "money-amount"),
    };
    match kind {
        "INCOME" => figure.add_css_class("money-in"),
        "TRANSFER" => figure.add_css_class("money-quiet"),
        _ => {}
    }
    figure.set_valign(gtk::Align::Center);
    inner.append(&figure);
    main.set_child(Some(&inner));
    main.set_tooltip_text(Some("Edit this entry"));
    let weak = Rc::downgrade(ui);
    let edit = tx.clone();
    main.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::edit_entry(&ui, edit.clone());
        }
    });
    row.append(&main);
    if deletable {
        let trash = crate::app::icon_button("trash", "Delete entry");
        trash.add_css_class("money-row-delete");
        trash.set_valign(gtk::Align::Center);
        let weak = Rc::downgrade(ui);
        let gone = tx.clone();
        trash.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                delete(&ui, gone.clone());
            }
        });
        row.append(&trash);
    }
    row
}

/// The Entries header's live parts: its context line, period caption and next key, and the
/// account and category filters with the ids behind their rows (`None` is "All").
struct TxHead {
    context: gtk::Label,
    period: gtk::Label,
    next: gtk::Button,
    accounts: gtk::DropDown,
    categories: gtk::DropDown,
    account_rows: RefCell<Vec<(Option<i64>, String)>>,
    category_rows: RefCell<Vec<(Option<i64>, String)>>,
    /// Set while a read refills the filters, so their change is not taken for the person's.
    filling: Cell<bool>,
}

thread_local! {
    static TX_HEAD: RefCell<Option<Rc<TxHead>>> = const { RefCell::new(None) };
}

/// A filter's drop-down, holding only its "All" row until the first read. Typing finds a row.
fn filter_picker(all: &str, name: &str, spoken: &str) -> gtk::DropDown {
    let picker = gtk::DropDown::from_strings(&[all]);
    picker.set_enable_search(true);
    // Search filters on this expression; without one it matches every row.
    picker.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
    picker.add_css_class("money-picker");
    picker.set_widget_name(name);
    picker.set_valign(gtk::Align::Center);
    picker.update_property(&[gtk::accessible::Property::Label(spoken)]);
    picker
}

/// Builds the headers that keep their widgets across reads (Entries' period keys, filters and
/// search, and Investments' keys), and shows a first message on every page.
pub fn install(ui: &Rc<Ui>) {
    for (name, caption, _) in super::PAGES {
        if let Some(page) = page(name) {
            title(&page.head, caption, "");
            message(&page.body, "Reading the ledger…");
        }
    }
    if let Some(page) = page("money-invest") {
        super::invest::head(ui, &page);
    }
    let Some(page) = page("money-transactions") else { return };
    clear(&page.head);
    page.head.append(&label("Entries", "money-title"));
    let context = label("", "money-context");
    context.set_wrap(true);
    page.head.append(&context);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.add_css_class("money-controls");
    let periods = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    // It takes the row's spare width, so the filters sit at the far end.
    periods.set_hexpand(true);
    let previous = crate::app::icon_button("chevron-left", "Previous period");
    previous.add_css_class("money-chrome");
    previous.set_widget_name("money-period-previous");
    let period = label("", "money-period");
    period.set_width_chars(16);
    period.set_xalign(0.5);
    let next = crate::app::icon_button("chevron-right", "Next period");
    next.add_css_class("money-chrome");
    next.set_widget_name("money-period-next");
    periods.append(&previous);
    periods.append(&period);
    periods.append(&next);
    controls.append(&periods);
    let filters = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let accounts = filter_picker("All accounts", "money-filter-account", "Show the entries of one account");
    let categories = filter_picker("All categories", "money-filter-category", "Show the entries of one category");
    filters.append(&accounts);
    filters.append(&categories);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search notes, categories, accounts"));
    search.set_width_chars(24);
    search.add_css_class("money-search");
    search.set_widget_name("money-search");
    filters.append(&search);
    controls.append(&filters);
    page.head.append(&controls);
    flex(&controls, Flex::Controls);
    flex(&filters, Flex::Controls);
    for (key, step) in [(&previous, -1_i64), (&next, 1)] {
        let weak = Rc::downgrade(ui);
        key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                super::set_period_offset((super::period_offset() + step).min(0));
                ui.refresh_page();
            }
        });
    }
    let weak = Rc::downgrade(ui);
    search.connect_search_changed(move |entry| {
        if let Some(ui) = weak.upgrade() {
            super::set_query(entry.text().trim());
            ui.refresh_page();
        }
    });
    let head = Rc::new(TxHead {
        context,
        period,
        next,
        accounts,
        categories,
        account_rows: RefCell::new(vec![(None, String::from("All accounts"))]),
        category_rows: RefCell::new(vec![(None, String::from("All categories"))]),
        filling: Cell::new(false),
    });
    for picker in [&head.accounts, &head.categories] {
        let (weak, read) = (Rc::downgrade(ui), Rc::downgrade(&head));
        picker.connect_selected_notify(move |_| {
            let (Some(ui), Some(head)) = (weak.upgrade(), read.upgrade()) else { return };
            if head.filling.get() {
                return;
            }
            let pick = |picker: &gtk::DropDown, rows: &RefCell<Vec<(Option<i64>, String)>>| rows.borrow().get(picker.selected() as usize).and_then(|row| row.0);
            super::set_filters(pick(&head.accounts, &head.account_rows), pick(&head.categories, &head.category_rows));
            ui.refresh_page();
        });
    }
    TX_HEAD.with(|h| *h.borrow_mut() = Some(head));
}

/// Refills the Entries filters from `money.lists`, keeping the person's choice selected. An
/// archived account or category stays listed while it is the one chosen. A chosen one the ledger
/// no longer has (a restore or a sync took it away) is dropped, and this says so: the entries were
/// read through it, and are read again.
fn fill_filters(lists: &Value) -> bool {
    let Some(head) = TX_HEAD.with(|h| h.borrow().clone()) else { return false };
    let (account, category) = super::filters();
    let listed = |key: &str, all: &str, chosen: Option<i64>| -> Vec<(Option<i64>, String)> {
        let mut items = vec![(None, all.to_string())];
        items.extend(
            rows(lists, key)
                .iter()
                .filter(|v| v["archived"] != true || v["id"].as_i64() == chosen)
                .map(|v| (v["id"].as_i64(), text(v, "name").to_string())),
        );
        items
    };
    head.filling.set(true);
    let mut gone = false;
    for (picker, rows, items, chosen) in [
        (&head.accounts, &head.account_rows, listed("accounts", "All accounts", account), account),
        (&head.categories, &head.category_rows, listed("categories", "All categories", category), category),
    ] {
        if *rows.borrow() != items {
            let names: Vec<&str> = items.iter().map(|(_, name)| name.as_str()).collect();
            picker.set_model(Some(&gtk::StringList::new(&names)));
            *rows.borrow_mut() = items;
        }
        let found = rows.borrow().iter().position(|row| row.0 == chosen);
        gone |= found.is_none();
        let position = found.unwrap_or(0) as u32;
        if picker.selected() != position {
            picker.set_selected(position);
        }
    }
    head.filling.set(false);
    if gone {
        let kept = |rows: &RefCell<Vec<(Option<i64>, String)>>, id: Option<i64>| id.filter(|id| rows.borrow().iter().any(|row| row.0 == Some(*id)));
        super::set_filters(kept(&head.account_rows, account), kept(&head.category_rows, category));
    }
    gone
}

/// What the Entries filters show, as words for the context line: "Dining · Visa".
fn filter_words() -> String {
    let Some(head) = TX_HEAD.with(|h| h.borrow().clone()) else { return String::new() };
    let (account, category) = super::filters();
    let name = |rows: &RefCell<Vec<(Option<i64>, String)>>, id: Option<i64>| id.and_then(|id| rows.borrow().iter().find(|row| row.0 == Some(id)).map(|row| row.1.clone()));
    [name(&head.category_rows, category), name(&head.account_rows, account)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// What an Entries read was for: its period, its search and its (account, category) filters.
type TxRead = (i64, String, (Option<i64>, Option<i64>));

thread_local! {
    /// The last Entries read whose `money.lists` answer is still good: the ledger has not moved
    /// since.
    static TX_READ: RefCell<Option<TxRead>> = const { RefCell::new(None) };
}

/// The ledger moved (`money.changed`): Entries reads its accounts and categories again.
pub fn forget_lists() {
    TX_READ.with(|r| *r.borrow_mut() = None);
}

pub async fn refresh(ui: &Rc<Ui>, name: &str) {
    let Some(page) = page(name) else { return };
    match name {
        "money-home" => {
            let result = ui.call("money.summary", json!({})).await;
            if *ui.page.borrow() != name {
                return;
            }
            match result {
                Ok(s) => home(ui, &page, &s),
                Err(e) => failed(ui, &page, "Home", &e),
            }
        }
        "money-transactions" => {
            let (offset, query, filters) = (super::period_offset(), super::query(), super::filters());
            let mut payload = json!({"period_offset":offset});
            if !query.is_empty() {
                payload["query"] = json!(query);
            }
            if let Some(id) = filters.0 {
                payload["account_id"] = json!(id);
            }
            if let Some(id) = filters.1 {
                payload["category_id"] = json!(id);
            }
            // A search typed since the last read changes only the entries: the accounts and
            // categories it read stand until the ledger moves (`forget_lists`).
            let searched = TX_READ.with(|r| r.borrow().as_ref().is_some_and(|(o, q, f)| (*o, f) == (offset, &filters) && *q != query));
            let mut lists_read = searched;
            let list = if searched {
                ui.call("money.tx.list", payload).await
            } else {
                let (lists, list) = tokio::join!(ui.call("money.lists", json!({})), ui.call("money.tx.list", payload));
                if *ui.page.borrow() != name || super::period_offset() != offset || super::query() != query || super::filters() != filters {
                    return;
                }
                if let Ok(lists) = &lists {
                    remember_currency(lists);
                    if fill_filters(lists) {
                        ui.refresh_page();
                        return;
                    }
                    lists_read = true;
                }
                list
            };
            if *ui.page.borrow() != name || super::period_offset() != offset || super::query() != query || super::filters() != filters {
                return;
            }
            // Only a read whose accounts and categories stand (both read, or kept) is one a
            // search may build on.
            TX_READ.with(|r| *r.borrow_mut() = (lists_read && list.is_ok()).then(|| (offset, query.clone(), filters)));
            match list {
                Ok(list) => transactions(ui, &page, &list, offset, &query),
                Err(e) => failed(ui, &page, "", &e),
            }
        }
        "money-invest" => {
            let (summary, activity, lists) = tokio::join!(
                ui.call("money.invest.summary", json!({})),
                ui.call("money.invest.list", json!({"limit":8})),
                ui.call("money.lists", json!({}))
            );
            if *ui.page.borrow() != name {
                return;
            }
            match summary {
                Ok(s) => super::invest::draw(ui, &page, &s, activity.as_ref().ok(), lists.as_ref().ok()),
                Err(e) => super::invest::failed(ui, &page, &e),
            }
        }
        "money-plan" => {
            let (summary, lists) = tokio::join!(ui.call("money.summary", json!({})), ui.call("money.lists", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match (summary, lists) {
                (Ok(s), Ok(l)) => plan(ui, &page, &s, &l),
                (Err(e), _) | (_, Err(e)) => failed(ui, &page, "Plan", &e),
            }
        }
        "money-data" => {
            let (summary, lists) = tokio::join!(ui.call("money.summary", json!({})), ui.call("money.lists", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match &summary {
                Ok(_) => data(ui, &page, summary.as_ref().ok(), lists.as_ref().ok()),
                Err(e) => failed(ui, &page, "Data", e),
            }
        }
        _ => {}
    }
}

pub(super) fn failed(ui: &Rc<Ui>, page: &Page, name: &str, error: &Error) {
    if !name.is_empty() {
        title(&page.head, name, "");
    }
    message(&page.body, &unavailable(error));
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

/// Overview: the period in one strip (left to spend, spent, income, net worth, and the pace), then
/// budgets beside recent entries, then bills, accounts and goals side by side: one screen.
fn home(ui: &Rc<Ui>, page: &Page, s: &Value) {
    remember_currency(s);
    let fmt = formatter();
    let period = &s["period"];
    let (days, days_left) = (n(period, "days"), n(period, "days_left"));
    let resets = date(text(period, "end_exclusive")).and_then(|d| d.format("%-d %b").ok()).map(|d| format!("resets {d}")).unwrap_or_default();
    let context = [format!("Day {} of {days}", (days - days_left + 1).clamp(1, days.max(1))), copy::plural(days_left, "day") + " left", resets];
    title(&page.head, &period_name(period), &context.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · "));
    clear(&page.body);
    if s["empty"] == true {
        empty_state(ui, &page.body);
        return;
    }
    let pace = &s["pace"];
    let lines = &s["lines"];
    let status = text(pace, "status");
    let budgeted = status != "NO_BUDGET" && n(pace, "budget") > 0;

    // The strip: four figures, then the pace across the period.
    let (strip, cells) = strip(&page.body);
    if budgeted {
        let remaining = n(pace, "remaining");
        let sub = [format!("of {}", fmt.format_whole(n(pace, "budget"))), text(lines, "margin").to_string()];
        let class = if remaining < 0 { Some("money-over") } else { None };
        cell(&cells, "card", "Left to spend", &fmt.format_whole(remaining), &sub.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · "), class);
    } else {
        cell(&cells, "card", "Left to spend", "No budget", "Set one in Plan to see your pace", None);
    }
    cell(&cells, "spend", "Spent", &fmt.format_whole(n(s, "spent")), text(lines, "versus_last"), None);
    let income = n(s, "income");
    cell(&cells, "income", "Income", &fmt.format_whole(income), "this period", (income > 0).then_some("money-in"));
    let accounts = rows(s, "accounts");
    let worth = n(s, "net_worth");
    cell(&cells, "bank", "Net worth", &fmt.format_whole(worth), &plural(accounts.len(), "account"), (worth < 0).then_some("money-over"));
    if budgeted {
        let pace_row = gtk::Box::new(gtk::Orientation::Vertical, 6);
        pace_row.add_css_class("tally-pace");
        let bar = meter_in(pace["spent_fraction"].as_f64().unwrap_or(0.0), pace["pace_fraction"].as_f64(), tone(status, None), 8);
        bar.set_tooltip_text(Some("The tick is where an even spend would be today"));
        pace_row.append(&bar);
        let legend = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let said = label(text(lines, "pace"), "tally-cell-sub");
        said.set_hexpand(true);
        said.set_ellipsize(gtk::pango::EllipsizeMode::End);
        if let Some(class) = tone_class(status) {
            said.add_css_class(class);
        }
        legend.append(&said);
        let spent_share = (pace["spent_fraction"].as_f64().unwrap_or(0.0) * 100.0).round();
        let time_share = (pace["pace_fraction"].as_f64().unwrap_or(0.0) * 100.0).round();
        legend.append(&label(&format!("{spent_share:.0}% spent · {time_share:.0}% of the period gone"), "tally-figures"));
        pace_row.append(&legend);
        strip.append(&pace_row);
    }

    // Budgets beside recent entries.
    let pace_fraction = pace["pace_fraction"].as_f64().unwrap_or(0.0);
    let mut budgets: Vec<Value> = rows(s, "budgets").into_iter().filter(|b| n(b, "budget") > 0).collect();
    let ahead = |b: &Value| n(b, "spent") as f64 / n(b, "budget").max(1) as f64 - pace_fraction;
    budgets.sort_by(|a, b| ahead(b).total_cmp(&ahead(a)));
    let pair = columns(&page.body, 2);
    let ask = (!budgets.is_empty()).then_some((ui, "Which of my budgets are ahead of pace this month, and why?"));
    let list = card_asking(&pair[0], "Budgets", Some(("Plan", go(ui, "money-plan"))), ask);
    if budgets.is_empty() {
        list.append(&icon_row(&tile("plan"), "No budgets yet", "Give each category a monthly budget in Plan", "", ""));
    }
    for b in budgets.iter().take(8) {
        list.append(&budget_key(ui, b, pace_fraction));
    }
    let bills = rows(s, "bills");
    if !bills.is_empty() {
        let list = card(&pair[0], "Upcoming bills", None);
        for bill in bills.iter().take(5) {
            let amount = n(bill, "amount");
            let income = text(bill, "type") == "INCOME";
            let (figure, class) = if income { (fmt.format_signed(amount), "money-in") } else { (format!("−{}", fmt.format(amount)), "") };
            let detail = format!("{} · {}", text(bill, "due_line"), human_date(text(bill, "next_date")));
            let row = icon_row(&tile(if income { "income" } else { "calendar" }), text(bill, "name"), &detail, &figure, class);
            if n(bill, "days_until") <= 3 && !income {
                row.add_css_class("tally-soon");
            }
            list.append(&row);
        }
    }
    let recent = rows(s, "recent");
    let list = card(&pair[1], "Recent", Some(("All entries", go(ui, "money-transactions"))));
    if recent.is_empty() {
        list.append(&icon_row(&tile("list"), "Nothing logged this period yet", "Add an entry with the Entry key above", "", ""));
    }
    for tx in recent.iter().take(8) {
        list.append(&tx_row(ui, tx, false, true));
    }

    // Accounts and goals, those there are, side by side.
    let goals = rows(s, "goals");
    let count = usize::from(!accounts.is_empty()) + usize::from(!goals.is_empty());
    if count == 0 {
        return;
    }
    let mut slots = columns(&page.body, count).into_iter();
    if !accounts.is_empty() {
        let investing = accounts.iter().any(|a| text(a, "type") == "INVESTMENT");
        let list = card(&slots.next().expect("counted"), "Accounts", investing.then(|| ("Investments", go(ui, "money-invest"))));
        for account in &accounts {
            let balance = n(account, "balance");
            let row = icon_row(
                &tile(account_icon(text(account, "type"))),
                text(account, "name"),
                &word(text(account, "type")),
                &fmt.format(balance),
                if balance < 0 { "money-over" } else { "" },
            );
            if text(account, "type") != "INVESTMENT" {
                list.append(&row);
                continue;
            }
            // An investment account is worth what its statement says: a click records that.
            let weak = Rc::downgrade(ui);
            let held = account.clone();
            let run: Action = Rc::new(move || {
                if let Some(ui) = weak.upgrade() {
                    super::record_value(&ui, &held);
                }
            });
            list.append(&row_key(&row, &format!("Record what {} is worth", text(account, "name")), run));
        }
        let total = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        total.add_css_class("tally-total");
        let caption = label("Net worth", "money-row-title");
        caption.set_hexpand(true);
        total.append(&caption);
        total.append(&label(&fmt.format(worth), "tally-total-figure"));
        list.append(&total);
    }
    if !goals.is_empty() {
        let list = card(&slots.next().expect("counted"), "Goals", None);
        for goal in &goals {
            let block = gtk::Box::new(gtk::Orientation::Vertical, 6);
            block.add_css_class("tally-row");
            let (saved, target) = (n(goal, "saved"), n(goal, "target"));
            let top = icon_row(&tile("spark"), text(goal, "name"), text(goal, "line"), &format!("{} / {}", fmt.format_whole(saved), fmt.format_whole(target)), "");
            top.remove_css_class("tally-row");
            block.append(&top);
            if target > 0 {
                block.append(&meter_in(saved as f64 / target as f64, None, Tone::Hue(1), 6));
            }
            list.append(&block);
        }
    }
}

/// An empty ledger says so, and offers the three ways to fill it.
fn empty_state(ui: &Rc<Ui>, body: &gtk::Box) {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 14);
    block.add_css_class("tally-card");
    block.add_css_class("money-empty");
    block.append(&label("Nothing here yet", "money-empty-title"));
    let about = label(
        "Bring over what Tally holds on your phone with its backup file, start fresh with an account, or load a sample household to look around first.",
        "money-muted",
    );
    about.set_wrap(true);
    about.set_max_width_chars(70);
    block.append(&about);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let import_key = button("Import a Tally backup…", "primary");
    import_key.add_css_class("money-hero-action");
    import_key.set_widget_name("money-empty-import");
    let account = button("Add an account…", "");
    account.set_widget_name("money-empty-account");
    let sample = button("Load sample data", "");
    sample.set_widget_name("money-empty-sample");
    actions.append(&import_key);
    actions.append(&account);
    actions.append(&sample);
    let weak = Rc::downgrade(ui);
    account.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::add_account(&ui);
        }
    });
    block.append(&actions);
    body.append(&block);
    let weak = Rc::downgrade(ui);
    import_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import(&ui, true);
        }
    });
    let weak = Rc::downgrade(ui);
    sample.connect_clicked(move |key| {
        if let Some(ui) = weak.upgrade() {
            load_sample(&ui, key);
        }
    });
}

/// What Entries drew last, so a read that changed little changes little: its strip and what the
/// strip says, its card of days with its question, and each row of that card by what it shows.
struct TxDrawn {
    strip: gtk::Widget,
    strip_key: u64,
    card: gtk::Widget,
    rows: gtk::Box,
    question: String,
    parts: std::collections::HashMap<u64, gtk::Widget>,
}

thread_local! {
    static TX_DRAWN: RefCell<Option<TxDrawn>> = const { RefCell::new(None) };
}

/// A key for what a row shows: equal keys draw the same row.
fn shown_key(parts: &[&dyn std::fmt::Display]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for part in parts {
        part.to_string().hash(&mut hasher);
    }
    hasher.finish()
}

/// Entries' strip: how many, in, out and net.
fn tx_strip(body: &gtk::Box, count: usize, name: &str, income: i64, spent: i64) -> gtk::Box {
    let fmt = formatter();
    let (strip, cells) = strip(body);
    cell(&cells, "list", "Entries", &count.to_string(), name, None);
    cell(&cells, "income", "Money in", &fmt.format_whole(income), "income", (income > 0).then_some("money-in"));
    cell(&cells, "spend", "Money out", &fmt.format_whole(spent), "spending", None);
    let net = income - spent;
    cell(&cells, "gauge", "Net", &signed_whole(&fmt, net), "in less out", Some(if net < 0 { "money-over" } else { "money-in" }));
    strip
}

/// A day's head in Entries: its name, how many entries, and its net.
fn day_head(day: &str, count: usize, day_net: i64) -> gtk::Box {
    let fmt = formatter();
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("tally-day");
    let when = label(&human_date(day), "tally-day-name");
    when.set_hexpand(true);
    header.append(&when);
    header.append(&label(&copy::plural_as(count as i64, "entry", "entries"), "tally-figures"));
    let total = label(&fmt.format_signed(day_net), "tally-day-total");
    total.set_xalign(1.0);
    if day_net > 0 {
        total.add_css_class("money-in");
    }
    header.append(&total);
    header
}

/// Entries: the period in a strip (count, in, out, net), then one card of days, each with its
/// net and its entries. A read like the last one keeps its widgets: a row stays while what it
/// shows does, and only rows that came, went or changed are touched.
fn transactions(ui: &Rc<Ui>, page: &Page, list: &Value, offset: i64, query: &str) {
    let entries = rows(list, "transactions");
    let name = period_name(&list["period"]);
    let count = copy::plural_as(entries.len() as i64, "entry", "entries");
    let filtered = filter_words();
    if let Some(head) = TX_HEAD.with(|h| h.borrow().clone()) {
        let mut context = if query.is_empty() { format!("{count} in {name}") } else { format!("{count} matching \u{201c}{query}\u{201d}") };
        if !filtered.is_empty() {
            context = format!("{context} · {filtered}");
        }
        head.context.set_text(&context);
        head.period.set_text(&name);
        head.next.set_sensitive(offset < 0);
    }
    let (income, spent) = (n(list, "income"), n(list, "spent"));
    let previous = TX_DRAWN.with(|d| d.borrow_mut().take());
    if entries.is_empty() {
        clear(&page.body);
        tx_strip(&page.body, entries.len(), &name, income, spent);
        let text = match (query.is_empty(), filtered.is_empty()) {
            (true, true) => format!("Nothing logged in {name}."),
            (true, false) => format!("Nothing in {name} for {filtered}."),
            (false, _) => format!("Nothing in {name} matches \u{201c}{query}\u{201d}."),
        };
        let rows = card(&page.body, "Entries", None);
        rows.append(&icon_row(&tile("list"), &text, "Add an entry with the Entry key above", "", ""));
        if !filtered.is_empty() {
            let clear_key = button("Show every entry", "money-secondary");
            clear_key.set_widget_name("money-filter-clear");
            clear_key.set_halign(gtk::Align::Start);
            clear_key.add_css_class("tally-save");
            let weak = Rc::downgrade(ui);
            clear_key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    super::set_filters(None, None);
                    ui.refresh_page();
                }
            });
            rows.append(&clear_key);
        }
        return;
    }
    let currency = super::currency();
    let today = today();
    let question = if filtered.is_empty() { format!("Where did my money go in {name}?") } else { format!("What stands out in my {filtered} entries for {name}?") };
    let strip_key = shown_key(&[&entries.len(), &name, &income, &spent, &currency]);
    // The last read's strip and card, while the page still holds just them.
    let body: &gtk::Widget = page.body.upcast_ref();
    let previous = previous.filter(|d| {
        d.question == question
            && body.first_child().as_ref() == Some(&d.strip)
            && d.strip.next_sibling().as_ref() == Some(&d.card)
            && d.card.next_sibling().is_none()
    });
    let (strip, rows_box, card, mut old_parts) = match previous {
        Some(d) => {
            let strip = if d.strip_key == strip_key {
                d.strip
            } else {
                let strip = tx_strip(&page.body, entries.len(), &name, income, spent);
                page.body.reorder_child_after(&strip, None::<&gtk::Widget>);
                page.body.remove(&d.strip);
                strip.upcast()
            };
            (strip, d.rows, d.card, d.parts)
        }
        None => {
            clear(&page.body);
            let strip = tx_strip(&page.body, entries.len(), &name, income, spent);
            let rows_box = card_asking(&page.body, "By day", None, Some((ui, &question)));
            let card = rows_box.parent().expect("card_asking puts its rows in a card");
            (strip.upcast(), rows_box, card, std::collections::HashMap::new())
        }
    };
    let mut parts = std::collections::HashMap::new();
    let mut wanted: Vec<gtk::Widget> = Vec::with_capacity(entries.len() + 16);
    let mut keep = |key: u64, build: &dyn Fn() -> gtk::Widget| {
        // A key seen twice in one read (it cannot be, ids differ) still draws twice.
        let w = if parts.contains_key(&key) { build() } else { old_parts.remove(&key).unwrap_or_else(build) };
        parts.entry(key).or_insert_with(|| w.clone());
        wanted.push(w);
    };
    let mut start = 0;
    while start < entries.len() {
        let day = text(&entries[start], "date").to_string();
        let end = entries[start..].iter().position(|t| text(t, "date") != day).map_or(entries.len(), |p| start + p);
        let day_net: i64 = entries[start..end]
            .iter()
            .map(|t| match text(t, "type") {
                "INCOME" => n(t, "amount"),
                "EXPENSE" => -n(t, "amount"),
                _ => 0,
            })
            .sum();
        let size = end - start;
        keep(shown_key(&[&"day", &day, &size, &day_net, &currency, &today]), &|| day_head(&day, size, day_net).upcast());
        for tx in &entries[start..end] {
            keep(shown_key(&[&"entry", tx, &currency]), &|| tx_row(ui, tx, true, false).upcast());
        }
        start = end;
    }
    let mut previous: Option<gtk::Widget> = None;
    for w in &wanted {
        if w.parent().as_ref() == Some(rows_box.upcast_ref()) {
            if w.prev_sibling() != previous {
                rows_box.reorder_child_after(w, previous.as_ref());
            }
        } else {
            rows_box.insert_child_after(w, previous.as_ref());
        }
        previous = Some(w.clone());
    }
    while let Some(extra) = previous.as_ref().map_or_else(|| rows_box.first_child(), |w| w.next_sibling()) {
        rows_box.remove(&extra);
    }
    TX_DRAWN.with(|d| *d.borrow_mut() = Some(TxDrawn { strip, strip_key, card, rows: rows_box, question, parts }));
}

/// Plan: the monthly budget and its split, then every expense category in one table with what it
/// has spent and its budget field.
fn plan(ui: &Rc<Ui>, page: &Page, s: &Value, lists: &Value) {
    remember_currency(s);
    let fmt = formatter();
    let typed = typing(ui, "money-budget");
    title(&page.head, "Plan", &format!("Budgets for {}. Leave a field empty for no budget; changes save as you leave it.", period_name(&s["period"])));
    clear_rebuilding(&page.body);
    let pace = &s["pace"];
    let status = text(pace, "status");
    let budget = if status == "NO_BUDGET" { 0 } else { n(pace, "budget") };
    let spent: std::collections::BTreeMap<i64, Value> = rows(s, "budgets").into_iter().map(|b| (n(&b, "category_id"), b)).collect();
    let assigned: i64 = spent.values().map(|b| n(b, "budget").max(0)).sum();

    let (_, cells) = strip(&page.body);
    cell(&cells, "plan", "Monthly budget", &if budget > 0 { fmt.format_whole(budget) } else { String::from("None") }, "everything, every month", None);
    cell(&cells, "list", "Given to categories", &fmt.format_whole(assigned), &copy::plural(spent.values().filter(|b| n(b, "budget") > 0).count() as i64, "budget"), None);
    let free = budget - assigned;
    let (free_text, free_sub, free_class) = if budget == 0 {
        (String::from("—"), String::from("set a monthly budget first"), None)
    } else if free < 0 {
        (fmt.format_whole(-free), String::from("categories exceed the monthly budget"), Some("money-over"))
    } else {
        (fmt.format_whole(free), String::from("not given to a category"), None)
    };
    cell(&cells, "coins", if free < 0 { "Over by" } else { "Unassigned" }, &free_text, &free_sub, free_class);
    cell(&cells, "spend", "Spent so far", &fmt.format_whole(n(pace, "spent")), &period_name(&s["period"]), None);

    let overall = card(&page.body, "Monthly budget", None);
    let detail = if budget > 0 { copy::of_budget(n(pace, "spent"), budget, &fmt) } else { String::from("No budget: Overview shows what you spent") };
    let fraction = if budget > 0 { n(pace, "spent") as f64 / budget as f64 } else { 0.0 };
    overall.append(&budget_row(ui, None, "Everything", &detail, budget, tone_class(status), Some(tile("plan")), (budget > 0).then(|| (fraction, tone(status, None)))));

    let list = card_asking(&page.body, "By category", None, Some((ui, "Are my budgets realistic, given what I have been spending?")));
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    head.add_css_class("tally-th");
    let caption = label("Category", "tally-th-text");
    caption.set_hexpand(true);
    head.append(&caption);
    let budget_caption = label("Monthly budget", "tally-th-text");
    budget_caption.set_xalign(1.0);
    head.append(&budget_caption);
    list.append(&head);
    let categories: Vec<Value> = rows(lists, "categories").into_iter().filter(|c| text(c, "kind") == "EXPENSE" && c["archived"] != true).collect();
    if categories.is_empty() {
        list.append(&icon_row(&tile("list"), "No expense categories yet", "Import a backup or load sample data", "", ""));
    }
    for category in &categories {
        let id = n(category, "id");
        let reading = spent.get(&id);
        let amount = reading.map_or(0, |b| n(b, "budget"));
        let used = reading.map_or(0, |b| n(b, "spent"));
        let detail = match reading {
            Some(b) if amount > 0 => copy::of_budget(n(b, "spent"), amount, &fmt),
            _ if used > 0 => format!("{} spent, no budget", fmt.format_whole(used)),
            _ => String::from("Nothing spent"),
        };
        let status = reading.map_or("", |b| text(b, "status"));
        let meter = (amount > 0).then(|| (used as f64 / amount as f64, tone(status, category["color"].as_i64())));
        let lead = badge_sized(text(category, "icon"), n(category, "color"), 28);
        list.append(&budget_row(ui, Some(id), text(category, "name"), &detail, amount, tone_class(status), Some(lead), meter));
    }
    retype(&page.body, typed);
}

thread_local! {
    static REBUILDING: Cell<bool> = const { Cell::new(false) };
}

/// The field being typed in, when its name starts with `prefix`: a read that lands while a
/// figure is typed keeps the typing ([`retype`]).
pub(super) fn typing(ui: &Ui, prefix: &str) -> Option<(String, String, i32)> {
    gtk::prelude::GtkWindowExt::focus(&ui.window)
        .and_then(|w| w.ancestor(gtk::Entry::static_type()))
        .and_downcast::<gtk::Entry>()
        .filter(|e| e.widget_name().starts_with(prefix))
        .map(|e| (e.widget_name().to_string(), e.text().to_string(), e.position()))
}

/// Puts [`typing`]'s field back as it was, in the rebuilt `body`.
pub(super) fn retype(body: &gtk::Box, typed: Option<(String, String, i32)>) {
    let Some((name, text, position)) = typed else { return };
    if let Some(entry) = named(body.upcast_ref(), &name).and_downcast::<gtk::Entry>() {
        entry.set_text(&text);
        entry.grab_focus();
        entry.set_position(position);
    }
}

/// Clears `body` for a rebuild. Removing a focused field ends its focus: that is a rebuild, not
/// the person leaving it, so its save-on-leave holds back while [`rebuilding`].
pub(super) fn clear_rebuilding(body: &gtk::Box) {
    REBUILDING.with(|r| r.set(true));
    clear(body);
    REBUILDING.with(|r| r.set(false));
}

pub(super) fn rebuilding() -> bool {
    REBUILDING.with(|r| r.get())
}

fn named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == name {
            return Some(widget);
        }
        if let Some(found) = named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

/// A budget line: its mark and name over its reading, a meter of what it has used, and the
/// amount field that saves on Enter or on leaving it.
#[allow(clippy::too_many_arguments)]
fn budget_row(
    ui: &Rc<Ui>,
    category: Option<i64>,
    name: &str,
    detail: &str,
    amount: i64,
    detail_class: Option<&str>,
    lead: Option<gtk::Box>,
    meter: Option<(f64, Tone)>,
) -> gtk::Box {
    let fmt = formatter();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    if let Some(lead) = lead {
        row.append(&lead);
    }
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_valign(gtk::Align::Center);
    words.set_size_request(200, -1);
    let title = label(name, "money-row-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    let d = label(detail, "money-row-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if let Some(class) = detail_class {
        d.add_css_class(class);
    }
    words.append(&d);
    row.append(&words);
    let track = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    track.set_hexpand(true);
    track.set_valign(gtk::Align::Center);
    if let Some((fraction, tone)) = meter {
        track.append(&meter_in(fraction, None, tone, 6));
    }
    row.append(&track);
    let field = gtk::Entry::new();
    field.add_css_class("money-field");
    field.set_widget_name(&match category {
        Some(id) => format!("money-budget-{id}"),
        None => String::from("money-budget-overall"),
    });
    field.set_width_chars(9);
    gtk::prelude::EntryExt::set_alignment(&field, 1.0);
    field.set_valign(gtk::Align::Center);
    field.set_placeholder_text(Some("None"));
    if amount > 0 {
        field.set_text(&fmt.format_input(amount));
    }
    field.update_property(&[gtk::accessible::Property::Label(&format!("Budget for {name}"))]);
    let symbol = label(&fmt.symbol, "money-suffix");
    symbol.set_valign(gtk::Align::Center);
    row.append(&symbol);
    row.append(&field);
    let saved = Rc::new(std::cell::Cell::new(amount));
    let commit = {
        let weak = Rc::downgrade(ui);
        let saved = saved.clone();
        move |field: &gtk::Entry| {
            let Some(ui) = weak.upgrade() else { return };
            if rebuilding() {
                return;
            }
            let typed = field.text();
            let value = if typed.trim().is_empty() { Some(0) } else { formatter().parse(&typed) };
            let Some(value) = value else {
                field.add_css_class("error");
                super::toast(&ui, "Type an amount, like 250 or 250.00. Leave it empty for no budget.", None);
                return;
            };
            field.remove_css_class("error");
            if value == saved.get() {
                return;
            }
            saved.set(value);
            let mut payload = json!({"amount":value});
            if let Some(id) = category {
                payload["category_id"] = json!(id);
            }
            glib::spawn_future_local(async move {
                if let Err(e) = ui.call("money.budget.set", payload).await {
                    ui.show_error(&e.to_string());
                }
                ui.refresh_page();
            });
        }
    };
    let on_enter = commit.clone();
    field.connect_activate(move |field| on_enter(field));
    let focus = gtk::EventControllerFocus::new();
    let weak = field.downgrade();
    focus.connect_leave(move |_| {
        if let Some(field) = weak.upgrade() {
            commit(&field);
        }
    });
    field.add_controller(focus);
    row
}

/// A settings row: a mark, what it is over a line about it, and its control at the end.
pub(super) fn setting_row(list: &gtk::Box, icon: &str, title: &str, about: &str, control: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    row.add_css_class("tally-setting");
    row.append(&tile(icon));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    words.append(&label(title, "money-row-title"));
    if !about.is_empty() {
        let d = label(about, "money-row-detail");
        d.set_wrap(true);
        d.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        words.append(&d);
    }
    row.append(&words);
    control.set_valign(gtk::Align::Center);
    row.append(control);
    list.append(&row);
}

/// Data: the phone that syncs, backups, the ledger itself and its settings, in two columns.
fn data(ui: &Rc<Ui>, page: &Page, summary: Option<&Value>, lists: Option<&Value>) {
    title(&page.head, "Data", "Your ledger lives on this PC, in money.db beside Relay's store. Nothing here leaves it.");
    clear(&page.body);
    let empty = summary.is_none_or(|s| s["empty"] == true);
    let pair = columns(&page.body, 2);
    if let Some(lists) = lists {
        phones(&pair[0], lists);
    }
    let backups = card(&pair[0], "Backups", None);
    let import_key = button("Import…", "");
    import_key.set_widget_name("money-import");
    setting_row(&backups, "download", "Import a Tally backup", "The .json from Tally's Settings, Backup. Replaces everything here.", &import_key);
    let export_key = button("Export…", "");
    export_key.set_widget_name("money-export");
    export_key.set_sensitive(!empty);
    setting_row(&backups, "upload", "Export a backup", "The same file, to keep or to restore on the phone.", &export_key);

    let ledger = card(&pair[1], "Ledger", None);
    let account = button("Add…", "");
    account.set_widget_name("money-account");
    setting_row(&ledger, "bank", "Add an account", "Cash, chequing, savings, credit or investment. The first brings Tally's categories.", &account);
    let wealthsimple = button("Import…", "");
    wealthsimple.set_widget_name("money-invest-import");
    setting_row(&ledger, "chart", "Import a Wealthsimple file", "A holdings report, an activities export or a monthly statement (.csv). It adds to what is here.", &wealthsimple);
    let sample = button("Load", "");
    sample.set_widget_name("money-sample");
    sample.set_sensitive(empty);
    if !empty {
        sample.set_tooltip_text(Some("Only into an empty ledger: erase everything first"));
    }
    setting_row(&ledger, "spark", "Sample household", "Something to look around with. Only into an empty ledger.", &sample);
    let erase = button("Erase…", "money-destructive");
    erase.set_widget_name("money-reset");
    erase.set_sensitive(!empty);
    setting_row(&ledger, "trash", "Erase everything", "Every entry, account, budget, bill and goal on this PC. The phone keeps its own.", &erase);
    if let Some(summary) = summary {
        ledger_settings(ui, &pair[1], summary);
    }

    let weak = Rc::downgrade(ui);
    account.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::add_account(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    import_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import(&ui, empty);
        }
    });
    let weak = Rc::downgrade(ui);
    wealthsimple.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::invest::import_from_page(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    export_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            export(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    sample.connect_clicked(move |key| {
        if let Some(ui) = weak.upgrade() {
            load_sample(&ui, key);
        }
    });
    let weak = Rc::downgrade(ui);
    confirm_inline(&erase, "Erase all of it", move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        glib::spawn_future_local(async move {
            match ui.call("money.reset", json!({})).await {
                Ok(_) => super::toast(&ui, "Erased. The ledger on this PC is empty.", None),
                Err(e) => ui.show_error(&e.to_string()),
            }
            ui.refresh_page();
        });
    });
}

/// The phones that sync with this ledger (`money.lists` `devices`, latest first).
fn phones(column: &gtk::Box, lists: &Value) {
    let list = card(column, "Phone sync", None);
    let devices = rows(lists, "devices");
    if devices.is_empty() {
        list.append(&icon_row(&tile("phone"), "No phone syncs with this PC yet", "Pair from Tally's Settings with the link `relay remote pair` prints", "", ""));
    }
    for device in &devices {
        let when = crate::relative::ago(text(device, "last_sync"), crate::relative::Form::Long);
        let name = match text(device, "name") {
            "" => "A phone",
            name => name,
        };
        let line = match when {
            Some(when) => format!("Synced {when}"),
            None => String::from("Has not synced yet"),
        };
        list.append(&icon_row(&tile("phone"), name, &line, "", ""));
    }
}

/// The ledger's currency and the day its budget period starts (`money.settings.set`).
fn ledger_settings(ui: &Rc<Ui>, column: &gtk::Box, summary: &Value) {
    let list = card(column, "Settings", None);
    let currency = gtk::Entry::new();
    currency.add_css_class("money-field");
    currency.set_widget_name("money-currency");
    currency.set_width_chars(5);
    currency.set_max_length(3);
    currency.set_text(text(summary, "currency"));
    currency.update_property(&[gtk::accessible::Property::Label("Currency code")]);
    setting_row(&list, "coins", "Currency", "An ISO code, such as CAD. Changing it relabels amounts; it converts nothing.", &currency);
    let start = gtk::SpinButton::with_range(1.0, 28.0, 1.0);
    start.add_css_class("money-field");
    start.set_widget_name("money-month-start");
    start.set_value(summary["month_start_day"].as_f64().unwrap_or(1.0));
    start.update_property(&[gtk::accessible::Property::Label("Month start day")]);
    setting_row(&list, "calendar", "Month starts on day", "To follow your pay: 15 runs each period from the 15th to the 14th.", &start);
    let save = button("Save settings", "");
    save.set_widget_name("money-settings-save");
    save.set_halign(gtk::Align::End);
    save.add_css_class("tally-save");
    list.append(&save);
    let weak = Rc::downgrade(ui);
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let code = currency.text().trim().to_uppercase();
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
            super::toast(&ui, "A currency is three letters, such as CAD.", None);
            return;
        }
        let payload = json!({"currency":code,"month_start_day":start.value_as_int()});
        key.set_sensitive(false);
        let key = key.clone();
        glib::spawn_future_local(async move {
            match ui.call("money.settings.set", payload).await {
                Ok(v) => {
                    super::remember_currency(&v);
                    super::toast(&ui, "Saved. Every amount and period reads with the new settings.", None);
                }
                Err(e) => ui.show_error(&e.to_string()),
            }
            key.set_sensitive(true);
            ui.refresh_page();
        });
    });
}

fn load_sample(ui: &Rc<Ui>, key: &gtk::Button) {
    key.set_sensitive(false);
    let ui = ui.clone();
    let key = key.clone();
    glib::spawn_future_local(async move {
        match ui.call("money.sample", json!({})).await {
            Ok(v) => super::toast(&ui, &format!("Loaded a sample household: {}.", plural(n(&v, "transactions").max(0) as usize, "entry").replace("entrys", "entries")), None),
            Err(e) => ui.show_error(&e.to_string()),
        }
        key.set_sensitive(true);
        ui.refresh_page();
    });
}

/// Choose a Tally backup and restore it; over a ledger with anything in it, only after a yes.
fn import(ui: &Rc<Ui>, empty: bool) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Tally backups (.json)"));
        filter.add_pattern("*.json");
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder().title("Import a Tally backup").filters(&filters).default_filter(&filter).build();
        let Ok(file) = dialog.open_future(Some(&ui.window)).await else { return };
        let Some(path) = file.path() else {
            ui.show_error("That file is not on this PC's disk. Copy it here first.");
            return;
        };
        if !empty {
            let panel = crate::panel::Panel::new(&ui, "Replace everything?", 440);
            panel.add_css_class("money-sheet");
            let about = label(
                &format!(
                    "{} replaces every entry, account, budget, bill and goal on this PC. Export a backup first if you might want them back.",
                    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                ),
                "money-muted",
            );
            about.set_wrap(true);
            panel.body.append(&about);
            if !panel.response("Replace everything").await {
                return;
            }
        }
        match ui.call("money.import", json!({"path":path.to_string_lossy()})).await {
            Ok(v) => super::toast(
                &ui,
                &format!(
                    "Imported {} and {}.",
                    plural(n(&v, "transactions").max(0) as usize, "entry").replace("entrys", "entries"),
                    plural(n(&v, "accounts").max(0) as usize, "account")
                ),
                None,
            ),
            Err(e) => ui.show_error(&e.to_string()),
        }
        ui.refresh_page();
    });
}

fn export(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let name = format!("tally-backup-{}.json", today());
        let dialog = gtk::FileDialog::builder().title("Export a backup").initial_name(name.as_str()).build();
        let Ok(file) = dialog.save_future(Some(&ui.window)).await else { return };
        let Some(path) = file.path() else { return };
        match ui.call("money.export", json!({"path":path.to_string_lossy()})).await {
            Ok(v) => super::toast(
                &ui,
                &format!(
                    "Saved {} to {}.",
                    plural(n(&v, "transactions").max(0) as usize, "entry").replace("entrys", "entries"),
                    text(&v, "path")
                ),
                None,
            ),
            Err(e) => ui.show_error(&e.to_string()),
        }
    });
}
