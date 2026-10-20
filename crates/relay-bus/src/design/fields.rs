//! Which fields each kind of node uses (B8), and the rules on what a caller writes
//! (MODEL-SPEC §2.7-§2.8).
//!
//! The table is read off what paint, layout and hit testing use. A strict list (every list but
//! a lenient one) is held to it on what it writes: [`Node::check_new`] for an add,
//! [`Node::check_update`] for an update. A node as stored is never held to it, so a stale field
//! an earlier build accepted stays editable and an undo (a lenient list) puts back exactly what
//! was there. The canvas fits what it writes with [`fit_written`] first, so it never sends what
//! a strict list refuses.

use super::hints::{bad_value, not_a_field};
use super::read::probe;
use super::{canonical_key, Node, NodeKind, NodeOp, Shadow, Sizing, MAX_SHADOW_BLUR};
use serde::ser::{self, Impossible, Serialize, SerializeStruct, Serializer};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A set of node kinds: bit `NodeKind as u16` for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Kinds(u16);

const P: u16 = 1 << NodeKind::Page as u16;
const F: u16 = 1 << NodeKind::Frame as u16;
const G: u16 = 1 << NodeKind::Group as u16;
const R: u16 = 1 << NodeKind::Rect as u16;
const E: u16 = 1 << NodeKind::Ellipse as u16;
const L: u16 = 1 << NodeKind::Line as u16;
const T: u16 = 1 << NodeKind::Text as u16;
const S: u16 = 1 << NodeKind::Sticky as u16;
const A: u16 = 1 << NodeKind::Path as u16;
const I: u16 = 1 << NodeKind::Image as u16;
const Y: u16 = 1 << NodeKind::Polygon as u16;
const C: u16 = 1 << NodeKind::Icon as u16;
const ALL: u16 = P | F | G | R | E | L | T | S | A | I | Y | C;

impl Kinds {
    pub const ALL: Kinds = Kinds(ALL);

    pub fn has(self, kind: NodeKind) -> bool {
        self.0 & (1 << kind as u16) != 0
    }

    pub fn iter(self) -> impl Iterator<Item = NodeKind> {
        NodeKind::ALL.into_iter().filter(move |k| self.has(*k))
    }

    /// `frame, rect, sticky, image`.
    pub fn names(self) -> String {
        self.iter().map(NodeKind::name).collect::<Vec<_>>().join(", ")
    }
}

/// Every field of a node, the kinds that use it, and, where the usual sentence would mislead,
/// the refusal to give instead. A field added to `Node` gets a row here
/// (`every_field_has_a_kinds_row`).
pub const FIELDS: &[(&str, Kinds, Option<&str>)] = &[
    ("id", Kinds(ALL), None),
    ("parent", Kinds(ALL), None),
    ("kind", Kinds(ALL), None),
    ("name", Kinds(ALL), None),
    ("x", Kinds(ALL), None),
    ("y", Kinds(ALL), None),
    ("w", Kinds(ALL), None),
    ("h", Kinds(ALL), None),
    ("hidden", Kinds(ALL), None),
    ("locked", Kinds(ALL), None),
    ("skip_checks", Kinds(ALL & !P), Some("skip_checks: a page is not checked; set it on what is on it")),
    ("opacity", Kinds(ALL & !P), None),
    ("blur", Kinds(ALL & !P), None),
    ("blend", Kinds(ALL & !P), None),
    ("w_sizing", Kinds(ALL & !P), None),
    ("h_sizing", Kinds(ALL & !P), None),
    // B99: a line's turn is its points (its end handles are placed from them).
    ("rotation", Kinds(ALL & !P & !L), Some("rotation: a page does not turn, and a line turns by its points: move them instead")),
    ("fill", Kinds(P | F | R | E | Y | S | T | L | A), None),
    ("gradient", Kinds(F | R | E | Y | S | L | A), None),
    ("stroke", Kinds(F | R | E | Y | S | I | L | A | C), None),
    ("stroke_width", Kinds(F | R | E | Y | S | I | L | A | C), None),
    // B96: an icon draws its dash, in its 24-unit grid.
    ("dash", Kinds(F | R | E | Y | S | I | L | A | C), None),
    ("radius", Kinds(F | R | S | I), None),
    ("radii", Kinds(F | R | S | I), None),
    ("shadow", Kinds(F | R | E | Y | S | I | T | L | A | C), None),
    ("clip", Kinds(F), None),
    ("layout", Kinds(F), None),
    ("gap", Kinds(F), None),
    ("justify", Kinds(F), None),
    ("items", Kinds(F), None),
    ("padding", Kinds(F | R | E | Y), None),
    ("text", Kinds(T | S | R | E | Y), None),
    ("font", Kinds(T | S | R | E | Y), None),
    ("font_size", Kinds(T | S | R | E | Y), None),
    ("font_weight", Kinds(T | S | R | E | Y), None),
    ("italic", Kinds(T | S | R | E | Y), None),
    ("underline", Kinds(T | S | R | E | Y), None),
    ("strike", Kinds(T | S | R | E | Y), None),
    ("align", Kinds(T | S | R | E | Y), None),
    ("line_height", Kinds(T | S | R | E | Y), None),
    ("letter_spacing", Kinds(T | S | R | E | Y), None),
    ("text_case", Kinds(T | S | R | E | Y), None),
    ("color", Kinds(T | S | R | E | Y | C), None),
    ("points", Kinds(L | A), None),
    ("closed", Kinds(L | A), None),
    ("arrow_start", Kinds(L | A), None),
    ("arrow_end", Kinds(L | A), None),
    ("smooth", Kinds(L), None),
    ("from", Kinds(L), None),
    ("to", Kinds(L), None),
    ("route", Kinds(L), None),
    ("src", Kinds(I), None),
    ("fit", Kinds(I), None),
    ("sides", Kinds(Y), None),
    ("star", Kinds(Y), None),
    ("icon", Kinds(C), None),
    ("flip_x", Kinds(Y | I | C), None),
    ("flip_y", Kinds(Y | I | C), None),
    ("arc", Kinds(E), None),
    ("pin_x", Kinds(ALL & !P), Some("pin_x: a page sits in no parent")),
    ("pin_y", Kinds(ALL & !P), Some("pin_y: a page sits in no parent")),
    ("absolute", Kinds(ALL & !P), Some("absolute: a page sits in no parent")),
    // A group's size comes from its children, a line's or path's from its points.
    ("min_w", Kinds(F | R | E | Y | S | T | I | C), None),
    ("max_w", Kinds(F | R | E | Y | S | T | I | C), None),
    ("min_h", Kinds(F | R | E | Y | S | T | I | C), None),
    ("max_h", Kinds(F | R | E | Y | S | T | I | C), None),
    ("row_gap", Kinds(F), None),
    ("scroll", Kinds(F), Some("scroll: only a frame scrolls")),
    ("stroke_cap", Kinds(F | R | E | Y | S | I | L | A), None),
    // A box's border turns its corners by `radius`.
    ("stroke_join", Kinds(E | Y | L | A), None),
    ("stroke_gradient", Kinds(F | R | E | Y | S | I | L | A), None),
    ("shadows", Kinds(F | R | E | Y | S | I | T | L | A | C), None),
    ("runs", Kinds(T | S | R | E | Y), None),
    ("max_lines", Kinds(T | S | R | E | Y), None),
    ("style", Kinds(ALL & !P), Some("style: a page takes no style; give it its own fill")),
    ("motion", Kinds(ALL & !P), Some("motion: a page does not move; give motion to what is on it")),
    ("timeline", Kinds(F | P), Some("timeline: only a frame or a page keeps time")),
];

