// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Function.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyVecNode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec2D.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/Operation.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/DoubleOperation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MySpecialDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Evaluation of an expression tree.
//!
//! GeoGebra evaluates a tree in two ways and the results differ in the last
//! digits and at special values, so both are kept:
//!
//! - *full* (`ExpressionNode.evaluate`), used for dependent numbers, points
//!   and vectors: the `MyDouble` operations with their special cases
//!   (`sin` snapping to zero, `tan` undefined at `π/2`, ...);
//! - *fast* (`ExpressionNode.evaluateDouble`), used for the coordinates of
//!   `(x, y)` and the parts of a fractional exponent: plain `+ - * /`,
//!   `Math.pow`, `Math.sin`, `Math.cos` and `Math.sqrt`; every other
//!   operation falls back to the full evaluation of its subtree.
//!
//! The full evaluation also does GeoGebra's exact decimal arithmetic
//! (`DoubleOperation`): `+ - * /` and whole powers of two operands that
//! both have a decimal (see [`Ex`]) are computed on decimals, so
//! `0.1 + 0.2` is `0.3`. The fast evaluation never does.
//!
//! A function is sampled by the fast evaluation with `x` set to a number
//! (`Function.value`). Inside a function, an applied function is expanded
//! (`AlgoDependentFunction.expandFunctionDerivativeNodes`): its `x` stands
//! for the argument's subtree, evaluated in place. Outside a function,
//! `f(a)` evaluates `a` and samples `f` there.

use super::decimal::Dec;
use super::math::{self, PI_180};
use super::twonum;
use super::{Bin, Entry, Node, Raw, Ty, Un};
use crate::algo::intersect::Hint;
use crate::kernel::{Construction, Def, Id, Kind, Value};
use std::borrow::Cow;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
enum Val {
    N(f64),
    V(f64, f64),
}

impl Val {
    fn num(self) -> f64 {
        match self {
            Val::N(v) => v,
            Val::V(..) => f64::NAN,
        }
    }
}

/// A result of the full evaluation with what GeoGebra's value object knows
/// besides its value: whether it is a `MySpecialDouble` (`special`), which
/// keeps a decimal, and that decimal (`toDecimal`). An object operand
/// (`obj`) is a `GeoNumeric`: as a right operand its decimal is its
/// `exactValue`; as a left operand GeoGebra takes `getNumber()`, which keeps
/// the decimal only for a free number that was never changed (`special`).
#[derive(Clone)]
struct Ex {
    val: Val,
    dec: Option<Exact>,
    special: bool,
    obj: bool,
}

impl Ex {
    fn plain(val: Val) -> Ex {
        Ex {
            val,
            dec: None,
            special: false,
            obj: false,
        }
    }

    /// `set(double)` on a copy of an operand: the class stays, the decimal
    /// goes.
    fn kept(special: bool, v: f64) -> Ex {
        Ex {
            val: Val::N(v),
            dec: None,
            special,
            obj: false,
        }
    }

    /// `MySpecialDouble.set(BigDecimal)`; an infinite value has no decimal.
    fn exact(d: Dec) -> Ex {
        let v = d.to_f64();
        Ex {
            val: Val::N(v),
            dec: v.is_finite().then_some(Exact::Dec(d)),
            special: true,
            obj: false,
        }
    }

    /// `getNumber()`: the class and decimal of the copy an operation works on.
    fn left(&self) -> (bool, Option<&Exact>) {
        if self.obj && !self.special {
            (false, None)
        } else {
            (self.special, self.dec.as_ref())
        }
    }

    fn num(&self) -> f64 {
        self.val.num()
    }
}

/// The exact decimal of a number: a decimal, or the exact decimal of its
/// double, which is only written out when an exact operation reads it (a
/// double such as `12.35` has about 50 digits).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Exact {
    Dec(Dec),
    Double,
}

impl Exact {
    /// The decimal of a number whose double is `v`.
    pub(super) fn get(&self, v: f64) -> Option<Cow<'_, Dec>> {
        match self {
            Exact::Dec(d) => Some(Cow::Borrowed(d)),
            Exact::Double => Dec::from_f64(v).map(Cow::Owned),
        }
    }
}

