//! Charts in a thread: what an agent's ```` ```chart ```` block draws (docs/THREADS.md).
//!
//! A chart keeps its question (a `money.series` payload), never its numbers: it reads them when it
//! is drawn and again whenever the ledger moves (`refresh_all`, on `money.changed`). One scrolled
//! out of view waits until it comes back (`load_stale`), so a long thread does not re-read every
//! chart it ever drew. Bars and lines are drawn with cairo; their axis and labels are plain labels
//! around the drawing, in Dev's type.
//! Colour is the data's: the newest period in Relay's signal, the ones before it in a fixed
//! palette, and categories in their own hue.
//!
//! A chart with `data` instead of a question carries Claude's own numbers, for what the ledger does
//! not hold (a plan, a projection): `{"labels", "series": [{"name", "values", "projected"?}],
//! "unit"?}`. Without `unit` the values are amounts in the currency as decimals (1234.56 or
//! "1234.56"); with one they are figures in it ("kg", "%", "km", "" for a count). A projected series
//! draws dashed or lighter, and the card says the numbers are the reply's and do not redraw.
//!
//! A chart with `gym` instead of `query` asks `gym.series` (Avex's training history, docs/GYM.md):
//! its values are weights, counts or minutes, shown in the unit the reading names, and a lift's
//! line starts its axis near its lowest value so progress shows. It reads again on every
//! `gym.changed` (`refresh_gym`).
//!
//! A `price` chart is Arbiter's: `{"type": "price", "title", "arbiter": <an arbiter.series
//! payload>, "cutoff": "2025-03-01"}`. It draws the closes, a strategy's buys (filled green) and
//! sells (hollow), and, given `cutoff`, where the model's memory ends: what it may remember is
//! shaded. It reads again on every `arbiter.changed` (`refresh_prices`).
use super::pages::{rgb, HUES};
use crate::app::{clear, label};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_client::thread_view::{ticks, ChartKind, ChartSpec};
use relay_money::money::{Locale, MoneyFormatter};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::f64::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

/// Series colours, the newest first: Relay's coral signal, then blue, grey, violet, teal, gold.
const PALETTE: [&str; 6] = ["#D97757", "#6A9FD8", "#A3A3A3", "#C27BC0", "#4FA9A0", "#D9A441"];
const PLOT_H: i32 = 184;
const PAD_T: f64 = 10.0;
const PAD_B: f64 = 8.0;
/// Room on the left for the axis labels.
const AXIS_W: i32 = 56;

/// The foot of a chart drawn from the numbers in its reply.
const WRITTEN: &str = "Claude's numbers from this reply, not Tally's: it does not redraw";
/// How strongly a projected series' bars and legend dot are drawn beside a known one's.
const PROJECTED_ALPHA: f64 = 0.42;

/// A chart drawn from a question: its body, its legend, the question, and whether the ledger moved
/// while it was out of view.
type Live = (glib::WeakRef<gtk::Box>, glib::WeakRef<gtk::Box>, ChartSpec, Rc<Cell<bool>>);
/// A price chart on screen: its body, its legend and what it asks Arbiter.
type PriceLive = (glib::WeakRef<gtk::Box>, glib::WeakRef<gtk::Box>, PriceSpec);
/// A training chart on screen: its body, its legend and what it asks Avex's history.
type GymLive = (glib::WeakRef<gtk::Box>, glib::WeakRef<gtk::Box>, ChartSpec);

thread_local! {
    /// Charts drawn from a question, to read again when the ledger moves.
    static LIVE: RefCell<Vec<Live>> = const { RefCell::new(Vec::new()) };
    /// Price charts, to read again when Arbiter moves.
    static PRICES: RefCell<Vec<PriceLive>> = const { RefCell::new(Vec::new()) };
    /// Training charts, to read again when a newer Avex export comes in.
    static GYM: RefCell<Vec<GymLive>> = const { RefCell::new(Vec::new()) };
}

fn series_color(index: usize, count: usize) -> &'static str {
    PALETTE[(count - 1 - index.min(count - 1)) % PALETTE.len()]
}

fn hue(color: Option<i64>, index: usize) -> &'static str {
    match color {
        Some(c) => HUES[c.rem_euclid(HUES.len() as i64) as usize],
        None => PALETTE[index % PALETTE.len()],
    }
}

fn dot(hex: &str) -> gtk::DrawingArea {
    dot_alpha(hex, 1.0)
}

/// A legend's dot, lighter for a projected series.
fn dot_alpha(hex: &str, alpha: f64) -> gtk::DrawingArea {
    let (r, g, b) = rgb(hex);
    let area = gtk::DrawingArea::new();
    area.set_content_width(9);
    area.set_content_height(9);
    area.set_valign(gtk::Align::Center);
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgba(r, g, b, alpha);
        cr.arc(f64::from(w) / 2.0, f64::from(h) / 2.0, 4.0, 0.0, 2.0 * PI);
        let _ = cr.fill();
    });
    area
}

/// A chart card for `spec`, reading its numbers now.
pub fn card(spec: ChartSpec) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("threads-chart");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let title = label(if spec.title.is_empty() { "Chart" } else { &spec.title }, "threads-chart-title");
    title.set_wrap(true);
    head.append(&title);
    let legend = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    legend.set_hexpand(true);
    legend.set_halign(gtk::Align::End);
    head.append(&legend);
    card.append(&head);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.append(&label(if spec.gym.is_some() { "Reading Avex…" } else { "Reading the ledger…" }, "threads-chart-note"));
    card.append(&body);
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    foot.add_css_class("threads-chart-foot");
    if spec.gym.is_some() {
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.add_css_class("threads-dot");
        lamp.set_valign(gtk::Align::Center);
        foot.append(&lamp);
        foot.append(&label("From Avex's last export, redraws when a newer one comes in", "threads-chart-note"));
        GYM.with(|l| l.borrow_mut().push((body.downgrade(), legend.downgrade(), spec.clone())));
    } else if spec.query.is_some() {
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.add_css_class("threads-dot");
        lamp.set_valign(gtk::Align::Center);
        foot.append(&lamp);
        foot.append(&label("Live from Tally, redraws when the ledger changes", "threads-chart-note"));
        LIVE.with(|l| l.borrow_mut().push((body.downgrade(), legend.downgrade(), spec.clone(), Rc::default())));
    } else {
        let note = label(WRITTEN, "threads-chart-note");
        note.set_ellipsize(gtk::pango::EllipsizeMode::End);
        note.set_hexpand(true);
        note.set_tooltip_text(Some(WRITTEN));
        foot.append(&note);
    }
    card.append(&foot);
    load(&body, &legend, &spec);
    card
}

