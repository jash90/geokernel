//! One-row evaluation follows GeoCasCell, CASInputHandler and CASCellProcessor
//! (GeoGebra 112919f, EUPL 1.2).

mod display;
#[cfg(feature = "giac")]
mod translation;

use super::syntax::{parse, Expr};
use super::{functional, input, neighboring_prime, solutions, Op};
use crate::cas::{real_roots, Quotient, Rat, Roots, Work};
use crate::{Construction, Value};

pub(super) const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_EXACT_UNITS: usize = 10_000;
const BUILTINS: &[&str] = &[
    "sqrt", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "exp",
    "ln", "log", "floor", "ceil",
];

/// The value and display strings of one evaluated CAS row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CasRow {
    /// The cell's output text, with `?` for an undefined result.
    pub text: String,
    /// The cell's LaTeX output, including its bold wrapper.
    pub latex: String,
    /// The exact result in CAS expression syntax, when available.
    pub exact: Option<String>,
}

impl Construction {
    /// Evaluates one CAS row against the current construction without changing it.
    ///
    /// Each call is independent. CAS assignments and references to other CAS rows
    /// require a CAS session. Equations and references to construction objects
    /// can be evaluated directly.
    pub fn evaluate_cas_row(&self, source: &str) -> CasRow {
        let result = evaluate(self, source).unwrap_or_else(undefined);
        display::render(&result)
    }
}

fn undefined() -> Expr {
    Expr::Name("?".into())
}

fn evaluate(c: &Construction, source: &str) -> Option<Expr> {
    if source.len() > MAX_INPUT_BYTES {
        return None;
    }
    let source = source.trim();
    let source = source.strip_suffix('=').unwrap_or(source);
    let tree = parse(source, true)?;
    #[cfg(feature = "giac")]
    match geo_giac::backend() {
        geo_giac::Backend::Giac => return translation::evaluate(c, &tree).map(cell_equation),
        geo_giac::Backend::Failed => return None,
        geo_giac::Backend::KernelOnly => {}
    }

    let meter = crate::expr::eval::Lists::new(Vec::new());
    let charge = |n| meter.charge(n as u64);
    let result = exact(c, tree, &mut Work::new(&charge))?;
    // Also checks depth after objects and nested equations have been expanded.
    super::syntax::bounded_tree(&result)?;
    let syntax = display::syntax(&result);
    if syntax.encode_utf16().count() > MAX_EXACT_UNITS {
        return None;
    }
    parse(&syntax, false).map(cell_equation)
}

// GeoCasCell reads an equation on the right as the two equations obtained by
// placing the left side on each side of the inner equation.
fn cell_equation(tree: Expr) -> Expr {
    if let Expr::Eq(left, right) = &tree {
        if let Expr::Eq(a, b) = &**right {
            return Expr::Tuple(vec![
                Expr::Eq(left.clone(), a.clone()),
                Expr::Eq(left.clone(), b.clone()),
            ]);
        }
    }
    tree
}

