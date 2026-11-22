//! Lexicographic Gröbner elimination and back-substitution for small systems.

use std::cmp::Ordering;
use std::collections::{btree_map::Entry, BTreeMap};

use super::extended::{isolate_approximate, scaled_residual};
use super::{factor, real_roots, Rat, Root, Roots, Work};

mod quadratic;

const MAX_EQUATIONS: usize = 8;
const MAX_BASIS: usize = 24;
const MAX_TERMS: usize = 256;
const MAX_DEGREE: usize = 16;
const MAX_SOLUTIONS: usize = 256;

pub(crate) type Equation<const N: usize = 3> = BTreeMap<[u8; N], Rat>;

#[derive(Debug, PartialEq)]
pub(crate) enum SystemRoots {
    Infinite,
    Finite(Vec<Vec<Root>>),
}

/// Parameterize an affine family with exactly one free variable. The
/// coordinate functions are returned in ascending coefficient order.
pub(crate) fn affine_system(
    equations: &[Equation],
    width: usize,
    work: &mut Work<'_>,
) -> Option<Vec<Vec<Rat>>> {
    if !(1..=3).contains(&width) || equations.len() > MAX_EQUATIONS {
        return None;
    }
    for equation in equations {
        work.spend(equation.len())?;
        if equation
            .keys()
            .any(|power| degree(power) > 1 || power[width..].iter().any(|&exponent| exponent != 0))
        {
            return None;
        }
    }
    let basis = basis(equations, work)?;
    if basis.len() + 1 != width {
        return None;
    }
    let mut coordinates = [[Rat::ZERO; 2]; 3];
    let mut free = 0;
    for variable in (0..width).rev() {
        work.spend(basis.len())?;
        let mut axis = [0; 3];
        axis[variable] = 1;
        let pivot = basis
            .iter()
            .find(|p| p.last_key_value().is_some_and(|(power, _)| *power == axis));
        if let Some(pivot) = pivot {
            work.spend(pivot.len())?;
            coordinates[variable][0] = pivot.get(&[0; 3]).copied().unwrap_or(Rat::ZERO).neg()?;
            for i in variable + 1..width {
                let mut axis = [0; 3];
                axis[i] = 1;
                if let Some(&coefficient) = pivot.get(&axis) {
                    let source = coordinates[i];
                    for (target, value) in coordinates[variable].iter_mut().zip(source) {
                        *target = target.sub(value.mul(coefficient)?)?;
                    }
                }
            }
        } else {
            coordinates[variable][1] = Rat::ONE;
            free += 1;
        }
    }
    if free != 1 {
        return None;
    }
    Some(
        coordinates[..width]
            .iter()
            .map(|coefficients| {
                let length = if coefficients[1] == Rat::ZERO { 1 } else { 2 };
                coefficients[..length].to_vec()
            })
            .collect(),
    )
}

