//! `Maximize(dep, indep)` and `Minimize(dep, indep)` over a bounded number.
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
use crate::kernel::{Construction, Error, Id, Kind, Value};

/// `Maximize(dep, indep)` / `Minimize(dep, indep)` over a number with slider
/// bounds (`AlgoOptimize` with `OptimizerNumeric`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimizeDef {
    pub dependent: Id,
    pub independent: Id,
    pub maximize: bool,
}

impl Group for OptimizeDef {
    fn kind(&self) -> Kind {
        Kind::Number
    }

    fn parents(&self) -> Parents {
        Parents::of(&[self.dependent, self.independent])
    }

    /// The dependent value is a number; the independent one a number, with
    /// its bounds checked when the search runs, not here (`CmdMinimize`
    /// only asks for a `GeoNumeric`).
    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.dependent, NUMBER)?;
        c.expect(self.independent, NUMBER)
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
                        let def = OptimizeDef {
                            dependent,
                            independent,
                            maximize,
                        };
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
    } = d;
    let Some((min, max)) = c.slider_interval(independent) else {
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
    let mut ids = chain.clone();
    ids.push(independent.0);
    let saved = crate::kernel::locus::Saved::of(c, &ids);
    let sign = if maximize { -1.0 } else { 1.0 };
    let optimum = crate::expr::fmin(min, max, |x| {
        // `Optimizer.value`: set the number, cascade its update, read the
        // dependent value
        c.set_free_number(independent, x);
        for &i in &chain {
            crate::kernel::locus::recompute(c, i);
        }
        sign * match c.value(dependent) {
            Value::Number(v) => v,
            _ => f64::NAN,
        }
    });
    // the number and the chain are restored with it, and the result keeps
    // the optimum alone (`result.set`, `setValue(old)`)
    saved.restore(c, &chain);
    match optimum {
        Some(x) => Value::Number(x),
        None => Value::Undefined,
    }
}
