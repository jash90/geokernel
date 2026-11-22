use super::{factor, Rat, Work};
use std::collections::{btree_map::Entry, BTreeMap};

type Monomial = [u8; 3];
const CONSTANT: Monomial = [0; 3];
const MAX_DEGREE: u8 = 64;
const MAX_TERMS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Poly(pub(crate) BTreeMap<Monomial, Rat>);

impl Poly {
    pub(crate) fn constant(value: Rat) -> Self {
        if value == Rat::ZERO {
            Self(BTreeMap::new())
        } else {
            Self(BTreeMap::from([(CONSTANT, value)]))
        }
    }

    pub(crate) fn variable(index: usize) -> Self {
        let mut exponents = CONSTANT;
        exponents[index] = 1;
        Self(BTreeMap::from([(exponents, Rat::ONE)]))
    }

    pub(crate) fn dense(coefficients: &[Rat]) -> Self {
        Self(
            coefficients
                .iter()
                .enumerate()
                .filter(|(_, value)| **value != Rat::ZERO)
                .map(|(i, &value)| ([i as u8, 0, 0], value))
                .collect(),
        )
    }

    pub(crate) fn coefficients(&self) -> Option<Vec<Rat>> {
        if self.0.keys().any(|p| p[1] != 0 || p[2] != 0) {
            return None;
        }
        let Some((&[degree, ..], _)) = self.0.last_key_value() else {
            return Some(Vec::new());
        };
        let mut result = vec![Rat::ZERO; usize::from(degree) + 1];
        for (&[power, ..], &value) in &self.0 {
            result[usize::from(power)] = value;
        }
        Some(result)
    }

    pub(crate) fn scalar(&self) -> Option<Rat> {
        match self.0.first_key_value() {
            None => Some(Rat::ZERO),
            Some((&CONSTANT, &value)) if self.0.len() == 1 => Some(value),
            _ => None,
        }
    }

    fn accumulate(&mut self, key: Monomial, coefficient: Rat) -> Option<()> {
        match self.0.entry(key) {
            Entry::Vacant(entry) => {
                if coefficient != Rat::ZERO {
                    entry.insert(coefficient);
                }
            }
            Entry::Occupied(mut entry) => {
                let sum = entry.get().add(coefficient)?;
                if sum == Rat::ZERO {
                    entry.remove();
                } else {
                    *entry.get_mut() = sum;
                }
            }
        }
        (self.0.len() <= MAX_TERMS).then_some(())
    }

    fn sum(mut self, rhs: Self, sign: Rat, work: &mut Work<'_>) -> Option<Self> {
        work.spend(self.0.len() + rhs.0.len())?;
        for (powers, coefficient) in rhs.0 {
            self.accumulate(powers, coefficient.mul(sign)?)?;
        }
        Some(self)
    }

    pub(super) fn mul(&self, rhs: &Self, work: &mut Work<'_>) -> Option<Self> {
        work.spend(self.0.len() * rhs.0.len())?;
        let mut result = Self::constant(Rat::ZERO);
        for (&[x, y, z], &a) in &self.0 {
            for (&[i, j, k], &b) in &rhs.0 {
                let degree = [x + i, y + j, z + k];
                if degree.iter().copied().map(u16::from).sum::<u16>() > u16::from(MAX_DEGREE) {
                    return None;
                }
                result.accumulate(degree, a.mul(b)?)?;
            }
        }
        Some(result)
    }

    pub(crate) fn scale(self, value: Rat, work: &mut Work<'_>) -> Option<Self> {
        self.mul(&Self::constant(value), work)
    }

    pub(crate) fn common_multiple(&self, rhs: &Self, work: &mut Work<'_>) -> Option<Self> {
        let left = self.coefficients()?;
        let right = rhs.coefficients()?;
        let gcd = factor::gcd(&left, &right, work)?;
        let lhs = Self::dense(&factor::divide_exact(&left, &gcd, work)?);
        let product = lhs.mul(rhs, work)?;
        let (_, primitive) = product.primitive(work)?;
        let a = self.content()?;
        let b = rhs.content()?;
        primitive.scale(Rat::new(lcm(a, b)?, 1)?, work)
    }

    fn content(&self) -> Option<i128> {
        let mut content = 0;
        for value in self.0.values() {
            if value.den != 1 {
                return None;
            }
            content = super::integer_gcd(content, value.num.unsigned_abs());
        }
        i128::try_from(content).ok()
    }

