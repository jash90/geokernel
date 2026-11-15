// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentNumber.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/AlgebraProcessor.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentBoolean.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentListExpression.java
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
//! Comparisons and logical operators make boolean objects (`b = a < 3`,
//! `AlgoDependentBoolean`): compared as doubles within 1e-8, never
//! undefined (a comparison of an undefined number is false). A boolean is
//! also the number 1 or 0. A constant boolean (`b = 1 < 2`) stays an
//! expression object: the kernel has no free boolean.
//!
//! Lists (`{1, 2, 3}`, `{A, B}`, `Sequence(k^2, k, 1, n)`) are list
//! objects whose elements are kept, flattened, in their entry (see
//! [`list`]); `l(n)`, `Element`, `Length` and `Sum` read them, and
//! arithmetic combines them element by element. Every list is an expression
//! object, also a constant one: the kernel has no free list. A list is not
//! allowed in a function of `x`.
//!
//! A function `f(x) = ...` keeps its tree and is evaluated wherever it is
//! used: sampled through [`function_value`], expanded inside other
//! functions (`g(x) = f(x) + 1`), or applied to a number (`v = f(2)`).
//!
//! An [`ExprDef`] is valid only in the construction that made it: adding it
//! anywhere else, or adding it a second time, is
//! [`Error::UnknownObject`](crate::Error::UnknownObject). To rebuild an
//! expression object elsewhere, take its tree with
//! [`Construction::expression`] and add it with
//! [`Construction::add_expression`]; the tree is checked again there.

mod decimal;
mod eval;
mod lex;
pub(crate) use lex::{is_name_char, plain_number};
mod list;
use list::Slot;
pub use list::{Element, List};
pub(crate) mod math;
mod names;
mod parse;
mod twonum;
mod types;
mod wide;

use crate::algo::intersect::Hint;
use crate::algo::vector::VectorDef;
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use decimal::Dec;

/// Handle of an expression object's tree, valid only in the construction
/// that made it; see the module documentation.
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
    /// A command computed inside the expression, with the intersection
    /// hint it got when the expression was parsed.
    Cmd(Def, Hint),
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
    /// `true` or `false` (`MyBoolean`).
    Bool(bool),
    /// `If(condition, then, rest)`: `rest` is the else branch, the next
    /// condition of a longer `If` (another `If` node of the same mode), or
    /// [`NONE`].
    If(IfMode, u32, u32, u32),
    /// `{a, b, ...}`: the elements are the [`Node::Item`]s from the first
    /// index on, as many as the second says, one after the other.
    List(u32, u32),
    /// One element of a list literal.
    Item(u32),
    /// A command on a list and an optional number ([`NONE`] when absent).
    ListOp(ListOp, u32, u32),
    /// `Sequence(n)`, `Sequence(a, b)` and `Sequence(a, b, step)`
    /// (`AlgoSequenceRange`): from ([`NONE`] for `Sequence(n)`), to, step
    /// ([`NONE`] when absent).
    Range(u32, u32, u32),
    /// `Sequence(expression, k, from, to, step)` (`AlgoSequence`): the
    /// expression, from, to, step ([`NONE`] when absent), and the depth of
    /// its variable `k`, a [`Node::Local`].
    Seq(u32, u32, u32, u32, u32),
    /// The variable of the enclosing `Sequence` of this depth (0 for the
    /// outermost).
    Local(u32),
}

/// The commands that read a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListOp {
    /// `Length(list)`.
    Length,
    /// `Sum(list)` and `Sum(list, n)` of numbers (`AlgoStats1D`).
    Sum,
    /// `l(n)`: rounded, from 1; a negative index counts from the end
    /// (`ExpressionNodeEvaluator.handleElementOf`).
    Index,
    /// `Element(l, n)`: rounded, from 1 (`AlgoListElement`).
    Element,
}

/// No node: the missing else branch of an `If`.
pub(crate) const NONE: u32 = u32::MAX;

