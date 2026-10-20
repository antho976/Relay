//! Named errors for what agents write (W16, W19; MODEL-SPEC §2.8, §2.11). An edit list is read
//! one edit at a time, and one that does not read is told as `ops[i] <label>: <field>: <why>`,
//! with advice for what agents commonly mean (an edit, a kind, a field or a value from another
//! tool) or the closest name this build knows. The same advice names a node field a strict list
//! refuses (`fields`), and `update`'s `set` gets a schema of the field names.
//!
//! The hint tables name only what does not exist: a name that becomes an edit, a kind or a field
//! leaves its table (`hints_never_shadow_what_exists`).

use super::{field_kinds, Node, NodeKind, NodeOp, FIELDS, MAX_LENIENT_OPS, MAX_OPS};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::de::{self, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};
use std::fmt;
use std::sync::OnceLock;

/// Edits agents name that are not edits, and what to do instead.
pub const OP_HINTS: &[(&[&str], &str)] = &[
    (&["duplicate", "clone"], "use copy {id, as}"),
    (&["instance", "component", "symbol"], "components are not supported yet: copy the node and give the copies a style"),
    (&["set", "patch", "edit", "modify", "change"], "use update with set"),
    (&["remove", "destroy"], "use delete"),
    (&["reorder", "reparent", "nest"], "use move with parent and index"),
    (&["create", "insert", "append"], "use add"),
    (&["group", "ungroup"], "add a group node and move nodes into it (ungroup: move them out, then delete the group)"),
    (&["token", "palette", "variable", "colour", "name_color"], "use color {name, value}"),
    (&["class", "theme"], "use style {name, set}"),
    (&["remove_style", "delete_color", "remove_color"], "use delete_style {name}"),
];

/// Kinds agents name that are not kinds, and the kind to use.
pub const KIND_HINTS: &[(&[&str], &str)] = &[
    (
        &["button", "card", "container", "screen", "artboard", "box", "div", "view", "panel", "section", "row", "column", "stack", "chip", "tag", "badge"],
        "use frame (layout: row or column lays out its children)",
    ),
    (&["rectangle", "square"], "use rect"),
    (&["circle", "oval"], "use ellipse"),
    (&["carousel", "scroller"], "use frame (scroll: x on a row)"),
    (&["progress", "ring", "gauge"], "use ellipse (arc with inner and cap: round)"),
    (&["donut", "pie"], "use ellipse (arc with inner draws a ring)"),
    (&["triangle", "diamond", "hexagon", "star"], "use polygon (sides; star for a star)"),
    (&["arrow", "connector"], "use line (arrow_end; from and to attach it)"),
    (&["label", "heading", "paragraph", "title", "span"], "use text"),
    (&["note", "comment"], "use sticky"),
    (&["picture", "photo", "img"], "use image (src from design.asset.add)"),
    (&["vector", "svg", "freehand"], "use path or line"),
];

/// Fields agents write that are not node fields (other tools' and CSS's names), and the field
/// to write instead.
pub const FIELD_HINTS: &[(&[&str], &str)] = &[
    (
        &["anchor", "anchors", "constraints"],
        "use pin_x (left, right, left_right, center, scale) and pin_y (top, bottom, top_bottom, center, scale); in auto layout also absolute: true",
    ),
    (&["position"], "use absolute: true (with x, y) inside auto layout; elsewhere x and y place a node from its parent's top-left"),
    (&["truncate", "ellipsis", "text_overflow", "line_clamp"], "use max_lines (the text needs a width)"),
    (&["spans", "rich_text"], "use runs: [{start, end, …}] (character offsets)"),
    (&["flex_wrap", "wrap"], "use layout: wrap"),
    (&["column_gap"], "use gap (row_gap between wrapped lines)"),
    (&["min_width", "max_width", "min_height", "max_height"], "use min_w, max_w, min_h, max_h"),
    (&["overflow", "scrollable"], "use scroll: x, y or both on a frame (clip hides what overflows)"),
    (&["box_shadow", "drop_shadow", "glow"], "use shadow, or shadows for several (a glow: x 0, y 0, blur)"),
    (&["line_cap", "stroke_linecap"], "use stroke_cap"),
    (&["line_join", "stroke_linejoin"], "use stroke_join"),
    (&["border_gradient", "stroke_fill"], "use stroke_gradient"),
    (&["animation", "keyframes", "transition", "motion"], "nodes do not animate yet"),
    (&["class", "token", "tokens", "variables"], "name colours with {op: color} and looks with {op: style}; a node takes style and $name colours"),
    (&["text_padding", "label_padding"], "use padding"),
    (&["background", "background_color", "bg"], "use fill"),
    (&["border", "border_color", "outline"], "use stroke (a colour) and stroke_width"),
    (&["border_width", "stroke_weight", "line_width"], "use stroke_width"),
    (&["border_radius", "corner_radius"], "use radius (radii for each corner)"),
    (&["font_family", "family"], "use font"),
    (&["font_style"], "use italic: true"),
    (&["text_align"], "use align"),
    (&["text_color", "font_color"], "use color"),
    (&["text_transform"], "use text_case (upper, lower or title)"),
    (&["content", "label", "value"], "use text"),
    (&["type"], "use kind"),
    (&["children"], "add each child as its own add edit, with parent set to this node's id"),
    (&["z", "z_index", "order", "layer"], "paint order is the place among siblings: move with index"),
    (&["visible", "display"], "use hidden: true"),
    (&["direction", "flex_direction", "orientation"], "use layout: row or column"),
    (&["spacing"], "use gap"),
    (&["align_items"], "use items"),
    (&["justify_content"], "use justify"),
    (
        &["padding_x", "padding_y", "padding_top", "padding_right", "padding_bottom", "padding_left"],
        "use padding: [all], [vertical, horizontal] or [top, right, bottom, left]",
    ),
    (&["url", "href", "image_url"], "store the picture with design.asset.add and give its src"),
    (&["rotate", "angle"], "use rotation"),
    (&["x1", "y1", "x2", "y2"], "use points: [[x1, y1], [x2, y2]]"),
    (&["bold", "weight"], "use font_weight (700 is bold)"),
    (&["dasharray", "dash_array", "stroke_dasharray"], "use dash, such as [6, 4]"),
];

