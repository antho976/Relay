//! Colours as the canvas paints them (client-geometry): the fill each kind takes when it names
//! none, the ground a text sits on, the colour it is given there, and how far two colours stand
//! apart. The painter, export, thumbnails and the render job's contrast check share these, so
//! what an agent is told about a colour is what the canvas shows. They read a node as drawn
//! (`Scene::node`), where a `$name` colour is already resolved.

use super::scene::Scene;
use relay_bus::design::{parse_color, Node, NodeKind};

/// Defaults the schema documents: a white frame, a grey shape, a yellow sticky.
pub const FRAME_FILL: &str = "#FFFFFF";
pub const SHAPE_FILL: &str = "#D9D9D9";
pub const STICKY_FILL: &str = "#FFE58A";
pub const STROKE: &str = "#8C8C8C";
/// A page's canvas when it names no fill: a step off the app's ground.
pub const CANVAS: &str = "#1E1D1B";
/// A text's own colour when it names none: dark on a light ground, light on a dark one.
pub const DARK_TEXT: &str = "#1A1A1A";
pub const LIGHT_TEXT: &str = "#EDE9E2";

/// Relative luminance (WCAG) of channels 0 to 1.
fn relative([r, g, b]: [f64; 3]) -> f64 {
    let lin = |c: f64| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// Relative luminance of a colour, 0 (black) to 1 (white), ignoring alpha; what does not parse
/// counts as black.
pub fn luminance(hex: &str) -> f64 {
    let [r, g, b, _] = parse_color(hex).unwrap_or([0, 0, 0, 255]);
    relative([r, g, b].map(|c| c as f64 / 255.0))
}

/// The fill a node paints, its kind's default when it names none.
pub fn fill_of(n: &Node) -> Option<String> {
    n.fill.clone().or_else(|| {
        match n.kind {
            NodeKind::Frame => Some(FRAME_FILL),
            NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon => Some(SHAPE_FILL),
            NodeKind::Sticky => Some(STICKY_FILL),
            NodeKind::Page => Some(CANVAS),
            _ => None,
        }
        .map(str::to_string)
    })
}

/// A text's colour: its own, or dark on a light ground and light on a dark one, the ground
/// being the shape a label sits on, else the nearest container that paints one at least half
/// opaque, else the canvas.
pub fn text_color(scene: &Scene, n: &Node) -> String {
    if let Some(c) = &n.color {
        return c.clone();
    }
    if matches!(n.kind, NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) {
        // A label sits on the shape itself.
        let ground = n.gradient.as_ref().and_then(|g| g.stops.first()).map(|s| s.color.clone()).or_else(|| fill_of(n)).unwrap_or_default();
        return if luminance(&ground) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }.into();
    }
    let ground = std::iter::once(n.id.as_str())
        .chain(scene.ancestors(&n.id))
        .filter_map(|id| scene.node(id))
        .filter(|a| a.id != n.id)
        .find_map(|a| ground_of(a).filter(|f| parse_color(f).is_some_and(|c| c[3] >= 128)))
        .unwrap_or_else(|| CANVAS.into());
    if luminance(&ground) > 0.35 { DARK_TEXT } else { LIGHT_TEXT }.into()
}

/// The colour a node shows behind what is on it: its fill, or the middle of its gradient.
pub fn ground_of(n: &Node) -> Option<String> {
    match &n.gradient {
        Some(g) if !g.stops.is_empty() => {
            let mut stops = g.stops.clone();
            stops.sort_by(|a, b| a.at.total_cmp(&b.at));
            Some(stops[stops.len() / 2].color.clone())
        }
        _ => fill_of(n),
    }
}

