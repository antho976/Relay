//! Motion (MODEL-SPEC §2.1, §2.6; feature-motion §1): how a node moves when its design plays.
//!
//! A node's `motion` holds keys on a clock: the nearest frame or page with a `timeline`, the node
//! itself included, else its page's own, as long as its latest key. Its stored fields are its
//! look at rest, which everything that does not play shows. Moves (`dx`, `dy`, `rotate`, the
//! scales) are offsets from where layout puts it, so motion never changes layout; looks
//! (`opacity`, `fill`, `stroke`, `color`, `blur`, `sweep`, `angle`, `trim`) show their value
//! instead of its own. Times carry their unit in the name: milliseconds.
//!
//! The types and their checks live here; the sampler that poses a node at a time is the
//! client's (client-motion).

use super::{color_ref, obj_vec, parse_color, NodeKind, MAX_COORD};
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
/// the client's sampler; never stored.
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
}
