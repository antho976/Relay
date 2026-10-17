//! A design's node model (`design.*`, BUS.md §10.18): the pages, frames, shapes, text, stickies,
//! pen paths and images a design is made of, the edits `design.apply` takes, and the rules that
//! patch and validate a node. The engine stores nodes by these rules and the native client
//! applies an edit with the same ones before the engine answers, so both read a node alike.
//!
//! Geometry is in pixels. A node's `x`/`y` are relative to its parent's top-left corner (a
//! page's origin is 0,0), so moving a frame moves everything in it. Children paint in order,
//! the last on top.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Most nodes one design holds.
pub const MAX_NODES: usize = 5_000;
/// Most edits one `design.apply` carries.
pub const MAX_OPS: usize = 2_000;
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
    /// Nodes moved together; it paints nothing itself.
    Group,
    Rect,
    Ellipse,
    /// A straight line through `points`; `arrow_start`/`arrow_end` put heads on it.
    Line,
    Text,
    /// A note card: `fill` paper with `text` on it.
    Sticky,
    /// A freehand stroke through `points`.
    Path,
    /// A picture from `design.asset.add`, named by `src`.
    Image,
    /// A regular polygon (`sides`: 3 a triangle, 4 a diamond, 6 a hexagon…) or, with `star`, a
    /// star of `sides` points. Its first point is at the top centre.
    Polygon,
    /// An interface icon from Relay's set (`icon`, one of [`ICONS`]), drawn in `stroke`.
    Icon,
}

