//! What is under the pointer. The canvas picks from the chain this returns: the deepest node
//! is what a click selects, its ancestors are the breadcrumb.

use super::geom::{quads_overlap, segment_distance, Rect};
use super::layout::Placed;
use super::scene::Scene;
use super::shapes;
use relay_bus::design::{Node, NodeKind};

/// Whether `(x, y)` is on `n`'s own shape (not its children), within `slop` page pixels for
/// thin strokes: a box with its rounded corners, an ellipse's drawn slice, a polygon, a line's
/// or path's stroke as it is drawn with its arrowheads.
pub fn on_shape(scene: &Scene, placed: &Placed, n: &Node, rect: &Rect, x: f64, y: f64, slop: f64) -> bool {
    match n.kind {
        NodeKind::Polygon => shapes::inside(&shapes::node_polygon(n, rect), x, y) || {
            let pts = shapes::node_polygon(n, rect);
            let closed: Vec<_> = pts.iter().chain(pts.first()).copied().collect();
            closed.windows(2).any(|w| segment_distance(x, y, w[0], w[1]) <= slop)
        },
        NodeKind::Line if shapes::routed(n) => {
            let (pts, curved) = shapes::line_points(scene, placed, n);
            let pts = if curved { shapes::flatten_curve(&pts) } else { pts };
            let reach = n.stroke_width.unwrap_or(2.0) / 2.0 + slop;
            pts.windows(2).any(|w| segment_distance(x, y, w[0], w[1]) <= reach) || on_heads(scene, placed, n, rect, x, y, slop)
        }
        NodeKind::Page | NodeKind::Group => false,
        // Only what is drawn: an arc's slice, not its missing part or its hole.
        NodeKind::Ellipse => shapes::on_ellipse(n, rect, x, y, slop),
        NodeKind::Line | NodeKind::Path => {
            let Some(points) = n.points.as_deref() else {
                return false;
            };
            let reach = n.stroke_width.unwrap_or(2.0) / 2.0 + slop;
            if points.len() == 1 {
                let (px, py) = (rect.x + points[0][0], rect.y + points[0][1]);
                return ((x - px).powi(2) + (y - py).powi(2)).sqrt() <= reach;
            }
            let outline = shapes::stroke_outline(n, rect);
            // A filled or closed one is a shape: its inside counts.
            let solid = shapes::closed_shape(n) || (shapes::fillable_line(n) && (n.fill.is_some() || n.gradient.is_some()));
            (solid && shapes::inside(&outline, x, y))
                || outline.windows(2).any(|w| segment_distance(x, y, w[0], w[1]) <= reach)
                || on_heads(scene, placed, n, rect, x, y, slop)
        }
        _ => {
            // Thin boxes keep a target of about twice the slop across; small ones a quarter of
            // their side, so neighbours stay apart.
            let pad = |side: f64| slop.min(side / 4.0).max(slop - side / 2.0).max(0.0);
            shapes::in_rounded_box(rect, shapes::corner_radii(n, rect), x, y, pad(rect.w), pad(rect.h))
        }
    }
}

