//! Live outputs of the numerical function-analysis commands.

mod asymptote;
mod curvature;
mod roots;
mod spline;

use super::{Group, Parents, NUMBER, POINT};
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

/// Separate output slots share a solve until the construction propagates.
#[derive(Default)]
pub(crate) struct AnalysisCache {
    roots: Mutex<HashMap<RootInput, Vec<Point>>>,
    splines: Mutex<HashMap<SplineInput, Option<Arc<Spline>>>>,
}

impl AnalysisCache {
    pub(crate) fn clear(&mut self) {
        self.roots.get_mut().expect("root cache").clear();
        self.splines.get_mut().expect("spline cache").clear();
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
            Self::Spline(input) => c.analysis.spline(c, input).map(|_| Value::Curve {
                start: 0.0,
                end: 1.0,
            }),
        }
    }

    fn parse(_c: &Construction, _func: &str, _args: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}
