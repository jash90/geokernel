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
}

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

/// The real roots of `a v^2 + b v + c` with integer coefficients, `a`
/// positive, `b` linear and `c` quadratic in the parameter, in the order
/// GeoGebra lists them.
///
/// The CAS answers `(-b + sqrt(D)) / (2a)` before `(-b - sqrt(D)) / (2a)`,
/// writes each in its normal form and sorts the pair
/// (`ggbsort(normal(solve(...)))`). The sort puts the smaller root first
/// where the CAS can tell the sign of the difference of the two normal
/// forms, and the simpler expression first where it cannot. Which of the
/// two happens was recorded for some three thousand conics through the
/// harness oracle and follows the rules below; golden/cas/order.json holds
/// a case for each of them.
fn quadratic(a: Rat, b: &[Rat], c: &[Rat], work: &mut Work<'_>) -> Option<Vec<Branch>> {
    work.spend(b.len() * b.len() + c.len())?;
    let four_a = a.mul(Rat::new(4, 1)?)?;
    let discriminant = trimmed(subtract(&multiply(b, b)?, &scale(c, four_a)?)?);
    let two_a = a.mul(Rat::new(2, 1)?)?;
    let numerator = scale(b, Rat::ONE.neg()?)?;
    // `(-b + sign * root) / (2 a)` with a polynomial root.
    let rational = |root: &[Rat], negative: bool| {
        let root = if negative {
            scale(root, Rat::ONE.neg()?)?
        } else {
            root.to_vec()
        };
        let sum = subtract(&root, b)?;
        Some(Branch::Polynomial(trimmed(scale(
            &sum,
            Rat::ONE.div(two_a)?,
        )?)))
    };
    // `(-b + sign * multiplier * sqrt(radicand)) / (2 a)` in lowest terms.
    let radical = |multiplier: &[Rat], radicand: &[Rat], negative: bool| {
        let content = |coefficients: &[Rat]| {
            coefficients.iter().try_fold(0, |g, c| {
                (c.den == 1).then(|| crate::cas::integer_gcd(g, c.num.unsigned_abs()))
            })
        };
        let common = (|| {
            let rational = crate::cas::integer_gcd(content(&numerator)?, content(&[two_a])?);
            i128::try_from(crate::cas::integer_gcd(rational, content(multiplier)?)).ok()
        })()
        .filter(|&g| g != 0)
        .unwrap_or(1);
        let reduce = Rat::new(1, common)?;
        Some(Branch::Function(Algebraic {
            numerator: scale(&numerator, reduce)?,
            denominator: vec![two_a.mul(reduce)?],
            radical: Some(Radical {
                negative,
                multiplier: scale(multiplier, reduce)?,
                radicand: radicand.to_vec(),
            }),
        }))
    };
    let pair = |multiplier: &[Rat], radicand: &[Rat], negative_first: bool| {
        Some(vec![
            radical(multiplier, radicand, negative_first)?,
            radical(multiplier, radicand, !negative_first)?,
        ])
    };
    // The integer content of the discriminant is `k m^2` with `k` free of
    // squares: `sqrt(D) = m sqrt(k) sqrt(D / (k m^2))`.
    let integers = discriminant
        .iter()
        .map(|c| (c.den == 1).then_some(c.num))
        .collect::<Option<Vec<_>>>()?;
    let content = integers
        .iter()
        .fold(0u128, |g, c| crate::cas::integer_gcd(g, c.unsigned_abs()));
    if content == 0 {
        // A double root.
        return Some(vec![rational(&[], false)?]);
    }
    let (outside, free) = square_part(content, work)?;
    let outside_rat = Rat::new(i128::try_from(outside).ok()?, 1)?;
    let free_rat = Rat::new(i128::try_from(free).ok()?, 1)?;
    let primitive = scale(&discriminant, Rat::new(1, i128::try_from(content).ok()?)?)?;
    // `D = content * q^2` with a polynomial `q`: lines.
    // A primitive quadratic is `+-(u t + v)^2` exactly when its outer
    // coefficients are squares of whole numbers `u` and `v` up to that
    // sign and the middle one is `+-2 u v`.
    let squared = |d0: Rat, d1: Rat, d2: Rat| {
        let sign = d2.num.signum();
        let u = integer_sqrt(d2.num.checked_mul(sign)?)?;
        let v = integer_sqrt(d0.num.checked_mul(sign)?)?;
        let middle = u.checked_mul(v)?.checked_mul(2)?;
        (d1.num.checked_abs()? == middle).then_some((sign, u, v * d1.num.signum() * sign))
    };
    let root_of_primitive = match primitive[..] {
        [d0] if d0.num > 0 => Some(vec![Rat::ONE]),
        [_] => return Some(Vec::new()),
        [d0, d1, d2] => match squared(d0, d1, d2) {
            // Never positive: at most one real point.
            Some((-1, ..)) => return Some(Vec::new()),
            Some((_, u, v)) => Some(vec![Rat::new(v, 1)?, Rat::new(u, 1)?]),
            None => None,
        },
        _ => None,
    };
    if let Some(q) = root_of_primitive {
        let slope_differs = q.len() == 2;
        if free == 1 {
            // Rational lines, in the order of their printed forms.
            let root = scale(&q, outside_rat)?;
            let mut lines = vec![rational(&root, false)?, rational(&root, true)?];
            if rational_line_first(&lines[1], &lines[0])? {
                lines.swap(0, 1);
            }
            return Some(lines);
        }
        if slope_differs && outside % 2 == 1 {
            // With an odd square part the CAS does not split the quadratic
            // over the irrational `sqrt(k)` and keeps `sqrt(k) |q|`: no
            // lines, and a difference whose sign it knows.
            return pair(&[Rat::ONE], &discriminant, true);
        }
        // Irrational lines `(-b +- m sqrt(k) q) / (2 a)`: with different
        // slopes the one with `+sqrt(k)` in its slope is the simpler
        // expression; with equal slopes the constants decide.
        let multiplier = scale(&q, outside_rat)?;
        return pair(&multiplier, &[free_rat], !slope_differs);
    }
    let positive_definite = match primitive[..] {
        [_, _] => false,
        [d0, d1, d2] => {
            // The sign of `d1^2 - 4 d2 d0`, which is not zero here: exact
            // where it fits, in doubles where the two terms are clearly
            // apart, undecided otherwise.
            let exact = (|| {
                let square = d1.num.checked_mul(d1.num)?;
                let product = d2.num.checked_mul(d0.num)?.checked_mul(4)?;
                Some(square.checked_sub(product)?.signum())
            })();
            let sign = match exact {
                Some(sign) => sign,
                None => {
                    let square = d1.to_f64() * d1.to_f64();
                    let product = 4.0 * d2.to_f64() * d0.to_f64();
                    let apart = (square - product).abs() > 1e-9 * (square + product.abs());
                    if !apart {
                        return None;
                    }
                    if square > product {
                        1
                    } else {
                        -1
                    }
                }
            };
            if sign < 0 && d2.num < 0 {
                // Negative everywhere.
                return Some(Vec::new());
            }
            sign < 0
        }
        // More than a conic: not solved.
        _ => return None,
    };
    // The coefficient of the root in the normal form is `q sqrt(k)` with
    // `q = m / (2 a)`. The CAS knows the sign of the difference of the two
    // roots, and lists the smaller first,
    // * when there is no rational part, as the difference is a sum of
    //   positive terms;
    // * when the discriminant is positive everywhere;
    // * with a second radical `sqrt(k)`, when `q` has the denominator 1
    //   or 2;
    // * without one, when `q` is a whole number above 1 and the rational
    //   part has whole coefficients.
    let q = outside_rat.div(two_a)?;
    let whole_rational_part = b.iter().all(|c| c.div(two_a).is_some_and(|c| c.den == 1));
    let negative_first = b.iter().all(|c| c.num == 0)
        || positive_definite
        || if free == 1 {
            q.den == 1 && q.num >= 2 && whole_rational_part
        } else {
            q.den <= 2
        };
    let radicand = scale(&primitive, free_rat)?;
    pair(&[outside_rat], &radicand, negative_first)
}

