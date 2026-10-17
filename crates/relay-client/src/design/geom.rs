//! Rectangles in page pixels and the camera that maps them to the screen.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }

    /// The box two corners span, whichever way round they are.
    pub fn spanning(a: (f64, f64), b: (f64, f64)) -> Self {
        Rect::new(a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs())
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }

    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x >= self.x && other.y >= self.y && other.right() <= self.right() && other.bottom() <= self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x <= other.right() && other.x <= self.right() && self.y <= other.bottom() && other.y <= self.bottom()
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }

    /// The union of `rects`, or `None` for none.
    pub fn bounds<'a>(rects: impl IntoIterator<Item = &'a Rect>) -> Option<Rect> {
        rects.into_iter().fold(None, |acc: Option<Rect>, r| Some(acc.map_or(*r, |a| a.union(r))))
    }

    pub fn inflate(&self, by: f64) -> Rect {
        Rect::new(self.x - by, self.y - by, self.w + 2.0 * by, self.h + 2.0 * by)
    }

    pub fn translate(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

/// The view onto a page: the page point at the screen's top-left corner, and the scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

/// Zoom stops, as fractions: the canvas goes from 2% to 6400%.
pub const MIN_ZOOM: f64 = 0.02;
pub const MAX_ZOOM: f64 = 64.0;

impl Default for Camera {
    fn default() -> Self {
        Camera { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

impl Camera {
    pub fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        ((x - self.x) * self.zoom, (y - self.y) * self.zoom)
    }

    pub fn to_page(&self, sx: f64, sy: f64) -> (f64, f64) {
        (sx / self.zoom + self.x, sy / self.zoom + self.y)
    }

    pub fn rect_to_screen(&self, r: &Rect) -> Rect {
        let (x, y) = self.to_screen(r.x, r.y);
        Rect::new(x, y, r.w * self.zoom, r.h * self.zoom)
    }

    /// The page area a `width` by `height` screen shows.
    pub fn visible(&self, width: f64, height: f64) -> Rect {
        Rect::new(self.x, self.y, width / self.zoom, height / self.zoom)
    }

    /// Zoom to `zoom` keeping the page point under the screen point `(sx, sy)` where it is.
    pub fn zoom_at(&mut self, sx: f64, sy: f64, zoom: f64) {
        let (px, py) = self.to_page(sx, sy);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.x = px - sx / self.zoom;
        self.y = py - sy / self.zoom;
    }

    pub fn pan(&mut self, dx_screen: f64, dy_screen: f64) {
        self.x -= dx_screen / self.zoom;
        self.y -= dy_screen / self.zoom;
    }

    /// Show all of `r` centred in a `width` by `height` screen with `margin` screen pixels
    /// round it, never past `max_zoom`.
    pub fn fit(r: &Rect, width: f64, height: f64, margin: f64, max_zoom: f64) -> Camera {
        let avail_w = (width - 2.0 * margin).max(1.0);
        let avail_h = (height - 2.0 * margin).max(1.0);
        let zoom = if r.w <= 0.0 || r.h <= 0.0 {
            1.0_f64.min(max_zoom)
        } else {
            (avail_w / r.w).min(avail_h / r.h).min(max_zoom)
        }
        .clamp(MIN_ZOOM, MAX_ZOOM);
        Camera { x: r.cx() - width / 2.0 / zoom, y: r.cy() - height / 2.0 / zoom, zoom }
    }
}

/// The next zoom stop above (`dir` > 0) or below `zoom`, as Figma's keys step it.
pub fn step_zoom(zoom: f64, dir: i32) -> f64 {
    const STOPS: [f64; 15] = [0.02, 0.05, 0.1, 0.125, 0.25, 0.333, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0, 8.0, 16.0, 64.0];
    if dir > 0 {
        STOPS.iter().copied().find(|s| *s > zoom * 1.001).unwrap_or(MAX_ZOOM)
    } else {
        STOPS.iter().rev().copied().find(|s| *s < zoom / 1.001).unwrap_or(MIN_ZOOM)
    }
}

/// Distance from `(px, py)` to the segment from `a` to `b`.
pub fn segment_distance(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Ramer–Douglas–Peucker: the points of a freehand stroke that keep its shape within
/// `tolerance`, so a pen path stores tens of points rather than one per motion event.
pub fn simplify(points: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        let (a, b) = ((points[first][0], points[first][1]), (points[last][0], points[last][1]));
        let mut worst = (0.0, first);
        for (i, p) in points.iter().enumerate().take(last).skip(first + 1) {
            let d = segment_distance(p[0], p[1], a, b);
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > tolerance {
            keep[worst.1] = true;
            stack.push((first, worst.1));
            stack.push((worst.1, last));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let mut cam = Camera { x: 100.0, y: 50.0, zoom: 1.0 };
        let before = cam.to_page(300.0, 200.0);
        cam.zoom_at(300.0, 200.0, 2.5);
        let after = cam.to_page(300.0, 200.0);
        assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
        assert_eq!(cam.zoom, 2.5);
        cam.zoom_at(0.0, 0.0, 1000.0);
        assert_eq!(cam.zoom, MAX_ZOOM);
    }

    #[test]
    fn fit_centres_the_box() {
        let r = Rect::new(0.0, 0.0, 1000.0, 500.0);
        let cam = Camera::fit(&r, 1200.0, 800.0, 100.0, 1.0);
        assert!((cam.zoom - 1.0).abs() < 1e-9);
        let (sx, sy) = cam.to_screen(r.cx(), r.cy());
        assert!((sx - 600.0).abs() < 1e-9 && (sy - 400.0).abs() < 1e-9);
        let big = Camera::fit(&Rect::new(0.0, 0.0, 4000.0, 1000.0), 1200.0, 800.0, 100.0, 1.0);
        assert!((big.zoom - 0.25).abs() < 1e-9);
    }

    #[test]
    fn zoom_steps_both_ways() {
        assert_eq!(step_zoom(1.0, 1), 1.5);
        assert_eq!(step_zoom(1.0, -1), 0.75);
        assert_eq!(step_zoom(0.9, 1), 1.0);
        assert_eq!(step_zoom(64.0, 1), MAX_ZOOM);
    }

    #[test]
    fn simplify_drops_points_on_a_straight_run() {
        let line: Vec<[f64; 2]> = (0..50).map(|i| [i as f64, 0.0]).collect();
        assert_eq!(simplify(&line, 0.5), vec![[0.0, 0.0], [49.0, 0.0]]);
        let corner = vec![[0.0, 0.0], [5.0, 0.1], [10.0, 0.0], [10.0, 5.0], [10.0, 10.0]];
        assert_eq!(simplify(&corner, 0.5), vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
    }

    #[test]
    fn rect_set_operations() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert!(a.intersects(&b));
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(Rect::spanning((10.0, 2.0), (4.0, 8.0)), Rect::new(4.0, 2.0, 6.0, 6.0));
        assert_eq!(Rect::bounds([&a, &b]), Some(Rect::new(0.0, 0.0, 15.0, 15.0)));
        assert!(a.contains_rect(&Rect::new(1.0, 1.0, 2.0, 2.0)));
    }
}
