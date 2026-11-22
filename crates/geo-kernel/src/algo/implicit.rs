// Licensed under the EUPL 1.2.

use crate::{Construction, Id, Value};
pub(crate) mod barycentric;
mod elimination;
pub(super) mod intersection;
pub(crate) mod locus_equation;
mod polynomial;
pub(crate) mod tangent;

const MAX_OUTPUTS: usize = 64;

pub(crate) fn asymptotes(
    c: &Construction,
    id: Id,
    charge: &impl Fn(u64) -> bool,
) -> Option<Vec<Value>> {
    let coefficients = c.implicit_coefficients(id)?;
    let degree = c.implicit_degree(id)?;
    let at = |x: usize, y: usize| {
        coefficients
            .get(x)
            .and_then(|row| row.get(y))
            .copied()
            .unwrap_or(0.0)
    };
    let homogeneous = |n| (0..=n).map(|x| at(x, n - x)).collect::<Vec<_>>();
    let leading = homogeneous(degree);
    let directions = ordered_roots(&leading, charge);
    let mut lines = Vec::new();
    for slope in directions {
        // In the chart y = t, x = slope*t + offset, the coefficient
        // of the largest surviving power of t determines the offsets.
        let mut offsets = vec![vec![0.0; degree + 1]; degree + 1];
        for n in 0..=degree {
            for (x, coefficient) in homogeneous(n).into_iter().enumerate() {
                let mut binomial = 1.0;
                for j in 0..=x {
                    offsets[n - j][j] += coefficient * binomial * slope.powi((x - j) as i32);
                    binomial *= (x - j) as f64 / (j + 1) as f64;
                }
            }
        }
        if !charge(((degree + 1).pow(3)) as u64) {
            return None;
        }
        if let Some(poly) = offsets
            .iter_mut()
            .rev()
            .find_map(|poly| clean(poly).then_some(poly))
        {
            lines.extend(
                ordered_roots(poly, charge)
                    .into_iter()
                    .map(|offset| Value::Line {
                        a: 1.0,
                        b: -slope,
                        c: -offset,
                    }),
            );
        }
    }
    // The remaining direction is horizontal. In the chart x = t,
    // the first nonzero row is directly the polynomial in the offset y.
    if let Some(row) = coefficients
        .iter()
        .take(degree + 1)
        .enumerate()
        .rev()
        .find_map(|(x, row)| {
            let mut row = row[..row.len().min(degree - x + 1)].to_vec();
            clean(&mut row).then_some(row)
        })
    {
        lines.extend(
            ordered_roots(&row, charge)
                .into_iter()
                .map(|offset| Value::Line {
                    a: 0.0,
                    b: 1.0,
                    c: -offset,
                }),
        );
    }
    Some(lines)
}

fn clean(poly: &mut [f64]) -> bool {
    for value in poly.iter_mut() {
        if value.abs() <= crate::tol::STANDARD_PRECISION {
            *value = 0.0;
        }
    }
    poly.iter().any(|v| *v != 0.0)
}

fn ordered_roots(coefficients: &[f64], charge: &impl Fn(u64) -> bool) -> Vec<f64> {
    if !charge(coefficients.len() as u64) || coefficients.iter().any(|v| !v.is_finite()) {
        return Vec::new();
    }
    let Some(last) = coefficients.iter().rposition(|v| *v != 0.0) else {
        return Vec::new();
    };
    let first = coefficients.iter().position(|v| *v != 0.0).unwrap_or(last);
    let coefficients = &coefficients[first..=last];
    let mut roots = if coefficients.len() <= 5 {
        let mut poly = [0.0; 5];
        poly[..coefficients.len()].copy_from_slice(coefficients);
        let (roots, count) = super::intersect::conics::polynomial_roots(poly);
        roots[..count].to_vec()
    } else {
        super::intersect::conics::graph_roots(coefficients, charge)
    };
    if first != 0 {
        roots.push(0.0);
    }
    roots.dedup_by(|a, b| (*a - *b).abs() <= crate::tol::STANDARD_PRECISION);
    roots
}
