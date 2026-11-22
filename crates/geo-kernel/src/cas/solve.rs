//! Real roots of bounded rational polynomials.

use std::cmp::Ordering;

use super::{factor, Rat, Work};

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
    let approximate: Vec<_> = coefficients.iter().map(|a| a.to_f64()).collect();
    Some(
        isolate(&approximate, repeated, work)?
            .into_iter()
            .map(Root::Numeric)
            .collect(),
    )
}

/// Rolle's theorem partitions the real line into monotone intervals using
/// the roots of the derivative. A strict sign change on one such interval
/// encloses exactly one root. Square-free input does not need to classify a
/// tiny residual at a critical point as a possible repeated root.
fn isolate(coefficients: &[f64], repeated: bool, work: &mut Work<'_>) -> Option<Vec<f64>> {
    work.spend(coefficients.len())?;
    let Some(degree) = coefficients.iter().rposition(|&a| a != 0.0) else {
        return Some(Vec::new());
    };
    let coefficients = &coefficients[..=degree];
    let first = coefficients.iter().position(|&a| a != 0.0)?;
    if first != 0 {
        let mut roots = isolate(&coefficients[first..], repeated, work)?;
        roots.insert(roots.partition_point(|&root| root < 0.0), 0.0);
        return Some(roots);
    }
    match coefficients {
        [_] => return Some(Vec::new()),
        [b, a] => {
            let root = -b / a;
            return root.is_finite().then_some(vec![root]);
        }
        _ => {}
    }
    let derivative: Vec<_> = coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, a)| i as f64 * a)
        .collect();
    let critical = isolate(&derivative, false, work)?;
    // Fujiwara's bound encloses roots even with a small leading coefficient.
    // https://doi.org/10.1627/tmj1911.8.78
    let bound = 2.0
        * coefficients[..degree]
            .iter()
            .enumerate()
            .map(|(i, a)| {
                (a / coefficients[degree])
                    .abs()
                    .powf(1.0 / (degree - i) as f64)
            })
            .fold(1.0_f64, f64::max);
    if !bound.is_finite() {
        return None;
    }
    let mut roots = Vec::new();
    let mut left = -bound;
    let mut sign_left = sample(coefficients, left, false, work)?;
    for right in critical
        .into_iter()
        .filter(|x| x.abs() < bound)
        .chain([bound])
    {
        let sign_right = sample(coefficients, right, repeated, work)?;
        if sign_right == 0.0 && right != bound {
            roots.push(right);
        } else if sign_left * sign_right < 0.0 {
            roots.push(bisect(coefficients, left, right, sign_left, work)?);
        }
        (left, sign_left) = (right, sign_right);
    }
    Some(roots)
}

fn sample(coefficients: &[f64], x: f64, repeated: bool, work: &mut Work<'_>) -> Option<f64> {
    work.spend(coefficients.len())?;
    let (value, envelope) = scaled_value(coefficients, x);
    if repeated && value.abs() <= 8.0 * f64::EPSILON * envelope || value == 0.0 {
        return Some(0.0);
    }
    value.is_finite().then(|| value.signum())
}

fn scaled_value(coefficients: &[f64], x: f64) -> (f64, f64) {
    // For large |x|, evaluate x^-degree * p(x) at 1/x. This keeps all
    // accumulated values bounded by the magnitude of the coefficients.
    if x.abs() <= 1.0 {
        horner(coefficients.iter().rev().copied(), x)
    } else {
        let (value, envelope) = horner(coefficients.iter().copied(), 1.0 / x);
        let change_sign = x < 0.0 && coefficients.len().is_multiple_of(2);
        (if change_sign { -value } else { value }, envelope)
    }
}

fn horner(coefficients: impl Iterator<Item = f64>, x: f64) -> (f64, f64) {
    let (mut sum, mut error, mut envelope) = (0.0_f64, 0.0_f64, 0.0_f64);
    for a in coefficients {
        // Compensated Horner with error-free transformations for the
        // product and sum (Graillat, Langlois and Louvet, 2005).
        let product = sum * x;
        let next = product + a;
        let recovered = next - product;
        let lost = (product - (next - recovered)) + (a - recovered);
        error = error.mul_add(x, sum.mul_add(x, -product) + lost);
        sum = next;
        envelope = envelope.mul_add(x.abs(), a.abs());
    }
    (sum + error, envelope)
}

fn bisect(
    coefficients: &[f64],
    mut left: f64,
    mut right: f64,
    sign_left: f64,
    work: &mut Work<'_>,
) -> Option<f64> {
    for _ in 0..640 {
        let mid = left.midpoint(right);
        if mid == left || mid == right {
            work.spend(2 * coefficients.len())?;
            let l = scaled_value(coefficients, left).0.abs();
            let r = scaled_value(coefficients, right).0.abs();
            return Some(if l <= r { left } else { right });
        }
        let sign = sample(coefficients, mid, false, work)?;
        if sign == 0.0 {
            return Some(mid);
        }
        if sign == sign_left {
            left = mid;
        } else {
            right = mid;
        }
    }
    None
}
