//! Drag to select in bulk, in every list that selects (a series' chapters, My space › Downloads, a
//! series' downloads): a long press on a row selects it and, without lifting, every row the finger
//! passes over, the range from the row it started on to the row under it now. Dragging back lets go
//! of the rows passed back over (as Google Photos does). A long press on a row already selected
//! de-selects instead. Near the list's top or bottom edge the list scrolls by itself, faster the
//! deeper the finger is in the edge band. While selecting, a fresh long press and drag does the
//! same; a drag that did not start with a long press still scrolls ([`crate::ui::Ui::claim`]).

use std::collections::BTreeSet;

use crate::ui::{Scroll, Ui};

/// The band at the list's top and bottom where a sweep scrolls it (units).
pub const EDGE: f32 = 72.0;
/// How fast it scrolls with the finger at the band's far end (units per second).
pub const EDGE_SPEED: f32 = 1100.0;

/// A row that can be selected, as drawn this frame: its id, top and height (content units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub id: i64,
    pub y: f32,
    pub h: f32,
}

#[derive(Default)]
pub struct Sweep {
    on: Option<On>,
    /// The rows as last drawn (screenshots drive a sweep from them).
    pub rows: Vec<Row>,
}

struct On {
    /// The row the long press landed on.
    anchor: i64,
    /// The sweep selects (true) or de-selects: the anchor was not selected before.
    adds: bool,
    /// The selection before the sweep: rows outside the range go back to it.
    before: BTreeSet<i64>,
    /// The row the range reaches now.
    at: Option<i64>,
}

/// What a frame of sweeping did.
#[derive(Default, Debug, PartialEq)]
pub struct Out {
    /// The list scrolled by itself (frames keep coming).
    pub moving: bool,
    /// Rows newly selected (or de-selected) this frame: a haptic tick each.
    pub ticks: usize,
}

impl Sweep {
    /// A sweep is going on.
    pub fn active(&self) -> bool {
        self.on.is_some()
    }

    pub fn reset(&mut self) {
        self.on = None;
    }

    /// After a list drew its `rows` (every selectable one, in order, off screen too): a long press
    /// on one starts a sweep, the finger extends it, and at the list's edges it scrolls. `band` is
    /// the list's visible top and bottom on screen (units, under the fixed top and over the bar);
    /// `covered`, a menu or dialog lies over the list. `sel` is the list's selection, None when not
    /// selecting: a sweep starts selecting.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, ui: &mut Ui, sc: &mut Scroll, sel: &mut Option<BTreeSet<i64>>, rows: Vec<Row>, band: (f32, f32), covered: bool, now: u64, dt: f32) -> Out {
        self.rows = rows;
        let mut out = Out::default();
        let Some((_, fy)) = ui.finger() else {
            self.on = None;
            return out;
        };
        if self.on.is_none() {
            if covered {
                return out;
            }
            let Some(((_, py), ns)) = ui.resting(now) else { return out };
            if ns < crate::series::LONG_PRESS_NS || py < band.0 || py >= band.1 {
                return out;
            }
            let cy = py + sc.off;
            let Some(r) = self.rows.iter().find(|r| cy >= r.y && cy < r.y + r.h) else { return out };
            ui.claim();
            let before = sel.clone().unwrap_or_default();
            self.on = Some(On { anchor: r.id, adds: !before.contains(&r.id), before, at: None });
        }
        let Some(on) = &mut self.on else { return out };
        // Selecting was stopped from elsewhere (Back), or the row it began on is gone.
        let anchor = self.rows.iter().position(|r| r.id == on.anchor);
        let (Some(anchor), false) = (anchor, on.at.is_some() && sel.is_none()) else {
            self.on = None;
            return out;
        };
        // At the edges, once the finger has moved, the list scrolls by itself.
        let (top, bottom) = band;
        let depth = if fy < top + EDGE {
            -((top + EDGE - fy) / EDGE).min(1.0)
        } else if fy > bottom - EDGE {
            ((fy - (bottom - EDGE)) / EDGE).min(1.0)
        } else {
            0.0
        };
        if depth != 0.0 && ui.drag_from.is_some() {
            let was = sc.off;
            sc.off = (sc.off + depth * EDGE_SPEED * dt.max(0.0)).clamp(0.0, sc.max.max(0.0));
            out.moving = sc.off != was;
        }
        // The row under the finger (within the list: not one hidden under the top or the bar).
        let cy = fy.clamp(top, bottom - 1.0) + sc.off;
        let here = self.rows.iter().rposition(|r| r.y <= cy).unwrap_or(0);
        let id = self.rows[here].id;
        if on.at == Some(id) {
            return out;
        }
        on.at = Some(id);
        let (a, b) = (anchor.min(here), anchor.max(here));
        let mut next = on.before.clone();
        for r in &self.rows[a..=b] {
            if on.adds {
                next.insert(r.id);
            } else {
                next.remove(&r.id);
            }
        }
        let was = sel.clone().unwrap_or_default();
        out.ticks = self.rows[a..=b].iter().filter(|r| was.contains(&r.id) != next.contains(&r.id)).count();
        *sel = Some(next);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Phase, Touch};

    fn rows(n: usize) -> Vec<Row> {
        (0..n).map(|i| Row { id: i as i64, y: 100.0 + 50.0 * i as f32, h: 50.0 }).collect()
    }

    fn touch(ui: &mut Ui, phase: Phase, y: f32, t_ns: u64) {
        ui.touch(Touch { id: 1, phase, x: 100.0, y, t_ns }, 100.0, y);
    }

    #[test]
    fn a_long_press_sweeps_the_range_and_back_lets_go() {
        let mut ui = Ui::default();
        let mut sc = Scroll::default();
        let mut sw = Sweep::default();
        let mut sel = None;
        touch(&mut ui, Phase::Down, 125.0, 0);
        ui.now_ns = 1_000;
        ui.begin_frame(None);
        let late = 1_000 + crate::series::LONG_PRESS_NS;
        let out = sw.update(&mut ui, &mut sc, &mut sel, rows(10), (0.0, 1000.0), false, late, 0.016);
        assert_eq!(sel, Some([0].into_iter().collect()));
        assert_eq!(out.ticks, 1);
        touch(&mut ui, Phase::Move, 280.0, 10_000_000);
        let out = sw.update(&mut ui, &mut sc, &mut sel, rows(10), (0.0, 1000.0), false, late, 0.016);
        assert_eq!(sel, Some((0..=3).collect()));
        assert_eq!(out.ticks, 3);
        touch(&mut ui, Phase::Move, 180.0, 20_000_000);
        sw.update(&mut ui, &mut sc, &mut sel, rows(10), (0.0, 1000.0), false, late, 0.016);
        assert_eq!(sel, Some((0..=1).collect()));
        touch(&mut ui, Phase::Up, 180.0, 30_000_000);
        sw.update(&mut ui, &mut sc, &mut sel, rows(10), (0.0, 1000.0), false, late, 0.016);
        assert!(!sw.active());
        assert!(ui.tap.is_none(), "lifting a sweep is not a tap");
    }
}
