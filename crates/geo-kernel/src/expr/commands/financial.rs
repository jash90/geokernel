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
use crate::expr::parse::Parser;
use crate::expr::{Finance, Node, NONE};
use crate::expr::math::{check_integer, is_equal, is_zero};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &[
    "Rate",
    "Periods",
    "Payment",
    "FutureValue",
    "PresentValue",
];

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
    let required = |slot: Slot| -> Option<f64> {
        input(slot).filter(|v| v.is_finite())
    };
    // `setNper`: the number of periods must be a positive integer; only
    // `Periods` does not read it (it is that command's output).
    let nper = if op == Finance::Periods {
        0.0
    } else {
        let Some(v) = required(Slot::Nper) else {
            return f64::NAN;
        };
        let n = check_integer(v);
        if !(n > 0.0) {
            return f64::NAN;
        }
        n
    };
    // `setPmt`: every command but `Payment` reads it.
    let Some(pmt) = (op == Finance::Payment).then_some(0.0).or(required(Slot::Pmt)) else {
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
    let factor = |rate: f64, nper: f64| (1.0 + rate * pmt_type) * ((1.0 + rate).powf(nper) - 1.0) / rate;
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

/// Brent's root method over a bracketing interval, with the evaluation
/// limit (100, `AlgoRootNewton.MAX_ITERATIONS`), the midpoint start and
/// the accuracies of the library solver the algo calls. `None` where the
/// interval does not bracket a root or the budget runs out, which the
/// caller treats exactly as the caught exception there.
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
        return brent(&mut value, min, start, y_min, y_start);
    }
    let y_max = value(max)?;
    if y_max.abs() <= 1e-15 {
        return Some(max);
    }
    if y_start * y_max < 0.0 {
        return brent(&mut value, start, max, y_start, y_max);
    }
    None
}

/// The inner loop of the solver: `brent(x0, x1, f0, f1)`, with `oldx` and
/// `oldf` holding the third point `c` of the standard method.
fn brent(
    value: &mut impl FnMut(f64) -> Option<f64>,
    x0: f64,
    x1: f64,
    f0: f64,
    f1: f64,
) -> Option<f64> {
    let (mut a, mut fa) = (x0, f0);
    let (mut b, mut fb) = (x1, f1);
    let (mut oldx, mut oldf) = (x0, f0);
    let mut delta = b - a;
    let mut old_delta = delta;
    loop {
        // Ensure |oldf| >= |fb|: `b` is the best point.
        if oldf.abs() < fb.abs() {
            a = b;
            b = oldx;
            oldx = a;
            fa = fb;
            fb = oldf;
            oldf = fa;
        }
        let tolerance = 2.0 * 1e-14 * b.abs() + 1e-6;
        let xm = 0.5 * (oldx - b);
        // `Precision.equals(fb, 0)` is a bitwise comparison, so a NaN ends
        // the search here too.
        if xm.abs() <= tolerance || fb.to_bits() == 0.0f64.to_bits() || fb.is_nan() {
            return Some(b);
        }
        let mut bisect = true;
        if old_delta.abs() >= tolerance && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (p, q) = if a == oldx {
                (2.0 * xm * s, 1.0 - s)
            } else {
                let q = fa / oldf;
                let r = fb / oldf;
                (
                    s * (2.0 * xm * q * (q - r) - (b - a) * (r - 1.0)),
                    (q - 1.0) * (r - 1.0) * (s - 1.0),
                )
            };
            let (p, q) = if p > 0.0 { (p, -q) } else { (-p, q) };
            let previous = old_delta;
            old_delta = delta;
            // The interpolation is accepted only when it stays inside the
            // bracket and is not larger than half the step before the
            // previous one.
            if p < 1.5 * xm * q - (tolerance * q).abs() && p < 0.5 * (previous * q).abs() {
                delta = p / q;
                bisect = false;
            }
        }
        if bisect {
            delta = xm;
            old_delta = delta;
        }
        a = b;
        fa = fb;
        b += if delta.abs() > tolerance {
            delta
        } else if xm > 0.0 {
            tolerance
        } else {
            -tolerance
        };
        fb = value(b)?;
        // The bracket is `b` and `oldx`: a sign change of `fb` against
        // `oldf` moves the far point to `a`.
        if (fb > 0.0 && oldf > 0.0) || (fb <= 0.0 && oldf <= 0.0) {
            oldx = a;
            oldf = fa;
            delta = b - a;
            old_delta = delta;
        }
    }
}

