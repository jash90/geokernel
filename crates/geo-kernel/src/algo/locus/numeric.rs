// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoSolveODE.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoSolveODE2.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoNSolveODE.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{locus_from_points, record_slope_field, LocusData, LocusPoint};
use crate::algo::{Parents, MAX_PARENTS, NUMBER};
use crate::expr::{fraction::FunctionFraction, IntersectionWork};
use crate::{Construction, Error, Id, Kind};

#[path = "numeric/rk.rs"]
mod rk;
use rk::{adaptive, classical};

const MAX_SYSTEM_SIZE: usize = 16;
const MAX_ODE_POINTS: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumericLocus {
    Slope {
        function: u32,
        options: [u32; 6],
        count: u8,
    },
    First([u32; 5]),
    Parametric([u32; 6]),
    Second([u32; 8]),
    System {
        args: [u32; 4],
        slot: u8,
    },
}

fn id(index: u32) -> Id {
    Id(index as usize)
}

impl NumericLocus {
    pub(crate) fn inputs(self) -> impl Iterator<Item = Id> {
        let mut indices = [0; 8];
        let len = match self {
            Self::Slope {
                function,
                options,
                count,
            } => {
                indices[0] = function;
                let n = usize::from(count).min(options.len());
                indices[1..1 + n].copy_from_slice(&options[..n]);
                1 + n
            }
            Self::First(args) => {
                indices[..5].copy_from_slice(&args);
                5
            }
            Self::Parametric(args) => {
                indices[..6].copy_from_slice(&args);
                6
            }
            Self::Second(args) => {
                indices = args;
                8
            }
            Self::System { args, .. } => {
                indices[..4].copy_from_slice(&args);
                4
            }
        };
        indices.into_iter().take(len).map(id)
    }

    pub(super) fn parents(self) -> Parents {
        let mut parents = [Id(0); MAX_PARENTS];
        let mut n = 0;
        for (slot, parent) in parents.iter_mut().zip(self.inputs()) {
            *slot = parent;
            n += 1;
        }
        Parents::of(&parents[..n])
    }

    pub(super) fn slope_source(self) -> Option<Id> {
        match self {
            Self::Slope { function, .. } => Some(id(function)),
            _ => None,
        }
    }

    pub(super) fn check(self, c: &Construction) -> Result<(), Error> {
        let functions = match self {
            Self::System {
                args: [f, a, y, b], ..
            } => {
                c.expect(id(f), &[Kind::List])?;
                c.expect(id(y), &[Kind::List])?;
                c.expect(id(a), NUMBER)?;
                return c.expect(id(b), NUMBER);
            }
            Self::Slope { .. } | Self::First(_) => 1,
            Self::Parametric(_) => 2,
            Self::Second(_) => 3,
        };
        for (i, parent) in self.inputs().enumerate() {
            c.expect(
                parent,
                if i < functions {
                    &[Kind::Function, Kind::FunctionNVar]
                } else {
                    NUMBER
                },
            )?;
        }
        Ok(())
    }

    pub(crate) fn system_size(c: &Construction, args: [u32; 4]) -> Option<usize> {
        let functions = c.elements(id(args[0]))?;
        let initial = c.elements(id(args[2]))?;
        let len = functions.len();
        (len == initial.len()
            && (1..=MAX_SYSTEM_SIZE).contains(&len)
            && (0..len).all(|i| {
                functions
                    .object(i)
                    .is_some_and(|f| matches!(c.kind(f), Kind::Function | Kind::FunctionNVar))
                    && initial.numeric_cell(i).is_some()
            }))
        .then_some(len)
    }

    pub(super) fn record(self, c: &Construction) -> LocusData {
        if let Self::Slope {
            function,
            options,
            count,
        } = self
        {
            let n = |i: usize| c.number(id(options[i]));
            let bounds = (count == 6).then(|| [2, 4, 3, 5].map(|i| n(i).unwrap_or(f64::NAN)));
            return record_slope_field(
                c,
                id(function),
                n(0),
                (count >= 2).then(|| n(1)).flatten(),
                bounds,
            );
        }
        self.solve(c)
            .map_or_else(LocusData::default, locus_from_points)
    }

    fn solve(self, c: &Construction) -> Option<Vec<LocusPoint>> {
        let work = IntersectionWork::new();
        let number = |arg| c.number(id(arg));
        let function = |arg| FunctionFraction::new(c, id(arg));
        let point = |x, y, line_to| LocusPoint { x, y, line_to };
        match self {
            Self::Slope { .. } => unreachable!(),
            Self::First([f, x, y, end, h]) => {
                let (x, y, end, h) = (number(x)?, number(y)?, number(end)?, number(h)?);
                let f = function(f)?;
                if f.has_nonconstant_denominator() {
                    classical(
                        0.0,
                        end,
                        h,
                        [x, y],
                        |_, [x, y]| {
                            let (dy, dx) = f.evaluate(x, y);
                            [dx, dy]
                        },
                        |_, [x, y], line| point(x, y, line),
                        &work,
                    )
                } else {
                    classical(
                        x,
                        end,
                        h,
                        [y],
                        |x, [y]| {
                            let (p, q) = f.evaluate(x, y);
                            [p / q]
                        },
                        |x, [y], line| point(x, y, line),
                        &work,
                    )
                }
            }
            Self::Parametric([f, g, x, y, end, h]) => {
                let (x, y, end, h) = (number(x)?, number(y)?, number(end)?, number(h)?);
                let (f, g) = (function(f)?, function(g)?);
                classical(
                    0.0,
                    end,
                    h,
                    [x, y],
                    |_, [x, y]| {
                        let (fx, fd) = f.evaluate(x, y);
                        let (gx, gd) = g.evaluate(x, y);
                        [gx / gd, fx / fd]
                    },
                    |_, [x, y], line| point(x, y, line),
                    &work,
                )
            }
            Self::Second([b, c1, f, x, y, dy, end, h]) => {
                let (x, y, dy, end, h) = (
                    number(x)?,
                    number(y)?,
                    number(dy)?,
                    number(end)?,
                    number(h)?,
                );
                let sample = |arg, x| work.sample(c, id(arg), x);
                if [b, c1, f]
                    .into_iter()
                    .any(|arg| !c.value(id(arg)).is_defined())
                {
                    return None;
                }
                classical(
                    x,
                    end,
                    h,
                    [y, dy],
                    |x, [y, dy]| [dy, sample(f, x) - sample(b, x) * dy - sample(c1, x) * y],
                    |x, [y, _], line| point(x, y, line),
                    &work,
                )
            }
            Self::System { args, slot } => {
                let len = Self::system_size(c, args)?;
                let slot = usize::from(slot);
                if slot >= len {
                    return None;
                }
                let functions = c.elements(id(args[0]))?;
                let initial = c.elements(id(args[2]))?;
                let functions: Vec<_> = (0..len)
                    .map(|i| functions.object(i))
                    .collect::<Option<_>>()?;
                let initial: Vec<_> = (0..len)
                    .map(|i| initial.numeric_cell(i))
                    .collect::<Option<_>>()?;
                adaptive(
                    number(args[1])?,
                    number(args[3])?,
                    &initial,
                    |t, ys, out| {
                        for (function, value) in functions.iter().zip(out.iter_mut()) {
                            *value = work.sample_system(c, *function, t, ys);
                        }
                    },
                    slot,
                    &work,
                )
            }
        }
    }
}
