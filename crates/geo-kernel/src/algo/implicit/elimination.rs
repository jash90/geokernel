// Licensed under the EUPL 1.2.

//! Projection and refinement for intersections of polynomial equations.

use super::polynomial::{
    add_scaled, contact_roots, elimination_roots, evaluate, near_roots, product, quotient,
    Polynomial,
};
use super::MAX_OUTPUTS;
use crate::expr::IntersectionWork;
use crate::{ConicData, ConicType};

type Point = (f64, f64);

pub(super) fn conic(
    f: &[Vec<f64>],
    conic: &ConicData,
    work: &IntersectionWork,
    limited: bool,
) -> Option<Vec<Point>> {
    let [a, b, d, h, u, v] = conic.matrix;
    if conic.ty == ConicType::Empty {
        return Some(Vec::new());
    }
    let g = vec![vec![d, 2.0 * v, b], vec![2.0 * u, 2.0 * h], vec![a]];
    if limited {
        // The part uses the full carrier equation. Keep separate candidates
        // around repeated projected roots when its radius comes from points.
        return intersections(f, &g, work, true);
    }
    let degree = f.len().checked_sub(1)?;
    if degree == 0 {
        return constant_in_x(&g, &f[0], work);
    }
    if a == 0.0 && degree >= 2 {
        // The conic carries three x rows even when its quadratic x
        // coefficient vanishes. The reference's elimination has no pivot.
        return Some(Vec::new());
    }
    let projection = if a != 0.0 {
        quadratic_projection(f, &g, a, work)?
    } else {
        let mut p = product(&f[0], &g[1], work)?;
        add_scaled(&mut p, &product(&f[1], &g[0], work)?, -1.0);
        p
    };
    let mut points = Vec::new();
    for y in elimination_roots(&projection, work) {
        let mut slice = g.iter().map(|row| evaluate(row, y)).collect::<Vec<_>>();
        if degree == 1 {
            slice = f.iter().map(|row| evaluate(row, y)).collect();
        }
        for x in elimination_roots(&slice, work) {
            if let Some(pair) = polish((x, y), f, &g, false) {
                insert(&mut points, pair);
                if points.len() > MAX_OUTPUTS {
                    return None;
                }
            }
        }
    }
    Some(points)
}

fn constant_in_x(f: &[Vec<f64>], y_poly: &[f64], work: &IntersectionWork) -> Option<Vec<Point>> {
    let mut points = Vec::new();
    for y in near_roots(y_poly, crate::tol::MIN_PRECISION, work) {
        let slice = f.iter().map(|row| evaluate(row, y)).collect::<Vec<_>>();
        let roots = near_roots(&slice, crate::tol::MIN_PRECISION, work);
        if points.len() + roots.len() > MAX_OUTPUTS {
            return None;
        }
        points.extend(roots.into_iter().map(|x| (x, y)));
    }
    Some(points)
}

/// In the quotient by x² + p(y)x + q(y), the other polynomial has
/// remainder low(y) + x*high(y). Its norm is the projected equation.
fn quadratic_projection(
    f: &[Vec<f64>],
    g: &[Vec<f64>],
    a: f64,
    work: &IntersectionWork,
) -> Option<Polynomial> {
    let p: Vec<_> = g[1].iter().map(|v| v / a).collect();
    let q: Vec<_> = g[0].iter().map(|v| v / a).collect();
    let mut low = Vec::new();
    let mut high = Vec::new();
    for coefficient in f.iter().rev() {
        let mut next_low = coefficient.clone();
        add_scaled(&mut next_low, &product(&high, &q, work)?, -1.0);
        add_scaled(&mut low, &product(&high, &p, work)?, -1.0);
        high = low;
        low = next_low;
    }
    let mut result = product(&low, &low, work)?;
    add_scaled(
        &mut result,
        &product(&product(&low, &high, work)?, &p, work)?,
        -1.0,
    );
    add_scaled(
        &mut result,
        &product(&product(&high, &high, work)?, &q, work)?,
        1.0,
    );
    Some(result)
}

