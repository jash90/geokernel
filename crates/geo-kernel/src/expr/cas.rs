//! Adapter between expression trees and the exact polynomial engine.

mod functional;
mod input;
mod solutions;

use super::eval::{Ctx, Ex, Val};
use super::{Node, Slot, Ty};
use crate::cas::{Poly, Quotient, Rat, Work};
use crate::Kind;

pub(super) use functional::{evaluate as function_at, function, print_poly, Function};
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
                    return Ex::plain(N(result.value(x, y)));
                }
            }
            let value =
                functional::evaluate(self, op, a, b).map_or(f64::NAN, |result| result.value(x, y));
            return Ex::plain(N(value));
        }
        let allowance = super::eval::Lists::new(Vec::new());
        let charge = |n| self.meter.unwrap_or(&allowance).charge(n as u64);
        let work = &mut Work::new(&charge);
        match op {
            Op::NextPrime | Op::PreviousPrime => {
                let n = self.full_number(a);
                Ex::plain(N(
                    neighboring_prime(n, op == Op::NextPrime, work).map_or(f64::NAN, |n| n as f64)
                ))
            }
            Op::Degree => {
                let value = read(self.c, self.e, a, &input::variables(), work)
                    .and_then(|arg| arg.coefficients(work))
                    .map_or(f64::NAN, |p| p.len().saturating_sub(1) as f64);
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
                let mut coefficients =
                    read(self.c, self.e, a, &input::variables(), work)?.coefficients(work)?;
                if coefficients.is_empty() {
                    coefficients.push(Rat::ZERO);
                }
                self.cas_slots(
                    coefficients
                        .into_iter()
                        .rev()
                        .map(|r| Slot::number(r.to_f64())),
                )
            }
            Op::Factors => functional::factor_list(self, a, work),
            Op::Solve { .. } => solutions::list(self, a, op, work),
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

fn neighboring_prime(n: f64, next: bool, work: &mut Work<'_>) -> Option<u64> {
    const MAX_EXACT_INTEGER: u64 = 1 << 53;
    if !n.is_finite() || n.fract() != 0.0 || n.abs() > MAX_EXACT_INTEGER as f64 {
        return None;
    }
    if next && n < 2.0 {
        return Some(2);
    }
    if !next && n <= 2.0 {
        return None;
    }
    let mut candidate = if next {
        n.floor() as u64 + 1
    } else {
        n.ceil() as u64 - 1
    };
    loop {
        if candidate > MAX_EXACT_INTEGER {
            return None;
        }
        work.spend(1)?;
        if super::commands::prime(candidate, |units| work.spend(units as usize).is_none())? {
            return Some(candidate);
        }
        candidate = if next { candidate + 1 } else { candidate - 1 };
    }
}

pub(crate) fn displayed(c: &crate::Construction, id: crate::Id) -> Option<String> {
    let crate::Def::Expr(def) = c.def(id) else {
        return None;
    };
    let entry = super::entry(c, def);
    if let super::StoredExtra::Cas(form) = &entry.extra {
        return Some(form.to_string());
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
