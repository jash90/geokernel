//! Bounded elimination for two real polynomial equations in two variables.

use super::intersect::graph_roots;
use crate::expr::IntersectionWork;
use crate::tol::STANDARD_PRECISION;

pub(crate) const MAX_PLANE_DEGREE: usize = 8;

/// Coefficients in ascending powers of the second variable. Each row is a
/// polynomial in the first variable.
pub(crate) type PlanePolynomial = Vec<Vec<f64>>;

pub(crate) fn evaluation_cost(polynomial: &PlanePolynomial) -> u64 {
    polynomial.len() as u64 + polynomial.iter().map(|row| row.len() as u64).sum::<u64>()
}

pub(crate) fn from_terms(terms: impl IntoIterator<Item = (usize, usize, f64)>) -> PlanePolynomial {
    let mut coefficients = Vec::new();
    for (x, y, value) in terms {
        if value == 0.0 {
            continue;
        }
        coefficients.resize_with(coefficients.len().max(y + 1), Vec::new);
        let row = &mut coefficients[y];
        row.resize(row.len().max(x + 1), 0.0);
        row[x] += value;
    }
    coefficients
}

pub(crate) fn transpose(polynomial: &PlanePolynomial) -> PlanePolynomial {
    from_terms(
        polynomial
            .iter()
            .enumerate()
            .flat_map(|(y, row)| row.iter().enumerate().map(move |(x, &value)| (y, x, value))),
    )
}

fn evaluate(coefficients: &[f64], t: f64) -> f64 {
    coefficients.iter().rev().fold(0.0, |sum, c| sum * t + c)
}

fn slice(polynomial: &PlanePolynomial, x: f64) -> Vec<f64> {
    polynomial.iter().map(|row| evaluate(row, x)).collect()
}

pub(crate) fn value(polynomial: &PlanePolynomial, x: f64, y: f64) -> f64 {
    polynomial
        .iter()
        .rev()
        .fold(0.0, |sum, row| sum * y + evaluate(row, x))
}

fn multiply_add(to: &mut Vec<f64>, a: &[f64], b: &[f64], sign: f64) {
    to.resize(to.len().max(a.len() + b.len() - 1), 0.0);
    for (i, u) in a.iter().enumerate() {
        for (j, v) in b.iter().enumerate() {
            to[i + j] += sign * u * v;
        }
    }
}

fn scaled_rows(polynomial: &PlanePolynomial) -> Option<(PlanePolynomial, f64)> {
    let scale = polynomial
        .iter()
        .flatten()
        .map(|v| v.abs())
        .fold(0.0, f64::max);
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    let rows: PlanePolynomial = polynomial
        .iter()
        .map(|row| row.iter().map(|v| v / scale).collect())
        .collect();
    let row_norm = rows.iter().flatten().map(|v| v.abs()).sum::<f64>();
    row_norm.is_finite().then_some((rows, row_norm))
}

