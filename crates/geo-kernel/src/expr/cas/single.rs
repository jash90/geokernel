// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoSolve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSolve.java
//   source/shared/common/src/main/java/org/geogebra/common/cas/giac/Ggb2giac.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! One equation solved for one variable: `Solve(f, x)`, `Solutions(g, y)`,
//! `NSolve(f, x = 1)`, `PlotSolve(k)`. An equation in the solving variable
//! alone gives its real roots; one that holds a second variable gives the
//! solving variable as a function of it, as long as that takes no more
//! than one square root.

use super::solutions::{exact, identity};
use super::{Poly, Quotient, Rat, Slot, Work};
use crate::cas::{
    guided_root, polynomial_gcd, polynomial_quotient, real_roots, Algebraic, Radical, Root, Roots,
};
use crate::expr::eval::Ctx;
use crate::{Kind, Value};

const NAMES: [&str; 3] = ["x", "y", "z"];

/// How the solutions of a single equation are nested.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Nesting {
    /// `{a, b}`.
    Flat,
    /// `{{a, b}}`: the assignments of a system in one variable.
    OneRow,
    /// `{{a}, {b}}`: one row per solution.
    Rows,
}

#[derive(Clone, Copy)]
pub(super) struct Mode {
    /// `Solve`, `NSolve`: equations `x = ...` in place of values.
    pub(super) assignments: bool,
    /// `NSolve`, `NSolutions`.
    pub(super) approximate: bool,
    /// `PlotSolve`: points `(value, 0)`.
    pub(super) points: bool,
    pub(super) nesting: Nesting,
}

/// Solves `equation = 0` for the variable `hint`, or without one for `x`
/// when the equation holds it and for its first variable otherwise
/// (`Solutions.1`: `when(contains(lname(%0), x), x, lname(%0)[0])`).
pub(super) fn solve<const M: bool>(
    ctx: &Ctx<'_, M>,
    equation: Quotient,
    hint: Option<u8>,
    mode: Mode,
    guess: Option<f64>,
    work: &mut Work<'_>,
) -> Option<(u32, u32)> {
    let used: Vec<usize> = (0..3)
        .filter(|&j| {
            equation
                .numerator
                .0
                .keys()
                .chain(equation.denominator.0.keys())
                .any(|p| p[j] != 0)
        })
        .collect();
    let variable = match hint {
        Some(axis) => usize::from(axis),
        None if used.contains(&0) => 0,
        None => used.first().copied().unwrap_or(0),
    };
    let others: Vec<usize> = used.iter().copied().filter(|&j| j != variable).collect();
    if others.is_empty() {
        return univariate(ctx, equation, variable, variable as u8, mode, guess, work);
    }
    // With an initial value every command but `Solve` runs the numeric
    // solver, which leaves a second variable unresolved.
    if mode.approximate || (guess.is_some() && !mode.assignments) {
        return None;
    }
    let branches = if used.contains(&variable) {
        let [parameter] = others[..] else {
            return None;
        };
        equation.denominator.scalar()?;
        parametric(&equation.numerator, variable, parameter, work)?
            .into_iter()
            .map(|branch| branch.slot(variable, parameter, mode))
            .collect::<Option<Vec<_>>>()?
    } else {
        // The equation does not hold the variable: no solution.
        Vec::new()
    };
    emit(ctx, branches, mode)
}

