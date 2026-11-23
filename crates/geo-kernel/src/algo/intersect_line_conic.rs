//! Stateful output slots for a line, segment or ray intersecting a conic.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectSingle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIntersect.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::pair::{closest, index, Operand, Output};
use super::{
    existing_line_circle_point, existing_line_conic_point, intersect_line_conic,
    intersect_line_general_conic, other_point, points_equal, to_value, within_carrier, Conic, Hint,
    Pt,
};
use crate::algo::{conic_parts, lines, Group, Parents, CARRIERS, CONIC_OBJECTS, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineConicDef {
    pub(crate) line: Id,
    pub(crate) conic: Id,
    pub(crate) output: Output,
    saved: Option<SavedSlots>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SavedSlots {
    first: bool,
    needs_permutation: bool,
    swapped: bool,
    labelled_second: bool,
    nearest: u8,
}

impl LineConicDef {
    pub(super) fn pair(c: &Construction, first: Id, second: Id) -> Option<Self> {
        let (line, conic) = if CARRIERS.contains(&c.kind(first)) {
            (first, second)
        } else {
            (second, first)
        };
        (CARRIERS.contains(&c.kind(line)) && CONIC_OBJECTS.contains(&c.kind(conic)))
            .then_some(Self::new(line, conic, Output::Slot(0)))
    }

    pub(crate) fn new(line: Id, conic: Id, output: Output) -> Self {
        Self {
            line,
            conic,
            output,
            saved: None,
        }
    }

    pub(crate) fn from_indexed(def: Def) -> Option<Self> {
        match def {
            Def::IntersectLineCircle {
                line,
                circle,
                which,
            } => Some(Self::new(line, circle, Output::Index(usize::from(which)))),
            Def::ConicPart(conic_parts::ConicPartDef::IntersectLine { line, conic, which }) => {
                Some(Self::new(line, conic, Output::Index(usize::from(which))))
            }
            _ => None,
        }
    }

    pub(crate) fn key(self) -> (Id, Id) {
        (self.line, self.conic)
    }

    pub(crate) fn with_state(mut self, state: &LineConicState) -> Self {
        self.saved = Some(SavedSlots {
            first: state.slots.first,
            needs_permutation: state.slots.needs_permutation,
            swapped: state.slots.order == [1, 0],
            labelled_second: state.labelled[1],
            nearest: state.nearest as u8,
        });
        self
    }

    pub(super) fn single(mut self, c: &Construction, selector: Id) -> Result<Self, String> {
        self.output = match c.kind(selector) {
            Kind::Point => Output::Nearest(selector),
            kind if kind.is_number() => Output::Number(selector),
            _ => return Err("expected an intersection index or point".into()),
        };
        Ok(self)
    }

    fn existing_point(self, c: &Construction) -> Option<Id> {
        let line = c.def(self.line);
        if c.kind(self.conic) == Kind::Circle {
            existing_line_circle_point(c, self.line, line, self.conic)
        } else {
            existing_line_conic_point(c, self.line, line, self.conic)
        }
    }

    pub(super) fn tangent(self, c: &Construction) -> Option<Pt> {
        c.tangent_contact(self.line, self.conic)
            .or_else(|| match c.def(self.line) {
                Def::Lines(d) => lines::tangent_point_on(c, d, self.conic),
                Def::ConicPart(d) => conic_parts::tangent_point_on(c, d, self.conic),
                _ => None,
            })
    }

    pub(super) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        let def = if c.kind(self.conic) == Kind::Circle {
            Def::IntersectLineCircle {
                line: self.line,
                circle: self.conic,
                which: 0,
            }
        } else {
            Def::ConicPart(conic_parts::ConicPartDef::IntersectLine {
                line: self.line,
                conic: self.conic,
                which: 0,
            })
        };
        super::hidden_parents(c, &def)
    }
}

impl Group for LineConicDef {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        match self.output {
            Output::Number(id) | Output::Nearest(id) => Parents::of(&[self.line, self.conic, id]),
            Output::Slot(_) | Output::Index(_) => Parents::of(&[self.line, self.conic]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.line, CARRIERS)?;
        c.expect(self.conic, CONIC_OBJECTS)?;
        match self.output {
            Output::Number(id) => c.expect(id, &[Kind::Number, Kind::Angle]),
            Output::Nearest(id) => c.expect(id, POINT),
            Output::Slot(slot) if slot >= 2 => Err(Error::BadIndex(slot as u8)),
            _ => Ok(()),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        c.preview_line_conic(*self)
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let ("Intersect", [first, second, selector]) = (func, args) else {
            return None;
        };
        let mut d = Self::pair(c, c.id(first)?, c.id(second)?)?;
        if let Some(id) = c.id(selector) {
            return Some(d.single(c, id));
        }
        d.output = Output::Index(index(crate::expr::plain_number(selector)?));
        Some(Ok(d))
    }
}

/// The permutation and old positions of an AlgoIntersectLineConic. A tangent
/// algorithm can also own one of these privately for its polar or diameter.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Slots {
    current: [Pt; 2],
    suppressed: Pt,
    previous: [Pt; 2],
    age: [i32; 2],
    alive: [bool; 2],
    order: [usize; 2],
    first: bool,
    needs_permutation: bool,
}

impl Default for Slots {
    fn default() -> Self {
        Self {
            current: [None; 2],
            suppressed: None,
            previous: [None; 2],
            age: [0; 2],
            alive: [false; 2],
            order: [0, 1],
            first: true,
            needs_permutation: true,
        }
    }
}

impl Slots {
    pub(crate) fn replace_current(&mut self, points: [Pt; 2]) {
        self.current = points;
        self.suppressed = None;
    }

    pub(crate) fn reset_permutation(&mut self) {
        self.age = [0; 2];
        self.alive = [false; 2];
        self.needs_permutation = true;
    }

    pub(crate) fn update(
        &mut self,
        candidates: [Pt; 2],
        on_path: [bool; 2],
        labelled: [bool; 2],
        continuous: bool,
    ) -> [Pt; 2] {
        let match_points = self.needs_permutation || continuous;
        if match_points {
            self.remember(labelled);
        }
        if self.first {
            self.current = [None; 2];
            let mut slot = 0;
            for point in (0..2).filter_map(|i| candidates[i].filter(|_| on_path[i])) {
                self.current[slot] = Some(point);
                self.previous[slot] = Some(point);
                slot += 1;
            }
            self.first = slot < 2 || points_equal(self.current[0], self.current[1]);
        } else {
            if match_points {
                let gap = super::conics::distance_squared(candidates[0], candidates[1]);
                if self.needs_permutation || gap > 1e-12 {
                    self.order = super::conics::nearest(
                        self.previous,
                        self.age,
                        self.alive,
                        candidates,
                        on_path,
                    );
                }
                self.needs_permutation = false;
            }
            self.current = self.order.map(|i| candidates[i]);
            let distinct = !points_equal(self.current[0], self.current[1]);
            for (slot, i) in self.order.into_iter().enumerate() {
                if !on_path[i] {
                    if match_points && distinct && self.current[slot].is_some() {
                        self.previous[slot] = self.current[slot];
                        self.age[slot] = -1;
                    }
                    self.current[slot] = None;
                }
            }
        }
        self.suppressed = None;
        if !labelled[1] && points_equal(self.current[0], self.current[1]) {
            self.suppressed = self.current[1].take();
        }
        self.current
    }

    fn remember(&mut self, labelled: [bool; 2]) {
        let distinct = !points_equal(self.current[0], self.current[1]);
        for (i, label) in labelled.into_iter().enumerate() {
            if distinct && self.current[i].is_some() {
                self.previous[i] = self.current[i];
                self.age[i] = 0;
            } else {
                self.age[i] = self.age[i].saturating_add(1);
            }
            self.alive[i] |= self.current[i].is_some() || label;
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Inputs {
    carrier: Value,
    raw: Option<(f64, f64, f64)>,
    conic: Option<Operand>,
}

impl Inputs {
    fn read(c: &Construction, d: LineConicDef) -> Self {
        Self {
            carrier: c.value(d.line),
            raw: c.raw_carrier(d.line),
            conic: Operand::get(c, d.conic),
        }
    }

    fn same(self, other: Self) -> bool {
        self.carrier.identical(&other.carrier)
            && self.raw == other.raw
            && Operand::same(self.conic, other.conic)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LineConicState {
    inputs: Option<Inputs>,
    slots: Slots,
    special: Hint,
    labelled: [bool; 2],
    nearest: usize,
}

impl LineConicState {
    pub(crate) fn create(
        d: LineConicDef,
        c: &Construction,
        prior: Option<Self>,
        continuous: bool,
    ) -> Self {
        let mut state = prior.unwrap_or_else(|| Self {
            special: super::point_hint(d.existing_point(c)),
            ..Self::default()
        });
        state.nearest = 0;
        if let Some(saved) = d.saved {
            // Rebuild the previous coordinates before fixing a pending
            // permutation on the next update.
            state.slots.first = saved.first || saved.needs_permutation;
            state.slots.needs_permutation = saved.needs_permutation;
            state.slots.order = if saved.swapped { [1, 0] } else { [0, 1] };
            state.labelled[1] = saved.labelled_second;
            state.nearest = usize::from(saved.nearest);
            state.inputs = None;
        }
        state.refresh(d, c, continuous, false);
        if !matches!(d.output, Output::Slot(_)) && d.saved.is_none() {
            // AlgoIntersectSingle initializes the parent and computes it
            // once more before it reads the selected slot.
            state.slots.reset_permutation();
            state.update(d, c, continuous);
        }
        state
    }

    pub(crate) fn label_output(&mut self, d: LineConicDef) {
        if let Output::Slot(slot) = d.output {
            self.labelled[slot] = true;
            if slot == 1 && self.slots.current[1].is_none() {
                self.slots.current[1] = self.slots.suppressed.take();
            }
        }
    }

    pub(crate) fn refresh(
        &mut self,
        d: LineConicDef,
        c: &Construction,
        continuous: bool,
        redefined: bool,
    ) {
        let new = Inputs::read(c, d);
        if redefined {
            self.special = super::point_hint(d.existing_point(c));
        }
        if redefined || self.inputs.is_none_or(|old| !old.same(new)) || self.special != Hint::None {
            self.inputs = Some(new);
            self.update(d, c, continuous);
        }
    }

    fn update(&mut self, d: LineConicDef, c: &Construction, continuous: bool) {
        let Some(input) = self.inputs else { return };
        if let Some(tangent) = d.tangent(c) {
            self.slots.replace_current([tangent, None]);
            return;
        }
        let candidates = match (input.raw, input.conic) {
            (Some((a, b, z)), Some(operand)) => {
                let g = [a, b, z];
                if let Some((x, y, r)) = c.circle(d.conic) {
                    intersect_line_conic(g, &Conic::new(x, y, r))
                } else {
                    intersect_line_general_conic(g, operand.conic())
                }
            }
            _ => [None; 2],
        };
        let on_paths = |pt| {
            within_carrier(c, d.line, pt) && input.conic.is_some_and(|operand| operand.contains(pt))
        };
        if let Some(s) = super::hinted_point(self.special, || d.existing_point(c)) {
            let first = c.point(s);
            let mut points = [first, other_point(&candidates, first)];
            if c.kind(d.line) != Kind::Line || matches!(c.kind(d.conic), Kind::Arc | Kind::Sector) {
                points = points.map(|point| point.filter(|&p| on_paths(p)));
            }
            self.slots
                .replace_current(self.slots.order.map(|i| points[i]));
            return;
        }
        let limited =
            c.kind(d.line) != Kind::Line || matches!(c.kind(d.conic), Kind::Arc | Kind::Sector);
        let on_path = if limited {
            candidates.map(|pt| pt.is_some_and(on_paths))
        } else {
            [true; 2]
        };
        self.slots
            .update(candidates, on_path, self.labelled, continuous);
    }

    pub(crate) fn restore_order(&mut self, swap: bool) {
        self.slots.order = if swap { [1, 0] } else { [0, 1] };
        self.slots.first = false;
        self.slots.needs_permutation = false;
    }

    pub(crate) fn nearest(&self) -> usize {
        self.nearest
    }

    pub(crate) fn set_nearest(&mut self, nearest: usize) {
        self.nearest = nearest;
    }

    pub(crate) fn value(&mut self, d: LineConicDef, c: &Construction) -> Option<Value> {
        let points = self.slots.current;
        let slot = match d.output {
            Output::Slot(slot) | Output::Index(slot) => slot,
            Output::Number(id) => index(match c.value(id) {
                Value::Number(n) => n,
                _ => f64::NAN,
            }),
            Output::Nearest(id) => {
                if let Some(seed) = c.point(id) {
                    self.nearest = closest(points, seed);
                }
                self.nearest
            }
        };
        if matches!(d.output, Output::Slot(_))
            || c.value(d.line).is_defined() && c.value(d.conic).is_defined()
        {
            to_value(*points.get(slot)?)
        } else {
            None
        }
    }
}
