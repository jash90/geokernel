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
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyBoolean.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec2D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentBoolean.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSequence.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSequenceRange.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoStats1D.java
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
//!
//! A list is built in a scratch buffer ([`Lists`]): its elements one after
//! the other, a nested list's elements further on. What an element leaves
//! behind is dropped before the next one, and the buffer is emptied for
//! the next evaluation, so a recompute allocates nothing once it has grown.

use super::decimal::Dec;
use super::list::{copy_tree, Slot};
use super::math::{self, PI_180};
use super::twonum;
use super::{Bin, Entry, IfMode, ListOp, Node, Raw, Ty, Un, NONE};
use crate::algo::intersect::Hint;
use crate::kernel::{Construction, Def, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

/// Elements one evaluation may build in all; past it every list being built
/// is undefined, so a `Sequence` with a huge range ends quickly.
const MAX_ELEMENTS: u64 = 1 << 20;

/// `Kernel.MIN_PRECISION`, the slack of the bounds of `Sequence`.
const MIN_PRECISION: f64 = 1e-5;

/// `MyMath.LARGEST_INTEGER`.
const LARGEST_INTEGER: f64 = 9007199254740992.0;

/// The scratch buffer of the lists of one evaluation (see the module
/// documentation), and the elements built so far.
pub(super) struct Lists {
    arena: RefCell<Vec<Slot>>,
    built: Cell<u64>,
}

impl Lists {
    pub(super) fn new(mut arena: Vec<Slot>) -> Lists {
        arena.clear();
        Lists {
            arena: RefCell::new(arena),
            built: Cell::new(0),
        }
    }

    pub(super) fn into_arena(self) -> Vec<Slot> {
        self.arena.into_inner()
    }

    fn len(&self) -> u32 {
        self.arena.borrow().len() as u32
    }

    fn truncate(&self, mark: u32) {
        self.arena.borrow_mut().truncate(mark as usize);
    }

    fn get(&self, i: u32) -> Slot {
        self.arena.borrow()[i as usize]
    }

    fn set(&self, i: u32, slot: Slot) {
        self.arena.borrow_mut()[i as usize] = slot;
    }

    /// Counts `n` more elements; false past [`MAX_ELEMENTS`].
    fn build(&self, n: u64) -> bool {
        let built = self.built.get().saturating_add(n);
        self.built.set(built);
        built <= MAX_ELEMENTS
    }

    /// `n` undefined slots at the end, where they start.
    fn reserve(&self, n: u32) -> Option<u32> {
        if !self.build(u64::from(n)) {
            return None;
        }
        let mut arena = self.arena.borrow_mut();
        let start = arena.len();
        arena.resize(start + n as usize, Slot::undefined(Kind::Number));
        Some(start as u32)
    }
}

/// The value of the variable of a `Sequence`, inside those around it.
struct Local<'a> {
    depth: u32,
    value: f64,
    up: Option<&'a Local<'a>>,
}

#[derive(Clone, Copy)]
enum Val {
    N(f64),
    V(f64, f64),
    /// A `MyBoolean`: its value and whether it is defined. A comparison
    /// of an infinite or undefined number is false and undefined; the flag
    /// only travels through the logical operators.
    B(bool, bool),
}

impl Val {
    fn num(self) -> f64 {
        match self {
            Val::N(v) => v,
            Val::V(..) => f64::NAN,
            Val::B(b, _) => bool_number(b),
        }
    }
}

/// `MyBoolean.getDouble`.
fn bool_number(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
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

#[derive(Clone, Copy)]
struct Ctx<'a> {
    c: &'a Construction,
    e: &'a Entry,
    env: &'a Env<'a>,
    /// The scratch of a tree with lists; `None` for a tree without.
    lists: Option<&'a Lists>,
    /// The variables of the `Sequence`s around the node being evaluated.
    locals: Option<&'a Local<'a>>,
}

impl<'a> Ctx<'a> {
    fn new(c: &'a Construction, e: &'a Entry, env: &'a Env<'a>) -> Ctx<'a> {
        Ctx {
            c,
            e,
            env,
            lists: None,
            locals: None,
        }
    }
}

