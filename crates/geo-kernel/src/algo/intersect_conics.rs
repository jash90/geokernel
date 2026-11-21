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
#[path = "intersect_conics_distance.rs"]
mod distance;
#[path = "intersect_conics_roots.rs"]
mod roots;
#[path = "intersect_conics_state.rs"]
mod state;

use self::distance::distance;
use self::roots::cubic;
pub(crate) use self::roots::polynomial_roots;
pub(crate) use self::state::Intersection;
pub(crate) use self::state::SavedSlots;

use crate::tol::{MAX_PRECISION, MIN_PRECISION, STANDARD_PRECISION};

type Matrix = [f64; 6];
type Points = [Pt; 4];

const IDENTITY: [usize; 4] = [0, 1, 2, 3];

fn pack_order(order: [usize; 4]) -> u8 {
    order
        .into_iter()
        .enumerate()
        .fold(0, |bits, (i, slot)| bits | (slot as u8) << (i * 2))
}

fn unpack_order(bits: u8) -> [usize; 4] {
    std::array::from_fn(|i| usize::from(bits >> (i * 2) & 3))
}

#[derive(Clone, Copy, Debug)]
struct RawScratch {
    points: Points,
    /// Positions of the current candidates in an uncached solution.
    order: u8,
}

impl Default for RawScratch {
    fn default() -> Self {
        Self {
            points: [None; 4],
            order: pack_order(IDENTITY),
        }
    }
}

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

fn solve(
    a: &ConicData,
    b: &ConicData,
    pencil: &mut ConicData,
    scratch: &mut RawScratch,
    continuous: bool,
) -> Points {
    scratch.order = pack_order(IDENTITY);
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

    let degenerate_input = a.ty.has_lines() || b.ty.has_lines();
    let found = test_points(a, b, &mut p, MIN_PRECISION);
    if degenerate_input {
        scratch.points = p;
    }
    if !found {
        let mut eps = MAX_PRECISION;
        while eps <= MIN_PRECISION {
            p = intersect(a, b, pencil, scratch, eps);
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

fn intersect(
    a: &ConicData,
    b: &ConicData,
    pencil: &mut ConicData,
    scratch: &mut RawScratch,
    eps: f64,
) -> Points {
    let m = a.matrix;
    let n = b.matrix;
    let minor = |i: usize, j: usize| (m[i] * n[j] - m[j] * n[i]).abs();
    if minor(0, 1) < eps && minor(0, 3) < eps && minor(1, 3) < eps {
        return equal_quadratic_parts(a, b, &mut scratch.points, eps);
    }

    // Eliminate a variable directly when one equation is linear in it.
    // The special branches keep the ordering of the reference solver.
    for (first, second) in [(a, b), (b, a)] {
        let m = first.matrix;
        let n = second.matrix;
        if m[0] == 0.0 && m[1] == 0.0 && n[3] == 0.0 {
            return solve_bilinear(first, second, scratch, eps);
        }
    }
    for (first, second) in [(a, b), (b, a)] {
        let m = first.matrix;
        let n = second.matrix;
        if m[3] == 0.0 && n[3] == 0.0 {
            if m[1] == 0.0 {
                return solve_axis_parabola(first, second, scratch, true, eps);
            }
            if m[0] == 0.0 {
                return solve_axis_parabola(first, second, scratch, false, eps);
            }
        }
    }
    for vertical in [true, false] {
        for (first, second) in [(a, b), (b, a)] {
            let m = first.matrix;
            let n = second.matrix;
            if m[usize::from(!vertical)] == 0.0 && n[0] == 0.0 && n[1] == 0.0 {
                return solve_one_quadratic(first, second, scratch, vertical, eps);
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

fn equal_quadratic_parts(a: &ConicData, b: &ConicData, scratch: &mut Points, eps: f64) -> Points {
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
        scratch[..2].copy_from_slice(&on_line(line, conic, eps));
        if test_points(a, b, scratch, MIN_PRECISION) {
            return *scratch;
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

fn solve_bilinear(a: &ConicData, b: &ConicData, scratch: &mut RawScratch, eps: f64) -> Points {
    let mut candidates = AxisSolutions::default();
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
        intersect_axes(
            a,
            b,
            &roots[..count],
            vertical,
            eps,
            &mut scratch.points,
            &mut candidates,
        );
    }
    candidates.finish(scratch)
}

fn axis_matrix([a, b, c, d, e, f]: Matrix, vertical: bool) -> Matrix {
    if vertical {
        [a, b, c, d, e, f]
    } else {
        [b, a, c, d, f, e]
    }
}

fn solve_axis_parabola(
    a: &ConicData,
    b: &ConicData,
    scratch: &mut RawScratch,
    vertical: bool,
    eps: f64,
) -> Points {
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
    let mut candidates = AxisSolutions::default();
    intersect_axes(
        a,
        b,
        &roots[..count],
        vertical,
        eps,
        &mut scratch.points,
        &mut candidates,
    );
    candidates.finish(scratch)
}

fn solve_one_quadratic(
    a: &ConicData,
    b: &ConicData,
    scratch: &mut RawScratch,
    vertical: bool,
    eps: f64,
) -> Points {
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
    let mut candidates = AxisSolutions::default();
    intersect_axes(
        a,
        b,
        &roots[..count],
        vertical,
        eps,
        &mut scratch.points,
        &mut candidates,
    );
    candidates.finish(scratch)
}

#[derive(Default)]
struct AxisSolutions {
    with_history: Points,
    fresh: Points,
}

impl AxisSolutions {
    fn collect(&mut self, scratch: &Points) {
        fn insert(points: &mut Points, candidates: &[Pt]) {
            for &point in candidates.iter().flatten() {
                if !points.iter().any(|&p| points_equal(p, Some(point))) {
                    if let Some(slot) = points.iter_mut().find(|slot| slot.is_none()) {
                        *slot = Some(point);
                    }
                }
            }
        }
        insert(&mut self.with_history, scratch);
        insert(&mut self.fresh, &scratch[..2]);
    }

    fn finish(self, scratch: &mut RawScratch) -> Points {
        if self.with_history != self.fresh {
            let mut available = [true; 4];
            let mut order = IDENTITY;
            for (i, point) in self.with_history.into_iter().enumerate() {
                let Some(slot) = (0..4).find(|&slot| {
                    available[slot]
                        && (point == self.fresh[slot] || points_equal(point, self.fresh[slot]))
                }) else {
                    return self.with_history;
                };
                order[i] = slot;
                available[slot] = false;
            }
            scratch.order = pack_order(order);
        }
        self.with_history
    }
}

fn intersect_axes(
    a: &ConicData,
    b: &ConicData,
    roots: &[f64],
    vertical: bool,
    eps: f64,
    scratch: &mut Points,
    candidates: &mut AxisSolutions,
) {
    for &root in roots {
        let line = if vertical {
            [1.0, 0.0, -root]
        } else {
            [0.0, 1.0, -root]
        };
        for conic in [a, b] {
            // The line-conic solver writes only the first two scratch slots.
            // The other two can still supply valid points and affect order.
            scratch[..2].copy_from_slice(&on_line(line, conic, eps));
            test_points(a, b, scratch, 10.0 * eps);
            candidates.collect(scratch);
        }
    }
}
