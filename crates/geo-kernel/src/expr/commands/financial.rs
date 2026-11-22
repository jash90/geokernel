// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoFinancial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/RateFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/RateFunctionDerivative.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdFinancialRate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdFinancialNper.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdFinancialPmt.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdFinancialPV.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdFinancialFV.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The financial group of the command matrix: `Rate`, `Periods`,
//! `Payment`, `FutureValue` and `PresentValue`, the compound-interest
//! family of `AlgoFinancial`. Everything but `Rate` is closed form; `Rate`
//! solves the fundamental equation
//! `pmt (1 + rate pmtType) ((1 + rate)^n - 1) / rate + pv (1 + rate)^n + fv = 0`
//! by bracketing a sign change and running Brent's root method over it,
//! then polishing the root with Newton steps, as `AlgoFinancial.computeRate`
//! does through its math library. The iteration structure of both solvers
//! follows the published algorithms (R. P. Brent, *Algorithms for
//! Minimization Without Derivatives*, chapter 4) with the accuracies,
//! the evaluation limit and the control flow of that library call
//! (absolute accuracy 1e-6, relative 1e-14, function value 1e-15, at most
//! 100 evaluations, midpoint start, exceptions treated as failure).

use crate::expr::eval::{Ctx, Val};
use crate::expr::math::{check_integer, is_equal, is_zero};
use crate::expr::parse::Parser;
use crate::expr::{Finance, Node, NONE};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["Rate", "Periods", "Payment", "FutureValue", "PresentValue"];

/// `Rate(nper, pmt, pv [, fv] [, type] [, guess])` and the closed forms of
/// the other four; of anything else they are the commands of the script
/// parser.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let Some(op) = Finance::of(text) else {
        return Ok(None);
    };
    let args = p.arguments()?;
    let (least, most) = op.arity();
    if !(least..=most).contains(&args.len()) {
        return Err(format!("`{text}` does not take {} argument(s)", args.len()));
    }
    let mut slots = [NONE; 6];
    for (slot, arg) in slots.iter_mut().zip(args) {
        *slot = arg;
    }
    Ok(Some(p.push(Node::Financial(op, slots))))
}

/// The value of a financial command (see `Node::Financial` in `eval`).
/// Every evaluation of the root function of the `Rate` search is charged
/// to the evaluation budget this command runs under.
pub(crate) fn value<const M: bool>(ctx: &Ctx<'_, M>, op: Finance, args: &[u32; 6]) -> Val {
    let mut values = [f64::NAN; 6];
    let count = args.iter().take_while(|&&a| a != NONE).count();
    for (slot, arg) in values.iter_mut().zip(args).take(count) {
        *slot = ctx.full_number(*arg);
    }
    let mut budget = 0u64;
    let charge = &mut || {
        budget += 1;
        ctx.stop(8)
    };
    Val::N(evaluate(op, &values[..count], charge))
}

impl Finance {
    /// The command's name.
    pub(super) fn of(name: &str) -> Option<Self> {
        Some(match name {
            "Rate" => Finance::Rate,
            "Periods" => Finance::Periods,
            "Payment" => Finance::Payment,
            "FutureValue" => Finance::FutureValue,
            "PresentValue" => Finance::PresentValue,
            _ => return None,
        })
    }

    /// How many arguments the forms of the command take, at least and at
    /// most (`CmdFinancialRate` and its siblings).
    pub(super) fn arity(self) -> (usize, usize) {
        match self {
            Finance::Rate => (3, 6),
            _ => (3, 5),
        }
    }

