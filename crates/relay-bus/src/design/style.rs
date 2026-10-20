//! Reuse (MODEL-SPEC §2.5, §2.9; feature-reuse §1): a design's named colours, which any colour
//! field of a node takes as `$name`, its named styles, which a node takes with `style`, and the
//! ids a `copy` gives what it copies.
//!
//! The types and their reading live here; the rules that set, resolve, check and detach them,
//! which the engine and the canvas share as they share `Node::patched`, are bus-styles'.

use super::{
    obj_opt, obj_vec_opt, object_only, valid_id, Blend, Gradient, Shadow, StrokeCap, StrokeJoin, TextAlign, TextCase, MAX_COORD,
    MAX_ID, MAX_NAME, MAX_SHADOWS,
};
use schemars::JsonSchema;
use serde::de::{DeserializeSeed, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
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
}