/// Splits a positive integer into `m^2 * k` with `k` free of squares.
/// Squares of the numbers up to a hundred thousand are divided out; what is left
/// is a square itself, or too small to hold the square of a larger number
/// times anything, or the split is not known and the answer is `None`.
fn square_part(mut n: u128, work: &mut Work<'_>) -> Option<(u128, u128)> {
    const TRIED: u128 = 100_000;
    let mut outside = 1u128;
    let mut p = 2u128;
    while p <= TRIED && p * p <= n {
        work.spend(1)?;
        while n.is_multiple_of(p * p) {
            n /= p * p;
            outside *= p;
        }
        p += 1;
    }
    if let Some(root) = i128::try_from(n).ok().and_then(integer_sqrt) {
        return Some((outside.checked_mul(root.unsigned_abs())?, 1));
    }
    (n < 2 * (TRIED + 1) * (TRIED + 1)).then_some((outside, n))
}

/// The whole square root of a perfect square, `None` for anything else.
fn integer_sqrt(n: i128) -> Option<i128> {
    if n < 0 {
        return None;
    }
    // A double holds 53 bits: the estimate is off by a few units in the
    // last place of a root of up to 64 bits, which Newton steps on whole
    // numbers remove.
    let mut root = (n as f64).sqrt().round() as i128;
    for _ in 0..8 {
        if root <= 0 {
            break;
        }
        let next = (root + n / root) / 2;
        if next == root {
            break;
        }
        root = next;
    }
    [root - 1, root, root + 1]
        .into_iter()
        .find(|r| *r >= 0 && r.checked_mul(*r) == Some(n))
}

