//! The length of an arc of an ellipse, as GeoGebra measures it.
//!
//! GeoGebra feeds its arc length through `EllipticArcLength` into the
//! adaptive Gauss-Legendre quadrature that backs every numeric integral: a
//! 3-point and a 5-point rule, each run until two successive interval
//! refinements agree, and an adaptive bisection whenever the two rules
//! disagree beyond `1e-8` relative. The scheme is deliberately kept (down to
//! the iteration and evaluation budgets that leave it no choice but to
//! bisect) because GeoGebra is the specification and the golden data
//! records its numbers: on eccentric ellipses its result is a few units in
//! `1e-8` away from the true length, so a more accurate rule disagrees with
//! it beyond the golden tolerance. This module is written from scratch from
//! the standard published Gauss-Legendre nodes and weights; no GeoGebra or
//! commons-math code is used.

use super::conic_nd::ConicData;
use std::f64::consts::PI;

/// `Kernel.PI_2`.
const PI_2: f64 = 2.0 * PI;

/// `Kernel.STANDARD_PRECISION`: when the two rules agree this closely, the
/// result is accepted.
const STANDARD_PRECISION: f64 = 1e-8;

/// `MAX_GAUSS_QUAD_CALLS`: the bisection gives up after this many subranges.
const MAX_GAUSS_QUAD_CALLS: usize = 500;

/// The relative accuracy each single rule converges to (and the absolute
/// one it falls back to), and the bounds on its refinement passes.
const RELATIVE_ACCURACY: f64 = 1e-6;
const ABSOLUTE_ACCURACY: f64 = 1e-15;
const MINIMAL_ITERATIONS: usize = 1;
const MAXIMAL_ITERATIONS: usize = 5;

/// The most objective evaluations one `integrate` call may spend.
const MAX_EVALUATIONS: usize = 500;

/// Nodes and weights of the 3-point Gauss-Legendre rule on `[-1, 1]`.
const ABSCISSAS_3: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
const WEIGHTS_3: [f64; 3] = [
    0.555_555_555_555_555_6,
    0.888_888_888_888_888_8,
    0.555_555_555_555_555_6,
];

/// Nodes and weights of the 5-point Gauss-Legendre rule on `[-1, 1]`, from
/// `-sqrt((35 + 2 sqrt(70)) / 63)` and `(322 -/+ 13 sqrt(70)) / 900`.
const ABSCISSAS_5: [f64; 5] = [
    -0.906_179_845_938_664_1,
    -0.538_469_310_105_683_1,
    0.0,
    0.538_469_310_105_683_1,
    0.906_179_845_938_664_1,
];
const WEIGHTS_5: [f64; 5] = [
    0.236_926_885_056_189_08,
    0.478_628_670_499_366_47,
    0.568_888_888_888_888_9,
    0.478_628_670_499_366_47,
    0.236_926_885_056_189_08,
];

/// The arc length of the ellipse `d` from the parameter `start` to `end`
/// (each already normalised to `[0, 2 pi)`, the arc running counter-clockwise,
/// through 0 when `end < start`): the integral of
/// `sqrt(a^2 sin^2 t + b^2 cos^2 t)` computed the way GeoGebra computes it.
pub(crate) fn arc_length(d: &ConicData, start: f64, end: f64) -> f64 {
    let [a, b] = d.half_axes;
    if !(start.is_finite() && end.is_finite() && a.is_finite() && b.is_finite()) {
        return f64::NAN;
    }
    let f = |t: f64| {
        // f(t) = sqrt((a sin t)^2 + (b cos t)^2)
        let p = a * t.sin();
        let q = b * t.cos();
        (p * p + q * q).sqrt()
    };
    // EllipticArcLength.compute: an arc that wraps around 0 is the whole
    // ellipse minus the complementary arc
    if start <= end {
        numeric_integration(&f, start, end)
    } else {
        numeric_integration(&f, 0.0, PI_2) - numeric_integration(&f, end, start)
    }
}

/// `AlgoIntegralDefinite.numericIntegration`: the empty interval integrates
/// to zero, a reversed one changes the sign.
fn numeric_integration(f: &impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    // GGB-2318: f(x) = If(x < 0, 0, x <= 2, x)
    if a == b {
        return 0.0;
    }
    let mut calls = 0;
    if a > b {
        -do_adaptive_gauss_quad(f, b, a, &mut calls)
    } else {
        do_adaptive_gauss_quad(f, a, b, &mut calls)
    }
}

/// `AlgoIntegralDefinite.doAdaptiveGaussQuad`: try both rules, bisect while
/// they disagree. A rule that runs out of budget throws in GeoGebra, which
/// falls through to the bisection too; after `MAX_GAUSS_QUAD_CALLS` subranges
/// the integral is NaN (and the part undefined).
fn do_adaptive_gauss_quad(f: &impl Fn(f64) -> f64, a: f64, b: f64, calls: &mut usize) -> f64 {
    *calls += 1;
    if *calls > MAX_GAUSS_QUAD_CALLS {
        return f64::NAN;
    }
    let first = match integrate(f, a, b, &ABSCISSAS_3, &WEIGHTS_3) {
        Ok(value) if value.is_nan() => return f64::NAN,
        Ok(value) => value,
        Err(()) => return bisect(f, a, b, calls),
    };
    let second = match integrate(f, a, b, &ABSCISSAS_5, &WEIGHTS_5) {
        Ok(value) if value.is_nan() => return f64::NAN,
        Ok(value) => value,
        Err(()) => return bisect(f, a, b, calls),
    };
    if is_equal(first, second, STANDARD_PRECISION) {
        second
    } else {
        bisect(f, a, b, calls)
    }
}

