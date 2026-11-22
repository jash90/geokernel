//! Real roots of bounded rational polynomials.

use std::cmp::Ordering;

use super::{factor, Rat, Work};

mod extended;
mod system;
#[cfg(test)]
mod tests;

pub(super) use system::{add_shifted, basis, Equation};
pub(crate) use system::{affine_system, systems, SystemRoots};

const MAX_DEGREE: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Root {
    Rational(Rat),
    Radical {
        offset: Rat,
        scale: Rat,
        radicand: Rat,
        degree: u8,
        value: f64,
    },
    NestedRadical {
        offset: Rat,
        scale: Rat,
        radicand_offset: Rat,
        radicand_scale: Rat,
        radicand: Rat,
        degree: u8,
        value: f64,
    },
    Numeric(f64),
}

impl Root {
    pub(crate) fn value(self) -> f64 {
        match self {
            Self::Rational(r) => r.to_f64(),
            Self::Radical { value, .. }
            | Self::NestedRadical { value, .. }
            | Self::Numeric(value) => value,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum Roots {
    All,
    Finite(Vec<Root>),
}

pub(crate) fn real_roots(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Roots> {
    work.spend(coefficients.len())?;
    let coefficients = trim(coefficients);
    if coefficients.len() > MAX_DEGREE + 1 {
        return None;
    }
    if coefficients.is_empty() {
        return Some(Roots::All);
    }
    let mut roots = if let Some(roots) = elementary(coefficients, work) {
        roots
    } else if let Some((_, factors)) = factor::square_free(coefficients, work) {
        let mut roots = Vec::new();
        for (factor, _) in factors {
            roots.extend(solve_square_free(&factor, work)?);
        }
        roots
    } else {
        numeric(coefficients, true, work)?
    };
    roots.sort_by(|a, b| a.value().total_cmp(&b.value()));
    Some(Roots::Finite(roots))
}

/// The real root the numeric solver answers for the start value `start`
/// (`fsolve(equation, variable = start)`), or `None` where that root is not
/// determined by the equation alone.
///
/// Which root a search from a start value ends on depends on the path of
/// the reference's numeric backend, which no EUPL source describes
/// (golden/cas/start.json records starts a step apart that end on
/// different roots). The answer is therefore given only where it does not
/// depend on the path: the polynomial has exactly one real root, simple, so
/// every successful search ends on it. The root must lie within
/// `GUIDED_ROOT_LIMIT` of 0, where the recorded probes show the reference
/// still answers (golden/cas/magnitude.json), and its value is certified:
/// the polynomial changes sign, beyond its rounding error, across an
/// interval far inside the comparison tolerance.
pub(crate) fn guided_root(coefficients: &[Rat], start: f64, work: &mut Work<'_>) -> Option<f64> {
    if !start.is_finite() {
        return None;
    }
    let root = unique_real_root(coefficients, work)?.value();
    (root.abs() <= GUIDED_ROOT_LIMIT && certified(trim(coefficients), root, work)?).then_some(root)
}

/// The largest root a guided solve answers: the reference answers the
/// recorded roots up to 1000 (`x^3 - 10^9` from 1) and none of those near
/// 10^6 (`x^3 - 10^18`).
const GUIDED_ROOT_LIMIT: f64 = 1e3;

/// Whether `root` is within `1e-10 * max(1, |root|)` of a sign change of the
/// polynomial, with both signs decided beyond the rounding bound of Horner's
/// scheme (`2 n u` times the evaluation of the absolute values; Higham,
/// Accuracy and Stability of Numerical Algorithms, section 5.1).
pub(crate) fn certified(coefficients: &[Rat], root: f64, work: &mut Work<'_>) -> Option<bool> {
    work.spend(2 * coefficients.len())?;
    let values: Vec<f64> = coefficients.iter().map(|c| c.to_f64()).collect();
    let bound = 2.0 * values.len() as f64 * f64::EPSILON;
    let sign = |x: f64| {
        let (value, absolute) = values.iter().rev().fold((0.0_f64, 0.0_f64), |(v, a), &c| {
            (v * x + c, a * x.abs() + c.abs())
        });
        if !value.is_finite() || value.abs() <= bound * absolute * 1.01 {
            0.0
        } else {
            value.signum()
        }
    };
    let delta = 1e-10 * root.abs().max(1.0);
    let (below, above) = (sign(root - delta), sign(root + delta));
    Some(below != 0.0 && above != 0.0 && below != above)
}

/// The only real root of the polynomial, `None` when it has none, several,
/// infinitely many, or when the one root is multiple.
pub(crate) fn unique_real_root(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Root> {
    let exact = trim(coefficients);
    if exact.len() < 2 {
        return None;
    }
    let Roots::Finite(roots) = real_roots(exact, work)? else {
        return None;
    };
    let [root] = roots[..] else {
        return None;
    };
    // A real root of the common factor of the polynomial and its
    // derivative is a multiple root, which can only be the one real root.
    let slope = factor::derivative(exact, work)?;
    let common = factor::gcd(exact, &slope, work)?;
    if common.len() > 1 {
        match real_roots(&common, work)? {
            Roots::Finite(multiple) if multiple.is_empty() => {}
            _ => return None,
        }
    }
    Some(root)
}

fn solve_square_free(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<Root>> {
    if let Some(roots) = elementary(coefficients, work) {
        return Some(roots);
    }
    let mut remaining = coefficients.to_vec();
    let mut roots = Vec::new();
    if let Some(rational) = factor::rational_roots(coefficients, work) {
        for root in rational {
            remaining = factor::divide_exact(&remaining, &[root.neg()?, Rat::ONE], work)?;
            roots.push(Root::Rational(root));
        }
    }
    if let Some(rest) = elementary(&remaining, work) {
        roots.extend(rest);
    } else if remaining.len() == 5 {
        // A quartic with no rational roots can still split into two quadratics.
        match factor::factor(&remaining, work) {
            Some((_, parts)) if parts.len() > 1 => {
                for (part, _) in parts {
                    roots.extend(solve_square_free(&part, work)?);
                }
            }
            _ => roots.extend(numeric(&remaining, false, work)?),
        }
    } else {
        roots.extend(numeric(&remaining, false, work)?);
    }
    Some(roots)
}

fn trim(coefficients: &[Rat]) -> &[Rat] {
    let len = coefficients
        .iter()
        .rposition(|a| a.num != 0)
        .map_or(0, |i| i + 1);
    &coefficients[..len]
}

fn elementary(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<Root>> {
    work.spend(coefficients.len())?;
    match coefficients {
        [] | [_] => Some(vec![]),
        [c, a] => Some(vec![Root::Rational(c.neg()?.div(*a)?)]),
        [c, b, a] => quadratic(*c, *b, *a, work),
        _ => shifted_power_roots(coefficients, work),
    }
}

fn shifted_power_roots(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<Root>> {
    let power = coefficients.len() - 1;
    let leading = coefficients[power];
    let negative_offset = coefficients[power - 1]
        .div(leading)?
        .div(Rat::new(power as i128, 1)?)?;
    let offset = negative_offset.neg()?;
    let mut residual = coefficients.to_vec();
    subtract_shifted_power(&mut residual, power, leading, negative_offset, work)?;
    let inner_degree = trim(&residual).len().saturating_sub(1);
    if inner_degree == 0 {
        let inner_root = Root::Rational(residual[0].neg()?.div(leading)?);
        return power_roots(offset, inner_root, power as u8, work);
    }
    if inner_degree <= 1 || 2 * inner_degree != power {
        return None;
    }
    // Test for a quadratic in (x − offset)^inner_degree.
    let middle = residual[inner_degree];
    subtract_shifted_power(&mut residual, inner_degree, middle, negative_offset, work)?;
    if trim(&residual).len() > 1 {
        return None;
    }
    let mut roots = Vec::new();
    for root in quadratic(residual[0], middle, leading, work)? {
        roots.extend(power_roots(offset, root, inner_degree as u8, work)?);
    }
    Some(roots)
}

fn subtract_shifted_power(
    coefficients: &mut [Rat],
    power: usize,
    leading: Rat,
    negative_offset: Rat,
    work: &mut Work<'_>,
) -> Option<()> {
    work.spend(3 * power + 1)?;
    let mut expected = leading;
    for (i, coefficient) in coefficients[..=power].iter_mut().enumerate().rev() {
        *coefficient = coefficient.sub(expected)?;
        if i > 0 {
            let ratio = Rat::new(i as i128, (power - i + 1) as i128)?;
            expected = expected.mul(negative_offset)?.mul(ratio)?;
        }
    }
    Some(())
}

fn power_roots(offset: Rat, inner: Root, degree: u8, work: &mut Work<'_>) -> Option<Vec<Root>> {
    let inner_value = inner.value();
    if inner_value == 0.0 {
        return Some(vec![Root::Rational(offset)]);
    }
    let negative = inner_value < 0.0;
    let signs: &[_] = if degree.is_multiple_of(2) {
        if negative {
            return Some(vec![]);
        }
        &[-1, 1]
    } else if negative {
        &[-1]
    } else {
        &[1]
    };
    let magnitude = inner_value.abs().powf(1.0 / f64::from(degree));
    signs
        .iter()
        .map(|&sign| {
            let scale = Rat::new(sign, 1)?;
            let value = offset.to_f64() + sign as f64 * magnitude;
            let inner_sign = Rat::new(if negative { -1 } else { 1 }, 1)?;
            match inner {
                Root::Rational(radicand) => radical(
                    offset,
                    scale,
                    radicand.mul(inner_sign)?,
                    degree,
                    value,
                    work,
                ),
                Root::Radical {
                    offset: radicand_offset,
                    scale: radicand_scale,
                    radicand,
                    degree: 2,
                    ..
                } => Some(Root::NestedRadical {
                    offset,
                    scale,
                    radicand_offset: radicand_offset.mul(inner_sign)?,
                    radicand_scale: radicand_scale.mul(inner_sign)?,
                    radicand,
                    degree,
                    value,
                }),
                _ => None,
            }
        })
        .collect()
}

fn quadratic(c: Rat, b: Rat, a: Rat, work: &mut Work<'_>) -> Option<Vec<Root>> {
    if b == Rat::ZERO {
        return power_roots(Rat::ZERO, Root::Rational(c.neg()?.div(a)?), 2, work);
    }
    let four = Rat::new(4, 1)?;
    let two_a = a.add(a)?;
    let discriminant = b.mul(b)?.sub(a.mul(c)?.mul(four)?)?;
    if discriminant.num < 0 {
        return Some(vec![]);
    }
    let center = b.neg()?.div(two_a)?;
    if discriminant.num == 0 {
        return Some(vec![Root::Rational(center)]);
    }
    let scale = Rat::ONE.div(two_a)?;
    // Evaluate the smaller-magnitude root via the product of the roots to
    // avoid subtracting nearly equal floating-point numbers.
    let sqrt = discriminant.to_f64().sqrt();
    let sign = if b.num < 0 { -1.0 } else { 1.0 };
    let q = -0.5 * (b.to_f64() + sign * sqrt);
    let far = q / a.to_f64();
    let near = c.to_f64() / q;
    let (minus, plus) = if sign < 0.0 { (near, far) } else { (far, near) };
    Some(vec![
        radical(center, scale.neg()?, discriminant, 2, minus, work)?,
        radical(center, scale, discriminant, 2, plus, work)?,
    ])
}

fn radical(
    offset: Rat,
    scale: Rat,
    radicand: Rat,
    degree: u8,
    value: f64,
    work: &mut Work<'_>,
) -> Option<Root> {
    let root = |n, work: &mut Work<'_>| integer_root(n, degree, work);
    if let (Some(num), Some(den)) = (root(radicand.num, work)?, root(radicand.den, work)?) {
        let exact = offset.add(scale.mul(Rat::new(num, den)?)?)?;
        Some(Root::Rational(exact))
    } else {
        Some(Root::Radical {
            offset,
            scale,
            radicand,
            degree,
            value,
        })
    }
}

fn integer_root(n: i128, degree: u8, work: &mut Work<'_>) -> Option<Option<i128>> {
    if n <= 1 {
        return Some(Some(n));
    }
    let mut left = 1_i128;
    let bits = i128::BITS - n.leading_zeros();
    let mut right = 1_i128 << bits.div_ceil(u32::from(degree));
    while left <= right {
        work.spend(usize::from(degree))?;
        let mid = left + (right - left) / 2;
        match mid
            .checked_pow(u32::from(degree))
            .map_or(Ordering::Greater, |value| value.cmp(&n))
        {
            Ordering::Less => left = mid + 1,
            Ordering::Greater => right = mid - 1,
            Ordering::Equal => return Some(Some(mid)),
        }
    }
    Some(None)
}

fn numeric(coefficients: &[Rat], repeated: bool, work: &mut Work<'_>) -> Option<Vec<Root>> {
    Some(
        extended::numeric(coefficients, repeated, work)?
            .into_iter()
            .map(Root::Numeric)
            .collect(),
    )
}
