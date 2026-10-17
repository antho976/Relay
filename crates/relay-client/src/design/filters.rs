//! A node's colour `filters` as one affine colour transform: `rgb' = m · rgb + offset`, the
//! adjustments applied in the order they are listed (brightness, contrast, saturation, hue,
//! sepia, invert). The canvas paints it as a colour-matrix node and SVG export writes it as an
//! `feColorMatrix`, so both agree.

use relay_bus::design::Filters;

pub type Affine = ([[f64; 3]; 3], [f64; 3]);

const IDENTITY: Affine = ([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [0.0; 3]);

/// `then` applied after `first`.
fn compose(first: &Affine, then: &Affine) -> Affine {
    let (a, ao) = first;
    let (b, bo) = then;
    let mut m = [[0.0; 3]; 3];
    let mut o = [0.0; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = (0..3).map(|k| b[i][k] * a[k][j]).sum();
        }
        o[i] = (0..3).map(|k| b[i][k] * ao[k]).sum::<f64>() + bo[i];
    }
    (m, o)
}

fn scale(s: f64, add: f64) -> Affine {
    ([[s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]], [add; 3])
}

fn mix(m: [[f64; 3]; 3], amount: f64) -> Affine {
    let mut out = IDENTITY.0;
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = out[i][j] * (1.0 - amount) + m[i][j] * amount;
        }
    }
    (out, [0.0; 3])
}

/// The transform, or `None` when the filters change nothing.
pub fn matrix(f: &Filters) -> Option<Affine> {
    if f.is_none() {
        return None;
    }
    let mut t = IDENTITY;
    if f.brightness < 0.0 {
        t = compose(&t, &scale(1.0 + f.brightness, 0.0));
    } else if f.brightness > 0.0 {
        t = compose(&t, &scale(1.0 - f.brightness, f.brightness));
    }
    if f.contrast != 0.0 {
        let k = 1.0 + f.contrast;
        t = compose(&t, &scale(k, 0.5 - 0.5 * k));
    }
    if f.saturation != 0.0 {
        let s = 1.0 + f.saturation;
        let m = [
            [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
        ];
        t = compose(&t, &(m, [0.0; 3]));
    }
    if f.hue != 0.0 {
        let (sin, cos) = f.hue.to_radians().sin_cos();
        let m = [
            [0.213 + cos * 0.787 - sin * 0.213, 0.715 - cos * 0.715 - sin * 0.715, 0.072 - cos * 0.072 + sin * 0.928],
            [0.213 - cos * 0.213 + sin * 0.143, 0.715 + cos * 0.285 + sin * 0.140, 0.072 - cos * 0.072 - sin * 0.283],
            [0.213 - cos * 0.213 - sin * 0.787, 0.715 - cos * 0.715 + sin * 0.715, 0.072 + cos * 0.928 + sin * 0.072],
        ];
        t = compose(&t, &(m, [0.0; 3]));
    }
    if f.sepia > 0.0 {
        let m = [[0.393, 0.769, 0.189], [0.349, 0.686, 0.168], [0.272, 0.534, 0.131]];
        t = compose(&t, &mix(m, f.sepia));
    }
    if f.invert > 0.0 {
        t = compose(&t, &scale(1.0 - 2.0 * f.invert, f.invert));
    }
    Some(t)
}

/// Apply `t` to a colour, clamped.
pub fn apply(t: &Affine, rgb: [f64; 3]) -> [f64; 3] {
    let (m, o) = t;
    let mut out = [0.0; 3];
    for i in 0..3 {
        out[i] = ((0..3).map(|k| m[i][k] * rgb[k]).sum::<f64>() + o[i]).clamp(0.0, 1.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn each_adjustment_does_what_it_says() {
        assert!(matrix(&Filters::default()).is_none());
        let red = [1.0, 0.0, 0.0];
        let f = |set: fn(&mut Filters)| {
            let mut f = Filters::default();
            set(&mut f);
            matrix(&f).unwrap()
        };
        assert!(near(apply(&f(|f| f.brightness = -1.0), red), [0.0; 3]));
        assert!(near(apply(&f(|f| f.brightness = 1.0), red), [1.0; 3]));
        assert!(near(apply(&f(|f| f.contrast = -1.0), red), [0.5; 3]));
        let grey = apply(&f(|f| f.saturation = -1.0), red);
        assert!(near(grey, [0.213; 3]), "{grey:?}");
        assert!(near(apply(&f(|f| f.invert = 1.0), red), [0.0, 1.0, 1.0]));
        // A third of a turn sends red most of the way to green.
        let turned = apply(&f(|f| f.hue = 120.0), red);
        assert!(turned[1] > turned[0] && turned[1] > turned[2], "{turned:?}");
        // White stays white under saturation and hue.
        assert!(near(apply(&f(|f| { f.saturation = 0.7; f.hue = 45.0 }), [1.0; 3]), [1.0; 3]));
    }
}
