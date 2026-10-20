//! Motion (MODEL-SPEC §2.1, §2.6; feature-motion §1): how a node moves when its design plays.
//!
//! A node's `motion` holds keys on a clock: the nearest frame or page with a `timeline`, the node
//! itself included, else its page's own, as long as its latest key. Its stored fields are its
//! look at rest, which everything that does not play shows. Moves (`dx`, `dy`, `rotate`, the
//! scales) are offsets from where layout puts it, so motion never changes layout; looks
//! (`opacity`, `fill`, `stroke`, `color`, `blur`, `sweep`, `angle`, `trim`) show their value
//! instead of its own. Times carry their unit in the name: milliseconds.
//!
//! The types, their checks and the sampler live here: [`Motion::pose`] gives a node's pose at a
//! time of its clock, and [`Pose::dress`] the node as that pose shows it. Which clock a node plays
//! on needs the tree, and is the client's (`relay_client::design::motion`).

use super::{color_ref, obj_vec, parse_color, Arc, Gradient, GradientKind, Node, NodeKind, MAX_COORD};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Most keys one motion holds.
pub const MAX_KEYS: usize = 64;
/// Latest time a key or a timeline names, in ms: ten minutes.
pub const MAX_MS: f64 = 600_000.0;
/// Shortest motion or timeline, in ms: below it an agent most likely wrote seconds or fractions.
pub const MIN_MOTION_MS: f64 = 50.0;
/// Widest turn a key adds, and widest angle it gives a gradient, in degrees either way.
pub const MAX_TURN: f64 = 3_600.0;
/// Largest scale a key gives.
pub const MAX_SCALE: f64 = 100.0;
/// How far outside its box a motion's `origin` may sit, in box widths and heights.
pub const MAX_ORIGIN: f64 = 10.0;

/// How a move speeds up and slows down between two keys, as CSS names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignEase")]
pub enum Ease {
    Linear,
    /// CSS `ease`, cubic-bezier(0.25, 0.1, 0.25, 1): the default.
    #[serde(rename = "ease")]
    Standard,
    /// cubic-bezier(0.42, 0, 1, 1): departures.
    EaseIn,
    /// cubic-bezier(0, 0, 0.58, 1): arrivals.
    EaseOut,
    /// cubic-bezier(0.42, 0, 0.58, 1).
    EaseInOut,
    /// cubic-bezier(0.34, 1.56, 0.64, 1): goes a little past and settles; pops and bounces.
    Overshoot,
    /// Holds the key's values until the next key, then jumps: blinks, frame by frame.
    Hold,
}

/// One moment of a node's motion: how it looks `at_ms` into its timeline. A key names only what
/// it sets; between two keys that set a property it is interpolated, before the first that sets
/// it the property holds that key's value, after the last that key's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignKey")]
pub struct Key {
    /// Milliseconds from the start of the timeline, 0 to 600000 (half a second is 500).
    #[schemars(range(min = 0, max = MAX_MS))]
    pub at_ms: f64,
    /// How the move from this key to the next eases (default: the motion's `ease`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
    /// Moved right by this many px from where it sits (negative: left), within ±1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -MAX_COORD, max = MAX_COORD))]
    pub dx: Option<f64>,
    /// Moved down by this many px from where it sits (negative: up), within ±1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -MAX_COORD, max = MAX_COORD))]
    pub dy: Option<f64>,
    /// Degrees clockwise added to its own rotation, about `origin`, within ±3600.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -MAX_TURN, max = MAX_TURN))]
    pub rotate: Option<f64>,
    /// Size multiplier about `origin`, both ways, 0 to 100 (1 as drawn).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_SCALE))]
    pub scale: Option<f64>,
    /// Width multiplier about `origin`, times `scale`, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_SCALE))]
    pub scale_x: Option<f64>,
    /// Height multiplier about `origin`, times `scale`, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_SCALE))]
    pub scale_y: Option<f64>,
    /// 0 to 1, shown instead of its own opacity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1))]
    pub opacity: Option<f64>,
    /// Shown instead of its fill and gradient: `#RGB`, `#RRGGBB` or `#RRGGBBAA` (a key takes no
    /// `$name` colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Shown instead of its stroke colour: `#RGB`, `#RRGGBB` or `#RRGGBBAA`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// Shown instead of its text colour: `#RGB`, `#RRGGBB` or `#RRGGBBAA`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Layer blur in px, 0 to 200, instead of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 200))]
    pub blur: Option<f64>,
    /// An ellipse's arc sweep in degrees, -360 to 360, instead of its own (a whole ellipse sweeps
    /// from the top): a ring that fills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -360, max = 360))]
    pub sweep: Option<f64>,
    /// A linear or conic gradient's angle in degrees, within ±3600, instead of its own (and the
    /// stroke gradient's); nothing without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -MAX_TURN, max = MAX_TURN))]
    pub angle: Option<f64>,
    /// How much of a line's or path's stroke is drawn from its start, 0 to 1: drawing on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1))]
    pub trim: Option<f64>,
}

/// How a node moves when its design plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignMotion")]
pub struct Motion {
    /// 2 to 64 keys in time order, the last at 50 ms or later.
    #[serde(deserialize_with = "obj_vec")]
    #[schemars(length(min = 2, max = MAX_KEYS))]
    pub keys: Vec<Key>,
    /// Ease of every move whose key names none (default `ease`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
    /// The point it turns and scales about, as fractions of its box, each within ±10: `[0.5, 0.5]`
    /// (the default) the centre, `[0.5, 1]` the bottom middle, `[0, 0]` the top left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(inner(range(min = -MAX_ORIGIN, max = MAX_ORIGIN)))]
    pub origin: Option<[f64; 2]>,
    /// Plays its keys again from 0 every last-key ms, whatever its timeline does: spinners,
    /// pulses (default false).
    #[serde(rename = "loop", default, skip_serializing_if = "Option::is_none")]
    pub looped: Option<bool>,
}

/// A frame's or page's clock for the motion inside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignTimeline")]
pub struct Timeline {
    /// Its length in ms, 50 to 600000; default the latest key of the motion it clocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = MIN_MOTION_MS, max = MAX_MS))]
    pub duration_ms: Option<f64>,
    /// Starts again when it ends (default true); false holds the last frame.
    #[serde(rename = "loop", default, skip_serializing_if = "Option::is_none")]
    pub looped: Option<bool>,
}

