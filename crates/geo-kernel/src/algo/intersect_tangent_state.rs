//! Private line/conic intersection used by each tangent construction.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentPointND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentLineND.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::line_conic_state::Slots;
use super::Pt;
use crate::algo::conic_nd::{ConicData, ConicType};
use crate::algo::conic_parts::{self, ConicPartDef};
use crate::algo::line_value;
use crate::algo::lines::{self, LineDef};
use crate::kernel::{Construction, Def, Id, Value};
use crate::tol::STANDARD_PRECISION;

type Line = (f64, f64, f64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct TangentDef {
    pub(crate) input: Id,
    pub(crate) conic: Id,
    pub(crate) which: u8,
    point: bool,
}

impl TangentDef {
    pub(crate) fn of(def: Def) -> Option<Self> {
        let (input, conic, which, point) = match def {
            Def::Lines(LineDef::TangentPoint {
                point,
                circle,
                which,
            }) => (point, circle, which, true),
            Def::ConicPart(ConicPartDef::TangentPoint {
                point,
                conic,
                which,
            }) => (point, conic, which, true),
            Def::Lines(LineDef::TangentLine {
                line,
                circle,
                which,
            }) => (line, circle, which, false),
            Def::ConicPart(ConicPartDef::TangentLine { line, conic, which }) => {
                (line, conic, which, false)
            }
            _ => return None,
        };
        Some(Self {
            input,
            conic,
            which,
            point,
        })
    }

    pub(crate) fn same_algorithm(self, other: Self) -> bool {
        self.input == other.input && self.conic == other.conic && self.point == other.point
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TangentState {
    slots: Slots,
    initialized: bool,
    equal_lines: bool,
    source: Option<Line>,
    conic: Option<ConicData>,
    values: [Option<Value>; 2],
    contacts: [Pt; 2],
}

impl TangentState {
    pub(crate) fn same_branch(self, other: Self) -> bool {
        self.equal_lines == other.equal_lines && self.slots.same_order(other.slots)
    }

    pub(crate) fn restore(&mut self, c: &Construction, d: TangentDef) {
        let saved = self.slots;
        self.slots = saved.before_restore();
        self.source = None;
        self.conic = None;
        self.values = [None; 2];
        self.contacts = [None; 2];
        self.refresh(c, d, false);
        self.slots.after_restore(saved);
    }

    pub(crate) fn refresh(&mut self, c: &Construction, d: TangentDef, continuous: bool) {
        let data = c.conic(d.conic).filter(|data| data.defined);
        let source = if d.point {
            c.point(d.input).map(|(x, y)| (x, y, 1.0))
        } else {
            c.raw_carrier(d.input)
        };
        let same_conic = match (data, self.conic) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same(&b),
            _ => false,
        };
        if self.initialized && self.source == source && same_conic {
            return;
        }
        self.source = source;
        self.conic = data;
        self.values = [None; 2];
        self.contacts = [None; 2];

        if let Some(data) = data.filter(|data| !data.ty.is_degenerate()) {
            let circle = c
                .circle(d.conic)
                .map(|(x, y, r)| lines::CircleMatrix::new(x, y, r));
            let on_conic = d.point
                && c.point(d.input).is_some_and(|p| {
                    if let Some(circle) = &circle {
                        circle.is_on(p.0, p.1, STANDARD_PRECISION)
                    } else {
                        data.on_full_conic(p, STANDARD_PRECISION)
                            || super::on_general_conic_by_construction(c, d.input, d.conic)
                    }
                });
            if !self.initialized || !on_conic {
                let candidates = source.map_or([None; 2], |(x, y, _)| {
                    let g = match &circle {
                        Some(circle) => {
                            let g = if d.point {
                                circle.polar(x, y)
                            } else {
                                circle.diameter(y, -x)
                            };
                            return lines::tangent_candidates(g, circle);
                        }
                        None if d.point => conic_parts::polar(&data, (x, y)),
                        None => conic_parts::diameter(&data, y, -x),
                    };
                    super::raw_helper_candidates(g, &data)
                });
                if !self.initialized {
                    self.slots
                        .update(candidates, [true; 2], [false; 2], continuous);
                }
                if !on_conic {
                    self.contacts = self
                        .slots
                        .update(candidates, [true; 2], [false; 2], continuous);
                    self.equal_lines = false;
                }
            }
            if let Some((x, y, _)) = source {
                if on_conic {
                    let [a, b, z] = circle.map_or_else(
                        || conic_parts::polar(&data, (x, y)),
                        |circle| {
                            let (a, b, z) = circle.polar(x, y);
                            [a, b, z]
                        },
                    );
                    self.equal_lines |= !self.initialized;
                    let p = Some((x, y));
                    self.contacts = [p, if self.equal_lines { None } else { p }];
                    self.slots.replace_current(self.contacts);
                    self.values = self.contacts.map(|p| p.and_then(|_| line_value(a, b, z)));
                } else if d.point {
                    self.values = self
                        .contacts
                        .map(|p| p.and_then(|(u, v)| line_value(y - v, u - x, x * v - y * u)));
                } else {
                    let line = |p: Pt| p.and_then(|(u, v)| line_value(x, y, -(u * x + v * y)));
                    self.values = if data.ty == ConicType::Parabola {
                        [line(self.contacts[1].or(self.contacts[0])), None]
                    } else {
                        self.contacts.map(line)
                    };
                }
            }
        }
        self.initialized = true;
    }

    pub(crate) fn value(&self, d: TangentDef) -> Option<Value> {
        self.values[usize::from(d.which)]
    }

    pub(crate) fn contact(&self, d: TangentDef) -> Pt {
        self.contacts[usize::from(d.which)]
    }
}
