//! Reuse (MODEL-SPEC §2.5, §2.9; feature-reuse §1): a design's named colours, which any colour
//! field of a node takes as `$name`, its named styles, which a node takes with `style`, and the
//! ids a `copy` gives what it copies.
//!
//! The rules live here so the engine and the canvas apply a colour or style edit identically, as
//! they share `Node::patched`: [`Styles::set_color`], [`Styles::set_style`] and
//! [`Styles::delete`] change the names and answer the edits that undo them, re-creates first;
//! [`Styles::check`] says whether every name a node uses is there (a lenient list's re-adds
//! [`Styles::forget_missing`] instead); [`Styles::resolve`] is a node as it draws; and
//! [`Styles::detach`] is the update that keeps a node's look once a name it used is gone. The
//! engine never resolves: it stores the names and checks them.
//!
//! A node takes each style field it does not set itself and its kind uses ([`Style::lay_under`]),
//! with `fill`/`gradient`, `radius`/`radii`, `shadow`/`shadows` and `stroke`/`stroke_gradient`
//! each counted as one field, so a node's own colour is never painted over by its style's
//! gradient.

use super::hints::bad_value;
use super::{
    field_hint, field_kinds, nearest, obj_opt, obj_vec_opt, object_only, parse_color, plain_label, takes, valid_id, Blend,
    Gradient, Node, NodeKind, NodeOp, Shadow, StrokeCap, StrokeJoin, TextAlign, TextCase, TextRun, MAX_COORD, MAX_ID,
    MAX_NAME, MAX_SHADOWS, MAX_SHADOW_BLUR,
};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::de::{DeserializeSeed, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

/// Most named colours and styles one design holds, together.
pub const MAX_STYLES: usize = 256;

/// The style fields, in order: what a [`Style`] holds, what copy and paste style carry, and what a
/// node sets for itself over its style.
pub const STYLE_FIELDS: &[&str] = &[
    "fill", "gradient", "stroke", "stroke_width", "dash", "radius", "radii", "shadow", "shadows", "opacity", "blur", "blend",
    "color", "font", "font_size", "font_weight", "italic", "underline", "strike", "align", "line_height", "letter_spacing",
    "text_case", "stroke_cap", "stroke_join", "stroke_gradient",
];

/// The name in a `$name` colour, when `text` is one: `$` and a name as an id is written.
pub fn color_ref(text: &str) -> Option<&str> {
    text.strip_prefix('$').filter(|name| valid_id(name))
}

/// The id `copy` gives node `id` of the subtree it copies from `from` as `to`: `to` for the
/// copied node itself; for an id that starts with `from` and one of `-`, `_`, `.` or `:`, that
/// start swapped for `to` (`card-title` → `card-2-title`); any other gets `to:` in front
/// (`text-14` → `card-2:text-14`). So a copy of a copy does not grow (`card-2:text-14` copied
/// from `card-2` as `card-3` is `card-3:text-14`). `None` when the result is longer than an id
/// may be.
pub fn copy_id(from: &str, to: &str, id: &str) -> Option<String> {
    let copied = if id == from {
        to.to_string()
    } else {
        match id.strip_prefix(from).filter(|rest| rest.starts_with(['-', '_', '.', ':'])) {
            Some(rest) => format!("{to}{rest}"),
            None => format!("{to}:{id}"),
        }
    };
    (copied.len() <= MAX_ID).then_some(copied)
}

/// A named look nodes share: any of the style fields, each as a node holds it (a colour may be a
/// `$name`). A node takes each field it does not set itself; `fill` and `gradient` count as one
/// field, and so do `radius` and `radii`, `shadow` and `shadows`, `stroke` and `stroke_gradient`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignStyle")]
pub struct Style {
    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub gradient: Option<Gradient>,
    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1000))]
    pub stroke_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 8), inner(range(min = 0, max = 1000)))]
    pub dash: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(inner(range(min = 0, max = MAX_COORD)))]
    pub radii: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub shadow: Option<Shadow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_vec_opt")]
    #[schemars(length(min = 1, max = MAX_SHADOWS))]
    pub shadows: Option<Vec<Shadow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1))]
    pub opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 200))]
    pub blur: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Blend>,
    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_NAME))]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<TextAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.1, max = 10))]
    pub line_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -100, max = 1000))]
    pub letter_spacing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_case: Option<TextCase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_cap: Option<StrokeCap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_join: Option<StrokeJoin>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub stroke_gradient: Option<Gradient>,
}

/// A design's named colours and styles: one namespace, at most [`MAX_STYLES`] names together,
/// stored as one JSON object and read whole.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DesignStyles")]
pub struct Styles {
    /// Each colour's literal value (`#RGB`, `#RRGGBB` or `#RRGGBBAA`), by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, String>,
    /// Each style, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", deserialize_with = "obj_map")]
    pub styles: BTreeMap<String, Style>,
}

/// A colour or style edit refused: `code` is the part of the error code after `design.`
/// (`style_invalid`, `style_not_found`, `style_exists`, `too_large`), and `why` starts with the
/// field or name at fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleError {
    pub code: &'static str,
    pub why: String,
}

impl StyleError {
    pub fn new(code: &'static str, why: impl Into<String>) -> StyleError {
        StyleError { code, why: why.into() }
    }
}

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.why)
    }
}

/// What a `delete_style` took out: a colour's value, or a style.
#[derive(Debug, Clone, PartialEq)]
// One per delete_style edit, taken apart at once: boxing the style would buy nothing.
#[allow(clippy::large_enum_variant)]
pub enum Removed {
    Color(String),
    Style(Style),
}

/// Bit `i` stands for `STYLE_FIELDS[i]`; a field's bit, found while compiling (a name that is no
/// style field does not compile).
const fn field_bit(name: &str) -> u32 {
    let (name, mut i) = (name.as_bytes(), 0);
    while i < STYLE_FIELDS.len() {
        let field = STYLE_FIELDS[i].as_bytes();
        if field.len() == name.len() {
            let mut j = 0;
            while j < name.len() && field[j] == name[j] {
                j += 1;
            }
            if j == name.len() {
                return 1 << i;
            }
        }
        i += 1;
    }
    panic!("not a style field")
}

const DASH: u32 = field_bit("dash");
const STROKE_CAP: u32 = field_bit("stroke_cap");

/// The code each style field needs, written once over the fields: `Style` and `Node` name them
/// alike (`of`'s struct literal fails to compile when a `Style` field is left out here).
macro_rules! per_field {
    ($($field:ident),* $(,)?) => {
        impl Style {
            /// The style fields `node` sets itself, as a style: its own look, without what its
            /// style lays under it.
            pub fn of(node: &Node) -> Style {
                Style { $($field: node.$field.clone()),* }
            }

            /// The fields it holds, as bits.
            fn held(&self) -> u32 {
                let mut bits = 0;
                $(if self.$field.is_some() {
                    bits |= const { field_bit(stringify!($field)) };
                })*
                bits
            }

            /// Write the fields of `bits` into `node`.
            fn lay(&self, bits: u32, node: &mut Node) {
                $(if bits & const { field_bit(stringify!($field)) } != 0 {
                    node.$field = self.$field.clone();
                })*
            }
        }

        /// Whether `node` sets style field `field` itself; a value this build cannot read, kept in
        /// `extra`, counts (MODEL-SPEC §2.3).
        fn sets(node: &Node, field: &str) -> bool {
            (!node.extra.is_empty() && node.extra.contains_key(field))
                || match field {
                    $(stringify!($field) => node.$field.is_some(),)*
                    _ => false,
                }
        }
    };
}

per_field!(
    fill, gradient, stroke, stroke_width, dash, radius, radii, shadow, shadows, opacity, blur, blend, color, font, font_size,
    font_weight, italic, underline, strike, align, line_height, letter_spacing, text_case, stroke_cap, stroke_join,
    stroke_gradient,
);

/// The field that counts as one with `field` when a style lays itself under a node: a node's own
/// `fill` is not painted over by its style's `gradient`, nor its `radius` by `radii`, its
/// `shadow` by `shadows` or its `stroke` by `stroke_gradient`, and the other way round.
fn partner(field: &str) -> Option<&'static str> {
    Some(match field {
        "fill" => "gradient",
        "gradient" => "fill",
        "radius" => "radii",
        "radii" => "radius",
        "shadow" => "shadows",
        "shadows" => "shadow",
        "stroke" => "stroke_gradient",
        "stroke_gradient" => "stroke",
        _ => return None,
    })
}

