//! What a node paints (client-geometry; MODEL-SPEC §1 #11): the box everything it draws fits in,
//! its words, shadows, stroke ends and arrowheads included, for export, the render job's area and
//! the canvas's fit and thumbnails. GTK-free, so CI tests it; it reads the same shapes helpers
//! the painter draws with.

use super::geom::Rect;
use super::layout::{wrap_width, Measure, Placed, STICKY_PAD};
use super::scene::Scene;
use super::shapes;
use relay_bus::design::{Node, NodeKind, StrokeCap, StrokeJoin};

/// Everything `id` paints, in its parent's coordinates (page coordinates under unturned
/// parents): what it draws before its turn (its box; a line's or path's drawn points wherever
/// its stored box is, with its stroke's reach and its arrowheads; a label's, text's or sticky's
/// words where they overflow the box), turned about its box's centre as paint turns it, then its
/// outer shadows and a ring segment's capped ends; a group's or unclipped frame's children,
/// turned with it, less those hidden or wholly clear; and its blur over all of it. `None` for
/// what is not placed (a hidden node, a page with nothing on it).
pub fn painted_bounds(scene: &Scene, placed: &Placed, measure: &dyn Measure, id: &str) -> Option<Rect> {
    let n = scene.node(id)?;
    let r = placed.rect(id)?;
    let turn = shapes::rotation(n);
    let own = match n.kind {
        NodeKind::Page | NodeKind::Group => None,
        NodeKind::Line | NodeKind::Path => {
            let drawn = shapes::drawn_box(scene, placed, n)?.inflate(stroke_reach(n));
            Some(shapes::arrowheads(scene, placed, n, &r).into_iter().filter_map(Rect::around).fold(drawn, |b, head| b.union(&head)))
        }
        NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon => {
            Some(shapes::label_box(n, &r, measure).map_or(r, |label| r.union(&words(label.x, label.y, label.w, measure.measure(n, Some(label.w))))))
        }
        NodeKind::Text => {
            let wrap = wrap_width(n, &r);
            let size = measure.measure(n, wrap);
            Some(r.union(&words(r.x, r.y, wrap.unwrap_or(size.0), size)))
        }
        NodeKind::Sticky if n.text.as_deref().is_some_and(|t| !t.is_empty()) => {
            let wrap = wrap_width(n, &r).unwrap_or(1.0);
            Some(r.union(&words(r.x + STICKY_PAD, r.y + STICKY_PAD, wrap, measure.measure(n, Some(wrap)))))
        }
        // An icon's stroke is drawn in its 24-unit grid, scaled to the box's shorter side.
        NodeKind::Icon => Some(r.inflate(n.stroke_width.unwrap_or(2.0).max(0.0) / 2.0 * r.w.min(r.h) / 24.0)),
        NodeKind::Sticky | NodeKind::Frame | NodeKind::Image => Some(r),
    };
    let own = own.map(|o| shapes::rotated_bounds_about(&o, turn, r.cx(), r.cy()).inflate(shapes::shadow_reach(n) + shapes::cap_reach(n, &r)));
    // What overflows a group or an unclipped frame shows, so it is pictured too.
    let clips = n.kind == NodeKind::Frame && n.clip != Some(false);
    let shown = |c: &&String| scene.node(c).is_some_and(|k| !k.is_hidden() && !k.opacity.is_some_and(|o| o <= 0.0));
    let kids = if n.kind.holds_children() && !clips {
        scene
            .children(id)
            .iter()
            .filter(shown)
            .filter_map(|c| painted_bounds(scene, placed, measure, c))
            .reduce(|a, b| a.union(&b))
            .map(|k| shapes::rotated_bounds_about(&k, turn, r.cx(), r.cy()))
    } else {
        None
    };
    let all = match (own, kids) {
        (Some(own), Some(kids)) => own.union(&kids),
        (own, kids) => own.or(kids).or((n.kind == NodeKind::Group).then_some(r))?,
    };
    Some(all.inflate(n.blur.unwrap_or(0.0).max(0.0) * 2.0))
}

