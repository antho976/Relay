//! A design's node model (`design.*`, BUS.md §10.18): the pages, frames, shapes, text, stickies,
//! pen paths and images a design is made of, the edits `design.apply` takes, and the rules that
//! patch and validate a node. The engine stores nodes by these rules and the native client
//! applies an edit with the same ones before the engine answers, so both read a node alike.
//!
//! Geometry is in pixels. A node's `x`/`y` are relative to its parent's top-left corner (a
//! page's origin is 0,0), so moving a frame moves everything in it. Children paint in order,
//! the last on top.
//!
//! A node another build wrote is read whole: what this build cannot read waits in
//! [`Node::extra`] and is written back as it was (`read`). Which fields each kind uses, and the
//! rules a caller's new writes are held to, are in `fields`.

use schemars::JsonSchema;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

mod expect;
mod fields;
mod hints;
pub mod chart;
pub mod motion;
pub mod path_data;
mod pins;
mod read;
mod style;

pub use expect::{Conflict, ConflictReason, Expect};
pub use fields::{field_kinds, fit_written, plain_label, strip_controls, takes, Kinds, FIELDS};
pub use hints::{
    de_ops, details, explain, field_hint, field_of, label, locate, nearest, patch_schema, value_hint, Unread, FIELD_HINTS,
    KIND_HINTS, OP_HINTS, SHAPE_HINTS, VALUE_HINTS,
};
pub use motion::{Ease, Key, Motion, Pose, Timeline};
pub use pins::{follow_axis, page_point, Pin};
pub use read::{read_node, read_nodes, read_nodes_opt, write_node, write_nodes, write_nodes_opt};
pub use style::{
    color_ref, copy_id, copy_id_renamed, renamed, read_styles, read_styles_opt, write_styles, write_styles_opt, Removed, Style, StyleError, Styles,
    MAX_STYLES, STYLE_FIELDS,
};

/// Most nodes one design holds.
pub const MAX_NODES: usize = 5_000;
/// Most edits one `design.apply` carries: any list of an agent's, lenient or not.
pub const MAX_OPS: usize = 2_000;
/// Most edits in a lenient list of the person's clients: an undo re-adds every node a delete
/// took, one edit each, so it may hold a whole design's worth besides a list's.
pub const MAX_LENIENT_OPS: usize = MAX_NODES + MAX_OPS;
/// Most ancestors a node has, its page included (a page's child is 1 deep).
pub const MAX_DEPTH: usize = 64;
/// The most one design holds, in bytes, counted as `design.get` returns it (B60): each node's
/// body, id and parent and [`NODE_OVERHEAD`], and its named colours' and styles' JSON. The
/// clients' sockets cap a line at 2 MiB (RA-227): this leaves room for the design row and the
/// envelope. A list is refused for growing a design past it, never for leaving one over it
/// smaller than it found it.
pub const MAX_DESIGN_BYTES: i64 = 1536 * 1024;
/// JSON bytes a node takes in a `design.get` reply beyond its body, id and parent:
/// `"id":"",` and `"parent":"",` and the comma between nodes.
pub const NODE_OVERHEAD: i64 = 21;
/// The largest picture `design.asset.add` copies in from a path.
pub const MAX_ASSET_FILE: u64 = 20 * 1024 * 1024;
/// The largest picture `design.asset.add` takes as `bytes_b64`, decoded.
pub const MAX_ASSET_B64: usize = 1024 * 1024;
/// How many revs back `design.get` can answer what changed since.
pub const SINCE_WINDOW: i64 = 1_000;
/// The widest shadow blur a caller may write, in px: a node's own `blur` cap.
pub const MAX_SHADOW_BLUR: f64 = 200.0;
/// Most entries of a node's `shadows`.
pub const MAX_SHADOWS: usize = 8;
/// Most entries of a node's `runs`.
pub const MAX_RUNS: usize = 64;
/// Most id starts one `copy` swaps (`rename`).
pub const MAX_RENAMES: usize = 16;
/// The longest id of a chart's frame: its parts' ids are made from it.
pub const MAX_CHART_ID: usize = 44;
/// Most fields an instance's node keeps as its own (`overrides`).
pub const MAX_OVERRIDES: usize = 64;
/// What an instance's node never takes from its main nor keeps as an override: what places it
/// in the design and links it.
pub const NOT_OVERRIDDEN: &[&str] = &["id", "parent", "kind", "component", "instance_of", "overrides"];
/// Longest text a node holds, in characters.
pub const MAX_TEXT: usize = 10_000;
/// Most points one path or line holds.
pub const MAX_POINTS: usize = 4_000;
/// Largest coordinate or size, in either direction.
pub const MAX_COORD: f64 = 1_000_000.0;
/// Longest node id, name or design title.
pub const MAX_ID: usize = 64;
pub const MAX_NAME: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignNodeKind")]
pub enum NodeKind {
    /// A sheet of the design: the root its other nodes sit in. Its `fill` is the canvas colour.
    Page,
    /// An artboard: a filled box that clips what is inside it and can lay its children out
    /// (`layout`). Screens, cards and buttons are frames.
    Frame,
    /// Nodes moved together; it paints nothing itself, and its box is its children's.
    Group,
    /// A box: `fill`, a border (`stroke`), corners (`radius`) and a centred label (`text`).
    Rect,
    /// An oval filling its box (`arc` draws part of one), with a centred label (`text`).
    Ellipse,
    /// A line through `points`, or a connector between two nodes (`from`, `to`);
    /// `arrow_start`/`arrow_end` put heads on it.
    Line,
    /// Words (`text`), as wide as they are until given a width, then wrapped to it.
    Text,
    /// A note card: `fill` paper with `text` on it, growing to fit its words.
    Sticky,
    /// A freehand stroke through `points`.
    Path,
    /// A picture from `design.asset.add`, named by `src`.
    Image,
    /// A regular polygon (`sides`: 3 a triangle, 4 a diamond, 6 a hexagon…) or, with `star`, a
    /// star of `sides` points (5 when `sides` is left out). Its first point is at the top centre.
    Polygon,
    /// An interface icon from Relay's set, named by `icon` (one of the names listed for `icon`),
    /// drawn in `stroke`.
    Icon,
}

impl NodeKind {
    pub const ALL: [NodeKind; 12] = [
        NodeKind::Page,
        NodeKind::Frame,
        NodeKind::Group,
        NodeKind::Rect,
        NodeKind::Ellipse,
        NodeKind::Line,
        NodeKind::Text,
        NodeKind::Sticky,
        NodeKind::Path,
        NodeKind::Image,
        NodeKind::Polygon,
        NodeKind::Icon,
    ];

    /// Whether nodes can sit inside this one.
    pub fn holds_children(self) -> bool {
        matches!(self, NodeKind::Page | NodeKind::Frame | NodeKind::Group)
    }

    /// Its name as stored and written: `polygon`.
    pub fn name(self) -> &'static str {
        match self {
            NodeKind::Page => "page",
            NodeKind::Frame => "frame",
            NodeKind::Group => "group",
            NodeKind::Rect => "rect",
            NodeKind::Ellipse => "ellipse",
            NodeKind::Line => "line",
            NodeKind::Text => "text",
            NodeKind::Sticky => "sticky",
            NodeKind::Path => "path",
            NodeKind::Image => "image",
            NodeKind::Polygon => "polygon",
            NodeKind::Icon => "icon",
        }
    }

    /// The kind `name` names; `None` for one this build does not know (a newer build's).
    pub fn from_name(name: &str) -> Option<NodeKind> {
        NodeKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// The icons an `icon` node may name: outline icons on a 24-unit grid, the vocabulary of most
/// interfaces.
pub const ICONS: &[&str] = &[
    "home", "search", "settings", "user", "users", "bell", "mail", "heart", "star", "plus", "minus",
    "x", "check", "chevron-left", "chevron-right", "chevron-up", "chevron-down", "arrow-left",
    "arrow-right", "arrow-up", "arrow-down", "menu", "more-horizontal", "more-vertical", "trash",
    "edit", "copy", "download", "upload", "share", "link", "lock", "unlock", "eye", "eye-off",
    "calendar", "clock", "camera", "image", "file", "folder", "shopping-cart", "credit-card",
    "globe", "map-pin", "phone", "message", "send", "filter", "sliders", "grid", "list", "play",
    "pause", "refresh", "log-out", "bookmark", "info", "alert", "help", "sun", "moon", "wifi",
    "battery", "terminal", "code", "zap", "cloud", "git-branch", "bar-chart",
    // Money, places, things and more controls (2026-10-20).
    "wallet", "bank", "receipt", "tag", "coins", "banknote", "dollar-sign", "percent", "calculator",
    "trending-up", "trending-down", "pie-chart", "line-chart", "shopping-bag", "gift", "utensils",
    "coffee", "car", "bus", "plane", "fuel", "briefcase", "book", "graduation-cap", "music", "film",
    "tv", "gamepad", "dumbbell", "activity", "pill", "shirt", "scissors", "wrench", "lightbulb",
    "droplet", "flame", "leaf", "umbrella", "thermometer", "inbox", "archive", "layers",
    "external-link", "paperclip", "at-sign", "hash", "smile", "thumbs-up", "flag", "award",
    "target", "shield", "key", "mic", "video", "headphones", "smartphone", "laptop", "printer",
    "map", "navigation", "compass", "package", "truck", "store", "database", "server", "cpu",
    "rotate-ccw", "repeat", "shuffle", "skip-forward", "skip-back", "volume", "maximize",
    "minimize", "plus-circle", "check-circle", "x-circle", "log-in", "user-plus", "sparkles",
    "timer", "history",
];

/// How a connector runs between its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignRoute")]
pub enum Route {
    Straight,
    /// Right angles: across, then down, then across (or the other way, whichever is longer).
    Elbow,
    /// A smooth curve leaving and arriving along the longer axis.
    Curve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignGradientKind")]
pub enum GradientKind {
    Linear,
    /// From the centre out to the box's edges.
    Radial,
    /// Round the centre, clockwise from `angle` (default 0, the top), as CSS
    /// `conic-gradient(from …)`.
    Conic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignGradientStop")]
pub struct Stop {
    /// 0 at the start, 1 at the end.
    #[schemars(range(min = 0, max = 1))]
    pub at: f64,
    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour.
    pub color: String,
}

/// A gradient fill, painted instead of `fill`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignGradient")]
pub struct Gradient {
    #[serde(default = "linear")]
    pub kind: GradientKind,
    /// Linear: its direction in degrees as CSS reads it, 180 (the default) top to bottom, 90 left
    /// to right. Conic: where the first stop sits, in degrees clockwise from the top (default 0).
    /// Radial: unused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// 2 to 8 colours along it.
    #[serde(deserialize_with = "obj_vec")]
    #[schemars(length(min = 2, max = 8))]
    pub stops: Vec<Stop>,
}

fn linear() -> GradientKind {
    GradientKind::Linear
}

/// How a node's width or height is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignSizing")]
pub enum Sizing {
    /// `w`/`h` as given.
    Fixed,
    /// Shrink to the content: a text's or sticky's words, or a laid-out frame's children and
    /// padding.
    Hug,
    /// Stretch to the room left in a laid-out parent (along its axis; in a `wrap` layout, the
    /// room left in its line), or to its full depth (across it).
    Fill,
}

/// The direction a frame lays its children out in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignLayout")]
pub enum Layout {
    /// Children keep their own `x`/`y`.
    None,
    /// Left to right, `gap` apart.
    Row,
    /// Top to bottom, `gap` apart.
    Column,
    /// Left to right like a row, starting a new line when the next child would pass the
    /// frame's inner width; `gap` apart along a line, `row_gap` between lines. A frame that hugs
    /// its width wraps only at `max_w`.
    Wrap,
}

/// How a child follows its parent's width when the parent is resized, or drawn wider or narrower
/// than its stored `w`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignPinX")]
pub enum PinX {
    /// Keeps its distance from the parent's left edge (the default).
    Left,
    /// Keeps its distance from the parent's right edge.
    Right,
    /// Keeps both distances: its width stretches with the parent's.
    LeftRight,
    /// Keeps its offset from the parent's centre line.
    Center,
    /// Keeps its place and width as fractions of the parent's width.
    Scale,
}

/// How a child follows its parent's height, as `pin_x` does its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignPinY")]
pub enum PinY {
    /// Keeps its distance from the parent's top edge (the default).
    Top,
    /// Keeps its distance from the parent's bottom edge.
    Bottom,
    /// Keeps both distances: its height stretches with the parent's.
    TopBottom,
    /// Keeps its offset from the parent's middle.
    Center,
    /// Keeps its place and height as fractions of the parent's height.
    Scale,
}

/// Which way a frame's content scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignScroll")]
pub enum Scroll {
    /// Across: a carousel.
    X,
    /// Down: a feed.
    Y,
    Both,
}

impl Scroll {
    /// Whether it scrolls across.
    pub fn x(self) -> bool {
        matches!(self, Scroll::X | Scroll::Both)
    }

    /// Whether it scrolls down.
    pub fn y(self) -> bool {
        matches!(self, Scroll::Y | Scroll::Both)
    }

    fn name(self) -> &'static str {
        match self {
            Scroll::X => "x",
            Scroll::Y => "y",
            Scroll::Both => "both",
        }
    }
}

/// Where a laid-out frame puts its children along its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignJustify")]
pub enum Justify {
    Start,
    Center,
    End,
    /// The first at the start, the last at the end, the rest evenly between.
    SpaceBetween,
}

/// Where a laid-out frame puts its children across its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignItems")]
pub enum Items {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignTextAlign")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// How an image fills its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignImageFit")]
pub enum ImageFit {
    /// Fill the box, cropping the overflow.
    Cover,
    /// Show it whole inside the box.
    Contain,
    /// Stretch to the box.
    Stretch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignShadow")]
pub struct Shadow {
    /// Offset right, in px (default 0).
    #[serde(default)]
    #[schemars(range(min = -10_000, max = 10_000))]
    pub x: f64,
    /// Offset down, in px (default 0).
    #[serde(default)]
    #[schemars(range(min = -10_000, max = 10_000))]
    pub y: f64,
    /// Softness in px, 0 to 200 (default 0, a hard edge).
    #[serde(default)]
    #[schemars(range(min = 0, max = MAX_SHADOW_BLUR))]
    pub blur: f64,
    /// How far it reaches past the edge before it blurs, in px (default 0).
    #[serde(default)]
    #[schemars(range(min = -10_000, max = 10_000))]
    pub spread: f64,
    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour.
    pub color: String,
    /// An inner shadow, cast inside the edge instead of under the node.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inset: bool,
}

/// Part of an ellipse: a pie, a donut or a ring segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignArc")]
pub struct Arc {
    /// Where it starts, in degrees clockwise from the top (default 0), within ±3600.
    #[serde(default)]
    #[schemars(range(min = -3600, max = 3600))]
    pub start: f64,
    /// How far round it goes, in degrees clockwise (default 360, the whole ellipse), within ±360.
    #[serde(default = "full_turn")]
    #[schemars(range(min = -360, max = 360))]
    pub sweep: f64,
    /// The hole in the middle as a fraction of the radius, from 0 (the default, a pie) to below
    /// 1: 0.6 a donut.
    #[serde(default)]
    #[schemars(range(min = 0))]
    pub inner: f64,
    /// The ends of a ring segment (`inner` above 0, `sweep` under 360): `butt` (flat, the
    /// default), `round` (a progress ring's rounded ends) or `square`. Each end reaches half the
    /// ring's thickness past `start` and `start + sweep`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<StrokeCap>,
}

/// How a stroke's ends are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignStrokeCap")]
pub enum StrokeCap {
    /// Flat at the point.
    Butt,
    Round,
    /// Square, half the weight past the point.
    Square,
}

/// How a stroke turns its corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignStrokeJoin")]
pub enum StrokeJoin {
    /// Sharp.
    Miter,
    Round,
    /// Cut off.
    Bevel,
}

/// One stretch of a text in a style of its own: characters `start` to `end` of `text` (counted
/// in characters, `end` not included). A field it leaves out is the node's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignTextRun")]
pub struct TextRun {
    /// Its first character, from 0.
    pub start: u32,
    /// The character after its last: above `start`, at most the text's length.
    #[schemars(range(min = 1))]
    pub end: u32,
    /// Its letters' colour: `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Its font family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_NAME))]
    pub font: Option<String>,
    /// Its letter size in px, 1 to 1000; it also sets its line's height (`line_height` times it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub font_size: Option<f64>,
    /// 1 to 1000 (700 is bold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    /// Px above the baseline (negative below), -1000 to 1000: a superscript, a raised unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -1000, max = 1000))]
    pub rise: Option<f64>,
    /// Extra space between its letters, in px, -100 to 1000: a spaced-out label in a sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -100, max = 1000))]
    pub letter_spacing: Option<f64>,
}

impl TextRun {
    /// Whether it sets anything of its own.
    fn styled(&self) -> bool {
        self.color.is_some()
            || self.font.is_some()
            || self.font_size.is_some()
            || self.font_weight.is_some()
            || self.italic.is_some()
            || self.underline.is_some()
            || self.strike.is_some()
            || self.rise.is_some()
            || self.letter_spacing.is_some()
    }
}

fn full_turn() -> f64 {
    360.0
}

/// A nested object, read only from a JSON object. serde's derived structs also read an array,
/// field by field in order, so `"arc": [0, 90]` would quietly be a start and a sweep (TM-16).
pub(crate) fn object_only<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    struct Only<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Only<T> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }
    d.deserialize_map(Only(PhantomData))
}

