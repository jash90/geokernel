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
        let result = brent(a, b, signed, |n| self.stop(n));
        result.map_or(Val::V(f64::NAN, f64::NAN), |x| Val::V(x, sample(x)))
    }
}

/// Brent minimization with a golden-section step whenever interpolation
/// falls outside the bracket or would make insufficient progress.
fn brent(
    mut a: f64,
    mut b: f64,
    f: impl Fn(f64) -> f64,
    stop: impl Fn(u64) -> bool,
) -> Option<f64> {
    let ratio = (3.0 - 5.0_f64.sqrt()) * 0.5;
    let mut x = a + ratio * (b - a);
    let (mut w, mut v) = (x, x);
    let mut fx = f(x);
    let (mut fw, mut fv) = (fx, fx);
    let (mut d, mut e): (f64, f64) = (0.0, 0.0);
    for _ in 0..1024 {
        if stop(1) {
            return None;
        }
        let midpoint = (a + b) * 0.5;
        let tol = 1.2e-16_f64.sqrt() * x.abs() + 5.0e-8 / 3.0;
        let twice = 2.0 * tol;
        if (x - midpoint).abs() <= twice - (b - a) * 0.5 {
            return Some(x);
        }
        let mut interpolate = false;
        if e.abs() > tol {
            let r = (x - w) * (fx - fv);
            let q = (x - v) * (fx - fw);
            let p = (x - v) * q - (x - w) * r;
            let q = 2.0 * (q - r);
            let p = if q > 0.0 { -p } else { p };
            let q = q.abs();
            let previous = e;
            e = d;
            if p.abs() < (0.5 * q * previous).abs() && p > q * (a - x) && p < q * (b - x) {
                d = p / q;
                let u = x + d;
                if u - a < twice || b - u < twice {
                    d = tol.copysign(midpoint - x);
                }
                interpolate = true;
            }
        }
        if !interpolate {
            e = if x >= midpoint { a - x } else { b - x };
            d = ratio * e;
        }
        let u = x + if d.abs() >= tol { d } else { tol.copysign(d) };
        let fu = f(u);
        if fu <= fx {
            if u >= x {
                a = x;
            } else {
                b = x;
            }
            (v, w, x) = (w, x, u);
            (fv, fw, fx) = (fw, fx, fu);
        } else {
            if u < x {
                a = u;
            } else {
                b = u;
            }
            if fu <= fw || w == x {
                (v, w) = (w, u);
                (fv, fw) = (fw, fu);
            } else if fu <= fv || v == x || v == w {
                (v, fv) = (u, fu);
            }
        }
    }
    None
}
