// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCommonTangents.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/matrix/CoordMatrix.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Common tangents via the points of contact on the first conic.

use super::conics::{on_part, Intersection, SavedSlots};
use super::Pt;
use crate::algo::{conic_nd::ConicData, lines::LineDef, CONIC_OBJECTS};
use crate::kernel::{Construction, Def, Id, Value};

type Matrix = [[f64; 3]; 3];

/// One output of `Tangent(conic, conic)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommonTangentDef {
    pub(crate) first: Id,
    pub(crate) second: Id,
    pub(crate) which: u8,
    saved: Option<SavedSlots>,
}

pub(super) fn parse_multi(c: &Construction, name: &str, args: &[&str]) -> Option<Vec<Def>> {
    let ("Tangent", [first, second]) = (name, args) else {
        return None;
    };
    let (first, second) = (c.id(first)?, c.id(second)?);
    if ![first, second]
        .into_iter()
        .all(|id| CONIC_OBJECTS.contains(&c.kind(id)))
    {
        return None;
    }
    Some(
        (0..4)
            .map(|which| {
                Def::Lines(LineDef::CommonTangent(CommonTangentDef::new(
                    first, second, which,
                )))
            })
            .collect(),
    )
}

impl CommonTangentDef {
    /// Build a tangent for one of the four slots, numbered 0 through 3.
    pub fn new(first: Id, second: Id, which: u8) -> Self {
        Self {
            first,
            second,
            which,
            saved: None,
        }
    }

    pub(crate) fn has_state(self, c: &Construction) -> bool {
        c.arc(self.first).is_some()
    }

    pub(crate) fn with_state(self, state: &CommonTangentState) -> Self {
        Self {
            saved: Some(state.intersection.saved_slots()),
            ..self
        }
    }

    pub(crate) fn create(self, c: &Construction) -> (CommonTangentState, Option<Value>) {
        let (state, contact) = self.initial_contact(c);
        (state, contact.map(|(line, _)| line))
    }

    fn initial_contact(self, c: &Construction) -> (CommonTangentState, Option<Contact>) {
        let mut state = CommonTangentState::default();
        if let Some(saved) = self.saved {
            state.intersection.restore_slots(saved);
        }
        let contact = state.polar(c, self, self.saved.is_none());
        (state, contact)
    }

    pub(crate) fn value(self, c: &Construction) -> Option<Value> {
        self.create(c).1
    }

    pub(crate) fn contact(self, c: &Construction) -> Pt {
        self.initial_contact(c).1.map(|(_, point)| point)
    }
}

#[derive(Default)]
pub(crate) struct CommonTangentState {
    intersection: Intersection,
}

impl CommonTangentState {
    pub(crate) fn update(&mut self, c: &Construction, def: CommonTangentDef) -> Option<Value> {
        self.polar(c, def, false).map(|(line, _)| line)
    }

    fn polar(
        &mut self,
        c: &Construction,
        def: CommonTangentDef,
        initialize: bool,
    ) -> Option<Contact> {
        let Some((a, b, auxiliary)) = conic(c, def.first).and_then(|a| {
            let b = conic(c, def.second)?;
            Some((a, b, polar_conic(&a, &b)?))
        }) else {
            self.intersection.undefined();
            return None;
        };
        let part = c.arc(def.first);
        // The helper intersects the first conic's path, so only that
        // operand's arc limits the points of contact.
        let on_path = |point| part.as_ref().is_none_or(|part| on_part(part, point));
        let mut points = self.intersection.update(&auxiliary, &a, false, on_path);
        if initialize && part.is_some() {
            // The reference helper computes at creation and again during
            // the first tangent computation, fixing its slot permutation.
            points = self.intersection.update(&auxiliary, &a, false, on_path);
        }
        *assign_slots(&a, &b, points).get(usize::from(def.which))?
    }
}

type Contact = (Value, (f64, f64));