/// A node posed at one time: its moves (identity at rest: no offset, no turn, scales 1) about
/// `origin`, and the looks a key shows instead of the node's own (`None` where none does). Made by
/// [`Motion::pose`]; never stored. `Pose::default()` is the rest pose.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub dx: f64,
    pub dy: f64,
    pub rotate: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Fractions of the box.
    pub origin: [f64; 2],
    pub opacity: Option<f64>,
    pub fill: Option<[u8; 4]>,
    pub stroke: Option<[u8; 4]>,
    pub color: Option<[u8; 4]>,
    pub blur: Option<f64>,
    pub sweep: Option<f64>,
    pub angle: Option<f64>,
    pub trim: Option<f64>,
}

/// What a key can set, as its refusal lists it.
const PROPERTIES: &str = "dx, dy, rotate, scale, scale_x, scale_y, opacity, fill, stroke, color, blur, sweep, angle, trim";

/// A motion's values, for a `kind` node: 2 to 64 keys in time order within 0 to 600000 ms, the
/// last at 50 ms or later, each setting something within its range, colours literal, `sweep` only
/// on an ellipse and `trim` only on a line or path, and an origin within ±10. Messages start with
/// the field (`motion.keys[1].at_ms: …`). Whether a kind moves at all (not a page) is a written
/// rule, in the fields table.
pub fn check(kind: NodeKind, m: &Motion) -> Result<(), String> {
    let n = m.keys.len();
    if !(2..=MAX_KEYS).contains(&n) {
        return Err(format!("motion.keys: give 2 to {MAX_KEYS} keys (this has {n})"));
    }
    for (i, key) in m.keys.iter().enumerate() {
        let at = key.at_ms;
        if !at.is_finite() || !(0.0..=MAX_MS).contains(&at) {
            return Err(format!("motion.keys[{i}].at_ms: {at} is outside 0 to {MAX_MS}"));
        }
        if let Some(before) = i.checked_sub(1).map(|b| m.keys[b].at_ms).filter(|before| at < *before) {
            return Err(format!("motion.keys[{i}].at_ms: {at} comes before keys[{}] ({before}); keys go in time order", i - 1));
        }
        check_key(kind, i, key)?;
    }
    let last = m.keys[n - 1].at_ms;
    if last < MIN_MOTION_MS {
        return Err(format!("motion.keys: the last key is at {last} ms; times are milliseconds (half a second is 500)"));
    }
    if m.origin.is_some_and(|o| o.iter().any(|v| !v.is_finite() || v.abs() > MAX_ORIGIN)) {
        return Err(format!("motion.origin: [x, y] fractions of the box, each within ±{MAX_ORIGIN}"));
    }
    Ok(())
}

/// Key `i` of a `kind` node's motion.
fn check_key(kind: NodeKind, i: usize, key: &Key) -> Result<(), String> {
    let ranged = [
        ("dx", key.dx, -MAX_COORD, MAX_COORD),
        ("dy", key.dy, -MAX_COORD, MAX_COORD),
        ("rotate", key.rotate, -MAX_TURN, MAX_TURN),
        ("scale", key.scale, 0.0, MAX_SCALE),
        ("scale_x", key.scale_x, 0.0, MAX_SCALE),
        ("scale_y", key.scale_y, 0.0, MAX_SCALE),
        ("opacity", key.opacity, 0.0, 1.0),
        ("blur", key.blur, 0.0, 200.0),
        ("sweep", key.sweep, -360.0, 360.0),
        ("angle", key.angle, -MAX_TURN, MAX_TURN),
        ("trim", key.trim, 0.0, 1.0),
    ];
    let colours = [("fill", &key.fill), ("stroke", &key.stroke), ("color", &key.color)];
    if ranged.iter().all(|(_, v, _, _)| v.is_none()) && colours.iter().all(|(_, c)| c.is_none()) {
        return Err(format!("motion.keys[{i}]: name something to animate ({PROPERTIES})"));
    }
    for (field, value, lo, hi) in ranged {
        if let Some(v) = value.filter(|v| !v.is_finite() || !(lo..=hi).contains(v)) {
            return Err(format!("motion.keys[{i}].{field}: {v} is outside {lo} to {hi}"));
        }
    }
    for (field, colour) in colours {
        match colour.as_deref() {
            Some(c) if color_ref(c).is_some() => {
                return Err(format!("motion.keys[{i}].{field}: {c:?}: a key takes a colour itself (#RGB, #RRGGBB or #RRGGBBAA), not a $name"))
            }
            Some(c) if parse_color(c).is_none() => return Err(format!("motion.keys[{i}].{field}: {c:?} is not #RGB, #RRGGBB or #RRGGBBAA")),
            _ => {}
        }
    }
    if key.sweep.is_some() && kind != NodeKind::Ellipse {
        return Err(format!("motion.keys[{i}].sweep: only an ellipse sweeps"));
    }
    if key.trim.is_some() && !matches!(kind, NodeKind::Line | NodeKind::Path) {
        return Err(format!("motion.keys[{i}].trim: only a line or path is drawn on"));
    }
    Ok(())
}

impl Timeline {
    /// Its values: a length within 50 to 600000 ms.
    pub fn check(&self) -> Result<(), String> {
        match self.duration_ms {
            Some(d) if !d.is_finite() || !(MIN_MOTION_MS..=MAX_MS).contains(&d) => {
                Err(format!("timeline.duration_ms: {d} is outside {MIN_MOTION_MS} to {MAX_MS}"))
            }
            _ => Ok(()),
        }
    }
}

// The sampler (feature-motion §1.2, client-motion). Pure, so the engine, the canvas and a later
// `design.sample` agree on what a motion shows at a time.

/// How close the eases' solver gets to the time asked for.
const EASE_EPSILON: f64 = 1e-7;

impl Ease {
    /// Its cubic-bezier's control points `[x1, y1, x2, y2]`, as CSS gives them; `None` for `hold`,
    /// which steps.
    fn bezier(self) -> Option<[f64; 4]> {
        match self {
            Ease::Linear => Some([0.0, 0.0, 1.0, 1.0]),
            Ease::Standard => Some([0.25, 0.1, 0.25, 1.0]),
            Ease::EaseIn => Some([0.42, 0.0, 1.0, 1.0]),
            Ease::EaseOut => Some([0.0, 0.0, 0.58, 1.0]),
            Ease::EaseInOut => Some([0.42, 0.0, 0.58, 1.0]),
            Ease::Overshoot => Some([0.34, 1.56, 0.64, 1.0]),
            Ease::Hold => None,
        }
    }

