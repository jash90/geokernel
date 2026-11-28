// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Line and conic equations as text. The implicit conic form is
//! `a x² + b x y + c y² + d x + e y = f`.
//!
//! Numbers are written by a formatter the caller passes, so the window and
//! the command line each keep their own rounding.

use crate::algo::conic_nd::{ConicData, ConicType};
use crate::kernel::EquationForm;
use crate::tol::STANDARD_PRECISION;

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

/// The explicit equation of a line given its raw coefficients `[a, b, c]`.
pub fn explicit_line([a, b, c]: [f64; 3], latex: bool, fmt: &dyn Fn(f64) -> String) -> String {
    let eq = if latex { r"\, = \," } else { " = " };
    if b.abs() < STANDARD_PRECISION {
        return format!("x{eq}{}", fmt(-c / a));
    }

    let slope = -a / b;
    let intercept = -c / b;
    const DISPLAY_ZERO: f64 = 0.005;
    if slope.abs() <= DISPLAY_ZERO {
        return format!("y{eq}{}", fmt(intercept));
    }
    let coefficient = format_coeff(slope, fmt);
    let spacing = if latex { " " } else { "" };
    let mut equation = format!("y{eq}{coefficient}{spacing}x");
    if intercept.abs() > DISPLAY_ZERO {
        equation.push_str(&format!(" {} {}", sign(intercept), fmt(intercept.abs())));
    }
    equation
}

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

/// A conic's selected form when the current coefficients can be written in it.
/// An impossible form falls back to the implicit equation.
pub(crate) fn styled_conic_equation(
    d: &ConicData,
    form: &EquationForm,
    latex: bool,
    fmt: &dyn Fn(f64) -> String,
) -> Option<String> {
    let [a, c, f, b, h, k] = d.matrix;
    let coeffs = [a, 2.0 * b, c, 2.0 * h, 2.0 * k, f];
    let zero = |x: f64| x.abs() < EPS;
    let vertical = zero(b) && zero(c) && !zero(k);
    let horizontal = zero(b) && zero(a) && !zero(h);
    let square = if latex { "^{2}" } else { "²" };
    match form {
        EquationForm::Explicit if vertical => Some(explicit_conic(coeffs, 4, latex, fmt)),
        EquationForm::Vertex if vertical && !zero(a) => {
            let factor = format_coeff(-a / coeffs[4], fmt);
            let x = shifted("x", h / a, fmt);
            let y = h * h / (coeffs[4] * a) - f / coeffs[4];
            let mut equation = format!("y = {factor}{x}{square}");
            if y != 0.0 {
                equation.push_str(&format!(" {}{}", sign(y), fmt(y.abs())));
            }
            Some(equation)
        }
        EquationForm::Explicit | EquationForm::Vertex | EquationForm::Conic
            if (vertical && !zero(a)) || (horizontal && !zero(c)) =>
        {
            let (a, b, c, var1, var2) = if a == 0.0 {
                (c, 2.0 * k, 2.0 * h, "y", "x")
            } else {
                (a, 2.0 * h, 2.0 * k, "x", "y")
            };
            let factor = format_coeff(-c / a, fmt);
            let x = b / (2.0 * a);
            let y = f / c - b * b / (4.0 * a * c);
            Some(format!(
                "{factor}({var2} {} {}) = ({var1} {} {}){square}",
                sign(y),
                fmt(y.abs()),
                sign(x),
                fmt(x.abs())
            ))
        }
        EquationForm::Specific => specific_conic(d, coeffs, latex, fmt),
        EquationForm::Parametric(parameter) => Some(parametric_conic(d, parameter, latex, fmt)),
        _ => None,
    }
}

fn sign(x: f64) -> char {
    if x < 0.0 {
        '-'
    } else {
        '+'
    }
}

fn shifted(var: &str, shift: f64, fmt: &dyn Fn(f64) -> String) -> String {
    if shift == 0.0 {
        var.into()
    } else {
        format!("({var} {} {})", sign(shift), fmt(shift.abs()))
    }
}

