//! What `design.render` answers besides the picture (render-SPEC-v2 §7; MODEL-SPEC §1 #29,
//! agent-api W51): every node's laid-out box as a row, the page area the picture covers and the
//! scale that keeps it within its limits, and the problems an agent should hear about (a child
//! past its frame's padding, siblings laid over each other, texts drawn over texts, a child its
//! frame cuts off, and words too faint on what they sit on). GTK-free, so CI tests it. The
//! desktop app's render job calls it with the scene as drawn, the still placement
//! (`layout::place`) and the painter's text measure, and answers with the relay-bus types it
//! builds, so the contract has one definition.

use super::bounds::{painted_bounds, painted_bounds_in_page};
use super::color::{contrast, fill_of, ground_of, text_color, CANVAS};
use super::edit::outermost;
use super::geom::Rect;
use super::hit::on_shape;
use super::layout::{Measure, Placed, STICKY_PAD};
use super::motion::{posed_settled, Clocks};
use super::scene::Scene;
use super::shapes;
use relay_bus::design::{parse_color, Layout, Node, NodeKind, Shadow, Sizing, TextAlign};
use relay_bus::ops::design::{RenderBoxes, RenderIssue, RenderRect, RENDER_BOXES_MAX};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::{json, Value};

/// The columns of every box row, in order.
pub const FIELDS: [&str; 10] = ["id", "kind", "parent", "x", "y", "w", "h", "rx", "ry", "rot"];
/// Issues an answer lists; [`issues`] counts every one it finds.
pub const MAX_ISSUES: usize = 100;
/// Picture pixels in all (w × h), 4 megapixels; export keeps its own cap per side.
pub const MAX_AREA_PX: f64 = 4_000_000.0;
/// Room around a page's content in its picture.
pub const PAGE_MARGIN: f64 = 32.0;
/// How far past an edge, or how much shared area (px²), is rounding rather than a problem.
const SLACK: f64 = 0.5;
/// Contrast that words need (WCAG AA): large words, and the rest.
const LARGE_NEEDS: f64 = 3.0;
const TEXT_NEEDS: f64 = 4.5;

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

/// What [`issues`] finds, in the order it lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Code {
    Overflow,
    Overlap,
    TextOverlap,
    Clipped,
    Contrast,
}

impl Code {
    fn name(self) -> &'static str {
        match self {
            Code::Overflow => "overflow",
            Code::Overlap => "overlap",
            Code::TextOverlap => "text_overlap",
            Code::Clipped => "clipped",
            Code::Contrast => "contrast",
        }
    }
}

/// An issue with where its nodes paint, for the order.
struct Found {
    code: Code,
    at: (usize, usize),
    ids: Vec<String>,
    message: String,
}

/// The page as it paints, for the checks that compare nodes or look under one.
struct Paint<'a> {
    scene: &'a Scene,
    placed: &'a Placed,
    measure: &'a dyn Measure,
    /// Every node of the page, the page first, in paint order (`design.get`'s).
    order: Vec<String>,
    at: FxHashMap<String, usize>,
    /// Each node of `order`'s parent there (the page's own is the page).
    up: Vec<usize>,
    /// Each node's opacity, and how much of what it draws shows through its own opacity and its
    /// ancestors' (none inside a clear one), as the painter fades a node with what is inside it.
    opacity: Vec<f64>,
    seen: Vec<f64>,
    /// Whether each node of `order` is drawn through a turn, its own or an ancestor's.
    turned: Vec<bool>,
    /// The nodes in scope.
    wanted: FxHashSet<String>,
    grounds: Grounds,
}

/// The nodes a text can sit on (not a text, line, path, icon or group; placed, and neither clear
/// nor inside a clear one), by where they are drawn on the page, so the ground under a text is
/// looked for among those around its centre, not among everything painted before it.
#[derive(Default)]
struct Grounds {
    /// Each [`CELL`]-sized square of the page, with the grounds drawn over it, in paint order.
    cells: FxHashMap<(i64, i64), Vec<usize>>,
    /// Grounds over more than [`MAX_CELLS`] squares, in paint order.
    wide: Vec<usize>,
}

const CELL: f64 = 256.0;
const MAX_CELLS: i64 = 64;

impl Grounds {
    fn new(scene: &Scene, placed: &Placed, order: &[String], turned: &[bool], seen: &[f64]) -> Grounds {
        let mut grounds = Grounds::default();
        for (i, id) in order.iter().enumerate().skip(1) {
            if seen[i] <= 0.0 || scene.node(id).is_none_or(|n| matches!(n.kind, NodeKind::Text | NodeKind::Line | NodeKind::Path | NodeKind::Icon | NodeKind::Group)) {
                continue;
            }
            let drawn = if turned[i] { shapes::visual(scene, placed, id).map(|(r, turn)| shapes::rotated_bounds(&r, turn)) } else { placed.rect(id) };
            let Some(b) = drawn else { continue };
            let (x0, y0, x1, y1) = (cell(b.x), cell(b.y), cell(b.right()), cell(b.bottom()));
            if (x1 - x0 + 1).saturating_mul(y1 - y0 + 1) > MAX_CELLS {
                grounds.wide.push(i);
                continue;
            }
            for x in x0..=x1 {
                for y in y0..=y1 {
                    grounds.cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        grounds
    }

    /// The grounds that may be drawn over page point `p` and are painted before `from`, nearest
    /// (last painted) first.
    fn under(&self, (x, y): (f64, f64), from: usize) -> Vec<usize> {
        let near = self.cells.get(&(cell(x), cell(y))).map_or(&[][..], Vec::as_slice);
        let (near, wide) = (&near[..near.partition_point(|&i| i < from)], &self.wide[..self.wide.partition_point(|&i| i < from)]);
        let mut out: Vec<usize> = near.iter().chain(wide).copied().collect();
        out.sort_unstable_by(|a, b| b.cmp(a));
        out
    }
}

/// The square of the page that coordinate `v` falls in, along one axis.
fn cell(v: f64) -> i64 {
    (v / CELL).floor() as i64
}

/// The problems an agent should hear about among the visible nodes of [`scope`], worst kind
/// first (overflow, overlap, text_overlap, clipped, contrast), then in paint order; at most
/// [`MAX_ISSUES`], with the count of all. Rotated boxes are not where they paint, so the box
/// checks pass over a turned node.
///
/// - `overflow`: a child an auto-layout frame lays out whose box passes the frame's padding
///   box by more than half a px, except along the axis the frame scrolls.
/// - `overlap`: two children an auto-layout frame lays out that share more than half a px² of
///   their boxes; a frame whose `gap` (or a wrap's `row_gap`) is negative stacks them on purpose.
/// - `text_overlap`: two text or sticky siblings, under any parent, drawn over each other: a
///   text where its words are (and its box when it has a fill), a sticky its paper and words.
/// - `clipped`: a free child of a clipping frame (any child of one without layout, an absolute
///   one of one with layout) whose box passes the frame's box, except along its scroll axis; any
///   child, inside it, whose outer shadow still shows past an edge it stands clear of, or is
///   flush with where the frame only wraps it (`shadow_cut`: a glow squared off); and any child
///   of one that does not scroll that paints into the cut of a rounded corner it is not flush
///   with (`cut_by_corner`; full-bleed content is rounded with its frame on purpose).
/// - `contrast`: a text's or a labelled shape's words, or one of its runs in its own colour,
///   below 4.5:1 (3:1 at 24 px, or 18.66 px from weight 700) on what is drawn under their
///   centre. Both colours there are worked out as the painter lays them down: each node's
///   ground in paint order, a node with an opacity faded together with what is inside it, so
///   a faded card fades its words and its fill alike. The ground the message names is the
///   nearest node whose shape is painted there (the text itself when it paints a ground: an
///   ellipse's drawn slice, not a ring's hole; a box within its round corners) that is not a
///   text, line, path, icon or group and is at least half opaque beside the words, else the page. A
///   picture over that ground, or a colour over it that does not parse, leaves nothing to
///   check; under it, it is passed over. Nodes are read as drawn (`Scene::node`): a `$name`
///   is the colour it names, and one the design does not hold leaves the kind's default.
///
/// An issue names a node in scope first; the frame, the sibling or the ground it names next
/// may lie outside. None names first a node that sets `skip_checks`.
pub fn issues(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str, ids: &[String]) -> (Vec<RenderIssue>, u32) {
    let found = all_found(scene, placed, measure, page, ids);
    let count = u32::try_from(found.len()).unwrap_or(u32::MAX);
    let listed = found.into_iter().take(MAX_ISSUES).map(|f| RenderIssue { code: f.code.name().into(), ids: f.ids, message: f.message, at_ms: None });
    (listed.collect(), count)
}

/// Every issue of [`issues`], worst kind first, then in paint order.
fn all_found(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str, ids: &[String]) -> Vec<Found> {
    let (paint, inside) = Paint::new(scene, placed, measure, page, ids);
    let mut found = Vec::new();
    for parent in &paint.order {
        if paint.scene.children(parent).iter().any(|c| paint.wanted.contains(c)) {
            paint.under(parent, &mut found);
        }
    }
    found.extend(inside.iter().filter_map(|id| paint.contrast(id)));
    found.retain(|f| f.ids.first().and_then(|id| scene.node(id)).is_none_or(|n| n.skip_checks != Some(true)));
    found.sort_by_key(|f| (f.code, f.at));
    found
}

/// What motion brings that the still does not have: the page posed at each of `times` and checked
/// as [`issues`] checks it, each problem the still lacks listed once, at the first time it shows
/// (`at_ms`), its message saying at how many of the times it does; in the order they first show,
/// then worst kind first; at most [`MAX_ISSUES`], with the count of all. Agents read a contact
/// sheet's first frame alone, so this is how they hear of a label a slide-in covers or a fade that
/// leaves words faint.
///
/// What is on its way is not a problem where it passes, only where it settles:
/// - a fade still under way is checked at its node's resting opacity (`motion::posed_settled`);
///   one whose clock has stopped (it does not loop, and its end has come) is checked as it holds,
///   and so is every fade when `held` (the times are where the motion ends);
/// - `overflow` and `overlap` are the still's alone: motion moves a box after layout, so a child
///   sliding inside a frame that hugs it has not outgrown the frame.
#[allow(clippy::too_many_arguments)] // the still's inputs, then the motion's
pub fn motion_issues(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str, ids: &[String], clocks: &Clocks, times: &[f64], held: bool) -> (Vec<RenderIssue>, u32) {
    if clocks.is_empty() || times.is_empty() {
        return (Vec::new(), 0);
    }
    let key = |f: &Found| (f.code, f.ids.clone());
    let still: FxHashSet<(Code, Vec<String>)> = all_found(scene, placed, measure, page, ids).iter().map(key).collect();
    // Each new problem: when it first shows, its first message, and at how many times it shows.
    let mut seen: FxHashMap<(Code, Vec<String>), usize> = FxHashMap::default();
    let mut moving: Vec<(f64, Found, usize)> = Vec::new();
    for &t in times {
        let (posed, moved) = posed_settled(scene, placed, clocks, t, held);
        for f in all_found(&posed, &moved, measure, page, ids) {
            let k = key(&f);
            if matches!(f.code, Code::Overflow | Code::Overlap) || still.contains(&k) {
                continue;
            }
            match seen.get(&k) {
                Some(&i) => moving[i].2 += 1,
                None => {
                    seen.insert(k, moving.len());
                    moving.push((t, f, 1));
                }
            }
        }
    }
    moving.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.code.cmp(&b.1.code)));
    let count = u32::try_from(moving.len()).unwrap_or(u32::MAX);
    let n = times.len();
    let listed = moving.into_iter().take(MAX_ISSUES).map(|(t, f, shows)| {
        let when = if n == 1 { format!(" (at {} ms of its motion)", fmt(t)) } else { format!(" (from {} ms of its motion; at {shows} of the {n} times pictured)", fmt(t)) };
        RenderIssue { code: f.code.name().into(), ids: f.ids, message: format!("{}{when}", f.message), at_ms: Some(t) }
    });
    (listed.collect(), count)
}

/// Every `contrast` issue of [`issues`] on `page`, uncapped and in paint order: the one check
/// design.apply's contrast flags run.
pub fn contrast_issues(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str) -> Vec<RenderIssue> {
    let (paint, inside) = Paint::new(scene, placed, measure, page, &[]);
    inside.iter()
        .filter_map(|id| paint.contrast(id))
        .filter(|f| f.ids.first().and_then(|id| scene.node(id)).is_none_or(|n| n.skip_checks != Some(true)))
        .map(|f| RenderIssue { code: f.code.name().into(), ids: f.ids, message: f.message, at_ms: None })
        .collect()
}

impl<'s> Paint<'s> {
    /// The page as the painter lays it down, for the checks of [`scope`]'s nodes, which it
    /// answers with.
    fn new(scene: &'s Scene, placed: &'s Placed, measure: &'s dyn Measure, page: &str, ids: &[String]) -> (Paint<'s>, Vec<String>) {
        let order = scene.subtree(page);
        let at: FxHashMap<String, usize> = order.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
        let n = order.len();
        let (mut up, mut opacity, mut seen, mut turned) = (vec![0; n], vec![1.0; n], vec![1.0; n], vec![false; n]);
        // Parents come before their children, so each is done before its children read it.
        for (i, id) in order.iter().enumerate() {
            let node = scene.node(id);
            let parent = scene.parent(id).and_then(|p| at.get(p)).copied().filter(|&p| p < i);
            up[i] = parent.unwrap_or(0);
            opacity[i] = node.and_then(|n| n.opacity).unwrap_or(1.0).clamp(0.0, 1.0);
            seen[i] = opacity[i] * parent.map_or(1.0, |p| seen[p]);
            turned[i] = node.is_some_and(|n| shapes::rotation(n) != 0.0) || parent.is_some_and(|p| turned[p]);
        }
        let inside = scope(scene, page, ids);
        let wanted = inside.iter().cloned().collect();
        let grounds = Grounds::new(scene, placed, &order, &turned, &seen);
        (Paint { scene, placed, measure, order, at, up, opacity, seen, turned, wanted, grounds }, inside)
    }
}

impl Paint<'_> {
    fn at(&self, id: &str) -> usize {
        self.at.get(id).copied().unwrap_or(usize::MAX)
    }

