//! Geometry of the richer nodes: a polygon's or star's corners, rotation about a node's centre,
//! and where a connector runs once its ends are attached to other nodes. The canvas draws with
//! these and hit testing reads them, so what you click is what you see.

use super::geom::Rect;
use super::layout::Placed;
use super::scene::Scene;
use relay_bus::design::{Node, NodeKind, Route};

/// A polygon's (or star's) corners inside `r`, first at the top centre, clockwise.
pub fn polygon(r: &Rect, sides: u8, star: Option<f64>) -> Vec<(f64, f64)> {
    let n = sides.clamp(3, 24) as usize;
    let (cx, cy) = (r.cx(), r.cy());
    let (rx, ry) = (r.w / 2.0, r.h / 2.0);
    let count = if star.is_some() { n * 2 } else { n };
    (0..count)
        .map(|i| {
            let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / count as f64;
            let k = match star {
                Some(inner) if i % 2 == 1 => inner,
                _ => 1.0,
            };
            (cx + rx * k * angle.cos(), cy + ry * k * angle.sin())
        })
        .collect()
}

/// The corners of a polygon node in its placed box (a star has 5 points unless it says).
pub fn node_polygon(n: &Node, r: &Rect) -> Vec<(f64, f64)> {
    let sides = n.sides.unwrap_or(if n.star.is_some() { 5 } else { 3 });
    polygon(r, sides, n.star)
}

