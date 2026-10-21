//! Motion on the canvas (client-motion; feature-motion §2): the clock each node plays on, the poses
//! at a time, the bounds a pose covers, and the ready-made motions.
//!
//! A node's motion plays on the clock of the nearest node among itself and its ancestors that has a
//! `timeline`, else on its page's implicit clock. A clock lasts its `duration_ms`, else as long as
//! the latest last key among the motions on it, and loops unless its timeline says `loop: false`.
//! Every clock starts together when Play starts; nested timelines run on their own. Motion never
//! changes layout, so nothing here lays out: a pose moves a laid-out box after the fact.

use super::geom::Rect;
use super::layout::Placed;
use super::scene::Scene;
use relay_bus::design::{Ease, Key, Motion, Node, NodeKind, Pose};
use std::collections::{HashMap, HashSet};

/// One clock: whose `timeline` it is (or the page's, for its implicit clock), how long it runs
/// and whether it starts again at its end.
#[derive(Debug, Clone, PartialEq)]
pub struct Clock {
    pub owner: String,
    pub duration_ms: f64,
    pub looped: bool,
}

/// A page's movers and the clocks they play on, from one walk of the page; what a frame of Play
/// repaints; and the span an export covers.
#[derive(Debug, Clone, Default)]
pub struct Clocks {
    /// Only clocks something plays on.
    clocks: Vec<Clock>,
    /// Each mover and its clock, in paint order.
    movers: Vec<(String, usize)>,
    /// The movers, what [`Clocks::touch`] was given, and their ancestors.
    touched: HashSet<String>,
}

impl Clocks {
    /// The movers inside `page` (the page's subtree, hidden subtrees skipped) and their clocks, from
    /// one walk. The walk keeps its own stack, so a nesting deeper than any rule allows (a legacy
    /// design) cannot overflow.
    pub fn new(scene: &Scene, page: &str) -> Clocks {
        let mut out = Clocks::default();
        let Some(root) = scene.node(page) else {
            return out;
        };
        // Every timeline met, the page's (its own, or the implicit one) first: its owner, its
        // length if it names one, and whether it loops.
        let mut timelines: Vec<(String, Option<f64>, bool)> = Vec::new();
        let own = |n: &Node| (n.id.clone(), n.timeline.as_ref().and_then(|t| t.duration_ms), n.timeline.as_ref().and_then(|t| t.looped).unwrap_or(true));
        timelines.push(own(root));
        let mut movers: Vec<(String, usize)> = Vec::new();
        let mut stack: Vec<(String, usize)> = vec![(page.to_string(), 0)];
        while let Some((id, mut clock)) = stack.pop() {
            let Some(n) = scene.node(&id) else { continue };
            if id != page {
                if n.is_hidden() {
                    continue;
                }
                if n.timeline.is_some() {
                    timelines.push(own(n));
                    clock = timelines.len() - 1;
                }
            }
            if n.motion.is_some() && n.kind != NodeKind::Page {
                movers.push((id.clone(), clock));
            }
            stack.extend(scene.children(&id).iter().rev().map(|c| (c.clone(), clock)));
        }
        // Each clock's length: its own, else its longest motion. Clocks nothing plays on are left
        // out, so they lengthen no span.
        let mut longest = vec![None::<f64>; timelines.len()];
        for (id, clock) in &movers {
            let last = scene.node(id).and_then(|n| n.motion.as_ref()).map_or(0.0, Motion::last_ms);
            longest[*clock] = Some(longest[*clock].map_or(last, |l| l.max(last)));
        }
        let mut renumber = vec![usize::MAX; timelines.len()];
        for (at, (owner, duration_ms, looped)) in timelines.into_iter().enumerate() {
            if let Some(longest) = longest[at] {
                renumber[at] = out.clocks.len();
                out.clocks.push(Clock { owner, duration_ms: duration_ms.unwrap_or(longest), looped });
            }
        }
        for (id, _) in &movers {
            touch(&mut out.touched, scene, id);
        }
        out.movers = movers.into_iter().map(|(id, c)| (id, renumber[c])).collect();
        out
    }

    /// Whether nothing on the page moves by its keys.
    pub fn is_empty(&self) -> bool {
        self.movers.is_empty()
    }

    /// The clocks something plays on, the page's first when anything plays on it.
    pub fn clocks(&self) -> &[Clock] {
        &self.clocks
    }

    /// The clock of `owner`'s timeline (a page's implicit clock when it has none), if anything
    /// plays on it: the length a Timeline field shows when it names none.
    pub fn clock(&self, owner: &str) -> Option<&Clock> {
        self.clocks.iter().find(|c| c.owner == owner)
    }

