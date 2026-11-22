// Licensed under the EUPL 1.2.

use super::solve::{add_shifted, basis, Equation};
use super::{multivariate, Poly, Rat, Work};

const VARIABLES: usize = 4;

/// The first two coordinates are eliminated. The last two are Cartesian x and y.
#[derive(Clone)]
pub(crate) struct PlanePolynomial(Equation<VARIABLES>);

impl PlanePolynomial {
    pub(crate) fn constant(value: Rat) -> Self {
        if value == Rat::ZERO {
            Self(Equation::new())
        } else {
            Self(Equation::from([([0; VARIABLES], value)]))
        }
    }

    pub(crate) fn variable(index: usize) -> Self {
        let mut power = [0; VARIABLES];
        power[index] = 1;
        Self(Equation::from([(power, Rat::ONE)]))
    }

    pub(crate) fn add_scaled(
        mut self,
        rhs: &Self,
        scale: Rat,
        work: &mut Work<'_>,
    ) -> Option<Self> {
        add_shifted(&mut self.0, &rhs.0, [0; VARIABLES], scale, work)?;
        Some(self)
    }

    pub(crate) fn product(&self, rhs: &Self, work: &mut Work<'_>) -> Option<Self> {
        work.spend(self.0.len())?;
        let mut product = Equation::new();
        for (&power, &coefficient) in &self.0 {
            add_shifted(&mut product, &rhs.0, power, coefficient, work)?;
        }
        Some(Self(product))
    }
}

pub(crate) fn project_plane(equations: Vec<PlanePolynomial>, work: &mut Work<'_>) -> Option<Poly> {
    let equations = equations.into_iter().map(|p| p.0).collect::<Vec<_>>();
    let basis = basis(&equations, work)?;
    let mut plane = Vec::new();
    for polynomial in basis {
        work.spend(polynomial.len())?;
        // A lexicographic basis element whose leader has no auxiliary
        // exponents lies entirely in the Cartesian variables.
        if matches!(polynomial.last_key_value(), Some(([0, 0, ..], _))) {
            plane.push(Poly(
                polynomial
                    .into_iter()
                    .map(|([_, _, x, y], v)| ([x, y, 0], v))
                    .collect(),
            ));
        }
    }
    let Some(generator) = plane
        .iter()
        .min_by_key(|p| p.0.last_key_value().map(|(key, _)| *key))
    else {
        return Some(Poly::constant(Rat::ZERO));
    };
    if generator.scalar().is_some() {
        return None;
    }
    for polynomial in &plane {
        if polynomial != generator {
            multivariate::divide_exact(&polynomial.0, &generator.0, work)?;
        }
    }
    let (_, factors) = super::factor_poly(generator, work)?;
    let mut square_free = Poly::constant(Rat::ONE);
    for (factor, _) in factors {
        square_free = square_free.mul(&factor, work)?;
    }
    Some(square_free.primitive(work)?.1)
}
