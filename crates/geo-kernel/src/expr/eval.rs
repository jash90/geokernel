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
//! A function is sampled by the fast evaluation with `x` set to a number
//! (`Function.value`). Inside a function, an applied function is expanded
//! (`AlgoDependentFunction.expandFunctionDerivativeNodes`): its `x` stands
//! for the argument's subtree, evaluated in place. Outside a function,
//! `f(a)` evaluates `a` and samples `f` there.

use super::math::{self, PI_180};
use super::{Bin, Entry, Node, Raw, Ty, Un};
use crate::kernel::{Construction, Def, Id, Kind, Value};

#[derive(Clone, Copy)]
enum Val {
    N(f64),
    V(f64, f64),
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

/// Evaluates an entry's tree (full evaluation).
pub(super) fn root(c: &Construction, e: &Entry) -> Raw {
    let ctx = Ctx {
        c,
        e,
        env: &Env::None,
    };
    match ctx.full(e.root) {
        Val::N(v) => Raw::Num(v),
        Val::V(x, y) => Raw::Vec(x, y),
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

    fn full(&self, i: u32) -> Val {
        use Val::{N, V};
        match self.node(i) {
            Node::Num(v) => N(v),
            Node::MinusOne => N(-1.0),
            Node::Degree => N(PI_180),
            Node::Var => match *self.env {
                Env::None => N(f64::NAN),
                Env::Value(x) => N(x),
                Env::Arg(caller, a) => N(caller.full_number(a)),
            },
            Node::Obj(id) => object(self.c, id),
            Node::Cmd(def) => command(self.c, def),
            // `MyVecNode.getVector`: coordinates by `evaluateDouble`.
            Node::Tuple(a, b) => V(self.fast(a), self.fast(b)),
            Node::Polar(r, phi) => {
                let (r, phi) = (self.fast(r), self.fast(phi));
                V(r * phi.cos(), r * phi.sin())
            }
            Node::Un(op, a) => self.unary(op, a),
            Node::Bin(Bin::Pow, a, b) => self.power(a, b),
            Node::Bin(op, a, b) => binary(op, self.full(a), self.full(b), self.is_angle(a)),
            Node::Apply(f, a) => N(self.apply(f, a, false)),
            Node::FnName(_) => N(f64::NAN),
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

    fn full_number(&self, i: u32) -> f64 {
        match self.full(i) {
            Val::N(v) => v,
            Val::V(..) => f64::NAN,
        }
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
            _ => self.full_number(i),
        }
    }

    /// `ExpressionNodeEvaluator.handlePower`.
    fn power(&self, a: u32, b: u32) -> Val {
        match (self.full(a), self.full(b)) {
            (Val::N(base), Val::N(e)) => {
                if math::is_euler(base) {
                    return Val::N(e.exp());
                }
                if base < 0.0 {
                    if let Some(v) = self.neg_power(base, b) {
                        return Val::N(v);
                    }
                }
                Val::N(math::my_pow(base, e))
            }
            // `v^2` is the scalar product of `v` with itself
            (Val::V(x, y), Val::N(e)) => Val::N(if e == 2.0 { x * x + y * y } else { f64::NAN }),
            _ => Val::N(f64::NAN),
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
    fn unary(&self, op: Un, a: u32) -> Val {
        let x = match self.full(a) {
            Val::N(x) => x,
            Val::V(x, y) => {
                return Val::N(match op {
                    Un::X => x,
                    Un::Y => y,
                    Un::Abs => length(x, y),
                    _ => f64::NAN,
                })
            }
        };
        let deg = self.is_angle(a);
        Val::N(match op {
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
            Un::Asin => math::asin(x),
            Un::Acos => math::acos(x),
            Un::Atan => x.atan(),
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
        })
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
fn command(c: &Construction, def: Def) -> Val {
    let nan = f64::NAN;
    match c.try_compute(def) {
        Some(Value::Number(v)) => Val::N(v),
        Some(Value::Point { x, y } | Value::Vector { x, y }) => Val::V(x, y),
        _ if def.kind() == Kind::Number => Val::N(nan),
        _ => Val::V(nan, nan),
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