/// Newton polishing: `x1 = x0 - f(x0) / f'(x0)` until the step stays within
/// the absolute accuracy (1e-6), under the same shared evaluation limit;
/// the derivative evaluations are charged to the same budget as the
/// values, as the library solver's counter does.
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

    /// `CommandsTest.cmdRate`: a savings-plan rate, two unsolvable ones and
    /// one that only converges from a guess.
    #[test]
    fn rate_upstream_values() {
        let ok = rate(&[60.0, -300.0, 10000.0]);
        assert!((ok - 0.021_750_422_875_496_778).abs() < 1e-15, "{ok}");
        assert!(rate(&[42.0, 4.0, 13.0, 1.0]).is_nan());
        let near = rate(&[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert!(
            (near - (-0.999_999_999_999_720_4)).abs() < 1e-10,
            "{near}"
        );
        assert!(rate(&[42.0, 4.0, 13.0, 0.0, 1.0, 42.0]).is_nan());
    }

    /// `CommandsTest.cmdPeriods`.
    #[test]
    fn periods_upstream_values() {
        let per = 10.0 / 100.0 / 12.0;
        let n = eval(Finance::Periods, &[per, -200.0, -400.0, 10000.0]);
        assert!((n - 39.978_946_328_199_51).abs() < 1e-9, "{n}");
        let n = eval(Finance::Periods, &[per, -200.0, -400.0, 10000.0, 1.0]);
        assert!((n - 39.702_015_879_545_73).abs() < 1e-9, "{n}");
    }

    /// `CommandsTest.cmdPayment`.
    #[test]
    fn payment_upstream_values() {
        let p = eval(Finance::Payment, &[42.0, 4.0, 13.0]);
        assert!((p - (-546.000_159_705_159_7)).abs() < 1e-9, "{p}");
        let p = eval(Finance::Payment, &[42.0, 4.0, 13.0, 1.0]);
        assert!((p - (-546.000_171_990_172)).abs() < 1e-9, "{p}");
        let p = eval(Finance::Payment, &[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert!((p - (-12.697_678_132_678_133)).abs() < 1e-9, "{p}");
    }

    /// `CommandsTest.cmdFutureValue`.
    #[test]
    fn future_value_upstream_values() {
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0]);
        assert!((v - (-1_058_200.0)).abs() < 1e-6, "{v}");
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0, 1.0]);
        assert!((v - (-4_477_001.0)).abs() < 1e-4, "{v}");
        let v = eval(Finance::FutureValue, &[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert!((v - (-45_502_600.0)).abs() < 1e-2, "{v}");
    }

    /// `CommandsTest.cmdPresentValue`.
    #[test]
    fn present_value_upstream_values() {
        let v = eval(Finance::PresentValue, &[42.0, 4.0, 13.0]);
        assert!((v - (-0.309_523_718_988_031_16)).abs() < 1e-12, "{v}");
        let v = eval(Finance::PresentValue, &[42.0, 4.0, 13.0, 1.0]);
        assert!((v - (-0.309_524_011_488_238_1)).abs() < 1e-12, "{v}");
        let v = eval(Finance::PresentValue, &[42.0, 4.0, 13.0, 0.0, 1.0]);
        assert!((v - (-13.309_519_916_485_34)).abs() < 1e-9, "{v}");
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
