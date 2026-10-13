//! The start screen's look: the Relay lockup over a field of dots, a greeting, and one card per
//! space, opened with one animation and kept alive by a quiet pulse. What a card does and what its
//! live line says is `money.rs`'s.
//!
//! The mark is a lowercase r written in one stroke on a 120 grid: a stem rising into a shoulder
//! (centre (58, 58), r29, from 180° to 288°), stroke 17 with round caps and joins, in ink; the
//! signal, r10, has left the end of the stroke and rests a little ahead of it, at 333° and 36 out.
//! The wordmark is Sora 600.
//!
//! The opening runs once when the screen opens, about 1.9 s, all from one frame-clock tick: the
//! r writes itself, stem then shoulder, with the signal as the pen's lit tip; at the end of the
//! stroke the signal leaves it for its place, and on landing it sends a wave across the field.
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
const WRITE: (f64, f64) = (0.02, 0.40);
const SIGNAL_IN: (f64, f64) = (0.40, 0.54);
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

/// The r on its 120 grid: the shoulder's centre and radius, the stem's length below that
/// centre, where the shoulder ends (degrees, clockwise from three o'clock) and the stroke.
const CENTRE: (f64, f64) = (58.0, 58.0);
const RADIUS: f64 = 29.0;
const STEM: f64 = 33.0;
const SHOULDER_END: f64 = 288.0;
const STROKE: f64 = 17.0;
/// The signal at rest, in polar terms around the shoulder's centre, and its radius.
const SIGNAL_ANGLE: f64 = 333.0;
const SIGNAL_RADIUS: f64 = 36.0;
const SIGNAL_DOT: f64 = 10.0;
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

