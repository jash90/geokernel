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
//! a trial value is undefined or the iteration limit
//! runs out.

use crate::algo::{Group, Parents, NUMBER};
use crate::kernel::{Construction, Def, Error, Id, IdList, Kind, Value};
use std::collections::HashSet;

use super::path::{self, PathDef};

/// `Maximize(dep, indep)` / `Minimize(dep, indep)` over a number with slider
/// bounds (`AlgoOptimize` with `OptimizerNumeric`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimizeDef {
    pub dependent: Id,
    pub independent: Id,
    pub maximize: bool,
    point: bool,
    /// `AlgoOptimize.compute` met the number without slider bounds: it
    /// returns with its recursion guard still set, so the result stays
    /// undefined from then on. The watched objects are kept, so the
    /// dependency edges stay those of the definition.
    disabled: bool,
    watch: Watch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Watch {
    Unattached,
    Attached(IdList),
    OverBudget,
}

impl OptimizeDef {
    pub(crate) fn new(c: &Construction, dependent: Id, independent: Id, maximize: bool) -> Self {
        Self {
            dependent,
            independent,
            maximize,
            point: c.kind(independent) == Kind::Point,
            disabled: false,
            watch: Watch::Unattached,
        }
    }

    pub(crate) fn watched(self) -> Option<IdList> {
        match self.watch {
            Watch::Attached(list) => Some(list),
            Watch::Unattached | Watch::OverBudget => None,
        }
    }

    // An edit can change the objective at a trial point while leaving its
    // value at the mover unchanged. Watch the objective's inputs directly.
    pub(crate) fn attach(mut self, c: &mut Construction) -> Result<Self, Error> {
        if self.watch != Watch::Unattached {
            return Ok(self);
        }
        const MAX_DEPENDENCY_EDGES: usize = 16_384;
        let mut remaining = MAX_DEPENDENCY_EDGES;
        let mut seen = HashSet::new();
        let mut watched = Vec::new();
        let mut pending = vec![self.dependent, self.independent];
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let parents = c.reads(id);
            let Some(left) = remaining.checked_sub(parents.len()) else {
                self.watch = Watch::OverBudget;
                return Ok(self);
            };
            remaining = left;
            watched.push(id);
            pending.extend(parents);
        }
        self.watch = Watch::Attached(c.store_list(&watched)?);
        Ok(self)
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

/// The definition to keep once `d` meets a number without slider bounds
/// (`OptimizerNumeric.hasBounds`): `AlgoOptimize.compute` sets its guard
/// against recursive calls and returns before clearing it, so every later
/// update returns at once and the result stays undefined, also after
/// bounds are added. The endpoints count as bounds whatever their values.
/// A point on a path always has bounds.
pub(crate) fn disabled(c: &Construction, d: OptimizeDef) -> Option<OptimizeDef> {
    (!d.point && !d.disabled && !c.has_slider_bounds(d.independent)).then_some(OptimizeDef {
        disabled: true,
        ..d
    })
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
        disabled,
        watch,
    } = d;
    if disabled || watch == Watch::OverBudget {
        return Value::Undefined;
    }
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
    if chain.len() as u64 > crate::expr::MAX_EVALUATION_WORK / 106 {
        return Value::Undefined;
    }
    let mut ids = chain.clone();
    ids.push(independent.0);
    let saved = crate::kernel::locus::Saved::of(c, &ids);
    let sign = if maximize { -1.0 } else { 1.0 };
    let mut undefined_trial = None;
    let start = crate::expr::work_done();
    let mut exhausted = false;
    let optimum = local_min(min, max, TOLERANCE, |x| {
        if exhausted {
            return f64::NAN;
        }
        // `Optimizer.value`: set the number, cascade its update, read the
        // dependent value
        if let Some((path, state)) = on_path {
            crate::kernel::locus::sample_path_point(c, independent, path, state, x);
        } else {
            c.set_free_number(independent, x);
        }
        for &i in &chain {
            crate::kernel::locus::recompute(c, i);
            crate::expr::charge(1);
            if crate::expr::work_done() - start > crate::expr::MAX_EVALUATION_WORK {
                exhausted = true;
                return f64::NAN;
            }
        }
        let objective = sign
            * match c.value(dependent) {
                Value::Number(v) => v,
                _ => f64::NAN,
            };
        if objective.is_nan() {
            undefined_trial.get_or_insert(x);
        }
        objective
    });
    let result = match (exhausted, on_path) {
        (true, _) => Value::Undefined,
        (false, Some((path, state))) => {
            // An undefined search parameter leaves a path point at the
            // trial that encountered the undefined objective.
            let optimum = optimum.or(undefined_trial);
            optimum.map_or(Value::Undefined, |x| {
                crate::kernel::locus::sample_path_point(c, independent, path, state, x)
            })
        }
        (false, None) => optimum.map_or(Value::Undefined, Value::Number),
    };
    // the number and the chain are restored with it, and the result keeps
    // the optimum alone (`result.set`, `setValue(old)`)
    saved.restore(c, &chain);
    result
}

