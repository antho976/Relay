//! Where every node of a page lands: auto layout, hugging text and stickies that grow to their
//! words. The canvas measures text (Pango); this decides everything else, so a frame an agent
//! wrote as `layout: column` lays out the same on every redraw.

use super::geom::Rect;
use super::scene::Scene;
use relay_bus::design::{follow_axis, Items, Justify, Layout, Node, NodeKind, Pin, Sizing, MAX_COORD};
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

/// How a frame is drawn against how it is stored, which is what its pinned children follow:
/// its stored `w`/`h` and its placed size. They differ when a layout fills, stretches or hugs
/// it, or a min or max bounds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub stored: (f64, f64),
    pub placed: (f64, f64),
}

/// The fit pinned children of `parent` follow: `None` for a page, a group (pins do not reach
/// through one) or what is not placed.
pub fn fit_of(scene: &Scene, placed: &Placed, parent: &str) -> Option<Fit> {
    let p = scene.node(parent).filter(|p| p.kind == NodeKind::Frame)?;
    let r = placed.rect(parent)?;
    Some(Fit { stored: (p.w, p.h), placed: (r.w, r.h) })
}

/// The stored `x`/`y`/`w`/`h` that put `n` at `rel` (its placed box less its parent's origin)
/// in a parent drawn as `fit`: where it sits while the parent is at its stored size. `rel`
/// itself with no fit, or for a node pinned left and top.
pub fn unpinned(n: &Node, fit: Option<Fit>, rel: Rect) -> Rect {
    let Some(fit) = fit else {
        return rel;
    };
    let (px, py) = n.pins();
    let (x, w) = follow_axis(px, rel.x, rel.w, fit.placed.0, fit.stored.0);
    let (y, h) = follow_axis(py, rel.y, rel.h, fit.placed.1, fit.stored.1);
    Rect::new(x, y, w, h)
}

/// How far `n`'s stored `x`/`y` move for it to show `dx`/`dy` further in a parent drawn as
/// `fit`: as far, except along a `scale` pin, which moves `stored / placed` as far.
pub fn stored_shift(n: &Node, fit: Option<Fit>, dx: f64, dy: f64) -> (f64, f64) {
    let Some(fit) = fit else {
        return (dx, dy);
    };
    let (px, py) = n.pins();
    let along = |pin: Pin, d: f64, stored: f64, placed: f64| {
        if pin == Pin::Scale && stored > 0.0 && placed > 0.0 {
            d * stored / placed
        } else {
            d
        }
    };
    (along(px, dx, fit.stored.0, fit.placed.0), along(py, dy, fit.stored.1, fit.placed.1))
}

