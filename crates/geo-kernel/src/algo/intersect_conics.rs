//! Intersections of full conics, including line pairs and single points.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectConics.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConicPart.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicPartParameters.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::algo::conic_nd::{ConicData, ConicType};
use crate::algo::intersect::{
    check_decimal_fraction, line_conic_points, normalized_coefficients, points_equal, Pt,
};
use crate::algo::{arc::ArcData, path::conic_parameter};
#[path = "intersect_conics_roots.rs"]
mod roots;
#[path = "intersect_conics_state.rs"]
mod state;

use self::roots::cubic;
pub(crate) use self::roots::polynomial_roots;
pub(crate) use self::state::Intersection;
pub(crate) use self::state::SavedSlots;

use crate::tol::{MAX_PRECISION, MIN_PRECISION, STANDARD_PRECISION};

type Matrix = [f64; 6];
type Points = [Pt; 4];

/// Check incidence with the curved path of an arc or a sector. A collinear
/// circumcircular arc can also define a bounded segment.
pub(crate) fn on_part(part: &ArcData, point: (f64, f64)) -> bool {
    let conic = &part.conic;
    if !part.defined || !conic.on_full_conic(point, MIN_PRECISION) {
        return false;
    }
    let parameter = match conic.ty {
        ConicType::Circle | ConicType::Ellipse => {
            let tau = std::f64::consts::TAU;
            let t = conic_parameter(conic, point, false);
            let t = if t < 0.0 { t + tau } else { t };
            let t = t - part.start;
            (if t < 0.0 { t + tau } else { t }) / part.extent
        }
        ConicType::ParallelLines if part.positive => {
            let Some([(x, y), (u, v)]) = part.segment() else {
                return false;
            };
            let (dx, dy) = (u - x, v - y);
            ((point.0 - x) * dx + (point.1 - y) * dy) / (dx * dx + dy * dy)
        }
        _ => return false,
    };
    let axis = conic.half_axes[0];
    let eps = if axis > 100.0 {
        MAX_PRECISION.max(MIN_PRECISION / axis)
    } else {
        MIN_PRECISION
    };
    (-eps..=1.0 + eps).contains(&parameter)
}

fn solve(a: &ConicData, b: &ConicData, pencil: &mut ConicData, continuous: bool) -> Points {
    if !a.defined || !b.defined || proportional(&a.matrix, &b.matrix) {
        return [None; 4];
    }
    if a.ty.has_lines() && b.ty.has_lines() && share_line(a, b) {
        return [None; 4];
    }

    let mut p = if a.ty.has_lines() {
        with_degenerate(b, a, STANDARD_PRECISION)
    } else if b.ty.has_lines() {
        with_degenerate(a, b, STANDARD_PRECISION)
    } else {
        [None; 4]
    };

    if !test_points(a, b, &mut p, MIN_PRECISION) {
        let mut eps = MAX_PRECISION;
        while eps <= MIN_PRECISION {
            p = intersect(a, b, pencil, eps);
            if p.iter().any(Option::is_some) {
                break;
            }
            eps *= 10.0;
        }
    }

    if continuous {
        p
    } else {
        compact(p)
    }
}

fn compact(p: Points) -> Points {
    let mut result = [None; 4];
    for (slot, point) in result.iter_mut().zip(p.into_iter().flatten()) {
        *slot = Some(point);
    }
    result
}

/// Suppress the duplicate of the first tangent point until its second slot
/// receives a label. Other duplicate slots follow the reference algorithm.
fn distinct(mut p: Points) -> Points {
    if points_equal(p[0], p[1]) {
        p[1] = None;
    }
    p
}

/// Both real branches of the bilinear elimination quartic can touch the
/// other conic. Preserve the two pairs of roots when assigning an existing
/// set of four outputs at this transition.
fn with_bilinear_multiplicity(a: &ConicData, b: &ConicData, points: Points) -> Points {
    let [Some(p), Some(q), None, None] = points else {
        return points;
    };
    let bilinear = |m: Matrix| m[0] == 0.0 && m[1] == 0.0 && m[3] != 0.0;
    if !(bilinear(a.matrix) && b.matrix[3] == 0.0 || bilinear(b.matrix) && a.matrix[3] == 0.0) {
        return points;
    }
    let normal = |[a, b, _, d, e, f]: Matrix, (x, y): (f64, f64)| {
        normalize([a * x + d * y + e, d * x + b * y + f])
    };
    let tangent = |point| {
        let [x, y] = normal(a.matrix, point);
        let [u, v] = normal(b.matrix, point);
        (x * v - y * u).abs() <= 8.0 * f64::EPSILON
    };
    if tangent(p) && tangent(q) {
        [Some(p), Some(p), Some(q), Some(q)]
    } else {
        points
    }
}