fn assign_slots(a: &ConicData, b: &ConicData, points: [Pt; 4]) -> [Option<Contact>; 4] {
    let mut result = [None; 4];
    let mut outer = 0;
    let mut inner = 2;
    for point in points {
        let contact = point.and_then(|(x, y)| {
            let [xx, yy, constant, xy, lx, ly] = a.matrix;
            let line = crate::algo::line_value(
                xx * x + xy * y + lx,
                xy * x + yy * y + ly,
                lx * x + ly * y + constant,
            )?;
            Some((line, (x, y)))
        });
        // An undefined polar uses an inner slot, just like a line
        // separating the two centers.
        let is_inner = match contact {
            Some((Value::Line { a: x, b: y, c }, _)) => {
                let side = |[px, py]: [f64; 2]| x * px + y * py + c;
                side(a.midpoint) * side(b.midpoint) < 0.0
            }
            _ => true,
        };
        let slot = if is_inner && inner < 4 {
            let slot = inner;
            inner += 1;
            slot
        } else if outer < 2 {
            let slot = outer;
            outer += 1;
            slot
        } else {
            continue;
        };
        result[slot] = contact;
    }
    result
}

fn polar_conic(a: &ConicData, b: &ConicData) -> Option<ConicData> {
    let ma = symmetric(a.matrix);
    // The polar of p on A is tangent to B if p^T A B^-1 A p = 0.
    let m = multiply(&multiply(&ma, &inverse(b.matrix)?), &ma);
    let mut auxiliary = ConicData::new();
    auxiliary.set_matrix([
        m[0][0],
        m[1][1],
        m[2][2],
        (m[0][1] + m[1][0]) / 2.0,
        (m[0][2] + m[2][0]) / 2.0,
        (m[1][2] + m[2][1]) / 2.0,
    ]);
    Some(auxiliary)
}

fn conic(c: &Construction, id: Id) -> Option<ConicData> {
    c.arc(id)
        .filter(|part| part.defined)
        .map(|part| part.conic)
        .or_else(|| c.conic(id))
        .filter(|conic| conic.defined)
}

fn symmetric([xx, yy, c, xy, x, y]: [f64; 6]) -> Matrix {
    [[xx, xy, x], [xy, yy, y], [x, y, c]]
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn inverse(matrix: [f64; 6]) -> Option<Matrix> {
    let mut left = symmetric(matrix);
    let mut right = symmetric([1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
    let mut columns = [2, 1, 0];
    // CoordMatrix eliminates rows from bottom to top, keeping unresolved
    // columns in reverse order for pivot ties. The rounding in the inverse
    // can change the ordering of the degenerate pencil used by the helper.
    for step in (1..3).rev() {
        let mut pivot_index = 0;
        for (i, &column) in columns[..=step].iter().enumerate().skip(1) {
            if left[step][column].abs() > left[step][columns[pivot_index]].abs() {
                pivot_index = i;
            }
        }
        let pivot = columns[pivot_index];
        let value = left[step][pivot];
        if value == 0.0 || !value.is_finite() {
            return None;
        }
        let (above, row) = left.split_at_mut(step);
        let (rhs_above, rhs) = right.split_at_mut(step);
        for &column in &columns[..=step] {
            row[0][column] /= value;
        }
        rhs[0] = rhs[0].map(|x| x / value);
        for (upper, upper_rhs) in above.iter_mut().zip(rhs_above) {
            let factor = upper[pivot];
            for &column in &columns[..=step] {
                upper[column] -= factor * row[0][column];
            }
            for (entry, rhs) in upper_rhs.iter_mut().zip(rhs[0]) {
                *entry -= factor * rhs;
            }
        }
        columns[pivot_index..=step].rotate_left(1);
    }
    let first = left[0][columns[0]];
    if first == 0.0 || !first.is_finite() {
        return None;
    }
    let mut inverse = [[0.0; 3]; 3];
    inverse[columns[0]] = right[0].map(|x| x / first);
    for (step, &pivot) in columns.iter().enumerate().skip(1) {
        inverse[pivot] = std::array::from_fn(|j| {
            let mut entry = right[step][j];
            for &column in &columns[..step] {
                entry -= left[step][column] * inverse[column][j];
            }
            entry
        });
    }
    Some(inverse)
}
