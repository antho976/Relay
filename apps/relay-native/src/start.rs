//! The start screen's look: the Relay lockup over a field of dots, a greeting, and one card per
//! space, opened with one animation and kept alive by a quiet pulse. What a card does and what its
//! live line says is `money.rs`'s.
//!
//! The mark is three lanes staggered like a relay's start, on a 120 grid, stroke 16 with round
//! caps, in ink: (y, from x, to x) = (87, 23, 77), (61, 37, 91), (35, 51, 71), back to front. The
//! signal, r10, has broken away ahead of the front lane and rests at (95, 35).
//! The wordmark is Sora 600.
//!
//! The opening runs once when the screen opens, about 1.9 s, all from one frame-clock tick: the
//! lanes draw in one after another from the back, each with a lit tip that cools as it lands; the
//! front lane's tip is the signal, which breaks away to its place with a short streak, and on
//! landing it kicks back through the lanes and sends a wave across the field.
//! The wordmark spells itself in, then the greeting, then the cards one after the other. After
//! that the signal beats and sends a fainter wave every few seconds, until a card is picked; the
//! screen then fades out. With animations off (GTK's setting) it opens at its last frame and
//! holds still. `RELAY_NATIVE_START_T=<0..1>` holds it at one point of the opening, for
//! screenshots.
use crate::app::{button, label};
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

/// The whole opening, in microseconds of frame-clock time.
const DURATION_US: f64 = 1_900_000.0;
/// After the opening, a pulse every `PULSE_EVERY_US`; each wave takes `PULSE_US` to cross.
const PULSE_EVERY_US: f64 = 4_800_000.0;
const PULSE_US: f64 = 2_600_000.0;
/// The fade when a card is picked.
const LEAVE_US: f64 = 260_000.0;

const INK: (f64, f64, f64) = (0xED as f64 / 255.0, 0xEE as f64 / 255.0, 0xF0 as f64 / 255.0);
const DIM: (f64, f64, f64) = (0x63 as f64 / 255.0, 0x66 as f64 / 255.0, 0x6B as f64 / 255.0);
const SIGNAL: (f64, f64, f64) = (0xC6 as f64 / 255.0, 0xF2 as f64 / 255.0, 0x4E as f64 / 255.0);

/// Where each part's motion sits on the 0..1 timeline: (from, to).
/// The lanes, back to front: each draws in from its start, overshoots a little and settles.
const LANES: [(f64, f64); 3] = [(0.02, 0.30), (0.07, 0.35), (0.12, 0.40)];
/// The signal breaking away from the front lane's tip.
const SIGNAL_IN: (f64, f64) = (0.40, 0.56);
/// The kick back through the lanes as the signal lands, front lane first, each `RECOIL_STEP` later.
const RECOIL: (f64, f64) = (0.50, 0.64);
const RECOIL_STEP: f64 = 0.03;
/// The dots coming up, from the mark outward.
const WAVE: (f64, f64) = (0.0, 0.7);
/// The wave the signal sends across the field as it lands.
const PING: (f64, f64) = (0.52, 1.0);
const WORDMARK: (f64, f64) = (0.36, 0.66);
const GREETING: (f64, f64) = (0.50, 0.78);
const QUESTION: (f64, f64) = (0.56, 0.84);
const CARD_DEV: (f64, f64) = (0.62, 0.92);
const CARD_THREADS: (f64, f64) = (0.70, 1.0);
const HINT: (f64, f64) = (0.84, 1.0);

/// The mark on its 120 grid: each lane (y, from x, to x), back to front, and the stroke.
const LANE: [(f64, f64, f64); 3] = [(87.0, 23.0, 77.0), (61.0, 37.0, 91.0), (35.0, 51.0, 71.0)];
const STROKE: f64 = 16.0;
/// The signal at rest, ahead of the front lane, and its radius.
const SIGNAL_AT: (f64, f64) = (95.0, 35.0);
const SIGNAL_DOT: f64 = 10.0;
/// The lit trail behind a moving lane's head, and how far the landing kicks the lanes back.
const TRAIL: f64 = 24.0;
const RECOIL_UNITS: f64 = 3.0;
/// Field dots: one every `DOT_STEP` pixels.
const DOT_STEP: f64 = 26.0;
/// A settled dot's alpha, at the mark; it dims toward the far corners.
const BASE_DOT: f64 = 0.11;

