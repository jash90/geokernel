// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoApplyMatrix.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDeterminant.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoMatrixRank.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVector.java
//   source/shared/common/src/main/java/org/geogebra/common/util/GgbMat.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{is_zero, line_value, zero_point_scale};
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

fn cells(list: List<'_>, cols: usize) -> impl Iterator<Item = Option<f64>> + '_ {
    list.iter().flat_map(move |item| {
        let row = row(item).filter(|row| row.len() == cols);
        (0..cols).map(move |i| numeric(row?.get(i)?))
    })
}

/// Open a rectangular `GgbMat` and run an elimination within the same work
/// bound as other list algorithms. Each pivot visits at most `rows * cols`
/// entries.
fn eliminate<T>(
    list: List<'_>,
    square: bool,
    f: impl FnOnce(&mut [f64], usize, usize) -> T,
) -> Option<T> {
    let rows = list.len();
    let cols = row(list.get(0)?)?.len();
    if cols == 0 || square && rows != cols {
        return None;
    }
    let len = rows.checked_mul(cols)?;
    let work = len.checked_mul(rows.min(cols))?;
    if work as u64 > MAX_EVALUATION_WORK {
        return None;
    }
    SCRATCH.with_borrow_mut(|data| {
        data.clear();
        for entry in cells(list, cols) {
            data.push(entry?);
        }
        Some(f(data, rows, cols))
    })
}

pub(super) fn determinant(list: List<'_>) -> Option<f64> {
    eliminate(list, true, |data, size, _| {
        let integers = data.iter().all(|&x| math::is_integer(x));
        let mut det = 1.0;
        for k in 0..size {
            let mut pivot = k;
            for i in k + 1..size {
                if data[i * size + k].abs() > data[pivot * size + k].abs() {
                    pivot = i;
                }
            }
            if data[pivot * size + k].abs() < STANDARD_PRECISION {
                return 0.0;
            }
            if pivot != k {
                swap_rows(data, size, k, pivot);
                det = -det;
            }
            let lead = data[k * size + k];
            det *= lead;
            for i in k + 1..size {
                let factor = data[i * size + k] / lead;
                for j in k + 1..size {
                    data[i * size + j] -= factor * data[k * size + j];
                }
            }
        }
        if integers {
            math::java_round(det) as f64
        } else {
            det
        }
    })
}

fn swap_rows(data: &mut [f64], cols: usize, a: usize, b: usize) {
    for j in 0..cols {
        data.swap(a * cols + j, b * cols + j);
    }
}

pub(super) fn rank(list: List<'_>) -> Option<f64> {
    eliminate(list, false, |data, rows, cols| {
        // `GgbMat.reducedRowEchelonFormImmediate`: the first sufficiently
        // nonzero entry leads. Skipped entries are set to zero.
        let mut r = 0;
        for lead in 0..cols {
            let mut pivot = r;
            while pivot < rows && is_zero(data[pivot * cols + lead]) {
                data[pivot * cols + lead] = 0.0;
                pivot += 1;
            }
            if pivot == rows {
                continue;
            }
            swap_rows(data, cols, r, pivot);
            let divisor = data[r * cols + lead];
            for j in 0..cols {
                data[r * cols + j] /= divisor;
            }
            for i in (0..rows).filter(|&i| i != r) {
                let factor = data[i * cols + lead];
                for j in 0..cols {
                    data[i * cols + j] -= factor * data[r * cols + j];
                }
            }
            r += 1;
            if r == rows {
                break;
            }
        }
        data.chunks_exact(cols)
            .take_while(|row| row.iter().any(|&x| !is_zero(x)))
            .count() as f64
    })
}

pub(super) fn apply(list: List<'_>, src: Value) -> Option<Value> {
    let size = list.len();
    if !matches!(size, 2 | 3) {
        return None;
    }
    let mut data = [0.0; 9];
    for (entry, dest) in cells(list, size).zip(&mut data) {
        *dest = entry?;
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
        (_, Value::Point { x, y } | Value::Vector { x, y }) => {
            // Both GeoPoint and GeoVector use homogeneous input (x, y, 1)
            // for a 3-by-3 matrix.
            let (x, y) = if size == 2 {
                (data[0] * x + data[1] * y, data[2] * x + data[3] * y)
            } else {
                let w = data[6] * x + data[7] * y + data[8];
                let tx = data[0] * x + data[1] * y + data[2];
                let ty = data[3] * x + data[4] * y + data[5];
                if matches!(src, Value::Point { .. }) && zero_point_scale(w, tx, ty) {
                    return None;
                }
                (tx / w, ty / w)
            };
            if matches!(src, Value::Point { .. }) {
                Some(Value::Point { x, y })
            } else {
                Some(Value::Vector { x, y })
            }
        }
        _ => None,
    }
}