fn table() -> &'static HashMap<&'static str, (Kinds, Option<&'static str>)> {
    static TABLE: OnceLock<HashMap<&'static str, (Kinds, Option<&'static str>)>> = OnceLock::new();
    TABLE.get_or_init(|| FIELDS.iter().map(|&(field, kinds, custom)| (field, (kinds, custom))).collect())
}

/// The kinds that use `field` (`width`/`height` are `w`/`h`); `None` when no node has it.
pub fn field_kinds(field: &str) -> Option<Kinds> {
    table().get(canonical_key(field)).map(|row| row.0)
}

/// Whether a `kind` node uses `field`; false for a name that is no field.
pub fn takes(kind: NodeKind, field: &str) -> bool {
    field_kinds(field).is_some_and(|kinds| kinds.has(kind))
}

/// Why a `kind` node may not be written `field`; `None` when it takes it, or when no node has
/// it (the node then holds it in `extra`, which says why).
fn foreign(kind: NodeKind, field: &str) -> Option<String> {
    let (kinds, custom) = table().get(canonical_key(field))?;
    if kinds.has(kind) {
        return None;
    }
    Some(match custom {
        Some(text) => (*text).to_string(),
        None => format!("{field}: {} nodes do not use it (it is for {})", kind.name(), kinds.names()),
    })
}

/// Why a node holds `key` in `extra`: a name no field has (with the field meant, when the hints
/// know it or one is close), or a field whose value does not read (with what it takes).
fn unreadable(kind: NodeKind, key: &str, value: &Value) -> String {
    if field_kinds(key).is_none() {
        return not_a_field(key);
    }
    match probe(kind, key, value) {
        Err(why) => bad_value(key, value, &why),
        // It reads alone: it was set aside because the other spelling of the size came too.
        Ok(()) => {
            let field = canonical_key(key);
            format!("{field}: give {field} or {key}, not both")
        }
    }
}

/// C0 controls and DEL (B43). A NUL panics a debug build's GTK setters and cuts the string short
/// in release; the others draw as boxes.
fn is_control(c: char) -> bool {
    c < ' ' || c == '\u{7f}'
}

fn is_break(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
}

/// A name, font or title as GTK can show it: no control characters (B43).
pub fn plain_label(s: &str) -> bool {
    !s.chars().any(is_control)
}

/// `s` without its control characters; with `keep_breaks`, a text's tabs and line breaks stay.
pub fn strip_controls(s: &str, keep_breaks: bool) -> Cow<'_, str> {
    let bad = |c: char| is_control(c) && !(keep_breaks && is_break(c));
    if s.chars().any(bad) {
        Cow::Owned(s.chars().filter(|c| !bad(*c)).collect())
    } else {
        Cow::Borrowed(s)
    }
}

/// A text, line, path or icon casts its shadows round its letters or strokes: outer ones only,
/// without spread (MODEL-SPEC §1 #13).
pub(super) fn casts_outer_only(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Line | NodeKind::Path | NodeKind::Icon)
}

/// The written rules on one shadow (`field` is `shadow` or `shadows[2]`): its blur within the node
/// blur's cap (B72), and on a kind that casts outer shadows only, no inset and no spread.
fn written_shadow(kind: NodeKind, field: &str, s: &Shadow) -> Result<(), String> {
    if s.blur > MAX_SHADOW_BLUR {
        return Err(format!("{field}.blur: {} is outside 0 to {MAX_SHADOW_BLUR}", s.blur));
    }
    if casts_outer_only(kind) {
        if s.inset {
            return Err(format!("{field}.inset: {} nodes cast no inner shadow", kind.name()));
        }
        if s.spread != 0.0 {
            return Err(format!("{field}.spread: {} nodes cast their shadow without spread; widen it with blur", kind.name()));
        }
    }
    Ok(())
}

/// Whether a `kind` node sizes to its content when it hugs (B49): a text or sticky to its words,
/// a frame to its children. Layout reads hug on any other as fixed, so a rect told to hug kept a
/// 0 size and drew nothing.
fn hugs(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Sticky | NodeKind::Frame)
}