    /// Each mover and the clock it plays on, in paint order.
    pub fn movers(&self) -> impl Iterator<Item = (&str, &Clock)> {
        self.movers.iter().map(|(id, c)| (id.as_str(), &self.clocks[*c]))
    }

    /// Mark `id` and its ancestors as repainted every frame: a picture that moves by itself (an
    /// animated SVG).
    pub fn touch(&mut self, scene: &Scene, id: &str) {
        touch(&mut self.touched, scene, id);
    }

    /// Whether `id` or something inside it moves: what a frame of Play repaints rather than
    /// reusing its picture at rest.
    pub fn touches(&self, id: &str) -> bool {
        self.touched.contains(id)
    }

    /// The longest clock, and whether every clock loops: what Play and an export of the page run
    /// over. `(0, false)` when nothing moves.
    pub fn span(&self) -> (f64, bool) {
        span_over(self.clocks.iter())
    }

    /// The same, over the clocks of the movers in `roots`' subtrees (an export of a selection).
    /// `(0, false)` when nothing there moves, else a span of at least 50 ms.
    pub fn span_of(&self, scene: &Scene, roots: &[String]) -> (f64, bool) {
        let roots: HashSet<&str> = roots.iter().map(String::as_str).collect();
        let mut used = vec![false; self.clocks.len()];
        for (id, clock) in &self.movers {
            if roots.contains(id.as_str()) || scene.ancestors(id).iter().any(|a| roots.contains(a)) {
                used[*clock] = true;
            }
        }
        span_over(self.clocks.iter().zip(used).filter(|(_, used)| *used).map(|(c, _)| c))
    }

    /// Each mover's pose at `t_ms` after Play started, written into `out` (cleared first, so a
    /// frame of Play reuses one map). A mover whose motion is gone since the clocks were made is
    /// left out.
    pub fn poses(&self, scene: &Scene, t_ms: f64, out: &mut HashMap<String, Pose>) {
        out.clear();
        for (id, clock) in &self.movers {
            let clock = &self.clocks[*clock];
            let local = local_time(t_ms, clock.duration_ms, clock.looped);
            if let Some(pose) = scene.node(id).and_then(|n| n.motion.as_ref().map(|m| m.pose(local))) {
                out.insert(id.clone(), pose);
            }
        }
    }
}

/// The scene and its placement as they stand `t_ms` after Play starts, for the checks a still
/// runs (`boxes::issues`): each mover's opacity, colours and sweep as posed and its turn added to
/// its own, and its box, with everything inside it, scaled about its origin and moved by its
/// offset, a mover inside another taking both. Blur, trim and a gradient's angle change nothing a
/// check reads. Text keeps its measure: a scaled text is checked at its own size.
pub fn posed_still(scene: &Scene, placed: &Placed, clocks: &Clocks, t_ms: f64) -> (Scene, Placed) {
    pose_still(scene, placed, clocks, t_ms, |_| true)
}

/// [`posed_still`] as motion's checks read it (`boxes::motion_issues`): a mover whose clock still
/// runs keeps its resting opacity, since a fade passes through faint on its way and a person reads
/// the words where it settles. A mover whose clock has stopped (it does not loop and `t_ms` is at
/// or past its end) holds its pose, its opacity with it, and so does every mover when `held`.
pub fn posed_settled(scene: &Scene, placed: &Placed, clocks: &Clocks, t_ms: f64, held: bool) -> (Scene, Placed) {
    pose_still(scene, placed, clocks, t_ms, |clock| held || (!clock.looped && t_ms >= clock.duration_ms))
}

/// [`posed_still`], each mover's posed opacity taken only where `fades` says of its clock.
fn pose_still(scene: &Scene, placed: &Placed, clocks: &Clocks, t_ms: f64, fades: impl Fn(&Clock) -> bool) -> (Scene, Placed) {
    let mut poses = HashMap::new();
    clocks.poses(scene, t_ms, &mut poses);
    let (mut posed, mut moved) = (scene.clone(), placed.clone());
    let hex = |[r, g, b, a]: [u8; 4]| format!("#{r:02X}{g:02X}{b:02X}{a:02X}");
    // Paint order: an outer mover's change is in place before an inner one's is laid on it.
    for (id, clock) in clocks.movers() {
        let Some(pose) = poses.get(id) else { continue };
        if let Some(n) = posed.node_mut(id) {
            if let Some(o) = pose.opacity.filter(|_| fades(clock)) {
                n.opacity = Some(o.clamp(0.0, 1.0));
            }
            for (field, value) in [(&mut n.fill, pose.fill), (&mut n.stroke, pose.stroke), (&mut n.color, pose.color)] {
                if let Some(value) = value {
                    *field = Some(hex(value));
                }
            }
            if let (Some(sweep), Some(arc)) = (pose.sweep, n.arc.as_mut()) {
                arc.sweep = sweep;
            }
            if pose.rotate != 0.0 {
                n.rotation = Some(n.rotation.unwrap_or(0.0) + pose.rotate);
            }
        }
        let Some(r) = moved.rect(id) else { continue };
        let (ox, oy) = (r.x + pose.origin[0] * r.w, r.y + pose.origin[1] * r.h);
        let at = |(x, y): (f64, f64)| (ox + (x - ox) * pose.scale_x + pose.dx, oy + (y - oy) * pose.scale_y + pose.dy);
        for inner in scene.subtree(id) {
            if let Some(b) = moved.rects.get_mut(&inner) {
                let ((x0, y0), (x1, y1)) = (at((b.x, b.y)), at((b.right(), b.bottom())));
                *b = Rect::spanning((x0.min(x1), y0.min(y1)), (x0.max(x1), y0.max(y1)));
            }
            for origins in [&mut moved.origins, &mut moved.content_origin] {
                if let Some(o) = origins.get_mut(&inner) {
                    *o = at(*o);
                }
            }
        }
    }
    (posed, moved)
}