/// What `build` hands back: the screen, each card's live line, the box for its recent rows, and
/// the cards (Dev, Threads).
pub struct Built {
    pub screen: gtk::Box,
    pub lines: [gtk::Label; 2],
    pub rows: [gtk::Box; 2],
    pub keys: [gtk::Button; 2],
}

/// The lamp beside a card's live line: grey, signal-lit, or amber when something waits on you.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lamp {
    Idle,
    Live,
    Attention,
}

/// `t` (0..1) within `span`, eased out: fast start, soft landing.
fn phase(t: f64, (from, to): (f64, f64)) -> f64 {
    let x = ((t - from) / (to - from)).clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

/// Like `phase`, with a small overshoot: the signal arrives and settles.
fn phase_back(t: f64, (from, to): (f64, f64)) -> f64 {
    let x = ((t - from) / (to - from)).clamp(0.0, 1.0);
    let c = 1.70158;
    1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
}

/// The mark at timeline point `t`, in a `size` square. The title bar draws it settled (`t` = 1).
pub(crate) fn draw_mark(cr: &gtk::cairo::Context, size: f64, t: f64) {
    draw_mark_beat(cr, size, t, 0.0);
}

/// Where the signal rests, on the 120 grid.
fn signal_rest() -> (f64, f64) {
    SIGNAL_AT
}

/// How far lane `order` (0 is the front) is kicked back at `t`, 0..1: out and home again.
fn recoil(t: f64, order: usize) -> f64 {
    let from = RECOIL.0 + RECOIL_STEP * order as f64;
    let x = ((t - from) / (RECOIL.1 - RECOIL.0)).clamp(0.0, 1.0);
    (PI * x).sin() * (1.0 - x)
}

/// One lane drawn from its start to `reach` of its length, `shift` units back, with a lit trail
/// behind its head while `lit` (0..1).
fn draw_lane(cr: &gtk::cairo::Context, (y, from, to): (f64, f64, f64), reach: f64, lit: f64, shift: f64) {
    let (a, b) = (from - shift, from + (to - from) * reach - shift);
    if b - a < 0.5 {
        return;
    }
    let (ir, ig, ib) = INK;
    let (red, green, blue) = SIGNAL;
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_source_rgb(ir, ig, ib);
    cr.move_to(a, y);
    cr.line_to(b, y);
    let _ = cr.stroke();
    if lit <= 0.0 {
        return;
    }
    let tail = (b - a).min(TRAIL);
    cr.set_line_cap(gtk::cairo::LineCap::Butt);
    for step in 0..8 {
        let x = b - tail * f64::from(8 - step) / 8.0;
        cr.set_source_rgba(red, green, blue, lit * (0.12 + 0.11 * f64::from(step)));
        cr.move_to(x, y);
        cr.line_to(x + tail / 8.0 + 0.6, y);
        let _ = cr.stroke();
    }
    cr.set_source_rgba(red, green, blue, lit);
    cr.arc(b, y, STROKE / 2.0, 0.0, 2.0 * PI);
    let _ = cr.fill();
}

/// The mark at `t`, its signal swollen by `beat` (0..1, the ambient pulse's heartbeat).
fn draw_mark_beat(cr: &gtk::cairo::Context, size: f64, t: f64, beat: f64) {
    let s = size / 120.0;
    cr.scale(s, s);
    cr.set_line_width(STROKE);
    let front = LANE.len() - 1;
    // The lanes come in from the back, each lit at its head until it lands; the front lane's
    // lit head is the signal, so it stays lit until the signal leaves it.
    for (k, (&lane, &span)) in LANE.iter().zip(LANES.iter()).enumerate() {
        let lit = if k == front {
            if t < SIGNAL_IN.0 { 1.0 } else { 0.0 }
        } else {
            1.0 - phase(t, (span.1 - 0.06, span.1 + 0.08))
        };
        draw_lane(cr, lane, phase_back(t, span), lit, RECOIL_UNITS * recoil(t, front - k));
    }
    if t < SIGNAL_IN.0 {
        return;
    }
    // The signal breaks away from the front lane's tip, a streak behind it, and settles ahead.
    let (red, green, blue) = SIGNAL;
    let (y, _, end) = LANE[front];
    let travel = phase_back(t, SIGNAL_IN);
    let x = end + (SIGNAL_AT.0 - end) * travel;
    let r = STROKE / 2.0 + (SIGNAL_DOT - STROKE / 2.0) * travel + 1.6 * beat;
    let streak = 1.0 - phase(t, SIGNAL_IN);
    if streak > 0.01 {
        cr.set_line_cap(gtk::cairo::LineCap::Butt);
        cr.set_line_width(r * 1.2);
        cr.set_source_rgba(red, green, blue, 0.55 * streak);
        cr.move_to((x - 30.0 * streak).max(end), y);
        cr.line_to(x, y);
        let _ = cr.stroke();
    }
    cr.set_source_rgb(red, green, blue);
    cr.arc(x, y, r, 0.0, 2.0 * PI);
    let _ = cr.fill();
}

/// One wave over the field: from the signal, `progress` (0..1) of the way out, at `strength`.
#[derive(Clone, Copy)]
struct Wave {
    progress: f64,
    strength: f64,
}

/// The field behind the lockup: a lime glow around the mark and a grid of dots that the
/// opening wave uncovers, dimming toward the edges. `signal` is where waves start, `glow` the
/// mark's centre, both in the field's pixels.
fn draw_field(cr: &gtk::cairo::Context, (w, h): (f64, f64), signal: (f64, f64), glow: (f64, f64), t: f64, pulses: &[Wave]) {
    let (red, green, blue) = SIGNAL;
    // The landing's wave runs at an even speed, stronger than the pulses after it.
    let mut waves = pulses.to_vec();
    let landing = ((t - PING.0) / (PING.1 - PING.0)).clamp(0.0, 1.0);
    if landing > 0.0 && landing < 1.0 {
        waves.push(Wave { progress: landing, strength: 0.9 });
    }
    let lit = phase(t, WAVE);
    let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
        .iter()
        .map(|&(x, y)| (x - signal.0).hypot(y - signal.1))
        .fold(1.0, f64::max);

    // The glow: soft, wide, and only as strong as the opening has come.
    let pulse = waves.iter().map(|wave| (1.0 - wave.progress) * wave.strength).fold(0.0, f64::max);
    let glow_alpha = 0.085 * lit + 0.03 * pulse;
    let gradient = gtk::cairo::RadialGradient::new(glow.0, glow.1, 0.0, glow.0, glow.1, 460.0);
    gradient.add_color_stop_rgba(0.0, red, green, blue, glow_alpha);
    gradient.add_color_stop_rgba(0.45, red, green, blue, glow_alpha * 0.32);
    gradient.add_color_stop_rgba(1.0, red, green, blue, 0.0);
    let _ = cr.set_source(&gradient);
    let _ = cr.paint();

    // Dots, bucketed by alpha so each layer is a single fill.
    const LEVELS: usize = 20;
    let mut base: Vec<Vec<(f64, f64)>> = vec![Vec::new(); LEVELS];
    let mut hot: Vec<Vec<(f64, f64)>> = vec![Vec::new(); LEVELS];
    let front = lit * (far + 160.0);
    let (x0, y0) = (signal.0.rem_euclid(DOT_STEP), signal.1.rem_euclid(DOT_STEP));
    let mut y = y0;
    while y < h {
        let mut x = x0;
        while x < w {
            let d = (x - signal.0).hypot(y - signal.1);
            let fade = (1.0 - (d / far).powf(1.15)).max(0.0);
            let shown = ((front - d) / 110.0).clamp(0.0, 1.0);
            let a = BASE_DOT * shown * fade;
            if a > 0.004 {
                base[((a / BASE_DOT) * (LEVELS - 1) as f64).round() as usize].push((x, y));
            }
            let mut heat: f64 = 0.0;
            for wave in &waves {
                let at = wave.progress * far * 0.85;
                heat = heat.max(wave.strength * (1.0 - wave.progress) * (-((d - at) / 70.0).powi(2)).exp());
            }
            let heat = heat * (0.35 + 0.65 * fade);
            if heat > 0.02 {
                hot[((heat.min(1.0)) * (LEVELS - 1) as f64).round() as usize].push((x, y));
            }
            x += DOT_STEP;
        }
        y += DOT_STEP;
    }
    for (level, dots) in base.iter().enumerate() {
        if dots.is_empty() {
            continue;
        }
        cr.set_source_rgba(1.0, 1.0, 1.0, BASE_DOT * level as f64 / (LEVELS - 1) as f64);
        for &(x, y) in dots {
            cr.rectangle(x - 1.0, y - 1.0, 2.0, 2.0);
        }
        let _ = cr.fill();
    }
    for (level, dots) in hot.iter().enumerate() {
        if dots.is_empty() {
            continue;
        }
        cr.set_source_rgba(red, green, blue, 0.8 * level as f64 / (LEVELS - 1) as f64);
        for &(x, y) in dots {
            cr.rectangle(x - 1.25, y - 1.25, 2.5, 2.5);
        }
        let _ = cr.fill();
    }

    // Each wave starts as a ring around the signal.
    for wave in &waves {
        let p = (wave.progress * 2.2).min(1.0);
        if p >= 1.0 {
            continue;
        }
        let strength = wave.strength;
        let p = 1.0 - (1.0 - p).powi(3);
        cr.set_source_rgba(red, green, blue, 0.6 * (1.0 - p) * strength);
        cr.set_line_width(1.5);
        cr.new_sub_path();
        cr.arc(signal.0, signal.1, 6.0 + 40.0 * p, 0.0, 2.0 * PI);
        let _ = cr.stroke();
    }
}

/// A widget's place on the timeline: it fades in over `span` and rises `rise` pixels into place.
/// The rise moves margin from above it to below it, so nothing around it shifts while it runs.
fn reveal(widget: &impl IsA<gtk::Widget>, t: f64, span: (f64, f64), rise: i32) {
    let p = phase(t, span);
    let above = ((1.0 - p) * f64::from(rise)).round() as i32;
    widget.set_opacity(p);
    widget.set_margin_top(above);
    widget.set_margin_bottom(rise - above);
}

/// The wordmark spelt in: each letter fades up a moment after the one before it.
fn spell(wordmark: &gtk::Label, t: f64) {
    let text = wordmark.text();
    let count = text.chars().count().max(1);
    let attrs = gtk::pango::AttrList::new();
    let mut byte = 0u32;
    for (i, ch) in text.chars().enumerate() {
        let width = (WORDMARK.1 - WORDMARK.0) * 0.55;
        let from = WORDMARK.0 + (WORDMARK.1 - WORDMARK.0 - width) * i as f64 / (count - 1).max(1) as f64;
        let a = phase(t, (from, from + width));
        let mut attr = gtk::pango::AttrInt::new_foreground_alpha((a * 65535.0).round().max(1.0) as u16);
        attr.set_start_index(byte);
        byte += ch.len_utf8() as u32;
        attr.set_end_index(byte);
        attrs.insert(attr);
    }
    wordmark.set_attributes(Some(&attrs));
}

/// Whether GTK animates at all.
fn animating() -> bool {
    gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())
}