    /// The argument index the semantic `slot` is read from, `None` when
    /// the command does not take it (its own output).
    fn slot(self, slot: Slot) -> Option<usize> {
        use Finance::*;
        let index = match (self, slot) {
            (Rate, Slot::Nper) => 0,
            (Rate, Slot::Pmt) => 1,
            (Rate, Slot::Pv) => 2,
            (Rate, Slot::Fv) => 3,
            (Rate, Slot::PmtType) => 4,
            (Rate, Slot::Guess) => 5,
            (Periods, Slot::Rate) => 0,
            (Periods, Slot::Pmt) => 1,
            (Periods, Slot::Pv) => 2,
            (Periods, Slot::Fv) => 3,
            (Periods, Slot::PmtType) => 4,
            (Payment, Slot::Rate) => 0,
            (Payment, Slot::Nper) => 1,
            (Payment, Slot::Pv) => 2,
            (Payment, Slot::Fv) => 3,
            (Payment, Slot::PmtType) => 4,
            (PresentValue, Slot::Rate) => 0,
            (PresentValue, Slot::Nper) => 1,
            (PresentValue, Slot::Pmt) => 2,
            (PresentValue, Slot::Fv) => 3,
            (PresentValue, Slot::PmtType) => 4,
            (FutureValue, Slot::Rate) => 0,
            (FutureValue, Slot::Nper) => 1,
            (FutureValue, Slot::Pmt) => 2,
            (FutureValue, Slot::Pv) => 3,
            (FutureValue, Slot::PmtType) => 4,
            _ => return None,
        };
        Some(index)
    }
}

/// The semantic slots of `AlgoFinancial`.
enum Slot {
    Rate,
    Nper,
    Pmt,
    Pv,
    Fv,
    PmtType,
    Guess,
}

/// `AlgoFinancial.compute` for one command: `args` holds the values of the
/// form's arguments in order. A slot the command does not read (its own
/// output) stays 0; an optional slot the form leaves out defaults exactly
/// as the `setXxx` helpers of the algo make it, and a value the algo
/// rejects makes the result undefined. `charge` budgets the iterative
/// `Rate` search; once it refuses, the rate is undefined.
pub(crate) fn evaluate(op: Finance, args: &[f64], charge: &mut dyn FnMut() -> bool) -> f64 {
    let input = |slot: Slot| args.get(op.slot(slot).unwrap_or(usize::MAX)).copied();
    let required = |slot: Slot| -> Option<f64> { input(slot).filter(|v| v.is_finite()) };
    // `setNper`: the number of periods must be a positive integer; only
    // `Periods` does not read it (it is that command's output).
    let nper = if op == Finance::Periods {
        0.0
    } else {
        let Some(v) = required(Slot::Nper) else {
            return f64::NAN;
        };
        let n = check_integer(v);
        if n.is_nan() || n <= 0.0 {
            return f64::NAN;
        }
        n
    };
    // `setPmt`: every command but `Payment` reads it.
    let Some(pmt) = (op == Finance::Payment)
        .then_some(0.0)
        .or(required(Slot::Pmt))
    else {
        return f64::NAN;
    };
    // `setPV`: read by every command but `PresentValue`, and absent only
    // from the shorter forms of `FutureValue`, where it defaults to 0.
    let Some(pv) = (op == Finance::PresentValue)
        .then_some(0.0)
        .or_else(|| match input(Slot::Pv) {
            Some(v) if v.is_finite() => Some(v),
            Some(_) => None,
            None if op == Finance::FutureValue => Some(0.0),
            None => None,
        })
    else {
        return f64::NAN;
    };
    // `setFV`: read by every command but `FutureValue`; an absent future
    // value defaults to 0.
    let Some(fv) = (op == Finance::FutureValue)
        .then_some(0.0)
        .or_else(|| match input(Slot::Fv) {
            Some(v) if v.is_finite() => Some(v),
            Some(_) => None,
            None => Some(0.0),
        })
    else {
        return f64::NAN;
    };
    // `setPmtType`: exactly 0 or 1.
    let pmt_type = match input(Slot::PmtType) {
        None => 0.0,
        Some(v) if v == 0.0 || v == 1.0 => v,
        Some(_) => return f64::NAN,
    };
    // `AlgoFinancial.pmtFactor`.
    let factor =
        |rate: f64, nper: f64| (1.0 + rate * pmt_type) * ((1.0 + rate).powf(nper) - 1.0) / rate;
    match op {
        Finance::Rate => {
            // The search brackets around the guess only when the form
            // supplied one; `setGuess` refuses an undefined guess.
            let guess = match input(Slot::Guess) {
                None => None,
                Some(v) if v.is_finite() => Some(v),
                Some(_) => return f64::NAN,
            };
            compute_rate(nper, pv, fv, pmt, pmt_type, guess, charge).unwrap_or(f64::NAN)
        }
        Finance::Periods => {
            let Some(rate) = required(Slot::Rate) else {
                return f64::NAN;
            };
            let nper = if is_zero(rate) {
                check_integer(-(pv + fv) / pmt)
            } else {
                let pmt2 = pmt * (1.0 + rate * pmt_type);
                check_integer(((pmt2 - rate * fv) / (pmt2 + rate * pv)).ln() / (1.0 + rate).ln())
            };
            if nper <= 0.0 {
                return f64::NAN;
            }
            nper
        }
        Finance::Payment => {
            let Some(rate) = required(Slot::Rate) else {
                return f64::NAN;
            };
            if rate == 0.0 {
                -(pv + fv) / nper
            } else {
                (-fv - pv * (1.0 + rate).powf(nper)) / factor(rate, nper)
            }
        }
        Finance::PresentValue => {
            let Some(rate) = required(Slot::Rate) else {
                return f64::NAN;
            };
            if rate == 0.0 {
                -pmt * nper - fv
            } else {
                (-fv - pmt * factor(rate, nper)) / (1.0 + rate).powf(nper)
            }
        }
        Finance::FutureValue => {
            let Some(rate) = required(Slot::Rate) else {
                return f64::NAN;
            };
            if rate == 0.0 {
                -pmt * nper - pv
            } else {
                -pmt * factor(rate, nper) - pv * (1.0 + rate).powf(nper)
            }
        }
    }
}