/// Put `id` and its ancestors in `touched`.
fn touch(touched: &mut HashSet<String>, scene: &Scene, id: &str) {
    let mut at = Some(id);
    while let Some(id) = at {
        // Once a node is in, so are its ancestors.
        if !touched.insert(id.to_string()) {
            break;
        }
        at = scene.parent(id);
    }
}

fn span_over<'a>(clocks: impl Iterator<Item = &'a Clock>) -> (f64, bool) {
    let (mut longest, mut any, mut looped) = (0.0f64, false, true);
    for c in clocks {
        longest = longest.max(c.duration_ms);
        any = true;
        looped &= c.looped;
    }
    (longest, any && looped)
}

/// A clock's own time `t_ms` after Play started: past its end a looping clock starts again and
/// another holds its end; exactly at the end it shows the end, so a picture "at the end" is the end
/// and not the next loop's start. A clock of no length stays at 0.
pub fn local_time(t_ms: f64, duration_ms: f64, looped: bool) -> f64 {
    let t = if t_ms.is_finite() { t_ms.max(0.0) } else { 0.0 };
    if duration_ms.is_nan() || duration_ms <= 0.0 {
        0.0
    } else if looped && t > duration_ms {
        t % duration_ms
    } else {
        t.min(duration_ms)
    }
}

/// The box `r` covers once moved by `pose`: its corners scaled and turned about the pose's origin
/// (fractions of `r`), then offset, as the painter does it (CSS order: translate, then turn and
/// scale about the origin).
pub fn posed_bounds(r: &Rect, pose: &Pose) -> Rect {
    if !pose.moves() {
        return *r;
    }
    let (ox, oy) = (r.x + pose.origin[0] * r.w, r.y + pose.origin[1] * r.h);
    let (sin, cos) = pose.rotate.to_radians().sin_cos();
    let (mut lo, mut hi) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
    for (x, y) in [(r.x, r.y), (r.right(), r.y), (r.x, r.bottom()), (r.right(), r.bottom())] {
        let (px, py) = ((x - ox) * pose.scale_x, (y - oy) * pose.scale_y);
        let (qx, qy) = (px * cos - py * sin + ox + pose.dx, px * sin + py * cos + oy + pose.dy);
        lo = (lo.0.min(qx), lo.1.min(qy));
        hi = (hi.0.max(qx), hi.1.max(qy));
    }
    Rect::new(lo.0, lo.1, hi.0 - lo.0, hi.1 - lo.1)
}

/// The times of a contact sheet's `frames` cells (at least 2) over a span: evenly from the start to
/// the end, both included, in reading order.
pub fn sheet_times(span_ms: f64, frames: u8) -> Vec<f64> {
    let n = usize::from(frames.max(2));
    let span = if span_ms.is_finite() { span_ms.max(0.0) } else { 0.0 };
    (0..n).map(|i| span * i as f64 / (n - 1) as f64).collect()
}

/// The times of an animation's frames at `fps` (at least 1), `1000 / fps` ms apart: over the span
/// without its end when it loops (the next loop's start follows the last frame), else up to and
/// including the end, which the last frame holds. Its length is what [`super::apng::MAX_FRAMES`]
/// limits.
pub fn frame_times(span_ms: f64, looped: bool, fps: u16) -> Vec<f64> {
    let step = 1000.0 / f64::from(fps.max(1));
    let span = if span_ms.is_finite() { span_ms.max(0.0) } else { 0.0 };
    // A frame time within a millionth of a step of the end counts as on it.
    let steps = span / step;
    let n = if looped { (steps - 1e-6).ceil().max(1.0) as usize } else { (steps + 1e-6).floor() as usize + 1 };
    let mut times: Vec<f64> = (0..n).map(|i| (i as f64 * step).min(span)).collect();
    if !looped && times.last().is_some_and(|t| span - t > 1e-6 * step) {
        times.push(span);
    }
    times
}