impl NodeKind {
    /// Whether nodes can sit inside this one.
    pub fn holds_children(self) -> bool {
        matches!(self, NodeKind::Page | NodeKind::Frame | NodeKind::Group)
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignGradientStop")]
pub struct Stop {
    /// 0 at the start, 1 at the end.
    pub at: f64,
    /// `#RRGGBB` or `#RRGGBBAA`.
    pub color: String,
}

/// A gradient fill, painted instead of `fill`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignGradient")]
pub struct Gradient {
    #[serde(default = "linear")]
    pub kind: GradientKind,
    /// Direction of a linear gradient in degrees, as CSS reads it: 180 (the default) runs top
    /// to bottom, 90 left to right.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// 2 to 8 colours along it.
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
    /// Shrink to the content: a text's words, or a laid-out frame's children and padding.
    Hug,
    /// Stretch to the room left in a laid-out parent (along its axis), or to its full depth
    /// (across it).
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
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub blur: f64,
    #[serde(default)]
    pub spread: f64,
    /// `#RRGGBB` or `#RRGGBBAA`.
    pub color: String,
}

/// One node of a design. Every field but `kind` is optional on input; an absent style field
/// takes the kind's default (a white frame, a grey rect, dark 16px Geist text, a yellow sticky).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignNode")]
pub struct Node {
    /// Unique in the design: letters, digits, `-`, `_`, `.` or `:`, at most 64. Give your own
    /// (`"hero"`, `"hero-title"`) to refer to it later in the same `design.apply`; left out,
    /// the engine assigns one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// The page, frame or group this node sits in. Required for everything but a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub kind: NodeKind,
    /// Shown in the layer list and above a frame on the canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Left edge, relative to the parent's top-left corner.
    #[serde(default)]
    pub x: f64,
    /// Top edge, relative to the parent's top-left corner.
    #[serde(default)]
    pub y: f64,
    #[serde(default, alias = "width")]
    pub w: f64,
    #[serde(default, alias = "height")]
    pub h: f64,
    /// Background: `#RRGGBB` or `#RRGGBBAA`. A page's fill is the canvas colour; a text's
    /// colour is `color`, not this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Outline colour; drawn `stroke_width` wide (default 1) inside the edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    /// Corner radius of a frame, rect, sticky or image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// 0 to 1, of the node and everything in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// A drop shadow under a frame, rect, ellipse, sticky or image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    /// Whether a frame hides what overflows it (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Locked nodes cannot be selected or moved on the canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,

    /// The words of a text or sticky. Newlines break lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Text colour (default `#1A1A1A`): `#RRGGBB` or `#RRGGBBAA`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Font family (default `Geist`; `Geist Mono` and `Sora` are always there).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// In px (default 16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 100 to 900 (default 400; 600 is semibold, 700 bold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<TextAlign>,
    /// Line height as a multiple of the font size (default 1.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f64>,
    /// Extra space between letters, in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f64>,

    /// Width rule (default `fixed`; a text `hug`s its words until given a width, then wraps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w_sizing: Option<Sizing>,
    /// Height rule (default `fixed`; a text `hug`s its lines).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_sizing: Option<Sizing>,
    /// A frame's auto layout: `row` or `column` places its children one after another, `gap`
    /// apart inside `padding`, ignoring their `x`/`y`. Default `none`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Space between laid-out children, in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f64>,
    /// Inner space of a laid-out frame in CSS order: `[all]`, `[vertical, horizontal]` or
    /// `[top, right, bottom, left]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justify: Option<Justify>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Items>,

    /// A line's or path's points, relative to the node's `x`/`y`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<[f64; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_start: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_end: Option<bool>,

    /// An image's picture: the `src` `design.asset.add` returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<ImageFit>,

    /// Clockwise turn in degrees about the node's centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// A polygon's corners (3 to 24, default 3), or a star's points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sides: Option<u8>,
    /// Makes a polygon a star: its inner points at this fraction of the outer (0.05 to 0.95;
    /// 0.4 is a classic star).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<f64>,
    /// A gradient painted instead of `fill`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// Corner radii top-left, top-right, bottom-right, bottom-left, instead of `radius`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radii: Option<[f64; 4]>,
    /// The stroke's dash pattern in px: `[6, 4]` dashes, `[1, 4]` dots. Absent is solid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Vec<f64>>,
    /// Blur of the whole node, in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<f64>,
    /// An icon node's icon: one of the names in the `ICONS` list (home, search, settings,
    /// user, bell, mail, heart, star, plus, check, x, chevron-*, arrow-*, menu, trash, edit,
    /// calendar, clock, image, file, folder, shopping-cart, credit-card, globe, map-pin, phone,
    /// message, send, filter, sliders, grid, list, play, pause, refresh, lock, eye, info,
    /// alert, help, sun, moon, wifi, battery, terminal, code, zap, cloud, bar-chart…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// A connector's start: the node it is attached to. The line then runs from that node's
    /// edge and follows it when it moves; `points` are only where it was last drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// A connector's end: the node it is attached to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// How a line runs between its ends (default `straight`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
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
        node: Node,
        /// Its place among the parent's children, 0 at the bottom; absent puts it on top.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
    },
    /// Change some fields of a node: each key of `set` is a node field other than `id`, `kind`
    /// and `parent` (use `move`); `null` puts a field back to its default.
    Update { id: String, set: Map<String, Value> },
    /// Put a node in another parent, or at another place among its siblings.
    Move {
        id: String,
        /// The new parent; absent keeps the current one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        /// 0 at the bottom; absent puts it on top.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
    },
    /// Delete a node and everything inside it. A design keeps at least one page.
    Delete { id: String },
}

impl NodeOp {
    /// The node the edit names; empty for an `add` whose node has no id yet.
    pub fn target(&self) -> &str {
        match self {
            NodeOp::Add { node, .. } => &node.id,
            NodeOp::Update { id, .. } | NodeOp::Move { id, .. } | NodeOp::Delete { id } => id,
        }
    }
}

/// The field a patch key names: `width` and `height` are `w` and `h`.
fn canonical_key(key: &str) -> &str {
    match key {
        "width" => "w",
        "height" => "h",
        other => other,
    }
}

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
        }
    }

    /// This node with `set` applied (`NodeOp::Update`), validated. The error names the field.
    /// `width` and `height` are read as `w` and `h`, as an `add` reads them.
    pub fn patched(&self, set: &Map<String, Value>) -> Result<Node, String> {
        let Value::Object(mut fields) = serde_json::to_value(self).map_err(|e| e.to_string())? else {
            return Err("a node serializes to an object".into());
        };
        for (key, value) in set {
            let key = &canonical_key(key).to_string();
            match key.as_str() {
                "id" | "kind" | "parent" => {
                    return Err(format!("{key} cannot change in an update; {}", match key.as_str() {
                        "parent" => "use a move",
                        _ => "delete the node and add another",
                    }))
                }
                _ if value.is_null() => {
                    fields.remove(key);
                }
                _ => {
                    fields.insert(key.clone(), value.clone());
                }
            }
        }
        let node: Node = serde_json::from_value(Value::Object(fields)).map_err(|e| e.to_string())?;
        node.validate()?;
        Ok(node)
    }

    /// The fields `set` would change, at their current values (`null` where absent): the
    /// update that undoes it.
    pub fn before(&self, set: &Map<String, Value>) -> Map<String, Value> {
        let current = serde_json::to_value(self).unwrap_or(Value::Null);
        set.keys()
            .map(|key| canonical_key(key))
            .map(|key| (key.to_string(), current.get(key).cloned().unwrap_or(Value::Null)))
            .collect()
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
            if let Some(c) = color.as_deref().filter(|c| parse_color(c).is_none()) {
                return Err(format!("{field}: {c:?} is not #RGB, #RRGGBB or #RRGGBBAA"));
            }
        }
        if let Some(shadow) = &self.shadow {
            if parse_color(&shadow.color).is_none() {
                return Err(format!("shadow.color: {:?} is not #RRGGBB or #RRGGBBAA", shadow.color));
            }
            if [shadow.x, shadow.y, shadow.blur, shadow.spread].iter().any(|v| !v.is_finite() || v.abs() > 10_000.0)
                || shadow.blur < 0.0
            {
                return Err("shadow: x, y and spread within ±10000, blur from 0".into());
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
        if let Some(g) = &self.gradient {
            if !(2..=8).contains(&g.stops.len()) {
                return Err("gradient: give 2 to 8 stops".into());
            }
            for stop in &g.stops {
                if !stop.at.is_finite() || !(0.0..=1.0).contains(&stop.at) {
                    return Err(format!("gradient: a stop's at ({}) is outside 0 to 1", stop.at));
                }
                if parse_color(&stop.color).is_none() {
                    return Err(format!("gradient: {:?} is not #RGB, #RRGGBB or #RRGGBBAA", stop.color));
                }
            }
            if g.angle.is_some_and(|a| !a.is_finite()) {
                return Err("gradient: angle must be a number".into());
            }
        }
        if self.radii.is_some_and(|r| r.iter().any(|v| !v.is_finite() || !(0.0..=MAX_COORD).contains(v))) {
            return Err(format!("radii: four numbers from 0 to {MAX_COORD}"));
        }
        if let Some(dash) = &self.dash {
            if dash.is_empty() || dash.len() > 8 || dash.iter().any(|v| !v.is_finite() || !(0.0..=1000.0).contains(v)) || dash.iter().all(|v| *v == 0.0) {
                return Err("dash: 1 to 8 lengths from 0 to 1000, not all 0".into());
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
        if let Some(name) = self.icon.as_deref().filter(|n| !ICONS.contains(n)) {
            return Err(format!("icon: {name:?} is not one of Relay's icons ({})", ICONS.join(", ")));
        }
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
        let typo = json!({"fil": "#fff"}).as_object().unwrap().clone();
        assert!(r.patched(&typo).unwrap_err().contains("fil"));
        let bad = json!({"fill": "red"}).as_object().unwrap().clone();
        assert!(r.patched(&bad).unwrap_err().starts_with("fill"));
        let neg = json!({"w": -1}).as_object().unwrap().clone();
        assert!(r.patched(&neg).unwrap_err().starts_with("w:"));
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
}
