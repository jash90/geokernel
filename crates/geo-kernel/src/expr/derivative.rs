// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Derivative.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFractionText.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Evaluate the rules of GeoGebra's `NoCASDerivativeCache` for tangent and
//! length. Read operand values through the ordinary evaluator and compute
//! gradients directly from the stored nodes, without expanding a derivative
//! tree. The budget includes operand evaluation and nested function calls.

use super::entry;
use super::eval::{scalar, Ctx, Env, Lists, Val};
use super::math;
use super::{children, Bin, Entry, IfMode, Node, Ty, Un, NONE};
use crate::kernel::{Construction, Def, Id, Value};
use crate::tol::STANDARD_PRECISION;
use std::f64::consts::{LN_10, LN_2};

pub(crate) struct FunctionDerivative<'a> {
    construction: &'a Construction,
    function: &'a Entry,
    budget: Lists,
}

impl<'a> FunctionDerivative<'a> {
    pub(crate) fn new(construction: &'a Construction, id: Id) -> Option<Self> {
        let (Def::Expr(def), Value::Function) = (construction.def(id), construction.value(id))
        else {
            return None;
        };
        Some(Self {
            construction,
            function: entry(construction, def),
            budget: Lists::new(Vec::new()),
        })
    }

    pub(crate) fn value(&self, x: f64) -> f64 {
        let env = Env::Value(x);
        self.context(self.function, &env).fast(self.function.root)
    }

    pub(crate) fn slope(&self, x: f64) -> f64 {
        let env = Env::Value(x);
        Walker {
            ctx: self.context(self.function, &env),
            x,
            budget: &self.budget,
        }
        .gradient(self.function.root, super::parse::MAX_DEPTH)
        .unwrap_or(f64::NAN)
    }

    fn context<'b>(&'b self, e: &'b Entry, env: &'b Env<'b, true>) -> Ctx<'b, true> {
        Ctx {
            meter: Some(&self.budget),
            ..Ctx::new(self.construction, e, env)
        }
    }
}

struct Walker<'a> {
    ctx: Ctx<'a, true>,
    /// The value of the function variable, which `sgn`'s derivative reads.
    x: f64,
    budget: &'a Lists,
}

