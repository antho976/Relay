//! Flip and copy style: the edits behind Shift H / Shift V and Ctrl Alt C / Ctrl Alt V.
//!
//! A flip mirrors each selected layer about the selection's centre and turns its own drawing
//! over: a line's or path's points, a box's corner radii, a gradient's angle, an arc, a turn,
//! and the `flip_x`/`flip_y` of a polygon, picture or icon. A frame or group mirrors what is
//! inside it the same way, unless it lays its children out itself.

use super::edit::{outermost, px};
use super::layout::Placed;
use super::scene::Scene;
use super::geom::Rect;
use relay_bus::design::{takes, GradientKind, Node, NodeKind, NodeOp};
use serde_json::{json, Map, Value};

/// The edits that flip `ids` left to right (`horizontal`) or top to bottom.
pub fn flip(scene: &Scene, placed: &Placed, ids: &[String], horizontal: bool) -> Vec<NodeOp> {
    let ids = outermost(scene, ids);
    let Some(bounds) = Rect::bounds(ids.iter().filter_map(|id| placed.rect(id)).collect::<Vec<_>>().iter()) else {
        return Vec::new();
    };
    let mut ops = Vec::new();
    for id in &ids {
        let movable = scene.parent(id).is_some_and(|p| !super::edit::lays_out(scene, p));
        mirror(scene, placed, id, &bounds, horizontal, movable, &mut ops);
    }
    ops
}

/// Mirror `id` about the centre of `about` (moving it when `movable`) and flip its drawing.
fn mirror(scene: &Scene, placed: &Placed, id: &str, about: &Rect, horizontal: bool, movable: bool, ops: &mut Vec<NodeOp>) {
    let (Some(n), Some(r)) = (scene.node(id), placed.rect(id)) else {
        return;
    };
    let mut set = own(n, horizontal);
    if movable {
        if horizontal {
            let dx = 2.0 * about.cx() - r.x - r.w - r.x;
            if dx.abs() >= 0.5 {
                set.insert("x".into(), px(n.x + dx));
            }
        } else {
            let dy = 2.0 * about.cy() - r.y - r.h - r.y;
            if dy.abs() >= 0.5 {
                set.insert("y".into(), px(n.y + dy));
            }
        }
    }
    if !set.is_empty() {
        ops.push(NodeOp::Update { id: id.to_string(), set });
    }
    if n.kind.holds_children() && n.kind != NodeKind::Page {
        let inside = !super::edit::lays_out(scene, id);
        for child in scene.children(id) {
            if inside {
                mirror(scene, placed, child, &r, horizontal, true, ops);
            } else if let Some(c) = scene.node(child) {
                // A laid-out frame keeps its order; each child still turns over.
                let set = own(c, horizontal);
                if !set.is_empty() {
                    ops.push(NodeOp::Update { id: child.clone(), set });
                }
                if c.kind.holds_children() {
                    if let Some(cr) = placed.rect(child) {
                        for grandchild in scene.children(child) {
                            mirror(scene, placed, grandchild, &cr, horizontal, !super::edit::lays_out(scene, child), ops);
                        }
                    }
                }
            }
        }
    }
}

