// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath2.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoNpR.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The numeric functions of the expression language, with GeoGebra's
//! special cases (`MyDouble`, `MyMath`, `MyMath2`, `DoubleUtil`).
//!
//! Java's `Math` and Rust's `f64` agree on the IEEE operations; where they
//! differ (`Math.pow`, `Math.round`) the Java behaviour is written out here.

use crate::tol::STANDARD_PRECISION;
use std::f64::consts::{E, PI};
use std::fmt::Write;

/// `Kernel.PI_180`: one degree in radians.
pub(crate) const PI_180: f64 = PI / 180.0;
/// `Kernel.CONST_180_PI`: one radian in degrees.
const CONST_180_PI: f64 = 180.0 / PI;
/// `Kernel.MAX_DOUBLE_PRECISION`.
const MAX_DOUBLE_PRECISION: f64 = 1e-15;
/// `MyMath.ONE_THIRD`.
const ONE_THIRD: f64 = 1.0 / 3.0;

/// `DoubleUtil.isZero`.
pub(crate) fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// `DoubleUtil.isEqual(a, b)`.
pub(crate) fn is_equal(a: f64, b: f64) -> bool {
    a == b || ((a - STANDARD_PRECISION) <= b && b <= (a + STANDARD_PRECISION))
}

/// `DoubleUtil.isGreater`.
pub(crate) fn is_greater(a: f64, b: f64) -> bool {
    a > b + STANDARD_PRECISION
}

/// `DoubleUtil.isGreaterEqual`.
pub(crate) fn is_greater_equal(a: f64, b: f64) -> bool {
    a + STANDARD_PRECISION > b
}

/// `DoubleUtil.isEqual(x, y, eps)`.
pub(crate) fn is_equal_eps(x: f64, y: f64, eps: f64) -> bool {
    x == y || ((x - eps) < y && y < (x + eps))
}

/// Java's `Math.round`: `floor(x + 1/2)` computed exactly, saturating to a
/// `long`; `NaN` gives 0.
pub(crate) fn java_round(x: f64) -> i64 {
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    r as i64
}

/// `DoubleUtil.isInteger`.
pub(crate) fn is_integer(x: f64) -> bool {
    if !x.is_finite() {
        return false;
    }
    if !(-1e17..=1e17).contains(&x) {
        return true;
    }
    is_equal(x, java_round(x) as f64)
}

/// `DoubleUtil.checkInteger`: snaps to the nearest integer within the
/// standard precision.
pub(crate) fn check_integer(x: f64) -> f64 {
    let r = java_round(x) as f64;
    if (x - r).abs() < STANDARD_PRECISION {
        r
    } else {
        x
    }
}

/// Java's `Math.pow`: unlike `powf`, a `NaN` exponent always gives `NaN`,
/// and so does `(±1)^(±∞)`.
pub(crate) fn java_pow(a: f64, b: f64) -> f64 {
    if b.is_nan() || (a.abs() == 1.0 && b.is_infinite()) {
        return f64::NAN;
    }
    // Use the portable fdlibm algorithm behind Java's StrictMath.pow.
    // Native pow can differ by one ulp, which changes rounded divisor sums.
    libm::pow(a, b)
}

/// `MyDouble.pow(double, double)`: `∞^0` and `1^∞` are undefined.
pub(crate) fn my_pow(a: f64, b: f64) -> f64 {
    if is_zero(b) && (a.is_infinite() || a.is_nan()) {
        return f64::NAN;
    }
    if a == 1.0 && b.is_infinite() {
        return f64::NAN;
    }
    java_pow(a, b)
}

/// `Operation.NROOT` and `MyDouble.powDoubleSgnChange`.
pub(crate) fn nth_root(base: f64, degree: f64) -> f64 {
    let exponent = 1.0 / degree;
    if base < 0.0 && is_integer(degree) && java_round(degree) % 2 == 1 {
        -my_pow(-base, exponent)
    } else {
        my_pow(base, exponent)
    }
}

/// `MyDouble.handlePower` for a number base `Math.E`: `exp` instead of `pow`.
pub(crate) fn is_euler(base: f64) -> bool {
    base == E
}

