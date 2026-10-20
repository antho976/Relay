//! Flip and copy style: the edits behind Shift H / Shift V and Ctrl Alt C / Ctrl Alt V.
//!
//! A flip mirrors each selected layer about the middle of what the selection draws, and turns
//! its own drawing over: a line's or path's points, a box's corner radii, a gradient's and a
//! stroke gradient's angle (a conic one's stops then run the other way), an arc, a turn, how it
//! moves when it plays, and the `flip_x`/`flip_y` of a polygon, picture or icon. Shadows keep
//! their offset: the light still comes from where it did. A frame or group mirrors what is
//! inside it the same way, hidden layers too, and a frame its children's pins with them; one
//! that lays its children out keeps their order and mirrors only its absolute ones. A connector
//! attached at one end mirrors its free end; one attached at both follows what it joins. A
//! locked layer is left as it is, unless what holds it flips.
//!
//! Copy style takes a layer's look as stored, the name of its style and its colour names kept;
//! paste lays it on each selected layer where its kind uses it.

use super::edit::{outermost, px, unlocked};
use super::geom::Rect;
use super::layout::{fit_of, is_laid, natural_size, Fit, Measure, Placed};
use super::scene::Scene;
use super::shapes;
use relay_bus::design::motion::{MAX_ORIGIN, MAX_TURN};
use relay_bus::design::{
    fit_written, follow_axis, takes, Arc, Gradient, GradientKind, Motion, Node, NodeKind, NodeOp, Pin, PinX, PinY, MAX_COORD,
};
use serde_json::{json, Map, Value};
use std::borrow::Cow;

/// The edits that flip `ids` left to right (`horizontal`) or top to bottom, each mirrored about
/// the middle of what they draw (a line's points, wherever its box is: B21). A hidden one is
/// mirrored too, at the size `measure` and layout give it shown (B22); a locked one is left out
/// (B16).
pub fn flip(scene: &Scene, placed: &Placed, measure: &dyn Measure, ids: &[String], horizontal: bool) -> Vec<NodeOp> {
    let ids = unlocked(scene, outermost(scene, ids));
    let drawn: Vec<Rect> = ids.iter().filter_map(|id| drawn_rect(scene, placed, id)).collect();
    let Some(about) = Rect::bounds(drawn.iter()) else {
        return Vec::new();
    };
    let flipping = ids.iter().flat_map(|id| scene.subtree(id)).collect();
    let mut f = Flip { scene, placed, measure, horizontal, flipping, ops: Vec::new() };
    for id in &ids {
        let parent = scene.parent(id);
        let within = Within {
            origin: parent.map_or((0.0, 0.0), |p| f.origin(p)),
            fit: parent.and_then(|p| fit_of(scene, placed, p)),
            frame: false,
        };
        f.mirror(id, &about, parent.is_some() && !is_laid(scene, id), within);
    }
    f.ops
}

/// Where layer `id` draws on the page: its shown box (a line's points, wherever its box is), and
/// for a group what its layers draw, every level of it, since a group's own box is where its
/// children's boxes are, not their drawing (a line the default 0×0 box holds).
fn drawn_rect(scene: &Scene, placed: &Placed, id: &str) -> Option<Rect> {
    match scene.node(id) {
        Some(n) if n.kind == NodeKind::Group => {
            let inside: Vec<Rect> = scene.children(id).iter().filter_map(|child| drawn_rect(scene, placed, child)).collect();
            Rect::bounds(inside.iter()).or_else(|| shapes::shown_rect(scene, placed, id))
        }
        _ => shapes::shown_rect(scene, placed, id),
    }
}

/// What a layer's parent puts it in: the page point its `x`/`y` count from, how the parent is
/// drawn against how it is stored (`layout::fit_of`, which its pins follow), and whether the
/// parent is a frame mirrored about its own box, whose children's pins mirror with them. A
/// selected layer keeps its pins: the selection turns over about its own middle, not its
/// parent's.
#[derive(Debug, Clone, Copy)]
struct Within {
    origin: (f64, f64),
    fit: Option<Fit>,
    frame: bool,
}

/// One flip: what it reads, and the edits so far.
struct Flip<'a> {
    scene: &'a Scene,
    placed: &'a Placed,
    measure: &'a dyn Measure,
    horizontal: bool,
    /// What turns over: the selection and everything inside it.
    flipping: std::collections::HashSet<String>,
    ops: Vec<NodeOp>,
}

