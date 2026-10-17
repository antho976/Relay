//! SVG path data (a vector node's `d`) read into absolute moves, lines, cubic curves and closes:
//! every command of the SVG grammar, relative or absolute, with `H`/`V` as lines, `S`/`Q`/`T`
//! as cubics and arcs as cubics. The engine checks a `d` with it; the canvas edits, measures
//! and hit-tests the segments; `write` turns them back into data.

use std::fmt::Write as _;

pub type Pt = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(Pt),
    Line(Pt),
    /// Two controls, then the end.
    Cubic(Pt, Pt, Pt),
    Close,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self) {
        while self.at < self.b.len() && (self.b[self.at].is_ascii_whitespace() || self.b[self.at] == b',') {
            self.at += 1;
        }
    }

    fn command(&mut self) -> Option<u8> {
        self.skip();
        let c = *self.b.get(self.at)?;
        c.is_ascii_alphabetic().then(|| {
            self.at += 1;
            c
        })
    }

    fn number(&mut self) -> Result<f64, String> {
        self.skip();
        let start = self.at;
        let b = self.b;
        let mut i = self.at;
        if i < b.len() && matches!(b[i], b'-' | b'+') {
            i += 1;
        }
        let mut dot = false;
        let mut digits = false;
        while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b'.' && !dot)) {
            dot |= b[i] == b'.';
            digits |= b[i].is_ascii_digit();
            i += 1;
        }
        if digits && i < b.len() && matches!(b[i], b'e' | b'E') {
            let mut j = i + 1;
            if j < b.len() && matches!(b[j], b'-' | b'+') {
                j += 1;
            }
            if j < b.len() && b[j].is_ascii_digit() {
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        if !digits {
            return Err(format!("a number expected at byte {start}"));
        }
        self.at = i;
        let v: f64 = std::str::from_utf8(&b[start..i]).ok().and_then(|t| t.parse().ok()).ok_or_else(|| format!("a bad number at byte {start}"))?;
        if !v.is_finite() || v.abs() > 1e7 {
            return Err(format!("a number out of range at byte {start}"));
        }
        Ok(v)
    }

    /// An arc flag: a lone 0 or 1, which may run into the next number.
    fn flag(&mut self) -> Result<bool, String> {
        self.skip();
        match self.b.get(self.at) {
            Some(b'0') => {
                self.at += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.at += 1;
                Ok(true)
            }
            _ => Err(format!("an arc flag (0 or 1) expected at byte {}", self.at)),
        }
    }
}

/// The segments `d` describes, every point absolute.
pub fn parse(d: &str) -> Result<Vec<Seg>, String> {
    let mut r = Reader { b: d.as_bytes(), at: 0 };
    let mut out = Vec::new();
    let mut cur = [0.0, 0.0];
    let mut start = [0.0, 0.0];
    // The last control point, for S and T.
    let mut last_cubic: Option<Pt> = None;
    let mut last_quad: Option<Pt> = None;
    let mut cmd = None;
    loop {
        let c = match r.command() {
            Some(c) => c,
            None if r.at >= r.b.len() => break,
            None => match cmd {
                // Repeated arguments repeat the command; after a move, they are lines.
                Some(b'M') => b'L',
                Some(b'm') => b'l',
                Some(c) if !matches!(c, b'Z' | b'z') => c,
                _ => return Err(format!("a command expected at byte {}", r.at)),
            },
        };
        if out.is_empty() && !matches!(c, b'M' | b'm') {
            return Err("path data starts with M".into());
        }
        let rel = c.is_ascii_lowercase();
        let base = if rel { cur } else { [0.0, 0.0] };
        let pt = |r: &mut Reader| -> Result<Pt, String> { Ok([base[0] + r.number()?, base[1] + r.number()?]) };
        let (mut cubic, mut quad) = (None, None);
        match c.to_ascii_uppercase() {
            b'M' => {
                cur = pt(&mut r)?;
                start = cur;
                out.push(Seg::Move(cur));
            }
            b'L' => {
                cur = pt(&mut r)?;
                out.push(Seg::Line(cur));
            }
            b'H' => {
                cur = [r.number()? + if rel { cur[0] } else { 0.0 }, cur[1]];
                out.push(Seg::Line(cur));
            }
            b'V' => {
                cur = [cur[0], r.number()? + if rel { cur[1] } else { 0.0 }];
                out.push(Seg::Line(cur));
            }
            b'C' => {
                let (a, b, e) = (pt(&mut r)?, pt(&mut r)?, pt(&mut r)?);
                out.push(Seg::Cubic(a, b, e));
                (cur, cubic) = (e, Some(b));
            }
            b'S' => {
                let a = last_cubic.map_or(cur, |p| [2.0 * cur[0] - p[0], 2.0 * cur[1] - p[1]]);
                let (b, e) = (pt(&mut r)?, pt(&mut r)?);
                out.push(Seg::Cubic(a, b, e));
                (cur, cubic) = (e, Some(b));
            }
            b'Q' => {
                let (q, e) = (pt(&mut r)?, pt(&mut r)?);
                out.push(quad_to_cubic(cur, q, e));
                (cur, quad) = (e, Some(q));
            }
            b'T' => {
                let q = last_quad.map_or(cur, |p| [2.0 * cur[0] - p[0], 2.0 * cur[1] - p[1]]);
                let e = pt(&mut r)?;
                out.push(quad_to_cubic(cur, q, e));
                (cur, quad) = (e, Some(q));
            }
            b'A' => {
                let (rx, ry, rot) = (r.number()?, r.number()?, r.number()?);
                let (large, sweep) = (r.flag()?, r.flag()?);
                let e = pt(&mut r)?;
                arc(&mut out, cur, rx, ry, rot, large, sweep, e);
                cur = e;
            }
            b'Z' => {
                out.push(Seg::Close);
                cur = start;
            }
            other => return Err(format!("{:?} is no path command", other as char)),
        }
        last_cubic = cubic;
        last_quad = quad;
        cmd = Some(c);
        if out.len() > 50_000 {
            return Err("at most 50000 segments".into());
        }
    }
    if out.is_empty() {
        return Err("path data starts with M".into());
    }
    Ok(out)
}

