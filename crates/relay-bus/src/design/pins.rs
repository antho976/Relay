//! Pins, absolute children, min and max sizes, and scrolling frames (MODEL-SPEC §2.6, §2.9;
//! feature-layout §1).
//!
//! A child's stored `x`/`y`/`w`/`h` are its box while its parent is at the parent's *stored*
//! size; `pin_x`/`pin_y` say how the box follows when the parent is drawn at another size (fill,
//! stretch, hug: layout reads [`follow_axis`]) or resized by an edit (the engine and the canvas
//! write [`Node::follow`]). The same function serves both, so the canvas, the engine and an
//! agent reading `x` agree: `x` always counts from the parent's left at its stored width.

use super::{Layout, Node, NodeKind, PinX, PinY, Sizing, MAX_COORD};
use serde_json::{Map, Value};

/// One axis of a pin, either way round: `Start` is left or top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pin {
    Start,
    End,
    Both,
    Center,
    Scale,
}

impl Pin {
    /// Whether the box's length changes with its parent's.
    pub fn stretches(self) -> bool {
        matches!(self, Pin::Both | Pin::Scale)
    }
}

impl PinX {
    pub fn axis(self) -> Pin {
        match self {
            PinX::Left => Pin::Start,
            PinX::Right => Pin::End,
            PinX::LeftRight => Pin::Both,
            PinX::Center => Pin::Center,
            PinX::Scale => Pin::Scale,
        }
    }

    /// Its name as written: `left_right`.
    pub fn name(self) -> &'static str {
        match self {
            PinX::Left => "left",
            PinX::Right => "right",
            PinX::LeftRight => "left_right",
            PinX::Center => "center",
            PinX::Scale => "scale",
        }
    }
}

impl PinY {
    pub fn axis(self) -> Pin {
        match self {
            PinY::Top => Pin::Start,
            PinY::Bottom => Pin::End,
            PinY::TopBottom => Pin::Both,
            PinY::Center => Pin::Center,
            PinY::Scale => Pin::Scale,
        }
    }

    /// Its name as written: `top_bottom`.
    pub fn name(self) -> &'static str {
        match self {
            PinY::Top => "top",
            PinY::Bottom => "bottom",
            PinY::TopBottom => "top_bottom",
            PinY::Center => "center",
            PinY::Scale => "scale",
        }
    }
}

/// Where a box pinned by `pin` sits and how long it is along one axis when its parent is `to`
/// long, given its offset `pos` and length `len` while the parent is `from` long. Swapping `from`
/// and `to` undoes it, except a length clamped at 0 and `Scale` from a parent of length 0, which
/// stays put.
pub fn follow_axis(pin: Pin, pos: f64, len: f64, from: f64, to: f64) -> (f64, f64) {
    let d = to - from;
    match pin {
        Pin::Start => (pos, len),
        Pin::End => (pos + d, len),
        Pin::Center => (pos + d / 2.0, len),
        Pin::Both => (pos, (len + d).max(0.0)),
        Pin::Scale if from > 0.0 => (pos * to / from, len * to / from),
        Pin::Scale => (pos, len),
    }
}

/// Where `chain[0]`'s `x`/`y` put it on its page: `chain` runs from it up its parents to the page
/// (a page's origin is 0,0). Refused for a link placed at layout time: a child of a frame with
/// auto layout that is not `absolute`, or an absolute one pinned there (the frame may be drawn at
/// another size than it stores). `move` with `keep_position` reads it.
pub fn page_point(chain: &[&Node]) -> Result<(f64, f64), String> {
    let (mut x, mut y) = (0.0, 0.0);
    for (i, n) in chain.iter().enumerate() {
        if n.kind == NodeKind::Page {
            break;
        }
        if let Some(parent) = chain.get(i + 1).filter(|p| p.layout_mode() != Layout::None) {
            if !n.is_absolute() {
                return Err(format!("{:?} sits in {:?}'s auto layout, so where it is drawn is not stored", n.id, parent.id));
            }
            if n.is_pinned() {
                return Err(format!("{:?} is pinned in {:?}, which lays out its children, so where it is drawn is not stored", n.id, parent.id));
            }
        }
        (x, y) = (x + n.x, y + n.y);
    }
    Ok((x, y))
}