/// WCAG contrast of two colours, 1 to 21; `fg`'s alpha is laid over `bg` first (`bg` counts as
/// opaque). What does not parse counts as black.
pub fn contrast(fg: &str, bg: &str) -> f64 {
    let [fr, fg_, fb, fa] = parse_color(fg).unwrap_or([0, 0, 0, 255]);
    let [br, bg_, bb, _] = parse_color(bg).unwrap_or([0, 0, 0, 255]);
    let a = fa as f64 / 255.0;
    let over = |f: u8, b: u8| (f as f64 * a + b as f64 * (1.0 - a)) / 255.0;
    let top = relative([over(fr, br), over(fg_, bg_), over(fb, bb)]);
    let under = relative([br, bg_, bb].map(|c| c as f64 / 255.0));
    (top.max(under) + 0.05) / (top.min(under) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::scene::tests::node;
    use relay_bus::design::{Gradient, GradientKind, NodeKind as K, Stop};

    #[test]
    fn contrast_matches_wcag() {
        assert!((contrast("#000000", "#FFFFFF") - 21.0).abs() < 1e-9);
        assert!((contrast("#FFFFFF", "#000000") - 21.0).abs() < 1e-9, "either way round");
        assert!((contrast("#777777", "#FFFFFF") - 4.48).abs() < 0.01);
        assert!((contrast("#ABC", "#ABC") - 1.0).abs() < 1e-9);
        // Half-clear black over white is a mid grey, not black.
        let half = contrast("#00000080", "#FFFFFF");
        assert!((half - contrast("#7F7F7F", "#FFFFFF")).abs() < 0.05, "{half}");
        assert!(contrast("#00000000", "#FFFFFF") < 1.0 + 1e-9, "a clear colour shows the ground");
    }

    #[test]
    fn moved_rules_match_the_painter() {
        let mut dark_rect = node("r", Some("p1"), K::Rect, [0.0; 4]);
        dark_rect.fill = Some("#202020".into());
        let mut tinted = node("t", Some("f"), K::Text, [0.0; 4]);
        tinted.color = Some("#FF0000".into());
        let mut glass = node("g", Some("p1"), K::Frame, [0.0; 4]);
        glass.fill = Some("#FFFFFF20".into());
        let mut lit = node("lit", Some("p1"), K::Frame, [0.0; 4]);
        lit.gradient = Some(Gradient {
            kind: GradientKind::Linear,
            angle: None,
            stops: ["#000000", "#FFFFFF", "#000000"].iter().enumerate().map(|(i, c)| Stop { at: i as f64 / 2.0, color: (*c).into() }).collect(),
            center: None,
            radius: None,
        });
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("f", Some("p1"), K::Frame, [0.0; 4]),
            node("on_frame", Some("f"), K::Text, [0.0; 4]),
            node("on_page", Some("p1"), K::Text, [0.0; 4]),
            node("sticky", Some("p1"), K::Sticky, [0.0; 4]),
            dark_rect,
            tinted,
            glass,
            node("in_glass", Some("g"), K::Text, [0.0; 4]),
            lit,
            node("in_lit", Some("lit"), K::Text, [0.0; 4]),
        ]);
        let color = |id: &str| text_color(&s, s.node(id).unwrap());
        assert_eq!(color("on_frame"), DARK_TEXT, "a white frame is light");
        assert_eq!(color("on_page"), LIGHT_TEXT, "the canvas is dark");
        assert_eq!(color("sticky"), DARK_TEXT, "a sticky's label sits on its yellow");
        assert_eq!(color("r"), LIGHT_TEXT, "a label on a dark shape");
        assert_eq!(color("t"), "#FF0000", "its own colour wins");
        assert_eq!(color("in_glass"), LIGHT_TEXT, "a mostly clear fill is passed over to the canvas");
        assert_eq!(color("in_lit"), DARK_TEXT, "a gradient shows its middle stop");
        assert_eq!(fill_of(s.node("p1").unwrap()).as_deref(), Some(CANVAS));
        assert_eq!(fill_of(s.node("on_page").unwrap()), None, "a text paints no box of its own");
        assert!(luminance("#FFFFFF") > 0.99 && luminance("not a colour") == 0.0);
    }
}
