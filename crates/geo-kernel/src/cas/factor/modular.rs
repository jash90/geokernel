//! Berlekamp–Zassenhaus factorization of a square-free rational polynomial.

mod field;

use super::{integer_gcd, normalize_integers, primitive, Rat, Work};
use field::Field;

const PRIMES: [i128; 25] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
];

type IntegerFactors = Vec<Vec<i128>>;

/// Reconstruct the primitive GCD by the Chinese remainder theorem. A modular
/// GCD bounds the rational GCD's degree above. An exact common divisor of that
/// degree certifies reconstruction at the current precision.
pub(super) fn gcd(left: &[i128], right: &[i128], work: &mut Work<'_>) -> Option<Option<Vec<i128>>> {
    let left_leading = *left.last()?;
    let right_leading = *right.last()?;
    let leading = integer_gcd(left_leading as u128, right_leading as u128) as i128;
    let mut modulus = 1_i128;
    let mut degree_bound = left.len().min(right.len());
    let mut residues = Vec::new();
    for prime in PRIMES {
        if left_leading % prime == 0 || right_leading % prime == 0 {
            continue;
        }
        work.spend(left.len() + right.len())?;
        let reduce = |polynomial: &[i128]| {
            polynomial
                .iter()
                .map(|term| term.rem_euclid(prime))
                .collect()
        };
        let field = Field(prime);
        let divisor = field.gcd(reduce(left), reduce(right), work)?;
        let degree = divisor.len() - 1;
        if degree == 0 {
            return Some(Some(vec![1]));
        }
        if degree > degree_bound {
            continue;
        }
        if degree < degree_bound {
            degree_bound = degree;
            modulus = 1;
            residues.resize(degree, 0);
            residues.fill(0);
        }
        let next_modulus = modulus.checked_mul(prime)?;
        let inverse = field.inverse(modulus % prime);
        for (residue, term) in residues.iter_mut().zip(divisor) {
            let reduced = leading % prime * term % prime;
            let digit = ((reduced - *residue % prime) * inverse).rem_euclid(prime);
            *residue += modulus * digit;
        }
        modulus = next_modulus;
        let candidate = residues
            .iter()
            .map(|&residue| center(residue, modulus))
            .chain([leading])
            .collect();
        let candidate = normalize_integers(candidate)?;
        if integer_divides(left, &candidate, work) == Some(true)
            && integer_divides(right, &candidate, work) == Some(true)
        {
            return Some(Some(candidate));
        }
    }
    Some(None)
}

fn center(residue: i128, modulus: i128) -> i128 {
    if residue > modulus / 2 {
        residue - modulus
    } else {
        residue
    }
}

fn add_mod(left: i128, right: i128, modulus: i128) -> i128 {
    if left >= modulus - right {
        left - (modulus - right)
    } else {
        left + right
    }
}

fn multiply_mod(left: i128, right: i128, modulus: i128, work: &mut Work<'_>) -> Option<i128> {
    if let Some(product) = left.checked_mul(right) {
        return Some(product % modulus);
    }
    // The unsigned sum of two residues below a signed modulus fits in u128.
    let modulus = modulus as u128;
    let mut left = left as u128;
    let mut right = right as u128;
    work.spend((u128::BITS - right.leading_zeros()) as usize)?;
    let mut product = 0;
    while right != 0 {
        if right & 1 != 0 {
            product = (product + left) % modulus;
        }
        left = (left + left) % modulus;
        right >>= 1;
    }
    Some(product as i128)
}

fn integer_divides(dividend: &[i128], divisor: &[i128], work: &mut Work<'_>) -> Option<bool> {
    let lead = *divisor.last()?;
    let steps = dividend.len() - divisor.len() + 1;
    work.spend(steps * divisor.len())?;
    let mut remainder = dividend.to_vec();
    for shift in (0..steps).rev() {
        let end = shift + divisor.len() - 1;
        if remainder[end] % lead != 0 {
            return Some(false);
        }
        let quotient = remainder[end] / lead;
        for (value, &term) in remainder[shift..end].iter_mut().zip(divisor) {
            // An exact division keeps the running remainder within the
            // quotient's coefficients, far below `i128`; only a non-divisor
            // explodes, which says "does not divide" rather than a limit.
            let Some(product) = quotient.checked_mul(term) else {
                return Some(false);
            };
            let Some(difference) = value.checked_sub(product) else {
                return Some(false);
            };
            *value = difference;
        }
    }
    Some(remainder[..divisor.len() - 1].iter().all(|&x| x == 0))
}

