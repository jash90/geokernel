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
//! family of `AlgoFinancial`. `Rate` solves the fundamental equation
//! `pmt (1 + rate pmtType) ((1 + rate)^n - 1) / rate + pv (1 + rate)^n + fv = 0`
//! by bisection followed by a Newton refinement. The expansion of the
//! bracket around a supplied guess or from the default seed follows the
//! EUPL command. The numerical iterations follow the descriptions in
//! NIST DLMF sections 3.8(ii) and 3.8(iii).

use crate::expr::eval::{Ctx, Val};
use crate::expr::math::{check_integer, is_equal, is_zero, java_pow};
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
/// In a metered expression, each evaluation of the rate equation or its
/// derivative is charged to the expression's budget.
pub(crate) fn value<const M: bool>(ctx: &Ctx<'_, M>, op: Finance, args: &[u32; 6]) -> Val {
    let mut values = [f64::NAN; 6];
    let count = args.iter().take_while(|&&a| a != NONE).count();
    for (slot, arg) in values.iter_mut().zip(args).take(count) {
        *slot = ctx.full_number(*arg);
    }
    let charge = &mut || ctx.stop(8);
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
    // An infinite input retains its value in the expression evaluator
    // even when the public numeric row shows it as undefined.
    let required = |slot: Slot| -> Option<f64> { input(slot).filter(|v| !v.is_nan()) };
    // `setNper`: the number of periods must be positive and is snapped
    // near integers. `Periods` returns this count instead of reading it.
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
            Some(v) if !v.is_nan() => Some(v),
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
            Some(v) if !v.is_nan() => Some(v),
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
    let compound = |rate: f64, nper: f64| java_pow(1.0 + rate, nper);
    let factor =
        |rate: f64, nper: f64| (1.0 + rate * pmt_type) * (compound(rate, nper) - 1.0) / rate;
    match op {
        Finance::Rate => {
            // The search brackets around the guess only when the form
            // supplied one; `setGuess` refuses an undefined guess.
            let guess = match input(Slot::Guess) {
                None => None,
                Some(v) if !v.is_nan() => Some(v),
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
                (-fv - pv * compound(rate, nper)) / factor(rate, nper)
            }
        }
        Finance::PresentValue => {
            let Some(rate) = required(Slot::Rate) else {
                return f64::NAN;
            };
            if rate == 0.0 {
                -pmt * nper - fv
            } else {
                (-fv - pmt * factor(rate, nper)) / compound(rate, nper)
            }
        }
        Finance::FutureValue => {
            let Some(rate) = required(Slot::Rate) else {
                return f64::NAN;
            };
            if rate == 0.0 {
                -pmt * nper - pv
            } else {
                -pmt * factor(rate, nper) - pv * compound(rate, nper)
            }
        }
    }
}

/// Bracket the root as in `AlgoFinancial.computeRate`. If the bracket
/// has no sign change, try the command's Newton seed. The open iteration
/// can start at a supplied guess outside the bracketing interval.
fn compute_rate(
    nper: f64,
    pv: f64,
    fv: f64,
    pmt: f64,
    pmt_type: f64,
    guess: Option<f64>,
    charge: &mut dyn FnMut() -> bool,
) -> Option<f64> {
    let equation = RateFunction {
        n: nper,
        pv,
        fv,
        pmt,
        pmt_type,
    };
    let value = |x: f64, charge: &mut dyn FnMut() -> bool| {
        if charge() {
            None
        } else {
            Some(equation.value(x))
        }
    };
    let (mut lower, mut upper);
    if let Some(guess) = guess {
        let mut radius = 0.001;
        lower = (guess - radius).max(-1.0);
        upper = (guess + radius).min(1.0);
        while radius < 1.0 && value(lower, charge)?.signum() == value(upper, charge)?.signum() {
            radius *= 2.0;
            lower = (guess - radius).max(0.0);
            upper = (guess + radius).min(1.0);
        }
    } else {
        lower = 0.0;
        upper = 0.001;
        let sign = value(lower, charge)?.signum();
        while upper < 1.0 && value(upper, charge)?.signum() == sign {
            upper += 0.05;
        }
    }

    let mut seed = bisect(lower, upper, |x| value(x, charge)).unwrap_or(guess.unwrap_or(0.1));
    if is_equal(seed, 1.0) || !seed.is_finite() {
        seed = 0.1;
    }
    let polished = newton_polish(seed, &equation, charge)?;
    let seed_residual = value(seed, charge)?.abs();
    let polished_residual = value(polished, charge)?.abs();
    Some(if polished_residual < seed_residual {
        polished
    } else {
        seed
    })
}

/// The cash-flow equation with the annuity's limit at zero restored.
struct RateFunction {
    n: f64,
    pv: f64,
    fv: f64,
    pmt: f64,
    pmt_type: f64,
}