/// The real roots of an equation in the variable `axis` alone, as numbers or
/// as assignments to the coordinate `coordinate`.
pub(super) fn univariate<const M: bool>(
    ctx: &Ctx<'_, M>,
    equation: Quotient,
    axis: usize,
    coordinate: u8,
    mode: Mode,
    guess: Option<f64>,
    work: &mut Work<'_>,
) -> Option<(u32, u32)> {
    let mut coefficients = dense_at(&equation.numerator, axis);
    let mut denominator = dense_at(&equation.denominator, axis);
    let guided = guess.filter(|_| mode.approximate || !mode.assignments);
    if (mode.approximate || guided.is_some()) && denominator.len() > 1 {
        // The numeric path simplifies the equation first, so a root the
        // numerator and the denominator share survives: NSolutions(x^2/x)
        // answers {0}, while Solutions(x^2/x = 0) answers {}.
        if let Some(common) = polynomial_gcd(&coefficients, &denominator, work) {
            if common.len() > 1 {
                coefficients = polynomial_quotient(&coefficients, &common, work)?;
                denominator = polynomial_quotient(&denominator, &common, work)?;
            }
        }
    }
    if coefficients.len() == 1 {
        // At this entry point GeoGebra treats constant false equations as
        // Boolean input and reports an undefined list.
        return None;
    }
    if let Some(start) = guided {
        // `fsolve(equation, variable = start)` answers one root.
        let root = Root::Numeric(guided_root(&coefficients, start, work)?);
        let slot = solution(root, mode.assignments, true, coordinate);
        return emit(ctx, vec![slot], mode);
    }
    let slots = match real_roots(&coefficients, work)? {
        Roots::All if denominator.len() > 1 => return None,
        // `(x, 0)` is the parametric form of the x-axis.
        Roots::All if mode.points => vec![Slot::line(Value::Line {
            a: 0.0,
            b: 1.0,
            c: 0.0,
        })],
        Roots::All if mode.assignments => vec![identity(coordinate)],
        Roots::All => vec![Slot::polynomial(Poly::variable(0))],
        Roots::Finite(roots) => roots
            .into_iter()
            .filter(|r| !is_pole(*r, &denominator))
            .map(|r| {
                if mode.points {
                    Slot::coords(Kind::Point, r.value(), 0.0)
                } else {
                    solution(r, mode.assignments, mode.approximate, coordinate)
                }
            })
            .collect(),
    };
    emit(ctx, slots, mode)
}

fn emit<const M: bool>(ctx: &Ctx<'_, M>, slots: Vec<Slot>, mode: Mode) -> Option<(u32, u32)> {
    if mode.points && slots.is_empty() {
        // `PlotSolve.1` reads the first solution: none is an error.
        return None;
    }
    if mode.nesting == Nesting::Flat || slots.is_empty() {
        return ctx.cas_slots(slots.into_iter());
    }
    let lists = ctx.lists?;
    if mode.nesting == Nesting::OneRow {
        let (inner, len) = ctx.cas_slots(slots.into_iter())?;
        return ctx.cas_slots(std::iter::once(Slot::list(inner, len)));
    }
    let count = slots.len().try_into().ok()?;
    let start = lists.reserve(count)?;
    for (i, slot) in slots.into_iter().enumerate() {
        let (inner, len) = ctx.cas_slots(std::iter::once(slot))?;
        lists.set(start + i as u32, Slot::list(inner, len));
    }
    Some((start, count))
}

/// The solving variable as a function of the parameter.
#[derive(Debug, PartialEq)]
enum Branch {
    /// A polynomial with rational coefficients, ascending.
    Polynomial(Vec<Rat>),
    /// A pole, a square root or an irrational coefficient.
    Function(Algebraic),
}

impl Branch {
    /// `[constant, slope]` of a branch of degree at most one.
    fn linear(&self) -> Option<[f64; 2]> {
        match self {
            Self::Polynomial(c) if c.len() <= 2 => Some([
                c.first().map_or(0.0, |c| c.to_f64()),
                c.get(1).map_or(0.0, |c| c.to_f64()),
            ]),
            Self::Polynomial(_) => None,
            Self::Function(f) => f.linear(),
        }
    }

    /// The rational slope of a branch of degree at most one.
    fn slope(&self) -> Option<Rat> {
        match self {
            Self::Polynomial(c) if c.len() <= 2 => Some(c.get(1).copied().unwrap_or(Rat::ZERO)),
            _ => None,
        }
    }