/// `MySpecialDouble.setPrecise`: a `MySpecialDouble` takes the exact
/// decimal of a finite result.
fn precise(special: bool, v: f64) -> Ex {
    if special && v.is_finite() && v != 0.0 {
        return Ex {
            val: Val::N(v),
            dec: Some(Exact::Double),
            special: true,
            obj: false,
        };
    }
    match special.then(|| Dec::from_f64(v)).flatten() {
        Some(d) => Ex::exact(d),
        None => Ex::kept(special, v),
    }
}

/// The decimal of `°` (`MySpecialDouble.DEGREE`).
fn degree() -> &'static Dec {
    static DEGREE: OnceLock<Dec> = OnceLock::new();
    DEGREE.get_or_init(|| Dec::parse("0.017453292519943295769").expect("a decimal"))
}

/// What the variable `x` stands for.
enum Env<'a> {
    None,
    Value(f64),
    /// A subtree of the caller: the argument of an expanded function.
    Arg(&'a Ctx<'a>, u32),
}

struct Ctx<'a> {
    c: &'a Construction,
    e: &'a Entry,
    env: &'a Env<'a>,
}

/// Evaluates an entry's tree (full evaluation), with the exact decimal of
/// a number result (`toDecimal`).
pub(super) fn root(c: &Construction, e: &Entry) -> (Raw, Option<Exact>) {
    let ctx = Ctx {
        c,
        e,
        env: &Env::None,
    };
    let x = ctx.full(e.root);
    match x.val {
        Val::N(v) => (Raw::Num(v), x.dec),
        Val::V(x, y) => (Raw::Vec(x, y), None),
    }
}

/// `Function.value`: a function entry at `x`.
pub(super) fn sample(c: &Construction, e: &Entry, x: f64) -> f64 {
    let ctx = Ctx {
        c,
        e,
        env: &Env::Value(x),
    };
    ctx.fast(e.root)
}

/// The tree of the function object `f`, when it is defined.
fn function_entry(c: &Construction, f: Id) -> Option<&Entry> {
    match (c.def(f), c.value(f)) {
        (Def::Expr(d), Value::Function) => Some(super::entry(c, d)),
        _ => None,
    }
}

