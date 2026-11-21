// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTwoNumFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMod.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDiv.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoGCD.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLCM.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMax.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMin.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListMinMax.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The commands of two numbers that an expression may use: `Mod`, `Div`,
//! `GCD`, `LCM`, `Max` and `Min` (`AlgoTwoNumFunction`). An undefined
//! (`NaN`) argument gives an undefined result.

use crate::expr::eval::{number_of, Ctx};
use crate::expr::math::{check_integer, is_equal, is_integer, is_zero, java_round};

/// `MyDouble.LARGEST_INTEGER`, 2^53.
const LARGEST_INTEGER: f64 = 9_007_199_254_740_992.0;
/// `Long.MAX_VALUE` as a `double`.
const LONG_MAX: f64 = i64::MAX as f64;

/// `AlgoMod.computeValue`: the remainder with the sign of `|b|`.
pub(crate) fn modulo(a: f64, b: f64) -> f64 {
    let a = check_integer(a);
    let b_abs = check_integer(b.abs());
    if is_equal(0.0, a % b) {
        return 0.0;
    }
    if a.abs() > LARGEST_INTEGER || b_abs > LARGEST_INTEGER {
        return f64::NAN;
    }
    let m = a % b_abs;
    if m < 0.0 {
        m + b_abs
    } else {
        m
    }
}

/// `AlgoDiv.computeValue`: the integer quotient, rounded towards minus
/// infinity for a positive divisor and towards plus infinity otherwise.
pub(crate) fn div(a: f64, b: f64) -> f64 {
    let (n, d) = (check_integer(a), check_integer(b));
    if n.abs() > LARGEST_INTEGER || d.abs() > LARGEST_INTEGER {
        return f64::NAN;
    }
    let fraction = n / d;
    let integer = java_round(fraction) as f64;
    if is_equal(fraction, integer) {
        integer
    } else if d > 0.0 {
        fraction.floor()
    } else {
        fraction.ceil()
    }
}

/// Whether `a` and `b` fit the `long` range of `AlgoGCD` and `AlgoLCM`.
fn in_long_range(a: f64, b: f64) -> bool {
    [a, b].iter().all(|v| (-LONG_MAX..=LONG_MAX).contains(v))
}

fn gcd(a: u128, b: u128) -> u128 {
    let (mut p, mut q) = (a, b);
    while q != 0 {
        (p, q) = (q, p % q);
    }
    p
}

/// `Math.round` of an integer-valued `a`, as a magnitude.
fn magnitude(a: f64) -> u128 {
    java_round(a).unsigned_abs().into()
}

/// `AlgoGCD.computeValue`: exact for integers, undefined from `10^15` on.
pub(crate) fn gcd_of(a: f64, b: f64) -> f64 {
    if !in_long_range(a, b) || !is_integer(a) || !is_integer(b) {
        return f64::NAN;
    }
    let g = gcd(magnitude(a), magnitude(b)) as f64;
    if g < 1e15 {
        g
    } else {
        f64::NAN
    }
}

/// `AlgoLCM.computeValue`: 0 when either number is zero, exact for
/// integers, undefined from `10^15` on.
pub(crate) fn lcm_of(a: f64, b: f64) -> f64 {
    if !in_long_range(a, b) {
        return f64::NAN;
    }
    if is_zero(a) || is_zero(b) {
        return 0.0;
    }
    if !is_integer(a) || !is_integer(b) {
        return f64::NAN;
    }
    let (i, j) = (magnitude(a), magnitude(b));
    let g = gcd(i, j);
    if g == 0 {
        // ±1e-8 is an integer (0) but not zero; `BigInteger.divide` throws
        return f64::NAN;
    }
    let l = (i / g * j) as f64;
    if l < 1e15 {
        l
    } else {
        f64::NAN
    }
}

/// Java's `Math.max`: `NaN` wins, and `0` is larger than `-0`.
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_negative() {
            b
        } else {
            a
        }
    } else {
        a.max(b)
    }
}

/// Java's `Math.min`: `NaN` wins, and `-0` is smaller than `0`.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_negative() {
            a
        } else {
            b
        }
    } else {
        a.min(b)
    }
}

impl<const M: bool> Ctx<'_, M> {
    /// `AlgoListMinMax`: the least or greatest element; an empty list, or
    /// one with an element that is no number, has none.
    pub(crate) fn min_max(&self, min: bool, l: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        let Some((start, len)) = self.list(l) else {
            return f64::NAN;
        };
        if len == 0 {
            return f64::NAN;
        }
        let mut best = if min {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        if !lists.charge(u64::from(len)) {
            return f64::NAN;
        }
        for k in 0..len {
            let Some(v) = number_of(lists.read(self.c, start + k)) else {
                return f64::NAN;
            };
            // `Math.min` and `Math.max`: NaN wins
            best = if v.is_nan() || best.is_nan() {
                f64::NAN
            } else if min {
                best.min(v)
            } else {
                best.max(v)
            };
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_commands_follow_geogebra() {
        assert_eq!(modulo(-7.0, 3.0), 2.0);
        assert_eq!(modulo(-7.0, -3.0), 2.0);
        assert!(modulo(-7.0, 0.0).is_nan());
        assert_eq!(modulo(0.0, 1e-9), 0.0);
        assert!(modulo(1e16, 3.0).is_nan());
        assert_eq!(div(-7.0, 3.0), -3.0);
        assert_eq!(div(-7.0, -3.0), 3.0);
        assert!(div(0.0, 1e-9).is_nan());
        assert_eq!(gcd_of(-7.0, 0.0), 7.0);
        assert!(gcd_of(7.5, 0.0).is_nan());
        assert!(gcd_of(1e16, 1e-9).is_nan());
        assert_eq!(lcm_of(7.0, -3.0), 21.0);
        assert_eq!(lcm_of(7.5, 0.0), 0.0);
        assert!(lcm_of(1e16, 3.0).is_nan());
        assert!(lcm_of(1e-8, -1e-8).is_nan());
        assert!(max(f64::NAN, 1.0).is_nan());
        assert!(max(-0.0, 0.0).is_sign_positive());
        assert!(min(0.0, -0.0).is_sign_negative());
    }
}
