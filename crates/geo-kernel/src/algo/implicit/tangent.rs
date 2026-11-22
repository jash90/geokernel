// Licensed under the EUPL 1.2.

use super::elimination::{gradient, tangent_contacts};
use super::MAX_OUTPUTS;
use crate::algo::lines::LineDef;
use crate::algo::Parents;
use crate::expr::IntersectionWork;
use crate::plot::implicit::evaluate;
use crate::{Construction, Def, Error, Id, Kind, Value};

/// One line in the ordered output of an implicit tangent command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tangent {
    inputs: [Id; 2],
    slot: u32,
}

impl Tangent {
    pub(crate) fn multi(c: &Construction, command: &str, args: &[&str]) -> Option<Vec<Def>> {
        let ("Tangent", [a, b]) = (command, args) else {
            return None;
        };
        let (a, b) = (c.id(a)?, c.id(b)?);
        let inputs = match (c.kind(a), c.kind(b)) {
            (Kind::Point | Kind::Line, Kind::ImplicitCurve) => [a, b],
            (Kind::ImplicitCurve, Kind::Point) => [b, a],
            _ => return None,
        };
        let first = Self { inputs, slot: 0 };
        Some(
            (0..first.with_lines(c, <[Value]>::len))
                .filter_map(|slot| first.grown_slot(slot))
                .collect(),
        )
    }

    pub(crate) fn can_grow(self) -> bool {
        self.slot == 0
    }

    pub(crate) fn grown_size(self, c: &Construction) -> Option<usize> {
        self.can_grow().then(|| self.with_lines(c, <[Value]>::len))
    }

    pub(crate) fn grown_slot(self, slot: usize) -> Option<Def> {
        (self.can_grow() && slot < MAX_OUTPUTS).then_some(Def::Lines(LineDef::ImplicitTangent(
            Self {
                slot: u32::try_from(slot).ok()?,
                ..self
            },
        )))
    }

    pub(crate) fn parents(self) -> Parents {
        Parents::of(&self.inputs)
    }

    pub(crate) fn check(self, c: &Construction) -> Result<(), Error> {
        c.expect(self.inputs[0], &[Kind::Point, Kind::Line])?;
        c.expect(self.inputs[1], &[Kind::ImplicitCurve])
    }

    pub(crate) fn value(self, c: &Construction) -> Option<Value> {
        self.with_lines(c, |lines| lines.get(self.slot as usize).copied())
    }

    fn with_lines<T>(self, c: &Construction, read: impl FnOnce(&[Value]) -> T) -> T {
        c.function_roots.implicit_tangents(
            self.inputs,
            || lines(c, self.inputs).unwrap_or_default(),
            read,
        )
    }
}

fn lines(c: &Construction, [source, curve]: [Id; 2]) -> Option<Vec<Value>> {
    let coefficients = c.implicit_coefficients(curve)?;
    let work = IntersectionWork::new();
    let (point, line) = match c.kind(source) {
        Kind::Point => (Some(c.point(source)?), None),
        Kind::Line => (None, Some(c.raw_line(source)?)),
        _ => return None,
    };
    let (u, v) = point.or_else(|| line.map(|(a, b, _)| (b, -a)))?;
    let degree = c.implicit_degree(curve)? as f64;
    let mut helper = coefficients.to_vec();
    for (i, row) in helper.iter_mut().enumerate() {
        for (j, coefficient) in row.iter_mut().enumerate() {
            let base = if point.is_some() {
                (degree - (i + j) as f64) * *coefficient
            } else {
                0.0
            };
            let dx = coefficients
                .get(i + 1)
                .and_then(|row| row.get(j))
                .copied()
                .unwrap_or(0.0);
            let dy = coefficients[i].get(j + 1).copied().unwrap_or(0.0);
            *coefficient = base + u * (i + 1) as f64 * dx + v * (j + 1) as f64 * dy;
            if coefficient.abs() <= crate::tol::STANDARD_PRECISION {
                *coefficient = 0.0;
            }
        }
    }
    helper.truncate(
        helper
            .iter()
            .rposition(|row| row.iter().any(|&coefficient| coefficient != 0.0))
            .map_or(1, |row| row + 1),
    );
    let mut output = Vec::new();
    if let Some((x, y)) = point {
        if evaluate(coefficients, x, y).abs() < crate::tol::MIN_PRECISION {
            let (a, b) = gradient(coefficients, x, y);
            if let Some(line) = line_value(a, b, -a * x - b * y, crate::tol::MIN_PRECISION) {
                output.push(line);
            }
        }
    }
    for (x, y) in tangent_contacts(coefficients, &helper, &work, point.is_some())? {
        if point.is_some_and(|(u, v)| (x - u).abs() <= 0.01 && (y - v).abs() <= 0.01) {
            continue;
        }
        let (dx, dy) = gradient(coefficients, x, y);
        if dx.abs() <= 1e-4 && dy.abs() <= 1e-4 {
            continue;
        }
        let (a, b, d) = if let Some((a, b, _)) = line {
            (a, b, -x * a - y * b)
        } else {
            (y - v, u - x, x * v - u * y)
        };
        if let Some(line) = line_value(a, b, d, crate::tol::STANDARD_PRECISION) {
            output.push(line);
        }
    }
    (!work.exhausted() && output.len() <= MAX_OUTPUTS).then_some(output)
}

fn line_value(a: f64, b: f64, c: f64, minimum: f64) -> Option<Value> {
    (a.is_finite() && b.is_finite() && c.is_finite() && (a.abs() > minimum || b.abs() > minimum))
        .then_some(Value::Line { a, b, c })
}