/// The ledger moved: read again every live chart in view, and mark the rest to read when they
/// come back into view.
pub fn refresh_all() {
    let viewport = super::threads::chart_viewport();
    let live: Vec<_> = LIVE.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|(body, _, _, _)| body.upgrade().is_some());
        l.iter().filter_map(|(b, g, s, stale)| Some((b.upgrade()?, g.upgrade()?, s.clone(), stale.clone()))).collect()
    });
    for (body, legend, spec, stale) in live {
        if in_view(&body, viewport.as_ref()) {
            stale.set(false);
            load(&body, &legend, &spec);
        } else {
            stale.set(true);
        }
    }
}

/// A newer Avex export came in, or the copy was forgotten: read every training chart again.
pub fn refresh_gym() {
    let live: Vec<_> = GYM.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|(body, _, _)| body.upgrade().is_some());
        l.iter().filter_map(|(b, g, s)| Some((b.upgrade()?, g.upgrade()?, s.clone()))).collect()
    });
    for (body, legend, spec) in live {
        load(&body, &legend, &spec);
    }
}

/// Read the charts the ledger moved under while they were out of view, now that they show.
pub fn load_stale() {
    let viewport = super::threads::chart_viewport();
    let stale: Vec<_> = LIVE.with(|l| {
        l.borrow()
            .iter()
            .filter(|(_, _, _, stale)| stale.get())
            .filter_map(|(b, g, s, stale)| Some((b.upgrade()?, g.upgrade()?, s.clone(), stale.clone())))
            .collect()
    });
    for (body, legend, spec, flag) in stale {
        if in_view(&body, viewport.as_ref()) {
            flag.set(false);
            load(&body, &legend, &spec);
        }
    }
}

/// Whether `body` shows in `viewport` (the conversation's scroller); without one, whether it is
/// mapped at all.
fn in_view(body: &gtk::Box, viewport: Option<&gtk::Widget>) -> bool {
    if !body.is_mapped() {
        return false;
    }
    let Some(viewport) = viewport else { return true };
    match body.compute_bounds(viewport) {
        Some(rect) => rect.y() + rect.height() >= 0.0 && rect.y() <= viewport.height() as f32,
        None => true,
    }
}

fn load(body: &gtk::Box, legend: &gtk::Box, spec: &ChartSpec) {
    if let Some(data) = &spec.data {
        let currency = data["currency"].as_str().filter(|c| !c.is_empty()).map_or_else(|| super::formatter().currency, str::to_string);
        draw_data(body, legend, spec.kind, read_written(data, &currency, spec.kind));
        return;
    }
    if let Some(query) = spec.gym.clone() {
        let Some(ui) = super::threads::the_ui_pub() else { return };
        let (body, legend, kind) = (body.downgrade(), legend.downgrade(), spec.kind);
        glib::spawn_future_local(async move {
            let read = ui.call("gym.series", query).await;
            let (Some(body), Some(legend)) = (body.upgrade(), legend.upgrade()) else { return };
            match read {
                Ok(series) => draw_data(&body, &legend, kind, read_gym(&series, kind)),
                Err(e) => {
                    clear(&body);
                    let note = label(&format!("This chart could not be read: {}", super::avex::said(&e)), "threads-error");
                    note.set_wrap(true);
                    body.append(&note);
                }
            }
        });
        return;
    }
    let Some(query) = spec.query.clone() else { return };
    let Some(ui) = super::threads::the_ui_pub() else { return };
    let (body, legend, kind) = (body.downgrade(), legend.downgrade(), spec.kind);
    glib::spawn_future_local(async move {
        let read = ui.call("money.series", query).await;
        let (Some(body), Some(legend)) = (body.upgrade(), legend.upgrade()) else { return };
        match read {
            Ok(series) => draw(&body, &legend, kind, &series),
            Err(e) => {
                clear(&body);
                let note = label(&format!("This chart could not be read: {}", super::pages::unavailable(&e)), "threads-error");
                note.set_wrap(true);
                body.append(&note);
            }
        }
    });
}

/// Each series' name and values, `None` where it has none.
type Series = Vec<(String, Vec<Option<i64>>)>;

struct Data {
    labels: Vec<String>,
    colors: Vec<Option<i64>>,
    /// Each series' name and values; a value the reading or the reply does not give is `None`,
    /// left out rather than drawn at zero.
    series: Series,
    /// Which series are projected: dashed on a line, lighter as bars, named so in the legend.
    projected: Vec<bool>,
    /// How many labels each series has reached (the period still running), else all of them.
    known: Vec<usize>,
    fmt: Fmt,
    /// The value axis starts at zero; a lift's line starts near its lowest value instead.
    from_zero: bool,
    /// What the card says when there is nothing to draw.
    empty: &'static str,
}

/// How a chart's values read: money in minor units, or a figure (training, or a unit the reply
/// names) in tenths of its unit.
#[derive(Clone)]
enum Fmt {
    Money(MoneyFormatter),
    Unit(String),
}

impl Fmt {
    fn fraction_digits(&self) -> u32 {
        match self {
            Fmt::Money(f) => f.fraction_digits,
            Fmt::Unit(_) => 1,
        }
    }

    fn unit_words(unit: &str, tenths: i64, whole: bool) -> String {
        let v = tenths as f64 / 10.0;
        let n = if whole || tenths % 10 == 0 { super::avex::group(v.round() as i64) } else { format!("{v:.1}") };
        match unit {
            "" => n,
            "%" => format!("{n}%"),
            "sessions" | "sets" | "minutes" if !whole => format!("{n} {unit}"),
            "sessions" | "sets" | "minutes" => n,
            unit => format!("{n} {unit}"),
        }
    }

    fn format(&self, v: i64) -> String {
        match self {
            Fmt::Money(f) => f.format(v),
            Fmt::Unit(u) => Fmt::unit_words(u, v, false),
        }
    }

    fn format_whole(&self, v: i64) -> String {
        match self {
            Fmt::Money(f) => f.format_whole(v),
            Fmt::Unit(u) => Fmt::unit_words(u, v, true),
        }
    }
}

/// A `gym.series` reading as a chart's numbers, in tenths of its unit. A lift's best has nothing
/// to read in a week it was not done: such points are left out rather than drawn at zero.
fn read_gym(v: &Value, kind: ChartKind) -> Data {
    let (labels, series) = gaps_closed(&v["labels"], &v["series"], |value| value.as_f64().map(|x| (x * 10.0).round() as i64));
    let measure = v["measure"].as_str().unwrap_or("");
    Data {
        known: vec![labels.len(); series.len()],
        projected: vec![false; series.len()],
        colors: vec![None; labels.len()],
        labels,
        series,
        fmt: Fmt::Unit(v["unit"].as_str().unwrap_or("").to_string()),
        from_zero: !(kind == ChartKind::Line && matches!(measure, "e1rm" | "top_weight")),
        empty: "Nothing to draw yet: no workouts match.",
    }
}

