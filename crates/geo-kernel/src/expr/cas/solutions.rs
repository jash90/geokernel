use super::input::{read, variables};
use super::single::{self, dense_at, solution, Mode, Nesting};
use super::{Poly, Rat, Slot, Work};
use crate::cas::{systems, Root, SystemRoots};
use crate::expr::{eval::Ctx, Node};
use crate::{Kind, Value};

pub(super) fn list<const M: bool>(
    ctx: &Ctx<'_, M>,
    equations: u32,
    guess: u32,
    op: super::Op,
    work: &mut Work<'_>,
) -> Option<(u32, u32)> {
    let super::Op::Solve {
        assignments,
        approximate,
        width,
        axes,
        system,
        rows,
        points,
    } = op
    else {
        return None;
    };
    let Node::List(start, length) = ctx.node(equations) else {
        return None;
    };
    let single = !system && length == 1 && width <= 1;
    let vars = &variables();
    let mut polynomials = Vec::with_capacity(length as usize);
    let mut constants = f64::INFINITY;
    for item in start..start + length {
        let Node::Item(i) = ctx.node(item) else {
            return None;
        };
        if single && (guess != crate::expr::NONE || (rows && approximate)) {
            constants = typed_bound(ctx, i, work);
        }
        match ctx.node(i) {
            Node::Obj(id) if ctx.c.kind(id) == Kind::List => {
                let list = ctx.c.elements(id)?;
                if list.is_empty() || list.len() > 8 {
                    return None;
                }
                for k in 0..list.len() {
                    work.spend(1)?;
                    polynomials.push(object_equation(ctx.c, list.object(k)?, work)?);
                }
            }
            Node::Obj(id)
                if single && ctx.c.kind(id) == Kind::Line && !ctx.c.value(id).is_defined() =>
            {
                // A typed line whose definition reduces to a false constant
                // (`a x = 3` with `a = 0`) is sent as `0 = 3`. `Solve.1` and
                // `Solutions.2` solve it to no solution; `Solutions.1` asks
                // for `lname(%0)[0]` of an equation without variables and
                // fails, as does the numeric solver (`Ggb2giac`). Anything
                // else about an undefined line is undefined.
                let equation = typed_equation(ctx.c, id, work)?;
                if !equation
                    .numerator
                    .0
                    .keys()
                    .all(|p| p.iter().all(|&e| e == 0))
                    || equation.numerator.0.is_empty()
                {
                    return None;
                }
                let failing = approximate || points || (!assignments && width == 0);
                return if failing {
                    None
                } else {
                    ctx.cas_slots(std::iter::empty())
                };
            }
            Node::Obj(id)
                if matches!(
                    ctx.c.kind(id),
                    Kind::Line | Kind::Conic | Kind::Circle | Kind::ImplicitCurve
                ) =>
            {
                polynomials.push(object_equation(ctx.c, id, work)?);
            }
            _ => polynomials.push(read(ctx.c, ctx.e, i, vars, work)?),
        }
    }
    let mode = |nesting| Mode {
        assignments,
        approximate,
        points,
        nesting,
        assumed: width == 1 && axes[1] == single::NO_VARIABLE,
        // `Solve` with a list of variables keeps main's ascending order.
        unordered: single && rows && !assignments,
        constants,
    };
    if single {
        if rows && approximate {
            // A list of variables sends one equation to the numeric system
            // solver, which starts at 0 (`Ggb2giac`, before `NSolutions.1`)
            // and answers whatever its search reaches: `(x - 2)^2` gives 2,
            // `(x - 2)^3` and `x^3 + 1` nothing, `x^2 - 4x + 3` only 1
            // (golden/cas/shape.json). Only a linear equation, whose one
            // root every search finds, has a determined answer.
            // The variable is the one the equation holds: a listed name is
            // read into the first slot, an object keeps its own axes.
            let equation = polynomials.pop()?;
            let held: Vec<usize> = (0..3)
                .filter(|&j| equation.numerator.0.keys().any(|p| p[j] != 0))
                .collect();
            let [axis] = held[..] else {
                return None;
            };
            equation.denominator.scalar()?;
            let coefficients = dense_at(&equation.numerator, axis);
            // The reference answers the recorded `a x - 1` with |a| up to
            // 10^10 and none with |a| from 10^15 on (golden/cas/magnitude.json):
            // every intermediate coefficient of the typed polynomial must be
            // within that bound.
            if coefficients.len() != 2 || constants.is_nan() || constants > 1e10 {
                return None;
            }
            let root = crate::cas::unique_real_root(&coefficients, work)?;
            let slot = solution(root, assignments, true, axes[0]);
            return ctx.cas_slots(std::iter::once(slot));
        }
        // A list of variables after one equation gives one row per value.
        let nesting = if rows && !assignments {
            Nesting::Rows
        } else {
            Nesting::Flat
        };
        let guess = (guess != crate::expr::NONE).then(|| ctx.full_number(guess));
        let hint = (width == 1).then_some(axes[0]);
        let mode = mode(nesting);
        return single::solve(ctx, polynomials.pop()?, hint, mode, guess, work);
    }
    if points {
        // `PlotSolve` of a list passes the variables as a second argument,
        // which the command does not take.
        return None;
    }
    if guess != crate::expr::NONE {
        // The reference hands a list and a start value to its system
        // solver, whose answer does not follow the start value
        // (golden/cas/shape.json): from 3 and from -0.5 the root 2 of
        // `x^2 = 4`, nothing for an odd degree.
        return None;
    }
    if system && width == 1 && !rows {
        // A list of equations and one variable: what solves all of them is
        // what solves their common factor.
        let axis = usize::from(axes[0]);
        let mut common: Option<Poly> = None;
        let mut alone = true;
        for equation in polynomials {
            equation.denominator.scalar()?;
            alone &= equation
                .numerator
                .0
                .keys()
                .all(|p| (0..3).all(|j| j == axis || p[j] == 0))
                && equation.numerator.0.keys().any(|p| p[axis] != 0);
            common = Some(match common {
                None => equation.numerator,
                Some(g) => crate::cas::poly_gcd(&g, &equation.numerator, work)?,
            });
        }
        if approximate {
            // The numeric system solver gives no values for one variable:
            // `NSolutions` answers no solution when every equation holds
            // that variable alone, and everything else is undefined.
            return if alone && !assignments {
                ctx.cas_slots(std::iter::empty())
            } else {
                None
            };
        }
        let common = common?;
        if common.0.keys().all(|p| p.iter().all(|&e| e == 0)) && !common.0.is_empty() {
            // Nothing solves all of the equations.
            return ctx.cas_slots(std::iter::empty());
        }
        let equation = crate::cas::Quotient::polynomial(common);
        let mode = mode(Nesting::RowIfSeveral);
        return single::solve(ctx, equation, Some(axes[0]), mode, None, work);
    }
    let active = |j| {
        polynomials.iter().any(|eq| {
            eq.numerator
                .0
                .keys()
                .chain(eq.denominator.0.keys())
                .any(|p| p[j] != 0)
        })
    };
    let used = (0..3).filter(|&j| active(j)).collect::<Vec<_>>();
    if polynomials.len() == 1 && width <= 1 && used.len() <= 1 {
        let axis = used.first().copied().unwrap_or(0);
        let coordinate = if width == 1 { axes[0] } else { axis as u8 };
        let nesting = if assignments {
            Nesting::OneRow
        } else {
            Nesting::Rows
        };
        let equation = polynomials.pop()?;
        let mode = mode(nesting);
        return single::univariate(ctx, equation, axis, coordinate, mode, None, work);
    }
    let width = if width == 0 {
        used.last().copied().map_or(1, |j| j + 1)
    } else {
        usize::from(width)
    };
    let equations = polynomials
        .into_iter()
        .map(|polynomial| {
            polynomial.denominator.scalar()?;
            Some(polynomial.numerator.0)
        })
        .collect::<Option<Vec<_>>>()?;
    match systems(&equations, width, work)? {
        SystemRoots::Finite(mut rows) => {
            rows.sort_by(|a, b| {
                a.iter()
                    .zip(b)
                    .find_map(|(r, s)| {
                        let cmp = r.value().total_cmp(&s.value());
                        cmp.is_ne().then_some(cmp)
                    })
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let lists = ctx.lists?;
            let count = rows.len().try_into().ok()?;
            let start = lists.reserve(count)?;
            for (i, roots) in rows.into_iter().enumerate() {
                let row = ctx.cas_slots(
                    roots
                        .into_iter()
                        .enumerate()
                        .map(|(j, root)| solution(root, assignments, approximate, axes[j])),
                )?;
                lists.set(start + i as u32, Slot::list(row.0, row.1));
            }
            Some((start, count))
        }
        SystemRoots::Infinite => {
            // Which variables stay free, and how the others are written in
            // them, is the CAS's choice (golden/cas/freevariable.json:
            // `{x + y = 1, z = 0}` keeps `z` itself), so a system with a
            // free variable is not answered.
            None
        }
    }
}

/// The equation of a line, a conic or an implicit curve as the reference
/// sends it to its CAS (`AlgoSolve.printCAS`): the definition as it was
/// typed. An object without one is printed from its coefficients
/// (`GeoLine.toValueString`, `Kernel.buildImplicitEquation`,
/// `Kernel.buildExplicitEquation`), which drops coefficients below the
/// kernel's precision and rounds the rest. That printing is reproduced only
/// for a line whose other coefficient is exactly zero, which prints as
/// `x = v` or `y = v` with `v` in 15 significant digits, and for a line with
/// whole coefficients, which prints as itself (`-x + y = 0`). Any other
/// object without a typed definition gives no equation (`Line((0, 1), (1,
/// 1 + 10^-9))` prints as `y = 1`).
fn object_equation(
    c: &crate::Construction,
    id: crate::Id,
    work: &mut Work<'_>,
) -> Option<crate::cas::Quotient> {
    if !c.value(id).is_defined() {
        return None;
    }
    if let Some(equation) = typed_equation(c, id, work) {
        return Some(equation);
    }
    let Value::Line { a, b, c: constant } = c.value(id) else {
        return None;
    };
    work.spend(3)?;
    // `DoubleUtil.isZero` of the remaining coefficient decides the form, so
    // it must be clearly nonzero.
    let axis_parallel = match (a == 0.0, b == 0.0) {
        (true, false) if b.abs() >= 1e-8 => Some((1, b)),
        (false, true) if a.abs() >= 1e-8 => Some((0, a)),
        _ => None,
    };
    let mut terms = std::collections::BTreeMap::new();
    if let Some((axis, coefficient)) = axis_parallel {
        // `x = v` or `y = v` (`Kernel.buildExplicitEquation`).
        let value = Rat::printed(-constant / coefficient)?;
        let mut power = [0u8; 3];
        power[axis] = 1;
        terms.insert(power, Rat::ONE);
        if value != Rat::ZERO {
            terms.insert([0, 0, 0], value.neg()?);
        }
        return Some(crate::cas::Quotient::polynomial(Poly(terms)));
    }
    // Whole coefficients, both of x and y clearly nonzero, are printed as
    // they are after dividing by their common divisor
    // (`Kernel.buildImplicitEquation` with `cancelDown`): the same line.
    let whole = |v: f64| v.fract() == 0.0 && v.abs() < 1e15;
    if !(whole(a) && whole(b) && whole(constant)) || a.abs() < 1e-8 || b.abs() < 1e-8 {
        return None;
    }
    for (power, v) in [([1, 0, 0], a), ([0, 1, 0], b), ([0, 0, 0], constant)] {
        if v != 0.0 {
            terms.insert(power, Rat::new(v as i128, 1)?);
        }
    }
    Some(crate::cas::Quotient::polynomial(Poly(terms)))
}

/// `lhs - rhs` of the equation as typed, where the object has one.
fn typed_equation(
    c: &crate::Construction,
    id: crate::Id,
    work: &mut Work<'_>,
) -> Option<crate::cas::Quotient> {
    let left = crate::expr::equation::side_entry(c, id, true)?;
    let right = crate::expr::equation::side_entry(c, id, false)?;
    let vars = &variables();
    let lhs = read(c, left, left.root, vars, work)?;
    let rhs = read(c, right, right.root, vars, work)?;
    lhs.sub(rhs, work)
}

/// The largest intermediate coefficient of what an equation item reads, as
/// typed (`input::typed_bound`): a typed object's sides, or the expression
/// itself. Infinite where it is not known or not a polynomial.
fn typed_bound<const M: bool>(ctx: &Ctx<'_, M>, i: u32, work: &mut Work<'_>) -> f64 {
    let largest = match ctx.node(i) {
        Node::Obj(id)
            if matches!(
                ctx.c.kind(id),
                Kind::Line | Kind::Conic | Kind::Circle | Kind::ImplicitCurve
            ) =>
        {
            let side = |left| crate::expr::equation::side_entry(ctx.c, id, left);
            match (side(true), side(false)) {
                (Some(l), Some(r)) => super::input::typed_bound_sides(ctx.c, [l, r], work),
                _ => None,
            }
        }
        _ => super::input::typed_bound(ctx.c, ctx.e, i, work),
    };
    largest.unwrap_or(f64::INFINITY)
}

pub(super) fn identity(axis: u8) -> Slot {
    let variable = ["x", "y", "z"][usize::from(axis)];
    Slot::undefined(Kind::Line).with_symbolic(format!("{variable} = {variable}"))
}

pub(super) fn exact(root: Root) -> Option<String> {
    let rational = super::functional::print_value;
    let power = |value: String, degree| {
        if degree == 2 {
            format!("sqrt({value})")
        } else {
            format!("({value})^(1/{degree})")
        }
    };
    let affine =
        |offset, scale, value| format!("{} + {}*{value}", rational(offset), rational(scale));
    match root {
        Root::Rational(value) => Some(rational(value)),
        Root::Radical {
            offset,
            scale,
            radicand,
            degree,
            ..
        } => Some(affine(offset, scale, power(rational(radicand), degree))),
        Root::NestedRadical {
            offset,
            scale,
            radicand_offset,
            radicand_scale,
            radicand,
            degree,
            ..
        } => {
            let radicand = affine(
                radicand_offset,
                radicand_scale,
                power(rational(radicand), 2),
            );
            Some(affine(offset, scale, power(radicand, degree)))
        }
        Root::Numeric(_) => None,
    }
}
