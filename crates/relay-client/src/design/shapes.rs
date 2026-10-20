//! Geometry of the richer nodes: a polygon's or star's corners, rotation about a node's centre,
//! where a connector runs once its ends are attached to other nodes, the outline a line or path
//! is stroked along and its arrowheads, the shadows a node casts, a ring segment's capped ends,
//! a box's rounded corners and a shape's label box. The canvas draws with these and hit testing
//! reads them, so what you click is what you see.

use super::geom::{segment_distance, Rect};
use super::layout::{Measure, Placed};
use super::scene::Scene;
use relay_bus::design::{parse_color, takes, Node, NodeKind, NodeOp, Route, Shadow, StrokeCap, MAX_COORD, MAX_SHADOW_BLUR};
use serde_json::{json, Map, Value};

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

/// One end of a ring segment: where it meets the outer and the inner edge, and the unit
/// tangent pointing away from the drawn part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcEnd {
    pub outer: (f64, f64),
    pub inner: (f64, f64),
    pub out: (f64, f64),
}

impl ArcEnd {
    /// The middle of the end, between its edges: a round cap's centre.
    pub fn mid(&self) -> (f64, f64) {
        ((self.outer.0 + self.inner.0) / 2.0, (self.outer.1 + self.inner.1) / 2.0)
    }

    /// Half the ring's thickness at this end: how far its cap reaches past it.
    pub fn cap_radius(&self) -> f64 {
        (self.outer.0 - self.inner.0).hypot(self.outer.1 - self.inner.1) / 2.0
    }
}

/// A ring segment's two ends, at `start` and at `start + sweep`, and how they are capped: only
/// for a ring segment (`inner` above 0, `sweep` under 360) whose `arc.cap` is round or square.
/// The point at angle `a` and fraction `k` of the radius is `(cx + rx·k·sin a, cy − ry·k·cos a)`.
pub fn arc_ends(n: &Node, r: &Rect) -> Option<(StrokeCap, [ArcEnd; 2])> {
    let cap = n.arc.as_ref()?.cap.filter(|c| *c != StrokeCap::Butt)?;
    let (start, sweep, inner) = arc(n)?;
    if inner <= 0.0 || sweep.abs() >= 360.0 {
        return None;
    }
    let (cx, cy, rx, ry) = (r.cx(), r.cy(), r.w / 2.0, r.h / 2.0);
    let end = |deg: f64, away: f64| {
        let (s, c) = deg.to_radians().sin_cos();
        let at = |k: f64| (cx + rx * k * s, cy - ry * k * c);
        // The way the ring runs at this angle, clockwise.
        let (tx, ty) = (rx * c, ry * s);
        let len = tx.hypot(ty).max(1e-9);
        ArcEnd { outer: at(1.0), inner: at(inner), out: (away * tx / len, away * ty / len) }
    };
    let way = if sweep >= 0.0 { 1.0 } else { -1.0 };
    Some((cap, [end(start, -way), end(start + sweep, way)]))
}

/// How far a ring segment's capped ends may reach past its box `r`: the larger end's cap
/// radius (half the ring's thickness there) when its `arc.cap` is round or square, else 0. A
/// round cap stays inside; a square one on a thick ring can pass the box by up to this.
pub fn cap_reach(n: &Node, r: &Rect) -> f64 {
    arc_ends(n, r).map_or(0.0, |(_, ends)| ends[0].cap_radius().max(ends[1].cap_radius()))
}

/// Whether `(x, y)` is on an ellipse's drawn part: inside its outline, outside its hole and
/// within its arc's sweep or on a capped end of it (a square cap is hit as a round one),
/// `slop` page pixels either way.
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
    let swept = if sweep >= 0.0 { (angle - start).rem_euclid(360.0) <= sweep } else { (start - angle).rem_euclid(360.0) <= -sweep };
    swept
        || arc_ends(n, r).is_some_and(|(_, ends)| {
            ends.iter().any(|e| {
                let m = e.mid();
                (x - m.0).hypot(y - m.1) <= e.cap_radius() + slop
            })
        })
}

/// The shadow a sticky casts when it names none: soft, 3 px down, 22% black.
pub fn sticky_shadow() -> Shadow {
    Shadow { x: 0.0, y: 3.0, blur: 10.0, spread: 0.0, color: "#00000038".into(), inset: false }
}