/// A label as written: a string, or a number ("2027").
fn label_text(label: &Value) -> String {
    match label {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Labels and each series' values read with `value`, keeping only the labels at least one series
/// gives a value for. A value that does not read stays `None`: left out, never drawn as zero.
fn gaps_closed(labels: &Value, series: &Value, value: impl Fn(&Value) -> Option<i64>) -> (Vec<String>, Series) {
    let labels: Vec<String> = labels.as_array().into_iter().flatten().map(label_text).collect();
    let lines: Series = series
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| (s["name"].as_str().unwrap_or("").to_string(), (0..labels.len()).map(|i| value(&s["values"][i])).collect()))
        .collect();
    let keep: Vec<usize> = (0..labels.len()).filter(|i| lines.iter().any(|(_, values)| values[*i].is_some())).collect();
    let series = lines.into_iter().map(|(name, values)| (name, keep.iter().map(|i| values[*i]).collect())).collect();
    (keep.iter().map(|i| labels[*i].clone()).collect(), series)
}

/// A decimal as an agent writes one in a reply: a number, or a string of digits with at most one
/// point and a sign ("1234.56", "-20", "+3.5"), scaled to `places` decimals. Anything else
/// ("$5", "12 kg", "n/a", null) is no value.
fn written_value(value: &Value, places: u32) -> Option<i64> {
    let x = match value {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => {
            let t = s.trim().replace('\u{2212}', "-");
            let digits = t.strip_prefix(['-', '+']).unwrap_or(&t);
            let plain = digits.chars().any(|c| c.is_ascii_digit())
                && digits.chars().all(|c| c.is_ascii_digit() || c == '.')
                && digits.matches('.').count() <= 1;
            if !plain {
                return None;
            }
            t.parse::<f64>().ok()?
        }
        _ => return None,
    };
    let scaled = (x * 10_f64.powi(places as i32)).round();
    (scaled.is_finite() && scaled.abs() < 9.0e15).then_some(scaled as i64)
}

/// A chart's own numbers, from its reply (`data`): amounts in `currency` as decimals, converted to
/// its minor units, or with a `unit`, figures in tenths of it as a training chart's are.
fn read_written(v: &Value, currency: &str, kind: ChartKind) -> Data {
    let fmt = match v["unit"].as_str() {
        Some(unit) => Fmt::Unit(unit.trim().to_string()),
        None => Fmt::Money(MoneyFormatter::new(currency, Locale::from_env())),
    };
    let places = fmt.fraction_digits();
    let (labels, series) = gaps_closed(&v["labels"], &v["series"], |value| written_value(value, places));
    let projected = v["series"].as_array().into_iter().flatten().map(|s| s["projected"] == true).collect();
    // A line of a measure (a weight, a share) starts near its lowest value, as a lift's does:
    // from zero, 80 kg to 75 kg would lie flat.
    let from_zero = !(kind == ChartKind::Line && matches!(fmt, Fmt::Unit(_)));
    Data {
        known: vec![labels.len(); series.len()],
        projected,
        colors: vec![None; labels.len()],
        labels,
        series,
        fmt,
        from_zero,
        empty: "Nothing to draw: the reply gave no numbers.",
    }
}

/// A series' name in the legend and its tooltip: "… (projected)" when it is.
fn legend_name(name: &str, projected: bool) -> String {
    match (name.trim(), projected) {
        ("", true) => String::from("Projected"),
        (name, true) if name.to_lowercase().contains("project") => name.to_string(),
        (name, true) => format!("{name} (projected)"),
        (name, false) => name.to_string(),
    }
}

/// A `money.series` reading: every period has a figure, and one with nothing spent reads as zero.
fn read(v: &Value) -> Data {
    let labels: Vec<String> = v["labels"].as_array().into_iter().flatten().map(|l| l.as_str().unwrap_or("").to_string()).collect();
    let colors = (0..labels.len()).map(|i| v["label_colors"][i].as_i64()).collect();
    let series: Series = v["series"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let values = (0..labels.len()).map(|i| Some(s["values"][i].as_i64().unwrap_or(0))).collect();
            (s["name"].as_str().unwrap_or("").to_string(), values)
        })
        .collect();
    let known = v["series"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| s["known"].as_u64().map_or(labels.len(), |k| (k as usize).min(labels.len())))
        .collect();
    let currency = v["currency"].as_str().filter(|c| !c.is_empty()).unwrap_or("CAD");
    Data {
        projected: vec![false; series.len()],
        labels,
        colors,
        series,
        known,
        fmt: Fmt::Money(MoneyFormatter::new(currency, Locale::from_env())),
        from_zero: true,
        empty: "Nothing to draw yet: no entries match.",
    }
}

fn draw(body: &gtk::Box, legend: &gtk::Box, kind: ChartKind, v: &Value) {
    draw_data(body, legend, kind, read(v));
}

fn draw_data(body: &gtk::Box, legend: &gtk::Box, kind: ChartKind, data: Data) {
    clear(body);
    clear(legend);
    if data.labels.is_empty() || data.series.iter().all(|(_, values)| values.iter().all(|x| x.unwrap_or(0) == 0)) {
        body.append(&label(data.empty, "threads-chart-note"));
        return;
    }
    match kind {
        ChartKind::Donut => donut(body, &data),
        _ => {
            let count = data.series.len();
            if count > 1 || kind == ChartKind::Line || data.projected.contains(&true) {
                for (i, (name, _)) in data.series.iter().enumerate() {
                    let projected = data.projected.get(i).copied().unwrap_or(false);
                    let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    item.append(&dot_alpha(series_color(i, count), if projected { PROJECTED_ALPHA } else { 1.0 }));
                    item.append(&label(&legend_name(name, projected), "threads-chart-legend"));
                    legend.append(&item);
                }
            }
            plot(body, &data, kind);
        }
    }
}

/// An axis tick's words: whole units when the step is a whole unit, else with the fraction it
/// needs, so half-percent steps read 0.5% and 1.5% rather than 1% twice.
fn axis_label(fmt: &Fmt, value: i64, step: i64) -> String {
    if step % 10_i64.pow(fmt.fraction_digits()) == 0 {
        fmt.format_whole(value)
    } else {
        fmt.format(value)
    }
}

