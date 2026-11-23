//! Thin singular value decomposition by Householder bidiagonalization and
//! implicitly shifted QR steps: the Golub-Kahan-Reinsch algorithm (Golub and
//! Reinsch, *Singular value decomposition and least squares solutions*,
//! Numerische Mathematik 14, 1970), in the formulation of the public-domain
//! JAMA package (NIST and MathWorks, 1998), which follows LINPACK's `dsvdc`.
//!
//! The reference delegates `SVD` to a bundled decomposition whose source is
//! not used here. Its conventions were fixed from black-box recordings of
//! the reference (cases `funcrest-svd-*` and a set of 72 random, rank
//! deficient and degenerate matrices recorded through the harness), and
//! agree with the algorithm above in every case:
//!
//! - a matrix with fewer rows than columns is decomposed transposed, and
//!   the two factors are swapped back;
//! - column and row norms accumulate with `hypot` as computed by fdlibm's
//!   `e_hypot` (Java's `Math.hypot`), whose last-bit rounding decides the
//!   null-space vectors of rank-deficient matrices;
//! - a `NaN` superdiagonal entry counts as negligible, so a matrix with
//!   `NaN` or infinite entries still gives a result.

type Rows = Vec<Vec<f64>>;

/// Decomposes the `m` by `n` matrix `a` (`m >= n >= 1`, row-major) into
/// `U` (`m` by `n`), the `n` singular values in descending order and `V`
/// (`n` by `n`), with `a = U diag(s) V^T`. `charge` is asked for the work
/// of every vector operation; `None` when it refuses.
pub(super) fn decompose(
    mut a: Rows,
    charge: impl Fn(u64) -> bool,
) -> Option<(Rows, Vec<f64>, Rows)> {
    let m = a.len();
    let n = a[0].len();
    debug_assert!(m >= n && n >= 1);
    let (mu, nu) = (m as u64, n as u64);
    let nct = (m - 1).min(n);
    let nrt = n.saturating_sub(2).min(m);
    let mut s = vec![0.0; (m + 1).min(n)];
    let mut u = vec![vec![0.0; n]; m];
    let mut v = vec![vec![0.0; n]; n];
    let mut e = vec![0.0; n];
    let mut work = vec![0.0; m];

    // Reduce `a` to bidiagonal form, storing the diagonal in `s` and the
    // superdiagonal in `e`.
    for k in 0..nct.max(nrt) {
        if !charge(mu * (nu - k as u64 + 1) * 2) {
            return None;
        }
        if k < nct {
            s[k] = 0.0;
            for row in &a[k..] {
                s[k] = hypot(s[k], row[k]);
            }
            if s[k] != 0.0 {
                if a[k][k] < 0.0 {
                    s[k] = -s[k];
                }
                for row in &mut a[k..] {
                    row[k] /= s[k];
                }
                a[k][k] += 1.0;
            }
            s[k] = -s[k];
        }
        for j in k + 1..n {
            if k < nct && s[k] != 0.0 {
                let mut t = 0.0;
                for row in &a[k..] {
                    t += row[k] * row[j];
                }
                t = -t / a[k][k];
                for row in &mut a[k..] {
                    row[j] += t * row[k];
                }
            }
            e[j] = a[k][j];
        }
        if k < nct {
            for i in k..m {
                u[i][k] = a[i][k];
            }
        }
        if k < nrt {
            e[k] = 0.0;
            for i in k + 1..n {
                e[k] = hypot(e[k], e[i]);
            }
            if e[k] != 0.0 {
                if e[k + 1] < 0.0 {
                    e[k] = -e[k];
                }
                for i in k + 1..n {
                    e[i] /= e[k];
                }
                e[k + 1] += 1.0;
            }
            e[k] = -e[k];
            if k + 1 < m && e[k] != 0.0 {
                work[k + 1..].fill(0.0);
                for j in k + 1..n {
                    for i in k + 1..m {
                        work[i] += e[j] * a[i][j];
                    }
                }
                for j in k + 1..n {
                    let t = -e[j] / e[k + 1];
                    for i in k + 1..m {
                        a[i][j] += t * work[i];
                    }
                }
            }
            for i in k + 1..n {
                v[i][k] = e[i];
            }
        }
    }

    // The final bidiagonal matrix of order `p`.
    let mut p = n.min(m + 1);
    if nct < n {
        s[nct] = a[nct][nct];
    }
    if m < p {
        s[p - 1] = 0.0;
    }
    if nrt + 1 < p {
        e[nrt] = a[nrt][p - 1];
    }
    e[p - 1] = 0.0;
    drop(a);

    // Generate U.
    for j in nct..n {
        for row in u.iter_mut() {
            row[j] = 0.0;
        }
        u[j][j] = 1.0;
    }
    for k in (0..nct).rev() {
        if !charge(mu * (nu - k as u64 + 1) * 2) {
            return None;
        }
        if s[k] != 0.0 {
            for j in k + 1..n {
                let mut t = 0.0;
                for row in &u[k..] {
                    t += row[k] * row[j];
                }
                t = -t / u[k][k];
                for row in &mut u[k..] {
                    row[j] += t * row[k];
                }
            }
            for row in &mut u[k..] {
                row[k] = -row[k];
            }
            u[k][k] += 1.0;
            for row in &mut u[..k.saturating_sub(1)] {
                row[k] = 0.0;
            }
        } else {
            for row in u.iter_mut() {
                row[k] = 0.0;
            }
            u[k][k] = 1.0;
        }
    }

    // Generate V.
    for k in (0..n).rev() {
        if !charge(nu * (nu - k as u64 + 1) * 2) {
            return None;
        }
        if k < nrt && e[k] != 0.0 {
            for j in k + 1..n {
                let mut t = 0.0;
                for row in &v[k + 1..] {
                    t += row[k] * row[j];
                }
                t = -t / v[k + 1][k];
                for row in &mut v[k + 1..] {
                    row[j] += t * row[k];
                }
            }
        }
        for row in v.iter_mut() {
            row[k] = 0.0;
        }
        v[k][k] = 1.0;
    }

    // Main iteration loop for the singular values.
    let pp = p - 1;
    let eps = f64::EPSILON;
    let tiny = 2.0_f64.powi(-966);
    // `x <= bound` or unordered: a `NaN` entry is negligible.
    let negligible = |x: f64, bound: f64| x.abs() <= bound || x.is_nan() || bound.is_nan();
    while p > 0 {
        // the largest `k < p - 1` with a negligible `e[k]`, or `None`
        let mut k = None;
        for i in (0..p - 1).rev() {
            if negligible(e[i], tiny + eps * (s[i].abs() + s[i + 1].abs())) {
                e[i] = 0.0;
                k = Some(i);
                break;
            }
        }
        let kase;
        let start;
        if k == Some(p.wrapping_sub(2)) || p == 1 {
            kase = 4;
            start = p - 1;
        } else {
            // the largest `ks` in `k + 1..p` with a negligible `s[ks]`
            let low = k.map_or(0, |k| k + 1);
            let mut found = None;
            for ks in (low..p).rev() {
                let t = (if ks != p { e[ks].abs() } else { 0.0 })
                    + (if ks != low { e[ks - 1].abs() } else { 0.0 });
                if negligible(s[ks], tiny + eps * t) {
                    s[ks] = 0.0;
                    found = Some(ks);
                    break;
                }
            }
            match found {
                None => {
                    kase = 3;
                    start = low;
                }
                Some(ks) if ks == p - 1 => {
                    kase = 1;
                    start = low;
                }
                Some(ks) => {
                    kase = 2;
                    start = ks + 1;
                }
            }
        }
        if !charge((mu + nu) * (p - start) as u64 * 2) {
            return None;
        }
        match kase {
            // Deflate a negligible `s[p - 1]`.
            1 => {
                let mut f = e[p - 2];
                e[p - 2] = 0.0;
                for j in (start..=p - 2).rev() {
                    let t = hypot(s[j], f);
                    let (cs, sn) = (s[j] / t, f / t);
                    s[j] = t;
                    if j != start {
                        f = -sn * e[j - 1];
                        e[j - 1] *= cs;
                    }
                    for row in v.iter_mut() {
                        let t = cs * row[j] + sn * row[p - 1];
                        row[p - 1] = -sn * row[j] + cs * row[p - 1];
                        row[j] = t;
                    }
                }
            }
            // Split at a negligible `s[start - 1]`.
            2 => {
                let k = start - 1;
                let mut f = e[k];
                e[k] = 0.0;
                for j in start..p {
                    let t = hypot(s[j], f);
                    let (cs, sn) = (s[j] / t, f / t);
                    s[j] = t;
                    f = -sn * e[j];
                    e[j] *= cs;
                    for row in u.iter_mut() {
                        let t = cs * row[j] + sn * row[k];
                        row[k] = -sn * row[j] + cs * row[k];
                        row[j] = t;
                    }
                }
            }
            // One QR step.
            3 => {
                let k = start;
                let scale = s[p - 1]
                    .abs()
                    .max(s[p - 2].abs())
                    .max(e[p - 2].abs())
                    .max(s[k].abs())
                    .max(e[k].abs());
                let sp = s[p - 1] / scale;
                let spm1 = s[p - 2] / scale;
                let epm1 = e[p - 2] / scale;
                let sk = s[k] / scale;
                let ek = e[k] / scale;
                let b = ((spm1 + sp) * (spm1 - sp) + epm1 * epm1) / 2.0;
                let c = (sp * epm1) * (sp * epm1);
                let mut shift = 0.0;
                if b != 0.0 || c != 0.0 {
                    shift = (b * b + c).sqrt();
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c / (b + shift);
                }
                let mut f = (sk + sp) * (sk - sp) + shift;
                let mut g = sk * ek;
                for j in k..p - 1 {
                    let t = hypot(f, g);
                    let (cs, sn) = (f / t, g / t);
                    if j != k {
                        e[j - 1] = t;
                    }
                    f = cs * s[j] + sn * e[j];
                    e[j] = cs * e[j] - sn * s[j];
                    g = sn * s[j + 1];
                    s[j + 1] *= cs;
                    for row in v.iter_mut() {
                        let t = cs * row[j] + sn * row[j + 1];
                        row[j + 1] = -sn * row[j] + cs * row[j + 1];
                        row[j] = t;
                    }
                    let t = hypot(f, g);
                    let (cs, sn) = (f / t, g / t);
                    s[j] = t;
                    f = cs * e[j] + sn * s[j + 1];
                    s[j + 1] = -sn * e[j] + cs * s[j + 1];
                    g = sn * e[j + 1];
                    e[j + 1] *= cs;
                    if j < m - 1 {
                        for row in u.iter_mut() {
                            let t = cs * row[j] + sn * row[j + 1];
                            row[j + 1] = -sn * row[j] + cs * row[j + 1];
                            row[j] = t;
                        }
                    }
                }
                e[p - 2] = f;
            }
            // Convergence: make the singular value nonnegative and sort.
            _ => {
                let mut k = start;
                if s[k] <= 0.0 {
                    s[k] = if s[k] < 0.0 { -s[k] } else { 0.0 };
                    for row in &mut v[..=pp] {
                        row[k] = -row[k];
                    }
                }
                while k < pp {
                    if s[k] >= s[k + 1] {
                        break;
                    }
                    s.swap(k, k + 1);
                    if k < n - 1 {
                        for row in v.iter_mut() {
                            row.swap(k, k + 1);
                        }
                    }
                    if k < m - 1 {
                        for row in u.iter_mut() {
                            row.swap(k, k + 1);
                        }
                    }
                    k += 1;
                }
                p -= 1;
            }
        }
    }
    s.truncate(n);
    Some((u, s, v))
}