/// Values agents give a field that are not its values (CSS's and other tools' names): the field
/// or fields, the values, and the value to write instead.
pub const VALUE_HINTS: &[(&[&str], &[&str], &str)] = &[
    (&["layout"], &["vertical", "vstack"], "column"),
    (&["layout"], &["horizontal", "hstack"], "row"),
    (&["layout"], &["flex_wrap"], "wrap"),
    (&["pin_x"], &["stretch", "both", "left_and_right"], "left_right"),
    (&["pin_y"], &["stretch", "both", "top_and_bottom"], "top_bottom"),
    (&["pin_x", "pin_y"], &["middle", "centre"], "center"),
    (&["scroll"], &["horizontal"], "x"),
    (&["scroll"], &["vertical"], "y"),
    (&["justify", "items"], &["flex_start", "left", "top"], "start"),
    (&["justify", "items"], &["flex_end", "right", "bottom"], "end"),
    (&["justify", "items"], &["middle", "centre"], "center"),
    (&["justify"], &["space_around", "space_evenly"], "space_between"),
    (&["items"], &["fill"], "stretch"),
    (&["w_sizing", "h_sizing"], &["auto", "content", "fit", "fit_content", "wrap", "shrink"], "hug"),
    (&["w_sizing", "h_sizing"], &["stretch", "full", "expand", "grow"], "fill"),
    (&["align"], &["centre", "middle"], "center"),
    (&["align"], &["start"], "left"),
    (&["align"], &["end"], "right"),
    (&["fit"], &["fill"], "stretch"),
    (&["text_case"], &["uppercase", "caps"], "upper"),
    (&["text_case"], &["lowercase"], "lower"),
    (&["text_case"], &["capitalize", "titlecase"], "title"),
    (&["gradient.kind", "stroke_gradient.kind"], &["sweep", "angular"], "conic"),
    (&["stroke_cap", "arc.cap"], &["flat"], "butt"),
];

/// What a field holds, for a value of the wrong shape (`padding: 16` from CSS habit).
pub const SHAPE_HINTS: &[(&str, &str)] = &[
    ("padding", "give a list: [all], [vertical, horizontal] or [top, right, bottom, left]"),
    ("points", "give a list of [x, y] pairs"),
    ("radii", "give four numbers: top-left, top-right, bottom-right, bottom-left"),
    ("dash", "give a list of lengths, such as [6, 4]"),
    ("shadow", "give an object: {x, y, blur, spread, color, inset}"),
    ("gradient", "give an object: {kind, angle, stops: [{at, color}, …]}"),
    ("arc", "give an object: {start, sweep, inner, cap}"),
    ("shadows", "give a list of shadows: [{x, y, blur, spread, color}, …]"),
    ("stroke_gradient", "give an object: {kind, angle, stops: [{at, color}, …]}"),
    ("runs", "give a list: [{start, end, font_size, color, …}, …]"),
];

/// `name` as the tables spell names: lower case, words joined by `_` (`backgroundColor`,
/// `background-color` and `Background Color` are all `background_color`).
fn normal(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut prev_lower = false;
    for c in name.chars() {
        if c == '-' || c == ' ' || c == '_' {
            out.push('_');
            prev_lower = false;
        } else if c.is_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.extend(c.to_lowercase());
            prev_lower = false;
        } else {
            out.push(c);
            prev_lower = c.is_lowercase() || c.is_ascii_digit();
        }
    }
    out
}

fn lookup(table: &[(&[&str], &'static str)], name: &str) -> Option<&'static str> {
    let name = normal(name);
    table.iter().find(|(names, _)| names.contains(&name.as_str())).map(|(_, hint)| *hint)
}

/// The advice for a key that is no node field, if the table has one.
pub fn field_hint(key: &str) -> Option<&'static str> {
    lookup(FIELD_HINTS, key)
}

/// The value to write for `field` instead of `given`, if the table has one.
pub fn value_hint(field: &str, given: &str) -> Option<&'static str> {
    let given = normal(given);
    VALUE_HINTS.iter()
        .find(|(fields, values, _)| fields.contains(&field) && values.contains(&given.as_str()))
        .map(|(_, _, hint)| *hint)
}

/// The id (or name) in `known` closest to `want`, if one is close: case ignored, an edit distance
/// of at most 1 for up to 4 characters, 2 for up to 12, 3 beyond. Ties go to the first in
/// `known`. Banded, so a 5,000-id design costs a few million steps, on the error path only.
pub fn nearest<'a>(want: &str, known: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let want: Vec<char> = want.chars().flat_map(char::to_lowercase).collect();
    if want.is_empty() {
        return None;
    }
    let limit = match want.len() {
        0..=4 => 1,
        5..=12 => 2,
        _ => 3,
    };
    let (mut best, mut row, mut next, mut other) = (None::<(usize, &str)>, Vec::new(), Vec::new(), Vec::new());
    for candidate in known {
        other.clear();
        other.extend(candidate.chars().flat_map(char::to_lowercase));
        if other.len().abs_diff(want.len()) > limit {
            continue;
        }
        let bound = best.map_or(limit, |(d, _)| d.saturating_sub(1));
        if let Some(d) = distance(&want, &other, bound, &mut row, &mut next) {
            best = Some((d, candidate));
            if d == 0 {
                break;
            }
        }
    }
    best.map(|(_, candidate)| candidate)
}

/// Levenshtein distance of `a` and `b` if it is at most `limit`, computing only the band of
/// cells within `limit` of the diagonal.
fn distance(a: &[char], b: &[char], limit: usize, row: &mut Vec<usize>, next: &mut Vec<usize>) -> Option<usize> {
    let far = limit + 1;
    row.clear();
    row.extend((0..=b.len()).map(|j| j.min(far)));
    for (i, ca) in a.iter().enumerate() {
        next.clear();
        next.resize(b.len() + 1, far);
        next[0] = (i + 1).min(far);
        let (lo, hi) = ((i + 1).saturating_sub(limit).max(1), (i + 1 + limit).min(b.len()));
        let mut least = next[0];
        for j in lo..=hi {
            let cost = usize::from(*ca != b[j - 1]);
            next[j] = (row[j - 1] + cost).min(row[j] + 1).min(next[j - 1] + 1).min(far);
            least = least.min(next[j]);
        }
        if least > limit {
            return None;
        }
        std::mem::swap(row, next);
    }
    Some(row[b.len()]).filter(|d| *d <= limit)
}

/// Every name a node field goes by: the fields, and `width`/`height`.
fn field_names() -> impl Iterator<Item = &'static str> {
    FIELDS.iter().map(|row| row.0).chain(["width", "height"])
}