/// The fields that turn one node's own drawing over, in place.
fn own(n: &Node, horizontal: bool) -> Map<String, Value> {
    let mut set = Map::new();
    if let Some(turn) = n.rotation.filter(|t| *t != 0.0) {
        set.insert("rotation".into(), json!(-turn));
    }
    match n.kind {
        NodeKind::Line | NodeKind::Path if n.from.is_none() && n.to.is_none() => {
            if let Some(points) = n.points.as_deref().filter(|p| !p.is_empty()) {
                let axis = if horizontal { 0 } else { 1 };
                let lo = points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
                let hi = points.iter().map(|p| p[axis]).fold(f64::NEG_INFINITY, f64::max);
                let flipped: Vec<[f64; 2]> = points
                    .iter()
                    .map(|p| {
                        let mut q = *p;
                        q[axis] = lo + hi - p[axis];
                        q
                    })
                    .collect();
                set.insert("points".into(), json!(flipped));
            }
        }
        NodeKind::Polygon | NodeKind::Image | NodeKind::Icon => {
            let key = if horizontal { "flip_x" } else { "flip_y" };
            let on = if horizontal { n.flip_x } else { n.flip_y } == Some(true);
            set.insert(key.into(), if on { Value::Null } else { json!(true) });
        }
        _ => {}
    }
    if let Some([tl, tr, br, bl]) = n.radii {
        let radii = if horizontal { [tr, tl, bl, br] } else { [bl, br, tr, tl] };
        if radii != [tl, tr, br, bl] {
            set.insert("radii".into(), json!(radii));
        }
    }
    if let Some(g) = n.gradient.as_ref().filter(|g| g.kind == GradientKind::Linear) {
        let a = g.angle.unwrap_or(180.0);
        let turned = if horizontal { 360.0 - a } else { 180.0 - a }.rem_euclid(360.0);
        if (turned - a.rem_euclid(360.0)).abs() > 1e-9 {
            let mut g = g.clone();
            g.angle = Some(turned);
            set.insert("gradient".into(), json!(g));
        }
    }
    if let Some(a) = n.arc.as_ref().filter(|_| n.kind == NodeKind::Ellipse) {
        let end = a.start + a.sweep;
        let start = if horizontal { -end } else { 180.0 - end }.rem_euclid(360.0);
        let mut a = a.clone();
        a.start = start;
        set.insert("arc".into(), json!(a));
    }
    set
}

/// The style fields a text's letters take.
const TEXT_STYLE: &[&str] = &[
    "color", "font", "font_size", "font_weight", "italic", "underline", "strike", "align", "line_height",
    "letter_spacing", "text_case",
];
/// The style fields a box, shape or stroke paints with.
const PAINT_STYLE: &[&str] = &["fill", "gradient", "stroke", "stroke_width", "dash", "radius", "radii", "shadow"];
/// The style fields every layer has.
const LAYER_STYLE: &[&str] = &["opacity", "blur", "blend"];

/// A node's style, as `paste_style` lays it on others: every style field, `null` where it has
/// the default. Its kind travels with it, under `kind`.
pub fn style_of(n: &Node) -> Map<String, Value> {
    let all = serde_json::to_value(n).unwrap_or(Value::Null);
    TEXT_STYLE
        .iter()
        .chain(PAINT_STYLE)
        .chain(LAYER_STYLE)
        .chain(&["kind"])
        .map(|k| (k.to_string(), all.get(*k).cloned().unwrap_or(Value::Null)))
        .collect()
}

fn holds_text(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon)
}

