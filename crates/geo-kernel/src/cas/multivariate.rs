//! Bounded Kronecker factorization over the rationals.
//!
//! See algorithm 1.5 in Kaltofen, *On the Complexity of Factoring Polynomials*.
//! https://kaltofen.math.ncsu.edu/bibliography/82/Ka82_thesis.pdf

use super::{factor as univariate, Poly, Rat, Work};
use std::collections::{btree_map::Entry, BTreeMap};

type Exponents = [u8; 3];
pub(super) type Terms = BTreeMap<Exponents, Rat>;
pub(super) type Decomposition = (Rat, Vec<(Terms, u32)>);

const ZERO_POWER: Exponents = [0; 3];
const MAX_TERMS: usize = 512;
const MAX_DEGREE: u16 = 64;
const MAX_COMBINATIONS: usize = 4096;

struct Encoding {
    strides: [usize; 3],
    radices: [usize; 3],
    degree: usize,
}

fn degree(powers: Exponents) -> u16 {
    powers.into_iter().map(u16::from).sum()
}

fn weight(strides: [usize; 3], powers: Exponents) -> usize {
    strides
        .into_iter()
        .zip(powers)
        .map(|(stride, power)| stride * usize::from(power))
        .sum()
}

impl Encoding {
    fn new(terms: &Terms, work: &mut Work<'_>) -> Option<Self> {
        const ORDERS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        work.spend((ORDERS.len() + 1) * terms.len())?;
        let mut radices = [1; 3];
        for &powers in terms.keys() {
            for (radix, power) in radices.iter_mut().zip(powers) {
                *radix = (*radix).max(usize::from(power) + 1);
            }
        }
        ORDERS
            .into_iter()
            .map(|order| {
                let mut stride = 1;
                let mut strides = [0; 3];
                for variable in order {
                    strides[variable] = stride;
                    stride *= radices[variable];
                }
                let degree = terms
                    .keys()
                    .map(|&powers| weight(strides, powers))
                    .max()
                    .unwrap_or(0);
                Self {
                    strides,
                    radices,
                    degree,
                }
            })
            .min_by_key(|encoding| encoding.degree)
    }

    fn encode(&self, terms: &Terms) -> Vec<Rat> {
        let mut coefficients = vec![Rat::ZERO; self.degree + 1];
        for (&powers, &coefficient) in terms {
            coefficients[weight(self.strides, powers)] = coefficient;
        }
        coefficients
    }

    fn decode(&self, coefficients: Vec<Rat>) -> Terms {
        coefficients
            .into_iter()
            .enumerate()
            .filter(|&(_, coefficient)| coefficient != Rat::ZERO)
            .map(|(exponent, coefficient)| {
                let powers =
                    std::array::from_fn(|i| (exponent / self.strides[i] % self.radices[i]) as u8);
                (powers, coefficient)
            })
            .collect()
    }
}

fn leading(terms: &Terms) -> Option<(Exponents, Rat)> {
    terms
        .iter()
        .max_by_key(|(&powers, _)| (degree(powers), powers))
        .map(|(&powers, &coefficient)| (powers, coefficient))
}