/// The refusal of `key`, which no node field has: the table's advice, else the closest field.
pub(crate) fn not_a_field(key: &str) -> String {
    match field_hint(key) {
        Some(hint) => format!("{key}: not a node field; {hint}"),
        None => match nearest(key, field_names()) {
            Some(near) => format!("{key}: not a node field; did you mean {near:?}?"),
            None => format!("{key}: not a node field; design.get lists a node's fields"),
        },
    }
}

/// Why `value` does not read as `field`, from serde's `why`, with what to write instead: the
/// field's range for a number its type cannot hold, a value hint or the closest of its values for
/// an unknown one, the shape it takes for a value of the wrong type.
pub(crate) fn bad_value(field: &str, value: &Value, why: &str) -> String {
    if let (Some(n), Some((lo, hi))) = (value.as_i64().filter(|_| why.starts_with("invalid value: integer")), range_of(field)) {
        return format!("{field}: {n} is outside {lo} to {hi}");
    }
    let tail = if why.starts_with("unknown variant") {
        variant_tail(field, value, why)
    } else if why.starts_with("invalid type") {
        SHAPE_HINTS.iter().find(|(f, _)| *f == field).map(|(_, hint)| (*hint).to_string())
    } else {
        None
    };
    match tail {
        Some(tail) => format!("{field}: {why}; {tail}"),
        None => format!("{field}: {why}"),
    }
}

/// What to write for a value that is not one of its field's: the table's advice, else the closest
/// variant. For a nested object (`gradient: {kind: "sweep"}`) the value is the one serde names,
/// looked up as `gradient.kind`.
fn variant_tail(field: &str, value: &Value, why: &str) -> Option<String> {
    let (path, given) = match value {
        Value::String(given) => (field.to_string(), given.as_str()),
        Value::Object(map) => {
            let named = why.split('`').nth(1)?;
            let (key, v) = map.iter().find(|(_, v)| v.as_str() == Some(named))?;
            (format!("{field}.{key}"), v.as_str()?)
        }
        _ => return None,
    };
    value_hint(&path, given).map(|hint| format!("use {hint:?}")).or_else(|| {
        let (variants, normal) = (variants(why), normal(given));
        variants.iter().copied().find(|v| *v == normal)
            .or_else(|| nearest(given, variants.iter().copied()))
            .map(|v| format!("did you mean {v:?}?"))
    })
}

/// The values serde's "unknown variant `x`, expected one of `a`, `b`" lists.
fn variants(why: &str) -> Vec<&str> {
    why.split_once("expected").map_or_else(Vec::new, |(_, list)| list.split('`').skip(1).step_by(2).collect())
}

/// The rendered `DesignNode` schema's properties, read once.
fn node_properties() -> &'static Map<String, Value> {
    static PROPERTIES: OnceLock<Map<String, Value>> = OnceLock::new();
    PROPERTIES.get_or_init(|| {
        let schema = serde_json::to_value(schemars::schema_for!(Node)).unwrap_or_default();
        schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default()
    })
}

/// The range the schema gives a numeric field.
fn range_of(field: &str) -> Option<(Value, Value)> {
    let property = node_properties().get(field)?;
    Some((property.get("minimum")?.clone(), property.get("maximum")?.clone()))
}

/// `update`'s `set`: node fields by name, read off `Node`'s own schema, so a new field joins it.
pub fn patch_schema(generator: &mut SchemaGenerator) -> Schema {
    let node = Value::from(<Node as JsonSchema>::json_schema(generator));
    let mut names: Vec<String> = node.get("properties").and_then(Value::as_object)
        .map(|p| p.keys().filter(|k| !matches!(k.as_str(), "id" | "kind" | "parent")).cloned().collect())
        .unwrap_or_default();
    for alias in ["width", "height"] {
        if !names.iter().any(|n| n == alias) {
            names.push(alias.into());
        }
    }
    json_schema!({
        "type": "object",
        "description": "Node fields to change, each with a value as an add takes it; null puts the field back to its default. Not id, kind or parent (move changes the parent).",
        "propertyNames": {"enum": names},
    })
}

/// Where edit `at` of a list stands, for its refusals: `ops[3] update "hero"`, or `ops[3] add
/// rect` for an add that leaves its id to the engine.
pub fn label(at: usize, op: &NodeOp) -> String {
    match op {
        NodeOp::Add { node, .. } if node.id.is_empty() => format!("ops[{at}] add {}", node.kind.name()),
        NodeOp::Add { node, .. } => format!("ops[{at}] add {:?}", node.id),
        NodeOp::Update { id, .. } => format!("ops[{at}] update {id:?}"),
        NodeOp::Move { id, .. } => format!("ops[{at}] move {id:?}"),
        NodeOp::Delete { id } => format!("ops[{at}] delete {id:?}"),
        NodeOp::Copy { id, .. } => format!("ops[{at}] copy {id:?}"),
        NodeOp::Color { name, .. } => format!("ops[{at}] color {name:?}"),
        NodeOp::Style { name, .. } => format!("ops[{at}] style {name:?}"),
        NodeOp::DeleteStyle { name } => format!("ops[{at}] delete_style {name:?}"),
    }
}

/// The key a refusal names first, if it names one: a key it starts with before a colon (`fill:
/// …`, `shadow.blur: …`, `fil: not a node field`), or a field it starts with (`parent "p" is not
/// …`).
pub fn field_of(why: &str) -> Option<&str> {
    let end = why.find([':', ' ']).unwrap_or(why.len());
    let path = &why[..end];
    let keyed = why[end..].starts_with(':') && !path.is_empty() && path.chars().all(|c| c.is_alphanumeric() || "_-.[]".contains(c));
    let base = path.split(['.', '[']).next().unwrap_or(path);
    (keyed || field_kinds(base).is_some()).then_some(path)
}

/// `details` of a refusal of edit `index`: `{op, index, id, field}`, so an agent or the canvas
/// can point at the edit without reading the message (`op` is `index` by its older name).
pub fn details(index: usize, id: Option<&str>, field: Option<&str>) -> Value {
    json!({"op": index, "index": index, "id": id, "field": field})
}

/// An edit that did not read, explained.
#[derive(Debug, Clone, PartialEq)]
pub struct Unread {
    pub index: usize,
    /// The node it names, when it names one.
    pub id: Option<String>,
    /// The key that did not read: `op`, `set`, `node.kind`…
    pub field: Option<String>,
    /// `ops[3] update "hero": set: …`.
    pub message: String,
}