fn explicit_conic(
    numbers: [f64; 6],
    pos: usize,
    latex: bool,
    fmt: &dyn Fn(f64) -> String,
) -> String {
    let vars = if latex {
        ["x^{2}", "x y", "y^{2}", "x", "y", ""]
    } else {
        ["x²", "x y", "y²", "x", "y", ""]
    };
    let mut out = format!("{} = ", vars[pos]);
    let zero = fmt(0.0);
    let first = (0..numbers.len()).find(|&i| i != pos && fmt(numbers[i].abs()) != zero);
    let Some(first) = first else {
        out.push('0');
        return out;
    };
    let term = |v, i| {
        if i == 5 {
            fmt(v)
        } else {
            format_coeff(v, fmt)
        }
    };
    let q = -numbers[pos];
    out.push_str(&term(numbers[first] / q, first));
    out.push_str(vars[first]);
    for (i, (&v, var)) in numbers.iter().zip(vars).enumerate().skip(first + 1) {
        let v = v / q;
        if i != pos && fmt(v.abs()) != zero {
            out.push_str(&format!(" {} {}{var}", sign(v), term(v.abs(), i)));
        }
    }
    out
}

fn specific_conic(
    d: &ConicData,
    coeffs: [f64; 6],
    latex: bool,
    fmt: &dyn Fn(f64) -> String,
) -> Option<String> {
    let square = if latex { "^{2}" } else { "²" };
    let [cx, cy] = d.midpoint;
    let x = shifted("x", if cx.abs() < EPS { 0.0 } else { -cx }, fmt);
    let y = shifted("y", if cy.abs() < EPS { 0.0 } else { -cy }, fmt);
    let [a, b] = d.half_axes.map(|axis| fmt(axis * axis));
    let x_first = d.eigenvec[0][1].abs() < EPS;
    Some(match d.ty {
        ConicType::Circle => format!("{x}{square} + {y}{square} = {a}"),
        ConicType::Ellipse if coeffs[1].abs() < EPS => {
            let (a, b) = if x_first { (a, b) } else { (b, a) };
            format!("{x}{square} / {a} + {y}{square} / {b} = 1")
        }
        ConicType::Hyperbola if coeffs[1].abs() < EPS => {
            let (x, y) = if x_first { (x, y) } else { (y, x) };
            format!("{x}{square} / {a} - {y}{square} / {b} = 1")
        }
        ConicType::Parabola if coeffs[0].abs() < EPS || coeffs[2].abs() < EPS => {
            let pos = if coeffs[2].abs() < EPS { 0 } else { 2 };
            explicit_conic(coeffs, pos, latex, fmt)
        }
        ConicType::DoubleLine => format!("({}){square} = 0", line_lhs(d.lines[0], fmt)),
        ConicType::IntersectingLines | ConicType::ParallelLines => {
            let [a, b] = d.lines.map(|line| line_lhs(line, fmt));
            format!("({a}) ({b}) = 0")
        }
        _ => return None,
    })
}

fn line_lhs(numbers: [f64; 3], fmt: &dyn Fn(f64) -> String) -> String {
    let scale = equation_scale(&numbers);
    let [a, b, c] = numbers.map(|v| v / scale);
    let mut out = if a.abs() < EPS {
        format!("{}y", format_coeff(b, fmt))
    } else {
        let mut out = format!("{}x", format_coeff(a, fmt));
        if fmt(b.abs()) != fmt(0.0) {
            out.push_str(&format!(" {}y", format_signed_coefficient(b, fmt)));
        }
        out
    };
    if fmt(c.abs()) != fmt(0.0) {
        out.push_str(&format!(" {} {}", sign(c), fmt(c.abs())));
    }
    out
}