/// Runs the Brent root search and the Newton polishing of
/// `AlgoFinancial.computeRate`; `None` where GeoGebra leaves the rate
/// undefined. A NaN guess stands for the argument the form left out.
fn compute_rate(
    nper: f64,
    pv: f64,
    fv: f64,
    pmt: f64,
    pmt_type: f64,
    guess: Option<f64>,
    charge: &mut dyn FnMut() -> bool,
) -> Option<f64> {
    let f = RateFunction {
        n: nper,
        pv,
        fv,
        pmt,
        pmt_type,
    };
    if charge() {
        return None;
    }
    // The helper `value` of the algo swallows the library's arithmetic
    // exceptions and returns NaN, whose signum makes comparisons false.
    let mut value = |x: f64| -> Option<f64> {
        if charge() {
            return None;
        }
        Some(f.value(x))
    };
    let mut sign = |x: f64| value(x).filter(|v| v.is_finite()).map(f64::signum);
    let (mut min, mut max, mut rate);
    if let Some(guess) = guess {
        rate = guess;
        let mut dx = 0.001;
        // Look for a sign change around the guess, within [-1, 1].
        min = (rate - dx).max(-1.0);
        max = (rate + dx).min(1.0);
        let mut min_sign = sign(min);
        let mut max_sign = sign(max);
        // A sensible bound on the rate (between 0 and 1).
        while min_sign == max_sign && dx < 1.0 {
            dx *= 2.0;
            min = (rate - dx).max(0.0);
            max = (rate + dx).min(1.0);
            min_sign = sign(min);
            max_sign = sign(max);
        }
    } else {
        // The default starting value if Brent fails.
        rate = 0.1;
        min = 0.0;
        max = 0.001;
        let min_sign = sign(min);
        let mut max_sign = sign(max);
        // A sensible bound on the rate (1): a quick and dirty look for an
        // interval with a sign change.
        while min_sign == max_sign && max < 1.0 {
            max += 0.05;
            max_sign = sign(max);
        }
    }
    if let Some(root) = brent_root(&f, min, max) {
        rate = root;
    }
    if is_equal(rate, 1.0) || !rate.is_finite() {
        rate = 0.1;
    }
    // A failing Newton search leaves the rate undefined (`computeRate`
    // returns false), whatever Brent found.
    let polished = newton_root(&f, min, max, rate)?;
    if f.value(polished).abs() < f.value(rate).abs() {
        rate = polished;
    }
    Some(rate)
}

/// The fundamental equation of `RateFunction`, with the removable hole at
/// `rate = 0` filled with its limit `n`.
struct RateFunction {
    n: f64,
    pv: f64,
    fv: f64,
    pmt: f64,
    pmt_type: f64,
}

impl RateFunction {
    fn value(&self, x: f64) -> f64 {
        if x == 0.0 {
            return self.fv + self.pmt * self.n + self.pv;
        }
        let pow = (1.0 + x).powf(self.n);
        self.fv + self.pmt * (1.0 + x * self.pmt_type) * (pow - 1.0) / x + self.pv * pow
    }