fn proportional(a: &Matrix, b: &Matrix) -> bool {
    let index = (0..6)
        .max_by(|&i, &j| a[i].abs().total_cmp(&a[j].abs()))
        .unwrap_or(0);
    let scale = a[index] / b[index];
    scale.is_finite()
        && a.iter().zip(b).all(|(&x, &y)| {
            let y = scale * y;
            (x - y).abs() <= 2.0 * f64::EPSILON * x.abs().max(y.abs())
        })
}

fn share_line(a: &ConicData, b: &ConicData) -> bool {
    let lines_a = a.lines.map(normalize);
    let lines_b = b.lines.map(normalize);
    lines_a.iter().any(|&[a, b, c]| {
        lines_b.iter().any(|&[d, e, f]| {
            (a * e - b * d).abs() <= STANDARD_PRECISION
                && (c * e - b * f).abs() <= STANDARD_PRECISION
                && (a * f - c * d).abs() <= STANDARD_PRECISION
        })
    })
}

fn on_line(line: [f64; 3], conic: &ConicData, eps: f64) -> [Pt; 2] {
    line_conic_points(
        normalized_coefficients(line),
        &conic.matrix,
        conic.ty == ConicType::DoubleLine,
        eps,
    )
}

fn with_degenerate(conic: &ConicData, degenerate: &ConicData, eps: f64) -> Points {
    match degenerate.ty {
        ConicType::IntersectingLines | ConicType::ParallelLines => {
            let [p, q] = on_line(degenerate.lines[1], conic, eps);
            let [r, s] = on_line(degenerate.lines[0], conic, eps);
            [p, q, r, s]
        }
        ConicType::DoubleLine => {
            let [p, q] = on_line(degenerate.lines[0], conic, eps);
            [p, q, None, None]
        }
        ConicType::Empty => {
            // The pencil should be degenerate. Its limiting double line
            // can still contain a tangent point after rounding.
            let [ex, ey] = degenerate.eigenvec[0];
            let [x, y] = degenerate.midpoint;
            let [p, q] = on_line([-ey, ex, y * -ex + x * ey], conic, eps);
            [p, q, None, None]
        }
        ConicType::SinglePoint => {
            let [x, y] = degenerate.midpoint;
            [Some((x, y)), None, None, None]
        }
        _ => [None; 4],
    }
}

/// Local estimate of the point's distance to the conic. It is only used
/// to decide whether replacing a computed point by a nearby decimal
/// fraction reduces the residual on both conics.
fn distance(conic: &ConicData, (x, y): (f64, f64)) -> f64 {
    let [a, b, c, d, e, f] = conic.matrix;
    let fx = a * x + d * y + e;
    let fy = d * x + b * y + f;
    let error = (x * fx + y * fy + e * x + f * y + c).abs();
    if error == 0.0 {
        0.0
    } else {
        error / fx.hypot(fy)
    }
}

fn test_points(a: &ConicData, b: &ConicData, p: &mut Points, eps: f64) -> bool {
    for slot in p.iter_mut() {
        let Some((x, y)) = *slot else { continue };
        let raw = (x, y);
        let rounded = (check_decimal_fraction(x), check_decimal_fraction(y));
        let q = if raw != rounded
            && distance(a, rounded) <= distance(a, raw)
            && distance(b, rounded) <= distance(b, raw)
        {
            rounded
        } else {
            raw
        };
        *slot = (q.0.is_finite()
            && q.1.is_finite()
            && a.on_full_conic(q, eps)
            && b.on_full_conic(q, eps))
        .then_some(q);
    }
    p.iter().any(Option::is_some)
}