/// Where the opening stands: held by `RELAY_NATIVE_START_T`, finished when animations are off,
/// else `None` (run it).
fn fixed_point() -> Option<f64> {
    if let Some(t) = std::env::var("RELAY_NATIVE_START_T").ok().and_then(|v| v.parse::<f64>().ok()) {
        return Some(t.clamp(0.0, 1.0));
    }
    (!animating()).then_some(1.0)
}

/// The small line drawing on each card: a prompt for Dev, two speech bubbles for Threads.
fn draw_glyph(cr: &gtk::cairo::Context, size: f64, threads: bool) {
    let s = size / 24.0;
    cr.scale(s, s);
    cr.set_line_width(1.6);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    let (ir, ig, ib) = INK;
    let (dr, dg, db) = DIM;
    if threads {
        // A bubble with its tail, and a second, smaller one answering it.
        let bubble = |cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64| {
            let r = 3.0;
            cr.new_sub_path();
            cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
            cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
            cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
            cr.arc(x + r, y + r, r, PI, 1.5 * PI);
            cr.close_path();
        };
        bubble(cr, 3.0, 4.0, 13.0, 9.5);
        cr.move_to(6.0, 13.5);
        cr.line_to(5.0, 17.0);
        cr.line_to(9.5, 13.5);
        cr.set_source_rgb(ir, ig, ib);
        let _ = cr.stroke();
        bubble(cr, 11.0, 11.5, 10.0, 7.5);
        cr.set_source_rgb(dr, dg, db);
        let _ = cr.stroke();
    } else {
        // A prompt and a cursor.
        cr.move_to(4.5, 7.0);
        cr.line_to(9.5, 12.0);
        cr.line_to(4.5, 17.0);
        cr.set_source_rgb(ir, ig, ib);
        let _ = cr.stroke();
        cr.move_to(12.5, 17.0);
        cr.line_to(19.0, 17.0);
        cr.set_source_rgb(dr, dg, db);
        let _ = cr.stroke();
    }
}

