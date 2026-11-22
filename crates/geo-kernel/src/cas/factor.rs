//! Exact univariate polynomial operations, with coefficients in degree order.

mod modular;

use super::rational::gcd as integer_gcd;
use super::{Rat, Work};

pub(super) const FACTOR_MAX_DEGREE: usize = 32;
pub(super) const KRONECKER_MAX_DEGREE: usize = 24;
pub(super) const GCD_MAX_DEGREE: usize = 64;

pub(super) type Decomposition = (Rat, Vec<(Vec<Rat>, u32)>);

fn trimmed(coefficients: &[Rat]) -> &[Rat] {
    let length = coefficients
        .iter()
        .rposition(|&coefficient| coefficient != Rat::ZERO)
        .map_or(0, |index| index + 1);
    &coefficients[..length]
}

fn monic(mut coefficients: Vec<Rat>, work: &mut Work<'_>) -> Option<Vec<Rat>> {
    if let Some(&lead) = coefficients.last() {
        work.spend(coefficients.len())?;
        for coefficient in &mut coefficients {
            *coefficient = coefficient.div(lead)?;
        }
    }
    Some(coefficients)
}

fn div_rem(dividend: &[Rat], divisor: &[Rat], work: &mut Work<'_>) -> Option<(Vec<Rat>, Vec<Rat>)> {
    let lead = *divisor.last()?;
    if dividend.len() < divisor.len() {
        return Some((vec![], dividend.to_vec()));
    }
    let steps = dividend.len() - divisor.len() + 1;
    work.spend(steps * divisor.len())?;
    let mut remainder = dividend.to_vec();
    let mut quotient = vec![Rat::ZERO; steps];
    for shift in (0..steps).rev() {
        let end = shift + divisor.len() - 1;
        let coefficient = remainder[end].div(lead)?;
        if coefficient != Rat::ZERO {
            for (value, &divisor_term) in remainder[shift..end].iter_mut().zip(divisor) {
                *value = value.sub(coefficient.mul(divisor_term)?)?;
            }
        }
        remainder[end] = Rat::ZERO;
        quotient[shift] = coefficient;
    }
    remainder.truncate(trimmed(&remainder).len());
    Some((quotient, remainder))
}

pub(crate) fn divide_exact(
    dividend: &[Rat],
    divisor: &[Rat],
    work: &mut Work<'_>,
) -> Option<Vec<Rat>> {
    work.spend(dividend.len().checked_add(divisor.len())?)?;
    let dividend = trimmed(dividend);
    let divisor = trimmed(divisor);
    if dividend.len().max(divisor.len()) > GCD_MAX_DEGREE + 1 {
        return None;
    }
    let (quotient, remainder) = div_rem(dividend, divisor, work)?;
    remainder.is_empty().then_some(quotient)
}

pub(crate) fn gcd(left: &[Rat], right: &[Rat], work: &mut Work<'_>) -> Option<Vec<Rat>> {
    work.spend(left.len().checked_add(right.len())?)?;
    let left = trimmed(left);
    let right = trimmed(right);
    if left.len().max(right.len()) > GCD_MAX_DEGREE + 1 {
        return None;
    }
    if left.is_empty() || left == right {
        return monic(right.to_vec(), work);
    }
    if right.is_empty() {
        return monic(left.to_vec(), work);
    }
    if left.len() == 1 || right.len() == 1 {
        return Some(vec![Rat::ONE]);
    }
    let mut left = primitive(left, work)?;
    let mut right = primitive(right, work)?;
    if let Some(divisor) = modular::gcd(&left, &right, work)? {
        left = divisor;
    } else {
        while right.len() > 1 {
            let remainder = pseudo_remainder(left, &right, work)?;
            left = right;
            right = remainder;
        }
        if right.len() == 1 {
            return Some(vec![Rat::ONE]);
        }
    }
    let leading = *left.last()?;
    work.spend(left.len())?;
    left.into_iter()
        .map(|term| Rat::new(term, leading))
        .collect()
}

fn derivative(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<Rat>> {
    work.spend(coefficients.len())?;
    coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(exponent, &coefficient)| coefficient.mul(Rat::new(exponent as i128, 1)?))
        .collect()
}

