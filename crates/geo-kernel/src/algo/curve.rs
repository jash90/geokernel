// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePointObject.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/LengthFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoLengthCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoCurveCartesianND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoIntegralDefinite.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::derivative::FunctionDerivative;
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

/// `LengthFunction.integral`: the arc length of the graph of `id` between
/// `a` and `b`. `T = hypot(1, f'(x))` is integrated by GeoGebra's adaptive
/// Gauss quadrature; an end where `T` is infinite but the function has a
/// value is replaced by a short chord.
pub(crate) fn length(c: &Construction, id: Id, a: f64, b: f64) -> Option<f64> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let function = FunctionDerivative::new(c, id)?;
    let f = |x| function.value(x);
    let t = |x| 1.0f64.hypot(function.slope(x));
    let mut min = a;
    let mut max = b;
    let mut edges = 0.0;
    let dx = (b - a) * crate::tol::MAX_PRECISION;
    if t(min).is_infinite() {
        edges += dx.hypot(f(min + dx) - f(min));
        min += dx;
    }
    if t(max).is_infinite() {
        edges += dx.hypot(f(max - dx) - f(max));
        max -= dx;
    }
    Some(super::elliptic::numeric_integration(&t, min, max).abs() + edges)
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

/// A Cartesian curve evaluated with a shared work limit across coordinates.
pub(super) struct Cartesian<'a> {
    x: FunctionDerivative<'a>,
    y: FunctionDerivative<'a>,
    start: f64,
    end: f64,
    work_at_start: u64,
}

impl<'a> Cartesian<'a> {
    pub(super) fn new(c: &'a Construction, id: Id) -> Option<Self> {
        use super::measure::MeasureDef;
        use crate::{Def, Value};
        let Def::Measure(MeasureDef::CartesianCurve {
            coordinates: [x, y],
            ..
        }) = c.def(id)
        else {
            return None;
        };
        let Value::Curve { start, end } = c.value(id) else {
            return None;
        };
        Some(Self {
            x: FunctionDerivative::new(c, Id(x as usize))?,
            y: FunctionDerivative::new(c, Id(y as usize))?,
            start,
            end,
            work_at_start: crate::expr::work_done(),
        })
    }

    fn has_budget(&self) -> bool {
        crate::expr::work_done().wrapping_sub(self.work_at_start) < crate::expr::MAX_EVALUATION_WORK
    }

    pub(super) fn length(&self, start: f64, end: f64) -> f64 {
        let integrand = |t| {
            if !self.has_budget() {
                return f64::NAN;
            }
            let x = self.x.slope(t);
            if !self.has_budget() {
                return f64::NAN;
            }
            let y = self.y.slope(t);
            (x * x + y * y).sqrt()
        };
        super::elliptic::numeric_integration(&integrand, start, end).abs()
    }

    pub(super) fn closest(&self, (px, py): (f64, f64)) -> f64 {
        let squared = |t| {
            if !self.has_budget() {
                return f64::NAN;
            }
            let dx = self.x.value(t) - px;
            if !self.has_budget() {
                return f64::NAN;
            }
            let dy = self.y.value(t) - py;
            dx * dx + dy * dy
        };
        // GeoCurveCartesianND samples before refining one bracket.
        let step = (self.end - self.start) / 100.0;
        let mut t = self.start;
        let mut best = t;
        let mut distance = squared(t);
        for _ in 0..100 {
            t += step;
            let d = squared(t);
            if d < distance || distance.is_nan() {
                best = t;
                distance = d;
            }
        }
        let left = self.start.max(best - step);
        let right = self.end.min(best + step);
        let adjust = |t| {
            if t < self.start {
                t + (self.end - self.start)
            } else if t > self.end {
                t - (self.end - self.start)
            } else {
                t
            }
        };
        let result = adjust(minimum(left, right, squared));
        if self.start < left {
            let alternate = adjust(minimum(self.start - step, self.start + step, squared));
            if squared(alternate) < squared(result) + crate::tol::MIN_PRECISION / 2.0 {
                return alternate;
            }
        }
        result
    }
}

/// Brent minimization with a bounded golden-section fallback.
fn minimum(mut left: f64, mut right: f64, f: impl Fn(f64) -> f64) -> f64 {
    const GOLDEN: f64 = 0.381_966_011_250_105_1;
    let mut x = left + GOLDEN * (right - left);
    let (mut w, mut v) = (x, x);
    let mut fx = f(x);
    let (mut fw, mut fv) = (fx, fx);
    let (mut step, mut span): (f64, f64) = (0.0, 0.0);
    for _ in 0..128 {
        let mid = (left + right) / 2.0;
        let tolerance = f64::EPSILON.sqrt() * x.abs() + crate::tol::MIN_PRECISION / 3.0;
        if (x - mid).abs() <= 2.0 * tolerance - (right - left) / 2.0 {
            break;
        }
        let mut parabolic = false;
        if span.abs() > tolerance {
            let r = (x - w) * (fx - fv);
            let q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            let mut q = 2.0 * (q - r);
            if q > 0.0 {
                p = -p;
            } else {
                q = -q;
            }
            let old_span = span;
            span = step;
            if p.abs() < (0.5 * q * old_span).abs() && p > q * (left - x) && p < q * (right - x) {
                step = p / q;
                let u = x + step;
                if u - left < 2.0 * tolerance || right - u < 2.0 * tolerance {
                    step = tolerance.copysign(mid - x);
                }
                parabolic = true;
            }
        }
        if !parabolic {
            span = if x >= mid { left - x } else { right - x };
            step = GOLDEN * span;
        }
        let u = x + if step.abs() >= tolerance {
            step
        } else {
            tolerance.copysign(step)
        };
        let fu = f(u);
        if fu <= fx {
            if u < x {
                right = x;
            } else {
                left = x;
            }
            (v, fv) = (w, fw);
            (w, fw) = (x, fx);
            (x, fx) = (u, fu);
        } else {
            if u < x {
                left = u;
            } else {
                right = u;
            }
            if fu <= fw || w == x {
                (v, fv) = (w, fw);
                (w, fw) = (u, fu);
            } else if fu <= fv || v == x || v == w {
                (v, fv) = (u, fu);
            }
        }
    }
    x
}