    fn slot(self, variable: usize, parameter: usize, mode: Mode) -> Option<Slot> {
        let linear = self.linear();
        if mode.points {
            // `(re(value), im(value))` with the parameter left in is the
            // parametric line through `(constant, 0)` along `(slope, 0)`.
            return match linear? {
                [constant, 0.0] => Some(Slot::coords(Kind::Point, constant, 0.0)),
                [_, slope] => Some(Slot::line(Value::Line {
                    a: 0.0,
                    b: slope,
                    c: 0.0,
                })),
            };
        }
        if !mode.assignments {
            return Some(match self {
                Self::Polynomial(c) if c.len() <= 1 => {
                    let value = c.first().copied().unwrap_or(Rat::ZERO);
                    solution(Root::Rational(value), false, false, variable as u8)
                }
                Self::Polynomial(c) => Slot::polynomial(Poly::dense(&c)),
                Self::Function(f) => Slot::algebraic(f, NAMES[parameter]),
            });
        }
        if variable > 1 || parameter > 1 {
            return None;
        }
        if let Some([constant, slope]) = linear {
            // `x = slope y + constant` or `y = slope x + constant`.
            let (a, b) = if variable == 0 {
                (1.0, -slope)
            } else {
                (-slope, 1.0)
            };
            let slot = Slot::line(Value::Line { a, b, c: -constant });
            return Some(match &self {
                Self::Polynomial(c) if c.len() <= 1 => {
                    let value = c.first().copied().unwrap_or(Rat::ZERO);
                    solution(Root::Rational(value), true, false, variable as u8)
                }
                _ => slot,
            });
        }
        // `y = f(x)` is a function; `y = a x^2 + b x + c` is a conic and
        // `x = f(y)` an implicit curve, which a list cannot hold.
        match self {
            Self::Polynomial(c) if variable == 1 && c.len() > 3 => {
                Some(Slot::polynomial(Poly::dense(&c)))
            }
            Self::Function(f) if variable == 1 => Some(Slot::algebraic(f, NAMES[parameter])),
            _ => None,
        }
    }
}

/// The branches of `equation = 0` solved for `variable`, with `parameter`
/// free, in the order GeoGebra lists them.
fn parametric(
    equation: &Poly,
    variable: usize,
    parameter: usize,
    work: &mut Work<'_>,
) -> Option<Vec<Branch>> {
    // The coefficients of the powers of the variable, polynomials in the
    // parameter, without their common factor: `x y = 0` solved for `x`
    // leaves `x = 0`.
    let mut powers: Vec<Vec<Rat>> = Vec::new();
    for (exponents, &coefficient) in &equation.0 {
        work.spend(1)?;
        let (k, j) = (
            usize::from(exponents[variable]),
            usize::from(exponents[parameter]),
        );
        if powers.len() <= k {
            powers.resize(k + 1, Vec::new());
        }
        if powers[k].len() <= j {
            powers[k].resize(j + 1, Rat::ZERO);
        }
        powers[k][j] = coefficient;
    }
    let mut common: Option<Vec<Rat>> = None;
    for p in powers.iter().filter(|p| !p.is_empty()) {
        common = Some(match common {
            None => p.clone(),
            Some(g) => polynomial_gcd(&g, p, work)?,
        });
    }
    let common = common?;
    if common.len() > 1 {
        for p in powers.iter_mut().filter(|p| !p.is_empty()) {
            *p = polynomial_quotient(p, &common, work)?;
        }
    }
    integral(&mut powers)?;
    let mut branches = match &powers[..] {
        [constant, slope] => {
            let numerator = scale(constant, Rat::ONE.neg()?)?;
            vec![match slope[..] {
                [divisor] => Branch::Polynomial(scale(&numerator, Rat::ONE.div(divisor)?)?),
                _ => Branch::Function(Algebraic {
                    numerator,
                    denominator: slope.clone(),
                    radical: None,
                }),
            }]
        }
        [c, b, a] => {
            let [a] = a[..] else {
                return None;
            };
            quadratic(a, b, c, work)?
        }
        _ => return None,
    };
    if branches.iter().all(|b| b.linear().is_some()) {
        branches.sort_by(linear_order);
    }
    Some(branches)
}

/// Clears the denominators and the integer content of the coefficients, as
/// the order of two roots depends on the leading coefficient that is left.
fn integral(powers: &mut [Vec<Rat>]) -> Option<()> {
    let all = || powers.iter().flatten();
    let scale = all().try_fold(1i128, |lcm, c| {
        let g = crate::cas::integer_gcd(lcm.unsigned_abs(), c.den.unsigned_abs()) as i128;
        lcm.checked_mul(c.den / g)
    })?;
    let scale = Rat::new(scale, 1)?;
    let mut content = 0u128;
    for c in all() {
        content = crate::cas::integer_gcd(content, c.mul(scale)?.num.unsigned_abs());
    }
    let mut divisor = i128::try_from(content).ok().filter(|&c| c != 0)?;
    // The highest power of the variable keeps a positive leading sign.
    if powers.last()?.last()?.num < 0 {
        divisor = -divisor;
    }
    let factor = scale.div(Rat::new(divisor, 1)?)?;
    for c in powers.iter_mut().flatten() {
        *c = c.mul(factor)?;
    }
    Some(())
}

