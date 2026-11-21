// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoApplyMatrix.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDeterminant.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoMatrixRank.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVector.java
//   source/shared/common/src/main/java/org/geogebra/common/util/GgbMat.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{is_zero, line_value};
use crate::expr::{math, Element, List, MAX_EVALUATION_WORK};
use crate::kernel::Value;
use crate::tol::STANDARD_PRECISION;
use std::cell::RefCell;

thread_local! {
    // Reuse the storage when a slider changes the entries of an existing matrix.
    static SCRATCH: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

fn row(element: Element<'_>) -> Option<List<'_>> {
    match element {
        Element::List(row) => Some(row),
        Element::Value(..) => None,
    }
}

fn numeric(element: Element<'_>) -> Option<f64> {
    match element {
        Element::Value(_, Value::Number(x)) => Some(x),
        _ => None,
    }
}

fn matrix_cell(element: Element<'_>) -> Option<f64> {
    match element {
        Element::Value(_, Value::Bool(b)) => Some(f64::from(u8::from(b))),
        _ => numeric(element),
    }
}

fn cells(
    list: List<'_>,
    cols: usize,
    read: fn(Element<'_>) -> Option<f64>,
) -> impl Iterator<Item = Option<f64>> + '_ {
    list.iter().flat_map(move |item| {
        let row = row(item).filter(|row| row.len() == cols);
        (0..cols).map(move |i| read(row?.get(i)?))
    })
}

pub(crate) fn budget() -> impl FnMut(u64) -> bool {
    let mut left = MAX_EVALUATION_WORK;
    move |work| {
        if let Some(remaining) = left.checked_sub(work) {
            left = remaining;
            true
        } else {
            false
        }
    }
}

/// Open a rectangular `GgbMat`, charging for its entries before copying.
fn eliminate<F: FnMut(u64) -> bool>(
    list: List<'_>,
    square: bool,
    mut charge: F,
    f: impl FnOnce(&mut [f64], usize, usize, F) -> Option<f64>,
) -> Option<f64> {
    let rows = list.len();
    let cols = row(list.get(0)?)?.len();
    if cols == 0 || square && rows != cols {
        return None;
    }
    let len = rows.checked_mul(cols)?;
    if !charge(len as u64) {
        return None;
    }
    SCRATCH.with_borrow_mut(|data| {
        data.clear();
        for entry in cells(list, cols, numeric) {
            data.push(entry?);
        }
        f(data, rows, cols, charge)
    })
}

pub(crate) fn determinant(list: List<'_>, charge: impl FnMut(u64) -> bool) -> Option<f64> {
    eliminate(list, true, charge, |data, size, _, mut charge| {
        if !charge(data.len() as u64) {
            return None;
        }
        let integers = data.iter().all(|&x| math::is_integer(x));
        let mut det = 1.0;
        for k in 0..size {
            let tail = size - k - 1;
            if !charge(3 * tail as u64 + 1) {
                return None;
            }
            let mut pivot = k;
            for i in k + 1..size {
                if data[i * size + k].abs() > data[pivot * size + k].abs() {
                    pivot = i;
                }
            }
            if data[pivot * size + k].abs() < STANDARD_PRECISION {
                return Some(0.0);
            }
            if pivot != k {
                if !charge(size as u64) {
                    return None;
                }
                swap_rows(data, size, k, pivot);
                det = -det;
            }
            let lead = data[k * size + k];
            det *= lead;
            let finite = data[k * size + k + 1..(k + 1) * size]
                .iter()
                .all(|v| v.is_finite());
            for i in k + 1..size {
                let factor = data[i * size + k] / lead;
                if factor == 0.0 && finite {
                    continue;
                }
                if !charge(tail as u64) {
                    return None;
                }
                for j in k + 1..size {
                    data[i * size + j] -= factor * data[k * size + j];
                }
            }
        }
        Some(if integers {
            math::java_round(det) as f64
        } else {
            det
        })
    })
}

fn swap_rows(data: &mut [f64], cols: usize, a: usize, b: usize) {
    for j in 0..cols {
        data.swap(a * cols + j, b * cols + j);
    }
}

pub(crate) fn rank(list: List<'_>, charge: impl FnMut(u64) -> bool) -> Option<f64> {
    eliminate(list, false, charge, |data, rows, cols, mut charge| {
        // `GgbMat.reducedRowEchelonFormImmediate`: the first sufficiently
        // nonzero entry leads. Skipped entries are set to zero.
        let mut r = 0;
        for lead in 0..cols {
            if !charge(rows as u64) {
                return None;
            }
            let mut pivot = r;
            while pivot < rows && is_zero(data[pivot * cols + lead]) {
                data[pivot * cols + lead] = 0.0;
                pivot += 1;
            }
            if pivot == rows {
                continue;
            }
            if !charge((rows + 3 * cols) as u64) {
                return None;
            }
            swap_rows(data, cols, r, pivot);
            let divisor = data[r * cols + lead];
            for j in 0..cols {
                data[r * cols + j] /= divisor;
            }
            let finite = data[r * cols..(r + 1) * cols].iter().all(|v| v.is_finite());
            for i in (0..rows).filter(|&i| i != r) {
                let factor = data[i * cols + lead];
                if factor == 0.0 && finite {
                    continue;
                }
                if !charge(cols as u64) {
                    return None;
                }
                for j in 0..cols {
                    data[i * cols + j] -= factor * data[r * cols + j];
                }
            }
            r += 1;
            if r == rows {
                break;
            }
        }
        if !charge(data.len() as u64) {
            return None;
        }
        Some(
            data.chunks_exact(cols)
                .take_while(|row| row.iter().any(|&x| !is_zero(x)))
                .count() as f64,
        )
    })
}

pub(super) fn apply(list: List<'_>, src: Value) -> Option<Value> {
    let size = list.len();
    if !matches!(size, 2 | 3) {
        return None;
    }
    let mut data = [0.0; 9];
    for (entry, dest) in cells(list, size, matrix_cell).zip(&mut data) {
        *dest = entry?;
    }
    if let Some((x, y, z)) = src.point_coords() {
        let [a, b, c, d, e, f, g, h, i] = data;
        return Some(if size == 2 {
            Value::homogeneous_point(a * x + b * y, c * x + d * y, z)
        } else {
            Value::homogeneous_point(
                a * x + b * y + c * z,
                d * x + e * y + f * z,
                g * x + h * y + i * z,
            )
        });
    }
    match (size, src) {
        (2, Value::Line { a, b, c }) => {
            // `GeoLine.matrixTransform` retains the original coefficient
            // scale, including the alternate formula for vertical lines.
            let [p, q, r, s, ..] = data;
            if is_zero(b) {
                line_value((s * a, -q * a, -q * r * c + s * p * c))
            } else {
                let x = r * b - s * a;
                let y = q * a - p * b;
                line_value((x * b, y * b, q * c * x + s * c * y))
            }
        }
        (3, Value::Line { a, b, c }) => {
            let [p, q, r, s, t, u, v, w, z] = data;
            line_value((
                (t * z - w * u) * a - (s * z - v * u) * b + (s * w - v * t) * c,
                -(q * z - r * w) * a + (p * z - v * r) * b - (p * w - q * v) * c,
                (q * u - r * t) * a - (p * u - r * s) * b + (p * t - q * s) * c,
            ))
        }
        (_, Value::Vector { x, y }) => {
            // `GeoVector.matrixTransform` uses (x, y, 1) and divides by the
            // resulting scale.
            let (x, y) = if size == 2 {
                (data[0] * x + data[1] * y, data[2] * x + data[3] * y)
            } else {
                let w = data[6] * x + data[7] * y + data[8];
                let tx = data[0] * x + data[1] * y + data[2];
                let ty = data[3] * x + data[4] * y + data[5];
                (tx / w, ty / w)
            };
            Some(Value::Vector { x, y })
        }
        _ => None,
    }
}
