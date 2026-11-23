// Licensed under the EUPL 1.2.

//! Bounded univariate arithmetic for substitution and elimination.

// The repeated-root retry is adapted from GeoGebra, commit
// 112919f2d450ec7430a5cd3b65dba226fdc43b74, AlgoIntersectImplicitPolynomials.java.
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

use crate::expr::IntersectionWork;
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};

pub(super) type Polynomial = Vec<f64>;
const MAX_DEGREE: usize = 64;

pub(super) fn evaluate(poly: &[f64], x: f64) -> f64 {
    poly.iter().rev().fold(0.0_f64, |value, &c| value * x + c)
}

pub(super) fn add_scaled(poly: &mut Polynomial, other: &[f64], scale: f64) {
    poly.resize(poly.len().max(other.len()), 0.0);
    for (a, &b) in poly.iter_mut().zip(other) {
        *a += scale * b;
    }
    trim(poly);
}

fn trim(poly: &mut Polynomial) {
    let length = poly.iter().rposition(|&a| a != 0.0).map_or(0, |i| i + 1);
    poly.truncate(length);
}

pub(super) fn product(a: &[f64], b: &[f64], work: &IntersectionWork) -> Option<Polynomial> {
    product_with_degree(a, b, work, MAX_DEGREE)
}

pub(super) fn resultant_product(
    a: &[f64],
    b: &[f64],
    work: &IntersectionWork,
) -> Option<Polynomial> {
    // Two supported implicit equations can have a projection of degree
    // up to the product of their degrees. Keep this allowance local to
    // elimination so the substitution budget still applies to graphs.
    const INPUT_DEGREE: usize = crate::expr::equation::implicit::MAX_DEGREE as usize;
    product_with_degree(a, b, work, INPUT_DEGREE * INPUT_DEGREE)
}

fn product_with_degree(
    a: &[f64],
    b: &[f64],
    work: &IntersectionWork,
    max_degree: usize,
) -> Option<Polynomial> {
    let a = &a[..a.iter().rposition(|&v| v != 0.0).map_or(0, |i| i + 1)];
    let b = &b[..b.iter().rposition(|&v| v != 0.0).map_or(0, |i| i + 1)];
    if a.is_empty() || b.is_empty() {
        return Some(Vec::new());
    }
    let length = a.len() + b.len() - 1;
    if length > max_degree + 1 || !work.charge((a.len() * b.len()) as u64) {
        return None;
    }
    let mut result = vec![0.0; length];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            result[i + j] += a * b;
        }
    }
    result.iter().all(|v| v.is_finite()).then_some(result)
}

pub(super) fn quotient(
    mut numerator: Polynomial,
    denominator: &[f64],
    work: &IntersectionWork,
) -> Option<Polynomial> {
    let degree = denominator
        .iter()
        .rposition(|v| v.abs() > STANDARD_PRECISION)?;
    let length = numerator.len().saturating_sub(degree);
    if !work.charge((length * (degree + 1)) as u64) {
        return None;
    }
    let mut result = vec![0.0; length];
    for i in (0..length).rev() {
        let coefficient = numerator[i + degree] / denominator[degree];
        if !coefficient.is_finite() {
            return None;
        }
        result[i] = coefficient;
        for (j, d) in denominator[..degree].iter().enumerate() {
            numerator[i + j] -= coefficient * d;
        }
    }
    trim(&mut result);
    Some(result)
}

pub(super) fn substitute(
    matrix: &[Vec<f64>],
    x: &[f64],
    y: &[f64],
    work: &IntersectionWork,
) -> Option<Polynomial> {
    let mut result = Vec::new();
    for row in matrix.iter().rev() {
        let mut coefficient = Vec::new();
        for &a in row.iter().rev() {
            coefficient = product(&coefficient, y, work)?;
            add_scaled(&mut coefficient, &[a], 1.0);
        }
        result = product(&result, x, work)?;
        add_scaled(&mut result, &coefficient, 1.0);
    }
    Some(result)
}

pub(super) fn roots(poly: &[f64], work: &IntersectionWork) -> Vec<f64> {
    let mut roots = super::super::intersect::graph_roots(poly, &|n| work.charge(n));
    roots.dedup_by(|a, b| (*a - *b).abs() <= MIN_PRECISION);
    roots
}

/// Include stationary roots of the eliminant. Several points can project
/// onto the same root, including at a tangency between the two curves.
pub(super) fn near_roots(poly: &[f64], tolerance: f64, work: &IntersectionWork) -> Vec<f64> {
    near_roots_with(poly, tolerance, work, roots)
}

pub(super) fn contact_roots(poly: &[f64], work: &IntersectionWork) -> Vec<f64> {
    near_roots_with(poly, 0.1, work, projected_roots)
}

