// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFunctionAreaSums.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLowerSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdUpperSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLeftSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRectangleSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdTrapezoidalSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdIntegral.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoIntegralDefinite.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/Bounds.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoIntegralFunctions.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdDerivative.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoDerivative.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Function.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/PolyFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/SymbolicPolyFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Derivative.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/FunctionVariable.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoIntegralNumericInterval.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolynomialFromCoordinates.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The calculus commands of the function group: the Riemann sums, the
//! definite and indefinite integrals, the derivatives, and polynomial
//! interpolation. Symbolic derivatives and antiderivatives of
//! non-polynomials still follow the upstream no-CAS behavior.
//!
//! A polynomial source takes the exact path of GeoGebra's
//! `PolyFunction`: the expression is expanded once per update (the shared
//! expansion of `expr::equation`) and the built tree is kept in the
//! construction's calculus records, sampled through the ordinary
//! evaluator. `NDerivative` of a non-polynomial builds the derivative tree
//! of `Derivative.get`, one `ExpressionNode.derivative` pass per order.

use super::{Group, Parents, POINT};
use crate::expr::derivative::decimal_to_fraction;
use crate::expr::equation::Builder;
use crate::expr::{self, Bin, Element, Entry, IfMode, Node, Parsed, Raw, Un, NONE};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

/// The largest number of rectangles a Riemann sum may use
/// (`AlgoFunctionAreaSums.MAX_RECTANGLES`).
const MAX_RECTANGLES: f64 = 10000.0;

/// The tolerance of the parabolic interpolation of the sums
/// (`AlgoFunctionAreaSums.TOLERANCE`).
const SUM_TOLERANCE: f64 = 1e-7;

/// The number of points the sums check a function on
/// (`AlgoFunctionAreaSums.CHECKPOINTS`).
const CHECKPOINTS: f64 = 100.0;

/// The graphics window of the headless application the golden data is
/// recorded with: the lower and upper sums subsample only inside it
/// (`kernel.getViewsXMin` of a construction without views), at 50 pixels
/// per unit (`App.countPixels`), which the locus sampling uses too.
const HEADLESS_VIEW: [f64; 6] = crate::kernel::locus::HEADLESS_VIEW;
const PIXELS_PER_UNIT: f64 = 50.0;

/// `Function.MAX_EXPAND_DEGREE`: a power whose exponent is above this and
/// whose base is no plain variable makes the derivative keep the tree
/// (`Function.getDerivative` refuses the polynomial expansion).
const MAX_EXPAND_DEGREE: f64 = 10.0;

/// How many nodes one tree of the no-CAS rules may hold: a product rule
/// roughly triples the tree per pass, so a high derivative order would
/// grow it without end while the work budget is only checked between the
/// passes (`rule_derivative`). Past it the derivative is undefined.
const MAX_RULE_NODES: usize = 4_000_000;

/// Which Riemann sum `Sum` computes (`AlgoFunctionAreaSums.SumType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sum {
    Lower,
    Upper,
    Left,
    Rectangle,
    Trapezoid,
}

/// One command of the calculus group. The objects a command reads are
/// `u32` indices (`Id` in a `usize`), so all its inputs still fit the
/// fixed size of a `Def`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CalculusDef {
    /// `LowerSum(f, a, b, n)`, `UpperSum`, `LeftSum`, `TrapezoidalSum`
    /// and `RectangleSum(f, a, b, n, divider)`.
    Sum {
        sum: Sum,
        function: u32,
        a: u32,
        b: u32,
        n: u32,
        /// The divider of `RectangleSum`, `None` for the other sums.
        divider: Option<u32>,
    },
    /// `Integral(f, a, b)`, `NIntegral(f, a, b)` and the evaluate form
    /// `Integral(f, a, b, evaluate)` (`AlgoIntegralDefinite`).
    DefiniteIntegral {
        function: u32,
        a: u32,
        b: u32,
        /// The `evaluate` boolean; `false` shades only and the number is
        /// undefined.
        evaluate: Option<u32>,
        /// The `NIntegral` flag: the symbolic helper is not even tried.
        numeric: bool,
    },
    /// `IntegralBetween(f, g, a, b)` and the four-number `Integral(f, c,
    /// a, b)` form (`AlgoIntegralFunctions`).
    IntegralFunctions {
        f: u32,
        g: u32,
        a: u32,
        b: u32,
        evaluate: Option<u32>,
    },
    /// `Integral(f)`, `Integral(f, var)` and `NIntegral(f)`
    /// (`AlgoIntegral`): a polynomial antiderivative without the CAS, the
    /// rule integrator of `NIntegral`, undefined otherwise.
    IndefiniteIntegral {
        function: u32,
        /// The `NIntegral` flag: the rule integrator instead of the
        /// polynomial path.
        numeric: bool,
    },
    /// `Derivative(f)`, `Derivative(f, order)` and its variable forms;
    /// `NDerivative` with `numeric` (`AlgoDerivative`).
    Derivative {
        function: u32,
        /// The order, `None` for the first derivative.
        order: Option<u32>,
        numeric: bool,
    },
    /// `Derivative(curve[, order])` and `NDerivative` of a curve
    /// (`GeoCurveCartesianND.setDerivative`): both coordinate functions
    /// differentiated on the fast no-CAS path, with the parent's parameter
    /// interval.
    CurveDerivative { curve: u32, order: Option<u32> },
    /// `NIntegral(f, a, b, c)` (`AlgoIntegralNumericInterval`): the
    /// antiderivative the rules build, shifted through `(a, b)` and cut
    /// to `[a, c]`.
    NumericInterval {
        function: u32,
        a: u32,
        b: u32,
        c: u32,
    },
    /// `Polynomial(points)`: the interpolating polynomial
    /// (`AlgoPolynomialFromCoordinates`).
    PolynomialPoints { points: u32 },
    /// `SolveODE(f[, P])` and `SolveODE(curve[, P])` (`AlgoSolveODECas`):
    /// the solution of `y' = f(x)` as a function. The kernel solves a
    /// polynomial right-hand side in `x` (its antiderivative, through `P`
    /// when there is one); everything else needs the CAS and is undefined,
    /// and a curve is undefined in the reference too.
    SymbolicOde { source: u32, point: Option<u32> },
    /// `Function(list)` (`AlgoFunctionFreehand`): the freehand function of
    /// the list `{min, max, y1, y2, ...}`, the values `y` at equal steps
    /// over `[min, max]`, linear between them and undefined outside.
    Freehand { list: u32 },
    /// `Function()` (`CmdDataFunction.emptyFunction`): the data function
    /// of no points, a defined function with no value anywhere.
    EmptyData,
}

/// The object index `x` as an `Id` again.
fn gid(x: u32) -> Id {
    Id(x as usize)
}

impl CalculusDef {
    /// Whether the value is a function whose tree is kept in a calculus
    /// record (see `Construction::calculus`).
    pub(crate) fn has_record(&self) -> bool {
        matches!(
            self,
            CalculusDef::IndefiniteIntegral { .. }
                | CalculusDef::Derivative { .. }
                | CalculusDef::CurveDerivative { .. }
                | CalculusDef::NumericInterval { .. }
                | CalculusDef::PolynomialPoints { .. }
                | CalculusDef::SymbolicOde { .. }
                | CalculusDef::Freehand { .. }
                | CalculusDef::EmptyData
        )
    }

    /// The value of the object `owner` at `x`, for a record-backed
    /// function.
    pub(crate) fn record_value(
        &self,
        c: &Construction,
        record: &CalcRecord,
        owner: Id,
        x: f64,
    ) -> Option<f64> {
        let _ = owner;
        match *self {
            // these two keep no tree: their values come from the list
            CalculusDef::Freehand { list } => Some(freehand_value(c, gid(list), x)),
            CalculusDef::EmptyData => Some(f64::NAN),
            _ => record
                .tree
                .as_ref()
                .map(|entry| expr::sample_tree(c, entry, x)),
        }
    }
}

pub(crate) struct CalcRecord {
    pub(crate) tree: Option<Entry>,
    /// The second coordinate tree of a curve derivative (`None` for every
    /// other record-backed object).
    pub(crate) tree2: Option<Entry>,
}

/// Initializes the record of a record-backed calculus object and its
/// value; called from `Construction::add`.
pub(crate) fn initialize(c: &Construction, d: &CalculusDef) -> (Value, CalcRecord) {
    refresh(c, d)
}

/// Recomputes the record of a record-backed calculus object and its
/// value.
pub(crate) fn refresh(c: &Construction, d: &CalculusDef) -> (Value, CalcRecord) {
    let (tree, tree2) = build_record(c, d);
    let defined = |entry: &Option<Entry>| {
        entry
            .as_ref()
            .is_some_and(|entry| expr::tree_defined(c, entry))
    };
    let value = match *d {
        // the curve keeps its parent's interval; it is defined when both
        // coordinates differentiated
        CalculusDef::CurveDerivative { curve, .. } => match c.value(gid(curve)) {
            Value::Curve { start, end } if defined(&tree) && defined(&tree2) => {
                Value::Curve { start, end }
            }
            _ => Value::Undefined,
        },
        CalculusDef::Freehand { list } if freehand_defined(c, gid(list)) => Value::Function,
        CalculusDef::EmptyData => Value::Function,
        _ if defined(&tree) => Value::Function,
        _ => Value::Undefined,
    };
    (value, CalcRecord { tree, tree2 })
}

impl Group for CalculusDef {
    fn kind(&self) -> Kind {
        match self {
            CalculusDef::Sum { .. }
            | CalculusDef::DefiniteIntegral { .. }
            | CalculusDef::IntegralFunctions { .. } => Kind::Number,
            CalculusDef::IndefiniteIntegral { .. }
            | CalculusDef::Derivative { .. }
            | CalculusDef::NumericInterval { .. }
            | CalculusDef::PolynomialPoints { .. }
            | CalculusDef::SymbolicOde { .. }
            | CalculusDef::Freehand { .. }
            | CalculusDef::EmptyData => Kind::Function,
            CalculusDef::CurveDerivative { .. } => Kind::Curve,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            CalculusDef::Sum {
                function,
                a,
                b,
                n,
                divider,
                ..
            } => match divider {
                Some(d) => Parents::of(&[gid(function), gid(a), gid(b), gid(n), gid(d)]),
                None => Parents::of(&[gid(function), gid(a), gid(b), gid(n)]),
            },
            CalculusDef::DefiniteIntegral {
                function,
                a,
                b,
                evaluate,
                ..
            } => match evaluate {
                Some(e) => Parents::of(&[gid(function), gid(a), gid(b), gid(e)]),
                None => Parents::of(&[gid(function), gid(a), gid(b)]),
            },
            CalculusDef::IntegralFunctions {
                f,
                g,
                a,
                b,
                evaluate,
            } => match evaluate {
                Some(e) => Parents::of(&[gid(f), gid(g), gid(a), gid(b), gid(e)]),
                None => Parents::of(&[gid(f), gid(g), gid(a), gid(b)]),
            },
            CalculusDef::IndefiniteIntegral { function, .. } => Parents::of(&[gid(function)]),
            CalculusDef::Derivative {
                function, order, ..
            } => match order {
                Some(order) => Parents::of(&[gid(function), gid(order)]),
                None => Parents::of(&[gid(function)]),
            },
            CalculusDef::CurveDerivative { curve, order } => match order {
                Some(order) => Parents::of(&[gid(curve), gid(order)]),
                None => Parents::of(&[gid(curve)]),
            },
            CalculusDef::NumericInterval {
                function, a, b, c, ..
            } => Parents::of(&[gid(function), gid(a), gid(b), gid(c)]),
            CalculusDef::PolynomialPoints { points } => Parents::of(&[gid(points)]),
            CalculusDef::SymbolicOde { source, point } => match point {
                Some(point) => Parents::of(&[gid(source), gid(point)]),
                None => Parents::of(&[gid(source)]),
            },
            CalculusDef::Freehand { list } => Parents::of(&[gid(list)]),
            CalculusDef::EmptyData => Parents::none(),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        let number = |id: Id| c.expect(id, super::NUMBER_OR_ANGLE);
        let boolean = |id: Id| c.expect(id, &[Kind::Boolean]);
        match *self {
            CalculusDef::Sum {
                function,
                a,
                b,
                n,
                divider,
                ..
            } => {
                c.expect(gid(function), &[Kind::Function])?;
                number(gid(a))?;
                number(gid(b))?;
                number(gid(n))?;
                if let Some(d) = divider {
                    number(gid(d))?;
                }
            }
            CalculusDef::DefiniteIntegral {
                function,
                a,
                b,
                evaluate,
                ..
            } => {
                c.expect(gid(function), &[Kind::Function])?;
                number(gid(a))?;
                number(gid(b))?;
                if let Some(e) = evaluate {
                    boolean(gid(e))?;
                }
            }
            CalculusDef::IntegralFunctions {
                f,
                g,
                a,
                b,
                evaluate,
                ..
            } => {
                c.expect(gid(f), &[Kind::Function])?;
                // the fourth number form `Integral(f, c, a, b)` reads a
                // number as the constant second function
                c.expect(gid(g), &[Kind::Function, Kind::Number, Kind::Angle])?;
                number(gid(a))?;
                number(gid(b))?;
                if let Some(e) = evaluate {
                    boolean(gid(e))?;
                }
            }
            CalculusDef::IndefiniteIntegral { function, .. }
            | CalculusDef::Derivative { function, .. } => {
                c.expect(gid(function), &[Kind::Function])?;
            }
            CalculusDef::CurveDerivative { curve, order } => {
                c.expect(gid(curve), &[Kind::Curve])?;
                if let Some(order) = order {
                    number(gid(order))?;
                }
            }
            CalculusDef::NumericInterval {
                function,
                a,
                b,
                c: end,
                ..
            } => {
                c.expect(gid(function), &[Kind::Function])?;
                number(gid(a))?;
                number(gid(b))?;
                number(gid(end))?;
            }
            CalculusDef::PolynomialPoints { points } => {
                c.expect(gid(points), &[Kind::List])?;
            }
            CalculusDef::SymbolicOde { source, point } => {
                c.expect(
                    gid(source),
                    &[Kind::Function, Kind::FunctionNVar, Kind::Curve],
                )?;
                if let Some(point) = point {
                    c.expect(gid(point), POINT)?;
                }
            }
            CalculusDef::Freehand { list } => {
                c.expect(gid(list), &[Kind::List])?;
            }
            CalculusDef::EmptyData => {}
        }
        Ok(())
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            CalculusDef::Freehand { .. } | CalculusDef::EmptyData => {
                Some(refresh(c, self).0).filter(Value::is_defined)
            }
            CalculusDef::Sum { .. } => sum_value(c, self),
            CalculusDef::DefiniteIntegral { .. } => definite_integral(c, self),
            CalculusDef::IntegralFunctions { .. } => integral_functions(c, self),
            // the record-backed functions answer through their record;
            // this path only decides whether they are defined
            CalculusDef::IndefiniteIntegral { .. }
            | CalculusDef::Derivative { .. }
            | CalculusDef::CurveDerivative { .. }
            | CalculusDef::NumericInterval { .. }
            | CalculusDef::PolynomialPoints { .. }
            | CalculusDef::SymbolicOde { .. } => {
                // the record-backed objects answer through their record;
                // this path only decides whether they are defined
                let (value, record) = refresh(c, self);
                match (&value, &record.tree) {
                    // an undefined tree: the rules failed, or the command
                    // needs the CAS
                    (&Value::Undefined, _) | (_, None) => None,
                    _ => Some(value),
                }
            }
        }
    }

