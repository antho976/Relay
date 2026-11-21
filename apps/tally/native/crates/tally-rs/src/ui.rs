//! Screens are immediate: every frame a screen draws itself and asks, per control, "was this
//! tapped?". This holds the one finger the app follows, and the scrolling it drives.

/// Movement beyond this (in units) turns a press into a drag.
const SLOP: f32 = 8.0;
/// A list coasting faster than this (units per second) is stopped by a touch, not tapped through.
const CATCH: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Down,
    Move,
    Up,
    Cancel,
}

/// A finger (or the mouse), in pixels, on the monotonic clock.
#[derive(Clone, Copy, Debug)]
pub struct Touch {
    pub id: u64,
    pub phase: Phase,
    pub x: f32,
    pub y: f32,
    pub t_ns: u64,
}

#[derive(Default)]
pub struct Ui {
    id: Option<u64>,
    start: (f32, f32),
    pos: (f32, f32),
    dragging: bool,
    /// A finger is resting here and has not started dragging: draw what is under it pressed.
    pub down: Option<(f32, f32)>,
    /// A tap finished here this frame.
    pub tap: Option<(f32, f32)>,
    /// Where the current drag began, while one is going on.
    pub drag_from: Option<(f32, f32)>,
    /// Vertical drag since the last frame, in units (only while the drag is vertical).
    pub drag_dy: f32,
    /// The drag went sideways first: it moves the drawer or a carousel, never the page.
    pub horizontal: bool,
    /// Sideways drag since the last frame, in units (only while the drag is sideways).
    pub drag_dx: f32,
    /// Sideways speed when the finger lifted from a sideways drag, in units per second.
    pub fling_x: Option<f32>,
    velocity_x: f32,
    /// Rows that scroll sideways this frame and the last (screen units): a swipe that starts on
    /// one is that row's, not the drawer's.
    hzones: Vec<[f32; 4]>,
    hzones_last: Vec<[f32; 4]>,
    /// Speed of the drag when the finger lifted, in units per second.
    pub fling: Option<f32>,
    velocity: f32,
    last_move_ns: u64,
    /// A finger came down since the last frame, here: a list coasting under it stops.
    landed: Option<(f32, f32)>,
    /// The finger now down stopped a coasting list: lifting it is not a tap.
    caught: std::cell::Cell<bool>,
    /// A sheet or dialog is over the page this frame ([`Ui::cover`]), and was last frame.
    covered: bool,
    covered_last: bool,
}

impl Ui {
    /// Feed a touch, already converted to units.
    pub fn touch(&mut self, t: Touch, x: f32, y: f32) {
        match t.phase {
            Phase::Down if self.id.is_none() => {
                self.id = Some(t.id);
                self.start = (x, y);
                self.pos = (x, y);
                self.dragging = false;
                self.down = Some((x, y));
                self.velocity = 0.0;
                self.velocity_x = 0.0;
                self.horizontal = false;
                self.last_move_ns = t.t_ns;
                self.landed = Some((x, y));
                self.caught.set(false);
            }
            Phase::Move if self.id == Some(t.id) => {
                let dy = y - self.pos.1;
                let dx = x - self.pos.0;
                let dt = (t.t_ns.saturating_sub(self.last_move_ns)) as f32 / 1e9;
                if dt > 0.0005 {
                    self.velocity = self.velocity * 0.5 + (dy / dt) * 0.5;
                    self.velocity_x = self.velocity_x * 0.5 + (dx / dt) * 0.5;
                }
                self.last_move_ns = t.t_ns;
                self.pos = (x, y);
                if !self.dragging && ((x - self.start.0).abs() > SLOP || (y - self.start.1).abs() > SLOP) {
                    self.dragging = true;
                    self.down = None;
                    self.drag_from = Some(self.start);
                    // The axis is decided once: mostly sideways is a swipe, anything else scrolls.
                    self.horizontal = (x - self.start.0).abs() > (y - self.start.1).abs() * 1.4;
                    if self.horizontal {
                        self.drag_dx += x - self.start.0;
                    } else {
                        self.drag_dy += y - self.start.1;
                    }
                } else if self.dragging {
                    if self.horizontal {
                        self.drag_dx += dx;
                    } else {
                        self.drag_dy += dy;
                    }
                }
            }
            Phase::Up if self.id == Some(t.id) => {
                if self.dragging {
                    // A finger that stopped before lifting does not fling.
                    let still = t.t_ns.saturating_sub(self.last_move_ns) > 80_000_000;
                    if self.horizontal {
                        self.fling_x = Some(if still { 0.0 } else { self.velocity_x });
                    } else {
                        self.fling = Some(if still { 0.0 } else { self.velocity });
                    }
                } else if !self.caught.get() {
                    self.tap = Some(self.start);
                }
                self.id = None;
                self.down = None;
                self.dragging = false;
            }
            Phase::Cancel if self.id == Some(t.id) => {
                self.id = None;
                self.down = None;
                self.dragging = false;
                self.drag_from = None;
            }
            _ => {}
        }
    }

    pub fn busy(&self) -> bool {
        self.id.is_some()
    }

