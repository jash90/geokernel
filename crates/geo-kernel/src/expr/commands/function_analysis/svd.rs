//! Thin singular value decomposition using cyclic one-sided Jacobi rotations.

type Columns = Vec<Vec<f64>>;

/// Returns left vectors, descending singular values and right vectors.
pub(super) fn decompose(
    mut left: Columns,
    charge: impl Fn(u64) -> bool,
) -> Option<(Columns, Vec<f64>, Columns)> {
    const MAX_SWEEPS: usize = 64;
    let n = left.len();
    let m = left[0].len();
    let units = (3 * m + 3 * n) as u64;
    if !charge((4 * m as u64 + 5 * n as u64) * n as u64) {
        return None;
    }
    let scale = left
        .iter()
        .flatten()
        .fold(0.0_f64, |acc, x| acc.max(x.abs()));
    let mut right = vec![vec![0.0; n]; n];
    for (i, column) in right.iter_mut().enumerate() {
        column[i] = if scale == 0.0 { -1.0 } else { 1.0 };
    }
    if scale != 0.0 {
        left.iter_mut().flatten().for_each(|x| *x /= scale);
    }
    for _ in 0..MAX_SWEEPS {
        let mut rotated = false;
        for i in 0..n {
            for j in i + 1..n {
                if !charge(units) {
                    return None;
                }
                let (a, b) = pair(&mut left, i, j);
                let (na, nb) = (norm(a), norm(b));
                if na == 0.0 || nb == 0.0 {
                    continue;
                }
                let dot: f64 = a
                    .iter()
                    .zip(b.iter())
                    .map(|(&x, &y)| (x / na) * (y / nb))
                    .sum();
                if dot.abs() <= 8.0 * f64::EPSILON {
                    continue;
                }
                let largest = na.max(nb);
                let (alpha, beta) = (na / largest, nb / largest);
                let cross = alpha * beta * dot;
                let delta = (beta - alpha) * (beta + alpha);
                let t = (2.0 * cross) / (delta + delta.signum() * delta.hypot(2.0 * cross));
                if !t.is_finite() || t == 0.0 {
                    return None;
                }
                let cos = 1.0 / t.hypot(1.0);
                let sin = t * cos;
                rotate(a, b, cos, sin);
                let (a, b) = pair(&mut right, i, j);
                rotate(a, b, cos, sin);
                rotated = true;
            }
        }
        if !rotated {
            let mut order: Vec<_> = left.iter().map(|col| norm(col)).enumerate().collect();
            order.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mut basis: Columns = Vec::with_capacity(n);
            let mut singular = Vec::with_capacity(n);
            let mut vectors = Vec::with_capacity(n);
            for (i, length) in order {
                let mut column = std::mem::take(&mut left[i]);
                let sigma = length * scale;
                if !sigma.is_finite() {
                    return None;
                }
                if length == 0.0 {
                    complete(&mut column, &basis, &charge)?;
                } else {
                    column.iter_mut().for_each(|x| *x /= length);
                }
                basis.push(column);
                singular.push(sigma);
                vectors.push(std::mem::take(&mut right[i]));
            }
            return Some((basis, singular, vectors));
        }
    }
    None
}

fn pair(columns: &mut Columns, i: usize, j: usize) -> (&mut [f64], &mut [f64]) {
    let (before, after) = columns.split_at_mut(j);
    (&mut before[i], &mut after[0])
}

fn norm(column: &[f64]) -> f64 {
    column.iter().fold(0.0_f64, |norm, &x| norm.hypot(x))
}

fn rotate(a: &mut [f64], b: &mut [f64], c: f64, s: f64) {
    for (x, y) in a.iter_mut().zip(b) {
        (*x, *y) = (c * *x - s * *y, s * *x + c * *y);
    }
}

fn complete(column: &mut [f64], basis: &Columns, charge: &impl Fn(u64) -> bool) -> Option<()> {
    let work = (column.len() as u64) * (2 + 4 * basis.len() as u64);
    for i in 0..column.len() {
        if !charge(work) {
            return None;
        }
        column.fill(0.0);
        column[i] = 1.0;
        for _ in 0..2 {
            for other in basis {
                let dot: f64 = column.iter().zip(other).map(|(x, y)| x * y).sum();
                for (x, y) in column.iter_mut().zip(other) {
                    *x -= dot * y;
                }
            }
        }
        let length = norm(column);
        if length > 0.25 {
            column.iter_mut().for_each(|x| *x /= length);
            return Some(());
        }
    }
    None
}