/// Whether a dash has 0-length dashes (round dots), which butt ends draw nothing of. A dash's
/// dashes are its even entries; an odd list repeats, so then every entry is one in turn.
fn dotted(dash: &[f64]) -> bool {
    dash.iter().enumerate().any(|(i, v)| *v == 0.0 && (i % 2 == 0 || dash.len() % 2 == 1))
}

/// The refusal of a style or node holding both `shadow` and `shadows`, as a node's.
const BOTH_SHADOWS: &str = "shadows: give shadow or shadows, not both (shadows is the list)";

impl Style {
    /// Ranges and colours exactly as a node's (laid on a probe rect and checked by
    /// `Node::validate`, so a message names the field: "font_size: 2000 is outside 1 to 1000"),
    /// and what a caller may write in a node: a font without control characters, `shadow` or
    /// `shadows` but not both, and each shadow's blur within 200. An inner shadow or a spread is
    /// accepted: a box draws it, and a text, line, path or icon the style lies under casts its
    /// outer shadows only, as paint reads them (MODEL-SPEC §1 #13).
    pub fn validate(&self) -> Result<(), String> {
        let mut probe = Node::new(NodeKind::Rect);
        probe.parent = Some("style".into());
        self.lay(self.held(), &mut probe);
        probe.validate()?;
        if self.font.as_deref().is_some_and(|f| !plain_label(f)) {
            return Err("font: no control characters".into());
        }
        if self.shadow.is_some() && self.shadows.is_some() {
            return Err(BOTH_SHADOWS.into());
        }
        let shadows = self.shadow.iter().map(|s| ("shadow".to_string(), s));
        let shadows = shadows.chain(self.shadows.iter().flatten().enumerate().map(|(i, s)| (format!("shadows[{i}]"), s)));
        for (field, s) in shadows {
            if s.blur > MAX_SHADOW_BLUR {
                return Err(format!("{field}.blur: {} is outside 0 to {MAX_SHADOW_BLUR}", s.blur));
            }
        }
        Ok(())
    }

    /// This style with `set` applied (a `style` edit): each key is a style field, `null` takes it
    /// out. Validated; the error names the key.
    pub fn patched(&self, set: &Map<String, Value>) -> Result<Style, String> {
        let mut fields = self.to_set();
        for (key, value) in set {
            if !STYLE_FIELDS.contains(&key.as_str()) {
                return Err(not_a_style_field(key));
            }
            if value.is_null() {
                fields.remove(key);
            } else {
                fields.insert(key.clone(), value.clone());
            }
        }
        let style = serde_json::from_value::<Style>(Value::Object(fields)).map_err(|error| {
            // The fields held already read, so a value of `set` did not: name it.
            set.iter()
                .filter(|(_, value)| !value.is_null())
                .find_map(|(key, value)| {
                    let alone = Map::from_iter([(key.clone(), value.clone())]);
                    let why = serde_json::from_value::<Style>(Value::Object(alone)).err()?;
                    Some(bad_value(key, value, &why.to_string()))
                })
                .unwrap_or_else(|| error.to_string())
        })?;
        style.validate()?;
        Ok(style)
    }

    /// The fields `set` would change, at their current values (`null` where absent): the `style`
    /// edit that undoes it.
    pub fn before(&self, set: &Map<String, Value>) -> Map<String, Value> {
        let current = self.to_set();
        set.keys().map(|key| (key.clone(), current.get(key).cloned().unwrap_or(Value::Null))).collect()
    }

    /// Every field it holds, as the `set` of a `style` edit that makes it again.
    pub fn to_set(&self) -> Map<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(fields)) => fields,
            _ => Map::new(),
        }
    }

    /// Lay it under `node`: each field it holds that the node's kind uses and the node does not
    /// set itself (`fill`/`gradient`, `radius`/`radii`, `shadow`/`shadows` and
    /// `stroke`/`stroke_gradient` counting as one field each; a key held in `extra` counts as
    /// set). A style's radius never lands on an ellipse, nor its stroke join on a box.
    pub fn lay_under(&self, node: &mut Node) {
        let bits = self.under(node);
        self.lay(bits, node);
    }

    /// The fields [`Style::lay_under`] writes into `node`, as bits. A node's own butt ends and a
    /// style's dotted dash (or the other way round) are not combined: butt ends draw nothing of
    /// a dot, which `Node::validate` refuses, so the style's half is left out and the drawn node
    /// is one a value rule takes (and a detach can write).
    fn under(&self, node: &Node) -> u32 {
        let held = self.held();
        let mut bits = 0;
        for (i, field) in STYLE_FIELDS.iter().enumerate() {
            let bit = 1 << i;
            if held & bit != 0
                && takes(node.kind, field)
                && !sets(node, field)
                && partner(field).is_none_or(|other| !sets(node, other))
            {
                bits |= bit;
            }
        }
        let dash = if bits & DASH != 0 { self.dash.as_deref() } else { node.dash.as_deref() };
        let cap = if bits & STROKE_CAP != 0 { self.stroke_cap } else { node.stroke_cap };
        if cap == Some(StrokeCap::Butt) && dash.is_some_and(dotted) {
            bits &= !(DASH | STROKE_CAP);
        }
        bits
    }

    /// The colours it names, each with the field a refusal names: `fill`, `stroke`, `color`,
    /// `shadow`, `shadows`, `gradient` or `stroke_gradient` (the `$` left off the name).
    pub fn names(&self) -> Vec<(&'static str, &str)> {
        let mut out = Vec::new();
        for (field, c) in [("fill", &self.fill), ("stroke", &self.stroke), ("color", &self.color)] {
            out.extend(c.as_deref().and_then(color_ref).map(|name| (field, name)));
        }
        for (field, shadows) in [("shadow", self.shadow.as_slice()), ("shadows", self.shadows.as_deref().unwrap_or_default())] {
            out.extend(shadows.iter().filter_map(|s| color_ref(&s.color)).map(|name| (field, name)));
        }
        for (field, g) in [("gradient", &self.gradient), ("stroke_gradient", &self.stroke_gradient)] {
            out.extend(g.iter().flat_map(|g| &g.stops).filter_map(|s| color_ref(&s.color)).map(|name| (field, name)));
        }
        out
    }
}

/// The refusal of `key` in a style's `set`: a node field a style does not hold, or no field.
fn not_a_style_field(key: &str) -> String {
    if field_kinds(key).is_some() {
        return format!("{key}: not a style field; set it on each node (a style holds {})", STYLE_FIELDS.join(", "));
    }
    match (field_hint(key), nearest(key, STYLE_FIELDS.iter().copied())) {
        (Some(hint), _) => format!("{key}: not a style field; {hint}"),
        (None, Some(near)) => format!("{key}: not a style field; did you mean {near:?}?"),
        (None, None) => format!("{key}: not a style field; a style holds {}", STYLE_FIELDS.join(", ")),
    }
}

/// What becomes of a `$name` colour in a node or a style.
enum Fix {
    Keep,
    /// The colour itself written in.
    Write(String),
    /// What holds it taken out: a field, one shadow of a list, one gradient stop (the gradient
    /// with fewer than 2 left), or a run's colour (the run when it keeps no style of its own).
    Drop,
}

/// The colour slots of a node or a style, open for writing.
struct Slots<'a> {
    plain: [(&'static str, &'a mut Option<String>); 3],
    shadow: &'a mut Option<Shadow>,
    shadows: &'a mut Option<Vec<Shadow>>,
    gradients: [(&'static str, &'a mut Option<Gradient>); 2],
    runs: Option<&'a mut Option<Vec<TextRun>>>,
}

fn node_slots(n: &mut Node) -> Slots<'_> {
    Slots {
        plain: [("fill", &mut n.fill), ("stroke", &mut n.stroke), ("color", &mut n.color)],
        shadow: &mut n.shadow,
        shadows: &mut n.shadows,
        gradients: [("gradient", &mut n.gradient), ("stroke_gradient", &mut n.stroke_gradient)],
        runs: Some(&mut n.runs),
    }
}