/// Ready-made motions, for the inspector and as the skill's vocabulary. Each is relative to the
/// node's look at rest: it arrives at, or returns to, what the node draws when nothing plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Fades in from clear.
    FadeIn,
    /// Rises 24 px into place as it fades in.
    SlideUp,
    /// Slides 48 px in from the left as it fades in.
    SlideIn,
    /// Grows from 0.6 into place, a little past and back, as it fades in.
    Pop,
    /// Swells to 1.06 and back, again and again.
    Pulse,
    /// Turns once a second, again and again.
    Spin,
    /// Shows for half a second, hides for half, again and again.
    Blink,
    /// A line or path drawn from its start.
    DrawOn,
    /// An ellipse's ring fills round to its own sweep.
    FillRing,
}

impl Preset {
    pub const ALL: [Preset; 9] = [Preset::FadeIn, Preset::SlideUp, Preset::SlideIn, Preset::Pop, Preset::Pulse, Preset::Spin, Preset::Blink, Preset::DrawOn, Preset::FillRing];

    /// Its name in a menu.
    pub fn label(self) -> &'static str {
        match self {
            Preset::FadeIn => "Fade in",
            Preset::SlideUp => "Slide up",
            Preset::SlideIn => "Slide in",
            Preset::Pop => "Pop",
            Preset::Pulse => "Pulse",
            Preset::Spin => "Spin",
            Preset::Blink => "Blink",
            Preset::DrawOn => "Draw on",
            Preset::FillRing => "Fill ring",
        }
    }
}

/// The presets a `kind` node can take: none for a page, which does not move; drawing on only for a
/// line or path, and filling a ring only for an ellipse.
pub fn presets_for(kind: NodeKind) -> &'static [Preset] {
    use Preset::*;
    const EVERY: &[Preset] = &[FadeIn, SlideUp, SlideIn, Pop, Pulse, Spin, Blink];
    const LINES: &[Preset] = &[FadeIn, SlideUp, SlideIn, Pop, Pulse, Spin, Blink, DrawOn];
    const RINGS: &[Preset] = &[FadeIn, SlideUp, SlideIn, Pop, Pulse, Spin, Blink, FillRing];
    match kind {
        NodeKind::Page => &[],
        NodeKind::Line | NodeKind::Path => LINES,
        NodeKind::Ellipse => RINGS,
        _ => EVERY,
    }
}

/// Preset `p`'s motion for `n`, its values read from `n`'s rest look (its opacity, an ellipse's
/// sweep).
pub fn preset(p: Preset, n: &Node) -> Motion {
    let rest = n.opacity.filter(|o| o.is_finite()).map_or(1.0, |o| o.clamp(0.0, 1.0));
    let key = |at_ms: f64, set: &dyn Fn(&mut Key)| {
        let mut k = Key::at(at_ms);
        set(&mut k);
        k
    };
    let (keys, ease, looped) = match p {
        Preset::FadeIn => (vec![key(0.0, &|k| k.opacity = Some(0.0)), key(400.0, &|k| k.opacity = Some(rest))], Ease::EaseOut, false),
        Preset::SlideUp => (vec![
            key(0.0, &|k| (k.dy, k.opacity) = (Some(24.0), Some(0.0))),
            key(500.0, &|k| (k.dy, k.opacity) = (Some(0.0), Some(rest))),
        ], Ease::EaseOut, false),
        Preset::SlideIn => (vec![
            key(0.0, &|k| (k.dx, k.opacity) = (Some(-48.0), Some(0.0))),
            key(500.0, &|k| (k.dx, k.opacity) = (Some(0.0), Some(rest))),
        ], Ease::EaseOut, false),
        Preset::Pop => (vec![
            key(0.0, &|k| (k.scale, k.opacity) = (Some(0.6), Some(0.0))),
            key(350.0, &|k| (k.scale, k.opacity) = (Some(1.0), Some(rest))),
        ], Ease::Overshoot, false),
        Preset::Pulse => (vec![key(0.0, &|k| k.scale = Some(1.0)), key(600.0, &|k| k.scale = Some(1.06)), key(1200.0, &|k| k.scale = Some(1.0))], Ease::EaseInOut, true),
        Preset::Spin => (vec![key(0.0, &|k| k.rotate = Some(0.0)), key(1000.0, &|k| k.rotate = Some(360.0))], Ease::Linear, true),
        Preset::Blink => (vec![key(0.0, &|k| k.opacity = Some(rest)), key(500.0, &|k| k.opacity = Some(0.0)), key(1000.0, &|k| k.opacity = Some(0.0))], Ease::Hold, true),
        Preset::DrawOn => (vec![key(0.0, &|k| k.trim = Some(0.0)), key(800.0, &|k| k.trim = Some(1.0))], Ease::EaseInOut, false),
        Preset::FillRing => {
            let sweep = n.arc.as_ref().map_or(360.0, |a| a.sweep.clamp(-360.0, 360.0));
            (vec![key(0.0, &|k| k.sweep = Some(0.0)), key(900.0, &|k| k.sweep = Some(sweep))], Ease::EaseOut, false)
        }
    };
    Motion { keys, ease: Some(ease), origin: None, looped: looped.then_some(true) }
}