    fn parse(_c: &Construction, _func: &str, _args: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}

// ----------------------------------------------------------------- the sums

/// The value of a Riemann sum: `AlgoFunctionAreaSums.compute`, with the
/// drawing arrays left out, so the loop allocates nothing.
fn sum_value(c: &Construction, d: &CalculusDef) -> Option<Value> {
    let CalculusDef::Sum {
        sum,
        function,
        a: a_id,
        b: b_id,
        n: n_id,
        divider,
    } = *d
    else {
        unreachable!("the caller matched Sum")
    };
    let (function, a_id, b_id, n_id) = (gid(function), gid(a_id), gid(b_id), gid(n_id));
    let divider = divider.map(gid);
    // every input must be defined and `RectangleSum` needs its divider
    let mut ids = vec![a_id, b_id, n_id];
    if let Some(divider) = divider {
        ids.push(divider);
    }
    if !c.value(function).is_defined() || !ids.iter().all(|&id| sum_input(c, id).is_some()) {
        return None;
    }
    let (ad, bd, ints) = (
        sum_input(c, a_id)?,
        sum_input(c, b_id)?,
        sum_input(c, n_id)?,
    );
    let dd = match divider {
        Some(d) => Some(sum_input(c, d)?),
        None => None,
    };
    let f = |x: f64| c.function_value(function, x).unwrap_or(f64::NAN);
    let mut budget = Budget::new();
    let rectangle = sum == Sum::Rectangle;
    if rectangle {
        // `RectangleSum` needs a divider in `[0, 1]`
        match dd {
            Some(d) if (0.0..=1.0).contains(&d) => {}
            _ => return None,
        }
    }
    let is_defined = function_defined(c, &mut budget, &f, ad, bd);
    if ints < 1.0 {
        return None;
    }
    let n = if ints > MAX_RECTANGLES {
        MAX_RECTANGLES as usize
    } else {
        (ints + 0.5).floor() as usize
    };
    let step = (bd - ad) / n as f64;
    let total = match sum {
        Sum::Lower | Sum::Upper => {
            let negated = sum == Sum::Upper;
            let fmin = |x: f64| if negated { -f(x) } else { f(x) };
            if step.is_infinite()
                && source_entry(c, function)
                    .or_else(|| c.calculus_record(function)?.tree.as_ref())
                    .is_some_and(|e| e.vars == 0)
            {
                // The extremum of a constant is its height even when
                // the first bracket starts at a NaN (0 * STEP).
                f(f64::NAN) * n as f64
            } else {
                lower_upper_area(c, &mut budget, &fmin, negated, (ad, bd, n, step))
            }
        }
        Sum::Left | Sum::Rectangle | Sum::Trapezoid => {
            let upper = if sum == Sum::Trapezoid { n + 1 } else { n };
            let mut total = 0.0;
            let (mut first, mut last) = (f64::NAN, f64::NAN);
            for i in 0..upper {
                if !budget.step(c, 1) {
                    return None;
                }
                // `ad + 0 * STEP` is NaN for an infinite step
                let left = ad + i as f64 * step;
                let value = if rectangle {
                    let d = dd.unwrap();
                    // the divider must not run past the right bound
                    let right_border = java_max(ad, bd);
                    f(java_min(right_border, left + d * step))
                } else {
                    f(left)
                };
                if i == 0 {
                    first = value;
                }
                last = value;
                total += value;
            }
            if sum == Sum::Trapezoid {
                // the two end heights `yval[0]` and `yval[N]` were counted
                // once too many
                total -= (first + last) / 2.0;
            }
            total
        }
    };
    // `sum.setValue(totalArea * STEP)`: an infinite total is a defined
    // infinite number, and a NaN one leaves the sum undefined
    let result = total * step;
    if budget.over() || result.is_nan() || !is_defined {
        return None;
    }
    Some(Value::Number(result))
}

/// The number an input of a Riemann sum holds. `GeoNumeric.isDefined` is
/// `!isNaN`, so a number of infinite value (`n = 1/0`) is a defined input
/// even though the kernel's object value of it is undefined; `None` when
/// the input is no number at all.
fn sum_input(c: &Construction, id: Id) -> Option<f64> {
    match c.def(id) {
        Def::Expr(d) => match expr::raw(c, d) {
            Raw::Num(v) => (!v.is_nan()).then_some(v),
            _ => None,
        },
        Def::Number(v) | Def::Angle(v) => Some(v),
        _ => match c.value(id) {
            Value::Number(v) => Some(v),
            _ => None,
        },
    }
}

/// The lower or upper sum over every rectangle: subsampling inside the
/// graphics window, then parabolic and golden-section search within the bracket
/// (`AlgoFunctionAreaSums.compute` for `LOWERSUM`/`UPPERSUM`).
fn lower_upper_area(
    c: &Construction,
    budget: &mut Budget,
    fmin: &impl Fn(f64) -> f64,
    negated: bool,
    (ad, bd, n, step): (f64, f64, usize, f64),
) -> f64 {
    // `kernel.getViewsXMin` of a construction without views: the bounds of
    // the one graphics view, and `App.countPixels` at its scale.
    let view = c.locus_view().unwrap_or(HEADLESS_VIEW);
    let visible_min = java_max(java_min(ad, bd), view[0]);
    let visible_max = java_min(java_max(ad, bd), view[1]);
    let x_zero = (0.0 - view[0]) * PIXELS_PER_UNIT;
    let count_pixels =
        |min: f64, max: f64| (x_zero + max * PIXELS_PER_UNIT) - (x_zero + min * PIXELS_PER_UNIT);
    let samples = count_pixels(visible_min, visible_max) / 5.0;
    let sub_step = (visible_max - visible_min).abs() / samples;
    let do_sub_samples = !is_zero(sub_step) && step.abs() > sub_step;
    let positive = step >= 0.0;
    let mut total = 0.0;
    for i in 0..n {
        let border = ad + i as f64 * step;
        let (mut left, mut right) = if positive {
            (border, border + step)
        } else {
            (border + step, border)
        };
        let mut min = f64::INFINITY;
        if do_sub_samples
            && (if step > 0.0 { left } else { right }) < visible_max
            && (if step > 0.0 { right } else { left }) > visible_min
        {
            let mut min_sample = left;
            let mut x = left;
            while x < right {
                if !budget.step(c, 1) {
                    return f64::NAN;
                }
                let y = fmin(x);
                if y < min {
                    min = y;
                    min_sample = x;
                }
                x += sub_step;
            }
            // the right border may hold the sample minimum: only the
            // bracket moves, `min` is left alone (GeoGebra's loop)
            let y = fmin(right);
            if y < min {
                min_sample = right;
            }
            left = java_max(left, min_sample - sub_step);
            right = java_min(right, min_sample + sub_step);
        }
        let x = if left.is_finite() && right.is_finite() {
            let Some(x) =
                expr::brent_minimum(left, right, SUM_TOLERANCE, fmin, |n| !budget.step(c, n))
            else {
                return f64::NAN;
            };
            x
        } else {
            // A bracket with a nonfinite width supplies a NaN argument.
            // Expressions such as If(x == 0, 2, 1) still have a value there.
            f64::NAN
        };
        let mut y = fmin(x);
        if y.is_nan() {
            return f64::NAN;
        }
        if y > min {
            y = min;
        }
        // store min/max: the upper sum flips its negated function back
        total += if negated { -y } else { y };
    }
    if budget.over() {
        return f64::NAN;
    }
    total
}

/// `AlgoFunctionAreaSums.functionDefined`: the function has a finite value
/// at 100 points of the interval and at its end.
fn function_defined(
    c: &Construction,
    budget: &mut Budget,
    f: &impl Fn(f64) -> f64,
    ad: f64,
    bd: f64,
) -> bool {
    let interval = (bd - ad) / CHECKPOINTS;
    let mut temp = ad;
    while (interval > 0.0 && temp < bd) || (interval < 0.0 && temp > bd) {
        if !budget.step(c, 1) {
            return false;
        }
        let val = f(temp);
        if val.is_nan() || val.is_infinite() {
            return false;
        }
        temp += interval;
    }
    let _ = f;
    let end = f(bd);
    end.is_finite()
}

/// `Math.min` of the reference: NaN when either argument is NaN, and `-0.0`
/// below `0.0`. `f64::min` returns the other argument for a NaN.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() {
            a
        } else {
            b
        }
    } else {
        a.min(b)
    }
}

/// `Math.max` of the reference: NaN when either argument is NaN, and `0.0`
/// above `-0.0`.
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() {
            a
        } else {
            b
        }
    } else {
        a.max(b)
    }
}

/// `DoubleUtil.isZero(v)`: within `Kernel.STANDARD_PRECISION`.
fn is_zero(v: f64) -> bool {
    v.abs() < STANDARD_PRECISION
}

/// The work budget of one computation: the units of `expr::work_done`.
struct Budget {
    end: u64,
}

impl Budget {
    fn new() -> Self {
        Budget {
            end: expr::work_done() + crate::expr::MAX_EVALUATION_WORK,
        }
    }

    /// Charges `n` units and tells whether the computation may continue.
    fn step(&mut self, _c: &Construction, n: u64) -> bool {
        expr::charge(n);
        !self.over()
    }

    fn over(&self) -> bool {
        expr::work_done() > self.end
    }
}

// ---------------------------------------------------------- definite integrals

/// The number a number-valued object holds.
fn number(c: &Construction, id: Id) -> f64 {
    match c.value(id) {
        Value::Number(v) => v,
        _ => f64::NAN,
    }
}

/// `AlgoIntegralDefinite.compute`.
fn definite_integral(c: &Construction, d: &CalculusDef) -> Option<Value> {
    let CalculusDef::DefiniteIntegral {
        function,
        a: a_id,
        b: b_id,
        evaluate,
        numeric,
    } = *d
    else {
        unreachable!("the caller matched DefiniteIntegral")
    };
    let (function, a_id, b_id) = (gid(function), gid(a_id), gid(b_id));
    let evaluate = evaluate.map(gid);
    if !(c.value(function).is_defined() && c.value(a_id).is_defined() && c.value(b_id).is_defined())
    {
        return None;
    }
    let lower = number(c, a_id);
    let upper = number(c, b_id);
    // `DoubleUtil.isEqual` at the standard precision
    if is_equal(lower, upper, STANDARD_PRECISION) {
        return Some(Value::Number(0.0));
    }
    let f = |x: f64| c.function_value(function, x).unwrap_or(f64::NAN);
    let (fa, fb) = (f(lower), f(upper));
    if lower.is_infinite()
        || upper.is_infinite()
        || fa.is_nan()
        || fa.is_infinite()
        || fb.is_nan()
        || fb.is_infinite()
    {
        // `computeSpecial` asks the CAS for the improper integral; without
        // it the number is undefined
        return None;
    }
    let shade_only = evaluate.is_some_and(|e| boolean(c, e) == Some(false));
    if shade_only {
        return None;
    }
    // a freehand function is summed over its own steps
    // (`freehandIntegration`)
    if let Def::Calculus(CalculusDef::Freehand { list }) = c.def(function) {
        let value = freehand_integration(c, gid(list), lower, upper);
        return value.is_finite().then_some(Value::Number(value));
    }
    let entry = source_entry(c, function)?;
    let conditional = is_conditional(entry, entry.root);
    let div_by_var = includes_division_by_var(&entry.nodes, entry.root);
    // the symbolic helper exists unless the function is conditional,
    // divides by its variable, or the command is numeric
    let helper = (evaluate.is_none() || boolean(c, evaluate.unwrap()) != Some(false))
        && !conditional
        && !div_by_var
        && !numeric;
    // the exact value of a polynomial antiderivative at the bounds; an
    // infinite value is the defined infinite number of GeoGebra, which
    // this kernel has no object value for, and a NaN one leaves the
    // number undefined (`GeoNumeric.isDefined` is `!isNaN`)
    let exact = |anti: &Parsed| -> Option<Value> {
        let value = tree_at(c, anti, upper) - tree_at(c, anti, lower);
        value.is_finite().then_some(Value::Number(value))
    };
    // the numeric path: a conditional function is split at the intervals
    // of its branches (`computeConditional`), everything else is the
    // adaptive Gauss quadrature
    let adaptive = || -> Option<Value> {
        let budget = Budget::new();
        let value = if conditional {
            conditional_integral(c, entry, lower, upper)
        } else {
            gauss_integral(&f, lower, upper)
        };
        if budget.over() || !value.is_finite() {
            return None;
        }
        Some(Value::Number(value))
    };
    if helper {
        if let Some(antiderivative) = polynomial_antiderivative(c, entry) {
            if !includes_non_continuous_integral(c, &entry.nodes, entry.root)
                && !includes_non_continuous_integral(c, &antiderivative.nodes, antiderivative.root)
            {
                let value = tree_at(c, &antiderivative, upper) - tree_at(c, &antiderivative, lower);
                if value.is_finite() {
                    return Some(Value::Number(value));
                }
                // GeoGebra keeps an infinite value as a defined number,
                // which this kernel has no object value for; only a NaN
                // one falls through to the numerical section, as
                // `AlgoIntegralDefinite.compute` does when `n` stays
                // undefined
                if !value.is_nan() {
                    return None;
                }
            } else {
                // `computeSpecial` asks the CAS for the improper integral
                // and `standardIntegral` answers without it
                return adaptive();
            }
        }
    }
    // the numerical section: the exact polynomial integral
    // (`getNumericPolynomialIntegral`) also answers `NIntegral`
    if let Some(antiderivative) = polynomial_antiderivative(c, entry) {
        return exact(&antiderivative);
    }
    adaptive()
}

/// `AlgoIntegralFunctions.compute`: the difference of two definite
/// integrals, each with its own polynomial or numeric path.
fn integral_functions(c: &Construction, d: &CalculusDef) -> Option<Value> {
    let CalculusDef::IntegralFunctions {
        f: f_id,
        g: g_id,
        a: a_id,
        b: b_id,
        evaluate,
    } = *d
    else {
        unreachable!("the caller matched IntegralFunctions")
    };
    let (f_id, g_id, a_id, b_id) = (gid(f_id), gid(g_id), gid(a_id), gid(b_id));
    let evaluate = evaluate.map(gid);
    if !(c.value(f_id).is_defined()
        && c.value(g_id).is_defined()
        && c.value(a_id).is_defined()
        && c.value(b_id).is_defined())
    {
        return None;
    }
    if evaluate.is_some_and(|e| boolean(c, e) == Some(false)) {
        return None;
    }
    let one = definite_integral(
        c,
        &CalculusDef::DefiniteIntegral {
            function: f_id.0 as u32,
            a: a_id.0 as u32,
            b: b_id.0 as u32,
            evaluate: None,
            numeric: false,
        },
    )
    .and_then(|v| match v {
        Value::Number(n) => Some(n),
        _ => None,
    })?;
    let two = match c.kind(g_id) {
        Kind::Function => definite_integral(
            c,
            &CalculusDef::DefiniteIntegral {
                function: g_id.0 as u32,
                a: a_id.0 as u32,
                b: b_id.0 as u32,
                evaluate: None,
                numeric: false,
            },
        )
        .and_then(|v| match v {
            Value::Number(n) => Some(n),
            _ => None,
        })?,
        // the four-number form reads a number as a constant function
        _ => {
            let (a, b) = (number(c, a_id), number(c, b_id));
            let value = number(c, g_id);
            if !(a.is_finite() && b.is_finite() && value.is_finite()) {
                return None;
            }
            value * (b - a)
        }
    };
    Some(Value::Number(one - two))
}

// ------------------------------------------------------------------ helpers

/// Whether the boolean object `id` is `value`.
fn boolean(c: &Construction, id: Id) -> Option<bool> {
    match c.value(id) {
        Value::Bool(v) => Some(v),
        _ => None,
    }
}

/// `DoubleUtil.isEqual(x, y, eps)`.
fn is_equal(x: f64, y: f64, eps: f64) -> bool {
    x == y || (x - eps) < y && y < (x + eps)
}

// --------------------------------------------------------- the freehand function

/// `AlgoFunctionFreehand.compute`: the function is defined for a defined
/// list of at least four numbers.
fn freehand_defined(c: &Construction, list: Id) -> bool {
    c.elements(list).is_some_and(|list| {
        list.len() >= 4
            && list
                .iter()
                .all(|item| matches!(item, Element::Value(Kind::Number, _)))
    })
}

/// The list of the freehand function `id` (`Function(list)`), which
/// `CmdFirst` reads as a `GeoLocusable`.
pub(crate) fn freehand_list(c: &Construction, id: Id) -> Option<Id> {
    match c.def(id) {
        Def::Calculus(CalculusDef::Freehand { list }) => Some(gid(list)),
        _ => None,
    }
}

/// `AlgoFunctionFreehand.getPoints`: the point of every value of the
/// list, at `min + step * i`, with the function's value there; no point
/// for fewer than four elements or a reversed interval.
pub(crate) fn freehand_points(c: &Construction, list_id: Id) -> Vec<(f64, f64)> {
    let Some(list) = c.elements(list_id) else {
        return Vec::new();
    };
    let values: Vec<f64> = (0..list.len())
        .map(|k| list.numeric_cell(k).unwrap_or(f64::NAN))
        .collect();
    freehand_points_of(&values)
}

/// [`freehand_points`] of the values of the list.
pub(crate) fn freehand_points_of(values: &[f64]) -> Vec<(f64, f64)> {
    if values.len() < 4 {
        return Vec::new();
    }
    let (min, max) = (values[0], values[1]);
    if min > max {
        return Vec::new();
    }
    let step = (max - min) / (values.len() - 3) as f64;
    (2..values.len())
        .map(|i| {
            let x = min + step * (i - 2) as f64;
            (x, freehand_value_at(values.len(), |k| values[k], x))
        })
        .collect()
}

/// `Operation.FREEHAND`: the value at `x` of the freehand function of the
/// list `{min, max, y1, ..., yn+1}`. The `n` steps divide `[min, max]`
/// evenly; outside the interval and for a reversed interval the value is
/// undefined. It reads at most four cells.
fn freehand_value(c: &Construction, list: Id, x: f64) -> f64 {
    let Some(list) = c.elements(list) else {
        return f64::NAN;
    };
    freehand_value_at(list.len(), |k| list.numeric_cell(k).unwrap_or(f64::NAN), x)
}

fn freehand_value_at(len: usize, at: impl Fn(usize) -> f64, x: f64) -> f64 {
    if len < 4 {
        return f64::NAN;
    }
    let n = len - 3;
    let (min, max) = (at(0), at(1));
    if min > max || x > max || x < min {
        return f64::NAN;
    }
    let step = (max - min) / n as f64;
    // Java's `(int) Math.floor(...)`: a NaN is 0, the rest saturates
    let index = ((x - min) / step).floor() as i32;
    if index < 0 {
        return f64::NAN;
    }
    let index = index as usize;
    if index > n - 1 {
        return at(n + 2);
    }
    let (y1, y2) = (at(index + 2), at(index + 3));
    let x1 = min + index as f64 * step;
    y1 + (x - x1) * (y2 - y1) / step
}