/// The real roots of `a v^2 + b v + c` with `b` and `c` polynomials in the
/// parameter.
fn quadratic(a: Rat, b: &[Rat], c: &[Rat], work: &mut Work<'_>) -> Option<Vec<Branch>> {
    work.spend(b.len() * b.len() + c.len())?;
    let four_a = a.mul(Rat::new(4, 1)?)?;
    let discriminant = trimmed(subtract(&multiply(b, b)?, &scale(c, four_a)?)?);
    let denominator = vec![a.mul(Rat::new(2, 1)?)?];
    let numerator = scale(b, Rat::ONE.neg()?)?;
    let rational = |root: Vec<Rat>, negative: bool| {
        // `(-b +- root) / (2 a)` with a polynomial root.
        let root = if negative {
            scale(&root, Rat::ONE.neg()?)?
        } else {
            root
        };
        let sum = subtract(&root, b)?;
        Some(Branch::Polynomial(trimmed(scale(
            &sum,
            Rat::ONE.div(denominator[0])?,
        )?)))
    };
    // `(-b +- multiplier sqrt(radicand)) / (2 a)` in lowest terms, the
    // negative root first or second, and whether the content of the
    // radicand left a second radical factor.
    let pair = |multiplier: &[Rat], radicand: &[Rat], work: &mut Work<'_>| {
        let (outside, radicand, leftover) = square_content(radicand, work)?;
        let content = |coefficients: &[Rat]| {
            coefficients.iter().try_fold(0, |g, c| {
                (c.den == 1).then(|| crate::cas::integer_gcd(g, c.num.unsigned_abs()))
            })
        };
        let common = (|| {
            let rational = crate::cas::integer_gcd(content(&numerator)?, content(&denominator)?);
            let radical = content(multiplier)?.checked_mul(outside)?;
            i128::try_from(crate::cas::integer_gcd(rational, radical)).ok()
        })()
        .filter(|&g| g != 0)
        .unwrap_or(1);
        let reduce = Rat::new(1, common)?;
        let numerator = scale(&numerator, reduce)?;
        let denominator = scale(&denominator, reduce)?;
        let multiplier = scale(multiplier, Rat::new(i128::try_from(outside).ok()?, common)?)?;
        let branch = move |negative| {
            Branch::Function(Algebraic {
                numerator: numerator.clone(),
                denominator: denominator.clone(),
                radical: Some(Radical {
                    negative,
                    multiplier: multiplier.clone(),
                    radicand: radicand.clone(),
                }),
            })
        };
        Some((branch(false), branch(true), leftover))
    };
    // A discriminant `k q^2` gives two lines: with a square `k` over the
    // rationals, otherwise with the irrational factor `sqrt(k)`.
    let square = match discriminant[..] {
        [] => Some((Rat::ZERO, vec![Rat::ONE])),
        [k] => Some((k, vec![Rat::ONE])),
        [d0, d1, d2] if d1.mul(d1)? == d2.mul(d0)?.mul(Rat::new(4, 1)?)? => {
            // d2 (t + d1 / (2 d2))^2
            Some((d2, vec![d1.div(d2.mul(Rat::new(2, 1)?)?)?, Rat::ONE]))
        }
        _ => None,
    };
    if let Some((k, q)) = square {
        if k.num < 0 {
            // At most one real point.
            return Some(Vec::new());
        }
        if k == Rat::ZERO {
            return Some(vec![rational(Vec::new(), false)?]);
        }
        return Some(match rational_sqrt(k) {
            Some(root) => {
                let root = scale(&q, root)?;
                vec![rational(root.clone(), false)?, rational(root, true)?]
            }
            None => {
                let (positive, negative, _) = pair(&q, &[k], work)?;
                vec![positive, negative]
            }
        });
    }
    let has_real_root = match discriminant[..] {
        [_, _] => true,
        [d0, d1, d2] => {
            let sign = d1.to_f64() * d1.to_f64() - 4.0 * d2.to_f64() * d0.to_f64();
            if sign < 0.0 && d2.num < 0 {
                // Negative everywhere.
                return Some(Vec::new());
            }
            sign > 0.0
        }
        // More than a conic: not solved.
        _ => return None,
    };
    // The order GeoGebra's CAS lists the two roots in, as recorded in
    // golden/cas/conic.json: the negative root first when there is no
    // rational part, when the discriminant has no real root, or when a
    // monic equation leaves a second radical factor; otherwise the
    // positive root first.
    let (positive, negative, leftover) = pair(&[Rat::ONE], &discriminant, work)?;
    let negative_first = b.iter().all(|c| c.num == 0)
        || !has_real_root
        || (a.num.abs() == 1 && a.den == 1 && leftover);
    Some(if negative_first {
        vec![negative, positive]
    } else {
        vec![positive, negative]
    })
}