/// [`object_only`] for an optional field: `null` is none.
pub(crate) fn obj_opt<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    struct Opt<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Opt<T> {
        type Value = Option<T>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object or null")
        }
        fn visit_none<E: de::Error>(self) -> Result<Option<T>, E> {
            Ok(None)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Option<T>, E> {
            Ok(None)
        }
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Option<T>, D::Error> {
            object_only(d).map(Some)
        }
    }
    d.deserialize_option(Opt(PhantomData))
}

/// [`obj_vec`] for an optional list: `null` is none.
pub(crate) fn obj_vec_opt<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Vec<T>>, D::Error> {
    struct OptVec<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for OptVec<T> {
        type Value = Option<Vec<T>>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of objects or null")
        }
        fn visit_none<E: de::Error>(self) -> Result<Option<Vec<T>>, E> {
            Ok(None)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Option<Vec<T>>, E> {
            Ok(None)
        }
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Option<Vec<T>>, D::Error> {
            obj_vec(d).map(Some)
        }
    }
    d.deserialize_option(OptVec(PhantomData))
}

/// [`object_only`] for each entry of a list.
pub(crate) fn obj_vec<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    struct Object<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for Object<T> {
        type Value = T;
        fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<T, D::Error> {
            object_only(d)
        }
    }
    struct Objects<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Objects<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of objects")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(64));
            while let Some(item) = seq.next_element_seed(Object(PhantomData))? {
                out.push(item);
            }
            Ok(out)
        }
    }
    d.deserialize_seq(Objects(PhantomData))
}

/// How a node mixes with what is under it in the same parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignBlend")]
pub enum Blend {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl Blend {
    pub const ALL: [Blend; 16] = [
        Blend::Normal,
        Blend::Multiply,
        Blend::Screen,
        Blend::Overlay,
        Blend::Darken,
        Blend::Lighten,
        Blend::ColorDodge,
        Blend::ColorBurn,
        Blend::HardLight,
        Blend::SoftLight,
        Blend::Difference,
        Blend::Exclusion,
        Blend::Hue,
        Blend::Saturation,
        Blend::Color,
        Blend::Luminosity,
    ];

    /// Its name as stored: `color_dodge`.
    pub fn name(self) -> &'static str {
        match self {
            Blend::Normal => "normal",
            Blend::Multiply => "multiply",
            Blend::Screen => "screen",
            Blend::Overlay => "overlay",
            Blend::Darken => "darken",
            Blend::Lighten => "lighten",
            Blend::ColorDodge => "color_dodge",
            Blend::ColorBurn => "color_burn",
            Blend::HardLight => "hard_light",
            Blend::SoftLight => "soft_light",
            Blend::Difference => "difference",
            Blend::Exclusion => "exclusion",
            Blend::Hue => "hue",
            Blend::Saturation => "saturation",
            Blend::Color => "color",
            Blend::Luminosity => "luminosity",
        }
    }
}

/// How a text's letters are cased when drawn; the stored words keep theirs. Upper and lower use
/// Unicode's default case mapping, one letter at a time (no language's own rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignTextCase")]
pub enum TextCase {
    Upper,
    Lower,
    /// The first letter of each word in capitals: after a space or any punctuation (`Well-Known`,
    /// `And/Or`), not after an apostrophe inside a word (`Don't`).
    Title,
}

impl TextCase {
    /// `text` as this case draws it.
    pub fn apply(self, text: &str) -> String {
        self.cased(text, None)
    }

    /// `text` as this case draws it, and for each of its characters (and one past the last) the
    /// byte where it starts in the cased text: a letter can become two (`ß` is `SS`), so styled
    /// stretches of the stored words are found again in what is drawn.
    pub fn apply_indexed(self, text: &str) -> (String, Vec<usize>) {
        let mut starts = Vec::with_capacity(text.len() + 1);
        let out = self.cased(text, Some(&mut starts));
        (out, starts)
    }

    fn cased(self, text: &str, mut starts: Option<&mut Vec<usize>>) -> String {
        let mut out = String::with_capacity(text.len());
        let mut words = WordStarts::default();
        for c in text.chars() {
            if let Some(starts) = starts.as_deref_mut() {
                starts.push(out.len());
            }
            match self {
                TextCase::Upper => out.extend(c.to_uppercase()),
                TextCase::Lower => out.extend(c.to_lowercase()),
                TextCase::Title if words.starts(c) => title_case(c, &mut out),
                TextCase::Title => out.push(c),
            }
        }
        if let Some(starts) = starts {
            starts.push(out.len());
        }
        out
    }
}

/// Where a word starts, a character at a time, as the canvas's text editor (Pango's word starts)
/// sees it: at a letter or digit after the start of the text, a space or any punctuation, but
/// not after an apostrophe between two letters (`don't`, `rock'n'roll`).
#[derive(Default)]
struct WordStarts {
    /// The last character was a letter or digit.
    in_word: bool,
    /// The last character was an apostrophe right after one.
    apostrophe: bool,
}

impl WordStarts {
    fn starts(&mut self, c: char) -> bool {
        let start = c.is_alphanumeric() && !self.in_word && !self.apostrophe;
        (self.in_word, self.apostrophe) = match c {
            c if c.is_alphanumeric() => (true, false),
            '\'' | '\u{2019}' => (false, self.in_word),
            _ => (false, false),
        };
        start
    }
}

/// The character indices where a title-cased word starts in `text` ([`TextCase::Title`]'s rule),
/// so an editor showing the stored words can capitalise the same letters.
pub fn title_starts(text: &str) -> Vec<usize> {
    let mut words = WordStarts::default();
    text.chars().enumerate().filter_map(|(i, c)| words.starts(c).then_some(i)).collect()
}

/// `c` as the first letter of a word: the titlecase digraphs (`ǅ`, `ǈ`, `ǋ`, `ǲ`) for their
/// letters, as GLib's `g_unichar_totitle`, else its capital.
fn title_case(c: char, out: &mut String) {
    match c {
        '\u{1C4}'..='\u{1C6}' => out.push('\u{1C5}'),
        '\u{1C7}'..='\u{1C9}' => out.push('\u{1C8}'),
        '\u{1CA}'..='\u{1CC}' => out.push('\u{1CB}'),
        '\u{1F1}'..='\u{1F3}' => out.push('\u{1F2}'),
        _ => out.extend(c.to_uppercase()),
    }
}

/// One node of a design. Every field but `kind` is optional on input; an absent style field
/// takes the kind's default (a white frame, a grey rect, Geist text at 16px, dark on a light
/// ground and light on a dark one, a yellow sticky).
//
// Unknown keys are not a serde error (they land in `extra`) so that a body a newer build wrote
// still reads; the tool schema still says `additionalProperties: false`, and a caller's add or
// update naming one is refused by `Node::check_new` / `Node::check_update`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[schemars(rename = "DesignNode", transform = size_aliases)]
pub struct Node {
    /// Unique in the design: letters, digits, `-`, `_`, `.` or `:`, at most 64. Give your own
    /// (`"hero"`, `"hero-title"`) to refer to it later in the same `design.apply`; left out,
    /// the engine assigns one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[schemars(length(max = MAX_ID), pattern(ID_PATTERN))]
    pub id: String,
    /// The page, frame or group this node sits in. Required for everything but a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub parent: Option<String>,
    /// What the node is; it decides which fields it draws (see each kind). Fixed once added:
    /// delete it and add another to change it.
    pub kind: NodeKind,
    /// Shown in the layer list and above a frame on the canvas; at most 200 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = MAX_NAME))]
    pub name: Option<String>,
    /// Left edge in px, relative to the parent's top-left corner, within ±1000000.
    #[serde(default)]
    #[schemars(range(min = -MAX_COORD, max = MAX_COORD))]
    pub x: f64,
    /// Top edge in px, relative to the parent's top-left corner, within ±1000000.
    #[serde(default)]
    #[schemars(range(min = -MAX_COORD, max = MAX_COORD))]
    pub y: f64,
    /// Width in px, 0 to 1000000 (`width` is read as `w`). A text with no `w` hugs its words; a
    /// rect, ellipse, polygon or frame added without one is 100 wide, an icon 24 (within
    /// `min_w`/`max_w`); an image needs the `w` `design.asset.add` answered; a line's follows its
    /// points. In a laid-out frame `w_sizing` hug or fill replaces it.
    #[serde(default, alias = "width")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub w: f64,
    /// Height in px, 0 to 1000000 (`height` is read as `h`). A text hugs its lines whatever `h`
    /// says, unless `h_sizing` is fixed; a sticky grows past it to fit its words. A shape, frame
    /// or icon added without one gets a default as with `w`, and an image needs its `h`.
    #[serde(default, alias = "height")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub h: f64,
    /// Background: `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour the design names (a
    /// `color` edit names one). Default: a frame `#FFFFFF`, a rect, ellipse or polygon
    /// `#D9D9D9`, a sticky `#FFE58A`, a page `#1E1D1B` (the canvas colour); a text, line or path
    /// paints none unless given one (a text's is a box behind its words, a line's or path's of
    /// three points or more fills inside them). A text's letters are `color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Outline colour: `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour. A box's border
    /// (frame, rect, sticky, image) and an ellipse's or polygon's outline are drawn only when set,
    /// inside the edge; a line or path defaults to `#8C8C8C`; an icon is drawn in it (default: its
    /// `color`, else dark on a light ground and light on a dark one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// In px, 0 to 1000. Default 1 for a border or outline, 2 for a line, path or icon (an
    /// icon's is in its 24-unit grid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1000))]
    pub stroke_width: Option<f64>,
    /// Corner radius of a frame, rect, sticky or image, in px (default 0; `radii` sets each
    /// corner).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub radius: Option<f64>,
    /// 0 to 1 (default 1), of the node and everything in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1))]
    pub opacity: Option<f64>,
    /// A shadow under a frame, rect, ellipse, polygon, sticky or image, or with `inset` inside its
    /// edge; a text, line, path or icon casts an outer one only, without spread (a glow when `x`
    /// and `y` are 0). A sticky casts a soft one of its own unless given one. `x`, `y` and `spread`
    /// within ±10000 px, `blur` 0 to 200. One shadow; `shadows` holds several (give one field or
    /// the other).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub shadow: Option<Shadow>,
    /// Whether a frame hides what overflows it (default true; a scrolling frame always does).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<bool>,
    /// Not drawn, not hit and left out of auto layout, with everything inside it (default
    /// false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Locked nodes cannot be selected or moved on the canvas (default false); edits still change
    /// them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    /// Leaves it out of the layout and contrast checks (default false): design.render lists no
    /// issue, and design.apply no contrast flag, that names it first. For words meant to be faint,
    /// or set on what the check cannot see (a picture's colours); not to quiet a real problem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_checks: Option<bool>,