/// `AlgoIntegralDefinite.freehandIntegration`: the trapezoidal rule over
/// the steps of the freehand list that lie between the bounds, plus one
/// trapezium at each end. Bounds outside the list's interval are
/// undefined.
fn freehand_integration(c: &Construction, list_id: Id, lower: f64, upper: f64) -> f64 {
    let (lower, upper, sign) = if lower > upper {
        (upper, lower, -1.0)
    } else {
        (lower, upper, 1.0)
    };
    let Some(list) = c.elements(list_id) else {
        return f64::NAN;
    };
    let at = |k: i64| {
        usize::try_from(k)
            .ok()
            .and_then(|k| list.numeric_cell(k))
            .unwrap_or(f64::NAN)
    };
    let (a1, b1) = (at(0), at(1));
    if lower < a1 || upper > b1 {
        return f64::NAN;
    }
    let step = (b1 - a1) / (list.len() as f64 - 3.0);
    // Java's `(int)` of a double: a NaN is 0, the rest saturates
    let start_gap = ((lower - a1) / step).ceil() as i32 as i64;
    let end_gap = ((b1 - upper) / step).ceil() as i32 as i64;
    let start_x = a1 + step * start_gap as f64;
    let end_x = b1 - step * end_gap as f64;
    let steps = (((b1 - a1) / step - end_gap as f64 - start_gap as f64 + 0.5).floor() as i32
        as i64)
        .saturating_add(1);
    let f = |x: f64| freehand_value(c, list_id, x);
    let area = if steps > 0 {
        if steps > list.len() as i64 {
            // past the list: the reference fails on the missing element
            return f64::NAN;
        }
        let mut sum = 0.0;
        for i in 0..steps {
            let y = at(2 + i + start_gap);
            sum += if i == 0 || i == steps - 1 { y } else { 2.0 * y };
        }
        let mut area = sum * step / 2.0;
        if !is_zero(start_x - lower) {
            area += (start_x - lower) * (f(start_x) + f(lower)) / 2.0;
        }
        if !is_zero(end_x - upper) {
            area += (upper - end_x) * (f(end_x) + f(upper)) / 2.0;
        }
        area
    } else {
        (upper - lower) * (f(lower) + f(upper)) / 2.0
    };
    // `DoubleUtil.checkDecimalFraction`: a value close to a multiple of
    // `1e-5` snaps to it
    let scaled = area * 1e5;
    let rounded = (scaled + 0.5).floor();
    let area = if is_equal(scaled, rounded, STANDARD_PRECISION) {
        rounded / 1e5
    } else {
        area
    };
    area * sign
}

// ------------------------------------------------------------- the polynomials

/// The entry of the source function `f`, `None` when it is no defined
/// expression function.
fn source_entry(c: &Construction, f: Id) -> Option<&Entry> {
    match (c.def(f), c.value(f)) {
        (Def::Expr(d), Value::Function) => Some(expr::source_entry(c, d)),
        _ => None,
    }
}

/// The adaptive Gauss quadrature every numeric integral runs
/// (`AlgoIntegralDefinite.numericIntegration`, the port `elliptic` keeps).
fn gauss_integral(f: &impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let charged = |x: f64| {
        // one unit per function evaluation, so the quadrature stays inside
        // the kernel's work budget
        expr::charge(1);
        f(x)
    };
    super::elliptic::numeric_integration(&charged, a, b)
}

/// `Bounds` (GeoGebra): the interval a branch of a conditional function is
/// taken on, with the sharpness of its ends, plus the residual conditions
/// the interval alone does not capture (`condition != null` upstream).
#[derive(Clone)]
struct Bounds {
    lower: Option<f64>,
    upper: Option<f64>,
    lower_sharp: bool,
    upper_sharp: bool,
    /// The residual conditions that must hold too: subtrees of the
    /// function's tree and whether they are negated (upstream the
    /// `ExpressionNode` conjunction `condition`).
    condition: Vec<(u32, bool)>,
}

impl Bounds {
    fn new() -> Self {
        Bounds {
            lower: None,
            upper: None,
            lower_sharp: false,
            upper_sharp: false,
            condition: Vec::new(),
        }
    }

    /// `Bounds.isValid`: the interval is not empty.
    fn is_valid(&self) -> bool {
        self.lower.is_none() || self.upper.is_none() || self.lower <= self.upper
    }

    /// `Bounds.isInterval`: no residual condition.
    fn is_interval(&self) -> bool {
        self.condition.is_empty()
    }

    /// `Bounds.getLower`: an open end is negative infinity.
    fn lower_bound(&self) -> f64 {
        self.lower.unwrap_or(f64::NEG_INFINITY)
    }

    /// `Bounds.getUpper`: an open end is positive infinity.
    fn upper_bound(&self) -> f64 {
        self.upper.unwrap_or(f64::INFINITY)
    }

    /// `Bounds.addRestriction`: refine the interval with one condition,
    /// negated or not. An unnegated `AND` splits into its sides (which is
    /// also what a chain such as `1 < x < 3` is); a comparison of the
    /// variable against a number tightens the interval; everything else
    /// becomes a residual condition.
    fn add_restriction(&self, c: &Construction, e: &Entry, i: u32, negated: bool) -> Bounds {
        let node = e.nodes[i as usize];
        if !negated {
            if let Node::Bin(Bin::And, l, r) = node {
                return self
                    .add_restriction(c, e, l, false)
                    .add_restriction(c, e, r, false);
            }
        }
        // `copyInterval`: the new restriction keeps the bounds and the
        // conditions it inherits
        let mut b = self.clone();
        match node {
            Node::Bin(op, l, r) => {
                // `ExpressionNode.negation`: comparisons flip, anything
                // else is wrapped in `NOT`
                let op = if negated { negate(op) } else { Some(op) };
                match op.filter(|op| is_comparison(*op)) {
                    Some(op) => {
                        if variable_number(e, l, r) {
                            tighten(op, number_constant(c, e, r), &mut b);
                        } else if variable_number(e, r, l) {
                            tighten(reverse(op), number_constant(c, e, l), &mut b);
                        } else {
                            b.condition.push((i, negated));
                        }
                    }
                    None => b.condition.push((i, negated)),
                }
            }
            _ => b.condition.push((i, negated)),
        }
        // an interval of zero width every residual condition holds on
        // needs no condition (`If[x==1,2,If[x==2,3,4]]`)
        if let (Some(lo), Some(hi)) = (b.lower, b.upper) {
            if !b.condition.is_empty() && is_equal(hi, lo, STANDARD_PRECISION) {
                let holds = b.condition.iter().all(|&(cond, negated)| {
                    expr::subtree_bool(c, e, cond, hi).is_some_and(|(v, _)| v != negated)
                });
                if holds {
                    b.condition.clear();
                }
            }
        }
        // `x != d` with `d` outside the interval always holds
        if b.condition.len() == 1 && !b.condition[0].1 {
            if let Node::Bin(Bin::NotEqual, l, r) = e.nodes[b.condition[0].0 as usize] {
                let d = if variable_number(e, l, r) {
                    Some(number_constant(c, e, r))
                } else if variable_number(e, r, l) {
                    Some(number_constant(c, e, l))
                } else {
                    None
                };
                if b.lower.is_some_and(|lo| d.is_some_and(|d| d < lo))
                    || b.upper.is_some_and(|hi| d.is_some_and(|d| d > hi))
                {
                    b.condition.clear();
                }
            }
        }
        b
    }
}

/// The comparisons `Bounds.addRestriction` matches (`simple` upstream).
/// `NOT_EQUAL` is not one of them.
fn is_comparison(op: Bin) -> bool {
    matches!(
        op,
        Bin::Less | Bin::Greater | Bin::LessEqual | Bin::GreaterEqual | Bin::Equal
    )
}

/// `Operation.negate` of a comparison; `None` stands for the `NOT` wrap.
fn negate(op: Bin) -> Option<Bin> {
    Some(match op {
        Bin::Greater => Bin::LessEqual,
        Bin::GreaterEqual => Bin::Less,
        Bin::Less => Bin::GreaterEqual,
        Bin::LessEqual => Bin::Greater,
        Bin::Equal => Bin::NotEqual,
        Bin::NotEqual => Bin::Equal,
        _ => return None,
    })
}

/// `Operation.reverseLeftToRight`: the comparison with its sides swapped,
/// for the variable on the right.
fn reverse(op: Bin) -> Bin {
    match op {
        Bin::Less => Bin::Greater,
        Bin::Greater => Bin::Less,
        Bin::LessEqual => Bin::GreaterEqual,
        Bin::GreaterEqual => Bin::LessEqual,
        op => op,
    }
}

/// One bound step of `addRestriction` with the variable on the left:
/// `x > d` and `x >= d` tighten the lower bound, `x < d` and `x <= d` the
/// upper one, and an existing tighter bound stays. `x == d` is both.
fn tighten(op: Bin, d: f64, b: &mut Bounds) {
    match op {
        Bin::Greater if b.lower.is_none_or(|l| l <= d) => {
            b.lower = Some(d);
            b.lower_sharp = true;
        }
        Bin::GreaterEqual | Bin::Equal if b.lower.is_none_or(|l| l < d) => {
            b.lower = Some(d);
            b.lower_sharp = false;
        }
        _ => {}
    }
    match op {
        Bin::Less if b.upper.is_none_or(|u| u >= d) => {
            b.upper = Some(d);
            b.upper_sharp = true;
        }
        Bin::LessEqual | Bin::Equal if b.upper.is_none_or(|u| u > d) => {
            b.upper = Some(d);
            b.upper_sharp = false;
        }
        _ => {}
    }
}

/// The simple pattern of `addRestriction`: `left` exactly the function
/// variable, `right` a number that reads no variable.
fn variable_number(e: &Entry, left: u32, right: u32) -> bool {
    matches!(e.nodes[left as usize], Node::Var)
        && !reads_variable(e, right)
        && expr::subtree_is_number(e, right)
}

/// `ExpressionValue.wrap().containsFreeFunctionVariable(null)` of a
/// subtree: whether it reads `x` (or `y`).
fn reads_variable(e: &Entry, i: u32) -> bool {
    let mut stack = vec![i];
    while let Some(i) = stack.pop() {
        match e.nodes[i as usize] {
            Node::Var | Node::VarY => return true,
            node => stack.extend(children_of(&node)),
        }
    }
    false
}

/// `evalConstants` + `evaluateDouble`: the value of a subtree that reads
/// no variable, at the current values of its objects.
fn number_constant(c: &Construction, e: &Entry, i: u32) -> f64 {
    expr::sample_subtree(c, e, i, 0.0)
}

/// `ExpressionNode.isConditional`: an `If` over `x` (`IF`, `IF_ELSE`,
/// `IF_LIST`), which the conditional integral splits further. An `AlgoIf`
/// command, whose conditions read no `x`, is not conditional.
fn is_conditional(e: &Entry, i: u32) -> bool {
    matches!(
        e.nodes[i as usize],
        Node::If(IfMode::List | IfMode::Else, ..)
    )
}

/// `Bounds.collectCases`: the branch subtrees of a conditional function
/// and the interval each is taken on, and whether the conditions cover
/// the whole line (`complete`). A chain of `IfMode::List` nodes is the
/// pair list of one `If` call (`IF_LIST`); an `IfMode::Else` node is one
/// `IF_ELSE`. The branch after the last condition is the last case; its
/// own bounds are never read.
fn collect_cases(
    c: &Construction,
    e: &Entry,
    i: u32,
    cases: &mut Vec<u32>,
    bounds: &mut Vec<Bounds>,
    parent: &Bounds,
) -> bool {
    let Node::If(mode, cond, then, rest) = e.nodes[i as usize] else {
        // a branch the caller did not split: one case of its own
        cases.push(i);
        bounds.push(parent.clone());
        return false;
    };
    match mode {
        IfMode::List => {
            // the pairs of one `If` call; the trailing else, if there is
            // one, is the last rest and is taken over whatever the pairs
            // leave (`IF_LIST` pushes it with the parent's bounds)
            let mut current = parent.clone();
            let mut node = i;
            let mut complete = false;
            while let Node::If(IfMode::List, ci, ti, ri) = e.nodes[node as usize] {
                bounds.push(current.add_restriction(c, e, ci, false));
                cases.push(ti);
                current = current.add_restriction(c, e, ci, true);
                if ri == NONE || !matches!(e.nodes[ri as usize], Node::If(IfMode::List, ..)) {
                    if ri != NONE {
                        cases.push(ri);
                        bounds.push(parent.clone());
                        complete = true;
                    }
                    break;
                }
                node = ri;
            }
            complete
        }
        IfMode::Else => {
            // `IF_ELSE`: the condition's branch and the else branch, each
            // itself split when conditional
            let mut complete = rest != NONE;
            let positive = parent.add_restriction(c, e, cond, false);
            let negative = if positive.is_valid() {
                parent.add_restriction(c, e, cond, true)
            } else {
                parent.clone()
            };
            if is_conditional(e, then) {
                complete &= collect_cases(c, e, then, cases, bounds, &positive);
            } else {
                cases.push(then);
                bounds.push(positive);
            }
            if rest != NONE {
                if is_conditional(e, rest) {
                    complete &= collect_cases(c, e, rest, cases, bounds, &negative);
                } else {
                    cases.push(rest);
                    bounds.push(negative);
                }
            }
            complete
        }
        // an `AlgoIf` command: one case of its own, never split
        IfMode::Algo | IfMode::AlgoNext => {
            cases.push(i);
            bounds.push(parent.clone());
            false
        }
    }
}

/// `AlgoIntegralDefinite.computeConditional`: the integral of a
/// conditional function, split at the intervals its branches are taken
/// on. Every piece is the adaptive Gauss quadrature of one branch; the
/// branch after the last condition covers what the others leave at the
/// ends of the integration interval.
fn conditional_integral(c: &Construction, e: &Entry, lower: f64, upper: f64) -> f64 {
    let (mut lo, mut hi, mut sign) = (lower, upper, 1.0);
    if upper < lower {
        lo = upper;
        hi = lower;
        sign = -1.0;
    }
    let mut cases = Vec::new();
    let mut bounds = Vec::new();
    let complete = collect_cases(c, e, e.root, &mut cases, &mut bounds, &Bounds::new());
    let size = if complete {
        cases.len() - 1
    } else {
        cases.len()
    };
    let integrate =
        |node: u32, a: f64, b: f64| gauss_integral(&|x| expr::sample_subtree(c, e, node, x), a, b);
    let whole = |x| expr::sample_subtree(c, e, e.root, x);
    let mut sum = 0.0;
    let mut covered_min = f64::NAN;
    let mut covered_max = f64::NAN;
    for i in 0..size {
        let bound = &bounds[i];
        if !bound.is_interval() {
            // a condition the interval cannot express
            return gauss_integral(&whole, lower, upper);
        }
        let (low, up) = (bound.lower_bound(), bound.upper_bound());
        if i == 0 {
            covered_min = low;
            covered_max = up;
        } else if low == covered_max {
            covered_max = up;
        } else if up == covered_min {
            covered_min = low;
        } else {
            // regions defined in a strange order
            return gauss_integral(&whole, lower, upper);
        }
        let node = cases[i];
        if low.is_infinite() {
            if up > lo {
                sum += sign * integrate(node, lo, up.min(hi));
            }
        } else if up.is_infinite() {
            if low < hi {
                sum += sign * integrate(node, low.max(lo), hi);
            }
        } else if up <= lo || low >= hi {
            // outside the integration interval
        } else if low >= lo && up <= hi {
            sum += sign * integrate(node, low, up);
        } else if (low.is_nan() || low <= lo) && up <= hi {
            sum += sign * integrate(node, lo, up);
        } else if (up.is_nan() || up >= hi) && low >= lo {
            sum += sign * integrate(node, low, hi);
        } else if low <= lo && up >= hi {
            sum += sign * integrate(node, lo, hi);
        }
    }
    if complete {
        // the branch after the last condition covers the ends the other
        // cases leave open
        let node = cases[size];
        if hi <= covered_min || lo >= covered_max {
            sum += sign * integrate(node, lo, hi);
        } else if lo >= covered_min && hi <= covered_max {
            // everything is covered already
        } else if lo <= covered_min && hi <= covered_max {
            sum += sign * integrate(node, lo, covered_min);
        } else if lo >= covered_min && hi >= covered_max {
            sum += sign * integrate(node, covered_max, hi);
        }
    }
    sum
}

/// The value of a built tree at `x`: `GeoFunction.value` of the
/// antiderivative at one of the bounds.
fn tree_at(c: &Construction, parsed: &Parsed, x: f64) -> f64 {
    expr::sample_parsed(c, parsed, x)
}

