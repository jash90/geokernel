//! One-row evaluation follows GeoCasCell, CASInputHandler and CASCellProcessor
//! (GeoGebra 112919f, EUPL 1.2).

mod display;
#[cfg(feature = "giac")]
mod translation;

use super::syntax::{parse, Expr};
use super::{functional, input, neighboring_prime, solutions, Op};
use crate::cas::{real_roots, Quotient, Rat, Roots, Work};
use crate::{Construction, Value};

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_EXACT_UNITS: usize = 10_000;
const BUILTINS: &[&str] = &[
    "sqrt", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "exp",
    "ln", "log", "floor", "ceil",
];

/// The value and display strings of one evaluated CAS row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CasRow {
    /// The cell's output text or error message, with `?` for an undefined result.
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
        evaluate(self, source).unwrap_or_else(|| display::render(&undefined()))
    }
}

fn undefined() -> Expr {
    Expr::Name("?".into())
}

fn evaluate(c: &Construction, source: &str) -> Option<CasRow> {
    if source.len() > MAX_INPUT_BYTES {
        return None;
    }
    let source = source.trim();
    let source = source.strip_suffix('=').unwrap_or(source);
    let tree = parse(source, true)?;
    #[cfg(feature = "giac")]
    match geo_giac::backend() {
        geo_giac::Backend::Giac => return translation::evaluate(c, &tree),
        geo_giac::Backend::Failed => return None,
        geo_giac::Backend::KernelOnly => {}
    }

    if let Some(command) = feature_command(&tree) {
        return Some(CasRow {
            text: format!("{command} requires a CAS build"),
            latex: String::new(),
            exact: None,
        });
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
    Some(display::render(&cell_equation(parse(&syntax, false)?)))
}

fn feature_command(tree: &Expr) -> Option<&str> {
    match tree {
        Expr::Call(name, _) if is_feature_command(name) => Some(name),
        Expr::Call(_, items) | Expr::List(items) | Expr::Tuple(items) => {
            items.iter().find_map(feature_command)
        }
        Expr::Neg(a) => feature_command(a),
        Expr::Bin(_, a, b) | Expr::Eq(a, b) => feature_command(a).or_else(|| feature_command(b)),
        Expr::Name(_) | Expr::Num(..) => None,
    }
}

fn is_feature_command(name: &str) -> bool {
    matches!(
        name,
        "Assume"
            | "CFactor"
            | "CIFactor"
            | "IntegralSymbolic"
            | "Eliminate"
            | "GroebnerLex"
            | "GroebnerDegRevLex"
            | "GroebnerLexDeg"
            | "Numeric"
            | "MixedNumber"
            | "Rationalize"
            | "SolveCubic"
            | "JordanDiagonalization"
            | "Eigenvectors"
            | "Eigenvalues"
            | "Laplace"
            | "InverseLaplace"
            | "Substitute"
            | "ToExponential"
            | "ExtendedGCD"
            | "ModularExponent"
            | "CharacteristicPolynomial"
            | "MinimalPolynomial"
            | "LUDecomposition"
            | "QRDecomposition"
            | "SolveQuartic"
    )
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
                Value::Number(_) => rational(input::object(c, id)?),
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
            let args = args
                .into_iter()
                .map(|arg| calculate(arg, work))
                .collect::<Option<Vec<_>>>()?;
            match (name.as_str(), args.as_slice()) {
                ("Solve" | "Solutions", [equation]) => solve(&name, equation, work)?,
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
        Expr::Num(..) => tree,
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
    work.spend(1)?;
    let read = |tree, work: &mut Work<'_>| quotient(tree, work);
    Some(match tree {
        Expr::Num(_, Some(text)) => input::number(input::from_decimal(
            &crate::expr::decimal::Dec::parse(text)?,
        )?),
        Expr::Num(n, None) => input::number(Rat::from_value(*n)?),
        Expr::Name(n) => {
            let axis = ["x", "y", "z"].iter().position(|name| n == name)?;
            Quotient::polynomial(crate::cas::Poly::variable(axis))
        }
        Expr::Neg(a) => input::number(Rat::ZERO).sub(read(a, work)?, work)?,
        Expr::Bin(op, a, b) => {
            let a = read(a, work)?;
            let b = read(b, work)?;
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
        _ => return None,
    })
}

fn solve(name: &str, equation: &Expr, work: &mut Work<'_>) -> Option<Expr> {
    let polynomial = match equation {
        Expr::Eq(a, b) => quotient(a, work)?.sub(quotient(b, work)?, work)?,
        expression => quotient(expression, work)?,
    };
    let Roots::Finite(roots) = real_roots(&polynomial.coefficients(work)?, work)? else {
        return Some(Expr::List(vec![Expr::Name("x".into())]));
    };
    let results = roots
        .into_iter()
        .map(|root| {
            let value = parse(&solutions::exact(root)?, false)?;
            Some(if name == "Solve" {
                Expr::Eq(Box::new(Expr::Name("x".into())), Box::new(value))
            } else {
                value
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Expr::List(results))
}