    /// How far a move has gone (0 to 1, and past 1 for a while under `overshoot`) once `u` of its
    /// time has passed (0 to 1, clamped): the cubic-bezier solved along time by Newton's method,
    /// with bisection where that stalls, to 1e-7. Every ease gives 0 at 0 and 1 at 1; `hold` stays
    /// at 0 until the end.
    pub fn apply(self, u: f64) -> f64 {
        let u = if u.is_nan() { 0.0 } else { u.clamp(0.0, 1.0) };
        match self.bezier() {
            None => if u >= 1.0 { 1.0 } else { 0.0 },
            Some(_) if self == Ease::Linear || u == 0.0 || u == 1.0 => u,
            Some([x1, y1, x2, y2]) => bezier(y1, y2, solve(x1, x2, u)),
        }
    }
}

/// One coordinate, at `s` (0 to 1), of a cubic-bezier from 0 to 1 whose control points have the
/// coordinates `p1` and `p2`.
fn bezier(p1: f64, p2: f64, s: f64) -> f64 {
    let c = 3.0 * p1;
    let b = 3.0 * (p2 - p1) - c;
    let a = 1.0 - c - b;
    ((a * s + b) * s + c) * s
}

/// The slope of [`bezier`] at `s`.
fn bezier_slope(p1: f64, p2: f64, s: f64) -> f64 {
    let c = 3.0 * p1;
    let b = 3.0 * (p2 - p1) - c;
    let a = 1.0 - c - b;
    (3.0 * a * s + 2.0 * b) * s + c
}