/// The screen for `greeting`, with the Dev and Threads cards; `remembered` is the space Escape
/// opens (true is Threads). `pick(true)` is Threads.
pub fn build(greeting: &str, remembered: bool, pick: Rc<dyn Fn(bool)>) -> Built {
    let screen = gtk::Box::new(gtk::Orientation::Vertical, 0);
    screen.add_css_class("start-screen");
    screen.set_widget_name("start-screen");
    screen.set_focusable(true);
    let stage = gtk::Overlay::new();
    stage.set_vexpand(true);
    let field = gtk::DrawingArea::new();
    field.set_can_target(false);
    field.set_hexpand(true);
    field.set_vexpand(true);
    stage.set_child(Some(&field));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_halign(gtk::Align::Center);
    column.set_valign(gtk::Align::Center);

    let lockup = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    lockup.add_css_class("start-lockup");
    lockup.set_halign(gtk::Align::Center);
    // The wordmark ends its slide with 14px after it; the same before keeps the lockup centred.
    lockup.set_margin_start(14);
    let t = Rc::new(Cell::new(fixed_point().unwrap_or(0.0)));
    let beat = Rc::new(Cell::new(0.0));
    let waves: Rc<std::cell::RefCell<Vec<Wave>>> = Rc::default();
    let mark = gtk::DrawingArea::new();
    // On the brand's lockup the ring stands a little taller than the wordmark's ascenders.
    mark.set_content_width(88);
    mark.set_content_height(88);
    mark.set_valign(gtk::Align::Center);
    mark.set_accessible_role(gtk::AccessibleRole::Img);
    mark.update_property(&[gtk::accessible::Property::Label("Relay")]);
    let (at, beating) = (t.clone(), beat.clone());
    mark.set_draw_func(move |_, cr, w, h| draw_mark_beat(cr, f64::from(w.min(h)), at.get(), beating.get()));
    lockup.append(&mark);
    let wordmark = label("relay", "start-wordmark");
    wordmark.set_valign(gtk::Align::Center);
    lockup.append(&wordmark);
    column.append(&lockup);

    {
        let (at, mark, waves) = (t.clone(), mark.clone(), waves.clone());
        field.set_draw_func(move |field, cr, w, h| {
            let size = (f64::from(w), f64::from(h));
            // Where the mark sits in the field; its centre before the first layout.
            let (signal, glow) = match mark.compute_bounds(field) {
                Some(b) if b.width() > 0.0 => {
                    let s = f64::from(b.width().min(b.height())) / 120.0;
                    let (x, y) = (f64::from(b.x()), f64::from(b.y()));
                    let (sx, sy) = signal_rest();
                    ((x + sx * s, y + sy * s), (x + 60.0 * s, y + 60.0 * s))
                }
                _ => ((size.0 / 2.0, size.1 * 0.3), (size.0 / 2.0, size.1 * 0.3)),
            };
            // Faded in from the top, so the glow meets the title bar without a seam.
            cr.push_group();
            draw_field(cr, size, signal, glow, at.get(), &waves.borrow());
            let _ = cr.pop_group_to_source();
            let edge = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, (glow.1 * 0.9).max(1.0));
            edge.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
            edge.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
            let _ = cr.mask(&edge);
        });
    }

    let words = gtk::Box::new(gtk::Orientation::Vertical, 8);
    words.add_css_class("start-words");
    let hello = label(greeting, "start-greeting");
    hello.set_xalign(0.5);
    hello.set_wrap(true);
    words.append(&hello);
    let ask = label("Where do you want to start?", "start-question");
    ask.set_xalign(0.5);
    words.append(&ask);
    column.append(&words);

    let cards = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    cards.add_css_class("start-cards");
    cards.set_halign(gtk::Align::Center);
    cards.set_homogeneous(true);
    let mut lines = Vec::new();
    let mut recents = Vec::new();
    let mut keys = Vec::new();
    for (index, (to_threads, title, about)) in [
        (false, "Dev", "Agents, tasks and code across your projects"),
        (true, "Threads", "Talk it through with agents that know your data"),
    ]
    .into_iter()
    .enumerate()
    {
        let card = button("", "start-card");
        card.add_css_class(if to_threads { "threads" } else { "dev" });
        card.set_widget_name(if to_threads { "start-money" } else { "start-dev" });
        card.set_tooltip_text(Some(&format!("Open {title}")));
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let glyph = gtk::DrawingArea::new();
        glyph.add_css_class("start-glyph");
        glyph.set_content_width(22);
        glyph.set_content_height(22);
        glyph.set_valign(gtk::Align::Center);
        glyph.set_draw_func(move |_, cr, w, h| draw_glyph(cr, f64::from(w.min(h)), to_threads));
        top.append(&glyph);
        let name = label(title, "start-title");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        top.append(&name);
        // The key that picks it, where a chevron would be: the card says how to reach it.
        let key = label(&(index + 1).to_string(), "start-key");
        key.set_valign(gtk::Align::Center);
        top.append(&key);
        inner.append(&top);
        let what = label(about, "start-about");
        what.set_xalign(0.0);
        what.set_wrap(true);
        what.set_max_width_chars(34);
        inner.append(&what);
        let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rule.add_css_class("start-rule");
        inner.append(&rule);
        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        foot.add_css_class("start-foot");
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("start-live");
        dot.set_valign(gtk::Align::Center);
        foot.append(&dot);
        let line = label("Reading…", "start-line");
        line.add_css_class("pending");
        line.set_xalign(0.0);
        line.set_hexpand(true);
        line.set_ellipsize(gtk::pango::EllipsizeMode::End);
        // Readings fill the card's width rather than set it.
        line.set_max_width_chars(1);
        foot.append(&line);
        inner.append(&foot);
        // Where you left off: two rows, their height held before they are read.
        let recent = gtk::Box::new(gtk::Orientation::Vertical, 0);
        recent.add_css_class("start-rows");
        inner.append(&recent);
        card.set_child(Some(&inner));
        let pick = pick.clone();
        card.connect_clicked(move |_| pick(to_threads));
        cards.append(&card);
        lines.push(line);
        recents.push(recent);
        keys.push(card);
    }
    column.append(&cards);

    let hint = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    hint.add_css_class("start-hint");
    hint.set_halign(gtk::Align::Center);
    let esc = label("Esc", "start-key");
    esc.set_valign(gtk::Align::Center);
    hint.append(&esc);
    hint.append(&label(&format!("opens {}", if remembered { "Threads" } else { "Dev" }), "start-hint-text"));
    column.append(&hint);
    stage.add_overlay(&column);
    screen.append(&stage);

    let apply = {
        let (field, mark, wordmark, hello, ask, hint) =
            (field.clone(), mark.clone(), wordmark.clone(), hello.clone(), ask.clone(), hint.clone());
        let cards: Vec<gtk::Button> = keys.clone();
        move |t: f64| {
            mark.queue_draw();
            field.queue_draw();
            let w = phase(t, WORDMARK);
            let before = ((1.0 - w) * 14.0).round() as i32;
            wordmark.set_opacity(phase(t, (WORDMARK.0, WORDMARK.0 + 0.08)));
            wordmark.set_margin_start(before);
            wordmark.set_margin_end(14 - before);
            spell(&wordmark, t);
            reveal(&hello, t, GREETING, 12);
            reveal(&ask, t, QUESTION, 8);
            reveal(&cards[0], t, CARD_DEV, 22);
            reveal(&cards[1], t, CARD_THREADS, 22);
            reveal(&hint, t, HINT, 0);
        }
    };
    apply(t.get());
    if fixed_point().is_none() {
        let start = Cell::new(None::<i64>);
        let (field, mark) = (field.clone(), mark.clone());
        screen.add_tick_callback(move |_, clock| {
            let now = clock.frame_time();
            let began = start.get().unwrap_or(now);
            start.set(Some(began));
            let elapsed = (now - began) as f64;
            if t.get() < 1.0 {
                let p = (elapsed / DURATION_US).clamp(0.0, 1.0);
                t.set(p);
                apply(p);
                return glib::ControlFlow::Continue;
            }
            // Settled: a pulse every few seconds, the first one shortly after the opening.
            let since = elapsed - DURATION_US - 900_000.0;
            let mut list = waves.borrow_mut();
            let before = !list.is_empty() || beat.get() > 0.0;
            list.clear();
            if since >= 0.0 {
                let into = since.rem_euclid(PULSE_EVERY_US);
                if into < PULSE_US {
                    list.push(Wave { progress: into / PULSE_US, strength: 0.55 });
                }
                // The signal swells and eases back as the wave leaves it.
                let b = (into / 420_000.0).min(1.0);
                beat.set(if into < 420_000.0 { (b * PI).sin() } else { 0.0 });
            }
            if !list.is_empty() || before {
                field.queue_draw();
            }
            if beat.get() > 0.0 || before {
                mark.queue_draw();
            }
            glib::ControlFlow::Continue
        });
    }
    let [dev, threads]: [gtk::Label; 2] = lines.try_into().expect("two cards");
    let [dev_rows, threads_rows]: [gtk::Box; 2] = recents.try_into().expect("two cards");
    let [dev_key, threads_key]: [gtk::Button; 2] = keys.try_into().expect("two cards");
    Built { screen, lines: [dev, threads], rows: [dev_rows, threads_rows], keys: [dev_key, threads_key] }
}

