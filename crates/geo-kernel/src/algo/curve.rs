// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePointObject.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::{Construction, Id};

/// A bracket with a nonpositive left value and a nonnegative right value.
pub(crate) fn bisect(mut lo: f64, mut hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    for _ in 0..60 {
        let mid = lo.midpoint(hi);
        if mid == lo || mid == hi {
            break;
        }
        if f(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo.midpoint(hi)
}

/// Differentiate through the public function evaluator. The symmetric
/// five-point rule cancels the quadratic error of a centered difference.
pub(crate) fn slope(f: impl Fn(f64) -> f64, x: f64) -> f64 {
    let h = 0.0005 * x.abs().max(1.0);
    let coarse = f(x + 2.0 * h) - f(x - 2.0 * h);
    let fine = f(x + h) - f(x - h);
    (8.0 * fine - coarse) / (12.0 * h)
}

/// Search for stationary points of squared distance in the interval that
/// can improve on the point at the source's x coordinate.
pub(crate) fn closest(c: &Construction, id: Id, (px, py): (f64, f64)) -> Option<(f64, f64)> {
    let f = |x| c.function_value(id, x).unwrap_or(f64::NAN);
    let squared = |x: f64| (x - px).powi(2) + (f(x) - py).powi(2);
    let mut best = px;
    let mut distance = squared(px);
    if !distance.is_finite() {
        let boundary = |sign: f64| {
            let mut offset = sign * 0.1;
            while offset.abs() < 1e15 {
                let mut to = px + offset;
                if !f(to).is_nan() {
                    let mut from = px + offset / 2.0;
                    for _ in 0..15 {
                        let mid = from.midpoint(to);
                        if squared(mid).is_nan() || squared(to) < squared(mid) {
                            from = mid;
                        } else {
                            to = mid;
                        }
                    }
                    return to;
                }
                offset *= 2.0;
            }
            f64::NAN
        };
        let (left, right) = (boundary(-1.0), boundary(1.0));
        best = if squared(left) < squared(right) {
            left
        } else {
            right
        };
    } else if distance > 0.0 {
        let radius = distance.sqrt();
        let intervals = (20.0 * radius).clamp(200.0, 10_000.0) as u32;
        let derivative = |x| x - px + (f(x) - py) * slope(f, x);
        let mut left = px - radius;
        let mut dl = derivative(left);
        for i in 1..=intervals {
            let right = px + radius * (2.0 * f64::from(i) / f64::from(intervals) - 1.0);
            let dr = derivative(right);
            if dl <= 0.0 && dr >= 0.0 {
                let root = bisect(left, right, derivative);
                let value = squared(root);
                if value < distance {
                    best = root;
                    distance = value;
                }
            }
            (left, dl) = (right, dr);
        }
    }
    let y = f(best);
    (best.is_finite() && y.is_finite()).then_some((best, y))
}
