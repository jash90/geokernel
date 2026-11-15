// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentNumber.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/AlgebraProcessor.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Arithmetic expressions: `c = a + 2*b`, `M = (A + B) / 2`.
//!
//! An expression is parsed once into a tree of [`Node`]s, kept in a side
//! table owned by the [`Construction`]; the object itself holds only a small
//! [`ExprDef`] handle. The tree is evaluated as GeoGebra's
//! `ExpressionNode.evaluate` does (see [`eval`]), and the objects it reads
//! are its parents in the dependency graph.
//!
//! An expression that reads no object is evaluated once and becomes a free
//! number, point or vector, as in GeoGebra.
//!
//! A function `f(x) = ...` keeps its tree and is evaluated wherever it is
//! used: sampled through [`function_value`], expanded inside other
//! functions (`g(x) = f(x) + 1`), or applied to a number (`v = f(2)`).

mod eval;
mod lex;
pub(crate) mod math;
mod names;
mod parse;
mod types;

use crate::algo::vector::VectorDef;
use crate::kernel::{Construction, Def, Id, Kind, Value};

/// Handle of an expression object's tree; see the module documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprDef {
    kind: Kind,
    index: u32,
}

impl ExprDef {
    pub fn kind(&self) -> Kind {
        self.kind
    }
}

/// One node of an expression tree. Children are indices into the same
/// arena and always smaller than the index of their parent.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Node {
    /// A constant (`MyDouble`): a literal, `π`, `e`, `∞` or `?`.
    Num(f64),
    /// The factor GeoGebra puts in front of a negated term.
    MinusOne,
    /// The degree sign, `π/180` with one angle dimension.
    Degree,
    Obj(Id),
    /// The variable `x` of a function.
    Var,
    /// A command computed inside the expression.
    Cmd(Def),
    /// `(x, y)`.
    Tuple(u32, u32),
    /// `(r; φ)`.
    Polar(u32, u32),
    Un(Un, u32),
    Bin(Bin, u32, u32),
    /// A function object applied to an argument.
    Apply(Id, u32),
    /// A one-argument function name without brackets, waiting for its
    /// argument (`sin a`). Never left in a finished tree.
    FnName(Un),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Un {
    Factorial,
    Sin,
    Cos,
    Tan,
    Csc,
    Sec,
    Cot,
    Sinh,
    Cosh,
    Tanh,
    Csch,
    Sech,
    Coth,
    Asin,
    Acos,
    Atan,
    Asinh,
    Acosh,
    Atanh,
    Sqrt,
    Cbrt,
    Abs,
    Sgn,
    Floor,
    Ceil,
    Round,
    Exp,
    Ln,
    Log10,
    Log2,
    Gamma,
    X,
    Y,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bin {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Atan2,
    /// `log(b, x)`: the base comes first.
    LogB,
    Round2,
}

/// Static type of a node: a number with its angle dimension (GeoGebra's
/// `angleDim`; 1 for an angle), or a point or vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    Num(i8),
    Vec,
}

/// The last result of an expression, with infinities kept: GeoGebra keeps
/// them in a dependent number (`1/(1/0)` is 0), although the object shows
/// as undefined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Raw {
    Num(f64),
    Vec(f64, f64),
}

impl Raw {
    /// Bit equality, so that a change from `∞` to `NaN` or from `0` to `-0`
    /// still reaches the children.
    fn same(self, other: Raw) -> bool {
        match (self, other) {
            (Raw::Num(a), Raw::Num(b)) => a.to_bits() == b.to_bits(),
            (Raw::Vec(a, b), Raw::Vec(c, d)) => {
                a.to_bits() == c.to_bits() && b.to_bits() == d.to_bits()
            }
            _ => false,
        }
    }
}

pub(crate) struct Entry {
    nodes: Vec<Node>,
    tys: Vec<Ty>,
    root: u32,
    parents: Vec<Id>,
    /// Whether this is a function of `x`.
    function: bool,
    /// The objects named in the tree (a function's inputs).
    inputs: Vec<Id>,
    /// The last result; for a function, 0 when it is defined and `NaN`
    /// when not.
    raw: Raw,
    depth: u32,
    cost: u64,
    vars: u64,
}

impl Entry {
    fn new(p: parse::Parsed, function: bool) -> Self {
        Entry {
            nodes: p.nodes,
            tys: p.tys,
            root: p.root,
            parents: p.parents,
            function,
            inputs: p.inputs,
            raw: Raw::Num(f64::NAN),
            depth: p.depth,
            cost: p.cost,
            vars: p.vars,
        }
    }

    fn root_ty(&self) -> Ty {
        self.tys[self.root as usize]
    }
}

/// The expression trees of one construction.
#[derive(Default)]
pub(crate) struct Table {
    entries: Vec<Entry>,
}

/// Whether `name` is a built-in function such as `sqrt` or `x`.
pub(crate) fn is_function_name(name: &str) -> bool {
    names::is_builtin(name)
}

/// The children of a node.
pub(crate) fn children(node: &Node) -> impl Iterator<Item = u32> {
    let (a, b) = match *node {
        Node::Tuple(a, b) | Node::Polar(a, b) | Node::Bin(_, a, b) => (Some(a), Some(b)),
        Node::Un(_, a) | Node::Apply(_, a) => (Some(a), None),
        _ => (None, None),
    };
    a.into_iter().chain(b)
}

fn entry(c: &Construction, d: ExprDef) -> &Entry {
    &c.exprs.entries[d.index as usize]
}

/// Whether an expression object is an angle (one angle dimension).
pub(crate) fn is_angle(c: &Construction, d: ExprDef) -> bool {
    entry(c, d).root_ty() == Ty::Num(1)
}