/// Whether an SVG's bytes carry their own animation: an SMIL element (`<animate…>`, `<set>`, with
/// or without a namespace prefix) or GTK's path animation states (`gpa:states`). A comment that
/// names one counts too; the cost is only a picture repainted while Play runs.
pub fn svg_moves(bytes: &[u8]) -> bool {
    if bytes.windows(10).any(|w| w == b"gpa:states") {
        return true;
    }
    let mut rest = bytes;
    while let Some(at) = rest.iter().position(|&b| b == b'<') {
        rest = &rest[at + 1..];
        let len = rest.iter().position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b':' | b'-' | b'_' | b'.'))).unwrap_or(rest.len());
        let name = &rest[..len];
        let local = name.rsplit(|&b| b == b':').next().unwrap_or(name);
        if local.starts_with(b"animate") || local == b"set" {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// A node as `design.get` gives it, checked as an agent's add is.
    fn node(body: Value) -> Node {
        let n = Node::read(body.clone()).unwrap_or_else(|why| panic!("{body}: {why}"));
        n.validate().unwrap_or_else(|why| panic!("{body}: {why}"));
        n
    }

    /// A motion of `ms` that moves `dx` from 0 to 10.
    fn moving(ms: u32) -> Value {
        json!({"ease": "linear", "keys": [{"at_ms": 0, "dx": 0}, {"at_ms": ms, "dx": 10}]})
    }

    /// A page `p` holding: a mover `a` (500 ms) on the page's clock; frame `f` with a 3000 ms
    /// timeline that plays once, holding `b` (800 ms) and frame `g` with its own timeline (no
    /// length) holding `c` (1200 ms); a hidden frame `h` holding a mover `d`; a still rect `s`.
    fn page() -> Scene {
        Scene::new(vec![
            node(json!({"id": "p", "kind": "page"})),
            node(json!({"id": "a", "parent": "p", "kind": "rect", "w": 10, "h": 10, "motion": moving(500)})),
            node(json!({"id": "f", "parent": "p", "kind": "frame", "w": 200, "h": 200, "timeline": {"duration_ms": 3000, "loop": false}})),
            node(json!({"id": "b", "parent": "f", "kind": "rect", "w": 10, "h": 10, "motion": moving(800)})),
            node(json!({"id": "g", "parent": "f", "kind": "frame", "w": 100, "h": 100, "timeline": {}})),
            node(json!({"id": "c", "parent": "g", "kind": "ellipse", "w": 10, "h": 10, "motion": moving(1200)})),
            node(json!({"id": "h", "parent": "p", "kind": "frame", "w": 50, "h": 50, "hidden": true})),
            node(json!({"id": "d", "parent": "h", "kind": "rect", "w": 10, "h": 10, "motion": moving(9000)})),
            node(json!({"id": "s", "parent": "p", "kind": "rect", "w": 10, "h": 10})),
        ])
    }

    #[test]
    fn clocks_follow_the_nearest_timeline() {
        let scene = page();
        let clocks = Clocks::new(&scene, "p");
        let movers: Vec<(&str, &str)> = clocks.movers().map(|(id, c)| (id, c.owner.as_str())).collect();
        assert_eq!(movers, [("a", "p"), ("b", "f"), ("c", "g")], "the hidden frame's mover is skipped");
        // The page's implicit clock lasts as long as its latest key and loops; a frame's timeline
        // its own length; a nested timeline without one as long as what plays on it.
        assert_eq!(clocks.clocks(), [
            Clock { owner: "p".into(), duration_ms: 500.0, looped: true },
            Clock { owner: "f".into(), duration_ms: 3000.0, looped: false },
            Clock { owner: "g".into(), duration_ms: 1200.0, looped: true },
        ]);
        assert_eq!(clocks.clock("g").map(|c| c.duration_ms), Some(1200.0));
        assert_eq!((clocks.clock("h"), clocks.is_empty()), (None, false));
        // A frame with a timeline that moves itself plays on its own clock; a clock nothing plays on
        // is left out; a page's own timeline sets its clock.
        let scene = Scene::new(vec![
            node(json!({"id": "p", "kind": "page", "timeline": {"duration_ms": 2000, "loop": false}})),
            node(json!({"id": "f", "parent": "p", "kind": "frame", "timeline": {"duration_ms": 700}, "motion": moving(400)})),
            node(json!({"id": "e", "parent": "p", "kind": "frame", "timeline": {"duration_ms": 9000}})),
            node(json!({"id": "a", "parent": "p", "kind": "rect", "motion": moving(100)})),
        ]);
        let clocks = Clocks::new(&scene, "p");
        assert_eq!(clocks.movers().map(|(id, c)| (id, c.owner.as_str())).collect::<Vec<_>>(), [("f", "f"), ("a", "p")]);
        assert_eq!(clocks.clocks(), [
            Clock { owner: "p".into(), duration_ms: 2000.0, looped: false },
            Clock { owner: "f".into(), duration_ms: 700.0, looped: true },
        ]);
        assert!(Clocks::new(&scene, "nowhere").is_empty());
        let still = Scene::new(vec![node(json!({"id": "p", "kind": "page", "timeline": {}})), node(json!({"id": "r", "parent": "p", "kind": "rect"}))]);
        let none = Clocks::new(&still, "p");
        assert!(none.is_empty() && none.clocks().is_empty() && !none.touches("p"));
        assert_eq!(none.span(), (0.0, false));
    }

    #[test]
    fn local_time_ends_on_the_end_and_wraps_past_it() {
        assert_eq!((local_time(0.0, 1000.0, true), local_time(400.0, 1000.0, true), local_time(1000.0, 1000.0, true)), (0.0, 400.0, 1000.0));
        assert_eq!((local_time(1250.0, 1000.0, true), local_time(2000.0, 1000.0, true)), (250.0, 0.0));
        assert_eq!((local_time(1250.0, 1000.0, false), local_time(99_999.0, 1000.0, false)), (1000.0, 1000.0), "one that plays once holds its end");
        assert_eq!((local_time(-5.0, 1000.0, true), local_time(f64::NAN, 1000.0, true), local_time(500.0, 0.0, true), local_time(500.0, f64::NAN, false)), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn touched_holds_movers_and_their_ancestors() {
        let scene = page();
        let mut clocks = Clocks::new(&scene, "p");
        for id in ["a", "b", "c", "f", "g", "p"] {
            assert!(clocks.touches(id), "{id}");
        }
        for id in ["s", "h", "d", "nowhere"] {
            assert!(!clocks.touches(id), "{id}");
        }
        // A picture that moves by itself is repainted too, with what holds it.
        let mut nodes = scene.to_nodes();
        nodes.push(node(json!({"id": "pic", "parent": "h", "kind": "image", "src": "0123456789abcdef.svg", "w": 24, "h": 24})));
        let scene = Scene::new(nodes);
        clocks.touch(&scene, "pic");
        assert!(clocks.touches("pic") && clocks.touches("h") && !clocks.touches("d") && !clocks.touches("s"));
        assert_eq!(clocks.movers().count(), 3, "touching is not moving");
    }

    #[test]
    fn poses_cover_only_movers() {
        let scene = page();
        let clocks = Clocks::new(&scene, "p");
        let mut poses = HashMap::new();
        poses.insert("stale".to_string(), Pose::default());
        clocks.poses(&scene, 250.0, &mut poses);
        let mut ids: Vec<&str> = poses.keys().map(String::as_str).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!((poses["a"].dx, poses["b"].dx), (5.0, 3.125));
        // At 1100 ms the page's clock has looped (100 of 500), frame f's holds no end yet (1100 of
        // 3000, so b's motion ended at 800), and g's is at 1100 of 1200.
        clocks.poses(&scene, 1100.0, &mut poses);
        assert_eq!((poses["a"].dx, poses["b"].dx, poses["c"].dx), (2.0, 10.0, 1100.0 / 120.0));
        // Past frame f's end (it plays once) its movers hold their end; a looping one inside a
        // nested timeline keeps going.
        clocks.poses(&scene, 3600.0, &mut poses);
        assert_eq!((poses["b"].dx, poses["c"].dx), (10.0, 0.0));
        // A motion gone since the clocks were made is left out.
        let mut scene = scene;
        scene.node_mut("a").unwrap().motion = None;
        clocks.poses(&scene, 100.0, &mut poses);
        assert!(!poses.contains_key("a") && poses.len() == 2);
    }

    #[test]
    fn posed_bounds_turn_and_scale_about_the_origin() {
        let r = Rect::new(100.0, 50.0, 40.0, 20.0);
        assert_eq!(posed_bounds(&r, &Pose::default()), r);
        let near = |a: Rect, b: Rect| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.w - b.w).abs() < 1e-9 && (a.h - b.h).abs() < 1e-9;
        // Offset only.
        let moved = Pose { dx: 5.0, dy: -10.0, ..Pose::default() };
        assert_eq!(posed_bounds(&r, &moved), Rect::new(105.0, 40.0, 40.0, 20.0));
        // Doubled about the centre, and about the bottom middle (a bar that grows up).
        let double = Pose { scale_x: 2.0, scale_y: 2.0, ..Pose::default() };
        assert!(near(posed_bounds(&r, &double), Rect::new(80.0, 40.0, 80.0, 40.0)));
        let grow = Pose { scale_y: 3.0, origin: [0.5, 1.0], ..Pose::default() };
        assert!(near(posed_bounds(&r, &grow), Rect::new(100.0, 10.0, 40.0, 60.0)));
        // A quarter turn about the centre swaps width and height in place; about the top left it
        // swings down to the left of that corner (clockwise, y down).
        let quarter = Pose { rotate: 90.0, ..Pose::default() };
        assert!(near(posed_bounds(&r, &quarter), Rect::new(110.0, 40.0, 20.0, 40.0)));
        let corner = Pose { rotate: 90.0, origin: [0.0, 0.0], ..Pose::default() };
        assert!(near(posed_bounds(&r, &corner), Rect::new(80.0, 50.0, 20.0, 40.0)));
        // Scale, then turn, then offset, as the painter's transform runs.
        let all = Pose { dx: 10.0, scale_x: 0.5, rotate: 90.0, ..Pose::default() };
        assert!(near(posed_bounds(&r, &all), Rect::new(120.0, 50.0, 20.0, 20.0)), "{:?}", posed_bounds(&r, &all));
        // An eighth turn of a square reaches its diagonal.
        let sq = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = posed_bounds(&sq, &Pose { rotate: 45.0, ..Pose::default() });
        assert!((b.w - 200f64.sqrt()).abs() < 1e-9 && (b.cx() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn span_of_a_selection() {
        let scene = page();
        let clocks = Clocks::new(&scene, "p");
        assert_eq!(clocks.span(), (3000.0, false), "the longest clock; frame f plays once");
        let span = |ids: &[&str]| clocks.span_of(&scene, &ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(span(&["a"]), (500.0, true));
        assert_eq!(span(&["g"]), (1200.0, true));
        assert_eq!(span(&["c"]), (1200.0, true), "a mover selected alone plays on its own clock");
        assert_eq!(span(&["b"]), (3000.0, false), "b's clock is its frame's timeline");
        assert_eq!(span(&["f"]), (3000.0, false));
        assert_eq!(span(&["a", "g"]), (1200.0, true));
        assert_eq!(span(&["p"]), clocks.span());
        assert_eq!((span(&["s"]), span(&["h"]), span(&[])), ((0.0, false), (0.0, false), (0.0, false)));
    }

    #[test]
    fn presets_validate_for_their_kinds() {
        let kinds = [
            json!({"kind": "frame", "w": 100, "h": 100}),
            json!({"kind": "group"}),
            json!({"kind": "rect", "w": 10, "h": 10, "opacity": 0.5}),
            json!({"kind": "ellipse", "w": 10, "h": 10}),
            json!({"kind": "ellipse", "w": 10, "h": 10, "arc": {"start": 30, "sweep": -270, "inner": 0.7, "cap": "round"}}),
            json!({"kind": "line", "points": [[0, 0], [40, 0]], "w": 40, "h": 0}),
            json!({"kind": "path", "points": [[0, 0], [10, 10], [20, 0]], "w": 20, "h": 10}),
            json!({"kind": "text", "text": "Hello", "opacity": 0}),
            json!({"kind": "sticky", "text": "Note", "w": 100, "h": 100}),
            json!({"kind": "image", "src": "0123456789abcdef.png", "w": 10, "h": 10}),
            json!({"kind": "polygon", "w": 10, "h": 10}),
            json!({"kind": "icon", "icon": "star", "w": 24, "h": 24}),
        ];
        let mut seen = HashSet::new();
        for mut body in kinds {
            body["id"] = json!("n");
            body["parent"] = json!("p");
            let n = node(body.clone());
            for &p in presets_for(n.kind) {
                seen.insert(p);
                let motion = preset(p, &n);
                let moved = Node { motion: Some(motion.clone()), ..n.clone() };
                moved.validate().unwrap_or_else(|why| panic!("{p:?} on {body}: {why}"));
                moved.check_new().unwrap_or_else(|why| panic!("{p:?} on {body}: {why}"));
                // A preset that arrives, arrives at the rest look; one that loops ends where it began
                // (a spin a whole turn round).
                let end = motion.pose(motion.last_ms());
                assert!(!Pose { rotate: end.rotate % 360.0, ..end.clone() }.moves(), "{p:?} ends at rest");
                if let Some(o) = end.opacity.filter(|_| motion.looped.is_none()) {
                    assert_eq!(o, n.opacity.unwrap_or(1.0), "{p:?} ends at the rest opacity");
                }
            }
        }
        assert_eq!(seen.len(), Preset::ALL.len(), "every preset has a kind that takes it");
        assert!(presets_for(NodeKind::Page).is_empty());
        assert!(!presets_for(NodeKind::Rect).contains(&Preset::DrawOn) && !presets_for(NodeKind::Line).contains(&Preset::FillRing));
        // Values read from the node's rest look.
        let ring = node(json!({"id": "r", "parent": "p", "kind": "ellipse", "arc": {"sweep": 270, "inner": 0.6}}));
        assert_eq!(preset(Preset::FillRing, &ring).pose(900.0).sweep, Some(270.0));
        assert_eq!(preset(Preset::FillRing, &node(json!({"id": "e", "parent": "p", "kind": "ellipse"}))).pose(900.0).sweep, Some(360.0));
        let faint = node(json!({"id": "t", "parent": "p", "kind": "rect", "opacity": 0.4}));
        let blink = preset(Preset::Blink, &faint);
        assert_eq!((blink.pose(0.0).opacity, blink.pose(700.0).opacity, blink.pose(1200.0).opacity), (Some(0.4), Some(0.0), Some(0.4)));
        assert_eq!(serde_json::to_value(preset(Preset::Spin, &faint)).unwrap(),
            json!({"keys": [{"at_ms": 0.0, "rotate": 0.0}, {"at_ms": 1000.0, "rotate": 360.0}], "ease": "linear", "loop": true}));
        assert_eq!(Preset::ALL.iter().map(|p| p.label()).collect::<Vec<_>>(), ["Fade in", "Slide up", "Slide in", "Pop", "Pulse", "Spin", "Blink", "Draw on", "Fill ring"]);
    }

    #[test]
    fn svg_moves_spots_smil() {
        let moving = [
            r#"<svg><circle r="4"><animate attributeName="r" from="4" to="8" dur="1s"/></circle></svg>"#,
            r#"<svg><g><animateTransform attributeName="transform" type="rotate" dur="2s"/></g></svg>"#,
            r#"<svg><rect><set attributeName="fill" to="red" begin="1s"/></rect></svg>"#,
            r#"<svg><rect><set
                attributeName="fill" to="red"/></rect></svg>"#,
            r#"<svg:svg xmlns:svg="http://www.w3.org/2000/svg"><svg:animateMotion dur="1s"/></svg:svg>"#,
            r#"<svg xmlns:gpa="https://www.gtk.org/grappa"><path gpa:states="0 1" d="M0 0h4"/></svg>"#,
        ];
        for svg in moving {
            assert!(svg_moves(svg.as_bytes()), "{svg}");
        }
        let still = [
            r##"<svg viewBox="0 0 24 24"><path d="M0 0h24v24z" fill="#4AF0FF"/></svg>"##,
            r#"<svg><settings/><setup/><animal/><text>animate set</text></svg>"#,
            "",
            "<",
        ];
        for svg in still {
            assert!(!svg_moves(svg.as_bytes()), "{svg}");
        }
    }

    #[test]
    fn frames_and_sheets_cover_the_span() {
        assert_eq!(sheet_times(3000.0, 6), [0.0, 600.0, 1200.0, 1800.0, 2400.0, 3000.0]);
        assert_eq!(sheet_times(1000.0, 1), [0.0, 1000.0], "a sheet has two cells at least");
        // A loop leaves out its end, which is the next loop's start; one that plays once holds it.
        let looped = frame_times(1000.0, true, 30);
        assert_eq!(looped.len(), 30);
        assert!((looped[1] - 1000.0 / 30.0).abs() < 1e-9 && looped[29] < 1000.0);
        let once = frame_times(1000.0, false, 30);
        assert_eq!((once.len(), once[30]), (31, 1000.0));
        assert_eq!(frame_times(3000.0, true, 30).len(), 90);
        // An end between two frames gets a last frame of its own when it plays once.
        assert_eq!(frame_times(110.0, false, 10), [0.0, 100.0, 110.0]);
        assert_eq!(frame_times(110.0, true, 10), [0.0, 100.0]);
        assert_eq!((frame_times(0.0, true, 30), frame_times(0.0, false, 30)), (vec![0.0], vec![0.0]));
        assert_eq!(frame_times(500.0, true, 0).len(), 1, "fps below 1 is 1");
    }
}