fn clamp(v: f64, lo: Option<f64>, hi: Option<f64>) -> f64 {
    // A minimum wins over a maximum, as in CSS (validate refuses one above the other).
    let v = hi.map_or(v, |hi| v.min(hi));
    lo.map_or(v, |lo| v.max(lo))
}

impl Node {
    /// Out of its frame's auto layout (`absolute: true`).
    pub fn is_absolute(&self) -> bool {
        self.absolute == Some(true)
    }

    /// Its pins across and down; `Start` when unset.
    pub fn pins(&self) -> (Pin, Pin) {
        (self.pin_x.map_or(Pin::Start, PinX::axis), self.pin_y.map_or(Pin::Start, PinY::axis))
    }

    /// Pinned on either axis to anything but the left or top.
    pub fn is_pinned(&self) -> bool {
        self.pins() != (Pin::Start, Pin::Start)
    }

    /// `w` within `min_w` and `max_w` (each optional).
    pub fn clamp_w(&self, w: f64) -> f64 {
        clamp(w, self.min_w, self.max_w)
    }

    /// `h` within `min_h` and `max_h` (each optional).
    pub fn clamp_h(&self, h: f64) -> f64 {
        clamp(h, self.min_h, self.max_h)
    }

    /// The fields this pinned child changes when its parent's stored size goes from `old` to
    /// `new` (only the axes asked for): `x` and `w`, `y` and `h`, as JSON numbers within what
    /// validate takes (±1000000, a size from 0 within its min and max). Empty when nothing moves.
    pub fn follow(&self, old: (f64, f64), new: (f64, f64), x: bool, y: bool) -> Map<String, Value> {
        let (pin_x, pin_y) = self.pins();
        let mut set = Map::new();
        let axes = [(x, pin_x, "x", "w", self.x, self.w, old.0, new.0), (y, pin_y, "y", "h", self.y, self.h, old.1, new.1)];
        for (asked, pin, pos_key, len_key, pos, len, from, to) in axes {
            if !asked || pin == Pin::Start {
                continue;
            }
            let (mut at, mut long) = follow_axis(pin, pos, len, from, to);
            if pin.stretches() {
                long = if len_key == "w" { self.clamp_w(long) } else { self.clamp_h(long) };
            }
            at = at.clamp(-MAX_COORD, MAX_COORD);
            long = long.clamp(0.0, MAX_COORD);
            if at != pos {
                set.insert(pos_key.into(), Value::from(at));
            }
            if long != len {
                set.insert(len_key.into(), Value::from(long));
            }
        }
        set
    }