impl Flip<'_> {
    /// The middle of `r` along the flip: across for a horizontal one.
    fn middle(&self, r: &Rect) -> f64 {
        if self.horizontal {
            r.cx()
        } else {
            r.cy()
        }
    }

    /// `p` mirrored about `c` along the flip.
    fn across(&self, p: (f64, f64), c: f64) -> (f64, f64) {
        if self.horizontal {
            (2.0 * c - p.0, p.1)
        } else {
            (p.0, 2.0 * c - p.1)
        }
    }

    /// Where `id`'s children count from on the page: as placed, or, not placed (hidden, or
    /// inside what is), its stored offsets from the nearest ancestor that is.
    fn origin(&self, id: &str) -> (f64, f64) {
        let scene = self.scene;
        let (mut x, mut y, mut at) = (0.0, 0.0, Some(id));
        for _ in 0..=scene.len() {
            let Some(a) = at else {
                break;
            };
            if self.placed.rect(a).is_some() {
                let (ox, oy) = self.placed.origin(a);
                return (ox + x, oy + y);
            }
            let Some(n) = scene.node(a) else {
                break;
            };
            (x, y) = (x + n.x, y + n.y);
            at = n.parent.as_deref();
        }
        (x, y)
    }

    /// The box a layer layout does not place (hidden, or inside what is) would take shown (B22):
    /// its own size (`layout::natural_size`: a hugging text's measured words, a group's
    /// children's bounds), at its `x`/`y` from `within`'s origin, through its pins.
    fn unplaced(&self, n: &Node, within: Within) -> Rect {
        let (w, h) = natural_size(self.scene, &n.id, self.measure);
        let (pin_x, pin_y) = n.pins();
        let (x, w) = within.fit.map_or((n.x, w), |f| follow_axis(pin_x, n.x, w, f.stored.0, f.placed.0));
        let (y, h) = within.fit.map_or((n.y, h), |f| follow_axis(pin_y, n.y, h, f.stored.1, f.placed.1));
        Rect::new(within.origin.0 + x, within.origin.1 + y, w, h)
    }

    /// Mirror `id` about the middle of `about` (moving it when `movable`; in place when its
    /// parent lays it out) and turn its drawing over, then what is inside it about its own box.
    fn mirror(&mut self, id: &str, about: &Rect, movable: bool, within: Within) {
        let scene = self.scene;
        let Some(n) = scene.stored(id) else {
            return;
        };
        let shown = self.placed.rect(id);
        let r = shown.unwrap_or_else(|| self.unplaced(n, within));
        // A group's own point, which its children count from; any other layer's box corner.
        let point = shown.map_or((r.x, r.y), |_| self.placed.point(id));
        let mut set = own(&laid(scene, n), self.horizontal);
        unstyled(scene, n, &mut set);
        let connector = n.kind == NodeKind::Line && (n.from.is_some() || n.to.is_some());
        let mut pins = n.pins();
        if within.frame && movable && !connector {
            if let Some((key, value, pin)) = mirrored_pin(n, self.horizontal) {
                set.insert(key.into(), value);
                *(if self.horizontal { &mut pins.0 } else { &mut pins.1 }) = pin;
            }
        }
        // A line attached as drawn: an end whose node is hidden draws from its own point.
        let ends = match (n.kind, shown) {
            (NodeKind::Line, Some(_)) => shapes::attached_ends(scene, self.placed, n),
            (NodeKind::Line, None) => (n.from.is_some(), n.to.is_some()),
            _ => (false, false),
        };
        let points = n.points.as_deref().filter(|p| !p.is_empty() && matches!(n.kind, NodeKind::Line | NodeKind::Path));
        match (ends, points) {
            ((false, false), Some(points)) => self.free_line(n, points, &r, about, movable, within, pins, &mut set),
            ((true, false) | (false, true), _) if shown.is_some() => self.half_attached(n, ends, &r, about, movable, within, &mut set),
            // Hidden and attached at one end: its stored points turn over in its box, so its free
            // end is mirrored when it shows again; the attached end follows its node anyway.
            ((true, false) | (false, true), Some(points)) => self.free_line(n, points, &r, about, movable, within, pins, &mut set),
            // A box, a group, a connector drawn from both its ends (its own points untouched).
            _ if movable => self.shift(n, &r, point, about, within, pins, &mut set),
            _ => {}
        }
        if !set.is_empty() {
            self.ops.push(NodeOp::Update { id: id.to_string(), set });
        }
        if n.kind.holds_children() && n.kind != NodeKind::Page {
            let origin = shown.map_or((r.x, r.y), |_| self.placed.origin(id));
            // A frame's children count from its corner (its content's, scrolled); a group's are
            // mirrored about their own bounds, which its point moves with.
            let inner = if n.kind == NodeKind::Frame { Rect::new(origin.0, origin.1, r.w, r.h) } else { r };
            let within = Within { origin, fit: fit_of(scene, self.placed, id), frame: n.kind == NodeKind::Frame };
            for child in scene.children(id) {
                self.mirror(child, &inner, !is_laid(scene, child), within);
            }
        }
    }

    /// Move a layer to where the mirror puts its box `r` (its own point `point` with it), writing
    /// the `x` or `y` that shows it there through the pins it ends with (`pins`) in a parent
    /// drawn as `within.fit`.
    #[allow(clippy::too_many_arguments)]
    fn shift(&self, n: &Node, r: &Rect, point: (f64, f64), about: &Rect, within: Within, pins: (Pin, Pin), set: &mut Map<String, Value>) {
        let (lo, len) = if self.horizontal { (r.x, r.w) } else { (r.y, r.h) };
        let d = 2.0 * self.middle(about) - 2.0 * lo - len;
        let (key, at, pin, old, fit) = if self.horizontal {
            ("x", point.0 + d - within.origin.0, pins.0, n.x, within.fit.map(|f| (f.placed.0, f.stored.0)))
        } else {
            ("y", point.1 + d - within.origin.1, pins.1, n.y, within.fit.map(|f| (f.placed.1, f.stored.1)))
        };
        let v = fit.map_or(at, |(placed, stored)| follow_axis(pin, at, len, placed, stored).0);
        if (v - old).abs() >= 0.5 {
            set.insert(key.into(), px(v.clamp(-MAX_COORD, MAX_COORD)));
        }
    }

    /// A free line's or path's points turned over with it (B21): its box mirrored about `about`
    /// and each point within the box, so each drawn point lands where the mirror puts it whatever
    /// size the box is; in place about their own middle when its parent lays it out. A line's
    /// turn (one an older Relay stored: a line turns by its points) goes into its points.
    #[allow(clippy::too_many_arguments)]
    fn free_line(
        &self,
        n: &Node,
        points: &[[f64; 2]],
        r: &Rect,
        about: &Rect,
        movable: bool,
        within: Within,
        pins: (Pin, Pin),
        set: &mut Map<String, Value>,
    ) {
        let turn = shapes::rotation(n);
        if n.kind == NodeKind::Line && turn != 0.0 {
            let drawn: Vec<(f64, f64)> = points.iter().map(|p| shapes::rotate(r.x + p[0], r.y + p[1], r.cx(), r.cy(), turn)).collect();
            set.insert("rotation".into(), Value::Null);
            self.redraw(n, &drawn, r, about, movable, within, pins, set);
            return;
        }
        let a = if self.horizontal { 0 } else { 1 };
        // Mirrored in its box, or (laid out) about the points' own middle; a turned path turns
        // about its box's centre, so it is mirrored in its box either way.
        let span = match movable || turn != 0.0 {
            true => [r.w, r.h][a],
            false => {
                let (lo, hi) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[a]), hi.max(p[a])));
                lo + hi
            }
        };
        let flipped: Vec<[f64; 2]> = points
            .iter()
            .map(|p| {
                let mut q = *p;
                q[a] = (span - p[a]).clamp(-MAX_COORD, MAX_COORD);
                q
            })
            .collect();
        if flipped != points {
            set.insert("points".into(), json!(flipped));
        }
        if movable {
            self.shift(n, r, (r.x, r.y), about, within, pins, set);
        }
    }

    /// A connector attached at one end (TM-61): its free end mirrored about `about` with the
    /// rest, while its attached end follows what it is attached to, which draws it from there
    /// whatever its points say. Its points are written as it is drawn, mirrored, so an end let go
    /// later lets go where it showed. When what it is attached to stays put (not in the flip),
    /// it turns over about that node's middle instead, so it keeps its length and leaves the
    /// node from its other side.
    #[allow(clippy::too_many_arguments)]
    fn half_attached(&self, n: &Node, ends: (bool, bool), r: &Rect, about: &Rect, movable: bool, within: Within, set: &mut Map<String, Value>) {
        let attached = if ends.0 { n.from.as_deref() } else { n.to.as_deref() };
        let staying = attached.filter(|end| !self.flipping.contains(*end)).and_then(|end| shapes::shown_rect(self.scene, self.placed, end));
        let about = staying.as_ref().unwrap_or(about);
        let (pts, _) = shapes::line_points(self.scene, self.placed, n);
        let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else {
            return;
        };
        let turn = shapes::rotation(n);
        let ends: Vec<(f64, f64)> = [a, b].iter().map(|p| shapes::rotate(p.0, p.1, r.cx(), r.cy(), turn)).collect();
        if turn != 0.0 {
            set.insert("rotation".into(), Value::Null);
        }
        // A connector takes no pins (`Node::validate`).
        self.redraw(n, &ends, r, about, movable, within, (Pin::Start, Pin::Start), set);
    }

    /// A line drawn through page points `drawn`, its turn taken into them, mirrored about
    /// `about` (in place about their own middle when its parent lays it out), written as the
    /// canvas lets a connector go: its box at their least corner and as big as they spread, each
    /// point from there, in whole pixels; laid out, only its points, from its placed box `r`.
    #[allow(clippy::too_many_arguments)]
    fn redraw(
        &self,
        n: &Node,
        drawn: &[(f64, f64)],
        r: &Rect,
        about: &Rect,
        movable: bool,
        within: Within,
        pins: (Pin, Pin),
        set: &mut Map<String, Value>,
    ) {
        let along = |p: &(f64, f64)| if self.horizontal { p.0 } else { p.1 };
        let c = match movable {
            true => self.middle(about),
            false => (drawn.iter().map(along).fold(f64::INFINITY, f64::min) + drawn.iter().map(along).fold(f64::NEG_INFINITY, f64::max)) / 2.0,
        };
        let at = if movable { within.origin } else { (r.x, r.y) };
        let rel: Vec<(f64, f64)> = drawn.iter().map(|p| self.across(*p, c)).map(|p| ((p.0 - at.0).round(), (p.1 - at.1).round())).collect();
        if !movable {
            let points: Vec<[f64; 2]> = rel.iter().map(|p| [p.0.clamp(-MAX_COORD, MAX_COORD), p.1.clamp(-MAX_COORD, MAX_COORD)]).collect();
            put(set, "points", n.points.as_ref().map(|p| json!(p)), json!(points));
            return;
        }
        let least = |f: fn(&(f64, f64)) -> f64| rel.iter().map(f).fold(f64::INFINITY, f64::min);
        let (lx, ly) = (least(|p| p.0), least(|p| p.1));
        let points: Vec<[f64; 2]> = rel.iter().map(|p| [(p.0 - lx).min(MAX_COORD), (p.1 - ly).min(MAX_COORD)]).collect();
        let spread = |a: usize| points.iter().map(|p| p[a]).fold(0.0, f64::max);
        let (w, h) = (spread(0), spread(1));
        // Where it shows from its parent's corner, stored through its pins (left, right or
        // centre: a line's points do not stretch).
        let (x, y) = match within.fit {
            Some(f) => (follow_axis(pins.0, lx, w, f.placed.0, f.stored.0).0, follow_axis(pins.1, ly, h, f.placed.1, f.stored.1).0),
            None => (lx, ly),
        };
        for (key, old, v) in [("x", n.x, x), ("y", n.y, y), ("w", n.w, w), ("h", n.h, h)] {
            put(set, key, Some(json!(old)), json!(v.round().clamp(-MAX_COORD, MAX_COORD)));
        }
        put(set, "points", n.points.as_ref().map(|p| json!(p)), json!(points));
    }
}