/// Whether `id` is placed by its parent's auto layout, so its own `x`/`y` mean nothing: the
/// parent lays out its children and `id` is not `absolute`.
pub fn is_laid(scene: &Scene, id: &str) -> bool {
    scene.node(id).is_some_and(|n| !n.is_absolute())
        && scene.parent(id).and_then(|p| scene.node(p)).is_some_and(|p| p.layout_mode() != Layout::None)
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
        pass.free_child(child, (0.0, 0.0), None);
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

    /// The children `id` lays out: those shown and not `absolute`.
    fn flow(&self, id: &str) -> Vec<&'a str> {
        self.scene.children(id).iter().filter(|c| self.visible(c).is_some_and(|n| !n.is_absolute())).map(String::as_str).collect()
    }

    /// A node's own size, given the width a laid-out parent offers it.
    fn size(&self, id: &'a str, offered: Option<f64>) -> (f64, f64) {
        self.size_as(id, offered, false)
    }

    /// A node's size when a column stretches it `width` wide: as a filling width reads it,
    /// whatever its own width rule, keeping its height rule (B23).
    fn stretched(&self, id: &'a str, width: f64) -> (f64, f64) {
        self.size_as(id, Some(width), true)
    }

    /// A node's size, worked out once a pass.
    fn size_as(&self, id: &'a str, offered: Option<f64>, fill: bool) -> (f64, f64) {
        let Some(n) = self.scene.node(id) else {
            return (0.0, 0.0);
        };
        let ws = if fill { Sizing::Fill } else { w_sizing(n) };
        // Only a filling width reads what is offered: every other offer is the same question.
        let offered = offered.filter(|_| ws == Sizing::Fill);
        let key = (id, offered.map(f64::to_bits));
        if let Some(size) = self.sizes.borrow().get(&key) {
            return *size;
        }
        let size = self.size_of(n, ws, offered);
        self.sizes.borrow_mut().insert(key, size);
        size
    }

    /// A node's size under width rule `ws`, offered `offered` px across: its kind's rule, then
    /// its min and max.
    fn size_of(&self, n: &'a Node, ws: Sizing, offered: Option<f64>) -> (f64, f64) {
        let (w, h) = match n.kind {
            // A group is as big as its children, whatever its own `w`/`h` or sizing say (B2).
            NodeKind::Group => return self.group_bounds(n).map_or((0.0, 0.0), |b| (b.w, b.h)),
            NodeKind::Text => self.text_size(n, ws, offered),
            NodeKind::Sticky => self.sticky_size(n, ws, offered),
            NodeKind::Frame if n.layout_mode() != Layout::None => self.frame_size(n, ws, offered),
            // A filling width takes what is offered; nothing offers one outside a layout.
            _ => (if ws == Sizing::Fill { offered.unwrap_or(n.w) } else { n.w }, n.h),
        };
        (n.clamp_w(w).max(0.0), n.clamp_h(h).max(0.0))
    }

    /// A text's words measured wrapped at `wrap` px, or unwrapped.
    fn words(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
        self.measure.measure(n, wrap)
    }

    /// A text: its words wrapped at its width (unwrapped while it hugs them, up to `max_w`), as
    /// tall as its lines unless its height is its own.
    fn text_size(&self, n: &Node, ws: Sizing, offered: Option<f64>) -> (f64, f64) {
        let (w, lines) = match ws {
            Sizing::Hug => {
                let (mw, mh) = self.words(n, None);
                match n.max_w.filter(|max| mw > *max) {
                    // Past its widest it wraps there: a chat bubble's text.
                    Some(max) => (max, self.words(n, Some(max)).1),
                    None => (mw, mh),
                }
            }
            Sizing::Fixed => {
                let w = n.clamp_w(n.w);
                (w, self.words(n, Some(w)).1)
            }
            // A filling text that no width is offered yet (a row shares its room only after
            // measuring) measures at its own `w`, or unwrapped; the row wraps it at its share.
            Sizing::Fill => match offered.or((n.w > 0.0).then_some(n.w)).map(|w| n.clamp_w(w)) {
                Some(w) => (w, self.words(n, Some(w)).1),
                None => self.words(n, None),
            },
        };
        (w, if h_sizing(n) == Sizing::Hug { lines } else { n.h })
    }

    /// A sticky: as wide as it says (or as it fills, or its words while it hugs), and as tall
    /// as it says, growing to fit its words; hugging, exactly as tall as they are (B119).
    fn sticky_size(&self, n: &Node, ws: Sizing, offered: Option<f64>) -> (f64, f64) {
        let pad = 2.0 * STICKY_PAD;
        let own = if n.w > 0.0 { n.w } else { STICKY_SIZE };
        let w = n.clamp_w(match ws {
            Sizing::Fill => offered.unwrap_or(own),
            Sizing::Hug => self.words(n, None).0 + pad,
            Sizing::Fixed => own,
        });
        let words = self.words(n, Some((w - pad).max(1.0))).1 + pad;
        let h = if h_sizing(n) == Sizing::Hug { words } else { (if n.h > 0.0 { n.h } else { STICKY_SIZE }).max(words) };
        (w, h)
    }

    /// A laid-out frame: its width first (its own, what is offered, or what its children take
    /// while it hugs, within its min and max), then its children laid at that width, its
    /// height hugging what they take.
    fn frame_size(&self, n: &'a Node, ws: Sizing, offered: Option<f64>) -> (f64, f64) {
        let (hug_w, hug_h) = (ws == Sizing::Hug, h_sizing(n) == Sizing::Hug);
        let own = match ws {
            Sizing::Fixed => Some(n.w),
            Sizing::Fill => offered.or((n.w > 0.0).then_some(n.w)),
            Sizing::Hug => None,
        };
        if !hug_w && !hug_h {
            // A box of its own size: what is inside does not change it.
            return (own.unwrap_or(n.w), n.h);
        }
        let [pt, pr, pb, pl] = n.padding_edges();
        let inner = |w: f64| (w - pl - pr).max(0.0);
        let (content_w, mut content_h) = self.content(n, own.map(|w| inner(n.clamp_w(w))));
        let mut w = own.unwrap_or(n.w);
        if hug_w {
            let hugged = content_w + pl + pr;
            w = n.clamp_w(hugged);
            if w != hugged {
                // Past its min or max it is laid out again at the width it takes, so what wraps
                // or fills in it reflows there and its height follows.
                content_h = self.content(n, Some(inner(w))).1;
            }
        }
        (w, if hug_h { content_h + pt + pb } else { n.h })
    }

    /// What a laid-out frame's children take across and down when laid in `room` px of inner
    /// width (none while the frame hugs it).
    fn content(&self, n: &'a Node, room: Option<f64>) -> (f64, f64) {
        let kids = self.flow(&n.id);
        let row = n.layout_mode() == Layout::Row;
        let gap = n.gap.unwrap_or(0.0);
        let (main, cross) = if row { (room, None) } else { (None, room) };
        let slots = self.line_sizes(&kids, main, cross, row, n.items.unwrap_or(Items::Start), gap);
        let along = (slots.iter().map(|s| s.main).sum::<f64>() + gap * slots.len().saturating_sub(1) as f64).max(0.0);
        let across = slots.iter().map(|s| s.cross).fold(0.0, f64::max);
        if row {
            (along, across)
        } else {
            (across, along)
        }
    }

    /// One line of auto layout: each child's length along it and depth across. Filling
    /// children share what `main` leaves, each within its min and max; stretched ones take
    /// `cross`, measured at it across a column (B23); a group does neither (B2). Without
    /// `main` or `cross` (a frame that hugs), each takes its own.
    fn line_sizes(&self, kids: &[&'a str], main: Option<f64>, cross: Option<f64>, row: bool, items: Items, gap: f64) -> Vec<Slot> {
        let scene = self.scene;
        let mut slots: Vec<Slot> = kids
            .iter()
            .map(|&c| {
                let k = scene.node(c).expect("laid children exist");
                let (main_rule, cross_rule) = if row { (w_sizing(k), h_sizing(k)) } else { (h_sizing(k), w_sizing(k)) };
                let group = k.kind == NodeKind::Group;
                let stretch = !group && (cross_rule == Sizing::Fill || items == Items::Stretch);
                let (w, h) = match (row, cross) {
                    (true, _) => self.size(c, None),
                    (false, Some(room)) if stretch => self.stretched(c, room),
                    (false, room) => self.size(c, room),
                };
                let (along, mut across) = if row { (w, h) } else { (h, w) };
                if let (true, true, Some(room)) = (row, stretch, cross) {
                    across = k.clamp_h(room);
                }
                Slot { main: along, cross: across, fill: !group && main_rule == Sizing::Fill, stretch }
            })
            .collect();
        let fills: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].fill).collect();
        let Some(room) = main.filter(|_| !fills.is_empty()) else {
            return slots;
        };
        let taken: f64 = slots.iter().filter(|s| !s.fill).map(|s| s.main).sum();
        let gaps = gap * (slots.len() - 1) as f64;
        let bounds: Vec<(Option<f64>, Option<f64>)> = fills
            .iter()
            .map(|&i| scene.node(kids[i]).map_or((None, None), |k| if row { (k.min_w, k.max_w) } else { (k.min_h, k.max_h) }))
            .collect();
        for (&i, share) in fills.iter().zip(shares(room - taken - gaps, &bounds)) {
            slots[i].main = share;
            if row {
                // A filling text wraps at its share, a filling frame lays out at it: their
                // height follows, unless the row stretches them.
                let h = self.size(kids[i], Some(share)).1;
                if !(slots[i].stretch && cross.is_some()) {
                    slots[i].cross = h;
                }
            }
        }
        slots
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

    /// A child of a page, group or free frame, or an absolute one of a laid-out frame: at its
    /// own `x`/`y` from `origin` (a group: its children's bounds where they sit), carried by its
    /// pins when its frame is drawn at another size than it stores (`fit`).
    fn free_child(&mut self, id: &'a str, origin: (f64, f64), fit: Option<Fit>) {
        let Some(n) = self.visible(id) else {
            return;
        };
        let fit = fit.filter(|_| n.is_pinned());
        let (pin_x, pin_y) = n.pins();
        if n.kind == NodeKind::Group {
            // A group's size is its children's: a pin only moves it (validate keeps stretching
            // pins off groups).
            let b = self.group_bounds(n).unwrap_or_default();
            let (x, y) = fit.map_or((n.x, n.y), |f| {
                (follow_axis(pin_x, n.x, b.w, f.stored.0, f.placed.0).0, follow_axis(pin_y, n.y, b.h, f.stored.1, f.placed.1).0)
            });
            self.arrange(id, Rect::new(origin.0 + x + b.x, origin.1 + y + b.y, b.w, b.h));
            return;
        }
        let (mut w, mut h) = self.size(id, None);
        let (mut x, mut y) = (n.x, n.y);
        if let Some(f) = fit {
            let (fx, fw) = follow_axis(pin_x, x, w, f.stored.0, f.placed.0);
            x = fx;
            let fw = if pin_x.stretches() { n.clamp_w(fw).min(MAX_COORD) } else { w };
            if fw != w {
                // Stretched wider or narrower, its words or children reflow there and its
                // height follows.
                (w, h) = (fw, self.stretched(id, fw).1);
            }
            let (fy, fh) = follow_axis(pin_y, y, h, f.stored.1, f.placed.1);
            y = fy;
            if pin_y.stretches() {
                h = n.clamp_h(fh).min(MAX_COORD);
            }
        }
        self.arrange(id, Rect::new(origin.0 + x, origin.1 + y, w, h));
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
                    self.free_child(child, (ox, oy), None);
                }
                let bounds = Rect::bounds(children.iter().filter_map(|c| self.placed.rects.get(c)));
                self.placed.rects.insert(id.to_string(), bounds.unwrap_or(Rect::new(ox, oy, 0.0, 0.0)));
            }
            (_, Layout::None) => {
                self.placed.rects.insert(id.to_string(), rect);
                let fit = (n.kind == NodeKind::Frame).then_some(Fit { stored: (n.w, n.h), placed: (rect.w, rect.h) });
                for child in children {
                    self.free_child(child, (rect.x, rect.y), fit);
                }
            }
            _ => {
                self.placed.rects.insert(id.to_string(), rect);
                self.lay_out(n, rect);
                // Absolute children after the flow, from the frame's corner (not its padding),
                // in no one's way.
                let fit = Some(Fit { stored: (n.w, n.h), placed: (rect.w, rect.h) });
                for child in children.iter().filter(|c| scene.node(c).is_some_and(Node::is_absolute)) {
                    self.free_child(child, (rect.x, rect.y), fit);
                }
            }
        }
    }

    /// Auto layout: children one after another along the frame's direction, inside its
    /// padding.
    fn lay_out(&mut self, frame: &'a Node, rect: Rect) {
        let [pt, pr, pb, pl] = frame.padding_edges();
        let inner = Rect::new(rect.x + pl, rect.y + pt, (rect.w - pl - pr).max(0.0), (rect.h - pt - pb).max(0.0));
        let kids = self.flow(&frame.id);
        let row = frame.layout_mode() == Layout::Row;
        let (main, cross) = if row { (inner.w, inner.h) } else { (inner.h, inner.w) };
        let slots = self.line_sizes(&kids, Some(main), Some(cross), row, frame.items.unwrap_or(Items::Start), frame.gap.unwrap_or(0.0));
        self.place_line(frame, &kids, &slots, inner, row);
    }

    /// Places one line's children in `inner`: `justify` along it, `items` across.
    fn place_line(&mut self, frame: &Node, kids: &[&'a str], slots: &[Slot], inner: Rect, row: bool) {
        let (length, depth) = if row { (inner.w, inner.h) } else { (inner.h, inner.w) };
        let mut gap = frame.gap.unwrap_or(0.0);
        let free = length - slots.iter().map(|s| s.main).sum::<f64>() - gap * slots.len().saturating_sub(1) as f64;
        let mut at = match frame.justify.unwrap_or(Justify::Start) {
            Justify::Start => 0.0,
            Justify::Center => free / 2.0,
            Justify::End => free,
            Justify::SpaceBetween => {
                // What does not fit overflows from the start, as in CSS, rather than overlapping
                // (B114); a gap of its own still applies.
                if slots.len() > 1 && !slots.iter().any(|s| s.fill) {
                    gap += free.max(0.0) / (slots.len() - 1) as f64;
                }
                0.0
            }
        };
        let items = frame.items.unwrap_or(Items::Start);
        for (&c, s) in kids.iter().zip(slots) {
            let offset = match items {
                Items::Start | Items::Stretch => 0.0,
                Items::Center => (depth - s.cross) / 2.0,
                Items::End => depth - s.cross,
            };
            let r = if row {
                Rect::new(inner.x + at, inner.y + offset, s.main, s.cross)
            } else {
                Rect::new(inner.x + offset, inner.y + at, s.cross, s.main)
            };
            self.arrange(c, r);
            at += s.main + gap;
        }
    }
}

