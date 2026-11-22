// The projection rules follow reference commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/cas/giac/CASgiac.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::solve::{add_shifted, basis, systems, Equation, SystemRoots};
use super::{multivariate, Poly, Rat, Work};

pub(crate) const PLANE_X: usize = 8;
pub(crate) const PLANE_Y: usize = PLANE_X + 1;
const VARIABLES: usize = PLANE_Y + 1;

/// Auxiliary variables precede Cartesian x and y in the elimination order.
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

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_empty()
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

    pub(crate) fn derivative(&self, index: usize, work: &mut Work<'_>) -> Option<Self> {
        work.spend(self.0.len())?;
        let mut derivative = Equation::new();
        for (&power, &coefficient) in &self.0 {
            let exponent = i128::from(power[index]);
            if exponent != 0 {
                let mut power = power;
                power[index] -= 1;
                derivative.insert(power, coefficient.mul(Rat::new(exponent, 1)?)?);
            }
        }
        Some(Self(derivative))
    }
}

pub(crate) fn project_plane(
    equations: Vec<PlanePolynomial>,
    precision: i64,
    work: &mut Work<'_>,
) -> Option<Poly> {
    let equations = equations.into_iter().map(|p| p.0).collect::<Vec<_>>();
    let basis = basis(&equations, work)?;
    let mut plane = Vec::new();
    for polynomial in basis {
        work.spend(polynomial.len())?;
        // A lexicographic basis element whose leader has no auxiliary
        // exponents lies entirely in the Cartesian variables.
        if polynomial
            .last_key_value()
            .is_some_and(|(power, _)| power[..PLANE_X].iter().all(|&p| p == 0))
        {
            plane.push(Poly(
                polynomial
                    .into_iter()
                    .map(|(power, v)| ([power[PLANE_X], power[PLANE_Y], 0], v))
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
        if polynomial != generator
            && multivariate::divide_exact(&polynomial.0, &generator.0, work).is_none()
        {
            return discrete_projection(plane, precision, work);
        }
    }
    let (_, factors) = super::factor_poly(generator, work)?;
    let mut square_free = Poly::constant(Rat::ONE);
    for (factor, _) in factors {
        square_free = square_free.mul(&factor, work)?;
    }
    Some(square_free.primitive(work)?.1)
}

fn discrete_projection(plane: Vec<Poly>, precision: i64, work: &mut Work<'_>) -> Option<Poly> {
    let equations: Vec<_> = plane.into_iter().map(|poly| poly.0).collect();
    let SystemRoots::Finite(points) = systems(&equations, 2, work)? else {
        return None;
    };
    let mut product = Poly::constant(Rat::ONE);
    let mut seen = std::collections::BTreeSet::new();
    for point in points {
        work.spend(1)?;
        let [x, y] = point.as_slice() else {
            return None;
        };
        let x = rounded_numerator(x.value(), precision)?;
        let y = rounded_numerator(y.value(), precision)?;
        if seen.insert((x, y)) {
            let squared_offset = |p, numerator: i128, work: &mut Work<'_>| {
                let offset = Poly::variable(p).sum(
                    Poly::constant(Rat::new(numerator.checked_neg()?, i128::from(precision))?),
                    Rat::ONE,
                    work,
                )?;
                offset.mul(&offset, work)
            };
            let xx = squared_offset(0, x, work)?;
            let yy = squared_offset(1, y, work)?;
            product = product.mul(&xx.sum(yy, Rat::ONE, work)?, work)?;
        }
    }
    Some(product.primitive(work)?.1)
}

pub(crate) fn rounded_numerator(value: f64, precision: i64) -> Option<i128> {
    let scaled = value * precision as f64;
    let floor = scaled.floor();
    let rounded = floor + f64::from(scaled - floor >= 0.5);
    (rounded.abs() < i64::MAX as f64).then_some(rounded as i128)
}
