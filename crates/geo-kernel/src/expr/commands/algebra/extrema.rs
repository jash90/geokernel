// The command wiring follows GeoGebra's EUPL 1.2 AlgoFunctionMinMax
// at commit 112919f2d450ec7430a5cd3b65dba226fdc43b74.

use crate::expr::eval::{Ctx, Env, Val};

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn extremum(&self, minimum: bool, f: u32, a: u32, b: u32) -> Val {
        let (a, b) = (self.full_number(a), self.full_number(b));
        if !a.is_finite() || !b.is_finite() || a >= b {
            return Val::V(f64::NAN, f64::NAN);
        }
        let sample = |x| {
            Ctx {
                env: &Env::Value(x),
                ..*self
            }
            .fast(f)
        };
        let signed = |x| if minimum { sample(x) } else { -sample(x) };
        let result = brent_minimum(a, b, 5e-8, signed, |n| self.stop(n));
        result.map_or(Val::V(f64::NAN, f64::NAN), |x| Val::V(x, sample(x)))
    }
}

/// The tolerance GeoGebra minimizes with (`5e-8`). `None` is an undefined
/// answer of the method itself (an undefined sample or the trial limit),
/// so a maximum is searched as the minimum of the negated function.
pub(crate) fn fmin(a: f64, b: f64, f: impl FnMut(f64) -> f64) -> Option<f64> {
    brent_minimum(a, b, 5e-8, f, |_| false)
}

/// Successive parabolic interpolation safeguarded by golden-section
/// steps, the approach described in Chapter 5 of Brent's *Algorithms
/// for Minimization without Derivatives*. The interval keeps the lowest
/// trial between its bounds. A sorted sample set supplies the parabola;
/// a poor fit or large extrapolation leaves the golden-section step.
pub(crate) fn brent_minimum(
    a: f64,
    b: f64,
    tolerance: f64,
    mut f: impl FnMut(f64) -> f64,
    mut stop: impl FnMut(u64) -> bool,
) -> Option<f64> {
    if !a.is_finite() || !b.is_finite() || a > b {
        return None;
    }
    let ratio = (3.0 - 5.0_f64.sqrt()) / 2.0;
    let mut sample = |x| {
        if stop(1) {
            return None;
        }
        let y: f64 = f(x);
        (!y.is_nan()).then_some(Trial { x, y })
    };
    let first = sample(a + ratio * (b - a))?;
    let unused = Trial {
        x: f64::NAN,
        y: f64::INFINITY,
    };
    let mut best = [first, unused, unused];
    let (mut lower, mut upper) = (a, b);
    for _ in 0..100 {
        let incumbent = best[0];
        let x = incumbent.x;
        let min_step = tolerance / 3.0 + 0.75 * f64::EPSILON.sqrt() * x.abs();
        let span = (x - lower).max(upper - x);
        if span <= 2.0 * min_step {
            return polish_minimum(incumbent, a, b, min_step, &mut sample);
        }

        let fit = parabolic_vertex(best);
        let golden = if x >= (lower + upper) / 2.0 {
            ratio * (lower - x)
        } else {
            ratio * (upper - x)
        };
        let step = if fit > lower + 2.0 * min_step
            && fit < upper - 2.0 * min_step
            && (fit - x).abs() < (1.0 - ratio) * span
        {
            fit - x
        } else {
            golden
        };
        let trial = sample(x + step.abs().max(min_step).copysign(step))?;

        // Unimodality lets us discard the side behind the higher sample.
        // Two equal samples bracket a minimum between their abscissae.
        if trial.y == incumbent.y {
            lower = x.min(trial.x);
            upper = x.max(trial.x);
        } else if (trial.x > x) == (trial.y < incumbent.y) {
            lower = x.min(trial.x);
        } else {
            upper = x.max(trial.x);
        }

        // Retain the three lowest samples, preferring the latest at a tie.
        if let Some(slot) = best.iter().position(|old| trial.y <= old.y) {
            best[slot..].rotate_right(1);
            best[slot] = trial;
        }
    }
    None
}

#[derive(Clone, Copy)]
struct Trial {
    x: f64,
    y: f64,
}

/// Fit a quadratic by divided differences through three distinct points.
fn parabolic_vertex(mut points: [Trial; 3]) -> f64 {
    points.sort_unstable_by(|left, right| left.x.total_cmp(&right.x));
    let [a, b, c] = points;
    let ab = (b.y - a.y) / (b.x - a.x);
    let bc = (c.y - b.y) / (c.x - b.x);
    let curvature = (bc - ab) / (c.x - a.x);
    if curvature > 0.0 {
        a.x.midpoint(b.x) - ab / (2.0 * curvature)
    } else {
        f64::NAN
    }
}

/// Near a smooth interior extremum the objective can round to the same
/// value at several adjacent trials. Estimate the slope with a symmetric
/// five-point stencil and accept a small curvature correction only when
/// it also satisfies the sampled objective.
fn polish_minimum(
    best: Trial,
    a: f64,
    b: f64,
    min_step: f64,
    sample: &mut impl FnMut(f64) -> Option<Trial>,
) -> Option<f64> {
    let x = best.x;
    let step = (f64::EPSILON.cbrt().sqrt() * x.abs().max(1.0)).min(0.25 * (x - a).min(b - x));
    if step < 64.0 * min_step {
        return Some(x);
    }
    let (Some(left), Some(right)) = (sample(x - step), sample(x + step)) else {
        return Some(x);
    };
    if left.y <= best.y || right.y <= best.y {
        return Some(x);
    }
    let (Some(far_left), Some(far_right)) = (sample(x - 2.0 * step), sample(x + 2.0 * step)) else {
        return Some(x);
    };

    let curvature = (left.y - best.y) + (right.y - best.y);
    let slope = (8.0 * (right.y - left.y) - (far_right.y - far_left.y)) / 12.0;
    let correction = step * slope / curvature;
    if correction.abs() <= 4.0 * min_step {
        if let Some(refined) = sample(x - correction).filter(|trial| trial.y <= best.y) {
            return Some(refined.x);
        }
    }
    Some(x)
}