/// Splits the integer content of a discriminant into a square and the rest:
/// the root of the square, the discriminant without it, and whether the
/// rest of the content is more than one, so that the root keeps a second
/// factor `sqrt(rest)`.
fn square_content(discriminant: &[Rat], work: &mut Work<'_>) -> Option<(u128, Vec<Rat>, bool)> {
    if discriminant.iter().any(|c| c.den != 1) {
        return Some((1, discriminant.to_vec(), false));
    }
    let mut rest = discriminant.iter().fold(0u128, |g, c| {
        crate::cas::integer_gcd(g, c.num.unsigned_abs())
    });
    let mut outside = 1u128;
    let mut p = 2u128;
    while p <= 1000 && p * p <= rest {
        work.spend(1)?;
        while rest % (p * p) == 0 {
            rest /= p * p;
            outside *= p;
        }
        p += 1;
    }
    let root = (rest as f64).sqrt().round() as u128;
    if root.checked_mul(root) == Some(rest) {
        outside = outside.checked_mul(root)?;
        rest = 1;
    }
    let square = i128::try_from(outside.checked_mul(outside)?).ok()?;
    let radicand = scale(discriminant, Rat::new(1, square)?)?;
    Some((outside, radicand, rest != 1))
}

fn rational_sqrt(value: Rat) -> Option<Rat> {
    let root = |n: i128| {
        let r = (n as f64).sqrt().round() as i128;
        (r.checked_mul(r) == Some(n)).then_some(r)
    };
    Rat::new(root(value.num)?, root(value.den)?)
}

/// The order of solutions that are lines, as recorded in
/// golden/cas/conic.json: constants first, positive slopes before negative
/// ones, whole slopes before fractions, the smaller first; equal slopes by
/// their constants.
fn linear_order(p: &Branch, q: &Branch) -> std::cmp::Ordering {
    let key = |branch: &Branch| {
        let [constant, slope] = branch.linear().unwrap_or([f64::NAN; 2]);
        let class = if slope == 0.0 {
            0
        } else if slope > 0.0 {
            1
        } else {
            2
        };
        let fraction = match branch.slope() {
            Some(r) if r.den == 1 => (0, r.num.abs(), 1),
            Some(r) => (1, r.num.abs(), r.den),
            None => (2, 0, 1),
        };
        (class, fraction, slope, constant)
    };
    let (a, b) = (key(p), key(q));
    a.0.cmp(&b.0)
        .then_with(|| a.1.cmp(&b.1))
        .then_with(|| {
            if a.1 .0 == 2 {
                // Conjugate irrational slopes keep their order.
                std::cmp::Ordering::Equal
            } else {
                a.2.total_cmp(&b.2)
            }
        })
        .then_with(|| a.3.total_cmp(&b.3))
}

fn trimmed(mut coefficients: Vec<Rat>) -> Vec<Rat> {
    while coefficients.last().is_some_and(|c| c.num == 0) {
        coefficients.pop();
    }
    coefficients
}

fn scale(coefficients: &[Rat], factor: Rat) -> Option<Vec<Rat>> {
    coefficients.iter().map(|c| c.mul(factor)).collect()
}

fn subtract(left: &[Rat], right: &[Rat]) -> Option<Vec<Rat>> {
    (0..left.len().max(right.len()))
        .map(|i| {
            let l = left.get(i).copied().unwrap_or(Rat::ZERO);
            l.sub(right.get(i).copied().unwrap_or(Rat::ZERO))
        })
        .collect()
}

fn multiply(left: &[Rat], right: &[Rat]) -> Option<Vec<Rat>> {
    if left.is_empty() || right.is_empty() {
        return Some(Vec::new());
    }
    let mut product = vec![Rat::ZERO; left.len() + right.len() - 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            product[i + j] = product[i + j].add(a.mul(*b)?)?;
        }
    }
    Some(product)
}