/// Evaluates an entry's tree (full evaluation), with the exact decimal of
/// a number result (`toDecimal`).
pub(super) fn root(c: &Construction, e: &Entry, lists: Option<&Lists>) -> (Raw, Option<Exact>) {
    let ctx = Ctx {
        lists,
        ..Ctx::new(c, e, &Env::None)
    };
    let x = ctx.full(e.root);
    if lists.is_some_and(|l| l.built.get() > MAX_ELEMENTS) {
        return (Raw::Num(f64::NAN), None);
    }
    match x.val {
        Val::N(v) => (Raw::Num(v), x.dec),
        Val::V(x, y) => (Raw::Vec(x, y), None),
        Val::B(b, _) => (Raw::Num(bool_number(b)), None),
    }
}

/// Evaluates an entry whose result is a list: its elements go to `out`,
/// compacted (the top level first); its length, `None` when undefined.
pub(super) fn root_list(
    c: &Construction,
    e: &Entry,
    lists: &Lists,
    out: &mut Vec<Slot>,
) -> Option<u32> {
    let ctx = Ctx {
        lists: Some(lists),
        ..Ctx::new(c, e, &Env::None)
    };
    let (start, len) = ctx.list(e.root)?;
    if lists.built.get() > MAX_ELEMENTS {
        return None;
    }
    out.clear();
    copy_tree(&lists.arena.borrow(), start, len, out);
    Some(len)
}

/// `Function.value`: a function entry at `x`.
pub(super) fn sample(c: &Construction, e: &Entry, x: f64) -> f64 {
    Ctx::new(c, e, &Env::Value(x)).fast(e.root)
}