/// A value axis for values from `lo` (at most 0) to `hi` (at least 0), the lowest drawn being
/// `least`: its base, one step for every tick, and how many steps go above and below the base.
/// A line that need not start at zero starts a step under its lowest value, on a tick of the
/// same step as the rest (80.5 to 75 kg reads 72, 74 … 82, not 72, 77, 82). With values below
/// zero the step fits the whole span, and nothing above zero leaves no empty half on top.
fn axis(lo: i64, hi: i64, least: i64, from_zero: bool, digits: u32) -> (i64, i64, u32, u32) {
    let steps = |span: i64, step: i64| ((span.max(0) + step - 1) / step) as u32;
    if lo < 0 {
        let (step, _) = ticks((hi - lo).max(1), digits);
        return (0, step, steps(hi, step), steps(-lo, step));
    }
    if from_zero || least <= 0 {
        let (step, up) = ticks(hi.max(1), digits);
        return (0, step, up, 0);
    }
    let (step, _) = ticks((hi - least).max(1), digits);
    let base = (least / step - 1).max(0) * step;
    (base, step, steps(hi - base, step).max(1), 0)
}

/// Bars or lines over a value axis, labels under each group.
fn plot(body: &gtk::Box, data: &Data, kind: ChartKind) {
    let all = data.series.iter().flat_map(|(_, v)| v.iter().flatten().copied());
    let (lo, hi) = all.fold((0_i64, 0_i64), |(lo, hi), x| (lo.min(x), hi.max(x)));
    let digits = data.fmt.fraction_digits();
    let least = data.series.iter().flat_map(|(_, v)| v.iter().flatten().copied()).min().unwrap_or(0);
    let (base, step, up, down) = axis(lo, hi, least, data.from_zero, digits);
    let (top, bottom) = (base + step * i64::from(up), base - step * i64::from(down));
    let span = (top - bottom).max(1) as f64;
    let y_of = move |value: i64, h: f64| PAD_T + (top - value) as f64 / span * (h - PAD_T - PAD_B);

    let overlay = gtk::Overlay::new();
    let area = gtk::DrawingArea::new();
    area.set_content_height(PLOT_H);
    area.set_hexpand(true);
    overlay.set_child(Some(&area));
    for k in -(down as i64)..=i64::from(up) {
        let value = base + k * step;
        let tick = label(&axis_label(&data.fmt, value, step), "threads-chart-axis");
        tick.set_halign(gtk::Align::Start);
        tick.set_valign(gtk::Align::Start);
        tick.set_margin_top((y_of(value, f64::from(PLOT_H)) - 7.0).max(0.0) as i32);
        tick.set_can_target(false);
        overlay.add_overlay(&tick);
    }
    body.append(&overlay);

    let groups = data.labels.len();
    let count = data.series.len();
    let series = data.series.clone();
    let known = data.known.clone();
    let projected = data.projected.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let (w, h) = (f64::from(width), f64::from(height));
        let left = f64::from(AXIS_W);
        let plot_w = (w - left - 4.0).max(10.0);
        let gw = plot_w / groups as f64;
        // Grid: one line per tick, the zero line a step brighter.
        for k in -(down as i64)..=i64::from(up) {
            let y = y_of(base + k * step, h).round() + 0.5;
            let (r, g, b) = rgb(if k == 0 { "#333333" } else { "#1C1C1C" });
            cr.set_source_rgb(r, g, b);
            cr.set_line_width(1.0);
            cr.move_to(left, y);
            cr.line_to(w - 4.0, y);
            let _ = cr.stroke();
        }
        let zero = y_of(base, h);
        match kind {
            ChartKind::Line => {
                for (i, (_, values)) in series.iter().enumerate() {
                    // A period still running stops at today: the days after it have not happened.
                    let values = &values[..known.get(i).copied().unwrap_or(values.len()).max(1).min(values.len())];
                    let (r, g, b) = rgb(series_color(i, count));
                    cr.set_source_rgb(r, g, b);
                    cr.set_line_width(2.5);
                    cr.set_line_join(gtk::cairo::LineJoin::Round);
                    // A projection is dashed, so it never reads as what happened.
                    if projected.get(i).copied().unwrap_or(false) {
                        cr.set_dash(&[6.0, 4.0], 0.0);
                    }
                    // A missing point breaks the line; the next point starts it again.
                    let mut drawing = false;
                    for (j, v) in values.iter().enumerate() {
                        let Some(v) = v else {
                            drawing = false;
                            continue;
                        };
                        let (x, y) = (left + (j as f64 + 0.5) * gw, y_of(*v, h));
                        if drawing { cr.line_to(x, y) } else { cr.move_to(x, y) }
                        drawing = true;
                    }
                    let _ = cr.stroke();
                    cr.set_dash(&[], 0.0);
                    for (j, v) in values.iter().enumerate() {
                        let Some(v) = v else { continue };
                        // A point alone between gaps has no line to show it: it keeps its dot.
                        let alone = (j == 0 || values[j - 1].is_none()) && values.get(j + 1).is_none_or(Option::is_none);
                        if groups <= 16 || alone {
                            cr.arc(left + (j as f64 + 0.5) * gw, y_of(*v, h), 3.0, 0.0, 2.0 * PI);
                            let _ = cr.fill();
                        }
                    }
                }
            }
            _ => {
                let bw = (gw * 0.72 / count as f64 - 3.0).clamp(3.0, 26.0);
                let inner = bw * count as f64 + 3.0 * (count as f64 - 1.0);
                for j in 0..groups {
                    let x0 = left + j as f64 * gw + (gw - inner) / 2.0;
                    for (i, (_, values)) in series.iter().enumerate() {
                        let v = values[j].unwrap_or(0);
                        if v == 0 {
                            continue;
                        }
                        let (r, g, b) = rgb(series_color(i, count));
                        // A projected bar is lighter than one that happened.
                        cr.set_source_rgba(r, g, b, if projected.get(i).copied().unwrap_or(false) { PROJECTED_ALPHA } else { 1.0 });
                        let x = x0 + i as f64 * (bw + 3.0);
                        let (y1, y2) = (y_of(v, h).min(zero), y_of(v, h).max(zero));
                        let rad = (bw / 2.0).min(3.0).min(y2 - y1);
                        // Rounded at the end away from zero.
                        if v > 0 {
                            cr.move_to(x, y2);
                            cr.line_to(x, y1 + rad);
                            cr.arc(x + rad, y1 + rad, rad, PI, 1.5 * PI);
                            cr.arc(x + bw - rad, y1 + rad, rad, 1.5 * PI, 0.0);
                            cr.line_to(x + bw, y2);
                        } else {
                            cr.move_to(x, y1);
                            cr.line_to(x, y2 - rad);
                            cr.arc_negative(x + rad, y2 - rad, rad, PI, FRAC_PI_2);
                            cr.arc_negative(x + bw - rad, y2 - rad, rad, FRAC_PI_2, 0.0);
                            cr.line_to(x + bw, y1);
                        }
                        cr.close_path();
                        let _ = cr.fill();
                    }
                }
            }
        }
    });
    // Hovering a group says its numbers.
    area.set_has_tooltip(true);
    let (labels, series, fmt, projected) = (data.labels.clone(), data.series.clone(), data.fmt.clone(), data.projected.clone());
    area.connect_query_tooltip(move |area, x, _, _, tooltip| {
        let left = f64::from(AXIS_W);
        let gw = (f64::from(area.width()) - left - 4.0).max(10.0) / labels.len() as f64;
        let j = ((f64::from(x) - left) / gw).floor();
        if j < 0.0 || j as usize >= labels.len() {
            return false;
        }
        let j = j as usize;
        let mut lines = vec![labels[j].clone()];
        for (i, (name, values)) in series.iter().enumerate().rev() {
            // A series with nothing here says nothing, not zero.
            let Some(value) = values[j] else { continue };
            let name = legend_name(name, projected.get(i).copied().unwrap_or(false));
            let name = if name.is_empty() { String::new() } else { format!("{name}  ") };
            lines.push(format!("{name}{}", fmt.format(value)));
        }
        tooltip.set_text(Some(&lines.join("\n")));
        true
    });

    // The group labels, thinned to about ten.
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_homogeneous(true);
    row.set_margin_start(AXIS_W);
    row.set_margin_end(4);
    let every = groups.div_ceil(10).max(1);
    for (j, name) in data.labels.iter().enumerate() {
        let shown = if j % every == 0 { name.as_str() } else { "" };
        let l = label(shown, "threads-chart-x");
        l.set_xalign(0.5);
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        l.set_tooltip_text(Some(name));
        row.append(&l);
    }
    body.append(&row);
}