fn style_slots(s: &mut Style) -> Slots<'_> {
    Slots {
        plain: [("fill", &mut s.fill), ("stroke", &mut s.stroke), ("color", &mut s.color)],
        shadow: &mut s.shadow,
        shadows: &mut s.shadows,
        gradients: [("gradient", &mut s.gradient), ("stroke_gradient", &mut s.stroke_gradient)],
        runs: None,
    }
}

/// Each `$name` colour of `slots` fixed as `fix` says for its name; the fields it changed, each
/// once (Node::names' order).
fn fix_colors(slots: Slots, fix: &mut dyn FnMut(&str) -> Fix) -> Vec<&'static str> {
    let mut changed = Vec::new();
    for (field, slot) in slots.plain {
        let Some(name) = slot.as_deref().and_then(color_ref) else { continue };
        match fix(name) {
            Fix::Keep => continue,
            Fix::Write(value) => *slot = Some(value),
            Fix::Drop => *slot = None,
        }
        changed.push(field);
    }
    if let Some(name) = slots.shadow.as_ref().and_then(|s| color_ref(&s.color)) {
        match fix(name) {
            Fix::Keep => {}
            Fix::Write(value) => {
                if let Some(s) = slots.shadow.as_mut() {
                    s.color = value;
                }
                changed.push("shadow");
            }
            Fix::Drop => {
                *slots.shadow = None;
                changed.push("shadow");
            }
        }
    }
    if let Some(list) = slots.shadows.as_mut() {
        let mut touched = false;
        list.retain_mut(|s| match color_ref(&s.color).map(&mut *fix) {
            None | Some(Fix::Keep) => true,
            Some(Fix::Write(value)) => {
                s.color = value;
                touched = true;
                true
            }
            Some(Fix::Drop) => {
                touched = true;
                false
            }
        });
        if touched {
            changed.push("shadows");
            if list.is_empty() {
                *slots.shadows = None;
            }
        }
    }
    for (field, slot) in slots.gradients {
        let Some(g) = slot.as_mut() else { continue };
        let mut touched = false;
        g.stops.retain_mut(|stop| match color_ref(&stop.color).map(&mut *fix) {
            None | Some(Fix::Keep) => true,
            Some(Fix::Write(value)) => {
                stop.color = value;
                touched = true;
                true
            }
            Some(Fix::Drop) => {
                touched = true;
                false
            }
        });
        if touched {
            changed.push(field);
            if g.stops.len() < 2 {
                *slot = None;
            }
        }
    }
    if let Some(slot) = slots.runs {
        if let Some(runs) = slot.as_mut() {
            let mut touched = false;
            runs.retain_mut(|run| match run.color.as_deref().and_then(color_ref).map(&mut *fix) {
                None | Some(Fix::Keep) => true,
                Some(Fix::Write(value)) => {
                    run.color = Some(value);
                    touched = true;
                    true
                }
                Some(Fix::Drop) => {
                    run.color = None;
                    touched = true;
                    run.styled()
                }
            });
            if touched {
                changed.push("runs");
                if runs.is_empty() {
                    *slot = None;
                }
            }
        }
    }
    changed
}

/// The fix that writes colour `name`'s `value` in and keeps every other.
fn write_in<'a>(name: &'a str, value: &'a str) -> impl FnMut(&str) -> Fix + 'a {
    move |color| if color == name { Fix::Write(value.to_string()) } else { Fix::Keep }
}