/// Yun's square-free decomposition over a characteristic-zero field.
/// https://doi.org/10.1145/800205.806320
pub(super) fn square_free(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Decomposition> {
    work.spend(coefficients.len())?;
    let coefficients = trimmed(coefficients);
    let leading = coefficients.last().copied().unwrap_or(Rat::ZERO);
    if coefficients.len() <= 1 {
        return Some((leading, vec![]));
    }
    if coefficients.len() > GCD_MAX_DEGREE + 1 {
        return None;
    }
    let polynomial = monic(coefficients.to_vec(), work)?;
    let df = derivative(&polynomial, work)?;
    let common = gcd(&polynomial, &df, work)?;
    if common.len() == 1 {
        return Some((leading, vec![(polynomial, 1)]));
    }
    let mut b = divide_exact(&polynomial, &common, work)?;
    let mut c = divide_exact(&df, &common, work)?;
    let mut multiplicity = 1;
    let mut factors = Vec::new();
    while b.len() > 1 {
        let db = derivative(&b, work)?;
        c.resize(c.len().max(db.len()), Rat::ZERO);
        for (term, derivative_term) in c.iter_mut().zip(db) {
            *term = term.sub(derivative_term)?;
        }
        c.truncate(trimmed(&c).len());
        let factor = gcd(&b, &c, work)?;
        b = divide_exact(&b, &factor, work)?;
        c = divide_exact(&c, &factor, work)?;
        if factor.len() > 1 {
            factors.push((factor, multiplicity));
        }
        multiplicity += 1;
    }
    Some((leading, factors))
}

fn normalize_integers(mut coefficients: Vec<i128>) -> Option<Vec<i128>> {
    let content = coefficients
        .iter()
        .fold(0, |gcd, term| integer_gcd(gcd, term.unsigned_abs()));
    if content != 0 {
        let content = i128::try_from(content).ok()?;
        let divisor = if coefficients.last().copied()? < 0 {
            -content
        } else {
            content
        };
        for coefficient in &mut coefficients {
            *coefficient = coefficient.checked_div(divisor)?;
        }
    }
    Some(coefficients)
}

// Cancel each leading term by cross multiplication and discard the content
// before the next cancellation. Each step preserves the remainder up to scale.
fn pseudo_remainder(mut left: Vec<i128>, right: &[i128], work: &mut Work<'_>) -> Option<Vec<i128>> {
    let right_lead = *right.last()?;
    while left.len() >= right.len() {
        work.spend(2 * left.len())?;
        let left_lead = left.pop()?;
        let common = integer_gcd(left_lead.unsigned_abs(), right_lead as u128) as i128;
        let scale = right_lead / common;
        let factor = left_lead / common;
        let shift = left.len() + 1 - right.len();
        let (prefix, difference) = left.split_at_mut(shift);
        for term in prefix {
            *term = term.checked_mul(scale)?;
        }
        for (a, &b) in difference.iter_mut().zip(right) {
            *a = a.checked_mul(scale)?.checked_sub(b.checked_mul(factor)?)?;
        }
        while left.last() == Some(&0) {
            left.pop();
        }
        left = normalize_integers(left)?;
    }
    Some(left)
}

pub(super) fn primitive(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<i128>> {
    work.spend(coefficients.len())?;
    let coefficients = trimmed(coefficients);
    let Some(leading) = coefficients.last() else {
        return Some(vec![]);
    };
    // Any prime dividing a denominator's LCM is absent from at least one
    // cleared numerator, since the rational coefficients are already reduced.
    let content = coefficients.iter().fold(0, |gcd, coefficient| {
        integer_gcd(gcd, coefficient.num.unsigned_abs())
    });
    let content = i128::try_from(content).ok()? * leading.num.signum();
    let denominator = coefficients.iter().try_fold(1_i128, |lcm, coefficient| {
        let gcd = integer_gcd(lcm as u128, coefficient.den as u128) as i128;
        (lcm / gcd).checked_mul(coefficient.den)
    })?;
    coefficients
        .iter()
        .map(|coefficient| (coefficient.num / content).checked_mul(denominator / coefficient.den))
        .collect()
}

pub(super) fn factor(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Decomposition> {
    constrained_factor(coefficients, FACTOR_MAX_DEGREE, work)
}

pub(super) fn factor_kronecker(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Decomposition> {
    constrained_factor(coefficients, KRONECKER_MAX_DEGREE, work)
}

fn constrained_factor(
    coefficients: &[Rat],
    degree_limit: usize,
    work: &mut Work<'_>,
) -> Option<Decomposition> {
    work.spend(coefficients.len())?;
    let coefficients = trimmed(coefficients);
    if coefficients.len() > degree_limit + 1 {
        return None;
    }
    if let Some(cyclotomic) = cyclotomic_split(coefficients, work)? {
        return Some(cyclotomic);
    }
    let (scalar, square_free_parts) = square_free(coefficients, work)?;
    let mut factors = Vec::new();
    for (part, multiplicity) in square_free_parts {
        let mut pending = vec![part];
        while let Some(polynomial) = pending.pop() {
            if let Some(divisor) = modular::proper_factor(&polynomial, work)? {
                pending.push(divide_exact(&polynomial, &divisor, work)?);
                pending.push(divisor);
            } else {
                factors.push((polynomial, multiplicity));
            }
        }
    }
    Some((scalar, factors))
}

/// A scalar multiple of `x^n - 1` or `x^n + 1` splits into the cyclotomic
/// polynomials it is the product of — each irreducible over the rationals —
/// by exact division alone, without any modular factorization. `x^15 - 1`
/// and its relatives defeat the subset recombination of the Berlekamp–
/// Zassenhaus path (every prime splits them into many factors), which is
/// why they take this road. Inner `None`: not of this shape.
fn cyclotomic_split(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Option<Decomposition>> {
    let Some(leading) = coefficients.last().copied() else {
        return Some(None);
    };
    let degree = coefficients.len() - 1;
    if degree < 2
        || coefficients[1..degree]
            .iter()
            .any(|coefficient| *coefficient != Rat::ZERO)
    {
        return Some(None);
    }
    let plus = coefficients[0] == leading;
    if !plus && coefficients[0] != leading.neg()? {
        return Some(None);
    }
    // x^n + 1 is the product of Phi_d over the divisors d of 2n that do not
    // divide n; x^n - 1 over the divisors of n. Every Phi_d is built from
    // x^d - 1 divided by all the Phi_e with e a proper divisor of d — also
    // the ones the answer will not keep, since x^d - 1 contains them too.
    let target = if plus { 2 * degree } else { degree };
    let mut cyclotomics: Vec<(usize, Vec<Rat>)> = Vec::new();
    for divisor in 1..=target {
        if target % divisor != 0 {
            continue;
        }
        let mut numerator = vec![Rat::ZERO; divisor + 1];
        numerator[0] = Rat::ONE.neg()?;
        numerator[divisor] = Rat::ONE;
        for &(smaller, ref phi) in &cyclotomics {
            if divisor.is_multiple_of(smaller) {
                numerator = divide_exact(&numerator, phi, work)?;
            }
        }
        cyclotomics.push((divisor, numerator));
    }
    let factors = cyclotomics
        .into_iter()
        .filter(|&(divisor, _)| !plus || !degree.is_multiple_of(divisor))
        .map(|(_, phi)| (phi, 1))
        .collect::<Vec<_>>();
    Some(Some((leading, factors)))
}

pub(super) fn rational_roots(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<Rat>> {
    let (_, square_free_parts) = square_free(coefficients, work)?;
    let mut roots = Vec::new();
    for (part, _) in square_free_parts {
        roots.extend(modular::rational_roots(&part, work)?);
    }
    Some(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integers(values: &[i128]) -> Vec<Rat> {
        values
            .iter()
            .map(|&value| Rat::new(value, 1).unwrap())
            .collect()
    }

    /// A work meter with the budget of a CAS command: `metered!(work, spent)`
    /// declares both; afterwards `spent.get()` reports the units charged.
    macro_rules! metered {
        ($work:ident, $spent:ident) => {
            let $spent = std::cell::Cell::new(0_usize);
            let charge = |n: usize| {
                let total = $spent.get() + n;
                $spent.set(total);
                total <= 500_000
            };
            let mut $work = Work::new(&charge);
        };
    }

    fn product_of(factors: &[(Vec<Rat>, u32)]) -> Vec<Rat> {
        let mut product = vec![Rat::ONE];
        for (factor, multiplicity) in factors {
            for _ in 0..*multiplicity {
                let mut next = vec![Rat::ZERO; product.len() + factor.len() - 1];
                for (i, &a) in product.iter().enumerate() {
                    for (j, &b) in factor.iter().enumerate() {
                        next[i + j] = next[i + j].add(a.mul(b).unwrap()).unwrap();
                    }
                }
                product = next;
            }
        }
        product
    }

    fn binomial(degree: usize, plus: bool) -> Vec<Rat> {
        let mut coefficients = vec![Rat::ZERO; degree + 1];
        coefficients[0] = if plus {
            Rat::ONE
        } else {
            Rat::ONE.neg().unwrap()
        };
        coefficients[degree] = Rat::ONE;
        coefficients
    }

    #[test]
    fn factor_preserves_constants_and_repeated_factors() {
        // CommandsTest.cmdFactor: Factor(x^2).
        let mut work = Work::new(&|_| true);
        assert_eq!(factor(&[], &mut work), Some((Rat::ZERO, vec![])));
        assert_eq!(
            factor(&integers(&[-17]), &mut work),
            Some((Rat::new(-17, 1).unwrap(), vec![]))
        );
        assert_eq!(
            factor(&integers(&[0, 0, 1]), &mut work),
            Some((Rat::ONE, vec![(integers(&[0, 1]), 2)]))
        );
        // CommandsTest.cmdIsFactored: (x^2 + 1)^2 and its expanded form.
        assert_eq!(
            factor(&integers(&[1, 0, 2, 0, 1]), &mut work),
            Some((Rat::ONE, vec![(integers(&[1, 0, 1]), 2)]))
        );
    }

    #[test]
    fn factors_quartic_without_linear_factors() {
        // CommandsTest.cmdIsFactored: x^4 + 4 is reducible over the rationals.
        let (scalar, factors) =
            factor(&integers(&[4, 0, 0, 0, 1]), &mut Work::new(&|_| true)).unwrap();
        assert_eq!(scalar, Rat::ONE);
        assert_eq!(factors.len(), 2);
        assert!(factors.contains(&(integers(&[2, -2, 1]), 1)));
        assert!(factors.contains(&(integers(&[2, 2, 1]), 1)));
    }

    #[test]
    fn keeps_irreducible_factors_whole() {
        // CommandsTest.cmdIFactor: x^4 - 2.
        let mut work = Work::new(&|_| true);
        for polynomial in [integers(&[-2, 0, 0, 0, 1]), {
            let mut coefficients = integers(&[-2]);
            coefficients.resize(13, Rat::ZERO);
            coefficients[12] = Rat::ONE;
            coefficients
        }] {
            assert_eq!(
                factor(&polynomial, &mut work),
                Some((Rat::ONE, vec![(polynomial.clone(), 1)]))
            );
        }
    }

    #[test]
    fn recovers_non_monic_factors_and_roots() {
        // CommandsTest.cmdIsFactored: 49(x - 3/7)(x + 3/7).
        let mut work = Work::new(&|_| true);
        let roots = rational_roots(&integers(&[-9, 0, 49]), &mut work).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&Rat::new(-3, 7).unwrap()));
        assert!(roots.contains(&Rat::new(3, 7).unwrap()));
        let factor_a = vec![Rat::new(1, 2).unwrap(), Rat::ZERO, Rat::ONE];
        let factor_b = vec![Rat::new(1, 3).unwrap(), Rat::ZERO, Rat::ONE];
        let (scalar, factors) = factor(&integers(&[1, 0, 5, 0, 6]), &mut work).unwrap();
        assert_eq!(scalar, Rat::new(6, 1).unwrap());
        assert_eq!(factors.len(), 2);
        assert!(factors.contains(&(factor_a, 1)));
        assert!(factors.contains(&(factor_b, 1)));
    }

    #[test]
    fn computes_monic_gcd_with_exact_quotient() {
        let left = integers(&[2, 1, 2, 1]);
        let right = integers(&[-9, 3, -9, 3]);
        let mut work = Work::new(&|_| true);
        let divisor = gcd(&left, &right, &mut work).unwrap();
        assert_eq!(divisor, integers(&[1, 0, 1]));
        assert_eq!(
            divide_exact(&left, &divisor, &mut work),
            Some(integers(&[2, 1]))
        );
        assert_eq!(
            divide_exact(&right, &divisor, &mut work),
            Some(integers(&[-9, 3]))
        );
        assert_eq!(divide_exact(&left, &right, &mut work), None);
    }

    #[test]
    fn gcd_bounds_coefficient_growth_during_square_free_decomposition() {
        let polynomial = integers(&[480, -438, 565, -515, 243, -123, 36]);
        let mut work = Work::new(&|_| true);
        let (scalar, factors) = factor(&polynomial, &mut work).unwrap();
        assert_eq!(scalar, Rat::new(36, 1).unwrap());
        assert_eq!(factors.len(), 4);
        assert!(factors.contains(&(integers(&[-2, 1]), 1)));
        assert!(factors.contains(&(integers(&[3, 0, 1]), 1)));
    }

    #[test]
    fn primitive_removes_common_numerator_before_clearing_denominators() {
        let huge = i128::MAX - 2;
        let coefficients = [Rat::new(huge, 2).unwrap(), Rat::new(huge, 3).unwrap()];
        assert_eq!(
            primitive(&coefficients, &mut Work::new(&|_| true)),
            Some(vec![3, 2]),
        );
    }

    #[test]
    fn cyclotomic_shortcut_splits_binomials_of_every_degree() {
        // The golden cas-factor cases (GeoGebra with Giac factors these
        // instantly): x^n - 1 for every n up to 30 and x^n + 1 beyond the
        // subset recombination limit of the plain Berlekamp-Zassenhaus path.
        let mut work = Work::new(&|_| true);
        for degree in 2..=30 {
            for plus in [false, true] {
                let coefficients = binomial(degree, plus);
                let (scalar, factors) = factor(&coefficients, &mut work).unwrap();
                assert_eq!(scalar, Rat::ONE, "leading of x^{degree} +/- 1");
                // x^n - 1 counts the divisors of n, x^n + 1 the divisors of
                // 2n that do not divide n (a power of two leaves Phi_2n alone).
                let expected = if plus {
                    (1..=2 * degree)
                        .filter(|d| 2 * degree % d == 0 && degree % d != 0)
                        .count()
                } else {
                    (1..=degree).filter(|d| degree % d == 0).count()
                };
                assert_eq!(factors.len(), expected, "x^{degree} +/- 1");
                for (factor, multiplicity) in &factors {
                    assert_eq!(*multiplicity, 1);
                    assert_eq!(factor.last().copied().unwrap(), Rat::ONE);
                }
                assert_eq!(product_of(&factors), coefficients, "x^{degree} +/- 1");
            }
        }
    }

    #[test]
    fn swinnerton_dyer_polynomials_stay_irreducible_within_budget() {
        // cas-factor-041/042: Swinnerton-Dyer style inputs split into many
        // factors modulo every prime, so no single reduction certifies
        // irreducibility — only the exhaustive, Landau-bounded recombination
        // does. The degree-16 input must not exhaust the work budget either.
        let swinnerton_dyer_16 = integers(&[
            46225, 0, -5596840, 0, 13950764, 0, -7453176, 0, 1513334, 0, -141912, 0, 6476, 0, -136,
            0, 1,
        ]);
        metered!(work, spent);
        let factorization = factor(&swinnerton_dyer_16, &mut work);
        assert!(spent.get() <= 500_000, "spent {}", spent.get());
        assert_eq!(
            factorization,
            Some((Rat::ONE, vec![(swinnerton_dyer_16, 1)])),
            "spent {}",
            spent.get()
        );
        // cas-factor-042: the degree-8 twin, irreducible the same way.
        let swinnerton_dyer_8 = integers(&[576, 0, -960, 0, 352, 0, -40, 0, 1]);
        metered!(work, spent);
        let (scalar, factors) = factor(&swinnerton_dyer_8, &mut work).unwrap();
        assert!(spent.get() <= 500_000, "spent {}", spent.get());
        assert_eq!(
            (scalar, factors),
            (Rat::ONE, vec![(swinnerton_dyer_8.clone(), 1)])
        );
    }

    #[test]
    fn products_of_many_linear_factors_peel_rational_roots_first() {
        // cas-factor-036..040: a product of 16 to 20 rational roots takes the
        // rational-root peeling instead of the subset recombination.
        for degree in [16, 17, 18, 19, 20] {
            let mut polynomial = vec![Rat::ONE];
            for root in 1..=degree as i128 {
                let factor = integers(&[-root, 1]);
                let mut next = vec![Rat::ZERO; polynomial.len() + 1];
                for (i, &a) in polynomial.iter().enumerate() {
                    for (j, &b) in factor.iter().enumerate() {
                        next[i + j] = next[i + j].add(a.mul(b).unwrap()).unwrap();
                    }
                }
                polynomial = next;
            }
            metered!(work, spent);
            let (scalar, factors) = factor(&polynomial, &mut work).unwrap();
            assert!(
                spent.get() <= 500_000,
                "degree {degree} spent {}",
                spent.get()
            );
            assert_eq!(scalar, Rat::ONE);
            assert_eq!(factors.len(), degree, "degree {degree}");
            assert_eq!(product_of(&factors), polynomial);
        }
    }
}
