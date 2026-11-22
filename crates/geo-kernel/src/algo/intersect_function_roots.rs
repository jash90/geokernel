// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRoots.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootNewton.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/roots/RealRootUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::conics::bracketed_root;
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};

pub(super) fn root_near(
    f: &impl Fn(f64) -> f64,
    derivative: impl Fn(f64) -> f64,
    seed: f64,
    inverse: impl FnOnce() -> Option<f64>,
) -> Option<f64> {
    let accepted = |&x: &f64| x.is_finite() && f(x).abs() < MIN_PRECISION;
    if let Some(x) =
        bracketed_root(f, seed - 1.0, seed + 1.0, seed, STANDARD_PRECISION).filter(accepted)
    {
        return Some(x);
    }
    let root = defined_interval(f, seed - 0.5, seed + 0.5).and_then(|(left, right)| {
        bracketed_root(f, left, right, seed, STANDARD_PRECISION)
            .filter(accepted)
            .or_else(|| {
                let start = if f(seed).is_finite() {
                    seed
                } else {
                    (0.9 * left + 0.1 * right + right) / 2.0
                };
                newton(f, derivative, start).filter(accepted)
            })
    });
    root.or_else(|| inverse().filter(accepted))
}

fn newton(f: &impl Fn(f64) -> f64, derivative: impl Fn(f64) -> f64, mut x: f64) -> Option<f64> {
    for _ in 0..50 {
        let next = x - f(x) / derivative(x);
        if (next - x).abs() <= 1e-6 {
            return Some(next);
        }
        if !next.is_finite() {
            return Some(x);
        }
        x = next;
    }
    None
}

fn defined_interval(f: &impl Fn(f64) -> f64, left: f64, right: f64) -> Option<(f64, f64)> {
    match (f(left).is_finite(), f(right).is_finite()) {
        (true, true) => Some((left, right)),
        (true, false) => Some((left, defined_border(f, right, left))),
        (false, true) => Some((defined_border(f, left, right), right)),
        (false, false) => {
            let middle = left.midpoint(right);
            f(middle).is_finite().then(|| {
                (
                    defined_border(f, left, middle),
                    defined_border(f, right, middle),
                )
            })
        }
    }
}

fn defined_border(f: &impl Fn(f64) -> f64, mut bad: f64, mut good: f64) -> f64 {
    for _ in 0..50 {
        let mid = bad.midpoint(good);
        if f(mid).is_finite() {
            good = mid;
        } else {
            bad = mid;
        }
    }
    good
}

/// Check a nearby short decimal and fraction. A nonfinite value there
/// indicates a hole, even if an approximate root was finite.
pub(super) fn check_root(x: f64, f: &impl Fn(f64) -> f64) -> Option<f64> {
    let value = f(x);
    if !value.is_finite() {
        return None;
    }
    let scaled = x * 1e5;
    let rounded = (scaled + 0.5).floor();
    if (scaled - rounded).abs() < 0.1 {
        let candidate = rounded / 1e5;
        let residual = f(candidate);
        if !residual.is_finite() {
            return None;
        }
        if residual.abs() < value.abs() {
            return Some(candidate);
        }
    }
    for denominator in 1..20 {
        let d = f64::from(denominator);
        let numerator = (x * d).round();
        let candidate = numerator / d;
        if numerator.abs() < 999.0 && (x - candidate).abs() < STANDARD_PRECISION {
            let residual = f(candidate);
            if !residual.is_finite() {
                return None;
            }
            if residual.abs() < value.abs() {
                return Some(candidate);
            }
            break;
        }
    }
    x.is_finite().then_some(x)
}

/// AlgoRoots uses one sample per five view pixels. Double the density if
/// crossings fill the grid. The headless view uses 50 pixels per unit.
pub(super) fn interval_roots(
    f: &impl Fn(f64) -> f64,
    a: f64,
    b: f64,
    exhausted: impl Fn() -> bool,
) -> Vec<f64> {
    const MAX_SAMPLES: usize = 400;
    let (left, right) = if a <= b { (a, b) } else { (b, a) };
    let width = right - left;
    if !width.is_finite() || exhausted() {
        return Vec::new();
    }
    if width < STANDARD_PRECISION {
        return if f(left).abs() < STANDARD_PRECISION && !exhausted() {
            vec![left]
        } else {
            Vec::new()
        };
    }
    let mut samples = (width * 10.0).round().clamp(50.0, MAX_SAMPLES as f64) as usize;
    loop {
        let roots = scan(f, left, width / samples as f64, samples, &exhausted);
        if roots.len() < samples / 2 || samples * 2 >= MAX_SAMPLES {
            return roots;
        }
        samples *= 2;
    }
}

fn scan(
    f: &impl Fn(f64) -> f64,
    left: f64,
    step: f64,
    samples: usize,
    exhausted: &impl Fn() -> bool,
) -> Vec<f64> {
    let mut roots = Vec::new();
    let mut sample_root = f64::NEG_INFINITY;
    let mut previous = f64::NAN;
    for i in 0..=samples {
        if exhausted() {
            return Vec::new();
        }
        let x = left + i as f64 * step;
        let y = f(x);
        if opposite(y, previous) {
            if let Some(root) = interval_root(f, x - step, x) {
                if x - sample_root < step {
                    if f(root).abs() < f(sample_root).abs() {
                        roots.pop();
                    } else {
                        previous = y;
                        continue;
                    }
                }
                if let Some(root) = check_root(root, f) {
                    roots.push(root);
                }
            }
        } else if y.abs() < MIN_PRECISION && changes_sign(f, x, (x - sample_root).min(step / 2.0)) {
            sample_root = x;
            if let Some(root) = check_root(x, f) {
                roots.push(root);
            }
        }
        previous = y;
    }
    if exhausted() {
        return Vec::new();
    }
    roots.dedup_by(|a, b| (*a - *b).abs() < STANDARD_PRECISION);
    roots
}

fn interval_root(f: &impl Fn(f64) -> f64, left: f64, right: f64) -> Option<f64> {
    let solve = |a, b| bracketed_root(f, a, b, a + 0.5 * (b - a), 1e-6);
    solve(left, right)
        .or_else(|| {
            let (left, right) = defined_interval(f, left, right)?;
            solve(left, right)
        })
        .filter(|&x| f(x).abs() < MIN_PRECISION)
}

fn opposite(a: f64, b: f64) -> bool {
    (a < 0.0 && b > 0.0) || (a > 0.0 && b < 0.0)
}

fn changes_sign(f: &impl Fn(f64) -> f64, x: f64, max_delta: f64) -> bool {
    let delta = (MIN_PRECISION * 10.0).min(max_delta / 101.0);
    let sample = |step: f64| {
        let mut t = x + step;
        for _ in 0..100 {
            let y = f(t);
            if y.abs() >= delta || y.is_nan() {
                return y;
            }
            t += step;
        }
        f(t)
    };
    opposite(sample(-delta), sample(delta))
}