/// `Kernel.gcd(long, long)` with Java's wrapping arithmetic.
fn gcd(m: i64, n: i64) -> i64 {
    if m == 0 || n == 0 {
        return m.wrapping_abs().max(n.wrapping_abs());
    }
    let (mut p, mut q) = (m, n);
    while p.wrapping_rem(q) != 0 {
        let r = p.wrapping_rem(q);
        p = q;
        q = r;
    }
    q
}

/// `ExpressionNodeEvaluator.negPower` once the numerator `a` and the
/// denominator `b` of the exponent are known to be integers:
/// `x^(a/b) = (x^a)^(1/b)`, a real root for odd `b`.
pub(crate) fn neg_power_integers(base0: f64, a: f64, b: f64) -> Option<f64> {
    let al = java_round(a);
    if !is_equal(a, al as f64) {
        return None;
    }
    let bl = java_round(b);
    if b == 0.0 {
        return Some(f64::NAN);
    }
    if !is_equal(b, bl as f64) {
        return None;
    }
    let g = gcd(al, bl);
    if g == 0 {
        return Some(f64::NAN);
    }
    let al = al.wrapping_div(g);
    let bl = bl.wrapping_div(g);
    let mut base = base0;
    if al != 1 {
        base = java_pow(base, al as f64);
    }
    if base > 0.0 {
        return Some(java_pow(base, 1.0 / bl as f64));
    }
    if bl.wrapping_abs().wrapping_rem(2) == 1 {
        return Some(-java_pow(-base, 1.0 / bl as f64));
    }
    Some(f64::NAN)
}

/// `MyDouble.sin`: zero is snapped for arguments away from zero.
pub(crate) fn sin(v: f64) -> f64 {
    let r = libm::sin(v);
    if v.abs() > 0.1 && is_zero(r) {
        0.0
    } else {
        r
    }
}

/// `MyDouble.cos`.
pub(crate) fn cos(v: f64) -> f64 {
    let r = libm::cos(v);
    if is_zero(r) {
        0.0
    } else {
        r
    }
}

/// `MyDouble.tan`: undefined at odd multiples of `π/2`.
pub(crate) fn tan(v: f64) -> f64 {
    if is_equal(v.abs() % PI, PI / 2.0) {
        return f64::NAN;
    }
    let r = v.tan();
    if v.abs() > 0.1 && is_zero(r) {
        0.0
    } else {
        r
    }
}

/// `MyMath.csc`.
pub(crate) fn csc(a: f64) -> f64 {
    let s = a.sin();
    if is_zero(s) {
        f64::NAN
    } else {
        1.0 / s
    }
}

/// `MyMath.sec`.
pub(crate) fn sec(a: f64) -> f64 {
    let c = a.cos();
    if is_zero(c) {
        f64::NAN
    } else {
        1.0 / c
    }
}

/// `MyMath.cot`.
pub(crate) fn cot(a: f64) -> f64 {
    let s = a.sin();
    if is_zero(s) {
        f64::NAN
    } else {
        a.cos() / s
    }
}

/// `MyMath.csch`.
pub(crate) fn csch(a: f64) -> f64 {
    if a == 0.0 {
        f64::NAN
    } else {
        1.0 / a.sinh()
    }
}

/// `MyMath.asin`: arguments just outside `[-1, 1]` are clamped.
pub(crate) fn asin(a: f64) -> f64 {
    if a > 1.0 && is_equal_eps(a, 1.0, MAX_DOUBLE_PRECISION) {
        return 1f64.asin();
    }
    if a < -1.0 && is_equal_eps(a, -1.0, MAX_DOUBLE_PRECISION) {
        return (-1f64).asin();
    }
    a.asin()
}

/// `MyMath.acos`.
pub(crate) fn acos(a: f64) -> f64 {
    if a > 1.0 && is_equal_eps(a, 1.0, MAX_DOUBLE_PRECISION) {
        return 1f64.acos();
    }
    if a < -1.0 && is_equal_eps(a, -1.0, MAX_DOUBLE_PRECISION) {
        return (-1f64).acos();
    }
    a.acos()
}

/// `MyMath.cbrt`: `cbrt(0)` is `-0`.
pub(crate) fn cbrt(a: f64) -> f64 {
    if a > 0.0 {
        java_pow(a, ONE_THIRD)
    } else {
        -java_pow(-a, ONE_THIRD)
    }
}

