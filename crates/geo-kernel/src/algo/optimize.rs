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
use crate::kernel::locus::{self, OptimizerCascade};
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
            // Interval listeners cascade through a slider only when the
            // slider value changes. Watching their endpoints directly would
            // rerun an optimizer even when the slider stays put.
            let parents = if matches!(c.def(id), Def::Number(_)) {
                Vec::new()
            } else {
                c.reads(id)
            };
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

    pub(crate) fn updates_construction(self, c: &Construction) -> bool {
        !self.disabled
            && self.watch != Watch::OverBudget
            && (self.point || c.has_slider_bounds(self.independent))
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
pub(crate) fn record(c: &mut Construction, owner: Id, d: OptimizeDef) -> Value {
    let mut cascade = OptimizerCascade::new(c, owner, d.independent);
    if let Some(cascade) = &mut cascade {
        return cascade.run(c, owner, d);
    }
    search(c, d, None)
}

pub(crate) fn search(
    c: &mut Construction,
    d: OptimizeDef,
    mut cascade: Option<&mut OptimizerCascade>,
) -> Value {
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
    // A circle whose command returned before writing keeps its last
    // record, and `OptimizerPoint.setValue` still places the point on it
    // (`pathChanged` reads the half axes and midpoint whatever `defined`
    // says): the search runs over that circle. Recorded in
    // misc-maximizepoint-008 to -017 and misc-minimizepoint-008 to -010.
    let mut record = None;
    let interval = if let Some((id, _)) = on_path {
        let geometry = path::geom(c, id);
        if geometry.is_defined() {
            Some(geometry.range())
        } else if matches!(
            c.def(id),
            Def::CircleRadius { .. } | Def::CircleThrough { .. }
        ) {
            record = c.circle_record(id).filter(|d| d.half_axes[0].is_finite());
            record.map(|d| path::Geom::Conic(d).range())
        } else {
            // A circle from an undefined radius segment remains unavailable
            // to the optimizer, even though its matrix readers can use it
            // (misc-maximizepoint-014).
            None
        }
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
    let chain = if cascade.is_none() {
        locus::chain(c, dependent, independent)
    } else {
        Vec::new()
    };
    if chain.len() as u64 > crate::expr::MAX_EVALUATION_WORK / 106 {
        return Value::Undefined;
    }
    if on_path.is_none() {
        // `Optimizer.setValue` goes through `GeoNumeric.setValue`, which
        // drops an exact decimal of the number for good; the trials must
        // not read it.
        crate::expr::forget_decimal(c, independent);
    }
    let saved = cascade.is_none().then(|| {
        let mut ids = chain.clone();
        ids.push(independent.0);
        locus::Saved::of(c, &ids)
    });
    let set_parameter = |c: &mut Construction, x| {
        if let Some((path, state)) = on_path {
            let mut state = c.path_state(independent).unwrap_or(state);
            state.set_t(x);
            match record {
                Some(d) => locus::sample_path_point_on(c, independent, d, state, state.parameter()),
                None => locus::sample_path_point(c, independent, path, state, state.parameter()),
            }
        } else {
            c.set_free_number(independent, x);
            c.value(independent)
        }
    };
    let old = on_path
        .map(|(_, state)| state.parameter())
        .or_else(|| c.number(independent))
        .unwrap_or(f64::NAN);
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
        set_parameter(c, x);
        if let Some(cascade) = cascade.as_deref_mut() {
            if !cascade.update(c, false) {
                return f64::NAN;
            }
        }
        // The locus recompute also updates the homogeneous point records
        // read by later objects in the trial.
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
    if let Some(cascade) = cascade {
        let result = set_parameter(c, optimum.unwrap_or(f64::NAN));
        cascade.set_result(c, result);
        set_parameter(c, old);
        cascade.update(c, true);
        return result;
    }
    let result = match (exhausted, on_path) {
        (true, _) => Value::Undefined,
        (false, Some(_)) => optimum
            .or(undefined_trial)
            .map_or(Value::Undefined, |x| set_parameter(c, x)),
        (false, None) => optimum.map_or(Value::Undefined, Value::Number),
    };
    // Restore the chain along with the mover after an isolated search.
    saved.expect("saved isolated search").restore(c, &chain);
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
        check_expression_bounds("a = Slider(0, sqrt(b))", false);
        // Also preserve the saved-file path with a free number whose exact
        // decimal must be dropped before the optimizer samples it.
        check_expression_bounds("a = 0", true);
    }

    fn check_expression_bounds(slider: &str, restored: bool) {
        let mut c = Construction::new();
        for line in ["b = 36", slider, "w = 2", "g = w + 1", "f = (a - g)^2"] {
            script::exec(&mut c, line).unwrap();
        }
        if restored {
            let a = c.id("a").unwrap();
            c.restore_slider_bounds(a, [Some("0"), Some("sqrt(b)")])
                .unwrap();
        }
        let (b, w) = (c.id("b").unwrap(), c.id("w").unwrap());
        let checkpoint = c.checkpoint();
        script::exec(&mut c, "m = Minimize(f, a)").unwrap();
        let near = |v: Option<f64>| v.is_some_and(|v| (v - 3.0).abs() < 1e-6);
        assert!(near(m(&c)), "{:?}", m(&c));
        c.set_number(b, -1.0).unwrap();
        assert!(near(m(&c)), "{:?}", m(&c));
        c.set_number(w, 3.0).unwrap();
        assert_eq!(m(&c), None);
        c.set_number(b, 36.0).unwrap();
        assert_eq!(m(&c), None);
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

#[cfg(test)]
mod cascade_tests {
    use crate::{script, Value};

    #[test]
    fn nested_searches_stop_at_the_nesting_limit_and_restore_the_mover() {
        let mut c = script::load("a = Slider(0, 6)\nf = (a - 3)^2").unwrap();
        let a = c.id("a").unwrap();
        let f = c.id("f").unwrap();
        c.set_number(a, 4.0).unwrap();
        for i in 0..9 {
            script::exec(&mut c, &format!("m_{{{i}}} = Minimize(f, a)")).unwrap();
        }
        let last = c.id("m_{8}").unwrap();
        assert_eq!(c.value(last), Value::Undefined);
        assert_eq!(c.value(a), Value::Number(4.0));
        assert_eq!(c.value(f), Value::Number(1.0));
        c.set_number(a, 1.0).unwrap();
        assert_eq!(c.value(last), Value::Undefined);
        assert_eq!(c.value(a), Value::Number(1.0));
        assert_eq!(c.value(f), Value::Number(4.0));
    }

    #[test]
    fn final_update_recomputes_an_optimizer_on_another_mover() {
        let mut c = script::load(
            "b = 36\na = Slider(0, sqrt(b))\nm = Minimize((a - 3)^2, a)\n\
             t = Slider(0, 6)\nd = 2m",
        )
        .unwrap();
        let [b, m, d] = ["b", "m", "d"].map(|label| c.id(label).unwrap());
        c.set_number(b, -1.0).unwrap();
        assert_eq!(c.value(m), Value::Number(3.0));
        script::exec(&mut c, "n = Minimize((t - 2)^2, t)").unwrap();
        assert_eq!(c.value(m), Value::Undefined);
        assert_eq!(c.value(d), Value::Undefined);
    }

    #[test]
    fn final_update_shares_work_across_optimizer_descendants() {
        let mut c = script::load(
            "b = 36\na = Slider(0, sqrt(b))\nm = Minimize((a - 3)^2, a)\nt = Slider(0, 6)",
        )
        .unwrap();
        for i in 1..=20 {
            script::exec(
                &mut c,
                &format!("d_{{{i}}} = If(IsDefined(m), 0, Sum(Sequence(sqrt(j), j, 1, 1000000)))"),
            )
            .unwrap();
        }
        c.set_number(c.id("b").unwrap(), -1.0).unwrap();

        let before = crate::expr::work_done();
        script::exec(&mut c, "n = Minimize((t - 2)^2, t)").unwrap();
        let work = crate::expr::work_done() - before;
        assert_eq!(c.value(c.id("m").unwrap()), Value::Undefined);
        assert!(c.number(c.id("d_{1}").unwrap()).unwrap() > 0.0);
        assert!(
            work <= crate::expr::MAX_EVALUATION_WORK + crate::expr::MAX_OVERRUN + 4096,
            "final update and the small outer search visited {work} work units",
        );

        let unrelated = script::exec(&mut c, "unrelated = 0").unwrap().unwrap();
        assert_eq!(c.set_number(unrelated, 1.0).unwrap(), 0);
    }

    #[test]
    fn cascade_exhaustion_restores_expensive_readers_within_the_shared_budget() {
        let mut c =
            script::load("a = Slider(0, 6)\nb = 0\nt = Take({1, 2, 3}, If(b > 0 && a > 0, 2, 4))")
                .unwrap();
        for i in 1..=20 {
            script::exec(
                &mut c,
                &format!("d_{{{i}}} = Sum(Sequence(sqrt(j + a), j, 1, 100000))"),
            )
            .unwrap();
        }
        script::exec(&mut c, "m = Minimize((a - 3)^2, a)").unwrap();
        // A positive trial gives Take two slots. A failed cascade must
        // leave its initially invalid result with no retained slots.
        c.set_number(c.id("b").unwrap(), 1.0).unwrap();
        let reader = c.id("d_{1}").unwrap();
        let old = c.value(reader);
        let before = crate::expr::work_done();
        let n = script::exec(&mut c, "n = Minimize((a - 2)^2, a)")
            .unwrap()
            .unwrap();
        let work = crate::expr::work_done() - before;
        assert_eq!(c.value(n), Value::Undefined);
        assert_eq!(c.value(c.id("a").unwrap()), Value::Number(0.0));
        assert_eq!(c.value(reader), old);
        let check = script::exec(&mut c, "check = d_{1}").unwrap().unwrap();
        assert_eq!(c.value(check), old);
        let length = script::exec(&mut c, "length = Length(t + 1)")
            .unwrap()
            .unwrap();
        assert_eq!(c.value(length), Value::Number(0.0));
        assert!(
            work <= crate::expr::MAX_EVALUATION_WORK + crate::expr::MAX_OVERRUN + 4096,
            "exhausted search and rollback visited {work} work units",
        );
    }

    #[test]
    fn cascade_exhaustion_restores_the_first_optimizer_and_all_twenty_readers() {
        let mut c = script::load("a = Slider(0, 6)\nh = 1\nk = 1").unwrap();
        let readers: Vec<_> = (1..=20)
            .map(|i| {
                script::exec(
                    &mut c,
                    &format!("d_{{{i}}} = Sum(Sequence(sqrt(j + a), j, 1, h))"),
                )
                .unwrap()
                .unwrap()
            })
            .collect();
        script::exec(&mut c, "m = Minimize((a - 3)^2, a)").unwrap();
        script::exec(&mut c, "n = Minimize(k * (a - 2)^2, a)").unwrap();
        let [a, h, k, m, n] = ["a", "h", "k", "m", "n"].map(|label| c.id(label).unwrap());
        assert_eq!(c.value(n), Value::Number(2.0));

        c.set_number(h, 100_000.0).unwrap();
        let previous_readers: Vec<_> = readers.iter().map(|&id| c.value(id)).collect();
        assert_eq!(c.value(m), Value::Number(3.0));
        assert!(previous_readers
            .iter()
            .all(|value| matches!(value, Value::Number(sum) if *sum > 1.0)));

        c.set_number(k, 2.0).unwrap();
        assert_eq!(c.value(n), Value::Undefined);
        assert_eq!(c.value(a), Value::Number(0.0));
        assert_eq!(c.value(m), Value::Number(3.0));
        for (id, previous) in readers.into_iter().zip(previous_readers) {
            assert_eq!(c.value(id), previous, "{}", c.label(id));
        }
    }

    #[test]
    fn two_optimizers_on_a_circle_with_a_kept_record() {
        let mut c = script::load(
            "r = 5\nC = (1, 2)\nc = Circle(C, r)\nP = Point(c)\nk = 1\n\
             M = Maximize(x(P) + k y(P), P)\nN = Minimize(x(P) - y(P), P)",
        )
        .unwrap();
        let [r, p, k, m, n] = ["r", "P", "k", "M", "N"].map(|label| c.id(label).unwrap());
        let start = c.value(p);
        let maximum = c.value(m);
        let minimum = c.value(n);
        c.set_number(r, -1.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        assert_eq!(c.value(m), maximum);
        assert_eq!(c.value(n), start);
        c.set_number(k, 2.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        assert_eq!(c.value(m), start);
        assert_eq!(c.value(n), minimum);
    }
}