fn dense_at(poly: &Poly, index: usize) -> Vec<Rat> {
    let len = poly
        .0
        .keys()
        .map(|p| p[index])
        .max()
        .map_or(0, |degree| usize::from(degree) + 1);
    let mut dense = vec![Rat::ZERO; len];
    for (power, &coefficient) in &poly.0 {
        dense[usize::from(power[index])] = coefficient;
    }
    dense
}

fn is_pole(root: Root, denominator: &[Rat]) -> bool {
    if denominator.len() <= 1 {
        return denominator.is_empty();
    }
    if let Root::Rational(r) = root {
        if let Some(value) = denominator
            .iter()
            .rev()
            .try_fold(Rat::ZERO, |v, &c| v.mul(r)?.add(c))
        {
            return value == Rat::ZERO;
        }
    }
    let x = root.value();
    let horner = |(v, m): (f64, f64), c: &Rat, x: f64| {
        (v.mul_add(x, c.to_f64()), m * x.abs() + c.to_f64().abs())
    };
    let (value, magnitude) = if x.abs() > 1.0 {
        denominator
            .iter()
            .fold((0.0, 0.0), |p, c| horner(p, c, x.recip()))
    } else {
        denominator
            .iter()
            .rev()
            .fold((0.0, 0.0), |p, c| horner(p, c, x))
    };
    value.abs() <= magnitude * (16.0 * f64::EPSILON)
}