/// `t` (0..1) within `span`, eased in and out: for travel between two rests.
fn phase_smooth(t: f64, (from, to): (f64, f64)) -> f64 {
    let x = ((t - from) / (to - from)).clamp(0.0, 1.0);
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
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

/// The stroke's length: the stem, then the shoulder.
fn stroke_length() -> f64 {
    STEM + RADIUS * (SHOULDER_END - 180.0).to_radians()
}

/// Where the signal rests, on the 120 grid.
fn signal_rest() -> (f64, f64) {
    let a = SIGNAL_ANGLE.to_radians();
    (CENTRE.0 + SIGNAL_RADIUS * a.cos(), CENTRE.1 + SIGNAL_RADIUS * a.sin())
}

/// The point `along` the stroke, from the foot of the stem.
fn stroke_point(along: f64) -> (f64, f64) {
    if along <= STEM {
        return (CENTRE.0 - RADIUS, CENTRE.1 + STEM - along);
    }
    let a = PI + (along - STEM) / RADIUS;
    (CENTRE.0 + RADIUS * a.cos(), CENTRE.1 + RADIUS * a.sin())
}

/// Adds the stretch of the stroke from `from` to `to` (lengths from the foot) as one sub-path.
fn trace(cr: &gtk::cairo::Context, from: f64, to: f64) {
    let to = to.min(stroke_length());
    cr.new_sub_path();
    if from < STEM {
        let (x, y) = stroke_point(from);
        cr.move_to(x, y);
        let (x, y) = stroke_point(to.min(STEM));
        cr.line_to(x, y);
    }
    if to > STEM {
        let a0 = PI + (from.max(STEM) - STEM) / RADIUS;
        let a1 = PI + (to - STEM) / RADIUS;
        cr.arc(CENTRE.0, CENTRE.1, RADIUS, a0, a1);
    }
}

/// The mark at `t`, its signal swollen by `beat` (0..1, the ambient pulse's heartbeat).
fn draw_mark_beat(cr: &gtk::cairo::Context, size: f64, t: f64, beat: f64) {
    let s = size / 120.0;
    cr.scale(s, s);
    cr.set_line_width(STROKE);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    let (ir, ig, ib) = INK;
    let (red, green, blue) = SIGNAL;
    // The r writes itself, stem first, at a pen's even pace.
    let drawn = stroke_length() * phase_smooth(t, WRITE);
    if drawn > 0.5 {
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.set_source_rgb(ir, ig, ib);
        trace(cr, 0.0, drawn);
        let _ = cr.stroke();
    }
    let travel = phase_smooth(t, SIGNAL_IN);
    if drawn > 0.5 && travel <= 0.0 {
        // While it writes, the signal is the pen's tip, with a short fading tail behind it.
        let tail = drawn.min(30.0);
        cr.set_line_cap(gtk::cairo::LineCap::Butt);
        for step in 0..8 {
            let from = drawn - tail * f64::from(8 - step) / 8.0;
            cr.set_source_rgba(red, green, blue, 0.12 + 0.11 * f64::from(step));
            trace(cr, from, from + tail / 8.0 + 0.6);
            let _ = cr.stroke();
        }
        let (x, y) = stroke_point(drawn);
        cr.set_source_rgb(red, green, blue);
        cr.arc(x, y, STROKE / 2.0, 0.0, 2.0 * PI);
        let _ = cr.fill();
    } else if travel > 0.0 {
        // Then it leaves the end of the stroke, carrying on round, and lands a little outside.
        let angle = (SHOULDER_END + (SIGNAL_ANGLE - SHOULDER_END) * travel).to_radians();
        let reach = RADIUS + (SIGNAL_RADIUS - RADIUS) * travel;
        let r = STROKE / 2.0 + (SIGNAL_DOT - STROKE / 2.0) * phase_back(t, SIGNAL_IN) + 1.6 * beat;
        cr.set_source_rgb(red, green, blue);
        cr.arc(CENTRE.0 + reach * angle.cos(), CENTRE.1 + reach * angle.sin(), r, 0.0, 2.0 * PI);
        let _ = cr.fill();
    }
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
        for span in [WRITE, WAVE, PING, WORDMARK, GREETING, QUESTION, CARD_DEV, CARD_THREADS, HINT] {
            assert_eq!(phase(0.0, span), 0.0);
            assert!((phase(1.0, span) - 1.0).abs() < 1e-9);
        }
        for span in [WRITE, SIGNAL_IN] {
            assert!(phase_smooth(0.0, span).abs() < 1e-9);
            assert!((phase_smooth(1.0, span) - 1.0).abs() < 1e-9);
        }
        assert!(phase_back(0.0, SIGNAL_IN).abs() < 1e-9);
        assert!((phase_back(1.0, SIGNAL_IN) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_stroke_is_one_line_from_the_foot_of_the_stem_to_the_end_of_the_shoulder() {
        let (x, y) = stroke_point(0.0);
        assert!((x - 29.0).abs() < 1e-9 && (y - 91.0).abs() < 1e-9);
        // The stem meets the shoulder where the shoulder starts: no step at the join.
        let (x, y) = stroke_point(STEM);
        assert!((x - (CENTRE.0 - RADIUS)).abs() < 1e-9 && (y - CENTRE.1).abs() < 1e-9);
        let end = SHOULDER_END.to_radians();
        let (x, y) = stroke_point(stroke_length());
        assert!((x - (CENTRE.0 + RADIUS * end.cos())).abs() < 1e-9);
        assert!((y - (CENTRE.1 + RADIUS * end.sin())).abs() < 1e-9);
    }

    #[test]
    fn the_signal_leaves_as_the_stroke_ends_and_rests_clear_of_it_inside_the_grid() {
        assert!((SIGNAL_IN.0 - WRITE.1).abs() < 1e-9);
        let (x, y) = signal_rest();
        let (ex, ey) = stroke_point(stroke_length());
        // A visible gap between the end of the stroke's cap and the dot.
        assert!((x - ex).hypot(y - ey) > STROKE / 2.0 + SIGNAL_DOT + 2.0);
        assert!(x + SIGNAL_DOT < 120.0 && y - SIGNAL_DOT > 0.0);
        // The whole mark sits about the grid's middle.
        let left = CENTRE.0 - RADIUS - STROKE / 2.0;
        let (top, bottom) = (CENTRE.1 - RADIUS - STROKE / 2.0, CENTRE.1 + STEM + STROKE / 2.0);
        assert!(((left + x + SIGNAL_DOT) / 2.0 - 60.0).abs() < 2.0);
        assert!(((top + bottom) / 2.0 - 60.0).abs() < 2.0);
    }

    #[test]
    fn the_words_follow_the_mark_and_the_cards_come_one_after_the_other() {
        assert!(WORDMARK.0 >= WRITE.0 && GREETING.0 > WORDMARK.0 && QUESTION.0 > GREETING.0);
        assert!(CARD_DEV.0 > QUESTION.0 - 0.1 && CARD_THREADS.0 > CARD_DEV.0);
        assert!(CARD_THREADS.1 <= 1.0 && HINT.1 <= 1.0 && PING.0 < SIGNAL_IN.1);
    }
}
