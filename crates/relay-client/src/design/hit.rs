//! What is under the pointer. The canvas picks from the chain this returns: the deepest node
//! is what a click selects, its ancestors are the breadcrumb.

use super::geom::{segment_distance, Rect};
use super::layout::Placed;
use super::scene::Scene;
use super::shapes;
use relay_bus::design::{Node, NodeKind};

/// Whether `(x, y)` is on `n`'s own shape (not its children), within `slop` page pixels for
/// thin strokes.
pub fn on_shape(scene: &Scene, placed: &Placed, n: &Node, rect: &Rect, x: f64, y: f64, slop: f64) -> bool {
    match n.kind {
        NodeKind::Polygon => shapes::inside(&shapes::node_polygon(n, rect), x, y) || {
            let pts = shapes::node_polygon(n, rect);
            let closed: Vec<_> = pts.iter().chain(pts.first()).copied().collect();
            closed.windows(2).any(|w| segment_distance(x, y, w[0], w[1]) <= slop)
        },
        NodeKind::Line if n.from.is_some() || n.to.is_some() || n.route.is_some() => {
            let (pts, curved) = shapes::line_points(scene, placed, n);
            let pts = if curved { shapes::flatten_curve(&pts) } else { pts };
            let reach = n.stroke_width.unwrap_or(2.0) / 2.0 + slop;
            pts.windows(2).any(|w| segment_distance(x, y, w[0], w[1]) <= reach)
        }
        NodeKind::Page | NodeKind::Group | NodeKind::Comment => false,
        NodeKind::Vector => {
            let reach = n.stroke_width.unwrap_or(1.0) / 2.0 + slop;
            shapes::vector_lines(n, rect).iter().any(|(pts, closed)| {
                ((*closed || n.open != Some(true)) && pts.len() > 2 && shapes::inside(pts, x, y))
                    || pts.windows(2).any(|w| segment_distance(x, y, w[0], w[1]) <= reach)
            })
        }
        NodeKind::Ellipse => {
            let (rx, ry) = (rect.w / 2.0, rect.h / 2.0);
            if rx <= 0.0 || ry <= 0.0 {
                return false;
            }
            let (dx, dy) = ((x - rect.cx()) / (rx + slop), (y - rect.cy()) / (ry + slop));
            dx * dx + dy * dy <= 1.0
        }
        NodeKind::Line | NodeKind::Path => {
            let Some(points) = n.points.as_deref() else {
                return false;
            };
            let reach = n.stroke_width.unwrap_or(2.0) / 2.0 + slop;
            let at = |p: &[f64; 2]| (rect.x + p[0], rect.y + p[1]);
            if points.len() == 1 {
                let (px, py) = at(&points[0]);
                return ((x - px).powi(2) + (y - py).powi(2)).sqrt() <= reach;
            }
            points.windows(2).any(|w| segment_distance(x, y, at(&w[0]), at(&w[1])) <= reach)
        }
        _ => rect.inflate(slop.min(rect.w.min(rect.h) / 4.0).max(0.0)).contains(x, y),
    }
}

/// The nodes under `(x, y)` on `page`, outermost first and deepest last: `[frame, card,
/// label]` for a click on a label in a card in a frame. Hidden and locked nodes are passed
/// over; a frame that clips hides what overflows it.
pub fn chain(scene: &Scene, placed: &Placed, page: &str, x: f64, y: f64, slop: f64) -> Vec<String> {
    let mut out = Vec::new();
    descend(scene, placed, page, x, y, slop, &mut out);
    out
}

fn descend(scene: &Scene, placed: &Placed, parent: &str, x: f64, y: f64, slop: f64, out: &mut Vec<String>) -> bool {
    for id in scene.children(parent).iter().rev() {
        let Some(n) = scene.node(id).filter(|n| !n.is_hidden()) else {
            continue;
        };
        let Some(rect) = placed.rect(id) else {
            continue;
        };
        // A turned node is tested, children and all, in its own unturned frame.
        let turn = shapes::rotation(n);
        let (x, y) = if turn != 0.0 { shapes::rotate(x, y, rect.cx(), rect.cy(), -turn) } else { (x, y) };
        // And a mirrored one in its unmirrored frame.
        let x = if n.flip_x == Some(true) { 2.0 * rect.cx() - x } else { x };
        let y = if n.flip_y == Some(true) { 2.0 * rect.cy() - y } else { y };
        let clips = n.kind == NodeKind::Frame && n.clip != Some(false);
        if n.kind.holds_children() && !(clips && !rect.contains(x, y)) {
            out.push(id.clone());
            if descend(scene, placed, id, x, y, slop, out) {
                if n.is_locked() {
                    // A locked container holds what is inside it.
                    out.truncate(out.iter().position(|o| o == id).unwrap_or(out.len()));
                    continue;
                }
                return true;
            }
            out.pop();
        }
        if n.is_locked() {
            continue;
        }
        if on_shape(scene, placed, n, &rect, x, y, slop) {
            out.push(id.clone());
            return true;
        }
    }
    false
}

