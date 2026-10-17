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

/// The corners of a polygon node in its placed box (a star has 5 points unless it says),
/// mirrored as it is flipped.
pub fn node_polygon(n: &Node, r: &Rect) -> Vec<(f64, f64)> {
    let sides = n.sides.unwrap_or(if n.star.is_some() { 5 } else { 3 });
    polygon(r, sides, n.star).into_iter().map(|p| flipped(n, r, p)).collect()
}

/// `p` mirrored inside `r` as `n` is flipped (`flip_x`, `flip_y`).
pub fn flipped(n: &Node, r: &Rect, (x, y): (f64, f64)) -> (f64, f64) {
    let x = if n.flip_x == Some(true) { 2.0 * r.cx() - x } else { x };
    let y = if n.flip_y == Some(true) { 2.0 * r.cy() - y } else { y };
    (x, y)
}

/// An ellipse's arc as start and sweep in degrees (clockwise from the top, the sweep within
/// ±360) and its hole as a fraction of the radius; `None` for a whole ellipse.
pub fn arc(n: &Node) -> Option<(f64, f64, f64)> {
    let a = n.arc.as_ref().filter(|_| n.kind == NodeKind::Ellipse)?;
    let sweep = a.sweep.clamp(-360.0, 360.0);
    let inner = a.inner.clamp(0.0, 0.99);
    (sweep.abs() < 360.0 || inner > 0.0).then_some((a.start, sweep, inner))
}

/// Whether `(x, y)` is on an ellipse's drawn part: inside its outline, outside its hole and
/// within its arc's sweep, `slop` page pixels either way.
pub fn on_ellipse(n: &Node, r: &Rect, x: f64, y: f64, slop: f64) -> bool {
    let (rx, ry) = (r.w / 2.0, r.h / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let (u, v) = ((x - r.cx()) / (rx + slop), (y - r.cy()) / (ry + slop));
    let d = (u * u + v * v).sqrt();
    if d > 1.0 {
        return false;
    }
    let Some((start, sweep, inner)) = arc(n) else {
        return true;
    };
    let (hu, hv) = ((x - r.cx()) / (rx * inner - slop).max(1e-9), (y - r.cy()) / (ry * inner - slop).max(1e-9));
    if inner > 0.0 && hu * hu + hv * hv < 1.0 {
        return false;
    }
    if sweep.abs() >= 360.0 {
        return true;
    }
    let angle = u.atan2(-v).to_degrees().rem_euclid(360.0);
    if sweep >= 0.0 {
        (angle - start).rem_euclid(360.0) <= sweep
    } else {
        (start - angle).rem_euclid(360.0) <= -sweep
    }
}

/// A smooth curve through `pts` (Catmull-Rom) as cubic segments: each its two control points
/// and its end, from `pts[0]`. Closed, it runs back to the first point.
pub fn smooth_segments(pts: &[(f64, f64)], closed: bool) -> Vec<[(f64, f64); 3]> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let at = |i: isize| -> (f64, f64) {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let count = if closed { n } else { n - 1 };
    (0..count as isize)
        .map(|i| {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
            let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
            [c1, c2, p2]
        })
        .collect()
}

/// A free line's or path's outline in page coordinates as a polyline: its points, the smooth
/// curve through them, and back to the start when closed. Connectors are `line_points`'s.
pub fn stroke_outline(n: &Node, r: &Rect) -> Vec<(f64, f64)> {
    let pts: Vec<(f64, f64)> = n.points.as_deref().unwrap_or(&[]).iter().map(|p| (r.x + p[0], r.y + p[1])).collect();
    let closed = n.closed == Some(true) && pts.len() > 2;
    if n.kind == NodeKind::Line && n.smooth == Some(true) && pts.len() > 2 {
        let mut out = vec![pts[0]];
        let mut from = pts[0];
        for [c1, c2, to] in smooth_segments(&pts, closed) {
            out.extend(flatten_curve(&[from, c1, c2, to]).into_iter().skip(1));
            from = to;
        }
        return out;
    }
    let mut out = pts;
    if closed {
        out.push(out[0]);
    }
    out
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
    fn flips_mirror_corners_and_arcs_cut_the_ellipse() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut tri = node("t", Some("p1"), K::Polygon, [0.0, 0.0, 100.0, 100.0]);
        tri.flip_y = Some(true);
        let pts = node_polygon(&tri, &r);
        assert!((pts[0].1 - 100.0).abs() < 1e-9, "the apex is at the bottom: {pts:?}");
        let mut pie = node("e", Some("p1"), K::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        assert!(arc(&pie).is_none());
        pie.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: 90.0, inner: 0.5 });
        // The top-right quarter, less its hole.
        assert!(on_ellipse(&pie, &r, 80.0, 30.0, 0.0));
        assert!(!on_ellipse(&pie, &r, 20.0, 30.0, 0.0), "outside the sweep");
        assert!(!on_ellipse(&pie, &r, 55.0, 45.0, 0.0), "in the hole");
        pie.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: -90.0, inner: 0.0 });
        assert!(on_ellipse(&pie, &r, 20.0, 30.0, 0.0), "a negative sweep runs anticlockwise");
    }

    #[test]
    fn smooth_lines_pass_through_their_points() {
        let pts = [(0.0, 0.0), (50.0, 40.0), (100.0, 0.0)];
        let segs = smooth_segments(&pts, false);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0][2], (50.0, 40.0));
        assert_eq!(segs[1][2], (100.0, 0.0));
        assert_eq!(smooth_segments(&pts, true).len(), 3);
        let mut line = node("l", Some("p1"), K::Line, [0.0; 4]);
        line.points = Some(vec![[0.0, 0.0], [50.0, 40.0], [100.0, 0.0]]);
        line.closed = Some(true);
        let outline = stroke_outline(&line, &Rect::new(10.0, 10.0, 100.0, 40.0));
        assert_eq!((outline.first(), outline.last()), (Some(&(10.0, 10.0)), Some(&(10.0, 10.0))));
        line.smooth = Some(true);
        assert!(stroke_outline(&line, &Rect::new(0.0, 0.0, 100.0, 40.0)).len() > 40);
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