/// `MyMath.sgn`: zero within the standard precision.
pub(crate) fn sgn(a: f64) -> f64 {
    if a.is_nan() {
        f64::NAN
    } else if is_zero(a) {
        0.0
    } else if a > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `MyDouble.log10`: `ln(x) / ln(10)`.
pub(crate) fn log10(a: f64) -> f64 {
    a.ln() / 10f64.ln()
}

/// `MyDouble.log2`: `ln(x) / ln(2)`.
pub(crate) fn log2(a: f64) -> f64 {
    a.ln() / 2f64.ln()
}

/// `MyDouble.floor`; `deg` for an angle, rounded in whole degrees.
pub(crate) fn floor(v: f64, deg: bool) -> f64 {
    if deg {
        PI_180 * check_integer(v * CONST_180_PI).floor()
    } else {
        check_integer(v).floor()
    }
}

/// `MyDouble.ceil`.
pub(crate) fn ceil(v: f64, deg: bool) -> f64 {
    if deg {
        PI_180 * check_integer(v * CONST_180_PI).ceil()
    } else {
        check_integer(v).ceil()
    }
}

/// `MyDouble.round(digits)`: `NaN` unless `digits` is an integer.
pub(crate) fn round(v: f64, digits: f64, deg: bool) -> f64 {
    let v = if is_integer(digits) { v } else { f64::NAN };
    // Java's `(int)` cast saturates and maps NaN to 0, as `as` does.
    let digits = digits as i32;
    if deg {
        PI_180 * round_decimal(v * CONST_180_PI, digits)
    } else {
        round_decimal(v, digits)
    }
}

/// Rounds half up to `scale` decimal places, working on the shortest decimal
/// form of `x` (what `Double.toString` prints), so that `2.675` rounds to
/// `2.68`. A negative number rounded to zero gives `-0`. This is the
/// behaviour `MyDouble.doRound` gets from its rounding helper.
pub(crate) fn round_decimal(x: f64, scale: i32) -> f64 {
    if !x.is_finite() {
        return if x.is_infinite() { x } else { f64::NAN };
    }
    // the shortest form, on the stack: at most 17 digits and a short exponent
    let mut text = Stack::default();
    if write!(text, "{:e}", x.abs()).is_err() {
        return x;
    }
    let Some((mantissa, exp)) = text.as_str().split_once('e') else {
        return x;
    };
    let Ok(exp) = exp.parse::<i64>() else {
        return x;
    };
    let mut digits = [0u8; 24];
    let mut len = 0;
    for b in mantissa.bytes().filter(u8::is_ascii_digit) {
        if len == digits.len() {
            return x;
        }
        digits[len] = b - b'0';
        len += 1;
    }
    // x = 0.d1 d2 d3 ... * 10^point
    let point = exp + 1;
    let keep = point + i64::from(scale);
    if keep >= len as i64 {
        return x;
    }
    let magnitude = if keep < 0 {
        0.0
    } else {
        let keep = keep as usize;
        let mut n: u64 = digits[..keep].iter().fold(0, |n, &d| n * 10 + u64::from(d));
        if digits[keep] >= 5 {
            n += 1;
        }
        super::decimal::to_f64_parts(u128::from(n), point - keep as i64)
    };
    let rounded = if x < 0.0 { -magnitude } else { magnitude };
    if rounded == 0.0 {
        0.0 * x
    } else {
        rounded
    }
}

/// A short text written on the stack.
#[derive(Default)]
struct Stack {
    buf: [u8; 32],
    len: usize,
}

impl Stack {
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl std::fmt::Write for Stack {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.len + s.len();
        if end > self.buf.len() {
            return Err(std::fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// The most factors a combinatorial product multiplies; beyond it the
/// log-gamma value stands (upstream would multiply every factor without a
/// bound; see `DIFFERENCES.md`).
const COMBINATORIAL_FACTOR_LIMIT: u64 = 100_000;

/// `MyMath.binomial(n, k)`: the binomial coefficient, exact through the
/// log-gamma path for small `n` and through an exact wide product for the
/// larger ones, `0` wherever the arguments do not make a count.
pub(crate) fn binomial(n0: f64, k: f64) -> f64 {
    if n0 == 0.0 && k == 0.0 {
        return 1.0;
    }
    let r = if k > n0 / 2.0 { n0 - k } else { k };
    if n0 < 1.0 || r < 0.0 || n0 < r {
        return 0.0;
    }
    if !is_equal(java_round(n0) as f64, n0) || !is_equal(java_round(r) as f64, r) {
        return 0.0;
    }
    let (n, r) = (java_round(n0), java_round(r));
    // `binomLog`: exact for n <= 37.
    let ncr = (0.5
        + (ln_gamma(n as f64 + 1.0) - ln_gamma(r as f64 + 1.0) - ln_gamma((n - r) as f64 + 1.0))
            .exp())
    .floor();
    if ncr == f64::INFINITY {
        return f64::INFINITY;
    }
    if n <= 37 {
        return ncr;
    }
    // `binomBig`: every intermediate value is an exact integer that divides
    // exactly, so a 128-bit product is the exact answer whenever it fits.
    // A count of factors past the work bound keeps the log-gamma value
    // (upstream multiplies without a bound; see `DIFFERENCES.md`).
    if r as u64 > COMBINATORIAL_FACTOR_LIMIT {
        return ncr;
    }
    let mut acc: u128 = 1;
    let mut dd: u128 = 1;
    let mut nn = n;
    while dd <= r as u128 && nn > 0 {
        acc = match acc.checked_mul(nn as u128) {
            Some(v) => v,
            None => return wide_product_binomial(n as f64, r as f64),
        };
        acc /= dd;
        nn -= 1;
        dd += 1;
    }
    acc as f64
}

/// The `binomBig` fallback beyond 128 bits: a plain product, accurate far
/// within the comparison tolerance.
fn wide_product_binomial(n: f64, r: f64) -> f64 {
    let mut acc = 1.0f64;
    let mut dd = 1.0f64;
    let mut nn = n;
    while dd <= r.min(COMBINATORIAL_FACTOR_LIMIT as f64) {
        acc *= nn / dd;
        nn -= 1.0;
        dd += 1.0;
    }
    acc
}

/// `AlgoNpR.nPr(n, r)`: the number of `r`-element permutations, exact
/// through the log-gamma path for small `n` and through an exact wide
/// product for the larger ones, `0` wherever the arguments do not make a
/// count.
pub(crate) fn n_pr(n: f64, r: f64) -> f64 {
    if n == 0.0 && r == 0.0 {
        return 1.0;
    }
    if n < 1.0 || r < 0.0 || n < r {
        return 0.0;
    }
    // An exact integer test, as `AlgoNpR` writes it (`Math.floor(n) != n`).
    if n.floor() != n || r.floor() != r {
        return 0.0;
    }
    if n.is_infinite() {
        return f64::INFINITY;
    }
    let npr = (0.5 + (ln_gamma(n + 1.0) - ln_gamma(n - r + 1.0)).exp()).floor();
    if npr == f64::INFINITY {
        return f64::INFINITY;
    }
    if n <= 16.0 {
        return npr;
    }
    // `nPrBig` walks the integers from the decimal of `n - r` plus one to
    // the decimal of `n`; the decimals come from the doubles, so once `r`
    // is below the spacing of `n` the two are the same double and the
    // product is empty (`nPr(1e40, 2)` is 1).
    let from = n - r;
    if from == n {
        return 1.0;
    }
    let factors = n - from;
    // Bound the range before converting to integers. Even the shortest
    // nonempty range above 2^128 exceeds the factor limit.
    if factors > COMBINATORIAL_FACTOR_LIMIT as f64 {
        return npr;
    }
    super::eval::work(factors as u64);
    let mut acc: u128 = 1;
    let top = n as u128;
    for factor in from as u128 + 1..=top {
        acc = match acc.checked_mul(factor) {
            Some(v) => v,
            // Keep advancing the integer factors after the accumulator
            // overflows. Adding 1.0 may not advance a factor above 2^53.
            None => return (factor..=top).fold(acc as f64, |product, k| product * k as f64),
        };
    }
    acc as f64
}

/// The number of terms in `AlgoFoldExpression`. The first term is always
/// included. The remaining indices start after the truncated lower bound.
pub(crate) fn fold_count(from: f64, to: f64) -> f64 {
    // An upper endpoint at least as large as i32::MAX would wrap the
    // reference's loop counter forever.
    if !from.is_finite() || !to.is_finite() || to >= f64::from(i32::MAX) {
        return f64::NAN;
    }
    let start = f64::from((from as i32).wrapping_add(1));
    1.0 + (to.floor() - start + 1.0).max(0.0)
}

/// `MyMath2.factorial`: a table product for small integers, `Γ(x + 1)`
/// otherwise, undefined below 0 and above 170.624.
pub(crate) fn factorial(x: f64) -> f64 {
    if !(0.0..=170.624).contains(&x) {
        return f64::NAN;
    }
    if x > 32.0 || x - x.floor() > 1e-10 {
        return ln_gamma(x + 1.0).exp();
    }
    let n = x as u32;
    (1..=n).fold(1.0, |p, k| p * f64::from(k))
}

/// `MyMath2.gamma`: undefined at the non-positive integers, by reflection
/// below zero.
pub(crate) fn gamma(x: f64) -> f64 {
    if x <= 0.0 && is_equal(x, java_round(x) as f64) {
        return f64::NAN;
    }
    if x >= 0.0 {
        return ln_gamma(x).exp();
    }
    -PI / (x * ln_gamma(-x).exp() * (PI * x).sin())
}

/// `MyMath2.psi` (`Gamma.digamma` of GeoGebra's commons-math build): the
/// digamma function, by recursion up to 49 and an asymptotic series past
/// it. The recursion folds from 49 downwards, as evaluating the recursive
/// formula does; far below zero, where GeoGebra would not stop either, the
/// result is `NaN`.
pub(crate) fn digamma(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return x;
    }
    if x > 0.0 && x <= 1e-5 {
        return -std::f64::consts::EULER_GAMMA - 1.0 / x;
    }
    if x >= 49.0 {
        let inv = 1.0 / (x * x);
        return x.ln()
            - 0.5 / x
            - inv * (0.083_333_333_333_333_33 + inv * (0.008_333_333_333_333_333 - inv / 252.0));
    }
    if x < -1e6 {
        return f64::NAN;
    }
    let steps = (49.0 - x).ceil() as u32;
    let mut value = digamma((x + f64::from(steps)).max(49.0));
    for step in (0..steps).rev() {
        value -= 1.0 / (x + f64::from(step));
    }
    value
}

/// `ln Γ(x)` for `x > 0` by the Lanczos approximation (`g = 7`, nine
/// coefficients). GeoGebra calls a library routine here; see
/// `DIFFERENCES.md`.
pub(crate) fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const P: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x.is_nan() || x <= 0.0 {
        return f64::NAN;
    }
    if x < 0.5 {
        // Γ(x) = Γ(x + 1) / x
        return ln_gamma(x + 1.0) - x.ln();
    }
    let x = x - 1.0;
    let mut a = P[0];
    for (i, p) in P.iter().enumerate().skip(1) {
        a += p / (x + i as f64);
    }
    let t = x + G + 0.5;
    0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binomial_counts_subsets() {
        // Pascal's recurrence counts subsets independently of the
        // implementation's factorial ratios.
        let choose = |n: usize, k: usize| {
            let mut row = vec![0.0; k + 1];
            row[0] = 1.0;
            for i in 1..=n {
                for j in (1..=i.min(k)).rev() {
                    row[j] += row[j - 1];
                }
            }
            row[k]
        };
        assert_eq!(binomial(5.0, -1.0), 0.0);
        assert_eq!(binomial(5.0, 1.0), 5.0);
        assert_eq!(binomial(5.0, 0.0), 1.0);
        assert_eq!(binomial(0.0, 0.0), 1.0);
        assert_eq!(binomial(10.0, 3.0), choose(10, 3));
        // `r` folds to the smaller side.
        assert_eq!(binomial(10.0, 7.0), choose(10, 3));
        // a non-integer count is 0, a near-integer one is not.
        assert_eq!(binomial(5.5, 2.0), 0.0);
        assert_eq!(binomial(5.000000001, 2.0), choose(5, 2));
        assert_eq!(binomial(-3.0, 2.0), 0.0);
        // past the log-gamma path the exact product runs.
        assert_eq!(binomial(38.0, 19.0), choose(38, 19));
        assert_eq!(binomial(52.0, 5.0), choose(52, 5));
        // huge values overflow the exact product but stay within the
        // comparison tolerance as a plain product.
        assert!((binomial(200.0, 100.0) / choose(200, 100) - 1.0).abs() < 1e-9);
        let n = 10_000_000_u64;
        assert_eq!(binomial(n as f64, 2.0), (n * (n - 1) / 2) as f64);
        let n = 1_000_000_u64;
        assert_eq!(binomial(n as f64, 3.0), (n * (n - 1) * (n - 2) / 6) as f64);
    }

    #[test]
    fn n_pr_counts_ordered_selections() {
        assert_eq!(n_pr(8.0, 7.0), (2..=8).product::<u64>() as f64);
        assert_eq!(n_pr(0.0, 0.0), 1.0);
        assert_eq!(n_pr(5.0, 0.0), 1.0);
        assert_eq!(n_pr(5.0, -1.0), 0.0);
        assert_eq!(n_pr(4.0, 5.0), 0.0);
        assert_eq!(n_pr(-3.0, 2.0), 0.0);
        assert_eq!(n_pr(5.000000001, 2.0), 0.0);
        assert_eq!(n_pr(5.5, 2.0), 0.0);
        assert_eq!(n_pr(17.0, 16.0), (2..=17).product::<u64>() as f64);
        assert_eq!(n_pr(20.0, 3.0), 20.0 * 19.0 * 18.0);
        // 1000P999 is 1000! , far beyond the double range: the log-gamma
        // path overflows first.
        assert_eq!(n_pr(1000.0, 999.0), f64::INFINITY);
        let n = 1_000_000_u64;
        assert_eq!(n_pr(n as f64, 3.0), (n * (n - 1) * (n - 2)) as f64);
    }

    #[test]
    fn java_round_is_floor_of_x_plus_half() {
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(0.499_999_999_999_999_94), 0);
        assert_eq!(java_round(f64::NAN), 0);
        assert_eq!(java_round(1e300), i64::MAX);
    }

    #[test]
    fn decimal_rounding_works_on_the_printed_digits() {
        assert_eq!(round_decimal(2.675, 2), 2.68);
        assert_eq!(round_decimal(1.005, 2), 1.01);
        assert_eq!(round_decimal(-2.5, 0), -3.0);
        assert_eq!(round_decimal(-0.5, 0), -1.0);
        assert_eq!(round_decimal(2.675, -1), 0.0);
        assert!(round_decimal(-2.675, -1).is_sign_negative());
        assert_eq!(round_decimal(9.99, 1), 10.0);
        assert_eq!(round_decimal(123.0, -2), 100.0);
        assert_eq!(round_decimal(1e-5, 3), 0.0);
        assert_eq!(round_decimal(0.1, 400), 0.1);
    }

    #[test]
    fn factorial_and_gamma() {
        assert_eq!(factorial(5.0), 120.0);
        assert_eq!(factorial(3.000_000_000_01), 6.0);
        assert!((factorial(3.5) - 11.631_728_396_567_446).abs() < 1e-12);
        assert!((factorial(40.0) / 8.159_152_832_479_056e47 - 1.0).abs() < 1e-12);
        assert!(factorial(-1.0).is_nan());
        assert!(factorial(171.0).is_nan());
        assert!((gamma(0.5) - PI.sqrt()).abs() < 1e-14);
        assert!((gamma(-0.5) + 2.0 * PI.sqrt()).abs() < 1e-13);
        assert!(gamma(-2.0).is_nan());
    }

    #[test]
    fn powers_follow_java() {
        assert!(java_pow(1.0, f64::NAN).is_nan());
        assert!(java_pow(-1.0, f64::INFINITY).is_nan());
        assert_eq!(java_pow(f64::NAN, 0.0), 1.0);
        assert!(my_pow(f64::INFINITY, 0.0).is_nan());
        assert_eq!(neg_power_integers(-8.0, 1.0, 3.0), Some(-2.0));
        assert_eq!(neg_power_integers(-8.0, -1.0, 3.0), Some(-0.5));
        assert!(neg_power_integers(-8.0, 1.0, 2.0).unwrap().is_nan());
        assert_eq!(neg_power_integers(-8.0, 0.5, 3.0), None);
        assert_eq!(gcd(i64::MIN, -1), -1);
    }
}
