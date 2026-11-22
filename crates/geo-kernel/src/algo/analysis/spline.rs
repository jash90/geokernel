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
        // The caller checks the degree against the point count of the last
        // solve; it can exceed the current count.
        let degree = super::spline_degree(c, input)?;
        if degree < 3 || n < 2 {
            return None;
        }
        let stride = degree + 1;
        let cols = (n - 1).checked_mul(stride)?;
        let work = IntersectionWork::new();
        // A row holds at most two pieces (the closed conditions hold the
        // first and the last), and each entry costs a power of up to the
        // degree: bound both before allocating.
        let entries = cols.checked_mul(2 * stride)?;
        if entries > MAX_ENTRIES || !work.charge((entries as u64).saturating_mul(stride as u64)) {
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
        let lengths = knots.clone();
        knots.iter_mut().for_each(|knot| *knot /= total);

        // The rows of `AlgoSpline.getLinearSystemParametric`, in its order
        // and with its arithmetic: values at both ends of each piece, the
        // derivative conditions at the inner knots from the first
        // derivative down, then the end conditions.
        let mut matrix = vec![Row::default(); cols];
        let mut row = 0;
        for end in 0..=1 {
            for piece in 0..n - 1 {
                let i = piece + end;
                for j in (0..stride).rev() {
                    *matrix[row].slot(piece * stride + stride - j - 1) = pow(knots[i], j as i32);
                }
                matrix[row].rhs = points[i];
                row += 1;
            }
        }
        for derivative in (1..stride - 1).rev() {
            for (piece, &knot) in knots[1..n - 1].iter().enumerate() {
                for k in 0..stride {
                    let value = coefficient(stride, k, derivative, knot);
                    *matrix[row].slot(piece * stride + k) = value;
                    *matrix[row].slot((piece + 1) * stride + k) = -value;
                }
                row += 1;
            }
        }
        let last = cols - stride;
        if points.first().zip(points.last()).is_some_and(|(a, b)| {
            (a[0] - b[0]).abs() < crate::tol::STANDARD_PRECISION
                && (a[1] - b[1]).abs() < crate::tol::STANDARD_PRECISION
        }) {
            for derivative in (1..stride - 1).rev() {
                for k in 0..stride {
                    *matrix[row].slot(k) = coefficient(stride, k, derivative, 0.0);
                    *matrix[row].slot(last + k) = -coefficient(stride, k, derivative, 1.0);
                }
                row += 1;
            }
            // `getLinearSystemParametric` skips one row after the closed
            // conditions, past the end of the system.
            row += 1;
        } else {
            *matrix[row].slot(0) = 0.0;
            *matrix[row].slot(1) = fact(degree - 1);
            row += 1;
            *matrix[row].slot(last) = fact(degree);
            *matrix[row].slot(last + 1) = fact(degree - 1);
            row += 1;
        }
        for (i, row) in matrix[row.min(cols)..].iter_mut().enumerate() {
            let num = i + 2;
            let piece = n.checked_sub(num + 1)?;
            *row.slot(piece * stride) = fact(degree) * lengths[num - 1] / total;
            *row.slot(piece * stride + 1) = fact(degree - 1);
        }
        let coefficients = eliminate(matrix, cols, &work)?;
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
        if derivative == 0 {
            // The piece as `AlgoSpline.compute` writes it: the terms
            // `c * t^j` added from the highest power, a constant term
            // within `MAX_PRECISION` of zero left out.
            let terms = row
                .iter()
                .enumerate()
                .map(|(i, &pair)| (self.stride - 1 - i, pair));
            let [x, y] = terms.fold([0.0; 2], |sum, (power, pair)| {
                let t_power = crate::expr::math::my_pow(t, power as f64);
                [0, 1].map(|axis| {
                    let coeff = pair[axis];
                    let negligible = power == 0
                        && -crate::tol::MAX_PRECISION < coeff
                        && coeff < crate::tol::MAX_PRECISION;
                    if negligible {
                        sum[axis]
                    } else {
                        sum[axis] + coeff * t_power
                    }
                })
            });
            return (x, y);
        }
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

/// `AlgoSpline.fact`: the factorial in a 32-bit integer, which wraps from
/// 13! on, returned as a `float`.
fn fact(n: usize) -> f64 {
    let product = (2..=n).fold(1_i32, |f, j| f.wrapping_mul(j as i32));
    f64::from(product as f32)
}

fn factor(power: usize, derivative: usize) -> f64 {
    (0..derivative).fold(1.0, |f, k| f * (power - k) as f64)
}

/// `Math.pow` for the integer powers of the system, correctly rounded:
/// the product is carried as an unevaluated sum of two doubles (exact
/// products by fused multiply-add) and rounded once. Java's `Math.pow`
/// is only specified within one ulp; `x * x` is exact rounding for the
/// square.
fn pow(t: f64, power: i32) -> f64 {
    match power {
        0 => 1.0,
        1 => t,
        2 => t * t,
        _ if power < 0 || !t.is_finite() => t.powf(f64::from(power)),
        _ => {
            let (mut hi, mut lo) = (t, 0.0_f64);
            for _ in 1..power {
                let product = hi * t;
                let error = hi.mul_add(t, -product);
                lo = lo.mul_add(t, error);
                hi = product;
            }
            let sum = hi + lo;
            if sum.is_finite() {
                sum
            } else {
                t.powf(f64::from(power))
            }
        }
    }
}

/// `AlgoSpline.calcCoeff`: the derivative condition's entry of column `k`
/// of a piece. It keeps the power `t^(exp - 1)` for every derivative order,
/// which scales the homogeneous row by a power of the knot.
fn coefficient(stride: usize, k: usize, derivative: usize, t: f64) -> f64 {
    let mut exp = (stride - k - 1) as i32;
    if exp == 0 {
        return 0.0;
    }
    let mut coeff = pow(t, exp - 1);
    for _ in derivative + 1..stride {
        coeff *= f64::from(exp);
        exp -= 1;
    }
    coeff
}

/// Bound on the stored entries of the system, about 32 MB.
const MAX_ENTRIES: usize = 1 << 22;

/// One row of the spline system: the entries from column `lo` on, the
/// columns past them are zero, and the two right-hand sides.
#[derive(Clone, Default)]
struct Row {
    lo: usize,
    values: Vec<f64>,
    rhs: [f64; 2],
}

impl Row {
    fn hi(&self) -> usize {
        self.lo + self.values.len()
    }

    fn get(&self, col: usize) -> f64 {
        col.checked_sub(self.lo)
            .and_then(|k| self.values.get(k))
            .copied()
            .unwrap_or(0.0)
    }

    /// The entry of `col`, widening the stored range with zeros.
    fn slot(&mut self, col: usize) -> &mut f64 {
        if self.values.is_empty() {
            self.lo = col;
        } else if col < self.lo {
            let mut values = vec![0.0; self.lo - col];
            values.append(&mut self.values);
            self.values = values;
            self.lo = col;
        }
        let k = col - self.lo;
        if k >= self.values.len() {
            self.values.resize(k + 1, 0.0);
        }
        &mut self.values[k]
    }

    /// Drops the leading zero entries, which never take part in a step.
    fn trim(&mut self) {
        let zeros = self.values.iter().take_while(|&&v| v == 0.0).count();
        self.values.drain(..zeros);
        self.lo = if self.values.is_empty() {
            usize::MAX
        } else {
            self.lo + zeros
        };
    }
}

/// `AlgoSpline.getSystemSolution` on the sparse rows, with the same
/// operations on every nonzero entry, so the results are bit-identical
/// while the work follows the band of the system.
///
/// For each column the reference sorts the remaining rows by the magnitude
/// of their entry with a selection sort (swapping whenever a later row is
/// strictly larger), takes the first row as the pivot and eliminates the
/// rows below in their new order, rescaling the pivot row by each
/// multiplier before subtracting it. Only rows with a nonzero entry in the
/// column take part in a swap or an elimination. A column without a
/// nonzero entry leaves the system without a solution. The work and the
/// stored entries are bounded; past either bound there is no solution.
fn eliminate(mut rows: Vec<Row>, n: usize, work: &IntersectionWork) -> Option<Vec<[f64; 2]>> {
    let mut stored = 0usize;
    // Rows by the column of their first nonzero entry.
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (r, row) in rows.iter_mut().enumerate() {
        row.trim();
        stored += row.values.len();
        if row.lo < n {
            buckets[row.lo].push(r);
        }
    }
    // `at[p]` is the row at position `p`, `position[r]` the position of row `r`.
    let mut position: Vec<usize> = (0..n).collect();
    let mut at: Vec<usize> = (0..n).collect();
    for i in 0..n {
        let candidates = std::mem::take(&mut buckets[i]);
        let k = candidates.len() as u64;
        if candidates.is_empty() || !work.charge(k * k + 1) {
            return None;
        }
        let key = |r: usize, rows: &[Row]| rows[r].get(i).abs();
        // The selection sort over positions i.., restricted to the
        // positions that hold a nonzero key: a zero key never displaces
        // anything, and a pass whose position ends with a zero key leaves
        // no nonzero key behind it.
        let mut nonzero: Vec<usize> = candidates.iter().map(|&r| position[r]).collect();
        nonzero.sort_unstable();
        let mut pass = i;
        while nonzero.last().is_some_and(|&p| p > pass) {
            let mut current = at[pass];
            let mut current_key = if nonzero.binary_search(&pass).is_ok() {
                key(current, &rows)
            } else {
                0.0
            };
            let mut j = nonzero.partition_point(|&p| p <= pass);
            while j < nonzero.len() {
                let q = nonzero[j];
                let other = at[q];
                let other_key = key(other, &rows);
                if current_key < other_key {
                    at[pass] = other;
                    at[q] = current;
                    position[other] = pass;
                    position[current] = q;
                    if current_key == 0.0 {
                        // the zero key moves to q, the pass position gains a nonzero key
                        nonzero.remove(j);
                        let at_pass = nonzero.partition_point(|&p| p < pass);
                        nonzero.insert(at_pass, pass);
                    }
                    current = other;
                    current_key = other_key;
                }
                j += 1;
            }
            if current_key == 0.0 {
                break;
            }
            pass += 1;
        }
        let pivot = at[i];
        let mut targets: Vec<usize> = candidates.into_iter().filter(|&r| r != pivot).collect();
        targets.sort_unstable_by_key(|&r| position[r]);
        let mut pivot_row = std::mem::take(&mut rows[pivot]);
        let span = pivot_row.hi() - i;
        for r in targets {
            if !work.charge(2 * span as u64 + 4) {
                return None;
            }
            let row = &mut rows[r];
            let value = row.get(i) / pivot_row.values[0];
            pivot_row.values.iter_mut().for_each(|p| *p *= value);
            pivot_row.rhs.iter_mut().for_each(|p| *p *= value);
            let before = row.values.len();
            if pivot_row.hi() > row.hi() {
                *row.slot(pivot_row.hi() - 1) += 0.0;
            }
            stored += row.values.len() - before;
            if stored > MAX_ENTRIES {
                return None;
            }
            for (value, p) in row.values.iter_mut().zip(&pivot_row.values) {
                *value -= p;
            }
            for (value, p) in row.rhs.iter_mut().zip(pivot_row.rhs) {
                *value -= p;
            }
            // Column i keeps a rounding residue that is never read again.
            row.values.remove(0);
            row.lo += 1;
            let before = row.values.len();
            row.trim();
            stored -= before - row.values.len();
            if row.lo < n {
                buckets[row.lo].push(r);
            }
        }
        if pivot_row.values[0] == 0.0 || !pivot_row.values[0].is_finite() {
            return None;
        }
        rows[pivot] = pivot_row;
    }
    let mut out = vec![[0.0; 2]; n];
    for i in (0..n).rev() {
        let row = &rows[at[i]];
        if !work.charge((row.hi() - i) as u64 + 1) {
            return None;
        }
        // The reference sums the known terms from the last column down.
        let mut buffer = [0.0; 2];
        for (&a, &[x, y]) in row.values[1..].iter().zip(&out[i + 1..]).rev() {
            buffer[0] += x * a;
            buffer[1] += y * a;
        }
        let x = [
            (row.rhs[0] - buffer[0]) / row.values[0],
            (row.rhs[1] - buffer[1]) / row.values[0],
        ];
        if x.iter().any(|v| !v.is_finite()) {
            return None;
        }
        out[i] = x;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use crate::{script, Construction, Value};
    use std::time::{Duration, Instant};

    fn run(lines: &[&str]) -> Construction {
        let mut c = Construction::new();
        for line in lines {
            script::exec(&mut c, line).unwrap();
        }
        c
    }

    fn defined(c: &Construction, label: &str) -> bool {
        !matches!(c.value(c.id(label).unwrap()), Value::Undefined)
    }

    #[test]
    fn long_systems_stay_inside_the_work_bound() {
        // A high degree makes the system dense: it is refused or solved
        // within the bound, never left running.
        for (n, degree) in [(300, 300), (2000, 3), (60, 60), (5000, 4)] {
            let start = Instant::now();
            let c = run(&[
                &format!("n = {n}"),
                &format!("s = Spline(Sequence((k, Mod(k^2, 7)), k, 1, n), {degree})"),
            ]);
            let _ = defined(&c, "s");
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "{n} points, degree {degree}"
            );
        }
        let c = run(&[
            "n = 2000",
            "s = Spline(Sequence((k, Mod(k^2, 7)), k, 1, n))",
        ]);
        assert!(defined(&c, "s"));
    }

    #[test]
    fn the_degree_check_reads_the_last_accepted_point_count() {
        let mut c = run(&["n = 4", "s = Spline(Sequence((k, Mod(k, 3)), k, 1, n))"]);
        let n = c.id("n").unwrap();
        let mut seen = Vec::new();
        for value in [2.0, 1.0, 4.0] {
            c.set_number(n, value).unwrap();
            seen.push(defined(&c, "s"));
        }
        assert_eq!(seen, [true, false, false]);
        // a new object starts from its own list
        script::exec(&mut c, "t = Spline(Sequence((k, Mod(k, 3)), k, 1, n))").unwrap();
        assert!(defined(&c, "t"));
    }
}

#[cfg(test)]
mod locus_tests {
    use crate::{script, Construction, Value};

    /// Locus sampling works on a copy: the live spline keeps its
    /// remembered point count (case `funcrest-splinelong-009`).
    #[test]
    fn locus_sampling_leaves_the_live_state() {
        let mut c = Construction::new();
        for line in ["a = 4", "l0 = Sequence((k, k), k, 1, a)", "s = Spline(l0)"] {
            script::exec(&mut c, line).unwrap();
        }
        let a = c.id("a").unwrap();
        c.restore_slider_bounds(a, [Some("2"), Some("4")]).unwrap();
        for line in [
            "P = Point(s, 0.5)",
            "O = (0, 0)",
            "Q = Midpoint(P, O)",
            "l = Locus(Q, a)",
        ] {
            script::exec(&mut c, line).unwrap();
        }
        // the samples at two points do not shrink the live count of four
        c.set_number(a, 3.0).unwrap();
        assert!(!matches!(c.value(c.id("s").unwrap()), Value::Undefined));
    }
}
