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

type SplineStates = HashMap<usize, (SplineInput, usize)>;

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
    spline_points: Mutex<HashMap<usize, (SplineInput, usize)>>,
    /// The spline states of the construction copy each locus samples, by
    /// the locus's index; a copy starts with fresh objects.
    copy_spline_points: Mutex<HashMap<usize, SplineStates>>,
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

    /// Puts the spline states of the construction copy of the locus
    /// `owner` in place of the live ones (`AlgoLocusSliderND` samples its
    /// own copy, whose objects keep their states between updates) and
    /// returns the live ones. The copy is made when the locus first
    /// samples: each spline of it starts as a new object with the list as
    /// it is then.
    pub(crate) fn enter_copy(&self, c: &Construction, owner: usize) -> SplineStates {
        let mut live = self.spline_points.lock().expect("spline states");
        let copy = self
            .copy_spline_points
            .lock()
            .expect("copy spline states")
            .remove(&owner)
            .unwrap_or_else(|| {
                live.iter()
                    .map(|(&spline, &(input, _))| {
                        let points = c.elements(input.points).map_or(0, |l| l.len());
                        (spline, (input, points))
                    })
                    .collect()
            });
        std::mem::replace(&mut *live, copy)
    }

    /// Restores the live spline states and keeps the copy's for the next
    /// sampling of the locus `owner`.
    pub(crate) fn leave_copy(&self, owner: usize, live: SplineStates) {
        let copy = std::mem::replace(
            &mut *self.spline_points.lock().expect("spline states"),
            live,
        );
        self.copy_spline_points
            .lock()
            .expect("copy spline states")
            .insert(owner, copy);
    }

    /// Drops the spline states of the objects from `len` on.
    pub(crate) fn truncate(&mut self, len: usize) {
        self.spline_points
            .get_mut()
            .expect("spline states")
            .retain(|&owner, _| owner < len);
        let copies = self
            .copy_spline_points
            .get_mut()
            .expect("copy spline states");
        copies.retain(|&owner, _| owner < len);
        for states in copies.values_mut() {
            states.retain(|&owner, _| owner < len);
        }
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
        // The first solve of an object (its creation, or a new definition)
        // sizes the array.
        let state = states.entry(owner).or_insert((input, points));
        if state.0 != input {
            *state = (input, points);
        }
        if !spline_degree(c, input).is_some_and(|d| (3..=state.1).contains(&d)) {
            return None;
        }
        state.1 = points;
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
            Some(if degree.is_nan() {
                0
            } else {
                degree.max(0.0) as usize
            })
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