fn square_free_reduction(
    polynomial: &[i128],
    field: Field,
    work: &mut Work<'_>,
) -> Option<Option<Vec<i128>>> {
    work.spend(polynomial.len())?;
    let prime = field.0;
    let lead = polynomial.last()? % prime;
    if lead == 0 {
        return Some(None);
    }
    let inverse = field.inverse(lead);
    let reduced: Vec<_> = polynomial
        .iter()
        .map(|&coefficient| coefficient.rem_euclid(prime) * inverse % prime)
        .collect();
    let derivative = reduced
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, coefficient)| (i as i128 * coefficient) % prime)
        .collect();
    Some((field.gcd(reduced.clone(), derivative, work)?.len() == 1).then_some(reduced))
}

/// The reduction to lift from: `None` when some prime proves the polynomial
/// irreducible (an irreducible reduction can only come from an irreducible
/// integer polynomial, by Gauss's lemma).
enum Reduction {
    Irreducible,
    Factors(Field, IntegerFactors),
}

/// How many primes to factor completely before settling for the one with the
/// fewest factors. Recombination from any complete modular factorization
/// reconstructs every rational factor, so more primes only lower the cost of
/// the subset search — never its outcome.
const SCANNED_PRIMES: usize = 8;

fn best_reduction(polynomial: &[i128], work: &mut Work<'_>) -> Option<Reduction> {
    let mut best = None;
    let mut fewest_factors = polynomial.len();
    let mut scanned = 0;
    for prime in PRIMES {
        if scanned == SCANNED_PRIMES {
            break;
        }
        let field = Field(prime);
        let Some(reduced) = square_free_reduction(polynomial, field, work)? else {
            continue;
        };
        scanned += 1;
        let factors = field.irreducible_factors(reduced, work)?;
        if factors.len() == 1 {
            return Some(Reduction::Irreducible);
        }
        if factors.len() <= 2 {
            // At most one partition into two nonconstant factors.
            return Some(Reduction::Factors(field, factors));
        }
        if factors.len() <= fewest_factors {
            // Among equals keep the later, larger prime: fewer Hensel steps
            // reach the reconstruction bound.
            fewest_factors = factors.len();
            best = Some(Reduction::Factors(field, factors));
        }
    }
    best
}

struct Lift<'a> {
    polynomial: &'a [i128],
    field: Field,
    g_mod_p: Vec<i128>,
    h_mod_p: Vec<i128>,
    inverse_h: Vec<i128>,
}

impl Lift<'_> {
    fn discrepancy(
        &self,
        g: &[i128],
        h: &[i128],
        modulus: i128,
        next: i128,
        work: &mut Work<'_>,
    ) -> Option<Vec<i128>> {
        let mut product = vec![0; self.polynomial.len()];
        work.spend(g.len() * h.len())?;
        for (i, &left) in g.iter().enumerate() {
            for (term, &right) in product[i..].iter_mut().zip(h) {
                let partial = multiply_mod(left, right, next, work)?;
                *term = add_mod(*term, partial, next);
            }
        }
        let lead = self.polynomial.last()?.rem_euclid(next);
        let prime = self.field.0;
        let inverse = self.field.inverse(lead % prime);
        let mut difference = Vec::with_capacity(product.len());
        for (original, value) in self.polynomial.iter().zip(product) {
            let scaled = multiply_mod(lead, value, next, work)?;
            let error = (original.rem_euclid(next) - scaled).rem_euclid(next);
            debug_assert_eq!(error % modulus, 0);
            difference.push((error / modulus * inverse) % prime);
        }
        while difference.last() == Some(&0) {
            difference.pop();
        }
        Some(difference)
    }

    fn step(
        &self,
        g: &mut [i128],
        h: &mut [i128],
        modulus: i128,
        next: i128,
        work: &mut Work<'_>,
    ) -> Option<()> {
        let field = self.field;
        let error = self.discrepancy(g, h, modulus, next, work)?;
        let product = field.multiply(&error, &self.inverse_h, work)?;
        let (_, correction_g) = field.divide(product, &self.g_mod_p, work)?;
        let product = field.multiply(&correction_g, &self.h_mod_p, work)?;
        let mut remainder = error;
        remainder.resize(remainder.len().max(product.len()), 0);
        for (left, right) in remainder.iter_mut().zip(product) {
            *left = (*left - right).rem_euclid(field.0);
        }
        let (correction_h, rest) = field.divide(remainder, &self.g_mod_p, work)?;
        debug_assert!(rest.is_empty());
        debug_assert!(correction_g.len() < g.len() && correction_h.len() < h.len());
        for (coefficients, correction) in [(g, correction_g), (h, correction_h)] {
            for (coefficient, digit) in coefficients.iter_mut().zip(correction) {
                *coefficient += digit * modulus;
            }
        }
        Some(())
    }

    fn candidate(&self, g: &[i128], modulus: i128, work: &mut Work<'_>) -> Option<Vec<i128>> {
        let leading = *self.polynomial.last()?;
        let scale = leading % modulus;
        let mut candidate = Vec::with_capacity(g.len());
        work.spend(g.len())?;
        for &coefficient in &g[..g.len() - 1] {
            let residue = multiply_mod(scale, coefficient, modulus, work)?;
            candidate.push(center(residue, modulus));
        }
        candidate.push(leading);
        normalize_integers(candidate)
    }

    /// Linear Hensel lift for a coprime pair. Try exact division at each
    /// precision, including before the reconstruction bound is reached.
    /// https://doi.org/10.1016/0022-314X(69)90047-X
    fn reconstruct(&self, work: &mut Work<'_>) -> Option<Option<Vec<i128>>> {
        let mut g = self.g_mod_p.clone();
        let mut h = self.h_mod_p.clone();
        let mut modulus = self.field.0;
        // For integral factors f = uv, every coefficient of lc(v)u is
        // at most 2^deg(u) times the L1 norm of f, by the Landau–Mignotte bound.
        // https://doi.org/10.2307/2005373
        let norm = self
            .polynomial
            .iter()
            .try_fold(0_u128, |sum, term| sum.checked_add(term.unsigned_abs()));
        let twice_bound = norm.and_then(|norm| norm.checked_mul(1_u128 << g.len()));
        loop {
            let candidate = self.candidate(&g, modulus, work)?;
            let divides = integer_divides(self.polynomial, &candidate, work);
            if divides == Some(true) {
                return Some(Some(candidate));
            }
            if twice_bound.is_some_and(|bound| modulus as u128 > bound) {
                return divides.map(|_| None);
            }
            let next = modulus.checked_mul(self.field.0)?;
            self.step(&mut g, &mut h, modulus, next, work)?;
            modulus = next;
        }
    }
}

