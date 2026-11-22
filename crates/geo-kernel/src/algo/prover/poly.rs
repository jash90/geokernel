// Licensed under the EUPL 1.2.

//! Sparse multivariate polynomials with integer coefficients.
//!
//! Variables are numbered from 0. A [`Poly`] keeps its terms sorted by the
//! monomial order it was last sorted with, largest first; the order is part
//! of the computation that uses it (see [`Order`]).

use std::cmp::Ordering;

use super::int::Int;
use super::Budget;

/// The largest number of variables of one system.
pub(crate) const MAX_VARS: usize = 40;
/// The largest exponent of a variable.
const MAX_EXPONENT: u8 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Mono(pub(crate) [u8; MAX_VARS]);

impl Mono {
    pub(crate) const ONE: Mono = Mono([0; MAX_VARS]);

    pub(crate) fn var(index: usize) -> Mono {
        let mut m = Self::ONE;
        m.0[index] = 1;
        m
    }

    pub(crate) fn degree(&self) -> u32 {
        self.0.iter().map(|&e| u32::from(e)).sum()
    }

    pub(crate) fn is_one(&self) -> bool {
        self.0.iter().all(|&e| e == 0)
    }

    pub(crate) fn mul(&self, other: &Mono) -> Option<Mono> {
        let mut m = Self::ONE;
        for i in 0..MAX_VARS {
            let e = self.0[i].checked_add(other.0[i])?;
            if e > MAX_EXPONENT {
                return None;
            }
            m.0[i] = e;
        }
        Some(m)
    }

    pub(crate) fn divides(&self, other: &Mono) -> bool {
        self.0.iter().zip(&other.0).all(|(a, b)| a <= b)
    }

    /// `other / self` for a divisor.
    pub(crate) fn quotient(&self, other: &Mono) -> Mono {
        let mut m = Self::ONE;
        for i in 0..MAX_VARS {
            m.0[i] = other.0[i] - self.0[i];
        }
        m
    }

    pub(crate) fn lcm(&self, other: &Mono) -> Mono {
        let mut m = Self::ONE;
        for i in 0..MAX_VARS {
            m.0[i] = self.0[i].max(other.0[i]);
        }
        m
    }

    pub(crate) fn coprime(&self, other: &Mono) -> bool {
        self.0.iter().zip(&other.0).all(|(&a, &b)| a == 0 || b == 0)
    }

    pub(crate) fn uses_any(&self, vars: std::ops::Range<usize>) -> bool {
        self.0[vars].iter().any(|&e| e != 0)
    }
}

/// A block order: graded reverse lexicographic on the variables below
/// `split`, then, for equal parts there, graded reverse lexicographic on the
/// rest. Every monomial that contains a variable below `split` is larger than
/// every monomial without one, so the elements of a Groebner basis free of
/// those variables generate the elimination ideal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Order {
    pub(crate) split: usize,
    pub(crate) vars: usize,
}

impl Order {
    pub(crate) fn cmp(&self, a: &Mono, b: &Mono) -> Ordering {
        grevlex(&a.0[..self.split], &b.0[..self.split])
            .then_with(|| grevlex(&a.0[self.split..self.vars], &b.0[self.split..self.vars]))
    }
}

