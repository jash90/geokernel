// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath2.java
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
    a.powf(b)
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
    let r = v.sin();
    if v.abs() > 0.1 && is_zero(r) {
        0.0
    } else {
        r
    }
}

/// `MyDouble.cos`.
pub(crate) fn cos(v: f64) -> f64 {
    let r = v.cos();
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

/// `ln Γ(x)` for `x > 0` by the Lanczos approximation (`g = 7`, nine
/// coefficients). GeoGebra calls a library routine here; see
/// `DIFFERENCES.md`.
fn ln_gamma(x: f64) -> f64 {
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