    /// The words of a text or sticky, or a centred label on a rect, ellipse or polygon; at most
    /// 10000 characters. Newlines break lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = MAX_TEXT))]
    pub text: Option<String>,
    /// Colour of the letters of a text, sticky or label, and of an icon without a `stroke`:
    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA`, or a `$name` colour (default: dark on a light ground,
    /// light on a dark one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Font family of a text, sticky or label (default `Geist`; `Geist Mono` and `Sora` are
    /// always there; `design.fonts` lists the rest). A family the desktop app lacks is drawn in
    /// a fallback face, and the reply warns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_NAME))]
    pub font: Option<String>,
    /// Letter size of a text, sticky or label in px, 1 to 1000 (default 16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub font_size: Option<f64>,
    /// 1 to 1000 (default 400; 600 is semibold, 700 bold); the nearest face the family has is
    /// drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub font_weight: Option<u16>,
    /// A text's or label's letters slanted (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// A text's or label's letters underlined (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    /// A line through a text's or label's letters (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    /// A text's lines across its box: `left` (the default; `center` for a label), `center` or
    /// `right`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<TextAlign>,
    /// Line height as a multiple of the font size, 0.1 to 10 (default 1.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.1, max = 10))]
    pub line_height: Option<f64>,
    /// Extra space between letters, in px, -100 to 1000 (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -100, max = 1000))]
    pub letter_spacing: Option<f64>,

    /// Width rule (default `fixed`: its `w`; a text without a `w` hugs its words). `hug`
    /// shrinks a text or sticky to its words and a laid-out frame to its children (nothing else
    /// hugs); `fill` takes the room a laid-out parent leaves along its direction, or its inner
    /// width across it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w_sizing: Option<Sizing>,
    /// Height rule (default `fixed`; a text hugs its lines). A sticky's fixed `h` is a minimum
    /// that grows to fit its words, and `hug` makes it exactly as tall as they are, as a text's
    /// (only a text, a sticky or a frame hugs).
    /// `fill` takes the room a laid-out parent leaves along its direction, or its inner height
    /// across it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_sizing: Option<Sizing>,
    /// A frame's auto layout: `row` or `column` places its children one after another, `gap`
    /// apart inside `padding`, ignoring their `x`/`y` (an `absolute` child keeps its own); `wrap`
    /// is a row that starts a new line when the next child would pass the frame's width. Default
    /// `none`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Space between laid-out children along a row, column or wrap line, in px, -10000 to 10000
    /// (default 0); negative overlaps them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -10_000, max = 10_000))]
    pub gap: Option<f64>,
    /// Inner space in px, each 0 to 10000, in CSS order: `[all]`, `[vertical, horizontal]` or
    /// `[top, right, bottom, left]`. A laid-out frame's around its children, a rect's,
    /// ellipse's or polygon's around its label (default `[0, 12]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 4), inner(range(min = 0, max = 10_000)))]
    pub padding: Option<Vec<f64>>,
    /// Where a laid-out frame puts its children along its direction (default `start`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justify: Option<Justify>,
    /// Where a laid-out frame puts its children across it (default `start`; `stretch` gives each
    /// the frame's inner depth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Items>,

    /// A line's or path's points in px, relative to the node's `x`/`y`: at most 4000, each
    /// within ±1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = MAX_POINTS), inner(inner(range(min = -MAX_COORD, max = MAX_COORD))))]
    pub points: Option<Vec<[f64; 2]>>,
    /// A head at a line's or path's first point (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_start: Option<bool>,
    /// A head at a line's or path's last point (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_end: Option<bool>,

    /// An image's picture: the `src` `design.asset.add` returned. It must be stored
    /// (`design.asset_not_found` otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// How an image fills its box (default `cover`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<ImageFit>,

    /// Clockwise turn in degrees about the node's centre, within ±3600.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -3600, max = 3600))]
    pub rotation: Option<f64>,
    /// A polygon's corners, or a star's points: 3 to 24 (default 3, or 5 points for a star).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 3, max = 24))]
    pub sides: Option<u8>,
    /// Makes a polygon a star: its inner points at this fraction of the outer (0.05 to 0.95;
    /// 0.4 is a classic star).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.05, max = 0.95))]
    pub star: Option<f64>,
    /// A gradient painted instead of `fill`.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub gradient: Option<Gradient>,
    /// Corner radii in px, top-left, top-right, bottom-right, bottom-left, instead of `radius`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(inner(range(min = 0, max = MAX_COORD)))]
    pub radii: Option<[f64; 4]>,
    /// The stroke's dash pattern in px, 1 to 8 lengths from 0 to 1000, not all 0: `[6, 4]`
    /// dashes, `[0, 4]` round dots. Absent is solid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 8), inner(range(min = 0, max = 1000)))]
    pub dash: Option<Vec<f64>>,
    /// Blur of the whole node, in px, 0 to 200.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 200))]
    pub blur: Option<f64>,
    /// An icon node's icon, one of these outline icons or one saved to Relay's set (design.icon.list,
    /// which every project's designs share), drawn on a 24-unit grid scaled to the smaller of `w`
    /// and `h` (24 is its natural size).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "icon_schema")]
    pub icon: Option<String>,
    /// An icon's own outline, drawn instead of `icon`'s: SVG path data on the 24-unit grid
    /// (`M4 6h16M4 12h16`), stroked as Relay's icons are, at most 8000 characters. `icon` naming
    /// an icon saved to Relay's set (design.icon.save) fills it in; naming a built-in one clears
    /// it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = path_data::MAX_PATH_DATA))]
    pub icon_path: Option<String>,
    /// A connector's start: a node on the same page, not the line itself or a page. The line
    /// then runs from that node's edge and follows it when it moves; `points` are only where it
    /// was last drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub from: Option<String>,
    /// A connector's end: a node on the same page, not the line itself or a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub to: Option<String>,
    /// How a line runs between its ends (default `straight`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// Joins a line's or path's last point back to its first: a shape of your own, which
    /// `fill` paints inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    /// Runs a line through its `points` in a smooth curve instead of straight segments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    /// Mirrors a polygon, image or icon left to right. Other kinds ignore it: a line or path is
    /// flipped by its points, a box by where it sits; text is never mirrored, and a flipped
    /// frame's words still read left to right.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_x: Option<bool>,
    /// Mirrors a polygon, image or icon top to bottom (other kinds as `flip_x`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_y: Option<bool>,
    /// Draws only part of an ellipse: a pie slice, a donut or an arc of a ring.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub arc: Option<Arc>,
    /// How the node mixes with what is under it in its parent (default `normal`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Blend>,
    /// Draws a text's, sticky's or label's letters upper, lower or title case; the stored words
    /// keep theirs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_case: Option<TextCase>,

    /// How it follows its parent's width: `left` (the default), `right`, `left_right` (its width
    /// stretches), `center` or `scale`. Its `x`/`w` are where it sits while the parent is at its
    /// stored `w`; a parent resized, or drawn wider or narrower (fill, stretch, hug), carries it
    /// along. Only a child free in a frame (one without auto layout, or `absolute`) is pinned. A
    /// stretching pin needs a fixed width (a text, a `w`); a line, path or group cannot stretch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_x: Option<PinX>,
    /// How it follows its parent's height: `top` (the default), `bottom`, `top_bottom` (its
    /// height stretches), `center` or `scale`, as `pin_x` (a text needs `h_sizing: fixed` to
    /// stretch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_y: Option<PinY>,
    /// In a frame with auto layout: keeps its own `x`/`y` from the frame's top-left, takes no
    /// room and is not counted when the frame hugs (a badge on a card); pin it to stick to an
    /// edge. Elsewhere it changes nothing. Default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute: Option<bool>,
    /// Smallest width in px, 0 to 1000000, whatever decides it (hug, fill, stretch, a pin). A
    /// fixed `w` must lie between `min_w` and `max_w`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub min_w: Option<f64>,
    /// Largest width in px, 0 to 1000000, whatever decides it; a hugging text wraps at it, and so
    /// does a hugging `wrap` frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub max_w: Option<f64>,
    /// Smallest height in px, 0 to 1000000, whatever decides it. A fixed `h` must lie between
    /// `min_h` and `max_h`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub min_h: Option<f64>,
    /// Largest height in px, 0 to 1000000, whatever decides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = MAX_COORD))]
    pub max_h: Option<f64>,
    /// Space between the lines of a `wrap` layout, in px, -10000 to 10000 (default: `gap`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -10_000, max = 10_000))]
    pub row_gap: Option<f64>,
    /// A frame whose content scrolls that way (`x`, `y` or `both`): what overflows is clipped, a
    /// still picture shows the start, and the canvas scrolls it with Alt+wheel. Children keep
    /// their sizes, so a row of fixed-width cards overflows into a carousel. The scrolling axis
    /// needs a fixed or fill size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll: Option<Scroll>,

    /// How a stroke's ends are drawn: `butt` (flat at the point), `round` or `square` (half the
    /// weight past it). Default: `round` for a solid line or path, `butt` for dashes, `round`
    /// for dots (`butt` would draw nothing of a dot). On a closed outline (a box, an ellipse, a
    /// polygon) only its dashes have ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_cap: Option<StrokeCap>,
    /// How the stroke of an ellipse, polygon, line or path turns its corners: `miter` (sharp),
    /// `round` (the default) or `bevel`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_join: Option<StrokeJoin>,
    /// A gradient painted along the stroke instead of `stroke`'s colour, over the stroke's own
    /// bounds (a box's border, a line's path); it draws the stroke even when `stroke` is unset.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub stroke_gradient: Option<Gradient>,
    /// Several shadows instead of `shadow`, 1 to 8, painted in order (the last on top). A glow
    /// is a shadow with no offset: `{"blur": 16, "color": "#4AF0FF99"}`. A text, line, path or
    /// icon casts outer ones only, without spread. Give `shadow` or `shadows`, not both.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_vec_opt")]
    #[schemars(length(min = 1, max = MAX_SHADOWS))]
    pub shadows: Option<Vec<Shadow>>,
    /// Parts of `text` styled on their own, 1 to 64, in order and not overlapping, counted in
    /// characters (`end` not included); a field a run leaves out is the node's:
    /// `"text": "8.4 km", "runs": [{"start": 3, "end": 6, "font_size": 14, "color": "#8C877F"}]`.
    /// A change of `text` sends `runs` (or `null`) with it.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_vec_opt")]
    #[schemars(length(min = 1, max = MAX_RUNS))]
    pub runs: Option<Vec<TextRun>>,
    /// Most lines shown, 1 to 1000; the rest is cut and the last line shown ends in "…". The
    /// words need a width to wrap at: a sticky, a shape's label, or a text whose width is fixed
    /// or fills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 1000))]
    pub max_lines: Option<u16>,
    /// A named style of the design (a `style` edit makes one; `design.get` lists them): each of
    /// its fields this node does not set itself applies to it, so changing the style restyles
    /// every node that names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub style: Option<String>,
    /// Makes this node and what is inside it a main component (any node but a page): a `copy`
    /// with `instance: true` makes an instance of it, and every change to it reaches every
    /// instance once its list has run, except the fields an instance overrides. Set by that copy
    /// when it is not set already; `null` makes it a plain node again and detaches its instances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<bool>,
    /// Set by the engine on every node of an instance: the id of the main component's node it
    /// follows. Not written by hand, but `null` on an instance's root detaches it, leaving plain
    /// nodes that look as they did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub instance_of: Option<String>,
    /// On an instance's node: the fields it keeps as its own while the main changes, at most 64.
    /// An update of an instance's node adds each field it sets; give `overrides` itself to say
    /// which (`[]` or `null` puts every field back to the main's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = MAX_OVERRIDES))]
    pub overrides: Option<Vec<String>>,
    /// A chart this frame (without `layout`) draws as its `generated` children, redrawn when it or
    /// the frame's size changes.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub chart: Option<Box<chart::Chart>>,
    /// Set on a chart's parts, which are edited through the chart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated: Option<bool>,
    /// How it moves when the design plays (any node but a page): 2 to 64 keys in time order on the
    /// clock of the nearest frame or page with a `timeline` (itself included), else its page's. Its own fields are its
    /// look at rest, which whatever does not play shows (the canvas until Play, exports,
    /// `design.render` without `at_ms`). `dx`, `dy`, `rotate` and the scales move it from where
    /// layout puts it; `opacity`, `fill`, `stroke`, `color`, `blur`, `sweep`, `angle` and `trim`
    /// show their value instead of its own. Times are milliseconds:
    /// `{"ease": "ease_out", "keys": [{"at_ms": 0, "dy": 24, "opacity": 0}, {"at_ms": 500, "dy": 0, "opacity": 1}]}`.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub motion: Option<Motion>,
    /// A frame's or page's clock for the motion inside it: `{"duration_ms": 3000, "loop": true}`
    /// (default: as long as the latest key, looping). A nested timeline keeps its own time.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "obj_opt")]
    pub timeline: Option<Timeline>,

    /// What this build cannot read in a node another build wrote: a key it does not know, or a
    /// known one whose value it cannot read (a newer gradient kind). Kept and written back as it
    /// was by [`Node::to_map`]; never set by this build. The derived `Serialize` leaves it out,
    /// so no key is written twice.
    #[serde(flatten, skip_serializing)]
    #[schemars(skip)]
    pub extra: Map<String, Value>,
}

/// `icon`'s schema: the names of [`ICONS`], so the list an agent reads is whole and stays in step
/// with what validates.
fn icon_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let names: Vec<Value> = ICONS.iter().map(|name| Value::from(*name)).collect();
    schemars::json_schema!({"anyOf": [
        {"type": "string", "enum": names},
        {"type": "string", "pattern": ID_PATTERN, "maxLength": MAX_ID, "description": "An icon saved to Relay's set: design.icon.list names them."},
        {"type": "null"},
    ]})
}

