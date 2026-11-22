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
    guided_root, polynomial_gcd, polynomial_quotient, real_roots, Algebraic, Root, Roots,
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
    /// A list of equations solved for one variable: `{}`, `{a}` or
    /// `{{a, b}}` (`AlgoSolve.compute` nests the answers of a list only
    /// when there are several).
    RowIfSeveral,
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
    /// `Solve(equation, name = value)` with a name other than `x`: the CAS
    /// then knows the sign of terms in that name and orders two branches
    /// by it, in a way that is not reproduced here.
    pub(super) assumed: bool,
    /// A list of variables after one equation: the system solver of the
    /// reference lists several solutions in the order its CAS returns them,
    /// which follows no rule (golden/cas/shape.json), so only an answer
    /// with at most one solution is given.
    pub(super) unordered: bool,
    /// The largest magnitude of a constant in the equation as typed (see
    /// `typed_constants`), which bounds where a start value is answered.
    pub(super) constants: f64,
}

/// The largest typed constant with which a start value is answered: the
/// reference answers the recorded equations with constants up to 10^10
/// (`x^3 + x - 10^10`, `10^10 (x^3 + x + 1)`) and none of those with
/// constants from 10^12 on (`(x - 10^4)^3 + (10^12 - 10)`, `x^3 - 10^18`)
/// (golden/cas/magnitude.json).
const GUIDED_CONSTANT_LIMIT: f64 = 1e10;

/// The second argument of a solve names something that is no variable of
/// an equation: an existing number (`AlgoSolve.printHint` prints its
/// value).
pub(in crate::expr) const NO_VARIABLE: u8 = 3;

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
    // With an initial value every command but `Solve` runs the numeric
    // solver, which leaves any other variable unresolved.
    let numeric = mode.approximate || (guess.is_some() && !mode.assignments);
    if hint == Some(NO_VARIABLE) {
        // Solving for a number: no solution, and nothing for the numeric
        // solver to start from.
        return if numeric || mode.points || guess.is_some() {
            None
        } else {
            ctx.cas_slots(std::iter::empty())
        };
    }
    let variable = match hint {
        Some(axis) => usize::from(axis),
        None if used.contains(&0) => 0,
        None => used.first().copied().unwrap_or(0),
    };
    let others: Vec<usize> = used.iter().copied().filter(|&j| j != variable).collect();
    if others.is_empty() {
        return univariate(ctx, equation, variable, variable as u8, mode, guess, work);
    }
    if numeric {
        return None;
    }
    let branches = if used.contains(&variable) {
        let [parameter] = others[..] else {
            return None;
        };
        equation.denominator.scalar()?;
        let branches = parametric(&equation.numerator, variable, parameter, work)?;
        if mode.assumed && branches.len() > 1 {
            return None;
        }
        branches
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
        if mode.constants.is_nan() || mode.constants > GUIDED_CONSTANT_LIMIT {
            return None;
        }
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
    if mode.unordered && slots.len() > 1 {
        return None;
    }
    let single = mode.nesting == Nesting::RowIfSeveral && slots.len() == 1;
    if mode.nesting == Nesting::Flat || slots.is_empty() || single {
        return ctx.cas_slots(slots.into_iter());
    }
    let lists = ctx.lists?;
    if matches!(mode.nesting, Nesting::OneRow | Nesting::RowIfSeveral) {
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
/// free; `None` where there are two or more, or the degree is too high.
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
    // A coefficient that is a number leaves no common factor to look for.
    let constant = powers.iter().any(|p| p.len() == 1);
    for p in powers.iter().filter(|p| !p.is_empty()) {
        common = Some(match common {
            None => p.clone(),
            Some(_) if constant => vec![Rat::ONE],
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
    let branches = match &powers[..] {
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
        // A quadratic with the shape of a conic: a constant leading
        // coefficient, a linear middle one.
        [c, b, a] if b.len() <= 2 && c.len() <= 3 => {
            let [a] = a[..] else {
                return None;
            };
            quadratic(a, b, c, work)?
        }
        _ => return None,
    };
    Some(branches)
}

/// Clears the denominators and the integer content of the coefficients and
/// makes the leading coefficient positive.
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

/// The real branches of `a v^2 + b v + c = 0` with integer coefficients,
/// `a` positive, `b` linear and `c` quadratic in the parameter: none when
/// the discriminant `b^2 - 4 a c` is never positive, the one rational
/// branch `-b / (2 a)` when it vanishes, and `None` when there are two.
///
/// Two branches are not answered: the reference lists them in the order
/// of its CAS's sort of the two normal forms (`ggbsort(normal(solve(...)))`
/// in `Ggb2giac`), which is backend behaviour that no EUPL source states.
fn quadratic(a: Rat, b: &[Rat], c: &[Rat], work: &mut Work<'_>) -> Option<Vec<Branch>> {
    work.spend(b.len() * b.len() + c.len())?;
    let four_a = a.mul(Rat::new(4, 1)?)?;
    let discriminant = trimmed(subtract(&multiply(b, b)?, &scale(c, four_a)?)?);
    match discriminant[..] {
        [] => {
            let root = scale(b, Rat::ONE.neg()?.div(a.mul(Rat::new(2, 1)?)?)?)?;
            Some(vec![Branch::Polynomial(trimmed(root))])
        }
        [d0] if d0.num < 0 => Some(Vec::new()),
        [d0, d1, d2] if d2.num < 0 => {
            // `d2 t^2 + d1 t + d0` with `d2 < 0` is never positive when
            // `d1^2 - 4 d2 d0 <= 0`; it then has at most one real point.
            let sign = d1.mul(d1)?.sub(d2.mul(d0)?.mul(Rat::new(4, 1)?)?)?.num;
            (sign <= 0).then(Vec::new)
        }
        _ => None,
    }
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

pub(super) fn dense_at(poly: &Poly, index: usize) -> Vec<Rat> {
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
    fn two_branches_are_not_answered() {
        let mut work = Work::new(&|_| true);
        // x^2 + y^2 - 4, x^2 - y^2 and (13x - 10y - 10)(3x - 5y + 2).
        for terms in [
            &[(2, 0, 1), (0, 2, 1), (0, 0, -4)][..],
            &[(2, 0, 1), (0, 2, -1)][..],
            &[
                (2, 0, 39),
                (1, 1, -95),
                (0, 2, 50),
                (1, 0, -4),
                (0, 1, 30),
                (0, 0, -20),
            ][..],
        ] {
            assert_eq!(parametric(&poly(terms), 0, 1, &mut work), None);
        }
        // (x - y)^2 has the one branch x = y.
        assert_eq!(
            branches(&[(2, 0, 1), (1, 1, -2), (0, 2, 1)], 0),
            [Branch::Polynomial(vec![Rat::ZERO, rat(1, 1)])]
        );
    }

    #[test]
    fn a_conic_without_real_points_has_no_branch() {
        assert!(branches(&[(2, 0, 1), (0, 2, 1), (0, 0, 4)], 0).is_empty());
        assert!(branches(&[(2, 0, 1), (0, 2, 1)], 0).is_empty());
    }
}