fn exact(c: &Construction, tree: Expr, work: &mut Work<'_>) -> Option<Expr> {
    work.spend(1)?;
    let calculate = |tree, work: &mut Work<'_>| exact(c, tree, work);
    Some(match tree {
        Expr::Name(name) => match c.resolve_label(&name) {
            Some(id) => match c.value(id) {
                Value::Line { a, b, c: offset } => {
                    let left = format!("({a})*x + ({b})*y");
                    let left = parse(&left, false)?;
                    let left = reduce(left, work)?;
                    let right = reduce(numeric(-offset), work)?;
                    Expr::Eq(Box::new(left), Box::new(right))
                }
                Value::Number(_) => rational(input::row_number(c, id, work)?),
                _ => return None,
            },
            None => Expr::Name(name),
        },
        Expr::Bin(op, a, b) => {
            let a = calculate(*a, work)?;
            let b = calculate(*b, work)?;
            match (a, b) {
                (Expr::Name(n), _) | (_, Expr::Name(n)) if n == "?" => undefined(),
                (Expr::Eq(..), Expr::Eq(..)) => return None,
                (Expr::Eq(a, b), other) => {
                    work.spend(super::syntax::bounded_tree(&other)?)?;
                    let left = reduce(binary(op, *a, other.clone()), work)?;
                    let right = reduce(binary(op, *b, other), work)?;
                    Expr::Eq(Box::new(left), Box::new(right))
                }
                (other, Expr::Eq(a, b)) => {
                    work.spend(super::syntax::bounded_tree(&other)?)?;
                    let left = reduce(binary(op, other.clone(), *a), work)?;
                    let right = reduce(binary(op, other, *b), work)?;
                    Expr::Eq(Box::new(left), Box::new(right))
                }
                (a, b) => reduce(binary(op, a, b), work)?,
            }
        }
        Expr::Neg(a) => match calculate(*a, work)? {
            Expr::Name(n) if n == "?" => undefined(),
            value => reduce(Expr::Neg(Box::new(value)), work)?,
        },
        Expr::Call(name, args) => {
            if let ("Solve" | "Solutions", [equation]) = (name.as_str(), args.as_slice()) {
                return solve(c, &name, equation, work);
            }
            let args = args
                .into_iter()
                .map(|arg| calculate(arg, work))
                .collect::<Option<Vec<_>>>()?;
            match (name.as_str(), args.as_slice()) {
                ("NextPrime" | "PreviousPrime", [arg]) => {
                    let n = quotient(arg, work)?.scalar()?;
                    numeric(neighboring_prime(n.to_f64(), name == "NextPrime", work)?)
                }
                ("Evaluate", [arg]) => arg.clone(),
                ("Simplify", [arg @ Expr::Eq(..)]) => arg.clone(),
                (name, [arg]) if Op::named(name).is_some_and(Op::is_function) => {
                    let op = Op::named(name)?;
                    let result = functional::transform(op, quotient(arg, work)?, None, work)?;
                    parse(&result.row_text(), false)?
                }
                ("!", [arg]) => {
                    let value = quotient(arg, work)?.scalar()?;
                    if value.den != 1 || !(0..=33).contains(&value.num) {
                        return None;
                    }
                    work.spend(value.num as usize)?;
                    integer((1..=value.num).product())
                }
                ("abs", [arg]) => match quotient(arg, work).and_then(|value| value.scalar()) {
                    Some(value) if value.num < 0 => rational(value.neg()?),
                    Some(value) => rational(value),
                    None => Expr::Call(name, args),
                },
                ("sqrt", [arg]) => {
                    let square = |n: Rat| {
                        let num = u128::try_from(n.num).ok()?.isqrt();
                        let den = (n.den as u128).isqrt();
                        (num * num == n.num as u128 && den * den == n.den as u128)
                            .then(|| Rat::new(num as i128, den as i128))
                            .flatten()
                    };
                    if let Some(root) = quotient(arg, work).and_then(|q| square(q.scalar()?)) {
                        rational(root)
                    } else {
                        Expr::Call(name, args)
                    }
                }
                (name, [_]) if BUILTINS.contains(&name) => Expr::Call(name.into(), args),
                _ => return None,
            }
        }
        Expr::Eq(a, b) => Expr::Eq(
            if matches!(&*a, Expr::Call(..)) {
                a
            } else {
                Box::new(calculate(*a, work)?)
            },
            Box::new(calculate(*b, work)?),
        ),
        Expr::List(items) => Expr::List(
            items
                .into_iter()
                .map(|item| calculate(item, work))
                .collect::<Option<Vec<_>>>()?,
        ),
        Expr::Tuple(items) => Expr::Tuple(
            items
                .into_iter()
                .map(|item| calculate(item, work))
                .collect::<Option<Vec<_>>>()?,
        ),
        Expr::Num(..) => reduce(tree, work)?,
    })
}

fn binary(op: u8, a: Expr, b: Expr) -> Expr {
    Expr::Bin(op, Box::new(a), Box::new(b))
}

fn numeric(n: f64) -> Expr {
    Expr::Num(n, None)
}

fn integer(n: i128) -> Expr {
    if n < 0 {
        return Expr::Neg(Box::new(Expr::Num(
            n.unsigned_abs() as f64,
            Some(n.unsigned_abs().to_string()),
        )));
    }
    Expr::Num(n as f64, Some(n.to_string()))
}

fn rational(value: Rat) -> Expr {
    if value.den == 1 {
        integer(value.num)
    } else {
        binary(b'/', integer(value.num), integer(value.den))
    }
}

fn reduce(tree: Expr, work: &mut Work<'_>) -> Option<Expr> {
    if let Expr::Bin(b'/', _, denominator) = &tree {
        if quotient(denominator, work).and_then(|q| q.scalar()) == Some(Rat::ZERO) {
            return Some(undefined());
        }
    }
    if let Some(value) = quotient(&tree, work) {
        if let Some(value) = value.scalar() {
            return Some(rational(value));
        }
        return parse(
            &functional::transform(Op::Simplify, value, None, work)?.to_string(),
            false,
        );
    }
    Some(tree)
}