    /// The layout value rules (part of [`Node::validate`]): pins, `absolute`, min and max sizes
    /// and `scroll`, each naming its field. Those that hang on the parent (pins on a laid child,
    /// `row_gap` outside a wrap) are inert, not refused.
    pub(super) fn check_layout(&self) -> Result<(), String> {
        let connector = self.kind == NodeKind::Line && (self.from.is_some() || self.to.is_some());
        let axes = [
            ("pin_x", self.pin_x.map(|p| (p.axis(), p.name())), "left, right", "width", "w", self.w, self.w_rule(), "w_sizing"),
            ("pin_y", self.pin_y.map(|p| (p.axis(), p.name())), "top, bottom", "height", "h", self.h, self.h_rule(), "h_sizing"),
        ];
        for (field, pin, ends, what, size_key, size, rule, sizing) in axes {
            let Some((pin, name)) = pin else { continue };
            if connector && pin != Pin::Start {
                return Err(format!("{field}: a connector follows the nodes it joins"));
            }
            if !pin.stretches() {
                continue;
            }
            match self.kind {
                NodeKind::Line | NodeKind::Path => {
                    return Err(format!("{field}: a line's points do not stretch; use {ends} or center"));
                }
                NodeKind::Group => return Err(format!("{field}: a group's size comes from its children; use {ends} or center")),
                NodeKind::Text if size <= 0.0 => {
                    return Err(format!("{field}: {name} stretches a fixed {what}; give the text a{} {size_key}", if size_key == "h" { "n" } else { "" }));
                }
                _ if rule != Sizing::Fixed => return Err(format!("{field}: {name} stretches a fixed {what}; set {sizing} to fixed")),
                _ => {}
            }
        }
        if self.is_absolute() {
            for (sizing, rule, pin) in [("w_sizing", self.w_sizing, "left_right"), ("h_sizing", self.h_sizing, "top_bottom")] {
                if rule == Some(Sizing::Fill) {
                    return Err(format!("{sizing}: an absolute child does not fill; pin it {pin} instead"));
                }
            }
        }
        let bounds = [
            ("min_w", "max_w", self.min_w, self.max_w, "w", self.w, self.w_rule(), "width"),
            ("min_h", "max_h", self.min_h, self.max_h, "h", self.h, self.h_rule(), "height"),
        ];
        for (lo_key, hi_key, lo, hi, size_key, size, rule, what) in bounds {
            if let (Some(lo), Some(hi)) = (lo, hi) {
                if lo > hi {
                    return Err(format!("{hi_key}: {hi} is below {lo_key} ({lo})"));
                }
            }
            // A size of 0 is one not given: layout clamps it.
            if (lo.is_some() || hi.is_some()) && rule == Sizing::Fixed && size > 0.0 {
                let (lo, hi) = (lo.unwrap_or(0.0), hi.unwrap_or(MAX_COORD));
                if size < lo || size > hi {
                    return Err(format!("{size_key}: {size} is outside {lo_key} to {hi_key} ({lo} to {hi}); a fixed {what} stays within them"));
                }
            }
        }
        if let Some(scroll) = self.scroll {
            if self.clip == Some(false) {
                return Err("clip: a scrolling frame always clips; clear scroll first".into());
            }
            for (scrolls, rule, what) in [(scroll.x(), self.w_rule(), "width"), (scroll.y(), self.h_rule(), "height")] {
                if scrolls && rule == Sizing::Hug {
                    return Err(format!("scroll: {} needs a fixed or fill {what}; a frame that hugs its {what} never overflows it", scroll.name()));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{NodeOp, Scroll};
    use serde_json::json;

    fn node(body: Value) -> Node {
        let mut body = body;
        if body.get("kind") != Some(&json!("page")) && body.get("parent").is_none() {
            body["parent"] = json!("p");
        }
        Node::read(body).unwrap()
    }

    fn refusal(body: Value) -> String {
        node(body).validate().unwrap_err()
    }

    /// Layout test list: what agents write reads, and round-trips; unset, none of it is written.
    #[test]
    fn pins_scroll_and_wrap_read_as_agents_write_them() {
        let body = json!({"parent": "p", "kind": "frame", "w": 300, "h": 200, "pin_x": "left_right", "pin_y": "bottom",
            "absolute": true, "min_w": 120, "max_w": 480, "scroll": "x", "layout": "wrap", "row_gap": 12});
        let n: Node = serde_json::from_value(body).unwrap();
        assert_eq!((n.pin_x, n.pin_y, n.absolute, n.min_w, n.max_w), (Some(PinX::LeftRight), Some(PinY::Bottom), Some(true), Some(120.0), Some(480.0)));
        assert_eq!((n.scroll, n.layout, n.row_gap), (Some(Scroll::X), Some(Layout::Wrap), Some(12.0)));
        assert_eq!(n.validate(), Ok(()));
        assert_eq!(n.check_new(), Ok(()));
        assert_eq!(serde_json::from_value::<Node>(serde_json::to_value(&n).unwrap()).unwrap(), n);
        let plain = serde_json::to_value(Node::new(NodeKind::Frame)).unwrap();
        for key in ["pin_x", "pin_y", "absolute", "min_w", "max_w", "min_h", "max_h", "row_gap", "scroll"] {
            assert!(plain.get(key).is_none(), "{key}");
        }
        let all = [json!("top"), json!("bottom"), json!("top_bottom"), json!("center"), json!("scale")];
        for (i, pin) in ["left", "right", "left_right", "center", "scale"].into_iter().enumerate() {
            let n = node(json!({"kind": "rect", "w": 10, "h": 10, "pin_x": pin, "pin_y": all[i]}));
            assert!(n.extra.is_empty() && n.validate().is_ok(), "{pin}");
        }
        assert_eq!((Scroll::Both.x(), Scroll::Both.y(), Scroll::X.y(), Scroll::Y.x()), (true, true, false, false));
    }

    /// Each pin refusal names its field.
    #[test]
    fn pin_rules_name_the_field() {
        let line = json!({"kind": "line", "points": [[0, 0], [9, 9]], "pin_x": "left_right"});
        assert_eq!(refusal(line), "pin_x: a line's points do not stretch; use left, right or center");
        assert_eq!(refusal(json!({"kind": "path", "points": [[0, 0]], "pin_y": "scale"})), "pin_y: a line's points do not stretch; use top, bottom or center");
        let link = json!({"kind": "line", "from": "a", "to": "b", "pin_x": "right"});
        assert_eq!(refusal(link), "pin_x: a connector follows the nodes it joins");
        assert_eq!(node(json!({"kind": "line", "from": "a", "to": "b", "pin_x": "left"})).validate(), Ok(()), "the default, written out");
        assert_eq!(refusal(json!({"kind": "group", "pin_x": "scale"})), "pin_x: a group's size comes from its children; use left, right or center");
        assert_eq!(refusal(json!({"kind": "text", "text": "Hi", "pin_x": "left_right"})), "pin_x: left_right stretches a fixed width; give the text a w");
        assert_eq!(refusal(json!({"kind": "text", "text": "Hi", "w": 100, "h": 20, "pin_y": "top_bottom"})),
            "pin_y: top_bottom stretches a fixed height; set h_sizing to fixed");
        assert_eq!(node(json!({"kind": "text", "text": "Hi", "w": 100, "h": 20, "h_sizing": "fixed", "pin_y": "top_bottom"})).validate(), Ok(()));
        assert_eq!(refusal(json!({"kind": "text", "text": "Hi", "w": 100, "h_sizing": "fixed", "pin_y": "top_bottom"})),
            "pin_y: top_bottom stretches a fixed height; give the text an h");
        assert_eq!(refusal(json!({"kind": "rect", "w_sizing": "fill", "pin_x": "left_right"})), "pin_x: left_right stretches a fixed width; set w_sizing to fixed");
        assert_eq!(refusal(json!({"kind": "rect", "w": 9, "absolute": true, "w_sizing": "fill"})), "w_sizing: an absolute child does not fill; pin it left_right instead");
        assert_eq!(refusal(json!({"kind": "rect", "w": 9, "absolute": true, "h_sizing": "fill"})), "h_sizing: an absolute child does not fill; pin it top_bottom instead");
        // A page sits in no parent: its pins are refused by kind, on what a caller writes.
        let page = node(json!({"kind": "page", "pin_x": "right"}));
        assert_eq!(page.validate(), Ok(()));
        assert_eq!(page.check_new().unwrap_err(), "pin_x: a page sits in no parent");
        // Pins that hang on the parent are inert, not refused: a child of a page, a laid child.
        assert_eq!(node(json!({"kind": "rect", "w": 9, "h": 9, "pin_x": "center", "pin_y": "scale"})).check_new(), Ok(()));
    }

    #[test]
    fn min_max_bound_and_keep_fixed_sizes_inside() {
        assert_eq!(refusal(json!({"kind": "rect", "min_w": 200, "max_w": 100})), "max_w: 100 is below min_w (200)");
        assert_eq!(refusal(json!({"kind": "rect", "min_h": 5, "max_h": 4})), "max_h: 4 is below min_h (5)");
        assert_eq!(refusal(json!({"kind": "rect", "w": 300, "max_w": 200})),
            "w: 300 is outside min_w to max_w (0 to 200); a fixed width stays within them");
        assert_eq!(refusal(json!({"kind": "rect", "h": 3, "min_h": 10})),
            "h: 3 is outside min_h to max_h (10 to 1000000); a fixed height stays within them");
        assert_eq!(refusal(json!({"kind": "rect", "min_w": -1})), "min_w: -1 is outside 0 to 1000000");
        // A size of 0 is not given; a hugging or filling one is decided by layout, then clamped.
        assert_eq!(node(json!({"kind": "rect", "w": 0, "min_w": 50})).validate(), Ok(()));
        assert_eq!(node(json!({"kind": "frame", "layout": "row", "w": 900, "w_sizing": "hug", "max_w": 400})).validate(), Ok(()));
        assert_eq!(node(json!({"kind": "text", "text": "Hi", "max_w": 200})).validate(), Ok(()));
        // A group's size comes from its children: refused by kind on what a caller writes.
        let group = node(json!({"kind": "group", "min_w": 10}));
        assert_eq!(group.validate(), Ok(()));
        assert!(group.check_new().unwrap_err().starts_with("min_w: group nodes do not use it"));
        // Clamps: a minimum wins over a maximum.
        let n = node(json!({"kind": "rect", "min_w": 50, "max_w": 80}));
        assert_eq!((n.clamp_w(10.0), n.clamp_w(60.0), n.clamp_w(100.0), n.clamp_h(7.0)), (50.0, 60.0, 80.0, 7.0));
    }

    #[test]
    fn scroll_rules() {
        let rect = node(json!({"kind": "rect", "w": 9, "h": 9, "scroll": "x"}));
        assert_eq!(rect.check_new().unwrap_err(), "scroll: only a frame scrolls");
        assert_eq!(refusal(json!({"kind": "frame", "w": 9, "h": 9, "scroll": "y", "clip": false})), "clip: a scrolling frame always clips; clear scroll first");
        assert_eq!(refusal(json!({"kind": "frame", "layout": "row", "w_sizing": "hug", "scroll": "x"})),
            "scroll: x needs a fixed or fill width; a frame that hugs its width never overflows it");
        assert_eq!(refusal(json!({"kind": "frame", "layout": "column", "w": 300, "h_sizing": "hug", "scroll": "both"})),
            "scroll: both needs a fixed or fill height; a frame that hugs its height never overflows it");
        assert_eq!(node(json!({"kind": "frame", "layout": "row", "w": 350, "h_sizing": "hug", "scroll": "x", "clip": true})).validate(), Ok(()));
        assert_eq!(node(json!({"kind": "frame", "w_sizing": "fill", "h": 80, "scroll": "x"})).validate(), Ok(()));
    }

    /// Each pin undoes itself by swapping the sizes, except a length clamped at 0 and a scale
    /// from nothing, which stay put.
    #[test]
    fn follow_axis_is_its_own_inverse() {
        for pin in [Pin::Start, Pin::End, Pin::Both, Pin::Center, Pin::Scale] {
            // Narrower by less than the box is long, so a stretch stays above 0.
            for (from, to) in [(300.0, 430.0), (430.0, 380.0), (100.0, 100.0), (390.0, 312.5)] {
                let (pos, len) = follow_axis(pin, 40.0, 80.0, from, to);
                let (back, long) = follow_axis(pin, pos, len, to, from);
                assert!((back - 40.0).abs() < 1e-9 && (long - 80.0).abs() < 1e-9, "{pin:?} {from} → {to}: {back}, {long}");
            }
        }
        assert_eq!(follow_axis(Pin::End, 300.0, 24.0, 390.0, 430.0), (340.0, 24.0));
        assert_eq!(follow_axis(Pin::Center, 175.0, 40.0, 390.0, 430.0), (195.0, 40.0));
        // A stretch past nothing stops at 0, and scaling from no size does nothing.
        assert_eq!(follow_axis(Pin::Both, 10.0, 30.0, 100.0, 20.0), (10.0, 0.0));
        assert_eq!(follow_axis(Pin::Both, 10.0, 0.0, 20.0, 100.0), (10.0, 80.0), "not back to 30");
        assert_eq!(follow_axis(Pin::Scale, 10.0, 30.0, 0.0, 200.0), (10.0, 30.0));
        assert!(Pin::Both.stretches() && Pin::Scale.stretches() && !Pin::End.stretches() && !Pin::Center.stretches());
    }

    #[test]
    fn follow_skips_unasked_axes_and_clamps() {
        let badge = node(json!({"kind": "rect", "x": 300, "y": 10, "w": 24, "h": 24, "pin_x": "right", "pin_y": "bottom"}));
        assert_eq!(Value::Object(badge.follow((390.0, 200.0), (430.0, 260.0), true, true)), json!({"x": 340.0, "y": 70.0}));
        assert_eq!(Value::Object(badge.follow((390.0, 200.0), (430.0, 260.0), true, false)), json!({"x": 340.0}));
        assert!(badge.follow((390.0, 200.0), (390.0, 200.0), true, true).is_empty(), "nothing moves");
        // A stretch stops at max_w; a scale may grow a fraction.
        let bar = node(json!({"kind": "rect", "x": 20, "w": 350, "h": 4, "pin_x": "left_right", "max_w": 360}));
        assert_eq!(Value::Object(bar.follow((390.0, 0.0), (430.0, 0.0), true, true)), json!({"w": 360.0}));
        let half = node(json!({"kind": "rect", "x": 100, "w": 100, "h": 4, "pin_x": "scale"}));
        assert_eq!(Value::Object(half.follow((400.0, 0.0), (300.0, 0.0), true, false)), json!({"x": 75.0, "w": 75.0}));
        // x stays within what validate takes; an unpinned node never moves.
        let far = node(json!({"kind": "rect", "x": 999_990, "w": 4, "h": 4, "pin_x": "right"}));
        assert_eq!(Value::Object(far.follow((10.0, 0.0), (500.0, 0.0), true, true)), json!({"x": MAX_COORD}));
        let still = node(json!({"kind": "rect", "x": 5, "w": 4, "h": 4}));
        assert!(still.follow((10.0, 10.0), (500.0, 500.0), true, true).is_empty() && !still.is_pinned());
        assert_eq!((badge.pins(), badge.is_pinned()), ((Pin::End, Pin::End), true));
    }

    #[test]
    fn page_point_counts_stored_offsets_and_refuses_auto_layout_links() {
        let page = node(json!({"id": "pg", "kind": "page"}));
        let frame = node(json!({"id": "f", "kind": "frame", "x": 100, "y": 50, "w": 400, "h": 300}));
        let group = node(json!({"id": "g", "parent": "f", "kind": "group", "x": 10, "y": 20}));
        let dot = node(json!({"id": "d", "parent": "g", "kind": "ellipse", "x": 5, "y": 6, "w": 4, "h": 4}));
        assert_eq!(page_point(&[&dot, &group, &frame, &page]), Ok((115.0, 76.0)));
        assert_eq!(page_point(&[&frame, &page]), Ok((100.0, 50.0)));
        assert_eq!(page_point(&[&page]), Ok((0.0, 0.0)));
        let mut row = frame.clone();
        row.layout = Some(Layout::Row);
        assert_eq!(page_point(&[&group, &row, &page]).unwrap_err(), "\"g\" sits in \"f\"'s auto layout, so where it is drawn is not stored");
        assert_eq!(page_point(&[&dot, &group, &row, &page]).unwrap_err(), "\"g\" sits in \"f\"'s auto layout, so where it is drawn is not stored");
        let mut badge = group.clone();
        badge.absolute = Some(true);
        assert_eq!(page_point(&[&badge, &row, &page]), Ok((110.0, 70.0)));
        badge.pin_x = Some(PinX::Right);
        assert!(page_point(&[&badge, &row, &page]).unwrap_err().starts_with("\"g\" is pinned in \"f\""));
    }

    /// B49: the default size of a shape added without one lies within its bounds.
    #[test]
    fn fill_default_size_keeps_within_min_and_max() {
        let mut wide = node(json!({"kind": "rect", "min_w": 160, "max_h": 40}));
        assert!(wide.fill_default_size());
        assert_eq!((wide.w, wide.h), (160.0, 40.0));
        assert_eq!(wide.validate(), Ok(()));
        let mut icon = node(json!({"kind": "icon", "icon": "home", "max_w": 16}));
        icon.fill_default_size();
        assert_eq!((icon.w, icon.h), (16.0, 24.0));
    }

    /// A move that keeps its place reads as agents write it.
    #[test]
    fn a_move_with_keep_position_reads() {
        let op: NodeOp = serde_json::from_value(json!({"op": "move", "id": "a", "parent": "b", "keep_position": true})).unwrap();
        assert!(matches!(op, NodeOp::Move { keep_position: Some(true), .. }));
        let plain = serde_json::to_value(NodeOp::Move { id: "a".into(), parent: None, index: None, keep_position: None }).unwrap();
        assert_eq!(plain, json!({"op": "move", "id": "a"}));
    }
}