/// Graded leading terms prevent a division step increasing total degree.
/// A failed exact division is distinct from an exceeded arithmetic or work bound.
fn candidate_quotient(
    dividend: &Terms,
    divisor: &Terms,
    work: &mut Work<'_>,
) -> Option<Option<Terms>> {
    let (divisor_power, divisor_coefficient) = leading(divisor)?;
    work.spend(dividend.len() + divisor.len())?;
    let mut remainder = dividend.clone();
    let mut quotient = Terms::new();
    while let Some((power, coefficient)) = leading(&remainder) {
        work.spend(divisor.len() + remainder.len())?;
        let Some(x) = power[0].checked_sub(divisor_power[0]) else {
            return Some(None);
        };
        let Some(y) = power[1].checked_sub(divisor_power[1]) else {
            return Some(None);
        };
        let Some(z) = power[2].checked_sub(divisor_power[2]) else {
            return Some(None);
        };
        let factor = coefficient.div(divisor_coefficient)?;
        quotient.insert([x, y, z], factor);
        for (&[i, j, k], &coefficient) in divisor {
            let key = [x + i, y + j, z + k];
            let delta = factor.mul(coefficient)?;
            match remainder.entry(key) {
                Entry::Occupied(mut entry) => {
                    let difference = entry.get().sub(delta)?;
                    if difference == Rat::ZERO {
                        entry.remove();
                    } else {
                        entry.insert(difference);
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(delta.neg()?);
                }
            }
        }
        if remainder.len().max(quotient.len()) > MAX_TERMS {
            return None;
        }
    }
    Some(Some(quotient))
}

pub(super) fn divide_exact(
    dividend: &Terms,
    divisor: &Terms,
    work: &mut Work<'_>,
) -> Option<Terms> {
    candidate_quotient(dividend, divisor, work)?
}

fn multiply_dense(left: &[Rat], right: &[Rat], work: &mut Work<'_>) -> Option<Vec<Rat>> {
    work.spend(left.len() * right.len())?;
    let mut product = vec![Rat::ZERO; left.len() + right.len() - 1];
    for (i, &a) in left
        .iter()
        .enumerate()
        .filter(|&(_, coefficient)| *coefficient != Rat::ZERO)
    {
        for (output, &b) in product[i..].iter_mut().zip(right) {
            if b != Rat::ZERO {
                *output = output.add(a.mul(b)?)?;
            }
        }
    }
    Some(product)
}

struct FactorPowers(Vec<Vec<Rat>>);

impl FactorPowers {
    fn new(factor: Vec<Rat>, multiplicity: u32, work: &mut Work<'_>) -> Option<Self> {
        let mut result = vec![vec![Rat::ONE], factor];
        for exponent in 2..=multiplicity as usize {
            result.push(multiply_dense(&result[exponent - 1], &result[1], work)?);
        }
        Some(Self(result))
    }

    fn degree(&self, count: usize) -> usize {
        self.0[count].len() - 1
    }
}

struct Piece {
    polynomial: Terms,
    counts: Vec<usize>,
}

fn split(
    piece: &Piece,
    encoding: &Encoding,
    factors: &[FactorPowers],
    work: &mut Work<'_>,
) -> Option<Option<[Piece; 2]>> {
    let combinations = piece
        .counts
        .iter()
        .try_fold(1_usize, |product, &count| product.checked_mul(1 + count))?;
    if combinations > MAX_COMBINATIONS {
        return None;
    }
    let combined_degree: usize = factors
        .iter()
        .zip(&piece.counts)
        .map(|(factor, &count)| factor.degree(count))
        .sum();
    let mut chosen = vec![0; factors.len()];
    for index in 1..combinations - 1 {
        work.spend(factors.len())?;
        let mut digits = index;
        let mut degree = 0;
        for ((selected, &available), factor) in chosen.iter_mut().zip(&piece.counts).zip(factors) {
            *selected = digits % (available + 1);
            digits /= available + 1;
            degree += factor.degree(*selected);
        }
        if 2 * degree > combined_degree
            || (2 * degree == combined_degree && 2 * index > combinations - 1)
        {
            continue;
        }
        let mut candidate = vec![Rat::ONE];
        for (&count, factor) in chosen.iter().zip(factors) {
            if count != 0 {
                candidate = multiply_dense(&candidate, &factor.0[count], work)?;
            }
        }
        let divisor = encoding.decode(candidate);
        if let Some(quotient) = candidate_quotient(&piece.polynomial, &divisor, work)? {
            let remaining = piece
                .counts
                .iter()
                .zip(&chosen)
                .map(|(count, selected)| count - selected)
                .collect();
            return Some(Some([
                Piece {
                    polynomial: divisor,
                    counts: chosen,
                },
                Piece {
                    polynomial: quotient,
                    counts: remaining,
                },
            ]));
        }
    }
    Some(None)
}

fn normalized(mut terms: Terms, scalar: Rat, work: &mut Work<'_>) -> Option<Terms> {
    if scalar != Rat::ONE {
        work.spend(terms.len())?;
        for coefficient in terms.values_mut() {
            *coefficient = coefficient.div(scalar)?;
        }
    }
    Some(terms)
}

fn variable(index: usize) -> Terms {
    let mut powers = ZERO_POWER;
    powers[index] = 1;
    Terms::from([(powers, Rat::ONE)])
}

/// The mixed radix is injective through the original coordinate degrees,
/// which bound every genuine factor. Exhausted recombination therefore
/// certifies irreducibility after each accepted, exactly dividing split.
pub(super) fn factor(terms: &Terms, work: &mut Work<'_>) -> Option<Decomposition> {
    work.spend(terms.len())?;
    if terms.is_empty() {
        return Some((Rat::ZERO, vec![]));
    }
    if terms.len() > MAX_TERMS || terms.keys().any(|&power| degree(power) > MAX_DEGREE) {
        return None;
    }
    let monomial = terms.keys().fold([MAX_DEGREE as u8; 3], |min, key| {
        std::array::from_fn(|i| min[i].min(key[i]))
    });
    let mut result = Vec::new();
    for (index, &power) in monomial.iter().enumerate() {
        if power != 0 {
            result.push((variable(index), u32::from(power)));
        }
    }
    let terms: Terms = terms
        .iter()
        .map(|(&powers, &coefficient)| {
            (
                std::array::from_fn(|i| powers[i] - monomial[i]),
                coefficient,
            )
        })
        .collect();
    if terms.len() == 1 {
        return Some((*terms.first_key_value()?.1, result));
    }
    let encoding = Encoding::new(&terms, work)?;
    if encoding.degree > univariate::KRONECKER_MAX_DEGREE {
        return None;
    }
    let (mut scalar, factors) = univariate::factor_kronecker(&encoding.encode(&terms), work)?;
    let counts = factors.iter().map(|(_, count)| *count as usize).collect();
    let factors = factors
        .into_iter()
        .map(|(polynomial, count)| FactorPowers::new(polynomial, count, work))
        .collect::<Option<Vec<_>>>()?;
    let mut pending = vec![Piece {
        polynomial: normalized(terms, scalar, work)?,
        counts,
    }];
    while let Some(piece) = pending.pop() {
        if let Some(pair) = split(&piece, &encoding, &factors, work)? {
            pending.extend(pair);
        } else {
            let lead = *piece.polynomial.last_key_value()?.1;
            let factor = normalized(piece.polynomial, lead, work)?;
            scalar = scalar.mul(lead)?;
            if let Some((_, count)) = result
                .iter_mut()
                .find(|(polynomial, _)| *polynomial == factor)
            {
                *count += 1;
            } else {
                result.push((factor, 1));
            }
        }
    }
    Some((scalar, result))
}

pub(super) fn gcd(left: &Terms, right: &Terms, work: &mut Work<'_>) -> Option<Terms> {
    work.spend(left.len() + right.len())?;
    if left.is_empty() || right.is_empty() || left == right {
        let polynomial = if left.is_empty() { right } else { left };
        let scalar = polynomial
            .last_key_value()
            .map_or(Rat::ONE, |(_, &coefficient)| coefficient);
        return normalized(polynomial.clone(), scalar, work);
    }
    let left_size = Encoding::new(left, work)?.degree;
    let right_size = Encoding::new(right, work)?.degree;
    let (first, other) = if left_size <= right_size {
        (left, right)
    } else {
        (right, left)
    };
    let ((_, factors), mut remainder) = match factor(first, work) {
        Some(factors) => (factors, other.clone()),
        None => (factor(other, work)?, first.clone()),
    };
    let mut common = Poly::constant(Rat::ONE);
    for (divisor, multiplicity) in factors {
        let divisor = Poly(divisor);
        for _ in 0..multiplicity {
            let Some(quotient) = candidate_quotient(&remainder, &divisor.0, work)? else {
                break;
            };
            common = common.mul(&divisor, work)?;
            remainder = quotient;
        }
    }
    Some(common.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(entries: &[([u8; 3], i128)]) -> Terms {
        entries
            .iter()
            .map(|&(powers, term)| (powers, Rat::new(term, 1).unwrap()))
            .collect()
    }

    #[test]
    fn recombines_aliased_univariate_factors() {
        // GeoSymbolicTest.testTutorial: Expand((x + y) (x - y)^2).
        let polynomial = terms(&[
            ([3, 0, 0], 1),
            ([2, 1, 0], -1),
            ([1, 2, 0], -1),
            ([0, 3, 0], 1),
        ]);
        let (scalar, factors) = factor(&polynomial, &mut Work::new(&|_| true)).unwrap();
        assert_eq!(scalar, Rat::ONE);
        assert_eq!(factors.len(), 2);
        assert!(factors.contains(&(terms(&[([1, 0, 0], 1), ([0, 1, 0], 1)]), 1)));
        assert!(factors.contains(&(terms(&[([1, 0, 0], 1), ([0, 1, 0], -1)]), 2)));
    }

    #[test]
    fn factors_across_three_independent_variables() {
        // GeoSymbolicTest.testFactorCommand: Factor(x^2 - a^2 y^2).
        let polynomial = terms(&[([2, 0, 0], 1), ([0, 2, 2], -1)]);
        let (scalar, factors) = factor(&polynomial, &mut Work::new(&|_| true)).unwrap();
        assert_eq!(scalar, Rat::ONE);
        assert_eq!(factors.len(), 2);
        assert!(factors.contains(&(terms(&[([1, 0, 0], 1), ([0, 1, 1], 1)]), 1)));
        assert!(factors.contains(&(terms(&[([1, 0, 0], 1), ([0, 1, 1], -1)]), 1)));
    }

    #[test]
    fn finds_a_repeated_non_linear_factor() {
        // CommandsUsingCASTest.cmdFactor: x^6 - 4 y^3 + 3 x^4 y.
        let polynomial = terms(&[([6, 0, 0], 1), ([0, 3, 0], -4), ([4, 1, 0], 3)]);
        let (scalar, factors) = factor(&polynomial, &mut Work::new(&|_| true)).unwrap();
        assert_eq!(scalar, Rat::ONE);
        assert_eq!(factors.len(), 2);
        assert!(factors.contains(&(terms(&[([2, 0, 0], 1), ([0, 1, 0], -1)]), 1)));
        assert!(factors.contains(&(terms(&[([2, 0, 0], 1), ([0, 1, 0], 2)]), 2)));
    }

    #[test]
    fn certifies_sparse_common_divisors() {
        let left = terms(&[([2, 0, 0], 1), ([0, 2, 0], -1)]);
        let right = terms(&[([1, 1, 0], 1), ([0, 2, 0], -1)]);
        let mut work = Work::new(&|_| true);
        let divisor = gcd(&left, &right, &mut work).unwrap();
        assert_eq!(divisor, terms(&[([1, 0, 0], 1), ([0, 1, 0], -1)]));
        assert_eq!(
            divide_exact(&left, &divisor, &mut work),
            Some(terms(&[([1, 0, 0], 1), ([0, 1, 0], 1)]))
        );
        assert_eq!(
            divide_exact(&right, &divisor, &mut work),
            Some(terms(&[([0, 1, 0], 1)]))
        );
    }

    #[test]
    fn rejects_spurious_factors_and_large_searches() {
        let prime = terms(&[([2, 0, 0], 1), ([0, 1, 0], -1)]);
        let large = terms(&[([40, 0, 0], 1), ([0, 24, 0], -1)]);
        let mut work = Work::new(&|_| true);
        assert_eq!(
            factor(&prime, &mut work),
            Some((Rat::ONE, vec![(prime.clone(), 1)]))
        );
        assert_eq!(factor(&large, &mut work), None);
        assert_eq!(factor(&prime, &mut Work::new(&|_| false)), None);
    }
}
