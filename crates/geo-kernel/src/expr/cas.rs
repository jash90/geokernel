//! Adapter between expression trees and the exact polynomial engine.

mod functional;
#[cfg(feature = "giac")]
mod giac;
mod input;
mod solutions;

use super::equation::Builder;
use super::eval::{Ctx, Ex, Val};
use super::{Entry, Node, Slot, Ty};
use crate::cas::{Poly, Quotient, Rat, Work};
use crate::{Construction, Kind};

pub(super) use functional::{evaluate as function_at, function, print_poly, FunctionResult};
use input::read;

pub(super) const NAMES: &[&str] = &[
    "Expand",
    "Factor",
    "IFactor",
    "Simplify",
    "CompleteSquare",
    "CommonDenominator",
    "Degree",
    "Coefficients",
    "IsFactored",
    "NextPrime",
    "PreviousPrime",
    "Solve",
    "Solutions",
    "NSolve",
    "NSolutions",
    "Polynomial",
    "Numerator",
    "Denominator",
    "Factors",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Expand,
    Factor,
    IFactor,
    Simplify,
    CompleteSquare,
    CommonDenominator,
    Polynomial,
    Numerator,
    Denominator,
    Factors,
    Degree,
    Coefficients,
    IsFactored,
    NextPrime,
    PreviousPrime,
    Solve {
        assignments: bool,
        approximate: bool,
        width: u8,
        axes: [u8; 3],
        system: bool,
    },
}

impl Op {
    pub(super) fn named(name: &str) -> Option<Self> {
        Some(match name {
            "Expand" => Self::Expand,
            "Factor" => Self::Factor,
            "IFactor" => Self::IFactor,
            "Simplify" => Self::Simplify,
            "CompleteSquare" => Self::CompleteSquare,
            "CommonDenominator" => Self::CommonDenominator,
            "Polynomial" => Self::Polynomial,
            "Numerator" => Self::Numerator,
            "Denominator" => Self::Denominator,
            "Factors" => Self::Factors,
            "Degree" => Self::Degree,
            "Coefficients" => Self::Coefficients,
            "IsFactored" => Self::IsFactored,
            "NextPrime" => Self::NextPrime,
            "PreviousPrime" => Self::PreviousPrime,
            "Solve" | "Solutions" | "NSolve" | "NSolutions" => Self::Solve {
                assignments: false,
                approximate: false,
                width: 0,
                axes: [0; 3],
                system: false,
            },
            _ => return None,
        })
    }

    pub(super) fn is_function(self) -> bool {
        matches!(
            self,
            Self::Expand
                | Self::Factor
                | Self::IFactor
                | Self::Simplify
                | Self::CompleteSquare
                | Self::CommonDenominator
                | Self::Polynomial
                | Self::Numerator
                | Self::Denominator
        )
    }

    pub(super) fn name(self) -> &'static str {
        NAMES
            .iter()
            .copied()
            .find(|name| Self::named(name) == Some(self))
            .unwrap_or("Solve")
    }

    pub(super) fn result_type(self, a: Ty, b: Option<Ty>) -> Result<Ty, String> {
        use super::Elem;
        let numeric = |ty| matches!(ty, Ty::Num(0) | Ty::Function);
        let ty = match self {
            Self::Solve {
                system,
                assignments,
                ..
            } => Ty::List(if system {
                Elem::List
            } else if assignments {
                Elem::Any
            } else {
                Elem::Num
            }),
            Self::Coefficients if numeric(a) || a == Ty::Object => Ty::List(Elem::Num),
            Self::Degree if numeric(a) => Ty::Num(0),
            Self::Factors if numeric(a) => Ty::List(Elem::Any),
            Self::IsFactored if numeric(a) => Ty::Bool,
            Self::NextPrime | Self::PreviousPrime if a == Ty::Num(0) => Ty::Num(0),
            Self::CommonDenominator if numeric(a) && b.is_some_and(numeric) => Ty::Num(0),
            Self::Numerator | Self::Denominator if matches!(a, Ty::Num(_)) => Ty::Num(0),
            op if op.is_function() && b.is_none() && numeric(a) => Ty::Num(0),
            _ => return Err(format!("invalid arguments to `{}`", self.name())),
        };
        Ok(ty)
    }
}