/// Depth, cost and variable count of a function object's tree.
pub(crate) fn function_metrics(c: &Construction, f: Id) -> (u32, u64, u64) {
    match c.def(f) {
        Def::Expr(d) => {
            let e = entry(c, d);
            (e.depth, e.cost, e.vars)
        }
        _ => (1, 1, 0),
    }
}

/// The objects an expression object reads.
pub(crate) fn parents(c: &Construction, def: &Def) -> Vec<Id> {
    match *def {
        Def::Expr(d) => entry(c, d).parents.clone(),
        _ => Vec::new(),
    }
}

/// Evaluates an expression object for the current values of its parents.
pub(crate) fn evaluate(c: &Construction, d: ExprDef) -> Raw {
    let e = entry(c, d);
    if e.function {
        Raw::Num(if inputs_defined(c, e) { 0.0 } else { f64::NAN })
    } else {
        eval::root(c, e)
    }
}

/// `AlgoDependentFunction.inputDefined`: a function is defined when every
/// object it names is, and every command in it has a result. A number with
/// an infinite raw value counts as defined, as in GeoGebra.
fn inputs_defined(c: &Construction, e: &Entry) -> bool {
    let objects = e.inputs.iter().all(|&id| match (c.kind(id), c.def(id)) {
        (Kind::Number, Def::Expr(d)) => !matches!(raw(c, d), Raw::Num(v) if v.is_nan()),
        _ => c.value(id).is_defined(),
    });
    objects
        && e.nodes.iter().all(|n| match *n {
            Node::Cmd(def) => match c.try_compute(def) {
                Some(Value::Number(v)) => !v.is_nan(),
                other => other.is_some(),
            },
            _ => true,
        })
}

/// Stores the result of [`evaluate`]; tells whether it changed. A function
/// always counts as changed: its values depend on its inputs even when it
/// stays defined.
pub(crate) fn store(c: &mut Construction, d: ExprDef, raw: Raw) -> bool {
    let e = &mut c.exprs.entries[d.index as usize];
    let changed = e.function || !e.raw.same(raw);
    e.raw = raw;
    changed
}

/// `GeoFunction.value`: the value of the function `f` at `x`, `None` when
/// `f` is not a defined function.
pub(crate) fn function_value(c: &Construction, f: Id, x: f64) -> Option<f64> {
    match (c.def(f), c.value(f)) {
        (Def::Expr(d), Value::Function) => Some(eval::sample(c, entry(c, d), x)),
        _ => None,
    }
}

/// The last stored result.
pub(crate) fn raw(c: &Construction, d: ExprDef) -> Raw {
    entry(c, d).raw
}

/// The value an object shows for a result. A dependent point with an
/// infinite coordinate is undefined (`AlgoDependentPoint`); a number or a
/// vector keeps its infinity in [`Raw`] but shows as undefined too.
pub(crate) fn value(d: ExprDef, raw: Raw) -> Value {
    match (d.kind, raw) {
        (Kind::Number, Raw::Num(v)) if v.is_finite() => Value::Number(v),
        (Kind::Point, Raw::Vec(x, y)) if x.is_finite() && y.is_finite() => Value::Point { x, y },
        (Kind::Vector, Raw::Vec(x, y)) if x.is_finite() && y.is_finite() => Value::Vector { x, y },
        (Kind::Function, Raw::Num(v)) if !v.is_nan() => Value::Function,
        _ => Value::Undefined,
    }
}

/// Defines `name = rhs` as an expression. `vector_label` tells whether a
/// coordinate result is a vector (lower-case label) or a point.
pub(crate) fn define(
    c: &mut Construction,
    name: &str,
    rhs: &str,
    vector_label: bool,
) -> Result<Id, String> {
    let parsed = parse::parse(c, rhs)?;
    if parsed.uses_var {
        return Err("`x` is only allowed in a function, `f(x) = ...`".into());
    }
    let kind = match parsed.tys[parsed.root as usize] {
        Ty::Num(_) => Kind::Number,
        Ty::Vec if vector_label => Kind::Vector,
        Ty::Vec => Kind::Point,
    };
    add(c, name, parsed, kind)
}

/// Defines the function `name(x) = rhs`. A function that reads no object
/// (`f(x) = 3`) stays a function.
pub(crate) fn define_function(c: &mut Construction, name: &str, rhs: &str) -> Result<Id, String> {
    let parsed = parse::parse(c, rhs)?;
    if parsed.tys[parsed.root as usize] == Ty::Vec {
        return Err("a function must give a number".into());
    }
    add(c, name, parsed, Kind::Function)
}

fn add(c: &mut Construction, name: &str, parsed: parse::Parsed, kind: Kind) -> Result<Id, String> {
    let function = kind == Kind::Function;
    let constant = parsed.parents.is_empty() && !function;
    let vector_label = kind == Kind::Vector;
    let entry = Entry::new(parsed, function);
    if constant {
        // `AlgebraProcessor`: an expression without objects is free.
        let free = match eval::root(c, &entry) {
            Raw::Num(v) if v.is_finite() => Some(Def::Number(v)),
            Raw::Vec(x, y) if x.is_finite() && y.is_finite() => Some(if vector_label {
                Def::Vector(VectorDef::Free { x, y })
            } else {
                Def::FreePoint { x, y }
            }),
            _ => None,
        };
        if let Some(def) = free {
            return c.add(name, def).map_err(|e| e.to_string());
        }
    }
    let index = u32::try_from(c.exprs.entries.len()).map_err(|_| "too many expressions")?;
    c.exprs.entries.push(entry);
    let def = Def::Expr(ExprDef { kind, index });
    c.add(name, def).map_err(|e| {
        c.exprs.entries.pop();
        e.to_string()
    })
}

#[cfg(test)]
mod tests;