/// Where the money went: a ring of the first series, and a list beside it.
fn donut(body: &gtk::Box, data: &Data) {
    let (_, values) = &data.series[data.series.len() - 1];
    let parts: Vec<(String, i64, &'static str)> = data
        .labels
        .iter()
        .zip(values)
        .enumerate()
        .filter_map(|(i, (l, v))| v.filter(|v| *v > 0).map(|v| (l.clone(), v, hue(data.colors[i], i))))
        .collect();
    let total: i64 = parts.iter().map(|(_, v, _)| v).sum();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    let overlay = gtk::Overlay::new();
    let ring = gtk::DrawingArea::new();
    ring.set_content_width(156);
    ring.set_content_height(156);
    ring.set_valign(gtk::Align::Center);
    let slices: Vec<(f64, &'static str)> = parts.iter().map(|(_, v, c)| (*v as f64 / total.max(1) as f64, *c)).collect();
    ring.set_draw_func(move |_, cr, w, h| {
        let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
        let radius = cx.min(cy) - 12.0;
        cr.set_line_width(22.0);
        let mut at = -FRAC_PI_2;
        for (fraction, color) in &slices {
            let sweep = fraction * 2.0 * PI;
            let (r, g, b) = rgb(color);
            cr.set_source_rgb(r, g, b);
            // A hairline gap between slices.
            let gap = if slices.len() > 1 { 0.012 } else { 0.0 };
            cr.arc(cx, cy, radius, at + gap, at + sweep - gap);
            let _ = cr.stroke();
            at += sweep;
        }
    });
    overlay.set_child(Some(&ring));
    let centre = gtk::Box::new(gtk::Orientation::Vertical, 0);
    centre.set_halign(gtk::Align::Center);
    centre.set_valign(gtk::Align::Center);
    let sum = label(&data.fmt.format_whole(total), "threads-chart-total");
    sum.set_xalign(0.5);
    centre.append(&sum);
    let caption = label(&data.series[data.series.len() - 1].0, "threads-chart-note");
    caption.set_xalign(0.5);
    centre.append(&caption);
    overlay.add_overlay(&centre);
    row.append(&overlay);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    list.set_valign(gtk::Align::Center);
    list.set_hexpand(true);
    for (name, value, color) in &parts {
        let item = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        item.append(&dot(color));
        let n = label(name, "threads-chart-legend");
        n.set_hexpand(true);
        item.append(&n);
        item.append(&label(&data.fmt.format_whole(*value), "threads-chart-value"));
        let share = label(&format!("{:.0}%", *value as f64 * 100.0 / total.max(1) as f64), "threads-chart-share");
        share.set_width_chars(4);
        share.set_xalign(1.0);
        item.append(&share);
        list.append(&item);
    }
    row.append(&list);
    body.append(&row);
}

/// The price line: blue, so green and ink stay the buys' and the sells'.
const PRICE_LINE: &str = "#6A9FD8";
const BUY: &str = "#2ec469";
const SELL: &str = "#EDEDED";

/// A price chart's question: `query` is an `arbiter.series` payload; `cutoff` (Unix seconds) is
/// where the model's training data ends.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceSpec {
    pub title: String,
    pub query: Value,
    pub cutoff: Option<i64>,
}

/// Read a ```` ```chart ```` block of `"type": "price"`. `None` for any other chart.
pub fn price_spec(text: &str) -> Option<PriceSpec> {
    let v: Value = serde_json::from_str(text.trim()).ok()?;
    if v["type"].as_str() != Some("price") {
        return None;
    }
    let query = v.get("arbiter").filter(|q| q["product"].is_string())?.clone();
    Some(PriceSpec { title: v["title"].as_str().unwrap_or("").to_string(), query, cutoff: v["cutoff"].as_str().and_then(utc_day) })
}

/// "2025-03-01" as midnight UTC, or a full RFC 3339 time, in Unix seconds.
fn utc_day(value: &str) -> Option<i64> {
    if let Ok(t) = glib::DateTime::from_iso8601(value, Some(&glib::TimeZone::utc())) {
        return Some(t.to_unix());
    }
    let mut parts = value.trim().split('-').map(|p| p.parse::<i32>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    glib::DateTime::from_utc(y, m, d, 0, 0, 0.0).ok().map(|t| t.to_unix())
}

/// A price card for `spec`, reading its candles now and on every `arbiter.changed`.
pub fn price_card(spec: PriceSpec) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("threads-chart");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let product = spec.query["product"].as_str().unwrap_or("Price").to_string();
    let title = label(if spec.title.is_empty() { &product } else { &spec.title }, "threads-chart-title");
    title.set_wrap(true);
    head.append(&title);
    let legend = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    legend.set_hexpand(true);
    legend.set_halign(gtk::Align::End);
    head.append(&legend);
    card.append(&head);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.append(&label("Reading prices…", "threads-chart-note"));
    card.append(&body);
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    foot.add_css_class("threads-chart-foot");
    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    lamp.add_css_class("threads-dot");
    lamp.set_valign(gtk::Align::Center);
    foot.append(&lamp);
    foot.append(&label("Live from Arbiter, redraws when it trades", "threads-chart-note"));
    card.append(&foot);
    PRICES.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|(body, _, _)| body.upgrade().is_some());
        l.push((body.downgrade(), legend.downgrade(), spec.clone()));
    });
    load_price(&body, &legend, &spec);
    card
}