/// Write `key` as `value` where it differs from what is there (`old`).
fn put(set: &mut Map<String, Value>, key: &str, old: Option<Value>, value: Value) {
    if old.as_ref() != Some(&value) {
        set.insert(key.into(), value);
    }
}

/// `n` as stored with its style laid under it, its colour names kept: what a flip turns over,
/// so a gradient or corner its style gives it is written on it turned over (and still names its
/// colours).
fn laid<'a>(scene: &Scene, n: &'a Node) -> Cow<'a, Node> {
    match n.style.as_deref().and_then(|name| scene.styles().styles.get(name)) {
        Some(style) => {
            let mut looks = n.clone();
            style.lay_under(&mut looks);
            Cow::Owned(looks)
        }
        None => Cow::Borrowed(n),
    }
}

/// What `set` (a flip's own fields, from `n` with its style laid under) writes back to `n`'s
/// style: a field `n` does not set itself but takes from its style, flipped twice, is the style's
/// value again, and is written `null` so the layer takes it from its style once more.
fn unstyled(scene: &Scene, n: &Node, set: &mut Map<String, Value>) {
    let Some(style) = n.style.as_deref().and_then(|name| scene.styles().styles.get(name)) else {
        return;
    };
    let given = style.to_set();
    for (key, value) in set.iter_mut() {
        if given.get(key).is_some_and(|from| from == value) {
            *value = Value::Null;
        }
    }
}

/// `n`'s pin along the flip mirrored, for a child of a frame mirrored about the frame's box: a
/// left pin (the default) becomes right and right left (written as none, the default), top and
/// bottom likewise; the rest pin both sides, the middle or a share, which mirror to themselves.
/// The key, the value to write and the pin it ends with.
fn mirrored_pin(n: &Node, horizontal: bool) -> Option<(&'static str, Value, Pin)> {
    let pin = if horizontal { n.pin_x.map_or(Pin::Start, PinX::axis) } else { n.pin_y.map_or(Pin::Start, PinY::axis) };
    let key = if horizontal { "pin_x" } else { "pin_y" };
    match pin {
        Pin::Start => Some((key, json!(if horizontal { "right" } else { "bottom" }), Pin::End)),
        Pin::End => Some((key, Value::Null, Pin::Start)),
        _ => None,
    }
}

/// The fields that turn a layer's own drawing over where it is, read from `n` as stored with
/// its style laid under (`laid`): its turn (not a line's, which turns by its points), the
/// `flip_x`/`flip_y` of a polygon, picture or icon, its corner radii, its gradients, its arc and
/// its motion.
fn own(n: &Node, horizontal: bool) -> Map<String, Value> {
    let mut set = Map::new();
    if let Some(turn) = n.rotation.filter(|t| *t != 0.0 && n.kind != NodeKind::Line) {
        set.insert("rotation".into(), json!(-turn));
    }
    if matches!(n.kind, NodeKind::Polygon | NodeKind::Image | NodeKind::Icon) {
        let key = if horizontal { "flip_x" } else { "flip_y" };
        let on = if horizontal { n.flip_x } else { n.flip_y } == Some(true);
        set.insert(key.into(), if on { Value::Null } else { json!(true) });
    }
    if let Some([tl, tr, br, bl]) = n.radii {
        let radii = if horizontal { [tr, tl, bl, br] } else { [bl, br, tr, tl] };
        if radii != [tl, tr, br, bl] {
            set.insert("radii".into(), json!(radii));
        }
    }
    for (key, g) in [("gradient", &n.gradient), ("stroke_gradient", &n.stroke_gradient)] {
        if let Some(g) = g.as_ref().and_then(|g| flipped_gradient(g, horizontal)) {
            set.insert(key.into(), json!(g));
        }
    }
    let sweeps = n.kind == NodeKind::Ellipse && n.motion.as_ref().is_some_and(|m| m.keys.iter().any(|k| k.sweep.is_some()));
    if n.kind == NodeKind::Ellipse {
        if let Some(arc) = flipped_arc(n.arc.as_ref(), horizontal, sweeps) {
            set.insert("arc".into(), arc);
        }
    }
    if let Some(m) = n.motion.as_ref().map(|m| flipped_motion(m, horizontal, sweeps)).filter(|m| Some(m) != n.motion.as_ref()) {
        set.insert("motion".into(), json!(m));
    }
    set
}

/// `v` without the last bits of noise a mirror leaves (`1 − 0.7` is not quite `0.3`), so a
/// flip twice gives back what was written.
fn tidy(v: f64) -> f64 {
    (v * 1e9).round() / 1e9 + 0.0
}

/// An angle in degrees as `0 ≤ a < 360`.
fn degrees(a: f64) -> f64 {
    let a = tidy(a.rem_euclid(360.0));
    if a >= 360.0 {
        0.0
    } else {
        a
    }
}

/// `-v`, never `-0`.
fn neg(v: f64) -> f64 {
    0.0 - v
}

/// A direction (degrees clockwise from the top) mirrored: across a horizontal flip, top to
/// bottom otherwise.
fn mirrored_angle(a: f64, horizontal: bool) -> f64 {
    degrees(if horizontal { -a } else { 180.0 - a })
}

/// A gradient turned over (drawing §5.4): a linear one's direction mirrored; a conic one's
/// start mirrored and its stops run the other way, since a clockwise sweep mirrored runs
/// anticlockwise; a radial one is its own mirror. `None` when that changes nothing.
fn flipped_gradient(g: &Gradient, horizontal: bool) -> Option<Gradient> {
    let default = match g.kind {
        GradientKind::Linear => 180.0,
        GradientKind::Conic => 0.0,
        GradientKind::Radial => return None,
    };
    let mut next = g.clone();
    let angle = mirrored_angle(g.angle.unwrap_or(default), horizontal);
    next.angle = if g.angle.is_none() && angle == default { None } else { Some(angle) };
    if g.kind == GradientKind::Conic {
        next.stops = g
            .stops
            .iter()
            .rev()
            .map(|s| {
                let mut s = s.clone();
                s.at = tidy(1.0 - s.at);
                s
            })
            .collect();
    }
    let same = degrees(next.angle.unwrap_or(default)) == degrees(g.angle.unwrap_or(default)) && next.stops == g.stops;
    (!same).then_some(next)
}

/// An ellipse's arc turned over: the same ring segment mirrored, or `None` when that changes
/// nothing. One that fills as it plays (`sweeps`: a `sweep` key, which sets how far it goes from
/// where it starts) is mirrored at its start and runs the other way round, so it fills from the
/// mirrored start as the original fills from its own; a whole ellipse that fills so gets an arc
/// only where its start moves (top to bottom). `null` for an arc that comes back whole and plain.
fn flipped_arc(arc: Option<&Arc>, horizontal: bool, sweeps: bool) -> Option<Value> {
    let next = match (arc, sweeps) {
        (_, true) => {
            let a = arc.cloned().unwrap_or(Arc { start: 0.0, sweep: 360.0, inner: 0.0, cap: None });
            Arc { start: mirrored_angle(a.start, horizontal), sweep: neg(a.sweep), ..a }
        }
        (Some(a), false) => Arc { start: mirrored_angle(a.start + a.sweep, horizontal), ..a.clone() },
        (None, false) => return None,
    };
    if arc == Some(&next) {
        return None;
    }
    if next.start == 0.0 && next.sweep.abs() >= 360.0 && next.inner == 0.0 && next.cap.is_none() {
        return arc.map(|_| Value::Null);
    }
    Some(json!(next))
}