/// `ExpressionNode.includesDivisionBy`: a division whose right side reads
/// the function variable, or a negative power of something that reads it.
fn includes_division_by_var(nodes: &[Node], root: u32) -> bool {
    fn contains_var(nodes: &[Node], i: u32) -> bool {
        match nodes[i as usize] {
            Node::Var | Node::VarY => true,
            node => children_of(&node).any(|child| contains_var(nodes, child)),
        }
    }
    fn constant(nodes: &[Node], i: u32) -> Option<f64> {
        // `right.evaluateDouble()`: the exponent's value
        match nodes[i as usize] {
            Node::Num(v) => Some(v),
            Node::MinusOne => Some(-1.0),
            _ => None,
        }
    }
    let mut stack = vec![root];
    while let Some(i) = stack.pop() {
        match nodes[i as usize] {
            Node::Bin(Bin::Pow, left, right) => {
                if contains_var(nodes, left) && constant(nodes, right).is_some_and(|v| v < 0.0) {
                    return true;
                }
                stack.extend([left, right]);
            }
            Node::Bin(Bin::Div, left, right) => {
                if contains_var(nodes, right) {
                    return true;
                }
                stack.push(left);
            }
            node => stack.extend(children_of(&node)),
        }
    }
    false
}

/// The children of a node (the `children` of `expr`, which is private
/// there).
fn children_of(node: &Node) -> impl Iterator<Item = u32> {
    crate::expr::children(node)
}

/// `ExpressionNode.includesNonContinuousIntegral`: an operation the
/// symbolic integral of a discontinuous function would silently move
/// across a jump of. The tree of an applied function counts too
/// (`Operation.FUNCTION` reads the function's own expression).
fn includes_non_continuous_integral(c: &Construction, nodes: &[Node], root: u32) -> bool {
    fn non_continuous(op: Un) -> bool {
        matches!(
            op,
            Un::Abs
                | Un::Sgn
                | Un::Floor
                | Un::Ceil
                | Un::Round
                | Un::Tan
                | Un::Cot
                | Un::Sec
                | Un::Csc
                | Un::Gamma
        )
    }
    let mut stack = vec![root];
    while let Some(i) = stack.pop() {
        match nodes[i as usize] {
            Node::Un(op, _) if non_continuous(op) => return true,
            Node::If(..) => return true,
            Node::Apply(f, arg) => {
                if let Some(entry) = source_entry(c, f) {
                    if includes_non_continuous_integral(c, &entry.nodes, entry.root) {
                        return true;
                    }
                }
                stack.push(arg);
            }
            node => stack.extend(children_of(&node)),
        }
    }
    false
}

/// The antiderivative tree of a polynomial source
/// (`getNumericPolynomialIntegral`): each coefficient divided by its new
/// power, built like `SymbolicPolyFunction.getFunction`.
fn polynomial_antiderivative(c: &Construction, entry: &Entry) -> Option<Parsed> {
    let mut b = Builder::function(entry);
    let poly = b.expand(c, entry.root).ok()?;
    let roots = poly.coefficient_roots(&mut b);
    // buildIntegral: the coefficient of x^k becomes that of x^(k+1), the
    // constant term is zero
    let mut shifted: Vec<Option<u32>> = vec![None];
    for (k, root) in roots.into_iter().enumerate() {
        let shifted_root = root.map(|root| {
            let divisor = b.push(Node::Num((k + 1) as f64));
            b.divide(root, divisor)
        });
        shifted.push(shifted_root);
    }
    poly_function_tree(c, &mut b, &shifted)
}

/// The solution of `y' = p(x)` for a polynomial source `p`: the
/// antiderivative with the constant zero (the initial value of the
/// arbitrary constant the CAS answer carries), or with the constant that
/// puts the graph through the point `through`. `None` for a source that
/// is no polynomial in `x` or has a coefficient that is not finite.
fn polynomial_ode_solution(
    c: &Construction,
    entry: &Entry,
    through: Option<(f64, f64)>,
) -> Option<Parsed> {
    let mut b = Builder::function(entry);
    let poly = b.expand(c, entry.root).ok()?;
    let roots = poly.coefficient_roots(&mut b);
    let mut shifted: Vec<Option<u32>> = vec![None];
    for (k, root) in roots.into_iter().enumerate() {
        shifted.push(root.map(|root| {
            let divisor = b.push(Node::Num((k + 1) as f64));
            b.divide(root, divisor)
        }));
    }
    // the coefficients of the antiderivative, the constant term left out
    let mut values = Vec::with_capacity(shifted.len());
    let mut evaluated = b
        .number_values(c, shifted.iter().copied().flatten())
        .into_iter();
    for root in &shifted {
        let value = match root {
            Some(_) => evaluated.next().flatten()?,
            None => 0.0,
        };
        if !value.is_finite() {
            return None;
        }
        values.push(value);
    }
    if let Some((x, y)) = through {
        let at_x = values.iter().rev().fold(0.0, |sum, &v| sum * x + v);
        let constant = y - at_x;
        if !constant.is_finite() {
            return None;
        }
        shifted[0] = Some(b.push(Node::Num(constant)));
    }
    poly_function_tree(c, &mut b, &shifted)
}

/// The n-th derivative tree of a polynomial source (`getDerivative` of a
/// `PolyFunction`, n times): each differentiation multiplies every
/// coefficient by its power and drops the constant.
fn polynomial_derivative(c: &Construction, entry: &Entry, order: u32) -> Option<Parsed> {
    let mut b = Builder::function(entry);
    let poly = b.expand(c, entry.root).ok()?;
    let mut roots = poly.coefficient_roots(&mut b);
    for _ in 0..order {
        if roots.len() < 2 {
            roots = vec![None];
            break;
        }
        roots.remove(0);
        for (k, root) in roots.iter_mut().enumerate() {
            *root = root.map(|root| {
                let factor = b.push(Node::Num((k + 1) as f64));
                b.multiply(root, factor)
            });
        }
    }
    poly_function_tree(c, &mut b, &roots)
}

/// `PolyFunction.getFunction`: the expanded polynomial as one expression
/// tree, `x^d cd + x^(d-1) cd-1 + ... + c1 x + c0`, the zero coefficients
/// dropped (`coeffs[i] != 0` for the middle powers, `DoubleUtil.isZero`
/// for the linear and constant ones). A coefficient root of `None` is the
/// zero coefficient.
///
/// The terms keep the coefficients as their evaluated numbers, not their
/// trees: the polynomial powers share coefficient subtrees, and a result
/// tree that kept them would evaluate each shared subtree once per
/// reference, unbounded work for a power like `(a x + b)^300` on every
/// evaluation of the result. The values are recomputed whenever the source
/// function's own objects change, which the definition's parents keep, so
/// only the symbolic form of the definition differs from upstream's.
fn poly_function_tree(c: &Construction, b: &mut Builder, roots: &[Option<u32>]) -> Option<Parsed> {
    // the values of the coefficients decide the drops, evaluated like
    // GeoGebra's `coeffs` array is
    let mut values = Vec::with_capacity(roots.len());
    let mut evaluated = b
        .number_values(c, roots.iter().copied().flatten())
        .into_iter();
    for root in roots {
        values.push(match root {
            Some(_) => evaluated.next().flatten()?,
            None => 0.0,
        });
    }
    let mut valid = values.len().min(1);
    for (i, &v) in values.iter().enumerate() {
        if v != 0.0 {
            valid = i + 1;
        }
    }
    let degree = valid - 1;
    let var = |b: &mut Builder| b.push(Node::Var);
    let term = |b: &mut Builder, k: usize, coeff: f64| -> u32 {
        let c = b.push(Node::Num(coeff));
        match k {
            0 => c,
            1 => {
                let v = var(b);
                b.multiply(v, c)
            }
            _ => {
                let v = var(b);
                let power = b.push(Node::Num(k as f64));
                let xk = b.push(Node::Bin(Bin::Pow, v, power));
                b.multiply(xk, c)
            }
        }
    };
    let mut root = match roots[degree] {
        // the leading coefficient: the zero polynomial is the constant 0
        Some(_) => term(b, degree, values[degree]),
        None => b.push(Node::Num(0.0)),
    };
    for k in (2..degree).rev() {
        if values[k] != 0.0 {
            let t = term(b, k, values[k]);
            root = b.push(Node::Bin(Bin::Add, root, t));
        }
    }
    if degree > 1 && !is_zero(values[1]) {
        let t = term(b, 1, values[1]);
        root = b.push(Node::Bin(Bin::Add, root, t));
    }
    if degree > 0 && !is_zero(values[0]) {
        let t = term(b, 0, values[0]);
        root = b.push(Node::Bin(Bin::Add, root, t));
    }
    b.finish(c, root).ok()
}

// ------------------------------------------------------ the no-CAS integral rules

/// `ExpressionNode.integral`: the antiderivative the rule integrator builds
/// for a function that has no polynomial form. A rule it cannot integrate
/// leaves a `NaN` where GeoGebra's `wrap(Double.NaN)` does, which makes the
/// result undefined as a whole (a `NaN` at the root) or undefined-valued on
/// part of the domain (a `NaN` inside).
fn rule_integral(c: &Construction, entry: &Entry) -> Option<Parsed> {
    let mut b = Builder::function(entry);
    let root = integrate_node(c, &mut b, entry.root, 0);
    b.finish(c, root).ok()
}

/// One step of the integrator for the subtree at `i`, producing a node in
/// the builder's arena. `depth` bounds the recursion (the tree is finite,
/// but a cyclic definition could otherwise loop).
fn integrate_node(c: &Construction, b: &mut Builder, i: u32, depth: u32) -> u32 {
    if depth > crate::expr::MAX_DEPTH {
        return b.push(Node::Num(f64::NAN));
    }
    expr::charge(1);
    match b.node(i) {
        // FunctionVariable.integral: x^2 / 2
        Node::Var => {
            let two = b.push(Node::Num(2.0));
            let power = b.push(Node::Bin(Bin::Pow, i, two));
            b.push(Node::Bin(Bin::Div, power, two))
        }
        // MyDouble.integral: the constant times x
        Node::Num(v) => {
            let _ = v;
            let var = b.push(Node::Var);
            b.push(Node::Bin(Bin::Mul, i, var))
        }
        Node::If(mode, cond, then, rest) => {
            // IF and IF_ELSE keep the condition and integrate the payload
            let then = integrate_node(c, b, then, depth + 1);
            let rest = if rest == u32::MAX {
                u32::MAX
            } else {
                integrate_node(c, b, rest, depth + 1)
            };
            b.push(Node::If(mode, cond, then, rest))
        }
        Node::Un(op, arg) => integrate_unary(c, b, op, arg, i, depth),
        Node::Bin(op, a, d) => integrate_binary(c, b, op, a, d, i, depth),
        // every other leaf behaves like a constant: this times x
        _ => {
            let var = b.push(Node::Var);
            b.push(Node::Bin(Bin::Mul, i, var))
        }
    }
}

/// `linearIntegral`: `op(arg)` scaled by `i`, divided (or, for the divide
/// coefficient, multiplied) by the linear coefficient of `arg`.
fn linear_integral(c: &Construction, b: &mut Builder, arg: u32, i: f64, op: Un) -> u32 {
    if b.node(arg) == Node::Var {
        let f = b.push(Node::Un(op, arg));
        let k = b.push(Node::Num(i));
        return b.push(Node::Bin(Bin::Mul, f, k));
    }
    if let Some(coeff) = linear_coefficient(c, b, arg) {
        let f = b.push(Node::Un(op, arg));
        let k = b.push(Node::Num(i));
        let scaled = b.push(Node::Bin(Bin::Mul, f, k));
        let d = b.push(Node::Num(coeff));
        return b.push(Node::Bin(Bin::Div, scaled, d));
    }
    if let Some(coeff) = linear_coefficient_div(c, b, arg) {
        let f = b.push(Node::Un(op, arg));
        let d = b.push(Node::Num(coeff));
        let scaled = b.push(Node::Bin(Bin::Mul, f, d));
        let k = b.push(Node::Num(i));
        return b.push(Node::Bin(Bin::Mul, scaled, k));
    }
    b.push(Node::Num(f64::NAN))
}

/// The `ln |...|` rules of `tan`, `sec`, `csc`, `cot`, `tanh`, `csch` and
/// `coth`: `inner` builds the logarithm's argument, the result is divided
/// (or multiplied, for the divide coefficient) by the coefficient scaled
/// with `sign`.
fn ln_rule(
    c: &Construction,
    b: &mut Builder,
    arg: u32,
    sign: f64,
    inner: impl Fn(&mut Builder, u32) -> u32,
) -> u32 {
    if let Some(coeff) = linear_coefficient(c, b, arg) {
        let tree = inner(b, arg);
        let d = b.push(Node::Num(sign * coeff));
        return b.push(Node::Bin(Bin::Div, tree, d));
    }
    if let Some(coeff) = linear_coefficient_div(c, b, arg) {
        let tree = inner(b, arg);
        let m = b.push(Node::Num(sign * coeff));
        return b.push(Node::Bin(Bin::Mul, tree, m));
    }
    b.push(Node::Num(f64::NAN))
}

/// `(u ln u - u)` of the argument at `i` (the body of the `ln`/`log` rules).
fn log_body(b: &mut Builder, i: u32) -> u32 {
    let ln = b.push(Node::Un(Un::Ln, i));
    let product = b.push(Node::Bin(Bin::Mul, ln, i));
    b.push(Node::Bin(Bin::Sub, product, i))
}

/// `ExpressionNode.integral` of a unary operation. `i` is the node itself,
/// for the rules that reuse it.
fn integrate_unary(c: &Construction, b: &mut Builder, op: Un, arg: u32, i: u32, depth: u32) -> u32 {
    let _ = depth;
    match op {
        Un::Sin => linear_integral(c, b, arg, -1.0, Un::Cos),
        Un::Cos => linear_integral(c, b, arg, 1.0, Un::Sin),
        Un::Exp => linear_integral(c, b, arg, 1.0, Un::Exp),
        Un::Sinh => linear_integral(c, b, arg, 1.0, Un::Cosh),
        Un::Cosh => linear_integral(c, b, arg, 1.0, Un::Sinh),
        Un::Tan => ln_rule(c, b, arg, 1.0, |b, arg| {
            let sec = b.push(Node::Un(Un::Sec, arg));
            b.push(Node::Un(Un::Abs, sec))
        }),
        Un::Sec => ln_rule(c, b, arg, 1.0, |b, arg| {
            let sec = b.push(Node::Un(Un::Sec, arg));
            let tan = b.push(Node::Un(Un::Tan, arg));
            let sum = b.push(Node::Bin(Bin::Add, sec, tan));
            b.push(Node::Un(Un::Abs, sum))
        }),
        Un::Csc => ln_rule(c, b, arg, -1.0, |b, arg| {
            let csc = b.push(Node::Un(Un::Csc, arg));
            let cot = b.push(Node::Un(Un::Cot, arg));
            let sum = b.push(Node::Bin(Bin::Add, csc, cot));
            b.push(Node::Un(Un::Abs, sum))
        }),
        Un::Cot => ln_rule(c, b, arg, 1.0, |b, arg| {
            let sin = b.push(Node::Un(Un::Sin, arg));
            b.push(Node::Un(Un::Abs, sin))
        }),
        Un::Tanh => ln_rule(c, b, arg, 1.0, |b, arg| {
            let cosh = b.push(Node::Un(Un::Cosh, arg));
            b.push(Node::Un(Un::Abs, cosh))
        }),
        Un::Csch => ln_rule(c, b, arg, -1.0, |b, arg| {
            let csch = b.push(Node::Un(Un::Csch, arg));
            let coth = b.push(Node::Un(Un::Coth, arg));
            let sum = b.push(Node::Bin(Bin::Add, csch, coth));
            b.push(Node::Un(Un::Abs, sum))
        }),
        Un::Coth => ln_rule(c, b, arg, 1.0, |b, arg| {
            let sinh = b.push(Node::Un(Un::Sinh, arg));
            b.push(Node::Un(Un::Abs, sinh))
        }),
        Un::Sech => {
            if let Some(coeff) = linear_coefficient(c, b, arg) {
                let exp = b.push(Node::Un(Un::Exp, arg));
                let atan = b.push(Node::Un(Un::Atan, exp));
                let d = b.push(Node::Num(coeff / 2.0));
                b.push(Node::Bin(Bin::Div, atan, d))
            } else if let Some(coeff) = linear_coefficient_div(c, b, arg) {
                let exp = b.push(Node::Un(Un::Exp, arg));
                let atan = b.push(Node::Un(Un::Atan, exp));
                let m = b.push(Node::Num(2.0 * coeff));
                b.push(Node::Bin(Bin::Mul, atan, m))
            } else {
                b.push(Node::Num(f64::NAN))
            }
        }
        Un::Ln | Un::Log10 | Un::Log2 => {
            let base_ln = match op {
                Un::Ln => 1.0,
                Un::Log10 => 10.0_f64.ln(),
                _ => 2.0_f64.ln(),
            };
            if let Some(coeff) = linear_coefficient(c, b, arg) {
                let body = log_body(b, arg);
                let d = b.push(Node::Num(base_ln * coeff));
                b.push(Node::Bin(Bin::Div, body, d))
            } else if let Some(coeff) = linear_coefficient_div(c, b, arg) {
                let body = log_body(b, arg);
                let m = b.push(Node::Num(coeff));
                let scaled = b.push(Node::Bin(Bin::Mul, body, m));
                if op == Un::Ln {
                    scaled
                } else {
                    let d = b.push(Node::Num(base_ln));
                    b.push(Node::Bin(Bin::Div, scaled, d))
                }
            } else {
                b.push(Node::Num(f64::NAN))
            }
        }
        Un::Sqrt => root_rule(c, b, arg, Un::Sqrt, 3.0, 2.0),
        Un::Cbrt => root_rule(c, b, arg, Un::Cbrt, 4.0, 3.0),
        // GeoGebra resolves Length of a point or vector to a number object.
        // Like XCOORD and YCOORD, a constant length integrates to this times x.
        Un::Length if b.contains_var(arg) => b.push(Node::Num(f64::NAN)),
        Un::X | Un::Y | Un::Length => {
            let var = b.push(Node::Var);
            b.push(Node::Bin(Bin::Mul, i, var))
        }
        _ => b.push(Node::Num(f64::NAN)),
    }
}