/// The shadows `n` draws, in paint order (the last on top): its `shadows`, else its `shadow`,
/// less any whose colour is wholly clear, which is how a node says it casts none. A sticky that
/// names none casts `sticky_shadow`. Text, lines, paths and icons cast outer shadows only and
/// without spread (a glow is one with no offset), whatever is stored or laid under by a style;
/// kinds that draw no shadow (a group, a page) cast none. Paint, hit reach and bounds read it.
pub fn effective_shadows(n: &Node) -> Vec<Shadow> {
    if !takes(n.kind, "shadow") {
        return Vec::new();
    }
    let list = n.shadow_list();
    if list.is_empty() {
        return if n.kind == NodeKind::Sticky { vec![sticky_shadow()] } else { Vec::new() };
    }
    let glow = matches!(n.kind, NodeKind::Text | NodeKind::Line | NodeKind::Path | NodeKind::Icon);
    list.iter()
        .filter(|s| !parse_color(&s.color).is_some_and(|c| c[3] == 0))
        .filter(|s| !(glow && s.inset))
        .map(|s| if glow { Shadow { spread: 0.0, ..s.clone() } } else { s.clone() })
        .collect()
}

/// How far `n`'s outer shadows reach past what casts them, whatever its turn: the largest of
/// twice the blur (capped at 200, as paint draws it), the spread past the edge and the offset's
/// length.
pub fn shadow_reach(n: &Node) -> f64 {
    effective_shadows(n)
        .iter()
        .filter(|s| !s.inset)
        .map(|s| s.blur.clamp(0.0, MAX_SHADOW_BLUR) * 2.0 + s.spread.max(0.0) + s.x.hypot(s.y))
        .fold(0.0, f64::max)
}

/// A kind's corner radius when it names none: a sticky's 6, 0 for everything else.
pub fn default_radius(kind: NodeKind) -> f64 {
    if kind == NodeKind::Sticky {
        6.0
    } else {
        0.0
    }
}

/// The one corner radius a field shows for `n`: the largest of its `radii`, else its `radius`,
/// else its kind's default (a sticky's 6).
pub fn radius(n: &Node) -> f64 {
    match n.radii {
        Some(radii) => radii.into_iter().fold(0.0, f64::max),
        None => n.radius.unwrap_or(default_radius(n.kind)),
    }
}

/// The edits that give `ids` corner radius `v` (0 or more) on every corner: `radius` null where
/// `v` is the kind's default, so a sticky at 6 keeps its default and a sticky at 0 is written,
/// and `radii` cleared. Nodes whose kind has no corners are left out.
pub fn radius_ops(scene: &Scene, ids: &[String], v: f64) -> Vec<NodeOp> {
    let v = if v.is_finite() { v.clamp(0.0, MAX_COORD) } else { 0.0 };
    ids.iter()
        .filter_map(|id| scene.node(id))
        .filter(|n| takes(n.kind, "radius"))
        .map(|n| {
            let mut set = Map::new();
            set.insert("radius".into(), if v == default_radius(n.kind) { Value::Null } else { json!(v) });
            set.insert("radii".into(), Value::Null);
            NodeOp::Update { id: n.id.clone(), set }
        })
        .collect()
}

/// A box's corner radii as painted, top-left, top-right, bottom-right, bottom-left: `radii`,
/// else `radius` (a sticky's 6 by default) on every corner, each at most half the box's shorter
/// side; all 0 for kinds whose box is not rounded.
pub fn corner_radii(n: &Node, r: &Rect) -> [f64; 4] {
    if !matches!(n.kind, NodeKind::Rect | NodeKind::Frame | NodeKind::Sticky | NodeKind::Image) {
        return [0.0; 4];
    }
    let cap = |v: f64| v.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    n.radii.unwrap_or([n.radius.unwrap_or(default_radius(n.kind)); 4]).map(cap)
}

/// The centre of each rounded corner of `r` with its radius and the way out of the box there,
/// top-left first, clockwise.
fn corners(r: &Rect, [tl, tr, br, bl]: [f64; 4]) -> [(f64, (f64, f64), (f64, f64)); 4] {
    [
        (tl, (r.x + tl, r.y + tl), (-1.0, -1.0)),
        (tr, (r.right() - tr, r.y + tr), (1.0, -1.0)),
        (br, (r.right() - br, r.bottom() - br), (1.0, 1.0)),
        (bl, (r.x + bl, r.bottom() - bl), (-1.0, 1.0)),
    ]
}