impl Walker<'_> {
    fn node(&self, i: u32) -> Node {
        self.ctx.e.nodes[i as usize]
    }

    fn value(&self, i: u32) -> f64 {
        self.ctx.fast(i)
    }

    fn angle(&self, i: u32) -> bool {
        matches!(self.ctx.e.tys[i as usize], Ty::Num(power) if power > 0)
    }

    /// The gradient of node `i`. `None` when the budget ran out; `NaN`
    /// where GeoGebra's derivative tree is `Double.NaN`.
    fn gradient(&mut self, i: u32, depth: u32) -> Option<f64> {
        if !self.budget.charge(1) {
            return None;
        }
        let depth = depth.checked_sub(1)?;
        Some(match self.node(i) {
            Node::Num(_) | Node::MinusOne | Node::Degree | Node::Obj(_) | Node::VarY => 0.0,
            Node::Bool(_) => 0.0,
            Node::Var => 1.0,
            Node::FnName(_) | Node::Call(..) => f64::NAN,
            Node::Un(Un::Floor | Un::Ceil | Un::Round, _) => 0.0,
            // The rule is the tree `0 / x`, regardless of the argument.
            Node::Un(Un::Sgn, _) => 0.0 / self.x,
            Node::Un(op, arg) => {
                let (u, du) = (self.value(arg), self.gradient(arg, depth)?);
                self.unary(op, u, du, self.angle(arg))
            }
            Node::Bin(op, a, b) => self.binary(op, a, b, depth)?,
            Node::Apply(f, arg) => {
                // `f'` applied to the argument, times the argument's own
                // derivative (`Operation.FUNCTION` of the derivative).
                let arg_value = self.value(arg);
                let (Def::Expr(def), Value::Function) = (self.ctx.c.def(f), self.ctx.c.value(f))
                else {
                    return Some(f64::NAN);
                };
                let e = entry(self.ctx.c, def);
                let env = Env::Value(arg_value);
                let df = Walker {
                    ctx: Ctx {
                        meter: Some(self.budget),
                        ..Ctx::new(self.ctx.c, e, &env)
                    },
                    x: arg_value,
                    budget: self.budget,
                }
                .gradient(e.root, depth)?;
                df * self.gradient(arg, depth)?
            }
            Node::If(mode, cond, then, rest) => {
                // `Operation.IF` and `IF_LIST` keep the condition and
                // differentiate the branches, so evaluating the derivative
                // picks a branch exactly as `conditional` does.
                let (value, defined) = match self.ctx.full(cond).val {
                    Val::B(v, d) => (v, d),
                    _ => (false, false),
                };
                if mode == IfMode::Else && !defined {
                    f64::NAN
                } else if value {
                    self.gradient(then, depth)?
                } else if rest != NONE {
                    self.gradient(rest, depth)?
                } else {
                    f64::NAN
                }
            }
            Node::Cmd(..)
            | Node::Tuple(..)
            | Node::Polar(..)
            | Node::Vector(_)
            | Node::Inverse(_)
            | Node::Dilate(..)
            | Node::ItemObject(_)
            | Node::List(..)
            | Node::Item(_)
            | Node::ListOp(..)
            | Node::Range(..)
            | Node::Seq(..)
            | Node::Local(..)
            | Node::Apply2(..)
            | Node::Side(..)
            | Node::Relation { .. }
            | Node::Text
            | Node::TextCmd(..)
            | Node::Base { .. }
            | Node::FunctionObject(_)
            | Node::Extremum(..)
            | Node::SystemFunction(..)
            | Node::RootList(..)
            | Node::FunctionAsymptotes(..)
            | Node::Iteration { .. }
            | Node::PolynomialPart(..)
            | Node::Algebra(..)
            | Node::ObjectDefined(_)
            | Node::Defined(_)
            | Node::Filter { .. } => f64::NAN,
        })
    }

    /// `Derivative.get` of a unary operation. The auxiliary operations are
    /// evaluated as GeoGebra's derivative tree evaluates them: `sin`,
    /// `cos` and the powers take `ExpressionNode.evaluateDouble`'s fast
    /// path, the other functions the full evaluator (`scalar`).
    fn unary(&self, op: Un, u: f64, du: f64, angle: bool) -> f64 {
        let f = |op: Un| scalar(op, u, angle);
        let squared = |v: f64| math::java_pow(v, 2.0);
        match op {
            Un::Sin => u.cos() * du,
            Un::Cos => -(u.sin() * du),
            Un::Tan => squared(f(Un::Sec)) * du,
            Un::Sec => -((f(Un::Sec) * f(Un::Tan)) * du),
            Un::Csc => -((f(Un::Csc) * f(Un::Cot)) * du),
            Un::Cot => -(squared(f(Un::Csc)) * du),
            Un::Sinh => f(Un::Cosh) * du,
            Un::Cosh => -(f(Un::Sinh) * du),
            Un::Tanh => squared(f(Un::Sech)) * du,
            Un::Sech => -((f(Un::Sech) * f(Un::Tanh)) * du),
            Un::Csch => -((f(Un::Csch) * f(Un::Coth)) * du),
            Un::Coth => -(squared(f(Un::Csch)) * du),
            // `subtractR(1)` builds `1 - f`, so `asin` and `acos` take the
            // ordinary square root domain.
            Un::Asin | Un::AsinD => du / (1.0 - squared(u)).sqrt(),
            Un::Acos | Un::AcosD => -(du / (1.0 - squared(u)).sqrt()),
            Un::Atan | Un::AtanD => du / (squared(u) + 1.0),
            Un::Asinh => du / (squared(u) + 1.0).sqrt(),
            Un::Acosh => du / ((u + 1.0).sqrt() * (u - 1.0).sqrt()),
            Un::Atanh => du / (1.0 - squared(u)),
            Un::Sqrt => (du * math::java_pow(u, -0.5)) / 2.0,
            Un::Cbrt => (du / math::cbrt(squared(u))) / 3.0,
            Un::Abs => (du * u) / u.abs(),
            Un::Floor | Un::Ceil | Un::Round | Un::Sgn => unreachable!(),
            Un::Exp => du * u.exp(),
            Un::Ln => du / u,
            Un::Log10 => (du / u) / LN_10,
            Un::Log2 => (du / u) / LN_2,
            Un::Gamma => (self.digamma(u) * math::gamma(u)) * du,
            Un::Factorial => (self.digamma(u + 1.0) * math::factorial(u)) * du,
            Un::X | Un::Y | Un::Not => 0.0,
        }
    }

    fn digamma(&self, x: f64) -> f64 {
        let steps = (49.0 - x).ceil() as u64;
        if self.budget.charge(steps) {
            math::digamma(x)
        } else {
            f64::NAN
        }
    }

    /// `Derivative.get` of a binary operation.
    fn binary(&mut self, op: Bin, a: u32, b: u32, depth: u32) -> Option<f64> {
        Some(match op {
            Bin::Add => self.gradient(a, depth)? + self.gradient(b, depth)?,
            Bin::Sub => self.gradient(a, depth)? - self.gradient(b, depth)?,
            Bin::Mul => {
                let (u, v) = (self.value(a), self.value(b));
                // a constant factor comes out, GeoGebra's right one first
                match (self.number_constant(b)?, self.number_constant(a)?) {
                    (Some(v), _) => self.gradient(a, depth)? * v,
                    (None, Some(u)) => self.gradient(b, depth)? * u,
                    (None, None) => {
                        (u * self.gradient(b, depth)?) + (v * self.gradient(a, depth)?)
                    }
                }
            }
            Bin::Div => {
                if let Some(v) = self.number_constant(b)? {
                    // `divideSimplify`: dividing by 1 keeps the numerator,
                    // and a constant numerator of 0 stays 0.
                    if v == 1.0 {
                        self.gradient(a, depth)?
                    } else if v == 0.0 && !self.reads_variable(a)? {
                        0.0
                    } else {
                        self.gradient(a, depth)? / v
                    }
                } else if let Node::Bin(Bin::Pow, rl, rr) = self.node(b) {
                    // `(f / g^n)'` with a constant exponent `n`
                    if !self.reads_variable(rr)? {
                        let (r, n, u) = (self.value(rl), self.value(rr), self.value(a));
                        let dr = self.gradient(rl, depth)?;
                        let da = self.gradient(a, depth)?;
                        ((da * r) - ((dr * u) * n)) / math::java_pow(r, n + 1.0)
                    } else {
                        self.quotient(a, b, depth)?
                    }
                } else {
                    self.quotient(a, b, depth)?
                }
            }
            Bin::Pow => self.power(a, b, depth)?,
            Bin::Round2 => 0.0,
            Bin::Atan2 => {
                let (u, v) = (self.value(a), self.value(b));
                let (du, dv) = (self.gradient(a, depth)?, self.gradient(b, depth)?);
                ((du * v) - (dv * u)) / (math::java_pow(u, 2.0) + math::java_pow(v, 2.0))
            }
            Bin::LogB => {
                if let Some(base) = self.number_constant(a)? {
                    (self.gradient(b, depth)? / self.value(b)) / base.ln()
                } else {
                    // `(ln g / ln f)'` by the quotient rule
                    let (u, v) = (self.value(a), self.value(b));
                    let (du, dv) = (self.gradient(a, depth)?, self.gradient(b, depth)?);
                    let (num, den) = (v.ln(), u.ln());
                    let (dnum, dden) = (dv / v, du / u);
                    ((dnum * den) - (dden * num)) / (den * den)
                }
            }
            // `x < c` and the other comparisons have derivative 0, so that
            // a product with one keeps the other factor's derivative
            Bin::Less | Bin::Greater | Bin::LessEqual | Bin::GreaterEqual | Bin::NotEqual => 0.0,
            _ => f64::NAN,
        })
    }

    /// The general quotient rule `(f / g)' = (f' g - g' f) / g²`.
    fn quotient(&mut self, a: u32, b: u32, depth: u32) -> Option<f64> {
        let (u, v) = (self.value(a), self.value(b));
        let (du, dv) = (self.gradient(a, depth)?, self.gradient(b, depth)?);
        Some(((du * v) - (dv * u)) / math::java_pow(v, 2.0))
    }

    /// `Derivative.derivativePower`.
    fn power(&mut self, a: u32, b: u32, depth: u32) -> Option<f64> {
        let Some(exponent) = self.number_constant(b)? else {
            // `f^g`: `f^g (g' ln f + g f'/f)`
            let (u, v) = (self.value(a), self.value(b));
            let (du, dv) = (self.gradient(a, depth)?, self.gradient(b, depth)?);
            let first = dv * u.ln();
            let second = (v * du) / u;
            return Some(math::java_pow(u, v) * (first + second));
        };
        if math::is_zero(exponent) {
            return Some(0.0);
        }
        let u = self.value(a);
        let du = self.gradient(a, depth)?;
        if math::is_equal(exponent, 2.0) {
            // `f²`: `2 f f'`
            Some((u * du) * exponent)
        } else if !math::is_integer(exponent) {
            // the fraction `p/q` the decimal is read as: `f^(p/q - 1) p/q f'`.
            // The exponent is a division node, so a negative base takes the
            // real root for an odd `q` (`calculateNegPower`).
            let (p, q) = decimal_to_fraction(exponent, STANDARD_PRECISION);
            if q == 0.0 {
                return Some(f64::NAN);
            }
            let power = if u < 0.0 {
                math::neg_power_integers(u, p - q, q)
                    .unwrap_or_else(|| math::java_pow(u, (p - q) / q))
            } else {
                math::java_pow(u, (p - q) / q)
            };
            Some(((power * p) / q) * du)
        } else {
            // `f^n`: `f^(n - 1) f' n`
            Some((math::java_pow(u, exponent - 1.0) * du) * exponent)
        }
    }

    /// `isNumberValue` and `!containsDeep(fv)`: a numeric subtree that
    /// reads no variable, with its current value.
    fn number_constant(&self, i: u32) -> Option<Option<f64>> {
        Some(
            (matches!(self.ctx.e.tys[i as usize], Ty::Num(_)) && !self.reads_variable(i)?)
                .then(|| self.value(i)),
        )
    }

    /// Whether the subtree at `i` reads the function variable or the
    /// variable of a `Sequence` around it (`containsDeep`).
    fn reads_variable(&self, i: u32) -> Option<bool> {
        let mut stack = vec![i];
        while let Some(i) = stack.pop() {
            if !self.budget.charge(1) {
                return None;
            }
            match self.node(i) {
                Node::Var | Node::VarY | Node::Local(..) => return Some(true),
                node => stack.extend(children(&node)),
            }
        }
        Some(false)
    }
}