fn intersect(a: &ConicData, b: &ConicData, pencil: &mut ConicData, eps: f64) -> Points {
    let m = a.matrix;
    let n = b.matrix;
    let minor = |i: usize, j: usize| (m[i] * n[j] - m[j] * n[i]).abs();
    if minor(0, 1) < eps && minor(0, 3) < eps && minor(1, 3) < eps {
        return equal_quadratic_parts(a, b, eps);
    }

    // Eliminate a variable directly when one equation is linear in it.
    // The special branches keep the ordering of the reference solver.
    for (first, second) in [(a, b), (b, a)] {
        let m = first.matrix;
        let n = second.matrix;
        if m[0] == 0.0 && m[1] == 0.0 && n[3] == 0.0 {
            return solve_bilinear(first, second, eps);
        }
    }
    for (first, second) in [(a, b), (b, a)] {
        let m = first.matrix;
        let n = second.matrix;
        if m[3] == 0.0 && n[3] == 0.0 {
            if m[1] == 0.0 {
                return solve_axis_parabola(first, second, true, eps);
            }
            if m[0] == 0.0 {
                return solve_axis_parabola(first, second, false, eps);
            }
        }
    }
    for vertical in [true, false] {
        for (first, second) in [(a, b), (b, a)] {
            let m = first.matrix;
            let n = second.matrix;
            if m[usize::from(!vertical)] == 0.0 && n[0] == 0.0 && n[1] == 0.0 {
                return solve_one_quadratic(first, second, vertical, eps);
            }
        }
    }

    let p = degenerate_pencil(a, b, normalize(m), normalize(n), pencil, eps);
    if p.iter().any(Option::is_some) {
        p
    } else {
        degenerate_pencil(b, a, normalize(n), normalize(m), pencil, eps)
    }
}

fn equal_quadratic_parts(a: &ConicData, b: &ConicData, eps: f64) -> Points {
    let m = a.matrix;
    let n = b.matrix;
    let (i, sign) = if m[0] == 0.0 && n[0] == 0.0 && m[1] == 0.0 && n[1] == 0.0 {
        (3, -1.0)
    } else if m[0] == 0.0 && n[0] == 0.0 && m[3] == 0.0 && n[3] == 0.0 {
        (1, -1.0)
    } else {
        (0, 1.0)
    };
    let entry = |j: usize| sign * (m[j] * n[i] - m[i] * n[j]);
    let line = [2.0 * entry(4), 2.0 * entry(5), entry(2)];
    for conic in [a, b] {
        let [p, q] = on_line(line, conic, eps);
        let mut p = [p, q, None, None];
        if test_points(a, b, &mut p, MIN_PRECISION) {
            return p;
        }
    }
    [None; 4]
}

fn normalize<const N: usize>(m: [f64; N]) -> [f64; N] {
    let max = m.iter().fold(0.0_f64, |max, v| max.max(v.abs()));
    m.map(|v| v / max)
}

fn determinant([a, b, c, d, e, f]: Matrix) -> f64 {
    c * (a * b - d * d) + e * (2.0 * d * f - b * e) - a * f * f
}

// The directional derivative of det(m) along n. Swapping m and n
// gives the quadratic coefficient of det(m + t*n).
fn det_derivative([a, b, c, d, e, f]: Matrix, [k, l, m, n, o, p]: Matrix) -> f64 {
    k * (b * c - f * f)
        + l * (a * c - e * e)
        + m * (a * b - d * d)
        + 2.0 * (n * (e * f - c * d) + o * (d * f - b * e) + p * (d * e - a * f))
}

fn degenerate_pencil(
    a: &ConicData,
    b: &ConicData,
    m: Matrix,
    n: Matrix,
    pencil: &mut ConicData,
    eps: f64,
) -> Points {
    let coefficients = [
        determinant(m),
        det_derivative(m, n),
        det_derivative(n, m),
        determinant(n),
    ];
    let (mut roots, count) = cubic(coefficients, eps);
    roots[..count].sort_by(f64::total_cmp);
    for &t in &roots[..count] {
        pencil.matrix = std::array::from_fn(|i| m[i] + t * n[i]);
        pencil.classify(true);
        for conic in [a, b] {
            if conic.ty.has_lines() && pencil.ty.has_lines() && share_line(conic, pencil) {
                continue;
            }
            let mut p = with_degenerate(conic, pencil, eps);
            if test_points(a, b, &mut p, MIN_PRECISION) {
                return p;
            }
        }
    }
    [None; 4]
}