/// Whether `(x, y)` is in `r` with its corners rounded by `radii` (`corner_radii`), reaching
/// `pad_x` and `pad_y` past its sides (and the smaller of the two past a rounded corner).
pub fn in_rounded_box(r: &Rect, radii: [f64; 4], x: f64, y: f64, pad_x: f64, pad_y: f64) -> bool {
    if x < r.x - pad_x || x > r.right() + pad_x || y < r.y - pad_y || y > r.bottom() + pad_y {
        return false;
    }
    let pad = pad_x.min(pad_y);
    corners(r, radii).iter().all(|&(c, (cx, cy), (sx, sy))| c <= 0.0 || (x - cx) * sx <= 0.0 || (y - cy) * sy <= 0.0 || (x - cx).hypot(y - cy) <= c + pad)
}

/// Whether `n` is a connector: a line drawn from its ends (`from`, `to`) or along a `route`,
/// not from its own points.
pub fn routed(n: &Node) -> bool {
    n.kind == NodeKind::Line && (n.from.is_some() || n.to.is_some() || n.route.is_some())
}

/// Whether `n` is drawn as a closed shape: a free line or path of three or more points with
/// `closed` (fewer points have no inside, and a connector is never closed). It strokes inside
/// its edge as a polygon does, and draws no arrowheads.
pub fn closed_shape(n: &Node) -> bool {
    fillable_line(n) && n.closed == Some(true)
}

/// Whether a free line or path has an inside to fill: three or more points, closed or not.
pub fn fillable_line(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Line | NodeKind::Path) && !routed(n) && n.points.as_ref().is_some_and(|p| p.len() > 2)
}

/// Where a rect's, ellipse's or polygon's label sits inside its box `r`, as top, right, bottom
/// and left: `padding`, else 12 px at the sides, less on a narrow shape (an eighth of its
/// width), so a small badge keeps room for its words.
pub fn label_inset(n: &Node, r: &Rect) -> [f64; 4] {
    if n.padding.is_some() {
        n.padding_edges()
    } else {
        let side = 12f64.min(r.w / 8.0).max(0.0);
        [0.0, side, 0.0, side]
    }
}

/// A rect's, ellipse's or polygon's label box in its box `r`: inside `label_inset`, as wide as
/// that leaves (the width its words wrap at), as tall as its words, centred on the inset box.
/// It may be taller than the shape. `None` without words. Paint draws the label here and
/// bounds count it.
pub fn label_box(n: &Node, r: &Rect, measure: &dyn Measure) -> Option<Rect> {
    if n.text.as_deref().is_none_or(str::is_empty) {
        return None;
    }
    let [t, rt, b, l] = label_inset(n, r);
    let inner = Rect::new(r.x + l, r.y + t, (r.w - l - rt).max(1.0), (r.h - t - b).max(0.0));
    let (_, h) = measure.measure(n, Some(inner.w));
    Some(Rect::new(inner.x, inner.cy() - h / 2.0, inner.w, h))
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

/// One piece of a stroke, from where the piece before it ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    /// Straight to a point.
    Line((f64, f64)),
    /// A quadratic curve: its control point and its end.
    Quad((f64, f64), (f64, f64)),
    /// A cubic curve: its two control points and its end.
    Cubic((f64, f64), (f64, f64), (f64, f64)),
}

/// A free line's or path's stroke as the painter draws it, from its first point (`origin` plus
/// each of `points`): one point is a dot; a `smooth` line is a Catmull-Rom curve through its
/// points; a path of three or more points runs in quadratic curves turning at each point
/// between the midpoints, then straight to its last; anything else runs straight from point to
/// point; and a closed shape (`closed_shape`) runs back to its start. The start and the pieces,
/// `None` without points. Paint builds its path from this and hit testing flattens it, so the
/// two cannot drift apart.
pub fn path_segments(n: &Node, origin: (f64, f64)) -> Option<((f64, f64), Vec<Seg>)> {
    let points = n.points.as_deref().filter(|p| !p.is_empty())?;
    let at = |p: &[f64; 2]| (origin.0 + p[0], origin.1 + p[1]);
    let start = at(&points[0]);
    let closed = closed_shape(n);
    let mut segs = Vec::with_capacity(points.len() + 1);
    if points.len() == 1 {
        segs.push(Seg::Line((start.0 + 0.01, start.1)));
    } else if n.kind == NodeKind::Line && n.smooth == Some(true) && points.len() > 2 {
        let pts: Vec<(f64, f64)> = points.iter().map(at).collect();
        segs.extend(smooth_segments(&pts, closed).into_iter().map(|[c1, c2, to]| Seg::Cubic(c1, c2, to)));
    } else if n.kind == NodeKind::Path && points.len() > 2 {
        for w in points.windows(2).skip(1) {
            let (c, next) = (at(&w[0]), at(&w[1]));
            segs.push(Seg::Quad(c, ((c.0 + next.0) / 2.0, (c.1 + next.1) / 2.0)));
        }
        segs.push(Seg::Line(at(&points[points.len() - 1])));
    } else {
        segs.extend(points[1..].iter().map(at).map(Seg::Line));
    }
    if closed {
        segs.push(Seg::Line(start));
    }
    Some((start, segs))
}