fn quad_to_cubic(p0: Pt, q: Pt, e: Pt) -> Seg {
    let a = [p0[0] + 2.0 / 3.0 * (q[0] - p0[0]), p0[1] + 2.0 / 3.0 * (q[1] - p0[1])];
    let b = [e[0] + 2.0 / 3.0 * (q[0] - e[0]), e[1] + 2.0 / 3.0 * (q[1] - e[1])];
    Seg::Cubic(a, b, e)
}

/// An elliptical arc as cubics (SVG implementation notes, F.6).
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<Seg>, p0: Pt, rx: f64, ry: f64, rot: f64, large: bool, sweep: bool, p1: Pt) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || p0 == p1 {
        out.push(Seg::Line(p1));
        return;
    }
    let phi = rot.to_radians();
    let (s, c) = phi.sin_cos();
    let dx = (p0[0] - p1[0]) / 2.0;
    let dy = (p0[1] - p1[1]) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = (num / den).max(0.0).sqrt();
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (p0[0] + p1[0]) / 2.0;
    let cy = s * cx1 + c * cy1 + (p0[1] + p1[1]) / 2.0;
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let n = (dt.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = dt / n as f64;
    let alpha = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |t: f64| {
        let (st, ct) = t.sin_cos();
        [cx + rx * ct * c - ry * st * s, cy + rx * ct * s + ry * st * c]
    };
    let deriv = |t: f64| {
        let (st, ct) = t.sin_cos();
        [-rx * st * c - ry * ct * s, -rx * st * s + ry * ct * c]
    };
    let mut t = t1;
    for i in 0..n {
        let (a, b) = (point(t), point(t + step));
        let (da, db) = (deriv(t), deriv(t + step));
        let end = if i + 1 == n { p1 } else { b };
        out.push(Seg::Cubic([a[0] + alpha * da[0], a[1] + alpha * da[1]], [b[0] - alpha * db[0], b[1] - alpha * db[1]], end));
        t += step;
    }
}

fn num(out: &mut String, v: f64) {
    let r = (v * 100.0).round() / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let _ = write!(out, "{r}");
}

/// Path data for `segs`, absolute, two decimals.
pub fn write(segs: &[Seg]) -> String {
    let mut out = String::new();
    for s in segs {
        if !out.is_empty() {
            out.push(' ');
        }
        let pts: &[Pt] = match s {
            Seg::Move(p) => {
                out.push('M');
                std::slice::from_ref(p)
            }
            Seg::Line(p) => {
                out.push('L');
                std::slice::from_ref(p)
            }
            Seg::Cubic(a, b, e) => {
                out.push('C');
                &[*a, *b, *e][..]
            }
            Seg::Close => {
                out.push('Z');
                &[]
            }
        };
        for (i, p) in pts.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            num(&mut out, p[0]);
            out.push(' ');
            num(&mut out, p[1]);
        }
    }
    out
}