fn parametric_conic(
    d: &ConicData,
    parameter: &str,
    latex: bool,
    fmt: &dyn Fn(f64) -> String,
) -> String {
    let (s1, s2, radii) = match d.ty {
        ConicType::Circle | ConicType::Ellipse => (
            format!("cos({parameter})"),
            format!("sin({parameter})"),
            d.half_axes,
        ),
        ConicType::Hyperbola => (
            format!("cosh({parameter})"),
            format!("sinh({parameter})"),
            d.half_axes,
        ),
        ConicType::Parabola => (
            format!("{parameter}²"),
            parameter.to_owned(),
            [d.linear_eccentricity, 2.0 * d.linear_eccentricity],
        ),
        _ => return parametric_degenerate_conic(d, latex, fmt),
    };
    let sep = if latex { r",\;" } else { ", " };
    let [x, y] = d.midpoint.map(fmt);
    let center = if latex {
        format!(r"\left({x}{sep}{y} \right)")
    } else {
        format!("({x}, {y})")
    };
    let term = |i: usize| {
        let a = d.eigenvec[0][i] * radii[0];
        let b = d.eigenvec[1][i] * radii[1];
        let coefficient = |v| {
            let n = format_coeff(v, fmt);
            if n.is_empty() {
                n
            } else {
                format!("{n} ")
            }
        };
        let zero = fmt(0.0);
        if fmt(a.abs()) == zero {
            if fmt(b.abs()) == zero {
                return zero;
            }
            return format!("{}{s2}", coefficient(b));
        }
        let a = if d.ty == ConicType::Hyperbola {
            let sign = if a < 0.0 { '∓' } else { '±' };
            format!("{sign} {}", coefficient(a.abs()))
        } else {
            coefficient(a)
        };
        if fmt(b.abs()) == zero {
            format!("{a}{s1}")
        } else {
            format!("{a}{s1} {} {}{s2}", sign(b), coefficient(b.abs()))
        }
    };
    format!("X = {center} + ({}{sep}{})", term(0), term(1))
}

fn parametric_degenerate_conic(d: &ConicData, latex: bool, fmt: &dyn Fn(f64) -> String) -> String {
    let [x, y] = d.midpoint.map(fmt);
    let [a, b, _] = d.lines[0];
    let [u, v, _] = d.lines[1];
    let pair = |first: [f64; 2], second: [f64; 2]| {
        let c = [(first[0] + second[0]) / 2.0, (first[1] + second[1]) / 2.0];
        let mut delta = [(second[0] - first[0]) / 2.0, (second[1] - first[1]) / 2.0];
        if delta
            .iter()
            .find(|v| v.abs() >= EPS)
            .is_some_and(|v| *v < 0.0)
        {
            delta = delta.map(|v| -v);
        }
        let coord = |i: usize| {
            let mut out = fmt(c[i]);
            let variation = fmt(delta[i].abs());
            if variation != fmt(0.0) {
                let sign = if delta[i] < 0.0 { '∓' } else { '±' };
                if out == fmt(0.0) {
                    out.clear();
                } else {
                    out.push(' ');
                }
                out.push_str(&format!("{sign} {variation}"));
            }
            out
        };
        format!("({}, {}, 0)", coord(0), coord(1))
    };
    match d.ty {
        ConicType::SinglePoint if latex => format!(r"X = \left({x},\;{y},\;0 \right)"),
        ConicType::SinglePoint => format!("X = ({x}, {y}, 0)"),
        ConicType::IntersectingLines => {
            format!("X = ({x}, {y}, 0) + λ {}", pair([b, -a], [v, -u]))
        }
        ConicType::ParallelLines => {
            let start = |[a, b, c]: [f64; 3]| {
                if a.abs() > b.abs() {
                    [-c / a, 0.0]
                } else {
                    [0.0, -c / b]
                }
            };
            let base = pair(start(d.lines[0]), start(d.lines[1]));
            format!("X = {base} + λ ({}, {}, 0)", fmt(b), fmt(-a))
        }
        ConicType::DoubleLine => format!("X = ({x}, {y}, 0) + λ ({}, {}, 0)", fmt(b), fmt(-a)),
        _ => "X = ?".into(),
    }
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