/// The value of the condition at `i` of an entry's `AlgoIf`, which reads
/// no `x`.
pub(super) fn condition(c: &Construction, e: &Entry, i: u32) -> bool {
    matches!(Ctx::new(c, e, &Env::None).full(i).val, Val::B(true, _))
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
            Node::Bool(b) => Ex::plain(Val::B(b, true)),
            Node::Un(Un::Not, a) => Ex::plain(match self.full(a).val {
                Val::B(b, defined) => Val::B(!b, defined),
                _ => Val::B(false, false),
            }),
            Node::Bin(op, a, b) if op.is_comparison() || op.is_logic() => {
                Ex::plain(logic(op, self.full(a).val, self.full(b).val))
            }
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
            Node::Bin(Bin::Sub, a, b)
                if self.is_list(a) != self.is_list(b) && (self.is_vec(a) || self.is_vec(b)) =>
            {
                Ex::plain(self.list_minus_vector(a, b))
            }
            Node::Bin(op, a, b) => {
                let (l, r) = (self.operand(a), self.operand(b));
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
            Node::If(mode, cond, then, rest) => self.conditional(i, mode, cond, then, rest),
            Node::Item(a) => self.full(a),
            // the variable of a `Sequence` is a `GeoNumeric`
            Node::Local(depth) => Ex {
                obj: true,
                ..Ex::plain(N(self.local(depth)))
            },
            Node::ListOp(op, l, n) => self.list_command(i, op, l, n),
            // a list is only read through `list`
            Node::FnName(_) | Node::List(..) | Node::Range(..) | Node::Seq(..) => {
                Ex::plain(N(f64::NAN))
            }
        }
    }

    /// `AlgoIf.compute` and `Operation.IF`, `IF_ELSE`, `IF_LIST` (see
    /// [`IfMode`]). Only the chosen branch is evaluated: an untaken branch
    /// changes no value.
    fn conditional(&self, i: u32, mode: IfMode, cond: u32, then: u32, rest: u32) -> Ex {
        let (value, defined) = match self.full(cond).val {
            Val::B(v, d) => (v, d),
            _ => (false, false),
        };
        let ty = self.e.tys[i as usize];
        let undefined = || match ty {
            Ty::Vec => Ex::plain(Val::V(f64::NAN, f64::NAN)),
            // `GeoBoolean.setUndefined` only sets false
            Ty::Bool => Ex::plain(Val::B(false, true)),
            Ty::Num(_) | Ty::List(_) => Ex::plain(Val::N(f64::NAN)),
        };
        if mode == IfMode::Else && !defined {
            return undefined();
        }
        let chosen = if value {
            then
        } else if rest != NONE {
            rest
        } else {
            return undefined();
        };
        let x = self.full(chosen);
        if mode != IfMode::Algo {
            // the operation hands on the branch's value itself
            return x;
        }
        // `AlgoIf.setResult`: an undefined branch makes the result
        // undefined; a number copies its value and exact decimal
        // (`GeoNumeric.set`) into a helper number
        match (ty, x.val) {
            (_, Val::N(v)) if v.is_nan() => undefined(),
            (Ty::Vec, Val::N(v)) => Ex::plain(Val::V(v, 0.0)),
            (Ty::Num(_), Val::N(v)) => {
                // `getNumber` keeps the decimal when the definition is
                // constant (`reuseDefinition`): the branch is a literal, a
                // constant expression or a free number that has its own
                let special = x.dec.is_some()
                    && match self.node(chosen) {
                        Node::Obj(_) => x.special,
                        _ => self.is_constant(chosen),
                    };
                Ex {
                    val: Val::N(v),
                    dec: x.dec,
                    special,
                    obj: true,
                }
            }
            (_, Val::B(b, _)) => Ex::plain(Val::B(b, true)),
            (_, val) => Ex::plain(val),
        }
    }

    /// Whether the subtree at `i` is a constant: no object, no `x`, no
    /// command; GeoGebra makes such an argument an independent number.
    fn is_constant(&self, i: u32) -> bool {
        match self.node(i) {
            Node::Obj(_)
            | Node::Var
            | Node::Cmd(..)
            | Node::Apply(..)
            | Node::If(..)
            | Node::Local(_)
            | Node::ListOp(..)
            | Node::Range(..)
            | Node::Seq(..) => false,
            Node::Bin(op, ..) if op.is_command() => false,
            node => super::children(&node).all(|child| self.is_constant(child)),
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
            let inner = Ctx::new(self.c, e, &env);
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
        Ctx::new(self.c, e, &env).full(e.root)
    }

    fn full_number(&self, i: u32) -> f64 {
        self.full(i).num()
    }

    /// The full evaluation of an operand of a numeric operation: a boolean
    /// is the plain number 1 or 0 (`MyBoolean.getNumber`).
    fn operand(&self, i: u32) -> Ex {
        let x = self.full(i);
        match x.val {
            Val::B(b, _) => Ex::plain(Val::N(bool_number(b))),
            _ => x,
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
            Node::Obj(id) => object(self.c, id).num(),
            _ => self.full_number(i),
        }
    }

    /// `Operation.ROUND2`: digits that evaluate to `NaN` fall back to
    /// rounding to an integer, unless they are an object.
    fn round2(&self, a: u32, b: u32) -> Ex {
        let (x, digits) = (self.operand(a), self.operand(b));
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
        let (l, r) = (self.operand(a), self.operand(b));
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
        let operand = self.operand(a);
        let x = match operand.val {
            Val::N(x) => x,
            Val::B(..) => f64::NAN,
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

/// Lists: built in the scratch buffer, read by the list commands.
impl Ctx<'_> {
    /// The value of the variable of the `Sequence` of `depth`.
    fn local(&self, depth: u32) -> f64 {
        let mut local = self.locals;
        while let Some(l) = local {
            if l.depth == depth {
                return l.value;
            }
            local = l.up;
        }
        f64::NAN
    }

    fn is_list(&self, i: u32) -> bool {
        matches!(self.e.tys[i as usize], Ty::List(_))
    }

    /// Builds the list at `i` in the scratch buffer: where its elements
    /// start and how many there are; `None` when it is undefined.
    fn list(&self, i: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        match self.node(i) {
            // `AlgoDependentList`: one element per item
            Node::List(first, len) => {
                let start = lists.reserve(len)?;
                for k in 0..len {
                    let slot = self.element(first + k);
                    lists.set(start + k, slot);
                }
                Some((start, len))
            }
            Node::Item(a) => self.list(a),
            Node::Obj(id) => self.object_list(id),
            Node::Bin(op, a, b) => self.list_binary(op, a, b),
            Node::Un(op, a) => self.list_unary(op, a),
            Node::Range(from, to, step) => self.range(from, to, step),
            Node::Seq(e, from, to, step, depth) => self.sequence(e, from, to, step, depth),
            // an element that is a list: its elements were built with the
            // list it belongs to
            Node::ListOp(op, l, n) => self.pick(op, l, n)?.run(),
            _ => None,
        }
    }

    /// One element of a list being built. A list element is built after
    /// what is there and kept; anything else is evaluated, and what that
    /// built is dropped.
    fn element(&self, i: u32) -> Slot {
        let i = match self.node(i) {
            Node::Item(a) => a,
            _ => i,
        };
        if self.is_list(i) {
            return match self.list(i) {
                Some((start, len)) => Slot::list(start, len),
                None => Slot::undefined(Kind::List),
            };
        }
        let mark = self.lists.map(Lists::len);
        let x = self.full(i).val;
        if let (Some(lists), Some(mark)) = (self.lists, mark) {
            lists.truncate(mark);
        }
        match x {
            Val::N(v) => Slot::number(v),
            Val::V(x, y) => Slot::coords(self.vec_kind(i), x, y),
            Val::B(b, _) => Slot::boolean(b),
        }
    }

    /// Whether a point or vector element is a point or a vector: an object
    /// or a command keeps its kind, coordinates are a point, and an
    /// operation is a point when a point takes part.
    fn vec_kind(&self, i: u32) -> Kind {
        let kind = |k: Kind| {
            if k == Kind::Vector {
                Kind::Vector
            } else {
                Kind::Point
            }
        };
        match self.node(i) {
            Node::Obj(id) => kind(self.c.kind(id)),
            Node::Cmd(def, _) => kind(def.kind()),
            Node::Item(a) | Node::Un(_, a) | Node::If(_, _, a, _) => self.vec_kind(a),
            Node::Bin(_, a, b) => {
                let kinds = [a, b].map(|j| self.is_vec(j).then(|| self.vec_kind(j)));
                if kinds.contains(&Some(Kind::Point)) || !kinds.contains(&Some(Kind::Vector)) {
                    Kind::Point
                } else {
                    Kind::Vector
                }
            }
            _ => Kind::Point,
        }
    }

    /// The elements of a list object, copied to the scratch buffer.
    fn object_list(&self, id: Id) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (Def::Expr(d), Value::List { len }) = (self.c.def(id), self.c.value(id)) else {
            return None;
        };
        let items = &super::entry(self.c, d).items;
        if !lists.build(items.len() as u64) {
            return None;
        }
        let mut arena = lists.arena.borrow_mut();
        let base = arena.len() as u32;
        arena.extend(items.iter().map(|s| match s.run() {
            Some((start, n)) => Slot::list(start + base, n),
            None => *s,
        }));
        Some((base, len))
    }

    /// `MyList.apply` of a two-operand operation: element by element, or
    /// every element with the other operand. Lists of unequal length give
    /// an empty list; a point or vector result is a point.
    fn list_binary(&self, op: Bin, a: u32, b: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        // the operand that is no list first, so that what it builds goes
        let scalar = |j: u32| {
            let mark = lists.len();
            let v = self.operand(j).val;
            lists.truncate(mark);
            v
        };
        let (la, lb) = (self.is_list(a), self.is_list(b));
        let left = (!la).then(|| scalar(a));
        let right = (!lb).then(|| scalar(b));
        let ra = if la { Some(self.list(a)?) } else { None };
        let rb = if lb { Some(self.list(b)?) } else { None };
        let len = match (ra, rb) {
            (Some((_, n)), Some((_, m))) if n != m => return Some((lists.len(), 0)),
            (Some((_, n)), _) | (_, Some((_, n))) => n,
            (None, None) => return None,
        };
        let start = lists.reserve(len)?;
        for k in 0..len {
            let x = left.unwrap_or_else(|| {
                ra.map_or(Val::N(f64::NAN), |(s, _)| slot_val(lists.get(s + k)))
            });
            let y = right.unwrap_or_else(|| {
                rb.map_or(Val::N(f64::NAN), |(s, _)| slot_val(lists.get(s + k)))
            });
            lists.set(start + k, combine(op, x, y));
        }
        Some((start, len))
    }

    /// `ExpressionNodeEvaluator.handleMinus` of a list and a point or
    /// vector, in either order, is not element by element: `GeoVec2D.sub`
    /// reads a list of two elements as `(x, y)` (`evaluateDouble`: a
    /// boolean is 0 or 1, a point or a list not a number); a list of any
    /// other length gives an undefined result.
    fn list_minus_vector(&self, a: u32, b: u32) -> Val {
        use Val::{N, V};
        let undefined = V(f64::NAN, f64::NAN);
        let Some(lists) = self.lists else {
            return undefined;
        };
        let (list, vector, reverse) = if self.is_list(a) {
            (a, b, false)
        } else {
            (b, a, true)
        };
        let coordinate = |slot: Slot| match slot_val(slot) {
            N(v) => v,
            Val::B(b, _) => bool_number(b),
            V(..) => f64::NAN,
        };
        let mark = lists.len();
        let xy = self.list(list).and_then(|(s, n)| {
            (n == 2).then(|| (coordinate(lists.get(s)), coordinate(lists.get(s + 1))))
        });
        lists.truncate(mark);
        let Some((lx, ly)) = xy else {
            return undefined;
        };
        match self.operand(vector).val {
            V(x, y) if reverse => V(x - lx, y - ly),
            V(x, y) => V(lx - x, ly - y),
            _ => undefined,
        }
    }

    /// `MyList.apply` of a one-operand operation.
    fn list_unary(&self, op: Un, a: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (s, n) = self.list(a)?;
        let start = lists.reserve(n)?;
        for k in 0..n {
            let slot = match slot_val(lists.get(s + k)) {
                Val::B(b, _) if op == Un::Not => Slot::boolean(!b),
                Val::B(b, _) => Slot::number(scalar(op, bool_number(b), false)),
                Val::N(x) => Slot::number(scalar(op, x, false)),
                Val::V(x, y) => Slot::number(match op {
                    Un::X => x,
                    Un::Y => y,
                    Un::Abs => complex_abs(x, y),
                    _ => f64::NAN,
                }),
            };
            lists.set(start + k, slot);
        }
        Some((start, n))
    }

    /// `AlgoSequenceRange`: `Sequence(n)` is 1 to `n` rounded; `Sequence(a,
    /// b)` runs from `a` to `b` rounded, up or down; `Sequence(a, b, step)`
    /// steps from `a`, on decimals when `a` and `step` have them and there
    /// are at most 100 steps.
    fn range(&self, from: u32, to: u32, step: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        if from == NONE {
            let n = u32::try_from(math::java_round(self.full_number(to)).max(0)).ok()?;
            let start = lists.reserve(n)?;
            for k in 0..n {
                lists.set(start + k, Slot::number(f64::from(k) + 1.0));
            }
            return Some((start, n));
        }
        let f = self.full(from);
        let s = (step != NONE).then(|| self.full(step));
        let (mut a, mut b) = (f.num(), self.full_number(to));
        if s.is_none() {
            a = math::java_round(a) as f64;
            b = math::java_round(b) as f64;
        }
        if a.abs() > LARGEST_INTEGER || b.abs() > LARGEST_INTEGER {
            return None;
        }
        let sign = if b >= a { 1.0 } else { -1.0 };
        let d = match &s {
            Some(s) => {
                let d = s.num();
                if math::is_zero(d) || d * sign < 0.0 {
                    return None;
                }
                d
            }
            None => sign,
        };
        let decimals = match (&f.dec, s.as_ref().and_then(|s| s.dec.as_ref())) {
            (Some(x), Some(y)) if (b - a) / d <= 100.0 => x.get(a).zip(y.get(d)),
            _ => None,
        };
        let decimals = decimals.as_ref().map(|(x, y)| (x.as_ref(), y.as_ref()));
        let values = |emit: &mut dyn FnMut(f64) -> bool| {
            if let Some((mut k, step)) = decimals.map(|(x, y)| (x.clone(), y)) {
                while k.to_f64() * sign <= b * sign + MIN_PRECISION {
                    if !emit(k.to_f64()) {
                        return;
                    }
                    match k.add(step) {
                        Some(next) => k = next,
                        None => return,
                    }
                }
                return;
            }
            let mut steps = 0.0;
            let mut k = a;
            while k * sign <= b * sign + MIN_PRECISION {
                if math::is_integer(k) {
                    k = math::java_round(k) as f64;
                }
                if !emit(k) {
                    return;
                }
                steps += 1.0;
                k = a + steps * d;
            }
        };
        let n = count(values)?;
        let start = lists.reserve(n)?;
        let mut k = 0;
        values(&mut |v| {
            lists.set(start + k, Slot::number(v));
            k += 1;
            k < n
        });
        Some((start, n))
    }

    /// `AlgoSequence`: the expression at the variable's values from `from`
    /// on by `step` while within `to` (and `MIN_PRECISION`); a value close
    /// to a whole number is made whole.
    fn sequence(&self, e: u32, from: u32, to: u32, step: u32, depth: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (f, t) = (self.full_number(from), self.full_number(to));
        let s = if step == NONE {
            1.0
        } else {
            self.full_number(step)
        };
        if f.is_nan() || t.is_nan() || s.is_nan() {
            return None;
        }
        if (t - f) * s <= -MIN_PRECISION {
            return Some((lists.len(), 0));
        }
        if ((t - f) / s).is_infinite() {
            return None;
        }
        let values = |emit: &mut dyn FnMut(f64) -> bool| {
            let mut v = f;
            while (s > 0.0 && v <= t + MIN_PRECISION) || (s < 0.0 && v >= t - MIN_PRECISION) {
                if !emit(v) {
                    return;
                }
                v += s;
                if math::is_integer(v) {
                    v = math::java_round(v) as f64;
                }
            }
        };
        let n = count(values)?;
        let start = lists.reserve(n)?;
        let mut k = 0;
        values(&mut |v| {
            let local = Local {
                depth,
                value: v,
                up: self.locals,
            };
            let slot = Ctx {
                locals: Some(&local),
                ..*self
            }
            .element(e);
            lists.set(start + k, slot);
            k += 1;
            k < n
        });
        Some((start, n))
    }

    /// The element `l(n)` or `Element(l, n)` picks, `None` when there is
    /// none. Its list is built and stays in the scratch buffer.
    fn pick(&self, op: ListOp, l: u32, n: u32) -> Option<Slot> {
        let lists = self.lists?;
        let mark = lists.len();
        let index = self.full_number(n);
        lists.truncate(mark);
        let (start, len) = self.list(l)?;
        let mut k = math::java_round(index) - 1;
        if op == ListOp::Index && k < 0 {
            k += i64::from(len) + 1;
        }
        (0..i64::from(len))
            .contains(&k)
            .then(|| lists.get(start + k as u32))
    }

    /// A list command that gives a number, a point or a boolean; the result
    /// is a helper object, and what it built is dropped.
    fn list_command(&self, i: u32, op: ListOp, l: u32, n: u32) -> Ex {
        let Some(lists) = self.lists else {
            return Ex::plain(Val::N(f64::NAN));
        };
        let mark = lists.len();
        let val = match op {
            ListOp::Length => Val::N(self.list(l).map_or(f64::NAN, |(_, len)| f64::from(len))),
            ListOp::Sum => Val::N(self.sum(l, n)),
            ListOp::Index | ListOp::Element => {
                match (self.pick(op, l, n), self.e.tys[i as usize]) {
                    (Some(slot), _) => slot_val(slot),
                    (None, Ty::Vec) => Val::V(f64::NAN, f64::NAN),
                    (None, Ty::Bool) => Val::B(false, true),
                    (None, _) => Val::N(f64::NAN),
                }
            }
        };
        lists.truncate(mark);
        Ex {
            obj: true,
            ..Ex::plain(val)
        }
    }

    /// `AlgoStats1D` with `STATS_SIGMAX`: the sum of the first `n` (all by
    /// default) elements as doubles; `n` is cut to a whole number, 0 gives
    /// 0 and anything outside the list is undefined.
    fn sum(&self, l: u32, n: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        let truncate = (n != NONE).then(|| self.full_number(n));
        let Some((start, len)) = self.list(l) else {
            return f64::NAN;
        };
        let mut size = len;
        if let Some(t) = truncate {
            if t.is_nan() {
                return f64::NAN;
            }
            // Java's `(int)`: towards zero, saturating
            let t = t as i32;
            if t == 0 {
                return 0.0;
            }
            match u32::try_from(t) {
                Ok(t) if t <= len => size = t,
                _ => return f64::NAN,
            }
        }
        let mut sum = 0.0;
        for k in 0..size {
            let slot = lists.get(start + k);
            sum += match (slot.kind, slot.value) {
                (_, Value::Number(v)) => v,
                (_, Value::Bool(b)) => bool_number(b),
                (Kind::Number, _) => f64::NAN,
                _ => return f64::NAN,
            };
        }
        sum
    }
}

/// How many values `values` gives, `None` past [`MAX_ELEMENTS`].
fn count(values: impl Fn(&mut dyn FnMut(f64) -> bool)) -> Option<u32> {
    let mut n: u64 = 0;
    values(&mut |_| {
        n += 1;
        n <= MAX_ELEMENTS
    });
    (n <= MAX_ELEMENTS).then_some(n as u32)
}

/// An element as an operand.
fn slot_val(s: Slot) -> Val {
    match (s.kind, s.value) {
        (_, Value::Number(v)) => Val::N(v),
        (_, Value::Point { x, y } | Value::Vector { x, y }) => Val::V(x, y),
        (_, Value::Bool(b)) => Val::B(b, true),
        (Kind::Point | Kind::Vector, _) => Val::V(f64::NAN, f64::NAN),
        (Kind::Boolean, _) => Val::B(false, true),
        _ => Val::N(f64::NAN),
    }
}

/// One element of `MyList.apply` of a two-operand operation.
fn combine(op: Bin, x: Val, y: Val) -> Slot {
    let number = |v: Val| match v {
        Val::B(b, _) => Val::N(bool_number(b)),
        v => v,
    };
    let (x, y) = (number(x), number(y));
    let r = match (op, x, y) {
        (Bin::Pow, Val::N(a), Val::N(e)) => Val::N(math::my_pow(a, e)),
        (Bin::Pow, Val::V(a, b), Val::N(e)) => {
            Val::N(if e == 2.0 { a * a + b * b } else { f64::NAN })
        }
        _ => binary(op, x, y, false),
    };
    match r {
        Val::N(v) => Slot::number(v),
        Val::V(a, b) => Slot::coords(Kind::Point, a, b),
        Val::B(b, _) => Slot::boolean(b),
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
        Un::X | Un::Y | Un::Not => f64::NAN,
    }
}

/// `Operation.LESS` and the other comparisons, `EQUAL_BOOLEAN`,
/// `NOT_EQUAL`, `AND`, `OR`, `XOR` and `IMPLICATION`. Comparisons are on
/// doubles with `Kernel.STANDARD_PRECISION`, never on exact decimals:
/// `0.1 + 0.2 == 0.3` holds because the doubles are within 1e-8. Both
/// operands are always evaluated, as upstream.
fn logic(op: Bin, l: Val, r: Val) -> Val {
    use Val::{B, N, V};
    let number = |v: Val| match v {
        N(x) => Some(x),
        B(b, _) => Some(bool_number(b)),
        V(..) => None,
    };
    match op {
        Bin::And | Bin::Or | Bin::Xor | Bin::Implies => match (l, r) {
            (B(a, da), B(b, db)) => B(
                match op {
                    Bin::And => a && b,
                    Bin::Or => a || b,
                    Bin::Xor => a ^ b,
                    _ => !a || b,
                },
                da && db,
            ),
            _ => B(false, false),
        },
        // `ExpressionNode.isEqual`: numbers by `isEqual(a, b, eps)`, points
        // and vectors by `GeoVec2D.isEqual`, anything else is not equal;
        // always defined
        Bin::Equal | Bin::NotEqual => {
            let equal = match (number(l), number(r), l, r) {
                (Some(a), Some(b), ..) => math::is_equal_eps(a, b, STANDARD_PRECISION),
                (_, _, V(a, b), V(c, d)) => math::is_equal(a, c) && math::is_equal(b, d),
                _ => false,
            };
            B(equal == (op == Bin::Equal), true)
        }
        _ => {
            let (a, b) = (number(l).unwrap_or(f64::NAN), number(r).unwrap_or(f64::NAN));
            let value = match op {
                Bin::Less => math::is_greater(b, a),
                Bin::Greater => math::is_greater(a, b),
                Bin::LessEqual => math::is_greater_equal(b, a),
                _ => math::is_greater_equal(a, b),
            };
            B(value, a.is_finite() && b.is_finite())
        }
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
            Raw::List(_) => Val::N(nan),
        },
        (Kind::Number, _, Value::Number(v)) => Val::N(v),
        (Kind::Number, _, _) => Val::N(nan),
        (Kind::Point, _, Value::Point { x, y }) | (Kind::Vector, _, Value::Vector { x, y }) => {
            Val::V(x, y)
        }
        (Kind::Point | Kind::Vector, _, _) => Val::V(nan, nan),
        // `GeoBoolean.getMyBoolean`: the value, defined
        (Kind::Boolean, _, Value::Bool(b)) => Val::B(b, true),
        (Kind::Boolean, _, _) => Val::B(false, true),
        _ => Val::N(nan),
    }
}

/// A command computed in place.
fn command(c: &Construction, def: Def, hint: Hint) -> Val {
    let nan = f64::NAN;
    match c.command_value(def, hint) {
        Some(Value::Number(v)) => Val::N(v),
        Some(Value::Point { x, y } | Value::Vector { x, y }) => Val::V(x, y),
        Some(Value::Bool(b)) => Val::B(b, true),
        _ if def.kind() == Kind::Boolean => Val::B(false, true),
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