impl<const M: bool> Ctx<'_, M> {
    pub(super) fn cas_scalar(&self, i: u32, op: Op, a: u32, b: u32) -> Ex {
        use Val::{B, N};
        if op.is_function() {
            let x = self.cas_parameter(0);
            let y = self.cas_parameter(1);
            if let (Some((root, ..)), super::StoredExtra::Cas(result)) =
                (self.e.cas_root(), &self.e.extra)
            {
                if root == i {
                    return Ex::plain(N(result.value(self.c, x, y)));
                }
            }
            let value = functional::evaluate(self, op, a, b)
                .map_or(f64::NAN, |result| result.value(self.c, x, y));
            return Ex::plain(N(value));
        }
        let allowance = super::eval::Lists::new(Vec::new());
        let charge = |n| self.meter.unwrap_or(&allowance).charge(n as u64);
        let work = &mut Work::new(&charge);
        match op {
            Op::NextPrime | Op::PreviousPrime => {
                let n = self.full_number(a);
                Ex::plain(N(
                    neighboring_prime(n, op == Op::NextPrime, work).unwrap_or(f64::NAN)
                ))
            }
            Op::Degree => {
                let value = polynomial_argument(self.c, self.e, a, work)
                    .map_or(f64::NAN, |coeffs| coeffs.len().saturating_sub(1) as f64);
                Ex::plain(N(value))
            }
            Op::IsFactored => {
                let result = input::is_factored(self.c, self.e, a, work);
                Ex::plain(B(result.unwrap_or(false), result.is_some()))
            }
            _ => Ex::plain(N(f64::NAN)),
        }
    }

    pub(super) fn cas_list(&self, op: Op, a: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let charge = |n| lists.charge(n as u64);
        let work = &mut Work::new(&charge);
        match op {
            Op::Coefficients => {
                if let Node::Obj(id) = self.node(a) {
                    if matches!(self.c.kind(id), Kind::Conic | Kind::Circle) {
                        let [xx, yy, c, xy, x, y] = self.c.conic(id)?.matrix;
                        let entries = [xx, yy, c, 2.0 * xy, 2.0 * x, 2.0 * y];
                        return self.cas_slots(entries.into_iter().map(Slot::number));
                    }
                }
                let coefficients =
                    if let Some(mut values) = polynomial_argument(self.c, self.e, a, work) {
                        values.reverse();
                        values
                    } else {
                        let (entry, root) = argument_tree(self.c, self.e, a)?;
                        super::constant_literals(entry, root)
                    };
                self.cas_slots(coefficients.into_iter().map(Slot::number))
            }
            Op::Factors => functional::factor_list(self, a, work),
            Op::Solve { .. } => {
                #[cfg(feature = "giac")]
                if let Some(result) = giac::solve(self, a, op) {
                    return result;
                }
                solutions::list(self, a, op, work)
            }
            _ => None,
        }
    }

    fn cas_slots(&self, values: impl ExactSizeIterator<Item = Slot>) -> Option<(u32, u32)> {
        let len = values.len().try_into().ok()?;
        let lists = self.lists?;
        let start = lists.reserve(len)?;
        for (i, value) in values.enumerate() {
            lists.set(start + i as u32, value);
        }
        Some((start, len))
    }
}

/// Resolve a named function to its current expression, including the output
/// of an earlier command that builds a function tree.
fn argument_tree<'a>(c: &'a Construction, e: &'a Entry, root: u32) -> Option<(&'a Entry, u32)> {
    let Node::FunctionObject(id) = e.nodes[root as usize] else {
        return Some((e, root));
    };
    if !c.value(id).is_defined() {
        return None;
    }
    let entry = match c.def(id) {
        crate::Def::Expr(d) => super::entry(c, d),
        crate::Def::Calculus(_) => c.calculus_record(id)?.tree.as_ref()?,
        _ => return None,
    };
    let entry = match &entry.extra {
        super::StoredExtra::Cas(result) => match result.as_ref() {
            FunctionResult::Tree(tree) => tree,
            _ => entry,
        },
        _ => entry,
    };
    Some((entry, entry.root))
}

