// Licensed under the EUPL 1.2.

use super::{build_normal, Builder, Entry, NotPolynomial};
use crate::algo::implicit::barycentric;
use crate::algo::{conic_nd::ConicData, general_conic};
use crate::expr::{eval, parse, shift_children, Element, Lit, Node, Raw, Ty};
use crate::{Construction, Def, Error, Id, Kind, Value};

/// Coefficients indexed by the power of x, then by the power of y.
pub type Coefficients = Vec<Vec<f64>>;

pub(crate) const MAX_DEGREE: u32 = 32;

#[derive(Debug)]
pub(crate) struct Record {
    source: Source,
    pub(crate) kind: Kind,
    pub(crate) coefficients: Coefficients,
    pub(crate) conic: Option<ConicData>,
    fingerprint: u64,
}

#[derive(Clone, Copy, Debug)]
enum Source {
    Equation([u32; 2]),
    Matrix,
    Function(Id),
    Points(Id),
    Cubic([Id; 4]),
}

impl Record {
    fn new(source: Source) -> Self {
        Self {
            source,
            kind: Kind::ImplicitCurve,
            coefficients: Vec::new(),
            conic: None,
            fingerprint: 0,
        }
    }

    pub(super) fn sides(&self) -> Option<[u32; 2]> {
        match self.source {
            Source::Equation(sides) => Some(sides),
            _ => None,
        }
    }
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
        Record::new(Source::Equation([first as u32, first as u32 + 1])),
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
                fingerprint: fingerprint(&coefficients),
                coefficients,
                ..Record::new(Source::Matrix)
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
                    row.iter().enumerate().filter_map(move |(y, &v)| {
                        (v.abs() > crate::tol::STANDARD_PRECISION).then_some(x + y)
                    })
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
    match c.implicit_curves[index as usize].1.source {
        Source::Equation(sides) => {
            for entry in sides {
                parents.extend(&c.exprs.entries[entry as usize].parents);
            }
            parents.sort_unstable();
            parents.dedup();
        }
        Source::Function(id) | Source::Points(id) => parents.push(id),
        Source::Cubic(inputs) => parents.extend(inputs),
        Source::Matrix => {}
    }
    parents
}

