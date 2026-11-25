//! Screens are immediate: every frame a screen draws itself and asks, per control, "was this
//! tapped?". This holds the one finger the app follows, and the scrolling it drives.

/// Movement beyond this (in units) turns a press into a drag.
const SLOP: f32 = 8.0;
/// A list coasting faster than this (units per second) is stopped by a touch, not tapped through.
const CATCH: f32 = 40.0;
/// A sheet let go after being pulled down past this share of its height closes.
pub const SHEET_CLOSE: f32 = 0.3;
/// So does one flicked down faster than this (units per second).
pub const SHEET_FLICK: f32 = 900.0;

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
    /// When the finger now down came down, on the frames' clock (for a long press): the time of
    /// the first frame that saw it ([`Ui::begin_frame`]), so a long press is timed by the same
    /// clock the frames pass it.
    down_at: Option<u64>,
    /// This frame's time (set by the app before [`Ui::begin_frame`]).
    pub now_ns: u64,
    /// Fixed chrome drawn over the page (tab bar, fixed tops) this frame and the last, screen
    /// units: a tap inside one is held from the page ([`Ui::begin_frame`]) and given back only to
    /// that chrome ([`Ui::chrome`]).
    chrome: Vec<[f32; 4]>,
    chrome_last: Vec<[f32; 4]>,
    /// This frame's tap, held from the page because it fell on chrome (or a dialog was over the
    /// page last frame: then only the dialog gets it back, through [`Ui::cover`]).
    held: Option<(f32, f32)>,
    held_for_cover: bool,
    /// The sheet over the page, pulled down this far by a finger (units), springing back to 0
    /// when let go short of closing ([`Ui::drag_sheet`]).
    sheet_off: f32,
    /// A sheet drew this frame: a pull belongs to it until none is drawn.
    sheet_seen: bool,
    /// The time this frame stands for, in seconds (set by the app before drawing).
    pub dt: f32,
    /// The finger now down sweeps a selection ([`crate::sweep`]): no list scrolls with it.
    claimed: bool,
    /// A second finger down beside the first (a pinch): its id and where it is (units).
    second: Option<(u64, (f32, f32))>,
}