fn polynomial_argument(
    c: &Construction,
    e: &Entry,
    root: u32,
    work: &mut Work<'_>,
) -> Option<Vec<f64>> {
    let (entry, input) = argument_tree(c, e, root)?;
    if !matches!(entry.tys[input as usize], Ty::Num(_)) {
        return None;
    }
    if let Some(values) = structural_coefficients(c, entry, input) {
        return Some(values);
    }
    let values = read(c, entry, input, &input::variables(), work)?.coefficients(work)?;
    if values.is_empty() {
        return Some(vec![0.0]);
    }
    Some(values.into_iter().map(Rat::to_f64).collect())
}

/// The coefficient of every power of `x`, evaluated, with the
/// `PolyFunction(c, validCoeffs)` shortening: the last nonzero coefficient
/// ends the list, which keeps at least one coefficient. `None` when a
/// coefficient cannot be evaluated.
///
/// The coefficient trees are evaluated with one memo over the builder's
/// arena (see [`Builder::number_values`]): the polynomial powers share
/// subtrees, and a plain evaluation of a tree like `(a x + b)^300` would
/// walk each shared subtree once per reference.
fn structural_coefficients(c: &Construction, entry: &Entry, root: u32) -> Option<Vec<f64>> {
    let mut b = Builder::function(entry);
    let poly = b.expand(c, root).ok()?;
    let roots = poly.univariate_coefficients(&mut b)?;
    let mut values = Vec::with_capacity(roots.len());
    let mut evaluated = b
        .number_values(c, roots.iter().copied().flatten())
        .into_iter();
    for root in roots {
        values.push(match root {
            Some(_) => evaluated.next().flatten()?,
            None => 0.0,
        });
    }
    let mut valid = values.len().min(1);
    for (i, &v) in values.iter().enumerate() {
        if v != 0.0 {
            valid = i + 1;
        }
    }
    values.truncate(valid);
    Some(values)
}

fn neighboring_prime(n: f64, next: bool, work: &mut Work<'_>) -> Option<f64> {
    // `AlgoNextPreviousPrime.compute` gives undefined only above the largest
    // integer a double holds exactly; every other input reaches the CAS.
    const LARGEST_INTEGER: f64 = 9007199254740992.0;
    if !n.is_finite() || n > LARGEST_INTEGER {
        return None;
    }
    let x = cas_input(n);
    if next {
        // The CAS runs `nextprime(x)`, which answers 2 below 2 whatever the
        // fractional part is and refuses a non-integer from 2 up.
        if x < 2.0 {
            return Some(2.0);
        }
        if x.fract() != 0.0 {
            return None;
        }
        if x > LARGEST_INTEGER {
            // Above the largest integer the next prime rounds back to the
            // input when the CAS prints it with 15 significant digits.
            return Some(x);
        }
        search_prime(x as u64, 1, work).map(|prime| prime as f64)
    } else {
        // The CAS runs `if (x > 2) then prevprime(x) else 0/0`.
        if x <= 2.0 {
            return None;
        }
        if x.fract() != 0.0 {
            return None;
        }
        if x > LARGEST_INTEGER {
            return Some(x);
        }
        search_prime(x as u64, -1, work).map(|prime| prime as f64)
    }
}

/// The value `init.toValueString(maxPrecision)` hands to the CAS. Integers
/// below 10^15 pass through; between 10^15 and the largest integer only a
/// 15-digit mantissa that is a power of ten survives, every other value is
/// scaled by 10^11 (recorded in golden/cas/prime.json).
fn cas_input(n: f64) -> f64 {
    const LARGEST_INTEGER: f64 = 9007199254740992.0;
    let magnitude = n.abs();
    if n.fract() != 0.0 || !(1e15..=LARGEST_INTEGER).contains(&magnitude) {
        return n;
    }
    let mantissa = (magnitude as u64 + 5) / 10;
    if mantissa == 100_000_000_000_000 {
        return n.signum() * 1e15;
    }
    n.signum() * (mantissa as f64 * 1e11)
}

