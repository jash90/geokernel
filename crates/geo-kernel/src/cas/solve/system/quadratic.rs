//! Exact linear back-substitution in a single real quadratic field.

use super::{degree, Equation, Rat, Root, Work};

#[derive(Clone, Copy)]
struct Element {
    offset: Rat,
    scale: Rat,
}

impl Element {
    const ZERO: Self = Self::rational(Rat::ZERO);

    const fn rational(offset: Rat) -> Self {
        Self {
            offset,
            scale: Rat::ZERO,
        }
    }

    fn add(self, rhs: Self) -> Option<Self> {
        Some(Self {
            offset: self.offset.add(rhs.offset)?,
            scale: self.scale.add(rhs.scale)?,
        })
    }

    fn mul(self, rhs: Self, radicand: Rat) -> Option<Self> {
        Some(Self {
            offset: self
                .offset
                .mul(rhs.offset)?
                .add(self.scale.mul(rhs.scale)?.mul(radicand)?)?,
            scale: self
                .offset
                .mul(rhs.scale)?
                .add(self.scale.mul(rhs.offset)?)?,
        })
    }

    fn divided_by(self, rhs: Self, radicand: Rat) -> Option<Self> {
        let denominator = rhs
            .offset
            .mul(rhs.offset)?
            .sub(rhs.scale.mul(rhs.scale)?.mul(radicand)?)?;
        let conjugate = Self {
            offset: rhs.offset,
            scale: rhs.scale.neg()?,
        };
        let numerator = self.mul(conjugate, radicand)?;
        Some(Self {
            offset: numerator.offset.div(denominator)?,
            scale: numerator.scale.div(denominator)?,
        })
    }
}

pub(super) fn linear_solution(
    equations: &[&Equation],
    variable: usize,
    row: [Root; 3],
    work: &mut Work<'_>,
) -> Option<Root> {
    let mut common = None;
    let mut values = [Element::ZERO; 3];
    for i in variable + 1..3 {
        values[i] = match row[i] {
            Root::Rational(r) => Element::rational(r),
            Root::Radical {
                offset,
                scale,
                radicand,
                degree: 2,
                ..
            } if common.is_none_or(|d| d == radicand) => {
                common = Some(radicand);
                Element { offset, scale }
            }
            _ => return None,
        };
    }
    let radicand = common?;
    for polynomial in equations {
        work.spend(polynomial.len())?;
        if polynomial.keys().any(|power| power[variable] > 1) {
            continue;
        }
        let mut coefficients = [Element::ZERO; 2];
        for (power, &coefficient) in *polynomial {
            work.spend((degree(power) + 1) * 6)?;
            let mut term = Element::rational(coefficient);
            for i in variable + 1..3 {
                for _ in 0..power[i] {
                    term = term.mul(values[i], radicand)?;
                }
            }
            coefficients[usize::from(power[variable])] =
                coefficients[usize::from(power[variable])].add(term)?;
        }
        let [constant, linear] = coefficients;
        if linear.offset == Rat::ZERO && linear.scale == Rat::ZERO {
            continue;
        }
        let root = constant.divided_by(linear, radicand)?;
        let offset = root.offset.neg()?;
        let scale = root.scale.neg()?;
        return Some(if scale == Rat::ZERO {
            Root::Rational(offset)
        } else {
            let sqrt = radicand.to_f64().sqrt();
            let value = scale.to_f64().mul_add(sqrt, offset.to_f64());
            Root::Radical {
                offset,
                scale,
                radicand,
                degree: 2,
                value,
            }
        });
    }
    None
}