fn polish(mut point: Point, f: &[Vec<f64>], g: &[Vec<f64>], contact: bool) -> Option<Point> {
    let (target_error, minimum_determinant) = if contact {
        // Further refinement can merge numerically separated contacts.
        (
            crate::tol::STANDARD_PRECISION,
            crate::tol::STANDARD_PRECISION,
        )
    } else {
        (1e-13, 0.0)
    };
    let mut before = f64::INFINITY;
    for _ in 0..16 {
        let (x, y) = point;
        let (p, q) = (residual(f, x, y), residual(g, x, y));
        let error = p.abs() + q.abs();
        if error <= target_error || error >= before || !error.is_finite() {
            break;
        }
        let ((px, py), (qx, qy)) = (gradient(f, x, y), gradient(g, x, y));
        let determinant = px * qy - py * qx;
        if determinant.abs() <= minimum_determinant || !determinant.is_finite() {
            break;
        }
        let next = (
            x - (p * qy - q * py) / determinant,
            y - (q * px - p * qx) / determinant,
        );
        if !next.0.is_finite() || !next.1.is_finite() {
            break;
        }
        let next_error = residual(f, next.0, next.1).abs() + residual(g, next.0, next.1).abs();
        if next_error >= error {
            break;
        }
        point = next;
        before = error;
    }
    let (x, y) = point;
    let (u, v) = (x.round(), y.round());
    if !contact
        && (x - u).abs() < crate::tol::MIN_PRECISION
        && (y - v).abs() < crate::tol::MIN_PRECISION
        && residual(f, u, v) == 0.0
        && residual(g, u, v) == 0.0
    {
        return Some((u, v));
    }
    let fits = residual(f, x, y).abs() + residual(g, x, y).abs() < crate::tol::STANDARD_PRECISION;
    fits.then_some(point)
}

fn residual(matrix: &[Vec<f64>], x: f64, y: f64) -> f64 {
    matrix
        .iter()
        .rev()
        .fold(0.0, |sum, row| sum * x + evaluate(row, y))
}

pub(super) fn gradient(matrix: &[Vec<f64>], x: f64, y: f64) -> Point {
    let mut result = (0.0_f64, 0.0_f64);
    for (i, row) in matrix.iter().enumerate().rev() {
        if i != 0 {
            result.0 = result.0 * x + i as f64 * evaluate(row, y);
        }
        let dy = row
            .iter()
            .enumerate()
            .skip(1)
            .rev()
            .fold(0.0_f64, |a, (j, &c)| a * y + c * j as f64);
        result.1 = result.1 * x + dy;
    }
    result
}

fn insert(points: &mut Vec<Point>, point: Point) {
    // Labels follow lexicographic order, with nearly equal x coordinates
    // ordered by their y coordinates.
    const SAME_COORDINATE: f64 = 1e-3;
    let mut slot = points.len();
    for (i, &(x, y)) in points.iter().enumerate() {
        if (x - point.0).abs() <= SAME_COORDINATE && (y - point.1).abs() <= SAME_COORDINATE {
            return;
        }
        if x - point.0 > SAME_COORDINATE || ((x - point.0).abs() <= SAME_COORDINATE && y > point.1)
        {
            slot = i;
            break;
        }
    }
    points.insert(slot, point);
}

/// Eliminate x via a bounded, reduced Sylvester determinant in y.
pub(super) fn pair(f: &[Vec<f64>], g: &[Vec<f64>], work: &IntersectionWork) -> Option<Vec<Point>> {
    intersections(f, g, work, false)
}

pub(super) fn tangent_contacts(
    f: &[Vec<f64>],
    g: &[Vec<f64>],
    work: &IntersectionWork,
    from_point: bool,
) -> Option<Vec<Point>> {
    if !from_point && g.len() == 1 {
        let mut points = Vec::new();
        for point in constant_in_x(f, &g[0], work)? {
            insert(&mut points, point);
        }
        return Some(points);
    }
    if !from_point && g.iter().all(|row| row.iter().skip(1).all(|&v| v == 0.0)) {
        return constant_in_y(f, g, work);
    }
    intersections(f, g, work, from_point)
}