/// A free line's or path's outline in page coordinates as a polyline: `path_segments` from its
/// box's corner, curves flattened. Connectors are `line_points`'s.
pub fn stroke_outline(n: &Node, r: &Rect) -> Vec<(f64, f64)> {
    let Some((start, segs)) = path_segments(n, (r.x, r.y)) else {
        return Vec::new();
    };
    let mut out = vec![start];
    let mut from = start;
    for seg in segs {
        let to = match seg {
            Seg::Line(to) => {
                out.push(to);
                to
            }
            Seg::Quad(c, to) => {
                // The same curve as a cubic: each control two thirds of the way to `c`.
                let third = |p: (f64, f64)| (p.0 + 2.0 / 3.0 * (c.0 - p.0), p.1 + 2.0 / 3.0 * (c.1 - p.1));
                flatten_into(&mut out, [from, third(from), third(to), to]);
                to
            }
            Seg::Cubic(c1, c2, to) => {
                flatten_into(&mut out, [from, c1, c2, to]);
                to
            }
        };
        from = to;
    }
    out
}

/// Append a cubic's points after its start, in as few steps as keep it within about a quarter
/// pixel (1 to 16), so a long pen path stays cheap to hover.
fn flatten_into(out: &mut Vec<(f64, f64)>, [a, c1, c2, b]: [(f64, f64); 4]) {
    let bend = segment_distance(c1.0, c1.1, a, b).max(segment_distance(c2.0, c2.1, a, b));
    let steps = ((bend / 0.25).sqrt().ceil() as usize).clamp(1, 16);
    out.extend((1..=steps).map(|i| cubic_at([a, c1, c2, b], i as f64 / steps as f64)));
}

fn cubic_at([a, c1, c2, b]: [(f64, f64); 4], t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let f = |a: f64, b: f64, c: f64, d: f64| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
    (f(a.0, c1.0, c2.0, b.0), f(a.1, c1.1, c2.1, b.1))
}

/// How far an arrowhead's wings reach to each side of its line, as a fraction of its length.
pub const ARROW_WING: f64 = 0.55;

/// The weight paint strokes a line or path at: `stroke_width`, else 2, at least 0.5.
pub fn line_width(n: &Node) -> f64 {
    n.stroke_width.unwrap_or(2.0).max(0.5)
}

/// An arrowhead's length along its line: three and a half times the weight, at least 8 px.
pub fn arrow_size(n: &Node) -> f64 {
    (line_width(n) * 3.5).max(8.0)
}

/// The arrowheads a line or path draws, end first, as triangles (tip, then its two wings) in
/// the coordinates its points are drawn in, with `r` its placed box: one per end that asks for
/// one (`arrow_end`, `arrow_start`) on the last or first segment, as long as it is. A closed
/// shape draws none. Paint fills these and hit testing takes them, so a click on a wing selects
/// the line.
pub fn arrowheads(scene: &Scene, placed: &Placed, n: &Node, r: &Rect) -> Vec<[(f64, f64); 3]> {
    let wants = |end: Option<bool>| end == Some(true);
    if !matches!(n.kind, NodeKind::Line | NodeKind::Path) || closed_shape(n) || !(wants(n.arrow_end) || wants(n.arrow_start)) {
        return Vec::new();
    }
    let pts: Vec<(f64, f64)> = if routed(n) {
        line_points(scene, placed, n).0
    } else {
        n.points.iter().flatten().map(|p| (r.x + p[0], r.y + p[1])).collect()
    };
    if pts.len() < 2 {
        return Vec::new();
    }
    let (last, size) = (pts.len() - 1, arrow_size(n));
    [(n.arrow_end, pts[last - 1], pts[last]), (n.arrow_start, pts[1], pts[0])]
        .into_iter()
        .filter(|(end, _, _)| wants(*end))
        .filter_map(|(_, from, tip)| {
            let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
            let len = dx.hypot(dy);
            if len < 0.001 {
                return None;
            }
            let (ux, uy) = (dx / len, dy / len);
            let back = (tip.0 - ux * size, tip.1 - uy * size);
            let (px, py) = (-uy * size * ARROW_WING, ux * size * ARROW_WING);
            Some([tip, (back.0 + px, back.1 + py), (back.0 - px, back.1 - py)])
        })
        .collect()
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

/// The node an end of `n` (`from` or `to`) is attached to and its placed box: one that is not
/// `n` itself, still in the scene, shown and placed.
fn attached<'a>(scene: &'a Scene, placed: &Placed, n: &Node, end: &Option<String>) -> Option<(&'a Node, Rect)> {
    end.as_deref().filter(|id| *id != n.id).and_then(|id| Some((scene.node(id).filter(|m| !m.is_hidden())?, placed.rect(id)?)))
}