/// Whether `(x, y)` is inside the closed polygon `points` (even-odd).
pub fn inside(points: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut odd = false;
    let mut j = points.len().wrapping_sub(1);
    for i in 0..points.len() {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// `(x, y)` turned by `degrees` clockwise about `(cx, cy)`.
pub fn rotate(x: f64, y: f64, cx: f64, cy: f64, degrees: f64) -> (f64, f64) {
    let (s, c) = degrees.to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + dx * c - dy * s, cy + dx * s + dy * c)
}

/// A node's turn, 0 for none.
pub fn rotation(n: &Node) -> f64 {
    n.rotation.unwrap_or(0.0)
}

/// The box a turned rectangle covers.
pub fn rotated_bounds(r: &Rect, degrees: f64) -> Rect {
    if degrees == 0.0 {
        return *r;
    }
    let corners = [(r.x, r.y), (r.right(), r.y), (r.right(), r.bottom()), (r.x, r.bottom())]
        .map(|(x, y)| rotate(x, y, r.cx(), r.cy(), degrees));
    let xs = corners.map(|c| c.0);
    let ys = corners.map(|c| c.1);
    let (x0, x1) = (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    let (y0, y1) = (ys.iter().copied().fold(f64::INFINITY, f64::min), ys.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Where a ray from the centre of `r` towards `(tx, ty)` leaves the node's outline: its box, or
/// its ellipse.
fn edge_point(n: &Node, r: &Rect, tx: f64, ty: f64) -> (f64, f64) {
    let (cx, cy) = (r.cx(), r.cy());
    let (dx, dy) = (tx - cx, ty - cy);
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let (hw, hh) = (r.w / 2.0, r.h / 2.0);
    let t = if n.kind == NodeKind::Ellipse {
        1.0 / ((dx / hw.max(1e-9)).powi(2) + (dy / hh.max(1e-9)).powi(2)).sqrt()
    } else {
        let tx = if dx != 0.0 { hw / dx.abs() } else { f64::INFINITY };
        let ty = if dy != 0.0 { hh / dy.abs() } else { f64::INFINITY };
        tx.min(ty)
    };
    (cx + dx * t, cy + dy * t)
}

/// A line's points in page coordinates: its own, or, where an end is attached (`from`, `to`) to
/// a node still on the page, from that node's edge. An elbow route adds its corners; a curve is
/// `[start, control, control, end]` (`curved` says so).
pub fn line_points(scene: &Scene, placed: &Placed, n: &Node) -> (Vec<(f64, f64)>, bool) {
    let r = placed.rect(&n.id).unwrap_or_default();
    let own: Vec<(f64, f64)> = n.points.as_deref().unwrap_or(&[]).iter().map(|p| (r.x + p[0], r.y + p[1])).collect();
    let attached = |end: &Option<String>| {
        end.as_deref()
            .filter(|id| *id != n.id)
            .and_then(|id| Some((scene.node(id).filter(|m| !m.is_hidden())?, placed.rect(id)?)))
    };
    let (from, to) = (attached(&n.from), attached(&n.to));
    let mut a = own.first().copied().unwrap_or((r.x, r.y));
    let mut b = own.last().copied().unwrap_or((r.right(), r.bottom()));
    if from.is_none() && to.is_none() && n.route.unwrap_or(Route::Straight) == Route::Straight {
        return (own, false);
    }
    // Each attached end aims at the other end's centre (or its free point).
    let aim_a = to.map(|(_, r)| (r.cx(), r.cy())).unwrap_or(b);
    let aim_b = from.map(|(_, r)| (r.cx(), r.cy())).unwrap_or(a);
    if let Some((m, mr)) = from {
        a = edge_point(m, &mr, aim_a.0, aim_a.1);
    }
    if let Some((m, mr)) = to {
        b = edge_point(m, &mr, aim_b.0, aim_b.1);
    }
    match n.route.unwrap_or(Route::Straight) {
        Route::Straight => (vec![a, b], false),
        Route::Elbow => {
            if (b.0 - a.0).abs() >= (b.1 - a.1).abs() {
                let mid = (a.0 + b.0) / 2.0;
                (vec![a, (mid, a.1), (mid, b.1), b], false)
            } else {
                let mid = (a.1 + b.1) / 2.0;
                (vec![a, (a.0, mid), (b.0, mid), b], false)
            }
        }
        Route::Curve => {
            let horizontal = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
            let (c1, c2) = if horizontal {
                let k = (b.0 - a.0) / 2.0;
                ((a.0 + k, a.1), (b.0 - k, b.1))
            } else {
                let k = (b.1 - a.1) / 2.0;
                ((a.0, a.1 + k), (b.0, b.1 - k))
            };
            (vec![a, c1, c2, b], true)
        }
    }
}

/// Points along a cubic curve, for hit testing it as a polyline.
pub fn flatten_curve(p: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let [a, c1, c2, b] = [p[0], p[1], p[2], p[3]];
    (0..=16)
        .map(|i| {
            let t = i as f64 / 16.0;
            let u = 1.0 - t;
            let f = |a: f64, b: f64, c: f64, d: f64| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
            (f(a.0, c1.0, c2.0, b.0), f(a.1, c1.1, c2.1, b.1))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::node;
    use relay_bus::design::NodeKind as K;

    #[test]
    fn polygons_start_at_the_top() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let tri = polygon(&r, 3, None);
        assert_eq!(tri.len(), 3);
        assert!((tri[0].0 - 50.0).abs() < 1e-9 && tri[0].1.abs() < 1e-9);
        let star = polygon(&r, 5, Some(0.4));
        assert_eq!(star.len(), 10);
        assert!(inside(&star, 50.0, 50.0));
        assert!(!inside(&star, 2.0, 2.0));
        assert!(inside(&tri, 50.0, 60.0) && !inside(&tri, 5.0, 5.0));
    }

    #[test]
    fn rotation_turns_clockwise_and_bounds_grow() {
        let (x, y) = rotate(10.0, 0.0, 0.0, 0.0, 90.0);
        assert!(x.abs() < 1e-9 && (y - 10.0).abs() < 1e-9);
        let b = rotated_bounds(&Rect::new(0.0, 0.0, 100.0, 20.0), 90.0);
        assert!((b.w - 20.0).abs() < 1e-9 && (b.h - 100.0).abs() < 1e-9);
        assert!((b.cx() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_connector_runs_edge_to_edge_and_follows() {
        let mut line = node("c", Some("p1"), K::Line, [0.0; 4]);
        line.from = Some("a".into());
        line.to = Some("b".into());
        line.points = Some(vec![[0.0, 0.0], [1.0, 1.0]]);
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]),
            node("b", Some("p1"), K::Ellipse, [300.0, 0.0, 100.0, 100.0]),
            line,
        ]);
        let p = place(&s, "p1", &Mono);
        let (pts, curved) = line_points(&s, &p, s.node("c").unwrap());
        assert!(!curved);
        assert_eq!(pts, vec![(100.0, 50.0), (300.0, 50.0)]);
        // The shape moves; the connector follows.
        s.node_mut("b").unwrap().y = 200.0;
        let p = place(&s, "p1", &Mono);
        let (pts, _) = line_points(&s, &p, s.node("c").unwrap());
        assert!(pts[1].1 > 200.0 && pts[1].0 < 350.0, "{pts:?}");
        s.node_mut("c").unwrap().route = Some(Route::Elbow);
        let (pts, _) = line_points(&s, &p, s.node("c").unwrap());
        assert_eq!(pts.len(), 4);
        assert!(pts.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1), "elbows are square: {pts:?}");
    }
}