fn solve_bilinear(a: &ConicData, b: &ConicData, eps: f64) -> Points {
    let mut p = [None; 4];
    for vertical in [true, false] {
        let [_, _, c, d, e, f] = axis_matrix(a.matrix, vertical);
        let [k, l, m, _, o, q] = axis_matrix(b.matrix, vertical);
        let coefficients = [
            c * c * l + 4.0 * f * f * m - 4.0 * c * f * q,
            8.0 * f * f * o - 4.0 * c * d * q + 4.0 * c * e * l + 8.0 * d * f * m - 8.0 * e * f * q,
            4.0 * d * d * m + 4.0 * e * e * l + 4.0 * f * f * k - 8.0 * d * e * q
                + 16.0 * d * f * o,
            8.0 * d * d * o + 8.0 * d * f * k,
            4.0 * d * d * k,
        ];
        let (roots, count) = polynomial_roots(coefficients);
        intersect_axes(a, b, &roots[..count], vertical, eps, &mut p);
    }
    p
}

fn axis_matrix([a, b, c, d, e, f]: Matrix, vertical: bool) -> Matrix {
    if vertical {
        [a, b, c, d, e, f]
    } else {
        [b, a, c, d, f, e]
    }
}

fn solve_axis_parabola(a: &ConicData, b: &ConicData, vertical: bool, eps: f64) -> Points {
    let [s, _, c, _, e, f] = axis_matrix(a.matrix, vertical);
    let [k, l, m, _, o, q] = axis_matrix(b.matrix, vertical);
    let coefficients = [
        -4.0 * f * c * q + l * c * c + 4.0 * f * f * m,
        4.0 * l * c * e - 8.0 * f * e * q + 8.0 * f * f * o,
        2.0 * l * s * c + 4.0 * l * e * e - 4.0 * s * f * q + 4.0 * f * f * k,
        4.0 * l * s * e,
        s * s * l,
    ];
    let (mut roots, count) = polynomial_roots(coefficients);
    roots[..count].sort_by(f64::total_cmp);
    let mut p = [None; 4];
    intersect_axes(a, b, &roots[..count], vertical, eps, &mut p);
    p
}

fn solve_one_quadratic(a: &ConicData, b: &ConicData, vertical: bool, eps: f64) -> Points {
    let [_, s, c, d, e, f] = axis_matrix(a.matrix, vertical);
    let [_, _, m, n, o, q] = axis_matrix(b.matrix, vertical);
    let coefficients = [
        4.0 * q * q * c - 4.0 * q * m * f + s * m * m,
        4.0 * s * m * o + 8.0 * q * q * e + 8.0 * q * n * c
            - 4.0 * q * m * d
            - 8.0 * q * o * f
            - 4.0 * n * m * f,
        4.0 * s * o * o + 16.0 * q * n * e - 8.0 * q * o * d + 4.0 * n * n * c
            - 4.0 * n * m * d
            - 8.0 * n * o * f,
        8.0 * n * n * e - 8.0 * n * o * d,
    ];
    let (roots, count) = cubic(coefficients, eps);
    let mut p = [None; 4];
    intersect_axes(a, b, &roots[..count], vertical, eps, &mut p);
    p
}

fn intersect_axes(
    a: &ConicData,
    b: &ConicData,
    roots: &[f64],
    vertical: bool,
    eps: f64,
    p: &mut Points,
) {
    for &root in roots {
        let line = if vertical {
            [1.0, 0.0, -root]
        } else {
            [0.0, 1.0, -root]
        };
        for conic in [a, b] {
            let [r, s] = on_line(line, conic, eps);
            let mut candidates = [r, s, None, None];
            test_points(a, b, &mut candidates, 10.0 * eps);
            for candidate in candidates.into_iter().flatten() {
                if !p.iter().any(|&prior| points_equal(prior, Some(candidate))) {
                    if let Some(slot) = p.iter_mut().find(|slot| slot.is_none()) {
                        *slot = Some(candidate);
                    } else {
                        return;
                    }
                }
            }
        }
    }
}