pub(super) fn solution(root: Root, assignment: bool, approximate: bool, axis: u8) -> Slot {
    let value = root.value();
    if assignment && axis > 1 {
        return Slot::undefined(Kind::Line);
    }
    let slot = if assignment {
        let (a, b) = if axis == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
        Slot::line(Value::Line { a, b, c: -value })
    } else {
        Slot::number(value)
    };
    let text = if approximate { None } else { exact(root) };
    if let Some(text) = text {
        slot.with_symbolic(if assignment {
            let variable = if axis == 0 { "x" } else { "y" };
            format!("{variable} = {text}")
        } else {
            text
        })
    } else {
        slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }

    /// The polynomial with the terms `(x power, y power, coefficient)`.
    fn poly(terms: &[(u8, u8, i128)]) -> Poly {
        Poly(
            terms
                .iter()
                .map(|&(x, y, c)| ([x, y, 0], rat(c, 1)))
                .collect(),
        )
    }

    fn branches(terms: &[(u8, u8, i128)], variable: usize) -> Vec<Branch> {
        let mut work = Work::new(&|_| true);
        parametric(&poly(terms), variable, 1 - variable, &mut work).unwrap()
    }

    fn signs(branches: &[Branch]) -> Vec<bool> {
        branches
            .iter()
            .map(|b| match b {
                Branch::Function(f) => f.radical.as_ref().unwrap().negative,
                Branch::Polynomial(_) => panic!("expected a radical"),
            })
            .collect()
    }

    #[test]
    fn a_line_is_solved_for_either_variable() {
        // 2x + 3y - 6 = 0
        let line = [(1, 0, 2), (0, 1, 3), (0, 0, -6)];
        assert_eq!(
            branches(&line, 0),
            [Branch::Polynomial(vec![rat(3, 1), rat(-3, 2)])]
        );
        assert_eq!(
            branches(&line, 1),
            [Branch::Polynomial(vec![rat(2, 1), rat(-2, 3)])]
        );
    }

    #[test]
    fn a_common_factor_without_the_variable_is_dropped() {
        // x y = 0 solved for x is x = 0; (x - 2)(y - 1) = 0 is x = 2.
        assert_eq!(branches(&[(1, 1, 1)], 0), [Branch::Polynomial(vec![])]);
        let product = [(1, 1, 1), (1, 0, -1), (0, 1, -2), (0, 0, 2)];
        assert_eq!(branches(&product, 0), [Branch::Polynomial(vec![rat(2, 1)])]);
    }

    #[test]
    fn a_hyperbola_solved_for_one_variable_has_a_pole() {
        // x y - 1 = 0
        let [Branch::Function(f)] = &branches(&[(1, 1, 1), (0, 0, -1)], 1)[..] else {
            panic!("expected one rational branch");
        };
        assert_eq!(f.value(4.0), 0.25);
        assert!(!f.value(0.0).is_finite());
    }

    #[test]
    fn the_roots_of_a_circle_come_negative_first() {
        // x^2 + y^2 - 4 = 0
        let circle = [(2, 0, 1), (0, 2, 1), (0, 0, -4)];
        let both = branches(&circle, 0);
        assert_eq!(signs(&both), [true, false]);
        let Branch::Function(lower) = &both[0] else {
            unreachable!()
        };
        assert_eq!(lower.value(0.0), -2.0);
        assert!(lower.value(3.0).is_nan());
    }

    #[test]
    fn the_order_of_two_roots_follows_the_recorded_cases() {
        // (x - 1)^2 + (y - 2)^2 = 9: a rational part and a discriminant
        // with real roots, positive first.
        let shifted = [(2, 0, 1), (1, 0, -2), (0, 2, 1), (0, 1, -4), (0, 0, -4)];
        assert_eq!(signs(&branches(&shifted, 0)), [false, true]);
        // 3x^2 - y^2 + x - 2 = 0: the discriminant 12 y^2 + 25 has no root.
        let definite = [(2, 0, 3), (0, 2, -1), (1, 0, 1), (0, 0, -2)];
        assert_eq!(signs(&branches(&definite, 0)), [true, false]);
        // x^2 + x y + y^2 - 3 = 0: monic, the discriminant 3 (4 - y^2)
        // leaves sqrt(3).
        let content = [(2, 0, 1), (1, 1, 1), (0, 2, 1), (0, 0, -3)];
        assert_eq!(signs(&branches(&content, 0)), [true, false]);
        // x^2 + 2 x y + 3 y^2 - 6 = 0 solved for y: not monic, positive first.
        let scaled = [(2, 0, 1), (1, 1, 2), (0, 2, 3), (0, 0, -6)];
        assert_eq!(signs(&branches(&scaled, 1)), [false, true]);
        // x^2 + x y + y^2 - 1 = 0: the discriminant 4 - 3 y^2 has no content.
        let plain = [(2, 0, 1), (1, 1, 1), (0, 2, 1), (0, 0, -1)];
        assert_eq!(signs(&branches(&plain, 0)), [false, true]);
    }

    #[test]
    fn a_conic_without_real_points_has_no_branch() {
        assert!(branches(&[(2, 0, 1), (0, 2, 1), (0, 0, 4)], 0).is_empty());
        assert!(branches(&[(2, 0, 1), (0, 2, 1)], 0).is_empty());
    }

    #[test]
    fn degenerate_conics_split_into_ordered_lines() {
        let slopes = |terms: &[(u8, u8, i128)]| -> Vec<[f64; 2]> {
            branches(terms, 0)
                .iter()
                .map(|b| b.linear().unwrap())
                .collect()
        };
        // x^2 - y^2: x = y before x = -y.
        assert_eq!(slopes(&[(2, 0, 1), (0, 2, -1)]), [[0.0, 1.0], [0.0, -1.0]]);
        // (x - 2y)(x - 3y): the smaller whole slope first.
        assert_eq!(
            slopes(&[(2, 0, 1), (1, 1, -5), (0, 2, 6)]),
            [[0.0, 2.0], [0.0, 3.0]]
        );
        // (x - y)(x - 2): the constant first.
        assert_eq!(
            slopes(&[(2, 0, 1), (1, 1, -1), (1, 0, -2), (0, 1, 2)]),
            [[2.0, 0.0], [0.0, 1.0]]
        );
        // (x + y)^2 - 1: equal slopes by their constants.
        assert_eq!(
            slopes(&[(2, 0, 1), (1, 1, 2), (0, 2, 1), (0, 0, -1)]),
            [[-1.0, -1.0], [1.0, -1.0]]
        );
        // (x - y)^2: one line.
        assert_eq!(slopes(&[(2, 0, 1), (1, 1, -2), (0, 2, 1)]), [[0.0, 1.0]]);
        // x^2 - 2 y^2: irrational slopes, positive first.
        let irrational = slopes(&[(2, 0, 1), (0, 2, -2)]);
        assert!((irrational[0][1] - std::f64::consts::SQRT_2).abs() < 1e-15);
        assert!((irrational[1][1] + std::f64::consts::SQRT_2).abs() < 1e-15);
    }
}