/// `sqrt(x^2 + y^2)` as fdlibm's `e_hypot` computes it (the algorithm of
/// Java's `StrictMath.hypot`): error below one ulp, not always correctly
/// rounded. An infinite argument gives infinity, even with a `NaN`.
fn hypot(x: f64, y: f64) -> f64 {
    let high = |v: f64| (v.to_bits() >> 32) as u32;
    let low = |v: f64| v.to_bits() as u32;
    let with_high =
        |v: f64, h: u32| f64::from_bits((u64::from(h) << 32) | (v.to_bits() & 0xffff_ffff));
    let from_high = |h: u32| f64::from_bits(u64::from(h) << 32);
    let (mut ha, mut hb) = (high(x) & 0x7fff_ffff, high(y) & 0x7fff_ffff);
    let (mut a, mut b) = if hb > ha {
        std::mem::swap(&mut ha, &mut hb);
        (y, x)
    } else {
        (x, y)
    };
    a = with_high(a, ha);
    b = with_high(b, hb);
    if ha - hb > 0x03c0_0000 {
        // a / b > 2^60
        return a + b;
    }
    let mut k: i32 = 0;
    if ha > 0x5f30_0000 {
        // a > 2^500
        if ha >= 0x7ff0_0000 {
            // infinity or NaN
            let mut w = a + b;
            if ((ha & 0x000f_ffff) | low(a)) == 0 {
                w = a;
            }
            if ((hb ^ 0x7ff0_0000) | low(b)) == 0 {
                w = b;
            }
            return w;
        }
        ha -= 0x2580_0000;
        hb -= 0x2580_0000;
        k += 600;
        a = with_high(a, ha);
        b = with_high(b, hb);
    }
    if hb < 0x20b0_0000 {
        // b < 2^-500
        if hb <= 0x000f_ffff {
            // subnormal b or 0
            if (hb | low(b)) == 0 {
                return a;
            }
            let t1 = from_high(0x7fd0_0000); // 2^1022
            b *= t1;
            a *= t1;
            k -= 1022;
            ha = high(a);
            hb = high(b);
        } else {
            ha += 0x2580_0000;
            hb += 0x2580_0000;
            k -= 600;
            a = with_high(a, ha);
            b = with_high(b, hb);
        }
    }
    let mut w = a - b;
    if w > b {
        let t1 = from_high(ha);
        let t2 = a - t1;
        w = (t1 * t1 - (b * (-b) - t2 * (a + t1))).sqrt();
    } else {
        a += a;
        let y1 = from_high(hb);
        let y2 = b - y1;
        let t1 = from_high(ha + 0x0010_0000);
        let t2 = a - t1;
        w = (t1 * y1 - (w * (-w) - (t1 * y2 + t2 * b))).sqrt();
    }
    if k != 0 {
        from_high((0x3ff0_0000_i32 + (k << 20)) as u32) * w
    } else {
        w
    }
}