/// How an `If` decides (`CmdIf`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IfMode {
    /// `AlgoIf`, the command of an expression that is not a function of
    /// `x` in its conditions: the first true condition picks its branch;
    /// an undefined condition is false; no true condition and no else
    /// branch is undefined. The result has the most general type of the
    /// branches: a number in a point result is the point `(v, 0)`.
    Algo,
    /// `Operation.IF_ELSE`, a function with one condition in `x` that
    /// reads no object, and an else branch: an undefined condition gives
    /// `NaN`. A condition that reads an object is a helper function whose
    /// value is a defined boolean (`CmdIf.wrap`), so it is [`IfMode::List`].
    Else,
    /// `Operation.IF` and `IF_LIST`, a function with conditions in `x`:
    /// the first true condition picks its branch, an undefined one is
    /// skipped; otherwise the else branch or `NaN`.
    List,
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
    /// `asind`, `acosd`, `atand`: the inverse functions that give an
    /// angle (`sin^-1 x`).
    AsinD,
    AcosD,
    AtanD,
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
    /// `¬a` or `!a`.
    Not,
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
    /// The commands of two numbers (see [`twonum`]).
    Mod,
    /// `Div`, the integer quotient.
    IntDiv,
    Gcd,
    Lcm,
    Max,
    Min,
    /// `<`, `>`, `<=`, `>=`: numbers compared with the tolerance 1e-8.
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    /// `==` and `!=`: numbers, or points and vectors coordinate by
    /// coordinate, equal within 1e-8.
    Equal,
    NotEqual,
    /// `&&` (also what a chain such as `1 < a < 3` joins its comparisons
    /// with, `AND_INTERVAL`), `||`, `⊕` and `->`.
    And,
    Or,
    Xor,
    Implies,
}

impl Bin {
    /// Whether this is one of the commands of two numbers.
    fn is_command(self) -> bool {
        matches!(
            self,
            Bin::Mod | Bin::IntDiv | Bin::Gcd | Bin::Lcm | Bin::Max | Bin::Min
        )
    }

    /// Whether this compares two values.
    fn is_comparison(self) -> bool {
        matches!(
            self,
            Bin::Less
                | Bin::Greater
                | Bin::LessEqual
                | Bin::GreaterEqual
                | Bin::Equal
                | Bin::NotEqual
        )
    }

    /// Whether this joins two booleans.
    fn is_logic(self) -> bool {
        matches!(self, Bin::And | Bin::Or | Bin::Xor | Bin::Implies)
    }
}

/// Static type of a node: a number with its angle dimension (GeoGebra's
/// `angleDim`; 1 for an angle), a point or vector, or a boolean. A boolean
/// is also a number, 1 or 0, wherever a number is expected (`MyBoolean`
/// is a `NumberValue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    Num(i8),
    Vec,
    Bool,
    List(Elem),
}

/// The type of the elements of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Elem {
    Num,
    /// Points and vectors.
    Vec,
    Bool,
    List,
    /// No element: `{}`.
    Empty,
    /// Elements of different types, such as `{1, (1, 2)}`.
    Any,
}

/// The last result of an expression, with infinities kept: GeoGebra keeps
/// them in a dependent number (`1/(1/0)` is 0), although the object shows
/// as undefined. A boolean is kept as 1 or 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Raw {
    Num(f64),
    Vec(f64, f64),
    /// A defined list of this many elements, kept in the entry.
    List(u32),
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
            (Raw::List(a), Raw::List(b)) => a == b,
            _ => false,
        }
    }
}

/// What GeoGebra's number object of a constant node is: a `MySpecialDouble`
/// (a literal or a named constant, with the decimal it was written as, see
/// [`decimal`]) or, by default, a plain `MyDouble` that the parser made.
#[derive(Clone, Debug, Default)]
pub(crate) struct Lit {
    pub(crate) special: bool,
    pub(crate) dec: Option<Dec>,
}