/// Puts a card's live reading in place of its "Reading…", its lamp lit as `lamp` says.
pub fn reading(line: &gtk::Label, text: &str, lamp: Lamp) {
    line.set_text(text);
    line.set_tooltip_text(Some(text));
    line.remove_css_class("pending");
    if let Some(dot) = line.prev_sibling() {
        dot.add_css_class(match lamp {
            Lamp::Idle => "idle",
            Lamp::Live => "live",
            Lamp::Attention => "attention",
        });
    }
    line.add_css_class(if lamp == Lamp::Attention { "attention" } else { "read" });
    fade_in(line);
}

/// Fills a card's recent rows: each a name and, at its end, a short detail. With none, `empty`
/// says so in their place.
pub fn recent(rows: &gtk::Box, items: &[(String, String)], empty: &str) {
    while let Some(child) = rows.first_child() {
        rows.remove(&child);
    }
    if items.is_empty() {
        let none = label(empty, "start-row-empty");
        none.set_xalign(0.0);
        rows.append(&none);
    }
    for (name, detail) in items.iter().take(2) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("start-row");
        let title = label(name, "start-row-name");
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(1);
        title.set_tooltip_text(Some(name));
        row.append(&title);
        row.append(&label(detail, "start-row-detail"));
        rows.append(&row);
    }
    fade_in(rows);
}