/// The `s` at which a curve whose x control coordinates are `x1` and `x2` reaches `x` (0 to 1).
/// The curve's x only grows on 0..1 (CSS holds x1 and x2 to 0..1), so bisection always ends there.
fn solve(x1: f64, x2: f64, x: f64) -> f64 {
    let mut s = x;
    for _ in 0..8 {
        let off = bezier(x1, x2, s) - x;
        if off.abs() < EASE_EPSILON && (0.0..=1.0).contains(&s) {
            return s;
        }
        let slope = bezier_slope(x1, x2, s);
        if slope.abs() < 1e-6 {
            break;
        }
        s -= off / slope;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    s = x;
    // 64 halvings take 1 well below the epsilon.
    for _ in 0..64 {
        let at = bezier(x1, x2, s);
        if (at - x).abs() < EASE_EPSILON {
            break;
        }
        if at < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    s
}

/// A time to read keys at: a negative or non-finite one is the start.
fn time(t_ms: f64) -> f64 {
    if t_ms.is_finite() { t_ms.max(0.0) } else { 0.0 }
}

/// Two colours mixed `e` of the way from `a` to `b` in sRGB with premultiplied alpha, as CSS mixes
/// them, so a fade from clear red to red stays red; `e` past 0..1 (an overshoot) is clamped
/// channel by channel.
fn mix(a: [u8; 4], b: [u8; 4], e: f64) -> [u8; 4] {
    let (alpha_a, alpha_b) = (f64::from(a[3]) / 255.0, f64::from(b[3]) / 255.0);
    let alpha = (alpha_a + (alpha_b - alpha_a) * e).clamp(0.0, 1.0);
    let mut out = [0u8; 4];
    for i in 0..3 {
        let (ca, cb) = (f64::from(a[i]), f64::from(b[i]));
        let c = if alpha > 0.0 {
            (ca * alpha_a + (cb * alpha_b - ca * alpha_a) * e) / alpha
        } else {
            // Nothing shows; keep the straight mix so the colour is still a sensible one.
            ca + (cb - ca) * e
        };
        out[i] = c.clamp(0.0, 255.0).round() as u8;
    }
    out[3] = (alpha * 255.0).round() as u8;
    out
}

/// `#RRGGBBAA`.
fn hex(c: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
}

impl Key {
    /// A key at `at_ms` that sets nothing yet (a key must set something to pass [`check`]).
    pub fn at(at_ms: f64) -> Key {
        Key {
            at_ms,
            ease: None,
            dx: None,
            dy: None,
            rotate: None,
            scale: None,
            scale_x: None,
            scale_y: None,
            opacity: None,
            fill: None,
            stroke: None,
            color: None,
            blur: None,
            sweep: None,
            angle: None,
            trim: None,
        }
    }
}

impl Motion {
    /// The time of its last key: how long it takes once.
    pub fn last_ms(&self) -> f64 {
        self.keys.last().map_or(0.0, |k| k.at_ms)
    }

    /// The time its keys are read at, `t_ms` into its timeline: past its last key a looping
    /// motion starts again (exactly on the last key it shows the end).
    pub fn local_ms(&self, t_ms: f64) -> f64 {
        let t = time(t_ms);
        let last = self.last_ms();
        if self.looped == Some(true) && last > 0.0 && t > last { t % last } else { t }
    }

    /// The node's pose `t_ms` into its timeline. Each property is read from the keys that set it:
    /// at or before the first that does, that key's value; at or after the last, that one's;
    /// between two, eased by the earlier key's `ease` (else the motion's, else `ease`) as CSS
    /// shapes the interval that starts at a keyframe. Opacity and trim are clamped to 0..1, blur
    /// and scales to 0 or more, sweep to ±360, colours to their bytes.
    pub fn pose(&self, t_ms: f64) -> Pose {
        let t = self.local_ms(t_ms);
        let (before, after) = self.keys.split_at(self.keys.partition_point(|k| k.at_ms <= t));
        let ease = |k: &Key| k.ease.or(self.ease).unwrap_or(Ease::Standard);
        // The value of the property `get` reads, mixed by `mix` between the keys around `t`.
        fn sample<T: Copy>(before: &[Key], after: &[Key], t: f64, ease: &dyn Fn(&Key) -> Ease,
            get: impl Fn(&Key) -> Option<T>, mix: impl Fn(T, T, f64) -> T) -> Option<T> {
            let from = before.iter().rev().find_map(|k| get(k).map(|v| (k, v)));
            let to = after.iter().find_map(|k| get(k).map(|v| (k, v)));
            match (from, to) {
                (None, None) => None,
                (Some((_, v)), None) | (None, Some((_, v))) => Some(v),
                // `a` is at or before `t` and `b` after it, so they are apart.
                (Some((a, va)), Some((b, vb))) => Some(mix(va, vb, ease(a).apply((t - a.at_ms) / (b.at_ms - a.at_ms)))),
            }
        }
        let number = |get: fn(&Key) -> Option<f64>| sample(before, after, t, &ease, get, |a, b, e| a + (b - a) * e);
        let colour = |get: fn(&Key) -> Option<&String>| {
            sample(before, after, t, &ease, |k| get(k).and_then(|c| parse_color(c)), mix)
        };
        let scale = number(|k| k.scale).map_or(1.0, |s| s.max(0.0));
        Pose {
            dx: number(|k| k.dx).unwrap_or(0.0),
            dy: number(|k| k.dy).unwrap_or(0.0),
            rotate: number(|k| k.rotate).unwrap_or(0.0),
            scale_x: scale * number(|k| k.scale_x).map_or(1.0, |s| s.max(0.0)),
            scale_y: scale * number(|k| k.scale_y).map_or(1.0, |s| s.max(0.0)),
            origin: self.origin.unwrap_or([0.5, 0.5]),
            opacity: number(|k| k.opacity).map(|v| v.clamp(0.0, 1.0)),
            fill: colour(|k| k.fill.as_ref()),
            stroke: colour(|k| k.stroke.as_ref()),
            color: colour(|k| k.color.as_ref()),
            blur: number(|k| k.blur).map(|v| v.max(0.0)),
            sweep: number(|k| k.sweep).map(|v| v.clamp(-360.0, 360.0)),
            angle: number(|k| k.angle),
            trim: number(|k| k.trim).map(|v| v.clamp(0.0, 1.0)),
        }
    }
}

impl Default for Pose {
    /// At rest: where layout puts it, as it is drawn.
    fn default() -> Pose {
        Pose {
            dx: 0.0,
            dy: 0.0,
            rotate: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            origin: [0.5, 0.5],
            opacity: None,
            fill: None,
            stroke: None,
            color: None,
            blur: None,
            sweep: None,
            angle: None,
            trim: None,
        }
    }
}

/// Whether a key's `angle` turns `g`: a linear or conic gradient does, a radial one has none.
fn turns(g: &Option<Gradient>) -> bool {
    g.as_ref().is_some_and(|g| g.kind != GradientKind::Radial)
}

impl Pose {
    /// Whether it moves the node at all: an offset, a turn or a scale away from 1.
    pub fn moves(&self) -> bool {
        self.dx != 0.0 || self.dy != 0.0 || self.rotate != 0.0 || self.scale_x != 1.0 || self.scale_y != 1.0
    }

    /// `n` as this pose shows it, or `None` when the pose changes none of its looks. Opacity,
    /// colours (as `#RRGGBBAA`) and blur replace the node's own; a fill clears its gradient and a
    /// stroke its stroke gradient; a sweep sets an ellipse's arc (a whole ellipse gets one from
    /// the top); an angle turns a linear or conic gradient and stroke gradient. Trim is not a node
    /// field: the painter reads it from the pose.
    pub fn dress(&self, n: &Node) -> Option<Node> {
        let sweeps = self.sweep.is_some() && n.kind == NodeKind::Ellipse;
        let turned = self.angle.is_some() && (turns(&n.gradient) || turns(&n.stroke_gradient));
        let looks = self.opacity.is_some() || self.fill.is_some() || self.stroke.is_some() || self.color.is_some() || self.blur.is_some();
        if !looks && !sweeps && !turned {
            return None;
        }
        let mut d = n.clone();
        if let Some(o) = self.opacity {
            d.opacity = Some(o);
        }
        if let Some(c) = self.fill {
            d.fill = Some(hex(c));
            d.gradient = None;
        }
        if let Some(c) = self.stroke {
            d.stroke = Some(hex(c));
            d.stroke_gradient = None;
        }
        if let Some(c) = self.color {
            d.color = Some(hex(c));
        }
        if let Some(b) = self.blur {
            d.blur = Some(b);
        }
        if let Some(sweep) = self.sweep.filter(|_| sweeps) {
            match &mut d.arc {
                Some(arc) => arc.sweep = sweep,
                None => d.arc = Some(Arc { start: 0.0, sweep, inner: 0.0, cap: None }),
            }
        }
        if let Some(angle) = self.angle {
            for g in [&mut d.gradient, &mut d.stroke_gradient].into_iter().flatten() {
                if g.kind != GradientKind::Radial {
                    g.angle = Some(angle);
                }
            }
        }
        Some(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Node;
    use serde_json::{json, Value};

    fn node(kind: &str, extra: Value) -> Result<Node, String> {
        let mut body = json!({"kind": kind});
        if kind != "page" {
            body["parent"] = json!("p");
        }
        match kind {
            "line" => body["points"] = json!([[0, 0], [9, 0]]),
            "path" => body["points"] = json!([[0, 0]]),
            _ => {}
        }
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        let node = Node::read(body)?;
        node.validate()?;
        node.check_new()?;
        Ok(node)
    }

    /// The skill's own example reads, validates and writes back as it was written, with `loop`.
    #[test]
    fn motion_reads_as_agents_write_it() {
        let title = json!({"ease": "ease_out", "keys": [{"at_ms": 0, "dy": 24, "opacity": 0}, {"at_ms": 500, "dy": 0, "opacity": 1}]});
        let loader = json!({"ease": "linear", "loop": true, "origin": [0.5, 1], "keys": [{"at_ms": 0, "rotate": 0}, {"at_ms": 1000, "rotate": 360}]});
        let ring = json!({"keys": [{"at_ms": 0, "sweep": 0, "ease": "overshoot"}, {"at_ms": 1200, "sweep": 270}]});
        for (kind, motion) in [("text", title), ("icon", loader), ("ellipse", ring)] {
            let extra = if kind == "icon" { json!({"icon": "refresh", "motion": motion}) } else { json!({"motion": motion}) };
            let n = node(kind, extra).unwrap_or_else(|why| panic!("{kind}: {why}"));
            let written = serde_json::to_value(n.motion.as_ref().unwrap()).unwrap();
            let read: Motion = serde_json::from_value(written.clone()).unwrap();
            assert_eq!(&read, n.motion.as_ref().unwrap());
            assert!(written.to_string().contains("\"at_ms\""));
            assert!(!written.to_string().contains("looped"), "{written}");
        }
        let looping = node("icon", json!({"icon": "refresh", "motion": {"loop": true, "keys": [{"at_ms": 0, "rotate": 0}, {"at_ms": 900, "rotate": 360}]}})).unwrap();
        assert_eq!(serde_json::to_value(looping.motion).unwrap()["loop"], true);
        // Absent options write nothing.
        let plain: Motion = serde_json::from_value(json!({"keys": [{"at_ms": 0, "opacity": 0}, {"at_ms": 400, "opacity": 1}]})).unwrap();
        assert_eq!(serde_json::to_value(&plain).unwrap(), json!({"keys": [{"at_ms": 0.0, "opacity": 0.0}, {"at_ms": 400.0, "opacity": 1.0}]}));
        let frame = node("frame", json!({"timeline": {"duration_ms": 3000, "loop": false}})).unwrap();
        assert_eq!(serde_json::to_value(frame.timeline.unwrap()).unwrap(), json!({"duration_ms": 3000.0, "loop": false}));
        assert!(node("page", json!({"timeline": {}})).is_ok(), "a page keeps time too");
        // Every ease reads by its CSS name.
        for (name, ease) in [("linear", Ease::Linear), ("ease", Ease::Standard), ("ease_in", Ease::EaseIn), ("ease_out", Ease::EaseOut),
            ("ease_in_out", Ease::EaseInOut), ("overshoot", Ease::Overshoot), ("hold", Ease::Hold)] {
            assert_eq!(serde_json::from_value::<Ease>(json!(name)).unwrap(), ease);
            assert_eq!(serde_json::to_value(ease).unwrap(), name);
        }
    }

    /// Every rule of feature-motion §1.1 names its field, and the kind rules are the fields table's.
    #[test]
    fn motion_refusals_name_the_field() {
        let keys = |keys: Value| json!({"motion": {"keys": keys}});
        let cases = [
            ("rect", keys(json!([{"at_ms": 0, "opacity": 0}])), "motion.keys: give 2 to 64 keys (this has 1)"),
            ("rect", keys(json!([{"at_ms": 400, "opacity": 0}, {"at_ms": 100, "opacity": 1}])),
                "motion.keys[1].at_ms: 100 comes before keys[0] (400); keys go in time order"),
            ("rect", keys(json!([{"at_ms": 0, "opacity": 0}, {"at_ms": 0.5, "opacity": 1}])),
                "motion.keys: the last key is at 0.5 ms; times are milliseconds (half a second is 500)"),
            ("rect", keys(json!([{"at_ms": -1, "opacity": 0}, {"at_ms": 500, "opacity": 1}])), "motion.keys[0].at_ms: -1 is outside 0 to 600000"),
            ("rect", keys(json!([{"at_ms": 0, "opacity": 0}, {"at_ms": 700000, "opacity": 1}])), "motion.keys[1].at_ms: 700000 is outside 0 to 600000"),
            ("rect", keys(json!([{"at_ms": 0}, {"at_ms": 500, "opacity": 1}])), &format!("motion.keys[0]: name something to animate ({PROPERTIES})")),
            ("rect", keys(json!([{"at_ms": 0, "opacity": 1.5}, {"at_ms": 500, "opacity": 1}])), "motion.keys[0].opacity: 1.5 is outside 0 to 1"),
            ("rect", keys(json!([{"at_ms": 0, "fill": "red"}, {"at_ms": 500, "fill": "#fff"}])), "motion.keys[0].fill: \"red\" is not #RGB, #RRGGBB or #RRGGBBAA"),
            ("rect", keys(json!([{"at_ms": 0, "fill": "$accent"}, {"at_ms": 500, "fill": "#fff"}])),
                "motion.keys[0].fill: \"$accent\": a key takes a colour itself (#RGB, #RRGGBB or #RRGGBBAA), not a $name"),
            ("rect", keys(json!([{"at_ms": 0, "dx": 2e6}, {"at_ms": 500, "dx": 0}])), "motion.keys[0].dx: 2000000 is outside -1000000 to 1000000"),
            ("rect", keys(json!([{"at_ms": 0, "rotate": 4000}, {"at_ms": 500, "rotate": 0}])), "motion.keys[0].rotate: 4000 is outside -3600 to 3600"),
            ("rect", keys(json!([{"at_ms": 0, "scale": -1}, {"at_ms": 500, "scale": 1}])), "motion.keys[0].scale: -1 is outside 0 to 100"),
            ("rect", keys(json!([{"at_ms": 0, "blur": 300}, {"at_ms": 500, "blur": 0}])), "motion.keys[0].blur: 300 is outside 0 to 200"),
            ("ellipse", keys(json!([{"at_ms": 0, "sweep": 400}, {"at_ms": 500, "sweep": 0}])), "motion.keys[0].sweep: 400 is outside -360 to 360"),
            ("rect", keys(json!([{"at_ms": 0, "sweep": 0}, {"at_ms": 500, "sweep": 90}])), "motion.keys[0].sweep: only an ellipse sweeps"),
            ("rect", keys(json!([{"at_ms": 0, "trim": 0}, {"at_ms": 500, "trim": 1}])), "motion.keys[0].trim: only a line or path is drawn on"),
            ("rect", json!({"motion": {"origin": [0.5, 20], "keys": [{"at_ms": 0, "scale": 0}, {"at_ms": 500, "scale": 1}]}}),
                "motion.origin: [x, y] fractions of the box, each within ±10"),
            ("page", keys(json!([{"at_ms": 0, "opacity": 0}, {"at_ms": 500, "opacity": 1}])), "motion: a page does not move; give motion to what is on it"),
            ("rect", json!({"timeline": {"duration_ms": 1000}}), "timeline: only a frame or a page keeps time"),
            ("frame", json!({"timeline": {"duration_ms": 10}}), "timeline.duration_ms: 10 is outside 50 to 600000"),
        ];
        for (kind, extra, want) in cases {
            assert_eq!(node(kind, extra.clone()).err().as_deref(), Some(want), "{kind} {extra}");
        }
        // Equal times are allowed (a jump), and so are a late first key (a delay) and a line drawn on.
        assert!(node("rect", keys(json!([{"at_ms": 300, "opacity": 0}, {"at_ms": 300, "opacity": 1}, {"at_ms": 600, "dx": 4}]))).is_ok());
        assert!(node("line", keys(json!([{"at_ms": 0, "trim": 0}, {"at_ms": 800, "trim": 1}]))).is_ok());
        assert!(node("path", keys(json!([{"at_ms": 0, "trim": 0, "stroke": "#4AF0FF"}, {"at_ms": 800, "trim": 1}]))).is_ok());
        // An unknown key field is refused, and serde lists the ones there are.
        let x = Node::read(json!({"kind": "rect", "parent": "p", "motion": {"keys": [{"at_ms": 0, "x": 4}, {"at_ms": 500, "x": 0}]}})).unwrap();
        let why = x.check_new().unwrap_err();
        assert!(why.starts_with("motion: unknown field `x`, expected one of `at_ms`, `ease`, `dx`"), "{why}");
    }

    /// A motion edit is an ordinary update: its undo puts back the motion as it was, whole.
    #[test]
    fn a_motion_patch_undoes_exactly() {
        let n = node("rect", json!({"motion": {"keys": [{"at_ms": 0, "opacity": 0}, {"at_ms": 400, "opacity": 1}]}})).unwrap();
        let set = json!({"motion": {"ease": "overshoot", "keys": [{"at_ms": 0, "scale": 0.6}, {"at_ms": 350, "scale": 1}]}, "timeline": null});
        let set = set.as_object().unwrap().clone();
        let next = n.patched(&set).unwrap();
        assert_eq!(next.motion.as_ref().unwrap().ease, Some(Ease::Overshoot));
        let back = next.patched(&n.before(&set)).unwrap();
        assert_eq!(back, n);
        let cleared = n.patched(json!({"motion": null}).as_object().unwrap()).unwrap();
        assert_eq!(cleared.motion, None);
        // A motion that does not read is set aside (kept as it came) and refused by name.
        let bad = n.patched(json!({"motion": {"keys": "soon"}}).as_object().unwrap()).unwrap();
        assert!(bad.extra.contains_key("motion"));
    }

    /// The motion and timeline schemas state the rules' ranges: each bound accepted, a step past
    /// it refused.
    #[test]
    fn the_schema_states_the_motion_rules_ranges() {
        let defs = serde_json::to_value(schemars::schema_for!(Node)).unwrap()["$defs"].clone();
        let mut seen = 0;
        for (key, property) in defs["DesignKey"]["properties"].as_object().unwrap() {
            let (Some(lo), Some(hi)) = (property["minimum"].as_f64(), property["maximum"].as_f64()) else { continue };
            let kind = match key.as_str() {
                "sweep" => "ellipse",
                "trim" => "line",
                _ => "rect",
            };
            for (v, ok) in [(lo, true), (hi, true), (lo - 0.001, false), (hi + 0.001, false)] {
                // The value goes in the first key, which a later key at the latest time follows.
                let first = if key == "at_ms" { json!({"at_ms": v, "opacity": 0}) } else { json!({"at_ms": 0, key: v}) };
                let motion = json!({"keys": [first, {"at_ms": MAX_MS, "opacity": 1}]});
                assert_eq!(node(kind, json!({"motion": motion})).is_ok(), ok, "{key} = {v}");
            }
            seen += 1;
        }
        assert_eq!(seen, 12, "every number a key holds states its range");
        let keys = &defs["DesignMotion"]["properties"]["keys"];
        assert_eq!((keys["minItems"].as_u64(), keys["maxItems"].as_u64()), (Some(2), Some(MAX_KEYS as u64)));
        let fill = |n: usize| json!({"motion": {"keys": (0..n).map(|i| json!({"at_ms": 100 * i, "opacity": 1})).collect::<Vec<_>>()}});
        assert!(node("rect", fill(MAX_KEYS)).is_ok() && node("rect", fill(MAX_KEYS + 1)).is_err() && node("rect", fill(1)).is_err());
        let origin = &defs["DesignMotion"]["properties"]["origin"]["items"];
        assert_eq!((origin["minimum"].as_f64(), origin["maximum"].as_f64()), (Some(-MAX_ORIGIN), Some(MAX_ORIGIN)));
        let duration = &defs["DesignTimeline"]["properties"]["duration_ms"];
        assert_eq!((duration["minimum"].as_f64(), duration["maximum"].as_f64()), (Some(MIN_MOTION_MS), Some(MAX_MS)));
    }

    /// A motion as agents write it, without the node around it.
    fn motion(value: Value) -> Motion {
        serde_json::from_value(value).unwrap()
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// feature-motion §1.2: a property holds its first key's value before it and its last key's
    /// after, moves between the keys that set it, steps under `hold`, jumps at equal times and
    /// waits for a late first key.
    #[test]
    fn sampling_holds_eases_and_interpolates() {
        let slide = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "dx": 0}, {"at_ms": 1000, "dx": 100}]}));
        let dx = |m: &Motion, t: f64| m.pose(t).dx;
        assert_eq!((dx(&slide, -5.0), dx(&slide, 0.0), dx(&slide, 500.0), dx(&slide, 1000.0), dx(&slide, 2000.0)), (0.0, 0.0, 50.0, 100.0, 100.0));
        assert_eq!((dx(&slide, f64::NAN), dx(&slide, f64::INFINITY)), (0.0, 0.0), "a time that is no time is the start");
        // What no key sets stays the node's own, and the rest pose does not move.
        let rest = slide.pose(0.0);
        assert_eq!((rest.opacity, rest.fill, rest.trim, rest.scale_x, rest.origin), (None, None, None, 1.0, [0.5, 0.5]));
        assert!(!rest.moves() && slide.pose(1.0).moves() && !Pose::default().moves());
        // The default ease is CSS `ease`: at half the time, 0.8024 of the way.
        let eased = motion(json!({"keys": [{"at_ms": 0, "dx": 0}, {"at_ms": 1000, "dx": 100}]}));
        assert!((dx(&eased, 500.0) - 80.2403).abs() < 1e-4, "{}", dx(&eased, 500.0));
        assert!(near(Ease::EaseInOut.apply(0.5), 0.5));
        // A key's own ease shapes the move that starts at it: `hold` keeps its value until the next key.
        let blink = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "opacity": 0, "ease": "hold"}, {"at_ms": 500, "opacity": 1}, {"at_ms": 1000, "opacity": 0}]}));
        let opacity = |t: f64| blink.pose(t).opacity.unwrap();
        assert_eq!((opacity(0.0), opacity(499.0), opacity(500.0), opacity(750.0)), (0.0, 0.0, 1.0, 0.5));
        // A property set only in keys 0 and 2 moves across key 1, which sets something else.
        let across = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "dx": 0}, {"at_ms": 500, "opacity": 1}, {"at_ms": 1000, "dx": 100}]}));
        assert_eq!((across.pose(500.0).dx, across.pose(250.0).opacity), (50.0, Some(1.0)));
        // Two keys at one time jump.
        let jump = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "dy": 0}, {"at_ms": 300, "dy": 0}, {"at_ms": 300, "dy": 50}, {"at_ms": 600, "dy": 50}]}));
        assert_eq!((jump.pose(299.9).dy, jump.pose(300.0).dy, jump.pose(450.0).dy), (0.0, 50.0, 50.0));
        // A late first key holds its value until it comes: a delay.
        let late = motion(json!({"ease": "linear", "keys": [{"at_ms": 300, "opacity": 0}, {"at_ms": 700, "opacity": 1}]}));
        assert_eq!((late.pose(0.0).opacity, late.pose(300.0).opacity, late.pose(500.0).opacity), (Some(0.0), Some(0.0), Some(0.5)));
        // Scales multiply, and every move reads at once.
        let all = motion(json!({"ease": "linear", "origin": [0.5, 1], "keys": [
            {"at_ms": 0, "scale": 1, "scale_x": 1, "scale_y": 2, "rotate": 0, "blur": 0, "trim": 0},
            {"at_ms": 100, "scale": 3, "scale_x": 2, "scale_y": 2, "rotate": 90, "blur": 8, "trim": 1}]}));
        let mid = all.pose(50.0);
        assert_eq!((mid.scale_x, mid.scale_y, mid.rotate, mid.blur, mid.trim, mid.origin), (3.0, 4.0, 45.0, Some(4.0), Some(0.5), [0.5, 1.0]));
    }

    /// Colours mix in sRGB with premultiplied alpha, so a fade from clear red stays red.
    #[test]
    fn colours_interpolate_premultiplied() {
        let fade = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "fill": "#FF000000"}, {"at_ms": 100, "fill": "#FF0000FF"}]}));
        assert_eq!(fade.pose(50.0).fill, Some([255, 0, 0, 128]));
        assert_eq!((fade.pose(0.0).fill, fade.pose(100.0).fill), (Some([255, 0, 0, 0]), Some([255, 0, 0, 255])));
        let grey = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "stroke": "#000"}, {"at_ms": 100, "stroke": "#FFFFFF"}]}));
        assert_eq!(grey.pose(50.0).stroke, Some([128, 128, 128, 255]));
        // Between two clear colours nothing shows, and the colour is still the straight mix.
        let clear = motion(json!({"ease": "linear", "keys": [{"at_ms": 0, "color": "#00000000"}, {"at_ms": 100, "color": "#FFFFFF00"}]}));
        assert_eq!(clear.pose(50.0).color, Some([128, 128, 128, 0]));
        // An overshoot past the last colour stays within the bytes.
        let pop = motion(json!({"ease": "overshoot", "keys": [{"at_ms": 0, "fill": "#00000000"}, {"at_ms": 100, "fill": "#FF8000FF"}]}));
        for t in 1..=100 {
            let [r, _, b, _] = pop.pose(f64::from(t)).fill.unwrap();
            assert_eq!((r, b), (255, 0), "{t}: red stays red, blue stays out");
        }
        assert_eq!(pop.pose(60.0).fill.unwrap()[3], 255, "alpha clamps at 1 while it overshoots");
    }

    /// Every ease meets 0 at the start and 1 at the end, the CSS curves give CSS's values, all but
    /// `overshoot` only grow, and what overshoots is clamped where it has a range.
    #[test]
    fn easings_meet_their_ends() {
        let all = [Ease::Linear, Ease::Standard, Ease::EaseIn, Ease::EaseOut, Ease::EaseInOut, Ease::Overshoot, Ease::Hold];
        for ease in all {
            assert_eq!((ease.apply(0.0), ease.apply(1.0)), (0.0, 1.0), "{ease:?}");
            assert_eq!((ease.apply(-1.0), ease.apply(2.0), ease.apply(f64::NAN)), (0.0, 1.0, 0.0), "{ease:?} clamps its time");
            let curve: Vec<f64> = (0..=1000).map(|i| ease.apply(f64::from(i) / 1000.0)).collect();
            if ease == Ease::Overshoot {
                let peak = curve.iter().cloned().fold(0.0, f64::max);
                assert!(peak > 1.09 && peak < 1.1, "overshoot peaks a little past 1: {peak}");
            } else {
                assert!(curve.windows(2).all(|w| w[1] >= w[0] - 1e-12), "{ease:?} only grows");
            }
        }
        // The values CSS's cubic-bezier gives, solved independently.
        let css = [
            (Ease::Standard, [0.408511, 0.802403, 0.960459]),
            (Ease::EaseIn, [0.093465, 0.315357, 0.621862]),
            (Ease::EaseOut, [0.378138, 0.684643, 0.906535]),
            (Ease::EaseInOut, [0.129162, 0.5, 0.870838]),
            (Ease::Overshoot, [0.816289, 1.087401, 1.059647]),
        ];
        for (ease, want) in css {
            for (u, w) in [0.25, 0.5, 0.75].into_iter().zip(want) {
                assert!((ease.apply(u) - w).abs() < 1e-5, "{ease:?}({u}) = {} not {w}", ease.apply(u));
            }
        }
        assert_eq!((Ease::Hold.apply(0.999), Ease::Linear.apply(0.3)), (0.0, 0.3));
        // Under an overshoot, opacity and trim stay within 0 to 1 while a move goes past its end.
        let pop = motion(json!({"ease": "overshoot", "keys": [{"at_ms": 0, "opacity": 0, "dx": 0, "scale": 0}, {"at_ms": 1000, "opacity": 1, "dx": 100, "scale": 1}]}));
        let poses: Vec<Pose> = (0..=1000).map(|t| pop.pose(f64::from(t))).collect();
        assert!(poses.iter().all(|p| p.opacity.unwrap() <= 1.0));
        assert!(poses.iter().any(|p| p.dx > 105.0) && poses.iter().any(|p| p.scale_x > 1.05), "moves are not clamped");
        let shrink = motion(json!({"ease": "overshoot", "keys": [{"at_ms": 0, "scale": 1, "blur": 4, "sweep": -300}, {"at_ms": 1000, "scale": 0, "blur": 0, "sweep": -360}]}));
        let low = (0..=1000).map(|t| shrink.pose(f64::from(t))).fold((f64::MAX, f64::MAX, 0.0f64), |acc, p| (acc.0.min(p.scale_x), acc.1.min(p.blur.unwrap()), acc.2.min(p.sweep.unwrap())));
        assert_eq!(low, (0.0, 0.0, -360.0), "scale and blur stay at 0 or more, sweep within -360");
    }

    /// A looping motion starts again past its last key, and landing on that key shows the end.
    #[test]
    fn a_looping_motion_repeats_and_ends_on_its_last_key() {
        let spin = motion(json!({"ease": "linear", "loop": true, "keys": [{"at_ms": 0, "rotate": 0}, {"at_ms": 1000, "rotate": 360}]}));
        assert_eq!(spin.last_ms(), 1000.0);
        assert_eq!((spin.local_ms(1000.0), spin.local_ms(1010.0), spin.local_ms(2500.0), spin.local_ms(-4.0)), (1000.0, 10.0, 500.0, 0.0));
        assert_eq!((spin.pose(1000.0).rotate, spin.pose(1010.0).rotate), (360.0, 3.6));
        let once = Motion { looped: None, ..spin.clone() };
        assert_eq!((once.local_ms(1010.0), once.pose(1010.0).rotate), (1010.0, 360.0));
        let not = Motion { looped: Some(false), ..spin };
        assert_eq!(not.local_ms(2500.0), 2500.0);
    }

    /// The looks a pose shows land on the node's own fields; what it cannot change gives `None`.
    #[test]
    fn dress_sets_paint_fields() {
        let rect = node("rect", json!({"fill": "#111111", "gradient": {"stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "#fff"}]}, "opacity": 0.9})).unwrap();
        let looks = Pose { opacity: Some(0.25), fill: Some([255, 0, 128, 255]), stroke: Some([1, 2, 3, 4]), color: Some([16, 32, 48, 64]), blur: Some(3.0), ..Pose::default() };
        let d = looks.dress(&rect).unwrap();
        assert_eq!((d.opacity, d.fill.as_deref(), d.gradient.as_ref(), d.stroke.as_deref(), d.color.as_deref(), d.blur),
            (Some(0.25), Some("#FF0080FF"), None, Some("#01020304"), Some("#10203040"), Some(3.0)));
        assert_eq!(Node { opacity: rect.opacity, fill: rect.fill.clone(), gradient: rect.gradient.clone(), stroke: None, color: None, blur: None, ..d },
            rect, "nothing else changes");
        // A whole ellipse gets an arc from the top; a ring keeps its start, hole and ends.
        let disc = node("ellipse", json!({})).unwrap();
        let fill = Pose { sweep: Some(120.0), ..Pose::default() };
        assert_eq!(fill.dress(&disc).unwrap().arc, Some(Arc { start: 0.0, sweep: 120.0, inner: 0.0, cap: None }));
        let ring = node("ellipse", json!({"arc": {"start": 90, "sweep": 300, "inner": 0.6, "cap": "round"}})).unwrap();
        let arc = fill.dress(&ring).unwrap().arc.unwrap();
        assert_eq!((arc.start, arc.sweep, arc.inner, arc.cap), (90.0, 120.0, 0.6, Some(crate::design::StrokeCap::Round)));
        // Nothing to change: a sweep off an ellipse, an angle without a gradient that turns, a trim.
        assert_eq!(fill.dress(&rect), None);
        let turn = Pose { angle: Some(45.0), ..Pose::default() };
        assert_eq!(turn.dress(&disc), None);
        let radial = node("rect", json!({"gradient": {"kind": "radial", "stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "#fff"}]}})).unwrap();
        assert_eq!(turn.dress(&radial), None, "a radial gradient has no angle");
        assert_eq!(turn.dress(&rect).unwrap().gradient.unwrap().angle, Some(45.0), "a linear one turns");
        assert_eq!(Pose { trim: Some(0.5), dx: 4.0, ..Pose::default() }.dress(&rect), None, "trim and moves are the painter's");
        assert_eq!(Pose::default().dress(&rect), None);
    }

    /// A stroke key shows its colour, not the stroke gradient under it, as a fill key does with
    /// the gradient; each leaves the other's gradient alone.
    #[test]
    fn dress_stroke_beats_a_stroke_gradient() {
        let stops = json!([{"at": 0, "color": "#4AF0FF"}, {"at": 1, "color": "#7C5CFF"}]);
        let card = node("rect", json!({"stroke_width": 2, "stroke_gradient": {"stops": stops}, "gradient": {"stops": stops}})).unwrap();
        let red = card_pose(json!({"stroke": "#F00"}));
        let d = red.dress(&card).unwrap();
        assert_eq!((d.stroke.as_deref(), d.stroke_gradient.as_ref(), d.gradient.as_ref()), (Some("#FF0000FF"), None, card.gradient.as_ref()));
        let d = card_pose(json!({"fill": "#0F0"})).dress(&card).unwrap();
        assert_eq!((d.fill.as_deref(), d.gradient.as_ref(), d.stroke_gradient.as_ref()), (Some("#00FF00FF"), None, card.stroke_gradient.as_ref()));
    }

    /// The pose of a motion whose two keys both show the looks in `looks`.
    fn card_pose(looks: Value) -> Pose {
        let key = |at_ms: u32| {
            let mut key = looks.clone();
            key["at_ms"] = json!(at_ms);
            key
        };
        motion(json!({"keys": [key(0), key(200)]})).pose(100.0)
    }

    /// An angle key turns a conic ring's gradient and a linear stroke gradient, not a radial one.
    #[test]
    fn an_angle_key_turns_a_conic_ring() {
        let stops = json!([{"at": 0, "color": "#4AF0FF"}, {"at": 1, "color": "#7C5CFF"}]);
        let ring = node("ellipse", json!({"arc": {"sweep": 360, "inner": 0.7}, "gradient": {"kind": "conic", "angle": 30, "stops": stops},
            "stroke_gradient": {"angle": 90, "stops": stops}, "motion": {"ease": "linear", "loop": true, "keys": [{"at_ms": 0, "angle": 0}, {"at_ms": 1000, "angle": 360}]}})).unwrap();
        let pose = ring.motion.as_ref().unwrap().pose(250.0);
        assert_eq!(pose.angle, Some(90.0));
        let d = pose.dress(&ring).unwrap();
        let (g, s) = (d.gradient.unwrap(), d.stroke_gradient.unwrap());
        assert_eq!((g.kind, g.angle, s.kind, s.angle), (GradientKind::Conic, Some(90.0), GradientKind::Linear, Some(90.0)));
        assert_eq!(d.arc, ring.arc, "the ring itself stays as it is");
        // Under a fill key the gradient is gone, and the angle still turns the stroke gradient.
        let radial = Gradient { kind: GradientKind::Radial, ..ring.gradient.clone().unwrap() };
        let mixed = Node { stroke_gradient: Some(radial.clone()), ..ring.clone() };
        let d = Pose { angle: Some(10.0), ..Pose::default() }.dress(&mixed).unwrap();
        assert_eq!((d.gradient.unwrap().angle, d.stroke_gradient), (Some(10.0), Some(radial)));
    }
}