    /// The derivative of `RateFunctionDerivative`, with the hole at
    /// `x = 0` filled with its limit.
    fn derivative(&self, x: f64) -> f64 {
        let (a, b, c, n) = (self.pv, self.pmt, self.pmt_type, self.n);
        if x == 0.0 {
            return (b / 2.0 * n * n) + (b * c * n) + (a * n) - (b / 2.0 * n);
        }
        let p = (1.0 + x).powf(n);
        let x2 = x * x;
        let xp = x * p;
        let num = a * n * x2 * p + b * (c * n * x2 * p + n * xp - xp - p + x + 1.0);
        num / (x2 * (x + 1.0))
    }
}

/// A bracketing root search over `[min, max]`: Brent's 1973 method for
/// finding a zero without derivatives, written from the published
/// description of the algorithm (R. P. Brent, *Algorithms for Minimization
/// without Derivatives*, 1973, the chapter on root finding; inverse
/// quadratic and secant steps that fall back to bisection). The search
/// starts from the midpoint, brackets one sign change and refines it under
/// the shared evaluation limit (100, `AlgoRootNewton.MAX_ITERATIONS`) and
/// the accuracies the caller passes: a relative one of `1e-14` and an
/// absolute one of `1e-6`. `None` where the interval holds no sign change
/// or the budget runs out, which the caller treats as the caught
/// exception there.
fn brent_root(f: &RateFunction, min: f64, max: f64) -> Option<f64> {
    let mut evaluations = 0usize;
    let mut value = |x: f64| -> Option<f64> {
        if evaluations >= 100 {
            return None;
        }
        evaluations += 1;
        Some(f.value(x))
    };
    let start = 0.5 * (min + max);
    if !(min..=max).contains(&start) {
        return None;
    }
    let y_start = value(start)?;
    if y_start.abs() <= 1e-15 {
        return Some(start);
    }
    let y_min = value(min)?;
    if y_min.abs() <= 1e-15 {
        return Some(min);
    }
    if y_start * y_min < 0.0 {
        return refine(&mut value, min, start, y_min, y_start);
    }
    let y_max = value(max)?;
    if y_max.abs() <= 1e-15 {
        return Some(max);
    }
    if y_start * y_max < 0.0 {
        return refine(&mut value, start, max, y_start, y_max);
    }
    None
}

/// The refining loop of the search: the bracket runs between `best` (the
/// point with the smaller value) and `far`, and `third` remembers the
/// point that just left the bracket, so an inverse quadratic step through
/// the three of them (a secant through two, when two coincide) is tried
/// before every bisection. The loop ends at the accuracy of the call.
fn refine(
    value: &mut impl FnMut(f64) -> Option<f64>,
    lo: f64,
    hi: f64,
    y_lo: f64,
    y_hi: f64,
) -> Option<f64> {
    // `best` carries the smaller value of the two ends; the values swap
    // the ends whenever the trial point takes the lead.
    let mut far = lo;
    let mut y_far = y_lo;
    let mut best = hi;
    let mut y_best = y_hi;
    let mut third = lo;
    let mut y_third = y_lo;
    let mut step = best - far;
    let mut previous = step;
    loop {
        if y_far.abs() < y_best.abs() {
            far = best;
            best = third;
            third = far;
            y_far = y_best;
            y_best = y_third;
            y_third = y_far;
        }
        let tolerance = 2.0 * 1e-14 * best.abs() + 1e-6;
        let half_gap = 0.5 * (third - best);
        // converged, or an exactly zero (or undefined) value
        if half_gap.abs() <= tolerance || y_best.to_bits() == 0.0f64.to_bits() || y_best.is_nan() {
            return Some(best);
        }
        let (taken, remembered) = interpolation(Interpolation {
            far,
            best,
            third,
            y_far,
            y_third,
            y_best,
            step,
            previous,
            half_gap,
            tolerance,
        });
        step = taken;
        previous = remembered;
        far = best;
        y_far = y_best;
        best += if step.abs() > tolerance {
            step
        } else if half_gap > 0.0 {
            tolerance
        } else {
            -tolerance
        };
        y_best = value(best)?;
        // the bracket keeps a sign change with `third`
        if (y_best > 0.0 && y_third > 0.0) || (y_best <= 0.0 && y_third <= 0.0) {
            third = far;
            y_third = y_far;
            step = best - far;
            previous = step;
        }
    }
}

