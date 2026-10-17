//! Smart guides: a box being moved or resized lines up with the edges and centres of its
//! neighbours when it comes within a few screen pixels of them.

use super::geom::Rect;

/// A guide line to draw, in page coordinates: vertical at `x = at` from `from` to `to`, or
/// horizontal at `y = at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f64,
    pub from: f64,
    pub to: f64,
}

/// Which edges of the moving box may snap: all three on a move, only the dragged ones on a
/// resize.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    pub left: bool,
    pub center_x: bool,
    pub right: bool,
    pub top: bool,
    pub center_y: bool,
    pub bottom: bool,
}

impl Edges {
    pub const ALL: Edges = Edges { left: true, center_x: true, right: true, top: true, center_y: true, bottom: true };
}

fn xs(r: &Rect) -> [f64; 3] {
    [r.x, r.cx(), r.right()]
}

fn ys(r: &Rect) -> [f64; 3] {
    [r.y, r.cy(), r.bottom()]
}

/// The shift that lines `moving` up with the nearest of `others` within `threshold`, per axis,
/// and the guides that show it.
pub fn snap(moving: &Rect, others: &[Rect], edges: Edges, threshold: f64) -> (f64, f64, Vec<Guide>) {
    let mine_x = [(moving.x, edges.left), (moving.cx(), edges.center_x), (moving.right(), edges.right)];
    let mine_y = [(moving.y, edges.top), (moving.cy(), edges.center_y), (moving.bottom(), edges.bottom)];
    let best = |mine: &[(f64, bool); 3], theirs: &dyn Fn(&Rect) -> [f64; 3]| {
        let mut best: Option<f64> = None;
        for o in others {
            for t in theirs(o) {
                for (m, on) in mine {
                    let d = t - m;
                    if *on && d.abs() <= threshold && best.is_none_or(|b| d.abs() < b.abs()) {
                        best = Some(d);
                    }
                }
            }
        }
        best
    };
    let dx = best(&mine_x, &xs).unwrap_or(0.0);
    let dy = best(&mine_y, &ys).unwrap_or(0.0);
    let moved = moving.translate(dx, dy);
    let mut guides = Vec::new();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    for o in others {
        for t in xs(o) {
            if mine_x.iter().zip(xs(&moved)).any(|((_, on), m)| *on && near(m, t)) {
                guides.push(Guide { vertical: true, at: t, from: moved.y.min(o.y), to: moved.bottom().max(o.bottom()) });
            }
        }
        for t in ys(o) {
            if mine_y.iter().zip(ys(&moved)).any(|((_, on), m)| *on && near(m, t)) {
                guides.push(Guide { vertical: false, at: t, from: moved.x.min(o.x), to: moved.right().max(o.right()) });
            }
        }
    }
    (dx, dy, guides)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_the_nearest_edge_within_reach() {
        let others = [Rect::new(0.0, 0.0, 100.0, 100.0)];
        let moving = Rect::new(103.0, 40.0, 50.0, 20.0);
        let (dx, dy, guides) = snap(&moving, &others, Edges::ALL, 5.0);
        // Left edge to the other's right edge; vertical centres line up too (50 vs 50).
        assert_eq!((dx, dy), (-3.0, 0.0));
        assert!(guides.iter().any(|g| g.vertical && g.at == 100.0));
        assert!(guides.iter().any(|g| !g.vertical && g.at == 50.0));
        let far = Rect::new(120.0, 300.0, 10.0, 10.0);
        assert_eq!(snap(&far, &others, Edges::ALL, 5.0).0, 0.0);
    }

    #[test]
    fn a_resize_snaps_only_the_dragged_edge() {
        let others = [Rect::new(0.0, 0.0, 100.0, 100.0)];
        let moving = Rect::new(2.0, 200.0, 96.0, 10.0);
        let right_only = Edges { left: false, center_x: false, right: true, top: false, center_y: false, bottom: false };
        let (dx, _, _) = snap(&moving, &others, right_only, 5.0);
        assert_eq!(dx, 2.0);
    }
}