fn search_prime(from: u64, step: i64, work: &mut Work<'_>) -> Option<u64> {
    let mut candidate = from as i64 + step;
    loop {
        work.spend(1)?;
        if super::commands::prime(candidate.unsigned_abs(), |units| {
            work.spend(units as usize).is_none()
        })? {
            return Some(candidate as u64);
        }
        candidate += step;
    }
}

pub(crate) fn displayed(c: &crate::Construction, id: crate::Id) -> Option<String> {
    let crate::Def::Expr(def) = c.def(id) else {
        return None;
    };
    let entry = super::entry(c, def);
    if let super::StoredExtra::Cas(form) = &entry.extra {
        return form.displayed();
    }
    if def.kind() == Kind::List && matches!(entry.nodes[entry.root as usize], Node::Cas(..)) {
        return Some(display_list(c.elements(id)?));
    }
    None
}

fn display_list(list: super::List<'_>) -> String {
    let contents = list
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if let Some(text) = list.symbolic_at(i) {
                return text;
            }
            match item {
                super::Element::List(row) => display_list(row),
                super::Element::Value(_, crate::Value::Number(value)) => format!("{value}"),
                super::Element::Value(_, crate::Value::Line { a, b: 0.0, c }) => {
                    format!("x = {}", -c / a)
                }
                super::Element::Value(_, crate::Value::Line { a: 0.0, b, c }) => {
                    format!("y = {}", -c / b)
                }
                super::Element::Value(_, crate::Value::Line { a, b, c }) => {
                    format!("{a}*x + {b}*y = {}", -c)
                }
                _ => "?".into(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{contents}}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(n: f64) -> Option<f64> {
        neighboring_prime(n, true, &mut Work::new(&|_| true))
    }

    fn previous(n: f64) -> Option<f64> {
        neighboring_prime(n, false, &mut Work::new(&|_| true))
    }

    #[test]
    fn prime_neighbors_follow_the_recorded_cas_edge_cases() {
        // CommandsTest.cmdNextPrime plus the values recorded in
        // golden/cas/prime.json with the Giac oracle.
        assert_eq!(next(3.0), Some(5.0));
        assert_eq!(next(2.0), Some(3.0));
        assert_eq!(next(1.0), Some(2.0));
        assert_eq!(next(-7.0), Some(2.0));
        assert_eq!(next(-2.5), Some(2.0));
        assert_eq!(next(3.4), None);
        assert_eq!(next(1e12), Some(1e12 + 39.0));
        assert_eq!(previous(5.0), Some(3.0));
        assert_eq!(previous(2.0), None);
        assert_eq!(previous(1.0), None);
        assert_eq!(previous(-7.0), None);
        assert_eq!(previous(-2.5), None);
        assert_eq!(next(f64::NAN), None);
    }

    #[test]
    fn prime_neighbors_answer_the_fifteen_digit_cas_input() {
        // Between 10^15 and the largest exact integer GeoGebra hands the CAS a
        // 15-digit mantissa times 10^11, where neighboring primes are
        // indistinguishable, so both commands answer that value. Above the
        // largest integer AlgoNextPreviousPrime answers undefined.
        let largest = 9007199254740992.0;
        let mangled = 9.00719925474099e25;
        assert_eq!(next(largest - 1.0), Some(mangled));
        assert_eq!(next(largest), Some(mangled));
        assert_eq!(previous(largest), Some(mangled));
        assert_eq!(next(9007199254741990.0), None);
        assert_eq!(previous(9007199254741990.0), None);
    }

    #[test]
    fn cas_input_rounds_the_mantissa_like_the_display() {
        assert_eq!(cas_input(3.0), 3.0);
        assert_eq!(cas_input(3.4), 3.4);
        assert_eq!(cas_input(-10.0), -10.0);
        assert_eq!(cas_input(1e15), 1e15);
        assert_eq!(cas_input(-1e15), -1e15);
        assert_eq!(cas_input(2e15), 2.0e25);
        assert_eq!(cas_input(9007199254740991.0), 9.00719925474099e25);
        assert_eq!(cas_input(9007199254741990.0), 9007199254741990.0);
    }
}