/// Every point of `segs` mapped through `f`.
pub fn map(segs: &[Seg], f: impl Fn(Pt) -> Pt) -> Vec<Seg> {
    segs.iter()
        .map(|s| match *s {
            Seg::Move(p) => Seg::Move(f(p)),
            Seg::Line(p) => Seg::Line(f(p)),
            Seg::Cubic(a, b, e) => Seg::Cubic(f(a), f(b), f(e)),
            Seg::Close => Seg::Close,
        })
        .collect()
}

/// The path as polylines, one per subpath, curves cut into `per_curve` pieces; and whether
/// each was closed.
pub fn flatten(segs: &[Seg], per_curve: usize) -> Vec<(Vec<Pt>, bool)> {
    let mut out: Vec<(Vec<Pt>, bool)> = Vec::new();
    let mut cur = [0.0, 0.0];
    for s in segs {
        match *s {
            Seg::Move(p) => {
                out.push((vec![p], false));
                cur = p;
            }
            Seg::Line(p) => {
                if let Some(last) = out.last_mut() {
                    last.0.push(p);
                }
                cur = p;
            }
            Seg::Cubic(a, b, e) => {
                if let Some(last) = out.last_mut() {
                    for i in 1..=per_curve.max(1) {
                        let t = i as f64 / per_curve.max(1) as f64;
                        let u = 1.0 - t;
                        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                        last.0.push([
                            w[0] * cur[0] + w[1] * a[0] + w[2] * b[0] + w[3] * e[0],
                            w[0] * cur[1] + w[1] * a[1] + w[2] * b[1] + w[3] * e[1],
                        ]);
                    }
                }
                cur = e;
            }
            Seg::Close => {
                if let Some(last) = out.last_mut() {
                    last.1 = true;
                    if let Some(first) = last.0.first().copied() {
                        cur = first;
                    }
                }
            }
        }
    }
    out
}

/// The box the path's curve covers: `[x, y, w, h]` (controls bound it loosely, so it flattens).
pub fn bounds(segs: &[Seg]) -> Option<[f64; 4]> {
    let lines = flatten(segs, 16);
    let mut it = lines.iter().flat_map(|(p, _)| p.iter());
    let first = it.next()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0], first[1]);
    for p in it {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    Some([x0, y0, x1 - x0, y1 - y0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_reads_absolute() {
        let segs = parse("M10 10 h 10 v10 H0 V0 l5 5 L 1,2 Z").unwrap();
        assert_eq!(segs, vec![
            Seg::Move([10.0, 10.0]), Seg::Line([20.0, 10.0]), Seg::Line([20.0, 20.0]), Seg::Line([0.0, 20.0]),
            Seg::Line([0.0, 0.0]), Seg::Line([5.0, 5.0]), Seg::Line([1.0, 2.0]), Seg::Close,
        ]);
        // Repeated pairs after a move are lines; numbers run together as SVG allows.
        assert_eq!(parse("m0 0 10 0 0 10").unwrap().len(), 3);
        assert_eq!(parse("M0-1.5.5.5").unwrap(), vec![Seg::Move([0.0, -1.5]), Seg::Line([0.5, 0.5])]);
        let curves = parse("M0 0 C 0 10 10 10 10 0 S 20 -10 20 0 Q 25 10 30 0 T 40 0").unwrap();
        assert_eq!(curves.len(), 5);
        assert!(matches!(curves[2], Seg::Cubic([10.0, -10.0], [20.0, -10.0], [20.0, 0.0])));
        // A half circle arc ends where it says.
        let arc = parse("M0 0 A 10 10 0 0 1 20 0").unwrap();
        let Seg::Cubic(_, _, end) = arc[arc.len() - 1] else { panic!() };
        assert!((end[0] - 20.0).abs() < 1e-9 && end[1].abs() < 1e-9);
        let b = bounds(&arc).unwrap();
        assert!((b[3] - 10.0).abs() < 0.2, "the arc bulges 10 up: {b:?}");
        // The flag shorthand: "a1 1 0 00 1 1".
        assert!(parse("M0 0a1 1 0 001 1").is_ok());
    }

    #[test]
    fn bad_data_is_refused_and_writing_reads_back() {
        for bad in ["", "L0 0", "M0", "M 0 0 X 1 1", "M0 0 L1e400 0", "M0 0 A1 1 0 2 0 1 1"] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let segs = parse("M0 0 C 1 2 3 4 5 6 L 7.125 8 Z").unwrap();
        assert_eq!(write(&segs), "M0 0 C1 2 3 4 5 6 L7.13 8 Z");
        assert_eq!(parse(&write(&segs)).unwrap().len(), segs.len());
    }
}