/// The node schema also names `width` and `height`, which an add reads as `w` and `h`, so a
/// client that holds a node to its schema lets them through.
fn size_aliases(schema: &mut schemars::Schema) {
    let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else { return };
    for (alias, field) in [("width", "w"), ("height", "h")] {
        let range = properties.get(field).cloned().unwrap_or_default();
        properties.insert(alias.into(), serde_json::json!({
            "type": "number",
            "minimum": range.get("minimum"),
            "maximum": range.get("maximum"),
            "description": format!("`{field}` by another name: give one or the other."),
        }));
    }
}

/// One edit of a design. `design.apply` runs a list of them in order, all or none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "DesignNodeOp")]
// Most edits of a list are adds, and an undo's are re-adds of whole subtrees: boxing the node
// would cost an allocation per edit to save space only in the rarer ones.
#[allow(clippy::large_enum_variant)]
pub enum NodeOp {
    /// Add `node` (its `parent` is a page, frame or group already there or added earlier in
    /// the same list).
    Add {
        // Read and written whole: an undo re-adds a stored node, `extra` included.
        #[serde(serialize_with = "write_node", deserialize_with = "read_node")]
        node: Node,
        /// Its place among the parent's children, 0 at the bottom; absent puts it on top.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
    },
    /// Change some fields of a node: each key of `set` is a node field other than `id`, `kind`
    /// and `parent` (use `move`); `null` puts a field back to its default.
    Update {
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        id: String,
        #[schemars(schema_with = "patch_schema")]
        set: Map<String, Value>,
    },
    /// Put a node in another parent, or at another place among its siblings.
    Move {
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        id: String,
        /// The new parent; absent keeps the current one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        parent: Option<String>,
        /// 0 at the bottom; absent puts it on top.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
        /// Keep it where it is on the page: its `x`/`y` are rewritten for the new parent from the
        /// stored offsets up both parents' chains. Refused (`design.position`) when it or a parent
        /// on either side is placed by auto layout, since that place is not stored. Default false:
        /// `x`/`y` stay as they are, relative to the new parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_position: Option<bool>,
    },
    /// Delete a node and everything inside it. A design keeps at least one page.
    Delete {
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        id: String,
    },
    /// Copy a node and everything inside it, as one edit (a page copies as a new page). The
    /// copy's id is `as` (left out, the engine assigns one); each node inside takes an id made
    /// from it: one starting with the copied node's id and `-`, `_`, `.` or `:` has that start
    /// swapped for the copy's (`card-title` becomes `card-2-title`), any other gets the copy's id
    /// and `:` in front (`text-14` becomes `card-2:text-14`), so later edits of the same list can
    /// name them; `rename` picks them yourself. A refusal of a later edit that names an id the
    /// copy did not make lists those it did. A connector inside joins the copies of its ends;
    /// one attached to a node outside the copy lets go of that end, staying where it is drawn.
    Copy {
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        id: String,
        /// The copy's id; left out, the engine assigns one.
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        as_id: Option<String>,
        /// The page, frame or group to put the copy in; absent, the original's. A page's copy
        /// takes none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        parent: Option<String>,
        /// Its place among the parent's children, 0 at the bottom; absent puts it on top.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
        /// Added to the copy's `x`, in px (default 0).
        #[serde(default, skip_serializing_if = "is_zero")]
        #[schemars(range(min = -MAX_COORD, max = MAX_COORD))]
        dx: f64,
        /// Added to the copy's `y`, in px (default 0).
        #[serde(default, skip_serializing_if = "is_zero")]
        #[schemars(range(min = -MAX_COORD, max = MAX_COORD))]
        dy: f64,
        /// Id starts to swap, 1 to 16: each node copied whose id starts with a key takes its
        /// value there instead (the longest key that fits), whatever the rule above would give;
        /// `{"v2-": "v3-"}` copies `v2-phone` as `v3-phone` (when `as` is left out) and `v2-fade`
        /// inside it as `v3-fade`. Ids no key starts keep the rule above.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(min = 1, max = MAX_RENAMES))]
        rename: Option<BTreeMap<String, String>>,
        /// Make the copy an instance of the node copied, which becomes a main component (its
        /// `component` set) if it is not one: it then follows every change to the main, but for
        /// the fields an edit of the instance sets (`overrides`). Instances inside the main are
        /// carried in. Of an instance, the copy is another instance of its main either way.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance: Option<bool>,
    },
    /// Name a colour that any colour field of a node, or of a style, takes as `$name`. Naming it
    /// again changes it wherever it is used.
    Color {
        /// 1 to 64 letters, digits, `-`, `_`, `.` or `:`; one namespace with the styles.
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        name: String,
        /// `#RGB`, `#RRGGBB` or `#RRGGBBAA` (not another `$name`).
        value: String,
    },
    /// Name a style, or change one: each key of `set` is a style field (fill, gradient, stroke,
    /// stroke_width, dash, radius, radii, shadow, shadows, opacity, blur, blend, color, font,
    /// font_size, font_weight, italic, underline, strike, align, line_height, letter_spacing,
    /// text_case, stroke_cap, stroke_join, stroke_gradient) with a value as a node takes it;
    /// `null` takes it out of the style. A node names it with `style`.
    Style {
        /// 1 to 64 letters, digits, `-`, `_`, `.` or `:`; one namespace with the colours.
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        name: String,
        #[schemars(schema_with = "style::style_set_schema")]
        set: Map<String, Value>,
    },
    /// Take a named colour or style out of the design. What used it keeps its look: the colour,
    /// or the style's fields a node did not set itself, are written into it.
    DeleteStyle {
        #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
        name: String,
    },
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl NodeOp {
    /// What the edit names: a node (empty for an `add` whose node has no id yet), or a colour's
    /// or style's name.
    pub fn target(&self) -> &str {
        match self {
            NodeOp::Add { node, .. } => &node.id,
            NodeOp::Update { id, .. } | NodeOp::Move { id, .. } | NodeOp::Delete { id } | NodeOp::Copy { id, .. } => id,
            NodeOp::Color { name, .. } | NodeOp::Style { name, .. } | NodeOp::DeleteStyle { name } => name,
        }
    }

    /// The node the edit acts on, as [`NodeOp::target`] names it; `None` for an edit of a colour
    /// or a style.
    pub fn node_target(&self) -> Option<&str> {
        match self {
            NodeOp::Color { .. } | NodeOp::Style { .. } | NodeOp::DeleteStyle { .. } => None,
            _ => Some(self.target()),
        }
    }

    /// The edit's name as written: `delete_style`.
    pub fn name(&self) -> &'static str {
        match self {
            NodeOp::Add { .. } => "add",
            NodeOp::Update { .. } => "update",
            NodeOp::Move { .. } => "move",
            NodeOp::Delete { .. } => "delete",
            NodeOp::Copy { .. } => "copy",
            NodeOp::Color { .. } => "color",
            NodeOp::Style { .. } => "style",
            NodeOp::DeleteStyle { .. } => "delete_style",
        }
    }
}

/// The field a patch key names: `width` and `height` are `w` and `h`.
pub(crate) fn canonical_key(key: &str) -> &str {
    match key {
        "width" => "w",
        "height" => "h",
        other => other,
    }
}

/// A node id's characters, as the schema states them ([`valid_id`] also holds it to 64).
pub const ID_PATTERN: &str = "^[A-Za-z0-9_.:-]+$";

/// A node id: 1 to 64 of letters, digits, `-`, `_`, `.` and `:`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

