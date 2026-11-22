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

/// Brent's derivative-free minimum search over a bracket, written from the
/// published description of the method (R. P. Brent, *Algorithms for
/// Minimization without Derivatives*, 1973; the parabolic interpolation
/// with a golden-section safeguard that section describes). The search
/// keeps the three best points it has seen, fits a parabola through them
/// and follows its minimizer while the step stays inside the bracket and
/// halves faster than the steps before, and otherwise takes a
/// golden-section step into the farther half of the bracket. It stops when
/// the best point is within the tolerance of the bracket's midpoint, or
/// after 100 trials.
pub(crate) fn brent_minimum(
    mut a: f64,
    mut b: f64,
    tolerance: f64,
    mut f: impl FnMut(f64) -> f64,
    stop: impl Fn(u64) -> bool,
) -> Option<f64> {
    // the golden-section ratio of the method, `(sqrt(5) - 1) / 2` halves
    let ratio = (3.0 - 5.0_f64.sqrt()) * 0.5;
    // the first trial sits a ratio into the bracket from `a`, and counts
    // as the best point and both of its predecessors
    let mut x = a + ratio * (b - a);
    let mut w = x;
    let mut v = x;
    let mut fx = f(x);
    if fx.is_nan() {
        return None;
    }
    let mut fw = fx;
    let mut fv = fx;
    // the step being taken and the one before it
    let mut d = 0.0;
    let mut e = 0.0;
    for _ in 0..101 {
        if stop(1) {
            return None;
        }
        let midpoint = (a + b) * 0.5;
        let tol = 1.2e-16_f64.sqrt() * x.abs() + tolerance / 3.0;
        let twice = 2.0 * tol;
        if (x - midpoint).abs() <= twice - (b - a) * 0.5 {
            return Some(x);
        }
        let (step, before) = next_step(
            NextStep {
                a,
                b,
                x,
                midpoint,
                tol,
                twice,
                d,
                e,
                w,
                v,
                fw,
                fv,
                fx,
                ratio,
            },
        );
        d = step;
        e = before;
        let u = x
            + if d.abs() >= tol {
                d
            } else if d > 0.0 {
                tol
            } else {
                -tol
            };
        let fu = f(u);
        if fu.is_nan() {
            return None;
        }
        // Equal values shrink the bracket at both the trial point and the
        // previous best point. Only a worse trial updates the other samples.
        if fx <= fu {
            if u < x {
                a = u;
            } else {
                b = u;
            }
        }
        if fu <= fx {
            if u >= x {
                a = x;
            } else {
                b = x;
            }
            v = w;
            w = x;
            x = u;
            fv = fw;
            fw = fx;
            fx = fu;
        } else if fu <= fw || w == x {
            v = w;
            fv = fw;
            w = u;
            fw = fu;
        } else if fu <= fv || v == x || v == w {
            v = u;
            fv = fu;
        }
    }
    None
}

/// The state of one step of the minimum search.
struct NextStep {
    a: f64,
    b: f64,
    x: f64,
    midpoint: f64,
    tol: f64,
    twice: f64,
    d: f64,
    e: f64,
    w: f64,
    v: f64,
    fw: f64,
    fv: f64,
    fx: f64,
    ratio: f64,
}

/// The step to take from the best point and the step to remember as the
/// previous one: the parabola through the three best points where it is
/// usable (its minimizer must stay inside the bracket and move less than
/// half the step before the previous one), a golden-section step into the
/// farther half of the bracket otherwise.
fn next_step(s: NextStep) -> (f64, f64) {
    let NextStep {
        a,
        b,
        x,
        midpoint,
        tol,
        twice,
        d,
        e,
        w,
        v,
        fw,
        fv,
        fx,
        ratio,
    } = s;
    if e.abs() > tol {
        // the divided differences of the parabola through `v`, `w` and `x`
        let r = (x - w) * (fx - fv);
        let q = (x - v) * (fx - fw);
        let p = (x - v) * q - (x - w) * r;
        let q = 2.0 * (q - r);
        let p = if q > 0.0 { -p } else { p };
        let q = q.abs();
        let before = e;
        let previous_step = d;
        if p.abs() < (0.5 * q * before).abs() && p > q * (a - x) && p < q * (b - x) {
            let mut step = p / q;
            let u = x + step;
            // never nearer to a bound of the bracket than the tolerance
            if u - a < twice || b - u < twice {
                step = if x >= midpoint { -tol } else { tol };
            }
            return (step, previous_step);
        }
        // the parabola is unusable: remember the step it replaced
        let far = if x >= midpoint { a - x } else { b - x };
        let step = ratio * far;
        return (step, far);
    }
    // too little curvature information: golden-section into the far half
    let far = if x >= midpoint { a - x } else { b - x };
    let step = ratio * far;
    (step, far)
}