fn intersections(
    f: &[Vec<f64>],
    g: &[Vec<f64>],
    work: &IntersectionWork,
    contact: bool,
) -> Option<Vec<Point>> {
    let (m, n) = (f.len().checked_sub(1)?, g.len().checked_sub(1)?);
    if m == 0 && n == 0 {
        return Some(Vec::new());
    }
    if m == 0 {
        return constant_in_x(g, &f[0], work);
    }
    if n == 0 {
        return constant_in_x(f, &g[0], work);
    }
    let projection = if m >= n {
        sylvester(f, g, work)?
    } else {
        sylvester(g, f, work)?
    };
    if projection.len() < 2 {
        return Some(Vec::new());
    }
    let (lower, higher) = if m < n { (f, g) } else { (g, f) };
    let roots = if contact {
        contact_roots
    } else {
        elimination_roots
    };
    let mut points = Vec::new();
    for y in roots(&projection, work) {
        let mut slice: Vec<_> = lower.iter().map(|row| evaluate(row, y)).collect();
        if slice
            .iter()
            .all(|&value| value.abs() <= crate::tol::STANDARD_PRECISION)
        {
            slice = higher.iter().map(|row| evaluate(row, y)).collect();
        }
        for x in roots(&slice, work) {
            if let Some(pair) = polish((x, y), f, g, contact) {
                insert(&mut points, pair);
                if points.len() > MAX_OUTPUTS {
                    return None;
                }
            }
        }
    }
    Some(points)
}

fn constant_in_y(f: &[Vec<f64>], g: &[Vec<f64>], work: &IntersectionWork) -> Option<Vec<Point>> {
    use super::polynomial::roots;
    let x_poly = g
        .iter()
        .map(|row| row.first().copied().unwrap_or(0.0))
        .collect::<Vec<_>>();
    let mut points = Vec::new();
    for x in roots(&x_poly, work) {
        let mut slice = Vec::new();
        for row in f.iter().rev() {
            for coefficient in &mut slice {
                *coefficient *= x;
            }
            add_scaled(&mut slice, row, 1.0);
        }
        for y in roots(&slice, work) {
            insert(&mut points, (x, y));
            if points.len() > MAX_OUTPUTS {
                return None;
            }
        }
    }
    Some(points)
}

fn sylvester(f: &[Vec<f64>], g: &[Vec<f64>], work: &IntersectionWork) -> Option<Polynomial> {
    const MAX_ROWS: usize = 12;
    let (m, n) = (f.len() - 1, g.len() - 1);
    if n > MAX_ROWS {
        return None;
    }
    let mut remainder = vec![Vec::new(); n - 1];
    remainder.extend(f.iter().cloned());
    let mut matrix = Vec::with_capacity(n);
    // Eliminate one leading x term at a time by a shifted copy of g.
    // Each of the last n steps contributes a row to the reduced matrix.
    for index in (n..m + n).rev() {
        let lead = std::mem::take(&mut remainder[index]);
        let offset = index - n;
        if lead.iter().any(|&v| v != 0.0) {
            let start = offset.min(n - 1);
            for (j, entry) in remainder.iter_mut().enumerate().take(index).skip(start) {
                let mut next = product(entry, &g[n], work)?;
                if j >= offset {
                    add_scaled(&mut next, &product(&g[j - offset], &lead, work)?, -1.0);
                }
                *entry = next;
            }
        }
        if offset < n {
            let mut row = remainder[offset..index].to_vec();
            let mut largest = row.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
            if !largest.is_finite() {
                return None;
            }
            if largest > 10.0 {
                let mut scale = 1.0;
                for _ in 0..f64::MAX_10_EXP {
                    if largest <= 10.0 {
                        break;
                    }
                    largest *= 0.1;
                    scale *= 0.1;
                }
                for value in row.iter_mut().flatten() {
                    *value *= scale;
                }
            }
            matrix.push(row);
        }
    }
    let mut previous = vec![1.0];
    for k in 0..n - 1 {
        let leading = |i: usize| {
            matrix[i][k]
                .iter()
                .rev()
                .find(|v| v.abs() > crate::tol::STANDARD_PRECISION)
                .copied()
                .unwrap_or(0.0)
                .abs()
        };
        let pivot = (k..n).min_by(|&i, &j| leading(j).total_cmp(&leading(i)))?;
        if leading(pivot) <= crate::tol::STANDARD_PRECISION {
            return Some(Vec::new());
        }
        matrix.swap(k, pivot);
        let (top, bottom) = matrix.split_at_mut(k + 1);
        let pivot = &top[k];
        for row in bottom {
            for j in k + 1..n {
                let mut value = product(&row[j], &pivot[k], work)?;
                add_scaled(&mut value, &product(&row[k], &pivot[j], work)?, -1.0);
                row[j] = quotient(value, &previous, work)?;
            }
        }
        previous.clone_from(&pivot[k]);
    }
    matrix.pop()?.pop()
}