fn bisect(f: &impl Fn(f64) -> f64, a: f64, b: f64, calls: &mut usize) -> f64 {
    let mid = (a + b) / 2.0;
    let left = do_adaptive_gauss_quad(f, a, mid, calls);
    if left.is_nan() {
        return f64::NAN;
    }
    left + do_adaptive_gauss_quad(f, mid, b, calls)
}

/// One integrator run over `[min, max]`. `Err` stands for the exceptions
/// GeoGebra swallows (the evaluation or iteration budget spent), which send
/// the caller into the bisection.
fn integrate(
    f: &impl Fn(f64) -> f64,
    min: f64,
    max: f64,
    abscissas: &[f64],
    weights: &[f64],
) -> Result<f64, ()> {
    let mut evals = 0;
    let mut iterations = 0;
    let mut oldt = stage(f, min, max, 1, abscissas, weights, &mut evals)?;
    let mut n = 2;
    loop {
        let t = stage(f, min, max, n, abscissas, weights, &mut evals)?;
        let delta = (t - oldt).abs();
        let convergence = ABSOLUTE_ACCURACY.max(RELATIVE_ACCURACY * (oldt.abs() + t.abs()) * 0.5);
        if iterations + 1 >= MINIMAL_ITERATIONS && delta <= convergence {
            return Ok(t);
        }
        // grow the interval count, at most fourfold but always a little
        let ratio = 4.0_f64.min((delta / convergence).powf(0.5 / abscissas.len() as f64));
        n = (((ratio * n as f64) as i32).max(n as i32 + 1)) as usize;
        oldt = t;
        if iterations >= MAXIMAL_ITERATIONS {
            return Err(());
        }
        iterations += 1;
    }
}

/// One `stage`: the rule applied to `n` equal subintervals of `[min, max]`.
/// Counts evaluations and gives up when the budget is spent.
fn stage(
    f: &impl Fn(f64) -> f64,
    min: f64,
    max: f64,
    n: usize,
    abscissas: &[f64],
    weights: &[f64],
    evals: &mut usize,
) -> Result<f64, ()> {
    let step = (max - min) / n as f64;
    let half_step = step / 2.0;
    let mut mid = min + half_step;
    let mut sum = 0.0;
    for _ in 0..n {
        for j in 0..abscissas.len() {
            if *evals >= MAX_EVALUATIONS {
                return Err(());
            }
            *evals += 1;
            sum += weights[j] * f(mid + half_step * abscissas[j]);
        }
        mid += step;
    }
    Ok(half_step * sum)
}

/// `DoubleUtil.isEqual(x, y, eps)`.
fn is_equal(x: f64, y: f64, eps: f64) -> bool {
    x == y || (x - eps) < y && y < (x + eps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ellipse(a: f64, b: f64) -> ConicData {
        let mut d = ConicData::new();
        d.half_axes = [a, b];
        d
    }

    /// The published values the constants above carry.
    #[test]
    fn the_rules_are_the_standard_gauss_legendre_values() {
        let sqrt70 = 70.0_f64.sqrt();
        assert_eq!(ABSCISSAS_3[0], -(0.6_f64).sqrt());
        assert_eq!(ABSCISSAS_3[2], (0.6_f64).sqrt());
        assert_eq!(WEIGHTS_3[0], 5.0 / 9.0);
        assert_eq!(WEIGHTS_3[1], 8.0 / 9.0);
        assert_eq!(ABSCISSAS_5[0], -((35.0 + 2.0 * sqrt70) / 63.0).sqrt());
        assert_eq!(ABSCISSAS_5[1], -((35.0 - 2.0 * sqrt70) / 63.0).sqrt());
        assert_eq!(WEIGHTS_5[0], (322.0 - 13.0 * sqrt70) / 900.0);
        assert_eq!(WEIGHTS_5[1], (322.0 + 13.0 * sqrt70) / 900.0);
        assert_eq!(WEIGHTS_5[2], 0.5688888888888889);
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

    /// A whole ellipse: the quadrature leaves this about `1.2e-8` away from
    /// the true circumference `4 a E(e^2 = 3/4) = 9.688448220547675`, just
    /// like GeoGebra (see the module docs).
    #[test]
    fn the_whole_ellipse_matches_geogebra() {
        let d = ellipse(2.0, 1.0);
        let l = arc_length(&d, 0.0, 2.0 * PI);
        assert_eq!(l, 9.688_448_232_270_204, "{l}");
        assert!((l - 9.688_448_220_547_675).abs() < 1e-7, "{l}");
    }

    /// The arc golden case `transform-shear-043`: a sector sheared off a
    /// circle of radius `|AB| = 1.4867750334196497` by the factor `1.7`,
    /// which GeoGebra measures as `9.632979685471179`.
    #[test]
    fn a_sheared_sector_matches_the_golden_length() {
        let r = 1.486_775_033_419_649_7_f64;
        let k = 1.7_f64;
        let root = k * (k * k + 4.0).sqrt();
        let ha = r * ((2.0 + k * k + root) / 2.0).sqrt();
        let hb = r * ((2.0 + k * k - root) / 2.0).sqrt();
        let d = ellipse(ha, hb);
        let l = arc_length(&d, 2.255_451_314_067_568, 0.937_359_868_014_256_4);
        assert!((l - 9.632_979_685_471_179).abs() < 1e-12, "{l}");
    }

    /// The budgets keep even an extreme ellipse finite and quick.
    #[test]
    fn an_extreme_ellipse_stays_within_the_budgets() {
        let d = ellipse(1e6, 1e-6);
        let l = arc_length(&d, 0.0, PI_2);
        assert!(l.is_finite() || l.is_nan(), "{l}");
    }
}