/// Whether each end of a line, `from` then `to`, is attached to a shown node, so that it is
/// drawn from that node's edge rather than from the line's own box and points. A line attached
/// at both ends draws nothing of its own `x`, `y`, `w`, `h` or `points`.
pub fn attached_ends(scene: &Scene, placed: &Placed, n: &Node) -> (bool, bool) {
    (attached(scene, placed, n, &n.from).is_some(), attached(scene, placed, n, &n.to).is_some())
}

/// A line's points in page coordinates: its own, or, where an end is attached (`from`, `to`) to
/// a node still on the page, from that node's edge. An elbow route adds its corners; a curve is
/// `[start, control, control, end]` (`curved` says so).
pub fn line_points(scene: &Scene, placed: &Placed, n: &Node) -> (Vec<(f64, f64)>, bool) {
    let r = placed.rect(&n.id).unwrap_or_default();
    let own: Vec<(f64, f64)> = n.points.as_deref().unwrap_or(&[]).iter().map(|p| (r.x + p[0], r.y + p[1])).collect();
    let (from, to) = (attached(scene, placed, n, &n.from), attached(scene, placed, n, &n.to));
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
    let q = [p[0], p[1], p[2], p[3]];
    (0..=16).map(|i| cubic_at(q, i as f64 / 16.0)).collect()
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
        pie.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: 90.0, inner: 0.5, cap: None });
        // The top-right quarter, less its hole.
        assert!(on_ellipse(&pie, &r, 80.0, 30.0, 0.0));
        assert!(!on_ellipse(&pie, &r, 20.0, 30.0, 0.0), "outside the sweep");
        assert!(!on_ellipse(&pie, &r, 55.0, 45.0, 0.0), "in the hole");
        pie.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: -90.0, inner: 0.0, cap: None });
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
        let curve = stroke_outline(&line, &Rect::new(0.0, 0.0, 100.0, 40.0));
        assert!(curve.len() > 12 && curve.contains(&(50.0, 40.0)), "a curve through its points: {curve:?}");
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

    /// One page holding `nodes`, placed.
    fn page(nodes: Vec<Node>) -> (Scene, Placed) {
        let s = Scene::new(std::iter::once(node("p1", None, K::Page, [0.0; 4])).chain(nodes).collect());
        let p = place(&s, "p1", &Mono);
        (s, p)
    }

    fn ring(cap: Option<StrokeCap>) -> Node {
        let mut e = node("e", Some("p1"), K::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        e.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: 90.0, inner: 0.6, cap });
        e
    }

    #[test]
    fn round_caps_sit_on_the_middle_of_the_ring() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(arc_ends(&ring(None), &r).is_none() && arc_ends(&ring(Some(StrokeCap::Butt)), &r).is_none(), "butt ends are the plain outline");
        let (cap, [start, end]) = arc_ends(&ring(Some(StrokeCap::Round)), &r).unwrap();
        assert_eq!(cap, StrokeCap::Round);
        // The ring runs from 30 to 50 px out from the centre: its middle is 40 out, its caps 10 round.
        for e in [start, end] {
            let m = e.mid();
            assert!(((m.0 - 50.0).hypot(m.1 - 50.0) - 40.0).abs() < 1e-9);
            assert!((e.cap_radius() - 10.0).abs() < 1e-9);
        }
        assert!(start.out.0 < -0.99 && start.out.1.abs() < 1e-9, "the start at the top looks back left: {:?}", start.out);
        assert!(end.out.1 > 0.99, "the end at 3 o'clock runs on downward: {:?}", end.out);
        assert!((cap_reach(&ring(Some(StrokeCap::Square)), &r) - 10.0).abs() < 1e-9);
        assert_eq!(cap_reach(&ring(None), &r), 0.0);
        // Just before the start at the top, on the middle of the ring.
        let (x, y) = (45.0, 10.5);
        assert!(on_ellipse(&ring(Some(StrokeCap::Round)), &r, x, y, 0.0), "inside the round cap");
        assert!(!on_ellipse(&ring(None), &r, x, y, 0.0), "past a butt end");
        let mut whole = ring(Some(StrokeCap::Round));
        whole.arc.as_mut().unwrap().inner = 0.0;
        assert!(arc_ends(&whole, &r).is_none(), "a pie has no ends to shape");
    }

    #[test]
    fn effective_shadow_sticky_default_and_explicit_none() {
        // B37: a sticky drew a built-in shadow nothing could take away.
        let sticky = node("s", Some("p1"), K::Sticky, [0.0; 4]);
        assert_eq!(effective_shadows(&sticky), vec![sticky_shadow()]);
        let clear = Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: "#00000000".into(), inset: false };
        let mut none = sticky.clone();
        none.shadow = Some(clear.clone());
        assert!(effective_shadows(&none).is_empty(), "a clear shadow says none");
        assert!(effective_shadows(&node("r", Some("p1"), K::Rect, [0.0; 4])).is_empty());
        let drop = Shadow { x: 2.0, y: 4.0, blur: 8.0, spread: 1.0, color: "#00000040".into(), inset: false };
        let mut rect = node("r", Some("p1"), K::Rect, [0.0; 4]);
        rect.shadow = Some(drop.clone());
        assert_eq!(effective_shadows(&rect), vec![drop.clone()]);
        // Several: the clear ones drop out, and text casts outer ones only, without spread.
        let inner = Shadow { inset: true, ..drop.clone() };
        let mut text = node("t", Some("p1"), K::Text, [0.0; 4]);
        text.shadows = Some(vec![drop.clone(), clear, inner.clone()]);
        assert_eq!(effective_shadows(&text), vec![Shadow { spread: 0.0, ..drop.clone() }]);
        rect.shadow = None;
        rect.shadows = text.shadows.clone();
        assert_eq!(effective_shadows(&rect), vec![drop.clone(), inner]);
        let mut group = node("g", Some("p1"), K::Group, [0.0; 4]);
        group.shadow = Some(drop);
        assert!(effective_shadows(&group).is_empty(), "a group draws no shadow");
    }

    #[test]
    fn shadow_reach_takes_the_largest_and_ignores_inset() {
        let s = |x: f64, y: f64, blur: f64, spread: f64, inset: bool| Shadow { x, y, blur, spread, color: "#000".into(), inset };
        let mut rect = node("r", Some("p1"), K::Rect, [0.0; 4]);
        rect.shadows = Some(vec![s(3.0, 4.0, 2.0, 1.0, false), s(0.0, 0.0, 10.0, 0.0, false), s(0.0, 0.0, 50.0, 30.0, true)]);
        // 5 (the offset's length) + 4 + 1, against a glow of 20.
        assert_eq!(shadow_reach(&rect), 20.0);
        rect.shadows = Some(vec![s(30.0, 40.0, 0.0, -4.0, false)]);
        assert_eq!(shadow_reach(&rect), 50.0, "an offset counts by its length; a negative spread adds nothing");
        assert_eq!(shadow_reach(&node("s", Some("p1"), K::Sticky, [0.0; 4])), 23.0, "a sticky's own: blur 10, 3 down");
        assert_eq!(shadow_reach(&node("t", Some("p1"), K::Text, [0.0; 4])), 0.0);
    }

    #[test]
    fn radius_and_its_default() {
        // B35: a sticky's corners are 6 unless it says.
        let mut sticky = node("s", Some("p1"), K::Sticky, [0.0; 4]);
        assert_eq!((radius(&sticky), radius(&node("r", Some("p1"), K::Rect, [0.0; 4]))), (6.0, 0.0));
        sticky.radii = Some([0.0, 12.0, 4.0, 0.0]);
        assert_eq!(radius(&sticky), 12.0, "per-corner radii show their largest");
        assert_eq!(corner_radii(&sticky, &Rect::new(0.0, 0.0, 100.0, 10.0)), [0.0, 5.0, 4.0, 0.0], "capped at half the shorter side");
        assert_eq!(corner_radii(&node("s", Some("p1"), K::Sticky, [0.0; 4]), &Rect::new(0.0, 0.0, 100.0, 100.0)), [6.0; 4]);
        assert_eq!(corner_radii(&node("e", Some("p1"), K::Ellipse, [0.0; 4]), &Rect::new(0.0, 0.0, 100.0, 100.0)), [0.0; 4]);
    }

    #[test]
    fn radius_ops_squares_a_sticky_in_a_mixed_selection() {
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("r", Some("p1"), K::Rect, [0.0; 4]),
            node("s", Some("p1"), K::Sticky, [0.0; 4]),
            node("e", Some("p1"), K::Ellipse, [0.0; 4]),
        ]);
        let ids = ["r", "s", "e"].map(String::from);
        let sets = |v: f64| -> Vec<(String, Value)> {
            radius_ops(&s, &ids, v)
                .into_iter()
                .map(|op| match op {
                    NodeOp::Update { id, set } => (id, Value::Object(set)),
                    other => panic!("{other:?}"),
                })
                .collect()
        };
        assert_eq!(sets(0.0), [("r".into(), json!({"radius": null, "radii": null})), ("s".into(), json!({"radius": 0.0, "radii": null}))], "an ellipse has no corners");
        assert_eq!(sets(6.0), [("r".into(), json!({"radius": 6.0, "radii": null})), ("s".into(), json!({"radius": null, "radii": null}))], "a sticky's 6 is its default");
        assert_eq!(sets(-3.0)[1].1, json!({"radius": 0.0, "radii": null}));
    }

    #[test]
    fn closed_shape_needs_three_free_points() {
        // B95 and B98: one test of "really closed" and "has an inside" for paint, hit and panels.
        let mut line = node("l", Some("p1"), K::Line, [0.0; 4]);
        line.points = Some(vec![[0.0, 0.0], [10.0, 0.0], [5.0, 8.0]]);
        line.closed = Some(true);
        assert!(closed_shape(&line) && fillable_line(&line));
        let mut two = line.clone();
        two.points = Some(vec![[0.0, 0.0], [10.0, 0.0]]);
        assert!(!closed_shape(&two) && !fillable_line(&two), "two points have no inside");
        let mut joined = line.clone();
        joined.from = Some("a".into());
        assert!(!closed_shape(&joined) && !fillable_line(&joined), "a connector is never closed");
        let mut routed_line = line.clone();
        routed_line.route = Some(Route::Elbow);
        assert!(!closed_shape(&routed_line));
        let mut path = line.clone();
        path.kind = K::Path;
        assert!(closed_shape(&path));
        path.closed = None;
        assert!(!closed_shape(&path) && fillable_line(&path), "an open path of three points still fills");
    }

    #[test]
    fn labels_sit_inside_their_inset() {
        let mut rect = node("r", Some("p1"), K::Rect, [0.0; 4]);
        let r = Rect::new(0.0, 0.0, 120.0, 40.0);
        assert!(label_box(&rect, &r, &Mono).is_none(), "no words, no label");
        rect.text = Some("Go".into());
        assert_eq!(label_box(&rect, &r, &Mono), Some(Rect::new(12.0, 10.0, 96.0, 20.0)), "12 px in, centred");
        rect.padding = Some(vec![4.0, 20.0]);
        assert_eq!(label_box(&rect, &r, &Mono), Some(Rect::new(20.0, 10.0, 80.0, 20.0)));
        // E-text-12: a small badge keeps room for its words.
        rect.padding = None;
        let badge = label_box(&rect, &Rect::new(0.0, 0.0, 40.0, 20.0), &Mono).unwrap();
        assert!(badge.w >= 30.0, "{badge:?}");
        // B32: a long label on a short box is taller than it.
        rect.text = Some("x".repeat(40));
        let tall = label_box(&rect, &r, &Mono).unwrap();
        assert!(tall.y < r.y && tall.bottom() > r.bottom(), "{tall:?}");
    }

    #[test]
    fn a_path_runs_through_its_midpoints() {
        // B118: one description of the stroke for paint and hit.
        let mut v = node("v", Some("p1"), K::Path, [0.0; 4]);
        v.points = Some(vec![[0.0, 0.0], [50.0, 100.0], [100.0, 0.0], [150.0, 100.0]]);
        let (start, segs) = path_segments(&v, (10.0, 0.0)).unwrap();
        assert_eq!(start, (10.0, 0.0));
        assert_eq!(segs, [Seg::Quad((60.0, 100.0), (85.0, 50.0)), Seg::Quad((110.0, 0.0), (135.0, 50.0)), Seg::Line((160.0, 100.0))]);
        v.points = Some(vec![[3.0, 4.0]]);
        assert_eq!(path_segments(&v, (0.0, 0.0)).unwrap().1, [Seg::Line((3.01, 4.0))], "a dot");
        let mut line = node("l", Some("p1"), K::Line, [0.0; 4]);
        line.points = Some(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        line.closed = Some(true);
        assert_eq!(path_segments(&line, (0.0, 0.0)).unwrap().1, [Seg::Line((10.0, 0.0)), Seg::Line((10.0, 10.0)), Seg::Line((0.0, 0.0))]);
        line.points = None;
        assert!(path_segments(&line, (0.0, 0.0)).is_none() && stroke_outline(&line, &Rect::default()).is_empty());
        // A long pen stroke of gentle bends flattens in few points.
        let mut pen = node("pen", Some("p1"), K::Path, [0.0; 4]);
        pen.points = Some((0..200).map(|i| [i as f64 * 4.0, if i % 2 == 0 { 0.0 } else { 0.4 }]).collect());
        assert!(stroke_outline(&pen, &Rect::default()).len() < 600);
    }

    #[test]
    fn arrowheads_are_drawn_on_the_end_segments() {
        let mut line = node("l", Some("p1"), K::Line, [200.0, 200.0, 100.0, 0.0]);
        line.points = Some(vec![[0.0, 0.0], [100.0, 0.0]]);
        line.stroke_width = Some(10.0);
        let (s, p) = page(vec![line.clone()]);
        let r = p.rect("l").unwrap();
        assert!(arrowheads(&s, &p, &line, &r).is_empty());
        line.arrow_end = Some(true);
        line.arrow_start = Some(true);
        let heads = arrowheads(&s, &p, &line, &r);
        // 35 px long (10 × 3.5), its wings 0.55 × 35 to each side.
        assert_eq!(heads[0], [(300.0, 200.0), (265.0, 219.25), (265.0, 180.75)]);
        assert_eq!(heads[1][0], (200.0, 200.0), "the start's tip is the first point");
        line.points = Some(vec![[0.0, 0.0], [100.0, 0.0], [50.0, 50.0]]);
        line.closed = Some(true);
        assert!(arrowheads(&s, &p, &line, &r).is_empty(), "a closed shape has no ends");
        line.points = Some(vec![[0.0, 0.0], [0.0, 0.0]]);
        assert!(arrowheads(&s, &p, &line, &r).is_empty(), "no direction, no head");
        assert_eq!(arrow_size(&node("t", Some("p1"), K::Line, [0.0; 4])), 8.0);
    }

    #[test]
    fn attached_ends_needs_both_visible() {
        // B91: a line attached at both ends draws nothing of its own box.
        let mut c = node("c", Some("p1"), K::Line, [0.0; 4]);
        (c.from, c.to, c.points) = (Some("a".into()), Some("b".into()), Some(vec![[0.0, 0.0], [1.0, 1.0]]));
        let mut nodes = vec![node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]), node("b", Some("p1"), K::Rect, [300.0, 0.0, 100.0, 100.0]), c.clone()];
        let (s, p) = page(nodes.clone());
        assert_eq!(attached_ends(&s, &p, &c), (true, true));
        let drawn = line_points(&s, &p, &c);
        let mut moved = c.clone();
        (moved.x, moved.y, moved.w, moved.h, moved.points) = (40.0, 70.0, 9.0, 9.0, Some(vec![[5.0, 5.0], [9.0, 0.0]]));
        let (s2, p2) = page(vec![nodes[0].clone(), nodes[1].clone(), moved.clone()]);
        assert_eq!(line_points(&s2, &p2, &moved), drawn, "its own box and points are not read");
        nodes[1].hidden = Some(true);
        let (s, p) = page(nodes.clone());
        assert_eq!(attached_ends(&s, &p, &c), (true, false));
        c.from = Some("c".into());
        assert!(!attached_ends(&s, &p, &c).0, "a line is not attached to itself");
    }
}