fn normal(
    c: &Construction,
    lhs: &Entry,
    rhs: Option<&Entry>,
) -> Result<(u32, Coefficients), NotPolynomial> {
    let mut builder = Builder::function(lhs);
    builder.max_degree = MAX_DEGREE;
    let poly = if let Some(rhs) = rhs {
        let offset = builder.nodes.len() as u32;
        builder.nodes.extend(rhs.nodes.iter().map(|node| {
            let mut node = *node;
            shift_children(&mut node, offset);
            node
        }));
        builder.lits.extend(rhs.lits.iter().cloned());
        build_normal(c, &mut builder, lhs.root, offset + rhs.root)?
    } else {
        let root = match builder.node(lhs.root) {
            Node::SystemFunction(2, body) => {
                for node in &mut builder.nodes {
                    match *node {
                        Node::Local(0, _) => *node = Node::Var,
                        Node::Local(1, _) => *node = Node::VarY,
                        _ => {}
                    }
                }
                body
            }
            Node::SystemFunction(..) => return Err(NotPolynomial::ImplicitCurve),
            _ => lhs.root,
        };
        builder.expand(c, root)?
    };
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
    let (kind, coefficients) = match record.source {
        Source::Matrix => {
            return if record.coefficients.is_empty() {
                Value::Undefined
            } else {
                Value::ImplicitCurve
            };
        }
        Source::Equation([left, right]) => {
            let Ok((degree, coefficients)) = normal(
                c,
                &c.exprs.entries[left as usize],
                Some(&c.exprs.entries[right as usize]),
            ) else {
                return Value::Undefined;
            };
            let kind = match degree {
                1 => Kind::Line,
                2 => Kind::Conic,
                _ => Kind::ImplicitCurve,
            };
            (kind, coefficients)
        }
        Source::Function(id) => {
            let Def::Expr(def) = c.def(id) else {
                return Value::Undefined;
            };
            let Ok((_, coefficients)) = normal(c, crate::expr::entry(c, def), None) else {
                return Value::Undefined;
            };
            (Kind::ImplicitCurve, coefficients)
        }
        Source::Points(id) => {
            let Some(coefficients) = through_points(c, id) else {
                return Value::Undefined;
            };
            (Kind::ImplicitCurve, coefficients)
        }
        Source::Cubic(inputs) => {
            let Some(coefficients) = barycentric::cubic(c, inputs) else {
                return Value::Undefined;
            };
            (Kind::ImplicitCurve, coefficients)
        }
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

const MAX_INTERPOLATION_DEGREE: usize = 8;
const MAX_POINTS: usize = point_count(MAX_INTERPOLATION_DEGREE);

const fn point_count(degree: usize) -> usize {
    (degree + 1) * (degree + 2) / 2 - 1
}

fn interpolation_degree(points: usize) -> Option<usize> {
    (1..=MAX_INTERPOLATION_DEGREE).find(|&degree| point_count(degree) == points)
}

pub(crate) fn command(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    let (command @ ("ImplicitCurve" | "Cubic"), args) = crate::script::split_call(input)? else {
        return None;
    };
    let start = c.checkpoint();
    let result = (|| {
        let source = if command == "Cubic" {
            let [a, b, d, n] = args.as_slice() else {
                return Err("Cubic needs three points and a number".into());
            };
            Source::Cubic([
                barycentric::argument(c, a, Kind::Point)?,
                barycentric::argument(c, b, Kind::Point)?,
                barycentric::argument(c, d, Kind::Point)?,
                barycentric::argument(c, n, Kind::Number)?,
            ])
        } else if let [argument] = args.as_slice() {
            let hidden = format!("\0implicit{}", c.len());
            match c.id(argument) {
                Some(id) if c.kind(id) == Kind::List => Source::Points(id),
                Some(id) if c.kind(id) == Kind::FunctionNVar => Source::Function(id),
                Some(_) => {
                    return Err(
                        "ImplicitCurve needs a point list or a function of two variables".into(),
                    )
                }
                None if crate::expr::argument_object(c, argument, &hidden, Kind::List) => {
                    Source::Points(c.id(&hidden).expect("resolved list"))
                }
                None => {
                    let parsed = parse::parse_equation(c, argument)?;
                    if !parsed.uses_var
                        || !parsed.uses_y
                        || !matches!(parsed.tys[parsed.root as usize], Ty::Num(_))
                    {
                        return Err(
                            "ImplicitCurve needs a point list or a function of two variables"
                                .into(),
                        );
                    }
                    Source::Function(crate::expr::add(c, &hidden, parsed, Kind::FunctionNVar)?)
                }
            }
        } else {
            if interpolation_degree(args.len()).is_none() {
                return Err(
                    "ImplicitCurve needs a point count that determines a polynomial".into(),
                );
            }
            let mut points = Vec::with_capacity(args.len());
            for argument in args {
                if let Some(point) = c.id(argument).filter(|&id| c.kind(id) == Kind::Point) {
                    points.push(point);
                } else {
                    let hidden = format!("\0implicit{}", c.len());
                    if !crate::expr::argument_object(c, argument, &hidden, Kind::Point) {
                        return Err(format!("expected a point, got `{argument}`"));
                    }
                    points.push(c.id(&hidden).expect("resolved point"));
                }
            }
            let hidden = format!("\0implicit{}", c.len());
            Source::Points(crate::expr::list_of_objects(c, &hidden, points)?)
        };
        let id = add(c, label, Record::new(source)).map_err(|e| e.to_string())?;
        c.set_equation_form(id, crate::kernel::EquationForm::Implicit)
            .map_err(|e| e.to_string())?;
        Ok(id)
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}

fn through_points(c: &Construction, list: Id) -> Option<Coefficients> {
    let list = c.elements(list)?;
    let degree = interpolation_degree(list.len())?;
    let points = list
        .iter()
        .take(MAX_POINTS)
        .map(|element| match element {
            Element::Value(Kind::Point, Value::Point { x, y }) => Some((x, y)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let powers: Vec<_> = (0..=degree)
        .flat_map(|i| (0..=degree - i).map(move |j| (i, j)))
        .collect();
    let rows: Vec<Vec<f64>> = points
        .iter()
        .map(|&(x, y)| {
            powers
                .iter()
                .map(|&(i, j)| x.powi(i as i32) * y.powi(j as i32))
                .collect()
        })
        .collect();
    if rows.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    // The lower-degree retry uses cached projection coordinates. For ordinary
    // 2D points those coordinates are all zero, so each retry is singular.
    let solution = (0..powers.len()).find_map(|fixed| solve(&rows, fixed))?;
    let mut coefficients = vec![vec![0.0; degree + 1]; degree + 1];
    for (&(i, j), coefficient) in powers.iter().zip(solution) {
        if coefficient.abs() >= crate::tol::STANDARD_PRECISION {
            coefficients[i][j] = coefficient;
        }
    }
    points
        .iter()
        .all(|&(x, y)| {
            crate::plot::implicit::evaluate(&coefficients, x, y).abs() < crate::tol::MIN_PRECISION
        })
        .then_some(coefficients)
}

fn solve(rows: &[Vec<f64>], fixed: usize) -> Option<Vec<f64>> {
    let n = rows.len();
    let mut a: Vec<Vec<f64>> = rows
        .iter()
        .map(|row| {
            let mut coefficients = row.clone();
            let rhs = -coefficients.remove(fixed);
            coefficients.push(rhs);
            coefficients
        })
        .collect();
    const MINIMUM_PIVOT: f64 = 1e-11;
    for k in 0..n {
        let pivot = (k..n).min_by(|&i, &j| a[j][k].abs().total_cmp(&a[i][k].abs()))?;
        let divisor = a[pivot][k];
        if divisor.abs() < MINIMUM_PIVOT || !divisor.is_finite() {
            return None;
        }
        a.swap(k, pivot);
        let (top, bottom) = a.split_at_mut(k + 1);
        for row in bottom {
            let factor = row[k] / divisor;
            for j in k + 1..=n {
                row[j] -= factor * top[k][j];
            }
        }
    }
    let mut result = vec![0.0; n];
    for k in (0..n).rev() {
        let residual = a[k][n] - (k + 1..n).map(|j| a[k][j] * result[j]).sum::<f64>();
        result[k] = residual / a[k][k];
        if !result[k].is_finite() {
            return None;
        }
    }
    result.insert(fixed, 1.0);
    Some(result)
}

pub(crate) fn asymptote_command(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    let ("Asymptote", args) = crate::script::split_call(input)? else {
        return None;
    };
    let [argument] = args.as_slice() else {
        return None;
    };
    let curve = c
        .id(argument)
        .filter(|&id| c.kind(id) == Kind::ImplicitCurve)?;
    let nodes = [Node::Obj(curve), Node::FunctionAsymptotes(0)];
    let Some(parsed) =
        crate::expr::reparse(c, Kind::List, &nodes, &[Lit::default(), Lit::default()], 1)
    else {
        return Some(Err("cannot build an implicit asymptote list".into()));
    };
    Some(crate::expr::add(c, label, parsed, Kind::List))
}