    /// `id` and its box when it is drawn where its box is: visible and not turned.
    fn level(&self, id: &str) -> Option<(&Node, Rect)> {
        let n = self.scene.node(id)?;
        let r = self.placed.rect(id)?;
        (shapes::rotation(n).rem_euclid(180.0) == 0.0).then_some((n, r))
    }

    /// Whether `id` or an ancestor is wholly clear, so nothing of it is seen.
    fn clear(&self, id: &str) -> bool {
        self.at.get(id).is_none_or(|&i| self.seen[i] <= 0.0)
    }

    /// Node `i` of `order` and its ancestors, the page first.
    fn chain(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        let mut at = i;
        while at != 0 {
            at = self.up[at];
            out.push(at);
        }
        out.reverse();
        out
    }

    /// How much node `i`'s own opacity and its ancestors' fade it beside a node whose `chain`
    /// is `mine`: a layer both are inside fades them alike, so it is left out.
    fn apart(&self, mut i: usize, mine: &[usize]) -> f64 {
        let mut o = 1.0;
        while !mine.contains(&i) {
            o *= self.opacity[i];
            i = self.up[i];
        }
        o
    }

    /// The issues among `parent`'s children.
    fn under(&self, parent: &str, found: &mut Vec<Found>) {
        let (Some(p), Some(pr)) = (self.scene.node(parent), self.placed.rect(parent)) else {
            return;
        };
        let kids = self.scene.children(parent);
        let mode = p.layout_mode();
        let (across, down) = p.scroll.map_or((false, false), |s| (s.x(), s.y()));
        if mode != Layout::None {
            let flow: Vec<(&Node, Rect)> = kids.iter().filter_map(|c| self.level(c)).filter(|(n, _)| !n.is_absolute()).collect();
            let [t, r, b, l] = p.padding_edges();
            let padded = [pr.x + l, pr.right() - r, pr.y + t, pr.bottom() - b];
            for (n, nr) in flow.iter().filter(|(n, _)| self.wanted.contains(&n.id)) {
                if let Some((side, past, edge, limit)) = passes(nr, padded, across, down) {
                    let message = format!("{} extends {} px past {parent}'s padding ({side} {}, limit {})", n.id, fmt(past), fmt(edge), fmt(limit));
                    found.push(self.found(Code::Overflow, &n.id, parent, message));
                }
            }
            let gap = p.gap.unwrap_or(0.0);
            if gap >= 0.0 && !(mode == Layout::Wrap && p.row_gap.unwrap_or(gap) < 0.0) {
                let name = match mode {
                    Layout::Row => "row",
                    Layout::Column => "column",
                    _ => "wrap",
                };
                for (a, b, shared) in self.pairs(&flow) {
                    let message = format!("{} and {} overlap {} × {} px in {parent}'s {name} layout", a.id, b.id, fmt(shared.w), fmt(shared.h));
                    found.push(self.found(Code::Overlap, &a.id, &b.id, message));
                }
            }
        }
        if p.kind == NodeKind::Frame && p.clip != Some(false) {
            let edges = [pr.x, pr.right(), pr.y, pr.bottom()];
            let corners = shapes::corner_radii(p, &pr);
            let inside = kids.iter().filter(|c| self.wanted.contains(*c) && !self.clear(c)).filter_map(|c| self.level(c));
            for (n, nr) in inside {
                let free = mode == Layout::None || n.is_absolute();
                if let Some((side, past, edge, limit)) = passes(&nr, edges, across, down).filter(|_| free) {
                    let message = format!("{} is cut off {} px past {parent}'s edge, which clips ({side} {}, limit {})", n.id, fmt(past), fmt(edge), fmt(limit));
                    found.push(self.found(Code::Clipped, &n.id, parent, message));
                    continue;
                }
                // Its box inside, its shadow squared off by an edge it stands clear of.
                if let Some((side, past, gap)) = shadow_cut(n, &nr, p, edges, across, down) {
                    let room = if gap > SLACK { format!("{} px from {}", fmt(gap), n.id) } else { format!("flush with {}, which {parent} wraps", n.id) };
                    let message = format!("{}'s shadow is cut off by {parent}'s {side} edge, which clips: it shows {} px past it, {room}; give it room or set clip false on {parent}", n.id, fmt(past));
                    found.push(self.found(Code::Clipped, &n.id, parent, message));
                    continue;
                }
                // A corner of what it paints in the rounded corner's cut, of a frame that does not scroll.
                if !(across || down) {
                    if let Some((cut, depth, radius)) = cut_by_corner(n, &nr, &pr, corners) {
                        let which = match cut.as_slice() {
                            [one] => format!("rounded {one} corner"),
                            [rest @ .., last] => format!("rounded {} and {last} corners", rest.join(", ")),
                            [] => continue,
                        };
                        let message = format!("{} is cut {} px by {parent}'s {which} (radius {}), which clips: move it in or round its own corner", n.id, fmt(depth), fmt(radius));
                        found.push(self.found(Code::Clipped, &n.id, parent, message));
                    }
                }
            }
        }
        let texts: Vec<(&Node, Rect)> = kids.iter().filter(|c| !self.clear(c)).filter_map(|c| self.level(c)).filter_map(|(n, r)| Some((n, self.words_drawn(n, &r)?))).collect();
        for (a, b, shared) in self.pairs(&texts) {
            let message = format!("{} and {} are drawn over each other ({} × {} px): text over text is hard to read", a.id, b.id, fmt(shared.w), fmt(shared.h));
            found.push(self.found(Code::TextOverlap, &a.id, &b.id, message));
        }
    }

    fn found(&self, code: Code, first: &str, second: &str, message: String) -> Found {
        Found { code, at: (self.at(first), self.at(second)), ids: vec![first.to_string(), second.to_string()], message }
    }

