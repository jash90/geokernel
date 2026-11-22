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

//! Numeric algebra commands and their expression forms.

mod base;
mod extrema;
mod integers;
pub(crate) mod polynomial;

use crate::expr::eval::{number_of, Ctx, Val};
use crate::expr::math::{check_integer, is_equal, is_integer, is_zero, java_round};
use crate::expr::parse::{list_form, Parser};
use crate::expr::types::of_numbers;
use crate::expr::{AlgebraOp, Bin, Elem, ListOp, Node, Ty, NONE};

pub(super) const NAMES: &[&str] = &[
    "Division",
    "PrimeFactors",
    "Divisors",
    "DivisorsList",
    "DivisorsSum",
    "IsPrime",
    "IsInteger",
    "ToBase",
    "FromBase",
];

/// The command of two numbers named `name`: a built-in name, like the
/// functions of two arguments (see `names`).
pub(super) fn binary_command(name: &str) -> Option<Bin> {
    Some(match name {
        "Mod" => Bin::Mod,
        "Div" => Bin::IntDiv,
        "GCD" => Bin::Gcd,
        "LCM" => Bin::Lcm,
        "CommonDenominator" => Bin::CommonDenominator,
        "Max" => Bin::Max,
        "Min" => Bin::Min,
        _ => return None,
    })
}

/// Whether this is one of the commands of two numbers.
pub(super) fn is_binary_command(op: Bin) -> bool {
    matches!(
        op,
        Bin::Mod | Bin::IntDiv | Bin::Gcd | Bin::Lcm | Bin::CommonDenominator | Bin::Max | Bin::Min
    )
}

/// The command of two numbers `op` of `a` and `b`.
pub(super) fn binary(op: Bin, a: f64, b: f64) -> f64 {
    match op {
        Bin::Mod => modulo(a, b),
        Bin::IntDiv => div(a, b),
        Bin::Gcd => gcd_of(a, b),
        Bin::Lcm => lcm_of(a, b),
        Bin::CommonDenominator => common_denominator(a, b),
        Bin::Max => max(a, b),
        Bin::Min => min(a, b),
        _ => f64::NAN,
    }
}

/// `Min(list)` and `Max(list)`; of anything else they are the commands of
/// two numbers.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if matches!(text, "ToBase" | "FromBase") {
        let [value, radix] = p.arguments()?[..] else {
            return Err(format!("`{text}` takes two arguments"));
        };
        return Ok(Some(p.push(Node::Base {
            to: text == "ToBase",
            value,
            radix,
        })));
    }
    if matches!(text, "Min" | "Max") {
        let uses_var = p.uses_var;
        if let Some(node) = p.function_command(|p, args, _| {
            let &[f, a, b] = args else { return Ok(None) };
            if !p.uses_function_variable(f) || !p.is_number(f) {
                return Err(format!("`{text}` needs a numeric function"));
            }
            Ok(Some(Node::Extremum(text == "Min", f, a, b)))
        })? {
            p.uses_var = uses_var;
            return Ok(Some(node));
        }
    }
    if matches!(text, "Div" | "Mod") {
        return p.function_command(|p, args, _| {
            let &[a, b] = args else { return Ok(None) };
            Ok((p.uses_function_variable(a) && p.uses_function_variable(b))
                .then_some(Node::PolynomialPart(text == "Mod", a, b)))
        });
    }
    let algebra = match text {
        "Division" => AlgebraOp::Division,
        "PrimeFactors" => AlgebraOp::PrimeFactors,
        "Divisors" => AlgebraOp::Divisors,
        "DivisorsList" => AlgebraOp::DivisorsList,
        "DivisorsSum" => AlgebraOp::DivisorsSum,
        "IsPrime" => AlgebraOp::IsPrime,
        "IsInteger" => AlgebraOp::IsInteger,
        _ => {
            let op = match text {
                "Min" => ListOp::Min,
                "Max" => ListOp::Max,
                "GCD" => ListOp::Gcd,
                "LCM" => ListOp::Lcm,
                _ => return Ok(None),
            };
            return p.list_command(|_, args, lists| {
                let frequencies =
                    matches!(op, ListOp::Min | ListOp::Max) && matches!(lists, [true, true]);
                list_form(text, op, frequencies, args, lists)
            });
        }
    };
    let args = p.arguments()?;
    let node = match (algebra, args.as_slice()) {
        (AlgebraOp::Division, &[a, b]) => Node::Algebra(algebra, a, b),
        (op, &[a]) if op != AlgebraOp::Division => Node::Algebra(op, a, NONE),
        _ => return Err(format!("`{text}` does not take {} argument(s)", args.len())),
    };
    Ok(Some(p.push(node)))
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    _elem: Elem,
    numbers: bool,
    _nodes: &[Node],
    tys: &[Ty],
    _l: u32,
    n: u32,
) -> Result<Option<Ty>, String> {
    Ok(Some(match op {
        // `AlgoListMinMax` reads numbers
        ListOp::Min | ListOp::Max if n != NONE && !matches!(tys[n as usize], Ty::List(_)) => {
            return Err("frequencies must be a list".into());
        }
        ListOp::Min | ListOp::Max | ListOp::Gcd | ListOp::Lcm => of_numbers(numbers)?,
        _ => return Ok(None),
    }))
}

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    _i: u32,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Min | ListOp::Max => Val::N(ctx.min_max(op == ListOp::Min, l, n)),
        ListOp::Gcd | ListOp::Lcm => Val::N(ctx.list_gcd_lcm(op == ListOp::Gcd, l)),
        _ => return None,
    })
}

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

/// The two numeric fractions use GeoGebra's continued-fraction tolerance.
fn common_denominator(a: f64, b: f64) -> f64 {
    use crate::expr::derivative::decimal_to_fraction;
    use crate::tol::STANDARD_PRECISION;

    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    let (_, p) = decimal_to_fraction(a, STANDARD_PRECISION);
    let (_, q) = decimal_to_fraction(b, STANDARD_PRECISION);
    p * q / gcd(magnitude(p), magnitude(q)) as f64
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
    fn min_max(&self, min: bool, l: u32, n: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        let Some((start, len)) = self.list(l) else {
            return f64::NAN;
        };
        if len == 0 {
            return f64::NAN;
        }
        let frequencies = if n == NONE {
            None
        } else {
            let Some((freq, count)) = self.list(n) else {
                return f64::NAN;
            };
            if count != len {
                return f64::NAN;
            }
            Some(freq)
        };
        let mut best = if min {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        let mut used = false;
        if !lists.charge(u64::from(len) * (1 + u64::from(frequencies.is_some()))) {
            return f64::NAN;
        }
        for k in 0..len {
            let Some(v) = number_of(lists.read(self.c, start + k)) else {
                return f64::NAN;
            };
            if let Some(freq) = frequencies {
                let Some(frequency) = number_of(lists.read(self.c, freq + k)) else {
                    return f64::NAN;
                };
                if frequency < 0.0 {
                    return f64::NAN;
                }
                if frequency == 0.0 {
                    continue;
                }
            }
            used = true;
            best = if min {
                self::min(best, v)
            } else {
                max(best, v)
            };
        }
        if !used {
            return f64::NAN;
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