impl Unread {
    pub fn details(&self) -> Value {
        details(self.index, self.id.as_deref(), self.field.as_deref())
    }
}

/// One edit as the schema describes it: its name, the keys it takes and needs, and the least
/// edit of it that reads, to try one key at a time in.
struct Shape {
    op: String,
    keys: Vec<String>,
    required: Vec<String>,
    least: Map<String, Value>,
}

/// The edits `NodeOp` reads, off its own schema, so a new edit is explained like the others.
fn shapes() -> &'static [Shape] {
    static SHAPES: OnceLock<Vec<Shape>> = OnceLock::new();
    SHAPES.get_or_init(|| {
        let schema = serde_json::to_value(schemars::schema_for!(NodeOp)).unwrap_or_default();
        let variants = schema.get("oneOf").and_then(Value::as_array).cloned().unwrap_or_default();
        variants.iter().filter_map(|variant| {
            let properties = variant.get("properties")?.as_object()?;
            let op = properties.get("op")?.get("const")?.as_str()?.to_string();
            let required: Vec<String> = variant.get("required")?.as_array()?.iter()
                .filter_map(Value::as_str).filter(|k| *k != "op").map(str::to_string).collect();
            let mut least = Map::from_iter([("op".to_string(), Value::from(op.as_str()))]);
            for key in &required {
                least.insert(key.clone(), least_value(&properties[key]));
            }
            let keys = properties.keys().filter(|k| *k != "op").cloned().collect();
            Some(Shape { op, keys, required, least })
        }).collect()
    })
}

/// The least value that reads as a property of this schema: a node a rect, else by its type.
fn least_value(property: &Value) -> Value {
    if property.get("$ref").and_then(Value::as_str).is_some_and(|r| r.ends_with("/DesignNode")) {
        return json!({"kind": "rect", "parent": "p"});
    }
    let ty = match property.get("type") {
        Some(Value::Array(types)) => types.first().and_then(Value::as_str),
        Some(ty) => ty.as_str(),
        None => None,
    };
    match ty {
        Some("string") => json!("x"),
        Some("object") => json!({}),
        Some("array") => json!([]),
        Some("integer" | "number") => json!(0),
        Some("boolean") => json!(false),
        _ => Value::Null,
    }
}

fn names(list: &[&str]) -> String {
    match list {
        [] => String::new(),
        [one] => (*one).to_string(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// Why edit `at`, which did not read as an edit, did not: its label and the key at fault.
pub fn explain(at: usize, edit: &Value) -> Unread {
    let mut unread = Unread { index: at, id: None, field: None, message: String::new() };
    let edits: Vec<&str> = shapes().iter().map(|s| s.op.as_str()).collect();
    let each = format!("each edit is an object with op: {}", names(&edits));
    let Some(map) = edit.as_object() else {
        unread.message = format!("ops[{at}]: {each}");
        return unread;
    };
    let Some(op) = map.get("op").and_then(Value::as_str) else {
        unread.field = Some("op".into());
        unread.message = format!("ops[{at}]: op: {each}");
        return unread;
    };
    let Some(shape) = shapes().iter().find(|s| s.op == op) else {
        let advice = lookup(OP_HINTS, op).map(str::to_string)
            .or_else(|| nearest(op, edits.iter().copied()).map(|near| format!("did you mean {near:?}?")))
            .unwrap_or_else(|| format!("the edits are {}", names(&edits)));
        unread.field = Some("op".into());
        unread.message = format!("ops[{at}]: op {op:?} is not an edit; {advice}");
        return unread;
    };
    let node = map.get("node").and_then(Value::as_object);
    // What the edit names, as `label` names it: a node's id, or a colour's or style's name.
    let id = match node {
        Some(node) => node.get("id"),
        None => map.get("id").or_else(|| map.get("name")),
    };
    unread.id = id.and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_string);
    let kind = node.and_then(|n| n.get("kind")).and_then(Value::as_str).and_then(NodeKind::from_name);
    let label = match (&unread.id, kind) {
        (Some(id), _) => format!("ops[{at}] {op} {id:?}"),
        (None, Some(kind)) => format!("ops[{at}] {op} {}", kind.name()),
        (None, None) => format!("ops[{at}] {op}"),
    };
    let (field, why) = why_edit(shape, map, edit);
    unread.message = format!("{label}: {why}");
    unread.field = field;
    unread
}

/// The key of an edit that does not read, and why: each key tried alone in the least edit of its
/// kind, then the keys it needs.
fn why_edit(shape: &Shape, map: &Map<String, Value>, edit: &Value) -> (Option<String>, String) {
    let op = shape.op.as_str();
    for (key, value) in map.iter().filter(|(key, _)| *key != "op") {
        if !shape.keys.contains(key) {
            let why = if field_kinds(key).is_some() && matches!(op, "update" | "style") {
                let what = if op == "style" { "style" } else { "node" };
                format!("{key}: a {what} field goes in set: {{\"set\": {{{key:?}: …}}}}")
            } else if field_kinds(key).is_some() && op == "add" {
                format!("{key}: a node field goes in node: {{\"node\": {{{key:?}: …}}}}")
            } else if op == "copy" && matches!(key.as_str(), "x" | "y" | "offset" | "position") {
                format!("{key}: not a field of copy; move the copy with dx and dy")
            } else {
                let keys: Vec<&str> = shape.keys.iter().map(String::as_str).collect();
                match nearest(key, keys.iter().copied()) {
                    Some(near) => format!("{key}: not a field of {op}; did you mean {near:?}?"),
                    None => format!("{key}: not a field of {op}, which takes {}", names(&keys)),
                }
            };
            return (Some(key.clone()), why);
        }
        let mut probe = shape.least.clone();
        probe.insert(key.clone(), value.clone());
        if let Err(error) = NodeOp::deserialize(&Value::Object(probe)) {
            if key == "node" {
                return why_node(value, &error.to_string());
            }
            let why = format!("{key}: {error}");
            return (Some(key.clone()), match key.as_str() {
                "set" if op == "style" => format!("{why}; give the style fields to set, such as {{\"fill\": \"$accent\", \"radius\": 12}}"),
                "set" => format!("{why}; give the node fields to change, such as {{\"fill\": \"#FFFFFF\"}}"),
                "index" => format!("{why}; 0 is the bottom of the parent's children"),
                _ => why,
            });
        }
    }
    if let Some(key) = shape.required.iter().find(|key| !map.contains_key(*key)) {
        let tail = match key.as_str() {
            "id" => format!("name the node to {op}"),
            "name" if op == "color" => "name the colour, as nodes will write it after a $".into(),
            "name" if op == "style" => "name the style, as nodes will write it in style".into(),
            "name" => "name the colour or style to take out".into(),
            "value" => "give the colour, such as \"#4C9DFF\"".into(),
            "set" if op == "style" => "give the style fields to set, such as {\"fill\": \"$accent\", \"radius\": 12}".into(),
            "node" => "give the node to add, such as {\"kind\": \"rect\", \"parent\": \"page-1\"}".into(),
            "set" => "give the node fields to change, such as {\"fill\": \"#FFFFFF\"}".into(),
            _ => format!("{op} needs it"),
        };
        return (Some(key.clone()), format!("{key}: missing; {tail}"));
    }
    (None, NodeOp::deserialize(edit).err().map(|e| e.to_string()).unwrap_or_default())
}

/// Why an add's node does not read: it reads whatever it holds (a field this build cannot read
/// waits for the strict check), so only its shape and its kind can be at fault.
fn why_node(node: &Value, error: &str) -> (Option<String>, String) {
    let kinds: Vec<&str> = NodeKind::ALL.iter().map(|k| k.name()).collect();
    let Some(map) = node.as_object() else {
        return (Some("node".into()), "node: an add's node is an object of node fields, such as {\"kind\": \"rect\", \"parent\": \"page-1\"}".into());
    };
    let field = Some("node.kind".to_string());
    match map.get("kind") {
        None => (field, format!("node.kind: missing; one of {}", names(&kinds))),
        Some(Value::String(kind)) => {
            let advice = lookup(KIND_HINTS, kind).map(str::to_string)
                .or_else(|| nearest(kind, kinds.iter().copied()).map(|near| format!("did you mean {near:?}?")))
                .unwrap_or_else(|| format!("the kinds are {}", names(&kinds)));
            (field, format!("node.kind: {kind:?} is not a kind; {advice}"))
        }
        Some(_) => (field, format!("node.kind: {}", error.strip_prefix("kind: ").unwrap_or(error))),
    }
}

/// `deserialize_with` for `design.apply`'s `ops`: each edit read straight into a `NodeOp`, with
/// no copy beyond the one serde's tagged read makes, so a list that reads costs what it did. An
/// edit that does not read is refused by its index and serde's reason; the bus then explains it
/// in full from the payload ([`locate`], on the refusal path only). Past `MAX_LENIENT_OPS` edits
/// the list is refused while it is read, before the rest is buffered.
pub fn de_ops<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<NodeOp>, D::Error> {
    struct Ops;
    impl<'de> Visitor<'de> for Ops {
        type Value = Vec<NodeOp>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of edits")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<NodeOp>, A::Error> {
            let mut ops = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(MAX_LENIENT_OPS));
            loop {
                if ops.len() == MAX_LENIENT_OPS {
                    return match seq.next_element::<IgnoredAny>()? {
                        Some(_) => Err(de::Error::custom(too_many())),
                        None => Ok(ops),
                    };
                }
                match seq.next_element::<NodeOp>() {
                    Ok(Some(op)) => ops.push(op),
                    Ok(None) => return Ok(ops),
                    Err(error) => return Err(de::Error::custom(format_args!("ops[{}]: {error}", ops.len()))),
                }
            }
        }
    }
    d.deserialize_seq(Ops)
}