/// Determinant of the Sylvester matrix, expanding rows and reusing minors
/// with the same set of selected columns. A zero determinant indicates a
/// shared component and supplies no isolated intersections.
fn resultant(
    a: &PlanePolynomial,
    b: &PlanePolynomial,
    work: &IntersectionWork,
) -> Option<Vec<f64>> {
    let m = a.len().checked_sub(1)?;
    let n = b.len().checked_sub(1)?;
    let size = m + n;
    if size == 0 {
        return Some(Vec::new());
    }
    if size > MAX_PLANE_DEGREE {
        return None;
    }
    if !work.charge(3 * (evaluation_cost(a) + evaluation_cost(b))) {
        return None;
    }
    let (a, a_norm) = scaled_rows(a)?;
    let (b, b_norm) = scaled_rows(b)?;
    // The product of row norms bounds the sum of absolute determinant terms.
    // Compare against roundoff on that scale before normalizing for root finding.
    let roundoff =
        (size * size) as f64 * f64::EPSILON * a_norm.powi(n as i32) * b_norm.powi(m as i32);
    let mut minors = vec![Vec::<f64>::new(); 1 << size];
    minors[0].push(1.0);
    for mask in 0..(1usize << size) - 1 {
        let row = mask.count_ones() as usize;
        let (polynomial, offset) = if row < n { (&a, row) } else { (&b, row - n) };
        let (used, next) = minors.split_at_mut(mask + 1);
        let current = &used[mask];
        if current.is_empty() {
            continue;
        }
        for (i, coefficient) in polynomial.iter().enumerate() {
            let column = i + offset;
            let bit = 1 << column;
            if mask & bit != 0 || coefficient.is_empty() {
                continue;
            }
            if !work.charge((current.len() * coefficient.len()) as u64) {
                return None;
            }
            let sign = if (mask >> column).count_ones().is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            multiply_add(
                &mut next[(mask | bit) - (mask + 1)],
                current,
                coefficient,
                sign,
            );
        }
    }
    let determinant = minors.pop()?;
    if !work.charge(determinant.len() as u64) {
        return None;
    }
    if determinant.iter().all(|v| v.abs() <= roundoff) {
        return Some(Vec::new());
    }
    Some(determinant)
}

/// Real roots from the coefficients as they are (`EquationSolver`
/// solves a quadratic with its own coefficients): no rescaling that could
/// shrink a small leading coefficient into a lower degree.
pub(crate) fn unscaled_roots(coefficients: &[f64], work: &IntersectionWork) -> Vec<f64> {
    graph_roots(coefficients, &|units| work.charge(units))
}

fn real_roots(coefficients: &[f64], work: &IntersectionWork) -> Vec<f64> {
    let scale = coefficients.iter().fold(0.0_f64, |m, c| m.max(c.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return Vec::new();
    }
    let normalized: Vec<_> = coefficients.iter().map(|c| c / scale).collect();
    graph_roots(&normalized, &|units| work.charge(units))
}

pub(crate) fn intersections(
    a: &PlanePolynomial,
    b: &PlanePolynomial,
    work: &IntersectionWork,
) -> Option<Vec<(f64, f64)>> {
    let scan_cost = evaluation_cost(a) + evaluation_cost(b);
    if !work.charge(scan_cost) {
        return None;
    }
    let xs = match (a.len(), b.len()) {
        (0, _) | (_, 0) | (1, 1) => return Some(Vec::new()),
        (1, _) => real_roots(&a[0], work),
        (_, 1) => real_roots(&b[0], work),
        _ => real_roots(&resultant(a, b, work)?, work),
    };
    let mut points = Vec::new();
    for x in xs {
        if !work.charge(scan_cost) {
            return None;
        }
        let p = slice(a, x);
        let q = slice(b, x);
        let accept = |coefficients: &[f64], y: f64| {
            let residual = evaluate(coefficients, y);
            let scale = evaluate_magnitude(coefficients, y);
            residual.is_finite()
                && scale.is_finite()
                && residual.abs() <= STANDARD_PRECISION * (1.0 + scale)
        };
        // A linear slice gives a well-conditioned second parameter whenever
        // its leading coefficient survives at the first parameter.
        let (first, second) = if p.len() <= q.len() {
            (&p, &q)
        } else {
            (&q, &p)
        };
        let ys = real_roots(first, work);
        let ys = if first.len() == 1
            || (ys.is_empty() && first.iter().all(|v| v.abs() < STANDARD_PRECISION))
        {
            real_roots(second, work)
        } else {
            ys
        };
        for y in ys {
            if accept(&p, y) && accept(&q, y) {
                points.push((x, y));
            }
        }
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    points.dedup_by(|a, b| {
        (a.0 - b.0).abs() < STANDARD_PRECISION && (a.1 - b.1).abs() < STANDARD_PRECISION
    });
    (!work.exhausted()).then_some(points)
}

fn evaluate_magnitude(coefficients: &[f64], t: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |sum, c| sum * t.abs() + c.abs())
}