/// How a layer moves, turned over as its drawing is (motion §2): its moves along the flip and
/// its turns the other way, its origin mirrored in its box, a gradient angle as the gradient's
/// (top to bottom from whichever of 180 and −180 keeps every key within ±`MAX_TURN`, so the
/// turn between keys is kept), and the sweeps of an arc that fills as it plays (`sweeps`) the
/// other way round, as `flipped_arc` runs the arc.
fn flipped_motion(m: &Motion, horizontal: bool, sweeps: bool) -> Motion {
    let mut m = m.clone();
    let base = match horizontal {
        true => 0.0,
        false if m.keys.iter().filter_map(|k| k.angle).all(|a| 180.0 - a <= MAX_TURN) => 180.0,
        false => -180.0,
    };
    for k in &mut m.keys {
        let along = if horizontal { &mut k.dx } else { &mut k.dy };
        *along = along.map(neg);
        k.rotate = k.rotate.map(neg);
        k.angle = k.angle.map(|a| tidy((base - a).clamp(-MAX_TURN, MAX_TURN)));
        if sweeps {
            k.sweep = k.sweep.map(neg);
        }
    }
    m.origin = m.origin.map(|[x, y]| {
        let mirror = |v: f64| tidy((1.0 - v).clamp(-MAX_ORIGIN, MAX_ORIGIN));
        if horizontal {
            [mirror(x), y]
        } else {
            [x, mirror(y)]
        }
    });
    m
}

/// The style fields a text's letters take.
const TEXT_STYLE: &[&str] = &[
    "color", "font", "font_size", "font_weight", "italic", "underline", "strike", "align", "line_height",
    "letter_spacing", "text_case",
];
/// The style fields a box, shape or stroke paints with.
const PAINT_STYLE: &[&str] = &[
    "fill", "gradient", "stroke", "stroke_width", "dash", "radius", "radii", "shadow", "shadows", "stroke_cap", "stroke_join",
    "stroke_gradient",
];
/// The style fields every layer has, and the name of the style it takes.
const LAYER_STYLE: &[&str] = &["opacity", "blur", "blend", "style"];

/// Layer `id`'s style as `paste_style` lays it on others, read as stored: every style field and
/// the name of its style, colour names kept, `null` where it sets none of its own (so its style,
/// or its kind's default, shows). Its kind travels with it, under `kind`. `None` for no such
/// layer.
pub fn style_of(scene: &Scene, id: &str) -> Option<Map<String, Value>> {
    let all = serde_json::to_value(scene.stored(id)?).ok()?;
    let keys = TEXT_STYLE.iter().chain(PAINT_STYLE).chain(LAYER_STYLE).chain(&["kind"]);
    Some(keys.map(|k| (k.to_string(), all.get(*k).cloned().unwrap_or(Value::Null))).collect())
}

fn holds_text(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon)
}