/// The `sqrt`/`cbrt` rules: `root(u) u` divided by `coeff n/d` (or
/// multiplied by `coeff d/n` for the divide coefficient).
fn root_rule(c: &Construction, b: &mut Builder, arg: u32, root: Un, n: f64, d: f64) -> u32 {
    if let Some(coeff) = linear_coefficient(c, b, arg) {
        let r = b.push(Node::Un(root, arg));
        let product = b.push(Node::Bin(Bin::Mul, r, arg));
        let divisor = b.push(Node::Num(coeff * n / d));
        b.push(Node::Bin(Bin::Div, product, divisor))
    } else if let Some(coeff) = linear_coefficient_div(c, b, arg) {
        let r = b.push(Node::Un(root, arg));
        let product = b.push(Node::Bin(Bin::Mul, r, arg));
        let factor = b.push(Node::Num(coeff * d / n));
        b.push(Node::Bin(Bin::Mul, product, factor))
    } else {
        b.push(Node::Num(f64::NAN))
    }
}

/// `ExpressionNode.integral` of a binary operation. `i` is the node itself,
/// for the rules that reuse it.
fn integrate_binary(
    c: &Construction,
    b: &mut Builder,
    op: Bin,
    a: u32,
    d: u32,
    i: u32,
    depth: u32,
) -> u32 {
    let nan = |b: &mut Builder| b.push(Node::Num(f64::NAN));
    match op {
        Bin::Add | Bin::Sub => {
            let left = integrate_node(c, b, a, depth + 1);
            let right = integrate_node(c, b, d, depth + 1);
            b.push(Node::Bin(op, left, right))
        }
        Bin::Mul => {
            // a constant factor comes out, on the right of the product
            if !b.contains_var(d) && b.number_value(c, d).is_some() {
                let left = integrate_node(c, b, a, depth + 1);
                b.push(Node::Bin(Bin::Mul, left, d))
            } else if !b.contains_var(a) && b.number_value(c, a).is_some() {
                let right = integrate_node(c, b, d, depth + 1);
                b.push(Node::Bin(Bin::Mul, right, a))
            } else {
                nan(b)
            }
        }
        Bin::Div => {
            if !b.contains_var(d) && b.number_value(c, d).is_some() {
                let left = integrate_node(c, b, a, depth + 1);
                b.push(Node::Bin(Bin::Div, left, d))
            } else if !b.contains_var(a) && b.number_value(c, a).is_some() && b.node(d) == Node::Var
            {
                // 4/x integrates to ln(x) times the constant
                let ln = b.push(Node::Un(Un::Ln, d));
                b.push(Node::Bin(Bin::Mul, ln, a))
            } else {
                nan(b)
            }
        }
        Bin::RootPow => {
            let Node::Bin(Bin::Div, _, degree) = b.node(d) else {
                return nan(b);
            };
            let Some(n) = b.number_value(c, degree) else {
                return nan(b);
            };
            let divisor = if let Some(coeff) = linear_coefficient(c, b, a) {
                (n + 1.0) * coeff
            } else if let Some(coeff) = linear_coefficient_div(c, b, a) {
                (n + 1.0) / coeff
            } else {
                return nan(b);
            };
            // ExpressionNode.integral: u * nroot(u, n) * n / ((n + 1) * u').
            let product = b.push(Node::Bin(Bin::Mul, i, a));
            let scaled = b.push(Node::Bin(Bin::Mul, product, degree));
            let divisor = b.push(Node::Num(divisor));
            b.push(Node::Bin(Bin::Div, scaled, divisor))
        }
        Bin::Pow => {
            // x^c
            if b.node(a) == Node::Var && !b.contains_var(d) {
                return match b.number_value(c, d).filter(|v| v.is_finite()) {
                    Some(index) => {
                        if is_zero(index + 1.0) {
                            // x^-1 integrates to ln(x)
                            b.push(Node::Un(Un::Ln, a))
                        } else {
                            let e = b.push(Node::Num(index + 1.0));
                            let power = b.push(Node::Bin(Bin::Pow, a, e));
                            let divisor = b.push(Node::Num(index + 1.0));
                            b.push(Node::Bin(Bin::Div, power, divisor))
                        }
                    }
                    None => nan(b),
                };
            }
            // c^x (the chain stops here: any other exponent is unhandled)
            if !b.contains_var(a) {
                if b.node(d) == Node::Var {
                    return match b.number_value(c, a).filter(|v| v.is_finite()) {
                        Some(base) => {
                            if is_equal(base, 1.0, crate::tol::STANDARD_PRECISION) {
                                b.push(Node::Var)
                            } else if base > 0.0 {
                                // (c^x) / ln(c)
                                let ln = b.push(Node::Num(base.ln()));
                                b.push(Node::Bin(Bin::Div, i, ln))
                            } else {
                                nan(b)
                            }
                        }
                        None => nan(b),
                    };
                }
                return nan(b);
            }
            // u^c with u linear in x
            if !b.contains_var(d) {
                if let Some(index) = b.number_value(c, d).filter(|v| v.is_finite()) {
                    if let Some(coeff) = linear_coefficient(c, b, a) {
                        if is_equal(index, -1.0, crate::tol::STANDARD_PRECISION) {
                            // u^-1 integrates to ln|u| / coeff
                            let abs = b.push(Node::Un(Un::Abs, a));
                            let ln = b.push(Node::Un(Un::Ln, abs));
                            let divisor = b.push(Node::Num(coeff));
                            return b.push(Node::Bin(Bin::Div, ln, divisor));
                        }
                        let e = b.push(Node::Num(index + 1.0));
                        let power = b.push(Node::Bin(Bin::Pow, a, e));
                        let divisor = b.push(Node::Num(coeff * (index + 1.0)));
                        return b.push(Node::Bin(Bin::Div, power, divisor));
                    }
                    if let Some(coeff) = linear_coefficient_div(c, b, a) {
                        if is_equal(index, -1.0, crate::tol::STANDARD_PRECISION) {
                            let abs = b.push(Node::Un(Un::Abs, a));
                            let ln = b.push(Node::Un(Un::Ln, abs));
                            let factor = b.push(Node::Num(coeff));
                            return b.push(Node::Bin(Bin::Mul, ln, factor));
                        }
                        let e = b.push(Node::Num(index + 1.0));
                        let power = b.push(Node::Bin(Bin::Pow, a, e));
                        let factor = b.push(Node::Num(coeff / (index + 1.0)));
                        return b.push(Node::Bin(Bin::Mul, power, factor));
                    }
                }
            }
            nan(b)
        }
        _ => nan(b),
    }
}

/// `getLinearCoefficient`: the coefficient of the variable in `3x + 1` and
/// its variants, `None` where GeoGebra returns `NaN` (not linear). The value
/// can still be infinite, exactly like the Java.
fn linear_coefficient(c: &Construction, b: &Builder, i: u32) -> Option<f64> {
    if b.node(i) == Node::Var {
        return Some(1.0);
    }
    let mut ev = i;
    let mut factor = 1.0;
    // strip a "+ 1" (or "- 1") constant off a sum
    if let Node::Bin(op @ (Bin::Add | Bin::Sub), l, r) = b.node(i) {
        if !b.contains_var(l) && b.number_value(c, l).is_some() {
            ev = r;
            factor = if matches!(op, Bin::Sub) { -1.0 } else { 1.0 };
        } else if !b.contains_var(r) && b.number_value(c, r).is_some() {
            ev = l;
        }
    }
    if b.node(ev) == Node::Var {
        return Some(factor);
    }
    if let Node::Bin(Bin::Mul, l, r) = b.node(ev) {
        if b.node(l) == Node::Var && !b.contains_var(r) {
            if let Some(v) = b.number_value(c, r) {
                return Some(v * factor);
            }
        }
        if b.node(r) == Node::Var && !b.contains_var(l) {
            if let Some(v) = b.number_value(c, l) {
                return Some(v * factor);
            }
        }
    }
    None
}

/// `getLinearCoefficientDiv`: the coefficient of the variable in `x / 3 + 1`
/// and its variants, `None` where GeoGebra returns `NaN`. Note the inverted
/// sign convention of the stripped constant, kept from the Java.
fn linear_coefficient_div(c: &Construction, b: &Builder, i: u32) -> Option<f64> {
    let mut ev = i;
    let mut factor = 1.0;
    if let Node::Bin(op @ (Bin::Add | Bin::Sub), l, r) = b.node(i) {
        if !b.contains_var(l) && b.number_value(c, l).is_some() {
            ev = r;
            factor = if matches!(op, Bin::Add) { 1.0 } else { -1.0 };
        } else if !b.contains_var(r) && b.number_value(c, r).is_some() {
            ev = l;
        }
    }
    if let Node::Bin(Bin::Div, l, r) = b.node(ev) {
        if b.node(l) == Node::Var && !b.contains_var(r) {
            if let Some(v) = b.number_value(c, r) {
                return Some(v * factor);
            }
        }
    }
    None
}

// ------------------------------------------------ the no-CAS derivative rules

/// `Function.getDerivativeNoCAS`: the derivative tree that the rules of
/// `Derivative.get` build, one `ExpressionNode.derivative` pass per order.
fn rule_derivative(c: &Construction, entry: &Entry, order: u32) -> Option<Parsed> {
    let budget = Budget::new();
    let mut parsed: Option<Parsed> = None;
    for _ in 0..order {
        let pass;
        let source = match &parsed {
            Some(p) => {
                pass = expr::function_entry(c, p.clone());
                &pass
            }
            None => entry,
        };
        // one pass walks the whole previous tree and roughly triples it;
        // charging that keeps a huge order (the rules build a new tree per
        // pass) inside the work budget, and past it the derivative is
        // undefined
        expr::charge(8 * source.nodes.len() as u64);
        if budget.over() {
            return None;
        }
        let mut b = Builder::function(source);
        // one pass stops at the budget while it builds, not only before:
        // a product rule roughly triples the tree per pass, and the last
        // pass would otherwise build millions of nodes first
        b.abort_at = budget.end;
        let root = differentiate_node(c, &mut b, source.root, 0);
        if stop_derivative(&mut b, 0) {
            return None;
        }
        parsed = Some(b.finish(c, root).ok()?);
    }
    parsed
}

/// One step of `Derivative.get` for the subtree at `i`, a node of the
/// derivative in the builder's arena.
fn differentiate_node(c: &Construction, b: &mut Builder, i: u32, depth: u32) -> u32 {
    if stop_derivative(b, depth) {
        return i;
    }
    expr::charge(1);
    match b.node(i) {
        // FunctionVariable.derivative: the variable itself derives to 1,
        // every other leaf (MyDouble, an object) to 0
        Node::Var => b.push(Node::Num(1.0)),
        Node::If(IfMode::List, ..) => {
            // An `IF_LIST` is one node holding its lists of conditions and
            // payloads, so its pieces are siblings, not a nesting: walk the
            // chain at one depth and charge each piece.
            let mut pieces = Vec::new();
            let mut node = i;
            while let Node::If(IfMode::List, cond, then, rest) = b.node(node) {
                expr::charge(1);
                pieces.push((cond, differentiate_node(c, b, then, depth + 1)));
                if b.exhausted || rest == u32::MAX {
                    node = u32::MAX;
                    break;
                }
                node = rest;
            }
            let mut root = if node == u32::MAX {
                u32::MAX
            } else {
                differentiate_node(c, b, node, depth + 1)
            };
            for (cond, then) in pieces.into_iter().rev() {
                root = b.push(Node::If(IfMode::List, cond, then, root));
            }
            root
        }
        Node::If(mode, cond, then, rest) => {
            // IF and IF_ELSE keep the condition and differentiate the
            // payload
            let then = differentiate_node(c, b, then, depth + 1);
            let rest = if rest == u32::MAX {
                u32::MAX
            } else {
                differentiate_node(c, b, rest, depth + 1)
            };
            b.push(Node::If(mode, cond, then, rest))
        }
        Node::Un(op, arg) => differentiate_unary(c, b, op, arg, depth),
        Node::Bin(op, a, d) => differentiate_binary(c, b, op, a, d, depth),
        Node::Apply(f, arg) => {
            // `Operation.FUNCTION`: the derivative of the applied
            // function, at the argument, times the argument's derivative.
            // GeoGebra materialises the derivative as its own function
            // object; here its tree is cloned into this one, with the
            // argument in place of the function variable.
            let Some(entry) = source_entry(c, f) else {
                return b.push(Node::Num(f64::NAN));
            };
            expr::charge(entry.nodes.len() as u64);
            if entry.nodes.len() > MAX_RULE_NODES || stop_derivative(b, depth) {
                b.exhausted = true;
                return i;
            }
            let mut fb = Builder::function(entry);
            fb.abort_at = b.abort_at;
            let df = differentiate_node(c, &mut fb, entry.root, depth + 1);
            if fb.exhausted {
                b.exhausted = true;
                return i;
            }
            let cloned = clone_with_subst(b, &fb, df, Some(arg), depth + 1);
            let d = differentiate_node(c, b, arg, depth + 1);
            b.push(Node::Bin(Bin::Mul, cloned, d))
        }
        // MyDouble.derivative and every object leaf: the constant 0
        _ => b.push(Node::Num(0.0)),
    }
}

fn stop_derivative(b: &mut Builder, depth: u32) -> bool {
    b.exhausted |= depth > expr::MAX_DEPTH
        || expr::work_done() > b.abort_at
        || b.nodes.len() >= MAX_RULE_NODES;
    b.exhausted
}

