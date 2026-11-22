//! Numeric real roots from the exact rational coefficients: isolation and
//! refinement in double-double arithmetic, so a polynomial whose integer
//! coefficients dwarf `f64` precision (the Wilkinson products) still yields
//! every root correct to the last bit a double can hold.

use super::Rat;
use crate::cas::Work;

/// A double-double number `hi + lo` with `|lo| <= ulp(hi) / 2`, carrying
/// about 106 significant bits.
#[derive(Clone, Copy)]
struct Dd(f64, f64);

impl Dd {
    const ZERO: Self = Self(0.0, 0.0);

    fn from_f64(value: f64) -> Self {
        Self(value, 0.0)
    }

    /// The two doubles closest to the exact rational: numerator and
    /// denominator each split exactly, then divided in double-double.
    fn from_rat(value: Rat, work: &mut Work<'_>) -> Option<Self> {
        work.spend(8)?;
        Some(split_integer(value.num).mul(reciprocal(split_integer(value.den))))
    }

    fn is_zero(self) -> bool {
        self.0 == 0.0 && self.1 == 0.0
    }

    fn abs(self) -> f64 {
        self.0.abs() + self.1.abs()
    }

    fn neg(self) -> Self {
        Self(-self.0, -self.1)
    }

    fn sub(self, other: Self) -> Self {
        self.add(other.neg())
    }

    fn add(self, other: Self) -> Self {
        // Knuth's two-sum of the leading doubles, then one renormalisation
        // step absorbing the trailing parts.
        let sum = self.0 + other.0;
        let error = if self.0.abs() >= other.0.abs() {
            (self.0 - sum) + other.0
        } else {
            (other.0 - sum) + self.0
        };
        quick_two_sum(sum, error + self.1 + other.1)
    }

    fn mul(self, other: Self) -> Self {
        let (product, mut error) = two_product(self.0, other.0);
        error += self.0.mul_add(other.1, self.1 * other.0);
        quick_two_sum(product, error)
    }

    fn scale(self, factor: i64) -> Self {
        let (product, error) = two_product(self.0, factor as f64);
        quick_two_sum(product, error + self.1 * factor as f64)
    }
}

/// An integer as the two doubles closest to it, both exact in `i128`.
fn split_integer(value: i128) -> Dd {
    let hi = value as f64;
    Dd(hi, (value - hi as i128) as f64)
}

/// 1 / d by two Newton steps from the leading reciprocal; the first step
/// reaches double-double accuracy, the second one guards its rounding.
fn reciprocal(d: Dd) -> Dd {
    let mut r = Dd::from_f64(1.0 / d.0);
    for _ in 0..2 {
        r = r.mul(Dd(2.0, 0.0).sub(d.mul(r)));
    }
    r
}

fn quick_two_sum(a: f64, b: f64) -> Dd {
    let sum = a + b;
    Dd(sum, b - (sum - a))
}

fn two_product(a: f64, b: f64) -> (f64, f64) {
    let product = a * b;
    // One fused multiply-add recovers the exact rounding error.
    (product, a.mul_add(b, -product))
}

/// Real roots as doubles, correct to the nearest representable value.
pub(super) fn numeric(
    coefficients: &[Rat],
    repeated: bool,
    work: &mut Work<'_>,
) -> Option<Vec<f64>> {
    let exact: Option<Vec<_>> = coefficients
        .iter()
        .map(|&value| Dd::from_rat(value, work))
        .collect();
    isolate(&exact?, repeated, work)
}

/// Isolation for coefficient vectors the caller has already rounded to
/// doubles: the system solver reduces its constraints in floating point.
pub(super) fn isolate_approximate(
    coefficients: &[f64],
    repeated: bool,
    work: &mut Work<'_>,
) -> Option<Vec<f64>> {
    let doubles: Vec<_> = coefficients.iter().map(|&a| Dd::from_f64(a)).collect();
    isolate(&doubles, repeated, work)
}

/// The scaled value and its error envelope at `x`, both as plain doubles.
pub(super) fn scaled_residual(coefficients: &[f64], x: f64) -> (f64, f64) {
    let doubles: Vec<_> = coefficients.iter().map(|&a| Dd::from_f64(a)).collect();
    let (value, envelope) = scaled_value(&doubles, x);
    (value.0 + value.1, envelope)
}

