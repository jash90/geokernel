// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoApplyMatrix.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDimension.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoInvert.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConic.java
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
use crate::kernel::{Construction, Def, Id, Kind, Value};
use crate::script::split_call;
use crate::tol::STANDARD_PRECISION;
use std::cell::RefCell;

thread_local! {
    // Reuse the storage when a slider changes the entries of an existing matrix.
    static SCRATCH: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

fn row(element: Element<'_>) -> Option<List<'_>> {
    match element {
        Element::List(row) => Some(row),
        Element::Value(..) | Element::Text(_) => None,
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
fn eliminate<F: FnMut(u64) -> bool, T>(
    list: List<'_>,
    square: bool,
    read: fn(List<'_>, usize) -> Option<f64>,
    mut charge: F,
    f: impl FnOnce(&mut [f64], usize, usize, F) -> Option<T>,
) -> Option<T> {
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
        for item in list.iter() {
            let row = row(item).filter(|r| r.len() == cols)?;
            for col in 0..cols {
                data.push(read(row, col)?);
            }
        }
        f(data, rows, cols, charge)
    })
}

pub(crate) fn determinant(list: List<'_>, charge: impl FnMut(u64) -> bool) -> Option<f64> {
    eliminate(
        list,
        true,
        finite_cell,
        charge,
        |data, size, _, mut charge| {
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
        },
    )
}

fn swap_rows(data: &mut [f64], cols: usize, a: usize, b: usize) {
    for j in 0..cols {
        data.swap(a * cols + j, b * cols + j);
    }
}

pub(crate) fn rank(list: List<'_>, charge: impl FnMut(u64) -> bool) -> Option<f64> {
    eliminate(
        list,
        false,
        finite_cell,
        charge,
        |data, rows, cols, mut charge| {
            reduce(data, rows, cols, false, &mut charge)?;
            if !charge(data.len() as u64) {
                return None;
            }
            Some(
                data.chunks_exact(cols)
                    .take_while(|row| row.iter().any(|&x| !is_zero(x)))
                    .count() as f64,
            )
        },
    )
}

fn reduce(
    data: &mut [f64],
    rows: usize,
    cols: usize,
    preserve_zero_sign: bool,
    mut charge: impl FnMut(u64) -> bool,
) -> Option<()> {
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
        // Rank only counts nonzero values, so it can skip finite rows
        // when a subtraction would only change the sign of zero.
        let skip_zero =
            !preserve_zero_sign && data[r * cols..(r + 1) * cols].iter().all(|v| v.is_finite());
        for i in (0..rows).filter(|&i| i != r) {
            let factor = data[i * cols + lead];
            if factor == 0.0 && skip_zero {
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
    Some(())
}

/// A rectangular matrix of scalar objects, as recognized by GeoList.isMatrix.
pub(crate) fn dimensions(
    list: List<'_>,
    mut charge: impl FnMut(u64) -> bool,
) -> Option<(usize, usize)> {
    let rows = list.len();
    let cols = row(list.get(0)?)?.len();
    if cols == 0 || !charge(rows.checked_mul(cols)? as u64) {
        return None;
    }
    list.iter()
        .all(|item| {
            row(item).is_some_and(|r| {
                r.len() == cols
                    && r.iter().all(|cell| {
                        !matches!(cell, Element::List(_) | Element::Value(Kind::List, _))
                    })
            })
        })
        .then_some((rows, cols))
}

pub(crate) fn rref<T>(
    list: List<'_>,
    charge: impl FnMut(u64) -> bool,
    output: impl FnOnce(&[f64], usize, usize) -> Option<T>,
) -> Option<T> {
    eliminate(list, false, raw_cell, charge, |data, rows, cols, charge| {
        reduce(data, rows, cols, true, charge)?;
        output(data, rows, cols)
    })
}

fn finite_cell(row: List<'_>, col: usize) -> Option<f64> {
    numeric(row.get(col)?)
}

fn raw_cell(row: List<'_>, col: usize) -> Option<f64> {
    row.numeric_cell(col)
}

/// LU decomposition with partial pivoting and the singularity threshold
/// used by GgbMat. Solve for each column of the identity.
pub(crate) fn inverse<T>(
    list: List<'_>,
    charge: impl FnMut(u64) -> bool,
    output: impl FnOnce(&[f64], usize, usize) -> Option<T>,
) -> Option<T> {
    eliminate(list, true, raw_cell, charge, |lu, n, _, mut charge| {
        let len = lu.len();
        if !charge(len as u64) {
            return None;
        }
        let finite = lu.iter().all(|x| x.is_finite());
        let integers = finite && lu.iter().all(|&x| math::is_integer(x));
        let mut pivot: Vec<_> = (0..n).collect();
        let mut determinant = 1.0;
        for col in 0..n {
            // Calculate the upper column and the unscaled lower column in
            // the same order as the LU decomposition used by GgbMat.
            for row in 0..n {
                let terms = row.min(col);
                if !charge(terms as u64 + 1) {
                    return None;
                }
                let mut sum = lu[row * n + col];
                for i in 0..terms {
                    sum -= lu[row * n + i] * lu[i * n + col];
                }
                lu[row * n + col] = sum;
            }
            if !charge((3 * n) as u64) {
                return None;
            }
            let mut best = col;
            let mut largest = f64::NEG_INFINITY;
            for row in col..n {
                let magnitude = lu[row * n + col].abs();
                if magnitude > largest {
                    best = row;
                    largest = magnitude;
                }
            }
            if lu[best * n + col].abs() < STANDARD_PRECISION {
                return None;
            }
            if best != col {
                swap_rows(lu, n, best, col);
                pivot.swap(best, col);
                determinant = -determinant;
            }
            let diagonal = lu[col * n + col];
            determinant *= diagonal;
            for row in col + 1..n {
                lu[row * n + col] /= diagonal;
            }
        }
        if !charge(len as u64) {
            return None;
        }
        let mut result = vec![0.0; len];
        for (row, col) in pivot.into_iter().enumerate() {
            result[row * n + col] = 1.0;
        }
        let mut subtract_row = |result: &mut [f64], row: usize, col: usize| {
            let scale = lu[row * n + col];
            if !charge(n as u64) {
                return None;
            }
            for k in 0..n {
                result[row * n + k] -= result[col * n + k] * scale;
            }
            Some(())
        };
        for col in 0..n {
            for row in col + 1..n {
                subtract_row(&mut result, row, col)?;
            }
        }
        for col in (0..n).rev() {
            let diagonal = lu[col * n + col];
            for k in 0..n {
                result[col * n + k] /= diagonal;
            }
            for row in 0..col {
                subtract_row(&mut result, row, col)?;
            }
        }
        if integers {
            let det = math::java_round(determinant) as f64;
            for v in &mut result {
                *v = math::java_round(*v * det) as f64 / det;
            }
        }
        output(&result, n, n)
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Matrix {
    data: [f64; 9],
    size: usize,
}

impl Matrix {
    pub(crate) fn read(list: List<'_>) -> Option<Self> {
        let size = list.len();
        if !matches!(size, 2 | 3) {
            return None;
        }
        let mut data = [0.0; 9];
        for (entry, dest) in cells(list, size, matrix_cell).zip(&mut data) {
            *dest = entry?;
        }
        Some(Self { data, size })
    }

    pub(crate) fn area_scale(self) -> f64 {
        let [a, b, c, d, e, ..] = self.data;
        if self.size == 2 {
            a * d - b * c
        } else {
            a * e - b * d
        }
    }

    /// Homogeneous image of a Cartesian point (`z = 1`) or derivative
    /// (`z = 0`). In two dimensions the last coordinate stays equal to `z`.
    pub(crate) fn curve_coordinates(self, (x, y): (f64, f64), z: f64) -> (f64, f64, f64) {
        let [a, b, c, d, e, f, g, h, i] = self.data;
        if self.size == 2 {
            (a * x + b * y, c * x + d * y, z)
        } else {
            (
                a * x + b * y + c * z,
                d * x + e * y + f * z,
                g * x + h * y + i * z,
            )
        }
    }

    pub(crate) fn conic(self, out: &mut crate::algo::conic_nd::ConicData) {
        let [a, b, c, d, e, f, g, h, i] = self.data;
        if self.size == 2 {
            out.matrix_transform(a, b, c, d);
            return;
        }
        // GeoConic.matrixTransform: adj(M)^T * Q * adj(M), retaining
        // the homogeneous scale and the order of the coefficient sums.
        let adj = [
            [e * i - f * h, c * h - b * i, b * f - c * e],
            [f * g - d * i, a * i - c * g, c * d - a * f],
            [d * h - e * g, b * g - a * h, a * e - b * d],
        ];
        let [xx, yy, zz, xy, xz, yz] = out.matrix;
        let q = [[xx, xy, xz], [xy, yy, yz], [xz, yz, zz]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let columns = std::array::from_fn::<_, 3, _>(|k| [adj[0][k], adj[1][k], adj[2][k]]);
        let product = columns.map(|column| q.map(|row| dot(row, column)));
        out.matrix = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)]
            .map(|(r, c)| dot(columns[r], product[c]));
        out.classify(false);
    }

    pub(crate) fn apply(self, src: Value) -> Option<Value> {
        let Self { data, size } = self;
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
}

// Point, vector, line, conic and polygon commands need named construction
// dependencies. List targets are built by the expression evaluator.
pub(crate) fn exec(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let ("ApplyMatrix", args) = split_call(rhs)? else {
        return None;
    };
    let &[matrix, target] = args.as_slice() else {
        return None;
    };
    let src = c.id(target)?;
    let kind = c.kind(src);
    if !matches!(
        kind,
        Kind::Point
            | Kind::Vector
            | Kind::Line
            | Kind::Circle
            | Kind::Conic
            | Kind::Polygon
            | Kind::PolyLine
            | Kind::Curve
            | Kind::Function
    ) {
        return None;
    }
    let checkpoint = c.checkpoint();
    let result = (|| {
        let matrix = match c.id(matrix) {
            Some(id) => id,
            None => {
                let label = format!("\0arg{}", c.len());
                if !crate::expr::argument_object(c, matrix, &label, Kind::List) {
                    return Err("`ApplyMatrix` needs a matrix list".into());
                }
                c.id(&label).expect("argument object was just added")
            }
        };
        let def = if matches!(kind, Kind::Circle | Kind::Conic) {
            Def::GeneralConic(crate::algo::general_conic::GeneralConicDef::ApplyMatrix {
                matrix,
                src,
            })
        } else {
            Def::Vector(super::VectorDef::ApplyMatrix { matrix, src, kind })
        };
        if matches!(kind, Kind::Polygon | Kind::PolyLine)
            && crate::algo::polygon::has_fixed_vertices(c, src)
        {
            crate::algo::polygon::add_transform(c, name, src, def)
        } else {
            c.add(name, def).map_err(|e| e.to_string())
        }
    })();
    if result.is_err() {
        c.discard_since(checkpoint);
    }
    Some(result.map(Some))
}