/// Whether the line `p` comes before the line `q` in the answer of the
/// reference. Lines with the same slope are sorted by their constants.
/// Other pairs are listed in the order of their printed forms
/// `slope * t + constant`, as recorded for sixteen hundred pairs
/// (golden/cas/order.json holds a case for each class): a whole constant
/// other than zero; then positive slopes without a constant, then with
/// one; then negative slopes with a constant, then without; zero and
/// fractions last. Within a class whole slopes come first by their size,
/// then fractions by numerator and denominator.
fn rational_line_first(p: &Branch, q: &Branch) -> Option<bool> {
    let parts = |branch: &Branch| match branch {
        Branch::Polynomial(c) if c.len() <= 2 => Some((
            c.get(1).copied().unwrap_or(Rat::ZERO),
            c.first().copied().unwrap_or(Rat::ZERO),
        )),
        _ => None,
    };
    let ((slope_p, constant_p), (slope_q, constant_q)) = (parts(p)?, parts(q)?);
    if slope_p == slope_q {
        return Some(constant_p.sub(constant_q)?.num < 0);
    }
    let key = |slope: Rat, constant: Rat| {
        let size = (
            u8::from(slope.den != 1),
            slope.num.unsigned_abs(),
            slope.den,
        );
        let class = match (slope.num.signum(), constant.num == 0) {
            (0, _) if constant.num != 0 && constant.den == 1 => 0,
            (0, _) => 5,
            (1, true) => 1,
            (1, false) => 2,
            (_, false) => 3,
            (_, true) => 4,
        };
        (class, size)
    };
    Some(key(slope_p, constant_p) < key(slope_q, constant_q))
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
        // (x - 1)^2 + (y - 2)^2 = 9: D = 4 (-y^2 + 4y + 5), the factor 1 in
        // front of the root, positive first.
        let shifted = [(2, 0, 1), (1, 0, -2), (0, 2, 1), (0, 1, -4), (0, 0, -4)];
        assert_eq!(signs(&branches(&shifted, 0)), [false, true]);
        // 3x^2 - y^2 + x - 2 = 0: the discriminant 12 y^2 + 25 has no root.
        let definite = [(2, 0, 3), (0, 2, -1), (1, 0, 1), (0, 0, -2)];
        assert_eq!(signs(&branches(&definite, 0)), [true, false]);
        // x^2 + x y + y^2 - 3 = 0: D = 3 (4 - y^2) leaves sqrt(3) / 2.
        let content = [(2, 0, 1), (1, 1, 1), (0, 2, 1), (0, 0, -3)];
        assert_eq!(signs(&branches(&content, 0)), [true, false]);
        // x^2 + 2 x y + 3 y^2 - 6 = 0 solved for y: D = 8 (9 - x^2) leaves
        // sqrt(2) / 3, positive first.
        let scaled = [(2, 0, 1), (1, 1, 2), (0, 2, 3), (0, 0, -6)];
        assert_eq!(signs(&branches(&scaled, 1)), [false, true]);
        // x^2 + x y + y^2 - 1 = 0: D = 4 - 3 y^2 with the factor 1/2.
        let plain = [(2, 0, 1), (1, 1, 1), (0, 2, 1), (0, 0, -1)];
        assert_eq!(signs(&branches(&plain, 0)), [false, true]);
    }

    #[test]
    fn every_rule_of_the_order_has_its_conic() {
        // The conics of golden/cas/order.json, solved for x: `true` first
        // is the root with the negative square root first.
        for (terms, negative_first, rule) in [
            // 2x^2 + 2xy + 2y^2 - 3: D = 12 (2 - y^2), sqrt(3) / 2.
            (
                &[(2, 0, 2), (1, 1, 2), (0, 2, 2), (0, 0, -3)][..],
                true,
                "a second radical with the factor 1/2",
            ),
            // 3x^2 + 6x - y^2 + 5y + 1: D = 12 (y^2 - 5y + 2), sqrt(3) / 3.
            (
                &[(2, 0, 3), (1, 0, 6), (0, 2, -1), (0, 1, 5), (0, 0, 1)][..],
                false,
                "a second radical with the factor 1/3",
            ),
            // x^2 + 6xy + 2x - 3y + 1: D = 36 (y^2 + y), the factor 3.
            (
                &[(2, 0, 1), (1, 1, 6), (1, 0, 2), (0, 1, -3), (0, 0, 1)][..],
                true,
                "a whole factor above 1",
            ),
            // x^2 - 4xy + 6y^2 + 4y - 1: D = 4 (-2y^2 - 4y + 1), the factor 1.
            (
                &[(2, 0, 1), (1, 1, -4), (0, 2, 6), (0, 1, 4), (0, 0, -1)][..],
                false,
                "the factor 1",
            ),
            // 5x^2 + 4y^2 + 3y - 3: no rational part.
            (
                &[(2, 0, 5), (0, 2, 4), (0, 1, 3), (0, 0, -3)][..],
                true,
                "no rational part",
            ),
            // x^2 + 6xy + 5x - y^2 - y - 5: D = 40y^2 + 64y + 45 > 0.
            (
                &[
                    (2, 0, 1),
                    (1, 1, 6),
                    (1, 0, 5),
                    (0, 2, -1),
                    (0, 1, -1),
                    (0, 0, -5),
                ][..],
                true,
                "a discriminant without a root",
            ),
            // x^2 + xy - y^2: D = 5 y^2 with an odd square part, kept as
            // sqrt(5 y^2).
            (
                &[(2, 0, 1), (1, 1, 1), (0, 2, -1)][..],
                true,
                "an odd square part",
            ),
            // (x - y)^2 - 2: equal slopes, the constants ascend.
            (
                &[(2, 0, 1), (1, 1, -2), (0, 2, 1), (0, 0, -2)][..],
                true,
                "equal irrational slopes",
            ),
        ] {
            assert_eq!(
                signs(&branches(terms, 0)),
                [negative_first, !negative_first],
                "{rule}"
            );
        }
    }

    #[test]
    fn large_coefficients_keep_the_arithmetic_exact_or_give_up() {
        let big = 1_000_000_000_000_000_003_i128;
        assert_eq!(integer_sqrt(big * big), Some(big));
        assert_eq!(integer_sqrt(big * big + 1), None);
        assert_eq!(integer_sqrt(-4), None);
        assert_eq!(integer_sqrt(0), Some(0));
        let mut work = Work::new(&|_| true);
        assert_eq!(square_part(12, &mut work), Some((2, 3)));
        assert_eq!(square_part(1009 * 1009, &mut work), Some((1009, 1)));
        assert_eq!(square_part(4 * 1_000_003, &mut work), Some((2, 1_000_003)));
        assert_eq!(square_part(2 * 1009 * 1009, &mut work), Some((1009, 2)));
        // 2 * 100003^2 hides a square the trial division does not reach.
        assert_eq!(square_part(2 * 100_003 * 100_003, &mut work), None);
        // 4 * 10^30 (x^2 + y^2) = 9 * 10^30 keeps its order.
        let scale = 1_000_000_000_000_000_i128;
        let circle = [(2, 0, 4 * scale), (0, 2, 4 * scale), (0, 0, -9 * scale)];
        assert_eq!(signs(&branches(&circle, 0)), [true, false]);
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