/// `Derivative.get` of a unary operation; `u` is the argument's node and
/// `du` its derivative.
fn differentiate_unary(c: &Construction, b: &mut Builder, op: Un, u: u32, depth: u32) -> u32 {
    let nan = |b: &mut Builder| b.push(Node::Num(f64::NAN));
    let du = |b: &mut Builder| differentiate_node(c, b, u, depth + 1);
    let chain = |b: &mut Builder, outer: Un| {
        let f = b.push(Node::Un(outer, u));
        let d = du(b);
        b.push(Node::Bin(Bin::Mul, f, d))
    };
    let minus_one = |b: &mut Builder, t: u32| {
        let m = b.push(Node::Num(-1.0));
        b.push(Node::Bin(Bin::Mul, t, m))
    };
    let squared = |b: &mut Builder, t: u32| {
        let two = b.push(Node::Num(2.0));
        b.push(Node::Bin(Bin::Pow, t, two))
    };
    let one = |b: &mut Builder| b.push(Node::Num(1.0));
    match op {
        Un::Sin => chain(b, Un::Cos),
        Un::Cos => {
            let t = chain(b, Un::Sin);
            minus_one(b, t)
        }
        Un::Tan => {
            let sec = b.push(Node::Un(Un::Sec, u));
            let sq = squared(b, sec);
            let d = du(b);
            b.push(Node::Bin(Bin::Mul, sq, d))
        }
        Un::Sec => {
            let sec = b.push(Node::Un(Un::Sec, u));
            let tan = b.push(Node::Un(Un::Tan, u));
            let product = b.push(Node::Bin(Bin::Mul, sec, tan));
            let d = du(b);
            b.push(Node::Bin(Bin::Mul, product, d))
        }
        Un::Csc => {
            let csc = b.push(Node::Un(Un::Csc, u));
            let cot = b.push(Node::Un(Un::Cot, u));
            let product = b.push(Node::Bin(Bin::Mul, csc, cot));
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Mul, product, d));
            minus_one(b, t)
        }
        Un::Cot => {
            let csc = b.push(Node::Un(Un::Csc, u));
            let sq = squared(b, csc);
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Mul, sq, d));
            minus_one(b, t)
        }
        Un::Sinh => chain(b, Un::Cosh),
        Un::Cosh => chain(b, Un::Sinh),
        Un::Tanh => {
            let sech = b.push(Node::Un(Un::Sech, u));
            let sq = squared(b, sech);
            let d = du(b);
            b.push(Node::Bin(Bin::Mul, sq, d))
        }
        Un::Sech => {
            let sech = b.push(Node::Un(Un::Sech, u));
            let tanh = b.push(Node::Un(Un::Tanh, u));
            let product = b.push(Node::Bin(Bin::Mul, sech, tanh));
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Mul, product, d));
            minus_one(b, t)
        }
        Un::Csch => {
            let csch = b.push(Node::Un(Un::Csch, u));
            let coth = b.push(Node::Un(Un::Coth, u));
            let product = b.push(Node::Bin(Bin::Mul, csch, coth));
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Mul, product, d));
            minus_one(b, t)
        }
        Un::Coth => {
            let csch = b.push(Node::Un(Un::Csch, u));
            let sq = squared(b, csch);
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Mul, sq, d));
            minus_one(b, t)
        }
        Un::Asin | Un::AsinD => {
            // u' / sqrt(u^2 - 1)
            let sq = u_squared(b, u);
            let one = one(b);
            let m = b.push(Node::Bin(Bin::Sub, sq, one));
            let sqrt = b.push(Node::Un(Un::Sqrt, m));
            let d = du(b);
            b.push(Node::Bin(Bin::Div, d, sqrt))
        }
        Un::Acos | Un::AcosD => {
            let sq = u_squared(b, u);
            let one = one(b);
            let m = b.push(Node::Bin(Bin::Sub, sq, one));
            let sqrt = b.push(Node::Un(Un::Sqrt, m));
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Div, d, sqrt));
            minus_one(b, t)
        }
        Un::Atan | Un::AtanD => {
            let sq = u_squared(b, u);
            let one = one(b);
            let s = b.push(Node::Bin(Bin::Add, sq, one));
            let d = du(b);
            b.push(Node::Bin(Bin::Div, d, s))
        }
        Un::Asinh => {
            let sq = u_squared(b, u);
            let one = one(b);
            let s = b.push(Node::Bin(Bin::Add, sq, one));
            let sqrt = b.push(Node::Un(Un::Sqrt, s));
            let d = du(b);
            b.push(Node::Bin(Bin::Div, d, sqrt))
        }
        Un::Acosh => {
            let first = one(b);
            let p = b.push(Node::Bin(Bin::Add, u, first));
            let sp = b.push(Node::Un(Un::Sqrt, p));
            let second = one(b);
            let m = b.push(Node::Bin(Bin::Sub, u, second));
            let sm = b.push(Node::Un(Un::Sqrt, m));
            let product = b.push(Node::Bin(Bin::Mul, sp, sm));
            let d = du(b);
            b.push(Node::Bin(Bin::Div, d, product))
        }
        Un::Atanh => {
            let sq = u_squared(b, u);
            let one = one(b);
            let m = b.push(Node::Bin(Bin::Sub, sq, one));
            let d = du(b);
            b.push(Node::Bin(Bin::Div, d, m))
        }
        Un::Abs => {
            // u' u / |u|
            let d = du(b);
            let product = b.push(Node::Bin(Bin::Mul, d, u));
            let abs = b.push(Node::Un(Un::Abs, u));
            b.push(Node::Bin(Bin::Div, product, abs))
        }
        Un::Sgn => {
            // the tree 0 / x
            let zero = b.push(Node::Num(0.0));
            let var = b.push(Node::Var);
            b.push(Node::Bin(Bin::Div, zero, var))
        }
        Un::Exp => {
            let d = du(b);
            let e = b.push(Node::Un(Un::Exp, u));
            b.push(Node::Bin(Bin::Mul, d, e))
        }
        Un::Ln => {
            let d = du(b);
            b.push(Node::Bin(Bin::Div, d, u))
        }
        Un::Log10 => {
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Div, d, u));
            let l = b.push(Node::Num(10.0_f64.ln()));
            b.push(Node::Bin(Bin::Div, t, l))
        }
        Un::Log2 => {
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Div, d, u));
            let l = b.push(Node::Num(2.0_f64.ln()));
            b.push(Node::Bin(Bin::Div, t, l))
        }
        Un::Sqrt => {
            // u' u^(-0.5) / 2
            let half = b.push(Node::Num(-0.5));
            let power = b.push(Node::Bin(Bin::Pow, u, half));
            let d = du(b);
            let product = b.push(Node::Bin(Bin::Mul, d, power));
            let two = b.push(Node::Num(2.0));
            b.push(Node::Bin(Bin::Div, product, two))
        }
        Un::Cbrt => {
            // u' / cbrt(u^2) / 3
            let sq = u_squared(b, u);
            let root = b.push(Node::Un(Un::Cbrt, sq));
            let d = du(b);
            let t = b.push(Node::Bin(Bin::Div, d, root));
            let three = b.push(Node::Num(3.0));
            b.push(Node::Bin(Bin::Div, t, three))
        }
        Un::Floor | Un::Ceil | Un::Round | Un::Not => b.push(Node::Num(0.0)),
        // CmdLength supplies a number object for a fixed point or vector.
        Un::Length if !b.contains_var(u) => b.push(Node::Num(0.0)),
        Un::X | Un::Y => {
            // x(A) / y(A): 0 unless the argument reads the function
            // variable, which the coordinate rules would have to expand
            // (not ported; GeoGebra logs and answers undefined)
            if b.contains_var(u) {
                nan(b)
            } else {
                b.push(Node::Num(0.0))
            }
        }
        // factorial and gamma need the polygamma function the kernel does
        // not have; everything else is outside these rules
        _ => nan(b),
    }
}

/// `u^2` as a fresh node.
fn u_squared(b: &mut Builder, u: u32) -> u32 {
    let two = b.push(Node::Num(2.0));
    b.push(Node::Bin(Bin::Pow, u, two))
}

/// `Derivative.get` of a binary operation; `u` and `v` are the operands.
fn differentiate_binary(
    c: &Construction,
    b: &mut Builder,
    op: Bin,
    u: u32,
    v: u32,
    depth: u32,
) -> u32 {
    let nan = |b: &mut Builder| b.push(Node::Num(f64::NAN));
    let du = |b: &mut Builder| differentiate_node(c, b, u, depth + 1);
    let dv = |b: &mut Builder| differentiate_node(c, b, v, depth + 1);
    // `isNumberValue() && !contains(fv)`: a number object counts as a
    // number, with its current value
    let is_number = |b: &Builder, i: u32| !b.contains_var(i) && b.number_value(c, i).is_some();
    match op {
        Bin::Add => {
            let l = du(b);
            let r = dv(b);
            b.push(Node::Bin(Bin::Add, l, r))
        }
        Bin::Sub => {
            let l = du(b);
            let r = dv(b);
            b.push(Node::Bin(Bin::Sub, l, r))
        }
        Bin::Mul => {
            // a constant factor comes out
            if is_number(b, v) {
                let d = du(b);
                b.push(Node::Bin(Bin::Mul, d, v))
            } else if is_number(b, u) {
                let d = dv(b);
                b.push(Node::Bin(Bin::Mul, d, u))
            } else {
                // u v' + v u'
                let dvv = dv(b);
                let left = b.push(Node::Bin(Bin::Mul, u, dvv));
                let duu = du(b);
                let right = b.push(Node::Bin(Bin::Mul, v, duu));
                b.push(Node::Bin(Bin::Add, left, right))
            }
        }
        Bin::Div => {
            if is_number(b, v) {
                let d = du(b);
                b.push(Node::Bin(Bin::Div, d, v))
            } else if let Node::Bin(Bin::Pow, rl, rr) = b.node(v) {
                if !b.contains_var(rr) {
                    // u / rl^rr: (u' rl - rl' u rr) / rl^(rr + 1)
                    let d_u = du(b);
                    let left = b.push(Node::Bin(Bin::Mul, d_u, rl));
                    let d_rl = differentiate_node(c, b, rl, depth + 1);
                    let t = b.push(Node::Bin(Bin::Mul, d_rl, u));
                    let right = b.push(Node::Bin(Bin::Mul, t, rr));
                    let num = b.push(Node::Bin(Bin::Sub, left, right));
                    let one = b.push(Node::Num(1.0));
                    let e = b.push(Node::Bin(Bin::Add, rr, one));
                    let den = b.push(Node::Bin(Bin::Pow, rl, e));
                    return b.push(Node::Bin(Bin::Div, num, den));
                }
                // (u' v - v' u) / v^2
                let (d_u, d_v) = (du(b), dv(b));
                quotient(b, d_u, d_v, u, v)
            } else {
                let (d_u, d_v) = (du(b), dv(b));
                quotient(b, d_u, d_v, u, v)
            }
        }
        Bin::RootPow => {
            if let Node::Bin(Bin::Div, _, degree) = b.node(v) {
                if is_number(b, degree) {
                    // Derivative.get: u' * nroot(u, n) / (u * n).
                    let root = b.push(Node::Bin(Bin::RootPow, u, v));
                    let d = du(b);
                    let numerator = b.push(Node::Bin(Bin::Mul, d, root));
                    let denominator = b.push(Node::Bin(Bin::Mul, u, degree));
                    return b.push(Node::Bin(Bin::Div, numerator, denominator));
                }
            }
            derivative_power(c, b, u, v, depth)
        }
        Bin::Pow => derivative_power(c, b, u, v, depth),
        Bin::Atan2 => {
            // (u' v - v' u) / (u^2 + v^2)
            let d_u = du(b);
            let left = b.push(Node::Bin(Bin::Mul, d_u, v));
            let d_v = dv(b);
            let right = b.push(Node::Bin(Bin::Mul, d_v, u));
            let num = b.push(Node::Bin(Bin::Sub, left, right));
            let us = u_squared(b, u);
            let vs = u_squared(b, v);
            let den = b.push(Node::Bin(Bin::Add, us, vs));
            b.push(Node::Bin(Bin::Div, num, den))
        }
        Bin::LogB => {
            // log(u, v) with the base first: ln(v)/ln(u) differentiated
            let lv = b.push(Node::Un(Un::Ln, v));
            let lu = b.push(Node::Un(Un::Ln, u));
            let t = b.push(Node::Bin(Bin::Div, lv, lu));
            differentiate_node(c, b, t, depth + 1)
        }
        // comparisons and the boolean operations derive to 0
        Bin::Less | Bin::Greater | Bin::LessEqual | Bin::GreaterEqual => b.push(Node::Num(0.0)),
        _ => nan(b),
    }
}

/// The quotient rule `(u' v - v' u) / v^2` of `Derivative.get`.
fn quotient(b: &mut Builder, d_u: u32, d_v: u32, u: u32, v: u32) -> u32 {
    let left = b.push(Node::Bin(Bin::Mul, d_u, v));
    let right = b.push(Node::Bin(Bin::Mul, d_v, u));
    let num = b.push(Node::Bin(Bin::Sub, left, right));
    let den = u_squared(b, v);
    b.push(Node::Bin(Bin::Div, num, den))
}

/// `Derivative.derivativePower`: the derivative of `u^v`.
fn derivative_power(c: &Construction, b: &mut Builder, u: u32, v: u32, depth: u32) -> u32 {
    if !b.contains_var(v) {
        if let Some(exponent) = b.number_value(c, v) {
            // x^0 derives to 0
            if is_zero(exponent) {
                return b.push(Node::Num(0.0));
            }
            // a literal exponent takes the special paths
            if is_literal(b, v) {
                if let Some(t) = derivative_constant_power(c, b, u, v, exponent, depth) {
                    return t;
                }
            }
            // u^(v-1) u' v
            let one = b.push(Node::Num(1.0));
            let em1 = b.push(Node::Bin(Bin::Sub, v, one));
            let power = b.push(Node::Bin(Bin::Pow, u, em1));
            let d = differentiate_node(c, b, u, depth + 1);
            let product = b.push(Node::Bin(Bin::Mul, power, d));
            return b.push(Node::Bin(Bin::Mul, product, v));
        }
    }
    // u^v (v' ln u + v u'/u)
    let d_v = differentiate_node(c, b, v, depth + 1);
    let ln = b.push(Node::Un(Un::Ln, u));
    let left = b.push(Node::Bin(Bin::Mul, d_v, ln));
    let d_u = differentiate_node(c, b, u, depth + 1);
    let right_p = b.push(Node::Bin(Bin::Mul, v, d_u));
    let right = b.push(Node::Bin(Bin::Div, right_p, u));
    let sum = b.push(Node::Bin(Bin::Add, left, right));
    let power = b.push(Node::Bin(Bin::Pow, u, v));
    b.push(Node::Bin(Bin::Mul, power, sum))
}

/// `Derivative.derivativeConstantPower` for a literal exponent: squares
/// and non-integer powers have their own trees.
fn derivative_constant_power(
    c: &Construction,
    b: &mut Builder,
    u: u32,
    v: u32,
    exponent: f64,
    depth: u32,
) -> Option<u32> {
    if is_equal(exponent, 2.0, STANDARD_PRECISION) {
        // (u u') v
        let d = differentiate_node(c, b, u, depth + 1);
        let product = b.push(Node::Bin(Bin::Mul, u, d));
        return Some(b.push(Node::Bin(Bin::Mul, product, v)));
    }
    if exponent != exponent.round() || !exponent.is_finite() {
        // a non-integer power a/b: u^((a-b)/b) a / b u'
        let (a, bottom) = decimal_to_fraction(exponent, STANDARD_PRECISION);
        if bottom == 0.0 {
            return Some(b.push(Node::Num(f64::NAN)));
        }
        let p = b.push(Node::Num(a - bottom));
        let q = b.push(Node::Num(bottom));
        let new_power = b.push(Node::Bin(Bin::Div, p, q));
        let power = b.push(Node::Bin(Bin::Pow, u, new_power));
        let an = b.push(Node::Num(a));
        let m = b.push(Node::Bin(Bin::Mul, power, an));
        let d = b.push(Node::Bin(Bin::Div, m, q));
        let du = differentiate_node(c, b, u, depth + 1);
        return Some(b.push(Node::Bin(Bin::Mul, d, du)));
    }
    None
}

/// Whether the subtree at `i` is a literal (`ExpressionNode.isConstant`):
/// no object and no function application inside.
fn is_literal(b: &Builder, i: u32) -> bool {
    let mut stack = vec![i];
    while let Some(i) = stack.pop() {
        match b.node(i) {
            Node::Obj(_) | Node::Apply(..) | Node::FnName(_) | Node::Cmd(..) => return false,
            node => stack.extend(crate::expr::children(&node)),
        }
    }
    true
}

/// Copies the subtree `i` of the builder `src` into `dst`, replacing the
/// function variable with `subst` where given (`Operation.FUNCTION` of the
/// derivative of an applied function).
fn clone_with_subst(
    dst: &mut Builder,
    src: &Builder,
    i: u32,
    subst: Option<u32>,
    depth: u32,
) -> u32 {
    if stop_derivative(dst, depth) {
        return 0;
    }
    expr::charge(1);
    if let Some(s) = subst {
        if src.node(i) == Node::Var {
            return s;
        }
    }
    let node = src.node(i);
    let child = |dst: &mut Builder, j: u32| clone_with_subst(dst, src, j, subst, depth + 1);
    let node = match node {
        Node::Un(op, a) => {
            let a = child(dst, a);
            Node::Un(op, a)
        }
        Node::Bin(op, a, d) => {
            let a = child(dst, a);
            let d = child(dst, d);
            Node::Bin(op, a, d)
        }
        Node::If(mode, cond, then, rest) => {
            let cond = child(dst, cond);
            let then = child(dst, then);
            let rest = if rest == u32::MAX {
                u32::MAX
            } else {
                child(dst, rest)
            };
            Node::If(mode, cond, then, rest)
        }
        Node::Apply(f, a) => {
            let a = child(dst, a);
            Node::Apply(f, a)
        }
        other => other,
    };
    dst.push(node)
}

// ------------------------------------------------- the numeric interval form

/// `AlgoIntegralNumericInterval.compute`: the antiderivative the rules
/// build, shifted through `(a, b)` and cut to `[a, c]` with an `If`. A
/// function the rules cannot integrate anywhere falls back to GeoGebra's
/// sampled data table, which the kernel has no equivalent of: that answer
/// is undefined here (a known gap).
fn numeric_interval_tree(c: &Construction, d: &CalculusDef) -> Option<Parsed> {
    let CalculusDef::NumericInterval {
        function,
        a: a_id,
        b: b_id,
        c: c_id,
    } = *d
    else {
        unreachable!("the caller matched NumericInterval")
    };
    let (function, a_id, b_id, c_id) = (gid(function), gid(a_id), gid(b_id), gid(c_id));
    let entry = source_entry(c, function)?;
    // a conditional source goes straight to the data table
    if matches!(
        expr::nodes_of(entry)[expr::root_of(entry) as usize],
        Node::If(..)
    ) {
        return None;
    }
    let integral = rule_integral(c, entry)?;
    // a NaN anywhere in the antiderivative is invalid, not just at the
    // root (`AlgoIntegralNumericInterval.isInvalid`)
    if parsed_has_nan(&integral) {
        return None;
    }
    let (ad, bd) = (number(c, a_id), number(c, b_id));
    // the offset b - I(a), both read at this update
    let at_a = expr::sample_parsed(c, &integral, ad);
    let offset = bd - at_a;
    let integral_root = integral.root;
    let integral = expr::function_entry(c, integral);
    let mut b = Builder::function(&integral);
    // the condition x >= a && x <= c
    let var = b.push(Node::Var);
    let obj_a = b.push(Node::Obj(a_id));
    let ge = b.push(Node::Bin(Bin::GreaterEqual, var, obj_a));
    let var = b.push(Node::Var);
    let obj_c = b.push(Node::Obj(c_id));
    let le = b.push(Node::Bin(Bin::LessEqual, var, obj_c));
    let cond = b.push(Node::Bin(Bin::And, ge, le));
    // the antiderivative plus the offset, under the condition
    let offset = b.push(Node::Num(offset));
    let payload = b.push(Node::Bin(Bin::Add, integral_root, offset));
    let root = b.push(Node::If(crate::expr::IfMode::List, cond, payload, u32::MAX));
    b.finish(c, root).ok()
}