/// The edits that give `ids` a copied `style`: its letters to whatever holds text, its paint
/// to shapes from a shape and to texts from a text, its opacity, blur and blend to all; each
/// only where the node's kind uses the field (`takes`), so a mixed selection takes the paste.
pub fn paste_style(scene: &Scene, ids: &[String], style: &Map<String, Value>) -> Vec<NodeOp> {
    let from: Option<NodeKind> = style.get("kind").cloned().and_then(|k| serde_json::from_value(k).ok());
    let from_text = from == Some(NodeKind::Text);
    ids.iter()
        .filter_map(|id| {
            let n = scene.node(id)?;
            let current = serde_json::to_value(n).ok()?;
            let mut keys: Vec<&str> = LAYER_STYLE.to_vec();
            if holds_text(n.kind) && from.is_some_and(holds_text) {
                keys.extend(TEXT_STYLE);
            }
            let paints = !matches!(n.kind, NodeKind::Page | NodeKind::Group);
            if paints && (n.kind == NodeKind::Text) == from_text {
                keys.extend(PAINT_STYLE);
            }
            let set: Map<String, Value> = keys
                .into_iter()
                .filter(|k| takes(n.kind, k))
                .filter_map(|k| {
                    let want = style.get(k).cloned().unwrap_or(Value::Null);
                    let have = current.get(k).cloned().unwrap_or(Value::Null);
                    (want != have).then(|| (k.to_string(), want))
                })
                .collect();
            (!set.is_empty()).then(|| NodeOp::Update { id: id.clone(), set })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::node;
    use relay_bus::design::NodeKind as K;

    fn applied(scene: &mut Scene, ops: &[NodeOp]) {
        scene.apply_all(ops).unwrap();
    }

    #[test]
    fn a_flip_mirrors_the_selection_about_its_centre() {
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 50.0]),
            node("b", Some("p1"), K::Rect, [200.0, 0.0, 50.0, 50.0]),
        ]);
        s.node_mut("a").unwrap().radii = Some([10.0, 0.0, 0.0, 0.0]);
        let p = place(&s, "p1", &Mono);
        let ops = flip(&s, &p, &["a".into(), "b".into()], true);
        applied(&mut s, &ops);
        assert_eq!((s.node("a").unwrap().x, s.node("b").unwrap().x), (150.0, 0.0));
        assert_eq!(s.node("a").unwrap().radii, Some([0.0, 10.0, 0.0, 0.0]));
        // Flipping twice is where it started.
        let p = place(&s, "p1", &Mono);
        let ops = flip(&s, &p, &["a".into(), "b".into()], true);
        applied(&mut s, &ops);
        assert_eq!((s.node("a").unwrap().x, s.node("b").unwrap().x), (0.0, 200.0));
    }

    #[test]
    fn a_flip_turns_points_polygons_and_frames_over() {
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 100.0]),
            node("in", Some("f"), K::Rect, [10.0, 10.0, 40.0, 40.0]),
            node("tri", Some("p1"), K::Polygon, [300.0, 0.0, 100.0, 100.0]),
            node("ln", Some("p1"), K::Line, [0.0, 200.0, 100.0, 50.0]),
        ]);
        s.node_mut("ln").unwrap().points = Some(vec![[0.0, 0.0], [100.0, 50.0]]);
        let p = place(&s, "p1", &Mono);
        let ops = flip(&s, &p, &["f".into(), "tri".into(), "ln".into()], false);
        applied(&mut s, &ops);
        assert_eq!(s.node("in").unwrap().y, 50.0, "the child mirrors inside its frame");
        assert_eq!(s.node("tri").unwrap().flip_y, Some(true));
        assert_eq!(s.node("ln").unwrap().points, Some(vec![[0.0, 50.0], [100.0, 0.0]]));
    }

    #[test]
    fn style_goes_where_it_fits() {
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("r", Some("p1"), K::Rect, [0.0; 4]),
            node("t", Some("p1"), K::Text, [0.0; 4]),
        ]);
        let mut src = node("src", Some("p1"), K::Rect, [0.0; 4]);
        src.fill = Some("#FF0000".into());
        src.font_size = Some(30.0);
        src.opacity = Some(0.5);
        let style = style_of(&src);
        let ops = paste_style(&s, &["r".into(), "t".into()], &style);
        applied(&mut s, &ops);
        let (r, t) = (s.node("r").unwrap(), s.node("t").unwrap());
        assert_eq!((r.fill.as_deref(), r.font_size, r.opacity), (Some("#FF0000"), Some(30.0), Some(0.5)));
        assert_eq!((t.fill.as_deref(), t.font_size, t.opacity), (None, Some(30.0), Some(0.5)), "a text takes no box paint from a shape");
    }

    /// Each node takes only the fields its kind uses, so a mixed selection takes the paste.
    #[test]
    fn style_skips_what_a_kind_does_not_use() {
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("e", Some("p1"), K::Ellipse, [0.0; 4]),
            node("i", Some("p1"), K::Image, [0.0; 4]),
            node("g", Some("p1"), K::Group, [0.0; 4]),
        ]);
        s.node_mut("i").unwrap().src = Some("0123456789abcdef0123.png".into());
        let mut src = node("src", Some("p1"), K::Rect, [0.0; 4]);
        (src.radius, src.fill, src.opacity) = (Some(12.0), Some("#FF0000".into()), Some(0.5));
        let ops = paste_style(&s, &["e".into(), "i".into(), "g".into()], &style_of(&src));
        applied(&mut s, &ops);
        let (e, i, g) = (s.node("e").unwrap(), s.node("i").unwrap(), s.node("g").unwrap());
        assert_eq!((e.radius, e.fill.as_deref(), e.opacity), (None, Some("#FF0000"), Some(0.5)));
        assert_eq!((i.radius, i.fill.as_deref(), i.opacity), (Some(12.0), None, Some(0.5)));
        assert_eq!((g.fill.as_deref(), g.opacity), (None, Some(0.5)));
    }
}