#[cfg(test)]
mod tests {
    use super::{decompose, hypot};

    #[test]
    fn hypot_follows_fdlibm() {
        // the accumulated norm of (3, 1, -3, -5) is one ulp above sqrt(44)
        let norm = [3.0, 1.0, -3.0, -5.0].iter().fold(0.0, |n, &x| hypot(n, x));
        assert_eq!(norm, 6.6332495807108005);
        assert_eq!(hypot(3.0, 4.0), 5.0);
        assert_eq!(hypot(f64::NAN, f64::INFINITY), f64::INFINITY);
        assert!(hypot(f64::NAN, 2.0).is_nan());
        assert_eq!(hypot(0.0, -3.0), 3.0);
        assert_eq!(hypot(1e300, 1e300), 1e300 * 2.0_f64.sqrt());
        assert_eq!(hypot(3e-310, 4e-310), 5e-310);
    }

    #[test]
    fn a_nan_entry_still_converges() {
        let (u, s, v) = decompose(vec![vec![f64::NAN, 1.0], vec![2.0, 3.0]], |_| true).unwrap();
        assert!(s.iter().all(|x| x.is_nan()));
        assert!(u.iter().flatten().all(|x| x.is_nan()));
        assert_eq!(v, [[0.0, 1.0], [1.0, 0.0]]);
    }
}