    pub(crate) fn primitive(mut self, work: &mut Work<'_>) -> Option<(Rat, Self)> {
        if self.0.is_empty() {
            return Some((Rat::ONE, self));
        }
        work.spend(self.0.len())?;
        let scale = self
            .0
            .values()
            .try_fold(1, |scale, value| lcm(scale, value.den))?;
        for value in self.0.values_mut() {
            *value = value.mul(Rat::new(scale, 1)?)?;
        }
        let content = self.content()? * self.0.last_key_value()?.1.num.signum();
        let divisor = Rat::new(content, 1)?;
        for value in self.0.values_mut() {
            *value = value.div(divisor)?;
        }
        Some((Rat::new(content, scale)?, self))
    }

    pub(crate) fn value(&self, variables: [f64; 3]) -> f64 {
        if self.0.is_empty() {
            return 0.0;
        }
        if self.0.keys().all(|p| p[1] == 0 && p[2] == 0) {
            let mut previous = self.0.last_key_value().map_or(0, |(p, _)| p[0]);
            let mut value = 0.0;
            let x = variables[0];
            for (&[power, ..], &coefficient) in self.0.iter().rev() {
                if previous != power {
                    value *= x.powi(i32::from(previous - power));
                }
                value += coefficient.to_f64();
                previous = power;
            }
            return if previous == 0 {
                value
            } else {
                value * x.powi(i32::from(previous))
            };
        }
        self.0
            .iter()
            .map(|(powers, coefficient)| {
                powers
                    .iter()
                    .zip(variables)
                    .filter(|(exponent, _)| **exponent != 0)
                    .fold(coefficient.to_f64(), |term, (&exponent, x)| {
                        term * x.powi(i32::from(exponent))
                    })
            })
            .sum()
    }