/// The state of one interpolation decision.
struct Interpolation {
    far: f64,
    best: f64,
    third: f64,
    y_far: f64,
    y_third: f64,
    y_best: f64,
    step: f64,
    previous: f64,
    half_gap: f64,
    tolerance: f64,
}

/// The step to take from `best` and the step to remember as the previous
/// one: an inverse quadratic (or secant) step through the known points
/// when it stays inside the bracket and promises real progress, the
/// bisection `half_gap` otherwise. The acceptance bound is Brent's own:
/// at most three halves of the bisection, and half of the step before
/// the previous one.
fn interpolation(s: Interpolation) -> (f64, f64) {
    let Interpolation {
        far,
        best,
        third,
        y_far,
        y_third,
        y_best,
        step,
        previous,
        half_gap,
        tolerance,
    } = s;
    if previous.abs() >= tolerance && y_far.abs() > y_best.abs() {
        let slope = y_best / y_far;
        let (p, q) = if far == third {
            (2.0 * half_gap * slope, 1.0 - slope)
        } else {
            let far_ratio = y_far / y_third;
            let best_ratio = y_best / y_third;
            (
                slope
                    * (2.0 * half_gap * far_ratio * (far_ratio - best_ratio)
                        - (best - far) * (best_ratio - 1.0)),
                (far_ratio - 1.0) * (best_ratio - 1.0) * (slope - 1.0),
            )
        };
        let (p, q) = if p > 0.0 { (p, -q) } else { (-p, q) };
        let before = previous;
        let remember = step;
        if p < 1.5 * half_gap * q - (tolerance * q).abs() && p < 0.5 * (before * q).abs() {
            return (p / q, remember);
        }
        return (half_gap, half_gap);
    }
    (half_gap, half_gap)
}

