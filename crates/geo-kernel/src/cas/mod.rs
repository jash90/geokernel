//! Bounded exact polynomial and rational-expression operations.

mod algebraic;
mod factor;
mod multivariate;
mod polynomial;
mod rational;
mod solve;

pub(crate) use algebraic::Algebraic;
pub(crate) use factor::{divide_exact as polynomial_quotient, gcd as polynomial_gcd};
pub(crate) use polynomial::{Poly, Quotient};
pub(crate) use rational::gcd as integer_gcd;
pub(crate) use rational::Rat;
pub(crate) use solve::{
    affine_system, guided_root, real_roots, systems, unique_real_root, Root, Roots, SystemRoots,
};
mod projection;
pub(crate) use projection::{project_plane, rounded_numerator, PlanePolynomial, PLANE_X, PLANE_Y};

pub(crate) type Factorization = (Rat, Vec<(Vec<Rat>, u32)>);

pub(crate) fn factor(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Factorization> {
    factor::factor(coefficients, work)
}

pub(crate) fn factor_poly(poly: &Poly, work: &mut Work<'_>) -> Option<(Rat, Vec<(Poly, u32)>)> {
    let result = if let Some(coefficients) = poly.coefficients() {
        let (scalar, factors) = factor(&coefficients, work)?;
        let factors = factors
            .into_iter()
            .map(|(factor, e)| (Poly::dense(&factor), e))
            .collect();
        (scalar, factors)
    } else {
        let (scalar, factors) = multivariate::factor(&poly.0, work)?;
        let factors = factors
            .into_iter()
            .map(|(factor, e)| (Poly(factor), e))
            .collect();
        (scalar, factors)
    };
    Some(result)
}

/// The greatest common divisor of two polynomials in up to three
/// variables; a constant when they share no factor.
pub(crate) fn poly_gcd(left: &Poly, right: &Poly, work: &mut Work<'_>) -> Option<Poly> {
    if let (Some(a), Some(b)) = (left.coefficients(), right.coefficients()) {
        return Some(Poly::dense(&factor::gcd(&a, &b, work)?));
    }
    multivariate::gcd(&left.0, &right.0, work).map(Poly)
}

pub(crate) struct Work<'a> {
    remaining: usize,
    charge: &'a dyn Fn(usize) -> bool,
}

impl<'a> Work<'a> {
    pub(crate) const fn new(charge: &'a dyn Fn(usize) -> bool) -> Self {
        Self {
            remaining: 500_000,
            charge,
        }
    }

    pub(crate) fn spend(&mut self, units: usize) -> Option<()> {
        self.remaining = self.remaining.checked_sub(units)?;
        (self.charge)(units).then_some(())
    }
}