/// The edits that give `ids` a copied `style`: its letters to whatever holds text, its paint
/// to shapes from a shape and to texts from a text, its opacity, blur, blend and style name to
/// all; each only where the node's kind uses the field (`takes`), so a mixed selection takes the
/// paste, and held to what the kind draws (a text, line, path or icon casts outer shadows only).
/// Compared with each node as stored, so what it already has is not sent.
pub fn paste_style(scene: &Scene, ids: &[String], style: &Map<String, Value>) -> Vec<NodeOp> {
    let from: Option<NodeKind> = style.get("kind").cloned().and_then(|k| serde_json::from_value(k).ok());
    let from_text = from == Some(NodeKind::Text);
    let kind_of = |id: &str| scene.node(id).map(|n| n.kind);
    ids.iter()
        .filter_map(|id| {
            let n = scene.stored(id)?;
            let current = serde_json::to_value(n).ok()?;
            let mut keys: Vec<&str> = LAYER_STYLE.to_vec();
            if holds_text(n.kind) && from.is_some_and(holds_text) {
                keys.extend(TEXT_STYLE);
            }
            let paints = !matches!(n.kind, NodeKind::Page | NodeKind::Group);
            if paints && (n.kind == NodeKind::Text) == from_text {
                keys.extend(PAINT_STYLE);
            }
            let set: Map<String, Value> =
                keys.into_iter().filter(|k| takes(n.kind, k)).map(|k| (k.to_string(), style.get(k).cloned().unwrap_or(Value::Null))).collect();
            let mut op = NodeOp::Update { id: id.clone(), set };
            let dropped = fit_written(&mut op, kind_of);
            let NodeOp::Update { mut set, .. } = op else {
                return None;
            };
            // A shadow it cannot cast at all (an inner one on a path) leaves it none.
            for key in dropped.into_iter().filter(|k| matches!(k.as_str(), "shadow" | "shadows")) {
                set.insert(key, Value::Null);
            }
            set.retain(|k, want| current.get(k).unwrap_or(&Value::Null) != want);
            (!set.is_empty()).then(|| NodeOp::Update { id: id.clone(), set })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::{node, styled};
    use relay_bus::design::{Layout, NodeKind as K, Shadow, Sizing, StrokeCap, Styles, STYLE_FIELDS};
    use std::collections::BTreeSet;

    fn page() -> Node {
        node("p1", None, K::Page, [0.0; 4])
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Flip `list` and run the edits as the canvas does (`apply_all`: strictly).
    fn flipped(s: &mut Scene, list: &[&str], horizontal: bool) -> Vec<NodeOp> {
        let p = place(s, "p1", &Mono);
        let ops = flip(s, &p, &Mono, &ids(list), horizontal);
        s.apply_all(&ops).unwrap_or_else(|e| panic!("{e}: {ops:?}"));
        ops
    }

    /// Where a line's points are drawn on the page, turned, across (or down), least first.
    fn drawn(s: &Scene, id: &str, across: bool) -> Vec<f64> {
        let p = place(s, "p1", &Mono);
        let (n, r) = (s.node(id).unwrap(), p.rect(id).unwrap());
        let turned = shapes::line_points(s, &p, n).0.into_iter().map(|q| shapes::rotate(q.0, q.1, r.cx(), r.cy(), shapes::rotation(n)));
        let mut v: Vec<f64> = turned.map(|q| ((if across { q.0 } else { q.1 }) * 1e6).round() / 1e6).collect();
        v.sort_by(f64::total_cmp);
        v
    }

    fn line(id: &str, parent: &str, rect: [f64; 4], points: &[[f64; 2]]) -> Node {
        let mut n = node(id, Some(parent), K::Line, rect);
        n.points = Some(points.to_vec());
        n
    }

    #[test]
    fn a_flip_mirrors_the_selection_about_its_centre() {
        let mut s = Scene::new(vec![
            page(),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 50.0]),
            node("b", Some("p1"), K::Rect, [200.0, 0.0, 50.0, 50.0]),
        ]);
        s.node_mut("a").unwrap().radii = Some([10.0, 0.0, 0.0, 0.0]);
        flipped(&mut s, &["a", "b"], true);
        assert_eq!((s.node("a").unwrap().x, s.node("b").unwrap().x), (150.0, 0.0));
        assert_eq!(s.node("a").unwrap().radii, Some([0.0, 10.0, 0.0, 0.0]));
        // Flipping twice is where it started.
        flipped(&mut s, &["a", "b"], true);
        assert_eq!((s.node("a").unwrap().x, s.node("b").unwrap().x), (0.0, 200.0));
    }

    #[test]
    fn a_flip_turns_points_polygons_and_frames_over() {
        let mut s = Scene::new(vec![
            page(),
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 100.0]),
            node("in", Some("f"), K::Rect, [10.0, 10.0, 40.0, 40.0]),
            node("tri", Some("p1"), K::Polygon, [300.0, 0.0, 100.0, 100.0]),
            line("ln", "p1", [0.0, 200.0, 100.0, 50.0], &[[0.0, 0.0], [100.0, 50.0]]),
        ]);
        flipped(&mut s, &["f", "tri", "ln"], false);
        let inside = s.node("in").unwrap();
        assert_eq!((inside.y, inside.pin_y), (50.0, Some(PinY::Bottom)), "the child mirrors inside its frame, and its pin with it");
        assert_eq!(s.node("tri").unwrap().flip_y, Some(true));
        assert_eq!(s.node("ln").unwrap().points, Some(vec![[0.0, 50.0], [100.0, 0.0]]));
    }

    /// B21: a free line's points are drawn from its box's corner whatever size the box is, so
    /// the flip mirrors each drawn point, not the box, and the selection's middle is that of
    /// what it draws.
    #[test]
    fn a_flip_mirrors_a_line_whose_box_is_not_its_points() {
        let pts = [[0.0, 0.0], [100.0, 0.0]];
        let mut s = Scene::new(vec![
            page(),
            line("ln", "p1", [0.0, 200.0, 0.0, 0.0], &pts),
            node("b", Some("p1"), K::Rect, [200.0, 200.0, 50.0, 50.0]),
        ]);
        flipped(&mut s, &["ln", "b"], true);
        assert_eq!((drawn(&s, "ln", true), s.node("b").unwrap().x), (vec![150.0, 250.0], 0.0));
        // Alone it turns over where it is drawn, whatever its box: none, as an agent writes a
        // line, or one wider than its points.
        for rect in [[0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 200.0, 10.0]] {
            let mut s = Scene::new(vec![page(), line("ln", "p1", rect, &pts)]);
            flipped(&mut s, &["ln"], true);
            assert_eq!(drawn(&s, "ln", true), [0.0, 100.0], "{rect:?}");
            flipped(&mut s, &["ln"], true);
            assert_eq!(s.node("ln").unwrap().points.as_deref(), Some(&pts[..]), "{rect:?}");
        }
        // Inside a free frame that flips, about the frame; back again where it was.
        let mut s = Scene::new(vec![page(), node("f", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]), line("ln", "f", [10.0, 0.0, 0.0, 0.0], &pts)]);
        flipped(&mut s, &["f"], true);
        assert_eq!(drawn(&s, "ln", true), [190.0, 290.0]);
        flipped(&mut s, &["f"], true);
        assert_eq!(drawn(&s, "ln", true), [10.0, 110.0]);
    }

    /// B22: layout places no hidden layer nor anything inside one, and a flip still mirrors them,
    /// at the size they take shown, so each shows where it belongs once shown again.
    #[test]
    fn a_flip_moves_hidden_layers_too() {
        let hidden = |mut n: Node| {
            n.hidden = Some(true);
            n
        };
        let mut hr = node("hr", Some("hg"), K::Rect, [0.0, 0.0, 20.0, 20.0]);
        hr.radii = Some([5.0, 0.0, 0.0, 0.0]);
        let mut s = Scene::new(vec![
            page(),
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 100.0]),
            node("vis", Some("f"), K::Rect, [10.0, 10.0, 40.0, 40.0]),
            hidden(node("hid", Some("f"), K::Rect, [20.0, 0.0, 30.0, 30.0])),
            hidden(node("hg", Some("f"), K::Group, [100.0, 0.0, 0.0, 0.0])),
            hr,
            // "Hi", 16 px wide as it hugs its words.
            hidden(node("ht", Some("f"), K::Text, [10.0, 60.0, 0.0, 0.0])),
        ]);
        let x = |s: &Scene, id: &str| s.node(id).unwrap().x;
        flipped(&mut s, &["f"], true);
        assert_eq!((x(&s, "vis"), x(&s, "hid"), x(&s, "hg") + x(&s, "hr"), x(&s, "ht")), (150.0, 150.0, 80.0, 174.0));
        assert_eq!(s.node("hr").unwrap().radii, Some([0.0, 5.0, 0.0, 0.0]));
        flipped(&mut s, &["f"], true);
        assert_eq!((x(&s, "vis"), x(&s, "hid"), x(&s, "hg"), x(&s, "hr"), x(&s, "ht")), (10.0, 20.0, 100.0, 0.0, 10.0));
        assert_eq!(s.node("hr").unwrap().radii, Some([5.0, 0.0, 0.0, 0.0]));
        // A hidden frame keeps its place in a column, and what is inside it mirrors in it.
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 300.0]);
        col.layout = Some(Layout::Column);
        let mut s = Scene::new(vec![
            page(),
            col,
            hidden(node("hf", Some("col"), K::Frame, [0.0, 0.0, 100.0, 50.0])),
            node("k", Some("hf"), K::Rect, [10.0, 0.0, 20.0, 20.0]),
        ]);
        flipped(&mut s, &["col"], true);
        assert_eq!((x(&s, "hf"), x(&s, "k")), (0.0, 70.0));
        // A selected hidden layer is mirrored about what the selection shows; alone, there is
        // nothing shown to mirror it about.
        let mut s = Scene::new(vec![
            page(),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]),
            hidden(node("h", Some("p1"), K::Rect, [0.0, 0.0, 20.0, 20.0])),
        ]);
        flipped(&mut s, &["a", "h"], true);
        assert_eq!((x(&s, "a"), x(&s, "h")), (0.0, 80.0));
        let p = place(&s, "p1", &Mono);
        assert!(flip(&s, &p, &Mono, &ids(&["h"]), true).is_empty());
    }

    /// B16: a locked layer, which stays selected once locked, is not flipped, and the rest of the
    /// selection is; one inside a frame that flips turns over with it, as it moves with it.
    #[test]
    fn a_locked_layer_is_not_flipped() {
        let mut s = Scene::new(vec![
            page(),
            node("l", Some("p1"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            node("u", Some("p1"), K::Rect, [100.0, 0.0, 10.0, 10.0]),
        ]);
        for id in ["l", "u"] {
            s.node_mut(id).unwrap().radii = Some([4.0, 0.0, 0.0, 0.0]);
        }
        s.node_mut("l").unwrap().locked = Some(true);
        let p = place(&s, "p1", &Mono);
        assert!(flip(&s, &p, &Mono, &ids(&["l"]), true).is_empty());
        let ops = flip(&s, &p, &Mono, &ids(&["l", "u"]), true);
        assert!(matches!(&ops[..], [NodeOp::Update { id, set }] if id == "u" && !set.contains_key("x")), "u alone, in place: {ops:?}");
        let mut s = Scene::new(vec![page(), node("f", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 100.0]), node("in", Some("f"), K::Rect, [10.0, 10.0, 20.0, 20.0])]);
        s.node_mut("in").unwrap().locked = Some(true);
        flipped(&mut s, &["f"], true);
        assert_eq!(s.node("in").unwrap().x, 70.0);
    }

    /// TM-61: a connector attached at one end mirrors its free end with the rest, and its other
    /// end still starts at the node it is attached to; one attached at both ends follows them,
    /// its own points untouched.
    #[test]
    fn a_half_attached_connector_mirrors_its_free_end() {
        let mut ln = line("ln", "p1", [100.0, 50.0, 200.0, 0.0], &[[0.0, 0.0], [200.0, 0.0]]);
        ln.from = Some("a".into());
        let mut both = line("both", "p1", [0.0, 0.0, 0.0, 0.0], &[[0.0, 0.0], [1.0, 1.0]]);
        (both.from, both.to) = (Some("a".into()), Some("b".into()));
        let mut s = Scene::new(vec![
            page(),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]),
            node("b", Some("p1"), K::Rect, [400.0, 0.0, 100.0, 100.0]),
            ln,
            both,
        ]);
        // Drawn from a's right edge (100) to 300: mirrored about 150, a lands at 200..300 and
        // the free end at 0, and the line still leaves a from its edge, now its left.
        flipped(&mut s, &["a", "ln"], true);
        assert_eq!((s.node("a").unwrap().x, drawn(&s, "ln", true)), (200.0, vec![0.0, 200.0]));
        let p = place(&s, "p1", &Mono);
        assert_eq!(shapes::line_points(&s, &p, s.node("ln").unwrap()).0, [(200.0, 50.0), (0.0, 50.0)]);
        assert_eq!(s.node("ln").unwrap().from.as_deref(), Some("a"));
        // Back again, where it was.
        flipped(&mut s, &["a", "ln"], true);
        assert_eq!((s.node("a").unwrap().x, drawn(&s, "ln", true)), (0.0, vec![100.0, 300.0]));
        let ops = flipped(&mut s, &["a", "b", "both"], true);
        assert!(ops.iter().all(|op| !matches!(op, NodeOp::Update { id, set } if id == "both" && set.contains_key("points"))), "{ops:?}");
    }

    /// Review of B21: a group's middle is that of what its layers draw, so a group holding a line
    /// with the default 0×0 box (as agents write lines) turns over in place, and beside another
    /// layer about their joint drawing.
    #[test]
    fn a_group_flips_about_what_its_layers_draw() {
        let mut s = Scene::new(vec![
            page(),
            node("g", Some("p1"), K::Group, [0.0; 4]),
            line("ln", "g", [0.0; 4], &[[0.0, 0.0], [100.0, 0.0]]),
            node("r", Some("p1"), K::Rect, [300.0, 0.0, 100.0, 20.0]),
        ]);
        flipped(&mut s, &["g"], true);
        assert_eq!(drawn(&s, "ln", true), [0.0, 100.0], "in place");
        flipped(&mut s, &["g", "r"], true);
        assert_eq!(drawn(&s, "ln", true), [300.0, 400.0], "mirrored about 200, the middle of 0..400");
        assert_eq!(s.node("r").unwrap().x, 0.0);
    }

    /// Review of TM-61: a connector attached at one end, flipped without the node it leaves,
    /// turns over about that node, keeping its length and leaving it from the other side; hidden
    /// in a frame that flips, its stored points turn over in its box.
    #[test]
    fn a_half_attached_connector_flipped_alone_keeps_its_length() {
        let mut ln = line("ln", "p1", [100.0, 50.0, 200.0, 0.0], &[[0.0, 0.0], [200.0, 0.0]]);
        ln.from = Some("a".into());
        let mut s = Scene::new(vec![page(), node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]), ln]);
        flipped(&mut s, &["ln"], true);
        let p = place(&s, "p1", &Mono);
        let pts = shapes::line_points(&s, &p, s.node("ln").unwrap()).0;
        assert_eq!((pts[0].0, pts[1].0), (0.0, -200.0), "from a's left edge, out to the left: {pts:?}");
        assert_eq!(s.node("a").unwrap().x, 0.0, "a stays");

        let mut hid = line("hid", "f", [0.0, 0.0, 300.0, 0.0], &[[100.0, 50.0], [300.0, 50.0]]);
        (hid.from, hid.hidden) = (Some("b".into()), Some(true));
        let mut s = Scene::new(vec![
            page(),
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 100.0]),
            node("b", Some("f"), K::Rect, [0.0, 0.0, 100.0, 100.0]),
            hid,
        ]);
        flipped(&mut s, &["f"], true);
        assert_eq!(s.node("b").unwrap().x, 300.0);
        assert_eq!(s.stored("hid").unwrap().points, Some(vec![[200.0, 50.0], [0.0, 50.0]]), "its free end mirrored in its box");
    }

    /// Review of B21: a turned path in a frame that lays it out is mirrored in its box, about the
    /// centre it turns about, so it does not jump.
    #[test]
    fn a_turned_path_in_a_row_flips_in_place() {
        let mut row = node("row", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut path = node("pa", Some("row"), K::Path, [0.0, 0.0, 100.0, 100.0]);
        (path.points, path.rotation) = (Some(vec![[0.0, 0.0], [40.0, 0.0], [40.0, 40.0]]), Some(90.0));
        let mut s = Scene::new(vec![page(), row, path]);
        let ops = flipped(&mut s, &["pa"], false);
        // Mirrored in its 100 × 100 box about the centre it turns about...
        assert!(matches!(&ops[0], NodeOp::Update { set, .. } if set["points"] == json!([[0.0, 100.0], [40.0, 100.0], [40.0, 60.0]])), "{ops:?}");
        // ...then, as every strict write is (B3), its box fitted to its points.
        let pa = s.node("pa").unwrap();
        assert_eq!((pa.points.clone(), pa.rotation, pa.w, pa.h), (Some(vec![[0.0, 40.0], [40.0, 40.0], [40.0, 0.0]]), Some(-90.0), 40.0, 40.0));
    }

    /// A line turns by its points (B99), so one an older Relay stored turned has its turn taken
    /// into its points as they are drawn, mirrored, and its `rotation` cleared: the flip passes
    /// the check a line's turn now meets, and draws the mirror.
    #[test]
    fn a_turned_line_flips_through_its_points() {
        let mut ln = line("ln", "p1", [0.0, 0.0, 100.0, 0.0], &[[0.0, 0.0], [100.0, 0.0]]);
        ln.rotation = Some(90.0);
        let mut s = Scene::new(vec![page(), ln, node("b", Some("p1"), K::Rect, [200.0, -50.0, 20.0, 100.0])]);
        // Drawn down x = 50, from -50 to 50; with b, mirrored about 135.
        assert_eq!((drawn(&s, "ln", true), drawn(&s, "ln", false)), (vec![50.0, 50.0], vec![-50.0, 50.0]));
        flipped(&mut s, &["ln", "b"], true);
        let n = s.node("ln").unwrap();
        assert_eq!((n.rotation, n.x, n.y, n.w, n.h), (None, 220.0, -50.0, 0.0, 100.0));
        assert_eq!((drawn(&s, "ln", true), drawn(&s, "ln", false), s.node("b").unwrap().x), (vec![220.0, 220.0], vec![-50.0, 50.0], 50.0));
    }

    /// A frame flipped mirrors its children's pins with them: one pinned left lands at the right
    /// pinned right, and each shows mirrored however wide a layout draws the frame (here twice
    /// its stored width). A layer flipped on its own keeps its pin: it turns over about its own
    /// middle, not its frame's.
    #[test]
    fn flip_mirrors_pins() {
        let mut row = node("row", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut f = node("f", Some("row"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        f.w_sizing = Some(Sizing::Fill);
        let child = |id: &str, x: f64, w: f64, pin: Option<PinX>| {
            let mut n = node(id, Some("f"), K::Rect, [x, 10.0, w, 20.0]);
            n.pin_x = pin;
            n
        };
        let mut s = Scene::new(vec![
            page(),
            row,
            f,
            child("l", 10.0, 40.0, None),
            child("r", 150.0, 40.0, Some(PinX::Right)),
            child("c", 80.0, 40.0, Some(PinX::Center)),
            child("s", 20.0, 40.0, Some(PinX::Scale)),
            child("lr", 10.0, 170.0, Some(PinX::LeftRight)),
        ]);
        let kids = ["l", "r", "c", "s", "lr"];
        let shown = |s: &Scene| -> Vec<(f64, f64)> {
            let p = place(s, "p1", &Mono);
            assert_eq!(p.rect("f").unwrap().w, 400.0, "the row fills f");
            kids.iter().map(|id| p.rect(id).map(|r| (r.x, r.w)).unwrap()).collect()
        };
        let (before, stored) = (shown(&s), s.to_nodes());
        flipped(&mut s, &["f"], true);
        let mirrored: Vec<(f64, f64)> = before.iter().map(|(x, w)| (400.0 - x - w, *w)).collect();
        assert_eq!(shown(&s), mirrored);
        let pins: Vec<Option<PinX>> = kids.iter().map(|id| s.node(id).unwrap().pin_x).collect();
        assert_eq!(pins, [Some(PinX::Right), None, Some(PinX::Center), Some(PinX::Scale), Some(PinX::LeftRight)]);
        let xs: Vec<f64> = kids.iter().map(|id| s.node(id).unwrap().x).collect();
        assert_eq!(xs, [150.0, 10.0, 80.0, 140.0, 20.0]);
        flipped(&mut s, &["f"], true);
        assert_eq!(s.to_nodes(), stored, "twice is where it started");
        flipped(&mut s, &["r"], true);
        assert_eq!(s.to_nodes(), stored, "alone, a box turns over in place, its pin kept");
    }

    /// A frame that lays its children out keeps their order and turns each over where it is;
    /// an absolute child, which it does not place, mirrors in it like a free one, pin and all.
    #[test]
    fn a_laid_out_frame_mirrors_only_its_absolute_children() {
        let mut row = node("row", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut z = node("z", Some("row"), K::Rect, [10.0, 20.0, 40.0, 40.0]);
        z.absolute = Some(true);
        let mut s = Scene::new(vec![
            page(),
            row,
            node("a", Some("row"), K::Rect, [0.0, 0.0, 50.0, 50.0]),
            node("b", Some("row"), K::Rect, [0.0, 0.0, 50.0, 50.0]),
            z,
        ]);
        let start = s.to_nodes();
        let ops = flipped(&mut s, &["row"], true);
        assert!(ops.iter().all(|op| op.target() == "z"), "the laid children stay where the row puts them: {ops:?}");
        let z = s.node("z").unwrap();
        assert_eq!((z.x, z.y, z.pin_x), (250.0, 20.0, Some(PinX::Right)));
        flipped(&mut s, &["row"], true);
        assert_eq!(s.to_nodes(), start);
    }

    /// A selected child pinned to a share of a frame drawn wider than it is stored moves its
    /// stored `x` by that share of how far it shows moved (`layout::stored_shift`).
    #[test]
    fn a_scaled_child_moves_by_its_share() {
        let mut row = node("row", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut f = node("f", Some("row"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        f.w_sizing = Some(Sizing::Fill);
        let mut sc = node("sc", Some("f"), K::Rect, [20.0, 10.0, 40.0, 20.0]);
        sc.pin_x = Some(PinX::Scale);
        let mut s = Scene::new(vec![page(), row, f, sc, node("t", Some("f"), K::Rect, [150.0, 10.0, 20.0, 20.0])]);
        // Shown at 40..120 and 150..170: mirrored about 105.
        flipped(&mut s, &["sc", "t"], true);
        let p = place(&s, "p1", &Mono);
        let (sc, t) = (s.node("sc").unwrap(), s.node("t").unwrap());
        assert_eq!((sc.x, sc.pin_x, t.x, t.pin_x), (45.0, Some(PinX::Scale), 40.0, None));
        assert_eq!((p.rect("sc").unwrap().x, p.rect("t").unwrap().x), (90.0, 40.0));
    }

    /// Motion turns over as the drawing does (motion §2): moves and turns the other way, the
    /// origin mirrored in the box, a gradient's angle as the gradient's.
    #[test]
    fn a_flip_mirrors_motion() {
        let motion: Motion = serde_json::from_value(json!({"origin": [0.2, 1], "keys": [
            {"at_ms": 0, "dx": 10, "dy": 5, "rotate": 30, "angle": 90},
            {"at_ms": 500, "dx": -20, "rotate": -45, "angle": 3500, "scale": 2},
        ]}))
        .unwrap();
        let mut m = node("m", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]);
        m.motion = Some(motion.clone());
        let start = Scene::new(vec![page(), m]);
        type Moves = Vec<(Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>)>;
        let keys = |s: &Scene| -> (Moves, Option<[f64; 2]>) {
            let m = s.node("m").unwrap().motion.clone().unwrap();
            (m.keys.iter().map(|k| (k.dx, k.dy, k.rotate, k.angle, k.scale)).collect(), m.origin)
        };
        let mut s = start.clone();
        flipped(&mut s, &["m"], true);
        let across = vec![(Some(-10.0), Some(5.0), Some(-30.0), Some(-90.0), None), (Some(20.0), None, Some(45.0), Some(-3500.0), Some(2.0))];
        assert_eq!(keys(&s), (across, Some([0.8, 1.0])));
        flipped(&mut s, &["m"], true);
        assert_eq!(s.node("m").unwrap().motion.as_ref(), Some(&motion), "twice is where it started");
        // Top to bottom an angle mirrors about 180, or about -180 where 180 would pass the
        // widest angle a key takes (180 - 3500 is in reach, so 180 here).
        let mut s = start.clone();
        flipped(&mut s, &["m"], false);
        let down = vec![(Some(10.0), Some(-5.0), Some(-30.0), Some(90.0), None), (Some(-20.0), None, Some(45.0), Some(-3320.0), Some(2.0))];
        assert_eq!(keys(&s), (down, Some([0.2, 0.0])));
        let far: Motion = serde_json::from_value(json!({"keys": [{"at_ms": 0, "angle": -3500}, {"at_ms": 500, "angle": 0}]})).unwrap();
        let angles: Vec<Option<f64>> = flipped_motion(&far, false, false).keys.iter().map(|k| k.angle).collect();
        assert_eq!(angles, [Some(3320.0), Some(-180.0)]);
    }

    /// A ring that fills as it plays fills from its mirrored start the other way round, so every
    /// moment of it is the mirror of the original's; a whole ellipse filling so starts from the
    /// bottom once turned top to bottom, and is whole again when turned back.
    #[test]
    fn a_filling_ring_fills_mirrored() {
        let fill: Motion = serde_json::from_value(json!({"keys": [{"at_ms": 0, "sweep": 0}, {"at_ms": 1000, "sweep": 270}]})).unwrap();
        let mut ring = node("ring", Some("p1"), K::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        ring.arc = Some(Arc { start: 30.0, sweep: 270.0, inner: 0.6, cap: Some(StrokeCap::Round) });
        ring.motion = Some(fill.clone());
        let mut disc = node("disc", Some("p1"), K::Ellipse, [200.0, 0.0, 100.0, 100.0]);
        disc.motion = Some(fill);
        let start = Scene::new(vec![page(), ring, disc]);
        let posed = |s: &Scene, id: &str, t: f64| {
            let n = s.node(id).unwrap();
            let arc = n.motion.as_ref().unwrap().pose(t).dress(n).unwrap().arc.unwrap();
            (arc.start, arc.sweep)
        };
        for horizontal in [true, false] {
            let mut s = start.clone();
            flipped(&mut s, &["ring"], horizontal);
            flipped(&mut s, &["disc"], horizontal);
            for t in [0.0, 250.0, 600.0, 1000.0] {
                for id in ["ring", "disc"] {
                    let (s0, w0) = posed(&start, id, t);
                    assert_eq!(posed(&s, id, t), (mirrored_angle(s0, horizontal), neg(w0)), "{id} at {t} ms");
                }
            }
            assert_eq!(s.node("disc").unwrap().arc.is_some(), !horizontal, "a whole disc starts at the top either way across");
            flipped(&mut s, &["ring"], horizontal);
            flipped(&mut s, &["disc"], horizontal);
            assert_eq!(s.to_nodes(), start.to_nodes(), "twice is where it started");
        }
    }

    /// Drawing §5.4: a conic gradient's start mirrors and its stops run the other way, so each
    /// direction shows the colour its mirror showed; a linear stroke gradient's angle mirrors; a
    /// radial one is its own mirror; twice is where it started.
    #[test]
    fn a_flip_turns_conic_and_stroke_gradients_over() {
        let gradient = |v: Value| -> Gradient { serde_json::from_value(v).unwrap() };
        let mut e = node("e", Some("p1"), K::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        e.gradient = Some(gradient(json!({"kind": "conic", "angle": 30, "stops": [
            {"at": 0, "color": "#FF0000"}, {"at": 0.3, "color": "#0000FF"}, {"at": 1, "color": "#00FF00"}]})));
        e.stroke_gradient = Some(gradient(json!({"angle": 90, "stops": [{"at": 0, "color": "#000000"}, {"at": 1, "color": "#FFFFFF"}]})));
        let start = Scene::new(vec![page(), e]);
        let mut s = start.clone();
        flipped(&mut s, &["e"], true);
        let n = s.node("e").unwrap();
        let (g, sg) = (n.gradient.clone().unwrap(), n.stroke_gradient.clone().unwrap());
        assert_eq!((g.angle, sg.angle), (Some(330.0), Some(270.0)));
        let stops: Vec<(f64, &str)> = g.stops.iter().map(|s| (s.at, s.color.as_str())).collect();
        assert_eq!(stops, [(0.0, "#00FF00"), (0.7, "#0000FF"), (1.0, "#FF0000")]);
        // How far round its sweep a direction (degrees clockwise from the top) falls: each
        // direction of the flipped one is as far from the end as its mirror from the start.
        let along = |g: &Gradient, theta: f64| (theta - g.angle.unwrap_or(0.0)).rem_euclid(360.0) / 360.0;
        let first = start.node("e").unwrap().gradient.clone().unwrap();
        for theta in [10.0, 100.0, 200.0, 300.0] {
            assert!((along(&first, theta) - (1.0 - along(&g, -theta))).abs() < 1e-9, "{theta}");
        }
        flipped(&mut s, &["e"], true);
        assert_eq!(s.to_nodes(), start.to_nodes(), "twice is where it started");
        // Top to bottom; then a radial gradient, which nothing changes.
        let mut s = start.clone();
        flipped(&mut s, &["e"], false);
        let n = s.node("e").unwrap();
        assert_eq!((n.gradient.as_ref().unwrap().angle, n.stroke_gradient.as_ref().unwrap().angle), (Some(150.0), Some(90.0)));
        s.node_mut("e").unwrap().gradient = Some(gradient(json!({"kind": "radial", "stops": [{"at": 0, "color": "#000"}, {"at": 1, "color": "#FFF"}]})));
        s.node_mut("e").unwrap().stroke_gradient = None;
        let p = place(&s, "p1", &Mono);
        assert!(flip(&s, &p, &Mono, &ids(&["e"]), true).is_empty());
    }

    /// What a style gives a layer (a corner, a gradient naming a colour) is written on the layer
    /// turned over, its colour names kept and its style still named.
    #[test]
    fn a_flip_keeps_colour_names_in_what_it_turns_over() {
        let styles: Styles = serde_json::from_value(json!({"colors": {"accent": "#4C9DFF"}, "styles": {"card": {
            "radii": [8, 0, 0, 0], "gradient": {"angle": 90, "stops": [{"at": 0, "color": "$accent"}, {"at": 1, "color": "#000000"}]}}}}))
        .unwrap();
        let mut b = node("b", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 50.0]);
        b.style = Some("card".into());
        let mut s = Scene::new(vec![page(), b]).with_styles(styles);
        flipped(&mut s, &["b"], true);
        let b = s.stored("b").unwrap();
        let g = b.gradient.as_ref().unwrap();
        assert_eq!((b.style.as_deref(), b.radii, g.angle, g.stops[0].color.as_str()), (Some("card"), Some([0.0, 8.0, 0.0, 0.0]), Some(270.0), "$accent"));
        // Twice is where it started: what the style gives goes back to the style.
        flipped(&mut s, &["b"], true);
        let b = s.stored("b").unwrap();
        assert_eq!((b.radii, b.gradient.as_ref()), (None, None), "{b:?}");
    }

    /// Reuse §3.3: copy style takes the layer as stored, with the name of its style and its
    /// colour names, so a paste gives another layer the same style and colours, not their values
    /// baked in, and clears what the copied layer does not set itself.
    #[test]
    fn style_of_carries_the_style_and_colour_names() {
        let mut s = styled();
        let card = style_of(&s, "b").unwrap();
        assert_eq!((&card["style"], &card["fill"], &card["kind"]), (&json!("card"), &Value::Null, &json!("rect")));
        assert_eq!(style_of(&s, "d").unwrap()["fill"], json!("$accent"));
        assert!(style_of(&s, "gone").is_none());
        // d, filled $accent of its own, takes card: its own fill cleared, card's shows.
        let ops = paste_style(&s, &ids(&["d"]), &card);
        s.apply_all(&ops).unwrap();
        let d = s.stored("d").unwrap();
        assert_eq!((d.style.as_deref(), d.fill.as_deref(), s.node("d").unwrap().fill.as_deref()), (Some("card"), None, Some("#4C9DFF")));
        assert!(paste_style(&s, &ids(&["d"]), &card).is_empty(), "pasting it again changes nothing");
    }

    /// Copy style carries every style field (MODEL-SPEC §1 #21), each once.
    #[test]
    fn copy_style_carries_every_style_field() {
        let ours: Vec<&str> = TEXT_STYLE.iter().chain(PAINT_STYLE).chain(LAYER_STYLE).copied().filter(|f| *f != "style").collect();
        assert_eq!(ours.iter().copied().collect::<BTreeSet<_>>(), STYLE_FIELDS.iter().copied().collect::<BTreeSet<_>>());
        assert_eq!(ours.len(), STYLE_FIELDS.len());
    }

    /// Drawing §5.4: a rect's style pasted on an ellipse leaves out its corner, and carries its
    /// shadows, stroke ends and stroke gradient; on a path, which casts outer shadows only, its
    /// inner shadow is left out, and with none left it casts none.
    #[test]
    fn paste_style_carries_the_drawing_fields_a_kind_takes() {
        let shadow = |inset: bool| Shadow { x: 0.0, y: 4.0, blur: 8.0, spread: 0.0, color: "#00000040".into(), inset };
        let mut src = node("src", Some("p1"), K::Rect, [0.0; 4]);
        src.radius = Some(12.0);
        src.shadows = Some(vec![shadow(false), shadow(true)]);
        src.stroke_cap = Some(StrokeCap::Round);
        src.stroke_gradient = serde_json::from_value(json!({"stops": [{"at": 0, "color": "#000000"}, {"at": 1, "color": "#FFFFFF"}]})).unwrap();
        let mut inner = node("inner", Some("p1"), K::Rect, [0.0; 4]);
        inner.shadow = Some(shadow(true));
        let mut path = line("path", "p1", [0.0, 0.0, 10.0, 10.0], &[[0.0, 0.0], [10.0, 10.0]]);
        path.kind = K::Path;
        path.shadow = Some(shadow(false));
        let mut s = Scene::new(vec![page(), src, inner, node("e", Some("p1"), K::Ellipse, [0.0; 4]), path]);
        let ops = paste_style(&s, &ids(&["e", "path"]), &style_of(&s, "src").unwrap());
        s.apply_all(&ops).unwrap();
        let (e, path) = (s.node("e").unwrap(), s.node("path").unwrap());
        assert_eq!((e.radius, e.shadows.as_ref().map(Vec::len), e.stroke_cap, e.stroke_gradient.is_some()), (None, Some(2), Some(StrokeCap::Round), true));
        assert_eq!((path.shadow.as_ref(), path.shadows.as_deref()), (None, Some(&[shadow(false)][..])));
        let ops = paste_style(&s, &ids(&["path"]), &style_of(&s, "inner").unwrap());
        s.apply_all(&ops).unwrap();
        let path = s.node("path").unwrap();
        assert_eq!((path.shadow.as_ref(), path.shadows.as_ref()), (None, None), "an inner shadow alone leaves a path none");
    }

    #[test]
    fn style_goes_where_it_fits() {
        let mut src = node("src", Some("p1"), K::Rect, [0.0; 4]);
        src.fill = Some("#FF0000".into());
        src.font_size = Some(30.0);
        src.opacity = Some(0.5);
        let mut s = Scene::new(vec![page(), node("r", Some("p1"), K::Rect, [0.0; 4]), node("t", Some("p1"), K::Text, [0.0; 4]), src]);
        let ops = paste_style(&s, &ids(&["r", "t"]), &style_of(&s, "src").unwrap());
        s.apply_all(&ops).unwrap();
        let (r, t) = (s.node("r").unwrap(), s.node("t").unwrap());
        assert_eq!((r.fill.as_deref(), r.font_size, r.opacity), (Some("#FF0000"), Some(30.0), Some(0.5)));
        assert_eq!((t.fill.as_deref(), t.font_size, t.opacity), (None, Some(30.0), Some(0.5)), "a text takes no box paint from a shape");
    }

    /// Each node takes only the fields its kind uses, so a mixed selection takes the paste.
    #[test]
    fn style_skips_what_a_kind_does_not_use() {
        let mut src = node("src", Some("p1"), K::Rect, [0.0; 4]);
        (src.radius, src.fill, src.opacity) = (Some(12.0), Some("#FF0000".into()), Some(0.5));
        let mut s = Scene::new(vec![
            page(),
            node("e", Some("p1"), K::Ellipse, [0.0; 4]),
            node("i", Some("p1"), K::Image, [0.0; 4]),
            node("g", Some("p1"), K::Group, [0.0; 4]),
            src,
        ]);
        s.node_mut("i").unwrap().src = Some("0123456789abcdef0123.png".into());
        let ops = paste_style(&s, &ids(&["e", "i", "g"]), &style_of(&s, "src").unwrap());
        s.apply_all(&ops).unwrap();
        let (e, i, g) = (s.node("e").unwrap(), s.node("i").unwrap(), s.node("g").unwrap());
        assert_eq!((e.radius, e.fill.as_deref(), e.opacity), (None, Some("#FF0000"), Some(0.5)));
        assert_eq!((i.radius, i.fill.as_deref(), i.opacity), (Some(12.0), None, Some(0.5)));
        assert_eq!((g.fill.as_deref(), g.opacity), (None, Some(0.5)));
    }
}