/// `painted_bounds` on the page: carried out through each turned ancestor, innermost first, so a
/// selected child of a turned frame gets the box it is drawn in.
pub fn painted_bounds_in_page(scene: &Scene, placed: &Placed, measure: &dyn Measure, id: &str) -> Option<Rect> {
    let b = painted_bounds(scene, placed, measure, id)?;
    Some(shapes::turned_ancestors(scene, placed, id).iter().rev().fold(b, |b, &(cx, cy, d)| shapes::rotated_bounds_about(&b, d, cx, cy)))
}

/// Words laid out `w` wide from `(x, y)` and measured `(mw, mh)`: a word too long to wrap passes
/// the sides by what it is over, whichever way it is aligned.
fn words(x: f64, y: f64, w: f64, (mw, mh): (f64, f64)) -> Rect {
    let over = (mw - w).max(0.0);
    Rect::new(x - over, y, w + 2.0 * over, mh)
}

/// How far a line's or path's stroke reaches past its points: half its weight, a square cap's
/// corner √2 times that, a mitred corner up to four times (the miter limit).
fn stroke_reach(n: &Node) -> f64 {
    let half = shapes::line_width(n) / 2.0;
    if n.stroke_join == Some(StrokeJoin::Miter) {
        half * 4.0
    } else if n.stroke_cap == Some(StrokeCap::Square) {
        half * std::f64::consts::SQRT_2
    } else {
        half
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::node;
    use relay_bus::design::{NodeKind as K, Shadow};

    /// One page holding `nodes`, placed.
    fn page(nodes: Vec<Node>) -> (Scene, Placed) {
        let s = Scene::new(std::iter::once(node("p1", None, K::Page, [0.0; 4])).chain(nodes).collect());
        let p = place(&s, "p1", &Mono);
        (s, p)
    }

    fn bounds(nodes: Vec<Node>, id: &str) -> Rect {
        let (s, p) = page(nodes);
        painted_bounds(&s, &p, &Mono, id).unwrap()
    }

    fn holds(b: &Rect, pts: impl IntoIterator<Item = (f64, f64)>) -> bool {
        pts.into_iter().all(|(x, y)| b.inflate(1e-9).contains(x, y))
    }

    #[test]
    fn export_bounds_hold_an_overflowing_label() {
        // B32: a long label on a short shape is painted above and below it.
        let mut rect = node("r", Some("p1"), K::Rect, [0.0, 0.0, 120.0, 40.0]);
        rect.text = Some("x".repeat(40));
        let label = shapes::label_box(&rect, &Rect::new(0.0, 0.0, 120.0, 40.0), &Mono).unwrap();
        let b = bounds(vec![rect.clone()], "r");
        assert!(b.contains_rect(&label) && b.y < 0.0 && b.bottom() > 40.0, "{b:?} holds {label:?}");
        rect.rotation = Some(90.0);
        let b = bounds(vec![rect], "r");
        assert!(holds(&b, label.corners().map(|(x, y)| shapes::rotate(x, y, 60.0, 20.0, 90.0))), "turned about the box's centre: {b:?}");
        let mut text = node("t", Some("p1"), K::Text, [0.0, 0.0, 96.0, 10.0]);
        (text.text, text.w_sizing, text.h_sizing) = (Some("y".repeat(30)), Some(relay_bus::design::Sizing::Fixed), Some(relay_bus::design::Sizing::Fixed));
        assert_eq!(bounds(vec![text], "t"), Rect::new(0.0, 0.0, 96.0, 60.0), "three lines of 20 px");
        let mut sticky = node("s", Some("p1"), K::Sticky, [0.0, 0.0, 100.0, 100.0]);
        sticky.text = Some("z".repeat(80));
        // Ten lines of eight at its 68 px wrap; layout grows the sticky to hold them.
        assert!(bounds(vec![sticky], "s").contains_rect(&Rect::new(16.0, 16.0, 68.0, 200.0)));
    }

    #[test]
    fn a_stickys_own_shadow_is_pictured() {
        // B121: export counted only a stored shadow, so a sticky's built-in one was cropped.
        let b = bounds(vec![node("s", Some("p1"), K::Sticky, [0.0, 0.0, 100.0, 100.0])], "s");
        assert!(b.bottom() >= 100.0 + 13.0, "{b:?}");
        let mut none = node("s", Some("p1"), K::Sticky, [0.0, 0.0, 100.0, 100.0]);
        none.shadow = Some(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: "#00000000".into(), inset: false });
        assert_eq!(bounds(vec![none], "s"), Rect::new(0.0, 0.0, 100.0, 100.0), "a clear shadow casts nothing");
        // An arrow 10 wide: its head is 35 long, its wings 0.55 × 35 off the line.
        let mut arrow = node("ln", Some("p1"), K::Line, [200.0, 200.0, 100.0, 0.0]);
        (arrow.points, arrow.stroke_width, arrow.arrow_end) = (Some(vec![[0.0, 0.0], [100.0, 0.0]]), Some(10.0), Some(true));
        let b = bounds(vec![arrow], "ln");
        assert!(b.y <= 200.0 - 0.55 * 35.0 && b.bottom() >= 200.0 + 0.55 * 35.0, "{b:?}");
    }

    #[test]
    fn turned_shadows_and_barbs_stay_inside() {
        // TM-28: the offset was counted as max(|x|, |y|) after the turn.
        let mut rect = node("r", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 50.0]);
        rect.rotation = Some(45.0);
        rect.shadow = Some(Shadow { x: 10.0, y: 10.0, blur: 0.0, spread: 0.0, color: "#000".into(), inset: false });
        let b = bounds(vec![rect], "r");
        let (ox, oy) = shapes::rotate(10.0, 10.0, 0.0, 0.0, 45.0);
        let cast = Rect::new(0.0, 0.0, 100.0, 50.0).corners().map(|(x, y)| shapes::rotate(x, y, 50.0, 25.0, 45.0)).map(|(x, y)| (x + ox, y + oy));
        assert!(holds(&b, cast), "{b:?}");
        // A short last segment: the head reaches back past the line's start.
        let mut stub = node("ln", Some("p1"), K::Line, [0.0, 0.0, 4.0, 0.0]);
        (stub.points, stub.stroke_width, stub.arrow_end) = (Some(vec![[0.0, 0.0], [4.0, 0.0]]), Some(10.0), Some(true));
        let b = bounds(vec![stub], "ln");
        assert!(b.x <= 4.0 - 35.0, "{b:?}");
    }

    #[test]
    fn a_line_without_a_box_is_pictured_whole() {
        // B3: a line an agent wrote without w/h has a 0x0 box; export cropped it to about 8x8.
        let mut line = node("ln", Some("p1"), K::Line, [100.0, 100.0, 0.0, 0.0]);
        line.points = Some(vec![[0.0, 0.0], [300.0, -50.0]]);
        assert_eq!(bounds(vec![line.clone()], "ln"), Rect::new(99.0, 49.0, 302.0, 52.0));
        line.stroke_join = Some(StrokeJoin::Miter);
        assert_eq!(bounds(vec![line], "ln"), Rect::new(96.0, 46.0, 308.0, 58.0), "a mitred corner can reach four half-weights");
        // A connector is pictured where it runs.
        let mut c = node("c", Some("p1"), K::Line, [0.0; 4]);
        (c.from, c.to) = (Some("a".into()), Some("b".into()));
        let b = bounds(vec![node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 100.0]), node("b", Some("p1"), K::Rect, [300.0, 0.0, 100.0, 100.0]), c], "c");
        assert_eq!(b, Rect::new(99.0, 49.0, 202.0, 2.0));
    }

    #[test]
    fn a_turned_group_turns_its_children() {
        // TM-29: a turned group's bounds took its children unturned.
        let mut g = node("g", Some("p1"), K::Group, [0.0, 0.0, 0.0, 0.0]);
        g.rotation = Some(45.0);
        let b = bounds(vec![g, node("c", Some("g"), K::Rect, [0.0, 0.0, 100.0, 10.0])], "g");
        assert!(holds(&b, Rect::new(0.0, 0.0, 100.0, 10.0).corners().map(|(x, y)| shapes::rotate(x, y, 50.0, 5.0, 45.0))), "{b:?}");
        // A clipping frame pictures its box; an unclipped one what spills out of it too.
        let mut f = node("f", Some("p1"), K::Frame, [0.0, 0.0, 50.0, 50.0]);
        let spill = node("c", Some("f"), K::Rect, [40.0, 40.0, 40.0, 40.0]);
        assert_eq!(bounds(vec![f.clone(), spill.clone()], "f"), Rect::new(0.0, 0.0, 50.0, 50.0));
        f.clip = Some(false);
        assert_eq!(bounds(vec![f, spill], "f"), Rect::new(0.0, 0.0, 80.0, 80.0));
    }

    #[test]
    fn clear_and_hidden_children_add_nothing() {
        // E-export-3: an opacity-0 layer far away widened the bounds.
        let mut ghost = node("ghost", Some("g"), K::Rect, [1000.0, 1000.0, 10.0, 10.0]);
        ghost.opacity = Some(0.0);
        let mut hidden = node("hid", Some("g"), K::Rect, [-500.0, 0.0, 10.0, 10.0]);
        hidden.hidden = Some(true);
        let nodes = vec![node("g", Some("p1"), K::Group, [0.0; 4]), node("c", Some("g"), K::Rect, [0.0, 0.0, 10.0, 10.0]), ghost, hidden];
        assert_eq!(bounds(nodes.clone(), "g"), Rect::new(0.0, 0.0, 10.0, 10.0));
        let (s, p) = page(nodes);
        assert!(painted_bounds(&s, &p, &Mono, "hid").is_none(), "a hidden node paints nothing");
        assert_eq!(painted_bounds(&s, &p, &Mono, "p1"), Some(Rect::new(0.0, 0.0, 10.0, 10.0)), "a page is what is on it");
    }

    #[test]
    fn a_child_of_a_turned_frame_is_pictured_on_the_page() {
        // TM-26 (part b): a selection's bounds through its ancestors' turns.
        let mut f = node("f", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        f.rotation = Some(30.0);
        let (s, p) = page(vec![f, node("c", Some("f"), K::Rect, [50.0, 45.0, 100.0, 10.0])]);
        assert_eq!(painted_bounds(&s, &p, &Mono, "c"), Some(Rect::new(50.0, 45.0, 100.0, 10.0)), "in the frame's coordinates");
        let b = painted_bounds_in_page(&s, &p, &Mono, "c").unwrap();
        let (cos, sin) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        assert!((b.w - (100.0 * cos + 10.0 * sin)).abs() < 1e-9 && (b.h - (100.0 * sin + 10.0 * cos)).abs() < 1e-9, "{b:?}");
        assert!((b.cx() - 100.0).abs() < 1e-9 && (b.cy() - 50.0).abs() < 1e-9, "turned about the frame's centre");
    }

    #[test]
    fn rings_and_blur_reach_out() {
        let mut ring = node("e", Some("p1"), K::Ellipse, [0.0, 0.0, 100.0, 100.0]);
        ring.arc = Some(relay_bus::design::Arc { start: 45.0, sweep: 90.0, inner: 0.2, cap: Some(StrokeCap::Square) });
        assert_eq!(bounds(vec![ring], "e"), Rect::new(-20.0, -20.0, 140.0, 140.0), "a square cap may pass the box by half the ring");
        let mut blurred = node("r", Some("p1"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        blurred.blur = Some(4.0);
        assert_eq!(bounds(vec![blurred], "r"), Rect::new(-8.0, -8.0, 26.0, 26.0));
        assert_eq!(bounds(vec![node("i", Some("p1"), K::Icon, [0.0, 0.0, 48.0, 48.0])], "i"), Rect::new(-2.0, -2.0, 52.0, 52.0), "an icon's stroke, scaled with it");
    }
}