fn grevlex(a: &[u8], b: &[u8]) -> Ordering {
    let da: u32 = a.iter().map(|&e| u32::from(e)).sum();
    let db: u32 = b.iter().map(|&e| u32::from(e)).sum();
    da.cmp(&db).then_with(|| {
        for (x, y) in a.iter().zip(b).rev() {
            if x != y {
                return y.cmp(x);
            }
        }
        Ordering::Equal
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Poly {
    pub(crate) terms: Vec<(Mono, Int)>,
}

impl Poly {
    pub(crate) fn zero() -> Self {
        Self::default()
    }

    pub(crate) fn constant(c: i64) -> Self {
        Self::term(Mono::ONE, Int::from_i64(c))
    }

    pub(crate) fn var(index: usize) -> Self {
        Self::term(Mono::var(index), Int::from_i64(1))
    }

    fn term(m: Mono, c: Int) -> Self {
        if c.is_zero() {
            Self::zero()
        } else {
            Self {
                terms: vec![(m, c)],
            }
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub(crate) fn is_constant(&self) -> bool {
        self.terms.iter().all(|(m, _)| m.is_one())
    }

    pub(crate) fn degree(&self) -> u32 {
        self.terms.iter().map(|(m, _)| m.degree()).max().unwrap_or(0)
    }

    pub(crate) fn uses_var(&self, index: usize) -> bool {
        self.terms.iter().any(|(m, _)| m.0[index] != 0)
    }

    /// Sorts the terms by `order`, largest first.
    pub(crate) fn sort(&mut self, order: &Order) {
        self.terms.sort_by(|a, b| order.cmp(&b.0, &a.0));
    }

    /// Terms in a canonical order, for equality tests that do not depend on
    /// a monomial order.
    pub(crate) fn canonical(mut self) -> Self {
        self.terms.sort_by(|a, b| b.0.cmp(&a.0));
        self
    }

    pub(crate) fn add(&self, other: &Poly, budget: &Budget) -> Option<Poly> {
        budget.charge(self.terms.len() + other.terms.len())?;
        let mut map = std::collections::BTreeMap::new();
        for (m, c) in self.terms.iter().chain(&other.terms) {
            let entry = map.entry(*m).or_insert_with(Int::zero);
            *entry = entry.add(c);
        }
        let terms = map.into_iter().filter(|(_, c)| !c.is_zero()).collect();
        Some(Poly { terms }.canonical())
    }

    pub(crate) fn neg(&self) -> Poly {
        Poly {
            terms: self.terms.iter().map(|(m, c)| (*m, c.neg())).collect(),
        }
    }

    pub(crate) fn sub(&self, other: &Poly, budget: &Budget) -> Option<Poly> {
        self.add(&other.neg(), budget)
    }

    pub(crate) fn mul(&self, other: &Poly, budget: &Budget) -> Option<Poly> {
        budget.charge(self.terms.len() * other.terms.len() + 1)?;
        let mut map = std::collections::BTreeMap::new();
        for (m1, c1) in &self.terms {
            for (m2, c2) in &other.terms {
                let entry = map.entry(m1.mul(m2)?).or_insert_with(Int::zero);
                *entry = entry.add(&c1.mul(c2));
            }
        }
        let terms = map.into_iter().filter(|(_, c)| !c.is_zero()).collect();
        Some(Poly { terms }.canonical())
    }

    /// Replaces variables by integers.
    pub(crate) fn substitute(&self, values: &[(usize, i64)], budget: &Budget) -> Option<Poly> {
        budget.charge(self.terms.len())?;
        let mut map = std::collections::BTreeMap::new();
        for (m, c) in &self.terms {
            let mut m = *m;
            let mut c = c.clone();
            for &(index, value) in values {
                let e = m.0[index];
                if e != 0 {
                    for _ in 0..e {
                        c = c.mul(&Int::from_i64(value));
                    }
                    m.0[index] = 0;
                }
            }
            let entry = map.entry(m).or_insert_with(Int::zero);
            *entry = entry.add(&c);
        }
        let terms = map.into_iter().filter(|(_, c)| !c.is_zero()).collect();
        Some(Poly { terms }.canonical())
    }

    /// Renames variable `i` to `map[i]`.
    pub(crate) fn rename(&self, map: &[usize]) -> Poly {
        let terms = self
            .terms
            .iter()
            .map(|(m, c)| {
                let mut renamed = Mono::ONE;
                for (i, &e) in m.0.iter().enumerate() {
                    if e != 0 {
                        renamed.0[map[i]] = e;
                    }
                }
                (renamed, c.clone())
            })
            .collect();
        Poly { terms }.canonical()
    }

    pub(crate) fn content(&self) -> Int {
        let mut g = Int::zero();
        for (_, c) in &self.terms {
            g = g.gcd(c);
            if g.is_unit() {
                break;
            }
        }
        g
    }

    /// The polynomial divided by its content, with a positive leading
    /// coefficient (in the current term order).
    pub(crate) fn primitive(mut self) -> Poly {
        let g = self.content();
        if g.is_zero() {
            return self;
        }
        let flip = self.terms[0].1.is_negative();
        let g = if flip { g.neg() } else { g };
        if !g.is_unit() || flip {
            for (_, c) in &mut self.terms {
                *c = c.div_exact(&g).expect("content divides every coefficient");
            }
        }
        self
    }

    /// The exact quotient by `divisor` over the integers, or `None` when the
    /// division leaves a remainder or a fraction. Both are in canonical
    /// order; the division uses the lexicographic order of the monomials.
    pub(crate) fn divide(&self, divisor: &Poly, budget: &Budget) -> Option<Poly> {
        let lead = divisor.terms.first()?;
        let mut rest = self.clone();
        let mut quotient = Vec::new();
        while let Some((m, c)) = rest.terms.first().cloned() {
            if !lead.0.divides(&m) {
                return None;
            }
            let coefficient = c.div_exact(&lead.1)?;
            let factor = lead.0.quotient(&m);
            let step = Poly {
                terms: divisor
                    .terms
                    .iter()
                    .map(|(dm, dc)| Some((factor.mul(dm)?, dc.mul(&coefficient))))
                    .collect::<Option<Vec<_>>>()?,
            };
            rest = rest.sub(&step, budget)?;
            quotient.push((factor, coefficient));
        }
        Some(Poly { terms: quotient }.canonical())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> Budget {
        Budget::new(1_000_000)
    }

    #[test]
    fn exact_division_and_associates() {
        let b = budget();
        let x = Poly::var(0);
        let y = Poly::var(1);
        let f = x.add(&y, &b).unwrap();
        let g = x.sub(&y, &b).unwrap();
        let product = f.mul(&g, &b).unwrap();
        assert_eq!(product.divide(&f, &b), Some(g.clone()));
        assert_eq!(product.divide(&x, &b), None);
    }

    #[test]
    fn block_order_puts_eliminated_variables_first() {
        let order = Order { split: 1, vars: 3 };
        let x = Mono::var(0);
        let big_free = Mono::var(1).mul(&Mono::var(2)).unwrap().mul(&Mono::var(2)).unwrap();
        assert_eq!(order.cmp(&x, &big_free), Ordering::Greater);
        // grevlex inside a block: y*z^2 is below y^2*z.
        let other = Mono::var(1).mul(&Mono::var(1)).unwrap().mul(&Mono::var(2)).unwrap();
        assert_eq!(order.cmp(&big_free, &other), Ordering::Less);
    }
}