/// `#RGB`, `#RRGGBB` or `#RRGGBBAA` as red, green, blue and alpha bytes.
pub fn parse_color(text: &str) -> Option<[u8; 4]> {
    let hex = text.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        3 => {
            let nib = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok().map(|n| n * 17);
            Some([nib(0)?, nib(1)?, nib(2)?, 255])
        }
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

impl Node {
    pub fn new(kind: NodeKind) -> Self {
        Node {
            id: String::new(),
            parent: None,
            kind,
            name: None,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            fill: None,
            stroke: None,
            stroke_width: None,
            radius: None,
            opacity: None,
            shadow: None,
            clip: None,
            hidden: None,
            locked: None,
            text: None,
            color: None,
            font: None,
            font_size: None,
            font_weight: None,
            italic: None,
            underline: None,
            strike: None,
            align: None,
            line_height: None,
            letter_spacing: None,
            w_sizing: None,
            h_sizing: None,
            layout: None,
            gap: None,
            padding: None,
            justify: None,
            items: None,
            points: None,
            arrow_start: None,
            arrow_end: None,
            src: None,
            fit: None,
            rotation: None,
            sides: None,
            star: None,
            gradient: None,
            radii: None,
            dash: None,
            blur: None,
            icon: None,
            from: None,
            to: None,
            route: None,
            closed: None,
            smooth: None,
            flip_x: None,
            flip_y: None,
            arc: None,
            blend: None,
            text_case: None,
            pin_x: None,
            pin_y: None,
            absolute: None,
            min_w: None,
            max_w: None,
            min_h: None,
            max_h: None,
            row_gap: None,
            scroll: None,
            stroke_cap: None,
            stroke_join: None,
            stroke_gradient: None,
            shadows: None,
            runs: None,
            max_lines: None,
            skip_checks: None,
            icon_path: None,
            component: None,
            instance_of: None,
            overrides: None,
            chart: None,
            generated: None,
            style: None,
            motion: None,
            timeline: None,
            extra: Map::new(),
        }
    }

    /// This node with `set` applied (`NodeOp::Update`), validated. The error names the field.
    /// `width` and `height` are read as `w` and `h`, as an `add` reads them; a `set` naming both
    /// spellings of one is refused (B71). A key no field reads lands in `extra`, which
    /// [`Node::check_update`] refuses in a strict list; a `null` removes a field or such a key.
    pub fn patched(&self, set: &Map<String, Value>) -> Result<Node, String> {
        for (field, alias) in [("w", "width"), ("h", "height")] {
            if set.contains_key(field) && set.contains_key(alias) {
                return Err(format!("{field}: give {field} or {alias}, not both"));
            }
        }
        let mut fields = self.to_map();
        for (key, value) in set {
            let key = canonical_key(key);
            match key {
                "id" | "kind" | "parent" => {
                    return Err(format!("{key} cannot change in an update; {}", match key {
                        "parent" => "use a move",
                        _ => "delete the node and add another",
                    }))
                }
                _ if value.is_null() => {
                    fields.remove(key);
                }
                _ => {
                    fields.insert(key.to_string(), value.clone());
                }
            }
        }
        let node = Node::read(Value::Object(fields))?;
        node.validate()?;
        Ok(node)
    }

    /// The fields `set` would change, at their current values (`null` where absent), `extra`
    /// keys included: the update that undoes it.
    pub fn before(&self, set: &Map<String, Value>) -> Map<String, Value> {
        let current = self.to_map();
        set.keys()
            .map(|key| canonical_key(key))
            .map(|key| (key.to_string(), current.get(key).cloned().unwrap_or(Value::Null)))
            .collect()
    }

    /// How its width is decided: `w_sizing`, or by default `fixed` (a text without a width
    /// hugs its words).
    pub fn w_rule(&self) -> Sizing {
        self.w_sizing.unwrap_or(if self.kind == NodeKind::Text && self.w <= 0.0 { Sizing::Hug } else { Sizing::Fixed })
    }

    /// How its height is decided: `h_sizing`, or by default `fixed` (a text hugs its lines).
    pub fn h_rule(&self) -> Sizing {
        self.h_sizing.unwrap_or(if self.kind == NodeKind::Text { Sizing::Hug } else { Sizing::Fixed })
    }

    /// Give a shape, frame or icon added without a size one (B49): 100 for a rect, ellipse,
    /// polygon or frame, 24 for an icon, within its `min_w`/`max_w` (`min_h`/`max_h`), on each
    /// axis that is 0 and `fixed`. An axis that fills or hugs keeps its 0. Whether anything
    /// changed.
    pub fn fill_default_size(&mut self) -> bool {
        let size = match self.kind {
            NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Frame => 100.0,
            NodeKind::Icon => 24.0,
            _ => return false,
        };
        let w = self.w == 0.0 && self.w_rule() == Sizing::Fixed;
        let h = self.h == 0.0 && self.h_rule() == Sizing::Fixed;
        if w {
            self.w = self.clamp_w(size);
        }
        if h {
            self.h = self.clamp_h(size);
        }
        w || h
    }

    /// Fit a free line's or path's box to its points (B3): the points start at 0,0, `w`/`h`
    /// are their extent, and `x`/`y` move so the drawing stays where it was, turned or not (a
    /// turn is about the box's centre, which the fit moves). A connector (`from`, `to` or
    /// `route`) is drawn from its ends and keeps its box. Whether anything changed.
    pub fn contain_points(&mut self) -> bool {
        let free = match self.kind {
            NodeKind::Path => true,
            NodeKind::Line => self.from.is_none() && self.to.is_none() && self.route.is_none(),
            _ => false,
        };
        let Some(points) = self.points.as_mut().filter(|p| free && !p.is_empty()) else {
            return false;
        };
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in points.iter() {
            for a in 0..2 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        let e = [hi[0] - lo[0], hi[1] - lo[1]];
        if lo == [0.0, 0.0] && e == [self.w, self.h] {
            return false;
        }
        // The new origin is the old plus m + (R − I)·k, with m the points' least corner and k
        // their centre less the old box's centre (MODEL-SPEC §2.8).
        let k = [lo[0] + e[0] / 2.0 - self.w / 2.0, lo[1] + e[1] / 2.0 - self.h / 2.0];
        let (s, c) = self.rotation.unwrap_or(0.0).to_radians().sin_cos();
        // Rounded to a billionth of a pixel: a quarter turn's sine and cosine are not exactly 0
        // and 1, and would leave noise like -3.6e-15 in what is stored.
        let tidy = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
        self.x = tidy(self.x + lo[0] + (c * k[0] - s * k[1]) - k[0]);
        self.y = tidy(self.y + lo[1] + (s * k[0] + c * k[1]) - k[1]);
        for p in points.iter_mut() {
            p[0] -= lo[0];
            p[1] -= lo[1];
        }
        (self.w, self.h) = (e[0], e[1]);
        true
    }

    /// Everything about one node that holds whatever its parent: ranges, colours, sizes. That
    /// the parent exists and can hold children is the store's to check.
    pub fn validate(&self) -> Result<(), String> {
        if !self.id.is_empty() && !valid_id(&self.id) {
            return Err(format!("id {:?}: use 1 to 64 letters, digits, '-', '_', '.' or ':'", self.id));
        }
        match (self.kind, &self.parent) {
            (NodeKind::Page, Some(_)) => return Err("a page has no parent".into()),
            (NodeKind::Page, None) => {}
            (_, None) => return Err(format!("a {:?} needs a parent (a page, frame or group)", self.kind).to_lowercase()),
            (_, Some(parent)) if !valid_id(parent) => return Err(format!("parent {parent:?} is not a node id")),
            _ => {}
        }
        if self.name.as_ref().is_some_and(|n| n.chars().count() > MAX_NAME) {
            return Err(format!("name: at most {MAX_NAME} characters"));
        }
        for (field, v) in [("x", self.x), ("y", self.y)] {
            if !v.is_finite() || v.abs() > MAX_COORD {
                return Err(format!("{field}: a number within ±{MAX_COORD}"));
            }
        }
        for (field, v) in [("w", self.w), ("h", self.h)] {
            if !v.is_finite() || !(0.0..=MAX_COORD).contains(&v) {
                return Err(format!("{field}: a number from 0 to {MAX_COORD}"));
            }
        }
        for (field, color) in [("fill", &self.fill), ("stroke", &self.stroke), ("color", &self.color)] {
            if let Some(c) = color.as_deref().filter(|c| !colour_ok(c)) {
                return Err(not_a_colour(field, c));
            }
        }
        if let Some(shadow) = &self.shadow {
            check_shadow("shadow", shadow)?;
        }
        if let Some(shadows) = &self.shadows {
            if !(1..=MAX_SHADOWS).contains(&shadows.len()) {
                return Err(format!("shadows: 1 to {MAX_SHADOWS} of them; null for none"));
            }
            for (i, shadow) in shadows.iter().enumerate() {
                check_shadow(&format!("shadows[{i}]"), shadow)?;
            }
        }
        let ranged = [
            ("stroke_width", self.stroke_width, 0.0, 1_000.0),
            ("radius", self.radius, 0.0, MAX_COORD),
            ("opacity", self.opacity, 0.0, 1.0),
            ("font_size", self.font_size, 1.0, 1_000.0),
            ("line_height", self.line_height, 0.1, 10.0),
            ("letter_spacing", self.letter_spacing, -100.0, 1_000.0),
            ("gap", self.gap, -10_000.0, 10_000.0),
            ("row_gap", self.row_gap, -10_000.0, 10_000.0),
            ("min_w", self.min_w, 0.0, MAX_COORD),
            ("max_w", self.max_w, 0.0, MAX_COORD),
            ("min_h", self.min_h, 0.0, MAX_COORD),
            ("max_h", self.max_h, 0.0, MAX_COORD),
        ];
        for (field, value, lo, hi) in ranged {
            if let Some(v) = value.filter(|v| !v.is_finite() || !(lo..=hi).contains(v)) {
                return Err(format!("{field}: {v} is outside {lo} to {hi}"));
            }
        }
        if let Some(w) = self.font_weight.filter(|w| !(1..=1000).contains(w)) {
            return Err(format!("font_weight: {w} is outside 1 to 1000"));
        }
        if self.font.as_ref().is_some_and(|f| f.is_empty() || f.chars().count() > MAX_NAME) {
            return Err(format!("font: a family name of 1 to {MAX_NAME} characters"));
        }
        if self.text.as_ref().is_some_and(|t| t.chars().count() > MAX_TEXT) {
            return Err(format!("text: at most {MAX_TEXT} characters"));
        }
        if let Some(padding) = &self.padding {
            if !matches!(padding.len(), 1 | 2 | 4)
                || padding.iter().any(|v| !v.is_finite() || !(0.0..=10_000.0).contains(v))
            {
                return Err("padding: [all], [vertical, horizontal] or [top, right, bottom, left], each 0 to 10000".into());
            }
        }
        if let Some(points) = &self.points {
            if points.len() > MAX_POINTS {
                return Err(format!("points: at most {MAX_POINTS}"));
            }
            if points.iter().flatten().any(|v| !v.is_finite() || v.abs() > MAX_COORD) {
                return Err(format!("points: numbers within ±{MAX_COORD}"));
            }
        }
        if let Some(r) = self.rotation.filter(|r| !r.is_finite() || r.abs() > 3600.0) {
            return Err(format!("rotation: {r} is outside ±3600 degrees"));
        }
        if let Some(n) = self.sides.filter(|n| !(3..=24).contains(n)) {
            return Err(format!("sides: {n} is outside 3 to 24"));
        }
        if let Some(v) = self.star.filter(|v| !v.is_finite() || !(0.05..=0.95).contains(v)) {
            return Err(format!("star: {v} is outside 0.05 to 0.95"));
        }
        for (field, g) in [("gradient", &self.gradient), ("stroke_gradient", &self.stroke_gradient)] {
            if let Some(g) = g {
                check_gradient(field, g)?;
            }
        }
        if self.radii.is_some_and(|r| r.iter().any(|v| !v.is_finite() || !(0.0..=MAX_COORD).contains(v))) {
            return Err(format!("radii: four numbers from 0 to {MAX_COORD}"));
        }
        if let Some(dash) = &self.dash {
            if dash.is_empty() || dash.len() > 8 || dash.iter().any(|v| !v.is_finite() || !(0.0..=1000.0).contains(v)) || dash.iter().all(|v| *v == 0.0) {
                return Err("dash: 1 to 8 lengths from 0 to 1000, not all 0".into());
            }
            // The dashes are the even entries; an odd list repeats, so every entry is one in turn.
            let dots = dash.iter().enumerate().any(|(i, v)| *v == 0.0 && (i % 2 == 0 || dash.len() % 2 == 1));
            if dots && self.stroke_cap == Some(StrokeCap::Butt) {
                return Err("stroke_cap: butt ends draw nothing for the 0-length dashes of a dotted dash; use round or square".into());
            }
        }
        if let Some(b) = self.blur.filter(|b| !b.is_finite() || !(0.0..=200.0).contains(b)) {
            return Err(format!("blur: {b} is outside 0 to 200"));
        }
        for (field, end) in [("from", &self.from), ("to", &self.to)] {
            if let Some(id) = end.as_deref().filter(|id| !valid_id(id)) {
                return Err(format!("{field}: {id:?} is not a node id"));
            }
        }
        if let Some(arc) = &self.arc {
            if [arc.start, arc.sweep, arc.inner].iter().any(|v| !v.is_finite())
                || arc.start.abs() > 3600.0
                || !(-360.0..=360.0).contains(&arc.sweep)
                || !(0.0..1.0).contains(&arc.inner)
            {
                return Err("arc: start within ±3600 degrees, sweep within ±360, inner from 0 to below 1".into());
            }
            if matches!(arc.cap, Some(StrokeCap::Round | StrokeCap::Square)) && (arc.inner == 0.0 || arc.sweep.abs() >= 360.0) {
                return Err("arc.cap: only a ring segment (inner above 0, sweep under 360) has ends to shape".into());
            }
        }
        if let Some(d) = &self.icon_path {
            path_data::parse(d).map_err(|why| format!("icon_path: {why}"))?;
        }
        // With its own outline, an icon's name is only what it was saved as.
        if let Some(name) = self.icon.as_deref().filter(|n| !ICONS.contains(n) && self.icon_path.is_none()) {
            return Err(format!("icon: {name:?} is neither one of Relay's icons ({}) nor one saved to its set (design.icon.list)", ICONS.join(", ")));
        }
        if let Some(chart) = &self.chart {
            chart.check()?;
            if self.layout_mode() != Layout::None {
                return Err("chart: a chart's frame places its parts itself; give it no layout".into());
            }
            if self.id.len() > MAX_CHART_ID {
                return Err(format!("chart: its parts are named from the frame's id, which for a chart is at most {MAX_CHART_ID} characters"));
            }
        }
        if let Some(id) = self.instance_of.as_deref().filter(|id| !valid_id(id)) {
            return Err(format!("instance_of: {id:?} is not a node id"));
        }
        if let Some(fields) = &self.overrides {
            if fields.len() > MAX_OVERRIDES {
                return Err(format!("overrides: at most {MAX_OVERRIDES} fields"));
            }
            if let Some(field) = fields.iter().find(|f| NOT_OVERRIDDEN.contains(&f.as_str()) || fields::field_kinds(f).is_none()) {
                return Err(format!("overrides: {field:?} is not a field an instance can keep as its own"));
            }
        }
        if let Some(v) = self.max_lines {
            if !(1..=1000).contains(&v) {
                return Err(format!("max_lines: {v} is outside 1 to 1000"));
            }
            if self.kind == NodeKind::Text && self.w_rule() == Sizing::Hug {
                return Err("max_lines: a text that hugs its words never wraps; give it a width (w with w_sizing fixed or fill)".into());
            }
        }
        if let Some(runs) = &self.runs {
            self.check_runs(runs)?;
        }
        if let Some(m) = &self.motion {
            motion::check(self.kind, m)?;
        }
        if let Some(t) = &self.timeline {
            t.check()?;
        }
        if let Some(style) = self.style.as_deref().filter(|s| !valid_id(s)) {
            return Err(format!("style: {style:?} is not a style name: use 1 to 64 letters, digits, '-', '_', '.' or ':'"));
        }
        self.check_layout()?;
        match self.kind {
            NodeKind::Line
                if self.points.as_ref().is_none_or(|p| p.len() < 2) && (self.from.is_none() || self.to.is_none()) =>
            {
                return Err("a line needs at least 2 points, or both from and to".into())
            }
            NodeKind::Icon if self.icon.is_none() => {
                return Err(format!("an icon needs icon: one of {}", ICONS.join(", ")))
            }
            NodeKind::Path if self.points.as_ref().is_none_or(Vec::is_empty) => {
                return Err("a path needs at least 1 point".into())
            }
            NodeKind::Image if self.src.as_deref().is_none_or(|s| !valid_src(s)) => {
                return Err("an image needs the src design.asset.add returned".into())
            }
            _ => {}
        }
        Ok(())
    }

    /// The parts of its text styled on their own: within the text, in order, each styling
    /// something, each field in its range.
    fn check_runs(&self, runs: &[TextRun]) -> Result<(), String> {
        if !(1..=MAX_RUNS).contains(&runs.len()) {
            return Err(format!("runs: 1 to {MAX_RUNS} of them; null for none"));
        }
        let n = match self.text.as_deref() {
            Some(text) if !text.is_empty() => text.chars().count(),
            _ => return Err("runs: a node without text has no runs".into()),
        };
        let mut last: Option<&TextRun> = None;
        for (i, run) in runs.iter().enumerate() {
            let (s, e) = (run.start, run.end);
            if s >= e {
                return Err(format!("runs[{i}]: start ({s}) is not before end ({e})"));
            }
            if e as usize > n {
                return Err(format!("runs[{i}].end: {e} is past the text's {n} characters"));
            }
            if let Some(prev) = last.filter(|prev| s < prev.end) {
                return Err(format!("runs[{i}]: starts at {s}, inside runs[{}] ({} to {}); give them in order without overlap", i - 1, prev.start, prev.end));
            }
            if !run.styled() {
                return Err(format!("runs[{i}]: give it a style (color, font, font_size, font_weight, italic, underline, strike, rise or letter_spacing)"));
            }
            if let Some(c) = run.color.as_deref().filter(|c| !colour_ok(c)) {
                return Err(not_a_colour(&format!("runs[{i}].color"), c));
            }
            if run.font.as_ref().is_some_and(|f| f.is_empty() || f.chars().count() > MAX_NAME) {
                return Err(format!("runs[{i}].font: a family name of 1 to {MAX_NAME} characters"));
            }
            for (field, value, lo, hi) in [("font_size", run.font_size, 1.0, 1000.0), ("rise", run.rise, -1000.0, 1000.0), ("letter_spacing", run.letter_spacing, -100.0, 1000.0)] {
                if let Some(v) = value.filter(|v| !v.is_finite() || !(lo..=hi).contains(v)) {
                    return Err(format!("runs[{i}].{field}: {v} is outside {lo} to {hi}"));
                }
            }
            if let Some(w) = run.font_weight.filter(|w| !(1..=1000).contains(w)) {
                return Err(format!("runs[{i}].font_weight: {w} is outside 1 to 1000"));
            }
            last = Some(run);
        }
        Ok(())
    }

    /// The colours and the style this node names, each with the field a refusal names: `fill`,
    /// `stroke`, `color`, `shadow`, `shadows`, `gradient`, `stroke_gradient`, `runs` (the `$`
    /// left off a colour's name) or `style`.
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
        out.extend(self.runs.iter().flatten().filter_map(|r| r.color.as_deref().and_then(color_ref)).map(|name| ("runs", name)));
        out.extend(self.style.as_deref().map(|style| ("style", style)));
        out
    }

    /// The shadows it casts, in paint order: `shadows`, else `shadow` (one holding both, written
    /// by two builds, casts `shadows`).
    pub fn shadow_list(&self) -> &[Shadow] {
        self.shadows.as_deref().unwrap_or(self.shadow.as_slice())
    }

    /// Where a rect's, ellipse's or polygon's label sits inside its box `width` wide (as placed),
    /// as top, right, bottom and left: `padding`, else 12 px at the sides, less on a narrow shape
    /// (an eighth of its width), so a small badge keeps room for its words.
    pub fn label_inset(&self, width: f64) -> [f64; 4] {
        if self.padding.is_some() {
            self.padding_edges()
        } else {
            let side = 12f64.min(width / 8.0).max(0.0);
            [0.0, side, 0.0, side]
        }
    }

    /// Padding as top, right, bottom and left.
    pub fn padding_edges(&self) -> [f64; 4] {
        match self.padding.as_deref() {
            Some([all]) => [*all; 4],
            Some([v, h]) => [*v, *h, *v, *h],
            Some([t, r, b, l]) => [*t, *r, *b, *l],
            _ => [0.0; 4],
        }
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden == Some(true)
    }

    pub fn is_locked(&self) -> bool {
        self.locked == Some(true)
    }

    pub fn layout_mode(&self) -> Layout {
        if self.kind == NodeKind::Frame {
            self.layout.unwrap_or(Layout::None)
        } else {
            Layout::None
        }
    }
}

/// Whether `c` is a colour a node may hold: a literal or a `$name` the design names.
fn colour_ok(c: &str) -> bool {
    parse_color(c).is_some() || color_ref(c).is_some()
}

/// The refusal of `c` in colour field `field`.
fn not_a_colour(field: &str, c: &str) -> String {
    format!("{field}: {c:?} is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour")
}

/// A shadow's values, its messages starting with `field` (`shadow`, `shadows[2]`): a colour, and
/// x, y and spread within ±10000 with a blur from 0 (a strict list also caps the blur at 200).
pub fn check_shadow(field: &str, s: &Shadow) -> Result<(), String> {
    if !colour_ok(&s.color) {
        return Err(not_a_colour(&format!("{field}.color"), &s.color));
    }
    if [s.x, s.y, s.blur, s.spread].iter().any(|v| !v.is_finite() || v.abs() > 10_000.0) || s.blur < 0.0 {
        return Err(format!("{field}: x, y and spread within ±10000, blur from 0"));
    }
    Ok(())
}

/// A gradient's values, its messages starting with `field` (`gradient`, `stroke_gradient`): 2 to
/// 8 stops, each at 0 to 1 with a colour, and an angle that is a number.
pub fn check_gradient(field: &str, g: &Gradient) -> Result<(), String> {
    if !(2..=8).contains(&g.stops.len()) {
        return Err(format!("{field}: give 2 to 8 stops"));
    }
    for stop in &g.stops {
        if !stop.at.is_finite() || !(0.0..=1.0).contains(&stop.at) {
            return Err(format!("{field}: a stop's at ({}) is outside 0 to 1", stop.at));
        }
        if !colour_ok(&stop.color) {
            return Err(not_a_colour(field, &stop.color));
        }
    }
    if g.angle.is_some_and(|a| !a.is_finite()) {
        return Err(format!("{field}: angle must be a number"));
    }
    Ok(())
}

/// An asset name as `design.asset.add` gives it: hex digest, a dot and a picture extension.
pub fn valid_src(src: &str) -> bool {
    let Some((stem, ext)) = src.split_once('.') else {
        return false;
    };
    (16..=64).contains(&stem.len())
        && stem.bytes().all(|b| b.is_ascii_hexdigit())
        && matches!(ext, "png" | "jpg" | "gif" | "webp" | "svg")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rect() -> Node {
        let mut n = Node::new(NodeKind::Rect);
        n.id = "r1".into();
        n.parent = Some("p1".into());
        n.w = 100.0;
        n.h = 50.0;
        n
    }

    #[test]
    fn colors_parse_in_three_lengths() {
        assert_eq!(parse_color("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_color("#102030"), Some([16, 32, 48, 255]));
        assert_eq!(parse_color("#10203080"), Some([16, 32, 48, 128]));
        assert_eq!(parse_color("102030"), None);
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("#ggg"), None);
    }

    #[test]
    fn a_patch_sets_clears_and_refuses_identity() {
        let mut r = rect();
        r.fill = Some("#ff0000".into());
        let set = json!({"x": 12, "fill": null, "radius": 8}).as_object().unwrap().clone();
        let p = r.patched(&set).unwrap();
        assert_eq!((p.x, p.fill.clone(), p.radius), (12.0, None, Some(8.0)));
        let undo = r.before(&set);
        assert_eq!(Value::Object(undo.clone()), json!({"x": 0.0, "fill": "#ff0000", "radius": null}));
        assert_eq!(p.patched(&undo).unwrap(), r);
        for key in ["id", "kind", "parent"] {
            let set = json!({key: "x"}).as_object().unwrap().clone();
            assert!(r.patched(&set).unwrap_err().contains(key));
        }
    }

    #[test]
    fn a_patch_reads_width_and_height_as_w_and_h() {
        let r = rect();
        let set = json!({"width": 300, "height": 40}).as_object().unwrap().clone();
        let p = r.patched(&set).unwrap();
        assert_eq!((p.w, p.h), (300.0, 40.0));
        assert_eq!(Value::Object(r.before(&set)), json!({"w": 100.0, "h": 50.0}));
    }

    #[test]
    fn a_patch_names_the_bad_field() {
        let r = rect();
        // A typo reads (it waits in `extra`); a strict list refuses it by name.
        let typo = json!({"fil": "#fff"}).as_object().unwrap().clone();
        let patched = r.patched(&typo).unwrap();
        assert_eq!(patched.extra.get("fil"), Some(&json!("#fff")));
        assert!(Node::check_update(&r, &typo, &patched).unwrap_err().starts_with("fil: not a node field"));
        let bad = json!({"fill": "red"}).as_object().unwrap().clone();
        assert!(r.patched(&bad).unwrap_err().starts_with("fill"));
        let neg = json!({"w": -1}).as_object().unwrap().clone();
        assert!(r.patched(&neg).unwrap_err().starts_with("w:"));
        // A value that does not read is named, with its range when its type cannot hold it.
        let wide = json!({"sides": 300}).as_object().unwrap().clone();
        let patched = r.patched(&wide).unwrap();
        assert_eq!(Node::check_update(&r, &wide, &patched).unwrap_err(), "sides: 300 is outside 3 to 24");
        // A type error names its key (W16).
        let text = json!({"x": "12"}).as_object().unwrap().clone();
        let patched = r.patched(&text).unwrap();
        assert_eq!(Node::check_update(&r, &text, &patched).unwrap_err(), "x: invalid type: string \"12\", expected f64");
    }

    /// B122: the weights the schema documents are the ones that validate.
    #[test]
    fn font_weight_runs_from_1_to_1000() {
        let mut t = rect();
        for (weight, ok) in [(1, true), (1000, true), (0, false), (1001, false)] {
            t.font_weight = Some(weight);
            match t.validate() {
                Ok(()) => assert!(ok, "{weight}"),
                Err(why) => assert!(!ok && why.starts_with("font_weight"), "{weight}: {why}"),
            }
        }
    }

    /// B125: a shadow takes `#RGB` like every colour, and its refusal names all three forms.
    #[test]
    fn a_shadow_takes_short_colours_and_names_all_three() {
        let mut r = rect();
        r.shadow = Some(Shadow { x: 0.0, y: 2.0, blur: 4.0, spread: 0.0, color: "#000".into(), inset: false });
        assert_eq!(r.validate(), Ok(()));
        r.shadow.as_mut().unwrap().color = "red".into();
        assert_eq!(r.validate().unwrap_err(), "shadow.color: \"red\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour");
        // Every colour's doc names the three forms.
        let schema = serde_json::to_value(schemars::schema_for!(Node)).unwrap();
        for doc in [&schema["properties"]["fill"], &schema["properties"]["stroke"], &schema["properties"]["color"],
            &schema["$defs"]["DesignShadow"]["properties"]["color"], &schema["$defs"]["DesignGradientStop"]["properties"]["color"]] {
            assert!(doc["description"].as_str().unwrap().contains("`#RGB`, `#RRGGBB` or `#RRGGBBAA`"), "{doc}");
        }
    }

    /// B71: an update naming both spellings of a size is refused, whichever comes first.
    #[test]
    fn a_patch_refuses_both_spellings_of_a_size() {
        let r = rect();
        for set in [json!({"w": 1, "width": 2}), json!({"width": 2, "w": 1})] {
            assert_eq!(r.patched(set.as_object().unwrap()).unwrap_err(), "w: give w or width, not both");
        }
        for set in [json!({"h": 1, "height": 2}), json!({"height": 2, "h": 1})] {
            assert_eq!(r.patched(set.as_object().unwrap()).unwrap_err(), "h: give h or height, not both");
        }
        assert_eq!(r.patched(json!({"width": 300}).as_object().unwrap()).unwrap().w, 300.0);
    }

    /// TM-16: a nested object written as an array is not read field by field.
    #[test]
    fn arrays_are_not_objects() {
        let stop = json!({"at": 0, "color": "#fff"});
        for (field, value) in [
            ("arc", json!([0, 90])),
            ("shadow", json!([0, 2, 6, 0, "#000"])),
            ("gradient", json!(["linear", 90, [stop.clone(), stop.clone()]])),
            ("gradient", json!({"stops": [stop.clone(), [1, "#000"]]})),
            ("shadows", json!([[0, 2, 6, 0, "#000"]])),
            ("runs", json!([[0, 1, "#000"]])),
            ("stroke_gradient", json!(["linear", 90, [stop.clone(), stop.clone()]])),
            ("stroke_gradient", json!({"stops": [stop.clone(), [1, "#000"]]})),
            ("motion", json!([[{"at_ms": 0, "opacity": 0}, {"at_ms": 500, "opacity": 1}]])),
            ("motion", json!({"keys": [[0, "ease", 4], {"at_ms": 500, "opacity": 1}]})),
            ("timeline", json!([3000, true])),
        ] {
            let body = json!({"kind": "ellipse", "parent": "p", "text": "Hi", field: value});
            assert!(serde_json::from_value::<Node>(body.clone()).is_err(), "{body}");
            let node = Node::read(body).unwrap();
            let why = node.check_new().unwrap_err();
            assert!(why.starts_with(&format!("{field}: invalid type: sequence, expected an object")), "{why}");
        }
        // An object still reads, and null is none.
        let node: Node = serde_json::from_value(json!({"kind": "ellipse", "parent": "p", "arc": {"sweep": 90},
            "shadow": null, "gradient": {"stops": [stop.clone(), stop]}})).unwrap();
        assert_eq!((node.arc.map(|a| a.sweep), node.shadow, node.gradient.map(|g| g.stops.len())), (Some(90.0), None, Some(2)));
    }

    fn read(body: Value) -> Node {
        Node::read(body).unwrap()
    }

    /// Reuse test list: `$name` colours in every colour slot, style names, and what a node names.
    #[test]
    fn colour_names_and_styles_validate() {
        let stops = json!([{"at": 0, "color": "$brand"}, {"at": 1, "color": "#fff"}]);
        let named = read(json!({"kind": "text", "parent": "p", "text": "8.4 km", "w": 90, "fill": "$paper", "color": "$ink",
            "stroke": "#000", "shadow": {"blur": 4, "color": "$shade"}, "runs": [{"start": 3, "end": 6, "color": "$muted"}], "style": "body.small"}));
        assert_eq!(named.validate(), Ok(()));
        assert_eq!(named.names(), [("fill", "paper"), ("color", "ink"), ("shadow", "shade"), ("runs", "muted"), ("style", "body.small")]);
        let glow = read(json!({"kind": "line", "parent": "p", "points": [[0, 0], [9, 0]], "stroke": "$a", "gradient": {"stops": stops},
            "stroke_gradient": {"stops": stops}, "shadows": [{"color": "#000"}, {"color": "$b"}]}));
        assert_eq!(glow.validate(), Ok(()));
        assert_eq!(glow.names(), [("stroke", "a"), ("shadows", "b"), ("gradient", "brand"), ("stroke_gradient", "brand")]);
        assert!(rect().names().is_empty());
        for (field, value) in [("fill", "$"), ("stroke", "$bad name"), ("color", "accent")] {
            let mut body = json!({"kind": "rect", "parent": "p"});
            body[field] = json!(value);
            assert_eq!(read(body).validate().unwrap_err(), format!("{field}: {value:?} is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour"));
        }
        let stop = read(json!({"kind": "rect", "parent": "p", "gradient": {"stops": [{"at": 0, "color": "$ x"}, {"at": 1, "color": "#fff"}]}}));
        assert_eq!(stop.validate().unwrap_err(), "gradient: \"$ x\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour");
        for style in ["", "my style", "a/b"] {
            let n = read(json!({"kind": "rect", "parent": "p", "style": style}));
            assert_eq!(n.validate().unwrap_err(), format!("style: {style:?} is not a style name: use 1 to 64 letters, digits, '-', '_', '.' or ':'"));
        }
        // A page takes no style.
        assert_eq!(read(json!({"kind": "page", "style": "card"})).check_new().unwrap_err(), "style: a page takes no style; give it its own fill");
    }

    /// Drawing test 1: the drawing fields read as agents write them, validate, and round-trip.
    #[test]
    fn drawing_fields_read_as_agents_write_them() {
        let stops = json!([{"at": 0, "color": "#4AF0FF"}, {"at": 1, "color": "#7B61FF"}]);
        for body in [
            json!({"kind": "line", "points": [[0, 0], [90, 0]], "stroke_cap": "square", "stroke_join": "bevel",
                "stroke_gradient": {"angle": 90, "stops": stops}, "shadows": [{"blur": 4, "color": "#4AF0FFCC"}, {"blur": 18, "color": "#4AF0FF66"}]}),
            json!({"kind": "ellipse", "w": 120, "h": 120, "gradient": {"kind": "conic", "angle": 350, "stops": stops},
                "arc": {"start": 0, "sweep": 252, "inner": 0.82, "cap": "round"}}),
            json!({"kind": "text", "text": "8.4 km", "w": 160, "max_lines": 1, "runs": [{"start": 3, "end": 6, "font_size": 14, "color": "#8C877F"}]}),
            json!({"kind": "rect", "w": 200, "h": 40, "text": "Continue", "padding": [8, 24]}),
        ] {
            let mut body = body;
            body["parent"] = json!("p");
            let n: Node = serde_json::from_value(body.clone()).unwrap_or_else(|e| panic!("{e}: {body}"));
            assert_eq!((n.validate(), n.check_new()), (Ok(()), Ok(())), "{body}");
            assert!(n.extra.is_empty());
            assert_eq!(serde_json::from_value::<Node>(serde_json::to_value(&n).unwrap()).unwrap(), n);
        }
        let ring = read(json!({"kind": "ellipse", "parent": "p", "arc": {"inner": 0.5, "sweep": 90, "cap": "round"}}));
        assert_eq!(ring.arc.unwrap().cap, Some(StrokeCap::Round));
        // Defaults: a label sits 12 px in from the sides, an eighth of a narrow shape's width;
        // padding moves it.
        assert_eq!(rect().label_inset(200.0), [0.0, 12.0, 0.0, 12.0]);
        assert_eq!(rect().label_inset(40.0), [0.0, 5.0, 0.0, 5.0]);
        let mut padded = rect();
        padded.padding = Some(vec![8.0, 24.0]);
        assert_eq!(padded.label_inset(40.0), [8.0, 24.0, 8.0, 24.0]);
        // Unset, none of it is written; an arc without a cap writes none.
        let plain = serde_json::to_value(Node::new(NodeKind::Ellipse)).unwrap();
        for key in ["stroke_cap", "stroke_join", "stroke_gradient", "shadows", "runs", "max_lines"] {
            assert!(plain.get(key).is_none(), "{key}");
        }
        assert_eq!(serde_json::to_value(Arc { start: 0.0, sweep: 90.0, inner: 0.0, cap: None }).unwrap(), json!({"start": 0.0, "sweep": 90.0, "inner": 0.0}));
    }

    /// Drawing test 2: `shadows` is a list beside `shadow`, read first.
    #[test]
    fn shadows_are_a_list_beside_the_old_field() {
        let glow = Shadow { x: 0.0, y: 0.0, blur: 16.0, spread: 0.0, color: "#4AF0FF99".into(), inset: false };
        let mut n = rect();
        assert!(n.shadow_list().is_empty());
        n.shadow = Some(Shadow { y: 4.0, ..glow.clone() });
        assert_eq!(n.shadow_list(), [Shadow { y: 4.0, ..glow.clone() }]);
        n.shadows = Some(vec![glow.clone(), glow.clone()]);
        assert_eq!(n.shadow_list(), [glow.clone(), glow.clone()], "shadows wins");
        n.shadow = None;
        n.shadows = Some(Vec::new());
        assert_eq!(n.validate().unwrap_err(), "shadows: 1 to 8 of them; null for none");
        n.shadows = Some(vec![glow.clone(); 9]);
        assert_eq!(n.validate().unwrap_err(), "shadows: 1 to 8 of them; null for none");
        n.shadows = Some(vec![glow.clone(), Shadow { color: "glow".into(), ..glow.clone() }]);
        assert_eq!(n.validate().unwrap_err(), "shadows[1].color: \"glow\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour");
        n.shadows = Some(vec![Shadow { blur: -1.0, ..glow }]);
        assert_eq!(n.validate().unwrap_err(), "shadows[0]: x, y and spread within ±10000, blur from 0");
        // An update moves one shadow into the list in one set.
        let one = read(json!({"kind": "rect", "parent": "p", "shadow": {"color": "#000"}}));
        let set = json!({"shadow": null, "shadows": [{"color": "#000"}, {"blur": 9, "color": "#fff"}]}).as_object().unwrap().clone();
        let after = one.patched(&set).unwrap();
        assert_eq!((after.shadow.clone(), after.shadow_list().len()), (None, 2));
        assert_eq!(Node::check_update(&one, &set, &after), Ok(()));
    }

    /// Drawing test 3: runs lie inside their text, in order, each styling something.
    #[test]
    fn runs_stay_inside_their_text_in_order() {
        let text = |runs: Value| read(json!({"kind": "text", "parent": "p", "text": "8.4 km", "runs": runs})).validate();
        assert_eq!(text(json!([{"start": 0, "end": 3, "font_weight": 700}, {"start": 3, "end": 6, "font_size": 14, "rise": 4}])), Ok(()));
        for (runs, why) in [
            (json!([]), "runs: 1 to 64 of them; null for none"),
            (json!([{"start": 3, "end": 7, "italic": true}]), "runs[0].end: 7 is past the text's 6 characters"),
            (json!([{"start": 0, "end": 4, "italic": true}, {"start": 3, "end": 6, "strike": true}]),
                "runs[1]: starts at 3, inside runs[0] (0 to 4); give them in order without overlap"),
            (json!([{"start": 2, "end": 4}]), "runs[0]: give it a style (color, font, font_size, font_weight, italic, underline, strike, rise or letter_spacing)"),
            (json!([{"start": 0, "end": 1, "letter_spacing": 1001}]), "runs[0].letter_spacing: 1001 is outside -100 to 1000"),
            (json!([{"start": 2, "end": 2, "italic": true}]), "runs[0]: start (2) is not before end (2)"),
            (json!([{"start": 0, "end": 1, "color": "grey"}]), "runs[0].color: \"grey\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour"),
            (json!([{"start": 0, "end": 1, "font_size": 0}]), "runs[0].font_size: 0 is outside 1 to 1000"),
            (json!([{"start": 0, "end": 1, "font_weight": 1001}]), "runs[0].font_weight: 1001 is outside 1 to 1000"),
            (json!([{"start": 0, "end": 1, "rise": -1001}]), "runs[0].rise: -1001 is outside -1000 to 1000"),
            (json!([{"start": 0, "end": 1, "font": ""}]), "runs[0].font: a family name of 1 to 200 characters"),
        ] {
            assert_eq!(text(runs.clone()).unwrap_err(), why, "{runs}");
        }
        // Counted in characters, not bytes.
        let wide = read(json!({"kind": "text", "parent": "p", "text": "straße", "runs": [{"start": 4, "end": 6, "underline": true}]}));
        assert_eq!(wide.validate(), Ok(()));
        let none = read(json!({"kind": "text", "parent": "p", "runs": [{"start": 0, "end": 1, "italic": true}]}));
        assert_eq!(none.validate().unwrap_err(), "runs: a node without text has no runs");
        // A run reads only its own fields.
        assert!(serde_json::from_value::<TextRun>(json!({"from": 0, "to": 1})).is_err());
    }

    /// Drawing test 4: `max_lines` needs a width to cut at.
    #[test]
    fn max_lines_needs_a_width() {
        let hug = "max_lines: a text that hugs its words never wraps; give it a width (w with w_sizing fixed or fill)";
        assert_eq!(read(json!({"kind": "text", "parent": "p", "text": "Hi", "max_lines": 2})).validate().unwrap_err(), hug);
        assert_eq!(read(json!({"kind": "text", "parent": "p", "text": "Hi", "w": 90, "w_sizing": "hug", "max_lines": 2})).validate().unwrap_err(), hug);
        for ok in [
            json!({"kind": "text", "parent": "p", "text": "Hi", "w": 160, "max_lines": 1}),
            json!({"kind": "text", "parent": "p", "text": "Hi", "w_sizing": "fill", "max_lines": 3}),
            json!({"kind": "sticky", "parent": "p", "text": "Hi", "max_lines": 1000}),
            json!({"kind": "rect", "parent": "p", "w": 90, "h": 30, "text": "Hi", "max_lines": 1}),
        ] {
            assert_eq!(read(ok.clone()).validate(), Ok(()), "{ok}");
        }
        for v in [0, 1001] {
            let n = read(json!({"kind": "sticky", "parent": "p", "max_lines": v}));
            assert_eq!(n.validate().unwrap_err(), format!("max_lines: {v} is outside 1 to 1000"));
        }
    }

    /// Drawing test 5: only a ring segment has ends to shape.
    #[test]
    fn arc_caps_need_a_ring_segment() {
        let ring = |arc: Value| read(json!({"kind": "ellipse", "parent": "p", "arc": arc})).validate();
        let why = "arc.cap: only a ring segment (inner above 0, sweep under 360) has ends to shape";
        assert_eq!(ring(json!({"inner": 0, "sweep": 270, "cap": "round"})).unwrap_err(), why);
        assert_eq!(ring(json!({"inner": 0.8, "sweep": 360, "cap": "square"})).unwrap_err(), why);
        assert_eq!(ring(json!({"inner": 0.8, "sweep": -360, "cap": "round"})).unwrap_err(), why);
        assert_eq!(ring(json!({"inner": 0.8, "sweep": 270, "cap": "round"})), Ok(()));
        assert_eq!(ring(json!({"inner": 0.8, "sweep": -90, "cap": "square"})), Ok(()));
        for arc in [json!({"inner": 0, "cap": "butt"}), json!({"inner": 0.5, "sweep": 360, "cap": "butt"})] {
            assert_eq!(ring(arc), Ok(()), "butt is the default written out");
        }
    }

    /// Drawing test 6: flat ends draw nothing of a dot.
    #[test]
    fn butt_caps_on_dots_are_refused() {
        let why = "stroke_cap: butt ends draw nothing for the 0-length dashes of a dotted dash; use round or square";
        let line = |dash: Value, cap: &str| read(json!({"kind": "line", "parent": "p", "points": [[0, 0], [9, 0]], "dash": dash, "stroke_cap": cap})).validate();
        assert_eq!(line(json!([0, 4]), "butt").unwrap_err(), why);
        assert_eq!(line(json!([6, 4, 0]), "butt").unwrap_err(), why, "an odd list repeats, so its 0 is a dash in turn");
        assert_eq!(line(json!([6, 0]), "butt"), Ok(()), "a 0 gap is no dot");
        assert_eq!(line(json!([0, 4]), "round"), Ok(()));
        assert_eq!(line(json!([6, 4]), "butt"), Ok(()));
        // A conic gradient takes 2 to 8 stops like the others, on the stroke too.
        let one = read(json!({"kind": "rect", "parent": "p", "stroke_gradient": {"kind": "conic", "stops": [{"at": 0, "color": "#000"}]}}));
        assert_eq!(one.validate().unwrap_err(), "stroke_gradient: give 2 to 8 stops");
    }

    /// What agents read of a node and its edits is plain text: no rustdoc link leaks into it.
    #[test]
    fn the_edit_schema_holds_no_rustdoc_links() {
        let schema = serde_json::to_value(schemars::schema_for!(NodeOp)).unwrap().to_string();
        assert!(!schema.contains("[`"), "a rustdoc link leaked into the schema");
        for def in ["DesignPinX", "DesignPinY", "DesignScroll", "DesignStrokeCap", "DesignTextRun", "DesignNode"] {
            assert!(schema.contains(def), "{def}");
        }
    }

    /// The tool schema still refuses unknown keys, and does not show `extra`.
    #[test]
    fn the_node_schema_still_refuses_unknown_keys() {
        let schema = serde_json::to_value(schemars::schema_for!(Node)).unwrap();
        assert_eq!(schema["additionalProperties"], json!(false), "{schema}");
        assert!(schema["properties"].get("extra").is_none());
        assert!(schema["properties"].get("w").is_some());
    }

    #[test]
    fn kinds_are_named_as_stored() {
        for kind in NodeKind::ALL {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.name());
            assert_eq!(NodeKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(NodeKind::from_name("component"), None);
    }

    fn line(points: Vec<[f64; 2]>) -> Node {
        let mut n = Node::new(NodeKind::Line);
        n.parent = Some("p".into());
        n.points = Some(points);
        n
    }

    /// Where `n` draws its points on the page: about its box's centre, turned.
    fn drawn(n: &Node) -> Vec<[f64; 2]> {
        let (s, c) = n.rotation.unwrap_or(0.0).to_radians().sin_cos();
        let (cx, cy) = (n.x + n.w / 2.0, n.y + n.h / 2.0);
        n.points.as_ref().unwrap().iter().map(|p| {
            let (dx, dy) = (n.x + p[0] - cx, n.y + p[1] - cy);
            [cx + dx * c - dy * s, cy + dx * s + dy * c]
        }).collect()
    }

    /// B3: a free line's or path's box is fitted to its points, and the drawing stays put.
    #[test]
    fn contain_points_fits_a_free_line_box() {
        let mut n = line(vec![[0.0, 0.0], [300.0, -50.0]]);
        (n.x, n.y) = (100.0, 100.0);
        assert!(n.contain_points());
        assert_eq!((n.x, n.y, n.w, n.h), (100.0, 50.0, 300.0, 50.0));
        assert_eq!(n.points, Some(vec![[0.0, 50.0], [300.0, 0.0]]));
        assert!(!n.contain_points(), "a fitted box stays");
        // A box larger than its points shrinks to them.
        let mut wide = line(vec![[10.0, 10.0], [20.0, 10.0]]);
        (wide.w, wide.h) = (100.0, 100.0);
        assert!(wide.contain_points());
        assert_eq!((wide.x, wide.y, wide.w, wide.h, wide.points.clone()), (10.0, 10.0, 10.0, 0.0, Some(vec![[0.0, 0.0], [10.0, 0.0]])));
        // A connector is drawn from its ends; it keeps its box.
        let mut link = line(vec![[0.0, 0.0], [300.0, -50.0]]);
        link.from = Some("a".into());
        assert!(!link.contain_points());
        link.from = None;
        link.route = Some(Route::Elbow);
        assert!(!link.contain_points());
        // A one-point path.
        let mut dot = Node::new(NodeKind::Path);
        dot.points = Some(vec![[-4.0, 7.0]]);
        assert!(dot.contain_points());
        assert_eq!((dot.x, dot.y, dot.w, dot.h, dot.points.clone()), (-4.0, 7.0, 0.0, 0.0, Some(vec![[0.0, 0.0]])));
        // Other kinds are left alone.
        let mut r = rect();
        r.points = Some(vec![[-5.0, -5.0]]);
        assert!(!r.contain_points());
    }

    #[test]
    fn contain_points_keeps_a_turned_path_still() {
        for rotation in [30.0, -75.0, 180.0, 412.0] {
            let mut n = Node::new(NodeKind::Path);
            (n.x, n.y, n.w, n.h, n.rotation) = (40.0, 60.0, 20.0, 10.0, Some(rotation));
            n.points = Some(vec![[-30.0, 5.0], [80.0, -20.0], [15.0, 90.0]]);
            let before = drawn(&n);
            assert!(n.contain_points());
            assert_eq!((n.w, n.h), (110.0, 110.0));
            assert_eq!(n.points.as_ref().unwrap()[0], [0.0, 25.0]);
            for (a, b) in before.iter().zip(drawn(&n)) {
                assert!((a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9, "{rotation}: {a:?} moved to {b:?}");
            }
        }
    }

    /// B49: a shape, frame or icon added without a size gets one on its fixed axes.
    #[test]
    fn a_shape_added_without_a_size_gets_one() {
        for (kind, size) in [(NodeKind::Rect, 100.0), (NodeKind::Ellipse, 100.0), (NodeKind::Polygon, 100.0), (NodeKind::Frame, 100.0), (NodeKind::Icon, 24.0)] {
            let mut n = Node::new(kind);
            assert!(n.fill_default_size());
            assert_eq!((n.w, n.h), (size, size), "{kind:?}");
            assert!(!n.fill_default_size(), "a size given stays");
        }
        // An axis that fills or hugs keeps its 0; the other gets its size.
        let mut fill = Node::new(NodeKind::Rect);
        fill.w_sizing = Some(Sizing::Fill);
        assert!(fill.fill_default_size());
        assert_eq!((fill.w, fill.h), (0.0, 100.0));
        let mut hug = Node::new(NodeKind::Frame);
        (hug.layout, hug.w_sizing, hug.h_sizing) = (Some(Layout::Column), Some(Sizing::Hug), Some(Sizing::Hug));
        assert!(!hug.fill_default_size());
        assert_eq!((hug.w, hug.h), (0.0, 0.0));
        // A rect given its width keeps it.
        let mut half = Node::new(NodeKind::Rect);
        half.w = 30.0;
        half.fill_default_size();
        assert_eq!((half.w, half.h), (30.0, 100.0));
        // Text hugs, a line follows its points, a group bounds its children, a page has no size.
        for kind in [NodeKind::Text, NodeKind::Line, NodeKind::Path, NodeKind::Group, NodeKind::Page, NodeKind::Sticky, NodeKind::Image] {
            let mut n = Node::new(kind);
            assert!(!n.fill_default_size(), "{kind:?}");
        }
    }

    #[test]
    fn size_rules_default_by_kind() {
        let mut t = Node::new(NodeKind::Text);
        assert_eq!((t.w_rule(), t.h_rule()), (Sizing::Hug, Sizing::Hug));
        t.w = 120.0;
        assert_eq!((t.w_rule(), t.h_rule()), (Sizing::Fixed, Sizing::Hug));
        t.h_sizing = Some(Sizing::Fixed);
        assert_eq!(t.h_rule(), Sizing::Fixed);
        assert_eq!((rect().w_rule(), rect().h_rule()), (Sizing::Fixed, Sizing::Fixed));
    }

    #[test]
    fn kinds_need_what_they_draw() {
        let mut line = Node::new(NodeKind::Line);
        line.parent = Some("p".into());
        assert!(line.validate().is_err());
        line.points = Some(vec![[0.0, 0.0], [10.0, 10.0]]);
        assert!(line.validate().is_ok());
        let mut page = Node::new(NodeKind::Page);
        assert!(page.validate().is_ok());
        page.parent = Some("p".into());
        assert!(page.validate().is_err());
        let mut img = Node::new(NodeKind::Image);
        img.parent = Some("p".into());
        img.src = Some("../etc/passwd".into());
        assert!(img.validate().is_err());
        img.src = Some("0123456789abcdef0123.png".into());
        assert!(img.validate().is_ok());
    }

    #[test]
    fn the_richer_fields_are_checked() {
        let mut star = Node::new(NodeKind::Polygon);
        star.parent = Some("p".into());
        star.sides = Some(5);
        star.star = Some(0.4);
        star.rotation = Some(-12.0);
        star.dash = Some(vec![6.0, 4.0]);
        assert!(star.validate().is_ok());
        star.sides = Some(2);
        assert!(star.validate().unwrap_err().starts_with("sides"));
        let mut icon = Node::new(NodeKind::Icon);
        icon.parent = Some("p".into());
        assert!(icon.validate().unwrap_err().contains("an icon needs icon"));
        icon.icon = Some("settings".into());
        assert!(icon.validate().is_ok());
        icon.icon = Some("nope".into());
        assert!(icon.validate().unwrap_err().starts_with("icon:"));
        // A connector needs no points once both ends are attached.
        let mut link = Node::new(NodeKind::Line);
        link.parent = Some("p".into());
        link.from = Some("a".into());
        assert!(link.validate().is_err());
        link.to = Some("b".into());
        assert!(link.validate().is_ok());
        let g: Node = serde_json::from_value(json!({"kind": "rect", "parent": "p",
            "gradient": {"stops": [{"at": 0, "color": "#fff"}, {"at": 1.5, "color": "#000"}]}})).unwrap();
        assert!(g.validate().unwrap_err().contains("outside 0 to 1"));
        let mut dots = Node::new(NodeKind::Rect);
        dots.parent = Some("p".into());
        dots.dash = Some(vec![0.0, 0.0]);
        assert!(dots.validate().is_err());
    }

    #[test]
    fn arcs_blends_and_cases_read_as_agents_write_them() {
        let donut: Node = serde_json::from_value(json!({"kind": "ellipse", "parent": "p",
            "arc": {"sweep": 270, "inner": 0.6}, "blend": "color_dodge", "text_case": "upper",
            "shadow": {"y": 2, "blur": 6, "color": "#00000033", "inset": true}})).unwrap();
        assert!(donut.validate().is_ok());
        let arc = donut.arc.clone().unwrap();
        assert_eq!((arc.start, arc.sweep, arc.inner), (0.0, 270.0, 0.6));
        assert_eq!(donut.blend.map(Blend::name), Some("color_dodge"));
        assert!(donut.shadow.as_ref().unwrap().inset);
        let mut full = donut.clone();
        full.arc = Some(Arc { start: 0.0, sweep: 360.0, inner: 1.0, cap: None });
        assert!(full.validate().unwrap_err().starts_with("arc"));
        // An outer shadow writes no `inset`, so older readers keep reading it.
        let plain = Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: "#000".into(), inset: false };
        assert!(serde_json::to_value(&plain).unwrap().get("inset").is_none());
        assert_eq!(TextCase::Title.apply("hello  wide-world 2go"), "Hello  Wide-World 2go");
        assert_eq!(TextCase::Upper.apply("straße"), "STRASSE");
    }

    /// B74: title case starts a word where the canvas's text editor does (after a space or any
    /// punctuation, not inside a word's apostrophe) and titlecases the digraphs.
    #[test]
    fn title_case_starts_words_where_the_editor_does() {
        assert_eq!(TextCase::Title.apply("well-known and/or don't \u{1C6}ep"), "Well-Known And/Or Don't \u{1C5}ep");
        assert_eq!(TextCase::Title.apply("3rd place"), "3rd Place");
        assert_eq!(TextCase::Title.apply("'tis rock'n'roll, (quoted) \u{2019}twas"), "'Tis Rock'n'roll, (Quoted) \u{2019}Twas");
        for (digraph, title) in [('\u{1C4}', '\u{1C5}'), ('\u{1C9}', '\u{1C8}'), ('\u{1CA}', '\u{1CB}'), ('\u{1F3}', '\u{1F2}')] {
            assert_eq!(TextCase::Title.apply(&format!("{digraph}x")), format!("{title}x"));
        }
        // The rest of a word keeps its case, as the editor's capitalising does.
        assert_eq!(TextCase::Title.apply("mcDonald iPHONE"), "McDonald IPHONE");
    }

    /// TM-34: one word-start rule for the canvas and the editor.
    #[test]
    fn title_starts_name_each_word() {
        let text = "jean-luc e-mail foo/bar foo.bar";
        let starts: Vec<char> = title_starts(text).into_iter().map(|i| text.chars().nth(i).unwrap()).collect();
        assert_eq!(starts, ['j', 'l', 'e', 'm', 'f', 'b', 'f', 'b']);
        assert_eq!(title_starts("don't 2go"), [0, 6]);
        assert!(title_starts("  -- ").is_empty());
        // The editor capitalises exactly what apply does.
        let text = "hello  wide-world 2go \u{1C6}ep";
        let mut drawn: Vec<char> = text.chars().collect();
        for i in title_starts(text) {
            let mut one = String::new();
            title_case(drawn[i], &mut one);
            drawn[i] = one.chars().next().unwrap();
        }
        assert_eq!(drawn.into_iter().collect::<String>(), TextCase::Title.apply(text));
    }

    /// Drawing test 12: each character's start in the cased text.
    #[test]
    fn case_offsets_follow_the_cased_text() {
        let (upper, starts) = TextCase::Upper.apply_indexed("straße km");
        assert_eq!(upper, "STRASSE KM");
        assert_eq!(starts, [0, 1, 2, 3, 4, 6, 7, 8, 9, 10]);
        assert_eq!(starts[7], 8, "the k");
        for case in [TextCase::Upper, TextCase::Lower, TextCase::Title] {
            for text in ["", "Hello  wide-WORLD 2go", "straße İstanbul ǆep", "a\u{0}b"] {
                let (cased, starts) = case.apply_indexed(text);
                assert_eq!(cased, case.apply(text), "{case:?} {text:?}");
                assert_eq!(starts.len(), text.chars().count() + 1);
                assert_eq!(starts.last(), Some(&cased.len()));
                assert!(starts.windows(2).all(|w| w[0] <= w[1]) && starts.iter().all(|s| cased.is_char_boundary(*s)));
            }
        }
        // Upper and lower map one letter at a time: no final-sigma rule.
        assert_eq!(TextCase::Lower.apply("ΟΔΟΣ"), "οδοσ");
    }

    #[test]
    fn ops_read_as_agents_write_them() {
        let ops: Vec<NodeOp> = serde_json::from_value(json!([
            {"op": "add", "node": {"id": "card", "parent": "p1", "kind": "frame", "width": 320, "height": 200, "layout": "column", "gap": 8, "padding": [16]}},
            {"op": "add", "node": {"parent": "card", "kind": "text", "text": "Hello"}, "index": 0},
            {"op": "update", "id": "card", "set": {"fill": "#111111"}},
            {"op": "move", "id": "card", "index": 0},
            {"op": "delete", "id": "card"}
        ]))
        .unwrap();
        assert_eq!(ops.len(), 5);
        let NodeOp::Add { node, .. } = &ops[0] else { panic!() };
        assert_eq!((node.w, node.h, node.padding_edges()), (320.0, 200.0, [16.0; 4]));
        assert_eq!(ops[1].target(), "");
        assert!(serde_json::from_value::<NodeOp>(json!({"op": "delete", "id": "a", "extra": 1})).is_err());
    }

    /// Reuse test list: the copy, colour and style edits read as agents write them, and write
    /// back only what they hold.
    #[test]
    fn reuse_edits_read_as_agents_write_them() {
        let edits = json!([
            {"op": "copy", "id": "card", "as": "card-2", "dx": 400},
            {"op": "copy", "id": "page-1", "index": 0},
            {"op": "color", "name": "accent", "value": "#4C9DFF"},
            {"op": "style", "name": "card", "set": {"fill": "$accent", "radius": 12, "shadow": null}},
            {"op": "delete_style", "name": "accent"}
        ]);
        let ops: Vec<NodeOp> = serde_json::from_value(edits.clone()).unwrap();
        let NodeOp::Copy { id, as_id, parent, index, dx, dy, .. } = &ops[0] else { panic!() };
        assert_eq!((id.as_str(), as_id.as_deref(), parent, index, *dx, *dy), ("card", Some("card-2"), &None, &None, 400.0, 0.0));
        assert_eq!(ops.iter().map(NodeOp::target).collect::<Vec<_>>(), ["card", "page-1", "accent", "card", "accent"]);
        assert_eq!(ops.iter().map(NodeOp::node_target).collect::<Vec<_>>(), [Some("card"), Some("page-1"), None, None, None]);
        assert_eq!(ops.iter().map(NodeOp::name).collect::<Vec<_>>(), ["copy", "copy", "color", "style", "delete_style"]);
        let labels: Vec<String> = ops.iter().enumerate().map(|(i, op)| label(i, op)).collect();
        assert_eq!(labels, ["ops[0] copy \"card\"", "ops[1] copy \"page-1\"", "ops[2] color \"accent\"", "ops[3] style \"card\"", "ops[4] delete_style \"accent\""]);
        // Written back as read: `as` by its name, a zero offset left out.
        let written = serde_json::to_value(&ops).unwrap();
        assert_eq!(written[0], json!({"op": "copy", "id": "card", "as": "card-2", "dx": 400.0}));
        assert_eq!(written[1], json!({"op": "copy", "id": "page-1", "index": 0}));
        assert_eq!(written.as_array().unwrap()[2..], edits.as_array().unwrap()[2..]);
        for wrong in [json!({"op": "copy", "id": "a", "offset": 1}), json!({"op": "color", "name": "a", "value": "#fff", "alpha": 1}),
            json!({"op": "delete_style", "name": "a", "force": true}), json!({"op": "copy", "id": "a", "as_id": "b"})] {
            assert!(serde_json::from_value::<NodeOp>(wrong.clone()).is_err(), "{wrong}");
        }
        // The style edit's schema names the style fields.
        let schema = serde_json::to_value(schemars::schema_for!(NodeOp)).unwrap();
        let style = schema["oneOf"].as_array().unwrap().iter().find(|v| v["properties"]["op"]["const"] == "style").unwrap();
        assert_eq!(style["properties"]["set"]["propertyNames"]["enum"], json!(STYLE_FIELDS));
        let copy = schema["oneOf"].as_array().unwrap().iter().find(|v| v["properties"]["op"]["const"] == "copy").unwrap();
        assert_eq!(copy["required"], json!(["op", "id"]));
        assert!(copy["properties"].get("as").is_some());
    }
}
