//! What `design.render` answers besides the picture (render-SPEC-v2 §7): every node's laid-out
//! box as a row, the page area the picture covers, and the scale that keeps it within its limits.
//! GTK-free, so CI tests it. The desktop app's render job calls it with the scene as drawn, the
//! still placement (`layout::place`) and the painter's text measure, and answers with the
//! relay-bus types it builds, so the contract has one definition.

use super::bounds::{painted_bounds, painted_bounds_in_page};
use super::edit::outermost;
use super::geom::Rect;
use super::layout::{Measure, Placed};
use super::scene::Scene;
use super::shapes;
use relay_bus::ops::design::{RenderBoxes, RenderRect, RENDER_BOXES_MAX};
use serde_json::{json, Value};

/// The columns of every box row, in order.
pub const FIELDS: [&str; 10] = ["id", "kind", "parent", "x", "y", "w", "h", "rx", "ry", "rot"];
/// Picture pixels in all (w × h), 4 megapixels; export keeps its own cap per side.
pub const MAX_AREA_PX: f64 = 4_000_000.0;
/// Room around a page's content in its picture.
pub const PAGE_MARGIN: f64 = 32.0;

/// The nodes a render measures, in `design.get`'s order (parents before children, siblings
/// bottom to top): with no `ids`, or the page among them, everything on `page` but the page;
/// else the outermost of `ids` that are on `page`, each with everything inside it.
pub fn scope(scene: &Scene, page: &str, ids: &[String]) -> Vec<String> {
    if whole(page, ids) {
        return scene.subtree(page).into_iter().skip(1).collect();
    }
    outermost(scene, &on_page(scene, page, ids)).iter().flat_map(|id| scene.subtree(id)).collect()
}

/// Whether `ids` ask for the whole page.
fn whole(page: &str, ids: &[String]) -> bool {
    ids.is_empty() || ids.iter().any(|id| id == page)
}

fn on_page(scene: &Scene, page: &str, ids: &[String]) -> Vec<String> {
    ids.iter().filter(|id| scene.page_of(id) == Some(page)).cloned().collect()
}

/// The page rectangle a picture of `ids` covers (none: the whole page), in page px: what the
/// page's visible content paints with [`PAGE_MARGIN`] around it, or what the outermost of
/// `ids` paint, carried through their ancestors' turns, then widened to whole px on every
/// side. `None` when nothing in scope is visible.
pub fn area(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str, ids: &[String]) -> Option<Rect> {
    let drawn = if whole(page, ids) {
        painted_bounds(scene, placed, measure, page)?.inflate(PAGE_MARGIN)
    } else {
        outermost(scene, &on_page(scene, page, ids))
            .iter()
            .filter_map(|id| painted_bounds_in_page(scene, placed, measure, id))
            .reduce(|a, b| a.union(&b))?
    };
    Some(whole_px(&drawn))
}

