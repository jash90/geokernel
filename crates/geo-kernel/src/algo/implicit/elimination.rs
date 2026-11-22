//! Projection and refinement for intersections of polynomial equations.

use super::polynomial::{add_scaled, elimination_roots, evaluate, product, roots, Polynomial};
use crate::expr::IntersectionWork;
use crate::plot::implicit;
use crate::{ConicData, ConicType};

type Point = (f64, f64);

pub(super) fn conic(f: &[Vec<f64>], conic: &ConicData, work: &IntersectionWork) -> Vec<Point> {
    intersect_eliminated(f, conic, work).unwrap_or_default()
}

fn intersect_eliminated(
    f: &[Vec<f64>],
    conic: &ConicData,
    work: &IntersectionWork,
) -> Option<Vec<Point>> {
    let [a, b, d, h, u, v] = conic.matrix;
    if conic.ty == ConicType::Empty {
        return Some(Vec::new());
    }
    let g = vec![vec![d, 2.0 * v, b], vec![2.0 * u, 2.0 * h], vec![a]];
    let degree = f.len().checked_sub(1)?;
    if degree == 0 {
        return Some(constant_in_x(&g, &f[0], work));
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
            if let Some(pair) = polish((x, y), f, &g) {
                insert(&mut points, pair);
            }
        }
    }
    Some(points)
}

fn constant_in_x(f: &[Vec<f64>], y_poly: &[f64], work: &IntersectionWork) -> Vec<Point> {
    let mut points = Vec::new();
    for y in roots(y_poly, work) {
        let slice = f.iter().map(|row| evaluate(row, y)).collect::<Vec<_>>();
        points.extend(roots(&slice, work).into_iter().map(|x| (x, y)));
    }
    points
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

pub(super) fn polish(mut point: Point, f: &[Vec<f64>], g: &[Vec<f64>]) -> Option<Point> {
    let mut before = f64::INFINITY;
    for _ in 0..16 {
        let (x, y) = point;
        let (p, q) = (implicit::evaluate(f, x, y), implicit::evaluate(g, x, y));
        let error = p.abs() + q.abs();
        if error <= 1e-13 {
            break;
        }
        if error >= before || !error.is_finite() {
            break;
        }
        let ((px, py), (qx, qy)) = (gradient(f, x, y), gradient(g, x, y));
        let determinant = px * qy - py * qx;
        if determinant == 0.0 || !determinant.is_finite() {
            break;
        }
        let next = (
            x - (p * qy - q * py) / determinant,
            y - (q * px - p * qx) / determinant,
        );
        if !next.0.is_finite() || !next.1.is_finite() {
            break;
        }
        let residual = implicit::evaluate(f, next.0, next.1).abs()
            + implicit::evaluate(g, next.0, next.1).abs();
        if residual >= error {
            break;
        }
        point = next;
        before = error;
    }
    let (x, y) = point;
    let fits = [f, g].into_iter().all(|matrix| {
        let residual = implicit::evaluate(matrix, x, y).abs();
        residual < crate::tol::STANDARD_PRECISION
    });
    fits.then_some(point)
}

pub(super) fn gradient(matrix: &[Vec<f64>], x: f64, y: f64) -> Point {
    let mut result = (0.0_f64, 0.0_f64);
    for (i, row) in matrix.iter().enumerate().rev() {
        if i != 0 {
            result.0 = result.0.mul_add(x, i as f64 * evaluate(row, y));
        }
        let dy = row
            .iter()
            .enumerate()
            .skip(1)
            .rev()
            .fold(0.0_f64, |a, (j, &c)| a.mul_add(y, c * j as f64));
        result.1 = result.1.mul_add(x, dy);
    }
    result
}

pub(super) fn insert(points: &mut Vec<Point>, point: Point) {
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

/// Eliminate x via a bounded Sylvester determinant over polynomials in y.
pub(super) fn pair(f: &[Vec<f64>], g: &[Vec<f64>], work: &IntersectionWork) -> Option<Vec<Point>> {
    let degree = |coefficients: &[Vec<f64>]| {
        coefficients
            .iter()
            .rposition(|row| row.iter().any(|&v| v != 0.0))
    };
    let (m, n) = (degree(f)?, degree(g)?);
    if m == 0 && n == 0 {
        return Some(Vec::new());
    }
    if m == 0 {
        return Some(constant_in_x(g, &f[0], work));
    }
    if n == 0 {
        return Some(constant_in_x(f, &g[0], work));
    }
    let only_x = |matrix: &[Vec<f64>]| {
        matrix
            .iter()
            .all(|row| row.iter().skip(1).all(|&v| v == 0.0))
    };
    if only_x(g) {
        return Some(constant_in_y(f, g, work));
    }
    if only_x(f) {
        return Some(constant_in_y(g, f, work));
    }
    let projection = sylvester(&f[..=m], &g[..=n], work)?;
    let (lower, higher) = if m < n { (f, g) } else { (g, f) };
    let mut points = Vec::new();
    for y in elimination_roots(&projection, work) {
        let mut slice: Vec<_> = lower.iter().map(|row| evaluate(row, y)).collect();
        if slice
            .iter()
            .all(|&value| value.abs() <= crate::tol::STANDARD_PRECISION)
        {
            slice = higher.iter().map(|row| evaluate(row, y)).collect();
        }
        for x in elimination_roots(&slice, work) {
            if let Some(pair) = polish((x, y), f, g) {
                insert(&mut points, pair);
            }
        }
    }
    Some(points)
}

fn constant_in_y(f: &[Vec<f64>], g: &[Vec<f64>], work: &IntersectionWork) -> Vec<Point> {
    let x_poly: Vec<_> = g
        .iter()
        .map(|row| row.first().copied().unwrap_or(0.0))
        .collect();
    let mut points = Vec::new();
    for x in roots(&x_poly, work) {
        let mut slice = Vec::new();
        for row in f.iter().rev() {
            for value in &mut slice {
                *value *= x;
            }
            add_scaled(&mut slice, row, 1.0);
        }
        for y in elimination_roots(&slice, work) {
            if let Some(pair) = polish((x, y), f, g) {
                insert(&mut points, pair);
            }
        }
    }
    points
}

fn sylvester(f: &[Vec<f64>], g: &[Vec<f64>], work: &IntersectionWork) -> Option<Polynomial> {
    const MAX_ROWS: usize = 12;
    let (m, n) = (f.len() - 1, g.len() - 1);
    let size = m + n;
    if size > MAX_ROWS {
        return None;
    }
    let all = (1usize << size) - 1;
    let mut minors = vec![Vec::new(); all + 1];
    minors[0].push(1.0);
    // One minor per subset of selected columns. Each row has a shifted
    // block of coefficients from just one of the input polynomials.
    for columns in 0..all {
        if minors[columns].is_empty() {
            continue;
        }
        let row = columns.count_ones() as usize;
        let (offset, block) = if row < n { (row, f) } else { (row - n, g) };
        for (j, coefficient) in block.iter().enumerate() {
            let col = offset + j;
            let bit = 1 << col;
            if columns & bit != 0 {
                continue;
            }
            let term = product(&minors[columns], coefficient, work)?;
            let inversions = (columns >> (col + 1)).count_ones();
            add_scaled(
                &mut minors[columns | bit],
                &term,
                if inversions % 2 == 0 { 1.0 } else { -1.0 },
            );
        }
    }
    minors.pop()
}
