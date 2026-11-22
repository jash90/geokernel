//! Real roots of bounded rational polynomials.

use std::cmp::Ordering;

use super::{factor, Rat, Work};

mod extended;
mod system;
#[cfg(test)]
mod tests;

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

/// Most Newton steps of one guided solve.
const GUIDED_STEPS: usize = 128;
/// Most halvings of one Newton step.
const GUIDED_HALVINGS: usize = 40;

/// One real root reached from `start` by Newton's method with step halving:
/// a step is shortened until the residual shrinks
/// ([Dennis and Schnabel, Numerical Methods for Unconstrained Optimization
/// and Nonlinear Equations, section 6.3](https://doi.org/10.1137/1.9781611971200)).
/// `None` when the derivative vanishes on the way, when no shortened step
/// lowers the residual, or when the budget runs out.
pub(crate) fn guided_root(coefficients: &[Rat], start: f64, work: &mut Work<'_>) -> Option<f64> {
    let coefficients: Vec<f64> = trim(coefficients).iter().map(|c| c.to_f64()).collect();
    if coefficients.len() < 2 || !start.is_finite() {
        return None;
    }
    let evaluate = |x: f64| {
        coefficients
            .iter()
            .rev()
            .fold((0.0, 0.0), |(value, slope): (f64, f64), &c| {
                (value * x + c, slope * x + value)
            })
    };
    let mut x = start;
    let (mut value, mut slope) = evaluate(x);
    for _ in 0..GUIDED_STEPS {
        work.spend(coefficients.len())?;
        if value == 0.0 {
            return Some(x);
        }
        if slope == 0.0 || !slope.is_finite() || !value.is_finite() {
            return None;
        }
        let step = value / slope;
        if step.abs() <= 1e-13 * x.abs().max(1.0) {
            return Some(x - step);
        }
        let mut scale = 1.0;
        let mut accepted = None;
        for _ in 0..=GUIDED_HALVINGS {
            work.spend(coefficients.len())?;
            let candidate = x - scale * step;
            let (v, s) = evaluate(candidate);
            if v.abs() < value.abs() {
                accepted = Some((candidate, v, s));
                break;
            }
            scale *= 0.5;
        }
        (x, value, slope) = accepted?;
    }
    None
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
