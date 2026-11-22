use super::input::{read, variables};
use super::single::{self, dense_at, solution, Mode, Nesting};
use super::{Poly, Rat, Slot, Work};
use crate::cas::{affine_system, systems, Root, SystemRoots};
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
    for item in start..start + length {
        let Node::Item(i) = ctx.node(item) else {
            return None;
        };
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
                // An undefined line prints no equation: the exact commands
                // answer no solution, the numeric ones nothing.
                return if approximate || points {
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
        unordered: single && rows,
    };
    if single {
        if rows && approximate {
            // A list of variables sends one equation to the numeric system
            // solver, which starts at 0 (`Ggb2giac`, before `NSolutions.1`)
            // and answers whatever its search reaches: `(x - 2)^2` gives 2,
            // `(x - 2)^3` and `x^3 + 1` nothing, `x^2 - 4x + 3` only 1
            // (golden/cas/shape.json). Only a linear equation, whose one
            // root every search finds, has a determined answer.
            let equation = polynomials.pop()?;
            let axis = usize::from(axes[0]);
            let alone = equation.denominator.scalar().is_some()
                && equation
                    .numerator
                    .0
                    .keys()
                    .all(|p| (0..3).all(|j| j == axis || p[j] == 0));
            if !alone {
                return None;
            }
            let coefficients = dense_at(&equation.numerator, axis);
            if coefficients.len() != 2 {
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
            if approximate {
                // The numeric system solver answers one point of a family,
                // the one its search ends on (`NSolutions(x + y = 1, {x, y})`
                // gives `{0.5, 0.5}`), which the family does not determine.
                return None;
            }
            let families = affine_families(&equations, width, work)?;
            let lists = ctx.lists?;
            let count = families.len().try_into().ok()?;
            let start = lists.reserve(count)?;
            for (i, coordinates) in families.into_iter().enumerate() {
                let free = coordinates
                    .iter()
                    .rposition(|p| p == &[Rat::ZERO, Rat::ONE])?;
                let row = ctx.cas_slots(coordinates.into_iter().enumerate().map(|(j, p)| {
                    if assignments {
                        affine_assignment(j, free, axes, &p)
                    } else if p.len() == 1 {
                        Slot::number(p[0].to_f64())
                    } else {
                        Slot::polynomial(Poly::dense(&p))
                    }
                }))?;
                lists.set(start + i as u32, Slot::list(row.0, row.1));
            }
            Some((start, count))
        }
    }
}

fn affine_families(
    equations: &[std::collections::BTreeMap<[u8; 3], Rat>],
    width: usize,
    work: &mut Work<'_>,
) -> Option<Vec<Vec<Vec<Rat>>>> {
    if let Some(family) = affine_system(equations, width, work) {
        return Some(vec![family]);
    }
    // One product equation is the union of the equations of its factors.
    // Each branch must be representable, since a partial union loses roots.
    let [equation] = equations else {
        return None;
    };
    let (_, factors) = crate::cas::factor_poly(&Poly(equation.clone()), work)?;
    let mut families: Vec<_> = factors
        .into_iter()
        .map(|(factor, _)| affine_system(&[factor.0], width, work))
        .collect::<Option<_>>()?;
    let key = |coordinate: &[Rat]| {
        let slope = coordinate.get(1).map_or(0.0, |r| r.to_f64());
        (
            coordinate.len(),
            coordinate[0].to_f64() - 8.0 * slope,
            slope,
        )
    };
    families.sort_by(|a, b| {
        a.iter()
            .zip(b)
            .find_map(|(u, v)| {
                let (rank_u, value_u, slope_u) = key(u);
                let (rank_v, value_v, slope_v) = key(v);
                let cmp = rank_u
                    .cmp(&rank_v)
                    .then_with(|| value_u.total_cmp(&value_v))
                    .then_with(|| slope_u.total_cmp(&slope_v));
                cmp.is_ne().then_some(cmp)
            })
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some(families)
}

/// The equation of a line, a conic or an implicit curve as the reference
/// sends it to its CAS (`AlgoSolve.printCAS`): the definition as it was
/// typed where the object has one, otherwise the object's coefficients.
/// Coefficients are taken exactly, every one of them; one that has no
/// exact rational form makes the equation unknown.
fn object_equation(
    c: &crate::Construction,
    id: crate::Id,
    work: &mut Work<'_>,
) -> Option<crate::cas::Quotient> {
    if !c.value(id).is_defined() {
        return None;
    }
    let side = |left| crate::expr::equation::side_entry(c, id, left);
    if let (Some(left), Some(right)) = (side(true), side(false)) {
        let vars = &variables();
        let lhs = read(c, left, left.root, vars, work)?;
        let rhs = read(c, right, right.root, vars, work)?;
        return lhs.sub(rhs, work);
    }
    let terms: Vec<([u8; 3], f64)> = match c.kind(id) {
        Kind::Line => {
            let Value::Line { a, b, c } = c.value(id) else {
                return None;
            };
            vec![([1, 0, 0], a), ([0, 1, 0], b), ([0, 0, 0], c)]
        }
        Kind::Conic | Kind::Circle => {
            let [xx, yy, c, xy, x, y] = c.conic(id)?.matrix;
            vec![
                ([2, 0, 0], xx),
                ([0, 2, 0], yy),
                ([0, 0, 0], c),
                ([1, 1, 0], 2.0 * xy),
                ([1, 0, 0], 2.0 * x),
                ([0, 1, 0], 2.0 * y),
            ]
        }
        Kind::ImplicitCurve => {
            // Rows are the powers of x, columns the powers of y.
            let matrix = c.implicit_coefficients(id)?;
            let mut terms = Vec::new();
            for (i, row) in matrix.iter().enumerate() {
                work.spend(row.len())?;
                for (j, &v) in row.iter().enumerate() {
                    terms.push(([u8::try_from(i).ok()?, u8::try_from(j).ok()?, 0], v));
                }
            }
            terms
        }
        _ => return None,
    };
    work.spend(terms.len())?;
    let coefficients = terms
        .into_iter()
        .filter(|&(_, v)| v != 0.0)
        .map(|(p, v)| Some((p, Rat::from_value(v)?)))
        .collect::<Option<_>>()?;
    Some(crate::cas::Quotient::polynomial(Poly(coefficients)))
}

pub(super) fn identity(axis: u8) -> Slot {
    let variable = ["x", "y", "z"][usize::from(axis)];
    Slot::undefined(Kind::Line).with_symbolic(format!("{variable} = {variable}"))
}

fn affine_assignment(j: usize, free: usize, axes: [u8; 3], coefficients: &[Rat]) -> Slot {
    if j == free {
        return identity(axes[j]);
    }
    if axes[j] > 1 || axes[free] > 1 {
        return Slot::undefined(Kind::Line);
    }
    let value = coefficients[0].to_f64();
    let scale = coefficients.get(1).map_or(0.0, |r| r.to_f64());
    let (a, b) = if axes[j] == 0 {
        (1.0, -scale)
    } else {
        (-scale, 1.0)
    };
    Slot::line(Value::Line { a, b, c: -value })
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