pub(crate) struct Entry {
    nodes: Vec<Node>,
    lits: Vec<Lit>,
    tys: Vec<Ty>,
    root: u32,
    parents: Vec<Id>,
    /// Whether this is a function of `x`.
    function: bool,
    /// Whether a function's tree has an `If`, which changes what decides
    /// whether it is defined (see [`inputs_defined`]).
    has_if: bool,
    /// The objects named in the tree (a function's inputs).
    inputs: Vec<Id>,
    /// The last result; for a function, 0 when it is defined and `NaN`
    /// when not.
    raw: Raw,
    /// The exact decimal of the last result of a number
    /// (`GeoNumeric.exactValue`, set by `AlgoDependentNumber`).
    dec: Option<eval::Exact>,
    /// Whether the tree has a list in it, which its evaluation builds in a
    /// scratch buffer.
    lists: bool,
    /// The elements of a list result (see [`list`]).
    items: Vec<Slot>,
    depth: u32,
    cost: u64,
    vars: u64,
}

impl Entry {
    fn new(p: parse::Parsed, function: bool) -> Self {
        let has_if = function && p.nodes.iter().any(|n| matches!(n, Node::If(..)));
        let lists =
            p.nodes.iter().any(is_list_node) || matches!(p.tys[p.root as usize], Ty::List(_));
        Entry {
            has_if,
            lists,
            items: Vec::new(),
            nodes: p.nodes,
            lits: p.lits,
            tys: p.tys,
            root: p.root,
            parents: p.parents,
            function,
            inputs: p.inputs,
            raw: Raw::Num(f64::NAN),
            dec: None,
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
    /// The tree being added, the only one a new [`ExprDef`] may name.
    pending: Option<u32>,
    /// The exact decimals of free numbers that a decimal literal or a
    /// constant expression made and that were never changed since
    /// (`GeoNumeric.exactValue` with a constant definition), by object.
    free: Vec<Option<Dec>>,
    /// Scratch buffers of list evaluations, kept so that a recompute does
    /// not allocate: the lists being built, and a list result before it is
    /// compared with the last one.
    arena: Vec<Slot>,
    out: Vec<Slot>,
}

/// An expression object's tree, owned: what [`Construction::expression`]
/// hands out and [`Construction::add_expression`] adds again. Like a
/// [`Def`], it names other objects by [`Id`], so it belongs in a
/// construction whose objects up to those ids are the same ones, such as a
/// rebuild of the construction it came from.
#[derive(Clone, Debug)]
pub struct Expression {
    kind: Kind,
    form: Form,
}

#[derive(Clone, Debug)]
enum Form {
    Tree {
        nodes: Vec<Node>,
        lits: Vec<Lit>,
        root: u32,
    },
    /// A free number with its exact decimal (see [`Table`]).
    Exact { def: Def, dec: Dec },
}

impl Expression {
    /// The kind of object the expression defines.
    pub fn kind(&self) -> Kind {
        self.kind
    }
}

impl Table {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Drops the trees added after the first `len`, and what is known about
    /// the objects from `objects` on (see `Construction::rollback`).
    pub(crate) fn truncate(&mut self, len: usize, objects: usize) {
        self.entries.truncate(len);
        self.free.truncate(objects);
    }
}

/// Whether `name` is a built-in function such as `sqrt` or `x`.
pub(crate) fn is_function_name(name: &str) -> bool {
    names::is_builtin(name)
}

/// The children of a node.
pub(crate) fn children(node: &Node) -> impl Iterator<Item = u32> {
    let mut fixed = [NONE; 4];
    let mut run = 0..0;
    match *node {
        Node::Tuple(a, b) | Node::Polar(a, b) | Node::Bin(_, a, b) | Node::ListOp(_, a, b) => {
            fixed[..2].copy_from_slice(&[a, b]);
        }
        Node::Un(_, a) | Node::Apply(_, a) | Node::Item(a) => fixed[0] = a,
        Node::If(_, a, b, c) | Node::Range(a, b, c) => fixed[..3].copy_from_slice(&[a, b, c]),
        Node::Seq(a, b, c, d, _) => fixed = [a, b, c, d],
        Node::List(first, len) => run = first..first.saturating_add(len),
        _ => {}
    }
    fixed.into_iter().filter(|&c| c != NONE).chain(run)
}

/// Whether a node makes or reads a list, which a command does in GeoGebra.
fn is_list_node(node: &Node) -> bool {
    matches!(
        node,
        Node::List(..)
            | Node::Item(_)
            | Node::ListOp(..)
            | Node::Range(..)
            | Node::Seq(..)
            | Node::Local(_)
    )
}

/// The commands that expressions compute themselves; `Length` and `Sum`
/// also of a list.
pub(crate) fn is_expression_command(name: &str) -> bool {
    matches!(name, "If" | "Sequence" | "Element" | "Length" | "Sum")
}

fn entry(c: &Construction, d: ExprDef) -> &Entry {
    &c.exprs.entries[d.index as usize]
}

/// `Construction::check` of an expression handle: only the tree being
/// added may be named, so a handle from another construction, or one that
/// an object already holds, is rejected.
pub(crate) fn check_handle(c: &Construction, d: ExprDef) -> Result<(), Error> {
    if c.exprs.pending == Some(d.index) {
        Ok(())
    } else {
        Err(Error::UnknownObject)
    }
}

/// The exact decimal of the free number `id`, if it has one.
pub(crate) fn free_decimal(c: &Construction, id: Id) -> Option<&Dec> {
    c.exprs.free.get(id.0).and_then(Option::as_ref)
}

fn set_free_decimal(c: &mut Construction, id: Id, d: Dec) {
    let free = &mut c.exprs.free;
    if free.len() <= id.0 {
        free.resize(id.0 + 1, None);
    }
    free[id.0] = Some(d);
}

/// Records the decimal of the free number `id` that a script line made from
/// the decimal literal `text` (a `MySpecialDouble`).
pub(crate) fn record_literal(c: &mut Construction, id: Id, text: &str) {
    if let (Some(_), Some(d)) = (plain_number(text), Dec::parse(text.trim())) {
        set_free_decimal(c, id, d);
    }
}

/// Forgets the exact decimal of a free number that is being changed
/// (`GeoNumeric.setValue`); whether it had one.
pub(crate) fn forget_decimal(c: &mut Construction, id: Id) -> bool {
    c.exprs.free.get_mut(id.0).and_then(Option::take).is_some()
}

/// The tree of the expression object `id`, or the decimal of an exact free
/// number, if it is one of these.
pub(crate) fn expression(c: &Construction, id: Id) -> Option<Expression> {
    if id.0 >= c.len() {
        return None;
    }
    match c.def(id) {
        Def::Expr(d) => {
            let e = entry(c, d);
            Some(Expression {
                kind: d.kind,
                form: Form::Tree {
                    nodes: e.nodes.clone(),
                    lits: e.lits.clone(),
                    root: e.root,
                },
            })
        }
        def @ (Def::Number(_) | Def::Angle(_)) => Some(Expression {
            kind: Kind::Number,
            form: Form::Exact {
                def,
                dec: free_decimal(c, id)?.clone(),
            },
        }),
        _ => None,
    }
}

/// Adds the expression `e` as `name`, checking its tree against this
/// construction as a parsed line is checked. A tree that names a missing
/// object, an object of another kind, or a command whose arguments do not
/// fit is [`Error::UnknownObject`].
pub(crate) fn add_expression(
    c: &mut Construction,
    name: &str,
    e: &Expression,
) -> Result<Id, Error> {
    let (nodes, lits, root) = match &e.form {
        Form::Tree { nodes, lits, root } => (nodes, lits, *root),
        Form::Exact { def, dec } => {
            let id = c.add(name, *def)?;
            set_free_decimal(c, id, dec.clone());
            return Ok(id);
        }
    };
    let parsed = reparse(c, e.kind, nodes, lits, root).ok_or(Error::UnknownObject)?;
    let fits = matches!(
        (e.kind, parsed.tys[parsed.root as usize]),
        (Kind::Function | Kind::Number, Ty::Num(_))
            | (Kind::Boolean, Ty::Bool)
            | (Kind::Point | Kind::Vector, Ty::Vec)
            | (Kind::List, Ty::List(_))
    );
    if !fits {
        return Err(Error::UnknownObject);
    }
    add_parsed(c, name, parsed, e.kind)
}

/// What [`parse::parse`] would have given for a tree, or `None` when the
/// tree does not fit this construction.
fn reparse(
    c: &Construction,
    kind: Kind,
    nodes: &[Node],
    lits: &[Lit],
    root: u32,
) -> Option<parse::Parsed> {
    let n = nodes.len();
    if root as usize >= n || lits.len() != n {
        return None;
    }
    let mut parents: Vec<Id> = Vec::new();
    let mut inputs: Vec<Id> = Vec::new();
    let mut read = |id: Id, input: bool| {
        if !parents.contains(&id) {
            parents.push(id);
        }
        if input && !inputs.contains(&id) {
            inputs.push(id);
        }
    };
    let mut uses_var = false;
    // the intersection hints are decided again for this construction, as
    // creation does: the tree's own may name objects that are not here
    let mut nodes = nodes.to_vec();
    for (i, node) in nodes.iter_mut().enumerate() {
        if children(node).any(|child| child as usize >= i) {
            return None;
        }
        match *node {
            Node::Obj(id) => {
                (id.0 < c.len()).then_some(())?;
                read(id, true);
            }
            Node::Apply(f, _) => {
                (f.0 < c.len() && c.kind(f) == Kind::Function).then_some(())?;
                read(f, true);
            }
            Node::Cmd(def, _) => {
                c.check(&def).ok()?;
                *node = Node::Cmd(def, crate::algo::intersect::hint(c, &def));
                for &p in def.parents().as_slice() {
                    read(p, false);
                }
                for p in crate::algo::intersect::hidden_parents(c, &def) {
                    read(p, false);
                }
                for p in crate::algo::vector::hidden_parents(c, &def) {
                    read(p, false);
                }
            }
            Node::Var => uses_var = true,
            _ => {}
        }
    }
    let (tys, depth, cost, vars) = types::check(c, &nodes, root).ok()?;
    let r = root as usize;
    if depth[r] > parse::MAX_DEPTH || cost[r] > parse::MAX_COST {
        return None;
    }
    if uses_var && kind != Kind::Function
        || kind == Kind::Function && nodes.iter().any(is_list_node)
    {
        return None;
    }
    Some(parse::Parsed {
        nodes,
        lits: lits.to_vec(),
        tys,
        root,
        parents,
        inputs,
        uses_var,
        depth: depth[r],
        cost: cost[r],
        vars: vars[r],
    })
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

/// A result of [`evaluate`]: the raw value and the exact decimal of a
/// number.
pub(crate) struct Outcome {
    pub(crate) raw: Raw,
    dec: Option<eval::Exact>,
}

/// Evaluates an expression object for the current values of its parents;
/// the elements of a list result go to `out`. `lists` is the scratch of a
/// tree with lists.
fn evaluate(
    c: &Construction,
    d: ExprDef,
    lists: Option<&eval::Lists>,
    out: &mut Vec<Slot>,
) -> Outcome {
    let e = entry(c, d);
    if e.function {
        let raw = Raw::Num(if inputs_defined(c, e) { 0.0 } else { f64::NAN });
        Outcome { raw, dec: None }
    } else if let (Ty::List(_), Some(lists)) = (e.root_ty(), lists) {
        let raw = match eval::root_list(c, e, lists, out) {
            Some(len) => Raw::List(len),
            None => {
                out.clear();
                Raw::Num(f64::NAN)
            }
        };
        Outcome { raw, dec: None }
    } else {
        let (raw, dec) = eval::root(c, e, lists);
        Outcome { raw, dec }
    }
}

/// Recomputes the expression object `d` and keeps its result; its value,
/// and whether it changed (see [`store`]). The scratch buffers of a tree
/// with lists are taken from the table and given back, so a recompute
/// allocates nothing once they have grown.
pub(crate) fn update(c: &mut Construction, d: ExprDef) -> (Value, bool) {
    if !entry(c, d).lists {
        let out = evaluate(c, d, None, &mut Vec::new());
        return (value(d, out.raw), store(c, d, out));
    }
    let lists = eval::Lists::new(std::mem::take(&mut c.exprs.arena));
    let mut items = std::mem::take(&mut c.exprs.out);
    items.clear();
    let out = evaluate(c, d, Some(&lists), &mut items);
    c.exprs.arena = lists.into_arena();
    let e = &mut c.exprs.entries[d.index as usize];
    let items_changed = !list::same_slots(&e.items, &items);
    if items_changed {
        e.items.clear();
        e.items.extend_from_slice(&items);
    }
    c.exprs.out = items;
    let v = value(d, out.raw);
    let changed = store(c, d, out);
    (v, changed || items_changed)
}

/// `AlgoDependentFunction.inputDefined`: a function is defined when every
/// object it names is, and every command in it has a result. A number with
/// an infinite raw value counts as defined, as in GeoGebra.
fn inputs_defined(c: &Construction, e: &Entry) -> bool {
    if e.has_if {
        return defined_below(c, e, e.root);
    }
    e.inputs.iter().all(|&id| object_defined(c, id))
        && e.nodes.iter().all(|n| match *n {
            Node::Cmd(def, hint) => command_defined(c, def, hint),
            _ => true,
        })
}

fn object_defined(c: &Construction, id: Id) -> bool {
    match (c.kind(id), c.def(id)) {
        (Kind::Number, Def::Expr(d)) => !matches!(raw(c, d), Raw::Num(v) if v.is_nan()),
        _ => c.value(id).is_defined(),
    }
}

fn command_defined(c: &Construction, def: Def, hint: crate::algo::intersect::Hint) -> bool {
    match c.command_value(def, hint) {
        Some(Value::Number(v)) => !v.is_nan(),
        other => other.is_some(),
    }
}

/// `inputDefined` of a function with an `If`: the objects inside an `If`
/// operation do not count (`ExpressionNode.getUnconditionalVars`); an
/// `AlgoIf` (an `If` whose conditions do not read `x`) is a helper
/// function, defined when a condition is true and the branch it picks is.
fn defined_below(c: &Construction, e: &Entry, i: u32) -> bool {
    match e.nodes[i as usize] {
        Node::Obj(id) => object_defined(c, id),
        Node::Apply(f, a) => object_defined(c, f) && defined_below(c, e, a),
        Node::Cmd(def, hint) => command_defined(c, def, hint),
        Node::If(IfMode::Algo, cond, then, rest) => {
            if eval::condition(c, e, cond) {
                defined_below(c, e, then)
            } else {
                rest != NONE && defined_below(c, e, rest)
            }
        }
        Node::If(..) => true,
        node => children(&node).all(|child| defined_below(c, e, child)),
    }
}

/// The value of an expression object for the current values of its
/// parents, without storing it.
#[inline(never)]
pub(crate) fn current(c: &Construction, d: ExprDef) -> Value {
    let lists = entry(c, d).lists.then(|| eval::Lists::new(Vec::new()));
    value(d, evaluate(c, d, lists.as_ref(), &mut Vec::new()).raw)
}

/// The elements of the list object `id`, when it is a defined list.
pub(crate) fn list(c: &Construction, id: Id) -> Option<List<'_>> {
    match (c.def(id), c.value(id)) {
        (Def::Expr(d), Value::List { len }) => Some(List::new(&entry(c, d).items, len)),
        _ => None,
    }
}

/// Stores the result of [`evaluate`]; tells whether it changed. A function
/// always counts as changed: its values depend on its inputs even when it
/// stays defined.
/// Keeps a result; whether it changed (a decimal of the same value counts:
/// it changes what reads the number).
pub(crate) fn store(c: &mut Construction, d: ExprDef, out: Outcome) -> bool {
    let e = &mut c.exprs.entries[d.index as usize];
    let changed = e.function || !e.raw.same(out.raw) || e.dec != out.dec;
    e.raw = out.raw;
    e.dec = out.dec;
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
        // `AlgoDependentBoolean`: a boolean is never undefined, a
        // comparison of an undefined number is false
        (Kind::Boolean, Raw::Num(v)) => Value::Bool(v == 1.0),
        (Kind::List, Raw::List(len)) => Value::List { len },
        _ => Value::Undefined,
    }
}

/// Defines `name = rhs` as an expression. `vector_label` tells whether a
/// coordinate result is a vector (lower-case label) or a point. An
/// expression in `x` defines a function, as `b = x + 1` does in GeoGebra.
pub(crate) fn define(
    c: &mut Construction,
    name: &str,
    rhs: &str,
    vector_label: bool,
) -> Result<Id, String> {
    let parsed = parse::parse(c, rhs)?;
    if parsed.uses_var {
        return function(c, name, parsed);
    }
    let kind = match parsed.tys[parsed.root as usize] {
        Ty::Num(_) => Kind::Number,
        Ty::Vec if vector_label => Kind::Vector,
        Ty::Vec => Kind::Point,
        Ty::Bool => Kind::Boolean,
        Ty::List(_) => Kind::List,
    };
    add(c, name, parsed, kind)
}

/// Defines the function `name(x) = rhs`. A function that reads no object
/// (`f(x) = 3`) stays a function.
pub(crate) fn define_function(c: &mut Construction, name: &str, rhs: &str) -> Result<Id, String> {
    function(c, name, parse::parse(c, rhs)?)
}

fn function(c: &mut Construction, name: &str, parsed: parse::Parsed) -> Result<Id, String> {
    match parsed.tys[parsed.root as usize] {
        Ty::Vec => return Err("a function must give a number".into()),
        Ty::Bool => return Err("a function that gives a boolean is not supported".into()),
        Ty::List(_) => return Err("a function cannot give a list".into()),
        Ty::Num(_) => {}
    }
    if parsed.nodes.iter().any(is_list_node) {
        return Err("a list in a function is not supported".into());
    }
    add(c, name, parsed, Kind::Function)
}

fn add(c: &mut Construction, name: &str, parsed: parse::Parsed, kind: Kind) -> Result<Id, String> {
    add_parsed(c, name, parsed, kind).map_err(|e| e.to_string())
}

fn add_parsed(
    c: &mut Construction,
    name: &str,
    parsed: parse::Parsed,
    kind: Kind,
) -> Result<Id, Error> {
    let function = kind == Kind::Function;
    // `AlgebraProcessor.processNumber`: independent unless it reads an
    // object; a command such as `Mod(7, 3)` makes a helper object, so an
    // expression with one is dependent even on constants.
    let has_command = parsed.nodes.iter().any(|n| {
        matches!(n, Node::Bin(op, ..) if op.is_command())
            || matches!(n, Node::Cmd(..) | Node::If(IfMode::Algo, ..))
            || is_list_node(n)
    });
    // a constant boolean (`b = 1 < 2`) or list stays an expression: there
    // is no free boolean or list object
    let constant = parsed.parents.is_empty()
        && !function
        && !has_command
        && !matches!(kind, Kind::Boolean | Kind::List);
    let vector_label = kind == Kind::Vector;
    let entry = Entry::new(parsed, function);
    if constant {
        // `AlgebraProcessor`: an expression without objects is free.
        let angle = entry.root_ty() == Ty::Num(1);
        let (raw, dec) = eval::root(c, &entry, None);
        let free = match raw {
            Raw::Num(v) if v.is_finite() && angle => Some(Def::Angle(v)),
            Raw::Num(v) if v.is_finite() => Some(Def::Number(v)),
            Raw::Vec(x, y) if x.is_finite() && y.is_finite() => Some(if vector_label {
                Def::Vector(VectorDef::Free { x, y })
            } else {
                Def::FreePoint { x, y }
            }),
            Raw::List(_) | Raw::Num(_) | Raw::Vec(..) => None,
        };
        if let Some(def) = free {
            let id = c.add(name, def)?;
            if let (Some(d), Kind::Number, Raw::Num(v)) = (dec, kind, raw) {
                if let Some(d) = d.get(v) {
                    set_free_decimal(c, id, d.into_owned());
                }
            }
            return Ok(id);
        }
    }
    let index = u32::try_from(c.exprs.entries.len()).map_err(|_| Error::UnknownObject)?;
    c.exprs.entries.push(entry);
    c.exprs.pending = Some(index);
    let result = c.add(name, Def::Expr(ExprDef { kind, index }));
    c.exprs.pending = None;
    if result.is_err() {
        c.exprs.entries.pop();
    }
    result
}

#[cfg(test)]
mod tests;