fn lift_factor(
    polynomial: &[i128],
    field: Field,
    g_mod_p: Vec<i128>,
    h_mod_p: Vec<i128>,
    work: &mut Work<'_>,
) -> Option<Option<Vec<i128>>> {
    let inverse_h = field.polynomial_inverse(&h_mod_p, &g_mod_p, work)?;
    Lift {
        polynomial,
        field,
        g_mod_p,
        h_mod_p,
        inverse_h,
    }
    .reconstruct(work)
}

/// The constant coefficients a subset product may carry: an integer factor
/// takes a constant term dividing the polynomial's and a leading coefficient
/// dividing its, so the subset's constant residue must match `d * e^-1` for
/// divisors `d` and `e` of the two, up to sign. `None` when a term is too
/// large to enumerate divisors of.
fn allowed_constants(polynomial: &[i128], field: Field, work: &mut Work<'_>) -> Option<Vec<i128>> {
    let constant = polynomial.first().copied()?.checked_abs()?;
    let leading = polynomial.last().copied()?.checked_abs()?;
    if constant == 0 || leading > 1_000_000_000 {
        return None;
    }
    let mut divisors = |n: i128| -> Option<Vec<i128>> {
        if n > 100_000_000 {
            return None;
        }
        let mut small = vec![1_i128];
        let mut d = 2_i128;
        while d.checked_mul(d).is_some_and(|square| square <= n) {
            work.spend(1)?;
            if n % d == 0 {
                small.push(d);
            }
            d += 1;
        }
        let mut all = Vec::with_capacity(2 * small.len());
        for e in small {
            all.push(e);
            all.push(n.checked_div(e)?);
        }
        all.sort();
        all.dedup();
        Some(all)
    };
    let divisors_of_constant = divisors(constant)?;
    let mut allowed = Vec::new();
    for &e in divisors(leading)?.iter() {
        let inverse = field.inverse(e.rem_euclid(field.0));
        for &d in divisors_of_constant.iter() {
            for sign in [1_i128, -1_i128] {
                allowed.push((sign * (d % field.0) * inverse).rem_euclid(field.0));
            }
        }
    }
    allowed.sort_unstable();
    allowed.dedup();
    Some(allowed)
}

