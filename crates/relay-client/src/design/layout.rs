//! Where every node of a page lands: auto layout, hugging text and stickies that grow to their
//! words. The canvas measures text (Pango); this decides everything else, so a frame an agent
//! wrote as `layout: column` lays out the same on every redraw.

use super::geom::Rect;
use super::scene::Scene;
use relay_bus::design::{Items, Justify, Layout, Node, NodeKind, Sizing};
use rustc_hash::FxHashMap;
use std::cell::RefCell;

/// Inner margin of a sticky's text.
pub const STICKY_PAD: f64 = 16.0;
/// Size a sticky starts at.
pub const STICKY_SIZE: f64 = 200.0;

/// Measures a text or sticky's words: their size when wrapped at `wrap` px (or unwrapped).
pub trait Measure {
    fn measure(&self, node: &Node, wrap: Option<f64>) -> (f64, f64);
}

/// The width rule a node follows when it names none: a text without a width hugs its words
/// (`Node::w_rule`, which the engine shares).
pub fn w_sizing(n: &Node) -> Sizing {
    n.w_rule()
}

/// The height rule a node follows when it names none: a text hugs its lines (`Node::h_rule`).
pub fn h_sizing(n: &Node) -> Sizing {
    n.h_rule()
}

/// Every placed node's box in page coordinates.
#[derive(Debug, Default, Clone)]
pub struct Placed {
    pub rects: FxHashMap<String, Rect>,
    /// Where a group's children count their `x`/`y` from: its parent's origin plus its own
    /// `x`/`y` (or where a layout put it). Its box in `rects` is its children's bounds instead.
    pub origins: FxHashMap<String, (f64, f64)>,
}

impl Placed {
    pub fn rect(&self, id: &str) -> Option<Rect> {
        self.rects.get(id).copied()
    }

    /// The page point `id`'s children count their `x`/`y` from, which is also where its own
    /// `x`/`y` put it: a group's own point (not its children's bounds), any other node's box
    /// corner, `(0, 0)` for what is not placed (a page).
    pub fn origin(&self, id: &str) -> (f64, f64) {
        self.origins.get(id).copied().or_else(|| self.rect(id).map(|r| (r.x, r.y))).unwrap_or((0.0, 0.0))
    }
}

/// Where a text's words wrap, given its placed box: `None` while it hugs them.
pub fn wrap_width(n: &Node, rect: &Rect) -> Option<f64> {
    match n.kind {
        NodeKind::Sticky => Some((rect.w - 2.0 * STICKY_PAD).max(1.0)),
        NodeKind::Text if w_sizing(n) != Sizing::Hug => Some(rect.w.max(1.0)),
        _ => None,
    }
}

/// A size asked of a pass: the node, and the width offered it (only a filling one reads it).
type SizeKey<'a> = (&'a str, Option<u64>);

/// One `place`: the scene, the measure, what is placed so far, and every size already worked
/// out. A size depends only on the node, the width offered it and the scene, none of which
/// change during a pass, so each is measured once however many ancestors ask (B45).
struct Pass<'a> {
    scene: &'a Scene,
    measure: &'a dyn Measure,
    placed: Placed,
    sizes: RefCell<FxHashMap<SizeKey<'a>, (f64, f64)>>,
    /// Each group's children's bounds, worked out once a pass.
    bounds: RefCell<FxHashMap<&'a str, Option<Rect>>>,
}

/// Place every node of `page`.
pub fn place(scene: &Scene, page: &str, measure: &dyn Measure) -> Placed {
    let mut pass = Pass::new(scene, measure);
    pass.placed.rects.insert(page.to_string(), Rect::default());
    for child in scene.children(page) {
        pass.free_child(child, 0.0, 0.0);
    }
    pass.placed
}

impl<'a> Pass<'a> {
    fn new(scene: &'a Scene, measure: &'a dyn Measure) -> Self {
        Pass { scene, measure, placed: Placed::default(), sizes: RefCell::default(), bounds: RefCell::default() }
    }