    fn shifted(&self, powers: Monomial) -> Self {
        Self(
            self.0
                .iter()
                .map(|(&p, &value)| (std::array::from_fn(|i| p[i] - powers[i]), value))
                .collect(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Quotient {
    pub(crate) numerator: Poly,
    pub(crate) denominator: Poly,
}

impl Quotient {
    pub(crate) fn polynomial(numerator: Poly) -> Self {
        Self {
            numerator,
            denominator: Poly::constant(Rat::ONE),
        }
    }

    pub(crate) fn scalar(&self) -> Option<Rat> {
        self.numerator.scalar()?.div(self.denominator.scalar()?)
    }

    pub(crate) fn add(self, rhs: Self, work: &mut Work<'_>) -> Option<Self> {
        self.sum(rhs, Rat::ONE, work)
    }

    pub(crate) fn sub(self, rhs: Self, work: &mut Work<'_>) -> Option<Self> {
        self.sum(rhs, Rat::ONE.neg()?, work)
    }

    fn sum(self, rhs: Self, sign: Rat, work: &mut Work<'_>) -> Option<Self> {
        if self.denominator == rhs.denominator {
            return Some(Self {
                numerator: self.numerator.sum(rhs.numerator, sign, work)?,
                denominator: self.denominator,
            });
        }
        Some(Self {
            numerator: self.numerator.mul(&rhs.denominator, work)?.sum(
                rhs.numerator.mul(&self.denominator, work)?,
                sign,
                work,
            )?,
            denominator: self.denominator.mul(&rhs.denominator, work)?,
        })
    }

    pub(crate) fn mul(self, rhs: Self, work: &mut Work<'_>) -> Option<Self> {
        Some(Self {
            numerator: self.numerator.mul(&rhs.numerator, work)?,
            denominator: self.denominator.mul(&rhs.denominator, work)?,
        })
    }

    pub(crate) fn div(self, rhs: Self, work: &mut Work<'_>) -> Option<Self> {
        if rhs.numerator.0.is_empty() {
            return None;
        }
        self.mul(
            Self {
                numerator: rhs.denominator,
                denominator: rhs.numerator,
            },
            work,
        )
    }

    pub(crate) fn pow(mut self, exponent: i128, work: &mut Work<'_>) -> Option<Self> {
        let mut power = u32::try_from(exponent.unsigned_abs()).ok()?;
        if power > u32::from(MAX_DEGREE) || exponent < 0 && self.numerator.0.is_empty() {
            return None;
        }
        if exponent < 0 {
            std::mem::swap(&mut self.numerator, &mut self.denominator);
        }
        let mut result = Self::polynomial(Poly::constant(Rat::ONE));
        while power != 0 {
            if power % 2 == 1 {
                result = result.mul(self.clone(), work)?;
            }
            power /= 2;
            if power != 0 {
                self = self.clone().mul(self, work)?;
            }
        }
        Some(result)
    }

    pub(crate) fn normalized(mut self, work: &mut Work<'_>) -> Option<Self> {
        if let Some(scale) = self.denominator.scalar() {
            return Some(Self::polynomial(
                self.numerator.scale(Rat::ONE.div(scale)?, work)?,
            ));
        }
        if self.numerator.0.is_empty() {
            return Some(Self::polynomial(self.numerator));
        }
        if self.numerator == self.denominator {
            return Some(Self::polynomial(Poly::constant(Rat::ONE)));
        }
        if let (Some(num), Some(den)) = (
            self.numerator.coefficients(),
            self.denominator.coefficients(),
        ) {
            let gcd = factor::gcd(&num, &den, work)?;
            self.numerator = Poly::dense(&factor::divide_exact(&num, &gcd, work)?);
            self.denominator = Poly::dense(&factor::divide_exact(&den, &gcd, work)?);
        } else if let Some(gcd) =
            super::multivariate::gcd(&self.numerator.0, &self.denominator.0, work)
        {
            self.numerator = Poly(super::multivariate::divide_exact(
                &self.numerator.0,
                &gcd,
                work,
            )?);
            self.denominator = Poly(super::multivariate::divide_exact(
                &self.denominator.0,
                &gcd,
                work,
            )?);
        } else {
            let powers = self
                .numerator
                .0
                .keys()
                .chain(self.denominator.0.keys())
                .fold([MAX_DEGREE; 3], |minimum, key| {
                    std::array::from_fn(|i| minimum[i].min(key[i]))
                });
            self.numerator = self.numerator.shifted(powers);
            self.denominator = self.denominator.shifted(powers);
        }
        let lead = *self.denominator.0.last_key_value()?.1;
        let scale = Rat::ONE.div(lead)?;
        Some(Self {
            numerator: self.numerator.scale(scale, work)?,
            denominator: self.denominator.scale(scale, work)?,
        })
    }

    pub(crate) fn integral_parts(mut self, work: &mut Work<'_>) -> Option<Self> {
        work.spend(self.numerator.0.len() + self.denominator.0.len())?;
        let scale = self
            .numerator
            .0
            .values()
            .chain(self.denominator.0.values())
            .try_fold(1, |scale, value| lcm(scale, value.den))?;
        let scale = Rat::new(scale, 1)?;
        self.numerator = self.numerator.scale(scale, work)?;
        self.denominator = self.denominator.scale(scale, work)?;
        Some(self)
    }

    pub(crate) fn coefficients(self, work: &mut Work<'_>) -> Option<Vec<Rat>> {
        let normalized = self.normalized(work)?;
        (normalized.denominator.scalar()? == Rat::ONE)
            .then(|| normalized.numerator.coefficients())
            .flatten()
    }

    pub(crate) fn slope(&self, x: f64, charge: impl Fn(u64) -> bool) -> Option<f64> {
        let deriv = |poly: &Poly| {
            poly.0
                .iter()
                .filter(|(p, _)| p[0] != 0)
                .map(|(p, c)| f64::from(p[0]) * c.to_f64() * x.powi(i32::from(p[0]) - 1))
                .sum::<f64>()
        };
        if !charge((self.numerator.0.len() + self.denominator.0.len()) as u64) {
            return None;
        }
        let coordinates = [x, f64::NAN, f64::NAN];
        let a = self.numerator.value(coordinates);
        let b = self.denominator.value(coordinates);
        Some((deriv(&self.numerator) * b - a * deriv(&self.denominator)) / (b * b))
    }

    pub(crate) fn value(&self, x: f64, y: f64) -> f64 {
        let variables = [x, y, f64::NAN];
        self.numerator.value(variables) / self.denominator.value(variables)
    }
}

fn lcm(a: i128, b: i128) -> Option<i128> {
    let gcd = i128::try_from(super::integer_gcd(a.unsigned_abs(), b.unsigned_abs())).ok()?;
    a.checked_div(gcd)?.checked_mul(b)
}