fn quotient(tree: &Expr, work: &mut Work<'_>) -> Option<Quotient> {
    quotient_with(tree, work, &mut |leaf, _| {
        let Expr::Name(name) = leaf else {
            return None;
        };
        let axis = ["x", "y", "z"]
            .iter()
            .position(|variable| name == *variable)?;
        Some(Quotient::polynomial(crate::cas::Poly::variable(axis)))
    })
}

fn quotient_with(
    tree: &Expr,
    work: &mut Work<'_>,
    leaf_value: &mut impl FnMut(&Expr, &mut Work<'_>) -> Option<Quotient>,
) -> Option<Quotient> {
    work.spend(1)?;
    Some(match tree {
        Expr::Num(_, Some(text)) => input::number(input::from_decimal(
            &crate::expr::decimal::Dec::parse(text)?,
        )?),
        Expr::Num(n, None) => input::number(Rat::from_value(*n)?),
        Expr::Neg(a) => input::number(Rat::ZERO).sub(quotient_with(a, work, leaf_value)?, work)?,
        Expr::Bin(op, a, b) => {
            let a = quotient_with(a, work, leaf_value)?;
            let b = quotient_with(b, work, leaf_value)?;
            match op {
                b'+' => a.add(b, work)?,
                b'-' => a.sub(b, work)?,
                b'*' => a.mul(b, work)?,
                b'/' => a.div(b, work)?,
                b'^' => {
                    let exponent = b.scalar()?;
                    if exponent.den != 1 {
                        return None;
                    }
                    a.pow(exponent.num, work)?
                }
                _ => return None,
            }
        }
        _ => leaf_value(tree, work)?,
    })
}

/// A one-argument solve prefers x when it occurs, then the first free name.
fn solve_variable<'a>(c: &Construction, equation: &'a Expr) -> &'a str {
    let mut first = None;
    let mut pending = vec![equation];
    while let Some(tree) = pending.pop() {
        match tree {
            Expr::Name(name)
                if !matches!(name.as_str(), "pi" | "e" | "inf" | "?")
                    && c.resolve_label(name).is_none() =>
            {
                if name == "x" {
                    return name;
                }
                first.get_or_insert(name.as_str());
            }
            Expr::Neg(a) => pending.push(a),
            Expr::Bin(_, a, b) | Expr::Eq(a, b) => pending.extend([&**b, &**a]),
            Expr::Call(_, items) | Expr::List(items) | Expr::Tuple(items) => {
                pending.extend(items.iter().rev());
            }
            _ => {}
        }
    }
    first.unwrap_or("x")
}

fn solve(c: &Construction, name: &str, equation: &Expr, work: &mut Work<'_>) -> Option<Expr> {
    let line;
    let equation = if matches!(equation, Expr::Name(label)
        if c.resolve_label(label).is_some_and(|id| matches!(c.value(id), Value::Line { .. })))
    {
        line = exact(c, equation.clone(), work)?;
        &line
    } else {
        equation
    };
    let variable = solve_variable(c, equation);
    let mut resolve_name = |leaf: &Expr, work: &mut Work<'_>| {
        let Expr::Name(name) = leaf else {
            return None;
        };
        if let Some(id) = c.resolve_label(name) {
            return Some(input::number(input::row_number(c, id, work)?));
        }
        (name == variable).then(|| Quotient::polynomial(crate::cas::Poly::variable(0)))
    };
    let mut resolve = |leaf: &Expr, work: &mut Work<'_>| match leaf {
        Expr::Call(..) => {
            let value = exact(c, leaf.clone(), work)?;
            quotient_with(&value, work, &mut resolve_name)
        }
        _ => resolve_name(leaf, work),
    };
    // Keep the original denominator until roots outside the domain have been removed.
    let polynomial = match equation {
        Expr::Eq(a, b) => {
            let a = quotient_with(a, work, &mut resolve)?;
            let b = quotient_with(b, work, &mut resolve)?;
            a.sub(b, work)?
        }
        expression => quotient_with(expression, work, &mut resolve)?,
    };
    let denominator = solutions::dense_at(&polynomial.denominator, 0);
    let Roots::Finite(roots) = real_roots(&polynomial.numerator.coefficients()?, work)? else {
        return (denominator.len() == 1).then(|| Expr::List(vec![Expr::Name(variable.into())]));
    };
    let mut results = Vec::new();
    for root in roots {
        work.spend(denominator.len())?;
        if solutions::is_pole(root, &denominator) {
            continue;
        }
        let value = parse(&solutions::exact(root)?, false)?;
        results.push(if name == "Solve" {
            Expr::Eq(Box::new(Expr::Name(variable.into())), Box::new(value))
        } else {
            value
        });
    }
    Some(Expr::List(results))
}
