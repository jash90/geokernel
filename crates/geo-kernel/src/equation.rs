// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The equation of a conic as text, in GeoGebra's default form for the
//! conics this kernel makes: the implicit form `a x² + b x y + c y² + d x +
//! e y = f` (`GeoConicND.toValueString` without an equation form, which
//! calls `Kernel.buildImplicitEquation`). Every conic command and every
//! transformation of a conic leaves that form in place.
//!
//! Numbers are written by a formatter the caller passes, so the window and
//! the command line each keep their own rounding.

use crate::algo::conic_nd::ConicData;

/// `GeoConicND.vars`.
const VARS: [&str; 5] = ["x²", "x y", "y²", "x", "y"];

/// Factor cancelled in the display form of an equation.
pub(crate) fn equation_scale(numbers: &[f64]) -> f64 {
    if numbers.iter().all(|&v| is_integer(v)) {
        let g = gcd(numbers);
        if g != 0.0 {
            return g;
        }
    }
    1.0
}

/// `Kernel.STANDARD_PRECISION`, the tolerance of `DoubleUtil.isZero` and
/// `DoubleUtil.isInteger`.
const EPS: f64 = 1e-8;

/// The implicit equation of the conic with flat matrix
/// `[A00, A11, A22, A01, A02, A12]`, numbers written by `fmt`. A
/// coefficient that `fmt` writes as zero is left out, as
/// `Kernel.isZeroFigure` leaves out one that rounds to zero.
pub fn conic_equation(d: &ConicData, fmt: &dyn Fn(f64) -> String) -> String {
    let m = d.matrix;
    // `GeoConicND.setCoeffs`
    let mut numbers = [m[0], 2.0 * m[3], m[1], 2.0 * m[4], 2.0 * m[5], m[2]];
    implicit(&mut numbers, fmt)
}

/// `Kernel.buildImplicitEquation(numbers, vars, cancelDown = true,
/// needsZ = false, tpl, implicit = true)`.
fn implicit(numbers: &mut [f64; 6], fmt: &dyn Fn(f64) -> String) -> String {
    let zero = fmt(0.0);
    let is_zero_figure = |v: f64| fmt(v.abs()) == zero;
    let leading = (0..VARS.len()).find(|&i| numbers[i].abs() >= EPS);
    // Integer coefficients are divided by their greatest common divisor.
    let scale = equation_scale(numbers);
    for v in numbers.iter_mut() {
        *v /= scale;
    }
    let mut s = String::new();
    match leading {
        None => s.push('0'),
        Some(lead) => {
            s.push_str(&format_coeff(numbers[lead], fmt));
            s.push_str(VARS[lead]);
            for i in lead + 1..VARS.len() {
                if !is_zero_figure(numbers[i]) {
                    s.push(' ');
                    s.push_str(&format_signed_coefficient(numbers[i], fmt));
                    s.push_str(VARS[i]);
                }
            }
        }
    }
    s.push_str(" = ");
    // A missing left side leaves the constant out (`buildImplicitVarPart`
    // returns 0 then).
    let constant = if leading.is_some() { numbers[5] } else { 0.0 };
    s.push_str(&fmt(-constant));
    s
}

/// `DoubleUtil.isInteger`.
fn is_integer(x: f64) -> bool {
    if !x.is_finite() {
        return false;
    }
    if !(-1e17..=1e17).contains(&x) {
        return true;
    }
    // `Math.round` rounds halves up.
    let r = (x + 0.5).floor();
    x == r || (x - EPS < r && r < x + EPS)
}

/// `Kernel.gcd(double[])`: every number cast to `long` (towards zero).
fn gcd(numbers: &[f64]) -> f64 {
    fn gcd2(m: i64, n: i64) -> i64 {
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
    let mut g = numbers[0] as i64;
    for &v in numbers {
        g = gcd2(v as i64, g);
    }
    g.wrapping_abs() as f64
}

/// `Kernel.formatCoeff`: `1` and `-1` are written as nothing and `-`.
fn format_coeff(x: f64, fmt: &dyn Fn(f64) -> String) -> String {
    if x.abs() == 1.0 {
        return if x > 0.0 { String::new() } else { "-".into() };
    }
    fmt(x)
}

/// `Kernel.formatSignedCoefficient`: `+ 2`, `- 2`, and `+ ` / `- ` for one.
fn format_signed_coefficient(x: f64, fmt: &dyn Fn(f64) -> String) -> String {
    if x == -1.0 {
        return "- ".into();
    }
    if x == 1.0 {
        return "+ ".into();
    }
    // `Kernel.formatSigned`
    if x >= 0.0 {
        format!("+ {}", fmt(x))
    } else {
        format!("- {}", fmt(-x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_decimals(v: f64) -> String {
        let s = format!("{v:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
        if s == "-0" {
            "0".into()
        } else {
            s
        }
    }

    fn eq(matrix: [f64; 6]) -> String {
        let mut d = ConicData::new();
        d.matrix = matrix;
        conic_equation(&d, &two_decimals)
    }

    #[test]
    fn integer_coefficients_are_divided_by_their_gcd() {
        assert_eq!(eq([1.0, 1.0, -4.0, 0.0, 0.0, 0.0]), "x² + y² = 4");
        assert_eq!(eq([2.0, 2.0, -8.0, 0.0, 0.0, 0.0]), "x² + y² = 4");
        assert_eq!(eq([3.0, -6.0, -9.0, 0.0, 0.0, 0.0]), "x² - 2y² = 3");
        assert_eq!(eq([-1.0, 1.0, 1.0, 0.0, 0.0, 0.0]), "-x² + y² = -1");
    }

    #[test]
    fn mixed_and_linear_terms() {
        // x² + 3 x y - y² + 4x - 6y - 5 = 0
        assert_eq!(
            eq([1.0, -1.0, -5.0, 1.5, 2.0, -3.0]),
            "x² + 3x y - y² + 4x - 6y = 5"
        );
        // A parabola y² = 4x: y² - 4x = 0.
        assert_eq!(eq([0.0, 1.0, 0.0, 0.0, -2.0, 0.0]), "y² - 4x = 0");
    }

    #[test]
    fn decimals_are_kept_and_tiny_terms_dropped() {
        assert_eq!(eq([0.25, 1.0, -1.0, 0.0, 0.0, 0.0]), "0.25x² + y² = 1");
        assert_eq!(
            eq([0.25, 1.0, -1.0, 0.0001, 0.0, 0.0]),
            "0.25x² + y² = 1",
            "a term that rounds to zero is left out"
        );
        assert_eq!(eq([1.0 / 3.0, 1.0, -2.0, 0.0, 0.0, 0.0]), "0.33x² + y² = 2");
    }

    #[test]
    fn no_left_side() {
        assert_eq!(eq([0.0, 0.0, 5.0, 0.0, 0.0, 0.0]), "0 = 0");
        assert_eq!(eq([0.0; 6]), "0 = 0");
    }

    #[test]
    fn integers_follow_the_reference() {
        assert!(is_integer(3.0 + 1e-9));
        assert!(!is_integer(3.1));
        assert!(is_integer(1e18));
        assert!(!is_integer(f64::NAN));
        assert_eq!(gcd(&[4.0, 6.0, -8.0]), 2.0);
        assert_eq!(gcd(&[0.0, 0.0]), 0.0);
    }
}
