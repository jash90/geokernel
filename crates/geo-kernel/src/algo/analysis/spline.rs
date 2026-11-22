// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSpline.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{Point, SplineInput};
use crate::expr::IntersectionWork;
use crate::{Construction, Id, Value};

pub(crate) struct Spline {
    knots: Vec<f64>,
    coefficients: Vec<[f64; 2]>,
    stride: usize,
}

impl Spline {
    pub(super) fn solve(c: &Construction, input: SplineInput) -> Option<Self> {
        let list = c.elements(input.points)?;
        let n = list.len();
        let degree = match input.degree {
            Some(id) => c.number(Id(id as usize))? as usize,
            None => 3,
        };
        if !(3..=n).contains(&degree) {
            return None;
        }
        let stride = degree + 1;
        let cols = (n - 1).checked_mul(stride)?;
        let work = IntersectionWork::new();
        // Reserve the work of elimination before allocating a dense matrix.
        let cost = (cols as u64).saturating_pow(3);
        if !work.charge(cost) {
            return None;
        }
        let points: Vec<_> = (0..n)
            .map(|i| match list.raw_point(i)?.cartesian() {
                Value::Point { x, y } => Some([x, y]),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let mut knots = vec![0.0];
        for pair in points.windows(2) {
            let (dx, dy) = (pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]);
            let length = input.weight.map_or_else(
                || {
                    // The default distance accumulates squared coordinates in a float.
                    let dx2 = (dx * dx) as f32;
                    f64::from((f64::from(dx2) + dy * dy) as f32).sqrt()
                },
                |w| work.sample_system(c, Id(w as usize), dx, &[dy]),
            );
            if !length.is_finite() || length <= 0.0 {
                return None;
            }
            knots.push(knots.last()? + length);
        }
        let total = *knots.last()?;
        if !total.is_finite() || work.exhausted() {
            return None;
        }
        knots.iter_mut().for_each(|knot| *knot /= total);

        let mut matrix = vec![vec![0.0; cols + 2]; cols];
        let mut row = 0;
        for end in 0..=1 {
            for piece in 0..n - 1 {
                let i = piece + end;
                fill(&mut matrix[row], piece * stride, degree, 0, knots[i], 1.0);
                matrix[row][cols..].copy_from_slice(&points[i]);
                row += 1;
            }
        }
        for order in 1..degree {
            for (piece, &knot) in knots[1..n - 1].iter().enumerate() {
                fill(&mut matrix[row], piece * stride, degree, order, knot, 1.0);
                fill(
                    &mut matrix[row],
                    (piece + 1) * stride,
                    degree,
                    order,
                    knot,
                    -1.0,
                );
                row += 1;
            }
        }
        let boundary_order = degree - 1;
        let last = cols - stride;
        if points.first().zip(points.last()).is_some_and(|(a, b)| {
            (a[0] - b[0]).abs() < crate::tol::STANDARD_PRECISION
                && (a[1] - b[1]).abs() < crate::tol::STANDARD_PRECISION
        }) {
            for order in 1..degree {
                // AlgoSpline's endpoint constraints use t^(power-1)
                // even for higher derivatives. At zero only order one
                // has a nonzero coefficient.
                if order == 1 {
                    fill(&mut matrix[row], 0, degree, order, 0.0, 1.0);
                }
                fill(&mut matrix[row], last, degree, order, 1.0, -1.0);
                row += 1;
            }
        } else {
            matrix[row][1] = factor(degree - 1, boundary_order);
            row += 1;
            matrix[row][last] = factor(degree, degree);
            matrix[row][last + 1] = factor(degree - 1, boundary_order);
            row += 1;
        }
        for (i, row) in matrix[row.min(cols)..].iter_mut().enumerate() {
            let piece = n.checked_sub(i + 3)?;
            row[piece * stride] = factor(degree, degree) * knots[i + 1];
            row[piece * stride + 1] = factor(degree - 1, boundary_order);
        }
        let coefficients = eliminate(matrix, cols)?;
        Some(Self {
            knots,
            coefficients,
            stride,
        })
    }

    pub(in crate::algo) fn pieces(&self) -> impl Iterator<Item = (f64, f64, &[[f64; 2]])> {
        self.knots.windows(2).enumerate().map(|(piece, interval)| {
            let start = piece * self.stride;
            (
                interval[0],
                interval[1],
                &self.coefficients[start..start + self.stride],
            )
        })
    }

    pub(crate) fn sample(&self, t: f64, derivative: usize) -> Point {
        let piece = self
            .knots
            .partition_point(|&knot| knot <= t)
            .saturating_sub(1)
            .min(self.knots.len() - 2);
        let row = &self.coefficients[piece * self.stride..(piece + 1) * self.stride];
        let [x, y] = row
            .iter()
            .enumerate()
            .take(self.stride.saturating_sub(derivative))
            .fold([0.0; 2], |[x, y], (i, [a, b])| {
                let f = factor(self.stride - 1 - i, derivative);
                [x * t + f * a, y * t + f * b]
            });
        (x, y)
    }
}

fn factor(power: usize, derivative: usize) -> f64 {
    (0..derivative).fold(1.0, |f, k| f * (power - k) as f64)
}

fn fill(row: &mut [f64], offset: usize, degree: usize, order: usize, t: f64, sign: f64) {
    for power in order..=degree {
        row[offset + degree - power] = sign * factor(power, order) * t.powi((power - order) as i32);
    }
}

fn eliminate(mut matrix: Vec<Vec<f64>>, n: usize) -> Option<Vec<[f64; 2]>> {
    for i in 0..n {
        let pivot = (i..n).max_by(|&a, &b| matrix[a][i].abs().total_cmp(&matrix[b][i].abs()))?;
        matrix.swap(i, pivot);
        let (upper, lower) = matrix.split_at_mut(i + 1);
        let row = &upper[i];
        let scale = row[i];
        if scale == 0.0 || !scale.is_finite() {
            return None;
        }
        for lower in lower {
            let multiplier = lower[i] / scale;
            for (value, pivot) in lower[i..].iter_mut().zip(&row[i..]) {
                *value -= multiplier * pivot;
            }
        }
    }
    let mut out = vec![[0.0; 2]; n];
    for i in (0..n).rev() {
        let mut rhs = [matrix[i][n], matrix[i][n + 1]];
        for (&a, &[x, y]) in matrix[i][i + 1..n].iter().zip(&out[i + 1..]) {
            rhs[0] -= a * x;
            rhs[1] -= a * y;
        }
        let x = rhs.map(|v| v / matrix[i][i]);
        if x.iter().any(|v| !v.is_finite()) {
            return None;
        }
        out[i] = x;
    }
    Some(out)
}