/// The written rules on one field's new value beyond its kind: no control characters in a name,
/// font, text or a run's font (B43), a shadow's blur within the node blur's cap (B72) and no inner
/// shadow or spread where only outer ones are cast, and hug only where something sizes to its
/// content (B49).
fn written(n: &Node, field: &str) -> Result<(), String> {
    match field {
        "w_sizing" | "h_sizing" => {
            let rule = if field == "w_sizing" { n.w_sizing } else { n.h_sizing };
            if rule == Some(Sizing::Hug) && !hugs(n.kind) {
                let kind = n.kind.name();
                let a = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
                return Err(format!("{field}: {a} {kind} cannot hug; only a text, a sticky or a frame sizes to its content"));
            }
            Ok(())
        }
        "name" | "font" => {
            let value = if field == "name" { &n.name } else { &n.font };
            match value {
                Some(s) if !plain_label(s) => Err(format!("{field}: no control characters")),
                _ => Ok(()),
            }
        }
        "text" => match &n.text {
            Some(t) if t.chars().any(|c| is_control(c) && !is_break(c)) => {
                Err("text: no control characters other than tab and newline".into())
            }
            _ => Ok(()),
        },
        "shadow" => n.shadow.as_ref().map_or(Ok(()), |s| written_shadow(n.kind, "shadow", s)),
        "shadows" => {
            for (i, s) in n.shadows.iter().flatten().enumerate() {
                written_shadow(n.kind, &format!("shadows[{i}]"), s)?;
            }
            Ok(())
        }
        "runs" => {
            for (i, run) in n.runs.iter().flatten().enumerate() {
                if run.font.as_deref().is_some_and(|f| !plain_label(f)) {
                    return Err(format!("runs[{i}].font: no control characters"));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The refusal of a node holding both `shadow` and `shadows`.
const BOTH_SHADOWS: &str = "shadows: give shadow or shadows, not both (shadows is the list)";

impl Node {
    /// An add as a caller writes it, in a strict list: nothing in `extra`, every field one its
    /// kind uses, and the written rules on each. The message starts with the field.
    pub fn check_new(&self) -> Result<(), String> {
        if let Some((key, value)) = self.extra.iter().next() {
            return Err(unreadable(self.kind, key, value));
        }
        for field in present(self) {
            if let Some(why) = foreign(self.kind, field) {
                return Err(why);
            }
            written(self, field)?;
        }
        if self.shadow.is_some() && self.shadows.is_some() {
            return Err(BOTH_SHADOWS.into());
        }
        // B49: an image added without its size draws nothing, unless layout gives it one.
        if self.kind == NodeKind::Image {
            for (key, size, rule) in [("w", self.w, self.w_rule()), ("h", self.h, self.h_rule())] {
                if size <= 0.0 && rule != Sizing::Fill {
                    return Err(format!("{key}: an image needs its size; design.asset.add answers w and h"));
                }
            }
        }
        Ok(())
    }

    /// An update as a caller writes it, in a strict list, given the node `before` and `after`
    /// `set`: each key with a value is a field `after`'s kind uses, that `after` could read, and
    /// that keeps the written rules. A `null` is always accepted for a known field, whatever the
    /// kind (it clears a stale value), and for a key the node holds in `extra` (it removes it);
    /// for any other name it is refused, since it would do nothing and is usually a typo.
    pub fn check_update(before: &Node, set: &Map<String, Value>, after: &Node) -> Result<(), String> {
        for (key, value) in set {
            let field = canonical_key(key);
            if value.is_null() {
                if field_kinds(field).is_none() && !before.extra.contains_key(field) {
                    return Err(not_a_field(key));
                }
                continue;
            }
            if let Some(held) = after.extra.get(field) {
                return Err(unreadable(after.kind, field, held));
            }
            if let Some(why) = foreign(after.kind, field) {
                return Err(why);
            }
            written(after, field)?;
            if matches!(field, "shadow" | "shadows") && after.shadow.is_some() && after.shadows.is_some() {
                return Err(BOTH_SHADOWS.into());
            }
        }
        // No stale range survives a rewrite of the words: the runs come with them.
        if set.contains_key("text") && !set.contains_key("runs") && before.runs.is_some() {
            return Err("runs: this text has runs; send runs (or runs: null) with its new text".into());
        }
        Ok(())
    }

    /// A copy for a fresh add (duplicate, paste): without `extra` or a field its kind does not
    /// use, so it passes [`Node::check_new`]'s kind rule. Nothing else is changed.
    pub fn without_foreign(&self) -> Node {
        let mut copy = self.clone();
        copy.extra.clear();
        if present(&copy).into_iter().all(|field| takes(copy.kind, field)) {
            return copy;
        }
        let mut fields = copy.to_map();
        fields.retain(|field, _| takes(copy.kind, field));
        Node::read(Value::Object(fields)).unwrap_or(copy)
    }
}

/// A shadow written to a `kind` node, fitted to the written rules: its blur within the cap, and
/// where only outer shadows are cast, no spread. False for an inner one there, which goes.
pub(super) fn fit_shadow(kind: NodeKind, s: &mut Shadow) -> bool {
    s.blur = s.blur.min(MAX_SHADOW_BLUR);
    if casts_outer_only(kind) {
        s.spread = 0.0;
        return !s.inset;
    }
    true
}

/// [`fit_shadow`] on a shadow as an update writes it (`outer_only` when the node's kind casts
/// outer shadows only).
fn fit_shadow_value(outer_only: bool, s: &mut Value) -> bool {
    if let Some(blur) = s.get_mut("blur").filter(|b| b.as_f64().is_some_and(|b| b > MAX_SHADOW_BLUR)) {
        *blur = Value::from(MAX_SHADOW_BLUR);
    }
    if outer_only {
        if let Some(spread) = s.get_mut("spread").filter(|v| v.as_f64() != Some(0.0)) {
            *spread = Value::from(0.0);
        }
        return s.get("inset") != Some(&Value::Bool(true));
    }
    true
}

/// Fit one edit the canvas is about to send to what a strict list takes (MODEL-SPEC §1 #33):
/// drop the keys its node's kind does not use (a `null` stays: it clears a stale value), drop
/// what the node holds in `extra` and keys no field has, take control characters out of a
/// name, font, text or a run's font, and fit shadows: a blur within its cap, and on a text,
/// line, path or icon no inner shadow (it goes) and no spread. A list of shadows replaces the
/// single one and the other way round (the other is cleared). `kind_of` names the kind of an
/// updated node (`None` when unknown: only unknown keys go). Returns the keys dropped.
pub fn fit_written(op: &mut NodeOp, kind_of: impl Fn(&str) -> Option<NodeKind>) -> Vec<String> {
    let mut dropped = Vec::new();
    match op {
        NodeOp::Add { node, .. } => {
            dropped.extend(node.extra.keys().cloned());
            let foreign: Vec<&str> = present(node).into_iter().filter(|field| !takes(node.kind, field)).collect();
            if !dropped.is_empty() || !foreign.is_empty() {
                dropped.extend(foreign.into_iter().map(str::to_string));
                *node = node.without_foreign();
            }
            for (value, keep_breaks) in [(&mut node.name, false), (&mut node.font, false), (&mut node.text, true)] {
                if let Some(s) = value.as_mut() {
                    if let Cow::Owned(clean) = strip_controls(s, keep_breaks) {
                        *s = clean;
                    }
                }
            }
            for run in node.runs.iter_mut().flatten() {
                if let Some(font) = run.font.as_mut() {
                    if let Cow::Owned(clean) = strip_controls(font, false) {
                        *font = clean;
                    }
                }
            }
            let kind = node.kind;
            for (field, rule) in [("w_sizing", &mut node.w_sizing), ("h_sizing", &mut node.h_sizing)] {
                if *rule == Some(Sizing::Hug) && !hugs(kind) {
                    *rule = None;
                    dropped.push(field.into());
                }
            }
            if node.shadow.as_mut().map(|s| fit_shadow(kind, s)) == Some(false) {
                node.shadow = None;
                dropped.push("shadow".into());
            }
            if node.shadows.as_mut().map(|list| { list.retain_mut(|s| fit_shadow(kind, s)); list.is_empty() }) == Some(true) {
                node.shadows = None;
                dropped.push("shadows".into());
            }
            // Both from two builds: `shadows` is what draws.
            if node.shadow.is_some() && node.shadows.is_some() {
                node.shadow = None;
                dropped.push("shadow".into());
            }
        }
        NodeOp::Update { id, set } => {
            let kind = kind_of(id);
            set.retain(|key, value| {
                let keep = field_kinds(key).is_some_and(|kinds| value.is_null() || kind.is_none_or(|k| kinds.has(k)));
                if !keep {
                    dropped.push(key.clone());
                }
                keep
            });
            for (field, keep_breaks) in [("name", false), ("font", false), ("text", true)] {
                if let Some(Value::String(s)) = set.get_mut(field) {
                    if let Cow::Owned(clean) = strip_controls(s, keep_breaks) {
                        *s = clean;
                    }
                }
            }
            if let Some(Value::Array(runs)) = set.get_mut("runs") {
                for font in runs.iter_mut().filter_map(|run| run.get_mut("font")) {
                    if let Value::String(f) = font {
                        if let Cow::Owned(clean) = strip_controls(f, false) {
                            *f = clean;
                        }
                    }
                }
            }
            for field in ["w_sizing", "h_sizing"] {
                if kind.is_some_and(|k| !hugs(k)) && set.get(field).and_then(Value::as_str) == Some("hug") {
                    set.remove(field);
                    dropped.push(field.into());
                }
            }
            let outer_only = kind.is_some_and(casts_outer_only);
            if set.get_mut("shadow").filter(|s| s.is_object()).map(|s| fit_shadow_value(outer_only, s)) == Some(false) {
                set.remove("shadow");
                dropped.push("shadow".into());
            }
            if let Some(Value::Array(list)) = set.get_mut("shadows") {
                list.retain_mut(|s| !s.is_object() || fit_shadow_value(outer_only, s));
                if list.is_empty() {
                    set.remove("shadows");
                    dropped.push("shadows".into());
                }
            }
            // One shadow replaces a list, and a list one shadow, so the node never holds both.
            if kind.is_some_and(|k| takes(k, "shadows")) {
                for (given, other) in [("shadow", "shadows"), ("shadows", "shadow")] {
                    if set.get(given).is_some_and(|v| !v.is_null()) && !set.contains_key(other) {
                        set.insert(other.into(), Value::Null);
                    }
                }
            }
        }
        // A copy writes what is stored, and colour and style edits name no node.
        NodeOp::Move { .. }
        | NodeOp::Delete { .. }
        | NodeOp::Copy { .. }
        | NodeOp::Color { .. }
        | NodeOp::Style { .. }
        | NodeOp::DeleteStyle { .. } => {}
    }
    dropped
}

/// The fields `node` holds (those its `Serialize` writes), read off without writing a value.
fn present(node: &Node) -> Vec<&'static str> {
    let mut fields = Vec::with_capacity(16);
    let _ = node.serialize(FieldNames(&mut fields));
    fields
}

/// A serializer that only records a struct's field names.
struct FieldNames<'a>(&'a mut Vec<&'static str>);

type NoValue = Impossible<(), serde::de::value::Error>;

fn not_a_struct<T>() -> Result<T, serde::de::value::Error> {
    Err(ser::Error::custom("not a node"))
}

macro_rules! not_a_struct {
    ($($method:ident($($arg:ty),*)),* $(,)?) => {
        $(fn $method(self, $(_: $arg),*) -> Result<(), Self::Error> { not_a_struct() })*
    };
}

impl Serializer for FieldNames<'_> {
    type Ok = ();
    type Error = serde::de::value::Error;
    type SerializeSeq = NoValue;
    type SerializeTuple = NoValue;
    type SerializeTupleStruct = NoValue;
    type SerializeTupleVariant = NoValue;
    type SerializeMap = NoValue;
    type SerializeStruct = Self;
    type SerializeStructVariant = NoValue;

    not_a_struct!(
        serialize_bool(bool), serialize_i8(i8), serialize_i16(i16), serialize_i32(i32), serialize_i64(i64),
        serialize_u8(u8), serialize_u16(u16), serialize_u32(u32), serialize_u64(u64), serialize_f32(f32),
        serialize_f64(f64), serialize_char(char), serialize_str(&str), serialize_bytes(&[u8]), serialize_none(),
        serialize_unit(), serialize_unit_struct(&'static str), serialize_unit_variant(&'static str, u32, &'static str),
    );

    fn serialize_some<V: ?Sized + Serialize>(self, _: &V) -> Result<(), Self::Error> {
        not_a_struct()
    }
    fn serialize_newtype_struct<V: ?Sized + Serialize>(self, _: &'static str, _: &V) -> Result<(), Self::Error> {
        not_a_struct()
    }
    fn serialize_newtype_variant<V: ?Sized + Serialize>(self, _: &'static str, _: u32, _: &'static str, _: &V) -> Result<(), Self::Error> {
        not_a_struct()
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<NoValue, Self::Error> {
        not_a_struct()
    }
    fn serialize_tuple(self, _: usize) -> Result<NoValue, Self::Error> {
        not_a_struct()
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<NoValue, Self::Error> {
        not_a_struct()
    }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<NoValue, Self::Error> {
        not_a_struct()
    }
    fn serialize_map(self, _: Option<usize>) -> Result<NoValue, Self::Error> {
        not_a_struct()
    }
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<NoValue, Self::Error> {
        not_a_struct()
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Self::Error> {
        Ok(self)
    }
}

impl SerializeStruct for FieldNames<'_> {
    type Ok = ();
    type Error = serde::de::value::Error;

    fn serialize_field<V: ?Sized + Serialize>(&mut self, key: &'static str, _: &V) -> Result<(), Self::Error> {
        self.0.push(key);
        Ok(())
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Shadow;
    use serde_json::json;

    fn node(body: Value) -> Node {
        let mut body = body;
        body["parent"] = json!("p");
        Node::read(body).unwrap()
    }

    fn set(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    /// B8: an add refuses a field its kind does not use, naming the field and the kinds that do.
    #[test]
    fn kinds_take_only_their_fields() {
        for (kind, field, value) in [
            ("ellipse", "radius", json!(4)),
            ("group", "layout", json!("row")),
            ("rect", "src", json!("0123456789abcdef0123.png")),
            ("text", "gradient", json!({"stops": [{"at": 0, "color": "#fff"}, {"at": 1, "color": "#000"}]})),
            ("group", "shadow", json!({"blur": 4, "color": "#000"})),
            ("group", "padding", json!([4])),
            ("rect", "flip_x", json!(true)),
            ("rect", "arc", json!({"sweep": 90})),
            ("rect", "sides", json!(5)),
            ("image", "fill", json!("#fff")),
            ("page", "opacity", json!(0.5)),
        ] {
            let why = node(json!({"kind": kind, field: value})).check_new().unwrap_err();
            let kinds = field_kinds(field).unwrap().names();
            assert_eq!(why, format!("{field}: {kind} nodes do not use it (it is for {kinds})"));
        }
        assert_eq!(node(json!({"kind": "ellipse", "radius": 4})).check_new().unwrap_err(),
            "radius: ellipse nodes do not use it (it is for frame, rect, sticky, image)");
        // B99: a line turns by its points; B96: an icon draws its dash.
        assert!(node(json!({"kind": "line", "points": [[0, 0], [9, 9]], "rotation": 30})).check_new().unwrap_err()
            .starts_with("rotation: a page does not turn, and a line turns by its points"));
        for ok in [
            json!({"kind": "text", "text": "Hi", "shadow": {"blur": 4, "color": "#000"}}),
            json!({"kind": "line", "points": [[0, 0], [9, 9]], "shadow": {"blur": 4, "color": "#000"}}),
            json!({"kind": "rect", "padding": [8, 24], "text": "Go"}),
            json!({"kind": "icon", "icon": "home", "color": "#fff", "dash": [2, 3]}),
            json!({"kind": "path", "points": [[0, 0]], "rotation": 30, "fill": "#fff"}),
            json!({"kind": "page", "fill": "#fff", "name": "Page 1"}),
        ] {
            assert_eq!(node(ok.clone()).check_new(), Ok(()), "{ok}");
        }
        // An update refuses the same, but a null clears any known field.
        let ellipse = node(json!({"kind": "ellipse"}));
        let radius = set(json!({"radius": 4}));
        let after = ellipse.patched(&radius).unwrap();
        assert!(Node::check_update(&ellipse, &radius, &after).unwrap_err().starts_with("radius: ellipse nodes"));
        let clear = set(json!({"radius": null, "src": null, "width": 20}));
        assert_eq!(Node::check_update(&ellipse, &clear, &ellipse.patched(&clear).unwrap()), Ok(()));
        // A null for a name no field has does nothing: refused as the typo it usually is.
        let typo = set(json!({"fil": null}));
        assert!(Node::check_update(&ellipse, &typo, &ellipse).unwrap_err().starts_with("fil: not a node field"));
    }

    /// A node as stored is never held to the written rules: legacy rows stay editable.
    #[test]
    fn stored_nodes_are_not_held_to_new_rules() {
        let group = node(json!({"kind": "group", "shadow": {"blur": 400, "color": "#000"}, "name": "a\u{1}b", "future": 1}));
        assert!(group.validate().is_ok());
        let x = set(json!({"x": 1}));
        let moved = group.patched(&x).unwrap();
        assert_eq!(Node::check_update(&group, &x, &moved), Ok(()));
        // A null for a key it holds in `extra` removes it.
        let gone = set(json!({"future": null}));
        let cleaned = group.patched(&gone).unwrap();
        assert!(cleaned.extra.is_empty());
        assert_eq!(Node::check_update(&group, &gone, &cleaned), Ok(()));
        // A copy for a fresh add drops what the kind does not use and what this build cannot read.
        let copy = group.without_foreign();
        assert_eq!((copy.shadow.clone(), copy.extra.len(), copy.name.as_deref()), (None, 0, Some("a\u{1}b")));
        assert_eq!(copy.without_foreign(), copy);
        let r = node(json!({"kind": "rect", "fill": "#fff", "w": 5}));
        assert_eq!(r.without_foreign(), r, "nothing to drop");
    }

    /// Every serialized field has a row and every row names a field: a field added to `Node`
    /// without one fails here (the start of W45's parity table).
    #[test]
    fn every_field_has_a_kinds_row() {
        let schema = serde_json::to_value(schemars::schema_for!(Node)).unwrap();
        // `width` and `height` are `w` and `h` by their other names.
        let mut fields: Vec<&str> = schema["properties"].as_object().unwrap().keys().map(String::as_str)
            .filter(|key| !matches!(*key, "width" | "height")).collect();
        let mut rows: Vec<&str> = FIELDS.iter().map(|row| row.0).collect();
        fields.sort_unstable();
        rows.sort_unstable();
        assert_eq!(rows, fields);
        // And a node with every field set serializes only keys with rows.
        let full: Node = serde_json::from_str(r##"{
            "id": "n", "parent": "p", "kind": "rect", "name": "n", "x": 1, "y": 1, "w": 1, "h": 1, "fill": "#fff",
            "stroke": "#000", "stroke_width": 1, "radius": 1, "opacity": 1, "shadow": {"color": "#000"}, "clip": true,
            "hidden": false, "locked": false, "skip_checks": true, "text": "t", "color": "#000", "font": "Geist", "font_size": 9,
            "font_weight": 400, "italic": true, "underline": true, "strike": true, "align": "left", "line_height": 1,
            "letter_spacing": 0, "w_sizing": "fixed", "h_sizing": "fixed", "layout": "row", "gap": 1, "padding": [1],
            "justify": "start", "items": "start", "points": [[0, 0]], "arrow_start": true, "arrow_end": true,
            "src": "0123456789abcdef0123.png", "fit": "cover", "rotation": 1, "sides": 3, "star": 0.5,
            "gradient": {"stops": []}, "radii": [1, 1, 1, 1], "dash": [1], "blur": 1, "icon": "home", "from": "a",
            "to": "b", "route": "straight", "closed": true, "smooth": true, "flip_x": true, "flip_y": true,
            "arc": {}, "blend": "normal", "text_case": "upper", "pin_x": "left", "pin_y": "top", "absolute": false,
            "min_w": 1, "max_w": 1, "min_h": 1, "max_h": 1, "row_gap": 1, "scroll": "x", "stroke_cap": "round",
            "stroke_join": "round", "stroke_gradient": {"stops": []}, "shadows": [], "runs": [], "max_lines": 1, "style": "card",
            "motion": {"keys": []}, "timeline": {}
        }"##).unwrap();
        let keys = present(&full);
        assert_eq!(keys.len(), FIELDS.len(), "{keys:?}");
        assert!(keys.iter().all(|k| field_kinds(k).is_some()));
    }

    /// A body of `kind` with what the kind needs to validate, and `field` set to `value`.
    fn body_of(kind: NodeKind, field: &str, value: Value) -> Value {
        let mut body = match kind {
            NodeKind::Page => json!({"kind": "page"}),
            NodeKind::Line => json!({"kind": "line", "parent": "p", "points": [[0, 0], [9, 9]]}),
            NodeKind::Path => json!({"kind": "path", "parent": "p", "points": [[0, 0]]}),
            NodeKind::Image => json!({"kind": "image", "parent": "p", "src": "0123456789abcdef0123.png", "w": 64, "h": 48}),
            NodeKind::Icon => json!({"kind": "icon", "parent": "p", "icon": "home"}),
            kind => json!({"kind": kind.name(), "parent": "p"}),
        };
        body[field] = value;
        if field == "runs" {
            body["text"] = json!("a".repeat(crate::design::MAX_RUNS));
        }
        body
    }

    /// What a strict list takes: it reads, validates and keeps the written rules.
    fn written(body: Value) -> bool {
        Node::read(body).is_ok_and(|n| n.validate().is_ok() && n.check_new().is_ok())
    }

    fn number(v: f64, integer: bool) -> Value {
        if integer { json!(v as i64) } else { json!(v) }
    }

    /// The bounds `property` states, each with a step past it: `[(value, accepted)]`.
    fn bounds(property: &Value) -> Vec<(f64, bool)> {
        let step = if property["type"].to_string().contains("integer") { 1.0 } else { 0.001 };
        let mut cases = Vec::new();
        if let Some(lo) = property["minimum"].as_f64() {
            cases.extend([(lo, true), (lo - step, false)]);
        }
        if let Some(hi) = property["maximum"].as_f64() {
            cases.extend([(hi, true), (hi + step, false)]);
        }
        cases
    }

    /// E-agent-3: every range, length and count the node schema states is the one the rules hold
    /// a written node to: each bound is accepted and a step past it refused. A field added with a
    /// schema range is checked here without a new line.
    #[test]
    fn the_schema_states_the_rules_ranges() {
        let schema = serde_json::to_value(schemars::schema_for!(Node)).unwrap();
        let mut checked = Vec::new();
        for (field, kinds, _) in FIELDS {
            let property = &schema["properties"][field];
            let kind = kinds.iter().find(|k| *k != NodeKind::Page).unwrap_or(NodeKind::Page);
            let integer = property["type"].to_string().contains("integer");
            for (v, ok) in bounds(property) {
                assert_eq!(written(body_of(kind, field, number(v, integer))), ok, "{field} = {v}");
                checked.push(*field);
            }
            if let Some(max) = property["maxLength"].as_u64() {
                assert!(written(body_of(kind, field, json!("a".repeat(max as usize)))), "{field}: {max} characters");
                assert!(!written(body_of(kind, field, json!("a".repeat(max as usize + 1)))), "{field}: {max} + 1 characters");
                checked.push(*field);
            }
            if property["minLength"].as_u64() == Some(1) {
                assert!(!written(body_of(kind, field, json!(""))), "{field}: empty");
            }
            if let Some(pattern) = property["pattern"].as_str() {
                assert_eq!(pattern, crate::design::ID_PATTERN);
                assert!(!written(body_of(kind, field, json!("a b"))), "{field}: a space");
            }
            // A list: its count, and the range of what it holds.
            let filler = |v: Value| match *field {
                "dash" => json!([v, 4]),
                "radii" => json!([v, 1, 1, 1]),
                "points" => json!([[v, 0], [0, 0]]),
                _ => json!([v]),
            };
            let items = if *field == "points" { &property["items"]["items"] } else { &property["items"] };
            for (v, ok) in bounds(items) {
                assert_eq!(written(body_of(kind, field, filler(json!(v)))), ok, "{field} holding {v}");
                checked.push(*field);
            }
            let count = |n: u64| Value::Array((0..n).map(|i| match *field {
                "points" => json!([1, 1]),
                "shadows" => json!({"color": "#000"}),
                "runs" => json!({"start": i, "end": i + 1, "italic": true}),
                _ => json!(1),
            }).collect());
            if let Some(min) = property["minItems"].as_u64() {
                assert!(written(body_of(kind, field, count(min))), "{field}: {min} of them");
                assert!(!written(body_of(kind, field, count(min - 1))), "{field}: {} of them", min - 1);
            }
            if let Some(max) = property["maxItems"].as_u64() {
                assert!(written(body_of(kind, field, count(max))), "{field}: {max} of them");
                assert!(!written(body_of(kind, field, count(max + 1))), "{field}: {} of them", max + 1);
                checked.push(*field);
            }
        }
        for field in ["x", "w", "stroke_width", "opacity", "font_size", "font_weight", "line_height", "letter_spacing", "gap", "rotation",
            "sides", "star", "blur", "radius", "name", "text", "font", "id", "padding", "dash", "radii", "points", "min_w", "max_h",
            "row_gap", "max_lines", "shadows", "runs"] {
            assert!(checked.contains(&field), "{field} states no range");
        }
        // The nested objects: a shadow, a gradient's stops, an arc.
        let defs = &schema["$defs"];
        type Wrap = fn(&str, Value) -> Value;
        let nested: [(&str, &str, Wrap); 4] = [
            ("DesignShadow", "shadow", |key, v| { let mut s = json!({"color": "#000"}); s[key] = v; s }),
            ("DesignGradientStop", "gradient", |key, v| { let mut s = json!({"at": 0, "color": "#000"}); s[key] = v; json!({"stops": [s, {"at": 1, "color": "#fff"}]}) }),
            ("DesignArc", "arc", |key, v| { let mut a = json!({}); a[key] = v; a }),
            ("DesignTextRun", "runs", |key, v| { let mut r = json!({"start": 0, "end": 1, "italic": true}); r[key] = v; json!([r]) }),
        ];
        for (def, field, wrap) in nested {
            let kind = field_kinds(field).unwrap().iter().next().unwrap();
            let mut seen = 0;
            for (key, property) in defs[def]["properties"].as_object().unwrap() {
                let integer = property["type"].to_string().contains("integer");
                for (v, ok) in bounds(property) {
                    assert_eq!(written(body_of(kind, field, wrap(key, number(v, integer)))), ok, "{field}.{key} = {v}");
                    seen += 1;
                }
            }
            assert!(seen >= 2, "{def} states no range");
        }
        let stops = &defs["DesignGradient"]["properties"]["stops"];
        let stop = |n: u64| json!({"stops": vec![json!({"at": 0.5, "color": "#000"}); n as usize]});
        for (n, ok) in [(stops["minItems"].as_u64().unwrap(), true), (stops["maxItems"].as_u64().unwrap(), true)] {
            assert_eq!(written(body_of(NodeKind::Rect, "gradient", stop(n))), ok);
            assert!(!written(body_of(NodeKind::Rect, "gradient", stop(if n == 2 { 1 } else { n + 1 }))), "{n} stops");
        }
        // The aliases state the size's own range.
        assert_eq!((&schema["properties"]["width"]["maximum"], &schema["properties"]["height"]["minimum"]),
            (&schema["properties"]["w"]["maximum"], &schema["properties"]["h"]["minimum"]));
    }

    /// E-agent-4: the icon list in the schema is the whole list that validates.
    #[test]
    fn the_schema_lists_every_icon() {
        let schema = serde_json::to_value(schemars::schema_for!(Node)).unwrap();
        let listed: Vec<&str> = schema["properties"]["icon"]["enum"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(listed, crate::design::ICONS);
        assert!(!schema.to_string().contains("[`ICONS`]"), "a rustdoc link leaked into the schema");
    }

    /// B43: control characters are refused in a name, font or text a caller writes (a text keeps
    /// its tabs and line breaks), and taken out of what the canvas sends.
    #[test]
    fn control_characters_are_refused() {
        for (field, value) in [("name", "a\u{0}b"), ("font", "Geist\u{1}"), ("text", "a\u{0}b"), ("text", "bell\u{7}"), ("name", "del\u{7f}")] {
            let why = node(json!({"kind": "text", "text": "x", field: value})).check_new().unwrap_err();
            assert!(why.starts_with(&format!("{field}: no control characters")), "{why}");
            let t = node(json!({"kind": "text", "text": "x"}));
            let s = set(json!({field: value}));
            assert!(Node::check_update(&t, &s, &t.patched(&s).unwrap()).unwrap_err().starts_with(field));
        }
        assert_eq!(node(json!({"kind": "text", "text": "one\ntwo\tthree\r\n", "name": "Title · 2"})).check_new(), Ok(()));
        assert!(plain_label("Héllo · wörld") && !plain_label("a\nb"));
        assert_eq!(strip_controls("a\u{0}b\tc\nd\u{7f}", true), "ab\tc\nd");
        assert_eq!(strip_controls("a\u{0}b\tc\nd", false), "abcd");
        assert!(matches!(strip_controls("clean", false), Cow::Borrowed(_)));
    }

    /// B72: a written shadow's blur stops at the node blur's 200; one already stored is not refused.
    #[test]
    fn shadow_blur_is_capped_like_blur() {
        for (blur, inset, ok) in [(201.0, false, false), (201.0, true, false), (200.0, false, true), (0.0, true, true)] {
            let mut n = node(json!({"kind": "ellipse"}));
            n.shadow = Some(Shadow { x: 0.0, y: 0.0, blur, spread: 5000.0, color: "#000".into(), inset });
            assert!(n.validate().is_ok());
            match n.check_new() {
                Ok(()) => assert!(ok, "{blur}"),
                Err(why) => assert!(!ok && why == format!("shadow.blur: {blur} is outside 0 to 200"), "{why}"),
            }
        }
        let e = node(json!({"kind": "ellipse"}));
        let s = set(json!({"shadow": {"blur": 300, "color": "#000"}}));
        assert!(Node::check_update(&e, &s, &e.patched(&s).unwrap()).unwrap_err().starts_with("shadow.blur"));
    }

    /// The written rules of the drawing fields (MODEL-SPEC §2.6, §2.8): a node holds one of
    /// `shadow` and `shadows`, a text, line, path or icon casts outer shadows without spread, a
    /// change of words carries its runs, and a run's font has no control characters.
    #[test]
    fn drawing_fields_keep_the_written_rules() {
        let both = node(json!({"kind": "rect", "shadow": {"color": "#000"}, "shadows": [{"color": "#000"}]}));
        assert_eq!(both.validate(), Ok(()), "a value rule would make a stored node unpatchable");
        assert_eq!(both.check_new().unwrap_err(), "shadows: give shadow or shadows, not both (shadows is the list)");
        for (kind, extra) in [("line", json!({"points": [[0, 0], [9, 9]]})), ("text", json!({"text": "Hi"})), ("path", json!({"points": [[0, 0]]})),
            ("icon", json!({"icon": "home"}))] {
            let mut body = extra.clone();
            body["kind"] = json!(kind);
            body["shadows"] = json!([{"blur": 8, "color": "#4AF0FF99"}, {"inset": true, "color": "#000"}]);
            assert_eq!(node(body.clone()).check_new().unwrap_err(), format!("shadows[1].inset: {kind} nodes cast no inner shadow"));
            body["shadows"] = Value::Null;
            body["shadow"] = json!({"spread": 2, "color": "#000"});
            assert_eq!(node(body.clone()).check_new().unwrap_err(), format!("shadow.spread: {kind} nodes cast their shadow without spread; widen it with blur"));
            body["shadow"] = json!({"blur": 16, "color": "#4AF0FF99"});
            assert_eq!(node(body).check_new(), Ok(()), "{kind}: a glow");
        }
        let inner = node(json!({"kind": "ellipse", "shadows": [{"inset": true, "spread": 3, "color": "#000"}, {"blur": 250, "color": "#000"}]}));
        assert_eq!(inner.check_new().unwrap_err(), "shadows[1].blur: 250 is outside 0 to 200");
        // An update setting one shadow field on a node holding the other is refused; clearing the
        // other in the same set is not.
        let one = node(json!({"kind": "rect", "shadow": {"color": "#000"}}));
        let list = set(json!({"shadows": [{"color": "#000"}, {"blur": 9, "color": "#fff"}]}));
        assert!(Node::check_update(&one, &list, &one.patched(&list).unwrap()).unwrap_err().starts_with("shadows: give shadow or shadows"));
        let swap = set(json!({"shadow": null, "shadows": [{"color": "#000"}, {"blur": 9, "color": "#fff"}]}));
        assert_eq!(Node::check_update(&one, &swap, &one.patched(&swap).unwrap()), Ok(()));
        // New words carry their runs.
        let rich = node(json!({"kind": "text", "text": "8.4 km", "runs": [{"start": 3, "end": 6, "font_size": 14}]}));
        let words = set(json!({"text": "12.1 km"}));
        assert_eq!(Node::check_update(&rich, &words, &rich.patched(&words).unwrap()).unwrap_err(),
            "runs: this text has runs; send runs (or runs: null) with its new text");
        for with in [json!({"text": "12.1 km", "runs": [{"start": 4, "end": 7, "font_size": 14}]}), json!({"text": "12.1 km", "runs": null})] {
            let with = set(with);
            assert_eq!(Node::check_update(&rich, &with, &rich.patched(&with).unwrap()), Ok(()));
        }
        let x = set(json!({"x": 4}));
        assert_eq!(Node::check_update(&rich, &x, &rich.patched(&x).unwrap()), Ok(()));
        let font = node(json!({"kind": "text", "text": "Hi", "runs": [{"start": 0, "end": 1, "font": "Ge\u{1}ist"}]}));
        assert_eq!(font.check_new().unwrap_err(), "runs[0].font: no control characters");
        // Kinds: runs and max_lines on text holders, stroke_join on shapes that turn corners.
        assert!(node(json!({"kind": "rect", "stroke_join": "miter"})).check_new().unwrap_err()
            .starts_with("stroke_join: rect nodes do not use it (it is for ellipse, line, path, polygon)"));
        assert!(node(json!({"kind": "group", "shadows": [{"color": "#000"}]})).check_new().unwrap_err().starts_with("shadows: group nodes"));
        assert!(node(json!({"kind": "icon", "icon": "home", "stroke_gradient": {"stops": []}})).check_new().unwrap_err().starts_with("stroke_gradient: icon nodes"));
        assert_eq!(node(json!({"kind": "polygon", "text": "Go", "max_lines": 1, "stroke_join": "bevel", "stroke_cap": "square"})).check_new(), Ok(()));
    }

    /// B49: hug only where something sizes to its content, and an image added with its size.
    #[test]
    fn hug_and_image_sizes_are_written_rules() {
        for kind in ["rect", "ellipse", "polygon", "group"] {
            for field in ["w_sizing", "h_sizing"] {
                let why = node(json!({"kind": kind, field: "hug"})).check_new().unwrap_err();
                let a = if kind == "ellipse" { "an" } else { "a" };
                assert_eq!(why, format!("{field}: {a} {kind} cannot hug; only a text, a sticky or a frame sizes to its content"));
            }
        }
        for ok in [json!({"kind": "text", "text": "Hi", "w_sizing": "hug"}), json!({"kind": "sticky", "h_sizing": "hug"}),
            json!({"kind": "frame", "layout": "row", "w_sizing": "hug", "h_sizing": "hug"}), json!({"kind": "rect", "w_sizing": "fill"})] {
            assert_eq!(node(ok.clone()).check_new(), Ok(()), "{ok}");
        }
        let rect = node(json!({"kind": "rect", "w": 10, "h": 10}));
        let hug = set(json!({"h_sizing": "hug"}));
        assert!(Node::check_update(&rect, &hug, &rect.patched(&hug).unwrap()).unwrap_err().starts_with("h_sizing: a rect cannot hug"));
        // A rect stored hugging stays editable.
        let stored = node(json!({"kind": "rect", "w_sizing": "hug"}));
        let x = set(json!({"x": 2}));
        assert_eq!(Node::check_update(&stored, &x, &stored.patched(&x).unwrap()), Ok(()));
        // An image needs its size, unless layout gives it one.
        let src = "0123456789abcdef0123.png";
        for (body, why) in [
            (json!({"kind": "image", "src": src}), Some("w: an image needs its size; design.asset.add answers w and h")),
            (json!({"kind": "image", "src": src, "w": 640}), Some("h: an image needs its size; design.asset.add answers w and h")),
            (json!({"kind": "image", "src": src, "w": 640, "h": 480}), None),
            (json!({"kind": "image", "src": src, "w_sizing": "fill", "h": 200}), None),
        ] {
            assert_eq!(node(body.clone()).check_new().err().as_deref(), why, "{body}");
        }
        // The canvas never sends a hug where it would be refused.
        let kinds = |id: &str| (id == "r").then_some(NodeKind::Rect);
        let mut op = NodeOp::Update { id: "r".into(), set: set(json!({"w_sizing": "hug", "h_sizing": "fill"})) };
        assert_eq!(fit_written(&mut op, kinds), ["w_sizing"]);
        let mut op = NodeOp::Add { node: node(json!({"id": "e", "kind": "ellipse", "w": 9, "h": 9, "h_sizing": "hug"})), index: None };
        assert_eq!(fit_written(&mut op, kinds), ["h_sizing"]);
        let NodeOp::Add { node: fitted, .. } = &op else { panic!() };
        assert_eq!((fitted.h_sizing, fitted.check_new()), (None, Ok(())));
    }

    /// What the canvas sends is fitted to what a strict list takes (MODEL-SPEC §1 #33).
    #[test]
    fn fit_written_sends_only_what_a_strict_list_takes() {
        let kinds = |id: &str| match id {
            "img" => Some(NodeKind::Image),
            "t" => Some(NodeKind::Text),
            _ => None,
        };
        // The context bar's fill on an image, one `set` for a mixed selection.
        let mut op = NodeOp::Update { id: "img".into(), set: set(json!({"fill": "#fff", "opacity": 0.5, "radius": null, "fil": 1})) };
        assert_eq!(fit_written(&mut op, kinds), ["fill", "fil"]);
        assert_eq!(op, NodeOp::Update { id: "img".into(), set: set(json!({"opacity": 0.5, "radius": null})) });
        // Text: control characters out, line breaks kept; a shadow's blur within its cap, and one
        // shadow replaces a list.
        let mut op = NodeOp::Update { id: "t".into(), set: set(json!({"text": "a\u{0}b\nc", "name": "x\ty", "shadow": {"blur": 900, "color": "#000"}})) };
        assert!(fit_written(&mut op, kinds).is_empty());
        let NodeOp::Update { set: fitted, .. } = &op else { panic!() };
        assert_eq!(Value::Object(fitted.clone()), json!({"text": "ab\nc", "name": "xy", "shadow": {"blur": 200.0, "color": "#000"}, "shadows": null}));
        // A text casts outer shadows only, without spread: an inner one goes, a spread is 0; a
        // run's font loses its control characters.
        let mut op = NodeOp::Update { id: "t".into(), set: set(json!({"shadow": {"inset": true, "color": "#000"},
            "shadows": [{"spread": 4, "blur": 300, "color": "#000"}, {"inset": true, "color": "#fff"}], "runs": [{"start": 0, "end": 1, "font": "Ge\u{0}ist"}]})) };
        assert_eq!(fit_written(&mut op, kinds), ["shadow"]);
        let NodeOp::Update { set: fitted, .. } = &op else { panic!() };
        assert_eq!(Value::Object(fitted.clone()), json!({"shadows": [{"spread": 0.0, "blur": 200.0, "color": "#000"}], "shadow": null,
            "runs": [{"start": 0, "end": 1, "font": "Geist"}]}));
        let mut op = NodeOp::Update { id: "t".into(), set: set(json!({"shadows": [{"inset": true, "color": "#000"}]})) };
        assert_eq!(fit_written(&mut op, kinds), ["shadows"]);
        assert_eq!(op, NodeOp::Update { id: "t".into(), set: Map::new() });
        // An image takes inner shadows, and an unknown kind keeps them.
        for id in ["img", "new"] {
            let mut op = NodeOp::Update { id: id.into(), set: set(json!({"shadow": {"inset": true, "spread": 2, "color": "#000"}})) };
            assert!(fit_written(&mut op, kinds).is_empty());
            let NodeOp::Update { set: fitted, .. } = &op else { panic!() };
            assert_eq!(fitted["shadow"], json!({"inset": true, "spread": 2, "color": "#000"}));
        }
        // An unknown target keeps every known field.
        let mut op = NodeOp::Update { id: "new".into(), set: set(json!({"fill": "#fff", "nope": 1})) };
        assert_eq!(fit_written(&mut op, kinds), ["nope"]);
        // An add: a legacy group duplicated as it was stored.
        let legacy = node(json!({"id": "g", "kind": "group", "shadow": {"blur": 4, "color": "#000"}, "name": "g\u{0}", "future": 1}));
        let mut op = NodeOp::Add { node: legacy, index: None };
        let mut dropped = fit_written(&mut op, kinds);
        dropped.sort();
        assert_eq!(dropped, ["future", "shadow"]);
        let NodeOp::Add { node: fitted, .. } = &op else { panic!() };
        assert_eq!((fitted.name.as_deref(), fitted.shadow.clone(), fitted.extra.len()), (Some("g"), None, 0));
        assert_eq!(fitted.check_new(), Ok(()));
        // An add of a line with an inner shadow and both shadow fields, as two builds wrote it.
        let line = node(json!({"id": "l", "kind": "line", "points": [[0, 0], [9, 0]], "shadow": {"inset": true, "color": "#000"},
            "shadows": [{"spread": 3, "color": "#000"}], "runs": null}));
        let mut op = NodeOp::Add { node: line, index: None };
        assert_eq!(fit_written(&mut op, kinds), ["shadow"]);
        let NodeOp::Add { node: fitted, .. } = &op else { panic!() };
        assert_eq!((fitted.shadow.clone(), fitted.shadows.as_ref().map(|s| s[0].spread)), (None, Some(0.0)));
        assert_eq!(fitted.check_new(), Ok(()));
        let both = node(json!({"id": "r", "kind": "rect", "shadow": {"color": "#000"}, "shadows": [{"color": "#fff"}]}));
        let mut op = NodeOp::Add { node: both, index: None };
        assert_eq!(fit_written(&mut op, kinds), ["shadow"]);
        // Moves and deletes are left alone.
        let mut op = NodeOp::Delete { id: "img".into() };
        assert!(fit_written(&mut op, kinds).is_empty());
    }
}