/// One child's place in a line of auto layout: its length along the line and depth across,
/// whether it fills along it and whether it stretches across.
#[derive(Debug, Clone, Copy)]
struct Slot {
    main: f64,
    cross: f64,
    fill: bool,
    stretch: bool,
}

/// Equal shares of `room` (none below 0) for filling children bounded by `bounds` (min, max).
/// A share a bound changes is kept at the bound and the others share what is left, the way CSS
/// resolves flexible lengths: when the bounds would add room in all, those held to a minimum are
/// kept first, when they would take it, those held to a maximum.
fn shares(room: f64, bounds: &[(Option<f64>, Option<f64>)]) -> Vec<f64> {
    let clamp = |v: f64, (lo, hi): (Option<f64>, Option<f64>)| {
        let v = hi.map_or(v, |hi| v.min(hi));
        lo.map_or(v, |lo| v.max(lo))
    };
    let mut kept: Vec<Option<f64>> = vec![None; bounds.len()];
    loop {
        let open: Vec<usize> = (0..bounds.len()).filter(|&i| kept[i].is_none()).collect();
        if open.is_empty() {
            break;
        }
        let each = ((room - kept.iter().flatten().sum::<f64>()) / open.len() as f64).max(0.0);
        let pull: f64 = open.iter().map(|&i| clamp(each, bounds[i]) - each).sum();
        for &i in &open {
            let bound = clamp(each, bounds[i]);
            if pull == 0.0 || (pull > 0.0 && bound > each) || (pull < 0.0 && bound < each) {
                kept[i] = Some(if pull == 0.0 { each } else { bound });
            }
        }
    }
    kept.into_iter().map(|s| s.unwrap_or(0.0)).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::design::scene::tests::node;
    use relay_bus::design::{NodeKind as K, PinX, PinY};

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

    /// B23: a stretched child was measured at its own width but laid out at the column's, so a
    /// fixed-height text reported its words' height and a stretched hugging frame its unwrapped
    /// height. Stretch is one rule now: filling the column's width, keeping its height rule.
    #[test]
    fn a_fixed_height_text_keeps_its_height_under_stretch() {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 0.0]);
        col.layout = Some(Layout::Column);
        col.h_sizing = Some(Sizing::Hug);
        col.items = Some(Items::Stretch);
        let mut t = text("t", "col", "Hi");
        t.h_sizing = Some(Sizing::Fixed);
        t.h = 40.0;
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), col, t]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("t"), Some(Rect::new(0.0, 0.0, 100.0, 40.0)));
        assert_eq!(p.rect("col").unwrap().h, 40.0);
    }

    #[test]
    fn a_stretched_hugging_frame_measures_at_the_column_width() {
        let mut outer = node("outer", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 0.0]);
        outer.layout = Some(Layout::Column);
        outer.h_sizing = Some(Sizing::Hug);
        outer.items = Some(Items::Stretch);
        let mut card = node("card", Some("outer"), K::Frame, [0.0; 4]);
        card.layout = Some(Layout::Column);
        card.h_sizing = Some(Sizing::Hug);
        let mut words = text("words", "card", "abcdefghijklmnopqrstuvwxyz0123"); // 240 px
        words.w_sizing = Some(Sizing::Fill);
        let mut caption = text("caption", "outer", "abcdefghijklmnopqrstuvwxyz0123"); // hugs: 240 px
        caption.hidden = Some(true);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), outer, card, words, caption]);
        let p = place(&s, "p1", &Mono);
        // 25 characters a line in 200 px: two lines.
        assert_eq!(p.rect("card"), Some(Rect::new(0.0, 0.0, 200.0, 40.0)));
        assert_eq!(p.rect("words"), Some(Rect::new(0.0, 0.0, 200.0, 40.0)));
        assert_eq!(p.rect("outer").unwrap().h, 40.0);
    }

    /// A hugging text a column stretches wraps at the column's width, and the column holds its
    /// lines.
    #[test]
    fn a_stretched_hugging_text_wraps_at_the_column_width() {
        let mut card = node("card", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 0.0]);
        card.layout = Some(Layout::Column);
        card.h_sizing = Some(Sizing::Hug);
        card.items = Some(Items::Stretch);
        card.padding = Some(vec![10.0]);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), card, text("t", "card", "abcdefghijklmnopqrstuvwxyz0123")]);
        let p = place(&s, "p1", &Mono);
        // 180 px holds 22 characters a line: two lines.
        assert_eq!(p.rect("t"), Some(Rect::new(10.0, 10.0, 180.0, 40.0)));
        assert_eq!(p.rect("card").unwrap().h, 60.0);
    }

    /// B114: space_between with children wider than the row overlapped them; they overflow
    /// from the start instead.
    #[test]
    fn space_between_overflows_instead_of_overlapping() {
        let mut bar = node("bar", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 10.0]);
        bar.layout = Some(Layout::Row);
        bar.justify = Some(Justify::SpaceBetween);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            bar,
            node("a", Some("bar"), K::Rect, [0.0, 0.0, 60.0, 10.0]),
            node("b", Some("bar"), K::Rect, [0.0, 0.0, 60.0, 10.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!((p.rect("a").unwrap().x, p.rect("b").unwrap().x), (0.0, 60.0));
    }

    /// B119: a sticky's height filling a column took its word height instead of its share, and
    /// hugging changed nothing; hugging is exactly its words now.
    #[test]
    fn a_filling_sticky_takes_its_share_of_a_column() {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 400.0]);
        col.layout = Some(Layout::Column);
        let mut sticky = node("s", Some("col"), K::Sticky, [0.0, 0.0, 200.0, 0.0]);
        sticky.text = Some("hi".into());
        sticky.h_sizing = Some(Sizing::Fill);
        let mut hugs = node("hug", Some("p1"), K::Sticky, [300.0, 0.0, 200.0, 0.0]);
        hugs.text = Some("hi".into());
        hugs.h_sizing = Some(Sizing::Hug);
        let mut wide = node("wide", Some("p1"), K::Sticky, [600.0, 0.0, 0.0, 0.0]);
        wide.text = Some("hello".into());
        wide.w_sizing = Some(Sizing::Hug);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            col,
            node("top", Some("col"), K::Rect, [0.0, 0.0, 200.0, 100.0]),
            sticky,
            hugs,
            wide,
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("s"), Some(Rect::new(0.0, 100.0, 200.0, 300.0)));
        assert_eq!(p.rect("hug").unwrap().h, 52.0);
        // Hugging its width: its words and the margins, as tall as it says or its words.
        assert_eq!(p.rect("wide"), Some(Rect::new(600.0, 0.0, 72.0, 200.0)));
    }

    /// Min and max bound every size layout works out: a hug, a fill share (the others share
    /// what a bounded one leaves), a stretch.
    #[test]
    fn min_and_max_bound_hug_and_fill() {
        let mut bar = node("bar", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 40.0]);
        bar.layout = Some(Layout::Row);
        bar.items = Some(Items::Stretch);
        let mut a = node("a", Some("bar"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        a.w_sizing = Some(Sizing::Fill);
        a.max_w = Some(50.0);
        a.max_h = Some(30.0);
        let mut b = node("b", Some("bar"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        b.w_sizing = Some(Sizing::Fill);
        let mut chip = node("chip", Some("p1"), K::Frame, [0.0, 100.0, 0.0, 0.0]);
        chip.layout = Some(Layout::Row);
        chip.w_sizing = Some(Sizing::Hug);
        chip.h_sizing = Some(Sizing::Hug);
        chip.min_w = Some(100.0);
        chip.max_h = Some(15.0);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            bar,
            a,
            b,
            chip,
            node("dot", Some("chip"), K::Ellipse, [0.0, 0.0, 30.0, 20.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("a"), Some(Rect::new(0.0, 0.0, 50.0, 30.0)));
        assert_eq!(p.rect("b"), Some(Rect::new(50.0, 0.0, 250.0, 40.0)));
        assert_eq!(p.rect("chip"), Some(Rect::new(0.0, 100.0, 100.0, 15.0)));
    }

    /// Fill shares resolve as CSS does: a bound that would add room in all keeps its minimum
    /// first, one that would take room keeps its maximum first.
    #[test]
    fn shares_keep_bounded_children_and_share_the_rest() {
        assert_eq!(shares(300.0, &[(None, Some(50.0)), (None, None)]), vec![50.0, 250.0]);
        assert_eq!(shares(100.0, &[(None, Some(30.0)), (Some(90.0), None)]), vec![10.0, 90.0]);
        assert_eq!(shares(100.0, &[(Some(80.0), None), (Some(80.0), None)]), vec![80.0, 80.0]);
        assert_eq!(shares(-20.0, &[(None, None), (Some(5.0), None)]), vec![0.0, 5.0]);
        assert_eq!(shares(90.0, &[(None, None); 3]), vec![30.0; 3]);
        assert!(shares(10.0, &[]).is_empty());
    }

    /// A hugging text past its max_w wraps there, and one short of its min_w is that wide.
    #[test]
    fn a_hugging_text_wraps_at_max_w() {
        let mut bubble = text("bubble", "p1", "abcdefghijklmnopqrstuvwxy"); // 200 px
        bubble.max_w = Some(100.0);
        let mut short = text("short", "p1", "Hi");
        short.min_w = Some(60.0);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), bubble, short]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("bubble"), Some(Rect::new(0.0, 0.0, 100.0, 60.0)));
        assert_eq!(p.rect("short"), Some(Rect::new(0.0, 0.0, 60.0, 20.0)));
    }

    /// A hugging frame past its max_w is laid out at it: a filling text in it wraps there, and
    /// the frame's height follows.
    #[test]
    fn a_hugging_frame_past_max_w_lays_out_at_max_w() {
        let mut card = node("card", Some("p1"), K::Frame, [0.0; 4]);
        card.layout = Some(Layout::Column);
        card.w_sizing = Some(Sizing::Hug);
        card.h_sizing = Some(Sizing::Hug);
        card.max_w = Some(120.0);
        let mut words = text("words", "card", "abcdefghijklmnopqrstuvwxy"); // 200 px
        words.w_sizing = Some(Sizing::Fill);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            card,
            node("bar", Some("card"), K::Rect, [0.0, 0.0, 200.0, 10.0]),
            words,
        ]);
        let p = place(&s, "p1", &Mono);
        // 120 px holds 15 characters a line: two lines.
        assert_eq!(p.rect("words"), Some(Rect::new(0.0, 10.0, 120.0, 40.0)));
        assert_eq!(p.rect("card"), Some(Rect::new(0.0, 0.0, 120.0, 50.0)));
    }

    /// A 400 px column holding a header that fills its width but stores `stored_w`, height 60,
    /// with `kids` in the header.
    fn header(stored_w: f64, kids: Vec<Node>) -> Scene {
        let mut screen = node("screen", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 800.0]);
        screen.layout = Some(Layout::Column);
        let mut head = node("head", Some("screen"), K::Frame, [0.0, 0.0, stored_w, 60.0]);
        head.w_sizing = Some(Sizing::Fill);
        let mut nodes = vec![node("p1", None, K::Page, [0.0; 4]), screen, head];
        nodes.extend(kids);
        Scene::new(nodes)
    }

    fn pinned(id: &str, rect: [f64; 4], pin: PinX) -> Node {
        let mut n = node(id, Some("head"), K::Rect, rect);
        n.pin_x = Some(pin);
        n
    }

    /// A header filling a 400 px screen but stored 300 wide: its right, centre, stretching
    /// and scaled children follow it; the others stay where they are.
    fn pinned_header() -> Scene {
        header(300.0, vec![
            pinned("right", [260.0, 10.0, 24.0, 24.0], PinX::Right),
            pinned("centre", [140.0, 10.0, 20.0, 20.0], PinX::Center),
            pinned("bar", [10.0, 50.0, 280.0, 4.0], PinX::LeftRight),
            pinned("half", [30.0, 0.0, 60.0, 10.0], PinX::Scale),
            node("left", Some("head"), K::Rect, [5.0, 5.0, 10.0, 10.0]),
        ])
    }

    #[test]
    fn a_filling_header_moves_its_right_and_centre_pins() {
        let s = pinned_header();
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("head"), Some(Rect::new(0.0, 0.0, 400.0, 60.0)));
        assert_eq!(p.rect("right"), Some(Rect::new(360.0, 10.0, 24.0, 24.0)));
        assert_eq!(p.rect("centre"), Some(Rect::new(190.0, 10.0, 20.0, 20.0)));
        assert_eq!(p.rect("bar"), Some(Rect::new(10.0, 50.0, 380.0, 4.0)));
        assert_eq!(p.rect("half"), Some(Rect::new(40.0, 0.0, 80.0, 10.0)));
        assert_eq!(p.rect("left"), Some(Rect::new(5.0, 5.0, 10.0, 10.0)));
    }

    /// Read back against the header's fit, each placed box is its stored one; a drag along a
    /// scale pin moves the stored `x` less than it shows.
    #[test]
    fn unpinned_reads_a_placed_box_back_to_stored() {
        let s = pinned_header();
        let p = place(&s, "p1", &Mono);
        let fit = fit_of(&s, &p, "head");
        assert_eq!(fit, Some(Fit { stored: (300.0, 60.0), placed: (400.0, 60.0) }));
        for id in ["right", "centre", "bar", "half", "left"] {
            let n = s.node(id).unwrap();
            assert_eq!(unpinned(n, fit, p.rect(id).unwrap()), Rect::new(n.x, n.y, n.w, n.h), "{id}");
        }
        assert_eq!(stored_shift(s.node("half").unwrap(), fit, 40.0, 8.0), (30.0, 8.0));
        assert_eq!(stored_shift(s.node("right").unwrap(), fit, 40.0, 8.0), (40.0, 8.0));
        assert_eq!((fit_of(&s, &p, "p1"), fit_of(&s, &p, "nope")), (None, None));
    }

    /// A parent with no stored width: pins count from 0, so a right pin's `x` is from the right
    /// edge and a scale does nothing.
    #[test]
    fn scale_from_a_parent_of_no_size_does_nothing() {
        let s = header(0.0, vec![
            pinned("half", [10.0, 0.0, 20.0, 10.0], PinX::Scale),
            pinned("close", [-40.0, 8.0, 24.0, 24.0], PinX::Right),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("half"), Some(Rect::new(10.0, 0.0, 20.0, 10.0)));
        assert_eq!(p.rect("close"), Some(Rect::new(360.0, 8.0, 24.0, 24.0)));
    }

    /// Pins act only in a frame: children of a page or a group stay at their `x`/`y`.
    #[test]
    fn pins_in_a_page_or_group_change_nothing() {
        let mut free = node("free", Some("p1"), K::Rect, [10.0, 10.0, 20.0, 20.0]);
        free.pin_x = Some(PinX::Right);
        let mut inside = node("inside", Some("g"), K::Rect, [5.0, 5.0, 20.0, 20.0]);
        inside.pin_y = Some(PinY::Bottom);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), free, node("g", Some("p1"), K::Group, [100.0, 0.0, 0.0, 0.0]), inside]);
        let p = place(&s, "p1", &Mono);
        assert_eq!((p.rect("free"), p.rect("inside")), (Some(Rect::new(10.0, 10.0, 20.0, 20.0)), Some(Rect::new(105.0, 5.0, 20.0, 20.0))));
    }

    /// An absolute child of a hugging column keeps its own place from the frame's corner and
    /// neither pushes its siblings nor grows the frame.
    #[test]
    fn an_absolute_child_keeps_its_place_and_takes_no_room() {
        let mut card = node("card", Some("p1"), K::Frame, [100.0, 50.0, 0.0, 0.0]);
        card.layout = Some(Layout::Column);
        card.gap = Some(10.0);
        card.padding = Some(vec![20.0]);
        card.w_sizing = Some(Sizing::Hug);
        card.h_sizing = Some(Sizing::Hug);
        let mut badge = node("badge", Some("card"), K::Ellipse, [150.0, -8.0, 16.0, 16.0]);
        badge.absolute = Some(true);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            card,
            text("t1", "card", "Title"),
            badge,
            node("r", Some("card"), K::Rect, [999.0, 999.0, 120.0, 30.0]),
        ]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("t1"), Some(Rect::new(120.0, 70.0, 40.0, 20.0)));
        assert_eq!(p.rect("r"), Some(Rect::new(120.0, 100.0, 120.0, 30.0)));
        assert_eq!(p.rect("card"), Some(Rect::new(100.0, 50.0, 160.0, 100.0)));
        assert_eq!(p.rect("badge"), Some(Rect::new(250.0, 42.0, 16.0, 16.0)));
    }

    /// Laid: in auto layout and not absolute. A group's and a page's children are not.
    #[test]
    fn is_laid_means_the_layout_places_it() {
        let mut row = node("row", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut badge = node("badge", Some("row"), K::Rect, [0.0; 4]);
        badge.absolute = Some(true);
        let s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            row,
            node("cell", Some("row"), K::Rect, [0.0; 4]),
            badge,
            node("g", Some("p1"), K::Group, [0.0; 4]),
            node("in", Some("g"), K::Rect, [0.0; 4]),
        ]);
        let laid: Vec<bool> = ["cell", "badge", "row", "in", "g", "p1", "nope"].iter().map(|id| is_laid(&s, id)).collect();
        assert_eq!(laid, [true, false, false, false, false, false, false]);
        assert_eq!(fit_of(&s, &place(&s, "p1", &Mono), "g"), None);
    }

    /// An absolute badge pinned right of a row that hugs its label, stored with no width: its
    /// `x` counts from the right edge however wide the label makes the row.
    #[test]
    fn an_absolute_badge_pinned_right_of_a_hugging_row() {
        let mut tab = node("tab", Some("p1"), K::Frame, [0.0, 0.0, 0.0, 40.0]);
        tab.layout = Some(Layout::Row);
        tab.w_sizing = Some(Sizing::Hug);
        tab.items = Some(Items::Center);
        tab.padding = Some(vec![0.0, 12.0]);
        let mut badge = node("badge", Some("tab"), K::Ellipse, [-20.0, -6.0, 16.0, 16.0]);
        badge.absolute = Some(true);
        badge.pin_x = Some(PinX::Right);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), tab, text("label", "tab", "Inbox"), badge]);
        let p = place(&s, "p1", &Mono);
        assert_eq!(p.rect("tab"), Some(Rect::new(0.0, 0.0, 64.0, 40.0)));
        assert_eq!(p.rect("label"), Some(Rect::new(12.0, 10.0, 40.0, 20.0)));
        assert_eq!(p.rect("badge"), Some(Rect::new(44.0, -6.0, 16.0, 16.0)));
    }

    /// A text pinned left and right in a panel drawn 100 px wider than it stores is 100 px
    /// wider, and its words rewrap there.
    #[test]
    fn a_left_right_text_rewraps_at_its_stretched_width() {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 400.0]);
        col.layout = Some(Layout::Column);
        let mut panel = node("panel", Some("col"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        panel.w_sizing = Some(Sizing::Fill);
        let mut words = text("words", "panel", "abcdefghijklmnopqrstuvwxyz0123"); // 240 px
        [words.x, words.y, words.w] = [10.0, 10.0, 180.0];
        words.pin_x = Some(PinX::LeftRight);
        let s = Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), col, panel, words]);
        let p = place(&s, "p1", &Mono);
        // Stored, 180 px wraps it on two lines; at 280 px it takes one.
        assert_eq!(p.rect("words"), Some(Rect::new(10.0, 10.0, 280.0, 20.0)));
        assert_eq!(wrap_width(s.node("words").unwrap(), &p.rect("words").unwrap()), Some(280.0));
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