/// Whether `(x, y)` is on one of `n`'s arrowheads, or within `slop` of one.
fn on_heads(scene: &Scene, placed: &Placed, n: &Node, rect: &Rect, x: f64, y: f64, slop: f64) -> bool {
    shapes::arrowheads(scene, placed, n, rect)
        .iter()
        .any(|t| shapes::inside(t, x, y) || (0..3).any(|i| segment_distance(x, y, t[i], t[(i + 1) % 3]) <= slop))
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
        let clips = n.kind == NodeKind::Frame && n.clip != Some(false);
        // A clipping frame hides what passes its rounded corners too.
        if n.kind.holds_children() && (!clips || shapes::in_rounded_box(&rect, shapes::corner_radii(n, &rect), x, y, 0.0, 0.0)) {
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

/// The children of `parent` a marquee over `area` (a page rectangle) takes: those it touches as
/// they are drawn. The area is carried into `parent`'s coordinates through every turn above it
/// (a turned artboard's children are picked where they show), and each child is its drawn box (a
/// line's or path's points) turned by its own turn.
pub fn marquee(scene: &Scene, placed: &Placed, parent: &str, area: &Rect) -> Vec<String> {
    let area = area.corners().map(|(x, y)| shapes::to_local(scene, placed, parent, x, y));
    scene
        .children(parent)
        .iter()
        .filter(|id| scene.node(id).is_some_and(|n| !n.is_hidden() && !n.is_locked()))
        .filter(|id| {
            let (Some(n), Some(r)) = (scene.node(id), placed.rect(id)) else {
                return false;
            };
            let Some(drawn) = shapes::drawn_box(scene, placed, n) else {
                return false;
            };
            let turn = shapes::rotation(n);
            quads_overlap(&area, &drawn.corners().map(|(x, y)| if turn == 0.0 { (x, y) } else { shapes::rotate(x, y, r.cx(), r.cy(), turn) }))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::{node, sample};
    use relay_bus::design::{Node, NodeKind as K};

    #[test]
    fn closed_paths_hit_inside_and_arcs_only_where_drawn() {
        let mut shape = node("v", Some("p1"), K::Line, [0.0, 0.0, 100.0, 100.0]);
        shape.points = Some(vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]]);
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), shape.clone()]);
        let p = crate::design::layout::place(&s, "p1", &crate::design::layout::tests::Mono);
        assert!(!on_shape(&s, &p, &shape, &r, 50.0, 30.0, 1.0), "an open outline is hit on its stroke");
        shape.closed = Some(true);
        assert!(on_shape(&s, &p, &shape, &r, 50.0, 30.0, 1.0));
        let mut ring = node("e", Some("p1"), K::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        ring.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: 360.0, inner: 0.5, cap: None });
        assert!(!on_shape(&s, &p, &ring, &r, 50.0, 50.0, 0.0), "the hole is not the ring");
        assert!(on_shape(&s, &p, &ring, &r, 50.0, 10.0, 0.0));
    }

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

    /// One page holding `nodes`, placed.
    fn page(nodes: Vec<Node>) -> (Scene, Placed) {
        let s = Scene::new(std::iter::once(node("p1", None, K::Page, [0.0; 4])).chain(nodes).collect());
        let p = place(&s, "p1", &Mono);
        (s, p)
    }

    #[test]
    fn thin_boxes_keep_a_reachable_edge() {
        // B115: a zero-width box was reachable only on its exact edge, a 1 px divider by a
        // quarter of a pixel.
        let hit = |n: &Node, x: f64, y: f64, slop: f64| {
            let (s, p) = page(vec![n.clone()]);
            on_shape(&s, &p, n, &p.rect(&n.id).unwrap(), x, y, slop)
        };
        let bare = node("z", Some("p1"), K::Rect, [100.0, 0.0, 0.0, 50.0]);
        assert!(hit(&bare, 101.5, 25.0, 2.0));
        assert!(!hit(&bare, 103.0, 25.0, 2.0));
        let divider = node("d", Some("p1"), K::Rect, [0.0, 200.0, 200.0, 1.0]);
        assert!(hit(&divider, 100.0, 204.0, 4.0), "3.5 px of slop below a 1 px divider");
        assert!(!hit(&divider, 100.0, 206.0, 4.0));
        let small = node("s", Some("p1"), K::Rect, [0.0, 0.0, 8.0, 8.0]);
        assert!(!hit(&small, 11.0, 4.0, 4.0), "a small box keeps its quarter-side pad");
    }

    #[test]
    fn rounded_corners_are_hit_only_where_drawn() {
        // B116: the empty part of a rounded corner selected the box, and a rounded frame's
        // clip let a child be clicked where it is clipped away.
        let mut pill = node("r", Some("p1"), K::Rect, [0.0, 0.0, 160.0, 40.0]);
        pill.radius = Some(20.0);
        let (s, p) = page(vec![pill.clone()]);
        let r = p.rect("r").unwrap();
        assert!(!on_shape(&s, &p, &pill, &r, 3.0, 3.0, 0.0));
        assert!(on_shape(&s, &p, &pill, &r, 20.0, 1.0, 0.0));
        assert!(on_shape(&s, &p, &pill, &r, 80.0, 20.0, 0.0));
        pill.radius = None;
        pill.radii = Some([0.0, 20.0, 0.0, 0.0]);
        assert!(on_shape(&s, &p, &pill, &r, 3.0, 3.0, 0.0));
        assert!(!on_shape(&s, &p, &pill, &r, 157.0, 3.0, 0.0));
        let mut f = node("f", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 100.0]);
        f.radius = Some(30.0);
        let (s, p) = page(vec![f, node("c", Some("f"), K::Rect, [0.0, 0.0, 20.0, 20.0])]);
        assert!(chain(&s, &p, "p1", 2.0, 2.0, 0.0).is_empty());
        assert_eq!(chain(&s, &p, "p1", 15.0, 15.0, 0.0), ["f", "c"]);
    }

    #[test]
    fn a_marquee_in_a_turned_artboard_takes_what_it_shows() {
        // B117: the marquee compared page space with the children's unturned boxes.
        let mut f = node("f", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        f.rotation = Some(90.0);
        let (s, p) = page(vec![f, node("c", Some("f"), K::Rect, [150.0, 40.0, 20.0, 20.0])]);
        // c's centre (160, 50) is drawn at (100, 110).
        assert_eq!(marquee(&s, &p, "f", &Rect::new(85.0, 95.0, 30.0, 30.0)), ["c"]);
        assert!(marquee(&s, &p, "f", &Rect::new(150.0, 40.0, 20.0, 20.0)).is_empty(), "where c would be unturned");
    }

    #[test]
    fn a_marquee_takes_a_line_by_its_points() {
        // B3: a line written without w/h has a 0x0 box at its origin; a marquee across its
        // middle missed it.
        let mut line = node("ln", Some("p1"), K::Line, [100.0, 100.0, 0.0, 0.0]);
        line.points = Some(vec![[0.0, 0.0], [300.0, -50.0]]);
        let (s, p) = page(vec![line]);
        assert_eq!(marquee(&s, &p, "p1", &Rect::new(240.0, 70.0, 20.0, 10.0)), ["ln"]);
        assert!(marquee(&s, &p, "p1", &Rect::new(90.0, 110.0, 20.0, 20.0)).is_empty());
    }

    #[test]
    fn a_sparse_path_is_hit_on_its_drawn_curve() {
        // B118: the path was hit on its raw polyline while it is painted as quadratics through
        // the midpoints of its points.
        let mut v = node("v", Some("p1"), K::Path, [0.0, 0.0, 100.0, 100.0]);
        v.points = Some(vec![[0.0, 0.0], [50.0, 100.0], [100.0, 0.0]]);
        v.stroke_width = Some(2.0);
        let (s, p) = page(vec![v.clone()]);
        let r = p.rect("v").unwrap();
        // The drawn curve passes (43.75, 62.5) and peaks near y 66.7; the corner at (50, 100)
        // is 33 px from any ink.
        assert!(on_shape(&s, &p, &v, &r, 43.75, 62.5, 1.0));
        assert!(!on_shape(&s, &p, &v, &r, 50.0, 99.0, 1.0));
        let outline = shapes::stroke_outline(&v, &r);
        assert_eq!((outline.first(), outline.last()), (Some(&(0.0, 0.0)), Some(&(100.0, 0.0))));
    }

    #[test]
    fn arrowheads_are_hit_where_drawn() {
        // B121: an arrowhead's wings reach past the stroke's hit reach.
        let mut line = node("ln", Some("p1"), K::Line, [200.0, 200.0, 100.0, 0.0]);
        line.points = Some(vec![[0.0, 0.0], [100.0, 0.0]]);
        line.stroke_width = Some(10.0);
        line.arrow_end = Some(true);
        let (s, p) = page(vec![line.clone()]);
        assert_eq!(chain(&s, &p, "p1", 275.0, 210.0, 0.0), ["ln"], "inside the head, 10 px off the stroke");
        assert!(chain(&s, &p, "p1", 210.0, 210.0, 0.0).is_empty());
        line.arrow_end = None;
        let (s, p) = page(vec![line]);
        assert!(chain(&s, &p, "p1", 275.0, 210.0, 0.0).is_empty());
        // A connector's head too.
        let mut c = node("c", Some("p1"), K::Line, [0.0; 4]);
        (c.from, c.to, c.stroke_width, c.arrow_end) = (Some("a".into()), Some("b".into()), Some(10.0), Some(true));
        let (s, p) = page(vec![node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]), node("b", Some("p1"), K::Rect, [300.0, 0.0, 100.0, 100.0]), c]);
        assert_eq!(chain(&s, &p, "p1", 280.0, 60.0, 0.0), ["c"]);
    }
}
