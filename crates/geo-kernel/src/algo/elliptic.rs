//! The length of an arc of an ellipse.
//!
//! GeoGebra measures it with its own numerical integrator, which is not
//! available under the EUPL; this is an independent adaptive Gauss-Kronrod
//! quadrature of the same integral, accurate to a few units in the last
//! place, so it agrees with GeoGebra wherever GeoGebra's result is accurate
//! to the golden tolerance (see `DIFFERENCES.md`).

use super::conic_nd::ConicData;
use std::f64::consts::PI;

/// Nodes of the 15-point Kronrod rule on `[-1, 1]` (the odd ones are the
/// 7-point Gauss nodes), from 0 outwards.
const XK: [f64; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_4,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_5,
    0.0,
];
const WK: [f64; 8] = [
    0.022_935_322_010_529_22,
    0.063_092_092_629_978_55,
    0.104_790_010_322_250_2,
    0.140_653_259_715_525_9,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_4,
    0.204_432_940_075_298_9,
    0.209_482_141_084_727_8,
];
/// Weights of the 7-point Gauss rule for the nodes `XK[1]`, `XK[3]`,
/// `XK[5]` and `XK[7]`.
const WG: [f64; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_7,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

/// The arc length of the ellipse `d` from the parameter `start` to `end`
/// (each in `[0, 2 pi]`, the arc running counter-clockwise, through 0 when
/// `end < start`): the integral of `sqrt(a^2 sin^2 t + b^2 cos^2 t)`.
pub(crate) fn arc_length(d: &ConicData, start: f64, end: f64) -> f64 {
    let [a, b] = d.half_axes;
    let end = if end < start { end + 2.0 * PI } else { end };
    let f = |t: f64| {
        let (s, c) = t.sin_cos();
        (a * a * s * s + b * b * c * c).sqrt()
    };
    if !(start.is_finite() && end.is_finite() && a.is_finite() && b.is_finite()) {
        return f64::NAN;
    }
    // split at every quarter turn, where the integrand has its extrema
    let mut total = 0.0;
    let mut lo = start;
    while lo < end {
        let next = ((lo / (PI / 2.0)).floor() + 1.0) * (PI / 2.0);
        let hi = next.min(end);
        total += adaptive(&f, lo, hi, 0);
        lo = hi;
    }
    total
}

/// One Gauss-Kronrod step: the Kronrod estimate and its error.
fn kronrod(f: &impl Fn(f64) -> f64, lo: f64, hi: f64) -> (f64, f64) {
    let mid = 0.5 * (lo + hi);
    let half = 0.5 * (hi - lo);
    let centre = f(mid);
    let mut k = WK[7] * centre;
    let mut g = WG[3] * centre;
    for i in 0..7 {
        let dx = half * XK[i];
        let sum = f(mid - dx) + f(mid + dx);
        k += WK[i] * sum;
        if i % 2 == 1 {
            g += WG[i / 2] * sum;
        }
    }
    (k * half, ((k - g) * half).abs())
}

fn adaptive(f: &impl Fn(f64) -> f64, lo: f64, hi: f64, depth: u32) -> f64 {
    let (k, err) = kronrod(f, lo, hi);
    if depth >= 30 || err <= 1e-15 * k.abs().max(f64::MIN_POSITIVE) {
        return k;
    }
    let mid = 0.5 * (lo + hi);
    adaptive(f, lo, mid, depth + 1) + adaptive(f, mid, hi, depth + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ellipse(a: f64, b: f64) -> ConicData {
        let mut d = ConicData::new();
        d.half_axes = [a, b];
        d
    }

    #[test]
    fn a_circle_gives_radius_times_angle() {
        let d = ellipse(2.0, 2.0);
        let l = arc_length(&d, 0.5, 2.0);
        assert!((l - 3.0).abs() < 1e-14, "{l}");
        // through 0
        let l = arc_length(&d, 6.0, 0.5);
        assert!((l - 2.0 * (0.5 + 2.0 * PI - 6.0)).abs() < 1e-13, "{l}");
    }

    #[test]
    fn the_whole_ellipse_matches_the_complete_elliptic_integral() {
        // circumference of a = 2, b = 1: 4 a E(e^2 = 3/4) = 9.688448220547675...
        let d = ellipse(2.0, 1.0);
        let l = arc_length(&d, 0.0, 2.0 * PI);
        assert!((l - 9.688_448_220_547_675).abs() < 1e-12, "{l}");
    }
}