    /// Forget one-frame events. Call after the screen has drawn.
    pub fn end_frame(&mut self) {
        self.tap = None;
        self.landed = None;
        self.drag_dy = 0.0;
        self.drag_dx = 0.0;
        self.fling = None;
        self.fling_x = None;
        self.hzones_last = std::mem::take(&mut self.hzones);
        self.covered_last = std::mem::take(&mut self.covered);
        if self.id.is_none() {
            self.drag_from = None;
            self.horizontal = false;
        }
    }

    /// A sheet or dialog is over the page this frame: a drag on it must not scroll the page under
    /// it. The page has scrolled by the time its sheet draws, so this holds the page still from
    /// the next frame on (a sheet opens with a tap, never mid-drag).
    pub fn cover(&mut self) {
        self.covered = true;
    }

    /// Something covered the page last frame ([`Ui::cover`]): it stays still under it.
    pub fn was_covered(&self) -> bool {
        self.covered_last
    }

    /// Drop everything, so a tap that opened a screen cannot also land on it.
    pub fn reset(&mut self) {
        *self = Ui::default();
    }

    fn inside(p: Option<(f32, f32)>, x: f32, y: f32, w: f32, h: f32) -> bool {
        p.is_some_and(|(px, py)| px >= x && px < x + w && py >= y && py < y + h)
    }

    /// Was this rectangle (screen units) tapped this frame? Takes the tap, so one tap lands once.
    pub fn tapped(&mut self, x: f32, y: f32, w: f32, h: f32) -> bool {
        if self.caught.get() {
            // The touch that stopped a fling (down and up within one frame) is not a tap.
            self.tap = None;
            return false;
        }
        if Self::inside(self.tap, x, y, w, h) {
            self.tap = None;
            true
        } else {
            false
        }
    }

    pub fn pressed(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        !self.caught.get() && Self::inside(self.down, x, y, w, h)
    }

    /// A finger landed in this area (screen units) since the last frame, on something coasting:
    /// it stops there, and that touch is not taken as a tap.
    fn catch(&self, area: [f32; 4], coasting: bool) -> bool {
        let [x, y, w, h] = area;
        let landed = Self::inside(self.landed, x, y, w, h);
        if landed && coasting {
            self.caught.set(true);
        }
        landed
    }

    /// A row that scrolls sideways is drawn here this frame.
    pub fn claim_h(&mut self, r: [f32; 4]) {
        self.hzones.push(r);
    }

    /// The point is on a row that scrolls sideways (as drawn last frame).
    pub fn in_hzone(&self, x: f32, y: f32) -> bool {
        self.hzones_last.iter().chain(self.hzones.iter()).any(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    }

    /// A sideways drag that began inside this rectangle (screen units) is going on or just ended.
    pub fn swiping_in(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.horizontal && Self::inside(self.drag_from, x, y, w, h)
    }
}

/// A sideways scroller for a row of cards: its offset and momentum.
#[derive(Default, Clone, Copy, Debug)]
pub struct HScroll {
    pub off: f32,
    vel: f32,
    pub max: f32,
}

impl HScroll {
    /// Follow a sideways drag that began in `area` (screen units), then coast. True while moving.
    pub fn update(&mut self, ui: &Ui, area: [f32; 4], dt: f32) -> bool {
        let [x, y, w, h] = area;
        let mine = ui.swiping_in(x, y, w, h);
        if ui.catch(area, self.vel.abs() > CATCH) {
            self.vel = 0.0;
        }
        if mine {
            self.off -= ui.drag_dx;
            self.vel = 0.0;
            if let Some(v) = ui.fling_x {
                self.vel = -v;
            }
        } else if self.vel.abs() > 1.0 {
            self.off += self.vel * dt;
            self.vel *= (-dt * 3.2).exp();
        } else {
            self.vel = 0.0;
        }
        let max = self.max.max(0.0);
        if self.off <= 0.0 || self.off >= max {
            self.off = self.off.clamp(0.0, max);
            self.vel = 0.0;
        }
        self.vel != 0.0 || (mine && ui.busy())
    }
}

/// A vertical scroller: its offset, momentum and how far it may go.
#[derive(Default, Clone, Copy, Debug)]
pub struct Scroll {
    pub off: f32,
    vel: f32,
    /// Content height less the viewport's, from the last frame drawn.
    pub max: f32,
}

impl Scroll {
    /// Move with this frame's drag if it began inside the viewport (screen units), then coast.
    /// Returns true while it is still moving.
    pub fn update(&mut self, ui: &Ui, viewport: [f32; 4], dt: f32) -> bool {
        let [x, y, w, h] = viewport;
        let mine = !ui.horizontal && ui.drag_from.is_some_and(|(px, py)| px >= x && px < x + w && py >= y && py < y + h);
        // A finger on a coasting list stops it, as a phone's lists do.
        if ui.catch(viewport, self.vel.abs() > CATCH) {
            self.vel = 0.0;
        }
        if mine {
            self.off -= ui.drag_dy;
            self.vel = 0.0;
            if let Some(v) = ui.fling {
                self.vel = -v;
            }
        } else if self.vel.abs() > 1.0 {
            self.off += self.vel * dt;
            self.vel *= (-dt * 2.6).exp();
        } else {
            self.vel = 0.0;
        }
        // Stop at either end: the owner does not want a page pulled past its top or bottom.
        let max = self.max.max(0.0);
        if self.off <= 0.0 || self.off >= max {
            self.off = self.off.clamp(0.0, max);
            self.vel = 0.0;
        }
        self.vel != 0.0 || (mine && ui.busy())
    }
}
