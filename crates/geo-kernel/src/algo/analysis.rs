//! Live outputs of the numerical function-analysis commands.

mod asymptote;
mod curvature;
mod roots;
mod spline;

use super::{Group, Parents, NUMBER, POINT};
use crate::kernel::locus::LocusData;
use crate::{Construction, Def, Error, Id, Kind, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub(crate) use asymptote::asymptotes;
pub(crate) use curvature::CurvatureKind;
pub(crate) use roots::Mode;
pub(crate) use spline::Spline;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RootInput {
    function: Id,
    mode: Mode,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SplineInput {
    pub points: Id,
    pub degree: Option<u32>,
    pub weight: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnalysisDef {
    Root {
        input: RootInput,
        slot: u32,
    },
    Curvature {
        point: Id,
        source: Id,
        output: CurvatureKind,
    },
    ParametricDerivative(Id),
    Spline(SplineInput),
}

type Point = (f64, f64);

/// Separate output slots share a solve in the current propagation pass.
/// The construction clears these entries before updates or rollback.
#[derive(Default)]
pub(crate) struct AnalysisCache {
    roots: Mutex<HashMap<RootInput, Vec<Point>>>,
    splines: Mutex<HashMap<SplineInput, Option<Arc<Spline>>>>,
    /// For each spline object (owner index and definition), the number of
    /// points of its last solve that passed the degree check. Unlike the
    /// solves it outlives the propagation pass; rollback drops the entries
    /// of removed objects.
    spline_points: Mutex<Vec<(usize, SplineInput, usize)>>,
    ode_systems: Mutex<HashMap<[u32; 4], Vec<LocusData>>>,
}

impl AnalysisCache {
    pub(crate) fn clear(&mut self) {
        self.roots.get_mut().expect("root cache").clear();
        self.splines.get_mut().expect("spline cache").clear();
        self.ode_systems
            .get_mut()
            .expect("ODE system cache")
            .clear();
    }

    pub(crate) fn ode_system(
        &self,
        args: [u32; 4],
        slot: usize,
        solve: impl FnOnce() -> Vec<LocusData>,
    ) -> LocusData {
        self.ode_systems
            .lock()
            .expect("ODE system cache")
            .entry(args)
            .or_insert_with(solve)
            .get(slot)
            .cloned()
            .unwrap_or_default()
    }

    fn roots<T>(&self, c: &Construction, input: RootInput, read: impl FnOnce(&[Point]) -> T) -> T {
        if let Some(points) = self.roots.lock().expect("root cache").get(&input) {
            return read(points);
        }
        let points = roots::solve(c, input);
        let result = read(&points);
        self.roots.lock().expect("root cache").insert(input, points);
        result
    }

    /// Drops the spline states of the objects from `len` on.
    pub(crate) fn truncate(&mut self, len: usize) {
        self.spline_points
            .get_mut()
            .expect("spline states")
            .retain(|&(owner, ..)| owner < len);
    }

    /// The value of the spline object `owner`. `AlgoSpline.compute` checks
    /// the degree against the size of its point array, which it resizes
    /// only after the check passed: the number of points of the last solve
    /// that passed, or of the list when the command was created. A spline
    /// whose list shrank below the degree and grew again stays undefined.
    pub(crate) fn spline_object(
        &self,
        c: &Construction,
        owner: usize,
        input: SplineInput,
    ) -> Option<Value> {
        let points = c.elements(input.points)?.len();
        let mut states = self.spline_points.lock().expect("spline states");
        // The first solve of an object (its creation) sizes the array.
        let k = match states
            .iter()
            .position(|&(o, i, _)| o == owner && i == input)
        {
            Some(k) => k,
            None => {
                states.retain(|&(o, ..)| o != owner);
                states.push((owner, input, points));
                states.len() - 1
            }
        };
        if !spline_degree(c, input).is_some_and(|d| (3..=states[k].2).contains(&d)) {
            return None;
        }
        states[k].2 = points;
        drop(states);
        self.spline(c, input).map(|_| Value::Curve {
            start: 0.0,
            end: 1.0,
        })
    }

    pub(super) fn spline(&self, c: &Construction, input: SplineInput) -> Option<Arc<Spline>> {
        if let Some(spline) = self.splines.lock().expect("spline cache").get(&input) {
            return spline.clone();
        }
        let spline = Spline::solve(c, input).map(Arc::new);
        self.splines
            .lock()
            .expect("spline cache")
            .insert(input, spline.clone());
        spline
    }
}

/// The degree of a spline: `(int) degree.getDouble()`, 3 by default.
pub(super) fn spline_degree(c: &Construction, input: SplineInput) -> Option<usize> {
    match input.degree {
        Some(id) => {
            let degree = c.number(Id(id as usize))?;
            // Java's narrowing: NaN is 0, out of range saturates
            Some(if degree.is_nan() { 0 } else { degree.max(0.0) as usize })
        }
        None => Some(3),
    }
}

impl AnalysisDef {
    pub(crate) fn outputs(c: &Construction, function: Id, mode: Mode) -> Vec<Def> {
        let input = RootInput { function, mode };
        let count = c.analysis.roots(c, input, |points| points.len()).max(1);
        (0..count)
            .map(|slot| {
                Def::Analysis(Self::Root {
                    input,
                    slot: slot as u32,
                })
            })
            .collect()
    }

    pub(crate) fn uses_view(self) -> bool {
        matches!(
            self,
            Self::Root {
                input: RootInput {
                    mode: Mode::Roots(_) | Mode::Polynomial | Mode::Extremum(_),
                    ..
                },
                ..
            }
        )
    }
}

impl Group for AnalysisDef {
    fn kind(&self) -> Kind {
        match self {
            Self::Root { .. } => Kind::Point,
            Self::Curvature { output, .. } => output.kind(),
            Self::Spline(_) | Self::ParametricDerivative(_) => Kind::Curve,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            Self::Root {
                input: RootInput { function, mode },
                ..
            } => match mode {
                Mode::Seed(seed) => Parents::of(&[function, Id(seed as usize)]),
                Mode::SingleInterval(a, b)
                | Mode::Roots(Some((a, b)))
                | Mode::Extremum(Some((a, b))) => {
                    Parents::of(&[function, Id(a as usize), Id(b as usize)])
                }
                _ => Parents::of(&[function]),
            },
            Self::Curvature { point, source, .. } => Parents::of(&[point, source]),
            Self::ParametricDerivative(source) => Parents::of(&[source]),
            Self::Spline(SplineInput {
                points,
                degree,
                weight,
            }) => match (degree, weight) {
                (Some(d), Some(w)) => Parents::of(&[points, Id(d as usize), Id(w as usize)]),
                (Some(d), None) => Parents::of(&[points, Id(d as usize)]),
                (None, Some(w)) => Parents::of(&[points, Id(w as usize)]),
                (None, None) => Parents::of(&[points]),
            },
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            Self::Root { input, .. } => {
                c.expect(input.function, &[Kind::Function])?;
                for &id in self.parents().as_slice().iter().skip(1) {
                    c.expect(id, NUMBER)?;
                }
                Ok(())
            }
            Self::Curvature { point, source, .. } => {
                c.expect(point, POINT)?;
                c.expect(
                    source,
                    &[
                        Kind::Function,
                        Kind::Curve,
                        Kind::Conic,
                        Kind::Circle,
                        Kind::Arc,
                        Kind::Sector,
                    ],
                )
            }
            Self::ParametricDerivative(source) => c.expect(source, &[Kind::Curve]),
            Self::Spline(SplineInput {
                points,
                degree,
                weight,
            }) => {
                c.expect(points, &[Kind::List])?;
                if let Some(degree) = degree {
                    c.expect(Id(degree as usize), NUMBER)?;
                }
                if let Some(weight) = weight {
                    c.expect(Id(weight as usize), &[Kind::Function, Kind::FunctionNVar])?;
                }
                Ok(())
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            Self::Root { input, slot } => c.analysis.roots(c, input, |points| {
                let &(x, y) = points.get(slot as usize)?;
                (x.is_finite() && y.is_finite()).then_some(Value::Point { x, y })
            }),
            Self::Curvature {
                point,
                source,
                output,
            } => curvature::compute(c, point, source, output),
            Self::ParametricDerivative(source) => {
                let value @ Value::Curve { .. } = c.value(source) else {
                    return None;
                };
                Some(value)
            }
            // without an object, the check reads the current list
            Self::Spline(input) => {
                let points = c.elements(input.points)?.len();
                if !spline_degree(c, input).is_some_and(|d| (3..=points).contains(&d)) {
                    return None;
                }
                c.analysis.spline(c, input).map(|_| Value::Curve {
                    start: 0.0,
                    end: 1.0,
                })
            }
        }
    }

    fn parse(_c: &Construction, _func: &str, _args: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}
