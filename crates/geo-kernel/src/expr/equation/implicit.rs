// Licensed under the EUPL 1.2.

use super::{build_normal, Builder, Entry, NotPolynomial};
use crate::algo::{conic_nd::ConicData, general_conic};
use crate::expr::{eval, parse, shift_children, Raw};
use crate::{Construction, Def, Error, Id, Kind, Value};

/// Coefficients indexed by the power of x, then by the power of y.
pub type Coefficients = Vec<Vec<f64>>;

pub(crate) const MAX_DEGREE: u32 = 32;

#[derive(Debug)]
pub(crate) struct Record {
    pub(super) sides: Option<[u32; 2]>,
    pub(crate) kind: Kind,
    pub(crate) coefficients: Coefficients,
    pub(crate) conic: Option<ConicData>,
    fixed: bool,
    fingerprint: u64,
}

pub(super) fn definition(
    c: &mut Construction,
    label: &str,
    sides: [parse::Parsed; 2],
) -> Result<Id, Error> {
    let first = c.exprs.entries.len();
    for parsed in sides {
        let entry = Entry::new(c, parsed, true);
        c.exprs.entries.push(entry);
    }
    let result = add(
        c,
        label,
        Record {
            sides: Some([first as u32, first as u32 + 1]),
            kind: Kind::ImplicitCurve,
            coefficients: Vec::new(),
            conic: None,
            fixed: false,
            fingerprint: 0,
        },
    );
    if let Ok(id) = result {
        c.set_equation_form(id, crate::kernel::EquationForm::User)?;
    } else {
        c.exprs.entries.truncate(first);
    }
    result
}

impl Construction {
    /// Adds a saved implicit polynomial with all coefficients given explicitly.
    /// An empty matrix represents an undefined saved curve.
    pub fn add_implicit_coefficients(
        &mut self,
        label: &str,
        coefficients: Coefficients,
    ) -> Result<Id, Error> {
        let size = MAX_DEGREE as usize + 1;
        if coefficients.len() > size
            || coefficients.iter().any(|row| row.len() > size)
            || coefficients.iter().flatten().any(|n| !n.is_finite())
        {
            return Err(Error::UnknownObject);
        }
        let id = add(
            self,
            label,
            Record {
                sides: None,
                kind: Kind::ImplicitCurve,
                fingerprint: fingerprint(&coefficients),
                coefficients,
                conic: None,
                fixed: true,
            },
        )?;
        self.set_equation_form(id, crate::kernel::EquationForm::Implicit)?;
        Ok(id)
    }

    /// The current implicit polynomial, indexed by the powers of x and y.
    pub fn implicit_coefficients(&self, id: Id) -> Option<&[Vec<f64>]> {
        let Def::ImplicitCurve(index) = self.def(id) else {
            return None;
        };
        let record = &self.implicit_curves[index as usize].1;
        (record.kind == Kind::ImplicitCurve && self.value(id).is_defined())
            .then_some(&record.coefficients)
    }

    /// A cache key for the current implicit coefficients.
    pub fn implicit_revision(&self, id: Id) -> Option<u64> {
        let Def::ImplicitCurve(index) = self.def(id) else {
            return None;
        };
        Some(self.implicit_curves[index as usize].1.fingerprint)
    }

    /// The degree of the current polynomial after evaluating its coefficients.
    pub fn implicit_degree(&self, id: Id) -> Option<usize> {
        Some(
            self.implicit_coefficients(id)?
                .iter()
                .enumerate()
                .flat_map(|(x, row)| {
                    row.iter()
                        .enumerate()
                        .filter_map(move |(y, &v)| (v != 0.0).then_some(x + y))
                })
                .max()
                .unwrap_or(0),
        )
    }
}

fn add(c: &mut Construction, label: &str, record: Record) -> Result<Id, Error> {
    let index = c.implicit_curves.len() as u32;
    c.implicit_curves.push((c.len(), record));
    let result = c.add(label, Def::ImplicitCurve(index));
    if result.is_err() {
        c.implicit_curves.pop();
    }
    result
}

pub(crate) fn parents(c: &Construction, index: u32) -> Vec<Id> {
    let mut parents = Vec::new();
    for entry in c.implicit_curves[index as usize]
        .1
        .sides
        .into_iter()
        .flatten()
    {
        parents.extend(&c.exprs.entries[entry as usize].parents);
    }
    parents.sort_unstable();
    parents.dedup();
    parents
}

