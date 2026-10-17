//! `design.code` (BUS.md §10.18): one node of a design and everything in it written out as a
//! static HTML fragment and the CSS for it, so what an agent or the person drew can be taken
//! into a codebase as a start.
//!
//! It follows the canvas's rules (`relay_client::design::layout`, the native painter): a frame
//! laid out as a row or column is a flexbox, `hug` is `max-content` and `fill` takes the room
//! left (`flex: 1`) along its parent's axis or stretches across it; any other parent places its
//! children absolutely at their `x`/`y`. Kinds the box model cannot draw — polygons, vectors,
//! icons, lines and pen paths — are inline SVG. Every element has a class named after its layer
//! (slugified, made unique) and every style is a rule in the CSS, none inline. Hidden nodes and
//! comments are left out: they are not part of what the design shows.

use relay_bus::design::{
    icon_path, parse_color, GradientKind, ImageFit, Items, Justify, Layout, ListStyle, Node, NodeKind, Route, Sizing, Span,
    TextAlign,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// The kinds' default paints, as the canvas has them.
const FRAME_FILL: &str = "#FFFFFF";
const SHAPE_FILL: &str = "#D9D9D9";
const STICKY_FILL: &str = "#FFE58A";
const STROKE: &str = "#8C8C8C";
const CANVAS: &str = "#1E1D1B";
const DARK_TEXT: &str = "#1A1A1A";
const LIGHT_TEXT: &str = "#EDE9E2";
/// A span's link, when it names no colour.
const ACCENT: &str = "#4C9DFF";
/// A sticky's inner margin and the size it starts at; a shape label's inset from the sides.
const STICKY_PAD: f64 = 16.0;
const STICKY_SIZE: f64 = 200.0;
const LABEL_INSET: f64 = 12.0;

/// What `design.code` answers.
#[derive(Debug, Default, PartialEq)]
pub struct Code {
    pub html: String,
    pub css: String,
    /// The `src`s the page names by path (`design-assets/<src>`), each once.
    pub assets: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum Refusal {
    NotFound(String),
    Page(String),
}

/// `root` of `nodes` (a design's, in any order: parents are found by id) as HTML and CSS. A
/// picture's URL is `asset(src)`: a `design-assets/` path, or a `data:` URL when inlined.
pub fn generate(nodes: &[Node], root: &str, asset: &mut dyn FnMut(&str) -> String) -> Result<Code, Refusal> {
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let Some(top) = by_id.get(root).copied() else {
        return Err(Refusal::NotFound(format!("the design has no node {root:?}")));
    };
    if top.kind == NodeKind::Page {
        return Err(Refusal::Page(format!("{root:?} is a page; give a frame on it")));
    }
    let mut children: HashMap<&str, Vec<&Node>> = HashMap::new();
    for n in nodes {
        if let Some(parent) = n.parent.as_deref() {
            children.entry(parent).or_default().push(n);
        }
    }
    let mut g = Gen { by_id, children, taken: HashSet::new(), html: String::new(), css: String::new(), assets: Vec::new(), asset };
    let class = g.class_for(top);
    let _ = writeln!(g.css, ".{class}, .{class} * {{\n  box-sizing: border-box;\n  margin: 0;\n}}\n");
    g.node(top, Place::Root, 0, Some(class));
    let css = format!("{}\n", g.css.trim_end());
    Ok(Code { html: g.html, css, assets: g.assets })
}

/// How a node's parent places it.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    /// The node written out: it sits where the page puts it.
    Root,
    /// At its own `x`/`y` inside the parent.
    Free,
    /// One after another in a laid-out frame; `stretch` when the frame stretches its children
    /// across.
    Flex { row: bool, stretch: bool, first: bool, gap: f64 },
}

struct Gen<'a, 'b> {
    by_id: HashMap<&'a str, &'a Node>,
    children: HashMap<&'a str, Vec<&'a Node>>,
    taken: HashSet<String>,
    html: String,
    css: String,
    assets: Vec<String>,
    asset: &'b mut dyn FnMut(&str) -> String,
}

/// A CSS rule under way.
#[derive(Default)]
struct Rule(Vec<(&'static str, String)>);

impl Rule {
    fn set(&mut self, prop: &'static str, value: impl Into<String>) {
        self.0.push((prop, value.into()));
    }
}

/// A run of a text's words and the element it sits in, if any.
struct Run {
    text: String,
    /// `(tag, class, href)`.
    wrap: Option<(&'static str, Option<String>, Option<String>)>,
}

/// `v` px, without trailing zeros.
fn px(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    format!("{}px", num(v))
}

fn num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// Text for HTML content.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Text for a quoted HTML attribute.
pub fn attr(text: &str) -> String {
    esc(text).replace('"', "&quot;").replace('\'', "&#39;")
}

/// Text inside a double-quoted CSS string, which can neither end the string nor the `<style>`
/// it may be pasted into.
fn css_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\3c "),
            '>' => out.push_str("\\3e "),
            '\n' | '\r' => out.push_str("\\a "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A link a page may carry: a web, mail or phone address, an anchor or a relative path; never
/// `javascript:` or the like.
fn safe_href(link: &str) -> Option<&str> {
    let link = link.trim();
    let scheme_end = link.find([':', '/', '?', '#']);
    match scheme_end {
        Some(i) if link[i..].starts_with(':') => {
            let scheme = link[..i].to_ascii_lowercase();
            matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel").then_some(link)
        }
        _ => (!link.is_empty()).then_some(link),
    }
}

/// A class from a layer's name: lowercase letters, digits and dashes, never starting with a
/// digit.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').chars().take(40).collect::<String>().trim_end_matches('-').to_string();
    match out.chars().next() {
        None => String::new(),
        Some(c) if c.is_ascii_digit() => format!("n-{out}"),
        Some(_) => out,
    }
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
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
        NodeKind::Vector => "vector",
        NodeKind::Comment => "comment",
    }
}

/// The fill a node paints, its kind's default when it names none.
fn fill_of(n: &Node) -> Option<&str> {
    n.fill.as_deref().or(match n.kind {
        NodeKind::Frame => Some(FRAME_FILL),
        NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon => Some(SHAPE_FILL),
        NodeKind::Sticky => Some(STICKY_FILL),
        NodeKind::Page => Some(CANVAS),
        _ => None,
    })
}

fn luminance(hex: &str) -> f64 {
    let [r, g, b, _] = parse_color(hex).unwrap_or([0, 0, 0, 255]);
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast_on(ground: &str) -> &'static str {
    if luminance(ground) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }
}

fn w_sizing(n: &Node) -> Sizing {
    n.w_sizing.unwrap_or(if n.kind == NodeKind::Text && n.w <= 0.0 { Sizing::Hug } else { Sizing::Fixed })
}

fn h_sizing(n: &Node) -> Sizing {
    n.h_sizing.unwrap_or(if n.kind == NodeKind::Text { Sizing::Hug } else { Sizing::Fixed })
}

/// Whether a node can hug its content: a text its words, a laid-out frame its children.
fn hugs(n: &Node) -> bool {
    n.kind == NodeKind::Text || n.layout_mode() != Layout::None
}

fn gradient_css(g: &relay_bus::design::Gradient) -> String {
    let stops: Vec<String> = g.stops.iter().map(|s| format!("{} {}%", s.color, num(s.at * 100.0))).collect();
    match g.kind {
        GradientKind::Linear => format!("linear-gradient({}deg, {})", num(g.angle.unwrap_or(180.0)), stops.join(", ")),
        GradientKind::Radial => format!("radial-gradient(ellipse farthest-side at center, {})", stops.join(", ")),
    }
}

/// The CSS filter functions for the node's blur and colour adjustments, which the canvas runs
/// as one colour matrix: brightness and contrast scale about 1, saturation likewise, hue turns.
fn filter_fns(n: &Node, shadow_as_drop: bool) -> Vec<String> {
    let mut out = Vec::new();
    if shadow_as_drop {
        if let Some(s) = &n.shadow {
            out.push(format!("drop-shadow({} {} {} {})", px(s.x), px(s.y), px(s.blur), s.color));
        }
    }
    if let Some(b) = n.blur.filter(|b| *b > 0.0) {
        out.push(format!("blur({})", px(b)));
    }
    if let Some(f) = n.filters.as_ref().filter(|f| !f.is_none()) {
        if f.brightness != 0.0 {
            out.push(format!("brightness({})", num(1.0 + f.brightness)));
        }
        if f.contrast != 0.0 {
            out.push(format!("contrast({})", num(1.0 + f.contrast)));
        }
        if f.saturation != 0.0 {
            out.push(format!("saturate({})", num(1.0 + f.saturation)));
        }
        if f.hue != 0.0 {
            out.push(format!("hue-rotate({}deg)", num(f.hue)));
        }
        if f.sepia != 0.0 {
            out.push(format!("sepia({})", num(f.sepia)));
        }
        if f.invert != 0.0 {
            out.push(format!("invert({})", num(f.invert)));
        }
    }
    out
}

/// A polygon's (or star's) corners in a `w`×`h` box, first at the top centre, clockwise.
fn polygon_points(n: &Node, w: f64, h: f64) -> Vec<(f64, f64)> {
    let sides = n.sides.unwrap_or(if n.star.is_some() { 5 } else { 3 }).clamp(3, 24) as usize;
    let count = if n.star.is_some() { sides * 2 } else { sides };
    (0..count)
        .map(|i| {
            let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / count as f64;
            let k = match n.star {
                Some(inner) if i % 2 == 1 => inner,
                _ => 1.0,
            };
            (w / 2.0 + w / 2.0 * k * angle.cos(), h / 2.0 + h / 2.0 * k * angle.sin())
        })
        .collect()
}

fn dash_attr(n: &Node) -> String {
    n.dash.as_ref().map_or(String::new(), |d| format!(" stroke-dasharray=\"{}\"", d.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ")))
}

impl<'a> Gen<'a, '_> {
    /// A class for `n`, unique in this output.
    fn class_for(&mut self, n: &Node) -> String {
        let base = [n.name.as_deref(), Some(n.id.as_str())].into_iter().flatten().map(slug).find(|s| !s.is_empty())
            .unwrap_or_else(|| kind_name(n.kind).to_string());
        let mut class = base.clone();
        let mut k = 2;
        while !self.taken.insert(class.clone()) {
            class = format!("{base}-{k}");
            k += 1;
        }
        class
    }

    /// A class nobody named: `base-suffix`, unique.
    fn sub_class(&mut self, base: &str, suffix: &str) -> String {
        let want = format!("{base}-{suffix}");
        let mut class = want.clone();
        let mut k = 2;
        while !self.taken.insert(class.clone()) {
            class = format!("{want}-{k}");
            k += 1;
        }
        class
    }

    fn rule(&mut self, selector: &str, rule: Rule) {
        if rule.0.is_empty() {
            return;
        }
        let _ = writeln!(self.css, "{selector} {{");
        for (prop, value) in rule.0 {
            let _ = writeln!(self.css, "  {prop}: {value};");
        }
        self.css.push_str("}\n\n");
    }

    fn url(&mut self, src: &str) -> String {
        let url = (self.asset)(src);
        if !url.starts_with("data:") && !self.assets.iter().any(|a| a == src) {
            self.assets.push(src.to_string());
        }
        url
    }

    fn kids(&self, id: &str) -> Vec<&'a Node> {
        self.children.get(id).map_or(Vec::new(), |k| {
            k.iter().copied().filter(|c| !c.is_hidden() && c.kind != NodeKind::Comment).collect()
        })
    }

    /// The colour a text takes when it names none: dark on a light ground, light on a dark one,
    /// the ground being the node itself for a shape's label, else the nearest container that
    /// paints one.
    fn text_color(&self, n: &Node) -> String {
        if let Some(c) = &n.color {
            return c.clone();
        }
        if matches!(n.kind, NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) {
            let ground = n.gradient.as_ref().and_then(|g| g.stops.first()).map(|s| s.color.as_str()).or_else(|| fill_of(n)).unwrap_or(CANVAS);
            return contrast_on(ground).into();
        }
        let mut at = n.parent.as_deref();
        let mut depth = 0;
        while let (Some(id), true) = (at, depth < 64) {
            let Some(a) = self.by_id.get(id) else { break };
            if let Some(fill) = fill_of(a).filter(|f| parse_color(f).is_some_and(|c| c[3] >= 128)) {
                return contrast_on(fill).into();
            }
            at = a.parent.as_deref();
            depth += 1;
        }
        contrast_on(CANVAS).into()
    }

    /// Where the node sits and how big it is.
    fn place(&self, n: &Node, place: Place, rule: &mut Rule) {
        match place {
            Place::Root => rule.set("position", "relative"),
            Place::Free => {
                rule.set("position", "absolute");
                rule.set("left", px(n.x));
                rule.set("top", px(n.y));
            }
            Place::Flex { row, first, gap, .. } => {
                rule.set("position", "relative");
                rule.set("flex-shrink", "0");
                if gap < 0.0 && !first {
                    rule.set(if row { "margin-left" } else { "margin-top" }, px(gap));
                }
            }
        }
        let sticky = n.kind == NodeKind::Sticky;
        let own_w = if sticky && n.w <= 0.0 { STICKY_SIZE } else { n.w };
        let own_h = if sticky && n.h <= 0.0 { STICKY_SIZE } else { n.h };
        let (ws, hs) = (w_sizing(n), h_sizing(n));
        let (row_parent, col_parent, stretch) = match place {
            Place::Flex { row, stretch, .. } => (row, !row, stretch),
            _ => (false, false, false),
        };
        let mut flexed = false;
        // Across a column: filling or stretched, the column decides the width.
        match ws {
            Sizing::Fill if row_parent => {
                rule.set("flex", "1 1 0");
                rule.set("min-width", "0");
                flexed = true;
            }
            Sizing::Fill if col_parent => rule.set("align-self", "stretch"),
            _ if col_parent && stretch => {}
            Sizing::Hug if hugs(n) => rule.set("width", "max-content"),
            _ => rule.set("width", px(own_w)),
        }
        match hs {
            Sizing::Fill if col_parent => {
                if !flexed {
                    rule.set("flex", "1 1 0");
                }
                rule.set("min-height", "0");
            }
            Sizing::Fill if row_parent => rule.set("align-self", "stretch"),
            _ if row_parent && stretch => {}
            Sizing::Hug if hugs(n) => {}
            _ if sticky => rule.set("min-height", px(own_h)),
            _ => rule.set("height", px(own_h)),
        }
    }

    /// Opacity, filters and the turn and flip every kind shares.
    fn effects(&self, n: &Node, rule: &mut Rule, shadow_as_drop: bool) {
        if let Some(o) = n.opacity.filter(|o| *o < 1.0) {
            rule.set("opacity", num(o));
        }
        let filters = filter_fns(n, shadow_as_drop);
        if !filters.is_empty() {
            rule.set("filter", filters.join(" "));
        }
        let mut transform = Vec::new();
        if let Some(r) = n.rotation.filter(|r| *r != 0.0) {
            transform.push(format!("rotate({}deg)", num(r)));
        }
        let (fx, fy) = (n.flip_x == Some(true), n.flip_y == Some(true));
        if fx || fy {
            transform.push(format!("scale({}, {})", if fx { -1 } else { 1 }, if fy { -1 } else { 1 }));
        }
        if !transform.is_empty() {
            rule.set("transform", transform.join(" "));
        }
    }

    /// Fill, picture or gradient, corners, border and shadow of a box-drawn node.
    fn paint_box(&mut self, n: &Node, rule: &mut Rule) {
        if let Some(src) = n.src.as_deref() {
            let url = self.url(src);
            rule.set("background-image", format!("url({})", css_str(&url)));
            rule.set("background-repeat", "no-repeat");
            match n.crop {
                // The part shown, stretched to the box.
                Some([x, y, w, h]) => {
                    rule.set("background-size", format!("{}% {}%", num(100.0 / w), num(100.0 / h)));
                    let at = |o: f64, s: f64| if s >= 1.0 { 0.0 } else { o / (1.0 - s) * 100.0 };
                    rule.set("background-position", format!("{}% {}%", num(at(x, w)), num(at(y, h))));
                }
                None => {
                    rule.set("background-size", match n.fit.unwrap_or(ImageFit::Cover) {
                        ImageFit::Cover => "cover",
                        ImageFit::Contain => "contain",
                        ImageFit::Stretch => "100% 100%",
                    });
                    rule.set("background-position", "center");
                }
            }
        } else if let Some(g) = &n.gradient {
            rule.set("background-image", gradient_css(g));
        } else if let Some(fill) = fill_of(n).filter(|f| n.kind != NodeKind::Image && parse_color(f).is_some_and(|c| c[3] > 0)) {
            rule.set("background-color", fill.to_string());
        }
        if n.kind == NodeKind::Ellipse {
            rule.set("border-radius", "50%");
        } else if let Some([a, b, c, d]) = n.radii {
            rule.set("border-radius", format!("{} {} {} {}", px(a), px(b), px(c), px(d)));
        } else if let Some(r) = n.radius.filter(|r| *r > 0.0) {
            rule.set("border-radius", px(r));
        }
        if let Some(stroke) = &n.stroke {
            let w = n.stroke_width.unwrap_or(1.0);
            if w > 0.0 {
                let style = match n.dash.as_deref() {
                    None => "solid",
                    Some([first, ..]) if *first <= w => "dotted",
                    Some(_) => "dashed",
                };
                rule.set("border", format!("{} {style} {stroke}", px(w)));
            }
        }
        match &n.shadow {
            Some(s) => rule.set("box-shadow", format!("{} {} {} {} {}", px(s.x), px(s.y), px(s.blur), px(s.spread), s.color)),
            None if n.kind == NodeKind::Sticky => rule.set("box-shadow", "0 3px 10px rgba(0, 0, 0, 0.22)"),
            None => {}
        }
    }

    /// Type: family, size, weight, style, decoration, alignment, spacing, colour.
    fn type_rules(&self, n: &Node, rule: &mut Rule, centred: bool) {
        let family = n.font.as_deref().unwrap_or("Geist");
        let fallback = if family.to_ascii_lowercase().contains("mono") { "ui-monospace, monospace" } else { "system-ui, sans-serif" };
        rule.set("font-family", format!("{}, {fallback}", css_str(family)));
        rule.set("font-size", px(n.font_size.unwrap_or(16.0)));
        rule.set("font-weight", n.font_weight.unwrap_or(400).to_string());
        if n.italic == Some(true) {
            rule.set("font-style", "italic");
        }
        let lines: Vec<&str> = [(n.underline, "underline"), (n.strike, "line-through")].into_iter()
            .filter(|(on, _)| *on == Some(true)).map(|(_, l)| l).collect();
        if !lines.is_empty() {
            rule.set("text-decoration", lines.join(" "));
        }
        let align = n.align.unwrap_or(if centred { TextAlign::Center } else { TextAlign::Left });
        if align != TextAlign::Left {
            rule.set("text-align", match align { TextAlign::Left => "left", TextAlign::Center => "center", TextAlign::Right => "right" });
        }
        rule.set("line-height", num(n.line_height.unwrap_or(1.3)));
        if let Some(s) = n.letter_spacing.filter(|s| *s != 0.0) {
            rule.set("letter-spacing", px(s));
        }
        rule.set("color", self.text_color(n));
    }

    /// A text's words as runs: its spans while they still spell its text, each styled one with
    /// a class of its own.
    fn runs(&mut self, n: &Node, class: &str) -> Vec<Run> {
        let text = n.text.clone().unwrap_or_default();
        let Some(spans) = n.live_spans().map(<[Span]>::to_vec) else {
            return vec![Run { text, wrap: None }];
        };
        let mut out = Vec::with_capacity(spans.len());
        for s in spans {
            let mut rule = Rule::default();
            if let Some(w) = s.weight {
                rule.set("font-weight", w.to_string());
            }
            if s.italic == Some(true) {
                rule.set("font-style", "italic");
            }
            let href = s.link.as_deref().and_then(safe_href).map(str::to_string);
            let lines: Vec<&str> = [(s.underline == Some(true) || href.is_some(), "underline"), (s.strike == Some(true), "line-through")]
                .into_iter().filter(|(on, _)| *on).map(|(_, l)| l).collect();
            if !lines.is_empty() {
                rule.set("text-decoration", lines.join(" "));
            }
            match (&s.color, &href) {
                (Some(c), _) => rule.set("color", c.clone()),
                (None, Some(_)) => rule.set("color", ACCENT),
                _ => {}
            }
            if let Some(size) = s.size {
                rule.set("font-size", px(size));
            }
            let code = s.code == Some(true);
            match (&s.font, code) {
                (Some(f), _) => rule.set("font-family", format!("{}, system-ui, sans-serif", css_str(f))),
                (None, true) => rule.set("font-family", "\"Geist Mono\", ui-monospace, monospace"),
                _ => {}
            }
            if code {
                rule.set("background-color", "rgba(127, 127, 127, 0.18)");
                rule.set("border-radius", "3px");
                rule.set("padding", "0 0.2em");
            }
            let tag = if href.is_some() { "a" } else if code { "code" } else { "span" };
            if rule.0.is_empty() && tag == "span" {
                out.push(Run { text: s.text, wrap: None });
                continue;
            }
            let span_class = (!rule.0.is_empty()).then(|| self.sub_class(class, "span"));
            if let Some(c) = &span_class {
                self.rule(&format!(".{c}"), rule);
            }
            out.push(Run { text: s.text, wrap: Some((tag, span_class, href)) });
        }
        out
    }

    /// Runs as HTML, one string per line of the text.
    fn lines(runs: Vec<Run>) -> Vec<String> {
        let mut lines = vec![String::new()];
        for run in runs {
            for (i, part) in run.text.split('\n').enumerate() {
                if i > 0 {
                    lines.push(String::new());
                }
                if part.is_empty() {
                    continue;
                }
                let line = lines.last_mut().expect("one line at least");
                match &run.wrap {
                    None => line.push_str(&esc(part)),
                    Some((tag, class, href)) => {
                        let _ = write!(line, "<{tag}");
                        if let Some(c) = class {
                            let _ = write!(line, " class=\"{c}\"");
                        }
                        if let Some(h) = href {
                            let _ = write!(line, " href=\"{}\"", attr(h));
                        }
                        let _ = write!(line, ">{}</{tag}>", esc(part));
                    }
                }
            }
        }
        lines
    }

    /// The words inside a text, sticky or label element: plain lines, or list items.
    fn words(&mut self, n: &Node, class: &str) -> (String, Option<&'static str>) {
        let runs = self.runs(n, class);
        let lines = Self::lines(runs);
        let Some(list) = n.list else {
            return (lines.join("\n"), None);
        };
        let mut out = String::new();
        for line in lines {
            out.push_str("<li>");
            if list == ListStyle::Check {
                let (ticked, rest) = match (line.strip_prefix("[x] ").or_else(|| line.strip_prefix("[X] ")), line.strip_prefix("[ ] ")) {
                    (Some(rest), _) => (true, rest.to_string()),
                    (None, Some(rest)) => (false, rest.to_string()),
                    _ => (false, line.clone()),
                };
                out.push_str(if ticked { "<input type=\"checkbox\" disabled checked> " } else { "<input type=\"checkbox\" disabled> " });
                out.push_str(&rest);
            } else {
                out.push_str(&line);
            }
            out.push_str("</li>");
        }
        (out, Some(if list == ListStyle::Number { "ol" } else { "ul" }))
    }

    fn list_rules(&mut self, class: &str, list: ListStyle, rule: &mut Rule) {
        if list == ListStyle::Check {
            rule.set("list-style", "none");
            rule.set("padding-left", "0");
            let mut boxes = Rule::default();
            boxes.set("margin", "0 0.4em 0 0");
            self.rule(&format!(".{class} input"), boxes);
        } else {
            rule.set("padding-left", "1.4em");
        }
    }

    fn open(&mut self, depth: usize, tag: &str, class: &str, n: &Node, extra: &str) {
        let _ = write!(self.html, "{}<{tag} class=\"{class}\" data-node=\"{}\"{extra}>", "  ".repeat(depth), attr(&n.id));
    }

    /// `n` and what is inside it, at `depth` of indentation.
    fn node(&mut self, n: &Node, place: Place, depth: usize, class: Option<String>) {
        if n.is_hidden() || n.kind == NodeKind::Comment || n.kind == NodeKind::Page {
            return;
        }
        let class = class.unwrap_or_else(|| self.class_for(n));
        let mut rule = Rule::default();
        self.place(n, place, &mut rule);
        let href = n.link.as_deref().and_then(safe_href).map(|h| format!(" href=\"{}\"", attr(h)));
        match n.kind {
            NodeKind::Frame | NodeKind::Group => self.container(n, &class, rule, depth, href),
            NodeKind::Rect | NodeKind::Ellipse | NodeKind::Sticky => {
                self.paint_box(n, &mut rule);
                self.effects(n, &mut rule, false);
                let sticky = n.kind == NodeKind::Sticky;
                let has_words = n.text.as_deref().is_some_and(|t| !t.is_empty());
                if sticky {
                    rule.set("padding", px(STICKY_PAD));
                    rule.set("white-space", "pre-wrap");
                    rule.set("overflow-wrap", "anywhere");
                    self.type_rules(n, &mut rule, false);
                } else if has_words {
                    rule.set("display", "flex");
                    rule.set("align-items", "center");
                    rule.set("justify-content", "center");
                    rule.set("padding", format!("0 {}", px(LABEL_INSET)));
                }
                let (tag, extra) = match &href {
                    Some(h) => ("a", h.clone()),
                    None => ("div", String::new()),
                };
                if sticky {
                    let (words, list) = self.words(n, &class);
                    self.rule(&format!(".{class}"), rule);
                    self.open(depth, tag, &class, n, &extra);
                    match list {
                        Some(l) => {
                            let lc = self.sub_class(&class, "list");
                            let mut lr = Rule::default();
                            self.list_rules(&lc, n.list.unwrap_or(ListStyle::Bullet), &mut lr);
                            self.rule(&format!(".{lc}"), lr);
                            let _ = write!(self.html, "<{l} class=\"{lc}\">{words}</{l}>");
                        }
                        None => self.html.push_str(&words),
                    }
                    let _ = writeln!(self.html, "</{tag}>");
                } else {
                    self.rule(&format!(".{class}"), rule);
                    self.open(depth, tag, &class, n, &extra);
                    if has_words {
                        self.label(n, &class);
                    }
                    let _ = writeln!(self.html, "</{tag}>");
                }
            }
            NodeKind::Text => {
                if let Some(fill) = n.fill.as_deref() {
                    rule.set("background-color", fill.to_string());
                }
                // A text wraps once it has a width: its own, a filling share, or a stretch.
                let stretched = matches!(place, Place::Flex { row: false, stretch: true, .. });
                let wraps = w_sizing(n) != Sizing::Hug || stretched;
                rule.set("white-space", if wraps { "pre-wrap" } else { "pre" });
                self.type_rules(n, &mut rule, false);
                self.effects(n, &mut rule, false);
                if let Some(list) = n.list {
                    self.list_rules(&class, list, &mut rule);
                }
                let (words, list) = self.words(n, &class);
                self.rule(&format!(".{class}"), rule);
                let (tag, extra) = match (list, &href) {
                    (Some(l), _) => (l, String::new()),
                    (None, Some(h)) => ("a", h.clone()),
                    (None, None) => ("p", String::new()),
                };
                self.open(depth, tag, &class, n, &extra);
                let _ = writeln!(self.html, "{words}</{tag}>");
            }
            NodeKind::Image => {
                self.paint_box(n, &mut rule);
                self.effects(n, &mut rule, false);
                let alt = attr(n.name.as_deref().unwrap_or(""));
                let src = n.src.clone().unwrap_or_default();
                if n.crop.is_some() {
                    // `object-fit` cannot show a part of a picture: the part is a background.
                    self.rule(&format!(".{class}"), rule);
                    self.open(depth, "div", &class, n, &format!(" role=\"img\" aria-label=\"{alt}\""));
                    let _ = writeln!(self.html, "</div>");
                } else {
                    // The picture is the element itself; the background rules are for a box.
                    rule.0.retain(|(p, _)| !p.starts_with("background"));
                    rule.set("display", "block");
                    rule.set("object-fit", match n.fit.unwrap_or(ImageFit::Cover) {
                        ImageFit::Cover => "cover",
                        ImageFit::Contain => "contain",
                        ImageFit::Stretch => "fill",
                    });
                    let url = self.url(&src);
                    self.rule(&format!(".{class}"), rule);
                    self.open(depth, "img", &class, n, &format!(" src=\"{}\" alt=\"{alt}\"", attr(&url)));
                    self.html.push('\n');
                }
            }
            NodeKind::Polygon | NodeKind::Vector => {
                self.effects(n, &mut rule, true);
                let (w, h) = (n.w.max(1.0), n.h.max(1.0));
                let paint = self.svg_paint(n, &class);
                let shape = if n.kind == NodeKind::Polygon {
                    let pts: Vec<String> = polygon_points(n, w, h).iter().map(|(x, y)| format!("{},{}", num(*x), num(*y))).collect();
                    format!("<polygon points=\"{}\"{paint}/>", pts.join(" "))
                } else {
                    format!("<path d=\"{}\"{paint} vector-effect=\"non-scaling-stroke\"/>", attr(n.d.as_deref().unwrap_or("")))
                };
                let view = match (n.kind, n.view) {
                    (NodeKind::Vector, Some([vw, vh])) => format!("0 0 {} {}", num(vw.max(0.01)), num(vh.max(0.01))),
                    _ => format!("0 0 {} {}", num(w), num(h)),
                };
                let mut svg_rule = Rule::default();
                svg_rule.set("position", "absolute");
                svg_rule.set("left", "0");
                svg_rule.set("top", "0");
                svg_rule.set("overflow", "visible");
                let has_words = n.kind == NodeKind::Polygon && n.text.as_deref().is_some_and(|t| !t.is_empty());
                if has_words {
                    rule.set("display", "flex");
                    rule.set("align-items", "center");
                    rule.set("justify-content", "center");
                    rule.set("padding", format!("0 {}", px(LABEL_INSET)));
                }
                self.rule(&format!(".{class}"), rule);
                self.rule(&format!(".{class} > svg"), svg_rule);
                self.open(depth, "div", &class, n, "");
                let defs = self.svg_defs(n, &class);
                let _ = write!(self.html, "<svg width=\"100%\" height=\"100%\" viewBox=\"{view}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">{defs}{shape}</svg>");
                if has_words {
                    self.label(n, &class);
                }
                let _ = writeln!(self.html, "</div>");
            }
            NodeKind::Icon => {
                self.effects(n, &mut rule, false);
                rule.set("display", "block");
                rule.set("overflow", "visible");
                self.rule(&format!(".{class}"), rule);
                let color = n.stroke.clone().unwrap_or_else(|| self.text_color(n));
                let d = n.icon.as_deref().and_then(icon_path).unwrap_or("");
                self.open(depth, "svg", &class, n, &format!(
                    " viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"",
                    attr(&color), num(n.stroke_width.unwrap_or(2.0))));
                let _ = writeln!(self.html, "<path d=\"{d}\"/></svg>");
            }
            NodeKind::Line | NodeKind::Path => {
                self.effects(n, &mut rule, false);
                rule.set("display", "block");
                rule.set("overflow", "visible");
                // Points may run past the box, as on the canvas.
                rule.0.retain(|(p, _)| *p != "width" && *p != "height");
                rule.set("width", px(n.w.max(1.0)));
                rule.set("height", px(n.h.max(1.0)));
                self.rule(&format!(".{class}"), rule);
                let color = attr(n.stroke.as_deref().unwrap_or(STROKE));
                let width = num(n.stroke_width.unwrap_or(2.0).max(0.5));
                let head = format!("{class}-head");
                let (start, end) = (n.arrow_start == Some(true), n.arrow_end == Some(true));
                let defs = if start || end {
                    format!("<defs><marker id=\"{head}\" viewBox=\"0 0 10 10\" refX=\"8\" refY=\"5\" markerWidth=\"5\" markerHeight=\"5\" orient=\"auto-start-reverse\"><path d=\"M0 0 10 5 0 10z\" fill=\"{color}\"/></marker></defs>")
                } else {
                    String::new()
                };
                let markers = format!("{}{}", if start { format!(" marker-start=\"url(#{head})\"") } else { String::new() },
                    if end { format!(" marker-end=\"url(#{head})\"") } else { String::new() });
                let d = line_d(n);
                self.open(depth, "svg", &class, n, " aria-hidden=\"true\"");
                let _ = writeln!(self.html, "{defs}<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{}{markers}/></svg>", dash_attr(n));
            }
            NodeKind::Comment | NodeKind::Page => {}
        }
    }

    /// A frame or group: its box, then its children placed by its layout.
    fn container(&mut self, n: &Node, class: &str, mut rule: Rule, depth: usize, href: Option<String>) {
        let layout = n.layout_mode();
        if n.kind == NodeKind::Frame {
            self.paint_box(n, &mut rule);
            if n.clip != Some(false) {
                rule.set("overflow", "hidden");
            }
        }
        self.effects(n, &mut rule, false);
        let kids = self.kids(&n.id);
        let mut place = Place::Free;
        if layout != Layout::None {
            let row = layout == Layout::Row;
            let gap = n.gap.unwrap_or(0.0);
            let items = n.items.unwrap_or(Items::Start);
            rule.set("display", "flex");
            rule.set("flex-direction", if row { "row" } else { "column" });
            if gap > 0.0 {
                rule.set("gap", px(gap));
            }
            let [t, r, b, l] = n.padding_edges();
            if [t, r, b, l].iter().any(|v| *v > 0.0) {
                rule.set("padding", format!("{} {} {} {}", px(t), px(r), px(b), px(l)));
            }
            rule.set("justify-content", match n.justify.unwrap_or(Justify::Start) {
                Justify::Start => "flex-start",
                Justify::Center => "center",
                Justify::End => "flex-end",
                Justify::SpaceBetween => "space-between",
            });
            rule.set("align-items", match items {
                Items::Start => "flex-start",
                Items::Center => "center",
                Items::End => "flex-end",
                Items::Stretch => "stretch",
            });
            place = Place::Flex { row, stretch: items == Items::Stretch, first: true, gap };
        }
        self.rule(&format!(".{class}"), rule);
        let (tag, extra) = match href {
            Some(h) => ("a", h),
            None => ("div", String::new()),
        };
        self.open(depth, tag, class, n, &extra);
        if kids.is_empty() {
            let _ = writeln!(self.html, "</{tag}>");
            return;
        }
        self.html.push('\n');
        for (i, kid) in kids.into_iter().enumerate() {
            let at = match place {
                Place::Flex { row, stretch, gap, .. } => Place::Flex { row, stretch, gap, first: i == 0 },
                other => other,
            };
            self.node(kid, at, depth + 1, None);
        }
        let _ = writeln!(self.html, "{}</{tag}>", "  ".repeat(depth));
    }

    /// A shape's words, centred inside it.
    fn label(&mut self, n: &Node, class: &str) {
        let lc = self.sub_class(class, "label");
        let mut rule = Rule::default();
        rule.set("position", "relative");
        rule.set("width", "100%");
        rule.set("white-space", "pre-wrap");
        self.type_rules(n, &mut rule, true);
        if let Some(list) = n.list {
            self.list_rules(&lc, list, &mut rule);
        }
        let (words, list) = self.words(n, &lc);
        self.rule(&format!(".{lc}"), rule);
        let tag = list.unwrap_or("p");
        let _ = write!(self.html, "<{tag} class=\"{lc}\">{words}</{tag}>");
    }

    /// The fill and stroke attributes of a polygon or vector.
    fn svg_paint(&mut self, n: &Node, class: &str) -> String {
        let open = n.kind == NodeKind::Vector && n.open == Some(true);
        let fill = if open {
            "none".to_string()
        } else if n.src.is_some() || n.gradient.is_some() {
            format!("url(#{class}-paint)")
        } else {
            let default = if n.kind == NodeKind::Vector { Some(SHAPE_FILL) } else { fill_of(n) };
            attr(n.fill.as_deref().or(default).unwrap_or("none"))
        };
        let mut out = format!(" fill=\"{fill}\"");
        let stroke = n.stroke.as_deref().or(if open { Some(STROKE) } else { None });
        if let Some(stroke) = stroke {
            let w = n.stroke_width.unwrap_or(if open { 2.0 } else { 1.0 });
            if w > 0.0 {
                let _ = write!(out, " stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"{}", attr(stroke), num(w), dash_attr(n));
            }
        }
        out
    }

    /// The gradient or picture a polygon or vector is filled with, as SVG paint servers.
    fn svg_defs(&mut self, n: &Node, class: &str) -> String {
        if n.kind == NodeKind::Vector && n.open == Some(true) {
            return String::new();
        }
        let id = format!("{class}-paint");
        if let Some(src) = n.src.as_deref() {
            let url = self.url(src);
            let aspect = match n.fit.unwrap_or(ImageFit::Cover) {
                ImageFit::Cover => "xMidYMid slice",
                ImageFit::Contain => "xMidYMid meet",
                ImageFit::Stretch => "none",
            };
            return format!("<defs><pattern id=\"{id}\" patternContentUnits=\"objectBoundingBox\" width=\"1\" height=\"1\"><image href=\"{}\" width=\"1\" height=\"1\" preserveAspectRatio=\"{aspect}\"/></pattern></defs>", attr(&url));
        }
        let Some(g) = &n.gradient else { return String::new() };
        let stops: String = g.stops.iter().map(|s| format!("<stop offset=\"{}\" stop-color=\"{}\"/>", num(s.at), attr(&s.color))).collect();
        match g.kind {
            GradientKind::Linear => {
                // CSS angles: 0 runs bottom to top, 90 left to right.
                let a = g.angle.unwrap_or(180.0).to_radians();
                let (dx, dy) = (a.sin() / 2.0, -a.cos() / 2.0);
                format!("<defs><linearGradient id=\"{id}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{stops}</linearGradient></defs>",
                    num(0.5 - dx), num(0.5 - dy), num(0.5 + dx), num(0.5 + dy))
            }
            GradientKind::Radial => format!("<defs><radialGradient id=\"{id}\" cx=\"0.5\" cy=\"0.5\" r=\"0.5\">{stops}</radialGradient></defs>"),
        }
    }
}

/// A line's or path's path data in its own pixels, routed as the canvas routes a connector
/// between its two ends.
fn line_d(n: &Node) -> String {
    let pts: Vec<(f64, f64)> = n.points.as_deref().unwrap_or(&[]).iter().map(|p| (p[0], p[1])).collect();
    let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else {
        return String::new();
    };
    let p = |(x, y): (f64, f64)| format!("{} {}", num(x), num(y));
    let route = if n.kind == NodeKind::Line { n.route.unwrap_or(Route::Straight) } else { Route::Straight };
    match route {
        Route::Elbow if pts.len() == 2 => {
            let corners = if (b.0 - a.0).abs() >= (b.1 - a.1).abs() {
                let mid = (a.0 + b.0) / 2.0;
                [(mid, a.1), (mid, b.1)]
            } else {
                let mid = (a.1 + b.1) / 2.0;
                [(a.0, mid), (b.0, mid)]
            };
            format!("M{} L{} L{} L{}", p(a), p(corners[0]), p(corners[1]), p(b))
        }
        Route::Curve if pts.len() == 2 => {
            let (c1, c2) = if (b.0 - a.0).abs() >= (b.1 - a.1).abs() {
                let k = (b.0 - a.0) / 2.0;
                ((a.0 + k, a.1), (b.0 - k, b.1))
            } else {
                let k = (b.1 - a.1) / 2.0;
                ((a.0, a.1 + k), (b.0, b.1 - k))
            };
            format!("M{} C{} {} {}", p(a), p(c1), p(c2), p(b))
        }
        _ => {
            let mut d = format!("M{}", p(a));
            for q in &pts[1..] {
                let _ = write!(d, " L{}", p(*q));
            }
            d
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn nodes(list: serde_json::Value) -> Vec<Node> {
        serde_json::from_value(list).unwrap()
    }

    fn code(list: serde_json::Value, root: &str) -> Code {
        generate(&nodes(list), root, &mut |src| format!("design-assets/{src}")).unwrap()
    }

    /// The declarations of the rule for `.class`.
    fn rule<'a>(css: &'a str, class: &str) -> &'a str {
        let open = css.find(&format!(".{class} {{")).unwrap_or_else(|| panic!("no rule for .{class} in\n{css}"));
        let body = &css[open..];
        &body[..body.find('}').unwrap()]
    }

    #[test]
    fn a_row_is_a_flexbox_and_its_children_share_it() {
        let c = code(json!([
            {"id": "p", "kind": "page"},
            {"id": "bar", "parent": "p", "kind": "frame", "name": "Tool bar", "x": 40, "y": 40, "w": 400, "h": 48,
             "layout": "row", "gap": 8, "padding": [8, 16], "items": "center", "justify": "space_between", "fill": "#222222", "radius": 8},
            {"id": "title", "parent": "bar", "kind": "text", "text": "Hello <world>", "font_weight": 600},
            {"id": "grow", "parent": "bar", "kind": "rect", "w": 10, "h": 10, "w_sizing": "fill"},
            {"id": "gone", "parent": "bar", "kind": "rect", "hidden": true},
            {"id": "note", "parent": "bar", "kind": "comment", "text": "fix", "author": "user"},
        ]), "bar");
        let bar = rule(&c.css, "tool-bar");
        for want in ["display: flex", "flex-direction: row", "gap: 8px", "padding: 8px 16px 8px 16px", "justify-content: space-between",
                     "align-items: center", "background-color: #222222", "border-radius: 8px", "position: relative", "width: 400px", "overflow: hidden"] {
            assert!(bar.contains(want), "{want} in {bar}");
        }
        assert!(!bar.contains("left:"), "the root ignores its place on the page: {bar}");
        let grow = rule(&c.css, "grow");
        assert!(grow.contains("flex: 1 1 0") && !grow.contains("width: 10px"), "{grow}");
        let title = rule(&c.css, "title");
        assert!(title.contains("white-space: pre") && title.contains("font-weight: 600"), "{title}");
        assert!(title.contains("color: #EDE9E2"), "light words on a dark bar: {title}");
        assert!(c.html.contains("Hello &lt;world&gt;</p>"), "{}", c.html);
        assert!(!c.html.contains("gone") && !c.html.contains("fix"), "hidden nodes and comments are left out: {}", c.html);
        assert!(c.css.starts_with(".tool-bar, .tool-bar * {"), "{}", c.css);
    }

    #[test]
    fn a_free_frame_places_its_children_where_they_are() {
        let c = code(json!([
            {"id": "p", "kind": "page"},
            {"id": "card", "parent": "p", "kind": "frame", "w": 300, "h": 200, "clip": false},
            {"id": "dot", "parent": "card", "kind": "ellipse", "x": 12, "y": 24.5, "w": 8, "h": 8, "rotation": 45, "flip_x": true,
             "opacity": 0.5, "blur": 2, "filters": {"brightness": -0.5, "hue": 90}},
            {"id": "box", "parent": "card", "kind": "rect", "name": "Card", "x": 0, "y": 0, "w": 10, "h": 10, "stroke": "#FF0000", "stroke_width": 2,
             "dash": [6, 4], "shadow": {"x": 0, "y": 2, "blur": 6, "color": "#00000040"}, "gradient": {"kind": "linear", "angle": 90,
             "stops": [{"at": 0, "color": "#000000"}, {"at": 1, "color": "#FFFFFF"}]}},
            {"id": "box2", "parent": "card", "kind": "rect", "name": "Card"},
        ]), "card");
        let card = rule(&c.css, "card");
        assert!(card.contains("position: relative") && !card.contains("overflow"), "{card}");
        let dot = rule(&c.css, "dot");
        for want in ["position: absolute", "left: 12px", "top: 24.5px", "border-radius: 50%", "transform: rotate(45deg) scale(-1, 1)",
                     "opacity: 0.5", "filter: blur(2px) brightness(0.5) hue-rotate(90deg)"] {
            assert!(dot.contains(want), "{want} in {dot}");
        }
        let card_box = rule(&c.css, "card-2");
        for want in ["border: 2px dashed #FF0000", "box-shadow: 0 2px 6px 0 #00000040", "linear-gradient(90deg, #000000 0%, #FFFFFF 100%)"] {
            assert!(card_box.contains(want), "{want} in {card_box}");
        }
        assert!(c.css.contains(".card-3 {"), "names are made unique: {}", c.css);
    }

    #[test]
    fn a_column_stretches_fills_and_hugs() {
        let c = code(json!([
            {"id": "p", "kind": "page"},
            {"id": "col", "parent": "p", "kind": "frame", "layout": "column", "items": "stretch", "w_sizing": "hug", "h_sizing": "hug", "gap": -4},
            {"id": "a", "parent": "col", "kind": "text", "text": "wraps"},
            {"id": "b", "parent": "col", "kind": "rect", "w": 50, "h": 20, "h_sizing": "fill"},
        ]), "col");
        let col = rule(&c.css, "col");
        assert!(col.contains("width: max-content") && !col.contains("height") && !col.contains("gap"), "{col}");
        let a = rule(&c.css, "a");
        assert!(a.contains("white-space: pre-wrap") && !a.contains("width"), "a stretched text wraps: {a}");
        let b = rule(&c.css, "b");
        assert!(b.contains("flex: 1 1 0") && b.contains("margin-top: -4px") && !b.contains("width"), "{b}");
    }

    #[test]
    fn spans_lists_and_links_are_markup_and_unsafe_links_are_dropped() {
        let c = code(json!([
            {"id": "p", "kind": "page"},
            {"id": "f", "parent": "p", "kind": "frame", "w": 100, "h": 100},
            {"id": "t", "parent": "f", "kind": "text", "text": "Go now\nx", "spans": [
                {"text": "Go", "link": "https://example.com/?a=1&b=\"2\""}, {"text": " "}, {"text": "now", "code": true}, {"text": "\nx", "weight": 700}]},
            {"id": "evil", "parent": "f", "kind": "text", "text": "x", "link": "javascript:alert(1)"},
            {"id": "todo", "parent": "f", "kind": "text", "text": "[x] done\n[ ] next", "list": "check"},
            {"id": "steps", "parent": "f", "kind": "text", "text": "one\ntwo", "list": "number"},
        ]), "f");
        assert!(c.html.contains("<a class=\"t-span\" href=\"https://example.com/?a=1&amp;b=&quot;2&quot;\">Go</a> <code class=\"t-span-2\">now</code>"), "{}", c.html);
        assert!(c.html.contains("<span class=\"t-span-3\">\n</span>") || c.html.contains("\n<span class=\"t-span-3\">x</span>"), "{}", c.html);
        assert!(!c.html.contains("javascript"), "{}", c.html);
        assert!(c.html.contains("<li><input type=\"checkbox\" disabled checked> done</li><li><input type=\"checkbox\" disabled> next</li></ul>"), "{}", c.html);
        assert!(c.html.contains("<ol class=\"steps\" data-node=\"steps\"><li>one</li><li>two</li></ol>"), "{}", c.html);
        assert!(rule(&c.css, "t-span-2").contains("Geist Mono"));
    }

    #[test]
    fn pictures_vectors_icons_and_lines_draw() {
        let src = "0123456789abcdef0123456789abcdef.png";
        let c = code(json!([
            {"id": "p", "kind": "page"},
            {"id": "f", "parent": "p", "kind": "frame", "w": 100, "h": 100},
            {"id": "photo", "parent": "f", "kind": "image", "src": src, "w": 40, "h": 30, "fit": "contain", "radius": 4},
            {"id": "cropped", "parent": "f", "kind": "image", "src": src, "w": 40, "h": 30, "crop": [0.25, 0, 0.5, 1]},
            {"id": "bg", "parent": "f", "kind": "rect", "src": src, "w": 40, "h": 30},
            {"id": "tri", "parent": "f", "kind": "polygon", "w": 20, "h": 20, "stroke": "#000000"},
            {"id": "blob", "parent": "f", "kind": "vector", "d": "M0 0 L10 0 L5 8 Z", "view": [10, 8], "w": 20, "h": 16, "gradient":
             {"kind": "radial", "stops": [{"at": 0, "color": "#FF0000"}, {"at": 1, "color": "#0000FF"}]}},
            {"id": "gear", "parent": "f", "kind": "icon", "icon": "settings", "w": 24, "h": 24, "stroke": "#123456"},
            {"id": "arrow", "parent": "f", "kind": "line", "points": [[0, 0], [50, 20]], "arrow_end": true, "route": "elbow"},
        ]), "f");
        assert!(c.html.contains(&format!("<img class=\"photo\" data-node=\"photo\" src=\"design-assets/{src}\" alt=\"\">")), "{}", c.html);
        assert!(rule(&c.css, "photo").contains("object-fit: contain"));
        let cropped = rule(&c.css, "cropped");
        assert!(cropped.contains("background-size: 200% 100%") && cropped.contains("background-position: 50% 0%"), "{cropped}");
        assert!(rule(&c.css, "bg").contains("background-size: cover"));
        assert_eq!(c.assets, vec![src.to_string()], "each picture once");
        assert!(c.html.contains("<polygon points=\"10,0 18.66,15 1.34,15\" fill=\"#D9D9D9\" stroke=\"#000000\""), "{}", c.html);
        assert!(c.html.contains("viewBox=\"0 0 10 8\"") && c.html.contains("<radialGradient id=\"blob-paint\""), "{}", c.html);
        assert!(c.html.contains(&format!("<path d=\"{}\"/>", icon_path("settings").unwrap())) && c.html.contains("stroke=\"#123456\""), "{}", c.html);
        assert!(c.html.contains("d=\"M0 0 L25 0 L25 20 L50 20\"") && c.html.contains("marker-end=\"url(#arrow-head)\""), "{}", c.html);

        let inlined = generate(&nodes(json!([{"id": "p", "kind": "page"}, {"id": "i", "parent": "p", "kind": "image", "src": src}])), "i",
            &mut |_| "data:image/png;base64,AAAA".into()).unwrap();
        assert!(inlined.assets.is_empty() && inlined.html.contains("src=\"data:image/png;base64,AAAA\""), "{inlined:?}");
    }

    #[test]
    fn a_page_or_a_missing_node_is_refused_and_names_escape() {
        let list = nodes(json!([{"id": "p", "kind": "page"}, {"id": "f", "parent": "p", "kind": "frame", "name": "\"><script>x</script>",
            "font": "x"}]));
        let mut asset = |s: &str| s.to_string();
        assert!(matches!(generate(&list, "p", &mut asset), Err(Refusal::Page(_))));
        assert!(matches!(generate(&list, "nope", &mut asset), Err(Refusal::NotFound(_))));
        let c = generate(&list, "f", &mut asset).unwrap();
        assert!(c.html.starts_with("<div class=\"script-x-script\" data-node=\"f\">"), "{}", c.html);
        assert_eq!(slug("Button / Primary"), "button-primary");
        assert_eq!(slug("2 Up"), "n-2-up");
        assert_eq!(slug("···"), "");
        assert_eq!(css_str("a\"</style>"), "\"a\\\"\\3c /style\\3e \"");
        assert_eq!(safe_href("mailto:a@b.c"), Some("mailto:a@b.c"));
        assert_eq!(safe_href("/docs#x"), Some("/docs#x"));
        assert_eq!(safe_href(" JavaScript:alert(1)"), None);
    }
}