fn near_roots_with(
    poly: &[f64],
    tolerance: f64,
    work: &IntersectionWork,
    solve: fn(&[f64], &IntersectionWork) -> Vec<f64>,
) -> Vec<f64> {
    if poly.is_empty() || !work.charge(poly.len() as u64) {
        return Vec::new();
    }
    let degree = poly.iter().rposition(|&v| v != 0.0).unwrap_or(0);
    let poly = &poly[..=degree];
    let mut found = solve(poly, work);
    let derivative: Vec<_> = poly
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, &v)| i as f64 * v)
        .collect();
    let sample = |x| work.charge(poly.len() as u64).then(|| evaluate(poly, x));
    for p in solve(&derivative, work) {
        let Some(residual) = sample(p) else {
            return Vec::new();
        };
        if found.len() < poly.len() && residual.abs() < tolerance {
            found.push(p);
        }
    }
    if found.is_empty() {
        if degree > 0 {
            // GeoGebra retries at the root of the (degree - 1)st derivative.
            let repeated = -poly[degree - 1] / degree as f64 / poly[degree];
            if sample(repeated).is_some_and(|v| v.abs() < STANDARD_PRECISION) {
                return vec![repeated];
            }
        }
        let mut x = 0.0;
        let Some(mut residual) = sample(x) else {
            return Vec::new();
        };
        for _ in 0..32 {
            if !work.charge(degree as u64) {
                return Vec::new();
            }
            let slope = evaluate(&derivative, x);
            if residual.abs() <= STANDARD_PRECISION || slope.abs() < STANDARD_PRECISION {
                break;
            }
            let next = x - residual / slope;
            let Some(value) = sample(next) else {
                return Vec::new();
            };
            if !value.is_finite() || value.abs() >= residual.abs() {
                break;
            }
            x = next;
            residual = value;
        }
        if residual.abs() < tolerance {
            found.push(x);
        }
    }
    found.sort_by(f64::total_cmp);
    found
}

fn projected_roots(poly: &[f64], work: &IntersectionWork) -> Vec<f64> {
    use super::super::intersect::conics::{bracketed_root, complex_roots, polynomial_newton};

    let Some(first) = poly.iter().position(|&v| v != 0.0) else {
        return Vec::new();
    };
    let last = poly.iter().rposition(|&v| v != 0.0).unwrap_or(first);
    if last - first <= 4 {
        return roots(poly, work);
    }
    let poly = &poly[first..=last];
    let derivative = poly
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, c)| i as f64 * c)
        .collect::<Vec<_>>();
    let mut candidates = complex_roots(poly, &|n| work.charge(n));
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let sample = |p: &[f64], x| {
        if work.charge(p.len() as u64) {
            evaluate(p, x)
        } else {
            f64::NAN
        }
    };
    let mut roots = Vec::new();
    for (i, &(x, _)) in candidates.iter().enumerate() {
        let left = i
            .checked_sub(1)
            .map_or(x - 1.0, |j| (x + candidates[j].0) / 2.0);
        let right = candidates.get(i + 1).map_or(x + 1.0, |p| (x + p.0) / 2.0);
        let bracket = |p: &[f64]| {
            bracketed_root(
                &|t| sample(p, t),
                left,
                right,
                left + (right - left) / 2.0,
                1e-6,
            )
        };
        let f = |t| sample(poly, t);
        let mut root = x;
        let refined = if f(left) * f(right) < 0.0 {
            bracket(poly)
        } else {
            polynomial_newton(poly, &derivative, x, &|n| work.charge(n))
        };
        if let Some(refined) = refined.or_else(|| bracket(&derivative)) {
            if f(refined).abs() < f(root).abs() {
                root = refined;
            }
        }
        let scaled = root * 1e5;
        let rounded = scaled.round();
        if (scaled - rounded).abs() < 0.1 && f(rounded / 1e5).abs() < f(root).abs() {
            root = rounded / 1e5;
        }
        if f(root).abs() < 1e-5 * sample(&derivative, root).abs().max(1.0) {
            roots.push(root);
        }
    }
    if first > 0 {
        roots.push(0.0);
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() <= MIN_PRECISION);
    roots
}

pub(super) fn elimination_roots(poly: &[f64], work: &IntersectionWork) -> Vec<f64> {
    let mut found = near_roots(poly, 0.1, work);
    for root in &mut found {
        let integer = root.round();
        if (integer - *root).abs() < MIN_PRECISION
            && work.charge(poly.len() as u64)
            && evaluate(poly, integer) == 0.0
        {
            *root = integer;
        }
    }
    found
}

/// Use the reference's bounded deflation for the larger projections.
/// EquationSolver.laguerreAll discards an incomplete deflation after 100
/// evaluations. Exact zero roots are factored out first.
pub(super) fn pair_roots(poly: &[f64], work: &IntersectionWork) -> Vec<f64> {
    if poly.len() <= MAX_DEGREE + 1 {
        return elimination_roots(poly, work);
    }
    near_roots_with(poly, 0.1, work, |p, work| {
        let mut roots =
            super::super::intersect::conics::polynomial_analysis_roots(p, &|n| work.charge(n));
        roots.dedup_by(|a, b| (*a - *b).abs() <= MIN_PRECISION);
        roots
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newton_fallback_charges_each_step() {
        let work = IntersectionWork::new();
        assert!(work.charge(crate::expr::MAX_EVALUATION_WORK - 13));
        let roots = near_roots_with(&[-2.0, 1.0, 1.0], 0.1, &work, |_, _| Vec::new());
        assert!(roots.is_empty());
        assert!(work.exhausted());
    }
}