impl RateFunction {
    fn value(&self, x: f64) -> f64 {
        let power = (1.0 + x).powf(self.n);
        let annuity = if x == 0.0 {
            self.n
        } else if x.abs() < 0.01 {
            // Subtracting 1 from the compounded value loses precision
            // around zero. Evaluate that difference directly.
            (x.ln_1p() * self.n).exp_m1() / x
        } else {
            (power - 1.0) / x
        };
        self.fv + self.pmt * (1.0 + x * self.pmt_type) * annuity + self.pv * power
    }

    fn derivative(&self, x: f64) -> f64 {
        let (pv, pmt, kind, n) = (self.pv, self.pmt, self.pmt_type, self.n);
        let factor = 1.0 + x;
        let power_derivative = n * factor.powf(n - 1.0);
        let (annuity, annuity_derivative) = if (x * n).abs() < 0.001 {
            // The first terms of the binomial series for
            // ((1+x)^n - 1)/x and its derivative at x = 0.
            let second = n * (n - 1.0) / 2.0;
            let third = second * (n - 2.0) / 3.0;
            let fourth = third * (n - 3.0) / 4.0;
            (
                n + x * (second + x * (third + x * fourth)),
                second + x * (2.0 * third + x * 3.0 * fourth),
            )
        } else {
            let annuity = (factor.powf(n) - 1.0) / x;
            (annuity, (power_derivative - annuity) / x)
        };
        pmt * (kind * annuity + (1.0 + x * kind) * annuity_derivative) + pv * power_derivative
    }
}

/// Bisection (DLMF 3.8(iii)) maintains a sign change between two endpoints.
/// Two dozen halvings of the initial interval are ample for the Newton
/// seed. The small absolute floor also handles roots close to zero.
fn bisect(
    mut lower: f64,
    mut upper: f64,
    mut value: impl FnMut(f64) -> Option<f64>,
) -> Option<f64> {
    if lower >= upper {
        return None;
    }
    let left = value(lower)?;
    let right = value(upper)?;
    if left == 0.0 {
        return Some(lower);
    }
    if right == 0.0 {
        return Some(upper);
    }
    if left.is_nan() || right.is_nan() || left.is_sign_negative() == right.is_sign_negative() {
        return None;
    }

    for _ in 0..32 {
        let mid = lower.midpoint(upper);
        let y = value(mid)?;
        if y.is_nan() {
            return None;
        }
        if y == 0.0 || upper - lower < 2.5e-8 {
            return Some(mid);
        }
        if y.is_sign_negative() == left.is_sign_negative() {
            lower = mid;
        } else {
            upper = mid;
        }
    }
    Some(lower.midpoint(upper))
}

/// Newton's tangent rule (DLMF 3.8(ii)) can leave the initial search
/// interval. Limit the number of steps and require a representable trial.
fn newton_polish(
    mut x: f64,
    equation: &RateFunction,
    charge: &mut dyn FnMut() -> bool,
) -> Option<f64> {
    for _ in 0..64 {
        if charge() {
            return None;
        }
        let residual = equation.value(x);
        if residual == 0.0 {
            return Some(x);
        }
        if charge() {
            return None;
        }
        let change = residual / equation.derivative(x);
        let next = x - change;
        if !next.is_finite() {
            return None;
        }
        if change.abs() <= 2.5e-7 * x.abs().max(1.0) {
            return Some(next);
        }
        x = next;
    }
    None
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

    #[test]
    fn refining_an_established_rate_bracket_uses_the_budget() {
        let args = [12.0, -5.0, 55.0, 0.0, 0.0, 0.013];
        assert!(rate(&args).is_finite());
        let mut remaining = 5usize;
        let mut stop = || {
            remaining = remaining.saturating_sub(1);
            remaining == 0
        };
        assert!(evaluate(Finance::Rate, &args, &mut stop).is_nan());
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

    /// A zero rate takes the straight-line branch, a non-positive period
    /// count is undefined, and payment type must be 0 or 1.
    #[test]
    fn degenerate_inputs() {
        let v = eval(Finance::FutureValue, &[0.0, 4.0, 13.0]);
        assert_eq!(v, -52.0);
        let v = eval(Finance::Payment, &[0.0, 4.0, 13.0]);
        assert_eq!(v, -3.25);
        // `setNper` rounds only near an integer and rejects nothing but a
        // non-positive count.
        let v = eval(Finance::FutureValue, &[42.0, 4.5, 13.0]);
        assert_eq!(v, -13.0 * ((43.0_f64.powf(4.5) - 1.0) / 42.0));
        assert!(eval(Finance::FutureValue, &[42.0, -4.0, 13.0]).is_nan());
        assert!(eval(Finance::FutureValue, &[42.0, 4.0, 13.0, 0.0, 2.0]).is_nan());
    }

    #[test]
    fn infinite_payments_propagate_their_sign() {
        for payment in [f64::INFINITY, f64::NEG_INFINITY] {
            let value = eval(Finance::FutureValue, &[0.0, 4.0, payment, 13.0]);
            assert_eq!(value, -payment);
        }
        assert!(eval(Finance::FutureValue, &[0.0, 4.0, f64::NAN, 13.0]).is_nan());
    }
}