    fn visible(&self, id: &str) -> Option<&'a Node> {
        self.scene.node(id).filter(|n| !n.is_hidden())
    }

    /// A node's own size, given the width a laid-out parent offers it: worked out once a pass.
    fn size(&self, id: &'a str, offered_w: Option<f64>) -> (f64, f64) {
        let Some(n) = self.scene.node(id) else {
            return (0.0, 0.0);
        };
        // Only a filling width reads what is offered: every other offer is the same question.
        let offered_w = offered_w.filter(|_| w_sizing(n) == Sizing::Fill);
        let key = (id, offered_w.map(f64::to_bits));
        if let Some(size) = self.sizes.borrow().get(&key) {
            return *size;
        }
        let size = self.size_of(n, offered_w);
        self.sizes.borrow_mut().insert(key, size);
        size
    }

    fn size_of(&self, n: &'a Node, offered_w: Option<f64>) -> (f64, f64) {
        let (ws, hs) = (w_sizing(n), h_sizing(n));
        let fill_w = |own: f64| if ws == Sizing::Fill { offered_w.unwrap_or(own) } else { own };
        match n.kind {
            // A group is as big as its children, whatever its own `w`/`h` or sizing say (B2).
            NodeKind::Group => self.group_bounds(n).map_or((0.0, 0.0), |b| (b.w, b.h)),
            NodeKind::Text => {
                // A filling text that no width is offered yet (a row shares its room only after
                // measuring) measures unwrapped; the row wraps it once it has its share.
                let fill_wrap = offered_w.or((n.w > 0.0).then_some(n.w));
                let wrap = match ws {
                    Sizing::Hug => None,
                    Sizing::Fixed => Some(n.w),
                    Sizing::Fill => fill_wrap,
                };
                let (mw, mh) = self.measure.measure(n, wrap);
                let w = match ws {
                    Sizing::Hug => mw,
                    Sizing::Fixed => n.w,
                    Sizing::Fill => fill_wrap.unwrap_or(mw),
                };
                (w, if hs == Sizing::Hug { mh } else { n.h })
            }
            NodeKind::Sticky => {
                let w = fill_w(if n.w > 0.0 { n.w } else { STICKY_SIZE });
                let (_, mh) = self.measure.measure(n, Some((w - 2.0 * STICKY_PAD).max(1.0)));
                let h = if n.h > 0.0 { n.h } else { STICKY_SIZE };
                (w, h.max(mh + 2.0 * STICKY_PAD))
            }
            NodeKind::Frame if n.layout_mode() != Layout::None => {
                let [pt, pr, pb, pl] = n.padding_edges();
                let inner_w = match ws {
                    Sizing::Fixed => Some(n.w - pl - pr),
                    Sizing::Fill => offered_w.map(|w| w - pl - pr),
                    Sizing::Hug => None,
                };
                let row = n.layout_mode() == Layout::Row;
                let gap = n.gap.unwrap_or(0.0);
                let scene = self.scene;
                let visible: Vec<&'a str> =
                    scene.children(&n.id).iter().filter(|c| self.visible(c).is_some()).map(String::as_str).collect();
                let gaps = gap * visible.len().saturating_sub(1) as f64;
                let stretch = n.items == Some(Items::Stretch);
                let fills = |c: &str| {
                    scene.node(c).is_some_and(|k| {
                        k.kind != NodeKind::Group
                            && if row { w_sizing(k) == Sizing::Fill } else { w_sizing(k) == Sizing::Fill || (stretch && k.kind == NodeKind::Text) }
                    })
                };
                // A row with a known width shares what is left among its filling children before
                // measuring them, so a filling text is measured wrapped at its share.
                let share = match (row, inner_w) {
                    (true, Some(inner)) => {
                        let fixed: f64 = visible.iter().filter(|c| !fills(c)).map(|c| self.size(c, None).0).sum();
                        let count = visible.iter().filter(|c| fills(c)).count().max(1) as f64;
                        Some(((inner - fixed - gaps) / count).max(0.0))
                    }
                    _ => None,
                };
                let kids: Vec<(f64, f64)> = visible
                    .iter()
                    .map(|&c| match (row, fills(c)) {
                        (true, true) => self.size(c, share),
                        (true, false) => self.size(c, None),
                        (false, true) => {
                            let mut probe = self.size(c, inner_w);
                            if stretch && w_sizing(scene.node(c).expect("visible")) != Sizing::Fill {
                                probe = self.stretched(c, inner_w);
                            }
                            probe
                        }
                        (false, false) => self.size(c, inner_w),
                    })
                    .collect();
                let (content_w, content_h) = if row {
                    (kids.iter().map(|k| k.0).sum::<f64>() + gaps, kids.iter().map(|k| k.1).fold(0.0, f64::max))
                } else {
                    (kids.iter().map(|k| k.0).fold(0.0, f64::max), kids.iter().map(|k| k.1).sum::<f64>() + gaps)
                };
                let w = if ws == Sizing::Hug { content_w + pl + pr } else { fill_w(n.w) };
                let h = if hs == Sizing::Hug { content_h + pt + pb } else { n.h };
                (w.max(0.0), h.max(0.0))
            }
            _ => (fill_w(n.w), n.h),
        }
    }

    /// A text stretched across a column: measured wrapped at the column's width.
    fn stretched(&self, id: &'a str, inner_w: Option<f64>) -> (f64, f64) {
        let (Some(n), Some(w)) = (self.scene.node(id), inner_w) else {
            return self.size(id, inner_w);
        };
        let mut probe = n.clone();
        probe.w = w;
        probe.w_sizing = Some(Sizing::Fixed);
        (w, self.size_of_probe(&probe))
    }

    /// A group's children's bounds from its own point, shown children only (`None` with none
    /// shown): each child's box at its `x`/`y`, a child group's own bounds where it sits.
    fn group_bounds(&self, g: &'a Node) -> Option<Rect> {
        if let Some(bounds) = self.bounds.borrow().get(g.id.as_str()) {
            return *bounds;
        }
        let mut bounds: Option<Rect> = None;
        for c in self.scene.children(&g.id) {
            let Some(k) = self.visible(c) else {
                continue;
            };
            let r = if k.kind == NodeKind::Group {
                let Some(inner) = self.group_bounds(k) else {
                    continue;
                };
                inner.translate(k.x, k.y)
            } else {
                let (w, h) = self.size(c, None);
                Rect::new(k.x, k.y, w, h)
            };
            bounds = Some(bounds.map_or(r, |b| b.union(&r)));
        }
        self.bounds.borrow_mut().insert(g.id.as_str(), bounds);
        bounds
    }

    /// A child of a page, group or frame without layout: at its own `x`/`y` (a group: its
    /// children's bounds where they sit).
    fn free_child(&mut self, id: &'a str, origin_x: f64, origin_y: f64) {
        let Some(n) = self.visible(id) else {
            return;
        };
        let (x, y) = (origin_x + n.x, origin_y + n.y);
        let rect = if n.kind == NodeKind::Group {
            let b = self.group_bounds(n).unwrap_or_default();
            Rect::new(x + b.x, y + b.y, b.w, b.h)
        } else {
            let (w, h) = self.size(id, None);
            Rect::new(x, y, w, h)
        };
        self.arrange(id, rect);
    }

    fn arrange(&mut self, id: &'a str, rect: Rect) {
        let scene = self.scene;
        let Some(n) = scene.node(id) else {
            return;
        };
        let children = scene.children(id);
        match (n.kind, n.layout_mode()) {
            (NodeKind::Group, _) => {
                // `rect` is where its children's bounds land, freely or in a layout's slot (B2):
                // they count from there less where their bounds start.
                let b = self.group_bounds(n).unwrap_or_default();
                let (ox, oy) = (rect.x - b.x, rect.y - b.y);
                self.placed.origins.insert(id.to_string(), (ox, oy));
                for child in children {
                    self.free_child(child, ox, oy);
                }
                let bounds = Rect::bounds(children.iter().filter_map(|c| self.placed.rects.get(c)));
                self.placed.rects.insert(id.to_string(), bounds.unwrap_or(Rect::new(ox, oy, 0.0, 0.0)));
            }
            (_, Layout::None) => {
                self.placed.rects.insert(id.to_string(), rect);
                for child in children {
                    self.free_child(child, rect.x, rect.y);
                }
            }
            (_, mode) => {
                self.placed.rects.insert(id.to_string(), rect);
                self.lay_out(n, children, rect, mode == Layout::Row);
            }
        }
    }

    /// Auto layout: children one after another along the frame's direction.
    fn lay_out(&mut self, frame: &Node, children: &'a [String], rect: Rect, row: bool) {
        let [pt, pr, pb, pl] = frame.padding_edges();
        let inner = Rect::new(rect.x + pl, rect.y + pt, (rect.w - pl - pr).max(0.0), (rect.h - pt - pb).max(0.0));
        let (inner_main, inner_cross) = if row { (inner.w, inner.h) } else { (inner.h, inner.w) };
        let kids: Vec<&'a str> = children.iter().filter(|c| self.visible(c).is_some()).map(String::as_str).collect();
        if kids.is_empty() {
            return;
        }
        let items = frame.items.unwrap_or(Items::Start);
        let mut gap = frame.gap.unwrap_or(0.0);
        let mut sizes: Vec<(f64, f64, bool, bool)> = kids
            .iter()
            .map(|&c| {
                let n = self.scene.node(c).expect("visible");
                let (main_rule, cross_rule) = if row { (w_sizing(n), h_sizing(n)) } else { (h_sizing(n), w_sizing(n)) };
                // A group takes its children's room: it neither fills nor stretches (B2).
                let group = n.kind == NodeKind::Group;
                let stretch = !group && (cross_rule == Sizing::Fill || items == Items::Stretch);
                let (w, h) = self.size(c, if row { None } else { Some(inner.w) });
                let (main, cross) = if row { (w, h) } else { (h, w) };
                (main, if stretch { inner_cross } else { cross }, !group && main_rule == Sizing::Fill, stretch)
            })
            .collect();
        let fills = sizes.iter().filter(|s| s.2).count();
        let fixed: f64 = sizes.iter().filter(|s| !s.2).map(|s| s.0).sum();
        let gaps = gap * (kids.len() - 1) as f64;
        if fills > 0 {
            let share = ((inner_main - fixed - gaps) / fills as f64).max(0.0);
            for s in sizes.iter_mut().filter(|s| s.2) {
                s.0 = share;
            }
        }
        // A text whose width a row or a stretch changed wraps anew: its height follows.
        for (i, &c) in kids.iter().enumerate() {
            let n = self.scene.node(c).expect("visible");
            let sticky = n.kind == NodeKind::Sticky;
            if !sticky && n.kind != NodeKind::Text {
                continue;
            }
            let resized = if row { sizes[i].2 } else { sizes[i].3 };
            let wraps = sticky || w_sizing(n) != Sizing::Hug || resized;
            if wraps && (sticky || h_sizing(n) == Sizing::Hug) {
                let mut probe = n.clone();
                probe.w = if row { sizes[i].0 } else { sizes[i].1 };
                probe.w_sizing = Some(Sizing::Fixed);
                let h = self.size_of_probe(&probe);
                if !row {
                    sizes[i].0 = h;
                } else if !sizes[i].3 {
                    sizes[i].1 = h;
                }
            }
        }
        let used: f64 = sizes.iter().map(|s| s.0).sum::<f64>() + gaps;
        let free = inner_main - used;
        let mut at = match frame.justify.unwrap_or(Justify::Start) {
            Justify::Start => 0.0,
            Justify::Center => free / 2.0,
            Justify::End => free,
            Justify::SpaceBetween => {
                if kids.len() > 1 && fills == 0 {
                    gap += free / (kids.len() - 1) as f64;
                }
                0.0
            }
        };
        for (&c, (main, cross, _, _)) in kids.iter().zip(sizes) {
            let offset = match items {
                Items::Start | Items::Stretch => 0.0,
                Items::Center => (inner_cross - cross) / 2.0,
                Items::End => inner_cross - cross,
            };
            let r = if row {
                Rect::new(inner.x + at, inner.y + offset, main, cross)
            } else {
                Rect::new(inner.x + offset, inner.y + at, cross, main)
            };
            self.arrange(c, r);
            at += main + gap;
        }
    }

    /// The height a text or sticky takes at the probe's fixed width.
    fn size_of_probe(&self, probe: &Node) -> f64 {
        match probe.kind {
            NodeKind::Sticky => {
                let (_, mh) = self.measure.measure(probe, Some((probe.w - 2.0 * STICKY_PAD).max(1.0)));
                let h = if probe.h > 0.0 { probe.h } else { STICKY_SIZE };
                h.max(mh + 2.0 * STICKY_PAD)
            }
            _ => self.measure.measure(probe, Some(probe.w)).1,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::design::scene::tests::node;
    use relay_bus::design::NodeKind as K;

    /// 8 px per character, 20 px a line, wrapping whole characters.
    pub(crate) struct Mono;
    impl Measure for Mono {
        fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
            let chars = n.text.as_deref().unwrap_or("").chars().count() as f64;
            let natural = chars * 8.0;
            match wrap {
                Some(w) if natural > w && w >= 8.0 => {
                    let per_line = (w / 8.0).floor();
                    (per_line * 8.0, (chars / per_line).ceil() * 20.0)
                }
                _ => (natural, 20.0),
            }
        }
    }

    fn text(id: &str, parent: &str, words: &str) -> Node {
        let mut n = node(id, Some(parent), K::Text, [0.0; 4]);
        n.text = Some(words.into());
        n
    }

    #[test]
    fn free_children_sit_at_their_offsets() {
        let s = crate::design::scene::tests::sample();
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("b"), Some(Rect::new(10.0, 10.0, 50.0, 50.0)));
        // "Hi" hugs: 2 characters, one line.
        assert_eq!(p.rect("c"), Some(Rect::new(100.0, 10.0, 16.0, 20.0)));
    }

    #[test]
    fn a_column_hugs_and_stacks() {
        let mut card = node("card", Some("p1"), K::Frame, [100.0, 50.0, 0.0, 0.0]);
        card.layout = Some(Layout::Column);
        card.gap = Some(10.0);
        card.padding = Some(vec![20.0]);
        card.w_sizing = Some(Sizing::Hug);
        card.h_sizing = Some(Sizing::Hug);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            card,
            text("t1", "card", "Title"),
            node("r", Some("card"), K::Rect, [999.0, 999.0, 120.0, 30.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("t1"), Some(Rect::new(120.0, 70.0, 40.0, 20.0)));
        assert_eq!(p.rect("r"), Some(Rect::new(120.0, 100.0, 120.0, 30.0)));
        assert_eq!(p.rect("card"), Some(Rect::new(100.0, 50.0, 160.0, 100.0)));
    }

    #[test]
    fn a_row_shares_its_room_and_centres_across() {
        let mut bar = node("bar", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 50.0]);
        bar.layout = Some(Layout::Row);
        bar.items = Some(Items::Center);
        bar.gap = Some(10.0);
        let mut grow = node("grow", Some("bar"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        grow.w_sizing = Some(Sizing::Fill);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            bar,
            node("icon", Some("bar"), K::Rect, [0.0, 0.0, 30.0, 30.0]),
            grow,
            node("end", Some("bar"), K::Rect, [0.0, 0.0, 40.0, 20.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("icon"), Some(Rect::new(0.0, 10.0, 30.0, 30.0)));
        assert_eq!(p.rect("grow"), Some(Rect::new(40.0, 20.0, 210.0, 10.0)));
        assert_eq!(p.rect("end"), Some(Rect::new(260.0, 15.0, 40.0, 20.0)));
    }

    #[test]
    fn space_between_and_hidden_children() {
        let mut bar = node("bar", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 10.0]);
        bar.layout = Some(Layout::Row);
        bar.justify = Some(Justify::SpaceBetween);
        let mut gone = node("gone", Some("bar"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        gone.hidden = Some(true);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            bar,
            node("a", Some("bar"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            gone,
            node("b", Some("bar"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("a").unwrap().x, 0.0);
        assert_eq!(p.rect("b").unwrap().x, 90.0);
        assert!(p.rect("gone").is_none());
    }

    #[test]
    fn a_filling_text_wraps_to_its_column() {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 0.0]);
        col.layout = Some(Layout::Column);
        col.h_sizing = Some(Sizing::Hug);
        let mut para = text("para", "col", "abcdefghijklmnopqrstuvwxy"); // 25 chars = 200 px
        para.w_sizing = Some(Sizing::Fill);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), col, para]);
        let p = place(&s, "p1", &Mono);
        // 12 characters a line in 100 px: 3 lines.
        assert_eq!(p.rect("para"), Some(Rect::new(0.0, 0.0, 100.0, 60.0)));
        assert_eq!(p.rect("col").unwrap().h, 60.0);
    }

    #[test]
    fn a_filling_text_in_a_hugging_row_wraps_to_its_share() {
        let mut card = node("card", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 0.0]);
        card.layout = Some(Layout::Row);
        card.gap = Some(10.0);
        card.h_sizing = Some(Sizing::Hug);
        let mut words = text("words", "card", "abcdefghijklmnopqrstuvwxyz0123"); // 30 chars = 240 px
        words.w_sizing = Some(Sizing::Fill);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            card,
            node("avatar", Some("card"), K::Ellipse, [0.0, 0.0, 30.0, 30.0]),
            words,
        ]);
        let p = place(&s, "p1", &Mono);
        // 160 px left after the avatar and the gap: 20 characters a line, 2 lines.
        assert_eq!(p.rect("words"), Some(Rect::new(40.0, 0.0, 160.0, 40.0)));
        // The row knew its width, so it measured the text at its share: it holds both lines.
        assert_eq!(p.rect("card").unwrap().h, 40.0);
    }

    #[test]
    fn a_group_counts_its_children_from_its_own_point_not_their_bounds() {
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("g", Some("p1"), K::Group, [100.0, 100.0, 0.0, 0.0]),
            node("r", Some("g"), K::Rect, [50.0, 40.0, 10.0, 10.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("g"), Some(Rect::new(150.0, 140.0, 10.0, 10.0)));
        assert_eq!(p.origin("g"), (100.0, 100.0));
    }

    #[test]
    fn groups_bound_their_children_and_stickies_grow() {
        let mut long = node("s", Some("p1"), K::Sticky, [0.0, 0.0, 0.0, 0.0]);
        long.text = Some("x".repeat(200));
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("g", Some("p1"), K::Group, [100.0, 100.0, 0.0, 0.0]),
            node("g1", Some("g"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            node("g2", Some("g"), K::Rect, [20.0, 30.0, 10.0, 10.0]),
            long,
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("g"), Some(Rect::new(100.0, 100.0, 30.0, 40.0)));
        assert_eq!((p.origin("g"), p.origin("g2"), p.origin("p1")), ((100.0, 100.0), (120.0, 130.0), (0.0, 0.0)));
        // 168 px of text width holds 21 characters a line: 10 lines, 200 + 32 > 200.
        assert_eq!(p.rect("s"), Some(Rect::new(0.0, 0.0, 200.0, 232.0)));
    }

    /// A group the column lays out, holding r1 [0,0,50,30] and r2 [60,10,40,40] shifted by
    /// `(dx, dy)`, then a 80×20 rect.
    fn grouped(dx: f64, dy: f64) -> Scene {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 0.0]);
        col.layout = Some(Layout::Column);
        col.gap = Some(10.0);
        col.h_sizing = Some(Sizing::Hug);
        Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            col,
            node("g", Some("col"), K::Group, [999.0, 999.0, 0.0, 0.0]),
            node("r1", Some("g"), K::Rect, [dx, dy, 50.0, 30.0]),
            node("r2", Some("g"), K::Rect, [60.0 + dx, 10.0 + dy, 40.0, 40.0]),
            node("next", Some("col"), K::Rect, [0.0, 0.0, 80.0, 20.0]),
        ])
    }

    /// B2: a group in auto layout took no room (its stored 0×0), so the next sibling landed on
    /// its children; it takes their bounds.
    #[test]
    fn a_group_in_a_column_takes_its_bounds_and_pushes_the_next_sibling_down() {
        let p = place(&grouped(0.0, 0.0), "p1", &Mono);
        assert_eq!(p.rect("g"), Some(Rect::new(0.0, 0.0, 100.0, 50.0)));
        assert_eq!(p.rect("next").unwrap().y, 60.0);
        assert_eq!(p.rect("col").unwrap().h, 80.0);
        assert_eq!((p.origin("g"), p.rect("r2")), ((0.0, 0.0), Some(Rect::new(60.0, 10.0, 40.0, 40.0))));
    }

    /// B2, B90: children that start off the group's point (20 px in, or 10 px left of it)
    /// still land in its slot, the group's point moving so they do.
    #[test]
    fn a_group_whose_children_start_off_its_point_lands_in_its_slot() {
        let p = place(&grouped(20.0, 20.0), "p1", &Mono);
        assert_eq!(p.rect("g"), Some(Rect::new(0.0, 0.0, 100.0, 50.0)));
        assert_eq!((p.origin("g"), p.rect("r1").unwrap().x), ((-20.0, -20.0), 0.0));
        let p = place(&grouped(-10.0, 0.0), "p1", &Mono);
        assert_eq!(p.rect("g"), Some(Rect::new(0.0, 0.0, 100.0, 50.0)));
        assert_eq!(p.origin("g"), (10.0, 0.0));
        assert_eq!(p.rect("next").unwrap().y, 60.0);
    }

    /// A group neither fills a row nor stretches across it, whatever sizing it stores: it is as
    /// big as its children, in a group of its own too.
    #[test]
    fn a_group_neither_fills_nor_stretches() {
        let mut bar = node("bar", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        bar.layout = Some(Layout::Row);
        bar.items = Some(Items::Stretch);
        let mut g = node("g", Some("bar"), K::Group, [0.0, 0.0, 0.0, 0.0]);
        g.w_sizing = Some(Sizing::Fill);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            bar,
            g,
            node("inner", Some("g"), K::Group, [5.0, 5.0, 0.0, 0.0]),
            node("dot", Some("inner"), K::Ellipse, [10.0, 0.0, 20.0, 20.0]),
            node("after", Some("bar"), K::Rect, [0.0, 0.0, 30.0, 30.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("g"), Some(Rect::new(0.0, 0.0, 20.0, 20.0)));
        assert_eq!(p.rect("dot"), Some(Rect::new(0.0, 0.0, 20.0, 20.0)));
        assert_eq!(p.rect("after"), Some(Rect::new(20.0, 0.0, 30.0, 100.0)));
    }

    /// Mono, counting what it is asked.
    pub(crate) struct Counted(pub(crate) std::cell::Cell<usize>);
    impl Measure for Counted {
        fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
            self.0.set(self.0.get() + 1);
            Mono.measure(n, wrap)
        }
    }

    /// B45: each row of a chain measured its children twice, so a text 20 rows down was measured
    /// about 2^20 times; a pass now asks each size once.
    #[test]
    fn a_deep_chain_of_rows_measures_each_text_once() {
        let mut nodes = vec![node("p1", None, K::Page, [0.0; 4])];
        for i in 0..20 {
            let parent = if i == 0 { "p1".to_string() } else { format!("r{}", i - 1) };
            let mut row = node(&format!("r{i}"), Some(&parent), K::Frame, [0.0, 0.0, 1000.0, 50.0]);
            row.layout = Some(Layout::Row);
            row.w_sizing = Some(Sizing::Fixed);
            nodes.push(row);
        }
        nodes.push(text("leaf", "r19", "Hi"));
        let s = Scene::new(nodes);
        let counted = Counted(Default::default());
        let p = place(&s, "p1", &counted);
        assert!(counted.0.get() <= 2, "{} measures", counted.0.get());
        assert_eq!(p.rect("leaf"), Some(Rect::new(0.0, 0.0, 16.0, 20.0)));
        assert_eq!(p.rect("r19"), place(&s, "p1", &Mono).rect("r19"));
    }
}
