//! Real roots of the determinant and elimination polynomials.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/EquationSolver.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::tol::STANDARD_PRECISION;

fn quadratic([c, b, a]: [f64; 3], eps: f64) -> ([f64; 3], usize) {
    if a.abs() < eps {
        if b.abs() < eps {
            return ([0.0; 3], 0);
        }
        return ([-c / b, 0.0, 0.0], 1);
    }
    if b.abs() < eps * a.abs() {
        let square = -c / a;
        if square.abs() < eps {
            return ([0.0; 3], 1);
        }
        if square < 0.0 {
            return ([0.0; 3], 0);
        }
        let root = square.sqrt();
        return ([root, -root, 0.0], 2);
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant.abs() < eps * b * b {
        return ([-b / (2.0 * a), 0.0, 0.0], 1);
    }
    if discriminant < 0.0 {
        return ([0.0; 3], 0);
    }
    let q = -(b + discriminant.sqrt().copysign(b)) / 2.0;
    ([q / a, c / q, 0.0], 2)
}

/// `EquationSolver.solveCubicS`. Its order is significant in the cubic
/// elimination case, while the determinant pencil sorts the real roots.
pub(super) fn cubic(coefficients: [f64; 4], eps: f64) -> ([f64; 3], usize) {
    let [c, b, a, d] = coefficients;
    if d.abs() < eps {
        return quadratic([c, b, a], eps);
    }
    if c == 0.0 {
        let (mut roots, count) = quadratic([b, a, d], eps);
        roots[count] = 0.0;
        return (roots, count + 1);
    }
    let (a, b, c) = (a / d, b / d, c / d);
    let q = a * a - 3.0 * b;
    let r = 2.0 * a * a * a - 9.0 * a * b + 27.0 * c;
    let (q9, r54) = (q / 9.0, r / 54.0);
    let r2 = 729.0 * r * r;
    let q3 = 2916.0 * q * q * q;
    let offset = a / 3.0;
    if r54 == 0.0 && q9 == 0.0 {
        ([-offset; 3], 3)
    } else if (r2 - q3).abs()
        < r2.max(q3) * 10.0 / (2.0 * a + 3.0).abs().max(1.0) * STANDARD_PRECISION
    {
        let sq = q9.sqrt();
        let roots = if r54 > 0.0 {
            [-2.0 * sq - offset, sq - offset, sq - offset]
        } else {
            [-sq - offset, -sq - offset, 2.0 * sq - offset]
        };
        (roots, 3)
    } else if r2 < q3 {
        let sq = q9.sqrt();
        let angle = (r54 / (sq * sq * sq)).clamp(-1.0, 1.0).acos();
        let roots = [0.0, std::f64::consts::TAU, -std::f64::consts::TAU]
            .map(|delta| -2.0 * sq * ((angle + delta) / 3.0).cos() - offset);
        (roots, 3)
    } else {
        let root = -(r54.abs() + (r54 * r54 - q9 * q9 * q9).sqrt())
            .powf(1.0 / 3.0)
            .copysign(r54);
        ([root + q9 / root - offset, 0.0, 0.0], 1)
    }
}

fn evaluate_polynomial(coefficients: &[f64], x: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |value, c| value * x + c)
}

/// Real roots of the quartic, isolated between critical points. The
/// quartic roots are ascending, like the reference Laguerre solver.
fn quartic(coefficients: [f64; 5]) -> ([f64; 4], usize) {
    let [c, b, a, d, e] = coefficients;
    let (mut critical, count) = cubic([b, 2.0 * a, 3.0 * d, 4.0 * e], 0.0);
    critical[..count].sort_by(f64::total_cmp);
    let bound = 1.0 + c.abs().max(b.abs()).max(a.abs()).max(d.abs()) / e.abs();
    let mut left = -bound;
    let mut fl = evaluate_polynomial(&coefficients, left);
    let mut roots = [0.0; 4];
    let mut used = 0;
    for right in critical[..count].iter().copied().chain([bound]) {
        if right <= left || !right.is_finite() {
            continue;
        }
        let fr = evaluate_polynomial(&coefficients, right);
        let zero = fr.abs()
            <= 8.0 * f64::EPSILON * evaluate_polynomial(&coefficients.map(f64::abs), right.abs());
        if !zero && fl != 0.0 && fl.signum() != fr.signum() {
            let mut lo = left;
            let mut hi = right;
            for _ in 0..128 {
                let mid = lo.midpoint(hi);
                if mid == lo || mid == hi {
                    break;
                }
                let fm = evaluate_polynomial(&coefficients, mid);
                if fm == 0.0 {
                    lo = mid;
                    hi = mid;
                    break;
                }
                if fm.signum() == fl.signum() {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            roots[used] = lo.midpoint(hi);
            used += 1;
        } else if zero {
            roots[used] = right;
            used += 1;
        }
        if used == roots.len() {
            break;
        }
        left = right;
        fl = if zero { 0.0 } else { fr };
    }
    (roots, used)
}

pub(crate) fn polynomial_roots(coefficients: [f64; 5]) -> ([f64; 4], usize) {
    let mut roots = [0.0; 4];
    let Some(degree) = coefficients.iter().rposition(|&v| v != 0.0) else {
        return (roots, 0);
    };
    let shift = coefficients.iter().position(|&v| v != 0.0).unwrap_or(0);
    let input = &coefficients[shift..=degree];
    let count = match input {
        [_] => 0,
        &[c, b] => {
            roots[0] = -c / b;
            1
        }
        &[c, b, a] => {
            let (values, count) = quadratic([c, b, a], STANDARD_PRECISION);
            roots[..3].copy_from_slice(&values);
            count
        }
        &[c, b, a, d] => {
            let (values, count) = cubic([c, b, a, d], STANDARD_PRECISION);
            roots[..3].copy_from_slice(&values);
            count
        }
        _ => return quartic(coefficients),
    };
    roots[count] = 0.0;
    (roots, count + usize::from(shift != 0))
}
