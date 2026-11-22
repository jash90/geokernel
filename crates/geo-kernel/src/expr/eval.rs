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
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListMinMax.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoReverse.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFirst.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLast.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSort.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAppend.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoin.java
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

use super::commands;
use super::decimal::Dec;
use super::list::{copy_tree, List, Slot};
use super::math::{self, PI_180};
use super::{Bin, Elem, Entry, ListOp, Node, Raw, Ty, Un};
use crate::algo::intersect::Hint;
use crate::kernel::{Construction, Def, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

/// The elements one evaluation of one object may build in all, across all
/// nesting levels, lists dropped again included: a bound on memory. Every
/// loop of the list code either builds elements counted against it, reads
/// elements already counted, or counts the values of a range within what is
/// left of it; once it is crossed nothing more is built or evaluated, and
/// the result is undefined. Upstream has no such bound (see
/// `DIFFERENCES.md`).
pub const MAX_ELEMENTS: u64 = 1 << 20;

/// The work one evaluation of one object with lists may do, in the units of
/// [`work_done`]: a bound on time, about 0.3 s of a release build (a unit
/// is about 3.5 ns). Past it the evaluation stops (see [`Lists::step`])
/// and the result is undefined. Upstream has no such bound (see
/// `DIFFERENCES.md`).
pub const MAX_EVALUATION_WORK: u64 = 80_000_000;

/// How far past [`MAX_EVALUATION_WORK`] the work of an evaluation may go:
/// a node is checked before it runs, so the last one to run may add its
/// visit and its exact decimal operations, which are charged when done:
/// the decimals of its two operands, the operation and the double of its
/// result.
pub const MAX_OVERRUN: u64 = FULL_VISIT + 4 * super::decimal::MAX_CHARGE;

/// The units of visiting a node in the full evaluation (which carries an
/// exact decimal along) and in `evaluateDouble`, and the extra units of a
/// power or a function of a number (`Math.pow`, `Math.sin`, ...), measured.
const FULL_VISIT: u64 = 3;
const FAST_VISIT: u64 = 1;
const POWER: u64 = 8;
const FUNCTION: u64 = 4;

#[cfg(test)]
thread_local! {
    /// The work bound the tests evaluate with: [`MAX_EVALUATION_WORK`], or
    /// none, to compare a result with the unbounded one.
    static LIMIT: Cell<u64> = const { Cell::new(MAX_EVALUATION_WORK) };
}

/// Evaluates `f` without a work bound (the element bound stays).
#[cfg(test)]
pub(super) fn unbounded<T>(f: impl FnOnce() -> T) -> T {
    with_work_limit(u64::MAX, f)
}

#[cfg(test)]
pub(super) fn with_work_limit<T>(limit: u64, f: impl FnOnce() -> T) -> T {
    let old = LIMIT.with(|l| l.replace(limit));
    let r = f();
    LIMIT.with(|l| l.set(old));
    r
}

#[inline]
fn limit() -> u64 {
    #[cfg(test)]
    return LIMIT.with(Cell::get);
    #[cfg(not(test))]
    MAX_EVALUATION_WORK
}

thread_local! {
    /// The work done on this thread (see [`work_done`]).
    static WORK: Cell<u64> = const { Cell::new(0) };
}

/// The evaluation work done on this thread so far, in units of about 3.5
/// ns of a release build: a node of an expression tree visited in an
/// evaluation with lists (functions sampled or expanded included, only the
/// branch of an `If` that is taken), a loop step of the list code (an
/// element built, copied or read, a step of counting a range), and every
/// exact decimal operation by its measured cost. Only the difference
/// between two readings means anything; tests use it to check that an
/// evaluation does bounded work without a clock.
#[doc(hidden)]
pub fn work_done() -> u64 {
    WORK.with(Cell::get)
}

/// Adds `n` to [`work_done`].
#[inline]
pub(super) fn work(n: u64) {
    WORK.with(|w| w.set(w.get().wrapping_add(n)));
}

/// The addresses of the elements of a list object read in place (see
/// [`Lists::read`]): the object in the high bits, the element in the low
/// ones. The scratch buffer stays below `1 << OBJECT_SHIFT` (at most about
/// [`MAX_ELEMENTS`] elements are ever built in it), and so does a list
/// object.
pub(super) const OBJECT_SHIFT: u32 = 24;
const OFFSET: u32 = (1 << OBJECT_SHIFT) - 1;
const MAX_OBJECTS: usize = 32;

/// The scratch buffer of the lists of one evaluation (see the module
/// documentation), the elements built so far, and where its work ends.
pub(super) struct Lists {
    arena: RefCell<Vec<Slot>>,
    /// The list objects read in place (`AlgoDependentList` and the commands
    /// read a list object without copying it): the elements of the `k`th
    /// are at the addresses `(k + 1) << OBJECT_SHIFT` on (see
    /// [`Lists::read`]). Kept inline, so that an evaluation allocates
    /// nothing for them.
    objects: RefCell<([Id; MAX_OBJECTS], usize)>,
    built: Cell<u64>,
    /// The reading of [`work_done`] past which the evaluation is over.
    end: u64,
}

impl Lists {
    pub(super) fn new(mut arena: Vec<Slot>) -> Lists {
        arena.clear();
        Lists {
            arena: RefCell::new(arena),
            objects: RefCell::new(([Id(0); MAX_OBJECTS], 0)),
            built: Cell::new(0),
            end: work_done().saturating_add(limit()),
        }
    }

    pub(super) fn into_arena(self) -> Vec<Slot> {
        self.arena.into_inner()
    }

    pub(super) fn len(&self) -> u32 {
        self.arena.borrow().len() as u32
    }

    pub(super) fn truncate(&self, mark: u32) {
        self.arena.borrow_mut().truncate(mark as usize);
    }

    /// The element at `i`: in the scratch buffer, or in a list object read
    /// in place, whose nested lists are given addresses in it too.
    pub(super) fn read(&self, c: &Construction, i: u32) -> Slot {
        let object = i >> OBJECT_SHIFT;
        if object == 0 {
            return self.arena.borrow()[i as usize].clone();
        }
        let id = self.objects.borrow().0[object as usize - 1];
        let mut slot =
            super::list_slots(c, id).expect("readable list slots")[(i & OFFSET) as usize].clone();
        if slot.run().is_some() {
            slot.start |= object << OBJECT_SHIFT;
        }
        slot
    }

    /// Where the elements of the list object `id` are read in place, `None`
    /// when too many list objects are read already.
    fn object(&self, id: Id) -> Option<u32> {
        let (ids, len) = &mut *self.objects.borrow_mut();
        let k = match ids[..*len].iter().position(|&o| o == id) {
            Some(k) => k,
            None if *len < MAX_OBJECTS => {
                ids[*len] = id;
                *len += 1;
                *len - 1
            }
            None => return None,
        };
        Some((k as u32 + 1) << OBJECT_SHIFT)
    }

    /// Copies the list of `len` elements at `start`, with the lists nested
    /// in it, to `out`. Lists read in place are copied there, and counted
    /// as built then; `false` past the element bound.
    fn copy_out(&self, c: &Construction, start: u32, len: u32, out: &mut Vec<Slot>) -> bool {
        if self.objects.borrow().1 == 0 {
            copy_tree(&self.arena.borrow(), start, len, out);
            return true;
        }
        let copy = |from: u32, n: u32, out: &mut Vec<Slot>| {
            if from >> OBJECT_SHIFT != 0 && !(self.build(u64::from(n)) && self.charge(u64::from(n)))
            {
                return false;
            }
            out.extend((from..from + n).map(|k| self.read(c, k)));
            true
        };
        if !copy(start, len, out) {
            return false;
        }
        let mut i = 0;
        while i < out.len() {
            if let Some((s, n)) = out[i].run() {
                out[i].start = out.len() as u32;
                if !copy(s, n, out) {
                    return false;
                }
            }
            i += 1;
        }
        true
    }

    /// Borrows a list as a flattened tree, copying only when nested entries
    /// may point into other list objects.
    pub(super) fn with_list<T>(
        &self,
        c: &Construction,
        start: u32,
        len: u32,
        f: impl FnOnce(List<'_>) -> Option<T>,
    ) -> Option<T> {
        if self.objects.borrow().1 == 0 {
            return f(List::from_run(&self.arena.borrow(), start, len));
        }
        if start >> OBJECT_SHIFT != 0 {
            let id = self.objects.borrow().0[(start >> OBJECT_SHIFT) as usize - 1];
            return f(List::from_run(
                super::list_slots(c, id)?,
                start & OFFSET,
                len,
            ));
        }
        let mut items = Vec::new();
        if !self.copy_out(c, start, len, &mut items) {
            return None;
        }
        f(List::new(&items, len))
    }

    pub(super) fn set(&self, i: u32, slot: Slot) {
        self.arena.borrow_mut()[i as usize] = slot;
    }

    /// Sorts `len` elements from `start` in place, numbers and booleans by
    /// value and points by `x`, then `y`; undefined ones go last.
    pub(super) fn sort(&self, start: u32, len: u32) {
        // `+ 0.0` makes -0 equal to 0
        let key = |s: &Slot| match s.shown() {
            Value::Number(v) => (v + 0.0, 0.0),
            Value::Bool(b) => (bool_number(b), 0.0),
            Value::Point { x, y } | Value::Vector { x, y } => (x + 0.0, y + 0.0),
            _ => (f64::NAN, f64::NAN),
        };
        let mut arena = self.arena.borrow_mut();
        let run = &mut arena[start as usize..(start + len) as usize];
        // a total order, so the sort cannot fail; it does not allocate
        run.sort_unstable_by(|a, b| {
            let (ka, kb) = (key(a), key(b));
            ka.0.is_nan()
                .cmp(&kb.0.is_nan())
                .then(ka.0.total_cmp(&kb.0))
                .then(ka.1.total_cmp(&kb.1))
        });
    }

    /// Counts `n` more elements; false past [`MAX_ELEMENTS`].
    fn build(&self, n: u64) -> bool {
        let built = self.built.get().saturating_add(n);
        self.built.set(built);
        built <= MAX_ELEMENTS
    }

    /// Charges the visit of one node, `n` units; true when the evaluation
    /// is past its bound, and the node is not evaluated. Every node is
    /// checked before it runs, and every loop of the list code before it
    /// runs (see [`Lists::charge`]), so the evaluation does at most
    /// [`MAX_OVERRUN`] more work than [`MAX_EVALUATION_WORK`] in all.
    #[inline]
    fn step(&self, n: u64) -> bool {
        WORK.with(|w| {
            let done = w.get();
            let over = done > self.end || self.built.get() > MAX_ELEMENTS;
            if !over {
                w.set(done.wrapping_add(n));
            }
            over
        })
    }

    /// Charges a loop of the list code of `n` units before it runs; false,
    /// and the evaluation is over, when it would go past the bound.
    pub(super) fn charge(&self, n: u64) -> bool {
        if self.over() || work_done().saturating_add(n) > self.end {
            self.give_up();
            return false;
        }
        work(n);
        true
    }

    /// Whether this evaluation went past [`MAX_ELEMENTS`] or [`MAX_EVALUATION_WORK`]:
    /// its result is undefined, so nothing more is built or evaluated.
    pub(super) fn over(&self) -> bool {
        self.built.get() > MAX_ELEMENTS || work_done() > self.end
    }

    /// Puts this evaluation past its bound.
    fn give_up(&self) {
        self.built.set(MAX_ELEMENTS + 1);
    }

    /// `n` undefined slots at the end, where they start.
    pub(super) fn reserve(&self, n: u32) -> Option<u32> {
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
pub(super) struct Local<'a> {
    pub(super) depth: u32,
    pub(super) value: Val,
    pub(super) run: Option<(u32, u32)>,
    pub(super) up: Option<&'a Local<'a>>,
}

#[derive(Clone, Copy)]
pub(super) enum Val {
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

/// A boolean operand of a numeric operation is the plain number 1 or 0
/// (`MyBoolean.getNumber`). A boolean result never carries a decimal, so
/// only its value and flags change, and nothing is dropped.
#[inline(always)]
fn as_number(x: &mut Ex) {
    if let Val::B(b, _) = x.val {
        debug_assert!(x.dec.is_none());
        x.val = Val::N(bool_number(b));
        x.special = false;
        x.obj = false;
    }
}

/// A result of the full evaluation with what GeoGebra's value object knows
/// besides its value: whether it is a `MySpecialDouble` (`special`), which
/// keeps a decimal, and that decimal (`toDecimal`). An object operand
/// (`obj`) is a `GeoNumeric`: as a right operand its decimal is its
/// `exactValue`; as a left operand GeoGebra takes `getNumber()`, which keeps
/// the decimal only for a free number that was never changed (`special`).
#[derive(Clone)]
pub(super) struct Ex {
    pub(super) val: Val,
    pub(super) dec: Option<Exact>,
    pub(super) special: bool,
    pub(super) obj: bool,
}

impl Ex {
    pub(super) fn plain(val: Val) -> Ex {
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

    pub(super) fn num(&self) -> f64 {
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
pub(super) enum Env<'a, const M: bool> {
    None,
    Value(f64),
    Pair(f64, f64),
    /// A subtree of the caller: the argument of an expanded function.
    Arg(&'a Ctx<'a, M>, u32),
}

/// An evaluation. `M` enables the work meter for lists, derivatives, and
/// function inverses.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a, const M: bool> {
    pub(super) c: &'a Construction,
    pub(super) e: &'a Entry,
    pub(super) env: &'a Env<'a, M>,
    /// The scratch of a tree with lists; `None` for a tree without.
    pub(super) lists: Option<&'a Lists>,
    /// The budget when `M`, charged for every node visited (see [`Lists::step`]),
    /// including the functions sampled or expanded.
    pub(super) meter: Option<&'a Lists>,
    /// The variables of the `Sequence`s around the node being evaluated.
    pub(super) locals: Option<&'a Local<'a>>,
    /// A saved template override for the text being evaluated.
    pub(super) text_latex_override: Option<bool>,
}

impl<'a, const M: bool> Ctx<'a, M> {
    pub(super) fn new(c: &'a Construction, e: &'a Entry, env: &'a Env<'a, M>) -> Ctx<'a, M> {
        Ctx {
            c,
            e,
            env,
            lists: None,
            meter: None,
            locals: None,
            text_latex_override: None,
        }
    }

    pub(super) fn text_latex(&self) -> bool {
        self.text_latex_override
            .unwrap_or_else(|| self.e.text_latex())
    }
}

/// Evaluates an entry's tree (full evaluation), with the exact decimal of
/// a number result (`toDecimal`).
#[inline]
pub(super) fn root(c: &Construction, e: &Entry, lists: Option<&Lists>) -> (Raw, Option<Exact>) {
    let inverse_meter = (lists.is_none() && e.inverts).then(|| Lists::new(Vec::new()));
    let x = match lists.or(inverse_meter.as_ref()) {
        None => Ctx::<false>::new(c, e, &Env::None).full(e.root),
        Some(meter) => {
            let x = Ctx::<true> {
                lists,
                meter: Some(meter),
                ..Ctx::new(c, e, &Env::None)
            }
            .full(e.root);
            if meter.over() {
                return (Raw::Num(f64::NAN), None);
            }
            x
        }
    };
    match x.val {
        Val::N(v) => (Raw::Num(v), x.dec),
        Val::V(x, y) => (Raw::Vec(x, y), None),
        Val::B(false, false)
            if matches!(
                e.nodes[e.root as usize],
                Node::Algebra(super::AlgebraOp::IsPrime, ..)
            ) =>
        {
            (Raw::Num(f64::NAN), None)
        }
        Val::B(b, _) => (Raw::Num(bool_number(b)), None),
    }
}

pub(super) fn text_root(c: &Construction, e: &Entry, latex: Option<bool>) -> super::TextResult {
    let meter = Lists::new(Vec::new());
    let ctx = Ctx::<true> {
        lists: Some(&meter),
        meter: Some(&meter),
        text_latex_override: latex,
        ..Ctx::new(c, e, &Env::None)
    };
    let text = ctx.text(e.root);
    let presentation = match ctx.node(e.root) {
        Node::TextCmd(op, first, count) => ctx.text_style(op, first, count),
        _ => crate::kernel::text::TextPresentation::default(),
    };
    let text = text.filter(|_| !meter.over()).map(|text| {
        let mut text = text.into_owned();
        while text.len() > 1 && text.ends_with('\n') {
            text.pop();
        }
        if !latex.unwrap_or(presentation.latex) && text.contains(r"\\n") {
            text = text.replace(r"\\n", "\n");
        }
        text
    });
    super::TextResult { text, presentation }
}

/// Evaluates an entry whose result is a list: its elements go to `out`,
/// compacted (the top level first); its length, `None` when undefined.
pub(super) fn root_list(
    c: &Construction,
    e: &Entry,
    lists: &Lists,
    out: &mut Vec<Slot>,
) -> Option<u32> {
    let ctx = Ctx::<true> {
        lists: Some(lists),
        meter: Some(lists),
        ..Ctx::new(c, e, &Env::None)
    };
    let (start, len) = ctx.list(e.root)?;
    if lists.over() {
        return None;
    }
    out.clear();
    lists.copy_out(c, start, len, out).then_some(len)
}

/// `Function.value`: a function entry at `x`, using the `y = 0` slice
/// for a two-variable slope.
pub(super) fn sample(c: &Construction, e: &Entry, x: f64) -> f64 {
    if !e.inverts {
        return Ctx::<false>::new(c, e, &Env::Pair(x, 0.0)).fast(e.root);
    }
    let meter = Lists::new(Vec::new());
    let y = Ctx::<true> {
        meter: Some(&meter),
        ..Ctx::new(c, e, &Env::Pair(x, 0.0))
    }
    .fast(e.root);
    if meter.over() {
        f64::NAN
    } else {
        y
    }
}

pub(super) fn system_locals(ctx: &Ctx<'_, true>, root: u32, depth: u32, values: &[f64]) -> f64 {
    if let Some((&x, rest)) = values.split_first() {
        let local = Local {
            depth,
            value: Val::N(x),
            run: None,
            up: ctx.locals,
        };
        system_locals(
            &Ctx {
                locals: Some(&local),
                ..*ctx
            },
            root,
            depth + 1,
            rest,
        )
    } else {
        ctx.fast(root)
    }
}

/// The tree of the function object `f`, when it is defined.
pub(super) fn function_entry(c: &Construction, f: Id) -> Option<&Entry> {
    match (c.def(f), c.value(f)) {
        (Def::Expr(d), Value::Function) => Some(super::entry(c, d)),
        _ => None,
    }
}

impl<const M: bool> Ctx<'_, M> {
    pub(super) fn node(&self, i: u32) -> Node {
        self.e.nodes[i as usize]
    }

    /// Charges the visit of a node to the evaluation with lists this one is
    /// part of (see [`Lists::step`]); true past its bound.
    #[inline]
    pub(super) fn stop(&self, n: u64) -> bool {
        self.meter.is_some_and(|m| m.step(n))
    }

    /// Charges `n` units to the evaluation with lists this one is part of.
    #[inline]
    fn charge(&self, n: u64) {
        if M {
            work(n);
        }
    }

    fn is_vec(&self, i: u32) -> bool {
        self.e.tys[i as usize] == Ty::Vec
    }

    fn is_angle(&self, i: u32) -> bool {
        self.e.tys[i as usize] == Ty::Num(1)
    }

    pub(super) fn full(&self, i: u32) -> Ex {
        use Val::{N, V};
        if M && self.stop(FULL_VISIT) {
            return Ex::plain(N(f64::NAN));
        }
        match self.node(i) {
            Node::Num(v) => {
                let lit = &self.e.lits[i as usize];
                Ex {
                    val: N(v),
                    dec: lit.decimal().cloned().map(Exact::Dec),
                    special: lit.special(),
                    obj: false,
                }
            }
            Node::MinusOne => Ex::plain(N(-1.0)),
            Node::Bool(b) => Ex::plain(Val::B(b, true)),
            Node::Un(Un::Not, a) => Ex::plain(self.not(a)),
            Node::Bin(op, a, b) if op.is_comparison() || op.is_logic() => {
                Ex::plain(self.logic(op, a, b))
            }
            Node::Degree => Ex {
                val: N(PI_180),
                dec: Some(Exact::Dec(degree().clone())),
                special: true,
                obj: false,
            },
            Node::Var => match *self.env {
                Env::None => Ex::plain(N(f64::NAN)),
                Env::Value(x) | Env::Pair(x, _) => Ex::plain(N(x)),
                // `AlgoDependentFunction.substituteFunction` puts a copy of
                // the argument in place of `x`: it keeps its decimal
                Env::Arg(caller, a) => caller.full(a),
            },
            Node::VarY => Ex::plain(N(match *self.env {
                Env::Pair(_, y) => y,
                _ => f64::NAN,
            })),
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
            // only a tree with lists has a list operand
            Node::Bin(Bin::Sub, a, b)
                if self.lists.is_some()
                    && self.is_list(a) != self.is_list(b)
                    && (self.is_vec(a) || self.is_vec(b)) =>
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
            Node::Apply2(f, a, b) => {
                let x = self.full_number(a);
                let y = self.full_number(b);
                let v = self
                    .meter
                    .map(|meter| super::sample_system(self.c, f, x, &[y], meter));
                Ex::plain(N(v.unwrap_or(f64::NAN)))
            }
            Node::Side(..) => Ex::plain(N(f64::NAN)),
            Node::Inverse(a) => Ex::plain(N(
                super::inverse::sample(self, a, self.variable()).unwrap_or(f64::NAN)
            )),
            Node::Dilate(src, factor, center) => {
                let (r, sx, sy) = self.dilation(factor, center);
                let f = Ctx {
                    env: &Env::Value(crate::algo::transform::dilation_preimage(
                        self.variable(),
                        r,
                        sx,
                    )),
                    ..*self
                };
                if self.e.tys[src as usize] == Ty::Bool {
                    f.full(src)
                } else {
                    Ex::plain(N(f.fast(src) * r + (-sy * r + sy)))
                }
            }
            Node::If(mode, cond, then, rest) => self.conditional(i, mode, cond, then, rest),
            Node::Item(a) | Node::Vector(a) => self.full(a),
            // the variable of a `Sequence` is a `GeoNumeric`
            Node::Local(depth, _) => Ex {
                obj: true,
                ..Ex::plain(self.local(depth).map_or(N(f64::NAN), |local| local.value))
            },
            Node::ListOp(op, l, n) => self.list_command(i, op, l, n),
            Node::Algebra(op, a, _) if !op.makes_list() => Ex::plain(self.algebra_number(op, a)),
            Node::TextCmd(op, first, count) => Ex::plain(N(self.text_number(op, first, count))),
            Node::Algebra(..)
            | Node::FunctionObject(_)
            | Node::Text
            | Node::Relation { .. }
            | Node::Base { to: true, .. } => Ex::plain(N(f64::NAN)),
            Node::Base {
                to: false,
                value,
                radix,
            } => Ex::plain(N(self.read_base_number(value, radix))),
            Node::Iteration {
                mode,
                formula,
                initial,
                count,
                ..
            } => Ex::plain(
                self.iteration_value(mode, formula, initial, count)
                    .unwrap_or_else(|| {
                        if self.is_vec(i) {
                            V(f64::NAN, f64::NAN)
                        } else {
                            N(f64::NAN)
                        }
                    }),
            ),
            Node::SystemFunction(_, a) => self.full(a),
            Node::RootList(_) | Node::FunctionAsymptotes(_) => Ex::plain(N(f64::NAN)),
            Node::Extremum(minimum, f, a, b) => Ex::plain(self.extremum(minimum, f, a, b)),
            Node::PolynomialPart(..) => Ex::plain(N(self.polynomial_value())),
            Node::ObjectDefined(id) => Ex::plain(Val::B(super::object_defined(self.c, id), true)),
            Node::Defined(a) => Ex::plain(Val::B(self.is_defined_argument(a), true)),
            Node::Filter {
                keep: false,
                list,
                condition,
                local,
                elem,
                explicit,
            } => {
                let count = self
                    .filter(list, condition, local, elem, explicit, false)
                    .map(|(_, n)| f64::from(n));
                Ex::plain(N(count.unwrap_or(f64::NAN)))
            }
            Node::Filter { .. } => Ex::plain(N(f64::NAN)),
            // a list is only read through `list`
            Node::FnName(_)
            | Node::List(..)
            | Node::ItemObject(_)
            | Node::Range(..)
            | Node::Seq(..) => Ex::plain(N(f64::NAN)),
        }
    }

    /// `¬a`; anything but a boolean gives an undefined false.
    #[inline(never)]
    fn not(&self, a: u32) -> Val {
        match self.full(a).val {
            Val::B(b, defined) => Val::B(!b, defined),
            _ => Val::B(false, false),
        }
    }

    /// A comparison or a logical operation, by the function `logic`.
    #[inline(never)]
    fn logic(&self, op: Bin, a: u32, b: u32) -> Val {
        logic(op, self.full(a).val, self.full(b).val)
    }

    /// Whether the subtree at `i` is a constant: no object, no `x`, no
    /// command; GeoGebra makes such an argument an independent number.
    pub(super) fn is_constant(&self, i: u32) -> bool {
        match self.node(i) {
            Node::Obj(_)
            | Node::Var
            | Node::VarY
            | Node::Cmd(..)
            | Node::Apply(..)
            | Node::If(..)
            | Node::Local(..)
            | Node::Algebra(..)
            | Node::ObjectDefined(_)
            | Node::Defined(_)
            | Node::Filter { .. }
            | Node::ListOp(..)
            | Node::Range(..)
            | Node::Seq(..) => false,
            Node::Bin(op, ..) if op.is_command() => false,
            node => super::children(&node).all(|child| self.is_constant(child)),
        }
    }

    /// The fast path for `f(a)` inside a function, with the argument
    /// substituted into the body.
    fn apply_fast(&self, f: Id, a: u32) -> f64 {
        let Some(e) = function_entry(self.c, f) else {
            return match self.c.def(f) {
                Def::Chart(_) if !self.stop(32) => {
                    super::function_value(self.c, f, self.fast(a)).unwrap_or(f64::NAN)
                }
                _ => f64::NAN,
            };
        };
        // the function's nodes are charged to the evaluation with lists
        // this one is part of, if any
        let env = Env::Arg(self, a);
        Ctx {
            meter: self.meter,
            ..Ctx::new(self.c, e, &env)
        }
        .fast(e.root)
    }

    /// `f(a)` in the full evaluation: inside a function the expanded tree
    /// keeps its decimals; elsewhere `f` is sampled at the value of `a`.
    fn apply_full(&self, f: Id, a: u32) -> Ex {
        let Some(e) = function_entry(self.c, f) else {
            if matches!(self.c.def(f), Def::Chart(_)) && !self.stop(32) {
                let x = self.full_number(a);
                return Ex::plain(Val::N(
                    super::function_value(self.c, f, x).unwrap_or(f64::NAN),
                ));
            }
            return Ex::plain(
                if super::function_signature(self.c, f).is_some_and(|f| f.boolean) {
                    Val::B(false, false)
                } else {
                    Val::N(f64::NAN)
                },
            );
        };
        let env = if self.e.function {
            Env::Arg(self, a)
        } else {
            Env::Value(self.full_number(a))
        };
        let inner = Ctx {
            meter: self.meter,
            ..Ctx::new(self.c, e, &env)
        };
        if self.e.function || e.root_ty() == Ty::Bool {
            inner.full(e.root)
        } else {
            Ex::plain(Val::N(inner.fast(e.root)))
        }
    }

    pub(super) fn full_number(&self, i: u32) -> f64 {
        self.full(i).num()
    }

    pub(super) fn variable(&self) -> f64 {
        match *self.env {
            Env::None => f64::NAN,
            Env::Value(x) | Env::Pair(x, _) => x,
            Env::Arg(ctx, a) => ctx.fast(a),
        }
    }

    pub(super) fn dilation(&self, factor: u32, center: u32) -> (f64, f64, f64) {
        let r = self.full_number(factor);
        let (sx, sy) = if center == super::NONE {
            (0.0, 0.0)
        } else if let Val::V(x, y) = self.full(center).val {
            (x, y)
        } else {
            (f64::NAN, f64::NAN)
        };
        (r, sx, sy)
    }

    /// The full evaluation of an operand of a numeric operation (see
    /// [`as_number`]).
    #[inline(always)]
    fn operand(&self, i: u32) -> Ex {
        let mut x = self.full(i);
        as_number(&mut x);
        x
    }

    /// `ExpressionNode.evaluateDouble`.
    pub(super) fn fast(&self, i: u32) -> f64 {
        if M && self.stop(FAST_VISIT) {
            return f64::NAN;
        }
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
                self.charge(POWER);
                let base = self.fast(a);
                if base < 0.0 {
                    if let Some(v) = self.neg_power(base, b) {
                        return v;
                    }
                }
                math::java_pow(base, self.fast(b))
            }
            Node::Un(Un::Sin, a) => {
                self.charge(FUNCTION);
                libm::sin(self.fast(a))
            }
            Node::Un(Un::Cos, a) => {
                self.charge(FUNCTION);
                libm::cos(self.fast(a))
            }
            Node::Un(Un::Sqrt, a) => self.fast(a).sqrt(),
            Node::Var => match *self.env {
                Env::None => f64::NAN,
                Env::Value(x) | Env::Pair(x, _) => x,
                Env::Arg(caller, a) => caller.fast(a),
            },
            Node::VarY => match *self.env {
                Env::Pair(_, y) => y,
                _ => f64::NAN,
            },
            Node::Apply(f, a) if self.e.function => self.apply_fast(f, a),
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
        self.charge(POWER);
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
        self.charge(FUNCTION);
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
impl<const M: bool> Ctx<'_, M> {
    /// The value of the variable of the `Sequence` of `depth`.
    #[inline(never)]
    fn local(&self, depth: u32) -> Option<&Local<'_>> {
        let mut local = self.locals;
        while let Some(l) = local {
            if l.depth == depth {
                return Some(l);
            }
            local = l.up;
        }
        None
    }

    fn is_list(&self, i: u32) -> bool {
        matches!(self.e.tys[i as usize], Ty::List(_))
    }

    /// Builds the list at `i` in the scratch buffer: where its elements
    /// start and how many there are; `None` when it is undefined.
    pub(super) fn list(&self, i: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        if lists.over() {
            return None;
        }
        match self.node(i) {
            // `AlgoDependentList`: one element per item
            Node::List(first, len) => {
                let start = lists.reserve(len)?;
                for k in 0..len {
                    if !lists.charge(1) {
                        return None;
                    }
                    let slot = self.element(first + k);
                    lists.set(start + k, slot);
                }
                Some((start, len))
            }
            Node::Item(a) => self.list(a),
            Node::Local(depth, Elem::List) => self.local(depth)?.run,
            Node::Obj(id) => self.object_list(id),
            Node::Bin(op, a, b) => self.list_binary(op, a, b),
            Node::Un(op, a) => self.list_unary(op, a),
            Node::RootList(a) => self.root_list(a),
            Node::FunctionAsymptotes(a) => self.function_asymptotes(a),
            Node::Iteration {
                collect: true,
                mode,
                formula,
                initial,
                count,
            } => self.iteration_list(mode, formula, initial, count),
            Node::Range(from, to, step) => self.range(from, to, step),
            Node::Algebra(op, a, b) => self.algebra_list(op, a, b),
            Node::TextCmd(op, first, count) => self.text_list(op, first, count),
            Node::Seq(e, from, to, step, depth) => self.sequence(e, from, to, step, depth),
            Node::Filter {
                keep: true,
                list,
                condition,
                local,
                elem,
                explicit,
            } => self.filter(list, condition, local, elem, explicit, true),
            Node::ListOp(op, l, n) if op.makes_list() => commands::list_fn(self, op, l, n),
            // an element that is a list: its elements were built with the
            // list it belongs to
            Node::ListOp(op, l, n) => self.pick(op, l, n)?.run(),
            Node::If(_, cond, then, rest) => self.list_if(cond, then, rest),
            Node::Dilate(src, factor, center) => {
                let (r, sx, sy) = self.dilation(factor, center);
                let source = self.list(src)?;
                commands::map_list(self, source, |value| {
                    crate::algo::transform::dilate_element(value, r, sx, sy)
                })
            }
            _ => None,
        }
    }

    /// One element of a list being built. A list element is built after
    /// what is there and kept; anything else is evaluated, and what that
    /// built is dropped.
    pub(super) fn element(&self, i: u32) -> Slot {
        let i = match self.node(i) {
            Node::Item(a) => a,
            Node::ItemObject(id) => return Slot::object(self.c, id),
            _ => i,
        };
        if self.is_list(i) {
            return match self.list(i) {
                Some((start, len)) => Slot::list(start, len),
                None => Slot::undefined(Kind::List),
            };
        }
        if let Node::FunctionObject(id) = self.node(i) {
            return Slot::object(self.c, id);
        }
        if let Node::Obj(id) = self.node(i) {
            if self.c.kind(id) == Kind::Point {
                return Slot::point(self.c.stored_value(id)).with_object(id);
            }
            if self.c.kind(id) == Kind::Angle {
                return Slot::object(self.c, id);
            }
        }
        let mark = self.lists.map(Lists::len);
        if self.e.tys[i as usize] == Ty::Text {
            let object = match self.node(i) {
                Node::Obj(id) => Some(id),
                _ => None,
            };
            let text = self.text(i);
            if let (Some(lists), Some(mark)) = (self.lists, mark) {
                lists.truncate(mark);
            }
            return text.map_or_else(|| Slot::undefined(Kind::Text), |s| Slot::text(s, object));
        }
        let x = self.full(i).val;
        if let (Some(lists), Some(mark)) = (self.lists, mark) {
            lists.truncate(mark);
        }
        match x {
            Val::N(v) => {
                let mut slot = Slot::number(v);
                if self.e.tys[i as usize] == Ty::Num(1) {
                    slot.kind = Kind::Angle;
                }
                if let Node::Obj(id) = self.node(i) {
                    slot.with_object(id)
                } else {
                    slot
                }
            }
            Val::V(x, y) => Slot::coords(
                super::types::vec_kind(self.c, &self.e.nodes, &self.e.tys, i),
                x,
                y,
            ),
            Val::B(false, false)
                if matches!(self.node(i), Node::Algebra(super::AlgebraOp::IsPrime, ..)) =>
            {
                Slot::undefined(Kind::Boolean)
            }
            Val::B(b, _) => Slot::boolean(b),
        }
    }

    /// The elements of a list object: read in place, as `AlgoDependentList`
    /// and the commands read the `GeoList` itself; copied to the scratch
    /// buffer when too many list objects are read in place already.
    fn object_list(&self, id: Id) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let Value::List { len } = self.c.value(id) else {
            return None;
        };
        if let Some(start) = lists.object(id) {
            return Some((start, len));
        }
        let items = super::list_slots(self.c, id)?;
        if !lists.build(items.len() as u64) {
            return None;
        }
        if !lists.charge(items.len() as u64) {
            return None;
        }
        let mut arena = lists.arena.borrow_mut();
        let base = arena.len() as u32;
        arena.extend(items.iter().map(|s| match s.run() {
            Some((start, n)) => Slot::list(start + base, n),
            None => s.clone(),
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
        if !lists.charge(u64::from(len)) {
            return None;
        }
        for k in 0..len {
            let x = left.unwrap_or_else(|| {
                ra.map_or(Val::N(f64::NAN), |(s, _)| {
                    slot_val(&lists.read(self.c, s + k))
                })
            });
            let y = right.unwrap_or_else(|| {
                rb.map_or(Val::N(f64::NAN), |(s, _)| {
                    slot_val(&lists.read(self.c, s + k))
                })
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
    #[inline(never)]
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
        let coordinate = |slot: Slot| match slot_val(&slot) {
            N(v) => v,
            Val::B(b, _) => bool_number(b),
            V(..) => f64::NAN,
        };
        let mark = lists.len();
        let xy = self.list(list).and_then(|(s, n)| {
            (n == 2).then(|| {
                (
                    coordinate(lists.read(self.c, s)),
                    coordinate(lists.read(self.c, s + 1)),
                )
            })
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
        if !lists.charge(u64::from(n)) {
            return None;
        }
        for k in 0..n {
            let slot = match slot_val(&lists.read(self.c, s + k)) {
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

    /// A list command that gives a number, a point or a boolean; the result
    /// is a helper object, and what it built is dropped.
    #[inline(never)]
    fn list_command(&self, i: u32, op: ListOp, l: u32, n: u32) -> Ex {
        let Some(lists) = self.lists else {
            return Ex::plain(Val::N(f64::NAN));
        };
        let mark = lists.len();
        let val = commands::list_value(self, i, op, l, n);
        lists.truncate(mark);
        Ex {
            obj: true,
            ..Ex::plain(val)
        }
    }

    /// A copy of `len` elements from `from` at the end of the buffer.
    pub(super) fn copy_run(&self, from: u32, len: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let start = lists.reserve(len)?;
        if !lists.charge(u64::from(len)) {
            return None;
        }
        for k in 0..len {
            lists.set(start + k, lists.read(self.c, from + k));
        }
        Some((start, len))
    }
}

/// The value of an element as `AlgoStats1D` reads it (`NumberValue`): a
/// number (NaN when undefined) or a boolean as 0 or 1; `None` for anything
/// else.
pub(super) fn number_of(slot: Slot) -> Option<f64> {
    match (slot.kind, slot.value) {
        (_, Value::Number(v)) => Some(v),
        (_, Value::Bool(b)) => Some(bool_number(b)),
        (Kind::Number | Kind::Angle, _) => Some(f64::NAN),
        _ => None,
    }
}

/// How many values `values` gives, `None` when they do not fit in what is
/// left of [`MAX_ELEMENTS`] or [`MAX_EVALUATION_WORK`]. Counting stops there, and the
/// evaluation is then past its bound, so that no loop around this one goes
/// on.
pub(super) fn count(lists: &Lists, values: impl Fn(&mut dyn FnMut(f64) -> bool)) -> Option<u32> {
    let room = MAX_ELEMENTS
        .saturating_sub(lists.built.get())
        .min(lists.end.saturating_sub(work_done()));
    let mut n: u64 = 0;
    values(&mut |_| {
        n += 1;
        n <= room
    });
    work(n);
    if n > room {
        lists.give_up();
        return None;
    }
    Some(n as u32)
}

/// An element as an operand.
pub(super) fn slot_val(s: &Slot) -> Val {
    match (s.kind, s.shown()) {
        (_, Value::Number(v)) => Val::N(v),
        (_, Value::Point { x, y } | Value::Vector { x, y }) => Val::V(x, y),
        (_, Value::Bool(b)) => Val::B(b, true),
        (Kind::Point | Kind::Vector, _) => Val::V(f64::NAN, f64::NAN),
        (Kind::Boolean, _) => Val::B(false, true),
        _ => Val::N(f64::NAN),
    }
}

/// One element of `MyList.apply` of a two-operand operation.
pub(super) fn combine(op: Bin, x: Val, y: Val) -> Slot {
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
    if !c.kind(id).is_number() {
        return Ex::plain(val);
    }
    let (dec, special) = match c.def(id) {
        Def::Expr(d) if d.angle_style == super::AngleStyle::Unbounded => {
            (super::entry(c, d).extra.exact(), false)
        }
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
        (op, N(a), N(b)) if op.is_command() => N(commands::binary(op, a, b)),
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
        (Kind::Angle, Def::Expr(d), Value::Undefined)
            if d.angle_style == super::AngleStyle::Unbounded =>
        {
            match super::raw(c, d) {
                Raw::Num(v) => Val::N(v),
                _ => Val::N(nan),
            }
        }
        (Kind::Number | Kind::Angle, _, Value::Number(v)) => Val::N(v),
        (Kind::Number | Kind::Angle, _, _) => Val::N(nan),
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
        _ if def.kind().is_number() => Val::N(nan),
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