/// `AlgoFractionText.decimalToFraction(decimal, accuracyFactor)`: a
/// continued fraction `numerator / denominator` within `accuracy` of
/// `decimal`.
pub(crate) fn decimal_to_fraction(decimal: f64, accuracy: f64) -> (f64, f64) {
    if decimal.is_nan() {
        return (0.0, 0.0);
    }
    if decimal == f64::INFINITY {
        return (1.0, 0.0);
    }
    if decimal == f64::NEG_INFINITY {
        return (-1.0, 0.0);
    }
    let sign = if decimal < 0.0 { -1.0 } else { 1.0 };
    let abs = decimal.abs();
    if (abs - abs.floor()).abs() < accuracy {
        // an exact integer, including 0
        return (abs * sign, 1.0);
    }
    if abs < 1e-19 {
        return (sign, 9999999999999999999.0);
    }
    if abs > 1e19 {
        return (9999999999999999999.0 * sign, 1.0);
    }
    let mut z = abs;
    let mut previous = 0.0;
    let mut denominator = 1.0;
    // There are fewer than 64 continued-fraction refinements before the
    // denominator exceeds the precision of a double.
    for _ in 0..64 {
        z = 1.0 / (z - z.floor());
        let scratch = denominator;
        denominator = denominator * z.floor() + previous;
        previous = scratch;
        let numerator = (abs * denominator + 0.5).floor();
        // `MyDouble.exactEqual(z, floor(z))` ends the loop
        if !((abs - numerator / denominator).abs() > accuracy && z != z.floor()) {
            return (sign * numerator, denominator);
        }
    }
    (0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{eval::with_work_limit, work_done, MAX_OVERRUN};

    #[test]
    fn derivative_samples_and_nested_functions_share_one_work_budget() {
        let c = crate::script::load("f(x) = x^3 + sin(x)\ng(x) = f(f(x))").unwrap();
        let id = c.id("g").unwrap();
        let before = work_done();
        with_work_limit(200, || {
            let derivative = FunctionDerivative::new(&c, id).unwrap();
            assert!(derivative.slope(1.0).is_finite());
            assert!((0..100).any(|_| derivative.slope(1.0).is_nan()));
            assert!(derivative.value(1.0).is_nan());
            assert!(derivative.slope(1.0).is_nan());
        });
        assert!(work_done() - before <= 200 + MAX_OVERRUN);
    }
}