fn partitions(
    polynomial: &[i128],
    field: Field,
    factors: &[Vec<i128>],
    work: &mut Work<'_>,
) -> Option<Option<Vec<i128>>> {
    let degree = polynomial.len() - 1;
    let allowed = allowed_constants(polynomial, field, work);
    for mask in 1..(1_usize << factors.len()) - 1 {
        work.spend(factors.len())?;
        let chosen = factors
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .collect::<Vec<_>>();
        let left_degree = chosen
            .iter()
            .map(|(_, factor)| factor.len() - 1)
            .sum::<usize>();
        if 2 * left_degree > degree || (2 * left_degree == degree && mask & 1 == 0) {
            continue;
        }
        if let Some(allowed) = &allowed {
            // A leading coefficient of the candidate divides the polynomial's,
            // so the chosen constants may only combine to `d / e` above.
            let mut residue = 1_i128;
            for (_, factor) in &chosen {
                residue = residue * factor[0] % field.0;
            }
            if allowed.binary_search(&residue).is_err() {
                continue;
            }
        }
        let mut pair = [vec![1], vec![1]];
        for (i, factor) in factors.iter().enumerate() {
            let index = usize::from(mask & (1 << i) == 0);
            pair[index] = field.multiply(&pair[index], factor, work)?;
        }
        let [g_mod_p, h_mod_p] = pair;
        if let Some(factor) = lift_factor(polynomial, field, g_mod_p, h_mod_p, work)? {
            return Some(Some(factor));
        }
    }
    Some(None)
}

fn quadratic_roots(polynomial: &[i128]) -> Option<Vec<Rat>> {
    let &[c, b, a] = polynomial else {
        return None;
    };
    let discriminant = b
        .checked_mul(b)?
        .checked_sub(a.checked_mul(c)?.checked_mul(4)?)?;
    if discriminant < 0 {
        return Some(vec![]);
    }
    let square_root = discriminant.isqrt();
    if square_root * square_root != discriminant {
        return Some(vec![]);
    }
    let denominator = a.checked_mul(2)?;
    let first = Rat::new(b.checked_neg()?.checked_sub(square_root)?, denominator)?;
    if square_root == 0 {
        return Some(vec![first]);
    }
    let second = Rat::new(b.checked_neg()?.checked_add(square_root)?, denominator)?;
    Some(vec![first, second])
}

pub(super) fn rational_roots(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Vec<Rat>> {
    if let &[constant, leading] = coefficients {
        return Some(vec![constant.neg()?.div(leading)?]);
    }
    let polynomial = primitive(coefficients, work)?;
    if let Some(roots) = quadratic_roots(&polynomial) {
        return Some(roots);
    }
    for prime in PRIMES {
        let field = Field(prime);
        let Some(reduced) = square_free_reduction(&polynomial, field, work)? else {
            continue;
        };
        work.spend(reduced.len() * prime as usize)?;
        let mut roots = Vec::new();
        for value in 0..prime {
            let residue = reduced
                .iter()
                .rev()
                .fold(0, |sum, &term| (sum * value + term) % prime);
            if residue == 0 {
                let linear = vec![(-value).rem_euclid(prime), 1];
                let (other, _) = field.divide(reduced.clone(), &linear, work)?;
                if let Some(factor) = lift_factor(&polynomial, field, linear, other, work)? {
                    roots.push(Rat::new(factor[0].checked_neg()?, factor[1])?);
                }
            }
        }
        return Some(roots);
    }
    None
}

/// Inner `None` certifies irreducibility. Outer `None` indicates a work or
/// arithmetic limit, or no suitable small prime.
pub(super) fn proper_factor(coefficients: &[Rat], work: &mut Work<'_>) -> Option<Option<Vec<Rat>>> {
    if coefficients.len() <= 2 {
        return Some(None);
    }
    if coefficients[0] == Rat::ZERO {
        return Some(Some(vec![Rat::ZERO, Rat::ONE]));
    }
    let polynomial = primitive(coefficients, work)?;
    if let Some(roots) = quadratic_roots(&polynomial) {
        return Some(match roots.first() {
            Some(root) => Some(vec![root.neg()?, Rat::ONE]),
            None => None,
        });
    }
    // Rational roots peel off by single linear lifts, long before the modular
    // factorization: a product of many linear factors would otherwise blow
    // the budget in the subset recombination.
    let roots = rational_roots(coefficients, work)?;
    if let Some(&root) = roots.first() {
        return Some(Some(vec![root.neg()?, Rat::ONE]));
    }
    let leading = *polynomial.last()?;
    let lower = &polynomial[..polynomial.len() - 1];
    for prime in PRIMES {
        work.spend(lower.len())?;
        if leading % prime != 0
            && lower[0] % (prime * prime) != 0
            && lower.iter().all(|coefficient| coefficient % prime == 0)
        {
            // Eisenstein's criterion avoids lifting sparse irreducible inputs.
            return Some(None);
        }
    }
    let factor = match best_reduction(&polynomial, work)? {
        Reduction::Irreducible => return Some(None),
        Reduction::Factors(field, factors) => {
            let Some(factor) = partitions(&polynomial, field, &factors, work)? else {
                return Some(None);
            };
            factor
        }
    };
    let factor_lead = *factor.last()?;
    Some(Some(
        factor
            .into_iter()
            .map(|term| Rat::new(term, factor_lead))
            .collect::<Option<Vec<_>>>()?,
    ))
}