impl Ctx<'_> {
    fn node(&self, i: u32) -> Node {
        self.e.nodes[i as usize]
    }

    fn is_vec(&self, i: u32) -> bool {
        self.e.tys[i as usize] == Ty::Vec
    }

    fn is_angle(&self, i: u32) -> bool {
        self.e.tys[i as usize] == Ty::Num(1)
    }

    fn full(&self, i: u32) -> Ex {
        use Val::{N, V};
        match self.node(i) {
            Node::Num(v) => {
                let lit = &self.e.lits[i as usize];
                Ex {
                    val: N(v),
                    dec: lit.dec.clone().map(Exact::Dec),
                    special: lit.special,
                    obj: false,
                }
            }
            Node::MinusOne => Ex::plain(N(-1.0)),
            Node::Degree => Ex {
                val: N(PI_180),
                dec: Some(Exact::Dec(degree().clone())),
                special: true,
                obj: false,
            },
            Node::Var => match *self.env {
                Env::None => Ex::plain(N(f64::NAN)),
                Env::Value(x) => Ex::plain(N(x)),
                // `AlgoDependentFunction.substituteFunction` puts a copy of
                // the argument in place of `x`: it keeps its decimal
                Env::Arg(caller, a) => caller.full(a),
            },
            Node::Obj(id) => object_ex(self.c, id),
            Node::Cmd(def, hint) => Ex {
                obj: true,
                ..Ex::plain(command(self.c, def, hint))
            },
            // `MyVecNode.getVector`: coordinates by `evaluateDouble`.
            Node::Tuple(a, b) => Ex::plain(V(self.fast(a), self.fast(b))),
            Node::Polar(r, phi) => {
                let (r, phi) = (self.fast(r), self.fast(phi));
                Ex::plain(V(r * phi.cos(), r * phi.sin()))
            }
            Node::Un(op, a) => self.unary(op, a),
            Node::Bin(Bin::Pow, a, b) => self.power(a, b),
            Node::Bin(Bin::Round2, a, b) => self.round2(a, b),
            Node::Bin(op, a, b) => {
                let (l, r) = (self.full(a), self.full(b));
                match (l.val, r.val) {
                    (N(_), N(_)) if matches!(op, Bin::Add | Bin::Sub | Bin::Mul | Bin::Div) => {
                        arithmetic(op, &l, &r)
                    }
                    // the result of a command is a helper `GeoNumeric`
                    _ if op.is_command() => Ex {
                        obj: true,
                        ..Ex::plain(binary(op, l.val, r.val, false))
                    },
                    (N(_), N(_)) => Ex::kept(l.left().0, binary(op, l.val, r.val, false).num()),
                    _ => Ex::plain(binary(op, l.val, r.val, self.is_angle(a))),
                }
            }
            Node::Apply(f, a) => self.apply_full(f, a),
            Node::FnName(_) => Ex::plain(N(f64::NAN)),
        }
    }

    /// `f(a)`: expanded inside a function, in the current mode; elsewhere
    /// `a` is evaluated and `f` sampled there (`GeoFunction.value`).
    fn apply(&self, f: Id, a: u32, fast: bool) -> f64 {
        let Some(e) = function_entry(self.c, f) else {
            return f64::NAN;
        };
        if self.e.function {
            let env = Env::Arg(self, a);
            let inner = Ctx {
                c: self.c,
                e,
                env: &env,
            };
            if fast {
                inner.fast(e.root)
            } else {
                inner.full_number(e.root)
            }
        } else {
            sample(self.c, e, self.full_number(a))
        }
    }

    /// `f(a)` in the full evaluation: inside a function the expanded tree
    /// keeps its decimals; elsewhere `f` is sampled at the value of `a`.
    fn apply_full(&self, f: Id, a: u32) -> Ex {
        if !self.e.function {
            return Ex::plain(Val::N(self.apply(f, a, false)));
        }
        let Some(e) = function_entry(self.c, f) else {
            return Ex::plain(Val::N(f64::NAN));
        };
        let env = Env::Arg(self, a);
        Ctx {
            c: self.c,
            e,
            env: &env,
        }
        .full(e.root)
    }

    fn full_number(&self, i: u32) -> f64 {
        self.full(i).num()
    }

    /// `ExpressionNode.evaluateDouble`.
    fn fast(&self, i: u32) -> f64 {
        match self.node(i) {
            Node::Num(v) => v,
            Node::MinusOne => -1.0,
            Node::Degree => PI_180,
            Node::Bin(Bin::Add, a, b) => self.fast(a) + self.fast(b),
            Node::Bin(Bin::Sub, a, b) => self.fast(a) - self.fast(b),
            Node::Bin(Bin::Div, a, b) => self.fast(a) / self.fast(b),
            Node::Bin(Bin::Mul, a, b) if !self.is_vec(a) && !self.is_vec(b) => {
                // `evaluateMultiplyDouble`: NaN goes the full way
                let mut v = self.fast(a);
                if !v.is_nan() {
                    v *= self.fast(b);
                }
                if v.is_nan() {
                    self.full_number(i)
                } else {
                    v
                }
            }
            Node::Bin(Bin::Pow, a, b) if !self.is_vec(a) => {
                // `evaluatePowerDouble`
                let base = self.fast(a);
                if base < 0.0 {
                    if let Some(v) = self.neg_power(base, b) {
                        return v;
                    }
                }
                math::java_pow(base, self.fast(b))
            }
            Node::Un(Un::Sin, a) => self.fast(a).sin(),
            Node::Un(Un::Cos, a) => self.fast(a).cos(),
            Node::Un(Un::Sqrt, a) => self.fast(a).sqrt(),
            Node::Var => match *self.env {
                Env::None => f64::NAN,
                Env::Value(x) => x,
                Env::Arg(caller, a) => caller.fast(a),
            },
            Node::Apply(f, a) if self.e.function => self.apply(f, a, true),
            Node::Obj(id) => object(self.c, id).num(),
            _ => self.full_number(i),
        }
    }

    /// `Operation.ROUND2`: digits that evaluate to `NaN` fall back to
    /// rounding to an integer, unless they are an object.
    fn round2(&self, a: u32, b: u32) -> Ex {
        let (x, digits) = (self.full(a), self.full(b));
        let object = matches!(self.node(b), Node::Obj(_) | Node::Cmd(..));
        match (x.val, digits.val) {
            (Val::N(v), Val::N(d)) => {
                let d = if d.is_nan() && !object { 0.0 } else { d };
                let (special, dec) = x.left();
                round(special, dec, v, d, self.is_angle(a))
            }
            (l, r) => Ex::plain(binary(Bin::Round2, l, r, self.is_angle(a))),
        }
    }

    /// `ExpressionNodeEvaluator.handlePower` and `MyDouble.pow`: a whole
    /// exponent from 0 to below 10^6 is `DoubleOperation.INT_POWER`.
    fn power(&self, a: u32, b: u32) -> Ex {
        let (l, r) = (self.full(a), self.full(b));
        match (l.val, r.val) {
            (Val::N(base), Val::N(e)) => {
                let (special, dec) = l.left();
                if math::is_euler(base) {
                    // `exponent.getNumber().exp()`
                    return Ex::kept(r.left().0, e.exp());
                }
                if base < 0.0 {
                    if let Some(v) = self.neg_power(base, b) {
                        return Ex::kept(special, v);
                    }
                }
                if (0.0..1e6).contains(&e) && math::is_integer(e) && base.is_finite() {
                    if let (Some(x), Some(_)) = (dec, r.dec.as_ref()) {
                        let d = x.get(base).and_then(|x| x.pow(math::java_round(e) as u32));
                        if let Some(d) = d {
                            return Ex::exact(d);
                        }
                    }
                }
                Ex::kept(special, math::my_pow(base, e))
            }
            // `v^2` is the scalar product of `v` with itself
            (Val::V(x, y), Val::N(e)) => {
                Ex::plain(Val::N(if e == 2.0 { x * x + y * y } else { f64::NAN }))
            }
            _ => Ex::plain(Val::N(f64::NAN)),
        }
    }

    /// `ExpressionNode.calculateNegPower`: a negative base with an exponent
    /// written as a fraction `a/b` (or `-(a/b)`) of integers has a real
    /// odd root. `None` when the exponent has another shape.
    fn neg_power(&self, base: f64, exponent: u32) -> Option<f64> {
        match self.node(exponent) {
            // an expanded function: the shape is the argument's
            Node::Var => match *self.env {
                Env::Arg(caller, a) => caller.neg_power(base, a),
                _ => None,
            },
            Node::Bin(Bin::Div, a, b) => Some(self.neg_power_fraction(base, a, b, exponent)),
            Node::Bin(Bin::Mul, m, d) if matches!(self.node(m), Node::MinusOne) => {
                match self.node(d) {
                    Node::Bin(Bin::Div, a, b) => Some(1.0 / self.neg_power_fraction(base, a, b, d)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `ExpressionNodeEvaluator.negPower`.
    fn neg_power_fraction(&self, base: f64, a: u32, b: u32, whole: u32) -> f64 {
        let n = self.fast(a);
        if math::is_equal(n, math::java_round(n) as f64) {
            if let Some(v) = math::neg_power_integers(base, n, self.fast(b)) {
                return v;
            }
        }
        math::my_pow(base, self.fast(whole))
    }

    /// The one-argument operations of `Operation` on a number, and `x()`,
    /// `y()` and `abs()` of a point or vector.
    /// The result keeps the class of the operand's `getNumber()` copy;
    /// `floor`, `ceil` and `round` of a `MySpecialDouble` are exact
    /// (`setPrecise`, `doRound`).
    fn unary(&self, op: Un, a: u32) -> Ex {
        let operand = self.full(a);
        let x = match operand.val {
            Val::N(x) => x,
            Val::V(x, y) => {
                return Ex::plain(Val::N(match op {
                    Un::X => x,
                    Un::Y => y,
                    Un::Abs => complex_abs(x, y),
                    _ => f64::NAN,
                }))
            }
        };
        let deg = self.is_angle(a);
        let (special, dec) = operand.left();
        let v = match op {
            Un::Floor | Un::Ceil if !deg => {
                let v = if op == Un::Floor {
                    math::floor(x, false)
                } else {
                    math::ceil(x, false)
                };
                return precise(special, v);
            }
            Un::Round => return round(special, dec, x, 0.0, deg),
            _ => scalar(op, x, deg),
        };
        Ex::kept(special, v)
    }
}

/// A one-argument operation on a number (`deg`: the argument is an angle).
pub(super) fn scalar(op: Un, x: f64, deg: bool) -> f64 {
    match op {
        Un::Factorial => math::factorial(x),
        Un::Sin => math::sin(x),
        Un::Cos => math::cos(x),
        Un::Tan => math::tan(x),
        Un::Csc => math::csc(x),
        Un::Sec => math::sec(x),
        Un::Cot => math::cot(x),
        Un::Sinh => x.sinh(),
        Un::Cosh => x.cosh(),
        Un::Tanh => x.tanh(),
        Un::Csch => math::csch(x),
        Un::Sech => 1.0 / x.cosh(),
        Un::Coth => 1.0 / x.tanh(),
        Un::Asin | Un::AsinD => math::asin(x),
        Un::Acos | Un::AcosD => math::acos(x),
        Un::Atan | Un::AtanD => x.atan(),
        Un::Asinh => x.asinh(),
        Un::Acosh => x.acosh(),
        Un::Atanh => x.atanh(),
        Un::Sqrt => x.sqrt(),
        Un::Cbrt => math::cbrt(x),
        Un::Abs => x.abs(),
        Un::Sgn => math::sgn(x),
        Un::Floor => math::floor(x, deg),
        Un::Ceil => math::ceil(x, deg),
        Un::Round => math::round(x, 0.0, deg),
        Un::Exp => x.exp(),
        Un::Ln => x.ln(),
        Un::Log10 => math::log10(x),
        Un::Log2 => math::log2(x),
        Un::Gamma => math::gamma(x),
        Un::X | Un::Y => f64::NAN,
    }
}

/// `DoubleOperation.apply` for `+ - * /` (with the checks of `MyDouble.mult`
/// and `MyDouble.div` first): on decimals when both operands have one and
/// neither is infinite, else on doubles.
fn arithmetic(op: Bin, l: &Ex, r: &Ex) -> Ex {
    let (special, ldec) = l.left();
    let (a, b) = (l.num(), r.num());
    let double = match op {
        Bin::Add => a + b,
        Bin::Sub => a - b,
        Bin::Mul => a * b,
        _ => a / b,
    };
    let skip = match op {
        // `? * anything = ?`
        Bin::Mul => a.is_nan() || b.is_nan(),
        // a division by zero is an imprecise double
        Bin::Div => b == 0.0,
        _ => false,
    };
    if !skip && a.is_finite() && b.is_finite() {
        if let (Some(x), Some(y)) = (ldec, r.dec.as_ref()) {
            let (Some(x), Some(y)) = (x.get(a), y.get(b)) else {
                return Ex::kept(special, double);
            };
            let (x, y) = (x.as_ref(), y.as_ref());
            let d = match op {
                Bin::Add => x.add(y),
                Bin::Sub => x.sub(y),
                Bin::Mul => x.mul(y),
                _ => x.div(y),
            };
            if let Some(d) = d {
                return Ex::exact(d);
            }
        }
    }
    Ex::kept(special, double)
}

/// `MyDouble.round(digits)` and `doRound`: digits that are not whole give
/// `NaN`; an angle in degrees rounds its degrees; a `MySpecialDouble` with a
/// decimal uses `setScale(digits, HALF_UP)`; otherwise the double is
/// rounded and, for a `MySpecialDouble`, made exact (`setPrecise`).
fn round(special: bool, dec: Option<&Exact>, x: f64, digits: f64, deg: bool) -> Ex {
    let v = math::round(x, digits, deg);
    if deg || !math::is_integer(digits) {
        return Ex::kept(special, v);
    }
    if let Some(d) = dec.filter(|_| special).and_then(|d| d.get(x)) {
        if let Some(r) = d.set_scale_half_up(i64::from(digits as i32)) {
            return Ex::exact(r);
        }
    }
    precise(special, v)
}

/// An object as an operand, with its decimal (see [`Ex`]).
fn object_ex(c: &Construction, id: Id) -> Ex {
    let val = object(c, id);
    if c.kind(id) != Kind::Number {
        return Ex::plain(val);
    }
    let (dec, special) = match c.def(id) {
        Def::Expr(d) => (super::entry(c, d).dec.clone(), false),
        Def::Number(_) => {
            let d = super::free_decimal(c, id).cloned().map(Exact::Dec);
            let special = d.is_some();
            (d, special)
        }
        // `GeoAngle.getNumber()` is a plain number
        Def::Angle(_) => (super::free_decimal(c, id).cloned().map(Exact::Dec), false),
        _ => (None, false),
    };
    Ex {
        val,
        dec,
        special,
        obj: true,
    }
}

/// `handlePlus`, `handleMinus`, `handleMult`, `handleDivide` and the
/// two-argument functions. A number added to a vector is added to both
/// coordinates; the product of two vectors is their scalar product.
fn binary(op: Bin, l: Val, r: Val, left_angle: bool) -> Val {
    use Val::{N, V};
    match (op, l, r) {
        (Bin::Add, N(a), N(b)) => N(a + b),
        (Bin::Add, V(a, b), V(c, d)) => V(a + c, b + d),
        (Bin::Add, V(a, b), N(n)) => V(a + n, b + n),
        (Bin::Add, N(n), V(a, b)) => V(a + n, b + n),
        (Bin::Sub, N(a), N(b)) => N(a - b),
        (Bin::Sub, V(a, b), V(c, d)) => V(a - c, b - d),
        (Bin::Sub, V(a, b), N(n)) => V(a - n, b - n),
        (Bin::Sub, N(n), V(a, b)) => V(n - a, n - b),
        (Bin::Mul, N(a), N(b)) => N(a * b),
        (Bin::Mul, N(n), V(a, b)) | (Bin::Mul, V(a, b), N(n)) => V(a * n, b * n),
        (Bin::Mul, V(a, b), V(c, d)) => N(a * c + b * d),
        (Bin::Div, N(a), N(b)) => N(a / b),
        (Bin::Div, V(a, b), N(n)) => V(a / n, b / n),
        (Bin::Atan2, N(y), N(x)) => N(y.atan2(x)),
        // `MyDouble.log(base)`
        (Bin::LogB, N(base), N(x)) => N(x.ln() / base.ln()),
        (Bin::Round2, N(x), N(digits)) => N(math::round(x, digits, left_angle)),
        (Bin::Mod, N(a), N(b)) => N(twonum::modulo(a, b)),
        (Bin::IntDiv, N(a), N(b)) => N(twonum::div(a, b)),
        (Bin::Gcd, N(a), N(b)) => N(twonum::gcd_of(a, b)),
        (Bin::Lcm, N(a), N(b)) => N(twonum::lcm_of(a, b)),
        (Bin::Max, N(a), N(b)) => N(twonum::max(a, b)),
        (Bin::Min, N(a), N(b)) => N(twonum::min(a, b)),
        _ => N(f64::NAN),
    }
}

/// An object as an operand: an expression object gives its raw result
/// (infinities kept); an undefined object gives `NaN`.
fn object(c: &Construction, id: Id) -> Val {
    let nan = f64::NAN;
    match (c.kind(id), c.def(id), c.value(id)) {
        (Kind::Number | Kind::Vector, Def::Expr(d), _) => match super::raw(c, d) {
            Raw::Num(v) => Val::N(v),
            Raw::Vec(x, y) => Val::V(x, y),
        },
        (Kind::Number, _, Value::Number(v)) => Val::N(v),
        (Kind::Number, _, _) => Val::N(nan),
        (Kind::Point, _, Value::Point { x, y }) | (Kind::Vector, _, Value::Vector { x, y }) => {
            Val::V(x, y)
        }
        (Kind::Point | Kind::Vector, _, _) => Val::V(nan, nan),
        _ => Val::N(nan),
    }
}

/// A command computed in place.
fn command(c: &Construction, def: Def, hint: Hint) -> Val {
    let nan = f64::NAN;
    match c.command_value(def, hint) {
        Some(Value::Number(v)) => Val::N(v),
        Some(Value::Point { x, y } | Value::Vector { x, y }) => Val::V(x, y),
        _ if def.kind() == Kind::Number => Val::N(nan),
        _ => Val::V(nan, nan),
    }
}

/// `GeoVec2D.complexAbs`, the modulus of `x + iy` (Apache Commons Math's
/// `Complex.abs`, which documents `NaN` when a part is `NaN`, else
/// infinity when a part is infinite); finite parts give `MyMath.length`.
fn complex_abs(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        f64::NAN
    } else if x.is_infinite() || y.is_infinite() {
        f64::INFINITY
    } else {
        length(x, y)
    }
}

/// `MyMath.length`: the length of `(x, y)` without overflow.
fn length(x: f64, y: f64) -> f64 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        ay
    } else if ay == 0.0 {
        ax
    } else if ax > ay {
        let t = ay / ax;
        ax * (1.0 + t * t).sqrt()
    } else {
        let t = ax / ay;
        ay * (1.0 + t * t).sqrt()
    }
}