pub(crate) fn systems(
    equations: &[Equation],
    width: usize,
    work: &mut Work<'_>,
) -> Option<SystemRoots> {
    if !(1..=3).contains(&width) || equations.len() > MAX_EQUATIONS {
        return None;
    }
    for equation in equations {
        work.spend(equation.len())?;
        if equation.len() > MAX_TERMS
            || equation
                .keys()
                .any(|power| degree(power) > MAX_DEGREE || power[width..].iter().any(|&e| e != 0))
        {
            return None;
        }
    }
    let basis = basis(equations, work)?;
    for polynomial in &basis {
        work.spend(polynomial.len())?;
        if let Some(constant) = polynomial.get(&[0; 3]) {
            // Even monomials with the sign of a nonzero constant cannot
            // cancel it anywhere over the reals.
            if polynomial.iter().all(|(power, coefficient)| {
                coefficient.num.signum() == constant.num.signum()
                    && power.iter().all(|exponent| exponent.is_multiple_of(2))
            }) {
                return Some(SystemRoots::Finite(vec![]));
            }
        }
    }
    // A positive-dimensional basis can contain one-variable contradictions
    // over the reals even when it has no nonzero constant.
    for variable in 0..width {
        let mut constraint = Vec::new();
        for polynomial in &basis {
            work.spend(polynomial.len())?;
            if polynomial
                .keys()
                .all(|power| degree(power) == usize::from(power[variable]))
            {
                let coefficients =
                    exact_specialization(polynomial, variable, &[Rat::ZERO; 3], work)?;
                constraint = factor::gcd(&constraint, &coefficients, work)?;
            }
        }
        if matches!(real_roots(&constraint, work)?, Roots::Finite(roots) if roots.is_empty()) {
            return Some(SystemRoots::Finite(vec![]));
        }
    }
    let mut results = Vec::new();
    let mut row = [Root::Rational(Rat::ZERO); 3];
    if !back_substitute(&basis, width, width, &mut row, &mut results, work)? {
        return Some(SystemRoots::Infinite);
    }
    let levels = results.len().checked_ilog2().unwrap_or(0) as usize + 1;
    work.spend(width * results.len() * levels)?;
    results.sort_by(|left: &Vec<Root>, right| {
        left.iter()
            .zip(right)
            .map(|(a, b)| a.value().total_cmp(&b.value()))
            .find(|&order| order != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
    Some(SystemRoots::Finite(results))
}

fn degree<const N: usize>(power: &[u8; N]) -> usize {
    power.iter().map(|&e| usize::from(e)).sum()
}

pub(crate) fn basis<const N: usize>(
    equations: &[Equation<N>],
    work: &mut Work<'_>,
) -> Option<Vec<Equation<N>>> {
    if equations.len() > MAX_EQUATIONS {
        return None;
    }
    let mut basis = Vec::new();
    for polynomial in equations {
        let polynomial = polynomial
            .iter()
            .filter(|(_, a)| **a != Rat::ZERO)
            .map(|(&e, &a)| (e, a))
            .collect();
        if let Some(constant) = insert_reduced(polynomial, &mut basis, work)? {
            return Some(vec![constant]);
        }
    }
    // Buchberger's algorithm (https://doi.org/10.4249/scholarpedia.7764).
    // Each newly appended polynomial creates pairs
    // with all earlier basis elements. Coprime leading monomials need no pair.
    let mut right = 1;
    while right < basis.len() {
        for left in 0..right {
            work.spend(1)?;
            if let Some(s) = s_polynomial(&basis[left], &basis[right], work)? {
                if let Some(constant) = insert_reduced(s, &mut basis, work)? {
                    return Some(vec![constant]);
                }
            }
        }
        right += 1;
    }
    Some(basis)
}

fn insert_reduced<const N: usize>(
    mut pending: Equation<N>,
    basis: &mut Vec<Equation<N>>,
    work: &mut Work<'_>,
) -> Option<Option<Equation<N>>> {
    let mut remainder = Equation::new();
    while let Some((&power, &coefficient)) = pending.last_key_value() {
        work.spend(basis.len() + 1)?;
        let reducible = basis.iter().find_map(|divisor| {
            let (&leading, &value) = divisor.last_key_value()?;
            Some((divisor, difference(power, leading)?, value))
        });
        if let Some((divisor, shift, value)) = reducible {
            add_shifted(
                &mut pending,
                divisor,
                shift,
                coefficient.neg()?.div(value)?,
                work,
            )?;
        } else {
            pending.pop_last();
            remainder.insert(power, coefficient);
        }
    }
    if let Some((&power, &leading)) = remainder.last_key_value() {
        if power == [0; N] {
            return Some(Some(remainder));
        }
        if basis.len() == MAX_BASIS {
            return None;
        }
        work.spend(remainder.len())?;
        for coefficient in remainder.values_mut() {
            *coefficient = coefficient.div(leading)?;
        }
        basis.push(remainder);
    }
    Some(None)
}

fn s_polynomial<const N: usize>(
    left: &Equation<N>,
    right: &Equation<N>,
    work: &mut Work<'_>,
) -> Option<Option<Equation<N>>> {
    let (&a, _) = left.last_key_value()?;
    let (&b, _) = right.last_key_value()?;
    if (0..N).all(|i| a[i] == 0 || b[i] == 0) {
        return Some(None);
    }
    let lcm = std::array::from_fn(|i| a[i].max(b[i]));
    let mut result = Equation::new();
    add_shifted(&mut result, left, difference(lcm, a)?, Rat::ONE, work)?;
    add_shifted(
        &mut result,
        right,
        difference(lcm, b)?,
        Rat::ONE.neg()?,
        work,
    )?;
    Some(Some(result))
}

fn difference<const N: usize>(mut power: [u8; N], subtract: [u8; N]) -> Option<[u8; N]> {
    for (a, b) in power.iter_mut().zip(subtract) {
        *a = a.checked_sub(b)?;
    }
    Some(power)
}

pub(crate) fn add_shifted<const N: usize>(
    target: &mut Equation<N>,
    source: &Equation<N>,
    shift: [u8; N],
    scale: Rat,
    work: &mut Work<'_>,
) -> Option<()> {
    work.spend(source.len())?;
    for (&power, &coefficient) in source {
        let power = std::array::from_fn(|i| power[i] + shift[i]);
        if degree(&power) > MAX_DEGREE {
            return None;
        }
        let coefficient = coefficient.mul(scale)?;
        if coefficient == Rat::ZERO {
            continue;
        }
        match target.entry(power) {
            Entry::Vacant(slot) => {
                slot.insert(coefficient);
            }
            Entry::Occupied(mut slot) => {
                let sum = slot.get().add(coefficient)?;
                if sum == Rat::ZERO {
                    slot.remove();
                } else {
                    slot.insert(sum);
                }
            }
        }
        if target.len() > MAX_TERMS {
            return None;
        }
    }
    Some(())
}

fn back_substitute(
    basis: &[Equation],
    pending: usize,
    width: usize,
    row: &mut [Root; 3],
    results: &mut Vec<Vec<Root>>,
    work: &mut Work<'_>,
) -> Option<bool> {
    let Some(variable) = pending.checked_sub(1) else {
        if results.len() == MAX_SOLUTIONS {
            return None;
        }
        work.spend(width)?;
        results.push(row[..width].to_vec());
        return Some(true);
    };
    work.spend(basis.len())?;
    let relevant: Vec<_> = basis
        .iter()
        .filter(|polynomial| {
            polynomial
                .last_key_value()
                .is_some_and(|(power, _)| power[..variable].iter().all(|&a| a == 0))
        })
        .collect();
    let mut exact_row = [Rat::ZERO; 3];
    let rational = row[pending..width].iter().enumerate().all(|(i, &root)| {
        if let Root::Rational(coefficient) = root {
            exact_row[pending + i] = coefficient;
            true
        } else {
            false
        }
    });
    let roots = if rational {
        let mut constraint = Vec::new();
        for polynomial in relevant {
            let specialized = exact_specialization(polynomial, variable, &exact_row, work)?;
            constraint = factor::gcd(&constraint, &specialized, work)?;
        }
        real_roots(&constraint, work)?
    } else {
        numeric_specialization(&relevant, variable, *row, work)?
    };
    let Roots::Finite(roots) = roots else {
        return Some(false);
    };
    for root in roots {
        row[variable] = root;
        if !back_substitute(basis, variable, width, row, results, work)? {
            return Some(false);
        }
    }
    Some(true)
}

fn exact_specialization(
    polynomial: &Equation,
    variable: usize,
    row: &[Rat; 3],
    work: &mut Work<'_>,
) -> Option<Vec<Rat>> {
    let mut coefficients = vec![Rat::ZERO; MAX_DEGREE + 1];
    for (power, &coefficient) in polynomial {
        work.spend(degree(power) + 1)?;
        let mut contribution = coefficient;
        for i in variable + 1..3 {
            for _ in 0..power[i] {
                contribution = contribution.mul(row[i])?;
            }
        }
        let slot = &mut coefficients[usize::from(power[variable])];
        *slot = slot.add(contribution)?;
    }
    Some(coefficients)
}

fn numeric_specialization(
    polynomials: &[&Equation],
    variable: usize,
    row: [Root; 3],
    work: &mut Work<'_>,
) -> Option<Roots> {
    let mut constraints = Vec::new();
    let values = row.map(Root::value);
    for polynomial in polynomials {
        let mut coefficients = [0.0; MAX_DEGREE + 1];
        let mut envelopes = [0.0; MAX_DEGREE + 1];
        for (power, &coefficient) in *polynomial {
            work.spend(degree(power) + 1)?;
            let mut term = coefficient.to_f64();
            for i in variable + 1..3 {
                term *= values[i].powi(i32::from(power[i]));
            }
            let slot = usize::from(power[variable]);
            coefficients[slot] += term;
            envelopes[slot] += term.abs();
        }
        for (coefficient, envelope) in coefficients.iter_mut().zip(envelopes) {
            if !coefficient.is_finite() || !envelope.is_finite() {
                return None;
            }
            if coefficient.abs() <= 32.0 * f64::EPSILON * envelope {
                *coefficient = 0.0;
            }
        }
        if let Some(index) = coefficients.iter().rposition(|&a| a != 0.0) {
            constraints.push(coefficients[..=index].to_vec());
        }
    }
    let Some(shortest) = constraints.iter().min_by_key(|p| p.len()) else {
        return Some(Roots::All);
    };
    let candidates = if shortest.len() == 2 {
        if let Some(root) = quadratic::linear_solution(polynomials, variable, row, work) {
            vec![root]
        } else {
            isolate_approximate(shortest, true, work)?
                .into_iter()
                .map(Root::Numeric)
                .collect()
        }
    } else {
        isolate_approximate(shortest, true, work)?
            .into_iter()
            .map(Root::Numeric)
            .collect()
    };
    let mut roots = Vec::new();
    for root in candidates {
        let mut passes = true;
        for polynomial in &constraints {
            work.spend(polynomial.len())?;
            let (residual, envelope) = scaled_residual(polynomial, root.value());
            passes &= residual.abs() <= 1e-11 * envelope;
        }
        if passes {
            roots.push(root);
        }
    }
    Some(Roots::Finite(roots))
}