fn normal(
    c: &Construction,
    lhs: &Entry,
    rhs: &Entry,
) -> Result<(u32, Coefficients), NotPolynomial> {
    let mut builder = Builder::function(lhs);
    let offset = builder.nodes.len() as u32;
    builder.nodes.extend(rhs.nodes.iter().map(|node| {
        let mut node = *node;
        shift_children(&mut node, offset);
        node
    }));
    builder.lits.extend(rhs.lits.iter().cloned());
    builder.max_degree = MAX_DEGREE;
    let poly = build_normal(c, &mut builder, lhs.root, offset + rhs.root)?;
    let degree = poly.degree();
    if degree > MAX_DEGREE {
        return Err(NotPolynomial::ImplicitCurve);
    }
    let (nx, ny) = poly
        .0
        .iter()
        .fold((0, 0), |(x, y), term| (x.max(term.x), y.max(term.y)));
    let mut coefficients = vec![vec![0.0; ny as usize + 1]; nx as usize + 1];
    let mut roots = vec![None; (nx as usize + 1) * (ny as usize + 1)];
    for term in poly.0 {
        let slot = &mut roots[(term.x * (ny + 1) + term.y) as usize];
        *slot = Some(slot.map_or(term.coeff, |root| builder.add(root, term.coeff)));
    }
    let mut values = builder
        .number_values(c, roots.iter().flatten().copied())
        .into_iter();
    for (root, slot) in roots.into_iter().zip(coefficients.iter_mut().flatten()) {
        if let Some(root) = root {
            *slot = values
                .next()
                .flatten()
                .or_else(|| coefficient_value(c, &builder, root))
                .unwrap_or(f64::NAN);
        }
    }
    Ok((degree, coefficients))
}

fn coefficient_value(c: &Construction, builder: &Builder, root: u32) -> Option<f64> {
    let parsed = crate::expr::reparse(c, Kind::Number, &builder.nodes, &builder.lits, root)?;
    match eval::root(c, &Entry::new(c, parsed, false), None).0 {
        Raw::Num(value) => Some(value),
        _ => None,
    }
}

pub(crate) fn update(c: &mut Construction, index: u32) -> Value {
    let (owner, record) = &c.implicit_curves[index as usize];
    if record.fixed {
        return if record.coefficients.is_empty() {
            Value::Undefined
        } else {
            Value::ImplicitCurve
        };
    }
    let [left, right] = record.sides.expect("an equation has two sides");
    let result = normal(
        c,
        &c.exprs.entries[left as usize],
        &c.exprs.entries[right as usize],
    );
    let Ok((degree, coefficients)) = result else {
        return Value::Undefined;
    };
    let kind = match degree {
        1 => Kind::Line,
        2 => Kind::Conic,
        _ => Kind::ImplicitCurve,
    };
    let owner = Id(*owner);
    let replacing = record.kind != kind;
    if replacing && !c.children(owner).is_empty() {
        return Value::Undefined;
    }
    if replacing && c.exists(owner) {
        c.set_equation_form(owner, crate::kernel::EquationForm::Implicit)
            .ok();
    }
    let record = &mut c.implicit_curves[index as usize].1;
    record.kind = kind;
    record.fingerprint = fingerprint(&coefficients);
    record.coefficients = coefficients;
    let coefficient = |x: usize, y: usize| {
        record
            .coefficients
            .get(x)
            .and_then(|row| row.get(y))
            .copied()
            .unwrap_or(0.0)
    };
    if record.coefficients.iter().flatten().any(|n| !n.is_finite()) {
        return Value::Undefined;
    }
    match kind {
        Kind::Line => super::line_value(&[coefficient(1, 0), coefficient(0, 1), coefficient(0, 0)]),
        Kind::Conic => {
            let mut data = record.conic.unwrap_or_default();
            super::set_conic(
                &mut data,
                &[
                    coefficient(2, 0),
                    coefficient(1, 1),
                    coefficient(0, 2),
                    coefficient(1, 0),
                    coefficient(0, 1),
                    coefficient(0, 0),
                ],
            );
            let value = general_conic::value(kind, &data);
            record.conic = Some(data);
            value
        }
        _ => Value::ImplicitCurve,
    }
}

fn fingerprint(coefficients: &[Vec<f64>]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    coefficients.len().hash(&mut hash);
    for row in coefficients {
        row.len().hash(&mut hash);
        for value in row {
            value.to_bits().hash(&mut hash);
        }
    }
    hash.finish()
}

pub(crate) fn raw_equation(coefficients: &[Vec<f64>], number: impl Fn(f64) -> String) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for (x, row) in coefficients.iter().enumerate().rev() {
        for (y, &coefficient) in row.iter().enumerate().rev() {
            if (x == 0 && y == 0) || coefficient == 0.0 {
                continue;
            }
            let magnitude = number(coefficient.abs());
            if magnitude == "0" {
                continue;
            }
            if out.is_empty() {
                if coefficient < 0.0 {
                    out.push('-');
                }
            } else {
                out.push_str(if coefficient < 0.0 { " - " } else { " + " });
            }
            if magnitude != "1" {
                out.push_str(&magnitude);
            }
            if x != 0 {
                out.push('x');
                if x > 1 {
                    let _ = write!(out, "^{x}");
                }
            }
            if y != 0 {
                if x != 0 {
                    out.push(' ');
                }
                out.push('y');
                if y > 1 {
                    let _ = write!(out, "^{y}");
                }
            }
        }
    }
    if out.is_empty() {
        out.push('0');
    }
    let constant = coefficients
        .first()
        .and_then(|row| row.first())
        .copied()
        .unwrap_or(0.0);
    let _ = write!(
        out,
        " = {}",
        number(if constant == 0.0 { 0.0 } else { -constant })
    );
    out
}