/// Whether any node of the parsed tree is the `NaN` the failed rules
/// leave behind.
fn parsed_has_nan(parsed: &Parsed) -> bool {
    parsed
        .nodes
        .iter()
        .any(|n| matches!(n, Node::Num(v) if v.is_nan()))
}

// ------------------------------------------------------------- the polynomial

/// `AlgoPolynomialFromCoordinates.setFromPoints`: the polynomial through
/// the points of the list, as one expression tree. The arithmetic of
/// GeoGebra's `polcoeBig` is exact BigDecimal; this port runs the same
/// recurrence in `f64`, which is exact for the small coordinates the
/// tests use and stays within the golden tolerance otherwise.
fn polynomial_points_tree(c: &Construction, d: &CalculusDef) -> Option<Parsed> {
    let CalculusDef::PolynomialPoints { points } = *d else {
        unreachable!("the caller matched PolynomialPoints")
    };
    let points = gid(points);
    let list = c.elements(points)?;
    let mut x = Vec::with_capacity(list.len());
    let mut y = Vec::with_capacity(list.len());
    for item in list.iter() {
        // every element must be a defined point, in homogeneous
        // coordinates (`GeoPoint.getInhomCoords`)
        let coords = match item {
            Element::Value(Kind::Point, value) => match value.cartesian() {
                Value::Point { x, y } => Some((x, y)),
                _ => None,
            },
            _ => None,
        }?;
        x.push(coords.0);
        y.push(coords.1);
    }
    let total = x.len();
    if total < 2 {
        return None;
    }
    // equal points collapse; the same x with two y values is undefined
    let mut remove = vec![false; total];
    for i in 0..total - 1 {
        for j in i + 1..total {
            if x[i] == x[j] {
                if y[i] == y[j] {
                    remove[j] = true;
                } else {
                    return None;
                }
            }
        }
    }
    let mut n = total;
    while n > 0 && remove[n - 1] {
        n -= 1;
    }
    if n > 2 {
        for i in (1..n - 1).rev() {
            if remove[i] {
                x[i] = x[n - 1];
                y[i] = y[n - 1];
                n -= 1;
            }
        }
    }
    if n == 0 {
        return None;
    }
    let mut cof = vec![0.0; n];
    polcoe(&x[..n], &y[..n], &mut cof)?;
    let mut b = Builder::empty();
    poly_from_coefficients(c, &mut b, &cof)
}

/// `polcoeBig` without the BigDecimals: the same recurrence over the
/// first `cof.len()` points.
fn polcoe(x: &[f64], y: &[f64], cof: &mut [f64]) -> Option<()> {
    let n = x.len();
    let mut s = vec![0.0; n];
    s[n - 1] = -x[0];
    for (i, &xi) in x.iter().enumerate().skip(1) {
        for j in n - 1 - i..n - 1 {
            s[j] -= xi * s[j + 1];
        }
        s[n - 1] -= xi;
    }
    for (j, &xj) in x.iter().enumerate() {
        let mut phi = n as f64;
        for k in (1..n).rev() {
            phi = k as f64 * s[k] + xj * phi;
        }
        // BigDecimal throws on a zero divisor
        if phi == 0.0 {
            return None;
        }
        let ff = y[j] / phi;
        let mut b = 1.0;
        for k in (0..n).rev() {
            cof[k] += b * ff;
            b = s[k] + xj * b;
        }
    }
    cof.iter().all(|v| v.is_finite()).then_some(())
}

/// `buildPolyFunctionExpression`: the expanded polynomial as one tree,
/// built top-down with the sign and unit-coefficient rules of
/// `addToPoly`.
pub(crate) fn poly_from_coefficients(
    c: &Construction,
    b: &mut Builder,
    cof: &[f64],
) -> Option<Parsed> {
    let root = poly_terms(b, cof)?;
    b.finish(c, root).ok()
}

/// The tree of `poly_from_coefficients` as a node of the builder's arena,
/// for trees that embed the polynomial (a spline piece).
fn poly_terms(b: &mut Builder, cof: &[f64]) -> Option<u32> {
    let n = cof.len();
    let mut poly: Option<u32> = None;
    for k in (0..n).rev() {
        let coeff = cof[k];
        if !coeff.is_finite() {
            return None;
        }
        if coeff == 0.0 {
            continue;
        }
        let power = match k {
            0 => None,
            1 => Some(b.push(Node::Var)),
            _ => {
                let v = b.push(Node::Var);
                let e = b.push(Node::Num(k as f64));
                Some(b.push(Node::Bin(Bin::Pow, v, e)))
            }
        };
        // addToPoly: a unit coefficient drops its factor; the negative
        // coefficient of a later term flips its node and subtracts
        let existing = poly.is_some();
        let positive_unit = is_equal(coeff, 1.0, STANDARD_PRECISION);
        let negative_unit = is_equal(coeff, -1.0, STANDARD_PRECISION);
        let part = if positive_unit || (existing && negative_unit) {
            match power {
                None => b.push(Node::Num(1.0)),
                Some(p) => p,
            }
        } else {
            let shown = if existing && coeff < 0.0 {
                -coeff
            } else {
                coeff
            };
            let cn = b.push(Node::Num(shown));
            match power {
                None => cn,
                Some(p) => b.push(Node::Bin(Bin::Mul, cn, p)),
            }
        };
        poly = Some(match poly {
            None => part,
            Some(p) if coeff < 0.0 => b.push(Node::Bin(Bin::Sub, p, part)),
            Some(p) => b.push(Node::Bin(Bin::Add, p, part)),
        });
    }
    Some(poly.unwrap_or_else(|| b.push(Node::Num(0.0))))
}

// ------------------------------------------------------------- the records

/// Builds the tree a record-backed calculus function samples; `None` when
/// the command has no answer without the CAS.
fn build_record(c: &Construction, d: &CalculusDef) -> (Option<Entry>, Option<Entry>) {
    let tree = match *d {
        CalculusDef::Derivative {
            function,
            order,
            numeric,
        } => {
            let Some(entry) = source_entry(c, gid(function)) else {
                return (None, None);
            };
            // the order rounds like Math.round; a negative order is
            // undefined, order zero a copy of the function
            let Some(order) = derivative_order(c, order) else {
                return (None, None);
            };
            if order == 0 {
                return (Some(expr::function_entry(c, expr::parsed_of(entry))), None);
            }
            // `Function.getDerivative`: the factor check runs only inside
            // the polynomial branch, so a non-polynomial never routes
            // around it; without the CAS such a derivative is undefined
            // and only `NDerivative` (and the factor path, which returns
            // `getDerivativeNoCAS`) walks the rules
            derivative_tree(c, entry, order, !numeric).map(|p| expr::function_entry(c, p))
        }
        CalculusDef::CurveDerivative { curve, order } => {
            let Some(order) = derivative_order(c, order) else {
                return (None, None);
            };
            let Some((x, y)) =
                curve_coordinates(c, gid(curve), MAX_CURVE_SOURCES, &mut Budget::new())
            else {
                return (None, None);
            };
            if order == 0 {
                return (Some(x), Some(y));
            }
            // a curve always differentiates through the fast no-CAS path
            // (`setDerivative` calls `getDerivative(n, true)`)
            let (Some(x), Some(y)) = (
                derivative_tree(c, &x, order, false),
                derivative_tree(c, &y, order, false),
            ) else {
                return (None, None);
            };
            return (
                Some(expr::function_entry(c, x)),
                Some(expr::function_entry(c, y)),
            );
        }
        CalculusDef::IndefiniteIntegral { function, numeric } => {
            let Some(entry) = source_entry(c, gid(function)) else {
                return (None, None);
            };
            if numeric {
                rule_integral(c, entry).map(|p| expr::function_entry(c, p))
            } else {
                polynomial_antiderivative(c, entry).map(|p| expr::function_entry(c, p))
            }
        }
        CalculusDef::NumericInterval { .. } => {
            numeric_interval_tree(c, d).map(|p| expr::function_entry(c, p))
        }
        CalculusDef::PolynomialPoints { .. } => {
            polynomial_points_tree(c, d).map(|p| expr::function_entry(c, p))
        }
        CalculusDef::SymbolicOde { source, point } => {
            // a curve, a function of two variables and a function made by
            // a command have no entry here: they stay undefined
            let Some(entry) = source_entry(c, gid(source)) else {
                return (None, None);
            };
            let through = match point {
                Some(point) => match c.value(gid(point)) {
                    Value::Point { x, y } if x.is_finite() && y.is_finite() => Some((x, y)),
                    _ => return (None, None),
                },
                None => None,
            };
            polynomial_ode_solution(c, entry, through).map(|p| expr::function_entry(c, p))
        }
        _ => None,
    };
    (tree, None)
}

/// Round to a signed long, then keep the low 32 bits as a signed order.
fn derivative_order(c: &Construction, order: Option<u32>) -> Option<u32> {
    let order = order.map_or(1, |id| {
        expr::math::java_round(expr::relation_number(c, gid(id))) as i32
    });
    u32::try_from(order).ok()
}

/// `Function.getDerivative(n, fast)`: the polynomial path with its factor
/// check, and the rule tree where the polynomial path does not answer.
/// `cas_gated` marks the forms that need the CAS for a non-polynomial (the
/// derivative of a function; a curve is always fast).
fn derivative_tree(c: &Construction, entry: &Entry, order: u32, cas_gated: bool) -> Option<Parsed> {
    if let Some(p) = polynomial_derivative(c, entry, order) {
        if has_high_power(c, entry) {
            rule_derivative(c, entry, order)
        } else {
            Some(p)
        }
    } else if cas_gated {
        // the CAS answers; without it the derivative is undefined
        None
    } else {
        rule_derivative(c, entry, order)
    }
}

const MAX_CURVE_SOURCES: u8 = 12;

/// The two coordinate functions of a defined Cartesian curve.
fn curve_coordinates(
    c: &Construction,
    id: Id,
    depth: u8,
    budget: &mut Budget,
) -> Option<(Entry, Entry)> {
    use super::{analysis::AnalysisDef, transform::TransformDef, vector::VectorDef};
    let depth = depth.checked_sub(1)?;
    let id = c.conditional_source(id)?;
    if !matches!(c.value(id), Value::Curve { .. }) {
        return None;
    }
    let copy = |entry: &Entry, budget: &mut Budget| {
        (entry.nodes.len() <= MAX_RULE_NODES && budget.step(c, entry.nodes.len() as u64))
            .then(|| expr::function_entry(c, expr::parsed_of(entry)))
    };
    match c.def(id) {
        Def::Measure(super::measure::MeasureDef::CartesianCurve {
            coordinates: [x, y],
            ..
        }) => {
            let entry = |id: u32| match c.def(gid(id)) {
                Def::Expr(d) => Some(expr::source_entry(c, d)),
                _ => None,
            };
            Some((copy(entry(x)?, budget)?, copy(entry(y)?, budget)?))
        }
        Def::Calculus(CalculusDef::CurveDerivative { .. }) => {
            let record = c.calculus_record(id)?;
            let (x, y) = (record.tree.as_ref()?, record.tree2.as_ref()?);
            Some((copy(x, budget)?, copy(y, budget)?))
        }
        Def::Analysis(AnalysisDef::Spline(input)) => spline_coordinates(c, input),
        Def::Analysis(AnalysisDef::ParametricDerivative(source)) => {
            let (x, y) = curve_coordinates(c, source, depth, budget)?;
            let dx = expr::function_entry(c, derivative_tree(c, &x, 1, false)?);
            let dy = expr::function_entry(c, derivative_tree(c, &y, 1, false)?);
            let nodes = dx.nodes.len() + dy.nodes.len();
            if nodes >= MAX_RULE_NODES || !budget.step(c, nodes as u64) {
                return None;
            }
            let mut builder = Builder::function(&dy);
            let denominator = builder.append_tree(&dx);
            let root = builder.push(Node::Bin(Bin::Div, dy.root, denominator));
            let slope = expr::function_entry(c, builder.finish(c, root).ok()?);
            Some((x, slope))
        }
        Def::Transform(TransformDef::Dilate {
            src,
            factor,
            center,
            ..
        }) => {
            let coordinates = curve_coordinates(c, src, depth, budget)?;
            let ratio = c.number(factor)?;
            let (cx, cy) = center.map_or(Some((0.0, 0.0)), |id| c.point(id))?;
            let affine = [
                [ratio, 0.0, cx - ratio * cx],
                [0.0, ratio, cy - ratio * cy],
                [0.0, 0.0, 1.0],
            ];
            transform_coordinate_trees(c, coordinates, affine, budget)
        }
        Def::Vector(VectorDef::ApplyMatrix { matrix, src, kind }) => {
            let coordinates = match kind {
                Kind::Curve => curve_coordinates(c, src, depth, budget)?,
                Kind::Function => {
                    let y =
                        source_entry(c, src).or_else(|| c.calculus_record(src)?.tree.as_ref())?;
                    let mut builder = Builder::empty();
                    let root = builder.push(Node::Var);
                    let x = expr::function_entry(c, builder.finish(c, root).ok()?);
                    (x, copy(y, budget)?)
                }
                _ => return None,
            };
            let matrix = super::vector::matrix::Matrix::read(c.elements(matrix)?)?;
            let (a, d, g) = matrix.curve_coordinates((1.0, 0.0), 0.0);
            let (b, e, h) = matrix.curve_coordinates((0.0, 1.0), 0.0);
            let (p, q, r) = matrix.curve_coordinates((0.0, 0.0), 1.0);
            transform_coordinate_trees(c, coordinates, [[a, b, p], [d, e, q], [g, h, r]], budget)
        }
        _ => None,
    }
}

fn transform_coordinate_trees(
    c: &Construction,
    (x, y): (Entry, Entry),
    [first, second, last]: [[f64; 3]; 3],
    budget: &mut Budget,
) -> Option<(Entry, Entry)> {
    let nodes = x.nodes.len() + y.nodes.len();
    if nodes > MAX_RULE_NODES || !budget.step(c, 4 * nodes as u64) {
        return None;
    }
    let mut builder = Builder::function(&x);
    let roots = [x.root, builder.append_tree(&y)];
    let mut linear = |[a, b, offset]: [f64; 3]| {
        let constant = builder.push(Node::Num(offset));
        [a, b]
            .into_iter()
            .zip(roots)
            .fold(constant, |sum, (factor, src)| {
                if factor == 0.0 {
                    return sum;
                }
                let factor = builder.push(Node::Num(factor));
                let term = builder.push(Node::Bin(Bin::Mul, factor, src));
                builder.push(Node::Bin(Bin::Add, sum, term))
            })
    };
    let mut roots = [linear(first), linear(second)];
    if last != [0.0, 0.0, 1.0] {
        let scale = linear(last);
        roots = roots.map(|root| builder.push(Node::Bin(Bin::Div, root, scale)));
    }
    let make = |root| Some(expr::function_entry(c, builder.finish(c, root).ok()?));
    Some((make(roots[0])?, make(roots[1])?))
}

/// The coordinate functions of a spline (`AlgoSpline.compute`): one
/// `IF_LIST` chain over the piece polynomials, per coordinate, with the
/// condition `t < limit` between the pieces and the last piece as the
/// trailing else.
fn spline_coordinates(
    c: &Construction,
    input: super::analysis::SplineInput,
) -> Option<(Entry, Entry)> {
    let spline = c.analysis.spline(c, input)?;
    let pieces: Vec<_> = spline
        .pieces()
        .map(|(_, right, row)| (right, row))
        .collect();
    let coordinate = |axis: usize| -> Option<Entry> {
        let mut b = Builder::empty();
        let mut payloads = Vec::with_capacity(pieces.len());
        for &(_, row) in &pieces {
            let coefficients: Vec<f64> = row.iter().rev().map(|&pair| pair[axis]).collect();
            payloads.push(poly_terms(&mut b, &coefficients)?);
        }
        // the last piece is the trailing else of the `IF_LIST` chain
        let last = pieces.len() - 1;
        let mut root = payloads.pop()?;
        for (i, &(right, _)) in pieces[..last].iter().enumerate().rev() {
            let limit = b.push(Node::Num(right));
            let var = b.push(Node::Var);
            let cond = b.push(Node::Bin(Bin::Less, var, limit));
            root = b.push(Node::If(IfMode::List, cond, payloads[i], root));
        }
        b.finish(c, root).ok().map(|p| expr::function_entry(c, p))
    };
    Some((coordinate(0)?, coordinate(1)?))
}