/// The JSON of each of `fields` in `node`, as an update writes it (`null` for one it no longer
/// holds).
fn fields_of(node: &Node, fields: &[&'static str]) -> Map<String, Value> {
    let map = node.to_map();
    fields.iter().map(|field| (field.to_string(), map.get(*field).cloned().unwrap_or(Value::Null))).collect()
}

/// The rule names follow, said for a colour or a style.
fn bad_name(name: &str, what: &str) -> StyleError {
    let why = match name.strip_prefix('$').filter(|bare| valid_id(bare)) {
        Some(bare) => format!("name: give the {what}'s name without its $ ({bare:?})"),
        None => format!("name: {name:?} is not a {what} name: use 1 to 64 letters, digits, '-', '_', '.' or ':'"),
    };
    StyleError::new("style_invalid", why)
}

impl Styles {
    /// Whether `node` names any colour or style, and so draws otherwise than it is stored.
    pub fn uses(node: &Node) -> bool {
        !node.names().is_empty()
    }

    /// `node` as it draws: its style laid under it, then each `$name` as the colour it names. A
    /// name the design does not hold (none should be left) is taken out as
    /// [`Styles::forget_missing`] takes it, so the kind's default shows.
    pub fn resolve(&self, node: &Node) -> Node {
        let mut drawn = node.clone();
        match node.style.as_deref().map(|name| self.styles.get(name)) {
            Some(Some(style)) => style.lay_under(&mut drawn),
            Some(None) => drawn.style = None,
            None => {}
        }
        fix_colors(node_slots(&mut drawn), &mut |name| match self.colors.get(name) {
            Some(value) => Fix::Write(value.clone()),
            None => Fix::Drop,
        });
        drawn
    }

    /// Every name `node` uses is one the design holds; else `style_not_found` naming the field
    /// ("style: the design has no style \"cards\"; did you mean \"card\"?", "fill: the design has
    /// no colour $acent; did you mean $accent?").
    pub fn check(&self, node: &Node) -> Result<(), StyleError> {
        for (field, name) in node.names() {
            match field {
                "style" if !self.styles.contains_key(name) => return Err(self.no_style(name)),
                "style" => {}
                _ if !self.colors.contains_key(name) => return Err(self.no_color(field, name)),
                _ => {}
            }
        }
        Ok(())
    }

    /// Take out of `node` only what names something the design does not hold (a lenient list's
    /// re-adds): a field naming a missing colour, one entry of `shadows`, one gradient stop (the
    /// gradient when fewer than 2 are left), one run's colour (the run when it keeps no style of
    /// its own), and a missing `style`. The fields it changed.
    pub fn forget_missing(&self, node: &mut Node) -> Vec<&'static str> {
        let mut changed =
            fix_colors(node_slots(node), &mut |name| if self.colors.contains_key(name) { Fix::Keep } else { Fix::Drop });
        if node.style.as_ref().is_some_and(|name| !self.styles.contains_key(name)) {
            node.style = None;
            changed.push("style");
        }
        changed
    }

    /// Name colour `name` as literal `value`, or change it. Answers the edit that undoes it:
    /// `delete_style` for a new name, else `color` with the old value. Refused: a name not
    /// written as an id, or a value that is no literal colour (`style_invalid`); a name a style
    /// holds (`style_exists`); a new name past [`MAX_STYLES`] (`too_large`).
    pub fn set_color(&mut self, name: &str, value: &str) -> Result<NodeOp, StyleError> {
        if !valid_id(name) {
            return Err(bad_name(name, "colour"));
        }
        if parse_color(value).is_none() {
            let why = match color_ref(value) {
                Some(_) => "value: a colour's value is #RGB, #RRGGBB or #RRGGBBAA, not another $name".to_string(),
                None => format!("value: {value:?} is not #RGB, #RRGGBB or #RRGGBBAA"),
            };
            return Err(StyleError::new("style_invalid", why));
        }
        if self.styles.contains_key(name) {
            return Err(StyleError::new("style_exists", format!("name: {name:?} is a style; give the colour another name")));
        }
        let undo = match self.colors.get(name) {
            Some(old) => NodeOp::Color { name: name.into(), value: old.clone() },
            None => {
                self.room()?;
                NodeOp::DeleteStyle { name: name.into() }
            }
        };
        self.colors.insert(name.into(), value.into());
        Ok(undo)
    }

    /// Make style `name` from `set` (patched onto an empty style), or patch it. Each `$name`
    /// colour it then holds must be one the design names. Answers the edit that undoes it:
    /// `delete_style` for a new style, else `style` with the fields' old values. Refused as
    /// [`Styles::set_color`] is, a value as [`Style::patched`] says (`style_invalid`), and a
    /// colour it names that is not here (`style_not_found`).
    pub fn set_style(&mut self, name: &str, set: &Map<String, Value>) -> Result<NodeOp, StyleError> {
        if !valid_id(name) {
            return Err(bad_name(name, "style"));
        }
        if self.colors.contains_key(name) {
            return Err(StyleError::new("style_exists", format!("name: {name:?} is a colour; give the style another name")));
        }
        let old = self.styles.get(name);
        if old.is_none() {
            self.room()?;
        }
        let style = old.cloned().unwrap_or_default().patched(set).map_err(|why| StyleError::new("style_invalid", why))?;
        if let Some((field, missing)) = style.names().into_iter().find(|(_, color)| !self.colors.contains_key(*color)) {
            return Err(self.no_color(field, missing));
        }
        let undo = match old {
            Some(old) => NodeOp::Style { name: name.into(), set: old.before(set) },
            None => NodeOp::DeleteStyle { name: name.into() },
        };
        self.styles.insert(name.into(), style);
        Ok(undo)
    }

    /// Take colour or style `name` out. A colour is written into every style that names it.
    /// Answers what went and the edits that undo it so far, in the order they replay: the
    /// re-create first, then each style patched back to name the colour again. The caller
    /// detaches the nodes that use it ([`Styles::detach`]) and appends their undos.
    pub fn delete(&mut self, name: &str) -> Result<(Removed, Vec<NodeOp>), StyleError> {
        if let Some(style) = self.styles.remove(name) {
            let undo = vec![NodeOp::Style { name: name.into(), set: style.to_set() }];
            return Ok((Removed::Style(style), undo));
        }
        let Some(value) = self.colors.remove(name) else {
            let near = nearest(name, self.colors.keys().chain(self.styles.keys()).map(String::as_str));
            let why = match near {
                Some(near) => format!("name: the design has no colour or style {name:?}; did you mean {near:?}?"),
                None => format!("name: the design has no colour or style {name:?}"),
            };
            return Err(StyleError::new("style_not_found", why));
        };
        let mut undo = vec![NodeOp::Color { name: name.into(), value: value.clone() }];
        for (style_name, style) in &mut self.styles {
            if !style.names().iter().any(|(_, color)| *color == name) {
                continue;
            }
            let old = style.clone();
            let changed = fix_colors(style_slots(style), &mut write_in(name, &value));
            let now = style.to_set();
            let set: Map<String, Value> =
                changed.iter().map(|field| (field.to_string(), now.get(*field).cloned().unwrap_or(Value::Null))).collect();
            undo.push(NodeOp::Style { name: style_name.clone(), set: old.before(&set) });
        }
        Ok((Removed::Color(value), undo))
    }

    /// The update that keeps `node` looking as it did once `removed`, named `name`, is gone, or
    /// `None` when the node does not use it. For a style: `style: null` and each field the style
    /// laid under the node (only fields its kind uses, as [`Style::lay_under`] lays them). For a
    /// colour: each field naming it, with the colour written in, whole (`shadow`, `shadows`, a
    /// gradient and `runs` as objects and lists); a field the node's kind does not use is
    /// rewritten too, since it already holds it and would otherwise name nothing.
    pub fn detach(node: &Node, name: &str, removed: &Removed) -> Option<Map<String, Value>> {
        match removed {
            Removed::Style(style) => {
                if node.style.as_deref() != Some(name) {
                    return None;
                }
                let held = style.to_set();
                let bits = style.under(node);
                let mut set = Map::from_iter([("style".to_string(), Value::Null)]);
                for (i, field) in STYLE_FIELDS.iter().enumerate() {
                    if let Some(value) = held.get(*field).filter(|_| bits & (1 << i) != 0) {
                        set.insert(field.to_string(), value.clone());
                    }
                }
                Some(set)
            }
            Removed::Color(value) => {
                if !node.names().iter().any(|(field, color)| *field != "style" && *color == name) {
                    return None;
                }
                let mut kept = node.clone();
                let changed = fix_colors(node_slots(&mut kept), &mut write_in(name, value));
                Some(fields_of(&kept, &changed))
            }
        }
    }

    /// Room for one more name, or `too_large`.
    fn room(&self) -> Result<(), StyleError> {
        if self.colors.len() + self.styles.len() >= MAX_STYLES {
            return Err(StyleError::new(
                "too_large",
                format!("a design holds at most {MAX_STYLES} colours and styles together; delete_style one it no longer uses"),
            ));
        }
        Ok(())
    }

    /// The refusal of `$name` in colour field `field`.
    fn no_color(&self, field: &str, name: &str) -> StyleError {
        let why = if self.styles.contains_key(name) {
            format!("{field}: ${name} is a style, not a colour; a node takes a style with style: {name:?}")
        } else {
            match nearest(name, self.colors.keys().map(String::as_str)) {
                Some(near) => format!("{field}: the design has no colour ${name}; did you mean ${near}?"),
                None => format!(
                    "{field}: the design has no colour ${name}; name it first with {{\"op\": \"color\", \"name\": {name:?}, \"value\": \"#RRGGBB\"}}"
                ),
            }
        };
        StyleError::new("style_not_found", why)
    }

    /// The refusal of `style: name`.
    fn no_style(&self, name: &str) -> StyleError {
        let why = if self.colors.contains_key(name) {
            format!("style: {name:?} is a colour, not a style; a colour field takes it as \"${name}\"")
        } else {
            match nearest(name, self.styles.keys().map(String::as_str)) {
                Some(near) => format!("style: the design has no style {name:?}; did you mean {near:?}?"),
                None => format!("style: the design has no style {name:?}; make it first with {{\"op\": \"style\", \"name\": {name:?}, \"set\": {{…}}}}"),
            }
        };
        StyleError::new("style_not_found", why)
    }
}

/// A `style` edit's `set`: the style fields by name.
pub(crate) fn style_set_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "description": "Style fields to set, each with a value as a node takes it; null takes the field out of the style.",
        "propertyNames": {"enum": STYLE_FIELDS},
    })
}

