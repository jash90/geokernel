use super::input::{read, variables};
use super::single::{self, solution, Mode, Nesting};
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
                    polynomials.push(object_equation(ctx.c, list.object(k)?)?);
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
                polynomials.push(object_equation(ctx.c, id)?);
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
            // solver, which answers a linear equation in that variable
            // alone and nothing else.
            let equation = polynomials.pop()?;
            let axis = usize::from(axes[0]);
            let linear = equation.denominator.scalar().is_some()
                && equation
                    .numerator
                    .0
                    .keys()
                    .all(|p| p[axis] <= 1 && (0..3).all(|j| j == axis || p[j] == 0))
                && equation.numerator.0.keys().any(|p| p[axis] == 1);
            if !linear {
                return None;
            }
            let mode = mode(Nesting::Flat);
            return single::univariate(ctx, equation, axis, axes[0], mode, None, work);
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

fn object_equation(c: &crate::Construction, id: crate::Id) -> Option<crate::cas::Quotient> {
    match c.kind(id) {
        Kind::Line => {
            let Value::Line { a, b, c } = c.value(id) else {
                return None;
            };
            as_equation(&[([1, 0, 0], a), ([0, 1, 0], b), ([0, 0, 0], c)])
        }
        Kind::Conic | Kind::Circle => {
            let [xx, yy, c, xy, x, y] = c.conic(id)?.matrix;
            as_equation(&[
                ([2, 0, 0], xx),
                ([0, 2, 0], yy),
                ([0, 0, 0], c),
                ([1, 1, 0], 2.0 * xy),
                ([1, 0, 0], 2.0 * x),
                ([0, 1, 0], 2.0 * y),
            ])
        }
        Kind::ImplicitCurve => {
            // Rows are the powers of x, columns the powers of y.
            let matrix = c.implicit_coefficients(id)?;
            let terms: Vec<([u8; 3], f64)> = matrix
                .iter()
                .enumerate()
                .flat_map(|(i, row)| {
                    row.iter().enumerate().map(move |(j, &v)| {
                        Some(([u8::try_from(i).ok()?, u8::try_from(j).ok()?, 0], v))
                    })
                })
                .collect::<Option<_>>()?;
            as_equation(&terms)
        }
        _ => None,
    }
}

fn as_equation(terms: &[([u8; 3], f64)]) -> Option<crate::cas::Quotient> {
    // What a cancellation leaves of a coefficient that is zero, a few units
    // in the last place of the largest one, is zero.
    let largest = terms.iter().fold(0.0_f64, |m, (_, v)| m.max(v.abs()));
    let coefficients = terms
        .iter()
        .filter(|(_, v)| v.abs() > largest * 1e-13)
        .map(|&(p, v)| Some((p, fraction(v)?)))
        .collect::<Option<_>>()?;
    Some(crate::cas::Quotient::polynomial(Poly(coefficients)))
}

/// The coefficient of an object as the CAS reads it. The reference sends
/// the equation of a line or a conic as it was entered, so `x y / 3` holds
/// one third and not its nearest double: a fraction within a few units in
/// the last place is that number, found by continued fractions. Its
/// denominator `q` is kept so small that a number that is no fraction is
/// not taken for one: fractions up to `q` lie `1 / q^2` apart, and the
/// window around the double is a hundred million times narrower. Any other
/// double is the decimal of its 15 significant digits, which is what the
/// reference prints for the CAS.
fn fraction(value: f64) -> Option<Rat> {
    let magnitude = value.abs();
    let tolerance = magnitude * 4.0 * f64::EPSILON;
    let (mut p0, mut q0, mut p1, mut q1) = (0_i128, 1_i128, 1_i128, 0_i128);
    let mut rest = magnitude;
    // The denominators at least double every second term.
    for _ in 0..40 {
        let whole = rest.floor();
        if !(whole < 1e15) {
            break;
        }
        let term = whole as i128;
        let (p, q) = (term * p1 + p0, term * q1 + q0);
        if q > 1 && (q as f64) * (q as f64) * tolerance > 1e-8 {
            break;
        }
        if (p as f64 / q as f64 - magnitude).abs() <= tolerance {
            return Rat::new(if value < 0.0 { -p } else { p }, q);
        }
        (p0, q0, p1, q1) = (p1, q1, p, q);
        let part = rest - whole;
        if part <= 0.0 {
            break;
        }
        rest = part.recip();
    }
    let text = format!("{value:.14e}");
    let (mantissa, exponent) = text.split_once('e')?;
    let digits = mantissa.replace('.', "").parse().ok()?;
    Rat::decimal(digits, exponent.parse::<i64>().ok()? - 14)
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

#[cfg(test)]
mod tests {
    use super::{fraction, Rat};

    #[test]
    fn a_coefficient_near_a_simple_fraction_is_that_fraction() {
        let rat = |n, d| Rat::new(n, d);
        assert_eq!(fraction(1.0 / 3.0), rat(1, 3));
        assert_eq!(fraction(1.0 / 3.0 - 0.5), rat(-1, 6));
        assert_eq!(fraction(0.1 + 0.2), rat(3, 10));
        assert_eq!(fraction(-2.5), rat(-5, 2));
        assert_eq!(fraction(7.0), rat(7, 1));
        assert_eq!(fraction(0.0), rat(0, 1));
        assert_eq!(fraction(0.3333), rat(3333, 10000));
        // No fraction: 15 significant digits.
        assert_eq!(
            fraction(std::f64::consts::SQRT_2),
            rat(141_421_356_237_310, 100_000_000_000_000)
        );
        // A convergent with a large denominator that happens to be close
        // is no evidence of a fraction.
        let close = -108_212_005.0 / 265_251.0;
        assert_eq!(fraction(close).map(|r| r.den > 265_251), Some(true));
        assert_eq!(fraction(f64::NAN), None);
        assert_eq!(fraction(f64::INFINITY), None);
    }
}