/// `Function.getDerivative`'s factor check: any power whose base is not a
/// leaf and whose exponent is over `MAX_EXPAND_DEGREE` routes around the
/// polynomial expansion. The walk covers the tree (`ExpressionNode.any`),
/// not the parser's arena, and reads each exponent with the objects' current
/// values (`evaluateDouble`, which charges the work meter per node).
fn has_high_power(c: &Construction, entry: &Entry) -> bool {
    let nodes = expr::nodes_of(entry);
    let mut stack = vec![entry.root];
    let mut seen = vec![false; nodes.len()];
    while let Some(i) = stack.pop() {
        if seen[i as usize] {
            continue;
        }
        seen[i as usize] = true;
        expr::charge(1);
        let node = nodes[i as usize];
        if let Node::Bin(Bin::Pow, base, exponent) = node {
            let leaf = matches!(
                nodes[base as usize],
                Node::Num(_) | Node::Var | Node::MinusOne | Node::Degree | Node::Obj(_)
            );
            // `isMaxExpandDegreeReached`: a NaN exponent is not over the
            // degree, as in Java's comparison
            if !leaf && expr::sample_subtree(c, entry, exponent, 0.0) > MAX_EXPAND_DEGREE {
                return true;
            }
        }
        let mut kids: Vec<u32> = crate::expr::children(&node)
            .filter(|&k| k != crate::expr::NONE)
            .collect();
        kids.reverse();
        stack.extend(kids);
    }
    false
}

// ------------------------------------------------------------- the commands

/// The calculus commands of a definition line, after the inline numeric
/// arguments have been resolved to hidden objects (see
/// `script::resolve_geometry_arguments`). `None` when the line is not one
/// of these commands; the command's own error otherwise, exactly where
/// GeoGebra's `CmdIntegral` and friends reject their arguments.
pub(crate) fn exec(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (func, args) = crate::script::split_call(rhs)?;
    let add = |c: &mut Construction, def: CalculusDef| {
        c.add(name, Def::Calculus(def))
            .map(Some)
            .map_err(|e| e.to_string())
    };
    let result = match (func, args.as_slice()) {
        ("LowerSum", [f, a, b, n]) => sum(c, name, Sum::Lower, [f, a, b, n], None),
        ("UpperSum", [f, a, b, n]) => sum(c, name, Sum::Upper, [f, a, b, n], None),
        ("LeftSum", [f, a, b, n]) => sum(c, name, Sum::Left, [f, a, b, n], None),
        ("TrapezoidalSum", [f, a, b, n]) => sum(c, name, Sum::Trapezoid, [f, a, b, n], None),
        ("RectangleSum", [f, a, b, n, divider]) => {
            sum(c, name, Sum::Rectangle, [f, a, b, n], Some(divider))
        }
        // the command name does not narrow the one- and two-argument
        // forms (`CmdIntegral.process` checks none): `IntegralBetween(f)`
        // is the antiderivative too
        ("Integral", [f]) | ("NIntegral", [f]) | ("IntegralBetween", [f]) => {
            let function = function_argument(c, f)?;
            add(
                c,
                CalculusDef::IndefiniteIntegral {
                    function,
                    numeric: func == "NIntegral",
                },
            )
        }
        ("Integral", [f, x]) | ("NIntegral", [f, x]) | ("IntegralBetween", [f, x]) => {
            // the variable form `Integral(f, x)`
            let function = function_argument(c, f)?;
            if let Err(e) = variable(c, x, None) {
                return Some(Err(e));
            }
            add(
                c,
                CalculusDef::IndefiniteIntegral {
                    function,
                    numeric: func == "NIntegral",
                },
            )
        }
        ("Integral", [f, a, b]) | ("NIntegral", [f, a, b]) | ("IntegralBetween", [f, a, b]) => {
            let (function, a, b) = (
                function_argument(c, f)?,
                number_argument(c, a)?,
                number_argument(c, b)?,
            );
            add(
                c,
                CalculusDef::DefiniteIntegral {
                    function,
                    a,
                    b,
                    evaluate: None,
                    numeric: func == "NIntegral",
                },
            )
        }
        ("Integral", [f, x, a, b])
        | ("NIntegral", [f, x, a, b])
        | ("IntegralBetween", [f, x, a, b]) => {
            let f_id = function_argument(c, f)?;
            // a boolean fourth argument is the `evaluate` flag, checked
            // before the number forms (CmdIntegral's second branch)
            if let Some(evaluate) = boolean_argument(c, b) {
                let (a, b) = (number_argument(c, x)?, number_argument(c, a)?);
                return Some(add(
                    c,
                    CalculusDef::DefiniteIntegral {
                        function: f_id,
                        a,
                        b,
                        evaluate: Some(evaluate),
                        numeric: false,
                    },
                ));
            }
            let (a, b) = (number_argument(c, a)?, number_argument(c, b)?);
            if func == "NIntegral" {
                // the four-number form: the antiderivative through (a, b)
                // up to c
                let start = number_argument(c, x)?;
                add(
                    c,
                    CalculusDef::NumericInterval {
                        function: f_id,
                        a: start,
                        b: a,
                        c: b,
                    },
                )
            } else {
                // two curves: a function, or a number as the constant one
                let second = match function_argument(c, x) {
                    Some(g) => g,
                    None => match constant_function(c, x).or_else(|| number_argument(c, x)) {
                        Some(n) => n,
                        None => return Some(Err("`g` must be a function or a number".into())),
                    },
                };
                add(
                    c,
                    CalculusDef::IntegralFunctions {
                        f: f_id,
                        g: second,
                        a,
                        b,
                        evaluate: None,
                    },
                )
            }
        }
        ("Integral", [f, g, a, b, evaluate])
        | ("NIntegral", [f, g, a, b, evaluate])
        | ("IntegralBetween", [f, g, a, b, evaluate]) => {
            let Some(evaluate) = boolean_argument(c, evaluate) else {
                return Some(Err("the fifth argument must be a boolean".into()));
            };
            integral_between(c, name, f, g, a, b, Some(evaluate))
        }
        ("Derivative", [f]) | ("NDerivative", [f]) => {
            if let Some(curve) = curve_argument(c, f) {
                return Some(add(c, CalculusDef::CurveDerivative { curve, order: None }));
            }
            let function = function_argument(c, f)?;
            add(
                c,
                CalculusDef::Derivative {
                    function,
                    order: None,
                    numeric: func == "NDerivative",
                },
            )
        }
        ("Derivative", [f, order]) | ("NDerivative", [f, order]) => {
            if let Some(curve) = curve_argument(c, f) {
                if let Some(order) = number_argument(c, order) {
                    return Some(add(
                        c,
                        CalculusDef::CurveDerivative {
                            curve,
                            order: Some(order),
                        },
                    ));
                }
                // the parameter name is the only variable a curve reads
                // (`resArgsLocalNumVar` checks it against the curve's own)
                if let Err(e) = variable(c, order, curve_parameter(c, gid(curve))) {
                    return Some(Err(e));
                }
                return Some(add(c, CalculusDef::CurveDerivative { curve, order: None }));
            }
            let function = function_argument(c, f)?;
            if let Some(order) = number_argument(c, order) {
                add(
                    c,
                    CalculusDef::Derivative {
                        function,
                        order: Some(order),
                        numeric: func == "NDerivative",
                    },
                )
            } else {
                if let Err(e) = variable(c, order, None) {
                    return Some(Err(e));
                }
                add(
                    c,
                    CalculusDef::Derivative {
                        function,
                        order: None,
                        numeric: func == "NDerivative",
                    },
                )
            }
        }
        // the three-argument form reads any name or label as the variable
        // (`getVariableName` accepts a variable, a labelled object or the
        // function variable) and ignores it: a number literal or an
        // expression in that place is an error
        ("Derivative", [f, x, order]) | ("NDerivative", [f, x, order]) => {
            if !(c.id(x).is_some()
                || crate::script::is_identifier(x) && !matches!(*x, "pi" | "Pi" | "\u{3c0}"))
            {
                return Some(Err(format!("`{x}` is not a variable name")));
            }
            let order = number_argument(c, order)?;
            if let Some(curve) = curve_argument(c, f) {
                return Some(add(
                    c,
                    CalculusDef::CurveDerivative {
                        curve,
                        order: Some(order),
                    },
                ));
            }
            let function = function_argument(c, f)?;
            add(
                c,
                CalculusDef::Derivative {
                    function,
                    order: Some(order),
                    numeric: func == "NDerivative",
                },
            )
        }
        ("Polynomial", [single]) => {
            let points = if let Ok(id) = crate::script::object(c, single) {
                c.expect(id, &[Kind::List]).ok()?;
                id
            } else {
                let label = format!("\0arg{}", c.len());
                expr::argument_object(c, single, &label, Kind::List).then_some(())?;
                c.id(&label)?
            };
            add(
                c,
                CalculusDef::PolynomialPoints {
                    points: points.0 as u32,
                },
            )
        }
        ("Polynomial", points) if points.len() >= 2 => {
            let mut ids = Vec::with_capacity(points.len());
            for p in points {
                ids.push(crate::script::object(c, p).ok()?);
            }
            if !ids.iter().all(|&id| c.kind(id) == Kind::Point) {
                return None;
            }
            let list = point_list(c, &ids)?;
            add(
                c,
                CalculusDef::PolynomialPoints {
                    points: list.0 as u32,
                },
            )
        }
        _ => return None,
    };
    Some(result)
}

/// The curve argument of a calculus command: the label of a curve object.
fn curve_argument(c: &Construction, arg: &str) -> Option<u32> {
    crate::script::object(c, arg)
        .ok()
        .filter(|&id| c.kind(id) == Kind::Curve)
        .map(|id| id.0 as u32)
}

/// The parameter name of a Cartesian curve (`getFunctionVariables` of a
/// curve reads its first coordinate function).
fn curve_parameter(c: &Construction, mut id: Id) -> Option<&str> {
    for _ in 0..MAX_CURVE_SOURCES {
        id = c.conditional_source(id)?;
        match c.def(id) {
            Def::Measure(super::measure::MeasureDef::CartesianCurve {
                coordinates: [x, _],
                ..
            }) => {
                return match c.def(gid(x)) {
                    Def::Expr(d) => expr::curve_parameter(expr::source_entry(c, d)),
                    _ => None,
                };
            }
            Def::Analysis(super::analysis::AnalysisDef::Spline(_)) => return Some("t"),
            Def::Vector(super::vector::VectorDef::ApplyMatrix {
                kind: Kind::Function,
                ..
            }) => return Some("t"),
            Def::Analysis(super::analysis::AnalysisDef::ParametricDerivative(source))
            | Def::Transform(super::transform::TransformDef::Dilate { src: source, .. })
            | Def::Vector(super::vector::VectorDef::ApplyMatrix {
                src: source,
                kind: Kind::Curve,
                ..
            }) => {
                id = source;
            }
            Def::Calculus(CalculusDef::CurveDerivative { curve, .. }) => id = gid(curve),
            _ => return None,
        }
    }
    None
}

/// Whether `arg` names the variable `expect` (`x` for a function); the
/// message is the command processor's own.
fn variable(_c: &Construction, arg: &str, expect: Option<&str>) -> Result<(), String> {
    (arg == expect.unwrap_or("x"))
        .then_some(())
        .ok_or_else(|| format!("`{arg}` is not a variable name"))
}

/// The function argument of a calculus command: the label of a function
/// object, or an inline function expression (`Derivative(sin(x))`) that
/// becomes a hidden function object.
fn function_argument(c: &mut Construction, arg: &str) -> Option<u32> {
    if let Ok(id) = crate::script::object(c, arg) {
        return c
            .expect(id, &[Kind::Function])
            .is_ok()
            .then_some(id.0 as u32);
    }
    let label = format!("\0arg{}", c.len());
    expr::argument_function(c, arg, &label).map(|id| id.0 as u32)
}

/// A number as the constant function it counts as where GeoGebra accepts
/// one (`isRealValuedFunction` of a `GeoNumeric`): the second curve of
/// `Integral` and `IntegralBetween` (`CmdIntegral`'s first four-argument
/// branch).
fn constant_function(c: &mut Construction, arg: &str) -> Option<u32> {
    let label = format!("\0arg{}", c.len());
    expr::constant_function(c, arg, &label).map(|id| id.0 as u32)
}

/// The number argument of a calculus command: the label of a number (or
/// angle) object.
fn number_argument(c: &Construction, arg: &str) -> Option<u32> {
    let id = crate::script::object(c, arg).ok()?;
    c.expect(id, &[Kind::Number, Kind::Angle])
        .is_ok()
        .then_some(id.0 as u32)
}

/// The boolean argument of a calculus command: the label of a boolean
/// object.
fn boolean_argument(c: &Construction, arg: &str) -> Option<u32> {
    let id = crate::script::object(c, arg).ok()?;
    c.expect(id, &[Kind::Boolean])
        .is_ok()
        .then_some(id.0 as u32)
}

/// A Riemann sum line.
fn sum(
    c: &mut Construction,
    name: &str,
    sum: Sum,
    args: [&str; 4],
    divider: Option<&str>,
) -> Result<Option<Id>, String> {
    let [f, a, b, n] = args;
    let (function, a, b, n) = (
        function_argument(c, f).ok_or("the first argument must be a function")?,
        number_argument(c, a).ok_or("`a` must be a number")?,
        number_argument(c, b).ok_or("`b` must be a number")?,
        number_argument(c, n).ok_or("`n` must be a number")?,
    );
    let divider = match divider {
        Some(d) => Some(number_argument(c, d).ok_or("the position must be a number")?),
        None => None,
    };
    c.add(
        name,
        Def::Calculus(CalculusDef::Sum {
            sum,
            function,
            a,
            b,
            n,
            divider,
        }),
    )
    .map(Some)
    .map_err(|e| e.to_string())
}

/// `IntegralBetween(f, g, a, b[, evaluate])`: two functions, or a
/// function and a constant.
fn integral_between(
    c: &mut Construction,
    name: &str,
    f: &str,
    g: &str,
    a: &str,
    b: &str,
    evaluate: Option<u32>,
) -> Result<Option<Id>, String> {
    let first = function_argument(c, f)
        .or_else(|| constant_function(c, f))
        .ok_or("the first argument must be a function")?;
    let second = match function_argument(c, g) {
        Some(g) => g,
        None => constant_function(c, g)
            .or_else(|| number_argument(c, g))
            .ok_or("`g` must be a function or a number")?,
    };
    let (a, b) = (
        number_argument(c, a).ok_or("`a` must be a number")?,
        number_argument(c, b).ok_or("`b` must be a number")?,
    );
    c.add(
        name,
        Def::Calculus(CalculusDef::IntegralFunctions {
            f: first,
            g: second,
            a,
            b,
            evaluate,
        }),
    )
    .map(Some)
    .map_err(|e| e.to_string())
}

/// A hidden list of the point objects, so the polynomial keeps its
/// dependency on them (`CmdPolynomial` builds a `GeoList`).
fn point_list(c: &mut Construction, ids: &[Id]) -> Option<Id> {
    let mut b = Builder::empty();
    // the objects first, then the contiguous run of items the list reads
    let objects: Vec<u32> = ids.iter().map(|&id| b.push(Node::Obj(id))).collect();
    let first = objects.len() as u32;
    for object in objects {
        b.push(Node::Item(object));
    }
    let root = b.push(Node::List(first, ids.len() as u32));
    let parsed = b.finish(c, root).ok()?;
    expr::add_built(c, &format!("\0pts{}", c.len()), parsed, Kind::List).ok()
}

#[test]
fn derivative_discards_a_tree_that_exceeds_the_depth_limit() {
    let c = Construction::new();
    let mut b = Builder::empty();
    let x = b.push(Node::Var);
    let condition = b.push(Node::Bool(false));
    // nested through the payloads: each level is one step deeper
    let mut root = x;
    for _ in 0..=expr::MAX_DEPTH {
        root = b.push(Node::If(IfMode::List, condition, root, x));
    }
    let entry = expr::function_entry(&c, b.finish(&c, root).unwrap());
    assert!(rule_derivative(&c, &entry, 1).is_none());
}

#[test]
fn derivative_walks_a_long_condition_list_at_one_depth() {
    // the pieces of one `IF_LIST` are siblings (a 600-piece spline)
    let c = Construction::new();
    let mut b = Builder::empty();
    let x = b.push(Node::Var);
    let condition = b.push(Node::Bool(false));
    let mut root = x;
    for _ in 0..600 {
        root = b.push(Node::If(IfMode::List, condition, x, root));
    }
    let entry = expr::function_entry(&c, b.finish(&c, root).unwrap());
    assert!(rule_derivative(&c, &entry, 1).is_some());
}

#[test]
fn freehand_samples_read_a_few_cells_each() {
    // 100 000 samples of a 100 000-value freehand function: each sample
    // interpolates between two cells, as `Operation.FREEHAND` does
    let mut c = Construction::new();
    for line in [
        "L = Sequence(k, k, 1, 100000)",
        "f = Function(L)",
        "R = Sequence(f(1 + k / 100000), k, 1, 100000)",
    ] {
        crate::script::exec(&mut c, line).unwrap();
    }
    let start = std::time::Instant::now();
    assert!(c.value(c.id("R").unwrap()).is_defined());
    crate::script::exec(&mut c, "g = Sequence(f(1 + k / 50000), k, 1, 100000)").unwrap();
    assert!(c.value(c.id("g").unwrap()).is_defined());
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
}