/// Read every price chart still on screen again.
pub fn refresh_prices() {
    let live: Vec<_> = PRICES.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|(body, _, _)| body.upgrade().is_some());
        l.iter().filter_map(|(b, g, s)| Some((b.upgrade()?, g.upgrade()?, s.clone()))).collect()
    });
    for (body, legend, spec) in live {
        load_price(&body, &legend, &spec);
    }
}

fn load_price(body: &gtk::Box, legend: &gtk::Box, spec: &PriceSpec) {
    let Some(ui) = super::threads::the_ui_pub() else { return };
    let (body, legend, query, cutoff) = (body.downgrade(), legend.downgrade(), spec.query.clone(), spec.cutoff);
    glib::spawn_future_local(async move {
        let read = ui.call("arbiter.series", query).await;
        let (Some(body), Some(legend)) = (body.upgrade(), legend.upgrade()) else { return };
        match read {
            Ok(series) => draw_price(&body, &legend, &series, cutoff),
            Err(e) => {
                clear(&body);
                let message = match &e {
                    crate::client::Error::Bus(b) => b.message.clone(),
                    e => e.to_string(),
                };
                let note = label(&format!("These prices could not be read: {message}"), "threads-error");
                note.set_wrap(true);
                body.append(&note);
            }
        }
    });
}

/// Round steps for a price axis from `lo` to `hi`: (step, bottom, top).
fn price_ticks(lo: f64, hi: f64) -> (f64, f64, f64) {
    let span = (hi - lo).max(hi.abs() * 1e-4).max(1e-9);
    let rough = span / 4.0;
    let power = 10_f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * power).find(|s| *s >= rough).unwrap_or(10.0 * power);
    (step, (lo / step).floor() * step, (hi / step).ceil() * step)
}

/// A price for an axis: grouped, with as many decimals as the step needs.
fn axis_price(x: f64, step: f64) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    let text = format!("{x:.decimals$}");
    let (whole, fraction) = text.split_once('.').map_or((text.as_str(), None), |(w, f)| (w, Some(f)));
    let (sign, digits) = whole.strip_prefix('-').map_or(("", whole), |d| ("−", d));
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    match fraction {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// A price in a tooltip: cents above 1, six places below.
fn tip_price(p: f64) -> String {
    axis_price(p, if p >= 1.0 { 0.01 } else { 0.000_001 })
}

fn local_time(unix: i64, format: &str) -> String {
    glib::DateTime::from_unix_local(unix).ok().and_then(|d| d.format(format).ok()).map(|s| s.to_string()).unwrap_or_default()
}

fn ring(hex: &str, filled: bool) -> gtk::DrawingArea {
    let (r, g, b) = rgb(hex);
    let area = gtk::DrawingArea::new();
    area.set_content_width(10);
    area.set_content_height(10);
    area.set_valign(gtk::Align::Center);
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgb(r, g, b);
        cr.arc(f64::from(w) / 2.0, f64::from(h) / 2.0, 3.6, 0.0, 2.0 * PI);
        if filled {
            let _ = cr.fill();
        } else {
            cr.set_line_width(1.5);
            let _ = cr.stroke();
        }
    });
    area
}