fn too_many() -> String {
    format!("ops: more than {MAX_LENIENT_OPS} edits; send at most {MAX_OPS} in one list")
}

/// Why and where a `design.apply` payload that did not read went wrong, for `bus.schema`: the
/// edit at fault explained ([`explain`]) with `details {op, index, id, field}`, or the list too
/// long with `{field: "ops"}`. `None` when no edit is at fault (another key of the payload).
pub fn locate(payload: &Value) -> Option<(String, Value)> {
    let ops = payload.get("ops")?.as_array()?;
    if ops.len() > MAX_LENIENT_OPS {
        return Some((too_many(), json!({"field": "ops"})));
    }
    let (at, edit) = ops.iter().enumerate().find(|(_, edit)| NodeOp::deserialize(*edit).is_err())?;
    let unread = explain(at, edit);
    let details = unread.details();
    Some((unread.message, details))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::design::ApplyIn;

    /// `ops` read as the bus reads a `design.apply` payload: the edits, or the refusal's reason.
    fn read(ops: Value) -> Result<Vec<NodeOp>, String> {
        let payload = json!({"design_id": 1, "ops": ops});
        let entry = crate::registry::Registry::global().get("design.apply").unwrap();
        match entry.check(&payload) {
            Ok(()) => Ok(serde_json::from_value::<ApplyIn>(payload).unwrap().ops),
            Err(error) => Err(error.message.strip_prefix("payload for design.apply does not match its schema: ").unwrap().to_string()),
        }
    }

    /// The refusal a strict list gives edit `at`: its label and the first rule it breaks.
    fn refusal(at: usize, op: &NodeOp) -> String {
        let NodeOp::Add { node, .. } = op else { panic!("an add") };
        format!("{}: {}", label(at, op), node.check_new().unwrap_err())
    }

    fn add(at: usize, node: Value) -> String {
        let mut ops = vec![json!({"op": "delete", "id": "a"}); at];
        ops.push(json!({"op": "add", "node": node}));
        let ops = read(Value::Array(ops)).unwrap();
        refusal(at, &ops[at])
    }

    fn update(before: Value, set: Value) -> String {
        let node = Node::read(before).unwrap();
        let set = set.as_object().unwrap().clone();
        match node.patched(&set) {
            Ok(after) => Node::check_update(&node, &set, &after).unwrap_err(),
            Err(why) => why,
        }
    }

    /// W16, W19: an edit's node reads whatever it holds; the strict check names a misspelt field
    /// with the field meant, and the edit by its index.
    #[test]
    fn an_unknown_field_is_named_with_its_index_and_a_suggestion() {
        let why = add(2, json!({"id": "x", "parent": "p", "kind": "rect", "fil": "#fff"}));
        assert_eq!(why, "ops[2] add \"x\": fil: not a node field; did you mean \"fill\"?");
        assert_eq!(add(0, json!({"parent": "p", "kind": "text", "text": "a", "lineHeight": 2})),
            "ops[0] add text: lineHeight: not a node field; did you mean \"line_height\"?");
        assert_eq!(add(0, json!({"parent": "p", "kind": "text", "text": "a", "zzzz": 2})),
            "ops[0] add text: zzzz: not a node field; design.get lists a node's fields");
    }

    #[test]
    fn a_wrong_type_names_the_field_inside_a_tagged_edit() {
        let why = add(1, json!({"id": "s", "parent": "p", "kind": "rect", "shadow": [1, 2]}));
        assert_eq!(why, "ops[1] add \"s\": shadow: invalid type: sequence, expected an object; give an object: {x, y, blur, spread, color, inset}");
    }

    /// TM-2: the refusals agents met as serde's bare message, each named by edit and field.
    #[test]
    fn each_bad_edit_names_its_index_and_field() {
        let why = add(3, json!({"id": "x", "parent": "p", "kind": "frame", "layout": "grid"}));
        assert_eq!(why, "ops[3] add \"x\": layout: unknown variant `grid`, expected one of `none`, `row`, `column`, `wrap`");
        assert_eq!(add(0, json!({"id": "x", "parent": "p", "kind": "polygon", "sides": 300})), "ops[0] add \"x\": sides: 300 is outside 3 to 24");
        assert_eq!(add(0, json!({"id": "x", "parent": "p", "kind": "polygon", "sides": -1})), "ops[0] add \"x\": sides: -1 is outside 3 to 24");
        let rect = json!({"id": "r", "parent": "p", "kind": "rect"});
        assert_eq!(update(rect.clone(), json!({"padding": 16})),
            "padding: invalid type: integer `16`, expected a sequence; give a list: [all], [vertical, horizontal] or [top, right, bottom, left]");
        assert_eq!(update(rect.clone(), json!({"fill": "red"})), "fill: \"red\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour");
        assert_eq!(update(rect, json!({"background": "#fff"})), "background: not a node field; use fill");
    }

    #[test]
    fn unsupported_fields_kinds_values_and_edits_get_their_hint() {
        let node = |key: &str, value: Value| {
            let mut node = json!({"id": "t", "parent": "p", "kind": "text", "text": "Hi"});
            node[key] = value;
            add(0, node)
        };
        assert!(node("anchor", json!("top")).ends_with("anchor: not a node field; use pin_x (left, right, left_right, center, scale) and pin_y (top, bottom, top_bottom, center, scale); in auto layout also absolute: true"));
        assert!(node("minWidth", json!(10)).ends_with("minWidth: not a node field; use min_w, max_w, min_h, max_h"));
        let pinned = json!({"id": "t", "parent": "p", "kind": "rect", "pin_x": "stretch"});
        assert!(add(0, pinned).ends_with("; use \"left_right\""));
        assert!(node("truncate", json!(1)).ends_with("truncate: not a node field; use max_lines (the text needs a width)"));
        assert!(node("spans", json!([])).ends_with("spans: not a node field; use runs: [{start, end, …}] (character offsets)"));
        assert!(node("glow", json!(1)).ends_with("glow: not a node field; use shadow, or shadows for several (a glow: x 0, y 0, blur)"));
        assert!(node("strokeLinecap", json!("round")).ends_with("strokeLinecap: not a node field; use stroke_cap"));
        // A nested value is looked up by its path.
        let stops = json!([{"at": 0, "color": "#fff"}, {"at": 1, "color": "#000"}]);
        let sweep = json!({"id": "e", "parent": "p", "kind": "ellipse", "gradient": {"kind": "sweep", "stops": stops}});
        assert!(add(0, sweep).ends_with("gradient: unknown variant `sweep`, expected one of `linear`, `radial`, `conic`; use \"conic\""));
        let ring = json!({"id": "e", "parent": "p", "kind": "ellipse", "arc": {"inner": 0.8, "cap": "rond"}});
        assert!(add(0, ring).ends_with("; did you mean \"round\"?"));
        let flat = json!({"id": "l", "parent": "p", "kind": "line", "points": [[0, 0], [9, 0]], "stroke_cap": "flat"});
        assert!(add(0, flat).ends_with("; use \"butt\""));
        assert!(node("Background-Color", json!("#fff")).ends_with("Background-Color: not a node field; use fill"));
        let frame = json!({"id": "f", "parent": "p", "kind": "frame", "layout": "vertical"});
        assert!(add(0, frame).ends_with("layout: unknown variant `vertical`, expected one of `none`, `row`, `column`, `wrap`; use \"column\""));
        let justify = json!({"id": "f", "parent": "p", "kind": "frame", "justify": "space-between"});
        assert!(add(0, justify).ends_with("; did you mean \"space_between\"?"));
        let align = json!({"id": "t", "parent": "p", "kind": "text", "align": "centre"});
        assert!(add(0, align).ends_with("; use \"center\""));
        // Edits and kinds that are not.
        let why = read(json!([{"op": "add", "node": {"kind": "page"}}, {"op": "duplicate", "id": "a"}])).unwrap_err();
        assert_eq!(why, "ops[1]: op \"duplicate\" is not an edit; use copy {id, as}");
        assert!(read(json!([{"op": "instance"}])).unwrap_err().ends_with("components are not supported yet: copy the node and give the copies a style"));
        assert!(read(json!([{"op": "palette"}])).unwrap_err().ends_with("use color {name, value}"));
        assert!(read(json!([{"op": "theme"}])).unwrap_err().ends_with("use style {name, set}"));
        assert!(read(json!([{"op": "remove_style"}])).unwrap_err().ends_with("use delete_style {name}"));
        assert_eq!(read(json!([{"op": "updte", "id": "a", "set": {}}])).unwrap_err(), "ops[0]: op \"updte\" is not an edit; did you mean \"update\"?");
        assert_eq!(read(json!([{"op": "frobnicate"}])).unwrap_err(), "ops[0]: op \"frobnicate\" is not an edit; the edits are add, update, move, delete, copy, color, style or delete_style");
        assert_eq!(read(json!([{"op": "add", "node": {"id": "b", "parent": "p", "kind": "button"}}])).unwrap_err(),
            "ops[0] add \"b\": node.kind: \"button\" is not a kind; use frame (layout: row or column lays out its children)");
        assert_eq!(read(json!([{"op": "add", "node": {"parent": "p", "kind": "elipse"}}])).unwrap_err(),
            "ops[0] add: node.kind: \"elipse\" is not a kind; did you mean \"ellipse\"?");
    }

    /// What `de_ops` refuses itself: the shape of each edit.
    #[test]
    fn each_edit_is_read_alone_and_explained() {
        let cases = [
            (json!(["add"]), "ops[0]: each edit is an object with op: add, update, move, delete, copy, color, style or delete_style"),
            (json!([{"id": "a"}]), "ops[0]: op: each edit is an object with op: add, update, move, delete, copy, color, style or delete_style"),
            (json!([{"op": "update", "set": {}}]), "ops[0] update: id: missing; name the node to update"),
            (json!([{"op": "update", "id": "a"}]), "ops[0] update \"a\": set: missing; give the node fields to change, such as {\"fill\": \"#FFFFFF\"}"),
            (json!([{"op": "update", "id": "a", "set": "red"}]),
                "ops[0] update \"a\": set: invalid type: string \"red\", expected a map; give the node fields to change, such as {\"fill\": \"#FFFFFF\"}"),
            (json!([{"op": "update", "id": "a", "fill": "#fff"}]), "ops[0] update \"a\": fill: a node field goes in set: {\"set\": {\"fill\": …}}"),
            (json!([{"op": "add", "kind": "rect", "parent": "p"}]), "ops[0] add: kind: a node field goes in node: {\"node\": {\"kind\": …}}"),
            (json!([{"op": "move", "id": "a", "parnet": "p"}]), "ops[0] move \"a\": parnet: not a field of move; did you mean \"parent\"?"),
            (json!([{"op": "move", "id": "a", "index": -1}]),
                "ops[0] move \"a\": index: invalid value: integer `-1`, expected u32; 0 is the bottom of the parent's children"),
            (json!([{"op": "delete", "id": 7}]), "ops[0] delete: id: invalid type: integer `7`, expected a string"),
            (json!([{"op": "add"}]), "ops[0] add: node: missing; give the node to add, such as {\"kind\": \"rect\", \"parent\": \"page-1\"}"),
            (json!([{"op": "add", "node": "rect"}]), "ops[0] add: node: an add's node is an object of node fields, such as {\"kind\": \"rect\", \"parent\": \"page-1\"}"),
            (json!([{"op": "copy", "id": "card", "offset": 1}]), "ops[0] copy \"card\": offset: not a field of copy; move the copy with dx and dy"),
            (json!([{"op": "color", "name": "accent"}]), "ops[0] color \"accent\": value: missing; give the colour, such as \"#4C9DFF\""),
            (json!([{"op": "color", "value": "#fff"}]), "ops[0] color: name: missing; name the colour, as nodes will write it after a $"),
            (json!([{"op": "style", "name": "card", "radius": 12}]), "ops[0] style \"card\": radius: a style field goes in set: {\"set\": {\"radius\": …}}"),
            (json!([{"op": "style", "name": "card", "set": [12]}]),
                "ops[0] style \"card\": set: invalid type: sequence, expected a map; give the style fields to set, such as {\"fill\": \"$accent\", \"radius\": 12}"),
            (json!([{"op": "delete_style"}]), "ops[0] delete_style: name: missing; name the colour or style to take out"),
            (json!([{"op": "add", "node": {"id": "n", "parent": "p"}}]),
                "ops[0] add \"n\": node.kind: missing; one of page, frame, group, rect, ellipse, line, text, sticky, path, image, polygon or icon"),
        ];
        for (ops, want) in cases {
            assert_eq!(read(ops.clone()).unwrap_err(), want, "{ops}");
        }
        // What reads is read whole, in order.
        let ops = read(json!([{"op": "add", "node": {"id": "f", "parent": "p", "kind": "frame"}}, {"op": "delete", "id": "f"}])).unwrap();
        assert_eq!(ops.iter().map(NodeOp::target).collect::<Vec<_>>(), ["f", "f"]);
        assert!(read(json!({"op": "add"})).unwrap_err().contains("expected a list of edits"));
    }

    /// I-2: where a list that does not read went wrong, for `bus.schema`'s details; serde alone
    /// (de_ops) names the edit by its index.
    #[test]
    fn locate_names_the_edit_and_field() {
        let payload = json!({"design_id": 1, "ops": [{"op": "delete", "id": "a"}, {"op": "update", "id": "hero", "set": 3}]});
        let (why, details) = locate(&payload).unwrap();
        assert!(why.starts_with("ops[1] update \"hero\": set: invalid type: integer `3`, expected a map"), "{why}");
        assert_eq!(details, json!({"op": 1, "index": 1, "id": "hero", "field": "set"}));
        let serde = serde_json::from_value::<ApplyIn>(payload).unwrap_err().to_string();
        assert_eq!(serde, "ops[1]: invalid type: integer `3`, expected a map");
        let error = crate::registry::Registry::global().get("design.apply").unwrap()
            .check(&json!({"design_id": 1, "ops": [{"op": "add", "node": {"parent": "p", "kind": "div"}}]})).unwrap_err();
        assert_eq!((error.code.as_str(), error.details), ("bus.schema", Some(json!({"op": 0, "index": 0, "id": null, "field": "node.kind"}))));
        assert_eq!(locate(&json!({"design_id": 1, "ops": [{"op": "delete", "id": "a"}]})), None);
        assert_eq!(locate(&json!({"design_id": "one"})), None);
        let long = json!({"ops": vec![json!({"op": "delete", "id": "a"}); MAX_LENIENT_OPS + 1]});
        assert_eq!(locate(&long).map(|(_, details)| details), Some(json!({"field": "ops"})));
        let unread = explain(4, &json!({"op": "clone", "id": "a"}));
        assert_eq!((unread.index, unread.id.as_deref(), unread.field.as_deref()), (4, None, Some("op")));
    }

    #[test]
    fn an_over_long_list_stops_while_reading() {
        let edit = json!({"op": "delete", "id": "a"});
        assert_eq!(read(Value::Array(vec![edit.clone(); MAX_LENIENT_OPS])).unwrap().len(), MAX_LENIENT_OPS);
        let why = read(Value::Array(vec![edit; MAX_LENIENT_OPS + 1])).unwrap_err();
        assert_eq!(why, format!("ops: more than {MAX_LENIENT_OPS} edits; send at most {MAX_OPS} in one list"));
        // Past the limit nothing more is read: a broken edit after it is not what is reported.
        let mut ops = vec![json!({"op": "delete", "id": "a"}); MAX_LENIENT_OPS];
        ops.push(json!({"op": "nope"}));
        assert!(read(Value::Array(ops)).unwrap_err().starts_with("ops: more than"));
    }

    #[test]
    fn nearest_finds_close_ids_and_not_far_ones() {
        let ids = ["card-title", "card-body", "hero", "Save"];
        assert_eq!(nearest("card_title", ids), Some("card-title"));
        assert_eq!(nearest("CARD-TITLE", ids), Some("card-title"));
        assert_eq!(nearest("save", ids), Some("Save"));
        assert_eq!(nearest("hro", ids), Some("hero"));
        // Up to 4 characters, one edit; up to 12, two; beyond, three.
        assert_eq!(nearest("hxrx", ids), None);
        assert_eq!(nearest("crd-bdy", ids), Some("card-body"));
        assert_eq!(nearest("cd-by", ids), None);
        assert_eq!(nearest("the-long-header-x", ["the-long-header"]), Some("the-long-header"));
        assert_eq!(nearest("the-long-hea", ["the-long-header"]), None);
        assert_eq!(nearest("", ids), None);
        assert_eq!(nearest("hero", []), None);
        // The closest wins; a tie goes to the first.
        assert_eq!(nearest("fil", ["file", "fill"]), Some("file"));
        assert_eq!(nearest("filll", ["fil", "fill"]), Some("fill"));
        assert_eq!(nearest("ab", ["ba", "ab"]), Some("ab"));
    }

    #[test]
    fn the_patch_schema_lists_every_node_field_but_identity() {
        let schema = serde_json::to_value(schemars::schema_for!(NodeOp)).unwrap();
        let update = schema["oneOf"].as_array().unwrap().iter().find(|v| v["properties"]["op"]["const"] == "update").unwrap();
        let names: Vec<&str> = update["properties"]["set"]["propertyNames"]["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let mut want: Vec<&str> = FIELDS.iter().map(|row| row.0).filter(|f| !matches!(*f, "id" | "kind" | "parent")).collect();
        want.extend(["width", "height"]);
        let (mut got, mut want) = (names.clone(), want);
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
        assert_eq!(update["properties"]["set"]["type"], "object");
    }

    /// `design.apply`'s schema states the limits the engine holds a list and a design to.
    #[test]
    fn limits_in_the_schema_match_the_constants() {
        use crate::design::{MAX_DEPTH, MAX_DESIGN_BYTES, MAX_NODES};
        use crate::registry::Op;
        let schema = serde_json::to_value(schemars::schema_for!(ApplyIn)).unwrap();
        let ops = &schema["properties"]["ops"];
        assert_eq!((ops["minItems"].as_u64(), ops["maxItems"].as_u64()), (Some(1), Some(MAX_OPS as u64)));
        let doc = ops["description"].as_str().unwrap();
        for limit in [format!("1 to {MAX_OPS} edits"), format!("{MAX_NODES} nodes"), format!("{} KiB", MAX_DESIGN_BYTES >> 10), format!("{MAX_DEPTH} levels")] {
            assert!(doc.contains(&limit), "{limit} in {doc}");
        }
        let summary = crate::ops::design::Apply::META.summary;
        for limit in [format!("{MAX_OPS} edits"), format!("{MAX_NODES} nodes"), format!("{} KiB", MAX_DESIGN_BYTES >> 10)] {
            assert!(summary.contains(&limit), "{limit} in {summary}");
        }
    }

    /// A hint names only what does not exist: when an edit, a kind or a field of that name
    /// arrives, its entry leaves the table (or the advice would hide the real thing). Each value
    /// hint's advice is a value its fields read.
    #[test]
    fn hints_never_shadow_what_exists() {
        let edits: Vec<&str> = shapes().iter().map(|s| s.op.as_str()).collect();
        assert_eq!(edits, ["add", "update", "move", "delete", "copy", "color", "style", "delete_style"]);
        for (names, _) in OP_HINTS {
            assert!(names.iter().all(|n| !edits.contains(n) && normal(n) == *n), "{names:?}");
        }
        for (names, _) in KIND_HINTS {
            assert!(names.iter().all(|n| NodeKind::from_name(n).is_none() && normal(n) == *n), "{names:?}");
        }
        for (names, _) in FIELD_HINTS {
            assert!(names.iter().all(|n| field_kinds(n).is_none() && normal(n) == *n), "{names:?}");
        }
        for (fields, values, advice) in VALUE_HINTS {
            for path in *fields {
                // A nested value (`gradient.kind`) is probed inside the least object of its field.
                let (field, key) = path.split_once('.').map_or((*path, None), |(f, k)| (f, Some(k)));
                let kind = NodeKind::ALL.into_iter().find(|k| crate::design::takes(*k, field)).unwrap();
                let wrap = |value: &str| match key {
                    None => json!(value),
                    Some(key) if field.ends_with("gradient") => json!({key: value, "stops": []}),
                    Some(key) => json!({key: value}),
                };
                let probe = |value: &str| super::super::read::probe(kind, field, &wrap(value));
                assert_eq!(probe(advice), Ok(()), "{field}: {advice}");
                for value in *values {
                    assert!(probe(value).is_err() && normal(value) == *value, "{field}: {value} is a value");
                }
            }
        }
        for (field, _) in SHAPE_HINTS {
            assert!(field_kinds(field).is_some(), "{field}");
        }
    }

    #[test]
    fn names_are_compared_as_the_tables_spell_them() {
        for (given, want) in [("backgroundColor", "background_color"), ("Background-Color", "background_color"), ("z-index", "z_index"),
            ("Font Family", "font_family"), ("x1", "x1"), ("flexWrap", "flex_wrap"), ("HTMLText", "htmltext")] {
            assert_eq!(normal(given), want, "{given}");
        }
        assert_eq!(field_hint("borderRadius"), Some("use radius (radii for each corner)"));
        assert_eq!(value_hint("w_sizing", "fit-content"), Some("hug"));
        assert_eq!(value_hint("layout", "fit-content"), None);
    }

    #[test]
    fn a_refusal_names_its_field() {
        for (why, field) in [
            ("fill: \"red\" is not #RGB", Some("fill")),
            ("shadow.blur: 300 is outside 0 to 200", Some("shadow.blur")),
            ("parent \"p\" is not in the design", Some("parent")),
            ("width: invalid type", Some("width")),
            ("fil: not a node field; did you mean \"fill\"?", Some("fil")),
            ("runs[1].end: 9 is past the text", Some("runs[1].end")),
            ("a line needs at least 2 points, or both from and to", None),
            ("id \"a b\": use 1 to 64 letters", Some("id")),
            ("a line needs at least 2 points", None),
            ("no such node", None),
            ("", None),
        ] {
            assert_eq!(field_of(why), field, "{why}");
        }
    }
}