/// Rolle's theorem partitions the real line into monotone intervals using
/// the roots of the derivative. A strict sign change on one such interval,
/// evaluated in double-double precision, encloses exactly one root of the
/// exact polynomial.
fn isolate(coefficients: &[Dd], repeated: bool, work: &mut Work<'_>) -> Option<Vec<f64>> {
    work.spend(coefficients.len())?;
    let Some(degree) = coefficients.iter().rposition(|&a| !a.is_zero()) else {
        return Some(Vec::new());
    };
    let coefficients = &coefficients[..=degree];
    let first = coefficients.iter().position(|&a| !a.is_zero())?;
    if first != 0 {
        let mut roots = isolate(&coefficients[first..], repeated, work)?;
        roots.insert(roots.partition_point(|&root| root < 0.0), 0.0);
        return Some(roots);
    }
    if degree == 0 {
        return Some(Vec::new());
    }
    if degree == 1 {
        let root = -coefficients[0].0 / coefficients[1].0;
        return root.is_finite().then_some(vec![root]);
    }
    // Scale by a power of two, the one sign tests never feel, so that no
    // intermediate of the Horner evaluation exceeds the coefficients: the
    // double-double rounding error shrinks with that magnitude. Without it
    // the Wilkinson products, whose coefficients dwarf their roots' scale,
    // lose the sign exactly where the roots sit.
    let magnitude = coefficients
        .iter()
        .map(|&a| a.abs())
        .fold(0.0_f64, f64::max);
    // The unbiased binary exponent, read from the bits: `log2` rounds the
    // powers of two themselves.
    let shift = ((magnitude.to_bits() >> 52) & 0x7ff) as i32 - 1023;
    let factor = 2.0_f64.powi(-shift);
    let scale = |a: Dd| Dd(a.0 * factor, a.1 * factor);
    let coefficients: Vec<_> = coefficients.iter().map(|&a| scale(a)).collect();
    let coefficients = coefficients.as_slice();
    let derivative: Vec<_> = coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, &a)| a.scale(i as i64))
        .collect();
    let critical = isolate(&derivative, false, work)?;
    // Fujiwara's bound encloses the roots even with a small leading
    // coefficient; the slack absorbs the rounding of the leading doubles.
    // https://doi.org/10.1627/tmj1911.8.78
    let bound = 2.000_001
        * coefficients[..degree]
            .iter()
            .enumerate()
            .map(|(i, &a)| (a.abs() / coefficients[degree].abs()).powf(1.0 / (degree - i) as f64))
            .fold(1.0_f64, f64::max);
    if !bound.is_finite() {
        return None;
    }
    let mut roots = Vec::new();
    let mut left = -bound;
    let mut sign_left = sample(coefficients, left, repeated, work)?;
    for right in critical
        .into_iter()
        .filter(|&x| x.abs() < bound)
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

/// The sign of the exact polynomial at `x`: 1, -1, or 0 when the value is
/// zero to double-double precision (a multiple root of a polynomial that is
/// not square-free).
fn sample(coefficients: &[Dd], x: f64, repeated: bool, work: &mut Work<'_>) -> Option<f64> {
    work.spend(coefficients.len())?;
    let (value, envelope) = scaled_value(coefficients, x);
    if repeated && value.abs() <= 8.0 * f64::EPSILON * envelope {
        return Some(0.0);
    }
    let sign = value.0.signum();
    sign.is_finite().then_some(sign)
}

fn scaled_value(coefficients: &[Dd], x: f64) -> (Dd, f64) {
    // For large |x|, evaluate x^-degree * p(x) at 1/x. This keeps all
    // accumulated values bounded by the magnitude of the coefficients.
    if x.abs() <= 1.0 {
        horner(coefficients.iter().rev().copied(), x)
    } else {
        let (value, envelope) = horner(coefficients.iter().copied(), 1.0 / x);
        let change_sign = x < 0.0 && coefficients.len().is_multiple_of(2);
        (if change_sign { value.neg() } else { value }, envelope)
    }
}

fn horner(coefficients: impl Iterator<Item = Dd>, x: f64) -> (Dd, f64) {
    let (mut sum, mut envelope) = (Dd::ZERO, 0.0_f64);
    let step = Dd::from_f64(x);
    for a in coefficients {
        sum = sum.mul(step).add(a);
        envelope = envelope.mul_add(x.abs(), a.abs());
    }
    (sum, envelope)
}

fn bisect(
    coefficients: &[Dd],
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