/// The children of `parent` a marquee over `area` takes: those it touches.
pub fn marquee(scene: &Scene, placed: &Placed, parent: &str, area: &Rect) -> Vec<String> {
    scene
        .children(parent)
        .iter()
        .filter(|id| scene.node(id).is_some_and(|n| !n.is_hidden() && !n.is_locked() && n.kind != NodeKind::Comment))
        .filter(|id| {
            let turn = scene.node(id).map_or(0.0, shapes::rotation);
            placed.rect(id).is_some_and(|r| area.intersects(&shapes::rotated_bounds(&r, turn)))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::{node, sample};
    use relay_bus::design::NodeKind as K;

    #[test]
    fn the_chain_runs_from_frame_to_leaf() {
        let s = sample();
        let p = place(&s, "p1", &Mono);
        assert_eq!(chain(&s, &p, "p1", 20.0, 20.0, 0.0), ["a", "b"]);
        // Inside the frame but on no child: the frame alone.
        assert_eq!(chain(&s, &p, "p1", 300.0, 200.0, 0.0), ["a"]);
        // The ellipse's corner is outside its shape.
        assert!(chain(&s, &p, "p1", 501.0, 1.0, 0.0).is_empty());
        assert_eq!(chain(&s, &p, "p1", 540.0, 40.0, 0.0), ["d"]);
    }

    #[test]
    fn turned_nodes_and_polygons_are_hit_by_their_shape() {
        let mut bar = node("bar", Some("p1"), K::Rect, [0.0, 0.0, 200.0, 20.0]);
        bar.rotation = Some(90.0);
        let mut tri = node("tri", Some("p1"), K::Polygon, [300.0, 0.0, 100.0, 100.0]);
        tri.sides = Some(3);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), bar, tri]);
        let p = place(&s, "p1", &Mono);
        // Turned upright about its centre (100, 10): it now covers x 90..110, y -90..110.
        assert_eq!(chain(&s, &p, "p1", 100.0, -80.0, 0.0), ["bar"]);
        assert!(chain(&s, &p, "p1", 10.0, 10.0, 0.0).is_empty());
        assert_eq!(chain(&s, &p, "p1", 350.0, 70.0, 0.0), ["tri"]);
        assert!(chain(&s, &p, "p1", 305.0, 5.0, 0.0).is_empty(), "a triangle's top corners are empty");
    }

    #[test]
    fn clipping_locking_and_lines() {
        let mut locked = node("l", Some("f"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        locked.locked = Some(true);
        let mut line = node("ln", Some("p1"), K::Line, [200.0, 200.0, 100.0, 0.0]);
        line.points = Some(vec![[0.0, 0.0], [100.0, 0.0]]);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 50.0, 50.0]),
            node("over", Some("f"), K::Rect, [40.0, 40.0, 40.0, 40.0]),
            locked,
            line,
        ]);
        let p = place(&s, "p1", &Mono);
        // The part of `over` outside the frame is clipped away.
        assert!(chain(&s, &p, "p1", 70.0, 70.0, 0.0).is_empty());
        assert_eq!(chain(&s, &p, "p1", 45.0, 45.0, 0.0), ["f", "over"]);
        assert_eq!(chain(&s, &p, "p1", 5.0, 5.0, 0.0), ["f"]);
        assert_eq!(chain(&s, &p, "p1", 250.0, 203.0, 2.0), ["ln"]);
        assert!(chain(&s, &p, "p1", 250.0, 210.0, 2.0).is_empty());
        let area = Rect::new(-10.0, -10.0, 30.0, 30.0);
        assert_eq!(marquee(&s, &p, "p1", &area), ["f"]);
        assert_eq!(marquee(&s, &p, "f", &area), Vec::<String>::new());
    }
}
