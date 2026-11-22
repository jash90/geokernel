//! Bounded univariate arithmetic for substitution and elimination.

use crate::expr::IntersectionWork;

pub(super) type Polynomial = Vec<f64>;
const MAX_DEGREE: usize = 64;

pub(super) fn evaluate(poly: &[f64], x: f64) -> f64 {
    poly.iter()
        .rev()
        .fold(0.0_f64, |value, &c| value.mul_add(x, c))
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
    let a = &a[..a.iter().rposition(|&v| v != 0.0).map_or(0, |i| i + 1)];
    let b = &b[..b.iter().rposition(|&v| v != 0.0).map_or(0, |i| i + 1)];
    if a.is_empty() || b.is_empty() {
        return Some(Vec::new());
    }
    let length = a.len() + b.len() - 1;
    if length > MAX_DEGREE + 1 || !work.charge((a.len() * b.len()) as u64) {
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
    roots.dedup_by(|a, b| (*a - *b).abs() <= crate::tol::MIN_PRECISION);
    roots
}

/// Include stationary roots of the eliminant. Several points can project
/// onto the same root, including at a tangency between the two curves.
pub(super) fn elimination_roots(poly: &[f64], work: &IntersectionWork) -> Vec<f64> {
    let mut found = roots(poly, work);
    let derivative: Vec<_> = poly
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, &v)| i as f64 * v)
        .collect();
    for p in roots(&derivative, work) {
        let magnitude = poly
            .iter()
            .rev()
            .fold(0.0_f64, |v, &c| v * p.abs() + c.abs());
        if magnitude.is_finite() && evaluate(poly, p).abs() <= 64.0 * f64::EPSILON * magnitude {
            found.retain(|&q| (p - q).abs() > 1e-5 * p.abs().max(1.0));
            found.push(p);
        }
    }
    found.sort_by(f64::total_cmp);
    found
}