/// The closes as a line over a price axis, the trades on it, and the memory line.
fn draw_price(body: &gtk::Box, legend: &gtk::Box, v: &Value, cutoff: Option<i64>) {
    clear(body);
    clear(legend);
    let candles: Vec<(i64, f64)> = v["candles"].as_array().into_iter().flatten().filter_map(|c| Some((c["start"].as_i64()?, c["close"].as_f64()?))).collect();
    if candles.len() < 2 {
        body.append(&label("No prices to draw yet.", "threads-chart-note"));
        return;
    }
    // (when, a buy, the price).
    let markers: Vec<(i64, bool, f64)> = v["markers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| Some((m["at"].as_i64()?, m["side"].as_str()? == "buy", m["price"].as_f64()?)))
        .collect();
    let product = v["product"].as_str().unwrap_or("Price");
    let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    item.append(&dot(PRICE_LINE));
    item.append(&label(product, "threads-chart-legend"));
    legend.append(&item);
    if markers.iter().any(|m| m.1) {
        let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        item.append(&ring(BUY, true));
        item.append(&label("Bought", "threads-chart-legend"));
        legend.append(&item);
    }
    if markers.iter().any(|m| !m.1) {
        let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        item.append(&ring(SELL, false));
        item.append(&label("Sold", "threads-chart-legend"));
        legend.append(&item);
    }
    let (t0, t1) = (candles[0].0, candles[candles.len() - 1].0);
    let prices = candles.iter().map(|c| c.1).chain(markers.iter().map(|m| m.2));
    let (lo, hi) = prices.fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let (step, bottom, top) = price_ticks(lo, hi);
    let span = (top - bottom).max(1e-9);
    let y_of = move |value: f64, h: f64| PAD_T + (top - value) / span * (h - PAD_T - PAD_B);
    let left = f64::from(AXIS_W);
    let x_of = move |t: i64, w: f64| left + (t - t0) as f64 / (t1 - t0).max(1) as f64 * (w - left - 4.0).max(10.0);
    // Where the memory line falls: inside the range, or before or after all of it.
    let memory = cutoff.filter(|c| *c > t0 && *c < t1);

    let overlay = gtk::Overlay::new();
    let area = gtk::DrawingArea::new();
    area.set_content_height(PLOT_H);
    area.set_hexpand(true);
    overlay.set_child(Some(&area));
    let ticks = ((top - bottom) / step).round() as i64;
    for k in 0..=ticks {
        let value = bottom + k as f64 * step;
        let tick = label(&axis_price(value, step), "threads-chart-axis");
        tick.set_halign(gtk::Align::Start);
        tick.set_valign(gtk::Align::Start);
        tick.set_margin_top((y_of(value, f64::from(PLOT_H)) - 7.0).max(0.0) as i32);
        tick.set_can_target(false);
        overlay.add_overlay(&tick);
    }
    if let Some(at) = memory {
        let words = label("model's memory ends", "threads-chart-memory");
        words.set_halign(gtk::Align::Start);
        words.set_valign(gtk::Align::Start);
        words.set_can_target(false);
        overlay.add_overlay(&words);
        // The label sits just right of the line, wherever the width puts it.
        let placed = words.downgrade();
        area.connect_resize(move |_, w, _| {
            if let Some(words) = placed.upgrade() {
                words.set_margin_start(x_of(at, f64::from(w)) as i32 + 6);
            }
        });
    }
    body.append(&overlay);

    let drawn = candles.clone();
    let marks = markers.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let (w, h) = (f64::from(width), f64::from(height));
        for k in 0..=ticks {
            let y = y_of(bottom + k as f64 * step, h).round() + 0.5;
            let (r, g, b) = rgb("#1C1C1C");
            cr.set_source_rgb(r, g, b);
            cr.set_line_width(1.0);
            cr.move_to(left, y);
            cr.line_to(w - 4.0, y);
            let _ = cr.stroke();
        }
        if let Some(at) = memory {
            // What the model may remember, faintly shaded, and the line where it stops.
            let x = x_of(at, w).round() + 0.5;
            cr.set_source_rgba(0.93, 0.91, 0.89, 0.05);
            cr.rectangle(left, 0.0, x - left, h);
            let _ = cr.fill();
            let (r, g, b) = rgb("#878787");
            cr.set_source_rgb(r, g, b);
            cr.set_line_width(1.0);
            cr.set_dash(&[4.0, 3.0], 0.0);
            cr.move_to(x, 0.0);
            cr.line_to(x, h);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
        }
        let (r, g, b) = rgb(PRICE_LINE);
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(2.0);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        for (i, (t, close)) in drawn.iter().enumerate() {
            let (x, y) = (x_of(*t, w), y_of(*close, h));
            if i == 0 { cr.move_to(x, y) } else { cr.line_to(x, y) }
        }
        let _ = cr.stroke();
        for (t, buy, price) in &marks {
            let (x, y) = (x_of(*t, w), y_of(*price, h));
            if *buy {
                // A dark ring keeps a dot readable where it sits on the line.
                let (r, g, b) = rgb("#070707");
                cr.set_source_rgb(r, g, b);
                cr.arc(x, y, 5.5, 0.0, 2.0 * PI);
                let _ = cr.fill();
                let (r, g, b) = rgb(BUY);
                cr.set_source_rgb(r, g, b);
                cr.arc(x, y, 4.0, 0.0, 2.0 * PI);
                let _ = cr.fill();
            } else {
                let (r, g, b) = rgb("#070707");
                cr.set_source_rgb(r, g, b);
                cr.arc(x, y, 4.5, 0.0, 2.0 * PI);
                let _ = cr.fill();
                let (r, g, b) = rgb(SELL);
                cr.set_source_rgb(r, g, b);
                cr.set_line_width(1.75);
                cr.arc(x, y, 4.0, 0.0, 2.0 * PI);
                let _ = cr.stroke();
            }
        }
    });
    // Hovering says the bar's time and close, and the trades on it.
    area.set_has_tooltip(true);
    let bar = (t1 - t0) / (candles.len() as i64 - 1).max(1);
    area.connect_query_tooltip(move |area, x, _, _, tooltip| {
        let w = f64::from(area.width());
        let plot_w = (w - left - 4.0).max(10.0);
        if f64::from(x) < left {
            return false;
        }
        let t = t0 + ((f64::from(x) - left) / plot_w * (t1 - t0) as f64) as i64;
        let Some((at, close)) = candles.iter().min_by_key(|c| (c.0 - t).abs()) else { return false };
        let mut lines = vec![local_time(*at, "%a %-d %b %H:%M"), tip_price(*close)];
        for (when, buy, price) in markers.iter().filter(|m| m.0 >= *at && m.0 < at + bar.max(1)) {
            lines.push(format!("{} at {} · {}", if *buy { "Bought" } else { "Sold" }, tip_price(*price), local_time(*when, "%H:%M")));
        }
        tooltip.set_text(Some(&lines.join("\n")));
        true
    });

    // Five times under the plot.
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_homogeneous(true);
    row.set_margin_start(AXIS_W);
    row.set_margin_end(4);
    let format = if t1 - t0 > 3 * 86_400 { "%-d %b" } else { "%H:%M" };
    for i in 0..5 {
        let t = t0 + ((t1 - t0) as f64 * (f64::from(i) + 0.5) / 5.0) as i64;
        let l = label(&local_time(t, format), "threads-chart-x");
        l.set_xalign(0.5);
        row.append(&l);
    }
    body.append(&row);
    match cutoff {
        Some(c) if c <= t0 => body.append(&label(&format!("Every bar here is after the model's memory ends ({}).", local_time(c, "%-d %b %Y")), "threads-chart-note")),
        Some(c) if c >= t1 => body.append(&label("The model's memory reaches past every bar here: it may remember how this went.", "threads-chart-note")),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn written_amounts_are_decimals_in_the_currency() {
        let data = read_written(&json!({"labels": ["Nov", "Dec", "Jan"], "series": [{"name": "Savings", "values": [1234.56, "1500.5", "-20"]}]}), "CAD", ChartKind::Bar);
        assert_eq!(data.labels, ["Nov", "Dec", "Jan"]);
        assert_eq!(data.series[0].1, [Some(123_456), Some(150_050), Some(-2_000)]);
        assert!(matches!(data.fmt, Fmt::Money(_)));
        // A currency without cents counts whole units; a typographic minus and a plus read too.
        let yen = read_written(&json!({"labels": [2027, 2028], "series": [{"name": "", "values": ["1500", "\u{2212}12"]}]}), "JPY", ChartKind::Bar);
        assert_eq!(yen.labels, ["2027", "2028"]);
        assert_eq!(yen.series[0].1, [Some(1_500), Some(-12)]);
        assert_eq!(written_value(&json!("+3.5"), 2), Some(350));
        assert_eq!(written_value(&json!(0.1), 2), Some(10));
    }

    #[test]
    fn written_garbage_is_left_out_not_drawn_as_zero() {
        for garbage in [json!("$5"), json!("12 kg"), json!("n/a"), json!(null), json!(""), json!("."), json!("1.2.3"), json!(true), json!([1])] {
            assert_eq!(written_value(&garbage, 2), None, "{garbage}");
        }
        // A label no series reaches goes; one series missing a point keeps a gap there.
        let data = read_written(
            &json!({"labels": ["a", "b", "c", "d"], "series": [
                {"name": "Spent", "values": [10, "ten", null, "4"]},
                {"name": "Saved", "values": [null, null, null, 2]},
            ]}),
            "CAD",
            ChartKind::Bar,
        );
        assert_eq!(data.labels, ["a", "d"]);
        assert_eq!(data.series[0].1, [Some(1_000), Some(400)]);
        assert_eq!(data.series[1].1, [None, Some(200)]);
        assert_eq!(data.known, [2, 2]);
        // Nothing that reads at all: nothing to draw, said as the reply's.
        let none = read_written(&json!({"labels": ["a"], "series": [{"name": "x", "values": ["?"]}]}), "CAD", ChartKind::Bar);
        assert!(none.labels.is_empty());
        assert_eq!(none.empty, "Nothing to draw: the reply gave no numbers.");
    }

    #[test]
    fn a_written_unit_counts_in_tenths_of_it() {
        let kg = read_written(&json!({"labels": ["Now", "June"], "unit": "kg", "series": [{"name": "Weight", "values": [80.5, "78"]}]}), "CAD", ChartKind::Bar);
        assert_eq!(kg.series[0].1, [Some(805), Some(780)]);
        assert_eq!(kg.fmt.format(805), "80.5 kg");
        assert_eq!(kg.fmt.format_whole(780), "78 kg");
        let share = read_written(&json!({"labels": ["Rent"], "unit": "%", "series": [{"name": "", "values": [12.5]}]}), "CAD", ChartKind::Bar);
        assert_eq!(share.fmt.format(125), "12.5%");
        // An empty unit is a plain count.
        let count = read_written(&json!({"labels": ["Q1"], "unit": "", "series": [{"name": "", "values": ["3"]}]}), "CAD", ChartKind::Bar);
        assert_eq!(count.series[0].1, [Some(30)]);
        assert_eq!(count.fmt.format(30), "3");
        // A line of a measure (a weight on its way to 75 kg) starts near its lowest value, as a
        // lift's does; bars, and money, count from zero.
        let toward = json!({"labels": ["Now", "May", "June"], "unit": "kg", "series": [{"name": "Weight", "values": [80.5, 78, 75]}]});
        assert!(!read_written(&toward, "CAD", ChartKind::Line).from_zero);
        assert!(read_written(&toward, "CAD", ChartKind::Bar).from_zero);
        let balance = json!({"labels": ["Oct", "Nov"], "series": [{"name": "Balance", "values": [5000, 5200]}]});
        assert!(read_written(&balance, "CAD", ChartKind::Line).from_zero);
    }

    #[test]
    fn an_axis_fits_what_it_draws() {
        // 80.5 to 75 kg, tenths: one step for the base and the ticks.
        let (base, step, up, down) = axis(0, 805, 750, false, 1);
        assert_eq!((base, step, down), (720, 20, 0));
        assert!(base + step * i64::from(up) >= 805 && base + step * i64::from(up - 1) < 805);
        // Every month in deficit: nothing above zero, the steps all below it.
        let (base, step, up, down) = axis(-120_000, 0, -120_000, true, 2);
        assert_eq!((base, up), (0, 0));
        assert!(step * i64::from(down) >= 120_000);
        // Both sides: one step across the whole span.
        let (_, step, up, down) = axis(-50_000, 150_000, -50_000, true, 2);
        assert!(step * i64::from(up) >= 150_000 && step * i64::from(down) >= 50_000);
        assert!(up + down <= 6, "{up} + {down} ticks");
    }

    #[test]
    fn an_axis_under_one_unit_shows_its_fractions() {
        let share = Fmt::Unit(String::from("%"));
        let (step, up) = ticks(12, 1);
        assert_eq!(step, 5, "0.4% to 1.2% steps by half a percent");
        let labels: Vec<String> = (0..=i64::from(up)).map(|k| axis_label(&share, k * step, step)).collect();
        assert_eq!(labels, ["0%", "0.5%", "1%", "1.5%"]);
        // Whole steps read whole, money included.
        let kg = Fmt::Unit(String::from("kg"));
        assert_eq!(axis_label(&kg, 800, 50), "80 kg");
        let dollars = Fmt::Money(MoneyFormatter::new("CAD", Locale::from_env()));
        assert_eq!(axis_label(&dollars, 50_000, 10_000), dollars.format_whole(50_000));
    }

    #[test]
    fn a_projected_series_is_marked_and_named_so() {
        let data = read_written(
            &json!({"labels": ["Oct", "Nov", "Dec"], "series": [
                {"name": "Balance", "values": [100, 120, null]},
                {"name": "Balance", "values": [null, "120", "150.25"], "projected": true},
            ]}),
            "CAD",
            ChartKind::Line,
        );
        assert_eq!(data.projected, [false, true]);
        assert_eq!(data.series[1].1, [None, Some(12_000), Some(15_025)]);
        assert_eq!(legend_name("Balance", true), "Balance (projected)");
        assert_eq!(legend_name("Projected savings", true), "Projected savings");
        assert_eq!(legend_name("", true), "Projected");
        assert_eq!(legend_name("Balance", false), "Balance");
        // Only a literal true projects.
        let loose = read_written(&json!({"labels": ["a"], "series": [{"name": "x", "values": [1], "projected": "yes"}]}), "CAD", ChartKind::Bar);
        assert_eq!(loose.projected, [false]);
    }

    #[test]
    fn a_training_chart_leaves_a_missing_week_out_of_every_series() {
        let data = read_gym(
            &json!({"labels": ["W1", "W2", "W3"], "unit": "lb", "measure": "volume", "series": [
                {"name": "Bench", "values": [100.0, null, 110.0]},
                {"name": "Squat", "values": [null, null, 200.0]},
            ]}),
            ChartKind::Line,
        );
        assert_eq!(data.labels, ["W1", "W3"]);
        assert_eq!(data.series[1].1, [None, Some(2_000)]);
        // Each series has its own reach, so a second line is drawn, not indexed past the first.
        assert_eq!(data.known.len(), 2);
    }

    #[test]
    fn a_price_block_keeps_its_question_and_memory_line() {
        let spec = price_spec(r#"{"type":"price","title":"ETH","arbiter":{"product":"ETH-CAD","granularity":"ONE_HOUR","bars":300,"strategy_id":2},"cutoff":"2025-03-01"}"#).unwrap();
        assert_eq!(spec.query["product"], "ETH-CAD");
        assert_eq!(spec.cutoff, Some(1_740_787_200));
        assert!(price_spec(r#"{"type":"bar","query":{}}"#).is_none());
        assert!(price_spec(r#"{"type":"price"}"#).is_none());
    }

    #[test]
    fn price_axes_round_and_group() {
        let (step, bottom, top) = price_ticks(3912.0, 4188.0);
        assert_eq!(step, 100.0);
        assert_eq!((bottom, top), (3900.0, 4200.0));
        assert_eq!(axis_price(4200.0, step), "4,200");
        assert_eq!(axis_price(0.2134, 0.01), "0.21");
        assert_eq!(tip_price(4123.456), "4,123.46");
    }
}