    /// Each two of `boxes` that share more than [`SLACK`] px² and at least one of which is in
    /// scope, the one painted first first, with what they share.
    fn pairs<'n>(&self, boxes: &[(&'n Node, Rect)]) -> Vec<(&'n Node, &'n Node, Rect)> {
        let mut by_x: Vec<usize> = (0..boxes.len()).collect();
        by_x.sort_by(|&a, &b| boxes[a].1.x.total_cmp(&boxes[b].1.x));
        let mut out = Vec::new();
        for (k, &i) in by_x.iter().enumerate() {
            let (a, ar) = boxes[i];
            for &j in by_x[k + 1..].iter().take_while(|&&j| boxes[j].1.x < ar.right()) {
                let (b, br) = boxes[j];
                let shared = Rect::spanning((ar.x.max(br.x), ar.y.max(br.y)), (ar.right().min(br.right()), ar.bottom().min(br.bottom())));
                let apart = ar.right() <= br.x || br.right() <= ar.x || ar.bottom() <= br.y || br.bottom() <= ar.y;
                if apart || shared.w * shared.h <= SLACK || !(self.wanted.contains(&a.id) || self.wanted.contains(&b.id)) {
                    continue;
                }
                out.push(if self.at(&a.id) <= self.at(&b.id) { (a, b, shared) } else { (b, a, shared) });
            }
        }
        out
    }

    /// Where a text or sticky is drawn, in its parent's coordinates, for `text_overlap`: a
    /// text's words (with its box when it paints a fill), a sticky's paper and words. `None`
    /// for any other node and a text that draws nothing.
    fn words_drawn(&self, n: &Node, r: &Rect) -> Option<Rect> {
        let has_words = n.text.as_deref().is_some_and(|t| !t.trim().is_empty());
        let words = |x: f64, y: f64| {
            let wrap = self.placed.wrap_width(n, r);
            let (mw, mh) = self.measure.measure(n, wrap);
            let room = wrap.unwrap_or(mw);
            let x = x + match n.align.unwrap_or(TextAlign::Left) {
                TextAlign::Left => 0.0,
                TextAlign::Center => (room - mw) / 2.0,
                TextAlign::Right => room - mw,
            };
            Rect::new(x, y, mw, mh)
        };
        match n.kind {
            NodeKind::Text => {
                let words = has_words.then(|| words(r.x, r.y));
                let fill = n.fill.is_some().then_some(*r);
                words.into_iter().chain(fill).reduce(|a, b| a.union(&b))
            }
            NodeKind::Sticky if has_words => Some(r.union(&words(r.x + STICKY_PAD, r.y + STICKY_PAD))),
            NodeKind::Sticky => Some(*r),
            _ => None,
        }
    }

    /// The contrast issue of `id`'s words, when they are a text's or a labelled shape's and
    /// some part of them, the text's own colour or a run's, is too faint on its ground.
    fn contrast(&self, id: &str) -> Option<Found> {
        let n = self.scene.node(id)?;
        let words = n.text.as_deref().filter(|t| !t.trim().is_empty())?;
        let labelled = matches!(n.kind, NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Sticky);
        if !(n.kind == NodeKind::Text || labelled) || self.clear(id) {
            return None;
        }
        let r = self.placed.rect(id)?;
        let (ground, under, mine) = self.under_words(n, &r)?;
        let ground = &self.order[ground];
        let bg = under.clone().color();
        let own = text_color(self.scene, n);
        let (size, weight) = (n.font_size.unwrap_or(16.0), n.font_weight.unwrap_or(400));
        let fade = mine.last().map_or(1.0, |&i| self.seen[i]);
        let runs = n.runs.as_deref().unwrap_or(&[]);
        let len = words.chars().count() as u32;
        let covered: u32 = runs.iter().map(|run| run.end.min(len).saturating_sub(run.start.min(len))).sum();
        // The words in the text's own colour, when any are outside every run, then each run.
        let plain = (covered < len).then_some((None, own.as_str(), size, weight));
        let parts = runs.iter().map(|run| {
            let color = run.color.as_deref().unwrap_or(&own);
            (Some((run.start, run.end)), color, run.font_size.unwrap_or(size), run.font_weight.unwrap_or(weight))
        });
        let worst = plain
            .into_iter()
            .chain(parts)
            .filter_map(|(chars, color, size, weight)| {
                let ink = parse_color(color)?;
                let mut drawn = under.clone();
                drawn.lay(&mine, &self.opacity, ink);
                let fg = drawn.color();
                let ratio = contrast(&hex(fg), &hex(bg));
                let needs = if size >= 24.0 || (size >= 18.66 && weight >= 700) { LARGE_NEEDS } else { TEXT_NEEDS };
                (ratio < needs).then_some((ratio / needs, chars, ink, fg, ratio, needs))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        let (_, chars, ink, fg, ratio, needs) = worst;
        let what = match chars {
            Some((start, end)) => format!("{id}, characters {start} to {end}"),
            None => id.to_string(),
        };
        let faded = if fade < 1.0 { format!(" at {}% opacity", (fade * 100.0).round()) } else { String::new() };
        let drawn = if fade < 1.0 || ink[3] < 255 { format!(" (drawn {})", hex(fg)) } else { String::new() };
        let message = format!("{what}: {}{faded}{drawn} on {} ({ground}) is {}:1; text this size needs {}:1", hex(ink), hex(bg), short(ratio), fmt(needs));
        let ids = if ground == id { vec![id.to_string()] } else { vec![id.to_string(), ground.clone()] };
        Some(Found { code: Code::Contrast, at: (self.at(id), self.at(ground)), ids, message })
    }

    /// What is drawn under the centre of `n` (placed at `r`) before its words, with the ground
    /// they are read on and `n`'s [`Paint::chain`], to lay its words down with. Every node
    /// painted before it there that is not a text, line, path, icon or group (itself first, when
    /// it paints a ground of its own) is laid down, down to the first that hides what is under
    /// it, on the page's fill. The ground is the nearest of them whose colour is at least half
    /// opaque beside the words ([`Paint::apart`]), else the page. `None` when a picture, or a
    /// colour that does not parse, is nearer than the ground; one under it is passed over.
    fn under_words(&self, n: &Node, r: &Rect) -> Option<(usize, Stack, Vec<usize>)> {
        let parent = n.parent.as_deref().unwrap_or_default();
        let centre = if self.turned.get(self.at(parent)).copied().unwrap_or(false) {
            shapes::to_page_pt(self.scene, self.placed, parent, r.cx(), r.cy())
        } else {
            (r.cx(), r.cy())
        };
        let from = self.at.get(&n.id).copied()?;
        let mine = self.chain(from);
        let own = ground_of(n).is_some().then_some(from);
        let (mut ground, mut laid) = (None, Vec::new());
        for i in own.into_iter().chain(self.grounds.under(centre, from)) {
            let id = &self.order[i];
            let (Some(c), Some(cr)) = (self.scene.node(id), self.placed.rect(id)) else { continue };
            let (x, y) = if self.turned[i] { shapes::to_local(self.scene, self.placed, id, centre.0, centre.1) } else { centre };
            // What it paints, not its box: a ring's hole, a round corner or a pie's missing slice
            // shows what is under it.
            if !on_shape(self.scene, self.placed, c, &cr, x, y, 0.0) {
                continue;
            }
            let color = match c.kind {
                NodeKind::Image => None,
                _ => match ground_of(c) {
                    Some(g) => parse_color(&g),
                    None => continue,
                },
            };
            let Some(color) = color else {
                // Nothing can be said over it; under the ground it is passed over.
                ground?;
                continue;
            };
            laid.push((i, color));
            if ground.is_none() && f64::from(color[3]) / 255.0 * self.apart(i, &mine) >= 0.5 {
                ground = Some(i);
            }
            if color[3] == 255 && self.seen[i] >= 1.0 {
                break;
            }
        }
        let page = self.scene.node(self.order.first()?).and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        let mut under = Stack::new(parse_color(&page)?);
        for (i, color) in laid.into_iter().rev() {
            under.lay(&self.chain(i), &self.opacity, color);
        }
        Some((ground.unwrap_or(0), under, mine))
    }
}

/// One point of the page as the painter lays it down: on the page's fill, the layers open there,
/// outermost first, each a node of `order`, its opacity and what is drawn in it so far
/// (premultiplied red, green, blue and alpha, 0 to 1). A layer is laid down on the one under it
/// at its opacity when it closes, so a node and what is inside it fade together.
#[derive(Clone)]
struct Stack(Vec<(usize, f64, [f64; 4])>);

impl Stack {
    fn new([r, g, b, _]: [u8; 4]) -> Stack {
        let [r, g, b] = [r, g, b].map(|c| f64::from(c) / 255.0);
        Stack(vec![(usize::MAX, 1.0, [r, g, b, 1.0])])
    }

    /// Lay `color` down in the layer of the last of `chain` (a node and its ancestors, the
    /// page first; `opacity` each node's), closing the layers it is not inside and opening
    /// those of its own.
    fn lay(&mut self, chain: &[usize], opacity: &[f64], color: [u8; 4]) {
        let keep = self.0[1..].iter().zip(chain).take_while(|(open, i)| open.0 == **i).count();
        while self.0.len() > keep + 1 {
            self.close();
        }
        self.0.extend(chain[keep..].iter().map(|&i| (i, opacity[i], [0.0; 4])));
        let a = f64::from(color[3]) / 255.0;
        let [r, g, b] = [color[0], color[1], color[2]].map(|c| f64::from(c) / 255.0 * a);
        if let Some(top) = self.0.last_mut() {
            top.2 = over([r, g, b, a], top.2);
        }
    }

    /// Lay the top layer down on the one under it, at its opacity.
    fn close(&mut self) {
        if let [.., under, (_, o, px)] = self.0.as_mut_slice() {
            under.2 = over(px.map(|v| v * *o), under.2);
            self.0.pop();
        }
    }

    /// The colour drawn there once every layer is laid down, opaque.
    fn color(mut self) -> [u8; 4] {
        while self.0.len() > 1 {
            self.close();
        }
        let [r, g, b, _] = self.0[0].2;
        let [r, g, b] = [r, g, b].map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
        [r, g, b, 255]
    }
}

/// Premultiplied `top` laid over `under`.
fn over(top: [f64; 4], under: [f64; 4]) -> [f64; 4] {
    [0, 1, 2, 3].map(|k| top[k] + under[k] * (1.0 - top[3]))
}

/// Which side of `r` passes `[left, right, top, bottom]` furthest, by more than [`SLACK`]: the
/// side, how far, `r`'s edge there and the limit. The sides across or down are passed over
/// when the parent scrolls that way.
fn passes(r: &Rect, [left, right, top, bottom]: [f64; 4], across: bool, down: bool) -> Option<(&'static str, f64, f64, f64)> {
    let mut sides = Vec::with_capacity(4);
    if !across {
        sides.extend([("left", left - r.x, r.x, left), ("right", r.right() - right, r.right(), right)]);
    }
    if !down {
        sides.extend([("top", top - r.y, r.y, top), ("bottom", r.bottom() - bottom, r.bottom(), bottom)]);
    }
    sides.into_iter().filter(|s| s.1 > SLACK).max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Where an outer shadow `s` stops showing past its box, along each side (left, right, top,
/// bottom): its offset and spread, and for a blur, as far as it stays at least 5% dark (a blur
/// falls off as a gaussian of sigma blur / 2, taken here as a logistic curve). `None` for a
/// shadow too faint to see anywhere.
fn shadow_reach(s: &Shadow) -> Option<[f64; 4]> {
    const SEEN: f64 = 0.05;
    let alpha = f64::from(parse_color(&s.color)?[3]) / 255.0;
    let blur = s.blur.max(0.0);
    let fade = if blur > 0.0 {
        if alpha <= 2.0 * SEEN {
            return None;
        }
        blur / 2.0 / 1.702 * (alpha / SEEN - 1.0).ln()
    } else if alpha >= SEEN {
        0.0
    } else {
        return None;
    };
    let out = s.spread + fade;
    Some([out - s.x, out + s.x, out - s.y, out + s.y])
}

/// The side of clipping frame `frame` whose edge (`edges`: left, right, top, bottom) squares off
/// the outer shadows of `n` drawn at `r`, with how far they show past it and how far `r` stands
/// from it, for a cut past half a px and never along the axis the frame scrolls. Along a side `r`
/// is flush with, only where the frame wraps `n` there (it hugs along that axis, or is `n`'s
/// own box): a glowing button in a row that hugs it is cut, a bar flush with its screen casts
/// past the screen's edge on purpose.
fn shadow_cut(n: &Node, r: &Rect, frame: &Node, edges: [f64; 4], across: bool, down: bool) -> Option<(&'static str, f64, f64)> {
    let gaps = [r.x - edges[0], edges[1] - r.right(), r.y - edges[2], edges[3] - r.bottom()];
    let sides = ["left", "right", "top", "bottom"];
    let snug = gaps.iter().all(|g| g.abs() <= SLACK);
    let wraps = |k: usize| snug || if k < 2 { frame.w_sizing == Some(Sizing::Hug) } else { frame.h_sizing == Some(Sizing::Hug) };
    let mut worst: Option<(&'static str, f64, f64)> = None;
    for s in shapes::effective_shadows(n).iter().filter(|s| !s.inset) {
        let Some(reach) = shadow_reach(s) else { continue };
        for k in 0..4 {
            let scrolls = if k < 2 { across } else { down };
            let (gap, past) = (gaps[k], reach[k] - gaps[k].max(0.0));
            if scrolls || (gap <= SLACK && !wraps(k)) || past <= SLACK {
                continue;
            }
            if worst.is_none_or(|w| past > w.1) {
                worst = Some((sides[k], past, gap));
            }
        }
    }
    worst
}

/// The rounded corners of a clipping frame drawn at `frame` with `radii` (top-left, top-right,
/// bottom-right, bottom-left) that cut what `n` paints at `r`, in that order, with the deepest
/// cut past a corner's curve and that corner's radius. Only for what paints out to its box's
/// corners (a box with a fill, border or picture, an ellipse as the circle in its box), never at
/// a corner `r` is flush with both edges of (full-bleed content is rounded with its frame on
/// purpose), and none under half a px.
fn cut_by_corner(n: &Node, r: &Rect, frame: &Rect, radii: [f64; 4]) -> Option<(Vec<&'static str>, f64, f64)> {
    let paints = n.stroke.is_some() || n.gradient.is_some() || fill_of(n).and_then(|f| parse_color(&f)).is_some_and(|c| c[3] > 0);
    let own = match n.kind {
        NodeKind::Rect | NodeKind::Frame | NodeKind::Sticky if paints => shapes::corner_radii(n, r),
        NodeKind::Image => shapes::corner_radii(n, r),
        NodeKind::Ellipse if paints && n.arc.is_none() => [r.w.min(r.h) / 2.0; 4],
        _ => return None,
    };
    let corners = [("top-left", -1.0, -1.0), ("top-right", 1.0, -1.0), ("bottom-right", 1.0, 1.0), ("bottom-left", -1.0, 1.0)];
    let cuts: Vec<(&'static str, f64, f64)> = corners
        .iter()
        .zip(radii.iter().zip(own))
        .filter(|(_, (c, _))| **c > 0.0)
        .filter_map(|(&(name, sx, sy), (&c, mine))| {
            let edge = |s: f64, lo: f64, hi: f64| if s < 0.0 { lo } else { hi };
            // Flush with (or past) both of the frame's edges there: rounded with it on purpose.
            let flush = (edge(sx, r.x, r.right()) - edge(sx, frame.x, frame.right())) * sx >= -SLACK
                && (edge(sy, r.y, r.bottom()) - edge(sy, frame.y, frame.bottom())) * sy >= -SLACK;
            let centre = (edge(sx, frame.x, frame.right()) - sx * c, edge(sy, frame.y, frame.bottom()) - sy * c);
            let at = (edge(sx, r.x, r.right()) - sx * mine, edge(sy, r.y, r.bottom()) - sy * mine);
            // Only where it reaches into the corner's square, outward of the curve's centre both ways.
            let (dx, dy) = ((at.0 - centre.0) * sx, (at.1 - centre.1) * sy);
            let depth = dx.hypot(dy) + mine - c;
            (!flush && dx > 0.0 && dy > 0.0 && depth > SLACK).then_some((name, depth, c))
        })
        .collect();
    // The deepest cut, the first of equal ones.
    let &(_, depth, radius) = cuts.iter().reduce(|a, b| if b.1 > a.1 { b } else { a })?;
    Some((cuts.iter().map(|c| c.0).collect(), depth, radius))
}

/// A colour as `#RRGGBB`, or `#RRGGBBAA` when it is not opaque.
fn hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// A length for a message, to 0.01 px without trailing zeros.
fn fmt(v: f64) -> String {
    format!("{}", (v * 100.0).round() / 100.0 + 0.0)
}

/// A contrast ratio for a message, cut (never rounded up) to 0.01, so a ratio just short of
/// what is needed never reads as enough.
fn short(v: f64) -> String {
    format!("{}", (v * 100.0).floor() / 100.0 + 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::motion::posed_still;
    use crate::design::scene::tests::node;
    use relay_bus::design::{NodeKind as K, Scroll, Sizing, Styles, TextRun};

    fn page(nodes: Vec<Node>) -> (Scene, Placed) {
        filled_page(None, nodes)
    }

    /// `nodes` on a page filled `fill` (none: the canvas).
    fn filled_page(fill: Option<&str>, nodes: Vec<Node>) -> (Scene, Placed) {
        let mut p1 = node("p1", None, K::Page, [0.0; 4]);
        p1.fill = fill.map(String::from);
        let s = Scene::new(std::iter::once(p1).chain(nodes).collect());
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

    fn codes(found: &(Vec<RenderIssue>, u32)) -> Vec<(&str, Vec<&str>)> {
        found.0.iter().map(|i| (i.code.as_str(), i.ids.iter().map(String::as_str).collect())).collect()
    }

    fn all_issues(s: &Scene, p: &Placed) -> (Vec<RenderIssue>, u32) {
        issues(s, p, &Mono, "p1", &[])
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
        let row_ids = |b: &RenderBoxes| b.rows.iter().map(|r| r[0].clone()).collect::<Vec<_>>();
        let part = boxes(&s, &p, "p1", &ids(&["b", "a", "d"]), 200);
        assert_eq!(row_ids(&part), [json!("a"), json!("b"), json!("d")], "the outermost and what is inside them, once each");
        assert_eq!(row_ids(&boxes(&s, &p, "p1", &ids(&["d", "a"]), 200)), [json!("a"), json!("b"), json!("d")], "in get order, whatever order the ids come in");
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
    fn overflow_past_padding() {
        let mut card = frame("settings-card", "p1", [0.0, 0.0, 424.0, 200.0], Layout::Column);
        card.padding = Some(vec![24.0]);
        let (s, p) = page(vec![card, node("save-button", Some("settings-card"), K::Rect, [0.0, 0.0, 388.0, 40.0]), node("fits", Some("settings-card"), K::Rect, [0.0, 0.0, 376.5, 40.0])]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("overflow", vec!["save-button", "settings-card"])], "half a px over is rounding");
        assert_eq!(found.0[0].message, "save-button extends 12 px past settings-card's padding (right 412, limit 400)");
        assert_eq!(found.1, 1);
        // The frame need not be in scope; the child must.
        assert_eq!(codes(&issues(&s, &p, &Mono, "p1", &ids(&["save-button"]))).len(), 1);
        assert!(issues(&s, &p, &Mono, "p1", &ids(&["fits"])).0.is_empty());
    }

    #[test]
    fn overlap_only_in_laid_out_frames() {
        // Auto layout lines its children up, so only sizes it was not given overlap them: a
        // placement drawn over itself is reported.
        let row = frame("row", "p1", [0.0, 0.0, 300.0, 50.0], Layout::Row);
        let (s, mut p) = page(vec![row, node("a", Some("row"), K::Rect, [0.0, 0.0, 100.0, 50.0]), node("b", Some("row"), K::Rect, [0.0, 0.0, 100.0, 50.0])]);
        assert!(all_issues(&s, &p).0.is_empty(), "laid out side by side");
        p.rects.insert("b".into(), Rect::new(90.0, 0.0, 100.0, 50.0));
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("overlap", vec!["a", "b"])]);
        assert_eq!(found.0[0].message, "a and b overlap 10 × 50 px in row's row layout");
        // Free siblings may lie over each other: a badge on a card.
        let (s, p) = page(vec![node("a", Some("p1"), K::Rect, [0.0, 0.0, 100.0, 50.0]), node("b", Some("p1"), K::Rect, [50.0, 0.0, 100.0, 50.0])]);
        assert!(all_issues(&s, &p).0.is_empty());
        // A negative gap stacks them on purpose: avatars.
        let mut stack = frame("stack", "p1", [0.0, 0.0, 0.0, 40.0], Layout::Row);
        (stack.gap, stack.w_sizing) = (Some(-12.0), Some(Sizing::Hug));
        let (s, p) = page(vec![stack, node("x", Some("stack"), K::Ellipse, [0.0, 0.0, 40.0, 40.0]), node("y", Some("stack"), K::Ellipse, [0.0, 0.0, 40.0, 40.0])]);
        assert_eq!(p.rect("y").unwrap().x, 28.0);
        assert!(all_issues(&s, &p).0.is_empty());
    }

    /// A glow squared off: a pause button 4 px from the edge of a screen that clips, its 16 px
    /// glow showing about 12 px. A bar flush with the edge casts past it on purpose, and a glow
    /// with room to fade, or too faint to see, is not cut.
    #[test]
    fn a_shadow_cut_by_a_clipping_frame_is_clipped() {
        let mut screen = node("screen", Some("p1"), K::Frame, [0.0, 0.0, 390.0, 200.0]);
        screen.fill = Some("#101418".into());
        let glow = |id: &str, x: f64, color: &str| {
            let mut n = node(id, Some("screen"), K::Ellipse, [x, 80.0, 40.0, 40.0]);
            n.fill = Some("#4AF0FF".into());
            n.shadow = Some(Shadow { x: 0.0, y: 0.0, blur: 16.0, spread: 0.0, color: color.into(), inset: false });
            n
        };
        let mut bar = node("bar", Some("screen"), K::Rect, [0.0, 150.0, 390.0, 50.0]);
        bar.shadow = Some(Shadow { x: 0.0, y: -2.0, blur: 12.0, spread: 0.0, color: "#00000066".into(), inset: false });
        let (s, p) = page(vec![screen, glow("pause", 4.0, "#4AF0FF99"), glow("roomy", 150.0, "#4AF0FF99"), glow("faint", 346.0, "#4AF0FF0A"), bar]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("clipped", vec!["pause", "screen"])]);
        let message = &found.0[0].message;
        assert!(message.starts_with("pause's shadow is cut off by screen's left edge, which clips: it shows 7.27 px past it, 4 px from pause"), "{message}");
        // Unclipped, nothing is cut.
        let (mut s, _) = (s, p);
        s.node_mut("screen").unwrap().clip = Some(false);
        let p = place(&s, "p1", &Mono);
        assert!(all_issues(&s, &p).0.is_empty());
    }

    /// A rounded corner cuts what paints out to it: a legend swatch tucked in a card's corner.
    /// An inner card whose corner follows the outer one, words, and a frame that scrolls are not
    /// cut.
    #[test]
    fn a_rounded_corner_cuts_what_reaches_into_it() {
        let mut card = node("card", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 200.0]);
        card.radius = Some(24.0);
        let mut swatch = node("swatch", Some("card"), K::Rect, [4.0, 4.0, 10.0, 10.0]);
        swatch.fill = Some("#4C7DFF".into());
        let mut inner = node("inner", Some("card"), K::Frame, [8.0, 8.0, 284.0, 184.0]);
        (inner.radius, inner.fill) = (Some(16.0), Some("#F4F4F5".into()));
        let words = text("words", "card", [2.0, 180.0, 0.0, 0.0], "Hi");
        let (s, p) = page(vec![card, inner, swatch, words]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("clipped", vec!["swatch", "card"])]);
        let message = &found.0[0].message;
        assert!(message.starts_with("swatch is cut 4.28 px ") && message.contains("by card's rounded top-left corner (radius 24)"), "{message}");
        let mut s = s;
        s.node_mut("card").unwrap().scroll = Some(Scroll::Y);
        let p = place(&s, "p1", &Mono);
        assert!(all_issues(&s, &p).0.is_empty(), "what scrolls past a round corner is meant to");
    }

    /// Full-bleed content rounded with its frame is meant: a photo filling a rounded card, its
    /// colour and shade layers over it, and a sidebar flush in a rounded window. An inset layer
    /// is cut at all four corners alike, and said so.
    #[test]
    fn full_bleed_content_is_rounded_on_purpose() {
        let mut card = node("card", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 200.0]);
        card.radius = Some(24.0);
        let mut photo = node("photo", Some("card"), K::Image, [0.0, 0.0, 300.0, 160.0]);
        photo.src = Some("a.png".into());
        let mut tint = node("tint", Some("card"), K::Rect, [0.0, 0.0, 300.0, 160.0]);
        tint.fill = Some("#4C7DFF33".into());
        let mut shade = node("shade", Some("card"), K::Rect, [0.0, 100.0, 300.0, 100.0]);
        shade.fill = Some("#00000066".into());
        let mut window = node("window", Some("p1"), K::Frame, [400.0, 0.0, 600.0, 400.0]);
        window.radius = Some(12.0);
        let mut sidebar = node("sidebar", Some("window"), K::Rect, [0.0, 0.0, 180.0, 400.0]);
        sidebar.fill = Some("#1B2230".into());
        let nodes = vec![card, photo, tint, shade, window, sidebar];
        let (s, p) = page(nodes.clone());
        assert!(all_issues(&s, &p).0.iter().all(|i| i.code != "clipped"), "{:?}", all_issues(&s, &p).0);
        let mut inset = node("inset", Some("window"), K::Rect, [2.0, 2.0, 596.0, 396.0]);
        inset.fill = Some("#222222".into());
        let (s, p) = page(nodes.into_iter().chain([inset]).collect());
        let found = all_issues(&s, &p);
        let cut: Vec<&RenderIssue> = found.0.iter().filter(|i| i.code == "clipped").collect();
        assert_eq!(cut.len(), 1, "{found:?}");
        assert!(cut[0].message.contains("by window's rounded top-left, top-right, bottom-right and bottom-left corners (radius 12)"), "{}", cut[0].message);
    }

    /// A glowing button in a row that hugs it: flush on every side, its glow cut flat all round.
    #[test]
    fn a_glow_in_a_frame_that_hugs_it_is_cut() {
        let mut row = frame("row", "p1", [20.0, 20.0, 0.0, 0.0], Layout::Row);
        (row.w_sizing, row.h_sizing) = (Some(Sizing::Hug), Some(Sizing::Hug));
        let mut pause = node("pause", Some("row"), K::Ellipse, [0.0, 0.0, 40.0, 40.0]);
        pause.fill = Some("#4AF0FF".into());
        pause.shadow = Some(Shadow { x: 0.0, y: 0.0, blur: 16.0, spread: 0.0, color: "#4AF0FF99".into(), inset: false });
        let (s, p) = page(vec![row, pause]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("clipped", vec!["pause", "row"])]);
        assert!(found.0[0].message.contains("it shows 11.27 px past it, flush with pause, which row wraps"), "{}", found.0[0].message);
    }

    #[test]
    fn text_overlap_under_any_parent() {
        let (s, p) = page(vec![text("title", "p1", [0.0, 0.0, 0.0, 0.0], "Settings"), text("sub", "p1", [8.0, 10.0, 0.0, 0.0], "Account")]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("text_overlap", vec!["title", "sub"])]);
        assert_eq!(found.0[0].message, "title and sub are drawn over each other (56 × 10 px): text over text is hard to read");
        // In a frame, and a sticky's paper counts.
        let (s, p) = page(vec![
            node("f", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 400.0]),
            node("note", Some("f"), K::Sticky, [0.0, 0.0, 200.0, 200.0]),
            text("over", "f", [150.0, 150.0, 0.0, 0.0], "Hi"),
        ]);
        assert_eq!(codes(&all_issues(&s, &p)), [("text_overlap", vec!["note", "over"])]);
        // Words that spill out of a fixed-height text are where it is drawn.
        let mut spill = text("spill", "p1", [0.0, 0.0, 80.0, 20.0], &"x".repeat(30));
        (spill.w_sizing, spill.h_sizing) = (Some(Sizing::Fixed), Some(Sizing::Fixed));
        let (s, p) = page(vec![spill.clone(), text("below", "p1", [0.0, 30.0, 0.0, 0.0], "Next")]);
        assert_eq!(codes(&all_issues(&s, &p)), [("text_overlap", vec!["spill", "below"])]);
        // A wide box with few words: the words are drawn at its left, and the box is empty.
        let mut wide = text("wide", "p1", [0.0, 0.0, 300.0, 20.0], "Hi");
        wide.w_sizing = Some(Sizing::Fixed);
        let (s, p) = page(vec![wide.clone(), text("right", "p1", [200.0, 0.0, 0.0, 0.0], "There")]);
        assert!(all_issues(&s, &p).0.is_empty());
        wide.fill = Some("#FFFFFF".into());
        let (s, p) = page(vec![wide, text("right", "p1", [200.0, 0.0, 0.0, 0.0], "There")]);
        assert_eq!(codes(&all_issues(&s, &p))[0].0, "text_overlap", "a filled text paints its box");
    }

    #[test]
    fn clipped_child_of_a_clipping_frame() {
        let (s, p) = page(vec![
            node("card", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 400.0]),
            node("img", Some("card"), K::Rect, [100.0, 300.0, 100.0, 112.0]),
            node("inside", Some("card"), K::Rect, [0.0, 0.0, 400.0, 400.0]),
        ]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("clipped", vec!["img", "card"])]);
        assert_eq!(found.0[0].message, "img is cut off 12 px past card's edge, which clips (bottom 412, limit 400)");
        let mut open = s.node("card").unwrap().clone();
        open.clip = Some(false);
        let (s, p) = page(vec![open, node("img", Some("card"), K::Rect, [100.0, 300.0, 100.0, 112.0])]);
        assert!(all_issues(&s, &p).0.is_empty(), "an unclipped frame shows what spills");
    }

    #[test]
    fn touching_edges_are_not_overlap() {
        let (s, p) = page(vec![text("a", "p1", [0.0, 0.0, 0.0, 0.0], "Left"), text("b", "p1", [32.0, 0.0, 0.0, 0.0], "Right"), text("c", "p1", [0.0, 20.0, 0.0, 0.0], "Below")]);
        assert!(all_issues(&s, &p).0.is_empty());
        let (s, p) = page(vec![text("a", "p1", [0.0, 0.0, 0.0, 0.0], "Left"), text("b", "p1", [31.99, 0.0, 0.0, 0.0], "Right")]);
        assert!(all_issues(&s, &p).0.is_empty(), "0.2 px² is rounding");
        let (s, p) = page(vec![text("a", "p1", [0.0, 0.0, 0.0, 0.0], "Left"), text("b", "p1", [31.9, 0.0, 0.0, 0.0], "Right")]);
        assert_eq!(all_issues(&s, &p).1, 1, "2 px² is not");
    }

    #[test]
    fn rotated_nodes_are_skipped() {
        let mut turned = node("t", Some("card"), K::Rect, [100.0, 300.0, 100.0, 200.0]);
        turned.rotation = Some(45.0);
        let mut flipped = text("u", "p1", [500.0, 0.0, 0.0, 0.0], "Upside");
        flipped.rotation = Some(180.0);
        let (s, p) = page(vec![node("card", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 400.0]), turned, flipped, text("v", "p1", [520.0, 0.0, 0.0, 0.0], "Down")]);
        assert_eq!(codes(&all_issues(&s, &p)), [("text_overlap", vec!["u", "v"])], "a half turn covers the same box");
    }

    #[test]
    fn issues_are_ordered_and_capped_at_100_with_the_full_count() {
        let mut nodes = vec![node("card", Some("p1"), K::Frame, [0.0, 0.0, 100.0, 100.0])];
        nodes.extend((0..120).map(|i| node(&format!("c{i:03}"), Some("card"), K::Rect, [90.0, 0.0, 20.0, 10.0])));
        let mut col = frame("col", "p1", [200.0, 0.0, 100.0, 100.0], Layout::Column);
        col.padding = Some(vec![10.0]);
        nodes.extend([col, node("wide", Some("col"), K::Rect, [0.0, 0.0, 200.0, 10.0])]);
        nodes.extend([text("t1", "p1", [0.0, 500.0, 0.0, 0.0], "One"), text("t2", "p1", [8.0, 500.0, 0.0, 0.0], "Two")]);
        let (s, p) = page(nodes);
        let (list, count) = all_issues(&s, &p);
        assert_eq!((list.len(), count), (MAX_ISSUES, 122));
        assert_eq!((list[0].code.as_str(), list[1].code.as_str(), list[2].code.as_str()), ("overflow", "text_overlap", "clipped"));
        assert_eq!(list[2].ids[0], "c000", "then in paint order");
        assert_eq!(list[99].ids[0], "c097");
    }

    #[test]
    fn issues_skip_the_scroll_axis_of_scrolling_frames() {
        let mut carousel = frame("carousel", "p1", [0.0, 0.0, 200.0, 100.0], Layout::Row);
        carousel.scroll = Some(Scroll::X);
        let cards = (0..4).map(|i| node(&format!("card{i}"), Some("carousel"), K::Rect, [0.0, 0.0, 90.0, 120.0]));
        let (s, p) = page(std::iter::once(carousel).chain(cards).collect());
        let found = all_issues(&s, &p);
        assert_eq!(found.1, 4, "the cards past its right edge scroll into view: {:?}", found.0);
        assert!(found.0.iter().all(|i| i.code == "overflow" && i.message.ends_with("(bottom 120, limit 100)")), "down still overflows: {:?}", found.0);
        let mut feed = node("feed", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        feed.scroll = Some(Scroll::Y);
        let (s, p) = page(vec![feed, node("post", Some("feed"), K::Rect, [0.0, 50.0, 100.0, 400.0])]);
        assert!(all_issues(&s, &p).0.is_empty(), "a feed's content passes its bottom on purpose");
    }

    #[test]
    fn absolute_children_are_left_out_of_overflow_and_overlap_but_can_be_clipped() {
        let mut card = frame("card", "p1", [0.0, 0.0, 200.0, 100.0], Layout::Column);
        card.padding = Some(vec![16.0]);
        let mut badge = node("badge", Some("card"), K::Ellipse, [190.0, -5.0, 20.0, 20.0]);
        badge.absolute = Some(true);
        // A pin over the laid-out body (16..184 by 16..56), past the padding's right (184).
        let mut pin = node("pin", Some("card"), K::Ellipse, [170.0, 10.0, 20.0, 20.0]);
        pin.absolute = Some(true);
        let body = node("body", Some("card"), K::Rect, [0.0, 0.0, 168.0, 40.0]);
        let (s, p) = page(vec![card.clone(), body.clone(), badge.clone(), pin.clone()]);
        assert_eq!(p.rect("body"), Some(Rect::new(16.0, 16.0, 168.0, 40.0)));
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("clipped", vec!["badge", "card"])], "a badge hanging off a clipping card is cut; the pin is neither overflow nor overlap");
        assert_eq!(found.0[0].message, "badge is cut off 10 px past card's edge, which clips (right 210, limit 200)");
        card.clip = Some(false);
        let (s, p) = page(vec![card, body, badge, pin]);
        assert!(all_issues(&s, &p).0.is_empty(), "nor past an open one's padding");
    }

    /// A text on a frame filled `ground`.
    fn on_ground(ground: &str, words: Node) -> (Scene, Placed) {
        let mut f = node("email", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        f.fill = Some(ground.into());
        page(vec![f, words])
    }

    #[test]
    fn contrast_flags_grey_on_white_and_spares_large_text() {
        let mut hint = text("settings-hint", "email", [10.0, 10.0, 0.0, 0.0], "Your address");
        hint.color = Some("#9A9A9A".into());
        let (s, p) = on_ground("#F4F4F4", hint.clone());
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["settings-hint", "email"])]);
        assert_eq!(found.0[0].message, "settings-hint: #9A9A9A on #F4F4F4 (email) is 2.55:1; text this size needs 4.5:1");
        // #888888 on white is 3.54:1: too faint at 16 px, enough at 24.
        hint.color = Some("#888888".into());
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0[0].message.ends_with("is 3.54:1; text this size needs 4.5:1"));
        hint.font_size = Some(24.0);
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0.is_empty());
        hint.color = Some("#AAAAAA".into());
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0[0].message.ends_with("is 2.32:1; text this size needs 3:1"));
        (hint.font_size, hint.font_weight) = (Some(18.66), Some(700));
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0[0].message.ends_with("is 2.32:1; text this size needs 3:1"), "bold 18.66 px is large too");
        hint.font_weight = Some(600);
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0[0].message.ends_with("needs 4.5:1"));
        hint.font_size = Some(16.0);
        // A ratio just short of what is needed never reads as enough.
        hint.color = Some("#777777".into());
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0[0].message.contains("is 4.47:1"), "{:?}", all_issues(&s, &p).0);
        hint.color = Some("#767676".into());
        let (s, p) = on_ground("#FFFFFF", hint);
        assert!(all_issues(&s, &p).0.is_empty(), "4.54:1 is enough");
        // The colour the painter picks itself always reads.
        let (s, p) = on_ground("#F4F4F4", text("plain", "email", [10.0, 10.0, 0.0, 0.0], "Plain"));
        assert!(all_issues(&s, &p).0.is_empty());
    }

    #[test]
    fn contrast_reads_the_shape_under_the_text() {
        // On the dark canvas a text with no colour of its own is light; on a light card it fades.
        let card = node("card", Some("p1"), K::Rect, [0.0, 0.0, 300.0, 100.0]);
        let (s, p) = page(vec![card.clone(), text("label", "p1", [10.0, 40.0, 0.0, 0.0], "Name")]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["label", "card"])]);
        assert_eq!(found.0[0].message, "label: #EDE9E2 on #D9D9D9 (card) is 1.16:1; text this size needs 4.5:1");
        // A shape whose box misses the text's centre is not under it; one painted later is not
        // under it either.
        let (s, p) = page(vec![text("label", "p1", [10.0, 40.0, 0.0, 0.0], "Name"), card.clone()]);
        assert!(all_issues(&s, &p).0.is_empty(), "the canvas is under it");
        let mut aside = card.clone();
        aside.x = 200.0;
        let (s, p) = page(vec![aside, text("label", "p1", [10.0, 40.0, 0.0, 0.0], "Name")]);
        assert!(all_issues(&s, &p).0.is_empty());
        // A labelled shape's words sit on the shape itself.
        let mut button = node("button", Some("p1"), K::Rect, [0.0, 0.0, 120.0, 40.0]);
        (button.text, button.fill, button.color) = (Some("Save".into()), Some("#2D6BFF".into()), Some("#1A1A1A".into()));
        let (s, p) = page(vec![button]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["button"])]);
        assert_eq!(found.0[0].message, "button: #1A1A1A on #2D6BFF (button) is 3.86:1; text this size needs 4.5:1");
        // A ground under half opaque is laid over what is beneath it, and the next one is the
        // ground: white at 25% on black is a dark grey.
        let mut glass = node("glass", Some("p1"), K::Rect, [0.0, 0.0, 300.0, 100.0]);
        glass.fill = Some("#FFFFFF40".into());
        let mut ink = text("ink", "p1", [10.0, 40.0, 0.0, 0.0], "Ink");
        ink.color = Some("#222222".into());
        let mut black = card;
        black.fill = Some("#000000".into());
        let (s, p) = page(vec![black, glass, ink]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["ink", "card"])]);
        assert!(found.0[0].message.contains("#222222 on #404040 (card)"), "{}", found.0[0].message);
    }

    /// Words in a progress ring's hole are read on what shows through it, not on the ring; the
    /// ring's own label too. A box's round corner shows what is under it.
    #[test]
    fn contrast_reads_through_a_rings_hole_and_round_corners() {
        let mut dark = node("screen", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 300.0]);
        dark.fill = Some("#101418".into());
        let mut ring = node("ring", Some("screen"), K::Ellipse, [50.0, 50.0, 200.0, 200.0]);
        (ring.fill, ring.arc) = (Some("#E8F0FF".into()), Some(relay_bus::design::Arc { start: 0.0, sweep: 250.0, inner: 0.8, cap: None }));
        let mut pct = text("pct", "screen", [130.0, 140.0, 40.0, 20.0], "69%");
        pct.color = Some("#FFFFFF".into());
        let (s, p) = page(vec![dark.clone(), ring.clone(), pct.clone()]);
        assert!(all_issues(&s, &p).0.is_empty(), "white on the dark screen: {:?}", all_issues(&s, &p).0);
        // Dark words in the hole are flagged against the screen, not the light ring.
        pct.color = Some("#30343A".into());
        let (s, p) = page(vec![dark.clone(), ring.clone(), pct]);
        assert_eq!(codes(&all_issues(&s, &p)), [("contrast", vec!["pct", "screen"])]);
        // The ring's own label sits in its hole too.
        let mut labelled = ring.clone();
        (labelled.text, labelled.color) = (Some("69%".into()), Some("#FFFFFF".into()));
        let (s, p) = page(vec![dark.clone(), labelled]);
        assert!(all_issues(&s, &p).0.is_empty(), "{:?}", all_issues(&s, &p).0);
        // A pie without a hole is under its words, where it is drawn.
        let mut pie = ring;
        pie.arc = Some(relay_bus::design::Arc { start: 0.0, sweep: 360.0, inner: 0.0, cap: None });
        let mut ink = text("ink", "screen", [130.0, 140.0, 40.0, 20.0], "69%");
        ink.color = Some("#FFFFFF".into());
        let (s, p) = page(vec![dark.clone(), pie, ink.clone()]);
        assert_eq!(codes(&all_issues(&s, &p)), [("contrast", vec!["ink", "ring"])]);
        // A round corner leaves the screen showing at the box's very corner.
        let mut pill = node("pill", Some("screen"), K::Rect, [0.0, 0.0, 40.0, 40.0]);
        (pill.fill, pill.radius) = (Some("#FFFFFF".into()), Some(20.0));
        let corner = text("corner", "screen", [0.0, 0.0, 4.0, 4.0], ".");
        let (s, p) = page(vec![dark, pill, corner]);
        assert!(all_issues(&s, &p).0.is_empty(), "{:?}", all_issues(&s, &p).0);
    }

    /// `skip_checks` drops the issues that name the node first, and only those.
    #[test]
    fn skip_checks_leaves_out_what_names_it_first() {
        let card = node("card", Some("p1"), K::Rect, [0.0, 0.0, 300.0, 100.0]);
        let mut faint = text("faint", "p1", [10.0, 40.0, 0.0, 0.0], "Name");
        faint.skip_checks = Some(true);
        let loud = text("loud", "p1", [10.0, 70.0, 0.0, 0.0], "Name");
        let (s, p) = page(vec![card, faint, loud]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["loud", "card"])]);
        assert_eq!(found.1, 1, "nor is it counted");
    }

    fn moving(keys: serde_json::Value) -> relay_bus::design::Motion {
        serde_json::from_value(json!({"keys": keys})).unwrap()
    }

    /// Motion's problems are checked at the times pictured, each once at its first time, saying
    /// how many times show it; what the still has already is not listed again; nothing moving,
    /// nothing listed. A fade under way is not one: its words are read where it settles.
    #[test]
    fn motion_brings_its_own_issues() {
        let mut screen = node("screen", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 200.0]);
        screen.fill = Some("#101418".into());
        let mut title = text("title", "screen", [10.0, 10.0, 0.0, 0.0], "Left to spend");
        title.color = Some("#FFFFFF".into());
        title.motion = Some(moving(json!([{"at_ms": 0, "opacity": 0.1}, {"at_ms": 1000, "opacity": 1}])));
        let mut dim = text("dim", "screen", [10.0, 60.0, 0.0, 0.0], "Faint at rest");
        dim.color = Some("#30343A".into());
        let (s, p) = page(vec![screen.clone(), title.clone(), dim]);
        let clocks = Clocks::new(&s, "p1");
        let (still, _) = all_issues(&s, &p);
        assert_eq!(codes(&(still.clone(), 0)), [("contrast", vec!["dim", "screen"])]);
        let times = [0.0, 250.0, 500.0, 1000.0];
        assert!(motion_issues(&s, &p, &Mono, "p1", &[], &clocks, &times, false).0.is_empty(), "a fade in passes through faint");
        assert!(motion_issues(&s, &p, &Mono, "p1", &[], &Clocks::default(), &[0.0], false).0.is_empty());
        // A fade out that stops faint holds there: flagged from where it stops.
        let mut timed = screen.clone();
        timed.timeline = serde_json::from_value(json!({"duration_ms": 1000, "loop": false})).unwrap();
        title.motion = Some(moving(json!([{"at_ms": 0, "opacity": 1}, {"at_ms": 1000, "opacity": 0.1}])));
        let (s, p) = page(vec![timed, title.clone()]);
        let clocks = Clocks::new(&s, "p1");
        let (found, count) = motion_issues(&s, &p, &Mono, "p1", &[], &clocks, &times, false);
        assert_eq!((count, found.len()), (1, 1), "{found:?}");
        assert_eq!((found[0].code.as_str(), found[0].ids.clone(), found[0].at_ms), ("contrast", ids(&["title", "screen"]), Some(1000.0)));
        assert!(found[0].message.ends_with("(from 1000 ms of its motion; at 1 of the 4 times pictured)"), "{}", found[0].message);
        assert!(found[0].message.contains("at 10% opacity"), "{}", found[0].message);
        // One time: said as that time. Where a looping fade ends, `held` reads it as it stands.
        let (s, p) = page(vec![screen, title]);
        let clocks = Clocks::new(&s, "p1");
        assert!(motion_issues(&s, &p, &Mono, "p1", &[], &clocks, &[1000.0], false).0.is_empty(), "it loops, so it never stops faint");
        let (end, _) = motion_issues(&s, &p, &Mono, "p1", &[], &clocks, &[1000.0], true);
        assert!(end[0].message.ends_with("(at 1000 ms of its motion)"), "{}", end[0].message);
    }

    /// Motion moves a box after layout: words sliding up inside a column that hugs them have not
    /// overflowed it, nor met their sibling in its layout.
    #[test]
    fn motion_leaves_layout_to_the_still() {
        let mut card = frame("card", "p1", [0.0, 0.0, 200.0, 0.0], Layout::Column);
        (card.h_sizing, card.padding, card.clip) = (Some(Sizing::Hug), Some(vec![8.0]), Some(false));
        let mut rising = text("rising", "card", [0.0, 0.0, 0.0, 0.0], "Strength · Upper body");
        rising.motion = Some(moving(json!([{"at_ms": 0, "dy": 24}, {"at_ms": 400, "dy": 0}])));
        let (s, p) = page(vec![card, rising, text("below", "card", [0.0, 0.0, 0.0, 0.0], "45 min")]);
        assert!(all_issues(&s, &p).0.is_empty());
        let clocks = Clocks::new(&s, "p1");
        let (posed, moved) = posed_still(&s, &p, &clocks, 0.0);
        assert!(issues(&posed, &moved, &Mono, "p1", &[]).0.iter().any(|i| i.code == "overflow"), "as drawn at 0 ms it is past the padding");
        let (found, _) = motion_issues(&s, &p, &Mono, "p1", &[], &clocks, &[0.0, 200.0, 400.0], false);
        assert!(found.iter().all(|i| i.code != "overflow" && i.code != "overlap"), "{found:?}");
    }

    /// A pose moves a box with what is inside it: a toast sliding up over a label is drawn over it
    /// partway, and a colour key is read as posed.
    #[test]
    fn motion_moves_boxes_and_colours_as_posed() {
        let label = text("label", "p1", [10.0, 10.0, 0.0, 0.0], "Balance");
        let mut toast = node("toast", Some("p1"), K::Frame, [0.0, 200.0, 200.0, 40.0]);
        toast.motion = Some(moving(json!([{"at_ms": 0, "dy": 0}, {"at_ms": 1000, "dy": -200}])));
        let note = text("note", "toast", [10.0, 10.0, 0.0, 0.0], "Saved");
        let (s, p) = page(vec![label, toast, note]);
        let clocks = Clocks::new(&s, "p1");
        let (posed, moved) = posed_still(&s, &p, &clocks, 1000.0);
        assert_eq!(moved.rect("note").map(|r| (r.x, r.y)), Some((10.0, 10.0)), "the child moves with it");
        assert_eq!(posed.node("toast").map(|n| n.y), Some(200.0), "the node itself is as stored");
        assert!(motion_issues(&s, &p, &Mono, "p1", &[], &clocks, &[0.0, 1000.0], false).0.is_empty(), "a frame over a label is drawn on top of it");
        // A text sliding onto another is text over text.
        let mut sliding = text("sliding", "p1", [10.0, 200.0, 0.0, 0.0], "Saved");
        sliding.motion = Some(moving(json!([{"at_ms": 0, "dy": 0}, {"at_ms": 1000, "dy": -190}])));
        let (s, p) = page(vec![text("label", "p1", [10.0, 10.0, 0.0, 0.0], "Balance"), sliding]);
        let (found, _) = motion_issues(&s, &p, &Mono, "p1", &[], &Clocks::new(&s, "p1"), &[0.0, 500.0, 1000.0], false);
        assert_eq!(found.iter().map(|i| (i.code.as_str(), i.ids.clone(), i.at_ms)).collect::<Vec<_>>(), [("text_overlap", ids(&["label", "sliding"]), Some(1000.0))]);
        let mut chip = node("chip", Some("p1"), K::Rect, [0.0, 0.0, 120.0, 40.0]);
        (chip.text, chip.fill, chip.color) = (Some("Paid".into()), Some("#FFFFFF".into()), Some("#111111".into()));
        chip.motion = Some(moving(json!([{"at_ms": 0, "fill": "#FFFFFF"}, {"at_ms": 500, "fill": "#111111"}])));
        let (s, p) = page(vec![chip]);
        let (found, _) = motion_issues(&s, &p, &Mono, "p1", &[], &Clocks::new(&s, "p1"), &[0.0, 500.0], false);
        assert_eq!(found.iter().map(|i| (i.code.as_str(), i.at_ms)).collect::<Vec<_>>(), [("contrast", Some(500.0))]);
    }

    #[test]
    fn contrast_skips_text_over_a_picture() {
        let mut photo = node("photo", Some("email"), K::Image, [0.0, 0.0, 300.0, 100.0]);
        photo.src = Some("a.png".into());
        let mut caption = text("caption", "email", [10.0, 40.0, 0.0, 0.0], "Sunset");
        caption.color = Some("#FFFFFF".into());
        let mut f = node("email", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        f.fill = Some("#FFFFFF".into());
        let (s, p) = page(vec![f.clone(), photo.clone(), caption.clone()]);
        assert!(all_issues(&s, &p).0.is_empty());
        // On a scrim over the picture, the scrim is the ground.
        let mut scrim = node("scrim", Some("email"), K::Rect, [0.0, 0.0, 300.0, 100.0]);
        scrim.fill = Some("#FFFFFFCC".into());
        let (s, p) = page(vec![f, photo, scrim, caption]);
        assert_eq!(codes(&all_issues(&s, &p)), [("contrast", vec!["caption", "scrim"])]);
    }

    #[test]
    fn contrast_checks_each_run_with_its_own_colour() {
        let mut price = text("price", "email", [10.0, 10.0, 0.0, 0.0], "8.4 km away");
        price.color = Some("#1A1A1A".into());
        price.runs = Some(vec![TextRun { start: 4, end: 6, color: Some("#BBBBBB".into()), font: None, font_size: None, font_weight: None, italic: None, underline: None, strike: None, rise: None, letter_spacing: None }]);
        let (s, p) = on_ground("#FFFFFF", price.clone());
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["price", "email"])]);
        assert!(found.0[0].message.starts_with("price, characters 4 to 6: #BBBBBB on #FFFFFF (email) is 1.9"), "{}", found.0[0].message);
        // A large run needs less; a run without a colour of its own takes the text's.
        let runs = price.runs.as_mut().unwrap();
        runs[0].font_size = Some(40.0);
        runs[0].color = Some("#949494".into());
        let (s, p) = on_ground("#FFFFFF", price.clone());
        assert!(all_issues(&s, &p).0.is_empty(), "3.03:1 is enough at 40 px");
        price.color = Some("#AAAAAA".into());
        price.runs = Some(vec![TextRun { start: 0, end: 11, color: None, font: None, font_size: Some(30.0), font_weight: None, italic: None, underline: None, strike: None, rise: None, letter_spacing: None }]);
        let (s, p) = on_ground("#FFFFFF", price);
        let found = all_issues(&s, &p);
        assert!(found.0[0].message.starts_with("price, characters 0 to 11: #AAAAAA on #FFFFFF (email) is 2.32:1; text this size needs 3:1"), "{:?}", found.0);
    }

    #[test]
    fn contrast_counts_opacity() {
        let mut faded = text("faded", "email", [10.0, 10.0, 0.0, 0.0], "Disabled");
        (faded.color, faded.opacity) = (Some("#1A1A1A".into()), Some(0.4));
        let (s, p) = on_ground("#FFFFFF", faded.clone());
        let found = all_issues(&s, &p);
        assert!(found.0[0].message.starts_with("faded: #1A1A1A at 40% opacity (drawn #A3A3A3) on #FFFFFF (email) is 2.52:1"), "{:?}", found.0);
        faded.opacity = Some(0.0);
        let (s, p) = on_ground("#FFFFFF", faded.clone());
        assert!(all_issues(&s, &p).0.is_empty(), "a clear text is not read");
        (faded.opacity, faded.color) = (None, Some("#00000080".into()));
        let (s, p) = on_ground("#FFFFFF", faded);
        assert!(all_issues(&s, &p).0[0].message.starts_with("faded: #00000080 (drawn #7F7F7F) on #FFFFFF (email)"), "{:?}", all_issues(&s, &p).0);
    }

    #[test]
    fn contrast_counts_the_opacity_of_what_holds_the_words_and_their_ground() {
        let white = Some("#FFFFFF");
        let ink = |id: &str, parent: &str, color: &str| {
            let mut t = text(id, parent, [10.0, 40.0, 0.0, 0.0], "Words");
            t.color = Some(color.into());
            t
        };
        // A disabled card: black words on its white fill, both drawn at 30% on a white page.
        let mut card = node("card", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        (card.fill, card.opacity) = (Some("#FFFFFF".into()), Some(0.3));
        let (s, p) = filled_page(white, vec![card.clone(), ink("label", "card", "#000000")]);
        let found = all_issues(&s, &p);
        assert_eq!(codes(&found), [("contrast", vec!["label", "card"])]);
        assert!(found.0[0].message.starts_with("label: #000000 at 30% opacity (drawn #B3B3B3) on #FFFFFF (card) is 2.09:1"), "{}", found.0[0].message);
        // A group fades what is inside it too.
        let mut off = node("off", Some("p1"), K::Group, [0.0, 0.0, 0.0, 0.0]);
        off.opacity = Some(0.5);
        let (s, p) = filled_page(white, vec![off, ink("label", "off", "#000000")]);
        assert!(all_issues(&s, &p).0[0].message.starts_with("label: #000000 at 50% opacity (drawn #808080) on #FFFFFF (p1)"), "{:?}", all_issues(&s, &p).0);
        // Faded together, a dark card and its white words both show the white page through:
        // the words stay white and the card turns grey, 4.29:1 rather than white on #222222.
        (card.fill, card.opacity) = (Some("#222222".into()), Some(0.6));
        let (s, p) = filled_page(white, vec![card, ink("label", "card", "#FFFFFF")]);
        assert_eq!(all_issues(&s, &p).0[0].message, "label: #FFFFFF at 60% opacity (drawn #FFFFFF) on #7A7A7A (card) is 4.29:1; text this size needs 4.5:1");
        // A sheet in a faded overlay the words are not in is a faint ground, not a solid one:
        // white words read on it, and grey words are read on the page it lets through.
        let mut overlay = node("overlay", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        (overlay.fill, overlay.opacity) = (Some("#00000000".into()), Some(0.3));
        let sheet = |fill: &str| {
            let mut n = node("sheet", Some("overlay"), K::Rect, [0.0, 0.0, 300.0, 100.0]);
            n.fill = Some(fill.into());
            n
        };
        let (s, p) = page(vec![overlay.clone(), sheet("#FFFFFF"), ink("note", "p1", "#FFFFFF")]);
        assert!(all_issues(&s, &p).0.is_empty(), "{:?}", all_issues(&s, &p).0);
        let (s, p) = page(vec![overlay.clone(), sheet("#FFFFFF"), ink("note", "p1", "#999999")]);
        assert_eq!(codes(&all_issues(&s, &p)), [("contrast", vec!["note", "p1"])]);
        // Nothing inside a clear frame is under anything.
        overlay.opacity = Some(0.0);
        let (s, p) = filled_page(white, vec![overlay, sheet("#000000"), ink("note", "p1", "#111111")]);
        assert!(all_issues(&s, &p).0.is_empty(), "{:?}", all_issues(&s, &p).0);
    }

    #[test]
    fn contrast_reads_colours_as_drawn_and_passes_over_what_does_not_parse() {
        // The scene holds nodes as drawn (`Scene::node`, `Styles::resolve`): each `$name` is the
        // colour it names, and one the design does not hold is left out, so the default shows.
        let drawn = |colors: &[(&str, &str)], nodes: Vec<Node>| {
            let styles = Styles { colors: colors.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Styles::default() };
            page(nodes.iter().map(|n| styles.resolve(n)).collect())
        };
        let mut email = node("email", Some("p1"), K::Frame, [0.0, 0.0, 300.0, 100.0]);
        email.fill = Some("$paper".into());
        let mut hint = text("hint", "email", [10.0, 10.0, 0.0, 0.0], "Your address");
        hint.color = Some("$muted".into());
        let (s, p) = drawn(&[("muted", "#BBBBBB"), ("paper", "#FFFFFF")], vec![email.clone(), hint.clone()]);
        assert_eq!(all_issues(&s, &p).0[0].message, "hint: #BBBBBB on #FFFFFF (email) is 1.91:1; text this size needs 4.5:1");
        let (s, p) = drawn(&[("paper", "#FFFFFF")], vec![email.clone(), hint.clone()]);
        assert!(all_issues(&s, &p).0.is_empty(), "no $muted: the painter's own colour, dark on white");
        hint.color = Some("#EEEEEE".into());
        let (s, p) = drawn(&[("paper", "#222222")], vec![email.clone(), hint.clone()]);
        assert!(all_issues(&s, &p).0.is_empty(), "light words on a dark $paper");
        let (s, p) = drawn(&[], vec![email, hint.clone()]);
        assert_eq!(all_issues(&s, &p).0[0].message, "hint: #EEEEEE on #FFFFFF (email) is 1.16:1; text this size needs 4.5:1", "no $paper: a frame's white");
        // A colour that does not parse is not read, as the words' colour or as their ground.
        hint.color = Some("#12345".into());
        let (s, p) = on_ground("#FFFFFF", hint.clone());
        assert!(all_issues(&s, &p).0.is_empty());
        hint.color = Some("#EEEEEE".into());
        let (s, p) = on_ground("#12345", hint);
        assert!(all_issues(&s, &p).0.is_empty());
    }

    #[test]
    fn contrast_finds_the_ground_through_turns() {
        // A text in a turned card is still on the card; on the page beside it, it is not.
        let mut card = node("card", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 100.0]);
        (card.rotation, card.fill) = (Some(90.0), Some("#FFFFFF".into()));
        let mut inside = text("inside", "card", [300.0, 40.0, 0.0, 0.0], "On");
        inside.color = Some("#BBBBBB".into());
        let mut beside = text("beside", "p1", [300.0, 40.0, 0.0, 0.0], "Off");
        beside.color = Some("#BBBBBB".into());
        let (s, p) = page(vec![card, inside, beside]);
        assert_eq!(codes(&all_issues(&s, &p)), [("contrast", vec!["inside", "card"])], "the card turned away from (308, 50)");
    }

    #[test]
    fn contrast_finds_grounds_of_any_size_anywhere() {
        // A backdrop over thousands of px, a small tile far out, and a text on each: each finds
        // its own ground wherever it is drawn.
        let mut backdrop = node("backdrop", Some("p1"), K::Rect, [-5000.0, -5000.0, 20000.0, 20000.0]);
        backdrop.fill = Some("#FFFFFF".into());
        let mut tile = node("tile", Some("p1"), K::Rect, [9000.0, 9000.0, 40.0, 40.0]);
        tile.fill = Some("#000000".into());
        let mut near = text("near", "p1", [300.0, 300.0, 0.0, 0.0], "Near");
        near.color = Some("#EEEEEE".into());
        let mut far = text("far", "p1", [9004.0, 9010.0, 0.0, 0.0], "Far");
        far.color = Some("#111111".into());
        let (s, p) = page(vec![backdrop, tile, near, far]);
        assert_eq!(codes(&all_issues(&s, &p)), [("contrast", vec!["near", "backdrop"]), ("contrast", vec!["far", "tile"])]);
    }

    #[test]
    fn issues_of_a_part_are_about_what_is_in_it() {
        let mut hint = text("hint", "email", [10.0, 10.0, 0.0, 0.0], "Your address");
        hint.color = Some("#BBBBBB".into());
        let (s, p) = on_ground("#FFFFFF", hint);
        assert_eq!(issues(&s, &p, &Mono, "p1", &ids(&["hint"])).1, 1);
        assert_eq!(issues(&s, &p, &Mono, "p1", &ids(&["email"])).1, 1, "inside the node asked for");
        let (s, p) = page(vec![node("other", Some("p1"), K::Rect, [500.0, 0.0, 10.0, 10.0])]);
        assert_eq!(issues(&s, &p, &Mono, "p1", &ids(&["other"])).1, 0);
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
    fn rows_and_messages_read_as_numbers() {
        assert_eq!([num(1.0), num(-0.001), num(2.345), num(f64::NAN), num(-3.5)], [json!(1), json!(0), json!(2.35), json!(0), json!(-3.5)]);
        assert_eq!((fmt(12.0), fmt(0.125), fmt(-0.001)), ("12".into(), "0.13".into(), "0".into()));
        assert_eq!((short(4.4999), short(21.0)), ("4.49".into(), "21".into()));
        assert_eq!((hex([255, 0, 16, 255]), hex([0, 0, 0, 128])), ("#FF0010".into(), "#00000080".into()));
        assert_eq!(render_rect(&Rect::new(1.0, 2.0, 3.0, 4.0)), RenderRect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 });
    }

    #[test]
    fn columns_and_picture_cap_are_the_ones_agents_are_told() {
        let doc = relay_bus::schema::render();
        let said = |def: &str, field: &str| doc["$defs"][def]["properties"][field]["description"].as_str().unwrap_or_default().replace('\n', " ");
        let fields = serde_json::to_string(&FIELDS).unwrap();
        assert!(said("DesignRenderBoxes", "fields").contains(&format!("Always {fields}")), "{}", said("DesignRenderBoxes", "fields"));
        assert!(said("DesignRenderIn", "scale").contains(&format!("{} megapixels", MAX_AREA_PX / 1e6)), "{}", said("DesignRenderIn", "scale"));
    }
}
