//! Optimize an objective over a bounded slider or a point on a path.
//!
//! GeoGebra computes the numeric forms without a CAS: `CmdMaximize` and
//! `CmdMinimize` accept a dependent number and an independent number and run
//! `AlgoMaximize`/`AlgoMinimize`, both `AlgoOptimize` with an
//! `OptimizerNumeric`. The optimizer packages the construction as one
//! univariate function: set the independent number, cascade its update,
//! then read the dependent number. The kernel searches for the extreme
//! over the slider interval by parabolic interpolation with a
//! golden-section fallback.
//! The result is a copy of the independent number at the optimum; the number
//! itself is restored and the construction recomputed afterwards, so nothing
//! but the result remembers the search.
//!
//! Without slider bounds the result is undefined (`OptimizerNumeric
//! .hasBounds`); the search itself also fails, to an undefined result, when
//! the sampled value is undefined at the start point or the iteration limit
//! runs out.

use crate::algo::{Group, Parents, NUMBER};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};

use super::path::{self, PathDef};

/// `Maximize(dep, indep)` / `Minimize(dep, indep)` over a number with slider
/// bounds (`AlgoOptimize` with `OptimizerNumeric`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimizeDef {
    pub dependent: Id,
    pub independent: Id,
    pub maximize: bool,
    point: bool,
}

impl OptimizeDef {
    pub(crate) fn new(c: &Construction, dependent: Id, independent: Id, maximize: bool) -> Self {
        Self {
            dependent,
            independent,
            maximize,
            point: c.kind(independent) == Kind::Point,
        }
    }
}

impl Group for OptimizeDef {
    fn kind(&self) -> Kind {
        if self.point {
            Kind::Point
        } else {
            Kind::Number
        }
    }

    fn parents(&self) -> Parents {
        Parents::of(&[self.dependent, self.independent])
    }

    /// The dependent value is a number; the independent one a number, with
    /// its bounds checked when the search runs, not here (`CmdMinimize`
    /// only asks for a `GeoNumeric`).
    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.dependent, NUMBER)?;
        if self.point {
            c.expect(self.independent, &[Kind::Point])
        } else {
            c.expect(self.independent, NUMBER)
        }
    }

    /// The search mutates the construction, so the value comes from
    /// [`record`] through the update machinery, like a locus.
    fn compute(&self, _c: &Construction) -> Option<Value> {
        None
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let maximize = match func {
            "Maximize" => true,
            "Minimize" => false,
            _ => return None,
        };
        Some(match args {
            [dependent, independent] => {
                match (
                    crate::script::object(c, dependent),
                    crate::script::object(c, independent),
                ) {
                    (Ok(dependent), Ok(independent)) => {
                        let def = Self::new(c, dependent, independent, maximize);
                        def.check(c).map_err(|e| e.to_string()).map(|_| def)
                    }
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            _ => Err(format!("`{func}` does not take {} argument(s)", args.len())),
        })
    }
}

/// Runs the search and returns the value of the result: the independent
/// number at the optimum, or undefined when it has no bounds or the search
/// fails (`AlgoOptimize.compute`).
pub(crate) fn record(c: &mut Construction, d: OptimizeDef) -> Value {
    let OptimizeDef {
        dependent,
        independent,
        maximize,
        point,
    } = d;
    let on_path = if point {
        let Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) = c.def(independent)
        else {
            return Value::Undefined;
        };
        let Some(state) = c.path_state(independent) else {
            return Value::Undefined;
        };
        Some((path, state))
    } else {
        None
    };
    let interval = if let Some((id, _)) = on_path {
        let geometry = path::geom(c, id);
        geometry.is_defined().then(|| geometry.range())
    } else {
        c.slider_interval(independent)
    };
    let Some((min, max)) = interval else {
        return Value::Undefined;
    };
    // A slider with `min == max` still has bounds (`hasBounds`): the
    // interval is one point and the search returns it at once. Only a
    // reversed interval (`Slider(5, 3)`) has no search.
    if !max.is_finite() || min > max {
        return Value::Undefined;
    }
    // what one sample recomputes: everything between the number and the
    // dependent value (`updateCascade` from the number)
    let chain = crate::kernel::locus::chain(c, dependent, independent);
    if point && chain.len() as u64 > crate::expr::MAX_EVALUATION_WORK / 106 {
        return Value::Undefined;
    }
    let mut ids = chain.clone();
    ids.push(independent.0);
    let saved = crate::kernel::locus::Saved::of(c, &ids);
    let sign = if maximize { -1.0 } else { 1.0 };
    let mut first = None;
    let optimum = crate::expr::fmin(min, max, |x| {
        // `Optimizer.value`: set the number, cascade its update, read the
        // dependent value
        if let Some((path, state)) = on_path {
            crate::kernel::locus::sample_path_point(c, independent, path, state, x);
        } else {
            c.set_free_number(independent, x);
        }
        for &i in &chain {
            crate::kernel::locus::recompute(c, i);
        }
        let objective = sign
            * match c.value(dependent) {
                Value::Number(v) => v,
                _ => f64::NAN,
            };
        first.get_or_insert((x, objective));
        objective
    });
    let result = match on_path {
        Some((path, state)) => {
            // An undefined initial objective leaves the first trial in
            // place in the reference search.
            let optimum = optimum
                .or_else(|| first.and_then(|(x, objective)| objective.is_nan().then_some(x)));
            optimum.map_or(Value::Undefined, |x| {
                crate::kernel::locus::sample_path_point(c, independent, path, state, x)
            })
        }
        None => optimum.map_or(Value::Undefined, Value::Number),
    };
    // the number and the chain are restored with it, and the result keeps
    // the optimum alone (`result.set`, `setValue(old)`)
    saved.restore(c, &chain);
    result
}