/// Newton's method for polishing the rate: `x - f(x) / f'(x)` until the
/// step stays within the absolute accuracy of the call (`1e-6`), under
/// the shared evaluation limit; the derivative evaluations are charged to
/// the same budget as the values. A zero derivative, an undefined value
/// or step, or the limit leaves the rate undefined, whatever the
/// bracketing search found.
fn newton_root(f: &RateFunction, min: f64, max: f64, start: f64) -> Option<f64> {
    if !(min..=max).contains(&start) || start.is_nan() {
        return None;
    }
    let mut evaluations = 0usize;
    let mut x0 = start;
    loop {
        if evaluations >= 100 {
            return None;
        }
        evaluations += 2;
        let y = f.value(x0);
        let d = f.derivative(x0);
        if d == 0.0 || !y.is_finite() || !d.is_finite() {
            return None;
        }
        let x1 = x0 - y / d;
        if !x1.is_finite() {
            return None;
        }
        if (x1 - x0).abs() <= 1e-6 {
            return Some(x1);
        }
        x0 = x1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(op: Finance, args: &[f64]) -> f64 {
        evaluate(op, args, &mut || false)
    }

    fn rate(args: &[f64]) -> f64 {
        eval(Finance::Rate, args)
    }

    /// The rate the search returns solves the fundamental equation
    /// `fv + pmt (1 + r type) ((1 + r)^n - 1) / r + pv (1 + r)^n = 0`;
    /// the exact recorded answers (including the convergence artefact
    /// near `-1` and the two undefined ones) are the harness cases
    /// `financial-rate-012` and `financial-rate-013`.
    #[test]
    fn rate_solves_the_fundamental_equation() {
        let ok = rate(&[60.0, -300.0, 10000.0]);
        let (n, pmt, pv) = (60.0, -300.0, 10000.0);
        let pow = (1.0 + ok).powf(n);
        let equation = pmt * (pow - 1.0) / ok + pv * pow;
        assert!(equation.abs() < 1e-2, "{ok}: {equation}");
        // no bracket around a root: undefined, with or without a guess
        assert!(rate(&[42.0, 4.0, 13.0, 1.0]).is_nan());
        assert!(rate(&[42.0, 4.0, 13.0, 0.0, 1.0, 42.0]).is_nan());
        // without a bracket the polish crawls to the `-1` bound of the
        // interval and stops there
        let near = rate(&[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert!((near - -1.0).abs() < 1e-9, "{near}");
    }

    /// The period count answers the annuity equation
    /// `(1 + r)^n = (pmt2 - r fv) / (pmt2 + r pv)` with
    /// `pmt2 = pmt (1 + r type)`; the recorded answers are the harness
    /// case `financial-periods-001`.
    #[test]
    fn periods_answer_the_annuity_equation() {
        let (r, pmt, pv, fv) = (10.0 / 100.0 / 12.0, -200.0, -400.0, 10000.0);
        for pmt_type in [0.0, 1.0] {
            let n = eval(Finance::Periods, &[r, pmt, pv, fv, pmt_type]);
            let pmt2 = pmt * (1.0 + r * pmt_type);
            let both_sides = ((pmt2 - r * fv) / (pmt2 + r * pv), (1.0 + r).powf(n));
            assert!(
                (both_sides.0 - both_sides.1).abs() < 1e-6 * both_sides.1.abs(),
                "{n}: {:?}",
                both_sides
            );
        }
    }

    /// The payment balances the equation to zero: `fv + pmt f + pv (1+r)^n`
    /// with `f = (1 + r type) ((1+r)^n - 1) / r`; the recorded answers are
    /// the harness case `financial-payment-001`.
    #[test]
    fn payment_balances_the_equation() {
        // the form is `Payment(rate, nper, pv[, fv[, type]])`
        for (fv, pmt_type) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
            let p = eval(Finance::Payment, &[42.0, 4.0, 13.0, fv, pmt_type]);
            let pow = 43.0f64.powi(4);
            let f = (1.0 + 42.0 * pmt_type) * (pow - 1.0) / 42.0;
            assert!((fv + p * f + 13.0 * pow).abs() < 1e-3, "{p}");
        }
    }

    /// The future value is `-pmt f - pv (1+r)^n`, straight for `r = 0`;
    /// the recorded answers are the harness case `financial-future-001`.
    #[test]
    fn future_value_is_the_growth_of_both_streams() {
        // the form is `FutureValue(rate, nper, pmt[, pv[, type]])`
        let pow = 43.0f64.powi(4);
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0]);
        assert_eq!(v, -13.0 * (pow - 1.0) / 42.0);
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0, 1.0]);
        assert_eq!(v, -13.0 * (pow - 1.0) / 42.0 - 1.0 * pow);
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert_eq!(v, -13.0 * 43.0 * (pow - 1.0) / 42.0);
        // zero rate: the plain sum of the payments and the present value
        assert_eq!(eval(Finance::FutureValue, &[0.0, 4.0, 13.0]), -52.0);
    }

    /// The present value is `-(fv + pmt f) / (1+r)^n`; the recorded
    /// answers are the harness case `financial-present-001`.
    #[test]
    fn present_value_discounts_both_streams() {
        // the form is `PresentValue(rate, nper, pmt[, fv[, type]])`
        let pow = 43.0f64.powi(4);
        let v = eval(Finance::PresentValue, &[42.0, 4.0, 13.0]);
        assert_eq!(v, -(13.0 * (pow - 1.0) / 42.0) / pow);
        let v = eval(Finance::PresentValue, &[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert_eq!(v, -(13.0 * 43.0 * (pow - 1.0) / 42.0) / pow);
    }

    /// A zero rate takes the straight-line branch, a fractional or
    /// non-positive period count is undefined, and a payment type other
    /// than 0 or 1 is refused (`setPmtType`, `setNper`).
    #[test]
    fn degenerate_inputs() {
        let v = eval(Finance::FutureValue, &[0.0, 4.0, 13.0]);
        assert_eq!(v, -52.0);
        let v = eval(Finance::Payment, &[0.0, 4.0, 13.0]);
        assert_eq!(v, -3.25);
        // `setNper` rounds only near an integer and rejects nothing but a
        // non-positive count.
        let v = eval(Finance::FutureValue, &[42.0, 4.5, 13.0]);
        assert!((v - (-6_939_083.166_575_921)).abs() < 1.0, "{v}");
        assert!(eval(Finance::FutureValue, &[42.0, -4.0, 13.0]).is_nan());
        assert!(eval(Finance::FutureValue, &[42.0, 4.0, 13.0, 0.0, 2.0]).is_nan());
        // A missing present value defaults to 0, a missing future value too.
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0]);
        assert!((v - (-1_058_200.0)).abs() < 1e-6, "{v}");
    }
}