/// `r` widened to whole px on every side, at least 1 px each way: its left and top floored, its
/// right and bottom ceiled, so a fractional edge is never cut (TM-27).
pub fn whole_px(r: &Rect) -> Rect {
    let (x0, y0, x1, y1) = (r.x.floor(), r.y.floor(), r.right().ceil(), r.bottom().ceil());
    Rect::new(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
}

/// `r` as the answer's rectangle.
pub fn render_rect(r: &Rect) -> RenderRect {
    RenderRect { x: r.x, y: r.y, w: r.w, h: r.h }
}

/// One row of [`FIELDS`] per visible node in [`scope`], the first `limit` of them: its box in
/// page px as layout placed it (unturned), its corner from its parent's origin (a group's own
/// point, not its children's bounds) and its turn as stored, each to 0.01 px. A node in scope
/// with no box (hidden, or inside a hidden one) is counted in `hidden`.
pub fn boxes(scene: &Scene, placed: &Placed, page: &str, ids: &[String], limit: usize) -> RenderBoxes {
    let (mut rows, mut total, mut hidden) = (Vec::new(), 0u32, 0u32);
    for id in scope(scene, page, ids) {
        let (Some(n), Some(r)) = (scene.node(&id), placed.rect(&id)) else {
            hidden += 1;
            continue;
        };
        total += 1;
        if rows.len() < limit {
            let parent = n.parent.as_deref().unwrap_or(page);
            let (ox, oy) = placed.origin(parent);
            let at = [r.x, r.y, r.w, r.h, r.x - ox, r.y - oy, shapes::rotation(n)].map(num);
            rows.push([json!(n.id), json!(n.kind.name()), json!(parent)].into_iter().chain(at).collect());
        }
    }
    RenderBoxes { fields: FIELDS.map(String::from).to_vec(), rows, total, hidden }
}

/// The warning an answer carries when `boxes` left rows out, saying how to see them.
pub fn more_boxes(boxes: &RenderBoxes) -> Option<String> {
    let more = (boxes.total as usize).saturating_sub(boxes.rows.len());
    let what = if more == 1 { "box" } else { "boxes" };
    (more > 0).then(|| format!("{more} more {what}: pass nodes: [...] to measure a part, or boxes_limit up to {RENDER_BOXES_MAX}"))
}

/// A row's number, to 0.01: a whole one as an integer, so a row reads as the design does.
fn num(v: f64) -> Value {
    let v = if v.is_finite() { (v * 100.0).round() / 100.0 + 0.0 } else { 0.0 };
    if v.fract() == 0.0 && v.abs() < 1e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

/// The scale to picture `area` at: `requested`, lowered until the picture, `ceil(w × scale)` by
/// `ceil(h × scale)` px, is at most `max_side` along each side and `max_area_px` in all. An
/// empty or non-finite area keeps `requested`.
pub fn fit_scale(area: &Rect, requested: f64, max_side: u32, max_area_px: f64) -> f64 {
    let (w, h) = (area.w, area.h);
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return requested;
    }
    let side = f64::from(max_side.max(1));
    let mut s = requested.min(side / w.max(h)).min((max_area_px / (w * h)).sqrt());
    if !fits(area, s, side, max_area_px) {
        // Each side rounds up by under a pixel: (w·s + 1)(h·s + 1) within the area always fits.
        let (a, b, c) = (w * h, w + h, 1.0 - max_area_px);
        s = s.min((-b + (b * b - 4.0 * a * c).max(0.0).sqrt()) / (2.0 * a));
    }
    for _ in 0..64 {
        if s <= 0.0 || fits(area, s, side, max_area_px) {
            break;
        }
        s = s.next_down();
    }
    s
}

fn fits(area: &Rect, s: f64, side: f64, max_area_px: f64) -> bool {
    let (w, h) = picture_size(area, s);
    f64::from(w.max(h)) <= side && f64::from(w) * f64::from(h) <= max_area_px
}

/// The picture of `area` at `scale`, in px: `ceil(w × scale)` by `ceil(h × scale)`, each at
/// least 1.
pub fn picture_size(area: &Rect, scale: f64) -> (u32, u32) {
    let px = |v: f64| (v * scale).ceil().clamp(1.0, f64::from(u32::MAX)) as u32;
    (px(area.w), px(area.h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::node;
    use relay_bus::design::{Layout, Node, NodeKind as K, Sizing};


    fn page(nodes: Vec<Node>) -> (Scene, Placed) {
        let s = Scene::new(std::iter::once(node("p1", None, K::Page, [0.0; 4])).chain(nodes).collect());
        let p = place(&s, "p1", &Mono);
        (s, p)
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn text(id: &str, parent: &str, rect: [f64; 4], words: &str) -> Node {
        let mut n = node(id, Some(parent), K::Text, rect);
        n.text = Some(words.into());
        n
    }

    fn frame(id: &str, parent: &str, rect: [f64; 4], layout: Layout) -> Node {
        let mut f = node(id, Some(parent), K::Frame, rect);
        f.layout = Some(layout);
        f
    }

    fn rows(b: &RenderBoxes) -> Vec<String> {
        b.rows.iter().map(|r| serde_json::to_string(r).unwrap()).collect()
    }


    #[test]
    fn area_of_a_page_has_a_32_px_margin_and_whole_px() {
        let (s, p) = page(vec![node("a", Some("p1"), K::Rect, [10.25, 20.5, 100.0, 50.0]), node("b", Some("p1"), K::Rect, [300.0, 0.0, 10.0, 10.0])]);
        let a = area(&s, &p, &Mono, "p1", &[]).unwrap();
        assert_eq!(a, Rect::new(-22.0, -32.0, 364.0, 135.0), "10.25 - 32 floors to -22; 70.5 + 32 ceils to 103");
        assert_eq!(area(&s, &p, &Mono, "p1", &ids(&["p1"])), Some(a), "the page among the ids is the whole page");
        let mut hid = node("h", Some("p1"), K::Rect, [0.0; 4]);
        hid.hidden = Some(true);
        let (s, p) = page(vec![hid]);
        assert_eq!(area(&s, &p, &Mono, "p1", &[]), None, "nothing visible: no picture");
    }

    #[test]
    fn area_covers_a_fractional_right_and_bottom_edge() {
        let (s, p) = page(vec![node("a", Some("p1"), K::Rect, [10.1, 0.0, 100.0, 20.0])]);
        assert_eq!(area(&s, &p, &Mono, "p1", &ids(&["a"])), Some(Rect::new(10.0, 0.0, 101.0, 20.0)));
        // TM-27: a bordered button centred at x 10.5 ends at 110.5, so the picture ends at 111.
        let (s, p) = page(vec![node("b", Some("p1"), K::Rect, [10.5, 4.5, 100.0, 20.0])]);
        let a = area(&s, &p, &Mono, "p1", &ids(&["b"])).unwrap();
        assert_eq!((a.x, a.right(), a.y, a.bottom()), (10.0, 111.0, 4.0, 25.0));
        assert_eq!(whole_px(&Rect::new(3.0, 3.0, 0.0, 0.0)), Rect::new(3.0, 3.0, 1.0, 1.0), "at least a pixel each way");
    }

    #[test]
    fn area_of_nodes_is_their_outermost_painted_bounds() {
        let mut shadowed = node("s", Some("f"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        shadowed.blur = Some(2.0);
        let mut turned = node("t", Some("p1"), K::Frame, [100.0, 0.0, 100.0, 50.0]);
        turned.rotation = Some(90.0);
        let (s, p) = page(vec![
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 50.0, 50.0]),
            shadowed,
            turned,
            node("in", Some("t"), K::Rect, [0.0, 0.0, 100.0, 10.0]),
            node("other", Some("p2"), K::Rect, [0.0; 4]),
        ]);
        // The frame holds its child: the child's own reach adds nothing past the clipping frame.
        assert_eq!(area(&s, &p, &Mono, "p1", &ids(&["s", "f"])), Some(Rect::new(0.0, 0.0, 50.0, 50.0)));
        assert_eq!(area(&s, &p, &Mono, "p1", &ids(&["s"])), Some(Rect::new(-4.0, -4.0, 18.0, 18.0)), "blur reaches twice its radius");
        // A child of a turned frame is pictured where it is drawn: turned a quarter about the
        // frame's centre (150, 25), its 100 x 10 box stands upright about (170, 25).
        assert_eq!(area(&s, &p, &Mono, "p1", &ids(&["in"])), Some(Rect::new(165.0, -25.0, 10.0, 100.0)));
        assert_eq!(area(&s, &p, &Mono, "p1", &ids(&["missing"])), None, "ids on no page, or another, picture nothing");
    }

    #[test]
    fn rows_are_in_get_order_with_page_coordinates() {
        let mut turned = node("d", Some("p1"), K::Ellipse, [500.0, 0.0, 80.0, 80.0]);
        turned.rotation = Some(30.0);
        let mut hid = node("c", Some("a"), K::Rect, [0.0; 4]);
        hid.hidden = Some(true);
        let (s, p) = page(vec![node("a", Some("p1"), K::Frame, [100.0, 50.0, 400.0, 300.0]), node("b", Some("a"), K::Rect, [10.125, 10.0, 50.0, 50.0]), hid, turned]);
        let b = boxes(&s, &p, "p1", &[], 200);
        assert_eq!(b.fields, FIELDS);
        assert_eq!(rows(&b), [
            r#"["a","frame","p1",100,50,400,300,100,50,0]"#,
            r#"["b","rect","a",110.13,60,50,50,10.13,10,0]"#,
            r#"["d","ellipse","p1",500,0,80,80,500,0,30]"#,
        ]);
        assert_eq!((b.total, b.hidden), (3, 1));
        let part = boxes(&s, &p, "p1", &ids(&["b", "a", "d"]), 200);
        assert_eq!(rows(&part).len(), 3, "the outermost and what is inside them, once each");
        assert_eq!(boxes(&s, &p, "p1", &ids(&["b"]), 200).rows[0][7], json!(10.13));
    }

    #[test]
    fn group_child_rx_ry_count_from_the_groups_point_not_its_bounds() {
        let (s, p) = page(vec![node("g", Some("p1"), K::Group, [100.0, 100.0, 0.0, 0.0]), node("c", Some("g"), K::Rect, [20.0, 30.0, 10.0, 10.0])]);
        let b = boxes(&s, &p, "p1", &[], 200);
        assert_eq!(rows(&b), [r#"["g","group","p1",120,130,10,10,120,130,0]"#, r#"["c","rect","g",120,130,10,10,20,30,0]"#]);
    }

    #[test]
    fn auto_layout_child_rx_ry_are_the_layouts_result() {
        let mut col = frame("col", "p1", [40.0, 40.0, 200.0, 0.0], Layout::Column);
        (col.padding, col.gap, col.h_sizing) = (Some(vec![16.0]), Some(8.0), Some(Sizing::Hug));
        let (s, p) = page(vec![col, node("one", Some("col"), K::Rect, [999.0, 999.0, 100.0, 20.0]), node("two", Some("col"), K::Rect, [0.0, 0.0, 100.0, 20.0])]);
        let b = boxes(&s, &p, "p1", &ids(&["col"]), 200);
        assert_eq!(rows(&b)[1..], [r#"["one","rect","col",56,56,100,20,16,16,0]"#.to_string(), r#"["two","rect","col",56,84,100,20,16,44,0]"#.into()]);
    }

    #[test]
    fn hugged_text_has_its_measured_width() {
        let (s, p) = page(vec![text("t", "p1", [10.0, 10.0, 0.0, 0.0], "Hello")]);
        assert_eq!(rows(&boxes(&s, &p, "p1", &[], 200)), [r#"["t","text","p1",10,10,40,20,10,10,0]"#]);
    }

    #[test]
    fn hidden_subtrees_are_counted_not_listed() {
        let mut f = node("f", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 100.0]);
        f.hidden = Some(true);
        let (s, p) = page(vec![f, node("in", Some("f"), K::Rect, [0.0; 4]), node("shown", Some("p1"), K::Rect, [0.0, 0.0, 5.0, 5.0])]);
        let b = boxes(&s, &p, "p1", &[], 200);
        assert_eq!((b.rows.len(), b.total, b.hidden), (1, 1, 2));
        assert_eq!(b.rows[0][0], json!("shown"));
        let only = boxes(&s, &p, "p1", &ids(&["f"]), 200);
        assert_eq!((only.rows.len(), only.total, only.hidden), (0, 0, 2));
    }

    #[test]
    fn rows_stop_at_the_limit_and_total_counts_all() {
        let (s, p) = page((0..7).map(|i| node(&format!("n{i}"), Some("p1"), K::Rect, [i as f64 * 10.0, 0.0, 5.0, 5.0])).collect());
        let b = boxes(&s, &p, "p1", &[], 5);
        assert_eq!((b.rows.len(), b.total), (5, 7));
        assert_eq!(more_boxes(&b).as_deref(), Some("2 more boxes: pass nodes: [...] to measure a part, or boxes_limit up to 500"));
        assert_eq!(more_boxes(&boxes(&s, &p, "p1", &[], 6)).as_deref(), Some("1 more box: pass nodes: [...] to measure a part, or boxes_limit up to 500"));
        let none = boxes(&s, &p, "p1", &[], 0);
        assert_eq!((none.rows.len(), none.total), (0, 7));
        assert_eq!(more_boxes(&boxes(&s, &p, "p1", &[], 7)), None);
    }


    #[test]
    fn fit_scale_honours_max_side_area_and_request() {
        let r = |w, h| Rect::new(0.0, 0.0, w, h);
        assert_eq!(fit_scale(&r(400.0, 300.0), 1.0, 1568, MAX_AREA_PX), 1.0);
        assert_eq!(fit_scale(&r(400.0, 300.0), 2.0, 1568, MAX_AREA_PX), 2.0);
        assert_eq!(fit_scale(&r(3136.0, 100.0), 1.0, 1568, MAX_AREA_PX), 0.5, "the longer side");
        assert_eq!(fit_scale(&r(4000.0, 4000.0), 2.0, 2560, MAX_AREA_PX), 0.5, "2000 x 2000 is 4 megapixels");
        for bad in [r(0.0, 100.0), r(f64::NAN, 10.0), r(10.0, f64::INFINITY)] {
            assert_eq!(fit_scale(&bad, 1.5, 1568, MAX_AREA_PX), 1.5);
        }
        assert_eq!(fit_scale(&r(1.0, 1.0), 2.0, 1568, MAX_AREA_PX), 2.0);
        assert_eq!(picture_size(&r(1.0, 1.0), 0.25), (1, 1), "a picture is at least a pixel");
        // Whatever the area, the whole-pixel picture keeps within both limits.
        for (w, h) in [(2001.0, 1999.5), (3001.7, 1333.3), (2560.5, 1.0), (1e6, 3.7), (7919.0, 7907.0), (8_388_608.0, 8_388_608.0)] {
            for (side, scale) in [(2560, 2.0), (1568, 1.0), (64, 0.25)] {
                let s = fit_scale(&r(w, h), scale, side, MAX_AREA_PX);
                let (pw, ph) = picture_size(&r(w, h), s);
                assert!(s > 0.0 && pw.max(ph) <= side && f64::from(pw) * f64::from(ph) <= MAX_AREA_PX, "{w} x {h} at {s}: {pw} x {ph}");
            }
        }
    }


    #[test]
    fn rows_read_as_numbers() {
        assert_eq!([num(1.0), num(-0.001), num(2.345), num(f64::NAN), num(-3.5)], [json!(1), json!(0), json!(2.35), json!(0), json!(-3.5)]);
        assert_eq!(render_rect(&Rect::new(1.0, 2.0, 3.0, 4.0)), RenderRect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 });
    }
}