/// The precision `AlgoOptimize` asks of its search.
const TOLERANCE: f64 = 5.0e-8;

/// Most iterations of [`local_min`] after the first sample.
const MAX_ITERATIONS: usize = 101;

/// The minimum search of `AlgoOptimize`: Brent's method without
/// derivatives (Brent, *Algorithms for Minimization without Derivatives*,
/// 1973, chapter 5; the `fmin` procedure of Forsythe, Malcolm and Moler,
/// *Computer Methods for Mathematical Computations*, 1977), parabolic
/// interpolation safeguarded by golden-section steps.
///
/// The details are fixed from black-box recordings of the reference's
/// search (its sample sequences, not its source): the
/// relative part of the step floor is the square root of `1.2e-16`; a zero
/// or undefined step falls back to the floor downwards; a parabola with an
/// undefined coefficient is refused; a sample that equals the best value
/// moves to it and narrows the interval to the two samples; the first
/// undefined sample ends the search without a result, and so does an
/// iteration past [`MAX_ITERATIONS`]. Checked against the reference's own
/// sample sequences for random objectives and intervals.
fn local_min(a: f64, b: f64, tol: f64, mut f: impl FnMut(f64) -> f64) -> Option<f64> {
    let golden = 0.5 * (3.0 - 5.0_f64.sqrt());
    let eps = 1.2e-16_f64.sqrt();
    let toward = |floor: f64, d: f64| if d > 0.0 { floor } else { -floor };
    let (mut a, mut b) = (a, b);
    let mut x = a + golden * (b - a);
    let (mut v, mut w) = (x, x);
    let (mut d, mut e) = (0.0_f64, 0.0_f64);
    let mut fx = f(x);
    if fx.is_nan() {
        return None;
    }
    let (mut fv, mut fw) = (fx, fx);
    for _ in 0..MAX_ITERATIONS {
        let xm = 0.5 * (a + b);
        let tol1 = eps * x.abs() + tol / 3.0;
        let tol2 = 2.0 * tol1;
        if (x - xm).abs() <= tol2 - 0.5 * (b - a) {
            return Some(x);
        }
        let mut parabolic = false;
        if e.abs() > tol1 {
            let r = (x - w) * (fx - fv);
            let mut q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            q = 2.0 * (q - r);
            if q > 0.0 {
                p = -p;
            }
            q = q.abs();
            let previous = e;
            e = d;
            if p.abs() < (0.5 * q * previous).abs() && p > q * (a - x) && p < q * (b - x) {
                d = p / q;
                let u = x + d;
                if u - a < tol2 || b - u < tol2 {
                    d = toward(tol1, xm - x);
                }
                parabolic = true;
            }
        }
        if !parabolic {
            e = if x >= xm { a - x } else { b - x };
            d = golden * e;
        }
        let u = if d.abs() >= tol1 {
            x + d
        } else {
            x + toward(tol1, d)
        };
        let fu = f(u);
        if fu.is_nan() {
            return None;
        }
        if fu <= fx {
            if fu == fx {
                (a, b) = if u >= x { (x, u) } else { (u, x) };
            } else if u >= x {
                a = x;
            } else {
                b = x;
            }
            (v, fv, w, fw, x, fx) = (w, fw, x, fx, u, fu);
            continue;
        }
        if u < x {
            a = u;
        } else {
            b = u;
        }
        if fu <= fw || w == x {
            (v, fv, w, fw) = (w, fw, u, fu);
        } else if fu <= fv || v == x || v == w {
            (v, fv) = (u, fu);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::local_min;

    #[test]
    fn local_min_follows_the_recorded_samples() {
        // the reference's samples for the maximum of 4 + 5 sin(t) on
        // [-pi, pi] (misc-maximizepoint-001)
        let mut samples = Vec::new();
        let pi = std::f64::consts::PI;
        let best = local_min(-pi, pi, 5.0e-8, |t| {
            samples.push(t);
            -(4.0 + 5.0 * t.sin())
        });
        assert_eq!(best, Some(1.5707963279430484));
        assert_eq!(
            samples,
            [
                -0.7416294238611401,
                0.7416294238611396,
                1.658333805867513,
                1.9482463800077365,
                1.5642244467043525,
                1.5702086670936306,
                1.5707984276102673,
                1.5707963279430484,
                1.57079629406917,
                1.5707963618169267,
            ]
        );
    }

    #[test]
    fn local_min_gives_up_on_undefined_samples_and_long_searches() {
        assert_eq!(local_min(0.0, 1.0, 5.0e-8, |_| f64::NAN), None);
        let mut count = 0;
        let best = local_min(-1e300, 1e300, 1e-300, |x| {
            count += 1;
            (x - 1e-20).abs()
        });
        assert_eq!((best, count), (None, 102));
        // a constant objective narrows onto the golden sections
        assert!(local_min(-6.0, 4.0, 5.0e-8, |_| 1.0).is_some());
    }
}

#[cfg(test)]
mod bound_tests {
    use crate::{script, Construction, Value};

    fn m(c: &Construction) -> Option<f64> {
        match c.value(c.id("m").unwrap()) {
            Value::Number(v) => Some(v),
            _ => None,
        }
    }

    /// Case `misc-optimizebounds-001`: endpoint objects are bounds whatever
    /// their values, and rollback leaves no stale edge.
    #[test]
    fn expression_bounds_do_not_disable_and_rollback_is_clean() {
        let mut c = Construction::new();
        for line in ["b = 36", "a = 0", "w = 2", "g = w + 1", "f = (a - g)^2"] {
            script::exec(&mut c, line).unwrap();
        }
        let (a, b, w) = (c.id("a").unwrap(), c.id("b").unwrap(), c.id("w").unwrap());
        c.restore_slider_bounds(a, [Some("0"), Some("sqrt(b)")])
            .unwrap();
        let checkpoint = c.checkpoint();
        script::exec(&mut c, "m = Minimize(f, a)").unwrap();
        let near = |v: Option<f64>| v.is_some_and(|v| (v - 3.0).abs() < 1e-6);
        // defined at creation (its value there is not compared: the search
        // after restoring bounds is a separate matter)
        assert!(m(&c).is_some());
        c.set_number(b, -1.0).unwrap();
        c.set_number(w, 3.0).unwrap();
        assert_eq!(m(&c), None);
        c.set_number(b, 36.0).unwrap();
        c.set_number(w, 2.0).unwrap();
        assert!(near(m(&c)), "{:?}", m(&c));
        // the same after the optimizer is rolled back
        c.set_number(b, -1.0).unwrap();
        c.set_number(w, 3.0).unwrap();
        c.rollback(checkpoint).unwrap();
        c.set_number(w, 4.0).unwrap();
        assert!(c.id("m").is_none());
    }
}