/// A reading arriving: it fades up rather than appearing, unless nothing animates.
fn fade_in(widget: &impl IsA<gtk::Widget>) {
    if !animating() || std::env::var_os("RELAY_NATIVE_START_T").is_some() {
        return;
    }
    widget.set_opacity(0.0);
    let start = Cell::new(None::<i64>);
    widget.add_tick_callback(move |widget, clock| {
        let now = clock.frame_time();
        let began = start.get().unwrap_or(now);
        start.set(Some(began));
        let p = phase((now - began) as f64 / 320_000.0, (0.0, 1.0));
        widget.set_opacity(p);
        if p >= 1.0 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
    });
}

/// Fades the screen out and lifts its column a little, then calls `done` (which removes it).
/// Nothing on it takes the pointer meanwhile. With animations off it is immediate.
pub fn leave(screen: &gtk::Widget, done: impl FnOnce() + 'static) {
    screen.set_can_target(false);
    if !animating() {
        done();
        return;
    }
    let start = Cell::new(None::<i64>);
    let done = Cell::new(Some(done));
    screen.add_tick_callback(move |screen, clock| {
        let now = clock.frame_time();
        let began = start.get().unwrap_or(now);
        start.set(Some(began));
        let p = ((now - began) as f64 / LEAVE_US).clamp(0.0, 1.0);
        let eased = p * p;
        screen.set_opacity(1.0 - eased);
        if p >= 1.0 {
            if let Some(done) = done.take() {
                done();
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_part_starts_hidden_and_ends_in_place() {
        for span in [WAVE, PING, WORDMARK, GREETING, QUESTION, CARD_DEV, CARD_THREADS, HINT] {
            assert_eq!(phase(0.0, span), 0.0);
            assert!((phase(1.0, span) - 1.0).abs() < 1e-9);
        }
        for span in LANES.into_iter().chain([SIGNAL_IN]) {
            assert!(phase_back(0.0, span).abs() < 1e-9);
            assert!((phase_back(1.0, span) - 1.0).abs() < 1e-9);
        }
        // The kick back is over, for every lane, before the opening ends.
        for order in 0..LANE.len() {
            assert_eq!(recoil(0.0, order), 0.0);
            assert!(recoil(1.0, order).abs() < 1e-9);
        }
    }

    #[test]
    fn the_lanes_come_in_back_to_front_and_stagger_like_a_start() {
        for pair in LANES.windows(2) {
            assert!(pair[1].0 > pair[0].0 && pair[1].1 > pair[0].1);
        }
        let step = LANE[1].1 - LANE[0].1;
        for pair in LANE.windows(2) {
            // Each lane is above the one behind it and starts the same step further on.
            assert!(pair[1].0 < pair[0].0);
            assert!((pair[1].1 - pair[0].1 - step).abs() < 1e-9);
        }
    }

    #[test]
    fn the_signal_leaves_the_front_lane_as_it_lands_and_rests_clear_of_it_inside_the_grid() {
        let (y, _, end) = LANE[LANE.len() - 1];
        assert!((SIGNAL_IN.0 - LANES[LANES.len() - 1].1).abs() < 1e-9);
        assert_eq!(signal_rest().1, y);
        // A visible gap between the end of the front lane's cap and the dot.
        assert!(SIGNAL_AT.0 - SIGNAL_DOT - (end + STROKE / 2.0) > 4.0);
        // The whole mark sits about the grid's middle.
        let left = LANE.iter().map(|l| l.1).fold(f64::MAX, f64::min) - STROKE / 2.0;
        let right = (SIGNAL_AT.0 + SIGNAL_DOT).max(LANE.iter().map(|l| l.2).fold(0.0, f64::max) + STROKE / 2.0);
        let top = SIGNAL_AT.1 - SIGNAL_DOT.max(STROKE / 2.0);
        let bottom = LANE[0].0 + STROKE / 2.0;
        assert!(((left + right) / 2.0 - 60.0).abs() < 2.0);
        assert!(((top + bottom) / 2.0 - 60.0).abs() < 2.0);
    }

    #[test]
    fn the_words_follow_the_mark_and_the_cards_come_one_after_the_other() {
        assert!(WORDMARK.0 >= LANES[0].0 && GREETING.0 > WORDMARK.0 && QUESTION.0 > GREETING.0);
        assert!(CARD_DEV.0 > QUESTION.0 - 0.1 && CARD_THREADS.0 > CARD_DEV.0);
        assert!(CARD_THREADS.1 <= 1.0 && HINT.1 <= 1.0 && PING.0 < SIGNAL_IN.1);
        assert!(RECOIL.0 >= PING.0 - 0.05 && RECOIL.1 + RECOIL_STEP * 2.0 <= 1.0);
    }
}