impl Ui {
    /// Feed a touch, already converted to units.
    pub fn touch(&mut self, t: Touch, x: f32, y: f32) {
        match t.phase {
            // A second finger: a pinch. The first is no longer a tap, a press or a drag of the page.
            Phase::Down if self.id.is_some_and(|id| id != t.id) && self.second.is_none() => {
                self.second = Some((t.id, (x, y)));
                self.down = None;
                self.caught.set(true);
            }
            Phase::Move if self.second.is_some_and(|(id, _)| id == t.id) => {
                self.second = Some((t.id, (x, y)));
            }
            Phase::Up | Phase::Cancel if self.second.is_some_and(|(id, _)| id == t.id) => {
                self.second = None;
            }
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
                self.claimed = false;
                self.down_at = None;
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
                self.second = None;
            }
            Phase::Cancel if self.id == Some(t.id) => {
                self.second = None;
                self.id = None;
                self.down = None;
                self.dragging = false;
                self.drag_from = None;
            }
            _ => {}
        }
    }

    /// Before the page draws: a tap that fell on fixed chrome (drawn last frame, or `also`, such
    /// as the tab bar a tabbed page always has) is taken aside, so nothing under it can take it;
    /// the chrome gets it back as it draws ([`Ui::chrome`]). While a dialog or menu was over the
    /// page, every tap is held for it ([`Ui::cover`]).
    pub fn begin_frame(&mut self, also: Option<[f32; 4]>) {
        if self.id.is_some() && self.down_at.is_none() {
            self.down_at = Some(self.now_ns);
        }
        self.held = None;
        self.held_for_cover = false;
        let Some((x, y)) = self.tap else { return };
        let on = |r: &[f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if self.covered_last {
            self.held = self.tap.take();
            self.held_for_cover = true;
        } else if self.chrome_last.iter().chain(also.iter()).any(on) {
            self.held = self.tap.take();
        }
    }

    /// Fixed chrome is drawn over the page here (screen units), after the page: a held tap inside
    /// it is its own again. Call before its controls ask whether they were tapped.
    pub fn chrome(&mut self, r: [f32; 4]) {
        self.chrome.push(r);
        if !self.held_for_cover && Self::inside(self.held, r[0], r[1], r[2], r[3]) {
            self.tap = self.held.take();
        }
    }

    pub fn busy(&self) -> bool {
        self.id.is_some()
    }

    /// Where the finger is now, while one is down (units).
    pub fn finger(&self) -> Option<(f32, f32)> {
        self.id.map(|_| self.pos)
    }

    /// Both fingers of a pinch, while two are down (units).
    pub fn pinch(&self) -> Option<((f32, f32), (f32, f32))> {
        Some((self.finger()?, self.second?.1))
    }

    /// A finger resting without moving: where, and for how long (ns) by `now`.
    pub fn resting(&self, now: u64) -> Option<((f32, f32), u64)> {
        if self.caught.get() {
            return None;
        }
        self.down.zip(self.down_at).map(|(p, at)| (p, now.saturating_sub(at)))
    }

    /// The touch now down is taken (a long press): lifting it is not a tap.
    pub fn swallow(&self) {
        self.caught.set(true);
    }

    /// The finger now down sweeps a selection from here on: it is not a tap, and no list
    /// scrolls or coasts with it ([`crate::sweep`]).
    pub fn claim(&mut self) {
        self.caught.set(true);
        self.claimed = true;
    }

    /// A sweep holds the finger now down (or held the one lifted this frame).
    pub fn claimed(&self) -> bool {
        self.claimed
    }

    /// A sheet resting with its top at `rest_top` (screen units), `height` tall: a drag down that
    /// began on it pulls it down with the finger; let go past [`SHEET_CLOSE`] of its height, or
    /// flicked down, it closes (true); else it springs back. Returns how far down it is now and
    /// whether it closes. A drag that began on it is the sheet's alone: the page under it is held
    /// still ([`Ui::cover`]), and nothing sideways under it moves either ([`HScroll::update`]).
    /// Only a drag that began within `grab` of its top pulls it (a sheet whose list scrolls
    /// inside it is pulled by its head).
    pub fn drag_sheet(&mut self, rest_top: f32, height: f32, grab: f32) -> (f32, bool) {
        self.sheet_seen = true;
        let mine = !self.horizontal && self.drag_from.is_some_and(|(_, y)| y >= rest_top && y < rest_top + grab);
        if mine {
            self.sheet_off = (self.sheet_off + self.drag_dy).max(0.0);
            self.drag_dy = 0.0;
            if let Some(v) = self.fling {
                let pulled = self.sheet_off;
                if pulled > height * SHEET_CLOSE || (v > SHEET_FLICK && pulled > 0.0) {
                    self.sheet_off = 0.0;
                    return (pulled, true);
                }
            }
        } else if self.sheet_off > 0.0 {
            self.sheet_off *= (-self.dt.max(0.0) * 16.0).exp();
            if self.sheet_off < 0.5 {
                self.sheet_off = 0.0;
            }
        }
        (self.sheet_off, false)
    }

    /// A sheet is still springing back: frames keep coming.
    pub fn sheet_moving(&self) -> bool {
        self.sheet_off > 0.0
    }

    /// Forget one-frame events. Call after the screen has drawn.
    pub fn end_frame(&mut self) {
        if !std::mem::take(&mut self.sheet_seen) {
            self.sheet_off = 0.0;
        }
        self.tap = None;
        self.landed = None;
        self.drag_dy = 0.0;
        self.drag_dx = 0.0;
        self.fling = None;
        self.fling_x = None;
        self.hzones_last = std::mem::take(&mut self.hzones);
        self.covered_last = std::mem::take(&mut self.covered);
        self.chrome_last = std::mem::take(&mut self.chrome);
        self.held = None;
        self.held_for_cover = false;
        if self.id.is_none() {
            self.drag_from = None;
            self.horizontal = false;
            self.claimed = false;
        }
    }

    /// A sheet or dialog is over the page this frame: a drag on it must not scroll the page under
    /// it. The page has scrolled by the time its sheet draws, so this holds the page still from
    /// the next frame on (a sheet opens with a tap, never mid-drag).
    pub fn cover(&mut self) {
        self.covered = true;
        // A tap held because the dialog was over the page last frame is the dialog's; so is one
        // held for fixed chrome: the menu or dialog lies over the chrome too (a menu opened in the
        // frame before, so not yet known as a cover when this frame began).
        if self.held.is_some() {
            self.tap = self.held.take();
            self.held_for_cover = false;
        }
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
        // Under a sheet or a menu nothing moves sideways: the drag is the sheet's.
        let mine = !ui.covered_last && !ui.claimed && ui.swiping_in(x, y, w, h);
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
    /// Stop coasting (the reader's strip reached an end the scroller cannot see).
    pub fn stop(&mut self) {
        self.vel = 0.0;
    }

    /// Still coasting.
    pub fn coasting(&self) -> bool {
        self.vel != 0.0
    }

    /// Move with this frame's drag if it began inside the viewport (screen units), then coast.
    /// Returns true while it is still moving.
    pub fn update(&mut self, ui: &Ui, viewport: [f32; 4], dt: f32) -> bool {
        let [x, y, w, h] = viewport;
        let mine = !ui.horizontal && !ui.claimed && ui.drag_from.is_some_and(|(px, py)| px >= x && px < x + w && py >= y && py < y + h);
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