/// A map of styles, each read only from a JSON object (TM-16): serde would read an array field by
/// field.
fn obj_map<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<BTreeMap<String, T>, D::Error> {
    struct Object<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for Object<T> {
        type Value = T;
        fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<T, D::Error> {
            object_only(d)
        }
    }
    struct Objects<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Objects<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object of objects")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<BTreeMap<String, T>, A::Error> {
            let mut out = BTreeMap::new();
            while let Some(name) = map.next_key::<String>()? {
                out.insert(name, map.next_value_seed(Object(PhantomData))?);
            }
            Ok(out)
        }
    }
    d.deserialize_map(Objects(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{field_kinds, Node};
    use serde_json::{json, Value};

    /// Reuse test list: copy ids swap the copied node's prefix, else add one, within 64.
    #[test]
    fn copy_ids_swap_the_prefix_or_add_one() {
        for (id, want) in [
            ("card", "card-2"),
            ("card-title", "card-2-title"),
            ("card_photo", "card-2_photo"),
            ("card.inner", "card-2.inner"),
            ("card:dot", "card-2:dot"),
            ("text-14", "card-2:text-14"),
            ("cardholder", "card-2:cardholder"),
        ] {
            assert_eq!(copy_id("card", "card-2", id).as_deref(), Some(want), "{id}");
        }
        // A copy of a copy does not grow.
        assert_eq!(copy_id("card-2", "card-3", "card-2:text-14").as_deref(), Some("card-3:text-14"));
        // Past 64 characters there is no id.
        let long = "x".repeat(60);
        assert_eq!(copy_id("card", &long, "text-14"), None);
        assert_eq!(copy_id("card", &"y".repeat(64), "card").map(|id| id.len()), Some(64));
        // The one case two ids meet: X and {from}:X both become {to}:X (the engine refuses it).
        assert_eq!(copy_id("card", "c2", "x"), copy_id("card", "c2", "card:x"));
    }

    #[test]
    fn colour_names_are_ids_after_a_dollar() {
        assert_eq!(color_ref("$accent"), Some("accent"));
        assert_eq!(color_ref("$brand.primary-2"), Some("brand.primary-2"));
        let long = format!("${}", "a".repeat(65));
        for not in ["$", "$bad name", "accent", "#fff", "$$a", long.as_str()] {
            assert_eq!(color_ref(not), None, "{not}");
        }
    }

    /// A style holds exactly the style fields, in STYLE_FIELDS' order, each read as a node reads
    /// the field of the same name.
    #[test]
    fn style_fields_are_node_fields() {
        let full = json!({
            "fill": "$accent", "gradient": {"kind": "conic", "stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "#fff"}]},
            "stroke": "#000", "stroke_width": 1, "dash": [6, 4], "radius": 12, "radii": [1, 2, 3, 4], "shadow": {"blur": 4, "color": "#00000033"},
            "shadows": [{"blur": 8, "color": "#000"}], "opacity": 0.5, "blur": 2, "blend": "multiply", "color": "#111", "font": "Geist",
            "font_size": 14, "font_weight": 600, "italic": true, "underline": true, "strike": false, "align": "center", "line_height": 1.4,
            "letter_spacing": 0.5, "text_case": "upper", "stroke_cap": "round", "stroke_join": "bevel",
            "stroke_gradient": {"stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "#fff"}]}
        });
        let style: Style = serde_json::from_value(full.clone()).unwrap();
        let written = serde_json::to_value(&style).unwrap();
        let keys: Vec<&str> = written.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, STYLE_FIELDS, "written in STYLE_FIELDS' order");
        assert_eq!(STYLE_FIELDS.len(), 26);
        let schema = serde_json::to_value(schemars::schema_for!(Style)).unwrap();
        let named: Vec<&str> = schema["properties"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(named, STYLE_FIELDS, "the struct's own order");
        assert_eq!(schema["additionalProperties"], json!(false));
        // Each reads as the node's field of the same name, and is one.
        let mut body = full.as_object().unwrap().clone();
        body.insert("kind".into(), json!("rect"));
        body.insert("parent".into(), json!("p"));
        let node = Node::read(Value::Object(body)).unwrap();
        assert!(node.extra.is_empty(), "{:?}", node.extra);
        assert!(STYLE_FIELDS.iter().all(|f| field_kinds(f).is_some()));
        // A node's own look is a style of the same fields, and each field's bit is its place.
        assert_eq!(Style::of(&node), style);
        assert_eq!(style.held(), (1 << STYLE_FIELDS.len()) - 1);
        assert!(STYLE_FIELDS.iter().enumerate().all(|(i, f)| field_bit(f) == 1 << i));
        assert_eq!((DASH, STROKE_CAP), (1 << 4, 1 << 23));
        // A key that is not a style field is refused by name; a nested array is not an object.
        assert!(serde_json::from_value::<Style>(json!({"w": 1})).unwrap_err().to_string().contains("unknown field `w`"));
        assert!(serde_json::from_value::<Style>(json!({"shadow": [0, 1, 2, 0, "#000"]})).is_err());
        assert!(serde_json::from_value::<Style>(json!({"shadows": [[0, 1, 2, 0, "#000"]]})).is_err());
    }

    #[test]
    fn styles_read_whole_and_write_only_what_they_hold() {
        let styles: Styles = serde_json::from_value(json!({"colors": {"accent": "#4C9DFF"}, "styles": {"card": {"radius": 12, "fill": "$accent"}}})).unwrap();
        assert_eq!(styles.colors["accent"], "#4C9DFF");
        assert_eq!(styles.styles["card"].radius, Some(12.0));
        assert_eq!(serde_json::to_value(Styles::default()).unwrap(), json!({}));
        assert_eq!(serde_json::from_value::<Styles>(json!({})).unwrap(), Styles::default());
        // TM-16: a style written as an array is not read field by field.
        assert!(serde_json::from_value::<Styles>(json!({"styles": {"card": ["#fff"]}})).is_err());
        let error = StyleError::new("style_not_found", "style: the design has no style \"crad\"");
        assert_eq!((error.code, error.to_string()), ("style_not_found", "style: the design has no style \"crad\"".to_string()));
        assert_ne!(Removed::Color("#fff".into()), Removed::Style(Style::default()));
    }

    fn set(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    fn node(value: Value) -> Node {
        Node::read(value).unwrap()
    }

    /// Colour and style edits applied in order, as the engine and the scene apply them; the
    /// edits that undo the whole list, in the order they replay.
    fn replay(styles: &mut Styles, ops: &[NodeOp]) -> Vec<NodeOp> {
        let undos: Vec<Vec<NodeOp>> = ops
            .iter()
            .map(|op| match op {
                NodeOp::Color { name, value } => vec![styles.set_color(name, value).unwrap()],
                NodeOp::Style { name, set } => vec![styles.set_style(name, set).unwrap()],
                NodeOp::DeleteStyle { name } => styles.delete(name).unwrap().1,
                other => panic!("not a colour or style edit: {other:?}"),
            })
            .collect();
        undos.into_iter().rev().flatten().collect()
    }

    /// A design as an agent names it: a palette, then looks that use it.
    fn palette() -> Styles {
        let mut styles = Styles::default();
        replay(&mut styles, &[
            NodeOp::Color { name: "ink".into(), value: "#141414".into() },
            NodeOp::Color { name: "accent".into(), value: "#4C9DFF".into() },
            NodeOp::Color { name: "paper".into(), value: "#FFFDF8".into() },
            NodeOp::Style { name: "card".into(), set: set(json!({
                "gradient": {"angle": 90, "stops": [{"at": 0, "color": "$accent"}, {"at": 1, "color": "$paper"}]},
                "radii": [16, 16, 4, 4], "shadows": [{"y": 4, "blur": 16, "color": "$ink"}, {"blur": 2, "color": "#0000001F"}],
                "stroke_gradient": {"stops": [{"at": 0, "color": "$ink"}, {"at": 1, "color": "#FFFFFF"}]},
                "stroke_width": 2, "dash": [6, 4], "stroke_cap": "butt", "stroke_join": "bevel", "opacity": 0.9,
                "font_size": 20, "color": "$ink"
            })) },
            NodeOp::Style { name: "title".into(), set: set(json!({
                "font": "Sora", "font_size": 30, "font_weight": 700, "color": "$ink", "text_case": "upper", "letter_spacing": 1,
                "fill": "$paper"
            })) },
        ]);
        styles
    }

    /// Reuse 8: a node's own fields win over its style's, one field of a pair blocking the other,
    /// and `$name` colours resolve wherever they are written.
    #[test]
    fn a_style_lays_under_own_fields() {
        let styles = palette();
        let own = node(json!({"id": "a", "parent": "p", "kind": "rect", "style": "card", "fill": "#000", "radius": 4,
            "shadow": {"blur": 2, "color": "#000"}, "stroke": "$accent"}));
        let drawn = styles.resolve(&own);
        assert_eq!((drawn.fill.as_deref(), &drawn.gradient), (Some("#000"), &None), "own fill blocks the style's gradient");
        assert_eq!((drawn.radius, drawn.radii), (Some(4.0), None), "own radius blocks radii");
        assert_eq!((drawn.shadow.is_some(), &drawn.shadows), (true, &None), "own shadow blocks shadows");
        assert_eq!((drawn.stroke.as_deref(), &drawn.stroke_gradient), (Some("#4C9DFF"), &None), "$ resolves in the node");
        assert_eq!((drawn.font_size, drawn.color.as_deref()), (Some(20.0), Some("#141414")), "$ resolves in the style");
        assert_eq!((drawn.stroke_width, drawn.opacity, drawn.stroke_cap), (Some(2.0), Some(0.9), Some(StrokeCap::Butt)));
        assert_eq!(drawn.stroke_join, None, "a rect's border turns its corners by its radius");
        assert_eq!(drawn.style.as_deref(), Some("card"));
        assert_eq!(own.stroke.as_deref(), Some("$accent"), "the stored node keeps its names");

        let bare = styles.resolve(&node(json!({"id": "b", "parent": "p", "kind": "rect", "style": "card"})));
        let gradient = bare.gradient.unwrap();
        assert_eq!((gradient.stops[0].color.as_str(), gradient.stops[1].color.as_str()), ("#4C9DFF", "#FFFDF8"));
        assert_eq!(bare.radii, Some([16.0, 16.0, 4.0, 4.0]));
        let shadows = bare.shadows.unwrap();
        assert_eq!((shadows[0].color.as_str(), shadows[1].color.as_str()), ("#141414", "#0000001F"));
        assert_eq!(bare.stroke_gradient.unwrap().stops[0].color, "#141414");

        // A run's colour resolves; a text takes no stroke, a box's join or a gradient.
        let text = styles.resolve(&node(json!({"id": "t", "parent": "p", "kind": "text", "style": "card", "text": "Hello",
            "runs": [{"start": 0, "end": 1, "color": "$accent"}]})));
        assert_eq!(text.runs.unwrap()[0].color.as_deref(), Some("#4C9DFF"));
        assert_eq!((text.stroke_width, &text.gradient, text.stroke_join), (None, &None, None));
        assert_eq!((text.font_size, text.shadows.map(|s| s.len())), (Some(20.0), Some(2)));

        // What the design no longer names is taken out, so the kind's default shows.
        let lost = styles.resolve(&node(json!({"id": "l", "parent": "p", "kind": "rect", "fill": "$gone", "style": "nope"})));
        assert_eq!((lost.fill, lost.style, lost.font_size), (None, None, None));
    }

    /// MODEL-SPEC §1 #23: a style lays, and a detach writes, only what the node's kind uses.
    #[test]
    fn a_style_radius_never_lands_on_an_ellipse() {
        let style = Style {
            radius: Some(12.0),
            radii: Some([1.0, 2.0, 3.0, 4.0]),
            fill: Some("#FFFFFF".into()),
            stroke_join: Some(StrokeJoin::Bevel),
            font: Some("Sora".into()),
            ..Style::default()
        };
        let stored = node(json!({"id": "e", "parent": "p", "kind": "ellipse", "style": "card"}));
        let mut ellipse = stored.clone();
        style.lay_under(&mut ellipse);
        assert_eq!((ellipse.radius, ellipse.radii), (None, None));
        assert_eq!((ellipse.fill.as_deref(), ellipse.stroke_join), (Some("#FFFFFF"), Some(StrokeJoin::Bevel)));
        let mut rect = node(json!({"parent": "p", "kind": "rect", "style": "card"}));
        style.lay_under(&mut rect);
        assert_eq!((rect.radius, rect.stroke_join), (Some(12.0), None));
        let mut image = node(json!({"parent": "p", "kind": "image", "src": "0123456789abcdef.png", "style": "card"}));
        style.lay_under(&mut image);
        assert_eq!((image.radius, image.fill.as_deref(), image.font.as_deref()), (Some(12.0), None, None));

        let detach = Styles::detach(&stored, "card", &Removed::Style(style.clone())).unwrap();
        assert_eq!(Value::Object(detach.clone()), json!({"style": null, "fill": "#FFFFFF", "stroke_join": "bevel", "font": "Sora"}));
        let kept = stored.patched(&detach).unwrap();
        assert_eq!(Node::check_update(&stored, &detach, &kept), Ok(()), "a strict list would take it");
        assert_eq!(Styles::detach(&stored, "other", &Removed::Style(style)), None);
    }

    /// MODEL-SPEC §2.3: a field this build cannot read is still the node's own, and its style does
    /// not paint over it.
    #[test]
    fn a_key_held_in_extra_counts_as_set() {
        let mut n = node(json!({"parent": "p", "kind": "rect", "style": "card", "font_size": "large",
            "gradient": {"kind": "spiral", "stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "#fff"}]}}));
        assert!(n.extra.contains_key("gradient") && n.extra.contains_key("font_size"));
        let style = Style { fill: Some("#111111".into()), font_size: Some(14.0), opacity: Some(0.5), ..Style::default() };
        let stored = n.clone();
        style.lay_under(&mut n);
        assert_eq!((n.fill, n.font_size, n.opacity), (None, None, Some(0.5)), "fill pairs with the unread gradient");
        let detach = Styles::detach(&stored, "card", &Removed::Style(style)).unwrap();
        assert_eq!(Value::Object(detach), json!({"style": null, "opacity": 0.5}));
    }

    /// Reuse 8: a style edit and its undo round trip, and a refusal names the key.
    #[test]
    fn a_style_patch_round_trips() {
        let style = Style::default().patched(&set(json!({"fill": "#fff", "radius": 8, "shadow": {"blur": 4, "color": "#00000033"}}))).unwrap();
        let change = set(json!({"fill": null, "radius": 12, "font_size": 18}));
        let after = style.patched(&change).unwrap();
        assert_eq!((after.fill.as_deref(), after.radius, after.font_size), (None, Some(12.0), Some(18.0)));
        assert_eq!(Value::Object(style.before(&change)), json!({"fill": "#fff", "radius": 8.0, "font_size": null}));
        assert_eq!(after.patched(&style.before(&change)).unwrap(), style);
        assert_eq!(Style::default().patched(&after.to_set()).unwrap(), after, "to_set makes it again");
        assert_eq!(style.patched(&Map::new()).unwrap(), style);

        let refuse = |change: Value| style.patched(&set(change)).unwrap_err();
        assert!(refuse(json!({"w": 1})).starts_with("w: not a style field; set it on each node (a style holds fill, gradient,"));
        assert_eq!(refuse(json!({"radus": 1})), "radus: not a style field; did you mean \"radius\"?");
        assert_eq!(refuse(json!({"background": "#fff"})), "background: not a style field; use fill");
        assert!(refuse(json!({"font_size": "big"})).starts_with("font_size: invalid type: string \"big\""));
        assert_eq!(refuse(json!({"font_size": 2000})), "font_size: 2000 is outside 1 to 1000");
        assert_eq!(refuse(json!({"font_weight": 70000})), "font_weight: 70000 is outside 1 to 1000");
        assert!(refuse(json!({"blend": "multiplyy"})).ends_with("did you mean \"multiply\"?"));
        assert!(refuse(json!({"shadow": [0, 4, 8, 0, "#000"]})).starts_with("shadow: invalid type: sequence"));
        assert!(refuse(json!({"radius": 2, "gradient": {"stops": []}})).starts_with("gradient: give 2 to 8 stops"));
    }

    /// A style holds what a caller may write in a node's field of the same name.
    #[test]
    fn a_style_is_held_to_what_a_node_may_be_written() {
        let refuse = |change: Value| Style::default().patched(&set(change)).unwrap_err();
        assert_eq!(refuse(json!({"font": "Geist\u{0}"})), "font: no control characters");
        assert_eq!(refuse(json!({"font": ""})), "font: a family name of 1 to 200 characters");
        assert_eq!(refuse(json!({"shadow": {"color": "#000"}, "shadows": [{"color": "#000"}]})), BOTH_SHADOWS);
        assert_eq!(
            refuse(json!({"shadows": [{"color": "#000"}, {"color": "red"}]})),
            "shadows[1].color: \"red\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour"
        );
        assert_eq!(refuse(json!({"shadows": [{"blur": 300, "color": "#000"}]})), "shadows[0].blur: 300 is outside 0 to 200");
        assert_eq!(refuse(json!({"shadow": {"blur": 201, "color": "#000"}})), "shadow.blur: 201 is outside 0 to 200");
        assert_eq!(refuse(json!({"shadows": []})), "shadows: 1 to 8 of them; null for none");
        assert!(refuse(json!({"dash": [0, 4], "stroke_cap": "butt"})).starts_with("stroke_cap: butt ends draw nothing"));
        assert_eq!(refuse(json!({"fill": "accent"})), "fill: \"accent\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour");
        assert_eq!(refuse(json!({"stroke_width": -1})), "stroke_width: -1 is outside 0 to 1000");

        // An inner shadow with spread is a box's look; a text it lies under casts outer ones only,
        // as paint reads them (§1 #13), so it is taken here and laid as written.
        let glow = Style::default().patched(&set(json!({"fill": "$accent", "shadows": [
            {"inset": true, "spread": 2, "blur": 8, "color": "#00000044"}, {"blur": 200, "color": "$ink"}]}))).unwrap();
        let mut text = node(json!({"parent": "p", "kind": "text", "text": "Hi"}));
        glow.lay_under(&mut text);
        assert_eq!(text.shadows.as_ref().map(Vec::len), Some(2));
        assert_eq!(glow.names(), [("fill", "accent"), ("shadows", "ink")]);
    }

    /// Reuse 8: colours and styles are set, changed and deleted, each answering its undo, and the
    /// undo puts back exactly what was there (and its own undo the change again).
    #[test]
    fn styles_set_delete_and_undo() {
        let mut styles = Styles::default();
        assert_eq!(styles.set_color("accent", "#4C9DFF"), Ok(NodeOp::DeleteStyle { name: "accent".into() }));
        assert_eq!(styles.set_color("accent", "#FF0000"), Ok(NodeOp::Color { name: "accent".into(), value: "#4C9DFF".into() }));
        styles.set_color("ink", "#141414").unwrap();
        assert_eq!(styles.set_style("card", &set(json!({"fill": "$accent", "radius": 12}))), Ok(NodeOp::DeleteStyle { name: "card".into() }));
        assert_eq!(
            styles.set_style("card", &set(json!({"radius": 16, "font_size": 14}))),
            Ok(NodeOp::Style { name: "card".into(), set: set(json!({"radius": 12.0, "font_size": null})) })
        );
        assert_eq!(styles.styles["card"], Style { fill: Some("$accent".into()), radius: Some(16.0), font_size: Some(14.0), ..Style::default() });

        // Refusals change nothing.
        let held = styles.clone();
        let refused = |result: Result<NodeOp, StyleError>| {
            let error = result.unwrap_err();
            (error.code, error.why)
        };
        let why = |code: &'static str, why: &str| (code, why.to_string());
        assert_eq!(refused(styles.set_color("card", "#fff")), why("style_exists", "name: \"card\" is a style; give the colour another name"));
        assert_eq!(refused(styles.set_style("ink", &Map::new())), why("style_exists", "name: \"ink\" is a colour; give the style another name"));
        assert_eq!(
            refused(styles.set_color("bad name", "#fff")),
            why("style_invalid", "name: \"bad name\" is not a colour name: use 1 to 64 letters, digits, '-', '_', '.' or ':'")
        );
        assert_eq!(refused(styles.set_color("$ink", "#fff")), why("style_invalid", "name: give the colour's name without its $ (\"ink\")"));
        assert_eq!(
            refused(styles.set_color("ink", "$accent")),
            why("style_invalid", "value: a colour's value is #RGB, #RRGGBB or #RRGGBBAA, not another $name")
        );
        assert_eq!(refused(styles.set_color("ink", "black")), why("style_invalid", "value: \"black\" is not #RGB, #RRGGBB or #RRGGBBAA"));
        assert_eq!(refused(styles.set_style("title", &set(json!({"font_size": 0})))), why("style_invalid", "font_size: 0 is outside 1 to 1000"));
        assert_eq!(refused(styles.set_style("title", &set(json!({"color": "$inc"})))), why("style_not_found", "color: the design has no colour $inc; did you mean $ink?"));
        assert_eq!(
            refused(styles.set_style("card", &set(json!({"shadow": {"color": "$card"}})))),
            why("style_not_found", "shadow: $card is a style, not a colour; a node takes a style with style: \"card\"")
        );
        assert_eq!(styles, held);

        // At the cap a new name is refused, a change is not.
        let mut full = Styles::default();
        for i in 0..MAX_STYLES {
            full.set_color(&format!("c{i}"), "#000").unwrap();
        }
        assert_eq!(full.set_color("one-more", "#000").unwrap_err().code, "too_large");
        assert_eq!(full.set_style("one-more", &Map::new()).unwrap_err().code, "too_large");
        assert!(full.set_color("c0", "#fff").is_ok());

        // Deleting a colour writes it into the styles naming it; the undo re-creates it first.
        let (removed, undo) = styles.delete("accent").unwrap();
        assert_eq!(removed, Removed::Color("#FF0000".into()));
        assert_eq!(styles.styles["card"].fill.as_deref(), Some("#FF0000"));
        assert_eq!(undo, [
            NodeOp::Color { name: "accent".into(), value: "#FF0000".into() },
            NodeOp::Style { name: "card".into(), set: set(json!({"fill": "$accent"})) },
        ]);
        let deleted = styles.clone();
        let redo = replay(&mut styles, &undo);
        assert_eq!(styles, held);
        replay(&mut styles, &redo);
        assert_eq!(styles, deleted, "the undo's undo deletes it again");
        replay(&mut styles, &undo);

        // Deleting a style answers it whole, and the undo makes it again.
        let (removed, undo) = styles.delete("card").unwrap();
        assert_eq!(removed, Removed::Style(held.styles["card"].clone()));
        assert_eq!(undo, [NodeOp::Style { name: "card".into(), set: held.styles["card"].to_set() }]);
        replay(&mut styles, &undo);
        assert_eq!(styles, held);

        let missing = styles.delete("acent").unwrap_err();
        assert_eq!((missing.code, missing.why.as_str()), ("style_not_found", "name: the design has no colour or style \"acent\"; did you mean \"accent\"?"));
        assert_eq!(styles, held);
    }

    /// Reuse 8: once a colour or style is deleted, each node it detaches draws as it did, and names
    /// nothing missing.
    #[test]
    fn detach_keeps_the_look() {
        let styles = palette();
        let nodes = [
            json!({"id": "card-1", "parent": "p", "kind": "rect", "style": "card", "fill": "$paper", "text": "Plan"}),
            json!({"id": "ring", "parent": "p", "kind": "ellipse", "style": "card", "arc": {"sweep": 270, "inner": 0.8}}),
            json!({"id": "head", "parent": "p", "kind": "text", "style": "title", "text": "Hello there",
                "runs": [{"start": 0, "end": 5, "color": "$accent"}, {"start": 6, "end": 11, "color": "$ink", "italic": true}]}),
            json!({"id": "link", "parent": "p", "kind": "line", "points": [[0, 0], [80, 0]], "stroke": "$accent",
                "stroke_gradient": {"stops": [{"at": 0, "color": "$ink"}, {"at": 1, "color": "#FFFFFF"}]}, "style": "card"}),
            json!({"id": "panel", "parent": "p", "kind": "frame", "style": "card", "w": 320, "h": 200,
                "shadows": [{"color": "$accent", "blur": 24}, {"color": "#000", "y": 1}]}),
            json!({"id": "note", "parent": "p", "kind": "sticky", "style": "title", "color": "$ink", "text": "Later"}),
            json!({"id": "star", "parent": "p", "kind": "icon", "icon": "star", "stroke": "$accent", "style": "card",
                "shadow": {"blur": 6, "color": "$ink"}}),
            json!({"id": "shot", "parent": "p", "kind": "image", "src": "0123456789abcdef.png", "w": 40, "h": 40, "style": "card"}),
            json!({"id": "plain", "parent": "p", "kind": "rect", "fill": "#123456"}),
        ]
        .map(node);
        // How a node looks: as it draws, less the style it names (a name, not a look).
        let look = |styles: &Styles, n: &Node| Node { style: None, ..styles.resolve(n) };
        for name in ["card", "title", "accent", "ink", "paper"] {
            let mut after = styles.clone();
            let (removed, _) = after.delete(name).unwrap();
            let mut detached = 0;
            for n in &nodes {
                let kept = match Styles::detach(n, name, &removed) {
                    Some(set) => {
                        detached += 1;
                        n.patched(&set).unwrap_or_else(|why| panic!("{name} from {}: {why}", n.id))
                    }
                    None => n.clone(),
                };
                assert_eq!(look(&after, &kept), look(&styles, n), "{name} from {}", n.id);
                assert_eq!(after.check(&kept), Ok(()), "{name} from {}", n.id);
                assert!(!kept.names().iter().any(|(_, used)| *used == name), "{name} from {}", n.id);
            }
            assert!(detached > 0, "{name} detached something");
        }
    }

    /// A colour's detach writes each field naming it whole, and only those.
    #[test]
    fn a_colour_detach_writes_whole_fields() {
        let n = node(json!({"parent": "p", "kind": "rect", "fill": "$b",
            "shadows": [{"blur": 4, "color": "$a"}, {"color": "#000"}],
            "gradient": {"stops": [{"at": 0, "color": "$a"}, {"at": 1, "color": "#fff"}]}}));
        let detach = Styles::detach(&n, "a", &Removed::Color("#123456".into())).unwrap();
        let keys: Vec<&str> = detach.keys().map(String::as_str).collect();
        assert_eq!(keys, ["shadows", "gradient"]);
        let kept = n.patched(&detach).unwrap();
        assert_eq!(kept.shadows.as_ref().unwrap()[0].color, "#123456");
        assert_eq!(kept.shadows.as_ref().unwrap()[1].color, "#000");
        assert_eq!(kept.gradient.as_ref().unwrap().stops[0].color, "#123456");
        assert_eq!(kept.fill.as_deref(), Some("$b"));
        assert_eq!(Styles::detach(&n, "c", &Removed::Color("#000".into())), None);

        // A stored field the kind does not use is rewritten too: left naming the colour, it would
        // name nothing, and every later update of the node would be refused.
        let stale = node(json!({"parent": "p", "kind": "image", "src": "0123456789abcdef.png", "w": 4, "h": 4, "fill": "$a"}));
        let detach = Styles::detach(&stale, "a", &Removed::Color("#123456".into())).unwrap();
        assert_eq!(Value::Object(detach), json!({"fill": "#123456"}));
    }

    /// Bus-styles: a lenient re-add loses only what names something missing.
    #[test]
    fn forget_missing_keeps_the_other_shadows_and_stops() {
        let styles = Styles { colors: BTreeMap::from([("a".into(), "#111111".into())]), styles: BTreeMap::new() };
        let mut text = node(json!({"parent": "p", "kind": "text", "text": "Hello there", "fill": "$gone", "color": "$a",
            "shadows": [{"color": "$a"}, {"color": "$gone"}, {"color": "#000"}],
            "runs": [{"start": 0, "end": 2, "color": "$gone"}, {"start": 3, "end": 5, "color": "$gone", "italic": true},
                     {"start": 6, "end": 8, "color": "$a"}],
            "style": "missing"}));
        assert_eq!(styles.forget_missing(&mut text), ["fill", "shadows", "runs", "style"]);
        assert_eq!((text.fill.as_deref(), text.color.as_deref(), text.style.as_deref()), (None, Some("$a"), None));
        let colors: Vec<&str> = text.shadows.iter().flatten().map(|s| s.color.as_str()).collect();
        assert_eq!(colors, ["$a", "#000"]);
        let runs = text.runs.clone().unwrap();
        assert_eq!(runs.iter().map(|r| (r.start, r.color.as_deref(), r.italic)).collect::<Vec<_>>(), [(3, None, Some(true)), (6, Some("$a"), None)]);
        assert_eq!((styles.check(&text), text.validate()), (Ok(()), Ok(())));

        let mut rect = node(json!({"parent": "p", "kind": "rect", "shadow": {"color": "$gone"},
            "gradient": {"stops": [{"at": 0, "color": "$gone"}, {"at": 0.5, "color": "$a"}, {"at": 1, "color": "#fff"}]},
            "stroke_gradient": {"stops": [{"at": 0, "color": "$gone"}, {"at": 1, "color": "#fff"}]},
            "runs": [{"start": 0, "end": 1, "color": "$gone"}], "text": "x"}));
        assert_eq!(styles.forget_missing(&mut rect), ["shadow", "gradient", "stroke_gradient", "runs"]);
        assert_eq!((&rect.shadow, &rect.stroke_gradient, &rect.runs), (&None, &None, &None), "a run left with no style goes, and the list with it");
        let stops: Vec<&str> = rect.gradient.iter().flat_map(|g| &g.stops).map(|s| s.color.as_str()).collect();
        assert_eq!(stops, ["$a", "#fff"]);
        assert_eq!((styles.check(&rect), rect.validate()), (Ok(()), Ok(())));

        let mut fine = node(json!({"parent": "p", "kind": "rect", "fill": "$a"}));
        let was = fine.clone();
        assert!(styles.forget_missing(&mut fine).is_empty());
        assert_eq!(fine, was);
    }

    /// A refusal names the field and the name, and the one meant when one is close.
    #[test]
    fn check_names_what_is_missing() {
        let styles = palette();
        let why = |body: Value| {
            let mut body = body.as_object().cloned().unwrap();
            body.extend([("parent".to_string(), json!("p"))]);
            let error = styles.check(&node(Value::Object(body))).unwrap_err();
            assert_eq!(error.code, "style_not_found");
            error.why
        };
        assert_eq!(why(json!({"kind": "rect", "fill": "$acent"})), "fill: the design has no colour $acent; did you mean $accent?");
        assert_eq!(why(json!({"kind": "rect", "style": "cards"})), "style: the design has no style \"cards\"; did you mean \"card\"?");
        assert_eq!(why(json!({"kind": "rect", "style": "accent"})), "style: \"accent\" is a colour, not a style; a colour field takes it as \"$accent\"");
        assert_eq!(why(json!({"kind": "rect", "stroke": "$card"})), "stroke: $card is a style, not a colour; a node takes a style with style: \"card\"");
        assert_eq!(
            why(json!({"kind": "text", "text": "abc", "runs": [{"start": 0, "end": 1, "color": "$zzz"}]})),
            "runs: the design has no colour $zzz; name it first with {\"op\": \"color\", \"name\": \"zzz\", \"value\": \"#RRGGBB\"}"
        );
        assert_eq!(
            why(json!({"kind": "rect", "style": "hero"})),
            "style: the design has no style \"hero\"; make it first with {\"op\": \"style\", \"name\": \"hero\", \"set\": {…}}"
        );
        assert_eq!(styles.check(&node(json!({"kind": "rect", "parent": "p", "style": "card", "fill": "$ink"}))), Ok(()));
        assert_eq!(styles.check(&node(json!({"kind": "rect", "parent": "p"}))), Ok(()));
    }

    #[test]
    fn a_node_uses_a_name_in_any_colour_slot_or_its_style() {
        let uses = |body: Value| Styles::uses(&node(body));
        assert!(!uses(json!({"kind": "rect", "parent": "p", "fill": "#fff", "shadow": {"color": "#000"}})));
        for named in [
            json!({"kind": "rect", "parent": "p", "style": "card"}),
            json!({"kind": "rect", "parent": "p", "stroke": "$a"}),
            json!({"kind": "rect", "parent": "p", "shadows": [{"color": "#000"}, {"color": "$a"}]}),
            json!({"kind": "rect", "parent": "p", "stroke_gradient": {"stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "$a"}]}}),
            json!({"kind": "text", "parent": "p", "text": "ab", "runs": [{"start": 0, "end": 1, "color": "$a"}]}),
        ] {
            assert!(uses(named.clone()), "{named}");
        }
    }

    /// Butt ends draw nothing of a dot: a style never lays one half of that under the other.
    #[test]
    fn a_style_never_makes_a_dotted_butt_stroke() {
        let line = |extra: Value| {
            let mut body = json!({"parent": "p", "kind": "line", "points": [[0, 0], [10, 0]], "style": "s"});
            body.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap());
            node(body)
        };
        let dots = Style { dash: Some(vec![0.0, 4.0]), stroke_width: Some(3.0), ..Style::default() };
        let butt = Style { stroke_cap: Some(StrokeCap::Butt), stroke_width: Some(3.0), ..Style::default() };
        let own_butt = line(json!({"stroke_cap": "butt"}));
        let mut drawn = own_butt.clone();
        dots.lay_under(&mut drawn);
        assert_eq!((&drawn.dash, drawn.stroke_width), (&None, Some(3.0)));
        assert_eq!(drawn.validate(), Ok(()));
        let own_dots = line(json!({"dash": [0, 4]}));
        let mut drawn = own_dots.clone();
        butt.lay_under(&mut drawn);
        assert_eq!((drawn.stroke_cap, drawn.validate()), (None, Ok(())));
        let mut dashes = line(json!({"dash": [6, 4]}));
        butt.lay_under(&mut dashes);
        assert_eq!(dashes.stroke_cap, Some(StrokeCap::Butt), "a dash's ends take it");
        // So a detach writes what was laid, which the node takes.
        let detach = Styles::detach(&own_butt, "s", &Removed::Style(dots)).unwrap();
        assert_eq!(Value::Object(detach.clone()), json!({"style": null, "stroke_width": 3.0}));
        assert!(own_butt.patched(&detach).is_ok());
    }
}
